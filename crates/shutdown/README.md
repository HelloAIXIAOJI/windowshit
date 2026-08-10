# shutdown

Rust 重写的 Windows `shutdown.exe`（跨平台整活）。

## 功能

- 关机 `/s`、重启 `/r`、注销 `/l`、休眠 `/h`、立即关机 `/p`
- `/a` 取消待执行的关机
- `/t xxx` 超时秒（默认 30）、`/f` 强制、`/hybrid` 快速启动
- `/m \\computer` 远程、`/c "注释"`、`/d [p|u:]xx:yy` 原因代码
- 权限要求与原版一致：Windows 需管理员，Linux 需 root

## 用法

```
shutdown [/i | /l | /s | /sg | /r | /g | /a | /p | /h | /e | /o] [/hybrid] [/soft] [/fw] [/f]
        [/m \\computer] [/t xxx] [/d [p|u:]xx:yy [/c "comment"]]
```

## 实现策略

| 平台 | 方式 |
| --- | --- |
| Windows | 直接委托 `InitiateSystemShutdownExW` / `AbortSystemShutdownW`（带横幅倒计时与可取消机制，最接近原版） |
| Linux | 调用 `systemctl poweroff` / `systemctl reboot` / `systemctl hibernate` / `loginctl terminate-user` |
| 权限 | Windows `GetTokenInformation` 检查是否提权；Linux `geteuid()==0` 检查 root |

## 实测对齐的行为（Windows 11 原版）

| 场景 | 输出 | 退出码 |
| --- | --- | --- |
| 无参数 / `/?` / 无效参数 / 缺主操作 | 完整帮助（含 Reasons 表） | 1 |
| `/a` 无待取消关机 | `Unable to abort the system shutdown because no shutdown was in progress.(1116)` | 1116 |
| `/s` 非管理员 | `Access denied.(5)` | 5 |
| `/t abc` 无效数字 | 完整帮助 | 1 |
| `/t` 超 315360000 | 完整帮助 | 1 |
| `/d abc` 原因格式错误 | 完整帮助 | 1 |

## Linux 行为

- `/t > 0` 先打印倒计时（Ctrl+C 可取消），对应 Windows 横幅倒计时
- `/m` 远程关机不支持（明确报错）
- `/a` 调用 `systemctl cancel`
- 注销 `/l` 走 `loginctl terminate-user`

## 已知差异

- `/sg` `/g`（自动登录重启）在 Linux 上等同 `/r`，ARSO 语义不适用
- `/hybrid` `/fw` `/soft` 参数被接受但委托系统默认行为
- `/i` GUI、`/e` 记录原因、`/o` 高级启动在 Linux 上不支持（明确报错）
- Windows 上非管理员运行 `/a` 时 `AbortSystemShutdownW` 返回的是权限错误(5)，管理员运行才返回 1116
