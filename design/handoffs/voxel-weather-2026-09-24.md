# Weather: a day clock, a climate, three kinds of rain

Wrysk, 2026-09-24, after the 24 h terrarium runs: "we should have like maybe 3 rain
modes. drizzle, shower, and downpour, with downpour also showing as a storm with
thunder/lightning … the situation its seeing now should produce more heavy storms. there
definitely has to be an atmospheric water limit." He also wants temperature that varies
through the day and from day to day, setting how much water the air can hold, so that
cold fronts bring rain and cool, damp air brings fog or mist. Local real weather is a
someday idea.

**Approved by Wrysk the same day ("sounds good"); WX1 and WX2 are dispatched.** His
answers are recorded in the next section; the rest of the model is Fable's routine call
and he can veto any of it.

## Wrysk's decisions (2026-09-24)

1. **The day clock is a toml setting** with three modes: a cycle (its length and its share
   of daylight), synced to local time, or off ("always day or night, but only affects
   visuals").
2. **The 5–15 min shower cadence becomes a minimum dry gap**, not the trigger. He expects
   we may drop it: "i want metrics on how often its triggered vs a natural rain event
   occurs."
3. **Night stops photosynthesis** (WX3).
4. **Animals get circadian rhythms**, with restful and active phases (WX4).
5. **Climate presets** set temperature and wetness. One is needed now: the
   **tropical jungle** the project has been assuming.
6. **Which kind of rain falls is a random draw** each interval, weighted by humidity,
   temperature, time of day and so on, "according to the climate preset and estimates
   from nature / what works for our ecology".
7. **The placeholder visuals get real effort**: "dont be afraid to use shaders" (WX2).
8. **On a short day, weather runs faster than nature, but not 12× faster** (later the
   same day): "10-20 minutes could be long … for a 2hr cycle, we might want weather
   conditions to cycle relatively faster (maybe not 12x faster but something)". See
   *Weather speed*.
9. **Storms that make streams** are an exploratory notion, "unless theres a viable path".
   See *Storm streams* under Packages.

## Why (measured, main 86e5ed6, eidolon, heuristic founders)

- The desktop terrarium over 24 h:
  - **Seed 2:** the air holds 1–3 m³ all day and the lake is steady at 46 m³.
  - **Seed 1:** it grew 12,000 plants. The air rose from 3 to 42 m³ and the lake fell
    from 15.7 to 6.6 m³ by hour 22 (census `atmosphere`, `lake_m3`).
- **Cause:** a shower releases at most `shower_volume_m3`, at a fixed rate R₀ = 3.5e-5
  m/s, every 5–15 min (`water::shower`, `crates/cubarium-voxel/src/water.rs`). What the
  plants give off grows as they spread; what rain can return does not, and nothing limits
  the air. The earlier water-cycle brief (`voxel-water-cycle-2026-09-20.md`) chose one
  lumped air store now and clouds with positions later (route C). This stays within
  that.

## The model (WX1)

Still one lumped air store; no clouds with positions. Every number below is a
**starting value** in the `tropical_jungle` preset, calibrated in WX1 against the
targets at the end of this section.

### Day clock: `[world.day]`

```toml
[world.day]
mode = "cycle"      # "cycle" | "local" | "off"
length_min = 120    # cycle: one day and night, minutes of simulated time
daylight = 0.67     # cycle: the share of it the sun is up
start_hour = 8.0    # cycle: where a fresh world starts, on a 24-hour clock of its day
# local: the wall clock and today's sunrise and sunset at this place
# (06:00 and 18:00 when these are absent)
latitude = 47.6
longitude = -122.3
# off: no night in the simulation (plants always lit, animals never sleep)
look = "day"        # off only: "day" or "night", the picture alone
```

- `day_phase`: 0 at midnight, 0.5 at noon. In a cycle, the sun rises at
  `0.5 − daylight/2` and sets at `0.5 + daylight/2`.
- The sun's elevation is a sine that peaks at 70° at noon and crosses zero at sunrise
  and sunset (negative through the night). `daylight` is a smoothstep of the elevation
  over −6°..+6° (civil twilight).
- `local` is the one place a wall clock enters the simulation. That is fine because no
  bit-identical runs are required. Sunrise and sunset come from the standard NOAA solar
  approximation; the season changes the day length.
- `off`: the simulation's light is 1 at all times, and the diurnal temperature term is
  zero. The view shows a fixed noon (`look = "day"`) or midnight (`"night"`).
- For WX3 and WX4, the world exposes `World::life_daylight() -> f64` (0..1; always 1 with
  the clock off) and `World::day_phase() -> f64`.

### Climate presets: `[world.climate]`

```toml
[world.climate]
preset = "tropical_jungle"   # the only preset for now
# any preset key can be overridden here, e.g.
# temperature_c = 27.0
```

- A preset is a named struct of the keys below. Overrides replace single keys.
- The preset sets temperature and wetness. The landform preset still sets how much water
  the world has.

### Temperature

`T = temperature_c + diurnal + drift + front + rain_cooling`, updated every tick.

| term | tropical_jungle | nature, lowland rainforest |
|---|---|---|
| mean `temperature_c` | 26 °C | 25–27 °C, almost no seasons |
| `diurnal_c` (±) | 4 °C: coolest at sunrise, warmest ⅔ of the way through the daylight | daily range 6–10 °C, low at dawn, peak mid-afternoon |
| drift | mean-reverting random walk: `drift_sd_c` 1.0 over `drift_days` 3 (days = day cycles) | day-to-day spread is small in the tropics |
| fronts | `fronts_per_day` 0.1; each cools by 2–4 °C over ~30 min and recovers over `front_recovery_days` 0.3 | tropical "cold surges" are rare and mild |
| rain cooling | the temperature relaxes toward −0.5 / −1.5 / −3 °C (drizzle / shower / downpour) while it rains, with a 20 min time constant, and back afterwards | rain-cooled air under storms |

Drawn from the world's weather stream (`WEATHER_STREAM`, keyed by seed and minute), so a
world's weather follows from its seed.

### Capacity, humidity, evaporation

- **Capacity:** `C(T) = capacity_share · expected_total · e^{0.07 (T − temperature_c)}`,
  the Clausius–Clapeyron slope: 7 % per °C, so 10 °C of cooling halves what the air
  holds. `capacity_share` is 0.03 of the world's water. It works at any world size, and a
  dry world gets a small sky. (Terrarium: about 6.5 m³.)
- **Relative humidity:** `RH = atmosphere_m3 / C(T)`.
- **Soft limit:** evaporation from open water scales with `max(0, 1 − RH) / 0.4` and with
  `C(T)/C(temperature_c)`. That is today's rate at the mean temperature and RH 0.6, and
  zero at RH ≥ 1.
- **Hard limit:** RH never passes 1.5. Reaching it starts a **forced** downpour at once,
  inside the dry gap or not, and turns an event already running into a downpour.
- Transpiration is unchanged; the plant model is not touched in WX1.

### Rain events: a random draw each minute

Once per simulated minute, when no event is running, each mode `m` has a hazard, in
events per simulated hour:

`λ_m = λmax_m · σ((RH − rh50_m) / width_m) · max(0, 1 + g_aft_m·A + g_cool_m·K + g_front_m·F)`

- `σ` is the logistic function.
- `A` is the afternoon bump (convection): 1 at the temperature peak, a Gaussian 0.08 of
  the cycle wide.
- `K` is how fast the air is cooling, in °C per simulated hour (0 while warming, capped
  at 3).
- `F` is 1 for the first 30 min after a front arrives, else 0.
- The minute's draw treats the three as competing: one uniform draw, and each mode starts
  with probability `1 − e^{−λ_m/60}`.
- An event rains at its mode's rate. It ends when RH falls to its stop value or its
  longest duration is reached, whichever comes first.
- One event at a time. No escalation, except the forced downpour at RH 1.5.

| mode | λmax /h | rh50 | width | g_aft | g_cool | g_front | rate (× R₀) | stops at RH | longest |
|---|---|---|---|---|---|---|---|---|---|
| drizzle | 1.5 | 0.93 | 0.03 | −0.8 | 1.0 | 0.5 | 0.1 | 0.8 | 90 min |
| shower | 2.0 | 1.00 | 0.04 | 1.0 | 0.5 | 1.0 | 1.0 | 0.7 | 30 min |
| downpour | 1.5 | 1.15 | 0.04 | 3.0 | 0.5 | 3.0 | 2.5 (≥ 2) | 0.5 | 50 min |

- **Storms stay short and intense.** At a terrarium capacity of ~6.5 m³ and 2.5 R₀
  (0.4 m³/min per R₀ on 192 m²), a downpour empties 1.25 → 0.5 in about 5 min. That is
  about right for a 2 h day (see *Weather speed*). Keep the downpour at ≥ 2 R₀; do not
  lower it to stretch storms. Heavy, short storms are also what could feed streams.
- The "longest" column is in **nature minutes**, divided by the weather speed: 18, 6 and
  10 min on the default 2 h day.
- Placement is unchanged: even over every cell open to the sky, with the existing
  reserve, spread and refund, and the ledger exact.
- The seed bank's end-of-shower flush fires at the end of every event.

### Weather speed

`weather_speed` in `[world.climate]`, default `"auto"`: `(24 h / cycle length)^0.65`.
That is **5× nature on a 2 h day**, and 1 on a 24 h cycle and in `local` mode (`off`
uses `length_min`).

- **Divided by it** (nature values in the preset): the events' longest durations, the
  fog and rain-cooling time constants, a front's ramp and recovery, the drift's time
  scale, and the 30 min front window. The hazards' `λmax` are multiplied by it.
- **Tied to the cycle** (fully compressed): the diurnal temperature, the afternoon bump,
  the dawn fog window.
- **Not scaled:** the dry gap (Wrysk's number, being measured), the lightning interval
  (a visual rhythm), and the rain rates (physical).
- Targets on the 2 h day: downpour 5–10 min, shower 2–6 min, drizzle 5–20 min.

### The dry gap, and the metrics to judge it

- `dry_gap_min_s` 300 and `dry_gap_max_s` 900 (in `[world.climate]`); 0 / 0 turns it off.
  After an event ends, a gap is drawn in that range.
- **Inside the gap the minute's draw still happens.** A draw that would have started an
  event counts as `gap_blocked` (per mode) and starts nothing. A forced downpour ignores
  the gap and counts as `forced`.
- The per-event log records the time since the previous event ended. From the gap-off
  arms we can then read how often nature alone restarts rain within 15 min.

### Fog

A state, not an event, and visual only (no effect on the simulation).

- **Forms** when RH ≥ 0.9, no shower or downpour is falling, and either the phase is in
  the dawn window (from 0.1 of the cycle before sunrise to 0.06 after) or the air is
  cooling faster than 0.5 °C per simulated hour.
- **Density** rises toward `min(1, (RH − 0.85) / 0.15)` with a 10 min time constant.
- **Clears** (15 min time constant) when RH < 0.85, or once the phase is 0.1 of the cycle
  past sunrise.

### Lightning

Only in downpours.

- Strikes arrive as a Poisson process, about one every 30 s, with each interval clamped
  to 10–60 s.
- A strike hits the highest top of four random sky-open columns, so tall things get hit.
- It has no effect on the world in WX1. The view carries the most recent strike.

### What it should produce (tropical_jungle; calibration targets, not hard rules)

Nature reference, lowland tropical rainforest: RH 77–88 % by day and near saturation at
night; rain mostly from deep convection peaking in the mid-to-late afternoon; heavy rain
carries most of the volume; valley fog and low cloud at dawn; drizzle uncommon, mostly
at night and dawn.

- Rain on at least 70 % of days (a day is one cycle).
- At least half the rain volume falls between 12:00 and 18:00 of the day.
- A lush world (terrarium seed 1 by hour 3–4): showers every ~40–60 min, and downpours
  carrying at least 40 % of the volume, at least one every two days.
- A young world: something every hour or two, fog on at least half the dawns, and
  drizzle mostly at night and dawn.
- RH never passes 1.5; the lake stays within ±15 % after hour 2.
- Stands at 4 h within ~20 % of today's (default ~1,750–2,000; terrarium ~8,500 seed 1,
  ~5,300 seed 2). WX3 will move this; WX1 must not.

## The seam: `WeatherView` (pinned on main)

`crates/cubarium-voxel/src/weather.rs` defines `WeatherView`, `RainMode` and `Strike`,
and `VoxelView::weather` carries one per tick:

- `WeatherView { temperature_c, rh, mode, rain_m_per_s, fog, cloud_cover, day_phase,
  daylight, sun_elevation, last_strike }`.
- `last_strike` is the most recent strike, not a per-tick flag, so a presenter that
  stages every few ticks still sees it.
- `cloud_cover = clamp((RH − 0.4) / 0.6)`, pushed up to at least 0.9 during a downpour.
- Until WX1 lands, `WeatherView::legacy` fills it from the shower store (a fixed noon, a
  shower whenever one falls). **Neither package changes these fields' meaning without
  Fable.** Adding a field is fine.

## Packages

- **WX1: weather model** (cubarium-voxel). Brief below. Opus, high effort.
- **WX2: weather and day/night visuals** (the GPU renderer). Brief below. Runs in
  parallel with WX1 against the pinned view. Renderer worker, high effort.
- **WX3: night stops photosynthesis** (flora). Plant light × `life_daylight()`. With ⅔
  daylight, production per day falls by about a third or more. Needs a colonisation
  re-check. After WX1.
- **WX4: animal circadian rhythms** (fauna). Each species has an active phase (diurnal,
  nocturnal, crepuscular).
  - In its rest phase an animal stops and pays reduced upkeep. A starving animal stays
    active to forage.
  - This is the same cheap-rest state the predation handoff needs for a predator's long
    rest (`predation-and-flight-2026-09-23.md`), and sleeping prey is a vulnerable state
    for it.
  - The brain gets a light or phase input in the next sensor manifest (the next training
    round needs one anyway).
  - Starting picks: browsers diurnal, shredders nocturnal, lanternjaw nocturnal, bellwing
    diurnal. After WX1.
- **Storm streams** (exploratory, not dispatched; Wrysk's call). Soil takes water at
  `K = 0.2 · 0.35 · 0.25 = 0.0175 m/s`, about 500× R₀, so rain on soil never runs off;
  only rock (5e-6 m/s) sheds it. Two paths:
  - **Stream swell** (cheap, O(1), conserved): a linear "upstream catchment" reservoir
    takes a share of each event's rain and drains through the spring cell over ~10–20
    min. The existing stream and its falls swell after a storm, and foam follows.
  - **Storm runoff** (a probe first): a surface infiltration cap on the top soil face,
    near R₀, so showers soak in and downpours run off as sheets that collect in low
    ground and reach the lake. Whether it reads as streams depends on the terrain having
    channels. It is also a hydrology rule change (plant water, lake level, solver cost).
- **WX5: more climates** (later), when there are species for them.
- **WX6: local weather feed** (someday): a `WeatherSource` that sets temperature and turns
  real rain into a mode.

---

## Brief WX1: weather model

**Objective:** replace the closed cycle's shower calendar with the model above, and
measure it.

**Where:**
- a new `crates/cubarium-voxel/src/weather.rs` model beside the pinned view types;
- `water.rs` (`rain` / `shower` / `deliver` / `evaporate`) and `world.rs` (state, and
  `view()` fills the real `WeatherView`);
- `config.rs` and `recipe.rs` (`[world.day]`, `[world.climate]`), the snapshot;
- the census (`crates/cubarium/examples/voxel_census.rs`) and the configs under `config/`.

**Rules:**
- **Water conservation:** the ledger's residual stays 0 through every mode.
- The closed cycle always runs the weather model. `shower_trigger_fraction` and
  `shower_volume_m3` go, and `shower_interval_*` becomes `dry_gap_*`. The open budget
  keeps its prescribed rain and still gets the clock and temperature.
- `is_raining()` means `mode != Clear`.
- Snapshot schema 17 → 18. Old worlds are refused; never migrate.
- FxHash on any sim-path map. Weather work is O(1) per tick plus the existing rain
  spread; the per-minute draw must not scan the world.

**Tests first** (tiny, a few ticks each; commit them before the model):
- capacity halves with ~10 °C of cooling;
- RH ≥ 1.5 forces a downpour inside the dry gap;
- with the hazards set to certainty, an event starts at the next minute outside the gap,
  and a would-be start inside the gap counts `gap_blocked` and starts nothing;
- an event stops at its stop RH, or at its longest duration;
- lightning only in a downpour, and `last_strike` persists after it;
- a front raises RH at a fixed store;
- evaporation is 0 at RH ≥ 1;
- the diurnal low is at sunrise;
- the clock's modes:
  - `cycle` phase and daylight at sunrise, noon and midnight;
  - `off` gives `life_daylight` 1 while the view shows the look;
  - `local` with a fixed injected wall time gives the expected phase;
- the ledger's residual is 0 through a forced storm;
- the tropical_jungle preset parses from a toml with one override.

**Measure:** on eidolon (the `scripts/voxel-remote-ship.sh` pattern, znver5, pinned by
physical core), 4 h runs:
- default seeds 1–4 and terrarium seeds 1–2;
- each with the gap on (300–900 s) and off (0 / 0).

Calibrate the starting values against the targets, the downpour rate first. The census
gains `temp_c, rh, mode, fog, rain_in, drizzles, showers, downpours, forced, gap_blocked,
strikes`. `--weather-log FILE` writes one row per event: start and end tick, mode, cause
(natural or forced), RH and °C and phase at the start, seconds since the previous event,
and volume.

**Return (≤ 40 lines):**
- the commits, and the values you settled on;
- per arm, per 4 h:
  - counts per mode, `forced`, `gap_blocked`;
  - rain volume by mode and by quarter of the day;
  - max RH, and the lake at 2 h and 4 h;
  - stands against today's;
- in the gap-off arms, the share of natural starts that came within 15 min of the
  previous event;
- anything in the targets you could not reach, and why.

**Not yours:** transpiration, WX3, WX4, anything under `crates/cubarium-gpu` or
`crates/cubarium/src/sink`.

---

## Brief WX2: weather and day/night visuals

**Objective:** make the weather and the time of day look good in the lit tier, with
shaders, as **interim** visuals that Wrysk judges from GIFs. The real look stays with
Wrysk and the art direction. This is effortful placeholder work, not a stage-2 decision.

**Read:**
- `design/art-direction/Cubarium_Art_Direction_v0.1.md` §05 (the palette: indigo and
  violet masses, electric cyan, blue, violet and magenta, selective warm accents; calm
  at a glance);
- the presentation plan, `design/handoffs/presentation-plan-2026-09-24.md` (lit tier, sun
  in the front hemisphere, bloom, lit water with rain rings);
- the pinned `crates/cubarium-voxel/src/weather.rs`;
- presentation plan §V (volumetric light, in flight in another worktree). V builds the
  sun-visibility volume and in-scatter, and exposes `airDensity(p)` and `sunInAir(p)` for
  your fog. It keys its rebake off the sun direction the sink hands the renderer each
  frame, which is how your moving sun reaches it.

**Where:**
- `crates/cubarium-gpu/shaders/` (`voxel.frag`, `scene.glsl`, `background.frag`, the bloom
  passes) and `crates/cubarium-gpu/src/voxel.rs` (uniforms; `update_weather` grows to
  take the `WeatherView`);
- `crates/cubarium/src/sink/gpu/voxel.rs` (feeds it);
- the CPU renderer stays basic and untouched.

**What to draw:**

1. **Day and night.**
   - The sun's direction follows `day_phase` and `sun_elevation`, arcing through the front
     hemisphere: low with long shadows at dawn and dusk, warm-tinted near the horizon (the
     warm accent).
   - Ambient level and colour follow `daylight`. Today's clear look is the daytime
     reference.
   - Night is deep indigo at a floor that still reads. The emitters and bloom (glowcap,
     bloomcrown cores, water highlights) carry it, and a weak cool moonlight is optional.
   - The sky gradient follows the phase: night with stars that twinkle smoothly, dawn and
     dusk with a warm magenta-orange horizon band, day as today.
2. **Clouds.**
   - A procedural cloud layer in the sky band (layered or domain-warped noise, slow
     drift). It replaces today's moisture wisps.
   - Coverage follows `cloud_cover`: lower, heavier and darker in a downpour; edges lit
     by a low sun; lit from inside by lightning.
3. **Rain.**
   - Three looks for three modes:
     - drizzle: fine, sparse and slow, almost a haze;
     - shower: streaks;
     - downpour: dense, long, slanted sheets with gusts.
   - Two or three parallax layers, depth-aware against the walk's hit so rain does not
     paint over nearer terrain as if it were in front.
   - Splashes on sky-open tops. Rain-ring density on water scales with the rate (open
     item from W).
   - Rain animates on the every-frame clock, not the 20 Hz tick (open item from W).
4. **Storm.** The sky and ambient darken with the downpour.
5. **Lightning**, from `last_strike`:
   - a branching bolt from the cloud base to the struck column's top, seeded by
     `strike.seed`, into bloom;
   - a cool-white fill flash with a fast attack and an exponential decay of ~150–300 ms,
     with one or two re-strokes;
   - the clouds lit from within.
   - **Keep it calm:** a moderate peak, and never more than ~3 flashes a second.
   - Thunder has no sound. A delayed rumble (shake) is an open art question; leave it
     out.
6. **Fog** (checkpoint 2, built on V's `airDensity` / `sunInAir` once V merges, not a
   second in-scatter). Height fog along the ground and water, thicker in low basins and over the
   lake, drifting noise, density from `fog`. Lit by the ambient light and the sun (it
   glows at dawn), with a halo around emitters inside it.
7. **Optional, if the rest is done:** tops darken and gain a sheen while wet, drying over
   a few minutes after the rain.

**Rules:**
- **Nothing pops.** Every effect eases toward the view on the every-frame clock: rain
  fades in and out over seconds, fog over its own time constants, day and night
  continuously. Lightning is the only fast attack, and it decays smoothly.
- Rendering effects may be smooth. The pixel-art rule binds illustration only.
- No analytical UI on the normal display.
- **Flat tier (the Tachyon panel):** no new full-screen passes, and its redraw rate must
  not rise when only slow clouds move. Day/night tint, rain streaks and the lightning
  flash are enough there.
- **Lit tier budget:** ≤ 1 ms per frame added in the worst case (downpour, fog and a
  strike) at the desktop config on the 5080. Measure it.
- The lit tier redraws every frame while weather animates, as water does.
- Windows: never on Wrysk's screen. Capture headless (`--gpu-target headless
  --gpu-capture DIR`) or through `scripts/hidden.sh`.

**Dev tool:** a capture-only `--weather-preview` that overrides the view's
`WeatherView` with a scripted loop of about 90 s:

1. dawn fog;
2. clear morning;
3. clouds building;
4. a downpour with strikes;
5. clearing;
6. dusk;
7. night drizzle;
8. clear night.

It drives every capture without waiting on the simulation, and stays as a tool.

**Checkpoints:**
- **Checkpoint 1** (day/night, sky, clouds): stop and return looping GIFs at real-time
  speed (ffmpeg palettegen + `paletteuse=dither=none`, `-loop 0`) of the full day with no
  rain, plus stills of noon beside today's main. Fable shows Wrysk.
- **Checkpoint 2:** rain, storm, lightning, fog; a GIF of the preview loop, and the
  measured frame costs.

**Return (≤ 40 lines):** commits, GIF paths (absolute), the frame costs (lit and flat),
and open look questions.

**Not yours:** anything under `crates/cubarium-voxel` except reading `weather.rs`.

## Not in this thread

Clouds with positions and uneven rain (route C); wind tied to the weather (the wind
packet stays as it is); snow and frost; sound; new animal inputs (those come with WX4).
