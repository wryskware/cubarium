//! An independent pass over the plant boundary and the water table: tests written from
//! `design/handoffs/voxel-producers-briefs-2026-09-16.md` and
//! `design/voxel-ecology-sketch-2026-09-16.md`, not from the implementation, by a worker
//! that did not write it. Where `tests/core.rs` already pins a claim exactly as the
//! brief states it, nothing is repeated here; what is here is what the brief states and
//! that file pins more weakly, or not at all.
//!
//! Fixtures are `World::empty` plus `SetMaterial`, one slab deep and one metre per voxel
//! unless a test says otherwise, so a "unit" of water is a cubic metre and a full cell.
//! Wet soil is made exactly: free water into an air cell, then `SetMaterial` to soil,
//! which keeps the volume as pore water.

use cubarium_voxel::{Command, Config, Material, World};

/// A single-column world: `width = depth = 1`, so nothing spreads sideways and no
/// equalization can move water between columns. The aquifer's capacity per metre of head
/// is `width · depth · cell_area · aquifer_porosity`, which for one square metre of
/// footprint is `aquifer_porosity` itself — hence the enormous porosity in the water
/// table fixtures below, which is a fixture value and not a tuning: see `table_column`.
fn column_world(height: u32, porosity: f64, head_m: f64) -> World {
    World::empty(Config {
        width: 1,
        height,
        depth: 1,
        voxel_m: 1.0,
        seed: 17,
        aquifer_porosity: porosity,
        initial_aquifer_head_m: head_m,
        ..Config::default()
    })
}

/// One voxel of soil holding exactly `pore` of its capacity, by the brief's trick: the
/// water goes into the air cell first and the material change keeps it.
fn wet_soil(w: &mut World, x: i64, y: u32, z: u32, pore: f64) {
    let cap = Material::Soil.pore_capacity() * w.config().voxel_volume();
    if pore > 0.0 {
        let got = w.apply(Command::AddWater {
            x,
            y,
            z,
            volume_m3: pore * cap,
        });
        assert!(
            (got - pore * cap).abs() < 1e-12,
            "the void took {got} of {}",
            pore * cap
        );
    }
    w.apply(Command::SetMaterial {
        x,
        y,
        z,
        material: Material::Soil,
    });
    assert!(
        (w.view().pore_at(x, y, z) - pore).abs() < 1e-12,
        "{}",
        w.view().pore_at(x, y, z)
    );
    assert_eq!(
        w.view().free_at(x, y, z),
        0.0,
        "nothing may be left standing"
    );
}

fn residual(w: &World) -> f64 {
    w.view().stored_m3() - w.view().ledger.expected_stored()
}

fn run(w: &mut World, ticks: u32) {
    for _ in 0..ticks {
        w.step();
    }
}

// ============================================================ the water table
//
// `aquifer_porosity` is 1000 in these three fixtures against the default 0.35. It is not
// a tuning of anything: the claims under test are about *where the head is*, and a head
// that moves while the world drinks from it cannot pin a boundary. One square metre of
// footprint at porosity 1000 stores a thousand cubic metres per metre of head, so
// saturating a soil voxel (0.35) moves the table by a third of a millimetre and the
// boundary stays where the fixture put it. The named spring and the outlet are absent
// from a `World::empty` world, and rain and evaporation are off by default, so the
// aquifer and the voxels are the only two parties.

/// Bedrock floor, soil at `y = 1` and `y = 2`, air above, one column. The table is set so
/// that `y = 1` is under it (centre 1.5 m) and `y = 2` is above it (centre 2.5 m).
fn table_column(pore1: f64, pore2: f64, head_m: f64) -> World {
    let mut w = column_world(8, 1000.0, head_m);
    wet_soil(&mut w, 0, 1, 0, pore1);
    wet_soil(&mut w, 0, 2, 0, pore2);
    w
}

