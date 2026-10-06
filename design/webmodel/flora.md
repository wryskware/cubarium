---
design_status: exploration
last_reviewed: 2026-10-05
decision_refs: []
---

# The whole-web model: the producer half

**Status:** calibrated against the sim's species ledgers and validated open loop on soak v2
(seeds 5 and 6 used in calibration, seed 1 held out), on patch, and on the maint and capfix
arms. The main stocks (springturf, velvetpad, lanternberry, vaulttree, glowcap) pass the bar
within a factor of 2. Bloomcrown, siphonreed and stonecushion fail on some seeds (see
Validation). Not canon.

Code: `crates/cubarium-webmodel/src/flora/`. Driver: `examples/flora_model.rs` (its usage
line lists the subcommands and levers). Tests: `tests/flora_laws.rs`.

**Start states are census files** (2026-10-05): `flora_model export SAVE OUT` writes a save's
census, read by the build that shares its schemas, to `OUT/faces.tsv` with the plant config it
was read under (`flora.toml`); `census::load` takes the census file in a directory, or the one
beside a save (`census-*/`) nearest the run's plant config, before the save itself. The soak and
patch saves (flora schema 30) have theirs, written by 170671be with the census commit 07e97f03
cherry-picked: `census-terrarium/` (and `census-ewater20/`, turf `establish_water_min` 0.2) beside
the soak saves, `census-patch/` beside the patch saves. A run that moves a gate, crown or shade
knob needs a census exported under it (the loader prints the keys that differ). Check: 24 model
days on s6 from the save and from its census at the export build print the same lines.

A world day (120 min) runs in about 0.13 s, and setup takes about 40 s.

## State and laws

