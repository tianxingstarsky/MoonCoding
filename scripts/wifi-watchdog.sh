#!/bin/sh
# Product WiFi watchdog — heal when association OR datapath dies.
# Datapath probe = local gateway only (not public ICMP).

NET_SCRIPT="${NET_SCRIPT:-/root/mooncoding/board-net-ready.sh}"
INTERVAL="${WIFI_WATCHDOG_INTERVAL:-15}"
LOG="${WIFI_WATCHDOG_LOG:-/tmp/wifi-watchdog.log}"

log() { echo "$(date -u '+%H:%M:%S') [wifi-wd] $*" >>"$LOG"; }

wlan_l3_ok() {
	[ -e /sys/class/net/wlan0 ] || return 1
	wpa_cli -i wlan0 status 2>/dev/null | grep -q '^wpa_state=COMPLETED' || return 1
	ip -4 addr show wlan0 2>/dev/null | grep -q 'inet ' || return 1
	ip -4 route show default dev wlan0 2>/dev/null | grep -q . || return 1
	return 0
}

wlan_gateway() {
	ip -4 route show default dev wlan0 2>/dev/null | awk '/default/ {print $3; exit}'
}

# Never trust ARP alone — measured 0x2 + 100% loss on AIC8800.
wlan_datapath_ok() {
	gw=$(wlan_gateway)
	[ -n "$gw" ] || return 1
	ping -c 1 -W 2 "$gw" >/dev/null 2>&1 || return 1
	ping -c 1 -W 2 "$gw" >/dev/null 2>&1
}

wlan_product_ok() {
	wlan_l3_ok || return 1
	wlan_datapath_ok || return 1
	return 0
}

pidfile=/var/run/mooncoding-wifi-watchdog.pid
if [ -f "$pidfile" ]; then
	old=$(cat "$pidfile" 2>/dev/null)
	if [ -n "$old" ] && kill -0 "$old" 2>/dev/null; then
		exit 0
	fi
fi
echo $$ >"$pidfile"

: >"$LOG"
log "started interval=${INTERVAL}s (ping-gw)"
fail_streak=0
heal_cooldown=0

while true; do
	sleep "$INTERVAL"
	if [ "$heal_cooldown" -gt 0 ]; then
		heal_cooldown=$((heal_cooldown - 1))
	fi

	if wlan_product_ok; then
		fail_streak=0
		continue
	fi

	fail_streak=$((fail_streak + 1))
	st=$(wpa_cli -i wlan0 status 2>/dev/null | sed -n 's/^wpa_state=//p')
	if wlan_l3_ok; then
		log "zombie datapath streak=$fail_streak state=$st"
	else
		log "l3-down streak=$fail_streak state=$st"
	fi
	# One confirmed ping miss (~15s) is enough — ARP lies on this chip.
	[ "$fail_streak" -lt 1 ] && continue
	[ "$heal_cooldown" -gt 0 ] && continue

	log "heal begin"
	if [ -f "$NET_SCRIPT" ]; then
		FORCE_HEAL=1 sh "$NET_SCRIPT" >>"$LOG" 2>&1 || true
	fi
	heal_cooldown=1
	if wlan_product_ok; then
		log "heal ok"
		fail_streak=0
	else
		log "heal still down"
	fi
done
