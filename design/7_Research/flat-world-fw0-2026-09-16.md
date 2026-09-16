---
design_status: exploration
last_reviewed: 2026-09-16
---

# FW-0: the vendored raster format, and the four measured numbers

Brief: `design/handoffs/flat-world-fw0-opus-2026-09-16.md`. Worktree
`.claude/worktrees/tachyon-screen`, branch `tachyon-screen`. The measurement
table also lives in `design/flat-world-plan-2026-09-16.md` §6, "FW-0
measurements", which is where the gate is decided; this file adds the
evidence and the commands.

## 1. Vendor

`scripts/sync-cube-proto.sh /home/wrysk/vuzic/led-cube-shim/.claude/worktrees/tachyon-screen`

Vendored revision, in `vendor/cube-proto.rev`:
**`51bb7636d05753a86d54c5efc1f91200d218959a`** (the shim worktree's
`tachyon-screen` HEAD, clean; the script refuses a dirty `crates/cube-proto`
and had nothing to refuse).

Files changed: `src/{lib,wire,client}.rs`, `tests/wire_format.rs`, and two
new files `src/raster.rs` and `tests/raster_strip.rs`. +332 / −17.

`vendor/cube-proto/Cargo.toml` reviewed by hand against
`crates/cube-proto/Cargo.toml` in the shim and **left unchanged**: the source
gained no dependency and no feature with the raster work — still `serde`,
optional, and nothing else — so the vendored copy's only divergences remain
the two deliberate ones it already had, `edition = "2021"` and
`license = "MIT"` standing in for the shim workspace's inherited keys. No
source change was forced anywhere else in the workspace.

`cargo test --workspace --release`: **1,472 passed, 0 failed.**
`cargo test --release -p cubarium --test shim_sink`: 3 passed — cube frames
are byte-identical, which they are by construction since formats 0 and 1 are
untouched.

### The API note, for FW-3 and FW-4

Everything is at the crate root of `cube_proto`; nothing is behind a feature.

- **`Raster`** — `raster.rs`. A flat row-major RGB8 image, `x` right and `y`
  down, fields **private** so `data.len()` cannot drift from `w·h·3`.
  `Raster::black(w, h)` (panics off-range), `try_black(w, h) -> Option`,
  `width()`, `height()`, `size() -> (u16, u16)`, `row_bytes()`,
  `get(x, y) -> [u8; 3]`, `set(x, y, rgb)`, `as_bytes()`, `as_bytes_mut()`,
  `fill(rgb)`, `row(y) -> &[u8]`, `rows_mut(y0, rows) -> Option<&mut [u8]>`.
  `MAX_RASTER_DIM = 4096`; both sides must be `1..=4096`. **There is no
  constructor from an existing `Vec<u8>`** — a producer fills a `Raster` in
  place through `as_bytes_mut`/`rows_mut`/`set`, which is what FW-3's
  `Canvas::pixels()` should feed.
- **`encode_raster(raster: &Raster, seq: u32, max_datagram: usize, out: &mut Vec<Vec<u8>>) -> Result<(), ProtoError>`**
  — `wire.rs`. Clears `out` and appends one datagram per horizontal strip,
  each the largest whole number of rows that fits. **Every strip of one image
  carries the same `seq`**, which is how a receiver tells a torn frame from a
  new one. Fails only if one row cannot fit; at `MAX_DATAGRAM = 65_507` that
  is impossible.
- **`decode_strip(payload: &[u8]) -> Result<(Strip, &[u8]), ProtoError>`** —
  splits a decoded `Format::RasterStrip` payload into geometry and pixels.
- **`Strip { width, height, y0, rows }`**, all `u16` and all public;
  `pixel_bytes()`, `payload_len()`, `datagram_len()`, `validate()`.
  `width`/`height` are the **whole image's**, repeated in every strip, so a
  receiver can size its buffer from any single strip.
