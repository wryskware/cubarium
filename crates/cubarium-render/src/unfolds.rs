//! A cache of stamp footprints, for anchors that do not move (FW-P's W2).
//!
//! `unfold_pixels` is 14.4 % of the cube frame (2.00 ms on the board) and it is called
//! once per stamp, every frame, from anchors that are fixed for the presenter's life: a
//! plant slot's `slot_of(cell).at` and the ground lattice never move. Forty-four per cent
//! of those calls miss the in-chart fast path and walk every chart image of the anchor's
//! face, and each one of those allocates a `Vec<ChartImage>` on the way (W2b).
//!
//! [`Unfolds`] answers the same question from a table keyed by the anchor.
//!
//! **Why a cache can be right at all, when the radius changes every frame.** The wind
//! moves a stamp's radius (`(extent + |amplitude|) · scale`) from frame to frame, so
//! keying on the radius as well would miss every time. It does not have to:
//! `unfold_pixels` at radius `R` returns a *superset* of its own answer at any `r ≤ R`,
//! with each pixel's owning unfolding, `local` and `distance` identical — the owner is the
//! shortest valid image and does not depend on how far the query looked. So one entry per
//! anchor, held at the largest radius that anchor has asked for, answers every smaller
//! query by dropping the pixels past `r + GEOM_EPS`. That test is one comparison against a
//! number the entry already carries, against a rebuild that costs microseconds.
//!
//! **Bucketed by row band.** `unfold_pixels` emits its pixels sorted by `(chart, y, x)`,
//! which is exactly the canvas's global row order, so a band's share of a cached footprint
//! is a contiguous sub-slice (`Canvas::band_pixels`) — the split costs two partition points
//! per stamp rather than making every band redo every stamp's geometry.
//!
//! **Invalidation** is by hand and total: [`Unfolds::clear`] when the anchor set changes.
//! There is nothing to invalidate otherwise, because the answer is a function of the
//! topology and the anchor alone.

use std::collections::HashMap;

use cubarium_surface::{PixelImage, SurfacePoint, Topology, unfold_pixels};

/// An anchor, by the bits of its chart coordinates: two anchors that are equal here give
/// the same footprint, and a recomputed anchor is bit-equal to the one it replaces.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key(u8, u64, u64);

struct Entry {
    start: u32,
    len: u32,
    /// The radius this entry was built at; it answers every query up to it.
    radius: f64,
}

/// The scratch a stamp unfolds into, optionally with a footprint cache behind it.
///
/// [`Unfolds::new`] is a plain scratch buffer and costs exactly what passing a
/// `Vec<PixelImage>` costs. [`Unfolds::cached`] adds the table.
pub struct Unfolds {
    scratch: Vec<PixelImage>,
    map: HashMap<Key, Entry>,
    pool: Vec<PixelImage>,
    /// Live pixels in `pool`; the rest is space left behind by entries that grew.
    live: usize,
    enabled: bool,
    /// Stop admitting new anchors past this many pooled pixels, so a moving anchor cannot
    /// grow the table without bound.
    capacity: usize,
    hits: u64,
    misses: u64,
}

/// Roughly 64 MiB of `PixelImage` at the cube's 48 bytes each: far more than the 1,280
/// plant slots and 320 ground stamps need, and a hard stop for anything that moves.
const DEFAULT_CAPACITY: usize = 1 << 20;

/// Below this the dead space is not worth a rebuild.
const COMPACT_FLOOR: usize = 1 << 16;

impl Unfolds {
    /// A plain scratch buffer: every stamp unfolds, exactly as before.
    pub fn new() -> Unfolds {
        Unfolds {
            scratch: Vec::new(),
            map: HashMap::new(),
            pool: Vec::new(),
            live: 0,
            enabled: false,
            capacity: DEFAULT_CAPACITY,
            hits: 0,
            misses: 0,
        }
    }

    /// A scratch buffer that remembers each anchor's footprint.
    pub fn cached() -> Unfolds {
        Unfolds { enabled: true, ..Unfolds::new() }
    }

