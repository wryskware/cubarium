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

## Step rule, 2026-09-22

Package 1a (`design/handoffs/voxel-founder-step-2026-09-22.md`), measured on
branch `founder-step` at 75007b0 (from main at 1d81f3a). "Before" is that
branch point; "after" is the same binaries with the step rule in. Founders now
have a climb height in metres on their physiology — browser 0.25 m, shredder
0.125 m, both placeholders in `design/backlog.md` §1 — a sub-step may change the
standing layer by up to that many whole voxels, contact reports only solids
above it, and route connectivity everywhere is one function
(`cubarium_voxel::walk::components`) with that one rule.

### t = 0 route, `voxel_edible_stock 0 preset=…`

Fractions of total standing foliage; same seed (1), same landforms, same flora
in both arms. `reach_today` is unchanged by this package and is printed as the
ceiling the route is now measured against.

| t = 0 | small | default | wide |
| --- | --- | --- | --- |
| foliage reachable today (unchanged) | 0.4944 | 0.7181 | 0.7494 |
| **route-connected, before** | 0.3994 | 0.2472 | 0.3524 |
| **route-connected, after** | **0.4944** | **0.4841** | **0.3646** |
| route-connected under the decided band, before | 0.3994 | 0.2345 | 0.2333 |
| route-connected under the decided band, after | 0.4174 | 0.3921 | 0.3353 |
| bloomcrown route-connected, before | 0.0000 | 0.0000 | 0.2137 |
| bloomcrown route-connected, after | 0.1322 | 0.5000 | 0.2058 |
| glowcap cap in a shredder's component, before | 0.4071 | 0.0000 | 0.4492 |
| glowcap cap in a shredder's component, after | 1.0000 | 0.7390 | 1.0000 |

Three things the table says. **On `small` and `default` the route stopped being
the binding link**: after the rule, `route_today` equals `reach_today` to the
digit on `small` (0.4944) and closes most of the gap on `default` (0.4841
against 0.7181). What is left out on `small` is out of reach, not out of walk.
**The shredders' detritus is no longer stranded**: eight founders seeded on
seven or eight distinct heights now share components covering all or most of the
litter and glowcap cap. **`wide` barely moved** (0.3524 → 0.3646) and its
bloomcrown share fell slightly (0.2137 → 0.2058); on that preset the seeder's
own choice of faces changed with the walk — one browser founder moved from layer
35 to 43 — and 0.25 m of climb against a 256-column ring of deeper relief joins
less than it does on the two smaller presets. `wide` is still route-limited.

### 60 minutes, `voxel_founder_autopsy 60 preset=…`

| 60 min | small before | small after | default before | default after |
| --- | --- | --- | --- | --- |
| shredder starved / drowned | 13 / 2 | 12 / 1 | 14 / 1 | 23 / 1 |
| browser starved / drowned | 8 / 2 | 10 / 4 | 16 / 1 | 11 / 3 |
| removed (no support face) | 0 | 0 | 0 | 0 |
| **browser extinct at minute** | **33** | **39** | **44** | **never (2 alive)** |
| shredders alive at 60 min | 1 | 9 | 4 | 8 |
| shredder bites | 11,903 | 19,385 | 24,779 | 41,057 |
| browser bites | 12,247 | 21,340 | 27,480 | 21,404 |
| born (of 36 / 48 births) | 10 | 20 | 20 | 32 |
| gestations failed | 3 | 0 | 3 | 4 |

The browser line on `default` is the result that matters: the lineage that went
extinct at minute 44 now still has two bodies at the hour, off 21 % fewer bites
but spread over a component that holds twice the foliage. On `small` the
extinction moved six minutes later and did not stop; the browser's remaining
problem there is reach, which is package 1b's and the layers package's, not the
route's. Both shredder arms roughly doubled their intake and their survivors —
the litter and glowcap they were seeded next to but could not walk to is now
theirs. Nothing was removed for standing on air in either arm, which is the
check that the rule never puts a body on a face that is not a support face.
Residuals over 72,000 ticks: fauna ≤ 4.6e-11, flora ≤ 1.2e-10 absolute.

### What the trained centres did with the changed contact channel

`crates/cubarium/tests/encounter_contract.rs` runs 200 ticks of the shipped
authored habitat with the built-in centres and used to pin the trajectory to the
bit. It moved, and by more than the seeding did:

| 200 ticks, authored habitat | before | after |
| --- | --- | --- |
| standing layers, blind founders | 12 17 22 25 28 36 **39 45** | 12 17 22 25 28 36 **38 46** |
| standing layers, browsers | **13** 19 20 20 **20 22 33** 48 | **12** 19 20 20 **23 32 36** 48 |
| Σ pose x | 211.034 | 220.751 |
| Σ pose z | 45.608 | 54.821 |
| Σ heading | 60.242 | 50.742 |
| eaten organic | 0.16375 | 0.10860 |
| Σ browser cone reading | 36.853 | 28.603 |

The blind founders' seeding is untouched by this package (they are placed from
the litter pool, not from `browser_faces`), so their two changed layers are pure
step-rule evidence: two of eight left the layer they were seeded on inside ten
seconds. The browsers' larger change mixes the step with a seeding set the new
walk enlarged. The centres were trained against a contact channel that called a
steppable ledge a wall and now reads it as open ground; over ten seconds that
shows up as bodies travelling further (Σ pose x and z both up) and turning less,
and eating a third less in that window — which the hour-long arms above say is a
transient of the first seconds and not the steady state. Nothing refused to load
and no centre was retrained: the observation vector is the same 23 and 37 inputs
it was.

## Bodies in metres, 2026-09-22

