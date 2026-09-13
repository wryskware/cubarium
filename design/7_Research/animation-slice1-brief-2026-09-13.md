---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Animation slice 1 — implementation brief

Fable's working brief for the first animation slice authorized in
`animation-implementation-handoff-2026-09-13.md`, informed by
`astra-animation-plan-2026-09-13.md`. Evidence and a work order, not canon. Every
constant below is a presentation choice, review-tunable from a viewing session; none of
it touches the simulation, its tick rate, or saved worlds.

## What is already landed (do not redo)

- `cubarium-render`: `Pose { first, second, mix }`, `Mask::{None, Axial{reveal},
  Radial{reveal}}`, `stamp_pose(canvas, anchor, heading, pose, scale, opacity, mask,
  scratch)`. One unfold, one source-over per pixel, premultiplied linear lerp between
  the two samples, coverage mask in tile pixels. `stamp_sprite` is now literally
  `stamp_pose(Pose::still(s), Mask::None)`. `Sprite::subtract(other, rows)`,
  `Sprite::width/height`. Normative semantics are in the doc comments of
  `crates/cubarium-render/src/sprite.rs`.
- `crates/cubarium/src/art.rs`: `Clip::sample(seconds) -> Pose` (normative doc comment;
  looping wraps last→first, non-looping clamps to the last sample), pack **v4** (creature
  and plant sample counts are data: `frames`, `plant_frames`, both in
  `MIN_FRAMES..=MAX_FRAMES` = 2..=32; v1–v3 packs still load), `TallPlant::cap` (crown
  minus the trunk pattern it duplicates over `CROWN_CAP_ROWS` = 4..16),
  `ArtPack::creature_frames()`, `ArtPack::plant_frames()`.
- `art/bake.gd`: `FRAMES = 16` (creatures), `PLANT_FRAMES = 24` (plants and tall
  plants), `version: 4`. `assets/atelier` is rebaked (byte-reproducible; phase-0 samples
  identical to the v3 pack). Ground tiles stay four authored frames over 6 s.

## Time

`pub fn present_seconds(tick: u64, f: f64) -> f64` =
`(tick.saturating_sub(1) as f64 + clamp(f, 0, 1)) · DT`; a non-finite `f` reads as 0.
This is the one presentation-time helper for every looping clip, the water shimmer, the
ground breath and the rain. Rationale: bodies are drawn `f` of the way along the path
they traveled *during* tick `tick − 1 → tick`, so the scene's clock is that same
interval; it is continuous at every tick boundary (`f → 1` of tick `T` meets `f = 0` of
`T + 1`), it never reads wall time, `--speed` scales it because ticks do, and a paused
world holds it. (Rain used `tick + f` before; it moves to this helper.)

`pub fn clip_time(clip: &Clip, seconds: f64, phase: f64, gestation: Option<f32>) -> f64`
— **signature change**: looping → `seconds + phase`; non-looping → `progress ·
clip.seconds`, `None` → 0.

`pub fn water_brightness(seconds: f64, phase: f64) -> f32` (was `tick`): same formula on
`seconds / WATER_SHIMMER_SECONDS`.

`pub fn ground_pose(tile: &GroundTile, seconds: f64) -> Pose<'_>` replaces `ground_frame`:
the looping sample rule of `Clip::sample` applied to the tile's frames and `seconds`.

## Rain

- `pub fn rain_blink(seconds: f64) -> f32`: with `u = seconds mod RAIN_PERIOD`, a raised
  cosine `0.5 · (1 − cos(2π · u / RAIN_BLINK))` for `u < RAIN_BLINK`, else 0; non-finite →
  0. Peak 1 at `u = RAIN_BLINK / 2`, continuous everywhere.
