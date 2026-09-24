//! A hand-rolled sparse set of cells: the **active set** a phase iterates instead of the
//! grid.
//!
//! Two storages, chosen when the set is built:
//!
//! - **Row masks** ([`CellSet::reset_columns`]), for a world at most
//!   [`crate::world::MASK_ROWS`] cells tall — every preset: one `u128` per column, bit `y`
//!   set when the cell at row `y` is a member. The water phases walk a column's rows
//!   straight off its word and keep it as they go, so a phase split across a pool keeps
//!   the set without touching anything another task owns
//!   (`design/handoffs/voxel-water-parallel-2026-09-24.md`). Membership is a bit test;
//!   there is no member list to keep, sort or swap-remove from.
//! - **A member list** ([`CellSet::reset`]), for taller worlds: one `Vec` of the members and
//!   one dense slot array over every cell, so `insert`, `remove` and `contains` are all
//!   constant time and the iteration is a linear walk of a small `Vec`. The order is the
//!   order of insertion with swap-removal, which is why every consumer either does
//!   per-cell work or reads the members back ascending ([`CellSet::sorted_into`])
//!   (`design/7_Research/voxel-tick-profile-2026-09-18.md`).
//!
//! **Not serialized.** A set is a cache of what the water arrays already say, so
//! [`World`](crate::World)'s copy is `#[serde(skip)]` and comes back from a snapshot empty
//! and `dirty`; the next step rebuilds it. Equality is therefore **set equality** and not
//! layout equality: two worlds holding the same water compare equal whatever order their
//! sets were built in.

use serde::{Deserialize, Serialize};

/// A set of cell indices with O(1) insert, remove and membership.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CellSet {
    /// List storage: the members, in insertion order with swap-removal.
    cells: Vec<usize>,
    /// List storage: per cell, its index in `cells` plus one, or zero when it is not a
    /// member.
    slot: Vec<u32>,
    /// Mask storage: per column (`index % plane`), the member rows, bit `index / plane`.
    cols: Vec<u128>,
    /// Columns per row when the set keeps row masks; zero for list storage.
    plane: usize,
    /// The number of cells the set covers.
    n: usize,
    /// Mask storage: how many members.
    count: usize,
    /// Set when something wrote the water arrays directly — a snapshot, generation, a
    /// resize — so the next step rebuilds instead of trusting the cache.
    dirty: bool,
}

impl Default for CellSet {
    fn default() -> CellSet {
        CellSet {
            cells: Vec::new(),
            slot: Vec::new(),
            cols: Vec::new(),
            plane: 0,
            n: 0,
            count: 0,
            dirty: true,
        }
    }
}

/// Set equality, not layout equality: the same members in any order are the same set, and a
/// **stale** set carries no information at all, so it equals anything. Two worlds holding
/// the same water are the same world whether or not either of them has built its cache
/// yet — which is what a snapshot round trip compares.
impl PartialEq for CellSet {
    fn eq(&self, other: &CellSet) -> bool {
        if self.dirty || other.dirty {
            return true;
        }
        if self.n != other.n || self.len() != other.len() {
            return false;
        }
        if self.plane != 0 && self.plane == other.plane {
            return self.cols == other.cols;
        }
        (0..self.n).all(|i| self.contains(i) == other.contains(i))
    }
}

impl CellSet {
    /// Whether this set has to be rebuilt before it is trusted: a fresh or decoded world,
    /// or one whose cell count no longer matches.
    pub(crate) fn needs_rebuild(&self, n: usize) -> bool {
        self.dirty || self.n != n
    }

    /// Start again over `n` cells as a member list. The caller fills it.
    pub(crate) fn reset(&mut self, n: usize) {
        self.cells.clear();
        self.slot.clear();
        self.slot.resize(n, 0);
        self.cols = Vec::new();
        self.plane = 0;
        self.n = n;
        self.count = 0;
        self.dirty = false;
    }

    /// Start again over `n` cells as per-column row masks of a world with `plane` columns
    /// (at most 128 rows). The caller fills it.
    pub(crate) fn reset_columns(&mut self, n: usize, plane: usize) {
        debug_assert!(plane > 0 && n.div_ceil(plane) <= 128);
        self.cells = Vec::new();
        self.slot = Vec::new();
        self.cols.clear();
        self.cols.resize(plane, 0);
        self.plane = plane;
        self.n = n;
        self.count = 0;
        self.dirty = false;
    }

    /// The per-column row masks, or `None` for a member list.
    pub(crate) fn columns(&self) -> Option<&[u128]> {
        (self.plane != 0).then_some(&self.cols[..])
    }

    /// The row masks for a phase to keep in place, and the member count it reports back
    /// through [`CellSet::adjust`]. `None` for a member list.
    pub(crate) fn columns_mut(&mut self) -> Option<&mut [u128]> {
        (self.plane != 0).then_some(&mut self.cols[..])
    }

    /// Account for `delta` members a phase added (or, negative, removed) through
    /// [`CellSet::columns_mut`].
    pub(crate) fn adjust(&mut self, delta: isize) {
        self.count = self
            .count
            .checked_add_signed(delta)
            .expect("a set cannot hold fewer than no members");
    }

    /// Row `y` of column `col`, for a set keeping row masks.
    #[inline]
    pub(crate) fn insert_at(&mut self, y: usize, col: usize) {
        let bit = 1u128 << y;
        if self.cols[col] & bit == 0 {
            self.cols[col] |= bit;
            self.count += 1;
        }
    }

