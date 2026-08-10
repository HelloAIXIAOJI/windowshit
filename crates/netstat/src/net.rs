//! 连接数据层。
//!
//! TCP / UDP 连接表跨平台走 `netstat2`（Windows GetExtendedTcpTable /
//! GetExtendedUdpTable，Linux netlink inet_diag，macOS libproc）。
//!
//! 已知局限：`netstat2` 的 `UdpSocketInfo` 不含远端地址，而原版 netstat
//! 会显示已连接 UDP 的对端（如 `UDP 0.0.0.0:59693 223.6.6.6:53`）。
//! 这里在 Linux 上解析 `/proc/net/udp[6]` 补齐；Windows/macOS 无公开
//! API 可取，显示 `*:*`。

use std::net::IpAddr;

use netstat2::{
    get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState,
};

use crate::args::Args;

/// `-q` 模式显示的绑定未监听端口状态。
pub const STATE_BOUND: &str = "BOUND";

pub struct Entry {
    /// 显示用协议名：原版 IPv6 连接同样显示 "TCP" / "UDP"，
    /// 地址族由 [`Entry::v6`] 区分（-p 过滤需要）。
    pub proto: &'static str,
    pub v6: bool,
    pub local_ip: IpAddr,
    pub local_port: u16,
    pub remote_ip: Option<IpAddr>,
    pub remote_port: Option<u16>,
    pub state: Option<&'static str>,
    pub pid: Option<u32>,
}

impl Entry {
    pub fn is_tcp(&self) -> bool {
        self.proto == "TCP"
    }
}

/// 采集全部连接（未过滤、未排序）。
pub fn query() -> Result<Vec<Entry>, String> {
    let infos = get_sockets_info(
        AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6,
        ProtocolFlags::TCP | ProtocolFlags::UDP,
    )
    .map_err(|e| e.to_string())?;

    let mut out = Vec::with_capacity(infos.len());
    for si in infos {
        match si.protocol_socket_info {
            ProtocolSocketInfo::Tcp(t) => out.push(Entry {
                proto: "TCP",
                v6: !t.local_addr.is_ipv4(),
                local_ip: t.local_addr,
                local_port: t.local_port,
                remote_ip: Some(t.remote_addr),
                remote_port: Some(t.remote_port),
                state: Some(state_name(t.state)),
                pid: si.associated_pids.first().copied(),
            }),
            ProtocolSocketInfo::Udp(u) => out.push(Entry {
                proto: "UDP",
                v6: !u.local_addr.is_ipv4(),
                local_ip: u.local_addr,
                local_port: u.local_port,
                remote_ip: None,
                remote_port: None,
                state: None,
                pid: si.associated_pids.first().copied(),
            }),
        }
    }
    Ok(out)
}

/// 平台补齐 UDP 远端（Linux /proc/net/udp[6]，其它平台无操作）。
#[cfg_attr(not(target_os = "linux"), allow(unused_variables))]
pub fn apply_udp_remotes(conns: &mut Vec<Entry>) {
    #[cfg(target_os = "linux")]
    {
        let remotes = udp_proc::remote_map();
        for e in conns.iter_mut() {
            // UDP 行没有 state
            if e.state.is_none() {
                if let Some((rip, rport)) = remotes.get(&(e.local_ip, e.local_port)) {
                    e.remote_ip = Some(*rip);
                    e.remote_port = Some(*rport);
                }
            }
        }
    }
}

/// 过滤 + 排序（还原原版输出顺序）。
///
/// 实测原版顺序：TCP v4 → TCP v6 → UDP v4 → UDP v6；组内按本地地址
/// 字节序 + 端口升序；`-q` 的 BOUND 行排在本组末尾。
pub fn filter_and_sort<'a>(conns: &'a [Entry], args: &Args) -> Vec<&'a Entry> {
    let mut v: Vec<&Entry> = conns.iter().filter(|e| visible(e, args)).collect();
    v.sort_by_key(|e| {
        (
            group(e),
            e.state == Some(STATE_BOUND),
            ip_key(&e.local_ip),
            e.local_port,
        )
    });
    v
}

