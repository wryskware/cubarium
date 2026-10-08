---
status: open
date: 2026-10-07
owner: orchestrator; packages H1–H4 go to workers
supersedes: the rendering half of design/handoffs/godot-direction-2026-10-07.md (D-0004 → D-0005)
---

# Hybrid: Godot owns the app, the Rust renderer draws the world

Decision D-0005 (`design/0_Canon/DECISIONS.md`), Wrysk 2026-10-07, after the built-in
lighting comparison showed Godot's rendering far from the old renderer's look:
- "agree" to keeping the Rust renderer for the world inside the Godot app.
- "the pixel effect was a lot more load bearing to the look than i had realized. definitely
  keep it."
- "the smooth movement and rotations and interpolation was the one thing about godot that was
  like, oooh this is good. so yeah i think we should get that working in our renderer, and
  also revisit that packing problem cuz it seemed pretty slow.."

Workers read `design/handoffs/worker-rules-2026-10-07.md` first.

## The split

- **Rust, simulation**: `crates/cubarium-session` and the sim crates, as today. Headless tools
  (`cubarium-world`, the study examples) never need Godot.
- **Rust, world rendering**: `crates/cubarium-gpu` with its pack in `crates/cubarium/src/sink/gpu/`
  and the voxel presentation in `crates/cubarium/src/voxel/`. The look is this renderer's: the
  per-pixel voxel walk, the face rules, its lighting model (sun march, sky visibility, glow
  volume, AO crease, haze, reflections), the sky, and the **pixel-scale raster with its nearest
  upscale**. It runs inside the Godot extension and also, unchanged, in `cubarium voxel` for the
  ambient displays.
- **Godot, the app**: everything already in `game/` except the world's drawing: start-up, pause
  menu, save slots, new-world picker, inspect, input and gamepad; later audio, settings, export,
  and animation authoring.
- Godot's mesh `WorldView` and its built-in lighting (packages B and the lighting comparison,
  merged a3688c63) are a fallback to delete once H1 is accepted.

## What exists

- The comparison: `/tmp/claude-1000/-home-wrysk-wryskware-cubarium/52e9e909-186e-4829-9f05-c696fc818730/scratchpad/light/out/`
  (contact sheet, pairs, costs). Old lit tier: 3.8 / 4.1 GPU ms at 256 / wide.
- The shell codes against the `WorldView` contract in
  `design/handoffs/godot-direction-2026-10-07.md` ("B and C in parallel"): `attach`, `detach`,
  `show_view`, `input_enabled`, `reset_framing`, `animal_hovered` / `animal_clicked`,
  `set_highlight`, `follow` / `stop_follow`, `capture(size)`. The headless shell test
  (`game/tests/shell_test.gd`, 54 checks) must keep passing.
- `run_voxel` already runs the GPU sink with a presenter thread (frames at the display rate from
  the latest upload) and a headless target with readback (`--gpu-target headless
  --gpu-capture`). The change-only pack (3c94dd34) cut 256 from ~10 to ~1.8 ms/tick, but the
  wide world still packs ~15 ms/tick after a ~13 ms step, serially (backlog item 9).
- Inventory of the old renderer's features: `design/7_Research/godot-port-inventory-2026-10-07.md`.

## H1: the Rust renderer inside the Godot app (first)

- The session hosts the GPU sink the way `run_voxel` does: the sim thread steps and packs; a
  presenter thread draws frames into a headless target at the display rate; finished frames go
  to a newest-frame mailbox. No stall on the GPU: a ring of staging buffers for readback.
- Godot takes the newest frame each `_process` into a texture shown full-screen under the UI with
  nearest filtering. Where the final nearest upscale happens (renderer or Godot) is the
  worker's call: the picture must match `cubarium voxel`'s on the same save and screen size.
- A new `res://world/world_view.tscn` backed by the Rust renderer implements the whole WorldView
  contract, so the shell works unchanged:
  - **Panning**: a view offset in the renderer (wrapping x round the ring, clamped y), eased in
    Godot; mouse drag, keys, gamepad stick, as the mesh view had.
  - **Picking**: screen → raster → the renderer's projection inverted → the animal under the
    pointer (id). Highlight and follow drawn by the renderer.
  - **`show_view`** for candidates and **`capture`** for slot thumbnails: one-off headless renders.
