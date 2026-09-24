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
//! comparison or `heuristic` for the disclosed control, and `threads=N` to run on `N`
//! threads instead of every core (for runs side by side).
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
//! - `DROWN` — one more row per drowning (package S): whether the body was a founder or
//!   born in the run, the face, the depth the rule read there
//!   (`VoxelView::water_depth_m`) and how many cells it summed, the fill of the bottom
//!   one, whether rain fell that tick, and the same face's depth 100 ticks earlier. A
//!   rain column counted as depth reads many part-filled cells over a dry-ish bottom and
//!   was shallow a moment before; rising water fills the bottom cell and was already
//!   deepening. A drowned body cannot have stepped that tick — a step is refused past
//!   `wade_depth_m`, below `drown_depth_m`, on the same view the death clause reads — so
//!   the face is the one it was last seen on.
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
use cubarium::voxel::{install_founders_with, policy_args};
use cubarium_voxel::{Command as WorldCommand, VoxelView, World};
use cubarium_voxel_fauna::{
    Animal, ConeCensus, ConeHit, Departure, Fauna, FaunaConfig, Food, Founder, Senses, TICK_HZ,
    browser_cone_census, browser_cone_readings, browser_mouth_candidates, effective_config,
};
use cubarium_voxel_flora::{Flora, FloraConfig, FloraView, Site, Species as Plant};
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

/// First seed a `preset=` arm offers the host's lake gate. The host draws random seeds;
/// an arm has to be re-runnable, so the draws are `base`, `base + 1`, … instead. Any
/// other base is `seed=N`.
const PRESET_SEED_BASE: u64 = 1;

