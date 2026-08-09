//! ICMP 协议层：基于 surge-ping 封装，输出可分类的回复结果。

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use surge_ping::{Client, Config, IcmpPacket, PingSequence, SurgeError, ICMP};
use tokio::net::lookup_host;

/// 一次 ping 请求的结果分类（对齐 Windows 的输出分支）。
#[derive(Debug)]
pub enum Outcome {
    /// 收到正常的回显应答
    Reply {
        source: IpAddr,
        data_len: usize,
        ttl: Option<u8>,
        rtt_ms: u128,
    },
    /// TTL 在传输中过期（ICMP Time Exceeded）
    TimeExceeded { source: IpAddr },
    /// 无法访问目标主机（ICMP Destination Unreachable）
    DestUnreachable { source: IpAddr },
    /// 请求超时
    Timeout,
    /// 其它 ICMP 错误
    Other { source: IpAddr },
}

/// 解析目标主机，优先返回一个符合 -4/-6 要求的地址。
pub async fn resolve(host: &str, want_v4: bool, want_v6: bool) -> Result<IpAddr, String> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        let ok = if want_v4 {
            ip.is_ipv4()
        } else if want_v6 {
            ip.is_ipv6()
        } else {
            true
        };
        return if ok {
            Ok(ip)
        } else {
            Err(format!("{host} 与 -4/-6 参数不匹配"))
        };
    }

    let addrs = lookup_host((host, 0))
        .await
        .map_err(|e| e.to_string())?;
    for a in addrs {
        if want_v4 && a.is_ipv4() {
            return Ok(a.ip());
        }
        if want_v6 && a.is_ipv6() {
            return Ok(a.ip());
        }
        if !want_v4 && !want_v6 {
            return Ok(a.ip());
        }
    }
    Err(format!("无法解析主机 {host}"))
}

/// 反向解析 IP 为主机名（-a 选项）。
pub async fn reverse_lookup(ip: IpAddr) -> Option<String> {
    tokio::task::spawn_blocking(move || dns_lookup::lookup_addr(&ip).ok())
        .await
        .ok()
        .flatten()
}

/// 构建 Client，应用 -i TTL 与 -S 源地址。
pub fn build_client(
    addr: IpAddr,
    ttl: Option<u32>,
    src_addr: Option<IpAddr>,
) -> Result<Client, String> {
    let mut builder = if addr.is_ipv6() {
        Config::builder().kind(ICMP::V6)
    } else {
        Config::builder().kind(ICMP::V4)
    };
    if let Some(ttl) = ttl {
        builder = builder.ttl(ttl);
    }
    if let Some(src) = src_addr {
        builder = builder.bind(SocketAddr::new(src, 0));
    }
    // 优先 RAW socket：Linux 的 DGRAM ICMP 收到的是裸 ICMP（无 IP 头），
    // 拿不到 TTL（会显示 TTL=0）。RAW 下能取到 IP 头里的 TTL。
    // Windows 上 RAW ICMP 免管理员（XP SP1 起对 ICMP 豁免），Linux 上
    // root 或 CAP_NET_RAW 可用；创建失败时 surge-ping 自动回退 DGRAM。
    builder = builder.sock_type_hint(socket2::Type::RAW);
    Client::new(&builder.build()).map_err(|e| e.to_string())
}

/// 给 socket 设置 -f（不分片）与 -v（TOS）选项。
/// 通过 surge-ping 暴露的原生 socket 句柄借用设置，不转移所有权。
pub fn apply_socket_options(
    client: &Client,
    no_fragment: bool,
    tos: Option<u8>,
) -> Result<(), String> {
    use std::mem::ManuallyDrop;

    let native = client.get_socket().get_native_sock();

    // ManuallyDrop 防止从原始句柄构造的 Socket 在作用域结束时关闭它，
    // 句柄所有权仍属于 surge-ping 内部的 AsyncSocket。
    // 此处的用法与 socket2 内部 SockRef 的实现一致。
    #[cfg(windows)]
    let sock = unsafe {
        use std::os::windows::io::FromRawSocket;
        ManuallyDrop::new(socket2::Socket::from_raw_socket(native))
    };
    #[cfg(unix)]
    let sock = unsafe {
        use std::os::fd::FromRawFd;
        ManuallyDrop::new(socket2::Socket::from_raw_fd(native))
    };

    if no_fragment {
        set_dont_fragment(&sock).map_err(|e| e.to_string())?;
    }
    if let Some(tos) = tos {
        sock.set_tos_v4(u32::from(tos)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Windows 上用 WinSock 设置 IP_DONTFRAGMENT（即 ping 的 -f 不分片标志）。
#[cfg(windows)]
fn set_dont_fragment(sock: &socket2::Socket) -> io::Result<()> {
    use std::os::windows::io::AsRawSocket;
    use windows_sys::Win32::Networking::WinSock::{setsockopt, IPPROTO_IP, IP_DONTFRAGMENT, SOCKET};

    // SAFETY: 句柄来自存活的 AsyncSocket，参数类型与长度均正确。
    unsafe {
        let enabled: u32 = 1;
        let ret = setsockopt(
            sock.as_raw_socket() as SOCKET,
            IPPROTO_IP,
            IP_DONTFRAGMENT,
            &enabled as *const u32 as *const u8,
            std::mem::size_of::<u32>() as i32,
        );
        if ret != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Unix 上 ICMP ping 默认即不分片，无需显式设置。
#[cfg(not(windows))]
fn set_dont_fragment(_sock: &socket2::Socket) -> io::Result<()> {
    Ok(())
}

/// 将 surge-ping 返回的包分类。
pub fn classify(packet: &IcmpPacket, rtt_ms: u128) -> Outcome {
    match packet {
        IcmpPacket::V4(p) => {
            let source = IpAddr::V4(p.get_source());
            match p.get_icmp_type().0 {
                0 => Outcome::Reply {
                    source,
                    data_len: p.get_size().saturating_sub(8),
                    ttl: p.get_ttl(),
                    rtt_ms,
                },
                11 => Outcome::TimeExceeded { source },
                3 => Outcome::DestUnreachable { source },
                _ => Outcome::Other { source },
            }
        }
        IcmpPacket::V6(p) => {
            let source = IpAddr::V6(p.get_source());
            match p.get_icmpv6_type().0 {
                129 => Outcome::Reply {
                    source,
                    data_len: p.get_size().saturating_sub(8),
                    ttl: None,
                    rtt_ms,
                },
                3 => Outcome::TimeExceeded { source },
                1 => Outcome::DestUnreachable { source },
                _ => Outcome::Other { source },
            }
        }
    }
}

/// 判断 ping() 的 Err 是否为超时。
pub fn is_timeout(err: &SurgeError) -> bool {
    matches!(err, SurgeError::Timeout { .. })
}

/// 构造一次带超时的 ping 请求（供主循环使用）。
pub async fn ping_once(
    pinger: &mut surge_ping::Pinger,
    seq: u16,
    payload: &[u8],
    timeout_ms: u64,
) -> Outcome {
    pinger.timeout(Duration::from_millis(timeout_ms));
    match pinger.ping(PingSequence(seq), payload).await {
        Ok((packet, dur)) => classify(&packet, dur.as_millis()),
        Err(e) if is_timeout(&e) => Outcome::Timeout,
        Err(_) => Outcome::Other {
            source: pinger.host,
        },
    }
}
