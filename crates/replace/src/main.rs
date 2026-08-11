//! replace —— 用源目录文件替换目标目录文件（复刻 Windows replace.exe）。
//!
//! 实测对齐的行为：
//! - 无源：`No files replaced` + 红色 `Source path required`，退出码 11
//! - 源无匹配（含源是目录）：`No files replaced` + 红色 `No files found - X`，退出码 2
//! - 无效开关：`No files replaced` + 红色 `Invalid switch - /x`，退出码 11
//! - 正常替换：`Replacing <dst\file>`，退出码 0
//! - /a：`Adding <dst\file>`（只添加目标不存在的文件）
//! - 源文件不存在（非通配符）：静默退出码 0
//! - /s：递归目标目录；/u：仅替换目标比源旧的；/w：等待按键
//! - 原版输出恒为英文（不随系统语言变），错误用 ANSI 红色（31;1m）

use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use windowshit_args::{parse, Flag, Kind, Unknown};

const HELP: &str = "Replaces files.

REPLACE [drive1:][path1]filename [drive2:][path2] [/A] [/P] [/R] [/W]
REPLACE [drive1:][path1]filename [drive2:][path2] [/P] [/R] [/S] [/W] [/U]

  [drive1:][path1]filename Specifies the source file or files.
  [drive2:][path2]         Specifies the directory where files are to be
                           replaced.
  /A                       Adds new files to destination directory. Cannot
                           use with /S or /U switches.
  /P                       Prompts for confirmation before replacing a
                           destination file.
  /R                       Replaces read-only files as well as unprotected
                           files.
  /S                       Replaces files in all subdirectories of the
                           destination directory. Cannot use with /A switch.
  /W                       Waits for you to insert a disk before beginning.
  /U                       Replaces only those files on the destination
                           directory that are older than those in the source
                           directory. Cannot use with /A switch.
  /?                       Displays this help message.";

fn red(s: &str) -> String {
    format!("\x1b[31;1m{s}\x1b[0m")
}

/// 判断文件名是否匹配通配符模式（`*` `?`，大小写不敏感）。
fn wild_match(pattern: &str, name: &str) -> bool {
    fn bytes(p: &[u8], n: &[u8]) -> bool {
        if p.is_empty() && n.is_empty() {
            return true;
        }
        if let Some(&c) = p.first() {
            if c == b'*' {
                return bytes(&p[1..], n) || (!n.is_empty() && bytes(p, &n[1..]));
            }
        }
        if let (Some(&c), Some(&nc)) = (p.first(), n.first()) {
            if c == b'?' || c == nc {
                return bytes(&p[1..], &n[1..]);
            }
        }
        false
    }
    bytes(
        &pattern.to_lowercase().into_bytes(),
        &name.to_lowercase().into_bytes(),
    )
}

/// 源路径最后一段是模式 → 枚举目录；否则返回单文件（不存在也返回，保持原版静默）。
fn expand_source(raw: &str) -> Vec<PathBuf> {
    let p = PathBuf::from(raw);
    let name = p.file_name().map(|s| s.to_string_lossy().to_string());
    let dir = p.parent().map(Path::to_path_buf).unwrap_or_default();

    let Some(name) = name else {
        return Vec::new();
    };

    if name.contains('*') || name.contains('?') {
        let mut out = Vec::new();
        if let Ok(entries) = fs::read_dir(&dir) {
            for e in entries.flatten() {
                if wild_match(&name, &e.file_name().to_string_lossy()) {
                    out.push(e.path());
                }
            }
        }
        out
    } else {
        // 单文件：无论是否存在都返回（存在性由后续处理决定）
        vec![p]
    }
}

/// 递归收集目标目录下所有匹配 base_name 的文件路径。
fn collect_targets(dir: &Path, base_name: &str, recursive: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            if recursive {
                out.extend(collect_targets(&path, base_name, true));
            }
        } else if path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(base_name))
        {
            out.push(path);
        }
    }
    out
}

