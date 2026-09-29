#!/bin/bash
# Audit round step 2: heuristic walker vs heuristic parker (and round 2's brains, with the
# Self references as the contract has them (A) and read at the mass scale (B)) at every
# founder x1, x30, x100 and x300 heavier, seeds 1-8 x 60 min, new defaults, still water.
# Usage: mass-scale.sh [threads-per-run]   (two runs at a time)
cd /home/wrysk/wryskware/cubarium/.claude/worktrees/live-train
C=config/training/centres
M=config/training/mass-scale
T=${1:-8}
O=runs/mass
mkdir -p $O
run() { # name browser shredder extra...
  local name=$1 b=$2 s=$3; shift 3
  nice -n 10 taskset -c 8-15,24-31 target/release/cubarium-search voxel-ecology-check \
    --centre browser=$b --centre littershredder=$s \
    --arm x1=default --arm x30=$M/x30.toml --arm x100=$M/x100.toml --arm x300=$M/x300.toml \
    --seeds 1,2,3,4,5,6,7,8 --hours 1 --water-every 1 --threads $T "$@" \
    --out $O/$name > $O/$name.log 2> $O/$name.err
}
R2B=$C/r2-frondgrazer-upd160.json
R2S=$C/r2-littershredder-upd512.json
run walker heuristic heuristic &
run parker parker parker &
wait
run r2a $R2B $R2S &
run r2b $R2B $R2S --mass-relative-self &
wait
