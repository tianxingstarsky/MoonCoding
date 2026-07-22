#!/bin/bash
# Finish WebEngine without Quick (Apps uses QWebEngineView / Widgets only).
# QQmlWebChannel is missing from this sysroot, so Quick cannot link.
set -euo pipefail
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:${HOME}/.cargo/bin"
OUT="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox"
BR="${HOME}/Lyra-sdk/buildroot"
export PATH="${PATH}:${OUT}/host/bin"
WE_BUILD="${OUT}/build/qt6webengine-6.4.3/buildroot-build"
LOG="/mnt/e/newvibecode/build-board/lyra-qt6webengine-build.log"
JOBS="${MOONCODING_WE_JOBS:-8}"

bash /mnt/e/newvibecode/scripts/buildroot/fix-sysroot-path-doubling.sh
bash /mnt/e/newvibecode/scripts/buildroot/ensure-host-nss.sh
bash /mnt/e/newvibecode/scripts/buildroot/patch-webengine-time-abi.sh
bash /mnt/e/newvibecode/scripts/buildroot/fix-webengine-process-time-abi.sh

test -f "${WE_BUILD}/lib/libQt6WebEngineCore.so.6.4.3"
test -f "${WE_BUILD}/lib/libQt6WebEngineWidgets.so.6.4.3"
test -x "${WE_BUILD}/libexec/QtWebEngineProcess"

# Disable Quick in the already-configured tree.
if [[ -f "${WE_BUILD}/CMakeCache.txt" ]]; then
  sed -i \
    -e 's/^QT_FEATURE_qtwebengine_quick_build:.*=.*/QT_FEATURE_qtwebengine_quick_build:BOOL=OFF/' \
    -e 's/^FEATURE_qtwebengine_quick_build:.*=.*/FEATURE_qtwebengine_quick_build:BOOL=OFF/' \
    "${WE_BUILD}/CMakeCache.txt" || true
  if ! grep -q 'QT_FEATURE_qtwebengine_quick_build:BOOL=OFF' "${WE_BUILD}/CMakeCache.txt"; then
    echo 'QT_FEATURE_qtwebengine_quick_build:BOOL=OFF' >> "${WE_BUILD}/CMakeCache.txt"
  fi
fi

# Build only the modules we need, then install.
echo "===== $(date -Is) FINISH widgets-only qt6webengine jobs=${JOBS} =====" >>"${LOG}"
(
  cd "${WE_BUILD}"
  # Ensure process is linked with fixed main.o
  ninja -j"${JOBS}" \
    lib/libQt6WebEngineCore.so.6.4.3 \
    lib/libQt6WebEngineWidgets.so.6.4.3 \
    libexec/QtWebEngineProcess \
    >>"${LOG}" 2>&1
  # Install without building Quick: use cmake install for existing targets.
  # Exclude quick targets by installing via cmake component-less but skipping failing deps:
  cmake --install . --prefix /usr \
    >>"${LOG}" 2>&1 || true
)

# cmake --install may partially fail; force-copy the critical artifacts.
SYS="${OUT}/host/arm-buildroot-linux-gnueabihf/sysroot"
TARGET="${OUT}/target"
for root in "${SYS}" "${TARGET}"; do
  mkdir -p "${root}/usr/lib" "${root}/usr/libexec" "${root}/usr/lib/cmake"
  cp -a "${WE_BUILD}/lib"/libQt6WebEngineCore.so* "${root}/usr/lib/"
  cp -a "${WE_BUILD}/lib"/libQt6WebEngineWidgets.so* "${root}/usr/lib/"
  cp -a "${WE_BUILD}/libexec/QtWebEngineProcess" "${root}/usr/libexec/"
  # resources / locales if present
  if [[ -d "${WE_BUILD}/translations" ]]; then
    mkdir -p "${root}/usr/translations"
    cp -a "${WE_BUILD}/translations"/qtwebengine* "${root}/usr/translations/" 2>/dev/null || true
  fi
  for res in resources/icudtl.dat resources/qtwebengine_resources.pak \
             resources/qtwebengine_resources_100p.pak resources/qtwebengine_resources_200p.pak \
             resources/qtwebengine_devtools_resources.pak; do
    if [[ -f "${WE_BUILD}/${res}" ]]; then
      mkdir -p "${root}/usr/resources"
      cp -a "${WE_BUILD}/${res}" "${root}/usr/resources/"
    fi
  done
  # Also look under libexec / share common layouts
  if [[ -d "${WE_BUILD}/src/core/Release/armv7l" ]]; then
    find "${WE_BUILD}/src/core/Release/armv7l" -maxdepth 2 \( -name '*.pak' -o -name 'icudtl.dat' -o -name 'v8_context_snapshot.bin' -o -name 'snapshot_blob.bin' \) \
      -exec cp -a {} "${root}/usr/resources/" \; 2>/dev/null || true
    mkdir -p "${root}/usr/resources"
  fi
done

# CMake packages (needed to cross-link mooncoding)
if [[ -d "${WE_BUILD}/lib/cmake/Qt6WebEngineCore" ]]; then
  cp -a "${WE_BUILD}/lib/cmake/Qt6WebEngineCore" "${SYS}/usr/lib/cmake/"
fi
if [[ -d "${WE_BUILD}/lib/cmake/Qt6WebEngineWidgets" ]]; then
  cp -a "${WE_BUILD}/lib/cmake/Qt6WebEngineWidgets" "${SYS}/usr/lib/cmake/"
fi
# Sometimes cmake configs live under build dir differently
find "${WE_BUILD}" -type d -name 'Qt6WebEngineCore' -path '*/cmake/*' 2>/dev/null | head -5
find "${WE_BUILD}" -type d -name 'Qt6WebEngineWidgets' -path '*/cmake/*' 2>/dev/null | head -5

# Mark Buildroot stamps so dependents see package as installed.
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
  ls -la "${SYS}/usr/lib"/libQt6WebEngineCore.so* "${SYS}/usr/lib"/libQt6WebEngineWidgets.so* "${SYS}/usr/libexec/QtWebEngineProcess"
else
  echo "ERROR: widgets WebEngine artifacts missing after finish script" >>"${LOG}"
  exit 1
fi
