#!/bin/bash
# Cross-build full MoonCoding for Lyra (armhf) against Buildroot Qt6 sysroot.
set -euo pipefail
# Buildroot/host tools break when Windows PATH (spaces) leaks into WSL.
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:${HOME}/.cargo/bin:${PATH}"
# Drop any PATH elements with spaces/tabs after prepend.
export PATH="$(echo "$PATH" | tr ':' '\n' | awk 'NF && $0 !~ /[ \t]/' | paste -sd: -)"
export LANG=C.UTF-8

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SDK="${LYRA_SDK_ROOT:-${HOME}/Lyra-sdk}"
BR_OUT="${LYRA_BR_OUTPUT:-${SDK}/buildroot/output/rockchip_rk3506_luckfox}"
HOST="${BR_OUT}/host"
SYSROOT="${HOST}/arm-buildroot-linux-gnueabihf/sysroot"
BUILD_DIR="${ROOT}/build-board"
TOOLCHAIN="${ROOT}/cmake/lyra-rk3506-toolchain.cmake"
RUST_TARGET="armv7-unknown-linux-gnueabihf"

echo "ROOT=${ROOT}"
echo "BR_OUT=${BR_OUT}"

if [[ ! -x "${HOST}/bin/arm-buildroot-linux-gnueabihf-g++" ]]; then
  echo "ERROR: cross g++ missing — wait for Buildroot host toolchain at ${HOST}"
  exit 1
fi

# Qt6 Widgets must exist in sysroot (after qt6base installs).
if [[ ! -e "${SYSROOT}/usr/lib/libQt6Widgets.so" ]] \
   && [[ ! -e "${SYSROOT}/usr/lib/libQt6Widgets.so.6" ]] \
   && ! ls "${SYSROOT}/usr/lib"/libQt6Widgets.so* >/dev/null 2>&1; then
  echo "ERROR: Qt6 Widgets not in sysroot yet — wait for qt6base in buildroot"
  echo "  log: ${HOME}/lyra-qt6-buildroot.log"
  exit 1
fi

rustup target add "${RUST_TARGET}" >/dev/null || true

export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER="${HOST}/bin/arm-buildroot-linux-gnueabihf-gcc"
export CC_armv7_unknown_linux_gnueabihf="${HOST}/bin/arm-buildroot-linux-gnueabihf-gcc"
export CXX_armv7_unknown_linux_gnueabihf="${HOST}/bin/arm-buildroot-linux-gnueabihf-g++"
export PKG_CONFIG_SYSROOT_DIR="${SYSROOT}"
export PKG_CONFIG_PATH="${SYSROOT}/usr/lib/pkgconfig:${SYSROOT}/usr/share/pkgconfig"

mkdir -p "${BUILD_DIR}"
cmake -S "${ROOT}" -B "${BUILD_DIR}" \
  -DCMAKE_TOOLCHAIN_FILE="${TOOLCHAIN}" \
  -DLYRA_SDK_ROOT="${SDK}" \
  -DLYRA_BR_OUTPUT="${BR_OUT}" \
  -DMOONCODING_BUILD_TESTS=OFF \
  -DMOONCODING_RUST_TARGET="${RUST_TARGET}" \
  -DCMAKE_BUILD_TYPE=Release

cmake --build "${BUILD_DIR}" -j"$(nproc)"

echo "=== artifacts ==="
find "${BUILD_DIR}" -maxdepth 4 \( -name mooncoding -o -name 'libvibe_agent.so' -o -name vibe \) -type f 2>/dev/null
echo "CROSS_BUILD_OK"
