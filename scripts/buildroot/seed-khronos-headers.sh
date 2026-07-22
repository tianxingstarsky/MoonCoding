#!/bin/bash
# Seed Khronos/EGL/GLES headers into the Buildroot sysroot from the WSL host
# so Qt WebEngine configure can pass (Chromium requires these headers).
set -euo pipefail
SYSROOT="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/host/arm-buildroot-linux-gnueabihf/sysroot"
INC="${SYSROOT}/usr/include"
mkdir -p "$INC"
for d in KHR EGL GLES GLES2 GLES3; do
  if [[ -d "/usr/include/$d" ]]; then
    rm -rf "${INC}/${d}"
    cp -a "/usr/include/${d}" "${INC}/${d}"
    echo "seeded ${d}"
  fi
done
# Also seed into target staging mirror if distinct
TARGET_INC="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/target/usr/include"
if [[ -d "$(dirname "$TARGET_INC")" ]]; then
  mkdir -p "$TARGET_INC"
  for d in KHR EGL GLES GLES2 GLES3; do
    if [[ -d "/usr/include/$d" ]]; then
      rm -rf "${TARGET_INC}/${d}"
      cp -a "/usr/include/${d}" "${TARGET_INC}/${d}"
    fi
  done
fi
test -f "${INC}/KHR/khrplatform.h"
test -f "${INC}/EGL/egl.h"
test -f "${INC}/GLES2/gl2.h"
echo "KHRONOS_HEADERS_OK"
