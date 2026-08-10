//! type —— 显示文本文件内容（复刻 Windows type.exe）。

use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use windowshit_i18n::L10n;

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

    L10n::setup_console_utf8();

    let raw: Vec<String> = env::args().skip(1).collect();

    if raw.is_empty() || raw.iter().any(|a| a == "/?" || a == "-?") {
        println!("{}", i18n.help());
        return if raw.is_empty() {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        };
    }

    let mut stdout = std::io::stdout();
    let mut failed = false;

    for arg in &raw {
        let path = Path::new(arg);
        match fs::read(path) {
            Ok(bytes) => {
                // 原版 type 原样输出字节（含 BOM/换行符），不转换编码
                if stdout.write_all(&bytes).is_err() {
                    return ExitCode::from(1);
                }
                if !bytes.ends_with(b"\n") {
                    let _ = stdout.write_all(b"\r\n");
                }
            }
            Err(_) => {
                let mut a = windowshit_i18n::FluentArgs::new();
                a.set("file", arg);
                eprintln!("{}", i18n.tr("error-file-not-found", Some(&a)));
                failed = true;
            }
        }
    }

    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
