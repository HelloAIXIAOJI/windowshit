mod args;
mod i18n;
mod ping;

use std::net::IpAddr;
use std::process::ExitCode;

use fluent::FluentArgs;
use i18n::L10n;
use ping::Outcome;
use tokio::select;
use tokio::signal;

/// 让 Windows 控制台用 UTF-8 输出，避免中文乱码
#[cfg(windows)]
fn setup_console_utf8() {
    // SAFETY: 只调用标准 Win32 API，无其他副作用
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleOutputCP(65001);
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    #[cfg(windows)]
    setup_console_utf8();

    let i18n = L10n::detect();

    let raw: Vec<String> = std::env::args().skip(1).collect();
    let args = match args::parse(&raw, &i18n) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            eprintln!();
            eprintln!("{}", i18n.usage());
            return ExitCode::from(1);
        }
    };

    if args.help {
        println!("{}", i18n.help());
        return ExitCode::SUCCESS;
    }

    let target = match &args.target {
        Some(t) => t.clone(),
        None => {
            eprintln!("{}", i18n.tr("error-no-target", None));
            eprintln!();
            eprintln!("{}", i18n.usage());
            return ExitCode::from(1);
        }
    };

    // 尚未实现的 Windows 特有选项：诚实报错，而不是假装支持
    let unsupported = [
        (args.record_route.is_some(), "-r"),
        (args.timestamp.is_some(), "-s"),
        (args.loose_route.is_some(), "-j"),
        (args.strict_route.is_some(), "-k"),
        (args.reverse_route, "-R"),
        (args.compartment.is_some(), "-c"),
        (args.hyperv, "-p"),
    ];
    if let Some((_, flag)) = unsupported.iter().find(|(used, _)| *used) {
        let mut a = FluentArgs::new();
        a.set("flag", *flag);
        eprintln!("{}", i18n.tr("error-unsupported", Some(&a)));
        return ExitCode::from(1);
    }

    // 解析目标地址
    let addr = match ping::resolve(&target, args.ipv4, args.ipv6).await {
        Ok(a) => a,
        Err(_) => {
            let mut a = FluentArgs::new();
            a.set("host", &target);
            eprintln!("{}", i18n.tr("error-cannot-resolve", Some(&a)));
            return ExitCode::from(1);
        }
    };

    // -S 源地址解析
    let src_addr = match &args.src_addr {
        Some(s) => match s.parse::<IpAddr>() {
            Ok(ip) => Some(ip),
            Err(_) => {
                let mut a = FluentArgs::new();
                a.set("src", s.as_str());
                eprintln!("{}", i18n.tr("error-bad-src", Some(&a)));
                return ExitCode::from(1);
            }
        },
        None => None,
    };

    // 首行输出
    let start_line = if target.parse::<IpAddr>().is_ok() {
        if args.resolve_name {
            if let Some(name) = ping::reverse_lookup(addr).await {
                let mut a = FluentArgs::new();
                a.set("host", &name);
                a.set("addr", addr.to_string());
                a.set("size", args.size as u64);
                i18n.tr("ping-start-host", Some(&a))
            } else {
                let mut a = FluentArgs::new();
                a.set("addr", addr.to_string());
                a.set("size", args.size as u64);
                i18n.tr("ping-start-ip", Some(&a))
            }
        } else {
            let mut a = FluentArgs::new();
            a.set("addr", addr.to_string());
            a.set("size", args.size as u64);
            i18n.tr("ping-start-ip", Some(&a))
        }
    } else {
        let mut a = FluentArgs::new();
        a.set("host", &target);
        a.set("addr", addr.to_string());
        a.set("size", args.size as u64);
        i18n.tr("ping-start-host", Some(&a))
    };
    println!("{start_line}");

    // 初始化 ICMP
    let client = match ping::build_client(addr, args.ttl, src_addr) {
        Ok(c) => c,
        Err(e) => {
            let mut a = FluentArgs::new();
            a.set("err", &e);
            eprintln!("{}", i18n.tr("error-init-icmp", Some(&a)));
            eprintln!("{}", i18n.tr("error-init-hint", None));
            return ExitCode::from(1);
        }
    };

    // -f / -v（仅 IPv4 有效）
    if addr.is_ipv4() && (args.no_fragment || args.tos.is_some()) {
        if let Err(e) = ping::apply_socket_options(&client, args.no_fragment, args.tos) {
            let mut a = FluentArgs::new();
            a.set("err", &e);
            eprintln!("{}", i18n.tr("error-sockopt", Some(&a)));
            return ExitCode::from(1);
        }
    }

    // Windows 原生 ping 用进程 PID 作为 ICMP identifier
    let ident = surge_ping::PingIdentifier((std::process::id() & 0xFFFF) as u16);
    let mut pinger = client.pinger(addr, ident).await;

    let payload = vec![0u8; args.size];

    let mut seq: u16 = 0;
    let mut sent: u32 = 0;
    let mut received: u32 = 0;
    let mut rtts: Vec<u128> = Vec::new();
    let mut interrupted = false;

    // Windows 原版两个回显请求之间固定间隔 1 秒
    let interval = std::time::Duration::from_secs(1);
    let mut last_send = std::time::Instant::now();

    loop {
        select! {
            _ = signal::ctrl_c() => {
                interrupted = true;
            }
            outcome = ping::ping_once(&mut pinger, seq, &payload, args.timeout_ms) => {
                match outcome {
                    Outcome::Reply { source, data_len, ttl, rtt_ms } => {
                        received += 1;
                        rtts.push(rtt_ms);
                        let mut a = FluentArgs::new();
                        a.set("source", source.to_string());
                        a.set("data_len", data_len as u64);
                        if addr.is_ipv4() {
                            let ttl = ttl.unwrap_or(0);
                            a.set("ttl", u64::from(ttl));
                            if rtt_ms < 1 {
                                println!("{}", i18n.tr("reply-v4-fast", Some(&a)));
                            } else {
                                a.set("rtt", rtt_ms as u64);
                                println!("{}", i18n.tr("reply-v4", Some(&a)));
                            }
                        } else if rtt_ms < 1 {
                            println!("{}", i18n.tr("reply-v6-fast", Some(&a)));
                        } else {
                            a.set("rtt", rtt_ms as u64);
                            println!("{}", i18n.tr("reply-v6", Some(&a)));
                        }
                    }
                    Outcome::TimeExceeded { source } => {
                        let mut a = FluentArgs::new();
                        a.set("source", source.to_string());
                        println!("{}", i18n.tr("time-exceeded", Some(&a)));
                    }
                    Outcome::DestUnreachable { source } => {
                        let mut a = FluentArgs::new();
                        a.set("source", source.to_string());
                        println!("{}", i18n.tr("dest-unreachable", Some(&a)));
                    }
                    Outcome::Timeout => {
                        println!("{}", i18n.tr("timeout", None));
                    }
                    Outcome::Other { source } => {
                        let mut a = FluentArgs::new();
                        a.set("source", source.to_string());
                        println!("{}", i18n.tr("dest-unreachable", Some(&a)));
                    }
                }
            }
        }

        sent += 1;
        if interrupted {
            break;
        }
        if !args.continuous && sent >= args.count {
            break;
        }
        seq = seq.wrapping_add(1);

        // 保证发送间隔 >= 1 秒：回复快的补足间隔，超时慢的（已超过 1 秒）不额外等待
        let elapsed = last_send.elapsed();
        if elapsed < interval {
            let wait = interval - elapsed;
            select! {
                _ = signal::ctrl_c() => {
                    interrupted = true;
                }
                _ = tokio::time::sleep(wait) => {}
            }
        }
        last_send = std::time::Instant::now();
        if interrupted {
            break;
        }
    }

    // 统计信息（复刻 Windows 输出格式）
    println!();
    let loss = sent - received;
    let loss_pct = if sent > 0 { loss * 100 / sent } else { 0 };

    let mut a = FluentArgs::new();
    a.set("addr", addr.to_string());
    println!("{}", i18n.tr("stats-header", Some(&a)));

    let mut a = FluentArgs::new();
    a.set("sent", u64::from(sent));
    a.set("received", u64::from(received));
    a.set("loss", u64::from(loss));
    a.set("loss_pct", u64::from(loss_pct));
    println!("    {}", i18n.tr("stats-packets", Some(&a)));

    if received > 0 {
        let min = rtts.iter().min().copied().unwrap_or(0);
        let max = rtts.iter().max().copied().unwrap_or(0);
        let avg = rtts.iter().sum::<u128>() / rtts.len() as u128;
        println!("{}", i18n.tr("stats-rtt-header", None));

        let mut a = FluentArgs::new();
        a.set("min", min as u64);
        a.set("max", max as u64);
        a.set("avg", avg as u64);
        println!("    {}", i18n.tr("stats-rtt-line", Some(&a)));
    }

    if received > 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
