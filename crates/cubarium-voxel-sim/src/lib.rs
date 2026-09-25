//! **The voxel tick's backbone.** One [`bevy_ecs`] world holding the three layers as
//! resources, and one [`Schedule`] whose systems are the tick's phases in the order the
//! ledger needs.
//!
//! Wrysk (2026-09-18) decided the sim uses a real ECS crate rather than hand-rolled sets,
//! with multithreading early; the brief is
//! `design/handoffs/voxel-schedule-brief-2026-09-18.md` and the measurements it is aimed
//! at are `design/7_Research/voxel-tick-profile-2026-09-18.md`.
//!
//! # What is an entity here, and what is not
//!
//! **Nothing, yet.** The voxel grid is a dense resource and never one entity per voxel:
//! `material` is static, every water rule needs random access, and 147,456 entities would
//! buy nothing but archetype churn. Stands and animals stay in their own crates' sorted
//! stores this package too — they become entities when the senses spec
//! (`design/handoffs/voxel-senses-handoff-2026-09-18.md`) rewrites the body model, which
//! is the round that gives them components worth querying. So what this crate buys today
//! is the **schedule**: a declared phase order, system sets a host can hang its own
//! observers on, and one place that owns the thread pool.
//!
//! # Order is the rule; parallelism is inside a phase — and, overlapped, between two legs
//!
//! The tick is a dependency chain — rain feeds infiltration, infiltration feeds the
//! exchange, the exchange feeds what the plants drink, the plants feed the animals. On the
//! **chained** tick ([`Tick`], `SimConfig::overlap` off) **no two phases ever run at the
//! same time**: the schedule is `.chain()`ed and its executor is the single-threaded one.
//! The **overlapped** tick ([`OverlapTick`], the default since
//! `design/handoffs/voxel-phase-overlap-2026-09-24.md`) cuts exactly one link: the plants
//! and animals read the [`LaggedWorld`], the world as it stood when the tick began, so
//! `Water(t) ‖ (Flora(t) → Fauna(t))` run side by side on the multi-threaded executor and
//! the plants' drink is applied at a barrier after both. Within each leg the order is the
//! chained tick's. Speed otherwise comes from splitting the work *inside* a phase:
//!
//! | phase | parallel? | why |
//! | --- | --- | --- |
//! | `rain`, `evaporate` | no | one sky cell per column, a few hundredths of a millisecond on the desktop terrarium |
//! | `infiltrate`, `fall`, `drain` | **yes**, by column | every move is up or down one column; the active sets are per-column row masks each task keeps for its own columns; drainage sums what it hands the aquifer per chunk |
//! | `exchange` | **yes**, by column | read-old/write-new; a proposal into a neighbour's cell is an atomic add |
//! | `water_table` | **yes**, by column, when the aquifer can pay | it shares a scarce stock in index order **by rule**, so when the stock binds it runs in that order on one thread |
//! | `spring`, `outlet` | no | one cell each |
//! | `flora` | no | 0.6 % of the tick; see [`Sim::step`] |
//! | `fauna` | **yes**, its `sense` | one read-only plan per animal, applied serially in id order |
//!
//! Everything that is left serial is left serial for a stated reason and not for want of
//! trying; [`Sim::step`]'s doc carries the list.
//!
//! # Determinism
//!
//! Not a requirement (Wrysk, 2026-09-18: measurements are statistical, never bit-exact),
//! but the two cautions the profile note raised still hold and this crate keeps them: no
//! sum or draw is taken over a thread's arrival order — every parallel phase gathers its
//! results per chunk and folds them back in **chunk order**, so the fold is the same fold
//! the serial pass does — and nothing here iterates a hash map.
//!
//! # Always fresh
//!
//! This crate stores nothing of its own, so it has no schema and refuses no world: a
//! snapshot is still the three layers' own bytes, saved and loaded through them.

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{MultiThreadedExecutor, ScheduleLabel, SingleThreadedExecutor};
use bevy_ecs::system::{NonSendMarker, ScheduleSystem};
use bevy_tasks::{ComputeTaskPool, TaskPoolBuilder};

use cubarium_voxel::water;
use cubarium_voxel_fauna::{Fauna, Senses};
use cubarium_voxel_flora::Flora;

mod arena;
pub mod found;
pub mod habitat;
pub mod scene;
pub use arena::{
    ARENA_BYSTANDERS, ARENA_DEPTH, ARENA_HEIGHT, ARENA_VOXEL_M, ARENA_WIDTH, Arena, ArenaGrid,
    BROWSER_FOLIAGE_PER_STAND, FOUNDER_START, GROUND_Y, PatchKind, Placement,
    REACQUISITION_INITIAL_PATCH_FRACTION, REACQUISITION_LITTER_PER_PATCH, ReacquisitionArena,
    SuccessorBand, edible_stock, populate,
};
/// A support face: the coordinate a resource patch, a stand and a deposit all live at.
/// Re-exported so a caller holding an [`Arena`] can name its patch sites without
/// depending on the plant layer directly.
pub use cubarium_voxel_flora::Site;

