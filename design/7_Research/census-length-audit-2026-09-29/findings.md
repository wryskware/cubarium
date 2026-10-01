<!-- Evidence for design/7_Research/census-length-audit-2026-09-29.md. Written 2026-09-29 by a read-only research worker; checked by the audit thread. No new runs. -->

# Census audit: were the 4 h / 6 h runs needed? (read-only, 2026-09-29)

Method note. Raw CSVs survive only for the 2026-09-20/21 censuses (5 x 6 h, per-minute) and the
24 h water-only runs; those rows are computed from the CSVs (script: scratchpad/an.py).
Everything after 09-22 survives only as hourly/30-min tables in write-ups and commit bodies;
those rows are INFERRED from the tables and marked "(table)". runs/es*, runs/step*, the
ecology-check CSVs and the 09-22..09-25 census CSVs no longer exist. eidolon.local does not
resolve from here (ssh: could not resolve hostname), so nothing was read there.

## 1. The census tool (crates/cubarium/examples/voxel_census.rs)
- Logs one CSV row per simulated minute (1200 ticks): per-species stands, animals+mean body per
  species and per founder lineage, eggs/gestating, flora/fauna births+deaths, litter, water
  (stored, atmosphere, showers, residual); later columns: lake level, weather (temp, rh, rain events,
  stream), lanternjaw strikes/kills/eggs, ms/tick, plus hourly stderr lines (succession, seedmodes,
  seed bank, `phases:` ms/tick per minute). It logs NO dead-wood stock and no per-cause deaths
  (causes only via the autopsy tools).
- Args: `HOURS` (default 6) `[authored|generated] [open|closed] [heuristic]`, `preset=small|default|wide`,
  `config=FILE`, `seed=N` (default 1), `threads=N`, `nofauna`, `gap=off`, `night=F`, `eggbank=off`,
  `introduce=lanternjaw:N`, `save=DIR [save_min=M]`, `load=DIR` (resume; so a long run CAN be
  resumed/branched from a saved hour), `world=FILE`, `policy=`, `still`, `ab=`/`modes=`, `pin=`.
  One seed per process; multi-seed/arm is done by launching many processes (predation P1 runs 40).
- Founders: found.rs = lake gate -> pre-roll (water only) -> habitat::seed -> introduce_body.
  Animals are all introduced at t = 0, hungry, age_ticks = 0. Lifespan (lib.rs:1523
  draw_lifespan_ticks, arriving = true): median 4 h (LIFESPAN_MEDIAN_S = 14 400 s), newborn =
  uniform [0.8,1.2] x median = 192-288 min; founder = that x uniform [0.5,1.0) = 96-288 min
  (Monte Carlo: median 178 min, p5 118, p95 252; 6 % older-age-dead by 120 min, 27 % by 150,
  52 % by 180, 91 % by 240; only 38 % of founders fall inside 0.8-1.2 x the 240-min median).
  No Aged death is possible before minute 96. Aged shipped 2026-09-25 (3021eea9); every census
  before that had NO age mortality.

## 2. Per-census table
Y/N = would the snapshot at that time have given the same verdict as the end of the run.
"Verdict-clear" = earliest simulated minute at which the verdict stood.

