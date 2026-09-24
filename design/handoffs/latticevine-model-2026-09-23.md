# Brief: latticevine Blender model + voxelization (2026-09-23)

Wrysk: "go ahead and do the blender for latticevine … i'd rather see it modeled in
blender and voxellated than iterate on the finicky image model." He did **not** like
round 3 (`art/gen/runs/2026-09-23-latticevine/contact-sheet-r3.png`): "it looks too much
like O1", i.e. sparse, wiry, separate little trees. He likes the flowers, the beads and
the colours (all rounds); the diamond lattice was "not plant-like"; flowers and fruit
grow from anywhere on the plant, not just the edges.

Spec: `design/art-direction/species-dossier-D15-latticevine-2026-09-23.md`: niche, sizes,
hexes, phase table, "Revision 1" and the O4 subsection (braided fan with rosette tips).
Read it fully. This is a **look study**: nothing is wired into the simulation (the sim
has no wall-mounted stands yet; see the dossier's model requests).

## Deliverable

1. `scripts/blender/latticevine.py` (Blender 5.2 headless: `blender -b -P …`). A
   procedural builder, seeded, that grows the vine on a vertical rock face:
   - **Several runners** start from separate points along the foot, each from a small
     root arch (the dens). Some **braid** partway up (runners twisting around each
     other), then split again in Y-forks, each fork thinner. The spread is
     **lopsided**, following a diagonal. Holdfast pads sit where runners touch the rock.
     Everything is pressed flat to the face, except the lowest runners, which bow out
     about 1 voxel into bowers.
   - **Rosette clumps** of shingle scales at the tips and along the upper runners.
     Aim for **ivy density, not O1's sparseness.** Build two coverage variants, one
     about 50 % and one about 80 % of the spanned face, with rock showing in irregular
     gaps.
   - **Spurs everywhere, interior included:** hanging bells (pale lilac, cyan mouth) and
     round magenta bead bunches, and some bare stubs and buds. Scatter the phases.
   - The four growth sizes from the dossier (newborn 1×2, young 3×5, grown 6×10, full
     10×16 voxels of cover at 0.125 m).
   - Dossier hexes as flat materials, one material per role (runner old/young, pad,
     shingle ramp, bell, bell mouth, bead, rock), so the voxelizer can tag roles.
2. **Voxelize** at 0.125 m (the world voxel) with `voxelize()` from
   `scripts/blender/bake_voxel_models.py`, its accent rule included (bells, beads and
   mouths are sub-voxel accents). Import or reuse it; don't fork the rule. Also do
   0.25 m if it costs nothing.
3. **Renders** in the panel's elevated-oblique look (see `organism_lineup.py` /
   `animal_bodies.py`: Workbench, flat, the shear-parent oblique camera), all on the
   rock face:
   - smooth model: full size, both variants, plus the four-size growth row;
   - voxelized model: the same shots, cubes coloured by role;
   - one **true-scale** shot beside the frondgrazer reference and a bloomcrown from the
     lineup builders, so size reads against the world.
   If the real presenter can draw a baked CVM from a scratch directory **without code
   changes** (`cargo run --example voxel_specimens`, manifest format in
   `crates/cubarium/src/voxel/model.rs`), add one presenter render. Otherwise skip it and
   say why. Never write into `assets/voxel-models/`.
4. Output in `runs/latticevine-model-2026-09-23/`: PNGs, the `.blend`, and a short
   `README.md` (what each PNG is, the command lines). Then run
   `python3 art/gen/tools/build_index.py`.

## Constraints

- Colours and parts come from the dossier. Where it's silent (braid twist count,
  clump sizes), choose and state the choice in the README; those are look choices that
  Wrysk will judge.
- Desktop CPU is capped at 50 % for agents: keep Blender renders modest (Workbench
  only). ComfyUI jobs may be running concurrently from another worker; don't touch
  ComfyUI.
- Do not commit. Do not edit `organism_lineup.py` or `bake_voxel_models.py` beyond
  importing from them. If import needs a small refactor, stop and say so.

## Return (≤ 25 lines)

What you built and how the braid and density read; the voxel counts per size at
0.125 m; whether bells and beads survive voxelization; the look choices you made; which
PNG to look at first.
