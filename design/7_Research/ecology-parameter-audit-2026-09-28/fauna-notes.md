<!-- Appendix to design/7_Research/ecology-parameter-audit-2026-09-28.md: the fauna inventory's working notes, written by an audit worker at 3ccc6daf. Rows are in parameters.tsv (subsystem = fauna). Evidence, not decisions. -->

# Fauna inventory: notes (audit at 3ccc6daf, working tree clean)

Scope: `crates/cubarium-voxel-fauna/src/` (lib, body, step, controller, senses, encounter,
manifest, pose). Rows: `fauna.tsv` (90 rows: A 9, B 20, C 20, D 7, N 34). encounter.rs and
pose.rs hold only geometry numerics; none of their numbers rows. Paths below are relative to
that directory unless given in full.

## Rules in plain words

Symbols: B body (structure), R reserve, b_max, b_min, m = maintenance_per_s, q =
motor_respiration_per_s, β = bite_per_s, K = bite_half_stock, y = yield for the food class,
n_a = animal n_tissue, n_f = mineral fraction of the food, c = reserve_cap, g =
growth_max_per_s, DT = 0.05 s (20 Hz), P = controller period 0.25 s.

- **Upkeep** (step.rs:344). Every tick: `m · s · B · DT` is respired from R, then from B once R
  is empty. s = 1 awake, SLEEP_UPKEEP 0.5 asleep. Energy leaves as heat at the body's own
  density. The mineral in the respired matter goes to the gut.
- **Motor** (step.rs:872). Founders only: `q · B · (|v| + r·|ω|)/v_cruise · DT`, paid on what
  is *requested*, blocked or not. r is the footprint radius (width/2), ω ≤ yaw_cap 2 rad/s,
  v_cruise = 1 BL/s. On a wall: `q · B · effort · 2.0 · DT` while moving at 0.5 × cruise.
- **Hunger**: h = 1 − R/(c·B), clamped to [0,1] (step.rs:402).
- **Sated bite** (step.rs:415, 1248). Once per controller period, if feed ≥ 0.05:
  want = β · P · feed · h; bite = min(want · E/(E+K), (c·B − R)/y), where E is the edible stock
  at the mouth (foliage in the mouth band of one stand, or the richest detritus pool).
- **Assimilation** (step.rs:1417). The tissue built is A = min(y · bite, bite · n_f / n_a).
  It all goes to R. The (1−y)·bite share goes to the gut. The rest, (y·bite − A), is
  respired. Kept mineral is n_a·A; the rest goes to the gut. The browser eats foliage with
  n_f = 0.02 against n_a = 0.05, so A = 0.4·bite on every bite: the mineral budget binds
  before y = 0.5 does.
- **Gut**: emptied onto the face as a litter deposit every 30 s of age (GUT_VOID_S). No
  retention time and no capacity, so nothing limits intake except h and β.
- **Growth** (step.rs:376): ΔB = g · DT · min(R/(c·B),1), taken out of R until b_max.
- **Reproduction** (step.rs:1586). A body is *eligible* when B ≥ birth_body and R ≥ package +
  surplus_floor. After `surplus_hold_s` of unbroken eligibility:
  - browser: an escrow takes birth_cost from R in equal instalments over gestation_s. The
    newborn is b_min, with the rest of the escrow as its reserve.
  - blind: if the face has any litter, a clutch of `clutch_size × egg_organic` is paid at once.
    It hatches after incubation_s. It is lost to carrion if water over it exceeds the
    lineage's `core.drown_depth_m` (0.2 m).
  - Then `birth_interval_s` of refractory.
  - A failed gestation respires 0.25 of the escrow.
- **Death** (step.rs:1967), in this order: starved (B < b_min); drowned (water over the feet
  above drown_height_fraction × height, 1.0, for 60 s continuous); aged (age ≥ the lifespan
  drawn from U[0.8,1.2]·median, and for founders ×U[0.5,1.0)). The whole body becomes carrion.
