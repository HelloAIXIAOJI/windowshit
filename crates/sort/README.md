# windowshit-sort

用 Rust 重写的 Windows `sort.exe`，跨平台运行，对输入行排序。

## 用法

```
SORT [/R] [/+n] [/O output] [input_file]
```

| 参数 | 说明 |
| --- | --- |
| `/R` | 反向排序 |
| `/+n` | 从每行第 n 个字符开始比较 |
| `/O file` | 输出到文件（默认标准输出） |
| `input_file` | 输入文件（默认 stdin） |

## 还原的原版行为

- 排序始终**不区分大小写**（原版行为）
- 输出统一 `\r\n` 行尾
- `/M /L /REC /T` 等参数接受但忽略（不影响排序结果）

## 平台注意

| 平台 | 说明 |
| --- | --- |
| Windows | 无需权限 |
| Linux / macOS | 无需权限 |

## 构建

```bash
cargo build -p sort
```

## 依赖

- `windowshit-i18n`：帮助文本与错误消息本地化
