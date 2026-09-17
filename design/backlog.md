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
| voxel flora config (`FloraConfig`: `shade_k`, `decomposition`, `wood_decomposition`, `litter_energy_cap`, `initial_mineral`; `SpeciesConfig` per species: `alpha`, `wood_max`, `reserve_cap`, `maintenance`, `senescence`, `foliage_rate`, `wood_rate`, `build`, `dieback`, `alive_min`, `donor_min`, `donor_reserve_floor`, `propagule_rate`, `propagule_split`, `energy_density`, `reserve_share`, `reflush_below`, `assimilation`, `nutrient_half`, `nutrient_draw_max`, `light_half`, `rooting_depth`, `rooting_radius`, `wilt_pore`, `sat_pore`, `transpiration_m3_per_s`, `establish_pore_min`, `establish_light_min`, `drown_depth_m`, `hop`, `crown_height_voxels`, `crown_radius_voxels`, `n_tissue`, `seed_attrition_per_s`, `seed_max_age_s`, `saturated_pore`, `stress_rate_per_s`, `relax_rate_per_s`, `establish_saturated_max`) | `FloraConfig::default()` in code; no TOML table yet (voxel round 2) | restart | Every value is an untuned placeholder. `bloomcrown.drown_depth_m` was 0.0 and is now 0.05 — a wrong placeholder, not a tuned one: any free water at all killed the species everywhere it rained. Still known-wrong: `shade_k` 1.5 lets one full crown attenuate the light under it by only 9 %, so a canopy never closes; `umbrellafrond.wilt_pore` 0.3 and `establish_pore_min` 0.45 sit **above** soil's own `field_capacity` (0.25 of pore capacity), which is where every soil voxel settles under any rain the soil can absorb, so the wet species has `μ = 0` and no site to establish on anywhere that is not ponded. `maintenance`/`dieback`/`reserve_cap` put death hours away, so no short test reaches it at the defaults. Two meanings changed in voxel round 3b without any value changing: `propagule_rate` is the **gross rate a donor saves at, one recipient's worth per tick, into `Stand::parcel`** — not a per-recipient rate multiplied by the recipient count. A stand over `donor_min` asks for `propagule_rate · dt` every tick, is funded from its reserve above `donor_reserve_floor · reserve_cap · wood`, pays construction (`c_g`) on what it funds, and saves the rest; when the parcel holds one whole package (`alive_min / w_frac` = 0.05 net, 0.06 of reserve) that package lands on **one** drawn support face inside `hop`, occupied or not, never its own site, and the remainder keeps saving. At the placeholders 300 s of a donor's entire funded output buys one package, wherever `hop` reaches, so raising the rate raises only what a donor *asks* for — and `seed_cohorts_max` is the seed bank's **age resolution** (the arrival-bin width is `seed_max_age_s / seed_cohorts_max`) rather than a cohort count to enforce. Also known-wrong and not yet fixed: `shade_k` 1.5's 9 % single-crown attenuation is a current model choice, not evidence of a closed canopy or of a finite intercepted-light budget, and consumer carrying capacity must not be derived from total producer income until it is revisited. |
| voxel flora `SpeciesConfig::n_tissue` (ships **0.02** for both species) | `SpeciesConfig::v1_base()` in code (voxel round 3) | restart | Mineral nutrient per unit of organic matter a species builds. Growing `ΔO` draws `n_tissue · ΔO` from the site's mineral pool, and the pool caps income through `mineral / n_tissue`. An untuned placeholder: nobody has measured what tissue stoichiometry this world wants. **Corrected in round 3b (Astra R4.3):** the claim that 0.02 "makes the cap bite" was wrong arithmetic. At the defaults the stock cap `N / n_tissue` allows `50 · N` of income per tick while the `nutrient_draw_max` rate cap allows `f_max · N · dt` = `0.0005 · N`, so the rate cap is tighter by five orders of magnitude and the stock cap **cannot be the binding one for any positive `N`**; `n_tissue` sets the stoichiometry of the draw and the mineral density of tissue, not the ceiling on income. Two more statements of scope, neither of them a knob: `Stand::mineral` is an **inventory and not a reusable internal reserve** — income reads the *site's* pool through all three limits, so a mineral-rich stand on a bare pool fixes exactly zero at full light and moisture and burns reserve to stand still, and fixing that means separating carbon fixation and maintenance from mineral-funded construction rather than moving a number; and `FloraConfig::initial_mineral` is applied **lazily**, when a site first gets a `Ground`, so it provisions previously unrepresented ground and colonization imports booked mineral into the world. A fertility comparison wants a fixed per-site inventory laid down at creation instead, so that the total does not grow with how far the plants spread. |
| voxel flora seed bank: `SpeciesConfig::seed_attrition_per_s` (ships **0.001** /s both species), `seed_max_age_s` (ships **600** s both species) and `seed_cohorts_max` (ships **4** both species) | `SpeciesConfig::v1_base()` in code (voxel round 3) | restart | How fast a dormant seed cohort bleeds into the site's litter, and how old a cohort may get before the rest of it falls whole. Untuned placeholders, and the same number for both species on purpose: nothing has measured a seed-longevity difference between them, so a difference here would be invented. Together with `propagule_rate` they set the equilibrium bank a fed site holds, which is what decides whether a species can ever re-establish. `seed_cohorts_max` is how many cohorts of one species a site may hold before a landing merges its two oldest into one at the older of their two ages: a bound by construction, added by package J because the age merge alone bounds nothing (package I's pulsing donor stacked one cohort per pulse, with `seed_max_age_s` as the only ceiling at about 6,000 per site). 4 is a placeholder for "a few", which is what `Ground::seeds`' own doc always claimed. **Replaced in round 3b (Astra R4.1):** the old rule merged cohorts whose ages were within one tick and kept the *younger* age, so `seed_max_age_s` measured the time since the bank's last delivery and arbitrarily small continuing arrivals retained old material for ever; the cohort cap then bounded the `Vec` by sweeping nearly every old deposit into one bucket that could kill much younger material at the next expiry. A cohort is now an **arrival bin** whose age runs from `bin_start_tick` and never decreases, `seed_cohorts_max` is the bank's **age resolution** — the bin width is `seed_max_age_s / seed_cohorts_max`, 150 s and 3,000 ticks at the placeholders, derived and not a knob of its own — and the count bound `seed_cohorts_max + 1` follows by construction. Visible effect at the placeholders: a continuously fed bank sawtooths, releasing a 150 s window of material to litter at once when its oldest bin turns 600 s, where before it released nothing at all. A smaller value is coarser and releases in bigger steps; a larger one is finer. |
| voxel flora root-zone aeration: `SpeciesConfig::saturated_pore` (ships **0.95** both species), `stress_rate_per_s` (**0.2** /s bloomcrown, **0.01** /s umbrellafrond), `relax_rate_per_s` (**0.02** /s bloomcrown, **0.05** /s umbrellafrond), `establish_saturated_max` (**0.25** bloomcrown, **1.0** umbrellafrond) | `SpeciesConfig::bloomcrown()` / `umbrellafrond()` in code (voxel round 3) | restart | `saturated_pore` is the pore fraction at which a root voxel counts as having no air left; `establish_saturated_max` does double duty as the saturated root fraction above which a seed cohort of that species will not germinate **and** as the tolerance its waterlogging stress measures from, because a site a species may germinate on is a site it does not stress on; and the two rates are how fast `Stand::aeration_stress` closes the gap to the level that tolerance implies — `target = (f − establish_saturated_max) / (1 − establish_saturated_max)` clamped to `0..=1`, with income multiplied by `1 - stress`. Untuned placeholders whose only justification is the *direction* the sketch asks for — the sun producer is shut out of a quarter-waterlogged site and stresses ten times faster than it recovers; the wet producer has no aeration bound at all. Package J replaced package I's increment rule `stress += rate·dt·f − relax·dt·(1−f)`, which did not depend on `stress` and so made the pair of rates a **threshold** rather than a strength: every saturated fraction but the knife-edge `f* = relax / (rate + relax)` ramped to 0 or to 1, and `f*` was 0.0909 for bloomcrown — two saturated voxels of an eighteen-voxel root box pinned it at stress 1 forever, which is what stopped package I's experiment. First-order relaxation toward a target is a slope: nine of eighteen voxels is now a bloomcrown stress of exactly 1/3 and an income of 2/3 (`crates/cubarium-voxel-flora/tests/round3.rs`, `a_half_saturated_root_box_settles_at_an_interior_stress`). The number most likely to be wrong is now `establish_saturated_max`, because it alone sets *where* a species starts paying and how steeply; the rates only set how fast it gets there, and umbrellafrond's ceiling of 1.0 makes both of its own rates inert. **The trait contract, stated in round 3b (Astra R4.6):** one number doing double duty — the germination ceiling and the adult stress tolerance — is an *assumption*, because "can germinate here" does not imply "pays no stress here as an adult"; they are one number because nothing has measured them apart, and a role whose seedlings and adults differ needs them separated (two distinct hypotheses: a germination ceiling below the adult tolerance, or an adult response with an explicit subunit maximum or a funded cost of tolerance). Umbrellafrond's 1.0 makes it explicitly **saturation-immune**, which is a wetland-producer proxy and *not* the biosphere's moist-but-aerated understory role — that niche needs its own preset. Do not lower 1.0 slightly as a repair: for every tolerance below 1 a wholly saturated root box targets stress 1 and eventually takes all of that species' assimilation again, which is the failure package I measured. |
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
