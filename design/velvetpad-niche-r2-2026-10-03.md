---
date: 2026-10-03
author: velvet worker (opus), from `design/handoffs/velvetpad-study-2026-10-03.md`
status: proposal (revision 2 of `design/velvetpad-niche-2026-10-03.md`; for Fable, then Wrysk)
---

# Velvetpad's niche, revision 2: measured first

Tool: `crates/cubarium/examples/velvet_niche.rs` (`config`, `grid`, `drought`, `run`) over
`cubarium_voxel_flora::study` (the tick's own `grow_stand` under held conditions).

## Pass condition (fixed before the paired runs)

Written and committed before any paired run started. Plants-only terrarium
(`config/desktop/terrarium.toml`, `velvet_niche run`, no fauna), seeds 1, 3, 5 and 6, 6 h,
today's parameters against the candidate, same seed. Classes are read at each sample on
velvetpad's root box at today's rooting: **niche** = mean available water ≥ 1.2 or soggy
(saturated fraction ≥ 0.6); **non-niche** = everything drier. Shade = terrain sky < 0.6 or
canopy transmission at pad height < 0.5.

*Revised before any candidate run* (the first draft used water ≥ 1.0): the baseline's
class counts showed drained terrarium ground sits at field capacity. Seed 3 at minute 40:
5,773 sites at 0.8–1.0, 4,197 at 1.0–1.2, 891 at 1.2–1.6 and 1,900 soggy, of 13,226. A
boundary at 1.0 would call half the world the niche; at 1.2 the niche is about a fifth.

Non-niche sites change occupants:

- **N1.** At 6 h, velvetpad's share of occupied non-niche sites is at most half of the
  baseline's share and at most 25 %, in all four seeds.
- **N2.** At 6 h, occupied non-niche sites are at least 70 % of the baseline's count (the
  ground velvetpad leaves is taken, not left bare), in at least three of four seeds.
- **N3.** Retreat from an invaded world: each seed's baseline saved at minute 240, continued
  2 h under the candidate (configuration replaced on load) and under today's. Under the
  candidate, at least 25 % of the non-niche sites velvetpad held at the load are held by
  another species at the end, and at least twice the baseline continuation's share, in at
  least three of four seeds.

The niche persists and replaces itself:

- **P1.** At 6 h velvetpad has at least 100 stands and at least 10 donors, in all four seeds.
- **P2.** Over hours 4–6, velvetpad's births (germinated + clonal) are at least 0.8 × its
  deaths, and its stand count at 6 h is at least 0.8 × its count at 4 h, in all four seeds.
- **P3.** At 6 h velvetpad is the most common occupant of shaded niche sites, in at least
  three of four seeds. Lit wet sites may go to the umbrellafrond; nothing is required there.