    /// Turn the cache on or off without dropping what it holds — a presenter leaves it on
    /// for the passes whose anchors are fixed and off for the ones that move.
    pub fn set_caching(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_caching(&self) -> bool {
        self.enabled
    }

    /// Forget every footprint. The only invalidation there is: call it when the anchor set
    /// changes (a fresh world, a re-seeded lattice, a different topology).
    pub fn clear(&mut self) {
        self.map.clear();
        self.pool.clear();
        self.live = 0;
        self.hits = 0;
        self.misses = 0;
    }

    /// `(hits, misses, cached anchors, pooled pixels)`.
    pub fn stats(&self) -> (u64, u64, usize, usize) {
        (self.hits, self.misses, self.map.len(), self.live)
    }

    /// The pixels within `radius` of `anchor`, from the cache when it has an answer at
    /// least that wide. The slice may be a **superset**: a caller must drop the pixels
    /// whose `distance` exceeds its own limit, which is what every stamp already does.
    pub fn pixels(
        &mut self,
        topo: Topology,
        anchor: SurfacePoint,
        radius: f64,
    ) -> &[PixelImage] {
        if !self.enabled {
            unfold_pixels(topo, anchor, radius, &mut self.scratch);
            return &self.scratch;
        }
        let key = Key(anchor.face.index() as u8, anchor.u.to_bits(), anchor.v.to_bits());
        if let Some(e) = self.map.get(&key)
            && e.radius >= radius
        {
            self.hits += 1;
            let (start, len) = (e.start as usize, e.len as usize);
            return &self.pool[start..start + len];
        }
        self.misses += 1;
        unfold_pixels(topo, anchor, radius, &mut self.scratch);
        if self.live + self.scratch.len() > self.capacity {
            // Full: serve this one from the scratch and leave the table as it is, so a
            // moving anchor never evicts the fixed ones it is drawn among.
            return &self.scratch;
        }
        let start = self.pool.len();
        self.pool.extend_from_slice(&self.scratch);
        if let Some(old) = self.map.insert(
            key,
            Entry { start: start as u32, len: self.scratch.len() as u32, radius },
        ) {
            // The anchor asked for a wider radius than last time; its old run stays in the
            // pool as dead space until `clear`, which is bounded by how far the wind can
            // push one stamp.
            self.live -= old.len as usize;
        }
        self.live += self.scratch.len();
        if self.pool.len() > 2 * self.live && self.pool.len() > COMPACT_FLOOR {
            self.compact();
            let e = &self.map[&key];
            return &self.pool[e.start as usize..e.start as usize + e.len as usize];
        }
        &self.pool[start..start + self.scratch.len()]
    }

    /// Drop the runs left behind by entries that grew. Rare: an anchor's radius settles
    /// within a few seconds of wind, after which nothing grows and nothing is abandoned.
    fn compact(&mut self) {
        let mut pool = Vec::with_capacity(self.live);
        for e in self.map.values_mut() {
            let (start, len) = (e.start as usize, e.len as usize);
            e.start = pool.len() as u32;
            pool.extend_from_slice(&self.pool[start..start + len]);
        }
        debug_assert_eq!(pool.len(), self.live);
        self.pool = pool;
    }
}

impl Default for Unfolds {
    fn default() -> Unfolds {
        Unfolds::new()
    }
}

/// Where one stamp's footprint comes from: the caller's own buffer, or a cache.
pub(crate) enum Source<'a> {
    Scratch(&'a mut Vec<PixelImage>),
    Cached(&'a mut Unfolds),
}

impl Source<'_> {
    #[inline]
    pub(crate) fn pixels(
        &mut self,
        topo: Topology,
        anchor: SurfacePoint,
        radius: f64,
    ) -> &[PixelImage] {
        match self {
            Source::Scratch(v) => {
                unfold_pixels(topo, anchor, radius, v);
                v
            }
            Source::Cached(u) => u.pixels(topo, anchor, radius),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_surface::{Face, GEOM_EPS};

    fn plain(topo: Topology, anchor: SurfacePoint, radius: f64) -> Vec<PixelImage> {
        let mut v = Vec::new();
        unfold_pixels(topo, anchor, radius, &mut v);
        v
    }

    /// A cached answer, narrowed to the query's own limit, is the uncached answer — pixel
    /// for pixel, field for field, in the same order — including after the entry has been
    /// widened by a larger query.
    #[test]
    fn a_cached_footprint_narrowed_to_its_radius_is_the_uncached_one() {
        let topos = [Topology::Cube, Topology::Ring { w: 320, h: 180 }];
        for topo in topos {
            let anchors: Vec<SurfacePoint> = match topo {
                Topology::Cube => vec![
                    SurfacePoint::new(Face::Front, 32.5, 32.5),
                    SurfacePoint::new(Face::Front, 63.5, 63.5),
                    SurfacePoint::new(Face::Top, 0.5, 0.5),
                    SurfacePoint::new(Face::Left, 63.5, 0.5),
                ],
                _ => vec![
                    SurfacePoint::new(Face::Front, 0.5, 90.5),
                    SurfacePoint::new(Face::Front, 319.5, 0.5),
                    SurfacePoint::new(Face::Front, 160.5, 179.5),
                ],
            };
            let mut cache = Unfolds::cached();
            // Ascending, descending and interleaved radii: the entry grows, then answers
            // smaller queries from the wider list.
            for radius in [5.0, 9.0, 3.0, 7.25, 9.0, 1.0, 6.5] {
                for &anchor in &anchors {
                    let want = plain(topo, anchor, radius);
                    let limit = radius + GEOM_EPS;
                    let got: Vec<PixelImage> = cache
                        .pixels(topo, anchor, radius)
                        .iter()
                        .filter(|p| p.distance <= limit)
                        .copied()
                        .collect();
                    assert_eq!(got.len(), want.len(), "{topo:?} {anchor:?} r={radius}");
                    for (g, w) in got.iter().zip(want.iter()) {
                        assert_eq!(g.face, w.face);
                        assert_eq!((g.x, g.y), (w.x, w.y));
                        assert_eq!(g.local.x.to_bits(), w.local.x.to_bits());
                        assert_eq!(g.local.y.to_bits(), w.local.y.to_bits());
                        assert_eq!(g.distance.to_bits(), w.distance.to_bits());
                        assert_eq!(g.path, w.path);
                    }
                }
            }
            let (hits, misses, entries, _) = cache.stats();
            assert_eq!(entries, anchors.len());
            assert!(hits > 0 && misses > 0, "hits {hits} misses {misses}");
        }
    }

    /// The pixels stay sorted by global row, which is what lets a band take a slice.
    #[test]
    fn a_cached_footprint_is_in_global_row_order() {
        let topo = Topology::Cube;
        let mut cache = Unfolds::cached();
        let anchor = SurfacePoint::new(Face::Top, 0.5, 63.5);
        let pixels = cache.pixels(topo, anchor, 9.0);
        let row = |p: &PixelImage| (topo.chart_index(p.face), p.y, p.x);
        assert!(pixels.windows(2).all(|w| row(&w[0]) < row(&w[1])), "not in row order");
    }

    /// A cache turned off is a scratch buffer, and a full one stops admitting anchors
    /// instead of growing.
    #[test]
    fn the_cache_is_optional_and_bounded() {
        let topo = Topology::Cube;
        let mut off = Unfolds::new();
        let anchor = SurfacePoint::new(Face::Front, 32.5, 32.5);
        let n = off.pixels(topo, anchor, 9.0).len();
        assert_eq!(off.stats(), (0, 0, 0, 0), "nothing cached when caching is off");

        let mut small = Unfolds::cached();
        small.capacity = n; // room for exactly one anchor
        assert_eq!(small.pixels(topo, anchor, 9.0).len(), n);
        let other = SurfacePoint::new(Face::Front, 20.5, 20.5);
        assert_eq!(small.pixels(topo, other, 9.0).len(), n, "still answered, just not stored");
        assert_eq!(small.stats().2, 1, "the second anchor was not admitted");
        small.clear();
        assert_eq!(small.stats(), (0, 0, 0, 0));
    }
}
