# windowshit-pathping

用 Rust 重写的 Windows `pathping.exe`，跨平台运行（Windows / Linux / macOS），输出格式复刻 Windows 原版（两阶段：路由跟踪 + 逐跳丢包统计）。

## 用法

```
pathping [-n] [-h maximum_hops] [-i address] [-p period]
         [-q num_queries] [-w timeout] [-4] [-6] target_name
```

| 参数 | 说明 |
| --- | --- |
| `-h maximum_hops` | 搜索目标的最大跃点数（默认 30，范围 1 ~ 255） |
| `-i address` | 指定源地址 |
| `-n` | 不将地址解析成主机名 |
| `-p period` | 两次 ping 之间的间隔毫秒数（默认 250） |
| `-q num_queries` | 每跳查询次数（默认 100） |
| `-w timeout` | 每次回复的等待毫秒数（默认 3000） |
| `-4` / `-6` | 强制 IPv4 / IPv6 |

## 输出结构（复刻原版）

**阶段 1**：路由跟踪（hop 0 = 本机，与 tracert 相同技术）

```
Tracing route to 192.168.1.1 over a maximum of 4 hops

  0  192.168.0.105
  1  192.168.0.1
  2  192.168.1.1
```

**阶段 2**：逐跳 ping 统计，输出 Source to Here / This Node/Link 双列 + 竖线

```
Computing statistics for 0 seconds...
            Source to Here   This Node/Link
Hop  RTT    Lost/Sent = Pct  Lost/Sent = Pct  Address
  0                                           192.168.0.105
                                 0/   1 =  0%   |
  1    0ms     0/   1 =  0%     0/   1 =  0%  192.168.0.1
                                 0/   1 =  0%   |
  2    1ms     0/   1 =  0%     0/   1 =  0%  192.168.1.1

Trace complete.
```

- `Source to Here` = 到该跳的累计丢包；`This Node/Link` = 该段链路的丢包（相邻两跳差）
- `Computing statistics for N seconds...`：`N = ceil((q-1) * p / 1000)`（还原原版公式）
- 表头列标签保留英文（与原版一致，任何语言版本均如此）

## 未实现

`-g host-list`（松散源路由）不在帮助中显示，硬用明确报错。

## 技术说明

- 阶段 1 复用 `windowshit-trace-core`（workspace 公共探测层，与 tracert 同源）
- 阶段 2 用 `surge-ping` 对每个路由节点连续 ping `-q` 次统计丢包与平均 RTT
- 平台权限要求与 tracert 相同：Windows 免管理员；Linux 需 root 或 `CAP_NET_RAW`

## 构建

```bash
cargo build -p pathping
```

## 依赖

- `windowshit-trace-core`：路由探测（RAW ICMP + 逐跳 TTL）
- `surge-ping`：阶段 2 逐跳 ping 统计
- `dns-lookup`：地址解析与反解析
- `windowshit-i18n`：语言检测与翻译
