//! robocopy —— 复刻 Windows robocopy.exe（Robust File Copy，跨平台整活）。
//!
//! 阶段 1 核心功能：
//! - 目录树遍历，/S（非空子目录）/E（含空子目录）
//! - /MIR（= /E + /PURGE）、/PURGE（删目标多余文件/目录）
//! - /MOV（复制后删源文件）、/MOVE（移动文件+目录）
//! - /L 仅列出不实际操作
//! - 文件分类（New / Newer / Older / Same / Tweaked / Changed / Extra / Mismatch）
//! - 状态行输出、统计表格、位掩码退出码
//!
//! 实测对齐的行为（Windows 11 原版，2026-08-11）：
//! - 无参数：Header + Simple Usage，退出码 16
//! - 缺 Destination：Header + `ERROR : No Destination Directory Specified.` + Simple Usage，退出码 16
//! - 源不存在：`2026/08/11 20:36:32 ERROR 2 (0x00000002) Accessing Source Directory ...`，退出码 16
//! - 目录状态行：`\t  New Dir          2\t<path>`（数字 = 该目录下复制的文件数）
//! - 文件状态行：`\t    New File  \t\t      10\ta.txt`
//! - Options 行按固定顺序回显，/MT 仅显式传入时回显，末尾默认 `/R:1000000 /W:30`
//! - 输出恒为英文（不随系统语言变），Started/Ended 时间随 locale（暂按 UTC）

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use windowshit_args::{parse, Flag, Kind, Parsed, Unknown};

const HELP: &str = include_str!("../help.txt");

const USAGE_HEAD: &str = "       Simple Usage :: ROBOCOPY source destination /MIR\n\n             source :: Source Directory (drive:\\path or \\\\server\\share\\path).\n        destination :: Destination Dir  (drive:\\path or \\\\server\\share\\path).\n               /MIR :: Mirror a complete directory tree.\n\n    For more usage information run ROBOCOPY /?\n\n";

