---
status: open
date: 2026-10-07
owner: new thread with Wrysk in the loop (art)
---

# Model variants: browsed foliage and the young loftstrider

Wrysk, 2026-10-07: "foliage currently disappears from the top of the plant when its being
eaten or cleared. this doesnt really look right. it should come from the bottom more
likely, since thats where browsers eat from. and removing the crow[n] from a bloomcrown
looks really bad. removing the top crown from umbrafronds also looks bad. either we cut
away bottom folliage, or we go design new models for reduced foliage variants (kinda my
preference)." And: "for models: loftstrider needs a juvenile form." And: "art stuff needs me
in the loop."

Wrysk decides look. Agents generate candidates, show them, and implement his picks
(WORKING_POLICY "Art direction"; `design/art-direction/`).

## Answered 2026-10-07

Three states (full/browsed/stripped) with a cross-fade by foliage fraction; bloomcrown and
umbrellafrond first; no interim bottom-up cut, so wait for the variants; calf proportions on the same
rig, blended with age. Round 1 (candidates only): `foliage-variants-and-calf-round1-2026-10-07.md`.

## Questions for the thread

1. **Reduced-foliage variants.** How many states per species (for example full, browsed,
   stripped)? Which species first: bloomcrown and umbrellafrond, then the other browsed
   ones? Is the variant chosen by the remaining foliage fraction, with a cross-fade between
   states (animation never pops)? Does clearing (not browsing) use the same states?
2. **Interim.** Until variants exist, cut from the bottom of the canopy instead of the top
   (a mechanism-only change)? Cheap, but Wrysk prefers variants.
3. **Young loftstrider.** Proportions (a calf: long legs, short neck, bigger head?), how
   many stages (calf, subadult), blending size and proportions with age, whether it shares
   the adult rig.

## Where things are

- How the presented model loses foliage today: `crown_height_voxels`
  (`crates/cubarium/src/voxel/stand.rs:1537`), flora `crown_voxels`
  (`crates/cubarium-voxel-flora/src/lib.rs:1695`), and the browse band in
  `crates/cubarium-voxel-sim/src/habitat.rs` (a browser face is admitted by a stocked layer
  in its band, not the crown top). Use `graft ask`.
- Model pipeline: the 2026-10-01 model pass (merged 643ec6d6) built every model from artbook
  picks (`art/gen/artbook/`). The loftstrider was selected and baked as a specimen.
- Precedent for plant states: the canopy growth clips (pixel-exact sub-parts, the centre
  stays put, crowns cross-fade rather than scale).
- How juveniles are drawn today (a scaled adult?): check the animal presenter
  (`crates/cubarium/src/voxel/animal.rs`).
