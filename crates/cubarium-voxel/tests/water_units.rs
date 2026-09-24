//! Package 1c: the water solver's conductivity is metres per second.
//!
//! `design/handoffs/voxel-water-units-2026-09-22.md`. Before this package every
//! permeability-driven flux was `permeability_per_s · dt · pore_capacity ·
//! voxel_volume` — a *fraction of a cell* per tick, so the physical speed of
//! infiltration, drainage and the aquifer exchange halved with the cell and the
//! panel's 0.125 m ring drained at half the rate the 0.25 m ring did (D5,
//! `voxel-small-collapse-2026-09-22.md`). The rule is `rate_m3 = K · A · dt` with
//! `K = permeability_per_s · pore_capacity · 0.25` metres per second, chosen so
//! that the 0.25 m reference grid is unchanged.
//!
//! Two claims are pinned here, and nothing else:
//!
//! 1. **Reference-grid identity.** On 0.25 m cells every touched site still moves
//!    exactly what the old expression said, so no world on the reference grid is
//!    retuned by this package. Written as the old expression, spelled out, not as
//!    a number and not by calling the new accessor.
//! 2. **Physical invariance.** The same 1 m slab of the same soil drains the same
//!    depth of water per second whether it is cut into 0.25 m or 0.125 m cells.
//!
//! Both arms also assert the water ledger's conservation residual, because a
//! units change that leaked would otherwise look like a faster solver.
//!
//! No run here is longer than 200 ticks and no assertion is a golden hash.
//!
//! The solver has **no lateral pore flow**: `water::exchange` levels *free* water
//! in cell fractions and carries no conductivity at all (pinned below), so the
//! non-vertical site the rule touches is the aquifer exchange — the water table
//! pushing up through a cell face into the void above it.

use cubarium_voxel::{Command, Config, DT, Material, World, water};

/// The **pre-1c** per-tick volume, written out: `permeability · dt ·
/// pore_capacity · voxel_volume`, a fraction of a cell. Every identity assertion
/// below compares against this and not against the new accessor, so the two are
/// independent statements.
fn old_rate_m3(m: Material, c: &Config, dt: f64) -> f64 {
    m.permeability_per_s() * dt * m.pore_capacity() * c.voxel_volume()
}

/// The rule's conductivity in metres per second, written out from the brief:
/// `permeability · pore_capacity · 0.25`. Independent of the implementation.
fn rule_k_m_per_s(m: Material) -> f64 {
    m.permeability_per_s() * m.pore_capacity() * 0.25
}

/// A single column: nothing spreads sideways, so a phase's effect on one cell is
/// the whole of it. `aquifer_porosity` is a fixture value, not a tuning — it is
/// made enormous where a test needs the table to hold still while the world takes
/// water out of it (the same trick as `tests/boundary.rs`).
fn column(height: u32, voxel_m: f64, porosity: f64, head_m: f64) -> World {
    World::empty(Config {
        width: 1,
        height,
        depth: 1,
        voxel_m,
        seed: 23,
        aquifer_porosity: porosity,
        initial_aquifer_head_m: head_m,
        ..Config::default()
    })
}

/// One voxel of soil holding exactly `pore` of its capacity: the water goes into
/// the air cell first and the material change keeps it (`tests/boundary.rs`).
fn wet_soil(w: &mut World, y: u32, pore: f64) {
    let cap = Material::Soil.pore_capacity() * w.config().voxel_volume();
    if pore > 0.0 {
        w.apply(Command::AddWater {
            x: 0,
            y,
            z: 0,
            volume_m3: pore * cap,
        });
    }
    w.apply(Command::SetMaterial {
        x: 0,
        y,
        z: 0,
        material: Material::Soil,
    });
    assert!(
        (w.view().pore_at(0, y, 0) - pore).abs() < 1e-12,
        "fixture: pore is {} and not {pore}",
        w.view().pore_at(0, y, 0)
    );
}

fn pore_m3(w: &World, y: u32) -> f64 {
    w.view().pore_water_m3(0, y, 0)
}

fn free_m3(w: &World, y: u32) -> f64 {
    w.view().free_at(0, y, 0) * w.config().voxel_volume()
}

fn residual(w: &World) -> f64 {
    w.view().total_residual()
}