/// 开关表：阶段 1 实现的收 `Flag`/`Value`，暂不实现的收 `Ignore`（原版存在但本实现不处理）。
const FLAGS: &[Flag] = &[
    // —— 复制类 ——
    Flag::new("S", Kind::Flag),
    Flag::new("E", Kind::Flag),
    Flag::new("LEV", Kind::Value),
    Flag::new("PURGE", Kind::Flag),
    Flag::new("MIR", Kind::Flag),
    Flag::new("MOV", Kind::Flag),
    Flag::new("MOVE", Kind::Flag),
    Flag::new("CREATE", Kind::Flag),
    Flag::new("L", Kind::Flag),
    Flag::new("COPY", Kind::Value),
    Flag::new("DCOPY", Kind::Value),
    // —— 重试 ——
    Flag::new("R", Kind::Value),
    Flag::new("W", Kind::Value),
    Flag::new("REG", Kind::Ignore),
    Flag::new("TBD", Kind::Ignore),
    Flag::new("LFSM", Kind::Ignore),
    // —— 日志类 ——
    Flag::new("NP", Kind::Flag),
    Flag::new("NFL", Kind::Flag),
    Flag::new("NDL", Kind::Flag),
    Flag::new("NS", Kind::Flag),
    Flag::new("NC", Kind::Flag),
    Flag::new("NJH", Kind::Flag),
    Flag::new("NJS", Kind::Flag),
    Flag::new("V", Kind::Flag),
    Flag::new("X", Kind::Flag),
    Flag::new("TS", Kind::Flag),
    Flag::new("FP", Kind::Flag),
    Flag::new("BYTES", Kind::Flag),
    Flag::new("ETA", Kind::Flag),
    Flag::new("TEE", Kind::Flag),
    Flag::new("LOG", Kind::Value),
    Flag::new("LOG+", Kind::Value),
    Flag::new("UNILOG", Kind::Value),
    Flag::new("UNILOG+", Kind::Value),
    Flag::new("UNICODE", Kind::Ignore),
    // —— 文件选择（阶段 2）——
    Flag::new("A", Kind::Flag),
    Flag::new("M", Kind::Flag),
    Flag::new("IA", Kind::Value),
    Flag::new("XA", Kind::Value),
    Flag::new("XF", Kind::Value),
    Flag::new("XD", Kind::Value),
    Flag::new("XC", Kind::Flag),
    Flag::new("XN", Kind::Flag),
    Flag::new("XO", Kind::Flag),
    Flag::new("XX", Kind::Flag),
    Flag::new("XL", Kind::Flag),
    Flag::new("IS", Kind::Flag),
    Flag::new("IT", Kind::Flag),
    Flag::new("IM", Kind::Flag),
    Flag::new("MAX", Kind::Value),
    Flag::new("MIN", Kind::Value),
    Flag::new("MAXAGE", Kind::Value),
    Flag::new("MINAGE", Kind::Value),
    Flag::new("MAXLAD", Kind::Value),
    Flag::new("MINLAD", Kind::Value),
    Flag::new("FFT", Kind::Ignore),
    Flag::new("DST", Kind::Ignore),
    Flag::new("XJ", Kind::Ignore),
    Flag::new("XJD", Kind::Ignore),
    Flag::new("XJF", Kind::Ignore),
    // —— Windows 专属 / 暂不实现 ——
    Flag::new("Z", Kind::Ignore),
    Flag::new("B", Kind::Ignore),
    Flag::new("ZB", Kind::Ignore),
    Flag::new("J", Kind::Ignore),
    Flag::new("EFSRAW", Kind::Ignore),
    Flag::new("FAT", Kind::Ignore),
    Flag::new("256", Kind::Ignore),
    Flag::new("SEC", Kind::Ignore),
    Flag::new("COPYALL", Kind::Ignore),
    Flag::new("NOCOPY", Kind::Ignore),
    Flag::new("SECFIX", Kind::Ignore),
    Flag::new("TIMFIX", Kind::Ignore),
    Flag::new("NODCOPY", Kind::Ignore),
    Flag::new("MON", Kind::Ignore),
    Flag::new("MOT", Kind::Ignore),
    Flag::new("RH", Kind::Ignore),
    Flag::new("PF", Kind::Ignore),
    Flag::new("IPG", Kind::Ignore),
    Flag::new("SJ", Kind::Ignore),
    Flag::new("SL", Kind::Ignore),
    Flag::new("IOMAXSIZE", Kind::Ignore),
    Flag::new("IORATE", Kind::Ignore),
    Flag::new("THRESHOLD", Kind::Ignore),
    Flag::new("NOOFFLOAD", Kind::Ignore),
    Flag::new("COMPRESS", Kind::Ignore),
    Flag::new("SPARSE", Kind::Ignore),
    Flag::new("A+", Kind::Ignore),
    Flag::new("A-", Kind::Ignore),
    // —— Job ——
    Flag::new("JOB", Kind::Ignore),
    Flag::new("SAVE", Kind::Ignore),
    Flag::new("QUIT", Kind::Ignore),
    Flag::new("NOSD", Kind::Ignore),
    Flag::new("NODD", Kind::Ignore),
    Flag::new("IF", Kind::Ignore),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    New,
    Newer,
    Older,
    Same,
    Tweaked,
    Changed,
    Extra,
    Mismatch,
}

impl Class {
    /// 状态行分类标签（原版固定格式）。
    fn label(self) -> &'static str {
        match self {
            Class::New => "New File",
            Class::Newer => "Newer",
            Class::Older => "Older",
            Class::Same => "Same",
            Class::Tweaked => "Tweaked",
            Class::Changed => "Changed",
            Class::Extra => "*EXTRA File",
            Class::Mismatch => "*MISMATCH File",
        }
    }
}

