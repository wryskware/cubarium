---
design_status: exploration
last_reviewed: 2026-10-04
decision_refs: []
---

# Plant succession study: results (base, p = 0.01, p = 0.03, arm G)

This note reports the study in `design/handoffs/plant-succession-study-2026-10-03.md`. It is
judged against the S1–S4 definitions in Astra's review
(`design/7_Research/astra-plant-succession-review-2026-10-03.md`), and it is evidence only, not
a decision. The code is on branch `succession`, head f852e00d, not merged. The simulations
were run by the earlier worker. This note adds the analysis, the too-damp readout and a
re-run of the single-stand drought probe. No new ecological runs were made.

## What was run

- **Binary.** Every ecological run used `succession_study`, built 2026-10-03 22:07. That
  build predates 74015627, so "base" is the earlier physiology with
  `springturf.overtop_per_check = 0`.
- **Arms.** Each arm is a `sets=` file:
  - p01 and p03 set the overtop probability to 0.01 or 0.03, restricted to `["velvetpad"]`;
  - G is the redesign plus p = 0.03. Springturf: alpha 4, wood_max 0.3, graze_refuge 0.4,
    foliage_rate 0.0204, wilt_water 0, full_water 0.3, rooting_depth 2,
    establish_saturated_max 0.05, senescence 0.0026, seed bank ×2. Velvetpad: alpha 2.
- **Branch defaults differ from G.** 74015627 put G's physiology into the code defaults and
  the four presets with the overtop rate still at **0**. So "G plus p = 0.03" is the branch
  defaults plus one `set=` line.
- **Seeds.** Eight: 1, 3, 5, 6, 8, 9, 12, 14.
  - Per seed: one shared plants-only founding to minute 120, then each arm branched for
    4 h, plants only, to minute 360.
  - The p01 arms on seeds 8, 9, 12 and 14 were stopped at minutes 180–270. p01 is judged on
    seeds 1, 3, 5 and 6 only.
- **Fauna.** 2 h with the code-default founding table, heuristics, `threads=1`:
  - *established*: each arm's own m360 save;
  - *transition*: the shared m120 save, run under the arm's config.
  - All 96 fauna runs placed the full cohort, with shortfall 0.
- **Cohorts are shared.** They are fixed at the m120 load from the save's own config, so they
  are identical across arms, with one exception. G's cohort differs by one site on s6
  (1301 against 1300) and on s8 (815 against 814). That is negligible, but it is not
  explained.
- **Analysis.** `ana_plants.py`, `ana_fauna.py`, plus `ana_summary.py` and `ana_fate.py`
  written for this note. All four are in the session scratch directory
  `scratchpad/succession/`, with the raw outputs in `collect/`.

## S1–S4 verdicts

| | base | p01 (s1,3,5,6) | p03 | G |
|---|---|---|---|---|
| S1 (≥ 0.40 by 6 h in ≥ 3/4 of seeds 1,3,5,6) | **fail** 0/4 | **fail** 0/4 | **fail** 0/4 (1/8 over all eight) | **fail** 0/4 (0/8) |
| S2 refuge ratio ≥ 0.70 in every seed | pass (1.000, by definition) | pass (min 0.991) | pass (min 0.985) | pass (min 0.975) |
| S2 births ≥ deaths, h4–6 (judged against base) | fails in every seed | as base | as base, deaths ×1.04 | as base, **deaths ×1.27** |
| S3 drained photosynthetic ≥ 0.90 of base in ≥ 3/4 | pass by definition | pass 4/4 | pass 4/4 (8/8) | pass 4/4 (**7/8**, s14 0.886) |
| S4 focal median ≥ 0.90, ≥ 7/8 ≥ 0.80, both comparisons | – | not run | **pass** | **pass** |

### S1: succession

S1 is the share of the hour-2 S1 cohort that has turf at donor size by 6 h, still turf or now
woody. The cohort is 24–36 % of velvetpad sites per seed.

