//! The physical encounter query: what a body at a pose on a support face can stand on,
//! reach with its mouth and see.
//!
//! Read-only. Nothing here runs in a tick; every function is the *same rule* the live
//! path uses, exposed so a diagnostic can ask it of a face nobody is standing on and of
//! a body size nobody has yet. The contract these helpers serve is
//! `design/voxel-encounter-contract-2026-09-21.md`; the decisions they are measured
//! against are `design/handoffs/voxel-organism-decisions-2026-09-21.md`.
//!
//! Two things are deliberately *not* re-derived here:
//!
//! - **Mouth acceptance** is [`crate::body::foliage_stands_touching`], the scan behind
//!   the live [`crate::body::mouth_foliage_stand`]: a foliage-bearing stand whose one
//!   crown disc of cells sits at an accepted layer and covers a mouth column. Only the
//!   layer range is handed in, because the crown is one cell thick and any band over the
//!   standing surface therefore selects a contiguous run of layers.
//! - **Where a body may stand** is the predicate the seeder's `browser_faces` and the
//!   tick's `faces_in_column` both use: a support face, wadeable water, and
//!   [`Body::headroom_voxels`] voxels of void over it — the body's own height.
//!
//! A founder body changes its standing layer by at most its lineage's climb
//! ([`crate::climb_voxels`]; `design/handoffs/voxel-founder-step-2026-09-22.md`), so the
//! walkable neighbourhood of a founder is face-neighbouring support faces within that
//! many layers — which is what [`walkable_components`] computes, from the one shared
//! rule in `cubarium_voxel::walk`, for this crate, the seeder and the observer alike.

use std::ops::RangeInclusive;

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::{FloraView, Site, Stand};

use crate::body::{Body, has_headroom, mouth_columns, mouth_crown_layers};
use crate::senses::{self, ConeOccupancy};
use crate::{ConeHit, FaunaView, Pose};

/// How many headings a face is asked about when the question is "from *some* pose here".
///
/// The mouth region's furthest probe is `2 · footprint_radius + reach` from the body's
/// centre — under 0.2 m for either founder — so a five-degree turn moves it by about a
/// hundredth of a voxel. Seventy-two headings therefore enumerate the whole union of
/// mouth columns a body standing on one face could ever cover, and the acceptance rule
/// is a pure OR over headings, so the union may be tested in one call.
pub const HEADING_SAMPLES: usize = 72;

/// The standing surface of a support face, in metres: `Site.y` indexes the **solid**
/// support cell, so the surface a body stands on is its top
/// (`crates/cubarium-voxel/src/world.rs`, `is_support`; audit §1).
pub fn surface_m(standing_y: u32, voxel_m: f64) -> f64 {
    (f64::from(standing_y) + 1.0) * voxel_m
}

/// Every face a body of these dimensions could stand on, in `(y, x, z)` order.
///
/// The predicate is the seeder's and the tick's, not a new one: a support face, no more
/// than `wade_depth_m` of standing water on it, and [`Body::headroom_voxels`] voxels of
/// void over it (`crates/cubarium/src/voxel/habitat.rs`, `browser_faces`;
/// `crates/cubarium-voxel-fauna/src/step.rs`, `faces_in_column`).
pub fn standable_faces(view: &VoxelView<'_>, body: &Body, wade_depth_m: f64) -> Vec<Site> {
    let c = view.config;
    let room = body.headroom_voxels(c.voxel_m);
    let mut out = Vec::new();
    for y in 0..c.height {
        for x in 0..c.width {
            for z in 0..c.depth {
                let xi = i64::from(x);
                if !view.is_support(xi, y, z) {
                    continue;
                }
                if view.water_depth_m(xi, y, z) > wade_depth_m {
                    continue;
                }
                if !has_headroom(view, xi, y, z, room) {
                    continue;
                }
                out.push(Site { x, y, z });
            }
        }
    }
    out
}

