#!/bin/bash
# Rebuild QtWebEngineProcess main.cpp.o without -D_TIME_BITS=64 so sandbox
# localtime_* overrides match Chromium's 32-bit time_t (long).
set -euo pipefail
OUT="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox"
WE="${OUT}/build/qt6webengine-6.4.3/buildroot-build"
MAIN_O="src/process/CMakeFiles/QtWebEngineProcess.dir/main.cpp.o"
NM="${OUT}/host/bin/arm-buildroot-linux-gnueabihf-nm"
CXXFILT="${OUT}/host/bin/arm-buildroot-linux-gnueabihf-c++filt"
NINJA="${OUT}/host/bin/ninja"

test -d "${WE}"
cd "${WE}"

"${NINJA}" -t commands "${MAIN_O}" | tail -n 1 > /tmp/we-main-raw-cmd.txt
test -s /tmp/we-main-raw-cmd.txt

python3 - <<'PY' > /tmp/we-main-fixed-cmd.sh
import shlex
from pathlib import Path
raw = Path('/tmp/we-main-raw-cmd.txt').read_text().strip()
args = shlex.split(raw)
out = []
for a in args:
    if a == '-D_TIME_BITS=64' or a.startswith('-D_TIME_BITS='):
        continue
    out.append(a)
if out:
    out.insert(1, '-U_TIME_BITS')
print('#!/bin/bash')
print('set -euo pipefail')
print('cd /home/mooncoding/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/build/qt6webengine-6.4.3/buildroot-build')
print('exec ' + ' '.join(shlex.quote(a) for a in out))
PY
chmod +x /tmp/we-main-fixed-cmd.sh
echo "recompiling main.cpp.o without TIME_BITS=64 ..."
bash /tmp/we-main-fixed-cmd.sh
test -f "${WE}/${MAIN_O}"

echo '==== undefined localtime mangling ===='
"${NM}" -u "${WE}/${MAIN_O}" | grep localtime | "${CXXFILT}" || true
if "${NM}" -u "${WE}/${MAIN_O}" | grep -q 'localtime_overrideEPKx'; then
  echo "ERROR: still wants long long (EPKx)"
  exit 1
fi
if ! "${NM}" -u "${WE}/${MAIN_O}" | grep -q 'localtime_overrideEPKl'; then
  echo "WARN: did not find EPKl undef; full dump:"
  "${NM}" -u "${WE}/${MAIN_O}" | grep -i time | "${CXXFILT}" || true
fi
echo "PROCESS_TIME_ABI_OK"

# Prevent ninja from rebuilding process objs with TIME_BITS=64 again.
python3 - <<'PY'
from pathlib import Path
import re
p = Path('/home/mooncoding/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/build/qt6webengine-6.4.3/buildroot-build/build.ninja')
text = p.read_text()
# Only touch QtWebEngineProcess build edges / their FLAGS lines nearby.
out = []
lines = text.splitlines(keepends=True)
i = 0
changed = 0
while i < len(lines):
    line = lines[i]
    if 'QtWebEngineProcess' in line and (line.startswith('build ') or 'FLAGS =' in line):
        # rewrite this line and a small following window of variable assignments
        block = [line]
        j = i + 1
        while j < len(lines) and (lines[j].startswith('  ') or lines[j].startswith('\t')):
            block.append(lines[j])
            j += 1
        block_txt = ''.join(block)
        new_block = block_txt.replace('-D_TIME_BITS=64 ', '').replace('-D_TIME_BITS=64', '')
        if new_block != block_txt:
            changed += 1
        out.append(new_block)
        i = j
        continue
    out.append(line)
    i += 1
p.write_text(''.join(out))
print(f'stripped TIME_BITS from {changed} QtWebEngineProcess ninja blocks')
PY
