//! shutdown —— 关闭 / 重启 / 注销计算机（复刻 Windows shutdown.exe）。
//!
//! 实测对齐的行为（Windows 11 原版）：
//! - 无参数 / /? / 无效参数 / 缺主操作：显示 Usage 帮助，退出码 1
//! - /a 无待取消关机：`Unable to abort the system shutdown because no shutdown
//!   was in progress.(1116)`，退出码 1116
//!
//! 实现策略：
//! - Windows：直接委托 `InitiateSystemShutdownExW` / `AbortSystemShutdownW`
//!   （带横幅倒计时与可取消机制，最接近原版）
//! - Linux：调用 `systemctl poweroff` / `systemctl reboot` 等（需 root）
//!
//! 权限要求与原版一致：Windows 需管理员（SE_SHUTDOWN_NAME），Linux 需 root。

use std::env;
use std::process::ExitCode;

#[cfg(not(windows))]
use std::process::Command;
#[cfg(not(windows))]
use std::thread;
#[cfg(not(windows))]
use std::time::Duration;

use windowshit_args::{parse, Flag, Kind, Parsed, Unknown};

const HELP: &str = "Usage: C:\\Windows\\System32\\shutdown.exe [/i | /l | /s | /sg | /r | /g | /a | /p | /h | /e | /o] [/hybrid] [/soft] [/fw] [/f]
    [/m \\\\computer][/t xxx][/d [p|u:]xx:yy [/c \"comment\"]]


    No args    Display help. This is the same as typing /?.
    /?         Display help. This is the same as not typing any options.
    /i         Display the graphical user interface (GUI).
               This must be the first option.
    /l         Log off. This cannot be used with /m or /d options.
    /s         Shutdown the computer.
    /sg        Shutdown the computer. On the next boot, if Automatic Restart Sign-On
               is enabled, automatically sign in and lock last interactive user.
               After sign in, restart any registered applications.
    /r         Full shutdown and restart the computer.
    /g         Full shutdown and restart the computer. After the system is rebooted,
               if Automatic Restart Sign-On is enabled, automatically sign in and
               lock last interactive user.
               After sign in, restart any registered applications.
    /a         Abort a system shutdown.
               This can only be used during the time-out period.
               Combine with /fw to clear any pending boots to firmware.
    /p         Turn off the local computer with no time-out or warning.
               Can be used with /d and /f options.
    /h         Hibernate the local computer.
               Can be used with the /f option.
    /hybrid    Performs a shutdown of the computer and prepares it for fast startup.
               Must be used with /s option.
    /fw        Combine with a shutdown option to cause the next boot to go to the
               firmware user interface.
    /e         Document the reason for an unexpected shutdown of a computer.
    /o         Go to the advanced boot options menu and restart the computer.
               Must be used with /r option.
    /m \\\\computer Specify the target computer.
    /t xxx     Set the time-out period before shutdown to xxx seconds.
               The valid range is 0-315360000 (10 years), with a default of 30.
               If the timeout period is greater than 0, the /f parameter is
               implied.
    /c \"comment\" Comment on the reason for the restart or shutdown.
               Maximum of 512 characters allowed.
    /f         Force running applications to close without forewarning users.
               The /f parameter is implied when a value greater than 0 is
               specified for the /t parameter.
    /d [p|u:]xx:yy  Provide the reason for the restart or shutdown.
               p indicates that the restart or shutdown is planned.
               u indicates that the reason is user defined.
               If neither p nor u is specified the restart or shutdown is
               unplanned.
               xx is the major reason number (positive integer less than 256).
               yy is the minor reason number (positive integer less than 65536).

Reasons on this computer:
(E = Expected U = Unexpected P = planned, C = customer defined)
Type\tMajor\tMinor\tTitle

 U  \t0\t0\tOther (Unplanned)
E   \t0\t0\tOther (Unplanned)
E P \t0\t0\tOther (Planned)
 U  \t0\t5\tOther Failure: System Unresponsive
E   \t1\t1\tHardware: Maintenance (Unplanned)
E P \t1\t1\tHardware: Maintenance (Planned)
E   \t1\t2\tHardware: Installation (Unplanned)
E P \t1\t2\tHardware: Installation (Planned)
E   \t2\t2\tOperating System: Recovery (Unplanned)
E P \t2\t2\tOperating System: Recovery (Planned)
  P \t2\t3\tOperating System: Upgrade (Planned)
E   \t2\t4\tOperating System: Reconfiguration (Unplanned)
E P \t2\t4\tOperating System: Reconfiguration (Planned)
  P \t2\t16\tOperating System: Service pack (Planned)
    \t2\t17\tOperating System: Hot fix (Unplanned)
  P \t2\t17\tOperating System: Hot fix (Planned)
    \t2\t18\tOperating System: Security fix (Unplanned)
  P \t2\t18\tOperating System: Security fix (Planned)
E   \t4\t1\tApplication: Maintenance (Unplanned)
E P \t4\t1\tApplication: Maintenance (Planned)
E P \t4\t2\tApplication: Installation (Planned)
E   \t4\t5\tApplication: Unresponsive
E   \t4\t6\tApplication: Unstable
 U  \t5\t15\tSystem Failure: Stop error
 U  \t5\t19\tSecurity issue (Unplanned)
E   \t5\t19\tSecurity issue (Unplanned)
E P \t5\t19\tSecurity issue (Planned)
E   \t5\t20\tLoss of network connectivity (Unplanned)
 U  \t6\t11\tPower Failure: Cord Unplugged
 U  \t6\t12\tPower Failure: Environment
  P \t7\t0\tLegacy API shutdown";

fn show_help() {
    println!("{HELP}");
}

/// 编码 /d 原因：返回 Windows reason code。
/// 格式：[p|u:]xx:yy。p=planned, u=user defined。
fn encode_reason(s: &str) -> Option<u32> {
    let (body, planned, userdef) = if let Some(b) = s.strip_prefix("p:") {
        (b, true, false)
    } else if let Some(b) = s.strip_prefix("u:") {
        (b, false, true)
    } else {
        (s, false, false)
    };
    let mut parts = body.split(':');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || major >= 256 || minor >= 65536 {
        return None;
    }
    let mut code = (major << 16) | minor;
    if planned {
        code |= 0x8000_0000; // SHTDN_REASON_FLAG_PLANNED
    }
    if userdef {
        code |= 0x4000_0000; // SHTDN_REASON_FLAG_USER_DEFINED
    }
    Some(code)
}

#[cfg(windows)]
fn is_admin() -> bool {
    // 检查进程是否提升（elevated）——判断管理员，不执行任何关机操作
    unsafe {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::Security::{
            GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
        };
        use windows_sys::Win32::System::Threading::{
            GetCurrentProcess, OpenProcessToken,
        };

        let mut token = std::mem::zeroed();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
fn is_admin() -> bool {
    // 用 geteuid 判断 euid==0（安全，不执行任何系统操作）
    unsafe { geteuid() == 0 }
}

#[cfg(not(windows))]
extern "C" {
    #[link_name = "geteuid"]
    fn geteuid() -> u32;
}

fn main() -> ExitCode {
    let raw: Vec<String> = env::args().skip(1).collect();

    // 无参数 / /? → 帮助
    if raw.is_empty() {
        show_help();
        return ExitCode::from(1);
    }
    if raw.iter().any(|a| a == "/?" || a == "-?") {
        show_help();
        return ExitCode::from(1);
    }

    // 解析开关（windowshit-args 精确匹配，区分 /S 与 /SG）
    const FLAGS: &[Flag] = &[
        Flag::new("HYBRID", Kind::Flag),
        Flag::new("SOFT", Kind::Flag),
        Flag::new("FW", Kind::Flag),
        Flag::new("F", Kind::Flag),
        Flag::new("T", Kind::Value),
        Flag::new("C", Kind::Value),
        Flag::new("M", Kind::Value),
        Flag::new("D", Kind::Value),
        Flag::new("I", Kind::Flag),
        Flag::new("L", Kind::Flag),
        Flag::new("S", Kind::Flag),
        Flag::new("SG", Kind::Flag),
        Flag::new("R", Kind::Flag),
        Flag::new("G", Kind::Flag),
        Flag::new("A", Kind::Flag),
        Flag::new("P", Kind::Flag),
        Flag::new("H", Kind::Flag),
        Flag::new("E", Kind::Flag),
        Flag::new("O", Kind::Flag),
    ];
    let parsed: Parsed = match parse(&raw, FLAGS, Unknown::Ignore) {
        Ok(p) => p,
        Err(_) => {
            show_help();
            return ExitCode::from(1);
        }
    };

    let has = |k: &str| parsed.flags.contains_key(k);
    let action = if has("S") {
        Some("S")
    } else if has("SG") {
        Some("SG")
    } else if has("R") {
        Some("R")
    } else if has("G") {
        Some("G")
    } else if has("A") {
        Some("A")
    } else if has("P") {
        Some("P")
    } else if has("H") {
        Some("H")
    } else if has("L") {
        Some("L")
    } else if has("E") {
        Some("E")
    } else if has("O") {
        Some("O")
    } else if has("I") {
        Some("I")
    } else {
        None
    };

    if action.is_none() {
        show_help();
        return ExitCode::from(1);
    }

    let hybrid = has("HYBRID");
    let force = has("F");
    let timeout = parsed
        .flags
        .get("T")
        .and_then(|v| *v)
        .and_then(|t| t.parse::<u32>().ok());
    let comment = parsed.flags.get("C").and_then(|v| *v).map(str::to_string);
    let machine = parsed.flags.get("M").and_then(|v| *v).map(str::to_string);
    let reason = parsed.flags.get("D").and_then(|v| *v).map(str::to_string);

    // 校验 /t 范围（参数校验在权限检查之前，与原版一致）
    let timeout = match timeout {
        Some(t) if t > 315_360_000 => {
            show_help();
            return ExitCode::from(1);
        }
        Some(t) => t,
        None => 30,
    };

    // /t > 0 隐含 /f
    let force = force || timeout > 0;

    // /d 原因编码
    let reason_code = match &reason {
        Some(r) => match encode_reason(r) {
            Some(c) => c,
            None => {
                show_help();
                return ExitCode::from(1);
            }
        },
        None => 0,
    };

    // /a 特殊：取消关机（无需管理员检查即可尝试，原版如此）
    if action == Some("A") {
        return abort_shutdown(machine.as_deref());
    }

    // 其它操作需要管理员/root
    if !is_admin() {
        #[cfg(windows)]
        {
            eprintln!("Access denied.(5)");
            return ExitCode::from(5);
        }
        #[cfg(not(windows))]
        {
            eprintln!("Must be run as root to shutdown the system.");
            return ExitCode::from(1);
        }
    }

    execute_shutdown(
        action,
        hybrid,
        force,
        timeout,
        comment.as_deref(),
        machine.as_deref(),
        reason_code,
    )
}

/// Windows 执行：委托 InitiateSystemShutdownExW / AbortSystemShutdownW。
#[cfg(windows)]
fn execute_shutdown(
    action: Option<&str>,
    _hybrid: bool,
    force: bool,
    timeout: u32,
    comment: Option<&str>,
    machine: Option<&str>,
    reason: u32,
) -> ExitCode {
    use windows_sys::Win32::System::Shutdown::{InitiateSystemShutdownExW, SHTDN_REASON_MAJOR_OTHER};

    let reboot = matches!(action, Some("R") | Some("G") | Some("SG") | Some("O"));

    // 构造宽字符串（去 \\ 前缀）
    let machine_wide: Vec<u16> = machine
        .as_ref()
        .map(|s| {
            s.trim_start_matches("\\\\")
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect()
        })
        .unwrap_or_default();
    let comment_wide: Vec<u16> = comment
        .map(|s| s.encode_utf16().chain(std::iter::once(0)).collect())
        .unwrap_or_default();

    let machine_ptr = if machine_wide.is_empty() {
        std::ptr::null()
    } else {
        machine_wide.as_ptr()
    };
    let comment_ptr = if comment_wide.is_empty() {
        std::ptr::null()
    } else {
        comment_wide.as_ptr()
    };

    let r = unsafe {
        InitiateSystemShutdownExW(
            machine_ptr,
            comment_ptr,
            timeout,
            force as i32,
            reboot as i32,
            reason | SHTDN_REASON_MAJOR_OTHER as u32,
        )
    };
    if r != 0 {
        ExitCode::SUCCESS
    } else {
        let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        eprintln!("The system shutdown was unsuccessful.({err})");
        ExitCode::from(err as u8)
    }
}

#[cfg(windows)]
fn abort_shutdown(machine: Option<&str>) -> ExitCode {
    use windows_sys::Win32::System::Shutdown::AbortSystemShutdownW;

    let machine_wide: Vec<u16> = machine
        .as_ref()
        .map(|s| {
            s.trim_start_matches("\\\\")
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect()
        })
        .unwrap_or_default();
    let machine_ptr = if machine_wide.is_empty() {
        std::ptr::null()
    } else {
        machine_wide.as_ptr()
    };
    let r = unsafe { AbortSystemShutdownW(machine_ptr) };
    if r != 0 {
        ExitCode::SUCCESS
    } else {
        let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        eprintln!("Unable to abort the system shutdown because no shutdown was in progress.({err})");
        ExitCode::from(err as u8)
    }
}

/// Linux 执行：调用 systemctl。
#[cfg(not(windows))]
fn execute_shutdown(
    action: Option<&str>,
    _hybrid: bool,
    _force: bool,
    timeout: u32,
    comment: Option<&str>,
    machine: Option<&str>,
    _reason: u32,
) -> ExitCode {
    let _ = comment;

    if machine.is_some() {
        eprintln!("Remote shutdown is not supported on this platform.");
        return ExitCode::from(1);
    }

    // /t > 0：倒计时（Ctrl+C 可取消，对应 Windows 横幅倒计时）
    if timeout > 0 {
        let action_label = match action {
            Some("R") | Some("G") | Some("SG") | Some("O") => "restart",
            Some("H") => "hibernate",
            Some("L") => "log off",
            _ => "shutdown",
        };
        println!(
            "System will {action_label} in {timeout} seconds. (Ctrl+C to cancel)"
        );
        for remaining in (1..=timeout).rev() {
            if remaining % 10 == 0 || remaining <= 5 {
                println!("{remaining}");
            }
            thread::sleep(Duration::from_secs(1));
        }
    }

    // 检查 systemctl 存在
    if !has_command("systemctl") {
        eprintln!("systemctl not found. Cannot perform shutdown.");
        return ExitCode::from(1);
    }

    let args: Vec<&str> = match action {
        Some("S") | Some("P") => vec!["poweroff"],
        Some("R") | Some("G") | Some("SG") | Some("O") => vec!["reboot"],
        Some("H") => vec!["hibernate"],
        Some("L") => vec!["logout"], // systemctl 无 logout，下面走 loginctl
        _ => {
            eprintln!("Unsupported action on this platform.");
            return ExitCode::from(1);
        }
    };

    let status = if action == Some("L") {
        // 注销：loginctl terminate-user <当前用户>
        let user = env::var("USER").unwrap_or_default();
        Command::new("loginctl")
            .arg("terminate-user")
            .arg(&user)
            .status()
    } else {
        Command::new("systemctl").args(&args).status()
    };

    match status {
        Ok(st) if st.success() => ExitCode::SUCCESS,
        Ok(st) => {
            eprintln!("Failed to execute systemctl ({:?}).", st.code());
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("Failed to execute systemctl: {e}");
            ExitCode::from(1)
        }
    }
}

#[cfg(not(windows))]
fn abort_shutdown(_machine: Option<&str>) -> ExitCode {
    // 取消 systemctl 待执行的关机
    if !has_command("systemctl") {
        eprintln!("systemctl not found.");
        return ExitCode::from(1);
    }
    match Command::new("systemctl").arg("cancel").status() {
        Ok(st) if st.success() => ExitCode::SUCCESS,
        _ => {
            eprintln!("Unable to abort the system shutdown because no shutdown was in progress.");
            ExitCode::from(1)
        }
    }
}

#[cfg(not(windows))]
fn has_command(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| {
            std::env::split_paths(&p).any(|d| {
                let full = d.join(if cfg!(windows) {
                    format!("{name}.exe")
                } else {
                    name.to_string()
                });
                full.is_file()
            })
        })
        .unwrap_or(false)
}


