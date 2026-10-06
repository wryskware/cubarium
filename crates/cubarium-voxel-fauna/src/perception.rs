//! **Perception** (VEILHOUND-SEARCH, the base of the senses pass): which bodies a body
//! senses by its lineage's [`crate::SenseTable`] — sight, hearing and smell. A world query
//! standing for the senses, read by heuristics (today the Veilhound's pack tactic,
//! [`crate::pack::pack_sensed`]); the manifest's cone, the brain's observation, is separate
//! and unchanged.
//!
//! Cost: a distance test per (observer, body) pair inside the table's longest range, and
//! for sight alone — after smell and hearing missed — one terrain-only ray march
//! ([`crate::senses::ray_first_hit`], cell-exact, at most `sight_m / voxel` cells).

use cubarium_voxel::VoxelView;

use crate::{Animal, FaunaConfig, SenseTable};

/// The senses' indices (the ledger's `[sight, hearing, smell]` lines).
pub(crate) const SIGHT: u8 = 0;
pub(crate) const HEARING: u8 = 1;
pub(crate) const SMELL: u8 = 2;

/// The occupancy sight is marched under: terrain (and the world's edges and free water)
/// only — no body or stand blocks it.
struct TerrainOnly;

impl crate::senses::CellOccupancy for TerrainOnly {
    fn any(&self, _cell: usize) -> bool {
        false
    }
    fn other_body(&self, _cell: usize, _observer_id: u64) -> bool {
        false
    }
    fn environment(&self, _cell: usize) -> Option<crate::senses::Fine> {
        None
    }
    fn pool_top(&self, _cell: usize) -> Option<f64> {
        None
    }
}

/// Which of `b`'s senses (`table`) has `o`, `d` metres off along the ring delta `(dx, dz)`,
/// trying the cheapest first: smell (any body within `smell_m`), then hearing (a body moving
/// at `hearing_min_speed_m_s` or faster within `hearing_m`), then sight (within `sight_m`
/// inside the forward cone, on a line of sight from its eye to the other's middle that
/// terrain does not block). `None`: it senses nothing.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sense_of(
    config: &FaunaConfig,
    view: &VoxelView<'_>,
    table: &SenseTable,
    b: &Animal,
    o: &Animal,
    (dx, dz): (f64, f64),
    d: f64,
) -> Option<u8> {
    if d <= table.smell_m {
        return Some(SMELL);
    }
    if d <= table.hearing_m && o.speed_m_s >= table.hearing_min_speed_m_s {
        return Some(HEARING);
    }
    if !(d <= table.sight_m) {
        return None;
    }
    let (fx, fz) = b.pose.forward();
    if d > 0.0 && (dx * fx + dz * fz) / d < table.sight_half_angle_deg.to_radians().cos() - 1e-12 {
        return None;
    }
    let (Some(geometry), Some(og)) = (crate::body_of(config, b), crate::body_of(config, o)) else {
        return None;
    };
    let v = view.config.voxel_m;
    let origin = crate::senses::cone_origin(view, &b.pose, b.sense_layer(v), &geometry);
    let base = if o.mobility.mode != crate::MobilityMode::Ground && o.pose.y_m.is_finite() {
        o.pose.y_m
    } else {
        crate::surface_m(o.sense_layer(v), v)
    };
    let (ex, ey, ez) = (dx, base + 0.5 * og.height_m - origin.1, dz);
    let dist = (ex * ex + ey * ey + ez * ez).sqrt();
    if !(dist > 0.0) {
        return Some(SIGHT);
    }
    let dir = (ex / dist, ey / dist, ez / dist);
    match crate::senses::ray_first_hit(view, &TerrainOnly, b.id, origin, dir, dist) {
        Some((hit, _)) if hit < dist - v => None,
        _ => Some(SIGHT),
    }
}
