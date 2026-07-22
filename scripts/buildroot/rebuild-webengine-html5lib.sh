#!/bin/bash
set -euo pipefail
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:${HOME}/.cargo/bin"
OUT="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox"
BR="${HOME}/Lyra-sdk/buildroot"

python3 -m pip install --user html5lib || pip3 install --user html5lib
HP="${OUT}/host/bin/python3"
if [[ -x "$HP" ]]; then
  "$HP" -m pip install html5lib || true
fi
python3 -c 'import html5lib; print("html5lib", html5lib.__version__)'

cp -a /mnt/e/newvibecode/scripts/buildroot/qt6/qt6webengine/. \
  "${BR}/package/qt6/qt6webengine/"

# Prefer Buildroot host-python-html5lib when present
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