| # | date | edit / question | sim length | seeds x arms | wall | verdict | verdict-clear | 60 min | 120 min |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 09-20 | Run A: authored open world, legacy grazers (a3761d5) - does the seeded habitat last? CSV | 6 h | 1 x 1 | n/r | plants collapse on a schedule, animals extinct | animals: extinct min 47 (run A per the worktree summary; runs/voxel-census-authored-open-6h.csv on disk has the same values as run B; its header differs). Plants: springturf 0 at min 16; births frozen at 62 from min 0; glowcap 0 @139, velvetpad 0 @180 (halved @161), bloomcrown+umbrellafrond 0 @250 | Y (headline). Species timing N: only springturf gone; glowcap/velvetpad/bloom/umbrella still full at 60 and 120 | Y headline (glowcap gone by 139); N for velvetpad/bloom/umbrella (still 100 % at 120) |
| 2 | 09-20 | Run B: same + live founders (be9640f) - CSV | 6 h | 1 x 1 | n/r | 16 founders boom to 65 at min 5, all dead by 26 (shredders 19, grazers 26); plants identical to A to the minute | animals min 26 (halved from peak min 15-16) | Y | Y |
| 3 | 09-20 | Closed water budget on generated world (72101aa); reproduced by Fable byte-for-byte (2 more CSVs) | 6 h | 1 x 1 (+2 repro) | n/r | plants persist (6 stands types; 62 -> 376 births), "water was the whole plant story"; glowcap 0 @307 | persistence: min ~30-60 (births 129 at 30, 158 at 60, deaths 24). Glowcap: flat at 8 from min 53 to 229, halved min 229, zero 307 | Y for headline (births 158 vs 62). N for glowcap (looks healthy) | Y headline; N glowcap (8 stands, 100 % of start) |
| 4 | 09-20 | S2: saprotroph draws litter as well as dead wood (3d1841b, 864f926); CSV | 6 h | 1 x 1 (vs #3) | n/r | glowcap 12/10/9 at 1/3/6 h vs baseline 8/8/0 | fix visible only when baseline dies: min 229-248 (baseline 8 -> 2). At 240: 10 vs 2 | N (12 vs 8: within growth noise, both alive) | N (10 vs 8) - needs >= 240 min |
| 5 | 09-21 | Reproduction package (gestation/eggs) - CSV | 6 h | 1 x 1 | n/r | grazers 0 from min 35 (halved min 25); shredders 0 at min 122 (halved min 27); plants "6 of 6 alive" (velvetpad 2, bloomcrown 33) vs recorded 5 of 6 | animals min 35 / 122; plants: 6/6 alive at 60 and 120 | Y animals (grazers 0, shredders 14 = -36 % from 22); plants Y | animals Y (shredders 1); velvetpad 11 flat till 165, then -> 2 at 360: NOT visible at 120. "6 of 6 alive" at 6 h was misleading. |
| 6 | 09-21/22 | Edible stock at seeding vs 6 h, then Layers (cfcb3477) - lollipop vs layered plants, 3 presets, tool voxel_edible_stock (30-min blocks) (table) | 6 h | 3 presets x 2 builds x seed 1 | n/r | layers put real food on the floor, browsers last 2x longer (180 vs 90 min on default) and still die; small collapses either way | browsers: gone by 30 (small), 60-90 (small/default before), 180-210 (default after). Foliage 6 h reachable 0.414 vs 0.067 | partial: small/wide same verdict; default "browsers last longer" needs >= 120-180 | Y for direction (before 0 vs after >0 alive at 120); end-of-life 210 N |
| 7 | 09-22 | D5: why small's plants collapse (41d0a25, f2ac848) (table + t=0) | 6 h | 3 arms x seed 1 | n/r | small ring has no wet ground -> no solvent plant (umbrellafrond/velvetpad seeded 0) | t = 0 (eligible columns 0.4 %, want=round(0.0375)=0); drowning 14/27 at min 10; pore 0.25 -> 0.17 at 30 min | Y | Y |
| 8 | 09-22 | Diets (60dea6b0, 091151b9): cap tissue in the shredder's cue | 6 h | default x 3 arms x seed 1 | n/r | cue costing caps extinguishes shredders ~min 300 (10 alive -> 0); applied rule ok | C arm: 4 at 1 h vs 13, 2 at 2 h vs 10 (-70 % by 60 min) (table) | Y | Y |
| 9 | 09-22 | Startup acceptance (4163ea94): seeding after first shower, route | 6 h | default x before/after x seed 1 (+8 bases at 60 min) | n/r | 60-min autopsy over 8 bases decided it; 6 h census: shredders 7 vs 6 at 6 h, browsers 0 both, "direction not proof" | census verdict = hour-1 sign (6 -> 23 after) | Y (the 60-min verdict WAS the verdict) but note 60 min shows 6 vs 23 while 6 h shows 7 vs 6: the 6 h numbers do not confirm | same as 60 |
| 10 | 09-23 | Retrain P5-C gate (bb0cf406): 3 arms | 6 h census bases 1-3 + 60-min autopsy x 8 bases | 3 seeds x 2 arms (6 h) | n/r | NOT shipped: drownings up 21/19 -> 64/51 default, up per body-hour (from the 60-min autopsy) | drownings: min <= 60 (autopsy) | Y | Y. 6 h census was noise-dominated: shredders main 6 / 0 / 802, cand 479 / 0 / 500; seed 3 main has 802 shredders, seed 2 main 0 |
| 11 | 09-23 | Plant-viability scratch census, then SU/S/marks/L-N follow-ups (40872d1 ... 86e5ed6b, 2ccc98b0) (table) | 4 h (3 h, 2 h on terrarium) | default s1-4 x before/after (+nofauna, terrarium s1-2) | terrarium ~1 sim-min per wall-min at that date | plants starve at field capacity: all springturf dead min 35-60; browsers extinct 90-120; FC-capped probe 84 -> 250 stands; succession woody share 11.9 -> 22.8 % at 4 h | springturf min 35-60; browsers min 90-120; 250 stands is an end value | Y for cause; browser extinction N (halved, not gone) | Y. Succession/marks: marks 0.53/1.03/1.08 % (s1), 1.42/2.15/2.06 % (s2) at 1/2/4 h -> plateau by 2 h (Y at 120, half at 60); succession end-value 4 h not derivable from tables |
| 12 | 09-20 | Water cycle: 24 h x 3 seeds, water only (voxel-water-cycle) - CSV | 24 h | 3 x 1 | n/r | BOUNDED (stored 192-218 m3; drift 0.25-0.67 %) | stored within 2 % of final mean from min 50 / 60 / 90 | Y (seeds 1-2) / near (seed 3 within 5 %) | Y all three; nothing new after minute 120 |
| 13 | 09-24/25 | WX1 calibration: 4 h terrarium seeds 1-2 gap on/off (eidolon), 24 seeds x 4 h + 4 x 24 h lush mirror (8417b0df, f3963f5a) | 4 h (24 h lush) | 2 x 2 (+24) | n/r | rain 0-3 % 12-18 h, lake within +-2 % after hour 2, stands s2 -25 % | lake +-2 % after min 120 by definition; s2 -25 % stands from the lake drain in the first minutes (see 1d) | N/Y mixed (rain time-of-day stats need >= 2 days = 4 h of the 2 h day) | Y for lake; day-phase rain shares need the full 4 h |
| 14 | 09-28 | WX1d (65d0cf23): terrarium seed 2, stands -46 % at 4 h | 4 h | 1 | n/r | cause: lake drained 17.8 -> 42.8 m3 in the 8 min after founding, drowning shore plants | minute ~8 (lake volume) | Y | Y |
| 15 | 09-24/25 | Ecology search (fd0c1f2e, f6421d82): CMA-ES, horizon 3 h projected onto the 6 h band by last-hour trend | 3 h (scored on 6 h) | 16 worlds/generation | 6 h generation = 27-28 min on shared CCD1 (whole generation) | tuned vectors | the project already uses a 3 h run + projection | - | - |
| 16 | 09-25 | Lifespan/defence "6 h census with new defaults" (brief 9ea6643e asks for it) | 6 h | - | - | NO written result found | - | - | - |
| 17 | 09-28 (running now) | Predation P1: lanternjaw egg bank vs control (heuristic controllers) | terrarium 4 h; default and small 8 h | 40 jobs: terrarium s1-4, default s1-8, small s1-8, each egg/off | 8 processes x threads=2 at nice 10 on CCD1; after 32.5 wall-min: terrarium s1 71 sim-min, s2 42, s3 97, s4 84 (rows in the live CSVs) | pending | - | - | - |

Counting: 15 distinct census runs/families with a written verdict (rows 1-14 + 17 in progress;
row 15 is a search, row 16 not found). Headline verdict already clear by 60 min in 9 of 14
(rows 1, 2, 3, 5, 7, 8, 9, 10, 11-cause; 12 within ~60-90) and by 120 min in 12 of 14. The
exceptions needing more than 120 min: row 4 (glowcap fix, needs >= 240), the glowcap collapse
in row 3, the velvetpad decline in row 5, and the day-phase rain statistics in row 13.

## 3. Stability verdicts (CSV, generated closed 6 h; rows 3 and 5 alike)
First minute after which the species stays within +-20 % of its final-hour mean:
bloomcrown 165, springturf 242, stonecushion 225, velvetpad 223, umbrellafrond 275
(still growing at 360: 149 = its maximum), flora births 247, litter 288 (rises monotonically
14.6 -> 150; never stabilises). Reproduction run: bloomcrown 79, umbrellafrond 271, springturf
284, stonecushion 205, velvetpad 238 (its collapse), glowcap 296. So the 6 h census did NOT
reach a steady state in either run; a 60 or 120 min snapshot is never "stable". What 60/120 do
give is persistence (no zero, births > deaths). Decline >50 % from peak by 60 min: only
springturf (all runs), all animals (halved min 15-27), the dry-world plants none except springturf.

## 4. Slow clocks a census must span (simulated minutes)
Day 120 min (mode cycle, night 39.6 min, plants photosynthesise 24 h unless night factor set);
weather_speed 5.03.
- Animal lifespan: median 240; newborn 192-288; founders 96-288 (median 178).
- First birth >= ~60 min after own birth (browser body_min -> birth_body 2632 s = 44 min growth).
- Breeding cycle: browser surplus hold 15 + gestation 3 + interval 30 = 48 min (blind 15 + 5 incubation
  + 30 = ~45-50 min); as of 09-28 eligibility = surplus for 80 % of the last 900 s (schema 20).
- Defence half-life 90 min (5400 s), rise tau ~8.7 min (520 s).
- Plant time to first reproduction: springturf 2.7 min, bloomcrown 34 min, vaulttree 191 min.
  Leaf life 16.7 min (1000 s); regrowth 16.7 min; woody seed banks 2-12 h (glowcap 120, velvetpad 180,
  umbrellafrond 240, vaulttree 360, bloomcrown/lanternberry 480, siphonreed 600, springturf/stonecushion 720).
  Succession overtopping: mean ~25 min under a mat, but stand share drifts over 4+ h.
- Seed bank turnover: hours (2-12 h) -> a "bank turns over" claim needs >= 8 h (P1 brief).
- Showers: dry gap 5-15 min, event 5-6 min, ~1 event/h young world, 1.75/h lush; ~30-50 mm per
  sim-day; 33 showers per 6 h on small/default. Rain time-of-day share needs whole days (2 h each).
- Litter decay tau 16.7 min; carrion 3.3 min (P1 changed it to ~1 h); DEAD WOOD tau 167 min (1e4 s)
  - the substrate glowcap lives on; this is the clock behind glowcap's min-229 collapse.
- Water: closed budget stationary within 60-90 min; lake/aquifer drift ~0.1 m3 per 6 h.
- Plant maintenance/reserve bills: dry-world plants die at reserve/maintenance times 156-250 min
  (stonecushion extrapolated 615) - computed from t = 0 stocks by D1's plant-only autopsy.

## 5. Cohort effects (analytic; no survivable data)
No post-Aged census output survives (row 16 not found; runs/es3, es4 and the check CSVs are
deleted), so this is derived from the code, not measured.
- Founders are age 0 with remaining life 96-288 min (median 178 min = 0.74 x the median),
  i.e. the design already spreads the founder cohort over ~3 h; only 38 % die inside
  0.8-1.2 x median. A census that ends at 4 h sees almost all of G0 age out (91 % by 240).
- G1: first births >= 60-100 min after birth (founders' first offspring ~min 60-100), so G1
  lives 192-288 min from birth -> dies 250-390 min. That echo lands at 4-6.5 h, i.e. the last
  two hours of a 6 h census. G2 would peak beyond 7-8 h.
- Pre-Aged history: booms without an age brake ("grows exponentially until food per head
  runs out", b8d6a098: candidates 387-1390 animals at 6 h; retrain seed 3 shredders
  8 25 67 161 449 713 802). The early boom-bust (16 -> 65 at min 5 -> 0 at min 26) is starvation
  of hungry founders, not age.
- The search treated "shorter lives" as its only brake (lifespan x0.503, Spearman +0.80).
- A 60/120 min run contains zero age deaths (first possible at 96), so it cannot test lifespan,
  defence half-life (90 min) or the G1 echo.

## 6. Late events only the long tail caught, and early predictors
| event | when | early indicator? |
|---|---|---|
| glowcap 8 -> 0, generated closed (row 3) | flat 53-229, gone 307 | Yes mechanistically, no in the logged columns: dead-wood tau is 167 min and the substrate is 0.4 org per grove, but dead wood is not a census column. Counts, births, deaths, litter gave no warning (litter rising 14 -> 150). |
| velvetpad 11 -> 2 (row 5) | flat to 165, low by 360 | Yes in the autopsy: root-box pore band 0.65 -> 0.43 (30 min) -> 0.20 (6 h), mu 0.58 -> 0.03 (D5 table). Not in the census. |
| dry-world plants (row 1): bloomcrown, umbrellafrond 250, velvetpad 180, glowcap 139 | 139-250 | Yes at t = 0: pore 0.0000, mu 0 for all 62 founders (D1); reserve/maintenance predicts the minutes to the minute (plant-only run gave 15/155/179/249). Also births flat at 62 from minute 0 and springturf dead at 16. |
| diets C arm: shredders extinct ~300 | 300 | Yes: 4 at 1 h vs 13, 2 at 2 h vs 10; litter accumulating (foraging failure). |
| browsers alive at 180 in layered default | 180-210 | The direction was visible at 120 (before 0, after >0). |
| glowcap fix effect (row 4) | 229+ | No early separation at 60/120 (12 vs 8, 10 vs 8). Needs the baseline to die. |
| terrarium seed 2 stands -46 % at 4 h (row 14) | 4 h | Yes: lake 17.8 -> 42.8 m3 within 8 min. |
| lanternberry donors 18 -> 2 (s2), 19 -> 6 (s4) over 4 h | 4 h | Not determinable; no hourly table found. |
| retrain seed 2 cand shredders 13 2 1 0 | hour 2 | Yes, hour 1 to 2 drop (13 -> 2). |
| unbounded animal growth (802 shredders, 387-1390 at 6 h) | 4-6 h | Yes early: exponential growth rate by 30-60 min (8 25 67 161: x2.5 per hour); the ecology search already projects this from the last hour and early-stops at 150 animals. |

## 7. Throughput
- Tick rate 20 Hz -> one simulated minute = 1200 ticks. sim-min per wall-min = 50 / ms-per-tick
  (ms/tick 6.2 -> 8.1; 12.9 -> 3.9; 15.9 -> 3.1).
- Eidolon terrarium seed 1, threads=6 on 3 cores (ddf0f70f, 09-24): 6.2-6.5 ms/tick at min 60,
  8.6/7.7 at 120, 11.1/9.7 at 180, 12.9/9.7 at 240, 15.9/13.3 at hour 6 (10.7k stands): roughly
  8 -> 3 sim-min per wall-min, ~70-80 wall-min for 6 h on that world. It slows with stand count.
- 09-23 terrarium seed 1 (before dense-cost): ~1 sim-min per wall-min. 09-25: parallel water
  merged 045245c (terrarium shower 18.9 -> 2.9 ms; live step 17-25 -> ~4 ms/tick).
- Ecology search: 6 h generation, 7 draws + mean, 2 seeds, 16 worlds = 27-28 min, coarse water
  --water-every 4 (fewer than 8 generations in a 3 h budget).
- Predation P1 now (live logs, 8 processes x threads=2, nice 10, CCD1, 12:03 AM): 18 ms/tick at
  min 68 on terrarium seed 1 (split 2:2), ~22 s wall per sim-min; observed 1.3-3.0 sim-min per
  wall-min over 32.5 wall-min. At those rates a 4 h terrarium run needs ~1.5-3 wall-h and the
  full job list (32 + 128 + 128 = 288 sim-hours over 8 slots) roughly 12-20 wall-h; small/default
  are cheaper per minute than the terrarium, so this is an upper bound.
- Older worlds: memory note (2026-09-18) 29x real time at 16 threads (6 h ~ 12 min).
- Wall times were not written into any census note; only these commit-message figures exist.

## 8. Take-aways for the recommendations (evidence only)
1. Most verdicts (extinction, drowning, gate results, cause of collapse) were clear by 30-120 min
   and often at t = 0 (D1, D5 eligibility tables) or within ten minutes (WX1d lake drain).
   Several conclusions were actually drawn from 60-min autopsies (D3, D4, D5, startup, retrain).
2. The long tail bought three things: glowcap/velvetpad late decline (dead-wood tau 167 min;
   pore trend), confirmation of the glowcap fix (only visible >= 240 min), and, since 09-25,
   the only place aged departures and the G1 echo can appear (>= 96 min, echo 250-390).
3. None of the 6 h runs reached steady state (umbrellafrond, litter still climbing), so "stable
   at 6 h" was never actually tested; nothing shows a run > 6 h.
4. Single-seed 6 h numbers are noise-level: shredders 7 vs 12 vs 10 across changes that touch
   no ecological rule; retrain seeds 3 vs 2 differ 802 vs 0.
5. Early indicators that would have flagged every late event but were not logged: dead-wood
   stock, per-cause death rate vs births, pore/mu per species. Adding them to the census would
   let a 60-120 min run predict the tail.
6. Caveat: lifespan/defence effects (240 and 90 min clocks) cannot appear in < 96-120 min;
   any verification of those edits needs >= 4 h or a forced small lifespan (fauna tests use
   `short_lived`), or a `load=` resume from a saved hour-2 world.
