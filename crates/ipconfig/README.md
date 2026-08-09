# windowshit-ipconfig

用 Rust 重写的 Windows `ipconfig.exe`，跨平台运行（Windows / Linux / macOS），输出格式复刻 Windows 原版。

## 用法

```
ipconfig [/? | /all]
```

| 参数 | 说明 |
| --- | --- |
| （无参数） | 显示各适配器的 IP 地址、子网掩码、默认网关 |
| `/all` | 显示完整配置信息（Description、MAC、全部 IP、DNS 服务器） |
| `/?` | 显示帮助 |

## 还原的原版行为

- 首行 `Windows IP 配置` / `Windows IP Configuration`（跟随控制台代码页）
- 字段名 + `. . . . . . :` 点号对齐
- 适配器标题分类：`以太网适配器` / `隧道适配器` / `无线局域网适配器` / `未知适配器`
- 回环接口不显示
- `Media disconnected` 块
- `Autoconfiguration IPv4 Address`（169.254.x 自动配置）
- link-local IPv6 带 `%scope`（接口索引）
- 无参数不加 `(preferred)`，`/all` 才加
- 空值字段（如空 DNS Suffix）冒号后无尾随空格

## 未实现

以下原版选项不在帮助中显示，硬用报 `ERROR: The parameter is incorrect.`：

```
/flushdns /displaydns /renew /release /renew6 /release6
/registerdns /showclassid /setclassid /showclassid6 /setclassid6
/allcompartments
```

放弃原因（详见主 README 调研结论）：

- `/flushdns` `/displaydns`：没有可用的现成 crate（crates.io 无 DNS 缓存库；`windows` crate 只有裸 FFI，解析逻辑仍需自写），Linux/macOS 无统一 DNS 缓存接口
- `/renew` `/release` 系列：依赖 Windows DHCP 客户端服务，Linux 各发行版机制不同，风险高、演示价值低
- `/allcompartments`：Windows 网络隔离舱概念，其它平台不存在

## 平台注意

| 平台 | 权限要求 |
| --- | --- |
| Windows | 无需管理员 |
| Linux | 无需特权（只读网络信息），开箱即用 |
| macOS | 无需特权 |

Linux 上使用 `netdev` 采集数据：接口名（eth0/enp3s0…）分类为以太网，`docker0`/`virbr0`/`veth*` 等虚拟接口显示为 `未知适配器`。

## 构建

```bash
cargo build -p ipconfig
```

## 依赖

- `ipconfig`（Windows）：封装 `GetAdaptersAddresses`，即原版 ipconfig 的数据源
- `netdev`（Linux/macOS）：接口地址/前缀/MAC/默认网关/DNS 一站式采集
- `windowshit-i18n`：语言检测与翻译（workspace 内公共 crate）
