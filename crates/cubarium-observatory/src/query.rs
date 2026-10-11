//! **The query layer**: plain functions over the live world's three layers that return
//! serde types. No transport, no threads, no clock of their own: an in-game UI or a Godot
//! GDExtension calls these exactly as the HTTP server does, and the server only
//! serializes what they return.
//!
//! Everything here reads; nothing here steps, edits or caches world state. Each function
//! is O(animals + stands + ground sites), plus O(wet cells) for the standing water — never
//! a walk over every voxel — so a host can call them while it holds the world.

use cubarium_voxel::weather::{DayMode, RainMode};
use cubarium_voxel::{DT, Landform, World};
use cubarium_voxel_fauna::{Animal, Departure, Fauna, FaunaConfig, Food, Founder, SpeciesConfig, State};
use cubarium_voxel_flora::{Flora, Species as PlantSpecies};
use serde::{Deserialize, Serialize};

// --- Names ----------------------------------------------------------------------------

/// The name a lineage is shown by. The code identifier stays the founder's
/// ([`Founder::name`]); the chorister is shown as the **veilhound** (Wrysk's rename).
pub fn lineage_label(founder: Founder) -> &'static str {
    match founder {
        Founder::Chorister => "veilhound",
        f => f.name(),
    }
}

/// A rain mode's name, lower case.
pub fn rain_name(mode: RainMode) -> &'static str {
    match mode {
        RainMode::Clear => "clear",
        RainMode::Drizzle => "drizzle",
        RainMode::Shower => "shower",
        RainMode::Downpour => "downpour",
    }
}

// --- World summary --------------------------------------------------------------------

/// The schema versions of the three saved layers.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Schemas {
    pub world: u32,
    pub flora: u32,
    pub fauna: u32,
}

impl Schemas {
    pub fn current() -> Schemas {
        Schemas {
            world: cubarium_voxel::snapshot::SCHEMA,
            flora: cubarium_voxel_flora::snapshot::SCHEMA,
            fauna: cubarium_voxel_fauna::SCHEMA,
        }
    }
}

/// What world this is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldSummary {
    /// The host's own words for where the world came from (`generated (seed 7)`,
    /// `resumed …`); empty when the caller has none.
    pub label: String,
    /// The landform kind: `ridge`, `staged` or `terrarium`.
    pub landform: String,
    /// The climate preset's name.
    pub climate: String,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    /// Metres per voxel edge.
    pub voxel_m: f64,
    pub seed: u64,
    pub schema: Schemas,
    pub tick: u64,
    pub closed_water_budget: bool,
}

/// [`WorldSummary`] of `world`, under the host's `label`.
pub fn summary(world: &World, label: &str) -> WorldSummary {
    let c = world.config();
    WorldSummary {
        label: label.to_string(),
        landform: match c.landform {
            Landform::Ridge => "ridge",
            Landform::Staged(_) => "staged",
            Landform::Terrarium(_) => "terrarium",
        }
        .to_string(),
        climate: c.climate.preset.name().to_string(),
        width: c.width,
        height: c.height,
        depth: c.depth,
        voxel_m: c.voxel_m,
        seed: c.seed,
        schema: Schemas::current(),
        tick: world.tick(),
        closed_water_budget: c.closed_water_budget,
    }
}

// --- Diegetic time --------------------------------------------------------------------

/// A moment of the Cubarium year.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct YearInfo {
    /// Years since the first dry season; −1 before it.
    pub number: i64,
    /// Where in the year, `[0, 1)`; 0 is a dry season's start.
    pub phase: f64,
}

