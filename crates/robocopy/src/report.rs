//! 输出：Header / Options 回显 / Usage / 状态行 / 统计表格。

use std::path::Path;
use std::time::Instant;

use crate::flags::{Class, Options, Stats, COP, EXT, FAI, MIS, TOT};
use crate::time::{fmt_now_cn, fmt_now_log};
use crate::util::{fmt_bytes, fmt_bytes_sum, fmt_duration, thousands, thousands_decimal};

/// 输出一行（行尾 CRLF，对齐原版重定向输出；经 sink 分发到 stdout / 日志）。
#[macro_export]
macro_rules! outln {
    ($($t:tt)*) => { crate::sink::outln(&format!($($t)*)) };
}

/// 原样输出（不追加换行；经 sink 分发到 stdout / 日志）。
#[macro_export]
macro_rules! out {
    ($($t:tt)*) => { crate::sink::out(&format!($($t)*)) };
}

/// 分类字段：Extra/Mismatch 用 `  {:<12}`（14 宽），其它用 `    {:<10}`（14 宽）。
pub fn file_class_field(class: Class) -> String {
    if class == Class::Extra || class == Class::Mismatch {
        format!("  {:<12}", class.label())
    } else {
        format!("    {:<10}", class.label())
    }
}

/// 目录分类字段：`  New Dir          `（2 缩进 + 分类左对齐 17 字符 = 19 宽）。
pub fn dir_class_field(class: &str) -> String {
    format!("  {class:<17}")
}

/// 文件状态行的分类字段。
pub fn field_str(class: Class, opts: &Options) -> String {
    if opts.no_class {
        " ".repeat(14)
    } else {
        file_class_field(class)
    }
}

/// 文件状态行的大小字段（对齐原版：<1024 字节数，否则 `32.0 m` 人类可读，右对齐 8）。
pub fn sz_str(size: u64, opts: &Options) -> String {
    if opts.no_size {
        String::new()
    } else {
        format!("{:>8}", fmt_bytes(size))
    }
}

/// 文件状态行。`ts`：/TS 的源文件 mtime（UTC 秒）；`full_path`：/FP 的完整路径。
/// `progress=true` 时模拟原版重定向：文件行后 `\r` 回车再写 `100%  `。
/// `eta`：/ETA 的 `HH:MM -> HH:MM` 文本（拼接在文件名后，`\t\t` 分隔）。
pub fn output_file_line(
    class: Class,
    size: u64,
    name: &str,
    ts: Option<u64>,
    opts: &Options,
    progress: bool,
    eta: Option<&str>,
) {
    if opts.no_file_list {
        return;
    }
    // /MT 强制显示完整路径（原版行为），与 /FP 无关（不影响 Options 回显）
    let display_name = if opts.full_path || opts.mt.is_some() { name } else { file_name_of(name) };
    // 原版格式：`\t{field}\t\t{sz}[ {ts}]\t{name}[ \t\t{eta}]`（/TS 在大小后、文件名前）
    let ts_part = match ts {
        Some(t) => format!(" {}", crate::time::fmt_utc(t)),
        None => String::new(),
    };
    let eta_part = match eta {
        Some(e) => format!("\t\t{e}"),
        None => String::new(),
    };
    let line = format!("\t{}\t\t{}{ts_part}\t{display_name}{eta_part}", field_str(class, opts), sz_str(size, opts));
    if progress {
        out!("{line}\r100%  \r\n");
    } else {
        outln!("{line}");
    }
}

/// 取路径最后一个组件（无 /FP 时只显示文件名）。
fn file_name_of(p: &str) -> &str {
    p.rsplit(['\\', '/']).next().unwrap_or(p)
}

/// /V 的 skipped 行：小写分类右对齐 14 宽（实测原版 `          same`）。
pub fn output_skipped_line(class: Class, size: u64, name: &str, ts: Option<u64>, opts: &Options) {
    if opts.no_file_list {
        return;
    }
    let field = format!("{:>14}", class.lower());
    let display_name = if opts.full_path || opts.mt.is_some() { name } else { file_name_of(name) };
    let ts_part = match ts {
        Some(t) => format!(" {}", crate::time::fmt_utc(t)),
        None => String::new(),
    };
    outln!("\t{field}\t\t{}{ts_part}\t{}", sz_str(size, opts), display_name);
}