fn visible(e: &Entry, args: &Args) -> bool {
    // -p 协议过滤（v4 / v6 分开）
    if let Some(p) = args.proto.as_deref() {
        let ok = match p {
            "TCP" => e.proto == "TCP" && !e.v6,
            "UDP" => e.proto == "UDP" && !e.v6,
            "TCPV6" => e.proto == "TCP" && e.v6,
            "UDPV6" => e.proto == "UDP" && e.v6,
            _ => false,
        };
        if !ok {
            return false;
        }
    }

    if args.show_all || args.show_q {
        true
    } else {
        // 默认：仅 TCP 非监听、非 BOUND（不含 UDP）
        e.is_tcp() && e.state != Some("LISTENING") && e.state != Some(STATE_BOUND)
    }
}

fn group(e: &Entry) -> u8 {
    match (e.proto, e.v6) {
        ("TCP", false) => 0,
        ("TCP", true) => 1,
        ("UDP", false) => 2,
        _ => 3,
    }
}

fn ip_key(ip: &IpAddr) -> Vec<u8> {
    match ip {
        IpAddr::V4(v) => v.octets().to_vec(),
        IpAddr::V6(v) => v.octets().to_vec(),
    }
}

/// netstat2 的 TcpState → Windows netstat 风格状态名。
fn state_name(s: TcpState) -> &'static str {
    use TcpState::*;
    match s {
        Established => "ESTABLISHED",
        SynSent => "SYN_SENT",
        SynReceived => "SYN_RECEIVED",
        FinWait1 => "FIN_WAIT_1",
        FinWait2 => "FIN_WAIT_2",
        CloseWait => "CLOSE_WAIT",
        Closing => "CLOSING",
        LastAck => "LAST_ACK",
        TimeWait => "TIME_WAIT",
        DeleteTcb => "DELETE_TCB",
        Listen => "LISTENING",
        Closed => STATE_BOUND,
        Unknown => "UNKNOWN",
    }
}

/// Linux：解析 /proc/net/udp 与 /proc/net/udp6，建立
/// (local_ip, local_port) → (remote_ip, remote_port) 映射。
///
/// 文件各行格式（除去头行）：
///   sl  local_address rem_address st ...（地址为十六进制，IP 小端，端口大端）
#[cfg(target_os = "linux")]
mod udp_proc {
    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    pub fn remote_map() -> HashMap<(IpAddr, u16), (IpAddr, u16)> {
        let mut m = HashMap::new();
        parse_file("/proc/net/udp", false, &mut m);
        parse_file("/proc/net/udp6", true, &mut m);
        m
    }

    fn parse_file(
        path: &str,
        v6: bool,
        m: &mut HashMap<(IpAddr, u16), (IpAddr, u16)>,
    ) {
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        for line in text.lines().skip(1) {
            let mut cols = line.split_whitespace();
            let _sl = cols.next();
            let local = match cols.next() {
                Some(s) => s,
                None => continue,
            };
            let rem = match cols.next() {
                Some(s) => s,
                None => continue,
            };
            let Some((lip, lport)) = parse_addr(local, v6) else {
                continue;
            };
            let Some((rip, rport)) = parse_addr(rem, v6) else {
                continue;
            };
            // 只记录已连接 UDP（对端非未指定地址）
            if !rip.is_unspecified() {
                m.insert((lip, lport), (rip, rport));
            }
        }
    }

    fn parse_addr(s: &str, v6: bool) -> Option<(IpAddr, u16)> {
        let (addr_hex, port_hex) = s.split_once(':')?;
        let port = u16::from_str_radix(port_hex, 16).ok()?;
        if v6 {
            // 32 hex 字符 = 4 组小端 u32
            if addr_hex.len() != 32 {
                return None;
            }
            let mut b = [0u8; 16];
            for g in 0..4 {
                let v = u32::from_str_radix(&addr_hex[g * 8..g * 8 + 8], 16).ok()?;
                b[g * 4..g * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
            Some((IpAddr::V6(Ipv6Addr::from(b)), port))
        } else {
            // 8 hex 字符 = 小端 u32
            if addr_hex.len() != 8 {
                return None;
            }
            let v = u32::from_str_radix(addr_hex, 16).ok()?;
            Some((IpAddr::V4(Ipv4Addr::from(v.to_le_bytes())), port))
        }
    }
}
