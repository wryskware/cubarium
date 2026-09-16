#!/bin/bash
# Workstream XY2, Part 1: X's second training seed with its own control.
# The retained command verbatim, --train-seed 20260916, once per adapter.
set -eu
cd "$(dirname "$0")/.."
BIN=./bin/cubarium-search-xy2
CFG=/home/wrysk/wryskware/cubarium/runs/ecology-v1-calibration/selected/fast-leaf.toml

for arm in cub-act-1:runs/es-eco-v1-fastleaf-s2 cub-act-2:runs/es-eco-v1-fastleaf-act2-s2; do
  adapter="${arm%%:*}"
  out="${arm##*:}"
  echo "=== ${adapter} -> ${out}"
  "$BIN" es-train \
    --config "$CFG" \
    --pairs 16 --generations 16 --horizon 36000 --workers 8 \
    --wall-seconds 1200 --train-seed 20260916 --center-eval true \
    --adapter "$adapter" --out "$out" 2>&1 | tail -40
done
echo "=== part 1 training done"
