# Windowshit ping - 简体中文（zh-CN）

# 运行输出
ping-start-ip = 正在 Ping { $addr } 具有 { $size } 字节的数据:
ping-start-host = 正在 Ping { $host } [{ $addr }] 具有 { $size } 字节的数据:
reply-v4-fast = 来自 { $source } 的回复: 字节={ $data_len } 时间<1ms TTL={ $ttl }
reply-v4 = 来自 { $source } 的回复: 字节={ $data_len } 时间={ $rtt }ms TTL={ $ttl }
reply-v6-fast = 来自 { $source } 的回复: 时间<1ms
reply-v6 = 来自 { $source } 的回复: 时间={ $rtt }ms
time-exceeded = 来自 { $source } 的回复: TTL 在传输中过期。
dest-unreachable = 来自 { $source } 的回复: 无法访问目标主机。
timeout = 请求超时。
transmit-failed = PING: 传输失败。一般故障。
stats-header = { $addr } 的 Ping 统计信息:
stats-packets =     数据包: 已发送 = { $sent }，已接收 = { $received }，丢失 = { $loss } ({ $loss_pct }% 丢失)，
stats-rtt-header = 往返行程的估计时间(以毫秒为单位):
stats-rtt-line =     最短 = { $min }ms，最长 = { $max }ms，平均 = { $avg }ms

# 错误消息（对齐 Windows 中文版）
error-cannot-resolve = Ping 请求找不到主机 { $host }。请检查该名称，然后重试。
error-init-icmp = 初始化 ICMP 失败: { $err }
error-init-hint = 提示: Windows 下可能需要以管理员身份运行。
error-sockopt = 设置 socket 选项失败: { $err }
error-invalid-option = 选项无效: -{ $flag }。
error-unsupported = 错误: 此实现尚不支持选项 -{ $flag }。
error-bad-parameter = 参数无效: { $arg }。
error-option-needs-value = 选项 -{ $flag } 需要一个参数。
error-option-no-value = 选项 -{ $flag } 不应带参数。
error-value-range = 选项 -{ $flag } 的值无效，有效范围是 { $min } 到 { $max }。
error-host-list = 选项 -{ $flag } 的格式无效。
error-only-supported = 选项 -{ $flag } 仅支持用于 IPv{ $ver }。
error-not-valid-address = { $addr } 不是有效地址。
