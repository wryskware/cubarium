//! **The lanternjaw study** (package D2, `design/lanternjaw-detritus-web-proposal-2026-10-05.md`
//! §3.2–3.4 with `design/7_Research/astra-detritus-web-review-2026-10-05.md`): does the
//! patrolling lanternjaw live on the booming small prey, and does it spare the rare ones?
//!
//! ```text
//! hunter_study found seed=N save=DIR [minutes=120] [lanternjaws=3|1,3] [config=FILE] [set=P=V ...] [threads=T]
//! hunter_study run   load=DIR/mM arm=NAME hours=H [every=10] [reseed=N] [config=FILE] [set=...] [run=DIR ...]
//! hunter_study pack  load=DIR/mM [minutes=10] [sample_s=30] [reseed=N] [config=FILE] [set=...]
//! hunter_study ledger load=DIR/mM
//! hunter_study strikes load=DIR/mM [minutes=10] [config=FILE] [set=...]
//! hunter_study churn load=DIR/mM [minutes=10] [config=FILE] [set=...]
//! hunter_study approach load=DIR/mM [minutes=10] [config=FILE] [set=...]
//! hunter_study trace load=DIR/mM [seconds=60] [every_s=1] [config=FILE] [set=...]
//! ```
//!
//! `ledger` (S2b) prints a save's frondgrazer ledger since its founding: deaths by cause,
//! bites, grazer-hours, and the Veilhound's strikes, hits and dodges. `strikes` (S2b) runs a
//! save and prints one `S,` line per Veilhound strike on a frondgrazer as it resolves: the
//! distance and the prey's speed, heading off the striker's bearing, awareness and flight
//! at the windup, the dodge chance at contact and the outcome.
//!
//! `pack` (VEILHOUND-DIAG) prints every Veilhound's state each `sample_s` seconds — tactic
//! state, latch, fullness, distance to its den, nearest frondgrazer — with the pack's target
//! and feast and the chase and strike counters.
//!
//! `reseed=N` (N > 0) re-keys the loaded world's draws (weather, and the layers' keyed draws)
//! by `N`: replicates of one save under other weather and draws.
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
//! - `meal,` (HUNTER-MEAL) for the lanternjaw: lanternjaw-hours (alive, free), kills per
//!   lanternjaw-hour, the organic matter its kills died with (the carrion each laid), the
//!   carrion its mouths took, their ratio (the share of the kills eaten, scavenged carrion
//!   included), assimilated intake per lanternjaw-hour, and the **sated share**: free
//!   lanternjaw ticks with the eating latch off over free lanternjaw ticks.
//! - `vh,` (VEILHOUND-SEARCH) for the Veilhound (`chorister`): bodies, adults, pups, mean
//!   adult fullness, the window's emergency and hungry shares (ticks in an emergency, or
//!   wanting food, over alive ticks), strikes, hits, kills (all, and on frondgrazers), pack
//!   chases and chase kills, chases and draws by the sense that found the prey
//!   `[sight, hearing, smell]`, chases begun while drawn, and the **draw share**: hungry
//!   pack-tactic samples spent leading or following a draw, over all hungry samples.
//! - `vig,` (S2 GROUND RESPONSES): per lineage with a response, scan pauses, flights, alarm
//!   bolts, orientations, freezes and refuge samples; strikes on frondgrazers aware and
//!   unaware of their striker; the Veilhound's stalk, rush and stalk-freeze samples.
//! - `graz,` (S2b) for the frondgrazer: bodies, births (the change plus every departure),
//!   deaths by cause (starved, drowned, aged, removed, killed by the Veilhound, by the
//!   lanternjaw, by anything else), bites, grazer-hours and bites per grazer-hour; the
//!   Veilhound's strikes, hits and dodges.
//! - `age,` (HUNKER): the Veilhound's frondgrazer kills by age class (juvenile under the adult
//!   body, adult). `hunk,` (HUNKER): the frondgrazer's hunker share of its ticks, hunkers and
//!   bunching runs begun, glances, turn-overs, worked hunkered ticks and the hunters in contact
//!   per worked tick, kills by age class (any hunter).
//! - `budget` (HUNKER, the bite-collapse probe): `hunter_study budget load=DIR minutes=M
//!   [vig=off]` prints each frondgrazer's time by what holds it, its bites and their gaps, and
//!   its flights' lengths and what was near while it fled.
//! - `gait,` (GAIT): glides and forced glides, glider climbs onto stems and vines, the
//!   seedporter's flights and refuge samples; on the GAIT branch also sprint ticks and
//!   windings for the Veilhound and the seedporter.
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
        Some("pack") => pack_probe(&args),
        Some("ledger") => ledger_dump(&args),
        Some("strikes") => strike_probe(&args),
        Some("budget") => budget_probe(&args),
        Some("glide") => glide_probe(&args),
        Some("churn") => churn_probe(&args),
        Some("approach") => approach_probe(&args),
        Some("trace") => trace_probe(&args),
        Some("pit") => pit_probe(&args),
        Some("edge") => edge_probe(&args),
        Some("pit2") => pit2_probe(&args),
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
        arg(args, "world=").map_or(cubarium::voxel::Seeds::from(seed), |w| cubarium::voxel::Seeds::Asked(w.parse().unwrap())),
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

