# windowshit-netinfo

Windowshit 公共网络适配器数据层（库 crate）。

## 职责

统一采集网络适配器数据，让组件无需写平台分支：

| 平台 | 底层 | 说明 |
|---|---|---|
| Windows | `ipconfig` crate | 封装 GetAdaptersAddresses |
| Linux/macOS | `netdev` crate | 读 /proc/net 与 netlink |

## 数据结构

`AdapterData`：

- `friendly_name` / `description`
- `mac: Option<Vec<u8>>`
- `ipv4: Vec<(Ipv4Addr, u8)>`、`ipv6: Vec<(Ipv6Addr, u8)>`（IP + 前缀长度）
- `ipv6_scope: Option<u32>`（IPv6 接口索引）
- `gateways` / `dns`
- `is_up`、`kind`（Ethernet / Loopback / Tunnel / Wireless / Other）

辅助函数：`get_adapters()`、`prefix_to_mask4(bits)`。

## 平台差异

- 回环接口在 Windows（SoftwareLoopback）与 Linux（`lo*`）都被跳过，还原原版 ipconfig 行为
- unix 侧 `description` 为空、`friendly_name` 为接口名（`ens33` 等）
- 网关来自默认路由设备

## 使用方

`ipconfig`（完整字段）、`getmac`（MAC / transport）、`systeminfo`（描述/友好名/IP）。
