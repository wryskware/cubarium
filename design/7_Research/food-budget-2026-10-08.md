# FOOD-BUDGET: food supply against need, per lineage, d72 baseline v4 (2026-10-08)

Read-only. Sources: `runs/bl4-2026-10-08/{heuristic,c1}/s*/census.csv`, `vitals/vitals.csv`
(8 seeds, m63 to m183 = one world day), `score.txt`. Resolved knobs come from
`target/release/examples/params --config config/desktop/terrarium-d72.toml`, saved to
`scratchpad/fb/params-d72.txt`. Laws: `crates/cubarium-voxel-flora/src/step.rs` (photosynthesis
:3225, foliage regrowth `d_p = min(P_cap − P, foliage_rate·W·dt)` in `grow_stand_cause` :3268 ff,
fruit refill :3526, senescence shed :3571) and `flora/src/lib.rs:5776` (bite × (1 − defence)).
Scripts: `scratchpad/fb/{stocks,intake}.py`. No sims were run. Every number is a seed median
over the heuristic arm unless marked otherwise.

## 0. Whole-world books, one world day (m63 to m183)

- **Fixation** (`fixed_in`): 55,800 to 69,000 org per world day (s4, s1), about 45,700/h at
  midday. **All animals together ate** 90 to 119 per day (`consumed_organic`). That is
  **0.17 % of fixation.**
- **Foliage production** is about 22,000/day: litter input ≈ 20,000 + eaten 72 + Δ edible
  stock 1,550. The browsers (frondgrazer 43 + loftstrider 29 raw) take **0.33 %** of it.
- **Litter.** Input is about 13,000 to 15,600 per hour by day. Night decay implies an
  effective rate of about 4.4e-4/s against `flora.decomposition` 6.9e-4/s, so litter lasts
  25 to 40 min. Littershredders eat 5.3 per day, **0.03 %** of the input. The microbes'
  decay processes the rest.
- **Leaves live minutes.** `senescence` (a per-second share of foliage, `step.rs:3571`) is
  0.0026/s for springturf (6.4 min), 0.0018/s for the shrubs (9 min), 9.5e-4/s for the
  vaulttree (18 min) and 5.8e-4/s for the velvetpad (29 min).
- **Foliage regrowth is fast too.** Photosynthesis is linear in lit foliage
  (`a = assimilation·L·P·daylight·μ·limit`). A cropped stand keeps `graze_refuge` of its
  capacity (turf 0.4, pad 0.3, the rest 0.15) and regrows from it at a relative rate of about
  `assimilation·L`: 0.004 to 0.008/s. Turf cropped to its refuge refills in 2 to 4 minutes.
  Real grass grows about 0.1/day, which is 1.4e-5/s on this 2 h day. **The plant clock runs
  about 300 to 500 times faster than the animals' clock**, whose daily need is
  `f / 7,200 s`.
- **The gut ceiling.** `stomach_k/digestion_s × yield ÷ maintenance_per_s` is 18 to 45× upkeep
  in every lineage: frondgrazer 20, loftstrider 38, seedporter 45, bellwing 24, shredder 19,
  capgnawer 20, snail 18. A body breaks even feeding 2 to 5 % of the time. The bite asks twice
  the gut's throughput (`bite_per_body_s` = 2k/τ), so a bite cut in half still fills it.

## 1. Per lineage (heuristic arm; B = size_mean × body_max)

Units are raw org-units per body per hour. Upkeep is shown in raw terms (÷ yield).

