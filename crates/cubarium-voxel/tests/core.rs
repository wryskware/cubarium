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
    (0..w.config().height).map(|y| v.free_at(x, y, 0)).sum()
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
    assert!((w.view().free_at(1, 2, 0) - 0.3).abs() < 1e-3);
    assert!(residual(&w).abs() < 1e-9);
}

// ------------------------------------------------------------------ U-tube

/// Two shafts joined only along the bottom row, on a closed five-column ring.
/// `offset` slides the whole fixture — walls and input column together — around the
/// ring. `dir = -1` mirrors it in `x` about the input column, which moves the wet shaft
/// to the other side of the passage without changing anything else.
fn u_tube_at(offset: i64, dir: i64) -> World {
    let mut w = World::empty(cfg(5, 8));
    wall(&mut w, offset + dir, 2..=7);
    wall(&mut w, offset + 3 * dir, 1..=7);
    wall(&mut w, offset + 4 * dir, 1..=7);
    pour(&mut w, offset, 4.0);
    run(&mut w, 16);
    w
}

fn u_tube(offset: i64) -> World {
    u_tube_at(offset, 1)
}

#[test]
fn u_tube_equalizes() {
    let w = u_tube(0);
    let v = w.view();
    // Bottom row (three cells) full, then one unit shared by the two shafts at y = 2.
    for x in 0..3 {
        assert!((v.free_at(x, 1, 0) - 1.0).abs() < 1e-3, "floor {x}");
    }
    assert!((v.free_at(0, 2, 0) - 0.5).abs() < 1e-3, "near {}", v.free_at(0, 2, 0));
    assert!((v.free_at(2, 2, 0) - 0.5).abs() < 1e-3, "far {}", v.free_at(2, 2, 0));
    assert!(residual(&w).abs() < 1e-9);
}

#[test]
fn the_same_fixture_shifted_across_the_seam_gives_identical_stores() {
    let here = u_tube(0);
    let seam = u_tube(-2);
    assert!((here.view().stored_m3() - seam.view().stored_m3()).abs() < 1e-12);
    // Per cell, not per column: the shifted run must be the inverse shift of this one
    // everywhere, in both stores.
    let (a, b) = (here.view(), seam.view());
    for y in 0..here.config().height {
        for x in 0..5 {
            assert_eq!(a.material_at(x, y, 0), b.material_at(x - 2, y, 0), "material {x},{y}");
            let (f, g) = (a.free_at(x, y, 0), b.free_at(x - 2, y, 0));
            assert!((f - g).abs() < 1e-12, "free {x},{y}: {f} vs {g}");
            let (p, q) = (a.pore_at(x, y, 0), b.pore_at(x - 2, y, 0));
            assert!((p - q).abs() < 1e-12, "pore {x},{y}: {p} vs {q}");
        }
    }
}

/// A four-column basin behind a one-cell sill, a shallow two-column shelf beyond it and
/// a tall ridge closing the shelf off: no symmetry anywhere, and the source is off to
/// one side. `dir = -1` mirrors every wall and the input column about `x = 0`, which
/// puts the wet basin on the other side of the sill.
fn shelf(dir: i64) -> World {
    let mut w = World::empty(cfg(8, 6));
    wall(&mut w, 2 * dir, 1..=1);
    wall(&mut w, 5 * dir, 1..=4);
    pour(&mut w, dir, 5.0);
    run(&mut w, 20);
    w
}

