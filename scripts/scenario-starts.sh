#!/usr/bin/env bash
# Saved starts per scenario map (WORKING_POLICY "Staged runs and scenario maps"):
# found every map on a few seeds, run each to a settled minute as a staged run (the
# liveness check after the first segment, checkpoints every segment), and keep the
# last checkpoint as the start studies branch from (`load=OUT/<map>/seed-N/start`).
#
#   scripts/scenario-starts.sh BIN OUT [MINUTES] [SEEDS] [MAPS]
#
# BIN is a release `voxel_census`; OUT gets OUT/<map>/seed-N/ (the run directory:
# status, FAILED/DONE, ckpt/) with `start` linking its last checkpoint, plus
# OUT/<map>/seed-N.log, OUT/summary.tsv (map, seed, state, founding wall seconds,
# whole wall seconds, mean ms/tick over the run at THREADS threads, default 2) and
# OUT/DONE once every run has ended.
# Defaults: 60 settled minutes, seeds "1 2 3", every map in config/scenarios.
# Run from the repository root (the maps' `base` paths are relative to them).
#
# Starts are disposable: a schema bump means regenerate, never migrate. Founders run the
# seeder's heuristics (`heuristic`): a policy-driven save is refused by the heuristic
# studies' loader.
set -u
BIN=${1:?usage: scenario-starts.sh BIN OUT [MINUTES] [SEEDS] [MAPS]}
OUT=${2:?usage: scenario-starts.sh BIN OUT [MINUTES] [SEEDS] [MAPS]}
MINUTES=${3:-60}
SEEDS=${4:-"1 2 3"}
MAPS=${5:-"patch terrarium range dry-hills wet-valley"}
THREADS=${THREADS:-2}
mkdir -p "$OUT"
HOURS=$(awk "BEGIN { print $MINUTES / 60 }")
printf 'map\tseed\tstate\tfound_s\trun_s\tms_tick\n' > "$OUT/summary.tsv"

one() {
    local map=$1 seed=$2
    local dir="$OUT/$map/seed-$seed" log="$OUT/$map/seed-$seed.log"
    mkdir -p "$OUT/$map"
    rm -rf "$dir"
    local t0 t1
    t0=$(date +%s.%N)
    nice -n 10 "$BIN" "$HOURS" "config=config/scenarios/$map.toml" "seed=$seed" heuristic "threads=$THREADS" \
        "run=$dir" check_min=3 segment_min=10 keep=2 > "$OUT/$map/seed-$seed.csv" 2> "$log"
    t1=$(date +%s.%N)
    local state=unknown
    [ -f "$dir/DONE" ] && state=done
    [ -f "$dir/FAILED" ] && state=failed
    # The founding's wall time: from start to the staged run's first line.
    local found_s run_s ms
    found_s=$(awk -F'[ =]' '/^founded in /{print $3}' "$log" | head -1)
    run_s=$(awk "BEGIN { print $t1 - $t0 }")
    ms=$(grep -o 'tick_ms=[0-9.]*' "$log" | awk -F= '{ n++; s += $2 } END { if (n) printf "%.2f", s / n }')
    local last
    last=$(ls -d "$dir"/ckpt/m[0-9]* 2>/dev/null | grep -v partial | sort | tail -1)
    [ -n "$last" ] && ln -sfn "ckpt/$(basename "$last")" "$dir/start"
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$map" "$seed" "$state" "${found_s:-?}" "$run_s" "${ms:-?}" >> "$OUT/summary.tsv"
}

for map in $MAPS; do
    for seed in $SEEDS; do
        one "$map" "$seed" &
    done
done
wait
date > "$OUT/DONE"
