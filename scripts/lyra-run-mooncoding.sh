#!/bin/sh
# Portrait full-screen MoonCoding on Luckfox Lyra (native 720x1280 DSI).
# No software rotation — touch maps 1:1 to widgets via linuxfb + evdevtouch.
cd /root/mooncoding || exit 1

export LD_LIBRARY_PATH=/root/mooncoding:/root/mooncoding/lib:$LD_LIBRARY_PATH
export QT_PLUGIN_PATH=/root/mooncoding/plugins
export MOONCODING_BOARD=1
# Goodix capacitive panel. Keep keyboard off event0 (device advertises KEY bits).
export QT_QPA_EVDEV_TOUCHSCREEN_PARAMETERS="${QT_QPA_EVDEV_TOUCHSCREEN_PARAMETERS:-/dev/input/event0}"
export QT_QPA_EVDEV_KEYBOARD_PARAMETERS="${QT_QPA_EVDEV_KEYBOARD_PARAMETERS:-/dev/input/event1}"
export QT_QPA_EVDEV_MOUSE_PARAMETERS="${QT_QPA_EVDEV_MOUSE_PARAMETERS:-}"
export FONTCONFIG_PATH=/root/mooncoding/fonts
export FONTCONFIG_FILE=/root/mooncoding/fonts/fonts.conf
export QT_QPA_FONTDIR=/root/mooncoding/fonts
export LANG=C.UTF-8
export LC_ALL=C.UTF-8
export QT_LOGGING_RULES="${QT_LOGGING_RULES:-*.debug=false}"
export QT_FORCE_STDERR_LOGGING=1

# Qt WebEngine (optional — present after qt6webengine deploy). Soft-fail if missing.
export QTWEBENGINE_DISABLE_SANDBOX="${QTWEBENGINE_DISABLE_SANDBOX:-1}"
export QT_QUICK_BACKEND="${QT_QUICK_BACKEND:-software}"
if [ -z "${QTWEBENGINE_CHROMIUM_FLAGS:-}" ]; then
  export QTWEBENGINE_CHROMIUM_FLAGS="--disable-gpu --disable-gpu-compositing --no-sandbox --disable-dev-shm-usage"
fi
if [ -x /root/mooncoding/libexec/QtWebEngineProcess ]; then
  export QTWEBENGINEPROCESS_PATH=/root/mooncoding/libexec/QtWebEngineProcess
fi
if [ -d /root/mooncoding/resources ]; then
  export QTWEBENGINE_RESOURCES_PATH=/root/mooncoding/resources
fi
if [ -f /root/mooncoding/translations/qtwebengine_locales/en-US.pak ]; then
  export QTWEBENGINE_LOCALES_PATH=/root/mooncoding/translations/qtwebengine_locales
elif [ -f /root/mooncoding/resources/en-US.pak ]; then
  # Fallback when locale paks were staged into resources/ by older deploys.
  export QTWEBENGINE_LOCALES_PATH=/root/mooncoding/resources
fi

# Board rootfs often ships without CA store; python HTTPS (model list) needs this.
if [ -f /root/mooncoding/certs/cacert.pem ]; then
  export SSL_CERT_FILE=/root/mooncoding/certs/cacert.pem
  export REQUESTS_CA_BUNDLE=/root/mooncoding/certs/cacert.pem
  export CURL_CA_BUNDLE=/root/mooncoding/certs/cacert.pem
fi

PROJECTS_ROOT="${MOONCODING_PROJECTS_ROOT:-/root/Documents/MoonCodingProjects}"
mkdir -p "$PROJECTS_ROOT" /root/mooncoding/fonts /tmp/fontconfig-cache

# Resolve an isolated per-project workspace. NEVER force the legacy shared
# /root/mooncoding-ws folder — that caused cross-project file bleed.
CONF="${XDG_CONFIG_HOME:-/root/.config}/MoonCoding/MoonCoding.conf"
WS=""
if [ -f "$CONF" ]; then
  # Qt QSettings ini: lastWorkspace=/path
  WS=$(sed -n 's/^lastWorkspace=//p' "$CONF" 2>/dev/null | head -1 | tr -d '\r')
fi
case "$WS" in
"$PROJECTS_ROOT"|"$PROJECTS_ROOT"/*)
  [ -d "$WS" ] || WS=""
  ;;
*)
  WS=""
  ;;
esac

if [ -z "$WS" ]; then
  # Newest project directory under projects root
  WS=$(ls -1dt "$PROJECTS_ROOT"/*/ 2>/dev/null | head -1 | sed 's:/*$::')
fi

if [ -z "$WS" ] || [ ! -d "$WS" ]; then
  WS="$PROJECTS_ROOT/default"
  mkdir -p "$WS"
  if [ ! -f "$WS/index.html" ]; then
    cat >"$WS/index.html" <<'HTML'
<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>MoonCoding</title>
<style>
body{margin:0;font-family:sans-serif;background:#f6f4f1;color:#1a1a1a}
main{min-height:100vh;padding:24px 16px;box-sizing:border-box}
h1{font-size:28px;margin:0 0 12px}
p{font-size:16px;line-height:1.5}
</style>
</head>
<body>
<main>
  <h1>新项目</h1>
  <p>这是独立工作区。在聊天里让 AI 从 index.html 开始搭建竖屏应用。</p>
</main>
</body>
</html>
HTML
  fi
fi

# Keep kernel fb rotate at 0 — app uses native portrait geometry.
if [ -w /sys/class/graphics/fb0/rotate ]; then
  echo 0 >/sys/class/graphics/fb0/rotate 2>/dev/null || true
fi

exec ./mooncoding \
  -platform linuxfb:fb=/dev/fb0 \
  --workspace "$WS"
