#!/bin/bash
# Live training on the desktop terrarium from round 2's centres (transfer):
# [BIN=path] terrarium-train.sh <out> <sigma> <pairs> <worlds> <wall_seconds> [extra...]
cd /home/wrysk/wryskware/cubarium/.claude/worktrees/live-train
BIN=${BIN:-target/release/cubarium-search}
C=config/training/centres
O=$1; SIG=$2; P=$3; W=$4; WALL=$5; shift 5
mkdir -p $O
nice -n 10 taskset -c 8-15,24-31 $BIN voxel-live-train --world config/desktop/terrarium.toml \
  --centre browser=transfer:$C/r2-frondgrazer-upd160.json \
  --centre littershredder=transfer:$C/r2-littershredder-upd512.json \
  --sigma $SIG --pairs $P --worlds $W --threads $W --wall-seconds $WALL --save-every 1 \
  --out $O "$@" > $O/run.log 2> $O/run.err
