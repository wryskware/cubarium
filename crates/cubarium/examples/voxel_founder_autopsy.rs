//! Founder autopsy: what kills the seeded habitat's founders in the first half hour.
//!
//! ```text
//! cargo run --release -p cubarium --example voxel_founder_autopsy -- 30 > runs/voxel-founder-autopsy.csv
//! ```
//!
//! The diagnosis package D2 of `design/handoffs/voxel-collapse-diagnosis-2026-09-20.md`.
//! The census (`design/7_Research/voxel-census-2026-09-20.md`) could see 65 bodies leave
//! and not one reason; this example builds the same world — the default authored scene,
//! `habitat::seed`, the settled live [`Senses`] field, exactly the setup
//! `examples/voxel_census.rs` uses — steps it for the given number of simulated minutes
//! (default 30) and writes every event the question needs.
//!
//! **It is read-only on the model.** Nothing here changes a birth, feeding, movement or
//! physiology rule, the seeder, or a constant; the only new thing in the crates is the
//! fauna ledger's [`Departure`] counters, which are counters.
//!
//! # What it writes
//!
//! Five record types on stdout, each its own CSV shape behind a leading tag, so one file
//! holds the whole autopsy and `grep` picks a view out of it:
//!
//! - `MIN` — one row per simulated minute per lineage: alive, cumulative born and
//!   departures by cause, mean body and reserve, cumulative bites, and how many living
//!   bodies have food inside their sensed reach.
//! - `BODY` — one row per living body per minute: its age, stores, and the distance from
//!   its pose to the nearest thing **it** can eat, against its own sensed reach.
//! - `BIRTH` — one row per newborn, at the tick it appears: its body, its reserve, the
//!   parent that paid for it and what that left the parent holding, and the newborn's
//!   distance to food.
//! - `DEATH` — one row per body that leaves: the tick, the lineage, the cause, the age it
//!   reached and what it was holding a tick earlier.
//! - `SUMMARY` — the closing totals, including the ledger's own per-cause split, which is
//!   the authoritative one.
//!
//! # The two numbers this depends on and does not import
//!
//! A founder's **sensed reach**: 1.5 m for the blind littershredder (its litter cue's
//! reach) and 2.0 m for the browser (its cone's range). `cubarium_search::es::voxel::task`'s
//! `sensed_radius_m` holds their provenance; they are restated here as constants rather
//! than depended on, because an autopsy of the ambient world must not pull in the trainer.
//!
//! # What a distance here means, and what it does not
//!
//! Planar distance on the support layer, from the body's continuous pose to the centre of
//! the nearest site holding food of its own kind — litter for the blind feeder, foliage
//! for the browser — with `x` taking the short way round the ring. It is the honest
//! "is there anything out there" measure and **not** a claim that a bite would land: the
//! mouth reaches a quarter of a body length, a browser's crown has to be touchable at its
//! own standing layer, and a cue has to diffuse to the receptor. A body whose nearest food
//! is outside this distance certainly cannot eat; one inside it may still fail.

use cubarium::voxel::VoxelConfig;
use cubarium::voxel::habitat;
use cubarium::voxel::scene;
use cubarium_voxel::VoxelView;
use cubarium_voxel_fauna::{
    Animal, Departure, Fauna, FaunaConfig, Founder, Senses, TICK_HZ, effective_config,
};
use cubarium_voxel_flora::{Flora, FloraConfig, FloraView};
use cubarium_voxel_sim::{Sim, SimConfig};

/// One simulated minute, in ticks.
const TICKS_PER_MIN: u64 = 60 * TICK_HZ as u64;

/// The blind littershredder's litter-cue reach, metres. Restated from
/// `cubarium_search::es::voxel::task::BLIND_CUE_REACH_M`, which holds its provenance.
const BLIND_REACH_M: f64 = 1.5;
/// The browser founder's cone range, metres — its manifest's `cone_range_m`, restated for
/// the same reason.
const BROWSER_REACH_M: f64 = 2.0;

/// The sensed reach of one lineage.
fn reach_m(founder: Founder) -> f64 {
    match founder {
        Founder::Blind => BLIND_REACH_M,
        Founder::Browser => BROWSER_REACH_M,
    }
}

