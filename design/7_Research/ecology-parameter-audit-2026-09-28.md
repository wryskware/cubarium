---
design_status: exploration
last_reviewed: 2026-09-28
decision_refs: []
---

# Ecology parameter audit: physical basis, balance, and why browsers park

Evidence, not a decision (`7_Research/` is capped at evidence). Code audited at `3ccc6daf`.
Nothing was run; every number is arithmetic on the shipped tables.

- The full inventory, **342 rows**, is
  `ecology-parameter-audit-2026-09-28/parameters.tsv`. Each row gives the number's location,
  value, units, where it can be set, provenance, grade, real-world analog, couplings and
  whether it should evolve.
- The per-subsystem working notes (rules in plain words, derivations, inline literals, code
  shape) are `fauna-notes.md`, `flora-notes.md`, `abiotic-notes.md` and `harness-notes.md` in
  the same folder, and `scripts/` holds the arithmetic.
- The species field guide is `design/ecology-booklet.md`.

## The short version

- **Walking is not too expensive.** At full pace a browser pays 1× its resting upkeep extra.
  A real 15–50 kg mammal pays about 2× at the same pace, and a cow walking normally about 2.3×.
- **Food is far too plentiful per plant.** One adult bloomcrown, velvetpad or lanternberry
  regrows enough inside a browser's reach to feed 5–8 browsers forever. A real browser that
  stands still gets about 1 % of its food from regrowth. Relative to the plants, the 0.75 m
  browser is metabolically a ~10-gram mouse, and a mouse can live its whole life under one
  bush. **Parking is the correct strategy in this world; the brains are right.**
- **The training score cannot tell a walker from a parker.** Since the satiety rule, intake
  equals what an animal burns, so a sated walker at best ties a sated parker. Offspring do not
  score.
- **Breeding punishes trips.** Breeding needs 15 minutes of unbroken surplus, and the surplus
  margin is 100 s of walking.
- **The tuned training vector made walking 4× dearer.** The ecology search scales upkeep but
  never the walking cost.
- **The world runs on several clocks at once.** Metabolism runs ~3,000× nature, gestation
  ~60,000×, the grazing defence ~20–300×, days 12× and legs 1×. A 40-minute night is ~80 days
  of an animal's metabolism.
- **Of the 266 graded ecological numbers, about a third have a real-world basis, half are
  placeholders, and a fifth contradict nature by more than 10× after allowing for the clocks.**
  Fauna and flora have no config file at all; the ecology search can move 9 multipliers.
- **Most promising fix:** make each animal ~100× heavier in org-units, keeping its per-unit
  rates. One spot then runs out while walking still pays. The cheap test is to repeat the
  heuristic walker-vs-parker comparison at ×30, ×100 and ×300.

---

## 1. Why browsers park

### 1.1 What walking costs, next to real animals

The browser's walking price is `motor_respiration_per_s` = 0.001 of its body per second at
full cruise (1 body length/s = 0.75 m/s)
(`crates/cubarium-voxel-fauna/src/body.rs:394`, charged at `step.rs:872`). It is paid on the
*requested* motion, scales down linearly for slower walking, and charges turning at `r·|yaw|`,
so turning on the spot at the 2 rad/s cap costs 0.5× cruise. Standing costs
`maintenance_per_s` = 0.001 of its body per second (`lib.rs:1115`). **So walking at full
pace doubles the bill: it costs 1× basal extra.**

| | extra cost of walking, × resting |
|---|---|
| **browser (sim), 1 BL/s = 0.75 m/s** | **1.0×** |
| shredder (sim), 1 BL/s = 0.375 m/s | 1.0× |
| real 15 kg mammal at 0.75 m/s (Taylor et al. 1982 net cost of transport; Kleiber BMR) | 2.0× |
| real 50 kg mammal at 0.75 m/s | 1.8× |
| cow, 500 kg, normal walk 1.1 m/s | 2.3× |
| real 1 kg ectotherm (lizard or arthropod) at 0.375 m/s | ~8× |

Walking is **not** overpriced. Per second the browser walks at about half a real mammal's
relative price. A cow walking at an ordinary pace roughly triples its metabolic rate. Real
grazers keep walking cheap *per day* by walking little and slowly (cattle 3–5 km a day,
walking 5–15 % of daily energy), not because a metre is cheap. The shredder, as an
arthropod-like ectotherm, is if anything 3–8× too cheap.

The one place the price is wrong is training. `config/training/ecology-tuned-2026-09-28.toml`
sets `founder.maintenance_per_s` × 0.25, but the ecology search never scales
`motor_respiration_per_s` (`crates/cubarium-search/src/es/voxel/live/ecology.rs:374`). In
that world walking at cruise costs **4× basal**, twice a real mammal's ratio, and the animals
need a quarter of the food.

