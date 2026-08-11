//! 遍历、文件分类与复制核心。

use std::fs;
use std::io::{self, IsTerminal, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::flags::{Class, Options, Stats, COP, EXT, FAI, MIS, TOT};
use crate::report::{output_extra_file_line, output_file_line};
use crate::util::{is_dir_empty, matches_pattern, remove_dir_all_best};

/// 复制缓冲大小（1 MiB）。
const BUF_SIZE: usize = 1024 * 1024;

/// 文件复制动作。
enum Action {
    Copy,
    Skip,
    Mismatch,
}

/// 分类后动作。
fn class_action(class: Class, opts: &Options) -> Action {
    match class {
        Class::New if opts.exclude_lonely => Action::Skip, // /XL
        Class::New => Action::Copy,
        Class::Newer if opts.exclude_newer => Action::Skip, // /XN
        Class::Newer => Action::Copy,
        Class::Older if opts.exclude_older => Action::Skip, // /XO
        Class::Older => Action::Copy,
        Class::Changed if opts.exclude_changed => Action::Skip, // /XC
        Class::Changed => Action::Copy,
        Class::Same if opts.include_same => Action::Copy, // /IS
        Class::Tweaked if opts.include_tweaked => Action::Copy, // /IT
        Class::Same | Class::Tweaked => Action::Skip,
        Class::Extra | Class::Mismatch => Action::Mismatch,
    }
}

/// /FFT：FAT 文件时间（2 秒粒度），向下取整到偶数秒。
fn fft_round(t: std::time::SystemTime) -> std::time::SystemTime {
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs & !1)
}

/// 分类：源与目标文件比较。
fn classify(src: &Path, dst: &Path, opts: &Options) -> Class {
    let src_meta = match fs::metadata(src) {
        Ok(m) => m,
        Err(_) => return Class::Mismatch,
    };
    let dst_meta = match fs::metadata(dst) {
        Ok(m) => m,
        Err(_) => return Class::New, // 目标不存在 → 新文件
    };
    if src_meta.is_dir() != dst_meta.is_dir() {
        return Class::Mismatch; // 一边文件一边目录
    }
    if src_meta.is_dir() {
        return Class::Same; // 目录本身不分类
    }
    // /FFT：比较前将时间戳向下取整到 2 秒（FAT 粒度）
    let src_mt = src_meta
        .modified()
        .ok()
        .map(|t| if opts.fft { fft_round(t) } else { t });
    let dst_mt = dst_meta
        .modified()
        .ok()
        .map(|t| if opts.fft { fft_round(t) } else { t });
    match (src_mt, dst_mt) {
        (Some(s), Some(d)) if s < d => Class::Older,
        (Some(s), Some(d)) if s > d => Class::Newer,
        _ => {
            let src_sz = src_meta.len();
            let dst_sz = dst_meta.len();
            if src_sz != dst_sz {
                Class::Changed
            } else if !attrs_equal(src, dst) {
                // 属性不同 → Tweaked（/IT 才复制）
                Class::Tweaked
            } else {
                Class::Same
            }
        }
    }
}

/// 属性比较：Windows 用 GetFileAttributesW 完整属性位，Unix 比较只读位。
fn attrs_equal(src: &Path, dst: &Path) -> bool {
    #[cfg(windows)]
    {
        file_attrs(src) == file_attrs(dst)
    }
    #[cfg(not(windows))]
    {
        let s = fs::metadata(src)
            .map(|m| m.permissions().readonly())
            .unwrap_or(false);
        let d = fs::metadata(dst)
            .map(|m| m.permissions().readonly())
            .unwrap_or(false);
        s == d
    }
}

/// Windows 文件属性（失败时返回 0，避免误判 Tweaked）。
#[cfg(windows)]
fn file_attrs(p: &Path) -> u32 {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetFileAttributesW;
    let w: Vec<u16> = p
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: 标准 API，缓冲区正确
    unsafe {
        let a = GetFileAttributesW(w.as_ptr());
        if a == u32::MAX {
            0
        } else {
            a
        }
    }
}

