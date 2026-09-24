# Weather: temperature, a limited sky, three kinds of rain

Wrysk, 2026-09-24, after the 24 h terrarium runs: "we should have like maybe 3 rain
modes. drizzle, shower, and downpour, with downpour also showing as a storm with
thunder/lightning … the situation its seeing now should produce more heavy storms. there
definitely has to be an atmospheric water limit." He also wants temperature that varies
through the day and from day to day, with the temperature setting how much water the air
can hold. That way cold fronts bring rain, and cool, damp air brings fog or mist.
Day/night comes later. Local real weather is a someday idea. This is a proposal for a
new thread; nothing here is decided until Wrysk says so.

## Why now (measured, main 86e5ed6, eidolon, heuristic founders)

- The desktop terrarium over 24 h:
  - **Seed 2:** the air holds 1–3 m³ all day and the lake is steady at 46 m³.
  - **Seed 1:** it grew 12,000 plants. The water in the air rose from 3 to 36 m³ and
    the lake fell from 16.5 to 7.9 m³, still falling at hour 19. Water in the soil barely
    moved (47 → 42 m³).
- **Cause:** each shower releases at most `shower_volume_m3` (1–3 m³ in the terrarium
  presets; `crates/cubarium-voxel/src/water.rs:700`), at a fixed rate R₀ = 3.5e-5 m/s
  on the cells open to the sky, with 5–15 min between showers. How much the plants give
  off grows as they spread; how much rain can return does not. Nothing limits how much
  the air holds, so once there are enough plants, the air keeps the lake.
- The existing weather is `water::shower` (a store, a floor, a calendar) plus
  `water::evaporate` (a fixed rate from open water). The earlier water-cycle brief
  (`voxel-water-cycle-2026-09-20.md`) chose one shared air store now and clouds with
  positions later (route C); this stays within that choice.

## Proposed model (still one shared air store, no spatial clouds)

**Temperature** `T(t)`, °C (only the differences matter):
`T = T_base + diurnal + drift + fronts`
- **diurnal:** ±4 °C. Coolest at dawn, warmest about two-thirds of the way through the
  day. Follows whatever day length is chosen (see *Decisions*).
- **drift:** day-to-day wander, standard deviation 2.5 °C, lasting about 3 days
  (a mean-reverting random walk, in days).
- **fronts:** about 0.7 per day. Each drops the temperature by 3–8 °C over ~30 min, then
  recovers with a time constant of ~0.3 day.
- Drawn from the world's weather stream (`WEATHER_STREAM`), so a world's weather follows
  from its seed, as the showers do today.

**Capacity:** `C(T) = C_ref · e^{0.07 (T − T_ref)}`. That is the Clausius–Clapeyron
slope: 7 % per °C, so 10 °C of cooling halves what the air holds.
- `C_ref` is a share of the world's total water (default 0.03). It works at any world
  size and preset, and a dry world gets a small sky.
- In seed 2 the air sits at 1–3 m³ of ~100 m³ total. With 0.03, today's healthy worlds
  would sit at a relative humidity of 0.4–0.9.
- **Relative humidity** `RH = atmosphere_m3 / C(T)`.

**The limit on the air's water:**
- **Soft:** evaporation from open water scales with `max(0, 1 − RH)` and with
  `C(T)/C_ref`, so warm, dry air draws water up fast and saturated air draws none.
  Normalised so that at `T_ref` and `RH = 0.6` it equals today's rate.
- **Hard:** `RH` never passes 1.5. Reaching 1.5 starts a downpour at once, ignoring the
  dry gap.
- Transpiration stays as it is in WX1: the plant model is not touched. Tying it to
  humidity is a later option.

**Rain events.** Checked every sim-minute outside an event and outside the dry gap.
Each event's water is a share of the store, which is the budget:

| mode | when it starts | rate | how much falls | lasts |
|---|---|---|---|---|
| fog / mist | RH ≥ 0.9, no rain, temperature near the dawn low or falling | none (visual; evaporation is already ~0) | — | until RH < 0.85 or the air warms |
| drizzle | 0.9 ≤ RH < 1.05 and temperature falling; chance per minute ∝ RH − 0.9 | 0.1 R₀ | until RH is back to 0.8 | 20–90 min |
| shower | RH ≥ 1.0 | R₀ | until RH is back to 0.7 | 5–15 min |
| downpour (storm) | RH ≥ 1.25; or a front passing with RH ≥ 1.1; or near the afternoon peak with RH ≥ 1.15; or RH ≥ 1.5 (forced) | 2.5 R₀ | until RH is back to 0.5 | 10–30 min |

- **Dry gap:** after each event, 5–15 min drawn at random. Wrysk's 2026-09-21 shower
  cadence becomes the minimum spacing instead of the trigger.
