//! tracert 探测核心。
//!
//! 网络栈复用现成 crate：
//! - `socket2`：DGRAM ICMP socket（Windows 免管理员），包装为 std UdpSocket
//! - `nex-packet`：ICMP 包构造与 IP/ICMP 回复解析（校验和、IPv4/IPv6 头解析）
//! - `dns-lookup`：地址解析与反解析
//!
//! 探测循环（逐跳 3 次探测、`*` 超时、到达即停）是 tracert 的应用逻辑，自行实现。

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

/// 每跳 3 次探测
pub const PROBES_PER_HOP: usize = 3;
/// echo 数据区大小（原版 tracert 用 32 字节）
const PAYLOAD_SIZE: usize = 32;

pub struct TraceConfig {
    pub ip: IpAddr,
    pub max_hops: u8,
    pub wait: Duration,
    pub src: Option<IpAddr>,
}

/// 一跳的结果。
#[derive(Debug, Default)]
pub struct Hop {
    pub rtts: [Option<u128>; PROBES_PER_HOP],
    pub ip: Option<IpAddr>,
}

/// 创建 ICMP socket（DGRAM 优先，RAW 兜底；Windows 上 DGRAM ICMP 免管理员）。
/// 全程使用 socket2，以便运行中修改 TTL（std UdpSocket 没有 set_unicast_hops_v6）。
fn create_socket(ip: &IpAddr, src: Option<IpAddr>) -> io::Result<Socket> {
    let (domain, proto) = if ip.is_ipv4() {
        (Domain::IPV4, Some(Protocol::ICMPV4))
    } else {
        (Domain::IPV6, Some(Protocol::ICMPV6))
    };
    // Windows / Linux 上 DGRAM ICMP socket 只能收到 Echo Reply，收不到
    // TimeExceeded（内核不把 ICMP 错误消息投递给 DGRAM socket），所以
    // tracert 必须用 RAW socket（Windows 对 ICMP 的 RAW 免管理员；
    // Linux 需要 root 或 CAP_NET_RAW）。
    // 其它平台（macOS/BSD）DGRAM 优先（免 root），RAW 兜底。
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

/// 构造 ICMPv4 Echo Request 包。
/// 校验和用 nex_packet::util::checksum（skipword=1 跳过 type+code 字）。
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

/// 构造 ICMPv6 Echo Request 包。
/// IPv6 的 ICMP 校验和包含伪头，DGRAM socket 下由内核计算，这里填 0。
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

/// 校验回复是否匹配我们的 (ident, seq)。
/// `embedded_off` 是 TTL 过期/目标不可达包中内嵌原始 echo 的 id 偏移：
/// IPv4 内嵌 IP 头 20 字节，IPv6 内嵌 IPv6 头 40 字节。
fn match_ident_seq(payload: &[u8], ident: u16, seq: u16, embedded_off: usize) -> bool {
    payload.len() >= embedded_off + 4
        && be16(payload, embedded_off) == ident
        && be16(payload, embedded_off + 2) == seq
}

/// 解析 IPv4 回复。兼容两种 socket 形态：
/// RAW socket 收到带 IP 头的包，Windows DGRAM 收到裸 ICMP。
fn parse_v4(buf: &[u8], recv_src: IpAddr, ident: u16, seq: u16) -> Option<(IpAddr, bool)> {
    // 带 IP 头（RAW）
    if let Some(ip) = Ipv4Packet::from_buf(buf) {
        let src = IpAddr::V4(ip.header.source);
        if let Some(icmp) = IcmpPacket::from_buf(ip.payload.as_ref()) {
            return classify_v4(icmp, src, ident, seq);
        }
    }
    // 裸 ICMP（Windows DGRAM）
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
            // icmp.payload = unused(4) + 内嵌 IPv4 头(20) + 内嵌 echo 头(4) + id/seq
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

/// 解析 IPv6 回复。
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
            // icmp.payload = unused(4) + 内嵌 IPv6 头(40) + 内嵌 echo 头(4) + id/seq
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
pub fn trace(cfg: &TraceConfig) -> Result<Vec<Hop>, String> {
    let socket = create_socket(&cfg.ip, cfg.src).map_err(|e| e.to_string())?;

    let ident = (std::process::id() & 0xFFFF) as u16;
    let dst: socket2::SockAddr = SocketAddr::new(cfg.ip, 0).into();
    let mut seq: u16 = 0;
    let mut hops = Vec::new();
    let mut buf = [std::mem::MaybeUninit::<u8>::uninit(); 2048];

    for hop in 1..=cfg.max_hops {
        // 每跳设置 TTL / Hop Limit
        let ttl = u32::from(hop);
        if cfg.ip.is_ipv4() {
            socket.set_ttl_v4(ttl).map_err(|e| e.to_string())?;
        } else {
            socket.set_unicast_hops_v6(ttl).map_err(|e| e.to_string())?;
        }

        let mut rtts: [Option<u128>; PROBES_PER_HOP] = [None, None, None];
        let mut hop_ip: Option<IpAddr> = None;
        let mut reached = false;

        for slot in rtts.iter_mut() {
            seq = seq.wrapping_add(1);
            let pkt = if cfg.ip.is_ipv4() {
                build_echo_v4(ident, seq)
            } else {
                build_echo_v6(ident, seq)
            };

            let start = Instant::now();
            socket.send_to(&pkt, &dst).map_err(|e| e.to_string())?;

            // 循环接收直到匹配或超时
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
                        // 不匹配的包继续等
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
                *slot = Some(start.elapsed().as_millis());
                if hop_ip.is_none() {
                    hop_ip = Some(ip);
                }
                if is_reached {
                    // 到达目标：本跳仍发满 3 次探测（原版 3 列都有值），
                    // 跳出外层跳循环。
                    reached = true;
                }
            }
        }

        hops.push(Hop { rtts, ip: hop_ip });

        // 收到 EchoReply 或 DestinationUnreachable 即到达终点
        if reached {
            break;
        }
    }

    Ok(hops)
}
