//! 输出目标分发：stdout / 日志文件（/LOG /LOG+ /TEE）。

use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

struct State {
    log: Option<File>,
    tee: bool,
    /// /LOG 无 /TEE 时，stdout 在输出 "Log File" 行后静默
    console_quiet: bool,
    /// /UNILOG /UNILOG+：日志文件以 UTF-16LE（带 BOM）写入
    unicode_log: bool,
    /// /UNICODE：stdout 以 UTF-16LE 输出
    unicode_out: bool,
    /// 已向 stdout 写过内容（用于 /UNICODE 只写一次 BOM）
    stdout_started: bool,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| {
        Mutex::new(State {
            log: None,
            tee: false,
            console_quiet: false,
            unicode_log: false,
            unicode_out: false,
            stdout_started: false,
        })
    })
}

/// 字符串 → UTF-16LE 字节。
fn utf16le_bytes(s: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(s.len() * 2);
    for u in s.encode_utf16() {
        v.extend_from_slice(&u.to_le_bytes());
    }
    v
}

/// 向 stdout 写入（按 /UNICODE 编码；首次写入时带 UTF-16LE BOM）。
fn write_stdout(st: &mut State, s: &str) {
    let mut o = std::io::stdout();
    if st.unicode_out {
        if !st.stdout_started {
            let _ = o.write_all(&[0xFF, 0xFE]); // BOM
            st.stdout_started = true;
        }
        let _ = o.write_all(&utf16le_bytes(s));
    } else {
        let _ = o.write_all(s.as_bytes());
        st.stdout_started = true;
    }
    let _ = o.flush();
}

/// 初始化输出目标。
/// `log`：(路径, 是否追加)。覆盖式（truncate）或追加。
/// `tee`：/TEE 时 stdout 与日志双写。
/// `unicode_log`：/UNILOG /UNILOG+ 时日志文件写 UTF-16LE（带 BOM）。
/// `unicode_out`：/UNICODE 时 stdout 写 UTF-16LE（带 BOM）。
pub fn init(log: Option<(PathBuf, bool)>, tee: bool, unicode_log: bool, unicode_out: bool) {
    let mut st = state().lock().unwrap();
    st.tee = tee;
    st.console_quiet = false;
    st.unicode_log = unicode_log;
    st.unicode_out = unicode_out;
    st.stdout_started = false;
    st.log = None;
    if let Some((path, append)) = log {
        let f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(!append)
            .append(append)
            .open(&path)
            .ok();
        if let Some(mut f) = f {
            if unicode_log && !append {
                let _ = f.write_all(&[0xFF, 0xFE]); // UTF-16LE BOM
            }
            st.log = Some(f);
        }
    }
}

/// 向日志文件写入（按编码转换）。
fn write_log(f: &mut File, s: &str, unicode: bool) {
    if unicode {
        let _ = f.write_all(&utf16le_bytes(s));
    } else {
        let _ = f.write_all(s.as_bytes());
    }
    let _ = f.flush();
}

/// 输出（不换行）。写入 stdout（除非静默）和日志文件。
pub fn out(s: &str) {
    let mut st = state().lock().unwrap();
    let unicode_log = st.unicode_log;
    if !st.console_quiet {
        write_stdout(&mut st, s);
    }
    if let Some(f) = &mut st.log {
        write_log(f, s, unicode_log);
    }
}

/// 输出一行（追加 CRLF）。
pub fn outln(s: &str) {
    let mut st = state().lock().unwrap();
    let unicode_log = st.unicode_log;
    if !st.console_quiet {
        write_stdout(&mut st, &format!("{s}\r\n"));
    }
    if let Some(f) = &mut st.log {
        write_log(f, &format!("{s}\r\n"), unicode_log);
    }
}

/// 双格式输出：stdout 用 `console` 文本，日志文件用 `log` 文本（时间格式不同）。
pub fn emit_split(console: &str, log: &str) {
    let mut st = state().lock().unwrap();
    let unicode_log = st.unicode_log;
    if !st.console_quiet {
        write_stdout(&mut st, console);
    }
    if let Some(f) = &mut st.log {
        write_log(f, log, unicode_log);
    }
}

/// 打印 ` Log File : path` 行；无 /TEE 时 stdout 此后静默。
pub fn announce_log_file(path: &str) {
    let mut st = state().lock().unwrap();
    let mut o = std::io::stdout();
    let _ = write!(o, "\r\n Log File : {path}\r\n");
    let _ = o.flush();
    if !st.tee {
        st.console_quiet = true;
    }
}
