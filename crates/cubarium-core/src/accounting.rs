//! Compensated accumulation for the persisted cumulative energy ledgers.
//!
//! `design/7_Research/astra-long-run-energy-diagnostic-2026-09-13.md` establishes that
//! `heat_out_total` is the sum of a great many small payments, so after twelve hours of
//! simulated time its naive `+=` has thrown away enough low-order bits to fail the strict
//! cumulative energy audit on its own (measured: a persisted-minus-windowed heat discrepancy
//! of `2.0344e-5` against a `1.9564e-5` limit, while independently windowed sums of the same
//! flows stay near `1e-10`).
//!
//! The correction here is deliberately *additive*, never a replacement:
//!
//! - every existing raw addition happens exactly as before, in the same order, producing the
//!   same `f64`, so saved worlds, the ecology projection hashes and the legacy diagnostics
//!   are untouched;
//! - alongside each raw addition the bits it dropped are accumulated into a persisted signed
//!   [`EnergyCorrection`] component (Neumaier / Kahan-Babuška-Neumaier);
//! - the **corrected** cumulative total is `raw + correction`, and a precise delta over an
//!   interval is `(raw_end − raw_start) + (correction_end − correction_start)`.
//!
//! `raw + correction` is the persisted representation: the correction is checkpointed with
//! the world (schema 9) rather than recomputed by an observer, because compensating only an
//! external observer would be discarded by the next restart and is not this fix.
//!
//! **The correction begins at migration.** A schema 7 or schema 8 payload opens with zero
//! corrections, and that zero is not a claim that its raw totals are exact: the bits those
//! additions already lost are not recoverable from a snapshot. Only flow booked *after* the
//! migration is compensated.
//!
//! Corrections are signed and may be negative. They are validated (finite corrections,
//! finite and nonnegative combined totals) and never silently clamped or reset: an invalid
//! accounting state fails the decode like any other out-of-range value.

use serde::{Deserialize, Serialize};

/// Add `amount` to a persisted raw total, booking the rounding error into `correction`.
///
/// `*raw` is updated with exactly the `s + amount` the pre-correction build computed — that
/// is the compatibility requirement, not an implementation detail — and the error term
/// (which is exact whenever the larger operand is known, Dekker's two-sum) is added to
/// `*correction`:
///
/// ```text
/// next = s + amount
/// c   += |s| >= |amount| ? (s - next) + amount : (amount - next) + s
/// ```
///
/// The corrected total is `*raw + *correction`. A non-finite `amount` poisons both the raw
/// total and the correction, and is caught by validation rather than absorbed here.
#[inline]
pub fn accumulate(raw: &mut f64, correction: &mut f64, amount: f64) {
    let s = *raw;
    let next = s + amount;
    *correction += if s.abs() >= amount.abs() { (s - next) + amount } else { (amount - next) + s };
    *raw = next;
}

/// One compensated cumulative ledger read at an instant: the raw counter every build has
/// always written, plus the signed correction holding the bits its additions dropped.
///
/// Read one of these before an interval and one after, then use [`Ledger::since`]; reading
/// `total()` at both ends and subtracting would round the two large totals first and throw
/// the precision away again.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ledger {
    /// The raw persisted counter (`light_in_total` / `heat_out_total`). An explicit
    /// diagnostic and the legacy wire value — *not* a fully accurate cumulative total.
    pub raw: f64,
    /// The persisted signed correction for that counter, zero for a world that has just
    /// migrated from schema 7 or 8.
    pub correction: f64,
}

impl Ledger {
    /// The corrected cumulative total, `raw + correction`.
    pub fn total(self) -> f64 {
        self.raw + self.correction
    }

    /// The precise flow booked since `opening`:
    /// `(raw_end − raw_start) + (correction_end − correction_start)`.
    pub fn since(self, opening: Ledger) -> f64 {
        (self.raw - opening.raw) + (self.correction - opening.correction)
    }

