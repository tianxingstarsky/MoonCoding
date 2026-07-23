#!/bin/sh
killall mooncoding 2>/dev/null || true
sleep 1
chmod +x /root/mooncoding/mooncoding
rm -rf /root/.config/MoonCoding /root/.config/mooncoding
nohup sh /root/mooncoding/run-mooncoding.sh > /tmp/mooncoding.log 2>&1 &
sleep 3
ps | grep '[m]ooncoding' || true
echo '---LOG---'
head -30 /tmp/mooncoding.log
echo '---FB---'
cat /sys/class/graphics/fb0/rotate 2>/dev/null || true
cat /sys/class/graphics/fb0/virtual_size 2>/dev/null || true
