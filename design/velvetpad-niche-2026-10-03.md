---
date: 2026-10-03
author: Fable, from the plants3 worker's diagnosis
status: proposal (for Astra's review, then Wrysk)
---

# Velvetpad's niche: shaded, soggy ground

Wrysk, 2026-10-03: "most of the world is overrun with velvetpad.. which is fine but like,
basically every other plant should out-compete it (im sure we can have some niche where
velvetpad wins, like idk, soggy places that rarely dry maybe)."

## Diagnosis

Measured on the plants-only terrarium, branch `plants3` v1, seed 3, at 6 h. Tooling:
`crates/cubarium-voxel-flora/examples/stand_niche_diag` (commit ade04225).

1. **It holds 7,687 of 13,226 soil sites**, about 58 %. On main at 3–4 h it held
   5,400–7,400 in the four seeds measured.
2. **Its establishment gates pass on 97–100 % of moist and wet sites,** lit or shaded.
   - The bloomcrown's pass only on lit sites.
   - The terrarium has no dry sites, so no water niche excludes velvetpad anywhere.
3. **Shade costs it nothing.** Its `light_half` is 0.1, with the same assimilation as every
   other plant. Its senescence is halved, so it breaks even at L·μ 0.26, against:
   - 0.49 for the bloomcrown, umbrellafrond and lanternberry;
   - 0.41 for the springturf.
4. **It spreads fastest.** It grows twice as fast as the others and spreads by spores (hop 6)
   plus 30 % clonal growth. As ground cover, only woody seeds come up through it, at 0.1 per
   check.

So it wins every non-dry site, in light or shade, and holds them.

## Proposal

**Velvetpad becomes the plant of shaded, soggy ground.** Everywhere else, the others beat it.

| knob | today | proposed | effect |
|---|---|---|---|
| wilt / full / establish water | — | 0.9 / 1.3 / 1.0 | needs ground near or above saturation |
| `saturated_max` | — | 1.0 | tolerates waterlogging |
| aeration stress | on | off | soggy ground doesn't harm it |
| senescence | halved | 0.001 (the common value) | loses its cheap break-even |
| establishment light ceiling (new knob) | none | about 0.6 | lit wet hollows stay umbrellafrond's |

- **Drained ground** goes to the bloomcrown, springturf, stonecushion and lanternberry.
- **Lit wet ground** goes to the umbrellafrond.
- **Niche size:** on seed 3 at 6 h, 2,084 of 13,226 soil sites (about 16 %) are shaded and wet
  or soggy.

## Questions for Astra

1. Is the diagnosis complete? Is anything else (grazing refuge, litter, the woody-seed gate)
   carrying velvetpad, so that this change would leave it dominant or crash it?
2. Is a light ceiling on establishment the right lever? The alternatives are a shade-tolerant
   but sun-intolerant assimilation curve, or competition for light under canopy.
3. Will 16 % of sites, fixed by hydrology, keep velvetpad persistent? Is it robust to
   droughts?
4. The grazers that eat velvetpad: does halving its area starve them? Check them against
   the balance measures.
5. What tests and measure would show the niche works: site occupancy by water and light
   class, over 6 h, seeds 1/3/5/6?
