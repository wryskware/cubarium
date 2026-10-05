//! The plant clock (`FloraConfig::step_every`): a run stepped every `n` ticks at `n · DT`
//! against the per-tick run over the same span, on hand-built plots. Nothing here pins a
//! tuned number: every expectation is the other arm, the config, or a sampling bound.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, Materials, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Site, Species};

/// A strip of soil two voxels deep, `pore` of its capacity wet, no rain.
fn plot(width: u32, pore: f64) -> World {
    let config = VoxelConfig {
        width,
        height: 8,
        depth: 2,
        voxel_m: 1.0,
        seed: 7,
        rain_m_per_s: 0.0,
        ..VoxelConfig::default()
    };
    let mut world = World::empty(config.clone());
    let cap = Materials::DEFAULT.pore_capacity(Material::Soil) * world.config().voxel_volume();
    for z in 0..config.depth {
        for x in 0..config.width as i64 {
            for y in 1..=2 {
                if pore > 0.0 {
                    world.apply(WorldCommand::AddWater {
                        x,
                        y,
                        z,
                        volume_m3: pore * cap,
                    });
                }
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

fn with_clock(mut config: FloraConfig, n: u32) -> FloraConfig {
    config.step_every = n;
    config
}

fn run(flora: &mut Flora, world: &mut World, ticks: u64) {
    for _ in 0..ticks {
        world.step();
        flora.step_clocked(world, 1);
    }
}

fn assert_closed(flora: &Flora, world: &World, when: &str) {
    let v = flora.view();
    let o = v.organic() - v.ledger.expected_organic();
    let n = v.mineral() - v.ledger.expected_mineral();
    let e = v.energy() - v.ledger.expected_energy();
    assert!(o.abs() <= 1e-9 * v.organic().abs().max(1.0), "{when}: organic residual {o}");
    assert!(n.abs() <= 1e-9 * v.mineral().abs().max(1.0), "{when}: mineral residual {n}");
    assert!(e.abs() <= 1e-9 * v.energy().abs().max(1.0), "{when}: energy residual {e}");
    let water = world.view().stored_m3() - world.view().ledger.expected_stored();
    assert!(water.abs() < 1e-9, "{when}: water residual {water}");
    assert_eq!(
        v.ledger.transpired_m3,
        world.view().ledger.transpiration_out,
        "{when}: the plants' drink is the world's transpiration"
    );
}

/// Material held in stands: wood, foliage, reserve and what each saves toward a package.
fn standing(flora: &Flora) -> f64 {
    flora
        .view()
        .stands
        .iter()
        .map(|s| s.wood + s.foliage + s.reserve + s.parcel)
        .sum()
}

fn seeded(world: &World, config: FloraConfig) -> Flora {
    let mut flora = Flora::new(config);
    for (x, species, wood) in [
        (1i64, Species::Bloomcrown, 0.4),
        (4, Species::Umbrellafrond, 0.5),
        (7, Species::Bloomcrown, 0.05),
        (9, Species::Umbrellafrond, 0.1),
    ] {
        for z in 0..2 {
            assert!(flora.apply(world, Command::Seed { x, z, species, wood }));
        }
    }
    flora
}

#[test]
fn the_clocked_layer_steps_once_every_step_every_ticks_and_moves_its_tick_every_tick() {
    let n = 4;
    let mut world = plot(12, 0.5);
    let mut flora = seeded(&world, with_clock(FloraConfig::default(), n));
    let before = standing(&flora);
    for k in 1..n as u64 {
        world.step();
        flora.step_clocked(&mut world, 1);
        assert_eq!(flora.tick(), k);
        assert_eq!(standing(&flora), before, "tick {k} of {n} is not a plant step");
        assert_eq!(flora.view().ledger.transpired_m3, 0.0, "nothing drinks between plant steps");
    }
    world.step();
    flora.step_clocked(&mut world, 1);
    assert_eq!(flora.tick(), n as u64);
    assert_ne!(standing(&flora), before, "the n-th tick is the plant step");
    assert!(flora.view().ledger.transpired_m3 > 0.0);
}

/// Growth, upkeep, drink and the ledger over the same span: the coarse run is a first-order
/// integration of the same per-second rates, so it lands within the step's own
/// discretisation of the per-tick run, and every ledger closes in both.
#[test]
fn a_coarse_plant_step_grows_what_the_per_tick_one_grows_and_conserves_every_ledger() {
    let n = 5;
    let span = 40 * n as u64;
    let mut runs = Vec::new();
    for every in [1, n] {
        let mut world = plot(12, 0.5);
        let mut flora = seeded(&world, with_clock(FloraConfig::default(), every));
        let start = standing(&flora);
        run(&mut flora, &mut world, span);
        assert_closed(&flora, &world, &format!("step_every {every}"));
        runs.push((standing(&flora) - start, flora.view().ledger.transpired_m3, flora.view().ledger.fixed_in));
    }
    let (fine, coarse) = (runs[0], runs[1]);
    assert!(fine.2 > 0.0 && fine.1 > 0.0, "the per-tick plot neither grew nor drank");
    // The coarse step holds each stand's state for `n` ticks where the per-tick run lets it
    // move: a first-order error, measured against the gross flow it integrates. A
    // twentieth of that gross flow is far outside the step's discretisation (about 1.5 %
    // here) and far inside a broken clock (a lost or doubled step is 20 %, a dt off by `n`
    // is 400 %).
    let close = |a: f64, b: f64, gross: f64, what: &str| {
        let tol = 0.05 * gross.abs();
        assert!((a - b).abs() <= tol, "{what}: per tick {a}, coarse {b}, tolerance {tol}");
    };
    close(fine.2, coarse.2, fine.2, "fixed");
    close(fine.1, coarse.1, fine.1, "transpired");
    close(fine.0, coarse.0, fine.2, "standing material");
}

/// Seed attrition is a per-check chance off each seed's age: on the coarse clock every
/// banked site is still checked on its own tick, so the deaths over the same span agree
/// within the binomial spread of the draws.
#[test]
fn seed_attrition_dies_at_the_same_rate_on_either_clock() {
    let span = 2 * 600 + 7;
    let mut died = Vec::new();
    for every in [1u32, 5] {
        // Dry soil: the gates shut, so attrition is the only way a seed leaves the bank.
        let mut world = plot(24, 0.0);
        let mut config = with_clock(FloraConfig::default(), every);
        config.species_mut(Species::Bloomcrown).seed_attrition_per_s = 0.02;
        let check = config.seed_check_s;
        let rate = config.species(Species::Bloomcrown).seed_attrition_per_s;
        let mut flora = Flora::new(config);
        for x in 0..24 {
            for z in 0..2 {
                assert!(flora.deposit_seeds(Site { x, y: 2, z }, Species::Bloomcrown, 2.0, 0.0, 0.0));
            }
        }
        let banked: u64 = flora.view().ground.iter().map(|g| g.seeds.len() as u64).sum();
        assert!(banked > 0);
        run(&mut flora, &mut world, span);
        assert_closed(&flora, &world, &format!("step_every {every}"));
        died.push((flora.view().ledger.seeds_died[Species::Bloomcrown.index()], rate, check));
    }
    let (a, b) = (died[0].0 as f64, died[1].0 as f64);
    assert!(a > 0.0, "no seed died in the per-tick run");
    // Each arm's count is a sum of Bernoulli draws with mean `a`; their difference has a
    // standard deviation at most `sqrt(2a)`. Five of them.
    let tol = 5.0 * (2.0 * a.max(b)).sqrt();
    assert!((a - b).abs() <= tol, "seed deaths: per tick {a}, coarse {b}, tolerance {tol}");
}

/// The coarse drink asks for `n` ticks of demand at once and still never takes a root box
/// below its wilting point: the withdrawal is bounded by what each cell holds.
#[test]
fn a_coarse_drink_never_takes_a_root_box_below_its_wilting_point() {
    let mats = Materials::DEFAULT;
    let wp = mats.wilting_point(Material::Soil);
    let cap = mats.pore_capacity(Material::Soil);
    let mut world = plot(12, wp + 0.02);
    let mut config = with_clock(FloraConfig::default(), 50);
    // A thirst far past what the box holds in one coarse step.
    config.species_mut(Species::Bloomcrown).transpiration_m3_per_s = 10.0;
    let mut flora = seeded(&world, config);
    let held: f64 = (0..12i64)
        .flat_map(|x| (1..=2u32).flat_map(move |y| (0..2u32).map(move |z| (x, y, z))))
        .map(|(x, y, z)| ((world.view().pore_at(x, y, z) - wp).max(0.0)) * cap * world.config().voxel_volume())
        .sum();
    run(&mut flora, &mut world, 200);
    for x in 0..12i64 {
        for y in 1..=2 {
            for z in 0..2 {
                let p = world.view().pore_at(x, y, z);
                assert!(p >= wp - 1e-12, "({x},{y},{z}) at pore {p}, under the wilting point {wp}");
            }
        }
    }
    let drank = flora.view().ledger.transpired_m3;
    assert!(drank > 0.0);
    assert!(drank <= held + 1e-12, "drank {drank} of the {held} above the wilting point");
    assert_closed(&flora, &world, "after the drought");
}
