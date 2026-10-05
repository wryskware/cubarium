//! **Runner support** (turf-establish, 2026-10-05): a runner daughter tied to its parent
//! ([`crate::Stand::support`]) is paid `runner_support_share` of its shortfall — upkeep less
//! income — out of the parent's reserve above the parent's donor floor, booked, until it is
//! established (past seedling size and paying its upkeep at full daylight), its time runs
//! out, or the parent is gone. A child of `step`, so a case hands `support_runners` the
//! light, moisture and climate it chooses.

use super::*;
use crate::{Command, Flora, FloraConfig, Species, Support};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};

const TURF: Species = Species::Springturf;

fn slab() -> World {
    let mut w = World::empty(VoxelConfig {
        width: 6,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 5,
        ..VoxelConfig::default()
    });
    for x in 0..6i64 {
        for y in 1..=2u32 {
            let want = 0.6 * w.config().materials.pore_capacity(Material::Soil) * w.config().voxel_volume();
            w.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: want });
            w.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
        }
    }
    w
}

/// A full-grown parent on face 1 and a newborn-sized daughter on face 2, tied to it for
/// `left_s`. Returns the layer and the two stands' indices.
fn pair(left_s: f64) -> (Flora, usize, usize) {
    let world = slab();
    let mut flora = Flora::new(FloraConfig::default());
    let sc = flora.config.species(TURF).clone();
    let (p, d) = (Site { x: 1, y: 2, z: 0 }, Site { x: 2, y: 2, z: 0 });
    assert!(flora.apply(&world, Command::SeedOnFace { site: p, species: TURF, wood: sc.wood_max }));
    assert!(flora.apply(&world, Command::SeedOnFace { site: d, species: TURF, wood: sc.alive_min }));
    let pi = flora.stands.iter().position(|s| s.site == p).unwrap();
    let di = flora.stands.iter().position(|s| s.site == d).unwrap();
    let parent = flora.stands[pi].id;
    flora.stands[di].support = Some(Support { parent, parent_site: p, left_s });
    (flora, pi, di)
}

fn drink(moisture: f64) -> Drink {
    Drink { moisture, ..Drink::default() }
}

fn climate(daylight: f64) -> Climate {
    Climate { daylight, upkeep: 1.0, night: 1.0, to_dawn_s: None, draw_key: 0, dt: 1.0 }
}

/// The daughter's shortfall at no income: its whole upkeep over `dt`.
fn upkeep(flora: &Flora, di: usize, dt: f64) -> f64 {
    let sc = flora.config.species(TURF);
    let d = flora.stands[di];
    (sc.maintenance * d.wood + (1.0 + sc.build) * sc.senescence * d.foliage) * dt
}

fn totals(flora: &Flora) -> (f64, f64) {
    (flora.stands.iter().map(|s| s.material()).sum(), flora.stands.iter().map(|s| s.mineral).sum())
}

/// With no income (no light) the parent pays the share of the daughter's whole upkeep,
/// reserve to reserve with its share of mineral, booked; nothing is made or lost.
#[test]
fn a_daughter_with_no_income_is_paid_its_shortfall_from_the_parents_reserve() {
    let (mut flora, pi, di) = pair(100.0);
    let share = flora.config.species(TURF).runner_support_share;
    assert!(share > 0.0, "fixture: support pays something");
    let want = share * upkeep(&flora, di, 1.0);
    let (p0, d0) = (flora.stands[pi], flora.stands[di]);
    let before = totals(&flora);
    let n = flora.stands.len();
    let mut light = vec![1.0; n];
    light[di] = 0.0;
    support_runners(&mut flora, &light, &vec![drink(1.0); n], climate(1.0));
    let (p1, d1) = (flora.stands[pi], flora.stands[di]);
    assert!((d1.reserve - d0.reserve - want).abs() <= 1e-15, "paid the share of its shortfall");
    assert!((p0.reserve - p1.reserve - want).abs() <= 1e-15, "out of the parent's reserve");
    let moved = d1.mineral - d0.mineral;
    assert!(moved > 0.0 && (p0.mineral - p1.mineral - moved).abs() <= 1e-15, "with its mineral");
    let l = &flora.ledger;
    assert!((l.runner_support_organic[TURF.index()] - want).abs() <= 1e-15, "booked");
    assert!((l.runner_support_mineral[TURF.index()] - moved).abs() <= 1e-15, "booked");
    let after = totals(&flora);
    assert!((after.0 - before.0).abs() <= 1e-12 && (after.1 - before.1).abs() <= 1e-12, "an internal transfer");
    assert!(d1.support.is_some(), "still tied");
}