/// The world's own clock: the day clock's day and hour, and the simulated time behind it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldClock {
    pub tick: u64,
    /// Simulated seconds since the world's tick 0 (`tick · DT`).
    pub sim_elapsed_s: f64,
    /// `cycle`, `local` or `off`.
    pub mode: String,
    /// Simulated seconds in one day of this clock.
    pub day_length_s: f64,
    /// The day, counted from 1: the cycle's days since the world began at its start hour.
    pub day: i64,
    /// Where in the day, `0..1`: 0 midnight, 0.5 noon. Noon with the clock off.
    pub day_phase: f64,
    /// The same on a 24-hour dial, `0..24`.
    pub hour: f64,
    /// `HH:MM` on that dial.
    pub time_of_day: String,
    /// Sunrise and sunset as day phases.
    pub sunrise_phase: f64,
    pub sunset_phase: f64,
    /// The daylight plants and animals live by, 0 (night) to 1 (day).
    pub daylight: f64,
    /// Whether the sun is down.
    pub night: bool,
    pub year: Option<YearInfo>,
    pub dry_season: bool,
}

/// [`WorldClock`] of `world`.
pub fn clock(world: &World) -> WorldClock {
    let c = world.config();
    let tick = world.tick();
    let elapsed = tick as f64 * DT;
    let day_s = c.day.day_s();
    let start = match c.day.mode {
        DayMode::Cycle => c.day.start_hour / 24.0,
        DayMode::Local | DayMode::Off => 0.0,
    };
    let day = if day_s > 0.0 { (start + elapsed / day_s).floor() as i64 + 1 } else { 1 };
    let phase = world.day_phase();
    let hour = phase * 24.0;
    let minutes = (phase * 24.0 * 60.0).floor() as i64;
    let sun = world.weather().sun();
    WorldClock {
        tick,
        sim_elapsed_s: elapsed,
        mode: match c.day.mode {
            DayMode::Cycle => "cycle",
            DayMode::Local => "local",
            DayMode::Off => "off",
        }
        .to_string(),
        day_length_s: day_s,
        day,
        day_phase: phase,
        hour,
        time_of_day: format!("{:02}:{:02}", (minutes / 60) % 24, minutes % 60),
        sunrise_phase: sun.rise,
        sunset_phase: sun.set,
        daylight: world.life_daylight(),
        night: world.life_to_dawn_s().is_some(),
        year: world.year().map(|y| YearInfo { number: y.number, phase: y.phase }),
        dry_season: world.in_dry_season(),
    }
}

// --- Environment ----------------------------------------------------------------------

/// The water the world holds, by store, cubic metres.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaterStocks {
    /// The lumped atmosphere; zero under the open budget.
    pub atmosphere_m3: f64,
    /// The aquifer, and its water table in metres above the floor.
    pub aquifer_m3: f64,
    pub aquifer_head_m: f64,
    /// Free water standing in the world: lake, pools, streams and falling rain.
    pub standing_m3: f64,
    /// Pore water in soil and rock: the in-world store less the standing water and the
    /// aquifer, read from the water ledger (whose residual is floating-point noise).
    pub soil_m3: f64,
    /// Every store, the atmosphere included.
    pub total_m3: f64,
    /// What the spring's stream brought up last tick, m³/s.
    pub stream_m3_s: f64,
    pub outlet_open: bool,
}

/// A rain event, running.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RainEvent {
    pub mode: String,
    pub start_tick: u64,
    /// What it has dropped so far, m³.
    pub volume_m3: f64,
}

/// Rain events since the world began.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RainCounts {
    pub drizzles: u64,
    pub showers: u64,
    pub downpours: u64,
    pub strikes: u64,
}

/// The weather and the water now.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Environment {
    pub tick: u64,
    /// Air temperature, °C, and the one plants and animals live by (the climate's mean
    /// with the day clock off).
    pub temperature_c: f64,
    pub life_temperature_c: f64,
    /// Relative humidity: the atmosphere over what the air holds at its temperature.
    pub rh: f64,
    /// The air's moisture at the climate's mean temperature (what rain starts on).
    pub moisture: f64,
    /// `clear`, `drizzle`, `shower` or `downpour`.
    pub rain: String,
    /// The rate rain is falling at now, millimetres an hour.
    pub rain_mm_per_h: f64,
    /// Ground fog, 0..1, and cloud cover, 0..1.
    pub fog: f64,
    pub cloud_cover: f64,
    /// The simulation's daylight, 0..1, and the sun's elevation in degrees.
    pub daylight: f64,
    pub sun_elevation_deg: f64,
    pub dry_season: bool,
    /// A cold front's depth, °C, while one is passing.
    pub front_depth_c: Option<f64>,
    pub event: Option<RainEvent>,
    pub rain_counts: RainCounts,
    pub water: WaterStocks,
}

