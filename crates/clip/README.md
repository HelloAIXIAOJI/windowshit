# windowshit-clip

用 Rust 重写的 Windows `clip.exe`，跨平台运行，将命令输出重定向到剪贴板。

## 用法

```
command | clip
```

从 stdin 读取全部内容，写入系统剪贴板。成功时无输出。

## 技术说明

- 复用 `arboard` crate（1Password 维护），支持 Windows / macOS / Linux X11 + Wayland
- 无平台分支代码

## 平台注意

| 平台 | 说明 |
| --- | --- |
| Windows | 无需权限 |
| Linux / macOS | 需要图形会话（X11 / Wayland） |

## 构建

```bash
cargo build -p clip
```

## 依赖

- `arboard`：跨平台剪贴板读写
