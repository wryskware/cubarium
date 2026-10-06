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
        "n={},adults={},pups={pups},fullness={fullness:.3},emergency={:.3},hungry={:.3},strikes={},hits={},kills={all_kills},grazer_kills={},chases={},chase_kills={},chases_by_sense={},draws_by_sense={},draw_chases={},hungry_samples={hungry},draw_lead={lead},draw_follow={follow},draw_share={:.3},vh_hours={:.3},funnel_grazer={},funnel_all={}",
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
    format!(
        "bw_hours={:.3},nectar_per_bwh={:.5},glowcap_visits={},glowcap_failed={},glowcap_left={},glowcap_sipped={},flower_visits={},flower_failed={},bellwings={n},uv_expect={:.3},cyan_expect={:.3}",
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
    let mut hunts = Hunts { log: arg(args, "hunt_log=") == Some("1"), chase_kills: sim.fauna().view().ledger.pack_chase_kills, ..Hunts::default() };
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
            /*}HUNK*/
            println!("gait,{arm},{minute},{}", gait_line(&lw, &l));
            println!("hunt,{arm},{minute},{}", hunts.win.line());
            /*PT{*/
            println!("ptune,{arm},{minute},break_offs={},declined={}", l.pack_break_offs - lw.pack_break_offs, l.pack_declined - lw.pack_declined);
            /*}PT*/
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
    /*}HUNK*/
    println!("sumgait,{arm},{},{}", minute0, gait_line(&l0, &l));
    println!("sumhunt,{arm},{},{}", minute0, hunts.all.line());
    /*PT{*/
    println!("sumptune,{arm},{},break_offs={},declined={}", minute0, l.pack_break_offs - l0.pack_break_offs, l.pack_declined - l0.pack_declined);
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
