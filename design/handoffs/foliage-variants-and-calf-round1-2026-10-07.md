---
status: open
date: 2026-10-07
parent: model-variants-foliage-juveniles-2026-10-07.md
---

# Round 1: reduced-foliage variants and the loftstrider calf (candidates only)

Shared rules: `worker-rules-2026-10-07.md` (same directory) and WORKING_POLICY "Art direction".
Wrysk decides the look. This round **makes candidates for him to pick**. It does not wire anything
into the simulation, the presenter, the bake outputs in `assets/voxel-models/`, or Godot.

## Wrysk's decisions (2026-10-07, this thread)

- **Three foliage states** per browsed plant: full, browsed, stripped. The state follows the remaining
  foliage fraction and **cross-fades** between states (animation never pops).
- **Bloomcrown and umbrellafrond first.** Other browsed species follow once these two look right.
- **No interim fix.** Leave today's thinning as it is until the variants land; don't switch it to
  bottom-up.
- **Loftstrider calf:** calf proportions on the **same six-leg rig**: long legs, short neck, a big head,
  small fronds. Size and proportions blend toward the adult with age. Bring 2–3 candidates.
- Earlier, his words: foliage "should come from the bottom more likely, since thats where browsers eat
  from. and removing the crow[n] from a bloomcrown looks really bad. removing the top crown from
  umbrafronds also looks bad."

## Background

- Models are Blender-built from the simulation's numbers and baked to `.cvm`
  (`scripts/blender/bake_voxel_models.py`, `scripts/blender/organism_lineup.py`,
  `scripts/blender/animal_models.py`; format and tags in `crates/cubarium/src/voxel/model.rs`
  module docs). Plants come in crown-height steps with 2–3 variants each. Animals come in five size bins
  from newborn to adult. Foliage cells are tagged by layer and thinned per layer by a hash
  (`model::keeps`).
- Accepted looks: `art/gen/artbook/README.md` (bloomcrown keeps both crown colours, leaning pink;
  umbrellafrond geometric three tiers; loftstrider counts are Wrysk's: six legs, six eyes, six tusks,
  six fronds in 100/75/60 % sizes). `design/art-direction/` is binding.
- The Godot port (`godot-direction-2026-10-07.md`) reads the same `.cvm` models. Whatever format change
  the variants need must work for both readers.
- Headless pictures only: `blender -b`, `voxel_specimens --gpu`, `--gpu-target headless`.
- Outputs go in `/home/wrysk/wryskware/cubarium/art/gen/runs/2026-10-07-<package>/` (gitignored, so
  they stay on the desktop). Commit the scripts, not the images.

## Package A: foliage variants (bloomcrown, umbrellafrond)

1. **Diagnose first, briefly.** Why does foliage vanish from the *top* today when a plant is browsed or
   cleared? Candidates: the browse band admitting top layers, the order of `layer_stock`, or the
   crown-height step dropping when stock or wood falls (so a shorter model is drawn). Give the path with
   file:line. This decides where the state hooks in later. Don't fix it.
2. **Candidates.** Add browsed and stripped versions of each species' accepted model, with the same parts
   and palette:
   - foliage goes **from the bottom up**, browser height first;
   - bloomcrown keeps its crown while browsed;
   - umbrellafrond keeps its top tier while browsed;
   - stripped = stem or trunk plus a remnant.

   Make 2–3 candidates per state that differ in how much is removed and where. Don't pick a winner.
3. **Sheets.** One contact sheet per species: full | browsed | stripped side by side at the adult step,
   one row per candidate, plus the same in-scene through `voxel_specimens --gpu`. Absolute paths.
4. **Proposal, not code.** In ≤15 lines, say how the state could be carried in `.cvm`: extra steps, a
   state tag, or something else. Say how the selection by fraction and a voxel cross-fade would work.
   One idea: the outgoing state's cells drop and the incoming state's cells appear by the `keeps` hash
   threshold. Say what the Godot reader would need.

## Package B: loftstrider calf

1. **Today:** in ≤5 lines, how the five animal size bins are made (a scaled adult?) and what a newborn
   loftstrider looks like now. Put a picture in the sheet.
2. **Candidates:** add a calf builder next to `loftstrider()` in `scripts/blender/animal_models.py`, on
   the same rig and with the same counts. Make 2–3 candidates spanning the brief: legs long relative to
   the body, short neck, big head, small fronds; tusks may be stubs. Keep the colours unless the
   candidates differ on purpose, and say so.
3. **Sheet:** each candidate next to the adult, (a) at true relative size and (b) scaled to the same
   height. Add a strip showing how one candidate blends to the adult across the five bins. Use
   orthographic side and three-quarter views, rendered as voxels at 125 mm, plus a Blender render.
4. **Proposal, not code:** ≤10 lines on how proportions would blend by age in the bake (an interpolated
   rig per bin, or authored calf and subadult with a blend).

## Return (both packages)

≤40 lines: commits (branch only, explicit paths, never merge), sheet paths, the diagnosis or today's
state, the proposal, and anything that looks wrong.

## Round 1 results and Wrysk's picks (2026-10-08)

Round 1: plants 9e2a9356 (branch `worktree-agent-a578908e8c16e7fa7`), calf fbd3d3c8 (branch
`worktree-agent-a22f62e40bfde905b`). Sheets in `art/gen/runs/2026-10-07-{foliage-variants,loftstrider-calf}/`.

Wrysk's words: "bloomcrown: do browsed 3 crown (maybe even a bit more cut back) with browsed 2's
bottom. stripped 1 but add like one or two pink voxels at the ground so it looks like the foliage fell
off and is laying there. umbrafrond take the idea from browsed 3 but add two stages. browsedA has the
lowest tier light greeen, then browsedB makes the lowest tier look like stripped 3, but otherwise what
browsed 3 currenrly looks like for the upper two. and then used stripped 3 as stripped. Loftstrider i
feel like im not liking the long legs. shorter legs actually work for me. like more chibi. and then i
would pick the middle for the rest of the body shape."

Round 2, as read:
- **Bloomcrown**: browsed = round-1 browsed 3's chewed crown, optionally cut back a little more (show
  both), over browsed 2's stubbed rosette. Stripped = stripped 1 (stem + pod + bloom) plus one or two
  pink fallen-foliage voxels on the ground beside it (show 1 and 2).
- **Umbrellafrond gets four states**: full; browsed A = lowest tier as light-green bare ribs, upper two
  full; browsed B = lowest tier as stripped 3's rib stubs, middle tier light-green bare ribs, top full
  (browsed 3's upper two); stripped = round-1 stripped 3. So the state count is per species.
- **Loftstrider calf**: body shape from B (neck 0.30, head 0.72, fronds 0.35, tusks 0.55), legs
  **shorter**, chibi. Show a few leg lengths at or below the body's scale.
