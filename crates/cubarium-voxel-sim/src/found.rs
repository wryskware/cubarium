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
    /// `base` first, then redraws hashed from `base` and the attempt: the same founding
    /// every time, and two bases never redraw into the same world (with `base + 1`
    /// redraws, seeds 1, 2 and 3 of `patch` all founded seed 4's world).
    Derived { base: u64 },
    /// Draw one fresh base seed, then deterministic redraws as in [`Seeds::Derived`].
    Drawn(fn() -> u64),
}

impl Seeds {
    /// `base`, then redraws hashed from it ([`Seeds::Derived`]).
    pub fn from(base: u64) -> Seeds {
        Seeds::Derived { base }
    }
}

/// The `k`-th redraw after `base` (`k ≥ 1`): splitmix64 over the pair, so redraws of
/// different bases don't land on each other.
fn derived_seed(base: u64, k: u64) -> u64 {
    let mut z = base ^ k.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(29);
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn derived_seeds(base: u64) -> impl FnMut() -> u64 {
    let mut k = 0u64;
    move || {
        let seed = if k == 0 { base } else { derived_seed(base, k) };
        k = k.wrapping_add(1);
        seed
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
        Seeds::Derived { base } => (None, Box::new(derived_seeds(base))),
        Seeds::Drawn(draw) => (None, Box::new(derived_seeds(draw()))),
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
    // What a scenario map needs beyond the lake (2026-10-04): a stream, halls, groves.
    // A terrarium's build report says what it made; the other generators make none of
    // them and ask for none.
    let water = cfg.landform.water().copied();
    let build = |seed: u64| {
        let (world, report) = World::new_reporting(cubarium_voxel::Config {
            seed,
            ..cfg.clone()
        });
        let short = water.and_then(|w| features_short(&w, report.as_ref()));
        (world, short)
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
        let (world, short) = build(seed);
        if let Some(why) = short {
            eprintln!("cubarium voxel: seed {seed} was asked for, so it is kept — but {why}");
        }
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
        let (world, short) = build(seed);
        if let Some(why) = short {
            // A world without what the scenario needs is no candidate at all: redrawn,
            // and never kept as the wettest.
            eprintln!("cubarium voxel: seed {seed} rejected: {why}");
            continue;
        }
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
                eprintln!("cubarium voxel: procedural world generated with seed {seed}");
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
    let Some((got, tiers, seed, world)) = best else {
        panic!(
            "cubarium voxel: none of {tries} seeds made what this map asks for (its \
             `[world.landform.water]` min_stream_m / min_halls / min_groves); relax them or \
             change the map"
        );
    };
    eprintln!(
        "cubarium voxel: NO SEED of {tries} had a lake of {want:.1} m² with {want_tiers} \
         pool(s) over it; keeping the wettest, seed {seed} with {got:.1} m² and {tiers} — \
         this world will look dry"
    );
    (world, seed, tries)
}

/// How many times founding a **picked** terrain places its founders before it gives up
/// ([`found_terrain`]): the first try on the terrain's own seed, each later one with the
/// weather and the layers' keyed draws re-keyed ([`World::reseed`]), never a new terrain.
/// It stops sooner when two tries running are refused for the same reasons.
pub const PLACEMENT_TRIES: usize = 3;

/// The `k`-th placement seed of a terrain seeded `seed` (`k ≥ 1`; try 0 is `seed` itself).
pub fn placement_seed(seed: u64, k: u64) -> u64 {
    if k == 0 { seed } else { derived_seed(seed, k) }
}

/// **The cheap feasibility check**: whether the founders could be placed on this terrain
/// at all, judged on the bare world as it stands (`probe`, already settled a little — the
/// lake gate's [`GATE_SETTLE_TICKS`]), **without the pre-roll**. The ordinary seeder and
/// the ordinary acceptance check ([`habitat::seed_pre_rolled`]) run on `probe` with a
/// pre-roll that did nothing: every support face, and the water standing on it now as the
/// deepest it saw. `probe` is seeded by it and is the caller's to throw away.
///
/// **What it covers**: every lineage's placement rule and count (scaled by the dry land),
/// each producer's establishment gates and light, the glowcap's logs and dark faces, the
/// vines and film, the packs' covered dens near prey, the egg bank, and the acceptance
/// check's food and route-map components — all on the terrain the founding will use.
///
/// **What it cannot predict**: the opening shower and the lake's rest. They raise the
/// water, so faces the shower floods are offered to nobody and the lake's banks move; the
/// dry land shrinks, which scales every count down; and the weather is the founding's
/// own. A terrain that passes here can still be refused after the pre-roll, which is what
/// [`found_terrain`]'s retries on the same terrain are for.
pub fn feasibility(probe: &mut World, layers: &LayerConfig, founder_counts: [usize; Founder::COUNT]) -> habitat::Seeded {
    let sites = habitat::support_sites(probe);
    let wettest = {
        let view = probe.view();
        sites.iter().map(|s| view.water_depth_m(i64::from(s.x), s.y, s.z)).collect()
    };
    let pre = habitat::PreRoll {
        settle: cubarium_voxel::Settle::default(),
        report: habitat::PreRollReport::default(),
        sites,
        wettest,
    };
    let (mut flora, mut fauna) = layers.layers();
    habitat::seed_pre_rolled(probe, &mut flora, &mut fauna, &pre, founder_counts)
}

/// A generated terrain, bare: what a new-world picker shows before anything is founded.
pub struct Terrain {
    /// The world as generated: no settle, no plants, no animals.
    pub world: World,
    pub seed: u64,
    /// Draws refused for the lake gate or the scenario's features.
    pub lake_rejected: usize,
    /// Draws refused by the feasibility check ([`feasibility`]).
    pub infeasible_rejected: usize,
    /// The lake after the gate's short settle, m² visible, and the pools above it.
    pub lake_m2: f64,
    pub tier_pools: usize,
    /// What the feasibility check said of this terrain; `None` when it was not asked
    /// (a bare world founds no one).
    pub feasibility: Option<habitat::Acceptance>,
    /// Said loudly about this terrain, if anything (an asked seed kept despite its lake or
    /// its feasibility, the wettest kept when none had the lake asked for).
    pub warnings: Vec<String>,
}

/// Why no terrain came back from [`draw_terrain`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoTerrain {
    /// The caller stopped it.
    Cancelled,
    /// Every draw was refused; the reason, in plain words.
    Refused(String),
}

/// **A terrain for a picker**: the lake gate of [`generate_with_a_lake`] and, when
/// `check` is given, the feasibility check ([`feasibility`]) — nothing founded.
///
/// A drawn seed is redrawn, up to `tries`, while the scenario's features are missing or
/// the founders could not be placed on it; among the feasible draws the first that has
/// the lake asked for is returned, else the wettest feasible one with a warning. With no
/// feasible draw at all it refuses. An **asked** seed is the terrain whatever its lake and
/// its feasibility, with what they said as warnings. `go_on` is asked between draws and
/// stops the loop (`Cancelled`).
pub fn draw_terrain(
    cfg: &cubarium_voxel::Config,
    asked: Option<u64>,
    tries: usize,
    mut next_seed: impl FnMut() -> u64,
    check: Option<(&LayerConfig, [usize; Founder::COUNT])>,
    go_on: &dyn Fn() -> bool,
) -> Result<Terrain, NoTerrain> {
    let (want, want_tiers) = match &cfg.landform {
        Landform::Staged(r) => (r.water.min_lake_m2, r.water.min_tier_pools as usize),
        Landform::Terrarium(t) => (t.water.min_lake_m2, t.water.min_tier_pools as usize),
        Landform::Ridge => (0.0, 0),
    };
    let water = cfg.landform.water().copied();
    // One draw, judged: its features, its lake after the gate's settle, and (when asked)
    // its feasibility, read on the same settled probe.
    let judge = |seed: u64| {
        let (world, report) = World::new_reporting(cubarium_voxel::Config { seed, ..cfg.clone() });
        let short = water.and_then(|w| features_short(&w, report.as_ref()));
        let mut probe = world.clone();
        probe.settle(GATE_SETTLE_TICKS);
        let lake = cubarium_voxel::hydrate::lake(&probe);
        let tiers = cubarium_voxel::hydrate::tier_pools(&cubarium_voxel::hydrate::pools(&probe), lake.level_y);
        let feasible = check.map(|(layers, counts)| feasibility(&mut probe, layers, counts).acceptance);
        drop(probe);
        (world, short, lake.visible_m2, tiers, feasible)
    };
    let terrain = |world, seed, lake_m2, tier_pools, feasibility, warnings| Terrain {
        world,
        seed,
        lake_rejected: 0,
        infeasible_rejected: 0,
        lake_m2,
        tier_pools,
        feasibility,
        warnings,
    };
    if let Some(seed) = asked {
        if !go_on() {
            return Err(NoTerrain::Cancelled);
        }
        let (world, short, got, tiers, feasible) = judge(seed);
        let mut warnings = Vec::new();
        if let Some(why) = short {
            warnings.push(format!("seed {seed} was asked for, so it is kept — but {why}"));
        }
        if (want > 0.0 && got < want) || tiers < want_tiers {
            warnings.push(format!(
                "seed {seed} was asked for, so it is kept — but its lake is {got:.1} m² visible \
                 with {tiers} pool(s) above it, against the {want:.1} m² and {want_tiers} a drawn \
                 seed would need"
            ));
        }
        if let Some(a) = feasible.as_ref().filter(|a| !a.accepted) {
            warnings.push(format!(
                "seed {seed} was asked for, so it is kept — but its founders would not all be \
                 placed on the bare terrain ({})",
                a.reasons()
            ));
        }
        for w in &warnings {
            eprintln!("cubarium voxel: {w}");
        }
        return Ok(terrain(world, seed, got, tiers, feasible, warnings));
    }
    let (mut lake_rejected, mut infeasible_rejected) = (0usize, 0usize);
    let mut last_refusal = String::new();
    let mut best: Option<Terrain> = None;
    for _ in 0..tries.max(1) {
        if !go_on() {
            return Err(NoTerrain::Cancelled);
        }
        let seed = next_seed();
        let (world, short, got, tiers, feasible) = judge(seed);
        if let Some(why) = short {
            eprintln!("cubarium voxel: seed {seed} rejected: {why}");
            lake_rejected += 1;
            last_refusal = why;
            continue;
        }
        if let Some(a) = feasible.as_ref().filter(|a| !a.accepted) {
            eprintln!("cubarium voxel: seed {seed} rejected: its founders can't be placed — {}", a.reasons());
            infeasible_rejected += 1;
            last_refusal = format!("the founders can't be placed ({})", a.reasons());
            continue;
        }
        if got >= want && tiers >= want_tiers {
            let mut t = terrain(world, seed, got, tiers, feasible, Vec::new());
            t.lake_rejected = lake_rejected;
            t.infeasible_rejected = infeasible_rejected;
            return Ok(t);
        }
        eprintln!(
            "cubarium voxel: seed {seed} rejected: lake {got:.1} m² visible with {tiers} \
             pool(s) above it, need {want:.1} m² and {want_tiers}"
        );
        lake_rejected += 1;
        last_refusal = format!("a lake of {got:.1} m² with {tiers} pool(s) above it");
        if best.as_ref().is_none_or(|b| (tiers, got) > (b.tier_pools, b.lake_m2)) {
            best = Some(terrain(world, seed, got, tiers, feasible, Vec::new()));
        }
    }
    match best {
        Some(mut t) => {
            let w = format!(
                "no seed of {tries} had a lake of {want:.1} m² with {want_tiers} pool(s) over it; \
                 keeping the wettest the founders fit, seed {} with {:.1} m² and {}",
                t.seed, t.lake_m2, t.tier_pools
            );
            eprintln!("cubarium voxel: {w}");
            t.warnings.push(w);
            t.lake_rejected = lake_rejected;
            t.infeasible_rejected = infeasible_rejected;
            Ok(t)
        }
        None => Err(NoTerrain::Refused(format!(
            "none of {tries} terrains could take the founders: {lake_rejected} short of water or \
             features, {infeasible_rejected} with nowhere to place them (the last: {last_refusal})"
        ))),
    }
}

/// What [`found_terrain`] made of a picked terrain.
pub enum TerrainFounding {
    /// Founded and accepted, on the terrain it was given.
    Founded(Box<FoundedTerrain>),
    /// Every placement try was refused; why each was, in try order.
    Refused(Vec<String>),
    /// The watcher stopped it.
    Cancelled,
}

/// A picked terrain, founded: the world (its terrain unchanged), seeded and accepted.
pub struct FoundedTerrain {
    pub world: World,
    pub flora: Flora,
    pub fauna: Fauna,
    pub seeded: habitat::Seeded,
    /// The terrain's seed, which the world carries again once founded.
    pub seed: u64,
    /// The seed the accepted try's weather and keyed draws ran on ([`placement_seed`]).
    pub placement_seed: u64,
    /// Tries refused before it, and why.
    pub refused: Vec<String>,
}

/// **Found a picked terrain, keeping it**: pre-roll, seed and judge it as [`found`] does,
/// and on a refusal try the placement again **on the same terrain** — the weather and the
/// layers' keyed draws re-keyed to [`placement_seed`] (`World::reseed`), so the shower,
/// the water the founders are judged against and the keyed draws differ — up to `tries`
/// times, or until two tries running are refused for the same reasons (the weather made no
/// difference, and each try is a whole pre-roll). Never draws a new terrain. When a try is accepted the world is re-keyed back to
/// its terrain seed, so it carries the seed it was picked by. `watcher` hears each try
/// ([`FoundingWatch::attempt`]) and every tick of its pre-roll, and can stop it.
pub fn found_terrain(
    world: World,
    seed: u64,
    layers: &LayerConfig,
    founder_counts: [usize; Founder::COUNT],
    tries: usize,
    watcher: &dyn habitat::FoundingWatch,
) -> TerrainFounding {
    found_terrain_judged(world, seed, layers, founder_counts, tries, watcher, |s: &habitat::Seeded| {
        s.acceptance.accepted
    })
}

/// [`found_terrain`] with the verdict `accept` in place of the acceptance check.
pub fn found_terrain_judged(
    world: World,
    seed: u64,
    layers: &LayerConfig,
    founder_counts: [usize; Founder::COUNT],
    tries: usize,
    watcher: &dyn habitat::FoundingWatch,
    accept: impl Fn(&habitat::Seeded) -> bool,
) -> TerrainFounding {
    let tries = tries.max(1);
    let mut refused = Vec::new();
    let mut world = Some(world);
    for k in 0..tries {
        let placement = placement_seed(seed, k as u64);
        watcher.attempt(k, tries, placement);
        // The last try takes the terrain itself; every other a copy of it.
        let mut w = if k + 1 == tries {
            world.take().expect("the terrain is kept until the last try")
        } else {
            world.as_ref().expect("the terrain is kept until the last try").clone()
        };
        if k > 0 {
            w.reseed(placement);
        }
        let Some(pre) = habitat::pre_roll_watched(&mut w, true, None, watcher) else {
            return TerrainFounding::Cancelled;
        };
        watcher.progress(habitat::FoundingPhase::Placing, 0);
        let (mut flora, mut fauna) = layers.layers();
        let seeded = habitat::seed_pre_rolled(&mut w, &mut flora, &mut fauna, &pre, founder_counts);
        if !watcher.go_on() {
            return TerrainFounding::Cancelled;
        }
        if accept(&seeded) {
            if k > 0 {
                eprintln!(
                    "cubarium voxel: seed {seed} founded on placement try {} of {tries} (placement seed \
                     {placement})",
                    k + 1
                );
                w.reseed(seed);
            }
            return TerrainFounding::Founded(Box::new(FoundedTerrain {
                world: w,
                flora,
                fauna,
                seeded,
                seed,
                placement_seed: placement,
                refused,
            }));
        }
        let why = seeded.acceptance.reasons();
        eprintln!(
            "cubarium voxel: seed {seed}, placement try {} of {tries} (placement seed {placement}) \
             refused as a habitat: {why}",
            k + 1
        );
        // The weather made no difference: the same refusal twice running is what the rest
        // would say too, and each try is a whole pre-roll.
        let same = refused.last() == Some(&why);
        refused.push(why);
        if same {
            break;
        }
    }
    TerrainFounding::Refused(refused)
}

/// Why a terrarium build falls short of the water recipe's feature bar
/// ([`cubarium_voxel::Water::min_stream_m`], `min_halls`, `min_groves`), or `None` when
/// it does not. A bar of zero asks for nothing; a generator with no report makes none of
/// the features, so any nonzero bar refuses it.
pub fn features_short(water: &cubarium_voxel::Water, report: Option<&cubarium_voxel::terrarium::Report>) -> Option<String> {
    let (stream, halls, groves) = report.map_or((0.0, 0, 0), |r| (r.river_m, r.halls, r.groves));
    let mut short = Vec::new();
    if water.min_stream_m > 0.0 && stream < water.min_stream_m {
        short.push(format!("a {stream:.1} m stream against {:.1} m", water.min_stream_m));
    }
    if (halls as u32) < water.min_halls {
        short.push(format!("{halls} hall(s) against {}", water.min_halls));
    }
    if (groves as u32) < water.min_groves {
        short.push(format!("{groves} grove site(s) against {}", water.min_groves));
    }
    (!short.is_empty()).then(|| short.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A generated world has to have water somebody can see. The gate redraws until it
    /// does, keeps the wettest when no seed obliges, and never argues with a seed that
    /// was asked for by name.
    /// **Different seeds found different worlds.** With `base + 1` redraws a rejected seed
    /// 1 redrew into seed 2's world, and seeds 1, 2 and 3 of `patch` all founded seed 4's.
    /// Every base's own seed and redraws are distinct from every other base's.
    #[test]
    fn neighbouring_seeds_never_redraw_into_each_other() {
        let mut seen = std::collections::HashMap::new();
        for base in 0..64u64 {
            for k in 0..16u64 {
                let seed = if k == 0 { base } else { derived_seed(base, k) };
                if let Some(other) = seen.insert(seed, base) {
                    assert_eq!(other, base, "seeds {other} and {base} both draw {seed}");
                }
            }
        }
    }

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
        // A random draw supplies only the base; a refusal must derive its retry rather
        // than call this draw again (which would return the same refused seed here).
        let founded = found_judged(&cfg, &layers, [2, 2, 0, 0, 0, 0, 0, 0, 0], Seeds::Drawn(|| 5), false, first_fails);
        assert_eq!(judged.get(), 2, "two worlds were judged");
        assert_eq!(founded.seed, derived_seed(5, 1), "the refused first draw was replaced by the first redraw");
        assert_eq!(founded.world.config().seed, founded.seed, "report the kept world's seed");
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
            [2, 2, 0, 0, 0, 0, 0, 0, 0],
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

    /// A short staged ring with no lake bar and a one-second shower, as the founding-loop
    /// test uses.
    fn short_ring() -> cubarium_voxel::Config {
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
        cfg.climate.opening_min_s = 1.0;
        cfg
    }

    /// Records what a founding told it, and stops it after `stop_after` progress reports.
    #[derive(Default)]
    struct Recorder {
        attempts: std::cell::RefCell<Vec<(usize, usize, u64)>>,
        phases: std::cell::RefCell<Vec<habitat::FoundingPhase>>,
        reports: std::cell::Cell<usize>,
        stop_after: Option<usize>,
    }

    impl habitat::FoundingWatch for Recorder {
        fn attempt(&self, k: usize, of: usize, seed: u64) {
            self.attempts.borrow_mut().push((k, of, seed));
        }
        fn progress(&self, phase: habitat::FoundingPhase, _ticks: u32) {
            self.reports.set(self.reports.get() + 1);
            let mut p = self.phases.borrow_mut();
            if p.last() != Some(&phase) {
                p.push(phase);
            }
        }
        fn go_on(&self) -> bool {
            self.stop_after.is_none_or(|n| self.reports.get() < n)
        }
    }

    /// **A picked terrain keeps its terrain** (terrain-first package): a refused placement
    /// is tried again on the same terrain with the next placement seed, the accepted world
    /// has the terrain it was given and carries its terrain seed again; when every try is
    /// refused it reports each, and draws no other terrain.
    #[test]
    fn founding_a_picked_terrain_retries_on_it_and_then_reports() {
        let cfg = short_ring();
        let layers = LayerConfig::new(&Params::defaults(), cfg.voxel_m);
        let counts = [2, 2, 0, 0, 0, 0, 0, 0, 0];
        // Seed 6: a ring with stands on it, so "seeded" can be checked.
        let terrain = World::new(cubarium_voxel::Config { seed: 6, ..cfg.clone() });
        let hash = terrain.terrain_hash();

        let judged = std::cell::Cell::new(0usize);
        let second = |_: &habitat::Seeded| {
            judged.set(judged.get() + 1);
            judged.get() > 1
        };
        let watch = Recorder::default();
        let TerrainFounding::Founded(f) =
            found_terrain_judged(terrain.clone(), 6, &layers, counts, 3, &watch, second)
        else {
            panic!("the second try is accepted");
        };
        assert_eq!(judged.get(), 2);
        assert_eq!(f.world.terrain_hash(), hash, "the founded world has the picked terrain");
        assert_eq!(f.refused.len(), 1);
        assert_eq!(f.placement_seed, placement_seed(6, 1));
        assert_eq!(f.world.config().seed, 6, "it carries its terrain seed again");
        assert_eq!(*watch.attempts.borrow(), vec![(0, 3, 6), (1, 3, placement_seed(6, 1))]);
        assert!(f.world.tick() > 0 && f.seeded.stands > 0, "pre-rolled and seeded");
        assert_eq!(watch.phases.borrow().first(), Some(&habitat::FoundingPhase::Settling));
        assert_eq!(watch.phases.borrow().last(), Some(&habitat::FoundingPhase::Placing));

        let watch = Recorder::default();
        let TerrainFounding::Refused(why) =
            found_terrain_judged(terrain.clone(), 6, &layers, counts, 2, &watch, |_: &habitat::Seeded| false)
        else {
            panic!("every try is refused");
        };
        assert_eq!(why.len(), 2, "both tries reported");
        assert_eq!(watch.attempts.borrow().len(), 2, "and no more were made");
    }

    /// A watcher that says stop stops the founding part way through.
    #[test]
    fn a_founding_stops_when_its_watcher_says_so() {
        let cfg = short_ring();
        let layers = LayerConfig::new(&Params::defaults(), cfg.voxel_m);
        let terrain = World::new(cubarium_voxel::Config { seed: 5, ..cfg });
        let watch = Recorder { stop_after: Some(30), ..Recorder::default() };
        let made = found_terrain(terrain, 5, &layers, [2, 2, 0, 0, 0, 0, 0, 0, 0], 3, &watch);
        assert!(matches!(made, TerrainFounding::Cancelled));
        assert_eq!(watch.reports.get(), 30, "it stopped at the first refusal to go on");
        assert_eq!(watch.attempts.borrow().len(), 1);
    }

    /// **Terrain candidates**: a drawn terrain the founders can't be placed on is never
    /// offered (redrawn, then refused); one they fit on comes back bare, as generated; an
    /// asked seed comes back whatever the check says, with what it said; and a draw can be
    /// stopped.
    #[test]
    fn terrain_draws_refuse_infeasible_terrains_and_keep_asked_ones() {
        let cfg = short_ring();
        let layers = LayerConfig::new(&Params::defaults(), cfg.voxel_m);
        let mut impossible = [0usize; Founder::COUNT];
        impossible[Founder::Browser.index()] = 100_000;
        let drawn = std::cell::Cell::new(100u64);
        let next = || {
            drawn.set(drawn.get() + 1);
            drawn.get()
        };
        let go = || true;
        let refused = draw_terrain(&cfg, None, 3, next, Some((&layers, impossible)), &go);
        assert!(matches!(&refused, Err(NoTerrain::Refused(why)) if why.contains("can't be placed")), "{:?}", refused.err());
        assert_eq!(drawn.get(), 103, "three draws, all refused");

        let fits = draw_terrain(&cfg, None, 3, next, Some((&layers, [0; Founder::COUNT])), &go).expect("a terrain");
        assert!(fits.feasibility.as_ref().is_some_and(|a| a.accepted));
        let fresh = World::new(cubarium_voxel::Config { seed: fits.seed, ..cfg.clone() });
        assert_eq!(fits.world.terrain_hash(), fresh.terrain_hash());
        assert_eq!(fits.world.tick(), 0, "nothing was stepped on the terrain returned");

        let asked = draw_terrain(&cfg, Some(7), 3, next, Some((&layers, impossible)), &go).expect("kept");
        assert_eq!(asked.seed, 7);
        assert!(asked.feasibility.as_ref().is_some_and(|a| !a.accepted));
        assert!(asked.warnings.iter().any(|w| w.contains("asked")), "{:?}", asked.warnings);

        let stop = || false;
        assert!(matches!(draw_terrain(&cfg, None, 3, next, None, &stop), Err(NoTerrain::Cancelled)));
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
