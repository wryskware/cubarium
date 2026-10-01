<!-- Appendix to design/7_Research/ecology-parameter-audit-2026-09-28.md: the harness inventory's working notes, written by an audit worker at 3ccc6daf. Rows are in parameters.tsv (subsystem = harness). Evidence, not decisions. -->

# Harness inventory notes (world founding, acceptance, arenas, landscapes, ES score, ecology search)

Audit pinned at 3ccc6daf, working tree read 2026-09-28. Rows: `harness.tsv` (74). No runs were made; all numbers below are arithmetic on code constants.
Scope note: `crates/cubarium/src/voxel/placement.rs` is CPU/core pinning only (no ecology). `voxel/mod.rs` only wires `habitat::FOUNDER_COUNTS`, `FaunaConfig::default()`, `FloraConfig::for_voxel_size` (mod.rs:963-975); no ecological literal lives there.

## 1. Rules in plain words

**Founding (live and training worlds).** `found_a_habitat` = lake gate -> pre-roll (opening shower, lake rest) -> `seed_pre_rolled` -> `accept`, redrawn up to 8 times (habitat.rs; found.rs:32-37).
- Producers: for each of 8 species, `count = min(cap, pool_faces * cell_area * per_m2)`, planted in colonies of `patch_size`, 1.5 m apart, each at wood = [0.30,0.42,0.55,0.70,0.85] x wood_max cycled. Glowcap gets a 0.4 org log first. Vines: `seed_vine_founders`.
- Founders: 8 littershredders + 8 browsers, both `StartingStores::HUNGRY` (body 0.5 x body_max, reserve 0). A shredder gets a 0.2 org litter tile under it. A browser is placed only on a face whose mouth band reaches edible foliage and whose walkable component holds >= 2 stands.
- Acceptance: every wanted founder placed, none stranded, each occupied component holds reachable edible stock >= N x founders x adult upkeep, N = 1 h; upkeep/h = maintenance_per_s x body_max x 3600 (browser 0.18, shredder 0.045). Shredder also needs a living litter-shedding stand rooted in the component. This is a standing-stock check, not a supply check.
- The live host uses only Rust defaults (`FaunaConfig::default()`, `FloraConfig::for_voxel_size`); the ecology vector is applied only by the training harness (`found_world`).

**Frozen arena (phase-one ES).** 32x16x12 voxels @0.25 m (8x3 m), all-soil ground, births off, `DayMode::Off`, flora built `.without_graze_refuge().without_defence()`, no water/plant leg (arena.rs:561-566, 626-630). 6 food sites (Stage A) or 2 patches (Stage B). Start is off food, 2-4 columns (0.5-1.0 m) from the nearest food. Food per site: shredder litter/carrion/cap 0.2 org, browser 0.06 org of foliage (seedling or rosette). Body drawn per episode: structure U[body_min, body_max], reserve 0 (D11). Horizon 1200 ticks (60 s) Stage A, 2400 (120 s) Stage B.

**Frozen landscape (P5-C).** A whole founded world (small/default/wide presets), frozen; browsers 4800 ticks (240 s) fully static, shredders 24000 ticks with replayed litter production. Acting bodies start on uniform random standable faces (not on food), sized as in D11. Fauna config is `FaunaConfig::default()` (no ecology vector); flora config is the live one (defence and refuge present).

**Score.** `score = (I - M)/ref + 0.25 * survived`.
- `I` = sum of `feedback.intake` = `placed` = min(assimilated, reserve room), assimilated = min(yield x bite, mineral/n_tissue) (step.rs:1417-1447; driver.rs:1042).
- `M` = sum of `feedback.motor_respiration`.
- `ref` = body_max of the lineage (browser 0.05, shredder 0.0125).
- `survived` = ticks alive / ticks possible.
- Live ES: same formula per body over a 10-min window, then per-world z-score, then per-candidate mean (trainer.rs:128).

**Ecology search.** CMA-ES over 12 log-multipliers (6 shared + 3 x 2 per-lineage), each candidate scored on freshly founded `small` still-water worlds, brains fixed. Penalty = bound + peak + lively + 10 per lost lineage/species:
- bound = mean over the second half of (30 - n)/30 below or (n - 80)/80 above;
- peak = (max n - 120)/120;
- lively = (0.30 - browser moving share)/0.30.

