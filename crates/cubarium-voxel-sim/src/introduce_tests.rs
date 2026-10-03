//! P1 (predation substrate), tests first: the introduction control and the founding egg
//! bank (`design/predation-and-flight-2026-09-28.md` §2.9, decision 4 as amended; brief
//! P1 case 13). Red by design until the implementation commit adds, in `habitat.rs`:
//!
//! - `introduce_lanternjaws(world: &World, flora: &Flora, fauna: &mut Fauna, n: usize)
//!   -> Result<Vec<u64>, String>`: `n` adult lanternjaws with full stores, each on a
//!   face of a **prey component** (a walkable component of the lanternjaw's route map
//!   holding at least one living prey body); the ids placed, or why none could be;
//! - `bury_founding_clutches(world: &World, flora: &Flora, fauna: &mut Fauna, clutches:
//!   usize) -> Result<usize, String>`: the founding bank, buried in prey components; the
//!   eggs buried, or why none could be;
//!
//! and in the fauna crate `Founder::Lanternjaw`.

use cubarium_voxel::{Command as WorldCommand, Config, Material, World};
use cubarium_voxel_fauna::{Command as FaunaCommand, Fauna, Founder, StartingStores};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};

use crate::habitat::{bury_founding_clutches, introduce_lanternjaws};

const WIDTH: u32 = 48;
const DEPTH: u32 = 8;
const HEIGHT: u32 = 12;

/// A soil plain at 0.25 m cut into two walkable halves by two rock walls that run the
/// full height of the world (no face on top, so nothing climbs over): `x = 2..=23` and
/// `x = 26..=47`.
fn two_halves() -> World {
    let mut world = World::empty(Config {
        width: WIDTH,
        height: HEIGHT,
        depth: DEPTH,
        voxel_m: 0.25,
        ..Config::default()
    });
    for z in 0..DEPTH {
        for x in 0..i64::from(WIDTH) {
            let wall = matches!(x, 0 | 1 | 24 | 25);
            let top = if wall { HEIGHT - 1 } else { 2 };
            for y in 1..=top {
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: if wall { Material::Rock } else { Material::Soil },
                });
            }
        }
    }
    world
}

fn in_prey_half(site: Site) -> bool {
    (2..=23).contains(&site.x)
}

fn lanternjaws(fauna: &Fauna) -> usize {
    fauna
        .view()
        .animals
        .iter()
        .filter(|a| a.founder == Some(Founder::Lanternjaw))
        .count()
}

/// Case 13: the introduction places N adults, full, only where prey live; the founding
/// bank is buried only there; with no prey both refuse and say why; and over 200 ticks
/// nothing else adds a lanternjaw.
#[test]
fn lanternjaws_arrive_only_in_a_prey_component_and_only_by_the_controls() {
    let world = two_halves();
    let flora = Flora::new(FloraConfig::for_voxel_size(0.25));

    // No prey anywhere: refused, with a reason, and nothing placed.
    let mut empty = Fauna::new(Default::default());
    let why = introduce_lanternjaws(&world, &flora, &mut empty, 2).expect_err("no prey");
    assert!(!why.trim().is_empty());
    let why = bury_founding_clutches(&world, &flora, &mut empty, Founder::Lanternjaw, 2).expect_err("no prey");
    assert!(!why.trim().is_empty());
    assert_eq!(lanternjaws(&empty), 0);
    assert!(empty.view().clutches.is_empty());

    // Prey in the first half only.
    let mut fauna = Fauna::new(Default::default());
    for (x, z) in [(8, 3), (14, 5)] {
        assert!(fauna.apply(
            &world,
            FaunaCommand::IntroduceFounderOnFace {
                site: Site { x, y: 2, z },
                founder: Founder::Browser,
                stores: StartingStores::FULL,
                heading_rad: 0.0,
            }
        ));
    }
    let ids = introduce_lanternjaws(&world, &flora, &mut fauna, 3).expect("prey live here");
    assert_eq!(ids.len(), 3);
    let phys = *fauna.config().founder(Founder::Lanternjaw);
    for &id in &ids {
        let a = fauna.view().animal(id).expect("placed");
        assert_eq!(a.founder, Some(Founder::Lanternjaw));
        assert!(in_prey_half(a.site), "a lanternjaw at {:?}, away from the prey", a.site);
        assert_eq!(a.body, phys.core.body_max, "an adult");
        // The minimal gut: the ordinary arrival stores; it wants food from its s0 stomach.
        let r = fauna.config().founding.arrival_reserve;
        assert_eq!(a.reserve, r * phys.core.reserve_of(a.body), "at the arrival reserve");
        assert_ne!(a.state, cubarium_voxel_fauna::State::Asleep, "awake");
    }
    let eggs = bury_founding_clutches(&world, &flora, &mut fauna, Founder::Lanternjaw, 2).expect("prey live here");
    assert!(eggs > 0);
    let buried: Vec<_> = fauna
        .view()
        .clutches
        .iter()
        .filter(|c| c.lineage == Founder::Lanternjaw)
        .copied()
        .collect();
    assert_eq!(
        buried.iter().map(|c| c.count as usize).sum::<usize>(),
        eggs
    );
    for c in &buried {
        assert!(in_prey_half(c.site), "a clutch at {:?}, away from the prey", c.site);
    }

    // No spontaneous appearance: nothing but a hatching or a control adds one, and with
    // the prey standing still no egg hears a footfall.
    let mut flora = flora;
    let eggs_now = fauna.view().eggs_by_founder(Founder::Lanternjaw);
    for t in 0..200 {
        fauna.step_with(&world, &mut flora, 1);
        assert_eq!(lanternjaws(&fauna), 3, "tick {t}");
        assert_eq!(fauna.view().eggs_by_founder(Founder::Lanternjaw), eggs_now);
    }
}
