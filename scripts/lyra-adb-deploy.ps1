# Deploy full MoonCoding (Qt6) to Luckfox Lyra over adb.
param(
    [string]$Adb = "",
    [string]$BuildDir = "",
    [string]$StageDir = "",
    [string]$RemoteDir = "/root/mooncoding",
    [string]$WorkspaceOnBoard = "/root/mooncoding-ws",
    [switch]$Launch
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot

if (-not $Adb) {
    $candidates = @(
        "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe",
        "E:\workspace\luckyfoxRK3506B\platform-tools\adb.exe",
        "adb"
    )
    foreach ($c in $candidates) {
        if ($c -eq "adb") { $Adb = $c; break }
        if (Test-Path $c) { $Adb = $c; break }
    }
}
if (-not $BuildDir) { $BuildDir = Join-Path $RepoRoot "build-board" }
if (-not $StageDir) { $StageDir = Join-Path $BuildDir "qt6-stage" }

function Invoke-Adb {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Args)
    & $Adb @Args
    if ($LASTEXITCODE -ne 0) { throw ("adb failed: {0}" -f ($Args -join ' ')) }
}

Write-Host ("adb={0}" -f $Adb)
Invoke-Adb devices

# Refresh staged Qt6 libs via WSL (real files, no symlinks)
Write-Host "Staging Qt6 libs in WSL..."
wsl -d Ubuntu-22.04 -u mooncoding -- bash /mnt/e/WSL/stage-qt6-deps.sh
wsl -d Ubuntu-22.04 -u mooncoding -- bash /mnt/e/newvibecode/scripts/buildroot/stage-qt6webengine.sh
if (-not (Test-Path (Join-Path $StageDir "lib"))) {
    throw "qt6-stage missing after staging"
}

$uiBin = Get-ChildItem -Path $BuildDir -Recurse -Filter mooncoding -File -ErrorAction SilentlyContinue |
    Where-Object { $_.DirectoryName -match 'vibe-ui' } |
    Select-Object -First 1
if (-not $uiBin) {
    $uiBin = Get-ChildItem -Path $BuildDir -Recurse -Filter mooncoding -File -ErrorAction SilentlyContinue |
        Select-Object -First 1
}
$agentSo = Join-Path $BuildDir "vibe-ui\libvibe_agent.so"
if (-not (Test-Path $agentSo)) {
    $agentSo = (Get-ChildItem -Path $BuildDir -Recurse -Filter libvibe_agent.so -File |
        Select-Object -First 1).FullName
}
$vibeBin = Join-Path $BuildDir "vibe-ui\vibe"
if (-not (Test-Path $vibeBin)) {
    $vibeBin = (Get-ChildItem -Path $BuildDir -Recurse -Filter vibe -File |
        Where-Object { $_.DirectoryName -match 'vibe-target|vibe-ui' } |
        Select-Object -First 1).FullName
}

if (-not $uiBin) { throw "mooncoding not found under $BuildDir" }
if (-not $agentSo) { throw "libvibe_agent.so not found" }
if (-not $vibeBin) { throw "vibe not found" }

Write-Host ("Pushing binaries to {0}" -f $RemoteDir)
Invoke-Adb shell ("mkdir -p {0}/lib {0}/plugins/platforms {1}" -f $RemoteDir, $WorkspaceOnBoard)
Invoke-Adb push $uiBin.FullName ("{0}/mooncoding" -f $RemoteDir)
Invoke-Adb push $agentSo ("{0}/libvibe_agent.so" -f $RemoteDir)
Invoke-Adb push $vibeBin ("{0}/vibe" -f $RemoteDir)
Invoke-Adb shell ("chmod +x {0}/mooncoding {0}/vibe" -f $RemoteDir)

Write-Host "Pushing staged Qt6 libs..."
Get-ChildItem -Path (Join-Path $StageDir "lib") -File | ForEach-Object {
    Invoke-Adb push $_.FullName ("{0}/lib/" -f $RemoteDir)
}
$platDir = Join-Path $StageDir "plugins\platforms"
if (Test-Path $platDir) {
    Get-ChildItem -Path $platDir -File | ForEach-Object {
        Invoke-Adb push $_.FullName ("{0}/plugins/platforms/" -f $RemoteDir)
    }
}

# Optional WebEngine runtime (libexec + resources + locales)
$libexec = Join-Path $StageDir "libexec\QtWebEngineProcess"
if (Test-Path $libexec) {
    Write-Host "Pushing QtWebEngineProcess..."
    Invoke-Adb shell ("mkdir -p {0}/libexec {0}/resources {0}/translations/qtwebengine_locales" -f $RemoteDir)
    Invoke-Adb push $libexec ("{0}/libexec/QtWebEngineProcess" -f $RemoteDir)
    Invoke-Adb shell ("chmod +x {0}/libexec/QtWebEngineProcess" -f $RemoteDir)
}
$resDir = Join-Path $StageDir "resources"
if (Test-Path $resDir) {
    Get-ChildItem -Path $resDir -File -ErrorAction SilentlyContinue | ForEach-Object {
        Invoke-Adb push $_.FullName ("{0}/resources/" -f $RemoteDir)
    }
}
$locDir = Join-Path $StageDir "translations\qtwebengine_locales"
if (Test-Path $locDir) {
    Get-ChildItem -Path $locDir -File -ErrorAction SilentlyContinue | ForEach-Object {
        Invoke-Adb push $_.FullName ("{0}/translations/qtwebengine_locales/" -f $RemoteDir)
    }
}

# Symlink unversioned names on device
$linkScript = Join-Path $RepoRoot "scripts\lyra-link-qt-libs.sh"
Invoke-Adb push $linkScript "/root/mooncoding/link-qt-libs.sh"
Invoke-Adb shell "sed -i 's/\r$//' /root/mooncoding/link-qt-libs.sh; sh /root/mooncoding/link-qt-libs.sh"

# Prefer repo launch script (WebEngine env + isolated workspaces)
$runScript = Join-Path $RepoRoot "scripts\lyra-run-mooncoding.sh"
Invoke-Adb push $runScript ("{0}/run-mooncoding.sh" -f $RemoteDir)
Invoke-Adb shell ("sed -i 's/\r$//' {0}/run-mooncoding.sh; chmod +x {0}/run-mooncoding.sh" -f $RemoteDir)
Write-Host ("Wrote {0}/run-mooncoding.sh" -f $RemoteDir)

# Smoke: library/plugin probe with offscreen (no display required)
Write-Host "=== smoke: offscreen probe ==="
Invoke-Adb shell ("cd {0} && export LD_LIBRARY_PATH={0}:{0}/lib && export QT_PLUGIN_PATH={0}/plugins && ./mooncoding -platform offscreen --help >/tmp/mc-help.txt 2>&1; echo exit=`$?; head -20 /tmp/mc-help.txt || true" -f $RemoteDir)

if ($Launch) {
    Write-Host "Launching on board (background)..."
    Invoke-Adb shell ("cd {0} && nohup sh ./run-mooncoding.sh > /tmp/mooncoding.log 2>&1 & echo started; sleep 3; head -80 /tmp/mooncoding.log; ps | grep -E 'mooncoding|Qt' | grep -v grep || true" -f $RemoteDir)
}

Write-Host "DEPLOY_OK"
