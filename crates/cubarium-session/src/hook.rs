//! **[`LoopHook`]**: something that follows the running world on the loop's own thread.
//!
//! The session draws nothing. A presentation that must read the world itself between
//! ticks — the Rust renderer hosted in the Godot app (D-0005), which packs the world into
//! its voxel texture — hands the loop a hook ([`crate::Session::set_hook`]). The loop calls
//! it with the layers borrowed, at the same points the feed is built, so a hook never races
//! a tick and never locks the world from another thread.
//!
//! A hook's time is the loop's time: whatever it does in [`LoopHook::period`] is added to
//! the period, as `cubarium voxel`'s pack is added to its loop.

use cubarium_voxel::World;
use cubarium_voxel_fauna::Fauna;
use cubarium_voxel_flora::Flora;

/// Where the run's clock stands when a hook is called.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HookClock {
    /// The world's tick.
    pub tick: u64,
    /// Ticks stepped since the last call (0 while paused or for a non-tick call).
    pub stepped: u64,
    /// The run's speed (1 is 20 ticks a second).
    pub speed: f64,
    pub paused: bool,
}

/// Called on the session's loop thread. Every call gets the layers as they stand.
pub trait LoopHook: Send {
    /// A world was installed (a promotion, a load), or the hook was set while one runs:
    /// whatever the hook held of an earlier world is stale.
    fn installed(&mut self, world: &World, flora: &Flora, fauna: &Fauna, clock: HookClock);
    /// The end of a loop period (twenty a second at any speed, paused or not). `moved`
    /// says the world or its layers changed since the last call.
    fn period(&mut self, world: &World, flora: &Flora, fauna: &Fauna, moved: bool, clock: HookClock);
    /// The world was closed; nothing runs until the next [`LoopHook::installed`].
    fn closed(&mut self) {}
}
