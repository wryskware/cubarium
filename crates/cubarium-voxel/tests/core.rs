//! Small function tests on tiny `World::empty` fixtures, a few substeps each.
//! `voxel_m` is 1 m in the fixtures, so one "unit" of water is one cubic metre and one
//! full cell.

use cubarium_voxel::generate;
use cubarium_voxel::{Command, Config, Material, World};

fn cfg(width: u32, height: u32) -> Config {
    Config { width, height, depth: 1, voxel_m: 1.0, seed: 7, water_substeps: 4, ..Config::default() }
}

fn wall(w: &mut World, x: i64, ys: std::ops::RangeInclusive<u32>) {
    for y in ys {
        w.apply(Command::SetMaterial { x, y, z: 0, material: Material::Rock });
    }
}

/// Pour water into a column from the bottom up, as if it had already settled there.
fn pour(w: &mut World, x: i64, mut volume: f64) -> f64 {
    let height = w.config().height;
    let mut added = 0.0;
    for y in 0..height {
        if volume <= 1e-12 {
            break;
        }
        let got = w.apply(Command::AddWater { x, y, z: 0, volume_m3: volume });
        volume -= got;
        added += got;
    }
    added
}

fn column(w: &World, x: i64) -> f64 {
    let v = w.view();
    (0..w.config().height).map(|y| v.free_at(x, y, 0) as f64).sum()
}

fn residual(w: &World) -> f64 {
    w.view().stored_m3() - w.view().ledger.expected_stored()
}

fn run(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        w.step();
    }
}

// ------------------------------------------------------------------ spill threshold

/// Beds `[0, 1, 0]`: two one-cell sills on a four-column ring, so the water poured into
/// column 0 has to clear a sill to reach column 2.
fn spill(units: f64) -> World {
    let mut w = World::empty(cfg(4, 6));
    wall(&mut w, 1, 1..=1);
    wall(&mut w, 3, 1..=1);
    pour(&mut w, 0, units);
    run(&mut w, 20);
    w
}

#[test]
fn spill_threshold_below_the_sill_stays_put() {
    let w = spill(0.6);
    assert!((column(&w, 0) - 0.6).abs() < 1e-3, "{}", column(&w, 0));
    assert!(column(&w, 2) < 1e-6, "far column wet: {}", column(&w, 2));
    assert!(residual(&w).abs() < 1e-9);
}

#[test]
fn spill_threshold_just_over_the_sill_spills_and_leaves_the_sides_unequal() {
    let w = spill(1.4);
    // The near column can only hold one unit below the sill top; the rest crosses.
    assert!((column(&w, 0) - 1.0).abs() < 0.02, "near {}", column(&w, 0));
    assert!((column(&w, 2) - 0.4).abs() < 0.02, "far {}", column(&w, 2));
    assert!((w.view().stored_m3() - 1.4).abs() < 1e-6);
    assert!(residual(&w).abs() < 1e-9);
}

#[test]
fn spill_threshold_well_over_the_sill_equalizes_above_it() {
    let w = spill(3.2);
    // Level 2.3: both floors full, then four cells at y = 2 (two basins, two sills)
    // share the remaining 1.2 evenly.
    assert!((column(&w, 0) - 1.3).abs() < 1e-3, "near {}", column(&w, 0));
    assert!((column(&w, 2) - 1.3).abs() < 1e-3, "far {}", column(&w, 2));
    assert!((w.view().free_at(1, 2, 0) as f64 - 0.3).abs() < 1e-3);
    assert!(residual(&w).abs() < 1e-9);
}

// ------------------------------------------------------------------ U-tube

/// Two shafts joined only along the bottom row, on a closed five-column ring.
fn u_tube(offset: i64) -> World {
    let mut w = World::empty(cfg(5, 8));
    wall(&mut w, offset + 1, 2..=7);
    wall(&mut w, offset + 3, 1..=7);
    wall(&mut w, offset + 4, 1..=7);
    pour(&mut w, offset, 4.0);
    run(&mut w, 16);
    w
}

#[test]
fn u_tube_equalizes() {
    let w = u_tube(0);
    let v = w.view();
    // Bottom row (three cells) full, then one unit shared by the two shafts at y = 2.
    for x in 0..3 {
        assert!((v.free_at(x, 1, 0) as f64 - 1.0).abs() < 1e-3, "floor {x}");
    }
    assert!((v.free_at(0, 2, 0) as f64 - 0.5).abs() < 1e-3, "near {}", v.free_at(0, 2, 0));
    assert!((v.free_at(2, 2, 0) as f64 - 0.5).abs() < 1e-3, "far {}", v.free_at(2, 2, 0));
    assert!(residual(&w).abs() < 1e-9);
}

#[test]
fn the_same_fixture_shifted_across_the_seam_gives_identical_stores() {
    let here = u_tube(0);
    let seam = u_tube(-2);
    assert!((here.view().stored_m3() - seam.view().stored_m3()).abs() < 1e-12);
    for x in 0..5 {
        let a = column(&here, x);
        let b = column(&seam, x - 2);
        assert!((a - b).abs() < 1e-9, "column {x}: {a} vs {b}");
    }
}

