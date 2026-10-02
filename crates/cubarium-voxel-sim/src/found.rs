//! **The founding loop** — lake gate, pre-roll, seeding and the acceptance check,
//! redrawn on a refusal — shared by the host, the live trainer's worlds and the frozen
//! trainer's landscapes: [`found`] on a [`LayerConfig`], the config in force.
//!
//! A founding can also keep the kept try's world half way through its opening shower
//! ([`Founded::mid_shower`]): the second frozen water state a training landscape runs in
//! (decision D6).

use cubarium_voxel::{Landform, World};
use cubarium_voxel_fauna::{Fauna, FaunaConfig, Founder};
use cubarium_voxel_flora::{Flora, FloraConfig};

use crate::habitat;
use crate::params::Params;

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

/// **The episode-fixture config**: the plant and animal configs a world's layers are
/// built from. One shape for every world that gets layers — the host's, a live training
/// world, a frozen arena and a frozen landscape — so the config in force is always the
/// one passed, never a default picked up on the way.
#[derive(Clone, Debug, PartialEq)]
pub struct LayerConfig {
    /// The plant layer's config, already at the world's cell size.
    pub flora: FloraConfig,
    pub fauna: FaunaConfig,
}

impl LayerConfig {
    /// `params`' plants and animals, the plants rescaled to `voxel_m`
    /// ([`FloraConfig::rescaled_to`]).
    pub fn new(params: &Params, voxel_m: f64) -> LayerConfig {
        LayerConfig {
            flora: params.flora.clone().rescaled_to(voxel_m),
            fauna: params.fauna,
        }
    }

    /// Fresh, empty plant and animal layers on these configs.
    pub fn layers(&self) -> (Flora, Fauna) {
        (Flora::new(self.flora.clone()), Fauna::new(self.fauna))
    }
}

/// How a founding draws its terrain seeds.
#[derive(Clone, Copy, Debug)]
pub enum Seeds {
    /// This seed and no other, kept whatever its lake and its habitat verdict, with a
    /// warning: someone naming a seed wants that world, not a nearby one.
    Asked(u64),
    /// `base`, `base + stride`, …: the same founding every time.
    Counting { base: u64, stride: u64 },
    /// Whatever the function draws: the ambient run's fresh world.
    Drawn(fn() -> u64),
}

impl Seeds {
    /// `base`, `base + 1`, …
    pub fn from(base: u64) -> Seeds {
        Seeds::Counting { base, stride: 1 }
    }
}

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
    /// try, when the founding was asked to keep one and the world rained; otherwise
    /// `None`.
    pub mid_shower: Option<World>,
}

/// **The founding loop**: lake gate → pre-roll → seed → acceptance, redrawing the
/// terrain seed on a refusal, up to [`HABITAT_TRIES`]; the accepted world (or, when none
/// passes, the one with the best worse-lineage stock) is returned already seeded with
/// `founder_counts` on `layers`. `keep_mid_shower` also keeps each try's world half way
/// through its opening shower ([`Founded::mid_shower`]).
pub fn found(
    cfg: &cubarium_voxel::Config,
    layers: &LayerConfig,
    founder_counts: [usize; Founder::COUNT],
    seeds: Seeds,
    keep_mid_shower: bool,
) -> Founded {
    found_judged(
        cfg,
        layers,
        founder_counts,
        seeds,
        keep_mid_shower,
        |s: &habitat::Seeded| s.acceptance.accepted,
    )
}

/// [`found`] with the verdict `accept` in place of the acceptance check.
fn found_judged(
    cfg: &cubarium_voxel::Config,
    layers: &LayerConfig,
    founder_counts: [usize; Founder::COUNT],
    seeds: Seeds,
    keep_mid_shower: bool,
    accept: impl Fn(&habitat::Seeded) -> bool,
) -> Founded {
    let (asked, mut next_seed): (Option<u64>, Box<dyn FnMut() -> u64>) = match seeds {
        Seeds::Asked(seed) => (Some(seed), Box::new(|| unreachable!("an asked seed draws none"))),
        Seeds::Counting { base, stride } => {
            let mut next = base;
            (
                None,
                Box::new(move || {
                    let seed = next;
                    next = next.wrapping_add(stride);
                    seed
                }),
            )
        }
        Seeds::Drawn(draw) => (None, Box::new(draw)),
    };
    let mut lake_rejected = 0usize;
    let found = |world: World| {
        let mut world = world;
        let (mut flora, mut fauna) = layers.layers();
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
        let (world, seed, _) = generate_with_a_lake(cfg, Some(seed), LAKE_SEED_TRIES, &mut next_seed);
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
    let tries = HABITAT_TRIES;
    let mut best: Option<Founded> = None;
    for k in 0..tries {
        let (world, seed, rejected) = generate_with_a_lake(cfg, None, LAKE_SEED_TRIES, &mut next_seed);
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
        let mut cfg = cubarium_voxel::Config {
            width: 32,
            height: 24,
            depth: 4,
            voxel_m: 0.25,
            landform: Landform::Staged(recipe),
            ..cubarium_voxel::Config::default()
        };
        // A shower of a second, so two pre-rolls stay short.
        cfg.climate.opening_min_s = 1.0;
        let layers = LayerConfig::new(&Params::defaults(), cfg.voxel_m);
        let judged = std::cell::Cell::new(0usize);
        let first_fails = |_: &habitat::Seeded| {
            judged.set(judged.get() + 1);
            judged.get() > 1
        };
        let founded = found_judged(&cfg, &layers, [2, 2, 0, 0, 0, 0, 0, 0], Seeds::from(5), false, first_fails);
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
        let founded = found_judged(
            &cfg,
            &layers,
            [2, 2, 0, 0, 0, 0, 0, 0],
            Seeds::Asked(6),
            false,
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
