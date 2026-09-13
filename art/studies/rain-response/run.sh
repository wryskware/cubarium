#!/usr/bin/env bash
# Rain-response study (art/studies/rain-response): prepare an isolated source copy of the
# release commit, apply the presenter prototype, add the study's capture tool and tests,
# then build and run them. Production sources are never touched; every artifact lives under
# the workspace `captures/` directory (gitignored, on the spacious filesystem).
#
#   art/studies/rain-response/run.sh prepare   # copy 9cf0e1d, apply presenter-v2.patch, add files
#   art/studies/rain-response/run.sh test      # cargo test the study tests in the copy
#   art/studies/rain-response/run.sh capture   # paired native captures (quiet / natural / applied)
#   art/studies/rain-response/run.sh sheets    # comparison sheets + measurements from the captures
#
# `presenter-v1.patch` is the retained first candidate (unreadable: see the report);
# `RAIN_STUDY_PATCH=presenter-v1.patch run.sh prepare` rebuilds it. Both patches are
# generated with `diff -u` from the isolated copy against the base commit's file.
set -euo pipefail
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)
study="$repo/art/studies/rain-response"
patch_file=${RAIN_STUDY_PATCH:-presenter-v2.patch}
base=${RAIN_STUDY_BASE:-9cf0e1d}
src=${RAIN_STUDY_SRC:-$repo/captures/build-cache/rain-study-src}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$repo/captures/build-cache/fable-rain}
out=${RAIN_STUDY_OUT:-$repo/captures/rain-response-2026-09-13}
opening=${RAIN_STUDY_WORLD:-$repo/captures/hunter-openings-2026-09-13/seed-1/world-144000.cubw}

case "${1:-}" in
  prepare)
    mkdir -p -- "$(dirname -- "$src")"
    if ! mkdir -- "$src"; then
      echo "refusing existing source path; choose a fresh RAIN_STUDY_SRC" >&2
      exit 1
    fi
    git -C "$repo" archive "$base" | tar -x -C "$src"
    patch -p1 -d "$src" < "$study/$patch_file"
    cp "$study/rain_response_capture.rs" "$src/crates/cubarium/examples/rain_response_capture.rs"
    cp "$study/rain_response.rs" "$src/crates/cubarium/tests/rain_response.rs"
    echo "prepared $src from $base"
    ;;
  test)
    cd "$src" && cargo test --offline -p cubarium --test rain_response -- --nocapture
    ;;
  capture)
    mkdir -p -- "$(dirname -- "$out")"
    if ! mkdir -- "$out"; then
      echo "refusing existing capture path; choose a fresh RAIN_STUDY_OUT" >&2
      exit 1
    fi
    cd "$src" && cargo build --offline --release -p cubarium --example rain_response_capture
    bin="$CARGO_TARGET_DIR/release/examples/rain_response_capture"
    seed8=${RAIN_STUDY_WORLD8:-$repo/captures/hunter-openings-2026-09-13/seed-8/world-144000.cubw}
    # 1. Scans with no frames: where and how hard does natural rain fall in these openings?
    "$bin" --world "$opening" --ticks 36000 --out "$out/scan-long-seed1"
    "$bin" --world "$seed8" --ticks 36000 --out "$out/scan-long-seed8"
    # 2. The care contract's Standard Rain at its three fixed targets (all in the soil band,
    #    bare on the seed-1 opening), at a tap over grown stalks, and the natural peak.
    "$bin" --world "$opening" --ticks 900 --rain-at 300 --target 0 --from 280 --to 520 --out "$out/applied-t0"
    "$bin" --world "$opening" --ticks 450 --rain-at 300 --target 1 --from 330 --to 370 --out "$out/applied-t1-seam"
    "$bin" --world "$opening" --ticks 450 --rain-at 300 --target 2 --from 330 --to 370 --out "$out/applied-t2-rim"
    "$bin" --world "$opening" --ticks 900 --rain-at 300 --at 3,6,42 --from 280 --to 520 --out "$out/applied-seed1-left-1-10"
    "$bin" --world "$seed8" --ticks 18100 --from 18000 --to 18060 --out "$out/natural-seed8"
    # 3. The ceiling: a mature synthetic field under Standard-shaped rates.
    "$bin" --synthetic 0,8,6 --from 580 --to 800 --out "$out/synthetic-front-8-6"
    echo "captures under $out"
    ;;
  sheets)
    cd "$out"
    python3 "$study/sheet.py" applied-t0 --face 0 --cx 8 --cy 12 --ticks 290,320,340,360,380,400,420,440,460 --radius 14 --motion 340-380
    python3 "$study/sheet.py" applied-seed1-left-1-10 --face 3 --cx 1 --cy 10 --ticks 290,320,340,360,380,400,420,440,460 --radius 12 --motion 340-380
    python3 "$study/sheet.py" natural-seed8 --face 3 --cx 4 --cy 0 --ticks 18000,18010,18020,18030,18040,18050,18060 --radius 12 --motion 18000-18060
    python3 "$study/sheet.py" synthetic-front-8-6 --face 0 --cx 8 --cy 6 --ticks 590,610,630,660,690,710,720,730,740,760 --radius 12 --motion 640-680
    python3 "$study/sheet.py" synthetic-front-8-6 --face 0 --cx 8 --cy 6 --ticks 660,661,662,663,664,665,666,667,668,669 --radius 8 --out synthetic-front-8-6/sheet-consecutive-ticks.png
    ;;
  *)
    echo "usage: $0 prepare|test|capture|sheets" >&2; exit 2
    ;;
esac
