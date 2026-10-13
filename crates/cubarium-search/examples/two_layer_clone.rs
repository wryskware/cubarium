//! **Clone a menu lineage's heuristic into the two-layer brain** (TWO-LAYER half 2, step 6;
//! BRAIN-NEXT steps 3–6; `design/behaviour-ai-2026-10-11.md` §10.2, §11.3), beside a plain
//! GRU cloned on the same data; then a closed-loop correction round, per-mode readouts and
//! per-mode competence on staged scenes, and a first closed-loop read in live worlds.
//!
//! ```text
//! two_layer_clone founder=seedporter train_seeds=1,2,3 heldout_seed=9 record_s=600 \
//!     scene_seeds=1,2,3,4,5,6,7,8 scene_heldout=101,102,103,104 refit_rounds=1 \
//!     closed_seeds=21,22 closed_s=300 updates=300 workers=8 out=DIR \
//!     [fullness=F] [also=LINEAGE=N[:FULLNESS],...] [scenario=PATH] [hound_every_s=S hound_m=M]
//! ```
//!
//! 1. **Record** each seed's training world (`small`, still water, default founders) under
//!    the heuristics with the tap on. Every `alt_period_s` half the lineage's bodies (by id
//!    parity, alternating) are held to their own answer for `alt_len_s`
//!    (`Fauna::force_own`): those stretches are alternative-mode records. A terrain seed the
//!    training world refuses is skipped with a note.
//! 2. **Staged scenes** (`cubarium_voxel_sim::mode_scenes`): every scene of the lineage on
//!    `scene_seeds`, under its heuristic, recorded as teacher streams; `scene_heldout` seeds
//!    are held out.
//! 3. **Fit** the two-layer brain and a plain GRU on the training records
//!    (`two_layer::clone::fit`, teacher-forced, the same spec).
//! 4. **Closed-loop correction** (`refit_rounds`, §11.3): each clone drives the lineage in
//!    the training worlds (`closed_record_s`) and the training scenes, the dry-run teacher
//!    labelling the states it reaches (`Fauna::dry_teach`); each refits from where it stood
//!    on its first records plus its own closed-loop records.
//! 5. **Held out**: per-intent agreement (two-layer) and per-mode action error (both) on the
//!    held-out seed's and held-out scenes' teacher records.
//! 6. **Competence** (§11.3, "Competence before freezing"): every scene on the held-out
//!    scene seeds, closed loop, under the heuristic, the two-layer clone and the plain clone,
//!    on identical starts (`two_layer::scenes::competence`, with each arm's null pair).
//! 7. **Live closed loop** on `closed_seeds` (skipped with `closed_s=0`): energy, deaths and
//!    bodies of the lineage under each arm. A first read, not a verdict.
//!
//! One JSON summary in `out/summary.json` and a per-mode table on stdout; the clones' files in
//! `out/`. `confusion=1` (CLONE-AGREE) also prints the two-layer top's confusion matrix
//! (label row, pick column) on the held-out live teacher records and, in sample, on the
//! training teacher records, and adds both to the summary as `confusion`; `dump=1` also
//! writes every scored held-out live label to `out/heldout_picks.jsonl` (label, pick, the
//! hold before, the observation) for reading which senses separate a confused pair.
//!
//! LS-TEACH: `scenario=PATH` applies a scenario file (`[[calve]]`, `[[place]]`,
//! `[[populate]]`; `cubarium_voxel_sim::edit::Scenario`) to every live world after the
//! top-ups (the teacher worlds' calves: `config/scenarios/calves/ls-teach.toml`), and
//! `hound_every_s=S` moves a Veilhound to `hound_m` (default 8) metres from a grown body of the
//! lineage every `S` seconds of every live world ([`world::show_hunter`]). Both off by default:
//! the worlds as before.