/// [`Environment`] of `world`. `wet` is a scratch buffer reused between calls (the wet
/// cells' indices); its contents on return are unspecified.
pub fn environment_with(world: &World, wet: &mut Vec<usize>) -> Environment {
    let view = world.view();
    let w = world.weather();
    let vol = world.config().voxel_volume();
    let standing = if view.wet_cells(wet) {
        wet.iter().map(|&i| view.free[i]).sum::<f64>() * vol
    } else {
        world.pooled_m3()
    };
    let stored = view.ledger.expected_stored();
    let counts = w.counts;
    Environment {
        tick: world.tick(),
        temperature_c: w.temperature_c(),
        life_temperature_c: world.life_temperature_c(),
        rh: world.rh(),
        moisture: world.moisture(),
        rain: rain_name(w.mode()).to_string(),
        rain_mm_per_h: f64::from(view.weather.rain_m_per_s) * 3.6e6,
        fog: w.fog(),
        cloud_cover: f64::from(view.weather.cloud_cover),
        daylight: world.life_daylight(),
        sun_elevation_deg: w.sun().elevation.to_degrees(),
        dry_season: world.in_dry_season(),
        front_depth_c: w.front().map(|f| f.depth_c),
        event: w.event().map(|e| RainEvent {
            mode: rain_name(e.mode).to_string(),
            start_tick: e.start_tick,
            volume_m3: e.volume_m3,
        }),
        rain_counts: RainCounts {
            drizzles: counts.drizzles,
            showers: counts.showers,
            downpours: counts.downpours,
            strikes: counts.strikes,
        },
        water: WaterStocks {
            atmosphere_m3: view.atmosphere_m3,
            aquifer_m3: view.aquifer_m3,
            aquifer_head_m: world.aquifer_head_m(),
            standing_m3: standing,
            soil_m3: (stored - standing - view.aquifer_m3).max(0.0),
            total_m3: stored + view.atmosphere_m3,
            stream_m3_s: w.stream_m3_s(),
            outlet_open: view.outlet_open,
        },
    }
}

/// [`environment_with`] with a buffer of its own.
pub fn environment(world: &World) -> Environment {
    environment_with(world, &mut Vec::new())
}

// --- Populations ----------------------------------------------------------------------

/// Where a living body is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeStage {
    /// A hatchling clamped onto a host (the lanternjaw's parasite stage).
    Rider,
    /// Born into a pack and not yet independent.
    Pup,
    /// Free, below its lineage's `birth_body`: still growing to breeding size.
    Juvenile,
    /// At its lineage's `birth_body` or more.
    Adult,
}

impl LifeStage {
    pub fn name(self) -> &'static str {
        match self {
            LifeStage::Rider => "rider",
            LifeStage::Pup => "pup",
            LifeStage::Juvenile => "juvenile",
            LifeStage::Adult => "adult",
        }
    }
}

/// The physiology a body runs: its lineage's, or its species' for a body without one.
fn core_of<'c>(config: &'c FaunaConfig, a: &Animal) -> &'c SpeciesConfig {
    match a.founder {
        Some(f) => &config.founder(f).core,
        None => config.species(a.species),
    }
}