/// Index order is the seed and candidate tie-break in `equalize`, so a fixture mirrored
/// in `x` is where an order bias would show: the same geometry walked the other way
/// round, with the wet source on the other side of the sill it has to cross.
#[test]
fn the_mirrored_fixture_gives_the_mirrored_answer() {
    let here = shelf(1);
    let flipped = shelf(-1);
    assert!((here.view().stored_m3() - flipped.view().stored_m3()).abs() < 1e-12);
    let (a, b) = (here.view(), flipped.view());
    // The water crossed the sill, so the two sides really are unequal and a bias
    // between them would have somewhere to hide.
    assert!(column(&here, 3) > 1e-6, "nothing crossed the sill");
    assert!(column(&here, 0) > column(&here, 3), "the basin did not stay deeper");
    // 1e-6 rather than 1e-12: the spill decays toward its stopping point, so the
    // substep it stops on turns on a float comparison and the two runs settle a few
    // times 1e-8 apart. A seed or candidate *order* bias would move a fill by a
    // tenth, not by 1e-8. See the tie rule in the `water` module doc.
    for y in 0..here.config().height {
        for x in 0..8 {
            assert_eq!(a.material_at(x, y, 0), b.material_at(-x, y, 0), "material {x},{y}");
            let (f, g) = (a.free_at(x, y, 0), b.free_at(-x, y, 0));
            assert!((f - g).abs() < 1e-6, "free {x},{y}: {f} vs {g}");
        }
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
        assert!((v.free_at(x, 1, 0) - 0.75).abs() < 1e-3, "passage {x}: {}", v.free_at(x, 1, 0));
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
    assert!((v.free_at(8, 3, 0) - 0.25).abs() < 1e-3, "far {}", v.free_at(8, 3, 0));
    assert!((v.free_at(1, 3, 0) - 0.25).abs() < 1e-3, "near {}", v.free_at(1, 3, 0));
    assert!((v.free_at(4, 3, 0) - 0.25).abs() < 1e-3, "over the roof {}", v.free_at(4, 3, 0));
    assert!((v.free_at(8, 2, 0) - 1.0).abs() < 1e-3, "far shaft {}", v.free_at(8, 2, 0));
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
    let infiltrated: f64 = (0..6).map(|x| v.pore_at(x, 1, 0)).sum();
    assert!(infiltrated > 0.0, "no infiltration");
    // Raw conservation error: `free` and `pore` are `f64` and the ledger has no
    // correction term, so this is the whole story.
    assert!(residual(&w).abs() < 1e-9, "residual {}", residual(&w));
}

// ------------------------------------------------------------- material edits

/// Saturated soil at `(1, 1)` on a four-column ring, made by turning a brim-full air
/// cell into soil: the soil takes its whole 0.35 m3 pore capacity and the other 0.65 m3
/// is already sitting in its three void face neighbours, `(0, 1)`, `(2, 1)` and
/// `(1, 2)` — `(1, 0)` is the bedrock foundation. Never stepped, so nothing has drained.
fn wet_soil() -> World {
    let mut w = World::empty(cfg(4, 6));
    w.apply(Command::AddWater { x: 1, y: 1, z: 0, volume_m3: 1.0 });
    w.apply(Command::SetMaterial { x: 1, y: 1, z: 0, material: Material::Soil });
    assert!((w.view().pore_at(1, 1, 0) - 1.0).abs() < 1e-9, "the soil must start saturated");
    w
}

#[test]
fn set_material_moves_displaced_water_instead_of_booking_it_out() {
    let mut w = World::empty(cfg(4, 6));
    w.apply(Command::AddWater { x: 1, y: 1, z: 0, volume_m3: 1.0 });
    let before = w.view().stored_m3();
    w.apply(Command::SetMaterial { x: 1, y: 1, z: 0, material: Material::Rock });
    let after = w.view().stored_m3();
    // The rock keeps its pore share (0.02); the remaining 0.98 is shared equally by the
    // three void cells one step away.
    assert!((before - after).abs() < 1e-9, "{before} -> {after}");
    assert!((w.view().pore_at(1, 1, 0) - 1.0).abs() < 1e-9, "{}", w.view().pore_at(1, 1, 0));
    for (x, y) in [(0, 1), (2, 1), (1, 2)] {
        let f = w.view().free_at(x, y, 0);
        assert!((f - 0.98 / 3.0).abs() < 1e-9, "({x}, {y}) got {f}");
    }
    assert_eq!(w.view().ledger.displaced_out, 0.0);
    assert!(residual(&w).abs() < 1e-9);
}

#[test]
fn wet_soil_turned_to_rock_keeps_only_rock_capacity_and_displaces_the_rest() {
    let mut w = wet_soil();
    let before = w.view().stored_m3();
    w.apply(Command::SetMaterial { x: 1, y: 1, z: 0, material: Material::Rock });
    // Rock holds 0.02 of the voxel against soil's 0.35, so a full rock pore keeps
    // 0.02 m3 and the other 0.33 m3 is shared by the three neighbouring voids. Keeping
    // the *fraction* would have silently dropped that 0.33.
    assert!((w.view().pore_at(1, 1, 0) - 1.0).abs() < 1e-9, "{}", w.view().pore_at(1, 1, 0));
    for (x, y) in [(0, 1), (2, 1), (1, 2)] {
        let f = w.view().free_at(x, y, 0);
        assert!((f - 0.98 / 3.0).abs() < 1e-9, "({x}, {y}) got {f}");
    }
    assert!((w.view().stored_m3() - before).abs() < 1e-9, "{}", w.view().stored_m3());
    assert_eq!(w.view().ledger.displaced_out, 0.0);
    assert!(residual(&w).abs() < 1e-9);
}

#[test]
fn wet_soil_turned_to_air_releases_its_pore_water_as_free_water() {
    let mut w = wet_soil();
    let before = w.view().stored_m3();
    w.apply(Command::SetMaterial { x: 1, y: 1, z: 0, material: Material::Air });
    // Air has no pores: the soil's 0.35 m3 becomes free water, and it fits in the cell.
    assert!((w.view().free_at(1, 1, 0) - 0.35).abs() < 1e-9, "{}", w.view().free_at(1, 1, 0));
    assert_eq!(w.view().pore_at(1, 1, 0), 0.0);
    assert!((w.view().stored_m3() - before).abs() < 1e-9, "{}", w.view().stored_m3());
    assert_eq!(w.view().ledger.displaced_out, 0.0);
    assert!(residual(&w).abs() < 1e-9);
}

#[test]
fn a_full_recipient_is_skipped_and_the_next_shell_takes_the_water() {
    let mut w = wet_soil();
    for (x, y) in [(0, 1), (2, 1), (1, 2)] {
        w.apply(Command::AddWater { x, y, z: 0, volume_m3: 1.0 });
    }
    let before = w.view().stored_m3();
    w.apply(Command::SetMaterial { x: 1, y: 1, z: 0, material: Material::Rock });
    // Every cell one step away is brim full, so the 0.33 goes to the four cells two
    // void steps away: (3, 1) round the seam, (0, 2), (2, 2) and (1, 3).
    for (x, y) in [(0, 1), (2, 1), (1, 2)] {
        assert!((w.view().free_at(x, y, 0) - 1.0).abs() < 1e-9, "({x}, {y}) overfilled");
    }
    for (x, y) in [(3, 1), (0, 2), (2, 2), (1, 3)] {
        let f = w.view().free_at(x, y, 0);
        assert!((f - 0.33 / 4.0).abs() < 1e-9, "({x}, {y}) got {f}");
    }
    assert!((w.view().stored_m3() - before).abs() < 1e-9);
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
