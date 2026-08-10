//! expand —— 展开一个或多个压缩文件（复刻 Windows expand.exe）。
//!
//! 实测对齐的行为（Windows 11 原版）：
//! - 无参数：`No files specified.`，退出码 0
//! - `-d` 列表：`src.cab: 文件名`，多文件追加 `N files total.`
//! - 解压 CAB 到目录：`Adding dst\src.cab to Extraction Queue` + 空行 +
//!   `Expanding Files ....` + 空行 + `Expanding Files Complete ...`
//! - 多文件 CAB 无 -F 解压：提示需要 -F，但退出码仍为 0
//! - 源打不开：`Can't open input file: X.`，退出码 255
//! - 通配符多文件源：`Copying X to Y.` + `X: N bytes copied.` +
//!   `Total increase: ...`
//! - 命名规则：无 -R/-I 用源文件名，有 -R/-I 用归档内文件名
//! - 目标路径不存在时按文件创建（不建目录）
//!
//! 支持的压缩格式：CAB（cab crate）、SZDD（compress.exe 单文件产物，手写解压）。

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use windowshit_args::{parse, Flag, Kind, Parsed, Unknown};

const BANNER: &str = "Microsoft (R) File Expansion Utility\r\n\
Copyright (c) Microsoft Corporation. All rights reserved.";

const HELP: &str = "Expands one or more compressed files.

EXPAND [-R] Source Destination
EXPAND -R Source [Destination]
EXPAND -I Source [Destination]
EXPAND -D Source.cab [-F:Files]
EXPAND Source.cab -F:Files Destination

  -R\t\tRename expanded files.
  -I\t\tRename expanded files but ignore directory structure.
  -D\t\tDisplay list of files in source.
  Source\tSource file specification.  Wildcards may be used.
  -F:Files\tName of files to expand from a .CAB.
  Destination\tDestination file | path specification.
\t\tDestination may be a directory.
\t\tIf Source is multiple files and -r is not specified,
\t\tDestination must be a directory.";

/// 通配符匹配（Windows 风格：`*` 与 `?`，大小写不敏感）。
fn glob_match(name: &str, pat: &str) -> bool {
    fn m(n: &[u8], p: &[u8]) -> bool {
        if p.is_empty() {
            return n.is_empty();
        }
        match p[0] {
            b'*' => (0..=n.len()).any(|i| m(&n[i..], &p[1..])),
            b'?' => !n.is_empty() && m(&n[1..], &p[1..]),
            c => !n.is_empty() && n[0].eq_ignore_ascii_case(&c) && m(&n[1..], &p[1..]),
        }
    }
    m(name.as_bytes(), pat.as_bytes())
}

fn has_wildcard(s: &str) -> bool {
    s.contains('*') || s.contains('?')
}

/// 把模式拆成（目录, 文件名模式）。无目录时目录为 `"."`。
fn split_pattern(p: &str) -> (String, String) {
    let path = Path::new(p);
    match (path.parent(), path.file_name()) {
        (Some(d), Some(f)) if !f.is_empty() => {
            let dir = d.to_string_lossy().to_string();
            (if dir.is_empty() { ".".to_string() } else { dir }, f.to_string_lossy().to_string())
        }
        _ => (String::from("."), p.to_string()),
    }
}

fn join_path(dir: &str, name: &str) -> String {
    let d = if dir.is_empty() { "." } else { dir };
    if d == "." {
        name.to_string()
    } else {
        PathBuf::from(d).join(name).to_string_lossy().to_string()
    }
}

/// 展开通配符；无通配符时若文件存在则原样返回。
/// 返回 (匹配文件列表, 是否含通配符)。
fn glob_files(pattern: &str) -> (Vec<String>, bool) {
    if !has_wildcard(pattern) {
        if Path::new(pattern).exists() {
            return (vec![pattern.to_string()], false);
        }
        return (Vec::new(), false);
    }
    let (dir, file_pat) = split_pattern(pattern);
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if glob_match(&name, &file_pat) {
                out.push(join_path(&dir, &name));
            }
        }
    }
    out.sort();
    (out, true)
}

