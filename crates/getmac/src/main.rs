//! getmac —— 显示网卡 MAC 地址（复刻 Windows getmac.exe）。
//!
//! 数据层：Windows 用 `ipconfig` crate + 注册表 GUID 映射 transport name；
//! unix 用 `netdev`。格式层跨平台共用。

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

#[derive(Debug)]
struct Adapter {
    mac: String,
    transport: String,
    friendly: String,
    description: String,
}

/// 数据采集：Windows / unix 各自填字段（数据来源不同，无共享逻辑可抽）。
#[cfg(windows)]
fn collect() -> Vec<Adapter> {
    let mut out = Vec::new();
    if let Ok(adapters) = ipconfig::get_adapters() {
        for a in adapters {
            // 原版 getmac 不显示回环与隧道（Teredo 等）适配器
            let kind = a.if_type();
            if kind == ipconfig::IfType::SoftwareLoopback || kind == ipconfig::IfType::Tunnel {
                continue;
            }
            let friendly = a.friendly_name().to_string();
            let mac = a
                .physical_address()
                .map(|v| {
                    v.iter()
                        .map(|b| format!("{b:02X}"))
                        .collect::<Vec<_>>()
                        .join("-")
                })
                .unwrap_or_default();
            // 原版 getmac 不显示无物理地址的适配器（如 Wintun）
            if mac.is_empty() {
                continue;
            }
            let connected = a.oper_status() == ipconfig::OperStatus::IfOperStatusUp;
            let transport = if connected {
                match find_guid(&friendly) {
                    Some(guid) => format!("\\Device\\Tcpip_{guid}"), // guid 已含花括号
                    None => String::new(),
                }
            } else {
                "Media disconnected".to_string()
            };
            out.push(Adapter {
                mac,
                transport,
                friendly,
                description: a.description().to_string(),
            });
        }
    }
    out
}

#[cfg(not(windows))]
fn collect() -> Vec<Adapter> {
    let mut out = Vec::new();
    for i in netdev::get_interfaces() {
        if i.name.starts_with("lo") {
            continue;
        }
        let mac = i.mac_addr.map(|m| m.to_string().to_uppercase()).unwrap_or_default();
        out.push(Adapter {
            mac,
            transport: i.name.clone(),
            friendly: i.name.clone(),
            description: String::new(),
        });
    }
    out
}

