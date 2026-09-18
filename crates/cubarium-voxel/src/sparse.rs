//! A hand-rolled sparse set of cells: the **active set** a phase iterates instead of the
//! grid.
//!
//! One `Vec` of the cells that are in the set and one dense slot array over every cell, so
//! `insert`, `remove` and `contains` are all constant time and the iteration is a linear
//! walk of a small `Vec`. No hashing, no unsafe, and **no iteration order that depends on
//! storage history** in the sense that matters here: the order is the order of insertion
//! with swap-removal, which is why every consumer of a set either does per-cell work or
//! accumulates into a per-cell buffer rather than into a shared scalar
//! (`design/7_Research/voxel-tick-profile-2026-09-18.md`).
//!
//! This is the shape the profile's step 3 asked for, chosen over an ECS crate for the
//! reasons the note records: the core keeps `#![forbid(unsafe_code)]` and pulls no
//! dependency tail for it.
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
    /// The members, in insertion order with swap-removal.
    cells: Vec<usize>,
    /// Per cell: its index in `cells` plus one, or zero when it is not a member.
    slot: Vec<u32>,
    /// Set when something wrote the water arrays directly — a snapshot, generation, a
    /// resize — so the next step rebuilds instead of trusting the cache.
    dirty: bool,
}

impl Default for CellSet {
    fn default() -> CellSet {
        CellSet {
            cells: Vec::new(),
            slot: Vec::new(),
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
        self.cells.len() == other.cells.len()
            && self.slot.len() == other.slot.len()
            && self
                .slot
                .iter()
                .zip(other.slot.iter())
                .all(|(a, b)| (*a == 0) == (*b == 0))
    }
}

impl CellSet {
    /// Whether this set has to be rebuilt before it is trusted: a fresh or decoded world,
    /// or one whose cell count no longer matches.
    pub(crate) fn needs_rebuild(&self, n: usize) -> bool {
        self.dirty || self.slot.len() != n
    }

    /// Start again over `n` cells. The caller fills it.
    pub(crate) fn reset(&mut self, n: usize) {
        self.cells.clear();
        self.slot.clear();
        self.slot.resize(n, 0);
        self.dirty = false;
    }

    pub(crate) fn insert(&mut self, i: usize) {
        if self.slot.len() <= i || self.slot[i] != 0 {
            return;
        }
        self.cells.push(i);
        self.slot[i] = self.cells.len() as u32;
    }

    pub(crate) fn remove(&mut self, i: usize) {
        if self.slot.len() <= i || self.slot[i] == 0 {
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

    pub(crate) fn cells(&self) -> &[usize] {
        &self.cells
    }

    pub(crate) fn len(&self) -> usize {
        self.cells.len()
    }
}