/// The head is a boundary, not a gradient: the voxel under it saturates and the
/// permeable voxel just above it is left exactly where it was, at field capacity.
#[test]
fn a_permeable_voxel_just_above_the_head_is_not_saturated() {
    // Head 1.9 m: `y = 1`'s centre (1.5) is under it, `y = 2`'s (2.5) is not.
    let mut w = table_column(0.0, Material::Soil.field_capacity(), 1.9);
    assert!(
        (w.aquifer_head_m() - 1.9).abs() < 1e-12,
        "head {}",
        w.aquifer_head_m()
    );
    run(&mut w, 200);

    let v = w.view();
    assert!(
        v.pore_at(0, 1, 0) > 0.999,
        "the voxel under the head is at {}",
        v.pore_at(0, 1, 0)
    );
    assert_eq!(
        v.pore_at(0, 2, 0),
        Material::Soil.field_capacity(),
        "the voxel above the head took water from the table"
    );
    // And the head is still where it was put, to a millimetre: the fixture's premise.
    assert!(
        (w.aquifer_head_m() - 1.9).abs() < 1e-3,
        "head {}",
        w.aquifer_head_m()
    );
    assert!(residual(&w).abs() < 1e-9, "{}", residual(&w));
}

/// Seepage fills the void over saturated ground to the table's own level inside that
/// cell and stops there — `(table - y) / voxel_m` of the cell, not a full cell and not a
/// drop more — and a cell already above the table takes nothing at all.
///
/// "At the head" is the head the step that moved the water **read**, and that is the
/// honest form of the claim rather than a weakening of it: the table is read once per
/// step and held fixed, so the water that rises into the cell during a step is water the
/// aquifer no longer has, and the head that rose it is already gone by the time the step
/// ends. The settled surface therefore sits above the *final* head by exactly the fall
/// its own last withdrawal caused — bounded by one transfer, `permeability · DT ·
/// pore_capacity · voxel_volume` over the aquifer's capacity per metre of head, 3.5e-6 m
/// in this fixture against a 2.3 m table, of which 1.9e-6 m is realized. Both halves are
/// asserted: per step the surface never passes the head that step read, and at rest it
/// is within one transfer of the head. The gap scales as `1 / aquifer_porosity`, so at
/// the default 0.35 it would be a few millimetres of a 0.25 m voxel.
#[test]
fn seepage_stops_exactly_at_the_head() {
    // Head 2.3 m. `y = 1` (soil) is under it; the void at `y = 2` is reached to 0.3 of
    // its own height.
    let mut w = table_column(0.0, 0.0, 2.3);
    // `y = 2` is soil in `table_column`; this fixture wants it void, so dig it back out.
    w.apply(Command::SetMaterial {
        x: 0,
        y: 2,
        z: 0,
        material: Material::Air,
    });
    for tick in 0..400 {
        let level = w.aquifer_head_m() - 2.0;
        let before = w.view().free_at(0, 2, 0);
        w.step();
        let free = w.view().free_at(0, 2, 0);
        assert!(
            free <= before.max(level),
            "tick {tick}: the pond rose from {before} to {free}, past the {level} the step read"
        );
    }

    // One transfer of the seeping material, expressed as metres of head.
    let soil = Material::Soil;
    let one_transfer_m = soil.permeability_per_s()
        * cubarium_voxel::DT
        * soil.pore_capacity()
        * w.config().voxel_volume()
        / (w.config().width as f64 * w.config().depth as f64 * w.config().cell_area() * 1000.0);
    let level = w.aquifer_head_m() - 2.0;
    let free = w.view().free_at(0, 2, 0);
    assert!(
        w.view().pore_at(0, 1, 0) > 0.999,
        "the ground under the pond is not saturated"
    );
    assert!(
        free - level <= one_transfer_m && free >= level,
        "the settled pond is at {free} against a table at {level}: not within the {one_transfer_m} \
         of head that one transfer costs"
    );
    assert_eq!(
        w.view().free_at(0, 3, 0),
        0.0,
        "and nothing above the table's own cell"
    );
    assert!(residual(&w).abs() < 1e-9, "{}", residual(&w));

    // A cell already holding more than the table's level takes nothing: not a drop, and
    // not a drop back either, since the saturated ground below has no room for it.
    let mut w = table_column(1.0, 0.0, 2.3);
    w.apply(Command::SetMaterial {
        x: 0,
        y: 2,
        z: 0,
        material: Material::Air,
    });
    w.apply(Command::AddWater {
        x: 0,
        y: 2,
        z: 0,
        volume_m3: 0.6,
    });
    assert_eq!(w.view().free_at(0, 2, 0), 0.6);
    run(&mut w, 400);
    assert_eq!(
        w.view().free_at(0, 2, 0),
        0.6,
        "a cell above the table was touched"
    );
    assert!(residual(&w).abs() < 1e-9, "{}", residual(&w));
}

