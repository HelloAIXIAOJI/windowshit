//! 遍历、文件分类与复制核心。

use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::thread;

use crate::flags::{Class, Options, Stats, COP, EXT, FAI, MIS, TOT};
use crate::report::{output_extra_file_line, output_file_line};
use crate::util::{is_dir_empty, matches_pattern, remove_dir_all_best};

/// 复制缓冲大小（1 MiB）。
const BUF_SIZE: usize = 1024 * 1024;

/// 目录状态行（删除场景）。
fn output_extra_dir_line(src: &Path) {
    crate::outln!("\t{}\t{}", crate::report::dir_class_field("*EXTRA Dir"), crate::util::display_dir(src));
}

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

/// 分类：源与目标文件比较。
fn classify(src: &Path, dst: &Path, _opts: &Options) -> Class {
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
    let src_mt = src_meta.modified().ok();
    let dst_mt = dst_meta.modified().ok();
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
        let s = fs::metadata(src).map(|m| m.permissions().readonly()).unwrap_or(false);
        let d = fs::metadata(dst).map(|m| m.permissions().readonly()).unwrap_or(false);
        s == d
    }
}

/// Windows 文件属性（失败时返回 0，避免误判 Tweaked）。
#[cfg(windows)]
fn file_attrs(p: &Path) -> u32 {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetFileAttributesW;
    let w: Vec<u16> = p.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
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
    let w: Vec<u16> = p.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
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
    if fs::metadata(p).map(|m| m.permissions().readonly()).unwrap_or(false) {
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
    let mtime = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs());
    let atime = meta.accessed().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs());
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
    if opts.max_age.is_some() || opts.min_age.is_some() || opts.max_lad.is_some() || opts.min_lad.is_some()
    {
        if age_excluded(meta, opts) {
            return true;
        }
    }
    if opts.archive || opts.archive_move {
        if !has_archive(src) {
            return true;
        }
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

/// 目录是否被 /XD 排除或 /XJ 排除。
fn dir_excluded(src: &Path, name: &str, opts: &Options) -> bool {
    if !opts.xd.is_empty() && matches_pattern(name, &opts.xd) {
        return true;
    }
    if opts.exclude_junction && is_reparse_point(src) {
        return true;
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
        fs::symlink_metadata(p).map(|m| m.file_type().is_symlink()).unwrap_or(false)
    }
}

/// 逐块复制文件。`animate=true` 时每块后输出 `\rNN.N%`（TTY 动态进度，对齐原版一位小数）。
fn copy_streaming(src: &Path, dst: &Path, animate: bool) -> io::Result<()> {
    let mut reader = fs::File::open(src)?;
    let mut writer = fs::File::create(dst)?;
    let total = reader.metadata().map(|m| m.len()).unwrap_or(0);
    let mut buf = vec![0u8; BUF_SIZE];
    let mut copied: u64 = 0;
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

/// 复制文件，带 /R /W 重试。
/// 对齐原版 /COPY:DAT：复制前清除目标只读位（否则覆盖会失败且无限重试），
/// 复制后设置目标 mtime 与属性 = 源（这样二次运行分类为 Same）。
fn copy_with_retry(src: &Path, dst: &Path, opts: &Options, animate: bool) -> io::Result<()> {
    for i in 0..=opts.retries {
        // 目标存在且只读时先清除只读位（原版行为）
        clear_readonly(dst);
        match copy_streaming(src, dst, animate) {
            Ok(_) => {
                // 还原源文件修改时间（原版行为：二次运行时间戳相同 → Same 跳过）
                if let Ok(sm) = fs::metadata(src) {
                    if let Ok(mt) = sm.modified() {
                        let f = fs::File::options().write(true).open(dst)?;
                        let _ = f.set_modified(mt);
                    }
                }
                // 复制后属性 = 源属性（/COPY:DA 的 A 部分）
                copy_attrs(src, dst);
                return Ok(());
            }
            Err(_e) if i < opts.retries => thread::sleep(opts.wait),
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}

/// 递归遍历 src 目录。`new_dir`：目标目录为本次新建（显示 `New Dir`）。
/// `level`：当前层级（根=1），用于 /LEV 限制。`rc` 累积退出码标志。
pub fn walk(
    src: &Path,
    dst: &Path,
    opts: &Options,
    stats: &mut Stats,
    rc: &mut u32,
    new_dir: bool,
    level: u32,
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

    stats.dir(TOT, 1); // 每个访问的目录计入 Total

    // 第一遍：分类文件，统计本目录匹配的文件数（供目录行数字，含 /XF /MAX 等排除项）
    let mut matched_count: u64 = 0;
    let mut plan: Vec<(PathBuf, Class)> = Vec::new();
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
        plan.push((f.clone(), class));
    }

    // 目录状态行（数字 = 该目录匹配的文件数，与分类字段间无 tab）
    if !opts.no_dir_list {
        let field = if new_dir {
            crate::report::dir_class_field("New Dir")
        } else {
            " ".repeat(19)
        };
        crate::outln!("\t{field}{matched_count}\t{}", crate::util::display_dir(src));
    }

    // 第二遍：按分类执行
    for (f, class) in &plan {
        let name = f.file_name().unwrap_or_default().to_string_lossy();
        let sz = f.metadata().map(|m| m.len()).unwrap_or(0);
        let dst_file = dst.join(f.file_name().unwrap());
        match class_action(*class, opts) {
            Action::Copy => {
                stats.file(TOT, 1, sz);
                if opts.list_only {
                    stats.file(COP, 1, sz);
                    *rc |= 1;
                    output_file_line(*class, sz, &name, opts, false);
                    continue;
                }
                // 是否动画进度：TTY 且未关进度、未关文件列表
                let animate = !opts.no_progress && !opts.no_file_list && io::stdout().is_terminal();
                if animate {
                    // 文件行先不带换行打印，复制过程动态刷新百分比
                    crate::out!("\t{}\t\t{}\t{}", crate::report::field_str(*class, opts), crate::report::sz_str(sz, opts), name);
                }
                if copy_with_retry(f, &dst_file, opts, animate).is_ok() {
                    stats.file(COP, 1, sz);
                    *rc |= 1;
                    if animate {
                        crate::out!("\r100%  \r\n");
                    } else {
                        // 非 TTY：一次性进度行（对齐原版重定向字节 `name\r100%  \r\n`）
                        output_file_line(*class, sz, &name, opts, !opts.no_progress && !opts.no_file_list);
                    }
                } else {
                    stats.file(FAI, 1, sz);
                    *rc |= 8;
                    if animate {
                        crate::out!("\r\n"); // 失败补换行
                    } else {
                        output_file_line(*class, sz, &name, opts, false);
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
                    output_file_line(*class, sz, &name, opts, false);
                }
            }
            Action::Mismatch => {
                stats.file(TOT, 1, sz);
                stats.file(MIS, 1, sz);
                *rc |= 4;
                output_file_line(Class::Mismatch, sz, &name, opts, false);
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

        // /XX：排除 extra（不删除不报告），优先于 /PURGE
        if opts.exclude_extra {
            // 不处理
        } else if opts.purge {
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
                        output_extra_dir_line(ed);
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
    let max_level = opts.lev.unwrap_or(u32::MAX);
    for d in &dirs {
        let name = d.file_name().unwrap_or_default().to_string_lossy();
        // /XD 排除目录（整棵跳过）；/XJ /XJD 排除 junction 目录
        if dir_excluded(d, &name, opts) {
            continue;
        }
        // /LEV:n 限制层级
        if level >= max_level {
            continue;
        }
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
        walk(d, &dst_dir, opts, stats, rc, !dst_dir_existed, level + 1);
    }
}