use cubarium_search::es::voxel::live::{self, balance, ecology::EcologyParams, world};
use cubarium_search::es::voxel::store::load_brain_driver;
use cubarium_search::es::voxel::two_layer::clone::{self, CloneSpec, RecordSource, TrajectoryRecord};
use cubarium_search::es::voxel::two_layer::record::{closed_loop_records, per_mode, records_from_tap};
use cubarium_search::es::voxel::two_layer::scenes;
use cubarium_search::es::voxel::two_layer::{MenuHeads, TwoLayerNet, TwoLayerPolicyFile, TwoLayerShape};
use cubarium_search::es::voxel::EpisodeDriver;
use cubarium_voxel_fauna::{DryScope, Founder};
use cubarium_voxel_sim::mode_scenes::{self, ModeScene};
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

/// The top-up's fullness for every world this run founds (`fullness=`; unset: `None`).
static FULLNESS: std::sync::OnceLock<Option<f64>> = std::sync::OnceLock::new();

/// Other lineages topped up in every world this run founds, before the cloned one
/// (`also=LINEAGE=N[:FULLNESS],...`; BRAIN-REST step 4): a loftstrider's defence modes need
/// Veilhounds, which the training worlds found none of.
static ALSO: std::sync::OnceLock<Vec<world::TopUp>> = std::sync::OnceLock::new();

/// LS-TEACH: a scenario (`scenario=PATH`, the `[[calve]]`/`[[place]]`/`[[populate]]` file of
/// `cubarium_voxel_sim::edit::Scenario`) applied to every live world this run founds, after
/// the top-ups: the teacher worlds' calves. Unset: the worlds as before.
static SCENARIO: std::sync::OnceLock<Option<cubarium_voxel_sim::edit::Scenario>> = std::sync::OnceLock::new();

/// LS-TEACH: every `hound_every_s` seconds of a live world (0: never, the default), a
/// Veilhound is moved to `hound_m` metres along the ring from a grown body of the lineage, a
/// mother first ([`world::show_hunter`]), so its threat labels come with the hound in its
/// senses: `(hound_every_s, hound_m)`.
static HOUNDS: std::sync::OnceLock<(u64, f64)> = std::sync::OnceLock::new();

/// Step a live world one second; at second `s` of it, show a Veilhound when [`HOUNDS`] says.
fn second(sim: &mut cubarium_voxel_sim::Sim, f: Founder, s: u64) {
    let (every, at_m) = HOUNDS.get().copied().unwrap_or((0, 0.0));
    if every > 0 && s > 0 && s % every == 0 && f != Founder::Chorister {
        if let Err(why) = world::show_hunter(sim, f, Founder::Chorister, s / every, at_m) {
            eprintln!("# second {s}: no Veilhound shown: {why}");
        }
    }
    for _ in 0..TICKS_PER_S {
        sim.step();
    }
}

const TICKS_PER_S: u64 = cubarium_voxel_fauna::TICK_HZ as u64;

/// The training world (`small`, still water) with the ecology's own founders, then the
/// lineage topped up to `bodies` grown bodies under its heuristic, placed on the faces
/// other bodies stand on (the lanternjaw founds as an egg bank, the seedporter is off in
/// the default roster). `Err` for a terrain seed the founding refuses.
///
/// `fullness=` (BRAIN-FIX-2 step 3) sets the topped-up bodies' stores as a fraction of
/// capacity, so a lanternjaw recording sees hungry bodies (patrol and ambush) rather than
/// sated ones digesting; unset, a placed body's default, as before.
fn found(seed: u64, f: Founder, bodies: usize) -> Result<cubarium_voxel_sim::Sim, String> {
    let eco = EcologyParams::defaults();
    let (mut sim, _) = world::try_found_world(&world::training_config(), seed, eco.fauna().founder_counts(), &eco)?;
    for t in ALSO.get().map(Vec::as_slice).unwrap_or_default() {
        let placed = world::top_up(&mut sim, *t);
        eprintln!("# seed {seed}: {placed} {} placed beside it", t.founder.name());
    }
    let had = lineage_ids(&sim, f).len();
    let placed = world::top_up(&mut sim, world::TopUp { founder: f, bodies, fullness: FULLNESS.get().copied().flatten() });
    eprintln!("# seed {seed}: {had} {} founded, {placed} placed by the top-up", f.name());
    if let Some(sc) = SCENARIO.get().and_then(Option::as_ref) {
        let r = sim.edit(|e| sc.apply(e));
        eprintln!("# seed {seed}: scenario applied {:?} refused {:?}", r.applied, r.refused);
    }
    Ok(sim)
}

