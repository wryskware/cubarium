---
design_status: exploration
last_reviewed: 2026-09-16
---

# FW-P: where `ArtPresenter::draw`'s 13.9 ms goes, and what each lever buys

Follows FW-0 (`design/7_Research/flat-world-fw0-2026-09-16.md`,
`design/flat-world-plan-2026-09-16.md` §6). Worktree
`.claude/worktrees/tachyon-screen`. Everything here is measured on the desktop
(Ryzen 9 9950X3D, one pinned core) and converted to the board with the factor
FW-0's own two runs imply for `draw` alone: **13.897 / 8.274 = 1.680**.

## 1. Method

Two independent instruments, because the first alone would be a guess.

* **`perf record --call-graph fp`** on a `-C force-frame-pointers=yes` build
  (which costs 0.6 %: `draw` 8.326 vs 8.274 ms, so the profile is of the same
  program). Samples are folded and attributed to the **source line inside
  `draw_with_fruit`** that the sample's stack passes through, so every pass is a
  line range in `art_present/mod.rs` rather than a symbol guess. 21,071 samples.
* **`crates/cubarium/examples/presenter_budget.rs`** (new; `render_bench.rs` is
  untouched). Ablations against a doctored `RenderView` — `draw` re-reads the
  view every frame, so each difference *is* a pass's cost — plus micro-benches
  for the cache, geometry, unfold, encode and core-scaling levers.

The two agree where they overlap: the water pass is 15.0 % by profile and
1.240 ms of 8.325 ms = 14.9 % by ablation; bodies are 1.7 % and 0.145 ms = 1.7 %.

## 2. The per-pass table

Population 24, 3,000 ticks, `assets/atelier`, seed 1 — FW-0's world exactly.

| # | pass | `mod.rs` | % of `draw` | desktop ms | **board ms** | depends on | slowest input |
|---|---|---|---|---|---|---|---|
| 1 | `clear` + `draw_floor` | 1024–25 | 0.1 | 0.01 | 0.01 | nothing | static |
| 2 | producer ramp (+ horizon fade) | 1027–43 | 11.0 | 0.91 | **1.53** | `view.producer` | **tick** |
| 3 | detritus flecks | 1045–62 | 1.8 | 0.15 | 0.26 | detritus + carrion | **tick** |
| 4 | soil ground | 1064–67 | 3.3 | 0.27 | 0.46 | detritus + carrion | **tick** |
| 5 | ground cover (320 stamps) | 1069–1101 | 5.5 | 0.45 | 0.76 | density (tick) + tile breath | frame, 2.5 s loop |
| 6 | water | 1103–08 | 15.0 | 1.24 | **2.08** | depth + producer (tick) + shimmer | frame, 2.5 s loop |
| 7 | plants (1,280 cells) | 1110–1340 | **52.5** | 4.34 | **7.29** | growth (tick) + sway clip + bend | frame |
| 8 | soil snag | 1342–60 | 0.01 | — | — | dead soil stands (none here) | tick |
| 9 | tall columns | 1361–98 | 9.1 | 0.75 | **1.26** | height (tick) + sway | frame |
| 10 | rain | 1400–03 | 0.03 | 0.00 | 0.00 | rain field + fall phase | frame |
| 11 | bodies | 1405–30 | 1.7 | 0.14 | 0.23 | organisms, `f` | frame |
| 12 | retained prey, hunters | 1432–98 | 0.0 | — | — | organisms, `f` | frame |
|  | **total** | | 100 | **8.27** | **13.90** | | |

Ablations, `presenter_budget` (desktop, baseline 8.325 ms):

| ablation | Δ | reading |
|---|---|---|
| water field emptied | **−1.240 ms** | the whole water pass |
| water depths zeroed, field kept | −0.465 ms | of which the *wet-pixel* work; the remaining **0.776 ms is the dry walk** over all 20,480 pixels |
| organisms removed | −0.145 ms | 24 bodies |
| rain forced on **all 1,280 cells** | **+0.058 ms** | rain is not a cost at any rate |

**Rain is a non-issue** — the profiled world has 45 raining cells and rain is
0.03 % of `draw`; even a world-wide downpour adds 0.7 %. **Population is not a
cost either** (FW-0 already saw this: 15.28 ms at 24, 13.52 at 65). 98 % of the
frame is background.

