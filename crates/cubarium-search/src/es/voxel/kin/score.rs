//! **The kin-weighted score** (§10.3, §11.7): a body's own score, as today, plus `r` times
//! each relative's own base score over the same span.
//!
//! - A relative contributes its **own base** score, `(E_end − E_start + E_given)/body_ref`
//!   ([`MeteredBody::base`]), never its kin-augmented one, and not its curiosity term (the
//!   curiosity is a shaping term for the body that explores, not something its kin gain).
//! - A relative that died in the window is scored with `E_end = 0`, as every body is, so a
//!   killed calf's loss counts against its mother.
//! - A relative on the roster with no meter (born on the window's last tick) adds nothing.
//! - Every share 0 is today's score bit for bit: the relatives' term is not added at all.

use rustc_hash::FxHashMap;
use serde::Serialize;

use super::config::KinConfig;
use super::roster::{Relation, Roster};
use crate::es::voxel::live::window::MeteredBody;

/// One body's score and its parts (§11.7's readout: raw own, relative and combined).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct KinParts {
    /// Its own score, as today (energy, plus curiosity where configured).
    pub own: f64,
    /// `Σ r × relative's base`, over its relatives.
    pub relatives: f64,
    /// `own + relatives`: what the comparison ranks.
    pub combined: f64,
    /// How many relatives contributed.
    pub relatives_counted: u32,
}

/// Every metered body's kin-weighted score, by id, each under its own lineage's shares.
pub fn kin_scores(metered: &[MeteredBody], roster: &Roster, kin: &KinConfig) -> FxHashMap<u64, KinParts> {
    let base: FxHashMap<u64, f64> = metered.iter().map(|m| (m.body_id, m.base())).collect();
    metered
        .iter()
        .map(|m| {
            let shares = &kin.of(m.founder).shares;
            let own = m.score.score;
            if shares.is_zero() {
                return (m.body_id, KinParts { own, relatives: 0.0, combined: own, relatives_counted: 0 });
            }
            let mut relatives = 0.0;
            let mut counted = 0;
            for (other, relation) in roster.relatives(m.body_id) {
                let r = shares.of(*relation);
                if let (Some(b), true) = (base.get(other), r != 0.0) {
                    relatives += r * b;
                    counted += 1;
                }
            }
            (m.body_id, KinParts { own, relatives, combined: own + relatives, relatives_counted: counted })
        })
        .collect()
}

/// The relatives' term split by relation, for the readouts: `Σ r × base` per relation.
pub fn relatives_by_relation(
    id: u64,
    metered: &[MeteredBody],
    roster: &Roster,
    kin: &KinConfig,
) -> [(Relation, f64); 4] {
    let Some(me) = metered.iter().find(|m| m.body_id == id) else {
        return Relation::ALL.map(|r| (r, 0.0));
    };
    let shares = &kin.of(me.founder).shares;
    Relation::ALL.map(|rel| {
        let sum = roster
            .relatives(id)
            .iter()
            .filter(|(_, x)| *x == rel)
            .filter_map(|(o, _)| metered.iter().find(|m| m.body_id == *o))
            .map(|m| shares.of(rel) * m.base())
            .sum();
        (rel, sum)
    })
}