fn lineage_ids(sim: &cubarium_voxel_sim::Sim, f: Founder) -> Vec<u64> {
    sim.fauna().view().animals.iter().filter(|a| a.founder == Some(f)).map(|a| a.id).collect()
}

/// Record one seed: the tap's records, teacher and alternative-mode.
fn record(seed: u64, f: Founder, bodies: usize, shape: &TwoLayerShape, seconds: u64, alt_period_s: u64, alt_len_s: u64) -> Result<Vec<TrajectoryRecord>, String> {
    let mut sim = found(seed, f, bodies)?;
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
        second(&mut sim, f, s);
    }
    let now = sim.fauna().tick();
    forced.extend(on.drain(..).map(|(id, from)| (id, from, now)));
    let tap = sim.fauna_mut().take_tap().ok_or("tap")?;
    let is_forced = |id: u64, tick: u64| forced.iter().any(|&(b, from, to)| b == id && tick > from && tick <= to);
    Ok(records_from_tap(&tap, f, shape, &is_forced, 8).into_iter().map(|(_, r)| r).collect())
}

/// One live training world with the lineage on `driver` and the dry-run teacher beside it:
/// its closed-loop records.
fn record_closed(seed: u64, f: Founder, bodies: usize, shape: &TwoLayerShape, seconds: u64, driver: &EpisodeDriver) -> Result<Vec<TrajectoryRecord>, String> {
    let mut sim = found(seed, f, bodies)?;
    live::install_centres(sim.fauna_mut(), &[(f, driver.clone())]);
    sim.fauna_mut().dry_teach(f, Some(DryScope::Brains));
    sim.fauna_mut().start_tap();
    for s in 0..seconds {
        second(&mut sim, f, s);
    }
    let tap = sim.fauna_mut().take_tap().ok_or("tap")?;
    Ok(closed_loop_records(&tap, f, shape, 8).into_iter().map(|(_, r)| r).collect())
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

/// One live closed-loop arm on one seed (`None`: the heuristic).
fn closed(seed: u64, f: Founder, bodies: usize, arm: Option<&EpisodeDriver>, seconds: u64) -> Result<serde_json::Value, String> {
    let mut sim = found(seed, f, bodies)?;
    if let Some(d) = arm {
        live::install_centres(sim.fauna_mut(), &[(f, d.clone())]);
    }
    let start = tally(&sim, f);
    let started_ids = lineage_ids(&sim, f);
    for s in 0..seconds {
        second(&mut sim, f, s);
    }
    let end = tally(&sim, f);
    let born = lineage_ids(&sim, f).iter().filter(|id| !started_ids.contains(id)).count();
    Ok(json!({"seed": seed, "bodies_start": start.0, "bodies_end": end.0, "born": born,
               "energy_start": start.1, "energy_end": end.1, "deaths": end.2 - start.2}))
}

/// Run `f` on every item, one thread each, results in order.
fn each<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = items.iter().map(|x| scope.spawn(|| f(x))).collect();
        handles.into_iter().map(|h| h.join().expect("worker")).collect()
    })
}

/// Keep the records, note every refused seed.
fn keep(results: Vec<(u64, Result<Vec<TrajectoryRecord>, String>)>, skipped: &mut Vec<serde_json::Value>, what: &str) -> Vec<TrajectoryRecord> {
    let mut out = Vec::new();
    for (seed, r) in results {
        match r {
            Ok(rs) => out.extend(rs),
            Err(why) => {
                eprintln!("# skipped {what} seed {seed}: {why}");
                skipped.push(json!({"what": what, "seed": seed, "why": why}));
            }
        }
    }
    out
}