/// What one body was holding at the end of a tick: enough to write its obituary once it
/// is gone from the view.
#[derive(Clone, Copy)]
struct Seen {
    founder: Option<Founder>,
    body: f64,
    reserve: f64,
    age_ticks: u64,
    dist_m: f64,
    deep_water: bool,
    /// Where it stood, and where it has been: the pose, and the metres of ground it has
    /// covered since it appeared. A body that pays the motor budget every tick and
    /// covers nothing is **penned**, and only the second number says so.
    x: f64,
    z: f64,
    heading_rad: f64,
    travelled_m: f64,
    /// The actions it is holding, and what the tick resolved them into: a body that
    /// **asks** to walk and covers nothing is penned by the terrain, and a body that asks
    /// for nothing is idle. The two look identical in a position trace.
    held_forward: f64,
    held_turn: f64,
    held_feed: f64,
    state: cubarium_voxel_fauna::State,
    /// How many of the four neighbouring columns it could legally step onto from where
    /// it stands: a support face at its **own** standing layer, with no more than its
    /// `wade_depth_m` of water. Zero is a body that cannot leave its column at all.
    exits: usize,
    /// Why the column one voxel **straight ahead** would refuse a step, read off the
    /// three clauses `body::advance_candidate` applies: `wall` (solid at the body layer,
    /// which the Contact receptors do report), `drop` (no support face at the body's own
    /// standing layer, which they do **not**), `wet` (deeper than `wade_depth_m`, which
    /// they do not either), or `open`.
    ahead: &'static str,
}

