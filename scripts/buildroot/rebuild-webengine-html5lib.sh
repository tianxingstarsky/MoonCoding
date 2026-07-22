#!/bin/bash
set -euo pipefail
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:${HOME}/.cargo/bin"
OUT="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox"
BR="${HOME}/Lyra-sdk/buildroot"

# System html5lib for whatever python3 cmake finds first.
if ! python3 -c 'import html5lib' 2>/dev/null; then
  if command -v apt-get >/dev/null; then
    sudo apt-get update -qq
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq python3-html5lib python3-pip || true
  fi
fi
if ! python3 -c 'import html5lib' 2>/dev/null; then
  python3 -m ensurepip --user || true
  python3 -m pip install --user html5lib || true
fi
python3 -c 'import html5lib; print("html5lib OK", html5lib.__version__)'

cp -a /mnt/e/newvibecode/scripts/buildroot/qt6/qt6webengine/. \
  "${BR}/package/qt6/qt6webengine/"

CFG="${OUT}/.config"
if grep -q 'HOST_PYTHON_HTML5LIB' "${CFG}" 2>/dev/null; then
  if grep -q '^# BR2_PACKAGE_HOST_PYTHON_HTML5LIB is not set$' "${CFG}"; then
    sed -i 's/^# BR2_PACKAGE_HOST_PYTHON_HTML5LIB is not set$/BR2_PACKAGE_HOST_PYTHON_HTML5LIB=y/' "${CFG}"
  elif ! grep -q '^BR2_PACKAGE_HOST_PYTHON_HTML5LIB=y$' "${CFG}"; then
    echo 'BR2_PACKAGE_HOST_PYTHON_HTML5LIB=y' >>"${CFG}"
  fi
fi

rm -rf "${OUT}/build/qt6webengine-6.4.3"
exec bash /mnt/e/newvibecode/scripts/buildroot/build-qt6webengine.sh
