//! more —— 一次显示一屏输出（复刻 Windows more.exe）。
//!
//! 简化实现：/S 压缩空行、/Tn 展开制表符、+n 起始行。
//! stdout 非终端（管道）时直接全部输出；终端时分页暂停。

use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::process::ExitCode;

use windowshit_args::{parse, Flag, Kind, Parsed, Unknown};
use windowshit_i18n::{FluentArgs, L10n};

const PAGE_LINES: usize = 23;

fn main() -> ExitCode {
    let mut i18n = L10n::detect();
    match i18n.lang() {
        "zh-CN" => i18n.add_ftl(include_str!("../locales/zh-CN.ftl")),
        _ => i18n.add_ftl(include_str!("../locales/en-US.ftl")),
    }
    i18n.set_help(
        include_str!("../locales/help.zh.txt"),
        include_str!("../locales/help.en.txt"),
    );

    L10n::setup_console_utf8();

    let raw: Vec<String> = env::args().skip(1).collect();

    if raw.iter().any(|a| a == "/?" || a == "-?") {
        println!("{}", i18n.help());
        return ExitCode::SUCCESS;
    }

    let mut tab_size = 8usize;
    let mut start_line: usize = 1;
    let mut files: Vec<String> = Vec::new();

    // 预提取特殊形态：/Tn 连写（tab 宽度）、+n（起始行）
    let mut rest: Vec<String> = Vec::new();
    for a in &raw {
        if let Some(up) = a.strip_prefix('/').or_else(|| a.strip_prefix('-')) {
            if let Some(n) = up.strip_prefix('T') {
                if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) {
                    tab_size = n.parse().unwrap_or(8);
                    continue;
                }
            }
        }
        if let Some(n) = a.strip_prefix('+') {
            if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) {
                start_line = n.parse().unwrap_or(1).max(1);
                continue;
            }
        }
        rest.push(a.clone());
    }

    // 精确开关表；/T 单独出现 → 默认 8；/E /C /P 忽略；未知按路径
    const FLAGS: &[Flag] = &[
        Flag::new("S", Kind::Flag),
        Flag::new("E", Kind::Ignore),
        Flag::new("C", Kind::Ignore),
        Flag::new("P", Kind::Ignore),
        Flag::new("T", Kind::Ignore),
    ];
    let parsed = match parse(&rest, FLAGS, Unknown::Path) {
        Ok(p) => p,
        Err(_) => Parsed {
            flags: Default::default(),
            paths: rest.iter().map(String::as_str).collect(),
        },
    };
    let squeeze = parsed.flags.contains_key("S");
    for f in parsed.paths {
        files.push(f.to_string());
    }

    // 读取全部输入：文件列表或 stdin
    let mut content = String::new();
    if files.is_empty() {
        let mut buf = String::new();
        if io::stdin().read_to_string(&mut buf).is_err() {
            return ExitCode::from(1);
        }
        content = buf;
    } else {
        for f in &files {
            match fs::read_to_string(f) {
                Ok(c) => content.push_str(&c),
                Err(e) => {
                    let err = e.to_string();
                    let mut a = FluentArgs::new();
                    a.set("file", f.as_str());
                    a.set("err", &err);
                    eprintln!("{}", i18n.tr("error-open-file", Some(&a)));
                    return ExitCode::from(1);
                }
            }
        }
    }

    // 切行、去掉行尾 \r。借用 content，避免再复制一份（峰值 ~N）。
    let mut lines: Vec<&str> = content
        .lines()
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();

    // /S 压缩连续空行
    if squeeze {
        let mut squeezed: Vec<&str> = Vec::new();
        let mut prev_blank = false;
        for l in lines {
            let blank = l.trim().is_empty();
            if blank && prev_blank {
                continue;
            }
            squeezed.push(l);
            prev_blank = blank;
        }
        lines = squeezed;
    }

    // 展开制表符 /Tn：仅在确实含 tab 时才生成新行，避免无条件复制。
    let mut owned_lines: Vec<String> = Vec::new();
    if tab_size > 0 && lines.iter().any(|l| l.contains('\t')) {
        for l in &lines {
            let mut out = String::new();
            let mut col = 0usize;
            for ch in l.chars() {
                if ch == '\t' {
                    let spaces = tab_size - (col % tab_size);
                    out.push_str(&" ".repeat(spaces));
                    col += spaces;
                } else {
                    out.push(ch);
                    col += 1;
                }
            }
            owned_lines.push(out);
        }
        lines = owned_lines.iter().map(|s| s.as_str()).collect();
    }

    // 起始行 +n
    if start_line > 1 {
        let skip = (start_line - 1).min(lines.len());
        lines.drain(..skip);
    }

    let stdout_terminal = io::stdout().is_terminal();
    let stdin_terminal = io::stdin().is_terminal();
    let mut stdout = io::stdout();

    let mut shown = 0usize;
    for line in &lines {
        let _ = writeln!(stdout, "{line}");
        shown += 1;

        // 分页暂停：只在 stdout 是终端且还有更多内容时
        if stdout_terminal && shown % PAGE_LINES == 0 {
            let remaining = lines.len();
            if shown < remaining {
                let _ = write!(stdout, "-- More -- ");
                let _ = stdout.flush();
                if stdin_terminal {
                    // 读一个按键（Windows/Linux 通用：读一行）
                    let mut key = String::new();
                    let _ = io::stdin().read_line(&mut key);
                }
                let _ = write!(stdout, "\r");
                let _ = stdout.flush();
            }
        }
    }

    ExitCode::SUCCESS
}
