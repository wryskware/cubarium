# Lanternjaw: next production slice

Working proposal, 2026-09-12. Wrysk prefers Fable's Lanternjaw; that preference
does not settle the following engineering or ecological choices. The gallery
is still the reference study, not a runtime creature.

## Preserve the chosen silhouette

Keep the low segmented hull, cyan spine lanterns, tail fan and folded raptorial
forelimbs. Do not shrink the whole body into a common creature's tile simply to
fit the current exporter. Keep quiet resting poses and eased short accents.
The offspring study illustrates a possible cocoon, not a free-spawning effect.

## Why this needs a separate rendering slice

`crates/cubarium-render/src/sprite.rs` rejects sprite support extending beyond
9 pixels from its anchor, including filtering support; the current atelier pack
uses 16×16 tiles with pivot (8,8). Lanternjaw's long body and extended forelimbs
do not fit one such stamp. The surface library's broader maximum radius is not
permission to remove the sprite renderer's tested budget.

Proposed implementation: bake a small multipart rig (rear body/tail, front
body/head, articulated limbs), each with its own bounded pivot and offset from
one body anchor. Transport each offset and heading through the existing surface
geometry. Preserve explicit painting order and avoid duplicate overlap pixels.
Do not draw separate face-local copies or add hardware mapping corrections.

First acceptance is an isolated scripted art study, with these checks:

- Flat-face renders match the chosen native-pixel shape through rest, move and
  the entire strike; each part, including interpolation support, fits its budget.
- Continuous side edges, top edges, all top vertices and bottom-rim reflection
  preserve registration, pose direction and pixel ownership. Check fractional
  positions and several headings, not only horizontal travel.
- A repeated draw at the same instant is identical. Intermediate presentation
  frames, pause and resume do not restart motion or produce blink cuts.
- Capture native frames and measure the actual extra frame cost with one and
  two bodies. Desktop captures do not replace physical-cube observation.

The presenter's `rig_of` uses inherited form indices when present in the pack.
Adding a fifth rig must not silently change unrelated saved organisms whose
previously unavailable form index used the hue fallback. Audit the copied world
and keep the first study explicit, outside the production pack and founders.

## Ecology follows separately

After rendering validation, use copied worlds for one fixed hunter with paid
pursuit, attack, meals, digestion and single-offspring reproduction. Compare
prey recovery and all resource transfers against matched controls. An idle pose
or a scripted strike is not evidence of hunting capability. No free predator,
automatic rescue or guaranteed resident pair is implied by the preferred art.

The broader candidate ecology remains in
`design/7_Research/care-and-megafauna-proposal-2026-09-12.md`. Optional care and
its durable live rollout remain the current implementation package; this note
preserves the next direction without adding an unfinished renderer migration.