## 3. Waste inside the hot passes

| # | finding | file:line | cost (% of `draw` / board ms) |
|---|---|---|---|
| W1 | **pixel→cell geometry recomputed every frame, in four passes.** `cell_of(&SurfacePoint::pixel_center(..))` for the pixel *and its four neighbours*, per pixel, per pass | `present.rs:174,180`; `cubarium-render/src/field.rs:20,28`; `environment.rs:190,194`; `environment.rs:221` | `cell_of` 8.0 + `pixel_center` 2.2 + `pixel_neighbor` 1.6 = **11.8 % / 1.64 ms** |
| W2 | **`unfold_pixels` recomputed per stamp from anchors that never move.** `slot_of(cell).at` and the ground lattice are fixed for the presenter's life | `cubarium-render/src/sprite.rs:879` | **14.4 % / 2.00 ms** |
| W2a | — of which the **seam path**: 560 of 1,280 plant slots (44 %) miss the in-chart test and take `unfold_pixels_general`, at 4× the cost (10.04 % vs 2.58 % of program) | `cubarium-surface/src/raster.rs:46-53` | |
| W2b | — `let mut images = Vec::new();` — **one heap allocation per seam stamp**, ~560 per frame | `raster.rs:50` | |
| W2c | — and a `sort_by` over the same pixel list every frame | `raster.rs:129` | 1.9 % |
| W3 | **`Sprite::sample`**: four bounds-checked `pixel()` reads, a `floor`, and a 4-channel accumulate per covered pixel per layer. With `bend = 0` and `scale = 1` the four texel indices and the four bilinear weights are **per-slot constants** (`sprite.rs:881-883` derives them from fixed anchors) | `sprite.rs:284-310`, `sprite.rs:879-886` | **30.6 % / 4.25 ms** |
| W4 | **`floor` is an out-of-line libm call** in the stock desktop build (7.6 % of program). `-C target-cpu=native` takes `draw` from 8.274 to **6.585 ms, −20.4 %** | build flags | desktop-only for `floor` (aarch64 has `frintm` in the baseline); `-C target-cpu=cortex-a78` is one build worth trying on the board |
| W5 | **`draw_water` calls `water_coverage` (an `exp`) before rejecting a dry pixel**, over all 20,480 | `environment.rs:215-219` | 0.776 ms board; ~0 here (1,206/1,280 cells wet) but pure waste in a dry world |
| W6 | per-frame field copies of tick-rate data: `copy_field`, `litter.extend`, `threshold_field` | `mod.rs:1032,1050-52,1066` | 0.2 %, but all of it is 20 Hz work in the 60 Hz path |
| W7 | **`srgb_encode` widens to `f64` and calls `powf` per channel** — outside `draw`, inside `R` | `cubarium-render/src/canvas.rs:75` | encode 1.37 ms board; a 4,096-entry interpolated table measures **0.464 → 0.111 ms**, i.e. **1.37 → 0.33 ms** on the board |

**W1 and W2 are largely a *cube* cost.** `cell_of` walks chart math and
`unfold_pixels` exists because the five faces meet at seams. On FW-1's cylinder
the pixel→cell map is integer division and only the two wrap columns need a
general unfold. So ~20 % of `draw` should simply not reappear on the ring, and
the plan's `2.81 · S²` is conservative at S = 1. **This is a prediction about
FW-1's raster, not a measurement** — FW-3 should check it before spending it.

## 4. The four levers

**(1) Caching slowly changing layers.** Compositing a cached layer costs
`Canvas::clone_from` = **0.002 ms** (measured), so the saving is the pass cost
minus amortisation. Passes 1–4 (16.1 %) depend on **nothing but tick-rate
fields** and can be cached with no visual change whatever. Passes 5 and 6 add a
2.5 s ground breath and a 2.5 s, 8 %-amplitude water shimmer; sampling those at
20 Hz instead of 60 is not a visible difference, which brings the cacheable
prefix to **36.6 % of `draw`** — and a cached pass costs **one third** of what
it costs today, so the saving is two thirds of the prefix at whatever it costs
by then (5.09 ms board today, 3.30 ms once W1 has run). Passes 7 and 9 (61.6 %)
are tick-rate in *growth* but frame-rate in *sway*; quantising the sway to 20 Hz
(bend budgets are 0.3–1.3 px, so the step is sub-tenth-pixel) would move them
into the layer too, on the same two-thirds terms. That is a presentation
decision, not a performance one. Genuinely per-frame: rain, bodies, hunters,
retained prey — **1.7 % of `draw`**.