- **Lightning** only in downpours: random strikes, about one every 30 s (10–60 s),
  each a view event with a tick and a column. Thunder has no sound on this display, so
  what it looks like (a flash, a delayed rumble shake) is an art question.
- **Rain placement:** still even over every cell open to the sky. The "reserve, spread,
  refund" delivery and the ledger stay exactly as they are.
- **Seed flush:** the seed bank's end-of-shower flush fires at the end of every rain
  event.

**What this should produce** (a design goal to calibrate against, not a hard rule):
- A young world, with little rising off its plants and pools: drizzle and a shower
  every hour or two, fog at dawn.
- A lush world like seed 1: showers every ~40–60 min and a storm every few hours.
- A warm spell: dry days with the air filling up. The cold front that ends it: a storm.
- Droughts come from warm spells, not from water stuck in the sky (the "drought lock is
  a feature" note in the 2026-09-20 hydrology decisions still holds for other causes).

## What the view gains (dev mode only, until Wrysk decides the look)

`WeatherView { temperature_c, rh, mode, rain_rate, fog_density, cloud_cover,
day_phase, lightning: Option<Strike> }`.
- `cloud_cover = clamp((RH − 0.4)/0.6)`, the "cloud band from `atmosphere_m3`" the
  environment render plan asked for (`environment-render-plan-2026-09-23.md` §3).
- Dev-mode visuals are **interim** and named as interim:
  - rain streaks thinned or thickened by the rate;
  - the sky dimmed in a downpour;
  - one bright frame per lightning strike;
  - a grey haze near the ground for fog.
- The real look belongs to the art direction and to Wrysk (the presentation plan's L
  package already has a sun-direction setting that `day_phase` can drive later).

## Packages

- **WX1: weather model** (cubarium-voxel; a new `weather.rs` beside `water.rs`, the
  config and recipe keys, the terrarium/natural/Tachyon presets, the sim system):
  - temperature, capacity, the soft and hard limits, the four modes with their budgets,
    the dry gap, lightning events;
  - ledger counts per mode and a `WeatherView`;
  - census columns `temp_c, rh, mode, rain_in, drizzles, showers, downpours, strikes`;
  - snapshot schema bump; fresh worlds only.
  - Model-rule change: tests come from the list below, written before the code.
- **WX2: interim dev-mode weather drawing** (the presenter; interim visuals as listed
  above). Small. Can start when WX1's `WeatherView` exists.
- **WX3: day/night light for plants** (the plant model). Night light is 0, so
  photosynthesis stops. **Gated:** it roughly halves or thirds how much plants make in
  a day and moves the balance just reached. Needs Wrysk's go and a re-check of
  colonisation.
- **WX4: local weather feed** (someday). A `WeatherSource` interface: the built-in
  generator (default), or a feed that sets temperature and turns real rain into a mode.
  Only worth doing once there are enough species for different climates.

**Tests for WX1** (tiny, a few ticks each):
- capacity halves with ~10 °C of cooling;
- `RH ≥ 1` outside the dry gap starts a shower, and `RH ≥ 1.5` starts a downpour even
  inside the gap;
- an event stops at its target RH;
- lightning happens only in downpours;
- a cold front raises RH at a fixed store;
- evaporation is 0 at `RH ≥ 1`;
- the diurnal low is at dawn;
- the water ledger's residual is 0 through a full storm.

**Acceptance (WX1)**, eidolon (`scripts/voxel-remote-ship.sh` pattern, znver5, pinned
by physical core):
- terrarium seeds 1 and 2 for 24 h, and the default preset for 4 h;
- the air never passes 1.5 × capacity; the lake stays within ±15 % after hour 2;
- seed 1 gets storms;
- stand counts at 4 h within ~20 % of today's (default ~1,750–2,000, terrarium
  8,500 / 5,300);
- report counts per mode per 24 h.

## Decisions for Wrysk (asked up front; defaults in brackets)

1. **Day length.** Real local day and night, 2 h, or 1.5 h (1 h of day, 30 min of night)?
   [A setting from the start. Default 2 h with ⅔ daylight; `day = "local"` with
   latitude/longitude later. That mode is the one place a wall clock enters the sim,
   and it is fine because no bit-identical runs are required.]
2. **Does night stop photosynthesis (WX3)?** [Yes, as its own package after WX1, with a
   colonisation re-check.]
3. **Does temperature act on organisms directly** (growth, animal activity)? [No, not
   now. Animals take no new inputs, which would change the training contract.]
4. **The 5–15 min cadence becomes the dry gap, not the trigger.** [Yes.]
5. **Storm look:** what a downpour, lightning and fog look like, beyond the interim
   dev-mode versions. [Wrysk's art thread.]

## Not in this thread

Clouds with positions and rain that falls unevenly (route C); wind tied to the weather
(the wind packet stays as it is); snow and frost; sound; new inputs for animals.