Package 1b (`design/handoffs/voxel-body-anchors-2026-09-22.md`) on the step
rule's world: adult dimensions in metres on the founder physiology, growth by
`(body / body_max)^(1/3)`, the eye at `0.8 × height` over the standing surface,
the mouth band `[0, 1.33 × height]`, the horizontal reach `0.25 × length`, the
clearance `ceil(height / voxel)`, and canopy shade over a physical crown area in
m². Measured on this branch (`body-anchors`, from main at f1f3f1f), same command
as the step rule's section, same seeds. "Before" is f1f3f1f re-run here, not a
quoted number.

### t = 0, `voxel_edible_stock 0 preset=<name>`

| fraction of standing foliage | small | default | wide |
| --- | --- | --- | --- |
| reachable from any legal face, before → after | 0.494 → **0.417** | 0.718 → **0.500** | 0.749 → **0.486** |
| route-connected to a seeded browser, before → after | 0.494 → **0.417** | 0.484 → **0.223** | 0.365 → **0.366** |
| visible, before → after | 1.000 → 1.000 | 0.691 → 0.691 | 0.916 → 0.916 |
| bloomcrown reachable, after | 0.000 | 0.534 | 0.334 |
| bloomcrown route-connected, after | 0.000 | 0.000 | 0.088 |

**Reach falls everywhere, and it is the band doing it.** The adult browser's
ceiling is `1.33 × 0.1875 = 0.249375` m — a hair under one 0.25 m cell — so on
`default` and `wide` the mouth takes the standing layer and nothing above it,
where the old whole-voxel rule took one cell more. On `small` two 0.125 m cells
are inside the band, which is the same 0.25 m of air. All three "after" reach
numbers are **bit-identical to the `reach_band` column the observer printed
before this package** (0.417351, 0.500168, 0.486236): the implementation
reproduces the hypothesis the observer was measuring, which makes this row a
check rather than a result.
Clearance moved the other way (the browser now asks for `ceil(0.1875/v)` = 1 cell
on `default`/`wide` against `1 + mouth_reach_up_voxels` = 2, and 2 on `small`
against 3), so there are more legal faces than before; it did not offset the
band.

**Route falls on `default` because the seeder moved, not because the walk did.**
`browser_faces` places founders on faces whose crown is in the mouth band, and
the narrower band deleted the high feeding faces: the eight browsers were seeded
across standing layers 17/18/19/35/36 and are now all at 17 and 18. The foliage
on the upper terraces is still reachable (0.500) and is no longer anybody's
component (0.223). `small` and `wide` did not move: on `small` route has equalled
reach since the step rule, and on `wide` the seeding barely changed. Nothing here
is a route regression — the step rule's components are unchanged — it is where
the founders are put.

### 60 simulated minutes, `voxel_founder_autopsy 60 preset=<name>`

| | small before | small after | default before | default after |
| --- | --- | --- | --- | --- |
| deaths (accounted) | 27 | 25 | 38 | 39 |
| starved (shredder / browser) | 22 (12 / 10) | 25 (13 / 12) | 34 (23 / 11) | 35 (22 / 13) |
| drowned | 5 | **0** | 4 | 4 |
| **browser extinct at minute** | 39 | **30** | never (2 alive) | **never (1 alive)** |
| shredders alive at 60 min | 9 | 7 | 8 | 8 |
| browser bites | 21,340 | 10,412 | 21,404 | 16,621 |
| shredder bites | 19,385 | 16,619 | 41,057 | 49,352 |
| born (of 32 / 48 births) | 20 | 16 | 32 | 32 |

Browser intake falls with the band, as the t = 0 table says it must: half the
bites on `small`, 22 % fewer on `default`, and the `small` extinction moved nine
minutes earlier. What the browsers ate changed shape as well as size — on
`default` bloomcrown went from 4,283 bites to **none at all** (its one-cell crown
sits above the band and the seeder no longer stands anybody under a high one),
and stonecushion, a floor tissue, went from 1,812 to 4,771. On `small`
bloomcrown went the other way, 0 → 658, because a browser can now reach the
0.125–0.25 m cell a bloomcrown seedling's crown sits in. This is the succession
story decisions §2 asked for, and the answer to the hungry browser is the layers
package's basal rosette, not a raised ceiling. Drowning on `small` went from five to zero. That is not
isolated: the seeding, the clearance and every trajectory moved together in this
arm, and a five-to-zero count over one seed is a small sample. It is recorded,
not explained. Residuals over 72,000
ticks: fauna ≤ 1.1e-10, flora ≤ 6.4e-10 absolute.

### Shade

`shade_k` (per cell²) became `shade_k_per_m2` = `1.5 × (0.25 m)²` = **0.09375**,
and a crown's area became `π (radius_cells · voxel_m)²` floored at `(0.25 m)²`
instead of `π radius_cells²` floored at 1. On the 0.25 m reference grid both
substitutions cancel exactly — coefficient and area each pick up one factor of
`(0.25)²`, floor included — so `default` and `wide` are numerically identical,
which `crates/cubarium-voxel-flora/tests/shade_area.rs` asserts against the old
expression digit for digit. On `small` the same physical crown now casts the same
physical shade as on `default`; before, halving the cell quartered the optical
depth. No stock number in the tables above is attributable to it: the t = 0
observer does not step, and the two autopsy arms are 0.125 m and 0.25 m worlds
whose light gates move only over hours.

## Layers, 2026-09-22

