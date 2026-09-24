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
  Desktop defaults to `lit`. *(Review:)* the code default is `flat` and
  `config/desktop/*.toml` set `lit`, so the panel and cube configs need no edit and
  can't drift into `lit`. `lit` is a pipeline specialization constant (or a second
  SPIR-V), so the flat pipeline runs today's shader code on the panel's GPU. The flat
  picture at the panel config (6 px) matches main in a one-off comparison, not a
  pinned test.
- **GPU only.** The CPU renderer stays basic.
- Desktop target: 60 fps at auto px (9–13) with `lit` on. The desktop window's frame
  readback currently caps 13 px at about 22–26 fps (package P). *(Review:)* budget
  for `lit`: at most 8 ms of GPU time per frame at 13 px on the 5090. The main loop's
  pack stays within +0.5 ms a tick. Slow planes are recomputed off the loop thread.
- Fixed elevated-orthographic camera: visibility and many light terms can be
  precomputed per voxel and updated only when terrain changes. Terrain is static in a
  run except for the development `SetMaterial` command (`water.rs` `apply`).
- *(Review:)* **No new look.** Light colours come from the existing palette only: the
  sun is `lightC`, and the ambient follows the sky colours. New numbers (sun
  direction, gains, the light ladder) are config knobs with stated defaults. A package
  that needs a colour or a look the art direction doesn't cover stops and asks.

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

*(Revised in review, 2026-09-24.)*

**What the picture can show.** Only two faces of a voxel are ever drawn: the front
(normal −z, toward the camera) and the top (+y). A front face's open cell is
(x, y, z−1), and a top face's is (x, y+1, z). Face texel (dx, dy) lies at world point
(x + (dx+½)/S, y+1 − (dy+½)/S, z) on a front face and at
(x + (dx+½)/S, y+1, z+1 − (dy+½)/RISE) on a top face, using `voxel.frag`'s dx/dy
conventions. The view ray is (0, −RISE/S, 1).

**Replace the flat tier's fake light; don't stack on it.** In `lit`, the new terms
replace the flat tier's stand-ins for light: the column roof shade
(`roofShade`/`roofGap`) and the "lit" mixes on tops and plants (`TOP_GAIN`,
`TOP_TINT`, `PLANT_TOP_GAIN`, `PLANT_TOP_TINT`). The edge treatment stays exactly as
it is: rim, chamfer, bevel, `EDGE_DARK`, riser lean, contour row, grain, the glyph
atlas's baked tones, and haze.

**Quantise the light, not the colour.** A texel's base colour (strata colour, texture
texel or pigment) is unchanged. It is multiplied by a light value that is quantised
first. The sun term is binary per texel: lit, or one hard step darker in shadow. The
ambient product (sky × AO × canopy) snaps to a short ladder (`light_levels`,
default 4). No dithering, no blur.

1. **Voxel ambient occlusion, in the shader.** Classic 4-corner AO from the 8
   neighbours in the face's open plane, interpolated bilinearly across the face's
   texels, then snapped to the ambient ladder. Terrain and block parts (trunk, crown,
   heart, log, animal) occlude. Sprouts and vine cells don't. Test: corner counts on a
   hand-built fixture.
2. **Sky light and canopy.**
   - *Sky:* add a new slow plane (r8, keyed on `terrain_version`, like the roof table)
     holding the ecology's own `World::sky_visibility`, called unchanged. Fill it for
     every cell that can open onto a visible face. Open cell (x, y, z) stores
     `sky_visibility(x, y−1, z)`, so a top face reads exactly the model's number, and
     a wall reads the fan from the foot of its open cell (about ⅔ under open sky).
     Compute it off the loop thread. The flat tier's roof table stays.
   - *Canopy:* the model shades a stand by the Beer–Lambert attenuation of each layer
     above it, straight down (`crates/cubarium-voxel-flora/src/step.rs`,
     `shade_layers_into`). The picture does the same for the ambient term: crown cells
     above a texel attenuate it, so ground under a crown is as dim as the model says.
     A tilted sun alone would put that shade in the wrong place. The worker chooses
     how crown transmission reaches the GPU (per style, a per-stand plane, or at pack
     time), within the budget.
