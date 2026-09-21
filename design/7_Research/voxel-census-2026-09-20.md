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

## Diagnoses (D1, D2; 2026-09-20, at edfa8f6)

Brief: `design/handoffs/voxel-collapse-diagnosis-2026-09-20.md`. Examples:
`crates/cubarium/examples/voxel_plant_autopsy.rs` (edfa8f6) and
`voxel_founder_autopsy.rs` (23f81f7); the fauna ledger now books departures
by cause and lineage (93907e7, snapshot schema 7). Fable re-ran both
autopsies; the numbers below reproduced.

### D1 — the authored world is dry, so every plant's income is zero

`cubarium_voxel::Config::default()` has no rain and no aquifer head, and
`scene::authored` only pours free water into the bowl's air cells; pore
water is never created. At seeding all 62 founders read mean pore 0.0000 and
μ = 0, and 0 of 3,072 skyline columns pass any species' pore gate, at t = 0
and at 6 h. Deaths are pure reserve-then-dieback timing ordered by each
species' maintenance: a plant-only run (fauna never stepped) reproduces the
census to the minute (springturf 15, glowcap 155, velvetpad 179, bloomcrown
and umbrellafrond 249; 56 deaths, 0 drownings). Umbrellafrond stands in
0.4999 m of pool water with no soil in its root box (the bowl is carved to
rock) and dies of thirst in the pond. Stonecushion is not surviving; it is
the slowest bill (W 0.050 → 0.0215 at 6 h, extrapolated death near minute
615).

Why the study arena kept them: its generated world runs rain 2e-4 m/s with
an open outlet and a charged aquifer; moisture 0.85–1.00, zero deaths, same
presets and start size. Only the water differs.

Why nothing germinates: no propagule ever lands (a founder can fund a parcel
only from reserve above half its cap, and with zero income that pot is spent
once: 40–96 % of a package), and even a full bank has no eligible column
(the pore gate refuses all 3,072). The seeder's site proxy never consults the
establishment predicate: 62 of 62 founders stand where their own seeds are
refused.

