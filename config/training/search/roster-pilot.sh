#!/usr/bin/env bash
# The full-roster PILOT search (Astra's review 2026-10-02, F9/F10): roster-pilot.toml,
# lambda 8, three common seeds per generation, 2-h worlds, sigma0 0.08, water every tick.
# An explicitly partial, pre-night-mechanics exploration; launched by Fable, not by workers.
#
#   config/training/search/roster-pilot.sh <checkout> <workers> <wall-seconds> [out-dir]
#
# <checkout>  the worktree or checkout to build and run (its relative config paths).
# <workers>   worlds in parallel (each on one thread: CUBARIUM_THREADS=1). One generation is
#             9 candidates (8 draws + the reference) x 3 seeds = 27 worlds of ~26 min each.
# <wall-seconds> the run stops before a generation that would pass this.
#
# The nominee is <out>/best.toml, chosen by raw fitness over DIFFERENT seed sets: compare it
# with the incumbent (the pilot file itself, the exhibit's values) on common held-out seeds
# before promoting anything:
#   CUBARIUM_THREADS=1 nice -n 10 ./target/release/cubarium-search voxel-ecology-check --targets \
#     --arm incumbent=config/training/search/roster-pilot.toml --arm nominee=<out>/best.toml \
#     --seeds 9,10,11,12,13,14,15,16 --hours 2 --water-every 1 \
#     --world config/desktop/terrarium.toml --threads <workers> --out <out>/compare
# Both arms decode over their own inlined base; the brains are the file's [brains].
set -euo pipefail
CHECKOUT=${1:?usage: roster-pilot.sh <checkout> <workers> <wall-seconds> [out-dir]}
WORKERS=${2:?workers}
WALL=${3:?wall seconds}
cd "$CHECKOUT"
OUT=${4:-runs/roster-pilot-$(date +%Y%m%d-%H%M)}
nice -n 10 cargo build --release -j "$WORKERS" -p cubarium-search --bin cubarium-search
mkdir -p "$OUT"
git rev-parse HEAD > "$OUT/commit.txt"
ulimit -v $((64 * 1024 * 1024))
export CUBARIUM_THREADS=1 RAYON_NUM_THREADS=1
nice -n 10 ./target/release/cubarium-search voxel-ecology-search \
  --search config/training/search/roster-pilot.toml \
  --world config/desktop/terrarium.toml \
  --lambda 8 --seeds-per-generation 3 --hours 2 --sigma0 0.08 --water-every 1 \
  --generations 1000 --threads "$WORKERS" --wall-seconds "$WALL" \
  --out "$OUT" 2>&1 | tee "$OUT/search.log"
