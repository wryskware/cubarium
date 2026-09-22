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

## D4 — cone at the wall: the shipped landscapes, 2026-09-21

Brief: `design/handoffs/voxel-cone-autopsy-2026-09-21.md`. Read-only. Three
60-simulated-minute arms of `voxel_founder_autopsy` at `2f1cfed`, default
founder counts, the built-in trained centres, run in parallel:

- `voxel_founder_autopsy 60 preset=small` — the Tachyon panel's landscape,
  0.125 m cells, 160×48×24.
- `voxel_founder_autopsy 60 preset=default` — `cubarium voxel` with no TOML,
  0.25 m cells, 128×48×24.
- `voxel_founder_autopsy 60 generated closed` — the baseline arm, unchanged.

A `preset=` arm builds the world the host builds: `Preset::config()`, the
host's own lake gate over `LAKE_SEED_TRIES` draws (deterministic seed stream so
an arm re-runs; both presets took seed 1 with 0 rejections),
the recipe's own water through `World::new`,
`FloraConfig::for_voxel_size` as the host scales the plant layer,
`habitat::seed`'s counts, `install_default_founders`, a settled `Senses`, and
the outlet opened after the layers and before the first tick. The startup
viability probe is skipped: it runs on a clone and only reports.

**The baseline arm is not the ridge the brief expected.** Since 3d80bb4
`VoxelConfig::default().world` *is* `Preset::find("default").config()`, and
`World::new` writes a staged recipe's own water over the config
(`Recipe::cycle_into`), so `generated closed` discards its harness rain and
evaporation and differs from `preset=default` only in
`initial_aquifer_head_m` and in which seed it draws. It seeded 47 stands,
3 logs, 8 litter tiles and produced 15 browser deaths, all starvation, the last
at minute 43.45 — Fable's pre-brief measurement on main at 4b985d8 exactly, and
no longer the reproduction note's 13. Residuals ≤ 1.9e-9 on both ledgers in all
three arms.

### The table

| | preset=small | preset=default | generated closed |
|---|---|---|---|
| cells / ring | 0.125 m / 160×24 | 0.25 m / 128×24 | 0.25 m / 128×24 |
| stands seeded | 30 | 152 | 47 |
| browser deaths (all starvation) | 12 | 24 | 15 |
| extinction minute | none — 1 alive at 60 | 43 | 44 |
| death cohorts (minute: n) | 14:1 22:1 24:1 25:3 27:1 30:2 32:1 51:1 52:1 | 23:1 25:1 26:2 27:1 32:1 33:5 35:3 36:3 38:4 39:2 42:1 | 14:1 16:1 19:1 20:1 23:1 26:1 28:2 29:1 30:2 32:1 35:1 37:1 43:1 |
| foliage standing at the deaths | 2.70–3.47 (mean 2.84) | 6.44–9.31 (mean 7.45) | 0.10–0.62 (mean 0.21) |
| dominant fine first hit at death | clear 0.247 | clear 0.264 | clear 0.422 |
| next two | stripped 0.225, terrain 0.188 | terrain 0.228, stripped 0.194 | stripped 0.220, body 0.178 |
| foliage rays at death | 0.052 | 0.009 | 0.005 |
| nearest living crown, planar | 0.796 m | 1.668 m | 0.668 m |
| its layer against the eye | +4.67 voxels (+0.58 m) | +1.25 voxels (+0.31 m) | −0.90 voxels (−0.23 m) |
| its elevation from the eye (median) | +36.5° | +14.5° | −22.5° |
| inside the fan's ±20° pitch band | 4/12 | 15/24 | 6/15 |
| slant range ≤ 2 m | 12/12 | 13/24 | 15/15 |
| a ray aimed at it reaches foliage | 8/12 | 6/24 | 3/15 |
| **in range AND in band AND unoccluded** | **3/12** | **1/24** | **0/15** |
| wander, metres per browser-minute | 3.42 | 9.69 | 5.87 |
| distinct columns per browser-minute | 6.4 | 7.8 | 3.1 |
| ticks holding \|turn\| > 0.1 | 0.823 | 0.908 | 0.870 |
| legal exits, mean | 2.61 | 2.87 | 2.05 |
| lifetime at death (median) | 93.5 m / 27 columns | 271.4 m / 37 columns | 89.7 m / 8 columns |
| minutes blank in all three sectors | 0.494 | 0.656 | 0.711 |
| bites / assimilated | 29,601 / 1.0186 | 33,287 / 2.4750 | 19,044 / 1.1984 |

### The diagnosis, in one sentence

On the shipped landscapes the browser's cone is not mostly blocked — its
largest single first-hit class at death is `clear` (25–42 %) — and the reason it
reads no foliage is that the nearest living crown is almost never simultaneously
inside the 2 m range, inside the nine rays' ±20° pitch band and unoccluded
(4 of 51 deaths across the three arms), because the cone's geometry is measured
in **voxels** while the crowns are measured in metres.

