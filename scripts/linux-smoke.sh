#!/usr/bin/env bash
# Linux smoke for MoonCoding (WSL / native). Builds into build-linux on E:, not C:.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD="${MOONCODING_BUILD_DIR:-$ROOT/build-linux}"
JOBS="${MOONCODING_JOBS:-$(nproc)}"

cd "$ROOT"
# shellcheck disable=SC1090
source "$HOME/.cargo/env" 2>/dev/null || true

echo "==> cmake configure ($BUILD)"
cmake -S "$ROOT" -B "$BUILD" \
  -DCMAKE_BUILD_TYPE=Release \
  -DMOONCODING_BUILD_TESTS=ON \
  -DMOONCODING_LINUX_USE_SYSTEM_QT=ON

echo "==> build"
cmake --build "$BUILD" --target vibe_protocol vibe_agent_backend mooncoding mooncoding_ui_tests -j"$JOBS"

echo "==> vibe smoke"
# vibe exits non-zero when invoked without a subcommand; still prints usage.
vibe_help="$("$BUILD/vibe-target/release/vibe" 2>&1 || true)"
printf '%s\n' "$vibe_help" | head -n 1 | grep -qi 'vibe'

echo "==> agent library"
test -f "$BUILD/rust-target/release/libvibe_agent.so"

echo "==> ui binary + sidecars"
test -x "$BUILD/vibe-ui/mooncoding"
test -f "$BUILD/vibe-ui/libvibe_agent.so"
test -x "$BUILD/vibe-ui/vibe"
test -f "$BUILD/vibe-ui/translations/mooncoding_en.qm"

echo "==> project path helpers (Documents)"
DOCS="${XDG_DOCUMENTS_DIR:-$HOME/Documents}"
PROJ_ROOT="$DOCS/MoonCodingProjects"
mkdir -p "$PROJ_ROOT"
SMOKE="$PROJ_ROOT/.mooncoding-linux-smoke"
rm -rf "$SMOKE"
mkdir -p "$SMOKE"
test -d "$SMOKE"
rm -rf "$SMOKE"

if command -v xvfb-run >/dev/null 2>&1; then
  echo "==> ui tests (xvfb)"
  xvfb-run -a "$BUILD/vibe-ui/mooncoding_ui_tests"
else
  echo "==> ui tests skipped (install xvfb for headless Qt tests)"
fi

echo "OK: Linux smoke passed ($BUILD)"
