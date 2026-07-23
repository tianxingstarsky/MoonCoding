# Board smoke helpers for Luckfox Lyra (Qt6 full MoonCoding).
# Run from Windows PowerShell with board on USB adb.

param(
    [string]$Adb = "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe"
)

$ErrorActionPreference = "Stop"
if (-not (Test-Path $Adb)) {
    $Adb = "adb"
}

Write-Host "=== adb devices ==="
& $Adb devices

Write-Host "=== framebuffer / display ==="
& $Adb shell "ls -l /dev/fb0 /dev/dri 2>/dev/null; cat /sys/class/graphics/fb0/virtual_size 2>/dev/null; cat /sys/class/graphics/fb0/modes 2>/dev/null; free -m | head -3"

Write-Host "=== launch script present? ==="
& $Adb shell "ls -l /root/mooncoding/mooncoding /root/mooncoding/run-mooncoding.sh 2>/dev/null || echo 'not deployed yet'"

Write-Host "SMOKE_PROBE_DONE"
