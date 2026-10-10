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

**Ecological numbers** are the parameter registry: every path, value, unit and range is in the generated `design/parameters.md`. The table below lists only the knobs that are not registry rows yet.

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
| voxel **water flow** placeholders: `FLOW_PER_SUBSTEP` (**0.5**, the fraction of a head difference one face passes in one substep — the largest value that cannot make a pair overshoot) and `HEAD_PASSES` (**4**, how many local passes carry head through submerged water per substep, so pressure reaches 16 cells a tick) | `crates/cubarium-voxel/src/water.rs` (2026-09-18, the local exchange) | restart | **Untuned, and the whole of the new flow rule.** They replaced the connected-region solver, which levelled every water body in one substep and was 45 % of the tick; the exchange relaxes toward the same answer instead (a U-tube in about a second, a ten-column roofed passage in a few). `water_substeps` **4** is unchanged and is the third number in the same rule. Measured effects of the change, statistically: the world holds the same water (217.379 against 217.365 m³ over `community 60`) and the soil the ecology reads is the same to three decimals, but ten times fewer cells hold free water (1,995 against 19,524 — fewer, deeper puddles instead of a film over everything, which moves `water_depth_m` on support faces), the world drains a little faster, and a **closed surcharged passage no longer settles flat**. All three are recorded in `design/7_Research/voxel-tick-profile-2026-09-18.md`. |
| voxel **minimum spreading depth** placeholder: `MIN_SPREAD_DEPTH_M` (**1.25e-7 m**, 0.125 µm — free water shallower than this, in metres of depth in its own cell, makes no horizontal offer in the exchange; it still infiltrates, evaporates, falls and offers downward, and nothing is deleted) | `crates/cubarium-voxel/src/water.rs` (2026-09-22, package C) | restart | **Untuned; chosen from one census**, as the smallest decade past which the desktop world's forced shower stops getting steeply cheaper (shower step 20.9 -> 13.7 ms, proposals 303 k -> 171 k per tick; ten times the value gives 12.5 ms / 135 k, a hundred times 11.1 ms / 125 k). Pooled and pore water move by under 0.05 % at any of the three values. Ten times it fails `landform_composition`'s spring-route check, which counts invisible spray near the spring. |
| voxel tick **thread count** `SimConfig::threads` / `[voxel] threads` (0 = `available_parallelism`; the plain `World::step`/`Fauna::step` outside the sim crate use `cubarium_voxel::default_threads()`, the same count, `step_with(1)` is serial) | `cubarium_voxel_sim::SimConfig`; `cubarium voxel --config`'s `threads` | restart | **Execution only — it reaches no rule, no number and no picture**, and the schedule's own test holds the soil and every animal's face fixed across 1, 4 and 16 threads. Untuned: the only evidence is the bench addendum in `design/7_Research/voxel-tick-profile-2026-09-18.md`, which on this desktop has the tick flat from 8 threads up (the parallel legs are the exchange's column scan and fauna `sense`, and `sense` has one animal per worker by then). The embedded target will want a different number and a GUI slider is the wrong shape for it — a launch-line field is. |
| voxel **ground-pool bulk density** `senses::POOL_BULK_DENSITY` (**80** organic units per m³): a litter / carrion / dead-wood pool of organic `m` on a face of area `A` occludes a cone ray only below `m / (A · 80)` over the face | `crates/cubarium-voxel-fauna/src/senses.rs` (2026-09-22, contract v2, P5-A, decision D3); recorded in the founder manifest's canonical text as `pool_density` | restart, fresh world only; **moves the manifest digest** (a trained centre is refused) | **Untuned placeholder.** Chosen so a reference litter tile (`M_EMIT` 0.05) is about 1 cm deep on a 0.25 m face and 4 cm on a 0.125 m one — a film to an eye, per decisions §6 ("a ground pool is not a wall"). Lower it and heaps start hiding the floor rows of the fan. |
| voxel **founder cruise** `manifest::CRUISE_BODY_LENGTHS_PER_S` (**1.0** BL/s): browser **0.375 m/s**, shredder **0.19 m/s**, `forward_reference_m` = cruise × 0.25 s | `crates/cubarium-voxel-fauna/src/manifest.rs` (2026-09-22, contract v2, decision D2) | restart; **moves the manifest digest** | The standing rule (pace in body lengths), not a tuned number. `motor_respiration_per_s` is unchanged, so a metre costs two-thirds of what it did at 0.67 BL/s. |
| voxel **heuristic teachers' escape reflex** (`controller.rs`): `MOTOR_REFUSED` **0.5** (motor_delivery below it is a refused motor), `ESCAPE_SAMPLES` **3**, `ESCAPE_CREEP` **0.3**, `DROP_PROXIMITY` **0.55** (forward sector's mean hit proximity with only the floor rows hitting), `FLAT_CLEAR` 9/15 ± half a ray | `crates/cubarium-voxel-fauna/src/controller.rs` (2026-09-22, P5-A item 9) | recompile | Teacher internals, not model numbers: nothing in the world reads them, but the clone and ES start from what these teachers do. The drop threshold sits between an adult's one-voxel step down (≈0.65, walkable) and a two-voxel drop (≈0.47). |
| voxel **shredder teacher's correlated walk** (`controller.rs`, `Wander`; the browser keeps its flip-every-three-samples search): hold the heading `WANDER_HOLD` **8–24** samples (2–6 s), then turn `WANDER_TURN_SAMPLES` **1–4** samples at effort `WANDER_TURN` **0.8**, sign random, seeded from the body's first observation | `crates/cubarium-voxel-fauna/src/controller.rs` (2026-09-22, P5-C S3) | recompile | Teacher internals, placeholders: the shredder teacher's no-signal search in place of the flip-every-three-samples zig-zag, which covered ~7 m² in 4 min. |
| voxel **shredder training production replay** (P5-C S1): `PRODUCTION_BUCKET_TICKS` **100** (a replay's litter deposits are summed per 100 ticks and handed in at the bucket's first tick) | `crates/cubarium-search/src/es/voxel/landscape.rs` | recompile | A fixture choice, not a rate: training only. |
| voxel **startup acceptance bar** `habitat::ACCEPT_FOUNDER_HOURS` (**N = 1.0** founder-hour): a seeded world is accepted only if every founder-occupied walkable component holds reachable edible stock of at least `N · founders · upkeep`, upkeep = the lineage's adult basal maintenance `core.maintenance_per_s · core.body_max · 3600` (browser **0.18**, shredder **0.045** organic/h), and (shredder) a living litter-shedding stand or glowcap rooted in it | `crates/cubarium/src/voxel/habitat.rs` (2026-09-22, package 4; decisions §8) | restart, fresh world only | **An authored placeholder, not a measured bar.** Stock is organic matter as it stands, not assimilated; production per hour is reported beside it and not gated (package 6 needs it). At seed 1 every preset passes on its first try, so the bar has not yet refused anything on a shipped ring. |
| voxel **dung** `GUT_VOID_S` (**30 s**) | `crates/cubarium-voxel-fauna/src/step.rs` (2026-09-24, Wrysk: animals poop) | restart, fresh world only (fauna snapshot schema **17**) | **An authored placeholder.** Every animal holds a `gut`: the `(1 − yield)` undigested share of each bite (with its energy), the mineral a bite brings beyond what its tissue needs, and the mineral respiration sheds with the organic matter it burns (same fraction rule as energy, so body mineral stays in proportion to tissue). Every `GUT_VOID_S` of age the gut is dropped on the animal's face as litter; a death puts it in the corpse. Supersedes row 61's note (1): excess mineral is no longer deposited per bite, and the undigested share is no longer respired. |
| voxel **founding-loop bounds**: `HABITAT_TRIES` **8** (lake-gated worlds pre-rolled, seeded and judged before the best is kept), the opening shower's cap `SHOWER_CAP` **6000** ticks and the drain's cap `DRAIN_CAP` **2400** ticks | `crates/cubarium-voxel-sim/src/found.rs`, `crates/cubarium-voxel-sim/src/habitat.rs` (2026-09-22, package 4; moved out of the host by P5-B) | restart | Bounds, not rates: the drain stops on `World::settle`'s convergence and the shower when its allowance has fallen; the caps only end one that would not. Measured at seed 1: drain converges in 100 (`small`), 1,889 (`default`), 1,235 (`wide`) ticks. Each habitat try costs one pre-roll: 2,066 / 4,845 / 3,735 ticks on `small` / `default` / `wide`, 10–16 s on the desk. |
| voxel **training-world fixture placeholders** (P5-B): the arena's `ARENA_BYSTANDERS` **2** bodies of the other lineage, arriving `StartingStores::HUNGRY`, kept `BYSTANDER_CLEARANCE` **9** squared standard columns (0.75 m) from the start and every patch; a shredder **cap** patch split **half** glowcap cap tissue, **half** the litter it fruits on; arena **carrion** laid at the arena litter's mineral fraction **0.02** and energy density **2.0**; the browser **seedling** patch a bloomcrown at **0.2 · `wood_max`** (the seedling stage's own bound); D11's body draw **uniform in structure** over `[body_min, body_max]` with **no reserve**; the landscape redraw stride `LANDSCAPE_REDRAW_STRIDE` **1000**; the mid-shower capture at the first tick the shower's allowance is **half** what it was after its first tick | `crates/cubarium-voxel-sim/src/arena.rs`, `crates/cubarium-search/src/es/voxel/{driver,landscape}.rs`, `crates/cubarium-voxel-sim/src/habitat.rs` (`pre_roll_capturing`) (2026-09-22, P5-B) | code only; training fixtures, not the live world | **Fixture choices, not rates, and nothing measured any of them.** The stocks per patch are the arena's old ones (litter tile 0.2, Stage-B patch 0.015 halved for the initial one, browser stand 0.06). The stride exists because consecutive seed bases kept each other's worlds under the live `base + 1` redraw (13 of 48 training foundings). |
| pursuit stopping rule `--pursuit-stop {reach-envelope,half-space}` (ships `reach-envelope` since 2026-09-16) | `cubarium-search` only — `calibrate`, `precondition`, `factorial`, `es-population`, `apex-audit`; a `World` transient (`World::set_pursuit_stop`), never a `WorldConfig` field and never persisted | restart, fresh world only | **Not a knob for the viewer.** `half-space` exists to reproduce rows retained before the adoption and nothing else; the cube has no reason to run it, and a world saved under one rule is refused under the other (schema 17). List it here so the list is complete, not so it gets a slider. |
| voxel **standing-water reader** threshold `cubarium_voxel::STANDING_SUPPORT_FILL` (**0.5**: water above a cell counts as standing only while that cell is at least half full) | `crates/cubarium-voxel/src/world.rs` (package N) | restart | A reader over fills, not solver state: moving water deeper than half a cell reads as standing. |
| voxel habitat seeder, package N rows of `PRODUCERS` (per m² / patch / cap): vaulttree **0.1 / 1 / 12** (placed first), siphonreed **4.0 / 6 / 64** (a reed bed over a narrow habitat), lanternberry **0.25 / 3 / 40** | `crates/cubarium/src/voxel/habitat.rs` | restart | Placed wherever their own gates pass; interim dossier colours in `voxel/stand.rs` (not a look decision). |
| voxel **organism models** (package V, `design/handoffs/voxel-organism-models-2026-09-23.md`): `organisms = "models" \| "glyphs"` and `models_dir` (**assets/voxel-models**) in the voxel TOML; the bake's `VARIANTS` **3** per size step and `BINS` **5** animal size bins (newborn to adult, evenly spaced in length); a size step wherever the crown height crosses a **whole voxel** or the radius a **half voxel** | `crates/cubarium/src/voxel/mod.rs` (`VoxelConfig`), `scripts/blender/bake_voxel_models.py` | restart (keys); rebake + restart (bake constants) | **Placeholders.** Thinning keeps a foliage/drape cell iff 24 bits of a splitmix64 hash of `(stand id, offset, layer)` fall under the layer's `stock / capacity` (`model::keeps`); the variant is the same hash of the id mod the variant count. Wilt is quantised to **5** levels (`model::WILT_LEVELS`), a bloom opens at a parcel of **half** a package (`model::RIPE_AT`), a starving animal's body takes the wilt tint at **0.5** (`animal::STARVING_TINT`) and its accent `#7A2A78`. |

## 2. Artwork pass

Wrysk, 2026-09-16, on G's soil-band dead-wood stub (a cut-down stage-0 mushroom in the ash
tone): "i think a purpose drawn one is better, but lets put that in a backlog too and do a
artwork pass later."

