//! **Staged mode scenes** (BRAIN-NEXT step 3; `design/behaviour-ai-2026-10-11.md` §10.2,
//! §11.3 "Cloning data"): small seeded scenes, each built so one seedporter or lanternjaw
//! mode arises **naturally** under today's heuristics — night for the roost, a larva on a
//! troopmate for grooming, a young with its mother, a threat, a carcass, a larger animal that
//! hunts it at the carcass, a rest hour on open ground. They record teacher streams in modes the live training worlds
//! rarely show, and stage the per-mode competence checks.
//!
//! Setup and readout only, as ENGAGE's harness ([`crate::engagement`]): a bare soil arena,
//! terrain and flora frozen ([`Sim::new_static`]), the day clock set to the scene's hour,
//! every body on its heuristic. Placements come from the seed alone (one hashed draw per
//! body and quantity), so two calls with one seed give one start. No hunger, flight, roost,
//! grooming or nursing rule is changed.

use crate::{Sim, SimConfig};
use cubarium_voxel::registry::{Range, Registry, Row};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as WorldConfig, Material, World};
use cubarium_voxel_fauna::{BodySpec, BodyStage, Fauna, FaunaConfig, Founder, Group, Placed, Spot};
use cubarium_voxel_flora::{Carcass, Command as FloraCommand, Deposit, DepositKind, Flora, FloraConfig, Site, Species};
use serde::{Deserialize, Serialize};

/// One staged scene: the mode it stages and the lineage that shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeScene {
    /// Seedporters at night with a climbable crown near: `roost`.
    Roost,
    /// A troop by day, one carrying a lanternjaw larva: `groom`.
    Groom,
    /// A hungry young seedporter some way from its mother: `follow_mother`.
    FollowMother,
    /// A troop by day near a crown, a hungry Veilhound pair approaching: `flee_to_trees`.
    FleeToTrees,
    /// Seedporters at night on open ground, no crown in reach: `rest`.
    SeedporterRest,
    /// A hungry lanternjaw at night with its mouth over a carcass: `feed`.
    Feed,
    /// A hungry young lanternjaw at a carcass with a Veilhound (a larger body that hunts it)
    /// standing by: `give_way`.
    GiveWay,
}

impl ModeScene {
    pub const ALL: [ModeScene; 7] = [
        ModeScene::Roost,
        ModeScene::Groom,
        ModeScene::FollowMother,
        ModeScene::FleeToTrees,
        ModeScene::SeedporterRest,
        ModeScene::Feed,
        ModeScene::GiveWay,
    ];

    /// The lineage whose mode it stages.
    pub fn founder(self) -> Founder {
        match self {
            ModeScene::Roost | ModeScene::Groom | ModeScene::FollowMother | ModeScene::FleeToTrees | ModeScene::SeedporterRest => {
                Founder::Seedporter
            }
            ModeScene::Feed | ModeScene::GiveWay => Founder::Lanternjaw,
        }
    }

    /// The menu intent it stages (`cubarium_voxel_fauna::intent::menu_of`).
    pub fn intent(self) -> &'static str {
        match self {
            ModeScene::Roost => "roost",
            ModeScene::Groom => "groom",
            ModeScene::FollowMother => "follow_mother",
            ModeScene::FleeToTrees => "flee_to_trees",
            ModeScene::SeedporterRest => "rest",
            ModeScene::Feed => "feed",
            ModeScene::GiveWay => "give_way",
        }
    }

    /// A file-safe name.
    pub fn name(self) -> &'static str {
        match self {
            ModeScene::Roost => "roost",
            ModeScene::Groom => "groom",
            ModeScene::FollowMother => "follow_mother",
            ModeScene::FleeToTrees => "flee_to_trees",
            ModeScene::SeedporterRest => "seedporter_rest",
            ModeScene::Feed => "feed",
            ModeScene::GiveWay => "give_way",
        }
    }

    pub fn parse(s: &str) -> Option<ModeScene> {
        ModeScene::ALL.into_iter().find(|m| m.name() == s)
    }

    /// The scenes of one lineage.
    pub fn of(founder: Founder) -> Vec<ModeScene> {
        ModeScene::ALL.into_iter().filter(|m| m.founder() == founder).collect()
    }

    fn night(self) -> bool {
        matches!(self, ModeScene::Roost | ModeScene::SeedporterRest | ModeScene::Feed | ModeScene::GiveWay)
    }
}