/// 清除目标文件只读位（若存在且只读）。
fn clear_readonly(p: &Path) {
    #[cfg(windows)]
    {
        let a = file_attrs(p);
        if a & 0x1 != 0 {
            set_file_attrs(p, a & !0x1);
        }
    }
    #[cfg(not(windows))]
    {
        if let Ok(m) = fs::metadata(p) {
            if m.permissions().readonly() {
                let mut perms = m.permissions();
                perms.set_readonly(false);
                let _ = fs::set_permissions(p, perms);
            }
        }
    }
}

/// 复制源文件属性到目标（仅当前实现可表示的位）。
fn copy_attrs(src: &Path, dst: &Path) {
    #[cfg(windows)]
    {
        let a = file_attrs(src);
        if a != 0 {
            set_file_attrs(dst, a);
        }
    }
    #[cfg(not(windows))]
    {
        if let Ok(m) = fs::metadata(src) {
            let _ = fs::set_permissions(dst, m.permissions());
        }
    }
}

/// 设置 Windows 文件属性。
#[cfg(windows)]
fn set_file_attrs(p: &Path, attrs: u32) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::SetFileAttributesW;
    let w: Vec<u16> = p
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: 标准 API，缓冲区正确
    unsafe {
        let _ = SetFileAttributesW(w.as_ptr(), attrs);
    }
}

/// 文件是否设置归档位（/A /M 用）。Unix 无归档位，视为恒 true。
fn has_archive(_p: &Path) -> bool {
    #[cfg(windows)]
    {
        file_attrs(_p) & 0x20 != 0
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// 清除源文件归档位（/M 复制后）。
#[cfg(windows)]
fn clear_archive(p: &Path) {
    let a = file_attrs(p);
    if a & 0x20 != 0 {
        set_file_attrs(p, a & !0x20);
    }
}

/// 文件属性字母集合（/IA /XA 用）。
#[cfg(windows)]
fn attr_letters(p: &Path) -> Vec<char> {
    let a = file_attrs(p);
    let mut v = Vec::new();
    if a & 0x1 != 0 {
        v.push('R');
    }
    if a & 0x2 != 0 {
        v.push('H');
    }
    if a & 0x4 != 0 {
        v.push('S');
    }
    if a & 0x20 != 0 {
        v.push('A');
    }
    if a & 0x80 != 0 {
        v.push('N');
    }
    if a & 0x100 != 0 {
        v.push('T');
    }
    if a & 0x800 != 0 {
        v.push('C');
    }
    if a & 0x1000 != 0 {
        v.push('O');
    }
    if a & 0x2000 != 0 {
        v.push('I');
    }
    if a & 0x4000 != 0 {
        v.push('E');
    }
    v
}

/// Unix：仅支持只读属性 R。
#[cfg(not(windows))]
fn attr_letters(p: &Path) -> Vec<char> {
    if fs::metadata(p)
        .map(|m| m.permissions().readonly())
        .unwrap_or(false)
    {
        vec!['R']
    } else {
        Vec::new()
    }
}

/// /IA 包含匹配：文件的任一属性字母命中 include 列表。
fn include_attr_match(letters: &[char], include: &[char]) -> bool {
    include.is_empty() || letters.iter().any(|c| include.contains(c))
}

/// /XA 排除匹配：文件的任一属性字母命中 exclude 列表。
fn exclude_attr_match(letters: &[char], exclude: &[char]) -> bool {
    !exclude.is_empty() && letters.iter().any(|c| exclude.contains(c))
}

/// 时间过滤（/MAXAGE /MINAGE /MAXLAD /MINLAD，天）。返回 true 表示应排除。
fn age_excluded(meta: &fs::Metadata, opts: &Options) -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let secs_of = |day: u64| day.saturating_mul(86400);
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs());
    let atime = meta
        .accessed()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs());
    if let (Some(m), Some(n)) = (mtime, opts.max_age) {
        if now.saturating_sub(m) > secs_of(n) {
            return true; // 比 n 天更旧 → /MAXAGE 排除
        }
    }
    if let (Some(m), Some(n)) = (mtime, opts.min_age) {
        if now.saturating_sub(m) < secs_of(n) {
            return true; // 比 n 天更新 → /MINAGE 排除
        }
    }
    if let (Some(a), Some(n)) = (atime, opts.max_lad) {
        if now.saturating_sub(a) > secs_of(n) {
            return true; // 超过 n 天未访问 → /MAXLAD 排除
        }
    }
    if let (Some(a), Some(n)) = (atime, opts.min_lad) {
        if now.saturating_sub(a) < secs_of(n) {
            return true; // 小于 n 天未访问 → /MINLAD 排除
        }
    }
    false
}

