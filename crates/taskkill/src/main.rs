//! taskkill —— 按 PID / 映像名终止进程（复刻 Windows taskkill.exe）。
//!
//! 分工：
//! - 枚举进程：`sysinfo`（跨平台，tasklist 已验证可靠）
//! - 终止进程：`kill_tree`（跨平台统一 crate，内部封装平台 API：
//!   Windows 走 OpenProcess+TerminateProcess，Unix 走 nix 信号）
//! 本文件**无平台分支**。
//!
//! 行为对齐原版（实测校准）：
//! - 无目标 / 语法错误 → 退出码 1
//! - PID / IM 未找到 → 退出码 128
//! - 成功 / /FI 未命中 → 退出码 0
//! - /F → SIGKILL（Windows 同 TerminateProcess）；无 /F → SIGTERM
//! - /T 递归杀进程树（kill_tree 内置）
//! - /S /U /P 远程参数不支持，明确报错

use std::process::ExitCode;

use kill_tree::{blocking::kill_tree_with_config, Config, Output};
use sysinfo::System;
use windowshit_args::{parse, Error, Flag, Kind, Unknown};
use windowshit_i18n::{FluentArgs, L10n};

/// 让 Windows 控制台用 UTF-8 输出
#[cfg(windows)]
fn setup_console_utf8() {
    // SAFETY: 只调用标准 Win32 API，无其他副作用
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleOutputCP(65001);
    }
}

/// 公共库能表达的开关表。`/FI`、`/PID` 支持多值，在主流程前预提取。
const FLAGS: &[Flag] = &[
    Flag::new("IM", Kind::Value),
    Flag::new("F", Kind::Flag),
    Flag::new("T", Kind::Flag),
    Flag::new("S", Kind::Value),
    Flag::new("U", Kind::Value),
    Flag::new("P", Kind::Value),
    Flag::new("?", Kind::Flag),
];

/// 筛选器字段（跨平台有意义的两个）
enum Field {
    ImageName,
    Pid,
}

enum Op {
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
}

struct Filter {
    field: Field,
    op: Op,
    value: String,
}

/// 解析 `/FI "imagename eq xxx"` 形式的筛选器；无法识别返回 None。
fn parse_filter(s: &str) -> Option<Filter> {
    let mut it = s.split_whitespace();
    let field = it.next()?.to_ascii_uppercase();
    let op = it.next()?.to_ascii_lowercase();
    let value = it.collect::<Vec<_>>().join(" ");
    if value.is_empty() {
        return None;
    }
    let field = match field.as_str() {
        "IMAGENAME" => Field::ImageName,
        "PID" => Field::Pid,
        _ => return None,
    };
    let op = match op.as_str() {
        "eq" => Op::Eq,
        "ne" => Op::Ne,
        "gt" => Op::Gt,
        "lt" => Op::Lt,
        "ge" => Op::Ge,
        "le" => Op::Le,
        _ => return None,
    };
    // 原版 IMAGENAME 只支持 eq/ne
    if matches!(field, Field::ImageName) && !matches!(op, Op::Eq | Op::Ne) {
        return None;
    }
    Some(Filter { field, op, value })
}

fn filter_matches(f: &Filter, proc: &sysinfo::Process) -> bool {
    match f.field {
        Field::ImageName => {
            let name = proc.name().to_string_lossy();
            let m = wild_match(&f.value, &name);
            match f.op {
                Op::Eq => m,
                Op::Ne => !m,
                _ => false,
            }
        }
        Field::Pid => {
            let pid = proc.pid().as_u32();
            let v: u32 = match f.value.trim().parse() {
                Ok(v) => v,
                Err(_) => return false,
            };
            match f.op {
                Op::Eq => pid == v,
                Op::Ne => pid != v,
                Op::Gt => pid > v,
                Op::Lt => pid < v,
                Op::Ge => pid >= v,
                Op::Le => pid <= v,
            }
        }
    }
}

fn matches_all(proc: &sysinfo::Process, filters: &[Filter]) -> bool {
    filters.iter().all(|f| filter_matches(f, proc))
}

/// 通配符匹配（`*` `?`），大小写不敏感（Windows 风格）。
fn wild_match(pattern: &str, name: &str) -> bool {
    let p = pattern.to_lowercase().into_bytes();
    let n = name.to_lowercase().into_bytes();
    wild_bytes(&p, &n)
}

