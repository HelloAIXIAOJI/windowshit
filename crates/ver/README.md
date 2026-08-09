# windowshit-ver

用 Rust 重写的 Windows `ver.exe`，跨平台运行，输出系统版本信息。

## 用法

```
ver
```

无参数，输出一行系统版本。

## 还原的原版行为

Windows 上还原原版格式（任何语言版本的 ver 都是英文文本，故不随代码页翻译）：

```
Microsoft Windows [Version 10.0.22621.2428]
```

版本号包含完整 build 号（含 UBR 修订号）。

跨平台时按当前系统显示：

| 平台 | 输出 |
| --- | --- |
| Windows | `Microsoft Windows [Version ...]` |
| macOS | `macOS [Version ...]` |
| Linux | `Linux [Version ...]` |

## 技术说明

- 用 `os_info` crate 获取 OS 类型与版本（跨平台，无平台分支）
- Windows 的 build 修订号（UBR，如 `.2428`）`os_info` 不提供，从注册表 `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\UBR` 补全（REG_DWORD，仅 Windows 存在此字段）

## 平台注意

| 平台 | 说明 |
| --- | --- |
| Windows | 无需权限 |
| Linux / macOS | 无需权限 |

## 构建

```bash
cargo build -p ver
```

## 依赖

- `os_info`：跨平台获取操作系统类型与版本
- `windows-sys`：Windows 注册表读取 UBR（仅 Windows 分支）