fn main() {
    let minutes: f64 = std::env::args().nth(1).map_or(30.0, |a| {
        a.parse().expect("usage: voxel_founder_autopsy [MINUTES]")
    });

    let cfg = VoxelConfig::default();
    let mut world = scene::authored(cfg.world.clone());
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let seeded = habitat::seed(&mut world, &mut flora, &mut fauna);
    let fauna_cfg = *fauna.config();
    eprintln!(
        "seeded: stands={} logs={} litter_tiles={} founders={:?}",
        seeded.stands, seeded.logs, seeded.litter_tiles, seeded.founders
    );
    for f in Founder::ALL {
        let sc = fauna_cfg.founder(f).core;
        let m = f.manifest();
        // The arithmetic the brief asks for, printed from the constants themselves rather
        // than restated in prose: a newborn arrives at `body_min` with `birth_cost −
        // body_min` of reserve, and dieback past `body_min` is instant death because it
        // is already there.
        let newborn_reserve = sc.birth_cost - sc.body_min;
        let upkeep_per_s = sc.maintenance_per_s * sc.body_min;
        let rest_s = newborn_reserve / upkeep_per_s;
        // The heuristic foragers hold `forward = 1` whenever nothing blocks them, so the
        // motor charge is the cruise charge; the turn term adds `r · yaw_cap` on top.
        let r = m.body_width_m / 2.0;
        let cruise_s = newborn_reserve
            / (upkeep_per_s + fauna_cfg.founder(f).motor_respiration_per_s * sc.body_min);
        let turning_eq = (m.cruise_m_per_s + r * m.yaw_cap_rad_per_s) / m.cruise_m_per_s;
        let turning_s = newborn_reserve
            / (upkeep_per_s
                + fauna_cfg.founder(f).motor_respiration_per_s * sc.body_min * turning_eq);
        eprintln!(
            "{}: newborn body={} reserve={} | rest {:.0} s, cruise {:.0} s ({:.1} m), cruise+turn {:.0} s ({:.1} m) | \
             adult body_max={} birth_body={} birth_cost={} full_reserve={}",
            f.name(),
            sc.body_min,
            newborn_reserve,
            rest_s,
            cruise_s,
            cruise_s * m.cruise_m_per_s,
            turning_s,
            turning_s * m.cruise_m_per_s,
            sc.body_max,
            sc.birth_body,
            sc.birth_cost,
            sc.reserve_of(sc.body_max),
        );
    }

    let mut senses = Senses::new();
    senses.settle(&world.view(), &flora.view());
    let mut sim = Sim::new(world, flora, fauna, SimConfig::default(), Some(senses));

    // One header line per record type, so a `grep MIN,` of this file is a CSV with its
    // own column names a line above it.
    println!(
        "HEADER,MIN,tick,minute,lineage,alive,starved,drowned,mean_body,mean_reserve,in_reach,mean_dist_m"
    );
    println!("HEADER,MINX,tick,minute,lineage,bites,assimilated,removed,max_dist_m,births_total");
    println!("HEADER,FOOD,tick,minute,litter_organic,litter_sites,foliage,foliage_stands");
    println!(
        "HEADER,BODY,tick,minute,id,lineage,age_ticks,body,reserve,dist_m,reach_m,in_reach,pose_x,pose_z,travelled_m,exits,held_forward,held_turn,held_feed,state,heading_rad,ahead"
    );
    println!(
        "HEADER,BIRTH,tick,minute,id,lineage,body,reserve,dist_m,reach_m,parent_id,parent_reserve_after"
    );
    println!(
        "HEADER,DEATH,tick,minute,id,lineage,cause,age_ticks,body,reserve,dist_m,travelled_m,exits"
    );
    println!("HEADER,SUMMARY,ticks,what,values");
    let mut prev: Vec<(u64, Seen)> = snapshot(&sim, &[]);
    for (id, s) in &prev {
        report_body(0, 0, *id, s);
    }
    report_minute(0, &sim);

    let total_ticks = (minutes * 60.0 * f64::from(TICK_HZ)) as u64;
    for tick in 1..=total_ticks {
        sim.step();
        let now = snapshot(&sim, &prev);

        // Departures: an id in the previous tick and not in this one. The **counts** by
        // cause come from the ledger and are exact; this per-body label is re-derived
        // from the last state the body was seen in, so it is a best attribution and says
        // so. It agrees with the ledger's split wherever the two can be compared.
        for (id, was) in &prev {
            if now.binary_search_by_key(id, |(i, _)| *i).is_err() {
                let sc = founder_config(&fauna_cfg, was);
                let cause = if was.deep_water && was.body >= sc.body_min {
                    Departure::Drowned
                } else {
                    Departure::Starved
                };
                println!(
                    "DEATH,{tick},{:.2},{id},{},{},{},{:.7},{:.7},{:.3},{:.3},{}",
                    tick as f64 / TICKS_PER_MIN as f64,
                    lineage(was.founder),
                    cause.name(),
                    was.age_ticks,
                    was.body,
                    was.reserve,
                    was.dist_m,
                    was.travelled_m,
                    was.exits,
                );
            }
        }

        // Births: an id in this tick and not in the last. `step::births` walks parents in
        // id order and pushes newborns in that same order with consecutive ids, so the
        // k-th body whose reserve fell by a whole `birth_cost` is the k-th newborn's
        // parent — the pairing is the tick's own order and not a guess at who is near
        // whom.
        let newborns: Vec<(u64, Seen)> = now
            .iter()
            .filter(|(i, _)| prev.binary_search_by_key(i, |(j, _)| *j).is_err())
            .copied()
            .collect();
        if !newborns.is_empty() {
            let parents: Vec<(u64, f64)> = prev
                .iter()
                .filter_map(|(id, was)| {
                    let sc = founder_config(&fauna_cfg, was);
                    let now_res = now
                        .binary_search_by_key(id, |(i, _)| *i)
                        .ok()
                        .map(|k| now[k].1.reserve)?;
                    (was.reserve - now_res >= 0.9 * sc.birth_cost).then_some((*id, now_res))
                })
                .collect();
            for (k, (id, born)) in newborns.iter().enumerate() {
                let (parent, parent_reserve) =
                    parents.get(k).copied().unwrap_or((u64::MAX, f64::NAN));
                println!(
                    "BIRTH,{tick},{:.2},{id},{},{:.7},{:.7},{:.3},{:.3},{parent},{parent_reserve:.7}",
                    tick as f64 / TICKS_PER_MIN as f64,
                    lineage(born.founder),
                    born.body,
                    born.reserve,
                    born.dist_m,
                    born.founder.map_or(f64::NAN, reach_m),
                );
            }
        }

        if tick % TICKS_PER_MIN == 0 {
            let minute = tick / TICKS_PER_MIN;
            for (id, s) in &now {
                report_body(tick, minute, *id, s);
            }
            report_minute(minute, &sim);
        }
        prev = now;
    }

    summary(&sim, total_ticks);
}

fn lineage(founder: Option<Founder>) -> &'static str {
    founder.map_or("heuristic", Founder::name)
}

/// The physiology a seen body ran under. A body with no lineage is the placeholder
/// species, which this habitat no longer seeds but the fauna crate still has.
fn founder_config(cfg: &FaunaConfig, was: &Seen) -> cubarium_voxel_fauna::SpeciesConfig {
    match was.founder {
        Some(f) => cfg.founder(f).core,
        None => *cfg.species(cubarium_voxel_fauna::Species::Frondgrazer),
    }
}

