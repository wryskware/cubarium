# Presentation plan: per-species textures, light and shadow, water (2026-09-24)

Wrysk, 2026-09-24, after running the texture branch:

- The latticevine tiles look "way better in practice than the voxel shape".
- "All foliage has the exact same model rather than being created and tailored to that
  plant."
- "Another big problem with the new textures is that there are no shadows. That's
  probably the single biggest win we could implement now for visual quality: shadows and
  lighting … I'm fairly confident in your ability to roll your own."
- "Nice water shaders so it doesn't look like a solid pixel blob and maybe has some
  reflections or natural wateryness."

Main is at `a8ed4d5`, with textures (opt-in), the freeze fix and the latticevine
visual all merged.

## Standing constraints

- **Textures stay experimental** (`textures = false` by default). Plain voxels remain
  the main look, so every package below must also improve, or at least not break, the
  untextured picture.
- **Pixel art is still the look.** Lighting shades the px-per-voxel picture and never
  replaces it with smooth 3D. Cel steps: two tones plus a highlight (kit v0.4). No
  blur that dissolves pixels.
- **The ecology's light model is the source of truth** (backlog §5): the picture may
  shade more finely than the model's sky-visibility fan, but never differently.
- **Tiers:** a `lighting = "flat" | "lit"` key. The Tachyon panel and the cube keep
  `flat` (backlog §5) unless Wrysk says otherwise after a measured board test.
  Desktop defaults to `lit`.
- **GPU only.** The CPU renderer stays basic.
- Desktop target: 60 fps at auto px (9–13) with `lit` on. The desktop window's frame
  readback currently caps 13 px at about 22–26 fps (package P).
- Fixed elevated-orthographic camera: visibility and many light terms can be
  precomputed per voxel and updated only when terrain changes.

## Packages

### S: per-species texture sets (small; unblocks the Codex art pass)

Look up organism face textures by **species and role**:
`assets/voxel-textures/masters/species/<species>/<role>-<face>-<variant>.png`,
role ∈ bark, leaf, leaf1…, drape, accent; face ∈ top, side. Anything missing falls back
to the generic set. Species-specific masters use direct colour (as the vine tiles do).
Generic ones keep tinting. Also make the `voxel_specimens --gpu --textures DIR` preview
accept a scratch directory that has only some species. Contract doc: the module doc of
`crates/cubarium/src/voxel/textures.rs`, plus `design/handoffs/art-pass-codex-2026-09-24.md`.
Also lift the vine tiles' colours: full tiles are mostly `#1E2798` and vanish against
rock. That's a mechanical palette-shift pass on the tile generator.

### L: light and shadow (the big win)

In order, each step with screenshots before and after:

1. **Voxel ambient occlusion.** Darken the texels of each visible face by how many of
   its edge and corner neighbours are solid (the classic 4-corner voxel AO), quantised
   to the cel steps. Cheap, and it immediately separates terraces, crevices and crown
   interiors.
2. **Sky light.** Use the ecology's sky-visibility per voxel (the same hemisphere the
   flora reads light from, in a 3D texture refreshed on `terrain_version` change) as
   the ambient term. Overhangs, caves and deep canopy go dim exactly where the model
   says they're shaded.
3. **Sun and cast shadows.** A configurable sun direction. From each lit face texel,
   march a shadow ray through the voxel volume (terrain solid; organism cells
   partially transmissive by foliage fullness, so canopy casts **dappled** shade, and
   cutout holes let light through). Cap the march length. Use a coarse occupancy mip if
   the march is too slow, or precompute per-voxel sun visibility when the terrain
   changes and march only for the moving organism cells. Hard-edged pixel shadows, one
   cel step darker.
4. **Emissive bioluminescence.** Parts the art direction marks as glowing (glowcap,
   bloomcrown core, lanternberry lanterns, latticevine bell mouths, sense patches)
   emit. A low-resolution light volume (e.g. ¼ res, propagated a few steps) adds
   coloured local light to nearby faces, and a restrained pixel bloom on the emitters
   themselves (art direction: glow supports forms, never haze over them).

### W: water that reads as water (after L; same shader)

The water boundary must stay exact (art direction). Within it:

- **Depth absorption:** colour and opacity from the depth along the view ray. Clear
  shallows show the bed, deep water goes dark indigo.
- **Surface:** an animated normal from tiled noise on sim time, plus rain ripple rings
  while raining. It drives a **Fresnel** mix toward a reflection.
- **Reflection:** march the reflected ray through the voxel volume a short distance
  (terrain, crowns, sky gradient). A mirror image of the far bank in still pools.
- **Refraction:** offset the bed sample by the surface normal.
- **Shore foam** where depth is small, and pixel **glints** on ripple crests,
  quantised to the palette.
- Waterfalls and moving water streak along the flow if the water solver exposes it
  cheaply. Otherwise skip that and say so.

### P: direct GPU presentation on the desktop (optional, perf)

The desktop window reads every frame back to the CPU (minifb). Presenting straight
from the GPU swapchain removes the 22–26 fps ceiling at 13 px, which `lit` fullscreen
needs. The panel path is untouched.

## Order and workers

- **Now, in parallel:**
  - Stage Q art (sprite-forge, local Qwen): redo the plant sheets with the four fixes,
    and per-species texture exploration swatches.
  - **S** (one worker, medium).
- **Then:** **L** (one worker at high effort, checkpoint after each numbered step), and
  **W** after L in the same worker, since both live in `voxel.frag`. **P** can run
  alongside L in another worker (sink and window code, not the shader).
- **Codex art pass (Wrysk's thread):** starts on subjects Wrysk marks `final` in
  `art/gen/SIGNOFF.md`. S should land first, so finals can be previewed per species.

Every package ends with the same evidence: screenshots (same seed and tick) at 6 px and
auto, plain and `--textures`, frame times, and the panel config at 6 px on the desktop
GPU to show `flat` is unchanged. Short function tests only for the deterministic
pieces (AO neighbour counts, the shadow march against a known occluder, tile or
species lookup fallbacks).
