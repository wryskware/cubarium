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
    /// Free slots as `(slot, next generation)`, kept sorted by descending slot so that
    /// `pop` yields the lowest free slot. The generation of a freed slot lives here
    /// because an empty entry carries no generation of its own.
    free: Vec<(u32, u32)>,
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
        self.live += 1;
        if let Some((slot, generation)) = self.free.pop() {
            debug_assert!(self.entries[slot as usize].is_none());
            self.entries[slot as usize] = Some((generation, value));
            return OrganismId { slot, generation };
        }
        let slot = self.entries.len() as u32;
        self.entries.push(Some((1, value)));
        OrganismId { slot, generation: 1 }
    }

    /// Remove by ID; `None` if the ID is stale or empty.
    pub fn remove(&mut self, id: OrganismId) -> Option<T> {
        let entry = self.entries.get_mut(id.slot as usize)?;
        match entry {
            Some((g, _)) if *g == id.generation => {}
            _ => return None,
        }
        let (generation, value) = entry.take().expect("checked above");
        self.live -= 1;
        // The next occupant gets one higher; generation 0 is never handed out, so a
        // wrapped counter skips it.
        let next = match generation.wrapping_add(1) {
            0 => 1,
            g => g,
        };
        // Keep the free list sorted by descending slot so `pop` returns the lowest slot.
        let at = self.free.partition_point(|&(s, _)| s > id.slot);
        self.free.insert(at, (id.slot, next));
        Some(value)
    }

    pub fn get(&self, id: OrganismId) -> Option<&T> {
        match self.entries.get(id.slot as usize)? {
            Some((g, v)) if *g == id.generation => Some(v),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: OrganismId) -> Option<&mut T> {
        match self.entries.get_mut(id.slot as usize)? {
            Some((g, v)) if *g == id.generation => Some(v),
            _ => None,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_appends_and_starts_at_generation_one() {
        let mut s: Slots<&str> = Slots::with_capacity(4);
        assert!(s.is_empty());
        let a = s.insert("a");
        let b = s.insert("b");
        assert_eq!(a, OrganismId { slot: 0, generation: 1 });
        assert_eq!(b, OrganismId { slot: 1, generation: 1 });
        assert_eq!(s.len(), 2);
        assert_eq!(s.slot_count(), 2);
        assert_eq!(s.get(a), Some(&"a"));
        assert_eq!(s.get(b), Some(&"b"));
    }

    #[test]
    fn remove_bumps_the_generation_and_reuses_the_lowest_slot() {
        let mut s: Slots<u32> = Slots::with_capacity(4);
        let a = s.insert(0);
        let b = s.insert(1);
        let c = s.insert(2);
        assert_eq!(s.remove(c), Some(2));
        assert_eq!(s.remove(a), Some(0));
        assert_eq!(s.len(), 1);

        // Lowest free slot first, with a generation one above the previous occupant.
        let d = s.insert(10);
        assert_eq!(d, OrganismId { slot: 0, generation: 2 });
        let e = s.insert(11);
        assert_eq!(e, OrganismId { slot: 2, generation: 2 });
        assert_eq!(s.slot_count(), 3);
        assert_eq!(s.len(), 3);
        assert_eq!(s.get(b), Some(&1));
    }

    #[test]
    fn stale_and_empty_ids_never_resolve() {
        let mut s: Slots<u32> = Slots::with_capacity(2);
        let a = s.insert(7);
        assert_eq!(s.remove(a), Some(7));
        assert_eq!(s.remove(a), None);
        assert_eq!(s.get(a), None);
        assert_eq!(s.get_mut(a), None);

        let b = s.insert(8);
        assert_eq!(b.slot, a.slot);
        assert_ne!(b.generation, a.generation);
        // The stale ID must not resolve to the new occupant.
        assert_eq!(s.get(a), None);
        assert_eq!(s.remove(a), None);
        assert_eq!(s.get(b), Some(&8));

        // Out-of-range slots.
        assert_eq!(s.get(OrganismId { slot: 99, generation: 1 }), None);
        assert_eq!(s.remove(OrganismId { slot: 99, generation: 1 }), None);
    }

    #[test]
    fn get_mut_mutates_in_place() {
        let mut s: Slots<u32> = Slots::with_capacity(1);
        let a = s.insert(1);
        *s.get_mut(a).unwrap() = 42;
        assert_eq!(s.get(a), Some(&42));
    }

    #[test]
    fn iteration_is_by_slot_order_and_skips_holes() {
        let mut s: Slots<u32> = Slots::with_capacity(4);
        let ids: Vec<_> = (0..4).map(|i| s.insert(i)).collect();
        s.remove(ids[1]).unwrap();
        let seen: Vec<_> = s.iter().map(|(id, v)| (id.slot, *v)).collect();
        assert_eq!(seen, vec![(0, 0), (2, 2), (3, 3)]);
        for (_, v) in s.iter_mut() {
            *v += 100;
        }
        assert_eq!(s.get(ids[0]), Some(&100));
    }

    #[test]
    fn insert_after_many_removals_keeps_generations_monotonic() {
        let mut s: Slots<u32> = Slots::with_capacity(1);
        let mut id = s.insert(0);
        for g in 2..10 {
            s.remove(id).unwrap();
            id = s.insert(g);
            assert_eq!(id, OrganismId { slot: 0, generation: g });
            assert_eq!(s.slot_count(), 1);
        }
    }
}
