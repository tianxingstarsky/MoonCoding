#!/bin/sh
# Create unversioned / SONAME symlinks beside versioned shared libs.
cd /root/mooncoding/lib || exit 1
for f in *.so.*; do
  [ -f "$f" ] || continue
  # skip if already a symlink
  [ -L "$f" ] && continue
  base=$(echo "$f" | sed -E 's/\.so\..*/.so/')
  major=$(echo "$f" | sed -E 's/(\.so\.[0-9]+).*/\1/')
  ln -sf "$f" "$base"
  ln -sf "$f" "$major"
done
ls -la | head -80