/// Drainage is suspended under the table because the aquifer is what holds that water
/// up, and it resumes the moment the head falls below the voxel — nothing latches.
#[test]
fn drain_resumes_for_a_voxel_the_head_has_fallen_below() {
    // Head 2.9 m: both soil voxels (centres 1.5 and 2.5) are under it.
    let mut w = table_column(0.0, 0.0, 2.9);
    run(&mut w, 200);
    assert!(
        w.view().pore_at(0, 1, 0) > 0.999,
        "y=1 {}",
        w.view().pore_at(0, 1, 0)
    );
    assert!(
        w.view().pore_at(0, 2, 0) > 0.999,
        "y=2 {}",
        w.view().pore_at(0, 2, 0)
    );
    // Two hundred more ticks and the saturated zone has not drained a drop: the table
    // holds it up.
    let held = (w.view().pore_at(0, 1, 0), w.view().pore_at(0, 2, 0));
    run(&mut w, 200);
    assert_eq!(
        (w.view().pore_at(0, 1, 0), w.view().pore_at(0, 2, 0)),
        held,
        "it drained anyway"
    );

    // Pump the table down to 1.0 m, below both voxels' centres, and they drain to field
    // capacity like any unsupported ground.
    let want = w.config().aquifer_volume_for_head(1.0) - w.view().aquifer_m3;
    let moved = w.apply(Command::ChargeAquifer { volume_m3: want });
    assert!(
        (moved - want).abs() < 1e-9,
        "the withdrawal moved {moved} of {want}"
    );
    assert!(
        (w.aquifer_head_m() - 1.0).abs() < 1e-9,
        "head {}",
        w.aquifer_head_m()
    );
    run(&mut w, 600);

    let fc = Material::Soil.field_capacity();
    for y in [1u32, 2] {
        let pore = w.view().pore_at(0, y, 0);
        assert!(
            (pore - fc).abs() < 1e-6,
            "y={y} held {pore} after the table fell; field capacity is {fc}"
        );
    }
    assert!(residual(&w).abs() < 1e-9, "{}", residual(&w));
}

// ========================================================= sky visibility (e)
//
// The brief splits light in two: geometric sky visibility from the core, canopy
// attenuation owned by the plant layer. The partial-roof claim is the one the split
// rests on — a sheltered site must be neither dark nor open — and the plant-owned half
// is pinned from the flora crate, where a canopy exists to be seeded.

fn plain_slab(width: u32, height: u32, depth: u32) -> World {
    World::empty(Config {
        width,
        height,
        depth,
        voxel_m: 1.0,
        seed: 19,
        ..Config::default()
    })
}

