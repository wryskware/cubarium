//! The live-training package's tests (`design/handoffs/voxel-live-world-training-2026-09-24.md`,
//! "Tests"): each at most 200 ticks, no pinned hashes.

use cubarium_voxel::{Config, Landform, Preset, Recipe, World};
use cubarium_voxel_fauna::{Command as FaunaCommand, Founder, Response, StartingStores};
use cubarium_voxel_sim::{ScheduleMode, Sim, SimConfig};

use super::trainer::{ascend, pooled_scores};
use super::variants::VariantTable;
use super::window::{Placed, Window, newborn_candidate, place_twins};
use super::world::{is_still, still_water, training_config};
use crate::es::optimizer::Adam;
use crate::es::tensor::initial_center_shape;
use crate::es::voxel::driver::sampled_stores;
use crate::es::voxel::task::{PreparedArena, Stage, TRAINING_LAYOUT_SEEDS};

fn centre(founder: Founder) -> Vec<f64> {
    match founder {
        Founder::Blind => initial_center_shape::<23, 3>(5),
        Founder::Browser => initial_center_shape::<37, 3>(5),
    }
}

fn logits(r: Response) -> [f64; 3] {
    match r {
        Response::Logits(l) => l,
        other => panic!("a GRU answers logits, not {other:?}"),
    }
}

/// Two bodies of one lineage on different variants act differently on the same
/// observation; two on the same variant act identically.
#[test]
fn different_variants_act_differently_and_the_same_variant_identically() {
    for founder in Founder::ALL {
        let table =
            VariantTable::antithetic(founder, &centre(founder), 7, 0.5, 0, 2).expect("valid");
        assert_eq!(table.len(), 4);
        let n = founder.manifest().inputs();
        let obs: Vec<f64> = (0..n).map(|i| ((i * 7 % 11) as f64) / 11.0 - 0.3).collect();
        let (mut a, mut b, mut a2) = (
            table.controller(0),
            table.controller(1),
            table.controller(0),
        );
        // Several samples, so the memory is in play too.
        for _ in 0..5 {
            let (la, lb, la2) = (
                logits(a.drive(&obs)),
                logits(b.drive(&obs)),
                logits(a2.drive(&obs)),
            );
            assert_ne!(la, lb, "{founder:?}: pair0+ and pair0- answer alike");
            assert_eq!(la, la2, "{founder:?}: one variant, two answers");
        }
    }
}

fn arena_sim(founder: Founder) -> Sim {
    let seed = TRAINING_LAYOUT_SEEDS[0];
    let prepared = PreparedArena::build_stage(founder, seed, Stage::A);
    let mut arena = prepared.episode_arena();
    arena.refound(sampled_stores(founder, seed, seed, 0));
    let mut sim = arena.into_sim_prepared(SimConfig::with_threads(1), prepared.episode_senses());
    sim.set_mode(ScheduleMode::Static);
    sim
}

fn tables(pairs: usize) -> Vec<Option<VariantTable>> {
    Founder::ALL
        .iter()
        .map(|&f| Some(VariantTable::antithetic(f, &centre(f), 3, 0.05, 0, pairs).expect("valid")))
        .collect()
}

/// A body that appears mid-window gets a variant, and its survival counts from its own
/// arrival, not from the window's start.
#[test]
fn a_newborn_gets_a_variant_and_its_own_window() {
    let founder = Founder::Blind;
    let mut sim = arena_sim(founder);
    let tables = tables(2);
    let mut w = Window::begin(&mut sim, &tables, &[0; Founder::COUNT], 120);
    for _ in 0..60 {
        w.step(&mut sim);
    }
    let (voxel_m, depth) = (sim.world().config().voxel_m, sim.world().config().depth);
    let (cx, cz) = sim.fauna().view().animals[0]
        .pose
        .column(voxel_m, depth)
        .expect("on the grid");
    let placed = sim.with_layers_mut(|world, _, fauna| {
        fauna.apply(
            world,
            FaunaCommand::IntroduceFounder {
                x: cx,
                z: cz,
                founder,
                stores: StartingStores::FULL,
                heading_rad: 0.0,
            },
        )
    });
    assert!(placed, "the newcomer was placed");
    while !w.done() {
        w.step(&mut sim);
    }
    let r = w.finish(&sim, 0);
    assert_eq!(r.lineages[founder.index()].births, 1);
    let newborn: Vec<_> = r.scores.iter().filter(|b| b.newborn).collect();
    assert_eq!(newborn.len(), 1, "one newcomer scored");
    let b = newborn[0];
    assert_eq!(b.founder, founder);
    assert!(b.candidate < 4, "a variant of the table");
    assert_eq!(
        b.score.survival_term, 0.25,
        "alive for all of its own part of the window"
    );
}

