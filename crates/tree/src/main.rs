//! tree —— 以图形方式显示目录结构（复刻 Windows tree.exe）。
//!
//! 纯文件系统递归遍历，跨平台无平台分支。
//! Windows 特有：卷序列号（GetVolumeInformation）；Linux/macOS 无此概念不显示。

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use windowshit_args::{parse, Flag, Kind, Parsed, Unknown};
use windowshit_i18n::{FluentArgs, L10n};

/// 让 Windows 控制台用 UTF-8 输出
#[cfg(windows)]
fn setup_console_utf8() {
    // SAFETY: 只调用标准 Win32 API，无其他副作用
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleOutputCP(65001);
    }
}

/// Windows 卷序列号（XXXX-XXXX）。仅 Windows 存在。
#[cfg(windows)]
fn volume_serial(_path: &str) -> Option<String> {
    use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    // SAFETY: 标准 API，缓冲区正确
    unsafe {
        let root = wide("C:\\");
        let mut serial: u32 = 0;
        let ret = GetVolumeInformationW(
            root.as_ptr(),
            std::ptr::null_mut::<u16>(),
            0,
            &mut serial,
            std::ptr::null_mut::<u32>(),
            std::ptr::null_mut::<u32>(),
            std::ptr::null_mut::<u16>(),
            0,
        );
        if ret != 0 {
            Some(format!("{:04X}-{:04X}", (serial >> 16) & 0xFFFF, serial & 0xFFFF))
        } else {
            None
        }
    }
}

#[cfg(not(windows))]
fn volume_serial(_path: &str) -> Option<String> {
    None
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

    // 只认精确开关；Linux 绝对路径以 / 开头，未知一律按路径
    const FLAGS: &[Flag] = &[
        Flag::new("F", Kind::Flag),
        Flag::new("A", Kind::Flag),
    ];
    let parsed = match parse(&raw, FLAGS, Unknown::Path) {
        Ok(p) => p,
        Err(_) => Parsed {
            flags: Default::default(),
            paths: raw.iter().map(String::as_str).collect(),
        },
    };
    let show_files = parsed.flags.contains_key("F");
    let ascii = parsed.flags.contains_key("A");
    let target: Option<PathBuf> = parsed.paths.last().map(|s| PathBuf::from(s));

    let root = match target {
        Some(t) => t,
        None => env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };

    println!("{}", i18n.tr("header", None));
    if let Some(serial) = volume_serial("C:\\") {
        let mut a = FluentArgs::new();
        a.set("serial", &serial);
        println!("{}", i18n.tr("volume-serial", Some(&a)));
    }

    // 根路径行（Windows 原版显示大写路径）
    let root_str = display_root(&root);
    println!("{root_str}");

    // 收集顶层条目
    let mut subdirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&root) {
        for e in entries.flatten() {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                subdirs.push(e.path());
            } else {
                files.push(e.path());
            }
        }
    }
    subdirs.sort();
    files.sort();

    if subdirs.is_empty() && (!show_files || files.is_empty()) {
        println!("{}", i18n.tr("no-subfolders", None));
        return ExitCode::SUCCESS;
    }

    // 顶层条目（文件在前，子目录在后？原版 tree 按字母序混合。
    // 实测原版先列文件再子目录不确切；这里按子目录优先列出目录结构，
    // /F 时文件夹在目录项中按字母序处理。）
    let mut items: Vec<(PathBuf, bool)> = Vec::new();
    if show_files {
        for f in files {
            items.push((f, false));
        }
    }
    for d in subdirs {
        items.push((d, true));
    }

    for (idx, (item, is_dir)) in items.iter().enumerate() {
        let last = idx == items.len() - 1;
        let conn = if ascii {
            if last { "`-- " } else { "|-- " }
        } else if last {
            "└── "
        } else {
            "├── "
        };
        let name = item.file_name().unwrap_or_default().to_string_lossy();
        println!("{conn}{name}");
        if *is_dir {
            let child_prefix = if ascii {
                if last { "    " } else { "|   " }
            } else if last {
                "    "
            } else {
                "│   "
            };
            print_dir(item, child_prefix, show_files, ascii, &i18n);
        }
    }

    ExitCode::SUCCESS
}

fn print_dir(dir: &Path, prefix: &str, show_files: bool, ascii: bool, i18n: &L10n) {
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                dirs.push(e.path());
            } else {
                files.push(e.path());
            }
        }
    }
    dirs.sort();
    files.sort();

    let mut items: Vec<(PathBuf, bool)> = Vec::new();
    if show_files {
        for f in files {
            items.push((f, false));
        }
    }
    for d in dirs {
        items.push((d, true));
    }

    for (idx, (item, is_dir)) in items.iter().enumerate() {
        let last = idx == items.len() - 1;
        let conn = if ascii {
            if last { "`-- " } else { "|-- " }
        } else if last {
            "└── "
        } else {
            "├── "
        };
        let name = item.file_name().unwrap_or_default().to_string_lossy();
        println!("{prefix}{conn}{name}");
        if *is_dir {
            let child_prefix = if ascii {
                if last { "    " } else { "|   " }
            } else if last {
                "    "
            } else {
                "│   "
            };
            print_dir(item, &format!("{prefix}{child_prefix}"), show_files, ascii, i18n);
        }
    }
}

fn display_root(root: &Path) -> String {
    let s = root.to_string_lossy().to_string();
    if cfg!(windows) {
        s.to_uppercase()
    } else {
        s
    }
}