/// 读文件头，判断是 CAB（MSCF）、SZDD 还是普通文件。
enum FileKind {
    Cab,
    Szdd(u8, usize), // (原文件末尾字符, 解压后大小)
    Plain,
    Unreadable,
}

fn sniff(path: &str) -> FileKind {
    let Ok(data) = fs::read(path) else {
        return FileKind::Unreadable;
    };
    if data.len() >= 4 && &data[0..4] == b"MSCF" {
        return FileKind::Cab;
    }
    if data.len() >= 14
        && data[0..4] == *b"SZDD"
        && data[4..8] == [0x88, 0xF0, 0x27, 0x33]
        && data[8] == b'A'
    {
        let orig = data[9];
        let size = u32::from_le_bytes(data[10..14].try_into().unwrap()) as usize;
        return FileKind::Szdd(orig, size);
    }
    FileKind::Plain
}

/// SZDD（LZSS 变体）解压。4096 环形窗口，初始为空格。
fn szdd_decode(data: &[u8], size: usize) -> Option<Vec<u8>> {
    let mut window = [0x20u8; 4096];
    let mut pos = 4096usize - 16;
    let mut out = Vec::with_capacity(size);
    let mut i = 14;
    while out.len() < size {
        let control = *data.get(i)?;
        i += 1;
        let mut cbit = 0x01u8;
        // 组内达到目标大小即停止（最后一组可能不满 8 个 token）
        while cbit != 0 && out.len() < size {
            if control & cbit != 0 {
                let b = *data.get(i)?;
                i += 1;
                window[pos & 4095] = b;
                out.push(b);
                pos = (pos + 1) & 4095;
            } else {
                let b0 = *data.get(i)?;
                let b1 = *data.get(i + 1)?;
                i += 2;
                let mut mp = b0 as usize | ((b1 as usize & 0xF0) << 4);
                let mut len = (b1 as usize & 0x0F) + 3;
                while len > 0 {
                    let b = window[mp & 4095];
                    window[pos & 4095] = b;
                    out.push(b);
                    mp = (mp + 1) & 4095;
                    pos = (pos + 1) & 4095;
                    len -= 1;
                }
            }
            cbit <<= 1;
        }
    }
    if out.len() == size {
        Some(out)
    } else {
        None
    }
}

/// CAB 内文件条目 (名称, 大小)。
fn cab_list(path: &str) -> Result<Vec<(String, u32)>, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let cab = cab::Cabinet::new(file).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for folder in cab.folder_entries() {
        for f in folder.file_entries() {
            out.push((f.name().to_string(), f.uncompressed_size()));
        }
    }
    Ok(out)
}

/// 从 CAB 提取单个文件。
fn cab_extract(path: &str, name: &str, dst: &str) -> Result<u64, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut cab = cab::Cabinet::new(file).map_err(|e| e.to_string())?;
    let mut reader = cab.read_file(name).map_err(|e| e.to_string())?;
    let mut out = fs::File::create(dst).map_err(|e| e.to_string())?;
    io::copy(&mut reader, &mut out).map_err(|e| e.to_string())
}

/// destination 是否按目录处理（已存在目录或以分隔符结尾）。
fn is_dir_dest(dest: &str) -> bool {
    dest.ends_with('/') || dest.ends_with('\\') || Path::new(dest).is_dir()
}

/// 计算归档的内部名：SZDD 用源名替换末尾字符，CAB 用条目名。
fn internal_name(source: &str, orig_char: u8) -> String {
    let mut name = source.to_string();
    if !name.is_empty() && orig_char != 0 {
        name.pop();
        name.push(orig_char as char);
    }
    name
}