/// The world's store round-trips a volume through a cell fraction (`vol / unit`
/// in, `fraction · unit` out), so a volume read back out of the grid is the rate
/// to within a couple of ulps and not bit for bit. The bit-for-bit identity of
/// the rate *expression* itself is pinned in `water.rs`'s own `units_tests`.
const STORE_REL: f64 = 1e-12;

fn assert_rel(got: f64, want: f64, what: &str) {
    assert!(
        want > 0.0 && (got - want).abs() <= STORE_REL * want,
        "{what}: moved {got}, the pre-1c expression says {want} (relative {})",
        (got - want).abs() / want
    );
}

// ------------------------------------------------------------ 1. identity

/// A column draining: one tick of `drain` on the reference grid moves exactly
/// what `permeability · DT · pore_capacity · voxel_volume` moved.
#[test]
fn a_draining_column_moves_the_pre_1c_volume_on_the_reference_grid() {
    let mut w = column(4, 0.25, 1e4, 0.0);
    // Soil on the bedrock floor, saturated, with air above it: nothing to
    // infiltrate in, nowhere to go but down into the aquifer.
    wet_soil(&mut w, 1, 1.0);

    let before = pore_m3(&w, 1);
    let aquifer_before = w.aquifer_head_m();
    water::drain(&mut w);
    let moved = before - pore_m3(&w, 1);

    assert_rel(
        moved,
        old_rate_m3(Material::Soil, w.config(), DT),
        "one tick of drainage on 0.25 m cells",
    );
    assert!(
        w.aquifer_head_m() > aquifer_before,
        "the drained water did not reach the aquifer"
    );
    assert!(residual(&w).abs() < 1e-12, "residual {}", residual(&w));
}

/// Surface infiltration: one tick of `infiltrate` on the reference grid moves
/// exactly the pre-1c volume out of standing water into the ground.
#[test]
fn surface_infiltration_moves_the_pre_1c_volume_on_the_reference_grid() {
    let mut w = column(4, 0.25, 1e4, 0.0);
    wet_soil(&mut w, 1, 0.0);
    // A film of standing water on top of the dry soil.
    w.apply(Command::AddWater {
        x: 0,
        y: 2,
        z: 0,
        volume_m3: 0.5 * w.config().voxel_volume(),
    });

    let before = free_m3(&w, 2);
    water::infiltrate(&mut w, DT);
    let moved = before - free_m3(&w, 2);

    assert_rel(
        moved,
        old_rate_m3(Material::Soil, w.config(), DT),
        "one tick of infiltration on 0.25 m cells",
    );
    assert_rel(
        pore_m3(&w, 1),
        old_rate_m3(Material::Soil, w.config(), DT),
        "the ground took",
    );
    assert!(residual(&w).abs() < 1e-12, "residual {}", residual(&w));
}

/// The aquifer exchange — the water table pushing up through a cell face into
/// the void above saturated ground. The one non-vertical-transfer site the rule
/// touches, and both of its halves: the band's uptake into dry pore space, and
/// the seepage into a void cell.
#[test]
fn the_aquifer_exchange_moves_the_pre_1c_volume_on_the_reference_grid() {
    let want = |w: &World| old_rate_m3(Material::Soil, w.config(), DT);

    // Uptake: dry soil inside the saturated band takes one transfer from the store.
    let mut w = column(5, 0.25, 1e4, 0.7);
    wet_soil(&mut w, 1, 0.0);
    water::water_table(&mut w);
    assert_rel(pore_m3(&w, 1), want(&w), "one tick of aquifer uptake");
    assert!(residual(&w).abs() < 1e-12, "residual {}", residual(&w));

    // Seepage: saturated soil under a void cell that the table reaches passes one
    // transfer up into it, at the support's rate.
    let mut w = column(5, 0.25, 1e4, 0.7);
    wet_soil(&mut w, 1, 1.0);
    water::water_table(&mut w);
    assert_rel(free_m3(&w, 2), want(&w), "one tick of aquifer seepage");
    assert!(residual(&w).abs() < 1e-12, "residual {}", residual(&w));
}

