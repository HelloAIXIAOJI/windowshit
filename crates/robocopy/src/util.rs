//! 通用工具：通配符匹配、路径规范化、目录递归删除、千位分隔、大小格式化。

use std::fs;
use std::path::{Path, PathBuf};

/// 通配符匹配（`*` 任意序列，`?` 单字符，跨平台，无大小写折叠）。
pub fn matches_pattern(name: &str, patterns: &[String]) -> bool {
    if patterns.is_empty() {
        return true;
    }
    patterns.iter().any(|p| wildcard_match(name, p))
}

/// 单个通配符模式匹配（迭代实现，避免深度递归栈溢出）。
fn wildcard_match(name: &str, pattern: &str) -> bool {
    let n: Vec<char> = name.chars().collect();
    let p: Vec<char> = pattern.chars().collect();
    let mut ni = 0usize;
    let mut pi = 0usize;
    let mut star: Option<usize> = None;
    let mut mark = 0usize;
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            ni += 1;
            pi += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(sp) = star {
            pi = sp + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// 目录路径展示：直接字符串化 + 尾随 `\`（不 canonicalize，避免 `\\?\` 前缀）。
pub fn display_dir(p: &Path) -> String {
    let s = p.to_string_lossy().to_string();
    let sep = std::path::MAIN_SEPARATOR;
    if s.ends_with(sep) || s.ends_with('/') {
        s
    } else {
        format!("{s}{sep}")
    }
}

/// 相对路径 → 绝对路径。
pub fn absolutize(p: &str) -> PathBuf {
    let path = PathBuf::from(p);
    if path.is_absolute() {
        path
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(path)
    } else {
        path
    }
}

/// 空目录判断。
pub fn is_dir_empty(p: &Path) -> bool {
    fs::read_dir(p)
        .map(|mut it| it.next().is_none())
        .unwrap_or(false)
}

/// 递归删除目录（先删内容再删自身，尽力而为）。
pub fn remove_dir_all_best(p: &Path) -> bool {
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

/// 千位分隔。
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// 千位分隔 + 固定小数位（整数部分千位分隔，小数点用 `.` 对齐原版）。
pub fn thousands_decimal(x: f64, precision: usize) -> String {
    let s = format!("{x:.precision$}");
    match s.split_once('.') {
        Some((int, frac)) => {
            let int_part: u64 = if int.is_empty() {
                0
            } else {
                int.parse().unwrap_or(0)
            };
            format!("{}.{frac}", thousands(int_part))
        }
        None => thousands(s.parse().unwrap_or(0)),
    }
}

/// 文件大小格式化（文件状态行用，实测 2026-08-11）：
/// <1 MiB 显示字节数；≥1 MiB 显示 `{:.1} {unit}`（1 位小数，小写 k/m/g/t）。
pub fn fmt_bytes(n: u64) -> String {
    if n < 1024 * 1024 {
        n.to_string()
    } else {
        let units = ["k", "m", "g", "t"];
        let mut v = n as f64;
        let mut u = 0;
        while v >= 1024.0 && u < units.len() {
            v /= 1024.0;
            u += 1;
        }
        // u=1→k, u=2→m, u=3→g, u=4→t
        format!("{v:.1} {}", units[u.saturating_sub(1)])
    }
}

/// 文件大小格式化（summary 统计表用，实测 2026-08-11）：
/// <1 KiB 字节数；<1 MiB `{:.1} k`；≥1 MiB `{:.2} m/g/t`。
pub fn fmt_bytes_sum(n: u64) -> String {
    if n < 1024 {
        n.to_string()
    } else if n < 1024 * 1024 {
        format!("{:.1} k", n as f64 / 1024.0)
    } else {
        let units = ["m", "g", "t"];
        let mut v = n as f64 / (1024.0 * 1024.0); // 直接 MiB 起算
        let mut u = 0;
        while v >= 1024.0 && u < units.len() - 1 {
            v /= 1024.0;
            u += 1;
        }
        format!("{v:.2} {}", units[u])
    }
}

/// 时长格式化（Times 行用，小时无前导零）。
pub fn fmt_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h}:{m:02}:{s:02}")
}
