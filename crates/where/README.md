# windowshit-where

用 Rust 重写的 Windows `where.exe`，跨平台运行，在 PATH 中查找匹配文件。

## 用法

```
WHERE [/R dir] [/Q] [/F] [/T] pattern...
```

| 参数 | 说明 |
| --- | --- |
| `/R dir` | 从指定目录开始递归搜索 |
| `/Q` | 安静模式，只返回退出码 |
| `/F` | 文件名加双引号 |
| `/T` | 显示文件大小、修改日期和时间 |

## 还原的原版行为

- 默认搜索当前目录 + PATH 环境变量中的所有目录
- 支持 `*` `?` 通配符，大小写不敏感
- 模式无扩展名时自动追加 PATHEXT（`.COM;.EXE;.BAT;.CMD`，仅 Windows 语义）
- 退出码：0 = 找到，1 = 未找到，2 = 错误

## 技术说明

- 纯文件系统逻辑（`std::fs`），通配符匹配手写（`*`/`?` 递归匹配），无平台分支
- PATHEXT 仅在 Windows 上有语义，unix 上为空

## 平台注意

| 平台 | 说明 |
| --- | --- |
| Windows | 无需权限 |
| Linux / macOS | 无需权限 |

## 构建

```bash
cargo build -p where
```

## 依赖

- `windowshit-i18n`：帮助文本与错误消息本地化