### 1.2 What a parked browser gets

At rest a browser needs 5e-5 org-units/s of upkeep, bought with foliage at an effective yield
of 0.4. The table yield is 0.5, but foliage carries 0.02 mineral per unit and browser tissue
needs 0.05, so the mineral budget binds first (`step.rs:1417`). **Need = 1.25e-4 org-units of
foliage per second.**

Plants refill a grazed rosette first: foliage regrows bottom-up at up to `foliage_rate · W`
per second, funded by the whole crown's income (`flora step.rs:2442`, `:2559`), and
senescence strips the top first. The flora notes derive what each plant sustains in the
0–0.5 m mouth band, in units of one browser's need:

| food a browser can reach without moving | sustained regrowth ÷ need (**R**) | standing edible food, in body-turnovers of need (**S**) |
|---|---|---|
| one adult bloomcrown (its upper crown is out of reach and keeps photosynthesising) | 4.9 | 2.0 (34 min of need) |
| one adult lanternberry | 8.2 | 2.9 |
| one springturf | 2.5 | 0.7 |
| dense turf, ~7 springturf in reach | ~18 | 4.7 |
| velvetpad carpet, ~7 in reach | ~34 | 16 |
| one stonecushion (the only plant a browser can out-eat) | 0.2 | 1.0 |
| **real 15 kg browser, ~3 m² in reach, 0.3 kg DM/m², ~2 g DM/m²/day regrowth** | **~0.01** | **~0.03** (half a day of food) |

A real browser standing still has half a day of food around it and regrowth worth about 1 %
of its need, so it must move. The sim browser has **250–3,000× more regrowth and 25–500× more
standing food within reach**, relative to what it needs. Density adds to it. There is one
stand per support face (`flora seeds.rs:392`) and equal-height mats never shade each other
(`step.rs:672`), so the live 0.125 m grid carries up to 64 stands/m². A velvetpad carpet holds
~57 hours of one browser's need per square metre.

Walking is not the constraint. A browser at cruise sweeps ~0.3 m²/s with its mouth. Over a
dense turf it passes more than 1,000× the food it needs, even if it takes only a fifth of what
it passes. Call this ratio **F**, the walker's feasibility.

### 1.3 Four more things that make parking win

1. **The score cannot see it.** Both trainers score `(intake − motor)/body_ref + 0.25·survived`
   (`crates/cubarium-search/src/es/voxel/score.rs`, `live/window.rs:6`).
   - "Intake" is what the bite placed in the reserve. Since package G a bite asks in
     proportion to hunger and never takes more than the reserve has room for
     (`fauna step.rs:415` `sated_bite`, `:1248`).
   - The reserve only gains room from upkeep, motor, growth and births. So
     `intake − motor ≤ starting room + upkeep + 1.5·growth`: motor cancels, and the ceiling does
     not depend on distance walked.
   - A parked animal on food reaches that ceiling; a walker at best ties it, and any hungry
     stretch, drowning or pit makes it lower. Offspring do not score, and a starving parker's
     body loss is never subtracted.
   - Frozen training only rewarded walking because every episode started the body hungry and
     off food (reserve 0). A fed adult in the live world has no headroom, and the live search
     z-scores bodies within a world, so parker and walker tie. The σ = 0 control's "no signal
     above placement noise" is what this predicts.
2. **Breeding punishes trips.** A browser may breed only while its reserve stays ≥
   `birth_cost + surplus_floor` = 0.015 (60 % of its 0.025 cap) **unbroken for 900 s**
   (`fauna lib.rs:827`).
   - Above that line it holds 0.01 of spare reserve: **200 s standing or 100 s (75 m) walking
     without food** resets the 15-minute clock. The shredder's margin is 65 s (24 m).
   - A parked browser at rich food sits at hunger ≈ 0.06 and never resets.
   - The 2026-09-28 heuristic comparison (walkers eat 23–31 % more, yet give 14 births vs 52)
     is what this rule produces.
3. **The grazing defence is invisible and on the wrong clock.**
   - The defence works: at ~3 bites/s a plant's defence settles at 0.93 after ~9 min, and
     each bite then yields 6.7 % of its ask.
   - But the mouth takes the stand with the most reachable stock (`fauna body.rs:1330`), and
     taste reports that stock through a curve that saturates at 0.05 org-units (`body.rs:1655`,
     `M_EMIT`). Neither reads the defence.
   - A defended plant keeps its foliage, because the defence shrinks the bite and not the
     stock. It therefore stays the richest stand in reach and keeps tasting full. Only the
     intake feedback reveals it.
   - It relaxes with a 1.5 h half-life, which on the animal clock is a year (§2).