/// The mouth columns of one pose: [`crate::body::mouth_columns`], exposed.
pub fn mouth_columns_at(view: &VoxelView<'_>, pose: &Pose, body: &Body) -> Vec<(i64, u32)> {
    mouth_columns(view, pose, body)
}

/// The union of mouth columns over every heading a body standing on `face` could take.
///
/// The body is placed where the model places one on a face, [`Pose::at_site`], and the
/// heading swept in [`HEADING_SAMPLES`] equal steps.
pub fn mouth_columns_from_face(
    view: &VoxelView<'_>,
    face: Site,
    body: &Body,
) -> Vec<(i64, u32)> {
    let v = view.config.voxel_m;
    let mut cols: Vec<(i64, u32)> = Vec::new();
    for k in 0..HEADING_SAMPLES {
        let heading = std::f64::consts::TAU * (k as f64) / (HEADING_SAMPLES as f64);
        let mut pose = Pose::at_site(face, v);
        pose.heading_rad = heading;
        for entry in mouth_columns(view, &pose, body) {
            if !cols.contains(&entry) {
                cols.push(entry);
            }
        }
    }
    cols
}

/// The crown layers the implemented mouth accepts from `standing_y`:
/// [`crate::body::mouth_crown_layers`], exposed. Since 2026-09-22 this **is** the metre
/// band of [`band_crown_layers`], so the two no longer name different arms.
pub fn mouth_crown_layers_at(standing_y: u32, body: &Body, voxel_m: f64) -> RangeInclusive<i64> {
    mouth_crown_layers(standing_y, body, voxel_m)
}

/// The crown layers a **metre band** `[0, ceiling_m]` over the standing surface reaches.
///
/// A crown is one cell thick at layer `L`, so its physical slab is
/// `[L·v, (L+1)·v]`; the band selects every `L` whose slab overlaps it by a positive
/// amount. A touch at exactly the ceiling is not an overlap (audit §2: "a boundary touch
/// alone contributes no stock"). A band that reaches nothing comes back empty.
pub fn band_crown_layers(standing_y: u32, voxel_m: f64, ceiling_m: f64) -> RangeInclusive<i64> {
    let lo = i64::from(standing_y) + 1;
    let empty = lo..=(lo - 1);
    let q = ceiling_m / voxel_m;
    if !q.is_finite() || q <= 0.0 {
        return empty;
    }
    let hi = lo + (q.ceil() as i64) - 1;
    if hi < lo { empty } else { lo..=hi }
}

/// Every foliage-bearing stand whose crown cells sit at a layer in `layers` and cover one
/// of `cols` — the live mouth's own scan, with the layer range handed in.
pub fn foliage_stands_in_layers(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    cols: &[(i64, u32)],
    layers: &RangeInclusive<i64>,
) -> Vec<(Site, f64)> {
    crate::body::foliage_stands_touching(fv, view, cols, layers)
}

/// What a mouth over `cols` whose band selects `layers` can take from one stand,
/// **per foliage layer**: `(index among the stand's foliage layers, stock)`, bottom-up.
///
/// This is what closes package 0's "could not measure 1" (within-stand shares): an
/// observer can now say that a browser reaches an adult bloomcrown's rosette and not
/// its crown, rather than that it reaches the stand or does not
/// (`design/handoffs/voxel-edible-stock-2026-09-21.md`, integration note).
pub fn reachable_layers_of(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    stand: &Stand,
    cols: &[(i64, u32)],
    layers: &RangeInclusive<i64>,
) -> Vec<(usize, f64)> {
    crate::body::reachable_layers(fv, view, stand, cols, layers)
}

