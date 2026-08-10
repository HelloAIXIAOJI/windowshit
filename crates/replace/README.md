# replace

Rust 重写的 Windows `replace.exe`（跨平台整活）。

## 功能

- 用源目录文件替换目标目录中的同名文件
- 支持通配符源（`*.txt`）
- `/a` 添加：只把目标不存在的源文件添加到目标目录
- `/s` 递归：替换目标目录所有子目录中的同名文件
- `/u` 更新：仅替换目标比源旧的（按 mtime）
- `/p` 逐个确认、`/r` 替换只读文件、`/w` 等待插入磁盘

## 实测对齐的行为（Windows 原版）

| 场景 | 输出 | 退出码 |
|---|---|---|
| 无参数 | `No files replaced` + 红色 `Source path required` | 11 |
| 正常替换 | `Replacing <dst\file>` | 0 |
| `/a` 添加 | `Adding <dst\file>` | 0 |
| 源文件不存在 | 静默 | 0 |
| 源是目录 / 通配符无匹配 | `No files replaced` + 红色 `No files found - X` | 2 |
| 无效开关 | `No files replaced` + 红色 `Invalid switch - /x` | 11 |

## 实现细节

- 纯 `std`（glob 展开、递归枚举、mtime 比较都手写，无第三方依赖）
- 输出恒为英文 + ANSI 红色错误（`\x1b[31;1m`），与原版一致（不随系统语言变）
- `/r` 处理只读文件：Windows 上 `SetFileAttributesW` 清属性后重试

## 与真原版的已知差异

- 原版 `/a` 对目标已存在文件的处理细节未完全验证，本实现为"跳过不覆盖"
- 通配符仅支持最后一段（如 `dir\*.txt`），多段通配符（`dir\*\*.txt`）不支持