/// 解压模式主流程。
fn do_expand(paths: &[&str], rename: bool, filt: Option<&str>) -> ExitCode {
    let source = paths[0];
    let (files, wildcard) = glob_files(source);
    if files.is_empty() {
        eprintln!("Can't open input file: {source}.");
        return ExitCode::from(255);
    }

    // 多文件源（通配符）：copy 流程，与单文件 CAB/SZDD 的 extract 流程不同。
    if wildcard {
        return copy_many(&files, paths.get(1).copied());
    }

    let src = &files[0];
    let dest = paths.get(1).copied();

    match sniff(src) {
        FileKind::Cab => {
            let entries = match cab_list(src) {
                Ok(e) => e,
                Err(_) => {
                    eprintln!("Can't open input file: {src}.");
                    return ExitCode::from(255);
                }
            };
            if entries.len() > 1 && filt.is_none() {
                // 多文件 CAB 无 -F：原版提示，退出码仍为 0
                println!("The source file contains multiple files.  The -F:filespec option is");
                println!("required to specify which file(s) are to be expanded.  -F:* may be");
                println!("used to expand all files.  Type EXPAND -? for more details.");
                return ExitCode::SUCCESS;
            }
            let selected: Vec<&(String, u32)> = entries
                .iter()
                .filter(|(n, _)| filt.map_or(true, |f| glob_match(n, f)))
                .collect();
            if selected.is_empty() {
                // 原版对 -F 无匹配仍输出 Adding dest + 队列流程
                let placeholder = dest.unwrap_or(src).to_string();
                return run_queue(vec![(placeholder, false)], src, dest, rename);
            }
            let multi = selected.len() > 1;
            // 逐文件解压
            let mut out_files = Vec::new();
            for (name, _) in &selected {
                let dst = pick_dest(src, dest, rename, filt.is_some(), name, multi);
                match cab_extract(src, name, &dst) {
                    Ok(_) => out_files.push((dst, true)),
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::from(255);
                    }
                }
            }
            run_queue(out_files, src, dest, rename)
        }
        FileKind::Szdd(orig, size) => {
            let data = match fs::read(src) {
                Ok(d) => d,
                Err(_) => {
                    eprintln!("Can't open input file: {src}.");
                    return ExitCode::from(255);
                }
            };
            let decoded = match szdd_decode(&data, size) {
                Some(d) => d,
                None => {
                    eprintln!("Can't open input file: {src}.");
                    return ExitCode::from(255);
                }
            };
            let iname = internal_name(src, orig);
            let dst = pick_dest(src, dest, rename, false, &iname, false);
            if let Err(e) = fs::write(&dst, &decoded) {
                eprintln!("{e}");
                return ExitCode::from(255);
            }
            run_queue(vec![(dst, true)], src, dest, rename)
        }
        FileKind::Plain => {
            // 单文件普通源：当作复制处理。
            let dst = pick_dest(src, dest, rename, false, &basename(src), false);
            copy_one(src, &dst)
        }
        FileKind::Unreadable => {
            eprintln!("Can't open input file: {src}.");
            ExitCode::from(255)
        }
    }
}

fn basename(p: &str) -> String {
    Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| p.to_string())
}

/// 计算单个文件的目标路径。
/// - 用 -F 从 CAB 提取时（from_cab）一律用归档内文件名
/// - 否则：rename ? 内部名 : 源文件名
/// - dest 为目录（或 multi 多文件）：目录 + 文件名
/// - dest 为文件路径且单文件：直接用 dest
fn pick_dest(
    src: &str,
    dest: Option<&str>,
    rename: bool,
    from_cab: bool,
    iname: &str,
    multi: bool,
) -> String {
    let name = if rename || from_cab {
        iname.to_string()
    } else {
        basename(src)
    };
    match dest {
        Some(d) if is_dir_dest(d) || multi => join_path(d, &name),
        Some(d) => d.to_string(),
        None => name,
    }
}

/// 输出解压队列（Adding/Expanding）流程。
fn run_queue(
    files: Vec<(String, bool)>,
    _src: &str,
    _dest: Option<&str>,
    _rename: bool,
) -> ExitCode {
    for (dst, _) in &files {
        println!("Adding {dst} to Extraction Queue");
        println!();
    }
    println!("Expanding Files ....");
    println!();
    println!("Expanding Files Complete ...");
    if files.len() > 1 {
        println!("{} files total.", files.len());
    }
    ExitCode::SUCCESS
}

