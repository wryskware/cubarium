# Brief: textured voxel faces with multiple levels of detail (GPU renderer) (2026-09-23)

Wrysk, 2026-09-23: texture the voxels currently in the world as a middle ground before
the full Stage 2 art ("it would start to look very minecrafty instead of just being solid
voxels everywhere"); transparency where it helps voxels not read as cubes; and
**multiple levels of detail**, because the desktop draws 4 px per voxel when fullscreen
has room for ~4× that. Approved: "yes on multiple level textures".

## Standing constraints

- **GPU renderer only** (`crates/cubarium-gpu`, `shaders/voxel.frag`, plus the host
  wiring in `crates/cubarium/src/voxel/`). The **CPU renderer keeps its current look
  for good**: it's the PNG diagnostics path (Wrysk). Don't add texture support to it,
  and don't change its output.
- The Tachyon panel runs the GPU renderer at `px_per_voxel = 6`
  (`config/tachyon/voxel.toml`) and must stay 20 ticks/s and 60 fps. Don't deploy to the
  panel: Fable decides that.
- Look belongs to Wrysk (`WORKING_POLICY.md`; `design/art-direction/`). The textures
  you author are **interim placeholders** built from the existing material colours. They
  must read as their material and stay quiet. Generated textures replace them after
  the character and botany sheets are signed off.
- Development windows: `./scripts/run-voxel.sh --background` (never steal focus).
- Work in your worktree. Commit in slices with explicit paths (never `commit -a`). Don't
  merge to main.
- Short function tests only for the logic (auto px selection, LOD lookup and override,
  atlas packing). Screenshots verify the look. No full-suite runs beyond
  `cargo test -p cubarium-gpu -p cubarium` at the end.

## What to build

### Slice A: level-of-detail machinery and terrain textures

1. **`px_per_voxel = "auto"`** (the integer form keeps working). At startup, choose the
   largest integer that fits the output: window/fullscreen width for the desktop, the
   panel's raster for the panel. Only if resize rebuilding is cheap in the existing
   renderer lifecycle, also rebuild on window resize; otherwise say so. Keep
   `config/tachyon/voxel.toml` at 6. Set `config/desktop/*.toml` to `"auto"`.
2. **Texture masters at 48 × 48 px per face.** 48 divides evenly by 4, 6, 8, 12, 16 and
   24, so each level is an exact downsample. The oblique top face is drawn shorter
   than a side face (the 30° tilt / `rise`), so check `project.rs` and author top-face
   masters at the projected aspect (or state how the shader maps a square master onto
   the top face without smearing).
   - Location: `assets/voxel-textures/masters/<material>-<top|side|under>-<variant>.png`,
     indexed colour or RGBA.
3. **Derived levels.** One small tool generates every level's atlas from the masters,
   area-downsampled and then snapped to the palette. Reuse
   `art/gen/tools/palette_quantise.py` if it fits, or a Rust build step if that's cleaner.
   It writes `assets/voxel-textures/lod/<px>/…`. A hand-authored file already at
   `assets/voxel-textures/override/<px>/<name>.png` wins over the derived one. That's
   where Wrysk or the forge fixes a level that comes out mushy at 4 or 6 px.
4. **Shader sampling.** In `voxel.frag`, a visible terrain face samples its material's
   texture at the face-local texel. The variant is picked by a stable hash of the voxel
   position, so walls don't show a repeating tile. The texel modulates the material's
   base colour **before** the existing light, haze, dither, water and sky terms, which
   keep working as they do. Level = `px_per_voxel` exactly (integer, no filtering, no
   mip blending: pixel-crisp).
5. **Interim terrain masters** for every terrain material in the world (see
   `Material` and the `STRATA` colours in `crates/cubarium/src/voxel/present.rs`). Each
   is a quiet pattern within ±1–2 palette steps of today's colour: rock with faint strata
   and cracks, soil with grain, and so on, with 4 variants per face. **Soil side faces
   get a grass-and-litter fringe hanging over their top rows** (the grass-block trick)
   where the top face is exposed. Terrain stays **opaque**.

### Slice B: organism voxels

6. Baked organism models (`organisms = "models"`, cells tagged trunk, foliage layer i,
   drape i or accent; see `scripts/blender/bake_voxel_models.py` and
   `crates/cubarium/src/voxel/model.rs`) sample a per-role texture the same way.
   **Foliage and drape cells use cutout transparency**: texels marked transparent show
   whatever lies behind (the next voxel along the ray, or terrain), so crowns read as
   leaves, not cubes. The existing glyph path already has `TONE_TRANSPARENT`; reuse its
   idea. Trunk and accent cells stay opaque. The texture tints by the organism's existing
   style colours, so the colour pass (`colours.rs`) still decides hue.
7. Interim foliage masters: a leaf-cluster cutout (roughly 55–70 % opaque) per foliage
   role, and a bark grain for trunks.

## Verification (return evidence, not claims)

- Screenshots, GPU path, same seed and tick, before/after, saved to
  `runs/voxel-textures-2026-09-23/`:
  - the desktop terrarium at `px_per_voxel` 4 and at auto (state what auto chose at
    2560 × 1440 or your fullscreen);
  - the panel config (6) rendered on the desktop GPU;
  - one close crop per level (4, 6, 12, 24) of a soil edge with its grass fringe and a
    bloomcrown crown.
- Frame time (GPU timestamp queries already exist) before/after at 4, 6 and auto.
- A CPU-renderer PNG before/after showing it unchanged.
- Rerun `python3 art/gen/tools/build_index.py` so the screenshots show up in the index.

## Return (≤ 30 lines)

Slice commits; what auto chose and how; how the top face is mapped; the frame times;
where the level tool and overrides live; what doesn't look right yet (a Wrysk look
question, not yours to settle); which screenshot to open first.
