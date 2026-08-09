//! 输出格式化：还原 Windows ipconfig 的点号对齐与字段结构。

use std::net::{Ipv4Addr, Ipv6Addr};

use windowshit_i18n::{FluentArgs, L10n};

use crate::backend::{prefix_to_mask4, AdapterData, AdapterKind};

/// 原版字段行的对齐宽度：3 空格缩进 + label + 点填充，冒号统一落在一列。
/// 实测原版冒号大致在第 40 列（1-based），这里取固定宽度保证对齐。
const FIELD_WIDTH: usize = 37;

/// 字段行：label + 点填充 + ": " + 值。
/// 值为空时冒号后不加空格（原版行为，避免尾随空格）。
pub fn field(label: &str, value: &str) -> String {
    let mut s = format!("   {label}");
    while s.len() < FIELD_WIDTH {
        s.push('.');
        if s.len() < FIELD_WIDTH {
            s.push(' ');
        }
    }
    s.truncate(FIELD_WIDTH);
    if value.is_empty() {
        format!("{s}:")
    } else {
        format!("{s}: {value}")
    }
}

/// 适配器标题行，如 "Ethernet adapter Ethernet:"。
pub fn adapter_title(i18n: &L10n, a: &AdapterData) -> String {
    let key = match a.kind {
        AdapterKind::Ethernet => "adapter-ethernet",
        AdapterKind::Loopback => "adapter-loopback",
        AdapterKind::Tunnel => "adapter-tunnel",
        AdapterKind::Wireless => "adapter-wireless",
        AdapterKind::Other => "adapter-unknown",
    };
    let mut fa = FluentArgs::new();
    fa.set("name", a.friendly_name.as_str());
    i18n.tr(key, Some(&fa))
}

fn is_linklocal_v6(ip: &Ipv6Addr) -> bool {
    ip.segments()[0] == 0xfe80
}

fn is_autoconfig_v4(ip: &Ipv4Addr) -> bool {
    ip.octets()[0] == 169 && ip.octets()[1] == 254
}

/// MAC 字节 → "A4-BB-6D-XX-XX-XX"
fn format_mac(mac: &[u8]) -> String {
    mac.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join("-")
}

/// IPv6 地址字符串，link-local 附上 %scope（原版行为）。
fn fmt_v6(a: &AdapterData, ip: &Ipv6Addr) -> String {
    let mut s = ip.to_string();
    if is_linklocal_v6(ip) {
        if let Some(scope) = a.ipv6_scope {
            s.push_str(&format!("%{scope}"));
        }
    }
    s
}

/// 无参数模式：每个适配器显示 IP/掩码/网关。
pub fn render_basic(i18n: &L10n, adapters: &[AdapterData]) -> String {
    let mut out = String::new();
    out.push_str(&i18n.tr("header", None));
    out.push_str("\n\n\n\n");

    for a in adapters {
        out.push_str(&adapter_title(i18n, a));
        out.push_str("\n\n");

        if !a.is_up {
            out.push_str(&field(
                &i18n.tr("field-media-state", None),
                &i18n.tr("value-media-disconnected", None),
            ));
            out.push('\n');
            out.push_str(&field(&i18n.tr("field-dns-suffix", None), ""));
            out.push_str("\n\n");
            continue;
        }

        out.push_str(&field(&i18n.tr("field-dns-suffix", None), ""));
        out.push('\n');

        // IPv6：先 global 再 link-local（原版顺序）
        for (ip, _) in a.ipv6.iter().filter(|(ip, _)| !is_linklocal_v6(ip)) {
            out.push_str(&field(&i18n.tr("field-ipv6", None), &fmt_v6(a, ip)));
            out.push('\n');
        }
        for (ip, _) in a.ipv6.iter().filter(|(ip, _)| is_linklocal_v6(ip)) {
            out.push_str(&field(&i18n.tr("field-linklocal-v6", None), &fmt_v6(a, ip)));
            out.push('\n');
        }

        // IPv4（含自动配置 169.254.x）
        for (ip, _) in a.ipv4.iter() {
            let label_key = if is_autoconfig_v4(ip) {
                "field-autoconfig-v4"
            } else {
                "field-ipv4"
            };
            out.push_str(&field(&i18n.tr(label_key, None), &ip.to_string()));
            out.push('\n');
        }
        for (ip, plen) in a.ipv4.iter() {
            let mask = if is_autoconfig_v4(ip) {
                "255.255.0.0".to_string()
            } else {
                prefix_to_mask4(*plen)
            };
            out.push_str(&field(&i18n.tr("field-subnet-mask", None), &mask));
            out.push('\n');
        }

        // 默认网关
        if a.gateways.is_empty() {
            out.push_str(&field(&i18n.tr("field-default-gateway", None), ""));
        } else {
            for (i, gw) in a.gateways.iter().enumerate() {
                if i == 0 {
                    out.push_str(&field(&i18n.tr("field-default-gateway", None), &gw.to_string()));
                } else {
                    out.push_str(&field("", &gw.to_string()));
                }
                out.push('\n');
            }
        }
        out.push('\n');
    }

    out
}

