#!/bin/bash
# Install MoonCoding qt6webchannel + qt6webengine packages into Luckfox Buildroot
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

mkdir -p "${PKG}/qt6webchannel" "${PKG}/qt6webengine"
cp -a "${SRC}/qt6webchannel/." "${PKG}/qt6webchannel/"
cp -a "${SRC}/qt6webengine/." "${PKG}/qt6webengine/"

# Wire into package/qt6/Config.in if not already sourced.
if ! grep -q 'qt6webchannel/Config.in' "${PKG}/Config.in"; then
  python3 - <<'PY'
from pathlib import Path
import os
cfg = Path(os.environ["PKG"]) / "Config.in"
text = cfg.read_text()
needle = 'source "package/qt6/qt6svg/Config.in"'
extra = needle + '\n' + 'source "package/qt6/qt6webchannel/Config.in"\n' + 'source "package/qt6/qt6webengine/Config.in"'
if 'qt6webchannel/Config.in' not in text:
    if needle not in text:
        raise SystemExit("cannot find qt6svg source line in Config.in")
    text = text.replace(needle, extra, 1)
    cfg.write_text(text)
    print("patched package/qt6/Config.in")
else:
    print("Config.in already references qt6webchannel")
PY
fi

if [[ ! -f "${CFG}" ]]; then
  echo "ERROR: missing ${CFG} — run Buildroot configure first"
  exit 1
fi

# Enable required options (idempotent).
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
enable BR2_PACKAGE_QT6WEBCHANNEL
enable BR2_PACKAGE_QT6WEBENGINE

# Olddefconfig to resolve new selects / deps.
make -C "${BR}" O="${OUT}" olddefconfig

echo "=== webengine-related config ==="
grep -E 'QT6WEBENGINE|QT6WEBCHANNEL|QT6BASE_NETWORK|HOST_NODEJS|LIBNSS' "${CFG}" | head -40

echo "INSTALL_OK"
echo "Next: make -C ${BR} O=${OUT} qt6webengine"
echo "  (or: ${ROOT}/scripts/buildroot/build-qt6webengine.sh)"
