//! **The static phase-one arena**: a 32 × 16 × 12 voxel world at 0.25 m/voxel, prepared
//! once from a layout seed, with a finite consumable resource layout and the founder body
//! placed off it.
//!
//! This is the fixture `design/voxel-senses-phase1-plan.md`'s "Frozen arena contract"
//! describes. Terrain and water are laid down by [`Arena::build`] and then never evolve:
//! the arena is run through [`Sim::new_static`], whose schedule has no `Water` and no
//! `Flora` leg. What still runs is the production fauna tick — the same `sys_fauna` the
//! live schedule runs — so an animal placed here ages, pays maintenance and dies by the
//! real rules, and P1-B/P1-C add the real motion, contacts, senses and feeding behind the
//! same interfaces.
//!
//! # The two layouts
//!
//! [`Founder::Blind`] gets **litter** deposited on the ground: the finite stock the litter
//! feeder will eat. [`Founder::Browser`] gets **springturf** stands, each with full
//! foliage: the finite stock the browser will crop. Both are finite because neither
//! decomposition nor growth runs in the static schedule. [`Arena::resource_stock`] reads
//! the live total, so a test can watch a bite lower it.
//!
//! # What P1-A does *not* do
//!
//! The placed founder is idle. It has a continuous pose and a support face, but no
//! forward/turn/feed resolution, no sensory sample and no field; those are P1-B and P1-C.
//! P1-A's acceptance is that the world holds still and the body does not.
//!
//! # What P1-C adds
//!
//! [`Sim::new_static`] — which [`Arena::into_sim`]
//! reaches — settles the arena's litter cue field before the first tick samples it, and
//! the static schedule's fauna leg steps the production tick with that field, so a driven
//! founder's `Chem(litter)` reads the prepared cue while taste remains tied to current
//! mouth contact.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{Command as FaunaCommand, Fauna, FaunaConfig, Founder, Pose, Senses};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraConfig, Site, Species as Plant,
    Taken,
};

use crate::Sim;

/// Arena width in voxels; `x` wraps.
pub const ARENA_WIDTH: u32 = 32;
/// Arena height in voxels.
pub const ARENA_HEIGHT: u32 = 16;
/// Arena depth in voxels.
pub const ARENA_DEPTH: u32 = 12;
/// Edge length of one voxel, metres.
pub const ARENA_VOXEL_M: f64 = 0.25;
/// The `y` of the all-soil ground every column supports at.
pub const GROUND_Y: u32 = 4;

/// Fraction of a void cell a prepared pond cell is filled to.
const POND_FILL: f64 = 0.8;
/// Organic matter in one litter tile.
const LITTER_PER_TILE: f64 = 0.2;
/// Litter mineral fraction (a plant tissue's order of magnitude).
const LITTER_MINERAL_FRACTION: f64 = 0.02;
/// Litter retained-energy density, at the litter energy cap.
const LITTER_ENERGY_DENSITY: f64 = 2.0;
/// How many resource tiles a default layout lays.
const RESOURCE_TILES: usize = 6;

/// Deterministic scalar stream, splitmix64. No clock, no thread state.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }
}

/// A built arena and the founder it placed.
pub struct Arena {
    /// Which founder this arena feeds.
    pub founder: Founder,
    /// The layout seed the terrain, pond and resources were drawn from.
    pub layout_seed: u64,
    pub world: World,
    pub flora: Flora,
    pub fauna: Fauna,
    /// The finite resource sites, in column order: litter tiles or foliage stands.
    pub resources: Vec<Site>,
    /// The id of the idle founder body placed at build time, if it landed.
    pub animal_id: Option<u64>,
}

/// Stage B's two real, finite patches. The metadata is fixture-facing only: it never
/// enters an observation or controller and exists so a later evaluation can report
/// reacquisition without rediscovering a target from the world.
pub struct ReacquisitionArena {
    pub arena: Arena,
    /// The patch the founder starts within the ordinary Stage-A signal distance of.
    pub initial_patch: Site,
    /// A distinct finite patch left after the initial patch is depleted.
    pub successor_patch: Site,
}

impl ReacquisitionArena {
    /// Move the two-patch layout into the ordinary static simulator.
    pub fn into_arena(self) -> Arena {
        self.arena
    }

