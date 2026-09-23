---
status: open
date: 2026-09-22
owner: the terrain-generation line (Wrysk's thread); from Fable's organism line
---

# Note to the terrain line: `water_depth_m` counts water that is not standing

`World::water_depth_m` (`crates/cubarium-voxel/src/world.rs`) sums the fills of
every contiguous wet void cell above a support face. Flora drowning and
establishment, the habitat seeder's wettest-face watch, and every fauna water
rule (wade refusal, drowning, clutch loss, standable faces, the contact `Wet`
channel) read it.

**What the organism line found (P5-A, 2026-09-22).** On `default` seed 1 a
browser drowned while resting on a face that had held 0.044 m a tick before:
a shower's falling column (fills 0.11 / 0.25 / 0.37 / 0.26 / 0.04 up five
cells) read as 0.257 m of depth over about 0.03 m of water on the ground. Of
six drownings across small/default/wide seeds 1–2 at 60 min, three were this
reading and three were real puddles rising (bottom cell 0.78–0.82 full).

**Why the organism line did not patch it.** Two simple rules were measured
against every support face (probe: settle 600 ticks, then the opening shower,
presets `default` and `small`, seed 1):

| rule | settled `default`: faces changed (> 5 cm) | faces that stop reading > 0.2 m |
| --- | --- | --- |
| stop at the first cell not full (1e-6) | 899 (765), max 0.88 m | — |
| stop at the first cell under 0.5 full | 245 (103) | 81 |
| count above only if the bottom cell is ≥ 0.5 full | 227 (96) | 81 |

A settled pond's cells sit 0.87–0.96 full with water above them, so "full"
cannot mark standing water, and even the settled world has about 200 faces
whose bottom cell is under a quarter full beneath up to 0.88 m of water —
presumably streams and falls still moving at the settle cap. No fill threshold
separates standing water from water in transit; the solver's state would have
to say which is which.

**The ask.** A solver-side definition of standing depth at a face (water
supported from below, not falling through), exposed through the view, so the
readers above can switch to it. Whatever the definition, the check the
organism line will run after it is `voxel_founder_autopsy 60` over seed bases
1–8 with drowning causes from the ledger.

Until then the organism line trains on the world as it is (P5 D6's frozen
mid-shower state included), so policy and live physics agree.
