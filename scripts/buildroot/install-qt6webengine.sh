#!/bin/bash
# Install MoonCoding qt6* packages for WebEngine into Luckfox Buildroot
# and enable them in the rockchip_rk3506_luckfox .config.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SRC="${ROOT}/scripts/buildroot/qt6"
SDK="${LYRA_SDK_ROOT:-${HOME}/Lyra-sdk}"
BR="${SDK}/buildroot"
PKG="${BR}/package/qt6"
OUT="${LYRA_BR_OUTPUT:-${BR}/output/rockchip_rk3506_luckfox}"
CFG="${OUT}/.config"
export PKG

echo "ROOT=${ROOT}"
echo "BR=${BR}"
echo "OUT=${OUT}"

if [[ ! -d "${PKG}" ]]; then
  echo "ERROR: missing ${PKG}"
  exit 1
fi

for p in qt6shadertools qt6declarative qt6webchannel qt6webengine; do
  mkdir -p "${PKG}/${p}"
  cp -a "${SRC}/${p}/." "${PKG}/${p}/"
  echo "installed ${p}"
done

python3 - <<'PY'
from pathlib import Path
import os
cfg = Path(os.environ["PKG"]) / "Config.in"
text = cfg.read_text()
needed = [
    'source "package/qt6/qt6shadertools/Config.in"',
    'source "package/qt6/qt6declarative/Config.in"',
    'source "package/qt6/qt6webchannel/Config.in"',
    'source "package/qt6/qt6webengine/Config.in"',
]
changed = False
for line in needed:
    if line not in text:
        # Insert after qt6svg source line (or at end of if-block before endif).
        needle = 'source "package/qt6/qt6svg/Config.in"'
        if needle in text:
            text = text.replace(needle, needle + "\n" + line, 1)
        else:
            text = text.replace("\nendif\n", "\n" + line + "\nendif\n", 1)
        changed = True
        print("added", line)
if changed:
    cfg.write_text(text)
else:
    print("Config.in already has all WebEngine-related sources")
PY

if [[ ! -f "${CFG}" ]]; then
  echo "ERROR: missing ${CFG} — run Buildroot configure first"
  exit 1
fi

enable() {
  local key="$1"
  if grep -q "^${key}=y$" "${CFG}"; then
    return 0
  fi
  if grep -q "^# ${key} is not set$" "${CFG}"; then
    sed -i "s/^# ${key} is not set$/${key}=y/" "${CFG}"
  elif grep -q "^${key}=" "${CFG}"; then
    sed -i "s/^${key}=.*/${key}=y/" "${CFG}"
  else
    echo "${key}=y" >>"${CFG}"
  fi
  echo "enabled ${key}"
}

enable BR2_PACKAGE_QT6BASE_NETWORK
enable BR2_PACKAGE_HOST_NODEJS
enable BR2_PACKAGE_LIBNSS
enable BR2_PACKAGE_LIBXML2
enable BR2_PACKAGE_LIBXSLT
enable BR2_PACKAGE_JPEG
enable BR2_PACKAGE_QT6SHADERTOOLS
enable BR2_PACKAGE_QT6DECLARATIVE
enable BR2_PACKAGE_QT6WEBCHANNEL
enable BR2_PACKAGE_QT6WEBENGINE

# Force rebuild of webengine after adding Quick deps (previous build skipped Chromium).
rm -rf "${OUT}/build/qt6webengine-6.4.3" \
  "${OUT}/build/qt6shadertools-6.4.3" \
  "${OUT}/build/qt6declarative-6.4.3" \
  "${OUT}/build/host-qt6shadertools-6.4.3" \
  "${OUT}/build/host-qt6declarative-6.4.3"

# If host Gui was just enabled, rebuild host-qt6base (required for qsb).
if grep -q 'MOONCODING_HOST_QT6BASE_GUI' "${BR}/package/qt6/qt6base/qt6base.mk"; then
  if [[ ! -f "${OUT}/host/lib/libQt6Gui.so" ]] && [[ ! -f "${OUT}/host/lib/libQt6Gui.so.6" ]]; then
    echo "Rebuilding host-qt6base with Gui for qsb..."
    rm -rf "${OUT}/build/host-qt6base-6.4.3"
  fi
fi

make -C "${BR}" O="${OUT}" olddefconfig

echo "=== webengine-related config ==="
grep -E 'QT6WEBENGINE|QT6WEBCHANNEL|QT6DECLARATIVE|QT6SHADERTOOLS|QT6BASE_NETWORK|HOST_NODEJS|LIBNSS' "${CFG}" | head -40

echo "INSTALL_OK"
echo "Next: ${ROOT}/scripts/buildroot/build-qt6webengine.sh"
