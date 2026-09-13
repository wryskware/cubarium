---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Animation slice 2 — implementation brief (wind, rooted bend, growth pilot)

Fable's work order for the second animation slice, authorized by
`animation-slice2-handoff-2026-09-12.md` and directed by Astra's
`astra-animation-slice2-2026-09-12.md`. Evidence and a work order, not canon. Every
constant is a presentation choice, review-tunable from a viewing session; nothing here
touches the simulation, its 20 Hz tick, saved worlds, the shim, or the no-art image.

Integrator review during implementation: before acceptance, apply the corrections in
the handoff's **Integrator/Astra review while workers implement** section and the end of
`astra-animation-slice2-2026-09-12.md`. The original work order below is retained as
history; center-only footprint bounds, output-x radial masks, unchecked peak multipliers,
and hold-only flutter are not accepted as correct implementations.

## Invariants (restated for every worker)

- The working tree is the approved slice-1 checkpoint. Do not reset it, do not commit,
  do not touch `.vscode/`, the live preview at port 7396, original worlds or runners.
  Every write stays in this repository or `/tmp`.
- 20 Hz ecology, 60 fps render interpolation, presentation time only from
  `present_seconds(tick, f)`; `draw` stays pure (mutates nothing); pause holds the image;
  rewind snaps; no snapshot/schema change; fruit stays resource-gated.
- Reuse the shim/surface geometry (`unfold_pixels`, `SurfacePoint::embed`,
  `embed_tangent`, `travel`). Never hand-roll face wrapping. The nine-pixel per-stamp
  footprint cap is a hard bound and is **not** enlarged.
- Roots never skate. A tall column's base, trunk strips, cap and vine share one
  continuous deformation; row ownership (`Mask::Strip`) is unchanged; no seam gaps, no
  double opacity.
- Single source-over premultiplied blending (`stamp_layers`) is preserved; no analytic UI
  in normal display. Source scenes/SVGs are the art source of truth; the one-time Python
  authoring scripts must not overwrite hand edits. New pack versions keep old packs loading.
- Existing tests may adapt to changed appearance expectations but invariant tests are
  neither deleted nor weakened. Debug build stays warning-free; release keeps only the
  pre-existing `AUDIT_TOLERANCE` warning in core.

## Package A — wind sampler, rooted bend, species response (Opus 5, high)

Files: `crates/cubarium-render/src/sprite.rs` (+ `lib.rs` exports),
`crates/cubarium/src/art_present.rs`, `art/README.md`. Not `art.rs`, not `bake.gd`, not
the scenes (Package C owns those).

### A1. Renderer: `Bend`

