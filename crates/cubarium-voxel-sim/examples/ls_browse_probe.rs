//! **LS-BROWSE's probe** (Wrysk 2026-10-14, "yes on loftstriders"): the stand at the mouth over
//! each loftstrider visit to a stand. Headless; readouts only, nothing here changes a rule.
//!
//! `ls_browse_probe (config=PATH seed=N | load=DIR config=PATH) MINUTES OUT [save_at=MIN]
//! [set=K=V]... [threads=N] [water_every=N]`
//!
//! - With `load=`, the save's fauna config is replaced by `config=` with the `set=`s
//!   (`Fauna::replace_config`), so a save made without a knob runs with it.
//! - `save_at=MIN` writes a save of the world at that minute to `OUT/save`.
//!
//! **A visit** is a run of ticks one body crops ([`State::Cropping`]) within `ZONE_BL` of its
//! lengths of where the run began, ended by a crop farther off or `GAP_S` without one. Its
//! mouth may take from several stands in a visit (the induced defence moves the bite to the
//! next one). For each visit: the stands bitten, the sum of their reachable edible foliage
//! ([`browse_mouth`]) when first bitten (before the bite) and when last bitten (after it) and of
//! their full values, the bites' mean strip factor, the crop ticks, the stomach's fill (organic
//! over capacity) at both ends, the eating latch at the end, the leave rule's give-ups in the
//! lineage meanwhile, its trailing offer and remembered intake at the end. Writes
//! `OUT/meals.csv`, `OUT/summary.json`, `OUT/DONE`.
//!
//! **Latch episodes** (`OUT/latch.csv`): per grown loftstrider, each run of its eating latch
//! on (a meal, latch on → off at the full event) or off (full, off → on). `complete` is true
//! when both ends fell inside the run. For each: wall and awake seconds, the stomach's fill
//! and the reserve's fraction of its cap at both ends, and `swallowed`, the stomach pools'
//! rises summed tick by tick (what entered the stomach after the spill, net of that tick's
//! digestion). **Per minute** (`OUT/minutes.csv`): the lineage's foliage eaten (cumulative
//! ledger), and over grown bodies the count, awake, eating, mean fill and mean reserve
//! fraction.
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use cubarium_voxel_fauna::{BrowseMouth, Founder, State, TICK_HZ, browse_mouth, intent, roam};
use cubarium_voxel_sim::{
    Sim, SimConfig,
    found::{LayerConfig, Seeds, found},
    params::{Layers, Params},
};
use serde_json::json;

const GAP_S: f64 = 30.0;
const ZONE_BL: f64 = 2.0;

#[derive(Clone)]
struct Visit {
    body: u64,
    calf: bool,
    at: (f64, f64),
    start_tick: u64,
    last_tick: u64,
    crops: u64,
    /// Each stand bitten: (edible when first bitten, before; edible when last bitten, after; full).
    stands: BTreeMap<u64, (f64, f64, f64)>,
    strip_sum: f64,
    fill_start: f64,
    fill_end: f64,
    eating_end: bool,
    give_ups_start: u64,
}

impl Visit {
    fn sums(&self) -> (f64, f64, f64) {
        self.stands.values().fold((0.0, 0.0, 0.0), |s, v| (s.0 + v.0, s.1 + v.1, s.2 + v.2))
    }
}

fn wrap(d: f64, ring: f64) -> f64 {
    let d = d.rem_euclid(ring);
    if d > 0.5 * ring { d - ring } else { d }
}

fn fill(fauna: &cubarium_voxel_fauna::Fauna, a: &cubarium_voxel_fauna::Animal) -> f64 {
    let cap = a.founder.map_or(0.0, |f| fauna.config().founder(f).gut.capacity(a.body));
    if cap > 0.0 { a.stomach.organic() / cap } else { 0.0 }
}

