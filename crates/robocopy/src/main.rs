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
mod sink;
mod time;
mod util;

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use windowshit_args::{parse, Parsed, Unknown};

use crate::flags::{Options, Stats, FLAGS};
use crate::report::{print_header_with, print_help, print_summary, print_usage};
use crate::util::{absolutize, display_dir};

/// 解析 `/IA:xx` `/XA:xx` 属性字母串。
fn attr_chars(v: &str) -> Vec<char> {
    v.to_ascii_uppercase().chars().collect()
}

/// 解析参数，构造 Options。
fn build_opts(parsed: &Parsed, mt: Option<usize>, xf: Vec<String>, xd: Vec<String>) -> Options {
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
    let num = |key: &str| {
        parsed
            .flags
            .get(key)
            .and_then(|v| *v)
            .and_then(|v| v.parse::<u64>().ok())
    };
    Options {
        subdirs_nonempty: s || e || mir,
        subdirs_all: e || mir,
        mirror: mir,
        purge: purge || mir,
        move_files: parsed.flags.contains_key("MOV"),
        move_all: parsed.flags.contains_key("MOVE"),
        restartable: parsed.flags.contains_key("Z"),
        create_only: parsed.flags.contains_key("CREATE"),
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
        xf,
        xd,
        exclude_changed: parsed.flags.contains_key("XC"),
        exclude_newer: parsed.flags.contains_key("XN"),
        exclude_older: parsed.flags.contains_key("XO"),
        exclude_lonely: parsed.flags.contains_key("XL"),
        exclude_extra: parsed.flags.contains_key("XX"),
        max_size: num("MAX"),
        min_size: num("MIN"),
        max_age: num("MAXAGE"),
        min_age: num("MINAGE"),
        max_lad: num("MAXLAD"),
        min_lad: num("MINLAD"),
        archive: parsed.flags.contains_key("A"),
        archive_move: parsed.flags.contains_key("M"),
        include_attrs: parsed
            .flags
            .get("IA")
            .and_then(|v| *v)
            .map(attr_chars)
            .unwrap_or_default(),
        exclude_attrs: parsed
            .flags
            .get("XA")
            .and_then(|v| *v)
            .map(attr_chars)
            .unwrap_or_default(),
        lev: parsed
            .flags
            .get("LEV")
            .and_then(|v| *v)
            .and_then(|v| v.parse::<u32>().ok()),
        exclude_junction: parsed.flags.contains_key("XJ"),
        exclude_junction_file: parsed.flags.contains_key("XJF"),
        exclude_junction_dir: parsed.flags.contains_key("XJD"),
        show_ts: parsed.flags.contains_key("TS"),
        full_path: parsed.flags.contains_key("FP"),
        show_bytes: parsed.flags.contains_key("BYTES"),
        eta: parsed.flags.contains_key("ETA"),
        // /UNILOG /UNILOG+ 优先于 /LOG /LOG+（Unicode 日志）
        log_path: parsed
            .flags
            .get("UNILOG")
            .or_else(|| parsed.flags.get("UNILOG+"))
            .or_else(|| parsed.flags.get("LOG"))
            .or_else(|| parsed.flags.get("LOG+"))
            .and_then(|v| *v)
            .map(|s| PathBuf::from(s)),
        log_append: parsed.flags.contains_key("LOG+") || parsed.flags.contains_key("UNILOG+"),
        log_unicode: parsed.flags.contains_key("UNILOG") || parsed.flags.contains_key("UNILOG+"),
        tee: parsed.flags.contains_key("TEE"),
        fft: parsed.flags.contains_key("FFT"),
    }
}

/// 预提取 `/XF` `/XD` 多值开关（值取到下一个开关为止），并返回其余参数。
fn preextract_multi(raw: &[String]) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut xf: Vec<String> = Vec::new();
    let mut xd: Vec<String> = Vec::new();
    let mut rest: Vec<String> = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let a = &raw[i];
        let up = a
            .strip_prefix('/')
            .or_else(|| a.strip_prefix('-'))
            .map(|s| s.to_ascii_uppercase());
        match up.as_deref() {
            Some(u) if u == "XF" || u.starts_with("XF:") => {
                if let Some(v) = u.strip_prefix("XF:") {
                    if !v.is_empty() {
                        xf.push(v.to_string());
                    }
                }
                i += 1;
                while i < raw.len() {
                    let b = &raw[i];
                    if b.starts_with('/') || b.starts_with('-') {
                        break;
                    }
                    xf.push(b.clone());
                    i += 1;
                }
                continue;
            }
            Some(u) if u == "XD" || u.starts_with("XD:") => {
                if let Some(v) = u.strip_prefix("XD:") {
                    if !v.is_empty() {
                        xd.push(v.to_string());
                    }
                }
                i += 1;
                while i < raw.len() {
                    let b = &raw[i];
                    if b.starts_with('/') || b.starts_with('-') {
                        break;
                    }
                    xd.push(b.clone());
                    i += 1;
                }
                continue;
            }
            _ => {
                rest.push(a.clone());
            }
        }
        i += 1;
    }
    (xf, xd, rest)
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

    // 预提取 /XF /XD 多值
    let (xf, xd, rest) = preextract_multi(&raw);

    // robocopy 对未知开关静默忽略
    let parsed = match parse(&rest, FLAGS, Unknown::Ignore) {
        Ok(p) => p,
        Err(_) => Parsed::default(),
    };

    let mut opts = build_opts(&parsed, mt, xf, xd);

    // 初始化日志输出目标（/LOG /LOG+ /UNILOG /UNILOG+ /TEE）
    if let Some(log_path) = &opts.log_path {
        sink::init(Some((log_path.clone(), opts.log_append)), opts.tee, opts.log_unicode);
        let abs = fs::canonicalize(log_path).unwrap_or_else(|_| log_path.clone());
        let path_str = abs.to_string_lossy().replace('/', "\\");
        sink::announce_log_file(&path_str);
    } else {
        sink::init(None, false, opts.log_unicode);
    }

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
    // 注意：/MT 模式下 Dirs 的 Copied 恒等于 Total（原版统计怪癖），根目录由 walk 统一计数。
    let dst_existed = dst.exists();
    if !dst_existed {
        if !opts.list_only {
            if fs::create_dir_all(&dst).is_ok() {
                if opts.mt.is_none() {
                    stats.dir(flags::COP, 1);
                }
            } else {
                stats.dir(flags::FAI, 1);
                rc |= 8;
            }
        } else if opts.mt.is_none() {
            // /L：不实际创建，但目标不存在仍计入 Copied（原版实测）
            stats.dir(flags::COP, 1);
        }
    }

    // /MT 多线程池（目录内文件并行复制，输出保持有序）
    let pool = opts.mt.map(|_| copy::Pool::new(&opts));

    let mut eta_state = copy::EtaState {
        copied: 0,
        bytes: 0,
        start: Instant::now(),
    };
    copy::walk(
        &src,
        &dst,
        &opts,
        &mut stats,
        &mut rc,
        !dst_existed,
        1,
        if opts.eta { Some(&mut eta_state) } else { None },
        pool.as_ref(),
    );

    if !opts.no_job_summary {
        crate::outln!("\r\n------------------------------------------------------------------------------\r\n");
        print_summary(&stats, start, opts.mt.is_some());
    }

    ExitCode::from(rc as u8)
}
