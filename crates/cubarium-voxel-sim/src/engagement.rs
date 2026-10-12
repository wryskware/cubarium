//! Seeded family–pack encounters (ENGAGE). Setup and readout only: today's heuristics.
//! The wide bare-soil arena freezes terrain and flora, as the episode trainer does.
//! No hunger, pursuit, attack, rest or flight rule is changed.

use crate::{Sim, SimConfig, habitat};
use cubarium_voxel::registry::{Range, Registry, Row};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as WorldConfig, Material, World};
use cubarium_voxel_fauna::{
    ChaseEnd, Command, Fauna, FaunaConfig, Founder, PackEvent, StartingStores,
};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Composition {
    Family,
    Adults,
    Calf,
}

/// An arm changes roster or experimental parameters, never the seeded placement stream.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Arm {
    pub name: String,
    pub composition: Composition,
    /// Number of adult hounds in the one pack, 2–4.
    pub pack_size: usize,
    /// Optional ordinary registry assignments for future mechanisms, applied after setup.
    #[serde(default)]
    pub sets: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Simulated seconds per episode, at most 300 s.
    pub duration_s: f64,
    /// Episodes per arm, beginning at the requested seed.
    pub episodes: usize,
    /// Arena width in voxels (periodic).
    pub width: u32,
    /// Arena height in voxels.
    pub height: u32,
    /// Arena depth in voxels (closed edges).
    pub depth: u32,
    /// Metres per voxel.
    pub voxel_m: f64,
    /// Soil thickness in voxels above bedrock.
    pub soil_layers: u32,
    /// Metres from family centre to pack centre, inclusive range.
    pub distance_m: [f64; 2],
    /// Metres between adjacent starting bodies in a group.
    pub spacing_m: f64,
    /// Metres of centre placement jitter in depth, each side of arena middle.
    pub depth_jitter_m: f64,
    /// Adult structure as a fraction of its lineage's maximum, range.
    pub adult_body: [f64; 2],
    /// Calf structure as a fraction of loftstrider maximum, range.
    pub calf_body: [f64; 2],
    /// Family reserve and stomach fullness as fractions of capacity, range.
    pub family_fullness: [f64; 2],
    /// Hound reserve and stomach fullness as fractions of capacity, range.
    pub pack_fullness: [f64; 2],
    /// Adult arrival age in simulated seconds, range.
    pub adult_age_s: [f64; 2],
    /// Calf arrival age in simulated seconds, range.
    pub calf_age_s: [f64; 2],
    /// Include the usual non-pack, non-loftstrider founder counts.
    pub usual_prey: bool,
    pub arms: Vec<Arm>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            duration_s: 300.0,
            episodes: 50,
            width: 128,
            height: 48,
            depth: 96,
            voxel_m: 0.25,
            soil_layers: 2,
            distance_m: [4.0, 6.0],
            spacing_m: 1.5,
            depth_jitter_m: 1.0,
            adult_body: [0.9, 1.0],
            calf_body: [0.16, 0.22],
            family_fullness: [0.85, 0.95],
            pack_fullness: [0.1, 0.2],
            adult_age_s: [600.0, 900.0],
            calf_age_s: [0.0, 30.0],
            usual_prey: false,
            arms: [Composition::Family, Composition::Adults, Composition::Calf]
                .into_iter()
                .flat_map(|composition| {
                    (2..=4).map(move |pack_size| Arm {
                        name: format!(
                            "{}-{pack_size}",
                            match composition {
                                Composition::Family => "family",
                                Composition::Adults => "adults",
                                Composition::Calf => "calf",
                            }
                        ),
                        composition,
                        pack_size,
                        sets: vec![],
                    })
                })
                .collect(),
        }
    }
}

