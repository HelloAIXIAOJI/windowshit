//! windowshit-winreg —— 注册表读取辅助（库 crate）。
//!
//! 统一封装 Windows 注册表样板代码（宽字符编码、RegOpenKeyExW、
//! RegQueryValueExW、RegEnumKeyExW），供 `systeminfo`（REG_SZ）、
//! `ver`（REG_DWORD）、`getmac`（枚举子键）复用。
//! 非 Windows 平台为空实现，组件无需写平台分支。

/// 读取 REG_SZ 字符串值（UTF-16LE）。
#[cfg(windows)]
pub fn reg_query_string(key_path: &str, name: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ,
    };
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    // SAFETY: 标准注册表 API
    unsafe {
        let key_wide = wide(key_path);
        let mut key: HKEY = 0;
        if RegOpenKeyExW(HKEY_LOCAL_MACHINE, key_wide.as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return None;
        }
        let name_wide = wide(name);
        let mut buf = [0u8; 4096];
        let mut len: u32 = buf.len() as u32;
        let ret = RegQueryValueExW(
            key,
            name_wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
            buf.as_mut_ptr(),
            &mut len,
        );
        RegCloseKey(key);
        if ret != 0 || len < 2 {
            return None;
        }
        let count = (len as usize) / 2;
        let mut u16s = Vec::with_capacity(count);
        for i in 0..count {
            u16s.push(u16::from_le_bytes([buf[i * 2], buf[i * 2 + 1]]));
        }
        Some(String::from_utf16_lossy(&u16s).trim_end_matches('\0').to_string())
    }
}

/// 读取 REG_DWORD 值（u32）。
#[cfg(windows)]
pub fn reg_query_dword(key_path: &str, name: &str) -> Option<u32> {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ,
    };
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    // SAFETY: 标准注册表 API
    unsafe {
        let key_wide = wide(key_path);
        let mut key: HKEY = 0;
        if RegOpenKeyExW(HKEY_LOCAL_MACHINE, key_wide.as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return None;
        }
        let name_wide = wide(name);
        let mut value: u32 = 0;
        let mut len: u32 = std::mem::size_of::<u32>() as u32;
        let ret = RegQueryValueExW(
            key,
            name_wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
            &mut value as *mut u32 as *mut u8,
            &mut len,
        );
        RegCloseKey(key);
        if ret != 0 || len != std::mem::size_of::<u32>() as u32 {
            return None;
        }
        Some(value)
    }
}

/// 枚举指定键下的直接子键名。
#[cfg(windows)]
pub fn reg_enum_child_names(key_path: &str) -> Vec<String> {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ,
    };
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    let mut out = Vec::new();
    // SAFETY: 标准注册表 API
    unsafe {
        let key_wide = wide(key_path);
        let mut key: HKEY = 0;
        if RegOpenKeyExW(HKEY_LOCAL_MACHINE, key_wide.as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return out;
        }
        let mut index: u32 = 0;
        let mut buf = [0u16; 256];
        loop {
            let mut name_len: u32 = buf.len() as u32;
            let ret = RegEnumKeyExW(
                key,
                index,
                buf.as_mut_ptr(),
                &mut name_len,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            if ret != 0 {
                break;
            }
            out.push(String::from_utf16_lossy(&buf[..name_len as usize]));
            index += 1;
        }
        RegCloseKey(key);
    }
    out
}

#[cfg(not(windows))]
pub fn reg_query_string(_key_path: &str, _name: &str) -> Option<String> {
    None
}

#[cfg(not(windows))]
pub fn reg_query_dword(_key_path: &str, _name: &str) -> Option<u32> {
    None
}

#[cfg(not(windows))]
pub fn reg_enum_child_names(_key_path: &str) -> Vec<String> {
    Vec::new()
}
