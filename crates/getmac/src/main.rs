//! getmac —— 显示网卡 MAC 地址（复刻 Windows getmac.exe）。
//!
//! 数据层统一走 `windowshit-netinfo`（跨平台），注册表 GUID 反查走
//! `windowshit-winreg`。本文件无平台分支。

use std::process::ExitCode;

use windowshit_args::{parse, Flag, Kind, Parsed, Unknown};
use windowshit_i18n::{FluentArgs, L10n};
use windowshit_netinfo::{AdapterData, AdapterKind};

#[derive(Debug)]
struct Adapter {
    mac: String,
    transport: String,
    friendly: String,
    description: String,
}

/// 数据采集：统一走公共数据层。
fn collect() -> Vec<Adapter> {
    let mut out = Vec::new();
    if let Ok(adapters) = windowshit_netinfo::get_adapters() {
        for a in adapters {
            // 原版 getmac 不显示回环与隧道（Teredo 等）适配器
            if a.kind == AdapterKind::Loopback || a.kind == AdapterKind::Tunnel {
                continue;
            }
            let AdapterData {
                friendly_name,
                mac,
                is_up,
                description,
                ..
            } = a;
            let mac = mac
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
            let transport = if is_up {
                match find_guid(&friendly_name) {
                    Some(guid) => format!("\\Device\\Tcpip_{guid}"), // guid 已含花括号
                    // unix 无注册表，fallback 到接口名；Windows 兜底
                    None => friendly_name.clone(),
                }
            } else {
                "Media disconnected".to_string()
            };
            out.push(Adapter {
                mac,
                transport,
                friendly: friendly_name,
                description,
            });
        }
    }
    out
}

/// 通过注册表反查适配器的 interface GUID。
/// 键: HKLM\SYSTEM\CurrentControlSet\Control\Network\{4D36E972-E325-11CE-BFC1-08002BE10318}\<GUID>\Connection
/// 其 Name 值为友好名称。非 Windows 平台为空实现，直接返回 None。
fn find_guid(friendly: &str) -> Option<String> {
    const NET_ROOT: &str =
        "SYSTEM\\CurrentControlSet\\Control\\Network\\{4D36E972-E325-11CE-BFC1-08002BE10318}";
    for guid in windowshit_winreg::reg_enum_child_names(NET_ROOT) {
        let conn = format!("{NET_ROOT}\\{guid}\\Connection");
        if let Some(name) = windowshit_winreg::reg_query_string(&conn, "Name") {
            if name.eq_ignore_ascii_case(friendly) {
                return Some(guid);
            }
        }
    }
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

    L10n::setup_console_utf8();

    let raw: Vec<String> = std::env::args().skip(1).collect();

    if raw.iter().any(|a| a == "/?" || a == "-?") {
        println!("{}", i18n.help());
        return ExitCode::SUCCESS;
    }

    // 精确开关表；未知 /xxx 忽略（原版 getmac 对未知开关不报错）
    const FLAGS: &[Flag] = &[
        Flag::new("FO", Kind::Value),
        Flag::new("NH", Kind::Flag),
        Flag::new("V", Kind::Flag),
    ];
    let parsed = match parse(&raw, FLAGS, Unknown::Ignore) {
        Ok(p) => p,
        Err(_) => Parsed::default(),
    };
    let mut format = "TABLE".to_string();
    if let Some(v) = parsed.flags.get("FO").and_then(|v| *v) {
        format = v.to_uppercase();
    }
    let nh = parsed.flags.contains_key("NH");
    let verbose = parsed.flags.contains_key("V");

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