| seed | p01 | p03 | G |
|---|---|---|---|
| 1 | 0.157 | 0.224 | 0.195 |
| 3 | 0.171 | 0.197 | 0.045 |
| 5 | 0.173 | 0.187 | 0.048 |
| 6 | 0.283 | 0.354 | 0.175 |
| 8 | (stopped) | 0.376 | 0.227 |
| 9 | (stopped) | 0.105 | 0.042 |
| 12 | (stopped) | 0.036 | 0.071 |
| 14 | (stopped) | **0.607** | 0.257 |

- **Base.** 0 in every seed. Direct velvetpad→woody replacement is 41–102 cohort sites per
  seed; this is reported separately and does not count as a turf stage.
- **The main cause is seed arrival, not probability.** The S1 cohort over eight seeds is
  8,834 sites. By reach to the nearest turf donor at hour 2:

  | reach (voxels) | share of cohort |
  |---|---|
  | ≤ 6 | 19 % |
  | 6–12 | 10 % |
  | 12–24 | 24 % |
  | > 24 | 47 % |

  In p03, 60 % of the cohort saw no arrival in 4 h; in G, 53 %. Within 6 voxels, p03
  succeeds on 1560 of 1683 sites (93 %). Even perfect success within 12 voxels would cap S1
  near 0.29. The 40 % target is out of reach at the current hop and with no runners,
  whatever p is. This is the outcome Astra's rule sends to "study dispersal and runners
  next".
- **G adds a second failure: turf dies after entry.** Over all eight seeds:

  | | p03 | G |
  |---|---|---|
  | successes | 2366 | 1205 |
  | post-birth losses | 153 | **1621** |
  | arms | 2976 | 5584 |
  | arms lost before sprouting | 26 | **1365** |

  G's newborn turf needs L ≥ 0.6, against 0.5 before, according to the 74015627 viability
  grid. The S1 cohort's light band is still L ≥ 0.5. G's establish_saturated_max of 0.05
  also closes more arms at sprouting. Which of the two causes the post-birth losses has not
  been isolated. This is Astra's "address establishment versus adult light" case.

### S2: refuges

- **Occupancy holds.** Velvetpad occupancy of the shared wet and shaded cohort at 6 h is
  0.975–1.007 of the paired base in every seed and every arm. There are 1,282–2,786 surviving
  donors per seed.
- **Births ≥ deaths fails everywhere, base included.** Refuge totals over eight seeds,
  h4–6:

  | | births | deaths |
  |---|---|---|
  | base | 72 | 587 |
  | p03 | 64 | 605 |
  | G | 68 | 745 |

- **Judged against base**, p03 is flat (median deaths 1.04 × base). G loses refuge pads
  faster (median 1.27 × base). The likely cause is velvetpad's alpha being halved, but this
  was not isolated. Wet refuges are tiny: 0–135 sites per seed, almost all of them unchanged.

### S3: drained ground

- **Photosynthetic occupation** of the fixed drained cohort at 6 h:
  - p01 and p03: 0.992–1.007 of base;
  - G: 0.886–0.980 (s14 0.886, s8 0.908).
- **G shifts drained ground towards fungi**, for example s3 1243→1832 and s8 1345→2038 fungal
  sites.
- **Occupancy passes, but S3's food guard does not support G.** See Food.

### S4: fauna

The tables give the median of the paired B ratios, with the number of seeds at or above 0.80.

**Focal lineages:**

| lineage | comparison | base B → p03 B | ratio med (≥ 0.8) | base B → G B | ratio med (≥ 0.8) |
|---|---|---|---|---|---|
| littershredder | established | 3.30 → 3.29 | 1.013 (8/8) | 3.30 → 3.45 | 1.039 (8/8) |
| littershredder | transition | 2.52 → 2.57 | 1.082 (8/8) | 2.52 → 2.25 | 0.964 (7/8) |
| frondgrazer | established | 5.03 → 4.64 | 0.943 (8/8) | 5.03 → 4.57 | 0.957 (7/8; s9 0.79) |
| frondgrazer | transition | 4.34 → 4.29 | 0.955 (8/8) | 4.34 → 4.59 | 1.014 (8/8) |

