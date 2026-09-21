//! The **voxel episode driver and its ES**: phase-one package P1-D
//! (`design/voxel-senses-phase1-plan.md`, "Work packages").
//!
//! This module runs bounded ES over the two founder manifests against the **static voxel
//! arena** (`cubarium-voxel-sim::Arena`), through the landed shape-aware GRU arithmetic
//! ([`super::tensor`]'s `unflatten_shape`/`initial_center_shape`/`shape_policy`). It shares
//! the trainer's optimizer ([`super::optimizer`]), its keyed perturbation stream
//! ([`super::rng`]) and its float persistence ([`super::bits`]) so there is one neural
//! implementation and one ES arithmetic in the crate.
//!
//! The slices:
//!
//! - [`task`]: the frozen fixture — the two founders, the four training and eight
//!   evaluation layout seeds, the episode/protocol bounds, and the immutable prepared
//!   layout one episode clones its private mutable copy from.
//! - [`driver`]: one episode — build or clone an arena, **attach the episode's
//!   controller through the fauna's own controller table** (`Fauna::set_controller`;
//!   the fauna tick samples, holds and resolves), run on a single simulation thread to
//!   horizon or death, with the cancellation flag and wall-clock deadline checked
//!   **inside** the loop.
//! - [`controller`]: the controller bodies the seam runs — a validated `ShapePolicy`
//!   answering raw logits (the fauna's shared adapter transfers and deadbands them),
//!   the three disclosed open-loop controls (`no-intake`, `stationary-feeding`,
//!   `cruise`), the P2-B sense ablation, and the observation-only heuristic slot,
//!   which is the fauna's own `BlindForager`/`BrowserForager`.
//! - [`score`]: the capability-training score of the tests plan §2 and its components.
//! - [`trainer`]: shape-aware antithetic pairs, bounded workers, cancellation that counts
//!   its discarded work, and the run/checkpoint store.
//! - [`imitate`]: the P3-C imitation seed — teacher streams recorded from the fauna's
//!   own foraging heuristic on the training layouts.
//! - [`store`]: the exported policy file and checkpoint formats.
//! - [`commands`]: the `voxel-check` / `voxel-bench` / `voxel-train` / `voxel-evaluate`
//!   command family.
//!
//! # Boundaries this module holds
//!
//! - **The driver never touches the arena's settlement APIs.** `Arena::resources`,
//!   `resource_stock()` and `take()` are the fixture's privileged stock/settlement surface:
//!   they belong to arena tests and settlement, never to a controller input, an observation
//!   or the reward. Everything the driver reads is the fauna view, the placed animal and
//!   the ledger — read-only — plus the pose.
//! - **A controller receives only the observation vector and its own memory.** The fauna's
//!   `Controller` trait enforces that by shape; this module adds only the digest
//!   discipline and the bodies.
//! - **The observation seam is the fauna's own.** The fauna tick builds each due
//!   founder's observation (`body::observation`): real `Self`/contact/wet/taste channels
//!   now, the P1-C channels (chem/light/cone) as valid zeros until that worker lands its
//!   samplers there. There is deliberately no driver-side sampler for P1-C to replace —
//!   the pluggable point moved into the fauna with the controller stage.
//! - **One simulation thread per episode; the runtime worker ceiling reserves ten percent
//!   of logical CPUs and stops at sixteen; no nested parallelism.** Every episode runs its
//!   [`cubarium_voxel_sim::Sim`] with `SimConfig { threads: 1 }`, so the fauna leg never
//!   enters the process-wide task pool while episode workers are running.
//!
//! # What the digest plug-in is
//!
//! [`voxel_schema_digest`] is the single integration boundary
//! [`super::tensor::shape_policy`]'s doc anticipated: the driver passes the placed
//! founder's manifest digest and refuses a mismatch by name. A policy authored against
//! one founder's schema is refused against the other's.

use cubarium_voxel_fauna::Founder;

pub mod commands;
pub mod controller;
pub mod driver;
pub mod imitate;
pub mod score;
pub mod store;
pub mod task;
pub mod trainer;

pub use controller::{
    CRUISE_FORWARD, Cruise, EpisodeDriver, EpisodeGru, EpisodeKind, GruBlind, GruBrowser,
    GruPolicy, NoIntake, RecordingController, SELF_CHANNELS, StationaryFeeding, TeacherSink,
    TeacherStep, VoxelControl, teacher_sink,
};
pub use driver::{Episode, EpisodeError, Limits, ScoreCounters};
pub use score::{SURVIVAL_WEIGHT, ScoreComponents};
pub use store::VoxelPolicyFile;
pub use task::{EVALUATION_LAYOUT_SEEDS, HORIZON_TICKS, TRAINING_LAYOUT_SEEDS, TRAINING_SEED};
pub use trainer::{
    Discarded, GenerationError, GenerationPlan, GenerationReport, Job, TrainReport, TrainSpec,
    VoxelCheckpoint, VoxelProtocol,
};

/// The digest the voxel episode driver validates a policy against: the placed founder's
/// manifest digest, FNV-1a 64 over that manifest's `canonical_text()`.
///
/// This is where the digest plug-in [`super::tensor::shape_policy`] leaves a hole for
/// lands. The manifest is the landed P1-A interface; this crate now depends on
/// `cubarium-voxel-fauna` read-only to name it.
pub fn voxel_schema_digest(founder: Founder) -> u64 {
    founder.manifest().digest()
}

/// A founder from a command-line name. Accepts the lineage name, its role word, and the
/// two obvious shorthand spellings, so `--founder blind`, `--founder littershredder` and
/// `--founder Browser` all resolve.
pub fn parse_founder(s: &str) -> Result<Founder, String> {
    let t = s.trim().to_ascii_lowercase();
    Ok(match t.as_str() {
        "blind" | "littershredder" | "litter" => Founder::Blind,
        "browser" | "frondgrazer" | "sighted" | "foliage" => Founder::Browser,
        other => {
            return Err(format!(
                "unknown founder `{other}`; use `blind` (littershredder) or `browser` \
                 (frondgrazer)"
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The digest plug-in: the two founders' digests exist, are stable across calls, and
    /// separate the lineages.
    #[test]
    fn the_voxel_schema_digest_is_the_manifest_digest() {
        for founder in Founder::ALL {
            let a = voxel_schema_digest(founder);
            let b = voxel_schema_digest(founder);
            assert_ne!(a, 0);
            assert_eq!(a, b, "the digest is a pure function of the founder");
            assert_eq!(a, founder.manifest().digest());
        }
        assert_ne!(
            voxel_schema_digest(Founder::Blind),
            voxel_schema_digest(Founder::Browser),
            "the two schemas are different tasks"
        );
    }

    #[test]
    fn the_founder_name_parses_both_spellings() {
        assert_eq!(parse_founder("blind").expect("blind"), Founder::Blind);
        assert_eq!(
            parse_founder("littershredder").expect("lineage name"),
            Founder::Blind
        );
        assert_eq!(parse_founder("Browser").expect("role"), Founder::Browser);
        assert_eq!(
            parse_founder("frondgrazer").expect("lineage"),
            Founder::Browser
        );
        assert!(parse_founder("frondgrazer ").is_ok(), "trims");
        assert!(parse_founder("krill").is_err());
    }
}
