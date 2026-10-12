---
design_status: proposal
date: 2026-10-12
decision_refs: []
---

# Bellwing larvae and vaulttree nuts: build plan and budget (2026-10-12)

A proposal for Wrysk's approval before anything is built. It closes the seedporters' protein
gap: their young are nitrogen-bound (fruit carries mineral 0.01 per organic; seedporter tissue
wants 0.05). Facts and the first budget are in
`design/7_Research/bellwing-larva-facts-2026-10-12.md` (here "facts §n"). No ledger entry
(D-0001..D-0006) touches bellwings, vaulttrees, seedporters or insects. Nothing here is canon.

## 0. His decisions (2026-10-12; not reopened)

1. Hosts: "particularly on latticevine i think would work well since other trees dont have
   fruit so they are more of a detour for seedporters"; with nuts, "i would split the larvae
   between vaault tree and latticevine".
2. Soft-shelled vaulttree nuts, opened by seedporters: "(a) is fine. important part is thre
   nutrition aspect".
3. "yah (c) for mix": larvae are a density on the host's leaves; survivors emerge as
   individual adult bellwings.
4. "sure. b+c and adjust accordingly": seedporters eat larvae, browsers take them by accident
   with leaves, and a background death rate is set so emergence matches today's adults.
5. "a for now, art later": undrawn. No look is proposed here.
6. Insect flesh at 0.05 N. INSECT-N (76627f17) raised the other animals; the bellwing stays
   0.02 until the larvae ship, then goes to 0.05 with them.
7. Ratios, not absolute values; keep the world's current ratios.

## 1. Findings that shape the plan (code and runs)

- **The vaulttree has the whole fruit-crop machinery, switched off**: `fruit_per_s` 0,
  `fruit_cap` 0, `flower_s` 0, `pollination_floor` 1.0 (expanded config of main 16f89de3,
  `runs/lab/1012-135958-insect-n-1aca/main/expanded.toml:4598-4714`). With `flower_s` 0 a crop
  sets in full at once with no pollinator (`cubarium-voxel-flora/src/step.rs:3533-3538`). The
  crop makes fruit from reserve at `fruit_per_s` × `fruit_cost`, drops the uneaten crop to
  litter with its mineral at `fruit_drop_s`, then rests (`step.rs:3579-3632`, `end_crop`
  `step.rs:3756`).
- **Seedporters already forage any stand with fruit**: `fruit_candidates` takes every stand
  with `fruit > 0` and a climbable crown (`cubarium-voxel-fauna/src/glider.rs:1899-1948`,
  `climbable` `glider.rs:65`); `glider_feed` bites it through `take_stand_fruit`
  (`glider.rs:1179-1260`, `cubarium-voxel-flora/src/lib.rs:6330`). A fruit meal stows seeds of
  the source plant in the gut (`glider.rs:1238-1255`): wrong for a nut, which *is* the seed.
- **Energy is uniform**: every plant and animal `energy_density` is 2.0 (expanded config), so
  the world compresses energy ratios to 1:1. Fruit energy is the species' one density
  (`lib.rs:6336`).
- **Nothing browses latticevine leaves.** `Flora::crop` (`cover.rs:2203`) has no non-test
  caller; browsers eat stand foliage only (`take_foliage_of`, fauna `step.rs:4979`). So larvae
  on vines get no accidental browser mortality. On vaulttrees only the loftstrider reaches the
  crown (mouth 0.8-3.5 m: `adult_height_m` 3.4, mouth floor 0.235, ceiling 1.03); frondgrazers
  reach 0-0.5 m (saplings).
- **Vine leaf is low in N**: vine `mineral_fraction` 0.01 applies to leaf, nectar and fruit
  (`cover.rs:2229`); vaulttree tissue 0.02. A larva at 0.05 can build only 0.2 (vine) or 0.4
  (vaulttree) tissue per organic of leaf: the N-bound assimilation of `assimilate`
  (fauna `step.rs:5065-5085`).
- **Yield caps usable N**: a bite builds `min(yield × organic, mineral / n_tissue)`. With the
  seedporter's flesh yield 0.5 and tissue 0.05, food above 0.025 N per organic adds nothing.
- **Today (8 seeds × 8 h, `runs/lab/1011-204110-nurse-groom-bae9`, main arm)**: bellwing
  bodies m480 median 280 (237-376), of which adults ~94 (84-136) and juveniles ~170; eggs
  laid 332 per world; ~15 adults/h maturing in h5-8 (facts §2). Vaulttree stands 147 at m240,
  272 (141-698) at m480. Seedporter fruit 4.1 organic per 8 h (vine 2.4), 19.5 bellwing kills;
  vine faces ~62,600 on ~6,650 vines; stand leaf produced ~2.1e5 organic per 8 h.

