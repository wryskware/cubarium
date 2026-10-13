//! **Does a top-layer variant change a lanternjaw's window at all?** (LJ-TWIN) A
//! measurement tool, not a training mode. Live starts are founded as the training worlds
//! are (the `small` training preset, `--top-up lanternjaw=8:0.3`, the centre on every
//! lanternjaw), warmed up on the centre and captured; each start then runs as matched
//! copies of one window ([`matched::run_assigned`] on a saved start):
//!
//! - **all-sign**: every lanternjaw on `+ε_p` in one copy and `−ε_p` in the other;
//! - **focal**: one lanternjaw on `±ε_p`, every other body on the centre;
//! - **null**: every lanternjaw on candidate 0 in both copies (the determinism floor).
//!
//! ```text
//! lj_twin centre=CENTRE.json out=DIR [worlds=8] [starts=1] [warmup_min=20] [gap_min=10]
//!         [window_min=10] [sigmas=0.02,0.05,0.1] [pairs=8] [modes=all,focal]
//!         [threads=16] [world_seed=3001] [train_seed=20260924] [generation=0]
//!         [top_up=lanternjaw=8:0.3] [water_every=4] [info=1: print the centre's size, run nothing]
//! ```
//!
//! One JSON line per matched pair in `DIR/pairs.jsonl` (per lanternjaw: score, intake,
//! swallowed, intent shares and switches in each twin, and when the twins first differ in
//! proposal, held intent and pose); `scripts/brain-pilot/twin_signal.py DIR` reads it.
//! Every number here is a probe argument, not a world knob.

use std::collections::BTreeMap;

use cubarium_search::es::voxel::kin::matched::{self, Scene, SceneCopy, SimStart};
use cubarium_search::es::voxel::kin::{BrainStage, KinConfig};
use cubarium_search::es::voxel::live::ecology::EcologyParams;
use cubarium_search::es::voxel::live::variants::VariantTable;
use cubarium_search::es::voxel::live::{targets, window, world};
use cubarium_search::es::voxel::two_layer::TwoLayerPolicyFile;
use cubarium_search::es::voxel::EpisodeDriver;
use cubarium_voxel_fauna::{Founder, Subject};
use cubarium_voxel_sim::Sim;
use serde::Serialize;

const LJ: Founder = Founder::Lanternjaw;

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")))
}

/// A captured live start; every copy loads it and puts the lanternjaws on the centre.
struct LiveStart {
    start: SimStart,
    driver: EpisodeDriver,
    ticks: u64,
    water_every: u32,
}

struct LiveCopy {
    sim: Sim,
    left: u64,
}

impl SceneCopy for LiveCopy {
    fn sim(&self) -> &Sim {
        &self.sim
    }
    fn sim_mut(&mut self) -> &mut Sim {
        &mut self.sim
    }
    fn step(&mut self) -> bool {
        if self.left == 0 {
            return false;
        }
        self.sim.step();
        self.left -= 1;
        true
    }
}

impl Scene for LiveStart {
    type Copy = LiveCopy;
    fn open(&self) -> Result<LiveCopy, String> {
        let mut sim = self.start.open(1, self.water_every)?;
        cubarium_search::es::voxel::live::install_centres(sim.fauna_mut(), &[(LJ, self.driver.clone())]);
        Ok(LiveCopy { sim, left: self.ticks })
    }
    fn ticks(&self) -> u64 {
        self.ticks
    }
}

/// One lanternjaw's trace through one copy.
#[derive(Default)]
struct BodyTrace {
    /// Decision samples: tick, proposal, held, subject.
    samples: Vec<(u64, u8, u8, Subject)>,
    held_ticks: Vec<u64>,
    switches: u32,
    last_held: Option<u8>,
    /// A hash of its pose per tick (`None` once gone).
    pose: Vec<Option<u64>>,
    /// Body energy and digestible stomach at the end of each minute, organic units.
    energy_min: Vec<f64>,
    stomach_min: Vec<f64>,
    stomach_start: Option<f64>,
    stomach_last: f64,
    /// Body energy after the window's first tick (the per-minute scores' baseline).
    energy_first: Option<f64>,
    /// What it had given (births) after the first tick; a birth moves its escrow there.
    given_first: Option<f64>,
    given_last: f64,
}

struct Trace {
    tracked: Vec<u64>,
    /// A hash of every animal's pose after each tick.
    world: Vec<u64>,
    bodies: BTreeMap<u64, BodyTrace>,
    tick: u64,
    /// The fauna tick after the window's first step.
    first_tick: Option<u64>,
}