    /// Move the arena while retaining the two fixture sites for an evaluator's
    /// post-episode accounting. The sites remain outside the controller boundary.
    pub fn into_parts(self) -> (Arena, Site, Site) {
        (self.arena, self.initial_patch, self.successor_patch)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LayoutKind {
    StageA,
    Reacquisition,
}

impl Arena {
    /// Build the frozen arena for `founder` from `layout_seed`, and place one idle founder
    /// body off the food. The world is not stepped: the pond is a direct `AddWater` and the
    /// ground is direct `SetMaterial`, so there is no settling cost and nothing to freeze
    /// afterwards.
    pub fn build(founder: Founder, layout_seed: u64) -> Arena {
        Self::build_kind(founder, layout_seed, LayoutKind::StageA).0
    }

    /// Build Stage B's smallest real continuation task: one reachable finite patch and
    /// one distinct successor. Both use the normal flora deposits, fauna feeding and
    /// static sensory field; this type only records which physical sites form the two
    /// patches for an evaluator after the controller has acted.
    pub fn build_reacquisition(founder: Founder, layout_seed: u64) -> ReacquisitionArena {
        let (arena, initial_patch) =
            Self::build_kind(founder, layout_seed, LayoutKind::Reacquisition);
        let initial_patch = initial_patch.expect("the two-patch layout records its initial patch");
        let successor_patch = arena
            .resources
            .iter()
            .copied()
            .find(|site| *site != initial_patch)
            .expect("the two-patch layout records its successor patch");
        ReacquisitionArena {
            arena,
            initial_patch,
            successor_patch,
        }
    }

    fn build_kind(founder: Founder, layout_seed: u64, kind: LayoutKind) -> (Arena, Option<Site>) {
        let config = VoxelConfig {
            width: ARENA_WIDTH,
            height: ARENA_HEIGHT,
            depth: ARENA_DEPTH,
            voxel_m: ARENA_VOXEL_M,
            seed: layout_seed,
            ..VoxelConfig::default()
        };
        let mut world = World::empty(config);
        world.apply(WorldCommand::SetOutlet { open: false });

        // Ground: soil 1..=GROUND_Y, bedrock already at 0. Wrap the x walk so the seam
        // column is an ordinary interior column.
        for z in 0..ARENA_DEPTH {
            for x in 0..ARENA_WIDTH as i64 {
                for y in 1..=GROUND_Y {
                    world.apply(WorldCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }

        let mut rng = Rng::new(layout_seed);
        let voxel_volume = ARENA_VOXEL_M * ARENA_VOXEL_M * ARENA_VOXEL_M;
        let mut pond = vec![false; (ARENA_WIDTH * ARENA_DEPTH) as usize];
        // A shallow prepared pond: a rock-free patch of ground with free water standing on
        // it. One direct write, never solved, and the static schedule never moves it.
        let px = rng.below(ARENA_WIDTH as usize) as i64;
        let pz = rng.below(ARENA_DEPTH as usize) as u32;
        for dz in 0..2u32 {
            for dx in 0..4i64 {
                let x = px + dx;
                let z = pz + dz;
                if z >= ARENA_DEPTH {
                    continue;
                }
                let idx =
                    (x.rem_euclid(ARENA_WIDTH as i64) as usize) * ARENA_DEPTH as usize + z as usize;
                pond[idx] = true;
                let _ = world.apply(WorldCommand::AddWater {
                    x,
                    y: GROUND_Y + 1,
                    z,
                    volume_m3: POND_FILL * voxel_volume,
                });
            }
        }

        let mut flora = Flora::new(FloraConfig::default());

        // Candidate support faces: soil ground, not the pond, and — unless this layout is
        // the seam layout — at least two columns off the wrapped seam, so a default layout
        // does not accidentally test wrap geometry.
        let seam_layout = layout_seed % 4 == 3;
        let mut candidates: Vec<Site> = Vec::new();
        for z in 0..ARENA_DEPTH {
            for x in 0..ARENA_WIDTH as i64 {
                let idx = (x as usize) * ARENA_DEPTH as usize + z as usize;
                if pond[idx] {
                    continue;
                }
                if !seam_layout && (x < 2 || x >= ARENA_WIDTH as i64 - 2) {
                    continue;
                }
                candidates.push(Site {
                    x: x as u32,
                    y: GROUND_Y,
                    z,
                });
            }
        }
        // A seam layout lays its first tile on the wrapped seam column. The seam site is
        // drawn from the **same** dry candidate pool and removed from it before the other
        // picks, so a resource site can never be selected twice (the flaw that made seed
        // 95 lay `(0,4,8)` twice). Every later pick is also a `swap_remove` off `candidates`.
        let (mut resources, initial_patch) = match kind {
            LayoutKind::StageA => {
                let mut resources: Vec<Site> = Vec::new();
                if seam_layout {
                    let seam: Vec<Site> = candidates.iter().copied().filter(|s| s.x == 0).collect();
                    if !seam.is_empty() {
                        let pick = seam[rng.below(seam.len())];
                        let at = candidates
                            .iter()
                            .position(|s| *s == pick)
                            .expect("the seam candidate is in the pool");
                        candidates.swap_remove(at);
                        resources.push(pick);
                    }
                }
                let want = RESOURCE_TILES
                    .saturating_sub(resources.len())
                    .min(candidates.len());
                for _ in 0..want {
                    let pick = rng.below(candidates.len());
                    resources.push(candidates.swap_remove(pick));
                }
                (resources, None)
            }
            LayoutKind::Reacquisition => {
                // The first patch uses the same dry support pool as Stage A. Its successor
                // is at least 2 m away (64 squared arena columns), leaving a genuine
                // reacquisition leg without manufacturing a route or a movement rule.
                let initial_at = rng.below(candidates.len());
                let initial = candidates.swap_remove(initial_at);
                let successors: Vec<Site> = candidates
                    .iter()
                    .copied()
                    .filter(|site| arena_distance_squared(*site, initial) >= 64)
                    .collect();
                let successor = successors[rng.below(successors.len())];
                let at = candidates
                    .iter()
                    .position(|site| *site == successor)
                    .expect("the successor remains in the dry candidate pool");
                candidates.swap_remove(at);
                (vec![initial, successor], Some(initial))
            }
        };
        resources.sort();

        for &site in &resources {
            match founder {
                Founder::Blind => {
                    let organic = LITTER_PER_TILE;
                    let accepted = flora.deposit(
                        site,
                        Deposit {
                            kind: DepositKind::Litter,
                            organic,
                            mineral: organic * LITTER_MINERAL_FRACTION,
                            energy: organic * LITTER_ENERGY_DENSITY,
                        },
                    );
                    debug_assert!(accepted, "a litter deposit on a support face");
                }
                Founder::Browser => {
                    let wood = 0.5 * flora.config().species(Plant::Springturf).wood_max;
                    let accepted = flora.apply(
                        &world,
                        FloraCommand::Seed {
                            x: i64::from(site.x),
                            z: site.z,
                            species: Plant::Springturf,
                            wood,
                        },
                    );
                    debug_assert!(accepted, "a springturf founder on a support face");
                }
            }
        }

        // Stage A begins off food but already inside a useful signal. Pick a dry support
        // two to four columns from a resource. Two columns keeps the mouth out of feeding
        // contact; four keeps low foliage large enough for the deliberately sparse ray
        // fan and remains inside the settled litter gradient. The pool and pick remain
        // seed-deterministic.
        let start_targets: &[Site] = initial_patch
            .as_ref()
            .map_or(resources.as_slice(), std::slice::from_ref);
        let mut starts: Vec<(Site, Site, i64)> = candidates
            .iter()
            .copied()
            .filter(|s| !resources.contains(s))
            .filter_map(|s| {
                start_targets
                    .iter()
                    .copied()
                    .map(|r| (r, arena_distance_squared(s, r)))
                    .min_by_key(|(_, distance)| *distance)
                    .filter(|(_, distance)| (4..=16).contains(distance))
                    .map(|(target, distance)| (s, target, distance))
            })
            .collect();
        starts.sort_by_key(|(site, target, distance)| (*distance, *site, *target));
        let start = (!starts.is_empty()).then(|| starts[rng.below(starts.len())]);

        let mut fauna = Fauna::new(FaunaConfig::default());
        // Isolated arenas disable paid births (plan, "Frozen arena contract").
        fauna.set_births_enabled(false);
        let manifest = founder.manifest();
        let mut animal_id = None;
        if let Some((site, target, _)) = start {
            let body = manifest.body_reference;
            // Aim generally at the in-signal resource while retaining deterministic
            // heading variation. The browser's central ray fan spans +/-30 degrees, so
            // this +/-5 degree offset keeps the low foliage visible at the first sample.
            // The blind founder receives the same variation without privileged runtime
            // information; only arena construction uses the target.
            let dx = wrapped_dx(site.x, target.x) as f64;
            let dz = f64::from(target.z) - f64::from(site.z);
            let toward = dx.atan2(dz);
            let jitter_steps = (rng.next_u64() % 3) as i64 - 1;
            let heading_rad = toward + (jitter_steps as f64 * 5.0_f64.to_radians());
            if fauna.apply(
                &world,
                FaunaCommand::IntroduceFounder {
                    x: i64::from(site.x),
                    z: site.z,
                    founder,
                    body,
                    heading_rad,
                },
            ) {
                animal_id = Some(fauna.view().ledger.births - 1);
            }
        }

        (
            Arena {
                founder,
                layout_seed,
                world,
                flora,
                fauna,
                resources,
                animal_id,
            },
            initial_patch,
        )
    }

    /// The live finite stock in the resource layout: litter organic matter for a blind
    /// arena, stand foliage for a browser arena. This is what a bite lowers.
    pub fn resource_stock(&self) -> f64 {
        let fv = self.flora.view();
        self.resources
            .iter()
            .map(|&site| match self.founder {
                Founder::Blind => fv.ground_at(site).map_or(0.0, |g| g.litter),
                Founder::Browser => fv.stand_at(site).map_or(0.0, |s| s.foliage),
            })
            .sum()
    }

    /// The placed founder's pose, if it landed.
    pub fn animal_pose(&self) -> Option<Pose> {
        self.animal_id
            .and_then(|id| self.fauna.view().animal(id))
            .map(|a| a.pose)
    }

    /// Take up to `want` of the finite stock off one resource site, through the plant
    /// layer's production withdrawals — the same call P1-B's litter feeder and browser will
    /// make. `None` when the site holds nothing.
    pub fn take(&mut self, site: Site, want: f64) -> Option<Taken> {
        match self.founder {
            Founder::Blind => self.flora.take_litter(site, want),
            Founder::Browser => self.flora.take_foliage(site, want),
        }
    }

    /// Settle a reusable cue field for this exact frozen source layout. Episode-private
    /// trend history is still reset when the field enters a simulation.
    pub fn prepare_senses(&self) -> Senses {
        let mut senses = Senses::new();
        senses.settle(&self.world.view(), &self.flora.view());
        senses
    }

    /// Move the built layers into a [`Sim`] in [`crate::ScheduleMode::Static`].
    pub fn into_sim(self, config: crate::SimConfig) -> Sim {
        Sim::new_static(self.world, self.flora, self.fauna, config)
    }

    /// Move the built layers into a static [`Sim`] with a field already settled for this
    /// arena's unchanged source layout.
    pub fn into_sim_prepared(self, config: crate::SimConfig, senses: Senses) -> Sim {
        Sim::new_static_prepared(self.world, self.flora, self.fauna, config, senses)
    }
}

/// Shortest signed x-column displacement on the wrapped arena strip.
fn wrapped_dx(from: u32, to: u32) -> i64 {
    let width = i64::from(ARENA_WIDTH);
    let raw = i64::from(to) - i64::from(from);
    [raw, raw - width, raw + width]
        .into_iter()
        .min_by_key(|delta| delta.abs())
        .expect("three wrapped displacements")
}

fn arena_distance_squared(a: Site, b: Site) -> i64 {
    let dx = wrapped_dx(a.x, b.x);
    let dz = i64::from(b.z) - i64::from(a.z);
    dx * dx + dz * dz
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ScheduleMode, SimConfig};

    use cubarium_voxel_fauna::{Actions, Controller, Response, Scripted};

    const SEEDS: [u64; 4] = [1, 2, 3, 95];
    const STAGE_A_SEEDS: [u64; 12] = [1, 2, 4, 5, 6, 8, 9, 10, 13, 14, 17, 18];
    const REACQUISITION_SEEDS: [u64; 4] = [1, 2, 17, 95];

    /// [`Arena::build`]'s resource sites are always unique, including the seam layout
    /// (seeds `% 4 == 3`), where the seam tile used to collide with an interior pick
    /// (seed 95 laid `(0,4,8)` twice).
    #[test]
    fn resource_sites_are_unique_across_seeds() {
        for seed in SEEDS {
            for founder in Founder::ALL {
                let arena = Arena::build(founder, seed);
                let mut sorted = arena.resources.clone();
                sorted.sort();
                let n = sorted.len();
                sorted.dedup();
                assert_eq!(
                    sorted.len(),
                    n,
                    "{founder:?} seed {seed}: a resource site was selected twice"
                );
                if seed % 4 == 3 {
                    assert!(
                        sorted.iter().any(|s| s.x == 0),
                        "{founder:?} seed {seed}: the seam layout did not keep a seam tile"
                    );
                }
            }
        }
    }

    #[test]
    fn the_arena_is_the_frozen_size_with_finite_food() {
        for seed in SEEDS {
            for founder in Founder::ALL {
                let arena = Arena::build(founder, seed);
                let c = arena.world.config();
                assert_eq!((c.width, c.height, c.depth, c.voxel_m), (32, 16, 12, 0.25));
                assert!(!arena.resources.is_empty(), "{founder:?} seed {seed}");
                assert!(
                    arena.resource_stock() > 0.0,
                    "{founder:?} seed {seed}: the layout is not empty"
                );
                assert!(!arena.fauna.births_enabled(), "arenas disable paid births");
                let animal = arena
                    .animal_id
                    .and_then(|id| arena.fauna.view().animal(id))
                    .expect("an idle founder is placed");
                assert_eq!(animal.founder, Some(founder));
                assert!(arena.resources.iter().all(|r| *r != animal.site));
            }
        }
    }

    #[test]
    fn stage_a_starts_off_food_inside_signal_and_facing_it() {
        for seed in STAGE_A_SEEDS {
            for founder in Founder::ALL {
                let arena = Arena::build(founder, seed);
                let animal = arena
                    .animal_id
                    .and_then(|id| arena.fauna.view().animal(id))
                    .expect("the Stage A founder is placed");
                let (target, distance) = arena
                    .resources
                    .iter()
                    .copied()
                    .map(|r| (r, arena_distance_squared(animal.site, r)))
                    .min_by_key(|(_, distance)| *distance)
                    .expect("the arena has resources");
                assert!(
                    (4..=16).contains(&distance),
                    "{founder:?} seed {seed}: start {:?}, nearest {:?}, distance^2 {distance}",
                    animal.site,
                    target,
                );

                let toward = (wrapped_dx(animal.site.x, target.x) as f64)
                    .atan2(f64::from(target.z) - f64::from(animal.site.z));
                let heading_error = (animal.pose.heading_rad - toward)
                    .sin()
                    .atan2((animal.pose.heading_rad - toward).cos());
                assert!(
                    heading_error.abs() <= 5.0_f64.to_radians() + 1e-12,
                    "{founder:?} seed {seed}: heading error {} degrees",
                    heading_error.to_degrees(),
                );
            }
        }
    }

    /// Stage B is an explicit two-patch task, without changing Stage A's six-resource
    /// fixture. The sites are fixture metadata only; the founder remains off food and
    /// begins in the ordinary local signal range of its first finite patch.
    #[test]
    fn reacquisition_layouts_are_two_finite_deterministic_patches() {
        for seed in REACQUISITION_SEEDS {
            for founder in Founder::ALL {
                let mut stage = Arena::build_reacquisition(founder, seed);
                let again = Arena::build_reacquisition(founder, seed);
                assert_eq!(stage.arena.resources, again.arena.resources);
                assert_eq!(stage.initial_patch, again.initial_patch);
                assert_eq!(stage.successor_patch, again.successor_patch);
                assert_eq!(stage.arena.resources.len(), 2);
                assert_ne!(stage.initial_patch, stage.successor_patch);
                assert!(stage.arena.resources.contains(&stage.initial_patch));
                assert!(stage.arena.resources.contains(&stage.successor_patch));
                assert!(stage.arena.resource_stock() > 0.0);
                assert!(
                    patch_stock(&stage.arena, stage.initial_patch) > 0.0,
                    "{founder:?} seed {seed}: initial patch is not finite"
                );
                assert!(
                    patch_stock(&stage.arena, stage.successor_patch) > 0.0,
                    "{founder:?} seed {seed}: successor patch is not finite"
                );
                assert!(
                    arena_distance_squared(stage.initial_patch, stage.successor_patch) >= 64,
                    "{founder:?} seed {seed}: successor is not a separate reacquisition leg"
                );
                let animal = stage
                    .arena
                    .animal_id
                    .and_then(|id| stage.arena.fauna.view().animal(id))
                    .expect("a Stage B founder is placed");
                assert!(
                    (4..=16).contains(&arena_distance_squared(animal.site, stage.initial_patch)),
                    "{founder:?} seed {seed}: founder did not begin in its initial patch's signal"
                );
                let initial_stock = patch_stock(&stage.arena, stage.initial_patch);
                let successor_stock = patch_stock(&stage.arena, stage.successor_patch);
                let taken = stage
                    .arena
                    .take(stage.initial_patch, initial_stock)
                    .expect("each finite patch depletes through its founder's real source");
                assert!(taken.organic > 0.0, "{founder:?} seed {seed}");
                assert_eq!(patch_stock(&stage.arena, stage.initial_patch), 0.0);
                assert_eq!(
                    patch_stock(&stage.arena, stage.successor_patch),
                    successor_stock,
                    "{founder:?} seed {seed}: depletion crossed into the successor patch"
                );
            }
        }
    }

    /// The seeded Stage B placement always has enough dry support for two separated
    /// patches and an in-signal founder start. This is a construction sweep, not a long
    /// simulation study.
    #[test]
    fn reacquisition_placement_is_safe_across_a_broad_seed_sample() {
        for seed in 0..256 {
            for founder in Founder::ALL {
                let stage = Arena::build_reacquisition(founder, seed);
                assert_eq!(stage.arena.resources.len(), 2, "{founder:?} seed {seed}");
                assert!(
                    patch_stock(&stage.arena, stage.initial_patch) > 0.0,
                    "{founder:?} seed {seed}: initial patch is not finite"
                );
                assert!(
                    patch_stock(&stage.arena, stage.successor_patch) > 0.0,
                    "{founder:?} seed {seed}: successor patch is not finite"
                );
                assert!(
                    stage.arena.animal_id.is_some(),
                    "{founder:?} seed {seed}: no in-signal founder start"
                );
            }
        }
    }

    /// The static arena steps the world's and the fauna's clocks, ages and maintains the
    /// body, and **holds the terrain, water and unrelated plants still**. No motion, no
    /// feeding. The plant check is the stands' actual state — species, site, wood and
    /// foliage — not a count, so an empty-to-less-empty stand shuffle cannot satisfy it.
    #[test]
    fn a_static_arena_advances_time_and_freezes_the_world() {
        let arena = Arena::build(Founder::Browser, 7);
        let before_material = arena.world.view().material.to_vec();
        let before_free = arena.world.view().free.to_vec();
        let before_stands: Vec<_> = arena
            .flora
            .view()
            .stands
            .iter()
            .map(|s| (s.site, s.species, s.wood, s.foliage))
            .collect();
        let before_stock = arena.resource_stock();
        let before_animal = *arena.fauna.view().animal(arena.animal_id.unwrap()).unwrap();

        let mut sim = arena.into_sim(SimConfig { threads: 1 });
        assert_eq!(sim.mode(), ScheduleMode::Static);
        for _ in 0..10 {
            sim.step();
        }

        assert_eq!(sim.world().tick(), 10);
        assert_eq!(sim.fauna().tick(), 10);
        assert_eq!(
            sim.world().view().material,
            before_material,
            "terrain moved"
        );
        assert_eq!(sim.world().view().free, before_free, "water moved");
        let after_stands: Vec<_> = sim
            .flora()
            .view()
            .stands
            .iter()
            .map(|s| (s.site, s.species, s.wood, s.foliage))
            .collect();
        assert_eq!(after_stands, before_stands, "a stand grew, died or moved");

        let after = *sim
            .fauna()
            .view()
            .animal(before_animal.id)
            .expect("the idle body survives ten ticks");
        assert_eq!(after.age_ticks, before_animal.age_ticks + 10);
        assert!(
            after.organic() < before_animal.organic(),
            "maintenance was paid out of the body's own organic matter"
        );
        assert_eq!(after.site, before_animal.site, "the idle body moved");
        assert_eq!(after.pose, before_animal.pose, "the idle body's pose moved");
        assert_eq!(after.state, cubarium_voxel_fauna::State::Resting);

        let (_, flora, _) = sim.layers();
        let stock = match before_animal.founder {
            Some(Founder::Browser) => arena_stock(flora),
            _ => unreachable!("browser arena"),
        };
        assert_eq!(stock, before_stock, "the idle body ate nothing");
    }

    fn arena_stock(flora: &Flora) -> f64 {
        // The browser layout's stock: every springturf stand's foliage.
        flora
            .view()
            .stands
            .iter()
            .filter(|s| s.species == Plant::Springturf)
            .map(|s| s.foliage)
            .sum()
    }

    fn patch_stock(arena: &Arena, site: Site) -> f64 {
        match arena.founder {
            Founder::Blind => arena.flora.view().ground_at(site).map_or(0.0, |g| g.litter),
            Founder::Browser => arena.flora.view().stand_at(site).map_or(0.0, |s| s.foliage),
        }
    }

    fn module_channel(observation: &[f64], founder: Founder, module: &str, channel: &str) -> f64 {
        let manifest = founder.manifest();
        let module = manifest
            .modules
            .iter()
            .find(|candidate| candidate.name == module)
            .unwrap_or_else(|| panic!("{founder:?} manifest has no {module} module"));
        let channel = module
            .channels
            .iter()
            .position(|candidate| *candidate == channel)
            .unwrap_or_else(|| panic!("{} has no {channel} channel", module.name));
        observation[module.offset + channel]
    }

    /// The finite stock is real and earnable through the production withdrawals.
    #[test]
    fn the_finite_food_is_actually_takeable() {
        let mut arena = Arena::build(Founder::Blind, 1);
        let site = arena.resources[0];
        let before = arena.resource_stock();
        let taken = arena.take(site, 0.05).expect("litter off the tile");
        assert!(taken.organic > 0.0);
        assert!(arena.resource_stock() < before);

        let mut browser = Arena::build(Founder::Browser, 1);
        let site = browser.resources[0];
        let before = browser.resource_stock();
        let taken = browser.take(site, 0.01).expect("foliage off the stand");
        assert!(taken.organic > 0.0);
        assert!(browser.resource_stock() < before);
    }

    /// Depleting Stage B's first patch goes through the ordinary flora withdrawal. A new
    /// settled field then removes its contact/taste signal while the real successor
    /// remains chemically and mechanically present; no fixture-only target marker exists.
    #[test]
    fn reacquisition_depletion_updates_the_real_resource_and_sensory_sources() {
        let mut stage = Arena::build_reacquisition(Founder::Blind, 17);
        let initial = stage.initial_patch;
        let successor = stage.successor_patch;
        let before = stage.arena.resource_stock();
        let taken = stage
            .arena
            .take(initial, 1.0)
            .expect("the initial litter patch is finite and takeable");
        assert!(taken.organic > 0.0);
        assert!(stage.arena.resource_stock() < before);

        let prepared = stage.arena.prepare_senses();
        let initial_obs = first_sample_of_a_founder_on_the_tile(
            stage
                .arena
                .into_sim_prepared(SimConfig { threads: 1 }, prepared),
            initial,
        );

        let stage = Arena::build_reacquisition(Founder::Blind, 17);
        let prepared = stage.arena.prepare_senses();
        let successor_obs = first_sample_of_a_founder_on_the_tile(
            stage
                .arena
                .into_sim_prepared(SimConfig { threads: 1 }, prepared),
            successor,
        );
        assert_eq!(
            initial_obs.len(),
            Founder::Blind.manifest().inputs(),
            "the controller receives only its declared manifest vector"
        );
        assert_eq!(successor_obs.len(), initial_obs.len());
        assert_eq!(
            module_channel(&initial_obs, Founder::Blind, "Taste(1)", "cue"),
            0.0,
            "depleted litter has no mouth cue"
        );
        assert!(
            module_channel(&successor_obs, Founder::Blind, "Taste(1)", "cue") > 0.0,
            "the successor still has mouth cue"
        );
        assert_eq!(
            module_channel(&initial_obs, Founder::Blind, "Taste(1)", "valid"),
            1.0,
            "bare ground remains a valid taste contact"
        );
        let initial_chem = module_channel(&initial_obs, Founder::Blind, "Chem(litter)", "response");
        let successor_chem =
            module_channel(&successor_obs, Founder::Blind, "Chem(litter)", "response");
        assert!(
            successor_chem > initial_chem,
            "the surviving successor remains a stronger local cue: initial {}, successor {}",
            initial_chem,
            successor_chem,
        );
    }

    /// The live schedule is still the default; only an explicit static construction moves
    /// off it.
    #[test]
    fn live_is_still_the_default_mode() {
        let sim = Sim::from_configs(
            VoxelConfig::default(),
            FloraConfig::default(),
            FaunaConfig::default(),
            SimConfig { threads: 1 },
        );
        assert_eq!(sim.mode(), ScheduleMode::Live);
    }

    /// A founder driven through the production static schedule: the controller stage
    /// runs inside `sys_fauna`, the held actions move the body, and a held feed crops
    /// a stand through the same real withdrawals. No special arena path exists.
    #[test]
    fn a_controller_driven_founder_moves_and_feeds_through_the_static_schedule() {
        let arena = Arena::build(Founder::Browser, 11);
        let stand_site = arena.resources[0];
        let stock_before = arena.resource_stock();
        let idle_id = arena.animal_id.expect("the arena placed an idle body");

        // A founder body standing in the stand it will crop, off the arena's own idle
        // body (which has no controller and stays exactly where P1-A left it).
        let mut sim = arena.into_sim(SimConfig { threads: 1 });
        let (feeder, cruiser) = sim.with_layers_mut(|world, _, fauna| {
            let mut introduce = |z: u32| {
                assert!(fauna.apply(
                    world,
                    FaunaCommand::IntroduceFounder {
                        x: i64::from(stand_site.x),
                        z,
                        founder: Founder::Browser,
                        body: 0.04,
                        heading_rad: 0.0,
                    },
                ));
                fauna.view().ledger.births - 1
            };
            // The feeder stands in the stand it will crop; the cruiser starts beside
            // it and just drives.
            let feeder = introduce(stand_site.z);
            let cruiser = introduce(stand_site.z + 3);
            (feeder, cruiser)
        });
        assert!(sim.fauna_mut().set_controller(
            feeder,
            Box::new(Scripted::new(vec![Actions {
                forward: 0.0,
                turn: 0.0,
                feed: 1.0,
            }])),
        ));
        assert!(sim.fauna_mut().set_controller(
            cruiser,
            Box::new(Scripted::new(vec![Actions {
                forward: 1.0,
                turn: 0.0,
                feed: 0.0,
            }])),
        ));

        // Five controller periods: five feed attempts for the stand's resident, and a
        // moved body for the cruiser.
        for _ in 0..25 {
            sim.step();
        }

        let a = sim
            .fauna()
            .view()
            .animal(feeder)
            .expect("the driven founder survived");
        assert_eq!(
            a.pose,
            cubarium_voxel_fauna::Pose::at_site(stand_site, ARENA_VOXEL_M),
            "a feeding founder stands still"
        );
        assert!(
            a.founder_state.feedback.attempted_equivalent == 0.0,
            "no motion was requested, so none was attempted"
        );
        assert!(
            sim.fauna().view().ledger.bites >= 4,
            "the held feed attempted once per interval: {} bites",
            sim.fauna().view().ledger.bites
        );
        assert!(
            sim.fauna().view().ledger.eaten_organic_in > 0.0,
            "the bites transferred real foliage"
        );
        let c = sim.fauna().view().animal(cruiser).unwrap();
        assert_ne!(
            c.pose,
            cubarium_voxel_fauna::Pose::at_site(
                Site {
                    x: stand_site.x,
                    y: stand_site.y,
                    z: stand_site.z + 3
                },
                ARENA_VOXEL_M
            ),
            "the cruiser's held forward action moved it"
        );
        // The browser layout's finite stock: every springturf stand's foliage, read
        // through the sim's flora — the same read `Arena::resource_stock` does.
        let stock_after: f64 = sim
            .flora()
            .view()
            .stands
            .iter()
            .filter(|s| s.species == cubarium_voxel_flora::Species::Springturf)
            .map(|s| s.foliage)
            .sum();
        assert!(
            stock_after < stock_before,
            "the finite stock went down through the production withdrawals"
        );

        // The idle founder body without a controller did not move.
        let idle = sim.fauna().view().animal(idle_id).unwrap();
        assert_eq!(idle.state, cubarium_voxel_fauna::State::Resting);
    }

    /// A controller that records every observation and rests: the probe for what the
    /// static tick's controller stage actually sampled.
    struct Recorder {
        log: std::sync::Arc<std::sync::Mutex<Vec<Vec<f64>>>>,
    }

    impl Recorder {
        fn new() -> (Recorder, std::sync::Arc<std::sync::Mutex<Vec<Vec<f64>>>>) {
            let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            (Recorder { log: log.clone() }, log)
        }
    }

    impl Controller for Recorder {
        fn drive(&mut self, observation: &[f64]) -> Response {
            self.log.lock().unwrap().push(observation.to_vec());
            Response::Bounded(cubarium_voxel_fauna::Actions::REST)
        }

        fn reset(&mut self) {
            self.log.lock().unwrap().clear();
        }
    }

    fn first_sample_of_built_founder(mut sim: Sim, id: u64) -> Vec<f64> {
        let (recorder, log) = Recorder::new();
        assert!(sim.fauna_mut().set_controller(id, Box::new(recorder)));
        for _ in 0..6 {
            sim.step();
        }
        let log = log.lock().unwrap();
        assert_eq!(log.len(), 1, "one sampling in six ticks");
        log[0].clone()
    }

    #[test]
    fn stage_a_starts_present_the_promised_initial_signal() {
        for seed in STAGE_A_SEEDS {
            for founder in Founder::ALL {
                let arena = Arena::build(founder, seed);
                let id = arena.animal_id.expect("the Stage A founder is placed");
                let start = arena.animal_pose();
                let resources = arena.resources.clone();
                let obs =
                    first_sample_of_built_founder(arena.into_sim(SimConfig { threads: 1 }), id);
                match founder {
                    Founder::Blind => {
                        assert_eq!(obs[20], 1.0, "seed {seed}: chemical receptor valid");
                        assert!(obs[18] > 0.0, "seed {seed}: start is outside the cue");
                    }
                    Founder::Browser => {
                        assert_eq!(obs[36], 1.0, "seed {seed}: material cone valid");
                        let foliage = obs[20] + obs[26] + obs[32];
                        assert!(
                            foliage > 0.0,
                            "seed {seed}: no starting foliage ray hit; start {start:?}, resources {resources:?}, observation {obs:?}",
                        );
                    }
                }
            }
        }
    }

    /// Put a resting, recording founder on a litter tile of a static sim and return its
    /// first sampled observation.
    fn first_sample_of_a_founder_on_the_tile(mut sim: Sim, site: Site) -> Vec<f64> {
        let id = sim.with_layers_mut(|world, _, fauna| {
            assert!(fauna.apply(
                world,
                FaunaCommand::IntroduceFounder {
                    x: i64::from(site.x),
                    z: site.z,
                    founder: Founder::Blind,
                    body: 0.0125,
                    heading_rad: 0.0,
                },
            ));
            fauna.view().ledger.births - 1
        });
        let (recorder, log) = Recorder::new();
        assert!(sim.fauna_mut().set_controller(id, Box::new(recorder)));
        // Six ticks: the first sampling is at age 5, before the tick's own first field
        // update at tick 10 — whatever the controller read was settled in.
        for _ in 0..6 {
            sim.step();
        }
        let log = log.lock().unwrap();
        assert_eq!(log.len(), 1, "one sampling in six ticks");
        log[0].clone()
    }

    /// The static tick samples the **settled field**: a founder standing on a litter tile
    /// reads a valid, nonzero `Chem(litter)` at its very first sampling. Taste separately
    /// reports the material at its current mouth contact. The live schedule, by contrast,
    /// has no field at all.
    #[test]
    fn the_static_tick_samples_the_settled_field() {
        let arena = Arena::build(Founder::Blind, 1);
        let site = arena.resources[0];
        let sim = arena.into_sim(SimConfig { threads: 1 });
        assert!(
            sim.senses().is_some(),
            "a static arena holds its settled field"
        );
        let obs = first_sample_of_a_founder_on_the_tile(sim, site);
        assert_eq!(obs.len(), 23);
        assert_eq!(obs[20], 1.0, "chem validity from the settled field");
        assert!(
            obs[18] > 0.0,
            "a cue stands on the source tile: {}",
            obs[18]
        );
        assert_eq!(obs[17], 1.0, "taste validity");
        assert!(obs[15] > 0.0, "litter is present at the mouth contact");
        assert_eq!(obs[21..23], [1.0, 1.0], "open-sky light, valid");

        // The live schedule is senses-free: no field resource, and its fauna path is
        // exactly what it was.
        let live = Sim::from_configs(
            VoxelConfig::default(),
            cubarium_voxel_flora::FloraConfig::default(),
            cubarium_voxel_fauna::FaunaConfig::default(),
            SimConfig { threads: 1 },
        );
        assert!(live.senses().is_none());
        assert_eq!(live.mode(), ScheduleMode::Live);
    }

    /// A prepared field is reused: a sim built from a caller-settled [`Senses`] reads the
    /// settled cue on the first sampling — a fresh field would still read zero there,
    /// because the tick's own first update is not due until tick 10. The episode-private
    /// trend stores are cleared on the way in, so the first sample's trend is zero.
    #[test]
    fn a_prepared_field_is_reused_on_the_first_sampling() {
        let arena = Arena::build(Founder::Blind, 1);
        let site = arena.resources[0];
        // Settle once through the same API the search fixture caches.
        let prepared = arena.prepare_senses();
        let sim = arena.into_sim_prepared(SimConfig { threads: 1 }, prepared);
        let obs = first_sample_of_a_founder_on_the_tile(sim, site);
        assert_eq!(obs[20], 1.0, "chem validity");
        assert!(
            obs[18] > 0.0,
            "the prepared field was readable before any live update: {}",
            obs[18]
        );
        assert_eq!(obs[19], 0.0, "the first sample's trend is zero, not stale");
    }

    /// The setup and sensing numbers on the arena's own scale: build, settle (inside
    /// `new_static`), and the controller stage's sampling — one observation build with
    /// the cone — per step. Ignored because it is a named study, not CI work
    /// (`design/voxel-senses-phase1-tests.md` §3: measure setup separately from ticks);
    /// run it by name when the benchmark needs the current figures.
    #[test]
    #[ignore = "study: run by name for the setup and per-observation numbers"]
    fn the_setup_and_sampling_costs_are_measured() {
        use std::time::Instant;

        for founder in Founder::ALL {
            let t0 = Instant::now();
            let arena = Arena::build(founder, 1);
            let build = t0.elapsed();

            // An explicit settle, so the update count and the per-update cost are on
            // record alongside into_sim's own settle.
            let mut probe = cubarium_voxel_fauna::Senses::new();
            let t1 = Instant::now();
            let (updates, converged) = probe.settle(&arena.world.view(), &arena.flora.view());
            let settle = t1.elapsed();

            let t2 = Instant::now();
            let mut sim = arena.into_sim(SimConfig { threads: 1 });
            let into_sim = t2.elapsed();

            // One driven founder on an interior column: every controller period samples
            // one observation (blind: field + receptors; browser: 27-ray cone + receptors).
            // The idle body the builder placed samples too — two observers.
            let id = sim.with_layers_mut(|world, _, fauna| {
                assert!(world.view().surface_y(8, 4).is_some(), "an interior column");
                assert!(fauna.apply(
                    world,
                    FaunaCommand::IntroduceFounder {
                        x: 8,
                        z: 4,
                        founder,
                        body: founder.manifest().body_reference,
                        heading_rad: 0.0,
                    },
                ));
                fauna.view().ledger.births - 1
            });
            let (recorder, log) = Recorder::new();
            assert!(sim.fauna_mut().set_controller(id, Box::new(recorder)));

            let t3 = Instant::now();
            let steps = 100;
            for _ in 0..steps {
                sim.step();
            }
            let sampled = log.lock().unwrap().len() as f64;
            let tick_with_sampling = t3.elapsed() / steps;

            // The animal-free baseline, so the per-observation share is visible: remove
            // every body and time the same tick count.
            let sites: Vec<_> = sim
                .fauna()
                .view()
                .animals
                .iter()
                .map(|a| (i64::from(a.site.x), a.site.z))
                .collect();
            sim.with_layers_mut(|world, _, fauna| {
                for (x, z) in sites {
                    while fauna.apply(world, FaunaCommand::Remove { x, z }) {}
                }
            });
            let t4 = Instant::now();
            for _ in 0..steps {
                sim.step();
            }
            let tick_empty = t4.elapsed() / steps;
            println!(
                "{}: build {build:?}, settle {updates} updates in {settle:?} (converged \
                 {converged}), into_sim {into_sim:?}, tick+2 observers {tick_with_sampling:?}, \
                 tick empty {tick_empty:?}, per-observation ≈ {:?} ({} samplings of the driven \
                 body, the idle body samples too)",
                founder.name(),
                (tick_with_sampling
                    .checked_sub(tick_empty)
                    .unwrap_or_default()
                    * steps)
                    / (2 * sampled as u32),
                sampled,
            );
        }
    }
}
