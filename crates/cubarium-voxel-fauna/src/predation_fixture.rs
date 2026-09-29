//! P1 (predation substrate): the small worlds, placements and controllers the three
//! predation test files share. A child of `step`, so a case can reach the controller
//! table and the layer's own fields.
//!
//! API this file expects (added by P1's implementation commit):
//! - `Founder::Lanternjaw` (manifest.rs), with its `FounderPhysiology` in the default
//!   `FaunaConfig::founders` table (body.rs, lib.rs);
//! - `FounderPhysiology::hunter: Option<Hunter>` (body.rs), `Hunter` re-exported at the
//!   crate root;
//! - `Animal::strike: Option<Strike>`, `Strike::phase`, `StrikePhase::{Windup, Lunge,
//!   Recovery}` (predation.rs, re-exported at the crate root).

use std::f64::consts::FRAC_PI_2;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::controller::{Actions, Controller, Response};
use crate::{
    Activity, Animal, Command, Fauna, FaunaConfig, Founder, Hunter, Pose, StartingStores,
    StrikePhase,
};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Flora, Site};

pub(super) const VOXEL_M: f64 = 0.25;
/// Soil fills `y = 1..=2`, so every column's support face is `y = 2`.
pub(super) const SURFACE_Y: u32 = 2;
/// The standing surface, metres: the top of `y = 2`.
pub(super) const SURFACE_M: f64 = 3.0 * VOXEL_M;
/// The lane every strike case runs along, and where its striker stands, facing `+x`.
pub(super) const LANE_Z: f64 = 1.5;
pub(super) const STRIKER_X: f64 = 2.0;
/// A prey's centre this far ahead of a striker's centre is inside the strike capsule on
/// any reading of where the jaw is: its near edge is ahead of the jaw (at most half the
/// 1 m body ahead of the centre) and its far edge within the 0.6 m reach of it.
pub(super) const IN_CAPSULE_M: f64 = 0.75;
/// Facing `+x` (headings run from `+z` toward `+x`).
pub(super) const EAST: f64 = FRAC_PI_2;
pub(super) const WEST: f64 = -FRAC_PI_2;

/// A flat soil plain 12 m around the ring and 3 m deep, at 0.25 m, with the given clock.
pub(super) fn plain(day: DayClock, seed: u64) -> World {
    let (width, depth) = (48u32, 12u32);
    let mut w = World::empty(VoxelConfig {
        width,
        height: 8,
        depth,
        voxel_m: VOXEL_M,
        seed,
        day,
        ..VoxelConfig::default()
    });
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=SURFACE_Y {
                w.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    w
}

/// The plain held at one hour of the day (its tick is never advanced by a case that
/// uses it this way).
pub(super) fn at_hour(start_hour: f64, seed: u64) -> World {
    plain(
        DayClock {
            start_hour,
            ..DayClock::default()
        },
        seed,
    )
}

/// The plain with the day clock off: every rhythm awake.
pub(super) fn clock_off(seed: u64) -> World {
    plain(
        DayClock {
            mode: DayMode::Off,
            ..DayClock::default()
        },
        seed,
    )
}

/// The default table with the lanternjaw awake at every hour, so a case can pick the
/// prey's state by the hour alone and still have a striker.
pub(super) fn hunting_config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    c.founders[Founder::Lanternjaw.index()].active = Activity::Always;
    c
}

/// A lanternjaw hungry enough to strike and not full enough to lie down (its satiated
/// rest starts at 0.9 of its cap).
pub(super) const HUNTING: StartingStores = StartingStores {
    body: 1.0,
    reserve: 0.6,
};

/// Introduce one founder anywhere on the plain and return its id.
pub(super) fn introduce(
    fauna: &mut Fauna,
    world: &World,
    founder: Founder,
    stores: StartingStores,
) -> u64 {
    assert!(
        fauna.apply(
            world,
            Command::IntroduceFounder {
                x: 8,
                z: 6,
                founder,
                stores,
                heading_rad: 0.0,
            }
        ),
        "introducing a {}",
        founder.name()
    );
    fauna.animals.last().expect("just introduced").id
}

pub(super) fn animal(fauna: &Fauna, id: u64) -> Animal {
    *fauna.view().animal(id).expect("alive")
}