```rust
/// A rooted horizontal displacement applied in the sprite's own (unscaled) tile
/// coordinates before sampling. Forward map `x' = x + D(H)`, `y' = y`; the renderer
/// samples the exact inverse `x = x' − D(H)`. Rows are preserved, so `Mask` coverage
/// is evaluated at the same (material) row and strip ownership is unchanged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bend {
    /// Displacement at and beyond `root + length`, in tile pixels along tile +x.
    pub amplitude: f64,
    /// Height of the tile's bottom edge above the plant's root line, in tile pixels
    /// (0 for a small plant; `4i − 8` for column tile `i`).
    pub base: f64,
    /// Heights `H ≤ root` are fixed (the painted root rows stay still).
    pub root: f64,
    /// Fixed mature bend length (not the current growth height).
    pub length: f64,
}
```

**Normative**: for a sample at tile-local `p` (pixels from the tile's upper-left, unscaled),
`H = (tile_height − p.y) + base`; `D(H) = amplitude · smoothstep(clamp((H − root) /
length, 0, 1))` with the Hermite `t²(3 − 2t)`, so value and slope are 0 at the root; a
non-finite or zero `amplitude`, or a non-positive `length`, is the identity, which takes
exactly the existing code path (bit-identical image, no extra cost). `Bend::NONE` is that
identity. The mixed sample of every layer is read at `(p.x − D, p.y)` in each layer's own
sprite (all layers share a tile size and pivot); the mask is evaluated at `p` (equal to
the material row). The unfold radius is `min(9, (max participating Pose::extent + |amplitude|)
· scale)`: the caller guarantees, through the headroom rule below, that every painted
texel displaced by its own `D` still lies inside 9 px of the anchor (bilinear support
included), so the cap never clips; a debug assertion may check `extent + |amplitude| ≤ 9`.

API: `stamp_layers_bent(canvas, anchor, heading, layers, scale, opacity, mask, bend,
scratch)`; `stamp_layers(..)` becomes `stamp_layers_bent(.., Bend::NONE, ..)` and stays
bit-identical. Plus `Sprite::bend_headroom(root, length, base) -> f64`: the largest
`|amplitude|` for which every painted texel of this sprite, displaced by its own `D`, keeps
`hypot(|x| + D, |y|) + FRAC_1_SQRT_2 + 0.5 ≤ 9` (x, y measured from the pivot; texels with
`D = 0` impose no bound; a sprite with no bendable texel returns `f64::INFINITY`). Root-row
invariance, identity, monotone displacement, and seam light conservation get unit tests in
the render crate.

### A2. Presenter: one shared breeze

Pure functions of presentation seconds and position; no state, no wall time, evaluated
**once per plant slot / column per frame**, never per pixel (the per-pixel work is only `D`).

- `pub fn wind_strength(seconds) -> f64` in `[0, 1]`: a packet every `WIND_PERIOD` (30 s):
  smoothstep ease-in over `WIND_RISE` (5 s), a hold of `WIND_HOLD` (8 s) carrying a mild
  flutter (`1 − WIND_FLUTTER + WIND_FLUTTER · 0.5 · (1 + sin(2π · seconds / WIND_FLUTTER_SECONDS))`,
  `WIND_FLUTTER` 0.3, period 2.3 s), smoothstep ease-out over `WIND_FALL` (5 s), then
  **exactly 0** for the rest of the period (a quiet interval of 12 s). A slow secondary
  modulation of the packet's peak (`WIND_PEAK_SECONDS` ≈ 97 s, ±15 %) breaks the metronome.
  Both edges have zero slope. Non-finite → 0. Constants `pub`, review-tunable.
- `pub fn wind_chart(face, u, v) -> Vec2` (unnormalized, Astra's seam-compatible field):
  `a = u/32 − 1`, `b = v/32 − 1`; side faces `W = (−(1 − a²), 0)`; Top `W = (−b(1 − a²),
  a(1 − b²))`. **Verify** against the actual tangent transport (`embed_tangent` or the
  `travel` tangent map) that the vector agrees on both sides of every side/top seam and is
  zero at side/side seams and Top's vertices; write that as a test. Do not seed per-face
  phases and do not normalize.
- `pub fn wind_at(point: SurfacePoint, seconds, lag) -> Vec2` = `wind_chart(point) ·
  wind_strength(seconds − lag − WIND_TRAVEL_SECONDS · phase(point))` where `phase` is a
  small spatial term from `point.embed()` (e.g. `0.5 · (x + z)` of the embedded position,
  `WIND_TRAVEL_SECONDS` ≈ 0.6) — no angular branch cut.
- Species response table (asset name → `WindResponse { tip_px, lag_seconds }`),
  `pub const`s: lanternstalk 0.45 / 0.10, tendrilfan 0.55 / 0.15, reedspire 0.70 / 0.05,
  glowcap 0.12 / 0.0, rootveil 0.0, spiretree 0.9 / 0.2, glasscane 0.5 / 0.1, vinecoil
  follows its host column. Canopy (top face) species rotate instead of bend:
  umbrellafrond 2°, bloomcrown 1.5° about the stationary center, angle `θ = deg ·
  strength_at_root · |W(point)| / max|W|`, no translation.
- **Headroom rule**: the effective tip amplitude of a family is `min(tip_px, headroom)`
  with `headroom = min over every frame of every clip of that plant (stages, fruit, and
  any growth/transition clip present) of Sprite::bend_headroom(PLANT_BEND_ROOT,
  PLANT_BEND_LENGTH, 0)`; a column's headroom is the minimum over base, trunk, cap (never
  crown) and vine frames with the column's `base` per tile and `TALL_BEND_LENGTH`.
  Compute once at presenter construction, store per plant/column, log or expose the table
  (`ArtPresenter::bend_budget(name)`), never vary it per frame. Measured on the shipped
  pack (Fable, 2026-09-12): lanternstalk 3.8, tendrilfan 0.55 (fruit clip), reedspire 5.1,
  glowcap 3.6, rootveil 7.7, spiretree 0.55 (cap), glasscane 0.62 (trunk top rows),
  vinecoil 0.62 — so spiretree lands at 0.55 not 0.9; record it. Optional if a column
  reads motionless: because the trunk is 4-periodic, a strip owning rows 12..16 of tile `i`
  can be painted from rows 4..8 of the same sprite anchored at index `i + 2` (identical
  texels, |y| ≤ 4 instead of 7.5); only with its own seam/registration proof.
- Per-slot amplitude variation ±10 % from the existing slot hash stream (append to the
  stream; do not reorder the consumed values); no per-plant phase offsets beyond the
  shared spatial term.
- Wiring: for a side-face plant `amplitude = eff_tip · dot(wind_at(slot.at, s, lag),
  slot.heading)` (projection onto the tile's horizontal axis, jitter kept), `Bend { base:
  0, root: PLANT_BEND_ROOT = 1.0, length: PLANT_BEND_LENGTH = 13.0 }`, applied to every
  layer of the stage/fruit stamp and to both the in-flight lower and upper stamps. For a
  column: one wind sample at the base anchor, `amplitude = eff · dot(W, tall_heading)`,
  `Bend { base: 4i − 8 (cap: 4(height + 1) − 8), root: 0, length: TALL_BEND_LENGTH = 48 }`
  for base, every trunk strip, the cap and every vine tile — one amplitude for all.
  Ground tiles, rain and bodies are untouched. `wind_strength = 0` ⇒ `Bend::NONE` ⇒ the
  slice-1 image bit for bit.
- Docs: `art/README.md` "Live world" gains a "Wind" paragraph (packet timing, calm
  intervals, species table, headroom rule, what does not move).

### A3. Verification owned by A

Existing render/host suites green (`cargo test -p cubarium-render -p cubarium
--all-targets`); own unit tests for A1/A2; the crowded release timings
(`plant_and_body_draw_cost`, `art_water::everything_on_draw_cost`) re-measured with wind at
peak (add a knob or a fixture time inside a packet) and reported; a short 4 s/60 fps
capture from a copy of `/tmp/cap-state-base/world-382800.cubw` via
`target/release/cubarium run --state <copy> --art assets/atelier --sink png --seconds 4
--every 1 --out /tmp/<dir>` is welcome but Fable owns the reviewed captures. Return: the
diff summary, the headroom table, the timings, and every deviation from this brief with
its reason.

## Package C — lanternstalk growth pilot (Opus 5, high; two steps)

C1 (art + bake + loader; may run alongside A): files `art/plants/lanternstalk.tscn`,
`art/bake.gd`, `crates/cubarium/src/art.rs`, `assets/atelier/*`, `art/PLANTS.md`.
C2 (presenter playback; after A and C1): `crates/cubarium/src/art_present.rs`.

### C1. Authored clip, pack v5, loader

- In `lanternstalk.tscn` author a **non-looping** 4 s clip `grow01` (stage 0 → stage 1) on a
  new group `Grow01` at the shared root `(0.5, 7)`: fixed root contact (lowest painted row
  stays tile row 14), the stem extends upward first — scale a stem part about its bottom
  (the negative center offset moves with the vertical scale so the lower edge stays at the
  root) — then the bulb appears/enlarges after its support is there, keeping bulb–stalk
  contact (no gap), and the final frame equals the neutral `Stalk1` image (rotation 0,
  modulate 1; check pixel-for-pixel after the bake). The first frame is the sprout at
  modulate 1. Add every new animated property's neutral value (visibility false, scale 1,
  position) to `RESET`, including the new group, so the bake's RESET-before-sample loop
  cannot leak growth transforms into the stage/fruit rows. Zero scale is singular (the
  baker skips it): use visibility/opacity for first appearance. Do not use the Python
  generators. Godot 4.7.2 (`/usr/bin/godot`, `./scripts/art-bake.sh`).
- `bake.gd`: a new `transitions` branch after the plant rows: for each plant scene, every
  animation named `grow<from><to>` becomes one atlas row in `plants.png` after that
  plant's stage/fruit rows, sampled **inclusively** at `i / (PLANT_FRAMES − 1)` and marked
  `loop: false`; `pack.json` `version: 5`, each transition row `{name, band, stage:
  "grow", from, to, row, frames, seconds, loop: false}` in the `plants` array so the row
  order stays plant-major. Stage/fruit rows are unchanged; the bake stays byte-reproducible
  (bake twice, compare) and every stage/fruit tile of the new pack equals v4's (assert with
  a script; record the result). Bake into `assets/atelier` only after the loader accepts v5.
- `art.rs`: accept `version` 1..=5; `Plant` gains `transitions: Vec<Transition { from: u8,
  to: u8, clip: Clip /* looping: false */ }>` and `Plant::transition(from, to) ->
  Option<&Clip>`; a v4 pack loads with none. The loader requires transition frames to pass
  the same extent check, `from + 1 == to`, and no duplicates. Extend the loader tests
  (expected list) and the pack docs in `art/PLANTS.md` ("Pack v5: growth transitions").

### C2. Presenter playback (after A and C1 land)

In the in-flight branch of `draw_with_fruit`, when `plant.transition(lower, upper)` exists
(side or top face alike): with `t = gu` (the upper stage's progress), edge `GROW_BLEND =
0.12`, weights `w_from = 1 − smoothstep(t / GROW_BLEND)` (0 past the edge), `w_to =
smoothstep((t − (1 − GROW_BLEND)) / GROW_BLEND)` (0 before it), `w_grow = 1 − w_from − w_to`;
one `stamp_layers_bent` with layers `[(lower stage's idle sway pose, w_from), (transition
clip .sample(t · clip.seconds), w_grow), (upper stage's idle sway pose, w_to)]`, no mask,
opacity `mix(opacity_of(lower) or 0, opacity_of(upper), t)`, the slot's wind bend applied
as to any stamp. Reversal reuses the same `t` (the clip runs backward); the clip is never
restarted by a frame or a wind packet; the fruit accent stays `0` in flight as now;
`None → 0` and every pair without a clip keep the reveal masks. Document normatively in the
doc comment and `art/README.md`. Own tests: endpoint continuity (t → 0 equals the idle
lower image, t → 1 the idle upper image, both within the blend), reversal symmetry, no
restart on repeated draws, v4-pack fallback identical to before.

## Package B — independent tests (Opus 5, high, after A; growth part after C2)

Written from doc comments only, into `crates/cubarium-render/tests/bend.rs` and
`crates/cubarium/tests/art_wind.rs` (+ `art_growth_clip.rs` after C2): zero-wind identity
(bit for bit against `stamp_layers` and against the presenter with `wind_strength = 0`),
root-row invariance for every shipped plant frame at ±headroom, seam light conservation
of a bent stamp across a side/side and side/top seam and at a vertex, column continuity
(a synthetic 4-periodic striped column at fractional heights and wind extrema: every row
composited once, no gap, cap/vine on the trunk's curve), wind field transport agreement on
both sides of every seam and exact zeros where claimed, temporal continuity (bounded
per-frame change at 60 fps through a packet's edges), exact-zero quiet intervals,
render-rate independence (30/60/120 fps histories at equal simulated instants draw equal
images), pause (`f` held ⇒ image held), the headroom sweep (every pose of every plant at
its ±budget with seam and vertex anchors: no clipped support, no vanished stamp), and the
pack v5 contract (v4 still loads; transition rows inclusive endpoints, nonlooping).

## Accepted corrections (Fable, 2026-09-12, after Astra's review of this brief)

These override the sections above; `astra_wind_regressions.rs` is an acceptance gate.

1. **Headroom bounds the whole bilinear support, with one quantity for admission and
   unfolding.** `Sprite::bend_headroom(root, length, base)` is the largest `A ≥ 0` such
   that for every painted texel at `(x, y)` from the pivot, with `s_max` the profile
   factor at the *highest row of its support* (`H_texel + 1`, since `D` grows with `H`),
   `hypot(|x| + A · s_max, |y|) + FRAC_1_SQRT_2 + 0.5 ≤ 9`. This is the established
   support convention applied to the worst displacement anywhere in the texel's support;
   texels with `s_max = 0` impose no bound. The renderer unfolds `min(9, (max pose extent
   + |amplitude|) · scale)` and, for tests only, exposes a `#[doc(hidden)]`
   `stamp_layers_bent_with_radius(.., radius)` so an acceptance sweep can prove that
   radius 9 and a larger radius draw the same image at every admitted amplitude (no
   clipped support). Never drop a stamp for an over-budget bend; clamp `|amplitude|` to
   the admitted family budget before stamping and count such clamps in a test.
2. **Masks in material coordinates.** Coverage is evaluated at `q = (p.x − D, p.y)`;
   identical for Axial/Strip, required for Radial.
3. **The family budget includes every multiplier.** Admitted family amplitude `=
   min(tip_px, headroom / (1 + WIND_SLOT_VARIATION))`; `wind_strength ≤ 1` exactly (peak
   modulation normalized into `[0.85, 1]`); `|W| ≤ 1` on every face; a column's headroom
   is evaluated with the cap at its highest continuous base (`4 · (TALL_MAX_SEGMENTS + 1)
   − 8`), the trunk and vine at their highest integer base, the base tile at `−8` — the
   profile is monotone in `H`, so the highest placement bounds every lower one (state
   this in the doc comment).
4. **One continuous packet.** `wind_strength = peak(t) · env(t) · mod(t)` with `env` the
   smoothstep rise / 1 / smoothstep fall (zero value and slope at both ends of the
   active interval, exactly 0 outside it), `mod = 1 − WIND_FLUTTER + WIND_FLUTTER · 0.5 ·
   (1 + sin(2π t / WIND_FLUTTER_SECONDS))` multiplied through the **whole** active
   interval, `peak ∈ [0.85, 1]` from the slow secondary period. Continuous with continuous
   slope at all four boundaries; `[0, 1]` everywhere.
5. Tests state whether they check the global sampler (`wind_strength`) or the delayed
   per-root sampler (`wind_at`, which shifts the quiet interval slightly by position).

## Fable's own deliverables

Captures at native 64×64 from (1) a scripted sparse fixture: 36 s at 60 fps sampled every
2nd frame through a quiet interval and one packet, small plants of each species, a column
with vine, a reed, the growth pilot; (2) the seed copy live snapshot, 4 s/60 fps every frame
and 12 s every 6th, compared to the checkpoint binary's with `/tmp/s2-jumps.py`; nearest-
neighbour enlargements; release timings for both crowded fixtures; the research record
`animation-slice2-2026-09-12.md`; the backlog update in `design/animation-roadmap.md`.