/// Rock over every slab of the columns in `xs`, at `y`.
fn roof_over(w: &mut World, xs: std::ops::Range<i64>, y: u32) {
    for x in xs {
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
fn sky_visibility_under_a_partial_roof_is_between_open_and_roofed() {
    // Seven slabs deep so a 30-degree ray cannot leave through the front or back wall
    // before the roof one voxel up has stopped it, and sixteen columns wide so half a
    // roof is eight of them.
    let open = plain_slab(16, 6, 7).view().sky_visibility(4, 0, 3);
    assert_eq!(open, 1.0, "an open plain is open sky");

    let mut full = plain_slab(16, 6, 7);
    roof_over(&mut full, 0..16, 2);
    let roofed = full.view().sky_visibility(4, 0, 3);
    assert_eq!(roofed, 0.0, "a complete roof lets nothing out");

    // Half a roof: the columns from 4 to 11, so x = 4 is its edge. Rays toward -x leave
    // over open ground, the zenith and the +x fan do not.
    let mut half = plain_slab(16, 6, 7);
    roof_over(&mut half, 4..12, 2);
    let partial = half.view().sky_visibility(4, 0, 3);
    assert!(
        partial > roofed && partial < open,
        "under the edge of a roof: {partial}, between {roofed} and {open}"
    );
    // Strictly darker the further under the roof one stands, and dark at its middle.
    let deeper = half.view().sky_visibility(6, 0, 3);
    assert!(
        deeper < partial,
        "two columns further in is {deeper}, not darker than {partial}"
    );
    assert_eq!(
        half.view().sky_visibility(8, 0, 3),
        0.0,
        "the middle of the roof is roofed"
    );
    // The site out from under it is untouched: a roof shades what it covers.
    assert_eq!(
        half.view().sky_visibility(0, 0, 3),
        1.0,
        "open ground beside a roof"
    );
}

// ============================================================ the noise seed
//
// The instrument of the decisive experiment. The round-2 brief asked for identical
// outlet and spring cells under a noise reseed; that is false and was measured (seed 1:
// outlet (28, 9, 0) -> (12, 9, 0), spring (44, 17, 12) -> (28, 17, 12)). The outlet is an
// argmin over a basin floor the flattening rule makes nearly level, so a one-voxel wobble
// hands the title to another column, and the spring's x is derived from the outlet's.
// Nothing downstream of the wobble is invariant, only the RNG stream is. The two claims
// that are true are below: unmoved columns are identical, and the foundation rows are.

fn wobble_pair(seed: u64, noise_a: u64, noise_b: u64) -> (World, World) {
    let base = Config {
        seed,
        ..Config::default()
    };
    (
        World::new(Config {
            noise_seed: noise_a,
            ..base.clone()
        }),
        World::new(Config {
            noise_seed: noise_b,
            ..base
        }),
    )
}

/// The form that is true, checked over nine seed pairs on the world the experiment
/// actually runs on rather than one 64-wide fixture: a column whose own surface and
/// whose two neighbours' surfaces did not move is identical voxel for voxel, because
/// soil depth reads the neighbours and everything below it is drawn from the shared
/// stream.
///
/// Also checked with **both** noise seeds non-zero, which is the experiment's own form —
/// `noise_seed = 0` is the legacy path that draws the wobble from the main stream.
#[test]
fn noise_seed_leaves_every_unmoved_column_identical_in_every_pair() {
    let mut checked_total = 0u64;
    for seed in [1u64, 4, 9] {
        for (na, nb) in [(0u64, 99u64), (7, 99), (12345, 6789)] {
            let (a, b) = wobble_pair(seed, na, nb);
            let (va, vb) = (a.view(), b.view());
            let c = a.config().clone();
            let mut checked = 0u64;
            let mut moved = 0u64;
            for z in 0..c.depth {
                for x in 0..c.width as i64 {
                    if va.surface_y(x, z) != vb.surface_y(x, z) {
                        moved += 1;
                    }
                    if (-1..=1).any(|d| va.surface_y(x + d, z) != vb.surface_y(x + d, z)) {
                        continue;
                    }
                    checked += 1;
                    for y in 0..c.height {
                        assert_eq!(
                            va.material_at(x, y, z),
                            vb.material_at(x, y, z),
                            "seed {seed}, noise {na} vs {nb}: ({x}, {y}, {z}) moved under an \
                             unmoved surface and unmoved neighbours"
                        );
                    }
                }
            }
            let columns = (c.width * c.depth) as u64;
            assert!(
                moved > 0,
                "seed {seed}, noise {na} vs {nb}: the wobble did not move at all"
            );
            assert!(
                moved * 2 < columns,
                "seed {seed}, noise {na} vs {nb}: {moved} of {columns} surface cells moved, \
                 which is not a weak wobble"
            );
            assert!(
                checked * 4 > columns,
                "seed {seed}, noise {na} vs {nb}: only {checked} of {columns} columns held still"
            );
            checked_total += checked;
        }
    }
    assert!(checked_total > 0, "nothing was compared");
}

/// The half of the brief's claim that does hold, separated out and kept: the bedrock
/// *below* `y = 2` — the foundation row and the row above it — is identical under every
/// column whatever the noise seed does, because nothing in the wobble can reach it.
#[test]
fn noise_seed_leaves_the_foundation_rows_identical() {
    for seed in [1u64, 4, 9] {
        let (a, b) = wobble_pair(seed, 0, 99);
        let (va, vb) = (a.view(), b.view());
        for z in 0..a.config().depth {
            for x in 0..a.config().width as i64 {
                for y in 0..2 {
                    assert_eq!(
                        va.material_at(x, y, z),
                        vb.material_at(x, y, z),
                        "seed {seed}: ({x}, {y}, {z}) moved below y = 2"
                    );
                    assert_eq!(
                        va.material_at(x, y, z),
                        Material::Bedrock,
                        "({x}, {y}, {z})"
                    );
                }
            }
        }
    }
}
