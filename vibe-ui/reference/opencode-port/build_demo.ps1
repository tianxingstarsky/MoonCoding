# =============================================================================
# build_demo.ps1 — PowerShell build script for opencode-port Qt6 demo (Windows)
#
# Prerequisites:
#   Qt6 installed (e.g., C:\Qt\6.8.0\msvc2022_64 or C:\Qt\6.8.0\mingw_64)
#   CMake 3.20+
#   Ninja or Visual Studio 2022
#
# Usage:
#   .\build_demo.ps1                    # Build and run
#   .\build_demo.ps1 -BuildOnly         # Build only
#   .\build_demo.ps1 -Clean             # Clean and rebuild
#   .\build_demo.ps1 -QtPath "C:\Qt\6.8.0\msvc2022_64"  # Specify Qt path
# =============================================================================

param(
    [switch]$BuildOnly,
    [switch]$Clean,
    [string]$QtPath = ""
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$BuildDir = Join-Path $ScriptDir "build"

# ---- Detect Qt6 ----
if ($QtPath -eq "") {
    # Auto-detect common Qt6 locations.
    $candidates = @(
        "C:\Qt\6.8.0\msvc2022_64",
        "C:\Qt\6.8.0\mingw_64",
        "C:\Qt\6.7.0\msvc2022_64",
        "C:\Qt\6.7.0\mingw_64",
        "C:\msys64\mingw64",
        "C:\msys64\ucrt64"
    )
    foreach ($cand in $candidates) {
        if (Test-Path (Join-Path $cand "lib\cmake\Qt6")) {
            $QtPath = $cand
            Write-Host "Found Qt6 at: $QtPath" -ForegroundColor Green
            break
        }
    }
}

if ($QtPath -eq "") {
    Write-Host "Qt6 not auto-detected. Specify with -QtPath <path>" -ForegroundColor Yellow
    Write-Host "Example: .\build_demo.ps1 -QtPath C:\Qt\6.8.0\msvc2022_64"
    exit 1
}

# ---- Clean ----
if ($Clean) {
    Write-Host "==> Cleaning build directory..." -ForegroundColor Cyan
    if (Test-Path $BuildDir) {
        Remove-Item -Recurse -Force $BuildDir
    }
    Write-Host "==> Clean complete."
    if ($Clean -and -not $BuildOnly) { exit 0 }
}

# ---- Configure ----
Write-Host "==> Configuring CMake..." -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path $BuildDir | Out-Null
Push-Location $BuildDir

$cmakeArgs = @(
    "-G", "Ninja",
    "-DCMAKE_BUILD_TYPE=Release",
    "-DCMAKE_PREFIX_PATH=$QtPath",
    $ScriptDir
)

& cmake @cmakeArgs
if ($LASTEXITCODE -ne 0) {
    Write-Host "CMake configuration failed!" -ForegroundColor Red
    Pop-Location
    exit $LASTEXITCODE
}

# ---- Build ----
Write-Host "==> Building..." -ForegroundColor Cyan
& cmake --build . --config Release
if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    Pop-Location
    exit $LASTEXITCODE
}

Pop-Location
Write-Host "==> Build complete!" -ForegroundColor Green

# ---- Run ----
if (-not $BuildOnly) {
    Write-Host "==> Running OpenCodePortDemo..." -ForegroundColor Cyan
    $exePath = Join-Path $BuildDir "OpenCodePortDemo.exe"
    if (Test-Path $exePath) {
        & $exePath
    } else {
        Write-Host "Executable not found at: $exePath" -ForegroundColor Red
        Get-ChildItem -Recurse $BuildDir -Filter "*.exe" | Select-Object FullName
    }
}