3. **Sun and cast shadows.**
   - *Direction:* a `sun` knob. The default must sit in the front hemisphere, from the
     upper left: light travels away from the camera, so front faces are lit and
     shadows fall back into the scene. A sun behind the scene would leave every
     visible front face in self-shadow.
   - *March:* a per-texel DDA from the texel's world point toward the sun. Terrain and
     block parts are opaque. Crown cells are transmissive, using step 2's crown
     transmission, with a per-cell hash choosing which texels pass. That makes the
     shade dappled, with a mean equal to the transmission. Where textures are on, leaf
     cutout holes let light through. Cap the march. Add a coarse occupancy mip for
     skipping empty space only if the budget needs it.
   - *Test:* a headless render of a pillar on a flat floor. Texels inside the pillar's
     analytic shadow are the shadow step; texels outside it are lit.
4. **Emissive.**
   - *Data:* which texels emit is data, not shader code: a per-style emissive colour
     and a reserved glyph-atlas tone for emitting texels.
   - *What emits:* mark only parts that the dossiers name as luminous and that exist
     in today's models and appearance data: the glowcap lip, bloomcrown core,
     lanternberry lanterns, latticevine bell mouths and sense patches. Report which
     were marked and which were skipped.
   - *Look:* emitters draw unshadowed at full value.
   - *Local light:* a coarse (¼-res) RGB volume built on the CPU from the emitter
     list, propagated a few cells and blocked by terrain. It is added to the ambient
     before quantisation.
   - *No bloom pass.* The art direction says glow supports forms and never becomes
     haze. Offer bloom once Wrysk has seen step 4.

Checkpoints: stop and report after steps 1–2, after 3, and after 4. Each checkpoint
is a commit with its screenshots and frame times.

### W: water that reads as water (after L; same shader)

*(Revised in review, 2026-09-24.)*

**What the water data is.** One free-water byte per voxel (the g channel of
`VoxelTexel`). No velocity or flux reaches the renderer. The boundary rules stay
exactly the flat tier's (`fillPxQ`, the skin row, `nearerOwns`, `stop`), and W changes
colour only inside them. Test: on a fixture with a pool, a falling column and a
brim-full cell, `flat` and `lit` mark exactly the same pixels as water.

- **Depth absorption:** per-channel Beer–Lambert over the water path along the walk
  (each slab is √(1 + (RISE/S)²) voxels of path). The palette's surface-to-deep water
  colours are the in-scatter ramp. Clear shallows show the bed; deep water goes dark
  indigo.
- **Surface:**
  - The skin (top-face) rows get an animated normal from tiled noise on sim time. That
    needs a new per-frame time uniform: the tick plus the fraction of it elapsed.
  - The normal is quantised to a few directions and stepped at an animation rate
    (default 12 Hz), so it moves like pixel animation.
  - Rain ripple rings while it rains.
  - Animated water means every `lit` frame redraws. `flat` keeps its redraw skip.
- **Reflection, with a Fresnel mix:** march the reflected ray ((0, RISE/S, 1),
  perturbed by the normal) a capped distance through the volume. Shade the hit with
  L's lit face colour, or with the sky gradient if the ray escapes. Still pools mirror
  the far bank. Add a reflection gain knob: physical Fresnel at this angle is about
  5%, too weak to read.
- **Refraction:** behind the surface, the walk continues from a pixel offset by the
  normal, in whole pixels.
- **Shore foam and glints:** foam on skin texels next to a solid, and single-pixel
  glints where the quantised normal faces the sun. Palette colours only.
- **Waterfalls:** no flow reaches the renderer. Skip streaks unless the solver can
  expose a per-cell flux byte cheaply, and say which way it went.

### P: direct GPU presentation on the desktop (perf; alongside L)

*(Revised in review, 2026-09-24.)* Today `WindowTarget`
(`crates/cubarium/src/sink/gpu/target.rs`) does all of this on the loop thread:
render, wait, read the whole raster back, upscale it on the CPU, and hand it to
minifb's shm buffer. At 13 px, a raster 3328 px wide on the 4K screen, that comes to
about 22–26 fps.

