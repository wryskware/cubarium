# Brief: latticevine visual, a tile layer (2026-09-24)

Wrysk, 2026-09-24: "we can merge, but regardless we need to do something about its
visual". The latticevine sim is on main (`9afdccc`, flora schema 6): covered rock faces
with owner, leafiness, rooted and dormant flags and spur phase, exposed through
`FloraView::cover` (`Cover::draw` → `FaceDraw`) in `crates/cubarium-voxel-flora/src/cover.rs`.
Nothing draws them yet. Spec: the dossier
`design/art-direction/species-dossier-D15-latticevine-2026-09-23.md`, Revision 2
("The presenter: a tile layer"). The signed-off look is the **dense smooth Blender model**
(`scripts/blender/latticevine.py`, renders in `runs/latticevine-model-2026-09-23/`).

Two workers in parallel, joined only by the file interface below.

## Interface: vine tile masters

`assets/voxel-textures/masters/vine/`, RGBA PNG, 48 × 48, straight alpha (cutout:
alpha 0 or 255), **direct colour** (the dossier hexes; not a tint mask). Each is seen
face-on as the vine lies on a vertical rock face, row 0 at the top.

- `vine-<density>-<mask>.png`
  - density is `full` (leafiness ≥ 0.66), `thin` (≥ 0.2) or `bare` (below that, and
    all dormant faces): runners and holdfast pads only.
  - mask is one hex digit giving the covered neighbours in the face's plane: bit 0 up,
    bit 1 right, bit 2 down, bit 3 left (viewer's right on the face). Runners leave the
    tile through the edges whose bit is set, and the cover's edge is ragged where a bit
    is clear.
  - 3 densities × 16 masks = 48 files.
- `vine-root-<density>-<mask>.png`: the same for rooted faces. The root arch sits at the
  tile's bottom edge for a climbing root, or the top for a hanging root. If one drawing
  can't do both, add `-hang` variants and say so. 96 files for both.
- `vine-accent-bud.png`, `vine-accent-flower.png`, `vine-accent-fruit.png`: overlays
  drawn over the tile only while the face's spur is in that phase. Bell pale lilac with
  a cyan mouth, a round magenta bead bunch; mostly transparent, one accent near the
  tile's centre with a little jitter room.
- The smaller levels are derived by the existing texture tool (and can be overridden),
  the same as terrain.

## Worker 1: Blender tile renderer (the latticevine.py worker)

Add a `--tiles OUT_DIR` mode to `scripts/blender/latticevine.py` that renders every
file above from the same builder and materials as the dense model. Use an orthographic
camera face-on to a 0.125 m square of face, 48 px, flat shading, transparent film,
alpha thresholded to cutout. Keep runner and clump density matched to the dense
variant. Make the runners meet the tile edges at consistent positions (for example the
middle third of each edge), so neighbouring tiles join. Write `assets/voxel-textures/masters/vine/`
plus a contact sheet `runs/latticevine-model-2026-09-23/tiles-sheet.png` (the whole
set, and a 6 × 6 test wall assembled from random masks, one assembled at 12 px and one
at 48 px). Commit on your own branch from current main, with explicit paths. Return ≤ 15
lines.

## Worker 2: presenter layer (the texture worker, on `voxel-textures-lod`)

1. Merge current main (`9afdccc` or later) into `voxel-textures-lod` first, and resolve
   conflicts.
2. **GPU renderer:** every covered face draws as a cell in the **air voxel in front of
   the face** (below it, for an underside). The cell samples the vine tile for its
   (rooted, density, mask), with the neighbour mask computed from `Cover` adjacency in
   the face's plane. The accent overlay goes on top while the spur is in bud, flower
   or fruit, and at no other time. It uses cutout alpha like the foliage textures, and
   lighting, haze and dither as for foliage. It uses direct colour, not a style tint,
   and interim textures until worker 1's masters land; generate simple interim ones
   under the same names so the layer runs on its own. Only faces the camera can see
   need to look right; say how you handle the ±x side faces in the oblique view.
3. When a vine cell and an organism model or a ground stand claim the same air voxel,
   the organism wins. Say what you did.
4. **CPU renderer:** draw covered faces as a flat dark-violet cell (bare) or blue cell
   (leafy) only, with no textures and no accents. The diagnostics PNGs show cover, and
   that's all.
5. **Evidence:** screenshots in `runs/latticevine-visual-2026-09-24/`:
   - a hand-built fixture wall with a climbing and a hanging vine, mixed densities,
     one dormant vine, and faces in bud, flower and fruit, at 4, 6 and 12 px;
   - a terrarium world where vines are visible (plant extra founders in the fixture
     if the defaults are too slow to show cover);
   - GPU frame time before and after.
6. Tests: short function tests for the mask computation and the tile and accent
   selection (accent only in its phase; dormant → bare). No long runs.
7. Commit on `voxel-textures-lod` with explicit paths. Don't merge to main or deploy.
   Return ≤ 25 lines.

## Standing rules

GPU only for art; the CPU renderer stays basic (the flat cells above are the whole CPU
change). Use `./scripts/run-voxel.sh --background`. Desktop CPU is capped at about 50 %
for agents. Look choices belong to Wrysk; state yours.
