# Windowshit ping - English (en-US)

# Runtime output
ping-start-ip = Pinging { $addr } with { $size } bytes of data:
ping-start-host = Pinging { $host } [{ $addr }] with { $size } bytes of data:
reply-v4-fast = Reply from { $source }: bytes={ $data_len } time<1ms TTL={ $ttl }
reply-v4 = Reply from { $source }: bytes={ $data_len } time={ $rtt }ms TTL={ $ttl }
reply-v6-fast = Reply from { $source }: time<1ms
reply-v6 = Reply from { $source }: time={ $rtt }ms
time-exceeded = Reply from { $source }: TTL expired in transit.
dest-unreachable = Reply from { $source }: Destination host unreachable.
timeout = Request timed out.
stats-header = Ping statistics for { $addr }:
stats-packets =     Packets: Sent = { $sent }, Received = { $received }, Lost = { $loss } ({ $loss_pct }% loss),
stats-rtt-header = Approximate round trip times in milli-seconds:
stats-rtt-line =     Minimum = { $min }ms, Maximum = { $max }ms, Average = { $avg }ms

# Error messages
error-no-target = ERROR: No target specified.
error-cannot-resolve = Ping request could not find host { $host }. Please check the name and try again.
error-init-icmp = Failed to initialize ICMP: { $err }
error-init-hint = Hint: may require administrator privileges on Windows.
error-sockopt = Failed to set socket option: { $err }
error-bad-src = ERROR: Source address { $src } is invalid.
error-unsupported = ERROR: Option { $flag } is not supported by this implementation.
error-invalid-option = Invalid option: -{ $flag }
error-option-needs-value = Option -{ $flag } requires a value.
error-option-no-value = Option -{ $flag } does not take a value.
error-multiple-targets = Multiple targets specified.
error-v4-v6-conflict = -4 and -6 cannot be used together.
error-value-range = Option -{ $flag } value must be between { $min } and { $max }.
error-value-numeric = Option -{ $flag } value must be numeric.
error-host-list = Option -{ $flag } has an invalid format.
