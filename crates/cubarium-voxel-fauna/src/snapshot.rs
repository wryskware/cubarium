//! Save/load as postcard bytes behind a schema tag, the way
//! [`cubarium_voxel::snapshot`] does it. A different tag is refused; there is no
//! migration, ever.
//!
//! This is the standing rule and not a choice of this round: `always-fresh-never-migrate`
//! says a new schema refuses old worlds and nothing is synthesized or re-anchored. A saved
//! animal layer whose [`crate::FaunaConfig`] no animal can live under is refused too, for
//! the reason [`crate::Fauna::new`] panics on one: it is not a runtime condition.
//!
//! **What this does not do yet, and why.** The round's brief asks for the fauna to be
//! serialized *beside the flora*, and there is nothing to sit beside: the plant layer has
//! no snapshot at all. `cubarium_voxel_flora::Flora` does not implement `Serialize`, and
//! the host's `w`/`l` stdin commands write and read `World::save()` alone
//! (`crates/cubarium/src/voxel/mod.rs`), so a host-side envelope carrying the world, the
//! plants and the animals cannot be written without adding the derive to the flora crate —
//! which is another package's file this round. This module is therefore the animal layer's
//! own always-fresh snapshot, ready for that envelope, and the host wiring is reported as
//! owed.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::Fauna;

/// Schema 3: schema 2 added the per-plant ledger arrays (round 5b/5c merge). Schema 3
/// added the phase-one body contract to [`crate::Animal`] — a continuous [`crate::Pose`]
/// and the optional [`crate::Founder`] lineage marker — and the [`crate::Fauna`] birth
/// switch. Schema 4 added the P1-B founder state to [`crate::Animal`] — the held
/// controller actions and the prior-interval feedback — and the founders' own
/// physiology table to [`crate::FaunaConfig`]. Schema 5 added the respiration split to
/// the ledger and the per-interval motor-respiration counter to the feedback (P1-C).
/// Postcard is not self-describing, so older worlds are **refused**, not migrated
/// (`always-fresh-never-migrate`): start a fresh world.
pub const SCHEMA: u32 = 5;

#[derive(Serialize, Deserialize)]
struct Envelope {
    schema: u32,
    fauna: Fauna,
}

pub fn encode(fauna: &Fauna) -> Vec<u8> {
    postcard::to_stdvec(&Envelope {
        schema: SCHEMA,
        fauna: fauna.clone(),
    })
    .expect("a Fauna always serializes")
}

pub fn decode(bytes: &[u8]) -> anyhow::Result<Fauna> {
    let tag: u32 = postcard::from_bytes(bytes).context("not a voxel fauna snapshot")?;
    if tag != SCHEMA {
        bail!("voxel fauna snapshot schema {tag} is not {SCHEMA}; start a fresh world");
    }
    let env: Envelope = postcard::from_bytes(bytes).context("corrupt voxel fauna snapshot")?;
    if let Err(e) = env.fauna.config().validate() {
        bail!("invalid voxel fauna snapshot — {e}");
    }
    if let Err(e) = validate(&env.fauna) {
        bail!("invalid voxel fauna snapshot — {e}");
    }
    Ok(env.fauna)
}

