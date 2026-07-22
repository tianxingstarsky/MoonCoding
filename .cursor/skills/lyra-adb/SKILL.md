---
name: lyra-adb
description: >-
  Luckfox Lyra board ADB connection for this machine and MoonCoding deploy.
  Use when deploying to the board, running adb, checking devices, lyra-adb-deploy,
  board smoke, or when the agent claims adb is missing from PATH.
---

# Lyra ADB (this machine)

## ADB binary (authoritative)

Do **not** assume `adb` is on PATH. On this Windows host use:

```
E:\下载\adb_fastboot\adb.exe
```

Same folder also has `fastboot.exe`. Prefer this path in PowerShell and in deploy scripts.

```powershell
$Adb = "E:\下载\adb_fastboot\adb.exe"
& $Adb devices -l
```

## Board identity

Typical serial when connected over USB:

```
95b7c21fd4859196
```

If `adb devices` shows `device`, the board is ready. Do not report “no adb” when only PATH is empty — check `E:\下载\adb_fastboot\adb.exe` first.

## Deploy

Project script: `scripts/lyra-adb-deploy.ps1` (candidates must include the path above).

```powershell
powershell -File E:\newvibecode\scripts\lyra-adb-deploy.ps1
# or force:
powershell -File E:\newvibecode\scripts\lyra-adb-deploy.ps1 -Adb "E:\下载\adb_fastboot\adb.exe"
```

Remote app dir: `/root/mooncoding`. WebEngine runtime stage: `build-board/qt6-stage/`.

## WSL note

Board deploy is driven from **Windows adb**, not WSL `adb`. WSL is for cross-build / Buildroot only.
