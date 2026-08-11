# 阶段 4 对比测试：原版 robocopy vs 我们的实现（逐字节归一化对比）
# 用法：cargo build -p robocopy --release 后运行本脚本（需 Windows + 原版 robocopy）
$ErrorActionPreference = 'Continue'
$base = Join-Path $env:TEMP 'rc4test'
$workspace = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$rb = Join-Path $workspace 'target\release\robocopy.exe'

function New-Src {
    Remove-Item (Join-Path $base 'srcA') -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $base 'srcB') -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path (Join-Path $base 'srcA\sub') | Out-Null
    Set-Content -Path (Join-Path $base 'srcA\a.txt') -Value 'hello world' -NoNewline -Encoding ascii
    Set-Content -Path (Join-Path $base 'srcA\sub\b.txt') -Value 'bbb' -NoNewline -Encoding ascii
    Set-Content -Path (Join-Path $base 'srcA\sub\c.txt') -Value 'c' -NoNewline -Encoding ascii
    Copy-Item (Join-Path $base 'srcA') (Join-Path $base 'srcB') -Recurse
}

function Normalize([string]$text) {
    $text = $text -replace '\\rc4test\\[AB]\\', '\rc4test\X\'
    $text = $text -replace '\\rc4test\\src[AB]\\', '\rc4test\srcX\'
    $text = $text -replace 'Started :[^\r\n]*', 'Started : T'
    $text = $text -replace 'Ended :[^\r\n]*', 'Ended : T'
    $text = $text -replace '(?m)^\s*Speed :.*$', ''   # 速度与运行耗时相关，删除整行
    $text = $text -replace 'Times :[^\r\n]*', 'TIMES'
    $text = $text -replace '(\r?\n){2,}', "`r`n"  # 折叠连续换行为单个（Speed 删除后残留）
    $text
}

function Compare-Scenario([string]$name, [string[]]$opts) {
    New-Src
    Remove-Item (Join-Path $base 'A') -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $base 'B') -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path (Join-Path $base 'A') | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $base 'B') | Out-Null
    # 目标预置 extra
    Set-Content -Path (Join-Path $base 'A\extra.txt') -Value 'extra data here' -NoNewline -Encoding ascii
    New-Item -ItemType Directory -Force -Path (Join-Path $base 'A\sub2') | Out-Null
    Set-Content -Path (Join-Path $base 'A\sub2\y.txt') -Value 'y' -NoNewline -Encoding ascii
    Set-Content -Path (Join-Path $base 'B\extra.txt') -Value 'extra data here' -NoNewline -Encoding ascii
    New-Item -ItemType Directory -Force -Path (Join-Path $base 'B\sub2') | Out-Null
    Set-Content -Path (Join-Path $base 'B\sub2\y.txt') -Value 'y' -NoNewline -Encoding ascii

    $srcA = Join-Path $base 'srcA'
    $srcB = Join-Path $base 'srcB'
    $dstA = Join-Path $base 'A'
    $dstB = Join-Path $base 'B'
    $argline = ($opts -join ' ')
    # 路径均无空格，无需引号（cmd /c 引号解析会破坏参数）
    cmd /c "robocopy $srcA $dstA $argline > $base\o.txt 2>&1" | Out-Null
    cmd /c "$rb $srcB $dstB $argline > $base\n.txt 2>&1" | Out-Null

    $n1 = Normalize ([System.IO.File]::ReadAllText("$base\o.txt"))
    $n2 = Normalize ([System.IO.File]::ReadAllText("$base\n.txt"))
    if ($n1 -ceq $n2) {
        Write-Output ("PASS  " + $name)
    } else {
        Write-Output ("FAIL  " + $name)
        $l1 = $n1 -split "`r?`n"
        $l2 = $n2 -split "`r?`n"
        for ($i = 0; $i -lt [Math]::Max($l1.Count, $l2.Count); $i++) {
            if ($l1[$i] -cne $l2[$i]) {
                Write-Output ("  {0} |{1}| |{2}|" -f ($i + 1), $l1[$i], $l2[$i])
            }
        }
    }
}

function Compare-Rerun([string]$name, [string[]]$opts) {
    # 第一次复制（各自独立目录），再对比第二次运行（Same 全部跳过）
    New-Src
    $srcA = Join-Path $base 'srcA'; $srcB = Join-Path $base 'srcB'
    Remove-Item (Join-Path $base 'A'), (Join-Path $base 'B') -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path (Join-Path $base 'A') | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $base 'B') | Out-Null
    $argline = ($opts -join ' ')
    cmd /c "robocopy $srcA $base\A $argline > $base\first.txt 2>&1" | Out-Null
    cmd /c "$rb $srcB $base\B $argline > $base\first2.txt 2>&1" | Out-Null
    cmd /c "robocopy $srcA $base\A $argline > $base\o.txt 2>&1" | Out-Null
    cmd /c "$rb $srcB $base\B $argline > $base\n.txt 2>&1" | Out-Null
    $n1 = Normalize ([System.IO.File]::ReadAllText("$base\o.txt"))
    $n2 = Normalize ([System.IO.File]::ReadAllText("$base\n.txt"))
    if ($n1 -ceq $n2) {
        Write-Output ("PASS  " + $name)
    } else {
        Write-Output ("FAIL  " + $name)
        $l1 = $n1 -split "`r?`n"; $l2 = $n2 -split "`r?`n"
        for ($i = 0; $i -lt [Math]::Max($l1.Count, $l2.Count); $i++) {
            if ($l1[$i] -cne $l2[$i]) { Write-Output ("  {0} |{1}| |{2}|" -f ($i + 1), $l1[$i], $l2[$i]) }
        }
    }
}

Compare-Scenario 'MIR' @('/MIR', '/NP', '/R:1', '/W:1')
Compare-Scenario 'E' @('/E', '/NP', '/R:1', '/W:1')
Compare-Scenario 'E+X' @('/E', '/X', '/NP', '/R:1', '/W:1')
Compare-Scenario 'E+XX' @('/E', '/XX', '/NP', '/R:1', '/W:1')
Compare-Scenario 'L+E' @('/L', '/E', '/NP', '/R:1', '/W:1')
Compare-Scenario 'L+PURGE' @('/L', '/PURGE', '/NP', '/R:1', '/W:1')
Compare-Scenario 'MOV' @('/MOV', '/NP', '/R:1', '/W:1')
Compare-Scenario 'CREATE' @('/CREATE', '/NP', '/R:1', '/W:1')
Compare-Scenario 'Z' @('/Z', '/NP', '/R:1', '/W:1')
Compare-Scenario 'MT' @('/MT:4', '/E', '/NP', '/R:1', '/W:1')
Compare-Scenario 'MT+PURGE' @('/MT:2', '/E', '/PURGE', '/NP', '/R:1', '/W:1')
Compare-Scenario 'MT+Z+CREATE' @('/MT:2', '/Z', '/CREATE', '/NP', '/R:1', '/W:1')
Compare-Scenario 'V' @('/E', '/V', '/NP', '/R:1', '/W:1')
Compare-Rerun 'E rerun' @('/E', '/NP', '/R:1', '/W:1')
Compare-Rerun 'MIR rerun' @('/MIR', '/NP', '/R:1', '/W:1')
Compare-Rerun 'MT rerun' @('/MT:2', '/E', '/NP', '/R:1', '/W:1')