/// The pitch band the browser's cone actually covers: the manifest's ray pitch offsets.
/// A crown outside it at a given distance is not something a ray can reach, whatever the
/// occlusion map says.
const PITCH_BAND_DEG: f64 = 20.0;

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
    /// The support face it stood on.
    site: Site,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // `threads=N`: this run's share of the machine. The gate runs many of these side by
    // side; without it each takes every core (`cubarium_voxel::set_thread_override`).
    if let Some(n) = args
        .iter()
        .find_map(|a| a.strip_prefix("threads=").and_then(|s| s.parse().ok()))
    {
        cubarium_voxel::set_thread_override(n);
    }
    // `policy=<lineage>=<file>`: that lineage runs a saved policy instead of the
    // built-in centre (P5-C).
    let policies = policy_args(&args).expect("policy=<lineage>=<file>");
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
    // The **before** arm, on this build: every plant reduced to the one-disc lollipop
    // the model was before `design/handoffs/voxel-plant-layers-2026-09-22.md`, so an
    // arm can be compared against the layered one on the same landforms, seed, bodies
    // and centres rather than against a figure from another revision.
    let lollipop = args.iter().any(|a| a == "lollipop");
    // `preset=<small|default|wide>` is the **landscape arm**: the world the host builds,
    // not the world this file used to build for itself.
    let preset: Option<&'static cubarium_voxel::Preset> = args
        .iter()
        .find_map(|a| a.strip_prefix("preset="))
        .map(|name| {
            cubarium_voxel::Preset::find(name).unwrap_or_else(|| {
                let known: Vec<&str> = cubarium_voxel::PRESETS.iter().map(|p| p.name).collect();
                panic!("no landform preset is called {name:?}; the shipped ones are {known:?}")
            })
        });
    let seed_base: u64 = args
        .iter()
        .find_map(|a| a.strip_prefix("seed=").and_then(|s| s.parse().ok()))
        .unwrap_or(PRESET_SEED_BASE);
    assert!(
        !closed || generated || preset.is_some(),
        "closed diagnosis requires the generated world"
    );
    assert!(
        preset.is_none() || !generated,
        "a preset arm builds its own world: drop `generated closed`"
    );

    let cfg = VoxelConfig::default();
    let counts = if half_founders { [4, 4] } else { [8, 8] };
    let (world, flora, mut fauna, seeded, scene_label) = if let Some(preset) = preset {
        // Exactly what `cubarium voxel` does with no TOML, for this preset: the recipe's
        // own extents, cell size and water, the lake gate, the pre-roll with its opening
        // shower, the seeding and the acceptance check, redrawn on a refusal — through
        // `voxel::ambient_habitat`, which returns the world **already seeded**.
        let world_cfg = preset.config();
        let founded = cubarium::voxel::ambient_habitat(
            &world_cfg,
            seed_base,
            |v| {
                let flora_cfg = FloraConfig::for_voxel_size(v);
                if lollipop {
                    flora_cfg.one_layer_species()
                } else {
                    flora_cfg
                }
            },
            counts,
        );
        let world = founded.world;
        let lake = cubarium_voxel::hydrate::lake(&world);
        eprintln!(
            "scene: preset {} ({}x{}x{} at {} m, seed {}, {} lake / {} habitat rejected, {}), \
             lake {:.2} m3 over {:.1} m2 visible; flora scaled for {} m cells",
            preset.name,
            world.config().width,
            world.config().height,
            world.config().depth,
            world.config().voxel_m,
            founded.seed,
            founded.lake_rejected,
            founded.habitat_rejected,
            if founded.accepted {
                "accepted"
            } else {
                "NOT accepted"
            },
            lake.volume_m3,
            lake.visible_m2,
            world.config().voxel_m,
        );
        let label = format!("preset {} seed {}", preset.name, founded.seed);
        (world, founded.flora, founded.fauna, founded.seeded, label)
    } else {
        let world = if generated {
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
        let flora = Flora::new(if lollipop {
            FloraConfig::default().one_layer_species()
        } else {
            FloraConfig::default()
        });
        let label = format!(
            "{} world, {} water budget",
            if generated { "generated" } else { "authored" },
            if closed { "closed" } else { "open" },
        );
        let (mut world, mut flora) = (world, flora);
        let mut fauna = Fauna::new(FaunaConfig::default());
        let seeded = habitat::seed_with_founder_counts(&mut world, &mut flora, &mut fauna, counts);
        (world, flora, fauna, seeded, label)
    };
    let fauna_cfg = *fauna.config();
    eprintln!("scene: {scene_label}; founder counts {counts:?}");
    eprintln!(
        "seeded: stands={} logs={} litter_tiles={} founders={:?}",
        seeded.stands, seeded.logs, seeded.litter_tiles, seeded.founders
    );
    if lollipop {
        eprintln!("plants: one_layer_species (the pre-layers control)");
    }
    if heuristic {
        eprintln!("founders: the observation-only heuristic (control)");
    } else {
        install_founders_with(&mut fauna, &policies).expect("the founder centres validate");
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
    // The host opens a closed world's outlet here — after the layers are built and
    // before the first tick — because under a closed budget the outlet is the return
    // flow into the atmosphere, not an export (`voxel/mod.rs`). The `generated closed`
    // arm opened it before seeding and keeps doing so; a preset arm matches the host.
    if sim.world().config().closed_water_budget && !sim.world().outlet_open() {
        sim.world_mut()
            .apply(WorldCommand::SetOutlet { open: true });
        eprintln!("scene: closed water budget — the outlet is open as the return flow");
    }
    // The host also runs a viability probe here. It runs on a **clone** of the world and
    // reports only, so it cannot reach this run; it is skipped and nothing is matched by
    // skipping it.

    // One header line per record type, so a `grep MIN,` of this file is a CSV with its
    // own column names a line above it.
    println!(
        "HEADER,MIN,tick,minute,lineage,alive,starved,drowned,mean_body,mean_reserve,in_reach,mean_dist_m,eggs,gestating"
    );
    println!("HEADER,MINX,tick,minute,lineage,bites,assimilated,removed,max_dist_m,births_total");
    println!("HEADER,FOOD,tick,minute,litter_organic,litter_sites,foliage,foliage_stands");
    // What each food class of the standing stock is, and what has been bitten out of
    // it: the shredder's three detritus foods and the browser's foliage
    // (`design/handoffs/voxel-diets-2026-09-22.md`, deliverable 3).
    println!("HEADER,DIET,tick,minute,food,standing,bites,eaten");
    println!(
        "HEADER,BODY,tick,minute,id,lineage,age_ticks,body,reserve,dist_m,reach_m,in_reach,nearest_species,nearest_foliage,crown_in_mouth,same_height_supports_2m,pose_x,pose_z,travelled_m,exits,held_forward,held_turn,held_feed,state,heading_rad,ahead"
    );
    println!(
        "HEADER,BIRTH,tick,minute,id,lineage,body,reserve,dist_m,reach_m,parent_id,parent_reserve_after"
    );
    println!(
        "HEADER,DEATH,tick,minute,id,lineage,cause,age_ticks,body,reserve,nearest_dist_m,nearest_species,nearest_foliage,crown_in_mouth,same_height_supports_2m,travelled_m,exits"
    );
    println!(
        "HEADER,DROWN,tick,minute,id,lineage,origin,x,y,z,depth_m,cells,bottom_fill,raining,depth_100_ticks_earlier_m"
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
    println!(
        "HEADER,BCONE,tick,minute,id,sector,rays,clear,terrain,water,stripped,foliage,pool,body,mean_foliage_m"
    );
    println!(
        "HEADER,CONEX,tick,minute,sector,browsers,clear,terrain,water,stripped,foliage,pool,body,mean_foliage_m,blind_browsers"
    );
    println!(
        "HEADER,WANDER,tick,minute,id,travelled_m,distinct_columns,turn_share,mean_exits,cone_samples,blind_samples"
    );
    println!(
        "HEADER,DEATHCONE,tick,minute,id,cause,census_tick,clear,terrain,water,stripped,foliage,pool,body,crown_planar_m,crown_layer_offset_voxels,crown_elevation_deg,crown_in_pitch_band,probe_class,probe_m,blind_sample_share,blind_minute_share,life_travelled_m,life_columns,turn_share"
    );
    println!("HEADER,SUMMARY,ticks,what,values");
    let mut prev: Vec<(u64, Seen)> = snapshot(&sim, &[]);
    // Who was placed at the start, as against born in the run; and the last 100 ticks
    // of free water, for the `DROWN` rows.
    let founders_at_start: Vec<u64> = prev.iter().map(|(id, _)| *id).collect();
    let mut water_history = WaterHistory::default();
    water_history.observe(0, &sim.world().view());
    let mut rain_in = sim.world().view().ledger.rain_in;
    let mut wander: std::collections::HashMap<u64, Wander> = std::collections::HashMap::new();
    let mut cones: std::collections::HashMap<u64, LastCone> = std::collections::HashMap::new();
    // A body is in its terminal band when its reserve is under this many joules-equivalent
    // of resting upkeep; read off the browser's own physiology, not a tuned number.
    let terminal_reserve = {
        let sc = fauna_cfg.founder(Founder::Browser).core;
        sc.maintenance_per_s * sc.body_min * TERMINAL_UPKEEP_S
    };
    let mut previous_causes = [[0u64; Departure::COUNT]; Founder::COUNT];
    // The ledger's per-lineage cause counters as of the last tick: each tick's delta is
    // exactly what died of what, and the per-body `DEATH` label is read off it.
    let mut ledger_causes = sim.fauna().view().ledger.deaths_by_founder_cause;
    let mut crown_history = CrownHistory::default();
    observe_crowns(&sim, &mut crown_history);
    for (id, s) in &prev {
        report_body(0, 0, *id, s);
    }
    report_minute(0, &sim, &mut previous_causes, &crown_history);

    let total_ticks = (minutes * 60.0 * f64::from(TICK_HZ)) as u64;
    for tick in 1..=total_ticks {
        sim.step();
        water_history.observe(tick, &sim.world().view());
        let rained = {
            let now = sim.world().view().ledger.rain_in;
            let fell = now > rain_in;
            rain_in = now;
            fell
        };
        let now = snapshot(&sim, &prev);
        observe_crowns(&sim, &mut crown_history);
        // Wander, every tick: the ground one browser actually covers, against the ground
        // its cone can see. `travelled_m` alone cannot tell a body walking a circle from
        // one crossing the ring.
        {
            let c = sim.world().config().clone();
            for (id, s) in &now {
                if s.founder != Some(Founder::Browser) {
                    continue;
                }
                let w = wander.entry(*id).or_default();
                let col = (
                    ((s.x / c.voxel_m).floor() as i64).rem_euclid(i64::from(c.width)) as u32,
                    (s.z / c.voxel_m).floor().clamp(0.0, f64::from(c.depth - 1)) as u32,
                );
                w.columns.insert(col);
                w.life_columns.insert(col);
                w.ticks += 1;
                if s.held_turn.abs() > 0.1 {
                    w.turning_ticks += 1;
                }
                w.exits_sum += s.exits as u64;
                w.life_travelled_m = s.travelled_m;
            }
        }
        // The cone census: at the regular cadence for every browser, and **every** tick
        // for one in its terminal band, so the row printed at a death is the world that
        // body last looked at.
        {
            let regular = tick % CONE_SAMPLE_TICKS == 0;
            let (world, flora, fauna) = sim.layers();
            let view = world.view();
            let fv = flora.view();
            let av = fauna.view();
            for animal in av
                .animals
                .iter()
                .filter(|a| a.founder == Some(Founder::Browser))
            {
                let terminal = animal.reserve <= terminal_reserve;
                if !regular && !terminal {
                    continue;
                }
                let Some(cone) = last_cone(&view, &fv, &av, animal, tick) else {
                    continue;
                };
                if regular {
                    let w = wander.entry(animal.id).or_default();
                    w.samples += 1;
                    w.life_samples += 1;
                    let blank = cone
                        .per_sector
                        .iter()
                        .all(|(counts, _)| counts[ConeHit::FoliageCrown.index()] == 0);
                    if blank {
                        w.blind_samples += 1;
                        w.life_blind_samples += 1;
                    }
                }
                cones.insert(animal.id, cone);
            }
        }

        // Departures: an id in the previous tick and not in this one. The **cause** is the
        // ledger's: this tick's change in its per-lineage counters says exactly how many
        // of each lineage starved and drowned. Within one lineage and one tick the model
        // calls a body below `body_min` starved and any other departing body drowned, so
        // when both happen at once the lightest bodies take the starvations. (Until
        // 2026-09-22 this re-derived the cause from the previous tick's water, and a body
        // that walked into deep water on its last tick was labelled starved.)
        let now_causes = sim.fauna().view().ledger.deaths_by_founder_cause;
        let mut departed: Vec<(u64, Seen)> = prev
            .iter()
            .filter(|(id, _)| now.binary_search_by_key(id, |(i, _)| *i).is_err())
            .copied()
            .collect();
        departed.sort_by(|a, b| a.1.body.total_cmp(&b.1.body).then(a.0.cmp(&b.0)));
        let mut labels: Vec<(u64, Departure)> = Vec::with_capacity(departed.len());
        for f in Founder::ALL {
            let delta: Vec<u64> = Departure::ALL
                .iter()
                .map(|c| now_causes[f.index()][c.index()] - ledger_causes[f.index()][c.index()])
                .collect();
            let mut causes = Departure::ALL
                .iter()
                .zip(&delta)
                .flat_map(|(c, n)| std::iter::repeat_n(*c, *n as usize));
            let mine: Vec<u64> = departed
                .iter()
                .filter(|(_, was)| was.founder == Some(f))
                .map(|(id, _)| *id)
                .collect();
            for id in &mine {
                let cause = causes.next().expect("a departure the ledger did not book");
                labels.push((*id, cause));
            }
            assert!(
                causes.next().is_none(),
                "tick {tick}: the ledger booked {:?} {} departures and {} bodies left",
                delta,
                f.name(),
                mine.len()
            );
        }
        ledger_causes = now_causes;
        let drowned: Vec<(u64, Option<Founder>, Site)> = departed
            .iter()
            .filter(|(id, _)| labels.contains(&(*id, Departure::Drowned)))
            .map(|(id, was)| (*id, was.founder, was.site))
            .collect();
        for line in drowning_lines(
            tick,
            &drowned,
            &founders_at_start,
            &sim.world().view(),
            rained,
            &water_history,
        ) {
            println!("{line}");
        }
        for (id, was) in &departed {
            {
                let cause = labels
                    .iter()
                    .find(|(i, _)| i == id)
                    .map(|(_, c)| *c)
                    .expect("every departed founder is labelled");
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
                if was.founder == Some(Founder::Browser) {
                    report_death_cone(tick, *id, cause.name(), cones.get(id), wander.get(id));
                }
                cones.remove(id);
                wander.remove(id);
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
            report_cone_classes(tick, minute, &cones, &now);
            report_wander(tick, minute, &mut wander, &now);
        }
        prev = now;
    }

    summary(&sim, total_ticks);
}

/// How often the cone census is taken for a living browser, in ticks. The census
/// rebuilds the occupancy map, so it is not free; 5 s is fine enough that "all three
/// sectors blank" is measured and not inferred from one sample a minute, and coarse
/// enough that a 60-minute arm stays a few minutes of wall clock.
const CONE_SAMPLE_TICKS: u64 = 100;

/// Seconds of resting upkeep below which a body is in its terminal band and its cone is
/// censused **every** tick, so the row printed at its death is the world it last saw and
/// not one up to five seconds stale.
const TERMINAL_UPKEEP_S: f64 = 30.0;

/// What one browser's ground covering looks like inside one minute, and over its life.
#[derive(Clone, Default)]
struct Wander {
    /// `travelled_m` at the last minute boundary, so the minute's own metres are a
    /// difference and not a running total.
    travelled_at_minute_m: f64,
    columns: std::collections::HashSet<(u32, u32)>,
    ticks: u64,
    turning_ticks: u64,
    exits_sum: u64,
    /// Cone censuses taken this minute, and how many of them read zero foliage in all
    /// three sectors.
    samples: u64,
    blind_samples: u64,
    /// The same two over the whole life, plus the minutes in which **every** sample was
    /// blank: "the share of minutes with all three sectors at zero foliage".
    life_samples: u64,
    life_blind_samples: u64,
    minutes: u64,
    blind_minutes: u64,
    life_travelled_m: f64,
    life_columns: std::collections::HashSet<(u32, u32)>,
}

/// The cone as one browser last saw it, kept so the row can be printed once the body is
/// gone from the view.
#[derive(Clone)]
struct LastCone {
    tick: u64,
    /// First-hit counts over the whole 27-ray fan, indexed by [`ConeHit::index`].
    counts: [u32; ConeHit::ALL.len()],
    /// Per sector, the same counts and the mean distance of its foliage hits.
    per_sector: [([u32; ConeHit::ALL.len()], f64); 3],
    /// The nearest **living** crown by straight line: planar metres, the signed voxels
    /// between its crown layer and the eye, its elevation from the eye in degrees, and
    /// whether that elevation is inside the fan's pitch band.
    crown: Option<NearestCrown>,
}

#[derive(Clone, Copy)]
struct NearestCrown {
    planar_m: f64,
    layer_offset_voxels: f64,
    elevation_deg: f64,
    in_pitch_band: bool,
    /// What a ray aimed straight at it strikes first, and how far away.
    probe: ConeHit,
    probe_m: f64,
}

/// The cone census of one browser, reduced to what the rows need.
fn last_cone(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    av: &cubarium_voxel_fauna::FaunaView<'_>,
    animal: &Animal,
    tick: u64,
) -> Option<LastCone> {
    let crown = nearest_living_crown(view, fv, av.config, animal);
    // A probe aimed at that crown, in the ray's own frame: yaw from the body's heading,
    // pitch from horizontal. Marched by `browser_cone_census` with the same ray the fan
    // uses, so "blocked by what" is the fan's own answer and not a second opinion.
    let probes: Vec<(f64, f64)> = crown.iter().map(|c| (c.0, c.1)).collect();
    let census: ConeCensus = browser_cone_census(view, fv, av, animal, &probes)?;
    let mut counts = [0u32; ConeHit::ALL.len()];
    for ray in &census.rays {
        counts[ray.hit.index()] += 1;
    }
    let per_sector = std::array::from_fn(|si| {
        (
            census.counts(si),
            census.nearest_foliage_mean_m(si).unwrap_or(f64::INFINITY),
        )
    });
    let crown = crown.map(|(_, elevation_rad, planar_m, layer_offset_voxels)| {
        let probe = census.probes.first().copied();
        NearestCrown {
            planar_m,
            layer_offset_voxels,
            elevation_deg: elevation_rad.to_degrees(),
            in_pitch_band: elevation_rad.to_degrees().abs() <= PITCH_BAND_DEG,
            probe: probe.map_or(ConeHit::Clear, |r| r.hit),
            probe_m: probe.map_or(f64::INFINITY, |r| r.distance_m),
        }
    });
    Some(LastCone {
        tick,
        counts,
        per_sector,
        crown,
    })
}

/// The nearest stand with foliage standing in it, as the cone would have to reach it:
/// `(yaw from the body's heading, elevation from the eye, planar metres, signed voxels
/// between the crown's layer and the eye)`. Straight-line nearest, ignoring occlusion —
/// the question is whether anything edible is out there at all.
fn nearest_living_crown(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    config: &cubarium_voxel_fauna::FaunaConfig,
    animal: &Animal,
) -> Option<(f64, f64, f64, f64)> {
    let c = view.config;
    let v = c.voxel_m;
    let mut best: Option<(f64, &cubarium_voxel_flora::Stand)> = None;
    for stand in fv.stands.iter().filter(|s| s.foliage > 0.0) {
        let d = planar_m(view, animal, stand.site.x, stand.site.z, v);
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, stand));
        }
    }
    let (planar, stand) = best?;
    // The crown's own cell layer, the way `cone_occupancy` indexes it.
    let sc = fv.config.species(stand.species);
    let crown_layer = f64::from(stand.site.y) + f64::from(sc.crown_voxels(stand.wood, fv.config.voxel_m));
    // The eye, in metres: `0.8 × body height` over the standing surface
    // (`senses::cone_origin`; `design/handoffs/voxel-body-anchors-2026-09-22.md`).
    let eye_m = cubarium_voxel_fauna::surface_m(animal.site.y, v)
        + cubarium_voxel_fauna::body_of(config, animal).map_or(0.0, |b| b.eye_m);
    let layer_offset = crown_layer - eye_m / v;
    // Yaw in the ray's frame: `dir = (sin yaw, ., cos yaw)`, so it is measured from +z
    // toward +x, and `x` takes the short way round the ring.
    let w_m = f64::from(c.width) * v;
    let mut dx = (f64::from(stand.site.x) + 0.5) * v - animal.pose.x;
    if dx > w_m / 2.0 {
        dx -= w_m;
    } else if dx < -w_m / 2.0 {
        dx += w_m;
    }
    let dz = (f64::from(stand.site.z) + 0.5) * v - animal.pose.z;
    let yaw = dx.atan2(dz) - animal.pose.heading_rad;
    // The crown cell's centre against the eye's own height.
    let rise = (crown_layer + 0.5) * v - eye_m;
    let elevation = rise.atan2(planar.max(1e-9));
    Some((yaw, elevation, planar, layer_offset))
}

