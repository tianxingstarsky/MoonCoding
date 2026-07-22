#!/bin/bash
# Fix time_t vs long long ABI mismatch for ProfileAdapter::determineDownloadPath.
set -euo pipefail
OUT="${HOME}/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox"
WE="${OUT}/build/qt6webengine-6.4.3"
H="${WE}/src/core/profile_adapter.h"
C="${WE}/src/core/profile_adapter.cpp"
D="${WE}/src/core/download_manager_delegate_qt.cpp"

test -f "$H" && test -f "$C" && test -f "$D"

# Idempotent replacements
python3 - <<'PY'
from pathlib import Path

def repl(path, old, new):
    p = Path(path)
    text = p.read_text(encoding='utf-8')
    if new in text and old not in text:
        print(f'already patched: {path}')
        return
    if old not in text:
        raise SystemExit(f'pattern not found in {path}: {old!r}')
    p.write_text(text.replace(old, new, 1), encoding='utf-8')
    print(f'patched: {path}')

h = '/home/mooncoding/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/build/qt6webengine-6.4.3/src/core/profile_adapter.h'
c = '/home/mooncoding/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/build/qt6webengine-6.4.3/src/core/profile_adapter.cpp'
d = '/home/mooncoding/Lyra-sdk/buildroot/output/rockchip_rk3506_luckfox/build/qt6webengine-6.4.3/src/core/download_manager_delegate_qt.cpp'

repl(h,
     'QString determineDownloadPath(const QString &downloadDirectory, const QString &suggestedFilename, const time_t &startTime);',
     'QString determineDownloadPath(const QString &downloadDirectory, const QString &suggestedFilename, const qint64 &startTime);')
repl(c,
     'QString ProfileAdapter::determineDownloadPath(const QString &downloadDirectory, const QString &suggestedFilename, const time_t &startTime)',
     'QString ProfileAdapter::determineDownloadPath(const QString &downloadDirectory, const QString &suggestedFilename, const qint64 &startTime)')

# Call site: cast ToTimeT() to qint64 — match whatever formatting exists
dt = Path(d).read_text(encoding='utf-8')
old1 = 'm_profileAdapter->determineDownloadPath(defaultDownloadDirectory.absolutePath(), suggestedFilename, item->GetStartTime().ToTimeT())'
new1 = 'm_profileAdapter->determineDownloadPath(defaultDownloadDirectory.absolutePath(), suggestedFilename, static_cast<qint64>(item->GetStartTime().ToTimeT()))'
if new1 in dt:
    print(f'already patched: {d}')
elif old1 in dt:
    Path(d).write_text(dt.replace(old1, new1, 1), encoding='utf-8')
    print(f'patched: {d}')
else:
    # try looser search
    import re
    m = re.search(r'determineDownloadPath\([^;]+ToTimeT\(\)\)', dt)
    if not m:
        raise SystemExit(f'call site not found in {d}')
    print('found call:', m.group(0)[:120])
    raise SystemExit('unexpected call site formatting — inspect manually')
PY

echo "TIME_T_ABI_PATCH_OK"