- **Sleep** (step.rs:321). A founder sleeps when it is out of phase (diurnal: daylight ≤ 0.3;
  nocturnal: ≥ 0.3), h < 0.5, it is not on a wall and it is not submerged. Asleep it pays
  0.5×, is not sampled, and does not move or feed.
- **Senses** (arena and live habitat, when a SenseField exists):
  - chem cue: every 0.5 s a tile emits min((litter+carrion)/0.05, 1)·0.5. The cue decays with
    a 2 s half-life and mixes 0.4 with its 4 neighbours on the same layer.
  - taste = min(E/0.05,1)/(that+1). It is at most 0.5.
  - The browser cone is 45 rays over 2 m. A foliage cell counts if it has any edible foliage,
    so the cone gives no amount.
- **Legacy heuristic species** (founder: None): crop every tick when reach ≥ one bite, else
  step 1 face/s toward the best face within 8 voxels. The live world runs founders; these
  core fields (reach, climb, wade 0.05 m, drown 0.2 m, step 1 s, sense 8) matter only there.
  One exception: `core.drown_depth_m` also decides clutch loss.

## Lineage fact sheets

**Browser** (frondgrazer = browser founder; diurnal; live birth; diet: vascular foliage)
- Adult 0.75 × 0.375 × 0.375 m; implied ~20–50 kg.
- Structure: b_max 0.05, b_min 0.005 (newborn, 10 %), birth_body 0.03, birth_cost 0.01.
- Reserve cap 0.5·B (0.025 at b_max).
- Rates: m 0.001, q 0.001, β 0.002, K 0.005, g 9.5e-6.
- Food and tissue: yield 0.5 (effective 0.4, mineral), n_a 0.05, energy density 2.0.
- Reproduction: floor 0.005, hold 900 s, gestation 180 s, loss 0.25, interval 1800 s.
- Life and water: lifespan median 4 h. Step 0.375 m up and down; no wall climbing; wades 0.5 H;
  drowns at 1.0 H after 60 s.
- Senses and geometry: eye 0.8 H; mouth band 0–0.5 m (1.33 H); reach 0.19 m; cone 2 m, 45 rays.
- Motion and control: cruise 0.75 m/s; yaw 2 rad/s; organ share 0.10 (inert).
- Eats: every Photo species' edible foliage in its band.
- Is eaten by: nothing. It dies into carrion, which the shredder eats.

**Blind** (littershredder; nocturnal; eggs; diet: litter, carrion, glowcap cap)
- Adult 0.375 × 0.125 × 0.125 m; implied ~0.5–2 kg.
- Structure: b_max 0.0125, b_min 0.003125 (hatchling, 25 %), birth_body = b_max. Reserve cap
  0.00625.
- Rates: m 0.001, q 0.001, β 0.0005, K 0.00125, g 3.4e-6.
- Food and tissue: yield 0.5 on all foods, n_a 0.02 (never binds on litter at 0.02).
- Reproduction: 1 egg of 0.004; floor 0.000625; hold 900 s; interval 1800 s; incubation 300 s.
- Life and water: lifespan 4 h. Steps 0.25 m and climbs walls at 0.5 × cruise and 2× cost.
  Wades 0.25 H; drowns at 1.0 H after 60 s.
- Senses and control: mouth band 0–0.17 m; chem + taste + contact; cruise 0.375 m/s; organ
  share 0.05 (inert).
- Eats: detritus (litter, carrion) and fungal cap tissue.
- Is eaten by: nothing.

## Derived quantities (arithmetic from the code values; `python3` check run)

Basal upkeep at b_max = m·b_max: browser 0.001·0.05 = **5e-5 /s**; blind 0.001·0.0125 = **1.25e-5 /s**.

1. **Maximum assimilation ÷ basal** (h = 1, feed = 1, E ≫ K; the rate is β because β·P is taken once per P):
   - browser: 0.4·0.002 = 8e-4 → **16×** (organic intake ÷ basal 40×).
   - blind: 0.5·0.0005 = 2.5e-4 → **20×**, if litter carries ≥ 0.02 mineral; otherwise mineral binds.
   - The real ceiling is ~2–4× maintenance, so the model is 4–10× too high.
   - Balance at rest: intake = upkeep at h\* = 5e-5/8e-4 = **0.0625** (browser) and 0.05 (blind).
     A parked browser at a full plant holds its reserve at 94 % of the cap.
