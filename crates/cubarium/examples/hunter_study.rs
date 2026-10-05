//! **The lanternjaw study** (package D2, `design/lanternjaw-detritus-web-proposal-2026-10-05.md`
//! §3.2–3.4 with `design/7_Research/astra-detritus-web-review-2026-10-05.md`): does the
//! patrolling lanternjaw live on the booming small prey, and does it spare the rare ones?
//!
//! ```text
//! hunter_study found seed=N save=DIR [minutes=120] [lanternjaws=3|1,3] [config=FILE] [set=P=V ...] [threads=T]
//! hunter_study run   load=DIR/mM arm=NAME hours=H [every=10] [config=FILE] [set=...] [run=DIR ...]
//! ```
//!
//! - `found`: the config's world (default `config/scenarios/patch-hunters.toml`: patch with
//!   the lanternjaw's egg bank) from founding to `minutes`, then `lanternjaws` adults
//!   introduced on prey ground (`--introduce lanternjaw:N`'s control,
//!   `cubarium_voxel_sim::habitat::introduce_lanternjaws`), saved (`DIR/mM`): the shared
//!   start every arm branches from. A list (`lanternjaws=1,3`) saves `DIR/mM_ljN` for each
//!   count, nested: the 3-hunter save holds the 1-hunter save's hunter and two more.
//! - `run`: branch a save under the resolved config — both layers' configs replaced by the
//!   arm's and the heuristics reinstalled — and run `hours`, staged with `run=DIR`.
//!
//! **Readouts** every `every` minutes over the window just ended:
//! - `lj,`: lanternjaws alive, free adults (`birth_body` and up), free juveniles, riders,
//!   their minimum so far; intake (assimilated), the awake share (awake over alive ticks),
//!   the patrol's still share (wait samples over its samples) and engaged samples;
//!   strikes, hits, kills; the egg-bank chain: eggs laid, eggs in the bank now, hatched,
//!   riders attached, dropped off, freed, clutches, first clutches (replacement).
//! - `prey,` per prey lineage on its list: bodies now, mean over the window, the minimum
//!   living and recruitable (adults) so far, kills by lanternjaws, strikes begun on it,
//!   contact seconds (locally available, size-eligible prey met), the contact seconds its
//!   attack choice declined and strikes on a prey the choice did not accept (D2b), and the **per-prey
//!   mortality**: kills per mean body per hour.
//! - `stocks,`: litter, film, glowcap stands and cap tissue, bodies, ms/tick.
//! - At the end, `sum,` lines over the whole run.

use std::path::Path;

use cubarium_search::es::voxel::live::balance;
use cubarium_voxel_fauna::{Departure, Fauna, FaunaLedger, Founder, TICK_HZ};
use cubarium_voxel_flora::Species;
use cubarium_voxel_sim::params::{Layers, Params};
use cubarium_voxel_sim::staged;
use cubarium_voxel_sim::{Sim, SimConfig};

const LJ: Founder = Founder::Lanternjaw;
const PREY: [Founder; 5] = [Founder::Blind, Founder::Capgnawer, Founder::RippleSnail, Founder::Bellwing, Founder::Browser];

fn arg<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(k))
}

fn resolve(args: &[String]) -> Params {
    let path = arg(args, "config=").unwrap_or("config/scenarios/patch-hunters.toml");
    let cfg = cubarium::voxel::load_config(Path::new(path)).unwrap_or_else(|e| panic!("config {path}: {e}"));
    let mut file = toml::Table::new();
    if !cfg.flora.is_empty() {
        file.insert("flora".into(), toml::Value::Table(cfg.flora));
    }
    if !cfg.fauna.is_empty() {
        file.insert("fauna".into(), toml::Value::Table(cfg.fauna));
    }
    let sets: Vec<String> = args.iter().filter_map(|a| a.strip_prefix("set=").map(str::to_string)).collect();
    for s in &sets {
        eprintln!("set {s}");
    }
    Params::resolve_over(Params { world: cfg.world, ..Params::defaults() }, &Layers { file: Some(file), sets, ..Layers::default() })
        .unwrap_or_else(|e| panic!("the parameter set: {e}"))
}

