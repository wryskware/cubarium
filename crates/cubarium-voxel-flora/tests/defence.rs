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

/// A fast-inducing, fast-relaxing test config: `rise` per bite, `half` s.
fn config(rise: f64, half: f64) -> FloraConfig {
    let mut c = FloraConfig::for_voxel_size(0.25);
    for s in Species::ALL {
        let sc = c.species_mut(s);
        sc.defence_per_bite = rise;
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

/// Repeated bites on one stand take strictly less each time, a fresh neighbour gives the
/// whole bite, and the grazed stand's defence has risen.
#[test]
fn repeated_bites_shrink_and_a_fresh_neighbour_gives_a_full_bite() {
    let (_, mut flora, grazed, fresh) = two_crowns(config(0.025, 3600.0));
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
    let (mut world, mut flora, grazed, _) = two_crowns(config(0.025, 2.0));
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
    let (mut world, mut flora, grazed, fresh) = two_crowns(config(0.025, 30.0));
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

/// The audit round's clock (`design/handoffs/voxel-audit-round-2026-09-28.md` §4): with a
/// hand-set strength and half-life, the defence switches on after a number of bites the
/// strength fixes, and relaxes at the half-life it is given.
#[test]
fn the_defence_switches_on_by_bites_and_relaxes_at_the_half_life() {
    let (rise, half) = (0.02, 40.0);
    let (mut world, mut flora, grazed, _) = two_crowns(config(rise, half));
    let mut bites = 0u32;
    while defence(&flora, grazed) < 0.8 {
        flora.take_foliage(grazed, 1e-7).expect("a bite");
        bites += 1;
        assert!(bites < 5000, "the defence never switched on");
    }
    // Each bite raises the defence by `rise` of the headroom, so 0.8 takes about
    // ln(0.2) / ln(1 - rise) bites (at least that many, a few more for rounding).
    let want = (0.2f64.ln() / (1.0 - rise).ln()).ceil();
    assert!(
        (f64::from(bites) - want).abs() <= 0.25 * want,
        "0.8 after {bites} bites, expected about {want}"
    );
    let start = defence(&flora, grazed);
    for _ in 0..200 {
        flora.step(&mut world);
    }
    // 200 ticks = 10 s.
    let ratio = defence(&flora, grazed) / start;
    assert!(
        (ratio - 0.5f64.powf(10.0 / half)).abs() < 1e-4,
        "left {ratio} after 10 s at half-life {half}"
    );
}
