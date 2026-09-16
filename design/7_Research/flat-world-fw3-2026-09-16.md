---
design_status: exploration
last_reviewed: 2026-09-16
---

# FW-3 result: the render crate by topology, and what the CPU levers actually buy

Written by Opus on 2026-09-16 against
[the FW-3 brief](../handoffs/flat-world-fw3-opus-render-2026-09-16.md),
[FW-1's freeze](flat-world-fw1-2026-09-16.md),
[the ring-world plan](../flat-world-plan-2026-09-16.md) §3 and §9, and
[FW-P's budget](presenter-budget-2026-09-16.md). Worktree
`.claude/worktrees/tachyon-screen`, branch `tachyon-screen`.

## Summary

1. `Canvas` is shaped by the topology and carries the world scale; it encodes a cube
   `Frame` as before and a `Raster` for a ring. `field`, `trail`, `sprite`, `body` and
   `multipart` read the shape from the canvas, so no stamp can be budgeted or unfolded
   against a different world than the one it draws on.
2. Both stamp budget check sites read `Scale::footprint_radius()`. A `scale = 2` stamp
   draws on a world at `S = 2` instead of vanishing.
3. **`R` on one A78 goes from 17.03 ms to 9.00 ms**, and on the desktop from 9.86 to
   5.22 ms, with the cube's output **bit-identical** — the same twenty frames of the real
   `ArtPresenter` hash to the same linear `f32` and the same encoded bytes before and
   after every commit.
4. The sRGB encode is **1.367 → 0.096 ms** on the board, not the 0.33 FW-P projected, and
   it is exact rather than within half a code: the table is a tabulation of the old
   function's own step points, verified against it on all 1,065,353,217 `f32` in `[0, 1]`.
5. **The row-band split gives 1.94×, not 3.5×**, on four A78s, and it saturates at three
   bands. The hook is not the limit; the presenter's per-slot work, which every band
   redoes, is. §6's load arithmetic needs rewriting with 1.94.

## 1. The API, one line each

Everything not listed is unchanged. `cubarium_render`:

```rust
// canvas.rs — shaped by the topology, carrying the scale
pub fn Canvas::new(topo: Topology, scale: Scale) -> Canvas;  // cube: 5x64x64; ring: one w*h
pub fn Canvas::cube() -> Canvas;                     // new(Topology::Cube, Scale::ONE)
pub fn Canvas::topology(&self) -> Topology;
pub fn Canvas::scale(&self) -> Scale;
pub fn Canvas::width(&self) -> u16;                  // chart width
pub fn Canvas::height(&self) -> u16;                 // chart height
pub fn Canvas::charts(&self) -> &'static [Face];
pub fn Canvas::rows_of(&self, face: Face) -> Range<u16>;   // the loop bound every pass uses
pub fn Canvas::owns(&self, face: Face, y: u16) -> bool;
pub fn Canvas::coords(&self) -> impl Iterator<Item = (Face, u16, u16)>;
pub fn Canvas::pixels(&self) -> &[[f32; 3]];         // the flat store; for a ring, the image
pub fn Canvas::pixels_mut(&mut self) -> &mut [[f32; 3]];
pub fn Canvas::encode(&self, frame: &mut Frame);     // unchanged, cube only
pub fn Canvas::encode_raster(&self, raster: &mut Raster);  // nearest, sRGB, single-chart only
pub fn Canvas::band(&self) -> Range<u32>;            // the global rows this canvas owns
pub fn Canvas::band_pixels<'a>(&self, &'a [PixelImage]) -> &'a [PixelImage];
pub fn Canvas::bands(&self, n: usize) -> Bands;
pub fn Canvas::for_each_band(&mut self, &mut Bands, impl FnMut(&mut Canvas));
pub fn Canvas::par_each_band(&mut self, &mut Bands, impl Fn(&mut Canvas) + Sync);
pub fn Canvas::split_into(&self, &mut Bands);        // for a caller-driven thread pool
pub fn Canvas::gather(&mut self, &Bands);
pub struct Bands;  // len, is_empty, iter, iter_mut

// cells.rs — W1
pub struct PixelCells;
pub fn PixelCells::new(topo: Topology, scale: Scale) -> PixelCells;
pub fn PixelCells::{topology, scale}(&self);
pub fn PixelCells::fits(&self, topo: Topology, scale: Scale) -> bool;
pub fn PixelCells::cell(&self, face: Face, x: u16, y: u16) -> CellId;
pub fn PixelCells::value(&self, &ScalarField, face: Face, x: u16, y: u16) -> f64;
pub fn PixelCells::filtered(&self, &ScalarField, face: Face, x: u16, y: u16) -> f64;

// unfolds.rs — W2
pub struct Unfolds;
pub fn Unfolds::{new, cached}() -> Unfolds;          // new = a plain scratch buffer
pub fn Unfolds::{set_caching, is_caching, clear}(..);
pub fn Unfolds::stats(&self) -> (u64, u64, usize, usize);  // hits, misses, anchors, pixels
pub fn Unfolds::pixels(&mut self, Topology, SurfacePoint, f64) -> &[PixelImage];

// field.rs, sprite.rs
pub fn draw_field_with(&mut Canvas, &PixelCells, &ScalarField, f64, [f32; 3], bool);
pub fn stamp_layers_cached(.., tone: Tone, unfolds: &mut Unfolds);
pub fn Sprite::from_rgba_at(scale: Scale, w, h, pivot, bytes) -> Result<Sprite, String>;
pub fn Sprite::from_premultiplied_at(scale: Scale, w, h, pivot, pixels) -> Result<Sprite, String>;
```

`crates/cubarium/src/present.rs` gains `draw_ramp_field_with(canvas, cells, ..)`.

**Nothing existing changed shape** except `Canvas::new`. Every stamp entry point keeps its
`scratch: &mut Vec<PixelImage>` parameter and its behaviour; `stamp_layers_cached` is the
one new one, because every other stamp is that one with a still pose, an identity bend or
a zero tone.

### Where the topology and the scale come from

From the **canvas**, not from a new parameter. A canvas *is* the world's raster, so
`draw_field`, `draw_trail`, `stamp_body`, every `stamp_layers*` and `stamp_rig*` read
`canvas.topology()` and `canvas.scale()`. That is why 250 call sites did not have to grow
two arguments, and — more to the point — why a stamp can never be unfolded on one topology
and composited on another, or budgeted against a scale the canvas does not have.

## 2. The budget at both check sites

`FOOTPRINT_RADIUS = 9.0` is gone.

* **Stamping** (`sprite.rs`, `stamp_bent` and `stamp_pose_in_chart`): the budget is
  `canvas.scale().footprint_radius()`. On a world at `S = 2` an 8×8 tile stamped at
  `scale = 2` reaches 10.06 px, which is inside 18 and draws; on the cube, where `S` is
  pinned to 1, it is over budget and still draws nothing.
* **Construction** (`Sprite::from_rgba`, `from_premultiplied`): a `Sprite` carries the
  budget of the scale it was authored for. `from_rgba` means "authored at `S = 1`" and is
  unchanged for every pack that exists; `from_rgba_at(scale, ..)` is for a pack baked at
  higher resolution, which is stamped at `scale = 1` and may reach `9·S`. `bend_headroom`
  is a bound in the *tile's* own pixels, so it uses the sprite's own budget and scales
  with the art rather than with the stamp.

FW-6's `ring_stamp_scale.rs::a_scale_two_stamp_draws_on_a_world_at_s2` passes, and FW-6
has removed its `#[ignore]` (`b146e67`).

## 3. Bit-identity: the golden

Captured **before the first commit**, from `crates/cubarium/examples/fw3_canvas_digest.rs`
(a scratch tool, not committed): seed 1, `assets/atelier`, the real `ArtPresenter`, twenty
frames at `f = 0 .. 0.95`, FNV-1a over every pixel's three linear `f32` **bit patterns**
and over the whole 61,440-byte encoded frame.

| world | linear | encoded |
|---|---|---|
| 3,000 ticks | `f24708a7806da988` | `aa872056423de435` |
| 600 ticks | `899ebc4458de5c5d` | `26564360d68a77dd` |

Both are unchanged after every commit in this package, including the full lever adoption.
They were also checked in an isolated copy of `ea4dd5c` with only this package's crates
overlaid, so the comparison is against the same core and the same world — not against a
tree FW-2 had meanwhile changed.

Alongside: `cargo test --workspace --exclude cubarium-gpu` is **1,655 passed, 0 failed,
25 ignored**. The exhaustive sRGB proof (`#[ignore]`d, 1.1e9 values) passes in 2.9 s.
`cargo clippy --workspace --exclude cubarium-gpu --all-targets` reports nothing in any file
this package touched; the one remaining `error`-level lint is `approx_constant` on
`Vec2::new(-0.7071, -0.7071)` in `crates/cubarium/tests/hunter_present.rs:1321`, which is
present at `ea4dd5c` and is not ours.

## 4. The measured table

Seed 1, 3,000 ticks, `assets/atelier`, population 24 — FW-0's and FW-P's world exactly.
Medians; `n = 300` on the desktop (Ryzen 9 9950X3D, one pinned core, a shared and busy
machine, so the runs were interleaved and repeated three times each), `n = 200` on the
board (Tachyon, one A78 at 2.4 GHz, `taskset -c 4-7 … --pin 4`).

### Before and after, end to end

| | desktop before | desktop after | **board before** | **board after** |
|---|---|---|---|---|
| `presenter.draw` | 9.39 | **5.19** | 15.65 | **8.91** |
| `canvas.encode` | 0.470 | **0.036** | 1.367 | **0.096** |
| `R = draw + encode` | 9.86 | **5.22** | **17.03** | **9.00** |

"Before" is `ea4dd5c` — the branch head before any of this package landed — built and run
under the same conditions. FW-0 recorded `R = 15.28` on the board for the same code; this
run measures 17.03 for it, on a board that had other work on it. The before/after pair is
the number to trust, not either half against FW-0's.

### Per lever

Each row is what **reverting that one lever** costs, measured against the all-levers build
in one isolated tree, so the rows are internally consistent but do not have to sum to the
total. Desktop `draw` medians.

| lever | where | desktop `draw` with it reverted | cost of reverting |
|---|---|---|---|
| **W1** pixel→cell table | `PixelCells`, 5 field passes | 8.32 | **3.02 ms** |
| **W2** footprint cache | `Unfolds`, 3 stamp passes | 6.31 | **1.01 ms** |
| **W2b** no per-stamp allocation | `unfold_pixels` | 6.36 (cache off) / 5.31 (cache on) | 0.05 / ~0 ms |
| **W5** dry pixel before the `exp` | `draw_water` | — | not measurable here (§6) |
| all of the above | | 9.49 (nothing adopted) | 4.19 ms |
| **W7** encode table | `Canvas::encode` | 0.477 (`encode`) | **0.440 ms** |

The board's own micro-benches (`presenter_budget` sections C, D, D2) agree and are the
cleanest statement of each lever:

| | board, recomputed | board, from the table/cache |
|---|---|---|
| pixel→cell, one field pass (5 lookups × 20,480 px) | 1.326 ms | **0.021 ms** |
| `unfold_pixels` for the 1,280 plant slots | 6.437 ms | **0.001 ms** |
| the sRGB encode of a whole frame | 1.367 ms (before) | **0.094 ms** |
| — FW-P's proposed 4,096-entry interpolated table | | 0.259 ms |

### Memory

`PixelCells` is five `u16` per pixel: 200 KiB for the cube, 2.3 MiB for a 640×360 ring.
The footprint cache holds the live footprints of the 1,280 plant slots and the 320 ground
anchors. Measured peak RSS of `render_bench`: **12.3 MiB → 45 MiB**. Almost all of that is
live footprints rather than dead space — the pool compacts itself once the abandoned runs
pass half of it, and compaction does not move the peak. A band-split presenter pays it per
band, so four bands is about 130 MiB of footprints. That is the price of W2 and it should
be stated out loud before FW-5 builds four presenters.

## 5. The band split

`Canvas::bands(n)` cuts the canvas into `n` contiguous runs of **global rows**
(`chart_index · height + y`), always in the same places for the same `(topology, n)`. Each
band is a `Canvas` that stores only its rows: `set` and `add` outside them are dropped,
`rows_of` gives a pass its own loop bound so a band walks only its own pixels, and
`band_pixels` narrows an unfolded stamp to the band with two partition points — which is
free, because `unfold_pixels` already emits its pixels in `(chart, y, x)` order, and that
*is* global row order.

**Why the composite is the serial image.** Every pixel belongs to exactly one band; every
band runs the same sequence of passes in the same order; every per-pixel operation
(`add`, source-over `set`) depends only on that pixel's own prior value and the stamp. So
each pixel is written the same number of times, in the same order, from the same value.
`split_into` seeds each band from the canvas first, so a pass that reads what is under it
sees what it would have seen serially — proven by drawing the same stamped scene twice
over four bands and comparing against two serial passes.

Tested in `crates/cubarium-render/tests/band_split.rs` for `N = 1, 2, 4, 5, 16`, serially
and on scoped threads, on the cube and on a 320×180 ring, with a stamped scene whose
anchors sit on every rim, at the seams, on the ring's wrap and deliberately **on and around
the band cuts** — and again through the footprint cache. FW-6's `ring_canvas.rs` proves the
same for a pass that writes pixels directly.

### What it costs, and what it buys

`presenter_budget` section (F), added for this: one image, `n` bands, one **presenter per
band** on its own core, driven through `split_into` / scoped threads / `gather`, with the
composite checked against the serial image before anything is timed.

| | serial | split | speedup | efficiency |
|---|---|---|---|---|
| desktop, 4 bands | 5.112 | **1.872** | **2.73×** | 68 % |
| board, 2 bands | 8.927 | 5.788 | 1.54× | 77 % |
| board, 3 bands | 8.927 | 4.658 | 1.92× | 64 % |
| **board, 4 A78s** | 8.938 | **4.598** | **1.94×** | 49 % |

Composite bit-identical in every row. For contrast, section (E) — four presenters drawing
four whole, independent images, which is the ceiling FW-P measured — reaches **4.03× on
the same four board cores**, so the hardware is not the limit and neither is the hook.

**The limit is the presenter.** Each band re-runs the *per-slot* work for all 1,280 cells:
the growth interpolation, `slot_wind`'s trigonometry, the stage and layer selection, the
tone. Only the per-pixel composite shrinks with the band. Fitting `F + P/N` to the board's
two- and four-band numbers puts the band-independent share `F` at about **3.4 of the
8.9 ms**, which is why three bands already reach the floor.

The fix is in the presenter, not the canvas: a band should reject a slot before building
its pose when the slot's footprint cannot reach the band. The sound way to do that is a
conservative per-anchor row range — the cached entry is a superset of any narrower query,
so its rows bound the stamp — but only if the entry is held at a radius the wind can never
exceed, which means querying each anchor once at `scale.footprint_radius()` and paying a
wider pixel walk in the serial case. **That trade is FW-5's to make**, and it is the single
largest remaining item in §6's arithmetic.

### §6's load arithmetic, redone

Against `20 · tick_ms + fps · 2.81 · S² · R ≤ 1000` with FW-0's `tick_ms = 0.534`, at
320×180, `S = 1`, 60 fps (the split applies to `draw`; `encode` stays serial):

| configuration | effective `R` | load | verdict |
|---|---|---|---|
| FW-0's baseline, no split | 17.03 | 2,882 ms | fails 2.9× |
| FW-3's levers, no split | 9.00 | 1,528 ms | fails 1.53× |
| FW-3's levers + the four-band split | 4.69 | **802 ms, 80 %** | **clears 60 fps** |
| *(what §6 assumed: levers + a 3.5× split)* | *(2.67)* | *(461 ms, 46 %)* | — |

So the recommendation still holds and 320×180 at 60 fps is reachable, but at **80 % of the
wall second rather than 52 %**, which leaves much less for the shim and the sink than §6
claims. FW-5's tick-rate background layer moves from "optional insurance" to "the thing
that buys the headroom back".

## 6. What §3, §9 and FW-P had wrong

1. **§3's `Canvas::new(topo)` needs the scale too.** FW-1 already said so for the surface
   crate; it is true here for a different reason — the stamp budget is `9·S` and the canvas
   is the only thing every stamp already has.
2. **§3's `pixels()` and the brief's `pixels()` are two different functions.** §3 wants
   "one new `Canvas::pixels() -> impl Iterator<Item=(Face,u16,u16)>`" to replace the triple
   loops; the FW-3 brief wants "`pixels()`/`pixels_mut()` for the ring", which reads as
   slice access to the image. Both exist, under names that cannot be confused:
   **`pixels()`/`pixels_mut()` are the flat `[f32; 3]` store** (what `encode_raster`, a PNG
   sink and a GPU upload want) and **`coords()` is the triple iterator**. FW-6's
   independent test file uses `coords()` for the iterator, which is the same reading.
3. **§3's "sRGB encode is shared and untouched" is half right.** It is shared between
   `encode` and `encode_raster`. It is not untouched: W7 replaced it — bit-identically, and
   the bit-identity is the reason it could be replaced at all.
4. **FW-P's W5 was already done.** `water_coverage` guards its `exp` behind `w > 0` before
   the call, so there was no `exp` on a dry pixel to save. The 0.776 ms FW-P attributes to
   "the dry walk" is `filtered_at` — five `cell_of` per pixel over all 20,480 — which is
   W1's, and W1 takes it. The dry test has been hoisted into `draw_water` anyway, where it
   is visible; in this world (1,205 of 1,280 cells wet) it fires on almost nothing, and in a
   dry world it now saves a function call rather than an exponential.
5. **FW-P's W7 estimate is pessimistic, and its proposed table is the wrong one.** A
   4,096-entry interpolated table is accurate to within half a code, which is *not*
   bit-identical and would have moved pixels on the cube; measured on the board it also
   costs **0.259 ms**. Tabulating the old function's own 255 step points instead — found by
   bisecting `f32` bit patterns against it — is exact by construction and costs **0.094 ms**.
   The lookup is one indexed load and one compare: a positive `f32` below 1 is ordered by
   its bit pattern, the closest two thresholds are 0.89 % apart, and a bucket 2⁻⁸ wide in
   relative terms therefore holds at most one step (asserted while the table is built).
6. **The plan's ×3.5 for the row-band split does not hold: 1.94× on four A78s**, saturating
   at three bands, for the reason in §5. §9 calls FW-3's hook "required"; it is, and it is
   deterministic and bit-identical, but it is worth 1.94 and not 3.5 until the presenter
   stops redoing its per-slot work in every band.
7. **FW-P's "~20 % of `draw` should simply not reappear on the ring" is still a
   prediction.** It is not tested here, and one half of it is now wrong in the other
   direction: with W1 adopted, the pixel→cell map costs one indexed load on *either*
   topology, so the ring saves nothing there. The ring's saving is in `unfold_pixels`'s
   general path — only the two wrap columns need it — and W2 has already taken most of that
   on the cube as well. Whatever is left is smaller than 20 %.
8. **§9's "W2 keyed by anchor and radius" cannot work as written.** The wind changes a bent
   stamp's radius every frame, so an (anchor, radius) key misses every time. It is keyed by
   the anchor alone, held at the widest radius that anchor has asked for, and narrowed per
   stamp by the same `distance <= r + GEOM_EPS` test `unfold_pixels` itself applies — which
   is sound because the answer at `R` is a superset of the answer at any `r ≤ R`, pixel for
   pixel including each pixel's owning unfolding.

## 7. Files touched outside `crates/cubarium-render/**`

| path | what | why |
|---|---|---|
| 41 files across `crates/cubarium/**` | `Canvas::new()` → `Canvas::cube()` | mechanical follow-through of the one changed signature (commit `854167e`) |
| `crates/cubarium-surface/src/raster.rs` | `unfold_pixels`' chart-image buffer is per-thread | **W2b lives here, not in the render crate.** No signature and no result changed; `cube_by_value`'s `unfold_pixels` digest is unchanged |
| `crates/cubarium/src/present.rs` | `draw_ramp_field_with` added | W1 needs a cached entry point for the producer ramp |
| `crates/cubarium/src/art_present/mod.rs` | `PixelCells` and `Unfolds` fields; five passes through the table, three through the cache | this is where the levers earn the milliseconds |
| `crates/cubarium/src/art_present/environment.rs` | `filtered_at`/`draw_water` take the table; W5 | same |
| `crates/cubarium/src/art_present/tests.rs` | one call qualified | `stamp_pose` is no longer imported in the parent module |
| `crates/cubarium/examples/presenter_budget.rs` | section (F) | the brief asks for the split measured, and (E) measures a different thing |
| `crates/cubarium/tests/hunter_present.rs` | `Canvas::cube()` only | mechanical |
| `crates/cubarium-render/tests/{ring_canvas,ring_stamp_scale}.rs` | `Canvas::new()` → `Canvas::cube()`, one word each | FW-6's reserved paths. Left in the working tree and **never staged by FW-3**; FW-6 has since committed and extended both files |

Not touched: `cubarium-core`, `cubarium-gpu`, `cubarium-search`, `cubarium-surface-oracle`,
`vendor/cube-proto`.

## 8. A process note

`crates/cubarium-render/src/**` was staged in the shared index when FW-6's commit
`9c59de7` ran, and that commit carries this package's whole first stage —
`Canvas` by topology, `encode_raster`, the bands, the sRGB table, and the ports of
`field`, `trail`, `sprite`, `body` and `multipart` — under FW-6's message. The content is
intact and the branch is correct; the attribution is not. This is the second instance of
the same failure on this branch (FW-1 recorded the first, `8d26cf3`). Every FW-3 commit
after that one used `git commit -m … -- <paths>` and was checked with `git show --stat`.

Two other workers' in-progress edits were live in the same files during this work: FW-2
ran `git checkout --` over three host files that held uncommitted FW-3 conversions, and the
host crate did not compile for about half an hour while `RenderView` grew its topology.
All FW-3 measurements were therefore taken in **isolated copies** built from `git archive`,
not from the shared working tree, and every number above is from a tree whose contents are
known.

## Commands

```
# desktop, before and after, interleaved
taskset -c 30 ./target/release/examples/render_bench \
    --art assets/atelier --ticks 3000 --frames 300 --tick-samples 5

# the levers and the split, desktop and board
./target/release/examples/presenter_budget \
    --art assets/atelier --ticks 3000 --frames 200 --pin 24 --cores 25,26,27,28
ssh root@tachyon-8968c731.local 'cd /root/cubarium-fw3 && taskset -c 4-7 \
    ./target/release/examples/presenter_budget --art assets/atelier --ticks 3000 \
    --frames 150 --pin 4 --cores 4,5,6,7'

# one A78
ssh root@tachyon-8968c731.local 'cd /root/cubarium-fw3 && taskset -c 4-7 \
    ./target/release/examples/render_bench --art assets/atelier --ticks 3000 \
    --frames 200 --tick-samples 5 --pin 4'

# the sRGB table against the curve it replaces, on every f32 in [0, 1]
cargo test --release -p cubarium-render --lib -- --ignored
```
