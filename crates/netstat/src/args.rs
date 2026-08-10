//! Windows 风格 netstat 参数解析。
//!
//! 实测原版行为：
//! - 支持组合选项（`-ano`）、`-p proto`（消费下一个参数）、`/` 前缀
//! - 纯数字参数为 interval（刷新间隔秒），0 视为无效
//! - 无效选项 / 无效 -p 值 / 多余参数：stderr 打印完整帮助 + exit 1
//! - 已实现：-a -b -n -o -p -q + interval；未实现：-e -f -i -r -s -t -x -y
//!   （在 [args::UNSUPPORTED] 中明确登记，不假装支持）

/// 已知但本实现未实现的选项。
pub const UNSUPPORTED: &[char] = &['e', 'f', 'i', 'r', 's', 't', 'x', 'y'];

#[derive(Debug, Default)]
pub struct Args {
    /// -a 显示所有连接和监听端口
    pub show_all: bool,
    /// -b 显示进程可执行文件名
    pub show_b: bool,
    /// -n 数字形式显示地址端口
    pub numeric: bool,
    /// -o 显示关联进程 PID
    pub show_pid: bool,
    /// -q 显示所有连接、监听端口和绑定未监听端口（BOUND）
    pub show_q: bool,
    /// -p 协议过滤（大写：TCP / UDP / TCPV6 / UDPV6）
    pub proto: Option<String>,
    /// interval：重复显示间隔（秒）
    pub interval: Option<u64>,
    /// 命中的未实现选项（用于报错）
    pub unsupported: Option<char>,
}

/// 解析失败时由调用方打印帮助并退出 1（与原版一致）。
pub fn parse(raw: &[String]) -> Result<Args, ()> {
    let mut args = Args::default();
    let mut i = 0usize;

    while i < raw.len() {
        let a = &raw[i];

        if let Some(body) = a.strip_prefix('-').or_else(|| a.strip_prefix('/')) {
            if body.is_empty() {
                return Err(());
            }
            let mut chars = body.chars();
            while let Some(ch) = chars.next() {
                match ch {
                    'a' => args.show_all = true,
                    'b' => args.show_b = true,
                    'n' => args.numeric = true,
                    'o' => args.show_pid = true,
                    'q' => args.show_q = true,
                    '?' => return Err(()),
                    'p' => {
                        // -p 消费下一个参数作为协议值
                        i += 1;
                        if i >= raw.len() {
                            return Err(());
                        }
                        args.proto = Some(raw[i].to_uppercase());
                    }
                    c if UNSUPPORTED.contains(&c) => args.unsupported = Some(c),
                    _ => return Err(()),
                }
            }
        } else if let Ok(secs) = a.parse::<u64>() {
            // interval 必须为正数
            if secs == 0 {
                return Err(());
            }
            args.interval = Some(secs);
        } else {
            // 非选项非数字参数：无效
            return Err(());
        }

        i += 1;
    }

    // 校验 -p 值（连接模式仅支持四个协议，原版大小写不敏感）
    if let Some(p) = &args.proto {
        if !matches!(p.as_str(), "TCP" | "UDP" | "TCPV6" | "UDPV6") {
            return Err(());
        }
    }

    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn defaults() {
        let a = parse(&v(&[])).unwrap();
        assert!(!a.show_all && !a.numeric && !a.show_pid);
        assert!(a.proto.is_none());
        assert!(a.interval.is_none());
    }

    #[test]
    fn combined_flags() {
        let a = parse(&v(&["-ano"])).unwrap();
        assert!(a.show_all && a.numeric && a.show_pid);
    }

    #[test]
    fn slash_prefix() {
        let a = parse(&v(&["/a", "/n"])).unwrap();
        assert!(a.show_all && a.numeric);
    }

    #[test]
    fn proto_value() {
        let a = parse(&v(&["-p", "tcp"])).unwrap();
        assert_eq!(a.proto.as_deref(), Some("TCP"));
        let a = parse(&v(&["-anop", "udpv6"])).unwrap();
        assert_eq!(a.proto.as_deref(), Some("UDPV6"));
        assert!(a.show_all && a.numeric && a.show_pid);
    }

    #[test]
    fn interval() {
        let a = parse(&v(&["-an", "5"])).unwrap();
        assert_eq!(a.interval, Some(5));
        assert!(parse(&v(&["0"])).is_err());
    }

    #[test]
    fn bad_option() {
        assert!(parse(&v(&["-z"])).is_err());
        assert!(parse(&v(&["-p", "bogus"])).is_err());
        assert!(parse(&v(&["foo"])).is_err());
        assert!(parse(&v(&["-p"])).is_err());
    }

    #[test]
    fn unsupported_flags() {
        for ch in UNSUPPORTED {
            let a = parse(&v(&[&format!("-{ch}")])).unwrap();
            assert_eq!(a.unsupported, Some(*ch));
        }
    }
}
