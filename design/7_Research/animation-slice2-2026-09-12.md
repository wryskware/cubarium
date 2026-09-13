---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Animation slice 2 — what landed, measurements, limitations

Research record for the second animation pass (authorization:
`animation-slice2-handoff-2026-09-12.md`; direction: `astra-animation-slice2-2026-09-12.md`
including its review of the brief; work order: `animation-slice2-brief-2026-09-12.md` with
its "Accepted corrections"). Evidence, not canon. Nothing here changes the simulation, its
20 Hz tick, saved worlds, the shim, the display contract, or the no-art image; without
`--art` the picture is unchanged. No commits were made and no ledger entry was touched.

## Baseline (slice-1 checkpoint binary, before this slice)

Captured from copies of the read-only seed `/tmp/cap-state-base/world-382800.cubw` with the
checkpoint binary (SHA256 `365598d1…dde1aad`, verified equal to `target/release/cubarium`
at the start), `--art assets/atelier --sink png`, 1×, 60 fps, native net PNGs (256×128).
Statistics from `/tmp/s2-jumps.py` (mean |Δ| per pixel per step over the whole net, sRGB
0–1; "jump" is the largest single-channel change between consecutive frames):

| capture | frames | mean \|Δ\| (max) | jump median / p90 / max | steps > 0.5 |
| --- | --- | --- | --- | --- |
| every frame, 4 s (`/tmp/s2-cap-base-60`) | 237 | 0.00183 (0.00209) | 0.273 / 0.314 / 0.576 | 2 of 236 |
| every 6th frame, 12 s (`/tmp/s2-cap-base-12`) | 121 | 0.00877 (0.00968) | 0.776 / – / – | 119 of 119 |

These reproduce the slice-1 record's maximum-jump figures. Compare mean changes only
within this table and the matched after-table: this script averages all RGB channels
over the whole net, while the earlier record used a different aggregation.

## What landed

### Wind and rooted bend (`cubarium-render::sprite`, `cubarium::art_present`)