/// The sensory state as a bevy resource: the litter field and the per-body trend stores.
///
/// The **static arena** gets one from [`Sim::new_static`] (settled there, before the first
/// tick can sample it) or [`Sim::new_static_prepared`]. The **live schedule** gets one when
/// its caller hands [`Sim::new`] a settled [`Senses`] — the seeded habitat does, so its
/// founders smell the litter the living flora actually drops, decomposes and is eaten off.
/// Either schedule **without** one runs senses-free, which is exactly what the live path
/// was before this resource could reach it. Read through [`Sim::senses`].
#[derive(Resource)]
pub struct SenseField(pub Senses);

/// The voxel grid, free water, pore water and the aquifer: a **dense resource**, never one
/// entity per voxel.
#[derive(Resource)]
pub struct VoxelWorld(pub cubarium_voxel::World);

/// **The overlapped tick's read copy** (`design/handoffs/voxel-phase-overlap-2026-09-24.md`):
/// the world as it stood when the tick began — after the previous tick's water, its
/// barrier and whatever a host commanded between ticks. At the start of every overlapped
/// tick the live world's water arrays are **swapped** into it
/// ([`World::take_readable_from`](cubarium_voxel::World::take_readable_from)), which costs
/// nothing, and the water leg's first act copies them back
/// ([`World::restore_water_from`](cubarium_voxel::World::restore_water_from)) while the
/// plants and animals already read this: the copy is on the water leg, not ahead of
/// both. It is never stepped or commanded. Made on the first overlapped tick.
///
/// **Between the swap and the restore the live [`VoxelWorld`]'s water is stale**: a
/// host's own system in the overlapped tick that reads it belongs in
/// [`TickPhase::Sample`], or at least after [`TickPhase::Water`].
#[derive(Resource)]
pub struct LaggedWorld(pub cubarium_voxel::World);

/// What the overlapped tick's water leg needs to copy back only the cells that can have
/// changed since the last lend ([`cubarium_voxel::WaterLend`]); the water side's, so the
/// read copy stays shared with the plants and animals.
#[derive(Resource, Default)]
pub struct Lend(pub cubarium_voxel::WaterLend);

/// The overlapped tick's planned drink between the plant step and the barrier: taken out
/// of the plant layer when its step ends, withdrawn from the live world as soon as both
/// the water leg and the plant step are done — beside the animal step — and handed back
/// to the layer at the barrier ([`cubarium_voxel_flora::DrinkPlan`]).
#[derive(Resource, Default)]
pub struct PendingDrink(pub Option<cubarium_voxel_flora::DrinkPlan>);

/// The plant layer, with its own sorted stand and ground stores.
#[derive(Resource)]
pub struct FloraLayer(pub Flora);

/// The animal layer, with its own id-ordered animal store.
#[derive(Resource)]
pub struct FaunaLayer(pub Fauna);

/// How the tick is *executed*. Nothing in here is ecology: no rule, number, preset or
/// phase order reads it, and two runs at different thread counts are the same run to
/// within float reassociation.
#[derive(Resource, Clone, Copy, Debug)]
pub struct SimConfig {
    /// Worker threads for the in-phase splits. `1` runs every phase serially and never
    /// touches the pool at all; the default is [`cubarium_voxel::thread_override`] when a
    /// tool pinned one (`threads=`, `CUBARIUM_THREADS`), else
    /// [`std::thread::available_parallelism`].
    ///
    /// **Placeholder** (`design/backlog.md` §1): nothing measured a best value, and the
    /// bench addendum in `design/7_Research/voxel-tick-profile-2026-09-18.md` is the only
    /// evidence there is about which count pays.
    pub threads: usize,
    /// **Overlap the water with the plants and animals** (default on;
    /// `design/handoffs/voxel-phase-overlap-2026-09-24.md`). On, the live tick runs
    /// `Water(t) ‖ (Flora(t) → Fauna(t))` and then a barrier: the plants and animals read
    /// the [`LaggedWorld`] — the world as it stood when the tick began, one tick (50 ms)
    /// behind the water — and the plants' drink is applied to the live world at the
    /// barrier. That lag is the one rule it changes. Off is the chained tick, water then
    /// plants then animals, so two runs can be compared. The static and frozen-water
    /// schedules ignore it.
    pub overlap: bool,
}

impl Default for SimConfig {
    fn default() -> SimConfig {
        SimConfig {
            threads: cubarium_voxel::thread_override().unwrap_or_else(|| {
                std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
            }),
            overlap: true,
        }
    }
}

impl SimConfig {
    /// `threads` workers for every phase, with the default [`SimConfig::overlap`].
    pub fn with_threads(threads: usize) -> SimConfig {
        SimConfig {
            threads,
            ..SimConfig::default()
        }
    }

    /// Under overlap, the two legs' worker counts: `(water, plants and animals)`.
    ///
    /// The water's rayon pool and the compute pool the plants and animals split their
    /// reads over are separate, and the two legs run at once, so both at `threads` can
    /// put twice as many busy threads as cores. **Both at `threads` anyway**: it was the
    /// fastest split measured (eidolon, terrarium seed 1, 3 cores / 6 threads, minutes
    /// 242-290, ms/tick): 6:6 12.80, 6:3 13.19, 4:2 13.19, 6:2 13.63, 4:1 15.32, 5:1 15.39,
    /// chained 14.01. The plant leg is the long one on a grown world and its light and
    /// drink reads need their workers; fewer water workers buy it nothing. Giving it a
    /// physical core of its own (`cubarium::voxel::chiplet::split_overlap_pools`) was 4 %
    /// faster at hour 6 and 40 % slower while the young world is water-bound; renicing
    /// the water pool changed nothing. `CUBARIUM_OVERLAP_SPLIT=W,B` and
    /// [`set_overlap_split`] override it for measurement.
    pub fn overlap_split(&self) -> (usize, usize) {
        let n = self.threads.max(1);
        if let Some((w, b)) = split_override() {
            return (w.clamp(1, n), b.clamp(1, n));
        }
        (n, n)
    }
}