/// The scenes' knobs (`mode_scenes.*`): arena, hours, placements and stores.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Simulated seconds per scene, at most 300 s.
    pub duration_s: f64,
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
    /// The clock's hour at a night scene's start (24-hour clock of the cycle's day).
    pub night_hour: f64,
    /// The clock's hour at a day scene's start.
    pub day_hour: f64,
    /// Seedporters in a troop scene.
    pub troop: usize,
    /// Metres between adjacent starting bodies.
    pub spacing_m: f64,
    /// Metres of each body's placement jitter, each side.
    pub jitter_m: f64,
    /// Metres from the troop's first body to its crown (roost, flight to the trees).
    pub stand_distance_m: f64,
    /// Metres the crown stands above the lineage's least climbable, safe crown.
    pub crown_margin_m: f64,
    /// Metres from the troop to the Veilhound pair.
    pub threat_distance_m: f64,
    /// Hounds in the threat.
    pub hounds: usize,
    /// The hounds' reserve and stomach, as a fraction of capacity (hungry: they hunt).
    pub hound_fullness: f64,
    /// Metres from the mother to her young.
    pub mother_distance_m: f64,
    /// The young's age at the start, seconds (a dependent young).
    pub young_age_s: f64,
    /// The young's reserve and stomach, as a fraction of capacity (hungry: it goes to her).
    pub young_fullness: f64,
    /// The lanternjaw's reserve and stomach at a carcass, as a fraction of capacity.
    pub hunter_fullness: f64,
    /// The carcass's organic matter, kg.
    pub carcass_organic: f64,
    /// Metres from the carcass to the Veilhound standing by it.
    pub big_distance_m: f64,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            duration_s: 60.0,
            width: 96,
            height: 40,
            depth: 24,
            voxel_m: 0.25,
            soil_layers: 2,
            night_hour: 0.0,
            day_hour: 12.0,
            troop: 4,
            spacing_m: 0.75,
            jitter_m: 0.2,
            stand_distance_m: 2.5,
            crown_margin_m: 0.5,
            threat_distance_m: 6.0,
            hounds: 2,
            hound_fullness: 0.2,
            mother_distance_m: 3.0,
            young_age_s: 60.0,
            young_fullness: 0.2,
            hunter_fullness: 0.2,
            carcass_organic: 0.5,
            big_distance_m: 1.0,
        }
    }
}

