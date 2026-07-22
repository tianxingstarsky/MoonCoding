#!/bin/bash
# Poll qt6webengine build until BUILD_OK / failure. Prints short status lines.
set -euo pipefail
LOG="${1:-/mnt/e/newvibecode/build-board/lyra-qt6webengine-build.log}"
OUT="${LYRA_BR_OUTPUT:-$HOME/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox}"
for i in $(seq 1 360); do
  if grep -q '^BUILD_OK$' "$LOG" 2>/dev/null; then
    echo "DONE BUILD_OK"
    ls -la "$OUT/target/usr/lib"/libQt6WebEngine* 2>/dev/null | head || true
    find "$OUT/target" -name QtWebEngineProcess 2>/dev/null | head
    exit 0
  fi
  if ! pgrep -f 'make -C .*qt6webengine' >/dev/null 2>&1; then
    echo "MAKE_EXITED"
    tail -80 "$LOG"
    # treat missing BUILD_OK as failure
    exit 1
  fi
  pkgs=$(ls "$OUT/build" 2>/dev/null | grep -iE 'qt6web|libnss|nodejs|jpeg' | tr '\n' ' ')
  echo "[$i] still building… $pkgs"
  sleep 60
done
echo "TIMEOUT after 6h"
exit 2
