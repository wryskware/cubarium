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
- **Pixel art is the rule for illustration only** (Wrysk, 2026-09-24: "the pixel art rule
  is only for illustration. nothing in our current scope falls under that category."):
  sprites, textures and drawn art. Rendering effects — lighting, shadows, bloom, water —
  are not bound by it and may be smooth. Quantisation stays only where Wrysk has seen
  and accepted it (the light ladder, the ripple lines).
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

## L merged (2026-09-24)

L merged to main as `b5809f7c`.

- **Lit GPU time** after the crease and dapple change: 1.16 ms at 6 px, 3.82 ms at
  13 px, 4.13 ms at 13 px textured (5080).
- **Emission:** marks the glowcap lip (bake step only), ripe lanternberry lanterns and
  latticevine bell mouths.
- **Skipped:** the bloomcrown core and sense patches, since their dossiers don't call
  them luminous. Also the spent glowcap, since the model keeps no peak wood.
- **Textured leaf holes** no longer pass sun, because dapples are whole-cell.
- **Open for Wrysk:** glow strength (`glow = 0.5`, pools 12–16 voxels, with a faint
  round edge), whether the bloomcrown core and sense patches should glow anyway, the
  glowcap lip as a whole cell, and bloom.

W starts now, from the brief above, off `b5809f7c`. Its first checkpoint is depth
absorption, the boundary test, the surface normal and the reflection. The second is
refraction, foam, glints and rain ripples.

## L5: bloom and more emitters (Wrysk, 2026-09-24)

Wrysk: "glow is fine. it barely reads though without any bloom. bloomcrown, maybe yeah.
animal sense patch, i dont think so.. but some animals will have glowing parts, like
the chorister. glowcap can have a lip i guess."

- **Bloom, lit only.**
  - Emitting texels mark themselves (an emission output from `voxel.frag`). A bloom
    pass adds a restrained, pixel-art halo around them: the emitted colour gathered
    at voxel-cell resolution, spread a few cells, quantised to 2–3 steps, and added
    back with nearest sampling, so the halo is blocky.
  - The art direction applies: glow supports forms, never haze. The emitter itself
    stays crisp.
  - Knobs: `bloom` (strength) and `bloom_radius` (cells), with stated defaults.
  - Flat is untouched and byte-identical at the Tachyon config. Stay inside the lit
    budget.
- **Bloomcrown core glows**, in D9's own core hex (the alternates B and C describe it
  glowing).
- **Animals:**
  - No sense patches.
  - Animals will have glowing parts, starting with the chorister (CH-A2; see
    `design/animal-body-reimagining-2026-09-24.md` and `art/gen/SIGNOFF.md`), which is
    not in the engine yet.
  - Make sure the animal appearance path can mark emitting texels (the reserved glyph
    tone), and document how in the appearance module.
  - Mark an existing voxel animal's part only if its current design calls that part
    luminous.
- **Glowcap lip:** unchanged.

Queued behind W checkpoint 1: both live in `voxel.frag`, and the lean policy keeps
one worker per crate unless Wrysk asks for parallel work.

## W checkpoint 1 review (2026-09-24)

W-1 is at `5da09099` (not merged). It has depth absorption, a quantised animated
surface, a reflection march and a derived flow field (surface gradient plus falling
cells, in the shader, with no solver change). It costs +0.67 ms of lit GPU at 13 px.

Wrysk's calls:

- **Calmer ripples:** thin horizontal ripple lines instead of round noise blobs, and
  no blinking specks from tiny reflected objects.
- **Dark lake edge:** the lake's front face at the world edge goes back to deep indigo.
- **Brighter waterfall:** falling water stays bright cyan, as in flat and L, instead of
  dimming with the cliff's shade. The downward streaks stay.
- **Reflection strength:** unchanged (`water_reflect` 6.0, about 40 %).

These go in after L5, in the same worktree, and W checkpoint 2 (refraction, foam,
glints, rain ripples) follows.

## L5 and W fixes, round 2 (2026-09-24)

L5 is at `2a30d197` on the W branch: bloom, the bloomcrown core glowing, and the
animal emission path. Its bloom was blocky, which followed my brief, not a limit of
the technique.

Wrysk: "yes" to both of these:

- **Smooth bloom by default:** the emission buffer is downsampled, blurred and
  upsampled with bilinear filtering, then added. Emitters stay crisp. The blocky halo
  is kept as an option.
- **The three W-1 fixes:** calmer ripple lines, dark lake edge, bright waterfall.

A fresh worker does both in the W worktree.
- **Smooth lighting** (Wrysk: "lighting yes"): the ambient ladder goes. `light_levels = 0`
  (smooth) is the new default. Hard sun-shadow edges, crease AO and `sun_tint` stay.
