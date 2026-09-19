//! The frozen fixture: founders, layout seeds, episode bounds, and the immutable prepared
//! layout one episode clones its private mutable copy from.
//!
//! Everything here is data the tests plan fixes ("Use four training layout seeds and eight
//! untouched evaluation seeds per archetype"; "Begin with 1,200 ticks"; the §3 bounds
//! table). A layout seed varies geometry, resource placement and the arena's starting
//! heading within the task — never the sensory contract, which the manifest owns.
//!
//! # Prepared layouts
//!
//! [`Prepared`] is one [`cubarium_voxel_sim::Arena`] built once from
//! `(founder, layout_seed)` and kept **immutable**. An episode takes
//! [`Prepared::episode_arena`], which clones the three layers into the episode's own
//! private mutable copy — that clone *is* the fresh-episode reset of every mutable arena
//! datum (stocks, the fauna's ledger, the placed animal), and it is also what makes the
//! two signs of an antithetic pair bit-identical worlds, because [`Arena::build`] is a pure
//! function of its seed. Rebuilding from the seed stays available
//! ([`Prepared::rebuilt`]) so the bench can measure the two setup paths against each other.

use cubarium_voxel_fauna::{Founder, Senses};
use cubarium_voxel_sim::Arena;

/// The phase-one pilot's episode horizon: 1,200 ticks is 60 simulated seconds at 20 Hz
/// (`design/voxel-senses-phase1-tests.md` §2). "Ample for multiple body lengths without
/// inheriting the old 36,000-tick campaign."
pub const HORIZON_TICKS: u64 = 1_200;

/// The four training layout seeds. Chosen so **none** is a seam layout
/// (`layout_seed % 4 == 3` is the arena's deliberate seam test, and the plan's default
/// layouts keep food away from the seam).
pub const TRAINING_LAYOUT_SEEDS: [u64; 4] = [1, 2, 4, 5];

/// The eight untouched evaluation seeds. Held out: not used to choose parameters or the
/// best generation (`design/voxel-senses-phase1-tests.md` §2). Also non-seam.
pub const EVALUATION_LAYOUT_SEEDS: [u64; 8] = [6, 8, 9, 10, 13, 14, 17, 18];

/// The Stage-A start-heading convention this build's arena places founders under
/// (P2-B step 2): blind uniform over the circle, browser uniform within +/-90 degrees of
/// the bearing. Phase one aimed both founders at their food with a +/-5 degree jitter,
/// which is a different task; a policy trained under it is refused rather than
/// reinterpreted, both through [`super::trainer::VoxelProtocol::hash`] and through the
/// policy file's own field.
pub const START_HEADING_PROTOCOL: &str = "p2b-varied-1";

/// The trainer's default train seed, in the trainer's own stream — its randomness never
/// touches a world draw ([`super::trainer`]).
pub const TRAINING_SEED: u64 = 20_260_918;

/// Antithetic pairs per generation (plan §3: "Eight antithetic pairs on four training
/// layouts: 64 perturbation episodes").
pub const DEFAULT_PAIRS: usize = 8;

/// Centre evaluations per generation: the unperturbed centre on **every** training layout.
pub const CENTER_EVALUATIONS: usize = TRAINING_LAYOUT_SEEDS.len();

/// Updates per archetype the pilot bounds itself to (plan §3).
pub const MAX_UPDATES: u32 = 32;

/// The voxel trainer's measured saturation point. The machine-wide policy also reserves
/// ten percent of logical CPUs, but this workload gains little beyond sixteen workers.
pub const MAX_EPISODE_WORKERS: usize = 16;

/// The available voxel episode workers: obey the machine-wide CPU reserve and the
/// workload-specific saturation cap. Every episode itself stays single-threaded.
pub fn episode_worker_limit() -> usize {
    worker_limit_for(std::thread::available_parallelism().map_or(1, std::num::NonZero::get))
        .min(MAX_EPISODE_WORKERS)
}

fn worker_limit_for(cpus: usize) -> usize {
    let cpus = cpus.max(1);
    let reserved = ((cpus + 9) / 10).max(1);
    cpus.saturating_sub(reserved).max(1)
}

/// Wall seconds the plan allots one archetype's training ("up to eight minutes").
pub const DEFAULT_TRAIN_WALL_SECONDS: u64 = 480;

/// The plan's episode count for one archetype's full bounded training: the initial centre
/// evaluation (4) plus 32 updates × (2·8 perturbation + 4 centre evaluations) = 2,180
/// (`design/voxel-senses-phase1-tests.md` §3).
pub const DEFAULT_EPISODE_LIMIT: u64 = 2_180;

/// One immutable prepared arena: the terrain, the pond, the finite resources and the
/// placed founder, exactly as [`Arena::build`] left them.
pub struct Prepared {
    pub founder: Founder,
    pub layout_seed: u64,
    arena: Arena,
    /// The arena's settled cue field. Every episode clones it into its private simulator;
    /// field settlement belongs to fixture preparation, never to every candidate rollout.
    senses: Senses,
}

impl Prepared {
    /// Build the prepared layout: one [`Arena::build`], kept immutable.
    pub fn build(founder: Founder, layout_seed: u64) -> Prepared {
        let arena = Arena::build(founder, layout_seed);
        Prepared {
            founder,
            layout_seed,
            senses: arena.prepare_senses(),
            arena,
        }
    }

