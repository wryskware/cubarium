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
    /// Metres of clear ground between neighbouring starting bodies in a group (BALANCE): each
    /// body counts as the circle its length sweeps (half its length about its centre), so no two
    /// start overlapping at any heading, whatever the lineage's size.
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
            ("spacing_m", "m", Range::nonneg(), "Clear gap between neighbouring bodies"),
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
        let room = self.distance_m[1] + self.depth_jitter_m;
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
    family_defend_ticks: u64,
    calf_chases: u64,
    adult_chases: u64,
    ends: BTreeMap<String, u64>,
    roam: RoamRead,
}

/// YOUNG-ROAM's readout of one episode (the calf and its mother).
#[derive(Clone, Debug, Default)]
struct RoamRead {
    explore_ticks: u64,
    answer_ticks: u64,
    far_m: f64,
    /// The tick of the calf's last unanswered bleat (its excursion's end), where the calf
    /// was and how far its mother was from there.
    bleat: Option<(u64, f64, f64, f64)>,
    mother_approach_m: Vec<f64>,
    bleats_seen: u64,
    reunions_s: Vec<f64>,
    chase_from_mother_m: Vec<f64>,
}

/// The calf's distance to its mother now, metres on the ring (`None`: either gone).
fn calf_to_mother(fauna: &cubarium_voxel_fauna::Fauna, calf: Option<u64>, ring: f64) -> Option<f64> {
    let view = fauna.view();
    let c = view.animal(calf?)?;
    let m = view.animal(c.mother?)?;
    let mut dx = (m.pose.x - c.pose.x).rem_euclid(ring);
    if dx > 0.5 * ring {
        dx -= ring;
    }
    Some(dx.hypot(m.pose.z - c.pose.z))
}

