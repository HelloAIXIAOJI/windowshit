//! fc —— 比较两个文件（复刻 Windows fc.exe）。
//!
//! 原版 fc 输出永远是英文（不随系统语言变化），故英文硬编码。
//! 实测对齐的行为：
//! - 相同：`FC: no differences encountered`，退出码 0
//! - 不同：`***** file1` + 块 + `***** file2` + 块 + `*****`，退出码 1
//! - 文件不存在：`FC: cannot open X - No such file or folder`，退出码 2
//! - 无参数：`FC: Insufficient number of file specifications`，退出码 255
//! - /n 行号、/a 缩写（块>3行显示首尾+省略号）、/c 忽略大小写、/b 二进制

use std::env;
use std::fs;
use std::process::ExitCode;

const HELP: &str = "fc [/a] [/c] [/l] [/n] [/t] [/u] [/w] [/b] [options] file1 file2

Compares two files or sets of files and displays the differences between them.

/a     Displays only first and last lines for each set of differences.
/b     Performs a binary comparison.
/c     Disregards the case of letters.
/l     Compares files in ASCII mode.
/n     Displays the line numbers on an ASCII comparison.
/t     Does not expand tabs to spaces.
/u     Compares files as Unicode text files.
/w     Compresses white space (tabs and spaces) during comparison.
/????  Displays this help message.

Note:
    Use fc to compare two files. The first file is file1, the second is file2.";

/// Windows 上原版把路径显示为大写，其它平台原样。
fn display_path(p: &str) -> String {
    #[cfg(windows)]
    {
        p.to_uppercase()
    }
    #[cfg(not(windows))]
    {
        p.to_string()
    }
}

/// 把字节流按行拆分（支持 \r\n 与 \n），返回行 Vec（去掉行尾换行符）。
fn read_lines(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes);
    text.lines().map(|l| l.to_string()).collect()
}

/// 按 UTF-16LE 读行（/u）。
fn read_lines_utf16(bytes: &[u8]) -> Vec<String> {
    // 跳过 BOM
    let start = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        1
    } else {
        0
    };
    let mut u16s = Vec::new();
    let mut i = start * 2;
    while i + 1 < bytes.len() {
        u16s.push(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
        i += 2;
    }
    let text = String::from_utf16_lossy(&u16s);
    text.lines().map(|l| l.to_string()).collect()
}

/// 归一化用于比较（/c 忽略大小写、/w 压缩空白）。
fn norm(s: &str, ignore_case: bool, compress_ws: bool) -> String {
    let mut out = String::new();
    let mut last_space = false;
    for c in s.chars() {
        let c = if ignore_case {
            c.to_lowercase().next().unwrap_or(c)
        } else {
            c
        };
        if compress_ws && c.is_whitespace() {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
            continue;
        }
        last_space = false;
        out.push(c);
    }
    out
}

/// 按块输出差异。`lines1`/`lines2` 已按行号对齐比较。
/// 返回 true 表示有差异。
fn compare_text(
    lines1: &[String],
    lines2: &[String],
    f1: &str,
    f2: &str,
    line_num: bool,
    abbreviate: bool,
    ignore_case: bool,
    compress_ws: bool,
) -> bool {
    let n = lines1.len().max(lines2.len());
    // diff[i]：该行（1-based）是否不同（含缺行）
    let mut diff = vec![false; n];
    let mut any = false;
    for i in 0..n {
        let a = lines1.get(i).map(|l| norm(l, ignore_case, compress_ws));
        let b = lines2.get(i).map(|l| norm(l, ignore_case, compress_ws));
        if a != b {
            diff[i] = true;
            any = true;
        }
    }
    if !any {
        println!("FC: no differences encountered");
        return false;
    }

    // 差异块（行范围，[start, end) 0-based）：
    // - 每个块含差异行 + 差异前一行
    // - 差异块之间若被 <=2 行匹配隔开则合并（模拟原版重同步阈值）
    let mut blocks: Vec<(usize, usize)> = Vec::new();
    let mut i = 0usize;
    while i < n {
        if !diff[i] {
            i += 1;
            continue;
        }
        // 当前差异段末尾
        let mut e = i;
        while e < n && diff[e] {
            e += 1;
        }
        e -= 1;
        // 向后合并：匹配段 <=2 行且后面还有差异
        loop {
            let m_start = e + 1;
            if m_start >= n {
                break;
            }
            let mut m_end = m_start;
            while m_end < n && !diff[m_end] {
                m_end += 1;
            }
            let mlen = m_end - m_start;
            if mlen <= 2 && m_end < n && diff[m_end] {
                // 跳过后续差异段
                e = m_end;
                while e < n && diff[e] {
                    e += 1;
                }
                e -= 1;
            } else {
                break;
            }
        }
        let start = i.saturating_sub(1);
        blocks.push((start, e + 1));
        i = e + 1;
    }

    for (start, end) in blocks {
        println!("***** {}", display_path(f1));
        print_range(lines1, start, end, line_num, abbreviate);
        println!("***** {}", display_path(f2));
        print_range(lines2, start, end, line_num, abbreviate);
        println!("*****");
    }
    true
}

