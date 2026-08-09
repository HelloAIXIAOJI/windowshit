Write-Host "=== more (60 lines via pipe) ==="
$lines = 1..60 | ForEach-Object { 'line ' + $_ }
$lines | .\target\debug\more.exe
Write-Host "=== clip ==="
echo 'windowshit clip test' | .\target\debug\clip.exe
$t = Get-Clipboard
Write-Host ("CLIPBOARD: " + $t)