const FOUNDING_ONLY: &[&str] = &["seed=", "minutes=", "save=", "lanternjaws="];

fn main() {
    let raw: Vec<String> = std::env::args().collect();
    let args = staged::resume_args(&raw, FOUNDING_ONLY).unwrap_or_else(|e| panic!("{e}"));
    cubarium_voxel_sim::experiment_args(&args);
    if let Some(n) = arg(&args, "threads=").and_then(|s| s.parse().ok()) {
        cubarium_voxel::set_thread_override(n);
    }
    match args.get(1).map(String::as_str) {
        Some("found") => found(&args),
        Some("run") => run(&args),
        Some("probe") => probe(&args),
        _ => panic!("usage: hunter_study found|run [...]"),
    }
}

fn stage(args: &[String], sim: &mut Sim, minute0: u64, end: u64) -> Option<staged::Runner> {
    let cfg = staged::StageConfig::from_args(args).unwrap_or_else(|e| panic!("{e}"))?;
    let r = staged::Runner::start(cfg, args, sim, minute0, end).unwrap_or_else(|e| {
        eprintln!("staged: the load check failed — {e}");
        std::process::exit(2)
    });
    eprintln!("staged run {} to minute {}", r.dir().display(), r.end_minute());
    Some(r)
}

fn stop_at(runner: &mut Option<staged::Runner>, sim: &Sim, minute: u64) -> bool {
    let Some(r) = runner.as_mut() else { return false };
    match r.minute(sim, minute) {
        None => false,
        Some(ended) => {
            eprintln!("staged: {ended:?} at minute {minute}");
            ended != staged::Ended::Done
        }
    }
}

fn found(args: &[String]) {
    let params = resolve(args);
    let seed: u64 = arg(args, "seed=").map_or(1, |s| s.parse().unwrap());
    let minutes: u64 = arg(args, "minutes=").map_or(120, |m| m.parse().unwrap());
    // `lanternjaws=1,3`: one save per count, nested (the later count adds to the earlier's).
    let mut lanternjaws: Vec<usize> = arg(args, "lanternjaws=").map_or(vec![3], |m| m.split(',').map(|n| n.parse().unwrap()).collect());
    lanternjaws.sort_unstable();
    let dir = Path::new(arg(args, "save=").expect("save=DIR"));
    let wc = params.world.clone();
    let founded = cubarium::voxel::found(
        &wc,
        &cubarium::voxel::LayerConfig::new(&params, wc.voxel_m),
        params.fauna.founder_counts(),
        cubarium::voxel::Seeds::from(seed),
        false,
    );
    eprintln!("founded seed {} ({}): stands {}", founded.seed, if founded.accepted { "accepted" } else { "NOT accepted" }, founded.seeded.stands);
    println!("founded,{seed},{}", counts(&founded.fauna));
    let mut sim = Sim::ambient(founded.world, founded.flora, founded.fauna, SimConfig::experiment());
    let per_min = 60 * u64::from(TICK_HZ);
    let mut runner = stage(args, &mut sim, 0, minutes);
    for minute in 1..=minutes {
        for _ in 0..per_min {
            sim.step();
            if let Some(r) = runner.as_mut() {
                r.tick(&sim);
            }
        }
        if stop_at(&mut runner, &sim, minute) {
            return;
        }
    }
    let single = lanternjaws.len() == 1;
    let mut have = 0usize;
    for n in lanternjaws {
        let more = n.saturating_sub(have);
        if more > 0 {
            match sim.with_layers_mut(|w, fl, fa| cubarium_voxel_sim::habitat::introduce_lanternjaws(w, fl, fa, more)) {
                Ok(ids) => {
                    have += ids.len();
                    eprintln!("introduced {} lanternjaws at minute {minutes} ({have} in all)", ids.len());
                }
                Err(e) => eprintln!("no lanternjaws introduced: {e}"),
            }
        }
        let at = if single { dir.join(format!("m{minutes}")) } else { dir.join(format!("m{minutes}_lj{n}")) };
        let (w, fl, fa) = sim.layers();
        cubarium_voxel_sim::saved::save(&at, w, fl, fa, minutes).unwrap_or_else(|e| panic!("save: {e}"));
        println!("saved,{seed},{},{}", at.display(), counts(sim.fauna()));
    }
}

