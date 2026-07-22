#!/bin/bash
# Qt API / process objects are built with -D_TIME_BITS=64, but Chromium sandbox
# in libQt6WebEngineCore was built with 32-bit time_t. Rebuild process main
# without _TIME_BITS=64 so localtime_* overrides match (long const*).
set -euo pipefail
OUT="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox"
WE="${OUT}/build/qt6webengine-6.4.3/buildroot-build"
MAIN_O="${WE}/src/process/CMakeFiles/QtWebEngineProcess.dir/main.cpp.o"
MAIN_CXX="${OUT}/build/qt6webengine-6.4.3/src/process/main.cpp"
NINJA="${WE}/build.ninja"
CXX="${OUT}/host/bin/arm-buildroot-linux-gnueabihf-g++"
SYSROOT="${OUT}/host/arm-buildroot-linux-gnueabihf/sysroot"
NM="${OUT}/host/bin/arm-buildroot-linux-gnueabihf-nm"
CXXFILT="${OUT}/host/bin/arm-buildroot-linux-gnueabihf-c++filt"

test -f "${MAIN_CXX}"
test -f "${NINJA}"

# Extract the compile rule for main.cpp.o from build.ninja (first matching COMMAND)
CMD="$(python3 - <<'PY'
from pathlib import Path
ninja = Path('/home/mooncoding/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/build/qt6webengine-6.4.3/buildroot-build/build.ninja').read_text(errors='replace')
key = 'src/process/CMakeFiles/QtWebEngineProcess.dir/main.cpp.o'
# find build edge then COMMAND
idx = ninja.find(f'build {key}')
if idx < 0:
    raise SystemExit('build edge not found')
chunk = ninja[idx:idx+8000]
# COMMAND may be on following lines with $:
lines = chunk.splitlines()
cmd = None
for i,l in enumerate(lines):
    if 'COMMAND =' in l or l.strip().startswith('COMMAND'):
        # ninja might use rsp; collect until blank or next var
        parts = [l.split('COMMAND =',1)[-1].strip()]
        for j in range(i+1, min(i+20, len(lines))):
            if lines[j].startswith(' ') or lines[j].startswith('\t') or lines[j].startswith('$'):
                parts.append(lines[j].lstrip(' $\t'))
            elif '=' in lines[j] and not lines[j].startswith(' '):
                break
            else:
                break
        cmd = ' '.join(parts)
        break
if not cmd:
    # fallback: DESCRIPTION style — search compile_commands
    cc = Path('/home/mooncoding/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/build/qt6webengine-6.4.3/buildroot-build/compile_commands.json')
    if cc.exists():
        import json
        for e in json.loads(cc.read_text()):
            if e.get('file','').endswith('src/process/main.cpp') or e.get('output','').endswith('main.cpp.o'):
                print(e.get('command') or ' '.join(e.get('arguments',[])))
                raise SystemExit
    raise SystemExit('COMMAND not found')
print(cmd)
PY
)"

if [[ -z "${CMD}" ]]; then
  echo "ERROR: could not recover compile command for main.cpp.o"
  exit 1
fi

# Strip TIME_BITS=64 and force classic 32-bit time_t for this TU
CMD_FIXED="$(python3 - <<PY
import shlex, os
cmd = '''${CMD}'''
# fragile if cmd has quotes — use compile_commands preferentially next
print(cmd)
PY
)"

# Prefer compile_commands.json
CCJSON="${WE}/compile_commands.json"
if [[ -f "${CCJSON}" ]]; then
  python3 - <<'PY' > /tmp/we-main-compile.sh
import json, shlex
from pathlib import Path
cc = json.loads(Path('/home/mooncoding/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/build/qt6webengine-6.4.3/buildroot-build/compile_commands.json').read_text())
entry = None
for e in cc:
    f = e.get('file','')
    if f.endswith('/src/process/main.cpp') or f.endswith('\\src\\process\\main.cpp'):
        entry = e
        break
if not entry:
    raise SystemExit('main.cpp not in compile_commands.json')
if 'arguments' in entry:
    args = entry['arguments']
else:
    args = shlex.split(entry['command'])
# remove -D_TIME_BITS=64 ; add -U_TIME_BITS to be sure
out = []
for a in args:
    if a == '-D_TIME_BITS=64' or a.startswith('-D_TIME_BITS='):
        continue
    out.append(a)
# insert after compiler
out.insert(1, '-U_TIME_BITS')
# ensure output dir
print('#!/bin/bash')
print('set -euo pipefail')
print('cd /home/mooncoding/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/build/qt6webengine-6.4.3/buildroot-build')
print('exec ' + ' '.join(shlex.quote(a) for a in out))
PY
  chmod +x /tmp/we-main-compile.sh
  bash /tmp/we-main-compile.sh
else
  echo "ERROR: compile_commands.json missing"
  exit 1
fi

echo '==== verify mangling ===='
"${NM}" -u "${MAIN_O}" | grep localtime | "${CXXFILT}" || true
if "${NM}" -u "${MAIN_O}" | grep -q 'localtime_overrideEPKx'; then
  echo "ERROR: still wants long long"
  exit 1
fi
if ! "${NM}" -u "${MAIN_O}" | grep -q 'localtime_overrideEPKl'; then
  echo "WARN: unexpected localtime undef; dumping:"
  "${NM}" -u "${MAIN_O}" | grep localtime | "${CXXFILT}" || true
fi
echo "PROCESS_TIME_ABI_OK"