/// /all 模式：完整信息。
pub fn render_all(i18n: &L10n, adapters: &[AdapterData]) -> String {
    let mut out = String::new();
    out.push_str(&i18n.tr("header", None));
    out.push_str("\n\n\n\n");

    for a in adapters {
        out.push_str(&adapter_title(i18n, a));
        out.push_str("\n\n");

        out.push_str(&field(&i18n.tr("field-dns-suffix", None), ""));
        out.push('\n');

        if !a.description.is_empty() {
            out.push_str(&field(&i18n.tr("field-description", None), &a.description));
            out.push('\n');
        }
        if let Some(mac) = &a.mac {
            out.push_str(&field(
                &i18n.tr("field-physical-address", None),
                &format_mac(mac),
            ));
            out.push('\n');
        }

        if !a.is_up {
            out.push_str(&field(
                &i18n.tr("field-media-state", None),
                &i18n.tr("value-media-disconnected", None),
            ));
            out.push('\n');
            continue;
        }

        // IPv6 地址（完整）
        let v6_all: Vec<&(Ipv6Addr, u8)> = a.ipv6.iter().collect();
        let v6_linklocal = v6_all.iter().filter(|(ip, _)| is_linklocal_v6(ip)).copied().collect::<Vec<_>>();
        let v6_global = v6_all.iter().filter(|(ip, _)| !is_linklocal_v6(ip)).copied().collect::<Vec<_>>();
        for (ip, _) in v6_global {
            out.push_str(&field(&i18n.tr("field-ipv6", None), &fmt_v6(a, ip)));
            out.push('\n');
        }
        for (i, (ip, _)) in v6_linklocal.iter().enumerate() {
            let mut value = fmt_v6(a, ip);
            if i == 0 {
                value.push_str(&i18n.tr("preferred", None));
            }
            out.push_str(&field(&i18n.tr("field-linklocal-v6", None), &value));
            out.push('\n');
        }

        // IPv4
        for (i, (ip, plen)) in a.ipv4.iter().enumerate() {
            let label_key = if is_autoconfig_v4(ip) {
                "field-autoconfig-v4"
            } else {
                "field-ipv4"
            };
            let mut value = ip.to_string();
            if i == 0 {
                value.push_str(&i18n.tr("preferred", None));
            }
            out.push_str(&field(&i18n.tr(label_key, None), &value));
            out.push('\n');

            let mask = if is_autoconfig_v4(ip) {
                "255.255.0.0".to_string()
            } else {
                prefix_to_mask4(*plen)
            };
            out.push_str(&field(&i18n.tr("field-subnet-mask", None), &mask));
            out.push('\n');
        }

        // 默认网关
        if a.gateways.is_empty() {
            out.push_str(&field(&i18n.tr("field-default-gateway", None), ""));
            out.push('\n');
        } else {
            for (i, gw) in a.gateways.iter().enumerate() {
                if i == 0 {
                    out.push_str(&field(&i18n.tr("field-default-gateway", None), &gw.to_string()));
                } else {
                    out.push_str(&field("", &gw.to_string()));
                }
                out.push('\n');
            }
        }

        // DNS 服务器
        if a.dns.is_empty() {
            out.push_str(&field(&i18n.tr("field-dns-servers", None), ""));
            out.push('\n');
        } else {
            for (i, dns) in a.dns.iter().enumerate() {
                if i == 0 {
                    out.push_str(&field(&i18n.tr("field-dns-servers", None), &dns.to_string()));
                } else {
                    out.push_str(&field("", &dns.to_string()));
                }
                out.push('\n');
            }
        }
        out.push('\n');
    }

    out
}


