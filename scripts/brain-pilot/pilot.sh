#!/bin/bash
# Brain pilot 2026-10-08 (brief: brain-pilot.md; design/training-plan-2026-10-08.md 2a, 3):
# frondgrazer + littershredder on today's v2 contract, train-flat-nopred, run on eidolon.
#
#   setsid nohup bash pilot.sh > pilot.log 2>&1 < /dev/null &
#
# Run from the pilot directory R, which holds bin/cubarium-search, config/ (scenarios and
# desktop maps), policies/ (the C1 centres), scripts/noise.py. One chained script:
#   1. p1-noise: sigma 0, C1 centres, 2 generations, 8 worlds, 8 pairs, 10-min windows,
#      curiosity weight 0 -> noise.py readout + curiosity-2026-10-08.toml.
#   2. p2-train: warm start C1, sigma 0.02, lr 0.005, 8 pairs, 8 worlds, 10-min windows,
#      recalibrated curiosity, stomach-aware off, 4 h wall, checkpoint every generation.
#   3. gates (whole-lineage deployment, voxel_census through the kit's batch.py, 1 h):
#      midway (after 2 h of training, while it continues): trained-mid vs c1 vs heuristic,
#      train-flat-nopred seeds 1-8; end: trained-final on the same seeds, and trained-final
#      vs c1 on the d72 saved starts (seeds 1-8, minute-63 checkpoints).
# Writes DONE at the end, FAILED with the reason if a part fails.
set -u
R=$(cd "$(dirname "$0")" && pwd)
cd "$R"
ulimit -v $((64 * 1024 * 1024))   # 64 GiB address space, KiB
KIT=$HOME/cubarium-train/kit
CENSUS=$KIT/voxel_census.691d3f8e
BIN=$R/bin/cubarium-search
MAP=$R/config/scenarios/train-flat-nopred.toml
C1F=$R/policies/frondgrazer-curio-c1-2026-10-02.json
C1L=$R/policies/littershredder-curio-c1-2026-10-02.json
D72="$HOME/cubarium-train/starts-2026-10-08/runs/d72-s[1-8]/ckpt/m000063"
TRAIN_WALL=${TRAIN_WALL:-14400}
MID_AFTER=${MID_AFTER:-7200}

fail() { echo "$(date -Is) FAILED: $*"; echo "$*" > "$R/FAILED"; exit 1; }
log() { echo "$(date -Is) $*"; }
rm -f "$R/DONE" "$R/FAILED"

common=(--world "$MAP" --ecology "$MAP" --centre "frondgrazer=$C1F" --centre "littershredder=$C1L"
        --pairs 8 --worlds 8 --threads 8 --window-min 10 --save-every 1
        --first-generation-seconds 2400)

# ---- 1. noise first ----
if [ ! -f p1-noise/DONE ]; then
  log "part 1: sigma 0 control"
  printf '[frondgrazer]\nweight = 0.0\n[littershredder]\nweight = 0.0\n[lanternjaw]\nweight = 0.0\n' > curiosity-zero.toml
  nice -n 10 "$BIN" voxel-live-train "${common[@]}" --sigma 0 --generations 2 --wall-seconds 5400 \
    --curiosity curiosity-zero.toml --out p1-noise > p1-noise.log 2> p1-noise.err
  code=$?
  [ -f p1-noise/DONE ] || [ -f p1-noise/PAUSED ] || fail "part 1 ended without DONE (exit $code); see p1-noise.err, p1-noise/FAILED"
  [ -s p1-noise/generations.jsonl ] || fail "part 1 logged no generation"
fi
python3 scripts/noise.py p1-noise --curiosity-out curiosity-2026-10-08.toml > p1-noise.tsv 2> p1-noise.analysis.err \
  || fail "part 1 analysis: $(tail -1 p1-noise.analysis.err)"
log "part 1 done; curiosity: $(grep -h '^weight' curiosity-2026-10-08.toml | tr '\n' ' ')"

# ---- 2. train (background), with the midway gate beside it ----
log "part 2: training, wall ${TRAIN_WALL}s"
if [ -f p2-train/checkpoint.json ] && [ ! -f p2-train/DONE ] && [ ! -f p2-train/PAUSED ]; then resume=(--resume p2-train); else resume=(); fi
nice -n 10 "$BIN" voxel-live-train "${common[@]}" --sigma 0.02 --learning-rate 0.005 --generations 1000 \
  --wall-seconds "$TRAIN_WALL" --curiosity curiosity-2026-10-08.toml --out p2-train "${resume[@]}" \
  >> p2-train.log 2>> p2-train.err &
