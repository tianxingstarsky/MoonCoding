#!/bin/sh
# Product WiFi bring-up / heal for Luckfox AIC8800.
#
# Observed failure mode (chat / HTTPS load):
#   wpa_state=COMPLETED + IP + default route, but gateway ARP stuck / 100% loss.
#   Soft fix that works: wpa_cli reassociate (keep lease). Do NOT require
#   public ICMP — only probe the local gateway.

log() { echo "[board-net] $*"; }

wpa_state() {
	wpa_cli -i wlan0 status 2>/dev/null | sed -n 's/^wpa_state=//p'
}

wpa_alive() {
	wpa_cli -i wlan0 status >/dev/null 2>&1
}

wlan_has_ip() {
	ip -4 addr show wlan0 2>/dev/null | grep -q 'inet '
}

wlan_has_default() {
	ip -4 route show default dev wlan0 2>/dev/null | grep -q .
}

wlan_gateway() {
	ip -4 route show default dev wlan0 2>/dev/null | awk '/default/ {print $3; exit}'
}

# Association + address + default via wlan0 (control plane).
wlan_l3_ok() {
	[ "$(wpa_state)" = "COMPLETED" ] || return 1
	wlan_has_ip || return 1
	wlan_has_default || return 1
	return 0
}

# Data plane: can we reach the gateway?
# CRITICAL: do NOT trust ARP flags — AIC8800 can show 0x2 while 100% loss.
# Two pings: a single success can race with a dying link (watchdog false "already online").
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

write_resolv() {
	{
		echo "nameserver 8.8.8.8"
		echo "nameserver 1.1.1.1"
		if [ -n "$1" ] && [ "$1" != "8.8.8.8" ] && [ "$1" != "1.1.1.1" ]; then
			echo "nameserver $1"
		fi
	} >/etc/resolv.conf
}

disable_wifi_powersave() {
	if command -v iw >/dev/null 2>&1; then
		iw dev wlan0 set power_save off >/dev/null 2>&1 || true
	fi
	if command -v iwconfig >/dev/null 2>&1; then
		iwconfig wlan0 power off >/dev/null 2>&1 || true
	fi
	if [ -w /sys/class/net/wlan0/device/power/control ]; then
		echo on >/sys/class/net/wlan0/device/power/control 2>/dev/null || true
	fi
}

# Soft fix for zombie L3 (assoc OK, packets dead). Keeps IP when possible.
heal_zombie_datapath() {
	gw=$(wlan_gateway)
	log "zombie datapath (gw=${gw:-?}) — reassociate"
	disable_wifi_powersave
	# Drop stuck/wrong neighbors (permanent bad ARP was observed to brick TX).
	ip neigh flush dev wlan0 2>/dev/null || true
	if [ -n "$gw" ]; then
		ip neigh del "$gw" dev wlan0 2>/dev/null || true
	fi
	wpa_cli -i wlan0 reassociate >/dev/null 2>&1 || \
		wpa_cli -i wlan0 reconnect >/dev/null 2>&1 || true
	i=0
	while [ $i -lt 10 ]; do
		sleep 1
		if [ "$(wpa_state)" = "COMPLETED" ] && wlan_datapath_ok; then
			log "datapath ok after reassociate"
			return 0
		fi
		i=$((i + 1))
	done

	log "reassociate insufficient — disconnect/reconnect"
	ip neigh flush dev wlan0 2>/dev/null || true
	wpa_cli -i wlan0 disconnect >/dev/null 2>&1 || true
	sleep 1
	wpa_cli -i wlan0 reconnect >/dev/null 2>&1 || true
	i=0
	while [ $i -lt 12 ]; do
		sleep 1
		if [ "$(wpa_state)" = "COMPLETED" ] && wlan_datapath_ok; then
			log "datapath ok after disconnect/reconnect"
			return 0
		fi
		i=$((i + 1))
	done
	log "datapath still down after soft heals"
	return 1
}

