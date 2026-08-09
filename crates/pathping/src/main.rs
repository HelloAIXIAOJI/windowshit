mod stats;

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::process::ExitCode;
use std::time::Duration;

use windowshit_i18n::{FluentArgs, L10n};
use windowshit_trace_core::{trace, TraceConfig};

/// 让 Windows 控制台用 UTF-8 输出，避免中文乱码
#[cfg(windows)]
fn setup_console_utf8() {
    // SAFETY: 只调用标准 Win32 API，无其他副作用
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleOutputCP(65001);
    }
}

struct Args {
    hops: u8,
    queries: u32,
    period_ms: u64,
    wait_ms: u64,
    no_resolve: bool,
    src: Option<String>,
    v4: bool,
    v6: bool,
    target: Option<String>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            hops: 30,
            queries: 100,
            period_ms: 250,
            wait_ms: 3000,
            no_resolve: false,
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
            let (flag, rest) = if opt.len() > 1 { opt.split_at(1) } else { (opt, "") };
            let inline: Option<&str> = if rest.is_empty() { None } else { Some(rest) };

            // 未实现的选项：明确报错
            if flag == "g" {
                let mut a = FluentArgs::new();
                a.set("flag", flag);
                return Err(i18n.tr("error-unsupported", Some(&a)));
            }

            // 取值选项
            let take = matches!(flag, "h" | "q" | "w" | "p" | "i");
            let value: Option<String> = if take {
                let v = match inline {
                    Some(v) if !v.is_empty() => v.to_string(),
                    _ => {
                        i += 1;
                        if i >= raw.len() {
                            let mut a = FluentArgs::new();
                            a.set("flag", flag);
                            return Err(i18n.tr("error-bad-option", Some(&a)));
                        }
                        raw[i].clone()
                    }
                };
                Some(v)
            } else {
                None
            };

            match flag {
                "n" => args.no_resolve = true,
                "h" => {
                    let n: u64 = value.unwrap().parse().map_err(|_| bad(i18n, "h"))?;
                    if !(1..=255).contains(&n) {
                        return Err(bad(i18n, "h"));
                    }
                    args.hops = n as u8;
                }
                "q" => {
                    let n: u64 = value.unwrap().parse().map_err(|_| bad(i18n, "q"))?;
                    if !(1..=u32::MAX as u64).contains(&n) {
                        return Err(bad(i18n, "q"));
                    }
                    args.queries = n as u32;
                }
                "p" => {
                    args.period_ms = value.unwrap().parse().unwrap_or(250);
                }
                "w" => {
                    args.wait_ms = value.unwrap().parse().unwrap_or(3000);
                }
                "i" => args.src = value,
                "4" => args.v4 = true,
                "6" => args.v6 = true,
                _ => return Err(bad(i18n, flag)),
            }
        } else {
            if args.target.is_some() {
                return Err(bad(i18n, arg));
            }
            args.target = Some(arg.clone());
        }
        i += 1;
    }
    Ok(args)
}

fn bad(i18n: &L10n, flag: &str) -> String {
    let mut a = FluentArgs::new();
    a.set("flag", flag);
    i18n.tr("error-bad-option", Some(&a))
}

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

/// 本机在目标网络上的源地址（UDP connect 技巧）。
fn local_ip_for(ip: IpAddr) -> Option<IpAddr> {
    let bind: SocketAddr = if ip.is_ipv4() {
        "0.0.0.0:0".parse().ok()?
    } else {
        "[::]:0".parse().ok()?
    };
    let s = UdpSocket::bind(bind).ok()?;
    s.connect(SocketAddr::new(ip, 0)).ok()?;
    Some(s.local_addr().ok()?.ip())
}

