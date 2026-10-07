//! **Start states for the closed loop**: the plant half from a save and its weather
//! snapshots (the producer half's own setup, `examples/flora_model.rs run`), the animal half
//! from a census of the same save (one line per body and per clutch), and the fits the loop
//! runs with.

use std::path::{Path, PathBuf};

use cubarium_voxel_fauna::{Food, Founder};
use cubarium_voxel_flora::Species;
use cubarium_voxel_sim::params::Params;

use crate::fauna::fits::{Encounter, HunterFit, Response};
use crate::fauna::model::EggCohort;
use crate::fauna::{Cohort, FaunaModel, Fits};
use crate::flora::climate::DayModel;
use crate::flora::{census, config, fit, kernel, model, zones};

/// The resolved parameter set of a scenario file and its `path=value` sets: the one both
/// halves read (`flora::config::resolve`, the world table expanded as the sim expands it).
pub fn params(cfg: &Path, sets: &[String]) -> Result<Params, String> {
    config::resolve(cfg, sets)
}

/// **A scenario file with an overlay laid over it**: `overlay`'s own `[world]`, `[flora]` and
/// `[fauna]` tables (its `base` ignored) merged key by key over `cfg`'s chain, then resolved as
/// [`params`] resolves a file. The soak validation runs the soak's config under the soak
/// build's behaviour: the knobs added after 18ca6e81 (the patrol above all) at the values that
/// reproduce that build, which `config/scenarios/hunters/main.toml` records ("main, the
/// 2026-10-05 lanternjaw (the P1 crown-seeking ambush ...)").
pub fn params_with_overlay(cfg: &Path, overlay: &Path, sets: &[String]) -> Result<Params, String> {
    let mut table = config::config_table(cfg)?;
    let text = std::fs::read_to_string(overlay).map_err(|e| format!("reading {}: {e}", overlay.display()))?;
    let mut over: toml::Table = toml::from_str(&text).map_err(|e| format!("parsing {}: {e}", overlay.display()))?;
    over.remove("base");
    fn merge(under: &mut toml::Table, over: toml::Table) {
        for (k, v) in over {
            match (under.get_mut(&k), v) {
                (Some(toml::Value::Table(u)), toml::Value::Table(o)) => merge(u, o),
                (_, v) => {
                    under.insert(k, v);
                }
            }
        }
    }
    merge(&mut table, over);
    let world = match table.remove("world") {
        Some(toml::Value::Table(t)) => t,
        _ => toml::Table::new(),
    };
    let world = cubarium_voxel::world_config_from_table(world)?;
    let mut file = toml::Table::new();
    for key in ["flora", "fauna"] {
        if let Some(toml::Value::Table(t)) = table.remove(key) {
            file.insert(key.into(), toml::Value::Table(t));
        }
    }
    Params::resolve_over(
        Params { world, ..Params::defaults() },
        &cubarium_voxel_sim::params::Layers { file: Some(file), sets: sets.to_vec(), ..Default::default() },
    )
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
    let mut minute0 = 0.0;
    for (i, d) in dirs.iter().enumerate() {
        // The census file beside the save when there is one (`census::load`).
        let cs = census::load(d, &fc)?;
        dims = (cs.width, cs.depth);
        if i == 0 {
            minute0 = cs.minute;
        }
        faces.push(cs.faces);
    }
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
        m.eggs.push(EggCohort { lineage: f, n: f64::from(n), organic: organic / f64::from(n), age_s: census.tick.saturating_sub(laid) as f64 / hz, dormant, ..Default::default() });
    }
    m.t_s = census.tick as f64 / hz;
    m.merge_cohorts();
    m.mineral = m.mineral_wanted();
    m
}

/// The lineages the closed loop steps: every lineage, now that the held ones (the Veilhound,
/// the bellwing, the loftstrider, the seedporter) have fitted rates ([`closed_fits`]).
pub const STEPPED: [Founder; Founder::COUNT] = Founder::ALL;

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
    // The patrol's row: the D2b rates, met in each prey's own habitat (`Encounter::PreyHabitat`).
    let mut patrol = HunterFit { kill: [0.0; Founder::COUNT], reach: f.kill_reach, meal_cap: f64::INFINITY, strikes_per_kill: f.strikes_per_kill, encounter: Encounter::PreyHabitat };
    for g in Founder::ALL {
        patrol.kill[g.index()] = f.of(g).kill_rate;
    }
    f.hunters[lj.index()] = Some(patrol);
    // The crown ambush's row (`fauna_model fitambush`, the D2b `main` arm, m120-240, 8 seeds:
    // 10 kills matched): one rate for every prey on its list, met over the floor; the patrol's
    // reach and strikes.
    f.ambush[lj.index()] = Some(HunterFit { kill: [AMBUSH_RATE; Founder::COUNT], reach: f.kill_reach, meal_cap: f64::INFINITY, strikes_per_kill: f.strikes_per_kill, encounter: Encounter::Floor });
    held_recorded(&mut f);
    f
}

/// **The crown ambush's encounter rate**, kills per hunting lanternjaw-second per prey body at
/// strike share 1, patch floor units (`fauna_model fitambush`, 2026-10-05): the D2b `main` arm
/// (`hunters/main.toml`, the patrol off), m120-240, 8 seeds, 10 kills matched. The patrol's are
/// 4-9e-5: the ambush meets prey about ten times less often.
pub const AMBUSH_RATE: f64 = 5.2751e-6;