| lineage | N, B | observed intake / gut max / upkeep | food in mouth reach: renewal (R_o = ÷ observed intake) | world renewal ÷ population intake (P) | stay-put fails when need/renewal × k, k > | population limit k < |
|---|---|---|---|---|---|---|
| frondgrazer | 33.6, 0.69 | 0.64 / 1.24 / 0.062 | about 5 stands × 1.0/h ≈ 5/h, R_o ≈ 8 (R_u ≈ 80) | about 5,500/h of browsable foliage ÷ 21.6 ≈ 250 | 8 | 30 at 20 % offtake |
| loftstrider | 8.6, 2.4 | 1.70 / 3.6 / 0.096 | one mature crown regrows ≈ 5/h (vaulttree up to 26/h); a few in reach, R_o ≈ 6 to 15 | about 550/h of crown foliage ÷ 14.6 ≈ 40 | 6 to 15 | 8 (narrow) |
| seedporter | 14.7, 0.17 | 0.16 / 0.51 / 0.011 | one bush refills 1e-4/s = 0.36/h when eaten out, R_o ≈ 2.3 (R_u ≈ 32) | lanternberry about 90 × 0.36 + vine 13 to 35 ≈ 45 to 65/h ÷ 2.3 ≈ 20 to 28 | 2.3 | 3 to 5 |
| bellwing | 66.5, 0.012 | 0.022 / 0.042 / 0.0018 | one bloomcrown 1e-4/s = 0.36/h, R_o ≈ 16 (R_u ≈ 200) | about 50 × 0.36 + vine ≈ 20 to 30/h ÷ 1.5 ≈ 15 to 20 | 16 | 2 to 4 (overlaps: competition, not parking, sets it) |
| littershredder | 35.7, 0.16 | 0.075 / 0.118 / 0.005 | litter input ≈ 0.7/h per vegetated face; 1 to 3 faces in reach, R_o ≈ 9 to 28 | 10,000/h ÷ 2.7 ≈ 3,700 | 28 | 370 |
| capgnawer | 16.5, 0.31 | 0.093 / 0.224 / 0.009 | glowcap caps `foliage_rate·W` = 2.4e-5/s = 0.086/h per mature cap; 5 to 13 in reach, R_o ≈ 5 to 12 | 222 caps × 0.086 ≈ 19/h ÷ 1.5 ≈ 13, rising (caps ×7 in 2 h) | 5 to 12 | about 3 |
| ripple-snail | 32, 0.056 | 0.013 / 0.026 / 0.0011 | film `r·K/4` = 0.0015 × 0.4/4 per m² = 0.0084/h per face; 1 face, **R_o ≈ 0.6** | film area not logged | already < 1 | unknown |

**Frondgrazer, in detail**
- **Within one body length** (113 faces) it has about 23 raw standing, 18 h at gut max, and
  about 100/h of renewal, 80× its gut max.
- **Travel needed: 0 m/h.** Even with no regrowth at all it would need only
  `intake / (16 faces per m × edible per face)`: on pad 0.64/(16 × 0.28) = 0.14 m/h, on turf
  0.035 m/h. That is 0.2 s of walking per hour.
- **Density limits this.** About 12,000 vegetated faces over about 45 browsers is 270 faces
  per body, against 7 in mouth reach. So the window is at most about 40×. Any lever lands as
  patch-switching every few minutes, not long trips.
- **C1 against the heuristic:** C1 moves 36 % of the time against 0.75 % and eats 50.3 against
  43.1 (+17 %). Moving pays a little in intake and nothing in survival.

**The rest**
- **Loftstrider:** reach 0.55 m, about 0 % moving.
- **Seedporter** already travels about 670 m per body per 2 h, gliding between groves.
- **Bellwing** travels 347 m per body. Its standing bloom nectar sits at about half the cap
  (0.011 per flowering stand), so flowers are never drained.
- **Shredder:** standing litter in reach is 5 to 18 raw, 40 to 150 h at gut max. The bite
  saturates at 0.00125 per face against 0.17 to 0.65 per face present.
- **Capgnawer:** the grove supply grows faster than the capgnawers do.
- **Snail: the model case.** Its food has a low capacity per face (0.00625, 28 min of its
  intake) and logistic regrowth. It must creep, and it does: 27 % moving.
- **Predators:**
  - The chorister pack (4 adults) is at balance: assimilated 0.82 against upkeep 0.80 per
    2 h, fullness 0.97, about 2 kills per world-hour, its prey (frondgrazers) abundant.
  - The lanternjaw was held still in this batch (`still=lanternjaw`): no supply question.