/// Build the process's compute pool — the plant and animal leg's in-phase workers — of
/// `threads` workers now, if it does not exist yet. [`Sim::new`] does this itself; a host
/// that wants that pool's threads on CPUs of their own sets its thread's affinity first
/// and calls this (a spawned thread inherits its spawner's affinity), then builds the
/// water pool (`cubarium_voxel::water::prepare_pool`) under another.
pub fn prepare_compute_pool(threads: usize) {
    if threads > 1 {
        ComputeTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(threads).build());
    }
}

/// Set by [`set_overlap_split`]: `water << 16 | bio`, `0` unset.
static SPLIT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// **Measurement only**: pin [`SimConfig::overlap_split`] for the whole process, `None`
/// to clear, so one run can time several splits on one world. A water count the process
/// has no pool for yet builds one on first use.
pub fn set_overlap_split(split: Option<(usize, usize)>) {
    let packed = split.map_or(0, |(w, b)| (w.clamp(1, 0xffff) << 16) | b.clamp(1, 0xffff));
    SPLIT.store(packed, std::sync::atomic::Ordering::Relaxed);
}

/// [`set_overlap_split`]'s, else `CUBARIUM_OVERLAP_SPLIT=W,B` from the environment.
fn split_override() -> Option<(usize, usize)> {
    match SPLIT.load(std::sync::atomic::Ordering::Relaxed) {
        0 => {}
        p => return Some((p >> 16, p & 0xffff)),
    }
    static ENV: std::sync::OnceLock<Option<(usize, usize)>> = std::sync::OnceLock::new();
    *ENV.get_or_init(|| {
        let s = std::env::var("CUBARIUM_OVERLAP_SPLIT").ok()?;
        let (w, b) = s.split_once(',')?;
        Some((w.trim().parse().ok()?, b.trim().parse().ok()?))
    })
}

/// The phases of one tick, as system sets, so a host can order its own systems against
/// them (`.after(TickPhase::Fauna)` for a presenter, `.in_set(TickPhase::Sample)` for a
/// harness observer) without naming a private system.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TickPhase {
    /// Rebuild any active set a direct write invalidated. Always first.
    Begin,
    /// Prescribed rain and evaporation, then the substep loop, then drainage, the water
    /// table, the spring and the outlet: [`cubarium_voxel::water`]'s own order.
    Water,
    /// [`Flora::step`].
    Flora,
    /// [`Fauna::step`].
    Fauna,
    /// The overlapped tick's barrier, after both legs: the plants' planned drink applied
    /// to the live world. Empty on the chained tick.
    Settle,
    /// The world's tick counter moves here, after every phase that reads it.
    Advance,
    /// **Empty in this crate.** Where a caller's observers go: they run after the tick and
    /// before the next one, which is the only point at which the three layers agree.
    Sample,
}

/// The label of the tick schedule.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tick;

/// The label of the **overlapped** live tick ([`SimConfig::overlap`]): `Begin` (and the
/// read copy), then `Water` beside `Flora → Fauna`, then `Settle`, `Advance` and `Sample`.
/// [`Sim::step`] runs it instead of [`Tick`] while overlap is on.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OverlapTick;

/// The label of the inner free-water substep schedule, run `water_substeps` times by the
/// [`TickPhase::Water`] leg.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Substep;

/// The label of the **static arena** schedule: the [`Tick`] schedule with the `Begin`,
/// `Water` and `Flora` legs left out. Terrain, water and plant geometry were prepared once
/// by the arena builder and do not evolve; the fauna leg, the clock and a caller's samplers
/// still run (`design/voxel-senses-phase1-plan.md`, "Frozen arena contract").
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StaticTick;

/// The label of the **frozen-water** schedule: the static schedule with the live `Flora`
/// leg put back — plants grow, shed litter, decompose it and feed the cue's sources — and
/// still no `Begin` or `Water` leg (P5-C S1: shredder landscape episodes carry the
/// litter production the live world has, on water that does not move).
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrozenWaterTick;

/// Which schedule [`Sim::step`] runs. The live schedule is the default and is unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ScheduleMode {
    /// World, flora, fauna, advance: the full coupled tick.
    #[default]
    Live,
    /// Fauna and advance only: a frozen arena.
    Static,
    /// Flora, fauna and advance: live plants on frozen water ([`FrozenWaterTick`]).
    FrozenWater,
}

/// One coupled voxel simulation: the three layers, the schedule that steps them, and the
/// thread pool the parallel phases use.
pub struct Sim {
    ecs: World,
    mode: ScheduleMode,
}