    /// A fresh **private mutable copy** of the arena for one episode: every mutable datum
    /// — stocks, ledger, the placed animal — starts at the prepared state, and nothing one
    /// episode does can reach the prepared layout or another episode.
    ///
    /// The clone carries `births = false` (the arena set it at build) and the placed
    /// founder at its starting pose.
    pub fn episode_arena(&self) -> Arena {
        Arena {
            founder: self.arena.founder,
            layout_seed: self.arena.layout_seed,
            world: self.arena.world.clone(),
            flora: self.arena.flora.clone(),
            fauna: self.arena.fauna.clone(),
            resources: self.arena.resources.clone(),
            animal_id: self.arena.animal_id,
        }
    }

    /// A fresh copy of the settled cue field for one private episode simulator.
    pub fn episode_senses(&self) -> Senses {
        self.senses.clone()
    }

    /// A fresh arena rebuilt from the seed — the other setup path, measured beside the
    /// clone path by the bench. Both must prepare the identical arena.
    pub fn rebuilt(&self) -> Arena {
        Arena::build(self.founder, self.layout_seed)
    }

    /// The placed founder's id, if the arena landed one.
    pub fn animal_id(&self) -> Option<u64> {
        self.arena.animal_id
    }

    /// The immutable prepared arena, for the fixture's own validation surface. **The
    /// episode driver and every observation source must not read its `resources` or its
    /// stock** (`super`'s boundaries): this accessor exists for `voxel-check` and tests.
    pub fn fixture_arena(&self) -> &Arena {
        &self.arena
    }
}

impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prepared")
            .field("founder", &self.arena.founder.name())
            .field("layout_seed", &self.layout_seed)
            .field("animal_id", &self.arena.animal_id)
            .field("resources", &self.arena.resources.len())
            .finish()
    }
}

/// The training layout seeds, as [`Prepared`] layouts, in the frozen order.
pub fn training_layouts(founder: Founder) -> Vec<Prepared> {
    TRAINING_LAYOUT_SEEDS
        .map(|seed| Prepared::build(founder, seed))
        .into()
}

/// The evaluation layout seeds, as [`Prepared`] layouts, in the frozen order.
pub fn evaluation_layouts(founder: Founder) -> Vec<Prepared> {
    EVALUATION_LAYOUT_SEEDS
        .map(|seed| Prepared::build(founder, seed))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No training or evaluation seed is the arena's seam layout, so the default fixtures
    /// keep food away from the wrapped seam as the plan says.
    #[test]
    fn the_frozen_seeds_avoid_the_seam_layout() {
        for seed in TRAINING_LAYOUT_SEEDS
            .into_iter()
            .chain(EVALUATION_LAYOUT_SEEDS)
        {
            assert_ne!(
                seed % 4,
                3,
                "seed {seed} is the arena's deliberate seam layout"
            );
        }
    }

    #[test]
    fn worker_limit_reserves_ten_percent_then_stops_at_measured_saturation() {
        assert_eq!(worker_limit_for(1), 1);
        assert_eq!(worker_limit_for(2), 1);
        assert_eq!(worker_limit_for(10), 9);
        assert_eq!(worker_limit_for(32).min(MAX_EPISODE_WORKERS), 16);
        assert_eq!(worker_limit_for(128).min(MAX_EPISODE_WORKERS), 16);
    }

    /// The two setup paths prepare the identical arena: cloning the immutable layout and
    /// rebuilding from the seed give the same placed animal, the same stock and the same
    /// world material.
    #[test]
    fn clone_and_rebuild_prepare_identical_arenas() {
        for founder in Founder::ALL {
            let p = Prepared::build(founder, TRAINING_LAYOUT_SEEDS[2]);
            let cloned = p.episode_arena();
            let rebuilt = p.rebuilt();
            assert_eq!(cloned.animal_id, rebuilt.animal_id);
            assert_eq!(cloned.animal_pose(), rebuilt.animal_pose());
            assert_eq!(
                format!("{:?}", cloned.world),
                format!("{:?}", rebuilt.world),
                "the cloned world equals the rebuilt one"
            );
        }
    }

    /// The fresh-episode reset: mutable arena data starts at the prepared state, and what
    /// one episode's private copy does stays in that copy. The fixture itself is allowed
    /// to read the stock — this is exactly what the arena built the accessor for — while
    /// the driver never does.
    #[test]
    fn an_episode_leaves_the_prepared_layout_and_other_copies_untouched() {
        let p = Prepared::build(Founder::Blind, TRAINING_LAYOUT_SEEDS[0]);
        let before = p.fixture_arena().resource_stock();
        assert!(
            before.is_finite() && before > 0.0,
            "a finite prepared stock"
        );
        let site = p.fixture_arena().resources[0];

        // One episode copy consumes part of the stock through the fixture's settlement API.
        let mut episode = p.episode_arena();
        let taken = episode.take(site, 0.05).expect("the stock is takeable");
        assert!(taken.organic > 0.0);
        assert!(episode.resource_stock() < before);

        // The prepared layout and a second fresh copy are untouched by it.
        assert_eq!(p.fixture_arena().resource_stock(), before);
        let again = p.episode_arena();
        assert_eq!(again.resource_stock(), before);
        assert_eq!(again.animal_pose(), p.episode_arena().animal_pose());
    }
}
