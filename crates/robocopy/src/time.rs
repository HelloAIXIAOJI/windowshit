//! 时间格式化（Windows 本地时间，其余平台 UTC）。

use std::time::{SystemTime, UNIX_EPOCH};

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 本地时间各部分 (y, m, d, hh, mi, ss)。仅 Windows 取本地时间；其余平台返回 None（用 UTC）。
fn now_local_parts() -> Option<(u64, u64, u64, u64, u64, u64)> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::SYSTEMTIME;
        use windows_sys::Win32::System::SystemInformation::GetLocalTime;
        // SAFETY: 标准 API，缓冲区由系统填充
        unsafe {
            let mut st: SYSTEMTIME = std::mem::zeroed();
            GetLocalTime(&mut st);
            Some((
                st.wYear as u64,
                st.wMonth as u64,
                st.wDay as u64,
                st.wHour as u64,
                st.wMinute as u64,
                st.wSecond as u64,
            ))
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// `2026年8月11日 20:38:22`（Started/Ended 用，原版随 locale）。
pub fn fmt_now_cn() -> String {
    if let Some((y, m, d, hh, mi, ss)) = now_local_parts() {
        return format!("{y}年{m}月{d}日 {hh:02}:{mi:02}:{ss:02}");
    }
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
pub fn fmt_now_num() -> String {
    if let Some((y, m, d, hh, mi, ss)) = now_local_parts() {
        return format!("{y:04}/{m:02}/{d:02} {hh:02}:{mi:02}:{ss:02}");
    }
    let secs = now_secs();
    let days = secs / 86400;
    let (y, m, d) = days_to_ymd(days);
    let rem = secs % 86400;
    let hh = rem / 3600;
    let mi = (rem % 3600) / 60;
    let ss = rem % 60;
    format!("{y:04}/{m:02}/{d:02} {hh:02}:{mi:02}:{ss:02}")
}

/// 日志文件时间格式（实测原版）：`2026811 23:30:49`（年/月/日无分隔无前导零，时分秒有）。
pub fn fmt_now_log() -> String {
    if let Some((y, m, d, hh, mi, ss)) = now_local_parts() {
        return format!("{y}{m}{d} {hh:02}:{mi:02}:{ss:02}");
    }
    let secs = now_secs();
    let days = secs / 86400;
    let (y, m, d) = days_to_ymd(days);
    let rem = secs % 86400;
    let hh = rem / 3600;
    let mi = (rem % 3600) / 60;
    let ss = rem % 60;
    format!("{y}{m}{d} {hh:02}:{mi:02}:{ss:02}")
}

/// UNIX 秒 → UTC `YYYY/MM/DD HH:MM:SS`（/TS 时间戳用，原版显示 UTC）。
pub fn fmt_utc(secs: u64) -> String {
    let days = secs / 86400;
    let (y, m, d) = days_to_ymd(days);
    let rem = secs % 86400;
    let hh = rem / 3600;
    let mi = (rem % 3600) / 60;
    let ss = rem % 60;
    format!("{y:04}/{m:02}/{d:02} {hh:02}:{mi:02}:{ss:02}")
}

/// 当前本地时间 `HH:MM`（/ETA 起始时间）。
pub fn fmt_now_hm() -> String {
    if let Some((_, _, _, hh, mi, _)) = now_local_parts() {
        return format!("{hh:02}:{mi:02}");
    }
    let secs = now_secs();
    let rem = secs % 86400;
    let hh = rem / 3600;
    let mi = (rem % 3600) / 60;
    format!("{hh:02}:{mi:02}")
}

/// 当前本地时间 + `secs` 秒 → `HH:MM`（/ETA 预计完成时间，近似）。
pub fn fmt_hm_after(secs: u64) -> String {
    let base = if let Some((_, _, _, hh, mi, ss)) = now_local_parts() {
        hh * 3600 + mi * 60 + ss + secs
    } else {
        (now_secs() % 86400) + secs
    };
    let t = base % 86400;
    let hh = t / 3600;
    let mi = (t % 3600) / 60;
    format!("{hh:02}:{mi:02}")
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
