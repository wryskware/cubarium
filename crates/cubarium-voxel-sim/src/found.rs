//! **The founding loop** — lake gate, pre-roll, seeding and the acceptance check,
//! redrawn on a refusal — moved here out of the `cubarium` binary crate (P5-B,
//! `design/handoffs/voxel-retrain-2026-09-22.md` item 1) so the search crate can found
//! the same worlds the ambient run founds. The host calls it unchanged: `cubarium::voxel`
//! re-exports every name here.
//!
//! The one addition is [`found_a_landscape`] / [`ambient_landscape`], which run the same
//! loop and also keep the kept try's world half way through its opening shower
//! ([`Founded::mid_shower`]): the second frozen water state a training landscape runs in
//! (decision D6).

use cubarium_voxel::{Landform, World};
use cubarium_voxel_fauna::{Fauna, FaunaConfig, Founder};
use cubarium_voxel_flora::{Flora, FloraConfig};

use crate::habitat;

/// Ticks a candidate world is settled for before its water is judged.
///
/// Two simulated seconds, and that is enough: measured on `small`, the sheet `hydrate`
/// lays finds its own hollows inside twenty ticks and the reading does not move after
/// forty — seed 5 goes 3.7 m² to 1.5 at tick 20 and is still 1.4 at tick 400. Two hundred
/// ticks cost a full second per candidate on that preset, eight times what generating it
/// cost; forty costs a fifth of that and says the same thing.
const GATE_SETTLE_TICKS: u32 = 40;

/// The step a body takes between neighbouring support faces, metres — the bound the
/// generator's own walkability check uses.
const WALK_STEP_M: f64 = 0.5;

/// Random seeds tried before the generator gives up and keeps the wettest world it saw.
pub const LAKE_SEED_TRIES: usize = 24;

/// Habitat tries: how many lake-gated worlds the founding loop pre-rolls, seeds and
/// judges before it keeps the best of them. Bounded apart from [`LAKE_SEED_TRIES`]:
/// each try draws its own world through the lake gate.
pub const HABITAT_TRIES: usize = 8;

/// A founded world: generated, lake-gated, pre-rolled, **already seeded** and judged.
pub struct Founded {
    pub world: World,
    pub flora: Flora,
    pub fauna: Fauna,
    pub seeded: habitat::Seeded,
    pub seed: u64,
    /// Seeds the lake gate rejected, over every habitat try.
    pub lake_rejected: usize,
    /// Worlds that were seeded and failed the acceptance check.
    pub habitat_rejected: usize,
    /// Whether the returned world passed the acceptance check.
    pub accepted: bool,
    /// What the loop said loudly about the returned world, if anything.
    pub warnings: Vec<String>,
    /// The world as it stood **half way through the opening shower** of the returned
    /// try, when the founding was asked to keep one ([`found_a_landscape`]) and the
    /// world rained. `None` from [`found_a_habitat`], which the host runs: it keeps
    /// nothing it does not use.
    pub mid_shower: Option<World>,
}

/// The founding loop: lake gate → pre-roll → seed → acceptance, redrawing the terrain
/// seed on a rejection, up to `habitat_tries`; the accepted world is returned already
/// seeded. `accept` is the verdict (the real one is `|s| s.acceptance.accepted`).
#[allow(clippy::too_many_arguments)]
pub fn found_a_habitat(
    cfg: &cubarium_voxel::Config,
    asked: Option<u64>,
    lake_tries: usize,
    habitat_tries: usize,
    next_seed: impl FnMut() -> u64,
    layers: impl Fn(&World) -> (Flora, Fauna),
    founder_counts: [usize; Founder::COUNT],
    accept: impl Fn(&habitat::Seeded) -> bool,
) -> Founded {
    found(
        cfg,
        asked,
        lake_tries,
        habitat_tries,
        next_seed,
        layers,
        founder_counts,
        accept,
        false,
    )
}