/// 文件级过滤（/XF /MAX /MIN /MAXAGE 等）。返回 true 表示应排除。
fn file_excluded(src: &Path, name: &str, meta: &fs::Metadata, opts: &Options) -> bool {
    if !opts.xf.is_empty() && matches_pattern(name, &opts.xf) {
        return true;
    }
    if let Some(n) = opts.max_size {
        if meta.len() > n {
            return true;
        }
    }
    if let Some(n) = opts.min_size {
        if meta.len() < n {
            return true;
        }
    }
    if (opts.max_age.is_some()
        || opts.min_age.is_some()
        || opts.max_lad.is_some()
        || opts.min_lad.is_some())
        && age_excluded(meta, opts)
    {
        return true;
    }
    if (opts.archive || opts.archive_move) && !has_archive(src) {
        return true;
    }
    if !opts.include_attrs.is_empty() || !opts.exclude_attrs.is_empty() {
        let letters = attr_letters(src);
        if !include_attr_match(&letters, &opts.include_attrs) {
            return true;
        }
        if exclude_attr_match(&letters, &opts.exclude_attrs) {
            return true;
        }
    }
    false
}

/// 是否重解析点（Windows junction/symlink）。
/// 注意用 `symlink_metadata`（不跟随），否则 junction 会被解析到目标而识别不出。
fn is_reparse_point(p: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        fs::symlink_metadata(p)
            .map(|m| m.file_attributes() & 0x400 != 0) // FILE_ATTRIBUTE_REPARSE_POINT
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        fs::symlink_metadata(p)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    }
}

/// 逐块复制文件。`animate=true` 时每块后输出 `\rNN.N%`（TTY 动态进度，对齐原版一位小数）。
/// `restartable`（/Z）：目标存在部分数据（0 < 目标大小 < 源大小）时从断点继续，不重新覆盖。
fn copy_streaming(src: &Path, dst: &Path, animate: bool, restartable: bool) -> io::Result<()> {
    let mut reader = fs::File::open(src)?;
    let total = reader.metadata().map(|m| m.len()).unwrap_or(0);
    let mut copied: u64 = 0;
    // /Z 断点续传：检测目标残留的部分文件并从源偏移继续
    let mut writer = if restartable {
        if let Ok(dm) = fs::metadata(dst) {
            let dlen = dm.len();
            if dlen > 0 && dlen < total {
                let w = fs::OpenOptions::new().append(true).open(dst)?;
                copied = dlen;
                w
            } else {
                fs::File::create(dst)?
            }
        } else {
            fs::File::create(dst)?
        }
    } else {
        fs::File::create(dst)?
    };
    if copied > 0 {
        reader.seek(SeekFrom::Start(copied))?;
    }
    let mut buf = vec![0u8; BUF_SIZE];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
        copied += n as u64;
        if animate && total > 0 {
            let pct = copied as f64 / total as f64 * 100.0;
            // 原版格式：`3.1%`，100 时为 `100%`，左对齐 6 宽
            let s = if pct >= 99.95 {
                "100%".to_string()
            } else {
                format!("{pct:.1}%")
            };
            crate::out!("\r{:<6}", s);
        }
    }
    Ok(())
}

/// /CREATE：只创建零字节文件，并还原源 mtime 与属性（实测原版目标 mtime=源）。
fn create_empty(src: &Path, dst: &Path) -> io::Result<()> {
    fs::File::create(dst)?;
    if let Ok(sm) = fs::metadata(src) {
        if let Ok(mt) = sm.modified() {
            if let Ok(f) = fs::File::options().write(true).open(dst) {
                let _ = f.set_modified(mt);
            }
        }
    }
    Ok(())
}

/// 复制参数的值拷贝（供线程池 worker 使用，避开 `'static` 生命周期约束）。
#[derive(Clone, Copy)]
struct CopyCtx {
    retries: u32,
    wait: Duration,
    create_only: bool,
    restartable: bool,
}