impl Sim {
    /// Take ownership of three built layers and wire the tick schedule around them.
    ///
    /// The compute pool is **process-global** (`bevy_tasks::ComputeTaskPool`), so the
    /// first `Sim` in a process fixes its size; a second one with a different
    /// `SimConfig::threads` gets the pool that already exists. That is why the bench runs
    /// one process per thread count. The water phases' rayon pool is process-global too
    /// (`cubarium_voxel::water::prepare_pool`), one per thread count, so sims side by side
    /// asking for the same count share one; `threads = 1` builds neither and runs every
    /// phase on the calling thread.
    ///
    /// `senses` is the **live schedule's** optional sensory state: a [`Senses`] already
    /// settled by the caller against the layers it is passing in (settling is not done
    /// here — a caller that has one settled for this exact source layout should not pay
    /// twice, and the static entry points below settle for their own reasons). With one,
    /// the fauna leg steps through [`Fauna::step_with_senses`], so the litter field
    /// updates at its own [`cubarium_voxel_fauna::UPDATE_TICKS`] cadence from the *live*
    /// flora — which grows, drops litter, decomposes it and has it eaten. With `None`,
    /// the fauna leg is exactly what it was: senses-free, `Chem` reading zero with
    /// validity 0 for any founder body.
    pub fn new(
        world: cubarium_voxel::World,
        flora: Flora,
        fauna: Fauna,
        config: SimConfig,
        senses: Option<Senses>,
    ) -> Sim {
        let threads = config.threads.max(1);
        if threads > 1 {
            ComputeTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(threads).build());
            // The water phases' rayon pool of the same size, built now rather than on the
            // first exchange so its workers exist before a host pins its loop (a spawned
            // thread inherits its spawner's affinity). On the chained tick the two pools
            // never run at once; the overlapped tick runs them side by side, its water on
            // a pool of its own split's size (`SimConfig::overlap_split`).
            water::prepare_pool(threads);
            water::prepare_pool(config.overlap_split().0);
        }

        let mut ecs = World::new();
        ecs.insert_resource(VoxelWorld(world));
        ecs.insert_resource(FloraLayer(flora));
        ecs.insert_resource(FaunaLayer(fauna));
        ecs.insert_resource(config);
        if let Some(senses) = senses {
            ecs.insert_resource(SenseField(senses));
        }

        let mut substep = Schedule::new(Substep);
        substep.set_executor(SingleThreadedExecutor::new());
        substep.add_systems((sys_infiltrate, sys_fall, sys_exchange).chain());
        ecs.add_schedule(substep);

        let mut tick = Schedule::new(Tick);
        tick.set_executor(SingleThreadedExecutor::new());
        tick.configure_sets(
            (
                TickPhase::Begin,
                TickPhase::Water,
                TickPhase::Flora,
                TickPhase::Fauna,
                TickPhase::Advance,
                TickPhase::Sample,
            )
                .chain(),
        );
        tick.add_systems(sys_begin.in_set(TickPhase::Begin));
        tick.add_systems(
            (
                sys_rain,
                sys_evaporate,
                sys_substeps,
                sys_drain,
                sys_water_table,
                sys_spring,
                sys_outlet,
            )
                .chain()
                .in_set(TickPhase::Water),
        );
        tick.add_systems(sys_flora.in_set(TickPhase::Flora));
        tick.add_systems(sys_fauna.in_set(TickPhase::Fauna));
        tick.add_systems(sys_advance.in_set(TickPhase::Advance));
        ecs.add_schedule(tick);

        // The overlapped tick: the water leg on the thread that runs the schedule (its
        // exchange scratch is per thread and world-sized), the plant and animal leg on a
        // compute-pool worker beside it, reading the read copy; the barrier after both.
        let mut overlap = Schedule::new(OverlapTick);
        if threads > 1 {
            overlap.set_executor(MultiThreadedExecutor::new());
        } else {
            overlap.set_executor(SingleThreadedExecutor::new());
        }
        overlap.configure_sets(
            (
                TickPhase::Begin,
                TickPhase::Water,
                TickPhase::Settle,
                TickPhase::Advance,
                TickPhase::Sample,
            )
                .chain(),
        );
        overlap.configure_sets(
            (
                TickPhase::Begin,
                TickPhase::Flora,
                TickPhase::Fauna,
                TickPhase::Settle,
            )
                .chain(),
        );
        overlap.add_systems((sys_begin, sys_read_copy).chain().in_set(TickPhase::Begin));
        overlap.add_systems(sys_water_leg.in_set(TickPhase::Water));
        overlap.add_systems(sys_flora_lagged.in_set(TickPhase::Flora));
        overlap.add_systems(sys_fauna_lagged.in_set(TickPhase::Fauna));
        overlap.add_systems(
            sys_withdraw
                .after(TickPhase::Water)
                .after(TickPhase::Flora)
                .before(TickPhase::Settle),
        );
        overlap.add_systems(sys_settle.in_set(TickPhase::Settle));
        overlap.add_systems(sys_advance.in_set(TickPhase::Advance));
        ecs.add_schedule(overlap);