/// Every living body this tick, in id order, with the distance to what it eats.
fn snapshot(sim: &Sim, prev: &[(u64, Seen)]) -> Vec<(u64, Seen)> {
    let (world, flora, fauna) = sim.layers();
    let view = world.view();
    let fv = flora.view();
    fauna
        .view()
        .animals
        .iter()
        .map(|a| {
            let sc = effective_config(fauna.config(), a);
            let depth = view.water_depth_m(i64::from(a.site.x), a.site.y, a.site.z);
            (
                a.id,
                Seen {
                    founder: a.founder,
                    body: a.body,
                    reserve: a.reserve,
                    age_ticks: a.age_ticks,
                    dist_m: nearest_food_m(&view, &fv, a),
                    deep_water: depth > sc.drown_depth_m,
                    x: a.pose.x,
                    z: a.pose.z,
                    heading_rad: a.pose.heading_rad,
                    travelled_m: travelled(prev, a),
                    held_forward: a.founder_state.held.forward,
                    held_turn: a.founder_state.held.turn,
                    held_feed: a.founder_state.held.feed,
                    state: a.state,
                    exits: exits(&view, a, &sc),
                    ahead: ahead(&view, a, &sc),
                },
            )
        })
        .collect()
}

/// Planar metres from a body's pose to the nearest site holding food **of its own kind**,
/// `x` the short way round the ring. Infinite when the world holds none.
fn nearest_food_m(view: &VoxelView<'_>, fv: &FloraView<'_>, a: &Animal) -> f64 {
    let voxel_m = view.config.voxel_m;
    let mut best = f64::INFINITY;
    // The blind feeder eats litter off the ground under its mouth; everything else in
    // this world eats foliage off a stand.
    if matches!(a.founder, Some(Founder::Blind)) {
        for g in fv.ground {
            if g.litter > 0.0 {
                best = best.min(planar_m(view, a, g.site.x, g.site.z, voxel_m));
            }
        }
    } else {
        for s in fv.stands {
            if s.foliage > 0.0 {
                best = best.min(planar_m(view, a, s.site.x, s.site.z, voxel_m));
            }
        }
    }
    best
}

fn planar_m(view: &VoxelView<'_>, a: &Animal, sx: u32, sz: u32, voxel_m: f64) -> f64 {
    let w_m = f64::from(view.config.width) * voxel_m;
    let cx = (f64::from(sx) + 0.5) * voxel_m;
    let cz = (f64::from(sz) + 0.5) * voxel_m;
    let mut dx = (a.pose.x - cx).abs();
    if dx > w_m - dx {
        dx = w_m - dx;
    }
    let dz = a.pose.z - cz;
    (dx * dx + dz * dz).sqrt()
}

fn report_body(tick: u64, minute: u64, id: u64, s: &Seen) {
    println!(
        "BODY,{tick},{minute},{id},{},{},{:.7},{:.7},{:.3},{:.3},{},{:.3},{:.3},{:.3},{},{:.2},{:.2},{:.2},{:?},{:.3},{}",
        lineage(s.founder),
        s.age_ticks,
        s.body,
        s.reserve,
        s.dist_m,
        s.founder.map_or(f64::NAN, reach_m),
        s.founder.is_some_and(|f| s.dist_m <= reach_m(f)),
        s.x,
        s.z,
        s.travelled_m,
        s.exits,
        s.held_forward,
        s.held_turn,
        s.held_feed,
        s.state,
        s.heading_rad,
        s.ahead,
    );
}

/// Metres of ground covered since the body appeared: the previous tick's total plus this
/// tick's wrap-aware planar step. A newborn starts at zero.
fn travelled(prev: &[(u64, Seen)], a: &Animal) -> f64 {
    let Ok(k) = prev.binary_search_by_key(&a.id, |(i, _)| *i) else {
        return 0.0;
    };
    let was = prev[k].1;
    let dz = a.pose.z - was.z;
    let dx = a.pose.x - was.x;
    // A tick's step is far smaller than half the ring, so an apparent jump is the seam.
    let dx = if dx.abs() > 1.0 { 0.0 } else { dx };
    was.travelled_m + (dx * dx + dz * dz).sqrt()
}

/// Why the column one voxel along the heading refuses a step, in `advance_candidate`'s
/// own order. Naming it is the difference between "the terrain stops it" and "the
/// terrain stops it with something it cannot feel".
fn ahead(
    view: &VoxelView<'_>,
    a: &Animal,
    sc: &cubarium_voxel_fauna::SpeciesConfig,
) -> &'static str {
    let c = view.config;
    let (fx, fz) = a.pose.forward();
    let nx = (a.pose.x + fx * c.voxel_m).rem_euclid(f64::from(c.width) * c.voxel_m);
    let nz = a.pose.z + fz * c.voxel_m;
    if nz < 0.0 || nz >= f64::from(c.depth) * c.voxel_m {
        return "edge";
    }
    let cx = ((nx / c.voxel_m).floor() as i64).rem_euclid(i64::from(c.width));
    let cz = (nz / c.voxel_m).floor() as u32;
    let y = a.site.y;
    if view.material_at(cx, y + 1, cz).is_solid() {
        "wall"
    } else if !view.is_support(cx, y, cz) {
        "drop"
    } else if view.water_depth_m(cx, y, cz) > sc.wade_depth_m {
        "wet"
    } else {
        "open"
    }
}