fn mix(h: u64, x: u64) -> u64 {
    (h ^ x).wrapping_mul(0x0100_0000_01b3).rotate_left(23)
}

fn pose_hash(a: &cubarium_voxel_fauna::Animal) -> u64 {
    let mut h = mix(0xcbf2_9ce4_8422_2325, a.id);
    for v in [a.pose.x, a.pose.z, a.pose.heading_rad, a.pose.y_m] {
        h = mix(h, v.to_bits());
    }
    h
}

impl Trace {
    fn new(tracked: &[u64], menu: usize) -> Trace {
        Trace {
            tracked: tracked.to_vec(),
            world: Vec::new(),
            bodies: tracked.iter().map(|&id| (id, BodyTrace { held_ticks: vec![0; menu], ..Default::default() })).collect(),
            tick: 0,
            first_tick: None,
        }
    }

    fn observe(&mut self, sim: &Sim) {
        let fauna = sim.fauna();
        let now = fauna.tick();
        self.first_tick.get_or_insert(now);
        let phys = fauna.config().founder(LJ);
        let mut h = 0x9e37_79b9_7f4a_7c15u64;
        let mut seen = vec![false; self.tracked.len()];
        let minute_end = (self.tick + 1) % window::TICKS_PER_MIN == 0;
        for a in fauna.view().animals.iter() {
            h = mix(h, pose_hash(a));
            let Some(k) = self.tracked.iter().position(|&id| id == a.id) else { continue };
            seen[k] = true;
            let b = self.bodies.get_mut(&a.id).expect("tracked");
            let stomach = targets::digestible_stomach(a, phys);
            b.stomach_start.get_or_insert(stomach);
            b.energy_first.get_or_insert(window::body_energy(a));
            b.given_first.get_or_insert(a.reproduction.given);
            b.given_last = a.reproduction.given;
            b.stomach_last = stomach;
            b.pose.push(Some(pose_hash(a)));
            let held = a.intent.held;
            if let Some(t) = b.held_ticks.get_mut(usize::from(held)) {
                *t += 1;
            }
            if a.intent.sample_tick == now {
                b.samples.push((now, a.intent.proposal, held, a.intent.subject));
                if b.last_held.is_some_and(|was| was != held) {
                    b.switches += 1;
                }
                b.last_held = Some(held);
            }
            if minute_end {
                // As the window scores it: energy plus what was given since the start.
                b.energy_min.push(window::body_energy(a) + a.reproduction.given - b.given_first.unwrap_or(0.0));
                b.stomach_min.push(stomach);
            }
        }
        for (k, id) in self.tracked.iter().enumerate() {
            if !seen[k] {
                // Gone (a death scores `E_end = 0`; what it gave before still counts).
                let b = self.bodies.get_mut(id).expect("tracked");
                b.pose.push(None);
                if minute_end {
                    b.energy_min.push(b.given_last - b.given_first.unwrap_or(b.given_last));
                    b.stomach_min.push(0.0);
                }
            }
        }
        self.world.push(h);
        self.tick += 1;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    All,
    Focal,
    Null,
}

#[derive(Clone, Copy, Debug)]
struct Item {
    sigma_i: usize,
    mode: Mode,
    start: usize,
    pair: usize,
}

#[derive(Serialize)]
struct BodyRow {
    id: u64,
    on_variant: bool,
    /// Copy 0 (`+`, or the first null copy), copy 1.
    score: [f64; 2],
    intake: [f64; 2],
    /// Intake plus the change of digestible stomach, over the body reference.
    swallowed: [f64; 2],
    died: [bool; 2],
    shares: [Vec<f64>; 2],
    switches: [u32; 2],
    samples: [usize; 2],
    /// Seconds into the window; `null`: never.
    first_proposal_diff_s: Option<f64>,
    first_held_diff_s: Option<f64>,
    first_pose_diff_s: Option<f64>,
    /// Samples before the world first differs whose proposals differ, of those samples.
    proposal_diff_pre: [usize; 2],
    /// The energy score at the end of each minute (`(E_t − E_0)/ref`), each copy.
    score_by_min: [Vec<f64>; 2],
    /// The stomach-aware score at the end of each minute, each copy.
    stomach_score_by_min: [Vec<f64>; 2],
}

#[derive(Serialize)]
struct PairRow {
    sigma: f64,
    mode: Mode,
    start: usize,
    world: usize,
    pair: usize,
    focal: Option<u64>,
    lanternjaws: usize,
    ticks: [u64; 2],
    first_world_diff_s: Option<f64>,
    max_score_gap_other_lineages: f64,
    bodies: Vec<BodyRow>,
    wall_s: f64,
}

/// Run `f` over every item on `threads` workers, results in item order (the trainer's
/// work queue).
fn parallel<T: Send, R: Send>(items: Vec<T>, threads: usize, f: impl Fn(T) -> R + Sync) -> Vec<R> {
    let n = items.len();
    let queue = std::sync::Mutex::new(items.into_iter().enumerate().rev().collect::<Vec<_>>());
    let out = std::sync::Mutex::new((0..n).map(|_| None).collect::<Vec<Option<R>>>());
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1).min(n.max(1)) {
            scope.spawn(|| loop {
                let Some((i, item)) = queue.lock().expect("queue").pop() else { return };
                let r = f(item);
                out.lock().expect("out")[i] = Some(r);
            });
        }
    });
    out.into_inner().expect("out").into_iter().map(|r| r.expect("every item ran")).collect()
}

