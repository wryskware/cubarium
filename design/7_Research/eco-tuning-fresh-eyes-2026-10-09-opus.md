# Fresh eyes: tuning the ecology numbers (Opus, 2026-10-09)

**Top recommendation:** stop searching for genes whose *equilibrium* lands on the target. Start
worlds *at* the target and measure the *drift*: thin each lineage to its target headcount, apply
the candidate genes, run about 1.5 sim-h, and read each lineage's per-capita growth rate and
energy surplus. The genes are right when every drift is about 0 and stable. That turns a 6–10 h
relaxation problem into a short root-finding problem. Do a zero-compute roles triage first:
about half the targets can't be reached by any gene in the current box.

Evidence tags: **[checked]** means verified against code or `runs/wideyear2-2026-10-09` data
(scripts in `scratchpad/fresh-eyes/tools/`); **[hyp]** means a hypothesis.

## 1. What limits effectiveness per compute today

**1a. The 2 h screen measures the start, not the candidate.** [checked]
- Per-capita log growth rates in the baseline, min 360–480, mean ± sd over 8 seeds, per hour:
  - frondgrazer +0.20 ± 0.04 (e-folding 5 h); loftstrider +0.17 ± 0.13;
  - seedporter −0.21 ± 0.17; capgnawer −0.09 ± 0.11; shredder −0.05 ± 0.09;
  - snail −0.04 ± 0.03; lanternjaw −0.03 ± 0.06; bellwing +0.06 ± 0.14; Veilhound 0.
- The year-old starts sit far outside the bands: frondgrazer ln(300/79) ≈ 1.3 above the top.
- A candidate moves counts by r × 2 h ≈ 0.1–0.4 in log in a 2 h window. That is comparable to
  the between-seed sd and small against the start offset. So A and K mostly score the shared
  start; the varying terms are O, F, S, J and H.

**1b. Overstocked starts punish the right answer.** [checked arithmetic, hyp on the outcome]
- Rescale-on-load keeps headcounts. A candidate that is correct at 50 frondgrazers starts with
  4–7× its own equilibrium demand.
- Baseline offtake is 0.6–1.7 % at 200–370 grazers [checked]. The needed region (mass ×300–900,
  gut 1/6) therefore starts at roughly 60–180 % offtake. That means famine, crashes and
  possible L, and feasibility-first ranks those candidates last. The screen steers CMA away
  from the target region.

**1c. Rank-only CMA throws the vector away.** [checked: `eco_search.py` uses only score.py's
ranking]
- Each run measures 9 lineage trajectories, offtake, kills and vitals. sep-CMA keeps one rank
  among 8 arms.
- 9-D sep-CMA at λ 7 needs tens of generations. 4 generations (256 runs, about 380 core-h)
  converge nowhere and teach nothing about the gradients.

**1d. Several targets can't be reached by any gene in the box.** [checked]
- **Veilhound:** one pack of 4, one breeder, litter 1–4 by condition, once a year. 8 in every
  seed (one litter of 4), zero Veilhound deaths in 8 seeds. Year 1 can reach 5–6 only through a
  smaller litter. Beyond year 1 nothing regulates the pack except starvation, and a 10 h run
  sees one litter.
- **Loftstrider:** `birth_interval_s` 1098 (18 min), gestation 329 s. It doubles in about 4 h
  (2 → 9–81). The ×4–12 mass box would regulate it only by starving it. This is a
  life-history role, like the Veilhound's LIFE-HISTORY, not a number search.
- **Capgnawer, littershredder:** breeding-limited, not food-limited.
  - Adult fullness p50 is 0.86 and 0.68.
  - Capgnawers had 49 births against 83 deaths of age. Shredders had 135 births against
    101 aged and 19 killed by lanternjaws.
  - Litter is about 88× the shredder's K, so `mass_shredder` barely moves its count.
- **Seedporter:** predation-limited. 151 were killed by the Veilhound against 126 born.
  Fullness p50 is 0.01, it is hungry 98 % of the time, and the fruit offer is low 86 % of the
  time.
- **The `mass_small` tie points the wrong way.**
  - Raising it lowers bellwings (208–1,800), which are already in band (fitness-v3 `hi` is 900
    per d72 area).
  - It also cuts seedporter, capgnawer and lanternjaw. All three are below band, and two are
    declining.
