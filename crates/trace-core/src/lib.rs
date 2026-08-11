//! Windowshit 公共 traceroute 探测层。
//!
//! 被 `tracert`（逐跳 3 探测）和 `pathping`（路由阶段）共用。
//!
//! 网络栈复用现成 crate：
//! - `socket2`：RAW/DGRAM ICMP socket（Windows 免管理员，Linux 需 root/CAP_NET_RAW），
//!   逐跳修改 TTL
//! - `nex-packet`：ICMP 包构造与 IP/ICMP 回复解析（校验和、IPv4/IPv6 头解析）
//!
//! 关键经验（已在 Windows / Linux 实测校准）：
//! - Windows 与 Linux 的 DGRAM ICMP socket 都收不到 TimeExceeded（ICMP 错误消息
//!   不投递给 DGRAM socket），因此探测必须用 RAW socket
//! - TTL 过期/目标不可达包内嵌原始 echo 的 id/seq 偏移：IPv4 = 28
//!   （ICMP unused4 + 内嵌 IP 头 20 + 内嵌 echo 头 4），IPv6 = 48

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use nex_packet::icmp::{IcmpPacket, IcmpType, MutableIcmpPacket};
use nex_packet::icmpv6::{Icmpv6Packet, Icmpv6Type, MutableIcmpv6Packet};
use nex_packet::ipv4::Ipv4Packet;
use nex_packet::ipv6::Ipv6Packet;
use nex_packet::packet::{MutablePacket, Packet};
use nex_packet::util::checksum;
use socket2::{Domain, Protocol, Socket, Type};

/// echo 数据区大小（tracert / pathping 原版均为 32 字节）
const PAYLOAD_SIZE: usize = 32;

pub struct TraceConfig {
    pub ip: IpAddr,
    pub max_hops: u8,
    pub wait: Duration,
    /// 每跳探测次数（tracert 用 3，pathping 路由阶段用 1）
    pub probes_per_hop: usize,
    /// 源地址（绑定 socket，对应 -S / -i）
    pub src: Option<IpAddr>,
}

/// 一跳的结果。
#[derive(Debug, Default)]
pub struct Hop {
    /// 每次探测的 RTT（毫秒），未收到回复为 None
    pub rtts: Vec<Option<u128>>,
    /// 该跳响应的路由器地址
    pub ip: Option<IpAddr>,
}

/// 创建 ICMP socket（RAW 优先）。
///
/// Windows / Linux 的 DGRAM ICMP socket 收不到 TimeExceeded，探测必须用 RAW。
/// Windows 对 ICMP 的 RAW 免管理员（XP SP1 起豁免）；Linux 需 root 或
/// CAP_NET_RAW。其它平台（macOS/BSD）DGRAM 优先（免 root），RAW 兜底。
fn create_socket(ip: &IpAddr, src: Option<IpAddr>) -> io::Result<Socket> {
    let (domain, proto) = if ip.is_ipv4() {
        (Domain::IPV4, Some(Protocol::ICMPV4))
    } else {
        (Domain::IPV6, Some(Protocol::ICMPV6))
    };
    #[cfg(any(windows, target_os = "linux"))]
    let sock = Socket::new(domain, Type::RAW, proto)
        .or_else(|_| Socket::new(domain, Type::DGRAM, proto))?;
    #[cfg(not(any(windows, target_os = "linux")))]
    let sock = Socket::new(domain, Type::DGRAM, proto)
        .or_else(|_| Socket::new(domain, Type::RAW, proto))?;
    if let Some(src) = src {
        sock.bind(&SocketAddr::new(src, 0).into())?;
    }
    Ok(sock)
}

/// 构造 ICMPv4 Echo Request 包（校验和用 nex_packet::util::checksum）。
fn build_echo_v4(ident: u16, seq: u16) -> Vec<u8> {
    let mut buf = vec![0u8; 8 + PAYLOAD_SIZE];
    let mut icmp = MutableIcmpPacket::new_unchecked(&mut buf);
    icmp.set_type(IcmpType::EchoRequest);
    let payload = icmp.payload_mut();
    payload[0..2].copy_from_slice(&ident.to_be_bytes());
    payload[2..4].copy_from_slice(&seq.to_be_bytes());
    let sum = checksum(icmp.packet(), 1);
    icmp.set_checksum(sum);
    buf
}

/// 构造 ICMPv6 Echo Request 包（校验和含伪头，DGRAM/RAW 由内核计算，填 0）。
fn build_echo_v6(ident: u16, seq: u16) -> Vec<u8> {
    let mut buf = vec![0u8; 8 + PAYLOAD_SIZE];
    let mut icmp = MutableIcmpv6Packet::new_unchecked(&mut buf);
    icmp.set_type(Icmpv6Type::EchoRequest);
    let payload = icmp.payload_mut();
    payload[0..2].copy_from_slice(&ident.to_be_bytes());
    payload[2..4].copy_from_slice(&seq.to_be_bytes());
    buf
}

fn be16(buf: &[u8], off: usize) -> u16 {
    u16::from_be_bytes([buf[off], buf[off + 1]])
}

/// 校验回复是否匹配 (ident, seq)。`embedded_off` 是 TTL 过期/目标不可达包中
/// 内嵌原始 echo 的 id 偏移：IPv4 = 28，IPv6 = 48。
fn match_ident_seq(payload: &[u8], ident: u16, seq: u16, embedded_off: usize) -> bool {
    payload.len() >= embedded_off + 4
        && be16(payload, embedded_off) == ident
        && be16(payload, embedded_off + 2) == seq
}

