<!-- Appendix to design/7_Research/ecology-parameter-audit-2026-09-28.md: the flora inventory's working notes, written by an audit worker at 3ccc6daf. Rows are in parameters.tsv (subsystem = flora). Evidence, not decisions. -->

# Flora inventory — notes (audit at 3ccc6daf, working tree clean)

Scope: `crates/cubarium-voxel-flora/src/` (lib, step, seeds, layers, cover). Rows: `flora.tsv` (93).
Arithmetic script: `scripts/flora_derive.py`. Live grid is **0.125 m** (`config/desktop/terrarium.toml:19`),
so `for_voxel_size` doubles every voxel distance (rooting, hop, mycelium reach); crowns are metres.
Exposure: no flora TOML table exists (backlog §1 says so); everything is STRUCT/CONST except five ES
multipliers (`crates/cubarium-search/src/es/voxel/live/ecology.rs:45`): foliage_rate and senescence
(Photo species only), decomposition, defence_per_bite, defence_half_life_s — training only.
Backlog §1 rows for flora are stale (still quote `wilt_pore`/`establish_pore_min`, crown voxels).

## Rules in plain words (per tick, DT = 0.05 s, `step.rs:201`)

Order: prune → sky cache → cover prep → **drown** → light → drink → feed (fungi) → **grow** →
vine grow → decompose → seed bank → propagate. Symbols: W wood, P foliage, Q reserve, N site mineral.

- **Drown** (`step.rs:475`): water depth on the face (transit water included) > `drown_depth_m` ⇒ dies
  this tick. No duration.
- **Light** (`step.rs:582`): per foliage layer, open-sky geometric visibility × Π over strictly-higher
  layers of other stands covering the column of `exp(−k(1−p)·stock/area)`, k = `shade_k_per_m2`;
  stock-weighted over the stand's layers; then `L_eff = L(1+h)/(L+h)` (h = `light_half`). **No day/night.**
- **Water** (`step.rs:1192`): μ = ramp(mean available water of root box; wilt, full); available water 0 at
  wilting point, 1 at field capacity. Standing-water species: μ=0 without water ≥ `water_depth_min_m`
  beside. Transpiration demand = `transpiration · P · μ · dt`, split over the box by drinkable water.
- **Aeration**: s → target (f − tol)/(1 − tol) (f = saturated fraction at pore ≥ 0.95, tol =
  `establish_saturated_max`) at `stress_rate` up / `relax_rate` down, fraction of gap per second.
- **Income** (`step.rs:2449`): `A = min( g·L_eff·μ·(1−s)·P·N/(N+K_N)·dt , f_max·N·dt , N/n_tissue )`,
  capped at what the stand can spend `m + (1+c_g)(ΔP+ΔW) + ΔQ`. Saprotroph: take
  `uptake·W·μ·dt` of dead wood + litter in its box, keep `yield` of it.
- **Spend** (`step.rs:2481`): maintenance `m_w·W` first (income, then reserve; unpaid ⇒ dieback κ·unpaid
  wood to dead wood); then 20 % of the rest to reserve; foliage up to `r_p·W·dt` (costing 1+c_g),
  **filled bottom-up** into layers; reflush from reserve if P < 0.25·P_cap; wood up to `r_w·W·dt`;
  rest to reserve. New tissue draws `n_tissue` per unit from the site pool.
- **Senescence**: `m_p·P` shed from the **top** layer to litter on the site. Death: W < `alive_min`.
- **Decompose**: litter, dead wood, carrion lose `k·stock` per s (respired out); their mineral returns to
  the **same site's** pool.
- **Reproduction** (`seeds.rs:620`): a stand with W ≥ `donor_min` diverts `propagule_rate` from reserve
  above `floor·q_cap·W`, paying c_g; when the parcel holds one seed (`seed_mass·alive_min/w_frac`) it
  becomes a runner (prob `clonal_share`, onto an adjacent free face) or a seed (drop/wind/spores/water).
  Banked seeds are checked every 30 s: gates (water, aeration, depth, **geometric** sky ≥
  `establish_light_min`, substrate, standing water) → lottery weighted by count × seed mass → arm →
  sprout next check. Woody seeds overtop ground covers at 0.1 per check.
- **Bites** (`lib.rs:4375`): a bite asks `want·(1−d)`; takes lowest in-band layer first down to its
  floor `graze_refuge·share·α·W`; each successful bite raises d by `6e-4·(1−d)`; d halves every 5400 s.