- **Woods** are 0.4–1.1 % of floor at year 1 (`forest_floor_faces`, 260–692 faces) against a
  20–30 % target. The second grove is 0.3–0.5 m² against 29 m². This is flora; no fauna gene
  reaches it.
- **H:** 63 % of Veilhound kills are bellwings (824 of 1,300). Snapping a flyer counts as a
  hunt.

**1e. Cost is fauna, and it scales with the overpopulation.** [checked]
- At minute 595, seed 1: tick 38 ms = water 5.0 + flora 2.2 + fauna 30.7 (sense 27, serial
  reflex 21).
- A post-load perf probe (s7 m600, 1.5 sim-min) put about 25–35 % of samples in bellwing
  `step::nest_reflex`'s uncached face scan [partly checked]. s1 logged 3.5 M nest searches in
  720 k ticks.
- Overstocked worlds (837–1,809 animals) should cost 1.5–2× target-state ones (about 400)
  [hyp].

**1f. Practical traps.** [checked]
- The wideyear2 saves are fauna schema 75; main is 76 (V3-PLACES). Any new tool built on main
  can't load them.
- `web_model budget` on the wide config mispredicts the current world: frondgrazers 3.4 → 2.0
  with zero births, bellwings and capgnawers absent. Its behaviour fits are from soak v2.

## 2. The plan

