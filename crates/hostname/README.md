# windowshit-hostname

用 Rust 重写的 Windows `hostname.exe`，跨平台运行，输出系统主机名。

## 用法

```
hostname
```

无参数，输出一行主机名。

## 还原的原版行为

- 输出主机名原文（Windows 上通常为大写，如 `AIXIAOJI-DESKTOP`）
- 跟随控制台代码页处理错误消息（如无法获取主机名时的提示）

## 平台注意

| 平台 | 说明 |
| --- | --- |
| Windows | 无需权限 |
| Linux / macOS | 无需权限 |

## 构建

```bash
cargo build -p hostname
```

## 依赖

- `hostname`：跨平台获取主机名（封装 `gethostname` 等系统 API，无平台分支）
- `windowshit-i18n`：错误消息本地化
