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
velvetpad's root box at today's rooting: **niche** = wet (mean available water ≥ 1, i.e.
above field capacity) or soggy (saturated fraction ≥ 0.6); **non-niche** = dry or moist.
Shade = terrain sky < 0.6 or canopy transmission at pad height < 0.5.

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
