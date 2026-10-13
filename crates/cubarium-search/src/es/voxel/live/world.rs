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

/// How many base seeds a training or check world tries before its founding is a clean
/// `Err` (BRAIN-FIX-2 step 1). Each base already runs the founding's own redraws (up to
/// its habitat cap of terrains), so four bases are up to four times that many terrains.
/// A run-structure cap, not an ecological number.
pub const FOUNDING_BASE_TRIES: usize = 4;

/// The `k`-th base seed for a world asked for at `base`: `base` itself first, then
/// splitmix64 redraws hashed from `base` and `k`, so the same `base` always draws the same
/// sequence and two worlds' redraws don't land on each other.
pub fn redrawn_base(base: u64, k: usize) -> u64 {
    if k == 0 {
        return base;
    }
    let mut z = base ^ (k as u64).wrapping_mul(0xD1B5_4A32_D192_ED03).rotate_left(17);
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// **Found with redraws** (BRAIN-FIX-2 step 1): `try_found(seed)` on [`redrawn_base`]`(base,
/// k)` for `k = 0, 1, …` until one is accepted or [`FOUNDING_BASE_TRIES`] are refused, which
/// is a clean `Err` naming every refusal. Each refusal is passed to `note` (the run log) with
/// the next base. A first base that is accepted is exactly the plain founding.
pub fn found_redrawing<T>(
    base: u64,
    mut try_found: impl FnMut(u64) -> Result<T, String>,
    mut note: impl FnMut(&str),
) -> Result<T, String> {
    let mut refusals = Vec::new();
    for k in 0..FOUNDING_BASE_TRIES {
        let seed = redrawn_base(base, k);
        match try_found(seed) {
            Ok(t) => return Ok(t),
            Err(why) => {
                if k + 1 < FOUNDING_BASE_TRIES {
                    note(&format!("base seed {seed} (from {base}, try {k}) refused: {why}; redrawing from {}",
                        redrawn_base(base, k + 1)));
                }
                refusals.push(format!("{seed}: {why}"));
            }
        }
    }
    Err(format!("world base {base}: every one of {FOUNDING_BASE_TRIES} base seeds was refused ({})", refusals.join("; ")))
}

/// [`try_found_world`] through [`found_redrawing`]: a training world from `base` that
/// survives a refused terrain.
pub fn found_world_redrawing(
    cfg: &Config,
    base: u64,
    founder_counts: [usize; Founder::COUNT],
    ecology: &EcologyParams,
    note: impl FnMut(&str),
) -> Result<(Sim, FoundInfo), String> {
    found_redrawing(base, |seed| try_found_world(cfg, seed, founder_counts, ecology), note)
}

#[cfg(test)]
mod tests {
    /// BRAIN-FIX-2 step 1: an accepted first base is founded once, on `base` itself, with no
    /// note (so it founds exactly as before).
    #[test]
    fn an_accepted_first_base_founds_once_on_itself() {
        let (mut asked, mut notes) = (Vec::new(), Vec::new());
        let got = super::found_redrawing(77, |s| { asked.push(s); Ok::<u64, String>(s) }, |n| notes.push(n.to_string()));
        assert_eq!(got, Ok(77));
        assert_eq!(asked, vec![77]);
        assert!(notes.is_empty(), "{notes:?}");
    }

    /// A refused base redraws deterministically, with a note per refusal, and keeps the first
    /// accepted redraw.
    #[test]
    fn a_refused_base_redraws_deterministically_with_a_note() {
        let run = || {
            let (mut asked, mut notes) = (Vec::new(), Vec::new());
            let got = super::found_redrawing(21, |s| {
                asked.push(s);
                if asked.len() < 3 { Err(format!("no route {s}")) } else { Ok(s) }
            }, |n| notes.push(n.to_string()));
            (got, asked, notes)
        };
        let (got, asked, notes) = run();
        assert_eq!(asked.len(), 3);
        assert_eq!(asked[0], 21);
        assert_eq!(asked, (0..3).map(|k| super::redrawn_base(21, k)).collect::<Vec<_>>());
        assert_eq!(got, Ok(asked[2]));
        assert_eq!(notes.len(), 2);
        assert!(notes[0].contains("no route 21") && notes[0].contains(&asked[1].to_string()), "{notes:?}");
        assert_eq!(run().1, asked, "the same base draws the same sequence");
        // Two bases' redraws are distinct.
        assert_ne!(super::redrawn_base(21, 1), super::redrawn_base(22, 1));
        assert_ne!(super::redrawn_base(21, 1), super::redrawn_base(21, 2));
    }

    /// Past the cap the founding is a clean `Err` naming every refusal, not a panic.
    #[test]
    fn every_base_refused_is_a_clean_err() {
        let mut tries = 0;
        let got = super::found_redrawing::<()>(5, |s| { tries += 1; Err(format!("refused {s}")) }, |_| {});
        assert_eq!(tries, super::FOUNDING_BASE_TRIES);
        let why = got.expect_err("every base refused");
        assert_eq!(why.matches(": refused ").count(), super::FOUNDING_BASE_TRIES, "{why}");
    }

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

    /// A flat soil ring (support at y = 2), 24 m around and 8 m deep, with no bodies.
    fn flat_sim() -> super::Sim {
        use cubarium_voxel::{Command, Material};
        let (w, d, v) = (96u32, 32u32, 0.25);
        let mut world = cubarium_voxel::World::empty(cubarium_voxel::Config { width: w, height: 16, depth: d, voxel_m: v, ..cubarium_voxel::Config::default() });
        for z in 0..d {
            for x in 0..i64::from(w) {
                for y in 1..=2 {
                    world.apply(Command::SetMaterial { x, y, z, material: Material::Soil });
                }
            }
        }
        let flora = cubarium_voxel_flora::Flora::new(cubarium_voxel_flora::FloraConfig::for_voxel_size(v));
        let fauna = cubarium_voxel_fauna::Fauna::new(cubarium_voxel_fauna::FaunaConfig::default());
        super::Sim::new(world, flora, fauna, super::SimConfig::with_threads(1), None)
    }

    fn place(sim: &mut super::Sim, f: super::Founder, x_m: f64, z_m: f64) -> u64 {
        use cubarium_voxel_sim::edit::At;
        let at = At::Point { x_m, y_m: 0.75, z_m, aloft: false };
        match sim.edit(|e| e.place_animal(f, at, cubarium_voxel_fauna::BodySpec::default())).expect("placed") {
            cubarium_voxel_fauna::Placed::Body { id, .. } => id,
            other => panic!("not a body: {other:?}"),
        }
    }

    fn ring_gap(sim: &super::Sim, a: u64, b: u64) -> f64 {
        let c = sim.world().config();
        let ring = f64::from(c.width) * c.voxel_m;
        let v = sim.fauna().view();
        let (p, q) = (v.animal(a).expect("a").pose, v.animal(b).expect("b").pose);
        let dx = (q.x - p.x).rem_euclid(ring);
        dx.min(ring - dx).hypot(q.z - p.z)
    }

    /// LS-TEACH: a shown hunter is moved to about `at_m` from a grown prey body, a mother of a
    /// living young before one without; the sides alternate; with no hunter living, one is
    /// placed there.
    #[test]
    fn a_shown_hunter_stands_at_its_distance_from_a_mother_first() {
        use super::Founder::{Chorister as VH, Loftstrider as LS};
        let mut sim = flat_sim();
        let lone = place(&mut sim, LS, 4.0, 4.0);
        let mother = place(&mut sim, LS, 16.0, 4.0);
        let region = cubarium_voxel_sim::edit::Region { x: [56, 72], z: [0, 31] };
        let calved = sim.edit(|e| e.calve(LS, 1, 600.0, None, Some(region), None));
        assert_eq!(calved.calves.len(), 1, "{:?}", calved.refused);
        let hound = place(&mut sim, VH, 10.0, 7.0);
        let at_m = 3.0;
        let snap = 2.0 * sim.world().config().voxel_m;
        let mut sides = Vec::new();
        for k in 0..2 {
            let (h, prey) = super::show_hunter(&mut sim, LS, VH, k, at_m).expect("shown");
            assert_eq!((h, prey), (hound, mother), "the living hunter, to the mother");
            let gap = ring_gap(&sim, hound, mother);
            assert!((gap - at_m).abs() <= snap + 0.5, "k {k}: {gap} m from her, wanted {at_m}");
            sides.push(sim.fauna().view().animal(hound).unwrap().pose.x > sim.fauna().view().animal(mother).unwrap().pose.x);
        }
        assert_ne!(sides[0], sides[1], "the sides alternate");
        assert_ne!(lone, mother);
        // No hunter living: one is placed, as near.
        sim.edit(|e| e.remove_animal(hound)).expect("removed");
        let (placed, prey) = super::show_hunter(&mut sim, LS, VH, 0, at_m).expect("placed");
        assert_ne!(placed, hound);
        assert_eq!(prey, mother);
        assert_eq!(sim.fauna().view().animal(placed).and_then(|a| a.founder), Some(VH));
        assert!((ring_gap(&sim, placed, mother) - at_m).abs() <= snap + 0.5);
    }
}

/// **A training top-up** (BRAIN-FIX step 4, `--top-up LINEAGE=N[:FULLNESS]`): after the
/// founding, `founder` is brought up to `bodies` grown bodies on its heuristic, placed on the
/// faces other bodies stand on (then, when those run out, the columns' top faces on a
/// coarse grid), for a lineage the training worlds found none of (the
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
    // BRAIN-REST step 4: a large body (the loftstrider) fits few of the faces small bodies
    // stand on; when those run out, the columns' top faces on a coarse grid, in order.
    let (width, depth) = (w.config().width, w.config().depth);
    let stride = TOP_UP_STRIDE.max(1) as usize;
    let mut k = sites.len();
    'grid: for z in (0..depth).step_by(stride) {
        for x in (0..width).step_by(stride) {
            if have + placed >= t.bodies {
                break 'grid;
            }
            let Some(y) = w.view().surface_y(i64::from(x), z) else { continue };
            let site = cubarium_voxel_flora::Site { x, y, z };
            if sites.contains(&site) {
                continue;
            }
            let spec = BodySpec { stage: BodyStage::Adult, heading_rad: k as f64 * 1.3, fullness: t.fullness, ..BodySpec::default() };
            k += 1;
            if let Ok(Placed::Body { id, .. }) = sim.fauna_mut().place_body(&w, &flora, t.founder, Spot::Face(site), spec) {
                sim.fauna_mut().set_controller(id, cubarium_voxel_fauna::heuristic(t.founder, teachers));
                placed += 1;
            }
        }
    }
    placed
}