- **`Bend { amplitude, base, root, length }`** in the renderer: a horizontal displacement in
  the sprite's own unscaled tile coordinates, `D(H) = amplitude · smoothstep((H − root) /
  length)` with `H` the height above the plant's root line (`tile_height − p.y + base`);
  the renderer samples the exact inverse `x − D`, rows are preserved, every mask is
  evaluated at the material point `(x − D, y)` (identical for `Axial`/`Strip`, required for
  `Radial`). `stamp_layers_bent` is the entry point; `stamp_layers` is it with `Bend::NONE`
  and is bit-identical to before (one monomorphised loop, no cost when calm).
- **Headroom is measured, not assumed.** `Sprite::bend_headroom(root, length, base)` is the
  largest amplitude for which every painted texel's whole bilinear support, displaced by
  the profile one pixel above the texel, stays inside the nine-pixel unfold radius
  (`hypot(|x| + A·s_max + 1, |y| + 1) ≤ 9`). The renderer unfolds `min(9, extent + |A|)`;
  a test-only `stamp_layers_bent_with_radius` lets the acceptance sweeps prove that
  radius 9 and radius 14 draw the same image at every admitted amplitude. My first
  brief's texel-centre criterion was wrong (Astra's review, item 1): its budgets (3.8 /
  0.55 / 5.1 / …) admitted amplitudes whose filter tail fell outside the radius; the
  corrected numbers are the table below.
- **One shared breeze.** `wind_strength(seconds)` in `[0, 1]`: a packet every `WIND_PERIOD`
  30 s — smoothstep rise 5 s, hold 8 s, smoothstep fall 5 s, then **exactly 0** for 12 s —
  multiplied through its whole active interval by a flutter (`WIND_FLUTTER` 0.3 on 2.3 s)
  and a slow peak modulation (`[0.85, 1]` on 97 s), so no boundary steps and consecutive
  packets differ. `wind_chart(face, u, v)` is Astra's polynomial circulation (`a = u/32 −
  1`, `b = v/32 − 1`; sides `(−(1 − a²), 0)`, Top `(−b(1 − a²), a(1 − b²))`), unnormalized;
  the seam claim was verified against the surface crate's own tangent transport on all
  sixteen connected half-edges (agreement within 1e-6; exactly zero on side/side seams,
  Top's vertices and centre; `|W| ≤ 1`). `wind_at(point, seconds, lag)` delays the packet
  by `WIND_TRAVEL_SECONDS` 0.6 × a spatial phase from the embedded position, so the gust
  sweeps the cube as one front; the interval in which every root is calm at once is
  therefore `12 − 2·(0.6 + lag)` s of every 30.
- **Species response** (`WIND_RESPONSE`, review-tunable): desired tip travel / lag —
  lanternstalk 0.45 px / 0.10 s, tendrilfan 0.55 / 0.15, reedspire 0.70 / 0.05, glowcap
  0.12 / 0, rootveil still, spiretree 0.9 / 0.2, glasscane 0.5 / 0.1, vinecoil = its host
  column; canopy species rotate about their stationary centre instead of bending
  (umbrellafrond 2°, bloomcrown 1.5°, scaled by the local wind magnitude). Per-slot ±10 %
  amplitude variation from the slot hash (appended to the stream, nothing reordered), no
  per-plant phase offsets.
- **Budgets** are measured once at `ArtPresenter::new` over every frame of every clip a
  family can draw (stages, fruit, growth transitions; a column over base at `−8`, trunk
  and vine at the highest trunk base, cap at its highest continuous base), and the admitted
  family tip is `min(desired, budget / 1.1)` so no slot's variation can exceed it
  (`ArtPresenter::bend_budgets()`; independently recomputed by the test pass, exact):

  | asset | budget px | limiting frame | desired | effective (±10 %) |
  | --- | --- | --- | --- | --- |
  | glowcap | 2.40 | stage 2 f0 | 0.12 | 0.120–0.132 |
  | rootveil | 5.43 | stage 2 f0 | 0 | still |
  | lanternstalk | 3.23 | stage 2 f4 | 0.45 | 0.450–0.495 |
  | tendrilfan | **0.31** | fruit f2 | 0.55 | 0.286–0.314 |
  | umbrellafrond | 0.33 | stage 2 f6 | rotates 2° | – |
  | bloomcrown | 2.07 | stage 2 f0 | rotates 1.5° | – |
  | reedspire | 4.38 | stage 2 f16 | 0.70 | 0.700–0.770 |
  | spiretree | **0.30** | cap f0 | 0.90 | 0.272–0.299 |
  | glasscane | **0.47** | trunk f0 | 0.50 | 0.423–0.465 |
  | vinecoil | 0.47 | trunk f0 | host's | host's |

- **Wiring.** A side-face plant takes one wind sample at its root, projected onto the tile's
  own horizontal axis (jitter kept), `Bend { base 0, root PLANT_BEND_ROOT 1.5, length
  PLANT_BEND_LENGTH 13 }` on every stamp of that slot (idle, fruit blend, both in-flight
  stamps, the growth clip). The root is 1.5 px, not the brief's 1.0, because the plants'
  lowest painted row is tile row 14 whose centre sits 1.5 px above the tile's bottom edge:
  at 1.0 that row displaced by ~0.002 px and resampled, so the contact pixel was not
  bit-identical windy and calm; at 1.5 it is. A column takes one sample at its base anchor
  and one amplitude for base, every trunk strip, the cap at its fractional index and every
  vine tile, with `base = 4i − 8` (`TALL_BEND_LENGTH` 48, root 0): one continuous curve,
  `Mask::Strip` ownership untouched. Ground, water, rain and bodies do not move. In a quiet
  interval the image is the slice-1 image bit for bit (proved as a chain: every slot and
  column gets `Bend::NONE`, and `Bend::NONE` is bit-identical to `stamp_layers`).
- `wind_fixture_tick()` reads `CUBARIUM_WIND_TICK` for the two release timing fixtures
  (peak tick 121, calm tick 401). It is the only environment-reading function in the
  module; the drawn image never depends on it.

### Lanternstalk growth pilot (`art/plants/lanternstalk.tscn`, `art/bake.gd`,
`cubarium::art`, `cubarium::art_present`)

- An authored, non-looping 4 s `grow01` clip (sprout → stage 1) in the checked-in scene
  (hand-edited; the Python generators were not run): fixed root at scene `(0.5, 7)` (tile
  row 14 stays the lowest painted row in all 24 samples), the stem cross-fades in over the
  sprout (0.4–0.9 s), extends by scaling the `stalk1` sprite about its bottom (1.0–2.3 s,
  3 → 5 painted rows, the centre offset pinned to `−2.5·scale`), the bulb fades in on the
  stem top at 1.0–1.4 s and enlarges 1.6–3.5 s always overlapping the stem's top row; the
  last sample equals the neutral Stalk1 image, the first the neutral sprout. All new tracks
  have neutral values in `RESET`. The bulb starts fading in with the stem's extension
  rather than after it, because a strictly sequential schedule blinked the cyan tip out for
  six samples; the bud is never off the stalk top. Reviewed as an 8× strip
  (`/tmp/s2-grow01-strip.png`): sprout, stem rising, bud, lantern — readable at 64 px.
- **Pack v5**: `bake.gd` bakes every plant animation named `grow<from><to>` as a row after
  that plant's stage/fruit rows, sampled inclusively at `i / (PLANT_FRAMES − 1)`, `loop:
  false`, `stage: "grow"`, `from`, `to`; `assets/atelier/plants.png` now has 25 rows. The
  bake is byte-reproducible (two independent bakes identical) and every v4 stage/fruit
  tile is byte-identical in the new pack; the other four atlases are unchanged files. The
  loader accepts versions 1..=5; `Plant::transitions` / `Plant::transition(from, to)`
  (validated: `from + 1 == to`, no duplicates, extent budget, non-looping); v1–v4 packs
  load with none. Documented in `art/PLANTS.md` "Pack v5: growth transitions".
- **Playback.** When `plant.transition(lower, upper)` exists for the step in flight, the
  presenter makes one `stamp_layers_bent` of three layers — the lower stage's idle sway at
  `w_from = 1 − smoothstep(t / GROW_BLEND)`, the clip sampled at `t · seconds` at `w_grow`,
  the upper stage's idle sway at `w_to = smoothstep((t − (1 − GROW_BLEND)) / GROW_BLEND)`
  (`GROW_BLEND` 0.12) — with no mask, opacity lerped between the two stages, and the slot's
  wind bend. `t` is the same per-frame progress the mask path used, so a reversal replays
  the clip backwards and nothing restarts on a draw or a packet; the fruit accent takes no
  part; every other pair (`None → 0`, `1 → 2`, any v4 pack) keeps the reveal masks bit for
  bit. `GrowthStep`/`growth_step` and `growth_weights` are public so the independent tests
  could be written from the doc comments.

Docs: `art/README.md` gained "Wind" and "Authored growth (the pilot)" under *Live world*;
`art/PLANTS.md` gained the pack v5 section.

## Measurements after

Release draw cost on this machine (Fable's final run on the integrated tree; Package A's
earlier crowded runs were 11.08–11.14 ms, and wet runs were 12.26 ms):

| fixture | active gust (strength 0.71) | exact calm | slice 1 |
| --- | --- | --- | --- |
| `plant_and_body_draw_cost` (622 plants, 200 organisms) | 11.45 ms (69 %) | 9.86 ms (59 %) | 9.86 ms |
| `art_water::everything_on_draw_cost` (wet, raining, rich, all columns tall, 200 bodies) | 12.20 ms (73 %) | 10.50 ms (63 %) | 10.453 ms |

The calm numbers reproduce slice 1's, which is the evidence that the identity path costs
what it cost; the windy cost is the larger unfold (up to +0.5 px radius) and the per-pixel
profile. Both stay under the 16.7 ms budget with about 4.5 ms of headroom in this gust; these
are draw timings, not end-to-end frame pacing on the cube.

Root separately tested crowded transitions over twelve simulated seconds at 60 draws
per second, with rain and 200 bodies (`animation_load.rs`, release, run alone): dry
growth including the authored clip averaged **11.385 ms/frame**, worst one-second mean
**12.972 ms**; wet decline averaged **10.101 ms**, worst one-second mean **10.763 ms**.
These are window averages, not guarantees that every frame meets its deadline.

The isolated second-pass preview is `http://127.0.0.1:7397/`; its frame endpoint measured
**59.82 submitted frames/second** over a three-second sample at 1× time. This does not
measure browser or hardware pacing. The approved first-pass preview on port 7396 and
the original runners remain untouched; the new preview uses a copied world state.

