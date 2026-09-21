//! Founder autopsy: what kills the seeded habitat's founders in the first half hour.
//!
//! ```text
//! cargo run --release -p cubarium --example voxel_founder_autopsy -- 60 generated closed \
//!     > runs/voxel-founder-autopsy.csv
//! ```
//!
//! The diagnosis package D2 of `design/handoffs/voxel-collapse-diagnosis-2026-09-20.md`.
//! The census (`design/7_Research/voxel-census-2026-09-20.md`) could see 65 bodies leave
//! and not one reason; this example builds the same world — the generated scene with the
//! closed water budget,
//! `habitat::seed`, the settled live [`Senses`] field, exactly the setup
//! `examples/voxel_census.rs` uses — steps it for the given number of simulated minutes
//! (default 60) and writes every event the question needs. Add `half` for the founder-count
//! comparison or `heuristic` for the disclosed control.
//!
//! **It is read-only on the model.** Nothing here changes a birth, feeding, movement or
//! physiology rule, the seeder, or a constant.
//!
//! The founders are driven by the built-in trained centres by default, the same drivers
//! `cubarium voxel`'s ambient run installs, so this autopsies the world that ships. A
//! trailing `heuristic` argument keeps the old, observation-only control:
//! `voxel_founder_autopsy -- 60 generated closed heuristic`.
//!
//! # What it writes
//!
//! Five record types on stdout, each its own CSV shape behind a leading tag, so one file
//! holds the whole autopsy and `grep` picks a view out of it:
//!
//! - `MIN` — one row per simulated minute per lineage: alive, cumulative born and
//!   departures by cause, mean body and reserve, cumulative bites, how many living
//!   bodies have food inside their sensed reach, and — since the reproduction round —
//!   the eggs of that lineage standing in the world and the bodies of it gestating.
//! - `REPRO` — one row per simulated minute for the whole layer: the two reproduction
//!   rules' own counters, which is where an egg that never hatched and a gestation that
//!   never reached term are read.
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
use cubarium::voxel::install_default_founders;
use cubarium::voxel::scene;
use cubarium_voxel::{Command as WorldCommand, VoxelView, World};
use cubarium_voxel_fauna::{
    Animal, Departure, Fauna, FaunaConfig, Founder, Senses, TICK_HZ, browser_cone_readings,
    browser_mouth_candidates, effective_config,
};
use cubarium_voxel_flora::{Flora, FloraConfig, FloraView, Species as Plant};
use cubarium_voxel_sim::{Sim, SimConfig};

/// One simulated minute, in ticks.
const TICKS_PER_MIN: u64 = 60 * TICK_HZ as u64;

/// The blind littershredder's litter-cue reach, metres. Restated from
/// `cubarium_search::es::voxel::task::BLIND_CUE_REACH_M`, which holds its provenance.
const BLIND_REACH_M: f64 = 1.5;
/// The browser founder's cone range, metres — its manifest's `cone_range_m`, restated for
/// the same reason.
const BROWSER_REACH_M: f64 = 2.0;

const HARNESS_RAIN_M_PER_S: f64 = 0.0002;
const CLOSED_EVAPORATION_M_PER_S: f64 = 0.0001;