/// The four-neighbour exits from the column a body stands in: a support face at its own
/// standing layer with water no deeper than it will wade. This is the geometry
/// `body::advance_candidate` enforces on every sub-step, read here and not changed.
fn exits(view: &VoxelView<'_>, a: &Animal, sc: &cubarium_voxel_fauna::SpeciesConfig) -> usize {
    let (w, d) = (i64::from(view.config.width), view.config.depth);
    let (x, y, z) = (i64::from(a.site.x), a.site.y, a.site.z);
    [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .filter(|(dx, dz)| {
            let nx = (x + dx).rem_euclid(w);
            let nz = z as i64 + dz;
            nz >= 0
                && (nz as u32) < d
                && view.is_support(nx, y, nz as u32)
                && view.water_depth_m(nx, y, nz as u32) <= sc.wade_depth_m
        })
        .count()
}

/// One row per lineage per minute.
fn report_minute(minute: u64, sim: &Sim) {
    let (world, flora, fauna) = sim.layers();
    let view = world.view();
    let fv = flora.view();
    let av = fauna.view();
    let l = av.ledger;
    for f in Founder::ALL {
        let mine: Vec<&Animal> = av.animals.iter().filter(|a| a.founder == Some(f)).collect();
        let n = mine.len();
        let mean = |v: f64| if n == 0 { 0.0 } else { v / n as f64 };
        let body: f64 = mine.iter().map(|a| a.body).sum();
        let reserve: f64 = mine.iter().map(|a| a.reserve).sum();
        let dists: Vec<f64> = mine.iter().map(|a| nearest_food_m(&view, &fv, a)).collect();
        let in_reach = dists.iter().filter(|d| **d <= reach_m(f)).count();
        let mean_dist = if n == 0 {
            0.0
        } else {
            dists.iter().sum::<f64>() / n as f64
        };
        let max_dist = dists.iter().copied().fold(0.0f64, f64::max);
        println!(
            "MIN,{},{minute},{},{n},{},{},{:.7},{:.7},{in_reach},{mean_dist:.3}",
            minute * TICKS_PER_MIN,
            f.name(),
            l.departed_founder(f, Departure::Starved),
            l.departed_founder(f, Departure::Drowned),
            mean(body),
            mean(reserve),
        );
        println!(
            "MINX,{},{minute},{},{},{:.7},{},{max_dist:.3},{}",
            minute * TICKS_PER_MIN,
            f.name(),
            l.bites_by_founder[f.index()],
            l.assimilated_by_founder[f.index()],
            l.departed_founder(f, Departure::Removed),
            l.births,
        );
    }
    // The litter the blind feeders live off, and the foliage the browsers do.
    let litter: f64 = fv.ground.iter().map(|g| g.litter).sum();
    let sites = fv.ground.iter().filter(|g| g.litter > 0.0).count();
    let foliage: f64 = fv.stands.iter().map(|s| s.foliage).sum();
    let stands = fv.stands.iter().filter(|s| s.foliage > 0.0).count();
    println!(
        "FOOD,{},{minute},{litter:.4},{sites},{foliage:.4},{stands}",
        minute * TICKS_PER_MIN
    );
}

fn summary(sim: &Sim, ticks: u64) {
    let l = sim.fauna().view().ledger;
    println!(
        "SUMMARY,{ticks},minutes={:.1},births={} born={} introduced={} deaths={} accounted={}",
        ticks as f64 / TICKS_PER_MIN as f64,
        l.births,
        l.born,
        l.introduced,
        l.deaths,
        l.deaths_accounted(),
    );
    for c in Departure::ALL {
        println!(
            "SUMMARY,{ticks},{},total={} littershredder={} frondgrazer={}",
            c.name(),
            l.departed(c),
            l.departed_founder(Founder::Blind, c),
            l.departed_founder(Founder::Browser, c),
        );
    }
    for f in Founder::ALL {
        println!(
            "SUMMARY,{ticks},{},bites={} assimilated={:.7}",
            f.name(),
            l.bites_by_founder[f.index()],
            l.assimilated_by_founder[f.index()],
        );
    }
    assert_eq!(
        l.deaths,
        l.deaths_accounted(),
        "the cause counters must account for every death"
    );
}