/// `probe load=DIR`: every lanternjaw's state in a save, one line each.
fn probe(args: &[String]) {
    let params = resolve(args);
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    let p = params.fauna.founder(LJ);
    let sky = cubarium_voxel_sim::Sim::ambient(s.world, s.flora, s.fauna, SimConfig::experiment());
    let v = sky.fauna().view();
    println!("minute {} tick {}", s.minute, v.tick);
    for a in v.animals.iter().filter(|a| a.founder == Some(LJ)) {
        let k = p.gut.capacity(a.body);
        println!(
            "lj {} body={:.3} fullness={:.3} stomach/K={:.3} eating={} emergency={} state={:?} rest={:?} attached={} hunt={:?}/{} strike={}",
            a.id,
            a.body,
            a.reserve / p.core.reserve_of(a.body).max(1e-300),
            a.stomach.organic() / k.max(1e-300),
            a.eating,
            a.emergency,
            a.state,
            a.rest,
            a.attached.is_some(),
            a.hunt.mode,
            a.hunt.samples,
            a.strike.is_some(),
        );
    }
}

fn counts(fauna: &Fauna) -> String {
    let v = fauna.view();
    Founder::ALL
        .iter()
        .map(|f| format!("{}={}", f.name(), v.animals.iter().filter(|a| a.founder == Some(*f)).count()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Per-lineage state the windows integrate.
#[derive(Clone, Copy, Default)]
struct Track {
    min_n: Option<usize>,
    min_adults: Option<usize>,
    body_ticks: u64,
}

fn run(args: &[String]) {
    let hours: f64 = arg(args, "hours=").map_or(2.0, |h| h.parse().unwrap());
    let every: u64 = arg(args, "every=").map_or(10, |m| m.parse().unwrap());
    let arm = arg(args, "arm=").unwrap_or("arm").to_string();
    let params = resolve(args);
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    let (world, mut flora, mut fauna, minute0) = (s.world, s.flora, s.fauna, s.minute);
    flora.replace_config(params.flora_for_world()).expect("the arm's plant config validates");
    fauna.replace_config(params.fauna.clone()).expect("the arm's animal config validates");
    cubarium_voxel_sim::saved::restore(&mut fauna, &[]).expect("the heuristics go back on");
    eprintln!("{arm}: loaded {} at minute {minute0}: {}", dir.display(), counts(&fauna));
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    let per_min = 60 * u64::from(TICK_HZ);
    let mut runner = stage(args, &mut sim, minute0, minute0 + (hours * 60.0).round() as u64);
    let end = runner.as_ref().map_or(minute0 + (hours * 60.0).round() as u64, staged::Runner::end_minute);
    let mut track = [Track::default(); Founder::COUNT];
    let mut win = [Track::default(); Founder::COUNT];
    let l0 = sim.fauna().view().ledger.clone();
    let mut lw = l0.clone();
    let mut t0 = std::time::Instant::now();
    let mut ticks_in_window = 0u64;
    let mut total_ticks = 0u64;
    sample(&sim, &params, &arm, minute0, &lw, &lw, &win, &mut track, 0, 0.0);
    for minute in minute0 + 1..=end {
        for _ in 0..per_min {
            sim.step();
            ticks_in_window += 1;
            total_ticks += 1;
            if let Some(r) = runner.as_mut() {
                r.tick(&sim);
            }
            for a in sim.fauna().view().animals {
                if let Some(f) = a.founder {
                    win[f.index()].body_ticks += 1;
                }
            }
        }
        if (minute - minute0) % every == 0 || minute == end {
            let ms = t0.elapsed().as_secs_f64() * 1000.0 / ticks_in_window.max(1) as f64;
            let l = sim.fauna().view().ledger.clone();
            sample(&sim, &params, &arm, minute, &lw, &l, &win, &mut track, ticks_in_window, ms);
            for i in 0..Founder::COUNT {
                track[i].body_ticks += win[i].body_ticks;
            }
            win = [Track::default(); Founder::COUNT];
            lw = l;
            t0 = std::time::Instant::now();
            ticks_in_window = 0;
        }
        if stop_at(&mut runner, &sim, minute) {
            break;
        }
    }
    let l = sim.fauna().view().ledger.clone();
    println!("sum,{arm},{},{}", minute0, lj_line(&l0, &l));
    for f in PREY {
        if !params.fauna.founder(LJ).hunter.is_some_and(|h| h.hunts(f)) {
            continue;
        }
        let t = &track[f.index()];
        println!(
            "sum,{arm},{},min_n={},min_adults={},{}",
            f.name(),
            t.min_n.unwrap_or(0),
            t.min_adults.unwrap_or(0),
            prey_line(&l0, &l, f, t.body_ticks, total_ticks),
        );
    }
}

/// The lanternjaw's window measures, `a` → `b`.
fn lj_line(a: &FaunaLedger, b: &FaunaLedger) -> String {
    let i = LJ.index();
    let d = |x: &[u64; Founder::COUNT], y: &[u64; Founder::COUNT]| y[i].saturating_sub(x[i]);
    let ratio = |x: f64, y: f64| if y > 0.0 { x / y } else { f64::NAN };
    let alive = d(&a.alive_ticks_by_founder, &b.alive_ticks_by_founder);
    let awake = d(&a.awake_ticks_by_founder, &b.awake_ticks_by_founder);
    let wait = b.hunt_wait_samples - a.hunt_wait_samples;
    let travel = b.hunt_travel_samples - a.hunt_travel_samples;
    let engaged = b.hunt_engaged_samples - a.hunt_engaged_samples;
    let kills: u64 = Founder::ALL
        .iter()
        .map(|&f| b.departed_founder(f, Departure::Killed { by: LJ }) - a.departed_founder(f, Departure::Killed { by: LJ }))
        .sum();
    format!(
        "intake={:.4},awake={:.3},still={:.3},wait={wait},travel={travel},engaged={engaged},fruitless={},explored={},chosen={},strikes={},hits={},kills={kills},laid={},hatched={},attached={},dropoffs={},freed={},clutches={},replacement={},lj_deaths={}",
        b.assimilated_by_founder[i] - a.assimilated_by_founder[i],
        ratio(awake as f64, alive as f64),
        ratio(wait as f64, (wait + travel + engaged) as f64),
        b.hunt_fruitless - a.hunt_fruitless,
        b.hunt_explored - a.hunt_explored,
        b.hunt_chosen - a.hunt_chosen,
        d(&a.strikes_by_founder, &b.strikes_by_founder),
        d(&a.hits_by_founder, &b.hits_by_founder),
        d(&a.eggs_laid_by_founder, &b.eggs_laid_by_founder),
        d(&a.eggs_hatched_by_founder, &b.eggs_hatched_by_founder),
        b.riders_attached - a.riders_attached,
        b.rider_dropoffs - a.rider_dropoffs,
        b.riders_freed - a.riders_freed,
        d(&a.clutches_by_founder, &b.clutches_by_founder),
        d(&a.first_clutches_by_founder, &b.first_clutches_by_founder),
        Departure::ALL
            .iter()
            .filter(|c| c.is_death())
            .map(|&c| b.departed_founder(LJ, c) - a.departed_founder(LJ, c))
            .sum::<u64>(),
    )
}

/// One prey lineage's window measures, `a` → `b`, over `body_ticks` of its bodies in
/// `ticks` of world.
fn prey_line(a: &FaunaLedger, b: &FaunaLedger, f: Founder, body_ticks: u64, ticks: u64) -> String {
    let i = f.index();
    let kills = b.departed_founder(f, Departure::Killed { by: LJ }) - a.departed_founder(f, Departure::Killed { by: LJ });
    let strikes = b.strikes_on_by_founder[i] - a.strikes_on_by_founder[i];
    let contact = b.hunt_encounter_s_by_prey[i] - a.hunt_encounter_s_by_prey[i];
    let declined = b.hunt_declined_s_by_prey[i] - a.hunt_declined_s_by_prey[i];
    let unchosen = b.strikes_unchosen_by_prey[i] - a.strikes_unchosen_by_prey[i];
    let mean_n = if ticks > 0 { body_ticks as f64 / ticks as f64 } else { f64::NAN };
    let hours = ticks as f64 / f64::from(TICK_HZ) / 3600.0;
    let mortality = if mean_n > 0.0 && hours > 0.0 { kills as f64 / mean_n / hours } else { f64::NAN };
    format!("mean_n={mean_n:.2},kills={kills},strikes_on={strikes},unchosen={unchosen},contact_s={contact:.0},declined_s={declined:.0},mortality_per_h={mortality:.4}")
}

#[allow(clippy::too_many_arguments)]
fn sample(
    sim: &Sim,
    params: &Params,
    arm: &str,
    minute: u64,
    a: &FaunaLedger,
    b: &FaunaLedger,
    win: &[Track; Founder::COUNT],
    track: &mut [Track; Founder::COUNT],
    ticks: u64,
    ms: f64,
) {
    let v = sim.fauna().view();
    let bodies = |f: Founder| v.animals.iter().filter(move |x| x.founder == Some(f));
    // The lanternjaw.
    let n = bodies(LJ).count();
    let riders = bodies(LJ).filter(|x| x.attached.is_some()).count();
    let adults = bodies(LJ).filter(|x| balance::is_adult(x, &params.fauna)).count();
    let t = &mut track[LJ.index()];
    t.min_n = Some(t.min_n.map_or(n, |m| m.min(n)));
    t.min_adults = Some(t.min_adults.map_or(adults, |m| m.min(adults)));
    let bank: u64 = v.clutches.iter().filter(|c| c.lineage == LJ).map(|c| u64::from(c.count)).sum();
    let p = params.fauna.founder(LJ);
    let fullness = {
        let free: Vec<_> = bodies(LJ).filter(|x| x.attached.is_none()).collect();
        if free.is_empty() {
            f64::NAN
        } else {
            free.iter().map(|x| x.reserve / p.core.reserve_of(x.body).max(1e-300)).sum::<f64>() / free.len() as f64
        }
    };
    println!(
        "lj,{arm},{minute},n={n},adults={adults},juveniles={},riders={riders},min_n={},min_adults={},bank={bank},fullness={fullness:.3},{}",
        n - adults - riders,
        t.min_n.unwrap_or(0),
        t.min_adults.unwrap_or(0),
        lj_line(a, b),
    );
    for f in PREY {
        if !p.hunter.is_some_and(|h| h.hunts(f)) {
            continue;
        }
        let n = bodies(f).count();
        let adults = bodies(f).filter(|x| balance::is_adult(x, &params.fauna)).count();
        let t = &mut track[f.index()];
        t.min_n = Some(t.min_n.map_or(n, |m| m.min(n)));
        t.min_adults = Some(t.min_adults.map_or(adults, |m| m.min(adults)));
        println!(
            "prey,{arm},{minute},{},n={n},adults={adults},min_n={},min_adults={},{}",
            f.name(),
            t.min_n.unwrap_or(0),
            t.min_adults.unwrap_or(0),
            prey_line(a, b, f, win[f.index()].body_ticks, ticks),
        );
    }
    let fv = sim.flora().view();
    let litter: f64 = fv.ground.iter().map(|g| g.litter).sum();
    let film: f64 = fv.film_sites().map(|(_, f)| f).sum();
    let glow: Vec<_> = fv.stands.iter().filter(|s| s.species == Species::Glowcap).collect();
    let caps: f64 = glow.iter().map(|s| s.foliage).sum();
    println!(
        "stocks,{arm},{minute},litter={litter:.3},film={film:.4},glowcap_stands={},cap_tissue={caps:.4},bodies={},ms_tick={ms:.3},{}",
        glow.len(),
        v.animals.len(),
        Founder::ALL
            .iter()
            .map(|f| format!("{}={}", f.name(), bodies(*f).count()))
            .collect::<Vec<_>>()
            .join(" "),
    );
}