One caveat FW-5 must design around: a layer rebuilt on the tick frame makes that
frame cost `B + F` and the other two `F` — in the S = 2 configuration below that
tick frame is ~34 ms against a 16.7 ms period, two deadlines missed. The rebuild
has to be **pipelined**: build tick N+1's layer in thirds across tick N's three
frames (~11 ms + 2 ms of foreground per frame, which fits), or on a spare core.
Otherwise caching buys throughput and sells a 3:1 sawtooth.

**(2) Algorithmic waste.** W1 + W2 + W7 are mechanical, change no pixel, and are
worth **1.64 + 2.00 + 1.04 = 4.68 ms** of the board's 15.28 ms `R`. W3's
still-pose fast path is the largest single item left (4.25 ms) but is an
**estimate**: a third of it, −1.4 ms, is the figure used below and it is the one
number in this report that has not been measured.

**(3) Row-band parallelism.** Measured ceiling: four *independent* presenters,
each with its own art pack and canvas, on four pinned cores — a harsher working
set than a row split, which shares one pack — reach **3.73–3.89× aggregate
throughput (93–97 % efficiency)** across two runs. The work is compute-bound
(`Sprite::sample` dominates and the canvas is 245 KiB, L2-resident), which is
why efficiency is that high. Per-frame fixed costs are negligible:
`Canvas::clear` 0.001 ms, `clone_from` 0.002 ms. The plan's **×3.5** for four
A78s sharing one L3 and one memory controller is a fair, slightly conservative
number and I would keep it. Interaction worth naming: a band owner must know
which of a stamp's unfolded pixels are its own, so **W2's per-slot unfold cache
should be bucketed by band** — that makes the split cheap instead of making
every thread redo every stamp's geometry.

**(4) GPU.** Wrysk has since chosen the full GPU stack (`f937be2`), so this is
no longer a question but a work list; the table in §2 **is** the shader list,
and passes 2–6 (36.6 %) collapse into one full-screen fragment shader over the
scalar fields. Per-*tick* upload: four `f32` fields + growth stages + tall
heights + rain ≈ 60 KB at ring scale. Per-*frame* upload: organism transforms
only (24–65 × ~40 B) and `seconds`. Passes 5, 7, 9, 10, 11 are instanced quads
over a sprite atlas with per-instance (anchor, heading, stage pair, mix,
opacity, bend, tone) — the bend is one x-displacement in the shader
(`sprite.rs` already computes it as two multiplies and a clamp). The pixel→cell
map and `SOIL_WEIGHT` are static textures. **The sRGB encode becomes free**
(`*_SRGB` framebuffer format): W7's 1.37 ms disappears rather than shrinking.
The CPU rasterizer stays for the LED cube and the byte-exact tests, so levers
1–3 still earn their keep on that path.

## 5. Recommendation

Board milliseconds for one *cube* frame; the ring multiplies by `2.81 · S²`.

Each row keeps the one above it. W1 is distributed over passes 2, 3, 4, 6 and W2
over passes 5, 7, 9 in proportion to their measured cost, so the caching rows
discount the already-cheapened passes rather than the original ones.

| stage | `draw` | `encode` | `R` | owner |
|---|---|---|---|---|
| today (FW-0) | 13.90 | 1.37 | **15.28** | — |
| + W1 pixel→cell table, W2 unfold cache | 10.25 | 1.37 | 11.62 | **FW-3** |
| + W7 encode table | 10.25 | 0.33 | **10.58** | **FW-3** |
| + passes 1–6 at the tick rate | 8.05 | 0.33 | **8.38** | **FW-5** |
| + passes 7 & 9 at the tick rate *(presentation call)* | 3.58 | 0.33 | **3.91** | **FW-5** |
| + W3 still-pose sample fast path *(estimate, not measured)* | 3.16 | 0.33 | 3.49 | FW-3 |