- **Swapchain on the existing window.**
  - minifb 0.28 exposes raw-window-handle 0.6 handles: Wayland on Hyprland, Xlib
    elsewhere. Create the Vulkan surface with ash's `khr::wayland_surface` /
    `xlib_surface`. No new crates.
  - If minifb's Wayland backend fights the WSI for the surface, fall back to a bare
    wayland-client 0.29 xdg-toplevel. That crate is already in the lockfile.
  - Use winit only behind a non-default feature, and only after asking.
- **Never block the loop.**
  - On Wayland with NVIDIA, a FIFO present to a hidden window (on another workspace)
    can block indefinitely.
  - So present on a thread of its own, following the ShimPresenter pattern
    (`cubarium_gpu::target::presenter`). The loop hands over a finished frame and
    never waits. A frame the presenter can't take is dropped and counted.
  - Use mailbox where the driver offers it, otherwise FIFO on that thread.
- **Upscale on the GPU:**
  - Use the existing present pass (`present.frag`): integer nearest zoom, letterboxed.
  - Recreate the swapchain on resize, OUT_OF_DATE and SUBOPTIMAL.
  - A window smaller than the raster downscales nearest. It's a development window.
- Escape and close quit as they do today. The readback path stays as the fallback
  when surface creation fails. The panel's targets are untouched.
- Evidence: presented fps and ticks/s at 13 px before and after (flat, plus lit if L
  has landed). Also 30 s with the window on another workspace, with ticks/s holding
  at 20.

## Order and workers

*(Revised in review, 2026-09-24.)*

- **S landed** (merge `52625be`), so the L worker starts from S's `voxel.frag`.
- **L and P now, in parallel.** Each gets its own worktree off main and one
  `renderer-worker` (Opus, high effort). L is new substantial rendering; P is
  presentation threading.
- **W after L merges, by a fresh worker** from the brief below. By then the L worker's
  context is too large to resume cheaply.
- Stage Q art and the Codex art pass are unchanged.

Evidence for every package:

- Screenshots from the same seed and tick, at 6 px and auto, plain and `--textures`.
- Frame times.
- The panel config at 6 px on the desktop GPU, showing `flat` is unchanged.

Screenshots go under `captures/presentation/<package>-<step>/` (untracked), and the
report gives their absolute paths. Short function tests only, for the deterministic
pieces: AO neighbour counts, the shadow march against a known occluder, the sky plane
against `sky_visibility`, and the water mask being the same in flat and lit.

## Worker briefs

Common to all three:

- **Setup:** work in your own worktree, with `cargo -j 8`, under
  `taskset -c 8-15,24-31`.
- **Commits:** commit with explicit paths. Never `commit -a`. Never merge, deploy, or
  touch the board.
- **Windows:** open any window with `./scripts/run-voxel.sh --background`. It must
  not take focus or re-tile.
- **Scope:** don't touch the CPU renderer, the Tachyon and cube configs, or the panel
  targets.
- **Tests:** run only the crates you touched, `cubarium-gpu` needing the GPU. Rebuild
  SPIR-V with `crates/cubarium-gpu/shaders/compile.sh`.
- **The commit message is the report.** Return at most about 40 lines: commits,
  screenshot absolute paths, GPU ms flat/lit at 6 px and 13 px, the pack-ms change,
  the defaults you chose and their visible effect, and open questions.
- **Stop and ask** on any look question the art direction
  (`design/art-direction/Cubarium_Art_Direction_v0.1.md`) doesn't answer. Make routine
  implementation calls yourself and say what they do.

**L.**

- *Deliverable:* package L above, steps 1–4, with the revised standing constraints:
  the `lighting` key, the specialization constant, the budget, and no new look.
- *Checkpoints:* stop after steps 1–2, 3 and 4.
- *Files:*
  - `crates/cubarium-gpu/shaders/voxel.frag`
  - `crates/cubarium-gpu/src/voxel.rs` (uniforms, slow planes, pipeline)
  - `crates/cubarium/src/sink/gpu/voxel.rs` (Packer)
  - `crates/cubarium/src/voxel/mod.rs` (config)
  - `config/desktop/*.toml`
  - Read only: `World::sky_visibility` (`crates/cubarium-voxel/src/world.rs`) and
    `shade_layers_into` (`crates/cubarium-voxel-flora/src/step.rs`).
- *Out of scope:* water shading (W) and window presentation (P).

**P.**

