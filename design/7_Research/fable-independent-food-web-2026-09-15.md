---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# An independent food-web reconsideration (Fable, Stage 1)

Written blind to `design/ecological-niches-reconsideration-2026-09-15.md` and
the other 2026-09-15 ecology analyses. Read: `ecology.md`, `fauna-v2.md`, the
M2 conversion table and controller, the apex and recurrent plans, the
twelve-hour and development-flow evidence, `config.rs` defaults, and the live
`/status` (tick 28590, 82 organisms; per-kind counts are not exposed there).

## What the current rules predict (evidence)

| Owner's observation | Mechanism in the current rules | Support |
| --- | --- | --- |
| Booms | A fed adult refills 0.6 `R_max` in about a minute and buds every ~90 s after 120 s of age; a stripped cell needs minutes to regrow (`g = 0.008/s`, slowed by the Monod nutrient term). Consumers double faster than plants recover. | Rosenzweig 1971: enrichment plus saturating (type II) intake gives limit cycles or crash. `P_max = 1.5`, `K_P = 0.45`, `feed_min = 0.2` sit on the unstable side. |
| Depleted vegetation | Bodies lock material: ~1.3 k m exists cube-wide, and 300–500 bodies at 1–2 m each hold a third or more of it out of `N`. Corpses return through `D → N` at `k_d = 0.002/s` (eight-minute e-fold), so plants stay starved long after a crash. | Conversion table. |
| One persistent swarm | Every death, feces and leaf-fall rolls downhill (`fall = 0.02/s`) onto the soil floor, and fresh detritus is fully edible (`e_d_max = e_r`). The floor is a donor-controlled subsidy that peaks after each foliage boom, so the scavenger guild outlives everyone else. | Twelve-hour seed 2: 92 burrowers, 19 gliders, 0 grazers, 0 skimmers. Moore et al. 2004: detritus channels are donor-controlled and decouple detritivores from live production. |
| No stable specialists | `diet` splits one mouth rate linearly; a generalist loses nothing per unit food and gains insurance, and mutation drifts `diet` freely. | Futuyma & Moreno 1988: specialization needs a concave trade-off or a coarse-grained habitat. Neither exists. |
| Whole-cube synchrony | Litter fall, `N` diffusion and a shared cap couple all bands; only weather blobs and the depth drive desynchronize. | Huffaker 1958: oscillations persist only with dispersal barriers and out-of-phase patches. |

The skimmer losses (all starvation after initial growth) and the Lanternjaw
juveniles (15 births, none reached adult size) are the same budget failure at
two trophic levels: per-body intake cannot cover growth plus upkeep once a
patch is shared.

## Recommended model (design choices)

Keep the accounting, the five pools, the bands and the RNN controllers.
Change the rate structure, not the inventory of mechanisms.

1. **Pace consumers below plant recovery.** Gestation of minutes, a child that
   costs most of an adult reserve, adult size reached only by paid intake.
   Target: consumer doubling time ÷ stripped-cell recovery time > 1. Today it
   is well below 1.
2. **Give plants an ungrazable residual.** Type III intake (`P²/(P² + K²)`)
   or a higher refuge, so intake collapses before `P` does. Noy-Meir 1975:
   this turns discontinuous collapse into a stable low-crop state. Smallest
   change with the largest predicted effect on "plants look gone".
3. **Make specialization concave.** Assimilation depends on diet match
   (specialist ~0.6, half-and-half ~0.35) and each food class carries its own
   handling time. With concave returns, `diet` should segregate into leaf,
   fruit and detritus lineages instead of drifting to the middle.
4. **Localize the detritus subsidy.** Litter stays mostly where it falls
   (fall an order of magnitude lower); feces stay energy-free; decomposition
   scales with moisture so wet soil recycles fast and dry canopy slowly. The
   scavenger guild then lives on local turnover, not cube-wide mortality.
5. **Predation as a gate, not a fix.** A predator adds another Rosenzweig
   layer; add it only after 1–4 hold. It must satisfy the same doubling-time
   rule and take adults, with a speed trade-off for prey. Dormancy is compatible.
6. **Spatial decoupling.** Local nutrient diffusion, weather asynchrony kept,
   bands treated as partially isolated patches; report per-band population
   cross-correlation.

Not recommended: new pools, chemical signals, a second producer, waste stress,
quotas or reseeding. The model adds one handling time per food class and one
efficiency curve.

## Resource budget (rough, to be measured)

| Quantity | Estimate | Basis |
| --- | --- | --- |
| Gross producer growth, cube-wide | ~1 m/s | 1,280 cells at P≈0.5, L≈0.6, W≈0.7, N≈0.5 |
| Upkeep per size-1 body | ~0.005 e/s ≈ 0.004 m/s of leaf | `c_maint`, `c_move`; 1.36 e usable per m grazed |
| Sustainable consumers, evenly spread | ~150–250 | ratio of the rows above |
| Consumer doubling at saturated intake | ~2–3 min | bud rules |
| Stripped-cell recovery to 0.75 m | ~5–10 min | logistic regrowth with Monod term |

## Smallest discriminating tests (headless, ≤2 h simulated, 3 seeds)

- **Boom ratio:** fed-cohort doubling time vs 3×3 patch recovery, before and
  after change 1. Pass: ratio > 1.
- **Residual:** type II vs type III intake, all else fixed; minimum cube-wide
  `P` and time below `feed_min`. Pass: no cube-wide famine window.
- **Specialization:** concave vs linear efficiency; `diet` histogram at 2 h.
  Pass: multimodal, not central.
- **Subsidy:** litter fall 0.02 vs 0.002 /s; floor `D` after the first
  foliage crash and burrower share. Pass: burrowers under half afterward.
- **Coupling:** per-band cross-correlation at lag 0. Pass: below ~0.5 in at
  least two band pairs.

## References

- Rosenzweig, M. L. 1971. Paradox of enrichment: destabilization of exploitation ecosystems in ecological time. *Science* 171:385–387.
- Noy-Meir, I. 1975. Stability of grazing systems: an application of predator–prey graphs. *Journal of Ecology* 63:459–481.
- Moore, J. C. et al. 2004. Detritus, trophic dynamics and biodiversity. *Ecology Letters* 7:584–600.
- Futuyma, D. J. & Moreno, G. 1988. The evolution of ecological specialization. *Annual Review of Ecology and Systematics* 19:207–233.
- Huffaker, C. B. 1958. Experimental studies on predation: dispersion factors and predator–prey oscillations. *Hilgardia* 27:343–383.
