# Plant viability: soil water, grazing, seed bank and dispersal

Wrysk, 2026-09-23: the voxel world should be lush after a few hours and the desktop
terrarium fully colonised in about 24 h. Today plants mostly die and never spread. He
approved all three changes ("lets get all these changes in before another round") and
asked for a fan-out. Four packages run in parallel. Fable integrates.

## What was measured (scratch census, before L/N, default/small presets, 4 h)

- No drownings. Deaths are **starvation in full light**, because moisture is too low.
  - Soil drains to field capacity, 0.25 of pore space × porosity 0.35 ≈ 9 % water by
    volume. That is sand; loam holds ~25–30 %.
  - The species' moisture ramps are anchored near saturation, so drained ground reads
    dry: springturf μ ≈ 0.33 and velvetpad 0.12.
  - Without animals, all 35 springturf die between min 35 and 60 and never donate.
- Probe with the ramps capped to field capacity: 84 → 250 stands at 4 h. Bloomcrown
  stayed flat anyway (0 donors).
- Browsers:
  - They strip plants to zero. Nothing is out of reach at ground level.
  - There is no satiety: a full animal keeps biting and the surplus is respired, so about
    ⅓–½ of digestible intake is wasted.
  - They ate 37–60 % of plant production in the first half hour. The norm on land is
    ~10–20 %.
  - They breed about every 5 min when fed: 8 → 12–15 in 20–30 min, then extinct by
    90–120 min.
- Seeds:
  - A package lands uniformly within `hop` (0.25–0.75 m), on rock or occupied sites too.
  - A lone package can germinate only on the tick after it lands, because attrition
    drops it below a whole package.
  - Seeds live 600 s.
- Desk terrarium (wet ground), no animals: 86 → 312 at 2 h. Springturf (14 → 1) and
  bloomcrown (20 → 10) still starve.

## Shared rules (all four)

- **Base:** main at `40872d1`. Make your own worktree:
  `git worktree add .claude/worktrees/<name> -b <name> 40872d1`. Commit there with
  explicit paths. Fable merges.
- **Ownership:**
  - `soil-retention` owns `crates/cubarium-voxel` (materials, water, hydrate).
  - `plant-water` owns flora moisture, drinking, the establishment water gates and
    per-species water/economy numbers.
  - `grazing` owns `crates/cubarium-voxel-fauna` feeding, assimilation, growth and
    reproduction. It also owns the new flora refuge field and `withdraw_foliage`.
  - `seed-dispersal` owns the flora seed bank, propagation, dispersal and germination
    lottery.
  - `plant-water` and `seed-dispersal` both edit `cubarium-voxel-flora` `lib.rs` and
    `step.rs`, so stay inside your own functions and fields. Merge conflicts in the
    species constructors are Fable's.
- **Don't touch:**
  - fauna `senses.rs`, `controller.rs`, `manifest.rs` and the sim arena, because the
    unmerged `retrain` branch rewrites them;
  - the presenter (`stand.rs`, `animal.rs`, render), because package V is in flight;
  - the seeder counts in `crates/cubarium/src/voxel/habitat.rs`.
- **Tests first.** Write the tests listed for your package as a separate first commit
  (`Tests first: …`). They are function tests: a few ticks, each under 2 s. Run only the
  crates you touched (`cargo nextest run -p <crate>`).
- **Numbers.** Every number you choose goes in its field's doc comment with a one-line
  reason (house style). If the snapshot shape changes, bump that crate's schema; Fable
  reconciles to one bump per crate at merge.
