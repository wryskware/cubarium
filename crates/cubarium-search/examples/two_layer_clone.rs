//! **Clone a menu lineage's heuristic into the two-layer brain** (TWO-LAYER half 2, step 6;
//! `design/behaviour-ai-2026-10-11.md` §10.2, §11.3), beside a plain GRU cloned on the same
//! data, then a first closed-loop read of each in a live world against the heuristic.
//!
//! ```text
//! two_layer_clone founder=seedporter train_seeds=1,2,3 heldout_seed=9 record_s=600 \
//!     closed_seeds=21,22 closed_s=300 updates=300 workers=8 out=DIR
//! ```
//!
//! 1. **Record** each seed's training world (`small`, still water, default founders) under
//!    the heuristics with the tap on. Every `alt_period_s` half the lineage's bodies (by id
//!    parity, alternating) are held to their own answer for `alt_len_s`
//!    (`Fauna::force_own`): those stretches are alternative-mode records.
//! 2. **Fit** the two-layer brain and a plain GRU on the training seeds' records
//!    (`two_layer::clone::fit`, teacher-forced, the same spec).
//! 3. **Held out**: per-intent agreement (two-layer) and per-mode action error (both) on the
//!    held-out seed's teacher and alternative-mode records.
//! 4. **Closed loop** on matched seeds: the lineage on its heuristic, on the two-layer
//!    clone (loaded from its written file, a shadow teacher beside each body), and on the
//!    plain clone; energy (the lineage's body + stores), deaths, bodies. A first read, not
//!    a verdict.
//!
//! One JSON summary on stdout and in `out/summary.json`; the two-layer file in `out/`.

use std::sync::{Arc, Mutex};

use cubarium_search::es::voxel::live::{self, balance, ecology::EcologyParams, world};
use cubarium_search::es::voxel::store::load_brain_driver;
use cubarium_search::es::voxel::two_layer::clone::{self, CloneSpec, RecordSource, RecordedSample, TrajectoryRecord};
use cubarium_search::es::voxel::two_layer::record::{ShadowTeacher, per_mode, records_from_tap};
use cubarium_search::es::voxel::two_layer::{MenuHeads, TwoLayerNet, TwoLayerPolicyFile, TwoLayerShape};
use cubarium_search::es::voxel::EpisodeDriver;
use cubarium_voxel_fauna::{BodySpec, BodyStage, Controller, Founder, Placed, Spot};
use serde_json::json;

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")))
}
fn num<T: std::str::FromStr>(args: &[String], key: &str, default: T) -> T {
    arg(args, key).and_then(|v| v.parse().ok()).unwrap_or(default)
}
fn seeds(args: &[String], key: &str, default: &str) -> Vec<u64> {
    arg(args, key).unwrap_or(default).split(',').filter_map(|s| s.trim().parse().ok()).collect()
}

const TICKS_PER_S: u64 = cubarium_voxel_fauna::TICK_HZ as u64;

/// The training world (`small`, still water) with the ecology's own founders, then the
/// lineage topped up to `bodies` grown bodies under its heuristic, placed on the faces
/// other bodies stand on (the lanternjaw founds as an egg bank, the seedporter is off in
/// the default roster).
fn found(seed: u64, f: Founder, bodies: usize) -> cubarium_voxel_sim::Sim {
    let eco = EcologyParams::defaults();
    let (mut sim, _) = world::found_world(&world::training_config(), seed, eco.fauna().founder_counts(), &eco);
    let have = lineage_ids(&sim, f).len();
    if have < bodies {
        let (w, flora) = (sim.world().clone(), sim.flora().clone());
        let sites: Vec<_> = sim.fauna().view().animals.iter().map(|a| a.site).collect();
        let teachers = sim.fauna().config().teachers;
        let mut placed = 0;
        for (k, site) in sites.iter().enumerate() {
            if have + placed >= bodies {
                break;
            }
            let spec = BodySpec { stage: BodyStage::Adult, heading_rad: k as f64 * 1.3, ..BodySpec::default() };
            if let Ok(Placed::Body { id, .. }) = sim.fauna_mut().place_body(&w, &flora, f, Spot::Face(*site), spec) {
                sim.fauna_mut().set_controller(id, cubarium_voxel_fauna::heuristic(f, teachers));
                placed += 1;
            }
        }
    }
    sim
}

