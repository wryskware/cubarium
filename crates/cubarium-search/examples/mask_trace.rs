//! **Why a teacher label is masked** (LS-TEACH step 2): the `two_layer_clone` world stepped
//! under the heuristics with the tap on; every decision sample of the lineage whose proposal
//! (the top's label) its own intent mask rules out, counted by its steering layer, whether
//! it is a fresh label or one carried from the sample before, and what its percept record
//! holds of threats after the sample (tracks of the threat class: how many, the freshest's
//! age, whether sight carries it) and of its ground response (fleeing, the alarm it heeds).
//! The valid labels of the same intents are counted beside them.
//!
//! ```text
//! mask_trace founder=loftstrider seeds=9 seconds=600 bodies=8 [also=LINEAGE=N[:FULLNESS],...] \
//!     [scenario=PATH] [hound_every_s=S hound_m=M] [intents=watch,guard] [lines=1]
//! ```

use cubarium_search::es::voxel::live::{ecology::EcologyParams, world};
use cubarium_voxel_fauna::perception::class;
use std::collections::{BTreeMap, HashMap};

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    cubarium_voxel::world::set_thread_override(1);
    let f = cubarium_search::es::voxel::parse_founder(arg(&args, "founder").unwrap_or("loftstrider"))?;
    let seeds: Vec<u64> = arg(&args, "seeds").unwrap_or("9").split(',').filter_map(|s| s.parse().ok()).collect();
    let seconds: u64 = arg(&args, "seconds").and_then(|v| v.parse().ok()).unwrap_or(600);
    let bodies: usize = arg(&args, "bodies").and_then(|v| v.parse().ok()).unwrap_or(8);
    let also: Vec<world::TopUp> = arg(&args, "also")
        .map(|v| v.split(',').filter(|x| !x.is_empty()).map(|x| world::TopUp::parse(x).expect("also")).collect())
        .unwrap_or_default();
    let scenario = arg(&args, "scenario").map(|p| cubarium_voxel_sim::edit::Scenario::read(std::path::Path::new(p))).transpose()?;
    let lines = arg(&args, "lines").is_some();
    // LS-MOVEOFF: what the body is doing at the sample (speed, state, rest bout, eating latch,
    // the nearest threat track's range) beside its record.
    let detail = arg(&args, "detail").is_some();
    let dump = arg(&args, "dump").is_some();
    let top: usize = arg(&args, "top").and_then(|v| v.parse().ok()).unwrap_or(14);
    let menu = f.manifest().menu;
    let want: Vec<usize> = arg(&args, "intents")
        .unwrap_or("watch")
        .split(',')
        .map(|n| menu.index(n).ok_or_else(|| format!("no intent {n}")))
        .collect::<Result<_, _>>()?;
    let per_s = u64::from(cubarium_voxel_fauna::TICK_HZ);
    let stagger = 2 * cubarium_voxel_fauna::FaunaConfig::default().perception.stagger_ticks();
    let mut counts: BTreeMap<(String, bool, String), usize> = BTreeMap::new();
    for seed in seeds {
        let eco = EcologyParams::defaults();
        let (mut sim, _) = world::try_found_world(&world::training_config(), seed, eco.fauna().founder_counts(), &eco)?;
        for t in &also {
            world::top_up(&mut sim, *t);
        }
        world::top_up(&mut sim, world::TopUp { founder: f, bodies, fullness: None });
        if let Some(s) = &scenario {
            let r = sim.edit(|e| s.apply(e));
            eprintln!("# seed {seed}: scenario {:?} refused {:?}", r.applied, r.refused);
        }
        sim.fauna_mut().start_tap();
        let hound_every: u64 = arg(&args, "hound_every_s").and_then(|v| v.parse().ok()).unwrap_or(0);
        let hound_m: f64 = arg(&args, "hound_m").and_then(|v| v.parse().ok()).unwrap_or(8.0);
        // (id, tick) -> what its record holds after the sample.
        let mut seen: HashMap<(u64, u64), String> = HashMap::new();
        for tick in 0..seconds * per_s {
            if hound_every > 0 && tick % per_s == 0 && tick > 0 && (tick / per_s) % hound_every == 0 {
                if let Err(why) = world::show_hunter(&mut sim, f, cubarium_voxel_fauna::Founder::Chorister, tick / per_s / hound_every, hound_m) {
                    eprintln!("# no Veilhound shown: {why}");
                }
            }
            sim.step();
            let fauna = sim.fauna();
            let now = fauna.tick();
            for a in fauna.view().animals.iter().filter(|a| a.founder == Some(f) && a.intent.sample_tick == now) {
                let p = fauna.percept(a.id);
                let threats: Vec<_> = p.map(|p| p.tracks().iter().filter(|t| t.known && t.class == class::THREAT).copied().collect()).unwrap_or_default();
                let age = threats.iter().map(|t| now.saturating_sub(t.tick)).min();
                let age_band = match age {
                    None => "none".to_string(),
                    Some(a) if a <= stagger => "fresh".to_string(),
                    Some(a) => format!("{}s", (a as f64 / per_s as f64).ceil()),
                };
                let near = threats.iter().map(|t| t.d_m).fold(f64::INFINITY, f64::min);
                let near_band = if near.is_finite() { format!("{}m", (near / 10.0).floor() * 10.0) } else { "-".to_string() };
                let sight = threats.iter().any(|t| t.modes & cubarium_voxel_fauna::perception::SIGHT != 0);
                let v = p.map(|p| p.vigil).unwrap_or_default();
                let heed = fauna.config().founder(f).calls.heed_s;
                // The freshest call it heard and still heeds by age: its kind and voice.
                let call = cubarium_voxel_fauna::calls::heard(fauna, a.id)
                    .iter()
                    .filter(|r| heed.get(usize::from(r.kind)).is_some_and(|&h| now.saturating_sub(r.tick) as f64 * cubarium_voxel::DT <= h))
                    .max_by_key(|r| r.tick)
                    .map_or("none".to_string(), |r| format!("{}{}", ["alarm", "distress"].get(usize::from(r.kind)).unwrap_or(&"?"), ["", "-mother", "-own_young"][usize::from(r.relation).min(2)]));
                if dump && detail && v.calm_walk && a.speed_m_s <= 0.1 {
                    let t = threats.iter().min_by(|p, q| p.d_m.total_cmp(&q.d_m));
                    eprintln!(
                        "DUMP seed={seed} id={} tick={now} x={:.1} z={:.1} y={:.1} head={:.0} spd={:.2} state={:?} mode={:?} still={} held_fwd={:.2} att={:.3} del={:.3} delf={:.3} threat={:?}",
                        a.id, a.pose.x, a.pose.z, a.pose.y_m, a.pose.heading_rad.to_degrees(), a.speed_m_s, a.state, a.mobility.mode, a.still_ticks, a.founder_state.held.forward, a.founder_state.feedback.attempted_equivalent, a.founder_state.feedback.delivered_equivalent, a.founder_state.feedback.delivered_forward,
                        t.map(|t| (t.id, (t.x * 10.0).round() / 10.0, (t.z * 10.0).round() / 10.0, (t.d_m * 10.0).round() / 10.0, t.moving))
                    );
                }
                let body = if detail {
                    format!(
                        " pressed={} stood={} calm_walk={} moving={} state={:?} rest_bout={} eating={} still_s={} near_threat={near_band}",
                        fauna.press_of(a.id).0 > 0,
                        a.still_ticks >= fauna.config().predation.still_after_ticks(),
                        v.calm_walk,
                        a.speed_m_s > 0.1,
                        a.state,
                        a.rest_ticks > 0,
                        a.eating,
                        (a.still_ticks as f64 / per_s as f64).min(30.0).floor() as u64 / 10 * 10,
                    )
                } else {
                    String::new()
                };
                seen.insert(
                    (a.id, now),
                    format!(
                        "threat_tracks={} freshest={age_band} sight={sight} fleeing={} bolt={} scan={} heeding={} call={call} young={}",
                        threats.len(),
                        v.fleeing,
                        now < v.bolt_until,
                        v.scanning(now),
                        v.heard_from != 0,
                        a.body < fauna.config().founder(f).core.birth_body
                    ) + &body,
                );
            }
        }
        let tap = sim.fauna_mut().take_tap().ok_or("tap")?;
        for (id, g, samples) in tap.streams() {
            if g != f {
                continue;
            }
            let steer: HashMap<u64, _> = samples.iter().map(|s| (s.tick, s.steer)).collect();
            let Some(ds) = tap.decisions(id) else { continue };
            let mut last: Option<u8> = None;
            for d in ds {
                let k = usize::from(d.proposal);
                let carried = last == Some(d.proposal);
                last = Some(d.proposal);
                if !want.contains(&k) {
                    continue;
                }
                let masked = !d.prep.intent_mask.get(k).copied().unwrap_or(false);
                let key = format!(
                    "steer={:?} fwd={:.1} turn={:.0} {} held_before={} held={} {}",
                    steer.get(&d.tick),
                    d.executed.actions.forward,
                    d.executed.actions.turn,
                    if carried { "carried" } else { "fresh" },
                    menu.name(usize::from(d.held_before)),
                    menu.name(usize::from(d.held)),
                    seen.get(&(id, d.tick)).map_or("?", String::as_str)
                );
                if lines && masked {
                    println!("seed={seed} id={id} tick={} {} masked {key}", d.tick, menu.name(k));
                }
                *counts.entry((menu.name(k).to_string(), masked, key)).or_default() += 1;
            }
        }
    }
    for &k in &want {
        let name = menu.name(k);
        for masked in [true, false] {
            let mut rows: Vec<_> = counts.iter().filter(|((n, m, _), _)| n == name && *m == masked).map(|((_, _, key), n)| (key, *n)).collect();
            let total: usize = rows.iter().map(|r| r.1).sum();
            println!("== {name} {} {total}", if masked { "MASKED" } else { "valid" });
            rows.sort_by(|a, b| b.1.cmp(&a.1));
            for (key, n) in rows.into_iter().take(top) {
                println!("{n:6} {key}");
            }
        }
    }
    Ok(())
}
