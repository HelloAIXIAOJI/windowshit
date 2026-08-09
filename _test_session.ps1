Add-Type -TypeDefinition 'using System.Runtime.InteropServices; public class S2 { [DllImport("kernel32.dll", SetLastError=true)] public static extern bool ProcessIdToSessionId(uint processId, ref uint pSessionId); }'
foreach ($p in @(4,872,1648,65536)) {
  $sid = 0
  $ok = [S2]::ProcessIdToSessionId([uint32]$p, [ref]$sid)
  $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
  Write-Host ("pid=$p ok=$ok sid=$sid lastErr=$err")
}
