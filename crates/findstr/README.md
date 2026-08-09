# windowshit-findstr

用 Rust 重写的 Windows `findstr.exe`，跨平台运行，在文件中搜索字符串。

## 用法

```
FINDSTR [/B] [/E] [/L] [/R] [/S] [/I] [/X] [/V] [/N] [/M] [/C:string]
        strings [[drive:][path]filename[ ...]]
```

| 参数 | 说明 |
| --- | --- |
| `/B` | 模式在行首才匹配 |
| `/E` | 模式在行尾才匹配 |
| `/L` | 按字面搜索（非正则） |
| `/R` | 按正则搜索（默认） |
| `/S` | 递归搜索子目录 |
| `/I` | 忽略大小写 |
| `/X` | 整行完全匹配 |
| `/V` | 反向匹配（输出不匹配的行） |
| `/N` | 匹配行前打印行号 |
| `/M` | 只打印包含匹配的文件名 |
| `/C:string` | 将整个字符串作为字面搜索 |

## 还原的原版行为

- 默认按正则搜索（regex crate，findstr 正则语法是其子集）
- 多文件时匹配行带 `文件:行` 前缀
- 无文件参数时从 stdin 读取
- 退出码：0 = 找到，1 = 未找到，2 = 错误

## 技术说明

- 正则引擎复用 `regex` crate，不重复实现
- 跨平台无平台分支

## 平台注意

| 平台 | 说明 |
| --- | --- |
| Windows | 无需权限 |
| Linux / macOS | 无需权限 |

## 构建

```bash
cargo build -p findstr
```

## 依赖

- `regex`：正则匹配引擎
- `windowshit-i18n`：错误消息本地化