/// Every numerical harness setting has a registry row; this is a separate scenario config.
pub fn registry() -> &'static Registry {
    static TABLE: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut rows = vec![];
        for (key, unit, range, what) in [
            (
                "duration_s",
                "s",
                Range::closed(0.0, 300.0).open_lo(),
                "Episode limit",
            ),
            (
                "episodes",
                "count",
                Range::at_least(1.0),
                "Episodes per arm",
            ),
            (
                "width",
                "voxel",
                Range::at_least(1.0),
                "Periodic arena width",
            ),
            ("height", "voxel", Range::at_least(1.0), "Arena height"),
            ("depth", "voxel", Range::at_least(1.0), "Arena depth"),
            ("voxel_m", "m", Range::positive(), "Voxel pitch"),
            (
                "soil_layers",
                "voxel",
                Range::at_least(1.0),
                "Soil thickness",
            ),
            (
                "distance_m.*",
                "m",
                Range::positive(),
                "Family to pack centre distance",
            ),
            ("spacing_m", "m", Range::positive(), "Body separation"),
            (
                "depth_jitter_m",
                "m",
                Range::nonneg(),
                "Centre depth jitter",
            ),
            (
                "adult_body.*",
                "fraction",
                Range::fraction().open_lo(),
                "Adult structure",
            ),
            (
                "calf_body.*",
                "fraction",
                Range::fraction().open_lo(),
                "Calf structure",
            ),
            (
                "family_fullness.*",
                "fraction",
                Range::fraction(),
                "Family reserve and stomach",
            ),
            (
                "pack_fullness.*",
                "fraction",
                Range::fraction(),
                "Pack reserve and stomach",
            ),
            ("adult_age_s.*", "s", Range::nonneg(), "Adult arrival age"),
            ("calf_age_s.*", "s", Range::nonneg(), "Calf arrival age"),
            (
                "arms.*.pack_size",
                "count",
                Range::closed(2.0, 4.0),
                "Hounds per pack",
            ),
        ] {
            rows.push(Row::new(format!("engagement.{key}"), unit, range, what));
        }
        Registry::new(
            "engagement",
            rows,
            vec![],
            vec![
                "engagement.usual_prey".into(),
                "engagement.arms.*.name".into(),
                "engagement.arms.*.composition".into(),
                "engagement.arms.*.sets".into(),
            ],
        )
    })
}
impl Config {
    pub fn read(path: &std::path::Path) -> Result<Self, String> {
        let cfg: Self = toml::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        cfg.validate()?;
        Ok(cfg)
    }
    pub fn validate(&self) -> Result<(), String> {
        registry().check(self)?;
        for range in [
            self.distance_m,
            self.adult_body,
            self.calf_body,
            self.family_fullness,
            self.pack_fullness,
            self.adult_age_s,
            self.calf_age_s,
        ] {
            if range[0] > range[1] {
                return Err("reversed engagement range".into());
            }
        }
        if self.arms.is_empty() || self.soil_layers + 1 >= self.height {
            return Err("empty arms or soil above ceiling".into());
        }
        let room = self.distance_m[1] + self.depth_jitter_m + self.spacing_m * 2.0;
        if room >= f64::from(self.depth) * self.voxel_m / 2.0
            || room >= f64::from(self.width) * self.voxel_m / 2.0
        {
            return Err("arena needs more room around the seeded groups".into());
        }
        let mut names = BTreeSet::new();
        for arm in &self.arms {
            if arm.name.is_empty()
                || !arm
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                || !names.insert(&arm.name)
            {
                return Err("arm names must be unique safe filenames".into());
            }
        }
        Ok(())
    }
}

// A separate deterministic stream per body/quantity: omitted roster slots never move
// another body's draws, and an arm name never participates in a start.
fn draw(seed: u64, key: u64, range: [f64; 2]) -> f64 {
    let mut x = seed.wrapping_add(key.wrapping_mul(0x9e3779b97f4a7c15));
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^= x >> 31;
    range[0] + (range[1] - range[0]) * ((x >> 11) as f64 / ((1u64 << 53) as f64))
}

pub struct Episode {
    pub sim: Sim,
    pub seed: u64,
    pub arm: Arm,
    pub adults: Vec<u64>,
    pub calf: Option<u64>,
    pub hounds: Vec<u64>,
    pub pack: u64,
    limit: u64,
    tick: u64,
    encounters: BTreeSet<(u64, u64)>,
    tests: u64,
    chases: u64,
    kills: BTreeSet<u64>,
    hound_deaths: BTreeSet<u64>,
    wounds: BTreeMap<u64, f64>,
    abandoned: bool,
    outcome: Option<Outcome>,
    family_flee_ticks: u64,
}