- **Mouth band** (fauna `encounter.rs:124`, ceiling 0.49875 m): at 0.125 m voxels the band is cells
  +1..+4 above the standing face; a layer is in band iff `round(band_top·H/0.125) ≤ 4` ⇔ band_top·H <
  0.5625 m (standing on the plant's own face level).

## Species fact sheets (open sky, μ=1, N=1 ⇒ Monod 2/3; "need" = one browser, 1.25e-4 org/s)

Per species: W_max / P_cap; income at full canopy vs upkeep `m·W + 1.2·m_p·α·W` (margin); break-even L·μ;
leaf life 1/m_p; full regrowth α/r_p; reserve in s of upkeep; time newborn→donor_min (ln(donor/w0)/r_w);
seconds per seed; edible in band (share·(1−refuge)·P_cap); implied foliage DM (πr²·LAI 3·LMA 0.05 kg/m²).

- **bloomcrown** — sun forb-shrub of ridges. 0.6 / 1.2; 0.0032 vs 0.00156 (2.05×); break-even 0.49;
  leaf 1000 s; regrow 1000 s; reserve 192 s (2500 s wood-only); donor at 2015 s, W_max 2708 s; seed 300 s
  (0.1 units = 2 browser bodies); drop hop 2 (0.5 m). Band: juvenile wholly in band, adult keeps rosette
  [0,0.15] share 0.25 in band until death; crown escapes at t>0.6. **Edible 0.255 (2040 s of need)**.
  ~0.05 kg DM foliage physically. Eaten by browser; litter to shredder.
- **umbrellafrond** — wetland frond, saturation-immune, spores 1.5 m + rhizomes 0.5. Same economy as
  bloomcrown. Band: seedling (t≤0.15) all; juvenile lowest tier until t≈0.49 (≈2670 s after birth);
  **adult 0 in band**. H 2 m.
- **springturf** — pioneer mat. 0.06 / 0.12; 0.00064 vs 0.000264 (2.42×); break-even 0.41; leaf 1000 s;
  regrow 100 s; reserve 114 s; donor at 161 s, W_max 230 s; seed every 9 s; hop 3 + runners 0.3; 12 h bank.
  Whole plant in band. **Edible 0.084 (672 s)**. Income-limited: funded regrowth 0.00043 (3.5 needs),
  not its r_p·W ceiling 0.0012.
- **stonecushion** — rock cushion. 0.1 / 0.2; 5.3e-4 vs 7.7e-5 (6.9×); break-even 0.14; leaf 3333 s; regrow
  4000 s; reserve 649 s (10000 wood-only); donor 8047 s, W_max 11513 s; seed 600 s; wind hop 1 (median
  0.26 m). All in band. **Edible 0.12 (960 s)**; regrowth 5e-5 (0.4 needs) — the only plant a browser
  can out-eat.
- **velvetpad** — shade pad. 0.2 / 0.4; 0.00107 vs 0.00028 (3.8×); break-even 0.26; leaf 2000 s; regrow
  500 s; reserve 357 s; donor 949 s; seed 90 s; spores 1.5 m (2 tries) + runners 0.3; 3 h bank. All in
  band. **Edible 0.28 (2240 s)**.
- **glowcap** — wood/litter fungus. Income 0.4·0.02·W = 0.008·W vs upkeep 0.0026·W (3.1×); eats litter too
  (competes with the shredder). Cap tissue is shredder food, not browser food (diet on the consumer).
- **vaulttree** — canopy tree, falls as a log. 5.0 / 10; potential 0.0267 **capped at f_max·N = 0.01** vs
  upkeep 0.0065 (1.54×); break-even 0.24; leaf 2000 s; regrow 2000 s; reserve 1154 s; donor 11473 s,
  W_max 13784 s (3.8 h); seed 300 s (0.2 units = 4 browsers); wind hop 4 (median 1.04 m). Band: seedling
  and sapling to t≈0.22 only; **adult 0**. Foliage/wood = 2 like every species.
- **lanternberry** — fruiting shrub. 1.0 / 2.0; 0.00533 vs 0.0026 (2.05×); break-even 0.49; donor 1897 s;
  seed 150 s (0.15); drop hop 2; reserve floor 0.7. Band: whole plant until t≈0.55; adult skirt
  [0.3,0.45] share 3/14 (top 0.506 m) stays in band. **Edible 0.364 (2914 s)**.
- **siphonreed** — emergent reed needing ≥0.1 m standing water. 0.3 / 0.6; 0.0016 vs 0.00084 (1.9×);
  H ≥ 0.75 m at every size, single layer ⇒ **never in a ground browser's band**.
- **latticevine** (cover.rs) — rock-face cover, per-face economy (0.0625 m² face, ×0.25 per face at
  0.125 m). Income 9e-5·light per face, upkeep 1.2e-5, leaf 0.05, ~30 min per new face; no mineral limit;
  drought dormancy after 600 s dry. Leaves are not browser food; nectar 0.002 and fruit 0.005 per face
  feed flyers. e_v 1.0 vs plants 2.0.

## Derived: regrowth into the browser's mouth band (the walking question)

Three numbers per in-band species: ceiling r_p·W_max; funded gross regrowth at full canopy
min(r_p·W, (A(P_cap)−m·W)/1.2); sustained yield MSY = max over P of [min(r_p·W, (A(P)−m·W)/1.2) − m_p·P]
(bottom-up fill sends all regrowth to the band layer first; senescence comes off the top).

| species | edible stock | ceiling | funded | MSY | refill band |
|---|---|---|---|---|---|
| bloomcrown | 0.255 = 2040 s | 9.6 needs | 9.6 | **4.9** (crown settles at P≈0.585) | 213 s |
| lanternberry | 0.364 = 2914 s | 16 | 16 | **8.2** (P≈0.98) | 182 s |
| velvetpad | 0.28 = 2240 s | 6.4 | 6.4 | **4.9** | 350 s |
| springturf | 0.084 = 672 s | 9.6 | 3.5 | **2.5** | 194 s |
| stonecushion | 0.12 = 960 s | 0.4 | 0.4 | **0.21** | 2400 s |
| umbrellafrond, vaulttree (adult), siphonreed | 0 | — | — | 0 | — |

So one mature bloomcrown, velvetpad or lanternberry indefinitely feeds **5–8 browsers** at open sky and
refills its edible band in 3–6 min; a browser parked at one plant never runs it out. What keeps the bite
small is the defence, not the stock: at 3 successful bites/s (15–20 % of 20 Hz ticks, the lib.rs:635
measurement) `d_eq = 1.8e-3/(1.8e-3+1.28e-4) = 0.93`, so each bite yields 6.7 % of its ask after
τ ≈ 520 s (1 bite/s: d 0.82, 17.6 %, τ 1370 s); relaxation half-life 1.5 h. Whether 6.7 % of an ask
still sates is the fauna/harness question (`sated_bite`).

**Density makes it worse.** One stand per support face, and equal-height mats never shade each other
(strictly-higher rule, `step.rs:672`), so a carpet holds 1/voxel² stands: 64/m² at 0.125 m. A velvetpad
carpet at full size is 0.4·64 = 25.6 org/m² of foliage ≈ 2.0e5 s (57 h) of one browser's need per m²,
regrowing at up to 64·6.4 ≈ 410 needs/m². On the 0.25 m grid it would be a quarter of that: food per m²
depends on the grid.

## Derived: mass units

Browser body_max 0.05 units (fauna lib.rs:818 "5e-5 /s at body_max") ≈ 10–30 kg ⇒ 1 animal unit ≈
200–600 kg. Plant foliage from crown size (πr²·LAI 3·0.05 kg DM/m²): bloomcrown 0.046 kg DM in 1.2 units
⇒ 0.04 kg DM/unit; springturf 0.25; stonecushion 0.058; velvetpad 0.17; lanternberry 0.045; vaulttree
0.047. Plant units are **~1500–4000× (fresh basis) too large relative to animal units**, and inconsistent
across plant species by ~6×. Equivalent statements: a bloomcrown seed (0.1) weighs two browsers; a
real 15 kg browser eats 0.3–0.6 kg DM/day, i.e. a whole real 1 m forb-shrub's foliage in ~2–4 h, but
the sim bloomcrown's foliage is 9600 s ≈ 9.6 browser metabolic time constants (≈200–340 real days).

## Derived: clocks (K = τ_real/τ_sim; K_bio ≈ 3000)

- Leaf life: base 1000 s vs herb 1–3 mo ⇒ K 2600–7800 (on clock). Vaulttree/velvetpad 2000 s vs 6–24 mo
  ⇒ 7900–31500. Foliage regrowth α/r_p base 1000 s vs 2–4 wk ⇒ K 1200–2400.
- Time to first reproduction: springturf 161 s vs 2–6 mo ⇒ K 33000–97000 (**11–32× faster than clock**);
  bloomcrown 2015 s vs 1–2 yr ⇒ 16000–31000; vaulttree 11473 s vs 5–20 yr ⇒ 14000–55000. Plant life
  histories run faster than animal metabolism implies.
- Plant maintenance: base 2e-4/s on wood only (upkeep dominated by leaf replacement 1.2·m_p·α = 0.0024/W);
  real 1–3 %/day of biomass ⇒ K 570–1700.
- Litter 1000 s vs k 1–4/yr ⇒ K 7900–31500; dead wood 1e4 s vs 0.05–0.3/yr ⇒ 1e4–6.3e4; carrion 200 s vs
  3–10 d ⇒ 1300–4300 (on clock).
- Defence rise τ≈520 s vs hours–days ⇒ K 7–500; half-life 5400 s vs days–weeks ⇒ K 11–155: the
  defence is **20–400× slower** than the animal clock.
- Aeration stress bloomcrown/springturf τ 5 s vs injury 1–3 d ⇒ K 17000–52000 (6–17× fast); drowning
  is instant.
- Transpiration: full bloomcrown 3.9e-6 m/s over its crown (338 mm/day if sustained) vs 2–5 mm/day ⇒
  K ≈ 100 — neither K_bio (30× too little water per bio-time) nor K_day 12 (8× too much). Per crown
  area it ranges 53 (springturf) to 1056 mm/day (siphonreed) because it is set per org-unit.
- Dispersal is in real space (K=1): drops 0.25–0.75 m vs 1–5 m; wind medians 0.26 m (stonecushion) and
  1.04 m (vaulttree) vs 10–100 m.

## Top findings (ranked)

1. **A single plant out-regrows its grazer by 3–8×, and its standing edible stock is 11–49 min of need**
   (table above; `lib.rs:1185`, `step.rs:2442`, `layers.rs:209`). With bottom-up refill the mouth band
   is the first thing refilled. Nothing in flora makes walking pay at open sky except the defence.
2. **Plant and animal organic units disagree by ~10³** (`lib.rs:1181` wood_max, `lib.rs:1180` α=2 for all;
   fauna body_max 0.05). Every plant stock, seed and regrowth rate is ~1500–4000× too large for the
   browser. This is the root of 1.
3. **Stand density is 1/voxel² and grid-dependent** (`seeds.rs:392` one stand per free face; `step.rs:672`
   equal heights never shade; `lib.rs:2475` initial_mineral per site, not per m²). 64 stands/m² at the
   live grid; foliage and mineral per m² quadruple from 0.25 m to 0.125 m.
4. **Induced defence is the only walk incentive and is 20–400× slow** (`lib.rs:639,641`): after ~9 min at
   one plant the bite yields ~7 %, and the plant stays defended for hours.
5. **No day/night in flora** (`step.rs:201`; `World::life_daylight` is read only by fauna): plants fix 24 h,
   ~1.5× more income than a 0.67-daylight day would give. WX3 is briefed (225bafbb), not implemented.
6. **f_max caps income per stand regardless of size, and mineral is read from the one site** (`step.rs:2449`,
   `lib.rs:1205`): vaulttree potential 0.0267 is capped at 0.01·N; at full size its tissue holds 0.45 of
   the site's 1.0 and litter ~0.1, leaving N≈0.45 ⇒ cap 0.0045 < full-size upkeep 0.0065. Derived, not
   run: the vaulttree likely cannot hold W_max.
7. **Canopy shade is 6–30× too thin in optical depth** (`lib.rs:2470`): one full vaulttree crown passes 87 %,
   bloomcrown 77 %; shade-tolerance traits (light_half 0.1) have nothing to act on. Backlog lists it
   known-wrong.
8. **Drowning is instant and counts transit water** (`step.rs:496`); stress rise τ 5 s. Cushion/turf die in
   2–3 cm; a 50–100 mm/h storm's sheet flow is the risk to check with the abiotic worker.

## INLINE literals and rules easy to miss

- `seeds.rs:797` runner neighbourhood dx,dz ∈ −1..1 and `support_near` ±1 voxel — **not scaled** by
  `for_voxel_size`, so runners travel 0.125 m on the live grid (half the authored 0.25 m).
- `cover.rs:79` FOUNDER_SPACING 8 voxels, unscaled. `seeds.rs:62,69,70` WIND_TRIES 4, Lomax shape 2,
  scale 2.5 hops.
- `step.rs:496` instant drowning; `step.rs:2449` income ignores whether transpiration was actually met
  (reads μ only); `step.rs:2428` mineral from own site only; `step.rs:1665` saturation counted per voxel.
- `step.rs:2455` income capped by what can be spent (unused light not fixed); `step.rs:2533` 20 % reserve
  share before foliage; `lib.rs:1998` lanternberry skirt cut at 0.45 with shares 3/14, 11/14 to put it in
  the band; stage thresholds and 0.125 m seedling caps in every profile.
- `layers.rs:181` shade area floor 0.0625 m²; `step.rs:672` strictly-higher occlusion.
- `seeds.rs:123` newborn wood = w_frac·seed (40 % wood, 40 % foliage, 20 % reserve).

## Unresolved

- Whether the live `sated_bite` makes the 6.7 % defended bite matter (fauna/harness scope).
- Band membership assumes the browser stands at the plant's face level; a browser one cell up/down shifts
  every threshold by 0.125 m.
- MSY assumes open sky, μ=1, N=1 and no defence; shade and drying lower it, but the orders of magnitude
  hold down to L·μ ≈ 0.5.
- Vaulttree mineral limitation is arithmetic from the code, not a run.

## Code shape (flora crate, src/ only; counts by a comment/test line classifier)

1. **Shares.** src/ 13,682 lines: doc-comment prose ≈ 3,485 (25 %), logic ≈ 7,410 (54 %), in-file tests ≈ 2,270
   (17 %), blank ≈ 5 %. lib.rs is 42 % prose (the nine presets `lib.rs:1167-2065` are ~60 % prose); cover.rs 80 %
   logic, no tests in-file. Integration tests in `tests/` add 16,303 lines (more than the whole of src/).
2. **Duplicated logic.** Rng/keyed/mix copied: flora `step.rs:137-170` = fauna `step.rs:101-125`. Vine root-water
   read `cover.rs:1182-1191` re-implements `step.rs:1633-1657` (drinkable_m3, mean_available). Vine income →
   upkeep → dieback `cover.rs:1290-1320` restates grow_stand's maintenance-from-income-then-reserve
   `step.rs:2481-2492`. One germination predicate with seven entry points (`step.rs:3037,3147,3155,3174,3187`,
   `lib.rs:3738,3751,3767`). Three validators by hand (`lib.rs:2084-2321` ~240 lines, `lib.rs:2672`,
   `cover.rs:876`). `lib.rs:3642` reachable_foliage (Reach box) vs fauna's mouth scan (`encounter.rs:124`) — two
   rules for "what a mouth reaches". Test-only reference copies: `moisture_of` `step.rs:1703`, `light_full_sweep`
   `step.rs:3546`, `drink_sorted_sweep` `step.rs:3743`.
3. **Dead / legacy carried.** substrate_* fields inert on Photo, light fields and assimilation inert on
   Saprotroph (`lib.rs:652-687`); glowcap stress rates inert on income (`lib.rs:1795`); seed_attrition_per_s is
   always 1/seed_max_age_s (`lib.rs:1197` + every preset); fixture switches "not a shipped configuration"
   `lib.rs:2536-2636` (one_layer_species, drop_seeds_checked_each_tick, without_defence, without_graze_refuge,
   minimum_seeds, crowns_scaled) used by round3/4/5 tests; Provision::Lazy|AtCreation `lib.rs:2343`;
   sprout_after_arming `lib.rs:2449`; Reach/reachable_foliage mostly for the harvest probe and old tests.
4. **Where parameters live.** ~60-field SpeciesConfig × 9 presets written as Rust struct literals; 10
   FloraConfig globals (`lib.rs:2470-2479`); 36-field VineConfig (`cover.rs:812-846`); CONSTs DEFENCE_* `lib.rs:639`,
   WIND_* `seeds.rs:62-70`, MIN_LAYER_AREA_M2 `layers.rs:181`, FOUNDER_SPACING `cover.rs:79`, REFERENCE_VOXEL_M
   `lib.rs:2495`; inline rules (runner ±1 voxel `seeds.rs:797`). Voxel rescaling is a hand-kept field list
   (`lib.rs:2509-2523`, `cover.rs:857`); ES multipliers a second list (`cubarium-search/.../ecology.rs:45`).
   One registry — `{id, per-species default, unit, range, scaling: voxel-distance|face-area|none, provenance}`
   loaded from a data file — would generate defaults, validation, voxel rescale and the ES vector from one table.
5. **Refactors that shrink most** (rough):
   a. Presets + validators as data under a parameter registry: −900..−1,100 Rust lines (lib.rs 1167-2321,
      cover.rs 790-960), prose moves to a species doc.
   b. Shared organism budget kernel (income → maintenance from income then reserve → construction 1+c_g →
      mineral fraction rule → energy/heat booking) for stands, vine and fauna bodies: −300..−500 across crates,
      and Photo/Saprotroph become two income functions feeding one kernel.
   c. Vine as a stand variant reusing root box, drink, moisture ramp, income and dieback: −600..−900 of cover.rs.
   d. One gate predicate taking an optional precomputed {sky, substrate}: −100; one crate-level Rng: −40.
   e. Fixture switches into test helpers or data overlays, derive attrition from age: −120.
