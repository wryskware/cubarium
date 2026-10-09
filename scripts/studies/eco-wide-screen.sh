#!/usr/bin/env bash
# Protocol A's screen for the ecology search (training plan 2026-10-08 §6 item 22, build
# order step 4): every candidate on 8 wide worlds x 2 h from the year-old (hour-10) saves of
# the wide year baseline, profile wide-screen-2h, genome config/training/search/eco.toml.
#
# BIN and STARTS select the binary and checkpoint glob (environment or positional
# arguments: eco-wide-screen.sh [BIN [STARTS]]). Remake the starts with a binary that
# includes mass rescaling. Old wideyear-2026-10-09 saves are fauna schema 73 and newer
# binaries refuse them. Preflight loads every save with BIN; schema errors always stop.
#
# Sizing (desktop, scripts/run-study.py --list: 2 L3 groups x 16 logical CPUs, 16 physical
# cores): batch.py runs one world per physical core (measured best 2026-10-08), so 16 at a
# time. lambda 7 + base = 8 arms x 8 seeds = 64 runs = 4 full waves per generation; a wave is
# 2 sim-h of a year-old wide world, about 1.3-1.7 h wall at 1 thread (1.86x real time at
# hour 4, slower as the world matures), so about 6-7 h per generation. 4 generations fit a
# 26 h wall budget: a first desktop round, not a converged search (9 genes; EC2 for big
# rounds on Wrysk's go). Every run goes through run-study.py --slot under chrt -i 0
# nice -n 19 (batch.py). Check the desktop is otherwise idle before launching.
set -euo pipefail

REPO=$(cd "$(dirname "$0")/../.." && pwd)
BIN=${1:-${BIN:-$REPO/target/release/examples/voxel_census}}
STARTS=${2:-${STARTS:-$REPO/runs/wideyear-2026-10-09/heuristic/s*/run/ckpt/m000600}}
OUT=${OUT:-$REPO/runs/eco-screen-$(date +%Y-%m-%d)}

if [[ $# -gt 2 || ! -x "$BIN" ]]; then
    echo "usage: eco-wide-screen.sh [BIN [STARTS]]; BIN must be executable" >&2
    exit 1
fi
mapfile -t starts < <(python3 - "$STARTS" <<'PY'
import glob
import sys
for path in sorted(glob.glob(sys.argv[1])):
    print(path)
PY
)
n=${#starts[@]}
if [[ "$n" != 8 ]]; then
    echo "eco-wide-screen: STARTS matched $n of 8 required year-old checkpoints" >&2
    exit 1
fi
# expand= loads and validates the saves before exiting, without stepping a world.
# Use the study wrapper so any threads inherit one shared L3 group.
check=$(mktemp)
trap 'rm -f "$check"' EXIT
for i in "${!starts[@]}"; do
    if ! timeout 270 python3 "$REPO/scripts/run-study.py" --slot "$i" -- \
        "$BIN" 0 "load=${starts[$i]}" threads=1 \
        "genome=$REPO/config/training/search/eco.toml" "expand=$check" > /dev/null; then
        echo "eco-wide-screen: BIN cannot load ${starts[$i]}; binary/save schemas must match" >&2
        exit 1
    fi
done
rm -f "$check"
trap - EXIT

exec chrt -i 0 nice -n 19 python3 "$REPO/scripts/studies/eco_search.py" "$OUT" \
    --genome "$REPO/config/training/search/eco.toml" \
    --fitness "$REPO/config/training/fitness-v3.toml" --profile wide-screen-2h \
    --bin "$BIN" \
    --starts "$STARTS" \
    --required "$REPO/config/training/required-wide.toml" \
    --lambda 7 --seeds-per-gen 8 --seed-pool 1-8 \
    --generations 4 --wall-hours 26 --max-parallel 16 --threads 1 \
    --sigma0 0.15 --rng-seed 1 --detach