    /// Finite correction, and a finite nonnegative corrected total: both ledgers count a
    /// one-directional flow, so neither may run backwards once compensated.
    pub fn validate(self, name: &str) -> Result<(), String> {
        if !self.correction.is_finite() {
            return Err(format!("{name} correction is not finite"));
        }
        let total = self.total();
        if !total.is_finite() {
            return Err(format!("corrected {name} is not finite"));
        }
        if total < 0.0 {
            return Err(format!("corrected {name} is negative: {total}"));
        }
        Ok(())
    }
}

/// The persisted signed corrections for the cumulative energy ledgers: the accounting
/// extension schema 9 appends to `WorldState`, and the only field it adds.
///
/// Zero for a migrated world (see the module docs). Nested wire types are unchanged; this
/// struct is two `f64` appended after `care`, so the schema 8 projection of a current state
/// is still the byte-exact payload a schema 8 build would have written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EnergyCorrection {
    /// Correction for `WorldState::light_in_total`.
    pub light_in: f64,
    /// Correction for `WorldState::heat_out_total`.
    pub heat_out: f64,
}

/// Both compensated energy ledgers read at one instant: what an audit holds as its opening
/// reading and differences against later.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EnergyLedgers {
    pub light_in: Ledger,
    pub heat_out: Ledger,
}

impl EnergyLedgers {
    /// Corrected `light_in − heat_out` since world creation. Only meaningful as an absolute
    /// number for a world that was never migrated; use [`EnergyLedgers::net_since`] over an
    /// interval otherwise.
    pub fn net(self) -> f64 {
        self.light_in.total() - self.heat_out.total()
    }

    /// The corrected net energy booked since `opening`, each ledger differenced precisely
    /// before the subtraction.
    pub fn net_since(self, opening: EnergyLedgers) -> f64 {
        self.light_in.since(opening.light_in) - self.heat_out.since(opening.heat_out)
    }

