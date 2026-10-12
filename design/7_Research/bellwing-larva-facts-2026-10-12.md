# Bellwing larva: facts and options (2026-10-12)

Unclassified exploration. Research is evidence, not decisions. Nothing here is decided;
the forks below are for Wrysk, one at a time.

Trigger (Wrysk 2026-10-11): seedporters may need a richer diet, "probably just more
insects"; "maybe bellwing have larva state as well and its like super abundant with few
reaching adulthood, roughly matching the numbers that reach adulthood today." Earlier
(memory note, 2026-10-06): a caterpillar stage is a maybe, food for young lanternjaws and
seedporters; the cheap route named then was a separate larva lineage that turns into a
bellwing at maturity. No ledger entry (D-0001..D-0006) touches bellwings or insects.

## 1. The bellwing today

Code: `crates/cubarium-voxel-fauna/src/body.rs:2549-2627` (built on the littershredder table
`body.rs:2038`), constants `body.rs:124,140,157,203`. Values below are the wide-world
expanded config of the check run (`runs/lab/1011-204110-nurse-groom-bae9/main/expanded.toml`).

- Body: `body_max` 0.06, hatchling `body_min` 0.006 (0.1 of adult), breeds and counts as
  adult at `birth_body` 0.03 (0.5 of adult). Adult = body >= birth_body
  (`crates/cubarium-observatory/src/query.rs:340-350`). Tissue `n_tissue` 0.02, inherited
  from the shredder (not set for the bellwing).
- Eggs: clutch up to 6, egg = hatchling + 1/6 reserve (0.007), laid on a latticevine face
  (`EggSite::Vine`, VINE-C), incubation 183 s, not dormant. The hatchling clings to the vine
  face and is already a small flyer (`step.rs:6142-6240`).
- The dormant egg bank and `hatch_draw` (`predation.rs:297`, `step.rs:6253-6330`, chance
  2/3 per dusk) are the **lanternjaw's**, not the bellwing's: footfall-gated dormant eggs that
  hatch a rider onto a host.
- Growth ceiling 2.68e-5 /s would reach adult in ~15 min; actual median hatch-to-adult is
  **~245 min** (all 16 runs: 240-253). Nectar carries mineral 0.01 per organic
  (`cubarium-voxel-flora/src/lib.rs:2188`) against tissue 0.02, so young are likely
  mineral-bound, as seedporter young are.
- Life span median 12 h (6 two-hour days). Diet: nectar only, yield 0.95; awake from an hour
  before sunrise to an hour after sunset; sees near-IR (flowers reflect it, the lanternjaw
  lure emits it; red-dwarf sun).

## 2. The numbers today (8 seeds, 8 h, wide world, branch = nursing arm; main is similar)