4. **The training worlds are not the live world.**
   - The frozen arenas have no graze refuge, no defence and no night
     (`crates/cubarium-voxel-sim/src/arena.rs:561-566`, `:626-630`).
   - Neither arenas nor landscapes apply the ecology vector (`landscape.rs:154`,
     `arena.rs:857`).
   - So a brain trained there has never met a plant that defends itself, or a night that ends
     the search.

### 1.4 What would make walking pay

The goal is to keep **F ≫ 1** (walking must still be able to feed an animal) while pushing
**R ≪ 1** and **S ≲ 0.1** (one spot must run out). All three are per unit of the animal's
need, so one lever moves them together: **the animal's size in org-units relative to the
plants** (§3).

| browser org-unit mass × | R: one bloomcrown / dense turf / velvetpad carpet | S: dense turf / carpet | F: dense turf / carpet |
|---|---|---|---|
| 1 (today) | 4.9 / 18 / 34 | 4.7 / 16 | 1,290 / 4,300 |
| 30 | 0.16 / 0.58 / 1.1 | 0.16 / 0.52 | 43 / 143 |
| **100** | **0.05 / 0.17 / 0.34** | **0.05 / 0.16** | **13 / 43** |
| 300 | 0.016 / 0.06 / 0.11 | 0.016 / 0.05 | 4 / 14 |

"× mass" means multiplying every *absolute* animal quantity and leaving the per-unit-body rates
(`maintenance_per_s`, `motor_respiration_per_s`, `n_tissue`, yields) alone. The absolute
quantities are `body_max`, `body_min`, `birth_body`, `birth_cost`, `surplus_floor`,
`egg_organic`, `bite_per_s`, `bite_half_stock` and `growth_max_per_s`. Physical size in metres
does not change, and neither do the per-unit timescales (starving still takes 500 s).

It also bounds the population, which lifespan and density have been doing by hand. The
2026-09-24 terrarium study had the plants carrying 270–490 browsers eating ~0.7 % of what they
fixed. At ×100 and a real herbivore offtake of 5–15 %, the same plants carry roughly 35–100,
each needing many plants.

Shredders need the same treatment against litter: they out-breed their deaths 2–10× while
litter keeps rising, and a starter litter tile holds 16 shredder bodies of food.

×300 is where sparse habitat starts to starve walkers (F ≈ 4 on turf). ×100 is the centre of
the useful range. This is a first-order estimate from the tables, not a run. **The cheap test**
is the browser-movement handoff's step 1 (heuristic walker vs parker, seeds 1–8 × 60 min)
repeated at ×30, ×100 and ×300, plus one 6 h census at the winner.

The same rescale could be done from the plant side instead, by shrinking every plant stock
~100×. But that also shrinks litter, seeds, mineral draws and transpiration per plant, and
touches four subsystems. The animal side is nine fields.

---

## 2. One clock: how much faster than nature each process runs

Nothing here is meant to run at nature's speed — a 4-hour lifespan is a choice. But processes
that must balance each other need to be sped up by **the same** factor, or one of them wins by
default. `K` below is (real time constant) ÷ (sim time constant). Locomotion is shown in real
time.

| process | sim | real analog | K |
|---|---|---|---|
| animal upkeep: body burned at rest (`maintenance_per_s` 0.001) | 1,000 s | 15–50 kg mammal: 20–30 days at field metabolic rate | **~2,000–5,000** (reference) |
| starvation buffer (reserve at cap) | 500 s | weeks | ~2,000–7,000 |
| lifespan (median 4 h; 2 h in the tuned vector) | 4 h | wild median 2–5 y | 4,400–11,000 |
| birth → first offspring | ~62 min | 1–2 y | 12,000–24,000 |
| interval between offspring | 48 min | ~1 y | ~11,000 |
| gestation | 180 s | 130–200 d | **~60,000–100,000** |
| plant leaf life (`senescence` 0.001) | 1,000–3,300 s | herbs 1–3 months; tropical shrubs 6–24 months | 2,600–31,000 |
| plant foliage regrowth (α / `foliage_rate`) | 1,000 s base, 100 s springturf | 2–4 weeks | 1,200–2,400 base; ~20,000 springturf |
| springturf sprout → seeding | 161 s | 2–6 months | 33,000–97,000 |
| plant maintenance respiration | base 2e-4/s of wood | 1–3 % of biomass/day | 570–1,700 |
| litter / dead wood / carrion decay | 1,000 / 10,000 / 200 s | 3–12 months / 3–20 y / 3–10 d | 8,000–31,000 / 10,000–63,000 / 1,300–4,300 |
| grazing defence: switches on / relaxes (half-life) | ~9 min / 1.5 h | hours–days / days–weeks | **7–500 / 11–155** |
| plant waterlogging stress (bloomcrown, springturf) | τ 5 s | 1–3 days | 17,000–52,000 |
| transpiration per unit foliage | — | 2–5 mm/day | ~100 |
| soil drainage to field capacity | 0.9 s | 1–3 days | ~100,000–300,000 |
| rain frequency (2–3 showers per 2 h day) | — | one tropical afternoon storm a day | ~6–9 |
| day and night (`length_min` 120) | 2 h | 24 h | **12** |
| walking, sight, seed flight, drowning (60 s) | real space and time | | **1** |

