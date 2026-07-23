#!/bin/sh
# Disable AIC8800 802.11 power-save at boot (ps_on=N).
# Runtime sysfs write is rejected; must reload the module once early.

MARKER=/var/run/mooncoding-aic-ps-off
[ -f "$MARKER" ] && exit 0

mod=/sys/module/aic8800_fdrv/parameters/ps_on
if [ -e "$mod" ] && [ "$(cat "$mod" 2>/dev/null)" = "N" ]; then
	touch "$MARKER"
	exit 0
fi

# Only reload when wlan exists or modules are present — keep brief.
if [ ! -f /lib/modules/aic8800_fdrv.ko ]; then
	exit 0
fi

echo "[aic-ps] reloading aic8800_fdrv with ps_on=N"
killall -q wpa_supplicant 2>/dev/null || true
# Give USB a moment after boot.
sleep 1
rmmod aic8800_fdrv 2>/dev/null || true
rmmod aic_load_fw 2>/dev/null || true
insmod /lib/modules/aic_load_fw.ko 2>/dev/null || true
if ! insmod /lib/modules/aic8800_fdrv.ko ps_on=N 2>/dev/null; then
	insmod /lib/modules/aic8800_fdrv.ko 2>/dev/null || true
fi
sleep 2
if [ -x /usr/bin/wifibt-init.sh ]; then
	sh /usr/bin/wifibt-init.sh >/dev/null 2>&1 || true
fi
# Restore association.
if [ -f /etc/wpa_supplicant.conf ]; then
	mkdir -p /var/run/wpa_supplicant
	wpa_supplicant -B -i wlan0 -c /etc/wpa_supplicant.conf >/dev/null 2>&1 || true
	sleep 2
	wpa_cli -i wlan0 reconnect >/dev/null 2>&1 || true
fi
touch "$MARKER"
echo "[aic-ps] ps_on=$(cat /sys/module/aic8800_fdrv/parameters/ps_on 2>/dev/null)"
exit 0
