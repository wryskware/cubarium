#!/bin/bash
# The gate on the desktop terrarium at the shipped mass: the trained centres vs round 2's
# (transferred) vs the heuristic, seeds 1-8 x 60 min. Usage: [BIN=path] terrarium-gate.sh
#   <trained-browser.json> <trained-shredder.json> [threads-per-arm]
cd /home/wrysk/wryskware/cubarium/.claude/worktrees/live-train
C=config/training/centres
T=${3:-5}
export BIN
bash scripts/live-train/terrarium-check.sh runs/gate30/trained $T x30=default $1 $2 &
bash scripts/live-train/terrarium-check.sh runs/gate30/r2 $T x30=default \
  transfer:$C/r2-frondgrazer-upd160.json transfer:$C/r2-littershredder-upd512.json &
bash scripts/live-train/terrarium-check.sh runs/gate30/heuristic $T x30=default heuristic heuristic &
wait