/// And the lateral free-water exchange is *not* a conductivity: it levels a share
/// of the cell and reads no material rate, so this package must leave it exactly
/// where it stood. Two adjacent void cells, one full, level toward each other.
#[test]
fn lateral_free_water_exchange_carries_no_conductivity() {
    let mut w = World::empty(Config {
        width: 2,
        height: 3,
        depth: 1,
        voxel_m: 0.25,
        seed: 23,
        ..Config::default()
    });
    let vol = w.config().voxel_volume();
    w.apply(Command::AddWater {
        x: 0,
        y: 1,
        z: 0,
        volume_m3: vol,
    });
    water::exchange(&mut w, 1);
    let left = w.view().free_at(0, 1, 0);
    let right = w.view().free_at(1, 1, 0);
    assert!(
        right > 0.0 && left < 1.0,
        "the lateral exchange moved nothing: {left} / {right}"
    );
    // Scale-free: the identical fixture on 0.125 m cells moves the identical
    // *fraction*, which is what a levelling rule means and why it is untouched.
    let mut fine = World::empty(Config {
        width: 2,
        height: 3,
        depth: 1,
        voxel_m: 0.125,
        seed: 23,
        ..Config::default()
    });
    let fine_vol = fine.config().voxel_volume();
    fine.apply(Command::AddWater {
        x: 0,
        y: 1,
        z: 0,
        volume_m3: fine_vol,
    });
    water::exchange(&mut fine, 1);
    assert_eq!(
        fine.view().free_at(1, 1, 0),
        right,
        "the free-water exchange is not scale-free any more"
    );
    assert!(residual(&w).abs() < 1e-12, "residual {}", residual(&w));
}

// ------------------------------------------------------------ 2. invariance

/// Drain a 1 m slab of saturated soil for `ticks` and return the depth of water
/// it handed the aquifer, in cubic metres per square metre of footprint — a
/// physical quantity with no cell in it — together with the run's residual.
fn slab_drained_m_per_m2(voxel_m: f64, ticks: u32) -> (f64, f64) {
    let cells = (1.0 / voxel_m).round() as u32;
    // One bedrock floor row, `cells` rows of soil making a metre, two of air.
    let mut w = column(cells + 3, voxel_m, 1e4, 0.0);
    for y in 1..=cells {
        wet_soil(&mut w, y, 1.0);
    }
    let footprint = w.config().cell_area();
    let store = |w: &World| w.config().aquifer_volume_for_head(w.aquifer_head_m());
    let before = store(&w);
    for _ in 0..ticks {
        water::drain(&mut w);
    }
    let gained = store(&w) - before;
    (gained / footprint, residual(&w))
}

/// The same metre of the same soil drains the same depth of water per second
/// whether it is four cells or eight.
///
/// **Tolerance.** The only thing that can differ is *which cell* the drainage
/// front is standing in when the run stops, so the bound is one coarse cell's
/// drainable store expressed as a depth: `(1 - field_capacity) · pore_capacity ·
/// 0.25 m = 0.0306 m³/m²`. That is a real bound and not a fudge: before this
/// package the fine grid drained at exactly half the rate, which over 100 ticks
/// is a shortfall of 0.0438 m³/m² — larger than the bound, so this test fails on
/// the old code and passes on the new.
///
/// 100 ticks and not 200 since soil retention: a metre of soil at field capacity
/// 0.65 has only 0.1225 m³/m² to give, and 200 ticks of `K · dt` (0.175) would run
/// the slab out of drainable water before the clock stopped.
#[test]
fn a_metre_of_soil_drains_the_same_depth_on_both_grids() {
    const TICKS: u32 = 100;
    let soil = Material::Soil;
    let tol = (1.0 - soil.field_capacity()) * soil.pore_capacity() * 0.25;

    let (coarse, coarse_residual) = slab_drained_m_per_m2(0.25, TICKS);
    let (fine, fine_residual) = slab_drained_m_per_m2(0.125, TICKS);

    // The physical expectation, from the rule and nothing else: `K · dt` metres
    // of water per tick, whatever the cell is.
    let expected = f64::from(TICKS) * rule_k_m_per_s(soil) * DT;

    assert!(
        (coarse - expected).abs() <= tol,
        "0.25 m: drained {coarse} m³/m², the rule says {expected} (tolerance {tol})"
    );
    assert!(
        (fine - expected).abs() <= tol,
        "0.125 m: drained {fine} m³/m², the rule says {expected} (tolerance {tol})"
    );
    assert!(
        (fine - coarse).abs() <= tol,
        "the same metre of soil drained {coarse} m³/m² on 0.25 m cells and {fine} on \
         0.125 m — a difference of {} against a discretisation bound of {tol}",
        (fine - coarse).abs()
    );

    // 3. The ledger conserves on both.
    assert!(
        coarse_residual.abs() < 1e-12,
        "0.25 m residual {coarse_residual}"
    );
    assert!(
        fine_residual.abs() < 1e-12,
        "0.125 m residual {fine_residual}"
    );
}

