//! robocopy —— 复刻 Windows robocopy.exe（Robust File Copy，跨平台整活）。
//!
//! 阶段 1 核心功能：
//! - 目录树遍历，/S（非空子目录）/E（含空子目录）
//! - /MIR（= /E + /PURGE）、/PURGE（删目标多余文件/目录）
//! - /MOV（复制后删源文件）、/MOVE（移动文件+目录）
//! - /L 仅列出不实际操作
//! - 文件分类（New / Newer / Older / Same / Tweaked / Changed / Extra / Mismatch）
//! - 状态行输出、统计表格、位掩码退出码

mod copy;
mod flags;
mod report;
mod time;
mod util;

use std::env;
use std::fs;
use std::process::ExitCode;
use std::time::Instant;

use windowshit_args::{parse, Parsed, Unknown};

use crate::flags::{Options, Stats, FLAGS};
use crate::report::{print_header_with, print_help, print_summary, print_usage};
use crate::util::{absolutize, display_dir};

/// 解析参数，构造 Options。
fn build_opts(parsed: &Parsed, mt: Option<usize>) -> Options {
    let s = parsed.flags.contains_key("S");
    let e = parsed.flags.contains_key("E");
    let mir = parsed.flags.contains_key("MIR");
    let purge = parsed.flags.contains_key("PURGE");
    let retries = parsed
        .flags
        .get("R")
        .and_then(|v| *v)
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(1_000_000);
    let wait = parsed
        .flags
        .get("W")
        .and_then(|v| *v)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30);
    Options {
        subdirs_nonempty: s || e || mir,
        subdirs_all: e || mir,
        mirror: mir,
        purge: purge || mir,
        move_files: parsed.flags.contains_key("MOV"),
        move_all: parsed.flags.contains_key("MOVE"),
        list_only: parsed.flags.contains_key("L"),
        verbose: parsed.flags.contains_key("V"),
        report_extra: parsed.flags.contains_key("X"),
        retries,
        wait: std::time::Duration::from_secs(wait),
        mt,
        files: Vec::new(),
        no_progress: parsed.flags.contains_key("NP"),
        no_file_list: parsed.flags.contains_key("NFL"),
        no_dir_list: parsed.flags.contains_key("NDL"),
        no_size: parsed.flags.contains_key("NS"),
        no_class: parsed.flags.contains_key("NC"),
        no_job_header: parsed.flags.contains_key("NJH"),
        no_job_summary: parsed.flags.contains_key("NJS"),
        include_same: parsed.flags.contains_key("IS"),
        include_tweaked: parsed.flags.contains_key("IT"),
    }
}

/// 源目录不可访问错误行（对齐原版）。
fn access_error(src: &std::path::Path) -> (String, String) {
    let msg = format!(
        "{} ERROR 2 (0x00000002) Accessing Source Directory {}",
        time::fmt_now_num(),
        display_dir(src)
    );
    let code = if cfg!(windows) {
        "The system cannot find the file specified.".to_string()
    } else {
        "No such file or directory.".to_string()
    };
    (msg, code)
}

fn main() -> ExitCode {
    let raw: Vec<String> = env::args().skip(1).collect();

    if raw.iter().any(|a| a == "/?" || a == "-?") {
        print_help();
        // 原版 /? 也返回 16（严重错误）
        return ExitCode::from(16u8);
    }

    // 预提取 /MT 或 /MT:n（可选值形态，公共库不支持）
    let mut mt: Option<usize> = None;
    let mut rest: Vec<String> = Vec::new();
    for a in &raw {
        if let Some(up) = a.strip_prefix('/').or_else(|| a.strip_prefix('-')) {
            if let Some(n) = up.strip_prefix("MT") {
                if n.is_empty() {
                    mt = Some(8);
                    continue;
                } else if let Some(v) = n.strip_prefix(':') {
                    if let Ok(x) = v.parse::<usize>() {
                        mt = Some(x.clamp(1, 128));
                        continue;
                    }
                }
            }
        }
        rest.push(a.clone());
    }

    // robocopy 对未知开关静默忽略
    let parsed = match parse(&rest, FLAGS, Unknown::Ignore) {
        Ok(p) => p,
        Err(_) => Parsed::default(),
    };

    let mut opts = build_opts(&parsed, mt);

    // 位置参数：source destination [file...]
    let mut paths = parsed.paths.iter();
    let src_raw = paths.next();
    let dst_raw = paths.next();
    let files: Vec<String> = paths.map(|s| s.to_string()).collect();
    opts.files = files.clone();

    // 完全无参数：Header + Simple Usage
    if raw.is_empty() {
        crate::out!("\r\n");
        print_header_with(None, None, &[], &opts, true);
        print_usage();
        return ExitCode::from(16u8);
    }

    // 有参数但缺 source
    if src_raw.is_none() {
        crate::out!("\r\n");
        if !opts.no_job_header {
            print_header_with(None, None, &files, &opts, false);
            crate::outln!("\r\n------------------------------------------------------------------------------\r\n");
        }
        crate::outln!("ERROR : No Source Directory Specified.\r\n");
        print_usage();
        return ExitCode::from(16u8);
    }

    let src = absolutize(src_raw.unwrap());

    // 缺 destination
    if dst_raw.is_none() {
        if !opts.no_job_header {
            crate::out!("\r\n");
            print_header_with(Some(&src), None, &files, &opts, false);
            crate::outln!("\r\n------------------------------------------------------------------------------\r\n");
        }
        crate::outln!("ERROR : No Destination Directory Specified.\r\n");
        print_usage();
        return ExitCode::from(16u8);
    }

    let dst = absolutize(dst_raw.unwrap());

    // 正常流程
    if !opts.no_job_header {
        crate::out!("\r\n");
        print_header_with(Some(&src), Some(&dst), &files, &opts, false);
        crate::outln!("\r\n------------------------------------------------------------------------------\r\n");
    }

    // 源目录不可访问
    if !src.is_dir() {
        let (msg, code) = access_error(&src);
        crate::outln!("{msg}");
        crate::outln!("{code}\r");
        return ExitCode::from(16u8);
    }

    let mut stats = Stats::default();
    let mut rc: u32 = 0;
    let start = Instant::now();

    // 目标根目录不存在时先创建（原版自动创建）
    let dst_existed = dst.exists();
    if !opts.list_only && !dst_existed {
        if fs::create_dir_all(&dst).is_ok() {
            stats.dir(flags::COP, 1);
        } else {
            stats.dir(flags::FAI, 1);
            rc |= 8;
        }
    }

    copy::walk(&src, &dst, &opts, &mut stats, &mut rc, !dst_existed);

    if !opts.no_job_summary {
        crate::outln!("\r\n------------------------------------------------------------------------------\r\n");
        print_summary(&stats, start);
    }

    ExitCode::from(rc as u8)
}
