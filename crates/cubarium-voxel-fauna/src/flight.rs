//! **Flight** (F1, `design/predation-and-flight-2026-09-28.md` §3; Brief F1 in
//! `design/handoffs/voxel-flight-2026-09-28.md`): the geometry a flyer moves, perches and
//! senses with. The tick's rules (costs, takeoffs, sips) are in `step.rs`; this module
//! answers the questions they ask of the world.
//!
//! - **The body** is a vertical disc of radius `span / 2`, standing its current height
//!   tall on its base `y_m`. Air is any cell not solid and not
//!   under a column's standing water; the disc never overlaps a solid cell, and its base
//!   never goes under the water's surface in any column it covers.
//! - **The ceiling** is `band_max_m` over the top of the support face directly below the
//!   body's centre column (its `site`).
//! - **Crowns are passable**: inside a crown's foliage layer (the centre's cell) the speed
//!   cap is `foliage_speed_factor` of the open air's. A flyer descending onto a leafy
//!   layer's top at its centre column lands on it (`CrownTop`); one rising into a layer's
//!   underside hangs there (`Underside`), so it never climbs up through a crown.
//! - **Perches** (§3.4): a support face's top, a crown layer's top, a latticevine-covered
//!   face it presses against while descending, and an underside (a crown layer's or a
//!   solid overhang's) it rises into.

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::{Face as VineFace, FaceDir, FloraView, Site, SpurPhase, StandLayer};

use crate::{Animal, Bloom, ConeHit, Flight, FounderPhysiology, MobilityMode, Perch, PerchAnchor, PerchKind, surface_m};

/// Float slack on heights and contacts, metres.
pub(crate) const EPS: f64 = 1e-9;
/// How near a flyer's disc must come to a wall to cling to it, metres.
pub(crate) const CLING_M: f64 = 0.01;
/// How far a perch's height may drift from the body's before the perch is gone, metres
/// (a crown that grew a stage, a face that changed): a millimetre.
const PERCH_SLACK_M: f64 = 1e-3;

/// A flyer's body for collision: the disc's radius and the body's height, metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Shape {
    pub r: f64,
    pub h: f64,
}

impl Shape {
    /// The shape of a body holding `body` of structure: the span's disc (the table's, at
    /// every size: the collision sphere is `span_m / 2`) and the body's current height.
    pub(crate) fn of(phys: &FounderPhysiology, flight: &Flight, body: f64) -> Shape {
        Shape {
            r: 0.5 * flight.span_m,
            h: phys.body_at(body).height_m.max(1e-6),
        }
    }
}

fn wrap(x: i64, width: u32) -> i64 {
    x.rem_euclid(i64::from(width))
}

/// The columns (unwrapped `x`, `z`) a disc at `(x, z)` of radius `r` overlaps strictly, or
/// `None` when it leaves the strip in `z` (the edge is a wall).
fn disc_columns(view: &VoxelView<'_>, x: f64, z: f64, r: f64) -> Option<Vec<(i64, u32)>> {
    let c = view.config;
    let v = c.voxel_m;
    if z - r < -EPS || z + r > f64::from(c.depth) * v + EPS {
        return None;
    }
    let (x0, x1) = (((x - r) / v).floor() as i64, ((x + r) / v).floor() as i64);
    let (z0, z1) = (((z - r) / v).floor() as i64, ((z + r) / v).floor() as i64);
    let mut out = Vec::with_capacity(4);
    for cx in x0..=x1 {
        for cz in z0..=z1 {
            if cz < 0 || cz >= i64::from(c.depth) {
                continue;
            }
            let qx = x.clamp(cx as f64 * v, (cx as f64 + 1.0) * v);
            let qz = z.clamp(cz as f64 * v, (cz as f64 + 1.0) * v);
            let (dx, dz) = (x - qx, z - qz);
            if dx * dx + dz * dz < r * r || (cx as f64 * v <= x && x < (cx as f64 + 1.0) * v && cz as f64 * v <= z && z < (cz as f64 + 1.0) * v) {
                out.push((cx, cz as u32));
            }
        }
    }
    Some(out)
}

/// The row a height falls in, rounding a height on a cell boundary up into the cell above.
pub(crate) fn row_at(y: f64, v: f64) -> i64 {
    ((y + EPS) / v).floor() as i64
}

/// The support face directly below a point: the highest support face of column `(x, z)`
/// whose top is at or under `y_m`.
pub(crate) fn support_below(view: &VoxelView<'_>, x: f64, z: f64, y_m: f64) -> Option<Site> {
    let c = view.config;
    let v = c.voxel_m;
    let cz = (z / v).floor();
    if !(0.0..f64::from(c.depth)).contains(&cz) {
        return None;
    }
    let cx = wrap((x / v).floor() as i64, c.width);
    let top_row = row_at(y_m, v) - 1;
    if top_row < 0 {
        return None;
    }
    let y = view.highest_support_in(cx, cz as u32, 0, top_row.min(i64::from(u32::MAX)) as u32)?;
    Some(Site {
        x: cx as u32,
        y,
        z: cz as u32,
    })
}

/// The top of the solid (and the standing water on it) under `base` in one column:
/// `(top, the support face, the depth of the water on it)`.
fn column_floor(view: &VoxelView<'_>, cx: i64, cz: u32, base: f64) -> Option<(f64, Site, f64)> {
    let c = view.config;
    let v = c.voxel_m;
    let top_row = row_at(base, v) - 1;
    if top_row < 0 {
        return None;
    }
    let wx = wrap(cx, c.width);
    let y = view.highest_support_in(wx, cz, 0, top_row as u32)?;
    let solid = surface_m(y, v);
    let water = view.standing_depth_m(wx, y, cz);
    Some((
        solid + water,
        Site {
            x: wx as u32,
            y,
            z: cz,
        },
        water,
    ))
}