**Stage 0. Roles triage (0 core-h, about 2 agent-h, then Wrysk's calls one at a time).**
- Take the per-lineage demography table above to Wrysk: limiting factor, births, deaths by
  cause, intake/upkeep, fullness.
- Proposed calls:
  1. Loftstrider life history at a year scale.
  2. Veilhound: confirm litter-by-condition as the year-1 lever, and name a multi-year
     regulator (starvation of pups, or subadult dispersal).
  3. Seedporter refuge or fruit supply.
  4. Shredder and capgnawer breeding constraints.
  5. Split `mass_small` into `mass_bellwing` and `mass_fruit_cap`, keeping the lanternjaw tied
     to whichever it hunts most.
  6. Whether H counts only prey above a size ratio.
  7. Woods as a separate flora question.
- Searching before these calls spends compute on constant penalty offsets.

**Stage 1. Tools (about 0.5 day, opus-medium).**
- `thin=lineage:N` on load: a random stage-stratified subsample, booked as removed matter like
  rescale's introduced matter.
- A drift readout in the census: per-lineage births, deaths and per-capita net energy over a
  window.
- Optionally, nest-target caching (§4.9).
- Build these on a branch from 774bd98c to reuse the paid-for schema-75 saves. Otherwise
  remaking the start library costs about 65 core-h and 8–9 h wall.

**Stage 2. Probes (§5, about 15 core-h).** They decide the window length and the seeds per
point, and test the isometry and gut assumptions.

**Stage 3. Map with pinned drift (about 72 core-h).**
- Fidelity: mature wide saves, thinned to the band midpoints (Veilhound 5, loftstrider 4),
  shipped strides.
- Each run: 30 min settle (foliage stock, defence with T½ 7 min, gut at S = 1/6 has digestion
  about 72 min, so P4 sets this) then a 60 min read window.
- Design: 48 Sobol points over the (re-tied) genes × 2 starts = 96 runs.
  - The boxes are extended wherever §4.3's scaling says the target lies outside them, as
    "targets are to reach" requires.
  - One design factor is the thinning level (0.5×, 1×, 2× target), for stability.
  - Another is Veilhound present or absent, to split predation from food.
- Outputs per run and lineage:
  - ρ = (births − deaths)/(N·Δt);
  - the energy proxy ρ_E: assimilated − upkeep − motor, as potential births per body-hour,
    calibrated on the baseline;
  - offtake O, hunts by prey class, kill rates.
- Model: per lineage, ρ_l regressed on its own 2–4 genes (own mass, gut, defence or
  spur_share, predator presence), in log-gene coordinates, quadratic or GP. The prior is
  biomass collapse: for food-limited lineages ρ depends on N·k (§4.3).
- Decision: solve for ρ_l = 0 at target N for every fast lineage, with O in [0.10, 0.30] and
  H in band. Take the solution and its robust neighbourhood.
- Stop adding points when each lineage's leave-one-out error is under 0.03/h, or 2 batches.

**Stage 4. Confirm (about 47 core-h).**
- Pinned arms: the solution plus 8 arms of a resolution-III fractional factorial at ±0.15 of
  the box around it, × 4 starts × 1.5 h. This gives the local Jacobian, i.e. the gradients and
  interactions Wrysk wants.
- Unpinned arm: the solution from the 8 overstocked year-old saves × 3 h. Counts must head
  toward the bands.
- Accept when:
  - every fast lineage has |ρ| ≤ 0.05/h at target in ≥ 6/8 starts (half a ×3 band over 10 h);
  - ρ(2×) < 0 < ρ(0.5×), which is stability;
  - O and H are in band.
- Otherwise add the points to the data, refit and re-solve. Allow 2 rounds, then go back to
  roles.

**Stage 5. Year check (about 130 core-h).**
- Fresh founding × 8 seeds × 10 h for the best 1–2 candidates, judged on wide-year minus
  W/G_v until the flora call.
- Their hour-10 saves become the next start library, with landscapes grazed at candidate
  numbers rather than ungrazed. Round 2 of Stages 3–4 (about 60 core-h) runs from them.
- Megafauna are judged by energy balance at target: kills × prey mass against upkeep plus the
  litter cost, and whether a 5-dog pack nets about one surviving pup a year. A 10 h run cannot
  show multi-year regulation.

**Early stops in every run:** a required extinction, a boom above 5× target, or tick_ms above
2× the batch median.

## 3. Compute and outcome

Unit costs:
- overstocked mature wide: 0.6–1.2 core-h per sim-h [checked];
- thinned to target: 0.4–0.6 [hyp, P5];
- fresh year: about 0.8.

| Stage | core-h | Desktop (16 slots) | Desktop + eidolon (28) | EC2 2×c7g (about 120) |
|---|---|---|---|---|
| 2 probes | 15 | 1.5 h | 1 h | not worth booting |
| 3 map | 72 | 4.5 h | 3 h | 1 h + setup, about $1 |
| 4 confirm | 47 | 4 h | 3 h | 1.5 h, about $0.6 |
| 5 year | 130 | 9 h (one wave) | 9 h (run length bound) | 10–11 h, about $2 |
| round 2 | 60 | 4 h | 3 h | 1 h, about $0.7 |
| **total** | **about 320** | **about 23 h** | **about 19 h** | **about 15 h, $5–8** |

- Add 65 core-h and 9 h if the start library has to be remade for schema 76.
- Compare: the held CMA round is about 380 core-h and 26 h for 4 unconverged generations; the
  12-generation version is about 1,150 core-h.
- Money is not the constraint; wall time is. If Wrysk wants the desktop back, the year checks
  are EC2's best use: they are wall-bound and embarrassingly parallel.

Expected end state:
- Frondgrazer, snail, bellwing, seedporter (after the untie and spur/refuge calls) and
  loftstrider (after the life-history call) have zero-drift genes with per-lineage gradients.
- Leaf offtake is 10–30 %.
- The year check is in band on at least 6/8 seeds for those lineages.
- The surplus is a mapped response surface, not a single winner.

Still uncertain:
- multi-year Veilhound regulation;
- the lanternjaw (5–7 h to adult, so its proxy is weakest);
- the capgnawer and shredder until their roles change;
- woods and groves (flora);
- the drift under trained brains. Re-checking pinned drift with brains is cheap.

## 4. Novel ideas, ranked by effectiveness per compute

1. **Roles triage from the baseline before any search.**
   - Build: 0 compute, about 2 agent-h.
   - [checked]: §1d.
   - It removes targets no gene reaches, which otherwise dominate P as constants or randomly
     flip feasibility (capgnawer is at 3 in s4 and s7).
2. **Pin the state, measure the drift.**
   - Build: `thin=` plus a readout, 2–4 agent-h.
   - [hyp], tested by P4.
   - Each evaluation is about 1.5 sim-h at a cheaper state, not 2 h of start-dominated noise or
     a 10 h relaxation.
   - It also removes the famine bias of 1b.
3. **Biomass collapse: compute the mass genes, don't search them.**
   - Build: 0.
   - [checked] `mass_scale` scales every amount (`body_max`, `birth_cost`, `birth_body`,
     `growth_max_per_s`, `surplus_floor`, `egg_organic`) and leaves rates alone.
   - [hyp] For a food-limited lineage the drift then depends on N·k, so one measured
     equilibrium biomass B*_l gives k_l = B*_l/N_target.
   - That turns 5 search dimensions into calibrations. The breakdown cases are per-face bite
     depletion and predator size ratios; P2 measures them.
4. **Fit the vector per lineage; don't rank a composite.**
   - Build: about 0.5 day (numpy or stdlib).
   - [checked] the current loop keeps one rank per run.
   - Each lineage depends on 2–4 genes, so about 50 points fit all nine at once from the same
     runs.
5. **Re-tie the genes by target direction.**
   - Build: 0.
   - [checked] `mass_small` couples one in-band lineage to three below band (§1d).
6. **An energy-flux proxy for slow demography.**
   - Build: about 2 agent-h.
   - [hyp] Vitals already log intake, upkeep, motor and cohort energy, and life.csv logs
     `organic_given`.
   - Calibrate potential births from the surplus on the baseline. It averages over all bodies
     and cuts noise against discrete birth and kill counts.
7. **Clamp the predators to decompose.**
   - Build: 0 beyond `thin=`.
   - [hyp] Veilhound present/absent as a design factor.
   - Fit kill functions separately from the baseline: 268 frondgrazers, 151 seedporters,
     824 bellwings and 34 loftstriders taken over 80 world-hours [checked].
8. **The year check doubles as the next start library.**
   - Build: 0.
   - [checked] the schema 75/76 split means the library is remade anyway. This makes it grazed
     at candidate numbers.
9. **Cache the bellwing nest target** (1–2 agent-h; exact with invalidation, or 4 Hz like
   remote senses). [partly checked] §1e. [hyp] 15–30 % off mature ticks at today's bellwing
   numbers, less at target.
10. **Thinning and early stops as cost levers.** [checked] Fauna is 70–80 % of a mature tick,
    so out-of-band worlds are the expensive ones.
11. **Refit the web model to wideyear2** (0.5–1 day). [checked] It mispredicts today. Useful
    for unpinned forecasts and "budget before tuning"; pinned drift needs it less.
12. **d72 prey-only probes** with the Veilhound clamped (about 3.3× cheaper). [hyp] Predator
    density per area differs on d72, so use it only clamped.

## 5. First cheap probes (each under about 1 h wall on the desktop; core-h given)

- **P1. Baseline mining (0 core-h).**
  - Finish the demography table.
  - Calibrate the energy proxy against births per lineage, with lag.
  - Pass if the correlation is above about 0.7 for the fast lineages.
- **P2. Isometry and the food knee (8 runs, about 6 core-h, 774bd98c, no new code).**
  - 2 year-old saves × frondgrazer mass ×{1, 4, 16, 64} at shipped gut, 45 min.
  - Is O proportional to k at fixed N, and where does per-capita surplus start to fall?
  - The knee gives K(k) and checks biomass collapse.
- **P3. The gut's danger (6 runs, about 4 core-h).**
  - Gut S ∈ {1, 1/3, 1/6} × 2 saves, 45 min.
  - Measure per-capita surplus, breeding eligibility and births for shredder, capgnawer,
    seedporter and frondgrazer.
  - If S = 1/6 drops the breeding-limited lineages below their thresholds, the gut box needs
    Wrysk's call before any map.
- **P4. Pinned-drift pilot (4 runs, about 4 core-h, after `thin=`).**
  - 2 saves thinned to target and 2 unthinned, shipped genes, 1.5 h.
  - Measure the settle time of intake, fullness, foliage and defence, and the seed-to-seed sd
    of ρ and ρ_E.
  - This sets the window and the seeds per point.
- **P5. Steady-state cost (2 runs, about 0.3 core-h).**
  - perf from minute 3 after load, thinned against unthinned.
  - Confirms the nest_reflex share and the thinned-world tick cost.
- **P6. Life-history read (0 core-h).**
  - Read the Veilhound's `litter_by_condition` thresholds and the loftstrider and frondgrazer
    birth intervals.
  - Write the role options for Wrysk.
