//! Hot-path geometry, item 3 (`design/handoffs/voxel-hot-path-geometry-2026-09-24.md`):
//! the standable-face lookup.
//!
//! A support face is a solid voxel with void over it, inside the world
//! ([`VoxelView::is_support_direct`], the definition, read off `material`). The lookup
//! answers the same questions from a per-column bit mask the world keeps current. Every
//! test here compares the lookup with the direct test **for every cell** — and the column
//! queries with the walks they replace — after generation, after `SetMaterial` edits
//! (the one way a running world's terrain changes), after a snapshot round trip and after
//! a clone. Worlds are short, straddle one 64-row word, and go past the 128-row mask
//! (where the lookup is the direct test).

use cubarium_voxel::{Command, Config, Material, VoxelView, World};

/// The step rule's walk as `cubarium_voxel_fauna`'s `step_target_layer` wrote it before
/// the lookup: the support face nearest `s` within `[s - down, s + up]` (clipped to the
/// world), ties to the higher.
fn nearest_by_walk(v: &VoxelView<'_>, x: i64, z: u32, s: u32, down: u32, up: u32) -> Option<u32> {
    let lo = s.saturating_sub(down);
    let hi = (s + up).min(v.config.height.saturating_sub(1));
    let mut best: Option<u32> = None;
    for y in lo..=hi {
        if !v.is_support_direct(x, y, z) {
            continue;
        }
        let d = |a: u32| (i64::from(a) - i64::from(s)).abs();
        best = match best {
            None => Some(y),
            Some(b) if d(y) < d(b) || (d(y) == d(b) && y > b) => Some(y),
            keep => keep,
        };
    }
    best
}

/// The definition, spelled out once more from `material` so the direct test is checked
/// too.
fn support_by_hand(v: &VoxelView<'_>, x: i64, y: u32, z: u32) -> bool {
    let c = v.config;
    y + 1 < c.height
        && z < c.depth
        && v.material_at(x, y, z).is_solid()
        && !v.material_at(x, y + 1, z).is_solid()
}

/// Every cell: the lookup, the direct test and the hand-written definition agree, and so
/// does `is_solid` with `material`, on either side of the seam.
fn check_cells(world: &World, what: &str) {
    let v = world.view();
    let c = v.config;
    let w = i64::from(c.width);
    for z in 0..c.depth {
        for x in 0..w {
            for y in 0..c.height {
                let want = support_by_hand(&v, x, y, z);
                assert_eq!(v.is_support_direct(x, y, z), want, "{what}: direct ({x}, {y}, {z})");
                for xx in [x, x + w, x - w] {
                    assert_eq!(v.is_support(xx, y, z), want, "{what}: lookup ({xx}, {y}, {z})");
                    assert_eq!(
                        v.is_solid(xx, y, z),
                        v.material_at(x, y, z).is_solid(),
                        "{what}: solid ({xx}, {y}, {z})"
                    );
                }
            }
            let listed: Vec<u32> = (0..c.height).filter(|&y| support_by_hand(&v, x, y, z)).collect();
            assert_eq!(v.supports_in_column(x, z), listed, "{what}: column ({x}, {z})");
        }
    }
    // Off the strip's ends there is nothing to stand on.
    assert!(!v.is_support(0, 1, c.depth), "{what}");
    assert_eq!(v.lowest_support_in(0, c.depth, 0, c.height), None, "{what}");
    assert_eq!(v.highest_support_in(0, c.depth, 0, c.height), None, "{what}");
    assert_eq!(v.nearest_support(0, c.depth, 1, 2, 2), None, "{what}");
}

/// The column queries against the walks they replace, on every `every`-th column.
fn check_columns(world: &World, every: usize, what: &str) {
    let v = world.view();
    let c = v.config;
    let w = i64::from(c.width);
    let h = c.height;
    let columns: Vec<(i64, u32)> = (0..c.depth)
        .flat_map(|z| (0..w).map(move |x| (x, z)))
        .step_by(every.max(1))
        .collect();
    assert!(!columns.is_empty());
    for &(x, z) in &columns {
        for lo in 0..h {
            for hi in [lo, lo + 1, lo + 2, lo + 5, h - 1, h, h + 7] {
                let top = hi.min(h - 1);
                let lowest = (lo..=top).find(|&y| support_by_hand(&v, x, y, z));
                let highest = (lo..=top).rev().find(|&y| support_by_hand(&v, x, y, z));
                for xx in [x, x + w, x - w] {
                    assert_eq!(
                        v.lowest_support_in(xx, z, lo, hi),
                        lowest,
                        "{what}: lowest in ({xx}, {z}) [{lo}, {hi}]"
                    );
                    assert_eq!(
                        v.highest_support_in(xx, z, lo, hi),
                        highest,
                        "{what}: highest in ({xx}, {z}) [{lo}, {hi}]"
                    );
                }
            }
            // A range that is empty.
            if lo > 0 {
                assert_eq!(v.lowest_support_in(x, z, lo, lo - 1), None, "{what}");
                assert_eq!(v.highest_support_in(x, z, lo, lo - 1), None, "{what}");
            }
        }
        for s in 0..h + 2 {
            for (down, up) in [(0, 0), (1, 1), (2, 1), (1, 2), (3, 3), (0, 5), (6, 0), (70, 70)] {
                for xx in [x, x + w, x - w] {
                    assert_eq!(
                        v.nearest_support(xx, z, s, down, up),
                        nearest_by_walk(&v, x, z, s, down, up),
                        "{what}: nearest to {s} in ({xx}, {z}), down {down} up {up}"
                    );
                }
            }
        }
    }
}