## 2. Nuts (package NUTS)

- **Source: the vaulttree's own fruit crop.** No new stock and no new flora state: set the
  crop knobs and add two. It reuses crop clocks, litter drop, census and seedporter foraging.
- **Pollination: none (wind)**: `flower_s` 0, so a crop sets in full, as in wind-pollinated nut
  trees (hazel, walnut, chestnut). The nuts stay independent of bellwing numbers.
- **Nutrition at the world's ratios.** Real (USDA FoodData Central, per 100 g, recalled): almond
  protein 21.2 g at 4.4 g water, cashew 18.2/5.2, Brazil nut 14.3/3.5 → 2.4-3.5 % N dry; fat
  44-67 g → ~6-6.8 kcal/g dry. Banana 1.1 g protein at 75 g water → ~0.7 % N dry, ~3.5 kcal/g.
  Leaves ~2 % N (Wright et al. 2004, Nature 428). Lean flesh and insects 50-70 % protein dry →
  8-11 % N (Rumpold & Schlüter 2013, Mol. Nutr. Food Res. 57). The world maps real fruit:flesh
  0.074 to 0.2. As a power law that exponent is 0.62, and it puts real leaves (0.21) at 0.38 ×
  0.05 = 0.019, which matches the world's leaf 0.02. The same map puts nuts at 0.021-0.026.
  **Nut N 0.025 per organic**: half of flesh, 2.5× fruit, 1.25× leaf. It is also exactly what
  the seedporter's yield 0.5 can use. **Energy 2.0**, keeping the world's uniform energy.
- **Crop** (vaulttree, crowned stands only, `profile_stage >= 1`): `fruit_cap` 0.02 (one
  lanternberry crop), `fruit_per_s` 1e-5 (made in ~0.5 h of daylight), `fruit_drop_s` 14,400
  (4 h on the tree), `fruit_rest_s` 7,200, `fruit_cost` 2.0 (husk and soft shell, respired as
  overhead; real kernels are ~40-60 % of the in-shell nut). `fruit_mineral_fraction` 0.025.
- **Handling (new knob `fruit_handling`, plant species, 0-1, default 1):** it scales the bite
  in `glider_feed` for that species' fruit. Vaulttree 0.33: a nut costs ~3× the feeding time
  of fruit per organic. Recalled anchor: tree squirrels spend tens of seconds to minutes per
  ~1 g kernel, while small frugivores take pulp at several g/min. Longer exposure in the crown
  is the price; nothing scores it.
- **Seeds (new knob `fruit_seeds_in_gut`, default true; vaulttree false):** a nut eaten is a
  seed destroyed. The vaulttree keeps its own Wald dispersal.
- **Drop and other eaters:** an uneaten crop falls to the site's litter with its mineral
  (existing). Shredders and capgnawers eat it there as N-richer litter. Only seedporters climb
  for hanging nuts; no new eater.

## 3. Larva field (package LARVA)

- **Owner and shape:** fauna owns the field (it is the bellwing lineage), keyed by host:
  `Vine(VineId)` or `Stand(stand id)` for vaulttrees with a crown. Each host holds at most 3
  cohorts `{count, body per larva, age_s, mineral, energy}` and merges the oldest when full,
  after the seed-cohort precedent (`Ground::seeds`, `seeds.rs:351`). Flora gains no state.
  The field grazes through the existing take APIs (`Flora::crop` per vine face, pro rata to
  leafiness; `take_foliage_of` across a vaulttree crown), so mineral and energy only move.
- **Laying:** when a clutch is ready, a keyed draw picks the host kind with
  `larva.vaulttree_share` (default 0.5, his split). The adult flies to the nearest host of that
  kind with leaf within its nest search (`vine_egg_site` `step.rs:6147` and `places.rs:142`
  already find vines by near-IR); with none in range it uses the other kind. Eggs go straight
  into a host cohort and incubate there for `incubation_s` (183 s). The vine Clutch-on-face
  object and its hatch (`lay` `step.rs:5816`, `hatch` `step.rs:6473`) retire for the bellwing.
- **Egg:** `egg_organic` 0.0006, 1 % of the adult (real moth eggs 0.1-2 % of the female:
  Garcia-Barros 2000, cited at `body.rs:2561`). `clutch_size` 5 per `birth_interval_s`, still
  nectar-limited.