- No starvation in any focal arm.
- Focal B stays above 1 throughout, so they are not underfunded.
- In G established, frondgrazer's late window holds only 5/8 at ≥ 0.80 (median 0.954).

**The rest of the roster**, as "all" ratios:

| lineage | p03 established | p03 transition | G established | G transition |
|---|---|---|---|---|
| capgnawer | 0.895 (8/8) | 0.977 (8/8) | 1.013 (8/8) | 0.958 (8/8) |
| loftstrider | 0.999 (8/8) | 1.003 (8/8) | 1.018 (8/8) | 1.085 (8/8) |
| ripple-snail | 0.936 (8/8) | 1.104 (7/8) | 1.050 (8/8) | 0.994 (8/8) |
| bellwing | **0.706 (3/8)** | 0.933 (7/8) | 1.026 (5/8) | 0.817 (4/8) |
| seedporter | 0.868 (5/8) | 0.888 (7/8) | **0.798 (4/8)** | 1.035 (7/8) |
| Veilhound | 0.905 (4/8) | 1.533 (7/8) | **0.443 (1/8)** | 1.021 (6/8) |
| lanternjaw | dormant eggs, B 0 | | | |

- **Bellwing starvation:** G established 0→2, G transition 2→2, p03 transition 2→1.
- **Veilhound** fed only on carrion in these runs. The pack is two to four bodies, so its B
  is noisy. In G established its median falls from 1.69 to 0.91, access from 0.13 to 0.04,
  and two seeds had no carrion bites at all. This is not a plant-food signal, but it is a
  repeated deficit in G established. It needs a look before anyone calls the roster
  unaffected.
- **The fauna window cannot see a halving of food stocks.** Focal foliage consumption in 2 h
  is about 200 ou per window, summed over eight seeds. The standing edible stock is about
  1,900–3,600 ou per world. S4 passing in G therefore does not show that G's food loss is
  harmless over longer horizons.

## Too damp: turf pushed out, velvetpad retaking

**How it was measured.** `succession_study fate from=runs/sN/m120 to=runs/sN-ARM/m360` was run
for base, p03 and G on all eight seeds: on the desktop for s12 and s14, on eidolon for the
rest, with the f852e00d build.

- **"Wet"** means turf's aeration or standing-water gate fails on a face that is not
  flooded past velvetpad's drown depth. **"Soggy"** means velvetpad's box is saturated
  ≥ 0.6.
- **Both classes are read with the m120 save's config**, which is the old turf. G's turf is
  stricter (establish_saturated_max 0.05), so G-too-damp ground is undercounted.

**Faces already wet at hour 2**, summed over eight seeds:

| held at 2 h | arm | turf at 6 h | velvetpad at 6 h | vacant at 6 h |
|---|---|---|---|---|
| turf, 40 faces (s8 27, s12 13) | base | 3 | 7 | 30 |
| | p03 | 4 | 6 | 30 |
| | G | **0** | 8 | 32 |
| velvetpad, 373 faces | base | 0 | 362 | 7 |
| | p03 | **8** | 354 | 7 |
| | G | 0 | 354 | 12 |

**Every wet face at 6 h**, read on each arm's own m360 water:

| arm | wet faces | turf | velvetpad | vacant | other |
|---|---|---|---|---|---|
| base | 14,025 | 130 (0.9 %) | 1,439 (10.3 %) | 64 % | 24 % |
| p03 | 15,577 | **888 (5.7 %)** | 2,237 (14.4 %) | 57 % | 23 % |
| G | 14,168 | **36 (0.25 %)** | 2,003 (14.1 %) | 61 % | 25 % |

On soggy faces at 6 h, turf/velvetpad: base 35/358, p03 413/1,099, G 1/953.

**Reading:**
- **G does what Wrysk asked on the turf side.** Turf on damp ground falls to a quarter of
  base, and to 4 % of p03's. That happens only through turf's own water knobs: there is no
  reverse overtopping, and velvetpad cannot invade living turf.