struct Options {
    subdirs_nonempty: bool, // /S
    subdirs_all: bool,      // /E
    mirror: bool,           // /MIR
    purge: bool,            // /PURGE（含 /MIR 隐含）
    move_files: bool,       // /MOV
    move_all: bool,         // /MOVE
    list_only: bool,        // /L
    verbose: bool,          // /V
    report_extra: bool,     // /X
    retries: u32,           // /R:n（默认 1_000_000）
    wait: Duration,         // /W:n（默认 30s）
    mt: Option<usize>,      // /MT[:n]（阶段 1 仅回显，未做多线程）
    files: Vec<String>,     // 文件模式（默认匹配所有）
    // 输出控制
    no_progress: bool,   // /NP
    no_file_list: bool,  // /NFL
    no_dir_list: bool,   // /NDL
    no_size: bool,       // /NS
    no_class: bool,      // /NC
    no_job_header: bool, // /NJH
    no_job_summary: bool, // /NJS
}

/// 统计列索引。
const TOT: usize = 0; // Total
const COP: usize = 1; // Copied
const MIS: usize = 3; // Mismatch
const FAI: usize = 4; // FAILED
const EXT: usize = 5; // Extras

#[derive(Default)]
struct Stats {
    dirs: [u64; 6],
    files: [u64; 6],
    bytes: [u64; 6],
}

impl Stats {
    fn dir(&mut self, col: usize, n: u64) {
        self.dirs[col] += n;
    }
    fn file(&mut self, col: usize, n: u64, bytes: u64) {
        self.files[col] += n;
        self.bytes[col] += bytes;
    }
}

fn main() -> ExitCode {
    let raw: Vec<String> = env::args().skip(1).collect();

    if raw.iter().any(|a| a == "/?" || a == "-?") {
        print!("\r\n{HELP}");
        // 原版 /? 也返回 16（严重错误）
        return ExitCode::from(16);
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

    if src_raw.is_none() {
        // 无参数：Header + Simple Usage
        if !opts.no_job_header {
            print!("\r\n");
            print_header_with(None, None, &[], &opts, true);
        }
        print_usage();
        return ExitCode::from(16);
    }

    let src = absolutize(src_raw.unwrap());

    // 缺 destination
    if dst_raw.is_none() {
        if !opts.no_job_header {
            print!("\r\n");
            print_header_with(Some(&src), None, &files, &opts, false);
            println!("\r\n------------------------------------------------------------------------------\r\n");
        }
        println!("ERROR : No Destination Directory Specified.\r\n");
        print_usage();
        return ExitCode::from(16);
    }

    let dst = absolutize(dst_raw.unwrap());

    // 正常流程
    if !opts.no_job_header {
        print!("\r\n");
        print_header_with(Some(&src), Some(&dst), &files, &opts, false);
        println!("\r\n------------------------------------------------------------------------------\r\n");
    }

    // 源目录不可访问
    if !src.is_dir() {
        let (msg, code) = access_error(&src);
        println!("{msg}");
        println!("{code}");
        println!();
        return ExitCode::from(16);
    }

    let mut stats = Stats::default();
    let mut rc: u32 = 0;
    let start = Instant::now();

    // 目标根目录不存在时先创建（原版自动创建）
    let dst_existed = dst.exists();
    if !opts.list_only && !dst_existed {
        if fs::create_dir_all(&dst).is_ok() {
            stats.dir(COP, 1);
        } else {
            stats.dir(FAI, 1);
            rc |= 8;
        }
    }

    walk(&src, &dst, &opts, &mut stats, &mut rc, !dst_existed);

    if !opts.no_job_summary {
        println!("\r\n------------------------------------------------------------------------------\r\n");
        print_summary(&stats, start);
    }

    ExitCode::from(rc as u8)
}

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
        wait: Duration::from_secs(wait),
        mt,
        files: Vec::new(),
        no_progress: parsed.flags.contains_key("NP"),
        no_file_list: parsed.flags.contains_key("NFL"),
        no_dir_list: parsed.flags.contains_key("NDL"),
        no_size: parsed.flags.contains_key("NS"),
        no_class: parsed.flags.contains_key("NC"),
        no_job_header: parsed.flags.contains_key("NJH"),
        no_job_summary: parsed.flags.contains_key("NJS"),
    }
}

