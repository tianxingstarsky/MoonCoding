#!/bin/bash
# Cross-compiling Qt WebEngine requires a *host* gn on PATH.
# QT_FEATURE_webengine_build_gn cannot satisfy that (Qt fatals before building gn).
set -euo pipefail
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:${HOME}/.cargo/bin"
OUT="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox"
BR="${HOME}/Lyra-sdk/buildroot"
HOST_BIN="${OUT}/host/bin"
GN_BIN="${HOST_BIN}/gn"
# Scratch on E: only
WORK="/mnt/e/newvibecode/build-board/host-gn-build"

find_tarball() {
  local c
  for c in \
    "${BR}/dl/qt6webengine/qtwebengine-everywhere-src-6.4.3.tar.xz" \
    "${OUT}/../../dl/qt6webengine/qtwebengine-everywhere-src-6.4.3.tar.xz" \
    "/home/mooncoding/Lyra-sdk/buildroot/dl/qt6webengine/qtwebengine-everywhere-src-6.4.3.tar.xz"
  do
    if [[ -f "$c" ]]; then
      echo "$c"
      return 0
    fi
  done
  return 1
}

mkdir -p "${HOST_BIN}"
export PATH="${HOST_BIN}:${PATH}"

if [[ -x "${GN_BIN}" ]]; then
  echo "host gn already present: $("${GN_BIN}" --version 2>&1 | head -1)"
  exit 0
fi

if command -v gn >/dev/null 2>&1; then
  cp "$(command -v gn)" "${GN_BIN}"
  chmod +x "${GN_BIN}"
  echo "copied system gn -> ${GN_BIN} ($("${GN_BIN}" --version 2>&1 | head -1))"
  exit 0
fi

# Reuse already-extracted WebEngine tree if present (before force wipe).
GN_SRC=""
for c in \
  "${OUT}/build/qt6webengine-6.4.3/src/gn" \
  "/mnt/e/newvibecode/build-board/qt6webengine-src/src/gn"
do
  if [[ -f "${c}/CMakeLists.txt" ]]; then
    GN_SRC="$c"
    break
  fi
done

if [[ -z "${GN_SRC}" ]]; then
  DL_TAR="$(find_tarball)" || {
    echo "ERROR: qtwebengine tarball not found (need bundled gn sources)"
    exit 1
  }
  rm -rf "${WORK}"
  mkdir -p "${WORK}"
  PREFIX="$(tar -tJf "${DL_TAR}" | head -1 | cut -d/ -f1)"
  echo "extracting ${PREFIX}/src/gn from $(basename "${DL_TAR}") ..."
  tar -xJf "${DL_TAR}" -C "${WORK}" "${PREFIX}/src/gn"
  GN_SRC="${WORK}/${PREFIX}/src/gn"
fi

test -f "${GN_SRC}/CMakeLists.txt"
rm -rf "${WORK}/build"
mkdir -p "${WORK}/build"

cmake -S "${GN_SRC}" -B "${WORK}/build" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_C_COMPILER=gcc \
  -DCMAKE_CXX_COMPILER=g++ \
  -DCMAKE_INSTALL_PREFIX="${OUT}/host"
ninja -C "${WORK}/build"
cmake --install "${WORK}/build"

# Some installs put gn under libexec; ensure HOST_BIN/gn exists.
if [[ ! -x "${GN_BIN}" ]]; then
  found="$(find "${OUT}/host" -type f -name gn 2>/dev/null | head -1 || true)"
  if [[ -n "${found}" ]]; then
    cp "${found}" "${GN_BIN}"
    chmod +x "${GN_BIN}"
  fi
fi

test -x "${GN_BIN}"
echo "HOST_GN_OK $("${GN_BIN}" --version 2>&1 | head -1)"