/// The parent never goes below its own floor: near it, it pays only what lies above it,
/// and at it, nothing.
#[test]
fn the_parent_never_pays_below_its_own_floor() {
    let (mut flora, pi, di) = pair(100.0);
    let sc = flora.config.species(TURF).clone();
    let floor = sc.donor_reserve_floor * sc.reserve_cap * flora.stands[pi].wood;
    let want = sc.runner_support_share * upkeep(&flora, di, 1.0);
    assert!(want > 0.0, "fixture: a shortfall");
    flora.stands[pi].reserve = floor + 0.25 * want;
    let n = flora.stands.len();
    let mut light = vec![1.0; n];
    light[di] = 0.0;
    let d0 = flora.stands[di].reserve;
    support_runners(&mut flora, &light, &vec![drink(1.0); n], climate(0.0));
    assert!(flora.stands[pi].reserve >= floor - 1e-15, "at its floor, not under it");
    assert!((flora.stands[di].reserve - d0 - 0.25 * want).abs() <= 1e-15, "only what lay above it");
    let d1 = flora.stands[di].reserve;
    support_runners(&mut flora, &light, &vec![drink(1.0); n], climate(0.0));
    assert!(flora.stands[pi].reserve >= floor - 1e-15);
    assert_eq!(flora.stands[di].reserve, d1, "nothing more");
}

/// An established daughter — past its seedling size and paying its own upkeep — is weaned
/// and paid nothing; one still a seedling stays tied however well it earns.
#[test]
fn support_ends_once_the_daughter_is_established() {
    let (mut flora, pi, di) = pair(100.0);
    let sc = flora.config.species(TURF).clone();
    let n = flora.stands.len();
    let light = vec![1.0; n];
    let moisture = vec![drink(1.0); n];
    // A well-lit seedling that already pays: still a seedling, still tied.
    {
        let d = &mut flora.stands[di];
        d.foliage = sc.alpha * d.wood;
        d.bin_layers(&flora.config);
    }
    let d = flora.stands[di];
    let mineral = flora.ground.iter().find(|g| g.site == d.site).map_or(flora.config.initial_mineral, |g| g.mineral);
    let pays = |w: f64, p: f64| break_even_cover(&sc, w, p, 1.0, 1.0, monod(&sc, mineral)) >= 1.0;
    assert!(pays(d.wood, d.foliage), "fixture: it pays at full light");
    assert!(d.wood < sc.seedling_below * sc.alive_min, "fixture: a seedling");
    support_runners(&mut flora, &light, &moisture, climate(1.0));
    assert!(flora.stands[di].support.is_some(), "a seedling stays tied");
    // Past the seedling size: weaned, and nothing moves.
    {
        let d = &mut flora.stands[di];
        d.wood = sc.seedling_below * sc.alive_min;
        d.foliage = sc.alpha * d.wood;
        d.bin_layers(&flora.config);
    }
    let d = flora.stands[di];
    assert!(pays(d.wood, d.foliage), "fixture: it pays at full light");
    let (p0, d0) = (flora.stands[pi].reserve, d.reserve);
    support_runners(&mut flora, &light, &moisture, climate(0.0));
    assert!(flora.stands[di].support.is_none(), "weaned");
    assert_eq!(flora.ledger.runner_weaned[TURF.index()], 1);
    assert_eq!((flora.stands[pi].reserve, flora.stands[di].reserve), (p0, d0), "nothing paid");
}

/// The tie ends when its time runs out, and when the parent is gone.
#[test]
fn support_ends_when_its_time_runs_out_or_the_parent_is_gone() {
    let n_of = |f: &Flora| f.stands.len();
    let (mut flora, _, di) = pair(1.5);
    let n = n_of(&flora);
    let mut light = vec![1.0; n];
    light[di] = 0.0;
    support_runners(&mut flora, &light, &vec![drink(1.0); n], climate(1.0));
    assert!(flora.stands[di].support.is_some(), "half a second left");
    support_runners(&mut flora, &light, &vec![drink(1.0); n], climate(1.0));
    assert!(flora.stands[di].support.is_none(), "its time ran out");
    assert_eq!(flora.ledger.runner_support_ended[TURF.index()], 1);

    let (mut flora, pi, di) = pair(100.0);
    flora.stands[pi].id += 1000; // another stand on the parent's face now
    let r0 = flora.stands[di].reserve;
    support_runners(&mut flora, &light, &vec![drink(1.0); n], climate(1.0));
    assert!(flora.stands[di].support.is_none(), "the parent is gone");
    assert_eq!(flora.stands[di].reserve, r0, "nobody paid");
    assert_eq!(flora.ledger.runner_support_ended[TURF.index()], 1);
}