wait_for_wlan() {
	i=0
	while [ $i -lt 40 ]; do
		[ -e /sys/class/net/wlan0 ] && return 0
		sleep 0.5
		i=$((i + 1))
	done
	return 1
}

# One-time soft attempt to load driver with PS off (boot / missing iface only).
ensure_modules() {
	[ -e /sys/class/net/wlan0 ] && return 0
	if [ -x /usr/bin/wifibt-init.sh ]; then
		log "wlan0 missing; running wifibt-init.sh"
		sh /usr/bin/wifibt-init.sh >/dev/null 2>&1 || true
		sleep 2
	fi
	[ -e /sys/class/net/wlan0 ] && return 0
	if [ -f /lib/modules/aic8800_fdrv.ko ]; then
		log "reloading aic8800 (ps_on=N)"
		killall -q wpa_supplicant 2>/dev/null || true
		rmmod aic8800_fdrv 2>/dev/null || true
		rmmod aic_load_fw 2>/dev/null || true
		insmod /lib/modules/aic_load_fw.ko 2>/dev/null || true
		insmod /lib/modules/aic8800_fdrv.ko ps_on=N 2>/dev/null \
			|| insmod /lib/modules/aic8800_fdrv.ko 2>/dev/null || true
		sleep 2
		[ -x /usr/bin/wifibt-init.sh ] && sh /usr/bin/wifibt-init.sh >/dev/null 2>&1 || true
		sleep 1
	fi
	[ -e /sys/class/net/wlan0 ]
}

ensure_wpa() {
	ip link set wlan0 up 2>/dev/null || true
	mkdir -p /var/run/wpa_supplicant 2>/dev/null || true

	state=$(wpa_state)
	if wpa_alive && [ "$state" = "COMPLETED" ]; then
		return 0
	fi

	# If ctrl iface works but not associated, gentle reconnect — no kill.
	if wpa_alive && [ "$state" != "COMPLETED" ]; then
		log "wpa_state=${state:-?} — reconnect (keep process)"
		wpa_cli -i wlan0 reconnect >/dev/null 2>&1 || true
		i=0
		while [ $i -lt 15 ]; do
			[ "$(wpa_state)" = "COMPLETED" ] && return 0
			sleep 1
			i=$((i + 1))
		done
	fi

	log "wpa_supplicant restart"
	killall -q wpa_supplicant 2>/dev/null || true
	sleep 1
	rm -rf /var/run/wpa_supplicant 2>/dev/null || true
	mkdir -p /var/run/wpa_supplicant 2>/dev/null || true
	[ -f /etc/wpa_supplicant.conf ] || {
		log "WARNING: no /etc/wpa_supplicant.conf"
		return 1
	}
	wpa_supplicant -B -i wlan0 -c /etc/wpa_supplicant.conf >/dev/null 2>&1 || true
	sleep 2
	wpa_cli -i wlan0 reconfigure >/dev/null 2>&1 || true
	wpa_cli -i wlan0 reconnect >/dev/null 2>&1 || true
	i=0
	while [ $i -lt 20 ]; do
		[ "$(wpa_state)" = "COMPLETED" ] && {
			log "wpa_state=COMPLETED"
			return 0
		}
		sleep 1
		i=$((i + 1))
	done
	log "wpa_state=$(wpa_state) after wait"
	[ "$(wpa_state)" = "COMPLETED" ]
}

ensure_dhcp_daemon() {
	if ps 2>/dev/null | grep -q '[u]dhcpc -i wlan0'; then
		return 0
	fi
	# Background renew only — never flush existing address here.
	udhcpc -i wlan0 -b >/dev/null 2>&1 || true
}

