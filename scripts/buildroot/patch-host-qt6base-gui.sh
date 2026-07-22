#!/bin/bash
# Enable FEATURE_gui on host-qt6base so host-qt6shadertools can build qsb.
# Luckfox Buildroot defaults host Qt to Core-only tools.
set -euo pipefail
SDK="${LYRA_SDK_ROOT:-${HOME}/Lyra-sdk}"
MK="${SDK}/buildroot/package/qt6/qt6base/qt6base.mk"
export MK
if [[ ! -f "$MK" ]]; then
  echo "missing $MK"
  exit 1
fi
if grep -q 'MOONCODING_HOST_QT6BASE_GUI' "$MK"; then
  echo "host gui patch already applied"
  exit 0
fi
python3 - <<'PY'
from pathlib import Path
import os
mk = Path(os.environ['MK'])
text = mk.read_text()
old = """HOST_QT6BASE_DEPENDENCIES = \\
\thost-double-conversion \\
\thost-libb2 \\
\thost-pcre2 \\
\thost-zlib
HOST_QT6BASE_CONF_OPTS = \\
\t-DFEATURE_gui=OFF \\
\t-DFEATURE_concurrent=OFF \\
\t-DFEATURE_xml=ON \\
\t-DFEATURE_sql=OFF \\
\t-DFEATURE_testlib=OFF \\
\t-DFEATURE_network=OFF \\
\t-DFEATURE_dbus=OFF \\
\t-DFEATURE_icu=OFF \\
\t-DFEATURE_glib=OFF \\
\t-DFEATURE_system_doubleconversion=ON \\
\t-DFEATURE_system_libb2=ON \\
\t-DFEATURE_system_pcre2=ON \\
\t-DFEATURE_system_zlib=ON
"""
new = """# MOONCODING_HOST_QT6BASE_GUI: needed for host qsb / QtWebEngine toolchain
HOST_QT6BASE_DEPENDENCIES = \\
\thost-double-conversion \\
\thost-libb2 \\
\thost-pcre2 \\
\thost-zlib \\
\thost-freetype \\
\thost-libpng \\
\thost-libjpeg
HOST_QT6BASE_CONF_OPTS = \\
\t-DFEATURE_gui=ON \\
\t-DFEATURE_widgets=OFF \\
\t-DFEATURE_opengl=OFF \\
\t-DFEATURE_concurrent=OFF \\
\t-DFEATURE_xml=ON \\
\t-DFEATURE_sql=OFF \\
\t-DFEATURE_testlib=OFF \\
\t-DFEATURE_network=OFF \\
\t-DFEATURE_dbus=OFF \\
\t-DFEATURE_icu=OFF \\
\t-DFEATURE_glib=OFF \\
\t-DFEATURE_system_doubleconversion=ON \\
\t-DFEATURE_system_libb2=ON \\
\t-DFEATURE_system_pcre2=ON \\
\t-DFEATURE_system_zlib=ON \\
\t-DFEATURE_system_freetype=ON \\
\t-DFEATURE_system_png=ON \\
\t-DFEATURE_system_jpeg=ON
"""
if old not in text:
    raise SystemExit('HOST_QT6BASE block not found for patching')
mk.write_text(text.replace(old, new, 1))
print('patched', mk)
PY