// ------------------------------------------------------------------ roofed passage

/// A flat-roofed passage along the bottom row joining two shafts, in a closed box.
fn roofed(left: f64, right: f64) -> World {
    let mut w = World::empty(cfg(10, 8));
    wall(&mut w, 0, 1..=7);
    wall(&mut w, 9, 1..=7);
    for x in 3..=6 {
        wall(&mut w, x, 2..=2);
    }
    pour(&mut w, 1, left);
    pour(&mut w, 2, right);
    run(&mut w, 20);
    w
}

#[test]
fn a_roofed_passage_fills_and_the_far_side_rises() {
    let w = roofed(6.0, 0.0);
    let v = w.view();
    // Six units over the eight passage cells: level 1.75 under the roof, everywhere.
    for x in 1..=8 {
        assert!((v.free_at(x, 1, 0) as f64 - 0.75).abs() < 1e-3, "passage {x}: {}", v.free_at(x, 1, 0));
    }
    assert!(column(&w, 7) > 0.5, "far side dry: {}", column(&w, 7));
    assert!(residual(&w).abs() < 1e-9);
}

#[test]
fn a_roofed_passage_pushes_the_far_shaft_above_the_roof() {
    let w = roofed(7.0, 7.0);
    let v = w.view();
    // Fourteen units: the passage full (8), both shafts full at y = 2 (4), and the
    // remaining 2 shared by the eight cells of the row above the roof: level 3.25. The
    // far shaft stands above the roof, so the passage carried pressure through it.
    assert!((v.free_at(8, 3, 0) as f64 - 0.25).abs() < 1e-3, "far {}", v.free_at(8, 3, 0));
    assert!((v.free_at(1, 3, 0) as f64 - 0.25).abs() < 1e-3, "near {}", v.free_at(1, 3, 0));
    assert!((v.free_at(4, 3, 0) as f64 - 0.25).abs() < 1e-3, "over the roof {}", v.free_at(4, 3, 0));
    assert!((v.free_at(8, 2, 0) as f64 - 1.0).abs() < 1e-3, "far shaft {}", v.free_at(8, 2, 0));
    assert!(residual(&w).abs() < 1e-9);
}

// ------------------------------------------------------------------ dry ridge

#[test]
fn a_dry_ridge_stays_dry_until_overtopped() {
    let mut w = World::empty(cfg(7, 8));
    wall(&mut w, 3, 1..=4);
    wall(&mut w, 6, 1..=4);
    // Left basin is x = 0..2, right basin x = 4..5, ridges four cells tall between.
    pour(&mut w, 0, 2.0);
    run(&mut w, 12);
    assert!(column(&w, 4) < 1e-6, "ridge leaked: {}", column(&w, 4));
    assert!(column(&w, 5) < 1e-6, "ridge leaked: {}", column(&w, 5));

    // Twelve units fills the left basin exactly to the ridge top; fourteen overtops.
    for x in 0..3 {
        pour(&mut w, x, 4.0);
    }
    run(&mut w, 24);
    let across = column(&w, 4) + column(&w, 5);
    assert!(across > 1.5, "ridge did not overtop: {across}");
    assert!((w.view().stored_m3() - 14.0).abs() < 1e-6);
    assert!(residual(&w).abs() < 1e-9);
}

// ------------------------------------------------------------------ ledger

#[test]
fn the_residual_stays_below_1e_9_with_every_flux_firing() {
    let mut config = cfg(6, 6);
    config.depth = 2;
    config.voxel_m = 0.5;
    config.rain_m_per_s = 0.004;
    config.evaporation_m_per_s = 0.001;
    config.water_substeps = 2;
    config.spring_k_m2_per_s = 0.05;
    let mut w = World::empty(config);
    for x in 0..6 {
        for z in 0..2 {
            w.apply(Command::SetMaterial { x, y: 1, z, material: Material::Soil });
        }
    }
    w.set_spring_cell(Some((1, 2, 0)));
    w.set_outlet_cell(Some((4, 2, 0)));
    w.apply(Command::SetOutlet { open: true });
    w.apply(Command::ChargeAquifer { volume_m3: 2.0 });
    w.apply(Command::RainPulse { volume_m3: 0.5 });
    run(&mut w, 12);

    let v = w.view();
    assert!(v.ledger.rain_in > 0.5, "rain {}", v.ledger.rain_in);
    assert!(v.ledger.evaporation_out > 0.0, "no evaporation");
    assert!(v.ledger.outlet_out > 0.0, "no outlet export");
    assert!(v.ledger.displaced_out == 0.0, "displaced {}", v.ledger.displaced_out);
    assert!(v.aquifer_m3 < 2.0, "spring never discharged");
    let infiltrated: f32 = (0..6).map(|x| v.pore_at(x, 1, 0)).sum();
    assert!(infiltrated > 0.0, "no infiltration");
    assert!(residual(&w).abs() < 1e-9, "residual {}", residual(&w));
    // The quantization term must stay arithmetic-sized, not water-sized.
    assert!(v.ledger.rounding_m3.abs() < 1e-6, "rounding {}", v.ledger.rounding_m3);
}

