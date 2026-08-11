# windowshit-robocopy

用 Rust 重写的 Windows `robocopy.exe`（Robust File Copy），跨平台运行，复刻其目录复制 / 镜像 / 移动语义、状态行输出、统计表格与位掩码退出码。

## 用法

```
ROBOCOPY source destination [file [file]...] [options]
```

| 参数 | 说明 |
| --- | --- |
| `source` / `destination` | 源 / 目标目录 |
| `file` | 要复制的文件（支持通配符，默认 `*.*`） |
| `/S` | 复制子目录（不含空目录） |
| `/E` | 复制子目录（含空目录） |
| `/MIR` | 镜像目录树（= `/E` + `/PURGE`） |
| `/PURGE` | 删除目标中源不存在的文件/目录 |
| `/MOV` | 复制后删除源文件 |
| `/MOVE` | 移动文件及目录 |
| `/L` | 仅列出，不实际复制/删除 |
| `/R:n` `/W:n` | 复制失败重试次数 / 等待秒数（默认 100 万 / 30） |
| `/MT[:n]` | 多线程复制（默认 8 线程，1..128） |
| `/Z` | 可重启模式（断点续传） |
| `/CREATE` | 只建目录树和零字节文件 |
| `/NP` `/NFL` `/NDL` `/NS` `/NC` | 输出控制 |
| `/NJH` `/NJS` | 关闭 Job Header / Summary |
| `/V` `/X` | 详细输出 / 报告所有 extra 文件 |
| `/XF file...` | 排除文件（通配符，多值） |
| `/XD dir...` | 排除目录（通配符，多值） |
| `/XC` `/XN` `/XO` | 排除 Changed / Newer / Older |
| `/XL` `/XX` | 排除 Lonely（源有目标无）/ Extra（目标多余） |
| `/IS` `/IT` | 包含 Same / Tweaked |
| `/MAX:n` `/MIN:n` | 按大小过滤 |
| `/MAXAGE` `/MINAGE` | 按修改时间过滤（天） |
| `/MAXLAD` `/MINLAD` | 按最后访问时间过滤（天） |
| `/LEV:n` | 只复制前 n 层子目录 |
| `/A` `/M` | 归档位过滤 / 复制并清除归档位 |
| `/IA:attrs` `/XA:attrs` | 按属性包含 / 排除 |
| `/XJ` `/XJF` `/XJD` | 排除 junction / junction 文件 / junction 目录 |
| `/V` | 详细输出（显示 skipped 文件，小写分类） |
| `/TS` `/FP` `/BYTES` | 文件行时间戳 / 完整路径 / Options 回显 |
| `/ETA` | 文件行尾显示预计完成时间 |
| `/LOG:file` `/LOG+:file` | 写日志文件 / 追加 |
| `/UNILOG:file` `/UNILOG+:file` | 写 Unicode（UTF-16LE）日志文件 / 追加 |
| `/UNICODE` | 以 Unicode 输出（本实现 stdout 恒为 UTF-8，等价接受） |
| `/TEE` | 控制台 + 日志双输出 |
| `/FFT` | 按 FAT 文件时间（2 秒粒度）比较 |
| `/?` | 显示帮助 |

## 还原的原版行为

