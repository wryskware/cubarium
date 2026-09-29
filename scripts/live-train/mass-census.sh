#!/bin/bash
# Audit round step 2's census: heuristic walker vs heuristic parker at one mass scale,
# seeds 1-4 x 6 h, new defaults, still water. Usage: [BIN=path] mass-census.sh <k> [threads]
cd /home/wrysk/wryskware/cubarium/.claude/worktrees/live-train
BIN=${BIN:-target/release/cubarium-search}
K=$1; T=${2:-4}
A="--arm x$K=config/training/mass-scale/x$K.toml"; [ "$K" = 1 ] && A="--arm x1=default"
O=runs/census-x$K; mkdir -p $O
for b in walker parker; do
  c=heuristic; [ $b = parker ] && c=parker
  nice -n 10 taskset -c 8-15,24-31 $BIN voxel-ecology-check --centre browser=$c \
    --centre littershredder=$c $A --seeds 1,2,3,4 --hours 6 --water-every 1 --threads $T \
    --out $O/$b > $O/$b.log 2> $O/$b.err &
done
wait
