# windowshit-tracert

用 Rust 重写的 Windows `tracert.exe`，跨平台运行（Windows / Linux / macOS），输出格式复刻 Windows 原版。

## 用法

```
tracert [-d] [-h maximum_hops] [-w timeout] [-S srcaddr] [-4] [-6] target_name
```

| 参数 | 说明 |
| --- | --- |
| `-d` | 不将地址解析成主机名 |
| `-h maximum_hops` | 搜索目标的最大跃点数（默认 30，范围 1 ~ 255） |
| `-w timeout` | 等待每个回复的超时毫秒数（默认 4000） |
| `-S srcaddr` | 指定源地址 |
| `-4` / `-6` | 强制 IPv4 / IPv6 |

## 还原的原版行为

- 两行 header：`Tracing route to {target} [{ip}]` + `over a maximum of {n} hops:`（`-d` + IP 字面时单行无 `[ip]`，还原原版差异）
- 每跳发送 **3 个探测包**，显示 3 列 RTT（`<1 ms` / 右对齐毫秒 / `*` 超时）
- 地址列：反解析成功显示 `hostname [ip]`（即使 hostname 等于 ip），失败显示纯 IP
- 收到 EchoReply 或 DestinationUnreachable 即视为到达目标，停止探测
- `Trace complete.` 结尾
- 语言跟随控制台代码页（同 ping）

## 未实现

以下选项不在帮助中显示，硬用明确报错：

```
-j -R
```

## 技术说明

- 使用 RAW ICMP socket + 逐跳递增 TTL，从 TTL 过期（Time Exceeded）回复中提取路由器地址
- 内嵌 echo 的匹配偏移：IPv4 = 28（unused4 + IP头20 + echo头4），IPv6 = 48，逐字节实测校准
- 调研结论：`tracert` crate 0.12 无法还原 Windows 行为（每跳 1 探测、无 `*`、不处理 TimeExceeded），故自行实现控制流，网络层复用 `nex-packet`

## 平台注意

| 平台 | 权限要求 |
| --- | --- |
| Windows | 无需管理员（RAW ICMP 对 ICMP 豁免） |
| Linux | **必须 root 或 `CAP_NET_RAW`**（DGRAM 收不到 TimeExceeded，普通用户会全跳超时） |
| macOS | 普通用户可用 |

Linux 授权：

```bash
sudo setcap cap_net_raw+ep target/debug/tracert
```

## 构建

```bash
cargo build -p tracert
```

## 依赖

- `socket2`：DGRAM/RAW ICMP socket 与逐跳 TTL 设置
- `nex-packet`：ICMP 包构造与 IP/ICMP 回复解析（校验和等）
- `dns-lookup`：地址解析与反解析
- `windowshit-i18n`：语言检测与翻译（workspace 内公共 crate）
