//! Package SM (`design/handoffs/voxel-plant-viability-2026-09-23.md`, "Follow-up round"):
//! the D5 seed mark shows only a **recent** landing (within [`crate::SEED_MARK_RECENT_S`])
//! or a site **about to sprout** — armed at a check whose gates passed, germinating at the
//! next one. Kept apart from `seeds.rs`'s own tests so the two packages editing that file
//! do not collide.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, TICK_HZ, World};

use crate::seeds::{add_cohort, check_period, check_phase, package_of};
use crate::{Flora, FloraConfig, Ground, SEED_MARK_RECENT_S, Site, Species};

const SITE: Site = Site { x: 1, y: 2, z: 0 };

/// Three soil columns one slab deep at pore `pore`: bedrock at `y = 0`, soil at `y = 1..=2`,
/// support faces at `y = 2` in open sky.
fn slab(pore: f64) -> World {
    let mut w = World::empty(VoxelConfig {
        width: 3,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 11,
        ..VoxelConfig::default()
    });
    let cap = Material::Soil.pore_capacity() * w.config().voxel_volume();
    for x in 0..3i64 {
        for y in 1..=2u32 {
            if pore > 0.0 {
                w.apply(WorldCommand::AddWater {
                    x,
                    y,
                    z: 0,
                    volume_m3: pore * cap,
                });
            }
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

/// Land one bloomcrown seed on [`SITE`] at the flora's current tick, through the landing
/// path itself (`add_cohort`), booked as seeded material so the ledgers stay closed.
fn land(flora: &mut Flora) {
    let sc = flora.config.bloomcrown.clone();
    let organic = package_of(&sc);
    let mineral = sc.n_tissue * organic;
    let gi = match flora.ground.binary_search_by_key(&SITE, |g| g.site) {
        Ok(i) => i,
        Err(i) => {
            flora.ground.insert(i, Ground::new(SITE, 1.0));
            flora.ledger.seeded_mineral_in += 1.0;
            i
        }
    };
    let tick = flora.tick;
    add_cohort(
        &mut flora.ground[gi],
        Species::Bloomcrown,
        organic,
        mineral,
        tick,
        &sc,
    );
    flora.ledger.seeded_organic_in += organic;
    flora.ledger.seeded_mineral_in += mineral;
    flora.ledger.seeded_energy_in += sc.energy_density * organic;
    // A fresh wheel: rebuilt from the ground at the next step.
    flora.bank_wheel.clear();
}

/// The first tick after `after` on which [`SITE`] is checked.
fn next_check(flora: &Flora, after: u64) -> u64 {
    let period = check_period(&flora.config);
    let phase = check_phase(SITE, period);
    let t = after - after % period + phase;
    if t > after { t } else { t + period }
}

fn run_to(flora: &mut Flora, world: &mut World, tick: u64) {
    while flora.tick < tick {
        flora.step(world);
    }
}

fn mark(flora: &Flora) -> Option<Species> {
    flora
        .view()
        .ground_at(SITE)
        .and_then(|g| g.seed_mark(flora.tick))
}

fn config() -> FloraConfig {
    let mut c = FloraConfig::default();
    // These fixtures are about the mark, not about seeds dying.
    c.bloomcrown.seed_attrition_per_s = 0.0;
    c
}

/// A landing marks its site for [`SEED_MARK_RECENT_S`] and no longer: on dry ground,
/// where the gates never pass, the mark goes when the landing stops being recent, while
/// the seed stays banked.
#[test]
fn a_landing_marks_its_site_for_two_minutes_and_then_the_waiting_bank_shows_nothing() {
    let mut world = slab(0.05); // under the wilting point: the water gate is shut
    let mut flora = Flora::new(config());
    flora.tick = 1000;
    land(&mut flora);
    assert_eq!(mark(&flora), Some(Species::Bloomcrown), "just landed");

    let recent = (SEED_MARK_RECENT_S * f64::from(TICK_HZ)).round() as u64;
    run_to(&mut flora, &mut world, 1000 + recent);
    assert_eq!(mark(&flora), Some(Species::Bloomcrown), "still recent");
    flora.step(&mut world);
    assert_eq!(
        mark(&flora),
        None,
        "past the window, and the gates are shut"
    );
    let g = flora.view().ground_at(SITE).expect("banked");
    assert!(
        g.seed_organic(Species::Bloomcrown) > 0.0,
        "the seed still waits"
    );
    assert_eq!(g.sprouting, None);
}

/// Gates that pass at a check **arm** the site — it shows the mark of the species that
/// will sprout — and the seed germinates at the **next** check, not the one that armed it.
#[test]
fn passing_gates_arm_the_site_at_one_check_and_it_sprouts_at_the_next() {
    let mut world = slab(Material::Soil.field_capacity());
    let mut flora = Flora::new(config());
    land(&mut flora);
    // An hour later the landing is long past recent: only the flag can mark it now.
    flora.tick += 3600 * u64::from(TICK_HZ);
    assert_eq!(
        mark(&flora),
        None,
        "an old landing on its own shows nothing"
    );

    let first = next_check(&flora, flora.tick);
    run_to(&mut flora, &mut world, first);
    assert_eq!(flora.view().ledger.establishments, 0, "armed, not sprouted");
    assert_eq!(
        flora.view().ground_at(SITE).and_then(|g| g.sprouting),
        Some(Species::Bloomcrown)
    );
    assert_eq!(mark(&flora), Some(Species::Bloomcrown), "about to sprout");

    let second = next_check(&flora, first);
    run_to(&mut flora, &mut world, second - 1);
    assert_eq!(flora.view().ledger.establishments, 0, "not between checks");
    flora.step(&mut world);
    assert_eq!(
        flora.view().ledger.establishments,
        1,
        "sprouted at the next check"
    );
    let born = flora.view().stand_at(SITE).expect("a stand");
    assert_eq!(born.species, Species::Bloomcrown);
    assert_eq!(mark(&flora), None, "the seed is spent and the flag cleared");
}

/// An armed site whose gates have shut by its next check does not sprout, and its flag
/// and mark go.
#[test]
fn an_armed_site_whose_gates_shut_before_the_next_check_does_not_sprout() {
    let mut world = slab(Material::Soil.field_capacity());
    let mut flora = Flora::new(config());
    land(&mut flora);
    flora.tick += 3600 * u64::from(TICK_HZ);
    let first = next_check(&flora, flora.tick);
    run_to(&mut flora, &mut world, first);
    assert_eq!(mark(&flora), Some(Species::Bloomcrown), "armed");

    // Dry the root box to the wilting point and below.
    let cap = Material::Soil.pore_capacity() * world.config().voxel_volume();
    for x in 0..3i64 {
        for y in 1..=2u32 {
            world.apply(WorldCommand::WithdrawPore {
                x,
                y,
                z: 0,
                volume_m3: cap,
            });
        }
    }
    let second = next_check(&flora, first);
    run_to(&mut flora, &mut world, second);
    assert_eq!(flora.view().ledger.establishments, 0, "the gate shut");
    assert_eq!(flora.view().ground_at(SITE).and_then(|g| g.sprouting), None);
    assert_eq!(mark(&flora), None);
}