- **`Format::RasterStrip`**, wire code **2**, beside `FullFrame` (0) and
  `SingleFace` (1). `STRIP_HEADER_BYTES = 8` (four little-endian `u16`s)
  ahead of the pixels, after the existing `HEADER_BYTES` cube header.
  `Format::fixed_payload_len()` returns `None` for it.

So the names the brief guessed are the names that exist: `Raster`,
`encode_raster`, `decode_strip`.

## 2. Measure

Bench: `crates/cubarium/examples/render_bench.rs`. It times the run loop's
own two call sites — `presenter.draw(view, f, &mut canvas)` then
`canvas.encode(&mut frame)` (`runner/mod.rs:774-800`), and `World::step` —
on a world warmed to a populated steady state, not on tick 0 where there is
nothing to draw. `draw` and `encode` are reported apart as well as together;
the tick is reported both bare and as the non-headless loop pays it
(`step` + `render_view` + `observe` + `observe_hunters`), because §6's
`20 · tick_ms` term means the latter.

**The four-core render was not measured**, per the brief's mid-task
correction. A row-band split needs FW-3's hook — `Canvas` exposes no band and
`ArtPresenter::draw` takes `&mut self` — so anything measurable today would
be a benchmark-only arrangement the run loop will never execute. The plan's
four-core column reads "pending FW-3" and the S selection is provisional on
the serial numbers.

Common pinning: `WorldConfig::default()`, `seed = 1`, 3,000 headless ticks
(population 24), `--art assets/atelier`, 600 render samples, 200 tick
samples, `--pin <cpu>`, release profile, rustc 1.98.1 on both machines.

```
# device
ssh root@tachyon-8968c731.local
cd /root/cubarium && cargo build --release -p cubarium --example render_bench   # 1m53s
./target/release/examples/render_bench --art assets/atelier \
    --ticks 3000 --frames 600 --tick-samples 200 --pin 5
```

| quantity | pinning | median | p95 | min |
|---|---|---|---|---|
| **`R` = draw + encode, one cube frame** | **cpu5, A78 2.40 GHz** | **15.284 ms** | 15.557 | 14.808 |
| — `draw` | cpu5 | 13.897 | 14.179 | 13.399 |
| — `encode` | cpu5 | 1.367 | 1.466 | 1.364 |
| `R`, 12,000 ticks (population 65) | cpu5 | 13.518 | 13.797 | 13.081 |
| `R` | cpu7, A78 2.71 GHz | 14.123 | 14.216 | 14.026 |
| `R` | cpu0, A55 1.96 GHz | 74.002 | 75.002 | 73.311 |
| `R` | desktop 9950X3D, cpu0 | 8.760 | 8.851 | 8.719 |
| `R`, plain M2 presenter | cpu5 | 3.132 | 3.196 | 3.024 |
| **`tick_ms`, as the loop pays it** | **cpu5** | **0.534 ms** | 0.563 | 0.531 |
| — `World::step` alone | cpu5 | 0.316 | 0.348 | 0.314 |
| `tick_ms`, population 65 | cpu5 | 0.599 | 0.641 | 0.593 |
| `tick_ms`, desktop | 9950X3D cpu0 | 0.191 | 0.194 | 0.189 |
| **`R` across four A78 cores** | — | **pending FW-3** | | |

### The real runs

The display daemon `cube-screen-shim.service` was left running throughout and
is the receiver of the shim runs.

