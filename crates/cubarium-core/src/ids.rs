//! Generation-checked organism slots.

use serde::{Deserialize, Serialize};

/// Stable organism identity: a reusable slot plus a generation that changes on reuse,
/// so a stale reference never resolves to a different organism.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OrganismId {
    pub slot: u32,
    pub generation: u32,
}

/// A slot arena with a free list. Iteration is by slot index; `insert` never reorders
/// live entries. Removal is deferred by the world to its commit phase.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slots<T> {
    entries: Vec<Option<(u32, T)>>,
    free: Vec<u32>,
    live: u32,
}

impl<T> Slots<T> {
    pub fn with_capacity(cap: usize) -> Slots<T> {
        Slots { entries: Vec::with_capacity(cap), free: Vec::new(), live: 0 }
    }

    pub fn len(&self) -> usize {
        self.live as usize
    }

    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Insert into the lowest free slot (or append), returning the new ID with a
    /// generation one higher than the slot's previous occupant (starting at 1).
    pub fn insert(&mut self, value: T) -> OrganismId {
        let _ = value;
        todo!("Slots::insert")
    }

    /// Remove by ID; `None` if the ID is stale or empty.
    pub fn remove(&mut self, id: OrganismId) -> Option<T> {
        let _ = id;
        todo!("Slots::remove")
    }

    pub fn get(&self, id: OrganismId) -> Option<&T> {
        let _ = id;
        todo!("Slots::get")
    }

    pub fn get_mut(&mut self, id: OrganismId) -> Option<&mut T> {
        let _ = id;
        todo!("Slots::get_mut")
    }

    /// Live entries in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (OrganismId, &T)> {
        self.entries.iter().enumerate().filter_map(|(i, e)| {
            e.as_ref().map(|(g, v)| (OrganismId { slot: i as u32, generation: *g }, v))
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (OrganismId, &mut T)> {
        self.entries.iter_mut().enumerate().filter_map(|(i, e)| {
            e.as_mut().map(|(g, v)| (OrganismId { slot: i as u32, generation: *g }, v))
        })
    }

    /// Number of slots ever allocated (live + free).
    pub fn slot_count(&self) -> usize {
        self.entries.len()
    }
}