## 2. Does the score ever reward walking once a parked animal is sated? No.

Facts (all from code):
1. Bite size: `want = bite_per_s * 0.25 s * effort * hunger`, hunger = 1 - reserve/(reserve_cap * body) (step.rs:402-408, 1248). `sated_bite = min(want * E/(E+K), room/yield)` (415-428).
2. Intake booked is `min(yield*bite, mineral-funded, room)` (assimilate 1431-1447). A full reserve therefore books 0.
3. Reserve room is created only by (a) maintenance respiration, (b) motor respiration, (c) growth moving reserve to structure (cap rises by 0.5 per unit built, reserve falls by 1: 1.5 room per unit), (d) gestation/birth cost.
4. Hence total intake `I <= R0 + m + M + 1.5*G (+birth)`, R0 = starting room, m = maintenance paid, G = growth built. Then `I - M <= R0 + m + 1.5*G`. **M cancels: the ceiling of the score's numerator does not depend on how far the body walks.** A walker at best ties a parker that has food at its mouth; if the walk leaves any room unfilled (search time, no stock, defence/depleted stand) it loses.
5. The only ways walking wins: the parking spot has no reachable food (Stage A start 0.5-1 m off food; landscape start random face), or the local stand runs out (browser landscape plants are frozen, `E/(E+K)` shrinks the bite). Parking on food scores the ceiling. The score prices motor exactly, but never prices the risk or cost of not finding food, because a starving parker's body loss is not subtracted (intake counts only inflow).
6. Ceiling of a body that starts FULL and stays parked on food: `0.25 + maint*body_max*T/ref` = 0.31 (60 s), 0.37 (120 s), 0.49 (240 s) browser; shredder 24000 ticks 1.45. P2-B measured 0.3100 and 0.3700 (arena.rs:317-321) - the same numbers.
7. The P2-C fix (start hungry) buys R0: browser landscape mean body 0.0275 -> reserve room 0.01375 = 0.275 ref; plus growth 9.5e-6/s x 240 s x 1.5 = 0.0034 = 0.07 ref. So reachable reward for finding food in a 240 s episode is ~0.35 ref, against a motor cost of 0.00133 ref per metre. That gives a strong walking signal ONLY while the animal starts hungry and off food. The live world's sated adult has R0 ~ 0, and its numerator ceiling is m/ref (0.24 per 240 s at body_max), reached by standing still on a stand.
8. A parked animal that never finds food, hungry start, still scores 0.25 (0.0275 body dies of dieback only after ~800 s; survives the 240 s horizon), so "park and starve" costs 0.25 + 0 vs the ~0.6 of a walker that fills up: the arena does punish parking off food, the live window does not punish parking on food.
9. Live ES z-scores within a world, so a parker on food and a walker that refills equal each other; noise then decides. Sleep (diurnal browser, 0.5x upkeep, 33 % of a 2 h day) further shrinks `m` and sanctions stillness; the arena has no night.
10. Coupling that makes it worse: the tuned ecology vector (2026-09-28) scales `maintenance_per_s` x0.25 but not `motor_respiration_per_s` (ecology.rs:374), so cruise costs 4x basal and sated animals need 4x less food.

## 3. Derived quantities