| run | result |
|---|---|
| `taskset -c 4-7 cubarium run --fresh --sink none --seconds 120 --state /root/bench-state/paced` | 2,400 ticks in 120.03 s = **19.99 ticks/s**, 1× real time |
| `taskset -c 5 cubarium run --fresh --sink none --speed 0 --seconds 600 --state …/unlimited` | 12,000 ticks in 4.11 s = **2,920 ticks/s**, 146× real time |
| `taskset -c 4-7 cubarium run --fresh --art assets/atelier --sink shim --addr 127.0.0.1:7392 --fps 60 --seconds 120 --state …/shim60` | 7,196 frames in 120.22 s = **59.86 fps**; 2,400 ticks = 19.96 ticks/s; "7196 sent, 0 coalesced, 0 errors"; **zero** `behind by` lines |
| `journalctl -u cube-screen-shim` during it | `presented 60.4 fps \| received 7196 (+300, 60.0 fps, 3.52 MiB/s) \| stale 0 (+0) \| bad 0 \| live \| render 8.1 ms avg, 11.3–16.3 ms max` |
| `top -b -n 3 -d 5` during it | `cubarium` 85.0 / 89.1 / 98.4 % of one core; board-wide 17–19 % us, 3–7 % sy, 75–79 % id |
| the same at `--fps 120 --seconds 30` | 1,934 frames in 30.22 s = **64.0 fps** — the loop's ceiling |
| desktop `--sink none --speed 0 --seconds 600` | 12,000 ticks in 1.47 s = 8,163 ticks/s, 409× real time |

The 120 fps run is the independent check on `R`. The loop saturates at
64.0 fps, so `R = (1000 − 20·0.534)/64.0 = 15.45 ms` — the bench's 15.28 ms
measured a second way, through the real runner, the real sink and the real
daemon. Both the number and the shared-loop budget equation are confirmed,
not assumed.

Nothing was skipped except the four-core render, which the brief removed.
`minifb` did **not** block the device build: it compiles on Ubuntu 20.04
headless because `x11-dl` and `wayland-sys` `dlopen` their libraries rather
than link them. A full release build of `-p cubarium --example render_bench`
from cold took **1m53s** on the board.

## 3. The gate

With `20 · tick_ms = 10.7 ms` the render budget is 988 ms per wall second,
so `R_max = 988 / (fps · 2.81 · S²)`. The full table is in plan §6.
Measured `R` is **15.284 ms**, and it clears exactly one cell:

**S = 1 (320×180) at `--fps 20`** — `10.7 + 20·2.81·15.284 = 869.7 ms`,
87 % of the wall second. The ceiling at S = 1 is `(1000 − 10.7)/42.95 =
23.0 fps`. This selection is **provisional on the serial numbers**; FW-3's
measured split is what reopens it.

What fails, and by how much:

| candidate | needs | has | over by |
|---|---|---|---|
| 640×360, S = 2, 60 fps (the recommendation) | `R ≤ 1.5 ms` | 15.28 ms | **10.4×** |
| 480×270, S = 1.5, 30 fps (the fallback) | `R ≤ 5.2 ms` | 15.28 ms | **3.0×** |
| 320×180, S = 1, 30 fps | `R ≤ 11.7 ms` | 15.28 ms | 1.3× |

Even a *perfect* four-core split at the plan's assumed ×3.5 gives an
effective 4.37 ms, which admits S = 1 at 60 fps and S = 1.5 at 30 fps but
still misses S = 2 at 30 fps by 1.5×.

### Two of §6's assumptions were wrong, in opposite directions

- **A78 : A55 is 4.84×** (74.002 / 15.284), not the assumed 2.5–3×.
- **`tick_ms` is 0.53 ms**, not the architecture doc's 20 ms target, so the
  tick costs 10.7 ms of the wall second rather than 400. The render budget is
  988 ms, not 600 — a 1.65× gift that does not close a 10× gap.

### Where the cost actually is

One A78 at 2.40 GHz is only **1.74× slower than a Zen 5 core** on this code
(15.284 vs 8.760 ms), so the same cube frame costs 8.8 ms on the fastest
desktop core available. `R` is what the presenter costs, not what the Tachyon
costs. Within `R`, `ArtPresenter::draw` is 13.90 ms of 15.28 and the sRGB
encode is 1.37; the plain M2 presenter draws the same frame in 3.13 ms. `R`
is also close to population-independent — 15.28 ms at 24 organisms, 13.52 ms
at 65 — which says the cost is in the per-cell background passes, and those
are exactly the term that multiplies by 2.81·S².

