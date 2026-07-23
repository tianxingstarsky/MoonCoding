#!/bin/sh
# Restart MoonCoding UI if it exits (board product resilience).
BIN=/root/mooncoding/mooncoding
RUN=/root/mooncoding/run-mooncoding.sh
LOG=/tmp/mooncoding.log
PIDFILE=/var/run/mooncoding-ui-babysit.pid

if [ -f "$PIDFILE" ]; then
	old=$(cat "$PIDFILE" 2>/dev/null)
	if [ -n "$old" ] && kill -0 "$old" 2>/dev/null; then
		exit 0
	fi
fi
echo $$ >"$PIDFILE"

while true; do
	sleep 12
	if pidof mooncoding >/dev/null 2>&1; then
		continue
	fi
	echo "$(date -u '+%H:%M:%S') [babysit] mooncoding missing — restart" >>"$LOG"
	[ -f "$RUN" ] || continue
	chmod +x "$BIN" "$RUN" 2>/dev/null || true
	nohup sh "$RUN" >>"$LOG" 2>&1 &
	sleep 3
done