/// Whether a body of `shape` with its base at `base` and its centre at `(x, z)` overlaps a
/// solid cell, stands under the standing water of a column it covers, leaves the strip,
/// or reaches out of the world.
pub(crate) fn blocked(view: &VoxelView<'_>, shape: Shape, x: f64, z: f64, base: f64) -> bool {
    let c = view.config;
    let v = c.voxel_m;
    if !(base >= 0.0) || !x.is_finite() || !z.is_finite() {
        return true;
    }
    let Some(cols) = disc_columns(view, x, z, shape.r) else {
        return true;
    };
    let lo = row_at(base, v);
    let hi = (((base + shape.h - EPS) / v).ceil() as i64 - 1).max(lo);
    if hi >= i64::from(c.height) {
        return true;
    }
    for (cx, cz) in cols {
        let wx = wrap(cx, c.width);
        if view.solid_in(wx, cz, lo as u32, hi as u32) {
            return true;
        }
        if let Some((top, _, water)) = column_floor(view, cx, cz, base) {
            if water > 0.0 && top > base + EPS {
                return true;
            }
        }
    }
    false
}

/// The highest floor under the disc: solid tops and standing water, over every column the
/// disc covers. `(height, the face, the water's depth on it)`; `None` over a bottomless
/// disc.
pub(crate) fn floor_under(view: &VoxelView<'_>, shape: Shape, x: f64, z: f64, base: f64) -> Option<(f64, Site, f64)> {
    let cols = disc_columns(view, x, z, shape.r)?;
    let mut best: Option<(f64, Site, f64)> = None;
    for (cx, cz) in cols {
        if let Some(f) = column_floor(view, cx, cz, base) {
            if best.is_none_or(|b| f.0 > b.0) {
                best = Some(f);
            }
        }
    }
    best
}

/// The lowest solid over the disc whose bottom is at or above `top`: `(its bottom, the
/// cell)`, looking at most `look_m` up.
fn roof_over(view: &VoxelView<'_>, shape: Shape, x: f64, z: f64, top: f64, look_m: f64) -> Option<(f64, (u32, u32, u32))> {
    let c = view.config;
    let v = c.voxel_m;
    let cols = disc_columns(view, x, z, shape.r)?;
    let from = (((top - EPS) / v).ceil() as i64).max(0);
    let to = (((top + look_m) / v).ceil() as i64).min(i64::from(c.height) - 1);
    let mut best: Option<(f64, (u32, u32, u32))> = None;
    for (cx, cz) in cols {
        let wx = wrap(cx, c.width);
        if from > to || !view.solid_in(wx, cz, from as u32, to as u32) {
            continue;
        }
        for y in from..=to {
            if view.is_solid(wx, y as u32, cz) {
                let b = y as f64 * v;
                if best.is_none_or(|(bb, _)| b < bb) {
                    best = Some((b, (wx as u32, y as u32, cz)));
                }
                break;
            }
        }
    }
    best
}

/// The clear height over the body's top across its whole disc to the first solid, looking
/// at most `cap` up (`cap` when nothing is there).
pub(crate) fn overhead_clear(view: &VoxelView<'_>, shape: Shape, x: f64, z: f64, base: f64, cap: f64) -> f64 {
    let top = base + shape.h;
    roof_over(view, shape, x, z, top, cap).map_or(cap, |(bottom, _)| (bottom - top).clamp(0.0, cap))
}

/// The clear height over the base in the centre's column, to the first solid or water
/// cell, looking at most `cap` up (`cap` when nothing is there).
pub(crate) fn clear_height(view: &VoxelView<'_>, x: f64, z: f64, base: f64, cap: f64) -> f64 {
    let c = view.config;
    let v = c.voxel_m;
    let cz = (z / v).floor();
    if !(0.0..f64::from(c.depth)).contains(&cz) {
        return 0.0;
    }
    let (cx, cz) = (wrap((x / v).floor() as i64, c.width), cz as u32);
    let from = row_at(base, v).max(0);
    let to = (((base + cap) / v).ceil() as i64).min(i64::from(c.height) - 1);
    for y in from..=to {
        let yy = y as u32;
        if view.is_solid(cx, yy, cz) {
            return (y as f64 * v - base).clamp(0.0, cap);
        }
        let f = view.free_at(cx, yy, cz);
        if f > 0.0 && y as f64 * v > base + EPS {
            return (y as f64 * v - base).clamp(0.0, cap);
        }
    }
    cap
}

/// Every foliage-bearing layer holding foliage of a stand whose disc covers column
/// `(cx, cz)`: `(the stand's site, the layer)`.
pub(crate) fn crown_layers_at(view: &VoxelView<'_>, fv: &FloraView<'_>, cx: i64, cz: u32) -> Vec<(Site, StandLayer)> {
    let c = view.config;
    let wx = wrap(cx, c.width);
    let mut out = Vec::new();
    let mut consider = |stand: &cubarium_voxel_flora::Stand| {
        if !(stand.foliage > 0.0) {
            return;
        }
        let dz = i64::from(cz) - i64::from(stand.site.z);
        let mut dx = wx - i64::from(stand.site.x);
        let w = i64::from(c.width);
        dx = dx.rem_euclid(w);
        if dx > w / 2 {
            dx -= w;
        }
        for layer in fv.profile_layers(stand) {
            if !layer.kind.bears_foliage() || !(layer.stock > 0.0) {
                continue;
            }
            if ((dx * dx + dz * dz) as f64) <= layer.radius_v * layer.radius_v {
                out.push((stand.site, layer));
            }
        }
    };
    match fv.stands_near(wx, cz, c.width, c.depth) {
        Some(near) => {
            for &i in near {
                consider(&fv.stands[i as usize]);
            }
        }
        None => {
            for s in fv.stands {
                consider(s);
            }
        }
    }
    out
}