**Done looks like:** one authored tile set for a dead-wood snag in the soil band (and its
fade), under `assets/atelier`, replacing the `Mask::Axial` cut in `habitat.rs`; the
`art_ecology` soil-mark tests re-pointed at it. Also queued for the same pass, from B's and
G's notes: the stripped-canopy silhouette is less articulated than the side-face plant; the
water band shows nothing for dead wood; a tall dead column has no authored crown.

Queued by the package V colour pass (2026-09-23; Wrysk: "do a colour pass. feel free to get
creative with it. we dont need a strict 8/16bit color palette"): **every organism colour on
the model path is a look placeholder** for Wrysk to judge, all in one file,
`crates/cubarium/src/voxel/colours.rs`. One hue family per species (bloomcrown hot pink,
umbrellafrond turquoise, vaulttree emerald with cyan drapes, lanternberry lime with orange
lanterns, siphonreed pale celadon with magenta heads, springturf mint, velvetpad cobalt,
stonecushion pale lilac, glowcap aqua); a base-to-tip ramp in `BANDS` **4** height bands;
shadows pulled **0.55** toward indigo `#140A3C`; coral frondgrazers with cyan eyes, gold
littershredders; the cropping flash pale yellow `#FFF27A`. Judge with
`cargo run -p cubarium --release --example voxel_specimens -- OUT.png [--wilt W]`, which
draws every species at three sizes on a flat strip. Known and not colour: the adult
siphonreed bakes as a solid 3×3 column (its stems merge at 0.125 m), and glowcap and
littershredder are one or two cells.

Queued by voxel round 5c: **the frondgrazer's glyph is an interim placeholder.** The
presenter draws an animal as a 2x1x2 block of voxels in one chartreuse placeholder colour
(`crates/cubarium/src/voxel/animal.rs`, `INTERIM_ANIMAL_SRGB`), with no articulation, no
facing and no sub-tick motion between faces, and the GPU part table reserves ids 6 and 7
beside it for whatever the real look needs. The colour was chosen only to be unmistakably
*not* art — it is the one hue neither the five plants nor the strata use. The study at
68a8215 is paused and not canon; when `design/voxel-art-direction.md` lands, a follow-up
package replaces this module's geometry and colour and the `sink/gpu` stager that feeds it.

Queued by voxel round 5a: **remains on a voxel site have no glyph at all.** `Ground::carrion`
is a stock the model now keeps and decomposes, and neither the voxel presenter
(`crates/cubarium/src/voxel/stand.rs`, which draws stands and seed sprouts) nor the cube's
soil band draws it, so a corpse is invisible where a seed cohort is a sprout cell. The round-5a
brief put the glyph in the backlog on purpose — nothing deposits carrion until a consumer
exists. The look belongs to Wrysk's own art-direction thread
(`design/handoffs/voxel-art-direction-handoff-2026-09-17.md`): the agent-made consumer study
is **paused and not approved**, so nothing here proposes a palette or a shape for remains.

## 3. A light physics engine for movement

Wrysk, 2026-09-16: "i dont think a light physics engine is the wrong call to
implement at some point regardless", after directing that movement cost follow
rough physics (mass, momentum, bodies as balls or cylinders, turning cheaper
than moving, no modelling of outstretched claws).

**First step (flat-era proposal; not started on the voxel world):** every body a
uniform disc, rotation as energy-equivalent speed, an energy envelope, apex grasp
excluded from turning. It goes to Wrysk before it touches a live world.

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

### 5a. Cached surface shadows (Wrysk, 2026-09-25: "backlog it for the next round")

Today every lit pixel traces up to 128 cells toward the sun (`sunReaches`), every frame.
That is 2.8 of 4.9 ms per lit frame on the 5080 at 13 px (V's switch table). The shadows
barely change, so trace them only when the sun or the world moves.

- **Shadow map.** Bake a light-space shadow map (the depth of the first occluder, a few
  texels per voxel), and have each pixel do one lookup. V's per-voxel sun-visibility volume
  (`sunvis`) is too coarse: surfaces would get blocky shadow edges. Crowns are still
  deterministic per cell through `crownLets`, so the map stays exact.
- **Moving things.** Animals cast shadows, and those would lag behind them between bakes.
  Keep a cheap every-frame check for animals only, or drop animal shadows.
- **A moving sun.** Fading between bakes a minute apart can ghost a double shadow at dawn
  and dusk. The bake is cheap on the GPU, so rebake every few seconds while the sun moves
  and never while it holds still. Nothing pops.
- **The panel.** This may be the main lever for lit on the Tachyon. Lit there was 288 ms
  per redraw against 42 ms flat. Measure how much of that is shadows before counting on it.

## 6. Deferred decisions

- Shoulder 0.85 vs 0.95: Wrysk does not want to tune now; 0.95 stays on the cube by env
  override, 0.85 stays the shipped default. Revisit when the GUI exists.

## 7. CPU voxel renderer stays basic (Wrysk, 2026-09-23)

The CPU renderer keeps its current flat look for good: it is the PNG diagnostics path,
and less detail helps there. Textures and other art go into the GPU renderer only.
Later, not now: layers in the CPU renderer that can be toggled on and off (terrain,
water, flora, fauna, haze) for diagnostic captures.

## 8. Colour morphs: alternate colourways as lineages or an option (Wrysk, 2026-09-24)

Each animal has a final colourway plus a kept alternate
(`art/gen/SIGNOFF.md`, "Animal bodies and colour: final"). Wrysk likes some alternates
(the orchid-pink seedporter, the periwinkle frondgrazer), so keep them. Later, not now:
either a heritable colour morph (a lineage trait the presenter reads, so a population
visibly carries its founder's colouring) or an operator option to pick the colourway.

## 9. Presentation decoupled from the sim (Wrysk, 2026-10-07)

Pinned. On today's renderer the loop packs each tick right after the step, so a tick costs
step + pack (wide 640×128×96 world at minute 60: 13 + 15 ms; about 1.6× speed headroom at
the 20 fps this renderer needs). A plain double buffer — copy what the pack reads after
each step, pack it on its own thread while the next step runs — would make it about
max(step, pack) and roughly 3×. Wrysk: "ill pin it for now. with the new renderer we
should support things like running uncapped by presentation and just sampling the world to
render it." The Godot renderer should take that shape: the sim runs at its own speed and
the presentation samples published snapshots.

## 10. One save format (Wrysk, 2026-10-07: "i guess if its an easy package" — it wasn't)

Two formats coexist. The game's whole-world save (directory with `meta.json`, written by
`cubarium-session`'s store, `cubarium voxel`'s `save`, and `cubarium-world`) is what the Godot
game loads. The study tools write the older three-file layout (`world.bin`, `flora.bin`,
`fauna.bin`, `minute`) in about 13 files (`voxel_census`, `phase_profile`, `succession_study`,
`light_study`, …), read back by `cubarium-voxel-sim/src/saved.rs`. Unifying means moving the
whole-save format down into the sim crate (the session crate sits above it) and pointing every
study tool at the one writer and loader. Medium; touches tools other threads use mid-study.
`cubarium-world found|advance` covers making game-loadable saves headless meanwhile.

## 10. Water table as a new-world setting (Wrysk, 2026-10-07)

The wide terrarium's lake grows by its water table (`water_level_m`), and Wrysk picks 0.375
or 0.75 per seed. He would like to set it when choosing a seed in the new-world screen
(`design/handoffs/godot-direction-2026-10-07.md`): "it could be sortof approximate. doesnt
have to be a full recomputed world which might take way too long when just like moving a
slider". A cheap preview is possible because the table only decides what floods on a fixed
heightfield: re-flood the generated terrain at the slider's level without regenerating.
First pass: "lets not put a lot of effort on it and regenerating the worlds is fine until i
get tired of waiting lol".

`lake_scale` (a larger carved basin) is parked at 1.0 in the wide config: at 1.5 and 2.5 it
read as a dug-out. Its basin shaping needs rework before it is used; an unmerged attempt at
shaping the basin before erosion is on branch `water-table-basin-wip` (erosion silts such a
basin level with the yard; the massif's first riser is a long straight cliff any high table
reaches).

## 11. Seedporters groom loftstriders (Wrysk, 2026-10-08: idea, someday)

"eventually might be cool to have seedporters be able to climb loftstriders and like clean out gunk
of bugs from between their tusks." This would be a cleaner symbiosis. Seedporters already climb (the
climb pose, `5e819d74`), so a loftstrider would become something they can climb. They would feed on
bug gunk that builds up round the mouth ring, between the six tusks. Not designed and not scheduled.

## 11. Move the renderer to wgpu (Wrysk, 2026-10-08: "yeah this is why we shouldve been using wgpu... backlog it i guess. no need to switch graphics backends tonight")

The no-unsafe rule (WORKING_POLICY, 2026-10-08) and the renderer collide: `cubarium-gpu` talks
to Vulkan through `ash`, whose every call is `unsafe` in our code (213 uses). `ash` was chosen
only because the Tachyon panel needed zero-copy dma-buf scanout, which wgpu couldn't export
without dropping to `wgpu-hal`; the panel no longer constrains the renderer (D-0005).
- wgpu gives a safe API, Metal natively on macOS (instead of MoltenVK), and lifts the
  Vulkan 1.1 limits the panel imposed.
- Size: about 11,000 lines in `cubarium-gpu` plus the GPU sink's pass setup, and 3,400 lines of
  GLSL (keep them as SPIR-V through naga or translate to WGSL).
- The frame bridge into Godot stays a CPU readback (wgpu can't share Godot's device).
- Order question with H2 (the model pass for smooth rotating animals, which adds new GPU code):
  write H2 on `ash` now and port it later, or migrate first and write H2 on wgpu.

## 12. Seedporter night roosts and rain-grounded bellwings (Wrysk, 2026-10-08: one behaviour package after the foliage/calf work)

"seedporters should prefer to roost in vault trees at night, or up top mesas they can reach by climbing
latticevine." Queued to start once the foliage-states and loftstrider-calf work is finished. It is
behaviour only: at night a seedporter prefers a vaulttree crown, or a mesa top it reaches up a
latticevine wall, over its current rest spot. Check today's night and rest rules (animal circadian
rhythm, weather work of 2026-09-24) before designing.

