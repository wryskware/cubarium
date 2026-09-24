//! The seeded example habitat for `cubarium voxel`.
//!
//! A fresh `cubarium voxel` run starts with an empty ecology: a stand appears only from
//! an `f` stdin line and an animal only from a `g` line. That is the right contract for a
//! harness and the wrong one for `--sink gpu`, whose whole job is to put a living world on
//! a screen without anyone typing at it. This module is the smallest thing that closes the
//! gap: one deterministic founder habitat built entirely out of the flora and fauna
//! crates' own commands, with no model rule changed and no new mechanic.
//!
//! Since the live-founders package the animals it places are the two **sensed founder
//! lineages** — `Founder::Blind` (littershredder) and `Founder::Browser` (frondgrazer
//! founder) — introduced hungry through `Command::IntroduceFounder` with their own
//! observation-only heuristics installed. The legacy `Species::Frondgrazer` animal is
//! still in the fauna crate and still reachable from the `g` stdin line; this seeder
//! stopped placing it. Those heuristics are the seeder's floor: the ambient run installs
//! the trained P3-C centres over them by default (`crates/cubarium/assets/policies`),
//! and `--founder-heuristic` is what leaves this floor standing.
//!
//! **It is a stage-1 dev scene, not a tuned ecology.** The species are placed by simple
//! environment proxies — standing water, rock or soil, height band — in clustered,
//! mixed-size patches. This follows the Stage 1 treatment and habitat grouping in
//! `design/art-direction/Cubarium_Art_Direction_v0.1.md`; the counts are chosen for a
//! populated picture. Nothing here is a balance claim, nothing here persists, and the
//! world it makes is the ordinary disposable development world. `--empty` asks for the
//! bare world back.

use std::sync::Arc;

use std::collections::BTreeMap;

use cubarium_voxel::{Material, Settle, VoxelView, World};
use cubarium_voxel_fauna::{
    BlindForager, BrowserForager, Command as FaunaCommand, Controller, Fauna, Founder,
    FounderPhysiology, RouteMap, StartingStores, has_headroom,
};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraView, Site, Species, SpeciesConfig,
    adult_light_cover, establishment_gates_with_sky,
};

/// Hard cap on the startup settle, in ticks. `World::settle` stops as soon as the pooled
/// volume and the wet-cell count have both stopped moving; this is the bound on how long
/// it may look (`design/terrain-generation-plan-2026-09-21.md` §4.4).
const SETTLE_CAP: u32 = 600;

/// Ticks the stream is watched for after the settle, before anything is planted. Ten
/// simulated seconds: long enough for a spill front to finish arriving anywhere in the
/// chain, short enough to cost nothing anybody waits for.
const STREAM_WATCH_TICKS: u32 = 200;

/// Mixed starting sizes establish low growth, middle-height clumps and occasional
/// canopy anchors. These are actual wood stores, not cosmetic scale multipliers.
const FOUNDER_FRACTIONS: [f64; 5] = [0.30, 0.42, 0.55, 0.70, 0.85];

/// Dead wood laid under each glowcap: twice the fungus's `establish_substrate_min`, enough
/// for one grove to establish and spread along its log.
const LOG_ORGANIC: f64 = 0.4;

/// The five producers, how many founders per **square metre of habitat they can actually
/// live in**, the size of one colony, and the cap a world puts on any one species.
///
/// The rates are the stage-1 scene's own counts read off the ring they were chosen on --
/// 128 x 24 at 0.25 m is 192 m² -- so a `default` ring looks as it did while a wider one
/// gets more geography's worth of plants rather than the same number spread thinner
/// (plan §5: "count founders by usable physical area and intended starting coverage,
/// with small-world caps, rather than fixed totals").
///
/// Package N's three are placed wherever their own gates pass, on the same terms: the
/// vaulttree **first**, as a few single canopy anchors, so the crowns it places shade the
/// sites the rest are then chosen on; the siphonreed in clumps on the banks its
/// standing-water gate finds (a dense rate over a narrow habitat: a reed bed), the
/// lanternberry in small groups. Their rates and caps are placeholders
/// (`design/backlog.md` §1).
const PRODUCERS: [(Species, f64, usize, usize); 8] = [
    (Species::Vaulttree, 0.1, 1, 12),
    (Species::Bloomcrown, 0.25, 3, 64),
    (Species::Umbrellafrond, 0.15, 3, 40),
    (Species::Siphonreed, 4.0, 6, 64),
    (Species::Springturf, 0.6, 6, 160),
    (Species::Velvetpad, 0.4, 6, 120),
    (Species::Stonecushion, 0.2, 3, 64),
    (Species::Lanternberry, 0.25, 3, 40),
];

/// The decomposer grove, on the same terms.
const GLOWCAPS: (f64, usize, usize) = (0.15, 3, 40);

/// Least distance between two patch centres, metres. Colonies grow out from their own
/// anchor, so this is what keeps gaps and transition zones between them.
const PATCH_SPACING_M: f64 = 1.5;

/// How much of a site's sky a crown standing over it takes away. A **placement** proxy
/// and not the light model: the flora layer computes real shading every tick, and what
/// this does is stop the seeder from planting a second founder in a spot the first one
/// has just put in shade (plan §5: "recheck light suitability as canopies are placed").
const CANOPY_SHADE: f64 = 0.6;

/// The height a body steps between neighbouring support faces, metres — the same bound
/// `cubarium_voxel::walk::around_the_ring` asks the generator to keep, so a colony grows
/// over ground something could actually walk across.
const STEP_M: f64 = 0.5;

/// How many **sensed founder bodies** of each lineage the habitat starts with: eight
/// littershredders ([`Founder::Blind`]) and eight frondgrazer founders
/// ([`Founder::Browser`]), the same count the legacy `Species::Frondgrazer`
/// introductions used, now split across the two lineages. The legacy species stays in
/// the fauna crate and is still reachable from the `g` stdin line; this seeder simply
/// stops placing it.
const SHREDDERS: usize = 8;
const BROWSERS: usize = 8;

/// Leaf litter laid under each littershredder, in organic-matter units.
///
/// A fresh world has **no litter at all** — litterfall is senescence, and nothing has
/// senesced before the first tick — so a blind litter feeder placed on bare soil would
/// start with nothing to smell and nothing to eat. This is the same move the glowcap
/// grove already makes with its log: lay the substrate the founder's own gate needs. At
/// 0.2 the tile saturates the cue's emission term (`min(litter / 0.05, 1)`) exactly as a
/// Stage-A arena tile does, so the founder starts on a signal it has actually been seen
/// to follow. It is a dev-scene starter, not a claim about a standing litter layer.
const LITTER_ORGANIC: f64 = 0.2;
/// Mineral and retained-energy densities of that starter litter: a plant tissue's order
/// of magnitude, and the litter energy cap. The same pair the frozen arena deposits at.
const LITTER_MINERAL_FRACTION: f64 = 0.02;
const LITTER_ENERGY_DENSITY: f64 = 2.0;

/// **N**, the acceptance bar: a lineage's component must hold at least this many
/// founder-hours of adult basal upkeep in reachable edible stock for every founder
/// placed in it (decisions §8). An authored placeholder, `design/backlog.md` §1.
pub const ACCEPT_FOUNDER_HOURS: f64 = 1.0;

/// What the startup pre-roll did before anything was seeded.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PreRollReport {
    /// Whether the world opened with a shower (a scheduled closed cycle whose store
    /// could pay for one).
    pub opening_shower: bool,
    /// Ticks the opening shower fell for.
    pub shower_ticks: u32,
    /// The drain after it: the same convergence as the startup settle.
    pub drain: Settle,
    /// Every tick the pre-roll stepped: settle, shower, drain and the stream watch.
    pub ticks: u32,
}

/// The pre-roll's result: what it did, and the deepest standing water each support face
/// saw over the **whole** of it, shower included.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreRoll {
    /// The startup settle, before the shower.
    pub settle: Settle,
    pub report: PreRollReport,
    /// Every support face, in [`support_sites`] order.
    pub sites: Vec<Site>,
    /// The deepest water each of `sites` stood under at any tick after the settle.
    pub wettest: Vec<f64>,
}

/// One walkable component's food for one lineage, at seeding.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ComponentFood {
    /// Edible organic matter a mouth reaches from faces in this component.
    pub stock: f64,
    /// Stands that stock comes from (browser: vascular stands; shredder: glowcaps).
    pub stands: usize,
    /// Living producers of the renewable part rooted in the component (shredder:
    /// litter-shedding stands and glowcaps; browser: the reached stands themselves).
    pub producers: usize,
    /// **Reported, not gated.** Browser: the reached stands' foliage regrowth ceiling
    /// `r_p · W`; shredder: the rooted producers' litterfall `m_p · P`; per hour.
    pub production_per_h: f64,
}

/// One component a lineage's founders were placed in, judged.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ComponentVerdict {
    pub component: usize,
    pub founders: usize,
    pub food: ComponentFood,
    pub habitable: bool,
}

/// One lineage's acceptance.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineageVerdict {
    pub wanted: usize,
    pub placed: usize,
    /// Adult basal upkeep per founder per hour ([`upkeep_per_hour`]).
    pub upkeep_per_h: f64,
    /// Every component holding at least one of this lineage's founders.
    pub components: Vec<ComponentVerdict>,
    /// Founders standing on no face of the lineage's route map at all.
    pub stranded: usize,
    pub accepted: bool,
    /// The worst founder-occupied component's `stock / (founders · upkeep_per_h)`,
    /// scaled by `placed / wanted`; zero when nothing was placed and something was wanted.
    pub ratio: f64,
}

/// The world's acceptance (decisions §8): every lineage placed its full count, each in a
/// habitable component.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Acceptance {
    pub lineages: [LineageVerdict; Founder::COUNT],
    pub accepted: bool,
}

impl Acceptance {
    /// The lower of the two lineages' ratios: what "best of a bad lot" ranks on.
    pub fn worse_ratio(&self) -> f64 {
        self.lineages
            .iter()
            .map(|l| l.ratio)
            .fold(f64::INFINITY, f64::min)
    }