fn wild_bytes(p: &[u8], n: &[u8]) -> bool {
    if p.is_empty() && n.is_empty() {
        return true;
    }
    if let Some(&c) = p.first() {
        if c == b'*' {
            return wild_bytes(&p[1..], n) || (!n.is_empty() && wild_bytes(p, &n[1..]));
        }
    }
    if let (Some(&c), Some(&nc)) = (p.first(), n.first()) {
        if c == b'?' || c == nc {
            return wild_bytes(&p[1..], &n[1..]);
        }
    }
    false
}

/// 对单个 PID 执行终止，返回是否"至少有一个进程被终止/已消失"。
/// `im_name` 为 Some 时走 /IM 输出格式（映像名大写）。
fn kill_one(
    pid: u32,
    force: bool,
    im_name: Option<&str>,
    i18n: &L10n,
) -> bool {
    let signal = if force { "SIGKILL" } else { "SIGTERM" };
    let config = Config {
        signal: signal.to_string(),
        include_target: true,
    };
    match kill_tree_with_config(pid, &config) {
        Ok(outputs) => {
            for out in &outputs {
                match out {
                    Output::Killed {
                        process_id,
                        parent_process_id,
                        name,
                    } => {
                        let mut a = FluentArgs::new();
                        a.set("pid", *process_id);
                        match im_name {
                            Some(_) => {
                                let up = name.to_string().to_uppercase();
                                a.set("name", &up);
                                println!("{}", i18n.tr("success-im", Some(&a)));
                            }
                            None => {
                                // kill_tree 递归杀树：非目标进程按子进程输出
                                let parent = *parent_process_id;
                                if *process_id == pid {
                                    println!("{}", i18n.tr("success-pid", Some(&a)));
                                } else {
                                    a.set("parent", parent);
                                    println!("{}", i18n.tr("success-pid-child", Some(&a)));
                                }
                            }
                        }
                    }
                    Output::MaybeAlreadyTerminated { .. } => {
                        // 竞态：查时还在、杀时已退出，视为成功但不输出
                    }
                }
            }
            true
        }
        Err(_) => {
            let mut a = FluentArgs::new();
            a.set("pid", pid);
            eprintln!("{}", i18n.tr("cannot-terminate", Some(&a)));
            if !force {
                eprintln!("{}", i18n.tr("reason-force-only", None));
            }
            false
        }
    }
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

    #[cfg(windows)]
    setup_console_utf8();

    let raw: Vec<String> = std::env::args().skip(1).collect();

    if raw.iter().any(|a| a == "/?" || a == "-?") {
        println!("{}", i18n.help());
        return ExitCode::SUCCESS;
    }

    // 预提取支持多值的 /FI、/PID（原版可重复出现）
    let mut filters: Vec<String> = Vec::new();
    let mut pids: Vec<String> = Vec::new();
    let mut rest: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < raw.len() {
        let up = raw[i].to_ascii_uppercase();
        if up == "/FI" || up == "-FI" {
            i += 1;
            if i < raw.len() {
                filters.push(raw[i].clone());
            } else {
                eprintln!("{}", i18n.tr("missing-value", args_flag("fi").as_ref()));
                eprintln!("{}", i18n.tr("usage-hint", None));
                return ExitCode::from(1);
            }
        } else if up == "/PID" || up == "-PID" {
            i += 1;
            if i < raw.len() {
                pids.push(raw[i].clone());
            } else {
                eprintln!("{}", i18n.tr("missing-value", args_flag("pid").as_ref()));
                eprintln!("{}", i18n.tr("usage-hint", None));
                return ExitCode::from(1);
            }
        } else {
            rest.push(raw[i].clone());
        }
        i += 1;
    }

    let parsed = match parse(&rest, FLAGS, Unknown::Error) {
        Ok(p) => p,
        Err(Error::Unknown(a)) => {
            let mut fa = FluentArgs::new();
            fa.set("arg", a);
            eprintln!("{}", i18n.tr("invalid-option", Some(&fa)));
            eprintln!("{}", i18n.tr("usage-hint", None));
            return ExitCode::from(1);
        }
        Err(Error::MissingValue(a) | Error::UnexpectedValue(a)) => {
            let flag = a.trim_start_matches(['/', '-']).to_lowercase();
            eprintln!("{}", i18n.tr("missing-value", args_flag(&flag).as_ref()));
            eprintln!("{}", i18n.tr("usage-hint", None));
            return ExitCode::from(1);
        }
    };

    // 远程系统参数：明确报不支持
    if parsed.flags.contains_key("S")
        || parsed.flags.contains_key("U")
        || parsed.flags.contains_key("P")
    {
        eprintln!("{}", i18n.tr("unsupported-remote", None));
        return ExitCode::from(1);
    }

    let force = parsed.flags.contains_key("F");
    let im: Option<&str> = parsed.flags.get("IM").and_then(|v| *v);

    // /PID 与 /IM 互斥（实测原版报错）
    if !pids.is_empty() && im.is_some() {
        eprintln!("{}", i18n.tr("syntax-pid-im", None));
        eprintln!("{}", i18n.tr("usage-hint", None));
        return ExitCode::from(1);
    }

    // 无目标
    if pids.is_empty() && im.is_none() && filters.is_empty() {
        eprintln!("{}", i18n.tr("syntax-no-target", None));
        eprintln!("{}", i18n.tr("usage-hint", None));
        return ExitCode::from(1);
    }

    // 解析筛选器；无法识别 → 原版报错后跟一个空行
    let parsed_filters: Vec<Filter> = match filters
        .iter()
        .map(|f| parse_filter(f).ok_or(()))
        .collect::<Result<Vec<_>, ()>>()
    {
        Ok(v) => v,
        Err(_) => {
            eprintln!("{}", i18n.tr("filter-invalid", None));
            eprintln!();
            return ExitCode::from(1);
        }
    };

    let mut sys = System::new_all();
    sys.refresh_all();

    // 收集要杀的目标 PID 列表 + 是否 /IM 模式
    let mut targets: Vec<u32> = Vec::new();
    let mut im_mode: Option<&str> = None;

    if !pids.is_empty() {
        for pid_str in &pids {
            match pid_str.trim().parse::<u32>() {
                Ok(v) => targets.push(v),
                Err(_) => {
                    eprintln!("{}", i18n.tr("not-found", args_target(pid_str).as_ref()));
                    return ExitCode::from(128);
                }
            }
        }
    } else if let Some(im_pattern) = im {
        im_mode = Some(im_pattern);
        let matched: Vec<u32> = sys
            .processes()
            .values()
            .filter(|p| {
                wild_match(im_pattern, &p.name().to_string_lossy())
                    && matches_all(p, &parsed_filters)
            })
            .map(|p| p.pid().as_u32())
            .collect();
        if matched.is_empty() {
            if parsed_filters.is_empty() {
                eprintln!("{}", i18n.tr("not-found", args_target(im_pattern).as_ref()));
                return ExitCode::from(128);
            } else {
                println!("{}", i18n.tr("info-no-tasks", None));
                return ExitCode::SUCCESS;
            }
        }
        targets = matched;
    } else {
        // 仅 /FI
        let matched: Vec<u32> = sys
            .processes()
            .values()
            .filter(|p| matches_all(p, &parsed_filters))
            .map(|p| p.pid().as_u32())
            .collect();
        if matched.is_empty() {
            println!("{}", i18n.tr("info-no-tasks", None));
            return ExitCode::SUCCESS;
        }
        targets = matched;
    }

    // 逐个终止
    let mut any_ok = false;
    let mut all_not_found = true;
    for pid in &targets {
        // 目标必须存在（/IM 匹配出的必然存在；/PID 可能不存在）
        let exists = sys.process(sysinfo::Pid::from_u32(*pid)).is_some();
        if !exists {
            if im_mode.is_none() {
                eprintln!("{}", i18n.tr("not-found", args_target(&pid.to_string()).as_ref()));
            }
            continue;
        }
        all_not_found = false;
        let ok = kill_one(*pid, force, im_mode, &i18n);
        any_ok |= ok;
    }

    if any_ok {
        ExitCode::SUCCESS
    } else if all_not_found && !targets.is_empty() {
        ExitCode::from(128)
    } else {
        ExitCode::from(1)
    }
}

fn args_flag(flag: &str) -> Option<FluentArgs<'_>> {
    let mut a = FluentArgs::new();
    a.set("flag", flag);
    Some(a)
}

fn args_target(target: &str) -> Option<FluentArgs<'_>> {
    let mut a = FluentArgs::new();
    a.set("target", target);
    Some(a)
}