/// Whether the cell holding the body's middle is inside a crown's foliage layer.
pub(crate) fn inside_foliage(view: &VoxelView<'_>, fv: &FloraView<'_>, shape: Shape, x: f64, z: f64, base: f64) -> bool {
    let v = view.config.voxel_m;
    let cz = (z / v).floor();
    if !(0.0..f64::from(view.config.depth)).contains(&cz) {
        return false;
    }
    let row = ((base + 0.5 * shape.h) / v).floor() as i64;
    crown_layers_at(view, fv, (x / v).floor() as i64, cz as u32)
        .iter()
        .any(|(_, l)| l.cell == row)
}

pub(crate) fn foliage_layer_index(layer: &StandLayer) -> u8 {
    layer.foliage_index.unwrap_or(0) as u8
}

/// The layer a stand anchor names, now: `None` when the stand or the layer is gone or
/// holds no foliage.
fn anchored_layer(fv: &FloraView<'_>, site: Site, layer: u8) -> Option<StandLayer> {
    let stand = fv.stand_at(site)?;
    fv.profile_layers(stand)
        .into_iter()
        .find(|l| l.foliage_index == Some(usize::from(layer)) && l.stock > 0.0)
}

/// Whether a perched body's perch still holds it (§3.4): the face is still a support face
/// under no more standing water than `puddle_m` (the body stands on the water's top: the
/// caller follows it); the stand's layer still holds foliage at the height the body is
/// perched at; the vine still covers its face; the overhang is still solid.
pub(crate) fn perch_holds(view: &VoxelView<'_>, fv: &FloraView<'_>, a: &Animal, shape: Shape, puddle_m: f64) -> bool {
    let v = view.config.voxel_m;
    let Some(perch) = a.mobility.perch else {
        return false;
    };
    let y = a.pose.y_m;
    match perch.anchor {
        PerchAnchor::Face(site) => {
            view.is_support(i64::from(site.x), site.y, site.z)
                && view.standing_depth_m(i64::from(site.x), site.y, site.z) <= puddle_m
        }
        PerchAnchor::Stand { site, layer } => {
            let Some(l) = anchored_layer(fv, site, layer) else {
                return false;
            };
            match perch.kind {
                PerchKind::Underside => ((l.cell as f64 * v) - shape.h - y).abs() <= PERCH_SLACK_M,
                _ => (((l.cell + 1) as f64 * v) - y).abs() <= PERCH_SLACK_M,
            }
        }
        PerchAnchor::Vine(face) => fv.cover.face(face).is_some(),
        PerchAnchor::Rock { x, y: ry, z } => {
            ry < view.config.height
                && z < view.config.depth
                && view.is_solid(i64::from(x), ry, z)
        }
    }
}

/// **Room for a flyer on a face** (F1, founding and `IntroduceFounder*`): its disc fits on
/// the top of the support face `site` (centred on the face), with at least `headroom_m`
/// clear over its top across the disc.
pub fn flyer_fits_on_face(view: &VoxelView<'_>, phys: &FounderPhysiology, body: f64, site: Site, headroom_m: f64) -> bool {
    let Some(flight) = phys.flight else {
        return false;
    };
    let v = view.config.voxel_m;
    let shape = Shape::of(phys, &flight, body);
    let (x, z, y) = ((f64::from(site.x) + 0.5) * v, (f64::from(site.z) + 0.5) * v, surface_m(site.y, v));
    !blocked(view, shape, x, z, y) && overhead_clear(view, shape, x, z, y, headroom_m) >= headroom_m
}

/// Put a flyer perched on the top of the support face `site` (an introduced or a newborn
/// body).
pub(crate) fn perch_on_face(a: &mut Animal, site: Site) {
    a.mobility.mode = MobilityMode::Perched;
    a.mobility.perch = Some(Perch {
        kind: PerchKind::Face,
        anchor: PerchAnchor::Face(site),
    });
    a.mobility.falling = false;
    a.mobility.climb = None;
}

/// Where a flyer may be put at `pose` ([`crate::Command::IntroduceAloft`]): the support
/// face below and, when its base rests on a floor, the face perch it is on. `None` for a
/// pose outside the world, a body overlapping solid or water, no support below, or a base
/// above the ceiling.
pub(crate) fn placement(
    view: &VoxelView<'_>,
    phys: &FounderPhysiology,
    flight: &Flight,
    body: f64,
    pose: &crate::Pose,
) -> Option<(Site, Option<Perch>)> {
    if !pose.is_finite() || !pose.y_m.is_finite() {
        return None;
    }
    let shape = Shape::of(phys, flight, body);
    if blocked(view, shape, pose.x, pose.z, pose.y_m) {
        return None;
    }
    let site = support_below(view, pose.x, pose.z, pose.y_m)?;
    if pose.y_m > surface_m(site.y, view.config.voxel_m) + flight.band_max_m + EPS {
        return None;
    }
    let floor = floor_under(view, shape, pose.x, pose.z, pose.y_m);
    let puddle = phys.drown_depth_m(&phys.body_at(body));
    let perch = match floor {
        Some((top, face, water)) if (pose.y_m - top).abs() <= EPS && water <= puddle => Some(Perch {
            kind: PerchKind::Face,
            anchor: PerchAnchor::Face(face),
        }),
        _ => None,
    };
    Some((site, perch))
}

