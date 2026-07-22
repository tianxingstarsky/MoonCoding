#!/bin/bash
# Chromium (Qt WebEngine) prepends target_sysroot onto absolute -I/-L paths from
# pkg-config, producing doubled paths like:
#   $SYSROOT/home/.../host/bin/../arm-.../sysroot/usr/include/...
# Point that nested sysroot at the real one so headers resolve.
set -euo pipefail
OUT="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox"
SYS="${OUT}/host/arm-buildroot-linux-gnueabihf/sysroot"
# Resolved form of host/bin/../arm-buildroot-linux-gnueabihf/sysroot
NESTED="${SYS}/home/mooncoding/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/host/arm-buildroot-linux-gnueabihf/sysroot"
# Chromium -I uses host/bin/../arm-... ; create bin so ".." resolves.
mkdir -p "${SYS}/home/mooncoding/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/host/bin"
mkdir -p "$(dirname "${NESTED}")"
if [[ -L "${NESTED}" || -e "${NESTED}" ]]; then
  rm -rf "${NESTED}"
fi
ln -sfn "${SYS}" "${NESTED}"
# Sanity: doubled dbus + freetype paths must resolve
test -f "${NESTED}/usr/include/dbus-1.0/dbus/dbus.h"
test -f "${NESTED}/usr/include/freetype2/ft2build.h"
# Also cover the literal bin/../ spelling (same inode after resolution)
VIA_BIN="${SYS}/home/mooncoding/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/host/bin/../arm-buildroot-linux-gnueabihf/sysroot/usr/include/dbus-1.0/dbus/dbus.h"
test -f "${VIA_BIN}"
echo "SYSROOT_PATH_DOUBLING_OK ${NESTED} -> ${SYS}"