- **p03 alone has the opposite effect.** It puts about 6× base's turf on ground that becomes
  wet: turf comes up through pads while the face is drained and survives the later wetting.
- **Velvetpad "thrives and retakes the vacancies" only in part.**
  - On faces turf vacated, velvetpad took 8 of 32 vacancies in G within 4 h; 25 % retaken.
  - Velvetpad holds 39 % more wet faces than in base (2,003 against 1,439), and 2.7× the
    soggy faces.
  - But about 60 % of wet faces are vacant in every arm, base included. Damp ground is mostly
    empty, not mostly moss.
- **Death causes on these faces are not in the readout.** In plants-only runs the
  candidates are water stress or light; there is no grazing.

## Drought

**No ecological drought run was made.** None of the study's world-scale runs imposed a
drought; the climate was the default.

**Single-stand probe.** The probe from 74015627 (`velvet_niche drought`) was re-run for this
note with the f852e00d build: a few seconds each.
- G is the branch defaults.
- The earlier physiology was restored with a `sets=` file of the pre-74015627 preset values.
- Settings: light 1, wet 1.2, dry spell 55,000 s, a day wet either side.

| species | physiology | water 0.4 | water 0.2 | water 0.1 | water 0 |
|---|---|---|---|---|---|
| springturf | G | lives; 876 packages the day after | **lives**; 875 the day after | dies at 9,878 s | dies at 8,447 s |
| springturf | earlier | lives; 90 the day after | dies at 8,853 s | dies at 7,991 s | dies at 7,991 s |

Results are the same from an adult or a newborn start.

**This confirms the commit's claim.** G turf is drought-resistant down to available water
0.2 for longer than its seed bank lives, and dies at ≤ 0.1.

**Velvetpad side effect.** At its home light 0.2, with a 55,000 s spell:

| velvetpad | water 0.6 | water 0.4 | water 0.2 |
|---|---|---|---|
| G | lives | dies at 29,842 s | dies at 17,742 s |
| earlier | lives | dies at 36,655 s | dies at 17,742 s |

At light 1 and water 0.4, G's velvetpad dies at 45,193 s, whereas the earlier one lived.
Velvetpad's short D1 probe (7,200 s) still passes in full under G.

## Food

**World stocks at 6 h, plants only**, median of the per-seed ratios to base, eight seeds:

| | p03 | G |
|---|---|---|
| world edible foliage | 0.95 | **0.60** |
| world standing litter | 0.88 | **0.70** |
| velvetpad edible | 0.94 | **0.50** |
| springturf edible (median ou) | 36 (base 21) | **126** |
| springturf stands (median) | 821 (base 421) | 543 |
| h4–6 foliage built, pad + turf | 0.97 | **0.67** |
| h4–6 shed (litter input), pad + turf | 0.95 | **0.73** |
| h4–6 turf shed (median ou) | 407 (base 246) | 1,957 |
| h4–6 pad shed (median ou) | 10,085 (base 10,830) | 5,533 |

**Per seed in G at 6 h**, turf against pad: turf 454–1,213 stands carrying 101–236 ou edible;
pad 4,690–7,098 stands carrying 982–1,748 ou. Base turf carried 5–59 ou.

**Fauna runs, median stocks at the end:**

| | edible foliage | litter |
|---|---|---|
| G established | 3,616 → 1,903 | 4,209 → 3,103 |
| G transition | 2,408 → 1,265 | 2,401 → 1,934 |
| p03 established | 3,616 → 3,181 | 4,209 → 3,556 |

**Overtop pulse.** The detritus pulse is ×2–3 in both arms during the fauna windows:
- G established: early 1,049 → 2,358, late 438 → 704;
- G transition: early 630 → 1,759.

So part of the litter seen in the first hour is pad mass killed by replacement, which is the
subsidy Astra warned about.

**Reading.**
- G turf is now a real grazing crop: about 5× base turf's edible foliage per stand, and
  about 8× base turf's world shed in h4–6.