/// 额外文件行（/X 报告 / /PURGE 删除时）：`  *EXTRA File   <size>\t<name>`（size 为原始字节数）。
pub fn output_extra_file_line(name: &str, size: u64, opts: &Options) {
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
    outln!("\t{field}\t\t{sz}\t{name}");
}

const HELP: &str = include_str!("../help.txt");

const USAGE_HEAD: &str = "       Simple Usage :: ROBOCOPY source destination /MIR\r\n\r\n             source :: Source Directory (drive:\\path or \\\\server\\share\\path).\r\n        destination :: Destination Dir  (drive:\\path or \\\\server\\share\\path).\r\n               /MIR :: Mirror a complete directory tree.\r\n\r\n    For more usage information run ROBOCOPY /?\r\n\r\n";

/// 打印用法错误块（`****` 警告）。
pub fn print_usage() {
    out!("{USAGE_HEAD}");
    outln!("{}", " ".repeat(58));
    outln!("****  /MIR can DELETE files as well as copy them !");
}

/// 打印完整帮助（原版 /? 也返回 16）。
pub fn print_help() {
    out!("\r\n{HELP}");
}

/// 文件模式展示（多个用空格分隔）。
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
    if opts.show_ts {
        v.push("/TS".into());
    }
    if opts.full_path {
        v.push("/FP".into());
    }
    if opts.show_bytes {
        v.push("/BYTES".into());
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
    if opts.tee {
        v.push("/TEE".into());
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
    if opts.create_only {
        v.push("/CREATE".into());
    }
    if opts.restartable {
        v.push("/Z".into());
    }
    if opts.no_progress {
        v.push("/NP".into());
    }
    if opts.eta {
        v.push("/ETA".into());
    }
    if let Some(mt) = opts.mt {
        v.push(format!("/MT:{mt}"));
    }
    if opts.include_same {
        v.push("/IS".into());
    }
    if opts.include_tweaked {
        v.push("/IT".into());
    }
    if opts.exclude_lonely {
        v.push("/XL".into());
    }
    if opts.exclude_extra {
        v.push("/XX".into());
    }
    if opts.exclude_older {
        v.push("/XO".into());
    }
    if opts.exclude_newer {
        v.push("/XN".into());
    }
    if opts.exclude_changed {
        v.push("/XC".into());
    }
    // /XJ 与 /XJF /XJD：实测 /XJ 时只回显 /XJ；仅 /XJF /XJD 时回显 `/XJF /XJD`
    if opts.exclude_junction {
        v.push("/XJ".into());
    } else {
        if opts.exclude_junction_file {
            v.push("/XJF".into());
        }
        if opts.exclude_junction_dir {
            v.push("/XJD".into());
        }
    }
    if opts.fft {
        v.push("/FFT".into());
    }
    if opts.archive_move {
        v.push("/M".into());
    } else if opts.archive {
        v.push("/A".into());
    }
    if !opts.include_attrs.is_empty() {
        let s: String = opts.include_attrs.iter().collect();
        v.push(format!("/IA:{s}"));
    }
    if !opts.exclude_attrs.is_empty() {
        let s: String = opts.exclude_attrs.iter().collect();
        v.push(format!("/XA:{s}"));
    }
    if let Some(n) = opts.max_size {
        v.push(format!("/MAX:{n}"));
    }
    if let Some(n) = opts.min_size {
        v.push(format!("/MIN:{n}"));
    }
    if let Some(n) = opts.max_age {
        v.push(format!("/MAXAGE:{n}"));
    }
    if let Some(n) = opts.min_age {
        v.push(format!("/MINAGE:{n}"));
    }
    if let Some(n) = opts.max_lad {
        v.push(format!("/MAXLAD:{n}"));
    }
    if let Some(n) = opts.min_lad {
        v.push(format!("/MINLAD:{n}"));
    }
    if let Some(n) = opts.lev {
        v.push(format!("/LEV:{n}"));
    }
    v.push(format!("/R:{}", opts.retries));
    v.push(format!("/W:{}", opts.wait.as_secs()));
    v.join(" ")
}

/// Header 块。
pub fn print_header_with(
    src: Option<&Path>,
    dst: Option<&Path>,
    files: &[String],
    opts: &Options,
    simple: bool,
) {
    outln!("-------------------------------------------------------------------------------");
    outln!("{:<81}", "   ROBOCOPY     ::     Robust File Copy for Windows");
    outln!("-------------------------------------------------------------------------------");
    // Started 行：stdout 用中文 locale，日志文件用数字格式（实测原版）
    crate::sink::emit_split(
        &format!("\r\n  Started : {}\r\n", fmt_now_cn()),
        &format!("\r\n  Started : {}\r\n", fmt_now_log()),
    );
    if !simple {
        match src {
            Some(s) => outln!("{:>11} {}", "Source :", crate::util::display_dir(s)),
            None => outln!("{:>11} ", "Source -"),
        }
        match dst {
            Some(d) => outln!("{:>11} {}", "Dest :", crate::util::display_dir(d)),
            None => outln!("{:>11} ", "Dest -"),
        }
        outln!("\r\n{:>11} {}", "Files :", files_mode(files));
        outln!("\t    ");
        // /XF /XD 有独立行（实测原版格式：Exc Files 顶格、Exc Dirs 缩进 1 空格）
        if !opts.xf.is_empty() {
            outln!("Exc Files : {}", opts.xf.join(" "));
            outln!("\t    ");
        }
        if !opts.xd.is_empty() {
            outln!(" Exc Dirs : {}", opts.xd.join(" "));
            outln!("\t    ");
        }
        outln!("{:>11} {} ", "Options :", options_line(files, opts));
    }
}

/// 统计表格。`mt`：/MT 模式下原版 Dirs 统计为 Copied=Total、Skipped=已存在目录数。
pub fn print_summary(stats: &Stats, start: Instant, mt: bool) {
    let d = &stats.dirs;
    let f = &stats.files;
    let b = &stats.bytes;
    let d_skip = if mt {
        stats.dir_skip
    } else {
        d[TOT].saturating_sub(d[COP] + d[MIS] + d[FAI])
    };
    let f_skip = f[TOT].saturating_sub(f[COP] + f[MIS] + f[FAI]);
    let b_skip = b[TOT].saturating_sub(b[COP] + b[MIS] + b[FAI]);

    outln!(
        "{:>20}{:>10}{:>10}{:>10}{:>10}{:>10}",
        "Total", "Copied", "Skipped", "Mismatch", "FAILED", "Extras"
    );
    outln!(
        "{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}",
        "Dirs :",
        d[TOT],
        d[COP],
        d_skip,
        d[MIS],
        d[FAI],
        d[EXT]
    );
    outln!(
        "{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}",
        "Files :",
        f[TOT],
        f[COP],
        f_skip,
        f[MIS],
        f[FAI],
        f[EXT]
    );
    outln!(
        "{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}",
        "Bytes :",
        fmt_bytes_sum(b[TOT]),
        fmt_bytes_sum(b[COP]),
        fmt_bytes_sum(b_skip),
        fmt_bytes_sum(b[MIS]),
        fmt_bytes_sum(b[FAI]),
        fmt_bytes_sum(b[EXT])
    );

    let elapsed = start.elapsed().as_secs();
    // Times 行与其它行列宽一致（10），小时无前导零
    outln!(
        "{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}{:>10}",
        "Times :",
        fmt_duration(elapsed),
        fmt_duration(elapsed),
        "",
        "",
        fmt_duration(0),
        fmt_duration(0)
    );

    // Speed（实测原版：`Speed :` 10 宽 + 数字 20 宽，无中间空格）
    let secs = start.elapsed().as_secs_f64();
    let copied_bytes = b[COP] as f64;
    if secs > 0.0 && copied_bytes > 0.0 {
        let bps = copied_bytes / secs;
        let mbpm = bps / 1048576.0 * 60.0;
        outln!("\r\n\r\n{:>10}{:>20} Bytes/sec.", "Speed :", thousands(bps.round() as u64));
        outln!("{:>10}{:>20} MegaBytes/min.", "Speed :", thousands_decimal(mbpm, 3));
    }

    crate::sink::emit_split(
        &format!("{:>10} {}\r\n", "Ended :", fmt_now_cn()),
        &format!("{:>10} {}\r\n", "Ended :", fmt_now_log()),
    );
    crate::sink::outln("");
}
