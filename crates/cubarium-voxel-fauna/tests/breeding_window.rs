//! **Breeding eligibility** (`design/handoffs/voxel-audit-round-2026-09-28.md`, 3; Wrysk
//! decided): a body may breed after being in surplus for at least a share of the last
//! hold, instead of all of it unbroken, so a foraging trip is not a reset. The tests set
//! their own hold and share.
//! Pure tests on the window, plus one short stepped world; under 200 ticks.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_fauna::{
    Command, Fauna, FaunaConfig, Founder, Reproduction, StartingStores, SurplusWindow,
};
use cubarium_voxel_flora::{Flora, FloraConfig};

/// A short trip no longer resets the hold: 8 ticks in surplus, a 4-tick trip, then
/// surplus again, and the window is met once 16 of the last 20 ticks were in surplus.
/// The old unbroken rule would have needed 20 more ticks after the trip.
#[test]
fn a_short_trip_is_not_a_reset() {
    let (hold, share) = (20u64, 0.8);
    let mut w = SurplusWindow::default();
    let mut met_at = None;
    for t in 1..=40u64 {
        let eligible = !(9..=12).contains(&t);
        w.tick(eligible, hold);
        if met_at.is_none() && w.met(eligible, hold, share) {
            met_at = Some(t);
        }
    }
    // By tick 20: 16 of the 20 ticks in surplus.
    assert_eq!(
        met_at,
        Some(20),
        "met as soon as 80 % of the window is in surplus"
    );
}

/// Unbroken surplus meets the window at 80 % of the hold; half the time in surplus never
/// meets it; and a body not in surplus right now is not eligible whatever its past.
#[test]
fn the_share_is_what_counts() {
    let (hold, share) = (20u64, 0.8);
    let mut w = SurplusWindow::default();
    let first = (1..=20u64).find(|_| {
        w.tick(true, hold);
        w.met(true, hold, share)
    });
    assert_eq!(first, Some(16));
    let mut half = SurplusWindow::default();
    for t in 0..100u64 {
        half.tick(t % 2 == 0, hold);
        assert!(!half.met(t % 2 == 0, hold, share), "tick {t}");
    }
    let mut w = SurplusWindow::default();
    for _ in 0..30 {
        w.tick(true, hold);
    }
    assert!(w.met(true, hold, share) && !w.met(false, hold, share));
    w.reset();
    assert_eq!(w.count(), 0);
}

/// A 900 s hold at share 0.8, bucketed: a 100 s trip in the middle of 900 s of surplus
/// still meets the window at the end; a 300 s trip does not.
#[test]
fn a_hundred_second_trip_in_a_900_s_hold_is_not_a_reset() {
    let hold = (900.0 / DT) as u64;
    let run = |trip_s: f64| {
        let trip = (trip_s / DT) as u64;
        let (start, end) = (hold / 2 - trip / 2, hold / 2 + trip / 2);
        let mut w = SurplusWindow::default();
        for t in 0..hold {
            w.tick(!(start..end).contains(&t), hold);
        }
        w.met(true, hold, 0.8)
    };
    assert!(
        run(100.0),
        "a 100 s trip leaves 89 % of the window in surplus"
    );
    assert!(!run(300.0), "a 300 s trip leaves 67 %");
}

/// In a stepped world a browser eligible from its first tick opens its gestation at 80 %
/// of a short test hold.
#[test]
fn a_gestation_opens_at_eighty_percent_of_the_hold() {
    let mut world = World::empty(VoxelConfig {
        width: 4,
        height: 10,
        depth: 1,
        voxel_m: 1.0,
        seed: 5,
        ..VoxelConfig::default()
    });
    for x in 0..4 {
        for y in 1..=2 {
            world.apply(WorldCommand::SetMaterial {
                x,
                y,
                z: 0,
                material: Material::Soil,
            });
        }
    }
    let mut flora = Flora::new(FloraConfig::default());
    let mut c = FaunaConfig::default();
    c.founders[Founder::Browser.index()].core.reproduction = Reproduction {
        surplus_floor: 0.005,
        surplus_hold_s: 20.0 * DT,
        gestation_s: 10.0 * DT,
        birth_interval_s: 20.0 * DT,
        surplus_share: 0.8,
        ..Reproduction::LIVE_BIRTH_PLACEHOLDER
    };
    let mut fauna = Fauna::new(c);
    assert!(fauna.apply(
        &world,
        Command::IntroduceFounder {
            x: 1,
            z: 0,
            founder: Founder::Browser,
            stores: StartingStores::FULL,
            heading_rad: 0.0,
        }
    ));
    let mut opened = None;
    for t in 1..=30u64 {
        fauna.step(&world, &mut flora);
        if opened.is_none() && fauna.view().ledger.gestations_opened > 0 {
            opened = Some(t);
        }
    }
    assert_eq!(opened, Some(16), "80 % of a 20-tick hold");
}