### What the fine classes added

`Class::Occluder` hid four different failures and they are not the same failure
in the three arms:

- **preset=small (0.125 m).** The flora layer scales its crown heights by the
  cell size, so a crown is the same metres tall, but the eye is fixed at 1.5
  **voxels** — 0.19 m here against 0.375 m on a 0.25 m world. The nearest crown
  at death sits +4.67 voxels over the eye at 0.80 m planar: a median elevation
  of +36.5°, and 8 of 12 of those crowns are **completely unoccluded** — a ray
  aimed at them strikes foliage. The fan simply cannot look up that far. Only
  4/12 are inside the band, and 3/12 are both.
- **preset=default (0.25 m).** The crowns sit near the eye's own height
  (+1.25 voxels, median +14.5°) and 15/24 are inside the band — but 11 of 24 are
  **outside the 2 m range altogether** (median slant 1.95 m), and of the rest the
  commonest thing in the way is a `pool`: the litter/carrion/dead-wood occluder
  over a ground site, 8 of 24 probes. This arm has 152 stands and the richest
  litter layer, and its own dead matter is what stands between a body and the
  crown it is looking at. This class did not exist in the Stage B arena.
- **generated closed.** Here the canopy really has been eaten — 0.12 organic of
  foliage standing at minute 60 — and the remaining crowns are **below** the eye
  (−0.90 voxels, median −22.5°) at 0.65 m median slant, with `stripped` the
  commonest probe answer (8/15) and `body` 0.178 of the fan at death: the bodies
  crowd the last living crowns and occlude each other.

`water` is a real but minor wall: 0.145 of the fan at death on `small`, 0.042 on
`default`, 0.022 on the ridge. The thin-film hypothesis is not the main story on
any shipped landscape.

### Wander: the bodies move, but they do not search

No arm is penned: 2.05–2.87 legal exits, 3.4–9.7 m covered per browser-minute,
and |turn| > 0.1 in 82–91 % of ticks. But the ground **covered** is tiny against
the ground **walked**: a ridge browser that has walked 89.7 m by its death has
stood in 8 distinct columns, and a `small` browser 93.5 m in 27. That is a body
turning on the spot, not one crossing a ring 20 m round. The `default` arm walks
three times as far (271 m) over 37 columns and dies fastest of the three,
which is consistent with turning being the cost rather than the cure. Half to
seven-tenths of every body's minutes were blank in all three sectors before it
died (0.494 / 0.656 / 0.711).

### Proposed, not made — ranked by this evidence

Wrysk's decision, not the worker's.

1. **Scale the cone's vertical geometry to the cell size**, as the flora layer
   already scales crown height: the eye at a fixed height in metres rather than
   1.5 voxels, and `mouth_reach_up_voxels` likewise. This is the only candidate
   the evidence names directly — `small` puts 8 of 12 unoccluded crowns outside
   the band purely because 1.5 voxels is 0.19 m there — and it is the difference
   between two arms of the same animal.
2. **Widen or steer the pitch band.** ±20° with three fixed pitches covers a
   crown between roughly −0.7 m and +0.7 m of rise at 2 m, and almost nothing
   at 0.3 m. A fourth pitch, or a pitch that follows the terrain the body stands
   on, addresses the `small` (+36°) and ridge (−22°) failures together. It
   changes the observation vector, so it retrains.
3. **Train on landscape slices, not the flat arena.** Every class that dominates
   here — terrain from a slope, a ground pool, a stripped crown, another body —
   is a class Stage B never contained, and the policy has never had to tell a
   wall from an empty plain. This is the largest change and the one most likely
   to fix the search failure the wander numbers show.
4. **An occlusion rule for thin water.** Measurable but small on every shipped
   landscape (0.022–0.145 of the fan at death); it would not have saved these
   bodies.

The measurement that is missing: nothing here says whether a crown the fan
*could* see would be **eatable** — D3's mouth-access boundary is untouched by
this package and both failures can be true at once.

## Edible stock, 2026-09-21

Package 0 of the organism audit
(`design/handoffs/voxel-edible-stock-2026-09-21.md`; audit §2 "Seeded and
six-hour edible fraction", §10 order 0). The measurement D4 said was missing —
whether a crown the fan *could* see would be **eatable** — now exists.
Read-only: no controller input, `body.rs` rule, manifest value, `senses.rs`
reading, flora, water or seeding was changed, and
`crates/cubarium/tests/encounter_contract.rs` pins the live 200-tick readings
against the branch point.

Three six-simulated-hour arms of the new
`crates/cubarium/examples/voxel_edible_stock.rs` at `fe654fb`, run in parallel,
one report block every thirty simulated minutes. Each arm is the census's own
preset construction — `Preset::config()`, the host's lake gate (all three took
seed 1 with 0 rejections), `FloraConfig::for_voxel_size`, `habitat::seed`,
`install_default_founders`, a settled `Senses`, the outlet opened before the
first tick. CSVs are disposable (`runs/edible-stock-{small,default,wide}-6h.csv`).
Verified against `voxel_census 0.25 preset=small seed=1`: identical stand
counts per species (4/0/7/5/0/5), identical litter (1.6000) and identical water
residual (6.237e-11) at `t = 0`.

