---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Authored growth for the two canopy plants (umbrellafrond, bloomcrown)

Evidence for root's reconciliation, not a decision. The owner-priority item after the
Lanternjaw continuity commit (`12c41aa`): the two top-down canopy species were the only
plants still growing by the runtime's radial reveal. They now carry `grow01` and `grow12`
like every side species (pack v5 schema, no new schema or renderer feature; the presenter's
existing three-layer path plays them). Contract text: `art/PLANTS.md` "The four canopy
steps".

Owned and changed: `art/plants/umbrellafrond.tscn`, `art/plants/bloomcrown.tscn`, ten new
pixel-exact sub-parts in `art/parts/` (with their `.svg.import` sidecars), the splice script
`art/plants/author_grow.py` (`canopy_parts`, `umbrellafrond`, `bloomcrown`, ext-resource
support, `flip_v`), `art/PLANTS.md`, the regenerated `assets/atelier/plants.png` and
`pack.json`, the loader's expected list in `crates/cubarium/src/art.rs`, the growth tests
`crates/cubarium/tests/art_growth_pack.rs` and `art_growth_clip.rs`, and — narrowly — one
presenter unit test in `art_present.rs` whose premise ("no canopy species has a clip yet")
the package makes false, plus the matching doc-comment parenthetical. No core, care,
snapshot, runner, hunter, README, or `art/README.md` edit; nothing live, no state, no
deployment. `art/README.md` still says the two radial species keep the reveal masks
(lines "Authored growth" and "Any step the pack has no clip for"); root's file, flagged
below.

## What was authored

No part is scaled or faded as a whole sprite. Because the stage art is single sprites, each
was **cut into sub-parts pixel for pixel** — same colours at the same offsets, nothing
repainted — and `canopy_parts()` asserts the pieces of a stage partition it exactly, so a
clip's last frame *is* the stage art and the palette and silhouettes are untouched:

| stage part | pieces |
| --- | --- |
| `umbrellafrond frond1` (11×11) | `ribs1` (mint ribs + cyan tips, 9×9 cross), `blades1` (dark edge + teal, 9×9) |
| `umbrellafrond frond2` (15×15) | `cross2` (cardinal ribs, 13×13), `diag2` (diagonal ribs, 9×9), `blades2` (15×15) |
| `bloomcrown petals1` (11×11) | `petal1` (one 3×3 petal, drawn four times flipped); the dark cross under the centre is covered by `center1` |
| `bloomcrown petals2` (15×15) | `lobe2` (4×3 side lobe ×4), `tip2` (3×4 north petal ×2), `waist2` (2×1 notch ×2), `core2` (3×5 body under the crown) |

- **umbrellafrond 0→1**: the mint ribs come in under the sprout at half length (at scale 0.5
  the 9 px cross samples exactly the sprout's mint diamond) and extend to 9 px with their
  cyan tips; the blades unfurl out from the ribs as two opaque copies squeezed along one rib
  axis each, spreading sideways; the cyan centre lights last.
- **umbrellafrond 1→2**: frond1 holds underneath at full opacity to the last sample; the
  cardinal ribs come in at 9 of 13 px and push out; four diagonal ribs sprout from the
  centre; the blades first lengthen with the cross then spread sideways.
- **bloomcrown 0→1**: the warm centre swells over the sprout (same alpha plane) as a bud
  whose four petals show only their dark corners round it; the petals slide out diagonally,
  NW/SE leading, NE/SW after.
- **bloomcrown 1→2**: the small crown stays on top; the body core forms under it; four side
  lobes form over the old petals and slide one pixel outward; two new petals push out north
  and south from under the crown; the notches light; the 5 px crown cross-fades over the
  3 px one last. The `Fruit` part is never used; the only warm pixels are the bloom centre
  the stages already carry.

Two things the strips corrected before the bake was kept: the petals at their first
position showed a faint outline outside the crown (start moved to the exact centre, fade
finished before the slide); and a nearest-sampled scale-up of the 5×5 crown ring loses the
ring for a sample and reads as a solid warm fruit, so the crown opens by cross-fade instead.
A first waist placement was one pixel off (caught by the last-frame diff against stage 2).

Conventions kept from the side species: 4 s, `LOOP_NONE`, own hidden `Grow<from><to>` group,
every new property's neutral in `RESET`, endpoints RESET-neutral (the centre at modulate 1:
each endpoint's alpha plane equals the neighbouring stage's phase-0 sample, its colours
differ only by that sample's dimmer centre), `grow01`'s last frame byte-identical to
`grow12`'s first, nothing fades out before its replacement is opaque (the old bloom and
frond are dropped only when covered; the only outgoing fade was removed).

**The centred rule.** "Roots stay planted" has no root row on a radial plant; its anchor is
the tile centre. The rule becomes: every frame paints the centre pixel, the painted footprint
stays centred on it to within one pixel in each axis, and the painted reach lies between the
two stages' own. This is encoded in the canopy branch of the generic root test rather than
by skipping canopy (which the test did before).

## Verification (foreground, exit 0)

Bake and atlas, with Godot 4.7.2 (`/usr/bin/godot`, the reference baker):

- Baseline first: the unchanged tree baked into a temp dir reproduces the shipped
  `assets/atelier` byte for byte (five atlases and `pack.json`).