// ---------------------------------------------------------------------------
// Header / Options 回显
// ---------------------------------------------------------------------------

fn print_header_with(
    src: Option<&PathBuf>,
    dst: Option<&PathBuf>,
    files: &[String],
    opts: &Options,
    simple: bool,
) {
    println!("-------------------------------------------------------------------------------");
    println!("{:<81}", "   ROBOCOPY     ::     Robust File Copy for Windows");
    println!("-------------------------------------------------------------------------------");
    println!("\r\n  Started : {}", fmt_now_cn());
    if !simple {
        if let Some(s) = src {
            println!("{:>11} {}", "Source :", display_dir(s));
        }
        match dst {
            Some(d) => println!("{:>11} {}", "Dest :", display_dir(d)),
            None => println!("{:>11} ", "Dest -"),
        }
        println!("\r\n{:>11} {}", "Files :", files_mode(files));
        println!("\t    ");
        println!("{:>11} {} ", "Options :", options_line(files, opts));
    }
}

/// 文件模式回显：默认 `*.*`。
fn files_mode(files: &[String]) -> String {
    if files.is_empty() {
        "*.*".to_string()
    } else {
        files.join(" ")
    }
}

/// Options 行回显，固定顺序（实测原版 2026-08-11）。
fn options_line(files: &[String], opts: &Options) -> String {
    let mut v: Vec<String> = vec![files_mode(files)];
    if opts.verbose {
        v.push("/V".into());
    }
    if opts.report_extra {
        v.push("/X".into());
    }
    if opts.no_size {
        v.push("/NS".into());
    }
    if opts.no_class {
        v.push("/NC".into());
    }
    if opts.no_job_summary {
        v.push("/NJS".into());
    }
    if opts.no_job_header {
        v.push("/NJH".into());
    }
    if opts.list_only {
        v.push("/L".into());
    }
    // /S /E：/E 展开为 /S /E
    if opts.subdirs_nonempty {
        v.push("/S".into());
    }
    if opts.subdirs_all {
        v.push("/E".into());
    }
    v.push("/DCOPY:DA".into());
    v.push("/COPY:DAT".into());
    if opts.move_all {
        v.push("/MOVE".into());
    }
    if opts.move_files && !opts.move_all {
        v.push("/MOV".into());
    }
    if opts.purge {
        v.push("/PURGE".into());
    }
    if opts.mirror {
        v.push("/MIR".into());
    }
    if opts.no_progress {
        v.push("/NP".into());
    }
    if let Some(mt) = opts.mt {
        v.push(format!("/MT:{mt}"));
    }
    v.push(format!("/R:{}", opts.retries));
    v.push(format!("/W:{}", opts.wait.as_secs()));
    v.join(" ")
}

fn print_usage() {
    print!("{USAGE_HEAD}");
    println!("{}", " ".repeat(58));
    println!("****  /MIR can DELETE files as well as copy them !");
}

// ---------------------------------------------------------------------------
// 遍历 / 分类 / 复制
// ---------------------------------------------------------------------------