        // The static arena: the same `sys_fauna` and `sys_advance`, with no `Begin`, no
        // `Water` and no `Flora`. Nothing in the skipped legs is a rule of the fauna tick,
        // so an animal cannot tell which schedule stepped it except that the world around
        // it never moved.
        let mut stat = Schedule::new(StaticTick);
        stat.set_executor(SingleThreadedExecutor::new());
        stat.configure_sets((TickPhase::Fauna, TickPhase::Advance, TickPhase::Sample).chain());
        stat.add_systems(sys_fauna_static.in_set(TickPhase::Fauna));
        stat.add_systems(sys_advance.in_set(TickPhase::Advance));
        ecs.add_schedule(stat);

        // Frozen water: the static schedule plus the live plant leg, in the live order.
        let mut frozen = Schedule::new(FrozenWaterTick);
        frozen.set_executor(SingleThreadedExecutor::new());
        frozen.configure_sets(
            (
                TickPhase::Flora,
                TickPhase::Fauna,
                TickPhase::Advance,
                TickPhase::Sample,
            )
                .chain(),
        );
        frozen.add_systems(sys_flora.in_set(TickPhase::Flora));
        frozen.add_systems(sys_fauna_static.in_set(TickPhase::Fauna));
        frozen.add_systems(sys_advance.in_set(TickPhase::Advance));
        ecs.add_schedule(frozen);

