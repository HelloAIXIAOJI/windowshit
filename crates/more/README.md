# windowshit-more

用 Rust 重写的 Windows `more.exe`，跨平台运行，一次显示一屏输出。

## 用法

```
MORE [/E [/C] [/P] [/S] [/Tn] [+n]] [files]
command | MORE [/S] [/Tn] [+n]
```

| 参数 | 说明 |
| --- | --- |
| `/S` | 将多个连续空行压缩为一行 |
| `/Tn` | 将制表符展开为 n 个空格（默认 8） |
| `+n` | 从第 n 行开始显示 |
| `files` | 要显示的文件列表（无参数时从 stdin 读） |

## 还原的原版行为

- stdout 非终端（管道）时直接全部输出，不暂停
- stdout 是终端时分页暂停，显示 `-- More --` 等待按键
- `/E /C /P` 等扩展功能参数接受但简化处理

## 平台注意

| 平台 | 说明 |
| --- | --- |
| Windows | 无需权限 |
| Linux / macOS | 无需权限 |

## 构建

```bash
cargo build -p more
```

## 依赖

- `windowshit-i18n`：帮助文本与错误消息本地化
