//! **YOUNG-ROAM law tests** (the brief `design/handoffs/young-roam-2026-10-12.md`, "Checks").
//!
//! Mechanisms on hand-built fixtures, every number read from the config; no tuned default,
//! ratio or verdict is asserted. Nothing steps past a few hundred ticks.

#[path = "predation_fixture.rs"]
#[allow(dead_code)]
mod fixture;

#[allow(unused_imports)]
use super::*;
use crate::{Activity, Fauna, FaunaConfig, Founder, StartingStores};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use fixture::{EAST, SURFACE_Y, VOXEL_M, animal_mut, drive, hunting_config, introduce, place};

const LS: Founder = Founder::Loftstrider;

/// Arena columns and rows (0.25 m voxels): 40 m around the ring, 30 m deep.
const W: u32 = 160;
const D: u32 = 120;
const RING: f64 = W as f64 * VOXEL_M;
const MID_Z: f64 = D as f64 * VOXEL_M / 2.0;

/// A flat soil arena, the day clock off.
fn arena(seed: u64) -> World {
    let mut w = World::empty(VoxelConfig {
        width: W,
        height: 12,
        depth: D,
        voxel_m: VOXEL_M,
        seed,
        day: cubarium_voxel::weather::DayClock { mode: cubarium_voxel::weather::DayMode::Off, ..Default::default() },
        ..VoxelConfig::default()
    });
    for z in 0..D {
        for x in 0..i64::from(W) {
            for y in 1..=SURFACE_Y {
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w
}

fn config() -> FaunaConfig {
    let mut c = hunting_config();
    for f in Founder::ALL {
        c.founders[f.index()].active = Activity::Always;
        // No stop-and-go pauses: a driven body walks every tick.
        c.founders[f.index()].cadence = crate::Cadence::default();
    }
    c
}

fn world(seed: u64) -> (World, Fauna) {
    let w = arena(seed);
    let mut f = Fauna::new(config());
    f.world_geometry = Some((RING, VOXEL_M));
    f.tick = 1000;
    (w, f)
}

fn index(f: &Fauna, id: u64) -> usize {
    f.animals.iter().position(|a| a.id == id).expect("a live body")
}

// ------------------------------------------------------------------ step 1: young pace

#[test]
fn a_young_bodys_cruise_is_its_own_lengths_never_under_the_floor_and_an_adults_is_its_lineages() {
    let c = config();
    for founder in Founder::ALL {
        let p = c.founder(founder);
        let adult = c.manifest(founder).cruise_m_per_s;
        let birth = p.core.birth_body;
        // An adult, at and past its adult body: the lineage's cruise.
        for body in [birth, p.core.body_max.max(birth)] {
            assert_eq!(c.body_manifest(founder, body).cruise_m_per_s, adult, "{} adult", founder.name());
        }
        // A young body: its own length's, cube root of its body over the adult's.
        let floor = p.young.pace_floor;
        for share in [0.9, 0.5, 0.2, 1e-3] {
            let body = share * birth;
            let own = (body / birth).cbrt();
            let want = adult * own.max(floor).min(1.0);
            let got = c.body_manifest(founder, body).cruise_m_per_s;
            assert!((got - want).abs() <= 1e-12 * adult.max(1.0), "{} at {share}: {got} vs {want}", founder.name());
            assert!(got <= adult, "never faster than the adult");
        }
    }
    // The floor binds a tiny body; a floor of 1 is the adult's pace at any size.
    let mut c = config();
    let p = *c.founder(LS);
    let tiny = 1e-6 * p.core.birth_body;
    let adult = c.manifest(LS).cruise_m_per_s;
    assert!((c.body_manifest(LS, tiny).cruise_m_per_s - p.young.pace_floor * adult).abs() <= 1e-12);
    c.founders[LS.index()].young.pace_floor = 1.0;
    assert_eq!(c.body_manifest(LS, tiny).cruise_m_per_s, adult);
}

#[test]
fn a_driven_calf_moves_at_its_own_lengths_pace_and_a_driven_adult_at_its_lineages() {
    let (w, mut f) = world(1);
    let birth = f.config.founder(LS).core.birth_body;
    let adult = introduce(&mut f, &w, LS, StartingStores::FULL);
    let calf = introduce(&mut f, &w, LS, StartingStores::FULL);
    animal_mut(&mut f, adult).body = birth;
    animal_mut(&mut f, calf).body = 0.2 * birth;
    place(&mut f, adult, 5.0, MID_Z - 4.0, EAST);
    place(&mut f, calf, 5.0, MID_Z + 4.0, EAST);
    drive(&mut f, adult, 1.0, false);
    drive(&mut f, calf, 1.0, false);
    let mut flora = cubarium_voxel_flora::Flora::new(cubarium_voxel_flora::FloraConfig::default());
    let (mut top_adult, mut top_calf) = (0.0f64, 0.0f64);
    for _ in 0..60 {
        f.step(&w, &mut flora);
        top_adult = top_adult.max(f.animals[index(&f, adult)].speed_m_s);
        top_calf = top_calf.max(f.animals[index(&f, calf)].speed_m_s);
    }
    let cruise = f.config.manifest(LS).cruise_m_per_s;
    let scale = crate::roam::pace_scale(f.config.founder(LS), 0.2 * birth);
    assert!(scale < 1.0);
    assert!((top_adult - cruise).abs() <= 0.02 * cruise, "adult {top_adult} vs cruise {cruise}");
    assert!((top_calf - scale * cruise).abs() <= 0.02 * cruise, "calf {top_calf} vs {}", scale * cruise);
}