#[tokio::main]
async fn main() -> ExitCode {
    let mut i18n = L10n::detect();
    match i18n.lang() {
        "zh-CN" => i18n.add_ftl(include_str!("../locales/zh-CN.ftl")),
        _ => i18n.add_ftl(include_str!("../locales/en-US.ftl")),
    }
    i18n.set_help(
        include_str!("../locales/help.zh.txt"),
        include_str!("../locales/help.en.txt"),
    );

    #[cfg(windows)]
    setup_console_utf8();

    let raw: Vec<String> = std::env::args().skip(1).collect();

    if raw.is_empty() {
        println!("{}", i18n.help());
        return ExitCode::from(1);
    }
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
        eprintln!("{}", bad(&i18n, "4"));
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

    // ===== 阶段 1：路由跟踪 =====
    let cfg = TraceConfig {
        ip,
        max_hops: args.hops,
        wait: Duration::from_millis(args.wait_ms),
        probes_per_hop: 1,
        src,
    };
    let hops = match trace(&cfg) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };

    let mut a = FluentArgs::new();
    a.set("target", &target);
    a.set("hops", u64::from(args.hops));
    println!("{}", i18n.tr("trace-header", Some(&a)));
    println!();

    // hop0 = 本机
    let local = local_ip_for(ip).unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
    println!("{:>3}  {}", 0, fmt_addr(&local, args.no_resolve));

    // 阶段 1 输出
    let mut addr_list: Vec<IpAddr> = vec![local];
    for (idx, h) in hops.iter().enumerate() {
        let addr = match &h.ip {
            Some(a) => {
                addr_list.push(*a);
                fmt_addr(a, args.no_resolve)
            }
            None => i18n.tr("timeout-addr", None),
        };
        println!("{:>3}  {}", idx + 1, addr);
    }
    println!();

    // ===== 阶段 2：逐跳统计 =====
    let seconds = ((args.queries as u64 - 1) * args.period_ms + 999) / 1000;
    let mut a = FluentArgs::new();
    a.set("seconds", seconds);
    println!("{}", i18n.tr("computing-stats", Some(&a)));

    // 表头（实测原版列位置，任何语言版均保留英文技术缩写）
    println!("            Source to Here   This Node/Link");
    println!("Hop  RTT    Lost/Sent = Pct  Lost/Sent = Pct  Address");

    // 对本机之后每一跳做 q 次 ping 统计
    let stats = stats::collect(&addr_list[1..], args.queries, args.wait_ms).await;

    let q = args.queries;
    let mut prev_lost: u32 = 0;
    for (i, st) in stats.iter().enumerate() {
        let ip = addr_list[1 + i];
        let hop_idx = i + 1;

        let rtt_col = match st.avg_rtt_ms() {
            Some(rtt) => format!("{:>5}ms", rtt),
            None => " ".repeat(7),
        };

        if hop_idx == 1 {
            // hop0 行（本机）+ 竖线
            println!("{:>3}{}{:>17}{:>17}  {}", 0, " ".repeat(7), "", "", fmt_addr(&local, args.no_resolve));
            let link_pct = pct(st.lost, q);
            println!("{:>33}{}/{:>4} = {:>2}%   |", "", st.lost, q, link_pct);
        }

        // Source to Here = 到该跳累计丢包；This Node/Link = 相对上一跳的增量
        let src_col = format!("{:>5}{}/{:>4} = {:>2}%", "", st.lost, q, pct(st.lost, q));
        let link_lost = st.lost.saturating_sub(prev_lost);
        let link_col = format!("{:>5}{}/{:>4} = {:>2}%", "", link_lost, q, pct(link_lost, q));
        prev_lost = st.lost;

        println!("{:>3}{}{}{}  {}", hop_idx, rtt_col, src_col, link_col, fmt_addr(&ip, args.no_resolve));

        // 非最后一跳输出竖线行
        if i + 1 < stats.len() {
            let next_lost = stats[i + 1].lost;
            let next_link = next_lost.saturating_sub(st.lost);
            println!("{:>33}{}/{:>4} = {:>2}%   |", "", next_link, q, pct(next_link, q));
        }
    }

    println!();
    println!("{}", i18n.tr("trace-complete", None));

    ExitCode::SUCCESS
}

/// 地址列：-n 时不反解析。
fn fmt_addr(ip: &IpAddr, no_resolve: bool) -> String {
    if no_resolve {
        return ip.to_string();
    }
    match dns_lookup::lookup_addr(ip) {
        Ok(name) => format!("{name} [{ip}]"),
        Err(_) => ip.to_string(),
    }
}

fn pct(lost: u32, sent: u32) -> u32 {
    if sent == 0 {
        0
    } else {
        (u64::from(lost) * 100 / u64::from(sent)) as u32
    }
}
