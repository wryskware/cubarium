#!/bin/bash
# Step 1: heuristic vs round-2 browsers in the new-defaults and tuned worlds.
cd /home/wrysk/wryskware/cubarium/.claude/worktrees/live-train
C=config/training/centres
T=config/training/ecology-tuned-2026-09-28.toml
run() { # name browser hours seeds threads
  nice -n 10 taskset -c 8-15,24-31 target/release/cubarium-search voxel-ecology-check \
    --centre browser=$2 --centre littershredder=$C/r2-littershredder-upd512.json \
    --arm default=default --arm tuned=$T --seeds $4 --hours $3 --water-every 1 --threads $5 \
    --out runs/step1/$1 > runs/step1/$1.log 2> runs/step1/$1.err
}
mkdir -p runs/step1
run h6-heur heuristic 6 1,2 4 &
run h6-r2 $C/r2-frondgrazer-upd160.json 6 1,2 4 &
run h1-heur heuristic 1 1,2,3,4,5,6,7,8 4 &
run h1-r2 $C/r2-frondgrazer-upd160.json 1 1,2,3,4,5,6,7,8 4 &
wait