/// 通过注册表反查适配器的 interface GUID。
/// 键: HKLM\SYSTEM\CurrentControlSet\Control\Network\{4D36E972-E325-11CE-BFC1-08002BE10318}\<GUID>\Connection
/// 其 Name 值为友好名称。
#[cfg(windows)]
fn find_guid(friendly: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE,
        KEY_READ,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    // SAFETY: 标准注册表 API，缓冲区类型与大小正确
    unsafe {
        let net_root =
            wide("SYSTEM\\CurrentControlSet\\Control\\Network\\{4D36E972-E325-11CE-BFC1-08002BE10318}");
        let mut root_key: HKEY = 0;
        if RegOpenKeyExW(HKEY_LOCAL_MACHINE, net_root.as_ptr(), 0, KEY_READ, &mut root_key) != 0 {
            return None;
        }

        let mut index: u32 = 0;
        let mut guid_buf = [0u16; 39];
        loop {
            let mut name_len: u32 = guid_buf.len() as u32;
            let ret = RegEnumKeyExW(
                root_key,
                index,
                guid_buf.as_mut_ptr(),
                &mut name_len,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            if ret != 0 {
                break;
            }
            let guid = String::from_utf16_lossy(&guid_buf[..name_len as usize]);

            let conn_path = format!(
                "SYSTEM\\CurrentControlSet\\Control\\Network\\{{4D36E972-E325-11CE-BFC1-08002BE10318}}\\{guid}\\Connection"
            );
            let conn_wide = wide(&conn_path);
            let mut conn_key: HKEY = 0;
            if RegOpenKeyExW(HKEY_LOCAL_MACHINE, conn_wide.as_ptr(), 0, KEY_READ, &mut conn_key) == 0 {
                let mut buf = [0u8; 2048];
                let mut len: u32 = buf.len() as u32;
                let name_wide = wide("Name");
                let qret = RegQueryValueExW(
                    conn_key,
                    name_wide.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    buf.as_mut_ptr(),
                    &mut len,
                );
                RegCloseKey(conn_key);
                if qret == 0 && len >= 2 {
                    // REG_SZ 是 UTF-16LE
                    let count = (len as usize) / 2;
                    let mut u16s = Vec::with_capacity(count);
                    for i in 0..count {
                        u16s.push(u16::from_le_bytes([buf[i * 2], buf[i * 2 + 1]]));
                    }
                    let text = String::from_utf16_lossy(&u16s)
                        .trim_end_matches('\0')
                        .to_string();
                    if text.eq_ignore_ascii_case(friendly) {
                        RegCloseKey(root_key);
                        return Some(guid);
                    }
                }
            }
            index += 1;
        }
        RegCloseKey(root_key);
        None
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

    let raw: Vec<String> = std::env::args().skip(1).collect();

    if raw.iter().any(|a| a == "/?" || a == "-?") {
        println!("{}", i18n.help());
        return ExitCode::SUCCESS;
    }

    let mut format = "TABLE".to_string();
    let mut nh = false;
    let mut verbose = false;

    let mut i = 0usize;
    while i < raw.len() {
        let a = &raw[i];
        let upper = a.to_ascii_uppercase();
        if upper == "/FO" || upper == "-FO" {
            i += 1;
            if i < raw.len() {
                format = raw[i].to_uppercase();
            }
        } else if upper == "/NH" || upper == "-NH" {
            nh = true;
        } else if upper == "/V" || upper == "-V" {
            verbose = true;
        }
        i += 1;
    }

    if !matches!(format.as_str(), "TABLE" | "LIST" | "CSV") {
        let mut a = FluentArgs::new();
        a.set("fmt", &format);
        eprintln!("{}", i18n.tr("error-bad-format", Some(&a)));
        return ExitCode::from(1);
    }

    let adapters = collect();

    let col_p = i18n.tr("col-physical", None);
    let col_t = i18n.tr("col-transport", None);
    let f_conn = i18n.tr("field-connection", None);
    let f_net = i18n.tr("field-network-adapter", None);
    let f_phys = i18n.tr("field-physical", None);
    let f_trans = i18n.tr("field-transport", None);
    let media = i18n.tr("media-disconnected", None);

    // 预处理：Media disconnected 显示本地化文本
    let rows: Vec<(&Adapter, String)> = adapters
        .iter()
        .map(|a| {
            let t = if a.transport == "Media disconnected" {
                media.clone()
            } else {
                a.transport.clone()
            };
            (a, t)
        })
        .collect();

    match format.as_str() {
        "TABLE" => {
            if !nh {
                println!("{:<19} {col_t}", col_p);
                println!("{} {}", "-".repeat(19), "-".repeat(30));
            }
            for (a, t) in &rows {
                println!("{:<19} {t}", a.mac);
            }
        }
        "LIST" => {
            for (a, t) in &rows {
                if verbose {
                    println!("{f_conn}:  {}", a.friendly);
                    println!("{f_net}:  {}", a.description);
                    println!("{f_phys}: {}", a.mac);
                    println!("{f_trans}:  {t}");
                } else {
                    println!("{f_phys}: {}", a.mac);
                    println!("{f_trans}:  {t}");
                }
                println!();
            }
        }
        "CSV" => {
            if !nh {
                println!("\"{col_p}\",\"{col_t}\"");
            }
            for (a, t) in &rows {
                println!("\"{}\",\"{}\"", a.mac, t);
            }
        }
        _ => unreachable!(),
    }

    ExitCode::SUCCESS
}
