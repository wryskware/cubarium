---
status: open
date: 2026-09-22
owner: Fable (orchestration); one Opus worker, high effort
---

# Package 1b: bodies, eyes and mouths in metres; shade area in square metres

## Why

Decisions §1, §2, §6 and §7 (`voxel-organism-decisions-2026-09-21.md`) and
the audit's package 1 (`design/7_Research/organism-systems-audit-2026-09-21.md`
§1, §10). After the step rule (`voxel-founder-step-2026-09-22.md`) the route
no longer binds on small and default; reach does, and reach is computed from
an animal that is half metres and half voxels: the eye sits at a fixed
1.5 cells, the mouth accepts crown *layers* counted from the head cell, and
canopy shade divides foliage by an area in cells², so the same plant shades
four times less when the cell halves. This package makes the founders one
animal on every cell size and fixes the shade units. It does not change
what the fan looks at (pitch set) and does not add layers to plants; those
are the retrain (package 5) and the layers package (2).

## The rules

**Body.** Each founder lineage has adult dimensions in metres on its
physiology (not the `Manifest`, whose fields are in the trained-policy
digest): browser length 0.375, width 0.1875, height 0.1875; shredder length
0.19, width 0.0625, height 0.0625. A living body's dimensions are the adult's
× (body / body_max)^(1/3), so a newborn browser is 0.46 of adult length.
Footprint radius is width / 2; headroom is height. The presenter draws the
model body (retire the 2× shell in `crates/cubarium/src/voxel/animal.rs`).
The manifest's `body_length_m` / `body_width_m` stay as the trained
contract's record and stop being read for geometry; say so on the fields.

**Standing surface.** `Site.y` is the support cell; the surface is
`(y + 1) · voxel_m`. Every anchor below is measured from it.

**Eye.** At 0.8 × height above the surface, replacing the `standing_y + 1.5`
cells origin in `cone_readings`. Pitch set, sectors, yaw offsets, range and
classes unchanged: the observation vector keeps its 37 inputs and its digest,
so the shipped centres load. Record in `assets/policies/README.md` that they
were trained with the old eye and await package 5.

**Mouth.** A band `[0, 1.33 × height]` above the surface, and a horizontal
reach of 0.25 × length ahead of the footprint (as today). A stand is
reachable when its foliage slab (today: the crown cell at
`site.y + crown_voxels(wood)`, one cell thick, radius `crown_radius(wood)`;
the encounter contract's slab) intersects the band vertically and the reach
horizontally. This replaces `mouth_reach_up_voxels` and the head-layer
acceptance in `mouth_foliage_stand(s)` / `mouth_columns`; the encounter
helpers from package 0 and the observer read the same band. The manifest's
`mouth_reach_up_voxels` stays as a recorded field and stops being read.

**Contact.** Receptor positions at 0.5 × height above the surface, in
metres; the wall test of package 1a otherwise unchanged.

**Shade.** `light_per_stand` (`crates/cubarium-voxel-flora/src/step.rs`)
attenuates by foliage over the crown's *physical* area in m², with a physical
floor equal to one reference cell (0.25 m)². Keep the 0.25 m reference world
numerically identical (state the conversion of `shade_k`), so only finer
grids change, and they change to the physically right value.

**Placeholders** (backlog §1): the 0.8, 1.33 and 0.5 height fractions, the
0.25 reach fraction, the adult dimensions.

## Deliverables

1. Tests, ≤200 ticks each, authored from this brief before implementation:
   newborn and adult dimensions; the eye height in metres is equal on a
   0.125 m and a 0.25 m fixture; a crown slab at 0.20 m above the surface is
   reachable on both cell sizes and one at 0.30 m is not; horizontal reach;
   a body cannot enter a void shorter than its height; shade under one crown
   is identical on the 0.25 m reference grid before and after, and equal in
   physical terms between 0.125 m and 0.25 m after; the built-in centres
   still validate (digest unchanged); ledgers conserve across a bite.
2. The rules above, in `body.rs`, `senses.rs`, the encounter helpers, the
   presenter and `step.rs` (flora).
3. Observer and autopsy read the implemented band (the "decided" columns
   become the live ones; drop the duplicates).
4. Fauna snapshot schema bump if any serialised state changed (fresh worlds
   only; refuse old ones).
5. Measurement, after: `voxel_edible_stock 0 preset=small|default|wide`
   (reachable and route-connected fractions before → after) and
   `voxel_founder_autopsy 60 preset=small` and `preset=default` (deaths by
   cause, extinction minute, bites, alive at 60). Append to the census note
   as "Bodies in metres, 2026-09-22".

## Constraints

- No knob tuning beyond the placeholders listed; no golden hashes; fresh
  worlds; tests ≤200 ticks.
- Worktree `.claude/worktrees/body-anchors`, branch `body-anchors` from main;
  `CARGO_TARGET_DIR` inside it. Explicit-path commits only; every commit ends
  with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Never touch `design/handoffs/README.md`, `design/README.md`,
  `config/tachyon/*`, `scripts/tachyon-*`, `docs/tachyon.md`, `art/gen/*`,
  the shipped policy assets' JSON, or the digest's field set.
- `graft ask "<question>" --source` before opening files; `graft callers
  mouth_foliage_stand --depth all` and `graft callers cone_readings --depth
  all` before changing them. No windows.

## Verification

`cargo nextest run --workspace --exclude cubarium-gpu` green; the tests in
deliverable 1; Fable re-runs one arm.

## Return (≤40 lines)

Before/after reach and route table, the two autopsy arms, what the presenter
now draws, the shade conversion, the commit list, and anything the rules
could not express.
