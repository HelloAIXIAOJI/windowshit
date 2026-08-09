/// Windows whoami 无参数输出：`NetBIOS名\用户名`。
/// NetBIOS 名 = 主机名小写并截断到 15 字符（NetBIOS 上限），用跨平台
/// `whoami` crate 统一获取，无平台分支。

/// 让 Windows 控制台用 UTF-8 输出，避免中文乱码
#[cfg(windows)]
fn setup_console_utf8() {
    // SAFETY: 只调用标准 Win32 API，无其他副作用
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleOutputCP(65001);
    }
}

fn main() {
    #[cfg(windows)]
    setup_console_utf8();

    // 原版全小写（实测 aixiaoji-deskto\aixiaoji）
    let user = whoami_rs::username().unwrap_or_default().to_lowercase();
    let netbios: String = whoami_rs::hostname()
        .unwrap_or_default()
        .to_lowercase()
        .chars()
        .take(15)
        .collect();
    println!("{netbios}\\{user}");
}
