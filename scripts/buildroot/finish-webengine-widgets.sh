#!/bin/bash
# Install WebEngine Core+Widgets+Process without Quick and without letting
# ninja rebuild process main.cpp.o with -D_TIME_BITS=64.
set -euo pipefail
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:${HOME}/.cargo/bin"
OUT="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox"
export PATH="${PATH}:${OUT}/host/bin"
WE_BUILD="${OUT}/build/qt6webengine-6.4.3/buildroot-build"
LOG="/mnt/e/newvibecode/build-board/lyra-qt6webengine-build.log"
NINJA="${OUT}/host/bin/ninja"
NM="${OUT}/host/bin/arm-buildroot-linux-gnueabihf-nm"

bash /mnt/e/newvibecode/scripts/buildroot/fix-sysroot-path-doubling.sh
bash /mnt/e/newvibecode/scripts/buildroot/ensure-host-nss.sh
bash /mnt/e/newvibecode/scripts/buildroot/patch-webengine-time-abi.sh
bash /mnt/e/newvibecode/scripts/buildroot/fix-webengine-process-time-abi.sh

test -f "${WE_BUILD}/lib/libQt6WebEngineCore.so.6.4.3"
test -f "${WE_BUILD}/lib/libQt6WebEngineWidgets.so.6.4.3"

cd "${WE_BUILD}"

# Manually link QtWebEngineProcess using the fixed main.o (do NOT ninja-build it).
"${NINJA}" -t commands libexec/QtWebEngineProcess | tail -n 1 > /tmp/we-process-link-raw.txt
python3 - <<'PY' > /tmp/we-process-link.sh
import shlex
from pathlib import Path
raw = Path('/tmp/we-process-link-raw.txt').read_text().strip()
# CMake/ninja often emits: : && /path/g++ ... && :
if raw.startswith(': && '):
    raw = raw[len(': && '):]
if raw.endswith(' && :'):
    raw = raw[: -len(' && :')]
args = shlex.split(raw)
if not args:
    raise SystemExit('empty link command')
print('#!/bin/bash')
print('set -euo pipefail')
print('cd /home/mooncoding/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/build/qt6webengine-6.4.3/buildroot-build')
print('exec ' + ' '.join(shlex.quote(a) for a in args))
PY
chmod +x /tmp/we-process-link.sh
# Debug
head -5 /tmp/we-process-link.sh
echo '--- raw ---'
head -c 200 /tmp/we-process-link-raw.txt; echo

# Confirm main.o still has long (not long long) undefs
if "${NM}" -u src/process/CMakeFiles/QtWebEngineProcess.dir/main.cpp.o | grep -q 'localtime_overrideEPKx'; then
  echo "ERROR: main.cpp.o again wants long long — refusing to link"
  exit 1
fi
echo "manual link QtWebEngineProcess ..."
bash /tmp/we-process-link.sh
test -x libexec/QtWebEngineProcess

# Touch outputs so later ninja thinks they're fresh (optional)
touch libexec/QtWebEngineProcess \
  lib/libQt6WebEngineCore.so.6.4.3 \
  lib/libQt6WebEngineWidgets.so.6.4.3

echo "===== $(date -Is) FINISH widgets-only copy install =====" >>"${LOG}"

SYS="${OUT}/host/arm-buildroot-linux-gnueabihf/sysroot"
TARGET="${OUT}/target"
for root in "${SYS}" "${TARGET}"; do
  mkdir -p "${root}/usr/lib" "${root}/usr/libexec" "${root}/usr/resources" "${root}/usr/lib/cmake"
  cp -a "${WE_BUILD}/lib"/libQt6WebEngineCore.so* "${root}/usr/lib/"
  cp -a "${WE_BUILD}/lib"/libQt6WebEngineWidgets.so* "${root}/usr/lib/"
  cp -a "${WE_BUILD}/libexec/QtWebEngineProcess" "${root}/usr/libexec/"
done

# Resources from Chromium output
RES_DIR="${OUT}/build/qt6webengine-6.4.3/buildroot-build/src/core/Release/armv7l"
if [[ -d "${RES_DIR}" ]]; then
  for root in "${SYS}" "${TARGET}"; do
    mkdir -p "${root}/usr/resources"
    find "${RES_DIR}" -maxdepth 2 \( \
        -name 'icudtl.dat' -o -name '*.pak' -o -name 'v8_context_snapshot.bin' \
        -o -name 'snapshot_blob.bin' -o -name 'chrome_100_percent.pak' \
        -o -name 'chrome_200_percent.pak' \) \
      -exec cp -a {} "${root}/usr/resources/" \;
  done
fi

# CMake package configs if generated
for name in Qt6WebEngineCore Qt6WebEngineWidgets Qt6WebEngineCoreTools; do
  d="$(find "${WE_BUILD}" -type d -path "*/cmake/${name}" 2>/dev/null | head -1 || true)"
  if [[ -n "${d}" ]]; then
    cp -a "${d}" "${SYS}/usr/lib/cmake/"
  fi
done

# Also copy headers if present
if [[ -d "${WE_BUILD}/include/QtWebEngineCore" ]]; then
  mkdir -p "${SYS}/usr/include" "${TARGET}/usr/include"
  cp -a "${WE_BUILD}/include/QtWebEngineCore" "${SYS}/usr/include/"
  cp -a "${WE_BUILD}/include/QtWebEngineWidgets" "${SYS}/usr/include/" 2>/dev/null || true
  cp -a "${WE_BUILD}/include/QtWebEngineCore" "${TARGET}/usr/include/" 2>/dev/null || true
  cp -a "${WE_BUILD}/include/QtWebEngineWidgets" "${TARGET}/usr/include/" 2>/dev/null || true
fi

WE_PKG="${OUT}/build/qt6webengine-6.4.3"
touch "${WE_PKG}/.stamp_built" \
  "${WE_PKG}/.stamp_staging_installed" \
  "${WE_PKG}/.stamp_target_installed" \
  "${WE_PKG}/.stamp_installed"

if ls "${SYS}/usr/lib"/libQt6WebEngineCore.so* >/dev/null 2>&1 \
  && ls "${SYS}/usr/lib"/libQt6WebEngineWidgets.so* >/dev/null 2>&1 \
  && test -x "${SYS}/usr/libexec/QtWebEngineProcess"; then
  echo BUILD_OK_WIDGETS >>"${LOG}"
  echo BUILD_OK_WIDGETS
  ls -la "${SYS}/usr/lib"/libQt6WebEngineCore.so.6.4.3 \
    "${SYS}/usr/lib"/libQt6WebEngineWidgets.so.6.4.3 \
    "${SYS}/usr/libexec/QtWebEngineProcess"
  ls "${SYS}/usr/resources" 2>/dev/null | head -20 || true
else
  echo "ERROR: widgets WebEngine artifacts missing" >>"${LOG}"
  exit 1
fi