fn print_range(lines: &[String], start: usize, end: usize, line_num: bool, abbreviate: bool) {
    let len = end - start;
    if abbreviate && len > 3 {
        for (i, line) in lines.iter().enumerate().take(end).skip(start).take(1) {
            print_line(i, line, line_num);
        }
        println!("...");
        if let Some(line) = lines.get(end - 1) {
            print_line(end - 1, line, line_num);
        }
    } else {
        for i in start..end {
            if let Some(line) = lines.get(i) {
                print_line(i, line, line_num);
            } else {
                print_line(i, &String::new(), line_num);
            }
        }
    }
}

fn print_line(idx: usize, line: &str, line_num: bool) {
    if line_num {
        println!("{:5}:  {}", idx + 1, line);
    } else {
        println!("{line}");
    }
}

/// 二进制比较：逐字节，列出每个不同偏移（8 位 hex）。
fn compare_binary(d1: &[u8], d2: &[u8], f1: &str, f2: &str) -> bool {
    let n = d1.len().max(d2.len());
    let mut any = false;
    for i in 0..n {
        let a = d1.get(i).copied().unwrap_or(0);
        let b = d2.get(i).copied().unwrap_or(0);
        if a != b {
            any = true;
            println!("{:08X}: {:02X} {:02X}", i, a, b);
        }
    }
    if !any {
        println!("FC: no differences encountered");
    } else {
        let _ = (f1, f2);
    }
    any
}

fn main() -> ExitCode {
    let raw: Vec<String> = env::args().skip(1).collect();

    if raw.is_empty() {
        eprintln!("FC: Insufficient number of file specifications");
        return ExitCode::from(255);
    }

    let mut abbreviate = false;
    let mut binary = false;
    let mut ignore_case = false;
    let mut line_num = false;
    let mut unicode = false;
    let mut compress_ws = false;
    let mut files: Vec<&str> = Vec::new();

    for a in &raw {
        if a.starts_with('/') || a.starts_with('-') {
            match a[1..].to_ascii_uppercase().as_str() {
                "A" => abbreviate = true,
                "B" => binary = true,
                "C" => ignore_case = true,
                "L" | "N" => {
                    line_num = true;
                }
                "T" => {} // tab 不展开：默认行为差异可忽略
                "U" => unicode = true,
                "W" => compress_ws = true,
                "?" => {
                    println!("{HELP}");
                    return ExitCode::SUCCESS;
                }
                _ => {
                    eprintln!("FC: Invalid switch -{}.", a[1..].to_ascii_uppercase());
                    return ExitCode::from(2);
                }
            }
        } else {
            files.push(a);
        }
    }

    if files.len() < 2 {
        eprintln!("FC: Insufficient number of file specifications");
        return ExitCode::from(255);
    }
    if files.len() > 2 {
        eprintln!("FC: Too many files");
        return ExitCode::from(2);
    }

    let f1 = files[0];
    let f2 = files[1];
    println!("Comparing files {} and {}", display_path(f1), display_path(f2));

    let d1 = match fs::read(f1) {
        Ok(d) => d,
        Err(_) => {
            eprintln!(
                "FC: cannot open {} - No such file or folder",
                display_path(f1)
            );
            return ExitCode::from(2);
        }
    };
    let d2 = match fs::read(f2) {
        Ok(d) => d,
        Err(_) => {
            eprintln!(
                "FC: cannot open {} - No such file or folder",
                display_path(f2)
            );
            return ExitCode::from(2);
        }
    };

    if binary {
        return if compare_binary(&d1, &d2, f1, f2) {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        };
    }

    let (l1, l2) = if unicode {
        (read_lines_utf16(&d1), read_lines_utf16(&d2))
    } else {
        (read_lines(&d1), read_lines(&d2))
    };

    if compare_text(&l1, &l2, f1, f2, line_num, abbreviate, ignore_case, compress_ws) {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
