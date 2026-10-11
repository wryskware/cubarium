#!/usr/bin/env bash
# One detached, idle-priority chain. Builds both arms ONLY in this worktree, restores
# the committed files before testing, measures weather before making review media.
# Launch: setsid nohup chrt -i 0 nice -n 19 timeout 7200 scripts/fog-occlude-batch.sh
set -euo pipefail
tree=$(cd "$(dirname "$0")/.." && pwd)
cd "$tree"
out=/home/wrysk/wryskware/cubarium/art/gen/runs/fog-occlude-2026-10-11
cache="$tree/target/fog-occlude"
export CARGO_TARGET_DIR="$tree/target" CARGO_BUILD_JOBS=4 CUBARIUM_GPU=5080 CUBARIUM_THREADS=4
mkdir -p "$out/logs" "$out/captures" "$out/timing" "$cache/bin" "$cache/fixed"
phase=preflight
restore_needed=0
files=(crates/cubarium-gpu/src/voxel.rs crates/cubarium-gpu/src/plants.rs
  crates/cubarium-gpu/src/figures.rs crates/cubarium-gpu/shaders/voxel.frag
  crates/cubarium-gpu/shaders/world.glsl crates/cubarium-gpu/shaders/plant_main.glsl
  crates/cubarium-gpu/shaders/composite.frag)
