//! windowshit-args —— Windows 风格命令行参数解析公共库。
//!
//! 核心规则：**只把精确命中已知开关表的参数当开关，其余一律按策略处理**
//! （路径 / 报错 / 忽略）。
//!
//! 这是为了修复 Linux 上 `/tmp/xxx` 这类绝对路径被 `/` 前缀误判成开关的
//! 问题——开关集合是有限的、已知的，而路径是无限的，因此
//! "宁可误伤开关，不可误吞路径"。
//!
//! 支持的开关形式（与 Windows 原版一致）：
//! - 前缀：`/` 或 `-`，大小写不敏感
//! - 无值开关：`/R`
//! - 取值开关：`/O file` 或 `/O:file`（空格与冒号两种写法都支持）
//! - 已知但忽略：`/M`（原版存在但本实现不处理）
//!
//! 特殊形态（如 sort 的 `/+n`、more 的 `/Tn` 与 `+n`）不在本库范围内，
//! 由各组件在调用本库前自行预提取。

use std::collections::HashMap;

/// 开关的取值方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 无值开关：`/R`
    Flag,
    /// 取值开关：`/O file` 或 `/O:file`
    Value,
    /// 已知但忽略：`/M`
    Ignore,
}

/// 单个开关定义。
///
/// `name` 为不含前缀的大写开关名（如 `"FO"`、`"R"`），匹配时不区分大小写。
#[derive(Debug, Clone, Copy)]
pub struct Flag {
    pub name: &'static str,
    pub kind: Kind,
}

impl Flag {
    pub const fn new(name: &'static str, kind: Kind) -> Self {
        Flag { name, kind }
    }
}

/// 未知参数（以 `/` 或 `-` 开头但未命中开关表）的处理策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unknown {
    /// 当作普通参数（路径）收集到 `Parsed::paths`
    Path,
    /// 返回 `Error::Unknown`
    Error,
    /// 静默丢弃
    Ignore,
}

