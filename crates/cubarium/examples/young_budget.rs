//! **A young detritivore's energy budget, hatch to first clutch** (package D1b): one
//! hatchling of a lineage on a flat plain carpeted in litter that never runs out, the day
//! clock off (always awake), standing and feeding: the physiology's own ceiling, with
//! nothing about finding food or sleeping in it.
//!
//! ```text
//! young_budget [founder=littershredder] [hours=10] [every=10] [config=FILE] [set=P=V ...]
//! ```
//!
//! Config layers as `detritus_study` (default `config/scenarios/patch.toml`). The probe's
//! own changes, which do not touch the focal body's budget: its lineage's eggs never hatch
//! (so the plain holds one body), the nest-site brake is off, and it has no lifespan.
//!
//! Prints a `t,` line every `every` minutes (age, body, reserve and its fullness, the
//! brood, the stomach's fill, and the phase's cumulative assimilation, upkeep, growth and
//! eggs), a `clutch,` line per clutch, and `phase,` lines summing hatch → laying size
//! (maturation) and laying size → first clutch: minutes, assimilated, upkeep, motor,
//! growth, reserve change and eggs, each also per body-day.

use std::path::Path;

use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{
    Actions, Command, Controller, Fauna, FaunaLedger, Founder, Response, StartingStores, TICK_HZ,
};
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig, Site};
use cubarium_voxel_sim::params::{Layers, Params};

fn arg<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(k))
}

fn resolve(args: &[String]) -> Params {
    let path = arg(args, "config=").unwrap_or("config/scenarios/patch.toml");
    let cfg = cubarium::voxel::load_config(Path::new(path)).unwrap_or_else(|e| panic!("config {path}: {e}"));
    let mut file = toml::Table::new();
    if !cfg.flora.is_empty() {
        file.insert("flora".into(), toml::Value::Table(cfg.flora));
    }
    if !cfg.fauna.is_empty() {
        file.insert("fauna".into(), toml::Value::Table(cfg.fauna));
    }
    let sets: Vec<String> = args.iter().filter_map(|a| a.strip_prefix("set=").map(str::to_string)).collect();
    Params::resolve_over(Params { world: cfg.world, ..Params::defaults() }, &Layers { file: Some(file), sets, ..Layers::default() })
        .unwrap_or_else(|e| panic!("the parameter set: {e}"))
}

/// Stands still with `feed` held.
struct StandAndFeed;
impl Controller for StandAndFeed {
    fn drive(&mut self, _o: &[f64]) -> Response {
        Response::Bounded(Actions { forward: 0.0, turn: 0.0, feed: 1.0 })
    }
    fn reset(&mut self) {}
}

const VOXEL_M: f64 = 0.125;
const SURFACE_Y: u32 = 2;
const W: u32 = 32;
const D: u32 = 16;
const PILE: f64 = 1.0;

