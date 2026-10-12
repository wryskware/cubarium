//! **The reward roster** (`design/behaviour-ai-2026-10-11.md` §11.7): who counts as whose
//! relative over one window or episode.
//!
//! - Fixed at the start from true relations: [`cubarium_voxel_fauna::Animal::mother`] for
//!   mother, calf and sibling, the pack record ([`cubarium_voxel_fauna::PackMember::pack`])
//!   for packmates. A relation that forms later (a body joining a pack mid-window) is not
//!   read.
//! - A newborn joins at birth ([`Roster::join`]): it relates to the members already on the
//!   roster by the same rule, and they to it.
//! - One relation per pair, the closest: mother and calf first, then sibling, then
//!   packmate ([`Relation::closeness`]).
//! - A mother is a body on the roster. A mother id naming no body on it (a mother dead
//!   before the window, or a founding mother id that names no animal at all) gives no
//!   mother relation; bodies sharing that id are still siblings, since the id is a recorded
//!   mother.
//! - Bodies driven by their heuristic are on the roster like any other: the roster reads
//!   bookkeeping, not who drives the body.
//!
//! The roster reads bookkeeping the animal never senses. That is allowed: only behaviour
//! must come from the senses; rewards may read the records (§10.3).

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

/// How one body is related to another, read from the first body's side: `Calf` means the
/// other body is the first one's calf.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    /// The other body is its calf (it is the other's recorded mother).
    Calf,
    /// The other body is its recorded mother.
    Mother,
    /// The two share a recorded mother id.
    Sibling,
    /// The two belong to the same pack.
    Packmate,
}

impl Relation {
    pub const ALL: [Relation; 4] = [
        Relation::Calf,
        Relation::Mother,
        Relation::Sibling,
        Relation::Packmate,
    ];

    /// Lower is closer: mother and calf, then sibling, then packmate.
    pub fn closeness(self) -> u8 {
        match self {
            Relation::Calf | Relation::Mother => 0,
            Relation::Sibling => 1,
            Relation::Packmate => 2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Relation::Calf => "calf",
            Relation::Mother => "mother",
            Relation::Sibling => "sibling",
            Relation::Packmate => "packmate",
        }
    }
}

/// The true relations one body carries: its id, its recorded mother and its pack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KinRecord {
    pub id: u64,
    pub mother: Option<u64>,
    pub pack: Option<u64>,
}

impl KinRecord {
    /// A living animal's record, as it stands now.
    pub fn of(a: &cubarium_voxel_fauna::Animal) -> KinRecord {
        KinRecord {
            id: a.id,
            mother: a.mother,
            pack: a.pack.map(|p| p.pack),
        }
    }
}

/// `b`'s relation to `a`, read from `a`'s side, the closest that holds; `None` for
/// strangers and for a body and itself. A mother relation needs the mother herself as `b`,
/// so a mother id naming no roster member only ever keys siblings.
fn relation(a: &KinRecord, b: &KinRecord) -> Option<Relation> {
    if a.id == b.id {
        return None;
    }
    if b.mother == Some(a.id) {
        return Some(Relation::Calf);
    }
    if a.mother == Some(b.id) {
        return Some(Relation::Mother);
    }
    if a.mother.is_some() && a.mother == b.mother {
        return Some(Relation::Sibling);
    }
    if a.pack.is_some() && a.pack == b.pack {
        return Some(Relation::Packmate);
    }
    None
}

/// One window's or episode's roster.
#[derive(Clone, Debug, Default)]
pub struct Roster {
    members: Vec<KinRecord>,
    index: FxHashMap<u64, usize>,
    /// Per member, its relatives: `(other id, the other's relation to it)`.
    relatives: Vec<Vec<(u64, Relation)>>,
}

impl Roster {
    /// The roster fixed from `records`, the bodies standing at the start.
    pub fn fixed(records: &[KinRecord]) -> Roster {
        let mut r = Roster::default();
        for rec in records {
            r.join(*rec);
        }
        r
    }

    /// Add `record` (a newborn, or a body standing at the start): it relates to every
    /// member already here by the closest true relation, and they to it. A body already on
    /// the roster is left as it is (relations are fixed once made).
    pub fn join(&mut self, record: KinRecord) {
        if self.index.contains_key(&record.id) {
            return;
        }
        let k = self.members.len();
        let mut mine = Vec::new();
        for (j, other) in self.members.iter().enumerate() {
            let Some(theirs_to_me) = relation(&record, other) else {
                continue;
            };
            // The reverse read always agrees (calf ↔ mother, sibling ↔ sibling, packmate ↔
            // packmate).
            let mine_to_them = relation(other, &record).expect("relations are symmetric");
            mine.push((other.id, theirs_to_me));
            self.relatives[j].push((record.id, mine_to_them));
        }
        self.members.push(record);
        self.index.insert(record.id, k);
        self.relatives.push(mine);
    }

    /// `id`'s relatives and how each is related to it; empty for a body not on the roster.
    pub fn relatives(&self, id: u64) -> &[(u64, Relation)] {
        self.index
            .get(&id)
            .map_or(&[], |&k| self.relatives[k].as_slice())
    }

    /// Whether `id` is on the roster.
    pub fn contains(&self, id: u64) -> bool {
        self.index.contains_key(&id)
    }

    /// Every member, in the order they joined.
    pub fn members(&self) -> &[KinRecord] {
        &self.members
    }

    /// The families: members joined by any relation, as connected groups of ids, each in
    /// joining order, groups ordered by their first member. A body with no relative is a
    /// family of one.
    pub fn families(&self) -> Vec<Vec<u64>> {
        let n = self.members.len();
        let mut group = vec![usize::MAX; n];
        let mut out: Vec<Vec<u64>> = Vec::new();
        for start in 0..n {
            if group[start] != usize::MAX {
                continue;
            }
            let g = out.len();
            let mut stack = vec![start];
            group[start] = g;
            let mut ids = Vec::new();
            while let Some(k) = stack.pop() {
                ids.push(k);
                for (other, _) in &self.relatives[k] {
                    let j = self.index[other];
                    if group[j] == usize::MAX {
                        group[j] = g;
                        stack.push(j);
                    }
                }
            }
            ids.sort_unstable();
            out.push(ids.into_iter().map(|k| self.members[k].id).collect());
        }
        out
    }
}
