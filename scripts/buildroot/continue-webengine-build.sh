#!/bin/bash
# Continue qt6webengine build after a compile fix (keep configure + object cache).
set -euo pipefail
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:${HOME}/.cargo/bin"
OUT="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox"
BR="${HOME}/Lyra-sdk/buildroot"
export PATH="${PATH}:${OUT}/host/bin"
LOG="/mnt/e/newvibecode/build-board/lyra-qt6webengine-build.log"
JOBS="$(nproc)"

bash /mnt/e/newvibecode/scripts/buildroot/fix-sysroot-path-doubling.sh
test -x "${OUT}/host/bin/gn"
test -d "${OUT}/build/qt6webengine-6.4.3/buildroot-build"

# Only rebuild / reinstall; do not wipe configure.
rm -f "${OUT}/build/qt6webengine-6.4.3/.stamp_built" \
  "${OUT}/build/qt6webengine-6.4.3/.stamp_staging_installed" \
  "${OUT}/build/qt6webengine-6.4.3/.stamp_target_installed" \
  "${OUT}/build/qt6webengine-6.4.3/.stamp_installed"

echo "===== $(date -Is) CONTINUE qt6webengine jobs=${JOBS} =====" >>"${LOG}"
make -C "${BR}" O="${OUT}" qt6webengine -j"${JOBS}" >>"${LOG}" 2>&1
if ls "${OUT}/target/usr/lib"/libQt6WebEngineCore.so* >/dev/null 2>&1 \
  || ls "${OUT}/host/arm-buildroot-linux-gnueabihf/sysroot/usr/lib"/libQt6WebEngineCore.so* >/dev/null 2>&1; then
  echo BUILD_OK >>"${LOG}"
  echo BUILD_OK
else
  echo "ERROR: WebEngine libs missing" >>"${LOG}"
  exit 1
fi