- The mesh view moves to `game/world/legacy/` until Wrysk has seen H1.
- Checks: side-by-side stills of `cubarium voxel` and the Godot app on the same saves (256 and
  wide, midday/dusk/night from the comparison's saves); frame time; the shell test; the session
  and binding tests.

## H2: organisms as smooth, rotating models in the Rust renderer (after H1)

- Animals leave the voxel texture. A model pass draws each animal's baked voxel model as
  instanced geometry at the pixel-scale raster: positions and headings **interpolated between
  ticks** on the frame clock, **free rotation** (heading now; pitch on climbs and slopes when the
  feed gives it), fades on birth and death.
- **Pixel look without pixel stepping** (Wrysk, 2026-10-08: "i would ideally keep the look
  artificially without this limitation"):
  - **Rasterizing:** each animal is rasterized at the coarse pixel scale into its own small
    tile whose pixel grid moves with the animal.
  - **Placing:** the tile goes at the animal's exact interpolated screen position. The animal
    glides with no crawling pixels and keeps the same chunky pixels as the world.
  - **Turning:** a turn re-rasterizes the animal (a few pixels change, like a turning sprite).
    Fine angle steps blended over a few frames may calm it; the worker shows both.
  - **Panning:** sub-pixel as well. The world raster is drawn with a one-pixel margin and the
    upscaled image is shifted by the leftover fraction (asked of H1).
- Tiles carry depth and are tested against the voxel walk's depth, so terrain and animals
  occlude each other.
- Lit by the renderer's own lighting model, evaluated with the model's real normals and sampled
  at the fragment's world position (sun visibility, sky visibility, glow volume, haze). The old
  baked light-imitating tones (lit column, rim) become real lighting; markings and eyes stay.
- Animals' own shadows on terrain: the worker proposes the cheapest correct way (a small
  sun-space depth map for models, read by the walk).
- Then plants: sway and growth transitions (staggered per-voxel grow-in, not an alpha
  cross-fade), the same pass.

## H3: the pack off the critical path

- Profiled 2026-10-07 (`examples/pack_profile.rs --features pack-profile`, 300 ticks, midday
  saves at tick 24000, host load 50–60; full output in the session scratchpad `pp/{a,b}.out`).
  Pack median / p95 ms per tick: 256×128×48 **1.04 / 1.46** (step 1.48); 640×128×96
  **5.97 / 7.88** (step 12.0). Wide breakdown:
  - **Water, about 40%:** pore water 1.47 and free water 0.92. Pore water is also the largest
    phase on 256 (0.33).
  - **Grid-sized phases:** compose 0.69, terrain 0.32, wet list 0.29, ground marks 0.24.
  - **Organism-tied, about 1.5:** canopy 0.60, animals 0.46, stand keys 0.45. Animals and
    stand keys cost the same even when no stand changed, so it looks like a per-tick overhead.
  - **Small:** upload 0.07 (800 KiB per tick); light volumes about 0 at midday.

  The example needed two local fixes to load these saves: founders' policies (load with
  `Founder::ALL`), and the session store's `world-N.voxel` file names. See backlog item 10.
- Levers in order: the double buffer below (a tick becomes max(step, pack): wide 12 ms instead
  of 18); a sweep for pore water like the Godot feed's `pore_sweep` (it covers the grid over N
  updates); dropping animal stamping with H2.
- After H2 removes animal stamping: copy what the pack reads after each step, pack on its own
  thread while the next step runs (backlog item 9), so a tick costs about max(step, pack), and
  the presentation samples published snapshots.

## Terrain-first new-world picking (Wrysk 2026-10-08: "yes build it"; after the SIGBUS fix)

Founding 3–4 wide candidates took 3–4 minutes, and the app crashed in the parallel water
solver while founding them (SIGBUS in `water::column_edges`, fix on branch `fix-sigbus`).
Terrain alone takes about 5 s for three wide worlds. The picker shows 3–4 terrain-only
candidates (rerolls nearly free), and only the picked one is founded, with a progress bar.
- **The picked terrain never changes.** Today a refused habitat draws a new terrain seed.
  Founding the picked world instead retries the founders' placement on the same terrain.
- **Infeasible terrains are never offered.** A terrain that can't host the founders is caught
  by a cheap feasibility check during candidate generation and never shown.
- Candidate generation can be cancelled; founding reports progress.
- **Water slider** (Wrysk 2026-10-08: "add a slider that lets you choose how much water each
  world has (low/med/high) for the .375, .75, 1.5 levels"): Low / Medium / High set the
  terrarium's `water_level_m` to 0.375 / 0.75 / 1.5 m for the candidates and the founded world.

## H4: rigging (later)

- Animals are built in Blender (`scripts/blender/animal_models.py`): bake each part's voxels
  with its joint pivot; rigid voxel parts on bones in the model pass.
- Animations authored in Godot (`AnimationPlayer` on a proxy skeleton) or Blender; bone poses go
  to Rust each frame (a few dozen animals × ~10 bones). Walk, idle, sleep, hunker, glide, climb,
  feed, strike (crouch and lunge).

## Open

- macOS: the renderer through MoltenVK, verified when the game export is attempted.
- Where the old renderer's bloom and the pixel-scale raster meet smooth effects (Wrysk's rule:
  effects may be smooth, illustration pixelated): keep the old renderer's choices for parity.