impl CopyCtx {
    fn new(opts: &Options) -> CopyCtx {
        CopyCtx {
            retries: opts.retries,
            wait: opts.wait,
            create_only: opts.create_only,
            restartable: opts.restartable,
        }
    }
}

/// 复制文件，带 /R /W 重试。
/// 对齐原版 /COPY:DAT：复制前清除目标只读位（否则覆盖会失败且无限重试），
/// 复制后设置目标 mtime 与属性 = 源（这样二次运行分类为 Same）。
fn copy_with_retry(src: &Path, dst: &Path, opts: &Options, animate: bool) -> io::Result<()> {
    copy_with_ctx(src, dst, &CopyCtx::new(opts), animate)
}

fn copy_with_ctx(src: &Path, dst: &Path, ctx: &CopyCtx, animate: bool) -> io::Result<()> {
    for i in 0..=ctx.retries {
        // 目标存在且只读时先清除只读位（原版行为）
        clear_readonly(dst);
        let r = if ctx.create_only {
            create_empty(src, dst)
        } else {
            copy_streaming(src, dst, animate, ctx.restartable)
        };
        match r {
            Ok(_) => {
                if !ctx.create_only {
                    // 还原源文件修改时间（原版行为：二次运行时间戳相同 → Same 跳过）
                    if let Ok(sm) = fs::metadata(src) {
                        if let Ok(mt) = sm.modified() {
                            if let Ok(f) = fs::File::options().write(true).open(dst) {
                                let _ = f.set_modified(mt);
                            }
                        }
                    }
                }
                // 复制后属性 = 源属性（/COPY:DA 的 A 部分）
                copy_attrs(src, dst);
                return Ok(());
            }
            Err(_e) if i < ctx.retries => thread::sleep(ctx.wait),
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}

/// /ETA 状态：基于已复制字节的平均速率估算剩余时间。
pub struct EtaState {
    pub copied: u32,
    pub bytes: u64,
    pub start: std::time::Instant,
}

/// /ETA 估算：非首个复制文件显示 `\t\tHH:MM -> HH:MM`。
/// 用已复制字节的平均速率对当前文件大小估计剩余耗时（原版基于实时吞吐）。
fn eta_estimate(eta: &mut EtaState, sz: u64) -> Option<String> {
    eta.bytes += sz;
    eta.copied += 1;
    if eta.copied <= 1 {
        return None;
    }
    let elapsed = eta.start.elapsed().as_secs_f64().max(0.001);
    let rate = eta.bytes as f64 / elapsed;
    let secs_left = if rate > 0.0 {
        (sz as f64 / rate).ceil() as u64
    } else {
        0
    };
    let now = crate::time::fmt_now_hm();
    let eta_t = crate::time::fmt_hm_after(secs_left);
    Some(format!("{now} -> {eta_t}"))
}

/// 单个文件复制任务（/MT 批次内按提交顺序收集）。
struct MtJob {
    f: PathBuf,
    dst: PathBuf,
    sz: u64,
    class: Class,
    name: String,
    ts: Option<u64>,
}

/// 线程池任务：(源路径, 目标路径, 结果回传通道)。
type CopyJob = (PathBuf, PathBuf, mpsc::Sender<io::Result<()>>);

/// /MT 多线程复制池。目录内文件并行复制，结果按提交顺序返回，保证输出有序。
pub struct Pool {
    tx: mpsc::Sender<CopyJob>,
}

impl Pool {
    pub fn new(opts: &Options) -> Pool {
        let n = opts.mt.unwrap_or(8).clamp(1, 128);
        let ctx = CopyCtx::new(opts);
        let (tx, rx): (mpsc::Sender<CopyJob>, mpsc::Receiver<CopyJob>) = mpsc::channel();
        let rx = Arc::new(Mutex::new(rx));
        for _ in 0..n {
            let rx = Arc::clone(&rx);
            thread::spawn(move || loop {
                let job = rx.lock().unwrap().recv();
                match job {
                    Ok((src, dst, out)) => {
                        let r = copy_with_ctx(&src, &dst, &ctx, false);
                        let _ = out.send(r);
                    }
                    Err(_) => break, // 发送端关闭，退出
                }
            });
        }
        Pool { tx }
    }

    /// 提交复制任务，返回结果接收器（按提交顺序 `recv` 对应结果）。
    fn submit(&self, src: PathBuf, dst: PathBuf) -> mpsc::Receiver<io::Result<()>> {
        let (tx, rx) = mpsc::channel();
        let _ = self.tx.send((src, dst, tx));
        rx
    }
}

/// 递归遍历 src 目录。`new_dir`：目标目录为本次新建（显示 `New Dir`）。
/// `level`：当前层级（根=1），用于 /LEV 限制。`rc` 累积退出码标志。
/// `pool`：/MT 线程池（目录内文件并行复制）；None 为单线程。
#[allow(clippy::too_many_arguments)]
pub fn walk(
    src: &Path,
    dst: &Path,
    opts: &Options,
    stats: &mut Stats,
    rc: &mut u32,
    new_dir: bool,
    level: u32,
    mut eta: Option<&mut EtaState>,
    pool: Option<&Pool>,
) {
    let entries = match fs::read_dir(src) {
        Ok(e) => e,
        Err(_) => {
            // 源目录不可访问 → 该目录计入失败
            stats.dir(FAI, 1);
            *rc |= 8;
            return;
        }
    };
    let mut files: Vec<PathBuf> = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        // 注意：Rust std 在 Windows 上 `symlink_metadata().is_dir()` 对 junction 返回 false，
        // 需额外检查 reparse point（junction 一律视为目录）
        let is_dir = match fs::symlink_metadata(&p) {
            Ok(m) => m.is_dir() || is_reparse_point(&p),
            Err(_) => false,
        };
        if is_dir {
            dirs.push(p);
        } else {
            files.push(p);
        }
    }
    files.sort();
    dirs.sort();

    let mt = pool.is_some();

    stats.dir(TOT, 1); // 每个访问的目录计入 Total
    if mt {
        // /MT 原版统计怪癖：Dirs Copied 恒等于 Total，Skipped 单独记已存在（未新建）目录
        stats.dir(COP, 1);
        if !new_dir {
            stats.dir_skip += 1;
        }
    }

    // 第一遍：分类文件，统计本目录匹配的文件数（供目录行数字，含 /XF /MAX 等排除项）
    let mut matched_count: u64 = 0;
    let mut plan: Vec<(PathBuf, Class, Option<u64>)> = Vec::new();
    for f in &files {
        let name = f.file_name().unwrap_or_default().to_string_lossy();
        if !matches_pattern(&name, &opts.files) {
            continue;
        }
        matched_count += 1;
        // /XJF 排除 junction 文件
        if opts.exclude_junction_file && is_reparse_point(f) {
            continue;
        }
        let meta = match fs::metadata(f) {
            Ok(m) => m,
            Err(_) => continue,
        };
        // /XF /MAX /MIN /MAXAGE /MINAGE /MAXLAD /MINLAD /A /M /IA /XA 排除
        if file_excluded(f, &name, &meta, opts) {
            continue;
        }
        let dst_file = dst.join(f.file_name().unwrap());
        let class = classify(f, &dst_file, opts);
        // /TS：源文件 mtime（UTC 秒）
        let ts = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        plan.push((f.clone(), class, ts));
    }

    // 目录状态行（数字 = 该目录匹配的文件数，与分类字段间无 tab）
    // /MT 抑制目录行输出（原版行为）
    if !opts.no_dir_list && !mt {
        let field = if new_dir {
            crate::report::dir_class_field("New Dir")
        } else {
            " ".repeat(19)
        };
        crate::outln!(
            "\t{field}{matched_count}\t{}",
            crate::util::display_dir(src)
        );
    }

    // extra 处理（目标中存在而源中没有的）。
    // 原版实测顺序：目录行 → extra（目录递归报告内部文件，然后文件）→ 文件复制行。
    // extra 目录会被递归报告其内部所有文件；/MT 下 extra 文件行显示完整路径。
    {
        let process_extra = |stats: &mut Stats, rc: &mut u32| {
            if let Ok(entries) = fs::read_dir(dst) {
                let mut extra_files: Vec<PathBuf> = Vec::new();
                let mut extra_dirs: Vec<PathBuf> = Vec::new();
                for e in entries.flatten() {
                    let p = e.path();
                    let name = e.file_name();
                    let in_src = files
                        .iter()
                        .any(|f| f.file_name() == Some(name.as_os_str()))
                        || dirs.iter().any(|d| d.file_name() == Some(name.as_os_str()));
                    if in_src {
                        continue;
                    }
                    let is_dir = match fs::symlink_metadata(&p) {
                        Ok(m) => m.is_dir() || is_reparse_point(&p),
                        Err(_) => false,
                    };
                    if is_dir {
                        extra_dirs.push(p);
                    } else {
                        extra_files.push(p);
                    }
                }
                extra_files.sort();
                extra_dirs.sort();

                // /XX：排除 extra（不报告行），但统计 Extras 并置退出码（原版实测）
                if opts.exclude_extra {
                    for _ in &extra_dirs {
                        stats.dir(EXT, 1);
                        *rc |= 2;
                    }
                    for ef in &extra_files {
                        let sz = ef.metadata().map(|m| m.len()).unwrap_or(0);
                        stats.file(EXT, 1, sz);
                        *rc |= 2;
                    }
                } else if opts.purge {
                    // 目录先（递归报告内部文件），文件后。/L 时也报告并统计（原版实测），不实际删除。
                    for ed in &extra_dirs {
                        extra_dir_mt(ed, opts, stats, rc, true, mt);
                        if !opts.list_only && remove_dir_all_best(ed) {
                            // 实际删除目录树
                        }
                        stats.dir(EXT, 1);
                        *rc |= 2;
                    }
                    for ef in &extra_files {
                        let name = extra_name(ef, mt);
                        let sz = ef.metadata().map(|m| m.len()).unwrap_or(0);
                        let reported = if opts.list_only {
                            true
                        } else {
                            fs::remove_file(ef).is_ok()
                        };
                        if reported {
                            stats.file(EXT, 1, sz);
                            *rc |= 2;
                            output_extra_file_line(&name, sz, opts);
                        }
                    }
                } else {
                    // 默认（含 /X，实测原版）：报告 extra 目录行与顶层文件，统计 Extras，退出码 |2。
                    // /L 也报告并统计（原版实测）。不递归目录内容（/PURGE 才递归列出并删除）。
                    for ed in &extra_dirs {
                        if !opts.no_dir_list {
                            crate::outln!(
                                "\t{:<18}-1\t{}",
                                "*EXTRA Dir",
                                crate::util::display_dir(ed)
                            );
                        }
                        stats.dir(EXT, 1);
                        *rc |= 2;
                    }
                    for ef in &extra_files {
                        let name = extra_name(ef, mt);
                        let sz = ef.metadata().map(|m| m.len()).unwrap_or(0);
                        output_extra_file_line(&name, sz, opts);
                        stats.file(EXT, 1, sz);
                        *rc |= 2;
                    }
                }
            }
        };
        process_extra(stats, rc);
    }

    // 第二遍：按分类执行。单线程立即复制；/MT 收集到本目录任务，待 extra 处理后统一提交。
    let mut mt_jobs: Vec<MtJob> = Vec::new();
    for (f, class, ts) in &plan {
        let name = f.file_name().unwrap_or_default().to_string_lossy();
        // /FP：显示源完整路径（实测原版格式 `f:\...\src\a.txt`）；/MT 强制完整路径
        let display_name = if opts.full_path || mt {
            f.to_string_lossy().replace('/', "\\")
        } else {
            name.to_string()
        };
        let sz = f.metadata().map(|m| m.len()).unwrap_or(0);
        let dst_file = dst.join(f.file_name().unwrap());
        // /TS 仅在 opts.show_ts 时传（否则任何场景都不显示时间戳）
        let ts_arg = if opts.show_ts { *ts } else { None };
        match class_action(*class, opts) {
            Action::Copy => {
                stats.file(TOT, 1, sz);
                if opts.list_only {
                    stats.file(COP, 1, sz);
                    *rc |= 1;
                    output_file_line(*class, sz, &display_name, ts_arg, opts, false, None);
                    continue;
                }
                if mt {
                    // /MT：收集到本目录任务，稍后批量提交线程池
                    mt_jobs.push(MtJob {
                        f: f.clone(),
                        dst: dst_file,
                        sz,
                        class: *class,
                        name: display_name,
                        ts: ts_arg,
                    });
                    continue;
                }
                // 是否动画进度：TTY 且未关进度、未关文件列表
                let animate = !opts.no_progress && !opts.no_file_list && io::stdout().is_terminal();
                if animate {
                    // 文件行先不带换行打印，复制过程动态刷新百分比
                    crate::out!(
                        "\t{}\t\t{}\t{}",
                        crate::report::field_str(*class, opts),
                        crate::report::sz_str(sz, opts),
                        display_name
                    );
                }
                if copy_with_retry(f, &dst_file, opts, animate).is_ok() {
                    stats.file(COP, 1, sz);
                    *rc |= 1;
                    // /ETA：非首个复制文件显示 `\t\tHH:MM -> HH:MM`（基于实测平均速率）
                    let mut eta_str: Option<String> = None;
                    if opts.eta {
                        if let Some(e) = eta.as_deref_mut() {
                            eta_str = eta_estimate(e, sz);
                        }
                    }
                    let eta_part = eta_str
                        .as_ref()
                        .map(|e| format!("\t\t{e}"))
                        .unwrap_or_default();
                    if animate {
                        crate::out!("\r100%  {eta_part}\r\n");
                    } else {
                        // 非 TTY：一次性进度行（对齐原版重定向字节 `name[ \t\tETA]\r100%  \r\n`）
                        output_file_line(
                            *class,
                            sz,
                            &display_name,
                            ts_arg,
                            opts,
                            !opts.no_progress && !opts.no_file_list,
                            eta_str.as_deref(),
                        );
                    }
                } else {
                    stats.file(FAI, 1, sz);
                    *rc |= 8;
                    if animate {
                        crate::out!("\r\n"); // 失败补换行
                    } else {
                        output_file_line(*class, sz, &display_name, ts_arg, opts, false, None);
                    }
                }
                // /M：复制并清除源归档位
                if opts.archive_move {
                    #[cfg(windows)]
                    clear_archive(f);
                }
                if opts.move_files || opts.move_all {
                    let _ = fs::remove_file(f);
                }
            }
            Action::Skip => {
                stats.file(TOT, 1, sz);
                if opts.verbose {
                    crate::report::output_skipped_line(*class, sz, &display_name, ts_arg, opts);
                }
            }
            Action::Mismatch => {
                stats.file(TOT, 1, sz);
                stats.file(MIS, 1, sz);
                *rc |= 4;
                output_file_line(
                    Class::Mismatch,
                    sz,
                    &display_name,
                    ts_arg,
                    opts,
                    false,
                    None,
                );
            }
        }
    }

    // /MT：批量提交本目录复制任务（输出在 extra 之后，原版顺序）
    if mt {
        if let Some(pool) = pool {
            flush_mt_jobs(pool, mt_jobs, stats, rc, opts, eta.as_deref_mut());
        }
    }

    // 递归子目录
    let max_level = opts.lev.unwrap_or(u32::MAX);
    for d in &dirs {
        let name = d.file_name().unwrap_or_default().to_string_lossy();
        // /XD 排除目录（整棵跳过）；原版仍计入 Dirs Total（访问但不处理）。
        // /MT 下 Copied 恒等于 Total 的怪癖同样适用于被排除目录。
        if !opts.xd.is_empty() && matches_pattern(&name, &opts.xd) {
            stats.dir(TOT, 1);
            if mt {
                stats.dir(COP, 1);
            }
            continue;
        }
        // /XJ /XJD 排除 junction 目录
        if (opts.exclude_junction || opts.exclude_junction_dir) && is_reparse_point(d) {
            continue;
        }
        // /LEV:n 限制层级
        if level >= max_level {
            continue;
        }
        let dst_dir = dst.join(d.file_name().unwrap());
        let empty = is_dir_empty(d);
        // /E /MIR 含空目录；/S 仅非空目录
        let need = opts.subdirs_all || (opts.subdirs_nonempty && !empty);
        if !need {
            continue;
        }
        let dst_dir_existed = dst_dir.exists();
        if !dst_dir_existed {
            if !opts.list_only {
                if fs::create_dir_all(&dst_dir).is_ok() {
                    if !mt {
                        stats.dir(COP, 1); // /MT 已在目录访问时计过 Copied
                    }
                } else {
                    stats.dir(FAI, 1);
                    *rc |= 8;
                }
            } else if !mt {
                // /L：不实际创建，但目标不存在仍计入 Copied（原版实测）
                stats.dir(COP, 1);
            }
        }
        // /L 不创建目录，但目录在目标中不存在仍算 New Dir
        walk(
            d,
            &dst_dir,
            opts,
            stats,
            rc,
            !dst_dir_existed,
            level + 1,
            eta.as_deref_mut(),
            pool,
        );
    }
}

/// 批量提交 /MT 复制任务并按提交顺序输出结果（顺序稳定，便于与原版 diff）。
fn flush_mt_jobs(
    pool: &Pool,
    jobs: Vec<MtJob>,
    stats: &mut Stats,
    rc: &mut u32,
    opts: &Options,
    mut eta: Option<&mut EtaState>,
) {
    if jobs.is_empty() {
        return;
    }
    let mut rxs = Vec::with_capacity(jobs.len());
    for j in &jobs {
        rxs.push(pool.submit(j.f.clone(), j.dst.clone()));
    }
    for (j, rx) in jobs.into_iter().zip(rxs) {
        let ok = rx.recv().map(|r| r.is_ok()).unwrap_or(false);
        if ok {
            stats.file(COP, 1, j.sz);
            *rc |= 1;
            // /ETA：非首个复制文件显示 `\t\tHH:MM -> HH:MM`（基于实测平均速率）
            let mut eta_str: Option<String> = None;
            if opts.eta {
                if let Some(e) = eta.as_deref_mut() {
                    eta_str = eta_estimate(e, j.sz);
                }
            }
            output_file_line(
                j.class,
                j.sz,
                &j.name,
                j.ts,
                opts,
                false,
                eta_str.as_deref(),
            );
        } else {
            stats.file(FAI, 1, j.sz);
            *rc |= 8;
            output_file_line(j.class, j.sz, &j.name, j.ts, opts, false, None);
        }
        // /M：复制并清除源归档位
        if opts.archive_move {
            #[cfg(windows)]
            clear_archive(&j.f);
        }
        if opts.move_files || opts.move_all {
            let _ = fs::remove_file(&j.f);
        }
    }
}

/// extra 文件的显示名：/MT 用完整路径（原版行为），否则用文件名。
fn extra_name(ef: &Path, mt: bool) -> String {
    if mt {
        ef.to_string_lossy().replace('/', "\\")
    } else {
        ef.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }
}

/// 递归处理 extra 目录（原版会报告目录内部的所有文件，dir 行显示 `-1`）。
/// `purge=true`（/PURGE）：删除文件并计入 Extras；否则（/X）只报告不统计。
/// `mt`：/MT 下文件行显示完整路径。
fn extra_dir_mt(ed: &Path, opts: &Options, stats: &mut Stats, rc: &mut u32, purge: bool, mt: bool) {
    if !opts.no_dir_list {
        // 原版格式：`*EXTRA Dir` 18 宽（无缩进）+ `-1`（数字列）
        crate::outln!("\t{:<18}-1\t{}", "*EXTRA Dir", crate::util::display_dir(ed));
    }
    if let Ok(entries) = fs::read_dir(ed) {
        let mut files: Vec<PathBuf> = Vec::new();
        let mut dirs: Vec<PathBuf> = Vec::new();
        for e in entries.flatten() {
            let p = e.path();
            let is_dir = match fs::symlink_metadata(&p) {
                Ok(m) => m.is_dir() || is_reparse_point(&p),
                Err(_) => false,
            };
            if is_dir {
                dirs.push(p);
            } else {
                files.push(p);
            }
        }
        files.sort();
        dirs.sort();
        for f in &files {
            let name = extra_name(f, mt);
            let sz = f.metadata().map(|m| m.len()).unwrap_or(0);
            if purge {
                // /L：报告并统计，不删除（原版实测）
                let reported = if opts.list_only {
                    true
                } else {
                    fs::remove_file(f).is_ok()
                };
                if reported {
                    stats.file(EXT, 1, sz);
                    *rc |= 2;
                    output_extra_file_line(&name, sz, opts);
                }
            } else {
                output_extra_file_line(&name, sz, opts);
            }
        }
        for d in &dirs {
            extra_dir_mt(d, opts, stats, rc, purge, mt);
        }
    }
}