Same package, Wrysk: "rain should ground bellwings." In rain a bellwing stops flying and lands (on a
ledge, a crown or under cover; check its perch kinds), then flies again when the rain stops. It reads
the same rain signal the plants and weather already use.

## 13. Direct an animal: select it, then command it (Wrysk, 2026-10-08, later)

Wrysk: with an animal selected, let the viewer control it: make it move somewhere, eat a
certain food, or even force it to hunt or attack something. Planned for later; not scheduled,
no design decisions made.

Shape of the idea (all open, none binding):
- **Select, then command.** Click an animal in the viewer/app, then give it an order: go to a
  point, eat a named food (plant species, carcass, prey), hunt or attack a chosen target.
- **Orders feed the animal's own machinery.** The order should override or bias the target the
  brain/heuristic picks, and the body still pays motor cost, hunger, injury and fatigue as
  normal, so a forced animal can starve or die. Check how the brain interface (observations,
  heuristic seeding) would take an injected goal before choosing between "bias" and "override".
- **Open questions:** does the order expire (on arrival, after N seconds, on death of target),
  can the animal refuse when it is exhausted or starving, do orders leave a care-journal event
  (`--care`), and is this a display-mode feature only or also allowed in training/experiments
  (probably display only, so runs stay unperturbed).
- **Relation to other work:** the selection UI belongs to the Godot app shell (D-0005); the game
  roadmap (`design/game-roadmap-2026-09-16.md`) is parked until the ambient piece is done, and
  this feature fits there naturally.

