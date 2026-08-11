//! 时间格式化（Windows 本地时间，其余平台 UTC）。

use std::time::{SystemTime, UNIX_EPOCH};

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 本地时间各部分 (y, m, d, hh, mi, ss)。Windows 用 GetLocalTime，Unix 用 localtime_r。
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
        // Unix：用 libc localtime_r 获取本地时间（避免退化为 UTC）
        use libc::{localtime_r, time, tm};
        // SAFETY: 标准 C API，tm 缓冲区由 localtime_r 填充
        unsafe {
            let now = time(std::ptr::null_mut());
            if now == -1 {
                return None;
            }
            let mut t: tm = std::mem::zeroed();
            if localtime_r(&now, &mut t).is_null() {
                return None;
            }
            Some((
                (t.tm_year + 1900) as u64,
                (t.tm_mon + 1) as u64,
                t.tm_mday as u64,
                t.tm_hour as u64,
                t.tm_min as u64,
                t.tm_sec as u64,
            ))
        }
    }
}

/// `2026年8月11日 20:38:22`（Started/Ended 用，原版随 locale）。
/// 实测原版中文 locale：小时无前导零（`2:01:55`），分秒有前导零。
pub fn fmt_now_cn() -> String {
    if let Some((y, m, d, hh, mi, ss)) = now_local_parts() {
        return format!("{y}年{m}月{d}日 {hh}:{mi:02}:{ss:02}");
    }
    let secs = now_secs();
    let days = secs / 86400;
    let (y, m, d) = days_to_ymd(days);
    let rem = secs % 86400;
    let hh = rem / 3600;
    let mi = (rem % 3600) / 60;
    let ss = rem % 60;
    format!("{y}年{m}月{d}日 {hh}:{mi:02}:{ss:02}")
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
/// 实测原版日志：小时也无前导零（`2026812 2:02:24`）。
pub fn fmt_now_log() -> String {
    if let Some((y, m, d, hh, mi, ss)) = now_local_parts() {
        return format!("{y}{m}{d} {hh}:{mi:02}:{ss:02}");
    }
    let secs = now_secs();
    let days = secs / 86400;
    let (y, m, d) = days_to_ymd(days);
    let rem = secs % 86400;
    let hh = rem / 3600;
    let mi = (rem % 3600) / 60;
    let ss = rem % 60;
    format!("{y}{m}{d} {hh}:{mi:02}:{ss:02}")
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
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
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