/// 复制单个文件（普通源）。
fn copy_one(src: &str, dst: &str) -> ExitCode {
    let data = match fs::read(src) {
        Ok(d) => d,
        Err(_) => {
            eprintln!("Can't open input file: {src}.");
            return ExitCode::from(255);
        }
    };
    if let Err(e) = fs::write(dst, &data) {
        eprintln!("{e}");
        return ExitCode::from(255);
    }
    println!("Copying {src} to {dst}.");
    println!("{src}: {} bytes copied.", data.len());
    println!();
    ExitCode::SUCCESS
}

/// 多文件源复制流程。
fn copy_many(files: &[String], dest: Option<&str>) -> ExitCode {
    let d = match dest {
        Some(d) => d,
        None => {
            eprintln!("Can't open input file: {}.", files[0]);
            return ExitCode::from(255);
        }
    };
    let mut total = 0u64;
    for src in files {
        let data = match fs::read(src) {
            Ok(d) => d,
            Err(_) => {
                eprintln!("Can't open input file: {src}.");
                return ExitCode::from(255);
            }
        };
        let dst = join_path(d, &basename(src));
        if let Err(e) = fs::write(&dst, &data) {
            eprintln!("{e}");
            return ExitCode::from(255);
        }
        total += data.len() as u64;
        println!("Copying {src} to {dst}.");
        println!("{src}: {} bytes copied.", data.len());
        println!();
    }
    println!(
        "Total increase: {} files, {total} bytes expanded to {total} bytes, 0% increase.",
        files.len()
    );
    ExitCode::SUCCESS
}

