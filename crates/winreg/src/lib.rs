//! windowshit-winreg —— 注册表读取辅助（库 crate）。
//!
//! 统一封装 Windows 注册表样板代码（宽字符编码、RegOpenKeyExW、
//! RegQueryValueExW、RegEnumKeyExW），供 `systeminfo`（REG_SZ）、
//! `ver`（REG_DWORD）、`getmac`（枚举子键）复用。
//!
//! 非 Windows 平台参考 `regedit` 的做法，把注册表映射到 Linux 配置文件夹：
//! `HKEY_LOCAL_MACHINE`→`/etc`、`HKEY_CURRENT_USER`→`~/.config`、
//! `HKEY_SYSTEM_BOOT`→`/boot`，子键对应目录、值对应目录内配置文件里的
//! `key = value` 配置项。组件无需写平台分支。

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
        Some(
            String::from_utf16_lossy(&u16s)
                .trim_end_matches('\0')
                .to_string(),
        )
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

// ---------------------------------------------------------------------------
// Unix 实现：把注册表映射到 Linux 配置文件夹。
//
// 根键 → 目录：
//   HKEY_LOCAL_MACHINE（系统级配置）→ /etc
//   HKEY_CURRENT_USER（用户级配置） → ~/.config
//   HKEY_SYSTEM_BOOT（引导目录）    → /boot
// 子键路径 → 目录树；值 → 目录下配置文件里的一行「key = value」配置项。
// 注册表键名不分大小写，故按大小写不敏感匹配。
// ---------------------------------------------------------------------------

/// 按注册表根键前缀解析出对应的配置基目录。
#[cfg(not(windows))]
fn unix_base_dir(key_path: &str) -> Option<std::path::PathBuf> {
    let first = key_path.split('\\').next().unwrap_or("").trim().to_ascii_uppercase();
    match first.as_str() {
        "HKEY_CURRENT_USER" => std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .map(|h| h.join(".config")),
        "HKEY_SYSTEM_BOOT" => Some(std::path::PathBuf::from("/boot")),
        // 默认基于 HKLM（系统级配置）
        _ => Some(std::path::PathBuf::from("/etc")),
    }
}

/// 把注册表键路径解析为配置文件夹下的绝对目录；目录不存在时返回 None。
#[cfg(not(windows))]
fn unix_resolve_dir(key_path: &str) -> Option<std::path::PathBuf> {
    let base = unix_base_dir(key_path)?;
    let mut path = base;
    for (i, part) in key_path.split('\\').map(str::trim).filter(|s| !s.is_empty()).enumerate() {
        if i == 0 {
            let up = part.to_ascii_uppercase();
            if up == "HKEY_CURRENT_USER" || up == "HKEY_SYSTEM_BOOT" || up == "HKEY_LOCAL_MACHINE" {
                continue;
            }
        }
        path = path.join(part);
    }
    path.is_dir().then_some(path)
}

/// 从单行配置中拆出「键 / 值」；支持 `key=value`、`key: value`、`key value`。
#[cfg(not(windows))]
fn unix_split_key_value(line: &str) -> Option<(&str, &str)> {
    for (i, c) in line.char_indices() {
        if c == '=' || c == ':' {
            return Some((line[..i].trim(), line[i + 1..].trim()));
        }
        if c.is_whitespace() {
            return Some((line[..i].trim(), line[i..].trim()));
        }
    }
    None
}

/// 清洗配置值：去包裹引号、去尾部注释。
#[cfg(not(windows))]
fn unix_clean_value(v: &str) -> String {
    let mut s = v.trim().to_string();
    for sep in [" #", "\t#", " ;", "\t;"] {
        if let Some(idx) = s.find(sep) {
            s.truncate(idx);
        }
    }
    s.trim().trim_matches(|c| c == '"' || c == '\'').to_string()
}

/// 在配置文件内容中查找键 `name` 对应的值（大小写不敏感）。
#[cfg(not(windows))]
fn unix_value_from_config(content: &str, name: &str) -> Option<String> {
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            continue; // INI 节
        }
        if let Some((k, v)) = unix_split_key_value(line) {
            if k.eq_ignore_ascii_case(name) {
                return Some(unix_clean_value(v));
            }
        }
    }
    None
}

/// 读取目录下的文本配置文件，查找名为 `name` 的配置项。
#[cfg(not(windows))]
fn unix_read_config_value(key_path: &str, name: &str) -> Option<String> {
    let dir = unix_resolve_dir(key_path)?;
    for entry in std::fs::read_dir(&dir).ok()?.flatten() {
        if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                if let Some(v) = unix_value_from_config(&content, name) {
                    return Some(v);
                }
            }
        }
    }
    None
}

/// 把字符串解析为 u32（支持 0x 十六进制）。
#[cfg(not(windows))]
fn unix_parse_dword(v: &str) -> Option<u32> {
    let t = v.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u32::from_str_radix(hex, 16).ok();
    }
    t.parse::<u32>().ok()
}

/// 读取配置文件夹下对应目录中名为 `name` 的字符串配置项。
#[cfg(not(windows))]
pub fn reg_query_string(key_path: &str, name: &str) -> Option<String> {
    unix_read_config_value(key_path, name)
}

/// 读取配置文件夹下对应目录中名为 `name` 的数字配置项。
#[cfg(not(windows))]
pub fn reg_query_dword(key_path: &str, name: &str) -> Option<u32> {
    unix_parse_dword(&unix_read_config_value(key_path, name)?)
}

/// 枚举配置文件夹下对应目录的直接子目录名（类比子键）。
#[cfg(not(windows))]
pub fn reg_enum_child_names(key_path: &str) -> Vec<String> {
    let Some(dir) = unix_resolve_dir(key_path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                out.push(e.file_name().to_string_lossy().to_string());
            }
        }
    }
    out
}
