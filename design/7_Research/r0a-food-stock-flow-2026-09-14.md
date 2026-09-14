---
design_status: exploration
last_reviewed: 2026-09-14
decision_refs: []
---

# R0a food stock/flow measurement — 2026-09-14

Evidence for Fable's review of `design/recurrent-organism-plan.md`. This document reports
what the current core's local consumption and renewal **do**. It chooses nothing: no edible
tissue pool, no root model, no cell resolution, no odor diffusion, no reproduction balance and
no regrowth change is proposed or implied here.

## How it was produced

`cargo run --release -p cubarium-core --example food_stock_flow` — the real core, four arms of
12,000 ticks (600 s of world time) each, 2.7 s wall on one worker. The source of every number
below, and the reasoning behind each fixture choice, is in
`crates/cubarium-core/examples/food_stock_flow.rs`.

Intake comes from `World::intake_diagnostics`, a transient counter added in R0a that records
the material which actually left `P`, `F` and `D` through a mouth — after the per-cell
proportional share and after every clamp. No figure here is inferred from a reserve delta or
from a `Feeding` label.

### What is staged, and what that costs the reader

- **The consumers cannot move.** `organism.speed_max` and `drives.turn_rate_max_deg` are zero,
  so a body stays in the cell it was placed in. This isolates the rates. Nothing about ordinary
  foraging, patch departure or whether an animal *would* stand there can be read off it.
- **One cell is the patch.** Feeding reaches exactly the cell an organism stands in, so a cell
  is what "local" means to a mouth. Every other cell is emptied of `P`, `F`, `D` and `De`, which
  makes the world's own totals the patch's own totals.
- **Matched conditions.** No weather swing, no rain, mutation off, one genotype, identical
  initial inventories across arms.
- Everything else is the world's ordinary configuration, because the point is to measure this
  ecology rather than a convenient one.

## Headline relationships

- **A productive visit outruns local renewal, and then stops being productive.** One consumer
  took material at about **0.7×** the patch's undisturbed gross production rate — but only
  because the patch had already been drawn down to the point where intake and production
  balance. It did not reach a sustainable equilibrium: **it starved at tick 9,074** (454 s).
- **Four consumers is not four times the intake.** Per-head intake fell **4.2×**, and all four
  starved earlier (tick 7,811) than the single one did. The cell served **100%** of every
  within-tick request in both arms, so competition here is *not* a per-tick share: it is the
  standing stock the consumers jointly hold the patch down to, and the shorter time each
  survives there.
- **A consumed patch pins itself just under the feeding gate.** `P` settles at ≈0.198 against
  `feed_min` 0.20. The single-consumer arm spent **73%** of its horizon with no edible kind
  above that gate; the four-consumer arm **65%**.
- **Recovery is real but slow.** After consumption ceased at tick 6,000 with `P` at 0.198,
  the patch reached 0.347 by tick 12,000 — **64%** of the 0.539 the undisturbed patch held at
  the same tick, over 300 s, and still rising at the horizon.

## Why intake stopped, by cause

The four causes the handoff asks to be distinguished, as measured rather than assumed:

| Cause | Measured | Verdict |
| --- | --- | --- |
| Reserve saturation | 0.0% of requesting ticks in both consumed arms | Never the limit here |
| Mouth throughput | 0.0350 m/s at saturation, cut to 0.0108 m/s by the type-II term (`K_P` 0.45) at the `P` ≈ 0.2 a consumed patch settles to | Not the limit: the animals were taking what the law allows |
| Chemical quality | 0.880 m of detritus standing, of which only 0.075 m edible (`D_eff = D · min(1, ρ/e_r)`) | **This is why `eaten_D` is zero.** The scavenging gate needs `D_eff ≥ feed_min`; it never opened |
| Unreachable food | None by construction — every other cell was emptied | Not tested. A moving animal's reach is exactly what this staged fixture does not measure |

## Not measured (censored, not extrapolated)

- Anything after a consumer starved; each arm's intake horizon is its own.
- Fruit as a channel: `F` stayed at 0 in every consumed arm, so no frugivory rate exists.
- Any scavenging rate: the gate never opened, so there is nothing to report.
- Whether a *moving* animal would do better. The fixture pins every body in place.
- Multi-cell patches, other light levels, other nutrient conditions, any other config.

## The run, verbatim

