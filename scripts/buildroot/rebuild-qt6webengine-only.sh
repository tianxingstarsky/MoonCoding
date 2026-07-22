#!/bin/bash
# Rebuild ONLY qt6webengine (assumes declarative/shadertools already installed).
set -euo pipefail
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:${HOME}/.cargo/bin:${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/host/bin"
OUT="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox"
BR="${HOME}/Lyra-sdk/buildroot"
LOG="/mnt/e/newvibecode/build-board/lyra-qt6webengine-build.log"

python3 -c 'import html5lib; print("html5lib OK", html5lib.__version__)'
test -x "${OUT}/host/bin/qsb"
test -e "${OUT}/host/arm-buildroot-linux-gnueabihf/sysroot/usr/lib/libQt6Qml.so" \
  -o -e "${OUT}/host/arm-buildroot-linux-gnueabihf/sysroot/usr/lib/libQt6Qml.so.6" \
  || test -e "${OUT}/target/usr/lib/libQt6Qml.so" \
  || test -e "${OUT}/target/usr/lib/libQt6Qml.so.6"

cp -a /mnt/e/newvibecode/scripts/buildroot/qt6/qt6webengine/. "${BR}/package/qt6/qt6webengine/"
rm -rf "${OUT}/build/qt6webengine-6.4.3"

echo "Building qt6webengine only..." | tee "$LOG"
make -C "${BR}" O="${OUT}" qt6webengine -j"$(nproc)" 2>&1 | tee -a "$LOG"
# Verify real libs
if ! ls "${OUT}/host/arm-buildroot-linux-gnueabihf/sysroot/usr/lib"/libQt6WebEngineCore.so* >/dev/null 2>&1 \
  && ! ls "${OUT}/target/usr/lib"/libQt6WebEngineCore.so* >/dev/null 2>&1; then
  echo "ERROR: WebEngine libs missing after build" | tee -a "$LOG"
  exit 1
fi
echo BUILD_OK | tee -a "$LOG"