Live-snapshot captures (same seed copies, final build `8cd836ca…2b2707`):

| capture | frames | mean \|Δ\| (max) | jump median / p90 / max | steps > 0.5 |
| --- | --- | --- | --- | --- |
| every frame, 4 s (`/tmp/s2-cap-new-60`) | 237 | 0.00184 (0.00206) | 0.273 / 0.312 / 0.576 | 2 of 236 |
| every 6th frame, 12 s (`/tmp/s2-cap-new-12`) | 120 | 0.00878 (0.00971) | 0.776 / 0.828 / 0.859 | 119 of 119 |

The summary metrics are nearly identical to the baseline: this capture adds motion
without increasing the count of large jumps (the two > 0.5 steps are the same
body-at-the-rim events as before). The capture
window starts at presentation second 19139.95, i.e. at a packet boundary, so the first
frame differs from the checkpoint image by one 8-bit step in 66 pixels (the delayed
per-root sampler puts a few plants a fraction of a second into the packet) and the last
frame, 4 s into the rise, differs in 3781 pixels by at most 43/255 — the difference is on
column caps, the tops of foliage plants and the rotating canopy plants, nothing on bodies,
water or ground (`/tmp/s2-A-vs-base-236.png`, `/tmp/s2-front-base-vs-A-236.png`).