/// 递归遍历 src 目录。`new_dir`：目标目录为本次新建（显示 `New Dir`）。`rc` 累积退出码标志。
fn walk(src: &Path, dst: &Path, opts: &Options, stats: &mut Stats, rc: &mut u32, new_dir: bool) {
    let entries = match fs::read_dir(src) {
        Ok(e) => e,
        Err(_) => {
            stats.dir(FAI, 1);
            *rc |= 8;
            return;
        }
    };

    let mut files: Vec<PathBuf> = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            dirs.push(p);
        } else {
            files.push(p);
        }
    }
    files.sort();
    dirs.sort();

    stats.dir(TOT, 1); // 每个访问的目录计入 Total

    // 第一遍：分类文件，统计本目录匹配的文件数（供目录行数字，含跳过的）
    let mut matched_count: u64 = 0;
    let mut plan: Vec<(PathBuf, Class)> = Vec::new();
    for f in &files {
        let name = f.file_name().unwrap_or_default().to_string_lossy();
        if !matches_pattern(&name, &opts.files) {
            continue;
        }
        matched_count += 1;
        let dst_file = dst.join(f.file_name().unwrap());
        let class = classify(f, &dst_file, opts);
        plan.push((f.clone(), class));
    }

    // 目录状态行
    if !opts.no_dir_list {
        let field = if new_dir {
            dir_class_field("New Dir")
        } else {
            " ".repeat(19)
        };
        println!("\t{field}{matched_count}\t{}", display_dir(src));
    }

    // 处理文件
    for (f, class) in &plan {
        let dst_file = dst.join(f.file_name().unwrap());
        let name = f.file_name().unwrap_or_default().to_string_lossy().to_string();
        let sz = f.metadata().map(|m| m.len()).unwrap_or(0);
        match class_action(*class, opts) {
            Action::Copy => {
                stats.file(TOT, 1, sz);
                if opts.list_only {
                    stats.file(COP, 1, sz);
                    *rc |= 1;
                } else if copy_with_retry(f, &dst_file, opts).is_ok() {
                    stats.file(COP, 1, sz);
                    *rc |= 1;
                } else {
                    stats.file(FAI, 1, sz);
                    *rc |= 8;
                }
                output_file_line(*class, sz, &name, opts);
                if (opts.move_files || opts.move_all) && !opts.list_only {
                    let _ = fs::remove_file(f);
                }
            }
            Action::Skip => {
                stats.file(TOT, 1, sz);
                if opts.verbose {
                    output_file_line(*class, sz, &name, opts);
                }
            }
            Action::Mismatch => {
                stats.file(TOT, 1, sz);
                stats.file(MIS, 1, sz);
                *rc |= 4;
                output_file_line(Class::Mismatch, sz, &name, opts);
            }
        }
    }

    // extra 处理（目标中存在而源中没有的）—— 原版在该目录文件处理完后立即输出
    if let Ok(entries) = fs::read_dir(dst) {
        let mut extra_files: Vec<PathBuf> = Vec::new();
        let mut extra_dirs: Vec<PathBuf> = Vec::new();
        for e in entries.flatten() {
            let p = e.path();
            let name = e.file_name();
            let in_src = files.iter().any(|f| f.file_name() == Some(name.as_os_str()))
                || dirs.iter().any(|d| d.file_name() == Some(name.as_os_str()));
            if in_src {
                continue;
            }
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                extra_dirs.push(p);
            } else {
                extra_files.push(p);
            }
        }
        extra_files.sort();
        extra_dirs.sort();

        if opts.purge {
            for ef in &extra_files {
                let name = ef.file_name().unwrap_or_default().to_string_lossy();
                let sz = ef.metadata().map(|m| m.len()).unwrap_or(0);
                if !opts.list_only && fs::remove_file(ef).is_ok() {
                    stats.file(EXT, 1, sz);
                    *rc |= 2;
                    output_extra_file_line(&name, sz, opts);
                }
            }
            for ed in &extra_dirs {
                if !opts.list_only && remove_dir_all_best(ed) {
                    stats.dir(EXT, 1);
                    *rc |= 2;
                    if !opts.no_dir_list {
                        println!("\t{}\t{}", dir_class_field("*EXTRA Dir"), display_dir(ed));
                    }
                }
            }
        } else if opts.report_extra {
            for ef in &extra_files {
                let name = ef.file_name().unwrap_or_default().to_string_lossy();
                let sz = ef.metadata().map(|m| m.len()).unwrap_or(0);
                output_extra_file_line(&name, sz, opts);
            }
        }
    }

    // 递归子目录
    for d in &dirs {
        let dst_dir = dst.join(d.file_name().unwrap());
        let empty = is_dir_empty(d);
        let need = if opts.subdirs_all {
            true // /E /MIR：含空目录
        } else if opts.subdirs_nonempty && !empty {
            true // /S：非空目录
        } else {
            false
        };
        if !need {
            continue;
        }
        let dst_dir_existed = dst_dir.exists();
        if !opts.list_only && !dst_dir_existed {
            if fs::create_dir_all(&dst_dir).is_ok() {
                stats.dir(COP, 1);
            } else {
                stats.dir(FAI, 1);
                *rc |= 8;
            }
        }
        // /L 不创建目录，但目录在目标中不存在仍算 New Dir
        walk(d, &dst_dir, opts, stats, rc, !dst_dir_existed);
    }
}

