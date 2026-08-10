# Windowshit taskkill - English (en-US)

syntax-no-target = ERROR: Invalid syntax. Neither /FI nor /PID nor /IM were specified.
syntax-pid-im = ERROR: Invalid syntax. /PID and /IM cannot be used at the same time.
invalid-option = ERROR: Invalid argument/option - '{ $arg }'.
missing-value = ERROR: Invalid syntax. Value expected for '/{ $flag }'.
usage-hint = Type "TASKKILL /?" for usage.
filter-invalid = ERROR: The search filter cannot be recognized.
not-found = ERROR: The process "{ $target }" not found.
cannot-terminate = ERROR: The process with PID { $pid } could not be terminated.
reason-force-only = Reason: This process can only be terminated forcefully (with /F option).
success-pid = SUCCESS: The process with PID { $pid } has been terminated.
success-pid-child = SUCCESS: The process with PID { $pid } (child process of PID { $parent }) has been terminated.
success-im = SUCCESS: The process "{ $name }" with PID { $pid } has been terminated.
info-no-tasks = INFO: No tasks running with the specified criteria.
unsupported-remote = ERROR: Remote system operations (/S /U /P) are not supported.
