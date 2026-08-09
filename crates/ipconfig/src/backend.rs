//! 跨平台网络适配器数据层。
//!
//! Windows 用 `ipconfig` crate（封装 GetAdaptersAddresses）；
//! Linux/macOS 用 `netdev` + `network-interface` + `resolv-conf`。
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

/// DNS 服务器列表（unix 上全局读 resolv.conf，Windows 上按适配器返回）。
#[cfg(not(windows))]
fn global_dns_servers() -> Vec<IpAddr> {
    #[cfg(not(windows))]
    {
        let content = match std::fs::read_to_string("/etc/resolv.conf") {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        match resolv_conf::Config::parse(&content) {
            Ok(cfg) => cfg.nameservers.into_iter().map(IpAddr::V4).collect(),
            Err(_) => Vec::new(),
        }
    }
    #[cfg(windows)]
    {
        Vec::new() // Windows 端按适配器从 AdapterData.dns 取
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
                    // 去掉 IPv6 地址的 scope id（%数字），原版 ipconfig 不显示
                    let mut v6 = *v6;
                    if v6.segments()[0] == 0xfe80 {
                        v6 = v6.to_owned();
                    }
                    let plen = v6_prefixes.first().copied().unwrap_or(64);
                    ipv6.push((v6, plen));
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
    use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};

    let interfaces = NetworkInterface::get_all().map_err(|e| e.to_string())?;
    let netdev_interfaces = netdev::get_interfaces();
    let mut out = Vec::new();

    for iface in interfaces {
        let mut ipv4: Vec<(Ipv4Addr, u8)> = Vec::new();
        let mut ipv6: Vec<(Ipv6Addr, u8)> = Vec::new();
        for addr in iface.addrs {
            match addr {
                Addr::V4(v4) => {
                    let plen = v4.netmask.map(prefix_from_mask4).unwrap_or(24);
                    ipv4.push((v4.ip, plen));
                }
                Addr::V6(v6) => {
                    let plen = v6.netmask.map(prefix_from_mask6).unwrap_or(64);
                    ipv6.push((v6.ip, plen));
                }
            }
        }

        let kind = if iface.name.starts_with("lo") {
            AdapterKind::Loopback
        } else if iface.name.starts_with("wl") || iface.name.starts_with("wlan") || iface.name.starts_with("wlp") {
            AdapterKind::Wireless
        } else if iface.name.contains("tun") || iface.name.contains("ppp") {
            AdapterKind::Tunnel
        } else {
            AdapterKind::Ethernet
        };
        // 原版 ipconfig 不显示回环接口
        if kind == AdapterKind::Loopback {
            continue;
        }

        let mac = iface.mac_addr().map(|m| m.to_vec());

        // 网关：从 netdev 按接口名匹配
        let mut gateways: Vec<IpAddr> = Vec::new();
        let mut is_up = false;
        for nd in netdev_interfaces.iter() {
            if nd.name == iface.name {
                is_up = nd.is_up();
                for g in &nd.gateways {
                    gateways.push(IpAddr::V4(g.ip_addr));
                }
                break;
            }
        }

        out.push(AdapterData {
            friendly_name: iface.name.clone(),
            description: String::new(),
            mac,
            ipv4,
            ipv6,
            ipv6_scope: iface.index,
            gateways,
            dns: Vec::new(),
            is_up,
            kind,
        });
    }

    // 为所有适配器统一挂上全局 DNS
    let dns = global_dns_servers();
    for a in out.iter_mut() {
        a.dns = dns.clone();
    }

    Ok(out)
}

#[cfg(not(windows))]
fn prefix_from_mask4(mask: Ipv4Addr) -> u8 {
    let bits = u32::from(mask);
    (32 - bits.leading_zeros()) as u8
}

#[cfg(not(windows))]
fn prefix_from_mask6(mask: Ipv6Addr) -> u8 {
    let bits = u128::from(mask);
    (128 - bits.leading_zeros()) as u8
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
