//! LJ-HUNT probe: what a lanternjaw does with its time, and what comes within its reach.
//!
//! ```text
//! cargo run --release -p cubarium --example lj_hunt_probe -- load=DIR minutes=5 [threads=6] [set=PATH=VALUE ...]
//! ```
//!
//! Loads a saved run as `voxel_census load=` does (default heuristics), steps `minutes` and
//! reads every free lanternjaw on its controller samples. On stdout, per stage (juvenile,
//! adult):
//! - `samples` and the share of them by steer layer and hunt mode;
//! - `lit` (lure held lit), `eat` (eating latch on), `feed` (feed held), `move` (forward
//!   held over 0.05), `striking` (a strike in progress);
//! - per prey lineage: samples with an eligible body ([`is_prey_of`]) inside the strike
//!   reach (`(jaw_forward + strike_reach) × length` from the centre) and inside 2 m, and
//!   samples with a listed body inside 2 m that the size window refuses;
//! - the mean body and the window (`prey_max_ratio × body`).
//!
//! Then the ledger over the window: the lanternjaw's funnel per prey (sensed, approached,
//! struck, killed, eaten), strikes, hits, dodges, the patrol's counters, the lure's visits
//! and kills, and the bellwing samples spent steering to a lure.
//!
//! Measurement only: it changes nothing in the run.

use cubarium::voxel::install_founders_with;
use cubarium_observatory::query::{LifeStage, stage_of};
use cubarium_voxel_fauna::{BeaconKind, Founder, TICK_HZ, is_prey_of, window_size};
use cubarium_voxel_sim::{Sim, SimConfig};
use std::collections::BTreeMap;

#[derive(Default)]
struct Acc {
    samples: f64,
    lit: f64,
    eat: f64,
    feed: f64,
    moving: f64,
    striking: f64,
    body: f64,
    by_mode: BTreeMap<String, f64>,
    meal_eat: f64,
    meal_bite: f64,
    meal_smear: f64,
    meal_carrion: f64,
    reach: [f64; Founder::COUNT],
    capsule: [f64; Founder::COUNT],
    why: BTreeMap<&'static str, f64>,
    near: [f64; Founder::COUNT],
    too_big: [f64; Founder::COUNT],
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |k: &str| args.iter().find_map(|a| a.strip_prefix(k).map(str::to_string));
    let dir = get("load=").expect("load=DIR");
    let minutes: f64 = get("minutes=").map_or(5.0, |s| s.parse().expect("minutes="));
    let threads: usize = get("threads=").map_or(6, |s| s.parse().expect("threads="));
    cubarium_voxel::set_thread_override(threads);

