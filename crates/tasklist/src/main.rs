//! tasklist —— 显示进程列表（复刻 Windows tasklist.exe）。
//!
//! 进程数据复用 `sysinfo` crate（跨平台）。
//! 会话号是 Windows 概念：sysinfo 的 session_id() 在 Windows 上内部走
//! ProcessIdToSessionId（跨会话/受保护进程返回 ACCESS_DENIED），
//! Linux 上是 getsid()（POSIX 会话 ID，语义不同），两者都不能用，
//! 故 Windows 用 WTSEnumerateProcessesW（原版同机制），其它平台 N/A。

use std::collections::HashMap;
use std::process::ExitCode;

use sysinfo::System;
use windowshit_args::{parse, Flag, Kind, Unknown};
use windowshit_i18n::L10n;

struct Row {
    name: String,
    pid: u32,
    session_name: String,
    session_no: u32,
    mem_kb: u64,
}

/// 进程会话映射：pid → session id。
///
/// Windows 用 WTSEnumerateProcessesW 一次性枚举（普通权限可用，原版 tasklist
/// 同机制）。ProcessIdToSessionId 对跨会话/受保护进程返回 ACCESS_DENIED，
/// 故仅作为 WTS 失败时的兜底。其它平台无会话概念，返回空。
#[cfg(windows)]
fn session_map() -> HashMap<u32, u32> {
    use windows_sys::Win32::System::RemoteDesktop::{
        WTSEnumerateProcessesW, WTSFreeMemory, WTS_PROCESS_INFOW,
    };

    let mut map = HashMap::new();
    // SAFETY: WTS API，内存由 WTSFreeMemory 释放
    unsafe {
        let mut infos: *mut WTS_PROCESS_INFOW = std::ptr::null_mut();
        let mut count: u32 = 0;
        let ret = WTSEnumerateProcessesW(0, 0, 1, &mut infos, &mut count);
        if ret != 0 {
            for i in 0..count {
                let info = &*infos.add(i as usize);
                map.insert(info.ProcessId, info.SessionId);
            }
            WTSFreeMemory(infos as *mut std::ffi::c_void);
        }
    }
    map
}

#[cfg(not(windows))]
fn session_map() -> HashMap<u32, u32> {
    HashMap::new()
}

/// 兜底：单进程会话查询（ProcessIdToSessionId，跨会话时可能失败）。
#[cfg(windows)]
fn session_fallback(pid: u32) -> Option<u32> {
    unsafe extern "system" {
        fn ProcessIdToSessionId(process_id: u32, session_id: *mut u32) -> i32;
    }
    // SAFETY: 标准 API，缓冲区为栈上 u32
    let mut sid: u32 = 0;
    let ret = unsafe { ProcessIdToSessionId(pid, &mut sid) };
    if ret != 0 {
        Some(sid)
    } else {
        None
    }
}

#[cfg(not(windows))]
fn session_fallback(_pid: u32) -> Option<u32> {
    None
}

/// 千分位 + " K"（如 40,240 K）
fn format_kb(kb: u64) -> String {
    let digits = kb.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    format!("{out} K")
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

    // 精确开关表；未知 /xxx 忽略（原版 tasklist 对未知开关不报错）
    const FLAGS: &[Flag] = &[Flag::new("FO", Kind::Value), Flag::new("NH", Kind::Flag)];
    let parsed = parse(&raw, FLAGS, Unknown::Ignore).unwrap_or_default();
    let mut format = "TABLE".to_string();
    if let Some(v) = parsed.flags.get("FO").and_then(|v| *v) {
        format = v.to_uppercase();
    }
    let nh = parsed.flags.contains_key("NH");

    if !matches!(format.as_str(), "TABLE" | "LIST" | "CSV") {
        eprintln!("ERROR: Invalid argument/option - '/FO:{format}'");
        return ExitCode::from(1);
    }

    // 采集进程
    let mut sys = System::new_all();
    sys.refresh_all();

    let sessions = session_map();

    let mut rows: Vec<Row> = Vec::new();
    for proc_ in sys.processes().values() {
        let pid = proc_.pid().as_u32();
        // 会话号：WTS 枚举优先，兜底单查，再无则 0
        let sid = sessions
            .get(&pid)
            .copied()
            .or_else(|| session_fallback(pid))
            .unwrap_or(0);
        let (sname, sno) = if sid == 0 {
            ("Services".to_string(), 0)
        } else {
            ("Console".to_string(), sid)
        };
        rows.push(Row {
            name: proc_.name().to_string_lossy().to_string(),
            pid,
            session_name: sname,
            session_no: sno,
            // sysinfo 0.39 的 memory() 返回字节，tasklist 显示 KB
            mem_kb: proc_.memory() / 1024,
        });
    }
    rows.sort_by_key(|r| r.pid);

    let col_image = i18n.tr("col-image", None);
    let col_pid = i18n.tr("col-pid", None);
    let col_session = i18n.tr("col-session", None);
    let col_sno = i18n.tr("col-session-no", None);
    let col_mem = i18n.tr("col-mem", None);

    match format.as_str() {
        "TABLE" => {
            if !nh {
                println!(
                    "{:<25} {:>8} {:<16} {:>11} {:>12}",
                    col_image, col_pid, col_session, col_sno, col_mem
                );
                println!(
                    "{} {} {} {} {}",
                    "=".repeat(25),
                    "=".repeat(8),
                    "=".repeat(16),
                    "=".repeat(11),
                    "=".repeat(12)
                );
            }
            for r in &rows {
                println!(
                    "{:<25} {:>8} {:<16} {:>11} {:>12}",
                    r.name,
                    r.pid,
                    r.session_name,
                    r.session_no,
                    format_kb(r.mem_kb)
                );
            }
        }
        "LIST" => {
            println!("{}", i18n.tr("info-building", None));
            for r in &rows {
                println!("{col_image}:  {}", r.name);
                println!("{col_pid}:  {}", r.pid);
                println!("{col_session}: {}", r.session_name);
                println!("{col_sno}: {}", r.session_no);
                println!("{col_mem}: {}", format_kb(r.mem_kb));
                println!();
            }
        }
        "CSV" => {
            if !nh {
                println!(
                    "\"{col_image}\",\"{col_pid}\",\"{col_session}\",\"{col_sno}\",\"{col_mem}\""
                );
            }
            for r in &rows {
                println!(
                    "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"",
                    r.name,
                    r.pid,
                    r.session_name,
                    r.session_no,
                    format_kb(r.mem_kb)
                );
            }
        }
        _ => unreachable!(),
    }

    ExitCode::SUCCESS
}
