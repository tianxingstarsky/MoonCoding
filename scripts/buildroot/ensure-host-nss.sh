#!/bin/bash
# Host (x86_64) NSS/NSPR are required to link Chromium *host* tools while
# cross-building Qt WebEngine (target NSS in sysroot is ARM — incompatible).
set -euo pipefail
need=0
for lib in libnss3.so libnspr4.so; do
  if ! ls /usr/lib/x86_64-linux-gnu/"${lib}" >/dev/null 2>&1 \
    && ! ls /usr/lib/"${lib}" >/dev/null 2>&1; then
    need=1
  fi
done
if [[ "${need}" -eq 1 ]]; then
  echo "installing host libnss3-dev libnspr4-dev ..."
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq libnss3-dev libnspr4-dev
fi
test -e /usr/lib/x86_64-linux-gnu/libnss3.so -o -e /usr/lib/libnss3.so
test -e /usr/lib/x86_64-linux-gnu/libnspr4.so -o -e /usr/lib/libnspr4.so
echo "HOST_NSS_OK $(pkg-config --modversion nss 2>/dev/null || echo present)"