restore() {
  if (( restore_needed )); then
    for file in "${files[@]}"; do cp --reflink=auto "$cache/fixed/$file" "$file"; done
    cp --reflink=auto "$cache/fixed/crates/cubarium-gpu/shaders/"*.spv crates/cubarium-gpu/shaders/
    restore_needed=0
  fi
}
finish() {
  status=$?
  trap - EXIT
  restore
  printf 'exit=%s phase=%s\n' "$status" "$phase" > "$out/DONE"
}
trap finish EXIT
trap 'exit 143' TERM INT
test "$(git branch --show-current)" = fog-occlude
git diff --quiet
git diff --cached --quiet
base=$(git merge-base main HEAD)
fixed=$(git rev-parse HEAD)
printf 'main renderer=%s\nfixed=%s\nshared harness and identical weather timestamp instrumentation\n' "$base" "$fixed" > "$out/logs/sources.txt"
for file in "${files[@]}"; do cp --parents --reflink=auto "$file" "$cache/fixed/"; done
cp --parents --reflink=auto crates/cubarium-gpu/shaders/*.spv "$cache/fixed/"
# Extract main's rendering sources from Git, adding ONLY the same weather timestamps
# and reader to that arm. No main checkout or other worktree is built or modified.
python3 - "$base" "$cache" <<'PY'
import pathlib, subprocess, sys
base, cache = sys.argv[1], pathlib.Path(sys.argv[2])
files = [p for p in (cache/'fixed/crates').rglob('*') if p.is_file() and p.suffix != '.spv']
for src in files:
    rel = src.relative_to(cache/'fixed')
    dest = cache/'before'/rel
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(subprocess.check_output(['git', 'show', f'{base}:{rel}']))
rel = pathlib.Path('crates/cubarium-gpu/src/voxel.rs')
new = (cache/'fixed'/rel).read_text()
old = (cache/'before'/rel).read_text()
old = old.replace('const QUERY_SLOTS: u32 = 10;', 'const QUERY_SLOTS: u32 = 12;')
old = old.replace('const Q_SHADE_START: u32 = 9;', 'const Q_SHADE_START: u32 = 9;\nconst Q_WEATHER_START: u32 = 10;\nconst Q_WEATHER_END: u32 = 11;')
line = '                d.cmd_write_timestamp(cb, vk::PipelineStageFlags::BOTTOM_OF_PIPE, self.queries, q + 5);'
assert old.count(line) == 1
old = old.replace(line, line+'\n                d.cmd_write_timestamp(cb, vk::PipelineStageFlags::BOTTOM_OF_PIPE, self.queries, q + Q_WEATHER_START);')
marker = '                if let Some(b) = &self.bloom\n                    && self.params.bloom > 0.0'
assert old.count(marker) == 1
old = old.replace(marker, '                d.cmd_write_timestamp(cb, vk::PipelineStageFlags::BOTTOM_OF_PIPE, self.queries, q + Q_WEATHER_END);\n'+marker)
begin = new.index("    /// The last retired frame's weather overlay")
end = new.index("    /// The last retired frame's walk and plant pass", begin)
marker = "    /// The last retired frame's walk and plant pass"
assert old.count(marker) == 1
old = old.replace(marker, new[begin:end]+marker)
(cache/'before'/rel).write_text(old)
PY
restore_needed=1
for file in "${files[@]}"; do cp --reflink=auto "$cache/before/$file" "$file"; done
phase=build-main
crates/cubarium-gpu/shaders/compile.sh > "$out/logs/shaders-main.log" 2>&1
cargo build --release -p cubarium --example fog_occlude --example fog_capture > "$out/logs/build-main.log" 2>&1
cp --reflink=auto target/release/examples/fog_occlude "$cache/bin/today"
cp --reflink=auto target/release/examples/fog_capture "$cache/bin/live-today"
restore
phase=build-fixed
cargo build --release -p cubarium --example fog_occlude --example fog_capture > "$out/logs/build-fixed.log" 2>&1
cp --reflink=auto target/release/examples/fog_occlude "$cache/bin/fixed"
cp --reflink=auto target/release/examples/fog_capture "$cache/bin/live-fixed"
phase=mechanism-tests
cargo nextest run --release -p cubarium-gpu --test-threads 2 > "$out/logs/tests.log" 2>&1
phase=weather-cost
# ABBA, repeated twice. Warm-up frames are omitted by the reducer. The clock, scene
# and projection match in every arm; timestamp boundaries include weather barriers.
arms=(today fixed fixed today today fixed fixed today)
for i in "${!arms[@]}"; do
  arm=${arms[$i]}
  "$cache/bin/$arm" --case mixed --px 6 --frames 360 --out "$out/timing/$i-$arm" > "$out/logs/timing-$i-$arm.log" 2>&1
done
python3 - "$out" <<'PY'
import csv, pathlib, statistics, sys
out = pathlib.Path(sys.argv[1]); rounds = []
for folder in sorted((out/'timing').iterdir()):
    with (folder/'timing.csv').open() as f:
        data = [row for row in csv.DictReader(f) if int(row['frame']) >= 60]
    weather = [float(row['weather_ms']) for row in data]
    world = [float(row['world_ms']) for row in data]
    assert weather and min(weather) > 0, f'missing weather timestamps: {folder}'
    rounds.append((folder.name, statistics.median(weather), statistics.median(world)))
def values(arm, column): return [r[column] for r in rounds if r[0].endswith(arm)]
a, b = statistics.median(values('today', 1)), statistics.median(values('fixed', 1))
wa, wb = statistics.median(values('today', 2)), statistics.median(values('fixed', 2))
lines = [f'{name}: weather={w:.6f} ms, world={world:.6f} ms' for name,w,world in rounds]
lines += [f'weather main={a:.6f} ms fixed={b:.6f} ms delta={b-a:+.6f} ms',
          f'world main={wa:.6f} ms fixed={wb:.6f} ms delta={wb-wa:+.6f} ms']
if b-a > 0.1:
    lines += ['STOP: weather cost exceeds 0.1 ms; added depth/coverage fetches and extra',
              'weather integrations at partial silhouettes need profiling before polishing.']
(out/'cost.txt').write_text('\n'.join(lines)+'\n')
print('\n'.join(lines))
sys.exit(3 if b-a > 0.1 else 0)
PY
phase=raw-cmp
cmp_failed=0
: > "$out/cmp.txt"
for case in no-weather no-occluder; do
  for arm in today fixed; do
    "$cache/bin/$arm" --case "$case" --px 6 --frames 1 --out "$out/captures/$case-$arm" > "$out/logs/$case-$arm.log" 2>&1
  done
  a="$out/captures/$case-today/frame.rgba"
  b="$out/captures/$case-fixed/frame.rgba"
  printf 'cmp %s %s\n' "$a" "$b" >> "$out/cmp.txt"
  if cmp "$a" "$b" >> "$out/cmp.txt" 2>&1; then
    printf 'stdout: (empty); exit=0\n' >> "$out/cmp.txt"
  else
    printf 'exit=1\n' >> "$out/cmp.txt"
    cmp_failed=1
  fi
done
test "$cmp_failed" = 0
phase=comparison-stills
for px in 6 12; do
  for case in animals grove; do
    for arm in today fixed; do
      "$cache/bin/$arm" --case "$case" --px "$px" --frames 1 --out "$out/captures/$case-px$px-$arm" > "$out/logs/$case-px$px-$arm.log" 2>&1
    done
    ffmpeg -y -loglevel error -i "$out/captures/$case-px$px-today/frame.png" -i "$out/captures/$case-px$px-fixed/frame.png" \
      -filter_complex '[0:v]drawtext=text=today:x=12:y=12:fontcolor=white:box=1:boxcolor=black[a];[1:v]drawtext=text=fixed:x=12:y=12:fontcolor=white:box=1:boxcolor=black[b];[a][b]hstack' \
      -frames:v 1 -threads 4 "$out/$case-px$px-today-vs-fixed.png"
  done
done
phase=live-motion
# Two identically founded seed-1 worlds, normal simulation speed, forced held fog.
# No saved-load controller ambiguity. One L3 and four sim workers per run.
for arm in today fixed; do
  scripts/run-study.py --slot 1 -- "$cache/bin/live-$arm" --seed 1 --px 6 --seconds 6 --fps 60 --speed 1 \
    --preview fixed:0.5,0,clear,0.8 --video "$cache/$arm.mp4" --log "$out/live-$arm.csv" > "$out/logs/live-$arm.log" 2>&1
  grep -Eq 'liveness animals_moved=[1-9][0-9]*/' "$out/logs/live-$arm.log"
done
ffmpeg -y -loglevel error -i "$cache/today.mp4" -i "$cache/fixed.mp4" \
  -filter_complex '[0:v]drawtext=text=today:x=12:y=12:fontcolor=white:box=1:boxcolor=black[a];[1:v]drawtext=text=fixed:x=12:y=12:fontcolor=white:box=1:boxcolor=black[b];[a][b]hstack' \
  -c:v libx264 -pix_fmt yuv444p -crf 12 -preset fast -threads 4 "$out/animals-walking-fog-today-vs-fixed.mp4"
phase=restored-source-check
git diff --exit-code -- crates/cubarium-gpu/src/voxel.rs crates/cubarium-gpu/src/plants.rs crates/cubarium-gpu/src/figures.rs \
  crates/cubarium-gpu/shaders > "$out/logs/restored-source.log"
printf 'still pairs: animals/grove at px 6 and 12; live motion 6 s, speed 1, 60 fps\n' > "$out/media.txt"
phase=complete