fn lineage_ids(sim: &cubarium_voxel_sim::Sim, f: Founder) -> Vec<u64> {
    sim.fauna().view().animals.iter().filter(|a| a.founder == Some(f)).map(|a| a.id).collect()
}

/// Record one seed: the tap's records, teacher and alternative-mode.
fn record(seed: u64, f: Founder, bodies: usize, shape: &TwoLayerShape, seconds: u64, alt_period_s: u64, alt_len_s: u64) -> Vec<TrajectoryRecord> {
    let mut sim = found(seed, f, bodies);
    sim.fauna_mut().start_tap();
    let mut forced: Vec<(u64, u64, u64)> = Vec::new(); // (id, from tick, to tick)
    let mut on: Vec<(u64, u64)> = Vec::new();
    for s in 0..seconds {
        if alt_period_s > 0 && s % alt_period_s == 0 {
            let phase = (s / alt_period_s) % 2;
            let now = sim.fauna().tick();
            for id in lineage_ids(&sim, f) {
                if id % 2 == phase && s > 0 {
                    sim.fauna_mut().force_own(id, true);
                    on.push((id, now));
                }
            }
        }
        if alt_period_s > 0 && s % alt_period_s == alt_len_s {
            let now = sim.fauna().tick();
            for (id, from) in on.drain(..) {
                sim.fauna_mut().force_own(id, false);
                forced.push((id, from, now));
            }
        }
        for _ in 0..TICKS_PER_S {
            sim.step();
        }
    }
    let now = sim.fauna().tick();
    forced.extend(on.drain(..).map(|(id, from)| (id, from, now)));
    let tap = sim.fauna_mut().take_tap().expect("tap");
    let is_forced = |id: u64, tick: u64| forced.iter().any(|&(b, from, to)| b == id && tick > from && tick <= to);
    records_from_tap(&tap, f, shape, &is_forced, 8).into_iter().map(|(_, r)| r).collect()
}

/// A lineage's numbers in a live world.
fn tally(sim: &cubarium_voxel_sim::Sim, f: Founder) -> (usize, f64, u64) {
    let v = sim.fauna().view();
    let config = sim.fauna().config();
    let body: f64 = v.animals.iter().filter(|a| a.founder == Some(f)).map(|a| a.body).sum();
    let energy = body + balance::stores(v.animals, config, f);
    let deaths: u64 = v.ledger.deaths_by_founder_cause[f.index()].iter().sum();
    (v.animals.iter().filter(|a| a.founder == Some(f)).count(), energy, deaths)
}

enum Arm {
    Heuristic,
    Driver(EpisodeDriver),
    Shadow(Arc<TwoLayerNet>),
}

