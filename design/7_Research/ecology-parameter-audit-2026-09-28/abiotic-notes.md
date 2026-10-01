<!-- Appendix to design/7_Research/ecology-parameter-audit-2026-09-28.md: the abiotic inventory's working notes, written by an audit worker at 3ccc6daf. Rows are in parameters.tsv (subsystem = abiotic). Evidence, not decisions. -->

# Abiotic inventory (soil, water, weather, day clock, light) — pinned 3ccc6daf

Table: `abiotic.tsv` (85 rows). Evolve vocabulary extension: `yes lin[a,b]` for fractions and offsets
(the brief's `log` is for scale parameters). Scope: `crates/cubarium-voxel/src`, `crates/cubarium-rules/src/water`,
`config/**`. Shipped worlds: desktop/voidrunner `terrarium` (256x128x48 at 0.125 m = 32 x 16 x 6 m, footprint 192 m2),
tachyon `terrarium-small` (160x72x24, 60 m2); `desktop/voxel.toml` runs the `default` staged ring; `tachyon/natural.toml`
(`small` ring) is not installed. All shipped configs use `tropical_jungle`, `[world.day]` cycle 120 min / 0.67 / 8.0, and
override no climate key (they are commented out). `config/training/*.toml` hold only flora/founder multipliers (harness).
No abiotic parameter is in the ES vector (`ecology.rs` PARAMS): 0 of 85 rows are ES-evolvable today.

## Rules in plain words

**Tick (water.rs:563-590).** Each tick: (1) `weather::step` (clock, temperature, rain events); (2) evaporate; (3) four
substeps of infiltrate -> fall -> exchange; (4) drain -> water table -> spring -> outlet/lake drain. Ledger is exact
(closed budget: nothing leaves).

**Soil.** Voxel material is Air / Bedrock / Rock / Soil. Soil: pore 0.35 of volume, field capacity FC 0.65 of pore
(0.2275 vol), wilting point WP 0.28 of pore (0.098 vol), so plant-available water = 0.1295 vol = 129.5 mm per m of depth.
Flux across a face per step = K*A*dt (K = `permeability*pore*0.25` = 0.0175 m/s soil, 5e-6 m/s rock). Infiltration: free
water on a soil face enters it at K. Drainage: pore water above FC moves down at K (donor's K). Water table: every
permeable voxel whose centre is at or under the aquifer head fills to saturation from the aquifer at K; drainage skips it.
Plants can draw only between WP and FC (flora side).

**Rain.** Only in `closed` worlds (all shipped). Rain falls at `mode.rate_r0 * R0` on every sky-exposed column (a roof
shadows what is under it), taken from a single lumped **atmosphere store**. R0 = 3.5e-5 m/s = 126 mm/h.
Air capacity `C(T) = 0.03 * total_water * e^{0.07 (T-26)}` (desk ~6.5 m3 = 34 mm). `RH = store/C(T)`;
`M = store/C(26 degC)` (moisture, temperature-free) is what showers and downpours read.
Once per sim minute, if no event runs, each mode m has hazard
`lambda_m = lambda_max * weather_speed * sigma((H - rh50)/width) * max(0, 1 + g_aft*A + g_cool*K + g_front*F)` per hour
(`H` = M for shower/downpour, RH for drizzle; A = afternoon Gaussian sd 0.08 cycle at the temperature peak; K = cooling
rate; F = fresh front). p = 1-exp(-lambda/60); one draw picks at most one mode; a dry gap of 300-900 s after each event blocks
starts. An event ends when H falls to `stop_rh`, when the store is empty, or at `longest_min/weather_speed`. RH >= 1.5
forces a downpour. Water returns to the store from open-water evaporation and plant transpiration (and the outlet if any).
**Evaporation** is open-water cells only: `E*(1-RH)/0.4*e^{0.07 dT}` with E = 1e-5 m/s at RH 0.6; no soil evaporation and no
canopy interception exist.

**Groundwater loop.** The terrarium has no outlet. Water table = lake level (hydrate raises the aquifer head to the lake
surface); the lake floor exchanges with the aquifer at `k*(lake surface - table)`; a stream is pumped from the aquifer at the
spring cell (8 L/s desk, swell to 5x in a downpour, 45 s rise / 50 s fall) and returns through the lake drain. This loop
does not touch the atmosphere store.

**Clock and light.** Day 120 min, sun up 0.67 (80.4 min, sunrise 03:58 sunset 20:02 on the day clock), sun elevation a sine
peaking at 70 deg; `daylight` = smoothstep over +-6 deg. Temperature = 26 + 3*diurnal(-1 at sunrise, +1 two thirds through
the day) + drift + fronts + rain cooling. What life actually reads at 3ccc6daf: **fauna** read `life_daylight` (WX4: diurnal
awake above 0.3; sleep upkeep 0.5x); **flora** read `is_raining` (seed flush) and geometric `sky_visibility` (17-ray
cosine fan) times canopy shade. Nothing reads temperature or RH. WX3 (plants earn only by daylight; upkeep Q10 = 2.0) exists on
unmerged branch `worktree-agent-ae2a0976ade4bcce0` (commits 1812b4c6, 55d5df6b, c481dec2) - **mid-change, not in the audited tree**.

## Derived quantities (arithmetic)

Constants: voxel 0.125 m, A = 0.015625 m2, V = 1.953e-3 m3, dt = 0.05 s.
- K_soil = 0.2 * 0.35 * 0.25 = 0.0175 m/s = 63 m/h. Real Ks: sand 1e-5-1e-4, loam 1e-6-1e-5. Ratios 175 (sand top), 1,750 (loam top),
  17,500 (loam bottom). After K_day 12: 15x / 146x / 1,460x.
- K_soil / R0 = 0.0175/3.5e-5 = 500 (250 vs a downpour at 7e-5). Rain on soil never runs off; only rock (5e-6 m/s = 18 mm/h) sheds.
- Drainage: per-tick face flux = 0.0175*0.015625*0.05 = 1.37e-5 m3 = 2.0 % of a voxel's pore capacity (6.84e-4 m3). Saturated -> FC
  excess = 0.35*6.84e-4 = 2.39e-4 m3 -> 17.5 ticks = 0.875 s. Real: 1-3 days (K_day 12 -> 2-6 h = 7,200-21,600 s): ~8,000-25,000x fast.
- Rain per event (desk, C = 6.5 m3, footprint 192 m2 = 33.9 mm): drizzle (0.98-0.8) => 1.17 m3 = 6.1 mm at 12.6 mm/h, but capped by
  90/5.03 = 17.9 min => 3.8 mm; shower (1.0-0.7)*6.5 = 1.95 m3 = 10.2 mm at 126 mm/h => 4.8 min (cap 6 min = 12.6 mm);
  downpour (1.13-0.5)*6.5 = 4.09 m3 = 21.3 mm at 252 mm/h => 5.1 min (cap 10 min = 42 mm). Sky-exposed area < footprint, so mm on open
  ground is somewhat more.
- Rain per sim day (ESTIMATE from WX1 targets, not run): 2-3 showers * 10.2 + 0.5-1 downpour * 21.3 = ~35-50 mm; use 30-50 mm.
  A sim day stands for a real day (K_day), so 30-50 mm/day = 11-18 m/yr vs tropical 2-4 m/yr: 4-6x. Sky turnover 34 mm / 40 mm/day
  = 0.85 sim day vs real 5-8 days: K ~ 6-9.
- Hazard at M = 1.0 in the afternoon: shower 1.5*5.03*0.5*(1+4) = 18.9/h -> 27 % per minute; the dry gap (5-15 min) then paces events.
- Open-water evaporation: E = 1e-5 m/s = 36 mm/h = 864 mm per 24 h = 72 mm per 2 h sim day. Real 5-6 mm/day (6.9e-8 m/s) =>
  145x raw, 12x after K_day. Default-ring E = 3e-5 (3x worse). With the lake at maybe 5-10 % of the footprint (rough) lake evaporation
  contributes 3.6-7 mm/day over the footprint; the rest of a 30-50 mm/day rain budget must come from transpiration.
- Transpiration cross-check (flora scope): 1e-6 m3/s per foliage unit * 7,200 s = 7.2 L per sim day; over a 3x3 root footprint
  (9*0.015625 = 0.1406 m2) = 51 mm/day. Vs real 2-5 mm/day: 10-25x. Root box 3x3x2 soil voxels = 18*2.53e-4 = 4.55e-3 m3 of available
  water; at 1e-6 m3/s it lasts 4,553 s = 76 sim-min (0.63 sim day) from FC to WP.
- Showers a root layer needs: 0.25 m layer = 0.1295*0.25 = 32.4 mm to refill from WP; shower 10.2 mm => 3.2 showers (31.5 % each); downpour
  21.3 mm => 1.5 (65 % each). Grove 0.75 m layer = 97 mm => ~9.5 showers.
- Night: 0.33*120 = 39.6 min = 2,376 s = 7.9 h-equivalent; daylight > 0.3 for 81.6 min (68 % of the cycle); twilight band (0.05-0.95) 6.4 min.
  Base plant reserve = reserve_cap 0.5 / maintenance 0.0002 = 2,500 s => night = 0.95 of it. Animal reserve_cap 0.5 body / 0.001 per s = 500 s
  (1,000 s asleep at 0.5x): night = 4.75x / 2.4x. So animals must be fed at dusk and sleep through it; plants would just clear it (base).
- Weather speed: (1440/120)^0.65 = 5.03 vs K_day 12. Aquifer: 192*0.35 = 67.2 m3 per m of head; total water ~217 m3 (6.5/0.03) = 1.13 m.
- Stream: 8e-3 m3/s = 28.8 m3/h = 150 mm/h over the footprint; peak 40 L/s. Lake drain 0.8 m2/s: 8 L/s needs only 1 cm of head difference.

## Top findings (ranked)

1. **Soil is ~10^3-10^4 x too permeable, and that removes runoff and ponding.** K = 0.0175 m/s (material.rs:354) vs loam 1e-6-1e-5 m/s;
   infiltration capacity is 500x a shower (250x a downpour), and a saturated voxel drains to FC in 0.875 s. Consequences: water never
   sheets off soil, never perches, and "wet" ground is only where the table or a rock ledge is. Field capacity/wilting (0.2275 / 0.098 vol)
   are correct; the rate is not. (Rows: water.material.soil.conductivity_m_per_s, water.derived.*.)
2. **Plants and the water budget are locked together: rain equals plant transpiration + lake evaporation.** Closed budget; the sky holds one
   downpour (6.5 m3 = 34 mm). Demand at foliage 1 is 51 mm/sim-day per 3x3 footprint (10-25x real) against a ~30-50 mm/day rain (4-6x real):
   the flora doc itself says a full bloomcrown asks ~2x the mean rain on its footprint. Any change to transpiration changes rain frequency.
   (water.rain.per_day is an estimate; no run made.)
3. **Evaporation is 12x hot after the day clock (145x raw) and exists only over open water.** E = 1e-5 m/s (terrarium) is 72 mm per sim-day
   against 5-6 mm real; there is no soil evaporation and no interception, so all ground moisture leaves through plants, drainage or the aquifer.
4. **Day/night and temperature barely bind life on the audited tree.** Plants ignore daylight (WX3 unmerged), nothing reads temperature (Q10 2.0 is
   on the same unmerged branch) or RH. Night (39.6 min = 2,376 s) is 4.75x an animal's reserve at cap (500 s), so circadian sleep is the only
   thing that lets animals bridge it; +-3 degC is a +-23 % Q10 factor nobody applies.
5. **Rain intensity and event depth are the well-grounded parts** (R0/5 = 25 mm/h, downpour 50 mm/h, 10 / 21 mm per event, capacity 34 mm vs
   tropical precipitable water 40-60 mm, CC 7 %/K, FC/WP/porosity within 2x of loam). Frequency is the stretch: 2-3 showers per sim day, from
   `weather_speed` 5 not 12 and a fixed 5-15 min dry gap (not scaled).
6. **Roofed ground gets no rain.** Rain lands on the top sky-exposed cell only; halls and shelves are dry except for runoff (soil takes 500x rain so
   runoff is nil) and the table. Rock ledges and lake-bed rock are the only sheds. Grove sites are chosen open-above, so trees are fine; understorey
   under shelves depends on the water table alone.
7. **Water-table geometry sets who is wet.** Table = lake level (~1.5-1.75 m above y=0 on the desk ring); yard ground 1.25 m, rising 0.06 voxels per
   column away from the lake and 0.5 m to the back wall; storeys 2.75-3.5 m above. Terraces are dry (FC only); the lakeside can be saturated.
   `establish_saturated_max`, `drown_depth_m` (flora) and STANDING_SUPPORT_FILL (0.5) decide the outcome there.
8. **Undocumented drift between handoff and code:** the weather handoff table says diurnal 4, drizzle rh50 0.93, shower lambda 2.0, downpour 2.5 R0
   and gain 2.0; the code has 3.0, 0.98, 1.5, 2.0 R0 and 4.0 (WX1b re-calibration). Treat the code as truth; the handoff is stale.

## INLINE literals and consts worth a name

- `material.rs`: pore 0.35 / 0.02, permeability 0.2 / 0.001, FC 0.65 / 0.5, WP 0.28 / 0.2 (match arms, no config path), `REFERENCE_VOXEL_M` 0.25.
- `water.rs`: `MIN_SPREAD_DEPTH_M` 1.25e-7, `OUTLET_WEIR` 0.05 (and free^1.5), `SERIAL_BELOW_COLUMNS` 512, substep count 4 (config).
- `cubarium-rules/water/mod.rs:43-52`: `FLOW_PER_SUBSTEP` 0.5, `HEAD_PASSES` 4, `ROOM_EPS` 1e-12, `MASK_ROWS` 128.
- `weather.rs`: `CC_PER_C` 0.07, `RH_MAX` 1.5, `EVAPORATION_RH_SPAN` 0.4, `STREAM_FLOOR_RH` 0.5, `NOON_ELEVATION_DEG` 70, `TWILIGHT_DEG` 6,
  `SWELL_RISE_S/FALL_S` 45/50, `STRIKE_COLUMNS` 4, `AUTO_SPEED_EXPONENT` 0.65, `TICKS_PER_MIN`; inline 2/3 temperature peak (:464,:472), `/60` hazard
  conversion, `0.4/0.6` cloud cover, `cloud_floor` 0.6/0.75/0.9 (visual).
- `hydrate.rs`: `FOUNDING_RH` 0.6, `LAKE_TABLE_SHARE` 0.85, `RING_STREAM_FLOOR_M` 0.25, `POOL_MIN_FILL` 0.05, `MIN_POOL_CELLS` 4, `LAKE_FLOOR_BAND` 2.
- `world.rs`: `STANDING_SUPPORT_FILL` 0.5, ray fan weights (1, sin60, sin30) and quarter-voxel step (:11-60, :604), `SOLID_MASK_ROWS` 128.
- `terrarium.rs`: bedrock `y < 2` and lake bed `y > top-2` (:1011-1014), the `level()` rule (6 of 8 neighbours, :1637-1650), hardcoded stream `SLOPE 0.2`,
  `CLIMB/FALL/SIDE` (1272-1274) - terrain shape, not counted.
- Fauna (cross-ref): `Activity::DAY_LINE` 0.3, `TWILIGHT` (0.05, 0.95) (fauna lib.rs:514-516), `SLEEP_UPKEEP` 0.5.

## Terrain shape (one line)

Terrarium and staged-ring landform fields (crest, storeys, treads, risers, shelves, halls, arches, spires, erosion, seeds; ~70 in
`terrarium.rs:59-176`, `recipe.rs:840-1040`) are shape only. The conditions for life extracted above are: soil depth 0.25 m (0.75 m groves), table at lake
level, yard height, lake area, and the bedrock/rock-bed rows.

## Not resolved / caveats

- Rain per sim-day and the ET split are estimates from design targets; I did not run anything. Actual measured rain by mode is in `--weather-log` runs, not read.
- Founded total water (217 m3) is taken from the handoff's 6.5 m3 / 0.03 for the desk; the tachyon ring's absolute numbers are not derived (same ratios apply).
- Physical mass of a foliage unit is flora's; the 51 mm/sim-day transpiration depth depends on the footprint a unit foliage occupies.
- `weather.rs` doc comment (:872-885 area) is stale on nothing found; `WeatherView::legacy` (weather.rs:101) has no callers.
- Working tree looked clean at audit start; WX3/Q10 is on a side branch, so flora/abiotic coupling may change soon.

## Code shape

- **Duplicated exchange solver.** `water.rs:1427-1851` `exchange_dense` (+ helpers `offer_up_the_run`, `offer_from`, `scan_column(s)` :1862-2000, `Scratch` "dense fallback only" fields :1051-1060)
  re-implements what `cubarium-rules/src/water/{mod,host}.rs` (1,450 lines) does for worlds taller than `MASK_ROWS = 128`. It runs only when `height > 128`
  (`exchange_inner` :1270); every shipped world is <= 128 (desk terrarium is exactly 128). It survives as the reference in 2 tests (:3204, :3292). ~600 lines.
- **Column phases wrapped twice.** `water.rs:838-967` `column_phase` + `grounds()` :771 adapt `cubarium-rules/.../column.rs` (547 lines); the `Material` -> `Ground` map is
  the only glue. Fine, but `pore_flux_m3` (:239) and `Material::conductivity_m_per_s` both encode K.
- **Legacy/dead:** `WeatherView::legacy` (weather.rs:101-130, zero callers); open-budget prescribed rain (`prescribed_rain` water.rs:606, weather.rs:1340 else-branch,
  `Config.rain_m_per_s/evaporation_m_per_s`) survives for fixtures and the training arenas only; `Landform::Ridge` (54 non-test references);
  `spring_k_m2_per_s`/`outlet_m3_per_s` are 0/absent in every terrarium; stale docs cite removed `shower_volume_m3` (viability.rs:59).
- **Parameters buried:** material constants live in `match` arms (material.rs:36-108) with no `Config`/TOML path, yet they set K, FC, WP; four water knobs are triplicated
  (`Config` fields, `Water` recipe fields, `Terrarium.water` overrides, copied by `cycle_into`, recipe.rs:804) with layered `..Water::DEFAULT`/`SMALL` inheritance
  (recipe.rs:736-771, terrarium.rs:239-247/311-319).
- **Refactors that would simplify most:** (1) delete `exchange_dense` and the dense-only Scratch fields by making the rules crate handle any height (~ -600 lines,
  moves ~40 lines into rules); (2) one `SoilSpec {pore, k_m_s, fc, wp}` struct per material read from `Config` (and TOML), replacing the 4 match arms and the
  `REFERENCE_VOXEL_M` conversion (~ -60 lines, +40; makes 8 soil numbers evolvable); (3) collapse `Config` water fields into one `Water` value and drop
  `cycle_into`/`Water::DRY` duplicate defaults plus `WeatherView::legacy` and the open-budget rain branch (~ -150 lines).
