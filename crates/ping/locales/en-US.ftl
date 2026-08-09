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
transmit-failed = PING: transmit failed. General failure.
ip-param-problem = IP parameter problem.
stats-header = Ping statistics for { $addr }:
stats-packets =     Packets: Sent = { $sent }, Received = { $received }, Lost = { $loss } ({ $loss_pct }% loss),
stats-rtt-header = Approximate round trip times in milli-seconds:
stats-rtt-line =     Minimum = { $min }ms, Maximum = { $max }ms, Average = { $avg }ms

# Error messages (aligned with Windows English edition)
error-cannot-resolve = Ping request could not find host { $host }. Please check the name and try again.
error-init-icmp = Failed to initialize ICMP: { $err }
error-init-hint = Hint: may require administrator privileges on Windows.
error-sockopt = Failed to set socket option: { $err }
error-invalid-option = Bad option -{ $flag }.
error-bad-parameter = Bad parameter { $arg }.
error-option-needs-value = Option -{ $flag } requires a value.
error-option-no-value = Option -{ $flag } does not take a value.
error-value-range = Bad value for option -{ $flag }, valid range is from { $min } to { $max }.
error-host-list = Option -{ $flag } has an invalid format.
error-only-supported = The option -{ $flag } is only supported for IPv{ $ver }.
error-not-valid-address = { $addr } is not a valid address.
error-access-denied-c = Access denied. Option -c requires administrative privileges.
error-unable-ip-driver = Unable to contact IP driver. General failure.