Package 2 (`design/handoffs/voxel-plant-layers-2026-09-22.md`) on the
bodies-in-metres world: a species carries a `profile` staged by `wood / wood_max`,
a stand holds one stock per foliage-bearing layer of that stage summing to its
scalar `foliage`, bites come out of the layers the mouth's band reaches lowest
first, regrowth fills bottom-up and senescence sheds from the top, and light is
assessed per layer. The profiles are `design/organism-anatomy-2026-09-21.md` §3's
tables with decisions §5's corrections — the adult bloomcrown keeps a **0.25
basal rosette for life**, a woody seedling is a ground rosette capped at 0.125 m.
Measured on branch `plant-layers`, from main at 1447efd.

**"Before" is a control arm of this same build, not a quoted number.**
`voxel_edible_stock ... lollipop` and `voxel_founder_autopsy ... lollipop` run
`FloraConfig::one_layer_species()` — every plant the one disc it was — on the
same landforms, the same seed, the same bodies and the same trained centres. At
`t = 0` it reproduces the "Bodies in metres" table above **exactly** on all three
presets, which is what makes the differences below attributable to the anatomy
and to nothing else. Over a stepped run it is statistically, not bitwise, the old
model: a withdrawal now sums per-layer takes where it used to compute
`want.min(foliage)`, and an ulp in a bite is a different trajectory an hour later.

### t = 0, `voxel_edible_stock 0 preset=<name>`

| fraction of standing foliage | small | default | wide |
| --- | --- | --- | --- |
| standing foliage organic | 4.6752 | 20.2068 | 46.6440 |
| reachable from any legal face, before → after | 0.417 → **0.591** | 0.500 → **0.561** | 0.486 → **0.595** |
| route-connected to a seeded browser, before → after | 0.417 → **0.591** | 0.223 → 0.223 | 0.366 → **0.388** |
| visible, before → after | 1.000 → 1.000 | 0.691 → 0.703 | 0.916 → 0.892 |
| bloomcrown reachable, before → after | 0.000 → **1.000** | 0.534 → **0.664** | 0.334 → **0.536** |
| umbrellafrond reachable, before → after | — | 0.000 → 0.000 | 0.039 → **0.087** |

