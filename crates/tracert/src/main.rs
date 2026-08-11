mod trace;

use std::net::IpAddr;
use std::process::ExitCode;
use std::time::Duration;

use trace::TraceConfig;
use windowshit_i18n::{FluentArgs, L10n};

struct Args {
    d: bool,
    hops: u8,
    wait_ms: u64,
    src: Option<String>,
    v4: bool,
    v6: bool,
    target: Option<String>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            d: false,
            hops: 30,
            wait_ms: 4000,
            src: None,
            v4: false,
            v6: false,
            target: None,
        }
    }
}

fn parse_args(raw: &[String], i18n: &L10n) -> Result<Args, String> {
    let mut args = Args::default();
    let mut i = 0usize;

    while i < raw.len() {
        let arg = &raw[i];
        if let Some(opt) = arg.strip_prefix('-') {
            let (flag, rest) = if opt.len() > 1 {
                opt.split_at(1)
            } else {
                (opt, "")
            };
            let inline: Option<&str> = if rest.is_empty() { None } else { Some(rest) };

            // 未实现的选项：明确报错
            if matches!(flag, "j" | "R") {
                let mut a = FluentArgs::new();
                a.set("flag", flag);
                return Err(i18n.tr("error-unsupported", Some(&a)));
            }

            match flag {
                "d" => args.d = true,
                "h" => {
                    let v = match inline {
                        Some(v) if !v.is_empty() => v.to_string(),
                        _ => {
                            i += 1;
                            if i >= raw.len() {
                                return Err(i18n.tr("error-bad-option", args_flag(flag).as_ref()));
                            }
                            raw[i].clone()
                        }
                    };
                    let n: u64 = v.parse().map_err(|_| {
                        let mut a = FluentArgs::new();
                        a.set("flag", "h");
                        i18n.tr("error-bad-option", Some(&a))
                    })?;
                    if !(1..=255).contains(&n) {
                        let mut a = FluentArgs::new();
                        a.set("flag", "h");
                        return Err(i18n.tr("error-bad-option", Some(&a)));
                    }
                    args.hops = n as u8;
                }
                "w" => {
                    let v = match inline {
                        Some(v) if !v.is_empty() => v.to_string(),
                        _ => {
                            i += 1;
                            if i >= raw.len() {
                                return Err(i18n.tr("error-bad-option", args_flag("w").as_ref()));
                            }
                            raw[i].clone()
                        }
                    };
                    args.wait_ms = v.parse().unwrap_or(4000);
                }
                "S" => {
                    let v = match inline {
                        Some(v) if !v.is_empty() => v.to_string(),
                        _ => {
                            i += 1;
                            if i >= raw.len() {
                                return Err(i18n.tr("error-bad-option", args_flag("S").as_ref()));
                            }
                            raw[i].clone()
                        }
                    };
                    args.src = Some(v);
                }
                "4" => args.v4 = true,
                "6" => args.v6 = true,
                _ => {
                    let mut a = FluentArgs::new();
                    a.set("flag", flag);
                    return Err(i18n.tr("error-bad-option", Some(&a)));
                }
            }
        } else {
            if args.target.is_some() {
                let mut a = FluentArgs::new();
                a.set("flag", arg);
                return Err(i18n.tr("error-bad-option", Some(&a)));
            }
            args.target = Some(arg.clone());
        }
        i += 1;
    }
    Ok(args)
}

fn args_flag(flag: &str) -> Option<FluentArgs<'_>> {
    let mut a = FluentArgs::new();
    a.set("flag", flag);
    Some(a)
}

/// 解析目标地址。
fn resolve(host: &str, v4: bool, v6: bool) -> Option<IpAddr> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        let ok = if v4 {
            ip.is_ipv4()
        } else if v6 {
            ip.is_ipv6()
        } else {
            true
        };
        return if ok { Some(ip) } else { None };
    }
    let addrs = dns_lookup::lookup_host(host).ok()?;
    for a in addrs {
        if v4 && a.is_ipv4() {
            return Some(a);
        }
        if v6 && a.is_ipv6() {
            return Some(a);
        }
        if !v4 && !v6 {
            return Some(a);
        }
    }
    None
}