fn parse_v4(buf: &[u8], recv_src: IpAddr, ident: u16, seq: u16) -> Option<(IpAddr, bool)> {
    // RAW socket：带 IP 头
    if let Some(ip) = Ipv4Packet::from_buf(buf) {
        let src = IpAddr::V4(ip.header.source);
        if let Some(icmp) = IcmpPacket::from_buf(ip.payload.as_ref()) {
            return classify_v4(icmp, src, ident, seq);
        }
    }
    // DGRAM socket：裸 ICMP
    if let Some(icmp) = IcmpPacket::from_buf(buf) {
        return classify_v4(icmp, recv_src, ident, seq);
    }
    None
}

fn classify_v4(icmp: IcmpPacket, src: IpAddr, ident: u16, seq: u16) -> Option<(IpAddr, bool)> {
    match icmp.header.icmp_type {
        IcmpType::EchoReply => {
            if match_ident_seq(icmp.payload.as_ref(), ident, seq, 0) {
                Some((src, true))
            } else {
                None
            }
        }
        IcmpType::TimeExceeded => {
            if match_ident_seq(icmp.payload.as_ref(), ident, seq, 28) {
                Some((src, false))
            } else {
                None
            }
        }
        IcmpType::DestinationUnreachable => {
            if match_ident_seq(icmp.payload.as_ref(), ident, seq, 28) {
                Some((src, true))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn parse_v6(buf: &[u8], recv_src: IpAddr, ident: u16, seq: u16) -> Option<(IpAddr, bool)> {
    if let Some(ip) = Ipv6Packet::from_buf(buf) {
        let src = IpAddr::V6(ip.header.source);
        if let Some(icmp) = Icmpv6Packet::from_buf(ip.payload.as_ref()) {
            return classify_v6(icmp, src, ident, seq);
        }
    }
    if let Some(icmp) = Icmpv6Packet::from_buf(buf) {
        return classify_v6(icmp, recv_src, ident, seq);
    }
    None
}

fn classify_v6(icmp: Icmpv6Packet, src: IpAddr, ident: u16, seq: u16) -> Option<(IpAddr, bool)> {
    match icmp.header.icmpv6_type {
        Icmpv6Type::EchoReply => {
            if match_ident_seq(icmp.payload.as_ref(), ident, seq, 0) {
                Some((src, true))
            } else {
                None
            }
        }
        Icmpv6Type::TimeExceeded => {
            if match_ident_seq(icmp.payload.as_ref(), ident, seq, 48) {
                Some((src, false))
            } else {
                None
            }
        }
        Icmpv6Type::DestinationUnreachable => {
            if match_ident_seq(icmp.payload.as_ref(), ident, seq, 48) {
                Some((src, true))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// 执行 traceroute，返回每跳结果。
///
/// 每跳发 `probes_per_hop` 次探测；收到 EchoReply 或 DestinationUnreachable
/// 视为到达目标，停止后续跳。
pub fn trace(cfg: &TraceConfig) -> Result<Vec<Hop>, String> {
    let socket = create_socket(&cfg.ip, cfg.src).map_err(|e| e.to_string())?;

    let ident = (std::process::id() & 0xFFFF) as u16;
    let dst: socket2::SockAddr = SocketAddr::new(cfg.ip, 0).into();
    let mut seq: u16 = 0;
    let mut hops = Vec::new();
    let mut buf = [std::mem::MaybeUninit::<u8>::uninit(); 2048];

    for hop in 1..=cfg.max_hops {
        let ttl = u32::from(hop);
        if cfg.ip.is_ipv4() {
            socket.set_ttl_v4(ttl).map_err(|e| e.to_string())?;
        } else {
            socket.set_unicast_hops_v6(ttl).map_err(|e| e.to_string())?;
        }

        let mut rtts: Vec<Option<u128>> = Vec::with_capacity(cfg.probes_per_hop);
        let mut hop_ip: Option<IpAddr> = None;
        let mut reached = false;

        for _ in 0..cfg.probes_per_hop {
            seq = seq.wrapping_add(1);
            let pkt = if cfg.ip.is_ipv4() {
                build_echo_v4(ident, seq)
            } else {
                build_echo_v6(ident, seq)
            };

            let start = Instant::now();
            socket.send_to(&pkt, &dst).map_err(|e| e.to_string())?;
            socket
                .set_read_timeout(Some(cfg.wait))
                .map_err(|e| e.to_string())?;

            let reply = loop {
                match socket.recv_from(&mut buf) {
                    Ok((len, addr)) => {
                        // SAFETY: recv_from 返回的 len 字节已被内核写入
                        let data =
                            unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, len) };
                        let src_ip = addr
                            .as_socket()
                            .map(|s| s.ip())
                            .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
                        let parsed = if cfg.ip.is_ipv4() {
                            parse_v4(data, src_ip, ident, seq)
                        } else {
                            parse_v6(data, src_ip, ident, seq)
                        };
                        if let Some(r) = parsed {
                            break Some(r);
                        }
                    }
                    Err(e)
                        if e.kind() == io::ErrorKind::WouldBlock
                            || e.kind() == io::ErrorKind::TimedOut =>
                    {
                        break None;
                    }
                    Err(_) => break None,
                }
            };

            if let Some((ip, is_reached)) = reply {
                rtts.push(Some(start.elapsed().as_millis()));
                if hop_ip.is_none() {
                    hop_ip = Some(ip);
                }
                if is_reached {
                    reached = true;
                }
            } else {
                rtts.push(None);
            }
        }

        hops.push(Hop { rtts, ip: hop_ip });

        if reached {
            break;
        }
    }

    Ok(hops)
}