/// [`found_a_habitat`], keeping a copy of each try's world half way through its opening
/// shower ([`Founded::mid_shower`]): the second frozen water state a training landscape
/// runs in (`design/handoffs/voxel-retrain-2026-09-22.md`, D6). Everything else — the
/// draws, the pre-roll, the seeding, the verdict — is the host's loop, step for step.
#[allow(clippy::too_many_arguments)]
pub fn found_a_landscape(
    cfg: &cubarium_voxel::Config,
    asked: Option<u64>,
    lake_tries: usize,
    habitat_tries: usize,
    next_seed: impl FnMut() -> u64,
    layers: impl Fn(&World) -> (Flora, Fauna),
    founder_counts: [usize; Founder::COUNT],
    accept: impl Fn(&habitat::Seeded) -> bool,
) -> Founded {
    found(
        cfg,
        asked,
        lake_tries,
        habitat_tries,
        next_seed,
        layers,
        founder_counts,
        accept,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn found(
    cfg: &cubarium_voxel::Config,
    asked: Option<u64>,
    lake_tries: usize,
    habitat_tries: usize,
    next_seed: impl FnMut() -> u64,
    layers: impl Fn(&World) -> (Flora, Fauna),
    founder_counts: [usize; Founder::COUNT],
    accept: impl Fn(&habitat::Seeded) -> bool,
    keep_mid_shower: bool,
) -> Founded {
    let mut next_seed = next_seed;
    let mut lake_rejected = 0usize;
    let found = |world: World| {
        let mut world = world;
        let (mut flora, mut fauna) = layers(&world);
        let mut mid_shower = None;
        let seeded = habitat::seed_with_founder_counts_capturing(
            &mut world,
            &mut flora,
            &mut fauna,
            founder_counts,
            keep_mid_shower.then_some(&mut mid_shower),
        );
        (world, flora, fauna, seeded, mid_shower)
    };
    if let Some(seed) = asked {
        let (world, seed, _) = generate_with_a_lake(cfg, Some(seed), lake_tries, &mut next_seed);
        let (world, flora, fauna, seeded, mid_shower) = found(world);
        let accepted = accept(&seeded);
        let mut warnings = Vec::new();
        if !accepted {
            let w = format!(
                "cubarium voxel: seed {seed} was asked for, so it is kept — but its habitat \
                 fails the acceptance check ({})",
                seeded.acceptance.reasons()
            );
            eprintln!("{w}");
            warnings.push(w);
        }
        return Founded {
            world,
            flora,
            fauna,
            seeded,
            seed,
            lake_rejected: 0,
            habitat_rejected: 0,
            accepted,
            warnings,
            mid_shower,
        };
    }
    let tries = habitat_tries.max(1);
    let mut best: Option<Founded> = None;
    for k in 0..tries {
        let (world, seed, rejected) = generate_with_a_lake(cfg, None, lake_tries, &mut next_seed);
        lake_rejected += rejected;
        let (world, flora, fauna, seeded, mid_shower) = found(world);
        let founded = Founded {
            world,
            flora,
            fauna,
            seeded,
            seed,
            lake_rejected,
            habitat_rejected: k,
            accepted: false,
            warnings: Vec::new(),
            mid_shower,
        };
        if accept(&founded.seeded) {
            if k > 0 {
                eprintln!("cubarium voxel: seed {seed} accepted as a habitat after {k} refused");
            }
            return Founded {
                accepted: true,
                ..founded
            };
        }
        eprintln!(
            "cubarium voxel: seed {seed} refused as a habitat: {} (worse lineage at {:.2} \
             founder-hours per founder)",
            founded.seeded.acceptance.reasons(),
            founded.seeded.acceptance.worse_ratio(),
        );
        let ratio = founded.seeded.acceptance.worse_ratio();
        if best
            .as_ref()
            .is_none_or(|b| ratio > b.seeded.acceptance.worse_ratio())
        {
            best = Some(founded);
        }
    }
    let mut best = best.expect("at least one try");
    let w = format!(
        "cubarium voxel: NO SEED of {tries} passed the habitat acceptance check; keeping \
         seed {} with the best worse-lineage stock of {:.2} founder-hours per founder — \
         {}",
        best.seed,
        best.seeded.acceptance.worse_ratio(),
        best.seeded.acceptance.reasons(),
    );
    eprintln!("{w}");
    best.warnings.push(w);
    best.habitat_rejected = tries;
    best.lake_rejected = lake_rejected;
    best
}

/// Found the world the ambient run founds for `cfg`, deterministically: the lake gate
/// and the habitat loop drawing `base`, `base + 1`, … rather than random seeds, the
/// plant layer scaled to the cell size as the host scales it, `FaunaConfig::default()`
/// and the real acceptance check. For a diagnostic arm that has to be the shipped
/// landscape, **already seeded** — it must not be seeded again.
pub fn ambient_habitat(
    cfg: &cubarium_voxel::Config,
    base: u64,
    flora_config: impl Fn(f64) -> FloraConfig,
    founder_counts: [usize; Founder::COUNT],
) -> Founded {
    ambient(cfg, base, flora_config, founder_counts, false)
}

/// [`ambient_habitat`] keeping the kept try's mid-shower world ([`found_a_landscape`]):
/// the world a training landscape is founded as — the live founding, from seed base
/// `base`, exactly.
pub fn ambient_landscape(
    cfg: &cubarium_voxel::Config,
    base: u64,
    flora_config: impl Fn(f64) -> FloraConfig,
    founder_counts: [usize; Founder::COUNT],
) -> Founded {
    ambient(cfg, base, flora_config, founder_counts, true)
}

fn ambient(
    cfg: &cubarium_voxel::Config,
    base: u64,
    flora_config: impl Fn(f64) -> FloraConfig,
    founder_counts: [usize; Founder::COUNT],
    keep_mid_shower: bool,
) -> Founded {
    let mut next = base;
    found(
        cfg,
        None,
        LAKE_SEED_TRIES,
        HABITAT_TRIES,
        move || {
            let seed = next;
            next = next.wrapping_add(1);
            seed
        },
        |w: &World| {
            (
                Flora::new(flora_config(w.config().voxel_m)),
                Fauna::new(FaunaConfig::default()),
            )
        },
        founder_counts,
        |s: &habitat::Seeded| s.acceptance.accepted,
        keep_mid_shower,
    )
}

/// Draw generated worlds until one has a lake the camera can actually read.
///
/// **A ring with no visible water is not a habitat** (Wrysk, 2026-09-21: "reject any
/// generated terrain seeds that dont have a pond or something water related"). The
/// recipe says how much open water it wants in square metres
/// ([`cubarium_voxel::Water::min_lake_m2`]); a seed under it is logged and redrawn, up to
/// `tries`, and then the wettest of them is kept and said so loudly — a world is always
/// returned, because refusing to start is worse than starting dry.
///
/// An **asked-for** seed is honoured whatever its lake, with a warning. Someone naming a
/// seed wants that world, not a nearby one.
pub fn generate_with_a_lake(
    cfg: &cubarium_voxel::Config,
    asked: Option<u64>,
    tries: usize,
    mut next_seed: impl FnMut() -> u64,
) -> (World, u64, usize) {
    let (want, want_tiers) = match &cfg.landform {
        Landform::Staged(r) => (r.water.min_lake_m2, r.water.min_tier_pools as usize),
        Landform::Terrarium(t) => (t.water.min_lake_m2, t.water.min_tier_pools as usize),
        Landform::Ridge => (0.0, 0),
    };
    let build = |seed: u64| {
        World::new(cubarium_voxel::Config {
            seed,
            ..cfg.clone()
        })
    };
    // What the camera would make of this world's water — **after a short settle**, which
    // is the only reading that means anything. Out of `hydrate` the lake is a flat sheet
    // laid to the outlet's datum; over a stepped bed it drains into its own hollows
    // within a few seconds and can lose a third of its area doing it (`small` seed 5:
    // 3.7 m² to 1.6). Judging the sheet accepts worlds the panel then shows dry.
    //
    // It is paid on a **clone**: the world that is returned is the freshly generated one,
    // which the host settles again for real when it seeds the habitat.
    let read = |world: &World| {
        let mut probe = world.clone();
        probe.settle(GATE_SETTLE_TICKS);
        let lake = cubarium_voxel::hydrate::lake(&probe);
        let tiers = cubarium_voxel::hydrate::tier_pools(
            &cubarium_voxel::hydrate::pools(&probe),
            lake.level_y,
        );
        (lake.visible_m2, tiers)
    };
    // Walking is descriptive: some landscapes have disconnected or inaccessible regions
    // by design. Keep the observation for development without redrawing their seeds.
    let walkable = |world: &World| cubarium_voxel::walk::around_the_ring(world, WALK_STEP_M);
    if let Some(seed) = asked {
        let world = build(seed);
        let (got, tiers) = read(&world);
        if (want > 0.0 && got < want) || tiers < want_tiers {
            eprintln!(
                "cubarium voxel: seed {seed} was asked for, so it is kept — but its lake is \
                 {got:.1} m² visible with {tiers} pool(s) above it, against the {want:.1} m² \
                 and {want_tiers} a drawn seed would need"
            );
        }
        if !walkable(&world) {
            eprintln!(
                "cubarium voxel: seed {seed} has no {WALK_STEP_M:.1} m closed walking route \
                 (observation only)"
            );
        }
        return (world, seed, 0);
    }
    let mut best: Option<(f64, usize, u64, World)> = None;
    for k in 0..tries.max(1) {
        let seed = next_seed();
        let world = build(seed);
        let (got, tiers) = read(&world);
        if got >= want && tiers >= want_tiers {
            if !walkable(&world) {
                eprintln!(
                    "cubarium voxel: seed {seed} has no {WALK_STEP_M:.1} m closed walking \
                     route (observation only)"
                );
            }
            if k > 0 {
                eprintln!(
                    "cubarium voxel: seed {seed} accepted: lake {got:.1} m² visible, \
                     {tiers} pool(s) above it"
                );
            } else {
                eprintln!("cubarium voxel: procedural world generated with random seed {seed}");
            }
            return (world, seed, k);
        }
        eprintln!(
            "cubarium voxel: seed {seed} rejected: lake {got:.1} m² visible with {tiers} \
             pool(s) above it, need {want:.1} m² and {want_tiers}"
        );
        // Better means more tiers first and then more water: a ring with a cascade and a
        // small lake is the picture Wrysk asked for; a big lake alone is the one it had.
        if best
            .as_ref()
            .is_none_or(|(bw, bt, _, _)| (tiers, got) > (*bt, *bw))
        {
            best = Some((got, tiers, seed, world));
        }
    }
    let (got, tiers, seed, world) = best.expect("at least one try");
    eprintln!(
        "cubarium voxel: NO SEED of {tries} had a lake of {want:.1} m² with {want_tiers} \
         pool(s) over it; keeping the wettest, seed {seed} with {got:.1} m² and {tiers} — \
         this world will look dry"
    );
    (world, seed, tries)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A generated world has to have water somebody can see. The gate redraws until it
    /// does, keeps the wettest when no seed obliges, and never argues with a seed that
    /// was asked for by name.
    #[test]
    fn the_gate_rejects_and_redraws() {
        // A short staged ring, so four generations stay well inside a second.
        let staged = |min_lake_m2: f64| {
            let mut recipe = cubarium_voxel::Recipe::DEFAULT;
            recipe.water.min_lake_m2 = min_lake_m2;
            // This test is about the lake bar alone; the tier-pool bar is asked of the presets.
            recipe.water.min_tier_pools = 0;
            cubarium_voxel::Config {
                width: 32,
                height: 24,
                depth: 4,
                voxel_m: 0.25,
                landform: Landform::Staged(recipe),
                ..cubarium_voxel::Config::default()
            }
        };
        let seeds = |list: Vec<u64>| {
            let mut it = list.into_iter();
            move || {
                it.next()
                    .expect("the gate asked for more seeds than it was given")
            }
        };

        // Nothing is asked of the lake, so the first draw is the world.
        let (_, seed, rejected) = generate_with_a_lake(&staged(0.0), None, 4, seeds(vec![11, 12]));
        assert_eq!(
            (seed, rejected),
            (11, 0),
            "with no bar, the first seed passes"
        );

        // A bar no ring can clear: every seed is rejected and the wettest is kept.
        let cfg = staged(1e6);
        let (world, seed, rejected) = generate_with_a_lake(&cfg, None, 3, seeds(vec![21, 22, 23]));
        assert_eq!(rejected, 3, "every try was rejected");
        assert!(
            [21, 22, 23].contains(&seed),
            "it kept one of the tries: {seed}"
        );
        let kept = cubarium_voxel::hydrate::lake(&world).visible_m2;
        for other in [21u64, 22, 23] {
            let w = World::new(cubarium_voxel::Config {
                seed: other,
                ..cfg.clone()
            });
            assert!(
                cubarium_voxel::hydrate::lake(&w).visible_m2 <= kept + 1e-12,
                "seed {seed} was the wettest of the three, not seed {other}"
            );
        }

        // An asked-for seed is the world, bar or no bar.
        let (_, seed, rejected) = generate_with_a_lake(&cfg, Some(77), 3, seeds(vec![]));
        assert_eq!((seed, rejected), (77, 0), "a named seed is honoured");
    }

    /// **The founding loop** (item 5): lake gate, pre-roll, seed, acceptance. A world the
    /// verdict refuses is redrawn and the next one is returned **already seeded**, never
    /// seeded twice; a seed that was asked for is kept whatever its verdict, with a
    /// warning.
    #[test]
    fn the_founding_loop_redraws_a_refused_habitat_and_returns_it_seeded() {
        let mut recipe = cubarium_voxel::Recipe::DEFAULT;
        recipe.water.min_lake_m2 = 0.0;
        recipe.water.min_tier_pools = 0;
        // A shower of a few seconds, so two pre-rolls stay short.
        recipe.water.shower_volume_m3 = 0.005;
        let cfg = cubarium_voxel::Config {
            width: 32,
            height: 24,
            depth: 4,
            voxel_m: 0.25,
            landform: Landform::Staged(recipe),
            ..cubarium_voxel::Config::default()
        };
        let seeds = |list: Vec<u64>| {
            let mut it = list.into_iter();
            move || {
                it.next()
                    .expect("the loop asked for more seeds than it was given")
            }
        };
        let layers = |w: &World| {
            (
                Flora::new(FloraConfig::for_voxel_size(w.config().voxel_m)),
                Fauna::new(FaunaConfig::default()),
            )
        };
        let judged = std::cell::Cell::new(0usize);
        let first_fails = |_: &habitat::Seeded| {
            judged.set(judged.get() + 1);
            judged.get() > 1
        };
        let founded = found_a_habitat(
            &cfg,
            None,
            1,
            4,
            seeds(vec![5, 6]),
            layers,
            [2, 2],
            first_fails,
        );
        assert_eq!(judged.get(), 2, "two worlds were judged");
        assert_eq!(founded.seed, 6, "the refused first draw was replaced");
        assert!(founded.accepted && founded.habitat_rejected == 1);
        assert!(
            founded.seeded.stands > 0,
            "returned seeded: {:?}",
            founded.seeded
        );
        assert_eq!(
            founded.flora.view().stands.len(),
            founded.seeded.stands,
            "seeded once, by the loop, and not again"
        );
        assert_eq!(
            founded.fauna.view().animals.len(),
            founded.seeded.animals(),
            "the founders are the loop's"
        );
        assert!(
            founded.seeded.pre_roll.opening_shower,
            "the world opened with rain"
        );

        // An asked-for seed is honoured whatever the verdict, and says so. (Seed 6, whose
        // ring has stands on it, so "seeded" is something that can be checked.)
        let founded = found_a_habitat(
            &cfg,
            Some(6),
            1,
            4,
            seeds(vec![]),
            layers,
            [2, 2],
            |_: &habitat::Seeded| false,
        );
        assert_eq!(founded.seed, 6);
        assert!(!founded.accepted);
        assert!(founded.seeded.stands > 0, "still seeded");
        assert!(
            founded.warnings.iter().any(|w| w.contains("asked")),
            "{:?}",
            founded.warnings
        );
    }

    /// Walking remains available as a terrain observation, including for landscapes with
    /// deliberately disconnected regions. It is not a seed-acceptance condition.
    #[test]
    fn walking_reports_disconnected_terrain_without_making_it_invalid() {
        // A flat ring of rock with a wall across the whole strip: nothing climbs 0.75 m.
        let c = cubarium_voxel::Config {
            width: 16,
            height: 12,
            depth: 4,
            voxel_m: 0.25,
            ..cubarium_voxel::Config::default()
        };
        let build = |wall: bool| {
            let mut w = World::empty(c.clone());
            for z in 0..c.depth {
                for x in 0..c.width as i64 {
                    let top = if wall && x == 8 { 7 } else { 4 };
                    for y in 1..=top {
                        w.apply(cubarium_voxel::Command::SetMaterial {
                            x,
                            y,
                            z,
                            material: cubarium_voxel::Material::Rock,
                        });
                    }
                }
            }
            w
        };
        assert!(
            cubarium_voxel::walk::around_the_ring(&build(false), WALK_STEP_M),
            "flat ground is walkable"
        );
        assert!(
            !cubarium_voxel::walk::around_the_ring(&build(true), WALK_STEP_M),
            "a three-voxel wall across the strip is not a step"
        );
    }
}