- It does not replace what velvetpad loses at half alpha. World edible foliage drops by
  about 40 % and litter input by about 27 %.
- The food classes are unchanged: turf and pad still feed the same foliage and litter pools.

## Recommendation

**Arm G with p = 0.03 should not be presented as passing.**
- It passes S2 occupancy, S3 occupancy (on the preregistered seeds 1, 3, 5, 6) and the focal
  S4 comparisons.
- It **fails S1** in every seed, as do p01 and p03.
- It weakens three of the readouts Astra asked to keep visible:
  - world edible foliage −40 % and litter −30 % against base;
  - refuge deaths 1.27 × base;
  - Veilhound and seedporter established ratios of 0.44 and 0.80.

**What fails, and the next step for each:**
1. **S1 is dispersal-limited in every arm.** About 60 % of the cohort never receives a seed
   in 4 h, and 81 % lies more than 6 voxels from any turf donor at hour 2. Per Astra's rule
   the next study is dispersal or target-restricted runners, not a higher p.
2. **S1 also loses turf after entry in G.** Post-birth losses are 10× p03's and arm losses
   are 50× p03's. Check G's newborn light band (L ≥ 0.6) and saturation gate against the
   cohort's L ≥ 0.5.
3. **G's food readout.** If the halved velvetpad alpha is kept, turf production does not make
   up the loss. Astra's review asked for this to go to Wrysk as a separate production
   decision.

**Ready for Astra?** Yes, as a results package: the evidence is complete for every arm. It is
not ready as a passing merge candidate. p03 alone is the closest to the preregistered
conditions: it fails only S1 and keeps food within 5–12 % of base. But it puts 6× base's turf
on ground that later becomes wet, which is the opposite of Wrysk's too-damp requirement.

## Open questions

1. **Separate G's two changes?** Should G's springturf redesign and its velvetpad alpha 4 → 2
   be separated? Most of the food loss and the extra refuge deaths are probably the
   velvetpad change, but no arm isolated it.
2. **S1 recount.** Should S1 be recounted on the reachable cohort (≤ 12 voxels from a donor at
   hour 2)? The answer would only be descriptive. Astra's rule keeps the full denominator,
   and this note does.
3. **Fate classes on G's own gates.** The `fate` classes use the old turf's gates. A G-gated
   wet class would count more damp faces for G; the readout code would need a `reference=`
   choice.
4. **Veilhound in G established.** It fell to 1/8 seeds at ≥ 0.80, through carrion access.
   Is this noise from a small pack or a real effect of G on kill opportunities? A rerun is
   not needed to answer it; reading the kills per seed may do.
5. **G's cohort mismatch.** Why does G's cohort differ by one site on s6 and s8 when cohorts
   are read on the save's own config?
6. **Branch defaults versus arm G.** The branch defaults carry G with overtop rate 0. Which of
   these is the merge candidate?

## Spread rerun on the fast maps (2026-10-04 evening)

