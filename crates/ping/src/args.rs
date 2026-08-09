//! Windows 风格命令行参数解析（对齐 ping.exe 的参数约定）。

use crate::i18n::L10n;
use fluent::FluentArgs;

#[derive(Debug, Clone)]
pub struct Args {
    /// -t 持续 ping 直到中断
    pub continuous: bool,
    /// -a 将地址解析为主机名
    pub resolve_name: bool,
    /// -n 发送次数（默认 4）
    pub count: u32,
    /// -l 发送缓冲区大小（默认 32）
    pub size: usize,
    /// -f 设置"不拆分"标志（仅 IPv4）
    pub no_fragment: bool,
    /// -i 生存时间 TTL
    pub ttl: Option<u32>,
    /// -v 服务类型 TOS（仅 IPv4）
    pub tos: Option<u8>,
    /// -r 记录计数跃点的路由（仅 IPv4）
    pub record_route: Option<u32>,
    /// -s 计数跃点的时间戳（仅 IPv4）
    pub timestamp: Option<u32>,
    /// -j 松散源路由主机列表（仅 IPv4）
    pub loose_route: Option<Vec<String>>,
    /// -k 严格源路由主机列表（仅 IPv4）
    pub strict_route: Option<Vec<String>>,
    /// -w 等待每次回复的超时时间（毫秒，默认 4000）
    pub timeout_ms: u64,
    /// -R 使用路由标头测试反向路由（仅 IPv6）
    pub reverse_route: bool,
    /// -S 要使用的源地址
    pub src_addr: Option<String>,
    /// -c 路由隔离舱标识符
    pub compartment: Option<u32>,
    /// -p Ping Hyper-V 网络虚拟化提供程序地址
    pub hyperv: bool,
    /// -4 强制 IPv4
    pub ipv4: bool,
    /// -6 强制 IPv6
    pub ipv6: bool,
    /// -? 显示帮助
    pub help: bool,
    /// 目标主机名或 IP
    pub target: Option<String>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            continuous: false,
            resolve_name: false,
            count: 4,
            size: 32,
            no_fragment: false,
            ttl: None,
            tos: None,
            record_route: None,
            timestamp: None,
            loose_route: None,
            strict_route: None,
            timeout_ms: 4000,
            reverse_route: false,
            src_addr: None,
            compartment: None,
            hyperv: false,
            ipv4: false,
            ipv6: false,
            help: false,
            target: None,
        }
    }
}

/// 需要值的选项
enum OptKind {
    Flag,
    Count,
    Size,
    Ttl,
    Tos,
    RecordRoute,
    Timestamp,
    LooseRoute,
    StrictRoute,
    Timeout,
    SrcAddr,
    Compartment,
}

/// 解析命令行参数。错误返回按当前语言翻译的提示。
pub fn parse(raw: &[String], i18n: &L10n) -> Result<Args, String> {
    let mut args = Args::default();
    let mut i = 0usize;

    while i < raw.len() {
        let arg = &raw[i];

        if let Some(opt) = arg.strip_prefix('-') {
            // 支持 "-n 4" 和 "-n4" 两种写法
            let (flag, rest) = if opt.len() > 1 { opt.split_at(1) } else { (opt, "") };
            let inline_val: Option<&str> = if rest.is_empty() { None } else { Some(rest) };

            let kind = match flag {
                "t" => OptKind::Flag,
                "a" => OptKind::Flag,
                "?" | "h" => OptKind::Flag,
                "f" => OptKind::Flag,
                "R" => OptKind::Flag,
                "p" => OptKind::Flag,
                "4" => OptKind::Flag,
                "6" => OptKind::Flag,
                "n" => OptKind::Count,
                "l" => OptKind::Size,
                "i" => OptKind::Ttl,
                "v" => OptKind::Tos,
                "r" => OptKind::RecordRoute,
                "s" => OptKind::Timestamp,
                "j" => OptKind::LooseRoute,
                "k" => OptKind::StrictRoute,
                "w" => OptKind::Timeout,
                "S" => OptKind::SrcAddr,
                "c" => OptKind::Compartment,
                _ => {
                    let mut a = FluentArgs::new();
                    a.set("flag", flag);
                    return Err(i18n.tr("error-invalid-option", Some(&a)));
                }
            };

            // 取值选项需要消费一个值
            let take_value = matches!(
                kind,
                OptKind::Count
                    | OptKind::Size
                    | OptKind::Ttl
                    | OptKind::Tos
                    | OptKind::RecordRoute
                    | OptKind::Timestamp
                    | OptKind::LooseRoute
                    | OptKind::StrictRoute
                    | OptKind::Timeout
                    | OptKind::SrcAddr
                    | OptKind::Compartment
            );

            let value: Option<String> = if take_value {
                let v = match inline_val {
                    Some(v) if !v.is_empty() => v.to_string(),
                    _ => {
                        i += 1;
                        if i >= raw.len() {
                            let mut a = FluentArgs::new();
                            a.set("flag", flag);
                            return Err(i18n.tr("error-option-needs-value", Some(&a)));
                        }
                        raw[i].clone()
                    }
                };
                Some(v)
            } else {
                if inline_val.is_some() {
                    let mut a = FluentArgs::new();
                    a.set("flag", flag);
                    return Err(i18n.tr("error-option-no-value", Some(&a)));
                }
                None
            };

            match kind {
                OptKind::Flag => match flag {
                    "t" => args.continuous = true,
                    "a" => args.resolve_name = true,
                    "?" | "h" => args.help = true,
                    "f" => args.no_fragment = true,
                    "R" => args.reverse_route = true,
                    "p" => args.hyperv = true,
                    "4" => args.ipv4 = true,
                    "6" => args.ipv6 = true,
                    _ => unreachable!(),
                },
                OptKind::Count => {
                    args.count = parse_range(&value, "n", 1, u64::from(u32::MAX), i18n)?;
                }
                OptKind::Size => {
                    args.size = parse_range(&value, "l", 0, 65500, i18n)? as usize;
                }
                OptKind::Ttl => {
                    args.ttl = Some(parse_range(&value, "i", 1, 255, i18n)?);
                }
                OptKind::Tos => {
                    args.tos = Some(parse_range(&value, "v", 0, 255, i18n)? as u8);
                }
                OptKind::RecordRoute => {
                    args.record_route = Some(parse_range(&value, "r", 1, 9, i18n)?);
                }
                OptKind::Timestamp => {
                    args.timestamp = Some(parse_range(&value, "s", 1, 4, i18n)?);
                }
                OptKind::LooseRoute => {
                    args.loose_route = Some(parse_host_list(&value, "j", i18n)?);
                }
                OptKind::StrictRoute => {
                    args.strict_route = Some(parse_host_list(&value, "k", i18n)?);
                }
                OptKind::Timeout => {
                    args.timeout_ms = value
                        .as_deref()
                        .and_then(|v| v.parse::<u64>().ok())
                        .ok_or_else(|| {
                            let mut a = FluentArgs::new();
                            a.set("flag", "w");
                            i18n.tr("error-value-numeric", Some(&a))
                        })?;
                }
                OptKind::SrcAddr => {
                    args.src_addr = value;
                }
                OptKind::Compartment => {
                    args.compartment =
                        Some(parse_range(&value, "c", 1, u64::from(u32::MAX), i18n)?);
                }
            }
        } else {
            if args.target.is_some() {
                return Err(i18n.tr("error-multiple-targets", None));
            }
            args.target = Some(arg.clone());
        }

        i += 1;
    }

    if args.ipv4 && args.ipv6 {
        return Err(i18n.tr("error-v4-v6-conflict", None));
    }

    Ok(args)
}