Sparse fixture capture (`crates/cubarium/tests/art_wind_capture.rs`, `#[ignore]`d,
`CUBARIUM_CAPTURE_DIR`, default `/tmp/cubarium-wind`): every side species on Front, a
full spiretree column with vine on Front and a glasscane on Back, a reed in a pool on
Left, both canopy species on Top, and a second lanternstalk on Right fed at 6 s so its
authored growth plays inside the first gust; 36 simulated seconds after a 30 s warm-up,
1080 native net PNGs (every 2nd frame). Statistics: mean |Δ| 0.00013, jump median 0.224 /
p90 0.350 / max 0.424, **0 of 1079 steps over 0.5**. Reviewed as nearest-neighbour
enlargements and strips (`/tmp/s2-wind-f540-x4.png`, `/tmp/s2-wind-column-strip.png`,
`/tmp/s2-final-column-calm-peak-diff.png`, `/tmp/s2-final-pilot-strip.png`) and as
`captures/animation-review/wind-preview.mp4` (36 s at 30 fps, 4× nearest) next to
`live-wind-4s.mp4` (the 4 s live capture at 60 fps). What the images show: the full column
at calm and at the hold differ along the whole trunk and vine together, the base rows are
untouched and there is no gap or double-density line where strips meet; the cap's lean is
about a quarter pixel and reads only as a slow edge drift at 1×; the reed and the
lanternstalk lean visibly; the pilot's sprout is revealed by the mask and then the stem
rises with the bud on it (the bright square in the fed cell is the fixture's own producer
ramp, as in the slice-1 growth capture).

