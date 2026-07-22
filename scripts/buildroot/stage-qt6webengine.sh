#!/bin/bash
# Stage Qt WebEngine runtime next to qt6-stage for adb deploy.
# Safe no-op when WebEngine has not been built yet.
set -euo pipefail
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"

HOST="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/host"
SYS="${HOST}/arm-buildroot-linux-gnueabihf/sysroot"
TARGET="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/target"
STAGE="/mnt/e/newvibecode/build-board/qt6-stage"

mkdir -p "${STAGE}/lib" "${STAGE}/libexec" "${STAGE}/resources" \
  "${STAGE}/translations/qtwebengine_locales"

copied=0
copy_one() {
  local src="$1" dest="$2"
  if [[ -e "$src" ]]; then
    mkdir -p "$(dirname "$dest")"
    # Dereference symlinks so Windows adb can push real files (WSL symlinks
    # appear as 0-byte stubs on the E: mount).
    cp -aL "$src" "$dest" 2>/dev/null || cp -a "$src" "$dest"
    # If dest is still empty symlink/stub, replace with resolved content.
    if [[ ! -s "$dest" ]] && [[ -f "$src" ]]; then
      rm -f "$dest"
      cp -L "$src" "$dest" 2>/dev/null || cat "$src" > "$dest"
    fi
    echo "staged $(basename "$src")"
    copied=$((copied + 1))
  fi
}

# Shared libs
for pat in \
  libQt6WebEngineCore.so* \
  libQt6WebEngineWidgets.so* \
  libQt6WebChannel.so* \
  libQt6Network.so* \
  libQt6Positioning.so* \
  libQt6Qml.so* \
  libQt6Quick.so* \
  libnss3.so* libnssutil3.so* libsmime3.so* libssl3.so* libnspr4.so* \
  libplc4.so* libplds4.so*
do
  for base in "${TARGET}/usr/lib" "${SYS}/usr/lib"; do
    for f in ${base}/${pat}; do
      [[ -e "$f" ]] || continue
      copy_one "$f" "${STAGE}/lib/$(basename "$f")"
    done
  done
done

# Process binary
for cand in \
  "${TARGET}/usr/libexec/QtWebEngineProcess" \
  "${TARGET}/usr/lib/QtWebEngineProcess" \
  "${SYS}/usr/libexec/QtWebEngineProcess" \
  "${HOST}/libexec/QtWebEngineProcess"
do
  if [[ -x "$cand" ]]; then
    copy_one "$cand" "${STAGE}/libexec/QtWebEngineProcess"
    break
  fi
done

# Resources / locales
for base in "${TARGET}/usr/resources" "${SYS}/usr/resources" \
            "${TARGET}/usr/share/qt6/resources" "${SYS}/usr/share/qt6/resources"
do
  if [[ -d "$base" ]]; then
    cp -a "$base"/. "${STAGE}/resources/" || true
    copied=$((copied + 1))
    break
  fi
done

for base in \
  "${TARGET}/usr/translations/qtwebengine_locales" \
  "${SYS}/usr/translations/qtwebengine_locales" \
  "${TARGET}/usr/share/qt6/translations/qtwebengine_locales"
do
  if [[ -d "$base" ]]; then
    cp -a "$base"/. "${STAGE}/translations/qtwebengine_locales/" || true
    copied=$((copied + 1))
    break
  fi
done

if (( copied == 0 )); then
  echo "WEBENGINE_STAGE_SKIP: no Qt WebEngine artifacts in sysroot/target yet"
  exit 0
fi

echo "WEBENGINE_STAGE_OK (${copied} copy ops)"
ls -la "${STAGE}/libexec" 2>/dev/null | head || true
ls "${STAGE}/lib" | grep -iE 'WebEngine|WebChannel|Network' | head || true