fn parse_range(
    value: &Option<String>,
    flag: &str,
    min: u64,
    max: u64,
    i18n: &L10n,
) -> Result<u32, String> {
    let v = value
        .as_deref()
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or_else(|| {
            let mut a = FluentArgs::new();
            a.set("flag", flag);
            i18n.tr("error-value-numeric", Some(&a))
        })?;
    if v < min || v > max {
        let mut a = FluentArgs::new();
        a.set("flag", flag);
        a.set("min", min);
        a.set("max", max);
        return Err(i18n.tr("error-value-range", Some(&a)));
    }
    Ok(v as u32)
}

fn parse_host_list(value: &Option<String>, flag: &str, i18n: &L10n) -> Result<Vec<String>, String> {
    match value {
        Some(v) => {
            let list: Vec<String> = v.split(' ').map(|s| s.to_string()).collect();
            if list.is_empty() || list.iter().any(|s| s.is_empty()) {
                let mut a = FluentArgs::new();
                a.set("flag", flag);
                return Err(i18n.tr("error-host-list", Some(&a)));
            }
            Ok(list)
        }
        None => {
            let mut a = FluentArgs::new();
            a.set("flag", flag);
            Err(i18n.tr("error-option-needs-value", Some(&a)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::L10n;

    fn l10n() -> L10n {
        L10n::for_lang("zh-CN")
    }

    fn p(raw: &[&str]) -> Args {
        let i18n = l10n();
        let v: Vec<String> = raw.iter().map(|s| s.to_string()).collect();
        parse(&v, &i18n).unwrap()
    }

    #[test]
    fn defaults() {
        let a = p(&["baidu.com"]);
        assert_eq!(a.count, 4);
        assert_eq!(a.size, 32);
        assert_eq!(a.timeout_ms, 4000);
        assert!(!a.continuous);
        assert_eq!(a.target.as_deref(), Some("baidu.com"));
    }

    #[test]
    fn short_and_inline() {
        assert_eq!(p(&["-n5", "127.0.0.1"]).count, 5);
        assert_eq!(p(&["-n", "5", "127.0.0.1"]).count, 5);
        assert!(p(&["-t", "127.0.0.1"]).continuous);
    }

    #[test]
    fn ranges() {
        let i18n = l10n();
        assert!(parse(&["-n", "0", "h"].iter().map(|s| s.to_string()).collect::<Vec<_>>(), &i18n).is_err());
        assert!(parse(&["-l", "70000", "h"].iter().map(|s| s.to_string()).collect::<Vec<_>>(), &i18n).is_err());
        assert!(parse(&["-i", "256", "h"].iter().map(|s| s.to_string()).collect::<Vec<_>>(), &i18n).is_err());
    }

    #[test]
    fn conflicts() {
        let i18n = l10n();
        assert!(parse(&["-4", "-6", "h"].iter().map(|s| s.to_string()).collect::<Vec<_>>(), &i18n).is_err());
    }

    #[test]
    fn en_messages() {
        let i18n = L10n::for_lang("en-US");
        let err = parse(&["-z", "h"].iter().map(|s| s.to_string()).collect::<Vec<_>>(), &i18n);
        assert_eq!(err.unwrap_err(), "Invalid option: -z");
    }
}
