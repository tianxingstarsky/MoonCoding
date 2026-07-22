#!/bin/bash
set -euo pipefail
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:${HOME}/.cargo/bin"
OUT="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox"
BR="${HOME}/Lyra-sdk/buildroot"
export PATH="${PATH}:${OUT}/host/bin"
LOG="/mnt/e/newvibecode/build-board/lyra-qt6webengine-build.log"
CFG="${OUT}/.config"

# Ensure network module is on
if ! grep -q '^BR2_PACKAGE_QT6BASE_NETWORK=y$' "$CFG"; then
  sed -i 's/^# BR2_PACKAGE_QT6BASE_NETWORK is not set$/BR2_PACKAGE_QT6BASE_NETWORK=y/' "$CFG" || true
  grep -q '^BR2_PACKAGE_QT6BASE_NETWORK=y$' "$CFG" || echo 'BR2_PACKAGE_QT6BASE_NETWORK=y' >>"$CFG"
fi

if [[ ! -f "${OUT}/host/arm-buildroot-linux-gnueabihf/sysroot/usr/lib/cmake/Qt6Network/Qt6NetworkConfig.cmake" ]]; then
  echo "Rebuilding qt6base with Network..."
  # Cannot fake stamps — need real rebuild
  rm -rf "${OUT}/build/qt6base-6.4.3"
  make -C "${BR}" O="${OUT}" olddefconfig
  make -C "${BR}" O="${OUT}" qt6base -j"$(nproc)"
fi

test -f "${OUT}/host/arm-buildroot-linux-gnueabihf/sysroot/usr/lib/cmake/Qt6Network/Qt6NetworkConfig.cmake"
echo "Qt6Network OK"

# Re-assert Quick/Shader stamps after qt6base rebuild may have disturbed deps
bash /mnt/e/newvibecode/scripts/buildroot/force-webengine-build.sh