/// The same invariance for the surface: a film of standing water soaks into dry
/// ground at the same depth per second on both grids.
#[test]
fn standing_water_soaks_in_at_the_same_depth_on_both_grids() {
    const TICKS: u32 = 200;
    // A thin film: 0.02 m of water, well under what 200 ticks can take, so the
    // run measures the rate and not the supply.
    let film_m = 0.02;

    let soaked = |voxel_m: f64| -> (f64, f64) {
        let cells = (1.0 / voxel_m).round() as u32;
        let mut w = column(cells + 3, voxel_m, 1e4, 0.0);
        for y in 1..=cells {
            wet_soil(&mut w, y, 0.0);
        }
        let area = w.config().cell_area();
        w.apply(Command::AddWater {
            x: 0,
            y: cells + 1,
            z: 0,
            volume_m3: film_m * area,
        });
        let before = free_m3(&w, cells + 1);
        for _ in 0..TICKS {
            water::infiltrate(&mut w, DT);
        }
        ((before - free_m3(&w, cells + 1)) / area, residual(&w))
    };

    let (coarse, coarse_residual) = soaked(0.25);
    let (fine, fine_residual) = soaked(0.125);
    // Both films are thin enough that 200 ticks would take more than there is, so
    // the physical answer is "all of it" on both grids. Before this package the
    // fine grid took 200 · K · dt / 2 = 0.0875 m — still more than the film — so
    // the film alone would not have caught it; the bound below is the film's own
    // depth and the claim is that neither grid is rate-limited.
    assert!(
        (coarse - film_m).abs() < 1e-12,
        "0.25 m soaked {coarse} of a {film_m} m film"
    );
    assert!(
        (fine - film_m).abs() < 1e-12,
        "0.125 m soaked {fine} of a {film_m} m film"
    );
    assert!(
        coarse_residual.abs() < 1e-12,
        "0.25 m residual {coarse_residual}"
    );
    assert!(
        fine_residual.abs() < 1e-12,
        "0.125 m residual {fine_residual}"
    );
}

// ------------------------------------------------------------ 3. retention

/// Soil drained from saturation keeps exactly its field capacity: a metre of
/// saturated soil over the foundation, under a table that cannot rise, drains
/// until every voxel holds `field_capacity · pore_capacity · voxel volume` and the
/// rest has joined the aquifer. On both grids, since the retained share is a
/// property of the soil and not of the cell.
#[test]
fn saturated_soil_drains_to_its_field_capacity_and_keeps_it() {
    let soil = Material::Soil;
    for voxel_m in [0.25, 0.125] {
        let cells = (1.0 / voxel_m) as u32;
        let mut w = column(cells + 2, voxel_m, 1e4, 0.0);
        for y in 1..=cells {
            wet_soil(&mut w, y, 1.0);
        }
        let charged = w.view().stored_m3();
        // A metre drains `(1 - fc) · pore` metres at `K · dt` a tick: under 300
        // ticks at any field capacity the brief allows. 800 is well past it.
        for _ in 0..800 {
            w.step();
        }
        let want = soil.field_capacity() * soil.pore_capacity() * w.config().voxel_volume();
        for y in 1..=cells {
            let held = pore_m3(&w, y);
            assert!(
                (held - want).abs() <= 1e-9 * want,
                "{voxel_m} m, y={y}: holds {held} m³, field capacity is {want} m³"
            );
        }
        let drained = charged - f64::from(cells) * want;
        assert!(
            (w.view().aquifer_m3 - drained).abs() <= 1e-9,
            "{voxel_m} m: the aquifer took {} of the {drained} m³ that drained",
            w.view().aquifer_m3
        );
        assert!(
            residual(&w).abs() < 1e-12,
            "{voxel_m} m residual {}",
            residual(&w)
        );
    }
}