- **Water animation:** "id have to see. 12fps might have a charm". It stays at 12 Hz
  until Wrysk compares real-time clips of 12 Hz stepped against every-frame.

## W, round 2 merged; W checkpoint 2 (2026-09-24)

Wrysk after the 12 Hz vs smooth clips: "smooth waterfall is good. ripples still look
kinda random and just pop in and out of existence." Fable changed that at `37c7038b`. Each
ripple line now fades in and out over its own lifetime (sin²), grows from its middle and
glides with a breeze. `water_smooth = true` is the default. Wrysk: "acceptable. lets move
on". The W branch (W-1, L5, smooth bloom, the water fixes, smooth light, the ripples) is
merged to main.

**Standing rule for every animated effect:** nothing pops. Things fade in and out, or move
continuously, on the every-frame clock.

**W checkpoint 2** (fresh renderer-worker, own worktree off main):

- **Refraction:** behind a tilted surface texel, the walk continues from a pixel offset by
  the normal. Ripple lines are the only tilted texels now, so this may not read at all. If
  it doesn't show at 13 px, drop it and say so.
- **Shore foam:** on skin texels beside a solid, plus plunge foam where falling water meets
  a pool surface (the derived flow already knows the falling cells). The foam breathes
  (fades and shifts smoothly), with no blinking.
- **Glints:** the sun (`[light] sun`) sits on the camera's side, so a true mirror glint
  never lines up with this view. The default is a small sun-coloured highlight on ripple
  lines tilted toward the sun, scaled by the line's fade, in sunlit, unshadowed water only.
  Show it; Wrysk judges.
- **Rain ripples:** rings on open-sky water (the sky plane > 0) while it rains
  (`knobs.w`, rain_tick, is already in the uniform). Random drop sites per time slot; a
  ring grows and fades. Ring density may later follow the weather handoff's rain intensity
  (`design/handoffs/voxel-weather-2026-09-24.md`), but that isn't wired now.
- **Budget:** at most +0.5 ms lit GPU at 13 px for all four. Flat stays byte-identical at
  `config/tachyon/voxel.toml` 6 px, plain and textured.
- **Evidence:** 60 fps GIFs, old on the left and new on the right, cropped to the lake,
  the waterfall base and one shore. Make them with `light_capture --anim 240 --anim-hz 60`,
  then ffmpeg palettegen/paletteuse. Rain needs a raining state; add a capture-only
  override if needed.
- **Stop:** once, with all four done.

## W checkpoint 2 merged (2026-09-24)

W-2 is on main through `2c5b7ac2`: shore and plunge foam, sunward glints and rain rings,
with no measurable GPU cost. Refraction was dropped because it doesn't show at 13 px.

Wrysk's calls:

- **Waterfall:** "waterfall approved", including the foam on every step of the cascade.
- **Glints and rain rings:** they now lean halfway to white, `water_highlight = 0.5`. Wrysk
  saw 0, 0.7, and 0.7 with more strength, and said "just under middle. maybe 50%?".

Still open:

- Foam stays unlit bright cyan in shade (dimmed, it went grey).
- The screen-wide rain streaks step at the 20 Hz tick, not the every-frame clock.
- Ring density waits on rain intensity from the weather handoff.

## V: volumetric light and effect switches (2026-09-24)

Wrysk, 2026-09-24: "k do it. have we been gating any of these render effects behind
config flags? we probably should for a few and definitely this one. just "lit" / "no
lit" might not be enough. ... do plan it for recompute when the sun moves. if we can do
the recompute async and no more than once a minute, thats probably fine."

**Objective:**
- **Light shafts** in the lit tier's air: sunlight scatters where the sun reaches the air
  and not where terrain, trunks or crowns shade it. It shows through canopy gaps, arches,
  skylights and halls.
- **Emitter glow** scattered in the air.
- A **switch for each lit effect** that skips the effect's cost, not just its weight.

The design below is Fable's call. If you depart from it, give the reason in your return.

1. **Sun-visibility volume.**
   - A 3D texture with one texel per voxel, so shafts pass one-voxel slots and skylights.
     Each texel is 0..1: how much sun reaches that cell's open air.
   - Occluders are exactly those of `sunReaches`: terrain, trunks, logs, animals, and crown
     cells by the same `crownLets` hash. That keeps the shafts lined up with the shadows on
     surfaces.
   - Build it off the render thread from a snapshot of the grid. Either works; choose,
     measure and report:
     - on the CPU on a background thread, as `glowTex` is built on the CPU;
     - in a GPU pass that does not stall the frame.
   - For the algorithm, a sweep along the sun direction (each cell's own pass times the
     visibility of the cell upstream) costs O(cells). A march per cell is the fallback.