/// **Starting guesses** for the held lineages' fit (`web_model fitheld`): every Veilhound kill
/// rate 1e-5 per hunting body-second per prey body (patch floor units), the lanternjaw's reach
/// and strikes, carrion only at its kills; each herbivore's response flat at 0.5.
pub fn held_guess(f: &mut Fits) {
    let vh = Founder::Chorister;
    let mut h = HunterFit { kill: [0.0; Founder::COUNT], reach: 1.0, meal_cap: 0.34, strikes_per_kill: f.strikes_per_kill, encounter: Encounter::Floor };
    for g in [Founder::Browser, Founder::Loftstrider, Founder::Lanternjaw, Founder::Seedporter] {
        h.kill[g.index()] = 1e-5;
    }
    f.hunters[vh.index()] = Some(h);
    f.of_mut(vh).response[Food::Carrion.index()] = Some(Response::holling(0.0, 0.0));
    f.of_mut(Founder::Bellwing).response[Food::Nectar.index()] = Some(Response::holling(0.5, 0.0));
    f.of_mut(Founder::Loftstrider).response[Food::Foliage.index()] = Some(Response::holling(0.5, 0.0));
    f.of_mut(Founder::Seedporter).response[Food::Fruit.index()] = Some(Response::holling(0.5, 0.0));
}

/// **The held lineages' recorded fits** (`web_model fitheld`, 2026-10-05; the animal half alone
/// on soak v2's books, the plant stocks clamped to the log; training seeds 5 and 6, seed 1 held
/// out; `design/webmodel/fauna.md`, "The held lineages"):
/// - **the Veilhound**, its prey held to the logged counts while fitted, matched on kills by
///   prey over m370-1570: per hunting Veilhound-second per prey body (patch floor units, strike
///   share 1) frondgrazer 1.92e-5 (seed 5 alone, 111 kills: seed 6's packs killed none of the
///   97 at hand), loftstrider 9.71e-5 (190 kills, seeds 5 and 6), lanternjaw 3.07e-6 (3),
///   seedporter 1.59e-5 (7); the last rounds move them by 10-20 %. Its meal is capped at 0.208
///   organic a kill, its assimilation matched (52.3): it ate about the same per kill off a
///   3-unit loftstrider as off a 1-unit grazer (0.33 and 0.35 swallowed per kill);
/// - **the bellwing**: nectar, sigmoid (Hill 3, half 2.0, interference 0.01, `psi_max` 2.63),
///   on a grid of shape × half × interference (error 1.17; Hill 2-4 with half 1-3 score within
///   0.3 of it: the threshold near a nectar density of 2 is what is identified);
/// - **the loftstrider**: foliage, flat `psi_max` 0.415 (seed 6's assimilation per body-day);
/// - **the seedporter**: fruit, flat `psi_max` 0.027 (seeds 5 and 6).
pub fn held_recorded(f: &mut Fits) {
    held_guess(f);
    let vh = Founder::Chorister;
    if let Some(h) = f.hunters[vh.index()].as_mut() {
        h.kill[Founder::Browser.index()] = 1.917e-5;
        h.kill[Founder::Loftstrider.index()] = 9.715e-5;
        h.kill[Founder::Lanternjaw.index()] = 3.066e-6;
        h.kill[Founder::Seedporter.index()] = 1.586e-5;
        h.meal_cap = 0.208;
    }
    f.of_mut(Founder::Bellwing).response[Food::Nectar.index()] = Some(Response { psi_max: 2.6275, half: 2.0, interference: 0.01, hill: 3.0 });
    f.of_mut(Founder::Loftstrider).response[Food::Foliage.index()] = Some(Response::holling(0.415, 0.0));
    f.of_mut(Founder::Seedporter).response[Food::Fruit.index()] = Some(Response::holling(0.0272, 0.0));
    for g in [vh, Founder::Bellwing, Founder::Loftstrider, Founder::Seedporter] {
        f.fitted[g.index()] = true;
    }
    senses_refit(f);
}

/// **The Veilhound's frondgrazer rate under the new senses** (LIFE-HISTORY's refit,
/// 2026-10-06), kills per hunting Veilhound-second per grazer body (patch floor units, strike
/// share 1). The soak v2 fit above (1.917e-5, the old senses that could not find prey) killed
/// 111 grazers over 127.3 Veilhound-days at a mean 47.5 grazers (seed 5, m370-1570): 0.00918
/// kills per Veilhound-hour per grazer. S2's check on the desktop terrarium (the same floor),
/// with S1's senses and S2's vigilance (both on main), killed 47 over 144 Veilhound-hours at
/// the grazers' logged counts (4 seeds x 6 h, 10-minute readouts): 0.0257, 2.80x. Scaled by
/// that ratio, taking the hunting share of a Veilhound-hour as alike in both (both packs were
/// hungry most of the time). S1 alone (S2's `main` arm, no prey vigilance) was 0.0368, 4.0x.
/// The loftstrider, lanternjaw and seedporter rates keep the soak fit (S2's 13 and 15 kills
/// of loftstriders and seedporters are too few to refit).
pub const VH_GRAZER_RATE_SENSES: f64 = 1.917e-5 * 2.80;

/// Apply [`VH_GRAZER_RATE_SENSES`] to the Veilhound's row.
pub fn senses_refit(f: &mut Fits) {
    if let Some(h) = f.hunters[Founder::Chorister.index()].as_mut() {
        h.kill[Founder::Browser.index()] = VH_GRAZER_RATE_SENSES;
    }
}
