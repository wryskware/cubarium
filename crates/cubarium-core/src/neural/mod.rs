//! The recurrent sensory/action interface (R1a).
//!
//! Implements the slice of `design/recurrent-interface-contract.md` §9 that the world can
//! actually run: the 70-scalar spatial sampler (§1–2), the seven-channel action adapter (§3–4),
//! the GRU32 forward pass (§5), and the per-animal state the world persists.
//!
//! Nothing here trains, mutates or chooses weights. A policy is a fixed tensor set with a
//! digest; an animal is neural exactly when the world's [`state::NeuralState`] holds an entry
//! for its full `OrganismId`. Everything the world does to a body — the motor envelope, the
//! bills, the intake law, oxidation, growth, gestation, ageing and death — is unchanged and
//! still mandatory. What the policy replaces is the *behavioural* layer: mode hysteresis,
//! steering, the turn gate, `feed_min`, the quiet pause and the escape autopilot.
//!
//! The contract is a proposal, not canon: nothing in this module marks anything decided.

pub mod action;
pub mod gru;
pub mod obs;
pub mod state;

pub use action::{
    Action7, ActionAdapter, Capability, MOUTH_CHANNELS, VoxelAction3, VoxelActionAdapter,
};
pub use gru::{
    GRU_PARAMETERS, Gru, Gru23, Gru32, Gru37, HIDDEN, Policy, ShapePolicy, parameter_count,
};
pub use obs::{OBS_LEN, Observation70, SECTORS, SelfState, SensedBody, SensedCell};
pub use state::{AnimalState, Feedback, NeuralState};

/// The canonical text the schema digest is taken over (`contract §5`).
///
/// It names the observation layout, the action set, the recurrence convention, the commit
/// whose `motor::resolve` semantics the adapter targets, and the controller rate. Changing any
/// of those invalidates trained weights, so the digest changes with them. World *coefficients*
/// (speed, costs, `K_P`) deliberately do not enter it: they change what a policy is good at,
/// not what its inputs and outputs mean.
macro_rules! profile_text {
    ($act:literal) => {
        concat!(
            "cub-obs-1|",
            $act,
            "|gru32-reset-after-1|motor:r0b-99a2bfc|10hz",
            "|obs:p_here,f_here,d_here,food_near[6][3],food_far[6][3],",
            "body[6](presence,rel_size),crowd(fwd,side),water,light,height,up(fwd,side),",
            "reserve,energy,development,gestating,age,motor_avail,",
            "ate(graze,fruit,scavenge),moved,turned,delivered",
            "|act:thrust,turn,graze,fruit,scavenge,attack,reproduce",
            // Ecology v1 (`design/ecology-v1-contract.md` §15.2). The observation and action
            // *layouts* do not move — `d_here` is still "edible detrital material here", now
            // `D_eff + C_eff` — but what a channel means to the body does: the mouth masks
            // come from the two digestive capabilities, every channel bites at one shared
            // `mouth_rate`, and the `ate` feedback is normalised by that rate. Wrysk decided
            // on 2026-09-15 that training was cheap enough that old policies need not
            // reload, so the digest breaks and every existing policy file, every `runs/es-*`
            // centre and the R3a display seed is refused **by name** rather than silently
            // reinterpreted.
            "|eco:v1",
        )
    };
}

pub const PROFILE_TEXT: &str = profile_text!("cub-act-1");

/// The same text under the released-turn-band adapter
/// (`design/handoffs/ecology-v1-turn-deadband-opus-2026-09-16.md`). It is built by the **same**
/// macro from the same pieces, so the two texts cannot differ in anything but the adapter
/// token, and [`ActionAdapter`] is the only thing that chooses between them.
pub const PROFILE_TEXT_CUB_ACT_2: &str = profile_text!("cub-act-2");

/// The profile text a named adapter is authored against.
pub fn profile_text(adapter: ActionAdapter) -> &'static str {
    match adapter {
        ActionAdapter::CubAct1 => PROFILE_TEXT,
        ActionAdapter::CubAct2 => PROFILE_TEXT_CUB_ACT_2,
    }
}

/// FNV-1a 64 over [`PROFILE_TEXT`]. Stored in every [`Policy`] and in the extension header; a
/// policy whose digest differs from this build's is refused *by name* on load, never
/// reinterpreted.
pub fn schema_digest() -> u64 {
    schema_digest_in(ActionAdapter::CubAct1)
}

/// [`schema_digest`] under a named [`ActionAdapter`]. `CubAct1` is [`schema_digest`] itself,
/// byte for byte, so every retained policy, centre and protocol keeps the digest it has.
pub fn schema_digest_in(adapter: ActionAdapter) -> u64 {
    fnv1a(profile_text(adapter).as_bytes())
}

pub(crate) fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_digest_is_stable_and_sensitive() {
        assert_eq!(schema_digest(), schema_digest());
        assert_ne!(schema_digest(), fnv1a(b"cub-obs-1"));
    }

    #[test]
    fn the_parameter_count_is_the_contracts() {
        // `3·H·(I + H + 2) + O·(H + 1)` with I = 70, H = 32, O = 7.
        assert_eq!(GRU_PARAMETERS, 10_215);
    }
}
