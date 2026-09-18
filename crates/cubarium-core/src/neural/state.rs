//! The persisted neural extension (contract §5).
//!
//! `WorldState` gains one **appended** field, [`NeuralState`], in the same style as care,
//! hunters, quiet, dormancy and encounters. An animal is neural **iff** the table holds an
//! entry for its full `OrganismId`; there is no flag on the organism. Decoding an older
//! schema yields an empty extension, so every existing world loads with every organism
//! legacy-controlled, byte for byte the current tick. Migration of a legacy world is an
//! explicit development action that inserts entries; loading never does it.

use serde::{Deserialize, Serialize};

use crate::ids::OrganismId;

use super::action::{ACT_LEN, Action7};
use super::gru::{HIDDEN, Policy};

/// The feedback the world accumulates every tick and the animal consumes at its controller
/// tick (observation indices 64–69).
///
/// Sums, not ratios: normalisation happens once when the vector is built, against the
/// interval's own `ticks`, so a partial first interval is scaled by what actually elapsed
/// rather than by the nominal two ticks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Feedback {
    /// Material actually removed from the fields by this mouth: graze, fruit, scavenge.
    pub ate: [f64; 3],
    /// Σ resolved speed, px/s.
    pub speed_sum: f64,
    /// Σ signed physical turn, radians (transport excluded by construction).
    pub turn_sum: f64,
    /// Σ requested motor magnitude and Σ resolved motor magnitude, px/s.
    pub req_mag: f64,
    pub res_mag: f64,
    /// Ticks accumulated since the last controller update.
    pub ticks: u32,
}

impl Feedback {
    /// Reduce the interval to the six contract channels.
    ///
    /// `mouth_rate` and `dt` give the per-tick handling ceiling each `ate` channel is scaled
    /// by; `v_max` and `r` give the travel and pivot scales. An **empty** interval reads as
    /// zero intake, zero motion and `delivered = 1`: nothing was requested, so everything
    /// requested was delivered. That is the same rule as a non-empty interval that asked for
    /// nothing, and it is the value a newborn carries into its first update.
    ///
    /// Ecology v1 (`design/ecology-v1-contract.md` §6.3, §15.2): there is **one** mouth rate,
    /// so all three intake channels are normalised by the same ceiling. The diet-split rates
    /// they used before no longer exist.
    pub fn channels(&self, mouth_rate: f64, v_max: f64, r: f64, dt: f64) -> [f64; 6] {
        let ticks = f64::from(self.ticks);
        let interval = ticks * dt;
        let mouth_cap = mouth_rate * interval;
        let mut out = [0.0f64; 6];
        out[0] = share(self.ate[0], mouth_cap);
        out[1] = share(self.ate[1], mouth_cap);
        out[2] = share(self.ate[2], mouth_cap);
        out[3] = if ticks > 0.0 {
            share(self.speed_sum / ticks, v_max)
        } else {
            0.0
        };
        // `v_max / r` rad/s is the fixed per-body scale of a full-budget pivot, so the channel
        // uses its range at native pace whatever the body's size.
        let turn_scale = if r > 0.0 { (v_max / r) * interval } else { 0.0 };
        out[4] = if turn_scale > 0.0 {
            (self.turn_sum / turn_scale).clamp(-1.0, 1.0)
        } else {
            0.0
        };
        out[5] = if self.req_mag > 0.0 {
            (self.res_mag / self.req_mag).clamp(0.0, 1.0)
        } else {
            1.0
        };
        out
    }

    pub fn is_finite(&self) -> bool {
        self.ate.iter().all(|x| x.is_finite())
            && self.speed_sum.is_finite()
            && self.turn_sum.is_finite()
            && self.req_mag.is_finite()
            && self.res_mag.is_finite()
    }
}

