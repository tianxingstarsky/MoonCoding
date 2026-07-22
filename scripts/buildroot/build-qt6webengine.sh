#!/bin/bash
# Build qt6webengine (and deps) for Luckfox Lyra Buildroot output.
# Logs to ~/lyra-qt6webengine-build.log (override with LYRA_WEBENGINE_LOG).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SDK="${LYRA_SDK_ROOT:-${HOME}/Lyra-sdk}"
BR="${SDK}/buildroot"
OUT="${LYRA_BR_OUTPUT:-${BR}/output/rockchip_rk3506_luckfox}"
LOG="${LYRA_WEBENGINE_LOG:-${HOME}/lyra-qt6webengine-build.log}"
JOBS="${BR2_JLEVEL:-$(nproc)}"

bash "${ROOT}/scripts/buildroot/install-qt6webengine.sh"

# Prefer dl mirror cache: if operator already downloaded tarball to /mnt/e/下载
DL="${BR}/dl"
mkdir -p "${DL}"
for f in \
  /mnt/e/下载/qt6.4.3/qtwebchannel-everywhere-src-6.4.3.tar.xz \
  /mnt/e/下载/qt6.4.3/qtwebengine-everywhere-src-6.4.3.tar.xz
do
  if [[ -f "$f" ]]; then
    base=$(basename "$f")
    if [[ ! -f "${DL}/${base}" ]]; then
      echo "Caching ${base} into Buildroot dl/"
      cp -a "$f" "${DL}/${base}"
    fi
  fi
done

echo "Building qt6webengine (jobs=${JOBS}) — log: ${LOG}"
# shellcheck disable=SC2086
make -C "${BR}" O="${OUT}" qt6webengine -j${JOBS} 2>&1 | tee "${LOG}"
echo "BUILD_OK"
ls -la "${OUT}/host/arm-buildroot-linux-gnueabihf/sysroot/usr/lib"/libQt6WebEngine* 2>/dev/null | head || true
ls -la "${OUT}/target/usr/lib"/libQt6WebEngine* 2>/dev/null | head || true
find "${OUT}/target" -name 'QtWebEngineProcess' 2>/dev/null | head