/// The scenes' registry rows (root `mode_scenes`).
pub fn registry() -> &'static Registry {
    static TABLE: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let rows = [
            ("duration_s", "s", Range::closed(0.0, 300.0).open_lo(), "BRAIN-NEXT mode scenes: simulated seconds per staged scene"),
            ("width", "voxel", Range::at_least(1.0), "BRAIN-NEXT mode scenes: periodic arena width"),
            ("height", "voxel", Range::at_least(1.0), "BRAIN-NEXT mode scenes: arena height"),
            ("depth", "voxel", Range::at_least(1.0), "BRAIN-NEXT mode scenes: arena depth"),
            ("voxel_m", "m", Range::positive(), "BRAIN-NEXT mode scenes: voxel pitch"),
            ("soil_layers", "voxel", Range::at_least(1.0), "BRAIN-NEXT mode scenes: soil thickness"),
            ("night_hour", "h", Range::closed(0.0, 24.0), "BRAIN-NEXT mode scenes: the clock's hour at a night scene's start (roost, seedporter rest, feed, give way)"),
            ("day_hour", "h", Range::closed(0.0, 24.0), "BRAIN-NEXT mode scenes: the clock's hour at a day scene's start (groom, follow mother, flee to trees)"),
            ("troop", "count", Range::at_least(2.0), "BRAIN-NEXT mode scenes: seedporters in a troop scene"),
            ("spacing_m", "m", Range::positive(), "BRAIN-NEXT mode scenes: distance between adjacent starting bodies"),
            ("jitter_m", "m", Range::nonneg(), "BRAIN-NEXT mode scenes: each body's seeded placement jitter, each side"),
            ("stand_distance_m", "m", Range::positive(), "BRAIN-NEXT mode scenes: troop to its crown (roost, flight to the trees)"),
            ("crown_margin_m", "m", Range::nonneg(), "BRAIN-NEXT mode scenes: the crown's height over the lineage's least climbable safe crown"),
            ("threat_distance_m", "m", Range::positive(), "BRAIN-NEXT mode scenes: troop to the Veilhound pair (flight to the trees)"),
            ("hounds", "count", Range::closed(1.0, 4.0), "BRAIN-NEXT mode scenes: hounds in the threat"),
            ("hound_fullness", "fraction", Range::fraction(), "BRAIN-NEXT mode scenes: the hounds' reserve and stomach (flee to the trees, give way)"),
            ("mother_distance_m", "m", Range::positive(), "BRAIN-NEXT mode scenes: mother to her young (follow mother)"),
            ("young_age_s", "s", Range::nonneg(), "BRAIN-NEXT mode scenes: the young's age at the start (follow mother)"),
            ("young_fullness", "fraction", Range::fraction(), "BRAIN-NEXT mode scenes: the young's reserve and stomach (follow mother)"),
            ("hunter_fullness", "fraction", Range::fraction(), "BRAIN-NEXT mode scenes: the lanternjaw's reserve and stomach at its carcass (feed, give way)"),
            ("carcass_organic", "kg", Range::positive(), "BRAIN-NEXT mode scenes: the carcass's organic matter (feed, give way)"),
            ("big_distance_m", "m", Range::nonneg(), "BRAIN-NEXT mode scenes: carcass to the Veilhound standing by it (give way)"),
        ]
        .into_iter()
        .map(|(k, unit, range, what)| Row::new(format!("mode_scenes.{k}"), unit, range, what))
        .collect();
        Registry::new("mode_scenes", rows, vec![], vec![])
    })
}

impl Config {
    pub fn read(path: &std::path::Path) -> Result<Config, String> {
        let cfg: Config = toml::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<(), String> {
        registry().check(self)?;
        if self.soil_layers + 1 >= self.height {
            return Err("mode scenes: soil above the ceiling".into());
        }
        let span = self.stand_distance_m.max(self.threat_distance_m).max(self.mother_distance_m)
            + self.spacing_m * self.troop as f64
            + 2.0 * self.jitter_m;
        if span >= f64::from(self.width) * self.voxel_m / 2.0 || 2.0 * self.jitter_m + self.spacing_m >= f64::from(self.depth) * self.voxel_m / 2.0 {
            return Err("mode scenes: the arena needs more room around the seeded groups".into());
        }
        Ok(())
    }
}

// One hashed draw per body and quantity, as ENGAGE's.
fn draw(seed: u64, key: u64, range: [f64; 2]) -> f64 {
    let mut x = seed.wrapping_add(key.wrapping_mul(0x9e3779b97f4a7c15));
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^= x >> 31;
    range[0] + (range[1] - range[0]) * ((x >> 11) as f64 / ((1u64 << 53) as f64))
}

/// One staged scene, running.
pub struct Episode {
    pub sim: Sim,
    pub scene: ModeScene,
    pub seed: u64,
    /// The bodies expected to show the mode (the groomers, the young, the troop, the
    /// lanternjaw).
    pub focal: Vec<u64>,
    /// Every body of the scene's lineage.
    pub lineage: Vec<u64>,
    /// The mother (follow mother), the larva's host (groom), the hounds (flee to the trees),
    /// the Veilhound (give way).
    pub others: Vec<u64>,
    /// The crown's site (roost, flight to the trees) or the carcass's (feed, give way).
    pub site: Option<Site>,
    limit: u64,
    tick: u64,
}

impl Episode {
    /// One tick; `false` (without stepping) once the scene's time is up.
    pub fn step(&mut self) -> bool {
        if self.tick >= self.limit {
            return false;
        }
        self.sim.step();
        self.tick += 1;
        true
    }

    pub fn done(&self) -> bool {
        self.tick >= self.limit
    }

    /// Ticks run.
    pub fn ticks(&self) -> u64 {
        self.tick
    }

    /// The scene's length, ticks.
    pub fn limit(&self) -> u64 {
        self.limit
    }
}

struct Builder<'a> {
    cfg: &'a Config,
    world: World,
    flora: Flora,
    fauna: Fauna,
    seed: u64,
    key: u64,
}

