//! Windows 风格命令行参数解析（对齐 ping.exe 的参数约定）。

use std::net::IpAddr;
use windowshit_i18n::{FluentArgs, L10n};

/// 参数解析错误。`show_help` 表示原版在该错误后是否会打印完整帮助。
#[derive(Debug)]
pub struct ArgError {
    pub message: String,
    pub show_help: bool,
}

impl ArgError {
    fn new(i18n: &L10n, key: &str, args: Option<&FluentArgs>, show_help: bool) -> Self {
        ArgError {
            message: i18n.tr(key, args),
            show_help,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Args {
    /// -t 持续 ping 直到中断
    pub continuous: bool,
    /// -a 将地址解析为主机名
    pub resolve_name: bool,
    /// -n 发送次数（默认 4）
    pub count: u32,
    /// -l 发送缓冲区大小（默认 32；非数字解析为 0，同原版）
    pub size: usize,
    /// -f 设置"不拆分"标志（仅 IPv4）
    pub no_fragment: bool,
    /// -i 生存时间 TTL
    pub ttl: Option<u32>,
    /// -v 服务类型 TOS（仅 IPv4）
    pub tos: Option<u8>,
    /// -w 等待每次回复的超时时间（毫秒，默认 4000）
    pub timeout_ms: u64,
    /// -w 的值非数字时原版进入"发送失败"模式（transmit failed）
    pub transmit_fail: bool,
    /// -S 要使用的源地址
    pub src_addr: Option<String>,
    /// -4 强制 IPv4
    pub ipv4: bool,
    /// -6 强制 IPv6（与 -4 同时使用时报错，按出现顺序）
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
            timeout_ms: 4000,
            transmit_fail: false,
            src_addr: None,
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
    Timeout,
    SrcAddr,
}

/// 解析命令行参数。错误返回按当前语言翻译的提示。
pub fn parse(raw: &[String], i18n: &L10n) -> Result<Args, ArgError> {
    let mut args = Args::default();
    let mut i = 0usize;
    // 记录 -4/-6 出现的先后，冲突时报"后出现"的选项（同原版）
    let mut family_order: Vec<&str> = Vec::new();

    while i < raw.len() {
        let arg = &raw[i];

        // Windows 原版同时接受 "-n" 与 "/n" 两种选项前缀
        if let Some(opt) = arg.strip_prefix('-').or_else(|| arg.strip_prefix('/')) {
            if opt.is_empty() {
                let mut a = FluentArgs::new();
                a.set("arg", arg);
                return Err(ArgError::new(
                    i18n,
                    "error-bad-parameter",
                    Some(&a),
                    false,
                ));
            }
            // 支持 "-n 4" 和 "-n4" 两种写法
            let (flag, rest) = if opt.len() > 1 { opt.split_at(1) } else { (opt, "") };
            let inline_val: Option<&str> = if rest.is_empty() { None } else { Some(rest) };

            // 尚未真正实现的选项：明确报错，不假装支持
            const UNSUPPORTED: &[&str] = &["r", "s", "j", "k", "R", "c", "p"];
            if UNSUPPORTED.contains(&flag) {
                let mut a = FluentArgs::new();
                a.set("flag", flag);
                return Err(ArgError::new(i18n, "error-unsupported", Some(&a), false));
            }

            let kind = match flag {
                "t" => OptKind::Flag,
                "a" => OptKind::Flag,
                "?" | "h" => OptKind::Flag,
                "f" => OptKind::Flag,
                "4" => OptKind::Flag,
                "6" => OptKind::Flag,
                "n" => OptKind::Count,
                "l" => OptKind::Size,
                "i" => OptKind::Ttl,
                "v" => OptKind::Tos,
                "w" => OptKind::Timeout,
                "S" => OptKind::SrcAddr,
                _ => {
                    let mut a = FluentArgs::new();
                    a.set("flag", flag);
                    return Err(ArgError::new(
                        i18n,
                        "error-invalid-option",
                        Some(&a),
                        true, // 原版无效选项后打印完整帮助
                    ));
                }
            };

            // 取值选项需要消费一个值
            let take_value = matches!(
                kind,
                OptKind::Count
                    | OptKind::Size
                    | OptKind::Ttl
                    | OptKind::Tos
                    | OptKind::Timeout
                    | OptKind::SrcAddr
            );

            let value: Option<String> = if take_value {
                let v = match inline_val {
                    Some(v) if !v.is_empty() => v.to_string(),
                    _ => {
                        i += 1;
                        if i >= raw.len() {
                            let mut a = FluentArgs::new();
                            a.set("flag", flag);
                            return Err(ArgError::new(
                                i18n,
                                "error-option-needs-value",
                                Some(&a),
                                false,
                            ));
                        }
                        raw[i].clone()
                    }
                };
                Some(v)
            } else {
                if inline_val.is_some() {
                    let mut a = FluentArgs::new();
                    a.set("flag", flag);
                    return Err(ArgError::new(
                        i18n,
                        "error-option-no-value",
                        Some(&a),
                        false,
                    ));
                }
                None
            };

            match kind {
                OptKind::Flag => match flag {
                    "t" => args.continuous = true,
                    "a" => args.resolve_name = true,
                    "?" | "h" => args.help = true,
                    "f" => args.no_fragment = true,
                    "4" => {
                        args.ipv4 = true;
                        family_order.push("4");
                    }
                    "6" => {
                        args.ipv6 = true;
                        family_order.push("6");
                    }
                    _ => unreachable!(),
                },
                OptKind::Count => {
                    args.count = parse_u32_range(&value, "n", 1, u64::from(u32::MAX), i18n)?;
                }
                OptKind::Size => {
                    // 原版 -l 非数字解析为 0 字节，不报错
                    let v = value
                        .as_deref()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                    if v > 65500 {
                        let mut a = FluentArgs::new();
                        a.set("flag", "l");
                        a.set("min", 0u64);
                        a.set("max", 65500u64);
                        return Err(ArgError::new(
                            i18n,
                            "error-value-range",
                            Some(&a),
                            false,
                        ));
                    }
                    args.size = v as usize;
                }
                OptKind::Ttl => {
                    args.ttl = Some(parse_u32_range(&value, "i", 1, 255, i18n)?);
                }
                OptKind::Tos => {
                    args.tos = Some(parse_u32_range(&value, "v", 0, 255, i18n)? as u8);
                }
                OptKind::Timeout => {
                    match value.as_deref().and_then(|v| v.parse::<u64>().ok()) {
                        Some(ms) => args.timeout_ms = ms,
                        None => {
                            // 原版 -w 非数字不报错，进入"发送失败"模式
                            args.transmit_fail = true;
                        }
                    }
                }
                OptKind::SrcAddr => {
                    args.src_addr = value;
                }
            }
        } else {
            if args.target.is_some() {
                // 原版：目标之后再有参数 → Bad parameter {arg}。
                // （若第一个目标不是合法 IP，Windows 会先尝试解析它并报
                //  "could not find host"，此处交给后续解析自然处理。）
                if let Some(t) = &args.target {
                    if t.parse::<IpAddr>().is_ok() {
                        let mut a = FluentArgs::new();
                        a.set("arg", arg);
                        return Err(ArgError::new(
                            i18n,
                            "error-bad-parameter",
                            Some(&a),
                            false,
                        ));
                    }
                }
            } else {
                args.target = Some(arg.clone());
            }
        }

        i += 1;
    }

    // -4/-6 同时使用：原版报"后出现"的选项（实测 -4 -6 → -6；-6 -4 → -4）
    if args.ipv4 && args.ipv6 {
        let last = family_order.last().copied().unwrap_or("6");
        let ver = if last == "4" { "4" } else { "6" };
        let mut a = FluentArgs::new();
        a.set("flag", last);
        a.set("ver", ver);
        return Err(ArgError::new(
            i18n,
            "error-only-supported",
            Some(&a),
            false,
        ));
    }

    Ok(args)
}

fn parse_u32_range(
    value: &Option<String>,
    flag: &str,
    min: u64,
    max: u64,
    i18n: &L10n,
) -> Result<u32, ArgError> {
    let v = value.as_deref().and_then(|s| s.parse::<u64>().ok());
    let bad = v.map_or(true, |v| v < min || v > max);
    if bad {
        // 原版对非数字和越界统一报 range 错误
        let mut a = FluentArgs::new();
        a.set("flag", flag);
        a.set("min", min);
        a.set("max", max);
        return Err(ArgError::new(
            i18n,
            "error-value-range",
            Some(&a),
            false,
        ));
    }
    Ok(v.unwrap() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windowshit_i18n::L10n;

    fn l10n() -> L10n {
        let mut l = L10n::for_lang("en-US");
        l.add_ftl(include_str!("../locales/en-US.ftl"));
        l
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
    fn slash_prefix() {
        assert_eq!(p(&["/n", "3", "/a", "127.0.0.1"]).count, 3);
        assert!(p(&["/t", "127.0.0.1"]).continuous);
        assert_eq!(p(&["/?", "127.0.0.1"]).help, true);
    }

    #[test]
    fn non_numeric_values() {
        let i18n = l10n();
        // -n abc → range 错误（原版同此）
        let v = vec!["-n".to_string(), "abc".to_string(), "h".to_string()];
        let e = parse(&v, &i18n).unwrap_err();
        assert_eq!(e.message, "Bad value for option -n, valid range is from 1 to 4294967295.");
        assert!(!e.show_help);
        // -l abc → 0 字节（原版同此）
        assert_eq!(p(&["-l", "abc", "h"]).size, 0);
        // -w abc → transmit_fail
        assert!(p(&["-w", "abc", "h"]).transmit_fail);
    }

    #[test]
    fn range_errors() {
        let i18n = l10n();
        let v = vec!["-l".to_string(), "70000".to_string(), "h".to_string()];
        let e = parse(&v, &i18n).unwrap_err();
        assert!(e.message.contains("65500"));
        let v = vec!["-i".to_string(), "256".to_string(), "h".to_string()];
        assert!(parse(&v, &i18n).is_err());
    }

    #[test]
    fn v4v6_conflict_order() {
        let i18n = l10n();
        let v = vec!["-4".to_string(), "-6".to_string(), "h".to_string()];
        let e = parse(&v, &i18n).unwrap_err();
        assert_eq!(e.message, "The option -6 is only supported for IPv6.");
        let v = vec!["-6".to_string(), "-4".to_string(), "h".to_string()];
        let e = parse(&v, &i18n).unwrap_err();
        assert_eq!(e.message, "The option -4 is only supported for IPv4.");
    }

    #[test]
    fn bad_parameter() {
        let i18n = l10n();
        let v = vec!["127.0.0.1".to_string(), "8.8.8.8".to_string()];
        let e = parse(&v, &i18n).unwrap_err();
        assert_eq!(e.message, "Bad parameter 8.8.8.8.");
    }

    #[test]
    fn invalid_option_help() {
        let i18n = l10n();
        let v = vec!["-z".to_string(), "h".to_string()];
        let e = parse(&v, &i18n).unwrap_err();
        assert_eq!(e.message, "Bad option -z.");
        assert!(e.show_help);
    }

    #[test]
    fn unsupported_options() {
        let i18n = l10n();
        for flag in ["-r", "-s", "-j", "-k", "-R", "-c", "-p"] {
            let v = vec![flag.to_string(), "127.0.0.1".to_string()];
            let e = parse(&v, &i18n).unwrap_err();
            assert!(
                e.message.contains("not supported"),
                "flag {flag}: {}",
                e.message
            );
        }
    }
}
