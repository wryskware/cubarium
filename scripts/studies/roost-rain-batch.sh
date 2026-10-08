#!/usr/bin/env bash
# ROOSTS-RAIN evidence batch (design/handoffs/roosts-and-rain-2026-10-08.md): 4 seeds x 2 arms
# x 60 sim-minutes of roost_rain_study on one L3 slot, then the two MP4 captures, then the
# table and a DONE marker. Launch detached from the repo root:
#   setsid nohup scripts/studies/roost-rain-batch.sh OUT BIN_DIR SLOT > OUT/batch.log 2>&1 &
set -u
OUT=$1
BIN=$2
SLOT=$3
mkdir -p "$OUT"
rm -f "$OUT/DONE"
pids=()
for arm in before after; do
  for seed in 41 42 43 44; do
    d="$OUT/$arm-seed-$seed"
    mkdir -p "$d"
    chrt -i 0 nice -n 19 scripts/run-study.py --slot "$SLOT" -- \
      "$BIN/roost_rain_study" config/desktop/terrarium.toml "$seed" 60 "$d" arm=$arm threads=1 water_every=4 \
      > "$d/stdout.log" 2> "$d/stderr.log" &
    pids+=($!)
  done
done
# The captures (headless GPU, one sim thread each) beside the study.
chrt -i 0 nice -n 19 scripts/run-study.py --slot "$SLOT" -- \
  "$BIN/roost_rain_capture" "$OUT" seedporter --seed 41 --start-hour 19.0 --max-s 60 \
  > "$OUT/capture-seedporter.log" 2>&1 &
pids+=($!)
chrt -i 0 nice -n 19 scripts/run-study.py --slot "$SLOT" -- \
  "$BIN/roost_rain_capture" "$OUT" bellwing --seed 42 --start-hour 9.0 --max-s 25 \
  > "$OUT/capture-bellwing.log" 2>&1 &
pids+=($!)
for p in "${pids[@]}"; do wait "$p"; done
python3 scripts/studies/roost_rain_summary.py "$OUT" > "$OUT/summary.log" 2>&1
date > "$OUT/DONE"
