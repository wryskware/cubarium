//! The hand-authored test scene: `--scene authored`.
//!
//! `cubarium-voxel`'s generator is Package A's work. Until it lands — and afterwards, as
//! a fixture that does not move when the generator is retuned — this builds a strip out
//! of [`World::empty`] plus `SetMaterial` and `AddWater`, with exactly the features the
//! presenter has to get right:
//!
//! - a **broad ridge centred on x = 0**, so a landform straddles the seam instead of
//!   stopping at it, plus a rock cap spanning `x = −3..=3` so the seam carries an edge;
//! - a **hollow with standing water** at the far side of the strip;
//! - a **covered passage** carved into the ridge's flank, open at the front, and a
//!   **jutting shelf** past its mouth with air beneath it: the overhang silhouette;
//! - a **terrace** of quantised steps, which is what the autotiled chamfer is for.
//!
//! Every height comes from a periodic function of `x`, so the strip is continuous across
//! the seam by construction rather than by a fix-up. Nothing here simulates: the world's
//! `step` never runs during authoring.

use std::f64::consts::TAU;

use cubarium_voxel::{Command, Config, Material, World};

/// Soil thickness on top of the rock, in voxels.
const SOIL_DEPTH: u32 = 2;
/// Bedrock fills everything below this.
const BEDROCK_TOP: u32 = 2;
/// Terrace steps, in voxels.
const TERRACE_STEP: f64 = 3.0;
/// How far the ground rises through the middle of the habitat, in voxels.
const BACK_RELIEF: f64 = 2.0;

/// The number of the highest solid voxel in column `(x, z)` of the base landform.
///
/// Periodic in `x` with period `config.width`, so `surface(-1) == surface(width - 1)`
/// and the seam is not a special case.
fn surface(config: &Config, x: i64, z: u32) -> u32 {
    let h = f64::from(config.height);
    let w = i64::from(config.width);
    let xm = x.rem_euclid(w);
    let u = xm as f64 / w as f64 * TAU;

    // One broad ridge with its peak at x = 0, and a shallow basin opposite it.
    let ridge = (0.5 + 0.5 * u.cos()).powf(1.6) * 0.38 * h;
    let basin = (0.5 - 0.5 * u.cos()) * 0.10 * h;
    let ripple = 0.035 * h * (3.0 * u).cos();
    // A gentle rise through the middle of the habitat, so depth reads as depth rather
    // than as a flat extrusion of the front column. Deliberately small: every voxel the
    // terrain climbs going *back* turns one slice boundary into a visible riser, and a
    // steady ramp over sixteen slices makes the whole strip corduroy.
    let back = BACK_RELIEF
        * (std::f64::consts::PI * f64::from(z) / f64::from(config.depth.max(2) - 1)).sin();

    let mut y = 0.26 * h + ridge - basin + ripple + back;

    // The terrace: one band of the strip quantised into steps.
    let (t0, t1) = terrace_band(config);
    if (t0..t1).contains(&xm) {
        y = (y / TERRACE_STEP).floor() * TERRACE_STEP + TERRACE_STEP - 1.0;
    }

    y.round().clamp(f64::from(BEDROCK_TOP) + 1.0, h - 4.0) as u32
}

/// The terraced band, on the descending flank between the ridge and the hollow.
fn terrace_band(config: &Config) -> (i64, i64) {
    let w = i64::from(config.width);
    let start = w * 5 / 16;
    (start, start + (w / 8).max(3))
}

/// The covered passage's mouth: `(first x, one past the last x)`.
fn passage_band(config: &Config) -> (i64, i64) {
    let w = i64::from(config.width);
    let start = w / 16;
    (start, start + (w / 12).max(4))
}

