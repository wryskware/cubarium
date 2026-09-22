---
status: open
date: 2026-09-22
owner: the terrain-generation line (Wrysk's thread); from Fable's organism line
---

# Note to the terrain line: the small preset has no wet ground

D5 (`design/7_Research/voxel-census-2026-09-20.md`, "D5 — the small
preset"; brief `voxel-small-collapse-2026-09-22.md`) measured the panel's
`small` recipe against `default` at seed 1, both charged with the same 1.5 m
of water per m²:

| | small | default |
| --- | --- | --- |
| pore water per m² | 0.045 | 0.068 |
| pooled per m² | 0.037 | 0.130 |
| aquifer per m² (head) | 0.574 (1.64 m) | 0.378 (1.08 m) |
| lake | 7.2 m² on 60 m² | 66 m² on 192 |
| columns eligible for umbrellafrond at t = 0 | 0.4 % | 27.8 % |
| columns eligible for velvetpad | 0.2 % | 5.1 % |

Small buries 38 % of its inventory under a lake floor lifted to 1.625 m and
keeps 2.4 % in pools; the seeder finds no standing wet ground, so the two
producers that are solvent anywhere are never planted and the ring's plants
are gone in six hours. The levers named by the evidence are the recipe's
`lake_depth_m` (0.375), the tier-0 trough, and sizing the inventory against
the higher lake floor. The organism line is not changing the recipe; two
related units-and-rules fixes are its own (water conductivity in metres per
second, package 1c; seeding after the first shower, package 4). After either
recipe change, `voxel_plant_autopsy 6 preset=small` is the check, and the
eligibility line at t = 0 is the quick one.
