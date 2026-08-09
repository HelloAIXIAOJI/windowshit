mod args;
mod ping;

use std::net::IpAddr;
use std::process::ExitCode;

use args::ArgError;
use ping::Outcome;
use tokio::select;
use tokio::signal;
use windowshit_i18n::{FluentArgs, L10n};

/// 让 Windows 控制台用 UTF-8 输出，避免中文乱码
#[cfg(windows)]
fn setup_console_utf8() {
    // SAFETY: 只调用标准 Win32 API，无其他副作用
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleOutputCP(65001);
    }
}

/// 处理参数错误：原版无参数时直接打印完整帮助（无错误文本），
/// 无效选项后打印完整帮助，其它参数错误只打印一行。
fn report_arg_error(i18n: &L10n, e: ArgError) -> ExitCode {
    if !e.message.is_empty() {
        eprintln!("{}", e.message);
    }
    if e.show_help {
        eprintln!();
        println!("{}", i18n.help());
    }
    ExitCode::from(1)
}

#[tokio::main]
async fn main() -> ExitCode {
    // 必须先读代码页决定语言，再改 UTF-8 输出（否则语言检测读到被改掉的代码页）
    let mut i18n = L10n::detect();
    // 注入 ping 自己的翻译与帮助
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

    // 无参数：原版直接打印完整帮助并退出 1，无错误文本
    if raw.is_empty() {
        println!("{}", i18n.help());
        return ExitCode::from(1);
    }

    let args = match args::parse(&raw, &i18n) {
        Ok(a) => a,
        Err(e) => return report_arg_error(&i18n, e),
    };

    if args.help {
        println!("{}", i18n.help());
        return ExitCode::SUCCESS;
    }

    let target = args.target.clone().unwrap();

    // -S 源地址合法性
    let src_addr = match &args.src_addr {
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
            outcome = async {
                // -w 非数字：原版进入"发送失败"模式，不发包
                if args.transmit_fail {
                    tokio::time::sleep(std::time::Duration::from_millis(args.timeout_ms)).await;
                    return None;
                }
                Some(ping::ping_once(&mut pinger, seq, &payload, args.timeout_ms).await)
            } => {
                match outcome {
                    Some(outcome) => match outcome {
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
                    },
                    None => {
                        if args.transmit_fail {
                            println!("{}", i18n.tr("transmit-failed", None));
                        }
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

        // 保证发送间隔 >= 1 秒（快速回复补足，超时慢的等待已超过则不等）
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
