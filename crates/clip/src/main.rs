//! clip —— 将命令输出重定向到 Windows 剪贴板（复刻 Windows clip.exe）。
//!
//! 复用 `arboard` crate（1Password 维护的跨平台剪贴板库，支持
//! Windows/macOS/Linux X11+Wayland），无平台分支。

use std::io::{self, Read};
use std::process::ExitCode;

use windowshit_i18n::L10n;

fn main() -> ExitCode {
    L10n::setup_console_utf8();

    // 读取 stdin 全部内容
    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("clip: 读取输入失败");
        return ExitCode::from(1);
    }

    // 写入剪贴板
    match arboard::Clipboard::new() {
        Ok(mut cb) => match cb.set_text(input) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("clip: 无法写入剪贴板: {e}");
                ExitCode::from(1)
            }
        },
        Err(e) => {
            eprintln!("clip: 无法打开剪贴板: {e}");
            ExitCode::from(1)
        }
    }
}