#[test]
fn set_material_moves_displaced_water_instead_of_booking_it_out() {
    let mut w = World::empty(cfg(4, 6));
    w.apply(Command::AddWater { x: 1, y: 1, z: 0, volume_m3: 1.0 });
    let before = w.view().stored_m3();
    w.apply(Command::SetMaterial { x: 1, y: 1, z: 0, material: Material::Rock });
    let after = w.view().stored_m3();
    // The rock keeps its pore share; the rest lands in the nearest void cell. The store
    // only moves by the f32 quantization of that fill, which the ledger accounts for.
    assert!((before - after).abs() < 1e-6, "{before} -> {after}");
    assert!((w.view().free_at(0, 1, 0) as f64 - 0.98).abs() < 1e-6, "{}", w.view().free_at(0, 1, 0));
    assert_eq!(w.view().ledger.displaced_out, 0.0);
    assert!(residual(&w).abs() < 1e-9);
}

#[test]
fn the_transfer_cap_makes_a_fill_travel_instead_of_arriving() {
    let mut instant = World::empty(cfg(8, 4));
    pour(&mut instant, 0, 2.0);
    instant.step();

    let mut capped_config = cfg(8, 4);
    capped_config.free_transfer_cap = 0.02;
    let mut capped = World::empty(capped_config);
    pour(&mut capped, 0, 2.0);
    capped.step();

    // Uncapped, one tick levels the whole ring floor at 2 / 8 of a cell.
    assert!((column(&instant, 4) - 0.25).abs() < 1e-3, "{}", column(&instant, 4));
    assert!(column(&capped, 4) < 0.1, "capped arrived anyway: {}", column(&capped, 4));
    assert!(column(&capped, 4) > 0.0, "capped never moved");
    assert!(residual(&capped).abs() < 1e-9);
}

#[test]
fn the_same_seed_gives_the_same_run() {
    let config = Config { width: 48, height: 24, depth: 4, rain_m_per_s: 0.002, ..Config::default() };
    let mut a = World::new(config.clone());
    let mut b = World::new(config);
    a.apply(Command::RainPulse { volume_m3: 12.0 });
    b.apply(Command::RainPulse { volume_m3: 12.0 });
    run(&mut a, 5);
    run(&mut b, 5);
    assert_eq!(a, b);
}

// ------------------------------------------------------------------ snapshots

#[test]
fn save_load_round_trips() {
    let mut w = World::empty(cfg(6, 6));
    pour(&mut w, 2, 2.5);
    w.apply(Command::ChargeAquifer { volume_m3: 0.75 });
    run(&mut w, 3);
    let bytes = w.save();
    let back = World::load(&bytes).expect("round trip");
    assert_eq!(w, back);
    assert_eq!(w.view().stored_m3(), back.view().stored_m3());
    assert_eq!(w.tick(), back.tick());
}

#[test]
fn a_wrong_schema_tag_is_refused() {
    let w = World::empty(cfg(4, 5));
    let mut bytes = w.save();
    bytes[0] = 2;
    let err = World::load(&bytes).expect_err("must refuse another schema");
    assert!(format!("{err}").contains("schema"), "{err}");
}

// ------------------------------------------------------------------ generator

#[test]
fn the_generator_has_no_isolated_voids_a_ridge_above_the_basin_and_a_clean_seam() {
    for seed in [1u64, 2, 77] {
        let world = World::new(Config { seed, ..Config::default() });
        let v = world.view();
        assert!(
            generate::isolated_voids(&world).is_empty(),
            "seed {seed}: {} isolated voids",
            generate::isolated_voids(&world).len()
        );

        let mut low = u32::MAX;
        let mut high = 0;
        for z in 0..v.config.depth {
            for x in 0..v.config.width as i64 {
                let y = v.surface_y(x, z).expect("every column has ground");
                low = low.min(y);
                high = high.max(y);
            }
        }
        assert!(high >= low + 6, "seed {seed}: ridge {high} over basin {low}");

        for z in 0..v.config.depth {
            let last = v.surface_y(v.config.width as i64 - 1, z).unwrap() as i64;
            let first = v.surface_y(0, z).unwrap() as i64;
            assert!((first - last).abs() <= 2, "seed {seed}, z {z}: seam {last} -> {first}");
            for y in 0..v.config.height {
                assert_eq!(v.material_at(-1, y, z), v.material_at(v.config.width as i64 - 1, y, z));
            }
        }

        // The overhang and the covered passage both leave roofed void, and both stay
        // reachable: the isolated-void check above would have filled them in otherwise.
        let roofed = (1..v.config.height - 1)
            .flat_map(|y| (0..v.config.width as i64).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                (0..v.config.depth).any(|z| {
                    !v.material_at(x, y, z).is_solid() && v.material_at(x, y + 1, z).is_solid()
                })
            })
            .count();
        assert!(roofed >= 4, "seed {seed}: only {roofed} roofed cells, no overhang or passage");

        assert!(world.outlet_cell().is_some(), "seed {seed}: no outlet");
        assert!(world.spring_cell().is_some(), "seed {seed}: no spring");
    }
}
