//! **Live-world training**: every animal carries its own brain variant inside the
//! running ecology (`design/handoffs/voxel-live-world-training-2026-09-24.md`).
//!
//! The frozen-world trainer ([`super::trainer`]) scores a candidate on a world that
//! neither regrows nor crowds. Here the worlds are the live ones — plants grow, regrow,
//! seed and die, litter falls, animals breed and die, water moves — and they are **not
//! reset** between generations: a generation is a window of simulated time over which
//! every living body of a lineage runs one antithetic variant of that lineage's centre,
//! and the ecology carries on while the brains change.
//!
//! - [`world`]: the training world — the `small` staged preset with its flowing water
//!   off, founded by the live founding loop.
//! - [`variants`]: the per-generation variant table and the controller that holds a
//!   handle into it, so a body's weights are not a copy per body.
//! - [`window`]: one world's window — twin placement, newborns, the per-body score
//!   accumulators and the movement readout.
//! - [`readout`]: the ecology readout, every generation, pooled over worlds, as a list of
//!   named factors (adding one is one line).
//! - [`ecology`]: the ecology parameter vector each world holds — the seam for the
//!   ecology half of the search.
//! - [`trainer`]: the loop — many persistent worlds, both lineages at once, one ES update
//!   per lineage per window, population bounds.
//!
//! Nothing here is hard-wired to two lineages: a lineage is a [`Founder`] with a manifest
//! and a centre, and every table is indexed by [`Founder::index`] over [`Founder::ALL`].

use cubarium_voxel_fauna::{Controller, Fauna, Founder};

use super::EpisodeDriver;

pub mod curiosity;
pub mod ecology;
pub mod readout;
pub mod screen;
pub mod search;
pub mod targets;
pub mod trainer;
pub mod variants;
pub mod window;
pub mod world;

#[cfg(test)]
mod breeding_tests;
#[cfg(test)]
mod curiosity_tests;
#[cfg(test)]
mod gut_screen_tests;
#[cfg(test)]
mod knob_wiring_tests;
#[cfg(test)]
mod lineage_params_tests;
#[cfg(test)]
mod lockstep_tests;
#[cfg(test)]
mod mass_tests;
#[cfg(test)]
mod motor_tests;
#[cfg(test)]
mod movement_tests;
#[cfg(test)]
mod reward_tests;
#[cfg(test)]
mod roster_tests;
#[cfg(test)]
mod screen_tests;
#[cfg(test)]
mod search_tests;
#[cfg(test)]
mod targets_tests;
#[cfg(test)]
mod tests;

/// Install one centre per lineage on a whole layer, as the ambient host does: the birth
/// factory and every standing body of that lineage.
pub fn install_centres(fauna: &mut Fauna, drivers: &[(Founder, EpisodeDriver)]) {
    // The reflexes a centre falls back on run on the layer's own numbers.
    let teachers = fauna.config().teachers;
    for (founder, driver) in drivers {
        let driver = driver.clone();
        fauna.set_founder_factory(
            *founder,
            std::sync::Arc::new(move || -> Box<dyn Controller> { driver.fresh(teachers) }),
        );
        fauna.set_policy_driven(*founder, true);
    }
    let bodies: Vec<(u64, Founder)> = fauna
        .view()
        .animals
        .iter()
        .filter_map(|a| a.founder.map(|f| (a.id, f)))
        .collect();
    for (id, founder) in bodies {
        if drivers.iter().any(|(f, _)| *f == founder) {
            fauna.install_founder_controller(id, founder);
        }
    }
}