/// One `BCONE` row per living browser per sector, and one `CONEX` row per sector
/// aggregated over them: which wall the fan is looking at, named.
fn report_cone_classes(
    tick: u64,
    minute: u64,
    cones: &std::collections::HashMap<u64, LastCone>,
    now: &[(u64, Seen)],
) {
    let living: Vec<&u64> = now
        .iter()
        .filter(|(_, s)| s.founder == Some(Founder::Browser))
        .map(|(id, _)| id)
        .collect();
    let mut totals = [[0.0f64; ConeHit::ALL.len()]; 3];
    let mut foliage_m = [(0.0f64, 0u64); 3];
    let mut counted = 0u64;
    let mut blind = 0u64;
    for id in &living {
        let Some(cone) = cones.get(*id) else {
            continue;
        };
        counted += 1;
        if cone
            .per_sector
            .iter()
            .all(|(c, _)| c[ConeHit::FoliageCrown.index()] == 0)
        {
            blind += 1;
        }
        for (si, (counts, mean_m)) in cone.per_sector.iter().enumerate() {
            let rays: u32 = counts.iter().sum();
            let denom = f64::from(rays.max(1));
            print!("BCONE,{tick},{minute},{id},{si},{rays}");
            for hit in ConeHit::ALL {
                let share = f64::from(counts[hit.index()]) / denom;
                totals[si][hit.index()] += share;
                print!(",{share:.4}");
            }
            if mean_m.is_finite() {
                foliage_m[si].0 += mean_m;
                foliage_m[si].1 += 1;
                println!(",{mean_m:.3}");
            } else {
                println!(",inf");
            }
        }
    }
    for si in 0..3 {
        let denom = f64::from(counted.max(1) as u32);
        print!("CONEX,{tick},{minute},{si},{counted}");
        for hit in ConeHit::ALL {
            print!(",{:.4}", totals[si][hit.index()] / denom);
        }
        if foliage_m[si].1 > 0 {
            print!(",{:.3}", foliage_m[si].0 / foliage_m[si].1 as f64);
        } else {
            print!(",inf");
        }
        println!(",{blind}");
    }
}