- **Growth on leaves:** per larva, exponential to the emergence size over the larval period,
  capped by leaf intake × `min(yield, leaf N / 0.05)`. Larval foliage yield 0.4, which gives
  conversion 0.2 on vine and 0.4 on vaulttree. Real caterpillar ECI on tree leaves is
  ~0.1-0.2 (Scriber & Slansky 1981, Annu. Rev. Entomol. 26). Upkeep at the bellwing
  `maintenance_per_s`. The unassimilated rest goes to litter as frass. Tissue N 0.05.
- **Larval period 4 h** (today's hatch-to-adult median, 245 min) against a 12 h adult median
  life. Real larvae often live as long as the adult or longer (facts §5); see choices.
- **Deaths:** a background rate `larva.death_per_s` (parasites, disease) goes to the host's
  litter. Seedporters and browsers add their takes. Set for survival ~3 % egg to adult (real
  "a few percent": Zalucki, Clarke & Malcolm 2002, Annu. Rev. Entomol. 47). The total needed
  is μ = ln(1/0.03)/4 h = 0.88/h. The background takes what predation leaves:
  **`death_per_s` ≈ 1.5e-4 (0.55/h)**. It is a knob, so emergence can be re-matched after the
  check.
- **Emergence:** a cohort whose larvae reach `larva.emerge_body` becomes floor(count) adult
  bodies; the fraction left over goes to litter, booked. Each body carries exactly the
  cohort's per-larva tissue, mineral and energy. Default `emerge_body` = `body_max` 0.06:
  adult insects do not grow. Adults then spend nectar on upkeep, flight and eggs, and the
  bellwing's total biomass stays about today's (120 × 0.06 ≈ 280 × 0.026). A body emerges on
  its vine through the existing hatchling cling (`vine_cling`) or flying at a vaulttree crown.
  No pupa (`pupa_s` 0).
- **What retires:** today's nectar-fed juvenile stage (no juvenile bellwing bodies). The
  bellwing's vine clutches go too. The dormant egg bank and `hatch_draw` are the lanternjaw's
  (facts §1) and stay as they are. The bellwing's `n_tissue` goes to 0.05 (decision 6).
- **Founding:** each host near the founders' nectar gets larval cohorts at ages staggered
  over 0-4 h, sized for ~15 emerging per hour. Without them emergence starts only at h4-5, and
  from 15 founders it is 2-9/h, against today's h4 pulse of 40-84.

## 4. Eating larvae

- **Seedporter (rule path, existing channels):** a host whose visible larvae exceed a floor
  becomes a food candidate, beside fruit, in `fruit_candidates`, seen by sight within
  `larva_sight_m` (2 m; caterpillars are cryptic and found by close visual search). A vine
  candidate uses its leafy faces; a vaulttree candidate uses its crown, the same climb as
  roosting and nuts. It fills the existing FOOD track (`glider.rs:1962-1975`). It takes the
  nearest food, with no preference and no reward term. The bite is a new `glider_larva_feed`,
  the twin of `glider_feed`: `gut_bite` with a `larva_bite_half_stock`. It eats the host's
  larvae, youngest cohort last. It is a new `Food::Larva` (index 7) with seedporter yield 0.5
  (as flesh). Each bite carries 0.05 mineral and 2.0 energy per organic, and the field's count
  falls by bite / body.
- **Browser by accident:** in fauna `feed` (`step.rs:4977-4990`), when a bite comes off a
  vaulttree host, the host's larvae fall by the eaten fraction of its edible foliage (taken
  `organic` / `edible_foliage` before, `lib.rs:4931`). Their tissue goes into the same bite
  under `Food::Foliage`. Incidental insect intake by mammal herbivores is common (Gish, Ben-Ari
  & Inbar 2017, Oecologia 183; recalled). Vine larvae lose none: nothing browses vines.
- **Conservation:** every larval gram comes from leaf (organic, mineral, energy) and leaves as
  emerged body, an eater's bite, litter (deaths, frass, emergence remainder), or respiration.
  A host that dies or loses its last leafy face drops its larvae to litter.

## 5. Budget (whole web, one wide world, steady state h5-8)

Larva: egg m0 0.0006 to 0.06 (100×), period T 4 h, survival s 3 %, constant per-capita
death. Dead tissue per egg = m0·μT·(sR−1)/ln(sR) (R 100); standing biomass = eggs × m0·T·(sR−1)/ln(sR).