/// 解析错误。携带触发错误的原始参数，由组件按自身语义生成提示。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error<'a> {
    /// 未知开关（如 `/xyz`）
    Unknown(&'a str),
    /// 取值开关缺少值（如 `/O` 后面没有参数）
    MissingValue(&'a str),
    /// 无值开关却带了内联值（如 `/R:1`）
    UnexpectedValue(&'a str),
}

/// 解析结果。
#[derive(Debug, Default)]
pub struct Parsed<'a> {
    /// 命中的开关名（大写）→ 值（仅 `Kind::Value` 有值，`Flag` 为 `None`）。
    /// 同一开关多次出现时后者覆盖前者（与原版一致）。
    pub flags: HashMap<&'a str, Option<&'a str>>,
    /// 普通参数（不以前缀开头，或按 `Unknown::Path` 收集的路径）。
    pub paths: Vec<&'a str>,
}

/// 解析命令行参数。
pub fn parse<'a>(
    raw: &'a [String],
    flags: &[Flag],
    unknown: Unknown,
) -> Result<Parsed<'a>, Error<'a>> {
    let mut out = Parsed::default();
    let mut i = 0usize;
    while i < raw.len() {
        let a = raw[i].as_str();
        let (body, prefixed) = if let Some(b) = a.strip_prefix('/') {
            (b, true)
        } else if let Some(b) = a.strip_prefix('-') {
            (b, true)
        } else {
            (a, false)
        };

        if prefixed {
            // 分离内联值（冒号形式）：/O:file
            let (name, inline) = match body.find(':') {
                Some(p) => (&body[..p], Some(&body[p + 1..])),
                None => (body, None),
            };
            let upper = name.to_ascii_uppercase();
            if let Some(flag) = flags.iter().find(|f| f.name == upper) {
                match flag.kind {
                    Kind::Flag => {
                        if inline.is_some() {
                            return Err(Error::UnexpectedValue(a));
                        }
                        out.flags.insert(flag.name, None);
                    }
                    Kind::Value => {
                        let value = match inline {
                            Some(v) if !v.is_empty() => v,
                            _ => {
                                i += 1;
                                if i >= raw.len() {
                                    return Err(Error::MissingValue(a));
                                }
                                raw[i].as_str()
                            }
                        };
                        out.flags.insert(flag.name, Some(value));
                    }
                    Kind::Ignore => {}
                }
            } else {
                match unknown {
                    Unknown::Path => out.paths.push(a),
                    Unknown::Error => return Err(Error::Unknown(a)),
                    Unknown::Ignore => {}
                }
            }
        } else {
            out.paths.push(a);
        }
        i += 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| s.to_string()).collect()
    }

    const SORT: &[Flag] = &[
        Flag::new("R", Kind::Flag),
        Flag::new("O", Kind::Value),
        Flag::new("M", Kind::Ignore),
        Flag::new("L", Kind::Ignore),
        Flag::new("REC", Kind::Ignore),
        Flag::new("T", Kind::Ignore),
    ];

    const WHERE: &[Flag] = &[
        Flag::new("R", Kind::Value),
        Flag::new("Q", Kind::Flag),
        Flag::new("F", Kind::Flag),
        Flag::new("T", Kind::Flag),
    ];

    #[test]
    fn linux_absolute_path_is_path() {
        let args = v(&["/tmp/s.txt"]);
        let p = parse(&args, SORT, Unknown::Path).unwrap();
        assert_eq!(p.paths, vec!["/tmp/s.txt"]);
        assert!(p.flags.is_empty());
    }

    #[test]
    fn path_starting_with_known_switch_name_not_swallowed() {
        // /tmp/x 曾因 /T 前缀匹配被误判为 tab 宽度
        let args = v(&["/tmp/x.txt"]);
        let p = parse(&args, SORT, Unknown::Path).unwrap();
        assert_eq!(p.paths, vec!["/tmp/x.txt"]);
        assert!(p.flags.is_empty());
    }

    #[test]
    fn ignore_kind_known_switches() {
        // /T 是 Ignore（已知但忽略），不会进 paths
        let args = v(&["/T", "in.txt"]);
        let p = parse(&args, SORT, Unknown::Path).unwrap();
        assert!(p.flags.is_empty());
        assert_eq!(p.paths, vec!["in.txt"]);
    }

    #[test]
    fn value_space_and_colon_forms() {
        let args = v(&["/O", "out.txt", "in.txt"]);
        let a = parse(&args, SORT, Unknown::Path).unwrap();
        assert_eq!(a.flags.get("O"), Some(&Some("out.txt")));
        assert_eq!(a.paths, vec!["in.txt"]);

        let args = v(&["/O:out.txt", "in.txt"]);
        let b = parse(&args, SORT, Unknown::Path).unwrap();
        assert_eq!(b.flags.get("O"), Some(&Some("out.txt")));
        assert_eq!(b.paths, vec!["in.txt"]);
    }

    #[test]
    fn flag_kind() {
        let args = v(&["/R", "x.txt"]);
        let p = parse(&args, SORT, Unknown::Path).unwrap();
        assert_eq!(p.flags.get("R"), Some(&None));
        assert_eq!(p.paths, vec!["x.txt"]);
    }

    #[test]
    fn case_insensitive() {
        // /r 小写匹配 R，且 R 是 Value 类型会消费下一个参数
        let args = v(&["/r", "dir", "/q", "/F"]);
        let p = parse(&args, WHERE, Unknown::Error).unwrap();
        assert_eq!(p.flags.get("R"), Some(&Some("dir")));
        assert!(p.flags.contains_key("Q"));
        assert!(p.flags.contains_key("F"));
    }

    #[test]
    fn unknown_policy_error() {
        let args = v(&["/usr/bin/ls"]);
        let e = parse(&args, WHERE, Unknown::Error).unwrap_err();
        assert_eq!(e, Error::Unknown("/usr/bin/ls"));
    }

    #[test]
    fn unknown_policy_ignore() {
        let args = v(&["/zzz", "a.txt"]);
        let p = parse(&args, SORT, Unknown::Ignore).unwrap();
        assert!(p.flags.is_empty());
        assert_eq!(p.paths, vec!["a.txt"]);
    }

    #[test]
    fn missing_value() {
        let args = v(&["/O"]);
        let e = parse(&args, SORT, Unknown::Path).unwrap_err();
        assert_eq!(e, Error::MissingValue("/O"));
    }

    #[test]
    fn unexpected_value() {
        let args = v(&["/R:1"]);
        let e = parse(&args, SORT, Unknown::Path).unwrap_err();
        assert_eq!(e, Error::UnexpectedValue("/R:1"));
    }

    #[test]
    fn bare_slash_is_unknown() {
        let args = v(&["/"]);
        let p = parse(&args, SORT, Unknown::Path).unwrap();
        assert_eq!(p.paths, vec!["/"]);
    }

    #[test]
    fn non_prefixed_args_always_paths() {
        // 相对路径、+n、域名等不以 / - 开头，永远收集，不受 unknown 策略影响
        let args = v(&["in.txt", "+3", "baidu.com"]);
        let p = parse(&args, SORT, Unknown::Error).unwrap();
        assert_eq!(p.paths, vec!["in.txt", "+3", "baidu.com"]);
    }

    #[test]
    fn value_consumes_next_even_if_switch_like() {
        // /O /R 会把 /R 当值消费（原版行为）
        let args = v(&["/O", "/R"]);
        let p = parse(&args, SORT, Unknown::Path).unwrap();
        assert_eq!(p.flags.get("O"), Some(&Some("/R")));
        assert!(!p.flags.contains_key("R"));
    }

    #[test]
    fn last_occurrence_wins() {
        let args = v(&["/FO", "csv", "/FO", "list"]);
        let p = parse(&args, &[Flag::new("FO", Kind::Value)], Unknown::Ignore).unwrap();
        assert_eq!(p.flags.get("FO"), Some(&Some("list")));
    }

    #[test]
    fn empty_inline_value_takes_next() {
        let args = v(&["/O:", "out.txt"]);
        let p = parse(&args, SORT, Unknown::Path).unwrap();
        assert_eq!(p.flags.get("O"), Some(&Some("out.txt")));
    }
}
