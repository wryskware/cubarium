#!/bin/sh
# What the Tachyon is doing right now: both services, their last 20 lines, the
# panel connector, and the top of `top`. Read-only; safe at any time.
#
#     ./scripts/tachyon-status.sh
#     TACHYON=root@192.168.68.68 ./scripts/tachyon-status.sh
set -eu

TACHYON=${TACHYON:-root@192.168.68.68}

ssh "$TACHYON" 'set -eu
echo "=== date ==="
date -u +%Y-%m-%dT%H:%M:%SZ

for unit in cube-screen-shim cubarium; do
    echo
    echo "=== $unit.service ==="
    systemctl show "$unit.service" \
        -p ActiveState -p SubState -p NRestarts -p MainPID -p ExecMainStartTimestamp \
        -p MemoryCurrent 2>/dev/null
    echo "--- last 20 lines ---"
    journalctl -u "$unit.service" -n 20 --no-pager -o short-iso 2>/dev/null
done

echo
echo "=== panel (DP-1) ==="
for f in status enabled modes; do
    printf "%-8s %s\n" "$f:" "$(tr "\n" " " < /sys/class/drm/card0-DP-1/$f 2>/dev/null)"
done
ls -l /run/cube-screen-shim/frames.sock 2>/dev/null || echo "no frame socket"

echo
echo "=== top ==="
top -b -n 1 | head -15
'