| Flow (per world) | Today | Predicted | Basis |
| --- | --- | --- | --- |
| Adults emerging | ~15/h (h5-8) | 15/h (10-25) | target; facts §2 |
| Eggs laid | ~40/h at 0.007 | ~500/h at 0.0006 | 15 / 0.03 |
| Egg organic from nectar | 0.28/h | 0.30/h | same nectar buys 12× the eggs |
| Egg N from nectar | 0.006/h | 0.015/h (~1.5 organic nectar/h) | vs bellwing intake ~6/h |
| Standing larvae | none | ~550, ~2.2 organic | 500 × 0.97 / 0.88 |
| Larval tissue dying | none | ~1.9 organic/h (0.096 N) | 6.4 m0 per egg |
| Larval tissue built | none | ~2.5 organic/h | dead + emerged − eggs |
| Leaf eaten by larvae (50/50) | none | ~9.5 organic/h | 1.26/0.2 + 1.26/0.4 |
| Stand leaf produced | ~26,000/h | unchanged | census `leaf_produced` |
| Bellwing bodies at m480 | 280 (237-376) | 100-200 (median ~140) | adults only |

Headroom: larvae eat <0.1 % of stand leaf production. Per host it is ~0.015 organic/h on a
crowned vaulttree whose `foliage_rate` 1.44e-3/s × wood allows tens per hour, and ~1e-3/h on
a vine whose income is ~4.4e-4/s per lit face. The plants do not notice; plant pace is unchanged.

Larval deaths split (predicted): background 50-65 %, seedporters 25-45 % (0.5-0.9 organic/h),
loftstrider 0-15 % (vaulttree only; 0-2 loftstriders per world).

Seedporter nitrogen. It is counted as *usable* N, since yield caps it: usable per organic is
min(yield × 0.05, food N). Fruit 0.01, flesh and larva 0.025, nut 0.025. The need is facts
§3: ~0.056/h for 14 young growing 0.071 → 0.711, or ~0.07/h through milk (`nurse_efficiency`
0.8).

| Source | Today organic/h | Today usable N/h | Predicted organic/h | Predicted usable N/h |
| --- | --- | --- | --- | --- |
| Fruit (vine + lanternberry) | 0.51 | 0.005 | ~0.5 | 0.005 |
| Carrion of own kills (INSECT-N 0.05) | 0.35 | 0.009 (0.007 before) | ~0.35 | 0.009 |
| Adult bellwings (0.05 after LARVA) | ~0.06 | 0.001 | ~0.1 | 0.0025 |
| Larvae | 0 | 0 | 0.4-1.0 | 0.010-0.025 |
| Nuts (offered ~1.3/h) | 0 | 0 | 0.4-1.0 | 0.010-0.025 |
| **Total** | ~0.9 | **~0.014** | 1.8-2.9 | **0.037-0.067 (median ~0.05)** |

Nut offer: ~200 crowned vaulttrees × one 0.02 crop per ~3 h (eaten out) to 6 h (dropped) ≈
0.7-1.3 organic/h. The uneaten part falls to litter (5-10 organic per 8 h). Intake rises
because N-funded food fills the reserve that growth draws, so hunger returns. Today's intake
is satiety-capped, with N-poor food going to fuel. Together the two packages close ~70-95 %
of the young's need; neither closes it alone.

Bellwing numbers: with constant larval death, the population stays nectar-bound, as today.
Each adult lays ~5 eggs/h × ~10 h life × 3 % ≈ 1.5 adults, so it still climbs, but on adults
only. The larva bounds them only with a crowding term (choice 8). Juvenile bodies (~170 at
m480) disappear. At ~0.1 ms per animal per tick (facts §4) that predicts `tick_ms` 15-35 %
lower.

## 6. Brains and senses

The observation vector keeps its shape. Larvae enter through the existing FOOD track (sight)
and the rule forage path, and nuts are ordinary stand fruit. Two things need the behaviour-AI
thread (`.claude/ledgers/behaviour-ai.md`, `design/behaviour-ai-2026-10-11.md` §10.2):
(a) the FOOD track can now point at a larva host, a semantic change for trained or cloned
seedporter brains; (b) the seedporter forage labels gain larva bites, and clones should be
redone after LARVA. No new obs channel (smell, food kind) is proposed. If one is wanted, it is
that thread's call.

## 7. Packages

| Package | Files (main) | Schema | Size | Lane |
| --- | --- | --- | --- | --- |
| **NUTS** | flora `lib.rs` (SpeciesConfig `fruit_handling`, `fruit_seeds_in_gut`), vaulttree defaults, fauna `glider.rs` `glider_feed` (bite scale, seeds), `voxel_census.rs` (cols), registry | none: config only (`fauna/src/snapshot.rs:161`); flora stays 41 | small | Opus medium or Codex high |
| **LARVA** | fauna: new `larva.rs` (field, cohorts, growth, deaths, emergence, founding), `step.rs` (`lay`, browser hook in `feed`), `glider.rs` (candidates, `glider_larva_feed`), `lib.rs` (`Food::Larva`, config, ledger), `body.rs` (bellwing egg, clutch, `n_tissue` 0.05), snapshot; `voxel_census.rs`, vitals; registry | fauna `snapshot.rs:195` 84 → 85 | large | Opus high |