fn samples(rs: &[TrajectoryRecord], src: RecordSource) -> usize {
    rs.iter().filter(|r| r.source == src).map(|r| r.samples.len()).sum()
}

/// BRAIN-REST step 4: each intent's labels in `rs`'s teacher records: the held intent at every sample
/// and the proposal at every decision sample, by name (a lineage whose teacher never shows a
/// mode is read here before any fit).
fn label_counts(rs: &[TrajectoryRecord], names: &[&str]) -> serde_json::Value {
    let mut held = vec![0usize; names.len()];
    let mut proposed = vec![0usize; names.len()];
    for s in rs.iter().filter(|r| r.source == RecordSource::Teacher).flat_map(|r| &r.samples) {
        if let Some(h) = held.get_mut(s.held.intent) {
            *h += 1;
        }
        if let Some(p) = s.proposal.as_ref().and_then(|p| proposed.get_mut(p.intent)) {
            *p += 1;
        }
    }
    json!(names.iter().enumerate().map(|(k, n)| (n.to_string(), json!({"held": held[k], "proposed": proposed[k]}))).collect::<serde_json::Map<_, _>>())
}

/// A confusion matrix as text: one row per label with any count, its picks by intent.
/// `masked`: per label, the labels its intent mask ruled out (not in the matrix).
fn confusion_text(title: &str, names: &[&str], m: &[Vec<usize>], masked: &[usize]) -> String {
    let used: Vec<usize> = (0..names.len()).filter(|&k| m[k].iter().sum::<usize>() > 0 || m.iter().any(|r| r[k] > 0) || masked[k] > 0).collect();
    let short = |n: &str| n.chars().take(7).collect::<String>();
    let mut out = format!("# {title}: label (row) -> two-layer pick (column); masked: labels the mask ruled out\n{:16} {:>7} {:>7}", "label", "n", "masked");
    for &k in &used {
        out += &format!(" {:>7}", short(names[k]));
    }
    for &r in &used {
        let n: usize = m[r].iter().sum();
        if n == 0 && masked[r] == 0 {
            continue;
        }
        out += &format!("\n{:16} {:>7} {:>7}", names[r], n, masked[r]);
        for &k in &used {
            out += &format!(" {:>7}", m[r][k]);
        }
    }
    out
}

