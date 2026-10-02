<!-- Evidence, not a decision (design/7_Research). Recommendations only: Wrysk 2026-10-01 wants this tuned in the integrated co-training, no change now. Raw CSVs and the instrumentation patch were in a session scratchpad (disposable). -->

# Canopy night diagnosis — umbrellafrond and bloomcrown (2026-10-01)

Scratch worktree off main 72ec0b17; patch in `instrument.patch` (flora `diag` accumulators
plus a `genome=` and `CANOPY_CSV=` hook in `voxel_census`). Arms: `config=config/desktop/terrarium.toml`,
threads=2, nice 10, 2.7 h asked:
- live1/2/3: seeds 1-3, `genome=runs/live-view/a-g12/mean.toml` (applied through the live app's own
  `cubarium::voxel::params_with`), `policy=browser=centre-frondgrazer.json policy=littershredder=centre-littershredder.json`
- nofauna1: seed 1, live genome, no founders
- default1: seed 1, default params, same policies
live2 ran slowest and was stopped at sim min ~124 during cleanup (tables use rows to min 120); it still covers the night and
the first 25 min of morning. Raw rows: `<arm>.csv` (every 2 sim min, per species); tables:
`tables_seed1.txt`, `tables_live2.txt`, `tables_live3.txt`, `tables_nofauna1.txt`, `keyrows.txt`.

Columns: fill = mean per-stand foliage/(alpha W); fbot / ftop = mean per-stand fill of the bottom / top
foliage layer, computed as the renderer does (`layer_stock[i] / (share alpha W)`); topAgg = sum(top stock)/sum(top cap)
(biomass-weighted); top<25% = share of stands whose top layer draws below a quarter. The flows are summed over each interval.

## Day clock (measured)
daylight < 0.95 from min 54 (seed 2: min 50); full night min 58-92 (seed 2: 54-88); dawn 94; full day from 96.
Twilight lasts only about 4 min. plant_night_scale is 0.05 all night.

## Group membership and the live genome
Both species are in `@shrub` (crates/cubarium-voxel-flora/src/registry.rs:66, with lanternberry). The live genome
gives foliage_rate x0.475 x K_bio 1.113, senescence x1.069 x 1.113, alpha x1.713.
Growth runs at its foliage_rate cap all day (grown / (fr W dt) = 0.85-0.99), and shed = sen·P exactly. So the
daytime equilibrium is P/W = fr/sen, and the **crown fill ceiling is fr / (sen·alpha)**:
- default: 0.002 / (0.001 · 2) = **1.00**
- live genome: 0.00106 / (0.00119 · 3.43) = **0.26**
Measured P/W in the live arms is 0.6-0.88 (the ceiling is 0.89), against 1.3-1.67 in default1.

## Key rows (n / fill / topAgg / top<25% share) — see keyrows.txt
```
live1  UF  2:0.79/0.48  6:0.46/0.03  12:0.30/0.04  24:0.24/0.09  52:0.21/0.13  92:0.19/0.13  120:0.21/0.16  160:0.22/0.09 (95% <25%)
live1  BC  2:0.79/0.71  6:0.46/0.38  12:0.32/0.20  24:0.22/0.10  52:0.22/0.10  92:0.21/0.09  120:0.22/0.12  160:0.22/0.09 (97% <25%)
dflt1  UF  2:0.87/0.65  6:0.67/0.25  24:0.60/0.26  52:0.64/0.58  92:0.61/0.51  120:0.67/0.55  160:0.70/0.38
dflt1  BC  2:0.89/0.87  6:0.70/0.76  24:0.65/0.75  52:0.72/0.80  92:0.67/0.72  120:0.70/0.61  160:0.72/0.61 (0-6% <25%)
```
The other live arms and nofauna1 match live1 to within ±0.03 fill.

## Phase sums (foliage in organic units; the full set is in the tables)
```
arm/species     phase      foliage s->e    fill s->e   |  shed  grown grazed dieback
live1 UF        day1 2-52   7.60->4.52     0.79->0.21  | 19.10 16.27  2.12  0
live1 UF        night       4.75->4.18     0.22->0.19  |  0.58  0.01  0.02  0.035
live1 UF        day2 96-162 4.57->10.88    0.21->0.22  | 35.02 40.65  0     0.001
live1 BC        day1        11.48->7.31    0.79->0.22  | 28.10 23.19  2.95  0.071
live1 BC        night       7.67->6.80     0.23->0.21  |  0.94  0.01  0     0.032
live3 UF        night       5.60->4.29     0.23->0.20  |  0.76  0.25  0.76  0      (grazing = half the night loss)
nofauna1 BC     night       10.15->8.99    0.24->0.22  |  1.24  0.01  0     0
default1 UF     night       8.99->8.10     0.66->0.61  |  0.93  0.01  0.03  0.013
default1 BC     night       17.34->15.66   0.74->0.67  |  1.80  0.02  0     0
```
No stands died in any arm, and none drowned (died_n and drowned_n are about 0).

