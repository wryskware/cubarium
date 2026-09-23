//! Small function tests on tiny `World::empty` fixtures, a few substeps each.
//! `voxel_m` is 1 m in the fixtures, so one "unit" of water is one cubic metre and one
//! full cell.

use cubarium_voxel::generate;
use cubarium_voxel::{Command, Config, Material, World};

fn cfg(width: u32, height: u32) -> Config {
    Config {
        width,
        height,
        depth: 1,
        voxel_m: 1.0,
        seed: 7,
        water_substeps: 4,
        ..Config::default()
    }
}

fn wall(w: &mut World, x: i64, ys: std::ops::RangeInclusive<u32>) {
    for y in ys {
        w.apply(Command::SetMaterial {
            x,
            y,
            z: 0,
            material: Material::Rock,
        });
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
        let got = w.apply(Command::AddWater {
            x,
            y,
            z: 0,
            volume_m3: volume,
        });
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
    // Both sills end dry on top: the spill stopped at the sill top instead of levelling
    // across it, which is the whole point of the threshold.
    for x in [1, 3] {
        let over = w.view().free_at(x, 2, 0);
        assert!(
            over < 1e-9,
            "water left standing on the sill top at {x}: {over}"
        );
    }
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

// --------------------------------------------------------- symmetric spill (R2.1)

/// Two bedrock supports side by side with a one-cell hollow either side of them, closed
/// in by bedrock walls: the whole state — geometry and both sources — is symmetric under
/// `x -> 5 - x`, so the water perched on the supports has two identical ways down and no
/// substep may prefer one of them.
fn symmetric_spill() -> World {
    let mut w = World::empty(cfg(6, 5));
    for x in [0, 5] {
        for y in 1..=4 {
            w.apply(Command::SetMaterial {
                x,
                y,
                z: 0,
                material: Material::Bedrock,
            });
        }
    }
    for x in [2, 3] {
        w.apply(Command::SetMaterial {
            x,
            y: 1,
            z: 0,
            material: Material::Bedrock,
        });
        w.apply(Command::AddWater {
            x,
            y: 2,
            z: 0,
            volume_m3: 0.7,
        });
    }
    w
}

/// Mirror symmetry is a property of the *state*, so it has to hold after every tick and
/// not only once the spill has settled.
fn assert_mirrored(w: &World, width: i64, axis: i64, when: &str) {
    let v = w.view();
    for y in 0..w.config().height {
        for x in 0..width {
            assert_eq!(
                v.material_at(x, y, 0),
                v.material_at(axis - x, y, 0),
                "material {x},{y}"
            );
            let (l, r) = (v.free_at(x, y, 0), v.free_at(axis - x, y, 0));
            assert!(
                (l - r).abs() < 1e-9,
                "{when}: free {x},{y} is {l}, its mirror is {r}"
            );
        }
    }
}

#[test]
fn a_symmetric_spill_shares_itself_between_both_hollows() {
    let mut w = symmetric_spill();
    assert_mirrored(&w, 6, 5, "tick 0");
    for tick in 1..=20 {
        w.step();
        assert_mirrored(&w, 6, 5, &format!("tick {tick}"));
    }
    // 1.4 units over two one-unit hollows: both end at 0.7 and the shelf ends dry.
    let v = w.view();
    let (left, right) = (v.free_at(1, 1, 0), v.free_at(4, 1, 0));
    assert!((left - right).abs() < 1e-9, "hollows {left} vs {right}");
    assert!((left - 0.7).abs() < 1e-6, "left hollow {left}");
    assert!((right - 0.7).abs() < 1e-6, "right hollow {right}");
    for x in 1..=4 {
        assert!(
            v.free_at(x, 2, 0) < 1e-6,
            "the shelf kept water at {x}: {}",
            v.free_at(x, 2, 0)
        );
    }
    assert!(
        (v.stored_m3() - 1.4).abs() < 1e-9,
        "stored {}",
        v.stored_m3()
    );
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
    assert!(
        (v.free_at(0, 2, 0) - 0.5).abs() < 1e-3,
        "near {}",
        v.free_at(0, 2, 0)
    );
    assert!(
        (v.free_at(2, 2, 0) - 0.5).abs() < 1e-3,
        "far {}",
        v.free_at(2, 2, 0)
    );
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
            assert_eq!(
                a.material_at(x, y, 0),
                b.material_at(x - 2, y, 0),
                "material {x},{y}"
            );
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

/// Index order is the seed and row tie-break in `equalize`, so running this fixture
/// against its mirror image is where an order bias would show: the same geometry walked
/// the other way round, with the wet basin on the other side of the sill it has to cross.
///
/// What this pins down is *this* fixture. Growth is order independent within a row,
/// because a row of candidates is taken whole or not at all (see the `water` module
/// doc), and this fixture, the seam shift above and the symmetric spill are the three
/// cases that is checked against. None of them establishes that every geometry is
/// insensitive to the order its cells are reached in.
#[test]
fn the_mirrored_fixture_gives_the_mirrored_answer() {
    let here = shelf(1);
    let flipped = shelf(-1);
    assert!((here.view().stored_m3() - flipped.view().stored_m3()).abs() < 1e-12);
    let (a, b) = (here.view(), flipped.view());
    // The water crossed the sill, so the two sides really are unequal and a bias
    // between them would have somewhere to hide.
    assert!(column(&here, 3) > 1e-6, "nothing crossed the sill");
    assert!(
        column(&here, 0) > column(&here, 3),
        "the basin did not stay deeper"
    );
    // 5e-3 rather than 1e-12, and 1e-6 until package 1c
    // (`design/handoffs/voxel-water-units-2026-09-22.md`). The sill at `2·dir` and the
    // wall at `5·dir` seal two basins off from each other, so what this fixture really
    // pins is the *split* the spill froze between them, and that split is decided by a
    // single substep-level head comparison: one packet more or less over the sill is
    // worth 2.5e-3 of a cell in the resting levels, for ever, because nothing can level
    // across the sill afterwards. Moving the rock's uptake from a fraction of a cell per
    // tick to a conductivity in metres per second moved the operating point across that
    // comparison, and the mirror pair now settles 2.45e-3 apart (0.05 % of the 5 m³
    // poured) instead of exactly together; measured constant in the poured volume from
    // 4.9 to 7.0 m³ and constant in the tick count from 20 to 1000, so it is a frozen
    // decision and not a drift. Conservation, the materials and `stored_m3` are still
    // mirrored exactly, which is asserted above at 1e-12.
    for y in 0..here.config().height {
        for x in 0..8 {
            assert_eq!(
                a.material_at(x, y, 0),
                b.material_at(-x, y, 0),
                "material {x},{y}"
            );
            let (f, g) = (a.free_at(x, y, 0), b.free_at(-x, y, 0));
            assert!((f - g).abs() < 5e-3, "free {x},{y}: {f} vs {g}");
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
    // 200 ticks, not 20: the local exchange relaxes a body toward its level at half a head
    // difference per substep instead of re-levelling it in one, so a ten-column passage
    // takes a few seconds rather than a tick (`design/7_Research/voxel-tick-profile-2026-09-18.md`).
    run(&mut w, 200);
    w
}

#[test]
fn a_roofed_passage_fills_and_the_far_side_rises() {
    let w = roofed(6.0, 0.0);
    let v = w.view();
    // Six units over the eight passage cells: level 1.75 under the roof, everywhere.
    for x in 1..=8 {
        assert!(
            (v.free_at(x, 1, 0) - 0.75).abs() < 1e-3,
            "passage {x}: {}",
            v.free_at(x, 1, 0)
        );
    }
    assert!(column(&w, 7) > 0.5, "far side dry: {}", column(&w, 7));
    assert!(residual(&w).abs() < 1e-9);
}

/// **Pressure crosses a flooded passage.** Fourteen units in a box whose only connection
/// is a roofed passage: the passage floods, and water then stands **above the roof** on
/// both sides and over the roof itself, which it can only do if the far shaft was pushed
/// up through the submerged gap.
///
/// The claim is qualitative on purpose. A local exchange carries head one cell per pass and
/// has no pressure solve in it, so a *closed, surcharged* passage did not settle to one
/// flat surface the way the old region solver made it: the surface above the roof stayed
/// uneven by a few tenths of a cell (0.40 at 200 ticks; 0.26 after package H). Package P,
/// which caps what a stack sends across one face at one row's worth, ended the swapping
/// behind that: at 200 ticks the surface over the roof is flat to 1e-4 and every passage
/// cell is full. The residual below is still the check that matters; see
/// `design/7_Research/voxel-tick-profile-2026-09-18.md` for the history.
#[test]
fn a_roofed_passage_pushes_the_far_shaft_above_the_roof() {
    let w = roofed(7.0, 7.0);
    let v = w.view();
    // The passage is flooded and the far shaft is full to the roof line.
    for x in 3..=6 {
        assert!(
            (v.free_at(x, 1, 0) - 1.0).abs() < 1e-2,
            "passage {x}: {}",
            v.free_at(x, 1, 0)
        );
    }
    assert!(
        v.free_at(8, 2, 0) > 0.9,
        "far shaft short of the roof: {}",
        v.free_at(8, 2, 0)
    );
    // And water stands over the roof on both sides and in the middle: the surcharge got
    // there through the passage, since there is no other way across.
    for x in [1, 4, 8] {
        assert!(
            v.free_at(x, 3, 0) > 0.1,
            "nothing over the roof at {x}: {}",
            v.free_at(x, 3, 0)
        );
    }
    assert!(
        (w.view().stored_m3() - 14.0).abs() < 1e-6,
        "mass: {}",
        w.view().stored_m3()
    );
    assert!(residual(&w).abs() < 1e-9);
}

// ---------------------------------------------- the local exchange: what it has to do

/// **Mass is conserved to noise with everything running.** Two hundred ticks of rain on
/// the generated world with the outlet open, so every phase fires — rain, infiltration,
/// fall, the exchange, drainage, the water table, the spring and the export — and the
/// ledger residual stays at float noise. This is the check that matters for a solver
/// rewrite: the shape of the water is a rule, the conservation is a contract.
#[test]
fn two_hundred_ticks_of_rain_and_export_conserve_mass_to_noise() {
    let config = Config {
        width: 48,
        height: 24,
        depth: 4,
        rain_m_per_s: 0.002,
        outlet_m3_per_s: 0.05,
        ..Config::default()
    };
    let mut w = World::new(config);
    w.apply(Command::SetOutlet { open: true });
    run(&mut w, 200);
    let v = w.view();
    let stored = v.stored_m3();
    let residual = stored - v.ledger.expected_stored();
    assert!(v.ledger.rain_in > 0.0, "it did not rain");
    assert!(v.ledger.outlet_out > 0.0, "nothing was exported");
    assert!(
        residual.abs() <= 1e-9 * stored.max(1.0),
        "residual {residual} against {stored} stored"
    );
}

/// **A U-tube reaches equal levels, and in how long.** The fixture above settles by 16
/// ticks; this says so as a time and checks the levels rather than the substep count, which
/// is what a relaxation can be held to. `FLOW_PER_SUBSTEP` is a placeholder, so the time is
/// a measurement of this placeholder and not a requirement on the model.
#[test]
fn a_u_tube_levels_within_a_second() {
    let mut w = World::empty(cfg(5, 8));
    wall(&mut w, 1, 2..=7);
    wall(&mut w, 3, 1..=7);
    wall(&mut w, 4, 1..=7);
    pour(&mut w, 0, 4.0);
    // 20 ticks is one second at 20 Hz.
    run(&mut w, 20);
    let v = w.view();
    // Both shafts stand at 2.5 cells: floor full, half a cell above it on each side.
    assert!(
        (v.free_at(0, 2, 0) - 0.5).abs() < 1e-2,
        "near {}",
        v.free_at(0, 2, 0)
    );
    assert!(
        (v.free_at(2, 2, 0) - 0.5).abs() < 1e-2,
        "far {}",
        v.free_at(2, 2, 0)
    );
    for x in 0..3 {
        assert!(
            (v.free_at(x, 1, 0) - 1.0).abs() < 1e-2,
            "floor {x}: {}",
            v.free_at(x, 1, 0)
        );
    }
    assert!(residual(&w).abs() < 1e-9);
}

/// **A basin spills at its lowest exit.** A closed hollow with two sills — one two cells
/// up, one three — poured full past the lower one: the water leaves over the low sill and
/// the high side stays dry. A local exchange has no map of the basin, so this is the
/// fixture that says it still finds the way out.
#[test]
fn a_basin_fills_and_spills_at_its_lowest_exit() {
    let mut w = World::empty(cfg(9, 8));
    // The hollow: columns 3..=5, closed by a sill of one cell at x = 6 and of two at x = 2.
    wall(&mut w, 2, 1..=2);
    wall(&mut w, 6, 1..=1);
    // And far walls, so what spills has somewhere to stand and cannot come round the ring.
    wall(&mut w, 0, 1..=7);
    // Poured across the hollow's own three columns rather than stacked in one, so the
    // starting state is a basin holding water and not a tower standing over both sills.
    for x in 3..=5 {
        pour(&mut w, x, 1.2);
    }
    run(&mut w, 200);

    let v = w.view();
    // The 0.6 that stood above the low sill's top drained over it and spread over the two
    // columns beyond, whose floor is a cell lower: 0.3 each.
    let beyond_low: f64 = (7..9)
        .map(|x| (0..8).map(|y| v.free_at(x, y, 0)).sum::<f64>())
        .sum();
    let beyond_high = (0..8).map(|y| v.free_at(1, y, 0)).sum::<f64>();
    assert!(beyond_low > 0.55, "only {beyond_low} crossed the low sill");
    assert!(
        (v.free_at(7, 1, 0) - 0.3).abs() < 1e-2,
        "beyond the sill: {}",
        v.free_at(7, 1, 0)
    );
    assert!(
        beyond_high < 1e-6,
        "water crossed the high sill: {beyond_high}"
    );
    assert!(
        (v.stored_m3() - 3.6).abs() < 1e-6,
        "mass: {}",
        v.stored_m3()
    );
    assert!(residual(&w).abs() < 1e-9);
}

/// **A dam holds.** A wall six cells tall with four cells of water against it: after
/// twenty seconds the dry side is still dry, and every drop is still on the wet side.
#[test]
fn a_dam_holds() {
    let mut w = World::empty(cfg(10, 8));
    wall(&mut w, 5, 1..=6);
    wall(&mut w, 0, 1..=7);
    pour(&mut w, 2, 4.0);
    run(&mut w, 400);

    let v = w.view();
    let dry_side: f64 = (6..9)
        .map(|x| (0..8).map(|y| v.free_at(x, y, 0)).sum::<f64>())
        .sum();
    let wet_side: f64 = (1..5)
        .map(|x| (0..8).map(|y| v.free_at(x, y, 0)).sum::<f64>())
        .sum();
    assert!(dry_side < 1e-9, "the dam leaked: {dry_side}");
    assert!(
        (wet_side - 4.0).abs() < 1e-6,
        "the wet side holds {wet_side}"
    );
    // And it stands against the dam rather than piling up in one column.
    for x in 1..5 {
        assert!(
            (v.free_at(x, 1, 0) - 1.0).abs() < 1e-2,
            "floor {x}: {}",
            v.free_at(x, 1, 0)
        );
    }
    assert!(residual(&w).abs() < 1e-9);
}

/// **A waterfall.** Water poured onto a plateau runs to the edge, leaves it, falls down
/// the open column as thin cells — one cell per substep, which is `fall`'s own rule — and
/// pools on the floor below. Nothing about this is a new rule: it is the exchange pushing
/// into dry air at the lip and `fall` taking it down.
#[test]
fn water_runs_off_a_ledge_falls_and_pools_below() {
    let mut w = World::empty(cfg(12, 10));
    // A plateau four columns wide and five cells high, with open floor beyond it.
    for x in 0..4 {
        wall(&mut w, x, 1..=5);
    }
    // A wall at the far end so the pool cannot wrap round the ring into the plateau's back.
    wall(&mut w, 11, 1..=7);
    pour(&mut w, 1, 2.0);

    // Early on, the fall is in the air: some cell of the open columns beside the lip holds
    // a thin sheet, neither empty nor full.
    let mut falling = false;
    for _ in 0..40 {
        w.step();
        let v = w.view();
        for x in 4..8 {
            for y in 2..6 {
                let f = v.free_at(x, y, 0);
                if f > 1e-6 && f < 0.5 {
                    falling = true;
                }
            }
        }
    }
    assert!(falling, "nothing was ever in mid-air beside the lip");

    run(&mut w, 400);
    let v = w.view();
    let pool: f64 = (4..11)
        .map(|x| (0..3).map(|y| v.free_at(x, y, 0)).sum::<f64>())
        .sum();
    let left_on_top: f64 = (0..4)
        .map(|x| (6..10).map(|y| v.free_at(x, y, 0)).sum::<f64>())
        .sum();
    assert!(pool > 1.9, "the pool below holds {pool} of the 2.0 poured");
    assert!(
        left_on_top < 0.05,
        "water is still standing on the plateau: {left_on_top}"
    );
    assert!(
        (v.stored_m3() - 2.0).abs() < 1e-6,
        "mass: {}",
        v.stored_m3()
    );
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
            w.apply(Command::SetMaterial {
                x,
                y: 1,
                z,
                material: Material::Soil,
            });
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
    assert!(
        v.ledger.displaced_out == 0.0,
        "displaced {}",
        v.ledger.displaced_out
    );
    assert!(v.aquifer_m3 < 2.0, "spring never discharged");
    let infiltrated: f64 = (0..6).map(|x| v.pore_at(x, 1, 0)).sum();
    assert!(infiltrated > 0.0, "no infiltration");
    // Raw conservation error: `free` and `pore` are `f64` and the ledger has no
    // correction term, so this is the whole story.
    assert!(residual(&w).abs() < 1e-9, "residual {}", residual(&w));
}

// ------------------------------------------------------------------ spring

/// A spring at `(1, 3)` on a four-column ring, 1 m voxels: `h_spring` is 3 m and the
/// aquifer's head is `aquifer_m3 / 0.4`, so 4 m3 drives it and 1 m3 does not.
/// `sealed` walls `(1, 3)` in with bedrock — no pores, so nothing infiltrates out of it
/// before the spring runs — and roofs it, which is the blocked case.
fn spring_world(aquifer_m3: f64, sealed: bool) -> World {
    let mut c = cfg(4, 6);
    c.spring_k_m2_per_s = 0.5;
    // The heads these tests hand-work (10 m from 4 m3, 2.5 m from 1 m3) assume a tenth
    // of the footprint; the default rose to soil's pore capacity when the water table
    // arrived, so the fixture pins the value the arithmetic was done with.
    c.aquifer_porosity = 0.1;
    let mut w = World::empty(c);
    if sealed {
        for (x, y) in [(1, 2), (1, 4), (0, 3), (2, 3)] {
            w.apply(Command::SetMaterial {
                x,
                y,
                z: 0,
                material: Material::Bedrock,
            });
        }
    }
    w.set_spring_cell(Some((1, 3, 0)));
    w.apply(Command::ChargeAquifer {
        volume_m3: aquifer_m3,
    });
    w
}

#[test]
fn a_full_or_roofed_spring_cell_discharges_nothing_and_the_aquifer_keeps_it() {
    // The control: the same sealed cell, empty. Head 10 m over a 3 m spring drives
    // 0.5 * 7 * DT = 0.175 m3 into it, which is well inside the cell's room.
    let mut open = spring_world(4.0, true);
    open.step();
    assert!(
        (open.view().free_at(1, 3, 0) - 0.175).abs() < 1e-9,
        "{}",
        open.view().free_at(1, 3, 0)
    );
    assert!(
        (open.view().aquifer_m3 - 3.825).abs() < 1e-9,
        "{}",
        open.view().aquifer_m3
    );

    // Now fill it: no room, no discharge, and the aquifer is exactly where it was.
    let mut full = spring_world(4.0, true);
    full.apply(Command::AddWater {
        x: 1,
        y: 3,
        z: 0,
        volume_m3: 1.0,
    });
    let before = full.view().aquifer_m3;
    full.step();
    assert_eq!(
        full.view().aquifer_m3,
        before,
        "the aquifer paid for a blocked spring"
    );
    assert!((full.view().free_at(1, 3, 0) - 1.0).abs() < 1e-12);
    assert!(residual(&full).abs() < 1e-9);
}

#[test]
fn a_spring_above_the_aquifer_head_discharges_nothing() {
    // Head 2.5 m, spring floor at 3 m: no excess, so no flow at all.
    let mut w = spring_world(1.0, false);
    w.step();
    assert_eq!(w.view().aquifer_m3, 1.0, "the aquifer leaked without head");
    let wet: f64 = (0..4).map(|x| column(&w, x)).sum();
    assert_eq!(wet, 0.0, "water appeared without head: {wet}");
    assert!(residual(&w).abs() < 1e-9);
}

// ------------------------------------------------------------------ receipts

#[test]
fn a_clipped_addition_and_an_overlarge_withdrawal_report_what_they_moved() {
    let mut w = World::empty(cfg(4, 6));

    // One cell holds one cubic metre at 1 m voxels, so 1.5 asked gets 1.0 accepted.
    let got = w.apply(Command::AddWater {
        x: 1,
        y: 1,
        z: 0,
        volume_m3: 1.5,
    });
    assert!((got - 1.0).abs() < 1e-12, "receipt {got}");
    assert!(
        (w.view().stored_m3() - 1.0).abs() < 1e-12,
        "stored {}",
        w.view().stored_m3()
    );
    assert!(
        (w.view().ledger.user_in - 1.0).abs() < 1e-12,
        "user_in {}",
        w.view().ledger.user_in
    );

    // Charge the aquifer with 0.5 and then ask for 2.0 back: the receipt is -0.5, the
    // stock is empty, the store is where it was and `user_in` netted out.
    assert!((w.apply(Command::ChargeAquifer { volume_m3: 0.5 }) - 0.5).abs() < 1e-12);
    let back = w.apply(Command::ChargeAquifer { volume_m3: -2.0 });
    assert!((back + 0.5).abs() < 1e-12, "receipt {back}");
    assert_eq!(w.view().aquifer_m3, 0.0);
    assert!(
        (w.view().stored_m3() - 1.0).abs() < 1e-12,
        "stored {}",
        w.view().stored_m3()
    );
    assert!(
        (w.view().ledger.user_in - 1.0).abs() < 1e-12,
        "user_in {}",
        w.view().ledger.user_in
    );

    // Refusals accept nothing and book nothing; non-water commands report zero.
    assert_eq!(
        w.apply(Command::AddWater {
            x: 1,
            y: 2,
            z: 0,
            volume_m3: -1.0
        }),
        0.0
    );
    assert_eq!(
        w.apply(Command::AddWater {
            x: 1,
            y: 2,
            z: 0,
            volume_m3: f64::NAN
        }),
        0.0
    );
    assert_eq!(
        w.apply(Command::RainPulse {
            volume_m3: f64::INFINITY
        }),
        0.0
    );
    assert_eq!(
        w.apply(Command::ChargeAquifer {
            volume_m3: f64::NAN
        }),
        0.0
    );
    assert_eq!(w.apply(Command::SetOutlet { open: true }), 0.0);
    assert_eq!(
        w.apply(Command::SetMaterial {
            x: 3,
            y: 1,
            z: 0,
            material: Material::Rock
        }),
        0.0
    );
    assert_eq!(
        w.view().ledger.rain_in,
        0.0,
        "a refused pulse was booked as rain"
    );
    assert!(
        (w.view().ledger.user_in - 1.0).abs() < 1e-12,
        "user_in {}",
        w.view().ledger.user_in
    );
    assert_eq!(w.view().free_at(1, 2, 0), 0.0);
    assert!(residual(&w).abs() < 1e-9);
}

// ------------------------------------------------------------- material edits

/// Saturated soil at `(1, 1)` on a four-column ring, made by turning a brim-full air
/// cell into soil: the soil takes its whole 0.35 m3 pore capacity and the other 0.65 m3
/// is already sitting in its three void face neighbours, `(0, 1)`, `(2, 1)` and
/// `(1, 2)` — `(1, 0)` is the bedrock foundation. Never stepped, so nothing has drained.
fn wet_soil() -> World {
    let mut w = World::empty(cfg(4, 6));
    w.apply(Command::AddWater {
        x: 1,
        y: 1,
        z: 0,
        volume_m3: 1.0,
    });
    w.apply(Command::SetMaterial {
        x: 1,
        y: 1,
        z: 0,
        material: Material::Soil,
    });
    assert!(
        (w.view().pore_at(1, 1, 0) - 1.0).abs() < 1e-9,
        "the soil must start saturated"
    );
    w
}

#[test]
fn set_material_moves_displaced_water_instead_of_booking_it_out() {
    let mut w = World::empty(cfg(4, 6));
    w.apply(Command::AddWater {
        x: 1,
        y: 1,
        z: 0,
        volume_m3: 1.0,
    });
    let before = w.view().stored_m3();
    w.apply(Command::SetMaterial {
        x: 1,
        y: 1,
        z: 0,
        material: Material::Rock,
    });
    let after = w.view().stored_m3();
    // The rock keeps its pore share (0.02); the remaining 0.98 is shared equally by the
    // three void cells one step away.
    assert!((before - after).abs() < 1e-9, "{before} -> {after}");
    assert!(
        (w.view().pore_at(1, 1, 0) - 1.0).abs() < 1e-9,
        "{}",
        w.view().pore_at(1, 1, 0)
    );
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
    w.apply(Command::SetMaterial {
        x: 1,
        y: 1,
        z: 0,
        material: Material::Rock,
    });
    // Rock holds 0.02 of the voxel against soil's 0.35, so a full rock pore keeps
    // 0.02 m3 and the other 0.33 m3 is shared by the three neighbouring voids. Keeping
    // the *fraction* would have silently dropped that 0.33.
    assert!(
        (w.view().pore_at(1, 1, 0) - 1.0).abs() < 1e-9,
        "{}",
        w.view().pore_at(1, 1, 0)
    );
    for (x, y) in [(0, 1), (2, 1), (1, 2)] {
        let f = w.view().free_at(x, y, 0);
        assert!((f - 0.98 / 3.0).abs() < 1e-9, "({x}, {y}) got {f}");
    }
    assert!(
        (w.view().stored_m3() - before).abs() < 1e-9,
        "{}",
        w.view().stored_m3()
    );
    assert_eq!(w.view().ledger.displaced_out, 0.0);
    assert!(residual(&w).abs() < 1e-9);
}

#[test]
fn wet_soil_turned_to_air_releases_its_pore_water_as_free_water() {
    let mut w = wet_soil();
    let before = w.view().stored_m3();
    w.apply(Command::SetMaterial {
        x: 1,
        y: 1,
        z: 0,
        material: Material::Air,
    });
    // Air has no pores: the soil's 0.35 m3 becomes free water, and it fits in the cell.
    assert!(
        (w.view().free_at(1, 1, 0) - 0.35).abs() < 1e-9,
        "{}",
        w.view().free_at(1, 1, 0)
    );
    assert_eq!(w.view().pore_at(1, 1, 0), 0.0);
    assert!(
        (w.view().stored_m3() - before).abs() < 1e-9,
        "{}",
        w.view().stored_m3()
    );
    assert_eq!(w.view().ledger.displaced_out, 0.0);
    assert!(residual(&w).abs() < 1e-9);
}

#[test]
fn a_full_recipient_is_skipped_and_the_next_shell_takes_the_water() {
    let mut w = wet_soil();
    for (x, y) in [(0, 1), (2, 1), (1, 2)] {
        w.apply(Command::AddWater {
            x,
            y,
            z: 0,
            volume_m3: 1.0,
        });
    }
    let before = w.view().stored_m3();
    w.apply(Command::SetMaterial {
        x: 1,
        y: 1,
        z: 0,
        material: Material::Rock,
    });
    // Every cell one step away is brim full, so the 0.33 goes to the four cells two
    // void steps away: (3, 1) round the seam, (0, 2), (2, 2) and (1, 3).
    for (x, y) in [(0, 1), (2, 1), (1, 2)] {
        assert!(
            (w.view().free_at(x, y, 0) - 1.0).abs() < 1e-9,
            "({x}, {y}) overfilled"
        );
    }
    for (x, y) in [(3, 1), (0, 2), (2, 2), (1, 3)] {
        let f = w.view().free_at(x, y, 0);
        assert!((f - 0.33 / 4.0).abs() < 1e-9, "({x}, {y}) got {f}");
    }
    assert!((w.view().stored_m3() - before).abs() < 1e-9);
    assert_eq!(w.view().ledger.displaced_out, 0.0);
    assert!(residual(&w).abs() < 1e-9);
}

/// `free_transfer_cap` caps what one **face** may pass in one substep, so a fill travels
/// more slowly still. The local exchange already makes a fill travel — that is what
/// replaced the instantaneous re-level — so what this pins is that the cap slows it
/// further and never loses any of it.
#[test]
fn the_transfer_cap_makes_a_fill_travel_more_slowly() {
    fn ring(cap: f64, ticks: u32) -> World {
        let mut config = cfg(8, 4);
        config.free_transfer_cap = cap;
        let mut w = World::empty(config);
        pour(&mut w, 0, 2.0);
        run(&mut w, ticks);
        w
    }

    // One tick spreads a fill a few cells and nowhere near its level: the far side of an
    // eight-column ring is a long way short of 2 / 8 either way, and the cap is behind.
    // (Package P moved the far column's first-tick fill from 0.094 to 0.125, half its
    // level: the two-row fill no longer overshoots out of column 0, so the two fronts
    // meet at column 4 a little fuller. The bound was 0.1.)
    let (quick, slow) = (ring(0.0, 1), ring(0.02, 1));
    assert!(
        column(&quick, 4) < 0.15,
        "uncapped levelled in one tick: {}",
        column(&quick, 4)
    );
    assert!(
        column(&slow, 4) < column(&quick, 4),
        "the cap did not slow it in one tick"
    );

    // Twenty ticks in, the uncapped ring is at its level and the capped one is still
    // behind: the cap is a transit limit, not a different answer.
    let (quick, slow) = (ring(0.0, 20), ring(0.02, 20));
    assert!(
        (column(&quick, 4) - 0.25).abs() < 1e-3,
        "uncapped: {}",
        column(&quick, 4)
    );
    assert!(
        column(&slow, 4) < column(&quick, 4) - 1e-6,
        "the cap did not slow it: {}",
        column(&slow, 4)
    );
    assert!(column(&slow, 4) > 0.0, "the cap stopped it altogether");

    // And given long enough the cap changes nothing about where the water ends up.
    let (quick, slow) = (ring(0.0, 400), ring(0.02, 400));
    for w in [&quick, &slow] {
        assert!(
            (column(w, 4) - 0.25).abs() < 1e-3,
            "not level: {}",
            column(w, 4)
        );
    }
    for w in [&quick, &slow] {
        assert!(
            (w.view().stored_m3() - 2.0).abs() < 1e-9,
            "mass: {}",
            w.view().stored_m3()
        );
        assert!(residual(w).abs() < 1e-9);
    }
}

#[test]
fn the_same_seed_gives_the_same_run() {
    let config = Config {
        width: 48,
        height: 24,
        depth: 4,
        rain_m_per_s: 0.002,
        ..Config::default()
    };
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

/// Schema 1 is the pre-plant-boundary format: no `transpiration_out`, no
/// `terrain_version`. It is refused, not migrated.
#[test]
fn a_wrong_schema_tag_is_refused() {
    let w = World::empty(cfg(4, 5));
    let mut bytes = w.save();
    bytes[0] = 1;
    let err = World::load(&bytes).expect_err("must refuse another schema");
    assert!(format!("{err}").contains("schema"), "{err}");
}

#[test]
fn the_plant_boundary_fields_round_trip() {
    let mut w = wet_soil();
    w.apply(Command::SetMaterial {
        x: 3,
        y: 4,
        z: 0,
        material: Material::Rock,
    });
    let took = w.apply(Command::WithdrawPore {
        x: 1,
        y: 1,
        z: 0,
        volume_m3: 0.01,
    });
    assert!(took < 0.0, "a withdrawal is negative: {took}");
    assert!(w.terrain_version() > 0 && w.view().ledger.transpiration_out > 0.0);
    let back = World::load(&w.save()).expect("round trip");
    assert_eq!(w, back);
    assert_eq!(back.terrain_version(), w.terrain_version());
    assert_eq!(
        back.view().ledger.transpiration_out,
        w.view().ledger.transpiration_out
    );
}

// ------------------------------------------------------------------ generator

#[test]
fn the_generator_has_no_isolated_voids_a_ridge_above_the_basin_and_a_clean_seam() {
    for seed in [1u64, 2, 77] {
        let world = World::new(Config {
            seed,
            ..Config::default()
        });
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
        assert!(
            high >= low + 6,
            "seed {seed}: ridge {high} over basin {low}"
        );

        for z in 0..v.config.depth {
            let last = v.surface_y(v.config.width as i64 - 1, z).unwrap() as i64;
            let first = v.surface_y(0, z).unwrap() as i64;
            assert!(
                (first - last).abs() <= 2,
                "seed {seed}, z {z}: seam {last} -> {first}"
            );
            for y in 0..v.config.height {
                assert_eq!(
                    v.material_at(-1, y, z),
                    v.material_at(v.config.width as i64 - 1, y, z)
                );
            }
        }

        // Nothing roofed: the camera has to read every surface cell, so the default
        // landform carves no overhang and no covered passage. Those live in the
        // hand-built fixtures in `generate`'s tests and in the presenter's scene.
        let roofed = (1..v.config.height - 1)
            .flat_map(|y| (0..v.config.width as i64).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                (0..v.config.depth).any(|z| {
                    !v.material_at(x, y, z).is_solid() && v.material_at(x, y + 1, z).is_solid()
                })
            })
            .count();
        assert_eq!(
            roofed, 0,
            "seed {seed}: {roofed} roofed cells in the default landform"
        );

        assert!(world.outlet_cell().is_some(), "seed {seed}: no outlet");
        assert!(world.spring_cell().is_some(), "seed {seed}: no spring");
    }
}

// ------------------------------------------------------- the plant boundary

/// A saturated soil voxel holds `pore_capacity` cubic metres at `voxel_m = 1`: 0.35.
#[test]
fn withdraw_pore_caps_at_the_stock_and_books_the_ledger() {
    let mut w = wet_soil();
    let stock = w.view().pore_water_m3(1, 1, 0);
    assert!((stock - 0.35).abs() < 1e-12, "saturated soil holds {stock}");

    // A bite out of the middle: exactly what was asked for, booked as a loss.
    let got = w.apply(Command::WithdrawPore {
        x: 1,
        y: 1,
        z: 0,
        volume_m3: 0.1,
    });
    assert!(
        (got + 0.1).abs() < 1e-12,
        "a withdrawal is negative and exact: {got}"
    );
    assert!((w.view().ledger.transpiration_out - 0.1).abs() < 1e-12);
    assert!((w.view().pore_water_m3(1, 1, 0) - 0.25).abs() < 1e-12);

    // And then more than is left: capped by the stock, and the stock is empty, not
    // negative.
    let left = w.view().pore_water_m3(1, 1, 0);
    let got = w.apply(Command::WithdrawPore {
        x: 1,
        y: 1,
        z: 0,
        volume_m3: 10.0,
    });
    assert!(
        (got + left).abs() < 1e-12,
        "took {got}, the voxel held {left}"
    );
    assert_eq!(w.view().pore_at(1, 1, 0), 0.0);
    assert!((w.view().ledger.transpiration_out - 0.35).abs() < 1e-12);
    // The loss term is what keeps conservation: stored fell by exactly what was booked.
    assert!(residual(&w).abs() < 1e-9, "{}", residual(&w));
}

#[test]
fn withdraw_pore_refuses_a_bad_volume_and_takes_nothing_from_air() {
    let mut w = wet_soil();
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            w.apply(Command::WithdrawPore {
                x: 1,
                y: 1,
                z: 0,
                volume_m3: bad
            }),
            0.0
        );
    }
    assert_eq!(
        w.view().ledger.transpiration_out,
        0.0,
        "a refused volume books nothing"
    );
    assert_eq!(w.view().pore_at(1, 1, 0), 1.0, "and takes nothing");

    // Air has no pore space at all: zero accepted, zero booked.
    assert_eq!(
        w.apply(Command::WithdrawPore {
            x: 1,
            y: 4,
            z: 0,
            volume_m3: 1.0
        }),
        0.0
    );
    // Bedrock has none either.
    assert_eq!(
        w.apply(Command::WithdrawPore {
            x: 1,
            y: 0,
            z: 0,
            volume_m3: 1.0
        }),
        0.0
    );
    assert_eq!(w.view().ledger.transpiration_out, 0.0);
    // Outside the world: refused, nothing booked.
    assert_eq!(
        w.apply(Command::WithdrawPore {
            x: 1,
            y: 99,
            z: 0,
            volume_m3: 1.0
        }),
        0.0
    );
    assert_eq!(
        w.apply(Command::WithdrawPore {
            x: 1,
            y: 1,
            z: 9,
            volume_m3: 1.0
        }),
        0.0
    );
    assert_eq!(w.view().ledger.transpiration_out, 0.0);
}

/// A support face needs void above it *inside* the world, so the topmost row is never
/// one however solid it is; a roofed floor is one, because a plant's room is the void
/// over the face, not the open sky.
#[test]
fn is_support_skips_the_top_row_and_accepts_a_roofed_floor() {
    let c = cfg(4, 5);
    let mut w = World::empty(c.clone());
    for x in 0..4 {
        w.apply(Command::SetMaterial {
            x,
            y: 4,
            z: 0,
            material: Material::Rock,
        });
        w.apply(Command::SetMaterial {
            x,
            y: 2,
            z: 0,
            material: Material::Rock,
        });
    }
    let v = w.view();
    assert!(!v.is_support(0, 4, 0), "the top row has no room above it");
    assert!(
        v.is_support(0, 0, 0),
        "the foundation under a roof is a support"
    );
    assert!(v.is_support(0, 2, 0), "and so is the roof's own top face");
    assert!(!v.is_support(0, 1, 0), "air is not a support");
    assert_eq!(
        v.supports_in_column(0, 0),
        vec![0, 2],
        "ascending, and only the real ones"
    );
    assert_eq!(
        v.supports_in_column(0, 9),
        Vec::<u32>::new(),
        "no such slab"
    );
}

#[test]
fn water_depth_sums_fractional_fills_and_stops_at_a_dry_cell() {
    let mut w = World::empty(cfg(4, 8));
    w.apply(Command::AddWater {
        x: 1,
        y: 1,
        z: 0,
        volume_m3: 0.5,
    });
    w.apply(Command::AddWater {
        x: 1,
        y: 2,
        z: 0,
        volume_m3: 0.3,
    });
    // y = 3 stays dry; the water above it is not standing on this face.
    w.apply(Command::AddWater {
        x: 1,
        y: 4,
        z: 0,
        volume_m3: 0.4,
    });
    let v = w.view();
    assert!(
        (v.water_depth_m(1, 0, 0) - 0.8).abs() < 1e-12,
        "{}",
        v.water_depth_m(1, 0, 0)
    );
    assert_eq!(v.water_depth_m(2, 0, 0), 0.0, "a dry face reads zero");

    // A solid stops it too, and `voxel_m` scales it.
    let mut w = World::empty(Config {
        voxel_m: 0.25,
        ..cfg(4, 8)
    });
    w.apply(Command::AddWater {
        x: 1,
        y: 1,
        z: 0,
        volume_m3: 0.5 * 0.25 * 0.25 * 0.25,
    });
    w.apply(Command::SetMaterial {
        x: 1,
        y: 2,
        z: 0,
        material: Material::Rock,
    });
    w.apply(Command::AddWater {
        x: 1,
        y: 3,
        z: 0,
        volume_m3: 0.25 * 0.25 * 0.25,
    });
    let v = w.view();
    assert!(
        (v.water_depth_m(1, 0, 0) - 0.125).abs() < 1e-12,
        "{}",
        v.water_depth_m(1, 0, 0)
    );
}

#[test]
fn soil_below_stops_at_rock() {
    let mut w = World::empty(cfg(4, 8));
    for y in 1..=2 {
        w.apply(Command::SetMaterial {
            x: 1,
            y,
            z: 0,
            material: Material::Rock,
        });
    }
    for y in 3..=5 {
        w.apply(Command::SetMaterial {
            x: 1,
            y,
            z: 0,
            material: Material::Soil,
        });
    }
    let v = w.view();
    assert_eq!(
        v.soil_below(1, 5, 0),
        3,
        "three soil voxels down to the rock"
    );
    assert_eq!(v.soil_below(1, 4, 0), 2);
    assert_eq!(v.soil_below(1, 2, 0), 0, "rock is not soil");
    assert_eq!(v.soil_below(1, 6, 0), 0, "air is not soil");
}

// ------------------------------------------------------------ sky visibility

/// Seven slabs deep, so a 30-degree ray cannot slip out of the front or back wall
/// before a roof one voxel up has stopped it.
fn sky_fixture(width: u32, height: u32) -> World {
    World::empty(Config {
        width,
        height,
        depth: 7,
        voxel_m: 1.0,
        ..Config::default()
    })
}

fn roof(w: &mut World, y: u32) {
    for z in 0..w.config().depth {
        for x in 0..w.config().width as i64 {
            w.apply(Command::SetMaterial {
                x,
                y,
                z,
                material: Material::Rock,
            });
        }
    }
}

/// A rock wall across every slab: the fan must go over it, not round it.
fn sky_wall(w: &mut World, x: i64, ys: std::ops::RangeInclusive<u32>) {
    for y in ys {
        for z in 0..w.config().depth {
            w.apply(Command::SetMaterial {
                x,
                y,
                z,
                material: Material::Rock,
            });
        }
    }
}

#[test]
fn sky_visibility_is_one_on_an_open_plain_and_zero_under_a_roof() {
    let mut w = sky_fixture(8, 6);
    let v = w.view();
    for z in 0..7 {
        for x in 0..8 {
            assert_eq!(v.sky_visibility(x, 0, z), 1.0, "({x}, 0, {z}) is open sky");
        }
    }

    // One voxel of headroom and a complete roof: nothing gets out.
    roof(&mut w, 2);
    let v = w.view();
    assert_eq!(v.sky_visibility(0, 0, 3), 0.0, "a roofed floor sees no sky");
    assert_eq!(
        v.sky_visibility(0, 2, 3),
        1.0,
        "the roof's own top face does"
    );
}

#[test]
fn sky_visibility_beside_a_wall_is_between_and_mirrors_exactly() {
    // A two-voxel wall on a sixteen-column ring: the far side of the ring clears it at
    // 30 degrees of elevation, the column against it does not.
    let mut w = sky_fixture(16, 8);
    sky_wall(&mut w, 4, 1..=2);
    let open = w.view().sky_visibility(10, 0, 3);
    let beside = w.view().sky_visibility(5, 0, 3);
    assert_eq!(
        open, 1.0,
        "six columns from a two-voxel wall is still open sky"
    );
    assert!(beside > 0.0 && beside < 1.0, "beside a wall: {beside}");
    assert!(
        beside < open,
        "beside the wall must be darker than away from it"
    );

    // A fixture symmetric under x -> 7 - x (walls at 2 and 5, which map onto each
    // other) gives its two mirrored sites the same answer, bit for bit: the fan's
    // azimuths are closed under negating dx.
    let mut w = sky_fixture(8, 8);
    sky_wall(&mut w, 2, 1..=5);
    sky_wall(&mut w, 5, 1..=5);
    let v = w.view();
    for z in 0..7 {
        for x in 0..8 {
            assert_eq!(
                v.sky_visibility(x, 0, z),
                v.sky_visibility(7 - x, 0, z),
                "({x}, 0, {z}) is not the mirror of ({}, 0, {z})",
                7 - x
            );
        }
    }
    assert!(
        v.sky_visibility(3, 0, 3) < 1.0,
        "the site between the walls is shaded"
    );
}

// ---------------------------------------------------------- terrain version

#[test]
fn terrain_version_bumps_only_on_a_real_change() {
    let mut w = World::empty(cfg(4, 6));
    assert_eq!(w.terrain_version(), 0);
    w.apply(Command::SetMaterial {
        x: 1,
        y: 1,
        z: 0,
        material: Material::Soil,
    });
    assert_eq!(w.terrain_version(), 1);
    w.apply(Command::SetMaterial {
        x: 1,
        y: 1,
        z: 0,
        material: Material::Soil,
    });
    assert_eq!(
        w.terrain_version(),
        1,
        "setting the same material changes no terrain"
    );
    w.apply(Command::SetMaterial {
        x: 1,
        y: 9,
        z: 0,
        material: Material::Rock,
    });
    assert_eq!(w.terrain_version(), 1, "a refused edit changes no terrain");
    w.apply(Command::AddWater {
        x: 1,
        y: 2,
        z: 0,
        volume_m3: 0.2,
    });
    run(&mut w, 5);
    assert_eq!(w.terrain_version(), 1, "water is not terrain");
    w.apply(Command::SetMaterial {
        x: 1,
        y: 1,
        z: 0,
        material: Material::Rock,
    });
    assert_eq!(w.terrain_version(), 2);
}

// ----------------------------------------------------------------- noise seed

/// The decisive experiment's instrument: re-draw the final weak wobble and nothing else
/// moves. The main stream advances identically, so the rock phase, the strata warp and
/// the soil pockets are the same numbers; only the surface's last centimetres differ.
#[test]
fn noise_seed_moves_the_wobble_and_leaves_the_landform() {
    let base = Config {
        width: 64,
        height: 32,
        depth: 6,
        seed: 5,
        ..Config::default()
    };
    let a = World::new(base.clone());
    let b = World::new(Config {
        noise_seed: 99,
        ..base.clone()
    });
    let c = World::new(Config {
        noise_seed: 99,
        ..base.clone()
    });
    assert_eq!(b, c, "a noise seed is a seed: same inputs, same world");

    let (va, vb) = (a.view(), b.view());
    let mut moved = 0;
    for z in 0..base.depth {
        for x in 0..base.width as i64 {
            let (ya, yb) = (
                va.surface_y(x, z).unwrap() as i64,
                vb.surface_y(x, z).unwrap() as i64,
            );
            if ya != yb {
                moved += 1;
            }
            assert!(
                (ya - yb).abs() <= 2,
                "({x}, {z}) moved {ya} -> {yb}: not a weak wobble"
            );
        }
    }
    assert!(moved > 0, "the wobble did not move at all");
    assert!(
        moved < (base.width * base.depth) as i32 / 2,
        "{moved} of {} surface cells moved: that is not a weak wobble",
        base.width * base.depth
    );

    // The body under a column the wobble left alone is identical, voxel for voxel: the
    // rock thickness phase, the strata warp and the soil pockets are drawn after the
    // wobble from the same stream positions, so they cannot have moved. Soil depth
    // reads the two neighbouring columns, so a column only counts as untouched when its
    // neighbours are untouched too.
    let mut checked = 0;
    for z in 0..base.depth {
        for x in 0..base.width as i64 {
            if (-1..=1).any(|d| va.surface_y(x + d, z) != vb.surface_y(x + d, z)) {
                continue;
            }
            checked += 1;
            for y in 0..base.height {
                assert_eq!(
                    va.material_at(x, y, z),
                    vb.material_at(x, y, z),
                    "the body moved at ({x}, {y}, {z}) under an unmoved surface"
                );
            }
        }
    }
    assert!(
        checked > (base.width * base.depth) as i32 / 4,
        "only {checked} columns held still"
    );

    // The basin keeps its slab and its floor to within the wobble. Its *column* does
    // not, and that is not something the shared stream can give: the outlet is the
    // argmin over a basin floor the flattening rule makes nearly level, so a one-voxel
    // wobble hands the title to a different column, and the spring is placed from the
    // outlet's `x`. Nothing downstream of the wobble is invariant — only the stream is.
    let (ao, bo) = (a.outlet_cell().unwrap(), b.outlet_cell().unwrap());
    assert_eq!(
        ao.2, bo.2,
        "the outlet left the front slab: {ao:?} -> {bo:?}"
    );
    assert!(
        ao.1.abs_diff(bo.1) <= 1,
        "the basin floor moved more than the wobble: {ao:?} -> {bo:?}"
    );
}

// ------------------------------------------------------- rain on a slope

/// A staircase of soil six columns wide, stepping down toward `x = 0`, with a flat pair
/// of columns at its foot: `y = 1, 1, 2, 3, 4, 5, 6, 7` across the eight columns, one
/// soil voxel on a bedrock body, so every support face is permeable and drains to the
/// aquifer rather than into rock.
///
/// The wrap from `x = 7` down to `x = 0` is a cliff, which is just the steepest step.
fn staircase() -> World {
    let config = Config {
        width: 8,
        height: 12,
        depth: 1,
        voxel_m: 1.0,
        seed: 11,
        // 0.0025 and not 0.01 since package 1c
        // (`design/handoffs/voxel-water-units-2026-09-22.md`). This fixture's premise,
        // stated on both tests below, is that the rain is far under what the soil can
        // take — a seventh of it — so nothing the staircase does is runoff for want of
        // capacity. On these 1 m cells, four times the reference, the units fix makes
        // infiltration four times slower (8.75e-4 m3 a tick against the old 3.5e-3), so
        // the rain is quartered with it and the ratio is exactly what it was. A fixture
        // value, not a tuning: `rain_m_per_s` is untouched everywhere else.
        rain_m_per_s: 0.0025,
        water_substeps: 4,
        ..Config::default()
    };
    let mut w = World::empty(config.clone());
    for x in 0..8i64 {
        let top = if x < 2 { 1 } else { x as u32 };
        for y in 1..top {
            w.apply(Command::SetMaterial {
                x,
                y,
                z: 0,
                material: Material::Bedrock,
            });
        }
        w.apply(Command::SetMaterial {
            x,
            y: top,
            z: 0,
            material: Material::Soil,
        });
    }
    w
}

/// Rain has to wet a slope. The rate here is far below what the soil can take —
/// `rain_m_per_s * DT * cell_area` is 1.25e-4 m3 per column per tick against an
/// infiltration capacity of 8.75e-4 — so the only thing that can keep a sloping support
/// dry is the tick's own ordering: free water that is carried downhill before it is
/// offered to the soil under it never infiltrates at all.
#[test]
fn rain_wets_a_slope_as_well_as_it_wets_a_flat() {
    let mut w = staircase();
    run(&mut w, 400);
    let v = w.view();
    let flat = v.pore_at(0, 1, 0);
    assert!(
        flat > 0.1,
        "the flat foot of the staircase did not wet at all: {flat}"
    );
    for (x, y) in [(3i64, 3u32), (4, 4), (5, 5), (6, 6), (7, 7)] {
        let slope = v.pore_at(x, y, 0);
        assert!(
            slope >= 0.5 * flat && slope <= 2.0 * flat,
            "the support at ({x}, {y}) holds {slope} against the flat's {flat}"
        );
    }
    assert!(residual(&w).abs() < 1e-9, "{}", residual(&w));
}

// ------------------------------------------------------------- the water table

/// A basin between two ridges: soil at y = 1 and y = 2 across the middle four columns,
/// a soil cap at y = 6 on the two columns at each end, bedrock body, no rain.
///
/// `aquifer_porosity` is 0.5 here against the default 0.1, and that is not decoration.
/// A metre of head is `width * depth * cell_area * aquifer_porosity` cubic metres — 0.8
/// at the default against the 1.4 that one row of this basin's soil pores holds — so at
/// the default the aquifer cannot fill the ground it is supposed to be holding up and
/// the table collapses as it tries. At 0.5 a metre of head is 4 cubic metres and the
/// table settles instead.
fn water_table_basin(head_m: f64) -> World {
    let config = Config {
        width: 8,
        height: 10,
        depth: 1,
        voxel_m: 1.0,
        seed: 13,
        aquifer_porosity: 0.5,
        initial_aquifer_head_m: head_m,
        ..Config::default()
    };
    let mut w = World::empty(config.clone());
    for x in 0..8i64 {
        if x < 2 || x >= 6 {
            for y in 1..6 {
                w.apply(Command::SetMaterial {
                    x,
                    y,
                    z: 0,
                    material: Material::Bedrock,
                });
            }
            // The ridge cap starts saturated, so "it drains to field capacity" is a
            // measurement and not a tautology.
            w.apply(Command::AddWater {
                x,
                y: 6,
                z: 0,
                volume_m3: 0.35,
            });
            w.apply(Command::SetMaterial {
                x,
                y: 6,
                z: 0,
                material: Material::Soil,
            });
        } else {
            for y in 1..=2 {
                w.apply(Command::SetMaterial {
                    x,
                    y,
                    z: 0,
                    material: Material::Soil,
                });
            }
        }
    }
    w
}

/// The table saturates what is under it and seeps a pond, while ground above it drains
/// to field capacity as it always did. Nothing here is a flux: every transfer is between
/// the aquifer and a voxel, so the ledger is untouched and the residual must not move.
#[test]
fn the_water_table_saturates_the_basin_and_leaves_the_ridge_to_drain() {
    let mut w = water_table_basin(3.5);
    let charged = w.view().stored_m3();
    assert!(
        (w.aquifer_head_m() - 3.5).abs() < 1e-12,
        "head {}",
        w.aquifer_head_m()
    );
    run(&mut w, 500);

    let head = w.aquifer_head_m();
    let v = w.view();
    for x in 2..6 {
        for y in 1..=2 {
            let pore = v.pore_at(x, y, 0);
            assert!(pore > 0.99, "basin soil at ({x}, {y}) is only at {pore}");
        }
        // The pond: free water in the void over the basin, and its surface is the table
        // rather than some level of its own.
        let free = v.free_at(x, 3, 0);
        assert!(free > 0.0, "no pond over the basin at x {x}");
        let surface = 3.0 + free;
        assert!(
            (surface - head).abs() <= 1.0,
            "the pond's surface {surface} is not within a voxel of the table at {head}"
        );
    }
    for x in [0, 1, 6, 7] {
        let pore = v.pore_at(x, 6, 0);
        assert!(
            pore <= 0.25 + 1e-9,
            "the ridge cap at x {x} held {pore}, above field capacity"
        );
    }
    // The water only moved about: the world holds what it was charged with.
    assert!(
        (v.stored_m3() - charged).abs() < 1e-9,
        "{} against {charged}",
        v.stored_m3()
    );
    assert!(residual(&w).abs() < 1e-9, "{}", residual(&w));
}

/// A table at zero is no table at all: the staircase steps to exactly the state it
/// reached before the water table existed, down to the last bit of every pore fraction.
#[test]
fn a_table_at_zero_changes_nothing() {
    let mut w = staircase();
    assert_eq!(
        w.config().initial_aquifer_head_m,
        0.0,
        "the default is a dry aquifer"
    );
    // 1600 ticks and not 400: the fixture's rain is a quarter of what it was (see
    // `staircase`), so it takes four times as long to deliver the same 0.2 m3 a column
    // and put every support past its field capacity with drainage carrying the rest.
    // The claim — a table at zero leaves the staircase exactly where it stood without
    // one, down to the last bit — is unchanged, and this is still a settled state.
    run(&mut w, 1600);
    let v = w.view();
    for x in 0..8i64 {
        let top = if x < 2 { 1 } else { x as u32 };
        assert_eq!(v.pore_at(x, top, 0), 0.25, "x {x} moved");
        // The pore fraction is still exact; the free crumb above it is 8e-18 rather
        // than a hard zero since package 1c, because the tick now delivers the rain in
        // four times as many, four times smaller infiltration steps and the last one
        // leaves a rounding of the cell behind. Not standing water by any reading.
        assert!(
            v.free_at(x, top + 1, 0) < 1e-12,
            "x {x} is standing in water: {}",
            v.free_at(x, top + 1, 0)
        );
    }
    // The aquifer took the rest, and its head stayed under the soil it would saturate.
    assert!((v.aquifer_m3 - 0.9).abs() < 1e-9, "{}", v.aquifer_m3);
    assert!(
        w.aquifer_head_m() < 1.5,
        "the table reached the soil: {}",
        w.aquifer_head_m()
    );
    assert!(residual(&w).abs() < 1e-9, "{}", residual(&w));
}
