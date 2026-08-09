//! findstr —— 在文件中搜索字符串（复刻 Windows findstr.exe）。
//!
//! 正则引擎复用 `regex` crate（findstr 的正则语法是其子集）。
//! 跨平台无平台分支。

use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use regex::RegexBuilder;
use windowshit_i18n::{FluentArgs, L10n};

/// 让 Windows 控制台用 UTF-8 输出
#[cfg(windows)]
fn setup_console_utf8() {
    // SAFETY: 只调用标准 Win32 API，无其他副作用
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleOutputCP(65001);
    }
}

struct Args {
    recursive: bool,
    ignore_case: bool,
    line_number: bool,
    filename_only: bool,
    invert: bool,
    begin: bool,
    end: bool,
    whole: bool,
    literal: bool,
    patterns: Vec<String>,
    files: Vec<String>,
}

fn main() -> ExitCode {
    let mut i18n = L10n::detect();
    match i18n.lang() {
        "zh-CN" => i18n.add_ftl(include_str!("../locales/zh-CN.ftl")),
        _ => i18n.add_ftl(include_str!("../locales/en-US.ftl")),
    }
    i18n.set_help(
        include_str!("../locales/help.zh.txt"),
        include_str!("../locales/help.en.txt"),
    );

    #[cfg(windows)]
    setup_console_utf8();

    let raw: Vec<String> = env::args().skip(1).collect();

    if raw.iter().any(|a| a == "/?" || a == "-?") {
        println!("{}", i18n.help());
        return ExitCode::SUCCESS;
    }

    let mut args = Args {
        recursive: false,
        ignore_case: false,
        line_number: false,
        filename_only: false,
        invert: false,
        begin: false,
        end: false,
        whole: false,
        literal: false,
        patterns: Vec::new(),
        files: Vec::new(),
    };

    // 解析参数（/C:string 整体作为字面模式）
    let mut i = 0usize;
    while i < raw.len() {
        let a = &raw[i];
        if a.starts_with('/') || a.starts_with('-') {
            let (flag, inline) = if let Some(rest) = a.strip_prefix('/').or_else(|| a.strip_prefix('-')) {
                let (f, r) = if rest.len() > 1 { rest.split_at(1) } else { (rest, "") };
                (f.to_ascii_uppercase(), if r.is_empty() { None } else { Some(r.to_string()) })
            } else {
                (a.clone(), None)
            };
            match flag.as_str() {
                "S" => args.recursive = true,
                "I" => args.ignore_case = true,
                "N" => args.line_number = true,
                "M" => args.filename_only = true,
                "V" => args.invert = true,
                "B" => args.begin = true,
                "E" => args.end = true,
                "X" => args.whole = true,
                "L" => args.literal = true,
                "R" => args.literal = false,
                "C" => {
                    // /C:string
                    let v = match inline {
                        Some(v) => v,
                        None => {
                            i += 1;
                            if i < raw.len() {
                                raw[i].clone()
                            } else {
                                String::new()
                            }
                        }
                    };
                    args.patterns.push(v);
                    args.literal = true;
                }
                _ => {
                    // 未知 /xxx：Linux 绝对路径（/tmp/x.txt）以 / 开头，
                    // 不能当开关忽略，按文件路径处理
                    args.files.push(a.clone());
                }
            }
        } else {
            // 非开关：文件路径（无 * 通配时）；含通配符则按文件匹配展开
            args.files.push(a.clone());
        }
        i += 1;
    }

    // 剩余非开关参数：findstr 的 strings 出现在文件参数之前。
    // 简化：最后一个参数前的非开关参数中，除扩展名外是模式。
    // 这里按 findstr 实际用法：模式在前、文件在后。
    // 修正：上面把非开关都当文件了，需重新分配——模式 = 直到遇到真实文件。
    // 通过启发式：存在扩展名或路径分隔符的视为文件，否则为模式。
    let mut patterns: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    for f in std::mem::take(&mut args.files) {
        let looks_like_file = Path::new(&f).is_file()
            || f.contains(std::path::MAIN_SEPARATOR)
            || f.contains(':')
            || f.rsplit('.').next().map_or(false, |ext| !ext.is_empty() && ext.len() <= 4 && !ext.contains('*') && !ext.contains('?') && f.contains('.'));
        if looks_like_file {
            files.push(f);
        } else {
            patterns.push(f);
        }
    }
    if !args.patterns.is_empty() {
        // /C: 指定的优先
        patterns = std::mem::take(&mut args.patterns);
    }

    if patterns.is_empty() {
        eprintln!("{}", i18n.tr("error-no-pattern", None));
        return ExitCode::from(2);
    }

    // 构建正则
    let mut regexes = Vec::new();
    for p in &patterns {
        let mut expr = if args.literal { regex::escape(p) } else { p.clone() };
        if args.whole {
            expr = format!("^(?:{expr})$");
        } else {
            if args.begin {
                expr = format!("^(?:{expr})");
            }
            if args.end {
                expr = format!("(?:{expr})$");
            }
        }
        match RegexBuilder::new(&expr).case_insensitive(args.ignore_case).build() {
            Ok(re) => regexes.push(re),
            Err(_) => {
                eprintln!("{}", i18n.tr("error-bad-regex", None));
                return ExitCode::from(2);
            }
        }
    }

    // 展开文件列表（/S 递归）
    let mut search_files: Vec<PathBuf> = Vec::new();
    if files.is_empty() {
        // stdin
    } else {
        for f in &files {
            let p = PathBuf::from(f);
            if args.recursive {
                if p.is_dir() {
                    walk_files(&p, &mut search_files);
                } else if p.is_file() {
                    search_files.push(p);
                }
            } else if p.is_file() {
                search_files.push(p);
            } else {
                // 文件不存在：错误（退出码 2），但继续
                let mut a = FluentArgs::new();
                a.set("file", f.as_str());
                eprintln!("{}", i18n.tr("error-cannot-open", Some(&a)));
                return ExitCode::from(2);
            }
        }
    }

    let match_line = |line: &str| -> bool {
        let m = regexes.iter().any(|re| re.is_match(line));
        if args.invert { !m } else { m }
    };

    let mut found = false;

    if files.is_empty() {
        // stdin
        let stdin = io::stdin();
        let mut no = 0u64;
        for line in stdin.lock().lines().map_while(Result::ok) {
            no += 1;
            if match_line(&line) {
                found = true;
                if args.filename_only {
                    println!("<stdin>");
                    break;
                }
                print_matched(&mut io::stdout(), "", &line, no, &args);
            }
        }
    } else {
        for file in &search_files {
            match fs::read_to_string(file) {
                Ok(content) => {
                    if args.filename_only {
                        if content.lines().any(match_line) {
                            found = true;
                            println!("{}", file.display());
                        }
                        continue;
                    }
                    let prefix = if search_files.len() > 1 || args.recursive {
                        format!("{}:", file.display())
                    } else {
                        String::new()
                    };
                    for (no, line) in content.lines().enumerate() {
                        if match_line(line) {
                            found = true;
                            print_matched(&mut io::stdout(), &prefix, line, no as u64 + 1, &args);
                        }
                    }
                }
                Err(_) => {
                    let mut a = FluentArgs::new();
                    a.set("file", file.to_string_lossy());
                    eprintln!("{}", i18n.tr("error-cannot-open", Some(&a)));
                    return ExitCode::from(2);
                }
            }
        }
    }

    if found {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn print_matched(out: &mut impl Write, prefix: &str, line: &str, no: u64, args: &Args) {
    if args.line_number {
        let _ = writeln!(out, "{prefix}{no}:{line}");
    } else {
        let _ = writeln!(out, "{prefix}{line}");
    }
}

fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                walk_files(&p, out);
            } else if e.file_type().map(|t| t.is_file()).unwrap_or(false) {
                out.push(p);
            }
        }
    }
}