        Sim {
            ecs,
            mode: ScheduleMode::Live,
        }
    }

    /// Build the layers and wire them into a [`Sim`] in [`ScheduleMode::Static`]: the static
    /// arena's entry point. Everything else is [`Sim::new`].
    ///
    /// The static arena's own [`Senses`] is built and **settled here**, before any tick can
    /// sample it: at most 120 field updates, stopping early on convergence, in the same
    /// update order the tick itself uses. The cost is part of this construction — the setup
    /// the benchmark charges — and the field then evolves inside the tick at its own
    /// cadence. A caller that has already settled a field for this exact source layout
    /// passes it to [`Sim::new_static_prepared`] instead of paying again.
    pub fn new_static(
        world: cubarium_voxel::World,
        flora: Flora,
        fauna: Fauna,
        config: SimConfig,
    ) -> Sim {
        let mut sim = Sim::new(world, flora, fauna, config, None);
        sim.mode = ScheduleMode::Static;
        // Settle against the layers as they stand, then hold the field as a resource.
        let (w, f) = (sim.world(), sim.flora());
        let mut senses = Senses::new();
        senses.settle(&w.view(), &f.view());
        // Terrain, water and growth are frozen here: the cone occupancy is held across
        // controller stages and patched from bites and deaths (cone-speed item 4), and
        // the light receptor is memoed per standing face.
        senses.hold_cone();
        senses.hold_light();
        sim.ecs.insert_resource(SenseField(senses));
        sim
    }

    /// [`Sim::new_static`] with a caller-prepared [`Senses`]: a field settled earlier for
    /// this exact source layout — the same frozen terrain and the same resource stocks, or
    /// the prepared state is wrong — reused instead of re-settled. The per-body trend
    /// stores are cleared here whatever the caller passed: they are episode-private, and a
    /// stale trend history would read a false gradient on a new episode's first sample.
    pub fn new_static_prepared(
        world: cubarium_voxel::World,
        flora: Flora,
        fauna: Fauna,
        config: SimConfig,
        mut senses: Senses,
    ) -> Sim {
        let mut sim = Sim::new(world, flora, fauna, config, None);
        sim.mode = ScheduleMode::Static;
        senses.reset_trends();
        senses.hold_cone();
        senses.hold_light();
        sim.ecs.insert_resource(SenseField(senses));
        sim
    }

    /// Build the three layers from their configs and wire them up: the ordinary entry
    /// point.
    pub fn from_configs(
        voxel: cubarium_voxel::Config,
        flora: cubarium_voxel_flora::FloraConfig,
        fauna: cubarium_voxel_fauna::FaunaConfig,
        config: SimConfig,
    ) -> Sim {
        Sim::new(
            cubarium_voxel::World::new(voxel),
            Flora::new(flora),
            Fauna::new(fauna),
            config,
            None,
        )
    }

    /// One coupled tick: the whole [`Tick`] schedule.
    ///
    /// Equivalent to the three-call sequence `world.step(); flora.step(&mut world);
    /// fauna.step(&world, &mut flora);` — which is still what
    /// [`cubarium_voxel::World::step`] and the layers' own `step` do, and still what the
    /// core fixtures drive — with the water leg unrolled into its phases.
    ///
    /// **What did not parallelise, and what blocks it.** Stated here because a reader of
    /// the bench table needs it:
    ///
    /// - `rain`, `evaporate`, `infiltrate`, `fall`, `drain`: every one of them writes free
    ///   or pore water through `add_free`/`take_free`/`add_pore`/`take_pore`, and those
    ///   maintain [`cubarium_voxel`]'s two `CellSet`s — one shared `Vec` plus a dense slot
    ///   array, with swap-removal. Two threads inserting into it cannot both be right.
    ///   Splitting the sets per band is a data-structure change, not a scheduling one.
    /// - `water_table`: its module doc makes the share order a **rule** — "within a step
    ///   the fill runs bottom-up in index order, which is also the order a scarce stock is
    ///   shared in". Parallelising it would change who gets the last of a nearly empty
    ///   aquifer, which is a rule change and not mine.
    /// - `spring`, `outlet`: one cell each.
    /// - `flora`: its two read-then-apply phases *do* split cleanly — `light_per_stand`
    ///   and `drink`'s root-box read are per stand and touch nothing else — but the whole
    ///   plant layer is 40 µs of a 5,000 µs tick (0.6 %, six species and 48 stands), which
    ///   is at the scale of the task-spawn overhead itself. `light_per_stand` also writes
    ///   the sky cache as it reads it (`sky_at` takes `&mut Vec`), so it would need the
    ///   cache filled in a pass of its own first. Left serial, and measured that way.
    pub fn step(&mut self) {
        match self.mode {
            ScheduleMode::Live if self.config().overlap => {
                // The read copy is made on the first overlapped tick, so a sim that never
                // runs one — every static arena the trainer builds — never pays for it.
                if !self.ecs.contains_resource::<LaggedWorld>() {
                    let copy = self.world().clone();
                    self.ecs.insert_resource(LaggedWorld(copy));
                    self.ecs.init_resource::<PendingDrink>();
                    self.ecs.init_resource::<Lend>();
                }
                self.ecs.run_schedule(OverlapTick)
            }
            ScheduleMode::Live => self.ecs.run_schedule(Tick),
            ScheduleMode::Static => self.ecs.run_schedule(StaticTick),
            ScheduleMode::FrozenWater => self.ecs.run_schedule(FrozenWaterTick),
        }
    }

    /// Force one **static arena** tick whatever the mode: no `Begin`, `Water` or `Flora`
    /// leg, just the fauna leg and the clock. This is the static schedule's own system
    /// ([`sys_fauna_static`], the live [`sys_fauna`] with the arena's [`SenseField`]
    /// threaded through), so a static episode uses the production fauna path with the
    /// field its construction settled.
    pub fn step_static(&mut self) {
        self.ecs.run_schedule(StaticTick);
    }

    /// Which schedule [`Sim::step`] runs.
    pub fn mode(&self) -> ScheduleMode {
        self.mode
    }

    /// Select the live or static schedule.
    pub fn set_mode(&mut self, mode: ScheduleMode) {
        self.mode = mode;
        // What the senses may hold depends on what the schedule freezes. The held cone
        // occupancy pays only while plants are frozen: with the plant leg running every
        // stand's crown moves between stages and a held grid would be rebuilt whole every
        // stage, dearer than the per-stage window. The light memo reads terrain alone, so
        // frozen water keeps it; the live schedule holds neither, as it never did.
        if let Some(mut field) = self.ecs.get_resource_mut::<SenseField>() {
            let senses = &mut field.0;
            match mode {
                ScheduleMode::Static => {
                    senses.hold_cone();
                    senses.hold_light();
                }
                ScheduleMode::FrozenWater => {
                    senses.release_cone();
                    senses.hold_light();
                }
                ScheduleMode::Live => {
                    senses.release_cone();
                    senses.release_light();
                }
            }
        }
    }

    /// Turn [`SimConfig::overlap`] on or off between ticks. Nothing is pending at a tick
    /// boundary, so either tick can follow either.
    pub fn set_overlap(&mut self, on: bool) {
        self.ecs.resource_mut::<SimConfig>().overlap = on;
    }

    /// Add systems to [`TickPhase::Sample`]: a host's observers, run once per tick after
    /// every layer has stepped. The live schedules only — the chained and the overlapped
    /// tick both get them, hence `Clone`; [`Sim::add_static_samplers`] is the static
    /// schedule's.
    pub fn add_samplers<M>(
        &mut self,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M> + Clone,
    ) {
        let again = systems.clone();
        self.ecs.schedule_scope(Tick, |_, schedule| {
            schedule.add_systems(systems.in_set(TickPhase::Sample));
        });
        self.ecs.schedule_scope(OverlapTick, |_, schedule| {
            schedule.add_systems(again.in_set(TickPhase::Sample));
        });
    }

    /// Add systems to **both live schedules**, placed by the caller (`.in_set(...)`): a
    /// harness's hook inside a phase. `make` is called once per schedule.
    pub fn add_live_systems<M, S: IntoScheduleConfigs<ScheduleSystem, M>>(
        &mut self,
        make: impl Fn() -> S,
    ) {
        self.ecs.schedule_scope(Tick, |_, schedule| {
            schedule.add_systems(make());
        });
        self.ecs.schedule_scope(OverlapTick, |_, schedule| {
            schedule.add_systems(make());
        });
    }

    /// Add systems to [`TickPhase::Sample`] of the **static** schedule. A caller that runs
    /// a static arena and wants per-tick observers uses this; the same system shape as
    /// [`Sim::add_samplers`], on the other schedule.
    pub fn add_static_samplers<M>(&mut self, systems: impl IntoScheduleConfigs<ScheduleSystem, M>) {
        self.ecs.schedule_scope(StaticTick, |_, schedule| {
            schedule.add_systems(systems.in_set(TickPhase::Sample));
        });
    }

    pub fn config(&self) -> SimConfig {
        *self.ecs.resource::<SimConfig>()
    }

    pub fn world(&self) -> &cubarium_voxel::World {
        &self.ecs.resource::<VoxelWorld>().0
    }

    pub fn world_mut(&mut self) -> &mut cubarium_voxel::World {
        &mut self.ecs.resource_mut::<VoxelWorld>().into_inner().0
    }

    pub fn flora(&self) -> &Flora {
        &self.ecs.resource::<FloraLayer>().0
    }

    pub fn flora_mut(&mut self) -> &mut Flora {
        &mut self.ecs.resource_mut::<FloraLayer>().into_inner().0
    }

    pub fn fauna(&self) -> &Fauna {
        &self.ecs.resource::<FaunaLayer>().0
    }

    pub fn fauna_mut(&mut self) -> &mut Fauna {
        &mut self.ecs.resource_mut::<FaunaLayer>().into_inner().0
    }

    /// The world and both layers at once, for a caller that has to read all three
    /// together — a presenter, a harness observer, a conservation check.
    pub fn layers(&self) -> (&cubarium_voxel::World, &Flora, &Fauna) {
        (self.world(), self.flora(), self.fauna())
    }

    /// The world and both layers mutably at once, for the command plumbing that seeds a
    /// stand or introduces an animal against the world it stands in. Three nested
    /// `resource_scope`s, which is how bevy hands out several `&mut` resources at once.
    pub fn with_layers_mut<R>(
        &mut self,
        f: impl FnOnce(&mut cubarium_voxel::World, &mut Flora, &mut Fauna) -> R,
    ) -> R {
        self.ecs.resource_scope(|ecs, mut w: Mut<'_, VoxelWorld>| {
            ecs.resource_scope(|ecs, mut fl: Mut<'_, FloraLayer>| {
                ecs.resource_scope(|_ecs, mut fa: Mut<'_, FaunaLayer>| {
                    f(&mut w.0, &mut fl.0, &mut fa.0)
                })
            })
        })
    }

    /// The bevy world, for a host that wants to add its own resources and samplers.
    pub fn ecs(&mut self) -> &mut World {
        &mut self.ecs
    }

    /// The static arena's settled litter field, if this sim holds one: read-only, for a
    /// benchmark that wants the settle cost separated or a driver that wants to cache a
    /// prepared copy for an unchanged source layout (clone it and [`Senses::reset_trends`]
    /// the copy, or hand it to [`Sim::new_static_prepared`]). `None` when this sim holds
    /// no field: every static arena has one, and a live schedule has one only when its
    /// caller gave [`Sim::new`] a settled [`Senses`].
    pub fn senses(&self) -> Option<&Senses> {
        self.ecs.get_resource::<SenseField>().map(|s| &s.0)
    }
}