Three consequences follow directly.

1. **A night is a season.**
   - At K ≈ 3,000 the 40-minute night is ~80 days of an animal's metabolism.
   - Asleep at 0.5× upkeep, a browser hits its wake-hunger line after 500 s. So animals
     cannot sleep through a night; they wake and forage in the dark.
   - Real hibernation runs at 2–5 % of basal and daily torpor at 10–30 %. A night this long is
     survivable only on those terms, or with reserves sized for it.
   - Flora ignores the night on main (it never reads `life_daylight`, `flora step.rs:201`). When
     WX3 lands, the night is 0.95 of a base plant's whole reserve: that trial cut stands by
     25–67 %, and the mechanism was judged wrong.
2. **A metre is ~2,000× dearer than in nature.**
   - Metabolism runs ~3,000× fast while legs run at real speed. So each metre walked costs
     ~2,000× more of the body than it would a real animal: 0.13 % of the browser's body per
     metre at cruise, against ~0.00006 % for a real 15 kg mammal.
   - A full reserve walks 187 m; the eye sees 2 m (`cone_range_m`, `manifest.rs:616`).
   - The world stays walkable only because food is ~1,000× denser than nature relative to the
     animals (§3), which is exactly what makes parking pay. The fix must keep F ≫ 1 (§1.4).
3. **Life history runs faster than metabolism.**
   - Gestation is ~20–30× fast, maturity 4–8× and plant maturity up to 30× fast relative to
     the metabolic clock.
   - Lifespan is 14 body-turnovers (7 in the tuned vector), against 70–290 for a real mammal.
   - So populations turn over and breed faster than their food and energy budgets would
     support, and the search has leaned on "shorter lives" as its only brake (lifespan ×0.5,
     Spearman +0.80).

A consistent scheme would have three parts:
- One global `K_bio`, from which every biological rate is derived: metabolism, growth,
  senescence, gestation, maturity, lifespan, decomposition and the defence.
- `K_day` and `K_weather` kept as separate art settings.
- An explicit night rule, instead of the leftover of two clocks colliding: a torpor factor for
  animals and a dark-respiration factor for plants.

---

## 3. One unit: plants and animals are counted in the same org-units, ~1,000× apart

Decision 7 of the 2026-09-21 organism decisions kept "abstract organic units; no
allometry/kg". Nothing ties a unit of plant to a unit of animal, and the two drifted apart:

- The browser is drawn 0.75 × 0.375 × 0.375 m — a 10–30 kg animal — and holds 0.05 units. On
  that scale 1 unit ≈ 200–600 kg.
- An adult bloomcrown is a 1 m shrub whose real foliage would be ~0.05 kg dry. It holds 1.2
  units of foliage, so on that scale 1 unit ≈ 0.04 kg dry. Across the nine species a foliage
  unit implies 0.04–0.25 kg dry, so the plants disagree with one another by ~6× too.
- **Plant stocks are ~1,500–4,000× too large for the animals.** A bloomcrown seed (0.1 units)
  weighs two browsers.
- **Measured in the plants' units, the browser weighs about 10 grams.** Its bites are ~1 % of
  its own body each, and one springturf holds 23 real-equivalent days of its food.
- Every species carries foliage twice its wood (`alpha` = 2, `flora lib.rs:1180`), the 3.5 m
  vaulttree included. Real trees carry 0.02–0.1.
- The litter pool is 30–80× denser than the animals' own tissue in the same units
  (`POOL_BULK_DENSITY` 80, `fauna senses.rs:97`).
- The shredder is 4.5× denser than the browser.
- The two lineages' starter food differs 13× per body: a shredder's litter tile is 16 bodies
  of food, a browser's stand 1.2.

The mismatch is partly load-bearing. With metabolism ~3,000× fast and legs at real speed,
realistically sparse food would starve anything that walks (§2, point 2). The aim is not to
make every mass real. It is to choose the plant:animal scale **on purpose**, from targets for
R, S and F, and to state it as a parameter rather than leave it as an accident of two
placeholder tables.