/// 文件分类（源文件已存在，dst 路径为目标）。
fn classify(src_file: &Path, dst_file: &Path, opts: &Options) -> Class {
    let _ = opts;
    let dst_meta = match fs::metadata(dst_file) {
        Ok(m) => m,
        Err(_) => return Class::New, // 目标不存在
    };
    if dst_meta.is_dir() {
        return Class::Mismatch; // 源是文件、目标是目录
    }
    let src_meta = match fs::metadata(src_file) {
        Ok(m) => m,
        Err(_) => return Class::New,
    };
    let src_mt = src_meta.modified().ok();
    let dst_mt = dst_meta.modified().ok();

    match (src_mt, dst_mt) {
        (Some(s), Some(d)) if s != d => {
            if s > d {
                Class::Newer
            } else {
                Class::Older
            }
        }
        _ => {
            let src_sz = src_meta.len();
            let dst_sz = dst_meta.len();
            if src_sz != dst_sz {
                Class::Changed
            } else {
                // 属性不同 → Tweaked；阶段 1 仅比较只读位
                let sro = src_meta.permissions().readonly();
                let dro = dst_meta.permissions().readonly();
                if sro != dro {
                    Class::Tweaked
                } else {
                    Class::Same
                }
            }
        }
    }
}

#[derive(PartialEq, Eq)]
enum Action {
    Copy,
    Skip,
    Mismatch,
}

fn class_action(class: Class, _opts: &Options) -> Action {
    match class {
        // 阶段 1：时间戳或大小不同 → 复制（/IT /IS 等包含开关留待阶段 2）
        Class::New | Class::Newer | Class::Older | Class::Changed => Action::Copy,
        Class::Same | Class::Tweaked => Action::Skip,
        Class::Extra | Class::Mismatch => Action::Mismatch,
    }
}

