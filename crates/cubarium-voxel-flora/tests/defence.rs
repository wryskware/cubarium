//! **Induced grazing defence** (`design/handoffs/voxel-lifespan-defence-2026-09-25.md`, B):
//! every bite raises the bitten stand's defence, the defence shrinks the next bite, and
//! it decays once grazing stops. Short test settings; under 200 ticks; no pinned hashes.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Site, Species};

fn flat() -> World {
    let mut world = World::empty(VoxelConfig {
        width: 12,
        height: 6,
        depth: 4,
        voxel_m: 0.25,
        ..VoxelConfig::default()
    });
    for z in 0..4 {
        for x in 0..12 {
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

/// A fast-inducing, fast-relaxing test config: `rate` per second of grazing, `half` s.
fn config(rate: f64, half: f64) -> FloraConfig {
    let mut c = FloraConfig::for_voxel_size(0.25);
    for s in Species::ALL {
        let sc = c.species_mut(s);
        sc.defence_rate_per_s = rate;
        sc.defence_half_life_s = half;
    }
    c
}

/// Two full bloomcrowns, far apart: the grazed one and its fresh neighbour.
fn two_crowns(c: FloraConfig) -> (World, Flora, Site, Site) {
    let world = flat();
    let mut flora = Flora::new(c);
    let wood = flora.view().config.species(Species::Bloomcrown).wood_max * 0.5;
    for x in [2, 9] {
        assert!(flora.apply(
            &world,
            Command::Seed {
                x,
                z: 2,
                species: Species::Bloomcrown,
                wood,
            }
        ));
    }
    let v = flora.view();
    (world, flora.clone(), v.stands[0].site, v.stands[1].site)
}

fn defence(flora: &Flora, site: Site) -> f64 {
    flora.view().stand_at(site).expect("standing").defence
}

/// The shipped defaults: one strength and one half-life for every species, and on.
#[test]
fn the_defaults_are_shared_and_on() {
    let c = FloraConfig::default();
    let first = c.species(Species::ALL[0]);
    assert!(first.defence_rate_per_s > 0.0 && first.defence_half_life_s > 0.0);
    for s in Species::ALL {
        assert_eq!(c.species(s).defence_rate_per_s, first.defence_rate_per_s);
        assert_eq!(c.species(s).defence_half_life_s, first.defence_half_life_s);
    }
}

/// Repeated bites on one stand take strictly less each time, a fresh neighbour gives the
/// whole bite, and the grazed stand's defence has risen.
#[test]
fn repeated_bites_shrink_and_a_fresh_neighbour_gives_a_full_bite() {
    let (_, mut flora, grazed, fresh) = two_crowns(config(0.5, 3600.0));
    let want = 1e-5;
    let mut last = f64::INFINITY;
    for bite in 0..150 {
        let got = flora.take_foliage(grazed, want).expect("a bite").organic;
        assert!(got > 0.0 && got < last, "bite {bite}: {got} after {last}");
        last = got;
    }
    assert!(
        last < 0.25 * want,
        "150 ticks of grazing cut the bite to {last}"
    );
    assert!(defence(&flora, grazed) > 0.75);
    assert_eq!(defence(&flora, fresh), 0.0);
    let full = flora.take_foliage(fresh, want).expect("a bite").organic;
    assert!(
        (full - want).abs() < 1e-18,
        "the neighbour gave {full} of {want}"
    );
}

/// The defence decays toward zero once grazing stops, at its half-life.
#[test]
fn the_defence_decays_after_grazing_stops() {
    let (mut world, mut flora, grazed, _) = two_crowns(config(0.5, 2.0));
    for _ in 0..100 {
        flora.take_foliage(grazed, 1e-5);
    }
    let start = defence(&flora, grazed);
    assert!(start > 0.5);
    let mut last = start;
    for _ in 0..100 {
        flora.step(&mut world);
        let d = defence(&flora, grazed);
        assert!(d < last, "falling");
        last = d;
    }
    // 100 ticks = 5 s = 2.5 half-lives: about 18 % of the start is left.
    let ratio = last / start;
    assert!((ratio - 0.5f64.powf(2.5)).abs() < 0.02, "left {ratio}");
}

/// Mass and energy close with the defence on: bites and ticks interleaved, the plant
/// ledger's three residuals stay at rounding.
#[test]
fn the_plant_ledger_closes_with_the_defence_on() {
    let (mut world, mut flora, grazed, fresh) = two_crowns(config(0.5, 30.0));
    for t in 0..150 {
        flora.take_foliage(grazed, 2e-4);
        if t % 3 == 0 {
            flora.take_foliage(fresh, 2e-4);
        }
        flora.step(&mut world);
    }
    let v = flora.view();
    for (got, what) in [
        (v.organic() - v.ledger.expected_organic(), "organic"),
        (v.mineral() - v.ledger.expected_mineral(), "mineral"),
        (v.energy() - v.ledger.expected_energy(), "energy"),
    ] {
        assert!(got.abs() <= 1e-9, "{what} residual {got:e}");
    }
    assert!(v.ledger.consumed_organic_out > 0.0);
}