Root's independently generated integrated capture is preserved with reproduction notes
at `captures/animation-review/slice2/wind-and-growth.mp4` and the adjacent `README.md`.

## Tests

Final integrated run, `cargo test -p cubarium-render -p cubarium --all-targets`: **346
passed, 0 failed, 8 ignored** (timing and capture fixtures); debug build warning-free;
release keeps only the pre-existing `cubarium-core::AUDIT_TOLERANCE` warning.

Root's broader `cargo test --workspace --all-targets --quiet` also passed, covering
core, surface, oracle, and transport suites. After the reviewer's final rate-scheduling
refinement, root reran all nine growth-clip tests successfully. `git diff --check`
passes. Whole-workspace `cargo fmt --all -- --check` is not clean: it reports extensive
format differences in pre-existing code and vendor files; no bulk formatting was applied.

- Render crate: 31 unit (7 new for `Bend`/headroom/bent stamping), `astra_regressions` 7,
  `pose_layers` 10, Astra's `astra_wind_regressions` 5 (acceptance gate: identity, root
  rows, material-space radial mask, admitted headroom keeps the filter tail and corners
  inside the unfold), Package B's `bend.rs` 7 (identity for twelve degenerate bends × three
  anchors × four masks; root rows fixed to ±50 px; per-row displacement equals the Hermite
  profile; integer amplitudes shift material by whole texels; masks read the moved
  material; seam light conserved to 1e-6 on axis-aligned headings and 3e-3 on rotated
  ones — a bilinear kernel is only a partition of unity on an axis-aligned lattice; the
  admitted headroom draws the same image at radius 9 and 14).
- Host crate: 125 unit (13 new: packet, seam transport verification, budget table
  printer, growth-clip blend weights, endpoints, reversal, purity, fallback, wind through
  growth, top-face playback), `art_bands` 6, `art_mode` 25, `art_motion` 25, `art_plants`
  13, `art_water` 21, `astra_motion_regressions` 4, Astra's `astra_wind_regressions` 6,
  Package B's `art_wind.rs` 18 (global sampler in range / exactly quiet / no boundary
  jumps at 1 ms; chart field agrees in 3D across every seam via `embed_tangent`; exact
  calm points; the delayed sampler is zero through the shared quiet interval; budgets
  recomputed independently; the shipped-pack footprint sweep at ±budget over three anchors
  with radius 9 == radius 14 and nothing vanished; a synthetic striped column composited
  once per row at fractional heights and wind extrema; the vine shares its host's
  amplitude; a quiet tick bends nothing anywhere; the rising breeze moves only plant
  pixels; root rows identical windy and calm for every side species; no per-frame jump at
  60 fps; 30/60/120 fps identical; pause and determinism; canopy rotation about the pivot
  with no translation; v4 fallback), the growth-clip pass `art_growth_clip.rs` 9 (the
  pack's one transition; the played frame bit-identical to the documented three-layer
  stamp at windy and calm instants; blend edges and idle endpoints; reversal bit-identical
  at equal progress; purity and rate independence; a 60 fps per-frame bound of 0.188
  derived from the 24-sample cadence against a measured worst of 0.046; fallback identical
  to the hand-built mask image; fruit absent in flight; root contact fixed and nothing
  painted below it), and the unchanged `png_layout`, `raycast_inversion`, `run_*`,
  `shim_sink` suites.