/// One tick of air motion: what moved and what it landed on.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Moved {
    /// Horizontal distance covered, metres.
    pub distance_m: f64,
    /// Height gained (positive) or lost, metres.
    pub rise_m: f64,
    /// A horizontal move the air refused part of.
    pub blocked: bool,
    /// The perch it came to rest on.
    pub landed: Option<Perch>,
}

/// Move a flyer's pose through the air (§3.2): turn, then `distance` along its heading in
/// bounded sub-steps (sliding along a blocking wall, never into it, never over the
/// ceiling of the column it moves over), then `dy` up or down against the floor, the
/// roof, the ceiling and the crowns. A descent onto a crown layer's top, a floor or a
/// covered wall face, and a climb into an underside, end on a perch.
#[allow(clippy::too_many_arguments)]
pub(crate) fn move_through_air(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    shape: Shape,
    band_max_m: f64,
    puddle_m: f64,
    pose: &mut crate::Pose,
    distance: f64,
    dy: f64,
) -> Moved {
    let c = view.config;
    let v = c.voxel_m;
    let mut out = Moved::default();
    let ok = |x: f64, z: f64, y: f64| -> bool {
        if blocked(view, shape, x, z, y) {
            return false;
        }
        match support_below(view, x, z, y) {
            Some(s) => y <= surface_m(s.y, v) + band_max_m + EPS,
            None => false,
        }
    };

    // Horizontal, in sub-steps of at most a quarter voxel.
    if distance > 0.0 {
        let (fx, fz) = pose.forward();
        let n = ((distance / (0.25 * v)).ceil() as usize).max(1);
        let step = distance / n as f64;
        for _ in 0..n {
            let (x0, z0) = (pose.x, pose.z);
            let (tx, tz) = (x0 + fx * step, z0 + fz * step);
            if ok(tx, tz, pose.y_m) {
                pose.x = tx;
                pose.z = tz;
            } else {
                out.blocked = true;
                // Slide along the wall: each axis alone, else the furthest fraction of the
                // sub-step that fits.
                if fx != 0.0 && ok(tx, z0, pose.y_m) {
                    pose.x = tx;
                } else if fz != 0.0 && ok(x0, tz, pose.y_m) {
                    pose.z = tz;
                } else {
                    let (mut lo, mut hi) = (0.0f64, 1.0f64);
                    for _ in 0..24 {
                        let mid = 0.5 * (lo + hi);
                        if ok(x0 + fx * step * mid, z0 + fz * step * mid, pose.y_m) {
                            lo = mid;
                        } else {
                            hi = mid;
                        }
                    }
                    pose.x = x0 + fx * step * lo;
                    pose.z = z0 + fz * step * lo;
                }
            }
            let (dx, dz) = (pose.x - x0, pose.z - z0);
            out.distance_m += (dx * dx + dz * dz).sqrt();
        }
        pose.x = pose.x.rem_euclid(f64::from(c.width) * v);
    }

    let y0 = pose.y_m;
    let cz = (pose.z / v).floor().clamp(0.0, f64::from(c.depth) - 1.0) as u32;
    let cx = (pose.x / v).floor() as i64;
    if dy < 0.0 {
        let floor = floor_under(view, shape, pose.x, pose.z, y0);
        let floor_h = floor.map_or(0.0, |f| f.0);
        let mut y = (y0 + dy).max(floor_h);
        // The first leafy layer top crossed on the way down, at the centre's column.
        let crown = crown_layers_at(view, fv, cx, cz)
            .into_iter()
            .map(|(site, l)| ((l.cell + 1) as f64 * v, site, l))
            .filter(|&(top, _, _)| top <= y0 + EPS && top > y + EPS && top >= floor_h - EPS)
            .max_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((top, site, l)) = crown {
            y = top;
            out.landed = Some(Perch {
                kind: PerchKind::CrownTop,
                anchor: PerchAnchor::Stand {
                    site,
                    layer: foliage_layer_index(&l),
                },
            });
        } else if let Some((top, face, water)) = floor {
            // A face under no more water than a puddle the body stands in is a landing;
            // deeper water is not (it hovers on the surface).
            if y <= top + EPS && water <= puddle_m {
                y = top;
                out.landed = Some(Perch {
                    kind: PerchKind::Face,
                    anchor: PerchAnchor::Face(face),
                });
            }
        }
        if out.landed.is_none() {
            if let Some(face) = vine_contact(view, fv, shape, pose.x, pose.z, y) {
                out.landed = Some(Perch {
                    kind: PerchKind::VineFace,
                    anchor: PerchAnchor::Vine(face),
                });
            }
        }
        pose.y_m = y;
    } else if dy > 0.0 {
        let site = support_below(view, pose.x, pose.z, y0);
        let ceiling = site.map_or(y0, |s| surface_m(s.y, v) + band_max_m).max(y0);
        let top0 = y0 + shape.h;
        let mut y = (y0 + dy).min(ceiling);
        let roof = roof_over(view, shape, pose.x, pose.z, top0, dy + shape.h + v);
        if let Some((bottom, _)) = roof {
            y = y.min(bottom - shape.h).max(y0);
        }
        let crown = crown_layers_at(view, fv, cx, cz)
            .into_iter()
            .map(|(site, l)| (l.cell as f64 * v, site, l))
            .filter(|&(bottom, _, _)| bottom >= top0 - EPS && bottom < y + shape.h - EPS)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((bottom, site, l)) = crown {
            y = bottom - shape.h;
            out.landed = Some(Perch {
                kind: PerchKind::Underside,
                anchor: PerchAnchor::Stand {
                    site,
                    layer: foliage_layer_index(&l),
                },
            });
        } else if let Some((bottom, (rx, ry, rz))) = roof {
            if y + shape.h >= bottom - EPS {
                y = bottom - shape.h;
                out.landed = Some(Perch {
                    kind: PerchKind::Underside,
                    anchor: PerchAnchor::Rock { x: rx, y: ry, z: rz },
                });
            }
        }
        pose.y_m = y;
    }
    out.rise_m = pose.y_m - y0;
    out
}