    pub(crate) fn contains(&self, i: usize) -> bool {
        if i >= self.n {
            return false;
        }
        if self.plane != 0 {
            return (self.cols[i % self.plane] >> (i / self.plane)) & 1 != 0;
        }
        self.slot[i] != 0
    }

    pub(crate) fn insert(&mut self, i: usize) {
        if i >= self.n {
            return;
        }
        if self.plane != 0 {
            self.insert_at(i / self.plane, i % self.plane);
            return;
        }
        if self.slot[i] != 0 {
            return;
        }
        self.cells.push(i);
        self.slot[i] = self.cells.len() as u32;
    }

    pub(crate) fn remove(&mut self, i: usize) {
        if i >= self.n {
            return;
        }
        if self.plane != 0 {
            let (col, bit) = (i % self.plane, 1u128 << (i / self.plane));
            if self.cols[col] & bit != 0 {
                self.cols[col] &= !bit;
                self.count -= 1;
            }
            return;
        }
        if self.slot[i] == 0 {
            return;
        }
        let at = self.slot[i] as usize - 1;
        let last = self.cells.pop().expect("a member means a non-empty list");
        if at < self.cells.len() {
            self.cells[at] = last;
            self.slot[last] = at as u32 + 1;
        }
        self.slot[i] = 0;
    }

    /// Insert or remove by a predicate the caller has just made true or false.
    pub(crate) fn set(&mut self, i: usize, member: bool) {
        if member {
            self.insert(i);
        } else {
            self.remove(i);
        }
    }

    pub(crate) fn len(&self) -> usize {
        if self.plane != 0 {
            self.count
        } else {
            self.cells.len()
        }
    }

    /// The members, ascending. Allocates: for tests, the census and cold paths.
    pub(crate) fn members(&self) -> Vec<usize> {
        let (mut bits, mut out) = (Vec::new(), Vec::new());
        self.sorted_into(&mut bits, &mut out);
        out
    }

    /// The members in **ascending index order**, written into `out`, without a comparison
    /// sort. A member list sets one bit of `bits` per member and reads the words back in
    /// order: one pass over the members and one over `n / 64` words, against a sort's
    /// `m log m`. Row masks read row by row across the columns.
    ///
    /// `bits` is scratch the caller keeps between calls and is left all zero again.
    pub(crate) fn sorted_into(&self, bits: &mut Vec<u64>, out: &mut Vec<usize>) {
        out.clear();
        if self.plane != 0 {
            let rows = self.n.div_ceil(self.plane);
            let any = self.cols.iter().fold(0u128, |a, &m| a | m);
            for y in 0..rows {
                if (any >> y) & 1 == 0 {
                    continue;
                }
                let base = y * self.plane;
                for (col, &m) in self.cols.iter().enumerate() {
                    if (m >> y) & 1 != 0 {
                        out.push(base + col);
                    }
                }
            }
            return;
        }
        let words = self.slot.len().div_ceil(64);
        if bits.len() != words {
            bits.clear();
            bits.resize(words, 0);
        }
        for &i in &self.cells {
            bits[i >> 6] |= 1u64 << (i & 63);
        }
        for (w, word) in bits.iter_mut().enumerate() {
            let mut b = *word;
            if b == 0 {
                continue;
            }
            *word = 0;
            while b != 0 {
                out.push(w * 64 + b.trailing_zeros() as usize);
                b &= b - 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CellSet;

    /// The sorted read-back is the set, ascending, and leaves its scratch clean for the
    /// next call — including across the removals that scramble the insertion order.
    #[test]
    fn sorted_into_returns_the_members_ascending() {
        let mut set = CellSet::default();
        set.reset(200);
        for i in [150, 3, 64, 63, 199, 0, 128, 77] {
            set.insert(i);
        }
        set.remove(64);
        set.remove(0);
        let (mut bits, mut out) = (Vec::new(), Vec::new());
        set.sorted_into(&mut bits, &mut out);
        assert_eq!(out, [3, 63, 77, 128, 150, 199]);
        assert!(bits.iter().all(|&w| w == 0), "scratch left dirty");
        set.insert(64);
        set.sorted_into(&mut bits, &mut out);
        assert_eq!(out, [3, 63, 64, 77, 128, 150, 199]);
    }

    /// The row-mask storage holds the same set as the list: the same members ascending,
    /// the same count through inserts, repeats and removals, and it compares equal to a
    /// list holding them.
    #[test]
    fn row_masks_hold_the_same_set_as_the_list() {
        let (plane, n) = (7, 7 * 128);
        let (mut list, mut masks) = (CellSet::default(), CellSet::default());
        list.reset(n);
        masks.reset_columns(n, plane);
        for i in [0, 6, 7, 300, 127 * 7 + 6, 63 * 7 + 2, 64 * 7 + 2, 300, 5] {
            list.insert(i);
            masks.insert(i);
        }
        for i in [300, 5, 11] {
            list.remove(i);
            masks.remove(i);
        }
        assert_eq!(masks.len(), list.len());
        assert_eq!(masks.members(), list.members());
        assert_eq!(masks.members(), [0, 6, 7, 63 * 7 + 2, 64 * 7 + 2, 127 * 7 + 6]);
        assert!(masks.contains(127 * 7 + 6) && !masks.contains(300));
        assert_eq!(masks, list);
        masks.insert(8);
        assert_ne!(masks, list);
    }
}