/// One `WANDER` row per living browser per minute, and the minute's own accumulators
/// rolled into the life totals and cleared.
fn report_wander(
    tick: u64,
    minute: u64,
    wander: &mut std::collections::HashMap<u64, Wander>,
    now: &[(u64, Seen)],
) {
    for (id, s) in now
        .iter()
        .filter(|(_, s)| s.founder == Some(Founder::Browser))
    {
        let Some(w) = wander.get_mut(id) else {
            continue;
        };
        let metres = s.travelled_m - w.travelled_at_minute_m;
        let turn_share = if w.ticks == 0 {
            0.0
        } else {
            w.turning_ticks as f64 / w.ticks as f64
        };
        let mean_exits = if w.ticks == 0 {
            0.0
        } else {
            w.exits_sum as f64 / w.ticks as f64
        };
        println!(
            "WANDER,{tick},{minute},{id},{metres:.3},{},{turn_share:.4},{mean_exits:.3},{},{}",
            w.columns.len(),
            w.samples,
            w.blind_samples,
        );
        w.minutes += 1;
        if w.samples > 0 && w.samples == w.blind_samples {
            w.blind_minutes += 1;
        }
        w.travelled_at_minute_m = s.travelled_m;
        w.columns.clear();
        w.ticks = 0;
        w.turning_ticks = 0;
        w.exits_sum = 0;
        w.samples = 0;
        w.blind_samples = 0;
    }
}