/// A latticevine-covered wall face the disc is pressed against (within [`CLING_M`]) at
/// the row of the body's middle.
pub(crate) fn vine_contact(view: &VoxelView<'_>, fv: &FloraView<'_>, shape: Shape, x: f64, z: f64, base: f64) -> Option<VineFace> {
    if fv.cover.face_count() == 0 {
        return None;
    }
    let c = view.config;
    let v = c.voxel_m;
    let row = ((base + 0.5 * shape.h) / v).floor() as i64;
    if row < 0 || row >= i64::from(c.height) {
        return None;
    }
    let (cx, cz) = ((x / v).floor() as i64, (z / v).floor() as i64);
    let reach = shape.r + CLING_M;
    let mut best: Option<(f64, VineFace)> = None;
    for (dx, dz, dir) in [
        (1i64, 0i64, FaceDir::NegX),
        (-1, 0, FaceDir::PosX),
        (0, 1, FaceDir::NegZ),
        (0, -1, FaceDir::PosZ),
    ] {
        let (sx, sz) = (cx + dx, cz + dz);
        if sz < 0 || sz >= i64::from(c.depth) {
            continue;
        }
        let wx = wrap(sx, c.width);
        if !view.is_solid(wx, row as u32, sz as u32) {
            continue;
        }
        let gap = match dir {
            FaceDir::NegX => sx as f64 * v - x,
            FaceDir::PosX => x - (sx + 1) as f64 * v,
            FaceDir::NegZ => sz as f64 * v - z,
            _ => z - (sz + 1) as f64 * v,
        };
        if gap > reach || gap < 0.0 {
            continue;
        }
        let face = VineFace::new(wx as u32, row as u32, sz as u32, dir);
        if fv.cover.face(face).is_some() && best.is_none_or(|(g, _)| gap < g) {
            best = Some((gap, face));
        }
    }
    best.map(|(_, f)| f)
}

/// The mouth's point: the body's middle.
pub(crate) fn mouth_point(a: &Animal, shape: Shape) -> (f64, f64, f64) {
    (a.pose.x, a.pose.y_m + 0.5 * shape.h, a.pose.z)
}

/// The eye: the body's centre column at `eye_m` over its base.
pub(crate) fn eye(a: &Animal, geometry: &crate::Body) -> (f64, f64, f64) {
    (a.pose.x, a.pose.y_m + geometry.eye_m, a.pose.z)
}