/// A body's score counts only its own window: over a second window on the same world,
/// the bodies' summed motor is the ledger's motor over that window alone.
#[test]
fn a_bodys_score_counts_only_its_own_window() {
    for founder in Founder::ALL {
        let mut sim = arena_sim(founder);
        let tables = tables(2);
        let mut motor_by_window = Vec::new();
        for _ in 0..2 {
            let before = sim.fauna().view().ledger.respired_motor_out;
            let mut w = Window::begin(&mut sim, &tables, &[0; Founder::COUNT], 100);
            while !w.done() {
                w.step(&mut sim);
            }
            let r = w.finish(&sim, 0);
            let after = sim.fauna().view().ledger.respired_motor_out;
            let bodies: f64 = r
                .scores
                .iter()
                .map(|b| b.score.motor_loss_normalized * b.founder.manifest().body_reference)
                .sum();
            assert!(
                (bodies - (after - before)).abs() <= 1e-9 * (after - before).abs().max(1e-12),
                "{founder:?}: bodies {bodies:e} vs the window's ledger {:e}",
                after - before
            );
            motor_by_window.push(bodies);
        }
        assert!(motor_by_window[1] > 0.0, "the bodies moved");
    }
}

/// Twins are neighbours and pairs are numbered on from `first_pair`; a newborn joins its
/// nearest neighbour's pair on the side with fewer bodies.
#[test]
fn twins_are_neighbours_and_newborns_balance_their_pair() {
    let bodies: Vec<Placed> = [5.0, 1.0, 3.0, 9.0, 7.0]
        .iter()
        .enumerate()
        .map(|(i, &x)| Placed {
            id: i as u64,
            x_m: x,
        })
        .collect();
    let got = place_twins(&bodies, 4, 3);
    // Sorted: x=1 (id1), 3 (id2), 5 (id0), 7 (id4), 9 (id3).
    assert_eq!(got, vec![(1, 6), (2, 7), (0, 0), (4, 1), (3, 2)]);
    let assigned = [(1.0, 6), (3.0, 7), (5.0, 0), (7.0, 1), (9.0, 2)];
    let mut counts = vec![0u32; 8];
    for (_, c) in assigned {
        counts[c] += 1;
    }
    // Next to x = 9 (candidate 2, pair 1+): pair 1 has one + and no −.
    assert_eq!(newborn_candidate(9.2, 20.0, &assigned, &counts), 3);
    // Next to x = 1 across the seam (ring 20 m): pair 3 is balanced, so the twin side.
    assert_eq!(newborn_candidate(19.5, 20.0, &assigned, &counts), 7);
}

/// Antithetic twins on a toy world with a known best direction: pooled over worlds of
/// very different richness, with placement noise, the update points uphill (sign test
/// over independent runs).
#[test]
fn the_pooled_antithetic_update_points_the_right_way() {
    let dim = 12;
    let best: Vec<f64> = (0..dim)
        .map(|i| if i % 3 == 0 { 1.0 } else { -0.5 })
        .collect();
    let pairs = 8;
    let sigma = 0.1;
    let mut uphill = 0;
    let runs = 20;
    for run in 0..runs {
        let train_seed = 100 + run;
        let theta = vec![0.0; dim];
        let mut eps = vec![0.0; dim];
        let cands: Vec<Vec<f64>> = (0..pairs)
            .flat_map(|p| {
                [
                    crate::es::voxel::trainer::Candidate::Plus(p),
                    crate::es::voxel::trainer::Candidate::Minus(p),
                ]
            })
            .map(|c| {
                crate::es::voxel::trainer::candidate_theta(
                    &theta, train_seed, sigma, 0, c, &mut eps,
                )
            })
            .collect();
        // Four worlds, richness 0, 10, 100 and -5; three bodies per candidate per world,
        // each with its own luck of place.
        let mut noise = train_seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let mut rnd = || {
            noise ^= noise << 13;
            noise ^= noise >> 7;
            noise ^= noise << 17;
            (noise % 10_000) as f64 / 10_000.0 - 0.5
        };
        let per_world: Vec<Vec<(usize, f64)>> = [0.0, 10.0, 100.0, -5.0]
            .iter()
            .map(|&rich| {
                let mut v = Vec::new();
                for (c, t) in cands.iter().enumerate() {
                    let signal: f64 = t.iter().zip(&best).map(|(a, b)| a * b).sum();
                    for _ in 0..3 {
                        v.push((c, rich * (1.0 + signal) + 0.2 * rnd()));
                    }
                }
                v
            })
            .collect();
        let (scores, n) = pooled_scores(&per_world, 2 * pairs);
        assert!(n.iter().all(|&k| k == 12));
        let mut moved = theta.clone();
        let mut adam = Adam::new(dim);
        ascend(&mut moved, &mut adam, &scores, sigma, train_seed, 0);
        let dot: f64 = moved.iter().zip(&best).map(|(a, b)| a * b).sum();
        if dot > 0.0 {
            uphill += 1;
        }
    }
    assert!(uphill >= 18, "{uphill} of {runs} updates pointed uphill");
}