fn share(x: f64, cap: f64) -> f64 {
    if cap > 0.0 && x.is_finite() {
        (x / cap).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// One animal's private recurrent state. Never shared, never reset at a seam, a meal, a frame
/// or a save.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimalState {
    /// `[f64; 32]`, zero at birth.
    pub hidden: Vec<f64>,
    /// The post-squash, post-mask action in force between controller ticks.
    pub held: [f64; ACT_LEN],
    pub feedback: Feedback,
    /// Birth-tick parity, fixed for life, never heritable: the population's inference load is
    /// split across both ticks and every animal still sees `Δt_c = 0.1 s`.
    pub phase: u8,
    /// Index into [`NeuralState::policies`].
    pub policy: u32,
}

impl AnimalState {
    /// A fresh animal: zero hidden state, nothing held, no feedback, phase from the birth tick.
    pub fn fresh(birth_tick: u64, policy: u32) -> AnimalState {
        AnimalState {
            hidden: vec![0.0; HIDDEN],
            held: [0.0; ACT_LEN],
            feedback: Feedback::default(),
            phase: u8::try_from(birth_tick % 2).expect("0 or 1"),
            policy,
        }
    }

    /// Does this animal run its controller on `tick`?
    pub fn updates_on(&self, tick: u64) -> bool {
        (tick + u64::from(self.phase)) % 2 == 0
    }

    pub fn held_action(&self) -> Action7 {
        Action7(self.held)
    }
}

/// The appended world extension.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NeuralState {
    /// 0 for the inert default; 1 for the v1 interface.
    pub version: u8,
    /// Distinct weight sets. Identical weights are stored once and referenced; hidden state is
    /// never shared.
    pub policies: Vec<Policy>,
    /// Sorted by `OrganismId`, so the encoding is canonical and the `state_hash` does not
    /// depend on insertion order.
    pub animals: Vec<(OrganismId, AnimalState)>,
}

impl NeuralState {
    pub fn is_empty(&self) -> bool {
        self.animals.is_empty() && self.policies.is_empty()
    }

    pub fn index_of(&self, id: OrganismId) -> Option<usize> {
        self.animals.binary_search_by(|(k, _)| k.cmp(&id)).ok()
    }

    pub fn get(&self, id: OrganismId) -> Option<&AnimalState> {
        self.index_of(id).map(|i| &self.animals[i].1)
    }

    pub fn get_mut(&mut self, id: OrganismId) -> Option<&mut AnimalState> {
        self.index_of(id).map(|i| &mut self.animals[i].1)
    }

    pub fn contains(&self, id: OrganismId) -> bool {
        self.index_of(id).is_some()
    }

    pub fn policy(&self, animal: &AnimalState) -> Option<&Policy> {
        self.policies.get(animal.policy as usize)
    }

    /// Add a policy, reusing an identical existing one.
    pub fn intern(&mut self, policy: Policy) -> u32 {
        if let Some(i) = self.policies.iter().position(|p| *p == policy) {
            return u32::try_from(i).expect("policy table fits in u32");
        }
        self.version = self.version.max(1);
        self.policies.push(policy);
        u32::try_from(self.policies.len() - 1).expect("policy table fits in u32")
    }

    /// Attach `id` to a policy with fresh private state, replacing any existing entry.
    pub fn insert(&mut self, id: OrganismId, state: AnimalState) {
        self.version = self.version.max(1);
        match self.animals.binary_search_by(|(k, _)| k.cmp(&id)) {
            Ok(i) => self.animals[i].1 = state,
            Err(i) => self.animals.insert(i, (id, state)),
        }
    }

    /// Remove `id` at the boundary that removes the organism. Entries are keyed by the **full**
    /// `OrganismId` (slot and generation), so a reused slot can never inherit hidden state
    /// even if a removal were ever missed.
    pub fn remove(&mut self, id: OrganismId) -> Option<AnimalState> {
        self.index_of(id).map(|i| self.animals.remove(i).1)
    }

