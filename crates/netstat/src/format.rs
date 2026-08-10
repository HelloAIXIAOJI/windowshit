//! 输出格式化：还原 Windows netstat 的列对齐。
//!
//! 实测列宽（0-based）：
//!   Proto 从列 2，宽 3；Local 从列 9，宽 23；Foreign 从列 32，宽 23；
//!   State 从列 55，宽 16；PID 从列 71。
//! 字段为"最小宽度 + 左对齐"，内容超宽时后续列顺延（长 IPv6 地址行
//! 会自然撑开，与实测一致）。表头与数据使用同一布局。

use std::net::IpAddr;

use crate::{net, services::Services};

pub const HEADER: &str = "  Proto  Local Address          Foreign Address        State";
pub const HEADER_PID: &str = "  Proto  Local Address          Foreign Address        State           PID";

/// 一行连接。UDP 无 state（空字符串，保留列宽），无 PID 时行尾留白。
///
/// 实测列槽位：每字段宽 21 + 2 空格分隔（即每个字段占 23 列），
/// 内容超宽时左对齐自然溢出、分隔空格保留，长 IPv6 地址行不会粘连。
pub fn line(proto: &str, local: &str, foreign: &str, state: &str, pid: Option<u32>) -> String {
    let pid_s = match pid {
        Some(p) => p.to_string(),
        None => String::new(),
    };
    format!("  {proto:<3}    {local:<21}  {foreign:<21}  {state:<16}{pid_s}")
}

/// `IP:端口`（IPv6 加方括号），本地地址与 `-n` 模式的远端地址使用。
pub fn addr(ip: IpAddr, port: u16) -> String {
    match ip {
        IpAddr::V4(v) => format!("{v}:{port}"),
        IpAddr::V6(v) => format!("[{v}]:{port}"),
    }
}

/// 远端地址格式化。
///
/// 非 `-n` 模式复刻原版行为：
/// - 本地地址始终显示 IP 数字，只有远端解析主机名
/// - `0.0.0.0` / `::`（未指定）显示 `本机名:0`
/// - 其它远端反向解析主机名（失败回退 IP），端口查 services 显示服务名
pub fn foreign(e: &net::Entry, numeric: bool, svcs: &Services, host: &str) -> String {
    let (rip, rport) = match (e.remote_ip, e.remote_port) {
        (Some(ip), Some(p)) => (ip, p),
        _ => return "*:*".to_string(),
    };

    if numeric {
        return addr(rip, rport);
    }

    if rip.is_unspecified() {
        return format!("{host}:0");
    }

    let name = dns_lookup::lookup_addr(&rip)
        .ok()
        .map(|n| n.to_string())
        .unwrap_or_else(|| rip.to_string());
    let port_name = if e.is_tcp() {
        svcs.tcp_name(rport)
    } else {
        svcs.udp_name(rport)
    };
    format!("{name}:{port_name}")
}