TRAIN=$!
echo $TRAIN > p2-train.pid

gate_flat() { # out arms...
  local out=$1; shift
  python3 "$KIT/scripts/studies/batch.py" "$out" --bin "$CENSUS" "$@" --config "$MAP" --seeds 1-8 \
    --hours 1 --threads 1 --max-parallel "${GATE_PAR:-6}" --slot-base 8 --stage "check_min=3 segment_min=15 keep=2"
}
score() { # out baseline
  python3 "$KIT/scripts/studies/score.py" "$1" --baseline "$2" > "$1/score.txt" 2>&1
  python3 "$KIT/scripts/studies/vitals_summary.py" "$1"/*/s*/run > "$1/vitals.txt" 2>&1
  true
}

# Midway: sleep until 2 h of training (the detached pilot may wait; workers may not).
sleep "$MID_AFTER"
if kill -0 $TRAIN 2>/dev/null; then
  mkdir -p gate-mid/centres
  cp p2-train/centres/latest-frondgrazer.json p2-train/centres/latest-littershredder.json gate-mid/centres/ \
    || fail "midway: no latest centres"
  log "midway gate from generation $(grep -o '"generation":[0-9]*' gate-mid/centres/latest-frondgrazer.json)"
  M=$R/gate-mid/centres
  gate_flat "$R/gate-mid/flat" \
    --arm "trained=policy=frondgrazer=$M/latest-frondgrazer.json policy=littershredder=$M/latest-littershredder.json still=lanternjaw" \
    --arm "c1=still=lanternjaw" --arm "heuristic=heuristic still=lanternjaw" > gate-mid.log 2>&1 \
    || log "midway gate batch exited nonzero (see gate-mid.log)"
  score "$R/gate-mid/flat" c1
else
  log "training ended before the midway mark; midway gate skipped"
fi

wait $TRAIN
code=$?
# The wall budget ends training PAUSED (final centres written); DONE only if 1000 generations ran.
[ -f p2-train/FAILED ] && fail "part 2 FAILED (exit $code): $(cat p2-train/FAILED)"
[ -f p2-train/DONE ] || [ -f p2-train/PAUSED ] || fail "part 2 ended without DONE/PAUSED (exit $code); see p2-train.err"
python3 scripts/noise.py p2-train > p2-train.tsv 2>&1
log "part 2 done: $(tail -1 p2-train.log)"

# ---- 3. end gate ----
F=$R/p2-train/centres
[ -f "$F/final-frondgrazer.json" ] && [ -f "$F/final-littershredder.json" ] || fail "no final centres in $F"
trained="policy=frondgrazer=$F/final-frondgrazer.json policy=littershredder=$F/final-littershredder.json still=lanternjaw"
# c1 and heuristic on these seeds ran in the midway gate; run them here only if it didn't.
if [ -f gate-mid/flat/DONE ]; then extra=(); else extra=(--arm "c1=still=lanternjaw" --arm "heuristic=heuristic still=lanternjaw"); fi
GATE_PAR=6 gate_flat "$R/gate-end/flat" --arm "trained=$trained" "${extra[@]}" > gate-end-flat.log 2>&1 &
FLAT=$!
python3 "$KIT/scripts/studies/batch.py" "$R/gate-end/d72" --bin "$CENSUS" --arm "trained=$trained" --arm "c1=still=lanternjaw" \
  --starts "$D72" --hours 1 --threads 1 --max-parallel 8 --slot-base 14 \
  --stage "check_min=3 segment_min=15 keep=2" > gate-end-d72.log 2>&1 || log "d72 gate batch exited nonzero"
wait $FLAT || log "end flat gate batch exited nonzero"
score "$R/gate-end/d72" c1
if [ ${#extra[@]} -gt 0 ]; then score "$R/gate-end/flat" c1; else
  python3 "$KIT/scripts/studies/score.py" "$R/gate-end/flat" > "$R/gate-end/flat/score.txt" 2>&1
  python3 "$KIT/scripts/studies/vitals_summary.py" "$R"/gate-end/flat/*/s*/run > "$R/gate-end/flat/vitals.txt" 2>&1; fi
log "pilot done"
echo "done $(date -Is)" > "$R/DONE"
