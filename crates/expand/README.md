# expand

Rust 重写的 Windows `expand.exe`（跨平台整活）。

## 功能

- 解压 CAB 压缩包（`cab` crate）
- 解压 SZDD 单文件压缩格式（`compress.exe` 产物，如 `.ex_`/`.dl_`，手写 LZSS 解码）
- `-d` 只列出归档内文件
- `-F:files` 按通配符过滤提取
- `-R` / `-I` 重命名规则
- 通配符多文件源复制

## 用法

```
EXPAND [-R] Source Destination
EXPAND -R Source [Destination]
EXPAND -I Source [Destination]
EXPAND -D Source.cab [-F:Files]
EXPAND Source.cab -F:Files Destination
```

| 参数 | 说明 |
| --- | --- |
| `-R` | 重命名展开文件（用归档内文件名） |
| `-I` | 重命名但忽略目录结构 |
| `-D` | 显示源中文件列表，不解压 |
| `-F:files` | 从 CAB 中按通配符指定要展开的文件 |
| `Source` | 源文件规范（支持 `*` `?` 通配符） |
| `Destination` | 目标文件 / 路径 |

## 实测对齐的行为（Windows 11 原版）

| 场景 | 输出 | 退出码 |
| --- | --- | --- |
| 无参数 | `No files specified.` | 0 |
| `-d` 列表 | `src.cab: 文件名`，多文件追加 `N files total.` | 0 |
| 解压到目录 | `Adding dst\X to Extraction Queue` + `Expanding Files ....` + `Expanding Files Complete ...` | 0 |
| 多文件 CAB 无 `-F` | 提示需 `-F:*`，但退出码仍为 **0** | 0 |
| 源打不开 | `Can't open input file: X.` | 255 |
| 通配符多文件源 | `Copying X to Y.` + `X: N bytes copied.` + `Total increase: ...` | 0 |

命名规则：无 `-R`/`-I` 时单文件 CAB 用**源文件名**（`expand hello.cab dir` → `dir\hello.cab`）；用 `-F` 提取或带 `-R`/`-I` 时用**归档内文件名**（→ `dir\hello.txt`）。

## 已知差异

- 目标路径不存在时按文件创建（与原版一致，不建目录）
- SZDD 支持标准的 `SZDD 88F02733` 头；KWAJ 格式不支持
- 解压输出中不重复原版对绝对路径的规范化显示（保留用户输入形式）