2. **Recompute:**
   - Async, **at most once a minute**. Add a `[light] volumetric_rebake_s` knob, default 60.
   - Trigger it when the sun has moved more than a small angle, or the world has changed,
     since the last bake.
   - **Nothing pops.** Keep two volumes. The new bake fades in over the interval on the
     every-frame clock.
   - Bake for where the sun will be when the fade ends, so the shafts follow a moving sun
     rather than trailing it by a minute.
   - The sun is static today (`[light] sun`). WX2, in flight, moves it with the day clock.
     Key the trigger off the sun direction the sink hands the renderer each frame, so both
     cases work.
   - Prove it with a capture-only sun sweep, e.g. `light_capture --sun-sweep`, with the
     rebake interval shortened for the capture.
3. **In-scatter along the view ray,** through open air in front of the walk's hit and
   nothing behind it:
   - **Density:** a strength knob times a height falloff, so the air is thicker low down.
   - **Sun term:** `lightC` times the sun visibility.
   - **Emitter term:** `glowTex` times its own weight.
   - Both are hazed like the rest of the frame.
   - The camera is orthographic, so every pixel's ray has the same direction. Accumulate
     during the slab walk if that is cheaper than a separate march, and take no more than
     about 16–24 samples either way.
   - **No shimmer:** any jitter must be stable per pixel, with no noise that changes frame
     to frame.
   - The in-scatter stops at a water surface. Underwater shafts are out of scope.
4. **Fog seam:** WX2 checkpoint 2 adds height fog that the sun lights. Don't build fog, but
   make it easy for WX2 to reuse this work:
   - Put the air density in one function, e.g. `airDensity(p)`, that fog can add to.
   - Expose the sun-visibility lookup as a function, e.g. `sunInAir(p)`, that fog can call.

**Effect switches:**

- In `[light]`, add bools that skip each effect's cost, not just zero its weight:
  - at least `shadows` (the sun march), `reflections` (the water reflection march) and
    `volumetric`;
  - `volumetric` defaults to `false` until Wrysk picks it from the GIFs.
- Check that `bloom = 0`, `emission = false`, `ao = 0` and water foam, glint and rings at
  0 each skip their work. Fix any that don't.
- The other defaults keep today's look.
- Implement them as uniform-coherent branches or as specialization constants, your choice.
  `LIT` is already a specialization constant built from config.
- Add numeric knobs as needed, e.g. `volumetric_density`, `volumetric_glow` and
  `volumetric_rebake_s`, with defaults.

**Isolation from WX2:** WX2 is in flight in worktree `agent-a305b95986d3f88e4` and is
editing `voxel.frag`, `crates/cubarium-gpu/src/voxel.rs`, `sink/gpu/voxel.rs` and
`light_capture.rs` heavily.

- Put new code in new files:
  - a shader include, e.g. `shaders/volumetric.glsl`;
  - a volume module, e.g. `crates/cubarium-gpu/src/sunvis.rs`;
  - the CPU bake, if you choose one, under `crates/cubarium/src/sink/gpu/`.
- Keep edits to the shared files small:
  - append the uniform fields;
  - add one binding (10);
  - add one call site in the composition;
  - add the switch branches.
- Fable integrates the two.

**Budgets:**

- **Volumetric on:** at most 1.0 ms a frame of lit cost at the desktop config on the 5080.
- **Volumetric off:** no measurable cost.
- **Bake:**
  - off the render thread;
  - report its wall time and thread count;
  - no frame hitch when a bake lands: measure the frame times around one.
- **Flat tier:** byte-identical at `config/tachyon/voxel.toml` 6 px, plain and textured.
  Volumetric is lit-only.
- **Agent CPUs:** build and run with `taskset -c 8-15,24-31` and `-j 8`.

**Evidence (checkpoint 1, then stop):**

- Stills of volumetric off and on, side by side, from the terrarium state.
- A 60 fps real-time GIF of the lit terrarium with volumetric on, showing that nothing
  shimmers.
- A sun-sweep GIF, sped up with a shortened rebake, showing the shafts moving and the fade
  between bakes with no pop.
- A table of the switches with the measured lit cost of each, on and off. Include the bake
  time.
- Make the GIFs with ffmpeg: palettegen, `paletteuse=dither=none`, `-loop 0`.

**Rules:**

- Nothing pops.
- Rendering effects may be smooth; the pixel-art rule binds illustration only.
- Never open a window on Wrysk's screen. Capture headless or through `scripts/hidden.sh`.
- Commit with explicit paths. Never merge or deploy.

**Return (≤ 40 lines):**

- commits;
- the absolute path of each GIF and still;
- the switch and cost table;
- the bake design and its measured time;
- open questions about the look.