    /// Range checks after decode.
    pub fn validate(&self) -> Result<(), String> {
        if self.is_empty() {
            return Ok(());
        }
        if self.version != 1 {
            return Err(format!(
                "neural extension version {} is not supported by this build",
                self.version
            ));
        }
        for (i, p) in self.policies.iter().enumerate() {
            p.validate().map_err(|e| format!("policy {i}: {e}"))?;
        }
        let mut previous: Option<OrganismId> = None;
        for (id, a) in &self.animals {
            if previous.is_some_and(|p| p >= *id) {
                return Err("neural animals are not sorted by OrganismId".into());
            }
            previous = Some(*id);
            if a.hidden.len() != HIDDEN {
                return Err(format!(
                    "neural animal {id:?} carries {} hidden units, expected {HIDDEN}",
                    a.hidden.len()
                ));
            }
            if a.hidden.iter().any(|x| !x.is_finite())
                || a.held.iter().any(|x| !x.is_finite())
                || !a.feedback.is_finite()
            {
                return Err(format!("neural animal {id:?} carries a non-finite value"));
            }
            if a.phase > 1 {
                return Err(format!(
                    "neural animal {id:?} has phase {}, not 0 or 1",
                    a.phase
                ));
            }
            if self.policies.get(a.policy as usize).is_none() {
                return Err(format!(
                    "neural animal {id:?} references policy {} of {}",
                    a.policy,
                    self.policies.len()
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::neural::gru::Gru32;

    fn id(slot: u32, generation: u32) -> OrganismId {
        OrganismId { slot, generation }
    }

    #[test]
    fn the_table_stays_sorted_and_keys_on_the_full_id() {
        let mut s = NeuralState::default();
        let p = s.intern(Policy::new(Gru32::zeros()));
        for slot in [7u32, 2, 5] {
            s.insert(id(slot, 0), AnimalState::fresh(0, p));
        }
        assert_eq!(
            s.animals.iter().map(|(k, _)| k.slot).collect::<Vec<_>>(),
            vec![2, 5, 7]
        );
        // A reused slot is a different key.
        assert!(s.contains(id(5, 0)));
        assert!(!s.contains(id(5, 1)));
        s.remove(id(5, 0));
        assert!(!s.contains(id(5, 0)));
        s.validate().unwrap();
    }

    #[test]
    fn identical_weights_are_stored_once() {
        let mut s = NeuralState::default();
        let a = s.intern(Policy::new(Gru32::zeros()));
        let b = s.intern(Policy::new(Gru32::zeros()));
        assert_eq!(a, b);
        assert_eq!(s.policies.len(), 1);
    }

    #[test]
    fn the_cadence_splits_the_population_across_both_ticks() {
        let even = AnimalState::fresh(10, 0);
        let odd = AnimalState::fresh(11, 0);
        assert_eq!(even.phase, 0);
        assert_eq!(odd.phase, 1);
        for tick in 0..8u64 {
            assert_ne!(
                even.updates_on(tick),
                odd.updates_on(tick),
                "exactly one of the two phases updates on tick {tick}"
            );
        }
        // Each animal sees every second tick: Δt_c = 2·dt.
        let hits: Vec<u64> = (0..8).filter(|t| even.updates_on(*t)).collect();
        assert_eq!(hits, vec![0, 2, 4, 6]);
    }

    #[test]
    fn an_empty_interval_reports_delivered_one_and_nothing_else() {
        let f = Feedback::default();
        let c = f.channels(1.0, 5.0, 2.5, 0.05);
        assert_eq!(c[..5], [0.0; 5]);
        assert_eq!(
            c[5], 1.0,
            "nothing was requested, so all of it was delivered"
        );
    }

    #[test]
    fn a_partial_interval_is_scaled_by_the_ticks_that_actually_elapsed() {
        let mut one = Feedback {
            ticks: 1,
            ..Feedback::default()
        };
        one.ate[0] = 0.5 * 1.0 * 0.05; // half of one tick's handling
        one.speed_sum = 2.5;
        let c = one.channels(1.0, 5.0, 2.5, 0.05);
        assert!((c[0] - 0.5).abs() < 1e-12, "{c:?}");
        assert!((c[3] - 0.5).abs() < 1e-12, "mean speed over one tick");
    }

    #[test]
    fn validate_refuses_a_dangling_policy_a_bad_phase_and_a_short_hidden_state() {
        let mut s = NeuralState::default();
        s.intern(Policy::new(Gru32::zeros()));
        s.insert(id(0, 0), AnimalState::fresh(0, 9));
        assert!(s.validate().unwrap_err().contains("references policy 9"));

        let mut s = NeuralState::default();
        let p = s.intern(Policy::new(Gru32::zeros()));
        let mut a = AnimalState::fresh(0, p);
        a.phase = 3;
        s.insert(id(0, 0), a);
        assert!(s.validate().unwrap_err().contains("phase 3"));

        let mut s = NeuralState::default();
        let p = s.intern(Policy::new(Gru32::zeros()));
        let mut a = AnimalState::fresh(0, p);
        a.hidden.truncate(4);
        s.insert(id(0, 0), a);
        assert!(s.validate().unwrap_err().contains("4 hidden units"));
    }

    #[test]
    fn a_foreign_digest_is_refused_by_name() {
        let mut s = NeuralState::default();
        let mut p = Policy::new(Gru32::zeros());
        p.schema_digest ^= 0xdead_beef;
        let index = s.intern(p);
        s.insert(id(0, 0), AnimalState::fresh(0, index));
        let err = s.validate().unwrap_err();
        assert!(err.contains("schema_digest"), "{err}");
        assert!(err.contains("cannot be reinterpreted"), "{err}");
    }
}