/// `pack load=DIR minutes=M [sample_s=30] [reseed=N] [config=...] [set=...]`: the Veilhound
/// diagnosis (VEILHOUND-DIAG): every chorister's state every `sample_s` seconds, and the
/// strike / chase counters per minute.
fn pack_probe(args: &[String]) {
    let params = resolve(args);
    let minutes: u64 = arg(args, "minutes=").map_or(10, |m| m.parse().unwrap());
    let sample_s: u64 = arg(args, "sample_s=").map_or(30, |m| m.parse().unwrap());
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    let (mut world, mut flora, mut fauna, minute0) = (s.world, s.flora, s.fauna, s.minute);
    if let Some(n) = arg(args, "reseed=").and_then(|n| n.parse::<u64>().ok()).filter(|&n| n > 0) {
        let seed = world.config().seed ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        world.reseed(seed);
    }
    flora.replace_config(params.flora_for_world()).expect("plant config");
    fauna.replace_config(params.fauna.clone()).expect("animal config");
    cubarium_voxel_sim::saved::restore(&mut fauna, &[]).expect("heuristics");
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    let ch = Founder::Chorister;
    let p = *params.fauna.founder(ch);
    let man = params.fauna.manifest(ch);
    let ring = f64::from(sim.world().config().width) * sim.world().config().voxel_m;
    let v = sim.world().config().voxel_m;
    println!("cone_range_m={:.2} cruise={:.2} pack={:?}", man.cone_range_m, man.cruise_m_per_s, p.pack);
    let every = sample_s * u64::from(TICK_HZ);
    let total = minutes * 60 * u64::from(TICK_HZ);
    let l0 = sim.fauna().view().ledger.clone();
    for t in 0..=total {
        if t > 0 {
            sim.step();
        }
        if t % every != 0 {
            continue;
        }
        let fa = sim.fauna();
        let view = fa.view();
        let l = &view.ledger;
        let sec = t / u64::from(TICK_HZ);
        println!(
            "T,{},{}s,chases={},chase_kills={},chases_by_sense={:?},draws_by_sense={:?},draw_chases={},draw_lead={},draw_follow={},ch_strikes={},strikes_on_browser={},daylight_hint",
            minute0, sec,
            l.pack_chases - l0.pack_chases,
            l.pack_chase_kills - l0.pack_chase_kills,
            [0, 1, 2].map(|k| l.pack_chases_by_sense[k] - l0.pack_chases_by_sense[k]),
            [0, 1, 2].map(|k| l.pack_draws_by_sense[k] - l0.pack_draws_by_sense[k]),
            l.pack_draw_chases - l0.pack_draw_chases,
            l.pack_draw_lead_samples - l0.pack_draw_lead_samples,
            l.pack_draw_follow_samples - l0.pack_draw_follow_samples,
            l.strikes_by_founder[ch.index()] - l0.strikes_by_founder[ch.index()],
            l.strikes_on_by_founder[Founder::Browser.index()] - l0.strikes_on_by_founder[Founder::Browser.index()],
        );
        for r in fa.packs() {
            let drawn_d = r.drawn.and_then(|d| view.animal(d)).zip(r.breeder.and_then(|b| view.animal(b))).map(|(o, b)| {
                let (dx, dz) = wdelta(b.pose.x, b.pose.z, o.pose.x, o.pose.z, ring);
                (dx * dx + dz * dz).sqrt()
            });
            let target = r.target.and_then(|t| view.animal(t)).map(|o| {
                let b = r.breeder.and_then(|b| view.animal(b));
                let d = b.map_or(f64::NAN, |b| {
                    let (dx, dz) = wdelta(b.pose.x, b.pose.z, o.pose.x, o.pose.z, ring);
                    (dx * dx + dz * dz).sqrt()
                });
                format!("{}@({},{},{})d={d:.1}", o.founder.map_or("?", |f| f.name()), o.site.x, o.site.y, o.site.z)
            });
            let drawn = r.drawn.and_then(|t| view.animal(t)).map(|o| format!("{}@({},{},{})", o.founder.map_or("?", |f| f.name()), o.site.x, o.site.y, o.site.z));
            println!("P,{sec},pack={},den=({},{},{}),breeder={:?},target={:?},target_is={:?},feast={:?},abandoned={:?},drawn={:?},drawn_is={:?},breeder_to_drawn_m={:?}", r.id, r.den.x, r.den.y, r.den.z, r.breeder, r.target, target, r.feast, r.abandoned, r.drawn, drawn, drawn_d.map(|d| (d * 10.0).round() / 10.0));
        }
        for a in view.animals.iter().filter(|a| a.founder == Some(ch)) {
            let cap = p.core.reserve_of(a.body).max(1e-300);
            let k = p.gut.capacity(a.body).max(1e-300);
            let (dden, pk, pup) = match a.pack {
                Some(m) => {
                    let den = fa.pack(m.pack).map(|r| r.den);
                    let d = den.map_or(f64::NAN, |d| {
                        let (cx, cz) = ((f64::from(d.x) + 0.5) * v, (f64::from(d.z) + 0.5) * v);
                        let (dx, dz) = wdelta(a.pose.x, a.pose.z, cx, cz, ring);
                        (dx * dx + dz * dz).sqrt()
                    });
                    (d, m.pack as i64, m.pup)
                }
                None => (f64::NAN, -1, false),
            };
            let (fx, fz) = (a.pose.heading_rad.sin(), a.pose.heading_rad.cos());
            let mut near = f64::INFINITY;
            let mut near_ahead = f64::INFINITY;
            for o in view.animals.iter().filter(|o| o.founder == Some(Founder::Browser)) {
                let (dx, dz) = wdelta(a.pose.x, a.pose.z, o.pose.x, o.pose.z, ring);
                let d = (dx * dx + dz * dz).sqrt();
                near = near.min(d);
                if dx * fx + dz * fz >= 0.0 {
                    near_ahead = near_ahead.min(d);
                }
            }
            println!(
                "A,{sec},id={},pack={pk},pup={pup},body={:.3},state={:?},rest={:?},eating={},emerg={},fullness={:.3},stomach/K={:.3},d_den={:.1},homing={},speed={:.2},site=({},{},{}),near_browser={:.1},ahead={:.1},hunt={:?},strike={}",
                a.id, a.body, a.state, a.rest, a.eating, a.emergency, a.reserve / cap, a.stomach.organic() / k, dden, a.homing_ticks, a.speed_m_s, a.site.x, a.site.y, a.site.z, near, near_ahead, a.hunt.mode, a.strike.is_some(),
            );
        }
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

/// The lanternjaw's meal measures over a window (HUNTER-MEAL), integrated tick by tick.
#[derive(Clone, Copy, Default)]
struct Meal {
    /// Free (unattached) lanternjaw ticks, and those with the eating latch off.
    lj_ticks: u64,
    sated_ticks: u64,
    /// Organic matter the lanternjaws' kills died with (what each corpse laid as carrion).
    kill_organic: f64,
}

impl Meal {
    fn add(&mut self, o: &Meal) {
        self.lj_ticks += o.lj_ticks;
        self.sated_ticks += o.sated_ticks;
        self.kill_organic += o.kill_organic;
    }
}

/// What a body would lay as carrion if it died now (`kill`'s corpse: body, gut, gut seeds,
/// stomach).
fn corpse_organic(a: &cubarium_voxel_fauna::Animal) -> f64 {
    a.organic() + a.gut.organic + a.gut_seeds.total().organic + a.stomach.total().organic
}

fn kills_by_lj(l: &FaunaLedger) -> u64 {
    Founder::ALL.iter().map(|&f| l.departed_founder(f, Departure::Killed { by: LJ })).sum()
}

fn meal_line(a: &FaunaLedger, b: &FaunaLedger, m: &Meal) -> String {
    let i = LJ.index();
    let hours = m.lj_ticks as f64 / f64::from(TICK_HZ) / 3600.0;
    let per_h = |x: f64| if hours > 0.0 { x / hours } else { f64::NAN };
    let kills = kills_by_lj(b) - kills_by_lj(a);
    let eaten = b.eaten_carrion_by_founder[i] - a.eaten_carrion_by_founder[i];
    let intake = b.assimilated_by_founder[i] - a.assimilated_by_founder[i];
    format!(
        "lj_hours={hours:.3},kills={kills},kills_per_lj_h={:.3},kill_organic={:.4},eaten_carrion={eaten:.4},eaten_share={:.3},intake_per_lj_h={:.5},sated={:.3}",
        per_h(kills as f64),
        m.kill_organic,
        if m.kill_organic > 0.0 { eaten / m.kill_organic } else { f64::NAN },
        per_h(intake),
        if m.lj_ticks > 0 { m.sated_ticks as f64 / m.lj_ticks as f64 } else { f64::NAN },
    )
}

/// The HUNTER-MEAL rule's own counters over a window: meal-step samples walking to and
/// eating at a carcass, strikes withheld at one, squares passed over as off route, choices
/// made off the graph, kills remembered productive.
fn route_line(a: &FaunaLedger, b: &FaunaLedger) -> String {
    format!(
        "meal_walk={},meal_eat={},withheld={},unreachable_squares={},off_route={},productive_kills={}",
        b.hunt_meal_walk_samples - a.hunt_meal_walk_samples,
        b.hunt_meal_eat_samples - a.hunt_meal_eat_samples,
        b.hunt_strikes_withheld - a.hunt_strikes_withheld,
        b.hunt_unreachable_squares - a.hunt_unreachable_squares,
        b.hunt_off_route - a.hunt_off_route,
        b.hunt_productive_kills - a.hunt_productive_kills,
    )
}

const VH: Founder = Founder::Chorister;

/// The Veilhound's tick-integrated window measures (VEILHOUND-SEARCH).
#[derive(Clone, Copy, Default)]
struct Vh {
    alive_ticks: u64,
    emergency_ticks: u64,
    hungry_ticks: u64,
}

impl Vh {
    fn add(&mut self, o: &Vh) {
        self.alive_ticks += o.alive_ticks;
        self.emergency_ticks += o.emergency_ticks;
        self.hungry_ticks += o.hungry_ticks;
    }
}

/// **The ground responses' window line** (S2), `a` → `b`: scan pauses, flights, alarm bolts,
/// orientations, freezes and refuge samples per lineage that has any, the strikes on
/// frondgrazers that had sensed their striker and that had not, and the Veilhound's stalk,
/// rush and freeze samples.
fn vig_line(a: &FaunaLedger, b: &FaunaLedger) -> String {
    let (x, y) = (&a.vigil, &b.vigil);
    let d = |p: &[u64; Founder::COUNT], q: &[u64; Founder::COUNT], f: Founder| q[f.index()] - p[f.index()];
    let mut out = Vec::new();
    for f in [Founder::Browser, Founder::Loftstrider, Founder::Seedporter, Founder::Blind] {
        out.push(format!(
            "{}={}/{}/{}/{}/{}/{}",
            f.name(),
            d(&x.scans, &y.scans, f),
            d(&x.flees, &y.flees, f),
            d(&x.alarm_bolts, &y.alarm_bolts, f),
            d(&x.orients, &y.orients, f),
            d(&x.freezes, &y.freezes, f),
            d(&x.refuges, &y.refuges, f),
        ));
    }
    format!(
        "scans/flees/bolts/orients/freezes/refuges,{},grazer_aware={},grazer_unaware={},stalk={},rush={},stalk_freeze={}",
        out.join(","),
        d(&x.aware_strikes, &y.aware_strikes, Founder::Browser),
        d(&x.unaware_strikes, &y.unaware_strikes, Founder::Browser),
        y.stalk_samples - x.stalk_samples,
        y.rush_samples - x.rush_samples,
        y.stalk_freezes - x.stalk_freezes,
    )
}

/// **GAIT's line**, `a` → `b`: the gliders' glides (forced ones too), climbs onto a stem or a
/// vine (refuge entries among them), and the seedporter's flights and refuge samples.
fn gait_line(a: &FaunaLedger, b: &FaunaLedger) -> String {
    let sp = Founder::Seedporter.index();
    format!(
        "glides={},glides_forced={},climbs={},sp_flees={},sp_refuge_samples={},vh_sprint_ticks={},vh_winded={},sp_sprint_ticks={},sp_winded={}",
        b.glides - a.glides,
        b.glides_forced - a.glides_forced,
        b.climbs - a.climbs,
        b.vigil.flees[sp] - a.vigil.flees[sp],
        b.vigil.refuges[sp] - a.vigil.refuges[sp],
        b.sprint_ticks_by_founder[VH.index()] - a.sprint_ticks_by_founder[VH.index()],
        b.winded_by_founder[VH.index()] - a.winded_by_founder[VH.index()],
        b.sprint_ticks_by_founder[sp] - a.sprint_ticks_by_founder[sp],
        b.winded_by_founder[sp] - a.winded_by_founder[sp],
    )
}

/// The Veilhound's window line, `a` → `b`.
fn vh_line(sim: &Sim, params: &Params, a: &FaunaLedger, b: &FaunaLedger, w: &Vh) -> String {
    let i = VH.index();
    let v = sim.fauna().view();
    let p = params.fauna.founder(VH);
    let bodies: Vec<_> = v.animals.iter().filter(|x| x.founder == Some(VH)).collect();
    let pups = bodies.iter().filter(|x| x.pack.is_some_and(|m| m.pup)).count();
    let adults: Vec<_> = bodies.iter().filter(|x| !x.pack.is_some_and(|m| m.pup)).collect();
    let fullness = if adults.is_empty() {
        f64::NAN
    } else {
        adults.iter().map(|x| x.reserve / p.core.reserve_of(x.body).max(1e-300)).sum::<f64>() / adults.len() as f64
    };
    let share = |x: u64| if w.alive_ticks > 0 { x as f64 / w.alive_ticks as f64 } else { f64::NAN };
    let kills = |f: Founder| b.departed_founder(f, Departure::Killed { by: VH }) - a.departed_founder(f, Departure::Killed { by: VH });
    let all_kills: u64 = Founder::ALL.iter().map(|&f| kills(f)).sum();
    // [sight, hearing, tremor, smell] (SENSES-CORE: every sense in the mask counts).
    let d3 = |x: &[u64; 4], y: &[u64; 4]| format!("{}/{}/{}/{}", y[0] - x[0], y[1] - x[1], y[2] - x[2], y[3] - x[3]);
    // The hunt funnel (SENSES-CORE): sensed/approached/struck/killed/eaten, on frondgrazers
    // and on every prey.
    let funnel = |prey: &[Founder]| {
        let n = |s: usize| prey.iter().map(|p| b.funnel[i][p.index()][s] - a.funnel[i][p.index()][s]).sum::<u64>();
        (0..5).map(|s| n(s).to_string()).collect::<Vec<_>>().join("/")
    };
    let hungry = b.pack_hungry_samples - a.pack_hungry_samples;
    let lead = b.pack_draw_lead_samples - a.pack_draw_lead_samples;
    let follow = b.pack_draw_follow_samples - a.pack_draw_follow_samples;
    format!(
        "n={},adults={},pups={pups},fullness={fullness:.3},emergency={:.3},hungry={:.3},strikes={},hits={},kills={all_kills},grazer_kills={},chases={},chase_kills={},chases_by_sense={},draws_by_sense={},draw_chases={},hungry_samples={hungry},draw_lead={lead},draw_follow={follow},draw_share={:.3},vh_hours={:.3},funnel_grazer={},funnel_all={},litters_cum={},pups_born_cum={},pups_indep_cum={},season_waits_cum={},pregnant={},year={}",
        bodies.len(),
        adults.len(),
        share(w.emergency_ticks),
        share(w.hungry_ticks),
        b.strikes_by_founder[i] - a.strikes_by_founder[i],
        b.hits_by_founder[i] - a.hits_by_founder[i],
        kills(Founder::Browser),
        b.pack_chases - a.pack_chases,
        b.pack_chase_kills - a.pack_chase_kills,
        d3(&a.pack_chases_by_sense, &b.pack_chases_by_sense),
        d3(&a.pack_draws_by_sense, &b.pack_draws_by_sense),
        b.pack_draw_chases - a.pack_draw_chases,
        if hungry > 0 { (lead + follow) as f64 / hungry as f64 } else { f64::NAN },
        w.alive_ticks as f64 / f64::from(TICK_HZ) / 3600.0,
        funnel(&[Founder::Browser]),
        funnel(&Founder::ALL),
        // LIFE-HISTORY: litters, pups born and come of age (cumulative), conceptions a
        // season held back, breeders gestating now, and the year (number:phase).
        b.litters,
        b.pups_born,
        b.pups_independent,
        b.season_waits,
        adults.iter().filter(|x| x.reproduction.escrow.is_some()).count(),
        sim.world().year().map_or("-".to_string(), |y| format!("{}:{:.3}", y.number, y.phase)),
    )
}

const GRAZER: Founder = Founder::Browser;

/// **The frondgrazer's window line** (S2b), `a` → `b`: births are the change in bodies plus
/// every departure (deaths and removals), so they are exact without a birth counter.
fn graz_line(a: &FaunaLedger, b: &FaunaLedger, n_before: usize, n_now: usize, body_ticks: u64) -> String {
    let g = GRAZER.index();
    let d = |c: Departure| b.departed_founder(GRAZER, c) - a.departed_founder(GRAZER, c);
    let killed_by = |h: Founder| d(Departure::Killed { by: h });
    let killed_all: u64 = Founder::ALL.iter().map(|&h| killed_by(h)).sum();
    let (vh, lj) = (killed_by(VH), killed_by(LJ));
    let gone = d(Departure::Starved) + d(Departure::Drowned) + d(Departure::Aged) + d(Departure::Removed) + killed_all;
    let born = (n_now as i64 - n_before as i64) + gone as i64;
    let bites = b.bites_by_founder[g] - a.bites_by_founder[g];
    let hours = body_ticks as f64 / f64::from(TICK_HZ) / 3600.0;
    let i = VH.index();
    format!(
        "n={n_now},born={born},starved={},drowned={},aged={},removed={},killed_vh={vh},killed_lj={lj},killed_other={},bites={bites},grazer_h={hours:.3},bites_per_gh={:.1},vh_strikes={},vh_hits={},vh_dodged={}",
        d(Departure::Starved),
        d(Departure::Drowned),
        d(Departure::Aged),
        d(Departure::Removed),
        killed_all - vh - lj,
        if hours > 0.0 { bites as f64 / hours } else { f64::NAN },
        b.strikes_by_founder[i] - a.strikes_by_founder[i],
        b.hits_by_founder[i] - a.hits_by_founder[i],
        b.dodges_by_founder[i] - a.dodges_by_founder[i],
    )
}

/*HUNK{*/
/// **The hunker's window line** (HUNKER), `a` → `b`, for the frondgrazer: the share of its
/// ticks spent hunkered, hunkers and bunching runs begun, strikes that glanced off, turn-overs
/// (opens), hunkered ticks worked by at least one hunter and the mean hunters in contact over
/// them, kills (any hunter) of juveniles and adults.
fn hunk_line(a: &FaunaLedger, b: &FaunaLedger) -> String {
    let g = GRAZER.index();
    let (x, y) = (&a.vigil, &b.vigil);
    let d = |p: &[u64; Founder::COUNT], q: &[u64; Founder::COUNT]| q[g] - p[g];
    let ticks = b.founder_ticks_by_founder[g] - a.founder_ticks_by_founder[g];
    let worked = d(&x.worked_ticks, &y.worked_ticks);
    format!(
        "hunker_share={:.4},hunkers={},bunches={},glances={},opens={},worked_ticks={worked},members_per_worked={:.2},kills_juvenile={},kills_adult={}",
        if ticks > 0 { d(&x.hunker_ticks, &y.hunker_ticks) as f64 / ticks as f64 } else { f64::NAN },
        d(&x.hunkers, &y.hunkers),
        d(&x.bunches, &y.bunches),
        d(&x.glances, &y.glances),
        d(&x.opens, &y.opens),
        if worked > 0 { d(&x.worker_ticks, &y.worker_ticks) as f64 / worked as f64 } else { f64::NAN },
        d(&x.kills_juvenile, &y.kills_juvenile),
        d(&x.kills_adult, &y.kills_adult),
    )
}
/*}HUNK*/

/// **The bellwing's light line** (FINDINGS-FIX), `a` → `l`: bellwing-hours, nectar eaten
/// (gross) per bellwing-hour, glowcap visits begun / failed / left (closed as failed when it
/// flew away; 0 where the build has no such rule) / ended in a sip, flower visits begun /
/// failed, and the living bellwings' mean UV and cyan expectations now.
/// **BELLWING: lure encounters**, sampled once a sim second: seconds with a lit lure (an
/// awake lanternjaw's lure lit), with an awake bellwing in flight, with both, and with the
/// nearest pair (3D, bellwing to lure point) within 4, 10 and 20 m.
#[derive(Clone, Copy, Default)]
struct LureMeet {
    lit_s: u64,
    bw_s: u64,
    both_s: u64,
    near: [u64; 3],
}

impl LureMeet {
    fn tick(&mut self, sim: &Sim) {
        let f = sim.fauna().view();
        if f.tick % u64::from(TICK_HZ) != 0 {
            return;
        }
        let v = sim.world().config().voxel_m;
        let ring = f64::from(sim.world().config().width) * v;
        let lures: Vec<(f64, f64, f64)> = f
            .animals
            .iter()
            .filter(|a| a.state != cubarium_voxel_fauna::State::Asleep && a.attached.is_none() && a.founder_state.lure > 0.0)
            .filter_map(|a| cubarium_voxel_fauna::light::lure_point(&f.config, a, v))
            .collect();
        let bws: Vec<(f64, f64, f64)> = f
            .animals
            .iter()
            .filter(|a| a.founder == Some(Founder::Bellwing) && a.state != cubarium_voxel_fauna::State::Asleep && a.mobility.mode == cubarium_voxel_fauna::MobilityMode::Fly)
            .map(|a| (a.pose.x, a.pose.z, a.pose.y_m))
            .collect();
        self.lit_s += u64::from(!lures.is_empty());
        self.bw_s += u64::from(!bws.is_empty());
        if lures.is_empty() || bws.is_empty() {
            return;
        }
        self.both_s += 1;
        let mut best = f64::INFINITY;
        for &(lx, lz, ly) in &lures {
            for &(bx, bz, by) in &bws {
                let dx = (bx - lx).rem_euclid(ring);
                let dx = dx.min(ring - dx);
                let d = (dx * dx + (bz - lz).powi(2) + (by - ly).powi(2)).sqrt();
                best = best.min(d);
            }
        }
        for (k, r) in [4.0, 10.0, 20.0].into_iter().enumerate() {
            self.near[k] += u64::from(best <= r);
        }
    }

    fn line(&self) -> String {
        format!("lit_s={},bw_awake_s={},both_s={},near4_s={},near10_s={},near20_s={}", self.lit_s, self.bw_s, self.both_s, self.near[0], self.near[1], self.near[2])
    }
}

fn bwl_line(sim: &Sim, a: &FaunaLedger, l: &FaunaLedger) -> String {
    let b = Founder::Bellwing.index();
    let hours = (l.founder_ticks_by_founder[b] - a.founder_ticks_by_founder[b]) as f64 / f64::from(TICK_HZ) / 3600.0;
    let nectar = l.eaten_by_food[cubarium_voxel_fauna::Food::Nectar.index()] - a.eaten_by_food[cubarium_voxel_fauna::Food::Nectar.index()];
    let fauna = sim.fauna();
    let cfg = fauna.view().config.light;
    let now = fauna.view().tick;
    let (mut n, mut uv, mut cyan) = (0usize, 0.0, 0.0);
    for x in fauna.view().animals.iter().filter(|x| x.founder == Some(Founder::Bellwing)) {
        let p = fauna.percept(x.id).copied().unwrap_or_default();
        uv += cubarium_voxel_fauna::light::expectation(&cfg, &p, cubarium_voxel_fauna::light::BeaconKind::Flower, now);
        cyan += cubarium_voxel_fauna::light::expectation(&cfg, &p, cubarium_voxel_fauna::light::BeaconKind::Glowcap, now);
        n += 1;
    }
    let m = |s: f64| if n > 0 { s / n as f64 } else { f64::NAN };
    // BELLWING: the lure's visits, the bellwing's eggs and its kills by killer.
    let d = |x: &[u64; Founder::COUNT], y: &[u64; Founder::COUNT]| y[b].saturating_sub(x[b]);
    let killed = |by: Founder| l.departed_founder(Founder::Bellwing, Departure::Killed { by }) - a.departed_founder(Founder::Bellwing, Departure::Killed { by });
    let lj = LJ.index();
    let lj_hours = (l.alive_ticks_by_founder[lj] - a.alive_ticks_by_founder[lj]) as f64 / f64::from(TICK_HZ) / 3600.0;
    let budget = |m: cubarium_voxel_fauna::light::LightMode| l.light_budget[b][m as usize] - a.light_budget[b][m as usize];
    let eggs_now: u32 = fauna.view().clutches.iter().filter(|c| c.lineage == Founder::Bellwing).map(|c| c.count).sum();
    format!(
        "bw_hours={:.3},nectar_per_bwh={:.5},glowcap_visits={},glowcap_failed={},glowcap_left={},glowcap_sipped={},flower_visits={},flower_failed={},bellwings={n},uv_expect={:.3},cyan_expect={:.3},lure_visits={},lure_failed={},lure_kills={},bw_laid={},bw_hatched={},bw_clutches={},bw_eggs_now={eggs_now},bw_nest_refusals={},bw_ceiling_refusals={},bw_killed_lj={},bw_killed_vh={},bw_killed_sp={},bw_starved={},bw_aged={},lj_hours={:.3},bw_killed_lj_per_ljh={:.4},approach_lure={},inspect_lure={},repel={},light_samples={}",
        hours,
        if hours > 0.0 { nectar / hours } else { f64::NAN },
        l.beacon_visits[b][1] - a.beacon_visits[b][1],
        l.failed_visits[b][1] - a.failed_visits[b][1],
        l.left_visits[b][1] - a.left_visits[b][1],
        l.sipped_visits[b][1] - a.sipped_visits[b][1],
        l.beacon_visits[b][0] - a.beacon_visits[b][0],
        l.failed_visits[b][0] - a.failed_visits[b][0],
        m(uv),
        m(cyan),
        l.beacon_visits[b][2] - a.beacon_visits[b][2],
        l.failed_visits[b][2] - a.failed_visits[b][2],
        d(&a.lure_kills, &l.lure_kills),
        d(&a.eggs_laid_by_founder, &l.eggs_laid_by_founder),
        d(&a.eggs_hatched_by_founder, &l.eggs_hatched_by_founder),
        d(&a.clutches_by_founder, &l.clutches_by_founder),
        d(&a.nest_refusals_by_founder, &l.nest_refusals_by_founder),
        d(&a.ceiling_refusals_by_founder, &l.ceiling_refusals_by_founder),
        killed(LJ),
        killed(Founder::Chorister),
        killed(Founder::Seedporter),
        l.departed_founder(Founder::Bellwing, Departure::Starved) - a.departed_founder(Founder::Bellwing, Departure::Starved),
        l.departed_founder(Founder::Bellwing, Departure::Aged) - a.departed_founder(Founder::Bellwing, Departure::Aged),
        lj_hours,
        if lj_hours > 0.0 { killed(LJ) as f64 / lj_hours } else { f64::NAN },
        budget(cubarium_voxel_fauna::light::LightMode::ApproachLure),
        budget(cubarium_voxel_fauna::light::LightMode::InspectLure),
        budget(cubarium_voxel_fauna::light::LightMode::Repel),
        (0..cubarium_voxel_fauna::light::LightMode::COUNT).map(|k| l.light_budget[b][k] - a.light_budget[b][k]).sum::<u64>(),
    )
}

/// `ledger load=DIR` (S2b): a save's frondgrazer ledger since its founding.
fn ledger_dump(args: &[String]) {
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    let v = s.fauna.view();
    let l = &v.ledger;
    let n = v.animals.iter().filter(|a| a.founder == Some(GRAZER)).count();
    let zero = FaunaLedger::default();
    let g = GRAZER.index();
    println!(
        "ledger,{},minute={},{},grazer_ticks={},aware_strikes={},unaware_strikes={}",
        dir.display(),
        s.minute,
        graz_line(&zero, l, 0, n, l.founder_ticks_by_founder[g]),
        l.founder_ticks_by_founder[g],
        l.vigil.aware_strikes[g],
        l.vigil.unaware_strikes[g],
    );
}

/// One Veilhound strike on a frondgrazer, from its windup.
#[derive(Clone, Copy)]
struct StrikeRec {
    prey: u64,
    start_tick: u64,
    d_m: f64,
    prey_speed: f64,
    vh_speed: f64,
    rel_deg: f64,
    aware: bool,
    fleeing: bool,
    bolting: bool,
    scanning: bool,
    flight_age_s: f64,
    flight_d_m: f64,
}

/// Degrees between the prey's heading and the striker's bearing to it: 0 runs straight away.
fn rel_heading_deg(s: &cubarium_voxel_fauna::Animal, p: &cubarium_voxel_fauna::Animal, ring: f64) -> f64 {
    let (dx, dz) = wdelta(s.pose.x, s.pose.z, p.pose.x, p.pose.z, ring);
    let bearing = dx.atan2(dz);
    let d = (p.pose.heading_rad - bearing).rem_euclid(std::f64::consts::TAU);
    (if d > std::f64::consts::PI { std::f64::consts::TAU - d } else { d }).to_degrees()
}

/// `strikes load=DIR minutes=M` (S2b, Q2): every Veilhound strike on a frondgrazer, one `S,`
/// line as it resolves, then the tallies.
fn strike_probe(args: &[String]) {
    use cubarium_voxel_fauna::StrikePhase;
    let params = resolve(args);
    let minutes: u64 = arg(args, "minutes=").map_or(10, |m| m.parse().unwrap());
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    let (world, mut flora, mut fauna, minute0) = (s.world, s.flora, s.fauna, s.minute);
    flora.replace_config(params.flora_for_world()).expect("plant config");
    fauna.replace_config(params.fauna.clone()).expect("animal config");
    cubarium_voxel_sim::saved::restore(&mut fauna, &[]).expect("heuristics");
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    let ring = f64::from(sim.world().config().width) * sim.world().config().voxel_m;
    let hz = f64::from(TICK_HZ);
    let total = minutes * 60 * u64::from(TICK_HZ);
    let mut open: rustc_hash::FxHashMap<u64, StrikeRec> = Default::default();
    // Each grazer's flight: the tick it began and its nearest Veilhound then.
    let mut flight: rustc_hash::FxHashMap<u64, (u64, f64)> = Default::default();
    let mut tallies: rustc_hash::FxHashMap<String, u64> = Default::default();
    println!("S,minute0={minute0},cols=tick,striker,prey,d_m,prey_speed,vh_speed,rel_deg,aware_windup,fleeing,bolting,scanning,flight_age_s,flight_d_m,aware_contact,from_rear,condition,chance,outcome");
    for _ in 0..total {
        // Before the step: the strikes that resolve in it (a lunge on its last tick), and
        // the prey as it stands going into it.
        let fa = sim.fauna();
        let v = fa.view();
        let now = v.tick;
        let l0 = v.ledger.clone();
        let mut resolving: Vec<(u64, StrikeRec, bool, bool, f64, f64)> = Vec::new();
        for a in v.animals.iter().filter(|a| a.founder == Some(VH)) {
            let Some(st) = a.strike else { continue };
            if st.phase != StrikePhase::Lunge || st.ticks_left != 1 {
                continue;
            }
            let Some(rec) = open.get(&a.id).copied() else { continue };
            let Some(p) = v.animal(rec.prey).copied() else { continue };
            let pp = params.fauna.founder(GRAZER);
            let (dx, dz) = wdelta(a.pose.x, a.pose.z, p.pose.x, p.pose.z, ring);
            let (fx, fz) = p.pose.forward();
            let from_rear = -dx * fx + -dz * fz < 0.0;
            let cap = pp.core.reserve_of(p.body);
            let condition = if cap > 0.0 { p.reserve / cap } else { 0.0 };
            let record = fa.percept(p.id);
            let aware = cubarium_voxel_fauna::vigil::aware_of(&params.fauna, record, a.id, now);
            let awareness = cubarium_voxel_fauna::vigil::awareness(&params.fauna, &p, record, a.id, now);
            let chance = cubarium_voxel_fauna::dodge_chance_with(&params.fauna.predation, pp.agility, awareness, from_rear, condition);
            resolving.push((a.id, rec, aware, from_rear, condition, chance));
        }
        sim.step();
        let fa = sim.fauna();
        let v = fa.view();
        let l1 = &v.ledger;
        let i = VH.index();
        let dodged = l1.dodges_by_founder[i] - l0.dodges_by_founder[i];
        let hit = l1.hits_by_founder[i] - l0.hits_by_founder[i];
        let killed = l1.departed_founder(GRAZER, Departure::Killed { by: VH }) - l0.departed_founder(GRAZER, Departure::Killed { by: VH });
        for (id, rec, aware, from_rear, condition, chance) in &resolving {
            let outcome = if resolving.len() > 1 {
                "ambiguous"
            } else if killed > 0 && v.animal(rec.prey).is_none() {
                "kill"
            } else if hit > 0 {
                "hit"
            } else if dodged > 0 {
                "dodge"
            } else {
                "miss"
            };
            *tallies.entry(format!("{outcome},{}", if *aware { "aware" } else { "unaware" })).or_default() += 1;
            println!(
                "S,{},{id},{},{:.2},{:.2},{:.2},{:.0},{},{},{},{},{:.1},{:.2},{aware},{from_rear},{condition:.2},{chance:.3},{outcome}",
                rec.start_tick, rec.prey, rec.d_m, rec.prey_speed, rec.vh_speed, rec.rel_deg, rec.aware, rec.fleeing, rec.bolting, rec.scanning, rec.flight_age_s, rec.flight_d_m,
            );
        }
        for (id, ..) in &resolving {
            open.remove(id);
        }
        let now = v.tick;
        // Flights begun this tick.
        for p in v.animals.iter().filter(|a| a.founder == Some(GRAZER)) {
            let vg = fa.percept(p.id).map(|r| r.vigil).unwrap_or_default();
            if vg.fleeing || vg.running(now) {
                flight.entry(p.id).or_insert_with(|| {
                    let d = v
                        .animals
                        .iter()
                        .filter(|o| o.founder == Some(VH))
                        .map(|o| {
                            let (dx, dz) = wdelta(o.pose.x, o.pose.z, p.pose.x, p.pose.z, ring);
                            (dx * dx + dz * dz).sqrt()
                        })
                        .fold(f64::INFINITY, f64::min);
                    (now, d)
                });
            } else {
                flight.remove(&p.id);
            }
        }
        // Windups begun this tick on a frondgrazer.
        for a in v.animals.iter().filter(|a| a.founder == Some(VH)) {
            let Some(st) = a.strike else { continue };
            if st.phase != StrikePhase::Windup || open.contains_key(&a.id) {
                continue;
            }
            let Some(p) = v.animal(st.prey).copied() else { continue };
            if p.founder != Some(GRAZER) {
                continue;
            }
            let (dx, dz) = wdelta(a.pose.x, a.pose.z, p.pose.x, p.pose.z, ring);
            let vg = fa.percept(p.id).map(|r| r.vigil).unwrap_or_default();
            let (fl_age, fl_d) = flight.get(&p.id).map_or((f64::NAN, f64::NAN), |&(t, d)| ((now - t) as f64 / hz, d));
            open.insert(
                a.id,
                StrikeRec {
                    prey: p.id,
                    start_tick: now,
                    d_m: (dx * dx + dz * dz).sqrt(),
                    prey_speed: p.speed_m_s,
                    vh_speed: a.speed_m_s,
                    rel_deg: rel_heading_deg(a, &p, ring),
                    aware: cubarium_voxel_fauna::vigil::aware_of(&params.fauna, fa.percept(p.id), a.id, now),
                    fleeing: vg.fleeing,
                    bolting: now < vg.bolt_until,
                    scanning: vg.scanning(now),
                    flight_age_s: fl_age,
                    flight_d_m: fl_d,
                },
            );
        }
        // A strike that ended without resolving here (its prey gone): dropped.
        open.retain(|id, _| v.animal(*id).is_some_and(|a| a.strike.is_some()));
    }
    let mut t: Vec<_> = tallies.into_iter().collect();
    t.sort();
    for (k, n) in t {
        println!("T,{k},{n}");
    }
}

fn run(args: &[String]) {
    let hours: f64 = arg(args, "hours=").map_or(2.0, |h| h.parse().unwrap());
    let every: u64 = arg(args, "every=").map_or(10, |m| m.parse().unwrap());
    let arm = arg(args, "arm=").unwrap_or("arm").to_string();
    let params = resolve(args);
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    let (mut world, mut flora, mut fauna, minute0) = (s.world, s.flora, s.fauna, s.minute);
    if let Some(n) = arg(args, "reseed=").and_then(|n| n.parse::<u64>().ok()).filter(|&n| n > 0) {
        let seed = world.config().seed ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        world.reseed(seed);
        eprintln!("{arm}: reseeded by {n} (world seed {seed:#x})");
    }
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
    let mut meal_win = Meal::default();
    let mut meal_all = Meal::default();
    let mut vh_win = Vh::default();
    let mut vh_all = Vh::default();
    let preys: Vec<Founder> = Founder::ALL
        .iter()
        .copied()
        .filter(|&f| params.fauna.founder(LJ).hunter.is_some_and(|h| h.hunts(f)))
        .collect();
    let prey_bodies = |sim: &Sim| -> rustc_hash::FxHashMap<u64, f64> {
        sim.fauna()
            .view()
            .animals
            .iter()
            .filter(|a| a.founder.is_some_and(|f| preys.contains(&f)))
            .map(|a| (a.id, corpse_organic(a)))
            .collect()
    };
    let grazers = |sim: &Sim| sim.fauna().view().animals.iter().filter(|a| a.founder == Some(GRAZER)).count();
    let mut grazers_before = grazers(&sim);
    let grazers0 = grazers_before;
    let mut before = prey_bodies(&sim);
    let mut kills_before = kills_by_lj(&sim.fauna().view().ledger);
    // HUNKER's readout: the Veilhound's frondgrazer kills by the prey's age class (juvenile,
    // adult), from the bodies that went in the tick a kill was booked (both arms).
    let grazer_birth = params.fauna.founder(GRAZER).core.birth_body;
    let vh_grazer_kills = |sim: &Sim| sim.fauna().view().ledger.departed_founder(GRAZER, Departure::Killed { by: VH });
    let grazer_ages = |sim: &Sim| -> rustc_hash::FxHashMap<u64, bool> {
        sim.fauna().view().animals.iter().filter(|a| a.founder == Some(GRAZER)).map(|a| (a.id, a.body < grazer_birth)).collect()
    };
    let mut ages = grazer_ages(&sim);
    let mut vh_kills_before = vh_grazer_kills(&sim);
    let (mut age_win, mut age_all) = ([0u64; 2], [0u64; 2]);
    let (mut meet_win, mut meet_all) = (LureMeet::default(), LureMeet::default());
    let mut hunts = Hunts { log: arg(args, "hunt_log=") == Some("1"), chase_kills: sim.fauna().view().ledger.pack_chase_kills, ..Hunts::default() };
    let (mut strk_win, mut strk_all, mut strk_on) = (Contact::default(), Contact::default(), rustc_hash::FxHashSet::default());
    let mut stuck = Stuck { log: arg(args, "wedge_log=") == Some("1"), ..Stuck::default() };
    let mut wedge = Wedge::default();
    // EDGE: bodies born by lineage (a new id), against the ids standing at the start.
    let mut seen: rustc_hash::FxHashSet<u64> = sim.fauna().view().animals.iter().map(|a| a.id).collect();
    let mut born = [0u64; 9];
    let life0 = sim.fauna().view().ledger.clone();
    let vh_cadence = params.fauna.manifest(VH).cadence_ticks().max(1);
    let mut ends = Ends { log: arg(args, "ends_log=") == Some("1"), oor: sim.fauna().view().ledger.pack_out_of_reach, ..Ends::default() };
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
                    if seen.insert(a.id) {
                        born[f.index()] += 1;
                    }
                    win[f.index()].body_ticks += 1;
                    if f == VH {
                        vh_win.alive_ticks += 1;
                        vh_win.emergency_ticks += u64::from(a.emergency);
                        vh_win.hungry_ticks += u64::from(a.eating || a.emergency);
                    }
                    if f == LJ && a.attached.is_none() {
                        meal_win.lj_ticks += 1;
                        if !a.eating {
                            meal_win.sated_ticks += 1;
                        }
                    }
                }
            }
            // The kills this tick: the prey bodies gone, what they died with, shared out
            // when deaths of other causes fell in the same tick.
            let kills_now = kills_by_lj(&sim.fauna().view().ledger);
            let now = prey_bodies(&sim);
            if kills_now > kills_before {
                let gone: Vec<f64> = before.iter().filter(|(id, _)| !now.contains_key(id)).map(|(_, o)| *o).collect();
                if !gone.is_empty() {
                    let k = (kills_now - kills_before) as f64;
                    let share = (k / gone.len() as f64).min(1.0);
                    meal_win.kill_organic += share * gone.iter().sum::<f64>();
                }
            }
            kills_before = kills_now;
            let vk = vh_grazer_kills(&sim);
            let ages_now = grazer_ages(&sim);
            if vk > vh_kills_before {
                let mut left = vk - vh_kills_before;
                for (id, &juvenile) in ages.iter() {
                    if left > 0 && !ages_now.contains_key(id) {
                        age_win[usize::from(!juvenile)] += 1;
                        left -= 1;
                    }
                }
            }
            vh_kills_before = vk;
            ages = ages_now;
            before = now;
            hunts.tick(&sim, &params);
            meet_win.tick(&sim);
            strk_win.tick(&sim, &params, &mut strk_on);
            ends.tick(&mut sim, &params);
            stuck.tick(&sim, &params);
            wedge.tick(&sim, &params);
        }
        if (minute - minute0) % every == 0 || minute == end {
            let ms = t0.elapsed().as_secs_f64() * 1000.0 / ticks_in_window.max(1) as f64;
            let l = sim.fauna().view().ledger.clone();
            sample(&sim, &params, &arm, minute, &lw, &l, &win, &mut track, ticks_in_window, ms);
            println!("meal,{arm},{minute},{}", meal_line(&lw, &l, &meal_win));
            println!("route,{arm},{minute},{}", route_line(&lw, &l));
            println!("vh,{arm},{minute},{}", vh_line(&sim, &params, &lw, &l, &vh_win));
            println!("snail,{arm},{minute},{}", snail_line(&sim, &lw, &l));
            println!("vig,{arm},{minute},{}", vig_line(&lw, &l));
            let g_now = grazers(&sim);
            println!("graz,{arm},{minute},{}", graz_line(&lw, &l, grazers_before, g_now, win[GRAZER.index()].body_ticks));
            grazers_before = g_now;
            println!("age,{arm},{minute},vh_kills_juvenile={},vh_kills_adult={}", age_win[0], age_win[1]);
            age_all[0] += age_win[0];
            age_all[1] += age_win[1];
            age_win = [0; 2];
            /*HUNK{*/
            println!("hunk,{arm},{minute},{}", hunk_line(&lw, &l));
            println!("ffx,{arm},{minute},open_exits={},bared_entries={}", l.vigil.open_exits[GRAZER.index()] - lw.vigil.open_exits[GRAZER.index()], l.vigil.bared_entries[GRAZER.index()] - lw.vigil.bared_entries[GRAZER.index()]);
            println!("bwl,{arm},{minute},{}", bwl_line(&sim, &lw, &l));
            println!("meet,{arm},{minute},{}", meet_win.line());
            for (x, y) in [(&mut meet_all.lit_s, meet_win.lit_s), (&mut meet_all.bw_s, meet_win.bw_s), (&mut meet_all.both_s, meet_win.both_s)] {
                *x += y;
            }
            for k in 0..3 {
                meet_all.near[k] += meet_win.near[k];
            }
            meet_win = LureMeet::default();
            /*}HUNK*/
            println!("gait,{arm},{minute},{}", gait_line(&lw, &l));
            println!("hunt,{arm},{minute},{}", hunts.win.line());
            /*PT{*/
            println!("ptune,{arm},{minute},break_offs={},declined={},switches={}", l.pack_break_offs - lw.pack_break_offs, l.pack_declined - lw.pack_declined, l.pack_switches - lw.pack_switches);
            /*}PT*/
            println!("strk,{arm},{minute},{}", strk_win.line(&lw, &l));
            println!("strkcls,{arm},{minute},{}", strk_win.cls_line());
            println!("pry,{arm},{minute},{}", strk_win.pry_line());
            println!("ends,{arm},{minute},{}", Ends::line(&ends.win));
            ends.win.clear();
            println!("stuck,{arm},{minute},{}", Stuck::line(&stuck.win, vh_cadence));
            stuck.win = [0; 4];
            println!("wedge,{arm},{minute},{}", wedge.line(false));
            wedge.win = Default::default();
            strk_all.add(&strk_win);
            strk_win = Contact::default();
            hunts.win = HuntTally::default();
            vh_all.add(&vh_win);
            vh_win = Vh::default();
            meal_all.add(&meal_win);
            meal_win = Meal::default();
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
    meal_all.add(&meal_win);
    println!("summeal,{arm},{},{}", minute0, meal_line(&l0, &l, &meal_all));
    println!("sumroute,{arm},{},{}", minute0, route_line(&l0, &l));
    // The hunt funnel per hunter and prey over the run (S2's readout): sensed, approached,
    // struck, killed, eaten.
    for h in Founder::ALL {
        for p in Founder::ALL {
            let n: Vec<u64> = (0..5).map(|s| l.funnel[h.index()][p.index()][s] - l0.funnel[h.index()][p.index()][s]).collect();
            if n.iter().any(|&x| x > 0) {
                println!("funnel,{arm},{},{},{}", h.name(), p.name(), n.iter().map(u64::to_string).collect::<Vec<_>>().join(","));
            }
        }
    }
    vh_all.add(&vh_win);
    println!("sumvh,{arm},{},{}", minute0, vh_line(&sim, &params, &l0, &l, &vh_all));
    println!("sumsnail,{arm},{},{}", minute0, snail_line(&sim, &l0, &l));
    println!("sumvig,{arm},{},{}", minute0, vig_line(&l0, &l));
    println!("sumgraz,{arm},{},{}", minute0, graz_line(&l0, &l, grazers0, grazers(&sim), track[GRAZER.index()].body_ticks));
    println!("sumage,{arm},{},vh_kills_juvenile={},vh_kills_adult={}", minute0, age_all[0] + age_win[0], age_all[1] + age_win[1]);
    /*HUNK{*/
    println!("sumhunk,{arm},{},{}", minute0, hunk_line(&l0, &l));
    println!("sumffx,{arm},{},open_exits={},bared_entries={}", minute0, l.vigil.open_exits[GRAZER.index()] - l0.vigil.open_exits[GRAZER.index()], l.vigil.bared_entries[GRAZER.index()] - l0.vigil.bared_entries[GRAZER.index()]);
    println!("sumbwl,{arm},{},{}", minute0, bwl_line(&sim, &l0, &l));
    println!("summeet,{arm},{},{}", minute0, meet_all.line());
    /*}HUNK*/
    println!("sumgait,{arm},{},{}", minute0, gait_line(&l0, &l));
    println!("sumhunt,{arm},{},{}", minute0, hunts.all.line());
    strk_all.add(&strk_win);
    println!("sumstrk,{arm},{},{}", minute0, strk_all.line(&l0, &l));
    println!("sumstrkcls,{arm},{},{}", minute0, strk_all.cls_line());
    println!("sumpry,{arm},{},{}", minute0, strk_all.pry_line());
    println!("sumends,{arm},{},{}", minute0, Ends::line(&ends.all));
    println!("sumstuck,{arm},{},{}", minute0, Stuck::line(&stuck.all, vh_cadence));
    println!("sumwedge,{arm},{},{}", minute0, wedge.line(true));
    println!("sumwedgewhy,{arm},{},{}", minute0, wedge.why_line());
    // EDGE: per lineage, the founder-hours alive, the organic swallowed per alive hour (its
    // intake) and the bodies born over the run.
    {
        let l = sim.fauna().view().ledger.clone();
        let parts: Vec<String> = Founder::ALL
            .iter()
            .map(|f| {
                let k = f.index();
                let h = (l.alive_ticks_by_founder[k] - life0.alive_ticks_by_founder[k]) as f64 / f64::from(TICK_HZ) / 3600.0;
                let eat = l.swallowed_by_founder[k] - life0.swallowed_by_founder[k];
                format!("{}=alive_h:{h:.2}/intake_per_h:{:.5}/born:{}", f.name(), if h > 0.0 { eat / h } else { f64::NAN }, born[k])
            })
            .collect();
        println!("sumlife,{arm},{},{}", minute0, parts.join(","));
    }
    /*PT{*/
    println!("sumptune,{arm},{},break_offs={},declined={},switches={}", minute0, l.pack_break_offs - l0.pack_break_offs, l.pack_declined - l0.pack_declined, l.pack_switches - l0.pack_switches);
    /*}PT*/
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

/// **AMPHIBIOUS-SNAIL** readouts, `a` → `b`: ripple-snails now; their deaths by
/// desiccation, by lanternjaws and in all; the share of amphibious ticks ashore by day and
/// by night; intake (assimilated) per snail-hour and the share of it taken ashore; and
/// retractions begun.
fn snail_line(sim: &Sim, a: &FaunaLedger, b: &FaunaLedger) -> String {
    let f = Founder::RippleSnail;
    let i = f.index();
    let now = sim.fauna().view().animals.iter().filter(|x| x.founder == Some(f)).count();
    let dead = |c: Departure| b.departed_founder(f, c) - a.departed_founder(f, c);
    let all: u64 = Departure::ALL.iter().filter(|c| c.is_death()).map(|&c| dead(c)).sum();
    let t = |d: usize, w: usize| b.amphibious_ticks[d][w].saturating_sub(a.amphibious_ticks[d][w]) as f64;
    let share = |d: usize| {
        let n = t(d, 0) + t(d, 1);
        if n > 0.0 { t(d, 1) / n } else { f64::NAN }
    };
    let alive = b.alive_ticks_by_founder[i].saturating_sub(a.alive_ticks_by_founder[i]) as f64;
    let hours = alive / f64::from(TICK_HZ) / 3600.0;
    let intake = b.assimilated_by_founder[i] - a.assimilated_by_founder[i];
    let ashore = b.ashore_assimilated - a.ashore_assimilated;
    let ratio = |x: f64, y: f64| if y > 0.0 { x / y } else { f64::NAN };
    // SNAIL-FIX: the film on land-wet ground now, the faces holding it, and the land-wet
    // faces (column tops) it could stand on.
    let (land_film, land_filmed, land_wet) = cubarium_voxel_flora::land_census(sim.flora(), sim.world());
    let per_wet = if land_wet > 0 { land_film / land_wet as f64 } else { f64::NAN };
    format!(
        "n={now},desiccated={},lj_kills={},deaths={all},land_day={:.3},land_night={:.3},snail_h={hours:.2},intake_per_h={:.6},ashore_intake_share={:.3},retracts={},land_film={land_film:.5},land_film_faces={land_filmed},land_wet_faces={land_wet},land_film_per_wet_face={per_wet:.3e}",
        dead(Departure::Desiccation),
        dead(Departure::Killed { by: LJ }),
        share(0),
        share(1),
        ratio(intake, hours),
        ratio(ashore, intake),
        b.retracts - a.retracts,
    )
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

fn wdelta(fx: f64, fz: f64, tx: f64, tz: f64, ring: f64) -> (f64, f64) {
    let mut dx = (tx - fx).rem_euclid(ring);
    if dx > 0.5 * ring {
        dx -= ring;
    }
    (dx, tz - fz)
}

/// `budget load=DIR minutes=M [vig=off]` (HUNKER, the bite-collapse probe): every frondgrazer's
/// time per tick by what holds it (asleep, flight, bolt, orientation, scan, feeding still,
/// feeding on the move, walking, still), its bites and the gaps between them, and its
/// flights' lengths and what was near while it fled. `vig=off` turns every lineage's ground
/// response off on the same save.
// ------------------------------------------------------------------ PACK-TUNE: the hunt
//
// **A hunt** is one pack pursuit of one target, from commitment (the pack takes it) to a kill
// or a give-up. The pack's record is read every tick: a new target is a chase; a chase on the
// target the pack dropped alive within `HUNT_MERGE_S` is the same hunt resumed, not a new
// one; a new target within `RETARGET_S` of a dropped one is a re-target (a new hunt, counted
// apart so the measure is not inflated). A hunt is killed when its target died while it was
// the target and the pack booked a chase kill that tick.
const HUNT_MERGE_S: f64 = 60.0;
const RETARGET_S: f64 = 30.0;
/// A target with no kin of its lineage within this is isolated (readout only).
const ISOLATED_M: f64 = 3.0;
/// The prey classes at commitment: hunkered adult, other adult, juvenile, airborne, aware,
/// isolated (a hunt can be in several).
const HUNT_CLASSES: [&str; 6] = ["hunk_adult", "adult", "juvenile", "airborne", "aware", "isolated"];

#[derive(Clone, Copy, Default)]
struct HuntCur {
    target: u64,
    prey: usize,
    since: u64,
    struck: bool,
    classes: [bool; 6],
    chases: u32,
}

#[derive(Clone, Default)]
struct HuntTally {
    chases: u64,
    hunts: u64,
    resumes: u64,
    retargets: u64,
    kills: u64,
    struck: u64,
    struck_kills: u64,
    dropped_alive: u64,
    gone: u64,
    secs: f64,
    by_prey: [[u64; 2]; Founder::COUNT],
    by_class: [[u64; 2]; 6],
}

impl HuntTally {
    fn line(&self) -> String {
        let prey = Founder::ALL
            .iter()
            .filter(|f| self.by_prey[f.index()][0] > 0)
            .map(|f| format!("{}:{}/{}", f.name(), self.by_prey[f.index()][1], self.by_prey[f.index()][0]))
            .collect::<Vec<_>>()
            .join(";");
        let classes = HUNT_CLASSES
            .iter()
            .enumerate()
            .map(|(k, n)| format!("{n}:{}/{}", self.by_class[k][1], self.by_class[k][0]))
            .collect::<Vec<_>>()
            .join(";");
        format!(
            "chases={},hunts={},resumes={},retargets={},kills={},success={:.3},struck_hunts={},struck_kills={},dropped_alive={},gone={},mean_hunt_s={:.1},prey={prey},class={classes}",
            self.chases,
            self.hunts,
            self.resumes,
            self.retargets,
            self.kills,
            self.kills as f64 / self.hunts.max(1) as f64,
            self.struck,
            self.struck_kills,
            self.dropped_alive,
            self.gone,
            self.secs / self.hunts.max(1) as f64,
        )
    }
}

#[derive(Default)]
struct Hunts {
    cur: rustc_hash::FxHashMap<u64, HuntCur>,
    last_end: rustc_hash::FxHashMap<u64, (u64, u64, bool, bool, HuntCur)>,
    chase_kills: u64,
    win: HuntTally,
    all: HuntTally,
    log: bool,
}

impl Hunts {
    fn end(&mut self, pack: u64, c: HuntCur, now: u64, alive: bool, killed: bool, resumable: bool) {
        // A hunt is tallied when it ends for good: a kill, its target gone, or no resume in time.
        let _ = resumable;
        self.last_end.insert(pack, (c.target, now, killed || !alive, killed, c));
        if killed || !alive {
            self.close(c, now, alive, killed);
        }
    }

    fn close(&mut self, c: HuntCur, now: u64, alive: bool, killed: bool) {
        for t in [&mut self.win, &mut self.all] {
            t.hunts += 1;
            t.kills += u64::from(killed);
            t.struck += u64::from(c.struck);
            t.struck_kills += u64::from(c.struck && killed);
            t.dropped_alive += u64::from(alive && !killed);
            t.gone += u64::from(!alive && !killed);
            t.secs += now.saturating_sub(c.since) as f64 / f64::from(TICK_HZ);
            t.by_prey[c.prey][0] += 1;
            t.by_prey[c.prey][1] += u64::from(killed);
            for k in 0..6 {
                if c.classes[k] {
                    t.by_class[k][0] += 1;
                    t.by_class[k][1] += u64::from(killed);
                }
            }
        }
        if self.log {
            println!(
                "hunt_end,{now},{},{},{},chases={},s={:.1},struck={},killed={killed},alive={alive}",
                c.target,
                Founder::ALL[c.prey].name(),
                HUNT_CLASSES.iter().enumerate().filter(|(k, _)| c.classes[*k]).map(|(_, n)| *n).collect::<Vec<_>>().join("+"),
                c.chases,
                now.saturating_sub(c.since) as f64 / f64::from(TICK_HZ),
                c.struck,
            );
        }
    }

    /// One tick of the readout.
    fn tick(&mut self, sim: &Sim, params: &Params) {
        let fa = sim.fauna();
        let v = fa.view();
        let now = v.tick;
        let ring = f64::from(sim.world().config().width) * sim.world().config().voxel_m;
        let ck = v.ledger.pack_chase_kills;
        let killed_now = ck > self.chase_kills;
        self.chase_kills = ck;
        let merge = (HUNT_MERGE_S * f64::from(TICK_HZ)) as u64;
        let retarget = (RETARGET_S * f64::from(TICK_HZ)) as u64;
        // Dropped hunts not resumed in time close as given up.
        let stale: Vec<u64> = self
            .last_end
            .iter()
            .filter(|(p, (_, t, closed, _, _))| !closed && now.saturating_sub(*t) > merge && !self.cur.get(p).is_some_and(|c| c.target == self.last_end[p].0))
            .map(|(p, _)| *p)
            .collect();
        for p in stale {
            let (_, t, _, _, c) = self.last_end[&p];
            let alive = v.animal(c.target).is_some();
            self.close(c, t, alive, false);
            self.last_end.get_mut(&p).unwrap().2 = true; // closed
        }
        for rec in fa.packs().iter().filter(|r| r.lineage == VH) {
            let cur = self.cur.get(&rec.id).copied();
            if let Some(c) = cur {
                if rec.target != Some(c.target) {
                    let alive = v.animal(c.target).is_some();
                    self.cur.remove(&rec.id);
                    let killed = !alive && killed_now;
                    self.end(rec.id, c, now, alive, killed, true);
                } else {
                    let struck = v.animals.iter().any(|a| a.founder == Some(VH) && a.strike.is_some_and(|s| s.prey == c.target));
                    if struck {
                        self.cur.get_mut(&rec.id).unwrap().struck = true;
                    }
                    continue;
                }
            }
            let Some(t) = rec.target else { continue };
            let Some(o) = v.animal(t).copied() else { continue };
            self.win.chases += 1;
            self.all.chases += 1;
            let last = self.last_end.get(&rec.id).copied();
            if let Some((lt, tick, closed, was_kill, lc)) = last {
                if lt == t && !closed && now.saturating_sub(tick) <= merge {
                    self.win.resumes += 1;
                    self.all.resumes += 1;
                    self.cur.insert(rec.id, HuntCur { chases: lc.chases + 1, ..lc });
                    self.last_end.remove(&rec.id);
                    continue;
                }
                if !closed {
                    // The dropped hunt is over: a new target took its place.
                    let alive = v.animal(lc.target).is_some();
                    self.close(lc, tick, alive, false);
                }
                if !was_kill && now.saturating_sub(tick) <= retarget {
                    self.win.retargets += 1;
                    self.all.retargets += 1;
                }
                self.last_end.remove(&rec.id);
            }
            let f = o.founder.unwrap_or(Founder::Browser);
            let members: Vec<u64> = v.animals.iter().filter(|a| a.pack.is_some_and(|m| m.pack == rec.id)).map(|a| a.id).collect();
            let juvenile = o.body < params.fauna.founder(f).core.birth_body;
            let aware = members.iter().any(|&m| cubarium_voxel_fauna::vigil::aware_of(&params.fauna, fa.percept(o.id), m, now));
            let kin = v.animals.iter().any(|k| {
                k.id != o.id && k.founder == o.founder && {
                    let dx = (k.pose.x - o.pose.x + 0.5 * ring).rem_euclid(ring) - 0.5 * ring;
                    let dz = k.pose.z - o.pose.z;
                    (dx * dx + dz * dz).sqrt() <= ISOLATED_M
                }
            });
            let classes = [
                o.hunkered && !juvenile,
                !o.hunkered && !juvenile,
                juvenile,
                o.mobility.mode == cubarium_voxel_fauna::MobilityMode::Fly,
                aware,
                !kin,
            ];
            self.cur.insert(rec.id, HuntCur { target: t, prey: f.index(), since: now, struck: false, classes, chases: 1 });
        }
    }
}

/// **The glide probe** (PACK-TUNE): `hunter_study glide load=DIR minutes=M [max=N]` prints each
/// seedporter's mobility-mode changes (tick, id, from -> to, position, height, state, held
/// forward, fleeing, sprint stamina, the nearest Veilhound) up to `max` lines, then the counts.
fn glide_probe(args: &[String]) {
    let params = resolve(args);
    let minutes: u64 = arg(args, "minutes=").map_or(3, |m| m.parse().unwrap());
    let max: usize = arg(args, "max=").map_or(120, |m| m.parse().unwrap());
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    let (world, mut flora, mut fauna) = (s.world, s.flora, s.fauna);
    flora.replace_config(params.flora_for_world()).expect("plant config");
    fauna.replace_config(params.fauna.clone()).expect("animal config");
    cubarium_voxel_sim::saved::restore(&mut fauna, &[]).expect("heuristics");
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    let ring = f64::from(sim.world().config().width) * sim.world().config().voxel_m;
    let total = minutes * 60 * u64::from(TICK_HZ);
    let mut last: rustc_hash::FxHashMap<u64, String> = Default::default();
    let mut lines = 0usize;
    let mut changes: rustc_hash::FxHashMap<u64, u64> = Default::default();
    let g0 = sim.fauna().view().ledger.glides;
    for _ in 0..total {
        sim.step();
        let fa = sim.fauna();
        let v = fa.view();
        let vhs: Vec<_> = v.animals.iter().filter(|a| a.founder == Some(VH)).collect();
        for a in v.animals.iter().filter(|a| a.founder == Some(Founder::Seedporter)) {
            let m = format!("{:?}", a.mobility.mode);
            if last.get(&a.id).is_some_and(|l| *l == m) {
                continue;
            }
            let from = last.insert(a.id, m.clone()).unwrap_or_default();
            *changes.entry(a.id).or_default() += 1;
            if lines < max {
                lines += 1;
                let vg = fa.percept(a.id).map(|r| r.vigil).unwrap_or_default();
                let near = vhs
                    .iter()
                    .map(|h| {
                        let dx = (h.pose.x - a.pose.x + 0.5 * ring).rem_euclid(ring) - 0.5 * ring;
                        let dz = h.pose.z - a.pose.z;
                        ((dx * dx + dz * dz).sqrt(), format!("{:?}", h.state))
                    })
                    .min_by(|x, y| x.0.total_cmp(&y.0));
                println!(
                    "glide,{},{},{}->{},x={:.2},z={:.2},y={:.2},head={:.2},state={:?},fwd={:.2},turn={:.2},fleeing={},spent={:.2},winded={},vh={:?}",
                    v.tick, a.id, from, m, a.pose.x, a.pose.z, a.pose.y_m, a.pose.heading_rad, a.state,
                    a.founder_state.held.forward, a.founder_state.held.turn, vg.fleeing,
                    a.founder_state.sprint_spent, a.founder_state.winded, near
                );
            }
        }
    }
    println!("glides {} over {} min; mode changes by id {:?}", sim.fauna().view().ledger.glides - g0, minutes, changes);
}

fn budget_probe(args: &[String]) {
    use cubarium_voxel_fauna::State;
    let mut params = resolve(args);
    if arg(args, "vig=") == Some("off") {
        for f in params.fauna.founders.iter_mut() {
            f.vigilance.mode = cubarium_voxel_fauna::VigilMode::None;
        }
    }
    let minutes: u64 = arg(args, "minutes=").map_or(10, |m| m.parse().unwrap());
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    let (world, mut flora, mut fauna, minute0) = (s.world, s.flora, s.fauna, s.minute);
    flora.replace_config(params.flora_for_world()).expect("plant config");
    fauna.replace_config(params.fauna.clone()).expect("animal config");
    cubarium_voxel_sim::saved::restore(&mut fauna, &[]).expect("heuristics");
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    let ring = f64::from(sim.world().config().width) * sim.world().config().voxel_m;
    let stay = params.fauna.teachers.forage_stay_forward;
    let total = minutes * 60 * u64::from(TICK_HZ);
    const NAMES: [&str; 10] = ["asleep", "flee", "bolt", "orient", "scan", "feed_still", "feed_move", "walk", "still", "sated_any"];
    let mut t = [0u64; 10];
    let mut last_bite: rustc_hash::FxHashMap<u64, u64> = Default::default();
    let mut gaps = [0u64; 5]; // <=0.5 s, <=2 s, <=10 s, <=60 s, >60 s
    let mut bites = 0u64;
    let mut fl_start: rustc_hash::FxHashMap<u64, u64> = Default::default();
    let mut fl_lens: Vec<f64> = Vec::new();
    let mut fl_near = [0u64; 4]; // nearest VH while fleeing: asleep, resting, moving, none within 8 m
    let mut fl_still = 0u64;
    let b0 = sim.fauna().view().ledger.bites_by_founder[GRAZER.index()];
    for _ in 0..total {
        sim.step();
        let fa = sim.fauna();
        let v = fa.view();
        let now = v.tick;
        for a in v.animals.iter().filter(|a| a.founder == Some(GRAZER)) {
            let vg = fa.percept(a.id).map(|r| r.vigil).unwrap_or_default();
            let h = a.founder_state.held;
            let k = if a.state == State::Asleep {
                0
            } else if vg.fleeing {
                1
            } else if now < vg.bolt_until {
                2
            } else if now < vg.orient_until {
                3
            } else if vg.scanning(now) {
                4
            } else if h.feed > 0.5 && h.forward <= stay {
                5
            } else if h.feed > 0.5 {
                6
            } else if h.forward > 0.0 {
                7
            } else {
                8
            };
            t[k] += 1;
            if !a.eating {
                t[9] += 1;
            }
            if a.state == State::Cropping {
                bites += 1;
                if let Some(&l) = last_bite.get(&a.id) {
                    let g = (now - l) as f64 / f64::from(TICK_HZ);
                    let b = if g <= 0.5 { 0 } else if g <= 2.0 { 1 } else if g <= 10.0 { 2 } else if g <= 60.0 { 3 } else { 4 };
                    gaps[b] += 1;
                }
                last_bite.insert(a.id, now);
            }
            if vg.fleeing {
                fl_start.entry(a.id).or_insert(now);
                if a.speed_m_s < 0.05 {
                    fl_still += 1;
                }
                let near = v
                    .animals
                    .iter()
                    .filter(|o| o.founder == Some(VH))
                    .map(|o| {
                        let (dx, dz) = wdelta(o.pose.x, o.pose.z, a.pose.x, a.pose.z, ring);
                        ((dx * dx + dz * dz).sqrt(), o.state)
                    })
                    .min_by(|p, q| p.0.total_cmp(&q.0));
                fl_near[match near {
                    Some((d, _)) if d > 8.0 => 3,
                    Some((_, State::Asleep)) => 0,
                    Some((_, State::Resting)) => 1,
                    Some(_) => 2,
                    None => 3,
                }] += 1;
            } else if let Some(s0) = fl_start.remove(&a.id) {
                fl_lens.push((now - s0) as f64 / f64::from(TICK_HZ));
            }
        }
    }
    let open: Vec<f64> = fl_start.values().map(|&s0| (sim.fauna().view().tick - s0) as f64 / f64::from(TICK_HZ)).collect();
    let all: u64 = t[..9].iter().sum();
    let hours = all as f64 / f64::from(TICK_HZ) / 3600.0;
    let lb = sim.fauna().view().ledger.bites_by_founder[GRAZER.index()] - b0;
    let shares: Vec<String> = (0..10).map(|k| format!("{}={:.3}", NAMES[k], t[k] as f64 / all.max(1) as f64)).collect();
    println!("B,{},minute0={minute0},vig={},grazer_h={hours:.3},{}", dir.display(), arg(args, "vig=").unwrap_or("on"), shares.join(","));
    println!(
        "B,bites_state={bites},bites_ledger={lb},bites_per_gh={:.1},gaps<=0.5/2/10/60/>60={}/{}/{}/{}/{}",
        lb as f64 / hours.max(1e-9),
        gaps[0], gaps[1], gaps[2], gaps[3], gaps[4]
    );
    let mean = |x: &[f64]| if x.is_empty() { f64::NAN } else { x.iter().sum::<f64>() / x.len() as f64 };
    let max = |x: &[f64]| x.iter().copied().fold(0.0f64, f64::max);
    println!(
        "B,flights_ended={},mean_s={:.1},max_s={:.1},open={},open_mean_s={:.1},flee_still_share={:.3},nearest_vh_while_fleeing asleep/resting/moving/none8m={}/{}/{}/{}",
        fl_lens.len(), mean(&fl_lens), max(&fl_lens), open.len(), mean(&open),
        fl_still as f64 / t[1].max(1) as f64,
        fl_near[0], fl_near[1], fl_near[2], fl_near[3]
    );
}

// ------------------------------------------------------------------ CHURN: the re-target probe

/// **The churn probe** (CHURN): `hunter_study churn load=DIR minutes=M` runs a save and prints
/// every Veilhound chase end (`cend,`: why, the target, the member whose sampling dropped it,
/// the chase's seconds, the nearest awake adult's distance at the end and 5 s before, whether
/// it was closing, whether any adult came within the chase distance `2 × pack_radius_m`) and
/// every re-target (`rt,`: a new target within 30 s of one dropped alive: the old end's
/// reason and closing, the new score against the old target's as the taker sees it, the
/// taker, whether it was the member that dropped the old one, whether the new target is one
/// of the pack's last three), then `sumchurn,` shares.
fn churn_probe(args: &[String]) {
    use cubarium_voxel_fauna::{ChaseEnd, PackEvent};
    use std::collections::VecDeque;
    let params = resolve(args);
    let minutes: u64 = arg(args, "minutes=").map_or(10, |m| m.parse().unwrap());
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    let (world, mut flora, mut fauna, minute0) = (s.world, s.flora, s.fauna, s.minute);
    flora.replace_config(params.flora_for_world()).expect("plant config");
    fauna.replace_config(params.fauna.clone()).expect("animal config");
    cubarium_voxel_sim::saved::restore(&mut fauna, &[]).expect("heuristics");
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    let pk = params.fauna.founder(VH).pack.expect("the Veilhound has a pack table");
    let ring = f64::from(sim.world().config().width) * sim.world().config().voxel_m;
    let hz = u64::from(TICK_HZ);
    let retarget = 30 * hz;
    #[derive(Default, Clone)]
    struct Cur {
        target: u64,
        since: u64,
        taker: u64,
        hist: VecDeque<(u64, f64)>,
        engaged: bool,
        min_d: f64,
        d0: f64,
        strikes: u32,
        striking: bool,
        hunk_ticks: u64,
        contact_ticks: u64,
        pry_max: f64,
        opened: bool,
    }
    #[derive(Clone, Copy)]
    struct End {
        tick: u64,
        why: ChaseEnd,
        target: u64,
        by: u64,
        closing: bool,
        engaged: bool,
        alive: bool,
    }
    let mut cur: rustc_hash::FxHashMap<u64, Cur> = Default::default();
    let mut last_end: rustc_hash::FxHashMap<u64, End> = Default::default();
    let mut recent: rustc_hash::FxHashMap<u64, VecDeque<u64>> = Default::default();
    let mut ends: rustc_hash::FxHashMap<String, u64> = Default::default();
    let mut rt_by: rustc_hash::FxHashMap<String, u64> = Default::default();
    let (mut starts, mut rts, mut late_closing, mut late_engaged, mut late_n) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut prev_unseen, mut new_worse, mut new_close, mut new_better, mut pingpong, mut same_member, mut breeder_taker) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let near = |sim: &Sim, pack: u64, tid: u64| -> f64 {
        let v = sim.fauna().view();
        let Some(o) = v.animal(tid) else { return f64::NAN };
        v.animals
            .iter()
            .filter(|b| b.pack.is_some_and(|m| m.pack == pack && !m.pup) && b.state != cubarium_voxel_fauna::State::Asleep)
            .map(|b| {
                let (dx, dz) = wdelta(b.pose.x, b.pose.z, o.pose.x, o.pose.z, ring);
                (dx * dx + dz * dz).sqrt()
            })
            .fold(f64::INFINITY, f64::min)
    };
    let total = minutes * 60 * hz;
    for _ in 0..total {
        sim.step();
        let now = sim.fauna().view().tick;
        let events = sim.fauna_mut().take_pack_events();
        for e in events {
            match e {
                PackEvent::ChaseEnd { pack, why, target, by, .. } => {
                    let Some(c) = cur.remove(&pack) else { continue };
                    let alive = sim.fauna().view().animal(target).is_some();
                    let d_end = c.hist.back().map_or(f64::NAN, |h| h.1);
                    let d_5 = c.hist.iter().find(|h| now.saturating_sub(h.0) <= 5 * hz).map_or(f64::NAN, |h| h.1);
                    let closing = d_5 - d_end > 0.5;
                    let o = sim.fauna().view().animal(target).copied();
                    let founder = o.and_then(|o| o.founder).map_or("-", |f| f.name());
                    let juv = o.and_then(|o| o.founder.map(|f| o.body < params.fauna.founder(f).core.birth_body));
                    let at = sim.fauna().view().animals.iter().filter(|b| b.pack.is_some_and(|m| m.pack == pack && !m.pup)).filter(|b| {
                        o.is_some_and(|o| {
                            let (dx, dz) = wdelta(b.pose.x, b.pose.z, o.pose.x, o.pose.z, ring);
                            (dx * dx + dz * dz).sqrt() <= 1.5
                        })
                    }).count();
                    let dur = now.saturating_sub(c.since).max(1) as f64;
                    println!(
                        "cend,{now},pack={pack},why={why:?},target={target},{founder},juv={juv:?},by={by},taker={},s={:.1},d0={:.1},d5={d_5:.1},d_end={d_end:.1},min_d={:.1},closing={closing},engaged={},strikes={},hunk_share={:.2},contact_share={:.2},pry_max={:.2},opened={},members_within_1.5m={at},mode={:?}",
                        c.taker,
                        now.saturating_sub(c.since) as f64 / hz as f64,
                        c.d0,
                        c.min_d,
                        c.engaged,
                        c.strikes,
                        c.hunk_ticks as f64 / dur,
                        c.contact_ticks as f64 / dur,
                        c.pry_max,
                        c.opened,
                        o.map(|o| o.mobility.mode),
                    );
                    *ends.entry(format!("{why:?}")).or_default() += 1;
                    if why == ChaseEnd::Late {
                        late_n += 1;
                        late_closing += u64::from(closing);
                        late_engaged += u64::from(c.engaged);
                    }
                    last_end.insert(pack, End { tick: now, why, target, by, closing, engaged: c.engaged, alive });
                }
                PackEvent::ChaseStart { pack, taker, target, score, prev, prev_score, .. } => {
                    starts += 1;
                    let d0 = near(&sim, pack, target);
                    let r = recent.entry(pack).or_default();
                    let pp = r.contains(&target);
                    r.push_back(target);
                    if r.len() > 3 {
                        r.pop_front();
                    }
                    if let Some(le) = last_end.get(&pack).copied().filter(|le| le.alive && now.saturating_sub(le.tick) <= retarget) {
                        rts += 1;
                        *rt_by.entry(format!("{:?}{}", le.why, if le.why == ChaseEnd::Late { if le.closing { "+closing" } else if le.engaged { "+engaged" } else { "+never_engaged" } } else { "" })).or_default() += 1;
                        let ratio = prev_score.map(|p| if p > 0.0 { score / p } else { f64::INFINITY });
                        match ratio {
                            None => prev_unseen += 1,
                            Some(x) if x < 1.0 => new_worse += 1,
                            Some(x) if x < 1.25 => new_close += 1,
                            Some(_) => new_better += 1,
                        }
                        pingpong += u64::from(pp);
                        same_member += u64::from(le.by == taker);
                        let breeder = sim.fauna().pack(pack).and_then(|p| p.breeder) == Some(taker);
                        breeder_taker += u64::from(breeder);
                        println!(
                            "rt,{now},pack={pack},after_s={:.1},old={},{:?},closing={},engaged={},prev={prev:?},prev_is_old={},new={target},score={score:.4},prev_score={prev_score:?},ratio={ratio:?},taker={taker},dropper={},breeder_taker={breeder},pingpong={pp},d0={d0:.1}",
                            now.saturating_sub(le.tick) as f64 / hz as f64,
                            le.target,
                            le.why,
                            le.closing,
                            le.engaged,
                            prev == Some(le.target),
                            le.by,
                        );
                    }
                    cur.insert(pack, Cur { target, since: now, taker, hist: VecDeque::new(), engaged: d0 <= 2.0 * pk.pack_radius_m, min_d: d0, d0, ..Cur::default() });
                }
                PackEvent::FeastEnd { .. } => {}
            }
        }
        let packs: Vec<(u64, u64)> = cur.iter().map(|(&p, c)| (p, c.target)).collect();
        for (p, t) in packs {
            let d = near(&sim, p, t);
            let c = cur.get_mut(&p).unwrap();
            let v = sim.fauna().view();
            let striking = v.animals.iter().any(|a| a.founder == Some(VH) && a.strike.is_some_and(|s| s.prey == t));
            c.strikes += u32::from(striking && !c.striking);
            c.striking = striking;
            c.hunk_ticks += u64::from(v.animal(t).is_some_and(|o| o.hunkered));
            c.contact_ticks += u64::from(d <= 1.0);
            if let Some(p) = sim.fauna().percept(t) {
                c.pry_max = c.pry_max.max(p.vigil.pry);
                c.opened |= now < p.vigil.opened_until;
            }
            if d.is_finite() {
                c.hist.push_back((now, d));
                c.min_d = c.min_d.min(d);
                c.engaged |= d <= 2.0 * pk.pack_radius_m;
            }
            while c.hist.front().is_some_and(|h| now.saturating_sub(h.0) > 6 * hz) {
                c.hist.pop_front();
            }
        }
    }
    let f = |x: u64, n: u64| x as f64 / n.max(1) as f64;
    println!("sumchurn,minute0={minute0},minutes={minutes},starts={starts},retargets={rts},share={:.2},ends={ends:?}", f(rts, starts));
    println!("sumchurn_rt_by_old_end,{rt_by:?}");
    println!("sumchurn_late,n={late_n},closing={:.2},engaged={:.2}", f(late_closing, late_n), f(late_engaged, late_n));
    println!(
        "sumchurn_rt,prev_unseen={:.2},new_worse={:.2},new_within_25pct={:.2},new_better_25pct={:.2},pingpong={:.2},same_member_as_dropper={:.2},breeder_taker={:.2}",
        f(prev_unseen, rts),
        f(new_worse, rts),
        f(new_close, rts),
        f(new_better, rts),
        f(pingpong, rts),
        f(same_member, rts),
        f(breeder_taker, rts)
    );
}

/// **The approach probe** (STRIKE): each Veilhound controller sample with a member within 1 m
/// (centre to centre) of its pack's unhunkered target, why the strike capsule does or does not
/// take it — `in_capsule`, `striking`, `recovery`, `fed`, `band` (out of the jaw band), `behind`
/// (> 135° off), `side` (45–135°), `ahead_near` (ahead, its centre short of the jaw's reach),
/// `ahead_wide` — with the share at ≤ 0.4 m; the strikes begun on targets and the pack-seconds
/// in contact; and one `stuck,` line per chase that ends `Late` neither closing nor in contact,
/// with what the nearest member was doing.
fn approach_probe(args: &[String]) {
    use cubarium_voxel_fauna::{ChaseEnd, MobilityMode, PackEvent, State, StrikePhase};
    use std::collections::{BTreeMap, VecDeque};
    let params = resolve(args);
    let minutes: u64 = arg(args, "minutes=").map_or(10, |m| m.parse().unwrap());
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    let (world, mut flora, mut fauna, minute0) = (s.world, s.flora, s.fauna, s.minute);
    flora.replace_config(params.flora_for_world()).expect("plant config");
    fauna.replace_config(params.fauna.clone()).expect("animal config");
    cubarium_voxel_sim::saved::restore(&mut fauna, &[]).expect("heuristics");
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    let phys = *params.fauna.founder(VH);
    let h = phys.hunter.expect("the Veilhound hunts");
    let v = sim.world().config().voxel_m;
    let ring = f64::from(sim.world().config().width) * v;
    let hz = u64::from(TICK_HZ);
    let cadence = params.fauna.manifest(VH).cadence_ticks().max(1);
    let dist = |ax: f64, az: f64, bx: f64, bz: f64| {
        let (dx, dz) = wdelta(ax, az, bx, bz, ring);
        (dx * dx + dz * dz).sqrt()
    };
    let base = |a: &cubarium_voxel_fauna::Animal| -> f64 {
        if matches!(a.mobility.mode, MobilityMode::Perched | MobilityMode::Fly | MobilityMode::Climb | MobilityMode::Glide) && a.pose.y_m.is_finite() {
            a.pose.y_m
        } else {
            (f64::from(a.site.y) + 1.0) * v + a.wall_offset_m()
        }
    };
    let mut cls: BTreeMap<String, u64> = BTreeMap::new();
    let mut cls_prey: BTreeMap<String, u64> = BTreeMap::new();
    let (mut samples, mut inside, mut contact_ticks, mut strikes, mut starts, mut rts, mut kills) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut ends: BTreeMap<String, u64> = BTreeMap::new();
    let mut striking: rustc_hash::FxHashSet<u64> = Default::default();
    let mut hist: rustc_hash::FxHashMap<u64, VecDeque<(u64, f64, f64)>> = Default::default();
    let mut dhist: rustc_hash::FxHashMap<u64, VecDeque<(u64, f64)>> = Default::default();
    let mut last_end: rustc_hash::FxHashMap<u64, (u64, bool)> = Default::default();
    let moved = |hist: &rustc_hash::FxHashMap<u64, VecDeque<(u64, f64, f64)>>, id: u64, x: f64, z: f64| -> f64 {
        hist.get(&id).and_then(|q| q.front()).map_or(f64::NAN, |&(_, fx, fz)| {
            let (dx, dz) = wdelta(fx, fz, x, z, ring);
            (dx * dx + dz * dz).sqrt()
        })
    };
    let withheld0 = sim.fauna().view().ledger.hunt_strikes_withheld;
    let l0 = sim.fauna().view().ledger.clone();
    // Each Veilhound strike from its windup to its resolution: where the striker and the prey
    // were at the windup, the prey's mode and wound; the outcome by the ledger and the bodies.
    let mut open_strikes: rustc_hash::FxHashMap<u64, (u64, f64, f64, f64, f64, MobilityMode, f64)> = Default::default();
    let mut outcomes: BTreeMap<String, u64> = BTreeMap::new();
    let mut dodges_before = l0.dodges_by_founder[VH.index()];
    let mut glances_before: u64 = l0.vigil.glances.iter().sum();
    // Contact episodes (pack, target): from the nearest member within 1.5 m of the unhunkered
    // target until it is 2.5 m off or the target changes; whether a strike began, and when.
    let mut episode: rustc_hash::FxHashMap<u64, (u64, u64, Option<u64>)> = Default::default();
    let (mut eps, mut eps_struck, mut to_strike): (u64, u64, Vec<f64>) = (0, 0, Vec::new());
    let total = minutes * 60 * hz;
    for _ in 0..total {
        sim.step();
        let now = sim.fauna().view().tick;
        for e in sim.fauna_mut().take_pack_events() {
            match e {
                PackEvent::ChaseStart { pack, .. } => {
                    starts += 1;
                    if let Some(&(t, alive)) = last_end.get(&pack) {
                        if alive && now.saturating_sub(t) <= 30 * hz {
                            rts += 1;
                        }
                    }
                }
                PackEvent::ChaseEnd { pack, why, target, .. } => {
                    let fv = sim.fauna().view();
                    let alive = fv.animal(target).is_some();
                    last_end.insert(pack, (now, alive));
                    *ends.entry(format!("{why:?}")).or_default() += 1;
                    kills += u64::from(why == ChaseEnd::Killed);
                    if why != ChaseEnd::Late {
                        continue;
                    }
                    let Some(o) = fv.animal(target).copied() else { continue };
                    let q = dhist.get(&pack);
                    let d_end = q.and_then(|q| q.back()).map_or(f64::NAN, |x| x.1);
                    let d_5 = q.and_then(|q| q.front()).map_or(f64::NAN, |x| x.1);
                    if d_5 - d_end > 0.5 || d_end <= 1.0 {
                        continue;
                    }
                    // The nearest member, and what holds it.
                    let Some(b) = fv
                        .animals
                        .iter()
                        .filter(|b| b.pack.is_some_and(|m| m.pack == pack && !m.pup))
                        .min_by(|x, y| dist(x.pose.x, x.pose.z, o.pose.x, o.pose.z).total_cmp(&dist(y.pose.x, y.pose.z, o.pose.x, o.pose.z)))
                        .copied()
                    else {
                        continue;
                    };
                    let fb = b.founder_state.feedback;
                    let motor = if fb.attempted_equivalent > 1e-12 { fb.delivered_equivalent / fb.attempted_equivalent } else { f64::NAN };
                    let hungry = fv.animals.iter().filter(|x| x.pack.is_some_and(|m| m.pack == pack && !m.pup) && (x.eating || x.emergency) && x.state != State::Asleep).count();
                    println!(
                        "stuck,{now},pack={pack},target={target},{},mode={:?},hunk={},d_end={d_end:.2},dy={:.2},prey_moved5={:.2},member_moved5={:.2},member_state={:?},member_hungry={},hungry={hungry},motor={motor:.2},member_strike={}",
                        o.founder.map_or("-", |f| f.name()),
                        o.mobility.mode,
                        o.hunkered,
                        base(&o) - base(&b),
                        moved(&hist, o.id, o.pose.x, o.pose.z),
                        moved(&hist, b.id, b.pose.x, b.pose.z),
                        b.state,
                        b.eating || b.emergency,
                        b.strike.is_some(),
                    );
                }
                PackEvent::FeastEnd { .. } => {}
            }
        }
        let fauna = sim.fauna();
        let fv = fauna.view();
        {
            let dodges_now = fv.ledger.dodges_by_founder[VH.index()];
            let mut dodged = dodges_now - dodges_before;
            dodges_before = dodges_now;
            let glances_now: u64 = fv.ledger.vigil.glances.iter().sum();
            let mut glanced = glances_now - glances_before;
            glances_before = glances_now;
            for b in fv.animals.iter().filter(|b| b.founder == Some(VH)) {
                match b.strike {
                    Some(s) if s.phase == StrikePhase::Windup && !open_strikes.contains_key(&b.id) => {
                        if let Some(o) = fv.animal(s.prey) {
                            open_strikes.insert(b.id, (s.prey, b.pose.x, b.pose.z, o.pose.x, o.pose.z, o.mobility.mode, o.wound));
                        }
                    }
                    Some(s) if s.phase == StrikePhase::Recovery => {
                        if let Some((pid, bx, bz, ox, oz, mode0, w0)) = open_strikes.remove(&b.id) {
                            let o = fv.animal(pid);
                            let name = o.and_then(|o| o.founder).map_or("gone", |f| f.name());
                            let out = match o {
                                None => "hit(killed)".to_string(),
                                Some(o) if o.wound > w0 => "hit".to_string(),
                                Some(_) if dodged > 0 => {
                                    dodged -= 1;
                                    "dodge".to_string()
                                }
                                Some(_) if glanced > 0 => {
                                    glanced -= 1;
                                    "glance".to_string()
                                }
                                Some(o) => {
                                    let pm = dist(ox, oz, o.pose.x, o.pose.z);
                                    let lunge = dist(bx, bz, b.pose.x, b.pose.z);
                                    let why = if o.mobility.mode != mode0 {
                                        format!("mode {mode0:?}->{:?}", o.mobility.mode)
                                    } else if pm > 0.25 {
                                        "prey moved >0.25 m".to_string()
                                    } else if lunge < 0.5 * h.strike_reach_length_fraction * phys.body_at(b.body).length_m {
                                        "lunge blocked".to_string()
                                    } else {
                                        "other".to_string()
                                    };
                                    format!("miss({why})")
                                }
                            };
                            *outcomes.entry(format!("{name}:{out}")).or_default() += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
        // Histories: 5 s of positions for Veilhounds and targets, and each pack's nearest-member distance.
        let targets: Vec<(u64, u64)> = fv
            .animals
            .iter()
            .filter_map(|b| b.pack.filter(|m| !m.pup).map(|m| m.pack))
            .collect::<std::collections::BTreeSet<u64>>()
            .into_iter()
            .filter_map(|p| fauna.pack(p).and_then(|r| r.target).map(|t| (p, t)))
            .collect();
        for a in fv.animals.iter().filter(|a| a.founder == Some(VH) || targets.iter().any(|t| t.1 == a.id)) {
            let q = hist.entry(a.id).or_default();
            q.push_back((now, a.pose.x, a.pose.z));
            while q.front().is_some_and(|f| now.saturating_sub(f.0) > 5 * hz) {
                q.pop_front();
            }
        }
        for &(p, t) in &targets {
            let Some(o) = fv.animal(t).copied() else { continue };
            let near = fv
                .animals
                .iter()
                .filter(|b| b.pack.is_some_and(|m| m.pack == p && !m.pup) && b.state != State::Asleep)
                .map(|b| dist(b.pose.x, b.pose.z, o.pose.x, o.pose.z))
                .fold(f64::INFINITY, f64::min);
            if near.is_finite() {
                let q = dhist.entry(p).or_default();
                q.push_back((now, near));
                while q.front().is_some_and(|f| now.saturating_sub(f.0) > 5 * hz) {
                    q.pop_front();
                }
            }
            if near <= 1.0 && !o.hunkered {
                contact_ticks += 1;
            }
            let any_strike = fv.animals.iter().any(|b| b.pack.is_some_and(|m| m.pack == p) && b.strike.is_some_and(|s| s.prey == t && s.phase == StrikePhase::Windup));
            match episode.get_mut(&p) {
                Some(e) if e.0 == t && near <= 2.5 => {
                    if any_strike && e.2.is_none() {
                        e.2 = Some(now);
                    }
                }
                _ => {
                    if let Some(e) = episode.remove(&p) {
                        eps += 1;
                        if let Some(at) = e.2 {
                            eps_struck += 1;
                            to_strike.push(at.saturating_sub(e.1) as f64 / hz as f64);
                        }
                    }
                    if near <= 1.5 && !o.hunkered {
                        episode.insert(p, (t, now, any_strike.then_some(now)));
                    }
                }
            }
            // Strikes begun on the target.
            for b in fv.animals.iter().filter(|b| b.pack.is_some_and(|m| m.pack == p && !m.pup)) {
                let on = b.strike.is_some_and(|s| s.prey == t && s.phase == StrikePhase::Windup);
                if on && striking.insert(b.id) {
                    strikes += 1;
                } else if !on {
                    striking.remove(&b.id);
                }
                // The member's controller sample within 1 m of an unhunkered target (a strike
                // begun this tick passed the reach test there).
                let began_now = on && b.strike.is_some_and(|s| s.ticks_left == cubarium_voxel_fauna::Hunter::ticks(h.windup_s));
                if (b.age_ticks % cadence != 0 && !began_now) || o.hunkered || b.state == State::Asleep {
                    continue;
                }
                let d = dist(b.pose.x, b.pose.z, o.pose.x, o.pose.z);
                if d > 1.0 {
                    continue;
                }
                samples += 1;
                inside += u64::from(d <= 0.4);
                let g = phys.body_at(b.body);
                let pg = params.fauna.founder(o.founder.unwrap()).body_at(o.body);
                let jaw = h.jaw_forward_length_fraction * g.length_m;
                let reach = h.strike_reach_length_fraction * g.length_m;
                let rr = g.footprint_radius() + pg.footprint_radius();
                let (fx, fz) = b.pose.forward();
                let (dx, dz) = wdelta(b.pose.x, b.pose.z, o.pose.x, o.pose.z, ring);
                let along = dx * fx + dz * fz;
                let lateral = (dx * fz - dz * fx).abs();
                let bearing = lateral.atan2(along).to_degrees();
                let seg = if along < jaw { ((along - jaw).powi(2) + lateral * lateral).sqrt() } else if along > jaw + reach { ((along - jaw - reach).powi(2) + lateral * lateral).sqrt() } else { lateral };
                let s0 = base(&b);
                let p0 = base(&o);
                let in_band = p0 < s0 + h.jaw_band_height_fraction * g.height_m && s0 < p0 + pg.height_m;
                let c = if began_now {
                    "struck_now"
                } else if b.strike.is_some_and(|s| s.phase != StrikePhase::Recovery) {
                    "striking"
                } else if b.strike.is_some() {
                    "recovery"
                } else if !b.eating {
                    "fed"
                } else if seg <= rr && in_band {
                    "in_capsule"
                } else if !in_band {
                    "band"
                } else if bearing > 135.0 {
                    "behind"
                } else if bearing > 45.0 {
                    "side"
                } else if along < jaw {
                    "ahead_near"
                } else {
                    "ahead_wide"
                };
                *cls.entry(c.to_string()).or_default() += 1;
                let pn = o.founder.map_or("-", |f| f.name());
                let extra = match c {
                    "band" => format!("/dy{:+.2}", ((p0 - s0) / v).round() * v),
                    "in_capsule" => format!(
                        "/feed{}/prey_of{}",
                        u8::from(b.founder_state.held.feed > 0.0),
                        u8::from(cubarium_voxel_fauna::is_prey_of(&params.fauna, &b, &o))
                    ),
                    _ => String::new(),
                };
                *cls_prey.entry(format!("{c}/{pn}/{:?}{extra}", o.mobility.mode)).or_default() += 1;
            }
        }
    }
    let sec = contact_ticks as f64 / hz as f64;
    println!(
        "sumapproach,minute0={minute0},minutes={minutes},samples={samples},inside_0.4m={:.2},contact_pack_s={sec:.0},strikes={strikes},strikes_per_contact_s={:.3},starts={starts},retargets={rts},rt_share={:.2},kills={kills},ends={ends:?}",
        inside as f64 / samples.max(1) as f64,
        strikes as f64 / sec.max(1e-9),
        rts as f64 / starts.max(1) as f64,
    );
    let n = samples.max(1) as f64;
    println!("sumapproach_cls,{}", cls.iter().map(|(k, x)| format!("{k}={x}({:.2})", *x as f64 / n)).collect::<Vec<_>>().join(","));
    let l = sim.fauna().view().ledger.clone();
    to_strike.sort_by(f64::total_cmp);
    let q = |f: f64| to_strike.get(((to_strike.len() as f64 - 1.0) * f).round().max(0.0) as usize).copied().unwrap_or(f64::NAN);
    println!(
        "sumapproach_ledger,strikes_withheld={},vh_strikes={},hits={},dodges={},close_in={},step_offs={},pivots={},out_of_reach={},episodes={eps},struck={eps_struck},to_strike_s_p25/50/75={:.1}/{:.1}/{:.1}",
        l.hunt_strikes_withheld - withheld0,
        l.strikes_by_founder[VH.index()] - l0.strikes_by_founder[VH.index()],
        l.hits_by_founder[VH.index()] - l0.hits_by_founder[VH.index()],
        l.dodges_by_founder[VH.index()] - l0.dodges_by_founder[VH.index()],
        l.pack_close_in_samples - l0.pack_close_in_samples,
        l.pack_step_offs - l0.pack_step_offs,
        l.pack_pivots - l0.pack_pivots,
        l.pack_out_of_reach - l0.pack_out_of_reach,
        q(0.25),
        q(0.5),
        q(0.75),
    );
    println!("sumapproach_outcomes,{}", outcomes.iter().map(|(k, x)| format!("{k}={x}")).collect::<Vec<_>>().join(","));
    println!("sumapproach_prey,{}", cls_prey.iter().map(|(k, x)| format!("{k}={x}")).collect::<Vec<_>>().join(","));
}

/// **STRIKE's readout** (`strk,` per window, `sumstrk,` at the end), STRIKE-2's breakdown
/// (`strkcls,`/`sumstrkcls,`) and pry readout (`pry,`/`sumpry,`).
///
/// STRIKE's own numbers, kept as they were (`contact_s`, `strikes`, `per_contact_s`,
/// `fail_share`): pack-seconds with the pack's nearest awake member within 1 m (1.5 m) of its
/// unhunkered target, whatever that member was doing; every Veilhound strike begun on a pack
/// target, hunkered ones included; and the reach test's fail share over the member samples
/// within 1 m (1.5 m) of the unhunkered target, wanting food and not striking.
///
/// STRIKE-2's **corrected** measure of what STRIKE claimed (a member at its target, able to
/// strike, ends its approach in reach and facing it, and strikes): `ready_s`, the pack-seconds
/// with at least one member **ready** within 1 m (1.5 m) of an unarmoured target (unhunkered,
/// or hunkered and turned over), ready meaning awake, wanting food, no strike in hand (nor its
/// recovery) and the target in its jaw band; `ready_strikes`, the strikes begun on the target
/// while unarmoured; `per_ready_s` their ratio. STRIKE's denominator counted seconds whose only
/// near member was recovering or fed, and its numerator strikes on armoured bodies.
///
/// The breakdown: every member controller sample within 1 m of its pack's target, by what the
/// member was doing (`CLASSES`, first that applies), apart for an unarmoured, a turned-over
/// and an armoured (hunkered, not turned over) target.
#[derive(Default, Clone, Copy)]
struct Contact {
    ticks: [u64; 2],
    strikes: u64,
    samples: [u64; 2],
    fails: [u64; 2],
    ready_ticks: [u64; 2],
    ready_strikes: u64,
    cls: [[u64; CLASSES.len()]; 3],
    /// Armoured pack targets: ticks; ticks with a member in pry contact; pry member-ticks;
    /// ticks with enough in contact to open it; hungry member-ticks within `close_in_m` but
    /// outside pry contact; the least margin seen between pry contact and the stand-off.
    hunk_ticks: u64,
    worked_ticks: u64,
    pry_member_ticks: u64,
    enough_ticks: u64,
    near_out_ticks: u64,
    margin_min: f64,
    margin_seen: bool,
    /// Unarmoured `band` samples by the prey's base over the member's, in voxels (-4..=4).
    band_dy: [u64; 9],
    /// Armoured targets: hungry awake member-ticks by distance (pry contact, to `close_in_m`,
    /// to `pack_radius_m`, to twice it, beyond) and, within `close_in_m` outside pry contact,
    /// out of the jaw band / striking or recovering / motor refused / other.
    hunk_dist: [u64; 5],
    near_out_why: [u64; 4],
}

/// What a member within 1 m of its target was doing at a controller sample, first that applies:
/// it began a strike; it was in one; in its recovery; wanting no food; (armoured target) in pry
/// contact, or not; its capsule takes the target in the jaw band (a strike is due); the target
/// out of its jaw band; inside its outline (STRIKE's step-off); not facing it (the pivot);
/// facing it with its motor refused (blocked); facing it otherwise.
const CLASSES: [&str; 14] =
    ["struck", "striking", "recovery", "not_hungry", "pry", "hunk_out_of_pry", "in_capsule", "band", "step_off", "pivot", "ahead_blocked", "ahead_other", "in_capsule_feed0", "band_wedged"];
const ARMOUR: [&str; 3] = ["unhunkered", "opened", "armoured"];

impl Contact {
    fn add(&mut self, o: &Contact) {
        for k in 0..2 {
            self.ticks[k] += o.ticks[k];
            self.samples[k] += o.samples[k];
            self.fails[k] += o.fails[k];
            self.ready_ticks[k] += o.ready_ticks[k];
        }
        self.strikes += o.strikes;
        self.ready_strikes += o.ready_strikes;
        for a in 0..3 {
            for c in 0..CLASSES.len() {
                self.cls[a][c] += o.cls[a][c];
            }
        }
        self.hunk_ticks += o.hunk_ticks;
        self.worked_ticks += o.worked_ticks;
        self.pry_member_ticks += o.pry_member_ticks;
        self.enough_ticks += o.enough_ticks;
        self.near_out_ticks += o.near_out_ticks;
        for k in 0..9 {
            self.band_dy[k] += o.band_dy[k];
        }
        for k in 0..5 {
            self.hunk_dist[k] += o.hunk_dist[k];
        }
        for k in 0..4 {
            self.near_out_why[k] += o.near_out_why[k];
        }
        if o.margin_seen {
            self.margin_min = if self.margin_seen { self.margin_min.min(o.margin_min) } else { o.margin_min };
            self.margin_seen = true;
        }
    }

    fn line(&self, l0: &FaunaLedger, l: &FaunaLedger) -> String {
        let hz = TICK_HZ as f64;
        let s = |t: u64| t as f64 / hz;
        format!(
            "contact_s={:.0},contact15_s={:.0},strikes={},per_contact_s={:.3},per_contact15_s={:.3},samples={},fail_share={:.2},samples15={},fail_share15={:.2},close_in={},step_offs={},pivots={},out_of_reach={},ready_s={:.0},ready15_s={:.0},ready_strikes={},per_ready_s={:.3},per_ready15_s={:.3},withheld={}",
            s(self.ticks[0]),
            s(self.ticks[1]),
            self.strikes,
            self.strikes as f64 / s(self.ticks[0]).max(1e-9),
            self.strikes as f64 / s(self.ticks[1]).max(1e-9),
            self.samples[0],
            self.fails[0] as f64 / self.samples[0].max(1) as f64,
            self.samples[1],
            self.fails[1] as f64 / self.samples[1].max(1) as f64,
            l.pack_close_in_samples - l0.pack_close_in_samples,
            l.pack_step_offs - l0.pack_step_offs,
            l.pack_pivots - l0.pack_pivots,
            l.pack_out_of_reach - l0.pack_out_of_reach,
            s(self.ready_ticks[0]),
            s(self.ready_ticks[1]),
            self.ready_strikes,
            self.ready_strikes as f64 / s(self.ready_ticks[0]).max(1e-9),
            self.ready_strikes as f64 / s(self.ready_ticks[1]).max(1e-9),
            l.hunt_strikes_withheld - l0.hunt_strikes_withheld,
        )
    }

    /// The breakdown: `armour:class=n` for every class seen.
    fn cls_line(&self) -> String {
        let mut out = Vec::new();
        for (a, an) in ARMOUR.iter().enumerate() {
            let n: u64 = self.cls[a].iter().sum();
            out.push(format!("{an}_n={n}"));
            for (c, cn) in CLASSES.iter().enumerate() {
                if self.cls[a][c] > 0 {
                    out.push(format!("{an}:{cn}={}", self.cls[a][c]));
                }
            }
        }
        out.join(",")
    }

    fn pry_line(&self) -> String {
        let hz = TICK_HZ as f64;
        format!(
            "armoured_target_s={:.0},worked_s={:.0},pry_members_per_worked={:.2},enough_share_of_worked={:.2},near_out_member_s={:.0},margin_min_m={:.2},hungry_member_s_by_dist(pry/close_in/pack_r/2pack_r/beyond)={},near_out_why(band/strike/blocked/other)={},band_dy_vox(-4..4)={}",
            self.hunk_ticks as f64 / hz,
            self.worked_ticks as f64 / hz,
            self.pry_member_ticks as f64 / self.worked_ticks.max(1) as f64,
            self.enough_ticks as f64 / self.worked_ticks.max(1) as f64,
            self.near_out_ticks as f64 / hz,
            if self.margin_seen { self.margin_min } else { f64::NAN },
            self.hunk_dist.iter().map(|&t| format!("{:.0}", t as f64 / hz)).collect::<Vec<_>>().join("/"),
            self.near_out_why.iter().map(|&t| format!("{:.0}", t as f64 / hz)).collect::<Vec<_>>().join("/"),
            self.band_dy.iter().map(u64::to_string).collect::<Vec<_>>().join("/"),
        )
    }

    /// One tick's tally; `striking` holds the members in a windup already counted.
    fn tick(&mut self, sim: &Sim, params: &Params, striking: &mut rustc_hash::FxHashSet<u64>) {
        use cubarium_voxel_fauna::{MobilityMode, State, StrikePhase};
        let fauna = sim.fauna();
        let fv = fauna.view();
        let now = fv.tick;
        let v = sim.world().config().voxel_m;
        let ring = f64::from(sim.world().config().width) * v;
        let phys = params.fauna.founder(VH);
        let Some(h) = phys.hunter else { return };
        let Some(pk) = phys.pack else { return };
        let cadence = params.fauna.manifest(VH).cadence_ticks().max(1);
        let refused = params.fauna.teachers.motor_refused;
        let base = |a: &cubarium_voxel_fauna::Animal| -> f64 {
            if a.mobility.mode != MobilityMode::Ground && a.pose.y_m.is_finite() {
                a.pose.y_m
            } else {
                (f64::from(a.site.y) + 1.0) * v + a.wall_offset_m()
            }
        };
        for p in fauna.packs() {
            let Some(o) = p.target.and_then(|t| fv.animal(t)).copied() else { continue };
            let Some(of) = o.founder else { continue };
            let pf = params.fauna.founder(of);
            let pg = pf.body_at(o.body);
            let hk = pf.hunker;
            let opened = fauna.percept(o.id).is_some_and(|r| now < r.vigil.opened_until);
            let armour = if !o.hunkered { 0 } else if opened { 1 } else { 2 };
            let mut near = f64::INFINITY;
            let mut ready = [false; 2];
            let (mut pry_n, mut near_out) = (0u64, 0u64);
            for b in fv.animals.iter().filter(|b| b.pack.is_some_and(|m| m.pack == p.id && !m.pup)) {
                let on = b.strike.is_some_and(|s| s.prey == o.id && s.phase == StrikePhase::Windup);
                let began = on && striking.insert(b.id);
                if began {
                    self.strikes += 1;
                    self.ready_strikes += u64::from(armour < 2);
                } else if !on {
                    striking.remove(&b.id);
                }
                if b.state == State::Asleep {
                    continue;
                }
                let (dx, dz) = wdelta(b.pose.x, b.pose.z, o.pose.x, o.pose.z, ring);
                let d = (dx * dx + dz * dz).sqrt();
                near = near.min(d);
                let g = phys.body_at(b.body);
                let jaw = h.jaw_forward_length_fraction * g.length_m;
                let reach = h.strike_reach_length_fraction * g.length_m;
                let radii = g.footprint_radius() + pg.footprint_radius();
                let pry_d = 0.5 * (g.length_m + pg.length_m) + hk.pry_reach_m;
                let (s0, p0) = (base(b), base(&o));
                let in_band = p0 < s0 + h.jaw_band_height_fraction * g.height_m && s0 < p0 + pg.height_m;
                let fb = b.founder_state.feedback;
                let blocked = fb.attempted_equivalent > 1e-12 && fb.delivered_equivalent / fb.attempted_equivalent < refused;
                if armour == 2 && (b.eating || b.emergency) {
                    let k = if d <= pry_d {
                        0
                    } else if d <= pk.close_in_m {
                        1
                    } else if d <= pk.pack_radius_m {
                        2
                    } else if d <= 2.0 * pk.pack_radius_m {
                        3
                    } else {
                        4
                    };
                    self.hunk_dist[k] += 1;
                    if k == 1 {
                        let w = if !in_band {
                            0
                        } else if b.strike.is_some() {
                            1
                        } else if blocked {
                            2
                        } else {
                            3
                        };
                        self.near_out_why[w] += 1;
                    }
                }
                if armour == 2 {
                    let stand = jaw + pk.stand_off_reach_fraction * reach;
                    let m = pry_d - stand;
                    self.margin_min = if self.margin_seen { self.margin_min.min(m) } else { m };
                    self.margin_seen = true;
                    if d <= pry_d && b.attached.is_none() {
                        pry_n += 1;
                    } else if d <= pk.close_in_m && (b.eating || b.emergency) {
                        near_out += 1;
                    }
                }
                if d > 1.5 {
                    continue;
                }
                let (fx, fz) = b.pose.forward();
                let along = dx * fx + dz * fz;
                let lateral = (dx * fz - dz * fx).abs();
                let seg = if along < jaw {
                    ((along - jaw).powi(2) + lateral * lateral).sqrt()
                } else if along > jaw + reach {
                    ((along - jaw - reach).powi(2) + lateral * lateral).sqrt()
                } else {
                    lateral
                };
                let in_capsule = seg <= radii && in_band;
                if armour < 2 && b.eating && b.strike.is_none() && in_band {
                    for (k, lim) in [1.0, 1.5].into_iter().enumerate() {
                        ready[k] |= d <= lim;
                    }
                }
                let due = b.age_ticks % cadence == 0;
                // The breakdown, at controller samples within 1 m.
                if d <= 1.0 && (due || began) {
                    let diff = {
                        let mut x = dx.atan2(dz) - b.pose.heading_rad;
                        x = (x + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
                        x
                    };
                    let aligned = diff.abs() <= (radii / d.max(radii)).asin();
                    let c = if began {
                        0
                    } else if b.strike.is_some_and(|s| s.phase != StrikePhase::Recovery) {
                        1
                    } else if b.strike.is_some() {
                        2
                    } else if !b.eating {
                        3
                    } else if armour == 2 {
                        if d <= pry_d { 4 } else { 5 }
                    } else if in_capsule {
                        if b.founder_state.held.feed > 0.0 { 6 } else { 12 }
                    } else if !in_band {
                        if blocked { 13 } else { 7 }
                    } else if d < (jaw - radii).max(0.0) {
                        8
                    } else if !aligned {
                        9
                    } else if blocked {
                        10
                    } else {
                        11
                    };
                    self.cls[armour][c] += 1;
                    if (c == 7 || c == 13) && armour < 2 {
                        self.band_dy[(((p0 - s0) / v).round().clamp(-4.0, 4.0) + 4.0) as usize] += 1;
                    }
                }
                // STRIKE's own fail share, unchanged.
                if o.hunkered || !b.eating {
                    continue;
                }
                if began {
                    for (k, lim) in [1.0, 1.5].into_iter().enumerate() {
                        self.samples[k] += u64::from(d <= lim);
                    }
                    continue;
                }
                if !due || b.strike.is_some() {
                    continue;
                }
                for (k, lim) in [1.0, 1.5].into_iter().enumerate() {
                    if d <= lim {
                        self.samples[k] += 1;
                        self.fails[k] += u64::from(!in_capsule);
                    }
                }
            }
            if !o.hunkered {
                self.ticks[0] += u64::from(near <= 1.0);
                self.ticks[1] += u64::from(near <= 1.5);
            }
            for k in 0..2 {
                self.ready_ticks[k] += u64::from(ready[k]);
            }
            if armour == 2 {
                self.hunk_ticks += 1;
                self.near_out_ticks += near_out;
                if pry_n > 0 {
                    self.worked_ticks += 1;
                    self.pry_member_ticks += pry_n;
                    let ringed = fv
                        .animals
                        .iter()
                        .filter(|k| {
                            k.id != o.id && k.founder == Some(of) && k.hunkered && k.state != State::Asleep && {
                                let (dx, dz) = wdelta(o.pose.x, o.pose.z, k.pose.x, k.pose.z, ring);
                                (dx * dx + dz * dz).sqrt() <= hk.ring_m
                            }
                        })
                        .count() as u32;
                    if hk.members_to_open(o.body, pf.core.birth_body, ringed).is_some_and(|need| u64::from(need) <= pry_n) {
                        self.enough_ticks += 1;
                    }
                }
            }
        }
    }
}

/// **PIT's wedged bodies by lineage** (`wedge,` per window, `sumwedge,` at the end): as
/// [`Stuck`] (motor delivered under 5 % over runs of at least `STUCK_S`, at the controller
/// cadence) for every awake grounded walker of every lineage, the Veilhound's pups apart
/// (`chorister_pup`). Each item is `lineage=wedged share/awake seconds`.
#[derive(Default)]
struct Wedge {
    run: rustc_hash::FxHashMap<u64, u64>,
    /// Per lineage index, then the Veilhound's pups at 9: [awake samples, wedged samples, seconds a sample].
    win: [[f64; 3]; 10],
    all: [[f64; 3]; 10],
    /// EDGE: per lineage index (pups at 9), wedged samples by the layer that steered
    /// ([`Steer::ALL`] order), then at the strip's z end (the last slot).
    why: [[u64; STEERS + 1]; 10],
    /// EDGE: per lineage, [awake samples, refused samples of any run length]: a body that
    /// re-plans every few seconds and presses again is not wedged by the 30 s rule.
    refused: [[u64; 2]; 10],
}

const STEERS: usize = cubarium_voxel_fauna::Steer::ALL.len();

/// EDGE: whether a body stands within its radius and half a voxel of the strip's z end its
/// heading presses into (heading's z share over 0.3 toward that end).
fn at_z_end(sim: &Sim, params: &Params, b: &cubarium_voxel_fauna::Animal) -> bool {
    let Some(f) = b.founder else { return false };
    let c = sim.world().config();
    let v = c.voxel_m;
    let r = params.fauna.founder(f).body_at(b.body).footprint_radius();
    let depth_m = f64::from(c.depth) * v;
    let (_, fz) = b.pose.forward();
    (b.pose.z <= r + 0.5 * v && fz < -0.3) || (b.pose.z >= depth_m - r - 0.5 * v && fz > 0.3)
}

impl Wedge {
    fn tick(&mut self, sim: &Sim, params: &Params) {
        use cubarium_voxel_fauna::{MobilityMode, State};
        let fv = sim.fauna().view();
        for b in fv.animals.iter() {
            let Some(f) = b.founder else { continue };
            let cadence = params.fauna.manifest(f).cadence_ticks().max(1);
            if b.age_ticks % cadence != 0 || b.state == State::Asleep || b.mobility.mode != MobilityMode::Ground || b.attached.is_some() {
                continue;
            }
            let long = (STUCK_S * f64::from(TICK_HZ) / cadence as f64).ceil() as u64;
            let fb = b.founder_state.feedback;
            let refused = fb.attempted_equivalent > 1e-12 && fb.delivered_equivalent / fb.attempted_equivalent < 0.05;
            let r = self.run.entry(b.id).or_default();
            *r = if refused { *r + 1 } else { 0 };
            let add = if *r == long { long } else if *r > long { 1 } else { 0 };
            let k = if f == VH && b.pack.is_some_and(|m| m.pup) { 9 } else { f.index() };
            let s = cadence as f64 / f64::from(TICK_HZ);
            for t in [&mut self.win, &mut self.all] {
                t[k][0] += 1.0;
                t[k][1] += add as f64;
                t[k][2] = s;
            }
            self.refused[k][0] += 1;
            self.refused[k][1] += u64::from(refused && b.founder_state.held.forward > 0.0);
            if add > 0 {
                let src = sim.fauna().steer_of(b.id).unwrap_or(cubarium_voxel_fauna::Steer::Rest);
                let j = cubarium_voxel_fauna::Steer::ALL.iter().position(|&x| x == src).unwrap_or(0);
                self.why[k][j] += add;
                if at_z_end(sim, params, b) {
                    self.why[k][STEERS] += add;
                }
            }
        }
    }

    /// EDGE: per lineage with wedged samples, the share of them by the layer that steered
    /// and the share at the strip's z end.
    fn why_line(&self) -> String {
        let mut out = Vec::new();
        for (k, w) in self.why.iter().enumerate() {
            let n: u64 = w[..STEERS].iter().sum();
            if n == 0 {
                continue;
            }
            let name = if k == 9 { "chorister_pup".to_string() } else { Founder::ALL[k].name().to_string() };
            let parts: Vec<String> = cubarium_voxel_fauna::Steer::ALL
                .iter()
                .enumerate()
                .filter(|(j, _)| w[*j] > 0)
                .map(|(j, st)| format!("{}:{:.2}", st.name(), w[j] as f64 / n as f64))
                .collect();
            out.push(format!("{name}=zend:{:.2}/{}", w[STEERS] as f64 / n as f64, parts.join("/")));
        }
        for (k, r) in self.refused.iter().enumerate() {
            if r[0] > 0 {
                let name = if k == 9 { "chorister_pup".to_string() } else { Founder::ALL[k].name().to_string() };
                out.push(format!("pressed_any:{name}={:.3}", r[1] as f64 / r[0] as f64));
            }
        }
        out.join(",")
    }

    fn line(&self, all: bool) -> String {
        let t = if all { &self.all } else { &self.win };
        let mut out = Vec::new();
        for (k, c) in t.iter().enumerate() {
            if c[0] == 0.0 {
                continue;
            }
            let name = if k == 9 { "chorister_pup".to_string() } else { Founder::ALL[k].name().to_string() };
            out.push(format!("{name}={:.3}/{:.0}", c[1] / c[0], c[0] * c[2]));
        }
        out.join(",")
    }
}

/// **STRIKE-2's wedged members** (`stuck,` per window, `sumstuck,` at the end): awake adult
/// Veilhound controller samples whose motor delivered under 5 % of what it was asked over the
/// interval, the samples in runs of at least `STUCK_S` seconds (a body walking into the same
/// face that long is wedged, not turning a corner), the runs begun, the longest run.
const STUCK_S: f64 = 30.0;

#[derive(Default)]
struct Stuck {
    run: rustc_hash::FxHashMap<u64, u64>,
    /// Each member's last samples (tick, layer, strike phase, pack target distance, motor share),
    /// printed as a `wedge,` line when a run reaches `STUCK_S` and `log` is on.
    hist: rustc_hash::FxHashMap<u64, std::collections::VecDeque<(u64, u32, String, f64, f64)>>,
    log: bool,
    win: [u64; 4],
    all: [u64; 4],
}

impl Stuck {
    fn tick(&mut self, sim: &Sim, params: &Params) {
        use cubarium_voxel_fauna::State;
        let fv = sim.fauna().view();
        let cadence = params.fauna.manifest(VH).cadence_ticks().max(1);
        let long = (STUCK_S * f64::from(TICK_HZ) / cadence as f64).ceil() as u64;
        for b in fv.animals.iter().filter(|b| b.founder == Some(VH) && b.pack.is_some_and(|m| !m.pup)) {
            if b.age_ticks % cadence != 0 || b.state == State::Asleep {
                continue;
            }
            let fb = b.founder_state.feedback;
            let refused = fb.attempted_equivalent > 1e-12 && fb.delivered_equivalent / fb.attempted_equivalent < 0.05;
            let r = self.run.entry(b.id).or_default();
            *r = if refused { *r + 1 } else { 0 };
            let n = *r;
            if self.log {
                let ring = f64::from(sim.world().config().width) * sim.world().config().voxel_m;
                let d = b
                    .pack
                    .and_then(|m| sim.fauna().pack(m.pack))
                    .and_then(|p| p.target)
                    .and_then(|t| fv.animal(t))
                    .map_or(f64::NAN, |o| {
                        let (dx, dz) = wdelta(b.pose.x, b.pose.z, o.pose.x, o.pose.z, ring);
                        (dx * dx + dz * dz).sqrt()
                    });
                let share = if fb.attempted_equivalent > 1e-12 { fb.delivered_equivalent / fb.attempted_equivalent } else { f64::NAN };
                let q = self.hist.entry(b.id).or_default();
                q.push_back((fv.tick, b.site.y, b.strike.map_or("-".to_string(), |s| format!("{:?}", s.phase)), d, share));
                if q.len() as u64 > long + 40 {
                    q.pop_front();
                }
                if n == long {
                    let pre: Vec<String> = q.iter().take(q.len().saturating_sub(long as usize)).map(|(t, y, ph, d, sh)| format!("{t}:y{y}:{ph}:d{d:.1}:m{sh:.2}")).collect();
                    println!("wedge,{},id={},at=({:.2},{:.2},y{}),before={}", fv.tick, b.id, b.pose.x, b.pose.z, b.site.y, pre.join(" "));
                }
            }
            for t in [&mut self.win, &mut self.all] {
                t[0] += 1;
                if n == long {
                    t[1] += long;
                    t[2] += 1;
                } else if n > long {
                    t[1] += 1;
                }
                t[3] = t[3].max(n);
            }
        }
    }

    fn line(t: &[u64; 4], cadence: u64) -> String {
        let s = cadence as f64 / f64::from(TICK_HZ);
        format!("awake_member_s={:.0},wedged_s={:.0},wedged_share={:.3},wedges={},longest_s={:.0}", t[0] as f64 * s, t[1] as f64 * s, t[1] as f64 / t[0].max(1) as f64, t[2], t[3] as f64 * s)
    }
}

/// **STRIKE-2's chase-end causes** (`ends,` per window, `sumends,` at the end): every chase
/// end that leaves its target alive, by cause, and every re-target (a new chase within
/// `RETARGET_S` of one) by the cause of the end before it:
/// - `brokeoff:out_of_reach` (the ledger's out-of-reach count rose that tick);
/// - `brokeoff:herd_ring` (an armoured target its members at hand could open alone but not
///   with the awake kin within `ring_m`, as the prospect counts them);
/// - `brokeoff:too_few` (one they could not open even alone);
/// - `brokeoff:fewer_than_commit` (openable by the members counted at the commitment, not by
///   those at hand now);
/// - `brokeoff:low_other` (the kin, awareness or flight factors);
/// - `late:pry_short` / `late:armoured_untouched` / `late:unarmoured`, `switched`, `spent`,
///   `lost`, `trespass`.
///
/// A re-target is also marked `same_herd` (the new target of the old one's lineage within
/// `ring_m × 2` of where the old one stood) and `starving` (its taker in an emergency).
#[derive(Default)]
struct Ends {
    win: std::collections::BTreeMap<String, u64>,
    all: std::collections::BTreeMap<String, u64>,
    last: rustc_hash::FxHashMap<u64, (u64, String, Option<Founder>, f64, f64)>,
    commit_n: rustc_hash::FxHashMap<u64, u32>,
    oor: u64,
    log: bool,
}

impl Ends {
    fn bump(&mut self, k: String) {
        *self.win.entry(k.clone()).or_default() += 1;
        *self.all.entry(k).or_default() += 1;
    }

    fn line(m: &std::collections::BTreeMap<String, u64>) -> String {
        m.iter().map(|(k, x)| format!("{k}={x}")).collect::<Vec<_>>().join(",")
    }

    fn tick(&mut self, sim: &mut Sim, params: &Params) {
        use cubarium_voxel_fauna::{ChaseEnd, PackEvent, State};
        let events = sim.fauna_mut().take_pack_events();
        let fauna = sim.fauna();
        let fv = fauna.view();
        let now = fv.tick;
        let oor = fv.ledger.pack_out_of_reach;
        let oor_rose = oor > self.oor;
        self.oor = oor;
        let v = sim.world().config().voxel_m;
        let ring = f64::from(sim.world().config().width) * v;
        let phys = params.fauna.founder(VH);
        let Some(pk) = phys.pack else { return };
        let hz = u64::from(TICK_HZ);
        let dist = |ax: f64, az: f64, bx: f64, bz: f64| {
            let (dx, dz) = wdelta(ax, az, bx, bz, ring);
            (dx * dx + dz * dz).sqrt()
        };
        for e in events {
            match e {
                PackEvent::ChaseStart { pack, participants, taker, target, .. } => {
                    self.commit_n.insert(pack, participants);
                    let Some((t_end, cause, f0, x0, z0)) = self.last.remove(&pack) else { continue };
                    if now.saturating_sub(t_end) > (RETARGET_S * f64::from(TICK_HZ)) as u64 {
                        continue;
                    }
                    let o = fv.animal(target);
                    let same = o.is_some_and(|o| o.founder == f0 && f0.is_some_and(|f| dist(x0, z0, o.pose.x, o.pose.z) <= 2.0 * params.fauna.founder(f).hunker.ring_m.max(1.0)));
                    let starving = fv.animal(taker).is_some_and(|a| a.emergency);
                    self.bump(format!("rt:{cause}"));
                    if same {
                        self.bump("rt_flag:same_herd".into());
                    }
                    if starving {
                        self.bump("rt_flag:starving".into());
                    }
                    self.bump("rt:all".into());
                }
                PackEvent::ChaseEnd { pack, why, target, .. } => {
                    let Some(o) = fv.animal(target).copied() else {
                        self.last.remove(&pack);
                        continue;
                    };
                    let Some(of) = o.founder else { continue };
                    let pf = params.fauna.founder(of);
                    let hk = pf.hunker;
                    let opened = fauna.percept(o.id).is_some_and(|r| now < r.vigil.opened_until);
                    let members: Vec<_> = fv.animals.iter().filter(|b| b.pack.is_some_and(|m| m.pack == pack && !m.pup)).copied().collect();
                    let n = members
                        .iter()
                        .filter(|b| b.state != State::Asleep && (b.eating || b.emergency) && dist(b.pose.x, b.pose.z, o.pose.x, o.pose.z) <= 2.0 * pk.pack_radius_m)
                        .count() as u32;
                    let ringed = fv
                        .animals
                        .iter()
                        .filter(|k| k.id != o.id && k.founder == Some(of) && k.state != State::Asleep && dist(o.pose.x, o.pose.z, k.pose.x, k.pose.z) <= hk.ring_m)
                        .count() as u32;
                    let armoured = hk.enabled && !opened;
                    let adult = pf.core.birth_body;
                    let opens = |n: u32, r: u32| hk.members_to_open(o.body, adult, r).is_some_and(|need| need <= n);
                    let in_pry = members
                        .iter()
                        .filter(|b| {
                            b.state != State::Asleep
                                && dist(b.pose.x, b.pose.z, o.pose.x, o.pose.z) <= 0.5 * (phys.body_at(b.body).length_m + pf.body_at(o.body).length_m) + hk.pry_reach_m
                        })
                        .count();
                    let cause = match why {
                        ChaseEnd::BrokeOff if oor_rose => "brokeoff:out_of_reach",
                        ChaseEnd::BrokeOff if armoured && !opens(n, ringed) && opens(n, 0) => "brokeoff:herd_ring",
                        ChaseEnd::BrokeOff if armoured && !opens(n, ringed) && self.commit_n.get(&pack).is_some_and(|&c| opens(c, ringed)) => "brokeoff:fewer_than_commit",
                        ChaseEnd::BrokeOff if armoured && !opens(n, ringed) => "brokeoff:too_few",
                        ChaseEnd::BrokeOff => "brokeoff:low_other",
                        ChaseEnd::Late if o.hunkered && !opened && in_pry > 0 => "late:pry_short",
                        ChaseEnd::Late if o.hunkered && !opened => "late:armoured_untouched",
                        ChaseEnd::Late => "late:unarmoured",
                        ChaseEnd::Switched => "switched",
                        ChaseEnd::Spent => "spent",
                        ChaseEnd::Lost => "lost",
                        ChaseEnd::Trespass => "trespass",
                        _ => "other",
                    };
                    let juv = o.body < adult;
                    self.bump(format!("end:{cause}"));
                    self.bump(format!("end_prey:{cause}/{}{}", of.name(), if juv { "_juv" } else { "" }));
                    if self.log {
                        println!(
                            "cend,{now},pack={pack},cause={cause},target={target},{}{},hunk={},opened={opened},n={n},commit_n={:?},ringed={ringed},in_pry={in_pry},emergency={}",
                            of.name(),
                            if juv { "_juv" } else { "" },
                            o.hunkered,
                            self.commit_n.get(&pack),
                            members.iter().filter(|b| b.emergency).count(),
                        );
                    }
                    self.last.insert(pack, (now, cause.to_string(), Some(of), o.pose.x, o.pose.z));
                }
                PackEvent::FeastEnd { .. } => {}
            }
        }
        let _ = hz;
    }
}

/// `trace load=DIR [seconds=60] [every_s=1]` (STRIKE-2): each Veilhound pack member every
/// `every_s`: its target's distance and height over it (voxels), its held actions, its motor's
/// delivered share, its bearing off the target, hunger and state.
fn trace_probe(args: &[String]) {
    let params = resolve(args);
    let seconds: u64 = arg(args, "seconds=").map_or(60, |m| m.parse().unwrap());
    let every_s: u64 = arg(args, "every_s=").map_or(1, |m| m.parse().unwrap());
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    let (world, mut flora, mut fauna, _) = (s.world, s.flora, s.fauna, s.minute);
    flora.replace_config(params.flora_for_world()).expect("plant config");
    fauna.replace_config(params.fauna.clone()).expect("animal config");
    cubarium_voxel_sim::saved::restore(&mut fauna, &[]).expect("heuristics");
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    let v = sim.world().config().voxel_m;
    let ring = f64::from(sim.world().config().width) * v;
    let hz = u64::from(TICK_HZ);
    for t in 0..seconds * hz {
        sim.step();
        if t % (every_s * hz) != 0 {
            continue;
        }
        let fa = sim.fauna();
        let fv = fa.view();
        for r in fa.packs() {
            let o = r.target.and_then(|t| fv.animal(t)).copied();
            let ts = o.map(|o| {
                let (pry, open) = fa.percept(o.id).map_or((0.0, false), |p| (p.vigil.pry, fv.tick < p.vigil.opened_until));
                format!("{}{}@({:.2},{:.2},y{}) hunk={} open={open} pry={pry:.2} mode={:?} heading={:.2}", o.founder.map_or("-", |f| f.name()), o.id, o.pose.x, o.pose.z, o.site.y, o.hunkered, o.mobility.mode, o.pose.heading_rad)
            });
            println!("P,{}s,pack={},target={:?},feast={:?}", t / hz, r.id, ts, r.feast);
            for b in fv.animals.iter().filter(|b| b.pack.is_some_and(|m| m.pack == r.id && !m.pup)) {
                let (d, dy, bear) = o.map_or((f64::NAN, 0i64, f64::NAN), |o| {
                    let (dx, dz) = wdelta(b.pose.x, b.pose.z, o.pose.x, o.pose.z, ring);
                    let mut x = dx.atan2(dz) - b.pose.heading_rad;
                    x = (x + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
                    ((dx * dx + dz * dz).sqrt(), i64::from(o.site.y) - i64::from(b.site.y), x.to_degrees())
                });
                let fb = b.founder_state.feedback;
                let h = b.founder_state.held;
                println!(
                    "  M,{},d={d:.2},dy={dy},bear={bear:.0},held=({:.2},{:.2},{:.1}),motor={:.2},eat={},emerg={},state={:?},strike={:?},at=({:.2},{:.2},y{}),speed={:.2}",
                    b.id,
                    h.forward,
                    h.turn,
                    h.feed,
                    if fb.attempted_equivalent > 1e-12 { fb.delivered_equivalent / fb.attempted_equivalent } else { f64::NAN },
                    b.eating,
                    b.emergency,
                    b.state,
                    b.strike.map(|s| s.phase),
                    b.pose.x,
                    b.pose.z,
                    b.site.y,
                    b.speed_m_s,
                );
            }
        }
    }
}

/// **PIT's probe** (`hunter_study pit load=DIR/mM [minutes=30] [config=FILE] [log=1]`): every
/// grounded walker each tick, on or off its lineage's route graph (`RouteGraph::node_near`
/// within its own footprint radius, as `route_from` asks); each step off it printed as an
/// `enter,` line with how it came (layer change, mode, homing, strike, den distance), and per
/// lineage the off-graph share and the wedged share (motor delivered under 5 % over runs of
/// at least `STUCK_S`, sampled at the controller cadence). At the start each pack's den and
/// whether it is on the graph.
fn pit_probe(args: &[String]) {
    use cubarium_voxel_fauna::{MobilityMode, RouteGraph, State};
    let params = resolve(args);
    let minutes: u64 = arg(args, "minutes=").map_or(30, |m| m.parse().unwrap());
    let log = arg(args, "log=").is_some_and(|s| s == "1");
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    let (world, mut flora, mut fauna, minute0) = (s.world, s.flora, s.fauna, s.minute);
    flora.replace_config(params.flora_for_world()).expect("plant config");
    fauna.replace_config(params.fauna.clone()).expect("animal config");
    cubarium_voxel_sim::saved::restore(&mut fauna, &[]).expect("heuristics");
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    let v = sim.world().config().voxel_m;
    let ring = f64::from(sim.world().config().width) * v;
    let margin = params.fauna.founding.clearance_margin_m;
    let hz = u64::from(TICK_HZ);
    let mut graphs: rustc_hash::FxHashMap<usize, (u64, std::sync::Arc<RouteGraph>)> = Default::default();
    let graph_of = |sim: &Sim, graphs: &mut rustc_hash::FxHashMap<usize, (u64, std::sync::Arc<RouteGraph>)>, f: Founder, t: u64| {
        if let Some((b, g)) = graphs.get(&f.index()) {
            if t.saturating_sub(*b) < 60 * hz {
                return g.clone();
            }
        }
        let g = std::sync::Arc::new(RouteGraph::for_founder(&sim.world().view(), params.fauna.founder(f), margin));
        graphs.insert(f.index(), (t, g.clone()));
        g
    };
    // The nearest node within `r + 6` columns, any layer: its layer difference and distance.
    let nearest_any = |g: &RouteGraph, site: cubarium_voxel_flora::Site, x: f64, z: f64, r: u32| -> String {
        let mut best: Option<(f64, i64)> = None;
        for dy in -12i64..=12 {
            let y = i64::from(site.y) + dy;
            if y < 0 {
                continue;
            }
            if let Some(j) = g.node_near(cubarium_voxel_flora::Site { y: y as u32, ..site }, (x, z), r + 6) {
                let (cx, cz) = g.centre(j);
                let (ddx, ddz) = wdelta(x, z, cx, cz, ring);
                let d = (ddx * ddx + ddz * ddz).sqrt();
                if best.is_none_or(|(bd, bdy)| d + 0.05 * dy.unsigned_abs() as f64 * v < bd + 0.05 * bdy.unsigned_abs() as f64 * v) {
                    best = Some((d, i64::from(g.faces[j].y) - i64::from(site.y)));
                }
            }
        }
        best.map_or("none".into(), |(d, dy)| format!("d{d:.2}/dy{dy}"))
    };
    for f in Founder::ALL {
        let p = params.fauna.founder(f);
        let st = p.step_limits(v);
        let b = p.adult_body();
        println!("lineage,{},step_up={},step_down={},walls={},adult_r={:.3},headroom={},margin={margin}", f.name(), st.up, st.down, p.climbs_walls, b.footprint_radius(), b.headroom_voxels(v));
    }
    for r in sim.fauna().packs().to_vec() {
        let g = graph_of(&sim, &mut graphs, r.lineage, 0);
        let rr = (params.fauna.founder(r.lineage).adult_body().footprint_radius() / v).ceil() as u32;
        let (cx, cz) = cubarium_voxel_fauna::pack_centre(r.den, v);
        println!("den,pack={},den={:?},choice={:?},node={:?},near={:?},nearest={}", r.id, r.den, r.den_choice, g.face(r.den), g.node_near(r.den, (cx, cz), rr), nearest_any(&g, r.den, cx, cz, rr));
    }
    // id -> (on graph, site, pose xz, mode, homing ticks, state, strike, tick)
    #[derive(Clone, Copy)]
    struct Last {
        on: bool,
        site: cubarium_voxel_flora::Site,
        x: f64,
        z: f64,
        mode: MobilityMode,
        homing: u32,
        tick: u64,
    }
    let mut last: rustc_hash::FxHashMap<u64, Last> = Default::default();
    let mut hist: rustc_hash::FxHashMap<u64, std::collections::VecDeque<String>> = Default::default();
    // per lineage index: [samples, off samples, adult-VH samples, adult-VH off, entries, exits, born/start off]
    let mut tally = [[0u64; 7]; 9];
    // wedged per lineage: [awake cadence samples, wedged samples], runs per id
    let mut wedge = [[0u64; 2]; 9];
    // The Veilhound's wedged samples split: [adult awake, adult wedged, pup awake, pup wedged].
    let mut vh_wedge = [0u64; 4];
    let mut run: rustc_hash::FxHashMap<u64, u64> = Default::default();
    let total = minutes * 60 * hz;
    for t in 0..=total {
        if t > 0 {
            sim.step();
        }
        let fv = sim.fauna().view();
        let tick = fv.tick;
        let animals: Vec<cubarium_voxel_fauna::Animal> = fv.animals.iter().filter(|a| a.founder.is_some()).copied().collect();
        let dens: rustc_hash::FxHashMap<u64, cubarium_voxel_flora::Site> = sim.fauna().packs().iter().map(|p| (p.id, p.den)).collect();
        for a in &animals {
            let f = a.founder.unwrap();
            let k = f.index();
            let p = params.fauna.founder(f);
            // Wedged, at the controller cadence, awake bodies on the ground.
            let cadence = params.fauna.manifest(f).cadence_ticks().max(1);
            if a.age_ticks % cadence == 0 && a.state != State::Asleep && a.mobility.mode == MobilityMode::Ground && a.attached.is_none() {
                let long = (STUCK_S * f64::from(TICK_HZ) / cadence as f64).ceil() as u64;
                let fb = a.founder_state.feedback;
                let refused = fb.attempted_equivalent > 1e-12 && fb.delivered_equivalent / fb.attempted_equivalent < 0.05;
                let r = run.entry(a.id).or_default();
                *r = if refused { *r + 1 } else { 0 };
                wedge[k][0] += 1;
                let add = if *r == long { long } else if *r > long { 1 } else { 0 };
                wedge[k][1] += add;
                if f == Founder::Chorister {
                    let o = if a.pack.is_some_and(|m| m.pup) { 2 } else { 0 };
                    vh_wedge[o] += 1;
                    vh_wedge[o + 1] += add;
                }
            }
            if a.mobility.mode != MobilityMode::Ground || a.attached.is_some() {
                last.remove(&a.id);
                continue;
            }
            if t % 4 != 0 {
                continue;
            }
            let g = graph_of(&sim, &mut graphs, f, t);
            let geom = p.body_at(a.body);
            let r = (geom.footprint_radius() / v).ceil().max(0.0) as u32;
            let on = g.node_near(a.site, (a.pose.x, a.pose.z), r).is_some();
            let pup = a.pack.is_some_and(|m| m.pup);
            tally[k][0] += 1;
            if !on {
                tally[k][1] += 1;
            }
            if f == Founder::Chorister && !pup {
                tally[k][2] += 1;
                if !on {
                    tally[k][3] += 1;
                }
            }
            let den_d = a.pack.and_then(|m| dens.get(&m.pack)).map_or(f64::NAN, |d| {
                let (cx, cz) = cubarium_voxel_fauna::pack_centre(*d, v);
                let (dx, dz) = wdelta(a.pose.x, a.pose.z, cx, cz, ring);
                (dx * dx + dz * dz).sqrt()
            });
            let desc = format!("{}:y{}:({:.2},{:.2}):{:?}:h{}", tick, a.site.y, a.pose.x, a.pose.z, a.state, a.homing_ticks);
            let q = hist.entry(a.id).or_default();
            q.push_back(desc);
            if q.len() > 12 {
                q.pop_front();
            }
            match last.get(&a.id).copied() {
                None => {
                    if !on {
                        tally[k][6] += 1;
                        println!(
                            "first_off,{tick},{}{},pup={pup},age_s={},site={:?},den_d={den_d:.2},nearest={}",
                            f.name(), a.id, a.age_ticks / hz, a.site, nearest_any(&g, a.site, a.pose.x, a.pose.z, r)
                        );
                    }
                }
                Some(l) => {
                    if l.on && !on {
                        tally[k][4] += 1;
                        let (dx, dz) = wdelta(l.x, l.z, a.pose.x, a.pose.z, ring);
                        let hop = (dx * dx + dz * dz).sqrt();
                        let fb = a.founder_state.feedback;
                        println!(
                            "enter,{tick},{}{},pup={pup},age_s={},from={:?},to={:?},dy={},hop={hop:.2},dt={},mode0={:?},homing={}->{},state={:?},strike={:?},rest={},eating={},emerg={},den_d={den_d:.2},target={:?},hunt={:?},speed={:.2},motor={:.2},nearest={},body={:.3}",
                            f.name(), a.id, a.age_ticks / hz, l.site, a.site,
                            i64::from(a.site.y) - i64::from(l.site.y), tick - l.tick, l.mode, l.homing, a.homing_ticks, a.state,
                            a.strike.map(|s| s.phase), a.rest.is_some(), a.eating, a.emergency,
                            a.pack.and_then(|m| sim.fauna().pack(m.pack)).and_then(|p| p.target),
                            a.hunt.mode, a.speed_m_s,
                            if fb.attempted_equivalent > 1e-12 { fb.delivered_equivalent / fb.attempted_equivalent } else { f64::NAN },
                            nearest_any(&g, a.site, a.pose.x, a.pose.z, r), a.body,
                        );
                        if log {
                            println!("  hist,{}", hist[&a.id].iter().cloned().collect::<Vec<_>>().join(" "));
                        }
                    } else if !l.on && on {
                        tally[k][5] += 1;
                        println!("exit,{tick},{}{},to={:?}", f.name(), a.id, a.site);
                    }
                }
            }
            last.insert(a.id, Last { on, site: a.site, x: a.pose.x, z: a.pose.z, mode: a.mobility.mode, homing: a.homing_ticks, tick });
        }
        if t > 0 && t % (10 * 60 * hz) == 0 {
            for f in Founder::ALL {
                let k = f.index();
                if tally[k][0] == 0 && wedge[k][0] == 0 {
                    continue;
                }
                println!(
                    "win,{}m,{},off_share={:.3},vh_adult_off={:.3},entries={},exits={},first_off={},wedged_share={:.3}",
                    minute0 + t / (60 * hz), f.name(),
                    tally[k][1] as f64 / tally[k][0].max(1) as f64,
                    tally[k][3] as f64 / tally[k][2].max(1) as f64,
                    tally[k][4], tally[k][5], tally[k][6],
                    wedge[k][1] as f64 / wedge[k][0].max(1) as f64,
                );
            }
        }
    }
    for f in Founder::ALL {
        let k = f.index();
        if tally[k][0] == 0 && wedge[k][0] == 0 {
            continue;
        }
        println!(
            "sumpit,{},samples={},off_share={:.3},vh_adult_off={:.3},entries={},exits={},first_off={},wedged_share={:.3}",
            f.name(), tally[k][0], tally[k][1] as f64 / tally[k][0].max(1) as f64, tally[k][3] as f64 / tally[k][2].max(1) as f64,
            tally[k][4], tally[k][5], tally[k][6], wedge[k][1] as f64 / wedge[k][0].max(1) as f64,
        );
    }
    println!(
        "sumvh,adult_wedged_share={:.3},pup_wedged_share={:.3},adult_awake_samples={},pup_awake_samples={}",
        vh_wedge[1] as f64 / vh_wedge[0].max(1) as f64, vh_wedge[3] as f64 / vh_wedge[2].max(1) as f64, vh_wedge[0], vh_wedge[2]
    );
    // Who is off the graph at the end, and where.
    let fv = sim.fauna().view();
    for a in fv.animals.iter().filter(|a| a.founder.is_some() && a.mobility.mode == MobilityMode::Ground) {
        let f = a.founder.unwrap();
        let g = graph_of(&sim, &mut graphs, f, total + 60 * hz);
        let r = (params.fauna.founder(f).body_at(a.body).footprint_radius() / v).ceil() as u32;
        if g.node_near(a.site, (a.pose.x, a.pose.z), r).is_none() {
            if f == Founder::Chorister {
                pit_map(&sim, &g, a.site);
            }
            let geom = params.fauna.founder(f).body_at(a.body);
            let view = sim.world().view();
            let fb = a.founder_state.feedback;
            println!(
                "end_off,{}{},site={:?},pose=({:.3},{:.3}),pup={},still_s={},nearest={},water={:.3},wade={:.3},r={:.3},height={:.3},motor={:.2},held={:?}",
                f.name(), a.id, a.site, a.pose.x, a.pose.z, a.pack.is_some_and(|m| m.pup), a.still_ticks / hz, nearest_any(&g, a.site, a.pose.x, a.pose.z, r),
                view.water_depth_m(i64::from(a.site.x), a.site.y, a.site.z), params.fauna.founder(f).wade_depth_m(&geom), geom.footprint_radius(), geom.height_m,
                if fb.attempted_equivalent > 1e-12 { fb.delivered_equivalent / fb.attempted_equivalent } else { f64::NAN }, a.founder_state.held,
            );
        }
    }
}

/// The terrain around `site` (PIT): each column within 6 of it, the support face nearest
/// its layer (within 15) as the layer difference, `*` where that face is a node of `g`,
/// `#` with no face within 15; the body's own column bracketed.
fn pit_map(sim: &Sim, g: &cubarium_voxel_fauna::RouteGraph, site: cubarium_voxel_flora::Site) {
    let view = sim.world().view();
    let c = view.config;
    for dz in -6i64..=6 {
        let z = i64::from(site.z) + dz;
        if z < 0 || z >= i64::from(c.depth) {
            continue;
        }
        let mut row = String::new();
        for dx in -6i64..=6 {
            let x = (i64::from(site.x) + dx).rem_euclid(i64::from(c.width));
            let mut best: Option<i64> = None;
            for d in 0..=15i64 {
                for y in [i64::from(site.y) + d, i64::from(site.y) - d] {
                    if best.is_none() && y >= 0 && y < i64::from(c.height) && view.is_support(x, y as u32, z as u32) {
                        best = Some(y);
                    }
                }
            }
            let cell = match best {
                None => "  #".to_string(),
                Some(y) => {
                    let node = g.face(cubarium_voxel_flora::Site { x: x as u32, y: y as u32, z: z as u32 }).is_some();
                    let wet = view.water_depth_m(x, y as u32, z as u32) > 0.0;
                    format!("{:>2}{}", y - i64::from(site.y), if node { '*' } else if wet { '~' } else { ' ' })
                }
            };
            if dx == 0 && dz == 0 {
                row.push_str(&format!("[{}]", cell.trim()));
            } else {
                row.push_str(&cell);
            }
        }
        println!("  map,{row}");
    }
}

/// **EDGE's probe** (`hunter_study edge load=DIR/mM [minutes=30] [config=FILE] [lineages=a,b]`):
/// every awake grounded founder at its controller cadence, wedged as [`Wedge`] counts it
/// (motor delivered under 5 % over runs of at least `STUCK_S`). Each wedge's start is printed
/// as an `edgewedge,` line: where the body stands and how far from either z end, its heading,
/// the layer that steers it ([`cubarium_voxel_fauna::Steer`]), its state, eating latch,
/// flight, and the food it tracks. Per lineage at the end: the wedged share and its split by
/// steering layer and by z end. Also counted for every body (readout check): awake samples
/// in each mobility mode, and the refused samples by mode, so a climbing or perched body's
/// stillness shows whether it is in the wedged count or not.
fn edge_probe(args: &[String]) {
    use cubarium_voxel_fauna::{MobilityMode, State, Steer};
    let params = resolve(args);
    let minutes: u64 = arg(args, "minutes=").map_or(30, |m| m.parse().unwrap());
    let only: Option<Vec<String>> = arg(args, "lineages=").map(|s| s.split(',').map(str::to_string).collect());
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    let (world, mut flora, mut fauna, minute0) = (s.world, s.flora, s.fauna, s.minute);
    flora.replace_config(params.flora_for_world()).expect("plant config");
    fauna.replace_config(params.fauna.clone()).expect("animal config");
    cubarium_voxel_sim::saved::restore(&mut fauna, &[]).expect("heuristics");
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    let (width, depth, v) = (sim.world().config().width, sim.world().config().depth, sim.world().config().voxel_m);
    let depth_m = f64::from(depth) * v;
    let ring = f64::from(width) * v;
    println!("world,width={width},depth={depth},voxel_m={v},depth_m={depth_m:.2}");
    let hz = u64::from(TICK_HZ);
    let mut run: rustc_hash::FxHashMap<u64, u64> = Default::default();
    // per lineage: [awake ground samples, wedged samples]
    let mut tally = [[0u64; 2]; 9];
    let mut why = [[0u64; STEERS + 1]; 9];
    // per lineage: refused samples of any run length by steering layer
    let mut refused_by = [[0u64; STEERS]; 9];
    // per lineage, per mobility mode (debug name): [awake samples, refused samples]
    let mut modes: std::collections::BTreeMap<(usize, String), [u64; 2]> = Default::default();
    // EDGE-2: refused samples of the gliders and the loftstrider by where the body stands
    // (its pose clear or caught in solid; on its route graph, off it with or without a way
    // back on) and whether its motor would take a step in any of 16 headings, by layer; and by body.
    let mut pclass: std::collections::BTreeMap<(usize, String), u64> = Default::default();
    let mut pbody: std::collections::BTreeMap<(usize, u64), u64> = Default::default();
    let mut graphs: rustc_hash::FxHashMap<usize, (u64, cubarium_voxel_fauna::RouteGraph)> = Default::default();
    // EDGE-2: the seedporter's intake over the probe: its ledger's swallowed and alive ticks,
    // and its bodies' summed mass each second (body-seconds), adults (body at least 0.25) apart.
    let life0 = sim.fauna().view().ledger.clone();
    let mut mass_s = [0.0f64; 9];
    let mut adult_s = [0u64; 9];
    let mut all_s = [0u64; 9];
    let mut walk_s: std::collections::BTreeMap<(usize, String), u64> = Default::default();
    let total = minutes * 60 * hz;
    for t in 0..=total {
        if t > 0 {
            sim.step();
        }
        let fv = sim.fauna().view();
        for b in fv.animals.iter() {
            let Some(f) = b.founder else { continue };
            if only.as_ref().is_some_and(|o| !o.iter().any(|n| n == f.name())) {
                continue;
            }
            if t % hz == 0 {
                let k = f.index();
                mass_s[k] += b.body;
                all_s[k] += 1;
                adult_s[k] += u64::from(b.body >= 0.25);
                if f.name() == "seedporter" && b.state != State::Asleep {
                    let src = sim.fauna().steer_of(b.id).map_or("none", |s| s.name());
                    let key = format!("{:?}/{}/{}", b.mobility.mode, if b.eating { "eating" } else { "fed" }, src);
                    *walk_s.entry((k, key)).or_default() += 1;
                }
            }
            if t == 0 && f.name() == "seedporter" {
                let p = params.fauna.founder(f);
                let body = p.body_at(b.body);
                let (_, clear) = cubarium_voxel_fauna::motor_probe(&sim.world().view(), b.pose.x, b.pose.z, (0.0, 1.0), b.site.y, 0.0, &body, p.wade_depth_m(&body), p.step_limits(v));
                println!("edgeinit,{}{},mode={:?},state={:?},age_s={},body={:.3},r={:.3},at=({:.3},{:.3}) cols=({:.2},{:.2}),site={:?},pose_clear={clear}", f.name(), b.id, b.mobility.mode, b.state, b.age_ticks / hz, b.body, body.footprint_radius(), b.pose.x, b.pose.z, b.pose.x / v, b.pose.z / v, b.site);
            }
            let cadence = params.fauna.manifest(f).cadence_ticks().max(1);
            if b.age_ticks % cadence != 0 || b.state == State::Asleep {
                continue;
            }
            let fb = b.founder_state.feedback;
            let refused = fb.attempted_equivalent > 1e-12 && fb.delivered_equivalent / fb.attempted_equivalent < 0.05;
            let m = modes.entry((f.index(), format!("{:?}{}", b.mobility.mode, if b.attached.is_some() { "+attached" } else { "" }))).or_default();
            m[0] += 1;
            m[1] += u64::from(refused);
            if b.mobility.mode != MobilityMode::Ground || b.attached.is_some() {
                run.remove(&b.id);
                continue;
            }
            let k = f.index();
            let long = (STUCK_S * f64::from(TICK_HZ) / cadence as f64).ceil() as u64;
            let r = run.entry(b.id).or_default();
            *r = if refused { *r + 1 } else { 0 };
            let add = if *r == long { long } else if *r > long { 1 } else { 0 };
            tally[k][0] += 1;
            tally[k][1] += add;
            if refused {
                let src = sim.fauna().steer_of(b.id).unwrap_or(Steer::Rest);
                refused_by[k][Steer::ALL.iter().position(|&x| x == src).unwrap_or(0)] += 1;
                if matches!(f.name(), "seedporter" | "loftstrider") {
                    let p = params.fauna.founder(f);
                    let wv = sim.world().view();
                    let fresh = graphs.get(&k).is_some_and(|(t0, _)| fv.tick.saturating_sub(*t0) < 60 * hz);
                    if !fresh {
                        graphs.insert(k, (fv.tick, cubarium_voxel_fauna::RouteGraph::for_founder(&wv, p, params.fauna.founding.clearance_margin_m)));
                    }
                    let g = &graphs[&k].1;
                    let body = p.body_at(b.body);
                    let r = body.footprint_radius();
                    let own = (r / v).ceil() as u32;
                    let mut any = false;
                    let mut clear = true;
                    for q in 0..16 {
                        let h = f64::from(q) * std::f64::consts::TAU / 16.0;
                        let (ok, c) = cubarium_voxel_fauna::motor_probe(&wv, b.pose.x, b.pose.z, (h.sin(), h.cos()), b.site.y, 0.25 * r, &body, p.wade_depth_m(&body), p.step_limits(v));
                        any |= ok;
                        clear = c;
                    }
                    let place = if !clear {
                        "embedded"
                    } else if g.node_near(b.site, (b.pose.x, b.pose.z), own).is_some() {
                        "on_graph"
                    } else if g.way_on(&wv, b.site, r, body.headroom_voxels(v), p.step_limits(v), p.wade_depth_m(&body), own, 32).is_some() {
                        "off_graph_way"
                    } else {
                        "off_graph_pocket"
                    };
                    *pclass.entry((k, format!("{place}/{}/{}", if any { "free_dir" } else { "no_dir" }, src.name()))).or_default() += 1;
                    *pbody.entry((k, b.id)).or_default() += 1;
                }
                if *r == 1 && b.id % 3 == 0 && fv.tick % 7 == 0 {
                    let (fx, fz) = b.pose.forward();
                    println!("edgeshort,{},{}{},at=({:.2},{:.2},y{}),head=({fx:.2},{fz:.2}),steer={},held=({:.2},{:.2}),press={:?}", fv.tick, f.name(), b.id, b.pose.x, b.pose.z, b.site.y, src.name(), b.founder_state.held.forward, b.founder_state.held.turn, sim.fauna().press_of(b.id));
                    if arg(args, "detail=").is_some_and(|d| d == "1") {
                        let p = params.fauna.founder(f);
                        let g = cubarium_voxel_fauna::RouteGraph::for_founder(&sim.world().view(), p, params.fauna.founding.clearance_margin_m);
                        let body = p.body_at(b.body);
                        let r = body.footprint_radius();
                        let own = (r / v).ceil() as u32;
                        let near = g.node_near(b.site, (b.pose.x, b.pose.z), own).map(|j| g.faces[j]);
                        let wv = sim.world().view();
                        let way = g.way_on(&wv, b.site, r, body.headroom_voxels(v), p.step_limits(v), p.wade_depth_m(&body), own, 24);
                        println!("  detail,site={:?},near={near:?},way={way:?},wade={:.3},here_water={:.3}", b.site, p.wade_depth_m(&body), wv.water_depth_m(i64::from(b.site.x), b.site.y, b.site.z));
                        for z in b.site.z.saturating_sub(1)..=b.site.z + 2 {
                            let mut row = String::new();
                            for dx in -4i64..=3 {
                                let x = (i64::from(b.site.x) + dx).rem_euclid(i64::from(width));
                                let sup = wv.highest_support_in(x, z, 0, b.site.y + 8);
                                row.push_str(&format!(" {}:{}/{:.4}", x, sup.map_or("-".into(), |y| format!("{}", i64::from(y) - i64::from(b.site.y))), sup.map_or(0.0, |y| wv.water_depth_m(x, y, z))));
                            }
                            println!("  col,z{z}:{row}");
                        }
                    }
                }
            }
            if add == 0 {
                continue;
            }
            let src = sim.fauna().steer_of(b.id).unwrap_or(Steer::Rest);
            let j = Steer::ALL.iter().position(|&x| x == src).unwrap_or(0);
            why[k][j] += add;
            let zend = at_z_end(&sim, &params, b);
            if zend {
                why[k][STEERS] += add;
            }
            if *r == long || *r % (300 * hz / cadence) == 0 {
                let p = sim.fauna().percept(b.id);
                let food = p.map(|p| p.best[cubarium_voxel_fauna::perception::class::FOOD as usize]);
                let fleeing = p.is_some_and(|p| p.vigil.fleeing);
                let from_d = p.filter(|p| p.vigil.fleeing).map_or(f64::NAN, |p| {
                    let (dx, dz) = wdelta(b.pose.x, b.pose.z, p.vigil.from_x, p.vigil.from_z, ring);
                    (dx * dx + dz * dz).sqrt()
                });
                let threat = p.filter(|p| p.vigil.fleeing).and_then(|p| fv.animal(p.vigil.flee_id)).map_or("-".to_string(), |t| {
                    let (dx, dz) = wdelta(b.pose.x, b.pose.z, t.pose.x, t.pose.z, ring);
                    format!("{}{}:{:?}:d{:.2}:dy{}", t.founder.map_or("?", |f| f.name()), t.id, t.state, (dx * dx + dz * dz).sqrt(), i64::from(t.site.y) - i64::from(b.site.y))
                });
                // The nearest stand a glider might climb (foliage, crown at least its glider's
                // climb_min_stand_m), and its layer difference.
                let stand_s = params.fauna.founder(f).glider.map_or("-".to_string(), |g| {
                    let flv = sim.flora().view();
                    let mut best: Option<(f64, i64, f64, f64)> = None;
                    for st in flv.stands.iter().filter(|st| st.foliage > 0.0 && flv.config.species(st.species).crown_height_m_at(st.wood) >= g.climb_min_stand_m - 1e-9) {
                        let sx = (f64::from(st.site.x) + 0.5) * v;
                        let sz = (f64::from(st.site.z) + 0.5) * v;
                        let (dx, dz) = wdelta(b.pose.x, b.pose.z, sx, sz, ring);
                        let d = (dx * dx + dz * dz).sqrt();
                        if best.is_none_or(|q| d < q.0) {
                            best = Some((d, i64::from(st.site.y) - i64::from(b.site.y), dx, dz));
                        }
                    }
                    best.map_or("none".into(), |(d, dy, dx, dz)| format!("d{d:.2}/dy{dy}/({dx:.2},{dz:.2})"))
                });
                // The support faces in the column one voxel ahead, near its layer.
                let ahead = {
                    let (fx, fz) = b.pose.forward();
                    let ax = ((b.pose.x + fx * v) / v).floor() as i64;
                    let az = ((b.pose.z + fz * v) / v).floor() as i64;
                    let wv = sim.world().view();
                    let mut ys = Vec::new();
                    if az >= 0 && az < i64::from(depth) {
                        for y in b.site.y.saturating_sub(6)..b.site.y + 12 {
                            if wv.is_support(ax.rem_euclid(i64::from(width)), y, az as u32) {
                                ys.push(format!("{}", i64::from(y) - i64::from(b.site.y)));
                            }
                        }
                    }
                    // Solid profiles (y-3..y+4, low to high) of its own column and of the
                    // columns at a quarter, a half and one voxel ahead.
                    let prof = |px: f64, pz: f64| -> String {
                        let cx = ((px / v).floor() as i64).rem_euclid(i64::from(width));
                        let cz = (pz / v).floor() as i64;
                        if cz < 0 || cz >= i64::from(depth) {
                            return "out".into();
                        }
                        (b.site.y.saturating_sub(3)..=b.site.y + 4)
                            .map(|y| if wv.is_solid(cx, y, cz as u32) { '#' } else if wv.water_depth_m(cx, y, cz as u32) > 0.0 { 'w' } else { '.' })
                            .collect()
                    };
                    let r = params.fauna.founder(f).body_at(b.body).footprint_radius();
                    format!(
                        "{}:r{r:.3}:here{}:q{}:h{}:one{}",
                        ys.join("|"),
                        prof(b.pose.x, b.pose.z),
                        prof(b.pose.x + fx * (r + 0.25 * v), b.pose.z + fz * (r + 0.25 * v)),
                        prof(b.pose.x + fx * (r + 0.5 * v), b.pose.z + fz * (r + 0.5 * v)),
                        prof(b.pose.x + fx * v, b.pose.z + fz * v)
                    )
                };
                let food_s = food.filter(|t| t.known).map_or("-".to_string(), |t| {
                    let (dx, dz) = wdelta(b.pose.x, b.pose.z, t.x, t.z, ring);
                    format!("({:.2},{:.2})d{:.2}age{}", t.x, t.z, (dx * dx + dz * dz).sqrt(), (fv.tick.saturating_sub(t.tick)) / hz)
                });
                if *r == long && arg(args, "detail=").is_some_and(|d| d == "1") {
                    // EDGE-2: where a wedged body stands against its lineage's route graph,
                    // its step, its glide height, and the support layers round it.
                    let p = params.fauna.founder(f);
                    let g = cubarium_voxel_fauna::RouteGraph::for_founder(&sim.world().view(), p, params.fauna.founding.clearance_margin_m);
                    let body = p.body_at(b.body);
                    let r = body.footprint_radius();
                    let own = (r / v).ceil() as u32;
                    let near = g.node_near(b.site, (b.pose.x, b.pose.z), own).map(|j| g.faces[j]);
                    let wv = sim.world().view();
                    let way = g.way_on(&wv, b.site, r, body.headroom_voxels(v), p.step_limits(v), p.wade_depth_m(&body), own, 32);
                    println!(
                        "  wdetail,{}{},site={:?},near={near:?},way={way:?},step={:?},glide_min={:?},r={r:.3},attached={:?},mode={:?}",
                        f.name(), b.id, b.site, p.step_limits(v), p.glider.map(|gl| gl.glide_min_height_m), b.attached, b.mobility.mode
                    );
                    let mut dirs = String::new();
                    let mut clear_here = true;
                    for k in 0..16 {
                        let h = f64::from(k) * std::f64::consts::TAU / 16.0;
                        let (ok, clear) = cubarium_voxel_fauna::motor_probe(&wv, b.pose.x, b.pose.z, (h.sin(), h.cos()), b.site.y, 0.25 * r, &body, p.wade_depth_m(&body), p.step_limits(v));
                        clear_here = clear;
                        dirs.push(if ok { 'o' } else { 'x' });
                    }
                    println!("  wmotor,pose_clear={clear_here},dirs(N=+z clockwise via +x)={dirs},pose=({:.3},{:.3}) cols=({:.2},{:.2})", b.pose.x, b.pose.z, b.pose.x / v, b.pose.z / v);
                    for z in b.site.z.saturating_sub(4)..=(b.site.z + 4).min(depth - 1) {
                        let mut row = String::new();
                        for dx in -6i64..=6 {
                            let x = (i64::from(b.site.x) + dx).rem_euclid(i64::from(width));
                            let sup = wv.highest_support_in(x, z, 0, b.site.y + 12);
                            row.push_str(&format!(" {:>3}", sup.map_or("-".into(), |y| format!("{}", i64::from(y) - i64::from(b.site.y)))));
                        }
                        println!("  wcol,z{z}{}:{row}", if z == b.site.z { "*" } else { " " });
                    }
                }
                let (fx, fz) = b.pose.forward();
                let held = b.founder_state.held;
                println!(
                    "edgewedge,{},{}{},run_s={},at=({:.2},{:.2},y{}),z_lo={:.2},z_hi={:.2},head=({fx:.2},{fz:.2}),zend={zend},steer={},held=({:.2},{:.2}),state={:?},eating={},fleeing={fleeing},emerg={},food={food_s},body={:.3},from_d={from_d:.2},threat={threat},stand={stand_s},press={:?},ahead_dy={ahead}",
                    fv.tick, f.name(), b.id, *r * cadence / hz, b.pose.x, b.pose.z, b.site.y, b.pose.z, depth_m - b.pose.z,
                    src.name(), held.forward, held.turn, b.state, b.eating, b.emergency, b.body, sim.fauna().press_of(b.id)
                );
            }
        }
        if t > 0 && t % (10 * 60 * hz) == 0 {
            println!("win,{}m,{}", minute0 + t / (60 * hz), edge_line(&tally, &why));
        }
    }
    println!("sumedge,{}", edge_line(&tally, &why));
    for k in 0..9 {
        let n: u64 = refused_by[k].iter().sum();
        if n > 0 {
            let parts: Vec<String> = Steer::ALL.iter().enumerate().filter(|(j, _)| refused_by[k][*j] > 0).map(|(j, st)| format!("{}:{:.2}", st.name(), refused_by[k][j] as f64 / n as f64)).collect();
            println!("refusedby,{},samples={n},{}", Founder::ALL[k].name(), parts.join("/"));
        }
    }
    {
        let l = sim.fauna().view().ledger.clone();
        for f in Founder::ALL {
            let k = f.index();
            if all_s[k] == 0 {
                continue;
            }
            let h = (l.alive_ticks_by_founder[k] - life0.alive_ticks_by_founder[k]) as f64 / f64::from(TICK_HZ) / 3600.0;
            let eat = l.swallowed_by_founder[k] - life0.swallowed_by_founder[k];
            let mass_h = mass_s[k] / 3600.0;
            println!(
                "edgelife,{},alive_h={h:.3},intake_per_h={:.5},mean_body={:.3},intake_per_body_h={:.5},adult_share={:.2}",
                f.name(), if h > 0.0 { eat / h } else { f64::NAN }, mass_s[k] / all_s[k] as f64, if mass_h > 0.0 { eat / mass_h } else { f64::NAN }, adult_s[k] as f64 / all_s[k] as f64
            );
        }
        let n: u64 = walk_s.values().sum();
        for ((_, key), m) in &walk_s {
            println!("edgetime,seedporter,{key},{:.3}", *m as f64 / n.max(1) as f64);
        }
    }
    for ((k, class), n) in &pclass {
        println!("pressclass,{},{class},{n}", Founder::ALL[*k].name());
    }
    for ((k, id), n) in &pbody {
        println!("pressbody,{},{id},{n}", Founder::ALL[*k].name());
    }
    for ((k, mode), [n, refused]) in &modes {
        println!("mode,{},{mode},awake={n},refused={:.3}", Founder::ALL[*k].name(), *refused as f64 / (*n).max(1) as f64);
    }
}

fn edge_line(tally: &[[u64; 2]; 9], why: &[[u64; STEERS + 1]; 9]) -> String {
    let mut out = Vec::new();
    for k in 0..9 {
        if tally[k][0] == 0 {
            continue;
        }
        let n: u64 = why[k][..STEERS].iter().sum();
        let parts: Vec<String> = cubarium_voxel_fauna::Steer::ALL
            .iter()
            .enumerate()
            .filter(|(j, _)| why[k][*j] > 0)
            .map(|(j, st)| format!("{}:{:.2}", st.name(), why[k][j] as f64 / n.max(1) as f64))
            .collect();
        out.push(format!(
            "{}=wedged:{:.3}/zend:{:.2}/{}",
            Founder::ALL[k].name(),
            tally[k][1] as f64 / tally[k][0].max(1) as f64,
            why[k][STEERS] as f64 / n.max(1) as f64,
            parts.join("/")
        ));
    }
    out.join(",")
}

/// The pack tactic's return for body `id` (PIT-2's probe): the source line of `pack.rs`'s
/// return and the last point it walked at. The tactic does not record it; a probe build
/// patched to (the PIT-2 worker's `diag.py`, which tags each return) reads it here. Unpatched:
/// `(0, NaN, NaN)`.
fn pit2_diag(_id: u64) -> (u32, f64, f64) {
    (0, f64::NAN, f64::NAN)
}

/// **PIT-2's probe** (`hunter_study pit2 load=DIR/mM [minutes=30] [config=FILE] [trace=1]`): every
/// awake grounded Veilhound (adult or pup) at its controller cadence; while it is wedged (motor
/// under 5 % over runs of at least `STUCK_S`) each sample is classed by the tactic's return
/// ([`pit2_diag`]'s line), on or off the route graph, whether `way_on` finds a way within
/// `pit_search_m`, and what its disc touches; a `wedged,` line when a run reaches `STUCK_S`
/// and each minute after.
fn pit2_probe(args: &[String]) {
    use cubarium_voxel_fauna::{MobilityMode, RouteGraph, State};
    let params = resolve(args);
    let minutes: u64 = arg(args, "minutes=").map_or(30, |m| m.parse().unwrap());
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    let (world, mut flora, mut fauna, minute0) = (s.world, s.flora, s.fauna, s.minute);
    flora.replace_config(params.flora_for_world()).expect("plant config");
    fauna.replace_config(params.fauna.clone()).expect("animal config");
    cubarium_voxel_sim::saved::restore(&mut fauna, &[]).expect("heuristics");
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    let v = sim.world().config().voxel_m;
    let ring = f64::from(sim.world().config().width) * v;
    let margin = params.fauna.founding.clearance_margin_m;
    let hz = u64::from(TICK_HZ);
    let p = *params.fauna.founder(VH);
    let t = p.pack.unwrap();
    let cadence = params.fauna.manifest(VH).cadence_ticks().max(1);
    let long = (STUCK_S * f64::from(TICK_HZ) / cadence as f64).ceil() as u64;
    let mut graph: Option<(u64, std::sync::Arc<RouteGraph>)> = None;
    let mut run: rustc_hash::FxHashMap<u64, u64> = Default::default();
    let mut classes: std::collections::BTreeMap<String, u64> = Default::default();
    let mut awake = [0u64; 2];
    let mut wedged = [0u64; 2];
    let mut seen: rustc_hash::FxHashSet<u64> = Default::default();
    let trace_pups = arg(args, "trace=").is_some_and(|s| s == "1");
    let mut prev_on: rustc_hash::FxHashMap<u64, bool> = Default::default();
    let mut prev_pose: rustc_hash::FxHashMap<u64, (f64, f64, u32)> = Default::default();
    let total = minutes * 60 * hz;
    for tt in 0..=total {
        if tt > 0 {
            sim.step();
        }
        let fv = sim.fauna().view();
        let tick = fv.tick;
        let vh: Vec<cubarium_voxel_fauna::Animal> = fv.animals.iter().filter(|a| a.founder == Some(VH)).copied().collect();
        for a in &vh {
            if a.pack.is_some_and(|m| m.pup) && seen.insert(a.id) {
                let g = RouteGraph::for_founder(&sim.world().view(), &p, margin);
                let view = sim.world().view();
                let geom = p.body_at(a.body);
                let own_r = (geom.footprint_radius() / v).ceil() as u32;
                let on = g.node_near(a.site, (a.pose.x, a.pose.z), own_r).is_some();
                let way = g.way_on(&view, a.site, geom.footprint_radius(), geom.headroom_voxels(v), p.step_limits(v), p.wade_depth_m(&geom), own_r, 4 * (t.pit_search_m / v).ceil() as u32);
                let rec = a.pack.and_then(|m| sim.fauna().pack(m.pack)).copied();
                let den_d = rec.map_or(f64::NAN, |rc| { let (cx, cz) = cubarium_voxel_fauna::pack_centre(rc.den, v); let (dx, dz) = wdelta(a.pose.x, a.pose.z, cx, cz, ring); (dx * dx + dz * dz).sqrt() });
                let breeder = rec.and_then(|rc| rc.breeder).and_then(|b| fv.animal(b)).copied();
                let b_on = breeder.is_some_and(|b| g.node_near(b.site, (b.pose.x, b.pose.z), (p.body_at(b.body).footprint_radius() / v).ceil() as u32).is_some());
                println!("born,{tick},m{},vh{},age_s={},site={:?},on={on},way={},den_d={den_d:.2},breeder_on={b_on},breeder_state={:?},breeder_site={:?}", minute0 + tt / (60 * hz), a.id, a.age_ticks / hz, a.site, way.is_some(), breeder.map(|b| b.state), breeder.map(|b| b.site));
            }
            if trace_pups && a.pack.is_some_and(|m| m.pup) && a.age_ticks <= 240 * hz && (a.age_ticks % (2 * hz) == 0 || (a.age_ticks >= 12 * hz && a.age_ticks <= 26 * hz && a.age_ticks % cadence == 0)) {
                let g = RouteGraph::for_founder(&sim.world().view(), &p, margin);
                let own_r = (p.body_at(a.body).footprint_radius() / v).ceil() as u32;
                let on = g.node_near(a.site, (a.pose.x, a.pose.z), own_r).is_some();
                let (line, gx, gz) = pit2_diag(a.id);
                let fb = a.founder_state.feedback;
                if !on && prev_on.get(&a.id).copied().unwrap_or(true) {
                    if let Some(&(px, pz, py)) = prev_pose.get(&a.id) {
                        let view = sim.world().view();
                        let st = p.step_limits(v);
                        let (dx, dz) = wdelta(px, pz, gx, gz, ring);
                        let len = (dx * dx + dz * dz).sqrt();
                        let n = ((len / (0.5 * v)).ceil() as usize).max(1);
                        let mut y = py;
                        let mut last = (u32::MAX, u32::MAX);
                        let mut out = String::new();
                        for k in 1..=n {
                            let tt = k as f64 / n as f64;
                            let (x, z) = (px + dx * tt, pz + dz * tt);
                            let cx = ((x / v).floor() as i64).rem_euclid(i64::from(view.config.width));
                            let cz = (z / v).floor() as u32;
                            if (cx as u32, cz) == last { continue; }
                            last = (cx as u32, cz);
                            let body = view.nearest_support(cx, cz, y, st.down, st.up);
                            let nodes: Vec<String> = (0..view.config.height).filter(|&yy| g.face(cubarium_voxel_flora::Site { x: cx as u32, y: yy, z: cz }).is_some()).map(|yy| yy.to_string()).collect();
                            out.push_str(&format!(" ({cx},{cz}):body{}:nodes[{}]", body.map_or("X".into(), |b| b.to_string()), nodes.join("/")));
                            if let Some(b) = body { y = b; } else { break; }
                        }
                        println!("line,vh{},from=({px:.2},{pz:.2},y{py}),goal=({gx:.2},{gz:.2}):{out}", a.id);
                    }
                }
                {
                    let rec = a.pack.and_then(|m| sim.fauna().pack(m.pack)).copied();
                    if let Some(rc) = rec {
                        let (cx, cz) = cubarium_voxel_fauna::pack_centre(rc.den, v);
                        let from = g.node_near(a.site, (a.pose.x, a.pose.z), own_r);
                        let to = g.node_near(rc.den, (cx, cz), own_r);
                        let wl = from.is_some_and(|f| g.walk_line(f, (a.pose.x, a.pose.z), (cx, cz)));
                        let reach = match (from, to) { (Some(f), Some(t)) => g.distances(t, u32::MAX).contains_key(&f), _ => false };
                        println!("wh,{tick},vh{},from={:?},to={:?},walk_line={wl},in_field={reach}", a.id, from.map(|f| (g.faces[f], g.component_of(g.faces[f]))), to.map(|f| (g.faces[f], g.component_of(g.faces[f]))));
                    }
                }
                prev_on.insert(a.id, on);
                prev_pose.insert(a.id, (a.pose.x, a.pose.z, a.site.y));
                println!("pt,{tick},vh{},age_s={},site=({},{},{}),pose=({:.2},{:.2}),on={on},line={line},goal=({gx:.2},{gz:.2}),state={:?},motor={:.2},held=({:.2},{:.2})", a.id, a.age_ticks / hz, a.site.x, a.site.y, a.site.z, a.pose.x, a.pose.z, a.state, if fb.attempted_equivalent > 1e-12 { fb.delivered_equivalent / fb.attempted_equivalent } else { f64::NAN }, a.founder_state.held.forward, a.founder_state.held.turn);
            }
            if a.age_ticks % cadence != 0 || a.state == State::Asleep || a.mobility.mode != MobilityMode::Ground || a.attached.is_some() {
                continue;
            }
            let pup = a.pack.is_some_and(|m| m.pup);
            let fb = a.founder_state.feedback;
            let refused = fb.attempted_equivalent > 1e-12 && fb.delivered_equivalent / fb.attempted_equivalent < 0.05;
            let r = run.entry(a.id).or_default();
            *r = if refused { *r + 1 } else { 0 };
            let k = usize::from(pup);
            awake[k] += 1;
            if *r < long {
                continue;
            }
            wedged[k] += if *r == long { long } else { 1 };
            if graph.as_ref().is_none_or(|(b, _)| tick.saturating_sub(*b) >= 60 * hz) {
                graph = Some((tick, std::sync::Arc::new(RouteGraph::for_founder(&sim.world().view(), &p, margin))));
            }
            let g = graph.as_ref().unwrap().1.clone();
            let view = sim.world().view();
            let geom = p.body_at(a.body);
            let own_r = (geom.footprint_radius() / v).ceil() as u32;
            let on = g.node_near(a.site, (a.pose.x, a.pose.z), own_r).is_some();
            let reach = (t.pit_search_m / v).ceil() as u32;
            let way = if on { None } else { g.way_on(&view, a.site, geom.footprint_radius(), geom.headroom_voxels(v), p.step_limits(v), p.wade_depth_m(&geom), own_r, reach) };
            let way_far = if on || way.is_some() { None } else { g.way_on(&view, a.site, geom.footprint_radius(), geom.headroom_voxels(v), p.step_limits(v), p.wade_depth_m(&geom), own_r, 4 * reach) };
            let (line, gx, gz) = pit2_diag(a.id);
            let rec = a.pack.and_then(|m| sim.fauna().pack(m.pack)).copied();
            let (den_d, den_node, den_reach) = rec.map_or((f64::NAN, false, false), |rc| {
                let (cx, cz) = cubarium_voxel_fauna::pack_centre(rc.den, v);
                let (dx, dz) = wdelta(a.pose.x, a.pose.z, cx, cz, ring);
                let dn = g.node_near(rc.den, (cx, cz), own_r);
                let me = g.node_near(a.site, (a.pose.x, a.pose.z), own_r);
                let reach = match (dn, me) {
                    (Some(d), Some(m)) => g.distances(d, u32::MAX).contains_key(&m),
                    _ => false,
                };
                ((dx * dx + dz * dz).sqrt(), dn.is_some(), reach)
            });
            let target = rec.and_then(|rc| rc.target).and_then(|tid| fv.animal(tid)).copied();
            // What its disc touches: another body within the sum of radii + 5 cm, within 3 layers.
            let mut touch = "none".to_string();
            for o in fv.animals.iter().filter(|o| o.id != a.id && o.mobility.mode == MobilityMode::Ground) {
                let Some(of) = o.founder else { continue };
                let orr = params.fauna.founder(of).body_at(o.body).footprint_radius();
                let (dx, dz) = wdelta(a.pose.x, a.pose.z, o.pose.x, o.pose.z, ring);
                let d = (dx * dx + dz * dz).sqrt();
                if d <= geom.footprint_radius() + orr + 0.05 && (i64::from(o.site.y) - i64::from(a.site.y)).abs() <= 3 {
                    let rel = if Some(o.id) == target.map(|t| t.id) { "target" } else if o.pack.is_some_and(|m| Some(m.pack) == a.pack.map(|m| m.pack)) { "packmate" } else { of.name() };
                    touch = rel.to_string();
                    break;
                }
            }
            // The column ahead of its disc: support layer difference.
            let (fx, fz) = a.pose.forward();
            let ax = ((a.pose.x + fx * (geom.footprint_radius() + v)) / v).floor() as i64;
            let az = ((a.pose.z + fz * (geom.footprint_radius() + v)) / v).floor() as i64;
            let ahead = if az < 0 || az >= i64::from(view.config.depth) { "edge".to_string() } else {
                let ax = ax.rem_euclid(i64::from(view.config.width));
                view.nearest_support(ax, az as u32, a.site.y, 15, 15).map_or("none".into(), |y| format!("{}", i64::from(y) - i64::from(a.site.y)))
            };
            let wet = {
                let ax = ax.rem_euclid(i64::from(view.config.width));
                let az = az.clamp(0, i64::from(view.config.depth) - 1) as u32;
                let ya = view.nearest_support(ax, az, a.site.y, 15, 15).unwrap_or(a.site.y);
                format!("{:.3}/{:.3}/wade{:.3}", view.water_depth_m(i64::from(a.site.x), a.site.y, a.site.z), view.water_depth_m(ax, ya, az), p.wade_depth_m(&geom))
            };
            let goal = if gx.is_finite() {
                let (dx, dz) = wdelta(a.pose.x, a.pose.z, gx, gz, ring);
                format!("{:.2}", (dx * dx + dz * dz).sqrt())
            } else { "-".into() };
            let cls = format!("{},line={line},{},way={},touch={touch},ahead={ahead}", if pup { "pup" } else { "adult" }, if on { "on" } else { "off" }, if on { "-" } else if way.is_some() { "found" } else if way_far.is_some() { "beyond_reach" } else { "none" });
            *classes.entry(cls.clone()).or_default() += 1;
            if *r == long || (*r - long) % (60 * hz / cadence) == 0 {
                let held = a.founder_state.held;
                println!(
                    "wedged,{tick},m{},{}{},{cls},wet={wet},run_s={},site={:?},pose=({:.2},{:.2}),head={:.2},goal_d={goal},den_d={den_d:.2},den_node={den_node},den_reach={den_reach},homing={},eating={},emerg={},target={:?},target_d={},held=({:.2},{:.2},{:.2}),age_s={},body={:.3}",
                    minute0 + tt / (60 * hz), "vh", a.id, *r * cadence / hz, a.site, a.pose.x, a.pose.z, a.pose.heading_rad,
                    a.homing_ticks, a.eating, a.emergency, target.map(|o| o.id),
                    target.map_or("-".into(), |o| { let (dx, dz) = wdelta(a.pose.x, a.pose.z, o.pose.x, o.pose.z, ring); format!("{:.2}/dy{}", (dx * dx + dz * dz).sqrt(), i64::from(o.site.y) - i64::from(a.site.y)) }),
                    held.forward, held.turn, held.feed, a.age_ticks / hz, a.body,
                );
                if *r == long {
                    pit_map(&sim, &g, a.site);
                }
            }
        }
    }
    println!("sum2,adult_wedged={:.3},pup_wedged={:.3},adult_awake={},pup_awake={}", wedged[0] as f64 / awake[0].max(1) as f64, wedged[1] as f64 / awake[1].max(1) as f64, awake[0], awake[1]);
    let n: u64 = classes.values().sum();
    let mut v: Vec<_> = classes.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    for (c, k) in v {
        println!("class,{c},{k},{:.3}", k as f64 / n.max(1) as f64);
    }
}