    /// Both ledgers validated, naming the one that failed.
    pub fn validate(self) -> Result<(), String> {
        self.light_in.validate("light_in_total")?;
        self.heat_out.validate("heat_out_total")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reference is analytic, not another floating-point sum: `2^20` additions of
    /// `2^-60` to `1.0` total exactly `1 + 2^-40`, and every intermediate value here is
    /// exactly representable. The raw counter cannot represent any of it (`1 + 2^-60`
    /// rounds back to `1`), which is the long-run failure in miniature.
    #[test]
    fn tiny_additions_a_raw_counter_cannot_hold_are_recovered_exactly() {
        let amount = 2f64.powi(-60);
        let (mut raw, mut correction) = (1.0f64, 0.0f64);
        for _ in 0..(1u32 << 20) {
            accumulate(&mut raw, &mut correction, amount);
        }
        assert_eq!(raw, 1.0, "the raw counter is bit-identical to the naive sum");
        let ledger = Ledger { raw, correction };
        assert_eq!(ledger.total(), 1.0 + 2f64.powi(-40), "corrected total is the exact sum");
        assert_eq!(correction, 2f64.powi(-40));
        assert_eq!(ledger.since(Ledger { raw: 1.0, correction: 0.0 }), 2f64.powi(-40));
    }

    /// A signed sequence of 20,000 amounts against an exact `i128` reference.
    ///
    /// Every amount is a multiple of `2^-70` and strictly below `2^-53`, half the ulp of the
    /// `1.0` the counter opens at, so **no** amount survives the raw addition: the raw
    /// counter is still exactly `1.0` at the end, as the pre-correction build's would be.
    /// Each step's error term and the running correction are exactly representable at these
    /// magnitudes, so the delta helper must return the reference sum bit for bit.
    #[test]
    fn signed_additions_below_the_raw_ulp_match_an_exact_integer_reference() {
        let scale = 2f64.powi(-70);
        let opening = Ledger { raw: 1.0, correction: 0.0 };
        let (mut raw, mut correction) = (opening.raw, opening.correction);
        let mut units: i128 = 0;
        let (mut positives, mut negatives) = (0u32, 0u32);
        let mut lcg = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..20_000 {
            lcg = lcg.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            // A signed amount in (−2^-53, 2^-53), expressed in whole units of 2^-70.
            let step = i128::from(((lcg >> 40) % (1 << 17)) as i64) - (1 << 16);
            if step > 0 {
                positives += 1;
            } else if step < 0 {
                negatives += 1;
            }
            units += step;
            accumulate(&mut raw, &mut correction, step as f64 * scale);
        }
        assert!(positives > 1_000 && negatives > 1_000, "the sequence must go both ways");
        assert_eq!(raw, 1.0, "the raw counter is bit-identical to the naive sum, which kept nothing");
        let exact = units as f64 * scale;
        assert_ne!(exact, 0.0, "the reference sum must be nonzero for this to prove anything");
        assert_eq!(Ledger { raw, correction }.since(opening), exact, "delta missed the exact sum");
        assert_eq!(correction, exact);
    }

    /// Compensation is not allowed to change the raw counter, whatever the operand order.
    #[test]
    fn the_raw_counter_is_exactly_the_naive_sum() {
        let amounts = [1e16, 1.0, -3.5e-9, 0.0, 7.25, 1e-300, -1e-300, 2.5e17];
        let (mut raw, mut correction) = (0.0f64, 0.0f64);
        let mut naive = 0.0f64;
        for a in amounts {
            naive += a;
            accumulate(&mut raw, &mut correction, a);
            assert_eq!(raw, naive, "raw diverged from the naive sum at amount {a}");
        }
        assert!(correction.is_finite());
    }

    #[test]
    fn a_non_finite_amount_poisons_the_state_instead_of_being_absorbed() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let (mut raw, mut correction) = (10.0f64, 0.0f64);
            accumulate(&mut raw, &mut correction, bad);
            let ledger = Ledger { raw, correction };
            assert!(ledger.validate("light_in_total").is_err(), "{bad} passed validation");
        }
    }

    #[test]
    fn validation_names_the_failure_and_accepts_a_signed_correction() {
        // A negative correction on a positive total is ordinary.
        Ledger { raw: 100.0, correction: -1e-9 }
            .validate("heat_out_total")
            .expect("a signed correction is valid");
        Ledger::default().validate("light_in_total").expect("a fresh world is valid");

        let err = Ledger { raw: 1.0, correction: f64::NAN }
            .validate("light_in_total")
            .expect_err("a NaN correction must fail");
        assert!(err.contains("light_in_total") && err.contains("correction"), "{err}");

        let err = Ledger { raw: 1.0, correction: -2.0 }
            .validate("heat_out_total")
            .expect_err("a negative corrected total must fail");
        assert!(err.contains("heat_out_total") && err.contains("negative"), "{err}");
    }

    #[test]
    fn the_delta_helper_beats_differencing_two_corrected_totals() {
        // A large raw total with a tiny interval flow: subtracting `total()`s rounds the
        // flow away, differencing each component keeps it.
        let opening = Ledger { raw: 1e16, correction: 0.25 };
        let mut closing = opening;
        accumulate(&mut closing.raw, &mut closing.correction, 0.5);
        assert_eq!(closing.raw, 1e16, "0.5 is below the ulp of 1e16");
        assert_eq!(closing.since(opening), 0.5);
        assert_eq!(closing.total() - opening.total(), 0.0, "the naive difference loses it");
    }

    #[test]
    fn the_energy_ledgers_net_the_two_flows_precisely() {
        let opening = EnergyLedgers {
            light_in: Ledger { raw: 1e15, correction: 0.0 },
            heat_out: Ledger { raw: 1e15, correction: 0.0 },
        };
        let mut closing = opening;
        accumulate(&mut closing.light_in.raw, &mut closing.light_in.correction, 0.75);
        accumulate(&mut closing.heat_out.raw, &mut closing.heat_out.correction, 0.25);
        assert_eq!(closing.net_since(opening), 0.5);
        closing.validate().expect("both ledgers are sound");
    }
}
