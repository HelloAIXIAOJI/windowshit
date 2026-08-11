//! systeminfo —— 显示系统信息（复刻 Windows systeminfo.exe）。
//!
//! 通用字段复用 `sysinfo` + `os_info` + `hostname`（跨平台）；
//! Windows 专属字段（Product ID、安装日期、厂商、型号、BIOS 等）
//! 从注册表读取，其它平台省略这些字段。

use std::net::IpAddr;
use std::process::ExitCode;

use chrono::{Datelike, Timelike};
use sysinfo::System;
use windowshit_i18n::{FluentArgs, L10n};

/// 字段行：label 对齐到固定列（还原原版冒号列位置）。
fn line(label: &str, value: &str) -> String {
    format!("{label:<25} {value}")
}

/// 千分位
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// unix 时间戳 → 本地时间 "yyyy/M/d, H:mm:ss"（原版格式）
fn fmt_boot_time(ts: u64) -> String {
    match chrono::DateTime::from_timestamp(ts as i64, 0) {
        Some(dt) => {
            let l = dt.with_timezone(&chrono::Local);
            format!(
                "{}/{}/{}, {}:{}:{}",
                l.year(),
                l.month(),
                l.day(),
                l.hour(),
                l.minute(),
                l.second()
            )
        }
        None => String::new(),
    }
}

/// 网络卡信息：(description, friendly_name, ips)。统一走公共数据层。
fn network_cards() -> Vec<(String, String, Vec<IpAddr>)> {
    use windowshit_netinfo::{AdapterData, AdapterKind};
    let mut out = Vec::new();
    if let Ok(adapters) = windowshit_netinfo::get_adapters() {
        for a in adapters {
            // 原版 systeminfo 不列回环与隧道（Teredo）接口
            if a.kind == AdapterKind::Loopback || a.kind == AdapterKind::Tunnel {
                continue;
            }
            let ips: Vec<IpAddr> = a
                .ipv4
                .iter()
                .map(|(ip, _)| IpAddr::V4(*ip))
                .chain(a.ipv6.iter().map(|(ip, _)| IpAddr::V6(*ip)))
                .collect();
            let AdapterData {
                friendly_name,
                description,
                ..
            } = a;
            out.push((description, friendly_name, ips));
        }
    }
    out
}