/// The grid step of [`top_up`]'s fallback faces, voxels.
const TOP_UP_STRIDE: u32 = 4;

/// **A hunter shown to a prey family** (LS-TEACH: the loftstrider teacher worlds, so that
/// threat labels come with the threat in the prey's senses): the `k`-th showing moves a
/// living `hunter` body (the `k`-th in id order, round robin; none living: a grown one
/// placed, on its heuristic) to a face about `at_m` along the ring from a grown `prey` body
/// (the `k`-th, round robin, among the mothers of a living young first, else among all
/// grown ones), the side alternating with `k`, at the prey's depth or, refused there, a few
/// others across the ring and a little nearer. The hunter keeps its state and pack. `Ok`
/// with `(hunter, prey)`; `Err` when there is no grown prey or no face takes it.
pub fn show_hunter(sim: &mut Sim, prey: Founder, hunter: Founder, k: u64, at_m: f64) -> Result<(u64, u64), String> {
    use cubarium_voxel_fauna::{BodySpec, Placed};
    use cubarium_voxel_sim::edit::At;
    let birth = sim.fauna().config().founder(prey).core.birth_body;
    let animals = sim.fauna().view().animals;
    let mothers: Vec<u64> = animals.iter().filter(|a| a.founder == Some(prey)).filter_map(|a| a.mother).collect();
    let grown: Vec<_> = animals.iter().filter(|a| a.founder == Some(prey) && a.attached.is_none() && a.body >= birth).collect();
    let with_young: Vec<_> = grown.iter().filter(|a| mothers.contains(&a.id)).collect();
    let pick = |n: usize| (k % n.max(1) as u64) as usize;
    let target = if with_young.is_empty() { grown.get(pick(grown.len())).copied() } else { with_young.get(pick(with_young.len())).map(|a| **a) };
    let target = target.ok_or_else(|| format!("no grown {}", prey.name()))?;
    let (tid, pose) = (target.id, target.pose);
    let hunters: Vec<u64> = animals.iter().filter(|a| a.founder == Some(hunter) && a.attached.is_none()).map(|a| a.id).collect();
    let moving = hunters.get(pick(hunters.len())).copied();
    let c = sim.world().config();
    let (ring, depth) = (f64::from(c.width) * c.voxel_m, f64::from(c.depth) * c.voxel_m);
    let side = if k % 2 == 0 { 1.0 } else { -1.0 };
    let zs = [pose.z, 0.25 * depth, 0.5 * depth, 0.75 * depth];
    let mut last = String::from("no face");
    for shorter in [0.0, 1.0, 2.0] {
        for &z in &zs {
            let d = (at_m - shorter).max(0.5);
            let at = At::Point { x_m: (pose.x + side * d).rem_euclid(ring), y_m: pose.y_m, z_m: z.clamp(0.0, depth - 1e-6), aloft: false };
            let r = match moving {
                Some(h) => sim.edit(|e| e.move_animal(h, at)).map(|()| h),
                None => sim.edit(|e| e.place_animal(hunter, at, BodySpec::default())).and_then(|p| match p {
                    Placed::Body { id, .. } => Ok(id),
                    Placed::Eggs { .. } => Err(format!("{} lays eggs", hunter.name())),
                }),
            };
            match r {
                Ok(h) => {
                    if moving.is_none() {
                        let teachers = sim.fauna().config().teachers;
                        sim.fauna_mut().set_controller(h, cubarium_voxel_fauna::heuristic(hunter, teachers));
                    }
                    return Ok((h, tid));
                }
                Err(e) => last = e,
            }
        }
    }
    Err(last)
}