/// The census the body last took, printed once it is gone.
fn report_death_cone(
    tick: u64,
    id: u64,
    cause: &str,
    cone: Option<&LastCone>,
    wander: Option<&Wander>,
) {
    let minute = tick as f64 / TICKS_PER_MIN as f64;
    let Some(cone) = cone else {
        println!("DEATHCONE,{tick},{minute:.2},{id},{cause},none");
        return;
    };
    print!("DEATHCONE,{tick},{minute:.2},{id},{cause},{}", cone.tick);
    for hit in ConeHit::ALL {
        print!(",{}", cone.counts[hit.index()]);
    }
    match cone.crown {
        Some(c) => print!(
            ",{:.3},{:.2},{:.1},{},{},{}",
            c.planar_m,
            c.layer_offset_voxels,
            c.elevation_deg,
            c.in_pitch_band,
            c.probe.name(),
            if c.probe_m.is_finite() {
                format!("{:.3}", c.probe_m)
            } else {
                "inf".to_string()
            },
        ),
        None => print!(",inf,nan,nan,false,none,inf"),
    }
    match wander {
        Some(w) => println!(
            ",{:.4},{:.4},{:.2},{},{:.4}",
            if w.life_samples == 0 {
                f64::NAN
            } else {
                w.life_blind_samples as f64 / w.life_samples as f64
            },
            if w.minutes == 0 {
                f64::NAN
            } else {
                w.blind_minutes as f64 / w.minutes as f64
            },
            w.life_travelled_m,
            w.life_columns.len(),
            if w.ticks == 0 {
                f64::NAN
            } else {
                w.turning_ticks as f64 / w.ticks as f64
            },
        ),
        None => println!(",nan,nan,nan,0,nan"),
    }
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
            let food = food_probe(&view, &fv, fauna.config(), a);
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
                    x: a.pose.x,
                    z: a.pose.z,
                    heading_rad: a.pose.heading_rad,
                    travelled_m: travelled(prev, a),
                    held_forward: a.founder_state.held.forward,
                    held_turn: a.founder_state.held.turn,
                    held_feed: a.founder_state.held.feed,
                    state: a.state,
                    exits: exits(&view, a, &sc),
                    site: a.site,
                    ahead: ahead(&view, a, &sc),
                },
            )
        })
        .collect()
}