Against `20 · 0.534 + fps · 2.81 · S² · R ≤ 1000` (`R` divided by 3.5 where the
row-band split is used):

| target | needs `R` | reached by | effective `R` | load |
|---|---|---|---|---|
| **320×180, S = 1, 60 fps** | ≤ 5.86 ms | **FW-3's row-band split alone** (15.28 / 3.5) | 4.37 | 747 ms, **75 %** |
| 320×180, S = 1, 60 fps | ≤ 5.86 ms | **recommended:** W1+W2+W7, then the split (10.58 / 3.5) | 3.02 | 520 ms, **52 %** |
| 320×180, S = 1, 60 fps, **no split at all** | ≤ 5.86 ms | W1+W2+W7 + both caching rows | 3.91 | 670 ms, 67 % |
| **640×360, S = 2, 60 fps** | ≤ 1.46 ms | W1+W2+W7 + tick-rate background **including plants** + split (3.91 / 3.5) | **1.12** | 767 ms, **77 %** |
| 640×360, S = 2, 60 fps | ≤ 1.46 ms | the same, **without** quantising plant sway (8.38 / 3.5) | 2.39 | 1,624 ms — **fails 1.64×** |
| 640×360, S = 2, 60 fps | ≤ 1.46 ms | …and even with W3 added but the sway still at 60 Hz | 2.02 | 1,375 ms — **fails 1.39×** |

**The answers.** *320×180 at 60 fps is already reachable with one package:*
FW-3's row-band split, which FW-0 called required. Adding W1, W2 and W7 — none
of which changes a pixel — takes the load from 75 % to 52 % and is the
configuration I recommend, because 75 % of a wall second leaves nothing for the
shim, the sink or a bad frame. Caching then becomes optional insurance rather
than a dependency, and S = 1 clears 60 fps **even with no split at all** (67 %),
which is a useful fallback if the deterministic hook slips.

*640×360 at 60 fps is reachable on the CPU, but only if the plant and tall sway
runs at the tick rate.* That is the whole gap, and it is not close: with the
sway at 60 Hz every other lever stacked — including the unmeasured W3 — still
misses by 1.4–1.6×; with the sway at 20 Hz the same stack lands at 77 %. It is a
cheap-looking visual concession (sub-tenth-pixel steps on a 0.3–1.3 px bend) but
it is Wrysk's call, not FW-3's. If it is refused, S = 2 at 60 fps belongs to the
GPU stack, where the question does not arise. Two further discounts are in hand
and deliberately not spent above: W3's 1.30 ms and the ~20 % of `draw` that is
cube-seam and cube-chart work the ring should not have.

**Ownership.** **FW-3 (render crate):** the pixel→cell + neighbour table (W1),
the per-slot unfold cache bucketed by band (W2), the table-driven
`Canvas::encode` (W7), the row-band hook and its thread pool, and one
`-C target-cpu=cortex-a78` build on the board to price W4. **FW-5 (presenter):**
the tick-rate background layer, its pipelined rebuild, and the decision about
which passes may run at 20 Hz. **FW-9 / GS-0 (GPU):** §2 as the shader list,
§4(4) as the upload budget.

## Commands

```
cargo build --release -p cubarium --example presenter_budget
./target/release/examples/presenter_budget --art assets/atelier \
    --ticks 3000 --frames 400 --pin 0 --cores 0,1,2,3

CARGO_TARGET_DIR=/tmp/fp RUSTFLAGS="-C force-frame-pointers=yes" \
    cargo build --release -p cubarium --example render_bench
perf record -F 1500 --call-graph fp -o fp.data -- /tmp/fp/release/examples/render_bench \
    --art assets/atelier --ticks 3000 --frames 1500 --tick-samples 10
perf script -i fp.data -F comm,pid,time,period,event,ip,sym,srcline   # folded by mod.rs line

CARGO_TARGET_DIR=/tmp/native RUSTFLAGS="-C target-cpu=native" \
    cargo build --release -p cubarium --example render_bench          # W4
```