The planned predators and flyers are written in the same units and inherit the gap: lanternjaw
`body_max` 0.035, seedporter 0.012, bellwing 0.001
(`design/predation-and-flight-2026-09-28.md` §2.12, §3.9). P1 is being built now.

---

## 4. The inventory at a glance

342 rows. 73 are numerics that change outcomes (tick cadences, caches, bins) and are graded
N; the other 266 are graded:

| | A grounded | B plausible | C placeholder | D contradicts | N numerics |
|---|---|---|---|---|---|
| fauna (90) | 9 | 20 | 20 | 7 | 34 |
| flora (93) | 3 | 22 | 36 | 21 | 8 (+3 ungraded) |
| abiotic (85) | 15 | 15 | 34 | 6 | 15 |
| harness (74) | 3 | 6 | 35 | 14 | 16 |
| **all (342)** | **30 (11 %)** | **63 (24 %)** | **125 (47 %)** | **48 (18 %)** | 73 |

(Percentages are of the 266 graded.) What is grounded is mostly physical and static:
porosity, field capacity, wilting point, rain intensity, air humidity capacity, assimilation
efficiency of fungi, carrion decay. What is not is almost everything that sets a *rate between
organisms*.

**Where the numbers live.**
- 46 rows are settable without recompiling: the world, day clock and climate from TOML, and
  a few training settings from command-line flags.
- **Fauna and flora have no config table at all.** Every animal and plant number is Rust.
  Across the whole inventory, 121 numbers are struct literals, 86 named consts and 56 bare
  literals inside logic.
- Fauna parameters alone live in seven places: two structs, reproduction consts, loose consts
  in five files, manifest literals, inline literals and the ES layer.
- `design/backlog.md` §1, meant to be the list of user-configurable parameters, is stale for
  both fauna and flora (old body sizes, cruise speeds, hold times, pore-scale names) and misses
  about a dozen current parameters.

**What the ecology search can evolve today.**
- Nine multipliers, twelve dimensions with the per-lineage ones, each boxed to ×0.25–×4.
- `foliage_rate`, `senescence` and the defence share one multiplier across every species, so
  the search can shift levels but not the balance between species.
- `yield_by_food` is inert for the browser above ×0.8, because the mineral budget binds first.
- Bite rate, reserve, growth, motor cost, senses, soil, water and weather cannot be evolved.
- The ×100 the walking problem needs lies far outside a ×0.25–×4 box. Two of the nine ended
  pinned at their floor in the last tuning (senescence, maintenance).

**The grade-D numbers that matter most** (full list in `parameters.tsv`, filter grade `D`):

| number | where | problem |
|---|---|---|
| plant stocks vs animal body (`wood_max`, `alpha`, `body_max`) | flora `lib.rs:1180-1181`, fauna `lib.rs:1125` | ~1,500–4,000× unit mismatch (§3) |
| per-plant regrowth into the mouth band | flora `step.rs:2442`, `layers.rs:209` | one plant feeds 5–8 browsers indefinitely |
| one stand per voxel face; mineral per site | flora `seeds.rs:392`, `lib.rs:2475` | food and fertility per m² depend on the grid (64 stands/m² at 0.125 m) |
| `DEFENCE_PER_BITE`, `DEFENCE_HALF_LIFE_S` | flora `lib.rs:639`, `:641` | 20–400× slower than the animal clock; invisible to mouth and taste |
| `gestation_s` 180 | fauna `lib.rs:831` | 20–30× fast on the metabolic clock |
| blind clutch 1 egg of 32 % body | fauna `lib.rs:861-862` | real 15–60 eggs of < 1 % each |
| `cone_range_m` 2 m | fauna `manifest.rs:616` | 10–50× short of a browser's detection range |
| ES scales upkeep, not motor | search `live/ecology.rs:374` | tuned vector makes walking 4× basal |
| lifespan tuned to 2 h | `config/training/ecology-tuned-2026-09-28.toml` | 7 body-turnovers vs 70–290 real |
| founder density 8 per world; search band 30–80 | `habitat.rs`, search `search.rs:56-66` | 3,500–110,000× Damuth; fine as art, but the search optimises toward it |
| soil conductivity 0.0175 m/s | voxel `material.rs:354` | 1,750× loam: rain never runs off soil, a wet voxel drains in 0.9 s |
| open-water evaporation 1e-5 m/s | voxel `terrarium.rs:239`, `recipe.rs:700` | 12× real after the day clock (36× on the default rings) |
| transpiration per unit foliage | flora `lib.rs:1222` | set per org-unit, so per crown area it spans 53–1,056 mm/day; K ≈ 100 fits no clock |
| canopy shade `shade_k_per_m2` | flora `lib.rs:2470` | a full vaulttree passes 87 % of light; real crowns 5–22 % |
| plant drowning is instant, counts water in transit | flora `step.rs:496` | real intolerant plants survive 1–3 days |
| wind seed medians 0.26 m / 1.04 m | flora `seeds.rs:62-70` | real 10–100 m |
| runners move ±1 voxel, not rescaled | flora `seeds.rs:797` | half the authored distance on the live grid |

