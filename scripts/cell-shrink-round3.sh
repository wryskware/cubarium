#!/bin/bash
# Headless CELL-SHRINK timings and evidence. Launch once, detached; do not poll.
set -uo pipefail
W=$(git rev-parse --show-toplevel)
cd "$W"
ROOT=/home/wrysk/wryskware/cubarium
ART=$ROOT/art/gen/runs/cell-shrink-2026-10-10/round3
LOGS=$ROOT/art/gen/runs/cell-shrink-2026-10-10/logs3
STATE=$ROOT/runs/glow-ease-states/state-day2-f77
G=$W/target/release/examples/glow_capture
L=$W/target/release/examples/light_capture
MAIN=$W/runs/light_capture.main
export TMPDIR=$W/runs/scratch-round3
mkdir -p "$ART" "$LOGS" "$TMPDIR"
trap 'printf "batch exit %s\n" "$?" >> "$LOGS/status.txt"; touch "$W/runs/DONE-round3"' EXIT
OFF='plants = { light_by_size = false }'
DRY=(--flora-at 0 'springturf = { cure_water = 0.98, cure_resume_water = 0.99, cure_after_s = 5.0 }'
     --flora-at 0 'latticevine = { dry_water = 0.98, wet_water = 0.99, dry_spell_s = 5.0 }')
printf 'main source b99ff9f3\nbranch source %s\nslot 0 for every serial arm; census jobs occupy slot 1\n' \
    "$(git rev-parse HEAD)" > "$LOGS/provenance.txt"
sha256sum "$G" "$L" "$MAIN" > "$LOGS/binaries.sha256"
sha256sum "$STATE"/* config/desktop/terrarium.toml runs/main-src/config/desktop/terrarium.toml >> "$LOGS/provenance.txt"
cp runs/round3-tests.log runs/round3-tests.exit runs/round3-build.log runs/round3-build.exit "$LOGS/"
printf 'name\texit\n' > "$LOGS/commands.tsv"
run() {
    local name=$1
    shift
    timeout 270 scripts/run-study.py --slot 0 -- "$@" > "$LOGS/$name.log" 2>&1
    local result=$?
    printf '%s\t%s\n' "$name" "$result" >> "$LOGS/commands.tsv"
}

# Three interleaved still-state rounds, reversing order in the middle round.
for round in 1 2 3; do
    sides=(main r3)
    [ "$round" = 2 ] && sides=(r3 main)
    for side in "${sides[@]}"; do
        binary=$L
        config=$W/config/desktop/terrarium.toml
        if [ "$side" = main ]; then
            binary=$MAIN
            config=$W/runs/main-src/config/desktop/terrarium.toml
        fi
        run "cost-$side-$round" "$binary" --state "$STATE" --px 6 --frames 300 --repack \
            --config "$config" --name "day-$side-$round" --out "$W/runs/round3-light-$side"
    done
done

# B's timed walk and plant pass, with an identical off A in both runs. Empty arrays
# omit the on override; no inline-table braces occur inside Bash's ${...} expansion.
for kind in day shed; do
    seconds=20
    extra=()
    if [ "$kind" = shed ]; then seconds=9; extra=("${DRY[@]}"); fi
    for round in 1 2 3; do
        arms=(on off)
        [ "$round" = 2 ] && arms=(off on)
        for arm in "${arms[@]}"; do
            B=()
            [ "$arm" = off ] && B=(--b "$OFF")
            run "cost-$kind-$arm-$round" "$G" --load "$STATE" --colour --speed 1 \
                --seconds "$seconds" --a "$OFF" "${B[@]}" "${extra[@]}" \
                --plant-log "$LOGS/cost-$kind-$arm-$round.csv"
        done
    done
done

# Repeat Round 2's exact framing and duration for comparison with its existing clips.
run r3-shed "$G" --load "$STATE" --colour --speed 1 --seconds 9 --a "$OFF" \
    --close 160,96 --zoom 6 --close-at 688,512 "${DRY[@]}" \
    --video "$ART/shed-today-fixed-x1-full.mp4" \
    --close-video "$ART/shed-close6x-today-fixed-x1.mp4" --plant-log "$LOGS/r3-shed.csv"
run r3-day "$G" --load "$STATE" --colour --scan --speed 1 --seconds 30 --a "$OFF" \
    --video "$ART/day-today-fixed-x1-full.mp4" \
    --close-video "$ART/day-today-fixed-x1-close2x.mp4" --plant-log "$LOGS/r3-day.csv"

timeout 270 python3 -I scripts/cell-shrink-analyze.py "$LOGS" "$ART" > "$LOGS/analysis.txt" 2>&1
printf 'analysis exit %s\n' "$?" >> "$LOGS/status.txt"
sha256sum --check "$LOGS/binaries.sha256" >> "$LOGS/status.txt" 2>&1
# EXIT always writes the completion marker, including when a phase failed.