| piece | what it is | sim source | what the aggregation loses |
|---|---|---|---|
| zones (`zones.rs`) | faces classed by material, highest-in-column, a sky band cut at the light gates (0.05/0.35/0.6/0.75/0.9), and wet (springturf's aeration or flooding gate fails in at least half the snapshots); 24–26 zones | gates `step.rs:4029` | adjacency between zones |
| weather | each census snapshot is one weather state, read per zone and species (`μ`, aeration target, gate share, the same on held faces). `Replay` draws a state every 60 min and starts in the start save's own state; `Frozen` holds the time mean | `establishment_gates_with_sky` | the spatial pattern of wetting; the 60-min period is assumed |
| stand law (`stand.rs`) | W, P, Q, parcel, aeration stress, cure clock, nectar, fruit, runner support, restated line by line | `grow_stand_cause` `step.rs:2924–3260`, `dormant` :3293, funding `seeds.rs:880` | layer stocks, defence, the stand's mineral inventory (taken as `n_tissue · material`) |
| cohorts (`model.rs`) | stands per zone and species, rebinned hourly by log-wood (16 bins) × vigour (`P/αW` cut at 0.1, 0.4) × site class (light × mineral factor in 4 classes, × water rank in 3) | – | variation inside a bin; a cohort's site is its stands' mean |
| **cohort site** (`Site`) | the faces a cohort stands on: terrain light `l_base`, shade per shading species `tau_ref` at the zone foliage `f_ref`, the parents' shade `tau_fixed`, mineral factor `n_rel`, water rank `w_rel`. Light now is `sky · l_base · e^−τ`: a species' shade drops in proportion as its zone foliage shrinks, and grows by the zone's mean rate per unit foliage (`kappa`) as it grows. A non-cover stand past `donor_min` drops the covers' and its parents' shade. Mineral is the pool mean × `n_rel`; water is the held level × `w_rel` | light sweep `lib.rs:2857`, `canopy_light` `step.rs:3954` | the shade a stand's own growth escapes below `donor_min`; where in the zone new crowns land |
| **establishment** | per zone, species and occupant class (bare, or a cover it may take): seeded share × sampled faces' pass share × the sim's conversion rate × the fitted persistence `φ`. Bare faces are taken within two checks by weighted lottery (`seeds.rs:183`, :665). Covers are taken at `overtop_per_check / seed_check_s` (`seeds.rs:80`, :466) | `seeds.rs` | which faces carry the bank (beyond the clumping fit) |
| **sampled faces** (`zones::Sample`) | up to 48 faces per zone, state, species and kind (seeded, in a donor's runner reach, or gate-passing): terrain sky, `μ`, the optical depth over the newborn's crown top per shading species (the census's own `canopy_at` sweep), the gate pass, the face's mineral over its occupant's mean, its water, its occupant. A face passes the canopy gate when its mineral, moved with its class's mean, exceeds the newborn's break-even `N*` at its own light and `μ` | `canopy_pays` `step.rs:3899`, `break_even_cover` :3872 | faces outside the sample (48 per zone, an even stride) |
| seed shadow | `P(face seeded) = 1 − (1 + m/k)^−k`, `m` = bank per face, `k` fitted per zone and species on the faces its seed may take | – | where in the zone the pile sits |
| water seeds | siphonreed's bank spreads over the gate-passing faces only (margins), and its newborns read those faces | `water_target` `seeds.rs:1539` | the hop distance |
| runners | `clonal_share`; a donor's draw finds a face with prob `1 − (1 − θ)^n`, `θ` = Σ over classes of the zone share × the runner-reach samples' pass share × the box enrichment by occupant; otherwise the package is kept (`seeds.rs:913`); the parent supports the daughter for 1 h | `seeds.rs:1078`, `step.rs:2525` | the box's actual contents |
| dispersal (`kernel.rs`) | zone-to-zone landing matrices sampled over the world's faces (Drop, Wind Lomax, Wald, Spores with gate screening, dark spores) | `seeds.rs:1168–1524` | donors' exact positions |
| bank decay | one rate, `1 / ((1 − e⁻¹) · seed_max_age_s)` | `seeds.rs:745` | the shape of the age cut-off |
| mineral | a site pool per zone and occupant; packages export their mineral; dead seeds return it where they rot | `step.rs:3213`, `seeds.rs:953` | fungal translocation |
| fungus | substrate feed from the zone's litter and dead wood, yield, seep in the dark | `feed` `step.rs:2183`, `saprotrophy` :2898 | the 5×5 mycelium box: every fungus reaches its whole zone's pool, so litter swings with the weather state |
| detritus | litter, dead wood, carrion, film (logistic) | `step.rs:3659`, `film.rs` | the film's spread and its dry-site death |
| exchange | `supply()`, `take`, `returns`, applied each step | `lib.rs:3792` | the take's zone and per-species split |

**Drowning is not represented.** The sim drowns 200–600 velvetpad per 90 min in the wet
zones (the `sp` probes). The model has no standing-water depth per face over time. Pad
turnover in the wet zones is therefore lower, but the pad stock still holds (see
Validation).

## Fitted rates (spatial or emergent, never sim knobs)

| rate | value (spread) | form | from |
|---|---|---|---|
| seed clumping `k` | per zone and species. Global fits, s5 / s6 / s1: lanternberry 0.026 / 0.022 / 0.022, velvetpad 0.015 / 0.010 / 0.018, stonecushion 0.032 / 0.026 / 0.007, springturf 38 / 15 / 6.8 (near well mixed); s5 snapshot spread for lanternberry 0.015–0.026 | negative binomial on the takeable faces; per zone, least squares on the share, so empty shadows count | each run's own censuses (`fit::clumping`, `clumping_by_zone`) |
| self-shade `β` | lanternberry 0.56 ± 0.06 / 0.39 ± 0.08 / 0.25 ± 0.07 (s5 / s6 / s1, the ± its standard error), velvetpad 0.47 / 0.42 / 0.66; 0.5 where fewer than 5 points | own shade over the seeded faces ∝ (zone foliage)^β | each run's own censuses (`fit::self_shade_beta`) |
| establishment persistence `φ` | lanternberry 0.33 (0.24–0.49), velvetpad 0.30 (0.14–0.63), springturf 0.99 (weather-driven, 0.2–12), bloomcrown 0.41 (0.1–1.25), vaulttree 0.86 (0.56–1.2), stonecushion 0.19, umbrellafrond 0.29, siphonreed 0.31 (pooled) | sustained germination per eligible face ÷ the sim's conversion rate | `sp` probes e5, e6, l5, l6, minutes 30–90 after load (the load burst left out), against each start's census funnel |
| runner box enrichment | per zone, species and occupant | box share ÷ zone share | censuses |
| `kappa` | per zone, receiver and shader | Σ τ ÷ Σ foliage | censuses |
| mineral log-sd σ | 0.08–0.42 by occupant | carried over from the first pass | censuses m250, m370 and m1570, seeds 5 and 6 |
| juvenile and adult light `a` | fallback only, now that cohorts carry their own light | – | – |

## Calibration (the brief's step 1), with evidence

The `sp` ledger lines of the four 90-min probes are e5/e6 (soak saves m370, seeds 5 and 6)
and l5/l6 (v2 saves m1460). The census funnel (`flora_model elig`, `rcheck=SPECIES:seed`)
counts, at each start, the faces where a newborn could come up now.

**Finding 1: the canopy gate is the newborn's own light, not the zone's.** Seeds pile under
their parents, so the seeded faces are shaded and drained.
- At s5-m1460, lanternberry has 726 seeded cover faces. 233 of them pass the water and light
  gates, and only 12 pass the canopy gate. At terrain sky alone 161 would pass.
- Those 12 faces × 0.1/30 s × 5400 s give 216 germinations. The sim's l5 had 209.
- The seeded faces' mineral (median 0.53) is far below the class mean (0.92).
- **What blocks the newborns.** The first pass's suspect was ground covers shading woody
  newborns. That holds early: at m370, turf and pad give τ = 0.44 of 0.72 over lanternberry
  newborns. Late, the parents' own shade dominates: τ_lanternberry = 0.48 of 0.67 at m1460.
- The newborn-gate light equals the measured light of the census juveniles (ratio 1.00 for
  W < 0.4).
- Hence the per-face samples, the per-zone clumping fit, the per-class mineral, and the
  `β` self-shade.

**Finding 2: a merge bug killed newborns.** Merging two newborns at `alive_min` gave a
weighted mean a rounding below `alive_min`, and the death rule then removed the whole
cohort. This drove the model's pad and turf churn: pad starved 2,150 per 90 min where the
sim had 212. The merge now keeps every state inside its inputs' range. The test fails on the
old merge.

**Finding 3: within-zone spread decides survival.**
- The rebin averaged dying stands into fed ones, so cohorts are now also split by vigour and
  site.
- Turf survival is a threshold in light × mineral: a newborn at L 0.3 and N 0.5 dies, at
  N 1.0 it lives.
- Holding each census stand's own conditions for 2 h kills 230–870 turf. That matches the
  sim's 146–871 starved per 90 min. Zone means kill far fewer.
- Who dies (censuses s5 m250→310, 310→370, 1460→1520): the turf that vanished within an
  hour had W 0.007–0.015 and P ≈ 0.002, at light 0.31–0.46 against 0.59–0.74 for the
  survivors, on the same mineral. **The turf's losses are juveniles in shade, not mature
  turf.** This corrects the first pass.

**Finding 4: the held-face moisture change.** The level stays the mean over held faces. Each
cohort now carries its faces' rank. The held-out seed decides it:

| s1 at m1090 | sim | with rank | held mean only |
|---|---|---|---|
| velvetpad | 5949 | 6366 | 4250 |
| stonecushion | 1112 | 1578 | 3403 |
| springturf | 1719 | 1281 | 964 |

**Finding 5: siphonreed.** Its seeds float to standing-water margins (`water_target`).
Treating them as dropped diluted them over the zone. Its germination now matches (e5 13/15,
l6 12/12). Its stock still fails (see below).

**Other fixes:**
- Samples hold only faces the species may take. Turf runners had been counting turf faces.
- Runner box enrichment.
- The first weather state is the start save's own.
- Faces that a later snapshot lacks are skipped.

**Flows per 90 min:** sim / first model / calibrated model.

| flow | e5 | e6 | l5 | l6 |
|---|---|---|---|---|
| lanternberry germinated | 161 / 1105 / 119 | 113 / 907 / 375 | 209 / 2139 / 121 | 295 / 3038 / 220 |
| vaulttree germinated | 0 / 6 / 1 | 4 / 21 / 5 | 108 / 223 / 91 | 72 / 348 / 78 |
| bloomcrown germinated | 19 / 59 / 16 | 20 / 72 / 12 | 33 / 56 / 17 | 48 / 242 / 27 |
| springturf germinated | 657 / 324 / 904 | 167 / 194 / 2 | 63 / 308 / 75 | 62 / 594 / 54 |
| springturf runner births | 325 / 1917 / 616 | 890 / 1200 / 3 | 145 / 669 / 537 | 138 / 869 / 97 |
| springturf starved | 558 / 1439 / 663 | 871 / 816 / 582 | 146 / 996 / 144 | 232 / 1119 / 233 |
| springturf overtopped | 42 / 291 / 26 | 19 / 183 / 25 | 85 / 172 / 83 | 80 / 416 / 49 |
| velvetpad germinated | 853 / 2707 / 324 | 1427 / 2720 / 688 | 521 / 1912 / 201 | 344 / 2523 / 199 |
| velvetpad starved | 212 / 2150 / 320 | 227 / 2043 / 306 | 55 / 1799 / 68 | 41 / 2698 / 44 |
| velvetpad displaced | 69 / 1343 / 603 | 76 / 798 / 1 | 107 / 522 / 440 | 78 / 649 / 95 |
| stonecushion germinated | 24 / 8 / 167 | 41 / 11 / 166 | 52 / 379 / 13 | 83 / 132 / 104 |

- Turf runners and turf germination swing with the weather draw: the e6 start state has no
  turf face passing.
- The sim's first 30 min after a load are a burst, 2–5× the later rate. `φ` is fitted on the
  later hour.

## Validation (open loop)

### Soak v2

- Start: m370. Take clamped to the log. Replay weather.
- Model: mean [min, max] over 4 weather seeds, with the ratio to the sim.
- Seeds 5 and 6 are calibration seeds; their probes and censuses fed the fits. Seed 1 is
  held out: none of its data was used.
- The sims' own spread at m1090 across s5/s6/s1:
  - turf 1663–1724, pad 5949–6953, lanternberry 631–1750, vaulttree 66–227;
  - stonecushion 662–1112, bloomcrown 95–330, umbrellafrond 506–1447.

| stock | s5 m1570 | s6 m1570 | s1 m1090 (held out) |
|---|---|---|---|
| springturf | 1040 \| 1860 [1271, 2537] ×1.79 | 997 \| 1033 [739, 1218] ×1.04 | 1719 \| 1281 [153, 2179] ×0.75 |
| velvetpad | 6401 \| 6543 [5541, 6987] ×1.02 | 6002 \| 6590 [5988, 6949] ×1.10 | 5949 \| 6366 [5980, 6946] ×1.07 |
| lanternberry | 2412 \| 2898 [2673, 3011] ×1.20 | 2383 \| 2506 [2226, 2784] ×1.05 | 631 \| 662 [575, 752] ×1.05 |
| vaulttree | 445 \| 763 [712, 832] ×1.71 | 472 \| 631 [452, 925] ×1.34 | 66 \| 6 [5, 6] ×0.08 |
| stonecushion | 786 \| 362 [270, 526] ×0.46 | 944 \| 890 [674, 1082] ×0.94 | 1112 \| 1578 [1288, 1781] ×1.42 |
| bloomcrown | 182 \| 50 [31, 71] ×0.27 | 400 \| 75 [69, 84] ×0.19 | 330 \| 214 [190, 236] ×0.65 |
| umbrellafrond | 619 \| 797 [622, 1014] ×1.29 | 584 \| 1471 [1353, 1696] ×2.52 | 1447 \| 1439 [1395, 1496] ×0.99 |
| siphonreed | 28 \| 3 [0, 11] ×0.11 | 206 \| 50 [47, 54] ×0.24 | 150 \| 43 [41, 45] ×0.29 |
| glowcap | 10227 \| 10590 ×1.04 | 10836 \| 11223 ×1.04 | 10361 \| 10460 ×1.01 |

For comparison, the first pass at s5 m1570: turf 4, stonecushion 42, pad 3484, lanternberry
8153, vaulttree 296.

**Bar, stated factor 2.**
- **Pass:** springturf, velvetpad, lanternberry and glowcap on all three seeds, including
  the held-out one. Vaulttree on s5 and s6.
- **Fail:**
  - Vaulttree on s1: 7 stands at the start; the juveniles fail to mature on the dry summit.
  - Stonecushion on s5 (0.46×).
  - Bloomcrown on s5 and s6 (0.19–0.27×, falling late). Its germination runs at about half
    the sim's in l5/l6.
  - Umbrellafrond on s6 (2.5×).
  - Siphonreed everywhere (0.1–0.3×). Its germination now matches, so its loss lies in
    runner support or growth at the margin.
- **Secondary stocks:**
  - Standing edible foliage runs 1.5–2.2× the sim. Turf builds 1.7× the sim's foliage
    (`flux` built per 2 h: 18.6k against 10.7k); pad and lanternberry match early.
  - Litter swings 0.15–3× with the weather state, because every fungus feeds on its whole
    zone's pool.
  - Turf at s1 m1090 has the widest weather spread (153–2179).

### Patch

Seeds 1 and 3, founded under the soak build to m120, then 4 h. Model / sim at m360:

| stock | s1 | s3 |
|---|---|---|
| springturf | 884 / 1033 | 687 / 1246 |
| velvetpad | 2209 / 1867 | 1637 / 1122 |
| lanternberry | 131 / 178 | 79 / 106 |
| umbrellafrond | 190 / 453 | 313 / 331 |
| stonecushion | 65 / 39 | 91 / 64 |
| glowcap | 3184 / 3085 | 3383 / 3226 |

- Stocks are within 2.5×; turf on s3 is 0.55×.
- Pad recruits through runners where the sim recruits through seed: model / sim, 1227 / 341
  runners and 582 / 1363 seedlings (s1).

### Maint arms

- **The comparison.** Turf maintenance arms (sim per seed, from saves founded at the old 5×
  maintenance) against the model from the soak-build patch foundings (founded at the current
  maintenance). The maint saves are flora schema 29, so main refuses them; nothing was
  migrated.
- **Turf stands at m360**, change against the m020 arm (the current value):

  | arms m100 / m050 / m010 | sim | model |
  |---|---|---|
  | s1 | −44 / −25 / +6 % | −77 / −42 / +12 % |
  | s3 | −41 / −20 / +10 % | −67 / −37 / +39 % |

- **Pad:** sim +23 / +13 / −5 %; model +11 / +8 / −1 % (s1), about flat on s3.
- **Sign:** right everywhere.
- **Size:** turf answers about 1.7× too steeply, pad about half.

### Capfix arms

- Each arm runs from its own founding saves; the current config is the slow arm.
- At m240 the model gives:
  - glowcap stands: main < fast < slow on both seeds, as in the sim;
  - caps: main ≈ 0, as in the sim (0.2 / 0.1);
  - fast against slow caps: 200 / 255 (s1) and 120 / 230 (s3). The sim has 116 / 168 and
    299 / 208, and fast ≈ slow over 8 seeds.
- The main-arm stand deficit is half the sim's: −9 % and −6 % against −21 % and −12 %.

## Succession (the brief's step 4)

- From m370 (the grass stage).
- 100 world days, 2 weather seeds each, plus 300-day runs for the end state.
- Levers as scenarios: `hold=springturf` (its stands read the start mean, the pool topped up
  from outside), no woody overtopping (`set=flora.{lanternberry,vaulttree,bloomcrown}.overtop_per_check=0`),
  and `browse=`.

**Does the model reproduce the succession?** Yes. The order is turf → lanternberry on drained
ground, vaulttree on the summit, and stonecushion on dry ground. Pad and umbrellafrond hold
the wet zones.

**Grass-stage length**: world days from m370 until world turf falls under 50 / 25 / 10 % of
its m370 count.

| seed | weather seed 1 | weather seed 2 |
|---|---|---|
| s5 | 35 / 45 / 70 | 25 / 30 / 55 |
| s6 | 10 / 15 / 25 | 15 / 20 / 25 |
| s1 | 20 / 25 / 45 | 15 / 25 / 40 |

The sim halved its turf by about day 10 on both s5 and s6. The model's s5 is about 2× slow
(its turf is 1.8× at day 10), so the s6 numbers (halving at 10–15 days, under 10 % by about
25) are the better estimate.

**End state** (steady by about day 150; share of the zone's faces; s5 and s6 alike unless
noted):
- **mid, bright and open drained ground:** lanternberry 84–100 %.
- **summit-drained:** vaulttree 77–99 %, with stonecushion 17 % on s6.
- **open-wet and summit-wet:** umbrellafrond 53–100 %, the rest pad.
- **mid-wet, bright-wet and the rock-wet zones:** velvetpad 59–95 %, with umbrellafrond.
- **roof-dim:** pad or bare. **roof-dark:** bare (glowcap only).
- World totals at day 300:
  - s5: lanternberry 6,275, pad 3,410, umbrellafrond 1,563, vaulttree 967, turf 0;
  - s6: lanternberry 4,885, pad 3,142, umbrellafrond 2,701, vaulttree 1,246, turf 0.
- **The dry seed (s1)** ends differently:
  - stonecushion takes open-drained (85 %) and summit-drained (77 %);
  - pad and lanternberry share mid and bright drained (pad 54–58 %);
  - bloomcrown rises to about 1,100.

**The turf's lasting place.**
- None on s5 or s6: zero by day 150.
- On the dry s1, a remnant of 100–240 stands (8 % of summit-drained) holds to day 300. That
  ground is dry, open summit, where lanternberry cannot establish (water) and only
  stonecushion and sparse vaulttree compete.
- With a woody-seedling browser at 2 juveniles per day (the scenario lever), turf holds
  2,035 (s5) and 364 (s6) at day 100, against 155 and 0 without it. That browser eats about
  550 woody seedlings per world day.

**Self-depletion against overtopping.**
- Holding turf's site mineral at its start mean does not slow its decline. The share of the
  base decline removed is −0.37 to +0.15 across seeds and days 10–40, mostly negative.
- **The mechanism:** the drained turf faces fail the canopy gate for woody seedlings (the
  newborn's break-even needs mineral). So the turf's self-depletion shelters it from
  overtopping more than it starves it.
- Removing woody overtopping removes 58–100 % or more of the decline on s5 and 10–35 % on
  s6. On s1 it removes none: turf starves on the dry hill and stonecushion takes the gaps.
- Turf deaths over 100 days (s5): starved 18,587 (mostly juveniles in shade), overtopped
  8,059 (lanternberry 5,538, vaulttree 2,371).

## Supply for the animal half (the brief's step 5)

From four soak states, 2 world days, frozen weather, no take.

Per world day per zone: **caps built (the most they could regrow if grazed to nothing,
`foliage_rate · W`) / litter input** (shed, deaths, rotting seeds).

| zone | s5 m370 | s5 m1570 | s6 m1570 | s1 m1100 |
|---|---|---|---|---|
| bright-drained | 392 (447) / 9.1k | 224 (447) / 12.1k | 156 (311) / 8.3k | 202 (396) / 5.0k |
| bright-wet | 130 (151) / 6.0k | 75 (151) / 6.6k | 118 (237) / 10.3k | 28 (52) / 4.9k |
| mid-drained | 258 (295) / 5.7k | 148 (295) / 6.1k | 158 (311) / 6.8k | 271 (531) / 7.7k |
| mid-wet | 96 (114) / 3.6k | 57 (114) / 4.0k | 95 (190) / 7.0k | 27 (52) / 2.5k |
| open-drained | 250 (283) / 12.0k | 142 (283) / 9.9k | 101 (203) / 6.6k | 154 (291) / 5.0k |
| open-wet | 80 (92) / 7.4k | 46 (92) / 8.4k | 41 (81) / 6.1k | 10 (17) / 4.9k |
| summit-drained | 148 (168) / 8.9k | 84 (168) / 6.3k | 126 (251) / 10.5k | 8 (170) / 3.2k |
| summit-wet | 30 (34) / 2.2k | 18 (34) / 2.3k | 22 (41) / 2.7k | 3 (6) / 0.6k |
| roof-dim | 138 (162) / 1.8k | 106 (162) / 2.4k | 119 (180) / 1.8k | 72 (116) / 0.6k |
| roof-dark | 53 (61) / 1.8k | 30 (61) / 2.7k | 51 (103) / 3.9k | 25 (49) / 1.0k |
| **world** | 1595 (1830) / 60.1k | 943 (1830) / 62.8k | 1005 (1939) / 65.7k | 831 (1739) / 36.8k |

| world | s5 m370 | s5 m1570 | s6 m1570 | s1 m1100 |
|---|---|---|---|---|
| film grown / upkeep per day | 5.2 / 4.2 | 6.2 / 4.4 | 8.0 / 5.7 | 11.4 / 8.0 |
| film standing / capacity | 6.2 / 7.9 | 7.0 / 9.3 | 9.2 / 12.3 | 13.2 / 16.9 |
| caps standing / full | 2116 / 2118 | 2117 / 2118 | 2243 / 2245 | 1939 / 2013 |
| litter standing | 398 | 489 | 1059 | 2556 |

- **Caps.** Ungrazed caps sit full, and "built" is then the replacement of senescence. A
  grazer can draw up to the bracketed potential: about 1,700–1,900 per world day for the
  world. The capgnawers took about 1.5 per world day in the soak.
- **Film.** Net film production is 1–3 per world day.
- **Litter.**
  - Input is 37–66k per world day against 0.4–2.6k standing. The fungi and decomposition
    clear it within hours.
  - The model's turf foliage is 1.7× the sim's, so its litter input is high by up to that
    factor.
  - The sim's foliage shed is about 20k per world day (s5 early `flux`). The model's input
    adds rotting seeds and deaths to its shed.

## Closed loop: what the plant half offers and takes per step (the brief's step 6)

**Offers (`Supply`, world totals, the sim's tissue units):**
- `foliage[species]`: edible foliage above each layer's grazing floor, through the sim's own
  `layers_of(...).edible()`. The glowcap's entry is its caps.
- `litter`: summed over zones.
- `carrion`: its own pool, decaying at `carrion_decomposition`.
- `film`: above `film_graze_refuge · capacity`.
- `nectar`: bloomcrown stock. `fruit`: lanternberry stock.

**Takes back:**
- **`Take`, same fields:**
  - Foliage comes off each species' cohorts in proportion to their edible foliage above the
    refuge.
  - Caps come off the glowcap cohorts.
  - Litter comes off each zone in proportion to its pool.
  - Carrion and film come off their pools; film only above the refuge.
  - Nectar and fruit come off in proportion to the stock.
  - The mineral in the bites leaves (`taken_mineral`).
- **`Returns`:**
  - `litter` (dung): spread over the zones by area.
  - `carrion` (kills not eaten whole): into the carrion pool.

**What the closed loop needs that `exchange.rs` lacks** (for the parent):
1. **Where.** Supply and take per zone, not world totals. Grazers and capgnawers are not
   spread uniformly, and the plant half's zones differ by an order of magnitude in what they
   offer.
2. **The mineral returned in dung and carcasses.** The plant half loses `taken_mineral` and
   gets organic matter back without it. That is a slow leak over a long closed run.
3. **Per-species foliage take** as the animal half's own diet split. The soak log had only
   the total, spread here by edible share.
4. **Seedling browsing** as a take on woody juveniles: the `browse=` lever is its stand-in.

Meanwhile the module works around all four inside itself.

## Roles (Balance through roles, 2026-10-05)

| stock growing or shrinking without check | the role that should answer |
|---|---|
| woody (lanternberry) taking every drained face; turf gone by about day 25–70 | a woody-seedling browser. The scenario says one eating about 550 seedlings per world day keeps a lasting turf; no animal fills that role now |
| glowcap at its space cap, with 1,700–1,900 caps per day available against about 1.5 eaten | the capgnawer under-fills its role (its gut and breeding are the animal half's question) |
| umbrellafrond spreading over the wet zones (2.5× on s6) | no browser takes it; the model may also overstate it |
| litter input of tens of thousands per day | decomposers and fungi already answer; the model's fungus over-reaches (zone-wide feed) |

## Tests (`tests/flora_laws.rs`, 11, 0.05 s)

1. **Law anchor:** the stand law at the sim's tick equals `study::probe` to 1e-9 for 5 species
   through dusk and night; the plant clock equals `DayClock::sun`.
2. **Conservation:** the organic and mineral ledgers close over 1.5 days with every mechanism
   on, including the cohort sites, the water rank and the newborn samples.
3. **Config flow:**
   - turf maintenance up → fewer packages;
   - propagule rate up → never fewer;
   - a woody `overtop_per_check` of 0 → no turf overtopped;
   - glowcap `foliage_rate` up → caps regrow faster;
   - a stricter `establish_light_margin` → fewer woody seedlings through a shaded mat.
4. **Mechanisms on hand-built inputs:**
   - merged newborns never round below `alive_min`. This fails on the old merge;
   - the seed-shadow fit returns the census's share;
   - a water seed comes up only where its gates pass;
   - the hold lever keeps the pool at its start mean.

An independent test-authoring pass, at high effort, is still owed on the new mechanisms.

## Left

- **Bloomcrown:** germination at about half the sim's, and a late decline.
- **Siphonreed:** its runners and growth at the margin.
- **Stonecushion:** the s5 deficit.
- **Litter:** the fungus feed's zone-wide reach (a per-face feed share would fix it).
- **Drowning.**
- **Turf's foliage build:** 1.7× the sim's.
- **The maint response:** about 1.7× too steep.
- **More weather snapshots per seed:** s1 has 3. More would narrow the replay's spread.