| per world per 8 h (median, range) | value |
| --- | --- |
| eggs laid | 323 (263-365), ~40/h, 98 % hatch |
| adults reached (matured) | **102 (91-138)** |
| hourly maturing | 0 in h0-3; a pulse of 40-84 in h4 (the founders' first clutches); **~10-24/h, about 15/h, in h5-8** |
| hatch-to-adult, cohorts hatched before 4 h | **72 %** (849/1175; 6 % still juvenile at 8 h) |
| population at 0/2/4/6/8 h | 15 / ~117 / ~136 / ~138 / ~265, still climbing |

Deaths, 8 seeds summed (533, ~67 per world): starved 135 (all juvenile); seedporter 165
(108 juvenile, 57 adult); Veilhound 120 (57 / 63); lanternjaw 91 (59 / 32); aged 22.
**"Reaching adulthood today" ≈ 100 per world per 8 h, ~15/h at steady state.**

## 3. Who eats insects now

- Seedporter (`body.rs:2680-2700`: `catch_bellwings`, `opportunist`, prey bellwing,
  littershredder, small lanternjaw): **11-28 bellwings per world per 8 h** (branch; main 11-36),
  not 60-100. It also kills 14-35 littershredders. Bellwing prey body summed ~0.53 organic per
  run (mean 0.025 each); shredder prey ~6.4. Seedporter lifetime intake per run: fruit ~4.0,
  carrion (its kills) ~2.8.
- Lanternjaw: 5-25 bellwings per run. Veilhound: 1-38 bellwings per run (it takes adults as
  often as young).
- Nitrogen: seedporter tissue 0.05; fruit 0.01; bellwing and shredder tissue 0.02. So even
  insects are **below** seedporter tissue here, unlike real insects (protein-rich, at or above
  vertebrate tissue N). Mineral intake today ≈ 4.0×0.01 + 2.8×0.02 ≈ **0.10 per world per 8 h**;
  need for young growth alone ≈ 14 young × (0.711 − 0.071) × 0.05 ≈ **0.45 per 8 h** (~0.056/h),
  milk's N on top coming out of the mother.

## 4. Machinery a larva could reuse

- Individuals: the clutch list and vine hatching; the lanternjaw rider (attached body,
  dormant bank, per-dusk hatch draw); snail shore eggs; the `LifeStage` split (Rider, Pup,
  Juvenile, Adult) already reported in vitals.
- Fields: flora stands carry `foliage` with per-layer `layer_stock`
  (`cubarium-voxel-flora/src/lib.rs:299-420`), nectar and fruit; ground cells carry litter,
  film, carrion and **a list of seed cohorts aged on a wheel** (`Ground::seeds`,
  `seeds.rs:351` `seed_bank_at`) — the nearest precedent for a per-cell "larva cohort" store.
- Cost: across 736 census rows, tick time rises ~0.1 ms per animal per tick (corr 0.53; mixed
  hosts, confounded with run time) on a ~30 ms tick at 20 Hz. 500 more bodies ≈ +50 ms/tick,
  roughly 2-3× slower. A cell field costs close to nothing.

## 5. Real-world anchors (recalled literature; not re-verified online this pass)

- Lepidoptera egg-to-adult survival is usually a few percent; first-instar mortality alone is
  often 50-90 %+ (Zalucki, Clarke & Malcolm 2002, Annu. Rev. Entomol. 47; Cornell & Hawkins
  1995, Am. Nat. 145).
- Most caterpillars feed on host-plant foliage; cutworms (Noctuidae) shelter in soil and
  litter by day; aquatic caterpillars are rare (Acentropinae).
- Larval period is commonly as long as or longer than adult life: Manduca sexta grows from
  ~1 mg to ~10 g (~10,000×) in ~3 weeks, adults live 1-2 weeks; moth eggs are ~0.1-2 % of the
  female (Garcia-Barros 2000, cited at `body.rs:2561`).
- Tamarins and marmosets get most protein from arthropods: insects are a large share of
  foraging time; orthopterans (katydids, grasshoppers) dominate tamarin prey, with
  caterpillars, beetles and spiders also taken (Garber 1993; Nickle & Heymann 1996).
  Marmosets add gum (exudates).

## 6. Budget

Target emergence ≈ 15/h per world. At survival s, larvae hatched = 15/s per hour:
s 5 % → 300/h, 3 % → 500/h, 2 % → 750/h (vs 40 eggs/h today).

- Egg budget: at today's egg cost (0.007) 500/h costs adults 3.5 organic/h of nectar, ~12×
  today. With moth-sized eggs (~0.01 of adult, 0.0006) 500/h costs ~0.3/h, the same as today.
- Larval tissue that dies (constant mortality, s 3 %, exponential growth from 0.0006):
  emerging at 0.006 (today's hatchling) ≈ 2.0 × eggs × egg mass ≈ **0.6 organic/h**;
  emerging at 0.03 (breeding-ready) ≈ 4.3× ≈ **1.3 organic/h**. Today seedporters take ~0.07/h
  of bellwing. Foliage eaten ≈ growth / yield (~0.5): ~1.5-3 organic/h per world (plant
  headroom not checked here).
- Nitrogen offered if seedporters take half of the dying larvae: at n 0.02 → 0.006-0.013/h,
  **~10-25 %** of the ~0.056/h young-growth need; at n 0.05 → 25-60 %; at n 0.10 → 55-100 %+.
  So a larva fills the gap only if insect tissue N is raised toward realistic values, or larval
  biomass is larger than "match today's emergence" implies.

## 7. Forks (recommendation for each, his call)

1. **Where larvae live, what they eat.** (a) Host-plant foliage (caterpillar), on stands'
   `layer_stock`: real, reachable by climbing seedporters, ties to plants. (b) Leaf litter
   (cutworm-like): reachable by ground foragers, overlaps the littershredder. (c) Water: rare
   in reality, out of seedporter reach. Recommend (a), probably on the latticevine and
   bloomcrown where adults already lay and feed.
2. **Individuals or a field.** (a) Individual larva bodies (a lineage that becomes a bellwing):
   drawable, sensable, but 300-750 hatched/h and hundreds standing cost ~2-3× sim speed.
   (b) A per-stand or per-cell larva density field (seed-cohort precedent): ~free, eaten by
   foraging the plant like fruit, readable as a smell or taste channel, needs no art at first.
   (c) Hybrid: a field, emerging as individual bellwings. Recommend (c).
3. **Who eats them.** (a) Seedporters only. (b) Plus lanternjaw young and littershredders (on
   litter). (c) Plus frondgrazers by accident when browsing infested foliage (a free
   field-only mortality). Recommend (b)+(c) for frondgrazers by accident: matches the
   2026-10-06 intent (young lanternjaws and seedporters) and gives the low survival a cause.
4. **Drawn or not.** Art is Wrysk's. A field can start undrawn or as subtle leaf damage.
5. **Larval tissue N.** Keep 0.02 (larva adds only 10-25 % of the seedporter need) or raise
   insect tissue N toward realistic (0.05-0.10). Recommend raising, since the gap is N, not
   energy; ask with fork 1.
6. **Alternative: more insect niches.** A second insect lineage (e.g. an orthopteran-like
   leaf-chewer on the ground layer, the tamarins' main prey) could feed seedporters without
   changing the bellwing; costlier (a new founder, brain, art) than a field larva.
