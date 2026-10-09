#!/usr/bin/env bash
# Protocol A's screen for the ecology search (training plan 2026-10-08 §6 item 22, build
# order step 4): every candidate on 8 wide worlds x 2 h from the year-old (hour-10) saves of
# the wide year baseline, profile wide-screen-2h, genome config/training/search/eco.toml.
#
# Waits for: runs/wideyear-2026-10-09 to finish (10 h, keep 11; the hour-10 saves are each
# seed's final checkpoint, heuristic/s*/run/ckpt/m000600). The script refuses until all 8
# exist.
#
# BLOCKED until two decisions are made (ECO-GENOME report, 2026-10-09):
# 1. Mass genes on a loaded world kill at once. `load=` with a genome replaces the config
#    and leaves every body's stocks as saved (Fauna::replace_config), so a lineage whose
#    body_min rises past its bodies' saved mass starves in the first tick. Measured on the
#    baseline's hour-4 save (s1 m000243), one minute: frondgrazer 109 -> 0, Veilhound 8 -> 0,
#    lanternjaw 12 -> 0, loftstrider 11 -> 2, capgnawer 37 -> 15, bellwing 164 -> 70,
#    shredder 38 -> 20, seedporter 16 -> 9; the base arm lost none.
# 2. The saves are fauna schema 73 (binary 6b75f538, before VINE-C); main is schema 74 and
#    refuses them, so this screen runs the baseline's own binary, without VINE-C.
# FORCE=1 runs it anyway (for a deliberate check of exactly this).
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
MAIN=/home/wrysk/wryskware/cubarium
BASELINE=$MAIN/runs/wideyear-2026-10-09
BIN=$MAIN/runs/bins/voxel_census.6b75f538   # the binary that wrote the saves (schema 73)
OUT=${OUT:-$MAIN/runs/eco-screen-$(date +%Y-%m-%d)}

if [[ "${FORCE:-0}" != 1 ]]; then
    echo "eco-wide-screen: blocked (mass genes on loaded worlds; save schema). See this script's header." >&2
    exit 2
fi
n=$(ls -d "$BASELINE"/heuristic/s*/run/ckpt/m000600 2>/dev/null | wc -l)
if [[ "$n" != 8 ]]; then
    echo "eco-wide-screen: $n of 8 hour-10 saves exist; the wide year baseline is not done" >&2
    exit 1
fi

exec chrt -i 0 nice -n 19 python3 "$REPO/scripts/studies/eco_search.py" "$OUT" \
    --genome "$REPO/config/training/search/eco.toml" \
    --fitness "$REPO/config/training/fitness-v3.toml" --profile wide-screen-2h \
    --bin "$BIN" \
    --starts "$BASELINE/heuristic/s*/run/ckpt/m000600" \
    --required "$REPO/config/training/required-wide.toml" \
    --lambda 7 --seeds-per-gen 8 --seed-pool 1-8 \
    --generations 4 --wall-hours 26 --max-parallel 16 --threads 1 \
    --sigma0 0.15 --rng-seed 1 --detach
