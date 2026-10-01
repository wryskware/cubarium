//! The **training world**: the `small` staged preset with its flowing water off, founded
//! by the live founding loop, stepped live.
//!
//! Still water, not frozen water: the lake, the soil water, the water table and the
//! showers all run, so rising water after rain is in every window. What is off is what
//! makes water *run* across the ring — the river re-entry stream at the spring, the
//! aquifer seep at the spring, the lake drain — and the founding no longer asks for
//! tier pools to hold water (`min_tier_pools = 0`), since nothing feeds them now but rain.
//!
//! Everything here is a config edit on a copy of the preset. The shipped presets are
//! untouched ([`still_water`] takes the config it is given by value).

use cubarium_voxel::{Config, Landform, Preset, World};
use cubarium_voxel_fauna::{Founder, Senses};
use cubarium_voxel_sim::found::{self, Founded};
use cubarium_voxel_sim::{Sim, SimConfig};

use super::ecology::EcologyParams;

/// The preset the training worlds are founded on.
pub const TRAINING_PRESET: &str = "small";

/// The flowing-water switches, applied to `cfg`: no river re-entry, no aquifer seep at the
/// spring, no lake drain, and no tier pools asked of the founding. The lake, the showers,
/// evaporation, the outlet (the lake's spill back aloft under the closed budget), the soil
/// water and the water table are left exactly as the preset has them.
pub fn still_water(mut cfg: Config) -> Config {
    if let Landform::Staged(r) = &mut cfg.landform {
        r.water.reentry_m3_per_s = 0.0;
        r.water.reentry_from_aquifer = false;
        r.water.min_tier_pools = 0;
        r.water.lake_drain_m2_per_s = 0.0;
    }
    // `World::new` writes the recipe's cycle onto the config; these are the same values
    // for a reader that looks at the config before a world is built.
    cfg.reentry_m3_per_s = 0.0;
    cfg.reentry_from_aquifer = false;
    cfg.lake_drain_m2_per_s = 0.0;
    cfg.spring_k_m2_per_s = 0.0;
    cfg
}

/// The training world's config: [`TRAINING_PRESET`] through [`still_water`].
pub fn training_config() -> Config {
    let preset = Preset::find(TRAINING_PRESET).expect("the small preset ships");
    still_water(preset.config())
}

/// Whether a built world is still: nothing can enter it but the showers.
pub fn is_still(world: &World) -> bool {
    let c = world.config();
    c.reentry_m3_per_s == 0.0 && c.spring_k_m2_per_s == 0.0 && c.lake_drain_m2_per_s == 0.0
}

/// What the founding loop kept for one training world.
#[derive(Clone, Debug)]
pub struct FoundInfo {
    pub seed: u64,
    pub accepted: bool,
    pub lake_rejected: usize,
    pub habitat_rejected: usize,
    pub founders: [usize; Founder::COUNT],
    pub stands: usize,
}

/// One founded training world, ready to step: the live founding loop (lake gate,
/// pre-roll, seeding, acceptance) from seed base `base`, the senses settled, the closed
/// budget's outlet opened as the host does, on one simulation thread.
///
/// `ecology` is applied to the layers' configs before seeding; the defaults change
/// nothing.
pub fn found_world(
    cfg: &Config,
    base: u64,
    founder_counts: [usize; Founder::COUNT],
    ecology: &EcologyParams,
) -> (Sim, FoundInfo) {
    let Founded {
        world,
        flora,
        fauna,
        seeded,
        seed,
        lake_rejected,
        habitat_rejected,
        accepted,
        ..
    } = found::found(
        cfg,
        &crate::es::voxel::training_world::layers_for(ecology, cfg.voxel_m),
        founder_counts,
        found::Seeds::from(base),
        false,
    );
    let mut senses = Senses::with_cue(fauna.config().cue);
    senses.settle(&world.view(), &flora.view());
    let mut sim = Sim::new(
        world,
        flora,
        fauna,
        // The chained tick: at one thread the overlapped one only adds its read copy.
        SimConfig {
            threads: 1,
            overlap: false,
        },
        Some(senses),
    );
    if sim.world().config().closed_water_budget && !sim.world().outlet_open() {
        sim.world_mut()
            .apply(cubarium_voxel::Command::SetOutlet { open: true });
    }
    let info = FoundInfo {
        seed,
        accepted,
        lake_rejected,
        habitat_rejected,
        founders: seeded.founders,
        stands: seeded.stands,
    };
    (sim, info)
}
