//! The brain reward (`design/handoffs/cotrain-stage0-2026-10-01.md` §1, Wrysk 2026-10-01):
//! a body's net energy over its window, `(E_end − E_start + E_given) / body_ref`, with
//! `E_end = 0` for a body that died. Each at most 200 ticks; no pinned hashes.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{
    Actions, Command as FaunaCommand, Controller, Fauna, FaunaConfig, Founder, Response,
    StartingStores,
};
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig};

use super::window::{DEFAULT_WINDOW_MIN, EnergyMeter, NetEnergy};

fn flat_world() -> World {
    let mut world = World::empty(VoxelConfig {
        width: 8,
        height: 6,
        depth: 6,
        voxel_m: 0.25,
        ..VoxelConfig::default()
    });
    for z in 0..6 {
        for x in 0..8 {
            for y in 1..=2 {
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    world
}

/// A fixed action every sample.
struct Fixed(Actions);

impl Controller for Fixed {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        Response::Bounded(self.0)
    }
    fn reset(&mut self) {}
}

fn fixed(forward: f64, feed: f64) -> Box<dyn Controller> {
    Box::new(Fixed(Actions {
        forward,
        turn: 0.0,
        feed,
    }))
}

/// One body of `founder` on flat soil, optionally on a litter pile, run `ticks` under
/// `controller`; its net-energy score over those ticks.
fn run(
    cfg: FaunaConfig,
    founder: Founder,
    stores: StartingStores,
    litter: f64,
    controller: Box<dyn Controller>,
    ticks: u32,
) -> NetEnergy {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::for_voxel_size(0.25));
    let mut fauna = Fauna::new(cfg);
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 3,
            z: 3,
            founder,
            stores,
            heading_rad: 0.0,
        }
    ));
    let (id, site) = {
        let a = &fauna.view().animals[0];
        (a.id, a.site)
    };
    if litter > 0.0 {
        assert!(flora.deposit(
            site,
            Deposit {
                kind: DepositKind::Litter,
                organic: litter,
                mineral: 0.02 * litter,
                energy: 2.0 * litter,
            }
        ));
    }
    assert!(fauna.set_controller(id, controller));
    let mut meter = EnergyMeter::new(&fauna.view().animals[0]);
    for _ in 0..ticks {
        fauna.step(&world, &mut flora);
        match fauna.view().animals.iter().find(|a| a.id == id) {
            Some(a) => meter.observe(a),
            None => meter.die(),
        }
    }
    meter.score(0.0, 0.0, founder.manifest().body_reference)
}

/// For every trained lineage's default body (as the founding places it) over the window
/// the training CLIs default to, a body that dies in the window scores below an idle
/// survivor of the same window, whatever it ate before it died: the death loss.
#[test]
fn a_death_scores_below_an_idle_survivor_of_the_same_window() {
    let cfg = FaunaConfig::default();
    let window_s = DEFAULT_WINDOW_MIN * 60.0;
    for f in crate::es::voxel::TRAINED_LINEAGES {
        let sc = cfg.founder(f).core;
        let body = cfg.founding.arrival_body * sc.body_max;
        let e0 = body + cfg.founding.arrival_reserve * sc.reserve_of(body);
        // Idle and awake: upkeep only, no intake, no motion.
        let e_idle = e0 - sc.maintenance_per_s * body * window_s;
        assert!(
            e_idle > sc.body_min,
            "{f:?}: an idle founder does not outlive a {DEFAULT_WINDOW_MIN} min window \
             ({e_idle} vs body_min {})",
            sc.body_min
        );
        let r = f.manifest().body_reference;
        let idle = NetEnergy::of(e0, e_idle, 0.0, false, 0.0, 0.0, r);
        for eaten in [0.0, e0, 10.0 * e0] {
            let dead = NetEnergy::of(e0, e0 + eaten, 0.0, true, eaten, 0.0, r);
            assert!(
                dead.score < idle.score,
                "{f:?}: died after eating {eaten}: {} vs idle {}",
                dead.score,
                idle.score
            );
            assert!(dead.died);
        }
    }
}

/// With the same intake and motor — standing still, not feeding — a shredder that lays a
/// clutch in the window scores what one that cannot lay scores: the clutch is given, not
/// lost.
#[test]
fn breeding_does_not_lower_the_parents_score() {
    let arm = |can_lay: bool| {
        let mut cfg = FaunaConfig::default();
        let rule = &mut cfg.founders[Founder::Blind.index()].core.reproduction;
        // A hold of one tick, so a full founder lays at once.
        rule.surplus_hold_s = 0.05;
        if !can_lay {
            rule.litter_min = 1e9;
        }
        run(cfg, Founder::Blind, StartingStores::FULL, 0.5, fixed(0.0, 0.0), 120)
    };
    let (laid, kept) = (arm(true), arm(false));
    assert!(laid.given_normalized > 0.0, "the shredder laid: {laid:?}");
    assert_eq!(kept.given_normalized, 0.0);
    assert!(laid.delta_normalized < kept.delta_normalized, "the clutch left the body");
    assert!(
        (laid.score - kept.score).abs() <= 1e-9 * kept.score.abs().max(1e-12),
        "breeding moved the score: {} vs {}",
        laid.score,
        kept.score
    );
}

/// More intake raises the score: a hungry shredder feeding on litter beats one that
/// does not feed.
#[test]
fn more_intake_raises_the_score() {
    let cfg = FaunaConfig::default();
    let fed = run(cfg, Founder::Blind, StartingStores::HUNGRY, 0.5, fixed(0.0, 1.0), 200);
    let unfed = run(cfg, Founder::Blind, StartingStores::HUNGRY, 0.5, fixed(0.0, 0.0), 200);
    assert!(fed.score > unfed.score, "fed {fed:?} vs unfed {unfed:?}");
}

/// More motor lowers the score: a browser walking on bare soil loses more than one
/// standing still.
#[test]
fn more_motor_lowers_the_score() {
    let cfg = FaunaConfig::default();
    let walked = run(cfg, Founder::Browser, StartingStores::FULL, 0.0, fixed(1.0, 0.0), 200);
    let stood = run(cfg, Founder::Browser, StartingStores::FULL, 0.0, fixed(0.0, 0.0), 200);
    assert!(walked.score < stood.score, "walked {walked:?} vs stood {stood:?}");
}

/// The score is the body's net energy over the reference, and a death zeroes the end.
#[test]
fn the_score_is_net_energy_with_the_loss_at_death() {
    let s = NetEnergy::of(1.0, 1.5, 0.25, false, 0.0, 0.0, 0.5);
    assert!((s.score - 1.5).abs() < 1e-12, "{s:?}");
    let d = NetEnergy::of(1.0, 1.5, 0.25, true, 0.0, 0.0, 0.5);
    assert!((d.score - (-1.5)).abs() < 1e-12, "{d:?}");
}
