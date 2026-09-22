#!/bin/sh
# Throw the panel's world away and let it found a new one. Runs **on the development
# host**; the board keeps its binary, its art and its config.
#
#     ./scripts/tachyon-reseed.sh                 # a new world from a drawn seed
#     ./scripts/tachyon-reseed.sh --seed 12345    # a new world from that seed, and keep it
#     ./scripts/tachyon-reseed.sh --no-seed       # stop pinning a seed, draw again
#     TACHYON=root@192.168.68.68 ./scripts/tachyon-reseed.sh
#
# Wrysk, 2026-09-21: the saved worlds are not precious — this is how you see another
# variant of the terrain without redeploying.
#
# It deletes **only** `world-*.cubw` (and `world-*.voxel`, which the `cubarium voxel`
# harness writes if anyone has run that on the board). The journal, the art, the config
# and everything else in /var/lib/cubarium are left alone.
set -eu

TACHYON=${TACHYON:-root@192.168.68.68}
STATE=${STATE:-/var/lib/cubarium/state}
UNIT=cubarium.service
SEED=
DROPIN=/etc/systemd/system/cubarium.service.d/10-seed.conf

case "${1:-}" in
    --seed) SEED=${2:?--seed needs a number} ;;
    --no-seed) SEED=none ;;
    "") ;;
    *) echo "usage: $0 [--seed N | --no-seed]" >&2; exit 2 ;;
esac

echo "==> stop $UNIT"
# SIGINT, so the stop is a clean checkpoint rather than a torn one — and then the
# checkpoint it just wrote is deleted with the rest, which is the point.
ssh "$TACHYON" "systemctl stop $UNIT"

if [ -n "$SEED" ]; then
    # `Environment=` in a drop-in **replaces** the variable rather than appending to it,
    # and the unit already carries `--gpu-art-scale 2` in CUBARIUM_EXTRA_ARGS. So the
    # current effective value is read back from the unit and re-emitted with the seed on
    # the end; the base stays wherever it was, and `--no-seed` removes the file so the
    # unit's own value stands again.
    if [ "$SEED" = none ]; then
        echo "==> drop the seed pin"
        ssh "$TACHYON" "rm -f $DROPIN && systemctl daemon-reload"
    else
        echo "==> pin --seed $SEED"
        ssh "$TACHYON" "set -eu
            base=\$(systemctl show $UNIT -p Environment --value |
                   tr ' ' '\n' | sed -n 's/^CUBARIUM_EXTRA_ARGS=//p')
            # Anything already pinned is replaced, not stacked.
            base=\$(printf '%s' \"\$base\" | sed 's/--seed [0-9]*//g')
            mkdir -p \$(dirname $DROPIN)
            printf '[Service]\nEnvironment=\"CUBARIUM_EXTRA_ARGS=%s --seed %s\"\n' \\
                \"\$base\" '$SEED' > $DROPIN
            systemctl daemon-reload
            cat $DROPIN"
    fi
fi

echo "==> delete the saved world in $STATE"
ssh "$TACHYON" "set -eu
    ls -1 $STATE/world-*.cubw $STATE/world-*.voxel 2>/dev/null | wc -l |
        xargs -I{} echo 'removing {} snapshot(s)'
    rm -f $STATE/world-*.cubw $STATE/world-*.voxel"

echo "==> start $UNIT"
ssh "$TACHYON" "systemctl start $UNIT"

echo "==> waiting 20 s"
sleep 20

echo "==> last 30 journal lines"
ssh "$TACHYON" "journalctl -u $UNIT --no-pager -n 30"