/// `a`'s [`LifeStage`]: a rider while it rides, a pup while its pack says so, otherwise
/// adult from its lineage's `birth_body` (the size it may breed at) and juvenile below.
pub fn stage_of(config: &FaunaConfig, a: &Animal) -> LifeStage {
    if a.attached.is_some() {
        LifeStage::Rider
    } else if a.pack.is_some_and(|p| p.pup) {
        LifeStage::Pup
    } else if a.body >= core_of(config, a).birth_body {
        LifeStage::Adult
    } else {
        LifeStage::Juvenile
    }
}

/// One lineage's census.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimalPopulation {
    /// The lineage's code name ([`Founder::name`]).
    pub species: String,
    /// The name it is shown by ([`lineage_label`]).
    pub label: String,
    pub role: String,
    pub alive: u32,
    pub adults: u32,
    pub juveniles: u32,
    pub pups: u32,
    pub riders: u32,
    /// Eggs standing in the world, over every clutch (a dormant egg bank included).
    pub eggs: u64,
    /// Bodies carrying a gestation.
    pub gestating: u64,
    /// Deaths since the world began, and by cause (only the causes that happened).
    pub deaths: u64,
    pub deaths_by_cause: Vec<(String, u64)>,
}

/// One plant species' census.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlantPopulation {
    pub species: String,
    pub stands: u32,
    /// Counts in the species' anatomy-profile order (youngest first), selected by
    /// `SpeciesConfig::profile_index`. Empty when there is no staged anatomy.
    pub stages: Vec<u32>,
    /// Organic matter in the stands' tissue (wood + foliage + reserve), and the foliage
    /// and wood on their own.
    pub organic: f64,
    pub foliage: f64,
    pub wood: f64,
    /// Organic matter dormant in seed banks.
    pub seed_bank: f64,
}

/// The latticevine.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct VineCount {
    pub vines: u32,
    /// Faces the vines cover.
    pub faces: u32,
}

/// Dead and grazed stocks on the ground, organic units.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroundStocks {
    pub litter: f64,
    pub dead_wood: f64,
    pub carrion: f64,
    /// Glassfilm, the shore's grazed film.
    pub film: f64,
}

/// Every lineage and every plant species, in their fixed orders.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Populations {
    pub tick: u64,
    pub animals: Vec<AnimalPopulation>,
    pub plants: Vec<PlantPopulation>,
    pub vines: VineCount,
    pub ground: GroundStocks,
}

/// [`Populations`] of the two layers. Every lineage and species is listed, a dead one
/// with zeros, so a series never changes shape.
pub fn populations(flora: &Flora, fauna: &Fauna) -> Populations {
    let av = fauna.view();
    let mut animals: Vec<AnimalPopulation> = Founder::ALL
        .into_iter()
        .map(|f| {
            let mut deaths_by_cause = Vec::new();
            let mut deaths = 0;
            for cause in Departure::ALL {
                let n = av.ledger.departed_founder(f, cause);
                if n > 0 && cause.is_death() {
                    deaths += n;
                    deaths_by_cause.push((cause.name().to_string(), n));
                }
            }
            AnimalPopulation {
                species: f.name().to_string(),
                label: lineage_label(f).to_string(),
                role: f.role().to_string(),
                alive: 0,
                adults: 0,
                juveniles: 0,
                pups: 0,
                riders: 0,
                eggs: av.eggs_by_founder(f),
                gestating: av.gestating_by_founder(f),
                deaths,
                deaths_by_cause,
            }
        })
        .collect();
    for a in av.animals {
        let Some(f) = a.founder else { continue };
        let p = &mut animals[f.index()];
        p.alive += 1;
        match stage_of(av.config, a) {
            LifeStage::Rider => p.riders += 1,
            LifeStage::Pup => p.pups += 1,
            LifeStage::Juvenile => p.juveniles += 1,
            LifeStage::Adult => p.adults += 1,
        }
    }

    let fv = flora.view();
    let mut plants: Vec<PlantPopulation> = PlantSpecies::ALL
        .into_iter()
        .map(|s| PlantPopulation {
            species: s.name().to_string(),
            stands: 0,
            stages: if fv.config.species(s).profile.len() > 1 {
                vec![0; fv.config.species(s).profile.len()]
            } else {
                Vec::new()
            },
            organic: 0.0,
            foliage: 0.0,
            wood: 0.0,
            seed_bank: 0.0,
        })
        .collect();
    for s in fv.stands {
        let p = &mut plants[s.species.index()];
        p.stands += 1;
        if !p.stages.is_empty() {
            p.stages[fv.config.species(s.species).profile_index(s.wood)] += 1;
        }
        p.organic += s.organic();
        p.foliage += s.foliage;
        p.wood += s.wood;
    }
    let mut ground = GroundStocks { litter: 0.0, dead_wood: 0.0, carrion: 0.0, film: 0.0 };
    for g in fv.ground {
        ground.litter += g.litter;
        ground.dead_wood += g.dead_wood;
        ground.carrion += g.carrion;
        ground.film += g.film;
        for c in &g.seeds {
            plants[c.species.index()].seed_bank += c.organic;
        }
    }
    Populations {
        tick: av.tick,
        animals,
        plants,
        vines: VineCount {
            vines: fv.cover.vines().len() as u32,
            faces: fv.cover.face_count() as u32,
        },
        ground,
    }
}