```text
# R0a food stock/flow measurement
# staged fixture: consumers are immobile, one cell is the patch, every other
# cell is emptied of P/F/D/De so world totals are patch totals.
# patch cell CellId(1160)  feed_min 0.2  K_P 0.45  P_max 1.5  graze_rate(unit adult) 0.035000 m/s

## arm `undisturbed`  consumers 0
   tick         P         F     D_eff         N      De/D      grown    eaten_P    eaten_D   reserve
      0    0.3908    0.0000    0.0000    0.5000    0.0000     0.0000     0.0000     0.0000    0.0000
   1000    0.4218    0.0000    0.0193    0.4922    2.0000     0.0512     0.0000     0.0000    0.0000
   2000    0.4533    0.0001    0.0383    0.4910    2.0000     0.1047     0.0000     0.0000    0.0000
   3000    0.4801    0.0051    0.0573    0.4903    2.0000     0.1603     0.0000     0.0000    0.0000
   4000    0.4993    0.0162    0.0772    0.4899    2.0000     0.2172     0.0000     0.0000    0.0000
   5000    0.5128    0.0306    0.0984    0.4898    2.0000     0.2751     0.0000     0.0000    0.0000
   6000    0.5220    0.0463    0.1210    0.4898    2.0000     0.3336     0.0000     0.0000    0.0000
   7000    0.5282    0.0619    0.1448    0.4899    2.0000     0.3925     0.0000     0.0000    0.0000
   8000    0.5324    0.0765    0.1695    0.4901    2.0000     0.4516     0.0000     0.0000    0.0000
   9000    0.5352    0.0897    0.1946    0.4904    2.0000     0.5109     0.0000     0.0000    0.0000
  10000    0.5370    0.1014    0.2198    0.4907    2.0000     0.5704     0.0000     0.0000    0.0000
  11000    0.5383    0.1115    0.2448    0.4911    2.0000     0.6300     0.0000     0.0000    0.0000
  12000    0.5391    0.1202    0.2692    0.4914    2.0000     0.6896     0.0000     0.0000    0.0000

## arm `one`  consumers 1
   tick         P         F     D_eff         N      De/D      grown    eaten_P    eaten_D   reserve
      0    0.3908    0.0000    0.0000    0.5000    0.0000     0.0000     0.0000     0.0000    0.5000
   1000    0.1997    0.0000    0.0107    0.4965    0.2403     0.0330     0.2128     0.0000    0.6277
   2000    0.1998    0.0000    0.0192    0.5078    0.3928     0.0630     0.2327     0.0000    0.6226
   3000    0.1998    0.0000    0.0269    0.5259    0.5084     0.0935     0.2532     0.0000    0.4464
   4000    0.1995    0.0000    0.0338    0.5298    0.5978     0.1241     0.2742     0.0000    0.2705
   5000    0.1997    0.0000    0.0401    0.5326    0.6702     0.1548     0.2946     0.0000    0.0938
   6000    0.1998    0.0000    0.0458    0.5113    0.7293     0.1853     0.3151     0.0000    0.0000
   7000    0.1996    0.0000    0.0509    0.5058    0.7782     0.2156     0.3356     0.0000    0.0000
   8000    0.1999    0.0000    0.0556    0.5038    0.8203     0.2457     0.3555     0.0000    0.0000
   9000    0.1996    0.0000    0.0598    0.5030    0.8548     0.2759     0.3759     0.0000    0.0000
  10000    0.2195    0.0000    0.0641    0.5142    0.1222     0.3073     0.3770     0.0000    0.0000
  11000    0.2422    0.0000    0.0689    0.5139    0.1437     0.3416     0.3770     0.0000    0.0000
  12000    0.2665    0.0000    0.0745    0.5127    0.1692     0.3785     0.3770     0.0000    0.0000

## arm `four`  consumers 4
   tick         P         F     D_eff         N      De/D      grown    eaten_P    eaten_D   reserve
      0    0.3908    0.0000    0.0000    0.5000    0.0000     0.0000     0.0000     0.0000    2.0000
   1000    0.1987    0.0000    0.0098    0.4966    0.2236     0.0306     0.2125     0.0000    2.1275
   2000    0.1996    0.0000    0.0183    0.5854    0.3819     0.0609     0.2319     0.0000    1.8951
   3000    0.1979    0.0000    0.0260    0.6226    0.4947     0.0928     0.2556     0.0000    1.1413
   4000    0.1986    0.0000    0.0330    0.6374    0.5848     0.1250     0.2771     0.0000    0.3843
   5000    0.1989    0.0000    0.0393    0.5468    0.6566     0.1568     0.2986     0.0000    0.0000
   6000    0.1980    0.0000    0.0451    0.5252    0.7147     0.1874     0.3202     0.0000    0.0000
   7000    0.1991    0.0000    0.0502    0.5170    0.7672     0.2177     0.3396     0.0000    0.0000
   8000    0.2022    0.0000    0.0549    0.5485    0.0271     0.2481     0.3568     0.0000    0.0000
   9000    0.2246    0.0000    0.0599    0.5668    0.0325     0.2811     0.3568     0.0000    0.0000
  10000    0.2488    0.0000    0.0654    0.5662    0.0391     0.3171     0.3568     0.0000    0.0000
  11000    0.2747    0.0000    0.0716    0.5631    0.0472     0.3561     0.3568     0.0000    0.0000
  12000    0.3022    0.0000    0.0786    0.5590    0.0569     0.3981     0.3568     0.0000    0.0000

## arm `recovery`  consumers 4  (removed at tick 6000)
   tick         P         F     D_eff         N      De/D      grown    eaten_P    eaten_D   reserve
      0    0.3908    0.0000    0.0000    0.5000    0.0000     0.0000     0.0000     0.0000    2.0000
   1000    0.1987    0.0000    0.0098    0.4966    0.2236     0.0306     0.2125     0.0000    2.1275
   2000    0.1996    0.0000    0.0183    0.5854    0.3819     0.0609     0.2319     0.0000    1.8951
   3000    0.1979    0.0000    0.0260    0.6226    0.4947     0.0928     0.2556     0.0000    1.1413
   4000    0.1986    0.0000    0.0330    0.6374    0.5848     0.1250     0.2771     0.0000    0.3843
   5000    0.1989    0.0000    0.0393    0.5468    0.6566     0.1568     0.2986     0.0000    0.0000
   6000    0.1980    0.0000    0.0451    0.5252    0.7147     0.0000     0.0000     0.0000    0.0000
   7000    0.2192    0.0000    0.0507    0.5149    0.8176     0.0316     0.0000     0.0000    0.0000
   8000    0.2418    0.0000    0.0568    0.5101    0.9229     0.0657     0.0000     0.0000    0.0000
   9000    0.2659    0.0000    0.0635    0.5068    1.0282     0.1025     0.0000     0.0000    0.0000
  10000    0.2915    0.0000    0.0707    0.5044    1.1313     0.1420     0.0000     0.0000    0.0000
  11000    0.3186    0.0000    0.0785    0.5025    1.2301     0.1843     0.0000     0.0000    0.0000
  12000    0.3469    0.0000    0.0869    0.5009    1.3229     0.2293     0.0000     0.0000    0.0000

## Summary at tick 12000  (material units; 12,000 ticks = 600 s of world time)
arm               n    P_end     grown   eaten_P   eaten_D  per-head  served%  below%    sat%  starved at tick
undisturbed       0   0.5391    0.6896    0.0000    0.0000    0.0000     NaN%    0.0%    NaN%                -
one               1   0.2665    0.3785    0.3770    0.0000    0.3770   100.0%   73.2%    0.0%             9074
four              4   0.3022    0.3981    0.3568    0.0000    0.0892   100.0%   65.2%    0.0% 7811,7811,7811,7811
recovery          4   0.3469    0.2293    0.0000    0.0000    0.0000     NaN%   50.2%    NaN%          -,-,-,-

## Stock and flow
- Undisturbed patch: P 0.3908 -> 0.5391 m standing, gross production 0.6896 m over 600 s (1.149e-3 m/s mean). Nothing was eaten, so the difference between production and standing stock is mortality into detritus.
- One consumer: took 0.3770 m in the 454 s it lived (8.310e-4 m/s mean), against 1.149e-3 m/s of gross production in the same patch undisturbed. Intake ran at 0.7x production.
- Four consumers took 0.0892 m each against 0.3770 m for the single one: per-head intake fell 4.2x. The cell served 100.0% of every request in both arms, so the competition is not a within-tick share — it is the standing stock the four of them hold the patch down to, and the shorter time each of them survives there.
- Depletion floor: a consumed patch sits pinned just under the feeding gate (feed_min 0.20); the `one` arm spent 73.2% of its horizon with no edible kind above that gate, the `four` arm 65.2%.
- Recovery: P was 0.1980 m when consumption ceased at tick 6000 and 0.3469 m at tick 12000 — 64.4% of the 0.5391 m the undisturbed patch held at the same tick, recovered over 300 s. Recovery is still in progress at the horizon, not complete.

## Why intake stopped, by cause
- Reserve saturation: 0.0% of the `one` arm's requesting ticks and 0.0% of the `four` arm's were refused because the animal was already full. That is the animal's limit, not the patch's.
- Mouth throughput: a unit adult's graze rate is 0.0350 m/s at saturation; the type-II term (K_P 0.45) cuts it to 0.0108 m/s at the P 0.20 a consumed patch settles to. Throughput is not what stopped these animals — they were taking what the law allows.
- Chemical quality: the `one` arm ended with 0.8805 m of detritus in the patch of which only 0.0745 m was edible (D_eff = D · min(1, rho/e_r)). Detritus was never above feed_min as *edible* material, so the scavenging gate never opened and eaten_D is zero — a quality refusal, not an absence of matter.
- Unreachable food: none by construction. Every other cell was emptied, so nothing in this fixture was out of reach rather than absent. A moving animal's reach is exactly what this staged measurement does not test.

## Not measured here (censored, not extrapolated)
- Anything after a consumer starved: each arm's intake horizon is its own, listed above.
- Fruit as a channel: F stayed at 0 in every consumed arm, so no frugivory rate was observed.
- Scavenging rate: the gate never opened, so no detritus intake rate exists to report.
- Whether a *moving* animal would do better: the fixture pins every body in place.
- Multi-cell patches, nutrient limitation at other light levels, and any other config.

# wall time 2.6 s
```