Order: NUTS first; it is independent and gives an early read on N. LARVA can start in
parallel in its own worktree and rebase onto NUTS; both touch `glider.rs`, in different
functions. Census columns:
NUTS `nut_standing`, `nut_fell`, `fruit_eaten_seedporter_vaulttree`; LARVA `larvae_vine`,
`larvae_vaulttree`, `larva_mass`, `larva_eggs_in`, `larvae_emerged`, `larva_died_background`,
`larva_eaten_seedporter`, `larva_eaten_browser`, `larva_leaf_eaten`. Vitals for the bellwing
row: `n_larva`, `larva_mass`. Every number above is a serde knob with a registry row and its
real note.

Law tests (a separate authoring pass, Opus high; laws, never tuned numbers):
- NUTS: a nut bite moves `fruit_mineral_fraction` × organic out of the stand into the eater,
  with energy at density. At equal stock and time, a handled fruit's bite is
  `fruit_handling` × an unhandled one. An uneaten crop falls to litter with its mineral at
  `fruit_drop_s`. With `fruit_seeds_in_gut` off, the gut stows no seeds.
- LARVA: closed books per step (leaf in + eggs in = tissue change + emerged + eaten + litter
  + respired, for organic, mineral and energy). Larval mineral is always 0.05 × tissue, and
  growth never exceeds what the leaf's mineral funds. With no eaters the expected count
  follows exp(−`death_per_s` · t). A browser eating fraction f of a host's edible foliage
  takes fraction f of its larvae, and none from vines. Emergence yields floor(count) bodies at
  `emerge_body` with the remainder to litter. A host's death drops its larvae to litter. A
  share of 0 or 1 lays only on vines or only on vaulttrees when both are in range. A
  seedporter larva bite carries 0.05 mineral per organic and lowers the count.

8-seed check (wide world, 8 h, after both): emergence h5-8 10-25/h (median 15). Bellwing
bodies m480 100-200 (median ~140), juvenile bodies 0. Eggs 400-600/h after h2. Standing larvae
400-700. Seedporter larva intake 3-8 and nut intake 3-8 organic per 8 h. Seedporter usable N
0.04-0.06/h. Seedporter juveniles at m480 ≥ today's, with young growth 2-3× today's. Stand
leaf eaten unchanged within noise. `tick_ms` 15-35 % lower. NUTS alone: nut intake 3-8 per
8 h, seedporter usable N ~0.02-0.035/h.

## 8. Open choices (not his decisions above; recommendation first)

1. Nut N 0.025 (world-compressed real nut:flesh; the most yield 0.5 can use). Rec 0.025.
2. Nut energy 2.0 (the world's uniform energy) vs ~2.2 for fat. Rec 2.0.
3. A nut eaten destroys its seed (no gut dispersal of vaulttrees). Rec yes. **His call?** It
   changes how vaulttrees spread.
4. Fallen nuts as plain litter vs a ground nut pool seedporters pick up. Rec litter for now.
5. Emergence at `body_max` 0.06 (insects do not grow as adults) vs 0.03. Rec 0.06. **His
   call?** It changes adult size and roughly doubles larval food.
6. Larval period 4 h (today's timing; visible in an 8 h check) vs 12-24 h (real ≥ adult life).
   Rec 4 h.
7. Survival 3 % with eggs at 1 % of the adult. Rec as stated; lower survival means more larval
   food but more egg N from nectar.
8. Density-dependent (crowding) larval death per host to bound bellwings. **His call** (a new
   mechanism beyond "a background death rate"). Rec off for the first check; decide on the
   m480 body count.
9. Larva sense: short-range sight through the FOOD track; no obs change. Rec this. Flag to
    the behaviour-AI thread.
10. Mechanics: a new `Food::Larva` (vs reusing Carrion), the field owned by fauna keyed by host
    (vs stored in flora), a per-clutch host draw, no pupa. Rec all four as written.
11. Young lanternjaws eating larvae (the 2026-10-06 intent) is outside decision 4. Rec leave
    out. **His** if wanted.
12. Vine larvae have no browser deaths, because nothing browses latticevine. Rec accept, so
    vines carry a higher share of emergence. Inform him.