/// The `(x, z)` columns one **layer** covers, by the same disc rule the mouth and the
/// cone both use — its own radius, which is a fraction of the crown's.
pub fn layer_columns(
    view: &VoxelView<'_>,
    stand: &Stand,
    layer: &cubarium_voxel_flora::StandLayer,
) -> Vec<(u32, u32)> {
    let c = view.config;
    let span = layer.radius_v.floor() as i64;
    let r2 = layer.radius_v * layer.radius_v;
    let mut out = Vec::new();
    for dz in -span..=span {
        for dx in -span..=span {
            if (dx * dx + dz * dz) as f64 > r2 {
                continue;
            }
            let z = i64::from(stand.site.z) + dz;
            if z < 0 || z >= i64::from(c.depth) {
                continue;
            }
            let x = (i64::from(stand.site.x) + dx).rem_euclid(i64::from(c.width));
            out.push((x as u32, z as u32));
        }
    }
    out
}

/// The layer a stand's crown disc of cells sits at: `site.y + crown_voxels(wood)`.
pub fn crown_layer(fv: &FloraView<'_>, stand: &Stand) -> i64 {
    i64::from(stand.site.y) + i64::from(fv.config.species(stand.species).crown_voxels(stand.wood))
}

/// The stand's foliage slab in **metres above the world floor**: the one cell-thick disc
/// the model actually holds, `[L·v, (L+1)·v]`.
pub fn crown_slab_m(fv: &FloraView<'_>, stand: &Stand, voxel_m: f64) -> (f64, f64) {
    let l = crown_layer(fv, stand) as f64;
    (l * voxel_m, (l + 1.0) * voxel_m)
}

/// The `(x, z)` columns a stand's crown disc covers, by the same disc rule the mouth and
/// the cone both use.
pub fn crown_columns(fv: &FloraView<'_>, view: &VoxelView<'_>, stand: &Stand) -> Vec<(u32, u32)> {
    let c = view.config;
    let sc = fv.config.species(stand.species);
    let radius = sc.crown_radius(stand.wood).max(0.0);
    let span = radius.floor() as i64;
    let r2 = radius * radius;
    let mut out = Vec::new();
    for dz in -span..=span {
        for dx in -span..=span {
            if (dx * dx + dz * dz) as f64 > r2 {
                continue;
            }
            let z = i64::from(stand.site.z) + dz;
            if z < 0 || z >= i64::from(c.depth) {
                continue;
            }
            let x = (i64::from(stand.site.x) + dx).rem_euclid(i64::from(c.width));
            out.push((x as u32, z as u32));
        }
    }
    out
}

/// Where the eye is, in metres: the body's column and `0.8 × height` over the standing
/// surface ([`crate::senses::cone_origin`], exposed).
pub fn eye_origin_m(
    view: &VoxelView<'_>,
    pose: &Pose,
    standing_y: u32,
    body: &Body,
) -> (f64, f64, f64) {
    senses::cone_origin(view, pose, standing_y, body)
}

/// An eye a stated height in **metres above the standing surface**, for a diagnostic
/// measuring an arm the model does not run. The live eye is [`eye_origin_m`].
pub fn eye_above_surface_m(
    pose: &Pose,
    standing_y: u32,
    voxel_m: f64,
    height_m: f64,
) -> (f64, f64, f64) {
    (pose.x, surface_m(standing_y, voxel_m) + height_m, pose.z)
}

/// One ray's unit direction from a heading, a yaw and a pitch in degrees
/// ([`crate::senses::ray_direction`], exposed).
pub fn ray_direction_deg(heading_rad: f64, yaw_deg: f64, pitch_deg: f64) -> (f64, f64, f64) {
    senses::ray_direction(heading_rad, yaw_deg, 0.0, pitch_deg)
}

/// The occlusion map a cone marches against, built once and marched many times.
pub struct SightMap {
    occupancy: ConeOccupancy,
}

impl SightMap {
    /// Build it. `pools_occlude` is `true` for today's rule — a surface pool blocks the
    /// whole cell over its face — and `false` for the decided arm, where a ground pool is
    /// not a wall (decisions §6).
    pub fn new(
        view: &VoxelView<'_>,
        fv: &FloraView<'_>,
        av: &FaunaView<'_>,
        pools_occlude: bool,
    ) -> SightMap {
        SightMap {
            occupancy: senses::cone_occupancy_with(view, fv, av, pools_occlude),
        }
    }

