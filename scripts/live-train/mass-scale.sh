#!/bin/bash
# Audit round step 2: heuristic walker vs heuristic parker (and round 2's brains, with the
# Self references as the contract has them (A) and read at the mass scale (B)) at every
# founder x1, x30, x100 and x300 heavier, seeds 1-8 x 60 min, new defaults, still water.
# Usage: [BIN=path] mass-scale.sh [threads-per-run] [arm ...]   (arms: walker parker r2a r2b;
# all four by default, two at a time). BIN defaults to target/release/cubarium-search.
cd /home/wrysk/wryskware/cubarium/.claude/worktrees/live-train
C=config/training/centres
M=config/training/mass-scale
BIN=${BIN:-target/release/cubarium-search}
T=${1:-8}; shift
ARMS=${*:-walker parker r2a r2b}
O=runs/mass
mkdir -p $O
R2B=$C/r2-frondgrazer-upd160.json
R2S=$C/r2-littershredder-upd512.json
run() { # name
  local b s extra=""
  case $1 in
    walker) b=heuristic; s=heuristic ;;
    parker) b=parker; s=parker ;;
    r2a) b=$R2B; s=$R2S ;;
    r2b) b=$R2B; s=$R2S; extra=--mass-relative-self ;;
  esac
  nice -n 10 taskset -c 8-15,24-31 $BIN voxel-ecology-check \
    --centre browser=$b --centre littershredder=$s \
    --arm x1=default --arm x30=$M/x30.toml --arm x100=$M/x100.toml --arm x300=$M/x300.toml \
    --seeds 1,2,3,4,5,6,7,8 --hours 1 --water-every 1 --threads $T $extra \
    --out $O/$1 > $O/$1.log 2> $O/$1.err
}
set -- $ARMS
while [ $# -gt 0 ]; do
  run $1 & [ $# -gt 1 ] && run $2 &
  wait; shift; [ $# -gt 0 ] && shift
done