/// 复制文件，带 /R /W 重试。
fn copy_with_retry(src: &Path, dst: &Path, opts: &Options) -> io::Result<()> {
    for i in 0..=opts.retries {
        match fs::copy(src, dst) {
            Ok(_) => return Ok(()),
            Err(_e) if i < opts.retries => thread::sleep(opts.wait),
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}

// ---------------------------------------------------------------------------
// 状态行输出
// ---------------------------------------------------------------------------

/// 文件分类字段：`    New File  `（4 缩进 + 分类左对齐 10 字符）；`*EXTRA File` 特殊（2 缩进）。
fn file_class_field(class: Class) -> String {
    if class == Class::Extra || class == Class::Mismatch {
        format!("  {:<12}", class.label())
    } else {
        format!("    {:<10}", class.label())
    }
}

/// 目录分类字段：`  New Dir          `（2 缩进 + 分类左对齐 17 字符 = 19 宽）。
fn dir_class_field(class: &str) -> String {
    format!("  {class:<17}")
}

fn output_file_line(class: Class, size: u64, name: &str, opts: &Options) {
    if opts.no_file_list {
        return;
    }
    let field = if opts.no_class {
        " ".repeat(14)
    } else {
        file_class_field(class)
    };
    let sz = if opts.no_size {
        String::new()
    } else {
        format!("{:>8}", size)
    };
    let progress = if opts.no_progress { "" } else { "100%" };
    println!("\t{field}\t\t{sz}\t{name}{progress}");
}

fn output_extra_file_line(name: &str, size: u64, opts: &Options) {
    if opts.no_file_list {
        return;
    }
    let field = if opts.no_class {
        " ".repeat(14)
    } else {
        file_class_field(Class::Extra)
    };
    let sz = if opts.no_size {
        String::new()
    } else {
        format!("{:>8}", size)
    };
    println!("\t{field}\t\t{sz}\t{name}");
}

// ---------------------------------------------------------------------------
// 统计表格
// ---------------------------------------------------------------------------

fn print_summary(stats: &Stats, start: Instant) {
    println!(
        "{:>20}{:>10}{:>10}{:>10}{:>10}{:>10}",
        "Total", "Copied", "Skipped", "Mismatch", "FAILED", "Extras"
    );
    let d = stats.dirs;
    let f = stats.files;
    let b = stats.bytes;
    let d_skip = d[TOT].saturating_sub(d[COP] + d[MIS] + d[FAI]);
    let f_skip = f[TOT].saturating_sub(f[COP] + f[MIS] + f[FAI]);
    let b_skip = b[TOT].saturating_sub(b[COP] + b[MIS] + b[FAI]);
    println!(
        "{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}",
        "Dirs :", d[TOT], d[COP], d_skip, d[MIS], d[FAI], d[EXT]
    );
    println!(
        "{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}",
        "Files :", f[TOT], f[COP], f_skip, f[MIS], f[FAI], f[EXT]
    );
    println!(
        "{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}",
        "Bytes :",
        fmt_bytes(b[TOT]),
        fmt_bytes(b[COP]),
        fmt_bytes(b_skip),
        fmt_bytes(b[MIS]),
        fmt_bytes(b[FAI]),
        fmt_bytes(b[EXT])
    );
    let elapsed = start.elapsed().as_secs();
    // Times 行与其它行列宽一致（10），小时无前导零
    println!(
        "{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}",
        "Times :",
        fmt_duration(elapsed),
        fmt_duration(elapsed),
        "",
        "",
        fmt_duration(0),
        fmt_duration(0)
    );
    // Speed
    let secs = start.elapsed().as_secs_f64();
    let copied_bytes = b[COP] as f64;
    if secs > 0.0 && copied_bytes > 0.0 {
        let bps = copied_bytes / secs;
        let mbpm = bps / 1048576.0 * 60.0;
        println!("\r\n\r\n{:>10} {:>19} Bytes/sec.", "Speed :", thousands(bps.round() as u64));
        println!("{:>10} {:>19.3} MegaBytes/min.", "Speed :", mbpm);
    }
    println!("{:>10} {}", "Ended :", fmt_now_cn());
    println!();
}

/// 文件大小格式化：<1024 原样，否则 KB/MB/GB（一位小数）。
fn fmt_bytes(n: u64) -> String {
    if n < 1024 {
        n.to_string()
    } else {
        let units = ["KB", "MB", "GB", "TB"];
        let mut v = n as f64;
        let mut u = 0;
        while v >= 1024.0 && u < units.len() - 1 {
            v /= 1024.0;
            u += 1;
        }
        format!("{v:.1} {}", units[u])
    }
}

fn fmt_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h}:{m:02}:{s:02}")
}

/// 千位分隔。
fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

// ---------------------------------------------------------------------------
// 工具函数
// ---------------------------------------------------------------------------

fn matches_pattern(name: &str, patterns: &[String]) -> bool {
    if patterns.is_empty() {
        return true; // 默认匹配所有
    }
    patterns.iter().any(|p| p == "*.*" || wild_match(p, name))
}