// --- Animals --------------------------------------------------------------------------

/// What a body is doing, in words: the strongest of its conditions.
pub fn activity_of(a: &Animal) -> String {
    if let Some(at) = a.attached {
        return format!("riding #{}", at.host);
    }
    if let Some(s) = a.strike {
        return format!("striking #{}", s.prey);
    }
    if a.rest.is_some() || a.state == State::Asleep {
        return "asleep".into();
    }
    if a.hunkered {
        return "hunkered".into();
    }
    if a.yielding {
        return "yielding a carcass".into();
    }
    if a.mobility.falling {
        return "falling".into();
    }
    let doing = state_name(a.state);
    if a.eating {
        match a.bout {
            Some(b) if b.kind != cubarium_voxel_fauna::BoutKind::Normal => {
                format!("{doing}, feeding ({})", b.kind.name())
            }
            _ => format!("{doing}, feeding"),
        }
    } else {
        doing.to_string()
    }
}

pub fn state_name(s: State) -> &'static str {
    match s {
        State::Cropping => "cropping",
        State::Walking => "walking",
        State::Resting => "resting",
        State::Climbing => "climbing",
        State::Asleep => "asleep",
    }
}

/// One row of the animal table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimalRow {
    pub id: u64,
    /// The lineage's code name, and the name it is shown by.
    pub species: String,
    pub label: String,
    pub stage: LifeStage,
    /// Simulated seconds since it was born or introduced.
    pub age_s: f64,
    /// Its drawn lifespan, simulated seconds; 0 is none.
    pub lifespan_s: f64,
    /// Organic structure, and the same over its lineage's full size (`body_max`).
    pub body: f64,
    pub size: f64,
    /// Reserve over the reserve its body can hold, 0..1: how fed it is.
    pub fullness: f64,
    /// The energy in its tissue.
    pub energy: f64,
    /// The wound stock, 0..1 (dead at 1).
    pub wound: f64,
    /// An amphibious body's drying clock, 0 (wet) to 1; 0 for every other body.
    pub dryness: f64,
    /// [`activity_of`].
    pub activity: String,
    /// Position in metres: along the ring, above the floor, into the depth.
    pub x_m: f64,
    pub y_m: f64,
    pub z_m: f64,
    pub heading_rad: f64,
    /// The support face it stands on, as voxel indices.
    pub site: [u32; 3],
    pub pack: Option<u64>,
}

