//! The opt-in paid hunter extension: a fixed experimental apex lineage.
//!
//! `design/7_Research/astra-fixed-hunter-implementation-plan-2026-09-13.md` is the working
//! recipe; `fixed-hunter-core-handoff-2026-09-13.md` is the work order. **Every number in
//! [`FixedHunterProfile::lanternjaw_trial`] is a trial parameter, not validated balance and
//! not canon.** Nothing here is on by default: a world whose [`HunterState::profile`] is
//! `None` and whose member list is empty executes the pre-hunter tick, operation for
//! operation, with no extra draws (`crate::world::World::step` branches out immediately).
//!
//! What membership means: the organisms named in [`HunterState::members`] are the predators.
//! `genome.form` only picks a rig, so a saved unrelated form-4 organism can never become one,
//! and [`FixedHunterProfile::role`] carries the art direction as an explicit semantic role
//! the renderer maps to a body.
//!
//! The energy model is the world's, unchanged: adult structure carries **no** chemical
//! energy, reserve carries `e_r · R`, the usable battery is `E`, and an escrow carries
//! `e_r · (S_c + R_c) + E_c` until birth turns its structural part into heat. Calling prey
//! "meat" does not create a new energy density: a carried carcass keeps exactly the energy
//! that was removed from the prey, in [`HunterMember::gut_energy`], and that gut is part of
//! the world's stored totals, its invariants and both energy audits.

mod events;
mod geometry;
mod metabolism;
mod profile;
mod state;
mod strike;
#[cfg(test)]
mod tests;

pub use events::*;
pub use geometry::*;
pub use metabolism::*;
pub use profile::*;
pub use state::*;
pub use strike::*;
