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
use cubarium_voxel_fauna::Founder;
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
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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
    found_world_from(cfg, found::Seeds::from(base), founder_counts, ecology)
}

/// [`found_world`] from any [`found::Seeds`]: `Seeds::Asked(seed)` keeps that one world
/// whatever its verdict (the grid's fixed worlds across cells).
pub fn found_world_from(
    cfg: &Config,
    seeds: found::Seeds,
    founder_counts: [usize; Founder::COUNT],
    ecology: &EcologyParams,
) -> (Sim, FoundInfo) {
    try_found_world_from(cfg, seeds, founder_counts, ecology).unwrap_or_else(|why| panic!("{why}"))
}

/// [`found_world`], with a terrain the founding refuses returned as `Err` (its reason)
/// rather than a panic: for tools that skip a refused seed with a note (BRAIN-NEXT).
pub fn try_found_world(
    cfg: &Config,
    base: u64,
    founder_counts: [usize; Founder::COUNT],
    ecology: &EcologyParams,
) -> Result<(Sim, FoundInfo), String> {
    try_found_world_from(cfg, found::Seeds::from(base), founder_counts, ecology)
}

/// [`found_world_from`], refusing a rejected terrain by `Err`.
pub fn try_found_world_from(
    cfg: &Config,
    seeds: found::Seeds,
    founder_counts: [usize; Founder::COUNT],
    ecology: &EcologyParams,
) -> Result<(Sim, FoundInfo), String> {
    let Founded {
        world,
        flora,
        fauna,
        seeded,
        seed,
        lake_rejected,
        habitat_rejected,
        accepted,
        terrain_rejection,
        ..
    } = found::found(
        cfg,
        &crate::es::voxel::training_world::layers_for(ecology, cfg.voxel_m),
        founder_counts,
        seeds,
        false,
    );
    // A rejected terrain carries no founders; training on it would be silent nonsense.
    if let Some(why) = terrain_rejection {
        return Err(format!("terrain seed {seed} rejected: {why}"));
    }
    let sim = Sim::ambient(
        world,
        flora,
        fauna,
        // The chained tick: at one thread the overlapped one only adds its read copy.
        SimConfig {
            threads: 1,
            overlap: false,
            // The spec sets the coarse water on the world itself.
            water_every: 1,
        },
    );
    let info = FoundInfo {
        seed,
        accepted,
        lake_rejected,
        habitat_rejected,
        founders: seeded.founders,
        stands: seeded.stands,
    };
    Ok((sim, info))
}

#[cfg(test)]
mod tests {
    /// BRAIN-NEXT step 4: a terrain the founding refuses comes back as `Err` with its reason
    /// (the clone tool skips it with a note) instead of a panic.
    #[test]
    fn a_refused_terrain_is_an_err_not_a_panic() {
        let eco = super::EcologyParams::defaults();
        // Seed 1's terrain is refused before its pre-roll (no closed walking route); asked
        // for, it is kept and returned with that reason.
        let got = super::try_found_world_from(
            &super::training_config(),
            super::found::Seeds::Asked(1),
            eco.fauna().founder_counts(),
            &eco,
        );
        let why = got.err().expect("the refused terrain is an Err");
        assert!(why.contains("rejected"), "{why}");
    }
}

/// **A training top-up** (BRAIN-FIX step 4, `--top-up LINEAGE=N[:FULLNESS]`): after the
/// founding, `founder` is brought up to `bodies` grown bodies on its heuristic, placed on the
/// faces other bodies stand on, for a lineage the training worlds found none of (the
/// lanternjaw founds as an egg bank, which the training worlds leave out). `fullness` is
/// their reserve and stomach as a fraction of capacity (`None`: a placed body's default).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TopUp {
    pub founder: Founder,
    pub bodies: usize,
    pub fullness: Option<f64>,
}

impl TopUp {
    /// `lineage=N` or `lineage=N:fullness`.
    pub fn parse(s: &str) -> Result<TopUp, String> {
        let (name, rest) = s.split_once('=').ok_or_else(|| format!("top-up {s:?}: want LINEAGE=N[:FULLNESS]"))?;
        let founder = crate::es::voxel::parse_founder(name.trim())?;
        let (n, full) = match rest.split_once(':') {
            Some((n, f)) => (n, Some(f.trim().parse::<f64>().map_err(|e| format!("top-up {s:?}: {e}"))?)),
            None => (rest, None),
        };
        let bodies = n.trim().parse().map_err(|e| format!("top-up {s:?}: {e}"))?;
        if full.is_some_and(|f| !(0.0..=1.0).contains(&f)) {
            return Err(format!("top-up {s:?}: fullness is a fraction"));
        }
        Ok(TopUp { founder, bodies, fullness: full })
    }
}

/// Bring `t.founder` up to `t.bodies` grown bodies under its heuristic (the faces other
/// bodies stand on, in id order); how many were placed.
pub fn top_up(sim: &mut Sim, t: TopUp) -> usize {
    use cubarium_voxel_fauna::{BodySpec, BodyStage, Placed, Spot};
    let have = sim.fauna().view().animals.iter().filter(|a| a.founder == Some(t.founder)).count();
    if have >= t.bodies {
        return 0;
    }
    let (w, flora) = (sim.world().clone(), sim.flora().clone());
    let sites: Vec<_> = sim.fauna().view().animals.iter().map(|a| a.site).collect();
    let teachers = sim.fauna().config().teachers;
    let mut placed = 0;
    for (k, site) in sites.iter().enumerate() {
        if have + placed >= t.bodies {
            break;
        }
        let spec = BodySpec { stage: BodyStage::Adult, heading_rad: k as f64 * 1.3, fullness: t.fullness, ..BodySpec::default() };
        if let Ok(Placed::Body { id, .. }) = sim.fauna_mut().place_body(&w, &flora, t.founder, Spot::Face(*site), spec) {
            sim.fauna_mut().set_controller(id, cubarium_voxel_fauna::heuristic(t.founder, teachers));
            placed += 1;
        }
    }
    placed
}