fn mean(xs: impl Iterator<Item = f64>) -> f64 {
    let (s, n) = xs.fold((0.0, 0usize), |(s, n), x| (s + x, n + 1));
    if n > 0 { s / n as f64 } else { f64::NAN }
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
    let refit = CloneSpec { updates: num(&args, "refit_updates", spec.updates / 2), ..spec };
    let refit_rounds = num(&args, "refit_rounds", 1usize);
    let closed_record_s = num(&args, "closed_record_s", 300u64);
    let heads = MenuHeads { kick: false, call_kinds: 0, rest: false, top_hidden: num(&args, "top_hidden", 32usize), pointer_width: num(&args, "pointer_width", 16usize) };
    let bodies = num(&args, "bodies", 8usize);
    let fullness: Option<f64> = arg(&args, "fullness").map(|v| v.parse().expect("fullness: a fraction"));
    assert!(fullness.is_none_or(|x| (0.0..=1.0).contains(&x)), "fullness is a fraction");
    FULLNESS.set(fullness).expect("set once");
    let also: Vec<world::TopUp> = arg(&args, "also")
        .map(|v| v.split(',').filter(|x| !x.is_empty()).map(|x| world::TopUp::parse(x).expect("also: LINEAGE=N[:FULLNESS]")).collect())
        .unwrap_or_default();
    assert!(also.iter().all(|t| t.founder != f), "also= tops up other lineages");
    ALSO.set(also.clone()).expect("set once");
    let scenario_path = arg(&args, "scenario").map(str::to_string);
    let scenario = scenario_path.as_deref().map(|p| cubarium_voxel_sim::edit::Scenario::read(std::path::Path::new(p)).expect("scenario"));
    SCENARIO.set(scenario).expect("set once");
    let hounds = (num(&args, "hound_every_s", 0u64), num(&args, "hound_m", 8.0f64));
    assert!(hounds.1.is_finite() && hounds.1 > 0.0, "hound_m: metres");
    HOUNDS.set(hounds).expect("set once");
    let mut scene_cfg = mode_scenes::Config::default();
    if let Some(s) = arg(&args, "scene_s").and_then(|v| v.parse().ok()) {
        scene_cfg.duration_s = s;
    }
    scene_cfg.validate().expect("scene config");
    let manifest = f.manifest();
    let two = TwoLayerShape::menu_from_manifest(&manifest, heads);
    let plain = TwoLayerShape::plain_from_manifest(&manifest);
    let arb = cubarium_voxel_fauna::FaunaConfig::default().founder(f).brain;
    let t0 = std::time::Instant::now();
    let mut skipped = Vec::new();
    let lineage_scenes = ModeScene::of(f);

    // 1. Live training worlds, each on its own thread (each world single-threaded).
    let train_seeds = seeds(&args, "train_seeds", "1,2,3");
    let heldout_seed = num(&args, "heldout_seed", 9u64);
    let live_seeds: Vec<u64> = train_seeds.iter().copied().chain(std::iter::once(heldout_seed)).collect();
    let mut recorded: Vec<(u64, Result<Vec<TrajectoryRecord>, String>)> =
        live_seeds.iter().copied().zip(each(&live_seeds, |&s| record(s, f, bodies, &two, record_s, alt_period_s, alt_len_s))).collect();
    for (seed, r) in &recorded {
        if let Ok(rs) = r {
            eprintln!("# live seed {seed}: {} records, {} samples", rs.len(), rs.iter().map(|r| r.samples.len()).sum::<usize>());
        }
    }
    let held_live = keep(vec![recorded.pop().expect("held-out")], &mut skipped, "held-out live");
    let mut train = keep(recorded, &mut skipped, "live");

    // 2. Staged scenes: training seeds into the records, held-out seeds kept by scene.
    let scene_seeds = seeds(&args, "scene_seeds", "1,2,3,4,5,6,7,8");
    let scene_heldout = seeds(&args, "scene_heldout", "101,102,103,104");
    let jobs: Vec<(ModeScene, u64)> = lineage_scenes.iter().flat_map(|&m| scene_seeds.iter().map(move |&s| (m, s))).collect();
    let got = each(&jobs, |&(m, s)| scenes::record(&scene_cfg, m, s, &two, 8));
    let train_scene: Vec<TrajectoryRecord> =
        keep(jobs.iter().map(|j| j.1).zip(got).collect(), &mut skipped, "scene");
    let held_scene: Vec<(ModeScene, Vec<TrajectoryRecord>)> = lineage_scenes
        .iter()
        .map(|&m| {
            let rs = each(&scene_heldout, |&s| scenes::record(&scene_cfg, m, s, &two, 8));
            (m, keep(scene_heldout.iter().copied().zip(rs).collect(), &mut skipped, "held-out scene"))
        })
        .collect();
    train.extend(train_scene.iter().cloned());
    eprintln!(
        "# recorded {:.0} s: live teacher {} alt {}, scene teacher {}; held-out live {} scenes {}",
        t0.elapsed().as_secs_f64(),
        samples(&train, RecordSource::Teacher) - samples(&train_scene, RecordSource::Teacher),
        samples(&train, RecordSource::AlternativeMode),
        samples(&train_scene, RecordSource::Teacher),
        held_live.iter().map(|r| r.samples.len()).sum::<usize>(),
        held_scene.iter().flat_map(|(_, rs)| rs).map(|r| r.samples.len()).sum::<usize>()
    );

    let label_names: Vec<&str> = (0..manifest.menu.named.len()).map(|k| manifest.menu.name(k)).collect();
    let train_labels = label_counts(&train, &label_names);
    eprintln!("# training labels (held, proposed): {train_labels}");

    // 3. The first fits.
    let (theta2_first, report2) = clone::fit(&two, &arb, &train, None, &spec).expect("two-layer fit");
    let (theta_p_first, report_p) = clone::fit(&plain, &arb, &train, None, &spec).expect("plain fit");
    eprintln!("# fitted {:.0} s", t0.elapsed().as_secs_f64());

    // 4. Closed-loop correction rounds: each clone drives, the dry teacher labels its states.
    let driver_of = |shape: &TwoLayerShape, theta: &[f64], name: &str| -> EpisodeDriver {
        if shape.is_plain() {
            EpisodeDriver::gru(theta, f).expect("plain driver")
        } else {
            let net = TwoLayerNet::new(shape.clone(), theta.to_vec()).expect("net");
            let file = TwoLayerPolicyFile::new(f, &net, spec.seed, name);
            let path = out.join(format!("{}-{name}.json", f.name()));
            file.write(&path).expect("write");
            load_brain_driver(&path, f).expect("loads back")
        }
    };
    let (mut theta2, mut theta_p) = (theta2_first.clone(), theta_p_first.clone());
    let mut rounds = Vec::new();
    for round in 0..refit_rounds {
        let mut row = serde_json::Map::new();
        for (label, shape, theta) in [("two_layer", &two, &mut theta2), ("plain", &plain, &mut theta_p)] {
            let d = driver_of(shape, theta, &format!("{label}-round{round}"));
            let live_closed = each(&train_seeds, |&s| record_closed(s, f, bodies, &two, closed_record_s, &d));
            let mut closed_rs = keep(train_seeds.iter().copied().zip(live_closed).collect(), &mut skipped, "closed-loop live");
            let got = each(&jobs, |&(m, s)| scenes::closed_loop(&scene_cfg, m, s, &d, &two, 8));
            closed_rs.extend(keep(jobs.iter().map(|j| j.1).zip(got).collect(), &mut skipped, "closed-loop scene"));
            // The plain brain refits on the same labels: its shape reads only the bottom's.
            let mut all = train.clone();
            all.extend(closed_rs.iter().cloned());
            let (next, rep) = clone::fit(shape, &arb, &all, Some(theta), &refit).expect("refit");
            *theta = next;
            row.insert(label.into(), json!({"closed_loop_samples": closed_rs.iter().map(|r| r.samples.len()).sum::<usize>(), "last": rep.last}));
        }
        eprintln!("# refit round {round} {:.0} s", t0.elapsed().as_secs_f64());
        rounds.push(serde_json::Value::Object(row));
    }

    // 5. Held out, per mode.
    let menu = manifest.menu;
    let names: Vec<&str> = (0..menu.named.len()).map(|k| menu.name(k)).collect();
    let table = |rs: &[TrajectoryRecord]| {
        let m2 = per_mode(&two, &arb, &theta2, rs, names.len());
        let m1 = per_mode(&two, &arb, &theta2_first, rs, names.len());
        let mp = per_mode(&plain, &arb, &theta_p, rs, names.len());
        let (agree, err2) = m2.rates(two.actions.len());
        let (agree1, err1) = m1.rates(two.actions.len());
        let (_, errp) = mp.rates(plain.actions.len());
        let (switch2, switch1) = (m2.switch_rates(), m1.switch_rates());
        (0..names.len())
            .map(|k| {
                json!({"intent": names[k], "labels": m2.intent_labels[k], "mode_samples": m2.mode_samples[k],
                       "switch_labels": m2.switch_labels[k], "switch_agreement_two_layer": switch2[k], "switch_agreement_two_layer_before_refit": switch1[k],
                       "agreement_two_layer": agree[k], "agreement_two_layer_before_refit": agree1[k],
                       "action_mse_two_layer": err2[k], "action_mse_two_layer_before_refit": err1[k], "action_mse_plain": errp[k]})
            })
            .collect::<Vec<_>>()
    };
    let teacher = |rs: &[TrajectoryRecord]| rs.iter().filter(|r| r.source == RecordSource::Teacher).cloned().collect::<Vec<_>>();
    let mut held_all = teacher(&held_live);
    for (_, rs) in &held_scene {
        held_all.extend(teacher(rs));
    }
    let held_out = json!({
        "live_teacher": table(&teacher(&held_live)),
        "live_alternative_mode": table(&held_live.iter().filter(|r| r.source == RecordSource::AlternativeMode).cloned().collect::<Vec<_>>()),
        "scenes": held_scene.iter().map(|(m, rs)| json!({"scene": m.name(), "rows": table(&teacher(rs))})).collect::<Vec<_>>(),
        "all_teacher": table(&held_all),
    });

    // 6. Competence on the held-out scene seeds, three arms on identical starts.
    let two_driver = driver_of(&two, &theta2, "two-layer");
    let plain_driver = driver_of(&plain, &theta_p, "plain");
    let arms: [(&str, Option<&EpisodeDriver>); 3] = [("heuristic", None), ("two_layer", Some(&two_driver)), ("plain", Some(&plain_driver))];
    let mut competence = Vec::new();
    for &m in &lineage_scenes {
        let mut row = serde_json::Map::new();
        row.insert("scene".into(), json!(m.name()));
        row.insert("intent".into(), json!(m.intent()));
        for (label, d) in arms {
            let runs: Vec<_> = each(&scene_heldout, |&s| scenes::competence(&scene_cfg, m, s, d))
                .into_iter()
                .filter_map(|r| r.map_err(|why| eprintln!("# competence {} {label}: {why}", m.name())).ok())
                .collect();
            row.insert(label.into(), json!({
                "competence": mean(runs.iter().map(|r| r.competence)),
                "mode_share": mean(runs.iter().map(|r| r.mode_share)),
                "deaths": runs.iter().map(|r| r.deaths).sum::<u64>(),
                "energy_change": mean(runs.iter().map(|r| r.energy_end - r.energy_start)),
                "null_gap_max": runs.iter().map(|r| r.null_gap).fold(0.0, f64::max),
                "runs": runs,
            }));
        }
        competence.push(serde_json::Value::Object(row));
    }
    eprintln!("# competence {:.0} s", t0.elapsed().as_secs_f64());

    // 7. Live closed loop on matched seeds.
    let mut live_arms = serde_json::Map::new();
    if closed_s > 0 {
        let closed_seeds = seeds(&args, "closed_seeds", "21,22");
        for (label, d) in arms {
            let rows: Vec<serde_json::Value> = closed_seeds
                .iter()
                .zip(each(&closed_seeds, |&s| closed(s, f, bodies, d, closed_s)))
                .filter_map(|(s, r)| match r {
                    Ok(v) => Some(v),
                    Err(why) => {
                        eprintln!("# skipped live closed-loop seed {s}: {why}");
                        skipped.push(json!({"what": format!("live closed loop {label}"), "seed": s, "why": why}));
                        None
                    }
                })
                .collect();
            live_arms.insert(label.into(), json!(rows));
        }
        eprintln!("# live closed loop {:.0} s", t0.elapsed().as_secs_f64());
    }

    let confusion = (num(&args, "confusion", 0u8) > 0).then(|| {
        let held_teacher = teacher(&held_live);
        let live = per_mode(&two, &arb, &theta2, &held_teacher, names.len());
        if num(&args, "dump", 0u8) > 0 {
            use std::io::Write;
            let mut channels = vec![String::new(); two.observation];
            for m in f.manifest().modules.iter() {
                for k in 0..m.width {
                    if let Some(c) = channels.get_mut(m.offset + k) {
                        *c = format!("{}.{}", m.name, m.channels.get(k).copied().unwrap_or("?"));
                    }
                }
            }
            std::fs::write(out.join("obs_channels.json"), json!({"intents": names, "channels": channels}).to_string()).expect("channels");
            let mut w = std::io::BufWriter::new(std::fs::File::create(out.join("heldout_picks.jsonl")).expect("dump file"));
            for &[r, t, label, pick] in &live.picks {
                let s = &held_teacher[r].samples[t];
                let before = if t > 0 { Some(held_teacher[r].samples[t - 1].held.intent) } else { None };
                let row = json!({"record": r, "t": t, "label": label, "pick": (pick != usize::MAX).then_some(pick), "before": before, "obs": s.observation});
                writeln!(w, "{row}").expect("dump");
            }
        }
        let fit_on = per_mode(&two, &arb, &theta2, &teacher(&train), names.len());
        println!("{}", confusion_text("held-out live teacher", &names, &live.confusion, &live.masked_labels));
        println!("{}", confusion_text("training teacher (in sample)", &names, &fit_on.confusion, &fit_on.masked_labels));
        json!({"intents": names, "held_out_live": live.confusion, "held_out_live_masked": live.masked_labels,
               "training": fit_on.confusion, "training_masked": fit_on.masked_labels})
    });

    let summary = json!({
        "founder": f.name(),
        "bodies": bodies, "fullness": fullness, "also": also.iter().map(|t| json!({"founder": t.founder.name(), "bodies": t.bodies, "fullness": t.fullness})).collect::<Vec<_>>(), "scenario": scenario_path, "hound_every_s": hounds.0, "hound_m": hounds.1, "record_s": record_s, "closed_s": closed_s, "alt_period_s": alt_period_s, "alt_len_s": alt_len_s,
        "scene_s": scene_cfg.duration_s, "scene_seeds": scene_seeds, "scene_heldout": scene_heldout, "refit_rounds": refit_rounds,
        "train_labels": train_labels,
        "samples": {"train_teacher": samples(&train, RecordSource::Teacher), "train_alternative_mode": samples(&train, RecordSource::AlternativeMode),
                    "train_scene": samples(&train_scene, RecordSource::Teacher), "heldout_teacher": held_all.iter().map(|r| r.samples.len()).sum::<usize>()},
        "fit": {"updates": spec.updates, "two_layer_last": report2.last, "plain_last": report_p.last},
        "refit": rounds,
        "held_out": held_out,
        "competence": competence,
        "live_closed_loop": live_arms,
        "confusion": confusion,
        "skipped": skipped,
        "wall_s": t0.elapsed().as_secs_f64(),
    });
    std::fs::write(out.join("summary.json"), serde_json::to_string_pretty(&summary).expect("json")).expect("summary");
    // The per-mode table: held-out agreement and action error, competence by arm.
    println!(
        "{:16} {:>7} {:>7} {:>7} {:>9} {:>9} | {:>6} {:>6} {:>6} | {:>6} {:>6}",
        "intent", "labels", "agree2", "switch2", "mse2", "mseP", "compH", "comp2", "compP", "shareH", "share2"
    );
    let rows = summary["held_out"]["all_teacher"].as_array().cloned().unwrap_or_default();
    for r in rows {
        let intent = r["intent"].as_str().unwrap_or("?");
        let comp = summary["competence"].as_array().and_then(|c| c.iter().find(|c| c["intent"] == intent)).cloned();
        let c = |arm: &str| comp.as_ref().and_then(|c| c[arm]["competence"].as_f64()).map_or("-".into(), |x| format!("{x:.2}"));
        let share = |arm: &str| comp.as_ref().and_then(|c| c[arm]["mode_share"].as_f64()).map_or("-".into(), |x| format!("{x:.2}"));
        let g = |k: &str| r[k].as_f64().map_or("-".into(), |x| format!("{x:.4}"));
        println!(
            "{:16} {:>7} {:>7} {:>7} {:>9} {:>9} | {:>6} {:>6} {:>6} | {:>6} {:>6}",
            intent,
            r["labels"],
            g("agreement_two_layer"),
            g("switch_agreement_two_layer"),
            g("action_mse_two_layer"),
            g("action_mse_plain"),
            c("heuristic"),
            c("two_layer"),
            c("plain"),
            share("heuristic"),
            share("two_layer")
        );
    }
}
