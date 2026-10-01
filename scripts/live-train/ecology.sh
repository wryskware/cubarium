#!/bin/bash
cd /home/wrysk/wryskware/cubarium/.claude/worktrees/live-train
C=config/training/centres
cmd=$1; shift
exec nice -n 10 taskset -c 8-15,24-31 target/release/cubarium-search voxel-ecology-$cmd \
  --centre browser=$C/r2-frondgrazer-upd160.json --centre littershredder=$C/r2-littershredder-upd512.json "$@"
