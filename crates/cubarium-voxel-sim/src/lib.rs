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
//! # Order is the rule; parallelism is inside a phase
//!
//! The tick is a dependency chain — rain feeds infiltration, infiltration feeds the
//! exchange, the exchange feeds what the plants drink, the plants feed the animals — so
//! **no two phases ever run at the same time**. The schedule is `.chain()`ed and its
//! executor is the single-threaded one on purpose. Speed comes from splitting the work
//! *inside* a phase:
//!
//! | phase | parallel? | why |
//! | --- | --- | --- |
//! | `rain`, `evaporate` | no | both write free water through the shared active sets |
//! | `infiltrate`, `fall` | no | same: `CellSet::insert`/`remove` is one shared structure |
//! | `exchange` | **yes**, its scratch pass | read-old/write-new, and columns are disjoint |
//! | `drain`, `water_table`, `spring`, `outlet` | no | they share one scalar aquifer stock, and `water_table` shares a scarce stock in index order **by rule** |
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
use bevy_ecs::schedule::{ScheduleLabel, SingleThreadedExecutor};
use bevy_ecs::system::ScheduleSystem;
use bevy_tasks::{ComputeTaskPool, TaskPoolBuilder};

use cubarium_voxel::water;
use cubarium_voxel_fauna::Fauna;
use cubarium_voxel_flora::Flora;

/// The voxel grid, free water, pore water and the aquifer: a **dense resource**, never one
/// entity per voxel.
#[derive(Resource)]
pub struct VoxelWorld(pub cubarium_voxel::World);

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
    /// touches the pool at all; the default is [`std::thread::available_parallelism`].
    ///
    /// **Placeholder** (`design/backlog.md` §1): nothing measured a best value, and the
    /// bench addendum in `design/7_Research/voxel-tick-profile-2026-09-18.md` is the only
    /// evidence there is about which count pays.
    pub threads: usize,
}

impl Default for SimConfig {
    fn default() -> SimConfig {
        SimConfig {
            threads: std::thread::available_parallelism().map_or(1, std::num::NonZero::get),
        }
    }
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
    /// The world's tick counter moves here, after every phase that reads it.
    Advance,
    /// **Empty in this crate.** Where a caller's observers go: they run after the tick and
    /// before the next one, which is the only point at which the three layers agree.
    Sample,
}

/// The label of the tick schedule.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tick;

/// The label of the inner free-water substep schedule, run `water_substeps` times by the
/// [`TickPhase::Water`] leg.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Substep;

/// One coupled voxel simulation: the three layers, the schedule that steps them, and the
/// thread pool the parallel phases use.
pub struct Sim {
    ecs: World,
}

impl Sim {
    /// Take ownership of three built layers and wire the tick schedule around them.
    ///
    /// The compute pool is **process-global** (`bevy_tasks::ComputeTaskPool`), so the
    /// first `Sim` in a process fixes its size; a second one with a different
    /// `SimConfig::threads` gets the pool that already exists. That is why the bench runs
    /// one process per thread count.
    pub fn new(world: cubarium_voxel::World, flora: Flora, fauna: Fauna, config: SimConfig) -> Sim {
        let threads = config.threads.max(1);
        if threads > 1 {
            ComputeTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(threads).build());
        }

        let mut ecs = World::new();
        ecs.insert_resource(VoxelWorld(world));
        ecs.insert_resource(FloraLayer(flora));
        ecs.insert_resource(FaunaLayer(fauna));
        ecs.insert_resource(config);

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

        Sim { ecs }
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
        self.ecs.run_schedule(Tick);
    }

    /// Add systems to [`TickPhase::Sample`]: a host's observers, run once per tick after
    /// every layer has stepped.
    pub fn add_samplers<M>(&mut self, systems: impl IntoScheduleConfigs<ScheduleSystem, M>) {
        self.ecs.schedule_scope(Tick, |_, schedule| {
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

fn sys_infiltrate(mut w: ResMut<VoxelWorld>) {
    let substeps = w.0.config().water_substeps.max(1);
    let sub_dt = cubarium_voxel::DT / substeps as f64;
    water::infiltrate(&mut w.0, sub_dt);
}

fn sys_fall(mut w: ResMut<VoxelWorld>) {
    water::fall(&mut w.0);
}

fn sys_exchange(mut w: ResMut<VoxelWorld>, config: Res<SimConfig>) {
    water::exchange(&mut w.0, config.threads);
}

fn sys_drain(mut w: ResMut<VoxelWorld>) {
    water::drain(&mut w.0);
}

fn sys_water_table(mut w: ResMut<VoxelWorld>) {
    water::water_table(&mut w.0);
}

fn sys_spring(mut w: ResMut<VoxelWorld>) {
    water::spring(&mut w.0);
}

fn sys_outlet(mut w: ResMut<VoxelWorld>) {
    water::outlet(&mut w.0);
}

fn sys_flora(mut w: ResMut<VoxelWorld>, mut flora: ResMut<FloraLayer>) {
    flora.0.step(&mut w.0);
}

fn sys_fauna(
    w: Res<VoxelWorld>,
    mut flora: ResMut<FloraLayer>,
    mut fauna: ResMut<FaunaLayer>,
    config: Res<SimConfig>,
) {
    fauna.0.step_with(&w.0, &mut flora.0, config.threads);
}

fn sys_advance(mut w: ResMut<VoxelWorld>) {
    w.0.advance_tick();
}