/// Planar distance and contact evidence for the nearest stand with foliage. The browser
/// reach bit is checked against the actual mouth probe columns, not the wider cone.
fn food_probe(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    config: &cubarium_voxel_fauna::FaunaConfig,
    a: &Animal,
) -> FoodProbe {
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
    let mouth = browser_mouth_candidates(view, fv, config, a).unwrap_or_default();
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
            .map(|a| food_probe(&view, &fv, fauna.config(), a).dist_m)
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
    report_diet(minute, &fv, &av);
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

/// **Bites by food class**, against the standing stock of each (decisions §3). The
/// shredder eats litter, glowcap cap tissue and carrion; the browser eats vascular
/// foliage and nothing fungal, so `foliage` here excludes the cap.
fn report_diet(minute: u64, fv: &FloraView<'_>, av: &cubarium_voxel_fauna::FaunaView<'_>) {
    let fungal = |s: &cubarium_voxel_flora::Stand| {
        matches!(
            fv.config.species(s.species).trophic,
            cubarium_voxel_flora::Trophic::Saprotroph
        )
    };
    for food in Food::ALL {
        let standing: f64 = match food {
            Food::Litter => fv.ground.iter().map(|g| g.litter).sum(),
            Food::Carrion => fv.ground.iter().map(|g| g.carrion).sum(),
            Food::CapTissue => fv
                .stands
                .iter()
                .filter(|s| fungal(s))
                .map(|s| s.foliage)
                .sum(),
            Food::Foliage => fv
                .stands
                .iter()
                .filter(|s| !fungal(s))
                .map(|s| s.foliage)
                .sum(),
        };
        println!(
            "DIET,{},{minute},{},{standing:.6},{},{:.7}",
            minute * TICKS_PER_MIN,
            food.name(),
            av.ledger.bites_by_food[food.index()],
            av.ledger.eaten_by_food[food.index()],
        );
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
    // The other end of the same transfer: which food class each bite came out of
    // (decisions §3). `sum(bites_by_food) == bites` by construction.
    for food in Food::ALL {
        println!(
            "SUMMARY,{ticks},food_{},bites={} eaten={:.7}",
            food.name(),
            l.bites_by_food[food.index()],
            l.eaten_by_food[food.index()],
        );
    }
    // The two ledgers, on the world that actually ran: an escrow and a clutch are paid
    // packages the layer still owns, so a reproduction round that created or destroyed
    // matter would show here and nowhere else in this file.
    let av = sim.fauna().view();
    let fv = sim.flora().view();
    println!(
        "SUMMARY,{ticks},residuals,fauna_organic={:.3e} fauna_mineral={:.3e} fauna_energy={:.3e} flora_organic={:.3e} flora_mineral={:.3e} flora_energy={:.3e}",
        av.organic() - av.ledger.expected_organic(),
        av.mineral() - av.ledger.expected_mineral(),
        av.energy() - av.ledger.expected_energy(),
        fv.organic() - fv.ledger.expected_organic(),
        fv.mineral() - fv.ledger.expected_mineral(),
        fv.energy() - fv.ledger.expected_energy(),
    );
    assert_eq!(
        l.deaths,
        l.deaths_accounted(),
        "the cause counters must account for every death"
    );
}

/// What `VoxelView::water_depth_m` read at a support face, and how it got there: the
/// cells it summed going up from the face while each held water, and the fill of the
/// first of them.
#[derive(Clone, Copy, Debug, PartialEq)]
struct DepthRead {
    depth_m: f64,
    cells: u32,
    bottom_fill: f64,
}

/// [`VoxelView::water_depth_m`]'s own walk, counting as it goes: from `y + 1` up, while
/// the cell is not solid and holds water. `free` reads a cell's fill by index, so the
/// same walk runs on the live world or on a remembered one.
fn depth_read_with(view: &VoxelView<'_>, site: Site, free: impl Fn(usize) -> f64) -> DepthRead {
    let c = view.config;
    let mut read = DepthRead {
        depth_m: 0.0,
        cells: 0,
        bottom_fill: 0.0,
    };
    if site.z >= c.depth {
        return read;
    }
    let mut depth = 0.0;
    for y in site.y + 1..c.height {
        if view.material_at(i64::from(site.x), y, site.z).is_solid() {
            break;
        }
        let f = free(c.index(i64::from(site.x), y, site.z));
        if !(f > 0.0) {
            break;
        }
        if read.cells == 0 {
            read.bottom_fill = f;
        }
        read.cells += 1;
        depth += f;
    }
    read.depth_m = depth * c.voxel_m;
    read
}

fn depth_read(view: &VoxelView<'_>, site: Site) -> DepthRead {
    depth_read_with(view, site, |i| view.free[i])
}

/// The last 100 ticks of free water, one sparse snapshot (cell index, fill) per tick:
/// enough to read any face's depth as it was 100 ticks before a drowning.
#[derive(Default)]
struct WaterHistory {
    ticks: std::collections::VecDeque<(u64, Vec<(u32, f64)>)>,
}

impl WaterHistory {
    const BACK: u64 = 100;

    fn observe(&mut self, tick: u64, view: &VoxelView<'_>) {
        let wet: Vec<(u32, f64)> = view
            .free
            .iter()
            .enumerate()
            .filter(|(_, f)| **f > 0.0)
            .map(|(i, f)| (i as u32, *f))
            .collect();
        self.ticks.push_back((tick, wet));
        while self
            .ticks
            .front()
            .is_some_and(|(t, _)| *t + Self::BACK < tick)
        {
            self.ticks.pop_front();
        }
    }

    /// The depth read at `site` at tick `tick − 100`, on the terrain as it is now; `None`
    /// before the history reaches that far.
    fn depth_back(&self, tick: u64, view: &VoxelView<'_>, site: Site) -> Option<f64> {
        let want = tick.checked_sub(Self::BACK)?;
        let (_, wet) = self.ticks.iter().find(|(t, _)| *t == want)?;
        let fill = |i: usize| {
            wet.binary_search_by_key(&(i as u32), |(j, _)| *j)
                .map_or(0.0, |k| wet[k].1)
        };
        Some(depth_read_with(view, site, fill).depth_m)
    }
}

/// One `DROWN` row per body the tick drowned: `(id, lineage, the face it stood on)`.
fn drowning_lines(
    tick: u64,
    drowned: &[(u64, Option<Founder>, Site)],
    founders_at_start: &[u64],
    view: &VoxelView<'_>,
    raining: bool,
    history: &WaterHistory,
) -> Vec<String> {
    drowned
        .iter()
        .map(|&(id, founder, site)| {
            let read = depth_read(view, site);
            let origin = if founders_at_start.contains(&id) {
                "founder"
            } else {
                "born"
            };
            format!(
                "DROWN,{tick},{:.2},{id},{},{origin},{},{},{},{:.4},{},{:.4},{},{}",
                tick as f64 / TICKS_PER_MIN as f64,
                lineage(founder),
                site.x,
                site.y,
                site.z,
                read.depth_m,
                read.cells,
                read.bottom_fill,
                u8::from(raining),
                history
                    .depth_back(tick, view, site)
                    .map_or("na".to_string(), |d| format!("{d:.4}")),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_voxel::{Config as WorldConfig, Material};
    use cubarium_voxel_fauna::{Command, StartingStores};

    /// A flat one-row plain at 1 m cells, soil in `1..=2`, dry: every support face is
    /// `y = 2`.
    fn plain() -> World {
        let mut w = World::empty(WorldConfig {
            width: 6,
            height: 10,
            depth: 1,
            voxel_m: 1.0,
            seed: 5,
            ..WorldConfig::default()
        });
        for x in 0..6 {
            for y in 1..=2 {
                w.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z: 0,
                    material: Material::Soil,
                });
            }
        }
        w
    }

    /// Package S item 4: a body drowned on a hand-flooded face gets its `DROWN` row —
    /// the depth the rule read and how many cells it summed, the bottom cell's fill, the
    /// rain flag, and the same face's depth 100 ticks earlier (dry).
    #[test]
    fn a_hand_built_drowning_prints_its_drown_row() {
        let mut world = plain();
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::default());
        assert!(fauna.apply(
            &world,
            Command::IntroduceFounder {
                x: 4,
                z: 0,
                founder: Founder::Browser,
                stores: StartingStores::FULL,
                heading_rad: 0.0,
            }
        ));
        let (id, site) = {
            let a = &fauna.view().animals[0];
            (a.id, a.site)
        };
        let founders = vec![id];
        let mut history = WaterHistory::default();
        for tick in 0..=100 {
            history.observe(tick, &world.view());
        }

        // One whole voxel of water over the face: 1 m, past every drown depth.
        let volume = world.config().voxel_volume();
        let got = world.apply(WorldCommand::AddWater {
            x: 4,
            y: site.y + 1,
            z: 0,
            volume_m3: volume,
        });
        assert!(got > 0.0);
        fauna.step(&world, &mut flora);
        assert!(fauna.view().animals.is_empty(), "the body drowned");
        assert_eq!(fauna.view().ledger.departed(Departure::Drowned), 1);
        history.observe(101, &world.view());

        let view = world.view();
        let read = depth_read(&view, site);
        assert_eq!(
            read.depth_m,
            view.water_depth_m(i64::from(site.x), site.y, site.z),
            "the rule's own reader"
        );
        assert_eq!(read.cells, 1);
        assert_eq!(read.bottom_fill, 1.0);
        assert_eq!(history.depth_back(101, &view, site), Some(0.0));

        let lines = drowning_lines(
            101,
            &[(id, Some(Founder::Browser), site)],
            &founders,
            &view,
            false,
            &history,
        );
        assert_eq!(lines.len(), 1);
        let fields: Vec<&str> = lines[0].split(',').collect();
        assert_eq!(fields[0], "DROWN");
        assert_eq!(fields[1], "101");
        assert_eq!(fields[3], id.to_string());
        assert_eq!(fields[4], "frondgrazer");
        assert_eq!(fields[5], "founder");
        assert_eq!(
            &fields[6..9],
            &[
                site.x.to_string().as_str(),
                site.y.to_string().as_str(),
                site.z.to_string().as_str()
            ]
        );
        assert_eq!(fields[9].parse::<f64>().expect("depth"), 1.0);
        assert_eq!(fields[10], "1");
        assert_eq!(fields[11].parse::<f64>().expect("fill"), 1.0);
        assert_eq!(fields[12], "0", "no rain fell");
        assert_eq!(fields[13].parse::<f64>().expect("earlier"), 0.0);

        // A body born in the run says so, and a history too short says `na`.
        let short = WaterHistory::default();
        let born = drowning_lines(
            101,
            &[(id + 1, Some(Founder::Blind), site)],
            &founders,
            &view,
            true,
            &short,
        );
        let fields: Vec<&str> = born[0].split(',').collect();
        assert_eq!(fields[4], "littershredder");
        assert_eq!(fields[5], "born");
        assert_eq!(fields[12], "1");
        assert_eq!(fields[13], "na");
    }
}
