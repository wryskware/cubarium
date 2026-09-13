---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Living-world next packages — work order (Fable orchestrator, 2026-09-13)

Authorization: `living-world-next-handoff-2026-09-13.md`. This is the work order for the
two bounded packages it names, with the Fable-level decisions already made so that no worker
has to make a design choice. Evidence, not canon. Nothing here touches the simulation, saved
worlds, founders, the shim or the no-art image.

Rules for every worker: read `AGENTS.md`, `README.md`, `art/README.md` ("Live world",
"Wind", "Authored growth"), `art/PLANTS.md`, `design/animation-roadmap.md`, the slice-2
record `animation-slice2-2026-09-12.md`, and this file. Do **not** touch `state/`, any live
process, `.vscode/`, `crates/cubarium/examples/care_compare.rs`, `design/0_Canon`, or files
owned by another worker (table below). Do not commit; do not start long-lived servers (a
PNG capture run or a short web smoke test you close yourself is fine). Report ambiguities
instead of resolving them. Your report must carry evidence: commands run, test counts, the
artefact paths, measured numbers, and what you could not verify.

| Owner | Files |
| --- | --- |
| A (Lanternjaw implementation) | `crates/cubarium-render/src/multipart.rs` (bodies), `crates/cubarium/src/lanternjaw.rs` (bodies, private items), new `crates/cubarium/examples/lanternjaw_study.rs`, new `crates/cubarium/tests/lanternjaw_cost.rs` (`#[ignore]`), `captures/lanternjaw/` (small PNG contact sheets only) |
| C (Lanternjaw tests) | new `crates/cubarium-render/tests/multipart.rs`, new `crates/cubarium/tests/lanternjaw.rs` |
| B (growth art) | `art/plants/{lanternstalk,glowcap,rootveil,tendrilfan,reedspire}.tscn` (and `umbrellafrond`, `bloomcrown` only for the stretch goal), `art/parts/` (new SVGs only if unavoidable), `assets/atelier/{pack.json,plants.png}`, `crates/cubarium/src/art.rs` (the loader **test's** expected lists only), `art/PLANTS.md` |
| E (growth tests) | new `crates/cubarium/tests/art_growth_pack.rs`, new `crates/cubarium/tests/art_wind_top.rs` |
| Fable | everything else: `sprite.rs`/`lib.rs` exports (done), `art_present.rs` (done: the reed rule), `art/README.md`, design notes, commits |

The public interfaces are written and compile: `cubarium_render::{RigPart, rig_radius,
stamp_rig, stamp_rig_with_radius, RIG_MARGIN, Sprite::{from_premultiplied, pivot, texel,
sample_at, can_reach}}` and `cubarium::lanternjaw::*`. Their doc comments are
**normative**; A fills the `todo!()` bodies, C tests from the doc comments without reading
A's bodies.

**Revision after Astra's plan (`astra-lanternjaw-production-plan-2026-09-13.md`, arrived
during the session):** the first draft carried each part to its own anchor with `travel`
and a rim re-pivot. Astra's objection is right — two independent stamps near a top vertex
can pick different chart images for one pixel (a torn joint or doubled coverage), and a
travelled attachment reflects at the rim. The geometry is now **one root-owned
`unfold_pixels` query per body** (`stamp_rig`), radius bounded by `rig_radius` ≤ the
surface's `MAX_LOCAL_RADIUS` (32; nine stays the per-sprite material budget); pieces of one
material (equal `layer`) are summed on a shared lattice, depths source-over in layer order,
and states cross-fade as complete composites. The rim then has no pixels past it, so a
hanging part is cut, never reflected.

## Package 1 — Lanternjaw production rendering

Decisions (do not re-open):

1. **Code-native, per-frame rasterized, eight parts** (`PartName`, with layers per its
   table), drawn by one `stamp_rig`. No Godot rig, no atlas row, no pack change, no
   `rig_of` change, no founder.
2. **Fractional motion everywhere** (unrounded `dx_i`, `dy_i`, joints, leg swing, anchor);
   the leg lift becomes `sin(π u / 0.38)`. The colour envelopes are unchanged.
