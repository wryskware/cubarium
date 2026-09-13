#!/usr/bin/env bash
# Rain-response playback review (art/studies/rain-response-review): compare the original
# presenter, the committed v2 candidate and one bounded alternative (v3) on identical
# fixtures, measure motion, and generate browser playback pages. Everything runs in
# frozen source copies under captures/build-cache (never the production tree) and writes
# only to fresh capture directories; nothing here touches the earlier study's outputs.
#
#   run.sh prepare v3        # fresh copy of 9cf0e1d + presenter-v2.patch + presenter-v3.patch
#   run.sh build v2|v3       # install the review capture tool in the copy and build it
#   run.sh test v3           # the frozen study's six tests against the v3 copy
#   run.sh capture v2|v3     # the fixture set for one variant (refuses existing outputs)
#   run.sh measure           # motion.py over every fixture and variant → motion-*.json
#   run.sh pages             # playback pages (original / v2 / v3 side by side)
#
# A v2 copy is `art/studies/rain-response/run.sh prepare` with RAIN_STUDY_SRC set to the
# v2 path below; v3 is that plus `presenter-v3.patch` (generated with diff -u from the
# v2 copy). Both copies were prepared once and are frozen; `prepare` refuses to overwrite.
set -euo pipefail
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)
review="$repo/art/studies/rain-response-review"
study="$repo/art/studies/rain-response"
variant=${2:-}
src_of() { echo "${RAIN_REVIEW_SRC:-$repo/captures/build-cache/rain-review-src-$1}"; }
target_of() { echo "${RAIN_REVIEW_TARGET:-$repo/captures/build-cache/fable-rain-review}-$1"; }
out=${RAIN_REVIEW_OUT:-$repo/captures/rain-response-review-2026-09-13}
seed1=$repo/captures/hunter-openings-2026-09-13/seed-1/world-144000.cubw
seed8=$repo/captures/hunter-openings-2026-09-13/seed-8/world-144000.cubw

need_variant() { case "$variant" in v2|v3) ;; *) echo "usage: $0 $1 v2|v3" >&2; exit 2;; esac; }