/// 列表模式（-d）。
fn do_list(paths: &[&str], filt: Option<&str>) -> ExitCode {
    let mut total = 0usize;
    for p in paths {
        let (files, wildcard) = glob_files(p);
        if files.is_empty() {
            eprintln!("Can't open input file: {p}.");
            return ExitCode::from(255);
        }
        for src in &files {
            match sniff(src) {
                FileKind::Cab => match cab_list(src) {
                    Ok(entries) => {
                        let n: Vec<_> = entries
                            .iter()
                            .filter(|(name, _)| filt.map_or(true, |f| glob_match(name, f)))
                            .collect();
                        for (name, _) in &n {
                            println!("{src}: {name}");
                        }
                        total += n.len();
                    }
                    Err(_) => {
                        eprintln!("Can't open input file: {src}.");
                        return ExitCode::from(255);
                    }
                },
                FileKind::Szdd(orig, _) => {
                    let iname = internal_name(src, orig);
                    if filt.map_or(true, |f| glob_match(&iname, f)) {
                        println!("{src}: {iname}");
                        total += 1;
                    }
                }
                FileKind::Plain => {
                    println!("{src}: {}", basename(src));
                    total += 1;
                }
                FileKind::Unreadable => {
                    eprintln!("Can't open input file: {src}.");
                    return ExitCode::from(255);
                }
            }
        }
        if wildcard {
            let _ = total;
        }
    }
    if total > 1 {
        println!();
        println!("{total} files total.");
    }
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let raw: Vec<String> = env::args().skip(1).collect();

    let banner = || {
        println!("{BANNER}");
        println!();
    };

    if raw.is_empty() {
        banner();
        println!("No files specified.");
        return ExitCode::SUCCESS;
    }
    if raw.iter().any(|a| a == "/?" || a == "-?") {
        banner();
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }

    // 精确开关表；未知 /xxx 一律按路径处理（Linux 绝对路径以 / 开头）
    const FLAGS: &[Flag] = &[
        Flag::new("R", Kind::Flag),
        Flag::new("I", Kind::Flag),
        Flag::new("D", Kind::Flag),
        Flag::new("F", Kind::Value),
    ];
    let parsed: Parsed = match parse(&raw, FLAGS, Unknown::Path) {
        Ok(p) => p,
        Err(_) => Parsed::default(),
    };
    let rename = parsed.flags.contains_key("R") || parsed.flags.contains_key("I");
    let list = parsed.flags.contains_key("D");
    let filt = parsed.flags.get("F").and_then(|v| *v);
    let paths: Vec<&str> = parsed.paths;

    if paths.is_empty() {
        banner();
        println!("No files specified.");
        return ExitCode::SUCCESS;
    }

    banner();

    if list {
        do_list(&paths, filt)
    } else {
        do_expand(&paths, rename, filt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 简单贪心 LZSS 编码器（仅测试用），生成 SZDD 格式字节流。
    /// flag 位：1 = literal，0 = match（与 libmspack 描述一致）。
    fn encode_szdd(data: &[u8]) -> Vec<u8> {
        let mut window = [0x20u8; 4096];
        let mut pos = 4096usize - 16;
        let mut out = Vec::new();
        out.extend_from_slice(b"SZDD");
        out.extend_from_slice(&[0x88, 0xF0, 0x27, 0x33]);
        out.push(b'A');
        out.push(b'_'); // 原文件名末尾字符（测试随意）
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());

        let mut i = 0usize;
        while i < data.len() {
            let mut flags = 0u8;
            let mut tokens: Vec<u8> = Vec::new();
            let mut bit = 1u8;
            let mut n = 0usize;
            while n < 8 && i < data.len() {
                let max_len = (data.len() - i).min(18);
                let mut best_len = 0usize;
                let mut best_pos = 0usize;
                for mp in 0..4096 {
                    let mut l = 0usize;
                    while l < max_len && window[(mp + l) & 4095] == data[i + l] {
                        l += 1;
                    }
                    if l > best_len {
                        best_len = l;
                        best_pos = mp;
                    }
                }
                if best_len >= 3 {
                    // match：不设 flag 位
                    // b0 = matchpos 低 8 位；b1 高 4 位 = matchpos 高 4 位，低 4 位 = len-3
                    let b0 = (best_pos & 0xFF) as u8;
                    let b1 =
                        ((((best_pos >> 8) & 0x0F) << 4) | ((best_len - 3) & 0x0F)) as u8;
                    tokens.push(b0);
                    tokens.push(b1);
                    for k in 0..best_len {
                        window[pos & 4095] = data[i + k];
                        pos = (pos + 1) & 4095;
                    }
                    i += best_len;
                } else {
                    flags |= bit;
                    let b = data[i];
                    window[pos & 4095] = b;
                    pos = (pos + 1) & 4095;
                    tokens.push(b);
                    i += 1;
                }
                bit <<= 1;
                n += 1;
            }
            out.push(flags);
            out.extend_from_slice(&tokens);
        }
        out
    }

    fn roundtrip(data: &[u8]) {
        let encoded = encode_szdd(data);
        let size = u32::from_le_bytes(encoded[10..14].try_into().unwrap()) as usize;
        let decoded = szdd_decode(&encoded, size).expect("decode failed");
        assert_eq!(decoded, data);
    }

    #[test]
    fn szdd_literal_only() {
        roundtrip(b"0123456789ABCDEF");
    }

    #[test]
    fn szdd_with_matches() {
        roundtrip(b"ABCABCABCABCABCABCABCABC");
        roundtrip(b"Hello Hello Hello Hello World!");
        roundtrip(&vec![b'x'; 64]);
    }

    #[test]
    fn szdd_spaces_initial_window() {
        // 窗口初始为空格，数据里含空格应能直接匹配
        roundtrip(b"      padded");
    }

    #[test]
    fn glob_match_basic() {
        assert!(glob_match("hello.txt", "*.txt"));
        assert!(glob_match("hello.txt", "h?llo.*"));
        assert!(!glob_match("hello.txt", "*.exe"));
        assert!(glob_match("ABC.txt", "abc.*"));
    }
}
