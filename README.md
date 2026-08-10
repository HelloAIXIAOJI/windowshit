# Windowshit 屎窗

## 这是啥屎？

本项目致力于将 Windows 上的部分命令行驱动的程序（比如 CMD、ipconfig 等 exe 程序）通过 Rust 重写，输出与功能尽可能相似，并在 Linux、macOS 甚至 Windows 上运行。致力于让所有人都能吃到Windows的难绷设计。

这个项目为整活项目，不涉及任何商业用途。

## 组件列表

| 组件名 | 功能介绍 | 是否完成 |
| --- | --- | --- |
| cmd | Windows 命令提示符解释器（cmd.exe） | 否 |
| ipconfig | 显示网络配置信息（无参数 / /all） | 是 |
| ping | 测试网络连通性 | 是 |
| tracert | 跟踪数据包路由路径 | 是 |
| pathping | 结合 ping 与 tracert 的路由分析 | 是 |
| netstat | 显示网络连接、端口和协议统计 | 否 |
| arp | 显示 / 修改 ARP 缓存 | 否 |
| route | 显示 / 修改 IP 路由表 | 否 |
| nslookup | 查询 DNS 记录 | 否 |
| getmac | 显示网卡 MAC 地址 | 是 |
| fc | 比较两个文件并显示差异 | 是 |
| findstr | 在文件中查找字符串（支持正则） | 是 |
| sort | 对输入行排序 | 是 |
| where | 查找匹配文件的位置 | 是 |
| icacls | 修改文件 / 目录 ACL 权限 | 否 |
| expand | 解压 CAB 压缩包 | 否 |
| makecab | 制作 CAB 压缩包 | 否 |
| replace | 替换目标目录中的文件 | 是 |
| xcopy | 复制文件和目录树 | 否 |
| robocopy | 高级文件复制（多线程、镜像、重试） | 否 |
| tree | 以树形结构显示目录 | 是 |
| more | 分页显示文件内容 | 是 |
| type | 显示文本文件内容 | 是 |
| choice | 提供选项供用户选择 | 是 |
| systeminfo | 显示系统详细信息 | 是 |
| tasklist | 列出当前运行进程 | 是 |
| taskkill | 按 PID / 映像名结束进程 | 是 |
| reg | 注册表命令行操作 | 否 |
| sc | 服务控制管理器 | 否 |
| net | 网络 / 服务 / 用户管理 | 否 |
| shutdown | 关机 / 重启 / 注销 | 否 |
| powercfg | 电源管理配置 | 否 |
| bcdedit | 启动配置数据编辑 | 否 |
| whoami | 显示当前用户信息 | 是 |
| hostname | 显示计算机名 | 是 |
| ver | 显示系统版本 | 是 |
| runas | 以其他用户身份运行程序 | 否 |
| setx | 永久设置环境变量 | 否 |
| clip | 将输出复制到剪贴板 | 是 |
| schtasks | 计划任务管理 | 否 |
| help | 显示命令帮助 | 否 |
| dism | 部署映像服务与管理 | 否 |
| regsvr32 | 注册 / 反注册 COM 组件 | 否 |
| certutil | 证书服务工具 | 否 |

## 许可证

本项目使用 MIT 许可证，详情请参见 [LICENSE](LICENSE) 文件。
