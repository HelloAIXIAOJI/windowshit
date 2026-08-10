//! services 文件解析：端口 → 服务名。
//!
//! 复刻原版 netstat 非 `-n` 模式下 foreign 端口的服务名显示
//! （如 `443 → https`、`5357 → wsd`）。数据源：
//! - Windows：`%SystemRoot%\System32\drivers\etc\services`
//! - Linux/macOS：`/etc/services`
//!
//! 文件缺失或查不到时端口保持数字。格式：`<name> <port>/<protocol> [#注释]`。

use std::collections::HashMap;
use std::path::PathBuf;

pub struct Services {
    tcp: HashMap<u16, String>,
    udp: HashMap<u16, String>,
}

impl Services {
    pub fn load() -> Self {
        let path = services_path();
        let mut tcp: HashMap<u16, String> = HashMap::new();
        let mut udp: HashMap<u16, String> = HashMap::new();

        if let Ok(text) = std::fs::read_to_string(&path) {
            for raw in text.lines() {
                // 去掉注释，取本行主体
                let line = raw.split('#').next().unwrap_or("").trim();
                if line.is_empty() {
                    continue;
                }
                let mut it = line.split_whitespace();
                let name = match it.next() {
                    Some(n) => n,
                    None => continue,
                };
                let spec = match it.next() {
                    Some(s) => s,
                    None => continue,
                };
                let (port_s, proto) = match spec.split_once('/') {
                    Some(p) => p,
                    None => continue,
                };
                let Ok(port) = port_s.parse::<u16>() else {
                    continue;
                };
                let map = match proto.to_ascii_lowercase().as_str() {
                    "tcp" => &mut tcp,
                    "udp" => &mut udp,
                    _ => continue,
                };
                // 同名端口取第一条（与 IANA 常规条目一致）
                map.entry(port).or_insert_with(|| name.to_string());
            }
        }

        Services { tcp, udp }
    }

    /// TCP 端口 → 服务名；查不到返回端口数字。
    pub fn tcp_name(&self, port: u16) -> String {
        self.tcp
            .get(&port)
            .cloned()
            .unwrap_or_else(|| port.to_string())
    }

    /// UDP 端口 → 服务名；查不到返回端口数字。
    pub fn udp_name(&self, port: u16) -> String {
        self.udp
            .get(&port)
            .cloned()
            .unwrap_or_else(|| port.to_string())
    }
}

fn services_path() -> PathBuf {
    #[cfg(windows)]
    {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
        PathBuf::from(root)
            .join("System32")
            .join("drivers")
            .join("etc")
            .join("services")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/etc/services")
    }
}