/// [`AnimalRow`] of one body.
pub fn animal_row(fauna: &Fauna, a: &Animal) -> AnimalRow {
    let config = fauna.config();
    let core = core_of(config, a);
    let cap = core.reserve_of(a.body);
    let (species, label) = match a.founder {
        Some(f) => (f.name(), lineage_label(f)),
        None => (a.species.name(), a.species.name()),
    };
    AnimalRow {
        id: a.id,
        species: species.to_string(),
        label: label.to_string(),
        stage: stage_of(config, a),
        age_s: a.age_ticks as f64 * DT,
        lifespan_s: a.lifespan_ticks as f64 * DT,
        body: a.body,
        size: if core.body_max > 0.0 { a.body / core.body_max } else { 1.0 },
        fullness: if cap > 0.0 { (a.reserve / cap).clamp(0.0, 1.0) } else { 0.0 },
        energy: a.energy,
        wound: a.wound,
        dryness: a.mobility.dryness,
        activity: activity_of(a),
        x_m: a.pose.x,
        y_m: a.pose.y_m,
        z_m: a.pose.z,
        heading_rad: a.pose.heading_rad,
        site: [a.site.x, a.site.y, a.site.z],
        pack: a.pack.map(|p| p.pack),
    }
}

/// Every living body, by id.
pub fn animals(fauna: &Fauna) -> Vec<AnimalRow> {
    fauna.view().animals.iter().map(|a| animal_row(fauna, a)).collect()
}

/// One stomach pool, by food.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FoodAmount {
    pub food: String,
    pub organic: f64,
}

/// A living body in full: its row, and what the row leaves out.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimalDetail {
    #[serde(flatten)]
    pub row: AnimalRow,
    pub reserve: f64,
    pub mineral: f64,
    /// Raw food eaten and not yet digested, per food class (the nonzero ones).
    pub stomach: Vec<FoodAmount>,
    /// The eating latch, and the emergency latch (awake whatever the hour).
    pub eating: bool,
    pub emergency: bool,
    /// The feeding bout in progress: its kind and how long it has run, seconds.
    pub bout: Option<(String, f64)>,
    /// What it did on the last tick (`cropping`, `walking`, …).
    pub state: String,
    /// Which layer set its held actions last (`heuristic`, `flee`, `pack`, …).
    pub steer: Option<String>,
    /// The prey of the strike in progress.
    pub striking: Option<u64>,
    /// The host it rides.
    pub riding: Option<u64>,
    /// Gestating, and the offspring organic matter it has passed on so far.
    pub gestating: bool,
    pub given: f64,
    /// Born in the world (not founded or introduced).
    pub native: bool,
    /// A pack body's den face and whether it is a pup.
    pub den: Option<[u32; 3]>,
    pub pup: bool,
}

/// [`AnimalDetail`] of the living body `id`, or `None` when no body of that id lives.
pub fn animal(fauna: &Fauna, id: u64) -> Option<AnimalDetail> {
    let view = fauna.view();
    let a = view.animal(id)?;
    let tick = view.tick;
    let den = a.pack.and_then(|m| fauna.packs().iter().find(|p| p.id == m.pack)).map(|p| [p.den.x, p.den.y, p.den.z]);
    Some(AnimalDetail {
        row: animal_row(fauna, a),
        reserve: a.reserve,
        mineral: a.mineral,
        stomach: Food::ALL
            .into_iter()
            .filter(|f| a.stomach.pools[f.index()].organic > 0.0)
            .map(|f| FoodAmount { food: f.name().to_string(), organic: a.stomach.pools[f.index()].organic })
            .collect(),
        eating: a.eating,
        emergency: a.emergency,
        bout: a.bout.map(|b| (b.kind.name().to_string(), tick.saturating_sub(b.start_tick) as f64 * DT)),
        state: state_name(a.state).to_string(),
        steer: fauna.steer_of(id).map(|s| s.name().to_string()),
        striking: a.strike.map(|s| s.prey),
        riding: a.attached.map(|h| h.host),
        gestating: a.reproduction.escrow.is_some(),
        given: a.reproduction.given,
        native: a.reproduction.native,
        den,
        pup: a.pack.is_some_and(|p| p.pup),
    })
}