/// Distance from `p` to the box `[x0, x1] × [y0, y1] × [z0, z1]`.
fn box_distance(p: (f64, f64, f64), x: (f64, f64), y: (f64, f64), z: (f64, f64)) -> f64 {
    let d = |v: f64, (lo, hi): (f64, f64)| (lo - v).max(0.0).max(v - hi);
    let (dx, dy, dz) = (d(p.0, x), d(p.1, y), d(p.2, z));
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// The top foliage-bearing layer of a stand: its flower head, where a bloom is.
fn bloom_layer(fv: &FloraView<'_>, stand: &cubarium_voxel_flora::Stand) -> Option<StandLayer> {
    fv.profile_layers(stand)
        .into_iter()
        .filter(|l| l.kind.bears_foliage())
        .max_by_key(|l| l.cell)
}

/// **The nearest bloom within `reach` of `p`** (§3.7): a bloomcrown's flower head (its
/// top foliage layer) holding nectar, or a latticevine face in flower holding nectar and
/// facing `p`. `(the bloom, its nectar stock)`; a stand wins a tie.
pub(crate) fn bloom_in_reach(view: &VoxelView<'_>, fv: &FloraView<'_>, p: (f64, f64, f64), reach: f64) -> Option<(Bloom, f64)> {
    let c = view.config;
    let v = c.voxel_m;
    let ring = f64::from(c.width) * v;
    let mut best: Option<(f64, Bloom, f64)> = None;
    for stand in fv.stands.iter().filter(|s| s.nectar > 0.0) {
        let sx = (f64::from(stand.site.x) + 0.5) * v;
        let sz = (f64::from(stand.site.z) + 0.5) * v;
        let (dx, dz) = crate::pose::wrapped_delta(p.0, p.2, sx, sz, ring);
        if (dx * dx + dz * dz).sqrt() > reach + 3.0 * v + 1.0 {
            continue;
        }
        let Some(layer) = bloom_layer(fv, stand) else {
            continue;
        };
        let span = layer.radius_v.floor() as i64;
        let r2 = layer.radius_v * layer.radius_v;
        let y = (layer.cell as f64 * v, (layer.cell + 1) as f64 * v);
        // The stand's column centre in `p`'s unwrapped frame.
        let (ox, oz) = (p.0 + dx - 0.5 * v, p.2 + dz - 0.5 * v);
        for ddz in -span..=span {
            for ddx in -span..=span {
                if ((ddx * ddx + ddz * ddz) as f64) > r2 {
                    continue;
                }
                let gz = i64::from(stand.site.z) + ddz;
                if gz < 0 || gz >= i64::from(c.depth) {
                    continue;
                }
                let bx = ox + ddx as f64 * v;
                let bz = oz + ddz as f64 * v;
                let d = box_distance(p, (bx, bx + v), y, (bz, bz + v));
                if d <= reach && best.as_ref().is_none_or(|b| d < b.0) {
                    best = Some((d, Bloom::Stand(stand.site), stand.nectar));
                }
            }
        }
    }
    if fv.cover.face_count() > 0 {
        let (cx, cy, cz) = ((p.0 / v).floor() as i64, (p.1 / v).floor() as i64, (p.2 / v).floor() as i64);
        for sy in cy - 1..=cy + 1 {
            if sy < 0 || sy >= i64::from(c.height) {
                continue;
            }
            for sz in cz - 1..=cz + 1 {
                if sz < 0 || sz >= i64::from(c.depth) {
                    continue;
                }
                for sx in cx - 1..=cx + 1 {
                    let wx = wrap(sx, c.width);
                    if !view.is_solid(wx, sy as u32, sz as u32) {
                        continue;
                    }
                    let (x0, y0, z0) = (sx as f64 * v, sy as f64 * v, sz as f64 * v);
                    for dir in FaceDir::ALL {
                        // The face's plane, and whether `p` is in front of it.
                        let (front, d) = match dir {
                            FaceDir::NegX => (p.0 < x0, box_distance(p, (x0, x0), (y0, y0 + v), (z0, z0 + v))),
                            FaceDir::PosX => (p.0 > x0 + v, box_distance(p, (x0 + v, x0 + v), (y0, y0 + v), (z0, z0 + v))),
                            FaceDir::NegZ => (p.2 < z0, box_distance(p, (x0, x0 + v), (y0, y0 + v), (z0, z0))),
                            FaceDir::PosZ => (p.2 > z0 + v, box_distance(p, (x0, x0 + v), (y0, y0 + v), (z0 + v, z0 + v))),
                            FaceDir::Down => (p.1 < y0, box_distance(p, (x0, x0 + v), (y0, y0), (z0, z0 + v))),
                        };
                        if !front || d > reach || best.as_ref().is_some_and(|b| d >= b.0) {
                            continue;
                        }
                        let face = VineFace::new(wx as u32, sy as u32, sz as u32, dir);
                        if let Some(cf) = fv.cover.face(face) {
                            if cf.spur == SpurPhase::Flower && cf.nectar > 0.0 {
                                best = Some((d, Bloom::Vine(face), cf.nectar));
                            }
                        }
                    }
                }
            }
        }
    }
    best.map(|(_, b, n)| (b, n))
}

/// Whether a fan ray's first hit at `cell` is a bloom: a cell of a nectar-holding stand's
/// flower head, or (for a terrain hit) a flowering vine face the ray entered.
fn bloom_at_hit(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    cell: usize,
    terrain: bool,
    origin: (f64, f64, f64),
    dir: (f64, f64, f64),
    t: f64,
) -> bool {
    let c = view.config;
    let v = c.voxel_m;
    let (x, y, z) = c.coords(cell);
    let (x, y, z) = (i64::from(x), i64::from(y), z);
    if terrain {
        if fv.cover.face_count() == 0 {
            return false;
        }
        // The cell the ray came from, just before it entered.
        let back = (t - 1e-6).max(0.0);
        let (px, py, pz) = (
            ((origin.0 + dir.0 * back) / v).floor() as i64,
            ((origin.1 + dir.1 * back) / v).floor() as i64,
            ((origin.2 + dir.2 * back) / v).floor() as i64,
        );
        let w = i64::from(c.width);
        let mut ddx = (px - x).rem_euclid(w);
        if ddx > w / 2 {
            ddx -= w;
        }
        let face_dir = match (ddx, py - y, pz - i64::from(z)) {
            (-1, 0, 0) => FaceDir::NegX,
            (1, 0, 0) => FaceDir::PosX,
            (0, 0, -1) => FaceDir::NegZ,
            (0, 0, 1) => FaceDir::PosZ,
            (0, -1, 0) => FaceDir::Down,
            _ => return false,
        };
        let face = VineFace::new(x as u32, y as u32, z, face_dir);
        return fv
            .cover
            .face(face)
            .is_some_and(|cf| cf.spur == SpurPhase::Flower && cf.nectar > 0.0);
    }
    let w = i64::from(c.width);
    fv.stands.iter().filter(|s| s.nectar > 0.0).any(|s| {
        let Some(layer) = bloom_layer(fv, s) else {
            return false;
        };
        if layer.cell != y {
            return false;
        }
        let mut dx = (x - i64::from(s.site.x)).rem_euclid(w);
        if dx > w / 2 {
            dx -= w;
        }
        let dz = i64::from(z) - i64::from(s.site.z);
        ((dx * dx + dz * dz) as f64) <= layer.radius_v * layer.radius_v
    })
}

/// The face of terrain cell `(x, y, z)` a ray entered it through: the cell it came from
/// just before `t`, one step away along an axis.
fn entered_face(
    view: &VoxelView<'_>,
    (x, y, z): (i64, i64, u32),
    origin: (f64, f64, f64),
    dir: (f64, f64, f64),
    t: f64,
) -> Option<VineFace> {
    let c = view.config;
    let v = c.voxel_m;
    let back = (t - 1e-6).max(0.0);
    let (px, py, pz) = (
        ((origin.0 + dir.0 * back) / v).floor() as i64,
        ((origin.1 + dir.1 * back) / v).floor() as i64,
        ((origin.2 + dir.2 * back) / v).floor() as i64,
    );
    let w = i64::from(c.width);
    let mut ddx = (px - x).rem_euclid(w);
    if ddx > w / 2 {
        ddx -= w;
    }
    let face_dir = match (ddx, py - y, pz - i64::from(z)) {
        (-1, 0, 0) => FaceDir::NegX,
        (1, 0, 0) => FaceDir::PosX,
        (0, 0, -1) => FaceDir::NegZ,
        (0, 0, 1) => FaceDir::PosZ,
        (0, -1, 0) => FaceDir::Down,
        _ => return None,
    };
    Some(VineFace::new(x as u32, y as u32, z, face_dir))
}

/// Whether a cone ray's first hit at `cell` is **fruit** (F3, the seedporter's fruit cone):
/// a foliage cell of a stand hanging fruit, or (for a terrain hit) a latticevine face in
/// fruit, holding beads, that the ray entered.
pub(crate) fn fruit_at_hit(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    cell: usize,
    terrain: bool,
    origin: (f64, f64, f64),
    dir: (f64, f64, f64),
    t: f64,
) -> bool {
    let c = view.config;
    let (x, y, z) = c.coords(cell);
    let (x, y, z) = (i64::from(x), i64::from(y), z);
    if terrain {
        if fv.cover.face_count() == 0 {
            return false;
        }
        return entered_face(view, (x, y, z), origin, dir, t).is_some_and(|face| {
            fv.cover
                .face(face)
                .is_some_and(|cf| cf.spur == SpurPhase::Fruit && cf.fruit > 0.0)
        });
    }
    crown_layers_at(view, fv, x, z)
        .iter()
        .any(|(site, l)| l.cell == y && fv.stand_at(*site).is_some_and(|s| s.fruit > 0.0))
}

/// **The nearest fruit within `reach` of `p`** (F3): a foliage cell of a stand hanging
/// fruit (any of its foliage layers holding foliage), or a latticevine face in fruit holding
/// beads and facing `p`. `(the source, its fruit stock)`; a stand wins a tie.
/// `stands` limits which stands count: `None` none, `Some(None)` any, `Some(Some(site))` the
/// one rooted on `site`.
pub(crate) fn fruit_in_reach(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    p: (f64, f64, f64),
    reach: f64,
    stands: Option<Option<Site>>,
) -> Option<(Bloom, f64)> {
    let c = view.config;
    let v = c.voxel_m;
    let ring = f64::from(c.width) * v;
    let mut best: Option<(f64, Bloom, f64)> = None;
    for stand in fv
        .stands
        .iter()
        .filter(|s| s.fruit > 0.0 && stands.is_some_and(|only| only.is_none_or(|site| site == s.site)))
    {
        let sx = (f64::from(stand.site.x) + 0.5) * v;
        let sz = (f64::from(stand.site.z) + 0.5) * v;
        let (dx, dz) = crate::pose::wrapped_delta(p.0, p.2, sx, sz, ring);
        if (dx * dx + dz * dz).sqrt() > reach + 3.0 * v + 1.0 {
            continue;
        }
        let (ox, oz) = (p.0 + dx - 0.5 * v, p.2 + dz - 0.5 * v);
        for layer in fv.profile_layers(stand) {
            if !layer.kind.bears_foliage() || !(layer.stock > 0.0) {
                continue;
            }
            let span = layer.radius_v.floor() as i64;
            let r2 = layer.radius_v * layer.radius_v;
            let y = (layer.cell as f64 * v, (layer.cell + 1) as f64 * v);
            for ddz in -span..=span {
                for ddx in -span..=span {
                    if ((ddx * ddx + ddz * ddz) as f64) > r2 {
                        continue;
                    }
                    let gz = i64::from(stand.site.z) + ddz;
                    if gz < 0 || gz >= i64::from(c.depth) {
                        continue;
                    }
                    let bx = ox + ddx as f64 * v;
                    let bz = oz + ddz as f64 * v;
                    let d = box_distance(p, (bx, bx + v), y, (bz, bz + v));
                    if d <= reach && best.as_ref().is_none_or(|b| d < b.0) {
                        best = Some((d, Bloom::Stand(stand.site), stand.fruit));
                    }
                }
            }
        }
    }
    if fv.cover.face_count() > 0 {
        let (cx, cy, cz) = ((p.0 / v).floor() as i64, (p.1 / v).floor() as i64, (p.2 / v).floor() as i64);
        for sy in cy - 1..=cy + 1 {
            if sy < 0 || sy >= i64::from(c.height) {
                continue;
            }
            for sz in cz - 1..=cz + 1 {
                if sz < 0 || sz >= i64::from(c.depth) {
                    continue;
                }
                for sx in cx - 1..=cx + 1 {
                    let wx = wrap(sx, c.width);
                    if !view.is_solid(wx, sy as u32, sz as u32) {
                        continue;
                    }
                    let (x0, y0, z0) = (sx as f64 * v, sy as f64 * v, sz as f64 * v);
                    for dir in FaceDir::ALL {
                        let (front, d) = match dir {
                            FaceDir::NegX => (p.0 < x0, box_distance(p, (x0, x0), (y0, y0 + v), (z0, z0 + v))),
                            FaceDir::PosX => (p.0 > x0 + v, box_distance(p, (x0 + v, x0 + v), (y0, y0 + v), (z0, z0 + v))),
                            FaceDir::NegZ => (p.2 < z0, box_distance(p, (x0, x0 + v), (y0, y0 + v), (z0, z0))),
                            FaceDir::PosZ => (p.2 > z0 + v, box_distance(p, (x0, x0 + v), (y0, y0 + v), (z0 + v, z0 + v))),
                            FaceDir::Down => (p.1 < y0, box_distance(p, (x0, x0 + v), (y0, y0), (z0, z0 + v))),
                        };
                        if !front || d > reach || best.as_ref().is_some_and(|b| d >= b.0) {
                            continue;
                        }
                        let face = VineFace::new(wx as u32, sy as u32, sz as u32, dir);
                        if let Some(cf) = fv.cover.face(face) {
                            if cf.spur == SpurPhase::Fruit && cf.fruit > 0.0 {
                                best = Some((d, Bloom::Vine(face), cf.fruit));
                            }
                        }
                    }
                }
            }
        }
    }
    best.map(|(_, b, n)| (b, n))
}

/// A flyer's fan ray, first hit named, [`ConeHit::Bloom`] among them.
pub(crate) fn fan_first_hit(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    occupancy: &crate::senses::ConeOccupancy,
    observer: u64,
    origin: (f64, f64, f64),
    dir: (f64, f64, f64),
    range: f64,
) -> Option<(f64, ConeHit)> {
    let (t, fine, cell) = crate::senses::ray_first_hit_cell(view, occupancy, observer, origin, dir, range)?;
    let hit = crate::cone_hit_of(fine);
    let bloom = match hit {
        ConeHit::FoliageCrown => bloom_at_hit(view, fv, cell, false, origin, dir, t),
        ConeHit::Terrain => bloom_at_hit(view, fv, cell, true, origin, dir, t),
        _ => false,
    };
    Some((t, if bloom { ConeHit::Bloom } else { hit }))
}

/// **Sheltered** (the census's downpour reading, §3.8): something overhead within the
/// band — a solid or water cell in the centre's column, or a crown's foliage layer over
/// it.
pub fn flyer_sheltered(view: &VoxelView<'_>, fv: &FloraView<'_>, a: &Animal, band_max_m: f64) -> bool {
    let v = view.config.voxel_m;
    let base = a.pose.y_m;
    if clear_height(view, a.pose.x, a.pose.z, base, band_max_m) < band_max_m {
        return true;
    }
    let cz = (a.pose.z / v).floor();
    if !(0.0..f64::from(view.config.depth)).contains(&cz) {
        return false;
    }
    let head = base + 0.01;
    crown_layers_at(view, fv, (a.pose.x / v).floor() as i64, cz as u32)
        .iter()
        .any(|(_, l)| l.cell as f64 * v >= head - EPS && l.cell as f64 * v <= base + band_max_m)
}

/// **The open air** a flyer can reach (bellwing3 item 2): cells that are not solid, not
/// under standing water, and no higher than `band_max_m` over the support face below them,
/// split into 6-connected components (a point body), with the largest named the open air.
/// `founder_tally reach`'s rule.
pub(crate) struct OpenAir {
    width: usize,
    height: usize,
    comp: Vec<u32>,
    open: u32,
}

impl std::fmt::Debug for OpenAir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OpenAir {{ cells: {}, open: {} }}", self.comp.len(), self.open)
    }
}

