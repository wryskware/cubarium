#!/bin/bash
# 60-min checks on the desktop terrarium: [BIN=path] terrarium-check.sh <out> <threads> <arm-spec> <browser> <shredder> [extra...]
cd /home/wrysk/wryskware/cubarium/.claude/worktrees/live-train
BIN=${BIN:-target/release/cubarium-search}
O=$1; T=$2; A=$3; B=$4; S=$5; shift 5
mkdir -p $(dirname $O)
nice -n 10 taskset -c 8-15,24-31 $BIN voxel-ecology-check --world config/desktop/terrarium.toml \
  --centre browser=$B --centre littershredder=$S --arm $A --seeds 1,2,3,4,5,6,7,8 --hours 1 \
  --water-every 1 --threads $T --out $O "$@" > $O.log 2> $O.err
