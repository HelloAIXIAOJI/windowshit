//! where —— 在 PATH 中查找匹配文件（复刻 Windows where.exe）。
//!
//! 纯文件系统逻辑 + 通配符匹配，跨平台无平台分支。
//! PATHEXT 扩展名追加仅 Windows 有语义，unix 上为空。

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use windowshit_i18n::L10n;

/// 让 Windows 控制台用 UTF-8 输出
#[cfg(windows)]
fn setup_console_utf8() {
    // SAFETY: 只调用标准 Win32 API，无其他副作用
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleOutputCP(65001);
    }
}

struct Args {
    recursive: Option<PathBuf>,
    quiet: bool,
    force: bool,
    show_time: bool,
    patterns: Vec<String>,
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

    if raw.iter().any(|a| a == "/?" || a == "-?" || a == "/help") {
        println!("{}", i18n.help());
        return ExitCode::SUCCESS;
    }

    // 解析参数
    let mut args = Args {
        recursive: None,
        quiet: false,
        force: false,
        show_time: false,
        patterns: Vec::new(),
    };
    let mut i = 0usize;
    while i < raw.len() {
        let a = &raw[i];
        if a.starts_with('/') || a.starts_with('-') {
            match a[1..].to_ascii_uppercase().as_str() {
                "R" => {
                    i += 1;
                    if i >= raw.len() {
                        eprintln!("ERROR: /R requires a directory.");
                        return ExitCode::from(2);
                    }
                    args.recursive = Some(PathBuf::from(&raw[i]));
                }
                "Q" => args.quiet = true,
                "F" => args.force = true,
                "T" => args.show_time = true,
                "?" => {
                    println!("{}", i18n.help());
                    return ExitCode::SUCCESS;
                }
                _ => {
                    eprintln!("ERROR: Invalid switch -{}.", &a[1..]);
                    return ExitCode::from(2);
                }
            }
        } else {
            args.patterns.push(a.clone());
        }
        i += 1;
    }

    if args.patterns.is_empty() {
        eprintln!("{}", i18n.tr("error-no-pattern", None));
        println!();
        println!("{}", i18n.help());
        return ExitCode::from(2);
    }

    // 收集搜索目录
    let mut dirs: Vec<PathBuf> = Vec::new();
    match &args.recursive {
        Some(dir) => dirs.push(dir.clone()),
        None => {
            if let Ok(cwd) = env::current_dir() {
                dirs.push(cwd);
            }
            if let Some(path) = env::var_os("PATH") {
                for p in env::split_paths(&path) {
                    dirs.push(p);
                }
            }
        }
    }

    let pathext = pathexts();
    let mut found: Vec<PathBuf> = Vec::new();
    let mut errors = 0usize;

    for dir in &dirs {
        for pattern in &args.patterns {
            match &args.recursive {
                Some(_) => walk(dir, pattern, &pathext, &mut found, &mut errors),
                None => {
                    if let Ok(entries) = fs::read_dir(dir) {
                        for e in entries.flatten() {
                            let name = e.file_name().to_string_lossy().to_string();
                            if wild_match(pattern, &name) {
                                found.push(e.path());
                            } else if !has_ext(pattern) {
                                // PATHEXT 追加尝试：pattern+ext 匹配 name
                                for ext in &pathext {
                                    let cand = format!("{pattern}{}", ext.to_lowercase());
                                    if wild_match(&cand, &name) {
                                        found.push(e.path());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 去重（同一文件可能从多个 PATH 目录命中）
    found.sort();
    found.dedup();

    if !args.quiet {
        for path in &found {
            if args.show_time {
                if let Ok(meta) = fs::metadata(path) {
                    let size = meta.len();
                    let modified = meta.modified().ok().map(fmt_time).unwrap_or_default();
                    println!("{}  {size}  {modified}", display_path(path, args.force));
                }
            } else {
                println!("{}", display_path(path, args.force));
            }
        }
    }

    if errors > 0 {
        ExitCode::from(2)
    } else if found.is_empty() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// 显示路径：/F 加双引号
fn display_path(path: &Path, force: bool) -> String {
    let s = path.to_string_lossy().to_string();
    if force {
        format!("\"{s}\"")
    } else {
        s
    }
}

/// PATHEXT 扩展名列表（Windows 才有，unix 空）
fn pathexts() -> Vec<String> {
    #[cfg(windows)]
    {
        env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string())
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_lowercase())
            .collect()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

fn has_ext(pattern: &str) -> bool {
    pattern.contains('.')
}

/// 通配符匹配（* 和 ?），大小写不敏感（Windows 风格）。
fn wild_match(pattern: &str, name: &str) -> bool {
    let p = pattern.to_lowercase().into_bytes();
    let n = name.to_lowercase().into_bytes();
    wild_match_bytes(&p, &n)
}

fn wild_match_bytes(p: &[u8], n: &[u8]) -> bool {
    if p.is_empty() && n.is_empty() {
        return true;
    }
    if let Some(&first) = p.first() {
        if first == b'*' {
            let rest = &p[1..];
            return wild_match_bytes(rest, n) || (!n.is_empty() && wild_match_bytes(p, &n[1..]));
        }
    }
    if let (Some(&c), Some(&nc)) = (p.first(), n.first()) {
        if c == b'?' || c == nc {
            return wild_match_bytes(&p[1..], &n[1..]);
        }
    }
    false
}

/// 递归搜索（/R）
fn walk(dir: &Path, pattern: &str, pathext: &[String], found: &mut Vec<PathBuf>, errors: &mut usize) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => {
            *errors += 1;
            return;
        }
    };
    for e in entries.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            walk(&path, pattern, pathext, found, errors);
        } else if wild_match(pattern, &name) {
            found.push(path);
        } else if !has_ext(pattern) {
            for ext in pathext {
                let cand = format!("{pattern}{}", ext.to_lowercase());
                if wild_match(&cand, &name) {
                    found.push(path);
                    break;
                }
            }
        }
    }
}

/// 时间格式 mm/dd/yyyy hh:mm（Windows where /T 格式）
fn fmt_time(t: std::time::SystemTime) -> String {
    let dur = match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d,
        Err(_) => return String::new(),
    };
    let secs = dur.as_secs();
    let days = secs / 86400;
    let (mut y, mut m, mut d) = days_to_ymd(days);
    let mut rem = secs % 86400;
    let hh = rem / 3600;
    rem %= 3600;
    let mi = rem / 60;
    let _ = &mut y;
    let _ = &mut m;
    let _ = &mut d;
    format!("{y:04}-{m:02}-{d:02} {hh:02}:{mi:02}")
}

/// 从 UNIX 天数转换日期（简单算法）
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let mut y = 1970u64;
    let mut remaining = days;
    loop {
        let ydays = if is_leap(y) { 366 } else { 365 };
        if remaining >= ydays {
            remaining -= ydays;
            y += 1;
        } else {
            break;
        }
    }
    let mut m = 1u64;
    while m <= 12 {
        let mdays = month_days(y, m);
        if remaining >= mdays {
            remaining -= mdays;
            m += 1;
        } else {
            break;
        }
    }
    (y, m, remaining + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn month_days(y: u64, m: u64) -> u64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}
