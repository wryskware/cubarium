---
status: open
date: 2026-10-07
owner: orchestrator in a new thread with Wrysk; packages A–C below go to workers
supersedes: design/handoffs/app-shell-2026-10-07.md
---

# Godot direction: Rust is the simulation, Godot is everything you see and touch

Wrysk, 2026-10-07, after weighing egui against Godot:
- "if it fully moved into godot, we'd give it the voxel meshes and have it do all the rigging
  and movement ... rust would remain as the simulation layer, and be presentation agnostic."
- "it almost feels cleanest if its only the core sim engine, and we port things like save
  slots, presets, seeds picking etc into the godot harness."
- "overall, i think i would rather just do the full port. the short term features i asked for
  are not like i need them right now."
- "lets not worry about the tachyon. i think its more likely i have the ambient piece run off
  my desktop or legion go so it can support a decent sized world." macOS is a target for the
  full game, not for him today.

After weighing Unity and Unreal: "yeah godot is the pick for sure." Recorded as **D-0004** in
`design/0_Canon/DECISIONS.md`; the split below is its canonical source. Packages, checks and
names here remain proposals.

Workers read `design/handoffs/worker-rules-2026-10-07.md` first.

## The split

**Rust** (the sim crates plus a new session crate):
- The simulation: world, water, flora, fauna, weather, the day clock.
- World generation from a parameter set and a seed.
- The save format and its schema refusal (always fresh, never migrate).
- The observatory's query layer and the commands (speed, pause, and later more).
- A per-tick view feed for presentation.
- No presentation: no meshes, colours, poses or cameras. The sim crates never depend on Godot.

**Godot** (new project `game/`, Godot 4.7.2):
- All presentation: terrain and water meshes, organisms, rigging, animation, movement
  smoothed between the 20 Hz ticks, lighting, weather visuals, the camera, picking.
- The whole app shell: menus, save slots, the preset catalog, seed picking, inspect,
  settings, input (keyboard, mouse, gamepad on the Legion Go).
- Heavy presentation compute (meshing changed chunks) may be Rust inside the extension
  crate. That crate is Godot-side code; the sim crates do not know it exists.

**The binding**: a gdext (godot-rust) cdylib, `crates/cubarium-godot`. Keep it narrow and
data-driven, so the sim's daily churn does not reach it:
- Lifecycle: generate candidates, promote one, load a path, save a path, close.
- Run control: speed, pause, step. The sim runs on its own thread; Godot never blocks on a
  tick.
- `query(name, args)` returns the observatory layer's serde values as Dictionaries (or JSON).
  A new stat needs no binding change.
- `command(...)` mirrors the stdin console (`COMMANDS` in `crates/cubarium/src/voxel/mod.rs`).
- The view feed, per tick: changed terrain and water chunks as packed arrays; organisms as
  packed records (id, species id, position, heading, behaviour or pose state, life stage,
  growth stage); weather, light and the day clock; an event list (birth, meal, kill,
  injury, death) to cue animation.
- **Species are ids plus generic state.** What a species looks like lives in Godot's assets
  and data. A new species must not touch the binding; only a new *kind* of visible state
  adds a field.
- Pin the gdext release. If it predates Godot 4.7, generate the API from the installed
  binary (its custom-API feature against `/usr/bin/godot`).
- macOS later: the session and extension crates take no Linux-only dependency (no wayland,
  drm or ash). Godot draws through Metal there.

**The Rust presentation becomes legacy.** `crates/cubarium-gpu`, `crates/cubarium/src/sink/gpu*`
and the voxel presentation in `crates/cubarium/src/voxel/` (present, model, animal, stand,
poses; about 40,000 lines with the shaders) keep running the ambient piece on the desktop
and voidrunner until Godot reaches parity, then retire. Fixes only, no new features there.
The CPU presenter stays for PNG diagnostics. `ash` was chosen for the Tachyon panel only;
the panel is no longer a constraint (a window manager there is acceptable later).

## What exists

- **The run loop**: `run_voxel` (`crates/cubarium/src/voxel/mod.rs`, one ~680-line function)
  owns the world, flora and fauna, pacing, saves and the console.
- **Saves**: `save_whole_world`, `load_world`, `SnapshotWriter` in the same file; the layered
  world/flora/fauna snapshot; schema mismatch refused.
- **World generation**: the `terrarium` landform preset (`crates/cubarium-voxel/src/terrarium.rs`),
  at 0.125 m. Sizes on main: 256 × 128 × 48 (`config/*/terrarium.toml`), depth 72 and 96
  (`terrarium-d72.toml`, `-d96.toml`), and the desktop's wide 640 × 128 × 96
  (`config/desktop/terrarium-wide.toml`, for its 2560×720 screen).
- **Observatory** (merged d5af164b, `crates/cubarium-observatory`): transport-free `query`
  functions over `&World`, `&Flora`, `&Fauna` returning serde types; `observer::Observer`
  publishing snapshots off the loop; `LifeBook` with per-animal life histories; the
  `Command` enum (speed, pause, resume). The binding calls these directly. The HTTP server and
  dashboard stay as a dev tool.
- **Change detection**: the change-only pack in `crates/cubarium/src/sink/gpu/voxel.rs` already
  knows what changed per tick; reuse its logic for the feed.
