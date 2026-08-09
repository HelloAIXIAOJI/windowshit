# windowshit-tree

用 Rust 重写的 Windows `tree.exe`，跨平台运行，以图形方式显示目录结构。

## 用法

```
TREE [drive:][path] [/F] [/A]
```

| 参数 | 说明 |
| --- | --- |
| `/F` | 显示每个文件夹中的文件名 |
| `/A` | 使用 ASCII 字符（`|--`/`` `-- ``）代替扩展字符 |

## 还原的原版行为

- 首行 `Folder PATH listing`（跟随语言：`文件夹 PATH 列表`）
- Windows 显示卷序列号（`Volume serial number is XXXX-XXXX`）
- 根路径行（Windows 上显示大写路径）
- 树形结构：`├──` / `└──` / `│`（`/A` 时用 ASCII）
- 无子文件夹时显示 `No subfolders exist`

## 技术说明

- 纯 `std::fs` 递归遍历，跨平台无平台分支
- 卷序列号仅 Windows 存在（`GetVolumeInformationW`），Linux/macOS 不显示

## 平台注意

| 平台 | 说明 |
| --- | --- |
| Windows | 无需权限 |
| Linux / macOS | 无需权限 |

## 构建

```bash
cargo build -p tree
```

## 依赖

- `windowshit-i18n`：输出文本本地化
- `windows-sys`：Windows 卷序列号（仅 Windows 分支）