fn reserve_frac(fauna: &cubarium_voxel_fauna::Fauna, a: &cubarium_voxel_fauna::Animal) -> f64 {
    let cap = a.founder.map_or(0.0, |f| fauna.config().founder(f).core.reserve_of(a.body));
    if cap > 0.0 { a.reserve / cap } else { 0.0 }
}

/// One run of a grown body's eating latch in one state.
struct Episode {
    meal: bool,
    observed_start: bool,
    start_tick: u64,
    awake_s: f64,
    fill_0: f64,
    reserve_0: f64,
    swallowed: f64,
    stomach: f64,
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    cubarium_voxel_sim::experiment_args(&args);
    let get = |key: &str| args.iter().find_map(|a| a.strip_prefix(key));
    let pos: Vec<&String> = args.iter().filter(|a| !a.contains('=')).collect();
    if pos.len() < 2 {
        return Err("usage: ls_browse_probe (config=PATH seed=N | load=DIR config=PATH) MINUTES OUT [save_at=MIN] [set=K=V]...".into());
    }
    let minutes: f64 = pos[0].parse().map_err(|e| format!("{e}"))?;
    let out = Path::new(pos[1]);
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let save_at: Option<f64> = get("save_at=").map(str::parse).transpose().map_err(|e| format!("{e}"))?;
    let sets: Vec<String> = args.iter().filter_map(|a| a.strip_prefix("set=")).map(str::to_string).collect();
    let file = std::fs::read_to_string(get("config=").ok_or("config= required")?).map_err(|e| e.to_string())?.parse::<toml::Table>().map_err(|e| e.to_string())?;
    let p = Params::resolve(&Layers { file: Some(file), sets: sets.clone(), ..Layers::default() })?;
    let (mut sim, start_min) = if let Some(dir) = get("load=") {
        let mut s = cubarium_voxel_sim::saved::load(Path::new(dir), &[])?;
        s.fauna.replace_config(p.fauna.clone())?;
        cubarium_voxel_sim::saved::restore(&mut s.fauna, &[])?;
        (Sim::ambient(s.world, s.flora, s.fauna, SimConfig::experiment()), s.minute as f64)
    } else {
        let seed: u64 = get("seed=").unwrap_or("1").parse().map_err(|e| format!("{e}"))?;
        let f = found(&p.world, &LayerConfig::new(&p, p.world.voxel_m), p.fauna.founder_counts(), Seeds::from(seed), false);
        (Sim::ambient(f.world, f.flora, f.fauna, SimConfig::experiment()), 0.0)
    };
    let hz = f64::from(TICK_HZ);
    let ring = f64::from(sim.world().config().width) * sim.world().config().voxel_m;
    let limit = (minutes * 60.0 * hz).round() as u64;
    let gap = (GAP_S * hz).round() as u64;
    let save_tick = save_at.map(|m| ((m - start_min).max(0.0) * 60.0 * hz).round() as u64);
    let menu = intent::menu_of(Founder::Loftstrider);
    let mut meals = std::fs::File::create(out.join("meals.csv")).map_err(|e| e.to_string())?;
    writeln!(
        meals,
        "body,calf,start_min,dur_s,crops,stands,reach_edible_0,reach_edible_1,reach_full,share_0,share_1,strip_mean,fill_0,fill_1,eating_end,give_ups,offer_end,memory_end"
    )
    .map_err(|e| e.to_string())?;
    let mut latch = std::fs::File::create(out.join("latch.csv")).map_err(|e| e.to_string())?;
    writeln!(latch, "body,kind,complete,start_min,end_min,wall_s,awake_s,fill_0,fill_1,reserve_0,reserve_1,swallowed").map_err(|e| e.to_string())?;
    let mut per_min = std::fs::File::create(out.join("minutes.csv")).map_err(|e| e.to_string())?;
    writeln!(per_min, "minute,eaten_foliage,grown,awake,eating,fill_mean,reserve_mean").map_err(|e| e.to_string())?;
    let mut episodes: BTreeMap<u64, Episode> = BTreeMap::new();
    let mut open: BTreeMap<u64, Visit> = BTreeMap::new();
    let mut done: Vec<Visit> = Vec::new();
    let mut time: BTreeMap<String, f64> = BTreeMap::new();
    let mut asleep_s = 0.0;
    let started = std::time::Instant::now();
    let give_ups = |sim: &Sim| sim.fauna().forage_tally()[Founder::Loftstrider.index()].give_ups;
    let close = |v: Visit, sim: &Sim, meals: &mut std::fs::File, done: &mut Vec<Visit>| -> Result<(), String> {
        let stays = sim.fauna().forage_stays();
        let offer = stays.iter().find(|s| s.0 == v.body).and_then(|s| s.2);
        let memory = sim.fauna().forage_memory(v.body);
        let share = |e: f64, f: f64| if f > 0.0 { e / f } else { 1.0 };
        let (e0, e1, full) = v.sums();
        writeln!(
            meals,
            "{},{},{:.2},{:.1},{},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.3},{:.3},{},{},{},{}",
            v.body,
            v.calf,
            start_min + v.start_tick as f64 / hz / 60.0,
            (v.last_tick - v.start_tick) as f64 / hz,
            v.crops,
            v.stands.len(),
            e0,
            e1,
            full,
            share(e0, full),
            share(e1, full),
            v.strip_sum / v.crops.max(1) as f64,
            v.fill_start,
            v.fill_end,
            v.eating_end,
            give_ups(sim) - v.give_ups_start,
            offer.map_or(String::new(), |o| format!("{o:.3}")),
            memory.map_or(String::new(), |m| format!("{m:.3}")),
        )
        .map_err(|e| e.to_string())?;
        done.push(v);
        Ok(())
    };
    let write_ep = |latch: &mut std::fs::File, body: u64, e: &Episode, complete: bool, end_tick: u64, fill_1: f64, reserve_1: f64| -> Result<(), String> {
        writeln!(
            latch,
            "{},{},{},{:.2},{:.2},{:.1},{:.1},{:.3},{:.3},{:.3},{:.3},{:.3}",
            body,
            if e.meal { "meal" } else { "full" },
            complete,
            start_min + e.start_tick as f64 / hz / 60.0,
            start_min + end_tick as f64 / hz / 60.0,
            (end_tick - e.start_tick) as f64 / hz,
            e.awake_s,
            e.fill_0,
            fill_1,
            e.reserve_0,
            reserve_1,
            e.swallowed,
        )
        .map_err(|e| e.to_string())
    };
    for tick in 1..=limit {
        // Before the step: the stand each eating loftstrider's mouth would bite.
        let mut before: BTreeMap<u64, (BrowseMouth, f64)> = BTreeMap::new();
        {
            let fauna = sim.fauna();
            let view = sim.world().view();
            let fv = sim.flora().view();
            for a in fauna.view().animals.iter().filter(|a| a.founder == Some(Founder::Loftstrider) && a.eating) {
                if let Some(m) = browse_mouth(&view, &fv, fauna.config(), a) {
                    before.insert(a.id, (m, fill(fauna, a)));
                }
            }
        }
        sim.step();
        let fauna = sim.fauna();
        let view = sim.world().view();
        let fv = sim.flora().view();
        let mut ended = Vec::new();
        for a in fauna.view().animals.iter().filter(|a| a.founder == Some(Founder::Loftstrider)) {
            let calf = roam::young_of_mother(fauna.config(), a);
            if !calf {
                let stomach = a.stomach.organic();
                let (f1, r1) = (fill(fauna, a), reserve_frac(fauna, a));
                let fresh = |observed_start: bool| Episode {
                    meal: a.eating,
                    observed_start,
                    start_tick: tick,
                    awake_s: 0.0,
                    fill_0: f1,
                    reserve_0: r1,
                    swallowed: 0.0,
                    stomach,
                };
                match episodes.get_mut(&a.id) {
                    Some(e) if e.meal == a.eating => {
                        e.swallowed += (stomach - e.stomach).max(0.0);
                        e.stomach = stomach;
                    }
                    Some(e) => {
                        write_ep(&mut latch, a.id, e, e.observed_start, tick, f1, r1)?;
                        let mut n = fresh(true);
                        if a.eating {
                            n.swallowed = (stomach - e.stomach).max(0.0);
                        }
                        episodes.insert(a.id, n);
                    }
                    None => {
                        episodes.insert(a.id, fresh(false));
                    }
                }
                if a.state != State::Asleep {
                    if let Some(e) = episodes.get_mut(&a.id) {
                        e.awake_s += 1.0 / hz;
                    }
                }
            }
            if !calf {
                if a.state == State::Asleep {
                    asleep_s += 1.0 / hz;
                } else {
                    let len = cubarium_voxel_fauna::body_of(fauna.config(), a).map_or(1.0, |b| b.length_m);
                    let moving = a.speed_m_s > 0.1 * len;
                    let name = menu.named.get(usize::from(a.intent.held)).map_or("none", |c| c.name);
                    let key = format!("{name}|{}|{}", if moving { "moving" } else { "still" }, if a.eating { "eating" } else { "sated" });
                    *time.entry(key).or_default() += 1.0 / hz;
                }
            }
            let cropped = a.state == State::Cropping;
            let pre = before.get(&a.id).copied();
            if let (true, Some((m0, f0))) = (cropped, pre) {
                let after = browse_mouth(&view, &fv, fauna.config(), a).filter(|m| m.stand == m0.stand).unwrap_or(m0);
                let len = cubarium_voxel_fauna::body_of(fauna.config(), a).map_or(1.0, |b| b.length_m);
                let near = |p: (f64, f64)| wrap(a.pose.x - p.0, ring).hypot(a.pose.z - p.1) <= ZONE_BL * len;
                let fresh = |calf| Visit {
                    body: a.id,
                    calf,
                    at: (a.pose.x, a.pose.z),
                    start_tick: tick,
                    last_tick: tick,
                    crops: 0,
                    stands: BTreeMap::new(),
                    strip_sum: 0.0,
                    fill_start: f0,
                    fill_end: f0,
                    eating_end: a.eating,
                    give_ups_start: give_ups(&sim),
                };
                let v = match open.remove(&a.id) {
                    Some(v) if near(v.at) => v,
                    Some(v) => {
                        ended.push(v);
                        fresh(calf)
                    }
                    None => fresh(calf),
                };
                let mut v = v;
                v.last_tick = tick;
                v.crops += 1;
                v.strip_sum += m0.strip;
                v.stands.entry(m0.stand).or_insert((m0.reach_edible, after.reach_edible, m0.reach_full)).1 = after.reach_edible;
                v.fill_end = fill(fauna, a);
                v.eating_end = a.eating;
                open.insert(a.id, v);
            } else if let Some(v) = open.get_mut(&a.id) {
                v.eating_end = a.eating;
                if tick - v.last_tick > gap {
                    ended.push(open.remove(&a.id).expect("open"));
                }
            }
        }
        for v in ended {
            close(v, &sim, &mut meals, &mut done)?;
        }
        if save_tick == Some(tick) {
            let minute = (start_min + tick as f64 / hz / 60.0).round() as u64;
            cubarium_voxel_sim::saved::save(&out.join("save"), sim.world(), sim.flora(), sim.fauna(), minute)?;
        }
        if tick % (60 * TICK_HZ as u64) == 0 {
            let fauna = sim.fauna();
            let grown: Vec<_> = fauna.view().animals.iter().filter(|a| a.founder == Some(Founder::Loftstrider) && !roam::young_of_mother(fauna.config(), a)).collect();
            let n = grown.len().max(1) as f64;
            writeln!(
                per_min,
                "{:.0},{:.3},{},{},{},{:.3},{:.3}",
                start_min + tick as f64 / hz / 60.0,
                fauna.view().ledger.eaten_by_founder_food[Founder::Loftstrider.index()][cubarium_voxel_fauna::Food::Foliage.index()],
                grown.len(),
                grown.iter().filter(|a| a.state != State::Asleep).count(),
                grown.iter().filter(|a| a.eating).count(),
                grown.iter().map(|a| fill(fauna, a)).sum::<f64>() / n,
                grown.iter().map(|a| reserve_frac(fauna, a)).sum::<f64>() / n,
            )
            .map_err(|e| e.to_string())?;
            std::fs::write(out.join("status"), format!("minute={:.0} wall_s={:.0} visits={}\n", start_min + tick as f64 / hz / 60.0, started.elapsed().as_secs_f64(), done.len()))
                .map_err(|e| e.to_string())?;
        }
    }
    for v in std::mem::take(&mut open).into_values() {
        close(v, &sim, &mut meals, &mut done)?;
    }
    {
        let fauna = sim.fauna();
        for (id, e) in &episodes {
            let (f1, r1) = fauna.view().animals.iter().find(|a| a.id == *id).map_or((f64::NAN, f64::NAN), |a| (fill(fauna, a), reserve_frac(fauna, a)));
            write_ep(&mut latch, *id, e, false, limit, f1, r1)?;
        }
    }
    let awake: f64 = time.values().sum();
    let t = sim.fauna().forage_tally()[Founder::Loftstrider.index()];
    let grown: Vec<&Visit> = done.iter().filter(|v| !v.calf).collect();
    let mean = |f: &dyn Fn(&Visit) -> f64| if grown.is_empty() { 0.0 } else { grown.iter().map(|v| f(v)).sum::<f64>() / grown.len() as f64 };
    let share = |e: f64, f: f64| if f > 0.0 { e / f } else { 1.0 };
    let eaten = sim.fauna().view().ledger.eaten_by_founder_food[Founder::Loftstrider.index()][cubarium_voxel_fauna::Food::Foliage.index()];
    let r = json!({
        "sets": sets, "start_min": start_min, "minutes": minutes,
        "asleep_h": asleep_s / 3600.0, "awake_h": awake / 3600.0,
        "awake_share": time.iter().map(|(k, v)| (k.clone(), (v / awake.max(1e-9) * 1000.0).round() / 1000.0)).collect::<BTreeMap<_, _>>(),
        "eating_share": time.iter().filter(|(k, _)| k.ends_with("|eating")).map(|(_, v)| v).sum::<f64>() / awake.max(1e-9),
        "forage": {"stays": t.stays, "stay_s": t.stay_s, "stay_max_s": t.stay_max_s, "offer_samples": t.offer_samples,
            "offer_mean": if t.offer_samples > 0 { t.offer_sum / t.offer_samples as f64 } else { 0.0 }, "offer_low": t.offer_low,
            "give_ups": t.give_ups, "leave_samples": t.leave_samples},
        "visits_grown": grown.len(),
        "visit_mean_dur_s": mean(&|v| (v.last_tick - v.start_tick) as f64 / hz),
        "visit_mean_share_0": mean(&|v| { let (e0, _, f) = v.sums(); share(e0, f) }),
        "visit_mean_share_1": mean(&|v| { let (_, e1, f) = v.sums(); share(e1, f) }),
        "visit_mean_stands": mean(&|v| v.stands.len() as f64),
        "visits_ended_eating": grown.iter().filter(|v| v.eating_end).count(),
        "foliage_eaten_by_loftstriders": eaten,
        "wall_s": started.elapsed().as_secs_f64(),
    });
    let mut w = std::fs::File::create(out.join("summary.json")).map_err(|e| e.to_string())?;
    w.write_all(serde_json::to_string_pretty(&r).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::write(out.join("DONE"), "").map_err(|e| e.to_string())?;
    Ok(())
}
