# windowshit-getmac

用 Rust 重写的 Windows `getmac.exe`，跨平台运行，显示网卡 MAC 地址。

## 用法

```
GETMAC [/FO format] [/NH] [/V]
```

| 参数 | 说明 |
| --- | --- |
| `/FO format` | 输出格式：`TABLE`（默认）、`LIST`、`CSV` |
| `/NH` | 不显示列标题（仅 TABLE/CSV） |
| `/V` | 详细输出（LIST 时显示连接名称与网卡适配器） |

## 还原的原版行为

- 默认 TABLE：`Physical Address` + `Transport Name` 两列，对齐 + 分隔线
- `Transport Name`：Windows 上显示 `\Device\Tcpip_{GUID}`（从注册表反查接口 GUID），未连接的适配器显示 `Media disconnected`
- `/v /fo list` 显示 Connection Name / Network Adapter / Physical Address / Transport Name
- 不显示回环、隧道（Teredo）、无物理地址（Wintun）的适配器

## 技术说明

- Windows：`ipconfig` crate（GetAdaptersAddresses）+ 注册表 GUID 反查
- Linux/macOS：`netdev`（接口名作为 transport name）
- 输出格式跨平台共用

## 平台注意

| 平台 | 说明 |
| --- | --- |
| Windows | 无需权限 |
| Linux / macOS | 无需权限 |

## 构建

```bash
cargo build -p getmac
```

## 依赖

- `ipconfig`（Windows）：适配器数据
- `netdev`（Linux/macOS）：接口数据
- `windows-sys`：注册表 GUID 反查（仅 Windows 分支）
- `windowshit-i18n`：表头与字段名本地化
