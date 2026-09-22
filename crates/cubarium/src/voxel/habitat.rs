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

use cubarium_voxel::{Material, Settle, VoxelView, World};
use cubarium_voxel_fauna::{
    BlindForager, BrowserForager, Command as FaunaCommand, Controller, Fauna, Founder,
    StartingStores, has_headroom,
};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraView, Site, Species, SpeciesConfig,
    adult_light_cover, establishment_gates_with_sky, highest_support,
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
const PRODUCERS: [(Species, f64, usize, usize); 5] = [
    (Species::Bloomcrown, 0.25, 3, 64),
    (Species::Umbrellafrond, 0.15, 3, 40),
    (Species::Springturf, 0.6, 6, 160),
    (Species::Velvetpad, 0.4, 6, 120),
    (Species::Stonecushion, 0.2, 3, 64),
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

/// What [`seed`] put into the world.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
}

impl Seeded {
    /// Every animal the seeder introduced.
    pub fn animals(&self) -> usize {
        self.founders.iter().sum()
    }
}

/// Seed the example habitat. The world's water is settled first — measured, not assumed —
/// and then every unit enters through the ordinary founder, deposit and introduction
/// inflows the layers already name. Deterministic for a given world.
pub fn seed(world: &mut World, flora: &mut Flora, fauna: &mut Fauna) -> Seeded {
    seed_with_founder_counts(world, flora, fauna, [SHREDDERS, BROWSERS])
}

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
    let settle = world.settle(SETTLE_CAP);
    let mut seeded = Seeded {
        settle,
        ..Seeded::default()
    };

    // Every **support face**, not the skyline: a hollow's floor is a place to live, and
    // since slice 2b the rings have them (caves plan, "Ecology and presentation
    // follow-ups"). Sky visibility is read once per site and then carried, so a crown
    // placed here shades the sites under it for every species chosen after it.
    let sites = support_sites(world);
    let Some(highest_y) = sites.iter().map(|s| s.y).max() else {
        return seeded;
    };
    // **Watch the stream before planting in its way.** `settle` converges on the world's
    // stored volume and its wet-cell count, and under river re-entry both are constant
    // while the water is still *moving*: a spill front can be half way down the cascade
    // when the settle reports itself converged. Founders placed on the faces it is about
    // to reach drown in the first seconds — measured on `default`, an umbrellafrond under
    // 0.99 m one tick after seeding and a velvetpad under 0.19 m twelve ticks later, both
    // bone dry when they were planted. So the world runs on a little longer and every
    // face remembers the deepest water it saw; a species is offered a face only if it
    // could stand in that, not only in what happens to be there now.
    let wettest = watch_the_stream(world, &sites, STREAM_WATCH_TICKS);
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
    let mut taken: Vec<Site> = Vec::new();

    // The five producers. Suitability is the **flora layer's own** establishment gates
    // plus its adult-upkeep check — a seed gate alone does not establish that a founder
    // can keep itself standing — and the terrain-sector proxy only decides which species
    // *prefers* a site it is already allowed to live on.
    for (species, per_m2, patch_size, cap) in PRODUCERS {
        let sc = flora.config().species(species).clone();
        let mut pool: Vec<Site> = Vec::new();
        {
            let view = world.view();
            for (i, site) in sites.iter().enumerate() {
                if taken.contains(site)
                    || wettest[i] > sc.drown_depth_m
                    || !suitable(&view, *site, &sc, sky[i])
                {
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
            let Some(k) = index_of(&sites, site) else {
                continue;
            };
            if sky[k] < sc.establish_light_min || adult_light_cover(&sc, sky[k]) < 1.0 {
                continue;
            }
            let wood = FOUNDER_FRACTIONS[i % FOUNDER_FRACTIONS.len()] * wood_max;
            if flora.apply(
                world,
                FloraCommand::Seed {
                    x: i64::from(site.x),
                    z: site.z,
                    species,
                    wood,
                },
            ) {
                taken.push(site);
                seeded.stands += 1;
                shade_under(&sites, &mut sky, site, &sc, wood, width);
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
            if taken.contains(site) || wettest[i] > sc.drown_depth_m {
                continue;
            }
            // The substrate is the log this seeder is about to lay, so the gate is asked
            // with it in hand; everything else is read off the world as it stands.
            let gates = establishment_gates_with_sky(&view, *site, &sc, sky[i], LOG_ORGANIC, 0.0);
            if gates.passes()
                && view.material_at(i64::from(site.x), site.y, site.z) == Material::Soil
            {
                pool.push(*site);
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
            FloraCommand::Seed {
                x: i64::from(site.x),
                z: site.z,
                species: Species::Glowcap,
                wood,
            },
        ) {
            taken.push(site);
            seeded.stands += 1;
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

    // Littershredders on litter-bearing soil: dry open soil away from the stands, each
    // with its own starter tile of leaf litter laid under it first, so the founder
    // begins on a cue it can smell and a stock it can bite.
    let blind = *fauna.config().founder(Founder::Blind);
    let blind_room = blind.adult_body().headroom_voxels(world.config().voxel_m);
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
            })
            .map(|(_, s)| s)
            .collect()
    };
    let want = founder_counts[Founder::Blind.index()];
    for (k, site) in strided(&pool, want).into_iter().enumerate() {
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
    // body, water under its wade depth, foliage its mouth actually reaches from there,
    // and somewhere else to go when that patch is bare — a walkable route to a second
    // patch. There is no foodless fallback: a lineage with nowhere to live is a
    // shortfall in `Seeded` and nothing is placed (plan §5).
    let mut meadow = browser_faces(&world.view(), &flora.view(), fauna);
    // A body drowns in the stream's way as surely as a plant does.
    let browser_drown = fauna.config().founder(Founder::Browser).core.drown_depth_m;
    meadow.retain(|f| index_of(&sites, *f).is_none_or(|i| wettest[i] <= browser_drown));
    let want = founder_counts[Founder::Browser.index()];
    for (k, site) in strided(&meadow, want).into_iter().enumerate() {
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

    seeded
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
        FaunaCommand::IntroduceFounder {
            x: i64::from(site.x),
            z: site.z,
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

/// The faces a browser founder could actually live on: its body fits, the water is
/// wadeable, a seeded crown is inside its mouth's reach from there, and the walk from
/// there reaches a **second** foliage patch.
///
/// The last one is why this returns nothing rather than falling back to bare soil. A
/// founder browses through a 2 m cone with no target search; a body on ground whose only
/// crown is the one it is standing under eats that crown and then has nowhere to go.
fn browser_faces(view: &VoxelView<'_>, fv: &FloraView<'_>, fauna: &Fauna) -> Vec<Site> {
    let phys = *fauna.config().founder(Founder::Browser);
    let sc = phys.core;
    // The **adult** body: a founder is placed hungry and grows into this one in place,
    // so the seeder must not offer a slot the grown animal will not fit in. On both
    // shipped grids the hungry body asks for the same clearance anyway
    // (`design/handoffs/voxel-body-anchors-2026-09-22.md`).
    let body = phys.adult_body();
    let room = body.headroom_voxels(view.config.voxel_m);
    let width = i64::from(view.config.width);
    let depth = i64::from(view.config.depth);

    // Feeding faces, by the stand whose crown they reach.
    let mut feeding: Vec<(Site, u64)> = Vec::new();
    for stand in fv.stands.iter().filter(|s| s.foliage > 0.0) {
        let sp = fv.config.species(stand.species);
        let crown_y = i64::from(stand.site.y) + i64::from(sp.crown_voxels(stand.wood));
        let radius = sp.crown_radius(stand.wood).max(0.0);
        let span = radius.floor() as i64;
        let r2 = radius * radius;
        for dz in -span..=span {
            for dx in -span..=span {
                if (dx * dx + dz * dz) as f64 > r2 {
                    continue;
                }
                let cx = (i64::from(stand.site.x) + dx).rem_euclid(width);
                let cz = i64::from(stand.site.z) + dz;
                if cz < 0 || cz >= depth {
                    continue;
                }
                let Some(face) = highest_support(view, cx, cz as u32) else {
                    continue;
                };
                // The physical mouth band from that face (decisions §2), the same
                // rule the tick's bite uses.
                if !body
                    .mouth_layers(face.y, view.config.voxel_m)
                    .contains(&crown_y)
                {
                    continue;
                }
                if !has_headroom(view, cx, face.y, face.z, room) {
                    continue;
                }
                if view.water_depth_m(cx, face.y, face.z) > sc.wade_depth_m {
                    continue;
                }
                feeding.push((face, stand.id));
            }
        }
    }
    if feeding.is_empty() {
        return Vec::new();
    }

    // The walk. One rule, one function: `walkable_components` over **every** face this
    // body could stand on, with the lineage's climb
    // (`design/handoffs/voxel-founder-step-2026-09-22.md`). The route between two
    // feeding patches runs over ordinary ground, so the component has to be built on the
    // standable set and the feeding faces read off it — a component holding only one
    // patch's stands is a dead end.
    let climb = cubarium_voxel_fauna::climb_voxels(
        fauna.config().founder(Founder::Browser),
        view.config.voxel_m,
    );
    let walkable = cubarium_voxel_fauna::standable_faces(view, &body, sc.wade_depth_m);
    let component = cubarium_voxel_fauna::walkable_components(&walkable, view.config.width, climb);
    let component = {
        let mut map: std::collections::HashMap<Site, usize> =
            std::collections::HashMap::with_capacity(walkable.len());
        for (f, c) in walkable.iter().zip(component) {
            map.insert(*f, c);
        }
        map
    };
    let mut stands_in: std::collections::BTreeMap<usize, std::collections::BTreeSet<u64>> =
        Default::default();
    for (face, id) in &feeding {
        if let Some(&root) = component.get(face) {
            stands_in.entry(root).or_default().insert(*id);
        }
    }
    let mut out: Vec<Site> = {
        let mut all: Vec<Site> = feeding
            .iter()
            .map(|&(f, _)| f)
            .filter(|f| {
                component
                    .get(f)
                    .and_then(|root| stands_in.get(root))
                    .is_some_and(|s| s.len() >= 2)
            })
            .collect();
        all.sort_unstable_by_key(|s| (s.y, s.x, s.z));
        all.dedup();
        all
    };
    out.sort_by_key(|s| (s.y, s.x, s.z));
    out
}

/// Run the world on and report, per site, the **deepest standing water it saw**.
///
/// A settled world is not a still one when a river runs through it. This is the water a
/// face actually gets, rather than the water it happens to have at the instant the
/// founders are chosen.
fn watch_the_stream(world: &mut World, sites: &[Site], ticks: u32) -> Vec<f64> {
    let mut wettest = vec![0.0f64; sites.len()];
    for _ in 0..ticks {
        world.step();
        let view = world.view();
        for (w, s) in wettest.iter_mut().zip(sites) {
            let d = view.water_depth_m(i64::from(s.x), s.y, s.z);
            if d > *w {
                *w = d;
            }
        }
    }
    wettest
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
) {
    let radius = sc.crown_radius(wood).max(0.0);
    let top = at.y + sc.crown_voxels(wood);
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
        let preset = cubarium_voxel::Preset::find("default").unwrap();
        steps_with_closed_ledgers(World::new(Config {
            seed: 1,
            width: 48,
            ..preset.config()
        }));
    }

    fn steps_with_closed_ledgers(world: World) {
        let (mut world, mut flora, mut fauna, seeded) = seeded(world);
        assert!(seeded.stands > 0, "a habitat with plants in it: {seeded:?}");
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
    }
}
