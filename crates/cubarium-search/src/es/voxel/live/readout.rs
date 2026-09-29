//! **The ecology readout**, every generation, pooled over worlds: the draft list of the
//! large-scale factors Wrysk will choose targets from.
//!
//! A factor is a name, a pooling rule and a function of one world at the end of its
//! window ([`Ctx`]): [`WORLD_FACTORS`] for the world, [`LINEAGE_FACTORS`] once per
//! lineage (named `<lineage>.<factor>`). **Adding a factor is one line** in one of those
//! tables; a quantity no factor can read yet is one field on [`LineageStats`] (filled by
//! [`super::window`]) or one reading in [`Scan`].

use std::collections::BTreeMap;

use cubarium_voxel::{Material, World};
use cubarium_voxel_fauna::{Departure, Founder};
use cubarium_voxel_flora::Species;
use cubarium_voxel_sim::Sim;

/// How a factor's per-world values become one number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pool {
    /// Summed over worlds (populations, stocks).
    Sum,
    /// Averaged over the worlds that have a value (levels, shares, rates).
    Mean,
}

/// What one lineage did in one world over one window, as the window counted it.
#[derive(Clone, Debug, Default)]
pub struct LineageStats {
    pub births: u64,
    pub deaths: [u64; Departure::COUNT],
    /// Body-ticks lived inside the window.
    pub body_ticks: u64,
    /// Body-minutes (whole simulated minutes of one body) and how many of them the body
    /// averaged over 0.1 body lengths per second.
    pub body_minutes: u64,
    pub moving_minutes: u64,
    /// Distance walked, in body lengths (each tick's delivered forward over the body's
    /// own length at that tick).
    pub walked_bl: f64,
    /// Distinct columns each body stood on in the window, summed over bodies, m².
    pub unique_m2: f64,
    /// Scored bodies this window and their mean score (the ES reads the per-body list).
    pub scored: u64,
    pub mean_score: f64,
}

/// One world at the end of a window, and what the window counted.
pub struct Ctx<'a> {
    pub sim: &'a Sim,
    pub lineages: &'a [LineageStats; Founder::COUNT],
    pub scan: &'a Scan,
    /// The window's length, simulated hours.
    pub hours: f64,
    /// Wall milliseconds per tick this window.
    pub tick_ms: f64,
}

/// The readings more than one factor shares, taken once per world per window.
#[derive(Clone, Debug, Default)]
pub struct Scan {
    pub lake_level_m: f64,
    pub lake_volume_m3: f64,
    pub lake_area_m2: f64,
    /// Mean pore fill of the soil cells, `0..=1`.
    pub soil_moisture: f64,
    pub litter: f64,
    pub carrion: f64,
    pub foliage: f64,
    pub stands: [usize; Species::COUNT],
}

impl Scan {
    pub fn of(sim: &Sim) -> Scan {
        let world = sim.world();
        let lake = cubarium_voxel::hydrate::lake(world);
        let f = sim.flora().view();
        let mut stands = [0usize; Species::COUNT];
        for s in f.stands {
            stands[s.species.index()] += 1;
        }
        Scan {
            lake_level_m: f64::from(lake.level_y) * world.config().voxel_m,
            lake_volume_m3: lake.volume_m3,
            lake_area_m2: lake.visible_m2,
            soil_moisture: soil_moisture(world),
            litter: f.ground.iter().map(|g| g.litter).sum(),
            carrion: f.ground.iter().map(|g| g.carrion).sum(),
            foliage: f.stands.iter().map(|s| s.foliage).sum(),
            stands,
        }
    }
}

/// Mean pore fill over every soil cell.
pub fn soil_moisture(world: &World) -> f64 {
    let v = world.view();
    let c = world.config();
    let (mut sum, mut n) = (0.0, 0u64);
    for x in 0..i64::from(c.width) {
        for z in 0..c.depth {
            for y in 0..c.height {
                if v.material_at(x, y, z) == Material::Soil {
                    sum += v.pore_at(x, y, z);
                    n += 1;
                }
            }
        }
    }
    if n == 0 { 0.0 } else { sum / n as f64 }
}

/// The woody species, as the census's succession line names them (measurement only).
const WOODY: [Species; 3] = [
    Species::Bloomcrown,
    Species::Vaulttree,
    Species::Lanternberry,
];

fn share(a: f64, b: f64) -> f64 {
    if b > 0.0 { a / b } else { 0.0 }
}

fn stands_total(c: &Ctx) -> f64 {
    c.scan.stands.iter().sum::<usize>() as f64
}

fn lineage_bodies(c: &Ctx, f: Founder) -> Vec<u64> {
    c.sim
        .fauna()
        .view()
        .animals
        .iter()
        .filter(|a| a.founder == Some(f))
        .map(|a| a.age_ticks)
        .collect()
}

fn median(mut v: Vec<u64>) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_unstable();
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2] as f64
    } else {
        (v[n / 2 - 1] + v[n / 2]) as f64 / 2.0
    }
}

const TICKS_PER_MIN: f64 = 60.0 * cubarium_voxel_fauna::TICK_HZ as f64;