    /// March one ray and name its first hit, with the cell it struck. The march, the
    /// sub-step, the step cap and the order of the occlusion tests are the live cone's.
    pub fn first_hit(
        &self,
        view: &VoxelView<'_>,
        observer_id: u64,
        origin_m: (f64, f64, f64),
        dir: (f64, f64, f64),
        range_m: f64,
    ) -> Option<(f64, ConeHit, usize)> {
        senses::ray_first_hit_cell(view, &self.occupancy, observer_id, origin_m, dir, range_m)
            .map(|(distance, fine, cell)| (distance, crate::cone_hit_of(fine), cell))
    }
}

/// The connected components of a set of standing faces under the **founder step rule**.
///
/// One function, one rule: `cubarium_voxel::walk::components`, with `climb` the
/// lineage's climb height in whole voxels ([`crate::climb_voxels`]). Face-neighbouring
/// columns join when their standing layers differ by at most that, `x` wrapping and the
/// strip's `z` ends walls. The observer (`voxel_edible_stock`), the seeder's
/// feeding-face adjacency (`crates/cubarium/src/voxel/habitat.rs`, `browser_faces`) and
/// the generator's ring gate all go through it.
///
/// Returns a component id per face, in the order of `faces`.
pub fn walkable_components(faces: &[Site], width: u32, climb: u32) -> Vec<usize> {
    let cells: Vec<(u32, u32, u32)> = faces.iter().map(|f| (f.x, f.y, f.z)).collect();
    cubarium_voxel::walk::components(&cells, width, climb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::{mouth_foliage_stand, mouth_foliage_stands};
    use crate::manifest::Founder;
    use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
    use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species as Plant};

    /// The same flat fixture `body`'s own tests use: 8 × 6 × 6 at 0.25 m, soil in
    /// 1..=2, so the ground is a support face at y = 2.
    fn flat_world() -> World {
        let mut world = World::empty(VoxelConfig {
            width: 8,
            height: 6,
            depth: 6,
            voxel_m: 0.25,
            ..VoxelConfig::default()
        });
        for z in 0..6 {
            for x in 0..8 {
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

    /// The extracted scan **is** the live mouth: over every face and heading of a small
    /// planted world, the best stand `foliage_stands_in_layers` offers at the mouth's own
    /// layer range is exactly the stand `mouth_foliage_stand` picks, tie-break included.
    #[test]
    fn the_shared_scan_is_the_live_mouths_own_choice() {
        let world = flat_world();
        let view = world.view();
        let mut flora = Flora::new(FloraConfig::default());
        for (x, z, species, wood) in [
            (2i64, 2u32, Plant::Bloomcrown, 0.30),
            (5, 2, Plant::Bloomcrown, 0.12),
            (3, 3, Plant::Springturf, 0.05),
            (6, 4, Plant::Velvetpad, 0.05),
        ] {
            assert!(flora.apply(
                &world,
                FloraCommand::Seed {
                    x,
                    z,
                    species,
                    wood,
                },
            ));
        }
        let fv = flora.view();
        let body = crate::FounderPhysiology::frozen(Founder::Browser).adult_body();
        let layers = mouth_crown_layers_at(2, &body, view.config.voxel_m);
        let mut compared = 0usize;
        let mut hits = 0usize;
        for xi in 0..8 {
            for zi in 0..6 {
                for k in 0..HEADING_SAMPLES {
                    let heading = std::f64::consts::TAU * (k as f64) / (HEADING_SAMPLES as f64);
                    let pose = Pose {
                        x: (f64::from(xi) + 0.5) * 0.25,
                        z: (f64::from(zi) + 0.5) * 0.25,
                        heading_rad: heading,
                    };
                    let cols = mouth_columns_at(&view, &pose, &body);
                    let live = mouth_foliage_stand(&fv, &view, &cols, 2, &body);
                    let shared = foliage_stands_in_layers(&fv, &view, &cols, &layers);
                    assert_eq!(
                        shared,
                        mouth_foliage_stands(&fv, &view, &cols, 2, &body),
                        "the diagnostic list is the shared scan"
                    );
                    let best =
                        shared
                            .iter()
                            .copied()
                            .fold(None, |best, (site, foliage)| match best {
                                None => Some((site, foliage)),
                                Some((bs, bf)) => {
                                    if foliage > bf || (foliage == bf && site < bs) {
                                        Some((site, foliage))
                                    } else {
                                        Some((bs, bf))
                                    }
                                }
                            });
                    assert_eq!(live, best, "at {pose:?}");
                    compared += 1;
                    hits += usize::from(live.is_some());
                }
            }
        }
        assert_eq!(compared, 8 * 6 * HEADING_SAMPLES);
        assert!(hits > 0, "the fixture has to put some crown in a mouth");
    }

    /// The band is a layer range, and it is the range the arithmetic in the contract
    /// says: two layers on a 0.125 m grid at a 0.25 m ceiling, one on a 0.25 m grid, and
    /// nothing at all for a ceiling of zero.
    #[test]
    fn a_metre_band_selects_the_layers_whose_slabs_it_overlaps() {
        assert_eq!(band_crown_layers(4, 0.125, 0.25), 5..=6);
        assert_eq!(band_crown_layers(4, 0.25, 0.25), 5..=5);
        assert_eq!(band_crown_layers(4, 0.25, 0.30), 5..=6);
        assert!(band_crown_layers(4, 0.25, 0.0).is_empty());
    }

    /// Two faces a column apart on one terrace are one component; a face a layer up is
    /// its own when the lineage cannot climb, and joins when it can.
    #[test]
    fn a_level_step_joins_and_a_rise_needs_the_climb() {
        let faces = [
            Site { x: 0, y: 3, z: 1 },
            Site { x: 1, y: 3, z: 1 },
            Site { x: 2, y: 4, z: 1 },
            Site { x: 7, y: 3, z: 1 },
        ];
        let comp = walkable_components(&faces, 8, 0);
        assert_eq!(comp[0], comp[1], "four-adjacent at one height");
        assert_ne!(comp[1], comp[2], "a rise is not a step without a climb");
        assert_eq!(comp[0], comp[3], "x wraps");
        let comp = walkable_components(&faces, 8, 1);
        assert_eq!(comp[1], comp[2], "one voxel of climb makes the rise a step");
    }

    /// Two terraces a voxel apart are one walkable component under the step rule and
    /// two without it: the observer's route question and the seeder's are the same
    /// question (`design/handoffs/voxel-founder-step-2026-09-22.md`, deliverable 2).
    #[test]
    fn the_step_rule_joins_two_terraces_and_a_climb_of_zero_does_not() {
        // A low shelf at y = 3 in columns 0..4 and a high one at y = 4 in 4..8, on one
        // row; the only way across is the riser between x = 3 and x = 4.
        let mut faces: Vec<Site> = Vec::new();
        for x in 0..4u32 {
            faces.push(Site { x, y: 3, z: 1 });
        }
        for x in 4..8u32 {
            faces.push(Site { x, y: 4, z: 1 });
        }
        let level = walkable_components(&faces, 8, 0);
        assert_eq!(
            level.iter().collect::<std::collections::HashSet<_>>().len(),
            2,
            "level adjacency leaves the two terraces apart"
        );
        let stepped = walkable_components(&faces, 8, 1);
        assert_eq!(
            stepped
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            1,
            "one voxel of climb makes the riser a route"
        );
        // Two voxels apart is still two components for a one-voxel climb.
        let mut apart = faces.clone();
        for f in apart.iter_mut().filter(|f| f.y == 4) {
            f.y = 5;
        }
        assert_eq!(
            walkable_components(&apart, 8, 1)
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            2,
            "a two-voxel riser is not a step for a one-voxel climb"
        );
    }
}