/// Build the authored strip. Deterministic; `config.seed` is not consulted.
pub fn authored(config: Config) -> World {
    let mut world = World::empty(config.clone());
    let c = &config;
    let w = i64::from(c.width);
    let front_half = (c.depth / 2).max(1);

    // 1. Strata: bedrock, rock, and a soil skin that follows the landform.
    for z in 0..c.depth {
        for x in 0..w {
            let top = surface(c, x, z);
            for y in 0..=top {
                let m = if y < BEDROCK_TOP {
                    Material::Bedrock
                } else if y + SOIL_DEPTH <= top {
                    Material::Rock
                } else {
                    Material::Soil
                };
                world.apply(Command::SetMaterial {
                    x,
                    y,
                    z,
                    material: m,
                });
            }
        }
    }

    // 2. A rock cap straddling x = 0, so the seam carries a hard edge and not only a
    //    smooth slope.
    for x in -3i64..=3 {
        for z in 0..c.depth {
            let top = surface(c, x, z);
            if top + 1 < c.height {
                world.apply(Command::SetMaterial {
                    x,
                    y: top + 1,
                    z,
                    material: Material::Rock,
                });
            }
        }
    }

    // 3. A covered passage carved into the ridge's flank, open at the front so the
    //    recess is visible: three voxels of headroom under a three-voxel roof.
    let (p0, p1) = passage_band(c);
    for x in p0..p1 {
        for z in 0..front_half {
            let roof = surface(c, x, z).saturating_sub(3);
            let floor = roof.saturating_sub(3).max(BEDROCK_TOP + 1);
            for y in floor..roof {
                world.apply(Command::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Air,
                });
            }
        }
    }

    // 4. A shelf jutting past the passage's mouth, out over the descending slope: solid
    //    with air underneath, which is the overhang silhouette against the sky.
    let sx = p1;
    let shelf_y = surface(c, sx, 0);
    for dx in 0..6i64 {
        for z in 0..front_half {
            if shelf_y + 1 < c.height {
                world.apply(Command::SetMaterial {
                    x: sx + dx,
                    y: shelf_y,
                    z,
                    material: Material::Rock,
                });
                world.apply(Command::SetMaterial {
                    x: sx + dx,
                    y: shelf_y + 1,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }

    // 5. A hollow bowl opposite the ridge, then standing water in it.
    let cx = w / 2;
    let r = (w / 10).max(4);
    let bowl_depth = 8.0;
    for dx in -r..=r {
        let x = cx + dx;
        let cut = ((1.0 - (dx as f64 / r as f64).powi(2)) * bowl_depth).round() as u32;
        if cut == 0 {
            continue;
        }
        for z in 0..c.depth {
            let top = surface(c, x, z);
            let floor = top.saturating_sub(cut).max(BEDROCK_TOP);
            for y in (floor + 1)..=top {
                world.apply(Command::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Air,
                });
            }
        }
    }

    // Fill to a level a few voxels above the bowl's deepest point, so the pool has a
    // shoreline on both flanks rather than brimming over.
    let level = {
        let floor = world.view().surface_y(cx, 0).unwrap_or(BEDROCK_TOP);
        (floor + 5).min(c.height - 1)
    };
    let volume = c.voxel_volume();
    for dx in -r..=r {
        let x = cx + dx;
        for z in 0..c.depth {
            let top = world.view().surface_y(x, z).unwrap_or(0);
            for y in (top + 1)..=level {
                world.apply(Command::AddWater {
                    x,
                    y,
                    z,
                    volume_m3: volume,
                });
            }
        }
    }

    // 6. Moisture. The bowl is a pond and the rest of the strip was bone dry, which is a
    //    landscape nothing can establish on: the flora layer's own establishment gates
    //    want pore water in the root box, and a fresh `World::empty` has none anywhere.
    //    The staged rings get this from their recipe's water inventory
    //    (`cubarium_voxel::hydrate`); this hand-built fixture is charged with the same
    //    default so the authored scene is a habitat and not a desert with a puddle in it.
    //    Booked through the ledger like any other addition.
    cubarium_voxel::hydrate(&mut world, &cubarium_voxel::Water::AUTHORED);

    world
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The landform is periodic in `x` by construction, at every depth.
    #[test]
    fn the_landform_is_periodic_across_the_seam() {
        let c = Config {
            width: 48,
            height: 20,
            depth: 6,
            ..Config::default()
        };
        for z in 0..c.depth {
            for x in -8i64..8 {
                assert_eq!(
                    surface(&c, x, z),
                    surface(&c, x + i64::from(c.width), z),
                    "x = {x}, z = {z}"
                );
            }
        }
    }

    /// The authored world fits its config: nothing is written out of range and the
    /// bedrock floor survives.
    #[test]
    fn the_authored_world_stays_inside_its_config() {
        let c = Config {
            width: 40,
            height: 16,
            depth: 4,
            ..Config::default()
        };
        let world = authored(c.clone());
        let view = world.view();
        assert_eq!(view.material.len(), c.cells());
        for z in 0..c.depth {
            for x in 0..i64::from(c.width) {
                assert_eq!(
                    view.material_at(x, 0, z),
                    Material::Bedrock,
                    "the floor at {x},{z}"
                );
                assert_eq!(
                    view.material_at(x, c.height - 1, z),
                    Material::Air,
                    "the sky at {x},{z}"
                );
            }
        }
    }
}
