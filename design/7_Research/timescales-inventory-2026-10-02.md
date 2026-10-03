---
design_status: exploration
last_reviewed: 2026-10-02
decision_refs: []
---

# Time scales: what the world runs on today (2026-10-02)

Evidence, not a decision (`design/7_Research/`). Brief:
`design/handoffs/timescales-inventory-2026-10-02.md`. Code read at main `65043f6a`. Nothing
here recommends anything.

**Sources and conventions.**
- Parameter values are the registry dump `design/parameters.md` (generated from the code
  defaults; paths `fauna.founders.<lineage>.…`, `flora.<species>.…`, `world.day.…`).
- All animal stocks already include `mass_scale` = 30 (every lineage's
  `fauna.founders.*.mass_scale`).
- "Live defaults" = code defaults + the live preset TOMLs. No trained-ecology vector has
  landed in the live presets: `design/handoffs/live-defaults-curio-2026-10-02.md` is still an
  open brief, and `config/desktop/terrarium.toml` carries only founding counts and flags. The
  C1/C2 genomes in `config/training/curiosity-2026-10-02/` are training-only.
- Org-units (ou) are the sim's organic unit. 1 sim day = `world.day.length_min` = 120 min.
- "Arithmetic" rows are computed from the listed parameters with the rules cited; they are
  not runs. §6 is the one run.

## 1. The clock

### 1.1 Modes

| mode | day length | night fraction | source |
|---|---|---|---|
| `cycle` (default) | `length_min` = 120 min of sim time | 1 − `daylight` = 0.33 → sun down 39.6 min; measured full night (daylight 0) ≈ 34 min, twilight ≈ 4 min each side | `weather.rs:213-230` (`DayClock::default`), `:234-249`; measured in `canopy-night-loss-2026-10-01.md` "Day clock" (full night min 58–92) |
| `local` | 86,400 s, wall clock | with lat/lon: the date's NOAA sunrise/sunset, clamped to daylight 0.05–0.95; without: `civil_daylight` 0.5 → 12 h night | `weather.rs:234-238`, `:257-296` |
| `off` | `length_min` only as the climate's "day" unit | none: `life_daylight` ≡ 1, everyone awake, no diurnal temperature; picture holds `look` | `weather.rs:155-158`, `:1597-1603` |

Daylight is a smoothstep of solar elevation over ±`twilight_deg` 6° (`weather.rs:200-202`);
noon elevation 70°. Sim time runs at 1× wall time at `TICK_HZ` 20 (`DT` 0.05 s).

### 1.2 What each world uses

| world | clock | source |
|---|---|---|
| desktop (`config/desktop/terrarium.toml`, deployed by `config/hosts/voidwell.env` RUN_ARGS) | cycle, 120 min, daylight 0.67, start 08:00 | `terrarium.toml:26-30` |
| desktop voxel (`config/desktop/voxel.toml`) | same | `voxel.toml:30-34` |
| voidrunner (`config/voidrunner/terrarium.toml`) | no `[world.day]` table → code default = cycle 120 / 0.67 / 08:00 | file; `weather.rs:213-230` |
| tachyon (`config/tachyon/voxel.toml`, installed by `scripts/tachyon-install.sh:74`) | cycle, 120 min, 0.67, 08:00 | `voxel.toml:36-40` |
| co-training / curiosity gate (terrarium worlds) | the terrarium TOML's clock: cycle 120 / 0.67 | `design/cotraining-plan-2026-10-01.md` §"Result" ("8 common seeds × 1 h on the terrarium") |
| frozen training arenas | default cycle clock, start hour drawn per layout | `crates/cubarium-voxel-sim/src/arena.rs:610-616` |
| most unit-test fixtures | `DayMode::Off` | e.g. `cubarium-voxel-fauna/tests/founder_tick.rs:26` |

### 1.3 What night changes in the sim today