/// The flowing-water switches: the training world has no re-entry stream, no spring seep
/// and no lake drain, asks for no tier pools, and leaves the shipped preset alone; a
/// still world built on them conserves its water inventory over 200 ticks.
#[test]
fn the_training_world_is_still_and_conserves_its_water() {
    let cfg = training_config();
    let Landform::Staged(r) = &cfg.landform else {
        panic!("the training world is staged")
    };
    assert_eq!(r.water.reentry_m3_per_s, 0.0);
    assert_eq!(r.water.min_tier_pools, 0);
    assert_eq!(cfg.spring_k_m2_per_s, 0.0);
    assert!(r.water.closed_cycle, "the showers stay");
    let shipped = Preset::find("small").expect("small").config();
    let Landform::Staged(s) = &shipped.landform else {
        panic!("staged")
    };
    assert!(
        s.water.reentry_m3_per_s > 0.0 && s.water.min_tier_pools > 0,
        "preset unchanged"
    );

    // A small still world of the same recipe family, stepped 200 ticks with a shower.
    let mut world = World::new(still_water(Config {
        width: 32,
        height: 24,
        depth: 4,
        voxel_m: 0.25,
        landform: Landform::Staged(Recipe::DEFAULT),
        ..Config::default()
    }));
    assert!(is_still(&world));
    world.force_rain(cubarium_voxel::weather::RainMode::Shower);
    let total = |w: &World| {
        let v = w.view();
        v.stored_m3() + v.atmosphere_m3
    };
    let t0 = total(&world);
    for _ in 0..200 {
        world.step_with(1);
    }
    let v = world.view();
    assert_eq!(v.ledger.reentry_in, 0.0, "no stream re-entered");
    assert!(
        (total(&world) - t0).abs() <= 1e-9 * t0.max(1.0),
        "inventory {t0} became {}",
        total(&world)
    );
}

/// Coarse water: water is conserved over 200 ticks with the lateral phases on one tick
/// in four, through the world's own step and through the sim's schedule; and the mode is
/// never the live default — a world is built and decoded at one tick per step.
#[test]
fn coarse_water_conserves_and_refuses_to_be_the_default() {
    let cfg = still_water(Config {
        width: 32,
        height: 24,
        depth: 4,
        voxel_m: 0.25,
        landform: Landform::Staged(Recipe::DEFAULT),
        ..Config::default()
    });
    let total = |w: &World| {
        let v = w.view();
        v.stored_m3() + v.atmosphere_m3
    };
    let mut world = World::new(cfg.clone());
    assert_eq!(world.water_every(), 1, "built live");
    world.set_water_every(4);
    world.force_rain(cubarium_voxel::weather::RainMode::Shower);
    let t0 = total(&world);
    for _ in 0..200 {
        world.step_with(1);
    }
    assert!((total(&world) - t0).abs() <= 1e-9 * t0.max(1.0));
    assert!(world.view().ledger.rain_in > 0.0, "it rained");

    let decoded = World::load(&world.save()).expect("round trip");
    assert_eq!(
        decoded.water_every(),
        1,
        "a snapshot never carries coarse water"
    );

    let mut world = World::new(cfg);
    world.set_water_every(4);
    world.force_rain(cubarium_voxel::weather::RainMode::Shower);
    let t0 = total(&world);
    let mut sim = Sim::new(
        world,
        cubarium_voxel_flora::Flora::new(cubarium_voxel_flora::FloraConfig::default()),
        cubarium_voxel_fauna::Fauna::new(Default::default()),
        SimConfig::with_threads(1),
        None,
    );
    for _ in 0..200 {
        sim.step();
    }
    assert!((total(sim.world()) - t0).abs() <= 1e-9 * t0.max(1.0));
    assert_eq!(sim.world().water_every(), 4);
}
