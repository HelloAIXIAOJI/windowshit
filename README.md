# Windowshit

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
| nbtstat | 显示 NetBIOS 协议统计 | 否 |
| getmac | 显示网卡 MAC 地址 | 是 |
| telnet | 远程登录终端 | 否 |
| ftp | 文件传输客户端 | 否 |
| netsh | 网络配置命令行脚本工具 | 否 |
| tftp | 简单文件传输协议客户端 | 否 |
| attrib | 修改文件 / 目录属性 | 否 |
| fc | 比较两个文件并显示差异 | 否 |
| comp | 逐字节比较两个文件 | 否 |
| find | 在文件中查找字符串 | 否 |
| findstr | 在文件中查找字符串（支持正则） | 是 |
| sort | 对输入行排序 | 否 |
| where | 查找匹配文件的位置 | 是 |
| subst | 将路径映射为虚拟驱动器 | 否 |
| icacls | 修改文件 / 目录 ACL 权限 | 否 |
| cacls | 修改文件 ACL（旧版，已弃用） | 否 |
| cipher | 加密 / 解密文件（EFS） | 否 |
| compact | 显示 / 修改 NTFS 压缩状态 | 否 |
| expand | 解压 CAB 压缩包 | 否 |
| makecab | 制作 CAB 压缩包 | 否 |
| replace | 替换目标目录中的文件 | 否 |
| xcopy | 复制文件和目录树 | 否 |
| robocopy | 高级文件复制（多线程、镜像、重试） | 否 |
| tree | 以树形结构显示目录 | 是 |
| more | 分页显示文件内容 | 是 |
| type | 显示文本文件内容 | 是 |
| doskey | 命令行宏与历史记录 | 否 |
| choice | 提供选项供用户选择 | 否 |
| print | 打印文本文件 | 否 |
| recover | 恢复损坏文件中的数据 | 否 |
| systeminfo | 显示系统详细信息 | 是 |
| tasklist | 列出当前运行进程 | 是 |
| taskkill | 按 PID / 映像名结束进程 | 否 |
| driverquery | 显示已安装驱动程序 | 否 |
| pnputil | 管理驱动包 / 设备 | 否 |
| wmic | WMI 命令行查询工具 | 否 |
| msiexec | Windows Installer 管理 | 否 |
| sfc | 系统文件完整性检查 | 否 |
| chkdsk | 检查磁盘错误并修复 | 否 |
| defrag | 磁盘碎片整理 | 否 |
| diskpart | 磁盘分区管理 | 否 |
| format | 格式化磁盘 | 否 |
| fsutil | 文件系统高级管理 | 否 |
| convert | 将 FAT 卷转换为 NTFS | 否 |
| label | 设置磁盘卷标 | 否 |
| mountvol | 管理卷挂载点 | 否 |
| reg | 注册表命令行操作 | 否 |
| sc | 服务控制管理器 | 否 |
| net | 网络 / 服务 / 用户管理 | 否 |
| shutdown | 关机 / 重启 / 注销 | 否 |
| powercfg | 电源管理配置 | 否 |
| gpupdate | 刷新组策略 | 否 |
| gpresult | 显示组策略结果 | 否 |
| bcdedit | 启动配置数据编辑 | 否 |
| whoami | 显示当前用户信息 | 是 |
| hostname | 显示计算机名 | 是 |
| ver | 显示系统版本 | 是 |
| runas | 以其他用户身份运行程序 | 否 |
| setx | 永久设置环境变量 | 否 |
| clip | 将输出复制到剪贴板 | 是 |
| openfiles | 查看 / 管理已打开的文件 | 否 |
| schtasks | 计划任务管理 | 否 |
| query | 查询会话 / 进程状态 | 否 |
| logoff | 注销用户会话 | 否 |
| msg | 向用户发送消息 | 否 |
| mode | 配置控制台 / 串口设备 | 否 |
| help | 显示命令帮助 | 否 |
| tskill | 结束远程会话中的进程 | 否 |
| dism | 部署映像服务与管理 | 否 |
| w32tm | 时间服务配置 | 否 |
| regsvr32 | 注册 / 反注册 COM 组件 | 否 |
| secedit | 安全配置编辑器 | 否 |
| setspn | 管理服务主体名称（SPN） | 否 |
| waitfor | 等待网络上的信号 | 否 |
| winrm | 远程管理服务客户端 | 否 |
| logman | 性能计数器 / 日志管理 | 否 |
| eventcreate | 在事件日志中创建自定义事件 | 否 |
| bitsadmin | 后台智能传输服务管理 | 否 |
| certutil | 证书服务工具 | 否 |
| tracerpt | 跟踪日志解析 | 否 |
| typeperf | 将性能计数器输出到终端 | 否 |
| verifier | 驱动验证管理器 | 否 |
| iexpress | 制作自解压包 | 否 |
| tcmsetup | 配置 TAPI 客户端 | 否 |
| loadctr / unlodctr | 加载 / 卸载性能计数器 | 否 |

## 许可证

本项目使用 MIT 许可证，详情请参见 [LICENSE](LICENSE) 文件。