/// Found a start with stable IDs for all seven roster slots, even in smaller arms.
/// Bodies omitted by an arm are removed only after all draws and ancestry are assigned.
pub fn found(cfg: &Config, seed: u64, arm: &Arm) -> Result<Episode, String> {
    cfg.validate()?;
    if !(2..=4).contains(&arm.pack_size) {
        return Err("pack size must be 2–4".into());
    }
    let mut world = World::empty(WorldConfig {
        width: cfg.width,
        height: cfg.height,
        depth: cfg.depth,
        voxel_m: cfg.voxel_m,
        seed,
        day: DayClock {
            mode: DayMode::Off,
            ..DayClock::default()
        },
        ..WorldConfig::default()
    });
    for z in 0..cfg.depth {
        for x in 0..i64::from(cfg.width) {
            for y in 1..=cfg.soil_layers {
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    let flora = Flora::new(FloraConfig::for_voxel_size(cfg.voxel_m));
    let mut fauna = Fauna::new(FaunaConfig::default());
    let angle = draw(seed, 1, [0.0, std::f64::consts::TAU]);
    let cx = draw(seed, 2, [0.0, f64::from(cfg.width) * cfg.voxel_m]);
    let cz = f64::from(cfg.depth) * cfg.voxel_m / 2.0
        + draw(seed, 3, [-cfg.depth_jitter_m, cfg.depth_jitter_m]);
    let distance = draw(seed, 4, cfg.distance_m);
    let px = cx + angle.sin() * distance;
    let pz = cz + angle.cos() * distance;
    let face = |x: f64, z: f64| Site {
        x: (x / cfg.voxel_m).floor().rem_euclid(f64::from(cfg.width)) as u32,
        y: cfg.soil_layers,
        z: (z / cfg.voxel_m).floor() as u32,
    };
    let mut family = vec![];
    for slot in 0..3u64 {
        let side = (slot as f64 - 1.0) * cfg.spacing_m;
        let site = face(cx + angle.cos() * side, cz - angle.sin() * side);
        let fullness = draw(seed, 10 + slot, cfg.family_fullness);
        let body = draw(
            seed,
            20 + slot,
            if slot == 2 {
                cfg.calf_body
            } else {
                cfg.adult_body
            },
        );
        if !fauna.apply(
            &world,
            Command::IntroduceFounderOnFace {
                site,
                founder: Founder::Loftstrider,
                stores: StartingStores {
                    body,
                    reserve: fullness,
                },
                heading_rad: angle,
            },
        ) {
            return Err("family placement refused".into());
        }
        let id = fauna.view().ledger.births - 1;
        fauna.set_arrival_stomach(id, fullness);
        let age = draw(
            seed,
            30 + slot,
            if slot == 2 {
                cfg.calf_age_s
            } else {
                cfg.adult_age_s
            },
        );
        if !fauna.stage_identity(
            id,
            (age * f64::from(cubarium_voxel_fauna::TICK_HZ)) as u64,
            if slot == 2 { Some(family[0]) } else { None },
        ) {
            return Err("family identity refused".into());
        }
        family.push(id);
    }
    let pack = fauna
        .found_pack_sized(
            &world,
            face(px, pz),
            Founder::Chorister,
            StartingStores {
                body: draw(seed, 40, cfg.adult_body),
                reserve: cfg.pack_fullness[0],
            },
            angle + std::f64::consts::PI,
            4,
        )
        .ok_or("pack placement refused")?;
    let all_hounds: Vec<_> = fauna
        .view()
        .animals
        .iter()
        .filter(|a| a.pack.is_some())
        .map(|a| a.id)
        .collect();
    for (slot, &id) in all_hounds.iter().enumerate() {
        let side = (slot as f64 - 1.5) * cfg.spacing_m;
        fauna.probe_place(
            id,
            face(px + angle.cos() * side, pz - angle.sin() * side),
            cfg.voxel_m,
        );
        let fullness = draw(seed, 50 + slot as u64, cfg.pack_fullness);
        fauna.set_reserve_fraction(id, fullness);
        fauna.set_arrival_stomach(id, fullness);
        fauna.stage_identity(
            id,
            (draw(seed, 60 + slot as u64, cfg.adult_age_s)
                * f64::from(cubarium_voxel_fauna::TICK_HZ)) as u64,
            None,
        );
    }
    let adults = if arm.composition == Composition::Calf {
        vec![]
    } else {
        family[..2].to_vec()
    };
    let calf = if arm.composition == Composition::Adults {
        None
    } else {
        Some(family[2])
    };
    for &id in &family {
        if !adults.contains(&id) && calf != Some(id) {
            fauna.remove_body(id)?;
        }
    }
    for &id in &all_hounds[arm.pack_size..] {
        fauna.remove_body(id)?;
    }
    if cfg.usual_prey {
        for founder in Founder::ALL {
            if matches!(
                founder,
                Founder::Loftstrider | Founder::Chorister | Founder::Lanternjaw
            ) {
                continue;
            }
            for slot in 0..fauna.config().founder_counts()[founder.index()] {
                let site = face(
                    draw(
                        seed,
                        1000 + founder.index() as u64 * 100 + slot as u64,
                        [0.0, f64::from(cfg.width) * cfg.voxel_m],
                    ),
                    cz,
                );
                fauna.apply(
                    &world,
                    Command::IntroduceFounderOnFace {
                        site,
                        founder,
                        stores: StartingStores::FULL,
                        heading_rad: angle,
                    },
                );
            }
        }
    }
    let mut params = crate::params::Params {
        world: world.config().clone(),
        flora: flora.config().clone(),
        fauna: fauna.config().clone(),
    };
    params = crate::params::Params::resolve_over(
        params,
        &crate::params::Layers {
            sets: arm.sets.clone(),
            ..Default::default()
        },
    )?;
    if params.world != *world.config() || params.flora != *flora.config() {
        return Err("engagement arms may override fauna only".into());
    }
    fauna.replace_config(params.fauna)?;
    habitat::reinstall_heuristics(&mut fauna);
    fauna.set_census_readout(true);
    Ok(Episode {
        sim: Sim::new_static(
            world,
            flora,
            fauna,
            SimConfig {
                threads: 1,
                overlap: false,
                water_every: 1,
            },
        ),
        seed,
        arm: arm.clone(),
        adults,
        calf,
        hounds: all_hounds[..arm.pack_size].to_vec(),
        pack,
        limit: (cfg.duration_s * f64::from(cubarium_voxel_fauna::TICK_HZ)).ceil() as u64,
        tick: 0,
        encounters: BTreeSet::new(),
        tests: 0,
        chases: 0,
        kills: BTreeSet::new(),
        hound_deaths: BTreeSet::new(),
        wounds: BTreeMap::new(),
        abandoned: false,
        outcome: None,
        family_flee_ticks: 0,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    CalfKilled,
    AdultKilled,
    PackGaveUp,
    Timeout,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RowOut {
    pub seed: u64,
    pub arm: String,
    pub composition: Composition,
    pub pack_size: usize,
    pub outcome: Outcome,
    pub time_s: f64,
    /// Peak wound stock per body, including 1 for a recorded kill; summed by roster.
    pub hound_wounds: f64,
    pub hound_worst_wound: f64,
    pub hound_deaths: usize,
    pub calf_wounds: f64,
    pub adult_wounds: f64,
    /// Distinct sensed hound/family pairs, not distance guesses.
    pub encounters: usize,
    /// Chase ended with BrokeOff, after an observed ChaseStart.
    pub tests: u64,
    pub chases: u64,
    /// Today's bite has no holding stage; null explicitly means unavailable.
    pub captures: Option<u64>,
    pub kills: usize,
    pub family_flee_ticks: u64,
}
impl Episode {
    /// Step once and read state/events. Does not infer giving up or death from positions.
    pub fn step(&mut self) -> bool {
        if self.outcome.is_some() {
            return false;
        }
        self.sim.step();
        self.tick += 1;
        let fauna = self.sim.fauna();
        let family: Vec<_> = self.adults.iter().copied().chain(self.calf).collect();
        self.family_flee_ticks += family
            .iter()
            .filter(|&&id| matches!(fauna.steer_of(id), Some(cubarium_voxel_fauna::Steer::Flee)))
            .count() as u64;
        for &id in &self.hounds {
            if let Some(p) = fauna.percept(id) {
                for t in p.tracks() {
                    if t.known && family.contains(&t.id) {
                        self.encounters.insert((id, t.id));
                    }
                }
            }
        }
        for a in fauna.view().animals {
            if family.contains(&a.id) || self.hounds.contains(&a.id) {
                self.wounds
                    .entry(a.id)
                    .and_modify(|w| *w = w.max(a.wound))
                    .or_insert(a.wound);
            }
        }
        if let Some(readout) = fauna.census_readout() {
            for event in &readout.predation {
                if event.kill {
                    if family.contains(&event.prey_id) && event.predator == Founder::Chorister {
                        self.kills.insert(event.prey_id);
                        self.wounds.insert(event.prey_id, 1.0);
                    }
                    if self.hounds.contains(&event.prey_id) {
                        self.hound_deaths.insert(event.prey_id);
                        self.wounds.insert(event.prey_id, 1.0);
                    }
                }
            }
        }
        for event in self.sim.fauna_mut().take_pack_events() {
            match event {
                PackEvent::ChaseStart { pack, target, .. }
                    if pack == self.pack && family.contains(&target) =>
                {
                    self.chases += 1;
                    self.abandoned = false;
                }
                PackEvent::ChaseEnd {
                    pack, target, why, ..
                } if pack == self.pack && family.contains(&target) => {
                    if why == ChaseEnd::BrokeOff {
                        self.tests += 1;
                    }
                    self.abandoned = matches!(
                        why,
                        ChaseEnd::Late
                            | ChaseEnd::Spent
                            | ChaseEnd::Lost
                            | ChaseEnd::BrokeOff
                            | ChaseEnd::Trespass
                    );
                }
                _ => {}
            }
        }
        let fauna = self.sim.fauna();
        let eating_kill = fauna.pack(self.pack).is_some_and(|p| p.feast_eaten > 0.0)
            || fauna.view().ledger.funnel[Founder::Chorister.index()][Founder::Loftstrider.index()]
                [cubarium_voxel_fauna::perception::EATEN]
                > 0;
        if eating_kill && !self.kills.is_empty() {
            self.outcome = Some(if self.calf.is_some_and(|id| self.kills.contains(&id)) {
                Outcome::CalfKilled
            } else {
                Outcome::AdultKilled
            });
        } else if self
            .hounds
            .iter()
            .all(|&id| fauna.view().animal(id).is_none())
            || (self.abandoned
                && self.hounds.iter().all(|&id| {
                    fauna.view().animal(id).is_none()
                        || fauna.view().animal(id).is_some_and(|a| a.homing_ticks > 0)
                }))
        {
            self.outcome = Some(Outcome::PackGaveUp);
        } else if self.tick >= self.limit {
            self.outcome = Some(Outcome::Timeout);
        }
        true
    }
    pub fn done(&self) -> bool {
        self.outcome.is_some()
    }
    pub fn row(&self) -> RowOut {
        let wound = |id: &u64| self.wounds.get(id).copied().unwrap_or(0.0);
        RowOut {
            seed: self.seed,
            arm: self.arm.name.clone(),
            composition: self.arm.composition,
            pack_size: self.arm.pack_size,
            outcome: self.outcome.unwrap_or(Outcome::Timeout),
            time_s: self.tick as f64 / f64::from(cubarium_voxel_fauna::TICK_HZ),
            hound_wounds: self.hounds.iter().map(wound).sum(),
            hound_worst_wound: self.hounds.iter().map(wound).fold(0.0, f64::max),
            hound_deaths: cubarium_voxel_fauna::Departure::ALL
                .iter()
                .filter(|cause| cause.is_death())
                .map(|&cause| {
                    self.sim
                        .fauna()
                        .view()
                        .ledger
                        .departed_founder(Founder::Chorister, cause) as usize
                })
                .sum(),
            calf_wounds: self.calf.as_ref().map(wound).unwrap_or(0.0),
            adult_wounds: self.adults.iter().map(wound).sum(),
            encounters: self.encounters.len(),
            tests: self.tests,
            chases: self.chases,
            captures: None,
            kills: self.kills.len(),
            family_flee_ticks: self.family_flee_ticks,
        }
    }
}
pub fn run(cfg: &Config, seed: u64, arm: &Arm) -> Result<RowOut, String> {
    let mut episode = found(cfg, seed, arm)?;
    while episode.step() {}
    Ok(episode.row())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Config {
        Config {
            duration_s: 2.0,
            episodes: 1,
            width: 64,
            depth: 40,
            height: 24,
            distance_m: [1.0, 1.5],
            spacing_m: 0.5,
            depth_jitter_m: 0.0,
            ..Config::default()
        }
    }
    #[test]
    fn short_episode_founds_and_writes_a_row() {
        let cfg = fixture();
        let mut episode = found(&cfg, 73, &cfg.arms[0]).unwrap();
        let calf = episode
            .sim
            .fauna()
            .view()
            .animal(episode.calf.unwrap())
            .unwrap();
        assert_eq!(calf.mother, episode.adults.first().copied());
        assert_eq!(episode.sim.fauna().packs().len(), 1);
        for &id in episode
            .adults
            .iter()
            .chain(episode.calf.iter())
            .chain(&episode.hounds)
        {
            assert!(episode.sim.fauna().has_controller(id));
        }
        let id = episode.adults[0];
        let body = episode.sim.fauna().view().animal(id).unwrap();
        let remaining = body.lifespan_ticks.saturating_sub(body.age_ticks);
        let age = body.age_ticks + 1;
        assert!(episode.sim.fauna_mut().stage_identity(id, age, None));
        let body = episode.sim.fauna().view().animal(id).unwrap();
        if body.lifespan_ticks != 0 {
            assert_eq!(body.lifespan_ticks - body.age_ticks, remaining);
        }
        while episode.step() {}
        assert!(!episode.sim.fauna_mut().stage_identity(id, age, None));
        let text = serde_json::to_string(&episode.row()).unwrap();
        let row: RowOut = serde_json::from_str(&text).unwrap();
        assert_eq!(row.seed, 73);
        assert!(row.time_s <= cfg.duration_s + cubarium_voxel_fauna::DT);
        assert_eq!(row.pack_size, cfg.arms[0].pack_size);
        assert_eq!(row.captures, None);
    }
    #[test]
    fn different_arms_share_all_common_starting_bodies() {
        let cfg = fixture();
        let a = found(&cfg, 73, &cfg.arms[0]).unwrap();
        let b = found(&cfg, 73, &cfg.arms[4]).unwrap();
        assert_eq!(a.sim.world().tick(), 0);
        assert_eq!(b.sim.world().tick(), 0);
        let common: Vec<_> = b
            .sim
            .fauna()
            .view()
            .animals
            .iter()
            .filter_map(|body| {
                a.sim
                    .fauna()
                    .view()
                    .animal(body.id)
                    .map(|other| (body, other))
            })
            .collect();
        assert!(!common.is_empty());
        for (body, other) in common {
            assert_eq!(body.pose, other.pose);
            assert_eq!(body.energy, other.energy);
            assert_eq!(body.reserve, other.reserve);
            assert_eq!(body.age_ticks, other.age_ticks);
            assert_eq!(body.stomach, other.stomach);
        }
    }
    #[test]
    fn harness_registry_covers_numeric_settings_and_refuses_invalid_input() {
        let mut cfg = fixture();
        cfg.validate().unwrap();
        cfg.pack_fullness = [0.0, 2.0];
        assert!(cfg.validate().is_err());
    }
}