2. **Motor cost ÷ basal**:
   - Full cruise: q/m = **1.0** for both.
   - Maximum turn in place: r·ω/v = 0.1875·2/0.75 = **0.5** (browser) and 0.0625·2/0.375 = **0.33** (blind).
   - The sigmoid gives effort 0.5 at logit 0. Resting needs logit < −2.94.
   - The ES "founder.maintenance_per_s" scale does **not** move q. The es3 tuned file (maintenance
     ×0.25) therefore makes full cruise **4× upkeep**.
3. **Reserve at cap in seconds of upkeep** (0.025 and 0.00625):

   | state | browser | blind |
   |---|---|---|
   | rest | 500 s | 500 s |
   | cruise (basal + motor, 2×) | 250 s | 250 s |
   | asleep (0.5×) | 1000 s | 1000 s |
   | max turn | 333 s | 375 s |

   - A sleeper wakes at h = 0.5 after **500 s (8.3 min)**, however long the night is.
   - At K_bio 3000, 500 s is about 17 real days: a plausible fasting reserve.
4. **Cost per metre** at cruise (motor only):
   - browser: 0.001·0.05/0.75 = **6.7e-5 org/m** = **1.33e-3 body/m**.
   - blind: 0.001·0.0125/0.375 = **3.3e-5 org/m** = **2.67e-3 body/m**.
   - Real browser: COT 4.5 J/kg/m ÷ 7.5 MJ/kg = 6e-7 body/m. The model is **~2,200× the real
     value**, which is K_bio applied to distance walked on the K = 1 clock.
   - A full reserve walks 375 m (motor only) or **187 m** (with basal) for the browser, and
     187 m / **94 m** for the blind.
5. **Birth to first offspring**:
   - browser: (0.03−0.005)/9.5e-6 = 2632 s, + 900 hold + 180 gestation = **3712 s (62 min)**.
     Steady-fed eligibility needs B ≥ 0.0312, because at B = 0.03 the cap is 0.015 = the
     surplus reserve. That adds ~130 s.
   - blind: (0.0125−0.003125)/3.4e-6 = 2757 s + 900 = **3657 s to lay**, and the egg hatches at
     **3957 s**, so an egg-to-egg generation takes 66 min.
6. **Offspring per lifespan**:
   - browser: cycle 900+180+1800 = 2880 s; floor((L−3712)/2880)+1 = **3 / 4 / 5** at
     L = 11520 / 14400 / 17280 s.
   - blind: cycle 2700 s, **3 / 4 / 6** eggs.
   - The es3 tuned lifespan ×0.5 (7200 s median) leaves the browser **1–2** offspring. Its
     first birth at 3712 s falls within a 5760–8640 s life.
7. **Breeding-eligibility margin**:
   - browser: package 0.01 + floor 0.005 = 0.015 = **60 % of cap at b_max** (100 % at
     birth_body). Margin 0.01. It resets after **200 s** at rest, or **100 s = 75 m** at cruise,
     without food.
   - blind: 0.004 + 0.000625 = 0.004625 = **74 % of cap**. Margin 0.001625. It resets after
     **130 s** at rest, or **65 s = 24 m** at cruise.
   - The 900 s hold therefore demands 900 s of near-continuous feeding. A blind walk of 24 m
     without food restarts the clock.

Clock checks (K = τ_real/τ_sim, reference K_bio ≈ 3000):

| quantity | real | K | vs K_bio |
|---|---|---|---|
| gestation | 130 d | 62,000 | 21× fast |
| maturity | 1–2 y | 12–24k | 4–8× fast |
| lifespan (wild median 2–5 y) | 2–5 y | 4,400–11,000 | about right |
| birth cycle | ~1 y | 11,000 | 3.7× fast |
| day/night | — | 12 | a night is ~250× longer metabolically than a real one |
| gut void | 1–2 h | ~180 | — |
| cruise, cone, drowning | — | 1 | — |