**Done looks like:** select an animal in the app, issue move / eat / hunt orders, watch it carry
them out within its normal physics, with orders listed in the operator controls (§1).

## 14. Plant pass leftovers (Wrysk, 2026-10-08: "backlog the shadows thing, vines, and reflections. for now lets just call it done")

The plant pass (`[figures] plants = "models"`, merged e5f214f5, the default since) sways plants
and grows them in voxel by voxel. Three gaps were left:
- **Shadows during a change.** When a plant changes, its shadow and canopy light switch at the
  start of the change, while its voxels take `foliage_fade_s` to grow in. The stamp used to change
  them cell by cell. The shadow should follow the grow-in.
- **Vines don't sway.** Latticevines stay still on their walls.
- **Plants seen through water** (reflections, a fall, a pool's surface) are drawn by the walk,
  still and under the water's colour, so they don't sway or grow in.

## 15. Where edited-in life comes from, and where removed life goes (Wrysk, 2026-10-09: "its important and i think might be worth implementing depending on where we want to go in the game version")

The world-edit API (merged 39b02553) books every insertion as matter from outside the world
(`introduced_*_in` for animals, `seeded_*_in` for plants) and every removal as matter leaving it
(`removed_*_out`, no carcass). Picking up books out and setting down books back; a move books
nothing. The ledger closes, but the world's own cycle neither pays for an insertion nor gets a
removal back.

Options, to choose when the game version's direction is clearer:
- **Outside (today).** Free, simple; right for study scenarios.
- **Closed world.** A removal leaves a body or litter where it stood (feeding shredders and the
  soil); an insertion draws its organic and mineral matter from the local soil, litter or
  detritus pool, and is refused when there isn't enough.
- **A hidden biomass store** (Wrysk's idea): the xeno-ecologist's ship or base holds a stock.
  Removals go into it, insertions draw from it, and it can run low. That makes the player's
  interventions a budget with game meaning, and keeps the world itself closed.

Studies can keep "outside" either way (a per-call or per-scenario source switch).

## 16. Voxelize animals live on the world grid (Wrysk, 2026-10-10: "its worth doing a study but lets hold off")

Idea from the FEED-RIG trial's flipbook arm: draw animals from their 3D models and have the
GPU voxelize them every frame on the world's voxel boundaries, so all movement is on the
grid. Held as a study, not started: it needs a skinned-mesh pass, a per-frame voxelizer
that matches the bake's rules (coats, open faces, glyphs), and higher-fidelity models; on
the world grid, walking and turning would also step a whole voxel at a time, as the
flipbook arm did. Shipped instead: the bone rig with turned cubes (B).