Three consequences, none of them decided here:

1. **FW-9 does not rescue this.** Its own scope keeps the presenter
   rasterizing the `w×h` raster on the CPU; it moves the *daemon's* gather,
   measured separately above at 8.1 ms avg and not part of cubarium's budget.
2. FW-3's row-band hook is now the difference between 20 and 60 fps at S = 1.
   It should be treated as required, not optional.
3. Reaching S = 2 needs `ArtPresenter::draw` to get roughly an order of
   magnitude cheaper, or to move to the GPU — which is the "longer-term
   shaded renderer", outside this plan.

## 4. A device note every later package needs

`taskset -c 7` and `taskset -c 4` fail with `EINVAL` on an idle Tachyon, and
it is not a permissions or cpuset problem: the root cpuset is `0-7` and PID 1
allows `0-7`. The board's `core_ctl` driver **isolates** idle big cores —
`/sys/devices/system/cpu/cpu7/isolate` reads `1` at idle — and the scheduler
refuses an affinity mask naming only isolated cores. `strace` shows
`sched_setaffinity(0, 8, [7]) = -1 EINVAL`. Loading the machine brings them
back within a second.

`render_bench --pin` therefore retries while briefly loading every core and
prints the mask it obtained, so a reported number can never be silently
unpinned. `taskset -c 4-7` always succeeds because cpu5 and cpu6 stay
un-isolated, and the full mask is retained, so the process picks up cpu4 and
cpu7 as its own load un-isolates them.

Also worth recording: the panel is **up**. `DP-1` was `disconnected` at the
time of the tachyon-screen probe; during this work the daemon reported
`presented 60.4 fps` continuously and drove real frames from the shim runs.

## Commands, in order

```
scripts/sync-cube-proto.sh /home/wrysk/vuzic/led-cube-shim/.claude/worktrees/tachyon-screen
cargo test --workspace --release                       # 1472 passed, 0 failed
cargo build --release -p cubarium --example render_bench
./target/release/examples/render_bench --art assets/atelier --ticks 3000 \
    --frames 600 --tick-samples 200 --pin 0 --label "desktop 9950X3D"

rsync -a --delete --exclude target/ --exclude .git --exclude state/ \
    ./ root@tachyon-8968c731.local:/root/cubarium/
ssh root@tachyon-8968c731.local
  cd /root/cubarium
  cargo build --release -p cubarium --example render_bench   # 1m53s
  cargo build --release -p cubarium --bin cubarium
  ./target/release/examples/render_bench --art assets/atelier --ticks 3000 \
      --frames 600 --tick-samples 200 --pin 5            # and --pin 7, --pin 0
  ./target/release/examples/render_bench --art assets/atelier --ticks 12000 \
      --frames 600 --tick-samples 200 --pin 5
  ./target/release/examples/render_bench --ticks 3000 --frames 600 \
      --tick-samples 200 --pin 5                         # plain presenter
  taskset -c 5   ./target/release/cubarium run --fresh --sink none --speed 0 \
      --seconds 600 --state /root/bench-state/unlimited
  taskset -c 4-7 ./target/release/cubarium run --fresh --sink none \
      --seconds 120 --state /root/bench-state/paced
  taskset -c 4-7 ./target/release/cubarium run --fresh --art assets/atelier \
      --sink shim --addr 127.0.0.1:7392 --fps 60 --seconds 120 \
      --state /root/bench-state/shim60
  top -b -n 3 -d 5 | grep -E '^%Cpu|cubarium'
  journalctl -u cube-screen-shim --since '3 minutes ago' --no-pager
  taskset -c 4-7 ./target/release/cubarium run --fresh --art assets/atelier \
      --sink shim --addr 127.0.0.1:7392 --fps 120 --seconds 30 \
      --state /root/bench-state/shim120
```

Scratch state on the device is under `/root/bench-state/`; the daemon,
`/etc`, sysfs and debugfs were not written to.
