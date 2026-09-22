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
/// Schema 6 brought the founders into the **live** world: the per-lineage bite and
/// assimilated-intake counters on the ledger, and [`crate::Fauna::policy_driven`] — the
/// record of which lineages were driven by a saved policy, which is the one thing about
/// a founder's controller that a loader cannot re-derive and must not guess.
/// Schema 7 added the ledger's **departure-cause** counters — `deaths_by_cause` and
/// `deaths_by_founder_cause` ([`crate::Departure`]) — which the 2026-09-20 founder
/// autopsy needs to tell a starvation from a drowning. Pure instrumentation, and still a
/// schema bump: the serialized ledger grew two arrays and a world saved under schema 6
/// has no honest value for them.
/// Schema 8 added [`crate::Fauna::policy_digest`] beside `policy_driven`: the
/// `weights_fnv1a` of the centre each lineage is recorded as running, or `0` for a
/// heuristic. `policy_driven` alone could tell a trained lineage from a heuristic one,
/// but not one trained centre from another, so `install_founder_controllers` could not
/// refuse a policy-driven world handed a *different* centre for the same lineage — only
/// a bare demotion. A world saved under schema 7 has no honest digest to fill it with.
/// Schema 9 is **reproduction** (`design/handoffs/voxel-reproduction-2026-09-21.md`):
/// [`crate::ReproductionState`] on every animal — the surplus hold, the refractory and
/// the gestation [`crate::Escrow`] — the [`crate::Clutch`] records standing in the
/// world, and the ledger's egg, hatch and gestation counters with the fourth
/// respiration split. A world saved under schema 8 has no honest value for any of them:
/// its bodies were breeding under the placeholder rule that this one replaces, so there
/// is nothing to carry across and nothing to synthesize.
/// Schema 10 is the **founder step rule**
/// (`design/handoffs/voxel-founder-step-2026-09-22.md`):
/// [`crate::FounderPhysiology`] gained `climb_m`, which is serialised inside the
/// config, and `Animal::site.y` stopped being an invariant of a founder's life — a
/// body now changes its standing layer, so the same `site` in a schema-9 world means
/// something a schema-10 loader cannot assume about how it got there. There is nothing
/// to synthesize either way.
/// Schema 11 is **bodies in metres**
/// (`design/handoffs/voxel-body-anchors-2026-09-22.md`):
/// [`crate::FounderPhysiology`] gained the adult dimensions and the four anchor
/// fractions, seven more serialised fields inside the config. A schema-10 world's
/// animals were the wrong size, ate out of a whole-voxel layer range and looked out of
/// an eye anchored in cells; there is no honest value to carry across, and none is
/// synthesized.
/// Postcard is not self-describing, so older worlds are **refused**, not migrated
/// (`always-fresh-never-migrate`): start a fresh world.
pub const SCHEMA: u32 = 11;

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
/// A [`crate::Clutch`] is held to the same three conditions as a body — finite
/// nonnegative currencies, at least one egg, and a laying tick that is not in the
/// layer's future — because it is a paid package the stored totals count and an
/// incubation counted from a future tick would never hatch.
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
        let r = a.reproduction;
        if let Some(e) = r.escrow {
            for (field, value) in [
                ("escrowed organic", e.organic),
                ("escrowed mineral", e.mineral),
                ("escrowed energy", e.energy),
            ] {
                if !value.is_finite() || value < 0.0 {
                    return Err(format!(
                        "animal #{}'s {field} is {value}, not a finite nonnegative stock",
                        a.id
                    ));
                }
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
    // A clutch is a paid package the layer's stored totals count, so the same three
    // conditions apply to it as to a body: finite nonnegative currencies, and at least
    // one egg — a clutch of nothing is matter with nowhere to go.
    for c in v.clutches {
        if c.count == 0 {
            return Err(format!(
                "the clutch at {:?} holds no eggs: a clutch of nothing cannot hatch",
                c.site
            ));
        }
        for (field, value) in [
            ("organic", c.organic),
            ("mineral", c.mineral),
            ("energy", c.energy),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(format!(
                    "the clutch at {:?} has {field} {value}, not a finite nonnegative stock",
                    c.site
                ));
            }
        }
        if c.laid_tick > v.tick {
            return Err(format!(
                "the clutch at {:?} was laid on tick {} and the layer is on tick {}: a \
                 clutch cannot be laid in the future",
                c.site, c.laid_tick, v.tick
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
            reproduction: crate::ReproductionState::default(),
            body,
            reserve: 0.5 * body,
            mineral: 0.05 * body,
            energy: 2.0 * body,
            age_ticks: 0,
            state: State::Resting,
        }
    }

    /// **Schema 6 carries the founder bodies and refuses an older world.**
    ///
    /// The bodies: lineage marker, pose, held actions and prior-interval feedback all
    /// come back as they went in. The lineage's `policy_driven` flag comes back too,
    /// which is the whole reason the schema moved — a loader that could not tell a
    /// trained lineage from a heuristic one would put the heuristic back in silence.
    /// The controllers themselves do **not** come back: a mind is session state, and a
    /// loaded layer's founders rest until a driver installs controllers again.
    ///
    /// The refusal is checked by rewriting the leading schema tag, which is one postcard
    /// varint byte. No hash is pinned and no old bytes are kept in the tree.
    #[test]
    fn the_current_schema_round_trips_a_founder_world_and_refuses_an_older_one() {
        let mut body = animal(0, 0.02);
        body.founder = Some(crate::Founder::Browser);
        body.founder_state.held = crate::Actions {
            forward: 0.75,
            turn: -0.5,
            feed: 1.0,
        };
        body.founder_state.feedback.intake = 1e-4;
        body.pose = crate::Pose {
            heading_rad: 1.25,
            ..body.pose
        };

        let mut fauna = layer(vec![body], 1);
        fauna.set_policy_driven(crate::Founder::Browser, true);
        fauna.set_policy_digest(crate::Founder::Browser, 0xdead_beef_cafe_1234);
        fauna.ledger.bites_by_founder[crate::Founder::Browser.index()] = 3;
        fauna.ledger.assimilated_by_founder[crate::Founder::Browser.index()] = 2.5e-4;
        // A mind installed before the save is not part of the save.
        assert!(fauna.set_controller(
            0,
            Box::new(crate::Scripted::new(vec![body.founder_state.held]))
        ));

        let bytes = encode(&fauna);
        let mut back = decode(&bytes).expect("a founder world round-trips");
        assert_eq!(back.view().animals, fauna.view().animals);
        assert_eq!(back.view().ledger, fauna.view().ledger);
        assert!(back.policy_driven(crate::Founder::Browser));
        assert!(!back.policy_driven(crate::Founder::Blind));
        assert_eq!(
            back.policy_digest(crate::Founder::Browser),
            0xdead_beef_cafe_1234
        );
        assert_eq!(back.policy_digest(crate::Founder::Blind), 0);
        assert!(
            back.take_controller(0).is_none(),
            "a loaded layer's founders rest until a driver installs controllers again"
        );

        let mut older = bytes.clone();
        assert_eq!(older[0], SCHEMA as u8, "the tag is the leading varint byte");
        older[0] = SCHEMA as u8 - 1;
        let err = format!("{:#}", decode(&older).expect_err("an older world"));
        assert!(err.contains("start a fresh world"), "{err}");
        assert!(err.contains(&format!("is not {SCHEMA}")), "{err}");
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
