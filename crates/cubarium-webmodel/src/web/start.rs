//! **Start states for the closed loop**: the plant half from a save and its weather
//! snapshots (the producer half's own setup, `examples/flora_model.rs run`), the animal half
//! from a census of the same save (one line per body and per clutch), and the fits the loop
//! runs with.

use std::path::{Path, PathBuf};

use cubarium_voxel_fauna::{Food, Founder};
use cubarium_voxel_flora::Species;
use cubarium_voxel_sim::params::Params;

use crate::fauna::fits::Response;
use crate::fauna::model::EggCohort;
use crate::fauna::{Cohort, FaunaModel, Fits};
use crate::flora::climate::DayModel;
use crate::flora::{census, config, fit, kernel, model, zones};

/// The resolved parameter set of a scenario file and its `path=value` sets: the one both
/// halves read (`flora::config::resolve`, the world table expanded as the sim expands it).
pub fn params(cfg: &Path, sets: &[String]) -> Result<Params, String> {
    config::resolve(cfg, sets)
}

/// **The plant half on a save** (`start`) with its weather snapshots (`snaps`, the start's
/// own first): the producer half's own setup, line for line as `flora_model run` does it —
/// the zones, the dispersal kernels (3,000 samples, seed 7), the fitted light, the per-zone
/// seed clumping, the self-shade `β`, the weather replayed every 60 min from `wseed`.
pub fn flora(p: &Params, start: &Path, snaps: &[PathBuf], wseed: u64) -> Result<model::Model, String> {
    let fc = config::flora_of(p);
    let mut dirs: Vec<PathBuf> = vec![start.to_path_buf()];
    dirs.extend(snaps.iter().cloned());
    let mut faces = Vec::new();
    let mut dims = (0, 0);
    for d in &dirs {
        let (world, flora) = census::read_save(d)?;
        dims = (world.config().width, world.config().depth);
        faces.push(census::census(&world, &flora, Some(&fc)));
    }
    let minute0: f64 = std::fs::read_to_string(start.join("minute"))
        .map_err(|e| format!("{}/minute: {e}", start.display()))?
        .trim()
        .parse()
        .map_err(|e| format!("minute: {e}"))?;
    let zs = zones::build(&fc, &faces);
    let (kern, rb) = kernel::build(&fc, dims.0, dims.1, &zs, &faces, 3000, 7);
    let mut mp = model::Params::default();
    let lk = fit::light(&fc, &zs, &faces);
    for sp in Species::ALL {
        let k = sp.index();
        mp.light_k[k] = [[lk[k][0][0].value, lk[k][0][1].value], [lk[k][1][0].value, lk[k][1][1].value]];
    }
    let ck = fit::clumping(&zs);
    for sp in Species::ALL {
        mp.seed_k[sp.index()] = ck[sp.index()].value;
    }
    mp.seed_k_zone = fit::clumping_by_zone(&zs, &ck);
    let sb = fit::self_shade_beta(&zs);
    for sp in Species::ALL {
        mp.self_beta[sp.index()] = sb[sp.index()].value;
    }
    mp.weather = model::Weather::Replay { period_s: 3600.0, seed: wseed };
    Ok(model::Model::new(fc.clone(), DayModel::of(&p.world, fc.upkeep_q10), zs, kern, rb, &faces[0], minute0, mp))
}

/// One body or clutch of a fauna census (`census370` at the soak build: `animal,lineage,
/// body,reserve,age_ticks,host,lifespan_ticks` and `clutch,lineage,count,organic,laid_tick,
/// dormant`, after a `tick,N` line).
#[derive(Clone, Debug, Default)]
pub struct FaunaCensus {
    pub tick: u64,
    /// (lineage, body, reserve, age in ticks, host lineage).
    pub bodies: Vec<(Founder, f64, f64, u64, Option<Founder>)>,
    /// (lineage, eggs, organic in the whole clutch, laid tick, dormant).
    pub clutches: Vec<(Founder, u32, f64, u64, bool)>,
}

fn founder(name: &str) -> Option<Founder> {
    Founder::ALL.iter().copied().find(|f| f.name() == name)
}