3. **Colours as the study computes them, in sRGB, decoded once**; the opaque structure
   (rim, plates, seams, head plates, eye, jaw, sockets, legs, near limb, claw) keeps the
   study's mixes toward `#0B0525` as opaque colours; the fan, the lantern halo/glow, the
   cocoon and the far limb carry real alpha (the study's mix fraction over the pure colour).
   See the `Lanternjaw::parts` doc comment.
4. **Rim rule**: nothing — the root query has no pixels past the open rim; a part hanging
   over it is cut where the surface ends, exactly like any sprite.
5. **Overlap**: the hull's four pieces are one summed material on the body lattice
   (integer offsets and pivots); far limb, underside, hull, glow, near limb are depths in
   that order (`PartName::layer`).
6. The study route is an **example** (`lanternjaw_study`), default web port **7399**, with
   `--sink png --seconds N --every K --out captures/lanternjaw` for native captures, and
   scenes `gallery` (four modes on Front, heading +x, u = 32, v = 12/26/40/54), `seams`
   (Front (63.0, 32.0) heading (1,0) in `move`; Right (32, 3.0) heading (0,−1) in `move`;
   Top (2.5, 2.5) heading rotating one turn per 40 s in `rest`; Front (32, 61.5) heading
   (0.6, 0.8) in `hunt`; Back (32, 32) heading (−1, 0) in `bud`), and `walk` (one `move` body
   travelling 1.5 px/s along a gently curving heading through several seams via
   `travel`/`interpolate` as `art_study.rs` does, plus a `hunt` body at Top (32, 32)). Same
   floor and ground as `art_study.rs`; no plants needed. The example never rounds a position.
7. **Cost**: `tests/lanternjaw_cost.rs`, `#[ignore]`, release: mean and worst µs per
   `Lanternjaw::draw` for one body and for two bodies over 600 frames at 60 fps presentation
   time in `move` and in `hunt`, mid-face, printed; and the same with the anchor straddling
   the Front/Right seam. A's report carries the numbers.

Acceptance for A (run before reporting): `cargo build --release --examples` clean;
`cargo test -p cubarium-render -p cubarium` green including C's tests once they exist (if a
C test fails, read its assertion against the doc comment and fix the implementation unless
the test contradicts the doc, which you report); `cargo run --release -p cubarium --example
lanternjaw_study -- --sink png --seconds 12 --every 2 --scene gallery --out
captures/lanternjaw/gallery` and the same for `seams`; then assemble one nearest-neighbour
contact sheet per scene (Python/PIL or ImageMagick, ×4, a dozen frames including the strike
frames 3.10–3.54 s of the hunt cycle) into `captures/lanternjaw/*.png` and *look at them*:
the hull, chain, fan and folded limbs must read as the study did; no part missing at a seam;
the vertex body may show the documented ownership discontinuity but no duplicated pixels
(brightness > 1 impossible; check no doubled hull). Delete the raw frame directories after
the sheets are made (keep the repo small). Report what the sheets show honestly.

## Package 2 — Authored growth expansion

Decisions (do not re-open):

1. Pack stays **v5** (additive rows). `bake.gd` is unchanged. Every new clip is a
   `grow<from><to>` animation, `loop_mode = 0`, **4 s**, hand-authored in the `.tscn` text
   the way `lanternstalk`'s `grow01` was (a dedicated `Grow<from><to>` pivot group hidden in
   `RESET`, every newly animated property pinned in `RESET`, no part scaled to zero, fade
   with `visible` plus `self_modulate` alpha). Existing stage/fruit rows must stay
   **byte-identical** (two bakes identical; every pre-existing tile unchanged).
2. **Endpoint convention** for every new clip: frame 0 is the source stage's pose with
   every pivot at rotation 0 and every `self_modulate` at 1 (its RESET-neutral pose); the
   last frame is the target's neutral pose. The presenter's 12 % endpoint blends absorb the
   phase mismatch with the running loops. (The pilot's stricter "last frame == a loop sample"
   property continues to hold for `lanternstalk` 0→1 only.)
3. **Roots stay planted**: for the side species the lowest painted row of every frame is
   between the source stage's and the target's lowest painted rows and never below tile row
   14; nothing is painted in row 15. Where the stages differ by one row (lanternstalk 1→2:
   14 → 13) the change happens with the stem's extension, once.