#[derive(Clone, Copy)]
struct FoodProbe {
    dist_m: f64,
    species: Option<Plant>,
    foliage: f64,
    crown_in_mouth: bool,
}

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
    nearest_species: Option<Plant>,
    nearest_foliage: f64,
    nearest_crown_in_mouth: bool,
    same_height_2m: usize,
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
    let args: Vec<String> = std::env::args().collect();
    let minutes: f64 = args
        .iter()
        .skip(1)
        .find_map(|a| a.parse().ok())
        .unwrap_or(60.0);
    let generated = args.iter().any(|a| a == "generated");
    let closed = args.iter().any(|a| a == "closed");
    let half_founders = args.iter().any(|a| a == "half");
    // The shipped world's founders are driven by the built-in trained centres by
    // default, exactly as `cubarium voxel`'s ambient run installs them; `heuristic` as a
    // trailing argument keeps the old, observation-only control.
    let heuristic = args.iter().any(|a| a == "heuristic");
    assert!(
        !closed || generated,
        "closed diagnosis requires the generated world"
    );

    let cfg = VoxelConfig::default();
    let mut world = if generated {
        let mut world_cfg = cfg.world.clone();
        if closed {
            let dry = VoxelConfig::default().world;
            let basin_floor_m = World::new(dry.clone())
                .outlet_cell()
                .map_or(0.0, |(_, y, _)| f64::from(y) * dry.voxel_m);
            world_cfg = cubarium_voxel::Config {
                rain_m_per_s: HARNESS_RAIN_M_PER_S,
                evaporation_m_per_s: CLOSED_EVAPORATION_M_PER_S,
                initial_aquifer_head_m: basin_floor_m + 1.0,
                closed_water_budget: true,
                ..world_cfg
            };
        }
        let mut world = World::new(world_cfg);
        if closed {
            world.apply(WorldCommand::SetOutlet { open: true });
        }
        world
    } else {
        scene::authored(cfg.world.clone())
    };
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let counts = if half_founders { [4, 4] } else { [8, 8] };
    let seeded = habitat::seed_with_founder_counts(&mut world, &mut flora, &mut fauna, counts);
    let fauna_cfg = *fauna.config();
    eprintln!(
        "scene: {} world, {} water budget; founder counts {:?}",
        if generated { "generated" } else { "authored" },
        if closed { "closed" } else { "open" },
        counts,
    );
    eprintln!(
        "seeded: stands={} logs={} litter_tiles={} founders={:?}",
        seeded.stands, seeded.logs, seeded.litter_tiles, seeded.founders
    );
    if heuristic {
        eprintln!("founders: the observation-only heuristic (control)");
    } else {
        install_default_founders(&mut fauna).expect("the built-in centres validate");
    }
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
        "HEADER,MIN,tick,minute,lineage,alive,starved,drowned,mean_body,mean_reserve,in_reach,mean_dist_m,eggs,gestating"
    );
    println!("HEADER,MINX,tick,minute,lineage,bites,assimilated,removed,max_dist_m,births_total");
    println!("HEADER,FOOD,tick,minute,litter_organic,litter_sites,foliage,foliage_stands");
    println!(
        "HEADER,BODY,tick,minute,id,lineage,age_ticks,body,reserve,dist_m,reach_m,in_reach,nearest_species,nearest_foliage,crown_in_mouth,same_height_supports_2m,pose_x,pose_z,travelled_m,exits,held_forward,held_turn,held_feed,state,heading_rad,ahead"
    );
    println!(
        "HEADER,BIRTH,tick,minute,id,lineage,body,reserve,dist_m,reach_m,parent_id,parent_reserve_after"
    );
    println!(
        "HEADER,DEATH,tick,minute,id,lineage,cause,age_ticks,body,reserve,nearest_dist_m,nearest_species,nearest_foliage,crown_in_mouth,same_height_supports_2m,travelled_m,exits"
    );
    println!("HEADER,CAUSE,tick,minute,lineage,starved,drowned,removed");
    println!(
        "HEADER,REPRO,tick,minute,born,hatched,clutches_laid,eggs_laid,eggs_standing,eggs_lost,gestations_opened,gestations_failed"
    );
    println!(
        "HEADER,PLANT,tick,minute,species,near_foliage,near_stands,total_foliage,total_stands,bites,eaten"
    );
    println!("HEADER,REGROW,tick,minute,species,cropped_stands,regrowth_events");
    println!(
        "HEADER,CONE,tick,minute,sampled,front_left_fraction,front_fraction,front_right_fraction,front_left_proximity,front_proximity,front_right_proximity,all_sectors_nonzero"
    );
    println!("HEADER,SUMMARY,ticks,what,values");
    let mut prev: Vec<(u64, Seen)> = snapshot(&sim, &[]);
    let mut previous_causes = [[0u64; Departure::COUNT]; Founder::COUNT];
    let mut crown_history = CrownHistory::default();
    observe_crowns(&sim, &mut crown_history);
    for (id, s) in &prev {
        report_body(0, 0, *id, s);
    }
    report_minute(0, &sim, &mut previous_causes, &crown_history);

    let total_ticks = (minutes * 60.0 * f64::from(TICK_HZ)) as u64;
    for tick in 1..=total_ticks {
        sim.step();
        let now = snapshot(&sim, &prev);
        observe_crowns(&sim, &mut crown_history);

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
                    "DEATH,{tick},{:.2},{id},{},{},{},{:.7},{:.7},{:.3},{},{:.7},{},{},{:.3},{}",
                    tick as f64 / TICKS_PER_MIN as f64,
                    lineage(was.founder),
                    cause.name(),
                    was.age_ticks,
                    was.body,
                    was.reserve,
                    was.dist_m,
                    was.nearest_species.map_or("none", Plant::name),
                    was.nearest_foliage,
                    was.nearest_crown_in_mouth,
                    was.same_height_2m,
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
            report_minute(minute, &sim, &mut previous_causes, &crown_history);
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
            let food = food_probe(&view, &fv, a);
            (
                a.id,
                Seen {
                    founder: a.founder,
                    body: a.body,
                    reserve: a.reserve,
                    age_ticks: a.age_ticks,
                    dist_m: food.dist_m,
                    nearest_species: food.species,
                    nearest_foliage: food.foliage,
                    nearest_crown_in_mouth: food.crown_in_mouth,
                    same_height_2m: same_height_supports_2m(&view, a),
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

/// Planar distance and contact evidence for the nearest stand with foliage. The browser
/// reach bit is checked against the actual mouth probe columns, not the wider cone.
fn food_probe(view: &VoxelView<'_>, fv: &FloraView<'_>, a: &Animal) -> FoodProbe {
    let voxel_m = view.config.voxel_m;
    let mut best = f64::INFINITY;
    if matches!(a.founder, Some(Founder::Blind)) {
        for g in fv.ground {
            if g.litter > 0.0 {
                best = best.min(planar_m(view, a, g.site.x, g.site.z, voxel_m));
            }
        }
        return FoodProbe {
            dist_m: best,
            species: None,
            foliage: 0.0,
            crown_in_mouth: false,
        };
    }
    let mouth = browser_mouth_candidates(view, fv, a).unwrap_or_default();
    let mut nearest = None;
    for stand in fv.stands.iter().filter(|s| s.foliage > 0.0) {
        let dist = planar_m(view, a, stand.site.x, stand.site.z, voxel_m);
        if dist < best {
            best = dist;
            nearest = Some(stand);
        }
    }
    let Some(stand) = nearest else {
        return FoodProbe {
            dist_m: f64::INFINITY,
            species: None,
            foliage: 0.0,
            crown_in_mouth: false,
        };
    };
    FoodProbe {
        dist_m: planar_m(view, a, stand.site.x, stand.site.z, voxel_m),
        species: Some(stand.species),
        foliage: stand.foliage,
        crown_in_mouth: mouth.iter().any(|(site, _)| *site == stand.site),
    }
}

fn same_height_supports_2m(view: &VoxelView<'_>, a: &Animal) -> usize {
    if a.founder != Some(Founder::Browser) {
        return 0;
    }
    let radius = (BROWSER_REACH_M / view.config.voxel_m).ceil() as i64;
    let width = i64::from(view.config.width);
    let depth = view.config.depth;
    let r2 = BROWSER_REACH_M * BROWSER_REACH_M;
    let x = i64::from(a.site.x);
    let z = i64::from(a.site.z);
    (-(radius)..=radius)
        .flat_map(|dz| (-(radius)..=radius).map(move |dx| (dx, dz)))
        .filter(|(dx, dz)| {
            let distance_m = ((*dx as f64) * view.config.voxel_m).powi(2)
                + ((*dz as f64) * view.config.voxel_m).powi(2);
            distance_m <= r2 + 1e-12
        })
        .filter(|(dx, dz)| {
            let nz = z + dz;
            nz >= 0
                && (nz as u32) < depth
                && view.is_support((x + dx).rem_euclid(width), a.site.y, nz as u32)
        })
        .count()
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
        "BODY,{tick},{minute},{id},{},{},{:.7},{:.7},{:.3},{:.3},{},{},{:.7},{},{},{:.3},{:.3},{:.3},{},{:.2},{:.2},{:.2},{:?},{:.3},{}",
        lineage(s.founder),
        s.age_ticks,
        s.body,
        s.reserve,
        s.dist_m,
        s.founder.map_or(f64::NAN, reach_m),
        s.founder.is_some_and(|f| s.dist_m <= reach_m(f)),
        s.nearest_species.map_or("none", Plant::name),
        s.nearest_foliage,
        s.nearest_crown_in_mouth,
        s.same_height_2m,
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
fn report_minute(
    minute: u64,
    sim: &Sim,
    previous_causes: &mut [[u64; Departure::COUNT]; Founder::COUNT],
    crown_history: &CrownHistory,
) {
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
        let dists: Vec<f64> = mine
            .iter()
            .map(|a| food_probe(&view, &fv, a).dist_m)
            .collect();
        let in_reach = dists.iter().filter(|d| **d <= reach_m(f)).count();
        let mean_dist = if n == 0 {
            0.0
        } else {
            dists.iter().sum::<f64>() / n as f64
        };
        let max_dist = dists.iter().copied().fold(0.0f64, f64::max);
        println!(
            "MIN,{},{minute},{},{n},{},{},{:.7},{:.7},{in_reach},{mean_dist:.3},{},{}",
            minute * TICKS_PER_MIN,
            f.name(),
            l.departed_founder(f, Departure::Starved),
            l.departed_founder(f, Departure::Drowned),
            mean(body),
            mean(reserve),
            av.eggs_by_founder(f),
            av.gestating_by_founder(f),
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
        let current = std::array::from_fn(|i| l.deaths_by_founder_cause[f.index()][i]);
        println!(
            "CAUSE,{},{minute},{},{},{},{}",
            minute * TICKS_PER_MIN,
            f.name(),
            current[Departure::Starved.index()]
                .saturating_sub(previous_causes[f.index()][Departure::Starved.index()]),
            current[Departure::Drowned.index()]
                .saturating_sub(previous_causes[f.index()][Departure::Drowned.index()]),
            current[Departure::Removed.index()]
                .saturating_sub(previous_causes[f.index()][Departure::Removed.index()]),
        );
        previous_causes[f.index()] = current;
    }
    // Reproduction, once per minute for the whole layer: the two rules' own counters,
    // which are the only place an egg that never hatched or a gestation that never
    // reached term can be read (`design/handoffs/voxel-reproduction-2026-09-21.md`).
    println!(
        "REPRO,{},{minute},{},{},{},{},{},{},{},{}",
        minute * TICKS_PER_MIN,
        l.born,
        l.hatched,
        l.clutches_laid,
        l.eggs_laid,
        av.clutches.iter().map(|c| u64::from(c.count)).sum::<u64>(),
        l.eggs_lost,
        l.gestations_opened,
        l.gestations_failed,
    );
    // The litter the blind feeders live off, and the foliage the browsers do.
    let litter: f64 = fv.ground.iter().map(|g| g.litter).sum();
    let sites = fv.ground.iter().filter(|g| g.litter > 0.0).count();
    let foliage: f64 = fv.stands.iter().map(|s| s.foliage).sum();
    let stands = fv.stands.iter().filter(|s| s.foliage > 0.0).count();
    println!(
        "FOOD,{},{minute},{litter:.4},{sites},{foliage:.4},{stands}",
        minute * TICKS_PER_MIN
    );
    report_plants(minute, &view, &fv, &av);
    report_cone(minute, &view, &fv, &av);
    report_regrowth(minute, crown_history);
}

#[derive(Clone, Copy)]
struct CrownState {
    id: u64,
    species: Plant,
    last_foliage: f64,
    cropped: bool,
    regrowth_events: u64,
}

#[derive(Default)]
struct CrownHistory {
    stands: Vec<CrownState>,
}

fn observe_crowns(sim: &Sim, history: &mut CrownHistory) {
    for stand in sim.flora().view().stands {
        let Some(state) = history.stands.iter_mut().find(|s| s.id == stand.id) else {
            history.stands.push(CrownState {
                id: stand.id,
                species: stand.species,
                last_foliage: stand.foliage,
                cropped: false,
                regrowth_events: 0,
            });
            continue;
        };
        if stand.foliage + 1e-12 < state.last_foliage {
            state.cropped = true;
        } else if state.cropped && stand.foliage > state.last_foliage + 1e-12 {
            state.regrowth_events += 1;
            state.cropped = false;
        }
        state.last_foliage = stand.foliage;
    }
}

fn report_plants(
    minute: u64,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    av: &cubarium_voxel_fauna::FaunaView<'_>,
) {
    let browsers: Vec<&Animal> = av
        .animals
        .iter()
        .filter(|a| a.founder == Some(Founder::Browser))
        .collect();
    for species in Plant::ALL {
        let mut near_foliage = 0.0;
        let mut near_stands = 0usize;
        let mut total_foliage = 0.0;
        let mut total_stands = 0usize;
        for stand in fv
            .stands
            .iter()
            .filter(|s| s.species == species && s.foliage > 0.0)
        {
            total_foliage += stand.foliage;
            total_stands += 1;
            if browsers.iter().any(|a| {
                planar_m(view, a, stand.site.x, stand.site.z, view.config.voxel_m)
                    <= BROWSER_REACH_M
            }) {
                near_foliage += stand.foliage;
                near_stands += 1;
            }
        }
        let i = species.index();
        println!(
            "PLANT,{},{minute},{},{:.7},{},{:.7},{},{},{:.7}",
            minute * TICKS_PER_MIN,
            species.name(),
            near_foliage,
            near_stands,
            total_foliage,
            total_stands,
            av.ledger.bites_by_plant[i],
            av.ledger.eaten_by_plant[i],
        );
    }
}

fn report_cone(
    minute: u64,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    av: &cubarium_voxel_fauna::FaunaView<'_>,
) {
    let mut sampled = 0u64;
    let mut fractions = [0.0; 3];
    let mut proximities = [0.0; 3];
    let mut all_nonzero = 0u64;
    for animal in av
        .animals
        .iter()
        .filter(|a| a.founder == Some(Founder::Browser))
    {
        let Some(reading) = browser_cone_readings(view, fv, av, animal) else {
            continue;
        };
        sampled += 1;
        for (i, (fraction, proximity)) in reading.into_iter().enumerate() {
            fractions[i] += fraction;
            proximities[i] += proximity;
        }
        if reading.iter().all(|(fraction, _)| *fraction > 0.0) {
            all_nonzero += 1;
        }
    }
    let mean = |total: f64| {
        if sampled == 0 {
            0.0
        } else {
            total / sampled as f64
        }
    };
    println!(
        "CONE,{},{minute},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{}",
        minute * TICKS_PER_MIN,
        sampled,
        mean(fractions[0]),
        mean(fractions[1]),
        mean(fractions[2]),
        mean(proximities[0]),
        mean(proximities[1]),
        mean(proximities[2]),
        all_nonzero,
    );
}

fn report_regrowth(minute: u64, history: &CrownHistory) {
    for species in Plant::ALL {
        let cropped = history
            .stands
            .iter()
            .filter(|s| s.species == species && s.cropped)
            .count();
        let events: u64 = history
            .stands
            .iter()
            .filter(|s| s.species == species)
            .map(|s| s.regrowth_events)
            .sum();
        println!(
            "REGROW,{},{minute},{},{},{}",
            minute * TICKS_PER_MIN,
            species.name(),
            cropped,
            events,
        );
    }
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
            "SUMMARY,{ticks},{},bites={} assimilated={:.7} eggs_standing={} gestating={}",
            f.name(),
            l.bites_by_founder[f.index()],
            l.assimilated_by_founder[f.index()],
            sim.fauna().view().eggs_by_founder(f),
            sim.fauna().view().gestating_by_founder(f),
        );
    }
    println!(
        "SUMMARY,{ticks},reproduction,born={} hatched={} clutches_laid={} eggs_laid={} eggs_lost={} gestations_opened={} gestations_failed={}",
        l.born,
        l.hatched,
        l.clutches_laid,
        l.eggs_laid,
        l.eggs_lost,
        l.gestations_opened,
        l.gestations_failed,
    );
    for species in Plant::ALL {
        let i = species.index();
        println!(
            "SUMMARY,{ticks},plant_{},bites={} eaten={:.7}",
            species.name(),
            l.bites_by_plant[i],
            l.eaten_by_plant[i],
        );
    }
    assert_eq!(
        l.deaths,
        l.deaths_accounted(),
        "the cause counters must account for every death"
    );
}
