//! choice —— 提示选择并返回选择码（复刻 Windows choice.exe）。
//!
//! 实测对齐的行为：
//! - 提示格式：`message [C1,C2,C3]?`（无 /n 显示选项列表，无 /m 无消息）
//! - 按键即返回（无需回车），大小写不敏感（/cs 区分大小写）
//! - /t 秒 + /d 默认：超时自动选默认，并在提示后打印默认字符
//! - 退出码：选中第 k 个选项返回 k（1-based）
//! - /t 必须与 /d 同时指定
//! - 无参数默认选项 Y,N
//!
//! 读按键用 crossterm（跨平台 raw mode）；stdin 非终端（管道）时
//! 回退为读取一个字节。

use std::io::{self, IsTerminal, Read, Write};
use std::process::ExitCode;
use std::time::Duration;

use crossterm::event::{poll, read, Event, KeyCode, KeyModifiers};

const HELP: &str = "CHOICE [/C choices] [/N] [/CS] [/T timeout /D choice] [/M text]

Description:
    This tool allows users to select one item from a list
    of choices and returns the index of the selected choice.

Parameter List:
    /C    choices       Specifies the list of choices to be created.
                        Default list is \"YN\".
    /N    Hide the list of choices in the prompt.
                        The message before the prompt is still displayed.
    /CS   Enables case-sensitive choices to be selected.
    /T    timeout       The number of seconds to pause before defaulting
                        to the specified choice. Acceptable values are
                        from 0 to 9999. If 0 is specified, there will be
                        no pause and the default choice is selected.
    /D    choice        Specifies the default choice after timeout.
                        Must be one of the choices.
    /M    text          Specifies the message to be displayed before the
                        prompt.
    /?    Displays this help message.

NOTE:
    ERRORLEVEL environment variable is set to the index of the key that
    was selected from the set of choices. The first choice selected returns
    1, the second returns 2, and so on. If the user presses a key that is
    not a valid choice, the tool sounds a warning beep.";

struct Opts {
    choices: Vec<char>,
    no_list: bool,
    case_sensitive: bool,
    timeout: Option<u64>,
    default: Option<char>,
    message: Option<String>,
}

fn default_opts() -> Opts {
    Opts {
        choices: vec!['Y', 'N'],
        no_list: false,
        case_sensitive: false,
        timeout: None,
        default: None,
        message: None,
    }
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();

    if raw.iter().any(|a| a == "/?" || a == "-?") {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }

    let mut o = default_opts();
    let mut i = 0usize;
    while i < raw.len() {
        let a = &raw[i];
        if a.starts_with('/') || a.starts_with('-') {
            match a[1..].to_ascii_uppercase().as_str() {
                "C" => {
                    // 选项列表（如 /C YN 或 /C:YN）
                    let choices = if let Some(rest) = a[1..].strip_prefix("C:") {
                        rest.to_string()
                    } else {
                        i += 1;
                        raw.get(i).cloned().unwrap_or_default()
                    };
                    if choices.is_empty() {
                        eprintln!("ERROR: The choices list is empty.");
                        return ExitCode::from(255);
                    }
                    o.choices = choices.chars().collect();
                }
                "N" => o.no_list = true,
                "CS" => o.case_sensitive = true,
                "T" => {
                    let t = if let Some(rest) = a[1..].strip_prefix("T:") {
                        rest.to_string()
                    } else {
                        i += 1;
                        raw.get(i).cloned().unwrap_or_default()
                    };
                    match t.trim().parse::<u64>() {
                        Ok(v) if v <= 9999 => o.timeout = Some(v),
                        _ => {
                            eprintln!("ERROR: The timeout value is invalid.");
                            return ExitCode::from(255);
                        }
                    }
                }
                "D" => {
                    let d = if let Some(rest) = a[1..].strip_prefix("D:") {
                        rest.to_string()
                    } else {
                        i += 1;
                        raw.get(i).cloned().unwrap_or_default()
                    };
                    o.default = d.chars().next();
                }
                "M" => {
                    let m = if let Some(rest) = a[1..].strip_prefix("M:") {
                        rest.to_string()
                    } else {
                        i += 1;
                        raw.get(i).cloned().unwrap_or_default()
                    };
                    o.message = Some(m);
                }
                _ => {
                    eprintln!("ERROR: Invalid syntax.");
                    return ExitCode::from(255);
                }
            }
        } else {
            eprintln!("ERROR: Invalid syntax.");
            return ExitCode::from(255);
        }
        i += 1;
    }

    // /t 必须与 /d 同时指定（实测原版报错）
    if o.timeout.is_some() && o.default.is_none() {
        eprintln!("ERROR: Invalid syntax. /T can be specified only when /D is specified.");
        println!("Type \"CHOICE /?\" for usage.");
        return ExitCode::from(255);
    }
    if o.timeout.is_none() && o.default.is_some() {
        eprintln!("ERROR: Invalid syntax. /D can be specified only when /T is specified.");
        println!("Type \"CHOICE /?\" for usage.");
        return ExitCode::from(255);
    }

    // 默认字符必须在选项中
    if let Some(d) = o.default {
        let ok = o.choices.iter().any(|c| char_eq(*c, d, o.case_sensitive));
        if !ok {
            eprintln!("ERROR: The default choice is not in the choices list.");
            return ExitCode::from(255);
        }
    }

    // 打印提示
    let mut prompt = String::new();
    if let Some(m) = &o.message {
        prompt.push_str(m);
        prompt.push(' ');
    }
    if !o.no_list {
        let list: Vec<String> = o.choices.iter().map(|c| c.to_string()).collect();
        prompt.push('[');
        prompt.push_str(&list.join(","));
        prompt.push(']');
    }
    prompt.push('?');
    print!("{prompt}");
    let _ = io::stdout().flush();

    // 读取选择
    let is_tty = io::stdin().is_terminal();
    let mut raw_mode = false;
    if is_tty {
        let _ = crossterm::terminal::enable_raw_mode();
        raw_mode = true;
    }

    // 读取选择：循环直到有效键或超时。
    // 无效按键 → 蜂鸣（\x07）并继续等待（原版行为，不退出）。
    let idx = if let Some(t) = o.timeout {
        read_with_timeout(&o, t, is_tty)
    } else {
        read_until_valid(&o, is_tty)
    };

    if raw_mode {
        let _ = crossterm::terminal::disable_raw_mode();
    }

    match idx {
        Some(k) => {
            // raw mode 关闭了终端回显，手动回显选中的字符
            // （超时路径原版也会打印默认字符，实测 "Test [A,B,C]?B"）
            print!("{}", o.choices[k]);
            let _ = io::stdout().flush();
            println!();
            ExitCode::from((k as u8) + 1)
        }
        None => {
            // Ctrl+C 等中断 / EOF
            ExitCode::from(130)
        }
    }
}