/// One closed-loop arm on one seed; the shadow's labelled samples come back for the
/// closed-loop correction pass.
fn closed(seed: u64, f: Founder, bodies: usize, arm: &Arm, seconds: u64) -> (serde_json::Value, Vec<TrajectoryRecord>) {
    let mut sim = found(seed, f, bodies);
    let logs: Arc<Mutex<Vec<Arc<Mutex<Vec<RecordedSample>>>>>> = Arc::default();
    match arm {
        Arm::Heuristic => {}
        Arm::Driver(d) => live::install_centres(sim.fauna_mut(), &[(f, d.clone())]),
        Arm::Shadow(net) => {
            let fauna = sim.fauna_mut();
            let teachers = fauna.config().teachers;
            let arb = fauna.config().founder(f).brain;
            let (net, logs) = (Arc::clone(net), Arc::clone(&logs));
            fauna.set_founder_factory(
                f,
                Arc::new(move || -> Box<dyn Controller> {
                    let log = Arc::new(Mutex::new(Vec::new()));
                    logs.lock().unwrap().push(Arc::clone(&log));
                    let brain = net.brain(arb, f.manifest().controller_period_s);
                    Box::new(ShadowTeacher::new(brain, cubarium_voxel_fauna::heuristic(f, teachers), f, log))
                }),
            );
            fauna.set_policy_driven(f, true);
            for id in lineage_ids(&sim, f) {
                sim.fauna_mut().install_founder_controller(id, f);
            }
        }
    }
    let start = tally(&sim, f);
    let started_ids = lineage_ids(&sim, f);
    for _ in 0..seconds * TICKS_PER_S {
        sim.step();
    }
    let end = tally(&sim, f);
    let born = lineage_ids(&sim, f).iter().filter(|id| !started_ids.contains(id)).count();
    let period = f.manifest().controller_period_s;
    let shadow: Vec<TrajectoryRecord> = logs
        .lock()
        .unwrap()
        .iter()
        .map(|l| TrajectoryRecord { source: RecordSource::ClosedLoop, sample_period_s: period, samples: l.lock().unwrap().clone() })
        .filter(|r| !r.samples.is_empty())
        .collect();
    (
        json!({"seed": seed, "bodies_start": start.0, "bodies_end": end.0, "born": born,
               "energy_start": start.1, "energy_end": end.1, "deaths": end.2 - start.2}),
        shadow,
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    cubarium_voxel::world::set_thread_override(1);
    let f = cubarium_search::es::voxel::parse_founder(arg(&args, "founder").unwrap_or("seedporter")).expect("founder");
    let out = std::path::PathBuf::from(arg(&args, "out").unwrap_or("runs/two-layer-clone"));
    std::fs::create_dir_all(&out).expect("out dir");
    let (record_s, closed_s) = (num(&args, "record_s", 600u64), num(&args, "closed_s", 300u64));
    let (alt_period_s, alt_len_s) = (num(&args, "alt_period_s", 60u64), num(&args, "alt_len_s", 20u64));
    let spec = CloneSpec {
        updates: num(&args, "updates", 300u32),
        learning_rate: num(&args, "lr", 0.01),
        chunk: num(&args, "chunk", 120usize),
        clip: 5.0,
        seed: num(&args, "fit_seed", 7u64),
        workers: num(&args, "workers", 8usize),
    };
    let heads = MenuHeads { kick: false, call_kinds: 0, rest: false, top_hidden: num(&args, "top_hidden", 32usize), pointer_width: num(&args, "pointer_width", 16usize) };
    let bodies = num(&args, "bodies", 8usize);
    let manifest = f.manifest();
    let two = TwoLayerShape::menu_from_manifest(&manifest, heads);
    let plain = TwoLayerShape::plain_from_manifest(&manifest);
    let arb = cubarium_voxel_fauna::FaunaConfig::default().founder(f).brain;
    let t0 = std::time::Instant::now();

    // Every seed's recording on its own thread (each world single-threaded).
    let train_seeds = seeds(&args, "train_seeds", "1,2,3");
    let heldout_seed = num(&args, "heldout_seed", 9u64);
    let mut recorded: Vec<Vec<TrajectoryRecord>> = std::thread::scope(|scope| {
        let handles: Vec<_> = train_seeds
            .iter()
            .chain(std::iter::once(&heldout_seed))
            .map(|&s| {
                let two = &two;
                scope.spawn(move || record(s, f, bodies, two, record_s, alt_period_s, alt_len_s))
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("recording")).collect()
    });
    let held = recorded.pop().expect("held-out");
    let train: Vec<TrajectoryRecord> = recorded.into_iter().flatten().collect();
    let count = |rs: &[TrajectoryRecord], src: RecordSource| rs.iter().filter(|r| r.source == src).map(|r| r.samples.len()).sum::<usize>();
    eprintln!("# recorded {:.0} s: train teacher {} alt {}; held-out teacher {} alt {}", t0.elapsed().as_secs_f64(),
        count(&train, RecordSource::Teacher), count(&train, RecordSource::AlternativeMode),
        count(&held, RecordSource::Teacher), count(&held, RecordSource::AlternativeMode));

    let (theta2, report2) = clone::fit(&two, &arb, &train, None, &spec).expect("two-layer fit");
    eprintln!("# two-layer fitted {:.0} s", t0.elapsed().as_secs_f64());
    let (theta_p, report_p) = clone::fit(&plain, &arb, &train, None, &spec).expect("plain fit");
    eprintln!("# plain fitted {:.0} s", t0.elapsed().as_secs_f64());

    let menu = manifest.menu;
    let names: Vec<&str> = (0..menu.named.len()).map(|k| menu.name(k)).collect();
    let split = |src: RecordSource| held.iter().filter(|r| r.source == src).cloned().collect::<Vec<_>>();
    let mut held_out = serde_json::Map::new();
    for (label, rs) in [("teacher", split(RecordSource::Teacher)), ("alternative_mode", split(RecordSource::AlternativeMode))] {
        let m2 = per_mode(&two, &arb, &theta2, &rs, menu.named.len());
        let mp = per_mode(&plain, &arb, &theta_p, &rs, menu.named.len());
        let (agree, err2) = m2.rates(two.actions.len());
        let (_, errp) = mp.rates(plain.actions.len());
        let rows: Vec<_> = (0..names.len())
            .map(|k| json!({"intent": names[k], "labels": m2.intent_labels[k], "agreement_two_layer": agree[k],
                             "mode_samples": m2.mode_samples[k], "action_mse_two_layer": err2[k], "action_mse_plain": errp[k]}))
            .collect();
        held_out.insert(label.into(), json!(rows));
    }

    let net = Arc::new(TwoLayerNet::new(two.clone(), theta2).expect("net"));
    let file = TwoLayerPolicyFile::new(f, &net, spec.seed, "clone");
    let path = out.join(format!("{}-two-layer.json", f.name()));
    file.write(&path).expect("write");
    let two_driver = load_brain_driver(&path, f).expect("loads back");
    let plain_driver = EpisodeDriver::gru(&theta_p, f).expect("plain driver");

    let mut arms = serde_json::Map::new();
    let mut shadow_samples = 0;
    let closed_seeds = seeds(&args, "closed_seeds", "21,22");
    let plan: Vec<(u64, &str, Arm)> = closed_seeds
        .iter()
        .flat_map(|&s| {
            [
                (s, "heuristic", Arm::Heuristic),
                (s, "two_layer", Arm::Driver(two_driver.clone())),
                (s, "two_layer_shadowed", Arm::Shadow(Arc::clone(&net))),
                (s, "plain_gru", Arm::Driver(plain_driver.clone())),
            ]
        })
        .collect();
    let results: Vec<(serde_json::Value, Vec<TrajectoryRecord>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = plan.iter().map(|(s, _, arm)| scope.spawn(move || closed(*s, f, bodies, arm, closed_s))).collect();
        handles.into_iter().map(|h| h.join().expect("closed loop")).collect()
    });
    eprintln!("# closed loop {:.0} s", t0.elapsed().as_secs_f64());
    for ((_, label, _), (mut row, shadow)) in plan.iter().zip(results) {
        let n = shadow.iter().map(|r| r.samples.len()).sum::<usize>();
        shadow_samples += n;
        if *label == "two_layer_shadowed" {
            let m = per_mode(&two, &arb, net.theta(), &shadow, menu.named.len());
            let (_, err) = m.rates(two.actions.len());
            row["shadow_samples"] = json!(n);
            row["shadow_action_mse_by_held_mode"] =
                json!(names.iter().zip(&err).zip(&m.mode_samples).map(|((n, e), c)| json!({"intent": n, "samples": c, "mse": e})).collect::<Vec<_>>());
        }
        arms.entry(label.to_string()).or_insert_with(|| json!([])).as_array_mut().unwrap().push(row);
    }
    let summary = json!({
        "founder": f.name(),
        "bodies": bodies, "record_s": record_s, "closed_s": closed_s, "alt_period_s": alt_period_s, "alt_len_s": alt_len_s,
        "samples": {"train_teacher": count(&train, RecordSource::Teacher), "train_alternative_mode": count(&train, RecordSource::AlternativeMode),
                    "heldout_teacher": count(&held, RecordSource::Teacher), "heldout_alternative_mode": count(&held, RecordSource::AlternativeMode),
                    "closed_loop_shadow": shadow_samples},
        "fit": {"updates": spec.updates, "two_layer_last": report2.last, "plain_last": report_p.last},
        "held_out": held_out,
        "closed_loop": arms,
        "two_layer_file": path.display().to_string(),
        "wall_s": t0.elapsed().as_secs_f64(),
    });
    let text = serde_json::to_string_pretty(&summary).expect("json");
    std::fs::write(out.join("summary.json"), &text).expect("summary");
    println!("{text}");
}
