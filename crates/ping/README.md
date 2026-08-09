# windowshit-ping

用 Rust 重写的 Windows `ping.exe`，跨平台运行（Windows / Linux / macOS），输出格式复刻 Windows 原版（含各种"难绷"行为）。

## 用法

```
ping [-t] [-a] [-n count] [-l size] [-f] [-i TTL] [-v TOS]
     [-w timeout] [-S srcaddr] [-4] [-6] target_name
```

| 参数 | 说明 |
| --- | --- |
| `-t` | 持续 ping，Ctrl+C 停止并打印统计 |
| `-a` | 将 IP 地址反向解析为主机名 |
| `-n count` | 发送次数（默认 4，范围 1 ~ 4294967295） |
| `-l size` | 发送缓冲区大小（默认 32；非数字解析为 0，同原版） |
| `-f` | 设置"不拆分"标志（仅 IPv4） |
| `-i TTL` | 生存时间（1 ~ 255） |
| `-v TOS` | 服务类型（仅 IPv4） |
| `-w timeout` | 等待每次回复的超时毫秒数（默认 4000） |
| `-S srcaddr` | 指定源地址 |
| `-4` / `-6` | 强制 IPv4 / IPv6 |

支持 `-n 4` 和 `-n4` 两种写法，也支持 `/` 前缀（`/?`、`/t` 等，同原版）。

## 还原的原版行为

- 每次回复间隔 1 秒（快速回复时补足间隔）
- 超时等待满 `-w` 后再发下一个
- 输出文本、统计块（`已发送/已接收/丢失`、最短/最长/平均）、退出码（成功 0 / 失败 1）均与 Windows 一致
- 难绷行为也照抄：`-w abc` 不报错而是每包报 `PING: transmit failed. General failure.`、`-l abc` 按 0 字节、`-4 -6` 报"后出现"的选项
- 语言跟随控制台代码页：Windows `chcp 936` → 中文，`chcp 437`/`65001` → 英文

## 未实现

以下选项不在帮助中显示，硬用会明确报错（不假装支持）：

```
-r -s -j -k -R -c -p
```

## 平台注意

| 平台 | 权限要求 |
| --- | --- |
| Windows | 无需管理员（RAW ICMP 对 ICMP 豁免） |
| Linux | 需要 root 或 `CAP_NET_RAW`；普通用户回退 DGRAM 仍能 ping，但 TTL 显示为 0 |
| macOS | 普通用户可用 |

Linux 免 root 方案（二选一）：

```bash
# 允许所有用户使用 ping_group_range 内的 ICMP
sudo sysctl -w net.ipv4.ping_group_range="0 2147483647"
# 或给二进制授权
sudo setcap cap_net_raw+ep target/debug/ping
```

## 构建

```bash
cargo build -p ping
```

## 依赖

- `surge-ping`：ICMP 协议层（发送/接收/超时）
- `socket2`：socket 选项（TTL / 不分片 / TOS）
- `dns-lookup`：主机名解析与反解析
- `windowshit-i18n`：语言检测与翻译（workspace 内公共 crate）
