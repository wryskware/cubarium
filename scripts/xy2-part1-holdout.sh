#!/bin/bash
# Workstream XY2, Part 1: export each arm's selected centre and evaluate it on the held-out
# layouts, exactly as X did. The generations are the selection rule's, computed from the
# training results alone and frozen before any held-out episode runs.
set -eu
cd "$(dirname "$0")/.."
BIN=./bin/cubarium-search-xy2
CFG=/home/wrysk/wryskware/cubarium/runs/ecology-v1-calibration/selected/fast-leaf.toml

run() {
  local out="$1" adapter="$2" gen="$3"
  local g
  g=$(printf '%05d' "$gen")
  "$BIN" es-export \
    --checkpoint "${out}/checkpoint.json" --config "$CFG" \
    --generation "$gen" --out "${out}/selected/center-${g}-policy.json"
  "$BIN" es-evaluate \
    --policy "${out}/selected/center-${g}-policy.json" \
    --set holdout --horizon 36000 --wall-seconds 300 \
    --config "$CFG" --adapter "$adapter" --out "${out}/holdout.json"
}

run runs/es-eco-v1-fastleaf-s2 cub-act-1 "$1"
run runs/es-eco-v1-fastleaf-act2-s2 cub-act-2 "$2"
echo "=== held out done"