    let driven = cubarium::voxel::policy_founders(&[]);
    let saved = cubarium_voxel_sim::saved::load(std::path::Path::new(&dir), &driven).unwrap_or_else(|e| panic!("load: {e}"));
    let (world, mut flora, mut fauna, minute) = (saved.world, saved.flora, saved.fauna, saved.minute);
    let sets: Vec<String> = args.iter().filter_map(|a| a.strip_prefix("set=").map(str::to_string)).collect();
    if !sets.is_empty() {
        let loaded = cubarium_voxel_sim::params::Params::over_loaded(world.config().clone(), flora.config(), fauna.config().clone(), &sets)
            .unwrap_or_else(|e| panic!("the parameter set: {e}"));
        flora.replace_config(loaded.flora.clone()).unwrap_or_else(|e| panic!("set= flora: {e}"));
        fauna.replace_config(loaded.params.fauna.clone()).unwrap_or_else(|e| panic!("set= fauna: {e}"));
        eprintln!("set {sets:?}");
    }
    install_founders_with(&mut fauna, &[]).expect("default heuristic controllers");
    let lj = Founder::Lanternjaw;
    let bw = Founder::Bellwing;
    let count = |f: &cubarium_voxel_fauna::Fauna, s: LifeStage| {
        f.view().animals.iter().filter(|a| a.founder == Some(lj) && stage_of(f.config(), a) == s).count()
    };
    eprintln!(
        "loaded {dir} at minute {minute}: lanternjaws riders {} juveniles {} adults {}",
        count(&fauna, LifeStage::Rider),
        count(&fauna, LifeStage::Juvenile),
        count(&fauna, LifeStage::Adult)
    );
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    let l0 = sim.fauna().view().ledger.clone();
    let cadence = lj.manifest().cadence_ticks();
    let bw_cadence = bw.manifest().cadence_ticks();
    let ticks = (minutes * 60.0 * TICK_HZ as f64).round() as u64;
    let mut acc: BTreeMap<&'static str, Acc> = BTreeMap::new();
    // Strikes in progress: striker -> (prey id, prey lineage, prey wound at the start).
    let mut open: BTreeMap<u64, (u64, Founder, f64)> = BTreeMap::new();
    // Per prey lineage: strikes begun, prey wounded, prey gone (killed) at the strike's end.
    let mut outcomes: BTreeMap<&'static str, [u64; 3]> = BTreeMap::new();
    let mut bw_samples = 0.0;
    let mut bw_to_lure = 0.0f64;
    let mut bw_near_lit = 0.0;
    let mut bw_lure_d = [0.0f64; 5];
    let mut bw_lure_dy = 0.0f64;
    let mut bw_lure_steer = 0.0f64;
    let mut bw_lure_mode: BTreeMap<String, f64> = BTreeMap::new();
    for _ in 0..ticks {
        sim.step();
        let fauna = sim.fauna();
        let config = fauna.config();
        let wv = sim.world().view();
        let ring = f64::from(wv.config.width) * wv.config.voxel_m;
        let animals = fauna.view().animals;
        for a in animals.iter().filter(|a| a.founder == Some(lj)) {
            match (a.strike, open.get(&a.id).copied()) {
                (Some(st), None) => {
                    if let Some(o) = fauna.view().animal(st.prey)
                        && let Some(pf) = o.founder
                    {
                        open.insert(a.id, (st.prey, pf, o.wound));
                        outcomes.entry(pf.name()).or_default()[0] += 1;
                    }
                }
                (None, Some((prey, pf, w0))) => {
                    open.remove(&a.id);
                    match fauna.view().animal(prey) {
                        None => outcomes.entry(pf.name()).or_default()[2] += 1,
                        Some(o) if o.wound > w0 + 1e-12 => outcomes.entry(pf.name()).or_default()[1] += 1,
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        // Each tick of a strike on a bellwing: the strike phase, where the bellwing is against
        // the capsule, how it steers and whether it sees a loom.
        for a in animals.iter().filter(|a| a.founder == Some(lj)) {
            let Some(st) = a.strike else { continue };
            let Some(o) = fauna.view().animal(st.prey) else {
                eprintln!("strike t={} lj={} prey={} gone", fauna.view().tick, a.id, st.prey);
                continue;
            };
            if o.founder != Some(bw) {
                continue;
            }
            let phys = config.founder(lj);
            let h = phys.active_hunter().expect("a hunter");
            let g = phys.body_at(a.body);
            let jaw = (
                a.pose.x + h.jaw_forward_length_fraction * g.length_m * a.pose.heading_rad.sin(),
                a.pose.z + h.jaw_forward_length_fraction * g.length_m * a.pose.heading_rad.cos(),
            );
            let tip = (
                jaw.0 + h.strike_reach_length_fraction * g.length_m * a.pose.heading_rad.sin(),
                jaw.1 + h.strike_reach_length_fraction * g.length_m * a.pose.heading_rad.cos(),
            );
            let og = config.founder(bw).body_at(o.body);
            let off = segment_distance(jaw, tip, (o.pose.x, o.pose.z), ring) - g.footprint_radius() - og.footprint_radius();
            let loom = fauna.percept(o.id).map_or(0.0, |p| p.light.looming);
            eprintln!(
                "strike t={} lj={} body={:.3} phase={:?} left={} bw={} mode={:?} steer={:?} off_capsule={:.3} bw_y={:.3} lj_y={:.3} band={:.3} loom={:.2} bw_speed={:.2} lure={}",
                fauna.view().tick, a.id, a.body, st.phase, st.ticks_left, o.id, o.mobility.mode, fauna.steer_of(o.id), off,
                o.pose.y_m, a.pose.y_m, h.strike_height_m(&g), loom, o.speed_m_s, st.lure
            );
        }
        let lit_lures: Vec<(f64, f64)> =
            animals.iter().filter(|a| a.founder == Some(lj) && a.attached.is_none() && a.founder_state.lure > 0.0).map(|a| (a.pose.x, a.pose.z)).collect();
        for a in animals {
            if a.founder == Some(bw) && a.age_ticks % bw_cadence == 0 {
                bw_samples += 1.0;
                if let Some(p) = fauna.percept(a.id)
                    && p.light.beacon
                    && p.light.beacon_kind == BeaconKind::Lure as u8
                {
                    bw_to_lure += 1.0;
                    let (dx, dz) = wrapped(a.pose.x, a.pose.z, p.light.beacon_x, p.light.beacon_z, ring);
                    let d = dx.hypot(dz);
                    let bin = [0.5, 1.0, 3.0, 10.0].iter().position(|&b| d <= b).unwrap_or(4);
                    bw_lure_d[bin] += 1.0;
                    bw_lure_dy += a.pose.y_m - p.light.beacon_y;
                    if fauna.steer_of(a.id) == Some(cubarium_voxel_fauna::Steer::Light) {
                        bw_lure_steer += 1.0;
                    }
                    *bw_lure_mode.entry(format!("{:?}", a.mobility.mode)).or_default() += 1.0;
                }
                if lit_lures.iter().any(|&(x, z)| {
                    let (dx, dz) = wrapped(a.pose.x, a.pose.z, x, z, ring);
                    dx.hypot(dz) <= 3.0
                }) {
                    bw_near_lit += 1.0;
                }
            }
            if a.founder != Some(lj) || a.age_ticks % cadence != 0 {
                continue;
            }
            let stage = stage_of(config, a);
            let name = match stage {
                LifeStage::Juvenile => "juvenile",
                LifeStage::Adult => "adult",
                _ => continue,
            };
            let phys = config.founder(lj);
            let Some(h) = phys.active_hunter() else { continue };
            let g = phys.body_at(a.body);
            let reach = (h.jaw_forward_length_fraction + h.strike_reach_length_fraction) * g.length_m;
            let e = acc.entry(name).or_default();
            e.samples += 1.0;
            e.body += a.body;
            let held = a.founder_state.held;
            e.lit += f64::from(u8::from(a.founder_state.lure > 0.0));
            e.eat += f64::from(u8::from(a.eating));
            e.feed += f64::from(u8::from(held.feed > 0.0));
            e.moving += f64::from(u8::from(held.forward > 0.05));
            e.striking += f64::from(u8::from(a.strike.is_some()));
            // The most carrion under a carcass record on its own face or the two ahead.
            let best = {
                let fv = sim.flora().view();
                let v = wv.config.voxel_m;
                let mut best = 0.0f64;
                for k in 0..3 {
                    let d = f64::from(k) * v;
                    let x = ((a.pose.x + d * a.pose.heading_rad.sin()) / v).floor() as i64;
                    let z = ((a.pose.z + d * a.pose.heading_rad.cos()) / v).floor() as i64;
                    if z < 0 {
                        continue;
                    }
                    let site = cubarium_voxel_flora::Site { x: x.rem_euclid(i64::from(wv.config.width)) as u32, y: a.site.y, z: z as u32 };
                    if let Some(g) = fv.ground_at(site)
                        && g.carcass.is_some()
                    {
                        best = best.max(g.carrion);
                    }
                }
                best
            };
            if a.hunt.mode == cubarium_voxel_fauna::HuntMode::Meal && held.feed > 0.0 && held.forward == 0.0 {
                e.meal_eat += 1.0;
                e.meal_bite += f64::from(u8::from(a.state == cubarium_voxel_fauna::State::Cropping));
                e.meal_carrion += best;
                e.meal_smear += f64::from(u8::from(best < config.teachers.feed_gate));
            }
            let key = format!("{:?}/{:?}", fauna.steer_of(a.id), a.hunt.mode);
            *e.by_mode.entry(key).or_default() += 1.0;
            let jaw = (
                a.pose.x + h.jaw_forward_length_fraction * g.length_m * a.pose.heading_rad.sin(),
                a.pose.z + h.jaw_forward_length_fraction * g.length_m * a.pose.heading_rad.cos(),
            );
            let tip = (
                jaw.0 + h.strike_reach_length_fraction * g.length_m * a.pose.heading_rad.sin(),
                jaw.1 + h.strike_reach_length_fraction * g.length_m * a.pose.heading_rad.cos(),
            );
            let mut in_capsule = [false; Founder::COUNT];
            let mut in_reach = [false; Founder::COUNT];
            let mut near = [false; Founder::COUNT];
            let mut too_big = [false; Founder::COUNT];
            for o in animals {
                let Some(pf) = o.founder else { continue };
                if o.id == a.id || !h.hunts(pf) || o.attached.is_some() {
                    continue;
                }
                let (dx, dz) = wrapped(a.pose.x, a.pose.z, o.pose.x, o.pose.z, ring);
                let d = dx.hypot(dz);
                if d > 2.0 {
                    continue;
                }
                if is_prey_of(config, a, o) {
                    near[pf.index()] = true;
                    if d <= reach {
                        in_reach[pf.index()] = true;
                    }
                    let og = config.founder(pf).body_at(o.body);
                    if segment_distance(jaw, tip, (o.pose.x, o.pose.z), ring) <= g.footprint_radius() + og.footprint_radius() {
                        in_capsule[pf.index()] = true;
                    }
                } else if window_size(config, o) > h.prey_max_ratio * phys.size(a.body) {
                    too_big[pf.index()] = true;
                }
            }
            if in_capsule.iter().any(|&c| c) {
                let why = if a.strike.is_some() {
                    "striking"
                } else if !a.eating {
                    "latch_off"
                } else if held.feed <= 0.0 {
                    "no_feed"
                } else if best > 0.0 && config.hunt.meal_first {
                    "carcass_at_mouth"
                } else {
                    "ready"
                };
                *e.why.entry(why).or_default() += 1.0;
            }
            for k in 0..Founder::COUNT {
                e.capsule[k] += f64::from(u8::from(in_capsule[k]));
                e.reach[k] += f64::from(u8::from(in_reach[k]));
                e.near[k] += f64::from(u8::from(near[k]));
                e.too_big[k] += f64::from(u8::from(too_big[k]));
            }
        }
    }
    let fauna = sim.fauna();
    let config = fauna.config();
    let h = config.founder(lj).active_hunter().expect("a hunter");
    for (name, e) in &acc {
        let n = e.samples.max(1.0);
        println!(
            "stage={name} samples={} body={:.3} window={:.3} lit={:.3} eat={:.3} feed={:.3} move={:.3} striking={:.3}",
            e.samples,
            e.body / n,
            h.prey_max_ratio * config.founder(lj).size(e.body / n),
            e.lit / n,
            e.eat / n,
            e.feed / n,
            e.moving / n,
            e.striking / n
        );
        let me = e.meal_eat.max(1.0);
        println!(
            "  meal_eat={:.3} of samples: cropping={:.3} carrion_mean={:.5} under_feed_gate={:.3}",
            e.meal_eat / n,
            e.meal_bite / me,
            e.meal_carrion / me,
            e.meal_smear / me
        );
        let whys: Vec<String> = e.why.iter().map(|(k, v)| format!("{k}={v}")).collect();
        println!("  samples with an eligible prey in the capsule, by state: {}", whys.join(" "));
        let modes: Vec<String> = e.by_mode.iter().map(|(k, v)| format!("{k}={:.3}", v / n)).collect();
        println!("  modes {}", modes.join(" "));
        for f in Founder::ALL {
            if h.hunts(f) {
                let k = f.index();
                println!(
                    "  prey={} in_capsule={:.4} in_reach={:.4} within_2m={:.4} too_big_2m={:.4}",
                    f.name(),
                    e.capsule[k] / n,
                    e.reach[k] / n,
                    e.near[k] / n,
                    e.too_big[k] / n
                );
            }
        }
    }
    let l1 = fauna.view().ledger;
    let k = lj.index();
    println!(
        "ledger lanternjaw strikes={} hits={} dodges={} lure_kills_bw={} lure_kills_cap={}",
        l1.strikes_by_founder[k] - l0.strikes_by_founder[k],
        l1.hits_by_founder[k] - l0.hits_by_founder[k],
        l1.dodges_by_founder[k] - l0.dodges_by_founder[k],
        l1.lure_kills[bw.index()] - l0.lure_kills[bw.index()],
        l1.lure_kills[Founder::Capgnawer.index()] - l0.lure_kills[Founder::Capgnawer.index()],
    );
    for f in Founder::ALL {
        let p = f.index();
        let d: Vec<u64> = (0..5).map(|s| l1.funnel[k][p][s] - l0.funnel[k][p][s]).collect();
        let enc = l1.hunt_encounter_s_by_prey[p] - l0.hunt_encounter_s_by_prey[p];
        let dec = l1.hunt_declined_s_by_prey[p] - l0.hunt_declined_s_by_prey[p];
        if d.iter().any(|&x| x > 0) || enc > 0.0 {
            println!("  funnel prey={} sensed/approached/struck/killed/eaten={:?} encounter_s={enc:.0} declined_s={dec:.0}", f.name(), d);
        }
    }
    println!(
        "patrol wait={} travel={} engaged={} fruitless={} meal_walk={} meal_eat={} withheld={} chosen={} explored={}",
        l1.hunt_wait_samples - l0.hunt_wait_samples,
        l1.hunt_travel_samples - l0.hunt_travel_samples,
        l1.hunt_engaged_samples - l0.hunt_engaged_samples,
        l1.hunt_fruitless - l0.hunt_fruitless,
        l1.hunt_meal_walk_samples - l0.hunt_meal_walk_samples,
        l1.hunt_meal_eat_samples - l0.hunt_meal_eat_samples,
        l1.hunt_strikes_withheld - l0.hunt_strikes_withheld,
        l1.hunt_chosen - l0.hunt_chosen,
        l1.hunt_explored - l0.hunt_explored,
    );
    for (k, v) in &outcomes {
        println!("strikes on {k}: begun={} wounded={} killed={}", v[0], v[1], v[2]);
    }
    let lure = BeaconKind::Lure as usize;
    for f in [bw, Founder::Capgnawer] {
        let p = f.index();
        println!(
            "lure visits by {}: {} failed {}",
            f.name(),
            l1.beacon_visits[p][lure] - l0.beacon_visits[p][lure],
            l1.failed_visits[p][lure] - l0.failed_visits[p][lure]
        );
    }
    println!("bellwing samples={bw_samples} steering_to_lure={bw_to_lure} within_3m_of_lit_lure={bw_near_lit}");
    println!(
        "  lure-led bellwings: steer=Light {:.3}; horizontal d <=0.5/1/3/10/more {:?}; mean dy {:.2} m; modes {:?}",
        bw_lure_steer / bw_to_lure.max(1.0),
        bw_lure_d,
        bw_lure_dy / bw_to_lure.max(1.0),
        bw_lure_mode
    );
}

/// The shortest `(dx, dz)` from one point to another on the ring (x wraps, z does not).
fn wrapped(fx: f64, fz: f64, tx: f64, tz: f64, ring: f64) -> (f64, f64) {
    let mut dx = (tx - fx).rem_euclid(ring);
    if dx > ring / 2.0 {
        dx -= ring;
    }
    (dx, tz - fz)
}

/// The horizontal distance from `p` to the segment `a → b` on the ring.
fn segment_distance(a: (f64, f64), b: (f64, f64), p: (f64, f64), ring: f64) -> f64 {
    let (bx, bz) = wrapped(a.0, a.1, b.0, b.1, ring);
    let (px, pz) = wrapped(a.0, a.1, p.0, p.1, ring);
    let len2 = bx * bx + bz * bz;
    let t = if len2 > 0.0 { ((px * bx + pz * bz) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (px - t * bx).hypot(pz - t * bz)
}