### How each column is computed

A **legal face** is a support face with no more than `wade_depth_m` of standing
water and `headroom_voxels` of void over it — the predicate
`habitat::browser_faces` and `step::faces_in_column` already use. **One** set of
legal faces serves both arms, so the band, the body it is anchored to and the
fan are the only variables.

- **reachable** — the live mouth acceptance rule, `body::mouth_foliage_stand`'s
  own scan, asked of the union of mouth columns over 72 headings from each
  legal face. *Today* is the manifest's whole-voxel layer range; *band* is the
  decided `[0, 1.33 × body height]` with the decided adult body (0.375 m long,
  0.1875 m wide and tall), converted to the layers whose one-cell-thick crown
  slab it overlaps by a positive amount: two layers on `small`, one on
  `default`/`wide`, against today's three and two.
- **visible** — the live ray march from a legal face's eye, aimed at each of the
  stand's crown cells at each pitch of the arm's fan, occlusion order unchanged.
  *Today* is the eye at 1.5 voxels with pitches −20/0/+20; *decided* is the eye
  at `0.8 × body height` = 0.15 m with pitches −40/−20/0/+20/+40 and ground
  pools transparent (decisions §6 makes a pool a wall only above its physical
  height, and no volume-to-height convention is authored).
- **route-connected** — reachable from a legal face in the same walkable
  component as a living browser. The component rule is **not invented**: a
  founder's tick never writes `site.y` and `body::advance_candidate` refuses any
  centre column that is not a support face at the standing layer, so its
  neighbourhood is the level one — four-adjacent columns at its own height, `x`
  wrapping, the `z` ends walls — exactly as `habitat::browser_faces` states and
  as `cubarium-voxel/src/walk.rs` walks the ring. A founder cannot step up or
  down one voxel, ever.
- **route (seeded)** — the same, against the components the **seeded** founders
  stood in. After extinction the living measure is zero by definition; this one
  still describes the landscape.

### The table

| | preset=small | preset=default | preset=wide |
| --- | --- | --- | --- |
| cells / ring | 0.125 m / 160×24 | 0.25 m / 128×24 | 0.25 m / 256×24 |
| stands seeded | 21 | 98 | 197 |
| **t = 0** foliage organic | 4.0348 | 19.3256 | 40.3728 |
| reachable, today's mouth | **1.0000** | **0.9472** | **0.9495** |
| reachable, decided band | 0.9108 | 0.6330 | 0.6187 |
| visible, today's fan | **1.0000** | **1.0000** | **0.9730** |
| visible, decided fan | 1.0000 | 1.0000 | 0.9730 |
| route-connected, today's mouth | **0.2827** | **0.4905** | **0.2412** |
| route-connected, decided band | 0.2679 | 0.3128 | 0.1746 |
| **t = 6 h** foliage organic | 0.4548 | 1.9953 | 3.2376 |
| reachable, today / band | 1.0000 / 1.0000 | 1.0000 / 1.0000 | 1.0000 / 1.0000 |
| visible, today / decided | 1.0000 / 1.0000 | 1.0000 / 1.0000 | 0.9921 / 0.9921 |
| route-connected, today / band | 0.0000 / 0.0000 | 0.0000 / 0.0000 | 0.0000 / 0.0000 |
| route (seeded comp.), today / band | 0.0088 / 0.0000 | 0.2927 / 0.1725 | 0.0448 / 0.0107 |
| browsers alive, 0 → 6 h | 8 → 0 (gone by min 30) | 8 → 0 (2 at 60, 0 at 90) | 8 → 0 (1 at 90, 0 at 120) |
| shredders alive, 0 → 6 h | 8 → 0 (gone by min 60) | 8 → 0 (1 at 60, 0 at 90) | 8 → **5** |
| species surviving at 6 h | stonecushion | stonecushion | stonecushion, +traces |

Per species at `t = 0`, reachable today / reachable band / route today:

