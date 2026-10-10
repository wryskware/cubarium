#!/bin/bash
# CELL-SHRINK round 2 evidence and cost, chained. Writes runs/DONE-round2.
set -u
W=/home/wrysk/wryskware/cubarium/.claude/worktrees/cell-shrink
ART=/home/wrysk/wryskware/cubarium/art/gen/runs/cell-shrink-2026-10-10/round2
LOGS=/home/wrysk/wryskware/cubarium/art/gen/runs/cell-shrink-2026-10-10/logs2
S=/home/wrysk/wryskware/cubarium/runs/glow-ease-states/state-day2-f77
cd $W
NICE="chrt -i 0 nice -n 19"
CPUS="taskset -c 8-15,24-31"
G=runs/glow_capture.r2; cp target/release/examples/glow_capture $G
cp target/release/examples/light_capture runs/light_capture.r2
OFF='plants = { light_by_size = false }'
DRY=(--flora-at 0 'springturf = { cure_water = 0.98, cure_resume_water = 0.99, cure_after_s = 5.0 }'
     --flora-at 0 'latticevine = { dry_water = 0.98, wet_water = 0.99, dry_spell_s = 5.0 }')
# 0. main (b99ff9f3) light_capture, target seeded by reflink from the main checkout's.
rm -rf runs/main-src && mkdir -p runs/main-src runs/main-target
git archive b99ff9f3 | tar -x -C runs/main-src
[ -d runs/main-target/release ] || cp -a --reflink=auto /home/wrysk/wryskware/cubarium/target/release runs/main-target/release
( cd runs/main-src && CARGO_TARGET_DIR=$W/runs/main-target timeout 3600 $NICE $CPUS \
    cargo build --release -p cubarium --example light_capture -j 8 ) > runs/main-build.log 2>&1
cp runs/main-target/release/examples/light_capture runs/light_capture.main
# 1. The shed at 6x, today | fixed (A: light_by_size off), with the per-frame plant log.
timeout 1800 $NICE $CPUS $G --load $S --colour --speed 1 --seconds 9 --a "$OFF" --close 160,96 --zoom 6 --close-at 688,512 \
    "${DRY[@]}" --video $ART/shed-today-fixed-x1-full.mp4 --close-video $ART/shed-close6x-today-fixed-x1.mp4 \
    --plant-log runs/r2-shed.csv > runs/r2-shed.log 2>&1
python3 -I $LOGS/steps.py $ART/shed-close6x-today-fixed-x1.mp4 330 480 > $LOGS/shed-steps.txt 2>&1
# 2. A natural day, today | fixed, 30 s at x1.
timeout 2400 $NICE $CPUS $G --load $S --colour --scan --speed 1 --seconds 30 --a "$OFF" \
    --video $ART/day-today-fixed-x1-full.mp4 --close-video $ART/day-today-fixed-x1-close2x.mp4 \
    --plant-log runs/r2-day.csv > runs/r2-day.log 2>&1
python3 -I $LOGS/steps.py $ART/day-today-fixed-x1-close2x.mp4 0 1800 > $LOGS/day-steps.txt 2>&1
# 3. Cost: the walk, main vs branch, interleaved (no plant changes in a still frame).
for round in 1 2 3; do
  for side in main r2; do
    timeout 900 $CPUS runs/light_capture.$side --state $S --px 6 --frames 300 --repack \
        --name day-$side --out runs/cost-$side >> runs/cost-$side.log 2>&1
  done
done
# 4. Cost with cells changing: B's walk and plant pass, on vs off, interleaved; a natural
#    day (20 s) and the shed (9 s, 3,617 cells at once).
for round in 1 2 3; do
  for arm in on off; do
    B=(); [ "$arm" = off ] && B=(--b "$OFF")
    timeout 1800 $CPUS $G --load $S --colour --speed 1 --seconds 20 "${B[@]}" \
        --plant-log runs/cost-day-$arm-$round.csv > runs/cost-day-$arm-$round.log 2>&1
    timeout 1800 $CPUS $G --load $S --colour --speed 1 --seconds 9 "${B[@]}" "${DRY[@]}" \
        --plant-log runs/cost-shed-$arm-$round.csv > runs/cost-shed-$arm-$round.log 2>&1
  done
done
python3 -I $LOGS/summary.py runs > $LOGS/cost.txt 2>&1
cp runs/r2-*.csv runs/r2-*.log runs/cost-*.csv runs/cost-*.log runs/main-build.log runs/chain2.sh $LOGS/ 2>/dev/null
touch runs/DONE-round2