// ------------------------------------------------------------------ the systems
//
// One per phase, each a thin call into the layer that owns the rule. The phase order is
// the chain above; nothing here decides anything.

fn sys_begin(mut w: ResMut<VoxelWorld>) {
    water::begin(&mut w.0);
}

fn sys_rain(mut w: ResMut<VoxelWorld>) {
    water::rain(&mut w.0);
}

fn sys_evaporate(mut w: ResMut<VoxelWorld>) {
    water::evaporate(&mut w.0);
}

/// The free-water substep loop: the [`Substep`] schedule, run `water_substeps` times.
///
/// An exclusive system, because running a schedule needs the whole world. The substep
/// count and `sub_dt` come off the world's own config, exactly as
/// [`cubarium_voxel::water::step`] reads them.
fn sys_substeps(ecs: &mut World) {
    let substeps = ecs
        .resource::<VoxelWorld>()
        .0
        .config()
        .water_substeps
        .max(1);
    for _ in 0..substeps {
        ecs.run_schedule(Substep);
    }
}

fn sys_infiltrate(mut w: ResMut<VoxelWorld>, config: Res<SimConfig>) {
    let substeps = w.0.config().water_substeps.max(1);
    let sub_dt = cubarium_voxel::DT / substeps as f64;
    water::infiltrate(&mut w.0, sub_dt, config.threads);
}

fn sys_fall(mut w: ResMut<VoxelWorld>, config: Res<SimConfig>) {
    water::fall(&mut w.0, config.threads);
}

fn sys_exchange(mut w: ResMut<VoxelWorld>, config: Res<SimConfig>) {
    water::exchange(&mut w.0, config.threads);
}

fn sys_drain(mut w: ResMut<VoxelWorld>, config: Res<SimConfig>) {
    water::drain(&mut w.0, config.threads);
}

fn sys_water_table(mut w: ResMut<VoxelWorld>, config: Res<SimConfig>) {
    water::water_table(&mut w.0, config.threads);
}

fn sys_spring(mut w: ResMut<VoxelWorld>) {
    water::spring(&mut w.0);
}

fn sys_outlet(mut w: ResMut<VoxelWorld>) {
    water::outlet(&mut w.0);
}

fn sys_flora(mut w: ResMut<VoxelWorld>, mut flora: ResMut<FloraLayer>, config: Res<SimConfig>) {
    flora.0.step_with(&mut w.0, config.threads);
}

/// The **live** fauna leg. With a [`SenseField`] in the world — the seeded habitat's
/// settled litter field, handed to [`Sim::new`] — it steps through
/// [`Fauna::step_with_senses`], so the field updates at its own cadence from the living
/// flora before any controller samples it. Without one it is
/// [`Fauna::step_with`], byte for byte the senses-free path this system always was.
fn sys_fauna(
    w: Res<VoxelWorld>,
    mut flora: ResMut<FloraLayer>,
    mut fauna: ResMut<FaunaLayer>,
    config: Res<SimConfig>,
    mut senses: Option<ResMut<SenseField>>,
) {
    match senses.as_deref_mut() {
        Some(s) => fauna
            .0
            .step_with_senses(&w.0, &mut flora.0, config.threads, &mut s.0),
        None => fauna.0.step_with(&w.0, &mut flora.0, config.threads),
    }
}

