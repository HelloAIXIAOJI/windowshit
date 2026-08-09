/// 输出系统版本信息。
/// Windows 上还原原版 ver 的 `Microsoft Windows [Version x]` 格式
/// （原版任何语言版本都是英文文本，故不随代码页翻译）。
/// 跨平台用 `os_info` crate，无平台分支。

use os_info::Type;

/// 让 Windows 控制台用 UTF-8 输出
#[cfg(windows)]
fn setup_console_utf8() {
    // SAFETY: 只调用标准 Win32 API，无其他副作用
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleOutputCP(65001);
    }
}

/// 读取 Windows 注册表中的 UBR（Update Build Revision），补齐 os_info
/// 缺失的 build 号（如 10.0.22621.**2428**）。仅 Windows 存在此字段，
/// 其它平台直接忽略。
#[cfg(windows)]
fn ubuild() -> Option<String> {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    // SAFETY: 标准注册表 API；UBR 是 REG_DWORD，按 u32 读取
    unsafe {
        let path = wide("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");
        let mut key: HKEY = 0;
        if RegOpenKeyExW(HKEY_LOCAL_MACHINE, path.as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return None;
        }
        let name = wide("UBR");
        let mut value: u32 = 0;
        let mut len: u32 = std::mem::size_of::<u32>() as u32;
        let ret = RegQueryValueExW(
            key,
            name.as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
            &mut value as *mut u32 as *mut u8,
            &mut len,
        );
        RegCloseKey(key);
        if ret != 0 || len != std::mem::size_of::<u32>() as u32 {
            return None;
        }
        Some(value.to_string())
    }
}

fn main() {
    #[cfg(windows)]
    setup_console_utf8();

    let info = os_info::get();
    let os = match info.os_type() {
        Type::Windows => "Microsoft Windows".to_string(),
        Type::Macos => "Apple macOS".to_string(),
        Type::Linux => "GNU Linux".to_string(),
        other => other.to_string(),
    };

    let version = {
        #[cfg(windows)]
        {
            let mut v = info.version().to_string();
            if let Some(ubr) = ubuild() {
                if !v.is_empty() {
                    v.push('.');
                    v.push_str(&ubr);
                }
            }
            v
        }
        #[cfg(not(windows))]
        {
            info.version().to_string()
        }
    };

    println!("{os} [Version {version}]");
}