/// 通配符匹配（* 和 ?，大小写不敏感）。
fn wild_match(pattern: &str, name: &str) -> bool {
    fn bytes(p: &[u8], n: &[u8]) -> bool {
        if p.is_empty() && n.is_empty() {
            return true;
        }
        if let Some(&c) = p.first() {
            if c == b'*' {
                return bytes(&p[1..], n) || (!n.is_empty() && bytes(p, &n[1..]));
            }
        }
        if let (Some(&c), Some(&nc)) = (p.first(), n.first()) {
            if c == b'?' || c == nc {
                return bytes(&p[1..], &n[1..]);
            }
        }
        false
    }
    bytes(
        &pattern.to_lowercase().into_bytes(),
        &name.to_lowercase().into_bytes(),
    )
}

/// 目录是否为空。
fn is_dir_empty(p: &Path) -> bool {
    match fs::read_dir(p) {
        Ok(mut it) => it.next().is_none(),
        Err(_) => true,
    }
}

/// 递归删除目录（尽力）。
fn remove_dir_all_best(p: &Path) -> bool {
    let mut ok = true;
    if let Ok(entries) = fs::read_dir(p) {
        for e in entries.flatten() {
            let path = e.path();
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if !remove_dir_all_best(&path) {
                    ok = false;
                }
            } else if fs::remove_file(&path).is_err() {
                ok = false;
            }
        }
    }
    if fs::remove_dir(p).is_err() {
        ok = false;
    }
    ok
}

/// 相对路径 → 绝对路径。
fn absolutize(p: &str) -> PathBuf {
    let path = PathBuf::from(p);
    if path.is_absolute() {
        path
    } else if let Ok(cwd) = env::current_dir() {
        cwd.join(path)
    } else {
        path
    }
}

/// 目录显示路径（统一尾分隔符）。
fn display_dir(p: &Path) -> String {
    let s = p.to_string_lossy().to_string();
    let sep = std::path::MAIN_SEPARATOR;
    if s.ends_with(sep) || s.ends_with('/') {
        s
    } else {
        format!("{s}{sep}")
    }
}

/// 源目录不可访问时的错误输出。
fn access_error(src: &Path) -> (String, String) {
    let msg = format!(
        "{} ERROR 2 (0x00000002) Accessing Source Directory {}",
        fmt_now_num(),
        display_dir(src)
    );
    let code = if cfg!(windows) {
        "The system cannot find the file specified.".to_string()
    } else {
        "No such file or directory.".to_string()
    };
    (msg, code)
}

// ---------------------------------------------------------------------------
// 时间（暂按 UTC，README 记录差异）
// ---------------------------------------------------------------------------

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `2026年8月11日 20:38:22`（Started/Ended 用，原版随 locale，暂按 UTC）。
fn fmt_now_cn() -> String {
    let secs = now_secs();
    let days = secs / 86400;
    let (y, m, d) = days_to_ymd(days);
    let rem = secs % 86400;
    let hh = rem / 3600;
    let mi = (rem % 3600) / 60;
    let ss = rem % 60;
    format!("{y}年{m}月{d}日 {hh:02}:{mi:02}:{ss:02}")
}

/// `2026/08/11 20:36:32`（错误行用）。
fn fmt_now_num() -> String {
    let secs = now_secs();
    let days = secs / 86400;
    let (y, m, d) = days_to_ymd(days);
    let rem = secs % 86400;
    let hh = rem / 3600;
    let mi = (rem % 3600) / 60;
    let ss = rem % 60;
    format!("{y:04}/{m:02}/{d:02} {hh:02}:{mi:02}:{ss:02}")
}

/// 从 UNIX 天数转换日期。
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let mut y = 1970u64;
    let mut remaining = days;
    loop {
        let ydays = if is_leap(y) { 366 } else { 365 };
        if remaining >= ydays {
            remaining -= ydays;
            y += 1;
        } else {
            break;
        }
    }
    let mut m = 1u64;
    while m <= 12 {
        let mdays = month_days(y, m);
        if remaining >= mdays {
            remaining -= mdays;
            m += 1;
        } else {
            break;
        }
    }
    (y, m, remaining + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn month_days(y: u64, m: u64) -> u64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}
