//! 开关表、分类枚举、选项与统计结构。

use std::path::PathBuf;
use std::time::Duration;

use windowshit_args::{Flag, Kind};

/// 开关表：实现的收 `Flag`/`Value`，暂不实现的收 `Ignore`（原版存在但本实现不处理）。
pub const FLAGS: &[Flag] = &[
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
    Flag::new("XJ", Kind::Flag),
    Flag::new("XJD", Kind::Flag),
    Flag::new("XJF", Kind::Flag),
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

/// 文件分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
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
    /// 状态行分类标签（原版固定格式，大写）。
    pub fn label(self) -> &'static str {
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

    /// /V skipped 行的小写分类标签（实测原版：小写右对齐）。
    pub fn lower(self) -> &'static str {
        match self {
            Class::New => "new file",
            Class::Newer => "newer",
            Class::Older => "older",
            Class::Same => "same",
            Class::Tweaked => "tweaked",
            Class::Changed => "changed",
            Class::Extra => "*EXTRA File",
            Class::Mismatch => "*MISMATCH File",
        }
    }
}

/// 命令行选项。
pub struct Options {
    pub subdirs_nonempty: bool, // /S
    pub subdirs_all: bool,      // /E
    pub mirror: bool,           // /MIR
    pub purge: bool,            // /PURGE（含 /MIR 隐含）
    pub move_files: bool,       // /MOV
    pub move_all: bool,         // /MOVE
    pub list_only: bool,        // /L
    pub verbose: bool,          // /V
    pub report_extra: bool,     // /X
    pub retries: u32,           // /R:n（默认 1_000_000）
    pub wait: Duration,         // /W:n（默认 30s）
    pub mt: Option<usize>,      // /MT[:n]（阶段 1 仅回显，未做多线程）
    pub files: Vec<String>,     // 文件模式（默认匹配所有）
    // 输出控制
    pub no_progress: bool,   // /NP
    pub no_file_list: bool,  // /NFL
    pub no_dir_list: bool,   // /NDL
    pub no_size: bool,       // /NS
    pub no_class: bool,      // /NC
    pub no_job_header: bool, // /NJH
    pub no_job_summary: bool, // /NJS
    pub include_same: bool,    // /IS：Same 文件也复制
    pub include_tweaked: bool, // /IT：Tweaked 文件也复制
    // 阶段 2：文件选择与过滤
    pub xf: Vec<String>,       // /XF：排除文件（通配符）
    pub xd: Vec<String>,       // /XD：排除目录
    pub exclude_changed: bool, // /XC
    pub exclude_newer: bool,   // /XN
    pub exclude_older: bool,   // /XO
    pub exclude_lonely: bool,  // /XL：排除源有目标无
    pub exclude_extra: bool,   // /XX：排除目标多余
    pub max_size: Option<u64>, // /MAX:n
    pub min_size: Option<u64>, // /MIN:n
    pub max_age: Option<u64>,  // /MAXAGE:n（天）
    pub min_age: Option<u64>,  // /MINAGE:n（天）
    pub max_lad: Option<u64>,  // /MAXLAD:n（天）
    pub min_lad: Option<u64>,  // /MINLAD:n（天）
    pub archive: bool,          // /A：只复制归档位文件
    pub archive_move: bool,     // /M：复制并清除归档位
    pub include_attrs: Vec<char>, // /IA:...
    pub exclude_attrs: Vec<char>, // /XA:...
    pub lev: Option<u32>,         // /LEV:n
    pub exclude_junction: bool,     // /XJ
    pub exclude_junction_file: bool, // /XJF
    pub exclude_junction_dir: bool,  // /XJD
    // 阶段 3：日志与输出控制
    pub show_ts: bool,   // /TS：文件行显示 UTC 时间戳
    pub full_path: bool, // /FP：文件行显示完整源路径
    pub show_bytes: bool, // /BYTES：Options 回显
    pub eta: bool,        // /ETA：文件行尾显示预计完成时间
    pub log_path: Option<PathBuf>, // /LOG:file 或 /LOG+:file
    pub log_append: bool,          // /LOG+ 追加
    pub tee: bool,                 // /TEE 双输出
}

impl Default for Options {
    fn default() -> Self {
        Options {
            subdirs_nonempty: false,
            subdirs_all: false,
            mirror: false,
            purge: false,
            move_files: false,
            move_all: false,
            list_only: false,
            verbose: false,
            report_extra: false,
            retries: 1_000_000,
            wait: Duration::from_secs(30),
            mt: None,
            files: Vec::new(),
            no_progress: false,
            no_file_list: false,
            no_dir_list: false,
            no_size: false,
            no_class: false,
            no_job_header: false,
            no_job_summary: false,
            include_same: false,
            include_tweaked: false,
            xf: Vec::new(),
            xd: Vec::new(),
            exclude_changed: false,
            exclude_newer: false,
            exclude_older: false,
            exclude_lonely: false,
            exclude_extra: false,
            max_size: None,
            min_size: None,
            max_age: None,
            min_age: None,
            max_lad: None,
            min_lad: None,
            archive: false,
            archive_move: false,
            include_attrs: Vec::new(),
            exclude_attrs: Vec::new(),
            lev: None,
            exclude_junction: false,
            exclude_junction_file: false,
            exclude_junction_dir: false,
            show_ts: false,
            full_path: false,
            show_bytes: false,
            eta: false,
            log_path: None,
            log_append: false,
            tee: false,
        }
    }
}

/// 统计列索引。
pub const TOT: usize = 0; // Total
pub const COP: usize = 1; // Copied
pub const MIS: usize = 3; // Mismatch
pub const FAI: usize = 4; // FAILED
pub const EXT: usize = 5; // Extras

/// 统计计数。
#[derive(Default)]
pub struct Stats {
    pub dirs: [u64; 6],
    pub files: [u64; 6],
    pub bytes: [u64; 6],
}

impl Stats {
    pub fn dir(&mut self, col: usize, n: u64) {
        self.dirs[col] += n;
    }
    pub fn file(&mut self, col: usize, n: u64, bytes: u64) {
        self.files[col] += n;
        self.bytes[col] += bytes;
    }
}