    /// Why a world was refused, one clause per failing lineage; empty when accepted.
    pub fn reasons(&self) -> String {
        let mut out: Vec<String> = Vec::new();
        for founder in Founder::ALL {
            let l = &self.lineages[founder.index()];
            if l.accepted {
                continue;
            }
            let mut why: Vec<String> = Vec::new();
            if l.placed < l.wanted {
                why.push(format!("placed {} of {}", l.placed, l.wanted));
            }
            if l.stranded > 0 {
                why.push(format!("{} off the route map", l.stranded));
            }
            for c in l.components.iter().filter(|c| !c.habitable) {
                why.push(format!(
                    "component {} holds {:.3} for {} founder(s) needing {:.3}{}",
                    c.component,
                    c.food.stock,
                    c.founders,
                    ACCEPT_FOUNDER_HOURS * l.upkeep_per_h * c.founders as f64,
                    if founder == Founder::Blind && c.food.producers == 0 {
                        " and has no living producer"
                    } else {
                        ""
                    },
                ));
            }
            out.push(format!("{}: {}", founder.name(), why.join("; ")));
        }
        out.join(" | ")
    }
}

/// What [`seed`] put into the world.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Seeded {
    pub stands: usize,
    pub logs: usize,
    /// Founder bodies placed, by [`Founder::index`]: littershredders, then frondgrazer
    /// founders.
    pub founders: [usize; Founder::COUNT],
    /// Bodies asked for and **not placed**, by [`Founder::index`]: the habitat held no
    /// site this lineage could live on. Reported, not papered over with a foodless
    /// fallback (plan §5).
    pub shortfall: [usize; Founder::COUNT],
    /// Litter tiles laid for the littershredders.
    pub litter_tiles: usize,
    /// What the startup settle measured before anything was planted.
    pub settle: Settle,
    /// What the rest of the pre-roll did.
    pub pre_roll: PreRollReport,
    /// The acceptance check on the seeded world.
    pub acceptance: Acceptance,
    /// Founder stands planted, by [`Species::index`].
    pub stands_by_species: [usize; Species::COUNT],
    /// Faces each species was eligible for after the pre-roll (its establishment gates,
    /// adult upkeep and the water it would have stood in), by [`Species::index`]. A zero
    /// is an unmet niche.
    pub eligible_by_species: [usize; Species::COUNT],
    /// Everything the seeder brought in from outside, organic units: founder stands,
    /// logs, starter litter and founder bodies.
    pub imported_organic: f64,
}

impl Seeded {
    /// Every animal the seeder introduced.
    pub fn animals(&self) -> usize {
        self.founders.iter().sum()
    }
}

/// Seed the example habitat on a **fresh** world: its [`pre_roll`] first — settle, the
/// opening shower and its drain, the stream watched — and then every unit enters through
/// the ordinary founder, deposit and introduction inflows the layers already name.
/// Deterministic for a given world.
pub fn seed(world: &mut World, flora: &mut Flora, fauna: &mut Fauna) -> Seeded {
    seed_with_founder_counts(world, flora, fauna, [SHREDDERS, BROWSERS])
}

/// The default founder count per lineage: what [`seed`] places.
pub const FOUNDER_COUNTS: [usize; Founder::COUNT] = [SHREDDERS, BROWSERS];

/// Seed the example habitat with a caller-selected number of bodies per sensed lineage.
/// The normal ambient scene uses [`seed`]; this variant exists for bounded diagnosis arms
/// that need to change only the starting population while keeping the seeder's placement,
/// stores and controller setup identical.
pub fn seed_with_founder_counts(
    world: &mut World,
    flora: &mut Flora,
    fauna: &mut Fauna,
    founder_counts: [usize; Founder::COUNT],
) -> Seeded {
    let pre = pre_roll(world, true);
    seed_pre_rolled(world, flora, fauna, &pre, founder_counts)
}

/// Hard cap on the opening shower, in ticks: five simulated minutes. A preset's shower
/// is sized to fall in about a minute on its own footprint (`cubarium_voxel::Water`), so
/// this only ends a shower that could not finish.
const SHOWER_CAP: u32 = 6000;

/// Hard cap on the drain after the opening shower, in ticks: two simulated minutes. The
/// drain stops as soon as `World::settle`'s convergence says the water has stopped
/// moving; this is only the bound on how long it may look. Measured at seed 1 with no
/// cap: `small` converges in 100 ticks, `wide` in 1,235, `default` in 1,889 — a
/// one-minute cap stopped both larger rings still moving.
const DRAIN_CAP: u32 = 2400;

/// The startup pre-roll, before anything is seeded.
///
/// 1. **Settle** as the startup always has ([`SETTLE_CAP`]).
/// 2. For a fresh world with a scheduled cycle, **open with a shower**: the first shower
///    is made due now instead of at its drawn 5–15 minutes (later ones are drawn exactly
///    as before, from where this one ends), falls, and the water **drains** to rest
///    again under the settle's own convergence, capped at [`DRAIN_CAP`]. A closed
///    world's outlet is opened first, as the live run opens it before its first tick:
///    the shower the founders are judged against is the shower the running world has.
/// 3. **Watch the stream** for [`STREAM_WATCH_TICKS`], as before.
///
/// From the end of the settle on, every support face remembers the deepest water it
/// stood under, so a species is offered only a face it could have stood through that
/// shower on. The settle itself is not watched: out of the generator a lake is a flat
/// sheet laid to its datum that finds its own hollows in the first ticks, which is the
/// world arriving, not weather.
///
/// `open_with_a_shower` is `false` for a **resumed** world, which has had its weather.
pub fn pre_roll(world: &mut World, open_with_a_shower: bool) -> PreRoll {
    let settle = world.settle(SETTLE_CAP);
    let sites = support_sites(world);
    let mut wettest = vec![0.0f64; sites.len()];
    let watch = |w: &World, wettest: &mut [f64]| {
        let view = w.view();
        for (d, s) in wettest.iter_mut().zip(&sites) {
            let here = view.water_depth_m(i64::from(s.x), s.y, s.z);
            if here > *d {
                *d = here;
            }
        }
    };
    let mut report = PreRollReport {
        ticks: settle.ticks,
        ..PreRollReport::default()
    };
    if open_with_a_shower && world.config().shower_interval_max_s > 0.0 {
        if world.config().closed_water_budget && !world.outlet_open() {
            world.apply(cubarium_voxel::Command::SetOutlet { open: true });
        }
        let before = world.view().ledger.showers;
        world.bring_shower_forward();
        let mut ticks = 0u32;
        while ticks < SHOWER_CAP {
            world.step();
            ticks += 1;
            watch(world, &mut wettest);
            let started = world.view().ledger.showers > before;
            // A store under its floor cannot pay for the shower on its due tick; it
            // stays due and falls when the store can, in the running world.
            if !started || world.shower_left_m3() <= 0.0 {
                break;
            }
        }
        report.opening_shower = world.view().ledger.showers > before;
        report.shower_ticks = ticks;
        report.drain = world.settle_watching(DRAIN_CAP, |w| watch(w, &mut wettest));
        report.ticks += ticks + report.drain.ticks;
    }
    // **Watch the stream before planting in its way.** `settle` converges on the world's
    // stored volume and its wet-cell count, and under river re-entry both are constant
    // while the water is still *moving*: a spill front can be half way down the cascade
    // when the settle reports itself converged. Founders placed on the faces it is about
    // to reach drown in the first seconds — measured on `default`, an umbrellafrond under
    // 0.99 m one tick after seeding and a velvetpad under 0.19 m twelve ticks later, both
    // bone dry when they were planted.
    for _ in 0..STREAM_WATCH_TICKS {
        world.step();
        watch(world, &mut wettest);
    }
    report.ticks += STREAM_WATCH_TICKS;
    PreRoll {
        settle,
        report,
        sites,
        wettest,
    }
}