impl OpenAir {
    pub(crate) fn build(view: &VoxelView<'_>, band_max_m: f64) -> OpenAir {
        let c = view.config;
        let v = c.voxel_m;
        let (w, d, h) = (c.width as usize, c.depth as usize, c.height as usize);
        let idx = |x: usize, y: usize, z: usize| (z * h + y) * w + x;
        let air = |x: usize, y: usize, z: usize| -> bool {
            if y == 0 || view.is_solid(x as i64, y as u32, z as u32) {
                return false;
            }
            let Some(s) = view.highest_support_in(x as i64, z as u32, 0, (y - 1) as u32) else {
                return false;
            };
            let top = surface_m(s, v);
            let water = view.standing_depth_m(x as i64, s, z as u32);
            let base = y as f64 * v;
            base <= top + band_max_m + EPS && base + EPS >= top + water
        };
        let mut comp = vec![u32::MAX; w * h * d];
        let mut sizes: Vec<u64> = Vec::new();
        let mut stack = Vec::new();
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if comp[idx(x, y, z)] != u32::MAX || !air(x, y, z) {
                        continue;
                    }
                    let id = sizes.len() as u32;
                    comp[idx(x, y, z)] = id;
                    stack.push((x, y, z));
                    let mut n = 0u64;
                    while let Some((x, y, z)) = stack.pop() {
                        n += 1;
                        let mut visit = |a: usize, b: usize, cc: usize, stack: &mut Vec<(usize, usize, usize)>| {
                            if comp[idx(a, b, cc)] == u32::MAX && air(a, b, cc) {
                                comp[idx(a, b, cc)] = id;
                                stack.push((a, b, cc));
                            }
                        };
                        visit((x + 1) % w, y, z, &mut stack);
                        visit((x + w - 1) % w, y, z, &mut stack);
                        if y + 1 < h {
                            visit(x, y + 1, z, &mut stack);
                        }
                        if y > 0 {
                            visit(x, y - 1, z, &mut stack);
                        }
                        if z + 1 < d {
                            visit(x, y, z + 1, &mut stack);
                        }
                        if z > 0 {
                            visit(x, y, z - 1, &mut stack);
                        }
                    }
                    sizes.push(n);
                }
            }
        }
        let open = (0..sizes.len()).max_by_key(|&k| sizes[k]).map_or(u32::MAX, |k| k as u32);
        OpenAir { width: w, height: h, comp, open }
    }

    /// Whether the air cell over support face `site` is in the open air.
    pub(crate) fn over_face_is_open(&self, site: Site) -> bool {
        let y = site.y as usize + 1;
        if y >= self.height || self.open == u32::MAX {
            return false;
        }
        let k = (site.z as usize * self.height + y) * self.width + site.x as usize;
        self.comp.get(k).is_some_and(|&c| c == self.open)
    }
}
