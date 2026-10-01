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

**Ecological numbers are the parameter registry** (package R1,
`design/ecology-parameter-registry-2026-09-28.md`): every number of the voxel world's
`Config`, the plants' `FloraConfig` and the animals' `FaunaConfig` is a row, keyed by its
path. Set them in the config TOML's `[world]` / `[flora]` / `[fauna]` tables or with
`--set PATH=VALUE` (`cubarium voxel`, `voxel_census set=`); an unknown path or a value
outside its range is refused, and a run writes its resolved set as `params.toml` beside
its output. The dump at the end of this section is generated — do not edit it by hand;
`cargo run -p cubarium-voxel-sim --example params -- --backlog design/backlog.md`
rewrites it. The table below lists only the knobs that are **not** registry rows yet:
CLI flags, presentation, and the `const`s package R2 moves into the configs.

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

<details><summary>The registry dump: every parameter path, value, unit and range</summary>

<!-- registry dump: begin (cargo run -p cubarium-voxel-sim --example params -- --backlog design/backlog.md) -->

**`world.*`** (85 rows)

| path | value | unit | range | search | grade | provenance | what |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `world.width` | 128 | voxels | [1, inf) |  | - | structural | voxels along the strip (x wraps) |
| `world.height` | 48 | voxels | [1, inf) |  | - | structural | voxels of vertical extent |
| `world.depth` | 24 | voxels | [1, inf) |  | - | structural | voxels into the habitat |
| `world.voxel_m` | 0.25 | m | (0, inf) |  | N | decision | edge length of one voxel |
| `world.seed` | 1 | - | [0, inf) |  | - | structural | seed for every deterministic stream |
| `world.noise_seed` | 0 | - | [0, inf) |  | - | structural | seed for the landform's final wobble alone (0: from `seed`) |
| `world.water_substeps` | 4 | substeps per tick | [1, inf) |  | N | numerics | free-water solver substeps per tick |
| `world.free_transfer_cap` | 0.0 | fraction of a cell per substep | [0, inf) |  | N | numerics | cap on one cell's free-water change per substep (0: none) |
| `world.rain_m_per_s` | 0.0 | m/s | [0, inf) | 0.00001–0.0001 log | A | placeholder | R0: prescribed rain onto exposed tops (open budget) and the unit of a mode's rate_r0 |
| `world.evaporation_m_per_s` | 0.0 | m/s | [0, inf) | 0.000001–0.00003 log | D | tuned | evaporation from exposed free-water surfaces |
| `world.spring_k_m2_per_s` | 0.02 | m2/s | [0, inf) |  | C | placeholder | spring conductance from the aquifer: Q = k * max(head - h_spring, 0) |
| `world.aquifer_porosity` | 0.35 | fraction of footprint | (0, inf) |  | B | derived | aquifer pore space per metre of head (>= soil's 0.35) |
| `world.initial_aquifer_head_m` | 0.0 | m above y=0 | [0, inf) |  | C | tuned | water table at creation (0: a dry aquifer) |
| `world.outlet_m3_per_s` | 0.05 | m3/s | [0, inf) |  | C | tuned | free water an open outlet exports |
| `world.initial_atmosphere_m3` | 0.0 | m3 | [0, inf) |  | - | structural | water aloft at creation (closed budget) |
| `world.reentry_m3_per_s` | 0.0 | m3/s | [0, inf) |  | C | tuned | closed budget: the stream returned at the spring cell |
| `world.reentry_floor_head_m` | 0.0 | m above y=0 | [0, inf) |  | - | structural | water table at which the aquifer-fed stream stops |
| `world.lake_drain_m2_per_s` | 0.0 | m3/s per m of head | [0, inf) |  | C | derived | lake floor joined to the aquifer: flow = k * (lake - table) |
| `world.day.length_min` | 120.0 | min of sim time | (0, inf) |  | C | decision | one day and night (cycle); the 'day' the climate counts in |
| `world.day.daylight` | 0.67 | fraction of the cycle | [0.05, 0.95] | 0.4–0.7 | B | decision | the share of the cycle the sun is up |
| `world.day.start_hour` | 8.0 | h on the day's 24 h clock | [0, 24) |  | - | decision | where a fresh world starts |
| `world.day.latitude` | (absent) | deg north | [-90, 90] |  | - | decision | local clock: the place whose sunrise the day follows |
| `world.day.longitude` | (absent) | deg east | [-180, 180] |  | - | decision | local clock: the place's longitude |
| `world.day.plant_night_factor` | 0.05 | fraction of the daytime rate | [0, 1] | yes | - | decision | plant maintenance and senescence in the dark (1: no night rule) |
| `world.climate.weather_speed` | (absent) | x nature | (0, inf) | 1–12 log | C | decision | how much faster than nature the weather runs, when fixed ("auto" otherwise) |
| `world.climate.temperature_c` | 26.0 | degC | [0, inf] |  | A | preset | mean air temperature |
| `world.climate.diurnal_c` | 3.0 | degC | [0, inf] | 2–5 | A | preset | diurnal swing, +/- |
| `world.climate.drift_sd_c` | 1.0 | degC | [0, inf] |  | B | preset | day-to-day drift's standard deviation |
| `world.climate.drift_days` | 3.0 | days of nature | [0, inf] |  | B | preset | the drift's time scale |
| `world.climate.fronts_per_day` | 0.1 | per day | [0, inf] |  | B | preset | cold fronts per day |
| `world.climate.front_min_c` | 2.0 | degC | [0, inf] |  | B | preset | shallowest front |
| `world.climate.front_max_c` | 4.0 | degC | [0, inf] |  | B | preset | deepest front |
| `world.climate.front_onset_min` | 30.0 | min of nature | [0, inf] |  | B | preset | a front's cooling ramp |
| `world.climate.front_recovery_days` | 0.3 | days of nature | [0, inf] |  | B | preset | a front's recovery |
| `world.climate.front_window_min` | 30.0 | min of nature | [0, inf] |  | B | preset | F = 1 this long after a front arrives |
| `world.climate.rain_cool_min` | 20.0 | min of nature | [0, inf] |  | B | preset | rain cooling's time constant |
| `world.climate.capacity_share` | 0.03 | fraction of the world's water | (0, inf] | 0.01–0.1 log | A | preset | what the air holds at the mean temperature |
| `world.climate.afternoon_width` | 0.08 | fraction of the cycle | [0, inf] |  | A | preset | the afternoon convection bump's width |
| `world.climate.cooling_cap_c_per_h` | 3.0 | degC per sim hour | [0, inf] |  | N | numerics | cap on the cooling rate K |
| `world.climate.dry_gap_min_s` | 300.0 | s | [0, inf] | 0–1800 | C | decision | shortest wait after an event before a natural start (0/0: off) |
| `world.climate.dry_gap_max_s` | 900.0 | s | [0, inf] | 0–1800 | C | decision | longest wait after an event before a natural start |
| `world.climate.fog_form_rh` | 0.9 | RH | [0, inf] |  | - | visual | fog forms at or above (visual only) |
| `world.climate.fog_clear_rh` | 0.85 | RH | [0, inf] |  | - | visual | fog clears below (visual only) |
| `world.climate.fog_span_rh` | 0.15 | RH | [0, inf] |  | - | visual | fog density span (visual only) |
| `world.climate.fog_dawn_before` | 0.1 | fraction of the cycle | [0, inf] |  | - | visual | dawn window before sunrise (visual only) |
| `world.climate.fog_dawn_after` | 0.06 | fraction of the cycle | [0, inf] |  | - | visual | dawn window after sunrise (visual only) |
| `world.climate.fog_burn_after` | 0.1 | fraction of the cycle | [0, inf] |  | - | visual | fog burns off this far past sunrise (visual only) |
| `world.climate.fog_cooling_c_per_h` | 0.5 | degC per sim hour | [0, inf] |  | - | visual | fog forms while the air cools faster (visual only) |
| `world.climate.fog_form_min` | 10.0 | min of nature | [0, inf] |  | - | visual | fog's forming time (visual only) |
| `world.climate.fog_clear_min` | 15.0 | min of nature | [0, inf] |  | - | visual | fog's clearing time (visual only) |
| `world.climate.strike_mean_s` | 30.0 | s | [0, inf] |  | - | visual | mean lightning interval in a downpour (visual only) |
| `world.climate.strike_min_s` | 10.0 | s | [0, inf] |  | - | visual | shortest lightning interval (visual only) |
| `world.climate.strike_max_s` | 60.0 | s | [0, inf] |  | - | visual | longest lightning interval (visual only) |
| `world.climate.storm_stream_gain` | 4.0 | x stream flow | [0, inf] |  | C | decision | stream swell at a downpour's peak (0: off) |
| `world.climate.opening_min_s` | 60.0 | s | [0, inf] |  | N | preset | the host's opening rain falls at least this long |
| `world.climate.drizzle.lambda_max_per_h` | 1.5 | events per sim hour | [0, inf] | 0.5–5 log | C | tuned | ceiling of the event hazard |
| `world.climate.drizzle.rh50` | 0.98 | RH or moisture M | [-inf, inf] | 0.7–1.3 | C | tuned | humidity at which the hazard's logistic is half |
| `world.climate.drizzle.width` | 0.03 | RH or moisture M | (0, inf] |  | N | tuned | the logistic's width |
| `world.climate.drizzle.g_aft` | -0.8 | x afternoon bump | [-inf, inf] | 0.5–30 log | C | tuned | convective boost (1 + g_aft * A) |
| `world.climate.drizzle.g_cool` | 1.0 | x cooling rate K | [-inf, inf] | 0–2 | C | tuned | hazard boost from cooling air |
| `world.climate.drizzle.g_front` | 0.5 | x fresh-front flag | [-inf, inf] | 0–5 | C | tuned | hazard boost in a fresh front |
| `world.climate.drizzle.rate_r0` | 0.1 | x R0 | [0, inf] | 0.05–4 log | A | tuned | the rate it rains at |
| `world.climate.drizzle.stop_rh` | 0.8 | RH or moisture M | [0, inf] | 0.3–0.9 | C | tuned | the event ends when its reading falls to this |
| `world.climate.drizzle.longest_min` | 90.0 | min of nature | (0, inf] | 10–120 log | B | tuned | or when it has rained this long |
| `world.climate.drizzle.cool_c` | 0.5 | degC | [-inf, inf] |  | B | tuned | while it rains the air relaxes toward this much cooler |
| `world.climate.shower.lambda_max_per_h` | 1.5 | events per sim hour | [0, inf] | 0.5–5 log | C | tuned | ceiling of the event hazard |
| `world.climate.shower.rh50` | 1.0 | RH or moisture M | [-inf, inf] | 0.7–1.3 | C | tuned | humidity at which the hazard's logistic is half |
| `world.climate.shower.width` | 0.08 | RH or moisture M | (0, inf] |  | N | tuned | the logistic's width |
| `world.climate.shower.g_aft` | 4.0 | x afternoon bump | [-inf, inf] | 0.5–30 log | C | tuned | convective boost (1 + g_aft * A) |
| `world.climate.shower.g_cool` | 0.25 | x cooling rate K | [-inf, inf] | 0–2 | C | tuned | hazard boost from cooling air |
| `world.climate.shower.g_front` | 1.0 | x fresh-front flag | [-inf, inf] | 0–5 | C | tuned | hazard boost in a fresh front |
| `world.climate.shower.rate_r0` | 1.0 | x R0 | [0, inf] | 0.05–4 log | A | tuned | the rate it rains at |
| `world.climate.shower.stop_rh` | 0.7 | RH or moisture M | [0, inf] | 0.3–0.9 | C | tuned | the event ends when its reading falls to this |
| `world.climate.shower.longest_min` | 30.0 | min of nature | (0, inf] | 10–120 log | B | tuned | or when it has rained this long |
| `world.climate.shower.cool_c` | 1.5 | degC | [-inf, inf] |  | B | tuned | while it rains the air relaxes toward this much cooler |
| `world.climate.downpour.lambda_max_per_h` | 1.0 | events per sim hour | [0, inf] | 0.5–5 log | C | tuned | ceiling of the event hazard |
| `world.climate.downpour.rh50` | 1.13 | RH or moisture M | [-inf, inf] | 0.7–1.3 | C | tuned | humidity at which the hazard's logistic is half |
| `world.climate.downpour.width` | 0.08 | RH or moisture M | (0, inf] |  | N | tuned | the logistic's width |
| `world.climate.downpour.g_aft` | 20.0 | x afternoon bump | [-inf, inf] | 0.5–30 log | C | tuned | convective boost (1 + g_aft * A) |
| `world.climate.downpour.g_cool` | 0.5 | x cooling rate K | [-inf, inf] | 0–2 | C | tuned | hazard boost from cooling air |
| `world.climate.downpour.g_front` | 3.0 | x fresh-front flag | [-inf, inf] | 0–5 | C | tuned | hazard boost in a fresh front |
| `world.climate.downpour.rate_r0` | 2.0 | x R0 | [0, inf] | 0.05–4 log | A | tuned | the rate it rains at |
| `world.climate.downpour.stop_rh` | 0.5 | RH or moisture M | [0, inf] | 0.3–0.9 | C | tuned | the event ends when its reading falls to this |
| `world.climate.downpour.longest_min` | 50.0 | min of nature | (0, inf] | 10–120 log | B | tuned | or when it has rained this long |
| `world.climate.downpour.cool_c` | 3.0 | degC | [-inf, inf] |  | B | tuned | while it rains the air relaxes toward this much cooler |

**`flora.*`** (565 rows)

| path | value | unit | range | search | grade | provenance | what |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `flora.bloomcrown.substrate_uptake_per_s` | 0.0 | organic/s per unit mycelium | [0, inf) | yes | C | placeholder | saprotroph: dead wood and litter withdrawn |
| `flora.bloomcrown.substrate_yield` | 0.0 | fraction | [0, 1] | 0.2–0.7 | A | placeholder | saprotroph: share of the substrate taken that becomes tissue |
| `flora.bloomcrown.establish_substrate_min` | 0.0 | org-units | [0, inf) |  | C | placeholder | saprotroph: substrate a spore cohort needs in its box |
| `flora.bloomcrown.substrate_reach_up_down` | 1 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | saprotroph: the mycelium box's vertical reach |
| `flora.bloomcrown.alpha` | 2.0 | foliage per unit wood | [0, inf) | 0.05–3 log | D | placeholder | foliage cap P_cap = alpha * W |
| `flora.bloomcrown.wood_max` | 0.6 | org-units | (0, inf) | yes | D | placeholder | the most living wood one stand holds |
| `flora.bloomcrown.reserve_cap` | 0.5 | reserve per unit wood | [0, inf) | 0.1–2 log | B | placeholder | Q_max = q_cap * W |
| `flora.bloomcrown.maintenance` | 0.0002 | /s per unit wood | [0, inf) | yes | B | placeholder | wood upkeep m_w |
| `flora.bloomcrown.senescence` | 0.001 | /s of foliage | [0, inf) | yes | B | placeholder | leaf fall m_p, to litter |
| `flora.bloomcrown.foliage_rate` | 0.002 | organic/s per unit wood | [0, inf) | yes | D | placeholder | foliage regrowth cap r_p |
| `flora.bloomcrown.wood_rate` | 0.001 | /s per unit wood | [0, inf) | yes | C | placeholder | wood growth cap r_w |
| `flora.bloomcrown.build` | 0.2 | fraction | [0, inf) | 0.1–0.5 log | A | placeholder | construction respiration c_g: a unit built costs 1 + c_g |
| `flora.bloomcrown.dieback` | 1.0 | wood per unit unpaid upkeep | [0, inf) | 0.3–3 log | C | placeholder | kappa: wood lost per unit maintenance not paid |
| `flora.bloomcrown.alive_min` | 0.02 | org-units | [0, inf) | yes | C | placeholder | W_min: a stand with less wood dies |
| `flora.bloomcrown.donor_min` | 0.3 | org-units of wood | [0, inf) |  | C | placeholder | wood needed before sending propagules |
| `flora.bloomcrown.donor_reserve_floor` | 0.5 | fraction of Q_max | [0, inf) | 0–0.9 | C | placeholder | reserve a donor keeps back |
| `flora.bloomcrown.propagule_rate` | 0.0004 | organic/s gross | [0, inf) | yes | D | placeholder | reserve set aside for seed per second |
| `flora.bloomcrown.propagule_split.*` | 0.4, 0.4, 0.2 | fraction | [0, inf) | 0.1–0.8 | C | placeholder | how a landed package splits into wood, foliage and reserve (sums to 1) |
| `flora.bloomcrown.energy_density` | 2.0 | energy per org-unit | [0, inf) |  | D | placeholder | e_v of plant tissue |
| `flora.bloomcrown.seed_attrition_per_s` | 0.00003472222222222222 | /s | [0, inf) |  | C | derived | a banked seed's death rate |
| `flora.bloomcrown.seed_max_age_s` | 28800.0 | s | [0, inf) | yes | B | placeholder | dormancy: a bin older than this falls to litter |
| `flora.bloomcrown.seed_cohorts_max` | 4 | bins | [1, inf) |  | N | numerics | the bank's age resolution |
| `flora.bloomcrown.n_tissue` | 0.02 | mineral per org-unit | [0, inf) | 0.005–0.05 log | B | placeholder | stoichiometry of built tissue |
| `flora.bloomcrown.reserve_share` | 0.2 | fraction | [0, inf) | 0–0.6 | C | placeholder | share of a tick's surplus to the reserve first |
| `flora.bloomcrown.reflush_below` | 0.25 | fraction of P_cap | [0, inf) | 0–1 | C | placeholder | reserve funds foliage only below this |
| `flora.bloomcrown.assimilation` | 0.004 | organic/s per unit foliage | [0, inf) | yes | B | placeholder | g: gross fixation at full light and moisture |
| `flora.bloomcrown.nutrient_half` | 0.5 | mineral units | [0, inf) | 0.1–2 log | C | placeholder | K_N: mineral half-saturation |
| `flora.bloomcrown.nutrient_draw_max` | 0.01 | /s per unit site mineral | [0, inf) | 0.003–0.1 log | D | placeholder | f_max: rate cap on income from the site's mineral |
| `flora.bloomcrown.light_half` | 0.8 | fraction of open sky | [0, inf) | 0.05–1.5 | B | placeholder | light response half-saturation |
| `flora.bloomcrown.rooting_depth` | 2 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | root box depth below the support |
| `flora.bloomcrown.rooting_radius` | 1 | reference voxels (0.25 m) | [0, inf) |  | C | placeholder | root box half-width |
| `flora.bloomcrown.wilt_water` | 0.05 | available water (0 wp, 1 fc) | (-inf, inf) | yes | B | measured | mu = 0 at and below |
| `flora.bloomcrown.full_water` | 0.5 | available water | (-inf, inf) | yes | B | measured | mu = 1 at and above |
| `flora.bloomcrown.transpiration_m3_per_s` | 0.000001 | m3/s per unit foliage at mu = 1 | [0, inf) |  | D | measured | water drawn from the root box |
| `flora.bloomcrown.establish_water_min` | 0.1 | available water | (-inf, inf) | yes | B | placeholder | germination moisture gate |
| `flora.bloomcrown.establish_light_min` | 0.6 | sky visibility | (-inf, inf) | yes | C | placeholder | germination light gate |
| `flora.bloomcrown.saturated_pore` | 0.95 | pore fraction of capacity | (-inf, inf) | 0.8–1 | B | placeholder | a root voxel at or above this is saturated |
| `flora.bloomcrown.stress_rate_per_s` | 0.2 | /s fraction of gap | [0, inf) | 0.001–0.2 log | D | placeholder | aeration stress rise |
| `flora.bloomcrown.relax_rate_per_s` | 0.02 | /s fraction of gap | [0, inf) | yes | B | placeholder | aeration stress recovery |
| `flora.bloomcrown.establish_saturated_max` | 0.25 | fraction of root box saturated | (-inf, inf) | yes | C | placeholder | germination ceiling and adult stress onset |
| `flora.bloomcrown.drown_depth_m` | 0.05 | m of water | [0, inf) | yes | D | tuned | standing water over the support that kills the stand |
| `flora.bloomcrown.water_depth_min_m` | 0.0 | m of settled standing water | [0, inf) | yes | B | placeholder | needed on or beside the site |
| `flora.bloomcrown.hop` | 2 | reference voxels (0.25 m) | [0, inf) | yes | D | placeholder | dispersal radius / wind kernel scale |
| `flora.bloomcrown.clonal_share` | 0.0 | fraction of packages | [0, 1] | 0–1 | C | tuned | runner/rhizome share |
| `flora.bloomcrown.spore_tries` | 8 | tries | [1, inf) | yes | C | tuned | a spore package's landing attempts |
| `flora.bloomcrown.seed_mass` | 2 | minimum packages | [1, inf) | yes | D | placeholder | seed size and lottery weight |
| `flora.bloomcrown.overtop_per_check` | 0.1 | probability per check | [0, 1] | yes | C | tuned | a woody seedling comes up through a ground cover |
| `flora.bloomcrown.crown_height_m.*` | 0.375, 1.0 | m | [0, inf) | yes | B | placeholder | crown top above the support [min, max] |
| `flora.bloomcrown.crown_radius_m.*` | 0.125, 0.3125 | m | [0, inf) | yes | B | placeholder | crown half-width [min, max] |
| `flora.bloomcrown.graze_refuge` | 0.15 | fraction of each layer's capacity | [0, 1) | 0–0.6 | B | placeholder | foliage no bite can take |
| `flora.bloomcrown.defence_per_bite` | 0.003 | fraction toward 1 per bite | [0, 1] | yes | D | measured | induced defence rise |
| `flora.bloomcrown.defence_half_life_s` | 1200.0 | s (<= 0: no decay) | (-inf, inf) | yes | D | placeholder | induced defence half-life once grazing stops |
| `flora.bloomcrown.profile.*.wood_fraction_max` | 0.2, 0.5, inf | fraction of wood_max (inf: the last stage) | [0, inf] | yes | C | placeholder | a stage applies up to this wood fraction |
| `flora.bloomcrown.profile.*.height_m_max` | 0.125 | m | (0, inf) | yes | B | placeholder | a stage's crown-height ceiling (a capped seedling) |
| `flora.bloomcrown.profile.*.layers.*.band.*` | 0.0, 1.0, 0.0, 0.4 … (14 values) | fraction of crown height | [0, 1] | yes | C | placeholder | a layer's band [bottom, top] |
| `flora.bloomcrown.profile.*.layers.*.radius` | 1.0, 0.15, 0.7, 1.0 … (7 values) | fraction of crown radius | (0, 1] |  | C | placeholder | a layer's radius |
| `flora.bloomcrown.profile.*.layers.*.share` | 1.0, 0.0, 0.4, 0.6 … (7 values) | fraction of the stage's foliage | [0, inf) | yes | C | placeholder | a foliage layer's share (a trunk's is 0) |
| `flora.bloomcrown.profile.*.layers.*.porosity` | 0.4, 0.0, 0.4, 0.3 … (7 values) | fraction transmitted | [0, 1] | yes | C | placeholder | light a layer lets through |
| `flora.umbrellafrond.substrate_uptake_per_s` | 0.0 | organic/s per unit mycelium | [0, inf) | yes | C | placeholder | saprotroph: dead wood and litter withdrawn |
| `flora.umbrellafrond.substrate_yield` | 0.0 | fraction | [0, 1] | 0.2–0.7 | A | placeholder | saprotroph: share of the substrate taken that becomes tissue |
| `flora.umbrellafrond.establish_substrate_min` | 0.0 | org-units | [0, inf) |  | C | placeholder | saprotroph: substrate a spore cohort needs in its box |
| `flora.umbrellafrond.substrate_reach_up_down` | 1 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | saprotroph: the mycelium box's vertical reach |
| `flora.umbrellafrond.alpha` | 2.0 | foliage per unit wood | [0, inf) | 0.05–3 log | D | placeholder | foliage cap P_cap = alpha * W |
| `flora.umbrellafrond.wood_max` | 0.6 | org-units | (0, inf) | yes | D | placeholder | the most living wood one stand holds |
| `flora.umbrellafrond.reserve_cap` | 0.5 | reserve per unit wood | [0, inf) | 0.1–2 log | B | placeholder | Q_max = q_cap * W |
| `flora.umbrellafrond.maintenance` | 0.0002 | /s per unit wood | [0, inf) | yes | B | placeholder | wood upkeep m_w |
| `flora.umbrellafrond.senescence` | 0.001 | /s of foliage | [0, inf) | yes | B | placeholder | leaf fall m_p, to litter |
| `flora.umbrellafrond.foliage_rate` | 0.002 | organic/s per unit wood | [0, inf) | yes | D | placeholder | foliage regrowth cap r_p |
| `flora.umbrellafrond.wood_rate` | 0.001 | /s per unit wood | [0, inf) | yes | C | placeholder | wood growth cap r_w |
| `flora.umbrellafrond.build` | 0.2 | fraction | [0, inf) | 0.1–0.5 log | A | placeholder | construction respiration c_g: a unit built costs 1 + c_g |
| `flora.umbrellafrond.dieback` | 1.0 | wood per unit unpaid upkeep | [0, inf) | 0.3–3 log | C | placeholder | kappa: wood lost per unit maintenance not paid |
| `flora.umbrellafrond.alive_min` | 0.02 | org-units | [0, inf) | yes | C | placeholder | W_min: a stand with less wood dies |
| `flora.umbrellafrond.donor_min` | 0.3 | org-units of wood | [0, inf) |  | C | placeholder | wood needed before sending propagules |
| `flora.umbrellafrond.donor_reserve_floor` | 0.5 | fraction of Q_max | [0, inf) | 0–0.9 | C | placeholder | reserve a donor keeps back |
| `flora.umbrellafrond.propagule_rate` | 0.0002 | organic/s gross | [0, inf) | yes | D | placeholder | reserve set aside for seed per second |
| `flora.umbrellafrond.propagule_split.*` | 0.4, 0.4, 0.2 | fraction | [0, inf) | 0.1–0.8 | C | placeholder | how a landed package splits into wood, foliage and reserve (sums to 1) |
| `flora.umbrellafrond.energy_density` | 2.0 | energy per org-unit | [0, inf) |  | D | placeholder | e_v of plant tissue |
| `flora.umbrellafrond.seed_attrition_per_s` | 0.00006944444444444444 | /s | [0, inf) |  | C | derived | a banked seed's death rate |
| `flora.umbrellafrond.seed_max_age_s` | 14400.0 | s | [0, inf) | yes | B | placeholder | dormancy: a bin older than this falls to litter |
| `flora.umbrellafrond.seed_cohorts_max` | 4 | bins | [1, inf) |  | N | numerics | the bank's age resolution |
| `flora.umbrellafrond.n_tissue` | 0.02 | mineral per org-unit | [0, inf) | 0.005–0.05 log | B | placeholder | stoichiometry of built tissue |
| `flora.umbrellafrond.reserve_share` | 0.2 | fraction | [0, inf) | 0–0.6 | C | placeholder | share of a tick's surplus to the reserve first |
| `flora.umbrellafrond.reflush_below` | 0.25 | fraction of P_cap | [0, inf) | 0–1 | C | placeholder | reserve funds foliage only below this |
| `flora.umbrellafrond.assimilation` | 0.004 | organic/s per unit foliage | [0, inf) | yes | B | placeholder | g: gross fixation at full light and moisture |
| `flora.umbrellafrond.nutrient_half` | 0.5 | mineral units | [0, inf) | 0.1–2 log | C | placeholder | K_N: mineral half-saturation |
| `flora.umbrellafrond.nutrient_draw_max` | 0.01 | /s per unit site mineral | [0, inf) | 0.003–0.1 log | D | placeholder | f_max: rate cap on income from the site's mineral |
| `flora.umbrellafrond.light_half` | 0.15 | fraction of open sky | [0, inf) | 0.05–1.5 | B | placeholder | light response half-saturation |
| `flora.umbrellafrond.rooting_depth` | 4 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | root box depth below the support |
| `flora.umbrellafrond.rooting_radius` | 1 | reference voxels (0.25 m) | [0, inf) |  | C | placeholder | root box half-width |
| `flora.umbrellafrond.wilt_water` | 1.0 | available water (0 wp, 1 fc) | (-inf, inf) | yes | B | measured | mu = 0 at and below |
| `flora.umbrellafrond.full_water` | 1.5 | available water | (-inf, inf) | yes | B | measured | mu = 1 at and above |
| `flora.umbrellafrond.transpiration_m3_per_s` | 0.000001 | m3/s per unit foliage at mu = 1 | [0, inf) |  | D | measured | water drawn from the root box |
| `flora.umbrellafrond.establish_water_min` | 1.2 | available water | (-inf, inf) | yes | B | placeholder | germination moisture gate |
| `flora.umbrellafrond.establish_light_min` | 0.1 | sky visibility | (-inf, inf) | yes | C | placeholder | germination light gate |
| `flora.umbrellafrond.saturated_pore` | 0.95 | pore fraction of capacity | (-inf, inf) | 0.8–1 | B | placeholder | a root voxel at or above this is saturated |
| `flora.umbrellafrond.stress_rate_per_s` | 0.01 | /s fraction of gap | [0, inf) | 0.001–0.2 log | D | placeholder | aeration stress rise |
| `flora.umbrellafrond.relax_rate_per_s` | 0.05 | /s fraction of gap | [0, inf) | yes | B | placeholder | aeration stress recovery |
| `flora.umbrellafrond.establish_saturated_max` | 1.0 | fraction of root box saturated | (-inf, inf) | yes | C | placeholder | germination ceiling and adult stress onset |
| `flora.umbrellafrond.drown_depth_m` | 0.5 | m of water | [0, inf) | yes | D | tuned | standing water over the support that kills the stand |
| `flora.umbrellafrond.water_depth_min_m` | 0.0 | m of settled standing water | [0, inf) | yes | B | placeholder | needed on or beside the site |
| `flora.umbrellafrond.hop` | 6 | reference voxels (0.25 m) | [0, inf) | yes | D | placeholder | dispersal radius / wind kernel scale |
| `flora.umbrellafrond.clonal_share` | 0.5 | fraction of packages | [0, 1] | 0–1 | C | tuned | runner/rhizome share |
| `flora.umbrellafrond.spore_tries` | 8 | tries | [1, inf) | yes | C | tuned | a spore package's landing attempts |
| `flora.umbrellafrond.seed_mass` | 1 | minimum packages | [1, inf) | yes | D | placeholder | seed size and lottery weight |
| `flora.umbrellafrond.overtop_per_check` | 0.0 | probability per check | [0, 1] | yes | C | tuned | a woody seedling comes up through a ground cover |
| `flora.umbrellafrond.crown_height_m.*` | 1.0, 2.0 | m | [0, inf) | yes | B | placeholder | crown top above the support [min, max] |
| `flora.umbrellafrond.crown_radius_m.*` | 0.375, 0.75 | m | [0, inf) | yes | B | placeholder | crown half-width [min, max] |
| `flora.umbrellafrond.graze_refuge` | 0.15 | fraction of each layer's capacity | [0, 1) | 0–0.6 | B | placeholder | foliage no bite can take |
| `flora.umbrellafrond.defence_per_bite` | 0.003 | fraction toward 1 per bite | [0, 1] | yes | D | measured | induced defence rise |
| `flora.umbrellafrond.defence_half_life_s` | 1200.0 | s (<= 0: no decay) | (-inf, inf) | yes | D | placeholder | induced defence half-life once grazing stops |
| `flora.umbrellafrond.profile.*.wood_fraction_max` | 0.15, 0.5, inf | fraction of wood_max (inf: the last stage) | [0, inf] | yes | C | placeholder | a stage applies up to this wood fraction |
| `flora.umbrellafrond.profile.*.height_m_max` | 0.125 | m | (0, inf) | yes | B | placeholder | a stage's crown-height ceiling (a capped seedling) |
| `flora.umbrellafrond.profile.*.layers.*.band.*` | 0.0, 1.0, 0.0, 0.5 … (16 values) | fraction of crown height | [0, 1] | yes | C | placeholder | a layer's band [bottom, top] |
| `flora.umbrellafrond.profile.*.layers.*.radius` | 1.0, 0.15, 1.0, 0.6 … (8 values) | fraction of crown radius | (0, 1] |  | C | placeholder | a layer's radius |
| `flora.umbrellafrond.profile.*.layers.*.share` | 1.0, 0.0, 0.6, 0.4 … (8 values) | fraction of the stage's foliage | [0, inf) | yes | C | placeholder | a foliage layer's share (a trunk's is 0) |
| `flora.umbrellafrond.profile.*.layers.*.porosity` | 0.4, 0.0, 0.4, 0.5 … (8 values) | fraction transmitted | [0, 1] | yes | C | placeholder | light a layer lets through |
| `flora.springturf.substrate_uptake_per_s` | 0.0 | organic/s per unit mycelium | [0, inf) | yes | C | placeholder | saprotroph: dead wood and litter withdrawn |
| `flora.springturf.substrate_yield` | 0.0 | fraction | [0, 1] | 0.2–0.7 | A | placeholder | saprotroph: share of the substrate taken that becomes tissue |
| `flora.springturf.establish_substrate_min` | 0.0 | org-units | [0, inf) |  | C | placeholder | saprotroph: substrate a spore cohort needs in its box |
| `flora.springturf.substrate_reach_up_down` | 1 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | saprotroph: the mycelium box's vertical reach |
| `flora.springturf.alpha` | 2.0 | foliage per unit wood | [0, inf) | 0.05–3 log | D | placeholder | foliage cap P_cap = alpha * W |
| `flora.springturf.wood_max` | 0.06 | org-units | (0, inf) | yes | D | placeholder | the most living wood one stand holds |
| `flora.springturf.reserve_cap` | 0.5 | reserve per unit wood | [0, inf) | 0.1–2 log | B | placeholder | Q_max = q_cap * W |
| `flora.springturf.maintenance` | 0.002 | /s per unit wood | [0, inf) | yes | B | placeholder | wood upkeep m_w |
| `flora.springturf.senescence` | 0.001 | /s of foliage | [0, inf) | yes | B | placeholder | leaf fall m_p, to litter |
| `flora.springturf.foliage_rate` | 0.02 | organic/s per unit wood | [0, inf) | yes | D | placeholder | foliage regrowth cap r_p |
| `flora.springturf.wood_rate` | 0.01 | /s per unit wood | [0, inf) | yes | C | placeholder | wood growth cap r_w |
| `flora.springturf.build` | 0.2 | fraction | [0, inf) | 0.1–0.5 log | A | placeholder | construction respiration c_g: a unit built costs 1 + c_g |
| `flora.springturf.dieback` | 1.0 | wood per unit unpaid upkeep | [0, inf) | 0.3–3 log | C | placeholder | kappa: wood lost per unit maintenance not paid |
| `flora.springturf.alive_min` | 0.006 | org-units | [0, inf) | yes | C | placeholder | W_min: a stand with less wood dies |
| `flora.springturf.donor_min` | 0.03 | org-units of wood | [0, inf) |  | C | placeholder | wood needed before sending propagules |
| `flora.springturf.donor_reserve_floor` | 0.5 | fraction of Q_max | [0, inf) | 0–0.9 | C | placeholder | reserve a donor keeps back |
| `flora.springturf.propagule_rate` | 0.002 | organic/s gross | [0, inf) | yes | D | placeholder | reserve set aside for seed per second |
| `flora.springturf.propagule_split.*` | 0.4, 0.4, 0.2 | fraction | [0, inf) | 0.1–0.8 | C | placeholder | how a landed package splits into wood, foliage and reserve (sums to 1) |
| `flora.springturf.energy_density` | 2.0 | energy per org-unit | [0, inf) |  | D | placeholder | e_v of plant tissue |
| `flora.springturf.seed_attrition_per_s` | 0.000023148148148148147 | /s | [0, inf) |  | C | derived | a banked seed's death rate |
| `flora.springturf.seed_max_age_s` | 43200.0 | s | [0, inf) | yes | B | placeholder | dormancy: a bin older than this falls to litter |
| `flora.springturf.seed_cohorts_max` | 4 | bins | [1, inf) |  | N | numerics | the bank's age resolution |
| `flora.springturf.n_tissue` | 0.02 | mineral per org-unit | [0, inf) | 0.005–0.05 log | B | placeholder | stoichiometry of built tissue |
| `flora.springturf.reserve_share` | 0.2 | fraction | [0, inf) | 0–0.6 | C | placeholder | share of a tick's surplus to the reserve first |
| `flora.springturf.reflush_below` | 0.25 | fraction of P_cap | [0, inf) | 0–1 | C | placeholder | reserve funds foliage only below this |
| `flora.springturf.assimilation` | 0.008 | organic/s per unit foliage | [0, inf) | yes | B | placeholder | g: gross fixation at full light and moisture |
| `flora.springturf.nutrient_half` | 0.5 | mineral units | [0, inf) | 0.1–2 log | C | placeholder | K_N: mineral half-saturation |
| `flora.springturf.nutrient_draw_max` | 0.01 | /s per unit site mineral | [0, inf) | 0.003–0.1 log | D | placeholder | f_max: rate cap on income from the site's mineral |
| `flora.springturf.light_half` | 1.0 | fraction of open sky | [0, inf) | 0.05–1.5 | B | placeholder | light response half-saturation |
| `flora.springturf.rooting_depth` | 1 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | root box depth below the support |
| `flora.springturf.rooting_radius` | 1 | reference voxels (0.25 m) | [0, inf) |  | C | placeholder | root box half-width |
| `flora.springturf.wilt_water` | 0.1 | available water (0 wp, 1 fc) | (-inf, inf) | yes | B | measured | mu = 0 at and below |
| `flora.springturf.full_water` | 0.5 | available water | (-inf, inf) | yes | B | measured | mu = 1 at and above |
| `flora.springturf.transpiration_m3_per_s` | 0.000001 | m3/s per unit foliage at mu = 1 | [0, inf) |  | D | measured | water drawn from the root box |
| `flora.springturf.establish_water_min` | 0.3 | available water | (-inf, inf) | yes | B | placeholder | germination moisture gate |
| `flora.springturf.establish_light_min` | 0.75 | sky visibility | (-inf, inf) | yes | C | placeholder | germination light gate |
| `flora.springturf.saturated_pore` | 0.95 | pore fraction of capacity | (-inf, inf) | 0.8–1 | B | placeholder | a root voxel at or above this is saturated |
| `flora.springturf.stress_rate_per_s` | 0.2 | /s fraction of gap | [0, inf) | 0.001–0.2 log | D | placeholder | aeration stress rise |
| `flora.springturf.relax_rate_per_s` | 0.05 | /s fraction of gap | [0, inf) | yes | B | placeholder | aeration stress recovery |
| `flora.springturf.establish_saturated_max` | 0.3 | fraction of root box saturated | (-inf, inf) | yes | C | placeholder | germination ceiling and adult stress onset |
| `flora.springturf.drown_depth_m` | 0.03 | m of water | [0, inf) | yes | D | tuned | standing water over the support that kills the stand |
| `flora.springturf.water_depth_min_m` | 0.0 | m of settled standing water | [0, inf) | yes | B | placeholder | needed on or beside the site |
| `flora.springturf.hop` | 3 | reference voxels (0.25 m) | [0, inf) | yes | D | placeholder | dispersal radius / wind kernel scale |
| `flora.springturf.clonal_share` | 0.3 | fraction of packages | [0, 1] | 0–1 | C | tuned | runner/rhizome share |
| `flora.springturf.spore_tries` | 8 | tries | [1, inf) | yes | C | tuned | a spore package's landing attempts |
| `flora.springturf.seed_mass` | 1 | minimum packages | [1, inf) | yes | D | placeholder | seed size and lottery weight |
| `flora.springturf.overtop_per_check` | 0.0 | probability per check | [0, 1] | yes | C | tuned | a woody seedling comes up through a ground cover |
| `flora.springturf.crown_height_m.*` | 0.125, 0.1875 | m | [0, inf) | yes | B | placeholder | crown top above the support [min, max] |
| `flora.springturf.crown_radius_m.*` | 0.125, 0.25 | m | [0, inf) | yes | B | placeholder | crown half-width [min, max] |
| `flora.springturf.graze_refuge` | 0.3 | fraction of each layer's capacity | [0, 1) | 0–0.6 | B | placeholder | foliage no bite can take |
| `flora.springturf.defence_per_bite` | 0.003 | fraction toward 1 per bite | [0, 1] | yes | D | measured | induced defence rise |
| `flora.springturf.defence_half_life_s` | 1200.0 | s (<= 0: no decay) | (-inf, inf) | yes | D | placeholder | induced defence half-life once grazing stops |
| `flora.springturf.profile.*.wood_fraction_max` | inf | fraction of wood_max (inf: the last stage) | [0, inf] | yes | C | placeholder | a stage applies up to this wood fraction |
| `flora.springturf.profile.*.height_m_max` | (absent) | m | (0, inf) | yes | B | placeholder | a stage's crown-height ceiling (a capped seedling) |
| `flora.springturf.profile.*.layers.*.band.*` | 0.0, 1.0 | fraction of crown height | [0, 1] | yes | C | placeholder | a layer's band [bottom, top] |
| `flora.springturf.profile.*.layers.*.radius` | 1.0 | fraction of crown radius | (0, 1] |  | C | placeholder | a layer's radius |
| `flora.springturf.profile.*.layers.*.share` | 1.0 | fraction of the stage's foliage | [0, inf) | yes | C | placeholder | a foliage layer's share (a trunk's is 0) |
| `flora.springturf.profile.*.layers.*.porosity` | 0.3 | fraction transmitted | [0, 1] | yes | C | placeholder | light a layer lets through |
| `flora.stonecushion.substrate_uptake_per_s` | 0.0 | organic/s per unit mycelium | [0, inf) | yes | C | placeholder | saprotroph: dead wood and litter withdrawn |
| `flora.stonecushion.substrate_yield` | 0.0 | fraction | [0, 1] | 0.2–0.7 | A | placeholder | saprotroph: share of the substrate taken that becomes tissue |
| `flora.stonecushion.establish_substrate_min` | 0.0 | org-units | [0, inf) |  | C | placeholder | saprotroph: substrate a spore cohort needs in its box |
| `flora.stonecushion.substrate_reach_up_down` | 1 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | saprotroph: the mycelium box's vertical reach |
| `flora.stonecushion.alpha` | 2.0 | foliage per unit wood | [0, inf) | 0.05–3 log | D | placeholder | foliage cap P_cap = alpha * W |
| `flora.stonecushion.wood_max` | 0.1 | org-units | (0, inf) | yes | D | placeholder | the most living wood one stand holds |
| `flora.stonecushion.reserve_cap` | 0.5 | reserve per unit wood | [0, inf) | 0.1–2 log | B | placeholder | Q_max = q_cap * W |
| `flora.stonecushion.maintenance` | 0.00005 | /s per unit wood | [0, inf) | yes | B | placeholder | wood upkeep m_w |
| `flora.stonecushion.senescence` | 0.0003 | /s of foliage | [0, inf) | yes | B | placeholder | leaf fall m_p, to litter |
| `flora.stonecushion.foliage_rate` | 0.0005 | organic/s per unit wood | [0, inf) | yes | D | placeholder | foliage regrowth cap r_p |
| `flora.stonecushion.wood_rate` | 0.0002 | /s per unit wood | [0, inf) | yes | C | placeholder | wood growth cap r_w |
| `flora.stonecushion.build` | 0.2 | fraction | [0, inf) | 0.1–0.5 log | A | placeholder | construction respiration c_g: a unit built costs 1 + c_g |
| `flora.stonecushion.dieback` | 1.0 | wood per unit unpaid upkeep | [0, inf) | 0.3–3 log | C | placeholder | kappa: wood lost per unit maintenance not paid |
| `flora.stonecushion.alive_min` | 0.01 | org-units | [0, inf) | yes | C | placeholder | W_min: a stand with less wood dies |
| `flora.stonecushion.donor_min` | 0.05 | org-units of wood | [0, inf) |  | C | placeholder | wood needed before sending propagules |
| `flora.stonecushion.donor_reserve_floor` | 0.5 | fraction of Q_max | [0, inf) | 0–0.9 | C | placeholder | reserve a donor keeps back |
| `flora.stonecushion.propagule_rate` | 0.00005 | organic/s gross | [0, inf) | yes | D | placeholder | reserve set aside for seed per second |
| `flora.stonecushion.propagule_split.*` | 0.4, 0.4, 0.2 | fraction | [0, inf) | 0.1–0.8 | C | placeholder | how a landed package splits into wood, foliage and reserve (sums to 1) |
| `flora.stonecushion.energy_density` | 2.0 | energy per org-unit | [0, inf) |  | D | placeholder | e_v of plant tissue |
| `flora.stonecushion.seed_attrition_per_s` | 0.000023148148148148147 | /s | [0, inf) |  | C | derived | a banked seed's death rate |
| `flora.stonecushion.seed_max_age_s` | 43200.0 | s | [0, inf) | yes | B | placeholder | dormancy: a bin older than this falls to litter |
| `flora.stonecushion.seed_cohorts_max` | 4 | bins | [1, inf) |  | N | numerics | the bank's age resolution |
| `flora.stonecushion.n_tissue` | 0.02 | mineral per org-unit | [0, inf) | 0.005–0.05 log | B | placeholder | stoichiometry of built tissue |
| `flora.stonecushion.reserve_share` | 0.2 | fraction | [0, inf) | 0–0.6 | C | placeholder | share of a tick's surplus to the reserve first |
| `flora.stonecushion.reflush_below` | 0.25 | fraction of P_cap | [0, inf) | 0–1 | C | placeholder | reserve funds foliage only below this |
| `flora.stonecushion.assimilation` | 0.004 | organic/s per unit foliage | [0, inf) | yes | B | placeholder | g: gross fixation at full light and moisture |
| `flora.stonecushion.nutrient_half` | 0.5 | mineral units | [0, inf) | 0.1–2 log | C | placeholder | K_N: mineral half-saturation |
| `flora.stonecushion.nutrient_draw_max` | 0.01 | /s per unit site mineral | [0, inf) | 0.003–0.1 log | D | placeholder | f_max: rate cap on income from the site's mineral |
| `flora.stonecushion.light_half` | 0.3 | fraction of open sky | [0, inf) | 0.05–1.5 | B | placeholder | light response half-saturation |
| `flora.stonecushion.rooting_depth` | 2 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | root box depth below the support |
| `flora.stonecushion.rooting_radius` | 1 | reference voxels (0.25 m) | [0, inf) |  | C | placeholder | root box half-width |
| `flora.stonecushion.wilt_water` | 0.0 | available water (0 wp, 1 fc) | (-inf, inf) | yes | B | measured | mu = 0 at and below |
| `flora.stonecushion.full_water` | 0.2 | available water | (-inf, inf) | yes | B | measured | mu = 1 at and above |
| `flora.stonecushion.transpiration_m3_per_s` | 0.000001 | m3/s per unit foliage at mu = 1 | [0, inf) |  | D | measured | water drawn from the root box |
| `flora.stonecushion.establish_water_min` | 0.05 | available water | (-inf, inf) | yes | B | placeholder | germination moisture gate |
| `flora.stonecushion.establish_light_min` | 0.4 | sky visibility | (-inf, inf) | yes | C | placeholder | germination light gate |
| `flora.stonecushion.saturated_pore` | 0.95 | pore fraction of capacity | (-inf, inf) | 0.8–1 | B | placeholder | a root voxel at or above this is saturated |
| `flora.stonecushion.stress_rate_per_s` | 0.1 | /s fraction of gap | [0, inf) | 0.001–0.2 log | D | placeholder | aeration stress rise |
| `flora.stonecushion.relax_rate_per_s` | 0.02 | /s fraction of gap | [0, inf) | yes | B | placeholder | aeration stress recovery |
| `flora.stonecushion.establish_saturated_max` | 0.4 | fraction of root box saturated | (-inf, inf) | yes | C | placeholder | germination ceiling and adult stress onset |
| `flora.stonecushion.drown_depth_m` | 0.02 | m of water | [0, inf) | yes | D | tuned | standing water over the support that kills the stand |
| `flora.stonecushion.water_depth_min_m` | 0.0 | m of settled standing water | [0, inf) | yes | B | placeholder | needed on or beside the site |
| `flora.stonecushion.hop` | 1 | reference voxels (0.25 m) | [0, inf) | yes | D | placeholder | dispersal radius / wind kernel scale |
| `flora.stonecushion.clonal_share` | 0.0 | fraction of packages | [0, 1] | 0–1 | C | tuned | runner/rhizome share |
| `flora.stonecushion.spore_tries` | 8 | tries | [1, inf) | yes | C | tuned | a spore package's landing attempts |
| `flora.stonecushion.seed_mass` | 1 | minimum packages | [1, inf) | yes | D | placeholder | seed size and lottery weight |
| `flora.stonecushion.overtop_per_check` | 0.0 | probability per check | [0, 1] | yes | C | tuned | a woody seedling comes up through a ground cover |
| `flora.stonecushion.crown_height_m.*` | 0.125, 0.1875 | m | [0, inf) | yes | B | placeholder | crown top above the support [min, max] |
| `flora.stonecushion.crown_radius_m.*` | 0.125, 0.15625 | m | [0, inf) | yes | B | placeholder | crown half-width [min, max] |
| `flora.stonecushion.graze_refuge` | 0.4 | fraction of each layer's capacity | [0, 1) | 0–0.6 | B | placeholder | foliage no bite can take |
| `flora.stonecushion.defence_per_bite` | 0.003 | fraction toward 1 per bite | [0, 1] | yes | D | measured | induced defence rise |
| `flora.stonecushion.defence_half_life_s` | 1200.0 | s (<= 0: no decay) | (-inf, inf) | yes | D | placeholder | induced defence half-life once grazing stops |
| `flora.stonecushion.profile.*.wood_fraction_max` | inf | fraction of wood_max (inf: the last stage) | [0, inf] | yes | C | placeholder | a stage applies up to this wood fraction |
| `flora.stonecushion.profile.*.height_m_max` | (absent) | m | (0, inf) | yes | B | placeholder | a stage's crown-height ceiling (a capped seedling) |
| `flora.stonecushion.profile.*.layers.*.band.*` | 0.0, 0.6, 0.6, 1.0 | fraction of crown height | [0, 1] | yes | C | placeholder | a layer's band [bottom, top] |
| `flora.stonecushion.profile.*.layers.*.radius` | 1.0, 0.6 | fraction of crown radius | (0, 1] |  | C | placeholder | a layer's radius |
| `flora.stonecushion.profile.*.layers.*.share` | 0.7, 0.3 | fraction of the stage's foliage | [0, inf) | yes | C | placeholder | a foliage layer's share (a trunk's is 0) |
| `flora.stonecushion.profile.*.layers.*.porosity` | 0.2, 0.2 | fraction transmitted | [0, 1] | yes | C | placeholder | light a layer lets through |
| `flora.velvetpad.substrate_uptake_per_s` | 0.0 | organic/s per unit mycelium | [0, inf) | yes | C | placeholder | saprotroph: dead wood and litter withdrawn |
| `flora.velvetpad.substrate_yield` | 0.0 | fraction | [0, 1] | 0.2–0.7 | A | placeholder | saprotroph: share of the substrate taken that becomes tissue |
| `flora.velvetpad.establish_substrate_min` | 0.0 | org-units | [0, inf) |  | C | placeholder | saprotroph: substrate a spore cohort needs in its box |
| `flora.velvetpad.substrate_reach_up_down` | 1 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | saprotroph: the mycelium box's vertical reach |
| `flora.velvetpad.alpha` | 2.0 | foliage per unit wood | [0, inf) | 0.05–3 log | D | placeholder | foliage cap P_cap = alpha * W |
| `flora.velvetpad.wood_max` | 0.2 | org-units | (0, inf) | yes | D | placeholder | the most living wood one stand holds |
| `flora.velvetpad.reserve_cap` | 0.5 | reserve per unit wood | [0, inf) | 0.1–2 log | B | placeholder | Q_max = q_cap * W |
| `flora.velvetpad.maintenance` | 0.0002 | /s per unit wood | [0, inf) | yes | B | placeholder | wood upkeep m_w |
| `flora.velvetpad.senescence` | 0.0005 | /s of foliage | [0, inf) | yes | B | placeholder | leaf fall m_p, to litter |
| `flora.velvetpad.foliage_rate` | 0.004 | organic/s per unit wood | [0, inf) | yes | D | placeholder | foliage regrowth cap r_p |
| `flora.velvetpad.wood_rate` | 0.002 | /s per unit wood | [0, inf) | yes | C | placeholder | wood growth cap r_w |
| `flora.velvetpad.build` | 0.2 | fraction | [0, inf) | 0.1–0.5 log | A | placeholder | construction respiration c_g: a unit built costs 1 + c_g |
| `flora.velvetpad.dieback` | 1.0 | wood per unit unpaid upkeep | [0, inf) | 0.3–3 log | C | placeholder | kappa: wood lost per unit maintenance not paid |
| `flora.velvetpad.alive_min` | 0.015 | org-units | [0, inf) | yes | C | placeholder | W_min: a stand with less wood dies |
| `flora.velvetpad.donor_min` | 0.1 | org-units of wood | [0, inf) |  | C | placeholder | wood needed before sending propagules |
| `flora.velvetpad.donor_reserve_floor` | 0.5 | fraction of Q_max | [0, inf) | 0–0.9 | C | placeholder | reserve a donor keeps back |
| `flora.velvetpad.propagule_rate` | 0.0005 | organic/s gross | [0, inf) | yes | D | placeholder | reserve set aside for seed per second |
| `flora.velvetpad.propagule_split.*` | 0.4, 0.4, 0.2 | fraction | [0, inf) | 0.1–0.8 | C | placeholder | how a landed package splits into wood, foliage and reserve (sums to 1) |
| `flora.velvetpad.energy_density` | 2.0 | energy per org-unit | [0, inf) |  | D | placeholder | e_v of plant tissue |
| `flora.velvetpad.seed_attrition_per_s` | 0.00009259259259259259 | /s | [0, inf) |  | C | derived | a banked seed's death rate |
| `flora.velvetpad.seed_max_age_s` | 10800.0 | s | [0, inf) | yes | B | placeholder | dormancy: a bin older than this falls to litter |
| `flora.velvetpad.seed_cohorts_max` | 4 | bins | [1, inf) |  | N | numerics | the bank's age resolution |
| `flora.velvetpad.n_tissue` | 0.02 | mineral per org-unit | [0, inf) | 0.005–0.05 log | B | placeholder | stoichiometry of built tissue |
| `flora.velvetpad.reserve_share` | 0.2 | fraction | [0, inf) | 0–0.6 | C | placeholder | share of a tick's surplus to the reserve first |
| `flora.velvetpad.reflush_below` | 0.25 | fraction of P_cap | [0, inf) | 0–1 | C | placeholder | reserve funds foliage only below this |
| `flora.velvetpad.assimilation` | 0.004 | organic/s per unit foliage | [0, inf) | yes | B | placeholder | g: gross fixation at full light and moisture |
| `flora.velvetpad.nutrient_half` | 0.5 | mineral units | [0, inf) | 0.1–2 log | C | placeholder | K_N: mineral half-saturation |
| `flora.velvetpad.nutrient_draw_max` | 0.01 | /s per unit site mineral | [0, inf) | 0.003–0.1 log | D | placeholder | f_max: rate cap on income from the site's mineral |
| `flora.velvetpad.light_half` | 0.1 | fraction of open sky | [0, inf) | 0.05–1.5 | B | placeholder | light response half-saturation |
| `flora.velvetpad.rooting_depth` | 2 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | root box depth below the support |
| `flora.velvetpad.rooting_radius` | 1 | reference voxels (0.25 m) | [0, inf) |  | C | placeholder | root box half-width |
| `flora.velvetpad.wilt_water` | 0.25 | available water (0 wp, 1 fc) | (-inf, inf) | yes | B | measured | mu = 0 at and below |
| `flora.velvetpad.full_water` | 0.9 | available water | (-inf, inf) | yes | B | measured | mu = 1 at and above |
| `flora.velvetpad.transpiration_m3_per_s` | 0.000001 | m3/s per unit foliage at mu = 1 | [0, inf) |  | D | measured | water drawn from the root box |
| `flora.velvetpad.establish_water_min` | 0.4 | available water | (-inf, inf) | yes | B | placeholder | germination moisture gate |
| `flora.velvetpad.establish_light_min` | 0.05 | sky visibility | (-inf, inf) | yes | C | placeholder | germination light gate |
| `flora.velvetpad.saturated_pore` | 0.95 | pore fraction of capacity | (-inf, inf) | 0.8–1 | B | placeholder | a root voxel at or above this is saturated |
| `flora.velvetpad.stress_rate_per_s` | 0.1 | /s fraction of gap | [0, inf) | 0.001–0.2 log | D | placeholder | aeration stress rise |
| `flora.velvetpad.relax_rate_per_s` | 0.05 | /s fraction of gap | [0, inf) | yes | B | placeholder | aeration stress recovery |
| `flora.velvetpad.establish_saturated_max` | 0.6 | fraction of root box saturated | (-inf, inf) | yes | C | placeholder | germination ceiling and adult stress onset |
| `flora.velvetpad.drown_depth_m` | 0.1 | m of water | [0, inf) | yes | D | tuned | standing water over the support that kills the stand |
| `flora.velvetpad.water_depth_min_m` | 0.0 | m of settled standing water | [0, inf) | yes | B | placeholder | needed on or beside the site |
| `flora.velvetpad.hop` | 6 | reference voxels (0.25 m) | [0, inf) | yes | D | placeholder | dispersal radius / wind kernel scale |
| `flora.velvetpad.clonal_share` | 0.3 | fraction of packages | [0, 1] | 0–1 | C | tuned | runner/rhizome share |
| `flora.velvetpad.spore_tries` | 2 | tries | [1, inf) | yes | C | tuned | a spore package's landing attempts |
| `flora.velvetpad.seed_mass` | 1 | minimum packages | [1, inf) | yes | D | placeholder | seed size and lottery weight |
| `flora.velvetpad.overtop_per_check` | 0.0 | probability per check | [0, 1] | yes | C | tuned | a woody seedling comes up through a ground cover |
| `flora.velvetpad.crown_height_m.*` | 0.125, 0.125 | m | [0, inf) | yes | B | placeholder | crown top above the support [min, max] |
| `flora.velvetpad.crown_radius_m.*` | 0.1875, 0.375 | m | [0, inf) | yes | B | placeholder | crown half-width [min, max] |
| `flora.velvetpad.graze_refuge` | 0.3 | fraction of each layer's capacity | [0, 1) | 0–0.6 | B | placeholder | foliage no bite can take |
| `flora.velvetpad.defence_per_bite` | 0.003 | fraction toward 1 per bite | [0, 1] | yes | D | measured | induced defence rise |
| `flora.velvetpad.defence_half_life_s` | 1200.0 | s (<= 0: no decay) | (-inf, inf) | yes | D | placeholder | induced defence half-life once grazing stops |
| `flora.velvetpad.profile.*.wood_fraction_max` | inf | fraction of wood_max (inf: the last stage) | [0, inf] | yes | C | placeholder | a stage applies up to this wood fraction |
| `flora.velvetpad.profile.*.height_m_max` | (absent) | m | (0, inf) | yes | B | placeholder | a stage's crown-height ceiling (a capped seedling) |
| `flora.velvetpad.profile.*.layers.*.band.*` | 0.0, 1.0 | fraction of crown height | [0, 1] | yes | C | placeholder | a layer's band [bottom, top] |
| `flora.velvetpad.profile.*.layers.*.radius` | 1.0 | fraction of crown radius | (0, 1] |  | C | placeholder | a layer's radius |
| `flora.velvetpad.profile.*.layers.*.share` | 1.0 | fraction of the stage's foliage | [0, inf) | yes | C | placeholder | a foliage layer's share (a trunk's is 0) |
| `flora.velvetpad.profile.*.layers.*.porosity` | 0.6 | fraction transmitted | [0, 1] | yes | C | placeholder | light a layer lets through |
| `flora.glowcap.substrate_uptake_per_s` | 0.02 | organic/s per unit mycelium | [0, inf) | yes | C | placeholder | saprotroph: dead wood and litter withdrawn |
| `flora.glowcap.substrate_yield` | 0.4 | fraction | [0, 1] | 0.2–0.7 | A | placeholder | saprotroph: share of the substrate taken that becomes tissue |
| `flora.glowcap.establish_substrate_min` | 0.02 | org-units | [0, inf) |  | C | placeholder | saprotroph: substrate a spore cohort needs in its box |
| `flora.glowcap.substrate_reach_up_down` | 1 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | saprotroph: the mycelium box's vertical reach |
| `flora.glowcap.alpha` | 2.0 | foliage per unit wood | [0, inf) | 0.05–3 log | D | placeholder | foliage cap P_cap = alpha * W |
| `flora.glowcap.wood_max` | 0.1 | org-units | (0, inf) | yes | D | placeholder | the most living wood one stand holds |
| `flora.glowcap.reserve_cap` | 0.5 | reserve per unit wood | [0, inf) | 0.1–2 log | B | placeholder | Q_max = q_cap * W |
| `flora.glowcap.maintenance` | 0.0002 | /s per unit wood | [0, inf) | yes | B | placeholder | wood upkeep m_w |
| `flora.glowcap.senescence` | 0.001 | /s of foliage | [0, inf) | yes | B | placeholder | leaf fall m_p, to litter |
| `flora.glowcap.foliage_rate` | 0.004 | organic/s per unit wood | [0, inf) | yes | D | placeholder | foliage regrowth cap r_p |
| `flora.glowcap.wood_rate` | 0.002 | /s per unit wood | [0, inf) | yes | C | placeholder | wood growth cap r_w |
| `flora.glowcap.build` | 0.2 | fraction | [0, inf) | 0.1–0.5 log | A | placeholder | construction respiration c_g: a unit built costs 1 + c_g |
| `flora.glowcap.dieback` | 1.0 | wood per unit unpaid upkeep | [0, inf) | 0.3–3 log | C | placeholder | kappa: wood lost per unit maintenance not paid |
| `flora.glowcap.alive_min` | 0.01 | org-units | [0, inf) | yes | C | placeholder | W_min: a stand with less wood dies |
| `flora.glowcap.donor_min` | 0.05 | org-units of wood | [0, inf) |  | C | placeholder | wood needed before sending propagules |
| `flora.glowcap.donor_reserve_floor` | 0.5 | fraction of Q_max | [0, inf) | 0–0.9 | C | placeholder | reserve a donor keeps back |
| `flora.glowcap.propagule_rate` | 0.0005 | organic/s gross | [0, inf) | yes | D | placeholder | reserve set aside for seed per second |
| `flora.glowcap.propagule_split.*` | 0.4, 0.4, 0.2 | fraction | [0, inf) | 0.1–0.8 | C | placeholder | how a landed package splits into wood, foliage and reserve (sums to 1) |
| `flora.glowcap.energy_density` | 2.0 | energy per org-unit | [0, inf) |  | D | placeholder | e_v of plant tissue |
| `flora.glowcap.seed_attrition_per_s` | 0.0001388888888888889 | /s | [0, inf) |  | C | derived | a banked seed's death rate |
| `flora.glowcap.seed_max_age_s` | 7200.0 | s | [0, inf) | yes | B | placeholder | dormancy: a bin older than this falls to litter |
| `flora.glowcap.seed_cohorts_max` | 4 | bins | [1, inf) |  | N | numerics | the bank's age resolution |
| `flora.glowcap.n_tissue` | 0.02 | mineral per org-unit | [0, inf) | 0.005–0.05 log | B | placeholder | stoichiometry of built tissue |
| `flora.glowcap.reserve_share` | 0.2 | fraction | [0, inf) | 0–0.6 | C | placeholder | share of a tick's surplus to the reserve first |
| `flora.glowcap.reflush_below` | 0.25 | fraction of P_cap | [0, inf) | 0–1 | C | placeholder | reserve funds foliage only below this |
| `flora.glowcap.assimilation` | 0.0 | organic/s per unit foliage | [0, inf) | yes | B | placeholder | g: gross fixation at full light and moisture |
| `flora.glowcap.nutrient_half` | 0.5 | mineral units | [0, inf) | 0.1–2 log | C | placeholder | K_N: mineral half-saturation |
| `flora.glowcap.nutrient_draw_max` | 0.01 | /s per unit site mineral | [0, inf) | 0.003–0.1 log | D | placeholder | f_max: rate cap on income from the site's mineral |
| `flora.glowcap.light_half` | 0.5 | fraction of open sky | [0, inf) | 0.05–1.5 | B | placeholder | light response half-saturation |
| `flora.glowcap.rooting_depth` | 1 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | root box depth below the support |
| `flora.glowcap.rooting_radius` | 1 | reference voxels (0.25 m) | [0, inf) |  | C | placeholder | root box half-width |
| `flora.glowcap.wilt_water` | 0.2 | available water (0 wp, 1 fc) | (-inf, inf) | yes | B | measured | mu = 0 at and below |
| `flora.glowcap.full_water` | 0.9 | available water | (-inf, inf) | yes | B | measured | mu = 1 at and above |
| `flora.glowcap.transpiration_m3_per_s` | 0.0 | m3/s per unit foliage at mu = 1 | [0, inf) |  | D | measured | water drawn from the root box |
| `flora.glowcap.establish_water_min` | 0.2 | available water | (-inf, inf) | yes | B | placeholder | germination moisture gate |
| `flora.glowcap.establish_light_min` | 0.0 | sky visibility | (-inf, inf) | yes | C | placeholder | germination light gate |
| `flora.glowcap.saturated_pore` | 0.95 | pore fraction of capacity | (-inf, inf) | 0.8–1 | B | placeholder | a root voxel at or above this is saturated |
| `flora.glowcap.stress_rate_per_s` | 0.1 | /s fraction of gap | [0, inf) | 0.001–0.2 log | D | placeholder | aeration stress rise |
| `flora.glowcap.relax_rate_per_s` | 0.05 | /s fraction of gap | [0, inf) | yes | B | placeholder | aeration stress recovery |
| `flora.glowcap.establish_saturated_max` | 0.5 | fraction of root box saturated | (-inf, inf) | yes | C | placeholder | germination ceiling and adult stress onset |
| `flora.glowcap.drown_depth_m` | 0.05 | m of water | [0, inf) | yes | D | tuned | standing water over the support that kills the stand |
| `flora.glowcap.water_depth_min_m` | 0.0 | m of settled standing water | [0, inf) | yes | B | placeholder | needed on or beside the site |
| `flora.glowcap.hop` | 6 | reference voxels (0.25 m) | [0, inf) | yes | D | placeholder | dispersal radius / wind kernel scale |
| `flora.glowcap.clonal_share` | 0.0 | fraction of packages | [0, 1] | 0–1 | C | tuned | runner/rhizome share |
| `flora.glowcap.spore_tries` | 8 | tries | [1, inf) | yes | C | tuned | a spore package's landing attempts |
| `flora.glowcap.seed_mass` | 1 | minimum packages | [1, inf) | yes | D | placeholder | seed size and lottery weight |
| `flora.glowcap.overtop_per_check` | 0.0 | probability per check | [0, 1] | yes | C | tuned | a woody seedling comes up through a ground cover |
| `flora.glowcap.crown_height_m.*` | 0.125, 0.25 | m | [0, inf) | yes | B | placeholder | crown top above the support [min, max] |
| `flora.glowcap.crown_radius_m.*` | 0.0625, 0.0625 | m | [0, inf) | yes | B | placeholder | crown half-width [min, max] |
| `flora.glowcap.graze_refuge` | 0.15 | fraction of each layer's capacity | [0, 1) | 0–0.6 | B | placeholder | foliage no bite can take |
| `flora.glowcap.defence_per_bite` | 0.003 | fraction toward 1 per bite | [0, 1] | yes | D | measured | induced defence rise |
| `flora.glowcap.defence_half_life_s` | 1200.0 | s (<= 0: no decay) | (-inf, inf) | yes | D | placeholder | induced defence half-life once grazing stops |
| `flora.glowcap.profile.*.wood_fraction_max` | inf | fraction of wood_max (inf: the last stage) | [0, inf] | yes | C | placeholder | a stage applies up to this wood fraction |
| `flora.glowcap.profile.*.height_m_max` | (absent) | m | (0, inf) | yes | B | placeholder | a stage's crown-height ceiling (a capped seedling) |
| `flora.glowcap.profile.*.layers.*.band.*` | 0.0, 1.0 | fraction of crown height | [0, 1] | yes | C | placeholder | a layer's band [bottom, top] |
| `flora.glowcap.profile.*.layers.*.radius` | 1.0 | fraction of crown radius | (0, 1] |  | C | placeholder | a layer's radius |
| `flora.glowcap.profile.*.layers.*.share` | 1.0 | fraction of the stage's foliage | [0, inf) | yes | C | placeholder | a foliage layer's share (a trunk's is 0) |
| `flora.glowcap.profile.*.layers.*.porosity` | 0.5 | fraction transmitted | [0, 1] | yes | C | placeholder | light a layer lets through |
| `flora.vaulttree.substrate_uptake_per_s` | 0.0 | organic/s per unit mycelium | [0, inf) | yes | C | placeholder | saprotroph: dead wood and litter withdrawn |
| `flora.vaulttree.substrate_yield` | 0.0 | fraction | [0, 1] | 0.2–0.7 | A | placeholder | saprotroph: share of the substrate taken that becomes tissue |
| `flora.vaulttree.establish_substrate_min` | 0.0 | org-units | [0, inf) |  | C | placeholder | saprotroph: substrate a spore cohort needs in its box |
| `flora.vaulttree.substrate_reach_up_down` | 1 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | saprotroph: the mycelium box's vertical reach |
| `flora.vaulttree.alpha` | 2.0 | foliage per unit wood | [0, inf) | 0.05–3 log | D | placeholder | foliage cap P_cap = alpha * W |
| `flora.vaulttree.wood_max` | 5.0 | org-units | (0, inf) | yes | D | placeholder | the most living wood one stand holds |
| `flora.vaulttree.reserve_cap` | 1.5 | reserve per unit wood | [0, inf) | 0.1–2 log | B | placeholder | Q_max = q_cap * W |
| `flora.vaulttree.maintenance` | 0.0001 | /s per unit wood | [0, inf) | yes | B | placeholder | wood upkeep m_w |
| `flora.vaulttree.senescence` | 0.0005 | /s of foliage | [0, inf) | yes | B | placeholder | leaf fall m_p, to litter |
| `flora.vaulttree.foliage_rate` | 0.001 | organic/s per unit wood | [0, inf) | yes | D | placeholder | foliage regrowth cap r_p |
| `flora.vaulttree.wood_rate` | 0.0003 | /s per unit wood | [0, inf) | yes | C | placeholder | wood growth cap r_w |
| `flora.vaulttree.build` | 0.2 | fraction | [0, inf) | 0.1–0.5 log | A | placeholder | construction respiration c_g: a unit built costs 1 + c_g |
| `flora.vaulttree.dieback` | 1.0 | wood per unit unpaid upkeep | [0, inf) | 0.3–3 log | C | placeholder | kappa: wood lost per unit maintenance not paid |
| `flora.vaulttree.alive_min` | 0.02 | org-units | [0, inf) | yes | C | placeholder | W_min: a stand with less wood dies |
| `flora.vaulttree.donor_min` | 2.5 | org-units of wood | [0, inf) |  | C | placeholder | wood needed before sending propagules |
| `flora.vaulttree.donor_reserve_floor` | 0.5 | fraction of Q_max | [0, inf) | 0–0.9 | C | placeholder | reserve a donor keeps back |
| `flora.vaulttree.propagule_rate` | 0.0008 | organic/s gross | [0, inf) | yes | D | placeholder | reserve set aside for seed per second |
| `flora.vaulttree.propagule_split.*` | 0.4, 0.4, 0.2 | fraction | [0, inf) | 0.1–0.8 | C | placeholder | how a landed package splits into wood, foliage and reserve (sums to 1) |
| `flora.vaulttree.energy_density` | 2.0 | energy per org-unit | [0, inf) |  | D | placeholder | e_v of plant tissue |
| `flora.vaulttree.seed_attrition_per_s` | 0.000046296296296296294 | /s | [0, inf) |  | C | derived | a banked seed's death rate |
| `flora.vaulttree.seed_max_age_s` | 21600.0 | s | [0, inf) | yes | B | placeholder | dormancy: a bin older than this falls to litter |
| `flora.vaulttree.seed_cohorts_max` | 4 | bins | [1, inf) |  | N | numerics | the bank's age resolution |
| `flora.vaulttree.n_tissue` | 0.02 | mineral per org-unit | [0, inf) | 0.005–0.05 log | B | placeholder | stoichiometry of built tissue |
| `flora.vaulttree.reserve_share` | 0.2 | fraction | [0, inf) | 0–0.6 | C | placeholder | share of a tick's surplus to the reserve first |
| `flora.vaulttree.reflush_below` | 0.25 | fraction of P_cap | [0, inf) | 0–1 | C | placeholder | reserve funds foliage only below this |
| `flora.vaulttree.assimilation` | 0.004 | organic/s per unit foliage | [0, inf) | yes | B | placeholder | g: gross fixation at full light and moisture |
| `flora.vaulttree.nutrient_half` | 0.5 | mineral units | [0, inf) | 0.1–2 log | C | placeholder | K_N: mineral half-saturation |
| `flora.vaulttree.nutrient_draw_max` | 0.01 | /s per unit site mineral | [0, inf) | 0.003–0.1 log | D | placeholder | f_max: rate cap on income from the site's mineral |
| `flora.vaulttree.light_half` | 0.6 | fraction of open sky | [0, inf) | 0.05–1.5 | B | placeholder | light response half-saturation |
| `flora.vaulttree.rooting_depth` | 6 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | root box depth below the support |
| `flora.vaulttree.rooting_radius` | 2 | reference voxels (0.25 m) | [0, inf) |  | C | placeholder | root box half-width |
| `flora.vaulttree.wilt_water` | 0.1 | available water (0 wp, 1 fc) | (-inf, inf) | yes | B | measured | mu = 0 at and below |
| `flora.vaulttree.full_water` | 0.5 | available water | (-inf, inf) | yes | B | measured | mu = 1 at and above |
| `flora.vaulttree.transpiration_m3_per_s` | 0.000001 | m3/s per unit foliage at mu = 1 | [0, inf) |  | D | measured | water drawn from the root box |
| `flora.vaulttree.establish_water_min` | 0.2 | available water | (-inf, inf) | yes | B | placeholder | germination moisture gate |
| `flora.vaulttree.establish_light_min` | 0.9 | sky visibility | (-inf, inf) | yes | C | placeholder | germination light gate |
| `flora.vaulttree.saturated_pore` | 0.95 | pore fraction of capacity | (-inf, inf) | 0.8–1 | B | placeholder | a root voxel at or above this is saturated |
| `flora.vaulttree.stress_rate_per_s` | 0.005 | /s fraction of gap | [0, inf) | 0.001–0.2 log | D | placeholder | aeration stress rise |
| `flora.vaulttree.relax_rate_per_s` | 0.02 | /s fraction of gap | [0, inf) | yes | B | placeholder | aeration stress recovery |
| `flora.vaulttree.establish_saturated_max` | 0.2 | fraction of root box saturated | (-inf, inf) | yes | C | placeholder | germination ceiling and adult stress onset |
| `flora.vaulttree.drown_depth_m` | 0.1 | m of water | [0, inf) | yes | D | tuned | standing water over the support that kills the stand |
| `flora.vaulttree.water_depth_min_m` | 0.0 | m of settled standing water | [0, inf) | yes | B | placeholder | needed on or beside the site |
| `flora.vaulttree.hop` | 4 | reference voxels (0.25 m) | [0, inf) | yes | D | placeholder | dispersal radius / wind kernel scale |
| `flora.vaulttree.clonal_share` | 0.0 | fraction of packages | [0, 1] | 0–1 | C | tuned | runner/rhizome share |
| `flora.vaulttree.spore_tries` | 8 | tries | [1, inf) | yes | C | tuned | a spore package's landing attempts |
| `flora.vaulttree.seed_mass` | 4 | minimum packages | [1, inf) | yes | D | placeholder | seed size and lottery weight |
| `flora.vaulttree.overtop_per_check` | 0.1 | probability per check | [0, 1] | yes | C | tuned | a woody seedling comes up through a ground cover |
| `flora.vaulttree.crown_height_m.*` | 2.25, 3.5 | m | [0, inf) | yes | B | placeholder | crown top above the support [min, max] |
| `flora.vaulttree.crown_radius_m.*` | 0.625, 1.0 | m | [0, inf) | yes | B | placeholder | crown half-width [min, max] |
| `flora.vaulttree.graze_refuge` | 0.15 | fraction of each layer's capacity | [0, 1) | 0–0.6 | B | placeholder | foliage no bite can take |
| `flora.vaulttree.defence_per_bite` | 0.003 | fraction toward 1 per bite | [0, 1] | yes | D | measured | induced defence rise |
| `flora.vaulttree.defence_half_life_s` | 1200.0 | s (<= 0: no decay) | (-inf, inf) | yes | D | placeholder | induced defence half-life once grazing stops |
| `flora.vaulttree.profile.*.wood_fraction_max` | 0.1, 0.4, inf | fraction of wood_max (inf: the last stage) | [0, inf] | yes | C | placeholder | a stage applies up to this wood fraction |
| `flora.vaulttree.profile.*.height_m_max` | 0.125 | m | (0, inf) | yes | B | placeholder | a stage's crown-height ceiling (a capped seedling) |
| `flora.vaulttree.profile.*.layers.*.band.*` | 0.0, 1.0, 0.0, 0.6 … (14 values) | fraction of crown height | [0, 1] | yes | C | placeholder | a layer's band [bottom, top] |
| `flora.vaulttree.profile.*.layers.*.radius` | 1.0, 0.15, 1.0, 0.15 … (7 values) | fraction of crown radius | (0, 1] |  | C | placeholder | a layer's radius |
| `flora.vaulttree.profile.*.layers.*.share` | 1.0, 0.0, 1.0, 0.0 … (7 values) | fraction of the stage's foliage | [0, inf) | yes | C | placeholder | a foliage layer's share (a trunk's is 0) |
| `flora.vaulttree.profile.*.layers.*.porosity` | 0.4, 0.0, 0.4, 0.0 … (7 values) | fraction transmitted | [0, 1] | yes | C | placeholder | light a layer lets through |
| `flora.lanternberry.substrate_uptake_per_s` | 0.0 | organic/s per unit mycelium | [0, inf) | yes | C | placeholder | saprotroph: dead wood and litter withdrawn |
| `flora.lanternberry.substrate_yield` | 0.0 | fraction | [0, 1] | 0.2–0.7 | A | placeholder | saprotroph: share of the substrate taken that becomes tissue |
| `flora.lanternberry.establish_substrate_min` | 0.0 | org-units | [0, inf) |  | C | placeholder | saprotroph: substrate a spore cohort needs in its box |
| `flora.lanternberry.substrate_reach_up_down` | 1 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | saprotroph: the mycelium box's vertical reach |
| `flora.lanternberry.alpha` | 2.0 | foliage per unit wood | [0, inf) | 0.05–3 log | D | placeholder | foliage cap P_cap = alpha * W |
| `flora.lanternberry.wood_max` | 1.0 | org-units | (0, inf) | yes | D | placeholder | the most living wood one stand holds |
| `flora.lanternberry.reserve_cap` | 0.5 | reserve per unit wood | [0, inf) | 0.1–2 log | B | placeholder | Q_max = q_cap * W |
| `flora.lanternberry.maintenance` | 0.0002 | /s per unit wood | [0, inf) | yes | B | placeholder | wood upkeep m_w |
| `flora.lanternberry.senescence` | 0.001 | /s of foliage | [0, inf) | yes | B | placeholder | leaf fall m_p, to litter |
| `flora.lanternberry.foliage_rate` | 0.002 | organic/s per unit wood | [0, inf) | yes | D | placeholder | foliage regrowth cap r_p |
| `flora.lanternberry.wood_rate` | 0.001 | /s per unit wood | [0, inf) | yes | C | placeholder | wood growth cap r_w |
| `flora.lanternberry.build` | 0.2 | fraction | [0, inf) | 0.1–0.5 log | A | placeholder | construction respiration c_g: a unit built costs 1 + c_g |
| `flora.lanternberry.dieback` | 1.0 | wood per unit unpaid upkeep | [0, inf) | 0.3–3 log | C | placeholder | kappa: wood lost per unit maintenance not paid |
| `flora.lanternberry.alive_min` | 0.02 | org-units | [0, inf) | yes | C | placeholder | W_min: a stand with less wood dies |
| `flora.lanternberry.donor_min` | 0.4 | org-units of wood | [0, inf) |  | C | placeholder | wood needed before sending propagules |
| `flora.lanternberry.donor_reserve_floor` | 0.7 | fraction of Q_max | [0, inf) | 0–0.9 | C | placeholder | reserve a donor keeps back |
| `flora.lanternberry.propagule_rate` | 0.0012 | organic/s gross | [0, inf) | yes | D | placeholder | reserve set aside for seed per second |
| `flora.lanternberry.propagule_split.*` | 0.4, 0.4, 0.2 | fraction | [0, inf) | 0.1–0.8 | C | placeholder | how a landed package splits into wood, foliage and reserve (sums to 1) |
| `flora.lanternberry.energy_density` | 2.0 | energy per org-unit | [0, inf) |  | D | placeholder | e_v of plant tissue |
| `flora.lanternberry.seed_attrition_per_s` | 0.00003472222222222222 | /s | [0, inf) |  | C | derived | a banked seed's death rate |
| `flora.lanternberry.seed_max_age_s` | 28800.0 | s | [0, inf) | yes | B | placeholder | dormancy: a bin older than this falls to litter |
| `flora.lanternberry.seed_cohorts_max` | 4 | bins | [1, inf) |  | N | numerics | the bank's age resolution |
| `flora.lanternberry.n_tissue` | 0.02 | mineral per org-unit | [0, inf) | 0.005–0.05 log | B | placeholder | stoichiometry of built tissue |
| `flora.lanternberry.reserve_share` | 0.2 | fraction | [0, inf) | 0–0.6 | C | placeholder | share of a tick's surplus to the reserve first |
| `flora.lanternberry.reflush_below` | 0.25 | fraction of P_cap | [0, inf) | 0–1 | C | placeholder | reserve funds foliage only below this |
| `flora.lanternberry.assimilation` | 0.004 | organic/s per unit foliage | [0, inf) | yes | B | placeholder | g: gross fixation at full light and moisture |
| `flora.lanternberry.nutrient_half` | 0.5 | mineral units | [0, inf) | 0.1–2 log | C | placeholder | K_N: mineral half-saturation |
| `flora.lanternberry.nutrient_draw_max` | 0.01 | /s per unit site mineral | [0, inf) | 0.003–0.1 log | D | placeholder | f_max: rate cap on income from the site's mineral |
| `flora.lanternberry.light_half` | 0.4 | fraction of open sky | [0, inf) | 0.05–1.5 | B | placeholder | light response half-saturation |
| `flora.lanternberry.rooting_depth` | 3 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | root box depth below the support |
| `flora.lanternberry.rooting_radius` | 1 | reference voxels (0.25 m) | [0, inf) |  | C | placeholder | root box half-width |
| `flora.lanternberry.wilt_water` | 0.1 | available water (0 wp, 1 fc) | (-inf, inf) | yes | B | measured | mu = 0 at and below |
| `flora.lanternberry.full_water` | 0.5 | available water | (-inf, inf) | yes | B | measured | mu = 1 at and above |
| `flora.lanternberry.transpiration_m3_per_s` | 0.000001 | m3/s per unit foliage at mu = 1 | [0, inf) |  | D | measured | water drawn from the root box |
| `flora.lanternberry.establish_water_min` | 0.4 | available water | (-inf, inf) | yes | B | placeholder | germination moisture gate |
| `flora.lanternberry.establish_light_min` | 0.35 | sky visibility | (-inf, inf) | yes | C | placeholder | germination light gate |
| `flora.lanternberry.saturated_pore` | 0.95 | pore fraction of capacity | (-inf, inf) | 0.8–1 | B | placeholder | a root voxel at or above this is saturated |
| `flora.lanternberry.stress_rate_per_s` | 0.1 | /s fraction of gap | [0, inf) | 0.001–0.2 log | D | placeholder | aeration stress rise |
| `flora.lanternberry.relax_rate_per_s` | 0.05 | /s fraction of gap | [0, inf) | yes | B | placeholder | aeration stress recovery |
| `flora.lanternberry.establish_saturated_max` | 0.5 | fraction of root box saturated | (-inf, inf) | yes | C | placeholder | germination ceiling and adult stress onset |
| `flora.lanternberry.drown_depth_m` | 0.1 | m of water | [0, inf) | yes | D | tuned | standing water over the support that kills the stand |
| `flora.lanternberry.water_depth_min_m` | 0.0 | m of settled standing water | [0, inf) | yes | B | placeholder | needed on or beside the site |
| `flora.lanternberry.hop` | 2 | reference voxels (0.25 m) | [0, inf) | yes | D | placeholder | dispersal radius / wind kernel scale |
| `flora.lanternberry.clonal_share` | 0.0 | fraction of packages | [0, 1] | 0–1 | C | tuned | runner/rhizome share |
| `flora.lanternberry.spore_tries` | 8 | tries | [1, inf) | yes | C | tuned | a spore package's landing attempts |
| `flora.lanternberry.seed_mass` | 3 | minimum packages | [1, inf) | yes | D | placeholder | seed size and lottery weight |
| `flora.lanternberry.overtop_per_check` | 0.1 | probability per check | [0, 1] | yes | C | tuned | a woody seedling comes up through a ground cover |
| `flora.lanternberry.crown_height_m.*` | 0.625, 1.125 | m | [0, inf) | yes | B | placeholder | crown top above the support [min, max] |
| `flora.lanternberry.crown_radius_m.*` | 0.25, 0.4375 | m | [0, inf) | yes | B | placeholder | crown half-width [min, max] |
| `flora.lanternberry.graze_refuge` | 0.15 | fraction of each layer's capacity | [0, 1) | 0–0.6 | B | placeholder | foliage no bite can take |
| `flora.lanternberry.defence_per_bite` | 0.003 | fraction toward 1 per bite | [0, 1] | yes | D | measured | induced defence rise |
| `flora.lanternberry.defence_half_life_s` | 1200.0 | s (<= 0: no decay) | (-inf, inf) | yes | D | placeholder | induced defence half-life once grazing stops |
| `flora.lanternberry.profile.*.wood_fraction_max` | 0.2, inf | fraction of wood_max (inf: the last stage) | [0, inf] | yes | C | placeholder | a stage applies up to this wood fraction |
| `flora.lanternberry.profile.*.height_m_max` | 0.125 | m | (0, inf) | yes | B | placeholder | a stage's crown-height ceiling (a capped seedling) |
| `flora.lanternberry.profile.*.layers.*.band.*` | 0.0, 1.0, 0.0, 0.3 … (8 values) | fraction of crown height | [0, 1] | yes | C | placeholder | a layer's band [bottom, top] |
| `flora.lanternberry.profile.*.layers.*.radius` | 1.0, 0.4, 1.0, 1.0 | fraction of crown radius | (0, 1] |  | C | placeholder | a layer's radius |
| `flora.lanternberry.profile.*.layers.*.share` | 1.0, 0.0, 0.21428571428571427, 0.7857142857142857 | fraction of the stage's foliage | [0, inf) | yes | C | placeholder | a foliage layer's share (a trunk's is 0) |
| `flora.lanternberry.profile.*.layers.*.porosity` | 0.5, 0.7, 0.5, 0.5 | fraction transmitted | [0, 1] | yes | C | placeholder | light a layer lets through |
| `flora.siphonreed.substrate_uptake_per_s` | 0.0 | organic/s per unit mycelium | [0, inf) | yes | C | placeholder | saprotroph: dead wood and litter withdrawn |
| `flora.siphonreed.substrate_yield` | 0.0 | fraction | [0, 1] | 0.2–0.7 | A | placeholder | saprotroph: share of the substrate taken that becomes tissue |
| `flora.siphonreed.establish_substrate_min` | 0.0 | org-units | [0, inf) |  | C | placeholder | saprotroph: substrate a spore cohort needs in its box |
| `flora.siphonreed.substrate_reach_up_down` | 1 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | saprotroph: the mycelium box's vertical reach |
| `flora.siphonreed.alpha` | 2.0 | foliage per unit wood | [0, inf) | 0.05–3 log | D | placeholder | foliage cap P_cap = alpha * W |
| `flora.siphonreed.wood_max` | 0.3 | org-units | (0, inf) | yes | D | placeholder | the most living wood one stand holds |
| `flora.siphonreed.reserve_cap` | 0.5 | reserve per unit wood | [0, inf) | 0.1–2 log | B | placeholder | Q_max = q_cap * W |
| `flora.siphonreed.maintenance` | 0.0004 | /s per unit wood | [0, inf) | yes | B | placeholder | wood upkeep m_w |
| `flora.siphonreed.senescence` | 0.001 | /s of foliage | [0, inf) | yes | B | placeholder | leaf fall m_p, to litter |
| `flora.siphonreed.foliage_rate` | 0.002 | organic/s per unit wood | [0, inf) | yes | D | placeholder | foliage regrowth cap r_p |
| `flora.siphonreed.wood_rate` | 0.001 | /s per unit wood | [0, inf) | yes | C | placeholder | wood growth cap r_w |
| `flora.siphonreed.build` | 0.2 | fraction | [0, inf) | 0.1–0.5 log | A | placeholder | construction respiration c_g: a unit built costs 1 + c_g |
| `flora.siphonreed.dieback` | 1.0 | wood per unit unpaid upkeep | [0, inf) | 0.3–3 log | C | placeholder | kappa: wood lost per unit maintenance not paid |
| `flora.siphonreed.alive_min` | 0.02 | org-units | [0, inf) | yes | C | placeholder | W_min: a stand with less wood dies |
| `flora.siphonreed.donor_min` | 0.15 | org-units of wood | [0, inf) |  | C | placeholder | wood needed before sending propagules |
| `flora.siphonreed.donor_reserve_floor` | 0.5 | fraction of Q_max | [0, inf) | 0–0.9 | C | placeholder | reserve a donor keeps back |
| `flora.siphonreed.propagule_rate` | 0.0003 | organic/s gross | [0, inf) | yes | D | placeholder | reserve set aside for seed per second |
| `flora.siphonreed.propagule_split.*` | 0.4, 0.4, 0.2 | fraction | [0, inf) | 0.1–0.8 | C | placeholder | how a landed package splits into wood, foliage and reserve (sums to 1) |
| `flora.siphonreed.energy_density` | 2.0 | energy per org-unit | [0, inf) |  | D | placeholder | e_v of plant tissue |
| `flora.siphonreed.seed_attrition_per_s` | 0.00002777777777777778 | /s | [0, inf) |  | C | derived | a banked seed's death rate |
| `flora.siphonreed.seed_max_age_s` | 36000.0 | s | [0, inf) | yes | B | placeholder | dormancy: a bin older than this falls to litter |
| `flora.siphonreed.seed_cohorts_max` | 4 | bins | [1, inf) |  | N | numerics | the bank's age resolution |
| `flora.siphonreed.n_tissue` | 0.02 | mineral per org-unit | [0, inf) | 0.005–0.05 log | B | placeholder | stoichiometry of built tissue |
| `flora.siphonreed.reserve_share` | 0.2 | fraction | [0, inf) | 0–0.6 | C | placeholder | share of a tick's surplus to the reserve first |
| `flora.siphonreed.reflush_below` | 0.25 | fraction of P_cap | [0, inf) | 0–1 | C | placeholder | reserve funds foliage only below this |
| `flora.siphonreed.assimilation` | 0.004 | organic/s per unit foliage | [0, inf) | yes | B | placeholder | g: gross fixation at full light and moisture |
| `flora.siphonreed.nutrient_half` | 0.5 | mineral units | [0, inf) | 0.1–2 log | C | placeholder | K_N: mineral half-saturation |
| `flora.siphonreed.nutrient_draw_max` | 0.01 | /s per unit site mineral | [0, inf) | 0.003–0.1 log | D | placeholder | f_max: rate cap on income from the site's mineral |
| `flora.siphonreed.light_half` | 0.8 | fraction of open sky | [0, inf) | 0.05–1.5 | B | placeholder | light response half-saturation |
| `flora.siphonreed.rooting_depth` | 2 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | root box depth below the support |
| `flora.siphonreed.rooting_radius` | 1 | reference voxels (0.25 m) | [0, inf) |  | C | placeholder | root box half-width |
| `flora.siphonreed.wilt_water` | 1.0 | available water (0 wp, 1 fc) | (-inf, inf) | yes | B | measured | mu = 0 at and below |
| `flora.siphonreed.full_water` | 1.5 | available water | (-inf, inf) | yes | B | measured | mu = 1 at and above |
| `flora.siphonreed.transpiration_m3_per_s` | 0.000001 | m3/s per unit foliage at mu = 1 | [0, inf) |  | D | measured | water drawn from the root box |
| `flora.siphonreed.establish_water_min` | 1.2 | available water | (-inf, inf) | yes | B | placeholder | germination moisture gate |
| `flora.siphonreed.establish_light_min` | 0.6 | sky visibility | (-inf, inf) | yes | C | placeholder | germination light gate |
| `flora.siphonreed.saturated_pore` | 0.95 | pore fraction of capacity | (-inf, inf) | 0.8–1 | B | placeholder | a root voxel at or above this is saturated |
| `flora.siphonreed.stress_rate_per_s` | 0.01 | /s fraction of gap | [0, inf) | 0.001–0.2 log | D | placeholder | aeration stress rise |
| `flora.siphonreed.relax_rate_per_s` | 0.05 | /s fraction of gap | [0, inf) | yes | B | placeholder | aeration stress recovery |
| `flora.siphonreed.establish_saturated_max` | 1.0 | fraction of root box saturated | (-inf, inf) | yes | C | placeholder | germination ceiling and adult stress onset |
| `flora.siphonreed.drown_depth_m` | 0.5 | m of water | [0, inf) | yes | D | tuned | standing water over the support that kills the stand |
| `flora.siphonreed.water_depth_min_m` | 0.1 | m of settled standing water | [0, inf) | yes | B | placeholder | needed on or beside the site |
| `flora.siphonreed.hop` | 6 | reference voxels (0.25 m) | [0, inf) | yes | D | placeholder | dispersal radius / wind kernel scale |
| `flora.siphonreed.clonal_share` | 0.7 | fraction of packages | [0, 1] | 0–1 | C | tuned | runner/rhizome share |
| `flora.siphonreed.spore_tries` | 8 | tries | [1, inf) | yes | C | tuned | a spore package's landing attempts |
| `flora.siphonreed.seed_mass` | 1 | minimum packages | [1, inf) | yes | D | placeholder | seed size and lottery weight |
| `flora.siphonreed.overtop_per_check` | 0.0 | probability per check | [0, 1] | yes | C | tuned | a woody seedling comes up through a ground cover |
| `flora.siphonreed.crown_height_m.*` | 0.75, 1.5 | m | [0, inf) | yes | B | placeholder | crown top above the support [min, max] |
| `flora.siphonreed.crown_radius_m.*` | 0.0625, 0.125 | m | [0, inf) | yes | B | placeholder | crown half-width [min, max] |
| `flora.siphonreed.graze_refuge` | 0.15 | fraction of each layer's capacity | [0, 1) | 0–0.6 | B | placeholder | foliage no bite can take |
| `flora.siphonreed.defence_per_bite` | 0.003 | fraction toward 1 per bite | [0, 1] | yes | D | measured | induced defence rise |
| `flora.siphonreed.defence_half_life_s` | 1200.0 | s (<= 0: no decay) | (-inf, inf) | yes | D | placeholder | induced defence half-life once grazing stops |
| `flora.siphonreed.profile.*.wood_fraction_max` | inf | fraction of wood_max (inf: the last stage) | [0, inf] | yes | C | placeholder | a stage applies up to this wood fraction |
| `flora.siphonreed.profile.*.height_m_max` | (absent) | m | (0, inf) | yes | B | placeholder | a stage's crown-height ceiling (a capped seedling) |
| `flora.siphonreed.profile.*.layers.*.band.*` | 0.0, 1.0 | fraction of crown height | [0, 1] | yes | C | placeholder | a layer's band [bottom, top] |
| `flora.siphonreed.profile.*.layers.*.radius` | 1.0 | fraction of crown radius | (0, 1] |  | C | placeholder | a layer's radius |
| `flora.siphonreed.profile.*.layers.*.share` | 1.0 | fraction of the stage's foliage | [0, inf) | yes | C | placeholder | a foliage layer's share (a trunk's is 0) |
| `flora.siphonreed.profile.*.layers.*.porosity` | 0.7 | fraction transmitted | [0, 1] | yes | C | placeholder | light a layer lets through |
| `flora.latticevine.rooting_depth` | 2 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | root box depth |
| `flora.latticevine.rooting_radius` | 1 | reference voxels (0.25 m) | [0, inf) | yes | C | placeholder | root box radius |
| `flora.latticevine.transpiration_m3_per_s` | 0.00000006 | m3/s per reference face | [0, inf) |  | D | derived | water per full face |
| `flora.latticevine.wilt_water` | 0.1 | available water | [0, inf) | yes | B | placeholder | mu = 0 at and below |
| `flora.latticevine.full_water` | 0.5 | available water | [0, inf) | yes | B | placeholder | mu = 1 at and above |
| `flora.latticevine.dry_water` | 0.15 | available water | [0, inf) | yes | C | placeholder | the root box is dry below |
| `flora.latticevine.wet_water` | 0.4 | available water | [0, inf) | yes | C | placeholder | and wet again above |
| `flora.latticevine.dry_spell_s` | 600.0 | s | [0, inf) | yes | C | placeholder | dry this long: dormant |
| `flora.latticevine.wet_spell_s` | 300.0 | s | [0, inf) | yes | C | placeholder | wet this long: regreens |
| `flora.latticevine.leaf_fall_s` | 60.0 | s | (0, inf) |  | C | placeholder | a dormant face drops its leaf over this |
| `flora.latticevine.assimilation` | 0.00009 | organic/s per reference face | [0, inf) | yes | C | placeholder | income per full face in full light |
| `flora.latticevine.underside_light` | 0.15 | fraction | [0, inf) | yes | B | placeholder | light factor on a downward face |
| `flora.latticevine.leaf_mass` | 0.05 | org-units per reference face | (0, inf) | yes | C | placeholder | leaf tissue per full face |
| `flora.latticevine.runner_mass` | 0.02 | org-units per reference face | [0, inf) | yes | C | placeholder | runner tissue per face |
| `flora.latticevine.leaf_upkeep` | 0.00001 | organic/s per reference face | [0, inf) | yes | B | placeholder | leaf maintenance |
| `flora.latticevine.wood_upkeep` | 0.000002 | organic/s per reference face | [0, inf) | yes | B | placeholder | runner maintenance |
| `flora.latticevine.regrow_rate` | 0.002 | leafiness/s | [0, inf) | yes | B | placeholder | a face's releafing rate |
| `flora.latticevine.spread_cost` | 0.05 | org-units per reference face | [0, inf) | yes | C | placeholder | a new face's cost (at least its runner) |
| `flora.latticevine.spread_check_s` | 5.0 | s | (0, inf) |  | C | placeholder | how often a vine tries to spread |
| `flora.latticevine.sister_cost` | 0.1 | org-units per reference face | [0, inf) | yes | C | placeholder | a sister vine's cost |
| `flora.latticevine.sister_min_dist` | 4 | faces | [0, inf) | yes | C | placeholder | a sister roots at least this far along the cover |
| `flora.latticevine.contest_threshold` | 0.2 | leafiness | [0, 1] | yes | C | placeholder | a face below this is contestable |
| `flora.latticevine.contest_check_s` | 30.0 | s | (0, inf) |  | C | placeholder | the contest wheel's period |
| `flora.latticevine.spur_reserve_min` | 0.005 | org-units per reference face | [0, inf) | yes | C | placeholder | reserve per face before a flowering cycle |
| `flora.latticevine.spur_window_s` | 120.0 | s | [0, inf) | yes | C | placeholder | after a shower, a cycle may still start this long |
| `flora.latticevine.spur_jitter_s` | 60.0 | s | [0, inf) |  | C | placeholder | spread of the per-face start delay |
| `flora.latticevine.bud_s` | 60.0 | s | [0, inf) | yes | C | placeholder | bud phase |
| `flora.latticevine.flower_s` | 120.0 | s | [0, inf) | yes | C | placeholder | flower phase |
| `flora.latticevine.fruit_s` | 240.0 | s | [0, inf) | yes | C | placeholder | fruit phase |
| `flora.latticevine.spent_s` | 60.0 | s | [0, inf) | yes | C | placeholder | spent phase |
| `flora.latticevine.nectar` | 0.002 | org-units per reference face | [0, inf) | yes | C | placeholder | a flower's organic matter |
| `flora.latticevine.fruit` | 0.005 | org-units per reference face | [0, inf) | yes | C | placeholder | a bead bunch's organic matter |
| `flora.latticevine.energy_density` | 1.0 | energy per org-unit | [0, inf) |  | D | placeholder | vine tissue energy |
| `flora.latticevine.founders` | 6 | vines | [0, inf) | yes | C | placeholder | founder vines rooted on a fresh world |
| `flora.latticevine.founder_reserve` | 0.1 | org-units per reference face | [0, inf) | yes | C | placeholder | each founder's starting reserve |
| `flora.shade_k_per_m2` | 0.09375 | m2 per org-unit | [0, inf) | yes | D | derived | canopy attenuation per square metre |
| `flora.decomposition` | 0.001 | /s | [0, inf) | yes | B | placeholder | k_d: litter decomposition |
| `flora.wood_decomposition` | 0.0001 | /s | [0, inf) | yes | C | placeholder | k_w: dead-wood decomposition |
| `flora.carrion_decomposition` | 0.00028 | /s | [0, inf) | yes | A | placeholder | k_c: carrion decomposition |
| `flora.litter_energy_cap` | 2.0 | energy per org-unit | [0, inf) |  | C | placeholder | retained energy cap per unit of litter |
| `flora.initial_mineral` | 1.0 | mineral per site | [0, inf) | yes | D | placeholder | mineral a site is provisioned with |
| `flora.seed_check_s` | 30.0 | s | [0, inf) |  | N | numerics | how often a banked site is tested |
| `flora.voxel_m` | 0.25 | m | (0, inf) |  | - | derived | the world's cell size, recorded (set from world.voxel_m) |

**`fauna.*`** (201 rows)

| path | value | unit | range | search | grade | provenance | what |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `fauna.frondgrazer.maintenance_per_s` | 0.001 | org-units/s per unit body | [0, inf) | 0.00025–0.004 log | B | placeholder | basal upkeep, paid from reserve then body |
| `fauna.frondgrazer.bite_per_s` | 0.06 | org-units/s | [0, inf) | 0.0002–0.004 log | C | placeholder | the hungry mouth's intake rate |
| `fauna.frondgrazer.bite_half_stock` | 0.15 | org-units at the mouth | [0, inf) | 0.0001–0.05 log | C | tuned | K: a bite is want * E / (E + K) |
| `fauna.frondgrazer.n_tissue` | 0.05 | mineral per org-unit | [0, inf) | 0.01–0.12 log | A | placeholder | tissue stoichiometry: a bite builds at most mineral / n_tissue |
| `fauna.frondgrazer.body_max` | 1.5 | org-units | [0, inf) |  | D | placeholder | adult structure |
| `fauna.frondgrazer.growth_max_per_s` | 0.000285 | org-units/s | [0, inf) | 0.000001–0.0001 log | B | derived | fastest structure grows from the reserve |
| `fauna.frondgrazer.body_min` | 0.15 | org-units | (0, inf) | yes | B | placeholder | death threshold, and a newborn's body |
| `fauna.frondgrazer.birth_body` | 0.8999999999999999 | org-units | [0, inf) | yes | B | placeholder | structure at which it can pay for a birth |
| `fauna.frondgrazer.birth_cost` | 0.3 | org-units | [0, inf) | yes | B | placeholder | a gestation's package: the newborn's body_min plus its reserve |
| `fauna.frondgrazer.reserve_cap` | 0.5 | fraction of body | [0, inf) | 0.2–1.5 | A | placeholder | ceiling on new intake into the reserve |
| `fauna.frondgrazer.drown_depth_m` | 0.2 | m | [0, inf) |  | D | placeholder | standing water that kills the heuristic body and a clutch |
| `fauna.frondgrazer.energy_density` | 2.0 | energy per org-unit | [0, inf) |  | N | placeholder | energy an introduced animal arrives with |
| `fauna.frondgrazer.lifespan_median_s` | 14400.0 | s | [0, inf) | 3600–57600 log | B | derived | median lifespan (0: none) |
| `fauna.frondgrazer.reproduction.surplus_floor` | 0.15 | org-units above the package | [0, inf) | yes | C | placeholder | reserve margin above the package that counts as surplus |
| `fauna.frondgrazer.reproduction.surplus_hold_s` | 900.0 | s | [0, inf) | 60–3600 log | C | derived | the window a surplus is judged over |
| `fauna.frondgrazer.reproduction.surplus_share` | 0.8 | fraction of the hold | [0, 1] |  | - | decision | share of the hold spent in surplus before breeding |
| `fauna.frondgrazer.reproduction.gestation_s` | 180.0 | s | [0, inf) | 60–6000 log | D | placeholder | a gestation's length |
| `fauna.frondgrazer.reproduction.gestation_loss_fraction` | 0.25 | fraction | [0, 1] | 0–1 | C | placeholder | organic respired when a gestation fails |
| `fauna.frondgrazer.reproduction.birth_interval_s` | 1800.0 | s | [0, inf) | 300–10000 log | B | derived | refractory after a birth or a laying |
| `fauna.frondgrazer.reproduction.clutch_size` | 0 | eggs | [0, inf) | yes | D | derived | eggs per clutch (an egg-layer's at least 1) |
| `fauna.frondgrazer.reproduction.egg_organic` | 0.0 | org-units per egg | [0, inf) | yes | D | placeholder | one egg; at least the lineage's body_min |
| `fauna.frondgrazer.reproduction.incubation_s` | 0.0 | s | [0, inf) | 60–6000 log | B | placeholder | a clutch incubates this long |
| `fauna.founders.littershredder.core.maintenance_per_s` | 0.001 | org-units/s per unit body | [0, inf) | 0.00025–0.004 log | B | placeholder | basal upkeep, paid from reserve then body |
| `fauna.founders.littershredder.core.bite_per_s` | 0.015 | org-units/s | [0, inf) | 0.0002–0.004 log | C | placeholder | the hungry mouth's intake rate |
| `fauna.founders.littershredder.core.bite_half_stock` | 0.0375 | org-units at the mouth | [0, inf) | 0.0001–0.05 log | C | tuned | K: a bite is want * E / (E + K) |
| `fauna.founders.littershredder.core.n_tissue` | 0.02 | mineral per org-unit | [0, inf) | 0.01–0.12 log | A | placeholder | tissue stoichiometry: a bite builds at most mineral / n_tissue |
| `fauna.founders.littershredder.core.body_max` | 0.375 | org-units | [0, inf) |  | D | placeholder | adult structure |
| `fauna.founders.littershredder.core.growth_max_per_s` | 0.000102 | org-units/s | [0, inf) | 0.000001–0.0001 log | B | derived | fastest structure grows from the reserve |
| `fauna.founders.littershredder.core.body_min` | 0.09375 | org-units | (0, inf) | yes | B | placeholder | death threshold, and a newborn's body |
| `fauna.founders.littershredder.core.birth_body` | 0.375 | org-units | [0, inf) | yes | B | placeholder | structure at which it can pay for a birth |
| `fauna.founders.littershredder.core.birth_cost` | 0.1875 | org-units | [0, inf) | yes | B | placeholder | a gestation's package: the newborn's body_min plus its reserve |
| `fauna.founders.littershredder.core.reserve_cap` | 0.5 | fraction of body | [0, inf) | 0.2–1.5 | A | placeholder | ceiling on new intake into the reserve |
| `fauna.founders.littershredder.core.drown_depth_m` | 0.2 | m | [0, inf) |  | D | placeholder | standing water that kills the heuristic body and a clutch |
| `fauna.founders.littershredder.core.energy_density` | 2.0 | energy per org-unit | [0, inf) |  | N | placeholder | energy an introduced animal arrives with |
| `fauna.founders.littershredder.core.lifespan_median_s` | 14400.0 | s | [0, inf) | 3600–57600 log | B | derived | median lifespan (0: none) |
| `fauna.founders.littershredder.core.reproduction.surplus_floor` | 0.01875 | org-units above the package | [0, inf) | yes | C | placeholder | reserve margin above the package that counts as surplus |
| `fauna.founders.littershredder.core.reproduction.surplus_hold_s` | 900.0 | s | [0, inf) | 60–3600 log | C | derived | the window a surplus is judged over |
| `fauna.founders.littershredder.core.reproduction.surplus_share` | 0.8 | fraction of the hold | [0, 1] |  | - | decision | share of the hold spent in surplus before breeding |
| `fauna.founders.littershredder.core.reproduction.gestation_s` | 0.0 | s | [0, inf) | 60–6000 log | D | placeholder | a gestation's length |
| `fauna.founders.littershredder.core.reproduction.gestation_loss_fraction` | 0.25 | fraction | [0, 1] | 0–1 | C | placeholder | organic respired when a gestation fails |
| `fauna.founders.littershredder.core.reproduction.birth_interval_s` | 1800.0 | s | [0, inf) | 300–10000 log | B | derived | refractory after a birth or a laying |
| `fauna.founders.littershredder.core.reproduction.clutch_size` | 1 | eggs | [0, inf) | yes | D | derived | eggs per clutch (an egg-layer's at least 1) |
| `fauna.founders.littershredder.core.reproduction.egg_organic` | 0.12 | org-units per egg | [0, inf) | yes | D | placeholder | one egg; at least the lineage's body_min |
| `fauna.founders.littershredder.core.reproduction.incubation_s` | 300.0 | s | [0, inf) | 60–6000 log | B | placeholder | a clutch incubates this long |
| `fauna.founders.littershredder.motor_respiration_per_s` | 0.001 | org-units/s per unit body at full cruise | [0, inf) | 0.00025–0.01 log | B | placeholder | paid motion |
| `fauna.founders.littershredder.step_up_m` | 0.25 | m | [0, inf) | 0.1–1 | B | measured | tallest riser stepped (rounded down to voxels) |
| `fauna.founders.littershredder.step_down_m` | 0.25 | m | [0, inf) | yes | C | measured | deepest drop stepped |
| `fauna.founders.littershredder.wall_climb_cost_factor` | 2.0 | x walking cost | [0, inf) |  | - | derived | motor cost of a second on a wall |
| `fauna.founders.littershredder.wade_height_fraction` | 0.25 | x current height | [0, inf) | 0–1 | B | placeholder | deepest water walked into |
| `fauna.founders.littershredder.drown_height_fraction` | 1.0 | x current height | [0, inf) |  | A | decision | water deeper than this is over it |
| `fauna.founders.littershredder.drown_after_s` | 60.0 | s | [0, inf) | 10–36000 log | - | placeholder | continuous time under water before it drowns |
| `fauna.founders.littershredder.adult_length_m` | 0.375 | m | (0, inf) | yes | N | decision, digest | adult body length (cruise is 1 BL/s) |
| `fauna.founders.littershredder.adult_width_m` | 0.125 | m | (0, inf) | yes | N | decision, digest | adult body width |
| `fauna.founders.littershredder.adult_height_m` | 0.125 | m | (0, inf) | yes | N | decision, digest | adult body height |
| `fauna.founders.littershredder.eye_height_fraction` | 0.8 | x body height | [0, inf) |  | - | placeholder, digest | the eye over the standing surface |
| `fauna.founders.littershredder.mouth_ceiling_fraction` | 1.33 | x body height | [0, inf) | 0.3–2.5 | - | placeholder, digest | the mouth band's ceiling |
| `fauna.founders.littershredder.contact_height_fraction` | 0.5 | x body height | [0, inf) |  | - | placeholder, digest | the contact receptors' height |
| `fauna.founders.littershredder.mouth_reach_length_fraction` | 0.25 | x body length | [0, inf) | yes | - | placeholder, digest | the mouth's reach ahead of the footprint |
| `fauna.founders.littershredder.yield_by_food.*` | 0.5, 0.5, 0.5, 0.5 | fraction | [0, 1] | 0.05–0.9 | - | placeholder | assimilation yield of one food class |
| `fauna.founders.littershredder.organ_structure_fraction` | 0.05 | fraction of body | [0, 1) |  | N | placeholder | sensor/organ share of structure |
| `fauna.founders.littershredder.sleep_upkeep` | 0.5 | x basal upkeep | [0, 1] | 0.2–1 | - | decision | upkeep while asleep |
| `fauna.founders.littershredder.wake_hunger` | 0.5 | hunger 0..1 | [0, 1] | 0.1–1 | - | placeholder | hunger at or above which it stays awake |
| `fauna.founders.littershredder.armour` | 0.0 | fraction | [0, 1] |  | - | placeholder | a hit is reduced by 1 - armour |
| `fauna.founders.littershredder.agility` | 0.3 | fraction | [0, 1] |  | - | placeholder | the dodge's species factor |
| `fauna.founders.littershredder.mass_scale` | 30.0 | x placeholder mass | (0, inf) |  | - | decision | the org-unit mass scale the body was built at |
| `fauna.founders.littershredder.hunter.strike_power` | (absent) | wound per hit | [0, inf) |  | - | placeholder | a hit adds power x (S_pred / S_prey) x (1 - armour) |
| `fauna.founders.littershredder.hunter.prey_max_ratio` | (absent) | x own body | [0, inf) |  | - | placeholder | the size window: prey at most this x its own |
| `fauna.founders.littershredder.hunter.strike_reach_length_fraction` | (absent) | x body length | [0, inf) |  | - | placeholder | the strike capsule's reach |
| `fauna.founders.littershredder.hunter.jaw_band_height_fraction` | (absent) | x body height | [0, inf) |  | - | placeholder | the jaw band's top |
| `fauna.founders.littershredder.hunter.windup_s` | (absent) | s | [0, inf) |  | - | placeholder | the crouch |
| `fauna.founders.littershredder.hunter.lunge_s` | (absent) | s | [0, inf) |  | - | placeholder | the lunge |
| `fauna.founders.littershredder.hunter.recovery_s` | (absent) | s | [0, inf) |  | - | placeholder | the recovery, at half speed and unable to strike |
| `fauna.founders.littershredder.hunter.strike_cost_s` | (absent) | s of awake upkeep | [0, inf) |  | - | placeholder | a strike's cost, hit or miss |
| `fauna.founders.littershredder.hunter.lure_power` | (absent) | x awake upkeep | [0, inf) |  | - | placeholder | a lit lure's cost |
| `fauna.founders.littershredder.hunter.rest_at` | (absent) | fraction of reserve cap | [0, 1] |  | - | placeholder | satiated: rests at this reserve |
| `fauna.founders.littershredder.hunter.torpor_upkeep` | (absent) | x basal upkeep | [0, inf) |  | - | placeholder | upkeep in the satiated rest |
| `fauna.founders.littershredder.hunter.wake_at` | (absent) | fraction of reserve cap | [0, 1] |  | - | placeholder | wakes from the rest below this |
| `fauna.founders.littershredder.counter.reach` | (absent) | m | [0, inf) |  | - | placeholder | counter-strike reach (unused seam) |
| `fauna.founders.littershredder.counter.damage` | (absent) | wound | [0, inf) |  | - | placeholder | counter-strike damage (unused seam) |
| `fauna.founders.littershredder.counter.cost` | (absent) | org-units | [0, inf) |  | - | placeholder | counter-strike cost (unused seam) |
| `fauna.founders.frondgrazer.core.maintenance_per_s` | 0.001 | org-units/s per unit body | [0, inf) | 0.00025–0.004 log | B | placeholder | basal upkeep, paid from reserve then body |
| `fauna.founders.frondgrazer.core.bite_per_s` | 0.06 | org-units/s | [0, inf) | 0.0002–0.004 log | C | placeholder | the hungry mouth's intake rate |
| `fauna.founders.frondgrazer.core.bite_half_stock` | 0.15 | org-units at the mouth | [0, inf) | 0.0001–0.05 log | C | tuned | K: a bite is want * E / (E + K) |
| `fauna.founders.frondgrazer.core.n_tissue` | 0.05 | mineral per org-unit | [0, inf) | 0.01–0.12 log | A | placeholder | tissue stoichiometry: a bite builds at most mineral / n_tissue |
| `fauna.founders.frondgrazer.core.body_max` | 1.5 | org-units | [0, inf) |  | D | placeholder | adult structure |
| `fauna.founders.frondgrazer.core.growth_max_per_s` | 0.000285 | org-units/s | [0, inf) | 0.000001–0.0001 log | B | derived | fastest structure grows from the reserve |
| `fauna.founders.frondgrazer.core.body_min` | 0.15 | org-units | (0, inf) | yes | B | placeholder | death threshold, and a newborn's body |
| `fauna.founders.frondgrazer.core.birth_body` | 0.8999999999999999 | org-units | [0, inf) | yes | B | placeholder | structure at which it can pay for a birth |
| `fauna.founders.frondgrazer.core.birth_cost` | 0.3 | org-units | [0, inf) | yes | B | placeholder | a gestation's package: the newborn's body_min plus its reserve |
| `fauna.founders.frondgrazer.core.reserve_cap` | 0.5 | fraction of body | [0, inf) | 0.2–1.5 | A | placeholder | ceiling on new intake into the reserve |
| `fauna.founders.frondgrazer.core.drown_depth_m` | 0.2 | m | [0, inf) |  | D | placeholder | standing water that kills the heuristic body and a clutch |
| `fauna.founders.frondgrazer.core.energy_density` | 2.0 | energy per org-unit | [0, inf) |  | N | placeholder | energy an introduced animal arrives with |
| `fauna.founders.frondgrazer.core.lifespan_median_s` | 14400.0 | s | [0, inf) | 3600–57600 log | B | derived | median lifespan (0: none) |
| `fauna.founders.frondgrazer.core.reproduction.surplus_floor` | 0.15 | org-units above the package | [0, inf) | yes | C | placeholder | reserve margin above the package that counts as surplus |
| `fauna.founders.frondgrazer.core.reproduction.surplus_hold_s` | 900.0 | s | [0, inf) | 60–3600 log | C | derived | the window a surplus is judged over |
| `fauna.founders.frondgrazer.core.reproduction.surplus_share` | 0.8 | fraction of the hold | [0, 1] |  | - | decision | share of the hold spent in surplus before breeding |
| `fauna.founders.frondgrazer.core.reproduction.gestation_s` | 180.0 | s | [0, inf) | 60–6000 log | D | placeholder | a gestation's length |
| `fauna.founders.frondgrazer.core.reproduction.gestation_loss_fraction` | 0.25 | fraction | [0, 1] | 0–1 | C | placeholder | organic respired when a gestation fails |
| `fauna.founders.frondgrazer.core.reproduction.birth_interval_s` | 1800.0 | s | [0, inf) | 300–10000 log | B | derived | refractory after a birth or a laying |
| `fauna.founders.frondgrazer.core.reproduction.clutch_size` | 0 | eggs | [0, inf) | yes | D | derived | eggs per clutch (an egg-layer's at least 1) |
| `fauna.founders.frondgrazer.core.reproduction.egg_organic` | 0.0 | org-units per egg | [0, inf) | yes | D | placeholder | one egg; at least the lineage's body_min |
| `fauna.founders.frondgrazer.core.reproduction.incubation_s` | 0.0 | s | [0, inf) | 60–6000 log | B | placeholder | a clutch incubates this long |
| `fauna.founders.frondgrazer.motor_respiration_per_s` | 0.001 | org-units/s per unit body at full cruise | [0, inf) | 0.00025–0.01 log | B | placeholder | paid motion |
| `fauna.founders.frondgrazer.step_up_m` | 0.375 | m | [0, inf) | 0.1–1 | B | measured | tallest riser stepped (rounded down to voxels) |
| `fauna.founders.frondgrazer.step_down_m` | 0.375 | m | [0, inf) | yes | C | measured | deepest drop stepped |
| `fauna.founders.frondgrazer.wall_climb_cost_factor` | 2.0 | x walking cost | [0, inf) |  | - | derived | motor cost of a second on a wall |
| `fauna.founders.frondgrazer.wade_height_fraction` | 0.5 | x current height | [0, inf) | 0–1 | B | placeholder | deepest water walked into |
| `fauna.founders.frondgrazer.drown_height_fraction` | 1.0 | x current height | [0, inf) |  | A | decision | water deeper than this is over it |
| `fauna.founders.frondgrazer.drown_after_s` | 60.0 | s | [0, inf) | 10–36000 log | - | placeholder | continuous time under water before it drowns |
| `fauna.founders.frondgrazer.adult_length_m` | 0.75 | m | (0, inf) | yes | N | decision, digest | adult body length (cruise is 1 BL/s) |
| `fauna.founders.frondgrazer.adult_width_m` | 0.375 | m | (0, inf) | yes | N | decision, digest | adult body width |
| `fauna.founders.frondgrazer.adult_height_m` | 0.375 | m | (0, inf) | yes | N | decision, digest | adult body height |
| `fauna.founders.frondgrazer.eye_height_fraction` | 0.8 | x body height | [0, inf) |  | - | placeholder, digest | the eye over the standing surface |
| `fauna.founders.frondgrazer.mouth_ceiling_fraction` | 1.33 | x body height | [0, inf) | 0.3–2.5 | - | placeholder, digest | the mouth band's ceiling |
| `fauna.founders.frondgrazer.contact_height_fraction` | 0.5 | x body height | [0, inf) |  | - | placeholder, digest | the contact receptors' height |
| `fauna.founders.frondgrazer.mouth_reach_length_fraction` | 0.25 | x body length | [0, inf) | yes | - | placeholder, digest | the mouth's reach ahead of the footprint |
| `fauna.founders.frondgrazer.yield_by_food.*` | 0.5, 0.5, 0.5, 0.5 | fraction | [0, 1] | 0.05–0.9 | - | placeholder | assimilation yield of one food class |
| `fauna.founders.frondgrazer.organ_structure_fraction` | 0.1 | fraction of body | [0, 1) |  | N | placeholder | sensor/organ share of structure |
| `fauna.founders.frondgrazer.sleep_upkeep` | 0.5 | x basal upkeep | [0, 1] | 0.2–1 | - | decision | upkeep while asleep |
| `fauna.founders.frondgrazer.wake_hunger` | 0.5 | hunger 0..1 | [0, 1] | 0.1–1 | - | placeholder | hunger at or above which it stays awake |
| `fauna.founders.frondgrazer.armour` | 0.25 | fraction | [0, 1] |  | - | placeholder | a hit is reduced by 1 - armour |
| `fauna.founders.frondgrazer.agility` | 0.5 | fraction | [0, 1] |  | - | placeholder | the dodge's species factor |
| `fauna.founders.frondgrazer.mass_scale` | 30.0 | x placeholder mass | (0, inf) |  | - | decision | the org-unit mass scale the body was built at |
| `fauna.founders.frondgrazer.hunter.strike_power` | (absent) | wound per hit | [0, inf) |  | - | placeholder | a hit adds power x (S_pred / S_prey) x (1 - armour) |
| `fauna.founders.frondgrazer.hunter.prey_max_ratio` | (absent) | x own body | [0, inf) |  | - | placeholder | the size window: prey at most this x its own |
| `fauna.founders.frondgrazer.hunter.strike_reach_length_fraction` | (absent) | x body length | [0, inf) |  | - | placeholder | the strike capsule's reach |
| `fauna.founders.frondgrazer.hunter.jaw_band_height_fraction` | (absent) | x body height | [0, inf) |  | - | placeholder | the jaw band's top |
| `fauna.founders.frondgrazer.hunter.windup_s` | (absent) | s | [0, inf) |  | - | placeholder | the crouch |
| `fauna.founders.frondgrazer.hunter.lunge_s` | (absent) | s | [0, inf) |  | - | placeholder | the lunge |
| `fauna.founders.frondgrazer.hunter.recovery_s` | (absent) | s | [0, inf) |  | - | placeholder | the recovery, at half speed and unable to strike |
| `fauna.founders.frondgrazer.hunter.strike_cost_s` | (absent) | s of awake upkeep | [0, inf) |  | - | placeholder | a strike's cost, hit or miss |
| `fauna.founders.frondgrazer.hunter.lure_power` | (absent) | x awake upkeep | [0, inf) |  | - | placeholder | a lit lure's cost |
| `fauna.founders.frondgrazer.hunter.rest_at` | (absent) | fraction of reserve cap | [0, 1] |  | - | placeholder | satiated: rests at this reserve |
| `fauna.founders.frondgrazer.hunter.torpor_upkeep` | (absent) | x basal upkeep | [0, inf) |  | - | placeholder | upkeep in the satiated rest |
| `fauna.founders.frondgrazer.hunter.wake_at` | (absent) | fraction of reserve cap | [0, 1] |  | - | placeholder | wakes from the rest below this |
| `fauna.founders.frondgrazer.counter.reach` | (absent) | m | [0, inf) |  | - | placeholder | counter-strike reach (unused seam) |
| `fauna.founders.frondgrazer.counter.damage` | (absent) | wound | [0, inf) |  | - | placeholder | counter-strike damage (unused seam) |
| `fauna.founders.frondgrazer.counter.cost` | (absent) | org-units | [0, inf) |  | - | placeholder | counter-strike cost (unused seam) |
| `fauna.founders.lanternjaw.core.maintenance_per_s` | 0.0002 | org-units/s per unit body | [0, inf) | 0.00025–0.004 log | B | placeholder | basal upkeep, paid from reserve then body |
| `fauna.founders.lanternjaw.core.bite_per_s` | 0.042 | org-units/s | [0, inf) | 0.0002–0.004 log | C | placeholder | the hungry mouth's intake rate |
| `fauna.founders.lanternjaw.core.bite_half_stock` | 0.105 | org-units at the mouth | [0, inf) | 0.0001–0.05 log | C | tuned | K: a bite is want * E / (E + K) |
| `fauna.founders.lanternjaw.core.n_tissue` | 0.02 | mineral per org-unit | [0, inf) | 0.01–0.12 log | A | placeholder | tissue stoichiometry: a bite builds at most mineral / n_tissue |
| `fauna.founders.lanternjaw.core.body_max` | 1.05 | org-units | [0, inf) |  | D | placeholder | adult structure |
| `fauna.founders.lanternjaw.core.growth_max_per_s` | 0.00007500000000000001 | org-units/s | [0, inf) | 0.000001–0.0001 log | B | derived | fastest structure grows from the reserve |
| `fauna.founders.lanternjaw.core.body_min` | 0.102 | org-units | (0, inf) | yes | B | placeholder | death threshold, and a newborn's body |
| `fauna.founders.lanternjaw.core.birth_body` | 0.63 | org-units | [0, inf) | yes | B | placeholder | structure at which it can pay for a birth |
| `fauna.founders.lanternjaw.core.birth_cost` | 0.15 | org-units | [0, inf) | yes | B | placeholder | a gestation's package: the newborn's body_min plus its reserve |
| `fauna.founders.lanternjaw.core.reserve_cap` | 1.0 | fraction of body | [0, inf) | 0.2–1.5 | A | placeholder | ceiling on new intake into the reserve |
| `fauna.founders.lanternjaw.core.drown_depth_m` | 0.2 | m | [0, inf) |  | D | placeholder | standing water that kills the heuristic body and a clutch |
| `fauna.founders.lanternjaw.core.energy_density` | 2.0 | energy per org-unit | [0, inf) |  | N | placeholder | energy an introduced animal arrives with |
| `fauna.founders.lanternjaw.core.lifespan_median_s` | 43200.0 | s | [0, inf) | 3600–57600 log | B | derived | median lifespan (0: none) |
| `fauna.founders.lanternjaw.core.reproduction.surplus_floor` | 0.375 | org-units above the package | [0, inf) | yes | C | placeholder | reserve margin above the package that counts as surplus |
| `fauna.founders.lanternjaw.core.reproduction.surplus_hold_s` | 900.0 | s | [0, inf) | 60–3600 log | C | derived | the window a surplus is judged over |
| `fauna.founders.lanternjaw.core.reproduction.surplus_share` | 0.8 | fraction of the hold | [0, 1] |  | - | decision | share of the hold spent in surplus before breeding |
| `fauna.founders.lanternjaw.core.reproduction.gestation_s` | 0.0 | s | [0, inf) | 60–6000 log | D | placeholder | a gestation's length |
| `fauna.founders.lanternjaw.core.reproduction.gestation_loss_fraction` | 0.25 | fraction | [0, 1] | 0–1 | C | placeholder | organic respired when a gestation fails |
| `fauna.founders.lanternjaw.core.reproduction.birth_interval_s` | 1800.0 | s | [0, inf) | 300–10000 log | B | derived | refractory after a birth or a laying |
| `fauna.founders.lanternjaw.core.reproduction.clutch_size` | 3 | eggs | [0, inf) | yes | D | derived | eggs per clutch (an egg-layer's at least 1) |
| `fauna.founders.lanternjaw.core.reproduction.egg_organic` | 0.15 | org-units per egg | [0, inf) | yes | D | placeholder | one egg; at least the lineage's body_min |
| `fauna.founders.lanternjaw.core.reproduction.incubation_s` | 0.0 | s | [0, inf) | 60–6000 log | B | placeholder | a clutch incubates this long |
| `fauna.founders.lanternjaw.motor_respiration_per_s` | 0.0002 | org-units/s per unit body at full cruise | [0, inf) | 0.00025–0.01 log | B | placeholder | paid motion |
| `fauna.founders.lanternjaw.step_up_m` | 0.45 | m | [0, inf) | 0.1–1 | B | measured | tallest riser stepped (rounded down to voxels) |
| `fauna.founders.lanternjaw.step_down_m` | 0.45 | m | [0, inf) | yes | C | measured | deepest drop stepped |
| `fauna.founders.lanternjaw.wall_climb_cost_factor` | 2.0 | x walking cost | [0, inf) |  | - | derived | motor cost of a second on a wall |
| `fauna.founders.lanternjaw.wade_height_fraction` | 0.5 | x current height | [0, inf) | 0–1 | B | placeholder | deepest water walked into |
| `fauna.founders.lanternjaw.drown_height_fraction` | 1.0 | x current height | [0, inf) |  | A | decision | water deeper than this is over it |
| `fauna.founders.lanternjaw.drown_after_s` | 60.0 | s | [0, inf) | 10–36000 log | - | placeholder | continuous time under water before it drowns |
| `fauna.founders.lanternjaw.adult_length_m` | 1.0 | m | (0, inf) | yes | N | decision, digest | adult body length (cruise is 1 BL/s) |
| `fauna.founders.lanternjaw.adult_width_m` | 0.25 | m | (0, inf) | yes | N | decision, digest | adult body width |
| `fauna.founders.lanternjaw.adult_height_m` | 0.45 | m | (0, inf) | yes | N | decision, digest | adult body height |
| `fauna.founders.lanternjaw.eye_height_fraction` | 0.8 | x body height | [0, inf) |  | - | placeholder, digest | the eye over the standing surface |
| `fauna.founders.lanternjaw.mouth_ceiling_fraction` | 1.33 | x body height | [0, inf) | 0.3–2.5 | - | placeholder, digest | the mouth band's ceiling |
| `fauna.founders.lanternjaw.contact_height_fraction` | 0.5 | x body height | [0, inf) |  | - | placeholder, digest | the contact receptors' height |
| `fauna.founders.lanternjaw.mouth_reach_length_fraction` | 0.25 | x body length | [0, inf) | yes | - | placeholder, digest | the mouth's reach ahead of the footprint |
| `fauna.founders.lanternjaw.yield_by_food.*` | 0.5, 0.5, 0.5, 0.5 | fraction | [0, 1] | 0.05–0.9 | - | placeholder | assimilation yield of one food class |
| `fauna.founders.lanternjaw.organ_structure_fraction` | 0.1 | fraction of body | [0, 1) |  | N | placeholder | sensor/organ share of structure |
| `fauna.founders.lanternjaw.sleep_upkeep` | 0.5 | x basal upkeep | [0, 1] | 0.2–1 | - | decision | upkeep while asleep |
| `fauna.founders.lanternjaw.wake_hunger` | 0.5 | hunger 0..1 | [0, 1] | 0.1–1 | - | placeholder | hunger at or above which it stays awake |
| `fauna.founders.lanternjaw.armour` | 0.0 | fraction | [0, 1] |  | - | placeholder | a hit is reduced by 1 - armour |
| `fauna.founders.lanternjaw.agility` | 0.0 | fraction | [0, 1] |  | - | placeholder | the dodge's species factor |
| `fauna.founders.lanternjaw.mass_scale` | 30.0 | x placeholder mass | (0, inf) |  | - | decision | the org-unit mass scale the body was built at |
| `fauna.founders.lanternjaw.hunter.strike_power` | 0.7 | wound per hit | [0, inf) |  | - | placeholder | a hit adds power x (S_pred / S_prey) x (1 - armour) |
| `fauna.founders.lanternjaw.hunter.prey_max_ratio` | 1.5 | x own body | [0, inf) |  | - | placeholder | the size window: prey at most this x its own |
| `fauna.founders.lanternjaw.hunter.strike_reach_length_fraction` | 0.6 | x body length | [0, inf) |  | - | placeholder | the strike capsule's reach |
| `fauna.founders.lanternjaw.hunter.jaw_band_height_fraction` | 0.6 | x body height | [0, inf) |  | - | placeholder | the jaw band's top |
| `fauna.founders.lanternjaw.hunter.windup_s` | 0.15 | s | [0, inf) |  | - | placeholder | the crouch |
| `fauna.founders.lanternjaw.hunter.lunge_s` | 0.25 | s | [0, inf) |  | - | placeholder | the lunge |
| `fauna.founders.lanternjaw.hunter.recovery_s` | 3.0 | s | [0, inf) |  | - | placeholder | the recovery, at half speed and unable to strike |
| `fauna.founders.lanternjaw.hunter.strike_cost_s` | 100.0 | s of awake upkeep | [0, inf) |  | - | placeholder | a strike's cost, hit or miss |
| `fauna.founders.lanternjaw.hunter.lure_power` | 1.0 | x awake upkeep | [0, inf) |  | - | placeholder | a lit lure's cost |
| `fauna.founders.lanternjaw.hunter.rest_at` | 0.9 | fraction of reserve cap | [0, 1] |  | - | placeholder | satiated: rests at this reserve |
| `fauna.founders.lanternjaw.hunter.torpor_upkeep` | 0.25 | x basal upkeep | [0, inf) |  | - | placeholder | upkeep in the satiated rest |
| `fauna.founders.lanternjaw.hunter.wake_at` | 0.5 | fraction of reserve cap | [0, 1] |  | - | placeholder | wakes from the rest below this |
| `fauna.founders.lanternjaw.counter.reach` | (absent) | m | [0, inf) |  | - | placeholder | counter-strike reach (unused seam) |
| `fauna.founders.lanternjaw.counter.damage` | (absent) | wound | [0, inf) |  | - | placeholder | counter-strike damage (unused seam) |
| `fauna.founders.lanternjaw.counter.cost` | (absent) | org-units | [0, inf) |  | - | placeholder | counter-strike cost (unused seam) |
| `fauna.hatch_chance_per_dusk` | 0.3333333333333333 | probability per dusk | [0, 1] |  | - | placeholder | a dormant egg past its footfall gate hatches at one dusk |
| `fauna.parasite_attach_radius_m` | 0.75 | m | [0, inf) | 0.25–2 | N | placeholder | host search radius from a hatching egg |
| `fauna.parasite_max_per_host` | 1 | count | [0, inf) | 0–4 | N | placeholder | riders one host carries (0: dormant eggs never hatch) |
| `fauna.parasite_drain_of_host_basal` | 0.05 | x host basal upkeep | [0, inf) | 0.01–0.5 log | N | placeholder | a rider's take from its host |
| `fauna.parasite_detach_body_fraction` | 0.24 | fraction of lineage body_max | (0, 1] | 0.15–0.5 | N | placeholder | a rider drops off at this size |

<!-- registry dump: end -->

</details>

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