| species | small | default | wide |
| --- | --- | --- | --- |
| bloomcrown | 1.00 / 0.86 / **0.00** | 0.92 / **0.43** / 0.35 | 0.91 / **0.41** / **0.05** |
| umbrellafrond | — | — | 1.00 / **0.37** / **0.00** |
| springturf | 1.00 / 1.00 / 0.86 | 1.00 / 1.00 / 0.95 | 1.00 / 1.00 / 0.57 |
| stonecushion | 1.00 / 1.00 / 0.37 | 1.00 / 1.00 / 0.43 | 1.00 / 1.00 / 0.40 |
| velvetpad | — | 1.00 / 1.00 / 1.00 | 1.00 / 1.00 / 0.73 |
| glowcap | 1.00 / 1.00 / 1.00 | 1.00 / 1.00 / 0.82 | 1.00 / 1.00 / 0.89 |

Detritus, organic and the share in a component holding a living shredder:

| | small | default | wide |
| --- | --- | --- | --- |
| litter, t = 0 | 1.6000 / 1.00 | 1.6000 / 1.00 | 1.6000 / 1.00 |
| litter, 6 h | 0.1531 / 0.00 | 0.6917 / 0.00 | 0.9485 / 0.30 |
| glowcap cap, t = 0 | 0.5640 / **0.70** | 1.3820 / **0.22** | 2.8800 / **0.11** |
| carrion, t = 0 and 6 h | 0 | 0 | ~1e-6 |

Residuals at 6 h: fauna ≤ 1.8e-10 on all three currencies in every arm; flora
organic ≤ 2.7e-8 and energy ≤ 5.3e-8 (relative to stocks of 0.5–40, ≤ 1e-9
relative); water ≤ 2.1e-6 m³. These are accumulated floating-point drift over
432,000 ticks, not a leak: at `t = 0` every residual is ≤ 1.4e-14 except water
at ≤ 3.9e-9.

### One sentence per preset

- **small.** Every unit of standing foliage is inside today's mouth from some
  legal face and visible to today's fan from some legal face, yet only 28 % of
  it shares a level walkable component with a browser and the four bloomcrowns
  holding 63 % of the foliage share **none** — the broken link on `small` is the
  route, not the mouth and not the eye.
- **default.** The route is again the tightest live link (49 % at seeding,
  dropping to 29 % of the surviving stock by six hours), but this is the preset
  where the *decided* band would become the binding one: it cuts reachable stock
  from 95 % to 63 %, because bloomcrown carries 65 % of the foliage and its
  crown sits one voxel above a 0.25 m ceiling that today's 0.5 m acceptance slab
  clears.
- **wide.** The worst route of the three — 24 % at seeding and 4 % of what
  survives, with bloomcrown at 5 % and umbrellafrond at 0 % — so twice the ring
  with the same eight founders is not more habitat but more terraces the
  founders cannot leave; it is also the only arm where a lineage persists
  (5 shredders at six hours), on 30 % of the litter.

### What the numbers decide, and what they do not

1. **"Low food is unreachable or unseen" is refused, in the geometric sense.**
   At seeding, 95–100 % of the foliage organic is inside today's mouth from some
   legal face and 97–100 % is visible to today's fan from some legal face. The
   *reachable from somewhere* and *visible from somewhere* numbers are upper
   bounds over every legal face and heading; D4's numbers are the same question
   asked at the body's actual pose, and both can be true — the food is
   geometrically available and the body is not at it.
2. **"Not route-connected" is the link that is broken at seeding**, on every
   preset: 24–49 %. The cause is named and structural, not a tuning: a founder
   cannot change its standing layer, so its world is one terrace.
3. **The eight shredders are seeded at eight different standing heights** on all
   three presets (`small` 21/23/24/26/31/36/41/43), so each one begins alone in
   its own level component — which is why the glowcap caps that decisions §3
   makes their food are 11–70 % out of reach before anything has moved.
4. **Adopting decisions §2's band as written would take stock away**, not add
   it: 1.33 × 0.1875 m = 0.249 m is *tighter* than today's converted acceptance
   slab (0.375 m on `small`, 0.5 m on `default`/`wide`), and the loss lands
   almost entirely on bloomcrown and umbrellafrond. Whatever else the band
   fixes, it does not fix access.
5. **The decided fan changes nothing measurable here** (identical to today's on
   every arm and every snapshot), because with the yaw free and any legal face
   allowed, three pitches already reach what five do. Its value is at the body's
   actual pose, which this observer does not measure.

Not measurable with today's model, and why: the **share** of a stand's foliage
inside a band (the model holds one cell-thick slab and one undivided `foliage`
stock, so reach is all-or-nothing per stand — decisions §4, audit §10 order 2);
a **juvenile's** band, eye and footprint (no body height exists and the manifest
is static, so both arms use the adult — decisions §1); a ground pool as a
**partial** occluder (no volume-to-height convention); the shredder's decided
three-food diet (unimplemented, so the detritus block reports access, not
intake — decisions §3); **path length** through a component (planar distances
only); and decisions §8's acceptance gate itself (`N` founder-hours is a backlog
placeholder, though the t = 0 route fraction is the number such a gate reads).