ensure_address() {
	# Already have usable product path — do nothing disruptive.
	if wlan_product_ok; then
		ensure_dhcp_daemon
		return 0
	fi

	# Assoc + route but dead TX/RX — soft reassociate first.
	if wlan_l3_ok && ! wlan_datapath_ok; then
		heal_zombie_datapath && return 0
	fi

	# Associated but missing IP/route: request DHCP without flushing first.
	if [ "$(wpa_state)" = "COMPLETED" ]; then
		log "COMPLETED but missing IP/route — DHCP renew"
		udhcpc -i wlan0 -n -q -t 6 -T 2 -A 1 >/dev/null 2>&1 || true
		ensure_dhcp_daemon
		wlan_product_ok && return 0
	fi

	# Still broken: one gentle reconnect, then DHCP (still no flush unless needed).
	log "L3 down — one reconnect + DHCP"
	wpa_cli -i wlan0 reconnect >/dev/null 2>&1 || true
	sleep 2
	udhcpc -i wlan0 -n -q -t 8 -T 3 -A 1 >/dev/null 2>&1 || true
	ensure_dhcp_daemon
	wlan_product_ok && return 0

	# Last resort only: flush stale address then rediscover.
	if wlan_has_ip && ! wlan_has_default; then
		log "stale addr without route — flush + DHCP"
		ip -4 addr flush dev wlan0 2>/dev/null || true
		udhcpc -i wlan0 -n -q -t 8 -T 3 -A 1 >/dev/null 2>&1 || true
		ensure_dhcp_daemon
	fi
	wlan_product_ok
}

sync_clock() {
	year=$(date +%Y 2>/dev/null || echo 1970)
	[ "$year" -lt 2024 ] 2>/dev/null || {
		log "clock ok: $(date -u '+%Y-%m-%d %H:%M:%S UTC')"
		return 0
	}
	# Only sync when datapath is up — never block heal on clock.
	wlan_product_ok || {
		log "clock wrong; skip sync (no datapath)"
		return 1
	}
	log "clock looks wrong ($(date -u)); syncing"
	if command -v rdate >/dev/null 2>&1; then
		for host in 216.239.35.0 time.nist.gov; do
			if rdate -s "$host" >/dev/null 2>&1; then
				log "rdate ok via $host -> $(date -u '+%Y-%m-%d %H:%M:%S UTC')"
				return 0
			fi
		done
	fi
	log "rdate failed; clock still $(date -u)"
	return 1
}

# ── main ──
if ! wait_for_wlan; then
	log "wlan0 not present yet; trying module bring-up"
	ensure_modules || true
	wait_for_wlan || {
		log "WARNING: wlan0 missing"
		exit 0
	}
fi

disable_wifi_powersave

# Optional: FORCE_HEAL=1 skips the fast "already online" path (used by watchdog).
if [ "${FORCE_HEAL:-0}" != "1" ] && wlan_product_ok; then
	# Confirm once more — single flap caused "already online" while watchdog saw death.
	sleep 1
	if wlan_datapath_ok; then
		ensure_dhcp_daemon
		write_resolv "$(wlan_gateway)"
		log "already online (wlan0 ip=$(ip -4 addr show wlan0 | awk '/inet /{print $2; exit}'))"
		sync_clock
		exit 0
	fi
	log "datapath flapped — continuing heal"
fi

# Control plane looks fine but packets don't flow — most common chat failure.
if wlan_l3_ok && ! wlan_datapath_ok; then
	heal_zombie_datapath && {
		ensure_dhcp_daemon
		write_resolv "$(wlan_gateway)"
		log "online after zombie heal gw=$(wlan_gateway)"
		sync_clock
		exit 0
	}
fi

ensure_wpa || log "WARNING: wpa not COMPLETED"
disable_wifi_powersave
ensure_address || log "WARNING: datapath still down"

if wlan_product_ok; then
	write_resolv "$(wlan_gateway)"
	log "online gw=$(wlan_gateway)"
else
	write_resolv ""
	log "WARNING: WiFi not ready"
fi

sync_clock
exit 0