---

## 5. Other findings worth knowing

Not about walking, but they will bite the next rounds.

- **Water and plants are locked together.**
  - The budget is closed and the sky holds one downpour (~34 mm).
  - Rain returns only as transpiration plus lake evaporation, so any change to transpiration
    changes how often it rains.
  - A full bloomcrown asks about twice the mean rain on its footprint.
  - Roofed ground gets no rain, and because soil takes 500× the rain rate there is no runoff
    to wet halls and shelves either. The water table alone keeps them.
- **The vaulttree probably cannot hold full size.**
  - `nutrient_draw_max` caps income per stand, not per unit of stand, and mineral is read
    from the stand's one site.
  - At full size a vaulttree's income cap (0.0045) falls below its upkeep (0.0065). This is
    arithmetic, not a run (`flora step.rs:2449`, `lib.rs:1205`).
- **The shredder is out of scale in several places.**
  - Its litter yield is 0.5 against a real 0.05–0.3; carrion shares it (real 0.8–0.9).
  - Its tissue mineral equals its food's, so its diet never limits it.
  - It drowns after 60 s, where arthropods last hours.
  - Its clutches are lost at an absolute 0.2 m of water while the body itself drowns at
    0.125 m.
- **Taste and the teachers are coarse.**
  - Taste saturates at one browser body of food.
  - The heuristic teachers refuse food below 2.5–3.3× the half-stock K.
  - The shredder's chemical plume is detectable only within 2 cells of a pile.
- **Temperature and humidity reach nothing living.** The ±3 °C diurnal swing would be a ±23 %
  Q10 factor that no rule applies; WX3's Q10 is on an unmerged branch.
- **The weather handoff table is stale against the code.** The code is diurnal 3, drizzle rh50
  0.98, shower λ 1.5, downpour 2 R0, stream gain 4; the handoff says 4, 0.93, 2.0, 2.5, 2.
- **Startup acceptance checks stock, not supply.** N = 1 founder-hour of standing food per
  founder says nothing about whether regrowth can feed them.
- **The search's liveliness target (30 % moving) sits beside a 33 % sleeping night**, so at
  most 67 % of body-minutes can ever count as moving.

---

## 6. Making every number evolvable

The ask was to flag every hard-coded number so it can become an evolvable parameter.
`parameters.tsv` is that list. Each row's `evolve` column says `yes log[a,b]`, `yes
per-species`, `derive:<formula>` or `no: physics/structural/numerics`. The conversion is
cheapest as **one parameter registry** rather than 266 separate edits.

**The registry.** A data file (TOML) with one row per parameter:

```
id | default (per species / lineage) | unit | range (log or linear) | scaling rule
   (voxel distance / face area / per unit body / none) | provenance | evolvable? | in policy digest?