/// What a loaded layer has to satisfy beyond its schema tag and its config: the
/// **state**, and not only the configuration (Astra R9.6).
///
/// Three things, each of which an invariant of this layer depends on rather than a taste:
/// every stock is a finite nonnegative number, because a `NaN` body is a `NaN` in the
/// ledger within one step and a negative one is matter that does not exist; the ids are
/// **sorted and unique**, because every pass over `animals` is a binary search over that
/// order; and [`crate::FaunaLedger::births`] — the counter the next id comes from — is
/// **above every id present**, because an id is never reused and `Fauna::insert` treats a
/// collision as unreachable. A phase-one body also has to carry a finite [`crate::Pose`],
/// because the founder's movement and sensing read it and a `NaN` position is a `NaN` in
/// the world within a tick. And a founder's held actions are bounded actions with finite
/// feedback — the loader refuses anything that would drive a body outside the manifest's
/// caps or feed the observation a non-finite channel.
///
/// Postcard bytes are not the only way in: a hand-built or hand-edited snapshot is exactly
/// what this is for, and a round trip of a live layer cannot fail it.
fn validate(fauna: &Fauna) -> Result<(), String> {
    let v = fauna.view();
    let mut last: Option<u64> = None;
    for a in v.animals {
        for (field, value) in [
            ("body", a.body),
            ("reserve", a.reserve),
            ("mineral", a.mineral),
            ("energy", a.energy),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(format!(
                    "animal #{}'s {field} is {value}, not a finite nonnegative stock",
                    a.id
                ));
            }
        }
        if !a.pose.is_finite() {
            return Err(format!("animal #{}'s pose is not finite", a.id));
        }
        if a.founder.is_some() {
            let held = a.founder_state.held;
            for (field, value) in [("forward", held.forward), ("feed", held.feed)] {
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(format!(
                        "animal #{}'s held {field} is {value}, not a bounded action",
                        a.id
                    ));
                }
            }
            if !held.turn.is_finite() || !(-1.0..=1.0).contains(&held.turn) {
                return Err(format!(
                    "animal #{}'s held turn is {}, not a bounded action",
                    a.id, held.turn
                ));
            }
            let fb = a.founder_state.feedback;
            for (field, value) in [
                ("intake", fb.intake),
                ("structural_loss", fb.structural_loss),
                ("attempted equivalent displacement", fb.attempted_equivalent),
                ("delivered equivalent displacement", fb.delivered_equivalent),
                ("delivered forward", fb.delivered_forward),
                ("motor respiration", fb.motor_respiration),
            ] {
                if !value.is_finite() || value < 0.0 {
                    return Err(format!(
                        "animal #{}'s {field} feedback is {value}, not finite nonnegative",
                        a.id
                    ));
                }
            }
            if !fb.delivered_turn.is_finite() {
                return Err(format!(
                    "animal #{}'s delivered turn feedback is not finite",
                    a.id
                ));
            }
        }
        if let Some(prev) = last {
            if a.id <= prev {
                return Err(format!(
                    "animal ids are not sorted and unique: #{} comes after #{prev}",
                    a.id
                ));
            }
        }
        last = Some(a.id);
    }
    if let Some(max) = last {
        if v.ledger.births <= max {
            return Err(format!(
                "the next id is {} and animal #{max} already holds it: an id is never reused",
                v.ledger.births
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Animal, FaunaConfig, FaunaLedger, Site, Species, State};

    /// A layer built by hand, which is the only way to write a snapshot this module has to
    /// refuse: a live layer's own bytes cannot fail these checks.
    fn layer(animals: Vec<Animal>, births: u64) -> Fauna {
        let mut fauna = Fauna::new(FaunaConfig::default());
        fauna.ledger = FaunaLedger {
            births,
            ..FaunaLedger::default()
        };
        fauna.animals = animals;
        fauna
    }

    fn animal(id: u64, body: f64) -> Animal {
        Animal {
            id,
            species: Species::Frondgrazer,
            site: Site { x: 1, y: 2, z: 0 },
            pose: crate::Pose::at_site(Site { x: 1, y: 2, z: 0 }, 1.0),
            founder: None,
            founder_state: crate::FounderState::default(),
            body,
            reserve: 0.5 * body,
            mineral: 0.05 * body,
            energy: 2.0 * body,
            age_ticks: 0,
            state: State::Resting,
        }
    }

    /// A founder's held actions are bounded actions and its feedback is finite: the
    /// loader refuses a hand-built layer that would move a body outside the manifest's
    /// caps or feed the observation a non-finite channel.
    #[test]
    fn the_loader_refuses_a_founder_state_it_could_not_have_produced() {
        let mut held = animal(0, 0.02);
        held.founder = Some(crate::Founder::Blind);
        held.founder_state.held.turn = 7.0;
        let err = format!(
            "{:#}",
            decode(&encode(&layer(vec![held], 1))).expect_err("held turn 7")
        );
        assert!(err.contains("held turn"), "{err}");

        let mut feed = animal(0, 0.02);
        feed.founder = Some(crate::Founder::Blind);
        feed.founder_state.held.feed = -0.5;
        let err = format!(
            "{:#}",
            decode(&encode(&layer(vec![feed], 1))).expect_err("held feed -0.5")
        );
        assert!(err.contains("held feed"), "{err}");

        let mut nan = animal(0, 0.02);
        nan.founder = Some(crate::Founder::Blind);
        nan.founder_state.feedback.structural_loss = f64::NAN;
        let err = format!(
            "{:#}",
            decode(&encode(&layer(vec![nan], 1))).expect_err("a NaN channel")
        );
        assert!(err.contains("structural_loss"), "{err}");

        // And a heuristic body's default founder state never trips any of it.
        assert!(decode(&encode(&layer(vec![animal(0, 0.02)], 1))).is_ok());
    }

    /// **R9.6: the loader validates the state.** Two well-formed animals round-trip; a
    /// `NaN` or negative stock, an unsorted or repeated id, and a next-id counter that
    /// would reuse an id are each refused with the reason named.
    #[test]
    fn the_loader_refuses_a_snapshot_whose_state_cannot_be_true() {
        let good = layer(vec![animal(0, 0.02), animal(3, 0.04)], 4);
        let back = decode(&encode(&good)).expect("a well-formed layer");
        assert_eq!(back.view().animals, good.view().animals);

        let cases: [(&str, Fauna); 6] = [
            ("body", layer(vec![animal(0, f64::NAN)], 1)),
            ("reserve", {
                let mut a = animal(0, 0.02);
                a.reserve = -1e-9;
                layer(vec![a], 1)
            }),
            ("energy", {
                let mut a = animal(0, 0.02);
                a.energy = f64::INFINITY;
                layer(vec![a], 1)
            }),
            ("pose", {
                let mut a = animal(0, 0.02);
                a.pose = crate::Pose {
                    x: f64::NAN,
                    ..a.pose
                };
                layer(vec![a], 1)
            }),
            (
                "sorted and unique",
                layer(vec![animal(3, 0.02), animal(1, 0.02)], 4),
            ),
            (
                "never reused",
                layer(vec![animal(0, 0.02), animal(3, 0.02)], 3),
            ),
        ];
        for (what, fauna) in cases {
            let err = format!(
                "{:#}",
                decode(&encode(&fauna)).expect_err("a refusable layer")
            );
            assert!(err.contains(what), "{what}: {err}");
        }

        // A duplicate id is the same refusal as an unsorted one, and an empty layer with a
        // counter of its own is fine: nothing holds an id.
        let err = format!(
            "{:#}",
            decode(&encode(&layer(vec![animal(2, 0.02), animal(2, 0.03)], 3)))
                .expect_err("two animals cannot share an id")
        );
        assert!(err.contains("sorted and unique"), "{err}");
        assert!(decode(&encode(&layer(Vec::new(), 7))).is_ok());
    }
}