Proposed, not made: give the ambient world the study arena's water budget.
On the authored fixture that floods (no outlet cell exists): with arena rain
and a 1.5 m head, at 1 h the presets are solvent at 8–19× maintenance with 47
establishments, but 14 of 15 deaths are drownings and 1,245 columns are
under standing water. So the real choice is between wetting the authored
fixture (it needs an outlet) and running the ambient habitat on the
generated world. Two presets may have no niche here at all once wet:
umbrellafrond (rock bowl, no root soil) and velvetpad (its establish pore
0.30 is above soil's field capacity 0.25).

### D2 — all 65 founders starve; the blind heuristic freezes and cannot feel a drop

All 65 deaths are starvation (44 littershredder, 21 frondgrazer; 0 drowned),
every body at `body_min` with reserve 0. Newborn placement is fine: 49 of 49
born within reach of food (blind max 0.18 m of 1.5 m; browser 0.38 m of
2.0 m). Blind births leave the parent at exactly zero reserve (`birth_cost`
equals the full reserve) and each standing founder paid three births in its
first 111 s, refilling twice; the births are not the killer, they are what
stacks seven bodies on one tile.

Blind lineage: 0 of 44 travelled more than 1 m in its life, 36 travelled
0.000 m, while holding forward 1.0 every tick and paying the motor bill; not
one non-zero turn sample in 30 minutes. `BlindForager` returns turn 0
whenever a litter cue ≥ 0.02 is present with a flat trend, and after minute
1 litter lies on 70 sites so the diffuse cue is always present and flat. The
wall branch cannot rescue it: `contact_readings` only feels a solid at body
height, and 452 of 542 littershredder body-minutes had a drop or the strip's
end ahead, refusals the founder motion rule makes (destination must be a
support face at exactly the body's own standing height) and the receptors
cannot sense. Result: six stacks of seven bodies, each tile eaten out by
minute 3, fifteen minutes of starving in place while world litter rose
1.6 → 6.3.

Browsers fail differently: 14 of 21 travelled over 1 m (to 225 m), 2,822
bites, but the foliage under them collapses (D1) and only 2.2 % of browser
body-minutes had a stand inside the 0.0625 m mouth reach.

Newborn budget from the constants: 1,000 s at rest, 500 s at cruise (62.5 m
blind, 125 m browser). Observed newborn lifetimes 504–884 s: they cruise.

Proposed, not made: make `BlindForager` treat "cue present, trend flat" as
the wander case unless the previous interval actually delivered forward
motion (the delivered-forward channel already exists in Self). Second
candidate, unmeasured: the founder motion rule's same-height requirement
confines a founder to the flat patch it stands on; how large those patches
are on this landform was not measured.

## Closed water budget (route B), 2026-09-20, at 72101aa

With the closed budget on the **generated** world (brief and integration
note: `design/handoffs/voxel-water-cycle-2026-09-20.md`), the six-hour
census reads at 6 h: bloomcrown 42, umbrellafrond 149, springturf 34,
stonecushion 34, velvetpad 27, glowcap 0 (first zero at minute 307); flora
births 376 against 62 seeded, 90 deaths. Reproduced exactly by Fable. The
same build on the authored open world reproduces the collapse above. Water
was the whole plant story; glowcap (a dead-wood decomposer) and the founders
(D2) are the remaining questions.

## D3 — generated closed-world browsers starve at the mouth-access boundary, 2026-09-21

Brief: `design/handoffs/voxel-browser-autopsy-2026-09-20.md`. The instrumented
autopsy ran the trained built-in centres for 60 simulated minutes on the
generated closed world:
`voxel_founder_autopsy 60 generated closed`. A second arm used the same seeder,
water budget, controllers and horizon with four founders per lineage:
`voxel_founder_autopsy 60 generated closed half`. Raw CSVs were disposable
under `/tmp`; the harness now records cause deltas, exact mouth candidates,
same-height support counts, species bites, local foliage, cone sectors and
crown regrowth.

The trained arm introduced 16 bodies and born bodies brought the total to 100.
The browser lineage had 33 deaths, all starvation; it had no drowning or
terrain-removal deaths. Deaths by the integer part of the recorded death minute
were: `10:2, 11:5, 12:1, 13:1, 14:1, 15:2, 16:2, 17:2, 18:3, 19:2,
21:3, 22:1, 23:1, 25:1, 27:1, 31:1, 34:1, 41:1, 45:1, 54:1`. Every
death row held the browser at approximately `body_min` with zero reserve. The
first death was at 10.01 min and the last at 54.73 min.

At death, all 33 bodies had a live stand with foliage within the 2 m sensed
range; nearest-stand distance averaged 0.957 m, with a 0.176–1.813 m range.
The nearest species were umbrellafrond 14, stonecushion 8, velvetpad 6,
glowcap 4 and springturf 1. Only 2 of 33 nearest crowns touched the actual
0.0625 m mouth probe. The same-height geometry did not confine the bodies:
there were 11–31 support faces at the body's standing height within 2 m,
mean 25, and 1–3 immediately legal exits, mean 2.30. The browser could be
near food in cone space while lacking an edible crown at its mouth.

The browser bit every living species except stonecushion: bloomcrown 1,657
bites / 0.6435988 organic, umbrellafrond 8,387 / 0.4355061, springturf 236 /
0.1178078, velvetpad 5,224 / 1.9708931 and glowcap 3,417 / 0.2377075. The
trained arm's cumulative browser intake stopped after minute 45: it reached
18,921 bites and 1.3893157 assimilated organic, with no further bites through
minute 60. Foliage still stood near the surviving browsers at the stop:
4.5179 organic within 2 m at minute 45, while 13.6846 stood globally. By
minute 60 the browsers were gone, but global foliage was 14.0137.

The cone did not read foliage everywhere. At minute 26, when six browsers
remained, mean sector foliage fractions were `0.0000, 0.1111, 0.0926`; at
minute 35 they were `0.0000, 0.0000, 0.0000` for the three remaining bodies.
At minute 45 the two remaining bodies again read zero in all three sectors,
and the bite count stopped in that interval. Thus the cone's failure at intake
stop is an empty/weak directional signal, not canopy saturation. Cropped
crowns did regrow during the horizon: by minute 60 the tracker recorded
regrowth events for bloomcrown 615, umbrellafrond 8,347, springturf 40,
stonecushion 6, velvetpad 3,176 and glowcap 3,368. Food was being replenished
while browsers still died.

Halving the seeded founders reduced the browser deaths from 33 to 22 and left
one browser alive at 60 min; it also reduced browser bites from 18,921 to
14,705 and assimilated organic from 1.3893157 to 0.9817583. Every browser
death in that arm was still starvation, with zero drownings and zero removals.
The boom increases local depletion and loss, but does not explain the access
failure by itself.

Diagnosis: the dominant cause is a mismatch between the browser's 2 m cone
and its 0.0625 m, height-specific mouth geometry. The browser can sense or
pass near foliage and the crowns can regrow, but the movement signal does not
reliably bring a crown into the mouth. Same-height terrain confinement is
not supported by the measured support counts, and the canopy-occlusion
inverse is contradicted at the intake stop.

Proposed, not made: make the browser's movement target use the nearest crown
that is reachable by the actual mouth geometry, and treat a cone hit with no
mouth-reachable crown as a search state rather than edible progress. Evidence
of improvement in the same 60-minute arm would be continued browser bites
after minute 45, a higher than 2/33 death-time mouth-contact fraction, lower
starvation, and no loss of the observed plant regrowth or closed water ledger.
