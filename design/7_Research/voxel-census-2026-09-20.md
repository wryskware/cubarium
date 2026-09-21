---
design_status: exploration
date: 2026-09-20
---

# Voxel census, 2026-09-20: the seeded habitat does not last six hours

Two six-simulated-hour runs of `crates/cubarium/examples/voxel_census.rs`
on the default authored world and `habitat::seed`, one row per simulated
minute. Run A (Wrysk, worktree `census`, at a3761d5) is the world before the
live founders landed: eight legacy frondgrazers. Run B (Fable, at be9640f)
is the founder world: eight littershredders and eight frondgrazer founders,
hungry, with their heuristics and the live litter field. CSVs are disposable
(`runs/voxel-census-6h.csv`, `runs/voxel-census-founders-6h.csv`).

## Plants: the same collapse in both runs, to the minute

| species | seeded | first zero, run A | first zero, run B |
| --- | --- | --- | --- |
| springturf | 16 | min 16 | min 16 |
| glowcap | 8 | min 139 | min 156 |
| velvetpad | 10 | min 180 | min 180 |
| bloomcrown | 14 | min 250 | min 250 |
| umbrellafrond | 8 | min 250 | min 250 |
| stonecushion | 6 | never | never |

Flora births stay at the 62 seeded stands in both runs: no establishment
ever happens. The plant collapse timeline is unchanged by removing the
grazers, adding two founder lineages, or a 65-animal boom, so its cause is
on the plant or habitat side (placement, water, light, or the seeded
half-wood start), not consumption. The replacement study of 2026-09-16
(`voxel-round3-experiment-2026-09-16.md`) had residents persisting 1.8–5 h in
its own arena, so the difference is this habitat, not the plant model alone.

## Animals: boom and total loss inside half an hour

Run A: 8 legacy frondgrazers → 47 at minute 1, 178 births, 178 deaths,
extinct at minute 47.

Run B: 16 founders → 65 at minute 5 (littershredder peak 44 at minute 3,
frondgrazer 21 at minute 2), then 0 births after minute 5 and 65 deaths by
minute 30; littershredders extinct at minute 19, frondgrazers at minute 26.
Litter kept rising through the die-off (1.6 → 6.1 by minute 20, from the
dead springturf), so the shredders did not run out of litter in the world;
they failed to reach or use it. The fauna ledger records deaths without a
cause, so starved versus drowned is not readable from the census.

Founder reproduction as landed (`voxel-live-founders-2026-09-20.md`): a
browser founder breeds on its first tick at full stores and every tick it
can (birth_body 0.03 < body_max 0.05, cost 0.01, no cooldown); the blind
founder only at full body and full reserve (0.0125 / 0.00625). A newborn
arrives at `body_min` with `birth_cost − body_min` of reserve, hungry, at its
parent's pose.

## What this licenses

Not tuning. Two diagnoses, both read-only first: (1) why every seeded plant
except stonecushion dies on a fixed schedule in the authored habitat when the
same species persist in the study arena; (2) what kills the founders in the
first half hour, by cause, and whether newborns can reach food at all. The
first live ambient instance waits on both.