- **D1.** Drought (single-stand probe, `velvet_niche drought`): an adult velvetpad at wet
  ground (available water 1.2) and light 0.2 survives a 7,200 s dry spell (longer than the
  preset's 6,587 s seed bank) at available water 0.6, and funds at least one package in the
  day after it.

A candidate passes when every line passes. A line that fails is reported with its numbers,
not re-tuned around.

## 1. Resolved configuration (terrarium preset, 0.125 m voxel)

`velvet_niche config`. Day clock 120 min, daylight 0.67, plant night factor 0.05, Q10 2.

| knob | velvetpad | bloomcrown | springturf | stonecushion | umbrellafrond | lanternberry |
|---|---|---|---|---|---|---|
| α / g / m | 4 / .00656 / .000328 | 3.19 / .00656 / .000328 | 1.29 / .0131 / .00328 | 4 / .00656 / .000082 | 3.19 / .00656 / .000328 | 3.19 / .00656 / .000328 |
| senescence | .000581 | .00179 | .00156 | .000348 | .00179 | .00179 |
| light_half | 0.1 | 0.8 | 1.0 | 0.3 | 0.15 | 0.4 |
| wilt / full / est water | .25 / .9 / .4 | .05 / .5 / .1 | .1 / .5 / .3 | 0 / .2 / .05 | 1 / 1.5 / 1.2 | .1 / .5 / .4 |
| est light (terrain sky) | 0.05 | 0.6 | 0.75 | 0.4 | 0.1 | 0.35 |
| sat_max / drown m | .6 / .1 | .25 / .05 | .3 / .03 | .4 / .02 | 1 / .5 | .5 / .1 |
| foliage / wood rate | .0061 / .0033 | .0016 / .0016 | .0068 / .0164 | .00076 / .00033 | .0016 / .0016 | .0016 / .0016 |
| propagule rate, dispersal | .00082, spores hop 12 ×2, clonal .3 | .00066, drop 4 | .0033, drop 6, clonal .3 | .000082, wind 2 | .00033, spores 12, clonal .5 | .0020, drop 4 |
| seed age s / overtop | 6587 / 0 (cover) | 17565 / 0.1 | 26348 / 0 (cover) | 26348 / 0 (cover) | 8783 / 0 | 17565 / 0.1 |
| **BE L_eff·μ, reference** | **0.178** | 0.516 | 0.506 | 0.100 | 0.516 | 0.516 |
| BE, day means | 0.184 | 0.530 | 0.534 | 0.103 | 0.530 | 0.530 |

Reference assumptions: full foliage, site mineral 1 (Monod 2/3), no aeration stress,
daylight 1, upkeep 1, no growth or propagules. "Day means": income × mean daylight 0.67,
upkeep and shedding × their day means (0.72, 0.69). Measured site mineral under stands
stays 0.92–1.0 through 6 h, so the grid below runs at mineral 1.

## 2. Viability grid

`velvet_niche grid`: one stand, the tick's `grow_stand` and donor funding, one world day
on the preset clock; held water (available, 0.4–1.4, a half-saturated and a saturated
box), raw light L = terrain sky × canopy transmission (sky 0.05–1, canopy 1–0.1), mineral
1. Physiology reads only L; establishment reads sky, water and saturation. L* is the
least L that works.

| species | establishment gate | adult net ≥ 0 | newborn reaches donor in a day |
|---|---|---|---|
| velvetpad today | water ≥ 0.4, sat ≤ 0.6, sky ≥ 0.05 | L ≥ 0.05 at water 0.6–1.6 | L ≥ 0.08 (0.3 at water 0.6) |
| bloomcrown | sky ≥ 0.6, sat ≤ 0.25 | L ≥ 0.5 | L = 1 only |
| springturf | sky ≥ 0.75, sat ≤ 0.3 | L ≥ 0.5 | L ≥ 0.5 |
| stonecushion | sky ≥ 0.4, sat ≤ 0.4 | L ≥ 0.05 | L ≥ 0.8 (alive at 0.05, slow) |
| umbrellafrond | water ≥ 1.2 | water 1.4 L ≥ 0.7; soggy L ≥ 0.3 | never within a day |
| lanternberry | sky ≥ 0.35, sat ≤ 0.5 | L ≥ 0.4 | L ≥ 0.8 |
| velvetpad C1 | water ≥ 1.2, any sat, sky ≥ 0.05 | water ≥ 1.2: L ≥ 0.05; water ≤ 1.0: never (loses 77 % of wood in a day) | L ≥ 0.15 (1.2), 0.08 (≥ 1.4) |

Today's velvetpad is viable on every cell but the driest and the wholly saturated one,
in light down to 0.05, and an adult funds about 90 packages a day. No competitor's adult
breaks even below L 0.4–0.5 except stonecushion, whose newborn grows slowly; no
competitor's seed starts below sky 0.35.

## 3. Which advantage carries dominance (isolation screen)

From founding, seeds 1 and 3, 1 h, each arm removing one advantage. Velvetpad stands
(baseline 2,545 / 2,797) and its share of occupied drained sites (0.72 / 0.62):

| arm | stands | drained share |
|---|---|---|
| water knobs (C1 below) | 155 / 380 | 0.01 / 0.00 |
| senescence at bloomcrown's 0.00179 | 1,130 / 1,360 | 0.47 / 0.39 |
| dispersal (propagule .00033, no runners) | 1,626 / 1,857 | 0.58 / 0.49 |
| growth rates at the shrubs' | 1,675 / 1,997 | 0.62 / 0.52 |
| establishment light 0.4 | 1,697 / 2,097 | 0.62 / 0.53 |
| light_half 0.4 | 2,166 / 2,291 | 0.67 / 0.57 |

The water knobs carry it. Competitors' counts barely moved in any arm within the hour.

## 4. Retreat and invasion

- **Today, established mats hold.** Baseline worlds saved at minute 240, continued 2 h:
  of 5,439–7,123 drained sites velvetpad held, 2–5 % changed hands, every one to a woody
  overtop (lanternberry, bloomcrown, vaulttree) or a glowcap; 4–5 went vacant. Across the
  6 h baselines velvetpad lost about 110 sites per 6,400 it gained.
- **Competitors on vacancies**: only where the terrain sky passes their gate (0.35–0.75)
  and, as adults, L ≥ 0.4–0.5. Springturf, stonecushion and umbrellafrond cannot take an
  occupied pad (overtop 0); bloomcrown, lanternberry and vaulttree can, at 0.1 a check.
- **C1 on drained, lit ground**: μ = 0 at water ≤ 1.0, so an adult runs on reserve, then
  diebacks: −77 % wood in one world day (probe). In the world, 2 h after the
  configuration replacement, 7–16 % of the drained velvetpad sites had changed hands
  (vacant, glowcap, lanternberry), against 2–5 % today.
- **C1 over 4 h, seed 3 (beyond the pass condition):** 70 % of the 7,123 drained
  velvetpad sites are still velvetpad; the rest went to glowcap 760, lanternberry 480,
  vacant 455, bloomcrown 187, stonecushion 114, umbrellafrond 72, springturf 57.
  Velvetpad stands rose 7,523 → 8,217 in the first hour, then fell to 7,046 at 4 h:
  drained terrarium ground swings across 1.0–1.2 with the rain, so banked spores still
  find passing faces after showers and adults are not at μ = 0 for long. **Water stress
  alone does not evict established mats on this preset's weather.**
- **Dry spell (D1)**: an adult at water 1.2, L 0.2, dropped to water 0.6 for 7,200 s
  (> 6,587 s seed bank): today it does not notice (89 packages the next day); under C1
  its wood falls to 0.050 (alive 0.015), it recovers to full and funds 26 packages in the
  following day. A newborn grown in the wet survives the same spell.

## 5. Candidate C1 (water budget only)

| knob | today | C1 |
|---|---|---|
| `velvetpad.wilt_water` | 0.25 | **1.0** |
| `velvetpad.full_water` | 0.9 | **1.3** |
| `velvetpad.establish_water_min` | 0.4 | **1.2** |
| `velvetpad.establish_saturated_max` | 0.6 | **1.0** (aeration target 0; relaxation kept) |

`drown_depth_m` stays 0.1. No light ceiling, senescence, growth, dispersal or overtop
change: the isolation screen shows the water knobs alone remove the drained-ground
dominance, and changing more would hide which advantage remains (Astra finding 4).

## 6. Paired plants-only check (seeds 1/3/5/6 × 6 h) against the pass condition

| seed | drained share base → C1 | drained occupied base → C1 | N3 C1 / today | velvet stands, donors | births/deaths h4–6 | stands 6h/4h | shaded-niche top |
|---|---|---|---|---|---|---|---|
| 1 | .747 → .045 | 9,949 → 4,156 | .048 / .023 | 836, 634 | 346 / 161 | 836 / 652 | velvetpad 266 > umbrellafrond 215 |
| 3 | .716 → .055 | 9,842 → 5,305 | .078 / .046 | 1,262, 814 | 1,138 / 554 | 1,262 / 721 | velvetpad 373 > umbrellafrond 211 |
| 5 | .699 → .091 | 9,241 → 4,029 | .071 / .028 | 2,347, 1,684 | 1,081 / 49 | 2,347 / 1,315 | velvetpad 526 > glowcap 268 |
| 6 | .667 → .026 | 9,285 → 5,088 | .104 / .032 | 616, 539 | 130 / 44 | 616 / 530 | umbrellafrond 310 > velvetpad 64 |

N1 pass (4/4). **N2 fail (0/4)**: drained ground is 41–57 % as occupied as the baseline at
6 h. It is still filling (seed 3: 2,152 → 3,592 → 5,305 at 2/4/6 h; lit drained ground
40–66 % occupied, shaded 36–53 %), mostly by glowcap (1,500–2,300), springturf, lanternberry,
stonecushion and bloomcrown. **N3 fail (0/4)**: 5–10 % of drained mats lost in 2 h, against
the 25 % asked; a probe adult at μ = 0 dies in about 3 h (reserve, then dieback at the
maintenance rate), but in the world drained ground rewets after showers (section 4). P1 pass, P2 pass, P3 pass (3/4; seed 6 umbrellafrond). D1 pass.
Whole-world stands at 6 h fall from 8,770–9,764 to 3,541–5,283, and litter from
2,174–4,332 to 730–1,465. Velvetpad drownings rise (seed 3: 626 in 6 h): with saturation
allowed, it starts on faces beside pools that later flood.

C1 therefore **does not pass**. It gives the niche (velvetpad confined to wet and soggy
ground, persisting and replacing itself there) but the rest of the world is slow to be
taken: the competitors fill freed ground over more than 6 h, and established mats retreat
over about 3 h.

## 7. Open decisions (not settled here)

1. **Accept slow turnover, or speed it.** Candidates, each its own test: an adult
   water-stress dieback faster than maintenance-rate dieback (new knob, default off); a
   positive `overtop_per_check` for non-woody competitors scoped to velvetpad (needs a
   target restriction: today a positive value lets a species overtop every ground cover).
2. **Drained shade stays thin.** No competitor seed starts below terrain sky 0.35 or breaks
   even below L 0.4 (stonecushion excepted, slow). Filling it is a competitor change.
3. **Drowning sink** on saturated faces beside pools under `establish_saturated_max` 1.0.
4. **Fauna** (Astra finding 6, Fable's): the plant layer halves and litter falls 3–4×.

Proposed live preset change, **not applied**: the four C1 values above, if Fable accepts
the slow turnover or pairs it with one of item 1.