/// The **static arena's** fauna leg: [`Fauna::step_with_senses`] with the arena's own
/// [`SenseField`], so the controller stage samples the settled litter field its builder
/// prepared. The live schedule's [`sys_fauna`] is unchanged and senses-free — the field
/// is per-arena state, and this one system is the documented mechanism that passes it
/// into the tick (`design/voxel-senses-phase1-plan.md`, "Frozen arena contract"). A
/// static schedule without a field runs senses-free, exactly like the live path.
fn sys_fauna_static(
    w: Res<VoxelWorld>,
    mut flora: ResMut<FloraLayer>,
    mut fauna: ResMut<FaunaLayer>,
    config: Res<SimConfig>,
    mut senses: Option<ResMut<SenseField>>,
) {
    match senses.as_deref_mut() {
        Some(s) => fauna
            .0
            .step_with_senses(&w.0, &mut flora.0, config.threads, &mut s.0),
        None => fauna.0.step_with(&w.0, &mut flora.0, config.threads),
    }
}

fn sys_advance(mut w: ResMut<VoxelWorld>) {
    w.0.advance_tick();
}

// ------------------------------------------------------- the overlapped tick's systems

/// The read copy made to read as the live world does: its water swapped in, the rest
/// copied. The live world's water is stale until the water leg restores it.
fn sys_read_copy(
    mut w: ResMut<VoxelWorld>,
    mut lagged: ResMut<LaggedWorld>,
    mut lend: ResMut<Lend>,
) {
    lagged.0.take_readable_from(&mut w.0, &mut lend.0);
}

/// The whole water leg, in the chained tick's phase order, as one system on the thread
/// running the schedule ([`NonSendMarker`]): the exchange keeps a world-sized scratch per
/// calling thread, so the leg must not wander across pool workers.
fn sys_water_leg(
    _main: NonSendMarker,
    mut w: ResMut<VoxelWorld>,
    lagged: Res<LaggedWorld>,
    mut lend: ResMut<Lend>,
    config: Res<SimConfig>,
) {
    let threads = config.overlap_split().0;
    let w = &mut w.0;
    cubarium_voxel::voxel_phase!(WaterLeg, {
        // The water the read copy was lent at the tick's start, back before anything
        // reads it; the plants and animals are reading the copy meanwhile.
        cubarium_voxel::voxel_phase!(ReadCopy, {
            w.restore_water_from(&lagged.0, &mut lend.0, threads)
        });
        water::rain(w);
        water::evaporate(w);
        let substeps = w.config().water_substeps.max(1);
        let sub_dt = cubarium_voxel::DT / substeps as f64;
        for _ in 0..substeps {
            water::infiltrate(w, sub_dt, threads);
            water::fall(w, threads);
            water::exchange(w, threads);
        }
        water::drain(w, threads);
        water::water_table(w, threads);
        water::spring(w);
        water::outlet(w);
    });
}

/// The plant leg against the read copy: the drink planned, not applied, and handed out
/// for the live world to take.
fn sys_flora_lagged(
    lagged: Res<LaggedWorld>,
    mut flora: ResMut<FloraLayer>,
    mut pending: ResMut<PendingDrink>,
    config: Res<SimConfig>,
) {
    flora.0.step_planned(&lagged.0, config.overlap_split().1);
    pending.0 = flora.0.take_drink_plan();
}

/// [`sys_fauna`] against the read copy. The animal layer only ever reads the world.
fn sys_fauna_lagged(
    lagged: Res<LaggedWorld>,
    mut flora: ResMut<FloraLayer>,
    mut fauna: ResMut<FaunaLayer>,
    config: Res<SimConfig>,
    mut senses: Option<ResMut<SenseField>>,
) {
    let threads = config.overlap_split().1;
    match senses.as_deref_mut() {
        Some(s) => fauna
            .0
            .step_with_senses(&lagged.0, &mut flora.0, threads, &mut s.0),
        None => fauna.0.step_with(&lagged.0, &mut flora.0, threads),
    }
}

/// The planned drink withdrawn from the live world once this tick's water is done, each
/// voxel bounded by what it holds now. Runs beside the animal step, which reads only the
/// read copy; on the schedule's thread, like the water leg.
fn sys_withdraw(_main: NonSendMarker, mut w: ResMut<VoxelWorld>, mut pending: ResMut<PendingDrink>) {
    if let Some(plan) = pending.0.as_mut() {
        cubarium_voxel::voxel_phase!(Settle, { plan.withdraw(&mut w.0) });
    }
}

/// The barrier: the withdrawn drink booked in the plant layer, each shortfall off whoever
/// asked.
fn sys_settle(mut flora: ResMut<FloraLayer>, mut pending: ResMut<PendingDrink>) {
    if let Some(plan) = pending.0.take() {
        flora.0.finish_drink(plan);
    }
}