- 输出恒为英文（不随系统语言变），`Started` / `Ended` 时间随 locale
- 文件分类：`New File` / `Newer` / `Older` / `Same` / `Changed` / `*EXTRA File` 等，默认"时间戳或大小不同则复制"
- 目录状态行：`  New Dir          <n>\t<path>`（数字 = 该目录匹配的文件数）
- Options 行固定顺序回显（`/E` 展开为 `/S /E`，`/MIR` 回显 `/PURGE /MIR`，末尾默认 `/R:1000000 /W:30`）
- 统计表格：`Dirs / Files / Bytes` 各 `Total / Copied / Skipped / Mismatch / FAILED / Extras` 列，`Times` 行
- 退出码位掩码：`0` 无变化 / `1` 已复制 / `2` 有额外 / `4` 有不匹配 / `8` 失败 / `16` 严重错误（`>=8` 为失败）
- 用法错误：无参数 / 缺 Destination / 源不存在 → 对应错误输出 + 退出码 16
- `/R:n` `/W:n` 失败重试（默认 100 万次 / 30 秒，对齐原版）
- `/MT[:n]` 多线程复制：目录内文件并行、输出保持提交顺序；无目录行；文件行强制完整路径；Dirs 统计 Copied 恒等于 Total（原版怪癖）、Skipped 单独记已存在目录
- `/Z` 断点续传：目标存在部分数据（0 < 目标大小 < 源大小）时从源偏移继续，实测 1MiB 文件断点续传内容逐字节一致
- `/CREATE` 只建零字节文件并还原源 mtime/属性；统计仍按源大小（原版行为，目标 0 字节）
- 默认报告 extra：`*EXTRA Dir -1` 目录行 + 顶层 `*EXTRA File` 行，统计 Extras、退出码置 2（/L 也报告并统计；`/XX` 不报告行但仍统计；`/PURGE` 递归列出目录内文件并删除）
- `/IS` 复制 Same 文件（标签 `Same      `）、`/IT` 复制 Tweaked 文件（标签 `Tweaked   `），Options 行回显于 `/NP` 后
- `/COPY:DAT` 语义：复制后目标 mtime 与属性 = 源；覆盖只读目标前先清除只读位（原版行为）
- 重定向时进度：文件状态行后接 `\r100%  ` 独立进度行（无 `/NP` 时）
- TTY 动态进度：逐块（1MiB）复制时 `\r` 覆盖式刷新百分比（`3.1%`…`100%`，一位小数）
- 文件大小显示（实测 2026-08-11）：文件行 `<1 MiB` 字节数右对齐 8、`≥1 MiB` 用 `{:.1} m`；summary Bytes 列 `<1 KiB` 字节数、`<1 MiB` 用 `{:.1} k`、`≥1 MiB` 用 `{:.2} m`
- Speed 行：`Bytes/sec.` 千位分隔整数、`MegaBytes/min.` 千位分隔三位小数，数字右对齐 23
- `/XF` `/XD` 在 Header 有独立行：`Exc Files :` / `Exc Dirs :`
- 文件选择在**分类前**过滤（排除的文件不计入复制但计入目录行数字）
- `/XJ` 排除 junction（Rust std 在 Windows 对 junction 的 `is_dir()` 返回 false，需用 reparse point 位判断）
- `/TS`：文件行大小后显示源文件 UTC 时间戳 `YYYY/MM/DD HH:MM:SS`；`/FP` 显示源完整路径；`/V` 的 skipped 行小写分类右对齐 14 宽（`          same`）
- `/LOG:file`：stdout 只输出 ` Log File : path`，完整输出写入日志（`Started`/`Ended` 用数字格式 `2026811 23:30:49`，不随 locale）；`/LOG+` 追加；`/TEE` 控制台与日志双写
- `/UNILOG:file` `/UNILOG+:file`：同 `/LOG` 但日志文件写 UTF-16LE（带 BOM），与 UTF-8/ANSI 的 `/LOG` 区分；`/UNICODE` 表示 Unicode 输出，本实现 stdout 恒为 UTF-8，接受开关即等价
- `/FFT`：比较前把源/目标 mtime 向下取整到 2 秒（FAT 粒度），对齐原版 FAT 时间补偿
- `/ETA`：文件行尾 `\t\tHH:MM -> HH:MM`（非首个复制文件显示，预计时间基于已复制字节的实测平均速率估算）

## 与真原版的已知差异

以下差异已处理：`/FFT` FAT 时间补偿、`/ETA` 实测速率估算、Unix 平台本地时间（libc `localtime_r`）、`/UNILOG` `/UNILOG+` `/UNICODE`、`/XD` 排除目录计入 Dirs Total。剩余差异均为平台固有、原版行为不稳定或有意的设计取舍：

| 差异 | 说明 |
| --- | --- |
| `Started` / `Ended` 行尾 | 原版行尾空格随系统 locale 与字符显示宽度变化，无法在无原版环境下精确复刻；主格式已对齐 |
| TTY 进度动画细节 | 已复刻动态百分比（1MB 块、一位小数）；终端捕获工具下 `\r` 渲染方式属终端行为，可能与原版有细微视觉差 |
| Unix 属性 | `/IA` `/XA` 仅支持 R（只读）；Unix 无 archive / hidden / system 等属性，为平台固有差异 |
| `/MAXLAD` `/MINLAD` | 已实现；Windows 下最后访问时间更新策略与 NTFS 相关，极端场景可能有差异 |
| `/MT` + `/V` 的 skipped 标签 | 中文 locale 下原版部分文件显示本地化标签（如"相同"）且输出不稳定；本实现统一英文 |
| `/MT` 全局并行度 | 本实现按目录粒度批次并行（同目录多文件并行、跨目录串行），输出顺序稳定；原版全局线程池、输出乱序。此为有意设计，便于逐字节 diff |
| `/CREATE` 复制耗时 | 只创建空文件（不写数据），实际吞吐高于原版；Speed 行为动态值，对比时需归一化 |

## 平台注意

| 平台 | 说明 |
| --- | --- |
| Windows | 无需权限 |
| Linux / macOS | 无需权限 |

## 构建

```bash
cargo build -p robocopy
```

## 依赖

- `windowshit-args`：Windows 风格参数解析（未知开关静默忽略）
