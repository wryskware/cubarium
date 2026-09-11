//! Chord-filtered all-pairs neighbor lists with exact local unfolding.

use cubarium_surface::{ChartImage, SurfacePoint, Vec2};

use crate::ids::OrganismId;

/// One neighbor as seen from an observer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Neighbor {
    pub id: OrganismId,
    /// Neighbor position in the observer's chart.
    pub local: Vec2,
    pub distance: f64,
    pub extent: f64,
}

/// Per-organism bounded neighbor lists, rebuilt each tick.
#[derive(Clone, Debug, Default)]
pub struct NeighborLists {
    /// Indexed by slot; each list sorted by `(distance, id)` and truncated to `max_neighbors`.
    pub lists: Vec<Vec<Neighbor>>,
    /// Statistics for telemetry.
    pub pairs_considered: u64,
    pub pairs_unfolded: u64,
    pub lists_truncated: u64,
}

/// Input row for the pair pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub id: OrganismId,
    pub pos: SurfacePoint,
    pub sense_radius: f64,
    pub extent: f64,
}

/// Normative: for every unordered pair `(i, j)` with `i < j` by slot, let
/// `reach = max(sense_i, sense_j) + extent_i + extent_j`; skip if `chord_sq > reach²`;
/// else `unfold(pos_i, pos_j, reach)` once and, when `Some`, add `j` to `i`'s list if
/// `distance ≤ sense_i + extent_i + extent_j` and `i` to `j`'s list if
/// `distance ≤ sense_j + extent_i + extent_j` (the reverse `local` is
/// `pos_j.chart() + map.inverse().apply(pos_i.chart() − local)`; unfolding is symmetric).
/// Lists are then sorted by `(distance, id)` and truncated to `max_neighbors`, counting
/// truncations. `images` caches `chart_images` for the five faces (index by face).
pub fn build(bodies: &[Body], images: &[Vec<ChartImage>; 5], max_neighbors: usize, out: &mut NeighborLists) {
    let _ = (bodies, images, max_neighbors, out);
    todo!("pairs::build")
}
