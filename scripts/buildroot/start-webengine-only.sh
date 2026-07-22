#!/bin/bash
set -euo pipefail
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:${HOME}/.cargo/bin"
OUT="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox"
BR="${HOME}/Lyra-sdk/buildroot"
export PATH="${PATH}:${OUT}/host/bin"
LOG="/mnt/e/newvibecode/build-board/lyra-qt6webengine-build.log"

python3 -c 'import html5lib; print("html5lib OK", html5lib.__version__)'
test -x "${OUT}/host/bin/qsb"
test -e "${OUT}/target/usr/lib/libQt6Quick.so.6"

# If host tools already installed but stamps incomplete (interrupted), mark done.
mark_host_done() {
  local d="$1"
  [[ -d "$d" ]] || return 0
  if [[ -x "${OUT}/host/bin/qml" ]] || [[ -x "${OUT}/host/bin/qsb" ]]; then
    touch "$d/.stamp_built" "$d/.stamp_host_installed" "$d/.stamp_installed" 2>/dev/null || true
  fi
}
mark_host_done "${OUT}/build/host-qt6shadertools-6.4.3"
mark_host_done "${OUT}/build/host-qt6declarative-6.4.3"

# Target declarative/shadertools already in sysroot — restore stamps if build dir gone/partial
for pkg in qt6shadertools-6.4.3 qt6declarative-6.4.3 qt6webchannel-6.4.3; do
  d="${OUT}/build/${pkg}"
  if [[ -d "$d" ]]; then
    touch "$d/.stamp_configured" "$d/.stamp_built" "$d/.stamp_staging_installed" \
      "$d/.stamp_target_installed" "$d/.stamp_installed" 2>/dev/null || true
  fi
done

cp -a /mnt/e/newvibecode/scripts/buildroot/qt6/qt6webengine/. "${BR}/package/qt6/qt6webengine/"
rm -rf "${OUT}/build/qt6webengine-6.4.3"

{
  echo "===== $(date -Is) qt6webengine-only start ====="
  make -C "${BR}" O="${OUT}" qt6webengine -j"$(nproc)"
  if ls "${OUT}/target/usr/lib"/libQt6WebEngineCore.so* >/dev/null 2>&1 \
    || ls "${OUT}/host/arm-buildroot-linux-gnueabihf/sysroot/usr/lib"/libQt6WebEngineCore.so* >/dev/null 2>&1; then
    echo BUILD_OK
  else
    echo "ERROR: WebEngine libs missing"
    exit 1
  fi
} >>"${LOG}" 2>&1