impl Builder<'_> {
    fn face(&self, x: f64, z: f64) -> Site {
        let c = self.cfg;
        Site {
            x: (x / c.voxel_m).floor().rem_euclid(f64::from(c.width)) as u32,
            y: c.soil_layers,
            z: ((z / c.voxel_m).floor().max(0.0) as u32).min(c.depth - 1),
        }
    }

    /// A seeded spot near `(x, z)`.
    fn near(&mut self, x: f64, z: f64) -> Site {
        let j = self.cfg.jitter_m;
        self.key += 2;
        let (dx, dz) = (draw(self.seed, self.key, [-j, j]), draw(self.seed, self.key + 1, [-j, j]));
        self.face(x + dx, z + dz)
    }

    fn place(&mut self, founder: Founder, site: Site, stage: BodyStage, fullness: Option<f64>, group: Group) -> Result<(u64, Option<u64>), String> {
        self.key += 1;
        let heading_rad = draw(self.seed, self.key, [0.0, std::f64::consts::TAU]);
        match self.fauna.place_body(&self.world, &self.flora, founder, Spot::Face(site), BodySpec { stage, fullness, group, heading_rad })? {
            Placed::Body { id, pack } => Ok((id, pack)),
            Placed::Eggs { .. } => Err(format!("{}: placed eggs, not a body", founder.name())),
        }
    }

    /// A crown the lineage climbs and is safe in, at `(x, z)`.
    fn crown(&mut self, founder: Founder, x: f64, z: f64) -> Result<Site, String> {
        let site = self.face(x, z);
        let min = self.fauna.refuge_crown_m(founder).ok_or("a crown scene needs a glider lineage")? + self.cfg.crown_margin_m;
        let sc = self.flora.config().species(Species::Vaulttree).clone();
        let mut wood = sc.wood_max;
        while sc.crown_height_m_at(wood * 0.9) >= min && wood > 1e-9 {
            wood *= 0.9;
        }
        if sc.crown_height_m_at(wood) < min {
            return Err("no vaulttree grows a safe crown".into());
        }
        if !self.flora.apply(&self.world, FloraCommand::Seed { x: i64::from(site.x), z: site.z, species: Species::Vaulttree, wood }) {
            return Err("crown placement refused".into());
        }
        let stand = self.flora.view().stands.iter().find(|s| s.site.x == site.x && s.site.z == site.z).map(|s| s.site);
        stand.ok_or_else(|| "the crown did not stand".into())
    }
}