fn main() -> ExitCode {
    let raw: Vec<String> = env::args().skip(1).collect();

    if raw.is_empty() {
        println!("No files replaced");
        eprintln!("{}", red("Source path required"));
        return ExitCode::from(11);
    }
    if raw.iter().any(|a| a == "/?" || a == "-?") {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }

    // 精确开关表；未知 /xxx 一律按路径处理（Linux 绝对路径以 / 开头）
    const FLAGS: &[Flag] = &[
        Flag::new("A", Kind::Flag),
        Flag::new("P", Kind::Flag),
        Flag::new("R", Kind::Flag),
        Flag::new("S", Kind::Flag),
        Flag::new("U", Kind::Flag),
        Flag::new("W", Kind::Flag),
    ];
    let parsed = parse(&raw, FLAGS, Unknown::Path).unwrap_or_default();
    let add = parsed.flags.contains_key("A"); // /A 添加
    let prompt = parsed.flags.contains_key("P"); // /P 确认
    let read_only = parsed.flags.contains_key("R"); // /R 替换只读
    let recursive = parsed.flags.contains_key("S"); // /S 递归
    let older_only = parsed.flags.contains_key("U"); // /U 仅旧
    let wait = parsed.flags.contains_key("W"); // /W 等待
    let operands: Vec<&str> = parsed.paths;

    if operands.is_empty() {
        println!("No files replaced");
        eprintln!("{}", red("Source path required"));
        return ExitCode::from(11);
    }
    // 开关组合限制（help 明确：/A 不能与 /S /U 同用）
    if add && (recursive || older_only) {
        println!("No files replaced");
        eprintln!(
            "{}",
            red("Invalid syntax. /A cannot be used with /S or /U.")
        );
        return ExitCode::from(11);
    }

    // 目标目录：最后一个操作数
    let target_dir = PathBuf::from(operands[operands.len() - 1]);
    // 源：前面的操作数（只支持单个源，多源同原版报错）
    if operands.len() > 2 {
        println!("No files replaced");
        eprintln!("{}", red("Too many files."));
        return ExitCode::from(2);
    }
    let source_raw = operands[0];

    if wait {
        // 等待插入磁盘
        print!("Press any key to continue . . .");
        let _ = io::stdout().flush();
        let mut buf = [0u8; 1];
        let _ = io::stdin().read(&mut buf);
        println!();
    }

    let sources = expand_source(source_raw);

    // 单文件分支（非通配符）：
    let is_pattern = Path::new(source_raw)
        .file_name()
        .map(|n| {
            let n = n.to_string_lossy();
            n.contains('*') || n.contains('?')
        })
        .unwrap_or(false);
    if !is_pattern {
        let p = Path::new(source_raw);
        if p.is_file() {
            // 正常单文件，继续
        } else if p.exists() {
            // 存在但是目录 → No files found（实测原版行为）
            println!("No files replaced");
            eprintln!("{}", red(&format!("No files found - {source_raw}")));
            return ExitCode::from(2);
        } else {
            // 不存在：原版静默退出 0
            return ExitCode::SUCCESS;
        }
    }

    if sources.is_empty() {
        println!("No files replaced");
        eprintln!("{}", red(&format!("No files found - {source_raw}")));
        return ExitCode::from(2);
    }

    if !target_dir.is_dir() {
        println!("No files replaced");
        eprintln!(
            "{}",
            red(&format!("Path not found - {}", target_dir.display()))
        );
        return ExitCode::from(2);
    }

    let mut replaced_any = false;
    for src in &sources {
        let Ok(src_bytes) = fs::read(src) else {
            continue; // 单文件不存在：静默跳过（实测原版 EXIT 0）
        };
        let base = src.file_name().unwrap().to_string_lossy().to_string();

        let targets = collect_targets(&target_dir, &base, recursive);

        if add {
            // /A：只添加目标不存在的文件
            if targets.is_empty() {
                let dst = target_dir.join(&base);
                if write_file(&dst, &src_bytes, read_only).is_ok() {
                    println!("Adding {}", dst.display());
                    replaced_any = true;
                }
            }
            // 已存在则跳过（/A 不覆盖）
            continue;
        }

        for t in &targets {
            // /U：仅替换目标比源旧的
            if older_only {
                let older = fs::metadata(t)
                    .and_then(|m| m.modified())
                    .ok()
                    .zip(fs::metadata(src).and_then(|m| m.modified()).ok())
                    .is_some_and(|(dst_m, src_m)| dst_m < src_m);
                if !older {
                    continue;
                }
            }
            if prompt {
                print!("Replace {}? (Y/N): ", t.display());
                let _ = io::stdout().flush();
                let mut input = String::new();
                if io::stdin().read_line(&mut input).is_err()
                    || !input.trim().eq_ignore_ascii_case("y")
                {
                    continue;
                }
            }
            if write_file(t, &src_bytes, read_only).is_ok() {
                println!("Replacing {}", t.display());
                replaced_any = true;
            }
        }
    }

    ExitCode::from(if replaced_any { 0 } else { 2 })
}

/// 写文件；/R 时若因只读失败，尝试清除只读属性后重试。
fn write_file(path: &Path, bytes: &[u8], allow_readonly: bool) -> io::Result<()> {
    match fs::write(path, bytes) {
        Ok(()) => Ok(()),
        Err(_) if allow_readonly => {
            // 尝试清除只读属性（Windows）
            clear_readonly(path);
            fs::write(path, bytes)
        }
        Err(e) => Err(e),
    }
}

/// 清除只读属性（Windows SetFileAttributes；其它平台尽力而为）。
fn clear_readonly(path: &Path) {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{SetFileAttributesW, FILE_ATTRIBUTE_NORMAL};
        // SAFETY: 标准 API，宽字符串以 NUL 结尾
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            SetFileAttributesW(wide.as_ptr(), FILE_ATTRIBUTE_NORMAL);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
    }
}
