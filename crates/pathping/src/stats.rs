//! 阶段 2：对每个路由节点做 q 次 ping 统计。
//!
//! 复用 surge-ping 的 ICMP 层，不重复实现发包/等待逻辑。

use std::net::IpAddr;
use std::time::Duration;

use surge_ping::{Client, Config, PingIdentifier, PingSequence};

pub struct Stats {
    pub lost: u32,
    rtts: Vec<u128>,
}

impl Stats {
    pub fn avg_rtt_ms(&self) -> Option<u128> {
        if self.rtts.is_empty() {
            None
        } else {
            Some(self.rtts.iter().sum::<u128>() / self.rtts.len() as u128)
        }
    }
}

/// 对 addrs 中的每个地址连续 ping `queries` 次，返回统计。
pub async fn collect(addrs: &[IpAddr], queries: u32, wait_ms: u64) -> Vec<Stats> {
    let config = Config::builder()
        // RAW 优先：Linux root 下可用；失败自动回退 DGRAM
        .sock_type_hint(socket2::Type::RAW)
        .build();
    let client = match Client::new(&config) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let ident = PingIdentifier((std::process::id() & 0xFFFF) as u16);
    let payload = vec![0u8; 32];

    let mut out = Vec::with_capacity(addrs.len());
    for ip in addrs {
        let mut st = Stats {
            lost: queries,
            rtts: Vec::new(),
        };
        let mut pinger = client.pinger(*ip, ident).await;
        for s in 0..queries {
            pinger.timeout(Duration::from_millis(wait_ms));
            if let Ok((_, dur)) = pinger.ping(PingSequence(s as u16), &payload).await {
                st.lost = st.lost.saturating_sub(1);
                st.rtts.push(dur.as_millis());
            }
        }
        out.push(st);
    }
    out
}
