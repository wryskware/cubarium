---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Backlog

> Since 2026-09-16 the backlog is tracked as GitHub issues on wryskware/cubarium
> (labels `backlog`, `tachyon`, `gpu`, `art`). §1 is issue #14, §2 is #15, §3 is #16, §4 is
> folded into #14. This file stays as the long-form spec for those items.

Deferred work Wrysk has named but not scheduled. Each item says who asked, when, and
what "done" looks like. Fable keeps this current; anything picked up moves to a handoff.

## 1. Operator controls: a running list of user-configurable parameters, and a GUI for them

Wrysk, 2026-09-16: "keep a running list of user configurable parameters. at some point,
we'll add them to the webpage so it can be adjusted, or the daemon restarted with those
settings from a gui." Not now.

**Done looks like:** the viewer page (port 7393) exposes these controls, either live where
the presenter can take them at runtime, or as a "restart the daemon with these settings"
form that writes the launch line `scripts/run-cube.sh` uses. Until then this list is the
spec. Add to it whenever a new knob lands.

| knob | where it lives today | live or restart | notes |
| --- | --- | --- | --- |
| foliage shoulder `CUBARIUM_FOLIAGE_FULL` (0.5..=1.0, ships 0.85; 0.95 recommended by G, on the cube since 2026-09-16) | env var, read once at first presenter build (`art_present/habitat.rs`) | restart | 1.0 vetoed (bare wood on ungrazed stands). Logged at startup when overridden; not in `/status`. |
| world config TOML `--config <toml>` (every `WorldConfig` field: plant, producer, organism, drives, detritus, weather, founders, capacity, seed) | `cubarium run --config`; selected ecologies under `runs/ecology-v1-calibration/selected/` (`baseline.toml`, `fast-leaf.toml`) | restart, fresh world only (never migrate) | Policy files are refused by name against a different ecology. |
| `--seed <n>` | CLI, overrides `config.seed` on a fresh world | restart | |
| `--speed <x>` (0 = unlimited, no clock) | CLI | restart | |
| `--fps <n>` | CLI | restart | |
| `--care` (care journal / care effects on) | CLI | restart | |
| `--neural <policy.json>` + `--neural-count <n>` | CLI, fresh world only | restart | Refused on resume and across ecologies. |
| `--sink shim|web|none`, `--mirror-web`, `--web-port`, `--addr`, `--art <dir>` | CLI | restart | |
| `--telemetry`, `--fields`, `--events`, `--out`, `--every`, `--scale` | CLI (diagnostic outputs) | restart | |
| apex spawn controls (introduce one or two adults, paid lifecycle, never restocked) | viewer page | live | Present since ecology v1. |
| `organism.move_cost` (ships 0.00036; F measured 0.0018 and 0.006, both overshoot) | `WorldConfig` via TOML; calibration `--prices` axis | restart | Candidate for a slider once the finer ladder (I) reports. |
| voxel world config (`cubarium_voxel::Config`: `width/height/depth`, `voxel_m`, `seed`, **`noise_seed`**, `rain_m_per_s`, `evaporation_m_per_s`, `water_substeps`, `spring_k_m2_per_s`, `aquifer_porosity`, `outlet_m3_per_s`, `free_transfer_cap`) | `crates/cubarium/voxel.example.toml` via `cubarium voxel --config` | restart, fresh world only (schema 2) | `noise_seed` (0 = derive from `seed`) re-draws the landform's final weak wobble alone and is the instrument for the terrain-coupling experiment. |
| voxel flora config (`FloraConfig`: `shade_k`, `decomposition`, `wood_decomposition`, `litter_energy_cap`, `initial_nutrient`; `SpeciesConfig` per species: `alpha`, `wood_max`, `reserve_cap`, `maintenance`, `senescence`, `foliage_rate`, `wood_rate`, `build`, `dieback`, `alive_min`, `donor_min`, `donor_reserve_floor`, `propagule_rate`, `propagule_split`, `energy_density`, `reserve_share`, `reflush_below`, `assimilation`, `nutrient_half`, `nutrient_draw_max`, `light_half`, `rooting_depth`, `rooting_radius`, `wilt_pore`, `sat_pore`, `transpiration_m3_per_s`, `establish_pore_min`, `establish_light_min`, `drown_depth_m`, `hop`, `crown_height_voxels`, `crown_radius_voxels`) | `FloraConfig::default()` in code; no TOML table yet (voxel round 2) | restart | Every value is an untuned placeholder. `bloomcrown.drown_depth_m` was 0.0 and is now 0.05 — a wrong placeholder, not a tuned one: any free water at all killed the species everywhere it rained. Still known-wrong: `shade_k` 1.5 lets one full crown attenuate the light under it by only 9 %, so a canopy never closes; `umbrellafrond.wilt_pore` 0.3 and `establish_pore_min` 0.45 sit **above** soil's own `field_capacity` (0.25 of pore capacity), which is where every soil voxel settles under any rain the soil can absorb, so the wet species has `μ = 0` and no site to establish on anywhere that is not ponded. `maintenance`/`dieback`/`reserve_cap` put death hours away, so no short test reaches it at the defaults. |
| pursuit stopping rule `--pursuit-stop {reach-envelope,half-space}` (ships `reach-envelope` since 2026-09-16) | `cubarium-search` only — `calibrate`, `precondition`, `factorial`, `es-population`, `apex-audit`; a `World` transient (`World::set_pursuit_stop`), never a `WorldConfig` field and never persisted | restart, fresh world only | **Not a knob for the viewer.** `half-space` exists to reproduce rows retained before the adoption and nothing else; the cube has no reason to run it, and a world saved under one rule is refused under the other (schema 17). List it here so the list is complete, not so it gets a slider. |