- **Organism sources**: baked voxel models (`assets/voxel-models/<mm>/manifest.json`, loaded by
  `ModelLibrary` in `crates/cubarium/src/voxel/model.rs`), the dev glyphs (`stand.rs`,
  `animal.rs`) and the pose modes (hunker, sleep, idle, stand).
- **Godot here already**: 4.7.2 at `/usr/bin/godot`. `art/` is the Creature Atelier (cube-era
  cutout rigs with `AnimationPlayer`, and the art baker), launched by `scripts/godot.sh`. Keep
  `game/` separate from it.
- **Look**: `design/art-direction/` (v0.1). Voxel or illustrated pixel art is Wrysk's open
  decision in his own art thread. No worker decides it.

## First deliverable (D1): the Godot app with the shell features

Dev presentation only. D1 does not port the voxel look or invent a new one.

### Package A: session crate and binding (Rust; concurrency, so high effort)

1. A session crate (`crates/cubarium-session`, name is the worker's call). A `Session` owns
   the world, flora, fauna and the loop on its own thread, with the API above.
   - Rebuild `cubarium voxel` on top of it with the same behaviour; the ambient piece runs on
     it until parity. Check: a headless capture from the old and the new binary on one seed
     look alike (qualitative, not byte-identical).
   - Candidates: generate N worlds in parallel from (size, terrain preset, climate preset,
     seed) without starting the sim; promote one to the running session.
   - Save and load to a given path. A wrong-schema save gives a plain error. Beside each save,
     a small metadata file: world day, tick, size, presets, species counts.
   - Presets arrive as a parameter set the session validates (unknown fields refused). The
     catalog itself (names, descriptions, which parameters) is Godot data.
2. The binding crate `crates/cubarium-godot` and the bare `game/` project: a `.gdextension`,
   a build script that builds the cdylib into the project, and a main scene that creates a
   session, runs it and prints a query. Nothing drawn yet.

### Package B: dev presentation (Godot; new UI construction, so high effort)

- Terrain and water meshes from the changed chunks, in plain coloured voxels (the plain path
  is the main draw; textures stay opt-in). The elevated orthographic camera at 30°, as today.
- **Panning** (Wrysk, 2026-10-07: "panning. or at least at creation, but i dont see the harm
  in panning a live world"). The camera pans across the live world and across a candidate in
  the new-world picker: mouse drag, keys, and the gamepad stick, clamped to the world's edges,
  eased rather than snapped. The default framing fits the world as today. Panning matters most
  where the world is wider than the screen (the 640-wide world on voidrunner's 1920×1200).
- Organisms as their baked voxel models, else glyph boxes, placed by position and heading and
  smoothed between ticks. Pose modes may switch models; no rigging yet.
- Daylight from the day clock; rain as a simple effect. Animated effects fade or move, never
  pop.
- Measure fps and ms per frame on 256 × 128 × 48, depth 96 and the wide 640 × 128 × 96 world
  on the desktop. 60 fps is the bar for the Legion Go.

### Package C: the shell (Godot; high effort; parallel with B once A lands)

1. **Pause menu**: Esc toggles it and pauses the sim. Resume, Save, Load, New world, Quit.
   Esc never quits. Esc closes an open panel before it opens the menu.
2. **Save slots**: named slots under `user://saves/`, each with a thumbnail captured from
   Godot's view, the world day and a species summary from the query layer. Save, overwrite,
   rename, delete. A wrong-schema save stays listed as "made by an older version, can't be
   loaded" and can be deleted.
3. **New world**: size (terrarium, deep 1.5×, 2×, wide), terrain preset, climate preset.
   Generate 3–4 candidates in parallel; Godot draws each candidate's thumbnail in its own
   SubViewport; open one to pan across it; pick one or reroll.
4. **Inspect**: hover highlights an animal (Godot ray-casts its own scene), click opens its
   stats and life history from the observatory's animal data, and the panel follows the
   animal.
5. Every menu works with a gamepad through Godot's focus navigation.

### Checks for D1

- Rust: session tests on tiny hand-built worlds, checking mechanisms only. Pause stops ticks,
  save and load round-trip, a wrong schema is refused, candidates are independent. No pinned
  numbers. Test authoring is its own pass (global rules).
- Godot: a headless script (`godot --headless --script`) loads the extension, creates a
  session, steps it and queries it.
- Pictures of every screen without a window on Wrysk's screen. Godot's `--headless` does not
  render, so find a capture path through `scripts/hidden.sh` (`--hyprland` if Xvfb can't give
  Godot a renderer) and write the recipe in `game/README.md`.

## After D1 (order, not briefs)

- **D2, the look**: once Wrysk decides voxel or illustrated pixel art, build it in Godot.
  Organism rigs and animation (Skeleton2D or 3D, `AnimationPlayer`) driven by behaviour state
  and events from the feed; plants by growth stage; water, weather and lighting. The Rust
  renderer's lit effects (sun-march shadows, water reflections, light shafts, glow, AO;
  `crates/cubarium-gpu/src/sunvis.rs`) are the reference for what "lit" means, not code to port.
- **D3, parity and retirement**: the ambient piece runs from the Godot export on the desktop
  and voidrunner (deploy scripts and user services move over); the Rust GPU presentation is
  deleted.
- **D4, the game**: `design/game-roadmap-2026-09-16.md`, plus settings, input remapping, audio,
  Steam and the macOS export.

## Open for Wrysk

- The look: voxel or illustrated pixel art (gates D2, not D1).
- The project path `game/`.