fn plain() -> World {
    let mut w = World::empty(VoxelConfig {
        width: W,
        height: 8,
        depth: D,
        voxel_m: VOXEL_M,
        seed: 7,
        day: DayClock { mode: DayMode::Off, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    for z in 0..D {
        for x in 0..i64::from(W) {
            for y in 1..=SURFACE_Y {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w
}

/// Every face's litter back up to the pile.
fn carpet(flora: &mut Flora) {
    for z in 0..D {
        for x in 0..W {
            let site = Site { x, y: SURFACE_Y, z };
            let have = flora.view().ground_at(site).map_or(0.0, |g| g.litter);
            let add = PILE - have;
            if add > 1e-6 {
                flora.deposit(site, Deposit { kind: DepositKind::Litter, organic: add, mineral: 0.05 * add, energy: 2.0 * add });
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Mark {
    tick: u64,
    body: f64,
    reserve: f64,
    assim: f64,
    upkeep: f64,
    motor: f64,
    eggs_paid: f64,
}

fn mark(fauna: &Fauna, id: u64, f: Founder, tick: u64) -> Mark {
    let a = fauna.view().animal(id).copied().expect("the focal body is alive");
    let l: &FaunaLedger = &fauna.view().ledger;
    let i = f.index();
    Mark {
        tick,
        body: a.body,
        reserve: a.reserve,
        assim: l.assimilated_by_founder[i],
        upkeep: l.upkeep_by_founder[i],
        motor: l.motor_by_founder[i],
        eggs_paid: l.reproduction_by_founder[i],
    }
}

fn phase(name: &str, a: Mark, b: Mark, day_s: f64) {
    let hz = f64::from(TICK_HZ);
    let secs = (b.tick - a.tick) as f64 / hz;
    // Body-days: the mean of the two ends' structure (a log mean for growth), times days.
    let mean_body = if (b.body - a.body).abs() > 1e-12 { (b.body - a.body) / (b.body / a.body).ln() } else { a.body };
    let bd = mean_body * secs / day_s;
    let per = |x: f64| x / bd.max(1e-300);
    let assim = b.assim - a.assim;
    let upkeep = b.upkeep - a.upkeep;
    let motor = b.motor - a.motor;
    let growth = b.body - a.body;
    let dres = b.reserve - a.reserve;
    let eggs = b.eggs_paid - a.eggs_paid;
    println!(
        "phase,{name},min={:.1},body={:.4}->{:.4},assim={assim:.5},upkeep={upkeep:.5},motor={motor:.5},growth={growth:.5},d_reserve={dres:+.5},eggs={eggs:.5},residual={:+.1e},per_body_day:assim={:.3},upkeep={:.3},growth={:.3},eggs={:.3},g_net={:.3}",
        secs / 60.0,
        a.body,
        b.body,
        assim - upkeep - motor - growth - dres - eggs,
        per(assim),
        per(upkeep + motor),
        per(growth),
        per(eggs),
        per(assim - upkeep - motor),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let params = resolve(&args);
    let name = arg(&args, "founder=").unwrap_or("littershredder");
    let f = *Founder::ALL.iter().find(|x| x.name() == name).unwrap_or_else(|| panic!("no founder {name}"));
    let hours: f64 = arg(&args, "hours=").map_or(10.0, |h| h.parse().unwrap());
    let every: u64 = arg(&args, "every=").map_or(10, |m| m.parse().unwrap());
    let mut config = params.fauna.clone();
    {
        let r = &mut config.founders[f.index()].core.reproduction;
        r.incubation_s = 1e12;
        r.nest_radius_m = 0.0;
        config.founders[f.index()].core.lifespan_median_s = 0.0;
    }
    let p = *config.founder(f);
    let sc = p.core;
    let day_s = config.life.ecological_day_s;
    let world = plain();
    let mut flora = Flora::new(FloraConfig::default());
    carpet(&mut flora);
    let mut fauna = Fauna::new(config);
    // A hatchling: body_min with its egg's remainder as reserve.
    let hatch_reserve = (sc.reproduction.egg_organic - sc.body_min).max(0.0);
    let stores = StartingStores {
        body: sc.body_min / sc.body_max,
        reserve: (hatch_reserve / sc.reserve_of(sc.body_min)).min(1.0),
    };
    assert!(fauna.apply(&world, Command::IntroduceFounder { x: 8, z: 8, founder: f, stores, heading_rad: 0.0 }), "introduce");
    let id = fauna.view().animals.last().expect("one").id;
    fauna.set_controller(id, Box::new(StandAndFeed));
    println!(
        "lineage,{name},body_min={:.4},birth_body={:.4},body_max={:.4},reserve_cap={},egg={:.4},floor={:.4},clutch_max={},clutch_hold_s={:.0},brood_share={},hold_s={:.0},interval_s={:.0},stomach_k={},mass_exponent={},tau_litter={},yield_litter={:.3},maintenance_day={:.4},growth_max_day={:.4}",
        sc.body_min,
        sc.birth_body,
        sc.body_max,
        sc.reserve_cap,
        sc.reproduction.egg_organic,
        sc.reproduction.surplus_floor,
        sc.reproduction.clutch_max,
        sc.reproduction.clutch_hold_max_s,
        sc.reproduction.brood_share,
        sc.reproduction.surplus_hold_s,
        sc.reproduction.birth_interval_s,
        p.gut.stomach_k,
        p.gut.mass_exponent,
        p.gut.digestion_s[cubarium_voxel_fauna::Food::Litter.index()],
        p.yield_for(cubarium_voxel_fauna::Food::Litter),
        sc.maintenance_per_s * day_s,
        sc.growth_max_per_s * day_s,
    );
    let hz = u64::from(TICK_HZ);
    let end = (hours * 3600.0) as u64 * hz;
    let start = mark(&fauna, id, f, 0);
    let mut matured: Option<Mark> = None;
    let mut eligible_at: Option<u64> = None;
    let mut clutches = 0u64;
    let mut first: Option<Mark> = None;
    let mut eggs_laid = 0u64;
    for t in 1..=end {
        fauna.step_with(&world, &mut flora, 1);
        let Some(a) = fauna.view().animal(id).copied() else {
            println!("died,{:.1}", t as f64 / hz as f64 / 60.0);
            break;
        };
        if matured.is_none() && a.body >= sc.birth_body {
            matured = Some(mark(&fauna, id, f, t));
        }
        if eligible_at.is_none() && a.reproduction.eligible_now {
            eligible_at = Some(t);
        }
        let l = &fauna.view().ledger;
        let c = l.clutches_by_founder[f.index()];
        if c > clutches {
            let n = l.eggs_laid_by_founder[f.index()] - eggs_laid;
            eggs_laid = l.eggs_laid_by_founder[f.index()];
            clutches = c;
            println!("clutch,{},min={:.1},eggs={n},body={:.4}", clutches, t as f64 / hz as f64 / 60.0, a.body);
            if first.is_none() {
                first = Some(mark(&fauna, id, f, t));
            }
        }
        if t % (60 * hz) == 0 {
            carpet(&mut flora);
        }
        if t % (every * 60 * hz) == 0 {
            let cap = sc.reserve_of(a.body);
            let k = p.gut.capacity(a.body);
            let m = mark(&fauna, id, f, t);
            println!(
                "t,{:.0},body={:.4},reserve={:.5},fullness={:.3},brood={:.5},stomach={:.2},assim={:.5},upkeep={:.5},growth={:.5},eggs={:.5},clutches={clutches},eggs_laid={eggs_laid}",
                t as f64 / hz as f64 / 60.0,
                a.body,
                a.reserve,
                a.reserve / cap,
                a.reproduction.brood,
                a.stomach.organic() / k,
                m.assim - start.assim,
                m.upkeep - start.upkeep,
                m.body - start.body,
                m.eggs_paid - start.eggs_paid,
            );
        }
    }
    let mins = |t: u64| t as f64 / hz as f64 / 60.0;
    println!(
        "milestones,maturation_min={},first_eligible_min={},first_clutch_min={},clutches={clutches},eggs={eggs_laid}",
        matured.map_or("none".into(), |m| format!("{:.1}", mins(m.tick))),
        eligible_at.map_or("none".into(), |t| format!("{:.1}", mins(t))),
        first.map_or("none".into(), |m| format!("{:.1}", mins(m.tick))),
    );
    if let Some(m) = matured {
        phase("hatch_to_laying_size", start, m, day_s);
        if let Some(fc) = first {
            phase("laying_size_to_first_clutch", m, fc, day_s);
            phase("hatch_to_first_clutch", start, fc, day_s);
        }
    }
}