- With the clips: baked **twice** into `/tmp/bake-canopy-WLTYqX` and
  `/tmp/bake-canopy-2mZ2EA`; all five atlases and `pack.json` identical between the two
  bakes, and `assets/atelier` is a copy of the first.
- All 34 pre-existing plant rows pixel-identical to the shipped atlas (matched by name,
  stage, from, to; only row indices shift for reedspire's rows 29–33 → 33–37); 4 new rows
  (38 total, `plants.png` 384×608); `creatures.png`, `ground.png`, `habitat.png`,
  `tall.png` byte-identical; `pack.json` identical apart from the plant rows.
- Per clip, checked on the atlas: joins exact (0 visible pixels differ between
  `grow01[last]` and `grow12[first]`); last frames differ from the stage phase-0 samples only
  by the centre's modulate (5 px umbrellafrond, 5 and 21 px bloomcrown, all same-hue
  dimming); no transient hole (a pixel painted by the previous and the last frame but not
  by this one) in any clip; centre pixel painted in every frame; footprint asymmetry ≤ 1 px.

Tests, run in an isolated `git archive HEAD` copy (`/tmp/cubarium-fable-canopy-ydkBiN`)
with the owned files overlaid, because the shared checkout
carries another worker's in-flight core/care edits that break `runner.rs` compilation until
they finish (nothing of theirs was touched or built):

| target | result |
| --- | --- |
| `cargo test -p cubarium --test art_growth_pack` | 12 passed, 0 failed, 1 ignored (the capture) |
| `cargo test -p cubarium --test art_growth_clip` | 9 passed, 0 failed |
| `cargo test -p cubarium --lib --tests` (whole host crate, `--no-fail-fast`) | 513 passed, 0 failed, 15 ignored |

What the generic growth tests now cover for the canopy species, with no new schema: the
endpoint alpha-plane and silhouette-envelope test; the 60 fps derived per-frame motion bound
(`every_authored_step_moves_less_than_a_fraction_of_a_baked_sample_per_frame_at_60_fps`,
driven by a fed top-face cell through the climb); the documented three-layer playback at a
windy and a calm instant — extended with a **radial branch**: a top-face species with
`spin_deg > 0` must never bend and must be turned off its slot heading exactly at the windy
instant (both canopy species are now required to exercise it); reversal (wilting replays
the growing frames backwards, exact `t`); the fruiting plant wilting into 1→2 now on the
authored path (0 lingering accent frames for bloomcrown); the fallback comparison; row
placement; and the footprint/wind test: every frame's extent ≤ 9 px and the measured bend
budgets unchanged against the recorded 0.33 (umbrellafrond) and 2.07 px (bloomcrown) — the
`assert!(new >= old − 0.005)` is unconditional, so this is proven, not assumed.

New, top-specific: the centred branch above (every plant's two steps are now checked, side
by root row and canopy by centre; the count is asserted), and
`a_canopy_step_on_the_rim_of_the_top_face_opens_as_one_stamp_on_two_faces`: both clips of
both species at a rank-2 slot on the rim of the top face (umbrellafrond at Top cell (10, 0),
bloomcrown at (1, 15)), windy instant, hand-built expected image equal bit for bit, light on
the top face and a side face, every channel in `[0, 1]`.

The presenter unit test `a_radial_slot_with_a_clip_plays_it_unmasked_too` used to lend
lanternstalk's clip to umbrellafrond; it now plays the shipped canopy clip and compares
against the same pack with that plant's clips stripped (same assertions).

## Captures

- 8× strips of all four clips, 24 frames each, labelled with time:
  `/tmp/canopy-growth-OSDlYt/strips/*.png` (copied from `/tmp/canopy-strips/`; regenerate
  from the atlas rows).
- World-driven fixture at 60 fps: `capture_the_canopy_steps_as_native_frames` (ignored,
  `CANOPY_CAPTURE_DIR`) feeds the two interior rank-2 top-face slots (umbrellafrond at Top
  cell (9, 4), bloomcrown at (3, 3)) through sprout → 1 → 2 by the authored clips, holds,
  then starves them back down through the same clips, three frames per tick, 400 ticks, 1200
  native 256×128 net frames with a manifest of each slot's step and `t`. Sheet:
  `captures/canopy-growth.png` (gitignored) and `sheet.png` beside the frames in
  `/tmp/canopy-growth-OSDlYt` (with `sheet.py`); the presenter's in-place wind spin is
  visible on both.

## Limitations

- The clips are read at native 64 px on a screen crop; nothing here is evidence of
  room-distance LED legibility.
- Growth pacing, hysteresis, and the fruit accent are unchanged and remain the presenter's;
  the clips are pictures of a step, not an age or a resource state.
- Ten new SVG parts were added (all pixel-exact cuts of existing art); a reviewer who
  wanted zero new files would have to accept whole-sprite scaling instead, which the brief
  forbids.
- `art_growth_pack.rs` was not rustfmt-clean before this change (import order); it was not
  reformatted, per the no-broad-rewrite rule, so `rustfmt --check` still reports the
  pre-existing hunks.
- `art/README.md` (root's) still describes the two canopy species as keeping the reveal
  masks; one sentence to update.
