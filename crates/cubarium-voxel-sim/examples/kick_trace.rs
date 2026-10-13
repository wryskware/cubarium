//! **Who kicks which Veilhound, and why it was there** (VH-KICKED step 1): a saved run loaded
//! (`load=DIR`, a census checkpoint), stepped under the heuristics to `to=` minutes past the
//! load; every landed or missed kick on a hound is printed as it resolves (kicker, its
//! defence intent and steering, kick source, the hound's intent, pup or not, its wound before
//! and after, the distances), every hound death, and every `every=` seconds each hound within
//! `near=` m of a loftstrider. A diagnostic, not a readout.
//!
//! ```text
//! kick_trace load=DIR [from=0] [to=60] [every=5] [near=8] [threads=8] [save=DIR save_at=MIN]
//! ```

use cubarium_voxel_fauna::Founder;
use cubarium_voxel_fauna::intent::{LOFTSTRIDER, VEILHOUND};
use cubarium_voxel_fauna::kick::Phase;
use std::collections::BTreeMap;

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = arg(&args, "load").ok_or("load=DIR")?;
    let from: f64 = arg(&args, "from").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let to: f64 = arg(&args, "to").and_then(|v| v.parse().ok()).unwrap_or(60.0);
    let every: f64 = arg(&args, "every").and_then(|v| v.parse().ok()).unwrap_or(5.0);
    let near: f64 = arg(&args, "near").and_then(|v| v.parse().ok()).unwrap_or(8.0);
    let threads: usize = arg(&args, "threads").and_then(|v| v.parse().ok()).unwrap_or(8);
    let save: Option<(&str, f64)> = arg(&args, "save").map(|d| (d, arg(&args, "save_at").and_then(|v| v.parse().ok()).expect("save_at=MIN past the load")));
    let mut saved_once = false;
    let wall = std::time::Instant::now();
    cubarium_voxel::world::set_thread_override(threads);
    let saved = cubarium_voxel_sim::saved::load(std::path::Path::new(dir), &[])?;
    let minute0 = saved.minute;
    let mut sim = cubarium_voxel_sim::Sim::ambient(saved.world, saved.flora, saved.fauna, cubarium_voxel_sim::SimConfig::experiment());
    let hz = f64::from(cubarium_voxel_fauna::TICK_HZ);
    let stride = (every * hz).round().max(1.0) as u64;
    let t0 = sim.fauna().view().tick;
    let (ring, vm) = {
        let c = sim.world().config();
        (f64::from(c.width) * c.voxel_m, c.voxel_m)
    };
    let gap = |ax: f64, az: f64, bx: f64, bz: f64| {
        let wrap = |d: f64| d - ring * (d / ring).round();
        let (dx, dz) = (wrap(bx - ax), bz - az);
        (dx * dx + dz * dz).sqrt()
    };
    // Each kicker's phase last tick; each hound's wound last tick.
    let mut phase: BTreeMap<u64, (Phase, u64, cubarium_voxel_fauna::kick::Source)> = BTreeMap::new();
    let mut wound: BTreeMap<u64, f64> = BTreeMap::new();
    let mut hound_line: BTreeMap<u64, String> = BTreeMap::new();
    // At each windup's start: what the kicker held and saw of its target.
    let mut asked: BTreeMap<u64, String> = BTreeMap::new();
    let mut n = 0u64;
    loop {
        sim.step();
        n += 1;
        let fauna = sim.fauna();
        let view = fauna.view();
        let minute = minute0 as f64 + view.tick.saturating_sub(t0) as f64 / hz / 60.0;
        if minute - minute0 as f64 > to {
            break;
        }
        if let Some((d, at)) = save.filter(|_| !saved_once).filter(|(_, at)| minute - minute0 as f64 >= *at) {
            let m = minute0 + at.round() as u64;
            cubarium_voxel_sim::saved::save(std::path::Path::new(d), sim.world(), sim.flora(), fauna, m)?;
            eprintln!("saved {d} at minute {m} after {:.0} s", wall.elapsed().as_secs_f64());
            saved_once = true;
        }
        if n % (60 * hz as u64) == 0 {
            eprintln!("minute {minute:.1} wall {:.0} s", wall.elapsed().as_secs_f64());
        }
        let show = minute - minute0 as f64 >= from;
        let ls: Vec<_> = view.animals.iter().filter(|a| a.founder == Some(Founder::Loftstrider)).collect();
        let vh: Vec<_> = view.animals.iter().filter(|a| a.founder == Some(Founder::Chorister)).collect();
        let hound_desc = |h: &cubarium_voxel_fauna::Animal| {
            let pup = h.pack.is_some_and(|m| m.pup);
            let den = h.pack.and_then(|m| fauna.pack(m.pack)).map(|p| {
                gap(h.pose.x, h.pose.z, (f64::from(p.den.x) + 0.5) * vm, (f64::from(p.den.z) + 0.5) * vm)
            });
            let m = fauna.hound(h.id).copied().unwrap_or_default();
            let nl = ls.iter().map(|l| (l.id, gap(h.pose.x, h.pose.z, l.pose.x, l.pose.z))).min_by(|p, q| p.1.total_cmp(&q.1));
            format!(
                "hound {} body {:.2} pup {pup} breeder {} gest {} intent {} eating {} emerg {} state {:?} wound {:.2} reserve {:.2} den_d {:.1} tgt {:?} com {} at ({:.1},{:.1}) hd {:.0} near_ls {:?} held {:?} steer {:?}",
                h.id,
                h.body,
                h.pack.and_then(|m| fauna.pack(m.pack)).is_some_and(|p| p.breeder == Some(h.id)),
                h.reproduction.escrow.is_some(),
                VEILHOUND.name(usize::from(h.intent.held)),
                h.eating,
                h.emergency,
                h.state,
                h.wound,
                h.reserve,
                den.unwrap_or(f64::NAN),
                m.target,
                m.committed,
                h.pose.x,
                h.pose.z,
                h.pose.heading_rad.to_degrees(),
                nl.map(|(i, d)| format!("{i}@{d:.1}")),
                h.founder_state.held,
                fauna.steer_of(h.id),
            )
        };
        // Kicks resolving: a kicker whose windup ended this tick.
        for l in &ls {
            let was = phase.get(&l.id).copied();
            if l.kick.phase == Phase::Windup && !matches!(was, Some((Phase::Windup, _, _))) {
                let p = fauna.percept(l.id);
                let tr = p.and_then(|p| p.track(l.kick.target)).copied();
                let th = p
                    .map(|p| cubarium_voxel_fauna::vigil::threat_readout(fauna.config(), l, p, view.tick, ring, vm))
                    .and_then(|v| v.into_iter().find(|t| t.0 == l.kick.target));
                asked.insert(
                    l.id,
                    format!(
                        "asked: defend {} src {:?} track {:?} (pressing, calm) {:?}",
                        LOFTSTRIDER.name(usize::from(p.map_or(0, |p| p.vigil.defend_held))),
                        l.kick.source,
                        tr.map(|t| format!("size {:.2} moving {} posture {} modes {}", t.size_m, t.moving, t.posture, t.modes)),
                        th.map(|t| (t.2, t.3)),
                    ),
                );
            }
            if let Some((Phase::Windup, target, source)) = was {
                if l.kick.phase == Phase::Recovery {
                    let t = view.animal(target);
                    let w0 = wound.get(&target).copied().unwrap_or(0.0);
                    let defend = LOFTSTRIDER.name(usize::from(fauna.percept(l.id).map_or(0, |p| p.vigil.defend_held)));
                    let calves = ls.iter().filter(|c| c.body < 0.8 * fauna.config().founder(Founder::Loftstrider).core.body_max && c.id != l.id).map(|c| format!("{}@{:.1}", c.id, gap(l.pose.x, l.pose.z, c.pose.x, c.pose.z))).collect::<Vec<_>>();
                    let desc = match t {
                        Some(h) if h.founder == Some(Founder::Chorister) => {
                            format!("{} dist {:.2} wound {w0:.2}->{:.2}", hound_desc(h), gap(l.pose.x, l.pose.z, h.pose.x, h.pose.z), h.wound)
                        }
                        Some(o) => format!("non-hound {} {:?}", o.id, o.founder),
                        None => format!("target {target} gone (died this tick? last line: {})", hound_line.get(&target).cloned().unwrap_or_default()),
                    };
                    if show {
                        println!(
                            "{minute:8.3} KICK by {} body {:.2} src {:?} defend {defend} steer {:?} intent {} at ({:.1},{:.1}) calves [{}] {} -> {desc}",
                            l.id,
                            l.body,
                            source,
                            fauna.steer_of(l.id),
                            LOFTSTRIDER.name(usize::from(l.intent.held)),
                            l.pose.x,
                            l.pose.z,
                            calves.join(" "),
                            asked.get(&l.id).cloned().unwrap_or_default()
                        );
                    }
                }
            }
            phase.insert(l.id, (l.kick.phase, l.kick.target, l.kick.source));
        }
        // Deaths.
        let alive: std::collections::BTreeSet<u64> = vh.iter().map(|h| h.id).collect();
        for (id, line) in &hound_line {
            if !alive.contains(id) && show {
                println!("{minute:8.3} DEAD last: {line}");
            }
        }
        hound_line.retain(|id, _| alive.contains(id));
        for h in &vh {
            wound.insert(h.id, h.wound);
            hound_line.insert(h.id, hound_desc(h));
        }
        if show && n % stride == 0 {
            for h in &vh {
                let close = ls.iter().any(|l| gap(h.pose.x, h.pose.z, l.pose.x, l.pose.z) <= near);
                if close {
                    println!("{minute:8.3}   {}", hound_desc(h));
                }
            }
            for l in &ls {
                if vh.iter().any(|h| gap(h.pose.x, h.pose.z, l.pose.x, l.pose.z) <= near) {
                    println!(
                        "{minute:8.3}   ls {} body {:.2} at ({:.1},{:.1}) defend {} steer {:?} kick {:?} wound {:.2}",
                        l.id,
                        l.body,
                        l.pose.x,
                        l.pose.z,
                        LOFTSTRIDER.name(usize::from(fauna.percept(l.id).map_or(0, |p| p.vigil.defend_held))),
                        fauna.steer_of(l.id),
                        l.kick.phase,
                        l.wound
                    );
                }
            }
        }
    }
    Ok(())
}
