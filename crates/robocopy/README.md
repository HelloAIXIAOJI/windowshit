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
| `/NP` `/NFL` `/NDL` `/NS` `/NC` | 输出控制 |
| `/NJH` `/NJS` | 关闭 Job Header / Summary |
| `/V` `/X` | 详细输出 / 报告所有 extra 文件 |
| `/?` | 显示帮助 |

## 还原的原版行为

- 输出恒为英文（不随系统语言变），`Started` / `Ended` 时间随 locale
- 文件分类：`New File` / `Newer` / `Older` / `Same` / `Changed` / `*EXTRA File` 等，默认"时间戳或大小不同则复制"
- 目录状态行：`  New Dir          <n>\t<path>`（数字 = 该目录匹配的文件数）
- Options 行固定顺序回显（`/E` 展开为 `/S /E`，`/MIR` 回显 `/PURGE /MIR`，末尾默认 `/R:1000000 /W:30`）
- 统计表格：`Dirs / Files / Bytes` 各 `Total / Copied / Skipped / Mismatch / FAILED / Extras` 列，`Times` 行
- 退出码位掩码：`0` 无变化 / `1` 已复制 / `2` 有额外 / `4` 有不匹配 / `8` 失败 / `16` 严重错误（`>=8` 为失败）
- 用法错误：无参数 / 缺 Destination / 源不存在 → 对应错误输出 + 退出码 16
- `/R:n` `/W:n` 失败重试

## 与真原版的已知差异

| 差异 | 说明 |
| --- | --- |
| `Started` / `Ended` 时区 | 暂按 UTC 输出（原版为本地时间），格式 `2026年8月11日 20:38:22` |
| 复制进度 | 恒为 `100%` 且紧跟文件名输出（原版动态刷新，重定向时用 `\r`） |
| `/COPY:DAT` 时间戳 | 复制后未设置目标文件 mtime（待阶段 4） |
| 属性比较 | 仅比较只读位（Tweaked 分类不精确，待阶段 2 完善） |
| `/IT` `/IS` | 未实现（Tweaked / Same 默认跳过） |
| 帮助文本 | 行尾空格未逐字节对齐（内容一致） |

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