impl FaunaCensus {
    pub fn read(path: &Path) -> Result<FaunaCensus, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut c = FaunaCensus::default();
        for line in text.lines() {
            let p: Vec<&str> = line.split(',').collect();
            match p.first().copied() {
                Some("tick") => c.tick = p[1].parse().map_err(|e| format!("tick: {e}"))?,
                Some("animal") if p.len() >= 6 => {
                    let Some(f) = founder(p[1]) else { continue };
                    c.bodies.push((f, p[2].parse().unwrap_or(0.0), p[3].parse().unwrap_or(0.0), p[4].parse().unwrap_or(0), founder(p[5])));
                }
                Some("clutch") if p.len() >= 6 => {
                    let Some(f) = founder(p[1]) else { continue };
                    c.clutches.push((f, p[2].parse().unwrap_or(0), p[3].parse().unwrap_or(0.0), p[4].parse().unwrap_or(0), p[5] == "true"));
                }
                _ => {}
            }
        }
        Ok(c)
    }

    /// Bodies of `f` (free and riding).
    pub fn count(&self, f: Founder) -> usize {
        self.bodies.iter().filter(|b| b.0 == f).count()
    }
}

/// **The animal half on a census**: one cohort per body (the merge pass folds equal bodies
/// together), eggs and the bank as egg cohorts, the clock at the census tick. A body as old
/// as the world is a founder (its lifespan the arriving draw); the rest are native. Only the
/// `enabled` lineages are stepped; the others are held as they are and take nothing.
pub fn fauna(p: &Params, fits: Fits, census: &FaunaCensus, enabled: &[Founder], dt: f64) -> FaunaModel {
    let mut m = FaunaModel::new(p, fits, dt);
    let hz = f64::from(cubarium_voxel_fauna::TICK_HZ);
    for f in Founder::ALL {
        m.enabled[f.index()] = enabled.contains(&f);
    }
    for &(f, body, reserve, age, host) in &census.bodies {
        let sc = p.fauna.founder(f).core;
        let mut c = Cohort::new(1.0, body, reserve);
        c.age_s = age as f64 / hz;
        c.arriving = age >= census.tick;
        c.native = !c.arriving;
        c.matured = body >= sc.birth_body;
        c.host = host;
        m.add(f, c);
    }
    for &(f, n, organic, laid, dormant) in &census.clutches {
        if n == 0 {
            continue;
        }
        m.eggs.push(EggCohort { lineage: f, n: f64::from(n), organic: organic / f64::from(n), age_s: census.tick.saturating_sub(laid) as f64 / hz, dormant });
    }
    m.t_s = census.tick as f64 / hz;
    m.merge_cohorts();
    m.mineral = m.mineral_wanted();
    m
}

/// The lineages the animal half has fitted rates for, which the closed loop steps: the three
/// detritivores (D1b), the frondgrazer (`fitbrowser`) and the lanternjaw (D2b). The bellwing,
/// the loftstrider, the seedporter and the Veilhound have none (`fauna.md`, "What this half
/// cannot represent") and are held.
pub const STEPPED: [Founder; 5] = [Founder::Blind, Founder::Capgnawer, Founder::RippleSnail, Founder::Browser, Founder::Lanternjaw];

/// **The fits the loop runs with**: [`Fits::recorded`] and the hunter's, which the animal half
/// refits deterministically at each D2b run (`fauna_model d2b`, `fit_hunter` on the `d2` arm,
/// minutes 120-240, 8 seeds; the values its run of 2026-10-05 printed, `<S>/wmf2/d2b-v6.txt`):
/// the dormant eggs' loss 0.333 (24 of 72), carrion only at its own kills, a kill's reach
/// 0.358 (assimilation 5.474 matched), kill rates per prey body per hunting LJ-second
/// shredder 8.715e-5, capgnawer 7.633e-5, snail 3.946e-5, frondgrazer 4.351e-5 (kills 125 vs
/// 128), 2.38 strikes per kill (305 / 128).
pub fn closed_fits() -> Fits {
    let mut f = Fits::recorded();
    let lj = Founder::Lanternjaw;
    f.of_mut(lj).egg_loss = 0.333;
    f.of_mut(lj).response[Food::Carrion.index()] = Some(Response::holling(0.0, 0.0));
    f.kill_reach = 0.358;
    f.of_mut(Founder::Blind).kill_rate = 8.715e-5;
    f.of_mut(Founder::Capgnawer).kill_rate = 7.633e-5;
    f.of_mut(Founder::RippleSnail).kill_rate = 3.946e-5;
    f.of_mut(Founder::Browser).kill_rate = 4.351e-5;
    f.strikes_per_kill = 2.38;
    f.fitted[lj.index()] = true;
    f
}