Browser (body_max 0.05, maint 0.001/s, yield 0.5, mineral-limited effective ~0.4 per the brief, bite_per_s 0.002, K 0.005, reserve_cap 0.5, growth 9.5e-6/s, motor 0.001 x body per s at full cruise):
- basal upkeep at body_max 5.0e-5 org/s; reserve cap 0.025 = 500 s of upkeep.
- max assimilation at hunger 1: bite x yield = 0.002 x 0.5 = 1.0e-3 org/s = 0.020 ref/s = 20x upkeep at body_max (36x motor at mean body). Fill of 0.01375 room takes ~14 s.
- cost of walking: 0.00133 body (= ref) per metre; 1 BL/s = 0.75 m/s.
- Stage A total food 6 x 0.06 = 0.36 org = 7.2 body_max (3.6 after yield): 60 s cap of intake is room-limited (0.25-0.5 ref), not food-limited.
Shredder (body_max 0.0125, bite 0.0005, K 0.00125): same ratios; 6 tiles x 0.2 = 1.2 org = 96 body-masses of litter (one tile = 16).
Densities (browsers only, 8 founders):
- small training world 160x24 @0.125 = 60 m2: 0.133 /m2 = 133,000 /km2 = 11,000 x Damuth (12/km2 at 15 kg).
- desktop 256x48 @0.125 = 192 m2: 0.042 /m2 = 41,700 /km2 = 3,500 x Damuth.
- ecology-search band 30-80 animals per small world = 0.5-1.33 /m2 = 40,000-110,000 x Damuth (both lineages counted).
- animal mass 0.042/m2 x 15 kg = 0.63 kg/m2 vs herbivore biomass at Damuth ~ 1.8e-4 kg/m2.
Plants at founding (max): sum of caps 604 stands = 3.1/m2 at 192 m2 (10/m2 at 60 m2); actual = min(cap, pool x rate).
Time: 6 h search horizon = 750 real-eq days at K=3000; a 4 h median lifespan is 14 upkeep turnovers (tau 1000 s) vs 70-290 real; tuned 2 h = 7.
Share of parameters ES can evolve: 9 shared multipliers (12 search dims) vs the world's several hundred ecological numbers (see the other TSVs; my file lists 74 harness rows: 3 A, 6 B, 35 C, 14 D, 16 N; 9 are the ES rows). All species share one foliage_rate/senescence/defence multiplier: the search can shift levels, not the balance between species. Two of the nine ended pinned at their box floor in the last tuning (senescence 0.25, maintenance 0.25).

## 4. Top findings (ranked)

1. **The score's numerator ceiling ignores walking** (section 2). A sated animal is paid `0.25 + m/ref` for standing on food, walking adds nothing; evidence step.rs:415-428,1248,1443-1447 + score.rs:82. Fix candidates (design, not mine to choose): score net body change instead of inflow, score reproduction/energy at the horizon, or start founders FULL and require intake > maintenance via food distance.
2. **Live founders and D11 bodies start with reserve 0** (lib.rs:1879, driver.rs:487): the training signal to walk is an artefact of R0; the same animal after it is full has none.
3. **Frozen training differs from the live world in 4 ways at once**: no graze refuge, no defence, no night, no ecology vector (arena.rs:561-566,626-630; landscape.rs:154). A brain trained frozen has never seen a stand that defends, a stand that regrows, or a night that ends the search.
4. **`maintenance_per_s x0.25` without `motor_respiration_per_s` (ecology.rs:374)**: motor/basal goes 1:1 -> 4:1 (real mammals 1.5-2.5:1); K_bio drops to ~540, below the 1,000-10,000 window.
5. **Lifespan tuned to 2 h** (x0.503): 7 upkeep turnovers vs 70-290 real; the search treats "shorter lives" as the lever (Spearman +0.80) because there is no other mortality; that compensates for an oversized founder density.
6. **Founder density 3,500-11,000 x Damuth** and the band 30-80 (40,000-110,000 x): the search's "bounded" target is itself a physical outlier, so it optimises toward a crowded terrarium.
7. **Food per body is not one unit system**: shredder tile 0.2 = 16 body masses; browser stand 0.06 = 1.2 (13x); browser stand of 0.06 org = 24 kg at the browser scale vs a 0.1-0.3 kg shrub.
8. **Acceptance bar N = 1 h** is a stock at t0 (3.6 body masses of edible foliage per founder), not a supply; it says nothing about whether regrowth can feed 8 browsers.
9. Ecology vector exposes 9 scalars: no per-species balance, no bite_per_s, reserve_cap, growth, motor, senses, soil/water/weather. `moving_min` 0.30 target with a 33 % sleeping night caps the possible share at 0.67.

## 5. INLINE / hard-coded literals worth converting