## Top findings (ranked by consequence for "browsers won't walk")

1. **Walking is priced on the metabolic clock, distance on the real clock.**
   - 1 m costs 1.33e-3 of a browser's body, ~2,200× the real value (see derived 4).
   - Its eye sees 2 m (manifest.rs:616) and a full reserve walks 187 m.
   - Meanwhile the intake cap is 16× upkeep (see 2), so any local plant with E ≳ K covers upkeep.
   - Walking pays only when the local E ≪ K = 0.005, about 6 % of a springturf's edible stock
     (0.084 per lib.rs:1118).
   - Couples: motor_respiration_per_s (body.rs:394), cone_range_m, bite_half_stock, and the
     flora foliage stocks.
2. **Intake ceiling 16–20× upkeep, no gut capacity** (lib.rs:1116, body.rs:326, step.rs:296).
   - The real value is 2–4×.
   - With intake ∝ hunger (step.rs:1248), a parked animal balances at h\* = 6 %. Moving never
     raises its intake.
   - A plant (or its regrowth) that supplies 1/16 of the browser's hungry rate supports it
     indefinitely.
3. **Plant and animal mass units disagree ~1000×** (lib.rs:1109–1122 comment; body.rs:406).
   - A springturf's edible foliage (0.084 org) is 1.7 adult browser bodies. Real turf leaf mass
     ÷ a 20 kg browser is ~1e-3.
   - At the browser's need (5e-5 ÷ 0.4 = 1.25e-4 org/s) one turf's standing stock lasts 672 s,
     ≈ 23 real days at K 3000. The flora worker should confirm the crown numbers.
   - The same unit clash shows in **POOL_BULK_DENSITY 80 org/m³** (senses.rs:97): 30–80× denser
     than the animals built from the same units (browser ~1 org/m³, shredder ~2.7).
   - The shredder is 4.5× denser than the browser: org ratio 0.25 against volume ratio 0.056.
4. **The ES upkeep lever mis-couples motion** (ecology.rs:374 scales maintenance only).
   - The es3 tuned vector drove maintenance to its **floor 0.25**, which makes walking 4× upkeep.
   - `founder.yield_by_food` is **inert above ×0.8 for the browser**: the mineral budget caps A at
     0.4·bite. es3's ×0.946 did nothing for it.
   - ES exposes 3 of ~35 fauna ecological numbers: maintenance, lifespan, yield, plus
     surplus_floor.
5. **Gestation 180 s is 21–29× short** on K_bio (lib.rs:831). The browser's life history runs
   ~4–20× faster than its metabolism (maturity 4–8×, gestation 21×, cycle 3.7×).
6. **Night vs reserve.**
   - A 2 h day gives a night of tens of minutes (abiotic worker has the curve). Asleep, a full
     reserve hits the wake threshold in 500 s (SLEEP_UPKEEP body.rs:109 × reserve_cap, WAKE_HUNGER
     body.rs:112).
   - So diurnal browsers cannot sleep through a night. They wake hungry and forage in the dark
     unless fed to the cap at dusk.
   - K_day 12 against K_bio 3000 is a 250× mismatch.
7. **The blind lineage is out of scale in several places:**
   - egg 0.004 = 32 % of the adult, clutch_size 1 (real 15–60 eggs), lib.rs:861–862.
   - n_tissue equals its food's, so a litter diet has no nutrient limit (body.rs:332).
   - litter yield 0.5 against a real 0.05–0.3 (body.rs:115). The same constant also gives
     carrion 0.5 against a real 0.8–0.9.
   - drowns in 60 s, where arthropods survive hours (body.rs:107).
   - motor = basal, 3–8× cheap for an arthropod (body.rs:358).
   - clutch loss uses the absolute 0.2 m `core.drown_depth_m` (step.rs:1852), while the body
     drowns at 0.125 m.