fn main() -> ExitCode {
    let mut i18n = L10n::detect();
    match i18n.lang() {
        "zh-CN" => i18n.add_ftl(include_str!("../locales/zh-CN.ftl")),
        _ => i18n.add_ftl(include_str!("../locales/en-US.ftl")),
    }

    L10n::setup_console_utf8();

    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.iter().any(|a| a == "/?" || a == "-?") {
        if i18n.lang() == "zh-CN" {
            println!("SYSTEMINFO  [/FO format] [/S system] [/U username] [/P password]");
            println!();
            println!("该工具显示有关计算机及其操作系统的信息，包括硬件配置，");
            println!("软件信息，计算机名，处理器，内存，网络信息，以及操作系统补丁信息。");
        } else {
            println!("SYSTEMINFO  [/FO format] [/S system] [/U username] [/P password]");
            println!();
            println!("This tool displays information about a computer and its operating system,");
            println!("including hardware configuration, software information, computer name,");
            println!(
                "processor, memory, network information, and operating system patch information."
            );
        }
        return ExitCode::SUCCESS;
    }

    let mut sys = System::new_all();
    sys.refresh_all();

    let host = hostname_rs::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();
    let os = os_info::get();

    let mut lines: Vec<String> = Vec::new();
    lines.push(String::new()); // 首行空行（原版）
    lines.push(line(&i18n.tr("host-name", None), &host));

    // OS Name / OS Version
    #[cfg(windows)]
    let os_name = windowshit_winreg::reg_query_string(
        "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion",
        "ProductName",
    )
    .map(|p| format!("Microsoft {p}"))
    .unwrap_or_else(|| os.os_type().to_string());
    #[cfg(not(windows))]
    let os_name = {
        let mut n = os.os_type().to_string();
        if let Some(v) = os.version().to_string().split_whitespace().next() {
            n.push(' ');
            n.push_str(v);
        }
        n
    };
    lines.push(line(&i18n.tr("os-name", None), &os_name));
    lines.push(line(
        &i18n.tr("os-version", None),
        &os.version().to_string(),
    ));

    #[cfg(windows)]
    {
        lines.push(line(
            &i18n.tr("os-manufacturer", None),
            "Microsoft Corporation",
        ));
        lines.push(line(
            &i18n.tr("os-configuration", None),
            "Standalone Workstation",
        ));
        lines.push(line(&i18n.tr("os-build-type", None), "Multiprocessor Free"));

        let win_ver = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion";
        if let Some(v) = windowshit_winreg::reg_query_string(win_ver, "RegisteredOwner") {
            lines.push(line(&i18n.tr("registered-owner", None), &v));
        }
        if let Some(v) = windowshit_winreg::reg_query_string(win_ver, "RegisteredOrganization") {
            lines.push(line(&i18n.tr("registered-org", None), &v));
        }
        if let Some(v) = windowshit_winreg::reg_query_string(win_ver, "ProductId") {
            lines.push(line(&i18n.tr("product-id", None), &v));
        }
        if let Some(v) = windowshit_winreg::reg_query_string(win_ver, "InstallDate")
            .and_then(|s| s.parse::<u64>().ok())
        {
            lines.push(line(&i18n.tr("original-install", None), &fmt_boot_time(v)));
        }
        let bios = "HARDWARE\\DESCRIPTION\\System\\BIOS";
        if let Some(v) = windowshit_winreg::reg_query_string(bios, "SystemManufacturer") {
            lines.push(line(&i18n.tr("sys-manufacturer", None), &v));
        }
        if let Some(v) = windowshit_winreg::reg_query_string(bios, "SystemProductName") {
            lines.push(line(&i18n.tr("sys-model", None), &v));
        }
        let sys_type = if std::mem::size_of::<usize>() == 8 {
            "x64-based PC"
        } else {
            "x86-based PC"
        };
        lines.push(line(&i18n.tr("sys-type", None), sys_type));
    }

    lines.push(line(
        &i18n.tr("boot-time", None),
        &fmt_boot_time(System::boot_time()),
    ));

    #[cfg(windows)]
    {
        let bios = "HARDWARE\\DESCRIPTION\\System\\BIOS";
        if let Some(v) =
            windowshit_winreg::reg_query_string(bios, "BIOSVersion").map(|s| s.replace(',', ", "))
        {
            lines.push(line(&i18n.tr("bios-version", None), &v));
        }
        if let Some(v) = std::env::var_os("WINDIR").map(|v| v.to_string_lossy().to_string()) {
            lines.push(line(&i18n.tr("windows-dir", None), &v));
        }
        if let Some(v) = std::env::var_os("SystemRoot").map(|v| v.to_string_lossy().to_string()) {
            let dir = format!("{v}\\system32");
            lines.push(line(&i18n.tr("system-dir", None), &dir));
        }
    }

    // Processor(s)
    let cpus = sys.cpus();
    let mut p_lines: Vec<String> = Vec::new();
    for (idx, cpu) in cpus.iter().take(32).enumerate() {
        p_lines.push(format!(
            "{:>27} [{:02}]: {} ~{} Mhz",
            "",
            idx + 1,
            cpu.brand(),
            cpu.frequency()
        ));
    }
    let count_label = if cpus.len() == 1 {
        "1 Processor(s) Installed."
    } else {
        &format!("{} Processor(s) Installed.", cpus.len())
    };
    lines.push(line(&i18n.tr("processors", None), count_label));
    lines.extend(p_lines);

    // 内存（sysinfo 0.39 返回字节，原版显示 MB）
    let total = sys.total_memory() / 1024 / 1024;
    let available = sys.available_memory() / 1024 / 1024;
    let total_swap = sys.total_swap() / 1024 / 1024;
    let free_swap = sys.free_swap() / 1024 / 1024;
    let vm_max = total + total_swap;
    let vm_avail = available + free_swap;
    let vm_in_use = vm_max.saturating_sub(vm_avail);

    lines.push(line(
        &i18n.tr("total-physical", None),
        &format!("{} MB", thousands(total)),
    ));
    lines.push(line(
        &i18n.tr("available-physical", None),
        &format!("{} MB", thousands(available)),
    ));
    lines.push(line(
        &i18n.tr("vm-max", None),
        &format!("{} MB", thousands(vm_max)),
    ));
    lines.push(line(
        &i18n.tr("vm-available", None),
        &format!("{} MB", thousands(vm_avail)),
    ));
    lines.push(line(
        &i18n.tr("vm-in-use", None),
        &format!("{} MB", thousands(vm_in_use)),
    ));

    // 网络卡
    let cards = network_cards();
    let mut a = FluentArgs::new();
    a.set("count", cards.len() as u64);
    lines.push(line(
        &i18n.tr("network-cards", None),
        &i18n.tr("installed", Some(&a)),
    ));
    for (idx, (desc, friendly, ips)) in cards.iter().enumerate() {
        lines.push(format!("{:>27} [{:02}]: {desc}", "", idx + 1));
        lines.push(format!(
            "{:>33} {}: {friendly}",
            "",
            i18n.tr("connection-name", None)
        ));
        lines.push(format!("{:>33} {}", "", i18n.tr("ip-addresses", None)));
        for (i, ip) in ips.iter().enumerate() {
            lines.push(format!("{:>33} [{:02}]: {ip}", "", i + 1));
        }
    }

    for l in &lines {
        println!("{l}");
    }

    ExitCode::SUCCESS
}