## 2. What would make moving pay: rate or role

- **Frondgrazer and loftstrider (foliage).** The best lever is local: the induced defence.
  - Bite yield is `(1 − D)` (`flora lib.rs:5779`). D settles at
    `D/(1−D) = bites/s per stand × defence_per_bite × T½/ln 2`. T½ is 439 s, so T½/ln 2 is
    633 s.
  - A parked frondgrazer spreads its bites over about 5 stands, about 0.7 bites/s per stand.
  - Today (0.003 per bite) D ≈ 0.57: it gets 86 % of gut max.
  - At ×10, D ≈ 0.93: 0.17/h, 27 % of its observed intake.
  - At ×30, D ≈ 0.976: 0.06/h, about its upkeep.
  - A body that moves on to stands it has not bitten gets full bites, and D decays in about
    7 min. Total supply is unchanged, so the population is not at risk.
  - This is a world mechanism that already exists and is true to nature.
  - The rate lever is the plant clock (or the browser's mass, below): about ÷8 to ÷30.
- **Loftstrider:** the same defence lever. Its window on rates is narrow (6 to 15 against 8),
  so it should not take a big rate cut. Its real problem is grove patchiness (memory: dying
  out is fine until vaulttree groves exist).
- **Littershredder: this is a role question.** The microbes take 99.97 % of the litter.
  - The supply lever is litter fall (`senescence`): input ÷28 to ÷370.
  - Either the shredder's mass ×30+, or a litter-mould role (plan item 11), would make litter
    eaten visibly.
  - Raising decomposition thins the standing litter, but the bite saturates at 0.00125, so it
    would need ÷200 or more.
- **Seedporter: a rate.** `fruit_per_s` ×0.3 to 0.5 brings one bush below its intake, and the
  population holds to about ×0.2. First fix the instant refill (§4).
- **Bellwing: competition, not parking.** At 66 bodies on about 50 flowers, the population
  sets the pressure. A nectar cut risks the population at about ×0.1. Search it gently,
  ×0.25 to ×1.
- **Capgnawer: a rate,** but its window is about 5 to 12 against 3, and supply is rising.
  - It is better left as is, or given a mild `glowcap.foliage_rate` gene (×0.3 to ×1).
  - It also benefits from the defence gene (`glowcap.defence_per_bite`).
- **Ripple-snail: leave it.** It is the reference case.

## 3. Recommended genes for `scripts/studies/eco_search.py`, ordered by expected effect

| # | name | path | tie | range | expected effect |
|---|---|---|---|---|---|
| 1 | defence | `flora.*.defence_per_bite` | all | ×[1, 30] log | A parked browser or capgnawer falls to upkeep at ×20 to ×30. Movers are unaffected. The population is safe. |
| 2 | defence_memory | `flora.*.defence_half_life_s` | all | ×[0.5, 5] log | Longer memory means a patch stays poor longer, and bodies return less often. It also multiplies gene 1's D. |
| 3 | plant_clock | `flora.@bio_rate` (foliage/wood rates, assimilation, maintenance, senescence, decomposition, vine rates) | all | ×[0.03, 1] log | The one-clock fix. Every plant renewal ÷k with plant balance kept; succession runs k× slower. FG stay-put fails below about 0.12; populations hold above about 0.03. |
| 3' | browser_mass (alternative to 3, not both: they form a ridge) | `fauna.founders.{frondgrazer,loftstrider}.mass_scale` | each | ×[1, 30] log | The same ratio shift from the animal side; plants untouched (audit §1.4). |
| 4 | litter_fall | `flora.@photo.senescence` | all | ×[0.02, 1] log | Litter input ÷; the shredder stays put until about ×0.04. Leaves would live hours, not minutes. |
| 5 | fruit | `flora.lanternberry.fruit_per_s` + `flora.latticevine.fruit_per_face_s` | all | ×[0.15, 1] log | One bush no longer feeds a seedporter below about ×0.4. The population holds to about ×0.2. |
| 6 | refuge | `flora.@photo.graze_refuge` | all | ×[0.1, 1] log | Cropped refill takes ln(1/r)/g: turf 0.92/g → 3.2/g at r = 0.04, so ×1.5 to 3.5 slower. More stock becomes edible, so it partly offsets itself. Secondary. |
| 7 | nectar | `flora.bloomcrown.nectar_per_s` + `flora.latticevine.nectar_per_face_s` | all | ×[0.25, 1] log | Bellwing pressure. Below about ×0.1 the population is at risk. |
| 8 | caps | `flora.glowcap.foliage_rate` | all | ×[0.3, 1] log | Capgnawer. Its window is narrow; keep the box mild. |

Genes 1, 2 and 6 shape the situation locally: cropping makes the spot itself worse. Genes 3, 4,
5, 7 and 8 are supply rates. Keep V out of the objective (plan Q5). Read movement as a
diagnostic of whether genes 1 to 3 opened the window.

## 4. Absurd world cases: fix at the world, don't search

1. **The plant clock is about 300 to 500× the animal clock.**
   - Leaves live 6 to 29 minutes (`senescence`). Cropped foliage refills in 2 to 4 min
     (relative growth `assimilation·L` ≈ 0.004 to 0.008/s, against about 1.4e-5/s for real
     grass on a 2 h day).
   - Foliage is a minutes-long conveyor from fixation to litter to microbes. The audit's
     "one clock" (§2) should set `@bio_rate` and `@bio_time` once. Search around that, not
     over 3 decades.
2. **Standing stock per face.** A mature turf face holds `alpha·wood_max` = 6.3 × 0.3 = 1.89
   org per 0.0156 m², 121 org/m². That is about 80 adult frondgrazer bodies of foliage per m²,
   against about 0.02 bodies per m² in nature (about 4,000×). This density, one stand per
   face (audit §1.2), is why even zero regrowth needs < 1 m/h of travel.
3. **Fruit eaten out refills at once** (`step.rs:3526`): `fruit_rest_s` (900 s) applies only
   after an uneaten crop drops at 1,800 s. A bush picked clean is a perpetual feeder at
   0.36/h, 2.3× a seedporter's intake, so parking at one bush pays. The rest (or a crop clock)
   should follow any emptied crop.
4. **The animals are not the cycle's consumers.** Animals take 0.17 % of fixation, 0.33 % of
   foliage and 0.03 % of litter. The microbes' decay and senescence run the carbon cycle. This
   is a role gap (Balance through roles): a litter decomposer animal or mould, plan item 11.
   More shredders won't fill it.
5. **The gut ceiling is 18 to 45× upkeep.** Animals spend a tiny share of time eating, so every
   supply margin is wide.
   - This is the animal side and not a supply lever. Even so, it is why each supply cut must
     reach about ×20 before a parker drops to upkeep.
   - Real scope (maximum sustained intake over basal) is about 3 to 7×.
   - Worth Wrysk's look before the search: the `stomach` and `bite` genes exist in
     `config/training/search/roster.toml`.
6. **The defence works, but the senses can't read it.**
   - A defended plant keeps its stock, so taste reports it full (audit §1.3 item 3). With
     gene 1, the animal learns of the defence only through its bite yield.
   - "Leave a patch when bites stop paying" is a true rule on its own. Check that the
     heuristic has it before reading gene 1's result.

## Uncertain or estimated

- The frondgrazer's reach is taken as 5 vegetated stands (7 faces, about 70 % vegetated).
- The loftstrider's reachable crowns and the glowcap grove density are rough.
- The film area is not logged, so the snail's P is unknown.
- The browsable share of foliage is taken as 0.5.
- The world is still in early succession: velvetpad ×2 and glowcap ×7 over the day. P rises
  as it matures, so these windows are conservative on the population side.
