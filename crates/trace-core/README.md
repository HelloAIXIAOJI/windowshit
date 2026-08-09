# windowshit-trace-core

Windowshit 项目公共 traceroute 探测层（库 crate），被 `tracert`（每跳 3 探测）和 `pathping`（路由阶段，每跳 1 探测）使用。

## 功能

- RAW/DGRAM ICMP socket 创建（平台自动选择）
- 逐跳递增 TTL 的 Echo Request 发送
- TTL 过期 / 目标不可达 / Echo Reply 回复解析与匹配
- 内嵌原始包的 id/seq 匹配（IPv4 偏移 28，IPv6 偏移 48，逐字节实测校准）

## 用法

```rust
use windowshit_trace_core::{trace, TraceConfig};

let hops = trace(&TraceConfig {
    ip,                 // 目标地址
    max_hops: 30,       // 最大跳数
    wait: Duration::from_millis(4000),
    probes_per_hop: 1,  // 每跳探测次数
    src: None,          // 源地址（可选）
})?;
```

返回每跳的 `Hop { rtts, ip }`。

## 关键经验

- **Windows 与 Linux 的 DGRAM ICMP socket 都收不到 TimeExceeded**（ICMP 错误消息不投递给 DGRAM socket），因此必须用 RAW socket。Windows 对 ICMP 的 RAW 免管理员；Linux 需 root 或 `CAP_NET_RAW`；macOS/BSD 用 DGRAM 免特权。
- 内嵌 echo 的 id/seq 偏移：IPv4 = 28（ICMP unused4 + 内嵌 IPv4 头 20 + 内嵌 echo 头 4），IPv6 = 48。

## 依赖

- `socket2`：RAW/DGRAM ICMP socket 与逐跳 TTL 设置
- `nex-packet`：ICMP 包构造与 IP/ICMP 回复解析（校验和等）