The first spread batch was void: its loads froze every animal (fixed ea139c8c). This rerun
used `succession_study` at 7995a723, staged (`run=`, 30-min segments, a 10-min liveness
check with `expect=` the arm's counter). **Stage A** ran on `patch`: seeds 1, 3, 5, 6, 8, 9,
12 and 14 (terrain seed 3 is refused and founds as 4), founded under `main` to minute 120,
then each arm for 2 h. The table gives the median [min–max] over the 8 seeds at minute 240.
`pick` is every mechanism on, at wind mean 3.0 m and preset alpha 3.6. `off` is the same
file as `main`, so it was not run.

| arm | turf/pad edible per mature stand | edible+litter / preG | S1 under turf, h3 → h4 | burr deliveries (germinated, all seeds) |
|---|---|---|---|---|
| preG | 0.15 [0.07–0.30] | 1.00 | 0.00 → 0.00 | 0 |
| main | 1.26 [0.94–1.92] | 0.74 [0.64–1.20] | 0.00 → 0.00 | 0 |
| ot | 1.16 [0.88–1.73] | 0.71 [0.65–0.91] | 0.04 → 0.06 [0.00–0.31] | 0 |
| wind6 | 1.31 [1.11–1.55] | 0.74 [0.65–1.29] | 0.06 → 0.14 [0.02–0.38] | 0 |
| wind12 | 1.31 [1.19–1.56] | 0.75 [0.65–1.35] | 0.10 → 0.23 [0.15–0.41] | 0 |
| wind24 | 1.40 [1.13–1.56] | 0.76 [0.66–1.38] | 0.14 → 0.28 [0.19–0.42] | 0 |
| runners | 1.06 [0.72–1.63] | 0.72 [0.65–0.90] | 0.06 → 0.08 [0.00–0.36] | 0 |
| burrs | 1.14 [0.86–1.68] | 0.74 [0.64–0.91] | 0.04 → 0.06 [0.00–0.32] | 19 [1–86] (33) |
| all_a26 | 0.83 [0.64–0.95] | 0.85 [0.72–1.22] | 0.13 → 0.31 [0.20–0.61] | 2 [0–15] (9) |
| all_a30 (main defaults) | 0.98 [0.82–1.10] | 0.87 [0.76–1.31] | 0.13 → 0.29 [0.18–0.60] | 28 [0–59] (19) |
| all_a36 | 1.17 [1.08–1.35] | 0.90 [0.76–1.54] | 0.15 → 0.34 [0.21–0.54] | 42 [2–120] (42) |
| **pick** | 1.24 [1.04–1.31] | 0.89 [0.76–1.59] | 0.19 → 0.38 [0.26–0.60] | 47 [3–219] (69) |

- **Wind reach 24** (3.0 m) gave the largest S1 share of the three reaches at no cost in food.
  **Alpha 3.6** was best of the three alphas on all three readouts. Both are now the defaults
  on branch `spread-pick`. No arm reached the brief's targets of 1.5–2× per stand and
  1.0–1.15× edible+litter. With every mechanism on, velvetpad fills about 500 of the about 720
  vacant waterlogged faces, against 4 or 5 without it. Those young pads dilute the per-stand
  mean.
- Fauna B is within noise of preG for every lineage, on every arm (medians 0.86–1.29), except
  the seedporter: 0.67–0.80 on the all-on arms, 2–3 of 7 seeds at ≥ 0.8.
- **Liveness:**
  - Seed 1 failed every arm at the first check: its seedporter was 0 m walked with one body
    "Walking". It was rerun with `still=seedporter`.
  - Nine all-on and burr runs failed on `burrs_picked` within the first 10 min, on seeds 1, 5,
    6 and 8, where turf–animal contact is rare. They were rerun with the runner and overtop
    counters, or on a 30-min check. Burrs then fired on most of those seeds (0–120 a run).
- Coverage maps (S1 sites at minutes 120 and 240, every arm and seed) are in the session
  scratchpad, `spread/maps2/`.

**Stage B** ran on `terrarium` from the saved minute-60 starts, 2 h. The starts for seeds 2
and 3 are the same world (seed 2 is refused and founds as 3), so there are two distinct
worlds. Results at minute 180 (s1; s2 = s3):

| arm | edible+litter / preG | turf/pad per stand | Veilhound kills (carcasses) | fauna B / preG, median over seeds |
|---|---|---|---|---|
| preG | 1.00 | 0.11; 0.13 | 4; 3 (5; 3) | 1.00 |
| main | 1.28; 1.10 | 1.37; 1.59 | 6; 6 (7; 6) | bellwing 0.66, capgnawer 1.20, Veilhound 1.11, frondgrazer 1.12, littershredder 1.22, loftstrider 1.34, ripple-snail 0.95, seedporter 1.45 |
| pick | pending | pending | pending | pending |

On the terrarium starts, the first pick check (minutes 60–90) saw no runner or overtop events
on any seed. Those minutes are the night of the 2 h day clock, when no turf seed or runner
moves in any arm. The rerun checks to minute 130.
