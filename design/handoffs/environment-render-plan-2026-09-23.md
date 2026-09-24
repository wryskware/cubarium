# Brief: environment render plan (terrain, water, weather, clouds, light)

For a new thread Wrysk runs himself, from the organism design session (2026-09-23). Organisms
now have sizes, anatomy and (soon) voxel models baked from Blender
(`design/handoffs/voxel-organism-models-2026-09-23.md`). The ground they stand on has no
render plan. This thread writes one: how the voxel world's environment is drawn, from
dev mode through the stage-2 graphic pixel ecology, as a direct reflection of simulation
state. Wrysk's pillar: the picture reflects the simulation, and a good visual idea may
justify extending the simulation (his example: soil depth and quality).

## Read first

- Art direction: `design/art-direction/Cubarium_Art_Direction_v0.1.md`. Binding via
  `WORKING_POLICY.md`: §03 staged execution, §04 "voxels describe occupancy; art
  describes appearance" and "screen first", §05 colour, surfaces and living detail, and
  the reference inventory.
- What the organism pass already sketched: `design/art-direction/organism-scale-and-roster-2026-09-21.md`
  §6 (terrain, water, weather, light: what is simulated and what the art could show) and
  §7 decision 5 (**litter is an axis of the terrain surface tiles**, never floating
  sprites); `design/art-direction/species-dossiers-2026-09-21.md` D1–D2 (the dead-wood
  family and litter tiles).
- Scale and camera: panel 1920×1080, world 160 × 72 × 24 voxels of 0.125 m, 30° tilt,
  raster going to 6 px per voxel ×2 (roster §4 and §7). Organisms are 1–28 voxels tall;
  see `runs/scale-lineup-2026-09-22/panel-ladder*.png`.
- The world that is coming: `design/handoffs/landscape-generation-rethink-2026-09-22.md`
  and the terrarium generator decisions (a Kowloon-like continuous ring of tree/tower:
  open levels, some chambers, 1–2 spires, a dense base; not floating; ring kept). The
  landscape thread owns the generator; this thread owns how it looks.
- The simulation: terrain materials and strata (`crates/cubarium-voxel/src/recipe.rs`,
  `world.rs`); water: free water per voxel, pore water, transit water on faces, the
  lumped atmosphere `world.atmosphere_m3`, scheduled showers (`World::is_raining`),
  settled vs in-transit depth (`design/handoffs/voxel-terrain-note-standing-depth-2026-09-22.md`);
  light: geometric sky visibility plus per-layer canopy shade (flora `step.rs`). The
  presenter lives in `crates/cubarium/src/voxel/` (`present.rs`, `scene.rs`,
  `appearance.rs`, `project.rs`).

## Questions the plan must answer

1. **Terrain surfaces:** material autotiles by contour and layering; exposed strata on the
   cut face; cavity interiors; how soil depth and a soil-quality field (a model addition
   to propose, with what it would do ecologically) show as a gradient; litter and dead
   wood as tile axes.
2. **Water:** surface skin vs body; depth; transit water as wet-face darkening; seeps
   where soil saturates; falls between terrarium levels; the pool edge where siphonreed
   stands.
3. **Weather and sky:** rain streaks from the shower schedule; a cloud band whose density
   follows `atmosphere_m3` (honest before any cloud model); what the sky shows.
4. **Light:** shade under crowns from the same numbers the plants use; any day/night or
   slow light cycle (a model question, not decoration); emissive organisms (glowcap,
   lure, call flashes) against a dark ground.
5. **The terrarium's structure:** posts, platforms, bridges and chambers at 6 px per
   voxel; how open levels read in depth at a 30° tilt without clutter.
6. **Staging:** what dev mode shows (diagnostic), what stage 2 needs (tiles, sheets,
   shaders), what waits for stage 3. Which parts are voxel-rendered, sprite-drawn or
   3D-assisted, per the art direction's "choose techniques by the result".
7. **Performance on the Tachyon:** the budget at 20 ticks/s and 60 fps presented
   (`design/handoffs/` terrain line notes).

## Deliverable

`design/art-direction/environment-render-plan-<date>.md` (`design_status: proposal`):
per element, the state it reads, how it is drawn at each stage, and asset families with
their axes (composite sheets over per-object sprites, per Wrysk's cohesion rule).
Model additions it proposes are listed separately. Then packages, with the decisions that
are Wrysk's marked. Blender (headless, `scripts/blender/`) and the sprite-forge
(Qwen-Image 2.1, and local Ideogram 4 with the `ideogram4-prompt` skill, and Krea 2) are
available for studies. Look is Wrysk's call; workers never decide it.