/// Seed the habitat on a world that has had its [`pre_roll`]. Every eligibility gate
/// reads the world as the pre-roll left it, and every water gate reads the deepest water
/// the pre-roll saw.
pub fn seed_pre_rolled(
    world: &mut World,
    flora: &mut Flora,
    fauna: &mut Fauna,
    pre: &PreRoll,
    founder_counts: [usize; Founder::COUNT],
) -> Seeded {
    let mut seeded = Seeded {
        settle: pre.settle,
        pre_roll: pre.report,
        ..Seeded::default()
    };
    let organic_before = flora.view().ledger.seeded_organic_in
        + flora.view().ledger.deposited_organic_in
        + fauna.view().ledger.introduced_organic_in;

    // Every **support face**, not the skyline: a hollow's floor is a place to live, and
    // since slice 2b the rings have them (caves plan, "Ecology and presentation
    // follow-ups"). Sky visibility is read once per site and then carried, so a crown
    // placed here shades the sites under it for every species chosen after it. Since
    // package 4 the face checked is the face planted (`Command::SeedOnFace`).
    let sites = &pre.sites;
    let wettest = &pre.wettest;
    let Some(highest_y) = sites.iter().map(|s| s.y).max() else {
        seeded.acceptance = accept(world, flora, fauna, founder_counts);
        return seeded;
    };
    let mut sky: Vec<f64> = {
        let view = world.view();
        sites
            .iter()
            .map(|s| view.sky_visibility(i64::from(s.x), s.y, s.z))
            .collect()
    };
    let area = world.config().cell_area();
    let rise = (STEP_M / world.config().voxel_m).floor().max(1.0) as u32;
    let spacing = (PATCH_SPACING_M / world.config().voxel_m).round().max(1.0) as u32;
    let width = world.config().width;
    let voxel_m = world.config().voxel_m;
    let mut taken: Vec<Site> = Vec::new();
    // The lake, read once: a species that needs standing water is offered only its banks.
    let lake = lake_cells(world);

    // The producers. Suitability is the **flora layer's own** establishment gates
    // plus its adult-upkeep check — a seed gate alone does not establish that a founder
    // can keep itself standing — and the terrain-sector proxy only decides which species
    // *prefers* a site it is already allowed to live on.
    for (species, per_m2, patch_size, cap) in PRODUCERS {
        let sc = flora.config().species(species).clone();
        let mut pool: Vec<Site> = Vec::new();
        {
            let view = world.view();
            for (i, site) in sites.iter().enumerate() {
                if wettest[i] > sc.drown_depth_m || !suitable(&view, *site, &sc, sky[i]) {
                    continue;
                }
                // Standing water that lasts: the lake's margins, and not a puddle the
                // pre-roll left that drains within the hour ([`lake_margin`]).
                if sc.water_depth_min_m > 0.0 && !lake_margin(&view, &lake, *site) {
                    continue;
                }
                seeded.eligible_by_species[species.index()] += 1;
                if taken.contains(site) {
                    continue;
                }
                pool.push(*site);
            }
            // Preferred ground first, so the picture stays banded by landform, then the
            // rest of the habitat the species is welcome in.
            pool.sort_by_key(|s| {
                let preferred = prefers(&view, *s, highest_y) == species;
                (!preferred, s.y, s.x, s.z)
            });
        }
        let want = ((pool.len() as f64 * area * per_m2).round() as usize)
            .min(cap)
            .min(pool.len());
        let wood_max = sc.wood_max;
        for (i, site) in colonies(&pool, want, patch_size, spacing, width, rise)
            .into_iter()
            .enumerate()
        {
            // The light may have gone since the pool was built: a crown placed two
            // founders ago can shade this one out.
            let Some(k) = index_of(sites, site) else {
                continue;
            };
            if sky[k] < sc.establish_light_min || adult_light_cover(&sc, sky[k]) < 1.0 {
                continue;
            }
            let wood = FOUNDER_FRACTIONS[i % FOUNDER_FRACTIONS.len()] * wood_max;
            if flora.apply(
                world,
                FloraCommand::SeedOnFace {
                    site,
                    species,
                    wood,
                },
            ) {
                taken.push(site);
                seeded.stands += 1;
                seeded.stands_by_species[species.index()] += 1;
                shade_under(sites, &mut sky, site, &sc, wood, width, voxel_m);
            }
        }
    }

    // The decomposer grove: a log first, then the fungus that eats it. A fresh world has
    // no dead wood at all, so the log is the habitat and glowcap's own substrate gate
    // could not pass before it is laid — which is why the deposit is booked first and the
    // seed is offered only where it went in.
    let sc = flora.config().species(Species::Glowcap).clone();
    let mut pool: Vec<Site> = Vec::new();
    {
        let view = world.view();
        for (i, site) in sites.iter().enumerate() {
            if wettest[i] > sc.drown_depth_m {
                continue;
            }
            // The substrate is the log this seeder is about to lay, so the gate is asked
            // with it in hand; everything else is read off the world as it stands.
            let gates = establishment_gates_with_sky(&view, *site, &sc, sky[i], LOG_ORGANIC, 0.0);
            if gates.passes()
                && view.material_at(i64::from(site.x), site.y, site.z) == Material::Soil
            {
                seeded.eligible_by_species[Species::Glowcap.index()] += 1;
                if !taken.contains(site) {
                    pool.push(*site);
                }
            }
        }
    }
    let (per_m2, patch_size, cap) = GLOWCAPS;
    let want = ((pool.len() as f64 * area * per_m2).round() as usize)
        .min(cap)
        .min(pool.len());
    for (i, site) in colonies(&pool, want, patch_size, spacing, width, rise)
        .into_iter()
        .enumerate()
    {
        if !flora.deposit(
            site,
            Deposit {
                kind: DepositKind::DeadWood,
                organic: LOG_ORGANIC,
                mineral: sc.n_tissue * LOG_ORGANIC,
                energy: sc.energy_density * LOG_ORGANIC,
            },
        ) {
            continue;
        }
        seeded.logs += 1;
        let wood = FOUNDER_FRACTIONS[i % FOUNDER_FRACTIONS.len()] * sc.wood_max;
        if flora.apply(
            world,
            FloraCommand::SeedOnFace {
                site,
                species: Species::Glowcap,
                wood,
            },
        ) {
            taken.push(site);
            seeded.stands += 1;
            seeded.stands_by_species[Species::Glowcap.index()] += 1;
        }
    }

    // The two **sensed founder lineages**, which is what walks this habitat now. Both
    // arrive hungry (`StartingStores::HUNGRY`, the arenas' P2-C start): a body placed
    // full has nowhere to put what it eats, so eating would be worth nothing to it.
    // The seeder installs each lineage's own observation-only heuristic through the
    // fauna layer's ordinary controller boundary, and registers it as that lineage's
    // **birth factory** too, so a founder born here is handed a fresh controller of its
    // parent's kind instead of resting for ever while paying upkeep.
    //
    // That heuristic is the seeder's floor, not the live run's default: the ambient run
    // replaces both the factory and the standing bodies' controllers afterwards with the
    // trained P3-C centre built into the binary (`crates/cubarium/assets/policies`),
    // unless `--founder-heuristic` asks for this floor as the disclosed control or
    // `--founder-policy` names another centre. A caller that seeds a habitat without
    // going through `cubarium voxel` gets the heuristics and nothing else.
    install_heuristics(fauna);

    // **Founders only where they can live** (decisions §8). Each lineage's walkable
    // components are read on its own adult body, wade depth and climb, with the food a
    // mouth reaches from each; a component takes founders only while its stock covers
    // `N` founder-hours of upkeep for every one of them, and (shredder) only if a living
    // producer of its renewable food is rooted in it.

    // Littershredders on litter-bearing soil: dry open soil away from the stands, each
    // with its own starter tile of leaf litter laid under it first, so the founder
    // begins on a cue it can smell and a stock it can bite. The starter tile is stock
    // too, and it is counted.
    let blind = *fauna.config().founder(Founder::Blind);
    let blind_room = blind.adult_body().headroom_voxels(world.config().voxel_m);
    let blind_need = ACCEPT_FOUNDER_HOURS * upkeep_per_hour(&blind);
    let (blind_map, blind_food) =
        lineage_food(&world.view(), &flora.view(), &blind, Founder::Blind);
    let pool: Vec<Site> = {
        let view = world.view();
        sites
            .iter()
            .copied()
            .enumerate()
            .filter(|(i, s)| {
                !taken.contains(s)
                    && wettest[*i] <= blind.core.drown_depth_m
                    && view.material_at(i64::from(s.x), s.y, s.z) == Material::Soil
                    && view.water_depth_m(i64::from(s.x), s.y, s.z) <= 0.0
                    && has_headroom(&view, i64::from(s.x), s.y, s.z, blind_room)
                    && blind_map
                        .component_of(*s)
                        .and_then(|c| blind_food.get(&c))
                        .is_some_and(|f| f.producers > 0)
            })
            .map(|(_, s)| s)
            .collect()
    };
    let want = founder_counts[Founder::Blind.index()];
    let chosen = place(
        &pool,
        want,
        |s| blind_map.component_of(s),
        |c, k| {
            let stock = blind_food.get(&c).map_or(0.0, |f| f.stock);
            stock + k as f64 * LITTER_ORGANIC >= blind_need * k as f64
        },
    );
    for (k, site) in chosen.into_iter().enumerate() {
        if !flora.deposit(
            site,
            Deposit {
                kind: DepositKind::Litter,
                organic: LITTER_ORGANIC,
                mineral: LITTER_ORGANIC * LITTER_MINERAL_FRACTION,
                energy: LITTER_ORGANIC * LITTER_ENERGY_DENSITY,
            },
        ) {
            continue;
        }
        seeded.litter_tiles += 1;
        if introduce_founder(world, fauna, Founder::Blind, site, spread_heading(k, want)) {
            taken.push(site);
            seeded.founders[Founder::Blind.index()] += 1;
        }
    }
    seeded.shortfall[Founder::Blind.index()] =
        want.saturating_sub(seeded.founders[Founder::Blind.index()]);

    // Frondgrazer founders, **after the food**. A browser needs a face with room for its
    // body, water under its wade depth, a stocked foliage layer its mouth actually
    // reaches from there, and somewhere else to go when that patch is bare — a
    // walkable route to a second stand. There is no foodless fallback: a lineage with
    // nowhere to live is a shortfall in `Seeded` and nothing is placed (plan §5).
    let browser = *fauna.config().founder(Founder::Browser);
    let browser_need = ACCEPT_FOUNDER_HOURS * upkeep_per_hour(&browser);
    let browser_map = RouteMap::for_founder(&world.view(), &browser);
    let browser_food = food_on(&browser_map, &world.view(), &flora.view(), Founder::Browser);
    let mut meadow = browser_faces_on(&browser_map, &browser_food, &world.view(), &flora.view());
    // A body drowns in the stream's way as surely as a plant does.
    meadow.retain(|f| index_of(sites, *f).is_none_or(|i| wettest[i] <= browser.core.drown_depth_m));
    let want = founder_counts[Founder::Browser.index()];
    let chosen = place(
        &meadow,
        want,
        |s| browser_map.component_of(s),
        |c, k| browser_food.get(&c).map_or(0.0, |f| f.stock) >= browser_need * k as f64,
    );
    for (k, site) in chosen.into_iter().enumerate() {
        if introduce_founder(
            world,
            fauna,
            Founder::Browser,
            site,
            spread_heading(k, want),
        ) {
            seeded.founders[Founder::Browser.index()] += 1;
        }
    }
    seeded.shortfall[Founder::Browser.index()] =
        want.saturating_sub(seeded.founders[Founder::Browser.index()]);

    seeded.imported_organic = flora.view().ledger.seeded_organic_in
        + flora.view().ledger.deposited_organic_in
        + fauna.view().ledger.introduced_organic_in
        - organic_before;
    seeded.acceptance = accept(world, flora, fauna, founder_counts);
    seeded
}

