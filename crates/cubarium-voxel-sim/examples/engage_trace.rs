//! **An encounter's trace** (VH-FIX step 1): one staged episode of the engagement harness
//! (`config/engagement.toml`), printed every `every=` seconds — each hound's place, distance to
//! each family member, held intent, tactical memory and stamina; each family member's place,
//! speed and steering; and every pack event as it happens. A diagnostic, not a readout.
//!
//! `cargo run --release -p cubarium-voxel-sim --example engage_trace -- arm=calf-3 seed=1`
use cubarium_voxel_fauna::intent::{LOFTSTRIDER, VEILHOUND};
use cubarium_voxel_sim::engagement::{self, Config};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |key: &str| args.iter().find_map(|a| a.strip_prefix(key));
    let mut cfg = Config::read(std::path::Path::new(get("config=").unwrap_or("config/engagement.toml")))?;
    if let Some(s) = get("seconds=") {
        cfg.duration_s = s.parse().map_err(|e| format!("{e}"))?;
    }
    let seed: u64 = get("seed=").unwrap_or("1").parse().map_err(|e| format!("{e}"))?;
    let every: f64 = get("every=").unwrap_or("1").parse().map_err(|e| format!("{e}"))?;
    let arm = cfg.arms.iter().find(|a| a.name == get("arm=").unwrap_or("calf-3")).ok_or("unknown arm")?.clone();
    let mut ep = engagement::found(&cfg, seed, &arm)?;
    let hz = f64::from(cubarium_voxel_fauna::TICK_HZ);
    let stride = (every * hz).round().max(1.0) as u64;
    let family: Vec<u64> = ep.adults.iter().copied().chain(ep.calf).collect();
    let mut last: std::collections::BTreeMap<u64, (f64, f64)> = Default::default();
    let mut n = 0u64;
    loop {
        let go = ep.step();
        n += 1;
        let fauna = ep.sim.fauna();
        let view = fauna.view();
        let t = view.tick as f64 / hz;
        if n % stride == 0 || !go {
            for &id in &family {
                let Some(a) = view.animal(id) else {
                    println!("{t:7.1} prey {id} gone");
                    continue;
                };
                let (px, pz) = last.get(&id).copied().unwrap_or((a.pose.x, a.pose.z));
                let sp = ((a.pose.x - px).powi(2) + (a.pose.z - pz).powi(2)).sqrt() / every;
                last.insert(id, (a.pose.x, a.pose.z));
                println!(
                    "{t:7.1} prey {id} body {:.2} at ({:.1},{:.1}) hd {:.0} v {sp:.2} steer {:?} defend {} wound {:.2} held {:?}",
                    a.body,
                    a.pose.x,
                    a.pose.z,
                    a.pose.heading_rad.to_degrees(),
                    fauna.steer_of(id),
                    LOFTSTRIDER.name(usize::from(fauna.percept(id).map_or(0, |p| p.vigil.defend_held))),
                    a.wound,
                    a.founder_state.held
                );
            }
            for &id in &ep.hounds {
                let Some(a) = view.animal(id) else { continue };
                let (px, pz) = last.get(&id).copied().unwrap_or((a.pose.x, a.pose.z));
                let sp = ((a.pose.x - px).powi(2) + (a.pose.z - pz).powi(2)).sqrt() / every;
                last.insert(id, (a.pose.x, a.pose.z));
                let ds: Vec<String> = family
                    .iter()
                    .filter_map(|&f| view.animal(f).map(|b| format!("{f}:{:.1}", ((b.pose.x - a.pose.x).powi(2) + (b.pose.z - a.pose.z).powi(2)).sqrt())))
                    .collect();
                let m = fauna.hound(id).copied().unwrap_or_default();
                println!(
                    "{t:7.1} hound {id} at ({:.1},{:.1}) v {sp:.2} d [{}] intent {} tgt {:?} com {} test {:?} best {:.1} prog {:.1}s ago spent {:.2} winded {} strike {:?} held {:?} vel ({:.2},{:.2})",
                    a.pose.x,
                    a.pose.z,
                    ds.join(" "),
                    VEILHOUND.name(usize::from(a.intent.held)),
                    m.target,
                    m.committed,
                    m.test,
                    m.best_m,
                    view.tick.saturating_sub(m.progress_tick) as f64 / hz,
                    a.founder_state.sprint_spent,
                    a.founder_state.winded,
                    a.strike.map(|s| (s.phase, s.prey)),
                    a.founder_state.held,
                    m.vel[0],
                    m.vel[1],
                );
                for (pid, v) in fauna.hound_values(id) {
                    println!(
                        "{t:7.1}   values {id}->{pid}: value {:.3} meat {:.2} (raw {:.2}, usable {:.2}) p_one {:.3} p_kill {:.3} chase {:.2} kicks {:.2} risk {:.3} young {} guards {} facing {} hounds {} group {}",
                        v.value, v.meat, v.meat_raw, v.usable, v.p_one, v.p_kill, v.chase_cost, v.expected_kicks, v.risk, v.young, v.guards, v.facing, v.hounds, v.in_group
                    );
                }
            }
        }
        if !go {
            break;
        }
    }
    println!("{}", serde_json::to_string(&ep.row()).map_err(|e| format!("{e}"))?);
    Ok(())
}
