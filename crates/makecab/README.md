# makecab

Rust 重写的 Windows `makecab.exe`（跨平台整活）。

## 功能

- 将文件打包为 CAB（`cab` crate，MSZIP 压缩）
- 单文件模式：`makecab src [dest]`，默认目标名 = 源文件名末字符换 `_`
- `/F` 指令文件模式：多文件打包，默认生成 `disk1\1.cab` + `setup.inf` + `setup.rpt`
- `/D` 指令：`compressiontype`、`maxdisksize`、`cabinetname1`、`diskdirectorytemplate`、`compressionmemory`
- `/L` 输出目录、`/V[n]` 详细级别
- 原版风格的 CR 刷新进度条与统计块

## 用法

```
MAKECAB [/V[n]] [/D var=value ...] [/L dir] source [destination]
MAKECAB [/V[n]] [/D var=value ...] /F directive_file [...]
```

## 实测对齐的行为（Windows 11 原版）

| 场景 | 输出 | 退出码 |
| --- | --- | --- |
| 无参数 | 显示帮助 | 0 |
| 单文件打包 | `\r 0.00% - hello.txt (1 of 1)` → `100.00%` + flush 进度 | 0 |
| `/F` 多文件 | `Parsing directives` + `N bytes in N files` + 进度 + 统计块 | 0 |
| 源不存在 | `ERROR: Could not find file: X` | 1 |
| 无效压缩类型 | `ERROR: Invalid Compression Type: none` | 1 |
| `maxdisksize` 非数字 | `ERROR: Value of variable 'MaxDiskSize' must be a number: 0.1` | 1 |
| `maxdisksize` 非 512 倍数 | `ERROR: MaxDiskSize(1) is not a multiple of ClusterSize(512)` | 1 |

统计块：

```
Total files:              2
Bytes before:            50
Bytes after:             48
After/Before:            96.00% compression
Time:                     0.03 seconds ( 0 hr  0 min  0.03 sec)
Throughput:               1.68 Kb/second
```

## 已知差异

- **LZX 压缩不支持**（`cab` crate 只能解 LZX 不能写），请求时明确报错
- **分卷不支持**：`MaxDiskSize` 超限时明确报错，不会拆多卷
- 指令文件里的 `.set` / `.option` 指令忽略
- MSZIP 压缩实现与微软原版略有不同，`Bytes after` 可能偏差几个字节（不影响格式互操作，原版 `expand` 可正常解压本实现产物）
- `compressionmemory` 参数被解析但未使用（MSZIP 无内存级别概念）

## 互操作验证

本实现生成的 CAB 已用 Windows 原版 `expand.exe` 验证可正常解压，内容一致。