- `pub fn rain_blink_on(seconds: f64) -> bool` = `rain_blink(seconds) > 0`.
- `pub fn rain_marks(cell, k, seconds) -> Vec<((u8, u8), f32)>` — weighted marks.
  Side face: the streak is a 1×2 mark at continuous downhill position `s = origin +
  rain_fall(seconds)`; with `φ = fract(rain_fall)` and the head pixel at the wrapped
  integer position exactly as before (`(origin + floor(fall)).rem_euclid(4)` within the
  cell's four pixels), the marks are head `(1 − φ)`, head + 1 downhill `1`, head + 2
  downhill `φ`, each dropped if it leaves the face (the two trailing pixels are not
  wrapped, as before). Total weight is 2 whenever all three are on the face, so a
  streak's light is constant as it falls. Top face: one pixel at the origin with weight
  `rain_blink(seconds)`, omitted when 0.
- `draw_rain`: per-mark alpha `min(1, RAIN_OPACITY · min(rate, 1) · weight)`.

## Growth (small plants)

Per cell the presenter keeps:

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Growth {
    /// The stage the visual has completed (`None` = bare).
    pub from: Option<u8>,
    /// The stage it is moving to; `== from` when idle.
    pub to: Option<u8>,
    /// Progress of `from → to` in [0, 1]; 1 when idle.
    pub g: f64,
    /// The resource-driven target with hysteresis (what `stages` used to hold).
    pub target: Option<u8>,
    /// Fruit-accent blend in [0, 1].
    pub fruit: f64,
}
```

`Growth::snapped(target: Option<u8>, in_fruit: bool) -> Growth`: `from = to = target`,
`g = 1`, `fruit = 1` iff `in_fruit && target == Some(2)` else 0.

`pub fn advance_growth(g: Growth, target: Option<u8>, in_fruit: bool, dt: f64) -> Growth`
— **normative**, pure, one call per simulation tick with `dt` the simulated seconds
elapsed (0 is legal: only retargeting happens). Let `rank(None) = −1`, `rank(Some(s)) = s`.

1. `g.target = target`. A non-finite or negative `dt` is 0.
2. Idle (`from == to`) and `target != to`: start one step — `to = from ± 1` toward
   `target` (one stage at a time: `None → 2` runs `None → 0 → 1 → 2`), `g = 0`.
3. In flight (`from != to`) and `target != to`: if `target` lies on the `from` side of
   `to` (sign of `rank(target) − rank(to)` opposite to sign of `rank(to) − rank(from)`;
   this includes `target == from`), **reverse**: swap `from`/`to`, `g = 1 − g`. If
   `target` lies beyond `to` in the same direction, keep going and finish this step
   first.
4. Advance: if `from != to`, `rate = 1 / STAGE_GROW_SECONDS` when `rank(to) >
   rank(from)` else `1 / STAGE_WILT_SECONDS`; `g = min(1, g + dt · rate)`; when `g`
   reaches 1, `from = to`, `g = 1`. At most one step starts per call.
5. Fruit: `fruit_target = 1` iff `in_fruit && from == Some(2) && to == Some(2)`, else 0.
   `fruit` moves toward it by `dt / FRUIT_FADE_SECONDS` when rising and `dt /
   FRUIT_DROP_SECONDS` when falling, clamped to [0, 1], never overshooting.

Presenter (`ArtPresenter`):

- State: `growth: Vec<Growth>` (one per cell), `last_tick: Option<u64>`.
- `observe(view)`: `dt = (view.tick − last_tick) · DT` clamped to `[0,
  MAX_STEP_SECONDS]`. **Snap** (no transition, everything set to its target, bodies with
  no fade) when `last_tick` is `None` (first view: a mature world must not replay its
  growth) or `view.tick < last_tick` (a replaced or rewound world). Otherwise per cell:
  `band = cell_band(...)`; if the band changed, `growth = Growth::snapped(None, false)`
  (bare; the new band's plant then grows paced); `target = plant_cap(band, cell).map(|cap|
  next_stage(growth.target, t, thresholds, cap)).flatten()`; `growth =
  advance_growth(growth, target, fruit_stage(view.fruit[cell]), dt)`. Then
  `last_tick = Some(view.tick)`.
- `draw(view, f, canvas)`: if never observed, snap-initialize from the view first;
  otherwise **mutate nothing** — `update_stages`/`update_tall` are gone from `draw`. Draw
  is a pure function of (presenter state, view, f). Two draws of the same inputs give the
  same image; a thousand draws advance nothing.
- Accessors: `stage_of(cell) -> Option<u8>` returns `growth.target` (unchanged meaning:
  the stage the field warrants after hysteresis); `growth_of(cell) -> Growth`.

Drawing a cell at presentation seconds `s`, with `p` the band's plant, `t` its density,
`th` its thresholds, `ceiling = band_opacity(band)`, `phase = plant_phase_of(cell,
clip.seconds)`:

- Stage `k` idle: `pose = p.stages[k].sample(s + phase)`, opacity `stage_opacity(k, t,
  th, ceiling)`, `Mask::None`. If `k == 2`, `p.fruit` is `Some` and `growth.fruit > 0`:
  `Pose { first: p.stages[2].at(s + phase), second: fruit.at(s + phase), mix:
  growth.fruit }` — an exact premultiplied lerp between the two clips; during the fruit
  fade the sway is nearest-sample (125 ms steps) rather than blended. At `fruit == 1`
  use `fruit.sample(s + phase)`.
- In flight (`from != to`, progress `g`): `lower`/`upper` are the lower-/higher-ranked
  of the two; `gu` = the upper's reveal = `g` if `to` is upper else `1 − g`. Draw
  `lower` (if `Some`) as its idle pose at opacity `stage_opacity(lower, ..) · (1 − gu)`,
  then `upper` as its idle pose at `stage_opacity(upper, ..)` with mask
  `Mask::Axial { reveal: gu · PLANT_REVEAL_PX }` when `up_of(cell).is_some()` (a side
  face: the plant stands on the tile's bottom edge and extends upward) or `Mask::Radial {
  reveal: gu · (pose.extent() + 0.5) }` on the top face (radial plants open from their
  center). The lower stage's fruit blend still applies if `lower == Some(2)`.
- Heading, anchor and opacity ceilings are unchanged.

## Growth (tall columns)

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TallGrowth { pub height: f64 /* segments, 0..=TALL_MAX_SEGMENTS */, pub target: u8 }
pub fn advance_tall(g: TallGrowth, target: u8, dt: f64) -> TallGrowth
```

`height` moves toward `target` by `dt · TALL_GROW_PX_PER_S / 4` when rising and `dt ·
TALL_WILT_PX_PER_S / 4` when falling, never overshooting; `target` is `next_tall(g.target,
t_col)` computed by the presenter as before. `segments_of(column) -> u8` returns the
target (unchanged meaning); `tall_growth_of(column) -> TallGrowth`. Snap sets `height =
target`. Trunk, vine, base and cap all sample at `s + tall_phase_of(face, cx, seconds)`
so overlapping periodic texture never shears.

`pub fn tall_anchor_at(face, cx, i: f64) -> SurfacePoint`: the horizon cell's center moved
`4 · i` px up the face; `tall_anchor(face, cx, i)` equals `tall_anchor_at(face, cx, i as f64)`.

Drawing a column at height `h`: nothing when `h ≤ 0`. Let `fade = clamp(h /
TALL_BASE_FADE, 0, 1)`. Base at tile 0, opacity `MOTIF_OPACITY · fade`. For `i in
1..=ceil(h)`: `frac = clamp(h − (i − 1), 0, 1)`, `lo = TALL_FIRST_JOIN` if `i == 1` else
`TALL_JOIN`, `reveal = lo + (PLANT_REVEAL_PX − lo) · frac`; trunk tile `i` at
`tall_anchor(i)` with `Mask::Axial { reveal }`; the vine's trunk at every odd `i` with the
same mask. (The trunk is 4-periodic, so revealing rows above `lo` is exactly the newest
segment growing out of the one below; `lo` is where the tile below stops painting new
pixels: 12 px over a trunk, 9 px over the base whose join rows reach 5 px above the
horizon center.) Crown: `plant.cap` (never `plant.crown`) at `tall_anchor_at(face, cx, h
+ 1.0)`, `Mask::None`, opacity `MOTIF_OPACITY · fade` — it glides with the column at
continuous height over crisp trunk segments and, at rest on a whole cell, sits where the
crown used to.

## Bodies

```rust
struct BodyMemory { state: usize, prev: usize, switched_at: f64 /* presentation seconds */ }
```

`observe`: per organism, if its `state_of` differs from memory: `prev = old state`,
`switched_at = present_seconds(view.tick, 0.0)`, `state = new`. Unknown ids (and every
id on a snap) enter with `prev = state`, `switched_at = f64::NEG_INFINITY`. Ids absent
from the view are dropped. `draw`: `s = present_seconds(tick, f)`, `w = clamp((s −
switched_at) / BODY_FADE_SECONDS, 0, 1)`. If `w ≥ 1`: `pose = clip.sample(clip_time(clip,
s, phase, o.gestation))`. Else: `Pose { first: prev_clip.at(clip_time(prev_clip, s,
phase, prev_gestation)), second: clip.at(clip_time(clip, s, phase, o.gestation)), mix: w
}` where `prev_gestation = Some(1.0)` if `prev == 3` (the bud clip's last frame is the
birth) else `o.gestation` — an exact cross-clip lerp for 0.3 s with nearest samples
inside each clip. Anchor and heading from `present::interpolate` as before.

## Constants (all `pub`, review-tunable, in `art_present.rs`)

| name | value | meaning |
| --- | --- | --- |
| `STAGE_GROW_SECONDS` | 4.0 | one stage step upward |
| `STAGE_WILT_SECONDS` | 2.0 | one stage step downward |
| `FRUIT_FADE_SECONDS` | 2.0 | fruit accent fading in |
| `FRUIT_DROP_SECONDS` | 1.0 | fruit accent fading out |
| `TALL_GROW_PX_PER_S` | 1.5 | column extension |
| `TALL_WILT_PX_PER_S` | 3.0 | column decline |
| `TALL_BASE_FADE` | 0.25 | segments over which base and cap fade in from bare |
| `TALL_JOIN` | 12.0 | tile px below which a trunk tile only repeats the tile beneath |
| `TALL_FIRST_JOIN` | 9.0 | the same for the first trunk over the base |
| `PLANT_REVEAL_PX` | 16.5 | `Mask::Axial` reveal at which a 16-row tile is fully shown |
| `BODY_FADE_SECONDS` | 0.3 | cross-clip fade on a body's state change |
| `MAX_STEP_SECONDS` | 1.0 | cap on the simulated time one `observe` may advance |

## Revisions after Astra's review (2026-09-13, second pass)

`astra-animation-review-2026-09-13.md` and the root review notes changed the following;
the sections above are kept as the original work order, these override them:

- **Reveal start envelope.** `Mask::Axial`/`Radial` coverage is multiplied by
  `clamp(reveal, 0, 1)` so a reveal tending to zero tends to an empty stamp (no half-pixel
  pop at the pivot or from bilinear support below the bottom edge).
- **Single ownership of column rows.** New `Mask::Strip { floor, reveal }` (ramp ×
  `clamp(h − floor + 0.5)` × `clamp(reveal − floor)`); every trunk/vine tile paints only the
  rows it owns up to `tall_grown_px(height)` (5 px → 12 px over the first segment, +4 px
  per segment after), so no row is composited twice and the image is continuous at whole
  heights and at birth. Tall parts draw at `TALL_OPACITY` = 0.95 (the old triple stacking
  of 0.85 read as ≈ 0.997; this keeps the look close). Vine tiles own rows 4..12 (the
  topmost up to 16) and fade in with the base and cap.
- **Cap by measured tail, not color equality.** `TallPlant::tail_row` is measured at load
  as the first crown row from which every trunk-painted texel is painted identically in
  every frame (spiretree 8, glasscane 6); only rows `tail_row..16` are subtracted. The
  loader requires crown and trunk to share sample count and duration and rejects a tail
  starting above row 4. (`CROWN_CAP_ROWS` is gone.)
- **Cross-clip fades keep temporal blending.** `stamp_layers(&[(Pose, weight)])` mixes up to
  three temporally-blended poses in one source-over; fruit and body fades use it. A body
  switching state mid-fade starts from the blend on screen (`BodyMemory::switch_to`, at
  most two remembered states; a third drops the lightest).
- **Fruit is the food signal.** The accent fades *in* over `FRUIT_FADE_SECONDS` and is 0 the
  moment the field drops below `FRUIT_SHOW` or the plant leaves stage 2; `FRUIT_DROP_SECONDS`
  is gone. `draw_with_fruit` gates the accent by the fruit field it is given, so `None`
  always suppresses it.
- **Clock.** `present_seconds(0, f) = 0` for every `f`.
- **Same-tick observe.** A step starts only when `dt > 0`.
- **Rewind.** A snap derives targets from the empty baseline (`next_stage(None, ..)`,
  `next_tall(0, ..)`), so a restored view draws what a fresh presenter would.
- **Growth at the render rate.** The presenter keeps the previous tick's `Growth`/`TallGrowth`
  and draws `growth_between(prev, cur, f)` / `tall_between(prev, cur, f)`, so growth,
  height, fruit and reveals move every frame, not once per tick. The hysteresis *targets*
  still change at tick boundaries; the visuals interpolate toward them.
- `Pose::extent()` reports the endpoint actually drawn at weights 0 and 1.

## Non-goals of this slice (record, do not build)

Shared wind; runtime part rigs; droplets and nibbles; persisting in-flight visual growth
across a restart (a restart snaps to the field's target — recorded limitation); an
antialiased coverage bake (Astra's 4×4 subsample proposal — a follow-up once the
temporal blend is reviewed); authored per-plant growth clips (the reveal masks stand in
for them; authored "extending stalk" poses remain the better long-term answer).

## Verification owned by Fable

Existing renderer/host tests green; the independent motion tests
(`crates/cubarium/tests/art_motion.rs`, `crates/cubarium-render/tests/pose.rs`) green;
release draw cost of the crowded scene (`plant_and_body_draw_cost`, baseline 6.79 ms)
re-measured; native 64×64 PNG captures at 1× from a live world state and from a
scripted bare-to-rich fixture, reviewed at 1× and nearest-neighbour enlargements;
research note `animation-slice1-2026-09-13.md` with what landed, numbers, captures and
limitations.
