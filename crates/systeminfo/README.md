# windowshit-systeminfo

用 Rust 重写的 Windows `systeminfo.exe`，跨平台运行，显示系统详细信息。

## 用法

```
SYSTEMINFO [/FO format]
```

无参数显示系统信息，`/?` 显示帮助。

## 还原的原版行为

- 字段对齐：label 补空格到固定列，冒号列位置与原版一致
- 字段（全平台）：Host Name、OS Name、OS Version、System Boot Time（本地时区）、Processor(s)、内存（Total/Available/Virtual）、Network Card(s)
- 字段（Windows 专属，注册表读取）：Registered Owner、Product ID、Original Install Date、System Manufacturer/Model、BIOS Version、Windows/System Directory、System Type
- 内存格式：千分位 + ` MB`
- 网络卡：description + Connection Name + IP 列表，跳回环与隧道接口
- 不列回环/隧道接口

## 技术说明

- 数据复用 `sysinfo`（内存/CPU/启动时间）、`os_info`（OS 版本）、`hostname`、`ipconfig`/`netdev`（网卡）
- Windows 专属字段（Product ID、安装日期、厂商、型号、BIOS 等）从注册表读取
- 本地时间用 `chrono` 转换（Boot Time 原版为本地时间）

## 平台注意

| 平台 | 说明 |
| --- | --- |
| Windows | 无需权限 |
| Linux / macOS | 无需权限（Windows 专属字段省略） |

## 构建

```bash
cargo build -p systeminfo
```

## 依赖

- `sysinfo`：内存 / CPU / 启动时间
- `os_info`：操作系统版本
- `hostname`：主机名
- `ipconfig`（Windows）/ `netdev`（unix）：网络卡
- `chrono`：本地时间转换
- `windowshit-i18n`：字段标签本地化