8. **Taste saturates at E = 0.05** (M_EMIT, body.rs:80 reused for foliage at body.rs:1655).
   - The browser cannot taste a difference above one body-mass of edible foliage.
   - The teacher feed gates (controller.rs:509, :627) refuse food at E < 0.0125 / 0.0167, which
     is 2.5–3.3× K.
   - The chem plume clears the blind teacher's 0.02 gate only within **2 cells** of a saturated
     tile. Simulated steady state: 0.67 / 0.24 / 0.052 / 0.012 at 0 / 1 / 2 / 3 cells.

## INLINE literals (easy to miss)

- lib.rs:1426 lifespan U[0.8,1.2]·median; lib.rs:1428 arriving ×U[0.5,1.0).
- step.rs:1156 climb pace 0.5 × cruise.
- step.rs:1805 and body.rs:1858: laying needs litter > 0.
- step.rs:1852 and :1864: clutch loss depth = `core.drown_depth_m` (0.2 m absolute).
- step.rs:951 and :845: one bite per controller period.
- step.rs:1248: want linear in hunger.
- step.rs:390: growth linear in fullness.
- step.rs:328: wake rules (hunger, on a wall, submerged).
- body.rs:492: size ∝ (B/b_max)^(1/3).
- body.rs:1655–1658: taste curve min(E/M_EMIT,1)/(c+sat).
- senses.rs:280: sources = litter + carrion only.
- senses.rs:426: emission min(d/M_EMIT,1)·UPDATE_S.
- controller.rs:121: sigmoid forward/feed (logit 0 → 0.5 effort).
- controller.rs:504/670: blocked creep 0.1; controller.rs:509/627: feed gates 0.2/0.25.
- controller.rs:542–550: chem gates 0.02/±0.05.
- controller.rs:540/552/654/657/668: turn efforts 0.8/0.8/0.8/0.8/0.6.
- controller.rs:651: sector gate 0.02; controller.rs:664: flip every 3 samples.

## Exposure summary

- TOML: none. No config file sets a fauna number; `config/**` has no fauna table.
- ES-only (training), via `crates/cubarium-search/src/es/voxel/live/ecology.rs` PARAMS:
  - maintenance_per_s ×[0.25,4], per-lineage
  - lifespan_median_s ×[0.25,4], per-lineage
  - yield_by_food ×[0.25,2], per-lineage
  - reproduction.surplus_floor ×[0.25,4], shared
- Everything else is STRUCT/CONST/INLINE Rust.
- Manifest-digest numbers (body dims, anchor fractions, cruise, yaw cap, cone, taste resistances,
  tunings, pool density) cannot change without invalidating trained policies.

## Inert or stale numbers

- Inert:
  - `organ_structure_fraction`: `sensor_structure` is read nowhere.
  - manifest `light_reference`, `proximity_range_m`, `contact_binary`: digest only.
  - `Activity::TWILIGHT`: no crepuscular lineage.
  - Legacy core fields (reach, climb, wade_depth_m, step_period_s, sense_radius) for founders.
- `design/backlog.md` §1 is **stale** for fauna:
  - it lists browser dims 0.375/0.1875/0.1875 and shredder 0.19/0.0625 (code 0.75/0.375/0.375 and
    0.375/0.125/0.125).
  - `climb_m` 0.25/0.125 (code step_up/down 0.375/0.25).
  - cruise 0.375/0.19 m/s (code 0.75/0.375).
  - surplus_hold_s 120 and birth_interval_s 300 (code 900/1800).
  - Missing from §1 entirely: sleep_upkeep, wake_hunger, lifespan_median_s, drown_after_s,
    wall_climb_cost_factor, bite_half_stock, growth_max_per_s, organ fraction, controller period,
    yaw cap, cone range, cue half-life/diffusion.

## Code shape

1. **Share of the code.** `src/` holds 16,096 lines:
   - logic 6,894 (43 %)
   - doc comments 2,721 + line comments 355 (19 %)
   - in-file and `*_tests.rs` tests 5,576 (35 %)
   - blank 550

   `tests/` and `examples/` add another 10,630 lines. The doc prose is heaviest in lib.rs
   (849 doc lines against 1,435 of logic) and body.rs (522 against 1,198).
