# windowshit-whoami

用 Rust 重写的 Windows `whoami.exe`，跨平台运行。

## 用法

```
whoami
```

无参数，输出 `NetBIOS名\用户名`。

## 还原的原版行为

Windows 原版 whoami 无参数输出当前用户，格式为 `主机名\用户名`：

- **NetBIOS 名**：主机名转小写并截断到 15 字符（NetBIOS 名称上限）
- **用户名**：系统用户名（转小写）

例如：

```
aixiaoji-deskto\aixiaoji
```

## 未实现

`/user`、`/groups`、`/priv`、`/all` 等子命令尚未实现（无参数模式即可覆盖大多数使用场景）。

## 平台注意

| 平台 | 说明 |
| --- | --- |
| Windows | 无需权限 |
| Linux / macOS | 无需权限（Linux 上输出 `hostname\user` 格式） |

## 构建

```bash
cargo build -p whoami
```

## 依赖

- `whoami`：跨平台获取用户名与主机名（无平台分支）