- **Measurement tool:** `cargo build --release -p cubarium --example voxel_census`, then
  `target/release/examples/voxel_census <hours> preset=<small|default> seed=<n> [nofauna]`
  or `config=config/desktop/terrarium.toml`.
  - Columns include stands, `donors_<species>`, `fixed_in` and `consumed_organic`
    (cumulative; consumed/fixed is the herbivores' share) and `standing_foliage`.
  - Use multi-seed arms (seeds 1–4).
  - At most 6 census processes and `cargo -j 8` each: four packages share the machine.
- **Return:** ≤ 40 lines. List the commits, the numbers that decided things, and open
  questions for Wrysk. The commit messages are the report.

## W · soil-retention: drained soil holds loam-like water

- **Decision:** after drainage, soil keeps roughly 20–25 % of its volume in water, with
  roughly 10 % held below the wilting point.
  - At porosity 0.35: `field_capacity` ≈ 0.6–0.7 of pores, `Material::wilting_point`
    ≈ 0.25–0.3 (the hook exists with provisional numbers).
  - Rock keeps its crevice numbers unless you find a reason.
  - Change porosity only with a measured reason.
- **What else moves:**
  - shower infiltration and runoff (the drainable store gets smaller);
  - `hydrate` (worlds start at field capacity, so more water is locked in soil);
  - the closed budget (atmosphere + stores);
  - the founding lake gate;
  - the terrarium's standing water and groundwater stream;
  - soil evaporation;
  - water tick cost.
- **Authority:** pick the numbers inside that window. Adjust the world's water budget,
  the hydrate volume or shower volume so that the shipped presets still found lakes and
  the terrarium's water level reads as today (within a voxel after 2 h). If holding that
  needs a redesign of the water model, stop and report.
- **Tests:**
  1. A soil column drained from saturation settles at the new field capacity: retained
     volume = fc · pore · voxel volume.
  2. For every porous material, `0 < wilting_point < field_capacity < 1`.
  3. Existing water tests still pass. Replace any literal that encoded 0.25 with the
     material call.
- **Measure:** lake/stream level and stored water at 0, 30 and 120 min, before and after.
  Arms: small and default (seeds 1–4) and the terrarium config. Also the water ms/tick.

## F · plant-water: plants measure the water the ground offers

- **Decision:**
  - Per root cell, available water is `a = (p − wp)/(fc − wp)`, using that cell's own
    material, with `p` the pore fraction. Floor it at 0.
  - `a` is 1 at field capacity and above 1 in wetter ground, up to `(1−wp)/(fc−wp)` at
    saturation.
  - μ ramps over the root box's mean `a`, between the species' wilt and full thresholds
    on that scale. Rename the fields to say so.
  - Drinking never takes pore water below the wilting point.
  - Saturation and aeration gates stay on pore fraction.
- **Species scale:** upland plants are comfortable through the top half of the available
  water. Only wetland plants need ground wetter than drained.
  - Upland (bloomcrown, springturf, vaulttree): wilt ~0–0.1, full 0.5.
  - Stonecushion: wilt 0, full 0.2.
  - Damp-lovers (velvetpad, lanternberry, glowcap): wilt ~0.2–0.3, full ~0.9.
  - Wetland (umbrellafrond, siphonreed): wilt 1.0, full near saturation.
  - `establish_pore_min` moves to the same scale, in the same order.
- **Then viability, no fauna:**
  - Run default and small (seeds 1–4) and the terrarium config, 3 h each.
  - Run each twice: once on the provisional material numbers, and once on a local,
    uncommitted loam probe (Soil fc 0.65, wp 0.28). Report both.
  - Every upland species should have donors by ~2 h and not decline.
  - If bloomcrown (0 donors before) or any of vaulttree/lanternberry/siphonreed can't
    reach donor size in full light with full water, find which term fails: assimilation,
    maintenance/senescence, `light_half`, shedding, or the L size change. Fix that
    species' own numbers. Report before changing `v1_base`.
- **Tests:**
  1. `a` is 0 at wp and 1 at fc, and greater than 1 above fc, for soil and rock.
  2. μ is 0 at the species' wilt threshold and 1 at its full threshold.
  3. An upland stand on soil at field capacity has μ = 1.
  4. Drink leaves every root cell at or above wp.
  5. A wetland species on soil at field capacity has μ = 0.

## G · grazing: satiety, diminishing bites, a refuge, plant-time life history

- **Decisions:**
  1. **Satiety.** An animal never takes food it cannot store.
     - The bite scales with hunger (reserve short of its cap) and is capped by the room
       left in body + reserve after yield.
     - No digestible intake is respired as surplus.
     - A gut with a digestion rate is an allowed alternative if it fits this code
       better. Say why.
  2. **Diminishing bites.** `bite = want · E/(E + K)`, where `E` is the edible foliage
     the mouth reaches on that plant and `K` is set per species. A stripped plant gives
     crumbs, so moving on pays.
  3. **Refuge.** Add flora `SpeciesConfig::graze_refuge`.
     - The floor is `graze_refuge ×` the foliage the stand's wood carries.
     - `withdraw_foliage` never goes below the floor.
     - Every fauna reader of edible foliage (bite arrays, plan/feed, smell, the cone's
       edible reading) goes through **one** flora function that reports only foliage
       above the floor. A reader in `senses.rs` gets a one-line switch and nothing more.
     - Values: springturf and velvetpad 0.3 (basal buds), stonecushion 0.4, others 0.15.
       Lanternberry's browse line and the tall crowns keep their height refuge on top.
       Glowcap caps (shredder food) are your call; state it.
  4. **Life history on plant time**, for browsers and shredders.
     - Browsers: first birth no sooner than ~1 h after being born; births at least
       ~30 min apart.
     - Shredder clutches: the same in their own terms.
     - Implement it as a maximum structural growth rate (a juvenile cannot outgrow it
       however much it eats) plus a longer hold and interval. Derive the numbers from
       these targets.
- **Measure:**
  - Arms: coupled default, seeds 1–4, 2 h, before and after, plus one `nofauna` arm.
  - Read off: offtake (consumed/fixed after min 30; target ≤ ~25 %), browser and shredder
    counts (no more than 2× founders by 1 h), standing foliage against the `nofauna` arm,
    and when browsers go extinct.
  - The plants still starve on this base; the water packages join at merge. So read the
    fauna numbers, not the plant counts.
- **Trained policies stay as they are.** The next training round trains on these rules.
  Don't change observations or actions.
- **Tests:**
  1. A full animal (reserve at cap, body at max) takes nothing, and the plant keeps its
     foliage.
  2. A hungry animal's bite is capped by its room, and nothing from it is respired as
     surplus.
  3. The bite is `want · E/(E+K)`; it is half of `want` at `E = K`.
  4. `withdraw_foliage` never goes below the floor, and the reader reports 0 edible at
     the floor.
  5. A newborn fed at maximum intake reaches `birth_body` no sooner than the growth cap
     allows.
  6. Two births by one parent are at least the interval apart.

## S · seed-dispersal: a dormant seed bank and per-species dispersal

- **Decisions:**
  1. **Discrete seeds.**
     - The bank holds whole seeds per site, species and cohort. One seed funds one
       seedling (today's package).
     - Attrition kills whole seeds at a per-seed rate (deterministic draws), and the
       organic goes to litter as now.
     - A lone seed can therefore wait.
  2. **Dormancy lasting hours.** Lifetimes per species: most species 6–12 h, spores
     2–4 h. Rationale in the doc comments.
  3. **Throttled germination.**
     - Each banked site is checked on a slow, staggered clock (~30–60 s), plus a flush
       when a shower ends (the moisture cue).
     - The gates stay as they are. After merge they read F's available-water scale.
     - Report the ms/tick of the SeedBank and Propagate phases on the terrarium config,
       before and after. The growth must be small.
  4. **Dispersal mode per species** (a `Dispersal` enum, plus a clonal share):

     | species | mode | clonal |
     |---|---|---|
     | springturf | drop (hop 3) | runners, share ~0.5 |
     | velvetpad | spores → damp ground | runners |
     | stonecushion | wind | — |
     | bloomcrown | drop (animal vectors later) | — |
     | umbrellafrond | spores → wet ground | rhizomes |
     | glowcap | spores → its substrate | — |
     | vaulttree | wind (winged seed) | — |
     | lanternberry | drop beneath (fruit; seedporter later) | — |
     | siphonreed | water | rhizomes |

     What each mode does:
     - **Drop** is today's uniform kernel within `hop`.
     - **Wind** uses a fat-tailed distance. Most seeds land within a few hops, a few
       percent past 10 hops, capped at the world. Isotropic.
     - **Spores** land only on sites that pass the species' substrate or wetness gate
       inside a wide radius. This stands in for a spore rain that mostly falls where
       nothing grows.
     - **Water** lands on standing-water margins (`VoxelView::standing_depth_m`) inside a
       wide radius, downhill first.
     - **Runners/rhizomes** fund a daughter stand in a free neighbouring cell that passes
       the gates. The parent's reserve pays, and nothing is banked.
  5. D5 seed marks still draw every site with a cohort. Report how many sites hold a bank
     at 1 h and 3 h on the terrarium. If that is everywhere, it is a look question for
     Wrysk.
- **Measure:**
  - Arms: `nofauna` default and the terrarium config, seeds 1–2, 3 h.
  - Read off: stands per species, recruits by mode (stderr counters are fine), bank
    size and phase cost.
- **Tests:**
  1. A lone seed banked on a site whose gates fail germinates at the first check after
     the gate opens, hours later.
  2. Attrition removes whole seeds and conserves organic into litter.
  3. The wind kernel over many draws: most seeds land within 3 hops, and at least a few
     percent past 10.
  4. Spores land only on sites that pass the gate.
  5. Water landings fall only at standing-water margins.
  6. A runner daughter is adjacent, paid for from the reserve, and conserved.
  7. A site is not tested between checks, except on a shower flush.

## Integration (Fable)

1. Merge in the order W, F, G, S, with one schema bump per crate.
2. Run the full suite once.
3. Run the coupled census with and without fauna: default seeds 1–4 and the terrarium,
   4 h.
4. Report to Wrysk.
5. Then run the 24 h terrarium colonisation study.

The `retrain` branch must merge main after this lands, before the next training round.