/// Offsets along a row, centred on its middle, for bodies of radii `radii` in order, each
/// `gap` metres of clear ground from the next.
fn row_offsets(radii: &[f64], gap: f64) -> Vec<f64> {
    let mut at = Vec::with_capacity(radii.len());
    let mut x = 0.0;
    for (k, &r) in radii.iter().enumerate() {
        if k > 0 {
            x += radii[k - 1] + r + gap;
        }
        at.push(x);
    }
    let (lo, hi) = match (radii.first(), radii.last(), at.last()) {
        (Some(r0), Some(rn), Some(&xn)) => (-r0, xn + rn),
        _ => (0.0, 0.0),
    };
    let mid = 0.5 * (lo + hi);
    at.iter().map(|x| x - mid).collect()
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
    // The arm's settings first, so bodies are placed at the sizes they will have.
    let params = crate::params::Params::resolve_over(
        crate::params::Params {
            world: world.config().clone(),
            flora: flora.config().clone(),
            fauna: FaunaConfig::default(),
        },
        &crate::params::Layers {
            sets: arm.sets.clone(),
            ..Default::default()
        },
    )?;
    if params.world != *world.config() || params.flora != *flora.config() {
        return Err("engagement arms may override fauna only".into());
    }
    let fauna_config = params.fauna;
    let mut fauna = Fauna::new(fauna_config.clone());
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
        // Inside the closed depth: a row longer than a small arena's room stands at its edge.
        z: ((z / cfg.voxel_m).floor().max(0.0) as u32).min(cfg.depth.saturating_sub(1)),
    };
    let family_bodies: Vec<f64> = (0..3u64)
        .map(|slot| draw(seed, 20 + slot, if slot == 2 { cfg.calf_body } else { cfg.adult_body }))
        .collect();
    let ls = fauna_config.founder(Founder::Loftstrider);
    let family_sides = row_offsets(
        &family_bodies
            .iter()
            .map(|&b| 0.5 * ls.body_at(b * ls.core.body_max).length_m)
            .collect::<Vec<_>>(),
        cfg.spacing_m,
    );
    let mut family = vec![];
    for slot in 0..3u64 {
        let side = family_sides[slot as usize];
        let site = face(cx + angle.cos() * side, cz - angle.sin() * side);
        let fullness = draw(seed, 10 + slot, cfg.family_fullness);
        let body = family_bodies[slot as usize];
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
    let hound_sides = row_offsets(
        &all_hounds
            .iter()
            .map(|&id| {
                fauna
                    .view()
                    .animal(id)
                    .and_then(|a| cubarium_voxel_fauna::body_of(&fauna_config, a))
                    .map_or(0.0, |b| 0.5 * b.length_m)
            })
            .collect::<Vec<_>>(),
        cfg.spacing_m,
    );
    for (slot, &id) in all_hounds.iter().enumerate() {
        let side = hound_sides[slot];
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
    fauna.replace_config(fauna_config)?;
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
        family_defend_ticks: 0,
        calf_chases: 0,
        adult_chases: 0,
        ends: BTreeMap::new(),
        roam: RoamRead::default(),
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
    /// LS-DEFEND: family body-ticks steered by the defence layer, kicks landed by the
    /// family, and kick-backs drawn by landed bites.
    #[serde(default)]
    pub family_defend_ticks: u64,
    #[serde(default)]
    pub kicks: u64,
    #[serde(default)]
    pub kick_backs: u64,
    /// VH-TACTICS readouts (the fauna ledger; zero on the old tactic): tests begun, tests
    /// that ended in a commitment, hunts given up (a test broken off or a chase given up),
    /// strikes on a substituted young body, lunges that met a body before their prey.
    #[serde(default)]
    pub hound_tests: u64,
    #[serde(default)]
    pub test_commits: u64,
    #[serde(default)]
    pub give_ups: u64,
    #[serde(default)]
    pub substitutions: u64,
    #[serde(default)]
    pub lunges_blocked: u64,
    /// Chases started on the calf and on an adult (ChaseStart events).
    #[serde(default)]
    pub calf_chases: u64,
    #[serde(default)]
    pub adult_chases: u64,
    /// The pack's ChaseEnd events on the family by reason (`BrokeOff`, `Spent`, `Late`,
    /// `Lost`, ...): under VH-TACTICS a value give-up or a test broken off logs `BrokeOff`, a
    /// wounded or winded hound `Spent`, a stalled chase `Late`.
    #[serde(default)]
    pub ends: BTreeMap<String, u64>,
    /// YOUNG-ROAM readouts (the loftstrider's session tally and the calf and its mother):
    /// excursions started by goal (food, animal, place), ended by reason, the bleats at their
    /// ends, the calf's ticks out on one and its mother's answering one, the farthest the calf
    /// got from its mother, the seconds from each bleat until its mother was within the calf's
    /// birth comfort (`young.follow_m`), and the calf's distance to its mother at each chase
    /// started on it.
    #[serde(default)]
    pub excursions: [u64; 3],
    #[serde(default)]
    pub excursion_ends: BTreeMap<String, u64>,
    #[serde(default)]
    pub bleats: u64,
    #[serde(default)]
    pub explore_ticks: u64,
    #[serde(default)]
    pub answer_ticks: u64,
    #[serde(default)]
    pub calf_far_m: f64,
    #[serde(default)]
    pub reunions_s: Vec<f64>,
    #[serde(default)]
    pub calf_chase_from_mother_m: Vec<f64>,
    /// For each reunion, how much nearer the bleat's point the mother came (her share of the
    /// closing), metres.
    #[serde(default)]
    pub mother_approach_m: Vec<f64>,
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
        self.family_defend_ticks += family
            .iter()
            .filter(|&&id| matches!(fauna.steer_of(id), Some(cubarium_voxel_fauna::Steer::Defend)))
            .count() as u64;
        self.family_flee_ticks += family
            .iter()
            .filter(|&&id| matches!(fauna.steer_of(id), Some(cubarium_voxel_fauna::Steer::Flee)))
            .count() as u64;
        // YOUNG-ROAM: the calf's excursions, its bleats and its mother's coming.
        {
            let w = self.sim.world().config();
            let ring = f64::from(w.width) * w.voxel_m;
            let calf_mother = self.calf.and_then(|c| fauna.view().animal(c)).and_then(|c| c.mother);
            if let Some(c) = self.calf {
                if matches!(fauna.steer_of(c), Some(cubarium_voxel_fauna::Steer::Explore)) {
                    self.roam.explore_ticks += 1;
                }
            }
            if calf_mother.is_some_and(|m| matches!(fauna.steer_of(m), Some(cubarium_voxel_fauna::Steer::Answer))) {
                self.roam.answer_ticks += 1;
            }
            let d = calf_to_mother(fauna, self.calf, ring);
            if let Some(d) = d {
                self.roam.far_m = self.roam.far_m.max(d);
            }
            // From the calf's bleat point to its mother, now.
            let to_point = |x: f64, z: f64| -> Option<f64> {
                let m = fauna.view().animal(calf_mother?)?;
                let mut dx = (m.pose.x - x).rem_euclid(ring);
                if dx > 0.5 * ring {
                    dx -= ring;
                }
                Some(dx.hypot(m.pose.z - z))
            };
            let bleats = fauna.roam_tally(Founder::Loftstrider).bleats;
            if bleats > self.roam.bleats_seen {
                self.roam.bleats_seen = bleats;
                if self.roam.bleat.is_none() {
                    if let Some(c) = self.calf.and_then(|c| fauna.view().animal(c)) {
                        let (x, z) = (c.pose.x, c.pose.z);
                        if let Some(dm) = to_point(x, z) {
                            self.roam.bleat = Some((self.tick, x, z, dm));
                        }
                    }
                }
            }
            let comfort = fauna.view().config.founder(Founder::Loftstrider).young.follow_m;
            if let (Some((at, x, z, dm)), Some(d)) = (self.roam.bleat, d) {
                if d <= comfort {
                    self.roam.reunions_s.push((self.tick - at) as f64 / f64::from(cubarium_voxel_fauna::TICK_HZ));
                    self.roam.mother_approach_m.push(dm - to_point(x, z).unwrap_or(dm));
                    self.roam.bleat = None;
                }
            }
        }
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
                    if self.calf == Some(target) {
                        self.calf_chases += 1;
                        let w = self.sim.world().config();
                        let ring = f64::from(w.width) * w.voxel_m;
                        if let Some(d) = calf_to_mother(self.sim.fauna(), self.calf, ring) {
                            self.roam.chase_from_mother_m.push(d);
                        }
                    } else {
                        self.adult_chases += 1;
                    }
                    self.abandoned = false;
                }
                PackEvent::ChaseEnd {
                    pack, target, why, ..
                } if pack == self.pack && family.contains(&target) => {
                    if why == ChaseEnd::BrokeOff {
                        self.tests += 1;
                    }
                    *self.ends.entry(format!("{why:?}")).or_default() += 1;
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
        let eating_kill = fauna.view().ledger.funnel[Founder::Chorister.index()]
            [Founder::Loftstrider.index()][cubarium_voxel_fauna::perception::EATEN]
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
            || (self.abandoned && self.hounds.iter().all(|&id| self.withdrawn(id)))
        {
            self.outcome = Some(Outcome::PackGaveUp);
        } else if self.tick >= self.limit {
            self.outcome = Some(Outcome::Timeout);
        }
        true
    }
    /// **A hound withdrawn from the family** (the give-up read): gone; or homeward
    /// (`homing_ticks`); or, under VH-TACTICS, hunting no family member and in the rest after
    /// a give-up (its memory's `gave_up_tick` within `rest_s`), at home or walked off.
    fn withdrawn(&self, id: u64) -> bool {
        let fauna = self.sim.fauna();
        let view = fauna.view();
        let Some(a) = view.animal(id) else { return true };
        if a.homing_ticks > 0 {
            return true;
        }
        let family: Vec<u64> = self.adults.iter().copied().chain(self.calf).collect();
        let rest_s = view
            .config
            .founder(Founder::Chorister)
            .pack
            .map_or(0.0, |p| p.tactics.rest_s);
        fauna.hound(id).is_some_and(|m| {
            m.gave_up_tick > 0
                && !m.target.is_some_and(|t| family.contains(&t))
                && (view.tick.saturating_sub(m.gave_up_tick) as f64
                    / f64::from(cubarium_voxel_fauna::TICK_HZ))
                    < rest_s
        })
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
            family_defend_ticks: self.family_defend_ticks,
            kicks: self.sim.fauna().view().ledger.kicks_landed_by_founder[Founder::Loftstrider.index()],
            kick_backs: self.sim.fauna().view().ledger.kick_backs,
            hound_tests: self.sim.fauna().view().ledger.hound_tests,
            test_commits: self.sim.fauna().view().ledger.hound_test_commits,
            give_ups: self.sim.fauna().view().ledger.hound_give_ups,
            substitutions: self.sim.fauna().view().ledger.hound_substitutions,
            lunges_blocked: self.sim.fauna().view().ledger.lunges_blocked,
            calf_chases: self.calf_chases,
            adult_chases: self.adult_chases,
            ends: self.ends.clone(),
            excursions: self.sim.fauna().roam_tally(Founder::Loftstrider).started,
            excursion_ends: {
                let t = self.sim.fauna().roam_tally(Founder::Loftstrider);
                cubarium_voxel_fauna::roam::End::ALL.iter().map(|e| (e.name().to_string(), t.ended[e.index()])).collect()
            },
            bleats: self.sim.fauna().roam_tally(Founder::Loftstrider).bleats,
            explore_ticks: self.roam.explore_ticks,
            answer_ticks: self.roam.answer_ticks,
            calf_far_m: self.roam.far_m,
            reunions_s: self.roam.reunions_s.clone(),
            calf_chase_from_mother_m: self.roam.chase_from_mother_m.clone(),
            mother_approach_m: self.roam.mother_approach_m.clone(),
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
    /// **Family and pack bodies start clear of each other** (BALANCE step 1), whatever the
    /// lineages' sizes: the circles their lengths sweep, `spacing_m` apart at least.
    #[test]
    fn starting_bodies_do_not_overlap_for_any_body_size() {
        let base = Config { width: 160, depth: 160, distance_m: [10.0, 14.0], ..fixture() };
        let d = FaunaConfig::default();
        let ls = d.founder(Founder::Loftstrider).adult_length_m;
        let vh = d.founder(Founder::Chorister).adult_length_m;
        for scale in [0.5, 1.0, 2.0] {
            let arm = Arm {
                name: "family-4".into(),
                composition: Composition::Family,
                pack_size: 4,
                sets: vec![
                    format!("fauna.founders.loftstrider.adult_length_m={}", scale * ls),
                    format!("fauna.founders.chorister.adult_length_m={}", scale * vh),
                ],
            };
            for seed in 1..=4 {
                let ep = found(&base, seed, &arm).unwrap();
                let view = ep.sim.fauna().view();
                let config = view.config;
                let bodies: Vec<_> = ep.adults.iter().chain(ep.calf.iter()).chain(&ep.hounds).filter_map(|&id| view.animal(id)).collect();
                for (k, a) in bodies.iter().enumerate() {
                    for b in &bodies[k + 1..] {
                        let ra = 0.5 * cubarium_voxel_fauna::body_of(config, a).unwrap().length_m;
                        let rb = 0.5 * cubarium_voxel_fauna::body_of(config, b).unwrap().length_m;
                        let gap = (a.pose.x - b.pose.x).hypot(a.pose.z - b.pose.z) - ra - rb;
                        // A body stands at its voxel's centre: up to half a voxel's diagonal off.
                        let snap = base.voxel_m * std::f64::consts::SQRT_2;
                        assert!(gap >= base.spacing_m - snap, "scale {scale} seed {seed}: bodies {} and {} {gap:.2} m apart", a.id, b.id);
                    }
                }
            }
        }
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
    /// **The give-up read** (VH-TACTICS): a pack whose hounds give a hunt up on the first
    /// sample (`give_up_value` above any value, `commit_value` below) ends the episode as
    /// gave-up once every hound is withdrawn, with the give-ups on the row.
    #[test]
    fn a_pack_that_gives_up_ends_the_episode_as_gave_up() {
        let mut cfg = fixture();
        cfg.duration_s = 30.0;
        cfg.distance_m = [2.0, 2.5];
        let mut arm = cfg
            .arms
            .iter()
            .find(|a| a.composition == Composition::Calf)
            .cloned()
            .expect("a lone-calf arm");
        arm.sets = vec![
            "fauna.founders.chorister.pack.tactics.give_up_value=1e9".into(),
            "fauna.founders.chorister.pack.tactics.commit_value=-1e9".into(),
        ];
        let row = run(&cfg, 73, &arm).unwrap();
        assert!(row.give_ups >= 1, "{row:?}");
        assert_eq!(row.outcome, Outcome::PackGaveUp, "{row:?}");
        assert_eq!(row.kills, 0);
    }
    #[test]
    fn harness_registry_covers_numeric_settings_and_refuses_invalid_input() {
        let mut cfg = fixture();
        cfg.validate().unwrap();
        cfg.pack_fullness = [0.0, 2.0];
        assert!(cfg.validate().is_err());
    }
}