fn set(w: &mut World, x: i64, y: u32, z: u32, material: Material) {
    w.apply(Command::SetMaterial { x, y, z, material });
}

/// A hand-built strip of `height` rows: a soil floor, a pillar, a roofed hollow, a shelf
/// across the seam, a skyline cell at the top row, and a stack alternating solid and air
/// around the 64- and 128-row word boundaries.
fn hand(height: u32) -> World {
    let mut w = World::empty(Config {
        width: 12,
        height,
        depth: 3,
        voxel_m: 0.25,
        seed: 11,
        ..Config::default()
    });
    for z in 0..3 {
        for x in 0..12 {
            for y in 1..=3 {
                set(&mut w, x, y, z, Material::Soil);
            }
        }
    }
    // A pillar.
    for y in 4..(height - 2).min(9) {
        set(&mut w, 5, y, 1, Material::Rock);
    }
    // A roof over a hollow at x 1..=3: the floor under it stays a support face.
    for x in 1..=3 {
        set(&mut w, x, 6, 0, Material::Rock);
    }
    // A shelf across the seam, x 11 and 0.
    set(&mut w, 11, 5, 2, Material::Rock);
    set(&mut w, 0, 5, 2, Material::Rock);
    // The skyline row: a solid at `height - 1` is not a support, the one under it is not
    // either (it has solid over it), and a solid at `height - 2` with air over it is.
    set(&mut w, 8, height - 1, 0, Material::Rock);
    set(&mut w, 8, height - 2, 0, Material::Rock);
    set(&mut w, 9, height - 2, 1, Material::Rock);
    // Alternate around the word boundaries.
    for y in [30, 32, 62, 63, 65, 66, 126, 127, 129] {
        if y + 1 < height {
            set(&mut w, 10, y, 2, Material::Rock);
        }
    }
    w
}

#[test]
fn the_lookup_is_the_direct_test_on_hand_built_strips_of_every_height() {
    for height in [12, 63, 64, 65, 72, 128, 129, 131] {
        let world = hand(height);
        let what = format!("hand-built, {height} rows");
        check_cells(&world, &what);
        check_columns(&world, if height > 72 { 5 } else { 1 }, &what);
    }
}

#[test]
fn the_lookup_is_the_direct_test_on_a_generated_world() {
    let world = World::new(Config::default());
    check_cells(&world, "generated");
    check_columns(&world, 37, "generated");
}

/// Terrain edits through `SetMaterial`: dig under the surface (a roof appears over a new
/// floor), fill a column to the skyline, open the top row, carve across the seam — the
/// lookup follows each one, cell for cell.
#[test]
fn the_lookup_follows_every_terrain_edit() {
    for height in [20, 72, 130] {
        let mut world = hand(height);
        let what = format!("{height} rows");
        let before = world.terrain_version();
        // Dig a tunnel under the soil: the soil over it is a roof, the floor a support.
        for x in 2..=7 {
            set(&mut world, x, 2, 1, Material::Air);
        }
        assert!(world.terrain_version() > before, "an edit moves the terrain version");
        check_cells(&world, &format!("{what}, tunnel"));
        // Fill one column to the top.
        for y in 4..height {
            set(&mut world, 4, y, 2, Material::Rock);
        }
        check_cells(&world, &format!("{what}, filled column"));
        // Open the top of it again, and the pillar's foot.
        set(&mut world, 4, height - 1, 2, Material::Air);
        set(&mut world, 5, 4, 1, Material::Air);
        check_cells(&world, &format!("{what}, opened"));
        // Across the seam, given as x outside the strip.
        set(&mut world, -1, 4, 0, Material::Rock);
        set(&mut world, 12, 4, 0, Material::Soil);
        set(&mut world, 12, 3, 0, Material::Air);
        check_cells(&world, &format!("{what}, seam"));
        // A no-op edit (same material) leaves everything as it was.
        set(&mut world, 0, 1, 0, Material::Soil);
        check_cells(&world, &format!("{what}, no-op"));
        check_columns(&world, 3, &what);
    }
}

/// The lookup is derived, not saved: a world read back from its snapshot, and a clone,
/// answer exactly as the world did.
#[test]
fn a_loaded_world_and_a_clone_rebuild_the_lookup() {
    let mut world = hand(72);
    for x in 3..=9 {
        set(&mut world, x, 2, 0, Material::Air);
    }
    let loaded = World::load(&world.save()).expect("round trip");
    check_cells(&loaded, "loaded");
    check_columns(&loaded, 2, "loaded");
    let mut clone = world.clone();
    check_cells(&clone, "clone");
    // An edit to the clone is the clone's alone.
    set(&mut clone, 6, 8, 1, Material::Rock);
    check_cells(&clone, "edited clone");
    check_cells(&world, "the original after its clone was edited");
    assert!(clone.view().is_support(6, 8, 1) && !world.view().is_support(6, 8, 1));
}

/// Water and plants do not enter the test: a support face is terrain alone, so a pool
/// over a face leaves it a support face, and the lookup does not move with water.
#[test]
fn water_does_not_move_the_lookup() {
    let mut world = hand(24);
    let cell = world.config().voxel_volume();
    for y in 4..8 {
        world.apply(Command::AddWater {
            x: 7,
            y,
            z: 0,
            volume_m3: cell,
        });
    }
    let version = world.terrain_version();
    for _ in 0..5 {
        world.step_with(1);
    }
    assert_eq!(world.terrain_version(), version, "water is not terrain");
    check_cells(&world, "wet");
    assert!(world.view().is_support(7, 3, 0), "a pooled floor is still a floor");
}