| what | rule at night | source |
|---|---|---|
| photosynthesis | `assimilation × lit foliage × daylight × μ × limits` → **0** at daylight 0 (linear in daylight through twilight) | `flora step.rs:2496-2504` |
| plant maintenance + senescence | × `plant_night_scale` = `d + (1−d)·plant_night_factor`; factor **0.05** (Wrysk 2026-09-29) | `weather.rs:187-196`, `:1608-1611`; `flora step.rs:2576-2582` |
| plant maintenance vs temperature | `upkeep_q10` 2.0 around 26 °C; diurnal swing ±3 °C (`tropical_jungle`) → night upkeep ×~0.81 at the trough | `flora step.rs:2273-2283`; `parameters.md` `flora.upkeep_q10`, `world.climate.diurnal_c` |
| plant growth from stores at night | maintenance is paid from reserve (income 0). The reserve **reflush** of foliage is scaled by `flush_light` = daylight, so **no foliage is built from reserve in the dark**; wood/foliage growth needs income | `flora step.rs:2628-2657` |
| nectar, fruit | made only while `daylight > 0` | `flora step.rs:2673-2690` |
| seedling establishment | a photosynthetic species does not establish at `daylight ≤ 0` | `flora step.rs:3137-3146` (`establishes`), called from `seeds.rs:392`, `:805` |
| saprotroph (glowcap) | not night-scaled (factor 1); income from substrate + dark seep, independent of light | `flora step.rs:2576-2582`, `:2606-2625` |
| animal circadian | a lineage not in its hour **sleeps** unless hunger ≥ `wake_hunger` 0.5, on a wall, flying, submerged, mid-strike, alert or (pack) away from den | `fauna step.rs:434-500` (`circadian`) |
| day line | diurnal awake at daylight > `fauna.life.day_line` 0.3; nocturnal awake below it | `fauna lib.rs:567-577`, `knobs.rs:177` |
| sleep metabolic multiplier | `sleep_upkeep` **0.5** × basal, all nine lineages; a hunter's satiated rest uses `torpor_upkeep` **0.25** | `fauna body.rs:103`, `step.rs:529-539`; `parameters.md` |
| movement and feeding while asleep | none: controller not consulted, no moves, no bites | `fauna lib.rs:541-546` (`State::Asleep`) |
| awareness while asleep | dodge awareness 0 (`awareness_asleep`) → an asleep prey cannot dodge | `parameters.md` `fauna.predation.awareness_asleep`; `predation.rs:143-157` |
| flyers | a sleepy bellwing is grounded (drops to a perch) | `fauna step.rs:1281-1283` |
| lanternjaw dormant eggs | hatch only on the **dusk tick** (daylight first falls below 0.3), when distinct prey heard in the last clock-day ≥ the egg's threshold (2–10), no adult lanternjaw within 4 m, with chance 1/3 per dusk; viability log-uniform 6–24 h | `fauna step.rs:147-153`, `:2867-2949`; `predation.rs:177-191`; `parameters.md` `fauna.hatch_chance_per_dusk`, `fauna.predation.egg_*` |
| animal temperature | **no rule** reads temperature for animals | grep of `cubarium-voxel-fauna/src/step.rs`: no temperature term |

**Rhythms per lineage** (`fauna body.rs` placeholders; a derived lineage inherits its base's
`active` via `..base` unless set):

| lineage | rhythm | source |
|---|---|---|
| littershredder | nocturnal | `body.rs:868` |
| frondgrazer | diurnal | `body.rs:927` |
| lanternjaw | nocturnal | `body.rs:1009` |
| capgnawer | nocturnal (inherits shredder) | `body.rs:1047-1090` |
| loftstrider | diurnal (inherits grazer) | `body.rs:1091-1148` |
| ripple-snail | nocturnal (inherits shredder, `..shredder`) | `body.rs:1287-1330` |
| bellwing | diurnal | `body.rs:1177` |
| seedporter | diurnal | `body.rs:1223` |
| chorister | diurnal | `body.rs:1270` |