## 2. Artwork pass

Wrysk, 2026-09-16, on G's soil-band dead-wood stub (a cut-down stage-0 mushroom in the ash
tone): "i think a purpose drawn one is better, but lets put that in a backlog too and do a
artwork pass later."

**Done looks like:** one authored tile set for a dead-wood snag in the soil band (and its
fade), under `assets/atelier`, replacing the `Mask::Axial` cut in `habitat.rs`; the
`art_ecology` soil-mark tests re-pointed at it. Also queued for the same pass, from B's and
G's notes: the stripped-canopy silhouette is less articulated than the side-face plant; the
water band shows nothing for dead wood; a tall dead column has no authored crown.

## 3. A light physics engine for movement

Wrysk, 2026-09-16: "i dont think a light physics engine is the wrong call to
implement at some point regardless", after directing that movement cost follow
rough physics (mass, momentum, bodies as balls or cylinders, turning cheaper
than moving, no modelling of outstretched claws).

**First step, in flight:** workstream T
([brief](handoffs/ecology-v1-motor-inertial-opus-2026-09-16.md)) — every body a
uniform disc, rotation as energy-equivalent speed, energy envelope, apex grasp
excluded from turning; paired against the shipped sweep model on the apex and
on A's screen rows before it touches the cube.

**Done looks like:** a motor model where a body's motion cost is work against
inertia and drag: mass from structure, a moment of inertia from a simple shape,
a cost for accelerating (momentum) and for sustained speed, the same rule for
every body including the apex. Kept deliberately light: no collisions, no
contact forces, no rigid-body solver; a per-tick integrator on `(v, ω)` with a
power budget is the ceiling of ambition. Any step here is a whole-world change:
it goes through the matched calibration rows, records the model in the training
protocol, and is put to Wrysk before deployment.

## 4. Seasons and meta-climates (feature to explore)

Wrysk, 2026-09-16: seasons as a potential feature, with different meta-climates
depending on the seeded biosphere or the chosen map type. Not scheduled; the
theoretical-biosphere handoff is asked to say what a climate regime would do to
its web.

What it would mean on the current substrate: the prescribed-weather mode already
takes rain and evaporation as rates, so a season is a schedule over those two
plus, once they exist, day length (sky light) and temperature. A meta-climate is
the parameter set the schedule runs on (wet-dry monsoon, cold-warm, steady humid,
drought-prone), chosen with the map type or drawn from the same seed as the
biosphere so a world's plants are the ones that fit its year. The atmospheric
recycling mode in the terrain proposal (§4) is the other half: a season can move
water between the atmosphere store and the ground without inventing any.

Why it earns a place: the ecology's disturbance regime is otherwise flat. A dry
season that lowers the water table and contracts the pond is the cleanest way to
get the "dry weather contracts habitat toward the spring" story, dormancy and
seed banks a reason to exist, and boom-and-bust a rhythm a viewer can read.

Open questions for whoever picks it up: season length against organism lifetimes
(a season must be long enough for succession to show and short enough that a
desk viewer sees more than one); whether presentation lighting follows the
season or stays constant (roadmap §5 keeps it constant); whether the player
chooses a climate at world creation or unlocks weather control later.

## 5. Really good lighting on high-end PCs (feature to explore)

Wrysk, 2026-09-16: filtered sunlight through canopy, volumetric fog, raycast
shadows. Go ham on high-end PCs. The Tachyon panel and the cube keep the flat
pixel-art path; this is a second presentation tier for the desktop and, later,
the game, drawn from the same voxel world.

What the substrate already gives it: a true voxel volume with materials, free
water per voxel, a water table, the geometric sky-visibility fan (the same
hemisphere the ecology reads light from), canopy occupancy per stand with a real
crown height and radius, and a fixed elevated-orthographic camera. A lighting
pass has geometry to trace against; nothing has to be invented for it.

Candidates, roughly in order of what each buys: sun direction with raycast
shadows from terrain and crowns (the ecology's shade becomes visible, which the
first-wave presenter cannot show); filtered light under canopy as dappled
transmission rather than a flat tint; volumetric fog and haze that reads the
basin's humidity and the pond; wet surfaces and water refraction; emissive
glowcap light that is actually a light source; day and night once the ecology
has a day. Keep the ecology's light model as the source of truth: the picture
may shade more finely than the model, never differently.

Constraints: pixel art is still the look, so the lighting pass shades the
4 px per voxel picture rather than replacing it with 3D; the fixed camera keeps
this tractable (a single view, precomputable visibility); the cube and the
panel must never depend on it.

## 6. Deferred decisions

- Shoulder 0.85 vs 0.95: Wrysk does not want to tune now; 0.95 stays on the cube by env
  override, 0.85 stays the shipped default. Revisit when the GUI exists.
