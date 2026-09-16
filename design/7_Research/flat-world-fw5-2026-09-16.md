---
design_status: exploration
last_reviewed: 2026-09-16
---

# FW-5 result: the CPU presenter draws a ring world

Written by Opus on 2026-09-16 against
[the FW-5 brief](../handoffs/flat-world-fw5-opus-presenter-2026-09-16.md),
[FW-3's render result](flat-world-fw3-2026-09-16.md),
[FW-2's core result](flat-world-fw2-2026-09-16.md), the
[ring-world plan](../flat-world-plan-2026-09-16.md) §5 and §5a,
`design/stratified-world.md` and `design/appearance.md`. Worktree
`.claude/worktrees/tachyon-screen`, branch `tachyon-screen`.

## Summary

1. The art presenter and the M2 presenter both draw a ring, at 320×180 and at
   640×360, and the cube's frames are **unchanged to the bit** — the same two
   digests before the first commit and after the last.
2. `canopy_top` is a **presenter** constant, not a `WorldConfig` field, and the
   reasons are in §3. `band_of_height` compares against it; on a cube it is 1.0,
   which *is* the old `h >= 1.0` rule, pinned cell by cell.
3. The cube's "radial plant" rule was written as **the Top face**. On a cube that
   is exactly **the canopy band**, and stating it as the band is what makes a ring's
   crowns keep their free headings, their radial reveal and their spin. Without it
   every crown in the world faces the same way; it is the most visible thing I
   changed after looking at the first capture.
4. **A cube-shaped loop survives in `cubarium-core` and it is visible in the
   picture.** `Fields::react`'s litter/remains fall (3f) and both propagule loops
   (3h) iterate `CellId::all(Topology::Cube, Scale::ONE)`. On a ring only the first
   1,280 cells shed downhill, so the cell row containing index 1,280 becomes a
   permanent detritus dam — a bright horizontal line straight across the panel.
   Measured in §5. It is FW-2's crate and I did not touch it.
5. §5's "tall columns per side face" needed one more decision than it names: a
   column's **segment cap** is a property of the band, not a constant. It is
   `foliage rows − 2` — 9 on the cube, unchanged; 21 on a 320×180 ring — with
   `TALL_STEP` rescaled with it, or a ring's columns stop halfway up a band twice
   as tall as the cube's.
6. The per-slot band rejection (brief item 7) is **not taken**, with a reason in
   §7 rather than a measurement: there is no band-split consumer in the host to
   measure it against.

Five commits, all `git commit -m … -- <paths>` and each checked with
`git show --stat`:

| commit | what |
|---|---|
| `f83f42a` | the M2 presenter reads its world from the canvas |
| `9d8d8ec` | the art presenter is laid out by the world's own raster |
| `85dc692` | the ring's presenter tests and the two committed captures |
| `a3b91bf` | the M1 fixtures are built from a topology |
| this | the report |

## 1. The shape of the change

Every function in `art_present` that named `Topology::Cube` and `Scale::ONE` is
now a method on one small value:

```rust
pub struct ArtGeometry { topology: Topology, scale: Scale, canopy_top: f64 }
pub const ArtGeometry::CUBE: ArtGeometry;   // canopy_top = 1.0
pub fn ArtGeometry::new(topology, scale) -> ArtGeometry;
pub fn ArtGeometry::with_canopy_top(self, f64) -> ArtGeometry;  // 0..=1, else refused
```

and **every existing free function is that method on `ArtGeometry::CUBE`**. That
choice is deliberate and it is the reason this package touched no test file
outside its own: `band_of`, `slot_of`, `up_of`, `tall_columns`, `soil_weight`,
`rain_marks`, `wind_at` and the other 25 are used 300-odd times across
`crates/cubarium/tests/**`, which is FW-6's ground, and changing their signatures
would have meant editing twenty files I do not own. `the_cube_s_canopy_is_still_
exactly_the_top_face` asserts the delegation cell by cell rather than leaving it
on trust.

`ArtPresenter::new(pack)` still builds a cube. `ArtPresenter::for_world(pack,
topology, scale)` names one, and `observe`/`draw` **fit** the caches to the view's
own world first, so `runner/mod.rs:481`'s `ArtPresenter::new(pack)` draws a ring
world correctly without FW-4 changing a line. A fit is a no-op whenever the world
already matches, which is every frame of every cube run.

The same for the M2 presenter, which had to keep `Presenter::new()` argument-free
because FW-6's `ring_present.rs` calls it that way: its two field caches are sized
in `draw` from the canvas.

### What the geometry decides

| piece | cube | ring |
|---|---|---|
| caches | 1,280 cells | `cell_count()` — 3,600 at 320×180, 14,400 at 640×360 `S=1` |
| `w_soil`, shimmer phase | two `LazyLock` statics of 5·4096 | a per-raster `PixelTables`, which the cube shares by `Arc` so two cube presenters cost one copy |
| bands | `h ≥ 1` ⇒ canopy | `h ≥ canopy_top`; `height = 1 − 2v/h` |
| radial plants | `Face::Top` | the canopy band (§2) |
| columns | `Face::ALL × cx`, cap 9 | one pass along `u`, cap `foliage rows − 2` |
| wind chart field | the seam-joining field with its calm belts | the constant `(−1, 0)` |
| wind phase | `0.5·(x+z)` of `embed` | the same, on the cylinder — continuous round the wrap by construction |
| rain | 4-pixel cells, sparkle on Top | `cell_pixels` cells, no sparkle (no level chart), clipped at the rims |
| ground lattice | `x ≡ y ≡ 4 (mod 8)` | `n = extent/8` points centred in the extent |

## 2. What the ring capture shows

`crates/cubarium/tests/golden/ring_320x180.png` and `ring_640x360.png`, seed 1,
tick 3,000, `f = 0`, `assets/atelier`. `README.md` beside them says what is in
them. Both are `S = 1`: FW-7 owns the S-scaled art, and §9's staging says
"FW-1..FW-6 can ship a ring world at S = 1 on pack v5", so 640×360 here is
**160×90 cells at S = 1** rather than the ladder's 80×45 at `S = 2`. A `S = 2`
capture was taken too and is described below.

**What reads.** The three bands of `design/stratified-world.md` are three places,
now as rows of one panel: 7 canopy rows of 45, 23 foliage, 15 soil. Crowns with
fruit along the top, stalks and tall columns through the middle, a litter wash
with glowcaps and standing water along the floor. The horizon is the same
per-pixel `w_soil` blend the cube uses, which on a ring is one straight line. The
wrap is invisible: roll either image by half its width and there is no seam, and
the sixteen columns either side of `u = 0` carry no systematic brightness
difference — residual mean −0.23, +0.14 and −0.08 across three seeds, against a
column-to-column standard deviation of 4.4 and a whole-image extreme of 17–20.

**What looks wrong, for Wrysk.** Six things, in the order I would look at them:

1. **A bright horizontal line across the panel, about a third of the way down.**
   This is the `cubarium-core` bug of §5, not a presentation choice. It is the
   first thing to fix and it is not mine.
2. **The moat.** The bottom rows hold standing water with reeds in it, brighter
   than anything else in the image. FW-2 §4 predicted exactly this and named
   `evap_floor` as the knob; `design/backlog.md` owns knobs, so nothing was
   tuned. Whether a bright waterline along the floor is right is a look, not a
   number.
3. **Every tall crown in the world lands on the same row.** Every column has the
   same integer segment count at a given tick, because every column's mean wood
   density is nearly the same in a world with 24 animals on 3,600 cells. The cube
   has the identical property and four separate charts to hide it in. If Wrysk
   wants a ragged skyline the knob is a hashed per-column height offset, which
   would move the cube's image and so was not taken unasked.
4. **The columns are slender.** A 16-pixel-wide column crossing a 92-pixel band
   reads thinner than the same column crossing the cube's 44-pixel band. This is
   the art's size against the world's, and it is FW-7's, not a bug.
5. **640×360 at `S = 2` is not ready to look at.** The same 3,600-cell world at
   twice the resolution draws the art at its authored 16 pixels, so the plants are
   half-size against the world and the 8-pixel cells read as blocks. That is the
   plan's **Stage A0** gap exactly: `stamp_layers*` would have to be called at
   `scale = 2` (FW-3 made the budget allow it) and every art-relative constant in
   `tall.rs` — the 4-pixel tile pitch, `TALL_JOIN`, `PLANT_REVEAL_PX`,
   `SOIL_SNAG_PX` — scaled with it. FW-7 owns those constants and this package
   deliberately did not start on them.
6. **The world is a wall of plants.** With 24 founders on 3,600 cells the ring is
   2.81× thinner ecologically than the cube (FW-2 §7 item 8) and yet the *picture*
   is denser, because the same slot rule runs over 2.81× the cells. If it reads
   as too busy the lever is `RANK_FULL`/`RANK_MID` — the share of slots allowed to
   reach stage 1 and 2 — which is already documented as review-tunable.

The M2 (non-art) presenter was captured on a ring too and is correct: floor,
producer ramp, detritus flecks, trails and disc bodies, with the same detritus
line at cell 1,280.

## 3. The `canopy_top` wiring, and why it is not in `WorldConfig`

The brief allows one `cubarium-core` config key "if it does not exist yet … say
so". **It does not exist, and I did not add it there.** It is
`art_present::CANOPY_TOP = 0.67`, carried by `ArtGeometry`, defaulted per
topology (1.0 on a cube), moved by `ArtPresenter::with_canopy_top(f)` and
validated to `0..=1` — a non-finite or out-of-range value is refused and the
default kept, pinned by `the_canopy_threshold_is_validated_and_refuses_nonsense`.

Three reasons, in order of weight:

1. **The world must not know where the canopy is.** `design/stratified-world.md`
   is explicit: "Nothing in the world reads `soil_top`; only the presenter does,
   to draw the horizon." `SOIL_TOP` — the threshold `canopy_top` is the twin of —
   lives in `art_present/habitat.rs` and nowhere else. Putting its twin in
   `WorldConfig` would put a presentation decision inside the simulation's
   serialized state for the first time.
2. **It would cost a schema bump for nothing.** `WorldConfig` is inside
   `WorldState` (`world/state.rs:22-28`), so a new field is `CONFIG_VERSION` 9 →
   10 and schema 17 → 18. That invalidates FW-2's frozen `v16` mirror comparison,
   whose `ConfigProjection` is defined as "`WorldConfig` minus exactly three
   fields" (plan §4), the pinned projection hash `10304345502826573087`, FW-6's
   `ring_schema17.rs`, and every recorded run. A presentation constant is not
   worth a schema refusal of every world on disk.
3. **It is topology-dependent, which a config field is bad at.** The value that
   keeps the cube's frames identical is 1.0 and the value the plan proposes for a
   ring is 0.67. One config field would have to be either wrong on one topology or
   defaulted per topology anyway — which is what `ArtGeometry::new` does, in the
   one place that knows.

**The visible effect of 0.67**, since §5 asks for it to be reviewed rather than
derived: on a ring `height = 1 − 2v/h`, so the canopy is the top **16.5 %** of the
rows — 7 of 45 at 320×180, 15 of 90 at 640×360 — and the foliage is the 50 %
between it and `SOIL_TOP`'s bottom 33.5 %. Lowering it to 0.4 doubles the canopy
to 14 rows; the test pins both.

## 4. The cube proof

The golden of FW-3 §3, reproduced with a scratch tool built to its description:
seed 1, `assets/atelier`, the real `ArtPresenter`, twenty frames at `f = 0 … 0.95`,
FNV-1a over every pixel's three linear `f32` bit patterns and over the whole
61,440-byte encoded frame.

| world | linear | encoded |
|---|---|---|
| 3,000 ticks | `272b0ea7806da988` | `1544a356423de435` |
| 600 ticks | `9aecb54458de5c5d` | `7fdd0c60d68a77dd` |

**Unchanged after every commit in this package** — checked after each of the five,
including after the geometry refactor, after the radial-by-band change and after
the tall segment cap.

These are not literally FW-3's published figures, and the difference is worth
recording because it is informative rather than alarming. FW-3 published
`f24708a7806da988` / `aa872056423de435` and `899ebc4458de5c5d` /
`26564360d68a77dd`. **All four of mine agree with all four of FW-3's in the low 40
bits and differ only above them.** FNV-1a's multiply carries information upward
only, so the low `k` bits of a digest depend only on the low `k` bits of the seed
and on the data; four independent 40-bit agreements over ~1.2 MB of pixels each is
2⁻¹⁶⁰ against coincidence. The content is FW-3's exactly; the two tools' initial
accumulators differ above bit 40. I pin my own pair rather than FW-3's because a
before/after comparison against a tool I ran myself is the evidence, and I could
not recover FW-3's uncommitted scratch file.

Alongside: `cargo test --workspace --exclude cubarium-gpu --no-fail-fast` is
**1,683 passed, 0 failed, 26 ignored**. FW-6's `tests/ring_present.rs` passes
including `the_presenter_draws_a_ring_world`, which is still `#[ignore]`d in
FW-6's file — I did not edit it, per the brief; `cargo test -p cubarium --test
ring_present -- --include-ignored` is 4/4. FW-6 should drop the attribute, exactly
as it did for FW-3's `ring_stamp_scale.rs`.

`cargo clippy --workspace --exclude cubarium-gpu --all-targets`: **no new lint in
any file this package touched**. The lints that do fire in those files
(`neg_cmp_op_on_partial_ord` at `environment.rs:313`, `tall.rs:387,489`,
`manual_range_contains` in `lanternjaw/envelopes.rs` and `raster.rs`) are all on
lines this package did not write. The two `error`-level lints in the workspace —
`tests/hunter_present.rs:1321` and `cubarium-core/src/neural/gru.rs:233` — are the
same two FW-3 and FW-2 recorded and are not ours.

## 5. The finding: `Fields::react` still walks the cube

**A ring world's litter does not drain past cell 1,280.**

`crates/cubarium-core/src/fields.rs` iterates `CellId::all(Topology::Cube,
Scale::ONE)` in three **production** loops:

| line | pass | effect on a ring |
|---|---|---|
| `602` | 3f, the downhill fall of litter and remains | only cells `0..1280` shed; everything above the row containing cell 1,280 drains *into* it and it never drains on |
| `637` | 3h, the propagule donor budget | only cells `0..1280` can donate |
| `664` | 3h, the propagule transfer | only cells `0..1280` can receive |

(`habitat.rs:618` and `fields.rs:1243` are the same spelling inside `#[cfg(test)]`
modules and are harmless.)

The dam is measurable and it is the brightest artefact in the picture. Per-cell-row
mean `D`, seed 1, `assets/atelier` defaults:

| world | cells | dam row | `D` above | **`D` at the dam** | `D` below |
|---|---|---|---|---|---|
| 320×180 `S=1` | 80 × 45 | **16** = 1280/80 | 0.311 | **1.397** | 0.403 |
| 256×180 `S=1` | 64 × 45 | **20** = 1280/64 | — | **1.653** | — |
| 320×360 `S=1` | 80 × 90 | **16** | 0.164 | **0.710** | 0.206 |

The row is always the one containing **cell index 1,280 = `CUBE_CELL_COUNT`**, at
whatever `cy` that lands on for the world's column count, and the *excess* is the
same ≈ 83 units of `D` in every case — because 1,280 cells shed into it in every
case. It is absent at tick 0 (`D` = 0.512 at row 16, smoothly between its
neighbours' 0.486 and 0.528) and grows monotonically: 0.520 at tick 20, 0.638 at
300, 1.397 at 3,000. `FieldGraph`'s ring `downhill` table is **correct** — checked
for all 3,600 cells against `(cx, cy+1)` with `None` at both rims, and the
neighbour degrees are 4 everywhere but the two rims — so this is the loop bound
and nothing else.

Beyond the dam, 3h means **plants can only spread in the first 1,280 cells of a
ring**, which at 320×180 is the top 16 rows of 45: the canopy and the upper
foliage. That is not visible as a line but it is a real distortion of the ring's
ecology, and it would silently invalidate any ring steady-state number measured
from here.

Both are `crates/cubarium-core/**`, FW-2's exclusive path. Not touched. The fix is
three identical one-line substitutions of the world's own `(topology, scale)`,
which `Fields::react` already has in scope via `graph.topology()`.

Reproduced with a scratch probe (`--w/--h/--scale/--seed/--ticks`, per-row field
means); the probe is deliberately not committed, and the numbers above are what it
printed.

## 6. What §5 and §5a proved wrong, or left unsaid

1. **§5's "tall columns / rigs: choose columns along `u`" is necessary but not
   sufficient.** It says nothing about how *tall* a column may be, and
   `TALL_MAX_SEGMENTS = 9` is not a constant of the art: it is "one tile per
   foliage cell, which puts the crown exactly at the rim cell's center" for a band
   that is eleven cells tall. A ring's foliage band is 23 cells at 320×180 and 45
   at 640×360. Left at 9, columns stop a quarter of the way up and the band has a
   large empty middle. The cap is now `foliage rows − 2` (9 on the cube, by
   construction) and `TALL_STEP` is rescaled by `9/N` so that a column of cells at
   `W_max` still reaches the top of its band, which is the property the constant
   was measured for. The cube's 0.08 is kept exactly.
2. **§5's "on the top face plants are radial" is a statement about a band, not a
   chart, and it has to be rewritten that way before it ports.** Three separate
   places keyed off the cube's Top face: the slot's free heading
   (`up_of(cell).is_none()`), the growth reveal mask (the same), and the
   spin-in-place wind response (`slot.at.face == Face::Top`). On a cube all three
   are `band_of(cell) == Band::Canopy`; on a ring only that spelling survives. The
   first capture, before I changed it, had every canopy crown at heading `(1, 0)`
   — a repeating stamp across the top of the panel.
3. **§5's "bodies clip only at the top and bottom rims" is right, and it also
   applies to the ground lattice, which §5 does not mention.** The 8-pixel ground
   tile lattice divides 64 exactly and does not divide 180. Anchoring it at
   `y ≡ 4 (mod 8)` leaves a bare 4-pixel strip along the bottom rim; appending a
   final row leaves a double-stamped one. It is now centred in the extent, which
   leaves two pixels bare at each rim and is exactly today's lattice wherever 8
   divides the extent. It tiles the wrap exactly whenever `w % 8 == 0`, which both
   rungs of the ladder satisfy.
4. **Three per-pixel hashes packed their key as `face << 16 | x << 8 | y`** (the
   water shimmer, the ground-tile phase, and by the same shape the tall-column
   seed's `face << 8 | cx`). Those are injective on a cube because `x`, `y < 64`
   and `cx < 16`; on a ring they collide the moment a coordinate passes 255. §5
   does not raise it and it would have been a silent, subtle repetition rather
   than a failure. The cube's packing is kept verbatim; a ring uses one that
   cannot alias.
5. **§5a's `wind_phase` range widens on a ring, and that is fine.** `0.5·(x+z)`
   of the cylinder is `0.5·r·(cos θ + sin θ)` with `r = w/(2π·32·S) = 1.5916` —
   **identical at both rungs of the ladder**, since `w` and `S` double together —
   so the phase runs to ±1.125 rather than the cube's ±1. It shifts each root's
   clock by at most `WIND_TRAVEL_SECONDS · 1.125`; nothing depends on the bound.
6. **The ring's `wind_chart` cannot be the cube's field.** The cube's calm belts at
   `a = ±1` exist so that a continuous circulation closes across the side/side and
   side/Top seams. A ring has one seam, which is a pure translation, and two open
   rims, so there is nothing to close against: the field is the constant
   `(−1, 0)` and the gust's shape is carried entirely by `wind_phase`, as on the
   cube. Reusing the cube's field would have put two calm stripes down a ring for
   no reason at all.
7. **§5's presenter row in §9 says "bodies" and stops.** A body's anchor also has
   to be *canonicalized* against the right chart: `present::interpolate` clamped
   every interpolated point to `FACE_EXTENT` and called `canonicalize(Cube)`, so a
   ring body past `u = 64` would have been pulled back into the cube's chart.
   `interpolate_on(topo, ..)` is the fix and `interpolate` is now it at
   `Topology::Cube`, bit for bit.
8. **`Presenter::new()` cannot take a topology.** FW-6's `ring_present.rs` builds
   one with no arguments and hands it a ring canvas, so the M2 presenter's caches
   have to be fitted at draw time from the canvas rather than at construction.
   Worth knowing before anyone reaches for a constructor argument there.

## 7. The per-slot band rejection (brief item 7): not taken

FW-3 §5 calls it "the single largest remaining item", and the arithmetic is right:
each band re-runs the per-slot work for every cell, and `F ≈ 3.4` of the board's
8.9 ms is band-independent. I did not take it, for two reasons rather than one
measurement:

1. **There is no band-split consumer to measure it against.** FW-4's run loop
   draws one presenter onto one whole canvas (`runner/mod.rs:676` builds a single
   `Canvas`, `Show::Art` holds a single `ArtPresenter`). In a single-band
   presenter `canvas.band()` is the whole image, so every slot passes the test and
   the rejection is pure overhead with a measured effect of zero. The measurement
   the brief asks for — "the measured split factor before/after" — does not exist
   until the host actually splits, which is FW-4's or a later package's call.
2. **The sound version has a cube cost.** FW-3 §5 spells out that a conservative
   per-anchor row range is only valid if the cached entry is held at a radius the
   wind can never exceed, i.e. querying every anchor once at
   `scale.footprint_radius()` and paying a wider pixel walk in the serial case.
   That is a cost on the cube's own path, taken blind, under a brief whose first
   constraint is that the cube's bytes do not move. Bit-identity would survive it
   — the wider answer is a superset narrowed by the same `distance <= r + GEOM_EPS`
   test — but the cost would not be zero and it would be paid for a consumer that
   does not exist.

The cheap half is worth recording for whoever does take it: the rejection needs no
new geometry. `ArtGeometry` already knows each slot's cell row, and on a ring a
slot's footprint spans at most `slot.at.v ± (footprint_radius + bend)` rows, which
is a comparison against `canvas.band()` and nothing more. It is the cube's
seam-crossing footprints that need the conservative cached range.

FW-4's addendum measures the ring presenter at **16.48 ms** (320×180) against the
cube's **6.25 ms**, which is 2.64× for 2.81× the cells: the cost is per-slot, as
FW-3 said, and a ring is not paying a topology penalty. Nothing in this package
changes that ratio.

## 8. The M1 fixtures, and FW-4's `--scene` refusal

FW-4 refused `demo --scene patch` and `--scene all` on a ring because `PatchScene`
built its `ScalarField` and its `FieldGraph` at the cube's 1,280 cells while the
substrate pass reads the canvas's cells. All three fixtures are now built from
`(topology, scale)`:

* the patch field and its cell graph have the world's own cell count, and its four
  deposit centres are chosen for the surface — the cube's seam, twisted seam,
  vertex region and rim-clipped footprint have no ring counterpart, so a ring gets
  the wrap, each rim and one in the open;
* the wandering body is transported by its own topology and starts at the middle
  of its chart, so it circles the wrap instead of walking off toward `Face::Right`;
* the ownership fixture stands a body either side of the wrap, one against each
  rim and one in the open, in place of four Top-face vertices that do not exist.

`Scenes::new` still builds the cube set; `Scenes::on(topology, scale, kind, seed)`
names a surface. Two tests cover it, and a capture of the whole set on a 320×180
ring shows the wrap body drawn on both edges of the panel from one stamp.

**FW-4's guard is left standing**, because lifting it is not the one-line removal
the coordinator asked me to gate on. It needs, all in FW-4's files:

* `run.rs:61-63` — `let (topology, scale) = demo.shape();` moved above the scene
  construction and `Scenes::new(kind, demo.seed)` → `Scenes::on(topology, scale,
  kind, demo.seed)`;
* `cli.rs:428-444` — the fifteen-line `anyhow::bail!` block deleted;
* `cli.rs:875-893` — the test that asserts the refusal rewritten to assert that
  all four scenes now parse on a ring.

## 9. Files touched

| path | what |
|---|---|
| `crates/cubarium/src/present.rs` | canvas-driven floor and ramp, `interpolate_on`, the M2 presenter's cache fit |
| `crates/cubarium/src/art_present/{mod,habitat,environment,tall,wind}.rs` | `ArtGeometry`, `PixelTables`, and every cube-named site through them |
| `crates/cubarium/src/art_present/tests.rs` | eight ring tests and the golden generator |
| `crates/cubarium/src/scene.rs` | the M1 fixtures by topology |
| `crates/cubarium/tests/golden/{ring_320x180.png,ring_640x360.png,README.md}` | the captures |

Not touched: `crates/cubarium/src/lanternjaw/**` — it names no topology and no
chart at all, and draws through `Canvas`, so the rig is correct on a ring without
an edit; `cubarium-core`, including the three loops of §5; and every FW-4 path.
No `canopy_top` field was added to `cubarium-core` (§3).

## Commands

```
# the cube golden, before the first commit and after every one
cargo run --release -p cubarium --example fw5_canvas_digest -- --ticks 3000   # scratch, uncommitted
cargo run --release -p cubarium --example fw5_canvas_digest -- --ticks 600

# the ring captures, and re-recording them
cargo test --release -p cubarium --lib -- --ignored ring_goldens
CUBARIUM_WRITE_GOLDEN=1 cargo test --release -p cubarium --lib -- --ignored ring_goldens

# through FW-4's sink, for a look rather than a golden
cargo run --release -p cubarium -- run --fresh --state /tmp/ring --topology ring:320x180 \
    --seed 1 --art assets/atelier --sink png --out /tmp/ring/cap \
    --speed 40 --seconds 150 --fps 4

# FW-6's file, including the test it still marks ignored
cargo test --release -p cubarium --test ring_present -- --include-ignored

cargo test --release --workspace --exclude cubarium-gpu --no-fail-fast
cargo clippy --workspace --exclude cubarium-gpu --all-targets
```