- *Deliverable:* package P above.
- *Files:*
  - `crates/cubarium/src/sink/gpu/target.rs`
  - `crates/cubarium-gpu/src/target/` (presenter pattern)
  - `crates/cubarium-gpu/src/present.rs`
- *Out of scope:* shaders other than the present pass.
- *Evidence:* measure before and after, including the hidden-window run.

**W** (spawned after L merges).

- *Deliverable:* package W above, on top of L's shading functions.
- *Files:* `voxel.frag` water path (`waterAt` and the walk), the uniform block, and
  the time uniform through `crates/cubarium/src/sink/gpu/voxel.rs`.
- *Order:* depth absorption and the boundary test first, then surface and
  reflection, then refraction, foam and glints, each a commit with screenshots.

## L checkpoint 1 review (2026-09-24)

Landed on branch `worktree-agent-a7dfd042149480812` at `fa5b94a8` (not merged).

- Lit costs +0.06 ms GPU at 6 px and +0.22 ms at 13 px on the 5080, and +0.1–0.17 ms
  of pack a tick.
- Flat at the panel config is byte-identical to main.
- Screenshots: `.claude/worktrees/agent-a7dfd042149480812/captures/presentation/L-1/`.

Calls made at the checkpoint:

- **Self-shade:** a stand's own layers don't shade its own cells, because the model
  never lets a stand shade itself. Terrain under a crown is still attenuated.
- **Sky pop-in:** the first world frame waits for the sky plane (the founding pulse
  covers the ~2 s). After a development `SetMaterial` edit it may pop in.
- **Budget device:** the budget is held on the GPU the app actually runs on (the
  5080, which `Gpu::open` picks first).
- **Open for Wrysk (look): the sunlit term.** Lit drops the flat tier's lean of tops
  toward `lightC`, so terraces lose their lavender and read flatter (luminance −25 %).
  Step 3 builds the sunlit term in the flat tier's own form, a mix toward `lightC`
  gated by the binary sun term, with a `sun_tint` knob where 0 is purely
  multiplicative. Checkpoint 2 shows both, and Wrysk picks.
- **Also for Wrysk's eye:** AO at 0.5 gives every riser a darker lower band, which
  reads as horizontal striping on terraces at 13 px. Quantised bilinear AO makes
  diagonal wedges inside faces.

## L checkpoint 2 review (2026-09-24)

Sun and cast shadows landed at `feff1bed` on the L branch (not merged).

- **Cost:** lit takes 4.95 ms of GPU at 13 px (5.35 textured), inside the 8 ms budget,
  so no occupancy mip. The pack costs +0.50 ms, at the budget line.
- **Flat:** at the Tachyon config it is byte-identical to main.
- **Defaults:** `sun = [-1, 2, -1]` (upper left, about 55° up); `sun_tint` 0.18. The
  sun adds one rung, so a shadow is one rung down, open tops come out at 1.77 × base,
  and sunlit fronts at 1.4 × base (flat has 1.0).
- **Screenshots:** `.claude/worktrees/agent-a7dfd042149480812/captures/presentation/L-2/`.

Open for Wrysk, as style calls, since the big value structure works:

- `sun_tint` 0.18 (lavender tops back) or 0 (deeper purple).
- Fronts brighter than in flat.
- Per-texel shadow edges run diagonally down stepped cliffs as sawtooth teeth.
  Per-face shadows would be blockier and cleaner.
- Bilinear AO leaves triangular wedges inside faces. AO as a contact band along the
  occluded edge is the alternative.
- Half-voxel dapples break up crowns. Whole-cell dapples are the alternative.

Step 4 (emissive) proceeds meanwhile, without touching those knobs.

**Wrysk's calls on checkpoint 2 (2026-09-24):**

- **`sun_tint`:** 0.18 stays.
- **Shadows:** the per-texel "realistic" shadows stay; Wrysk likes them.
- **AO → crease line:** block faces stay flat and crisp. There is only a thin darker
  band along an edge where the neighbouring surface occludes, plus a corner square
  where only the diagonal neighbour does. The bilinear fade goes.
- **Dapples → whole-block spots:** a crown cell passes or blocks the sun as a whole
  cell (a hash per cell), with the same mean transmission.
- **Front brightness:** not raised; it stays as built.