`Crepuscular` exists but no lineage uses it (`parameters.md` `fauna.life.twilight_lo`: "unused
today"). §6 confirms the rhythms from measured sleep by phase.

### 1.4 Moonlight or night ambient light

**No model term.** `life_daylight` is the sun's smoothstep only (`weather.rs:1597-1603`);
photosynthesis is linear in it. The only moon is in the GPU renderer: `MOON` = 0.3 of the sun's
strength as **picture light** (`crates/cubarium-gpu/src/weather.rs:21`, `:321-342`). Nothing in
flora or fauna reads it.

## 2. Animals at live defaults (adult, `body_max`)

Rules used: upkeep `maintenance_per_s × body` (`fauna step.rs:539`); paid from reserve, then
from the body (`withdraw`, `step.rs:262-300`); death when `body < body_min`
(`step.rs:3047`). So from a full reserve with no food: reserve lasts `reserve_cap /
maintenance` s; the body then decays as `e^(−m t)` to `body_min`, `ln(body_max/body_min)/m` s.
Asleep at 0.5× both terms double, but **sleep only holds while hunger < 0.5**, i.e. at most
`reserve_cap/maintenance` s from full (half the reserve at half rate). Max intake =
`bite_per_s × yield` (yield 0.5 for every diet food; capgnawer's litter 0.25) — the hungry
rate; a bite asks in proportion to hunger (`step.rs:640+`), and the browser's mineral budget can
bind first (effective 0.4, audit §1.2).

### 2.1 Metabolism and starvation

| lineage | `maintenance_per_s` (/s per body) | body_max (ou) | upkeep (ou/s) | reserve at cap (ou) | reserve lasts, awake | full → starved, awake | full → starved, asleep (if it stayed asleep) | max sleep from full before the hunger wake | max assimilated intake (ou/s) | intake ÷ upkeep | feeding h per 1 h asleep (at max intake, standing) | motor, % body per body length |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| littershredder | 0.001 | 0.375 | 3.75e-4 | 0.188 | 0.14 h | 0.52 h | 1.05 h | 0.14 h | 7.5e-3 | 20 | 0.026 | 0.10 |
| frondgrazer | 0.001 | 1.5 | 1.5e-3 | 0.75 | 0.14 h | 0.78 h | 1.56 h | 0.14 h | 3.0e-2 | 20 | 0.026 | 0.10 |
| lanternjaw | 0.0002 | 1.05 | 2.1e-4 | 1.05 | 1.39 h | 4.63 h | 9.25 h | 1.39 h | 2.1e-2 | 100 | 0.005 | 0.033 |
| capgnawer | 0.001 | 0.638 | 6.4e-4 | 0.319 | 0.14 h | 0.52 h | 1.05 h | 0.14 h | 1.28e-2 | 20 | 0.026 | 0.10 |
| loftstrider | 0.001 | 5.28 | 5.3e-3 | 2.64 | 0.14 h | 0.78 h | 1.56 h | 0.14 h | 0.106 | 20 | 0.026 | 0.10 |
| ripple-snail | 0.001 | 0.146 | 1.5e-4 | 0.073 | 0.14 h | 0.52 h | 1.05 h | 0.14 h | 2.9e-3 | 20 | 0.026 | 0.33 |
| bellwing | 0.001 | 0.03 | 3.0e-5 | 0.015 | 0.14 h | 0.52 h | 1.05 h | 0.14 h | 6.0e-4 | 20 | 0.026 | 0.10 walking; flight 4× basal at 8 BL/s = 0.05 |
| seedporter | 0.001 | 0.356 | 3.6e-4 | 0.178 | 0.14 h | 0.78 h | 1.56 h | 0.14 h | 7.1e-3 | 20 | 0.026 | 0.10 |
| chorister | 0.0004 | 2.744 | 1.1e-3 | 2.744 | 0.69 h | 2.31 h | 4.63 h | 0.69 h | 5.5e-2 | 50 | 0.010 | 0.067 |

Motor = `motor_respiration_per_s / cruise_body_lengths_per_s` (paid at full cruise,
`parameters.md`; flight `flight.flight_factor` 4, hover 5). The night is 39.6 min = 0.66 h of a
120-min day: longer than every 1:1-reserve lineage's reserve (0.14 h) and longer than the
0.14 h a full reserve can be slept at 0.5×.

### 2.2 Life history

Eligibility to breed: structure ≥ `birth_body`, and reserve ≥ package + `surplus_floor` for
`surplus_share` 0.8 of `surplus_hold_s` 900 s (`step.rs:2262-2300`; `parameters.md`). Growth
`growth_max_per_s × fullness` out of the reserve (`step.rs:596-630`). Age at first breeding
below = growth from `body_min` at full fullness + the 900 s hold (+ gestation), arithmetic,
not counting feeding limits.

| lineage | birth mode | package (ou) | litter / clutch | gestation / incubation | `surplus_floor` (ou) | reserve needed ÷ cap at `birth_body` / at `body_max` | age at first breeding (min, arithmetic) | `birth_interval_s` | lifespan median (draw 0.8–1.2×) |
|---|---|---|---|---|---|---|---|---|---|
| littershredder | eggs on litter | 0.12 | 1 egg | 300 s incubation | 0.019 | 0.74 / 0.74 | 46 + 15 + 5 = ~66 | 1800 s | 4 h |
| frondgrazer | gestation | 0.30 | 1 young | 180 s | 0.15 | 1.00 / 0.60 | 44 + 15 + 3 = ~62 | 1800 s | 4 h |
| lanternjaw | dormant eggs (egg bank) | 0.45 | 3 eggs | dusk-gated hatch (6–24 h viability) | 0.375 | 1.31 / 0.79 → must reach body ≥ 0.825 (161 min of growth) | ~176 + dusk wait | 1800 s | 12 h |
| capgnawer | eggs | 0.204 | 1 egg | 300 s | 0.032 | 0.74 / 0.74 | 46 + 15 + 5 = ~66 | 1800 s | 4 h |
| loftstrider | gestation | 1.056 | 1 young | 540 s | 0.528 | 1.00 / 0.60 | 44 + 15 + 9 = ~68 | 1800 s | 12 h |
| ripple-snail | eggs on film | 0.047 | 1 egg | 300 s | 0.0073 | 0.74 / 0.74 | 46 + 15 + 5 = ~66 | 1800 s | 4 h |
| bellwing | eggs | 0.0096 | 1 egg | 300 s | 0.0015 | 0.74 / 0.74 | 46 + 15 + 5 = ~66 | 1800 s | 4 h |
| seedporter | gestation | 0.071 | 1 young | 180 s | 0.036 | 1.00 / 0.60 | 44 + 15 + 3 = ~62 | 1800 s | 4 h |
| chorister | pack: one breeder, gestation | 0.533 | `pack.litter_size_max` 4 | 180 s; `pack.litter_interval_s` 21,600 s; pups independent at 7,200 s | 0.274 | 0.49 / 0.29 | 118 + 15 + 3 = ~136 | 1800 s (pack interval 6 h) | 12 h |

Values: `parameters.md` `core.*`, `core.reproduction.*`, `pack.*`. Lifespan draw:
`fauna.life.lifespan_draw_min` 0.8, `_span` 0.4.

### 2.3 Predators

| | lanternjaw | chorister | source |
|---|---|---|---|
| hunting hour | nocturnal ambush with lure (`lure_power` 1.0 × awake upkeep while luring) | diurnal pack chase (`chase_give_up_s` 20, `chase_reserve_floor` 0.3) | `parameters.md` `hunter.*`, `pack.*` |
| strike | windup 0.15 s, lunge 0.25 s, recovery 3 s; reach 0.6 BL; costs 100 s of awake upkeep (0.021 / 0.110 ou) | same | `hunter.*` |
| prey size cap | 1.5 × own body = 1.58 ou (frondgrazer 1.5 fits; loftstrider 5.28 does not) | 1.5 × 2.744 = 4.1 ou (adult loftstrider 5.28 does not fit) | `hunter.prey_max_ratio` |
| damage / kill | `strike_power` 0.7 × (1 − armour); death at wound 1 → 2 hits for every prey (armour 0.25 → 0.525); wounds heal 0.5 per 30 min | same | `predation.rs:197-210`; `fauna.predation.wound_heal_per_s` |
| strike success | P(dodge) = agility × awareness(state) × aspect × condition (`predation.rs:143-157`). Agility: shredder/capgnawer/snail 0.3, grazer/seedporter 0.5, others 0. Awareness walking 1.0, resting 0.7, cropping 0.4, **asleep 0**; rear 0.5; condition = reserve fraction clamped ≥ 0.5. Hit chance therefore 0.5–1.0 vs grazer, 0.7–1.0 vs shredder, 1.0 vs asleep prey | same | `parameters.md` `fauna.predation.awareness_*`, `dodge_*` |
| satiety ("digestion") | satiated rest at reserve ≥ `rest_at` 0.9 of cap, at `torpor_upkeep` 0.25; wakes below `wake_at` 0.5 → 0.4 × 1.05 / (2.1e-4 × 0.25) = **2.2 h** | 0.4 × 2.744 / (1.1e-3 × 0.25) = **1.1 h** | `step.rs:465-475`, `hunter.*` |
| daily need (one 120-min day, basal only, at its rhythm: awake share × 1 + asleep share × 0.5) | 2.1e-4 × (2,376 s + 0.5 × 4,824 s) = **1.0 ou/day**; with lure all night +0.5 → ~1.5 | 1.1e-3 × (4,824 + 0.5 × 2,376) = **6.6 ou/day** per adult; pack of 4 = 26.4 | arithmetic |
| assimilated per kill (whole carcass × 0.5) | shredder (0.375 + 0.19) → 0.28; grazer (1.5 + 0.75) → 1.13 (above its 1.05 reserve cap; carcass lasts 1 h) | grazer 1.13; lanternjaw (1.05 + 1.05) → 1.05; seedporter (0.36 + 0.18) → 0.27 | `yield_by_food` 0.5 |
| kills per day to break even | shredders **3.6–5.4**, or grazers **0.9–1.3** | grazers **5.9 per adult, 23.5 per pack of 4** (+ pups) | arithmetic |
| pack provisioning | — | `provision_rate_per_s` 0.01 ou/s per unit giver body (= 0.027 ou/s per adult) × efficiency 0.7, while the giver's reserve > 0.6 of cap | `pack.*` |

## 3. Plants

Rules: foliage cap `P = alpha · W` (alpha 2.0 all species), reserve cap `Q = reserve_cap · W`;
foliage growth ≤ `foliage_rate · W` /s; senescence `senescence · P` /s; maintenance
`maintenance · W` /s (`flora step.rs:2571-2584`). Night: no income, maintenance and senescence
× 0.05 (§1.3). Dark-period columns are arithmetic at Q10 factor 1, starting from a full
reserve.

| species | W_max / P / Q (ou) | standing biomass, adult (ou) | leaf life 1/sen (min) | foliage regrowth 0 → P at the cap (min) | wood_rate (/s) | propagule_rate (ou/s) | nectar / fruit (ou/s per stand; cap) | one 39.6-min night: reserve used / foliage kept | reserve lasts in the dark | foliage kept after 8 h / 12 h dark | wood after 8 h / 12 h dark |
|---|---|---|---|---|---|---|---|---|---|---|---|
| springturf | 0.06 / 0.12 / 0.03 | 0.21 | 17 | 1.7 | 0.01 | 0.002 | — | 48 % / 0.89 | 1.4 h | 0.24 / 0.12 | 0.09 / 0.02 of W → below `alive_min` (0.10 W) at ~7.8 h: dies |
| velvetpad | 0.2 / 0.4 / 0.1 | 0.7 | 33 | 8.3 | 0.002 | 0.0005 | — | 4.8 % / 0.94 | 13.9 h | 0.49 / 0.34 | intact |
| stonecushion | 0.1 / 0.2 / 0.05 | 0.35 | 56 | 67 | 0.0002 | 0.00005 | — | 1.2 % / 0.96 | 56 h | 0.65 / 0.52 | intact |
| bloomcrown | 0.6 / 1.2 / 0.3 | 2.1 | 17 | 17 | 0.001 | 0.0004 | nectar 1e-4; cap 0.02 | 4.8 % / 0.89 | 13.9 h | 0.24 / 0.12 | intact |
| umbrellafrond | 0.6 / 1.2 / 0.3 | 2.1 | 17 | 17 | 0.001 | 0.0002 | — | 4.8 % / 0.89 | 13.9 h | 0.24 / 0.12 | intact |
| lanternberry | 1.0 / 2.0 / 0.5 | 3.5 | 17 | 17 | 0.001 | 0.0012 | fruit 1e-4; cap 0.02; drops at 1,800 s | 4.8 % / 0.89 | 13.9 h | 0.24 / 0.12 | intact |
| vaulttree | 5.0 / 10 / 7.5 | 22.5 | 33 | 33 | 0.0003 | 0.0008 | — | 0.8 % / 0.94 | 83 h | 0.49 / 0.34 | intact |
| siphonreed | 0.3 / 0.6 / 0.15 | 1.05 | 17 | 17 | 0.001 | 0.0003 | — | 9.5 % / 0.89 | 6.9 h | 0.24 / 0.12 | 0.93 / 0.69 |
| glowcap (saprotroph) | 0.1 / 0.2 / 0.05 | 0.35 | 17 | 8.3 | 0.002 | 0.0005 | — | not night-scaled; no light income to lose | — | unchanged by dark | unchanged by dark |
| latticevine (per reference face) | leaf 0.05, runner 0.02 | per face | dormant leaf falls over 60 s after a 600 s dry spell | regrow 0.002 leafiness/s (≈ 8 min) | — | spread 0.05 ou/face | nectar 0.002 / fruit 0.005 per face per flowering cycle (bud 60 + flower 120 + fruit 240 + spent 60 s, after a shower) | assimilation 2.7e-4 ou/s per face by light | — | — | — |
| glassfilm | `film_capacity_per_m2` 0.4 | — | — | growth 0.0015 /s | — | spread 0.0002 /s | — | maintenance 1e-4 /s | — | — | — |

Measured night loss (canopy study, live genome and defaults): **10–13 % of foliage per night**,
no stand deaths, recovery within 6–10 min of dawn (`canopy-night-loss-2026-10-01.md` "Answers"
(a)–(c)). The springturf row is arithmetic; the springturf night reserve draw (48 % of Q) is the
largest of the photosynthetic species.

## 4. Balance numbers

### 4.1 Grazed share at today's founding counts

Arithmetic demand (basal, awake all day, adult `body_max`, one 7,200-s day) at the desktop
terrarium founding counts (`config/desktop/terrarium.toml`; code defaults 8 / 8 / 3 egg clutches
for shredder / grazer / lanternjaw), against what the plants fixed in §6's run:

| herbivore | founders | food | demand (ou/day) | measured intake, 2 seeds × 1 day (ou) | intake ÷ plants' fixed organic (3,592 ou over both seeds) |
|---|---|---|---|---|---|
| littershredder | 8 | litter | 21.6 | 0.87 | 0.02 % |
| frondgrazer | 8 | foliage | 86.4 | 11.0 | 0.31 % |
| capgnawer | 6 | glowcap cap (litter fallback) | 27.6 | 46.2 | 1.3 % (glowcap is a saprotroph; its income is not in `fixed_in`) |
| loftstrider | 2 | foliage (high band) | 76.0 | 14.6 | 0.41 % |
| ripple-snail | 8 | glassfilm | 8.4 | 32.5 | 0.9 % (film growth is outside `fixed_in`) |
| bellwing | 6 | nectar | 1.3 | 0.016 | 0.0004 % |
| seedporter | 4 | fruit | 10.2 | 0.034 | 0.001 % |
| all herbivores | 42 | | 231.5 | 105 | 2.9 % |

Plant fixation per day: 1,423 ou (seed 1) and 2,169 ou (seed 2) (census `fixed_in` at min 120);
stands grew 222 → 2,994 and 215 → 3,925; standing foliage 82 → 381 and 82 → 580 ou. Whole-world
consumption (`consumed_organic`): 74.7 and 41.7 ou. Arithmetic demand at founding counts is
231.5 ou/day = 11–16 % of one day's fixation; nectar and fruit are capped per stand
(`nectar_cap` / `fruit_cap` 0.02 ou; production 1e-4 ou/s = 0.72 ou/day per stand at most, by
day only), against a bellwing's 0.22 and a seedporter's 2.6 ou/day of basal need (÷ yield 0.5
for food eaten).

### 4.2 Prey births needed per predator, against the 2026-10-02 gate

The gate CSVs (`runs/gate-curio/<arm>/out/*.csv`, seeds 9–16 × 120 min = one sim day per world)
carry only the three trained lineages; the roster lineages (capgnawer … chorister) are not in
them. Births and kills summed over the 8 worlds of each arm:

| arm | shredder births / world-day | frondgrazer births / world-day | lanternjaw births (all 8 worlds) | prey killed by lanternjaws (all 8 worlds) | starved (shr / grazer) |
|---|---|---|---|---|---|
| defaults | 2.0 | 0.75 | 1 | 0 | 17 / 46 |
| A15 | 7.5 | 1.6 | 3 | 0 | 15 / 44 |
| C1 | 5.1 | 9.8 | 8 | 1 shredder | 13 / 27 |
| C2 | 10.1 | 2.1 | 4 | 0 | 19 / 36 |
| X12 | 6.6 | 3.6 | 4 | 0 | 9 / 29 |
| X21 | 10.0 | 5.1 | 6 | 0 | 21 / 50 |

Needed to sustain one predator at the break-even kill rate (§2.3), counting each kill as one
prey birth:

| predator | on shredders | on frondgrazers | gate supply per world-day (range over arms) |
|---|---|---|---|
| one lanternjaw | 3.6–5.4 shredder births/day | 0.9–1.3 grazer births/day | shredders 2.0–10.1, grazers 0.75–9.8 |
| one chorister pack (4 adults, no pups) | — | 23.5 grazer births/day | grazers 0.75–9.8 |

## 5. Real-world anchors

From the existing audit (`ecology-parameter-audit-2026-09-28.md` §2 and Sources):
- Sim metabolism runs **~2,000–5,000×** nature (browser upkeep 1,000 s vs 20–30 days at field
  metabolic rate, Nagy 2005); the day runs **12×** (2 h vs 24 h); gestation ~60,000–100,000×;
  plant leaf life 2,600–31,000×. So a 40-min night is ~80 days of an animal's metabolism.
- Starvation buffer real: weeks for a 15–50 kg mammal (Lindstedt & Boyce 1985); sim 500 s.
- Hibernation 2–5 % of basal, daily torpor 10–30 % (audit §2 point 1).
- Plant maintenance respiration 1–3 % of biomass/day (audit table).

Added here, short and sourced:

| topic | value | source |
|---|---|---|
| torpor depth | mean minimum torpor metabolic rate ≈ **35 % of BMR** in daily heterotherms, **≈ 6 %** in hibernators; torpor bouts > 25× longer in hibernators | Geiser F (2004) *Annu Rev Physiol* 66:239–274; Ruf T, Geiser F (2015) *Biol Rev* 90:891–926 |
| starch at night | Arabidopsis leaf starch falls ~linearly through the night so that ~95 % is used by dawn; the rate is set by starch content ÷ time to expected dawn; an unexpectedly long night triggers carbon-starvation responses | Graf A et al. (2010) *PNAS* 107:9458–9463; Gibon Y et al. (2004) *Plant J* 39:847–862; Scialdone A et al. (2013) *eLife* 2:e00669 |
| moonlight | full-moon illuminance typically 0.05–0.1 lux at temperate latitudes (0.26–0.30 lux at the 2016 supermoon); the full Moon is ~1/380,000 the Sun's brightness | Kyba CCM, Mohar A, Posch T (2017) *Astron Geophys* 58:1.31–1.32 |
| small active predator | least weasel eats ~25–35 % (up to 40–60 %) of its body mass a day, ≈ 1–1.5 mice a day | species fact sheets (BioKIDS *Mustela nivalis*; Missouri Dept. of Conservation "least weasel") |
| ambush vs active predator | active-foraging coachwhips feed about every 10 days; sit-and-wait sidewinders about every 40 days; ambushers keep lower standing metabolism | Secor SM, field study cited in Secor's digestive-physiology reviews (e.g. *J Exp Biol* 185 and SICB "From field metabolic rates to genomics") |
| small-animal overnight fast | not added: the audit's Lindstedt & Boyce 1985 (fasting endurance ∝ M^0.44) is the cited baseline | audit Sources |

## 6. Measurement

**What ran.** `design/parameters.md` defaults, the desktop terrarium
(`config=config/desktop/terrarium.toml`, cycle 120 min / daylight 0.67 / start 08:00), seeds 1 and
2, **120 sim min = one full day**, `threads=4` each (8 total), `nice -n 10`, headless, built-in
trained centres for the trained lineages (the shipped drivers), main at `65043f6a`. Tool: a copy
of `crates/cubarium/examples/voxel_census.rs` with a per-tick probe, built in a disposable
scratch worktree (not committed; main untouched). Per tick and per founder body: assimilated
intake and paid motor (the search's `BodyMeter`, `cubarium-search/src/es/voxel/score.rs:55`),
**out = intake − change in stored organic** for bodies alive on both ticks (upkeep + motor +
reproduction paid out), a basal estimate `maintenance × rest factor × body`, sleep state,
births (new ids) and deaths by cause from `deaths_by_founder_cause`. Phase: day =
`life_daylight` ≥ 0.5. Night in these runs: min ~54–94 (seed 1) and ~58–98 (seed 2), from the
census `day_phase` (0.381 and 0.353 at min 0).

**Founding:** both worlds accepted; founders [8, 8, 0, 6, 2, 8, 6, 4, 4] (shredder, grazer,
lanternjaw, capgnawer, loftstrider, snail, bellwing, seedporter, chorister) + 9 lanternjaw dormant
eggs; founders arrive at adult body with reserve 0.5 of cap (`fauna.founding.arrival_reserve`).

**Energy, sleep, births and deaths by phase** (sum of both seeds; ou):

| lineage | phase | body-h awake | body-h asleep | intake | out | basal est. | motor | intake ÷ out | births | deaths |
|---|---|---|---|---|---|---|---|---|---|---|
| littershredder | day | 3.51 | 0.03 | 0.87 | 6.86 | 2.82 | 3.66 | 0.13 | 0 | starved 16 |
| littershredder | night | 0 | 0 | — | — | — | — | — | 0 | — |
| frondgrazer | day | 6.88 | 0 | 11.04 | 35.07 | 18.72 | 16.46 | 0.31 | 0 | starved 13, killed by chorister 3 |
| frondgrazer | night | 0 | 0 | — | — | — | — | — | 0 | — |
| lanternjaw | both | 0 | 0 | — | — | — | — | — | 0 | (no egg hatched; 9 dormant eggs at min 120) |
| capgnawer | day | 3.38 | 2.02 | 40.45 | 47.31 | 5.93 | 6.78 | 0.85 | 0 | starved 10 |
| capgnawer | night | 1.37 | 0.05 | 5.75 | 5.33 | 2.28 | 2.36 | 1.08 | 1 (hatch) | — |
| loftstrider | day | 1.62 | 0 | 14.55 | 33.99 | 15.29 | 18.50 | 0.43 | 0 | starved 3, killed by chorister 1 |
| loftstrider | night | 0 | 0 | — | — | — | — | — | 0 | — |
| ripple-snail | day | 4.35 | 8.69 | 30.02 | 31.26 | 4.05 | 2.76 | 0.96 | 0 | starved 9 |
| ripple-snail | night | 4.33 | 0.06 | 2.48 | 3.57 | 1.55 | 1.81 | 0.69 | 0 | starved 7 |
| bellwing | day | 3.20 | 0 | 0.016 | 0.376 | 0.184 | 0.191 | 0.04 | 0 | starved 12 |
| bellwing | night | 0 | 0 | — | — | — | — | — | 0 | — |
| seedporter | day | 1.79 | 0 | 0.034 | 2.38 | 1.14 | 1.26 | 0.01 | 0 | starved 3, drowned 3, killed by chorister 2 |
| seedporter | night | 0 | 0 | — | — | — | — | — | 0 | — |
| chorister | day | 9.99 | 0 | 1.15 | 26.13 | 23.36 | 1.26 | 0.04 | 0 | starved 4, drowned 1 |
| chorister | night | 5.28 | 0 | 0 | 5.50 | 5.38 | 0.12 | 0 | 0 | — |

**When founders died** (census `founders_<lineage>`, minute of first death / half gone / extinct):

| lineage | seed 1 | seed 2 |
|---|---|---|
| littershredder | 12 / 14 / 17 | 12 / 13 / 16 |
| frondgrazer | 11 / 27 / 42 | 1 / 25 / 40 |
| capgnawer | 11 / 14 / 15 | 14 / 16 / 3 alive at 120 (1 hatchling) |
| loftstrider | 11 / 11 / 46 | 21 / 21 / 21 |
| ripple-snail | 78 / 87 / 96 | 20 / 47 / 57 |
| bellwing | 6 / 24 / 28 | 7 / 13 / 17 |
| seedporter | 5 / 6 / 19 | 1 / 23 / 24 |
| chorister (4) | 95 / 112 / 2 alive | 115 / 118 / 1 alive |

Chorister strikes / hits: 10 / 6 (seed 1), 4 / 3 (seed 2); 6 kills over both seeds (3 grazers,
1 loftstrider, 2 seedporters). No lanternjaw strikes (none hatched).

**What the run shows (facts):**
- 7 of 9 lineages' founders were extinct or nearly so **before the first night** (min 15–46);
  every death of a diurnal lineage happened by day. Night deaths: 7 ripple-snails (seed 1).
- The only births in either world: one capgnawer hatchling, at night.
- Measured times to extinction (12–46 min) match §2.1's arithmetic for a founder arriving at
  half reserve and finding too little food: half reserve 0.07 h + body loss 0.38–0.64 h, faster
  with motor (motor ≈ basal for shredder, grazer, loftstrider).
- Rhythms confirmed by sleep: capgnawer slept 2.0 body-h by day vs 0.05 by night, ripple-snail
  8.7 vs 0.06 (nocturnal). Shredders, though nocturnal, slept 0.03 body-h: they were above the
  hunger wake line (0.5) from arrival. Choristers did not sleep at night (0 body-h).
- Plants grew throughout: stands ×13–18, foliage ×4.6–7, fixed 1,423–2,169 ou in the day; the
  animals took 2.9 % of it.
- This is one day on fresh worlds with default parameters, two seeds. It measures the founding
  day, not a steady state.