/// 无超时：循环读键直到命中选项。无效键蜂鸣后继续。
fn read_until_valid(o: &Opts, is_tty: bool) -> Option<usize> {
    loop {
        let ch = if is_tty { read_key() } else { read_byte() };
        match ch {
            Some(c) => {
                if let Some(idx) = o
                    .choices
                    .iter()
                    .position(|x| char_eq(*x, c, o.case_sensitive))
                {
                    return Some(idx);
                }
                // 无效按键：蜂鸣并继续（原版会重新等待）
                print!("\x07");
                let _ = io::stdout().flush();
                if !is_tty {
                    // 管道/EOF：无法再等，放弃
                    return None;
                }
            }
            None => return None, // Ctrl+C / EOF / 读取失败
        }
    }
}

/// 带超时：deadline 内循环读键；超时回退默认选项。
fn read_with_timeout(o: &Opts, t: u64, is_tty: bool) -> Option<usize> {
    let default_idx = o.default.and_then(|d| {
        o.choices
            .iter()
            .position(|c| char_eq(*c, d, o.case_sensitive))
    });
    if t == 0 {
        return default_idx;
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(t);
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let ch = if is_tty {
            match poll(remaining) {
                Ok(true) => read_key(),
                _ => None,
            }
        } else {
            read_byte_timeout(remaining)
        };
        match ch {
            Some(c) => {
                if let Some(idx) = o
                    .choices
                    .iter()
                    .position(|x| char_eq(*x, c, o.case_sensitive))
                {
                    return Some(idx);
                }
                // 无效按键：蜂鸣并继续等（在剩余时间内）
                print!("\x07");
                let _ = io::stdout().flush();
            }
            None => return default_idx, // 超时 → 默认
        }
    }
}

fn char_eq(a: char, b: char, case_sensitive: bool) -> bool {
    if case_sensitive {
        a == b
    } else {
        a.eq_ignore_ascii_case(&b)
    }
}

/// 终端模式：读取一个按键，返回对应字符。
fn read_key() -> Option<char> {
    loop {
        match read() {
            Ok(Event::Key(k)) => {
                // Ctrl 组合（含 Ctrl+C）视为中断
                if k.modifiers.contains(KeyModifiers::CONTROL) {
                    return None;
                }
                match k.code {
                    KeyCode::Char(c) => return Some(c),
                    KeyCode::Enter => return Some('\r'),
                    KeyCode::Esc => return Some('\x1b'),
                    KeyCode::Backspace => return Some('\x08'),
                    KeyCode::Tab => return Some('\t'),
                    _ => continue,
                }
            }
            Ok(_) => continue,
            Err(_) => return None,
        }
    }
}

/// 非终端模式：阻塞读一个字节（管道/重定向）。
fn read_byte() -> Option<char> {
    let mut buf = [0u8; 1];
    match io::stdin().read(&mut buf) {
        Ok(0) => None, // EOF
        Ok(_) => Some(buf[0] as char),
        Err(_) => None,
    }
}

/// 非终端模式：读一个字节，超时返回 None。
/// stdin read 是阻塞的，用线程读取 + 通道超时。
fn read_byte_timeout(t: Duration) -> Option<char> {
    let (tx, rx) = std::sync::mpsc::channel::<char>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 1];
        match io::stdin().read(&mut buf) {
            Ok(0) => {} // EOF
            Ok(_) => {
                let _ = tx.send(buf[0] as char);
            }
            Err(_) => {}
        }
    });
    rx.recv_timeout(t).ok()
}
