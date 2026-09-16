#!/bin/bash
# Workstream XY2, Part 1, Astra's addendum: the 2x2 of weights x adapter.
# Each seed's two selected centres, replayed on the same four training layouts under BOTH
# adapters with the intake and dwell trace, weights untouched. The stability half is not run:
# it reads one retained pair reduction that exists for one run and one generation.
set -eu
cd "$(dirname "$0")/.."
BIN=./bin/cubarium-search-xy2
CFG=/home/wrysk/wryskware/cubarium/runs/ecology-v1-calibration/selected/fast-leaf.toml
R=/home/wrysk/wryskware/cubarium/runs
OUT=runs/ecology-v1-round5-followups/replay

replay() {
  local label="$1" run="$2" gen="$3"
  echo "=== ${label}: ${run} generation ${gen}"
  "$BIN" es-turn-band --run "$run" --config "$CFG" --generation "$gen" \
    --horizon 36000 --workers 8 --wall-seconds 900 \
    --out "${OUT}/${label}" 2>&1 | tail -25
}

replay s1-act1 "$R/es-eco-v1-fastleaf"           9
replay s1-act2 "$R/es-eco-v1-fastleaf-act2"      11
replay s2-act1 runs/es-eco-v1-fastleaf-s2        13
replay s2-act2 runs/es-eco-v1-fastleaf-act2-s2   15
echo "=== 2x2 done"