2. **Duplicated logic.**
   - Seconds-to-ticks is written 5×: lib.rs:881, lib.rs:1150, body.rs:455,
     manifest.rs:772, and inline at step.rs:302.
   - splitmix64 appears 3× in the crate (step.rs:101–127, lib.rs:1416–1424,
     controller.rs:362–368), plus the flora's own copy (step.rs:98 says so).
   - The pro-rata move of organic, mineral and energy is written 4×: respire step.rs:245,
     gestate :1690, fail_gestation :1764, lay :1813.
   - A newborn body is built 2× (step.rs:1730, :1926).
   - The sated-bite rule appears twice: step.rs:415 and a hand copy in `crop` at step.rs:1361.
   - There are 3 finite/nonnegative validators: lib.rs:910, :1172, :1274.
   - `Manifest::blind`/`browser` are near-duplicates (manifest.rs:531–621). BLIND_TUNINGS equals
     BROWSER_TUNINGS (:451–467), and `anchors()` copies the physiology (:504).
3. **Dead or legacy paths still carried.**
   - The whole heuristic species path: `sense`/`plan_for` step.rs:559–735, `faces_in_column`
     :749, the heuristic branch of `act` :798–829, `crop` :1352, `toward`/`distance` :1482–1544,
     `steppable` lib.rs:2407, and the Rng domains.
   - The live world runs founders only.
   - SpeciesConfig fields the founder path never reads: reach, climb, wade_depth_m,
     step_period_s, sense_radius, and yield_fraction (founders use yield_by_food). birth_cost is
     unused for eggs. body.rs:341–343 says "Unused by the founder path".
   - `Species` has one variant. `FaunaConfig.frondgrazer` duplicates `founders[Browser].core`
     (body.rs:391). Hatchlings are stamped `Species::Frondgrazer` (step.rs:1928).
   - Inert: `organ_structure_fraction`, the manifest fields light_reference, proximity_range_m and
     contact_binary, and `Activity::TWILIGHT`.
4. **Where parameters live.** Seven places:
   - SpeciesConfig struct (lib.rs:1113, body.rs:324)
   - FounderPhysiology struct (body.rs:321)
   - Reproduction consts (lib.rs:827, :854)
   - loose consts (body.rs:80–118, lib.rs:514/516/1403, step.rs:296, senses.rs:76–97,
     manifest.rs:63/438–483, controller.rs:315–342)
   - manifest literals (manifest.rs:552–616)
   - inline literals (see INLINE above)
   - a hand-wired ES layer (ecology.rs:45–124, 358–377)

   One registry could replace them: a serde `Lineage` table (TOML), one per lineage, in which
   each field is declared once with (name, value, unit, range, provenance, evolvable). From that
   table the code would derive validation, the ES PARAMS vector, the backlog §1 listing, and the
   manifest anchors. The digest fields would be flagged in the same table.
5. **Refactors, rough net change:**
   - (a) Retire the heuristic species and make the frondgrazer a founder: about −450 logic lines,
     plus its tests.
   - (b) One `Lineage` record merging SpeciesConfig, FounderPhysiology, Reproduction and the
     consts, with a declarative registry that drives validate, ES and TOML: about −150, and
     every number becomes evolvable.
   - (c) One `move_fraction(from, amount) -> Taken` helper plus one `spawn_juvenile`: about −80.
   - (d) One `secs_to_ticks` and one shared splitmix in cubarium-voxel: about −50.
   - (e) Build the manifest from the lineage table, with shared tunings: about −45.
   - Doc prose could also shrink by about a third if the design history moved to the handoff docs
     it cites.

## Unresolved

- Litter's live mineral fraction is set by flora (senescence, dung, resorption). If it is below
  0.02 the blind's mineral budget binds and its effective yield is < 0.5. Not checked here.
- Night length in seconds: the abiotic worker has the daylight curve. The 500 s wake figure
  holds regardless.
- Whether the harness score rewards intake above upkeep: harness worker (sated_bite is
  step.rs:415, founder_feed step.rs:1237).