case "${1:-}" in
  prepare)
    need_variant prepare
    src=$(src_of "$variant")
    RAIN_STUDY_SRC="$src" "$study/run.sh" prepare
    if [ "$variant" = v3 ]; then patch -p1 -d "$src" < "$review/presenter-v3.patch"; fi
    ;;
  build)
    need_variant build
    src=$(src_of "$variant")
    cp "$review/rain_review_capture.rs" "$src/crates/cubarium/examples/rain_review_capture.rs"
    cd "$src" && CARGO_TARGET_DIR=$(target_of "$variant") cargo build --offline --release -p cubarium --example rain_review_capture
    ;;
  test)
    need_variant test
    src=$(src_of "$variant")
    cp "$review/rain_restart.rs" "$src/crates/cubarium/tests/rain_restart.rs"
    cd "$src" && CARGO_TARGET_DIR=$(target_of "$variant") cargo test --offline -p cubarium --test rain_response --test rain_restart -- --nocapture
    ;;
  capture)
    need_variant capture
    bin=$(target_of "$variant")/release/examples/rain_review_capture
    o="$out/$variant"
    mkdir -p -- "$out"
    if ! mkdir -- "$o"; then echo "refusing existing capture path $o" >&2; exit 1; fi
    # young: real stage-0/1 stalks on the seed-1 opening, a tap over Left (1,10); the
    # presenter restarted 0.5 s after the shower's last sample (the decay tail).
    "$bin" --world "$seed1" --ticks 900 --rain-at 300 --at 3,6,42 --from 280 --to 520 --restart-at 430 --out "$o/young"
    # mature: the synthetic field at its cap, Front (8,6); restarted at the shower's peak.
    "$bin" --synthetic 0,8,6 --from 580 --to 800 --restart-at 660 --out "$o/mature"
    # mature, restarted 0.25 s after the last sample.
    "$bin" --synthetic 0,8,6 --from 700 --to 800 --restart-at 725 --out "$o/mature-restart-after"
    # glowcap: the soil-band cap under the same shower, Front (11,12) (rootveil centre,
    # four glowcaps in the patch).
    "$bin" --synthetic 0,11,12 --from 580 --to 800 --out "$o/glowcap"
    # sustained natural drizzle: seed 8 at its rain peak, 15 s.
    "$bin" --world "$seed8" --ticks 18300 --from 18000 --to 18300 --out "$o/natural-sustained"
    # natural settling: the seed-8 run ends at elapsed 21252; 5 s before to 5 s after.
    "$bin" --world "$seed8" --ticks 21360 --from 21150 --to 21350 --out "$o/natural-settle"
    echo "captures under $o"
    ;;
  measure)
    for v in v2 v3; do
      o="$out/$v"
      python3 "$review/motion.py" "$o/young" --face 3 --cx 1 --cy 10 --radius 12 --label young
      python3 "$review/motion.py" "$o/young" --face 3 --cx 1 --cy 10 --radius 12 --from 280 --to 299 --label young-quiet-before
      python3 "$review/motion.py" "$o/young" --face 3 --cx 1 --cy 10 --radius 12 --from 340 --to 380 --label young-peak
      python3 "$review/motion.py" "$o/mature" --face 0 --cx 8 --cy 6 --radius 12 --label mature
      python3 "$review/motion.py" "$o/mature" --face 0 --cx 8 --cy 6 --radius 12 --from 640 --to 680 --label mature-peak
      python3 "$review/motion.py" "$o/mature-restart-after" --face 0 --cx 8 --cy 6 --radius 12 --label mature-restart-after
      python3 "$review/motion.py" "$o/glowcap" --face 0 --cx 11 --cy 12 --radius 12 --from 640 --to 680 --label glowcap-peak
      python3 "$review/motion.py" "$o/natural-sustained" --face 3 --cx 4 --cy 0 --radius 12 --label natural-sustained
      python3 "$review/motion.py" "$o/natural-settle" --face 3 --cx 4 --cy 0 --radius 12 --label natural-settle
    done
    ;;
  pages)
    p="$out/playback"; mkdir -p -- "$p"
    python3 "$review/playback.py" "$p/young.html" --title "young stalks, seed 1 Left (1,10), tap at tick 300" --column "original=$out/v2/young/old" --column "v2=$out/v2/young/new" --column "v3=$out/v3/young/new" --from 280 --to 520 --face 3 --cx 1 --cy 10
    python3 "$review/playback.py" "$p/mature.html" --title "mature synthetic field, Front (8,6)" --column "original=$out/v2/mature/old" --column "v2=$out/v2/mature/new" --column "v3=$out/v3/mature/new" --from 580 --to 800 --face 0 --cx 8 --cy 6
    python3 "$review/playback.py" "$p/mature-restart.html" --title "mature: restart at the peak (tick 660) vs continuous" --column "v2 continuous=$out/v2/mature/new" --column "v2 restarted@660=$out/v2/mature/restart" --column "v3 continuous=$out/v3/mature/new" --column "v3 restarted@660=$out/v3/mature/restart" --from 660 --to 800 --face 0 --cx 8 --cy 6
    python3 "$review/playback.py" "$p/mature-restart-after.html" --title "mature: restart 0.25 s after the last sample (tick 725) vs continuous" --column "v2 continuous=$out/v2/mature-restart-after/new" --column "v2 restarted@725=$out/v2/mature-restart-after/restart" --column "v3 continuous=$out/v3/mature-restart-after/new" --column "v3 restarted@725=$out/v3/mature-restart-after/restart" --from 725 --to 800 --face 0 --cx 8 --cy 6
    python3 "$review/playback.py" "$p/glowcap.html" --title "glowcap, synthetic Front (11,12)" --column "original=$out/v2/glowcap/old" --column "v2=$out/v2/glowcap/new" --column "v3=$out/v3/glowcap/new" --from 580 --to 800 --face 0 --cx 11 --cy 12
    python3 "$review/playback.py" "$p/natural-sustained.html" --title "sustained natural drizzle, seed 8 Left (4,0), 15 s" --column "original=$out/v2/natural-sustained/old" --column "v2=$out/v2/natural-sustained/new" --column "v3=$out/v3/natural-sustained/new" --from 18000 --to 18300 --face 3 --cx 4 --cy 0
    python3 "$review/playback.py" "$p/natural-settle.html" --title "natural drizzle ending, seed 8 Left (4,0)" --column "original=$out/v2/natural-settle/old" --column "v2=$out/v2/natural-settle/new" --column "v3=$out/v3/natural-settle/new" --from 21150 --to 21350 --face 3 --cx 4 --cy 0
    echo "open file://$p/<name>.html in a browser"
    ;;
  *)
    echo "usage: $0 prepare|build|test|capture v2|v3 ; measure ; pages" >&2; exit 2
    ;;
esac
