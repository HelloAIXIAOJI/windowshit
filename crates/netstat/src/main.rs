//! netstat —— 显示网络连接（复刻 Windows netstat.exe）。
//!
//! 已实现：-a -n -o -p proto -q -b、interval 重复显示、默认连接列表。
//! 未实现（明确报错）：-e -f -i -r -s -t -x -y。
//!
//! 实测原版行为对照：
//! - 默认只显示 TCP 非监听连接（含 TIME_WAIT 等，不含 UDP）
//! - `-a` 显示全部 TCP（含 LISTENING）与 UDP；`-q` 额外显示 BOUND
//! - `-p tcp` 只过滤 IPv4 TCP，`-p tcpv6` 只过滤 IPv6
//! - 非 `-n` 模式本地地址不解析，远端解析主机名 + services 服务名
//! - 无效选项 / 无效 -p 值：stderr 打印完整帮助，exit 1
//! - `-b` 非管理员：stdout 输出 "The requested operation requires elevation."，exit 1

mod args;
mod format;
mod net;
mod services;

use std::collections::HashMap;
use std::process::ExitCode;

use sysinfo::{ProcessesToUpdate, System};

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();

    // 帮助：/ ? 输出到 stdout，exit 0
    if raw.iter().any(|a| a == "/?" || a == "-?") {
        println!("{}", include_str!("../locales/help.txt"));
        return ExitCode::SUCCESS;
    }

    let args = match args::parse(&raw) {
        Ok(a) => a,
        Err(_) => {
            // 无效参数：stderr 打印完整帮助，exit 1（实测原版行为）
            eprintln!("\n{}", include_str!("../locales/help.txt"));
            return ExitCode::from(1);
        }
    };

    // 未实现选项：明确报错，不假装支持
    if let Some(ch) = args.unsupported {
        eprintln!("netstat: the option -{ch} is not supported in this implementation.");
        return ExitCode::from(1);
    }

    // -b 需要管理员权限（实测非管理员直接失败）
    #[cfg(windows)]
    if args.show_b && !is_elevated() {
        println!("The requested operation requires elevation.");
        return ExitCode::from(1);
    }

    let mut conns = match net::query() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("netstat: {e}");
            return ExitCode::from(1);
        }
    };
    net::apply_udp_remotes(&mut conns);

    let svcs = services::Services::load();
    let host = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();

    let procs = if args.show_b {
        process_map()
    } else {
        HashMap::new()
    };

    // 输出一次连接列表（interval 模式循环调用）
    let render = || {
        println!("\nActive Connections\n");
        if args.show_pid || args.show_b {
            println!("{}", format::HEADER_PID);
        } else {
            println!("{}", format::HEADER);
        }

        let entries = net::filter_and_sort(&conns, &args);
        for e in entries {
            let local = format::addr(e.local_ip, e.local_port);
            let foreign = format::foreign(e, args.numeric, &svcs, &host);
            let state = e.state.unwrap_or("");
            let pid = if args.show_pid || args.show_b {
                Some(e.pid.unwrap_or(0))
            } else {
                None
            };
            println!("{}", format::line(e.proto, &local, &foreign, state, pid));
            // -b：连接行后打印所属进程可执行名
            if args.show_b {
                if let Some(name) = e.pid.and_then(|p| procs.get(&p)) {
                    println!(" [{}]", name);
                }
            }
        }
    };

    match args.interval {
        Some(secs) => loop {
            render();
            std::thread::sleep(std::time::Duration::from_secs(secs));
        },
        None => render(),
    }

    ExitCode::SUCCESS
}

/// pid → 进程可执行名（-b 使用）。
fn process_map() -> HashMap<u32, String> {
    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.processes()
        .iter()
        .map(|(pid, info)| {
            (
                pid.as_u32(),
                info.name().to_string_lossy().to_string(),
            )
        })
        .collect()
}

/// Windows：当前进程是否以管理员（提权）运行。
#[cfg(windows)]
fn is_elevated() -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    // SAFETY: 标准令牌 API，TOKEN_ELEVATION 为栈上结构
    unsafe {
        let mut token: windows_sys::Win32::Foundation::HANDLE = 0;
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) != 0 {
            let mut elev: TOKEN_ELEVATION = std::mem::zeroed();
            let mut len: u32 = 0;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                &mut elev as *mut TOKEN_ELEVATION as *mut _,
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut len,
            ) != 0;
            CloseHandle(token);
            ok && elev.TokenIsElevated != 0
        } else {
            false
        }
    }
}
