#!/bin/bash
# Reconfigure qt6webengine in-place (keep Chromium object cache) after conf opts change.
set -euo pipefail
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:${HOME}/.cargo/bin"
OUT="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox"
BR="${HOME}/Lyra-sdk/buildroot"
export PATH="${PATH}:${OUT}/host/bin"
LOG="/mnt/e/newvibecode/build-board/lyra-qt6webengine-build.log"
JOBS="$(nproc)"

bash /mnt/e/newvibecode/scripts/buildroot/seed-khronos-headers.sh
bash /mnt/e/newvibecode/scripts/buildroot/install-host-gn.sh
bash /mnt/e/newvibecode/scripts/buildroot/fix-sysroot-path-doubling.sh
bash /mnt/e/newvibecode/scripts/buildroot/ensure-host-nss.sh
test -x "${OUT}/host/bin/gn"

force_installed() {
  local name="$1"
  local d="${OUT}/build/${name}"
  mkdir -p "$d"
  for s in downloaded extracted patched configured built staging_installed target_installed installed host_installed; do
    touch "${d}/.stamp_${s}"
  done
}
force_installed host-qt6shadertools-6.4.3
force_installed host-qt6declarative-6.4.3
force_installed qt6shadertools-6.4.3
force_installed qt6declarative-6.4.3
force_installed qt6webchannel-6.4.3

cp -a /mnt/e/newvibecode/scripts/buildroot/qt6/qt6webengine/. "${BR}/package/qt6/qt6webengine/"

WE="${OUT}/build/qt6webengine-6.4.3"
test -d "${WE}"
# Force reconfigure + rebuild without wiping extracted sources / object cache.
rm -f "${WE}/.stamp_configured" "${WE}/.stamp_built" \
  "${WE}/.stamp_staging_installed" "${WE}/.stamp_target_installed" \
  "${WE}/.stamp_installed"
rm -f "${WE}/buildroot-build/CMakeCache.txt"

echo "===== $(date -Is) RESUME reconfigure qt6webengine jobs=${JOBS} =====" >>"${LOG}"
make -C "${BR}" O="${OUT}" qt6webengine -j"${JOBS}" >>"${LOG}" 2>&1
if ls "${OUT}/target/usr/lib"/libQt6WebEngineCore.so* >/dev/null 2>&1 \
  || ls "${OUT}/host/arm-buildroot-linux-gnueabihf/sysroot/usr/lib"/libQt6WebEngineCore.so* >/dev/null 2>&1; then
  echo BUILD_OK >>"${LOG}"
  echo BUILD_OK
else
  echo "ERROR: WebEngine libs missing" >>"${LOG}"
  exit 1
fi
