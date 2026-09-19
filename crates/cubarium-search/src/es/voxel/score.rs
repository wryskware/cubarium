//! The capability-training score of the tests plan §2 and its separately reported
//! components.
//!
//! > rank candidates by settled assimilated intake minus paid motor organic loss, divided
//! > by a fixed founder reference body mass, plus a small survival term
//! > `0.25 × survived_fraction`
//!
//! so one episode scores
//!
//! ```text
//! score = (intake_organic − motor_organic) / body_reference
//!       + 0.25 · survived_fraction
//! ```
//!
//! with the three components — normalized intake, normalized motor loss, survival —
//! reported separately ([`ScoreComponents`]). It is a *capability-training* score, not the
//! world's reproductive fitness. **Resource positions, path distance and the food stock
//! are not observation inputs and not reward terms**: the score reads only what settled
//! into the placed animal's own tissue and what its own motion charged, which is exactly
//! why a stationary/no-intake control scores its survival term and nothing else.
//!
//! # What "settled" and "paid motor" are measured from
//!
//! Both are measured from the fauna layer's published state — the placed animal and the
//! boundary ledger — read-only. With exactly one placed animal (births disabled), the
//! fauna layer's ledger *is* that animal's ledger plus its boundary terms, so per episode:
//!
//! - **settled assimilated intake** is the organic matter that actually entered the body
//!   and stayed or was spent, i.e. `Δorganic + maintenance_paid + motor_paid + corpse`
//!   (the conservation reading of the fauna's own assimilation: `Δorganic` is what the
//!   bite built minus what upkeep and motion respired, and the corpse carries what a dead
//!   body still held). Gross bite organic is **not** the measure — the undigested fraction
//!   never settled.
//! - **maintenance paid** and **paid motor organic loss** are read from the fauna
//!   ledger's separate `respired_maintenance_out` and `respired_motor_out` counters.
//!   `respired_digestion_out` is deliberately excluded from settled intake: it is the
//!   gross bite share that never entered the animal.

/// The survival term's fixed weight: `0.25 × survived_fraction`, as the tests plan fixes.
pub const SURVIVAL_WEIGHT: f64 = 0.25;

use serde::{Deserialize, Serialize};

/// The score and its components for one episode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScoreComponents {
    /// Settled assimilated intake, divided by the founder's fixed reference body mass.
    pub intake_normalized: f64,
    /// Paid motor organic loss, divided by the same reference.
    pub motor_loss_normalized: f64,
    /// The survival term already weighted: `0.25 × survived_fraction`.
    pub survival_term: f64,
    /// The scalar the ES ranks by: intake − motor loss + the survival term.
    pub score: f64,
}

impl ScoreComponents {
    /// The score of one episode from its raw measurements.
    ///
    /// `intake_organic` is the settled assimilated intake the driver's epilogue measured
    /// (`Δorganic + maintenance + motor + corpse`, see the module docs), `motor_organic`
    /// the respiration the founder's motion charged, both in organic-matter units
    /// against the founder's fixed `body_reference`. A non-finite measurement or a
    /// fraction outside `[0, 1]` is an experiment error upstream; here every input is
    /// asserted finite so a broken sensor can never silently produce a ranking.
    pub fn of(
        intake_organic: f64,
        motor_organic: f64,
        survived_fraction: f64,
        body_reference: f64,
    ) -> ScoreComponents {
        assert!(intake_organic.is_finite() && motor_organic.is_finite());
        assert!(body_reference.is_finite() && body_reference > 0.0);
        assert!(survived_fraction.is_finite() && (0.0..=1.0).contains(&survived_fraction));
        let intake_normalized = (intake_organic / body_reference).max(0.0);
        let motor_loss_normalized = (motor_organic / body_reference).max(0.0);
        let survival_term = SURVIVAL_WEIGHT * survived_fraction;
        ScoreComponents {
            intake_normalized,
            motor_loss_normalized,
            survival_term,
            score: intake_normalized - motor_loss_normalized + survival_term,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REF: f64 = 0.0125;

    /// A stationary no-intake animal that survives the horizon scores exactly its
    /// survival term: no free intake, no scoring artifact.
    #[test]
    fn a_surviving_non_feeder_scores_only_its_survival_term() {
        // start == end organic, no maintenance beyond what the identity already charged,
        // no motor yet.
        let s = ScoreComponents::of(0.0, 0.0, 1.0, REF);
        assert_eq!(s.intake_normalized, 0.0);
        assert_eq!(s.motor_loss_normalized, 0.0);
        assert!((s.score - SURVIVAL_WEIGHT).abs() < 1e-12);
    }

    /// Intake pays: settled intake of one reference mass scores 1.0 above the survival
    /// term; motor loss subtracts at the same rate it costs; the components stay
    /// separately reported.
    #[test]
    fn intake_counts_and_motor_loss_subtracts() {
        let fed = ScoreComponents::of(REF, 0.0, 1.0, REF);
        assert!((fed.intake_normalized - 1.0).abs() < 1e-12, "{fed:?}");
        assert!((fed.score - (1.0 + SURVIVAL_WEIGHT)).abs() < 1e-12);
        // The same episode that also paid a reference mass of motor organic.
        let moving = ScoreComponents::of(REF, REF, 1.0, REF);
        assert!((moving.motor_loss_normalized - 1.0).abs() < 1e-12);
        assert!((moving.score - (fed.score - 1.0)).abs() < 1e-12);

        // A dead animal at half the horizon keeps only half the survival term.
        let dead = ScoreComponents::of(0.0, 0.0, 0.5, REF);
        assert!((dead.survival_term - 0.125).abs() < 1e-12);
        assert!(dead.score < fed.score);
    }

    /// A non-finite or out-of-range input is an assertion, never a score.
    #[test]
    #[should_panic(expected = "survived_fraction")]
    fn a_fraction_out_of_range_is_an_experiment_error() {
        ScoreComponents::of(0.0, 0.0, 1.5, REF);
    }
}
