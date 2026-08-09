# windowshit-tasklist

用 Rust 重写的 Windows `tasklist.exe`，跨平台运行，显示当前运行的进程列表。

## 用法

```
TASKLIST [/FO format] [/NH]
```

| 参数 | 说明 |
| --- | --- |
| `/FO format` | 输出格式：`TABLE`（默认）、`LIST`、`CSV` |
| `/NH` | 不显示列标题（仅 TABLE/CSV） |

## 还原的原版行为

- 表头：`Image Name` `PID` `Session Name` `Session#` `Mem Usage`，列宽与分隔线精确还原
- 会话名：`Services`（会话 0）/ `Console`（非 0）
- 内存格式：千分位 + ` K`（如 `40,240 K`）
- 进程按 PID 排序
- `/fo list` 每个进程一块，`/fo csv` 引号分隔

## 技术说明

- 进程数据复用 `sysinfo` crate（跨平台）
- 会话信息 Windows 用 `WTSEnumerateProcessesW` 一次性枚举（普通权限可用，原版同机制）；`ProcessIdToSessionId` 对跨会话/受保护进程返回 ACCESS_DENIED，仅作兜底
- 系统进程（Session 0）的内存可能显示 0 K（sysinfo 读不到受保护进程的工作集，原版可读）

## 平台注意

| 平台 | 说明 |
| --- | --- |
| Windows | 无需权限 |
| Linux / macOS | 无需权限（会话显示 N/A/0） |

## 构建

```bash
cargo build -p tasklist
```

## 依赖

- `sysinfo`：跨平台进程枚举
- `windows-sys`：Windows 会话枚举（仅 Windows 分支）
- `windowshit-i18n`：表头本地化