- Existing tests adapted without weakening: hand-stamped plant/column expectations in
  `art_plants`, `art_water` and one in `art_motion` now carry the same public
  `slot_wind`/`tall_amplitude` bend; the two "the image repeats after one clip period"
  tests are measured inside a quiet interval and additionally assert the instants are
  calm (the breeze's 30/2.3/97 s periods are deliberately incommensurate with the clips);
  the loader's expected lists carry each plant's transitions.
- Independent-pass findings, none an implementation bug: (1) "total light equal to a
  mid-face stamp at a vertex" is impossible by geometry — an unbent stamp at a Top vertex
  already carries 79 % of the mid-face light, the wedge the unfold's own doc describes;
  the test asserts the wind moves < 10 % of the light there (measured 2.8 %). (2) The
  brief's "root row identical across the whole growth step" is false by design: the entry
  blend mixes the sprout's whole-sprite pulse with the clip's neutral first frame, so
  row 14's brightness moves by ≤ 0.008; the test asserts instead that the pixels at or
  below the root are bit-identical bent or calm, keep the same footprint at every `t`,
  and nothing below the root line is ever painted. (3) `wind_at`'s doc said "exactly zero
  through a quiet interval"; qualified to the shared interval (fixed in the doc comment).

## Limitations and follow-ups

- **Amplitudes are small at 64 px.** The measured budgets, not the desired tips, bound
  tendrilfan (0.31 px), spiretree (0.30 px at the cap) and glasscane (0.47 px). A spiretree
  crown drifts by about a quarter pixel at full wind — legible as a slow lean at 1× only on
  a full-height column. Clearing the cap's top rows would not help (checked: the limiting
  texels are the cap's widest rows and the full-height 4-periodic trunk, whose own limit
  is 0.46 px). Candidate approaches include narrower art or revised, recentered per-part
  bend bounds and trunk-strip anchors, each requiring its own seam and registration
  proof. No single redesign is established as necessary. Deliberately deferred here:
  Wrysk approved the current silhouettes, and the
  lanternstalk (0.45 px) and reed (0.70 px) do read.
- **Short columns barely lean.** The bend length is a fixed 48 px so lower trunk pixels do
  not slide as a column grows; the live world's columns are about three segments tall, so
  their caps sit at `smoothstep(20/48) ≈ 0.3` of the amplitude, ≈ 0.1 px.
- **The quiet interval shifts slightly by position** (the delayed per-root sampler, up to
  ±0.6 s plus lag); the global sampler is exactly zero for 12 of every 30 s and every root
  at once for about 10.4 s.
- **A reed on a flooded top-face cell** is radial and has no spin, so it stands still.
- **Growth pilot covers one transition** (lanternstalk 0 → 1). 1 → 2 (the mature stalk has
  its own flare and a 7×7 bulb, so stretching cannot end on it), the other species, and
  the top-face radial species keep the reveal masks; the fruit-blended endpoint of a
  hypothetical 1 → 2 clip would use the stage's sway alone for the one tick a fruiting
  plant begins to wilt (documented, cannot arise on the shipped pack).
- Coverage-AA bake, droplets, nibble scars, LCD migration, persistent plant biology and
  fauna ecology remain later ideas (unchanged). Slice-1 limitations (restart snaps,
  20 Hz targets, fruit leaving in one frame, bake spatial quantization) are unchanged.

## Changed files

Source and art: `crates/cubarium-render/src/{sprite.rs, lib.rs}`,
`crates/cubarium/src/{art.rs, art_present.rs}`, `art/bake.gd`,
`art/plants/lanternstalk.tscn`, `assets/atelier/{pack.json, plants.png}` (the other
atlases are unchanged files), `art/README.md`, `art/PLANTS.md`. Adapted tests:
`crates/cubarium/tests/{art_plants.rs, art_water.rs, art_motion.rs}`. New tests:
`crates/cubarium-render/tests/bend.rs`, `crates/cubarium/tests/{art_wind.rs,
art_growth_clip.rs, art_wind_capture.rs}`; Astra's oracles
`crates/{cubarium-render,cubarium}/tests/astra_wind_regressions.rs` and root's
`crates/cubarium/tests/animation_load.rs` arrived during the pass and are not mine.
Design: this record, `animation-slice2-brief-2026-09-12.md`, and the status section
appended to `design/animation-roadmap.md`. Not committed.