/// **Found one staged scene** from `seed`.
pub fn found(cfg: &Config, scene: ModeScene, seed: u64) -> Result<Episode, String> {
    cfg.validate()?;
    let day = DayClock {
        mode: DayMode::Cycle,
        start_hour: if scene.night() { cfg.night_hour } else { cfg.day_hour },
        ..DayClock::default()
    };
    let mut world = World::empty(WorldConfig {
        width: cfg.width,
        height: cfg.height,
        depth: cfg.depth,
        voxel_m: cfg.voxel_m,
        seed,
        day,
        ..WorldConfig::default()
    });
    for z in 0..cfg.depth {
        for x in 0..i64::from(cfg.width) {
            for y in 1..=cfg.soil_layers {
                world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    let mut b = Builder {
        cfg,
        world,
        flora: Flora::new(FloraConfig::for_voxel_size(cfg.voxel_m)),
        fauna: Fauna::new(FaunaConfig::default()),
        seed,
        key: 100,
    };
    b.fauna.set_births_enabled(false);
    let x0 = draw(seed, 1, [0.0, f64::from(cfg.width) * cfg.voxel_m]);
    let z0 = f64::from(cfg.depth) * cfg.voxel_m / 2.0;
    let founder = scene.founder();
    let (mut lineage, mut others, mut site): (Vec<u64>, Vec<u64>, Option<Site>) = (Vec::new(), Vec::new(), None);
    let troop = |b: &mut Builder<'_>, n: usize, lineage: &mut Vec<u64>| -> Result<(), String> {
        for k in 0..n {
            let s = b.near(x0 - k as f64 * cfg.spacing_m, z0);
            lineage.push(b.place(Founder::Seedporter, s, BodyStage::Adult, None, Group::Auto)?.0);
        }
        Ok(())
    };
    let focal = match scene {
        ModeScene::Roost => {
            site = Some(b.crown(founder, x0 + cfg.stand_distance_m, z0)?);
            troop(&mut b, cfg.troop, &mut lineage)?;
            lineage.clone()
        }
        ModeScene::SeedporterRest => {
            troop(&mut b, cfg.troop, &mut lineage)?;
            lineage.clone()
        }
        ModeScene::Groom => {
            troop(&mut b, cfg.troop, &mut lineage)?;
            let host = lineage[0];
            let at = b.fauna.view().animal(host).map(|a| a.site).ok_or("the host")?;
            let (larva, _) = b.place(Founder::Lanternjaw, at, BodyStage::Rider, None, Group::Auto)?;
            if b.fauna.view().animal(larva).and_then(|a| a.attached).map(|t| t.host) != Some(host) {
                return Err("the larva did not take the troopmate as its host".into());
            }
            others.push(host);
            lineage[1..].to_vec()
        }
        ModeScene::FollowMother => {
            let s = b.near(x0, z0);
            let (mother, _) = b.place(founder, s, BodyStage::Adult, None, Group::Auto)?;
            let s = b.near(x0 + cfg.mother_distance_m, z0);
            let (young, _) = b.place(founder, s, BodyStage::Juvenile, Some(cfg.young_fullness), Group::Auto)?;
            let age = (cfg.young_age_s * f64::from(cubarium_voxel_fauna::TICK_HZ)) as u64;
            if !b.fauna.stage_identity(young, age, Some(mother)) {
                return Err("the young's identity was refused".into());
            }
            lineage = vec![mother, young];
            others.push(mother);
            vec![young]
        }
        ModeScene::FleeToTrees => {
            site = Some(b.crown(founder, x0 + cfg.stand_distance_m, z0)?);
            troop(&mut b, cfg.troop, &mut lineage)?;
            let mut pack = None;
            for k in 0..cfg.hounds {
                let s = b.near(x0 - cfg.threat_distance_m - k as f64 * cfg.spacing_m, z0);
                let group = pack.map_or(Group::New, Group::Join);
                let (id, p) = b.place(Founder::Chorister, s, BodyStage::Adult, Some(cfg.hound_fullness), group)?;
                pack = pack.or(p);
                others.push(id);
            }
            lineage.clone()
        }
        ModeScene::Feed | ModeScene::GiveWay => {
            let s = b.near(x0, z0);
            // Giving way: a young lanternjaw, inside the Veilhound's size window (a grown one
            // is past it, so nothing there hunts it and it never yields).
            let stage = if scene == ModeScene::GiveWay { BodyStage::Juvenile } else { BodyStage::Adult };
            let (lj, _) = b.place(founder, s, stage, Some(cfg.hunter_fullness), Group::Auto)?;
            // The carcass under its mouth: carrion on its face, with a carcass record.
            let mouth = b.fauna.view().animal(lj).map(|a| a.site).ok_or("the lanternjaw")?;
            let o = cfg.carcass_organic;
            if !b.flora.deposit(mouth, Deposit { kind: DepositKind::Carrion, organic: o, mineral: 0.05 * o, energy: 2.0 * o })
                || !b.flora.lay_carcass(mouth, Carcass { lineage: Founder::Blind.index() as u8, heading_rad: 0.0, organic: o })
            {
                return Err("carcass placement refused".into());
            }
            site = Some(mouth);
            lineage = vec![lj];
            if scene == ModeScene::GiveWay {
                let s = b.near(x0 + cfg.big_distance_m, z0);
                others.push(b.place(Founder::Chorister, s, BodyStage::Adult, Some(cfg.hound_fullness), Group::New)?.0);
            }
            vec![lj]
        }
    };
    crate::habitat::reinstall_heuristics(&mut b.fauna);
    b.fauna.set_census_readout(true);
    Ok(Episode {
        sim: Sim::new_static(b.world, b.flora, b.fauna, SimConfig { threads: 1, overlap: false, water_every: 1 }),
        scene,
        seed,
        focal,
        lineage,
        others,
        site,
        limit: (cfg.duration_s * f64::from(cubarium_voxel_fauna::TICK_HZ)).ceil() as u64,
        tick: 0,
    })
}