fn main() -> ExitCode {
    let mut i18n = L10n::detect();
    match i18n.lang() {
        "zh-CN" => i18n.add_ftl(include_str!("../locales/zh-CN.ftl")),
        _ => i18n.add_ftl(include_str!("../locales/en-US.ftl")),
    }
    i18n.set_help(
        include_str!("../locales/help.zh.txt"),
        include_str!("../locales/help.en.txt"),
    );

    L10n::setup_console_utf8();

    let raw: Vec<String> = std::env::args().skip(1).collect();

    // 无参数：原版显示帮助
    if raw.is_empty() {
        println!("{}", i18n.help());
        return ExitCode::from(1);
    }
    // -? / /? 帮助
    if raw.iter().any(|a| a == "-?" || a == "/?") {
        println!("{}", i18n.help());
        return ExitCode::SUCCESS;
    }

    let args = match parse_args(&raw, &i18n) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            eprintln!();
            println!("{}", i18n.help());
            return ExitCode::from(1);
        }
    };

    if args.v4 && args.v6 {
        eprintln!("{}", i18n.tr("error-bad-option", args_flag("4").as_ref()));
        return ExitCode::from(1);
    }

    let target = match &args.target {
        Some(t) => t.clone(),
        None => {
            println!("{}", i18n.help());
            return ExitCode::from(1);
        }
    };

    let ip = match resolve(&target, args.v4, args.v6) {
        Some(ip) => ip,
        None => {
            let mut a = FluentArgs::new();
            a.set("host", &target);
            eprintln!("{}", i18n.tr("unable-resolve", Some(&a)));
            return ExitCode::from(1);
        }
    };

    let src = match &args.src {
        Some(s) => match s.parse::<IpAddr>() {
            Ok(ip) => Some(ip),
            Err(_) => {
                let mut a = FluentArgs::new();
                a.set("addr", s.as_str());
                eprintln!("{}", i18n.tr("error-not-valid-address", Some(&a)));
                return ExitCode::from(1);
            }
        },
        None => None,
    };

    // Header
    let literal = target.parse::<IpAddr>().is_ok();
    let mut fa = FluentArgs::new();
    fa.set("target", &target);
    fa.set("ip", ip.to_string());
    fa.set("hops", u64::from(args.hops));

    if args.d && literal {
        // -d 且目标为 IP 字面：单行，无 [ip]
        let mut s = FluentArgs::new();
        s.set("target", &target);
        s.set("hops", u64::from(args.hops));
        println!("{}", i18n.tr("trace-header-single", Some(&s)));
    } else {
        if i18n.lang() == "zh-CN" {
            println!("{}", i18n.tr("trace-header-ip", Some(&fa)));
        } else {
            println!("{}", i18n.tr("trace-header-ip", Some(&fa)));
            println!("{}", i18n.tr("trace-header-hops", Some(&fa)));
        }
    }
    println!();

    // 执行探测
    let cfg = TraceConfig {
        ip,
        max_hops: args.hops,
        wait: Duration::from_millis(args.wait_ms),
        src,
    };
    let hops = match trace::trace(&cfg) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };

    // 打印每跳
    for (idx, hop) in hops.iter().enumerate() {
        let mut line = format!("{:>3}", idx + 1);
        for rtt in &hop.rtts {
            match rtt {
                Some(ms) if *ms < 1 => line.push_str("    <1 ms"),
                Some(ms) => line.push_str(&format!("{:>9}", format!("{ms} ms"))),
                None => line.push_str(&format!("{:>9}", "*")),
            }
        }
        line.push_str("  ");
        match &hop.ip {
            Some(ip) => {
                if args.d {
                    line.push_str(&ip.to_string());
                } else {
                    match dns_lookup::lookup_addr(ip) {
                        Ok(name) => line.push_str(&format!("{name} [{ip}]")),
                        Err(_) => line.push_str(&ip.to_string()),
                    }
                }
            }
            None => line.push_str(&i18n.tr("timeout-addr", None)),
        }
        println!("{line}");
    }

    println!();
    println!("{}", i18n.tr("trace-complete", None));

    ExitCode::SUCCESS
}