/// Choose up to `want` faces from `pool`, spread over it: a stride sample first and then
/// the rest of the pool in order, taking a face only while `fits(component, k)` says its
/// component can carry `k` founders with this one among them. A face on no component is
/// never taken.
fn place(
    pool: &[Site],
    want: usize,
    component: impl Fn(Site) -> Option<usize>,
    fits: impl Fn(usize, usize) -> bool,
) -> Vec<Site> {
    let mut chosen: Vec<Site> = Vec::new();
    let mut count: BTreeMap<usize, usize> = BTreeMap::new();
    let first = strided(pool, want);
    for site in first.iter().chain(pool.iter()) {
        if chosen.len() >= want {
            break;
        }
        if chosen.contains(site) {
            continue;
        }
        let Some(c) = component(*site) else {
            continue;
        };
        let k = count.get(&c).copied().unwrap_or(0) + 1;
        if fits(c, k) {
            count.insert(c, k);
            chosen.push(*site);
        }
    }
    chosen
}

/// A lineage's adult **basal upkeep per hour**, organic units:
/// `core.maintenance_per_s · core.body_max · 3600`. Basal maintenance only — no motor
/// respiration, no gestation — of a body at the size a founder grows into.
pub fn upkeep_per_hour(phys: &FounderPhysiology) -> f64 {
    phys.core.maintenance_per_s * phys.core.body_max * 3600.0
}

/// Each walkable component's food for a lineage, and the route map it was read on.
pub fn lineage_food(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    phys: &FounderPhysiology,
    founder: Founder,
) -> (RouteMap, BTreeMap<usize, ComponentFood>) {
    let map = RouteMap::for_founder(view, phys);
    let food = food_on(&map, view, fv, founder);
    (map, food)
}

/// The food each component of `map` holds for `founder`'s mouth.
///
/// - **Browser**: the **edible** foliage ([`cubarium_voxel_flora::StandLayer::edible`],
///   above the grazing floor) of every vascular stand's layers a mouth reaches
///   from a face in the component ([`RouteMap::faces_reaching_layer`], the tick's own
///   band and per-layer scan with the diet gate). Every such stand is its own producer.
/// - **Shredder**: litter and carrion a mouth reaches at a face's own height
///   ([`RouteMap::faces_reaching_pool`]), plus glowcap cap tissue in its band; its
///   producers are the living stands **rooted** in the component that renew any of it —
///   a stand that sheds litter, or a glowcap.
///
/// Each stock counts once per component however many faces reach it.
fn food_on(
    map: &RouteMap,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    founder: Founder,
) -> BTreeMap<usize, ComponentFood> {
    let diet = cubarium_voxel_fauna::Diet::of(founder);
    let mut food: BTreeMap<usize, ComponentFood> = BTreeMap::new();
    for stand in fv.stands.iter().filter(|s| s.foliage > 0.0) {
        let sc = fv.config.species(stand.species);
        if !diet.accepts(sc.trophic) {
            continue;
        }
        let mut reached: std::collections::BTreeSet<usize> = Default::default();
        for layer in fv.layers(stand) {
            let comps: std::collections::BTreeSet<usize> = map
                .faces_reaching_layer(fv, view, stand, &layer, diet)
                .into_iter()
                .map(|i| map.components[i])
                .collect();
            // What a mouth could take: the layer's edible foliage above its grazing
            // floor, not the refuge under it that no bite reaches (package G).
            for c in comps {
                food.entry(c).or_default().stock += layer.edible();
                reached.insert(c);
            }
        }
        for c in reached {
            let f = food.entry(c).or_default();
            f.stands += 1;
            if founder == Founder::Browser {
                f.producers += 1;
                f.production_per_h += sc.foliage_rate * stand.wood * 3600.0;
            }
        }
    }
    if founder == Founder::Blind {
        for g in fv.ground.iter() {
            let amount = g.litter.max(0.0) + g.carrion.max(0.0);
            if !(amount > 0.0) {
                continue;
            }
            let comps: std::collections::BTreeSet<usize> = map
                .faces_reaching_pool(g.site)
                .into_iter()
                .map(|i| map.components[i])
                .collect();
            for c in comps {
                food.entry(c).or_default().stock += amount;
            }
        }
        for stand in fv.stands.iter() {
            let sc = fv.config.species(stand.species);
            let sheds = sc.senescence > 0.0 && stand.foliage > 0.0;
            let cap = stand.species == Species::Glowcap;
            if !(sheds || cap) {
                continue;
            }
            let Some(c) = map.component_of(stand.site) else {
                continue;
            };
            let f = food.entry(c).or_default();
            f.producers += 1;
            f.production_per_h += sc.senescence * stand.foliage.max(0.0) * 3600.0;
        }
    }
    food
}

/// **The acceptance check** (decisions §8), read off the world as it stands: for each
/// lineage, the components its living founders stand in, each component's reachable
/// edible stock against `N · founders · upkeep`, and (shredder) a living producer rooted
/// in it. A world is accepted when every lineage has placed `wanted` founders, all of
/// them in habitable components.
pub fn accept(
    world: &World,
    flora: &Flora,
    fauna: &Fauna,
    wanted: [usize; Founder::COUNT],
) -> Acceptance {
    let view = world.view();
    let fv = flora.view();
    let av = fauna.view();
    let mut out = Acceptance::default();
    for founder in Founder::ALL {
        let phys = *fauna.config().founder(founder);
        let (map, food) = lineage_food(&view, &fv, &phys, founder);
        let u = upkeep_per_hour(&phys);
        let mut v = LineageVerdict {
            wanted: wanted[founder.index()],
            upkeep_per_h: u,
            ..LineageVerdict::default()
        };
        let mut count: BTreeMap<usize, usize> = BTreeMap::new();
        for a in av.animals.iter().filter(|a| a.founder == Some(founder)) {
            v.placed += 1;
            match map.component_of(a.site) {
                Some(c) => *count.entry(c).or_default() += 1,
                None => v.stranded += 1,
            }
        }
        let mut worst = f64::INFINITY;
        for (c, n) in count {
            let f = food.get(&c).copied().unwrap_or_default();
            let need = ACCEPT_FOUNDER_HOURS * u * n as f64;
            let habitable =
                f.stock >= need && (founder != Founder::Blind || f.producers > 0);
            worst = worst.min(if u > 0.0 { f.stock / (u * n as f64) } else { f64::INFINITY });
            v.components.push(ComponentVerdict {
                component: c,
                founders: n,
                food: f,
                habitable,
            });
        }
        if v.stranded > 0 {
            worst = 0.0;
        }
        v.accepted = v.placed >= v.wanted
            && v.stranded == 0
            && v.components.iter().all(|c| c.habitable);
        v.ratio = if v.wanted == 0 {
            f64::INFINITY
        } else if v.placed == 0 {
            0.0
        } else {
            worst * (v.placed.min(v.wanted) as f64 / v.wanted as f64)
        };
        out.lineages[founder.index()] = v;
    }
    out.accepted = out.lineages.iter().all(|l| l.accepted);
    out
}

/// Register each lineage's own observation-only heuristic as its birth factory: every
/// founder **born** in this layer is driven by one of these unless a caller replaces
/// them. Introduced bodies get theirs from [`introduce_founder`], out of the same
/// recipes.
pub fn install_heuristics(fauna: &mut Fauna) {
    fauna.set_founder_factory(
        Founder::Blind,
        Arc::new(|| -> Box<dyn Controller> { Box::new(BlindForager::new()) }),
    );
    fauna.set_founder_factory(
        Founder::Browser,
        Arc::new(|| -> Box<dyn Controller> { Box::new(BrowserForager::new()) }),
    );
}

/// One founder body on a support face, hungry, with its own heuristic installed. Returns
/// whether the layer accepted it.
fn introduce_founder(
    world: &World,
    fauna: &mut Fauna,
    founder: Founder,
    site: Site,
    heading_rad: f64,
) -> bool {
    if !fauna.apply(
        world,
        FaunaCommand::IntroduceFounderOnFace {
            site,
            founder,
            stores: StartingStores::HUNGRY,
            heading_rad,
        },
    ) {
        return false;
    }
    let id = fauna.view().ledger.births - 1;
    fauna.install_founder_controller(id, founder)
}

/// Headings spread evenly around the circle, one per body. Deterministic, and nothing
/// about the world reaches it: a founder is not aimed at its food.
fn spread_heading(k: usize, of: usize) -> f64 {
    std::f64::consts::TAU * (k as f64) / (of.max(1) as f64)
}

/// The faces a browser founder could actually live on: its adult body fits, the water
/// is wadeable, its mouth reaches a **foliage layer with stock** of a stand its diet
/// accepts — the tick's own band and per-layer scan, not the crown top against the band
/// — and the walk from there reaches a **second** stand.
///
/// The last one is why this returns nothing rather than falling back to bare soil. A
/// founder browses through a 2 m cone with no target search; a body on ground whose only
/// stand is the one it is standing under eats that stand and then has nowhere to go.
#[cfg(test)]
fn browser_faces(view: &VoxelView<'_>, fv: &FloraView<'_>, fauna: &Fauna) -> Vec<Site> {
    let phys = *fauna.config().founder(Founder::Browser);
    let map = RouteMap::for_founder(view, &phys);
    let food = food_on(&map, view, fv, Founder::Browser);
    browser_faces_on(&map, &food, view, fv)
}

/// [`browser_faces`] on a route map and its food already built.
fn browser_faces_on(
    map: &RouteMap,
    food: &BTreeMap<usize, ComponentFood>,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
) -> Vec<Site> {
    let diet = cubarium_voxel_fauna::Diet::of(Founder::Browser);
    let mut out: Vec<Site> = Vec::new();
    for stand in fv.stands.iter().filter(|s| s.foliage > 0.0) {
        for layer in fv.layers(stand) {
            for i in map.faces_reaching_layer(fv, view, stand, &layer, diet) {
                // The walk: one rule, `walkable_components` over every face this body
                // could stand on, with the lineage's climb
                // (`design/handoffs/voxel-founder-step-2026-09-22.md`). A component
                // holding only one stand is a dead end.
                if food
                    .get(&map.components[i])
                    .is_some_and(|f| f.stands >= 2)
                {
                    out.push(map.faces[i]);
                }
            }
        }
    }
    out.sort_unstable_by_key(|s| (s.y, s.x, s.z));
    out.dedup();
    out
}

