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
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| Mutex::new(State {
        log: None,
        tee: false,
        console_quiet: false,
    }))
}

/// 初始化输出目标。
/// `log`：(路径, 是否追加)。/LOG 覆盖（truncate），/LOG+ 追加。
/// `tee`：/TEE 时 stdout 与日志双写。
pub fn init(log: Option<(PathBuf, bool)>, tee: bool) {
    let mut st = state().lock().unwrap();
    st.tee = tee;
    st.console_quiet = false;
    st.log = None;
    if let Some((path, append)) = log {
        let f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(!append)
            .append(append)
            .open(&path)
            .ok();
        st.log = f;
    }
}

/// 输出（不换行）。写入 stdout（除非静默）和日志文件。
pub fn out(s: &str) {
    let mut st = state().lock().unwrap();
    if !st.console_quiet {
        let mut o = std::io::stdout();
        let _ = o.write_all(s.as_bytes());
        let _ = o.flush();
    }
    if let Some(f) = &mut st.log {
        let _ = f.write_all(s.as_bytes());
        let _ = f.flush();
    }
}

/// 输出一行（追加 CRLF）。
pub fn outln(s: &str) {
    let mut st = state().lock().unwrap();
    if !st.console_quiet {
        let mut o = std::io::stdout();
        let _ = write!(o, "{s}\r\n");
        let _ = o.flush();
    }
    if let Some(f) = &mut st.log {
        let _ = write!(f, "{s}\r\n");
        let _ = f.flush();
    }
}

/// 双格式输出：stdout 用 `console` 文本，日志文件用 `log` 文本（时间格式不同）。
pub fn emit_split(console: &str, log: &str) {
    let mut st = state().lock().unwrap();
    if !st.console_quiet {
        let mut o = std::io::stdout();
        let _ = o.write_all(console.as_bytes());
        let _ = o.flush();
    }
    if let Some(f) = &mut st.log {
        let _ = f.write_all(log.as_bytes());
        let _ = f.flush();
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