4. Berries/fruit parts never appear in a growth clip (`tendrilfan` must hide its `Berry`
   sprites in `grow12` exactly as `stage2` does; `bloomcrown`'s `Fruit` pivot stays hidden).
5. Clip recipes (silhouettes are the deliverable; timings are targets, tune by looking at
   an 8× strip of the 24 samples):
   - **lanternstalk grow12**: Stalk1 cross-fades (0.4–1.0 s) into a `Stem` using the
     `stalk2` texture at `scale.y = 0.625` with its bottom on row 14; the stem extends to
     `scale.y = 1` (1.0–2.6 s) with its bottom rising one row to the mature position (the
     mature `stalk2` sits on rows 6–13) so the last frame *is* the neutral Stalk2 image; a
     `Bulb` (`bulb2` texture) fades in over `bulb1` at the stem top (1.0–1.6 s) and enlarges
     from scale 0.43 to 1 riding the top (1.6–3.6 s), always overlapping the stem's top row.
   - **glowcap grow01**: sprout fades out under a `stem1` stem scaling about its bottom
     0.5 → 1 (0.8–2.4 s); the cap (`cap1` + `gills2b` textures, as `Cap1`) rides the stem
     top, fading in from 1.2 s while **widening** (`scale.x` 0.43 → 1 about its centre,
     1.2–3.2 s: an umbrella opening); gills fade in last (2.8–3.6 s).
   - **glowcap grow12**: stem1 → stem2 extending (scale 0.667 → 1 about the bottom); Cap1
     cross-fades to Cap2 which widens 0.78 → 1 with the stem top; the side shelf `Cap2b`
     sprouts from the stem (fade in 2.0–2.8 s, scale 0.5 → 1 about its own centre 2.0–3.6 s).
   - **rootveil grow01**: the crust spreads: `veil1` scales about its bottom-centre
     (`scale.x` 0.43 → 1, `scale.y` 0.667 → 1, 0.6–3.0 s) while the sprout fades out
     (0.4–1.0 s); `glints1` fade in 2.6–3.6 s. Row 14 painted throughout.
   - **rootveil grow12**: `veil2` cross-fades over veil1 (0.4–1.2 s) scaled (x 0.64, y 0.6)
     about its bottom-centre then spreads to 1 (1.0–3.2 s); `glints2` fade in late.
   - **tendrilfan grow01**: base1 fades in over the sprout (0.3–0.9 s); each tendril
     **uncurls**: the `TendrilL1`/`TendrilR1` pivots scale 0.3 → 1 about their bases and
     rotate from ±0.6 rad (curled inward) to 0 (1.0–3.4 s), left leading right by 0.3 s.
   - **tendrilfan grow12**: base2 widens over base1 (`scale.x` 0.7 → 1, cross-fade
     0.3–1.1 s); tendril1s fade into tendril2s which scale 0.6 → 1 about their pivots and
     rotate ±0.4 → 0 (0.8–3.2 s); `Center2` rises (`scale.y` 0.3 → 1 about its bottom pivot,
     1.4–3.6 s), berries hidden.
   - **reedspire grow01**: the sprout fades under `ReedA1` shooting up (`scale.y` 3/7 → 1
     about its bottom pivot, 0.6–2.4 s), `ReedB1` follows 0.8 s later; `Ripple1` fades in
     0.4–1.2 s.
   - **reedspire grow12**: A1/B1 cross-fade (0.3–1.1 s) into A2/B2 at matched heights
     (A2 at `scale.y` 7/11, B2 at 5/8; the one-column shift is what the art says, keep it in
     the cross-fade), then A2 and B2 extend to 1 (1.0–3.0 s) and `ReedC2` shoots up from the
     ripple (0.2 → 1, 1.8–3.4 s); `Ripple2` cross-fades in.
   - **Stretch, only after the seven above are baked, inspected and green**: canopy
     `umbrellafrond` and `bloomcrown` grow01 (the Frond1/Bloom1 group scaled about the
     centre 0.35 → 1 over 0.5–3.5 s while the sprout fades out) and grow12 (Frond2/Bloom2
     scaled 0.73 → 1 cross-fading from the stage-1 group). If not done, say exactly so.
6. The wind budgets must not shrink below what the shipped pack admits: for each species
   `effective_tip(desired, new budget) == effective_tip(desired, old budget)` (old budgets:
   glowcap 2.40, rootveil 5.43, lanternstalk 3.23, tendrilfan 0.31, reedspire 4.38 px). A
   mid-clip pose wider than the stages can lower a budget; if it does, narrow the pose.

Acceptance for B: `./scripts/art-bake.sh` twice → identical bytes; a Python check that
every v5 stage/fruit tile is byte-identical to `git show HEAD:assets/atelier/plants.png`;
`cargo test -p cubarium` green with the loader's expected lists updated (and E's tests once
they exist — same rule as A's); 8× strips of every new clip written to `/tmp/growth-strips/`
and *looked at*; `art/PLANTS.md` "Pack v5" lists the clips with one line each. Report the
budget table before/after (`ArtPresenter::bend_budgets`, there is a printing test).

## Reed on the top face (Fable, done)

`slot_wind` now turns in place only a top-face species with `spin_deg > 0`; a top-face
`reedspire` (tip 0.70, spin 0) bends along its own tile's horizontal axis like a side-face
plant, rooted at its ripple row. E tests it (`art_wind_top.rs`): on a flooded top-face cell
with a rank-2 reed, at a gust the reed's pixels move, its root row is bit-identical windy or
calm, the canopy species still rotate without translation, and a calm instant is bit-identical
to the image before this change (identity path).

## Test authoring (C and E)

Tests come from the doc comments, `art/PLANTS.md`, `art/README.md` and this brief — never
from implementation bodies. Follow the style of `crates/cubarium/tests/art_growth_clip.rs`
and `crates/cubarium-render/tests/bend.rs` (self-contained fixtures, hand-built expected
images through the public API, properties a wrong implementation would break). Every test
that fails only because implementation is not yet there should fail on `todo!()`, not on a
compile error; run `cargo test --no-run` before reporting.

C (Lanternjaw): `stamp_rig` — one state, one part at offset 0, layer 0 is bit-identical to
`stamp_sprite` with that sprite at integer and fractional roots, mid-face, on a seam and at a
vertex; **material reconstruction**: an opaque and a translucent test sprite each cut into
two (and four) lattice-aligned pieces in one layer draw what the uncut sprite draws (≤ 1e-6
per channel) at integer and subpixel roots and at diagonal headings — the joint neither
reveals background nor gains opacity; two layers source-over in order (an opaque layer-1
texel hides layer 0; a translucent one blends); a state mixture at weights `w, 1 − w` of two
one-part states equals the premultiplied mix of the two single-state images; a part
placed beyond the rim (root at Front (32, 60), offset (0, 6)) paints nothing below the rim
and exactly the flat placement's pixels on the rows that exist (compare with the same rig at
(32, 30) shifted by 30 rows); a body straddling a side/side and a side/top seam paints the
same total light as mid-face (1e-6 axis-aligned, 3e-3 rotated) and reaches two faces; at
each of the four top vertices approached from each incident face no pixel is brighter than 1
and every pixel is enumerated once; `rig_radius` equals the documented max and the image at
the rig's own radius equals the image at a larger legal radius (`stamp_rig_with_radius`,
e.g. 24) for every anchor above — nothing lost. `Lanternjaw` — purity (same `(seconds,
mode)` → identical texels; repeated draw identical); eight parts in `PartName::ALL` order
every frame with `Part::rig_part().layer == name.layer()`; hull pieces, underside and glow
on the body lattice (integer offsets and pivots); footprint bounds, `PART_EXTENT_MAX` and
`rig_radius ≤ QUERY_RADIUS_MAX` over all four modes sampled every 1/60 s for 12 s; `hunt`
periodic at 6 s and the strike accent colour absent
in the other modes; the eye blink and strike accent show intermediate values (largest
per-frame step of the eye texel ≤ 0.3 of its swing at 60 fps); overlap ownership (the near
limb over the hull at the strike, the far limb under it: pick texels and check which colour
won); no per-frame jump: mean |Δ| per painted texel between consecutive 60 fps frames in
`move` bounded (derive the bound from the wave speed, as `art_growth_clip.rs` derived its
0.188); heading (−1, 0) mirrors (1, 0) exactly about the anchor column on a flat face; the
same body drawn at a fractional anchor differs from the integer one by sub-pixel brightness
only (no texel changes by more than the bilinear weight).

E (growth pack): generic over `pack.plants` and their `transitions`: shape (`to == from + 1`,
non-looping, `plant_frames` samples, ≥ 8 distinct frames); the endpoint convention as
alpha-plane equality with the source/target stage's phase-0 sample; roots (decision 3);
nothing in row 15 for side species; the per-frame 60 fps bound of `art_growth_clip.rs`
applied to every clip; budgets (decision 6) — assert the *effective tips* are unchanged
against the constants above; the presenter plays each authored step as the documented
three-layer stamp for a cell of each band (soil, foliage, and a flooded water cell as
`art_water.rs` builds one) — the hand-built image equals the drawn one at a windy and a calm
instant, entering and leaving on the idle images; wilting through an authored 1→2 step never
samples the `fruit` clip and replays the same progress backwards; a v4-style pack with the
transitions removed keeps the masks bit for bit; canopy steps keep the radial mask unless
the pack carries a canopy clip, in which case the same playback rule holds. Write the tests
so they pass on the *current* pack (one transition) and gain coverage as B's rows land.

## Record

Fable writes `living-world-next-progress-2026-09-13.md` (commits, commands, artefacts,
limitations, next work) after integration. Workers' reports are its evidence.