/// Every support face in the world, at any height, sorted low to high: the ground, the
/// shelves and the hollow floors alike. A strided sample of it spreads over the whole
/// strip rather than one end of it.
fn support_sites(world: &World) -> Vec<Site> {
    let view = world.view();
    let c = world.config();
    let mut out: Vec<Site> = Vec::new();
    for z in 0..c.depth {
        for x in 0..c.width as i64 {
            for y in view.supports_in_column(x, z) {
                out.push(Site {
                    x: x.rem_euclid(i64::from(c.width)) as u32,
                    y,
                    z,
                });
            }
        }
    }
    out.sort_by_key(|s| (s.y, s.x, s.z));
    out
}

/// Whether a founder of this species could **establish and then keep standing** here:
/// the flora layer's own establishment gates on the terrain as it is, and its
/// adult-upkeep check on the light. Nothing about the rule lives in the host.
fn suitable(view: &VoxelView<'_>, site: Site, sc: &SpeciesConfig, sky: f64) -> bool {
    establishment_gates_with_sky(view, site, sc, sky, 0.0, 0.0).passes()
        && adult_light_cover(sc, sky) >= 1.0
}

/// The ring's **lake** as a per-cell mask: the cells of [`cubarium_voxel::hydrate::lake`],
/// the lowest standing water the sky sees, read off the world as it stands.
fn lake_cells(world: &World) -> Vec<bool> {
    let mut mask = vec![false; world.config().cells()];
    for i in cubarium_voxel::hydrate::lake(world).cells {
        mask[i] = true;
    }
    mask
}

/// Whether `site` is a **lake margin**: lake water stands on its face, or on the highest
/// face at or below it in one of its four neighbouring columns — the flora layer's
/// standing-water-beside rule with the water restricted to the lake.
///
/// Why the lake and nothing else (seeder-sites, 2026-09-23). A siphonreed needs settled
/// standing water beside it for life, and after the pre-roll the rings hold two kinds:
/// the lake, which the outlet's datum, the water table and the river keep, and puddles
/// the hydrated sheet and the opening shower leave on the loam above it, which drain into
/// the soil. Measured with no plants on `default` seeds 1–4 and `small` 1: of the reed's
/// eligible faces, the lake's margins kept their water 99–100 % at 2 h (s4 63 % at 4 h),
/// while the puddle banks the seeder used to pick went 0.20 → 0.07 m (s1) and
/// 0.16 → 0.05 m (s3) inside the hour and the reeds on them died. A longer settle would
/// find the same thing at a minute or more of startup on the larger worlds; the lake is
/// already a derived reading. The desk terrarium's reeds were all on lake margins
/// already (s1 64/64, s2 57/57), so it is unchanged.
fn lake_margin(view: &VoxelView<'_>, lake: &[bool], site: Site) -> bool {
    let c = view.config;
    let wet = |x: i64, y: u32, z: u32| y + 1 < c.height && lake[c.index(x, y + 1, z)];
    let x = i64::from(site.x);
    if wet(x, site.y, site.z) {
        return true;
    }
    for (dx, dz) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
        let z = i64::from(site.z) + dz;
        if z < 0 || z >= i64::from(c.depth) {
            continue;
        }
        let (nx, nz) = (x + dx, z as u32);
        if let Some(y) = (0..=site.y).rev().find(|&y| view.is_support(nx, y, nz))
            && wet(nx, y, nz)
        {
            return true;
        }
    }
    false
}

/// Which producer *prefers* a support face: the wetland one where water stands on it, the
/// rock cushion on bare rock, then one of the soil species by height band. **Composition,
/// not suitability** — [`suitable`] has already said the species may live here, and this
/// only decides who gets first refusal, so the picture comes out banded by landform.
fn prefers(view: &VoxelView<'_>, site: Site, highest_y: u32) -> Species {
    let x = i64::from(site.x);
    if view.water_depth_m(x, site.y, site.z) > 0.0 {
        return Species::Umbrellafrond;
    }
    if view.material_at(x, site.y, site.z) == Material::Rock {
        return Species::Stonecushion;
    }
    let rel = f64::from(site.y) / f64::from(highest_y.max(1));
    if rel > 0.62 {
        Species::Bloomcrown
    } else if rel > 0.38 {
        Species::Springturf
    } else {
        Species::Velvetpad
    }
}

/// Take the sky away from every site a new crown stands over. See [`CANOPY_SHADE`].
fn shade_under(
    sites: &[Site],
    sky: &mut [f64],
    at: Site,
    sc: &SpeciesConfig,
    wood: f64,
    width: u32,
    voxel_m: f64,
) {
    let radius = sc.crown_radius(wood, voxel_m).max(0.0);
    let top = at.y + sc.crown_voxels(wood, voxel_m);
    for (i, s) in sites.iter().enumerate() {
        if s.y > top {
            continue;
        }
        let dx = f64::from(wrapped(s.x, at.x, width));
        let dz = f64::from(s.z.abs_diff(at.z));
        if dx * dx + dz * dz <= radius * radius {
            sky[i] *= 1.0 - CANOPY_SHADE;
        }
    }
}

/// Where `site` sits in a list built by [`support_sites`].
fn index_of(sites: &[Site], site: Site) -> Option<usize> {
    sites
        .iter()
        .position(|s| s.x == site.x && s.y == site.y && s.z == site.z)
}

/// The shorter of the two ways round the ring, in voxels.
fn wrapped(a: u32, b: u32, width: u32) -> u32 {
    let d = a.abs_diff(b);
    d.min(width.saturating_sub(d))
}

/// A fixed stride sample of `want` items out of `pool`: the whole pool spread over, never
/// the same end of it twice. Returns fewer when the pool is smaller than `want`.
fn strided<T: Copy>(pool: &[T], want: usize) -> Vec<T> {
    if pool.is_empty() || want == 0 {
        return Vec::new();
    }
    let stride = (pool.len() / want).max(1);
    pool.iter().step_by(stride).take(want).copied().collect()
}