**All of the gain is the new low tissue, and the per-layer block says so.** The
observer now reports each species' layers separately (`layer,<min>,<species>,
<index>,<stands>,<band_lo_m>,<band_hi_m>,<stock>,<reach>,<route>,<f_reach>`),
with the band stock-weighted in metres above the world floor:

| preset | species | layer | band (m) | stock | reachable |
| --- | --- | --- | --- | --- | --- |
| small | bloomcrown | 0 rosette | 3.62–3.73 | 0.811 | **1.00** |
| small | bloomcrown | 1 crown | 3.94–4.21 | 1.913 | 0.00 |
| default | bloomcrown | 0 rosette | 9.74–9.84 | 2.828 | **1.00** |
| default | bloomcrown | 1 crown | 10.06–10.32 | 6.628 | 0.52 |
| default | umbrellafrond | 0 lowest tier | 4.24–4.38 | 2.309 | 0.00 |
| default | umbrellafrond | 1 | 4.51–4.65 | 1.804 | 0.00 |
| default | umbrellafrond | 2 top | 4.83–4.93 | 0.795 | 0.00 |
| wide | bloomcrown | 0 rosette | 8.95–9.05 | 6.834 | **1.00** |
| wide | bloomcrown | 1 crown | 9.29–9.54 | 15.966 | 0.34 |
| wide | umbrellafrond | 0 lowest tier | 4.48–4.62 | 4.171 | 0.14 |
| wide | stonecushion | 0 skirt · 1 cap | 4.62–4.74 | 2.864 · 1.228 | 1.00 · 1.00 |

- **small.** Every bloomcrown's rosette is floor food, where before *no*
  bloomcrown foliage was reachable at all: reach 0.417 → 0.591 and route with it,
  because on this preset every legal face is one component.
- **default.** Reach rises 0.500 → 0.561 and the route does not move at all
  (0.223), because the seeded browsers' walkable component holds no bloomcrown —
  the seeding problem package 1b flagged and package 4 owns. A rosette a browser
  cannot walk to is not food.
- **wide.** Reach 0.486 → 0.595 and route 0.366 → 0.388: bloomcrown's rosette is
  wholly reachable and a third of its crown still is, because on a 24-high ring
  many faces stand a voxel above a neighbouring stand.

Umbrellafrond's adult and juvenile tiers stay out of reach on every preset, which
is decisions §5's "escape at the seedling → juvenile transition" doing exactly
what it says. `visible` moved a little in both directions: trunks are occluders
for the first time, and a rosette is a second thing to see.

### 60 simulated minutes, `voxel_founder_autopsy 60 preset=<name>`

| | small before | small after | default before | default after |
| --- | --- | --- | --- | --- |
| deaths (accounted) | 27 | 30 | 41 | 36 |
| starved (shredder / browser) | 27 (15 / 12) | 24 (13 / 11) | 38 (25 / 13) | 34 (23 / 11) |
| drowned | 0 | 6 | 3 | 2 |
| **browser extinct at minute** | **30** | **53** | never (1 alive) | never (**2** alive) |
| shredders alive at 60 min | 6 | 3 | 9 | **13** |
| browser bites | 10,495 | 12,280 | 14,135 | 17,526 |
| browser assimilated | 0.743 | **1.252** | 1.566 | 1.352 |
| born (of 33 / 51 births) | 17 | 17 | 35 | 35 |
| bloomcrown bites / eaten | 640 / 0.095 | **3,380 / 1.294** | 0 / 0 | 0 / 0 |
| umbrellafrond bites / eaten | 0 / 0 | 0 / 0 | 2,269 / 0.623 | **7,417 / 0.876** |
| springturf bites / eaten | 1,585 / 0.732 | 1,860 / 0.789 | 4,190 / 1.729 | 2,897 / 1.228 |

**On `small` the rosette is the difference: the browser lineage lasts 53 minutes
instead of 30, on 1.25 of assimilated intake against 0.74, and bloomcrown goes
from 640 bites to 3,380.** That is the grazed meadow decisions §5 asked for,
measured. Two things moved the other way in the same arm and are recorded rather
than explained: six browsers drowned where none did before, and the shredders
ended at 3 instead of 6. Both are one seed and one trajectory — a browser that
lives twenty minutes longer walks somewhere else and leaves a different corpse —
and neither is a rule this package changed.

**On `default` the rosette changes nothing, because no browser can walk to one.**
What the browsers ate instead is umbrellafrond, three times over: its
**seedlings** are ground rosettes now, and a seedling frond is food where an
adult is not. Bloomcrown stays at zero bites on both arms. The lineage still
holds at 60 minutes and the shredder population is larger (13 against 9), which
follows the extra litter. This is the audit's §5 warning becoming visible:
"universal seedling browsing can instead prevent canopy recruitment", and it is
now a measurable pressure rather than a possibility.

Residuals over 72,000 ticks: fauna ≤ 6.2e-11, flora ≤ 1.6e-10 absolute on both
arms.

### 6 simulated hours, `voxel_edible_stock 6 preset=<name>`

| | small before | small after | default before | default after |
| --- | --- | --- | --- | --- |
| standing foliage, t = 0 → 6 h | 4.675 → **0.000** | 4.675 → **0.264** | 20.207 → 29.800 | 20.207 → **12.807** |
| species alive at 6 h | none | stonecushion | umbrellafrond 28.19, stonecushion 1.61 | umbrellafrond 11.27, stonecushion 1.54 |
| reachable at 6 h | — | 1.000 | 0.067 | **0.414** |
| browsers alive, 0 → 6 h | 8 → 0 (gone by min 30) | 8 → 0 (gone by min 60) | 8 → 0 (gone by min 90) | 8 → 0 (**1 alive at min 180**, gone by 210) |
| shredders alive, 0 → 6 h | 8 → 0 (gone by min 120) | 8 → 0 (gone by min 90) | 8 → **10** | 8 → **10** |

- **small.** The plant layer collapses on both arms — it did before this package
  and it does now — but the layered world ends holding 0.264 of stonecushion
  where the lollipop world ends at exactly zero, and all of it is floor food.
  Neither lineage survives six hours on this preset under either model. The
  collapse is not the layers' doing and the layers do not fix it.
- **default.** The forest regrows either way and it is umbrellafrond that regrows
  it. **The layered world ends with less than half the standing foliage of the
  lollipop one (12.8 against 29.8) and four times the share of it reachable
  (0.414 against 0.067)**, because the regenerating fronds are seedlings and a
  seedling is a ground rosette. The browser lineage lasts twice as long
  (min 180 against min 90) and still dies. The shredders are unaffected at 10.

The honest sentence for both presets: **layers put real food on the floor and the
browsers ate it for twice as long, and it was not enough.** Nothing here says the
lineage persists; it says the tissue it needs now exists and is reachable, and
that what is still missing on `default` is the route (package 4) and on `small`
the plant layer's own six-hour collapse, which predates this package.

Residuals at 6 h, over 432,000 ticks, against the stocks the observer now prints
beside them: `small` flora organic −1.4e-10 of 1.423, mineral −4.4e-11 of 260.3,
energy −2.7e-10 of 2.846; `default` flora organic −1.3e-8 of 42.11, mineral
−6.9e-11 of 384.8, energy −2.6e-8 of 84.21. Every one is ≤ 1e-9 **relative**
(worst 3.1e-10, `default` energy). Fauna ≤ 1.5e-11 absolute on both. Water
1.6e-7 m³ (`small`) and 2.7e-6 m³ (`default`). The 6 h `after` arms were run
twice, on two builds differing only by an added print, and reproduced digit for
digit.

### What the layers did not settle

**Porosity has no sight meaning and no meaning in the picture.** It enters the
light exponent as `(1 - p)` and nothing else reads it, so a porous canopy is
transparent to plant light and opaque to an eye and solid in the presenter. The
audit's §5 asked for one shared interpretation; this package records the
simplification instead of inventing one, because resolving it is a model decision
(`design/voxel-encounter-contract-2026-09-21.md` §8).

And that `(1 - p)` is the **one** way a single-layer species' shade differs from
what it was: springturf, velvetpad and glowcap now shade by `(1-p)` of their old
optical depth, with p 0.3, 0.6 and 0.5. Everything else about the shade of a
one-layer plant is the pre-layer number digit for digit — the reference height,
the footprint, the area in m², the occlusion order and the weight — which
`crates/cubarium-voxel-flora/tests/layers.rs` asserts in both forms. The three
are mats and caps whose band top sits at the ground, so they occlude almost
nothing on these landscapes, and no number in the tables above is attributable to
it; but it is a change, and it is a one-line change to undo.

## D5 — the small preset

Brief: `design/handoffs/voxel-small-collapse-2026-09-22.md`. Read-only; the only
code change is `crates/cubarium/examples/voxel_plant_autopsy.rs`, which now takes
`preset=<name>` through `voxel::ambient_world` (the host's own build: the recipe's
extents, cell size and water, the lake gate over 24 draws, `FloraConfig::for_voxel_size`)
and runs either `plants-only` (the fauna the seeder introduces is never stepped) or
`coupled` (the host's `Sim`, the settled sense field, the built-in trained founders).
Three arms, six simulated hours, seed base 1, 0 seeds rejected on either preset:
`small` plants-only, `small` coupled, `default` plants-only. CSVs under `runs/` are
disposable.

Both arms reproduce the "Layers" table at t = 0 exactly — standing foliage 4.675 on
`small`, 20.207 on `default` — and the `small` **coupled** arm ends at **0.264**, the
same 0.264 that section reports for `voxel_edible_stock 6 preset=small`. The plant
autopsy is measuring the same collapse.

### The table

| | small plants-only | small coupled | default plants-only |
| --- | --- | --- | --- |
| species seeded (stands) | 4 (27) | 4 (27) | **6 (87)** |
| standing foliage, t = 0 → 6 h | 4.675 → **0.664** | 4.675 → **0.264** | 20.207 → **25.754** |
| alive at 6 h | stonecushion 0.664 | stonecushion 0.264 | umbrellafrond 23.36, stonecushion 1.48, springturf 0.84, velvetpad 0.07, glowcap 0.02 |
| establishments / deaths | 33 / 29 | 12 / 29 | **215 / 141** |

Deaths by cause. The flora ledger books `deaths` with **no cause**, so every cause
below except drowning is inferred from the clause of the stand's own survival rule
that was failing at the tick it died; drowning is read straight off the gate.

| species | small plants-only | small coupled | default plants-only |
| --- | --- | --- | --- |
| bloomcrown | 9 deficit | 7 deficit, 2 no foliage | 18 deficit |
| umbrellafrond | — | — | 5 drown, 2 thirst, 2 deficit |
| springturf | 6 drown, 6 deficit | 6 drown, 6 no foliage | 1 drown, 1 thirst, 88 deficit |
| stonecushion | 0 | 0 | 1 deficit |
| velvetpad | — | — | 12 thirst, 3 deficit |
| glowcap | 8 drown | 8 drown | 2 drown, 6 at the alive floor while solvent |

Root-box pore against the band, mean over living stands, and the light beside it.
Every species' `establish_pore_min` and wilt/saturation band is in the runs' t = 0
block; soil's field capacity is 0.25 and rock's 0.5, so 0.25 is "drained soil".

| | t = 0 | 30 min | 6 h | band (wilt → sat) | mean light, whole run |
| --- | --- | --- | --- | --- | --- |
| small bloomcrown | 0.2500 | 0.165 (μ 0.20) | dead at 330 min | 0.08 → 0.50 | 0.98–1.00 |
| small springturf | 0.2500 | 0.242 (μ 0.31) | dead at 57 min | 0.15 → 0.45 | 0.94–0.96 |
| small stonecushion | 0.2500 | 0.172 (μ 0.46) | **0.071 (μ 0.16)** | 0.02 → 0.35 | 0.95–0.97 |
| default umbrellafrond | **0.9853** | 0.647 (μ 0.70) | **0.506 (μ 0.43)** | 0.30 → 0.80 | 0.96–0.97 |
| default velvetpad | **0.6478** | 0.431 (μ 0.58) | 0.204 (μ 0.03) | 0.20 → 0.60 | 0.99 |
| default bloomcrown | 0.2500 | 0.123 (μ 0.11) | dead | 0.08 → 0.50 | 0.87–0.89 |
| default springturf | 0.2599 | 0.254 (μ 0.30) | 0.375 (μ 0.65) | 0.15 → 0.45 | 0.89–0.96 |

**Light is never the constraint**: the mean light response is 0.87–1.00 for every
species in every arm, and no death was booked on light or on mineral.

The water ledger, per square metre of footprint, so the two rings are comparable
(`small` 60 m² and 90 m³; `default` 192 m² and 288 m³ — both exactly
`inventory_m` 1.5 m, since `Water::SMALL` inherits `Water::DEFAULT`'s inventory).

| m³ per m² | small t = 0 | small 6 h | default t = 0 | default 6 h |
| --- | --- | --- | --- | --- |
| pore | 0.0447 | 0.0481 | 0.0681 | 0.0611 |
| pooled | 0.0366 | 0.0084 | **0.1298** | 0.0131 |
| aquifer (head, m) | 0.574 (1.640) | 0.606 (1.731) | 0.378 (**1.081**) | 0.553 (1.579) |
| atmosphere | 0.845 | 0.837 | 0.924 | 0.873 |
| rain, cumulative | — | 0.0688 | — | 0.0688 |
| stream re-entry, cumulative | — | **0.0433** | — | **0.1127** |
| evaporation, cumulative | — | 0.0986 | — | 0.0969 |
| transpiration, cumulative | — | 0.0014 | — | 0.0216 |

33 showers on both, and the rain each ring received per square metre is identical to
five digits, so the per-preset `shower_volume_m3` scaling works. Residuals over
432,000 ticks: `small` 1.5e-7 m³ of 90 (1.7e-9 relative), `default` 2.6e-6 of 288
(9.1e-9 relative).

Establishment at t = 0. **Every seeded stand passes its own predicate where it
stands, on both presets** — `habitat::suitable` consults
`establishment_gates_with_sky` now, so D1's "62 of 62 founders stand where their own
seeds are refused" no longer holds. What differs is which species get planted at all,
and the skyline says why:

| eligible columns at t = 0 | small (of 3,840) | default (of 3,072) | seeded |
| --- | --- | --- | --- |
| bloomcrown | 3,046 (79 %) | 1,163 (38 %) | 4 / 15 |
| **umbrellafrond** (pore ≥ 0.45) | **16 (0.4 %)** | **853 (27.8 %)** | **0 / 8** |
| springturf | 2,709 (71 %) | 962 (31 %) | 12 / 36 |
| stonecushion | 3,104 (81 %) | 1,226 (40 %) | 5 / 15 |
| **velvetpad** (pore ≥ 0.30) | **7 (0.2 %)** | **157 (5.1 %)** | **0 / 4** |
| glowcap (dead wood) | 201 (5.2 %) | 90 (2.9 %) | 6 / 9 |

The seeder's own arithmetic closes it: `want = round(eligible_columns × cell_area ×
per_m²)`. On `small` that is `round(16 × 0.015625 × 0.15) = round(0.0375) = 0` for
umbrellafrond and `round(7 × 0.015625 × 0.4) = round(0.04) = 0` for velvetpad; on
`default` it is `round(853 × 0.0625 × 0.15) = 8` and `round(157 × 0.0625 × 0.4) = 4`,
which is exactly what was planted.

### The sentence

**The `small` preset's plants die of thirst in drained soil, and what makes that fatal
rather than merely thinning is that its ring has no standing wet ground for the seeder
to find, so the only two producers that are solvent anywhere — umbrellafrond and
velvetpad, income 6–18× maintenance in soil above field capacity — are never planted,
and every species `small` does get is one that is insolvent on `default` too.**

The supporting readings: bloomcrown covers 0.73 of its maintenance on `default` and
1.38 on `small` at 30 minutes and falls to 0.00 on both; springturf covers 0.63 and
0.49 at 30 minutes, is gone entirely by minute 60 on `small`, and loses 88 of its 90
`default` stands to the same deficit by the same minute (16 come back later, in the
wet ground the surviving canopy stands in). On `default` umbrellafrond covers 12.3–17.9
throughout and grows 8 founders into 80 stands and 23.4 of foliage; velvetpad covers
6.4–15.1 for the first two hours and then dries out too (12 of its 15 deaths are
thirst, and it ends at 0.065). `small`'s one solvent species is stonecushion, a ground mat that ends at
0.664 ungrazed and 0.264 grazed. Consumption is not the cause: the plants-only arm
reproduces the coupled arm species for species and 29 deaths for 29, including the
same 6 drowned springturf and 8 drowned glowcap; grazing only decides how much of the
surviving mat is left.

### Candidates, proposed and not made, ranked by the evidence

1. **The inventory's split, not its size.** `Water::SMALL` keeps
   `Water::DEFAULT`'s 1.5 m, and per square metre both rings are charged the same.
   Where it lands differs: `small` buries 38 % of it in the water table against
   `default`'s 25 %, because `hydrate` lifts the table to the lake floor and `small`'s
   lake floor stands at 1.625 m where `default`'s stands at 1.0 m — 0.35 × 1.625 =
   0.57 m per m² against 0.35 — and `small` is left with 2.4 % of its water standing in
   pools against `default`'s 8.7 % (a 7.2 m² lake on a 60 m² ring, 12 %, against 66 m²
   on 192 m², 34 %). The wet columns are the ones near standing water, and that is the
   whole difference in the eligibility table. The levers this points at are
   `lake_depth_m` (0.375 on `small`, 0.75 on `default`), the terraced tier-0 trough the
   lake sits in, and the inventory sized against a higher lake floor — not the 1.5 m.
2. **The seeder looks at the driest moment and rounds a small niche to zero.** It runs
   after the settle and the 200-tick stream watch, before the first shower (300–900 s),
   and thirty minutes of weather takes `small` from 16 to 224 umbrellafrond-eligible
   columns and from 7 to 267 velvetpad-eligible ones — 14× and 38× what the seeder saw,
   and at 6 h 278 and 334. But establishment needs a donor of that species and there is
   none, so the ring can never acquire them. Candidates: a floor of one founder wherever
   the niche exists at all, or judging the ring after a shower rather than before one.
3. **The solver's conductivity scales with the cell.** Infiltration and drainage are
   `permeability_per_s · dt · pore_capacity · voxel_volume` per voxel
   (`water.rs:757, 788, 1569`), which is `permeability · pore_capacity · voxel_m` metres
   of column per second — so at 0.125 m the `small` ring's soil takes water and drains it
   at **half** `default`'s speed in metres. Consistent with what the first shower did:
   at minute 10 the runoff stood 0.031–0.077 m over `small`'s low ground and drowned
   **14 of its 27 stands** (every springturf and every glowcap, against drown depths of
   0.03 and 0.05 m), while `default`'s first shower drowned 3 of 87 at minute 9. Candidate:
   a length per second rather than a cell fraction per second. **Not measured** — see below.
4. **Not `FloraConfig::for_voxel_size`.** It doubles every authored voxel distance
   exactly: a bloomcrown root box holds 22.2 soil voxels on `small` against 2.8 on
   `default`, which is 8× the voxels for ⅛ the cell volume — the same soil in metres —
   and the box reads the same 0.2500 mean pore on both presets at t = 0. No evidence of
   a scaling defect in the flora config.
5. **Not a `small` problem at all, for four of the six species.** Bloomcrown and
   springturf are insolvent on `default` too, and glowcap ends there at 0.016. Whether a
   producer should be able to pay its maintenance in soil at field capacity is a model
   question this diagnosis does not answer; `default` survives by having wetland, not by
   having healthy dryland plants.

### What this did not measure, and why

- **Cause of death is inferred.** `FloraLedger` has one `deaths` counter and no cause
  field, so every cause above except drowning is the failing clause of the stand's own
  survival rule at the tick it vanished. A stand grazed to zero foliage and a stand
  that never had income both read as a deficit unless the foliage clause fires first.
- **Per-species consumption is not readable here.** `consumed_organic_out` is one
  number for the layer (4.870 on the `small` coupled arm); what the browsers took from
  which species is the founder autopsy's measurement, not this one's.
- **Candidate 3 is arithmetic plus a consistent observation, not an experiment.** It
  needs one landform run at two cell sizes, which this package did not run.
- **One seed per preset** (base 1, 0 rejected). `small`'s own barren seeds — the recipe
  comment names five of eight — are not sampled, and `wide` was not run at all.
- The three arms are one trajectory each; nothing here is a distribution.

## Water units, 2026-09-22

Package 1c (`design/handoffs/voxel-water-units-2026-09-22.md`) turned the
solver's permeability-driven flux from a fraction of a cell per tick into a
conductivity in metres per second: `rate_m3 = K · face_area · dt` with
`K = permeability_per_s · pore_capacity · 0.25` m/s (soil 0.0175, rock 5e-6).
Four sites — the water table's band uptake and its seepage, surface
infiltration, and drainage. `permeability_per_s` and `pore_capacity` keep
their authored values, nothing serialised changed, and the conversion factor
is chosen so the 0.25 m reference grid is untouched. Rain and evaporation
were already per area; the outlet weir is a share of the cell on purpose and
was left alone. Candidate 3 of the D5 diagnosis above, measured.

**The reference grid is identical, and not just in arithmetic.** On 0.25 m
cells `pore_flux_m3` reproduces the old expression bit for bit (`to_bits()`
equality, every material, three timesteps). End to end, over a full hour:
`voxel_plant_autopsy 1 preset=default` writes a **byte-identical** CSV before
and after, and so does `water_cycle 1 … preset=default`. `wide` is 0.25 m too,
so the same holds there; `small` is the only shipped ring the fix moves.

**Drownings, 1 h, seed base 1, plants-only** (before = the same binary with the
pre-1c expression restored, so this is a controlled A/B and not a comparison
across commits):

| | small (0.125 m), 27 stands | default (0.25 m), 87 stands |
| --- | --- | --- |
| drowned by min 10, before | 14 (all at min 10) | 8 (5 at min 0, 3 at min 9) |
| drowned by min 10, after | **8** | 8 — identical |
| drowned at 60 min, before → after | 14 → **8** | 8 → 8 |
| deaths at 60 min, before → after | 22 → **20** | 43 → 43 |
| establishments | 19 → 19 | 42 → 42 |

The first shower still drowns eight of `small`'s stands. Halving the depth of
standing water is what the ground draining at its proper speed buys, and it is
not by itself enough: the springturf and glowcap drown depths are 0.03 and
0.05 m and the runoff still clears them. D5's other two findings stand.

**Closed cycle, 1 h** (`water_cycle 1 1 0.02 5.0 0.0001 4 preset=…`):

| | stored (m³) | pooled | pore | aloft | showers | residual |
| --- | --- | --- | --- | --- | --- | --- |
| default, before | 427.36–470.82 | 122.018 | 22.728 | 230.241 | 4 | 1.1e-9 |
| default, after | identical | identical | identical | identical | 4 | 1.1e-9 |
| small, before | 91.88–100.47 | 34.598 | 6.099 | 61.115 | 5 | 1.5e-8 |
| small, after | 91.66–100.43 | 34.381 | 6.097 | 61.335 | 5 | 4.8e-8 |

`small`'s cycle barely notices: 0.6 % less pooled and 0.4 % more aloft, the
same five showers, the same verdict. The ring's water is in the lake and the
aquifer, and doubling the speed at which the soil exchanges with them moves the
standing film, not the inventory. Conservation holds on both grids, and the
residuals are the example's own floating-point floor.

### What this did not measure, and why

- **One seed per preset, one trajectory each.** `wide` was not run: it is
  0.25 m, so the fix is a no-op there by the same identity.
- **Nothing beyond an hour.** The 6 h picture of D5's table is not re-run here.
- **The drowning counts are the autopsy's inferred cause**, unchanged from D5:
  a stand whose water depth passed its `drown_depth_m` on the tick it vanished.
- **A mirror-symmetry fixture lost strength.** `core::the_mirrored_fixture_
  gives_the_mirrored_answer` now holds to 5e-3 rather than 1e-6. Two basins
  sealed by a sill freeze whatever split the spill left them, and that split
  turns on one substep-level head comparison; the units change moved the
  operating point across it. Measured constant in the poured volume (4.9–7.0
  m³) and in the tick count (20–1000), so it is a frozen decision and not a
  drift, and conservation and the materials are still mirrored at 1e-12. The
  order dependence is the damp-set walk `drain`'s own doc names, not something
  1c introduced — but nothing here measured how far it reaches.

## Diets, 2026-09-22

Package 3 (`design/handoffs/voxel-diets-2026-09-22.md`; decisions §3) on top of
layers: the browser eats every **vascular** species' foliage in its band and
nothing fungal, the shredder is a detritivore with three foods — litter,
glowcap cap tissue and carrion — and the cue field's source is the sum of those
three at a face. Fauna snapshot schema 11 → 12; fresh worlds. `before` is main
at 4661f97, built from `git archive` into a scratch tree and run with the same
binaries' arguments; it reproduces the layers note digit for digit on
`default` (starved 34 = 23 shredder + 11 browser, drowned 2, umbrellafrond
7,417 bites, bloomcrown 0, 2 browsers and 13 shredders alive at 60), which is
the check that the two arms differ only by this package.

### `voxel_founder_autopsy 60`, bites by food class

| | small before | small after | default before | default after |
| --- | --- | --- | --- | --- |
| shredder bites | 10,978 | **23,629** | 61,254 | **38,104** |
| — litter | 10,978 | 22,052 | 61,254 | 33,553 |
| — glowcap cap tissue | – | 1,450 | – | 4,191 |
| — carrion | – | 127 | – | 360 |
| browser bites (all vascular foliage) | 12,280 | 4,155 | 17,526 | 10,696 |
| browser bites on glowcap | 3,905 | **0** | 2,407 | **0** |
| shredders alive at 60 | 3 | 6 | 13 | 4 |
| browsers alive at 60 | 0 | 0 | 2 | 1 |
| shredder starved / drowned | 13 / 2 | 16 / 0 | 23 / 2 | 24 / 0 |
| browser starved / drowned | 11 / 4 | 7 / 1 | 11 / 0 | 7 / 1 |
| browser extinct, minute | 53 | 28 | – | – |

`bites_by_plant[glowcap]` equals `bites_by_food[cap_tissue]` to the bite in
both after arms (1,450 and 4,191) and `bites_by_food[foliage]` equals the
browser's whole bite count (4,155 and 10,696), so every glowcap bite is a
shredder's and every browser bite is vascular: the acceptance rule is the one
that ran. Ledger residuals over 72,000 ticks: fauna ≤ 7.3e-11, flora ≤ 3.4e-10
absolute on both arms.

**Carrion finally has a consumer, and it is a trickle**: 127 and 360 bites,
0.013 and 0.039 organic — under 2 % of what the shredder ate. A corpse
decomposes at `carrion_decomposition` whether or not anyone finds it, so the
shredders are eating the few corpses that fall where they already are.

**On `small` the glowcap was the browser's nursery.** Before, the browser ate
the whole 0.624 of cap stock in the first ten minutes (3,880 bites), lived on
it, found the bloomcrowns at minute 10 and took 3,380 bites off them before
dying at 53. After, it has no cap, never reaches a bloomcrown at all
(**0** bloomcrown bites) and starves at 28. That is decisions §3 working as
written — fungal tissue is not leaf — and it is the coupled-assessment
package's problem, not a number to tune here.

### `voxel_census 6 preset=default`, shredders alive

| sim hour | 1 | 2 | 3 | 4 | 5 | 6 |
| --- | --- | --- | --- | --- | --- | --- |
| shredders, before | 13 | 10 | 10 | 8 | 11 | **10** |
| shredders, after | 4 | 2 | 1 | 1 | 0 | **0** |
| standing litter, before | 4.61 | 3.50 | 3.34 | 4.11 | 6.15 | 11.33 |
| standing litter, after | 6.25 | 4.90 | 6.68 | 8.07 | 11.28 | **15.20** |

**The shredder lineage goes extinct on `default` at about minute 300, with more
litter on the ground than the surviving arm has.** It is a foraging failure and
not a food shortage, and it is the largest thing this package did.

### Which half of the change costs it

Two same-build control arms, `voxel_founder_autopsy 60`, each differing from
the `after` build by one line:

| default, 60 min | before | **after** | A: taste left litter-only | B: cap not a cue source |
| --- | --- | --- | --- | --- |
| shredders alive at 60 | 13 | **4** | 3 | **17** |
| browsers alive at 60 | 2 | 1 | 2 | 2 |
| shredder bites | 61,254 | 38,104 | 32,913 | 71,226 |
| — litter | 61,254 | 33,553 | 27,267 | 68,836 |
| — cap tissue | – | 4,191 | 5,146 | 1,826 |
| — carrion | – | 360 | 500 | 564 |
| shredder assimilated | 1.144 | 0.729 | 0.684 | 1.299 |

Arm **A** leaves the shredder's **taste** on litter alone — the one channel
change the brief did not name. It is *worse*, not better (3 alive against 4 on
`default`, 0 against 6 on `small`), so the taste reading the richest food at
the mouth is carrying its weight and is kept.

Arm **B** keeps cap tissue as a **food** and takes it out of the **cue's
source**, one line in `detritus_at`. It is the whole regression and then some:
17 shredders alive against main's 13, 68,836 litter bites against main's
61,254, and the cap and carrion edges still used (1,826 and 564 bites). So the
diet edges are not what costs `default` its shredders — the cue's source set
is.

Why: emission is `min(stock / M_EMIT, 1)` with `M_EMIT` 0.05. A `default`
litter tile holds 0.2 and a glowcap cap 0.106, so **every one of the nine
glowcap stands became a fully saturated emitter**, next to eight litter tiles.
The source set nearly tripled in count while the food behind most of it is a
small, slowly-regrowing cap a shredder empties in a bite or two, and a policy
that climbs the cue is led to it. `M_EMIT`, the transport and the source set
are all things the brief holds fixed, and package 5's retrain is where a cue
of this shape is learned. **Arm B is not applied**: which of the three — ship
as decided and retrain, weight a source by its class, or drop cap tissue from
the cue and keep it in the diet — is a decision for Wrysk, and it is one line
either way.

### The observer, at t = 0

The browser's reach is now a permission test as well as a geometric one, so
glowcap counts in the standing foliage and never in the reach; it is reported
as the shredder's food in the detritus block, which already existed.

| t = 0, fraction of foliage | small | default | wide |
| --- | --- | --- | --- |
| reachable, before → after | 0.5907 → 0.4573 | 0.5611 → 0.5137 | 0.5946 → 0.5432 |
| route-connected, before → after | 0.5907 → 0.4573 | 0.2234 → 0.1985 | 0.3883 → 0.3369 |

Every point of the fall is glowcap and nothing else (`default`: the glowcap
row's reach 0.958 → 0.000 against an unchanged 0.958 of standing foliage). The
`visible` columns do not move: sight has no diet.
