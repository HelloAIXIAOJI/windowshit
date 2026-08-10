# Windowshit taskkill - 简体中文（zh-CN）

syntax-no-target = 错误: 无效的语法。既未指定 /FI，也未指定 /PID 或 /IM。
syntax-pid-im = 错误: 无效的语法。/PID 和 /IM 不能同时使用。
invalid-option = 错误: 无效的参数/选项 - '{ $arg }'。
missing-value = 错误: 无效的语法。应给 '/{ $flag }' 指定值。
usage-hint = 键入 "TASKKILL /?" 了解用法。
filter-invalid = 错误: 无法识别搜索筛选器。
not-found = 错误: 没有找到进程 "{ $target }"。
cannot-terminate = 错误: 无法终止 PID 为 { $pid } 的进程。
reason-force-only = 原因: 只能使用 /F 选项强制终止该进程。
success-pid = 成功: 已终止 PID 为 { $pid } 的进程。
success-pid-child = 成功: 已终止 PID 为 { $pid } 的进程（PID 为 { $parent } 的进程的子进程）。
success-im = 成功: 已终止进程 "{ $name }"，其 PID 为 { $pid }。
info-no-tasks = 信息: 没有正在运行的任务匹配指定条件。
unsupported-remote = 错误: 不支持远程系统操作（/S /U /P）。