- score.rs:82 motor weight 1.0; survival 0.25; driver.rs:1042 `.max(0.0)` clamp of intake.
- driver.rs:487-490 D11 body draw U[newborn,1], reserve 0.
- arena.rs:573-575 day Off; :626-630 no refuge/no defence; :757-780 start ring squared 4..=16; heading +/-90 deg; :761 selection by nearest food; :208 bystander clearance 9; pond 4x2 cells 0.2 m deep (POND_FILL 0.8, arena.rs:71).
- habitat.rs:74-83 PRODUCERS/GLOWCAPS table; :53 wood fractions; :57 log 0.4; :90 spacing 1.5; :96 shade 0.6; :101 step 0.5; :121 litter 0.2; :130 N = 1.
- landscape.rs:64 production replay only for shredder; :338-360 start-face draw; :49,53 horizons.
- search.rs:56-66 band; :160-191 projection clamps; :446-453 box penalty x10; window.rs:27 moving 0.1 BL/s and the half-minute rule (:369).
- ecology.rs ranges 0.25-4 (yield 0.25-2); fauna_config multiplies yield for all four food classes together (:370-372).

## 6. Code shape (owner's extra request)

Headline: the harness has ~3 copies of founding, 2 copies of per-body scoring, and 2 fixture families that diverge from the live world; a single `found` + `BodyMeter` + `Fixture` config would remove ~300 lines and close finding 3.
- Founding is repeated: the same `found_a_habitat` call with the same `Flora::new(FloraConfig::for_voxel_size(..)), Fauna::new(FaunaConfig::default())` closure is written in live/world.rs:92-110, landscape.rs:141-160, cubarium/src/voxel/mod.rs:963-975, found.rs:496,531 (tests) and examples/water_census.rs:74; the host has a second path at mod.rs:1283 (`pre_roll` on resume) and examples call `habitat::seed`/`pre_roll` directly (voxel_census.rs:191,341).
- Scoring/tracking is twinned: `Tracked` + feedback-delta logic at driver.rs:702-802 (frozen) and window.rs:119-143,284-290 (live) do the same cadence-aware `now - prev` accumulation; both then call `ScoreComponents::of` (driver.rs:1118, window.rs:407). The frozen trainer (trainer.rs 2448 lines, commands.rs 2067) and live trainer (live/trainer.rs 512 + search.rs 847) share only `pooled_scores`/`ascend` (live/trainer.rs:128,154).
- Constants are copied, not shared: LITTER_MINERAL_FRACTION 0.02 / LITTER_ENERGY_DENSITY 2.0 / litter 0.2 in arena.rs:73,90-92 and habitat.rs:121-125; `FaunaConfig::default()` hard-wired in arena.rs:857,880 and landscape.rs:154,166,828.
- Fixture switches are buried literals (arena.rs:561-566 day off, :626-630 no refuge/defence, births off in `populate`) rather than one config the live world also reads.
- Legacy/dead: `Species::Frondgrazer` introductions kept "reachable from the g stdin line" (habitat.rs:13,105-106); `habitat::seed` is now only an example/test wrapper (habitat.rs:299-301); `scene.rs` authored test scene (274 lines); protocol-history strings in task.rs:115-188 (p2b/p2c/p3a/p3b/p3d/p5 revisions kept only to refuse old centres); ecology-search "unified mode" not used by default (search.rs:19-21).
- Refactor 1: one `FoundingSpec {world cfg, EcologyParams, counts}` -> `found()` used by host, live world, landscape and arena-from-world (about -80 lines, and lets frozen episodes take the ecology vector).
- Refactor 2: one `BodyMeter` (intake, motor, walked, alive, cadence delta) shared by driver.rs and window.rs (about -120 lines); score components then come from one place.
- Refactor 3: an `EpisodeFixture` struct (day mode, refuge, defence, births, start stores, horizon, food amounts) that arenas, landscapes and the live window all fill, replacing scattered consts (about -100 lines net, and it turns findings 2-3 into parameters); retire the legacy wrappers and protocol strings after Wrysk confirms (about -150 more).

## 7. Unresolved

- Whether the landscape episode's day clock is at daylight for the whole 240 s (preset default is Cycle from 08:00; browsers would sleep at night). I did not trace the static schedule's clock.
- Effective browser yield (0.4 in the brief vs 0.5 in the table) is a fauna-worker number (mineral limit); I used 0.5 for assimilation ceilings, so real ceilings are ~20 % lower.
- `crates/cubarium-search/src/{movement,population,plant_budget,factorial,calibrate,params}.rs` are outside my scope and were not read; they may hold further ecological targets.
- Provenance for `body_reference` = body_max is "schema normaliser" in comments; I graded A as a normalisation, not a physiological claim.
