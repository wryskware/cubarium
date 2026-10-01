# Model pass: organism models from the artbook picks

Wrysk, 2026-10-01: "go ahead and do the model pass". The artbook
(`art/gen/artbook/README.md`, visual page `index.html`, merged fd146102) records the
selected look of nine plants and seven animals. The voxel model library
(`assets/voxel-models/`, package V, `design/handoffs/voxel-organism-models-2026-09-23.md`)
was baked on 09-23 from the older body plans. This pass rebuilds the Blender builders to
the picks and rebakes. Two packages, run in parallel: **MP-P** (plants) and **MP-A**
(animals).

Read first: the artbook README, the "Selected appearance" notes in
`design/art-direction/species-dossiers-2026-09-21.md` and
`design/animal-body-reimagining-2026-09-24.md`, `design/art-direction/species-colours-2026-09-24.md`,
`art/gen/SIGNOFF.md` (animal final table), the package V handoff above, and
`scripts/blender/bake_voxel_models.py`'s header.

## Decisions (Fable, 2026-10-01; implement, don't revisit)

1. **The Blender builder is the 3D model.** It carries every part, count, proportion and
   colour zone the pick shows, at blockout fidelity: primitives, tubes and lofts as in
   `scripts/blender/animal_bodies_r2.py` v3. No sculpting, no image textures. The voxel bake
   keeps whatever survives at the voxel size; that loss is expected and is reported, not
   fought.
2. **Simulation numbers do not change.** Plants keep `source.json` crown ranges, stage
   profiles and the size-step rule; layer tags stay true to the profile bands (thinning and
   wilt read them). Simulated animals keep their adult dimensions.
3. **Colours are sampled from the picked image**: one base→tip (or belly→back) ramp per part,
   eyedropped from the pick, not invented. Write the hexes into the renderer
   (`crates/cubarium/src/voxel/colours.rs`) and into the colours doc's plant/animal tables
   as "sampled 2026-10-01 from the artbook pick". Keep that doc's `design_status: proposal`.
4. **Selected only.** Bloomcrown pink (the blue crown is not built); seedporter selected
   image (the compact-head alternate and the orchid study are not built); velvetpad from its
   provisional pick.
5. **Bake at both voxel sizes as today** (0.125 m is live everywhere).
6. **Non-simulated animals become specimens**: chorister, ripple-snail, capgnawer,
   seedporter, bellwing get baked models under a new manifest key `specimens`
   (`{name: file}`), at the sizes the roster/reimagining docs give (state each in a
   placeholder table in the bake script). They are **not** added to `Founder` or the sim; the
   library loads them name-keyed and `voxel_specimens` draws them.
7. **Out of scope:** latticevine and lanternjaw (no artbook pick); terrain candidates;
   `assets/voxel-textures` (experimental, opt-in; leave as is even if it now mismatches);
   glowcap's mycelium in the dead wood (world scenery); halo cleanup of the images; poses
   and animation (one pose, as today); live presenter paths for unsimulated animals.

## MP-P · Plants

Bloomcrown, umbrellafrond, springturf, stonecushion, velvetpad, glowcap, vaulttree,
lanternberry, siphonreed. Owns the plant builders in `scripts/blender/organism_lineup.py`,
`PLANT_ROLE*` in the bake script, and `recipe()` in `colours.rs`. Start with vaulttree and
umbrellafrond (the most voxels, the most to get right).

## MP-A · Animals

Frondgrazer and littershredder (sim founders) plus the five specimens. Owns a new
`scripts/blender/animal_models.py` (the bake imports animal builders from it; leave the
lineup's old animal functions for the lineup render), the bake's animal path and
`specimens` key, `crates/cubarium/src/voxel/model.rs` loading, `animal()` and a name-keyed
specimen colour function in `colours.rs`, and `crates/cubarium/examples/voxel_specimens.rs`.
Start from the signed-off blockouts (CH v3, RS-D2 v2, CG-D2 v2, SP-AB2, FG/LT v2, BW-A in
`animal_bodies_r2.py`) and move them to the picks. Bellwing keeps its original colours;
only its tendril becomes the thin corkscrew.

## Shared rules

- **No windows.** `blender -b` only; `voxel_specimens --gpu` is headless. AGENTS.md
  "Windows" applies.
- Both packages may append names to `model::PALETTE` (and the bake's `PALETTE`); append
  only, never reorder. Fable resolves the merge.
- Bake only your subjects (`--only`). Fable rebakes everything once at merge.
- Commit with explicit paths, in your worktree; never merge, never deploy.
- Tests: `cargo nextest run -p cubarium` must stay green; add a small test only for new Rust
  logic (MP-A's specimen loading). Looks are judged by picture, not tests.

## Evidence (in `runs/model-pass-2026-10-01/<plants|animals>/`)

- One contact sheet per subject: **pick | Blender render of the builder (3/4 view, the
  pick's angle) | voxel model at 0.125 m** (adult step / adult bin), side by side, one PNG.
- `voxel_specimens --gpu` strip at 6 px, 1080p-class, after your bake.

## Return (≤ 40 lines)

Commits; absolute paths of the contact sheets and strip; per subject one line: what the
voxel model keeps of the pick and what it loses; any part you could not place or any call
the pick did not settle (stop and ask rather than invent).
