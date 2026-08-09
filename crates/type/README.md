# windowshit-type

用 Rust 重写的 Windows `type.exe`，跨平台运行，显示文本文件内容。

## 用法

```
TYPE [drive:][path]filename
```

支持多个文件参数，依次显示。无参数或 `/?` 显示帮助。

## 还原的原版行为

- 原样输出文件字节（含换行符，不转换编码）
- 文件末尾无换行时补一个换行
- 文件不存在时报 `系统找不到指定的文件。`（跟随语言）

## 平台注意

| 平台 | 说明 |
| --- | --- |
| Windows | 无需权限 |
| Linux / macOS | 无需权限 |

## 构建

```bash
cargo build -p type
```

## 依赖

- `windowshit-i18n`：帮助文本与错误消息本地化