type WorldFactor = (&'static str, Pool, fn(&Ctx) -> f64);
type LineageFactor = (&'static str, Pool, fn(&Ctx, Founder) -> f64);

/// The world's factors. One line each.
pub const WORLD_FACTORS: &[WorldFactor] = &[
    ("plants.stands", Pool::Sum, |c| stands_total(c)),
    ("plants.cover_share", Pool::Mean, |c| {
        let cfg = c.sim.flora().view().config;
        let n: usize = Species::ALL
            .iter()
            .filter(|s| cfg.species(**s).ground_cover)
            .map(|s| c.scan.stands[s.index()])
            .sum();
        share(n as f64, stands_total(c))
    }),
    ("plants.woody_share", Pool::Mean, |c| {
        let n: usize = WOODY.iter().map(|s| c.scan.stands[s.index()]).sum();
        share(n as f64, stands_total(c))
    }),
    ("plants.foliage", Pool::Sum, |c| c.scan.foliage),
    ("stock.litter", Pool::Sum, |c| c.scan.litter),
    ("stock.carrion", Pool::Sum, |c| c.scan.carrion),
    ("water.lake_level_m", Pool::Mean, |c| c.scan.lake_level_m),
    ("water.lake_volume_m3", Pool::Mean, |c| {
        c.scan.lake_volume_m3
    }),
    ("water.soil_moisture", Pool::Mean, |c| c.scan.soil_moisture),
    ("cost.tick_ms", Pool::Mean, |c| c.tick_ms),
];

/// One lineage's factors, one line each; named `<lineage>.<factor>`.
pub const LINEAGE_FACTORS: &[LineageFactor] = &[
    ("population", Pool::Sum, |c, f| {
        lineage_bodies(c, f).len() as f64
    }),
    ("births_per_h", Pool::Mean, |c, f| {
        c.lineages[f.index()].births as f64 / c.hours
    }),
    ("starved_per_h", Pool::Mean, |c, f| {
        c.lineages[f.index()].deaths[Departure::Starved.index()] as f64 / c.hours
    }),
    ("drowned_per_h", Pool::Mean, |c, f| {
        c.lineages[f.index()].deaths[Departure::Drowned.index()] as f64 / c.hours
    }),
    ("aged_per_h", Pool::Mean, |c, f| {
        c.lineages[f.index()].deaths[Departure::Aged.index()] as f64 / c.hours
    }),
    ("other_deaths_per_h", Pool::Mean, |c, f| {
        c.lineages[f.index()].deaths[Departure::Removed.index()] as f64 / c.hours
    }),
    ("median_age_min", Pool::Mean, |c, f| {
        median(lineage_bodies(c, f)) / TICKS_PER_MIN
    }),
    ("moving_share", Pool::Mean, |c, f| {
        let l = &c.lineages[f.index()];
        share(l.moving_minutes as f64, l.body_minutes as f64)
    }),
    ("pace_bl_per_s", Pool::Mean, |c, f| {
        let l = &c.lineages[f.index()];
        share(l.walked_bl, l.body_ticks as f64 * cubarium_voxel::DT)
    }),
    ("unique_m2_per_body_h", Pool::Mean, |c, f| {
        let l = &c.lineages[f.index()];
        share(l.unique_m2, l.body_ticks as f64 / (TICKS_PER_MIN * 60.0))
    }),
    ("mean_score", Pool::Mean, |c, f| {
        c.lineages[f.index()].mean_score
    }),
];

/// One world's factor values, in table order.
pub fn read(ctx: &Ctx) -> Vec<(String, Pool, f64)> {
    let mut out = Vec::new();
    for (name, pool, f) in WORLD_FACTORS {
        out.push(((*name).to_string(), *pool, f(ctx)));
    }
    for founder in Founder::ALL {
        for (name, pool, f) in LINEAGE_FACTORS {
            out.push((format!("{}.{name}", founder.name()), *pool, f(ctx, founder)));
        }
    }
    for s in Species::ALL {
        out.push((
            format!("plants.stands.{}", s.name()),
            Pool::Sum,
            ctx.scan.stands[s.index()] as f64,
        ));
    }
    out
}

/// Pool every world's values into one row, in first-seen order (a `BTreeMap` keyed by
/// position keeps it). Non-finite per-world values (a median of no bodies) are skipped.
pub fn pool(worlds: &[Vec<(String, Pool, f64)>]) -> Vec<(String, f64)> {
    let mut acc: BTreeMap<usize, (String, Pool, f64, u32)> = BTreeMap::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    for w in worlds {
        for (name, pool, v) in w {
            let k = *index.entry(name.clone()).or_insert_with(|| index_len(&acc));
            let e = acc
                .entry(k)
                .or_insert_with(|| (name.clone(), *pool, 0.0, 0));
            if v.is_finite() {
                e.2 += v;
                e.3 += 1;
            }
        }
    }
    acc.into_values()
        .map(|(name, pool, sum, n)| {
            let v = match pool {
                Pool::Sum => sum,
                Pool::Mean if n > 0 => sum / f64::from(n),
                Pool::Mean => f64::NAN,
            };
            (name, v)
        })
        .collect()
}

fn index_len<T>(m: &BTreeMap<usize, T>) -> usize {
    m.len()
}