pub(super) fn animal_mut(fauna: &mut Fauna, id: u64) -> &mut Animal {
    fauna
        .animals
        .iter_mut()
        .find(|a| a.id == id)
        .expect("alive")
}

/// Put a body at `(x_m, z_m)` facing `heading_rad`, its support site following the pose.
pub(super) fn place(fauna: &mut Fauna, id: u64, x_m: f64, z_m: f64, heading_rad: f64) {
    let a = animal_mut(fauna, id);
    a.pose = Pose {
        x: x_m,
        z: z_m,
        heading_rad,
    };
    a.site = Site {
        x: (x_m / VOXEL_M).floor() as u32,
        y: SURFACE_Y,
        z: (z_m / VOXEL_M).floor() as u32,
    };
}

/// The point `ahead_m` in front of `id`'s centre along its heading.
pub(super) fn ahead_of(fauna: &Fauna, id: u64, ahead_m: f64) -> (f64, f64) {
    let p = animal(fauna, id).pose;
    (
        p.x + ahead_m * p.heading_rad.sin(),
        p.z + ahead_m * p.heading_rad.cos(),
    )
}

/// A controller holding `forward` and a feed the test switches on and off.
pub(super) struct Toggle {
    pub forward: f64,
    pub feed: Arc<AtomicBool>,
}

impl Controller for Toggle {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        let feed = if self.feed.load(Ordering::Relaxed) {
            1.0
        } else {
            0.0
        };
        Response::Bounded(Actions {
            forward: self.forward,
            feed,
            ..Actions::REST
        })
    }
    fn reset(&mut self) {}
}

/// Install a [`Toggle`] on `id` and hand back its feed switch, set to `feed`.
pub(super) fn drive(fauna: &mut Fauna, id: u64, forward: f64, feed: bool) -> Arc<AtomicBool> {
    let switch = Arc::new(AtomicBool::new(feed));
    fauna.controllers.set(
        id,
        Box::new(Toggle {
            forward,
            feed: switch.clone(),
        }),
    );
    switch
}

pub(super) fn hunter(fauna: &Fauna) -> Hunter {
    fauna
        .config
        .founder(Founder::Lanternjaw)
        .hunter
        .expect("the lanternjaw is a hunter")
}

pub(super) fn phase(fauna: &Fauna, id: u64) -> Option<StrikePhase> {
    fauna
        .view()
        .animal(id)
        .and_then(|a| a.strike)
        .map(|s| s.phase)
}

pub(super) fn strikes(fauna: &Fauna) -> u64 {
    fauna.ledger.strikes_by_founder[Founder::Lanternjaw.index()]
}

pub(super) fn hits(fauna: &Fauna) -> u64 {
    fauna.ledger.hits_by_founder[Founder::Lanternjaw.index()]
}

pub(super) fn dodged(fauna: &Fauna) -> u64 {
    fauna.ledger.dodges_by_founder[Founder::Lanternjaw.index()]
}

/// Lanternjaws alive.
pub(super) fn lanternjaws(fauna: &Fauna) -> usize {
    fauna
        .animals
        .iter()
        .filter(|a| a.founder == Some(Founder::Lanternjaw))
        .count()
}

/// Step until `done` holds, at most `max` ticks; the number of ticks stepped, or `None`.
pub(super) fn step_until(
    fauna: &mut Fauna,
    world: &World,
    flora: &mut Flora,
    max: u32,
    mut done: impl FnMut(&Fauna) -> bool,
) -> Option<u32> {
    for t in 1..=max {
        fauna.step_with(world, flora, 1);
        if done(fauna) {
            return Some(t);
        }
    }
    None
}

pub(super) fn assert_closes(fauna: &Fauna, when: &str) {
    let v = fauna.view();
    for (held, expected, what) in [
        (v.organic(), v.ledger.expected_organic(), "organic"),
        (v.mineral(), v.ledger.expected_mineral(), "mineral"),
        (v.energy(), v.ledger.expected_energy(), "energy"),
    ] {
        assert!(
            (held - expected).abs() <= 1e-12 * held.abs().max(1e-6),
            "{when}: fauna {what} holds {held}, the ledger expects {expected}"
        );
    }
}
