/// 输出系统主机名（跨平台 `hostname` crate，无平台分支）。
/// 错误消息走 i18n 本地化。

use windowshit_i18n::L10n;

/// 让 Windows 控制台用 UTF-8 输出，避免中文乱码
#[cfg(windows)]
fn setup_console_utf8() {
    // SAFETY: 只调用标准 Win32 API，无其他副作用
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleOutputCP(65001);
    }
}

fn main() {
    // 必须先读代码页决定语言，再改 UTF-8 输出
    let mut i18n = L10n::detect();
    match i18n.lang() {
        "zh-CN" => i18n.add_ftl(include_str!("../locales/zh-CN.ftl")),
        _ => i18n.add_ftl(include_str!("../locales/en-US.ftl")),
    }

    #[cfg(windows)]
    setup_console_utf8();

    match hostname_rs::get() {
        Ok(name) => println!("{}", name.to_string_lossy()),
        Err(_) => eprintln!("{}", i18n.tr("error-get-hostname", None)),
    }
}