## Answers
(a) The night loss is real but small: 11-13% of total foliage in every arm (live1 UF -12%, BC -11%; default UF -10%, BC -10%),
which costs the canopy about 0.03 fill. It does come off the top layer first, but the top layer is already nearly empty in the live
genome: topAgg is 0.09-0.16 and 90-99% of stands draw their top layer below 25% **all day**. The large visible loss is the
founding collapse in the **first day**, not the night: fill falls 0.79→0.22 and UF's top layer 0.48→0.03 within 6 sim min of a fresh world, because founders are planted at
foliage = alpha W (lib.rs:4078) and the genome's ceiling is 0.26.
(b) Dusk: twilight is ~4 min, and growth still exceeds shed (UF 1.06 vs 0.83). Night: senescence at 5% dominates (~0.02-0.05/2 min); there is
no growth; dieback is negligible (≤0.06 total); grazing equals senescence for UF in live3 (0.76 vs 0.76) and is ~0 elsewhere. Dawn: growth restarts with the
light and exceeds shed within the first 2 min. Day: shed ≈ growth (live1 day 2: 35.0 vs 40.7) with growth pinned at the foliage_rate cap.
(c) Total foliage recovers its pre-night value within ~6-10 min of sunrise and keeps rising with stand count. Fill returns only to its
0.21-0.23 ceiling. The crown never recovers under the live genome: topAgg stays 0.09-0.16 and is still falling in day 2 (live3 BC 0.11→0.05) as new stands enter
the 3-layer stage empty-topped. Under default params the night costs UF topAgg 0.64→0.51, and it is back to 0.59 by min 110 (~15 min of day).
(d) Almost all of it comes from the genome. Default params hold fill 0.61-0.74 and BC topAgg 0.6-0.8 through the night. The live genome holds fill 0.19-0.23 and topAgg ~0.1 at every
hour. The night itself costs the same ~11% in both.
(e) Surprises: (1) The founder collapse happens in daylight within minutes, so a freshly restarted world loses its crowns before the first night. If
Wrysk restarts in the evening or looks the next morning, it reads as "overnight". (2) The growth knob and alpha are separate genes, but the fill ceiling is
their ratio. The search pushed alpha up 1.71x and growth down 0.47x, which cut the drawn fill ~4x without hurting ecology fitness (P/W, the
grazeable amount per stand, only fell from ~1.5 to ~0.8). (3) Even with default params, UF's top-layer fill falls in day 2 (0.59→0.38) as stands
graduate into the 3-layer stage. The stage rebin fills bottom-up, so a newly adult crown starts empty. (4) live bloomcrown shows some daytime dieback on
day 2 (live1 0.49, live3 1.42; default ~0.03), which means unpaid maintenance on thin crowns. It is small next to foliage but it is the first starvation signal.
(5) Seed 2 sunset arrives ~4 min earlier than seeds 1/3. The founding loop's sim time differs.

## Recommendations (ranked)
1. Couple growth to capacity (rule), or constrain the ratio in the search (genome). The visible fill ceiling is
   foliage_rate/(senescence·alpha). Option A (rule): define foliage_rate per unit of capacity (d_p ≤ fr·alpha·W·dt) so alpha no longer
   dilutes fill and the growth gene and the leaf_fall gene set the fill directly. Option B (genome, no rule): reparameterise the @group genes so
   `alpha` is not free against `growth`, e.g. gene = fill ceiling fr/(sen·alpha) with a floor ~0.6, or add a crown-fill floor to the
   fitness. Expected effect: crowns at default-like 0.6-0.75 fill, top layer drawn 0.5-0.8. In the genome: yes (B), or it changes the meaning of existing genes (A).
2. Seed founders at their species' equilibrium fill, min(1, fr/(sen·alpha)), not at alpha·W (rule, lib.rs:4078 `seed_founder`; not genome).
   Expected effect: no crown collapse in the first 10 min of a fresh world. Under the live genome it shows thin plants from the start rather than
   full ones that empty.
3. Make the canopy layering read proportionally when fill is short (rule, not genome): shed pro rata over layers, or fill the top layer
   with a share of growth rather than strictly bottom-up (also in the stage `rebin`). At fill 0.22 that draws as a thin crown rather than a bare stem
   with a rosette. This touches design/organism-anatomy-2026-09-21.md §3 ("drops its canopy before its rosette"), so check the ledger before changing it; it is Wrysk's call.
4. Leave plant_night_factor (0.05) alone, or at most make it a world knob in the search's world genes. The night costs ~11% foliage and ~0.03 fill in
   every arm and recovers within ~10 min of dawn. Lowering it to 0 would remove that small dip only (knob; not the cause).
5. Frondgrazer night feeding on umbrellafrond (live3: grazing = senescence at night; intake@frondgrazer 1.58) is already a genome knob.
   Watch it, but it is not the main effect.