```

From that one table the code would generate:
- the Rust defaults and validation (replacing ~600 lines of hand-written validators);
- the voxel-size rescaling (today a hand-kept field list);
- TOML overrides for fauna and flora, which have none today;
- the ES vector (today hand-wired in `ecology.rs`);
- the `backlog.md` §1 listing.

Fields in the trained-policy digest (body size, eye and mouth anchors, cruise, cone, taste
resistances) are flagged, so evolving them is a deliberate retrain and not an accident.

**Baselines × multipliers, not raw numbers.** The ES evolves well when every gene is a
dimensionless multiplier on a physically motivated baseline, in a log range where ×1 is the
baseline:

- **animals** — upkeep is Kleiber (or the ectotherm line) × `K_bio`, and motor cost is a
  multiple of upkeep from Taylor's cost of transport. Intake capacity is a multiple of upkeep
  (real 2–4×). Reserve, gestation, maturity and lifespan follow body-size allometry × `K_bio`.
  The org-unit mass comes from physical size via a per-lineage `mass_scale`.
- **plants** — relative growth rate, leaf life, foliage-per-wood (`alpha`) and maintenance
  come from functional-group baselines (herb, mat, shrub, tree) × `K_bio`. Transpiration is
  per m² of crown, not per org-unit.
- **decomposition and defence** — on `K_bio`, from literature decay constants and induction
  times.
- **fixed, not evolved** — soil physics (porosity, field capacity, wilting point, and
  conductivity once corrected) and numerics.

**Targets for the ecology search**, alongside Wrysk's existing bands (persistence,
boundedness, liveliness, life cycle, plant community, stocks, cost). These are the three
stay-or-go numbers, measurable from any world snapshot and the lineage tables:
- R, regrowth in reach ÷ need: 0.02–0.3;
- S, standing food in reach ÷ need over one body-turnover: 0.02–0.2;
- F, the walker's feasibility: ≥ 10.

They say whether movement is necessary and possible before a single brain is trained. The
search box must also widen: `mass_scale` needs roughly [1, 1,000] on a log scale.

---

## 7. "Almost 40k lines for living ecology?" — how the code looks

**Where the ~38k lines of `cubarium-voxel-fauna` + `cubarium-voxel-flora` go** (line
classifier, `scripts/loc.py`):

| | fauna | flora | both |
|---|---|---|---|
| ecology logic | 6,900 | 7,400 | **14,300 (38 %)** |
| doc-comment prose | 3,100 | 3,500 | 6,600 (17 %) |
| tests inside `src/` | 5,600 | 2,300 | 7,900 (21 %) |
| examples / study harnesses | 1,000 | 7,000 | 8,000 (21 %) |
| blank | 550 | 500 | 1,050 |

Integration tests in `tests/` add another ~26k (fauna 9.6k, flora 16.3k — more than flora's
whole `src/`). The world crate adds 11k lines of water, weather and terrain logic.

**Verdict.**
- The logic is careful and correct-minded. Every transfer conserves organic matter, mineral
  and energy exactly, randomness is deterministic and keyed, and the rules are explicit. It
  is not spaghetti.
- For what it does, ~14k lines of logic is heavy but not absurd. That covers nine plants with
  staged, layered crowns; light, shade, root-box water, aeration and mineral stoichiometry;
  seed banks with lotteries and four dispersal modes; runners; a flowering and fruiting vine;
  a fungus; and two animal lineages with continuous bodies, four senses, a trained-controller
  contract, gestation and eggs, sleep, wall climbing and drowning.
- A lean implementation of the same features would be maybe 6–9k lines.
- The weight is identifiable, and most of it is accretion rather than necessity.

**The five sources of bulk:**
1. **Design history in doc comments.** A fifth to a quarter of `src/` is prose, much of it
   history: review round numbers, package letters, "this was X before package Y". The nine
   flora presets are ~60 % prose. That belongs in commits and handoffs.
2. **Parameters as code.** Parameters are hand-wired in seven places per crate. There are ~600
   lines of hand-written validators, a hand-kept voxel-rescale list, a hand-kept ES list and a
   stale backlog list.
3. **Duplication.**
   - The splitmix RNG is written 4× (3 in fauna, 1 in flora); seconds→ticks 5×.
   - The pro-rata organic/mineral/energy transfer is written 4× in fauna, newborn
     construction 2× and the sated bite 2×.
   - World founding appears 5× across the host, sim and search, and per-body scoring 2×.
   - The latticevine re-implements the stands' root-water read, upkeep and dieback.
   - The germination predicate has 7 entry points, and there are two separate rules for
     "what a mouth reaches".
4. **Legacy paths kept alive "untouched".**
   - The heuristic frondgrazer species path: ~450 lines plus six config fields the live
     founders never read.
   - A ~600-line dense water solver that runs only for worlds taller than 128 rows, which no
     shipped world is.
   - Fixture-only switches, an open-budget rain branch, a `WeatherView::legacy` with no
     callers, and protocol-history strings.
   - Two old flora study examples of 2.5k lines each.
5. **Tests pinning old contracts.** Round-numbered integration suites (round3/4/5b/5c) are
   the kind the fast-iteration policy retired.

**Refactors, with the workers' rough estimates (logic lines):**

| refactor | saves | also gives |
|---|---|---|
| the parameter registry of §6 (presets and validators as data) | −1,000 to −1,300 | every number evolvable and settable from TOML — the audit's main ask |
| one **organism budget kernel** shared by stands, vine and animals (income → upkeep from income then reserve → build at 1 + c_g → mineral fraction rule → energy and heat booking → dieback) | −300 to −500 | plants and animals stop being two bookkeeping systems; Photo and Saprotroph become two income functions |
| latticevine as a stand variant (reusing root box, drink, moisture ramp, income, dieback) | −600 to −900 | |
| retire the heuristic frondgrazer path; make the frondgrazer a founder | −450 | |
| delete the dense water solver (let the rules crate handle any height) | −600 | |
| one `found()`, one per-body meter, one episode-fixture config for host, live and frozen training | −300 | training worlds become the live world (§1.3 point 4) |
| small shared helpers (RNG, seconds→ticks, three-currency transfer, spawn), one germination predicate | −300 | |
| legacy branches, fixture switches, protocol strings | −300 | |

- Together that is **roughly −3,900 to −4,700 lines of logic**. About 2,800–3,600 of them are
  in fauna and flora, a fifth to a quarter of their logic.
- Moving the design history out of doc comments removes ~3k more prose, and retiring the two
  old study harnesses 5k.
- The fauna + flora crates would land nearer 25k lines than 38k, with the same behaviour.
- Order: registry first (it is also the audit's main recommendation), then the pure deletions
  (legacy paths, dense solver), then the shared kernel.
- Anything that moves a manifest-digest field needs a retrain, and schema bumps mean fresh
  worlds, which is fine under the always-fresh policy.

---

## 8. Suggested order before the next training round

Suggestions, not decisions. Items marked **(Wrysk)** change a rule or a design choice.

1. **Tie motor cost to upkeep in the search.** Express it as `motor_factor ×
   maintenance_per_s` (default 1, real mammals 1.5–2.5), so an upkeep multiplier no longer
   silently changes the price of walking. Do not train on the 2026-09-28 tuned vector as it
   stands.
2. **Run the mass-scale test.** Heuristic walker vs parker at browser mass ×30 / ×100 / ×300
   (and shredder vs litter), seeds 1–8 × 60 min. If walkers win at ×100, add a per-lineage
   `mass_scale` to the ES with a log range of about [1, 1,000].
3. **(Wrysk) Decide what training rewards once satiety exists.** The options are: net body
   and energy change including starvation loss; offspring produced; or keep the score and let
   the world make parking unable to sate (2 and 4). The last needs no reward shaping.
4. **Make the defence readable and move it to the biological clock.** Mouth choice and taste
   should read `(1 − defence) × stock`. It should switch on within a few minutes of grazing
   and relax over tens of minutes, not 1.5 h.
5. **(Wrysk) Breeding eligibility.** Replace the unbroken 900 s hold with "in surplus for most
   of the last 900 s", or widen the margin, so a foraging trip is not a reset.
6. **Make the training worlds the live world.** Arenas and landscapes should have the graze
   refuge, the defence, the night and the ecology vector.
7. **(Wrysk) One clock.** Make `K_bio` a parameter and derive gestation, maturity, lifespan,
   decomposition and defence from it. Settle the night rule (animal torpor factor, plant dark
   respiration) before WX3 merges.
8. **The parameter registry** (§6): TOML for fauna and flora, every number evolvable,
   validation generated.
9. **Water, after the unit decision.**
   - Soil conductivity down 100–1,000×, so runoff and puddles exist.
   - Evaporation to the day clock.
   - Transpiration per m² of crown, not per org-unit.
   - These interact with any plant rescale.
10. **Predation and flight (P1, F1).** Define their placeholder masses through the same mass
    scale before they are tuned; today they inherit the mismatch.

---

## Sources

- Kleiber M (1961) *The Fire of Life*. Hemmingsen AM (1960) *Rep Steno Mem Hosp* 9:1–110.
  Gillooly JF et al. (2001) *Science* 293:2248.
- Taylor CR, Heglund NC, Maloiy GMO (1982) *J Exp Biol* 97:1–21 (cost of transport). Full RJ,
  Tu MS (1991) *J Exp Biol* 156:215–231.
- Nagy KA (2005) *J Exp Biol* 208:1621–1625 (field metabolic rate). Lindstedt SL, Boyce MS
  (1985) *Am Nat* 125:873–878 (fasting endurance).
- Calder WA (1984) *Size, Function, and Life History*. Peters RH (1983) *The Ecological
  Implications of Body Size*.
- Damuth J (1981) *Nature* 290:699–700 (density). Harestad AS, Bunnell FL (1979) *Ecology*
  60:389–402 (home range).
- Cyr H, Pace ML (1993) *Nature* 361:148–150 (herbivore offtake). Charnov EL (1976) *Theor
  Popul Biol* 9:129–136 (marginal value theorem).
- Grime JP, Hunt R (1975) *J Ecol* 63:393–422 (plant growth rates). Reich PB et al. (1992)
  *Ecol Monogr* 62:365–392; Wright IJ et al. (2004) *Nature* 428:821–827 (leaf life).
- Olson JS (1963) *Ecology* 44:322–331 (decomposition). Sterner RW, Elser JJ (2002)
  *Ecological Stoichiometry*.
- Rawls WJ, Brakensiek DL, Saxton KE (1982) *Trans ASAE* 25:1316–1320; Saxton KE, Rawls WJ
  (2006) *SSSAJ* 70:1569–1578 (soil water).
