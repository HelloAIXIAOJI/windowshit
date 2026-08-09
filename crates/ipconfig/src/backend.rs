//! 跨平台网络适配器数据层。
//!
//! Windows 用 `ipconfig` crate（封装 GetAdaptersAddresses）；
//! Linux/macOS 用 `netdev`（0.46，自带地址/前缀/scope/网关/DNS，无需其它 crate）。
//! 各平台只做数据采集与适配器标题归类，不重复实现网络栈逻辑。

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AdapterKind {
    Ethernet,
    Loopback,
    Tunnel,
    Wireless,
    Other,
}

#[derive(Debug, Clone)]
pub struct AdapterData {
    pub friendly_name: String,
    pub description: String,
    pub mac: Option<Vec<u8>>,
    /// (IP, prefix_len)
    pub ipv4: Vec<(Ipv4Addr, u8)>,
    pub ipv6: Vec<(Ipv6Addr, u8)>,
    /// IPv6 接口索引（link-local 地址的 %scope 显示）
    pub ipv6_scope: Option<u32>,
    pub gateways: Vec<IpAddr>,
    pub dns: Vec<IpAddr>,
    pub is_up: bool,
    pub kind: AdapterKind,
}

pub fn get_adapters() -> Result<Vec<AdapterData>, String> {
    #[cfg(windows)]
    {
        windows_adapters()
    }
    #[cfg(not(windows))]
    {
        unix_adapters()
    }
}

#[cfg(windows)]
fn windows_adapters() -> Result<Vec<AdapterData>, String> {
    let adapters = ipconfig::get_adapters().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for a in adapters {
        let kind = match a.if_type() {
            ipconfig::IfType::EthernetCsmacd => AdapterKind::Ethernet,
            ipconfig::IfType::SoftwareLoopback => AdapterKind::Loopback,
            ipconfig::IfType::Tunnel => AdapterKind::Tunnel,
            ipconfig::IfType::Ieee80211 => AdapterKind::Wireless,
            _ => AdapterKind::Other,
        };
        // 原版 ipconfig 不显示回环接口
        if kind == AdapterKind::Loopback {
            continue;
        }
        let is_up = a.oper_status() == ipconfig::OperStatus::IfOperStatusUp;

        // 收集 IPv4 / IPv6 及前缀长度。
        // ipconfig crate 0.3 将 unicast 地址与 prefix 分开返回，这里按地址族配对。
        let mut ipv4: Vec<(Ipv4Addr, u8)> = Vec::new();
        let mut ipv6: Vec<(Ipv6Addr, u8)> = Vec::new();
        let mut v4_prefixes: Vec<u8> = a
            .prefixes()
            .iter()
            .filter(|(ip, _)| ip.is_ipv4())
            .map(|(_, len)| (*len).min(32) as u8)
            .collect();
        let mut v6_prefixes: Vec<u8> = a
            .prefixes()
            .iter()
            .filter(|(ip, _)| ip.is_ipv6())
            .map(|(_, len)| (*len).min(128) as u8)
            .collect();
        for ip in a.ip_addresses() {
            match ip {
                IpAddr::V4(v4) => {
                    let plen = v4_prefixes.first().copied().unwrap_or(24);
                    ipv4.push((*v4, plen));
                    if !v4_prefixes.is_empty() {
                        v4_prefixes.remove(0);
                    }
                }
                IpAddr::V6(v6) => {
                    let plen = v6_prefixes.first().copied().unwrap_or(64);
                    ipv6.push((*v6, plen));
                    if !v6_prefixes.is_empty() {
                        v6_prefixes.remove(0);
                    }
                }
            }
        }

        out.push(AdapterData {
            friendly_name: a.friendly_name().to_string(),
            description: a.description().to_string(),
            mac: a.physical_address().map(|v| v.to_vec()),
            ipv4,
            ipv6,
            ipv6_scope: Some(a.ipv6_if_index()),
            gateways: a.gateways().to_vec(),
            dns: a.dns_servers().to_vec(),
            is_up,
            kind,
        });
    }
    Ok(out)
}

#[cfg(not(windows))]
fn unix_adapters() -> Result<Vec<AdapterData>, String> {
    use netdev::net::mac::MacAddr;

    let interfaces = netdev::get_interfaces();
    let mut out = Vec::new();

    for iface in interfaces {
        let kind = classify_kind(&iface.name);
        // 原版 ipconfig 不显示回环接口
        if kind == AdapterKind::Loopback {
            continue;
        }

        let mut ipv4: Vec<(Ipv4Addr, u8)> = Vec::new();
        for net in &iface.ipv4 {
            ipv4.push((net.addr(), net.prefix_len()));
        }
        let mut ipv6: Vec<(Ipv6Addr, u8)> = Vec::new();
        for net in &iface.ipv6 {
            ipv6.push((net.addr(), net.prefix_len()));
        }

        // MAC（netdev 的 MacAddr 是冒号格式字符串，转成字节数组，
        // 与 Windows 分支共用横线大写格式输出）
        let mac = iface.mac_addr.as_ref().map(mac_bytes::<MacAddr>);

        // 网关：netdev 的 gateway 是 Option<NetworkDevice>（默认路由设备）
        let gateways: Vec<IpAddr> = match &iface.gateway {
            Some(g) => g
                .ipv4
                .iter()
                .map(|v4| IpAddr::V4(*v4))
                .chain(g.ipv6.iter().map(|v6| IpAddr::V6(*v6)))
                .collect(),
            None => Vec::new(),
        };

        out.push(AdapterData {
            friendly_name: iface.name.clone(),
            description: String::new(),
            mac,
            ipv4,
            ipv6,
            // link-local 的 %scope 就是接口索引
            ipv6_scope: Some(iface.index),
            gateways,
            dns: iface.dns_servers.clone(),
            is_up: iface.is_up(),
            kind,
        });
    }

    Ok(out)
}

/// 按接口名粗略分类（Linux/macOS 命名约定）。
#[cfg(not(windows))]
fn classify_kind(name: &str) -> AdapterKind {
    if name.starts_with("lo") {
        AdapterKind::Loopback
    } else if name.starts_with("wl") || name.starts_with("wlan") || name.starts_with("wlp") {
        AdapterKind::Wireless
    } else if name.contains("tun") || name.contains("tap") || name.contains("ppp") {
        AdapterKind::Tunnel
    } else if name.starts_with("eth")
        || name.starts_with("en")
        || name.starts_with("ens")
        || name.starts_with("enp")
    {
        // Linux: eth0/enp3s0 等；macOS: en0 等
        AdapterKind::Ethernet
    } else {
        // docker0、virbr0、veth* 等虚拟接口 → Unknown adapter（还原原版标题）
        AdapterKind::Other
    }
}

/// netdev 的 MacAddr（冒号格式）→ 字节数组。
#[cfg(not(windows))]
fn mac_bytes<M: std::fmt::Display>(m: &M) -> Vec<u8> {
    m.to_string()
        .split(':')
        .filter_map(|h| u8::from_str_radix(h, 16).ok())
        .collect()
}

/// 前缀长度 → 点分十进制子网掩码字符串（IPv4）。
pub fn prefix_to_mask4(bits: u8) -> String {
    let bits = bits.min(32);
    let mask: u32 = if bits == 0 {
        0
    } else {
        u32::MAX << (32 - bits)
    };
    Ipv4Addr::from(mask).to_string()
}
