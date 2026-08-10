//! sort —— 文本排序（复刻 Windows sort.exe）。
//!
//! 纯标准库，无平台分支。排序始终不区分大小写（还原原版行为）。

use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::process::ExitCode;

use windowshit_i18n::{FluentArgs, L10n};

/// 让 Windows 控制台用 UTF-8 输出
#[cfg(windows)]
fn setup_console_utf8() {
    // SAFETY: 只调用标准 Win32 API，无其他副作用
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleOutputCP(65001);
    }
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

    let mut reverse = false;
    let mut key_start: usize = 0; // /+n：从第 n 个字符开始比较（1-based）
    let mut input_file: Option<String> = None;
    let mut output_file: Option<String> = None;

    let mut i = 0usize;
    while i < raw.len() {
        let a = &raw[i];
        if a.starts_with('/') || a.starts_with('-') {
            let up = a[1..].to_ascii_uppercase();
            if up == "R" {
                // 精确 /R：反向
                reverse = true;
            } else if let Some(n) = up.strip_prefix('+') {
                // /+n：从第 n 个字符开始比较
                if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) {
                    key_start = n.parse().unwrap_or(0);
                } else {
                    input_file = Some(a.clone());
                }
            } else if up == "O" {
                // /O file
                i += 1;
                if i < raw.len() {
                    output_file = Some(raw[i].clone());
                }
            } else if let Some(rest) = up.strip_prefix("O:") {
                // /O:file
                output_file = Some(rest.to_string());
            } else if matches!(up.as_str(), "M" | "L" | "REC" | "T") {
                // /M /L /REC /T：忽略（内存/区域/记录长度/临时目录不影响结果）
                // 只认精确开关，避免 Linux 绝对路径被误判
            } else {
                // 未知 /xxx：按路径处理（Linux 绝对路径以 / 开头）
                input_file = Some(a.clone());
            }
        } else {
            input_file = Some(a.clone());
        }
        i += 1;
    }

    // 读输入：文件或 stdin
    let content = match &input_file {
        Some(f) => match fs::read_to_string(f) {
            Ok(c) => c,
            Err(e) => {
                let err = e.to_string();
                let mut a = FluentArgs::new();
                a.set("file", f.as_str());
                a.set("err", &err);
                eprintln!("{}", i18n.tr("error-open-input", Some(&a)));
                return ExitCode::from(1);
            }
        },
        None => {
            let mut buf = String::new();
            if io::stdin().read_to_string(&mut buf).is_err() {
                eprintln!("{}", i18n.tr("error-read-input", None));
                return ExitCode::from(1);
            }
            buf
        }
    };

    let mut lines: Vec<&str> = content.lines().collect();
    // 保留原始换行结尾：lines() 会去掉 \r\n 的 \r 吗？lines() 按 \n 切，保留 \r。
    // Windows 文件 \r\n 行会残留 \r，这里统一去掉行尾 \r。
    for line in lines.iter_mut() {
        if let Some(stripped) = line.strip_suffix('\r') {
            *line = stripped;
        }
    }

    // 比较键：从 key_start 处开始，大小写不敏感
    let key = |line: &str| -> String {
        let s: String = line
            .chars()
            .skip(key_start)
            .flat_map(char::to_lowercase)
            .collect();
        s
    };

    if reverse {
        lines.sort_by(|a, b| key(b).cmp(&key(a)));
    } else {
        lines.sort_by(|a, b| key(a).cmp(&key(b)));
    }

    let mut out_text = String::new();
    for line in &lines {
        out_text.push_str(line);
        out_text.push_str("\r\n");
    }

    match &output_file {
        Some(f) => {
            if let Err(e) = fs::write(f, out_text) {
                let err = e.to_string();
                let mut a = FluentArgs::new();
                a.set("file", f.as_str());
                a.set("err", &err);
                eprintln!("{}", i18n.tr("error-write-output", Some(&a)));
                return ExitCode::from(1);
            }
        }
        None => {
            let mut stdout = io::stdout();
            if stdout.write_all(out_text.as_bytes()).is_err() {
                return ExitCode::from(1);
            }
        }
    }

    ExitCode::SUCCESS
}