/// Patch centres at least `spacing` voxels apart, each grown into a colony over the
/// **connected** suitable faces around it: neighbouring columns whose support faces are
/// within `rise` voxels of each other, which is the same neighbourhood the generator's
/// walkability check uses. Gaps between colonies are what the spacing is for, and a
/// colony never leaves the habitat the species was already allowed.
fn colonies(
    pool: &[Site],
    want: usize,
    patch_size: usize,
    spacing: u32,
    width: u32,
    rise: u32,
) -> Vec<Site> {
    let count = want.min(pool.len());
    if count == 0 {
        return Vec::new();
    }
    let mut out: Vec<Site> = Vec::with_capacity(count);
    let mut anchors: Vec<Site> = Vec::new();
    let mut taken = vec![false; pool.len()];
    for (i, &anchor) in pool.iter().enumerate() {
        if out.len() >= count {
            break;
        }
        if taken[i]
            || anchors
                .iter()
                .any(|a| wrapped(a.x, anchor.x, width) + a.z.abs_diff(anchor.z) < spacing)
        {
            continue;
        }
        anchors.push(anchor);
        // Grow the colony: a breadth-first walk over the pool's own adjacency.
        let mut queue = vec![i];
        let mut grown = 0;
        taken[i] = true;
        while let Some(k) = queue.pop() {
            out.push(pool[k]);
            grown += 1;
            if grown >= patch_size || out.len() >= count {
                break;
            }
            let here = pool[k];
            for (j, &other) in pool.iter().enumerate() {
                if taken[j] {
                    continue;
                }
                let dx = wrapped(other.x, here.x, width);
                let dz = other.z.abs_diff(here.z);
                if dx + dz == 1 && other.y.abs_diff(here.y) <= rise {
                    taken[j] = true;
                    queue.push(j);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_voxel::Config;

    /// A colony grows over the connected suitable faces around its own anchor, wrapping
    /// the seam, and a second anchor keeps its distance.
    #[test]
    fn a_colony_grows_over_connected_ground_and_anchors_keep_apart() {
        let at = |x, y| Site { x, y, z: 1 };
        // Four faces in a row across the seam (15, 0, 1) plus one far away and high.
        let pool = [at(0, 2), at(1, 2), at(8, 9), at(15, 2)];
        let patch = colonies(&pool, 3, 3, 4, 16, 2);
        assert_eq!(
            patch.len(),
            3,
            "one colony of three, grown across the seam: {patch:?}"
        );
        assert!(
            patch.contains(&at(15, 2)),
            "the colony wrapped the ring: {patch:?}"
        );
        assert!(
            !patch.contains(&at(8, 9)),
            "it did not jump the gap: {patch:?}"
        );
        // Room for two anchors once the spacing allows it.
        let two = colonies(&pool, 2, 1, 4, 16, 2);
        assert_eq!(two.len(), 2);
        assert!(two.contains(&at(8, 9)), "the far face is its own patch");
    }

    /// A roofed shelf is habitat. Every support face is a founder site — the shelf's own
    /// floor and the ground under it both — and a slab with another slab on top of it is
    /// not a face at all, so nothing is offered a place with no room over it.
    #[test]
    fn a_roofed_shelf_and_the_ground_under_it_are_both_founder_sites() {
        let cfg = Config {
            width: 8,
            height: 12,
            depth: 2,
            ..Config::default()
        };
        let mut world = World::empty(cfg.clone());
        for z in 0..cfg.depth {
            for x in 0..cfg.width as i64 {
                for y in 1..=2 {
                    world.apply(cubarium_voxel::Command::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
            // A shelf over open ground at x = 4, and a double slab at x = 6.
            world.apply(cubarium_voxel::Command::SetMaterial {
                x: 4,
                y: 5,
                z,
                material: Material::Rock,
            });
            for y in 5..=7 {
                world.apply(cubarium_voxel::Command::SetMaterial {
                    x: 6,
                    y,
                    z,
                    material: Material::Rock,
                });
            }
        }
        let sites = support_sites(&world);
        let has = |x: u32, y: u32| sites.contains(&Site { x, y, z: 0 });
        assert!(has(4, 5), "the shelf's own floor is a site");
        assert!(has(4, 2), "so is the ground in its shade");
        assert!(has(6, 7), "the top of the double slab is open sky");
        assert!(!has(6, 5), "a slab with rock on it is not a face");
        assert!(!has(6, 6), "nor is the one above it");
    }

    /// Suitability is the flora layer's own gates plus its adult-upkeep check. Shade
    /// separates the two producers it is supposed to separate: the shade-tolerant
    /// wetland frond keeps a hollow floor the sun-demanding bloomcrown cannot hold.
    #[test]
    fn shade_keeps_the_bloomcrown_out_of_a_hollow_and_lets_the_frond_in() {
        let (world, flora, _, _) = seeded(crate::voxel::scene::authored(config()));
        let frond = flora.config().species(Species::Umbrellafrond).clone();
        let crown = flora.config().species(Species::Bloomcrown).clone();
        let view = world.view();
        let bright = support_sites(&world)
            .into_iter()
            .find(|s| suitable(&view, *s, &frond, 1.0))
            .expect("the authored scene has moist ground the frond can live on");
        assert!(
            suitable(&view, bright, &frond, 0.15),
            "the frond earns its keep in shade"
        );
        assert!(
            !suitable(&view, bright, &crown, 0.15),
            "the bloomcrown's own light gate shuts in the same shade"
        );
        // And a saprotroph is refused everywhere it has no substrate, which is why the
        // seeder books the log before it offers the seed.
        let cap = flora.config().species(Species::Glowcap).clone();
        assert!(
            support_sites(&world)
                .into_iter()
                .all(|s| !suitable(&view, s, &cap, 1.0)),
            "a glowcap on bare ground has nothing to eat"
        );
    }

    /// No foodless fallback. A habitat with one lone patch gives a browser nowhere to go
    /// when it is bare, so nothing is placed and `Seeded` says how many were owed.
    #[test]
    fn a_browser_with_one_patch_is_a_shortfall_not_a_placement() {
        let cfg = Config {
            width: 24,
            height: 16,
            depth: 4,
            ..Config::default()
        };
        let mut world = World::empty(cfg.clone());
        for z in 0..cfg.depth {
            for x in 0..cfg.width as i64 {
                for y in 1..=2 {
                    world.apply(cubarium_voxel::Command::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        let mut flora = Flora::new(Default::default());
        let mut fauna = Fauna::new(Default::default());
        let sc = flora.config().species(Species::Springturf).clone();
        assert!(flora.apply(
            &mut world,
            FloraCommand::Seed {
                x: 5,
                z: 1,
                species: Species::Springturf,
                wood: 0.8 * sc.wood_max,
            },
        ));
        assert!(
            browser_faces(&world.view(), &flora.view(), &fauna).is_empty(),
            "one patch is not a route to a second one"
        );
        let out = seed_with_founder_counts(&mut world, &mut flora, &mut fauna, [0, 4]);
        assert_eq!(out.founders[Founder::Browser.index()], 0);
        assert_eq!(
            out.shortfall[Founder::Browser.index()],
            4,
            "the shortfall is reported, not filled with bare soil: {out:?}"
        );
    }

    fn config() -> Config {
        Config {
            width: 64,
            height: 24,
            depth: 6,
            ..Config::default()
        }
    }

    /// A flat soil plain at 0.25 m: soil in `y = 1..=2`, so every column's support face
    /// is `y = 2`, with room for either body over it.
    fn plain(width: u32, depth: u32) -> World {
        plain_with(Config {
            width,
            height: 12,
            depth,
            ..Config::default()
        })
    }

    fn plain_with(cfg: Config) -> World {
        let mut world = World::empty(cfg.clone());
        for z in 0..cfg.depth {
            for x in 0..cfg.width as i64 {
                for y in 1..=2 {
                    world.apply(cubarium_voxel::Command::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        world
    }

    fn plant(flora: &mut Flora, world: &World, x: u32, z: u32, species: Species, frac: f64) {
        let wood = frac * flora.config().species(species).wood_max;
        assert!(
            flora.apply(
                world,
                FloraCommand::SeedOnFace {
                    site: Site { x, y: 2, z },
                    species,
                    wood,
                },
            ),
            "{} on ({x}, 2, {z})",
            species.name()
        );
    }

    fn introduce(fauna: &mut Fauna, world: &World, founder: Founder, site: Site) {
        assert!(fauna.apply(
            world,
            FaunaCommand::IntroduceFounderOnFace {
                site,
                founder,
                stores: StartingStores::HUNGRY,
                heading_rad: 0.0,
            },
        ));
    }

    /// **The checked face is the seeded face** (item 2). Under a roofed shelf the column
    /// command resolves the roof; the face command lands on the floor it was given, for
    /// a stand and for a founder body alike.
    #[test]
    fn a_stand_and_a_founder_seeded_under_a_shelf_land_on_the_checked_face() {
        let cfg = Config {
            width: 8,
            height: 12,
            depth: 2,
            ..Config::default()
        };
        let mut world = plain_with(cfg.clone());
        for z in 0..cfg.depth {
            world.apply(cubarium_voxel::Command::SetMaterial {
                x: 4,
                y: 5,
                z,
                material: Material::Rock,
            });
        }
        let under = |z| Site { x: 4, y: 2, z };
        assert!(support_sites(&world).contains(&under(0)), "the floor is a face");

        let mut flora = Flora::new(Default::default());
        let wood = 0.8 * flora.config().species(Species::Springturf).wood_max;
        assert!(flora.apply(
            &world,
            FloraCommand::SeedOnFace {
                site: under(0),
                species: Species::Springturf,
                wood,
            },
        ));
        assert_eq!(flora.view().stands[0].site, under(0), "planted on the floor");
        // The column command, for contrast, plants the roof.
        assert!(flora.apply(
            &world,
            FloraCommand::Seed {
                x: 4,
                z: 1,
                species: Species::Springturf,
                wood,
            },
        ));
        assert!(
            flora.view().stands.iter().any(|s| s.site == Site { x: 4, y: 5, z: 1 }),
            "the column command still resolves the skyline"
        );
        // A face that is not a support face is refused.
        assert!(!flora.apply(
            &world,
            FloraCommand::SeedOnFace {
                site: Site { x: 4, y: 3, z: 0 },
                species: Species::Springturf,
                wood,
            },
        ));

        let mut fauna = Fauna::new(Default::default());
        introduce(&mut fauna, &world, Founder::Browser, under(1));
        let a = fauna.view().animals[0];
        assert_eq!(a.site, under(1), "the body stands on the floor, not the roof");
        assert!(!fauna.apply(
            &world,
            FaunaCommand::IntroduceFounderOnFace {
                site: Site { x: 4, y: 4, z: 1 },
                founder: Founder::Browser,
                stores: StartingStores::HUNGRY,
                heading_rad: 0.0,
            },
        ));
    }

    /// **Browser faces by layer** (item 3). An adult bloomcrown's crown is out of the
    /// browser's band and its rosette is in it: the faces round its stem are admitted.
    /// Crop the rosette to its grazing floor and the only in-band layer holds no edible
    /// stock, so they are refused.
    #[test]
    fn a_browser_face_is_admitted_by_a_stocked_layer_in_its_band_not_the_crown_top() {
        let world = plain(24, 6);
        let mut flora = Flora::new(cubarium_voxel_flora::FloraConfig::for_voxel_size(0.25));
        let fauna = Fauna::new(Default::default());
        // Two stands, so the "a second patch" rule is not what is being tested.
        plant(&mut flora, &world, 6, 3, Species::Bloomcrown, 0.85);
        plant(&mut flora, &world, 16, 3, Species::Bloomcrown, 0.85);
        let body = fauna.config().founder(Founder::Browser).adult_body();
        let band = body.mouth_layers(2, 0.25);
        {
            let fv = flora.view();
            let stand = &fv.stands[0];
            let layers: Vec<_> = fv.layers(stand).collect();
            assert_eq!(layers.len(), 2, "an adult: rosette and crown");
            assert!(band.contains(&layers[0].cell), "the rosette is in the band");
            assert!(layers[0].stock > 0.0, "and holds stock");
            assert!(
                !band.contains(&cubarium_voxel_fauna::crown_layer(&fv, stand)),
                "the crown top is out of it"
            );
        }
        let faces = browser_faces(&world.view(), &flora.view(), &fauna);
        assert!(!faces.is_empty(), "the rosette admits the faces round the stem");
        // How many columns from its own a mouth reaches: the probe furthest from the
        // face's centre is `2 r + reach` ahead of it (0.5625 m, 2.25 cells, for package
        // L's 0.75 m browser; the one-column rosette is at most two columns away).
        let reach_cells = ((2.0 * body.footprint_radius() + body.mouth_reach_m) / 0.25 + 0.5)
            .floor() as u32;
        assert_eq!(reach_cells, 2);
        for f in &faces {
            let near = [6u32, 16]
                .iter()
                .any(|&x| wrapped(f.x, x, 24) <= reach_cells);
            assert!(
                near && f.z.abs_diff(3) <= reach_cells,
                "{f:?} is not within a mouth's reach of a stem"
            );
        }

        let sites: Vec<Site> = flora.view().stands.iter().map(|s| s.site).collect();
        for site in sites {
            assert!(flora.take_foliage_in_layers(site, 1e9, &band).is_some());
        }
        for stand in flora.view().stands.iter() {
            let rosette = flora.view().layers(stand).next().unwrap();
            assert!(rosette.edible() <= 0.0, "cropped to its grazing floor");
        }
        assert!(
            browser_faces(&world.view(), &flora.view(), &fauna).is_empty(),
            "an in-band layer with no stock admits nothing"
        );
    }

    /// **Two food patches joined only by bare walkable ground are one component** (item
    /// 4): the route runs over ordinary ground, not from feeding face to feeding face.
    #[test]
    fn two_patches_joined_by_bare_ground_are_one_component() {
        let world = plain(32, 4);
        let mut flora = Flora::new(cubarium_voxel_flora::FloraConfig::for_voxel_size(0.25));
        let fauna = Fauna::new(Default::default());
        plant(&mut flora, &world, 3, 1, Species::Springturf, 0.8);
        plant(&mut flora, &world, 20, 2, Species::Springturf, 0.8);
        let phys = *fauna.config().founder(Founder::Browser);
        let (map, food) = lineage_food(&world.view(), &flora.view(), &phys, Founder::Browser);
        let a = map.component_of(Site { x: 3, y: 2, z: 1 }).expect("standable");
        let b = map.component_of(Site { x: 20, y: 2, z: 2 }).expect("standable");
        assert_eq!(a, b, "one walk joins them");
        let with_food: Vec<_> = food.iter().filter(|(_, f)| f.stock > 0.0).collect();
        assert_eq!(with_food.len(), 1, "{food:?}");
        assert_eq!(with_food[0].1.stands, 2, "both patches are in it: {food:?}");
    }

    /// **The acceptance bar** (item 4, decisions §8): a browser component passes with
    /// founders up to `stock / (N · upkeep)` and fails with one more; a shredder
    /// component with starter litter and no living producer rooted in it fails until a
    /// litter-shedding stand grows there.
    #[test]
    fn acceptance_needs_n_founder_hours_of_stock_and_a_living_producer() {
        let world = plain(24, 6);
        let mut flora = Flora::new(cubarium_voxel_flora::FloraConfig::for_voxel_size(0.25));
        plant(&mut flora, &world, 6, 3, Species::Bloomcrown, 0.85);
        plant(&mut flora, &world, 16, 3, Species::Bloomcrown, 0.85);
        let probe = Fauna::new(Default::default());
        let phys = *probe.config().founder(Founder::Browser);
        let u = upkeep_per_hour(&phys);
        assert!(
            (u - phys.core.maintenance_per_s * phys.core.body_max * 3600.0).abs() < 1e-12,
            "upkeep is adult basal maintenance per hour"
        );
        let (_, food) = lineage_food(&world.view(), &flora.view(), &phys, Founder::Browser);
        let stock: f64 = food.values().map(|f| f.stock).sum();
        let fits = (stock / (ACCEPT_FOUNDER_HOURS * u)).floor() as usize;
        assert!(fits >= 1, "the fixture feeds at least one founder: {stock} vs {u}");

        let with = |n: usize| {
            let mut fauna = Fauna::new(Default::default());
            for _ in 0..n {
                introduce(&mut fauna, &world, Founder::Browser, Site { x: 6, y: 2, z: 3 });
            }
            accept(&world, &flora, &fauna, [0, n])
        };
        let at = with(fits);
        let v = &at.lineages[Founder::Browser.index()];
        assert!(v.accepted && at.accepted, "{fits} founders on {stock}: {at:?}");
        assert_eq!(v.components.len(), 1);
        assert!(v.components[0].habitable);
        let over = with(fits + 1);
        assert!(
            !over.lineages[Founder::Browser.index()].accepted && !over.accepted,
            "{} founders on {stock}: {over:?}",
            fits + 1
        );

        // The shredder: litter on the ground and nothing alive to renew it.
        let mut bare = Flora::new(cubarium_voxel_flora::FloraConfig::for_voxel_size(0.25));
        let at_site = Site { x: 4, y: 2, z: 2 };
        assert!(bare.deposit(
            at_site,
            Deposit {
                kind: DepositKind::Litter,
                organic: LITTER_ORGANIC,
                mineral: LITTER_ORGANIC * LITTER_MINERAL_FRACTION,
                energy: LITTER_ORGANIC * LITTER_ENERGY_DENSITY,
            },
        ));
        let mut fauna = Fauna::new(Default::default());
        introduce(&mut fauna, &world, Founder::Blind, at_site);
        let got = accept(&world, &bare, &fauna, [1, 0]);
        let v = &got.lineages[Founder::Blind.index()];
        assert!(v.components[0].food.stock >= LITTER_ORGANIC - 1e-12, "{got:?}");
        assert!(!v.accepted && !got.accepted, "litter with no producer: {got:?}");
        plant(&mut bare, &world, 12, 2, Species::Springturf, 0.8);
        let got = accept(&world, &bare, &fauna, [1, 0]);
        assert!(
            got.lineages[Founder::Blind.index()].accepted && got.accepted,
            "a litter-shedding stand rooted in the component: {got:?}"
        );
    }

    /// A closed, scheduled cycle over a metre of soil, with a shower that falls hard for
    /// a second and a half, stands a few centimetres deep on the ground, and soaks away.
    fn showery_plain() -> World {
        let cfg = Config {
            width: 16,
            height: 12,
            depth: 4,
            closed_water_budget: true,
            initial_atmosphere_m3: 0.5,
            rain_m_per_s: 0.04,
            shower_volume_m3: 0.25,
            shower_trigger_fraction: 0.01,
            shower_interval_min_s: 300.0,
            shower_interval_max_s: 900.0,
            ..Config::default()
        };
        let mut world = World::empty(cfg.clone());
        for z in 0..cfg.depth {
            for x in 0..cfg.width as i64 {
                for y in 1..=4 {
                    world.apply(cubarium_voxel::Command::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        let water = cubarium_voxel::Water {
            inventory_m: 0.2,
            aquifer_head_m: 0.3,
            ..cubarium_voxel::Water::AUTHORED
        };
        cubarium_voxel::hydrate(&mut world, &water);
        world
    }

    /// **The pre-roll** (item 1). The world opens with its shower: by the time anything
    /// is planted it has fallen and ended, and the next one is drawn. A face that stood
    /// under more than a species' drown depth **during** the shower is not offered to
    /// that species, even though it is dry again when the founders are placed.
    #[test]
    fn the_world_opens_with_a_shower_and_a_face_it_flooded_is_not_offered() {
        let mut world = showery_plain();
        assert!(world.next_shower_tick() > 1000, "the drawn gap is minutes away");
        let pre = pre_roll(&mut world, true);
        assert!(pre.report.opening_shower, "{:?}", pre.report);
        assert_eq!(world.view().ledger.showers, 1, "exactly the opening shower fell");
        assert!(world.shower_left_m3() <= 0.0, "and it ended");
        assert!(world.next_shower_tick() > world.tick(), "the next one is drawn");

        // The plain stood under water and is dry again.
        let view = world.view();
        let dried = |i: usize| {
            let s = pre.sites[i];
            view.water_depth_m(i64::from(s.x), s.y, s.z) <= 0.0
        };
        assert!(
            (0..pre.sites.len()).all(|i| pre.wettest[i] > 0.02 && dried(i)),
            "the shower flooded the plain and it drained: {:?}",
            &pre.wettest[..4]
        );

        // The drown screen is under test, not the establishment water gate: on loam the
        // calm plain's metre of soil sits below every species' establishment water, so
        // that gate is opened for both arms.
        let mut flora_cfg = cubarium_voxel_flora::FloraConfig::for_voxel_size(0.25);
        for species in Species::ALL {
            flora_cfg.species_mut(species).establish_water_min = 0.0;
        }
        let mut flora = Flora::new(flora_cfg.clone());
        let mut fauna = Fauna::new(Default::default());
        let seeded = seed_pre_rolled(&mut world, &mut flora, &mut fauna, &pre, [0, 0]);
        assert_eq!(seeded.pre_roll, pre.report);
        let drowned_on = |species: Species, site: Site| {
            let i = index_of(&pre.sites, site).expect("a support face");
            pre.wettest[i] > flora_cfg.species(species).drown_depth_m
        };
        for stand in flora.view().stands.iter() {
            assert!(
                !drowned_on(stand.species, stand.site),
                "{} planted where the shower stood deeper than it can",
                stand.species.name()
            );
        }
        // The same world without the opening shower plants a species on faces the shower
        // would have drowned it on: the shower is what took them away.
        let mut calm = showery_plain();
        let calm_pre = pre_roll(&mut calm, false);
        let mut calm_flora = Flora::new(flora_cfg.clone());
        let mut calm_fauna = Fauna::new(Default::default());
        seed_pre_rolled(&mut calm, &mut calm_flora, &mut calm_fauna, &calm_pre, [0, 0]);
        assert!(
            calm_flora
                .view()
                .stands
                .iter()
                .any(|s| drowned_on(s.species, s.site)),
            "without the shower something is planted on a face it would have drowned on"
        );
    }

    fn seeded(world: World) -> (World, Flora, Fauna, Seeded) {
        let mut world = world;
        let mut flora = Flora::new(Default::default());
        let mut fauna = Fauna::new(Default::default());
        let summary = seed(&mut world, &mut flora, &mut fauna);
        (world, flora, fauna, summary)
    }

    /// The one thing the launch needs: a populated world, from the same world bytes twice.
    #[test]
    fn the_example_habitat_is_populated_and_deterministic() {
        let a = seeded(crate::voxel::scene::authored(config())).3;
        let b = seeded(crate::voxel::scene::authored(config())).3;
        assert_eq!(a, b, "the same world seeds the same habitat");
        assert!(a.stands >= 10, "a populated picture, not a specimen: {a:?}");
        assert!(a.logs > 0, "the glowcap grove has its wood: {a:?}");
        assert!(a.animals() > 0, "something moves: {a:?}");
        for founder in Founder::ALL {
            assert!(
                a.founders[founder.index()] > 0,
                "the habitat introduces {}: {a:?}",
                founder.name()
            );
        }
        assert!(
            a.litter_tiles >= a.founders[Founder::Blind.index()],
            "every littershredder got its starter tile: {a:?}"
        );
    }

    /// **Both founder lineages eat in the seeded habitat.** The bodies are the seeder's
    /// own — hungry, on the faces it chose, driven by the heuristics it installed — and
    /// they run through the live schedule with the live litter field, which is what the
    /// ambient run does. The evidence for a bite is the founder's own `Self`
    /// `assimilated_intake` channel: the prior interval's assimilated organic matter,
    /// read out of the packet the controller was handed. Nothing fixture-side reaches
    /// it, and nothing here reads a site or a stock.
    #[test]
    fn both_founder_kinds_take_a_bite_in_the_first_ten_seconds() {
        use cubarium_voxel_fauna::{Response, Senses};
        use cubarium_voxel_sim::{Sim, SimConfig};
        use std::sync::{Arc, Mutex};

        /// Passes the packet through untouched and remembers the largest intake channel
        /// it ever carried.
        struct Watched {
            inner: Box<dyn Controller>,
            intake: usize,
            best: Arc<Mutex<f64>>,
        }
        impl Controller for Watched {
            fn drive(&mut self, o: &[f64]) -> Response {
                let mut best = self.best.lock().expect("the sink");
                *best = best.max(o[self.intake]);
                drop(best);
                self.inner.drive(o)
            }
            fn reset(&mut self) {
                self.inner.reset();
            }
        }

        let (world, flora, mut fauna, seeded) = seeded(crate::voxel::scene::authored(config()));
        // One watcher per founder kind, wrapped around the controller the seeder
        // installed, so the heuristic under test is the one the habitat ships.
        let best: [Arc<Mutex<f64>>; Founder::COUNT] = std::array::from_fn(|_| Arc::default());
        let bodies: Vec<(u64, Founder)> = fauna
            .view()
            .animals
            .iter()
            .filter_map(|a| a.founder.map(|f| (a.id, f)))
            .collect();
        assert_eq!(bodies.len(), seeded.animals(), "every animal is a founder");
        for (id, founder) in bodies {
            let intake = founder
                .manifest()
                .modules
                .iter()
                .find(|m| m.name == "Self")
                .expect("every schema opens with Self")
                .offset
                + 4;
            let inner = fauna.take_controller(id).expect("the seeder installed one");
            assert!(fauna.set_controller(
                id,
                Box::new(Watched {
                    inner,
                    intake,
                    best: Arc::clone(&best[founder.index()]),
                }),
            ));
        }

        let mut senses = Senses::new();
        senses.settle(&world.view(), &flora.view());
        let mut sim = Sim::new(world, flora, fauna, SimConfig { threads: 1 }, Some(senses));
        for _ in 0..200 {
            sim.step();
        }
        for founder in Founder::ALL {
            let seen = *best[founder.index()].lock().expect("the sink");
            assert!(
                seen > 0.0,
                "no {} reported any assimilated intake in 200 ticks",
                founder.name()
            );
        }
    }

    /// A few coupled ticks, the fast-iteration bar: the seeded founders stand, the layers
    /// step, and the two ledgers stay closed. Not a study, and deliberately not long.
    ///
    /// Run on the hand-built fixture **and** on the shipped `default` landscape, which is
    /// what `cubarium voxel` with no TOML now generates: a habitat whose founders came off
    /// support faces and hollow floors has to step as cleanly as one off a skyline.
    #[test]
    fn the_seeded_habitat_steps_with_closed_ledgers() {
        steps_with_closed_ledgers(crate::voxel::scene::authored(config()));
    }

    #[test]
    fn the_staged_default_habitat_steps_with_closed_ledgers() {
        // The `default` recipe on a short ring: benches, grottos, a water inventory and
        // the closed cycle, at a sixteenth of the shipped ring's settle cost. Which ring
        // the ambient run builds is `the_config_defaults_are_the_documented_ones`'s
        // business; this one asks whether a habitat seeded off support faces steps with
        // its books closed.
        //
        // The opening shower is shortened to a few seconds so the pre-roll fits; the
        // ledgers are what is being asked, through the shower, the drain and the seeding.
        let preset = cubarium_voxel::Preset::find("default").unwrap();
        let mut recipe = preset.recipe;
        recipe.water.shower_volume_m3 = 0.01;
        let world = World::new(Config {
            seed: 1,
            width: 48,
            landform: cubarium_voxel::Landform::Staged(recipe),
            ..preset.config()
        });
        assert!(world.config().shower_interval_max_s > 0.0, "a scheduled cycle");
        steps_with_closed_ledgers(world);
    }

    fn steps_with_closed_ledgers(world: World) {
        let (mut world, mut flora, mut fauna, seeded) = seeded(world);
        assert!(seeded.stands > 0, "a habitat with plants in it: {seeded:?}");
        if world.config().shower_interval_max_s > 0.0 {
            assert!(seeded.pre_roll.opening_shower, "{:?}", seeded.pre_roll);
        }
        let water = world.view().total_residual();
        let scale = world.view().total_water_m3().max(1.0);
        assert!(
            water.abs() < 1e-9 * scale,
            "water residual {water:e} through the pre-roll and the seeding"
        );
        let before: Vec<(u64, Species)> = flora
            .view()
            .stands
            .iter()
            .map(|s| (s.id, s.species))
            .collect();
        for _ in 0..40 {
            world.step();
            flora.step(&mut world);
            fauna.step(&world, &mut flora);
        }
        let fv = flora.view();
        let av = fauna.view();
        let now: std::collections::BTreeSet<u64> = fv.stands.iter().map(|s| s.id).collect();
        let died: Vec<(u64, &str)> = before
            .iter()
            .filter(|(id, _)| !now.contains(id))
            .map(|(id, sp)| (*id, sp.name()))
            .collect();
        assert!(
            fv.stands.len() == seeded.stands,
            "seeded founder(s) {died:?} died in the first two settled seconds; stands {} of {}",
            fv.stands.len(),
            seeded.stands
        );
        for (got, expected) in [
            (fv.organic() - fv.ledger.expected_organic(), "organic"),
            (fv.mineral() - fv.ledger.expected_mineral(), "mineral"),
            (fv.energy() - fv.ledger.expected_energy(), "energy"),
        ] {
            assert!(got.abs() < 1e-6, "flora {expected} residual {got:e}");
        }
        for (got, expected) in [
            (av.organic() - av.ledger.expected_organic(), "organic"),
            (av.mineral() - av.ledger.expected_mineral(), "mineral"),
            (av.energy() - av.ledger.expected_energy(), "energy"),
        ] {
            assert!(got.abs() < 1e-9, "fauna {expected} residual {got:e}");
        }
        let water = world.view().total_residual();
        let scale = world.view().total_water_m3().max(1.0);
        assert!(
            water.abs() < 1e-9 * scale,
            "water residual {water:e} after 40 ticks"
        );
    }

    /// **The food check counts edible foliage** (seeder-sites item 2): a browser's
    /// component holds what its mouths could take — each reached layer's
    /// [`StandLayer::edible`], above the grazing floor — and not the refuge under it,
    /// which no bite can reach.
    #[test]
    fn the_browser_food_check_counts_edible_foliage_not_the_refuge() {
        let world = plain(24, 6);
        let mut flora = Flora::new(cubarium_voxel_flora::FloraConfig::for_voxel_size(0.25));
        plant(&mut flora, &world, 6, 3, Species::Bloomcrown, 0.85);
        plant(&mut flora, &world, 16, 3, Species::Bloomcrown, 0.85);
        let fauna = Fauna::new(Default::default());
        let phys = *fauna.config().founder(Founder::Browser);
        let (map, food) = lineage_food(&world.view(), &flora.view(), &phys, Founder::Browser);
        let diet = cubarium_voxel_fauna::Diet::of(Founder::Browser);
        let (fv, view) = (flora.view(), world.view());
        let (mut edible, mut stock) = (0.0, 0.0);
        for stand in fv.stands.iter() {
            for layer in fv.layers(stand) {
                if !map.faces_reaching_layer(&fv, &view, stand, &layer, diet).is_empty() {
                    edible += layer.edible();
                    stock += layer.stock;
                }
            }
        }
        assert!(edible > 0.0 && edible < stock, "the fixture needs a refuge: {edible} of {stock}");
        let counted: f64 = food.values().map(|f| f.stock).sum();
        assert!(
            (counted - edible).abs() < 1e-12,
            "the food check counted {counted}: edible {edible}, stock {stock}"
        );
    }

    /// A 16 × 4 soil block at 0.25 m, faces at `y = 6`, with a **lake** — the lowest
    /// standing water, `x` 2..=4, its floor at `y = 2` and water to the brim — and a
    /// **puddle** well above it, `x` 10..=11, floor at `y = 5` and one cell of water.
    fn lake_and_puddle() -> World {
        let cfg = Config {
            width: 16,
            height: 12,
            depth: 4,
            ..Config::default()
        };
        let mut world = World::empty(cfg.clone());
        for z in 0..cfg.depth {
            for x in 0..cfg.width as i64 {
                let top = match x {
                    2..=4 => 2,
                    10..=11 => 5,
                    _ => 6,
                };
                for y in 1..=top {
                    world.apply(cubarium_voxel::Command::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
                for y in top + 1..=6 {
                    let v = world.config().voxel_volume();
                    world.apply(cubarium_voxel::Command::AddWater {
                        x,
                        y,
                        z,
                        volume_m3: v,
                    });
                }
            }
        }
        world
    }

    /// **Siphonreed sites are lake margins** (seeder-sites item 1): the bank of the lake
    /// is a margin; the bank of a puddle above it — standing water that drains — is not,
    /// though the reed's own standing-water gate reads water beside both.
    #[test]
    fn the_lake_bank_is_a_margin_and_a_puddle_bank_is_not() {
        let world = lake_and_puddle();
        let lake = lake_cells(&world);
        let view = world.view();
        let reed = cubarium_voxel_flora::FloraConfig::for_voxel_size(0.25)
            .species(Species::Siphonreed)
            .clone();
        let lake_bank = Site { x: 5, y: 6, z: 1 };
        let puddle_bank = Site { x: 9, y: 6, z: 1 };
        for bank in [lake_bank, puddle_bank] {
            let g = cubarium_voxel_flora::establishment_gates(&view, bank, &reed);
            assert!(g.standing_ok, "{bank:?} has water beside it: {g:?}");
        }
        assert!(lake_margin(&view, &lake, lake_bank));
        assert!(lake_margin(&view, &lake, Site { x: 3, y: 2, z: 1 }), "the lake floor");
        assert!(!lake_margin(&view, &lake, puddle_bank), "a puddle is not the lake");
        assert!(!lake_margin(&view, &lake, Site { x: 13, y: 6, z: 1 }), "dry ground");
    }
}
