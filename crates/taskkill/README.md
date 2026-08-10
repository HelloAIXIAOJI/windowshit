# taskkill

Rust 重写的 Windows `taskkill.exe`（跨平台整活）。

## 功能

- `/PID`：按进程 ID 终止（支持多个 `/PID`）
- `/IM`：按映像名终止，支持 `*` `?` 通配符
- `/FI`：筛选器（`IMAGENAME`、`PID` 两字段，支持 `eq ne gt lt ge le`）
- `/T`：递归终止进程树（`kill_tree` 库内置）
- `/F`：强制终止（SIGKILL）；无 `/F` 发 SIGTERM
- `/S /U /P`：远程参数不支持，明确报错
- `/IM` 与 `/PID` 互斥（同原版）
- 退出码对齐原版：成功 0 / 语法错误 1 / 进程不存在 128

## 技术分工

| 职责 | 库 | 说明 |
|---|---|---|
| 枚举进程 | `sysinfo` | tasklist 已验证可靠 |
| 终止进程 | `kill_tree` | 跨平台统一库，内部封装平台 API |

`kill_tree` 内部实现：Windows 用 `OpenProcess` + `TerminateProcess`（这正是原版 taskkill 的做法），Linux/macOS 用 `nix` 发 `SIGTERM`/`SIGKILL`。**本文件无平台分支。**

## 与真原版的已知差异

1. **无 `/F` 时**：原版对控制台程序会失败（提示"只能使用 /F 强制终止"）；我们的实现发 SIGTERM，在 Windows 上 `kill_tree` 忽略信号直接强制终止，因此**无 `/F` 也能杀成功**。建议按原版习惯始终带 `/F`。
2. **无 `/T` 时连带杀子进程**：`kill_tree` 是递归库，杀目标时若目标有子进程会一并终止（实测 `Start-Process ping.exe` 会连带杀掉它的 conhost 子进程）。原版无 `/T` 只杀目标本身。对大多数无子进程的目标，两者行为一致。

## 实测（Windows 11）

```
> taskkill /pid 1234 /f
SUCCESS: The process with PID 1234 has been terminated.

> taskkill /im ping.exe /f
SUCCESS: The process "PING.EXE" with PID 5556 has been terminated.

> taskkill /pid 15152 /t /f
SUCCESS: The process with PID 10052 (child process of PID 15152) has been terminated.
SUCCESS: The process with PID 15152 has been terminated.
```

## 依赖

- `sysinfo`：进程枚举
- `kill_tree`：进程终止（跨平台统一）
- `windowshit-args` / `windowshit-i18n`：参数解析与本地化