fn first_diff<T: PartialEq>(a: &[T], b: &[T]) -> Option<usize> {
    a.iter().zip(b).position(|(x, y)| x != y).or_else(|| (a.len() != b.len()).then(|| a.len().min(b.len())))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let centre = arg(&args, "centre").expect("centre=CENTRE.json");
    let out = std::path::PathBuf::from(arg(&args, "out").expect("out=DIR"));
    let num = |k: &str, d: f64| arg(&args, k).map_or(d, |s| s.parse().unwrap_or_else(|e| panic!("{k}: {e}")));
    let worlds = num("worlds", 8.0) as usize;
    let starts_per_world = num("starts", 1.0) as usize;
    let warmup_min = num("warmup_min", 20.0);
    let gap_min = num("gap_min", 10.0);
    let window_min = num("window_min", 10.0);
    let pairs = num("pairs", 8.0) as usize;
    let threads = num("threads", 16.0) as usize;
    let world_seed = num("world_seed", 3001.0) as u64;
    let train_seed: u64 = arg(&args, "train_seed").map_or(20260924, |s| s.parse().expect("train_seed"));
    let generation = num("generation", 0.0) as u32;
    let water_every = num("water_every", f64::from(cubarium_voxel_sim::EXPERIMENT_WATER_EVERY)) as u32;
    let sigmas: Vec<f64> = arg(&args, "sigmas").unwrap_or("0.02,0.05,0.1").split(',').map(|s| s.parse().expect("sigma")).collect();
    let modes: Vec<Mode> = arg(&args, "modes")
        .unwrap_or("all,focal")
        .split(',')
        .map(|m| match m {
            "all" => Mode::All,
            "focal" => Mode::Focal,
            other => panic!("mode {other}: all or focal"),
        })
        .collect();
    let top_up = world::TopUp::parse(arg(&args, "top_up").unwrap_or("lanternjaw=8:0.3")).expect("top_up");
    let tpm = window::TICKS_PER_MIN;
    let ticks = (window_min * tpm as f64).round() as u64;
    std::fs::create_dir_all(&out).expect("out dir");
    cubarium_voxel::world::set_thread_override(1);

    // The centre, as the trainer reads a two-layer file.
    let file = TwoLayerPolicyFile::load(std::path::Path::new(centre)).expect("a two-layer centre");
    assert_eq!(cubarium_search::es::voxel::parse_founder(&file.founder).expect("founder"), LJ, "a lanternjaw centre");
    let ecology = EcologyParams::defaults();
    let arbitration = ecology.fauna().founder(LJ).brain;
    let driver = EpisodeDriver::two_layer(file.net().expect("net"), arbitration, LJ);
    let top = file.layout().range(cubarium_search::es::voxel::two_layer::Layer::Top);
    let rms = (file.theta[top.clone()].iter().map(|x| x * x).sum::<f64>() / top.len() as f64).sqrt();
    eprintln!("# centre: {} parameters, top {} (rms {rms:.4})", file.theta.len(), top.len());
    if arg(&args, "info").is_some() {
        return;
    }
    let tables: Vec<VariantTable> = sigmas
        .iter()
        .map(|&s| VariantTable::antithetic_two_layer(&file, BrainStage::Top, arbitration, train_seed, s, generation, pairs).expect("table"))
        .collect();

    // 1. The starts: found as the training worlds are, warm up on the centre, capture.
    let started = std::time::Instant::now();
    let cfg = world::training_config();
    let founded: Vec<Vec<(usize, SimStart, Vec<u64>)>> = parallel(
        (0..worlds).collect(),
        threads,
        |w| {
            let base = world_seed + w as u64 * cubarium_search::es::voxel::live::trainer::WORLD_SEED_STRIDE;
            let (mut sim, info) = world::found_world_redrawing(&cfg, base, ecology.fauna().founder_counts_without_egg_bank(), &ecology, |n| {
                eprintln!("# world {w}: {n}")
            })
            .expect("a founded world");
            let added = world::top_up(&mut sim, top_up);
            sim.world_mut().set_water_every(water_every);
            cubarium_search::es::voxel::live::install_centres(sim.fauna_mut(), &[(LJ, driver.clone())]);
            eprintln!("# world {w}: seed {} topped up {added}", info.seed);
            let mut got = Vec::new();
            let run = |sim: &mut Sim, minutes: f64| {
                for _ in 0..(minutes * tpm as f64).round() as u64 {
                    sim.step();
                }
            };
            run(&mut sim, warmup_min);
            for k in 0..starts_per_world {
                if k > 0 {
                    run(&mut sim, gap_min);
                }
                let mut ids: Vec<u64> = sim.fauna().view().animals.iter().filter(|a| a.founder == Some(LJ)).map(|a| a.id).collect();
                ids.sort_unstable();
                got.push((w, SimStart::capture(&sim, &[LJ]), ids));
            }
            got
        },
    );
    let starts: Vec<(usize, LiveStart, Vec<u64>)> = founded
        .into_iter()
        .flatten()
        .map(|(w, start, ids)| (w, LiveStart { start, driver: driver.clone(), ticks, water_every }, ids))
        .collect();
    eprintln!("# {} starts in {:.1}s; lanternjaws per start {:?}", starts.len(), started.elapsed().as_secs_f64(), starts.iter().map(|s| s.2.len()).collect::<Vec<_>>());

    // 2. The matched pairs.
    let mut items = Vec::new();
    for s in 0..starts.len() {
        items.push(Item { sigma_i: 0, mode: Mode::Null, start: s, pair: 0 });
        for (si, _) in sigmas.iter().enumerate() {
            for &mode in &modes {
                for p in 0..pairs {
                    items.push(Item { sigma_i: si, mode, start: s, pair: p });
                }
            }
        }
    }
    let kin = KinConfig::default();
    let menu = cubarium_voxel_fauna::menu_of(LJ).len();
    let reference = LJ.manifest().body_reference;
    let path = out.join("pairs.jsonl");
    let log = std::sync::Mutex::new(std::fs::File::create(&path).expect("pairs.jsonl"));
    let total = items.len();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let rows: Vec<Result<(), String>> = parallel(items, threads, |it| {
        let t0 = std::time::Instant::now();
        let (world_i, scene, ids) = &starts[it.start];
        if ids.is_empty() {
            return Err(format!("start {} has no lanternjaw", it.start));
        }
        let focal = (it.mode == Mode::Focal).then(|| ids[(it.pair + it.start) % ids.len()]);
        let cands: [usize; 2] = match it.mode {
            Mode::Null => [0, 0],
            _ => [2 * it.pair, 2 * it.pair + 1],
        };
        let table = &tables[it.sigma_i];
        let mut traces = Vec::new();
        let mut outcomes = Vec::new();
        for c in cands {
            let assign: Vec<(u64, usize)> = match focal {
                Some(f) => vec![(f, c)],
                None => ids.iter().map(|&id| (id, c)).collect(),
            };
            let mut tr = Trace::new(ids, menu);
            let o = matched::run_assigned(scene, &assign, table, &kin, &mut |sim| tr.observe(sim))?;
            traces.push(tr);
            outcomes.push(o);
        }
        let world_diff = first_diff(&traces[0].world, &traces[1].world);
        let secs = |t: usize| t as f64 / f64::from(cubarium_voxel_fauna::TICK_HZ);
        let mut bodies = Vec::new();
        for &id in ids {
            let (a, b) = (&traces[0].bodies[&id], &traces[1].bodies[&id]);
            let m = |k: usize| outcomes[k].metered.iter().find(|m| m.body_id == id).map(|m| m.score);
            let (ma, mb) = (m(0), m(1));
            let shares = |t: &BodyTrace| {
                let n: u64 = t.held_ticks.iter().sum();
                t.held_ticks.iter().map(|&x| if n > 0 { x as f64 / n as f64 } else { 0.0 }).collect::<Vec<f64>>()
            };
            let swallowed = |t: &BodyTrace, s: Option<window::NetEnergy>| {
                s.map_or(f64::NAN, |s| s.intake_normalized + (t.stomach_last - t.stomach_start.unwrap_or(0.0)) / reference)
            };
            // Samples up to the tick the world first differs saw identical inputs in both
            // copies (the decision at that tick was taken on the last identical state).
            let first_tick = traces[0].first_tick.unwrap_or(0);
            let limit = world_diff.map_or(u64::MAX, |w| first_tick + w as u64);
            let pre_samples: Vec<_> = a.samples.iter().zip(&b.samples).filter(|(x, _)| x.0 <= limit).collect();
            let at = |i: usize| secs((a.samples.get(i).or(b.samples.get(i)).map_or(first_tick, |s| s.0) + 1 - first_tick) as usize);
            let by_min = |t: &BodyTrace, stomach: bool| -> Vec<f64> {
                let e0 = t.energy_first.unwrap_or(0.0) + if stomach { t.stomach_start.unwrap_or(0.0) } else { 0.0 };
                t.energy_min
                    .iter()
                    .zip(&t.stomach_min)
                    .map(|(e, s)| (e + if stomach { *s } else { 0.0 } - e0) / reference)
                    .collect()
            };
            bodies.push(BodyRow {
                id,
                on_variant: focal.is_none_or(|f| f == id),
                score: [ma.map_or(f64::NAN, |s| s.score), mb.map_or(f64::NAN, |s| s.score)],
                intake: [ma.map_or(f64::NAN, |s| s.intake_normalized), mb.map_or(f64::NAN, |s| s.intake_normalized)],
                swallowed: [swallowed(a, ma), swallowed(b, mb)],
                died: [ma.is_some_and(|s| s.died), mb.is_some_and(|s| s.died)],
                shares: [shares(a), shares(b)],
                switches: [a.switches, b.switches],
                samples: [a.samples.len(), b.samples.len()],
                first_proposal_diff_s: first_diff(
                    &a.samples.iter().map(|s| (s.0, s.1)).collect::<Vec<_>>(),
                    &b.samples.iter().map(|s| (s.0, s.1)).collect::<Vec<_>>(),
                )
                .map(at),
                first_held_diff_s: first_diff(
                    &a.samples.iter().map(|s| (s.0, s.2, s.3)).collect::<Vec<_>>(),
                    &b.samples.iter().map(|s| (s.0, s.2, s.3)).collect::<Vec<_>>(),
                )
                .map(at),
                first_pose_diff_s: first_diff(&a.pose, &b.pose).map(|i| secs(i + 1)),
                proposal_diff_pre: [pre_samples.iter().filter(|(x, y)| x.1 != y.1).count(), pre_samples.len()],
                score_by_min: [by_min(a, false), by_min(b, false)],
                stomach_score_by_min: [by_min(a, true), by_min(b, true)],
            });
        }
        let mut other_gap: f64 = 0.0;
        for (id, x) in &outcomes[0].scores {
            if ids.contains(id) {
                continue;
            }
            if let Some(y) = outcomes[1].scores.get(id) {
                other_gap = other_gap.max((x.own - y.own).abs());
            }
        }
        let row = PairRow {
            sigma: if it.mode == Mode::Null { 0.0 } else { sigmas[it.sigma_i] },
            mode: it.mode,
            start: it.start,
            world: *world_i,
            pair: it.pair,
            focal,
            lanternjaws: ids.len(),
            ticks: [outcomes[0].ticks, outcomes[1].ticks],
            first_world_diff_s: world_diff.map(|i| secs(i + 1)),
            max_score_gap_other_lineages: other_gap,
            bodies,
            wall_s: t0.elapsed().as_secs_f64(),
        };
        let line = serde_json::to_string(&row).map_err(|e| e.to_string())?;
        {
            use std::io::Write;
            let mut f = log.lock().map_err(|_| "log poisoned")?;
            writeln!(f, "{line}").map_err(|e| e.to_string())?;
            f.flush().map_err(|e| e.to_string())?;
        }
        let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if n % 16 == 0 || n == total {
            eprintln!("# {n}/{total} pairs, {:.0}s", started.elapsed().as_secs_f64());
        }
        Ok(())
    });
    let failed: Vec<String> = rows.into_iter().filter_map(Result::err).collect();
    for f in &failed {
        eprintln!("# failed: {f}");
    }
    eprintln!("# wrote {} ({} failed) in {:.1}s", path.display(), failed.len(), started.elapsed().as_secs_f64());
    std::fs::write(out.join("DONE"), "").expect("DONE");
}
