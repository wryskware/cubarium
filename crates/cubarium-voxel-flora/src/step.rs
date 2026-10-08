//! One tick of the plant layer: the `design/ecology-v1-contract.md` §4 plant model with
//! light from the terrain's own geometry and water from the core's pore store.
//!
//! # Order within a tick
//!
//! 1. **Terrain.** A site whose support face is no longer a support — buried, dug out,
//!    turned to air — loses its stand and its ground, booked as `removed_*_out`.
//! 2. **Sky cache.** Dropped whole when `VoxelView::terrain_version` moved, refilled
//!    lazily per site.
//! 3. **Drowning.** A stand standing in water deeper than its species tolerates dies the
//!    §4.7 death.
//! 4. **Light**, from one snapshot of every stand's pre-tick crown: nothing a stand
//!    grows this tick shades anything this tick.
//! 5. **Water.** Every stand's demand per voxel is collected first; then each voxel
//!    issues exactly **one** `Command::WithdrawPore` for the total and the accepted
//!    volume is split among its demanders proportional to demand. `μ` for income is the
//!    one read taken before any withdrawal: a stand that got less than it asked for does
//!    not get a second read. The root box's **saturated fraction** comes off the same
//!    one read, for step 6's aeration stress. On the overlapped tick
//!    ([`crate::Flora::step_planned`]) the withdrawals are planned against a read copy and
//!    applied at the barrier ([`crate::Flora::settle_drink`]), each bounded by what the
//!    cell still holds after the same tick's water; the shortfall comes off the
//!    observational `water_m3` / `drank_m3` of whoever asked.
//! 5b. **Substrate**, for the saprotrophs only, and on the same shape as the water: every
//!    saprotroph's demand on every dead-wood pool of its **mycelium box** is collected
//!    first, then each pool is drawn on **once** for the total and what it gave up is split
//!    among its demanders proportional to demand. So two fungi on one log share it rather
//!    than the earlier site taking its fill first, exactly as two stands' roots share a
//!    soil voxel. `μ` from step 5 multiplies the demand, so a drying log starves the
//!    fungus. See [`feed`].
//! 6. **Per stand**, in site order: §4.1–4.4 income, maintenance and growth, §4.5
//!    senescence, §4.6 dieback, §4.7 death. These three subphases are purely local once
//!    light and water are in hand, so running them per stand in one pass is the same
//!    arithmetic as three passes over every stand — and chunks of stands run side by side,
//!    each stand's ledger lines folded afterwards in site order, so the ledger adds the
//!    same numbers in the same order (`grow`).
//! 7. **Decomposition** (§5), on the stocks the site held *before* this tick: litter,
//!    dead wood and carrion respire their organic matter out of the system and release
//!    their mineral to the site's pool at the same fraction; each pool's energy leaves as
//!    heat at its own current density.
//!    Then, once per simulated second, **glassfilm** (`film.rs`): growth from light on
//!    the site's mineral, upkeep, death to the site's own litter where dry or too deep,
//!    and spread to less-filmed wet neighbours. Its sky and canopy are read in step 4.
//! 8. **The seed bank** (`seeds.rs`). A bank holds **whole seeds**, and a banked site is
//!    tested only on its own slow, staggered check (every 30 s) and on the tick a shower
//!    ends: a lottery among the species with at least one seed that pass the predicate,
//!    weighted by seeds, and the winner spends one. This runs *after* the deaths of step
//!    6, so a gap opened this tick is filled at its site's next check. The same check
//!    then kills whole seeds by attrition (to litter) and sends a bin past its species'
//!    `seed_max_age_s` to litter whole.
//! 9. **Propagules** (§4.8, `seeds.rs`), from one snapshot of donors and recipients. A
//!    whole package is either a **runner** daughter on a free neighbouring face (the
//!    species' `clonal_share`) or one seed, landed by the species' [`crate::Dispersal`]
//!    mode into the arrival bin whose window covers this tick.
//!
//! The grazing defences relax after step 5 and before step 5b: nothing earlier reads
//! them.
//!
//! **The step in parts** (`PlantStep`): steps 1–3 and the vines' drink wants are the
//! **prep**, which writes the layer; steps 4 and 5's reads and plan are the **reads**,
//! which read the layer through `&Flora` and write only the step (it carries the sky cache
//! and the drink scratch meanwhile); the drink's withdrawal and credits **close** it; the
//! rest is the **finish**. The overlapped schedule runs the reads beside the animals'
//! read side, so on a plant tick the animals sense the stands as the previous step left
//! them, less what this step's prep removed.
//!
//! Dropped from v1 by the brief: fruit (3c), downhill transport of litter (3f) and
//! nutrient diffusion (3g).
//!
//! # Transfers, which happen between ticks
//!
//! A consumer's withdrawals and deposits ([`crate::Flora::take_foliage`] and friends,
//! [`crate::Flora::deposit`]) are **not** phases of this tick: they are applied between
//! ticks, like [`crate::Command`]s. The snapshot in step 7 is taken as the first thing
//! `step` does, so material deposited during the inter-tick before this one is in the
//! snapshot and decomposes in this tick, while anything this tick's own senescence,
//! dieback or death deposits waits for the next — one rule, and the same one for a plant
//! and for a corpse. A withdrawal has no phase at all: it lowers the stock it reads, and
//! the next tick's income and decomposition simply see less of it.
//!
//! **What waits is the organic throughput, and not every currency**. The
//! snapshot stores organic *amounts*, and [`decompose_pool`] then takes the mineral and the
//! energy at the pool's **current** density, so material parcels are not age-isolated:
//! one old unit holding no mineral plus one unit shed inside the tick holding one, at a
//! decomposition step of half the old stock, releases `1.0 · 0.5/2.0` = **0.25 of mineral
//! immediately**. That is the inherited well-mixed-pool rule, it conserves every currency
//! exactly, and it is pinned by
//! `tests/glowcap.rs::decomposition_delays_organic_matter_and_not_the_mineral_of_a_mixed_pool`
//! rather than changed.
//!
//! Dead wood keeps its energy, per §5: `e_v` per unit of it goes into
//! `Ground::dead_wood_energy` when the wood diebacks or the stand dies, and leaves as
//! heat only as the wood decomposes, at the stock's current density. A standing dead
//! trunk is energy-dense and unavailable, which is the point of it being its own stock.
//!
//! # A saprotroph's income
//!
//! A [`crate::Trophic::Saprotroph`] stand runs every rule above except one: its income is
//! not light. It withdraws organic matter from the **dead-wood and litter** pools of its
//! mycelium box (step 5b), keeps `substrate_yield` of it as income and respires the rest
//! at once, and the mineral that came with the substrate is netted against what its new
//! tissue actually needs. Both pools, one rate: the substrate a fungus eats is the sum of
//! the two, and the same `substrate_uptake_per_s` and `establish_substrate_min` apply to
//! it. Carrion is not in that sum. Nothing about that crosses the layer's boundary: the organic matter was already
//! in the system, in the log or in the leaf fall, so it is **not** `fixed_in` and the
//! withdrawal is **not** `consumed_*_out` — those name material a consumer *outside* this
//! layer took, and a glowcap is a stand inside it. A litter-shredding animal's
//! [`crate::Flora::take_litter`] **is** `consumed_*_out`, off the very same pool, which is
//! how the two ledgers stay apart while the two eaters share one stock. What the ledger gains is a per-species diagnostic flux,
//! [`FloraLedger::substrate_uptake`], which is a report and not a boundary term, exactly
//! like the three `propagule_*` arrays.
//!
//! # Organic matter and mineral
//!
//! The two are separate currencies (`lib.rs`'s `FloraLedger`). Assimilation creates **organic matter** from light (`fixed_in`); every respiration —
//! maintenance, construction `c_g`, reflush, a capped income's leftover, decomposition —
//! destroys organic matter and books it out (`respired_out`, energy to heat) and moves
//! **no** mineral. Mineral only ever moves between stocks: the site's pool pays
//! `n_tissue` per unit of new tissue, tissue that dies or falls carries its mineral into
//! litter and dead wood, and decomposition hands it back to the pool.
//!
//! A stand's mineral is its own stock and not `n_tissue · O`: respiration takes organic
//! matter and leaves mineral behind. Every transfer out of a stand therefore takes the
//! same *fraction* of its mineral as of its organic matter ([`pull_mineral`]).
//!
//! Every clamp is a `min` against the stock it reads, so nothing here can go negative.

use cubarium_voxel::organism::{Budget, settle_mineral, tissue_funded};
use cubarium_voxel::{DT, Material, Materials, VoxelView, World};
#[cfg(test)]
use cubarium_voxel::Command as WorldCommand;

use crate::{DeathCause, DeathRecord, Flora, FloraConfig, FloraLedger, Ground, Site, Species, SpeciesConfig, Stand, Taken, Trophic};

/// Stream keys, so two draws in one tick cannot be the same draw. One per rule that
/// draws; the seed bank's and dispersal's are drawn in `seeds.rs`.
pub(crate) const DOMAIN_DISPERSAL: u64 = 1;
pub(crate) const DOMAIN_GERMINATION: u64 = 2;
const DOMAIN_FALL: u64 = 3;
/// A lanternberry's rest between crops.
const DOMAIN_FRUIT_REST: u64 = 12;
pub(crate) const DOMAIN_WIND: u64 = 4;
pub(crate) const DOMAIN_SPORES: u64 = 5;
pub(crate) const DOMAIN_WATER: u64 = 6;
pub(crate) const DOMAIN_CLONAL: u64 = 7;
pub(crate) const DOMAIN_RUNNER: u64 = 8;
pub(crate) const DOMAIN_ATTRITION: u64 = 9;
pub(crate) const DOMAIN_OVERTOP: u64 = 10;
/// The burr pass's streams: pickup draws and retention draws.
pub(crate) const DOMAIN_BURR: u64 = 13;
pub(crate) const DOMAIN_BURR_HOLD: u64 = 14;

pub(crate) use cubarium_voxel::{KeyedRng as Rng, mix};

/// One world tick of the plant layer. `clocked`: on the plant clock
/// ([`FloraConfig::step_every`]), else a plant step of this one tick.
pub(crate) fn step(flora: &mut Flora, world: &mut World, threads: usize, clocked: bool) {
    // A plan an overlapped tick left unsettled is applied first, so the ledger still
    // closes if a host switches schedules between ticks.
    settle_drink(flora, world);
    if let Some(span) = advance(flora, clocked, world.tick()) {
        run(flora, Target::Now(world), threads, span);
    }
}

/// [`step`] against a read copy: the drink is planned and held for [`settle_drink`].
pub(crate) fn step_planned(flora: &mut Flora, world: &World, threads: usize, clocked: bool) {
    if let Some(span) = advance(flora, clocked, world.tick()) {
        run(flora, Target::Planned(world), threads, span);
    }
}

/// **The plant clock** ([`FloraConfig::step_every`]). Every world tick moves the
/// counter and clears the tick's observations (receipts and deaths describe one tick);
/// on a plant step's tick it returns how many ticks that step covers. Unclocked, every
/// tick is a plant step of one tick.
///
/// Clocked, the step falls on the world ticks `world_tick` that are multiples of
/// `step_every`: the phase the coarse water keeps ([`cubarium_voxel::World::water_due`]),
/// so with `water_every = step_every` the plant step and the coarse lateral water fall on
/// the same tick and a schedule can run the two long phases side by side. `world_tick` is
/// the world's clock as the caller's water read it. The step still covers every tick
/// since the last one, so its `dt` is exact whatever the phase it started from.
///
/// The counter moves **first**, so that `flora.tick` is the tick this step produces: the
/// one whose state the caller will read when the step returns. Everything dated inside a
/// step — an arrival bin's start, a bin's age, the germination lottery's seed — then uses
/// one clock reading, and a bin's age at the end of the tick it landed in is zero rather
/// than minus one.
fn advance(flora: &mut Flora, clocked: bool, world_tick: u64) -> Option<u64> {
    flora.tick += 1;
    // This tick's delivery receipts start empty: they are **observation and not state**,
    // they describe only the tick that is about to run, and a caller that never reads them
    // must not accumulate them (Astra R10.3, `DeliveryReceipt`).
    flora.deliveries.clear();
    flora.death_log.clear();
    flora.since_step = flora.since_step.saturating_add(1);
    if clocked && !plant_tick(world_tick, flora.config.step_every) {
        return None;
    }
    Some(u64::from(std::mem::take(&mut flora.since_step)))
}

/// Whether world tick `world_tick` is a plant step's on a clock of `step_every`.
pub(crate) fn plant_tick(world_tick: u64, step_every: u32) -> bool {
    world_tick.is_multiple_of(u64::from(step_every.max(1)))
}

/// The seconds a plant step of `span` world ticks covers.
pub(crate) fn span_dt(span: u64) -> f64 {
    span as f64 * DT
}

/// Whether a periodic event falls inside the step's window of world ticks
/// `(tick − span, tick]`: some tick `t` there with `(key + t) mod period == 0`
/// (wrapping, as the per-tick checks always read it). The per-tick test is `span = 1`.
pub(crate) fn due_in_window(key: u64, tick: u64, span: u64, period: u64) -> bool {
    let period = period.max(1);
    (0..span.min(tick + 1)).any(|j| key.wrapping_add(tick - j).is_multiple_of(period))
}

/// `1 − exp(−rate · dt)`: the share a first-order decay at `rate` per second takes in
/// `dt` seconds, whatever the step.
pub(crate) fn decay_share(rate: f64, dt: f64) -> f64 {
    if rate <= 0.0 || dt <= 0.0 {
        0.0
    } else {
        -(-rate * dt).exp_m1()
    }
}

/// The world a tick runs against: the live one, which the drink withdraws from as it goes
/// (the chained tick), or a read copy, against which the drink is only planned (the
/// overlapped tick). Every other phase only reads it.
pub(crate) enum Target<'a> {
    Now(&'a mut World),
    Planned(&'a World),
}

impl Target<'_> {
    fn world(&self) -> &World {
        match self {
            Target::Now(w) => w,
            Target::Planned(w) => w,
        }
    }
}

/// One plant step in its parts, on one thread's call: [`prep`], the reads
/// ([`PlantStep::read`]), the drink's close ([`drink_close`]) and the rest
/// ([`finish_step`]). A schedule that overlaps the reads with the animals' calls the parts
/// itself (`Flora::begin_planned_clocked` and its siblings).
fn run(flora: &mut Flora, mut target: Target<'_>, threads: usize, span: u64) {
    cubarium_voxel::voxel_phase!(FloraStep, {
        let mut step = prep(flora, target.world(), threads, span);
        step.read(flora, target.world(), threads);
        drink_close(flora, &mut step, &mut target, threads);
        finish_step(flora, target.world(), step, threads);
    });
}

/// **A plant step between its parts.** [`prep`] makes it; [`PlantStep::read`] fills its
/// light and the drink's plan reading the layer through `&Flora` only — the layer's sky
/// cache and drink scratch travel in here meanwhile, so nothing the reads write is the
/// layer's — and [`drink_close`] hands both back and books the drink; [`finish_step`]
/// consumes it. Not state: it lives inside one tick.
pub struct PlantStep {
    span: u64,
    climate: Climate,
    /// The stocks decomposition may draw on: what each site held as the step began.
    pre: Vec<Pre>,
    film_steps: u64,
    film_sites: Vec<Site>,
    /// The layer's sky cache, out of it from [`prep`] to [`drink_close`].
    sky: Vec<(Site, f64)>,
    /// The layer's drink scratch, likewise.
    scratch: DrinkScratch,
    /// Per stand, from the reads.
    light: Vec<f64>,
    film_open: Vec<f64>,
    /// Per stand, from the drink's close.
    moisture: Vec<Drink>,
}

impl std::fmt::Debug for PlantStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlantStep").field("span", &self.span).finish_non_exhaustive()
    }
}

/// **The plant step's first part**, everything that removes a stand or touches what the
/// reads read: the decomposition snapshot, terrain pruning, the sky cache's version,
/// the cover's prep, drowning, and the vines' drink wants. Writes the layer.
fn prep(flora: &mut Flora, world: &World, threads: usize, span: u64) -> PlantStep {
    let dt = span_dt(span);
    // The stocks decomposition is allowed to draw on: what each site held when the tick
    // started, taken before anything at all moves. Litter, dead wood and carrion deposited
    // by this tick's drownings, senescence and deaths are eligible from the next tick, per
    // §5; a consumer's deposit from the previous inter-tick is in here, because the
    // snapshot is taken before the tick and the deposit happened before that. Sites this
    // tick removes are simply never looked up again.
    let pre: Vec<Pre> = flora
        .ground
        .iter()
        .map(|g| Pre {
            site: g.site,
            litter: g.litter,
            dead_wood: g.dead_wood,
            carrion: g.carrion,
        })
        .collect();
    cubarium_voxel::voxel_phase!(Prune, { prune_unsupported(flora, world) });
    cubarium_voxel::voxel_phase!(SkyCache, { refresh_sky_cache(flora, world) });
    cubarium_voxel::voxel_phase!(Cover, { crate::cover::prep(flora, world) });
    cubarium_voxel::voxel_phase!(Drown, { drown(flora, world, threads) });

    let climate = Climate {
        dt,
        ..Climate::of(world, flora.config.upkeep_q10)
    };
    // On a film tick the light phase also reads the sky and canopy over every site
    // holding glassfilm (`film.rs`).
    let film_steps = crate::film::film_steps(flora.tick, span);
    let film_sites = if film_steps > 0 {
        crate::film::receivers(flora)
    } else {
        Vec::new()
    };
    // The vines' wants, here and not in the reads: reading their root boxes also writes
    // each vine's moisture, which the animals may read beside the reads.
    let mut scratch = std::mem::take(&mut flora.drink.0);
    debug_assert!(!scratch.pending, "a planned drink was never settled");
    cubarium_voxel::voxel_phase!(Drink, {
        let view = world.view();
        let DrinkScratch {
            vine_box,
            vine_drinkable,
            vine_wants,
            ..
        } = &mut scratch;
        crate::cover::drink_wants(flora, &view, vine_box, vine_drinkable, vine_wants, dt);
    });
    PlantStep {
        span,
        climate,
        pre,
        film_steps,
        film_sites,
        sky: std::mem::take(&mut flora.sky),
        scratch,
        light: Vec::new(),
        film_open: Vec::new(),
        moisture: Vec::new(),
    }
}

impl PlantStep {
    /// **The plant step's reads**: light, then the drink's per-stand reads, per-voxel
    /// totals and per-voxel bounds (the plan). Reads the layer through `&Flora` and writes
    /// only this step: the sky cache it carries and the drink scratch.
    pub(crate) fn read(&mut self, flora: &Flora, world: &World, threads: usize) {
        let (light, film_open) = cubarium_voxel::voxel_phase!(Light, {
            light_per_stand(
                &flora.config,
                &flora.stands,
                &mut self.sky,
                world,
                &self.film_sites,
                threads,
            )
        });
        self.light = light;
        self.film_open = film_open;
        cubarium_voxel::voxel_phase!(Drink, {
            drink_reads(flora, &self.sky, world, threads, &mut self.scratch, self.climate.dt)
        });
    }
}

/// **The drink's close**: the sky cache and the scratch back in the layer, and the drink
/// taken (the chained tick: from the live world now) or held as the plan (the overlapped
/// tick), each stand's and vine's share credited.
fn drink_close(flora: &mut Flora, step: &mut PlantStep, target: &mut Target<'_>, threads: usize) {
    flora.sky = std::mem::take(&mut step.sky);
    let mut scratch = std::mem::take(&mut step.scratch);
    step.moisture = cubarium_voxel::voxel_phase!(Drink, {
        drink_settle(flora, target, threads, &mut scratch)
    });
    flora.drink.0 = scratch;
}

/// **The plant step's last part**: defences relax, then feed, growth, the cover's growth,
/// decomposition and the film, the seed bank, propagation and the crowns.
fn finish_step(flora: &mut Flora, world: &World, step: PlantStep, threads: usize) {
    let PlantStep {
        span,
        climate,
        pre,
        film_steps,
        film_sites,
        light,
        film_open,
        moisture,
        ..
    } = step;
    let dt = climate.dt;
    // The induced grazing defence relaxes at each species' half-life
    // (`design/handoffs/voxel-lifespan-defence-2026-09-25.md`, B). Nothing in the step
    // before this reads it; the animals' reads beside the plant reads see it as the last
    // step left it.
    decay_defences(flora, dt);
    let substrate =
        cubarium_voxel::voxel_phase!(Feed, { feed(flora, world, &moisture, threads, dt) });
    // Runner daughters' support, out of their parents' reserves, before either grows.
    cubarium_voxel::voxel_phase!(Grow, { support_runners(flora, &light, &moisture, climate) });
    cubarium_voxel::voxel_phase!(Grow, {
        grow(flora, world, &light, climate, &moisture, &substrate, threads)
    });
    cubarium_voxel::voxel_phase!(Cover, { crate::cover::grow(flora, world, climate, span) });
    cubarium_voxel::voxel_phase!(Decompose, { decompose(flora, &pre, dt) });
    if film_steps > 0 {
        let open: Vec<(Site, f64)> = film_sites.into_iter().zip(film_open).collect();
        // Timed with decomposition: the profile has no phase of its own for it.
        cubarium_voxel::voxel_phase!(Decompose, {
            crate::film::step(flora, world, &open, climate.daylight, film_steps)
        });
    }
    cubarium_voxel::voxel_phase!(SeedBank, { crate::seeds::seed_bank(flora, world, span) });
    cubarium_voxel::voxel_phase!(Propagate, { crate::seeds::propagate(flora, world, dt) });
    // Growth, dieback, deaths, falls and germinations have all happened: the cached
    // crowns follow the stands they now describe.
    cubarium_voxel::voxel_phase!(Crowns, { flora.refresh_crowns(Some(world)) });
    #[cfg(feature = "profile")]
    {
        use cubarium_voxel::profile::{Count, add};
        add(Count::Stands, flora.stands.len() as u64);
        add(Count::GroundSites, flora.ground.len() as u64);
    }
}

/// [`step_planned`] on the plant clock in its parts, for a schedule that runs the reads
/// beside the animals' own: the clock and [`prep`], `None` on a tick that is not a plant
/// step.
pub(crate) fn begin_planned(flora: &mut Flora, world: &World, threads: usize) -> Option<PlantStep> {
    let span = advance(flora, true, world.tick())?;
    Some(cubarium_voxel::voxel_phase!(FloraStep, { prep(flora, world, threads, span) }))
}

/// The planned drink's close for [`begin_planned`]'s step: the plan is then held in the
/// layer for `take_plan`.
pub(crate) fn plan_drink(flora: &mut Flora, step: &mut PlantStep, world: &World, threads: usize) {
    cubarium_voxel::voxel_phase!(FloraStep, {
        drink_close(flora, step, &mut Target::Planned(world), threads)
    });
}

/// The rest of [`begin_planned`]'s step.
pub(crate) fn finish_planned(flora: &mut Flora, step: PlantStep, world: &World, threads: usize) {
    cubarium_voxel::voxel_phase!(FloraStep, { finish_step(flora, world, step, threads) });
}

/// The fewest stands a worker is handed: under this a split costs more than it saves.
const MIN_CHUNK: usize = 256;

/// How many contiguous chunks [`for_chunks`] splits `n` stands into for `threads` workers.
fn chunk_count(n: usize, threads: usize) -> usize {
    if cfg!(feature = "parallel") {
        threads.max(1).min(n / MIN_CHUNK).max(1)
    } else {
        1
    }
}

/// Run `f` over `0..n` split into [`chunk_count`] contiguous ranges, chunk `k` with
/// `states[k]` (grown with `fresh` as needed), on the compute pool when there is more than
/// one chunk. The results come back **in chunk order** whatever order the workers finished
/// in, so a caller that folds them in that order gets exactly what one pass over `0..n`
/// would have built.
fn for_chunks<S: Send, T: Send + 'static>(
    n: usize,
    threads: usize,
    states: &mut Vec<S>,
    fresh: impl Fn() -> S,
    f: impl Fn(std::ops::Range<usize>, &mut S) -> T + Sync,
) -> Vec<T> {
    let chunks = chunk_count(n, threads);
    while states.len() < chunks {
        states.push(fresh());
    }
    let per = n.div_ceil(chunks);
    let range = |k: usize| (k * per).min(n)..((k + 1) * per).min(n);
    #[cfg(feature = "parallel")]
    if chunks > 1 {
        let pool = bevy_tasks::ComputeTaskPool::get_or_init(|| {
            bevy_tasks::TaskPoolBuilder::new()
                .num_threads(threads)
                .build()
        });
        let f = &f;
        let mut parts: Vec<(usize, T)> = pool.scope(|scope| {
            for (k, state) in states.iter_mut().take(chunks).enumerate() {
                let r = range(k);
                scope.spawn(async move { (k, f(r, state)) });
            }
        });
        parts.sort_by_key(|&(k, _)| k);
        return parts.into_iter().map(|(_, t)| t).collect();
    }
    states
        .iter_mut()
        .take(chunks)
        .enumerate()
        .map(|(k, state)| f(range(k), state))
        .collect()
}

// ------------------------------------------------------------------ 1. terrain

/// Drop every site whose support face a terrain edit took away, booking its stocks out.
fn prune_unsupported(flora: &mut Flora, world: &World) {
    let Flora {
        config,
        stands,
        ground,
        ledger,
        ..
    } = flora;
    let view = world.view();
    let supported = |site: &Site| view.is_support(site.x as i64, site.y, site.z);

    // Nearly every tick every site is still supported: then nothing is copied at all.
    if !stands.iter().all(|s| supported(&s.site)) {
        let mut kept = Vec::with_capacity(stands.len());
        for stand in stands.iter() {
            if supported(&stand.site) {
                kept.push(*stand);
            } else {
                // The parcel is organic matter this layer holds, so it is booked out with
                // the stand that was saving it.
                let organic = stand.material();
                ledger.removed_organic_out += organic;
                ledger.removed_mineral_out += stand.mineral;
                ledger.removed_energy_out +=
                    config.species(stand.species).energy_density * organic;
            }
        }
        *stands = kept;
    }

    if ground.iter().all(|g| supported(&g.site)) {
        return;
    }
    let taken = std::mem::take(ground);
    let mut kept = Vec::with_capacity(taken.len());
    for g in taken {
        if supported(&g.site) {
            kept.push(g);
        } else {
            // **Every** stock the site held, carrion included. Astra's R8.1: the three
            // carrion stocks were missing here, and since they are in `FloraView`'s own
            // totals that was an actual residual and not a missing label — a deposit of
            // `(0.4, 0.012, 0.9)` on a face the terrain then took away left residuals of
            // −0.4, −0.012 and −0.9, and so did a deposit made directly on an unsupported
            // site, which `Flora::deposit` promises to book out here.
            ledger.removed_organic_out += g.litter + g.dead_wood + g.carrion + g.film;
            ledger.removed_mineral_out += g.mineral
                + g.litter_mineral
                + g.dead_wood_mineral
                + g.carrion_mineral
                + g.film_mineral;
            ledger.removed_energy_out +=
                g.litter_energy + g.dead_wood_energy + g.carrion_energy + g.film_energy;
            // The seed bank goes out with the ground it sat on: a cohort whose support
            // face is gone has nowhere to germinate, and it is booked as removed rather
            // than dropped.
            if let Some(l) = g.vine_seeds {
                ledger.removed_organic_out += l.organic;
                ledger.removed_mineral_out += l.mineral;
                ledger.removed_energy_out += config.latticevine.energy_density * l.organic;
            }
            for c in &g.seeds {
                ledger.removed_organic_out += c.organic;
                ledger.removed_mineral_out += c.mineral;
                ledger.removed_energy_out += config.species(c.species).energy_density * c.organic;
            }
        }
    }
    *ground = kept;
}

// ---------------------------------------------------------------- 2. sky cache

/// The sky cache is a site-sorted `Vec<(Site, f64)>` looked up by binary search and
/// dropped whole when the terrain version moves: sky visibility is pure geometry, so
/// nothing but a material change can invalidate it, and a `Vec` keeps iteration and
/// insertion deterministic where a `HashMap` would not.
fn refresh_sky_cache(flora: &mut Flora, world: &World) {
    let version = world.view().terrain_version;
    if flora.sky_version != Some(version) {
        flora.sky.clear();
        flora.sky_version = Some(version);
    }
}

pub(crate) fn sky_at(cache: &mut Vec<(Site, f64)>, view: &VoxelView<'_>, site: Site) -> f64 {
    match cache.binary_search_by_key(&site, |e| e.0) {
        Ok(i) => cache[i].1,
        Err(i) => {
            #[cfg(feature = "profile")]
            cubarium_voxel::profile::add(cubarium_voxel::profile::Count::SkyRays, 1);
            let value = view.sky_visibility(site.x as i64, site.y, site.z);
            cache.insert(i, (site, value));
            value
        }
    }
}

/// [`sky_at`] for every stand, in `stands` order, in one pass: the stands and the cache
/// are both site-sorted, so one merge walk finds every cached site, the missing sites'
/// visibility is computed (split over `threads` when there are enough of them — a moved
/// terrain misses every site), and they are merged in. The same values, and the same
/// cache entries in the same order, as one [`sky_at`] per stand.
pub(crate) fn sky_all(
    cache: &mut Vec<(Site, f64)>,
    view: &VoxelView<'_>,
    stands: &[Stand],
    threads: usize,
) -> Vec<f64> {
    sky_sites(cache, view, stands.len(), |i| stands[i].site, threads)
}

/// [`sky_all`] over any ascending list of `n` sites, `site(i)` the `i`-th.
pub(crate) fn sky_sites(
    cache: &mut Vec<(Site, f64)>,
    view: &VoxelView<'_>,
    n: usize,
    site: impl Fn(usize) -> Site + Sync,
    threads: usize,
) -> Vec<f64> {
    let mut open = vec![0.0; n];
    let mut missing: Vec<usize> = Vec::new();
    let mut k = 0;
    for i in 0..n {
        let s = site(i);
        while k < cache.len() && cache[k].0 < s {
            k += 1;
        }
        if k < cache.len() && cache[k].0 == s {
            open[i] = cache[k].1;
        } else {
            missing.push(i);
        }
    }
    if missing.is_empty() {
        return open;
    }
    #[cfg(feature = "profile")]
    cubarium_voxel::profile::add(
        cubarium_voxel::profile::Count::SkyRays,
        missing.len() as u64,
    );
    let values: Vec<f64> = for_chunks(
        missing.len(),
        threads,
        &mut Vec::new(),
        || (),
        |r, _| {
            r.map(|m| {
                let site = site(missing[m]);
                view.sky_visibility(site.x as i64, site.y, site.z)
            })
            .collect::<Vec<f64>>()
        },
    )
    .concat();
    let old = std::mem::take(cache);
    cache.reserve(old.len() + missing.len());
    let mut fresh = missing.iter().zip(&values).peekable();
    for entry in old {
        while let Some((&i, &v)) = fresh.next_if(|(i, _)| site(**i) < entry.0) {
            open[i] = v;
            cache.push((site(i), v));
        }
        cache.push(entry);
    }
    for (&i, &v) in fresh {
        open[i] = v;
        cache.push((site(i), v));
    }
    open
}

// ----------------------------------------------------------------- 3. drowning

fn drown(flora: &mut Flora, world: &World, threads: usize) {
    let Flora {
        config,
        stands,
        ground,
        ledger,
        death_log,
        ..
    } = flora;
    let view = world.view();
    // Who drowns is a read per stand, split over threads; the deaths are booked in order.
    let doomed: Vec<usize> = {
        let (config, stands, view) = (&*config, &stands[..], &view);
        for_chunks(
            stands.len(),
            threads,
            &mut Vec::new(),
            || (),
            |r, _| {
                r.filter(|&i| {
                    let stand = &stands[i];
                    let limit = config.species(stand.species).drown_depth_m;
                    view.water_depth_m(stand.site.x as i64, stand.site.y, stand.site.z) > limit
                })
                .collect::<Vec<usize>>()
            },
        )
        .concat()
    };
    for &i in doomed.iter().rev() {
        die(config, &view, &stands[i], ground, ledger, DeathCause::Drowned, death_log);
    }
    remove_at(stands, &doomed);
    settle_places(config, stands, ledger);
}

/// Remove the stands at `doomed` — ascending, distinct indices — in one pass, keeping the
/// rest in order. One compaction instead of a `Vec::remove` (a shift of every later stand)
/// per death.
pub(crate) fn remove_at(stands: &mut Vec<Stand>, doomed: &[usize]) {
    if doomed.is_empty() {
        return;
    }
    let mut next = 0;
    let mut i = 0;
    stands.retain(|_| {
        let gone = next < doomed.len() && doomed[next] == i;
        if gone {
            next += 1;
        }
        i += 1;
        !gone
    });
}

/// Insert `born` into `stands`, sorted by `(site, place)` ([`crate::FaceLayer`]), keeping
/// it sorted: every `(site, place)` in `born` is distinct and holds no stand yet. One merge
/// instead of a `Vec::insert` (a shift of every later stand) per birth; a lone birth is
/// still one insert.
pub(crate) fn insert_sorted(config: &FloraConfig, stands: &mut Vec<Stand>, mut born: Vec<Stand>) {
    if born.len() <= 1 {
        for s in born {
            let at = crate::find_stand(config, stands, s.site, s.place)
                .expect_err("a birth in a free place");
            stands.insert(at, s);
        }
        return;
    }
    born.sort_unstable_by_key(|s| config.stand_key(s));
    let old = std::mem::take(stands);
    let mut out = Vec::with_capacity(old.len() + born.len());
    let mut born = born.into_iter().peekable();
    for s in old {
        let key = config.stand_key(&s);
        while let Some(b) = born.next_if(|b| config.stand_key(b) < key) {
            out.push(b);
        }
        debug_assert!(born.peek().is_none_or(|b| config.stand_key(b) != key));
        out.push(s);
    }
    out.extend(born);
    *stands = out;
}

/// Whether two stands **on one face** shade each other: both plants ([`crate::FaceLayer`]).
/// A face's plant and its fungus never do.
#[inline]
pub(crate) fn same_face_shades(a: &Stand, b: &Stand) -> bool {
    a.place != crate::FaceLayer::Fungus && b.place != crate::FaceLayer::Fungus
}

/// **Each face's plants in their places** (CONTEST, [`crate::FaceLayer::Under`]): the taller
/// of a face's two plants by crown height holds [`crate::FaceLayer::Plant`] and the other
/// the place beneath (a tie keeps them as they are); a plant beneath with no plant above it
/// — the top one died — holds the face. A rise from beneath by height is booked in
/// [`FloraLedger::escapes`]. `stands` stays sorted by `(site, place)`.
pub(crate) fn settle_places(config: &FloraConfig, stands: &mut [Stand], ledger: &mut FloraLedger) {
    use crate::FaceLayer;
    let n = stands.len();
    let mut moved = false;
    let mut i = 0;
    while i < n {
        let site = stands[i].site;
        let mut j = i + 1;
        while j < n && stands[j].site == site {
            j += 1;
        }
        // In `(site, place)` order the place beneath is the run's last.
        if j - i >= 1 && stands[j - 1].place == FaceLayer::Under {
            let u = j - 1;
            match (i..u).find(|&k| stands[k].place == FaceLayer::Plant) {
                None => {
                    stands[u].place = FaceLayer::Plant;
                    moved = true;
                }
                Some(p) => {
                    let height = |s: &Stand| config.species(s.species).crown_height_m_at(s.wood);
                    if height(&stands[u]) > height(&stands[p]) {
                        stands[u].place = FaceLayer::Plant;
                        stands[p].place = FaceLayer::Under;
                        ledger.escapes[stands[u].species.index()] += 1;
                        moved = true;
                    }
                }
            }
        }
        i = j;
    }
    if moved {
        // Nearly sorted: only the runs that moved are out of order.
        stands.sort_by_key(|s| (s.site, s.place));
    }
}

// -------------------------------------------------------------------- 4. light

/// Light for every stand, in `stands` order.
///
/// **Receivers as well as occluders** (the audit's §5, "Light needs receivers as well
/// as occluders"). A stand's income is assessed **per foliage layer**, at that layer's
/// own physical height, and the stand's light is the average over its layers weighted
/// by each layer's share of its **stock** — where its leaves actually are. A stand
/// whose rosette is in shade and whose crown is in the sun earns between the two, and
/// a plant browsed down to its crown earns more light than one browsed down to its
/// rosette, at the same total foliage.
///
/// An **occluder** is any foliage-bearing layer of another stand whose band top is
/// strictly above the receiving layer's band top and whose footprint contains the
/// receiver's column; it attenuates by `exp(-k · (1-p) · stock / area_m2)` over its own
/// physical area. Porosity is the layer's transmission and enters only here — the cone
/// sees a porous canopy as leaves all the same
/// (`design/voxel-encounter-contract-2026-09-21.md` §8).
///
/// A stand never shades itself, and a layer level with another's top does not shade
/// it — only a strictly higher one does, which is the pre-layer rule verbatim.
///
/// **On a single-layer species this is numerically what it always was**: the one layer's
/// band top is the crown top the old model compared, its radius is the crown radius, its
/// stock is the whole of `P`, its weight is one, and its area is the crown's. The
/// authored porosity is the only new factor (`tests/layers.rs`).
fn light_per_stand(
    config: &FloraConfig,
    stands: &[Stand],
    sky: &mut Vec<(Site, f64)>,
    world: &World,
    film: &[Site],
    threads: usize,
) -> (Vec<f64>, Vec<f64>) {
    let view = world.view();
    let voxel_m = view.config.voxel_m;
    // Every stand's foliage layers, flat: stand `j`'s are `layers[span[j].0..span[j].1]`.
    // Built per chunk of stands and joined in stand order: each stand's layers are its
    // own arithmetic, so the split changes nothing.
    let parts = for_chunks(
        stands.len(),
        threads,
        &mut Vec::new(),
        || (),
        |r, _| {
            let mut layers: Vec<Shade> = Vec::with_capacity(r.len() * 2);
            let mut span: Vec<(u32, u32)> = Vec::with_capacity(r.len());
            for s in &stands[r] {
                let from = layers.len() as u32;
                shade_layers_into(config, s, voxel_m, &mut layers);
                span.push((from, layers.len() as u32));
            }
            (layers, span)
        },
    );
    let mut layers: Vec<Shade> = Vec::with_capacity(parts.iter().map(|p| p.0.len()).sum());
    let mut span: Vec<(u32, u32)> = Vec::with_capacity(stands.len());
    for (l, sp) in parts {
        let base = layers.len() as u32;
        span.extend(sp.into_iter().map(|(a, b)| (a + base, b + base)));
        layers.extend(l);
    }
    // A conservative bound per **stand**, so the pair loop below stays the O(n²) it was
    // before layers instead of O(n² · layers²): no layer of a stand can shade above the
    // stand's own highest layer top, or outside its widest layer's footprint. Skipping
    // on the bound changes no result — it only avoids opening a stand whose every layer
    // would have been rejected.
    let envelope: Vec<(f64, f64)> = span
        .iter()
        .map(|&(from, to)| {
            layers[from as usize..to as usize]
                .iter()
                .fold((f64::NEG_INFINITY, 0.0f64), |(t, r), l| {
                    (t.max(l.top), r.max(l.radius))
                })
        })
        .collect();
    let width = view.config.width as f64;
    // Only a stand whose widest footprint can reach a receiver's column can shade it, so
    // each receiver walks the stands filed under its own bucket instead of every stand
    // in the world. The buckets hold stand indices in ascending order, which is the order
    // the full sweep visited them in: the same factors multiply in the same order, and
    // the result is the full sweep's to the bit (`the_bucketed_light_is_the_full_sweep`).
    let index = ShadeIndex::build(
        stands,
        &envelope,
        view.config.width,
        view.config.depth,
        threads,
    );

    // The sky first, in one pass: it is the one read that fills a cache.
    let open: Vec<f64> = sky_all(sky, &view, stands, threads);
    let stands = &stands[..];
    let receive = |i: usize| -> f64 {
        let stand = &stands[i];
        let open = open[i];
        // A saprotroph earns nothing from light, so what it reads is its **exposure**
        // (GLOWCAP-2, [`SpeciesConfig::sun_mycelium_loss_per_s`]): the share of the open sun
        // that reaches its cap, raw, with its own face's plant over it as well — a cap
        // under the leaves of the plant it shares a face with is in that plant's shade.
        let saprotroph = config.species(stand.species).trophic == Trophic::Saprotroph;
        let mine = &layers[span[i].0 as usize..span[i].1 as usize];
        let near = index.near(stand.site);
        // The weights: where this stand's tissue is. A stand with no foliage left has
        // no tissue to weight, so it is assessed at its layers' capacities instead —
        // its income is zero either way (`A ∝ P`), and this keeps the number defined.
        let total: f64 = mine.iter().map(|l| l.stock).sum();
        let weight_by_stock = total > 0.0;
        let mut l = 0.0;
        let mut weighed = 0.0;
        for layer in mine {
            let w = if weight_by_stock {
                layer.stock
            } else {
                layer.share
            };
            if !(w > 0.0) {
                continue;
            }
            let mut here = open;
            for c in near {
                if c.j as usize == i || c.top <= layer.top {
                    continue;
                }
                // A face's plant and its fungus never shade each other (`FaceLayer`): the
                // cap grows under the plant's leaves, and earns nothing from light. A face's
                // two plants do, by the same law as any other pair (CONTEST).
                if stands[c.j as usize].site == stand.site
                    && !saprotroph
                    && !same_face_shades(&stands[c.j as usize], stand)
                {
                    continue;
                }
                let sx = wrapped_delta(c.x, layer.x, width);
                let sz = c.z - layer.z;
                if sx * sx + sz * sz > c.radius * c.radius {
                    continue;
                }
                let (from, to) = span[c.j as usize];
                for above in &layers[from as usize..to as usize] {
                    if above.top <= layer.top {
                        continue;
                    }
                    let dx = wrapped_delta(above.x, layer.x, width);
                    let dz = above.z - layer.z;
                    if dx * dx + dz * dz > above.radius * above.radius {
                        continue;
                    }
                    here *= above.attenuation;
                }
            }
            l += w * here;
            weighed += w;
        }
        let l = if weighed > 0.0 { l / weighed } else { open };
        if saprotroph {
            l
        } else {
            light_response(config.species(stand.species), l)
        }
    };
    let lit = for_chunks(
        stands.len(),
        threads,
        &mut Vec::new(),
        || (),
        |r, _| r.map(receive).collect::<Vec<f64>>(),
    )
    .concat();
    // Glassfilm (`film.rs`): the open sky over each film site times what every foliage
    // layer above the face and over it lets through, the same sweep a stand's layer gets.
    // The film lies on the face: a layer shades it when its top is above `site.y`.
    if film.is_empty() {
        return (lit, Vec::new());
    }
    let film_sky = sky_sites(sky, &view, film.len(), |i| film[i], threads);
    let film_open = film
        .iter()
        .zip(film_sky)
        .map(|(site, open)| {
            let (x, z, top) = (site.x as f64, site.z as f64, site.y as f64);
            let mut here = open;
            for c in index.near(*site) {
                if c.top <= top {
                    continue;
                }
                let sx = wrapped_delta(c.x, x, width);
                let sz = c.z - z;
                if sx * sx + sz * sz > c.radius * c.radius {
                    continue;
                }
                let (from, to) = span[c.j as usize];
                for above in &layers[from as usize..to as usize] {
                    if above.top <= top {
                        continue;
                    }
                    let dx = wrapped_delta(above.x, x, width);
                    let dz = above.z - z;
                    if dx * dx + dz * dz > above.radius * above.radius {
                        continue;
                    }
                    here *= above.attenuation;
                }
            }
            here
        })
        .collect();
    (lit, film_open)
}

/// Columns per side of one [`ShadeIndex`] bucket.
const SHADE_BUCKET: u32 = 2;

/// One stand filed under a bucket: its index and the envelope the sweep tests first,
/// packed so a receiver streams its bucket without chasing into the layers.
#[derive(Clone, Copy)]
struct Candidate {
    j: u32,
    top: f64,
    x: f64,
    z: f64,
    radius: f64,
}

/// Every stand filed under each bucket of `SHADE_BUCKET²` columns that its widest layer's
/// footprint reaches, in ascending stand order: a superset of the stands that can shade any
/// receiver in that bucket, and exactly the stands the full sweep would not have skipped
/// on its envelope test. Stored flat (CSR) per **segment** of bucket rows: bucket `b` of a
/// segment holds `stands[start[b]..start[b + 1]]`. The segments are built side by side,
/// each walking every stand in order and filing only its own rows, so a bucket's stands are
/// the one-segment build's, in the same order.
struct ShadeIndex {
    nbx: usize,
    /// Bucket rows per segment.
    rows: usize,
    segments: Vec<ShadeSegment>,
}

/// One segment of a [`ShadeIndex`]: its buckets' stands, CSR.
struct ShadeSegment {
    start: Vec<u32>,
    stands: Vec<Candidate>,
}

impl ShadeIndex {
    fn build(
        stands: &[Stand],
        envelope: &[(f64, f64)],
        width: u32,
        depth: u32,
        threads: usize,
    ) -> ShadeIndex {
        let b = SHADE_BUCKET as usize;
        let nbx = (width as usize).div_ceil(b).max(1);
        let nbz = (depth as usize).div_ceil(b).max(1);
        // One segment on one thread; with more, twice as many as threads, so an uneven
        // row does not hold the rest up. Small plains are not worth splitting.
        let segments = if chunk_count(stands.len(), threads) > 1 {
            (2 * threads).min(nbz)
        } else {
            1
        };
        let rows = nbz.div_ceil(segments);
        let segments = nbz.div_ceil(rows);
        let segments = par_map(segments, threads, |seg| {
            let lo = seg * rows;
            let hi = ((seg + 1) * rows).min(nbz);
            ShadeSegment::build(stands, envelope, width, depth, nbx, lo..hi)
        });
        ShadeIndex {
            nbx,
            rows,
            segments,
        }
    }

    /// The stands filed under `site`'s bucket, ascending.
    fn near(&self, site: Site) -> &[Candidate] {
        let b = SHADE_BUCKET as usize;
        let bz = site.z as usize / b;
        let seg = &self.segments[bz / self.rows];
        let k = (bz % self.rows) * self.nbx + site.x as usize / b;
        &seg.stands[seg.start[k] as usize..seg.start[k + 1] as usize]
    }
}

impl ShadeSegment {
    /// The buckets of rows `bz`, from every stand in ascending order.
    fn build(
        stands: &[Stand],
        envelope: &[(f64, f64)],
        width: u32,
        depth: u32,
        nbx: usize,
        bz: std::ops::Range<usize>,
    ) -> ShadeSegment {
        let b = SHADE_BUCKET as usize;
        let w = i64::from(width.max(1));
        let d = i64::from(depth.max(1));
        let buckets = bz.len() * nbx;
        // (bucket, stand) for every bucket of these rows a stand's footprint touches,
        // generated in ascending stand order. A footprint of radius `r` cells holds the
        // receiver columns at integer offsets `|dx|, |dz| <= floor(r)` — the envelope test
        // compares integer offsets against `r²` — with `x` wrapped and `z` clipped.
        let mut pairs: Vec<(u32, u32)> = Vec::new();
        let mut seen = vec![u32::MAX; nbx];
        let mut xs: Vec<usize> = Vec::with_capacity(nbx);
        for (j, stand) in stands.iter().enumerate() {
            let (top, radius) = envelope[j];
            // A stand with no foliage layer shades nothing: the sweep skips it on `top`.
            if top == f64::NEG_INFINITY {
                continue;
            }
            let reach = if radius.is_finite() {
                radius.max(0.0).floor() as i64
            } else {
                w
            };
            let z0 = i64::from(stand.site.z);
            let lo = ((z0 - reach).clamp(0, d - 1) as usize / b).max(bz.start);
            let hi = ((z0 + reach).clamp(0, d - 1) as usize / b).min(bz.end - 1);
            if lo > hi {
                continue;
            }
            xs.clear();
            if 2 * reach + 1 >= w {
                xs.extend(0..nbx);
            } else {
                let x0 = i64::from(stand.site.x);
                for cx in x0 - reach..=x0 + reach {
                    let bx = (cx.rem_euclid(w) as usize) / b;
                    if seen[bx] != j as u32 {
                        seen[bx] = j as u32;
                        xs.push(bx);
                    }
                }
            }
            for row in lo..=hi {
                for &bx in &xs {
                    pairs.push((((row - bz.start) * nbx + bx) as u32, j as u32));
                }
            }
        }
        // A stable counting sort by bucket keeps each bucket's stands ascending.
        let mut start = vec![0u32; buckets + 1];
        for &(k, _) in &pairs {
            start[k as usize + 1] += 1;
        }
        for k in 0..buckets {
            start[k + 1] += start[k];
        }
        let mut fill = start.clone();
        let blank = Candidate {
            j: 0,
            top: 0.0,
            x: 0.0,
            z: 0.0,
            radius: 0.0,
        };
        let mut filed = vec![blank; pairs.len()];
        for &(k, j) in &pairs {
            let (top, radius) = envelope[j as usize];
            let site = stands[j as usize].site;
            filed[fill[k as usize] as usize] = Candidate {
                j,
                top,
                x: site.x as f64,
                z: site.z as f64,
                radius,
            };
            fill[k as usize] += 1;
        }
        ShadeSegment {
            start,
            stands: filed,
        }
    }
}

/// `f(0..k)` over up to `threads` workers of the compute pool (the `parallel` feature), in
/// index order.
fn par_map<T: Send + 'static>(k: usize, threads: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    #[cfg(feature = "parallel")]
    if threads > 1 && k > 1 {
        let pool = bevy_tasks::ComputeTaskPool::get_or_init(|| {
            bevy_tasks::TaskPoolBuilder::new()
                .num_threads(threads)
                .build()
        });
        let f = &f;
        let mut parts: Vec<(usize, T)> = pool.scope(|scope| {
            for i in 0..k {
                scope.spawn(async move { (i, f(i)) });
            }
        });
        parts.sort_by_key(|&(i, _)| i);
        return parts.into_iter().map(|(_, t)| t).collect();
    }
    let _ = threads;
    (0..k).map(f).collect()
}

/// One foliage layer as the shade model sees it: where its top is, what it covers, and
/// what it holds.
struct Shade {
    x: f64,
    z: f64,
    /// The band's **top**, in voxels above the world floor. For a `[0, 1.0]` layer this
    /// is `site.y + crown_height(wood)`: the pre-layer crown top, unchanged.
    top: f64,
    /// Radius in **cells**, for the footprint test.
    radius: f64,
    stock: f64,
    share: f64,
    /// What this layer lets through to a layer under it:
    /// `exp(-k · (1-p) · stock / area_m2)`, the one factor the sweep multiplies in, taken
    /// once per layer rather than once per layer it shades.
    attenuation: f64,
}

/// A stand's foliage layers as the shade model sees them, appended to `out`.
fn shade_layers_into(config: &FloraConfig, stand: &Stand, voxel_m: f64, out: &mut Vec<Shade>) {
    out.extend(
        crate::layer_iter(config, stand, voxel_m)
            .filter(|l| l.kind.bears_foliage())
            .map(|l| Shade {
                x: stand.site.x as f64,
                z: stand.site.z as f64,
                top: l.band_v[1],
                radius: l.radius_v,
                stock: l.stock,
                share: l.share,
                attenuation: (-config.shade_k_per_m2 * (1.0 - l.porosity) * l.stock / l.area_m2)
                    .exp(),
            }),
    );
}

/// `L_eff = L (1 + light_half) / (L + light_half)`: a species with a small `light_half`
/// earns nearly its full income in shade, one with a large value needs open sky.
pub(crate) fn light_response(sc: &SpeciesConfig, l: f64) -> f64 {
    let half = sc.light_half.max(0.0);
    if l <= 0.0 {
        return 0.0;
    }
    if l + half <= 0.0 {
        return 1.0;
    }
    l * (1.0 + half) / (l + half)
}

fn wrapped_delta(a: f64, b: f64, period: f64) -> f64 {
    let mut d = a - b;
    while d > period * 0.5 {
        d -= period;
    }
    while d < -period * 0.5 {
        d += period;
    }
    d
}

// -------------------------------------------------------------------- 5. water

/// What one stand got, the moisture factor its income reads, and how waterlogged its
/// root box was — all three off the one pre-withdrawal read.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Drink {
    pub(crate) moisture: f64,
    pub(crate) taken_m3: f64,
    /// The fraction of the root box's voxels at or above the species' `saturated_pore`.
    pub(crate) saturated: f64,
    /// The stand's darkness ([`SpeciesConfig::darkness`] of its terrain sky): what its
    /// moisture read the cave damp at, and what its seep income is paid at. Zero for a
    /// species with no dark habitat.
    pub(crate) darkness: f64,
    /// The root box's mean **available water** (0 at the wilting point, 1 at field
    /// capacity; the cave damp included), what `μ` is the ramp of: curing reads it. Zero
    /// for an empty box.
    pub(crate) available: f64,
}

/// [`Flora::drink`]'s slot: the scratch, kept between ticks so a tick does not allocate
/// and zero a world-sized array (it was a thread-local; the overlapped tick runs the plant
/// step on whichever pool worker the executor picks, and one copy per worker is one
/// world-sized array per worker). **Not state**: a clone starts empty and a debug print
/// names it.
#[derive(Default)]
pub(crate) struct DrinkSlot(pub(crate) DrinkScratch);

impl Clone for DrinkSlot {
    fn clone(&self) -> DrinkSlot {
        DrinkSlot::default()
    }
}

impl std::fmt::Debug for DrinkSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DrinkSlot")
    }
}

/// Per-cell demand totals, what each touched cell gave up, and which cells were touched.
/// All zero between ticks, except while a planned drink waits for its settle.
#[derive(Default)]
pub(crate) struct DrinkScratch {
    /// `[total asked, accepted]` per cell, side by side so one read fetches both. Under a
    /// plan, "accepted" is what the read copy could give: the plan.
    cell: Vec<[f64; 2]>,
    /// One bit per cell: set when a demand first lands on it.
    touched: Vec<u64>,
    /// One per chunk of stands, reused tick to tick.
    chunks: Vec<DrinkChunk>,
    /// How many of `chunks` the last drink used.
    used: usize,
    /// The latticevines' (voxel, vine index, wanted), in vine order, and their box scratch.
    vine_wants: Vec<(u32, u32, f64)>,
    vine_box: Vec<usize>,
    vine_drinkable: Vec<f64>,
    /// A planned drink is waiting for [`settle_drink`].
    pub(crate) pending: bool,
    /// Under a plan, who the stand and vine indices in the wants were when it was made:
    /// each stand's `(site, id)` and each vine's id. The rest of the tick grows, kills and
    /// germinates, so an index is only a name through these.
    stand_ids: Vec<(Site, u64)>,
    vine_ids: Vec<u64>,
    /// The settle's cells that gave less than planned, `(voxel, got)`, ascending.
    short: Vec<(u32, f64)>,
    /// The settle's receipts, one per withdrawn voxel, ascending.
    receipts: Vec<f64>,
    /// The batched withdrawal's `(voxel, volume)` list, ascending, and what each gave.
    batch: Vec<(usize, f64)>,
    gave: Vec<f64>,
}

/// Cells per **band** of the drink's per-cell scratch: the wants are filed by band as they
/// are read, so the per-voxel totals and the plan split over bands, and a stand's shares
/// are credited band by band. A fixed partition, the same at every thread count; a
/// multiple of 64, so a band owns whole words of the touched bits.
const DRINK_BAND: usize = 1 << 14;

/// What one chunk of stands read out of its root boxes.
#[derive(Default)]
struct DrinkChunk {
    /// The chunk's own stands' [`Drink`]s, in stand order.
    out: Vec<Drink>,
    /// (voxel, stand, wanted) per [`DRINK_BAND`] of voxels, each in stand order.
    bands: Vec<Vec<(u32, u32, f64)>>,
    box_: Vec<usize>,
    drinkable: Vec<f64>,
}

impl DrinkChunk {
    /// Every want of the chunk, band by band.
    fn wants(&self) -> impl Iterator<Item = &(u32, u32, f64)> {
        self.bands.iter().flatten()
    }
}

/// `f(band, its cells, its touched words)` for every [`DRINK_BAND`] of the scratch, split
/// over up to `threads` workers of the compute pool (the `parallel` feature). The bands are
/// disjoint, so the split is free.
fn for_bands<F>(cell: &mut [[f64; 2]], touched: &mut [u64], threads: usize, f: F)
where
    F: Fn(usize, &mut [[f64; 2]], &mut [u64]) + Sync,
{
    if threads <= 1 {
        for (b, (c, t)) in cell
            .chunks_mut(DRINK_BAND)
            .zip(touched.chunks_mut(DRINK_BAND / 64))
            .enumerate()
        {
            f(b, c, t);
        }
        return;
    }
    let bands: Vec<(usize, &mut [[f64; 2]], &mut [u64])> = cell
        .chunks_mut(DRINK_BAND)
        .zip(touched.chunks_mut(DRINK_BAND / 64))
        .enumerate()
        .map(|(b, (c, t))| (b, c, t))
        .collect();
    #[cfg(feature = "parallel")]
    if threads > 1 && bands.len() > 1 {
        let pool = bevy_tasks::ComputeTaskPool::get_or_init(|| {
            bevy_tasks::TaskPoolBuilder::new()
                .num_threads(threads)
                .build()
        });
        // More groups than workers, so one dense band does not hold the rest up.
        let groups = (4 * threads).min(bands.len());
        let per = bands.len().div_ceil(groups);
        let f = &f;
        let mut bands = bands.into_iter();
        pool.scope(|scope| {
            loop {
                let group: Vec<_> = bands.by_ref().take(per).collect();
                if group.is_empty() {
                    break;
                }
                scope.spawn(async move {
                    for (b, c, t) in group {
                        f(b, c, t);
                    }
                });
            }
        });
        return;
    }
    let _ = threads;
    for (b, c, t) in bands {
        f(b, c, t);
    }
}

impl DrinkScratch {
    fn fit(&mut self, cells: usize) {
        if self.cell.len() != cells {
            self.cell = vec![[0.0; 2]; cells];
            self.touched = vec![0; cells.div_ceil(64)];
        }
    }
}

/// One root box read once for everything [`drink`] needs from it, a stand's or a
/// latticevine's: the box's soil voxels into `box_`, each one's [`drinkable_m3`] into
/// `drinkable`, and the `(water, band, saturated)` sums [`mean_available`] and
/// [`saturated_fraction`] take over the same box (a cell counts as saturated at
/// `saturated_pore` and above) — the same arithmetic in the same order, one pass instead
/// of four.
pub(crate) fn read_root_box(
    view: &VoxelView<'_>,
    site: Site,
    rooting_depth: u32,
    rooting_radius: u32,
    saturated_pore: f64,
    box_: &mut Vec<usize>,
    drinkable: &mut Vec<f64>,
) -> (f64, f64, usize) {
    box_.clear();
    drinkable.clear();
    #[cfg(feature = "profile")]
    cubarium_voxel::profile::add(
        cubarium_voxel::profile::Count::BoxVoxels,
        u64::from(rooting_depth.min(site.y + 1))
            * u64::from(2 * rooting_radius + 1)
            * u64::from(2 * rooting_radius + 1),
    );
    let (mut water, mut band, mut saturated) = (0.0, 0.0, 0usize);
    let c = view.config;
    let span = rooting_depth.min(site.y + 1);
    if span == 0 {
        return (water, band, saturated);
    }
    // A root box holds soil cells only, so every cell's material constants are soil's:
    // the same numbers the per-cell reads took, looked up once.
    let m = Material::Soil;
    let mats = &c.materials;
    let (wp, cap) = (mats.wilting_point(m), mats.pore_capacity(m));
    let w = (mats.field_capacity(m) - wp).max(0.0) * cap;
    let volume = c.voxel_volume();
    // [`root_box_dims_into`]'s walk, reading each soil cell as it is found: the same
    // cells in the same order, one pass instead of two.
    let y_lo = site.y + 1 - span;
    let r = rooting_radius as i64;
    let width = c.width as usize;
    let x_west = (site.x as i64 - r).rem_euclid(c.width as i64) as usize;
    let row_len = (2 * r + 1) as usize;
    for dz in -r..=r {
        let z = site.z as i64 + dz;
        if z < 0 || z >= c.depth as i64 {
            continue;
        }
        for y in y_lo..=site.y {
            let row = (y as usize * c.depth as usize + z as usize) * width;
            let mut x = x_west;
            for _ in 0..row_len {
                let i = row + x;
                if view.material[i] == Material::Soil {
                    let pore = view.pore[i];
                    water += available_water(mats, m, pore) * w;
                    band += w;
                    if pore >= saturated_pore {
                        saturated += 1;
                    }
                    box_.push(i);
                    drinkable.push((pore - wp).max(0.0) * cap * volume);
                }
                x += 1;
                if x == width {
                    x = 0;
                }
            }
        }
    }
    (water, band, saturated)
}

/// Step 5's read for stands `range`: each one's moisture, saturation and demand, and its
/// demand shared over its box into `chunk.bands`.
fn drink_read(
    flora: &Flora,
    sky: &[(Site, f64)],
    view: &VoxelView<'_>,
    range: std::ops::Range<usize>,
    chunk: &mut DrinkChunk,
    dt: f64,
) {
    let DrinkChunk {
        out,
        bands,
        box_,
        drinkable,
    } = chunk;
    out.clear();
    let nb = view.config.cells().div_ceil(DRINK_BAND);
    bands.resize_with(nb, Vec::new);
    for b in bands.iter_mut() {
        b.clear();
    }
    for i in range {
        let stand = &flora.stands[i];
        let sc = flora.config.species(stand.species);
        let mut d = Drink::default();
        let (water, band, saturated) = read_root_box(
            view,
            stand.site,
            sc.rooting_depth,
            sc.rooting_radius,
            sc.saturated_pore,
            box_,
            drinkable,
        );
        // `moisture_of`: the species' ramp over the box's mean available water, and an
        // empty box is a wilting one — unless it is dark and the species reads the cave
        // damp ([`SpeciesConfig::damp_available`]). The sky is the tick's memo, which the
        // light pass has filled for every stand.
        if sc.dark_sky_max > 0.0 {
            let site = stand.site;
            let sky = match sky.binary_search_by_key(&site, |e| e.0) {
                Ok(k) => sky[k].1,
                Err(_) => view.sky_visibility(site.x as i64, site.y, site.z),
            };
            d.darkness = sc.darkness(sky);
        }
        let mean = (band > 0.0).then(|| water / band);
        let available = sc.damp_available(mean, d.darkness);
        d.available = available.unwrap_or(0.0);
        d.moisture = available.map_or(0.0, |a| sc.moisture_at(a));
        // A species that needs standing water and has lost it reads
        // `μ = 0`: it earns nothing and pays its upkeep out of reserve.
        if sc.water_depth_min_m > 0.0
            && standing_water_beside(view, stand.site) < sc.water_depth_min_m
        {
            d.moisture = 0.0;
        }
        // `saturated_fraction`: an empty box is not waterlogged.
        d.saturated = if box_.is_empty() {
            0.0
        } else {
            saturated as f64 / box_.len() as f64
        };
        out.push(d);
        let demand = (sc.transpiration_m3_per_s * stand.foliage * d.moisture * dt).max(0.0);
        share_over_box(demand, box_, drinkable, |v, share| {
            bands[v / DRINK_BAND].push((v as u32, i as u32, share));
        });
    }
}

/// A root's `demand` shared over its box in proportion to what each cell holds **above
/// its wilting point** — water under it is held too tightly for a root — as
/// `push(voxel, share)`. A stand's and a vine's one rule.
pub(crate) fn share_over_box(
    demand: f64,
    box_: &[usize],
    drinkable: &[f64],
    mut push: impl FnMut(usize, f64),
) {
    if demand <= 0.0 || box_.is_empty() {
        return;
    }
    let stock: f64 = drinkable.iter().sum();
    if stock <= 0.0 {
        return;
    }
    for (&v, &d) in box_.iter().zip(drinkable.iter()) {
        let share = demand * d / stock;
        if share > 0.0 {
            push(v, share);
        }
    }
}

/// **The drink's reads**: every stand's demand per voxel (with the vines' from the step's
/// prep), and each voxel's bound. [`drink_settle`] then withdraws once per voxel (or holds
/// the plan) and splits what the core accepted proportional to demand.
///
/// The demands are summed per voxel into a dense per-cell scratch rather than sorted by
/// voxel: generated stand by stand, each voxel's total adds its demands in ascending stand
/// order — the order the sort handed them over in — and the withdrawals walk the touched
/// voxels in ascending index order, as before, so every total, every withdrawal and every
/// ledger sum is the sorted sweep's to the bit. Only the order a stand's own shares are
/// added into its observational `taken_m3` differs (`the_scratch_drink_is_the_sorted_sweep`).
fn drink_reads(
    flora: &Flora,
    sky: &[(Site, f64)],
    world: &World,
    threads: usize,
    scratch: &mut DrinkScratch,
    dt: f64,
) {
    debug_assert!(!scratch.pending, "a planned drink was never settled");
    scratch.fit(world.config().cells());
    let DrinkScratch {
        cell,
        touched,
        chunks,
        used,
        vine_wants,
        ..
    } = scratch;
    let n = flora.stands.len();
    let view = world.view();
    for_chunks(n, threads, chunks, DrinkChunk::default, |range, chunk| {
        drink_read(flora, sky, &view, range, chunk, dt)
    });
    *used = chunk_count(n, threads);
    // Each voxel's total, summed in ascending stand order: the chunks are in stand order,
    // and every want on a voxel is in that voxel's band. Band by band, over threads.
    {
        let chunks = &chunks[..*used];
        for_bands(cell, touched, threads, |b, cell, touched| {
            let base = b * DRINK_BAND;
            for chunk in chunks {
                for &(v, _, share) in &chunk.bands[b] {
                    let v = v as usize - base;
                    touched[v / 64] |= 1 << (v % 64);
                    cell[v][0] += share;
                }
            }
        });
    }
    // The vines' wants, read in the step's prep.
    for &(v, _, share) in vine_wants.iter() {
        let v = v as usize;
        touched[v / 64] |= 1 << (v % 64);
        cell[v][0] += share;
    }

    // Each voxel's bound, band by band over threads, off the pre-withdrawal read: never
    // below the wilting point, however many stands asked. Against a read copy that bound
    // is the plan; against the live world it is then withdrawn in one batch
    // ([`drink_settle`]), the voxel's first and only withdrawal this step, in ascending
    // voxel order.
    for_bands(cell, touched, threads, |b, cell, touched| {
        let base = b * DRINK_BAND;
        for (word_at, word) in touched.iter().enumerate() {
            let mut bits = *word;
            while bits != 0 {
                let local = word_at * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let volume = cell[local][0].min(drinkable_m3(&view, base + local));
                cell[local][1] = volume.max(0.0);
            }
        }
    });
}

/// The drink after [`drink_reads`]: withdrawn from the live world at once (the chained
/// tick) or held as the plan (the overlapped tick), and what each voxel gave split among
/// its demanders proportional to demand. Returns every stand's [`Drink`], in stand order.
fn drink_settle(
    flora: &mut Flora,
    target: &mut Target<'_>,
    threads: usize,
    scratch: &mut DrinkScratch,
) -> Vec<Drink> {
    let DrinkScratch {
        cell,
        touched,
        chunks,
        used,
        vine_wants,
        pending,
        stand_ids,
        vine_ids,
        batch,
        gave,
        ..
    } = scratch;
    let n = flora.stands.len();
    if let Target::Now(world) = target {
        batch.clear();
        for (word_at, word) in touched.iter().enumerate() {
            let mut bits = *word;
            while bits != 0 {
                let voxel = word_at * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                if cell[voxel][1] > 0.0 {
                    batch.push((voxel, cell[voxel][1]));
                }
            }
        }
        world.withdraw_pores(batch, gave);
        for (&(voxel, _), &got) in batch.iter().zip(gave.iter()) {
            flora.ledger.transpired_m3 += got;
            cell[voxel][1] = got;
        }
    }
    {
        // The same split as the read, so each chunk's `out` is its own stands'.
        let cell = &cell[..];
        for_chunks(n, threads, chunks, DrinkChunk::default, |range, chunk| {
            let first = range.start;
            let DrinkChunk { out, bands, .. } = chunk;
            for &(v, stand, want) in bands.iter().flatten() {
                let [total, got] = cell[v as usize];
                out[stand as usize - first].taken_m3 += split_proportional(got, want, total);
            }
        });
    }
    for &(v, vine, want) in vine_wants.iter() {
        let [total, got] = cell[v as usize];
        crate::cover::drink_credit(flora, vine as usize, split_proportional(got, want, total));
    }
    crate::cover::drink_done(flora);
    let mut out = Vec::with_capacity(n);
    for chunk in chunks.iter().take(*used) {
        out.extend_from_slice(&chunk.out);
    }
    match target {
        // Leave the scratch all zero for the next call.
        Target::Now(_) => clear_cells(cell, touched),
        // Hold the plan, and who it was made for, until the settle.
        Target::Planned(_) => {
            *pending = true;
            stand_ids.clear();
            stand_ids.extend(flora.stands.iter().map(|s| (s.site, s.id)));
            vine_ids.clear();
            vine_ids.extend(crate::cover::vine_ids(flora));
        }
    }
    out
}

/// The whole drink in one call, as the step's parts run it: the vines' wants, the reads
/// and the settle. The unit tests' handle on the phase.
#[cfg(test)]
fn drink(flora: &mut Flora, target: &mut Target<'_>, threads: usize, dt: f64) -> Vec<Drink> {
    let mut scratch = std::mem::take(&mut flora.drink.0);
    {
        let view = target.world().view();
        let DrinkScratch {
            vine_box,
            vine_drinkable,
            vine_wants,
            ..
        } = &mut scratch;
        crate::cover::drink_wants(flora, &view, vine_box, vine_drinkable, vine_wants, dt);
    }
    let sky = std::mem::take(&mut flora.sky);
    drink_reads(flora, &sky, target.world(), threads, &mut scratch, dt);
    flora.sky = sky;
    let out = drink_settle(flora, target, threads, &mut scratch);
    flora.drink.0 = scratch;
    out
}

/// Zero every touched cell and the touched bits.
fn clear_cells(cell: &mut [[f64; 2]], touched: &mut [u64]) {
    for (word_at, word) in touched.iter_mut().enumerate() {
        let mut bits = *word;
        while bits != 0 {
            cell[word_at * 64 + bits.trailing_zeros() as usize] = [0.0; 2];
            bits &= bits - 1;
        }
        *word = 0;
    }
}

/// [`crate::Flora::settle_drink`]: apply a planned drink to the live world, all three steps
/// on the calling thread.
pub(crate) fn settle_drink(flora: &mut Flora, world: &mut World) {
    if let Some(mut plan) = take_plan(flora) {
        plan.withdraw(world);
        finish(flora, plan);
    }
}

/// The planned drink, out of the layer; `None` when nothing is planned.
pub(crate) fn take_plan(flora: &mut Flora) -> Option<DrinkPlan> {
    if !flora.drink.0.pending {
        return None;
    }
    Some(DrinkPlan(std::mem::take(&mut flora.drink.0)))
}

/// A drink [`crate::Flora::step_planned`] planned against a read copy, out of the layer
/// ([`crate::Flora::take_drink_plan`]) so the live world can take it
/// ([`DrinkPlan::withdraw`]) while the rest of the plant and animal leg still runs, and
/// then handed back ([`crate::Flora::finish_drink`]) to book it. Holds the drink's scratch,
/// which goes back to the layer with it.
pub struct DrinkPlan(DrinkScratch);

impl std::fmt::Debug for DrinkPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DrinkPlan")
    }
}

impl DrinkPlan {
    /// The world's half: one `WithdrawPore` per planned voxel, in ascending voxel order —
    /// the order the chained tick withdraws in — each bounded by what the cell holds above
    /// its wilting point **now**. Books the world's `transpiration_out` (and, under a
    /// closed budget, the atmosphere) as each command does, and keeps every receipt for
    /// [`crate::Flora::finish_drink`]. Touches no plant.
    pub fn withdraw(&mut self, world: &mut World) {
        let DrinkScratch {
            cell,
            touched,
            receipts,
            short,
            batch,
            gave,
            ..
        } = &mut self.0;
        receipts.clear();
        short.clear();
        // Every planned voxel's bound against the live world now, then one batched
        // withdrawal in ascending voxel order.
        batch.clear();
        {
            let view = world.view();
            for (word_at, word) in touched.iter().enumerate() {
                let mut bits = *word;
                while bits != 0 {
                    let voxel = word_at * 64 + bits.trailing_zeros() as usize;
                    bits &= bits - 1;
                    let planned = cell[voxel][1];
                    if !(planned > 0.0) {
                        continue;
                    }
                    batch.push((voxel, planned.min(drinkable_m3(&view, voxel)).max(0.0)));
                }
            }
        }
        world.withdraw_pores(batch, gave);
        for (&(voxel, volume), &got) in batch.iter().zip(gave.iter()) {
            receipts.push(got);
            // Short only where the water left less than the plan: a receipt a rounding
            // below its request is the chained tick's receipt too.
            if volume < cell[voxel][1] {
                short.push((voxel as u32, got));
            }
        }
    }
}

/// [`crate::Flora::finish_drink`]: book what the world gave and debit the shortfalls.
pub(crate) fn finish(flora: &mut Flora, plan: DrinkPlan) {
    let mut scratch = plan.0;
    let DrinkScratch {
        cell,
        touched,
        chunks,
        used,
        vine_wants,
        pending,
        stand_ids,
        vine_ids,
        short,
        receipts,
        ..
    } = &mut scratch;
    *pending = false;
    // The receipts in the world's own order, so the two ledgers add the same numbers in
    // the same sequence and agree to the bit.
    for &got in receipts.iter() {
        flora.ledger.transpired_m3 += got;
    }
    receipts.clear();
    // A shortfall comes off whoever asked, by the same split that credited them.
    if !short.is_empty() {
        let debit = |v: u32, want: f64| -> Option<f64> {
            let k = short.binary_search_by_key(&v, |p| p.0).ok()?;
            let [total, planned] = cell[v as usize];
            let got = short[k].1;
            Some(split_proportional(planned, want, total) - split_proportional(got, want, total))
        };
        for chunk in chunks.iter().take(*used) {
            for &(v, stand, want) in chunk.wants() {
                let Some(d) = debit(v, want) else { continue };
                let (site, id) = stand_ids[stand as usize];
                // The stand by id among the site's (a plant and a fungus may share it).
                if let Some(j) = crate::on_site(&flora.stands, site).find(|&j| flora.stands[j].id == id) {
                    let s = &mut flora.stands[j];
                    s.water_m3 = (s.water_m3 - d).max(0.0);
                }
            }
        }
        for &(v, vine, want) in vine_wants.iter() {
            let Some(d) = debit(v, want) else { continue };
            crate::cover::drink_debit(flora, vine_ids[vine as usize], d);
        }
        short.clear();
    }
    clear_cells(cell, touched);
    flora.drink.0 = scratch;
}

/// One demander's share of what the core accepted. A zero total means a zero share:
/// never `0/0`.
pub(crate) fn split_proportional(accepted: f64, want: f64, total: f64) -> f64 {
    if total <= 0.0 || want <= 0.0 || accepted <= 0.0 {
        return 0.0;
    }
    if want >= total {
        return accepted;
    }
    accepted * want / total
}

/// The soil voxels of a site's root box, as flat voxel indices: `|dx| <=
/// rooting_radius`, `|dz| <= rooting_radius`, `support.y - rooting_depth < y <=
/// support.y`, `x` wrapped and `z` clipped at the walls. Membership is per voxel, not
/// contiguous: a soil pocket under a stratum is in the box if it is in the box.
fn root_box(view: &VoxelView<'_>, site: Site, sc: &SpeciesConfig) -> Vec<usize> {
    let mut out = Vec::new();
    root_box_into(view, site, sc, &mut out);
    out
}

/// [`root_box`] into a buffer the caller reuses, which is cleared first.
fn root_box_into(view: &VoxelView<'_>, site: Site, sc: &SpeciesConfig, out: &mut Vec<usize>) {
    root_box_dims_into(view, site, sc.rooting_depth, sc.rooting_radius, out);
}

/// [`root_box_into`] for a box given by its own depth and radius.
fn root_box_dims_into(
    view: &VoxelView<'_>,
    site: Site,
    rooting_depth: u32,
    rooting_radius: u32,
    out: &mut Vec<usize>,
) {
    out.clear();
    #[cfg(feature = "profile")]
    cubarium_voxel::profile::add(
        cubarium_voxel::profile::Count::BoxVoxels,
        u64::from(rooting_depth.min(site.y + 1))
            * u64::from(2 * rooting_radius + 1)
            * u64::from(2 * rooting_radius + 1),
    );
    let c = view.config;
    let span = rooting_depth.min(site.y + 1);
    if span == 0 {
        return;
    }
    let y_lo = site.y + 1 - span;
    let r = rooting_radius as i64;
    // `c.index`'s own arithmetic with the `x` wrap taken once per box rather than once per
    // voxel: row `(y, z)` starts at `(y · depth + z) · width`, and `x` steps from the box's
    // wrapped west edge, wrapping at the seam. Same voxels, same order.
    let width = c.width as usize;
    let x_west = (site.x as i64 - r).rem_euclid(c.width as i64) as usize;
    let row_len = (2 * r + 1) as usize;
    for dz in -r..=r {
        let z = site.z as i64 + dz;
        if z < 0 || z >= c.depth as i64 {
            continue;
        }
        for y in y_lo..=site.y {
            let row = (y as usize * c.depth as usize + z as usize) * width;
            let mut x = x_west;
            for _ in 0..row_len {
                let i = row + x;
                if view.material[i] == Material::Soil {
                    out.push(i);
                }
                x += 1;
                if x == width {
                    x = 0;
                }
            }
        }
    }
}

/// What one cell of ground offers a root, on the scale `μ` and the establishment water
/// gate read:
/// `a = (p − wp)/(fc − wp)` with `p` the cell's pore fraction and `wp`, `fc` its **own**
/// material's wilting point and field capacity, floored at zero.
///
/// `a` is 0 at the wilting point, 1 at field capacity — what drained ground holds — and
/// `(1 − wp)/(fc − wp)` at saturation. A material with no band between the two (air,
/// bedrock) offers nothing. `materials` is the world's own (`config.materials`).
pub fn available_water(materials: &Materials, material: Material, pore: f64) -> f64 {
    let (wp, fc) = (
        materials.wilting_point(material),
        materials.field_capacity(material),
    );
    if fc <= wp {
        return 0.0;
    }
    ((pore - wp) / (fc - wp)).max(0.0)
}

/// Cubic metres of cell `i`'s pore water above its wilting point: what a root may drink.
fn drinkable_m3(view: &VoxelView<'_>, i: usize) -> f64 {
    let (m, mats) = (view.material[i], &view.config.materials);
    (view.pore[i] - mats.wilting_point(m)).max(0.0)
        * mats.pore_capacity(m)
        * view.config.voxel_volume()
}

/// The mean [`available_water`] of a root box, `None` for a box with no plant-available
/// band in it at all. Each cell is weighted by the volume of its band,
/// `(fc − wp) · pore_capacity`, so the mean is the drinkable water the box holds over what
/// it would hold at field capacity: a box that mixes materials reads as the water it can
/// actually offer.
fn mean_available(view: &VoxelView<'_>, box_: &[usize]) -> Option<f64> {
    let mut water = 0.0;
    let mut band = 0.0;
    let mats = &view.config.materials;
    for &i in box_ {
        let m = view.material[i];
        let w = (mats.field_capacity(m) - mats.wilting_point(m)).max(0.0) * mats.pore_capacity(m);
        water += available_water(mats, m, view.pore[i]) * w;
        band += w;
    }
    if band <= 0.0 {
        None
    } else {
        Some(water / band)
    }
}

/// The fraction of a root box's voxels at or above the species' `saturated_pore`: how
/// much of the root zone has no air left in it. Counted per voxel, not weighted by
/// capacity, because what a root needs is somewhere to breathe and not a volume of it.
///
/// A box with no voxels at all is **not** waterlogged: there is no soil there to hold
/// water, and that stand's problem is `μ`, which already reads an empty box as wilting.
fn saturated_fraction(view: &VoxelView<'_>, box_: &[usize], sc: &SpeciesConfig) -> f64 {
    if box_.is_empty() {
        return 0.0;
    }
    let n = box_
        .iter()
        .filter(|&&i| view.pore[i] >= sc.saturated_pore)
        .count();
    n as f64 / box_.len() as f64
}

/// The aeration stress a root box at saturated fraction `f` asks for: zero at and below
/// the species' `establish_saturated_max`, one at a wholly saturated box, linear between.
///
/// The tolerance is `establish_saturated_max` and not a knob of its own, because a site a
/// species may germinate on is a site it does not stress on. A species whose ceiling is at
/// or above 1 — umbrellafrond's placeholder is exactly 1.0 — asks for no saturation stress
/// anywhere, and its drowning path is `drown_depth_m` alone.
///
/// This is a **target**, not an increment: the stress relaxes toward it first-order, so a
/// box held half saturated settles at a stress strictly inside `0..1` and the two rates
/// say how fast it gets there rather than which boundary it ramps to.
fn aeration_target(saturated: f64, sc: &SpeciesConfig) -> f64 {
    let tol = sc.establish_saturated_max;
    if tol >= 1.0 {
        return 0.0;
    }
    ((saturated - tol) / (1.0 - tol)).clamp(0.0, 1.0)
}

/// `μ`: the species' ramp ([`SpeciesConfig::moisture_at`]) over the root box's mean
/// available water, between `wilt_water` and `full_water`. An empty box is a wilting one.
/// [`drink`] reads the same thing in one pass with the rest of its box ([`read_root_box`]);
/// this is the reference its test holds it to.
#[cfg(test)]
fn moisture_of(view: &VoxelView<'_>, box_: &[usize], sc: &SpeciesConfig) -> f64 {
    match mean_available(view, box_) {
        None => 0.0,
        Some(mean) => sc.moisture_at(mean),
    }
}

/// Zero at or below `lo`, one at or above `hi`, linear between. A `hi` at or below `lo`
/// is a step at `hi`.
pub(crate) fn ramp(value: f64, lo: f64, hi: f64) -> f64 {
    if hi <= lo {
        return if value >= hi { 1.0 } else { 0.0 };
    }
    ((value - lo) / (hi - lo)).clamp(0.0, 1.0)
}

// --------------------------------------------------------------- 5b. substrate

/// A mycelium box's extent ([`mycelium_sites_into`]'s): its rows `y_lo..=y_hi` and its
/// half-width `r`, and whether a column can repeat (a box wider than the ring).
#[derive(Clone, Copy)]
struct MyceliumBox {
    site: Site,
    r: i64,
    y_lo: u32,
    y_hi: u32,
    width: i64,
    depth: i64,
}

impl MyceliumBox {
    fn of(view: &VoxelView<'_>, site: Site, sc: &SpeciesConfig) -> MyceliumBox {
        let c = view.config;
        let up_down = i64::from(sc.substrate_reach_up_down);
        MyceliumBox {
            site,
            r: sc.rooting_radius as i64,
            y_lo: (i64::from(site.y) - up_down).max(0) as u32,
            y_hi: (i64::from(site.y) + up_down).min(i64::from(c.height.max(1) - 1)) as u32,
            width: c.width.max(1) as i64,
            depth: c.depth as i64,
        }
    }

    /// Whether a column can be met twice: then the walks below do not hold, and the site
    /// list does.
    fn may_repeat(&self) -> bool {
        2 * self.r + 1 > self.width
    }

    /// The position of `(dx, y, dz)` in [`mycelium_sites_into`]'s order (`dz`, then `y`,
    /// then `dx`).
    fn order(&self, dx: i64, y: u32, dz: i64) -> u64 {
        let side = (2 * self.r + 1) as u64;
        let rows = u64::from(self.y_hi - self.y_lo + 1);
        (((dz + self.r) as u64 * rows) + u64::from(y - self.y_lo)) * side + (dx + self.r) as u64
    }

    /// Every ground entry in the box, `(order, ground index)` sorted in the site list's
    /// order, found by one search per column of `x` in the site-sorted ground instead of
    /// one per site. Not for a box that [`MyceliumBox::may_repeat`].
    fn ground_sorted(&self, ground: &[Ground], out: &mut Vec<(u64, usize)>) {
        out.clear();
        let (z_lo, z_hi) = (i64::from(self.site.z) - self.r, i64::from(self.site.z) + self.r);
        for dx in -self.r..=self.r {
            let x = (i64::from(self.site.x) + dx).rem_euclid(self.width) as u32;
            let from = ground.partition_point(|g| g.site < Site { x, y: self.y_lo, z: 0 });
            for (gi, g) in ground.iter().enumerate().skip(from) {
                if g.site.x != x || g.site.y > self.y_hi {
                    break;
                }
                let z = i64::from(g.site.z);
                if z >= z_lo && z <= z_hi && z < self.depth {
                    out.push((self.order(dx, g.site.y, z - i64::from(self.site.z)), gi));
                }
            }
        }
        out.sort_unstable_by_key(|e| e.0);
    }

    /// [`MyceliumBox::ground_sorted`] through a [`GroundIndex`]: one short column per
    /// `(x, z)` of the box.
    fn ground_indexed(&self, index: &GroundIndex, out: &mut Vec<(u64, usize)>) {
        out.clear();
        for dz in -self.r..=self.r {
            let z = i64::from(self.site.z) + dz;
            if z < 0 || z >= self.depth {
                continue;
            }
            for dx in -self.r..=self.r {
                let x = (i64::from(self.site.x) + dx).rem_euclid(self.width) as usize;
                for &(y, gi) in index.column(x, z as usize) {
                    if y >= self.y_lo && y <= self.y_hi {
                        out.push((self.order(dx, y, dz), gi as usize));
                    }
                }
            }
        }
        out.sort_unstable_by_key(|e| e.0);
    }
}

/// The **mycelium box** of a saprotroph on `site`: `|dx| <= rooting_radius`, `|dz| <=
/// rooting_radius`, `|dy| <= `[`SpeciesConfig::substrate_reach_up_down`], `x` wrapped and
/// `z` and `y` clipped at the walls, read as support **sites** rather than as soil voxels,
/// in a fixed geometric order, deduplicated.
///
/// Sites, not voxels, because what a fungus eats is a *stock on the ground* and the ground
/// stocks live one per support face. A site in the box that has never held anything has no
/// [`Ground`] and therefore no dead wood, which is the same thing as holding none.
///
/// **The vertical reach is its own field and symmetric**, unlike [`root_box`], which
/// reaches `rooting_depth` **down** and never up: mycelium in a log is not a root in soil,
/// so a spore one voxel above or below a log still finds it. `substrate_reach_up_down`
/// decides it and the soil-water box is separate.
///
/// Still a box and not a path: a log across a one-voxel wall is in reach of a mycelium that
/// could not actually grow through it, exactly as [`crate::Reach`] is a box with no line of
/// sight in it.
fn mycelium_sites(view: &VoxelView<'_>, site: Site, sc: &SpeciesConfig) -> Vec<Site> {
    let mut out = Vec::new();
    mycelium_sites_into(view, site, sc, &mut out);
    out
}

/// [`mycelium_sites`] into a buffer the caller keeps: the same sites in the same order.
fn mycelium_sites_into(view: &VoxelView<'_>, site: Site, sc: &SpeciesConfig, out: &mut Vec<Site>) {
    out.clear();
    let c = view.config;
    let up_down = i64::from(sc.substrate_reach_up_down);
    let y_lo = (i64::from(site.y) - up_down).max(0) as u32;
    let y_hi = (i64::from(site.y) + up_down).min(i64::from(c.height.max(1) - 1)) as u32;
    let r = sc.rooting_radius as i64;
    let width = c.width.max(1) as i64;
    // Only a box wider than the world can meet a column twice (the `x` wrap); otherwise
    // every site generated is new and the dedup scan is skipped.
    let may_repeat = 2 * r + 1 > width;
    for dz in -r..=r {
        let z = site.z as i64 + dz;
        if z < 0 || z >= c.depth as i64 {
            continue;
        }
        for y in y_lo..=y_hi {
            for dx in -r..=r {
                let x = (site.x as i64 + dx).rem_euclid(width) as u32;
                let s = Site { x, y, z: z as u32 };
                if !may_repeat || !out.contains(&s) {
                    out.push(s);
                }
            }
        }
    }
}

/// The **substrate** a saprotroph's mycelium box holds — dead wood **and litter**, as the
/// two numbers it is the sum of — which is what `establish_substrate_min` is compared
/// against and what step 5b draws on. **`(0.0, 0.0)` for a [`Trophic::Photo`] species**,
/// whose substrate gate is open whatever the ground holds — the box walk is skipped
/// entirely for the five plants.
///
/// Both pools, because dead organic matter is dead organic matter to a fungus: a log and
/// the leaf litter around it are the same food, differing in how fast they decompose on
/// their own, and a decomposer that could only eat trunks starved on a floor deep in
/// shed foliage. Carrion is **not** in the sum: a corpse is a consumer's own pool and
/// out of this round's scope.
pub(crate) fn substrate_pools_in_box(
    view: &VoxelView<'_>,
    ground: &[Ground],
    site: Site,
    sc: &SpeciesConfig,
) -> (f64, f64) {
    if sc.trophic != Trophic::Saprotroph {
        return (0.0, 0.0);
    }
    let mut dead_wood = 0.0;
    let mut litter = 0.0;
    let b = MyceliumBox::of(view, site, sc);
    if b.may_repeat() {
        for gi in mycelium_sites(view, site, sc)
            .into_iter()
            .filter_map(|s| ground.binary_search_by_key(&s, |g| g.site).ok())
        {
            dead_wood += ground[gi].dead_wood;
            litter += ground[gi].litter;
        }
        return (dead_wood, litter);
    }
    // The same entries summed in the same order as the site list's, found by column.
    let mut found = Vec::new();
    b.ground_sorted(ground, &mut found);
    for &(_, gi) in &found {
        dead_wood += ground[gi].dead_wood;
        litter += ground[gi].litter;
    }
    (dead_wood, litter)
}

/// [`substrate_pools_in_box`] summed: the one number the gate and the income rule read.
pub(crate) fn substrate_in_box(
    view: &VoxelView<'_>,
    ground: &[Ground],
    site: Site,
    sc: &SpeciesConfig,
) -> f64 {
    let (dead_wood, litter) = substrate_pools_in_box(view, ground, site, sc);
    dead_wood + litter
}

/// Step 5b: what every saprotroph took out of the **dead wood and the litter** this tick,
/// in `stands` order, zero for every [`Trophic::Photo`] stand.
///
/// **[`drink`]'s rule, on both dead pools.** Each saprotroph asks for
/// `substrate_uptake_per_s · W · μ · dt`, split across the pools of its own mycelium box
/// pro rata by what each holds; then every pool is drawn on **once** for the total asked of
/// it, through the same [`crate::take_pool`] a consumer's `take_dead_wood` and
/// `take_litter` use, and what it gave up is shared among its demanders proportional to
/// demand. Collect-then-withdraw is the model's rule for a shared bounded stock, and it is
/// the reason two fungi on one log share it instead of the earlier site in the sweep
/// eating its fill first.
///
/// **A site holds two pools, and each is a pool in that split.** The dead wood and the
/// litter of one ground entry are two separate stocks in the pro-rata draw, exactly as
/// two sites' logs are: there is no preference between them, so a box holding twice as
/// much litter as wood gives up twice as much litter, and neither is drawn on before the
/// other. The withdrawal order is `(site, pool)`, with dead wood before litter at a site,
/// which no result depends on because every pool is drawn on once for its whole share.
///
/// **The yield is applied per pool, not to the sum** ([`yield_of`]). The two pools carry
/// their own mineral and their own energy per unit — litter's retained energy is capped
/// by `e_d_max` and a log's is not — and `take_pool` hands each over at its own density,
/// so what became tissue is computed from each withdrawal separately and summed. A
/// high-energy log therefore cannot pay for tissue built out of low-energy litter; the
/// [`Substrate::gross`] this returns is that sum, and [`grow`] spends it.
///
/// The uptake is bounded twice — by the rate and by the pools — so an empty log feeds
/// nothing and a pool can never go negative. It is **not** multiplied by
/// `1 − aeration_stress`: a saprotroph's only water term is `μ`. So a spore will not
/// start on a waterlogged log, while the mycelium already in one pays nothing.
///
/// Nothing is booked at the layer's boundary here. The material moves from a ground stock
/// to a stand, both inside this layer, so the only ledger term it touches is the
/// per-species diagnostic [`FloraLedger::substrate_uptake`]; the organic matter the fungus
/// does **not** keep is respired in [`grow`], where every other respiration is.
fn feed(flora: &mut Flora, world: &World, drink: &[Drink], threads: usize, dt: f64) -> Vec<Substrate> {
    let mut out = vec![Substrate::default(); flora.stands.len()];
    let upkeep = Climate::of(world, flora.config.upkeep_q10).upkeep;
    let view = world.view();
    let index = GroundIndex::build(&flora.ground, view.config.width, view.config.depth);
    // Every fungus's wants, gathered per chunk of stands in stand order: what each asks of
    // each pool of its box. A read of the ground alone, so the chunks split cleanly.
    let parts: Vec<Vec<Want>> = {
        let Flora {
            config,
            stands,
            ground,
            ..
        } = &*flora;
        let (index, view) = (&index, &view);
        for_chunks(
            stands.len(),
            threads,
            &mut Vec::new(),
            || (),
            |range, _| {
                let mut wants: Vec<Want> = Vec::new();
                let mut sites: Vec<Site> = Vec::new();
                let mut found: Vec<(u64, usize)> = Vec::new();
                let mut pools: Vec<(usize, Pool, f64)> = Vec::new();
                for si in range {
                    let stand = &stands[si];
                    let sc = config.species(stand.species);
                    if sc.trophic != Trophic::Saprotroph {
                        continue;
                    }
                    let raw = (sc.substrate_uptake_per_s * stand.wood * drink[si].moisture * dt).max(0.0);
                    // A satiated decomposer cannot remove its neighbours' litter simply
                    // to respire income it has nowhere to put. This is a demand rule,
                    // using the same upkeep, construction and reserve room as growth.
                    let spend = substrate_spend(sc, stand, upkeep, dt);
                    if !(raw > 0.0) {
                        continue;
                    }
                    pools.clear();
                    // The box's ground entries in the site list's order, found column by
                    // column (or through the site list, for a box wider than the ring).
                    let b = MyceliumBox::of(view, stand.site, sc);
                    found.clear();
                    if b.may_repeat() {
                        mycelium_sites_into(view, stand.site, sc, &mut sites);
                        found.extend(sites.iter().filter_map(|&s| index.get(s)).map(|gi| (0, gi)));
                    } else {
                        b.ground_indexed(index, &mut found);
                    }
                    for &(_, gi) in &found {
                        for pool in [Pool::DeadWood, Pool::Litter] {
                            let held = pool.held(&ground[gi]);
                            if held > 0.0 {
                                pools.push((gi, pool, held));
                            }
                        }
                    }
                    let total: f64 = pools.iter().map(|&(_, _, w)| w).sum();
                    if !(total > 0.0) {
                        continue;
                    }
                    let want = if sc.substrate_budgeted {
                        // Income per unit of the pro-rata mix, at each pool's own
                        // energy and mineral density, rather than its nominal yield.
                        let gross: f64 = pools.iter().map(|&(gi, pool, _)| {
                            let g = &ground[gi];
                            let (organic, mineral, energy) = match pool {
                                Pool::DeadWood => (g.dead_wood, g.dead_wood_mineral, g.dead_wood_energy),
                                Pool::Litter => (g.litter, g.litter_mineral, g.litter_energy),
                            };
                            yield_of(sc, Taken { organic, mineral, energy })
                        }).sum();
                        if gross > 0.0 { raw.min(spend * total / gross) } else { 0.0 }
                    } else { raw };
                    for &(gi, pool, held) in &pools {
                        let share = split_proportional(want, held, total);
                        if share > 0.0 {
                            wants.push(Want {
                                gi,
                                pool,
                                si,
                                want: share,
                            });
                        }
                    }
                }
                wants
            },
        )
    };
    // Filed by `(site, pool)`: a stable counting sort of wants generated in ascending
    // stand order is the sort by `(ground index, pool, stand)` — no stand asks one pool
    // twice — so the withdrawal order is the ground's own site order, dead wood before
    // litter at a site, and nothing depends on how the stands were reached or split.
    let wants = file_by_pool(&parts, flora.ground.len());
    let Flora {
        config,
        stands,
        ground,
        ledger,
        ..
    } = flora;

    // Reused across pools so a tick with many fungi allocates once.
    let mut shares: Vec<Taken> = Vec::new();
    let mut at = 0;
    while at < wants.len() {
        let (gi, pool) = (wants[at].gi, wants[at].pool);
        let mut end = at;
        let mut total = 0.0;
        while end < wants.len() && wants[end].gi == gi && wants[end].pool == pool {
            total += wants[end].want;
            end += 1;
        }
        // One bounded withdrawal for the whole pool, whatever asked for it.
        if let Some(taken) = pool.take(&mut ground[gi], total) {
            ledger.substrate_by_pool[match pool { Pool::DeadWood => 0, Pool::Litter => 1 }] += taken.organic;
            split_shares(taken, &wants[at..end], &mut shares);
            for (w, share) in wants[at..end].iter().zip(shares.iter().copied()) {
                let s = &mut out[w.si];
                s.taken.organic += share.organic;
                s.taken.mineral += share.mineral;
                s.taken.energy += share.energy;
                // Per pool, at that pool's own mineral and energy density.
                s.gross += yield_of(config.species(stands[w.si].species), share);
            }
        }
        at = end;
    }
    for (si, stand) in stands.iter().enumerate() {
        ledger.substrate_uptake[stand.species.index()] += out[si].taken.organic;
    }
    out
}

fn substrate_spend(sc: &SpeciesConfig, stand: &Stand, upkeep: f64, dt: f64) -> f64 {
    let w = stand.wood;
    let caps = (sc.alpha * w - stand.foliage).max(0.0).min(sc.foliage_rate * w * dt);
    let wood = (sc.wood_max - w).max(0.0).min(sc.wood_rate * w * dt);
    sc.maintenance * upkeep * w * dt + (1.0 + sc.build) * (caps + wood)
        + (sc.reserve_cap * w - stand.reserve).max(0.0)
}

/// [`feed`]'s wants, filed by `(ground index, pool)` and within one pool in the order the
/// parts hold them.
fn file_by_pool(parts: &[Vec<Want>], grounds: usize) -> Vec<Want> {
    let key = |w: &Want| {
        2 * w.gi
            + match w.pool {
                Pool::DeadWood => 0,
                Pool::Litter => 1,
            }
    };
    let n: usize = parts.iter().map(Vec::len).sum();
    let mut out = Vec::with_capacity(n);
    if n == 0 {
        return out;
    }
    let mut start = vec![0u32; 2 * grounds + 1];
    for w in parts.iter().flatten() {
        start[key(w) + 1] += 1;
    }
    for k in 0..2 * grounds {
        start[k + 1] += start[k];
    }
    let blank = Want {
        gi: 0,
        pool: Pool::DeadWood,
        si: 0,
        want: 0.0,
    };
    out.resize(n, blank);
    for w in parts.iter().flatten() {
        let k = key(w);
        out[start[k] as usize] = *w;
        start[k] += 1;
    }
    out
}

/// Every ground entry filed by its column, ascending `y` within a column (the ground is
/// site-sorted, and a stable counting sort keeps that): a site lookup reads one short
/// column instead of binary-searching the whole ground, which is what a mycelium box of a
/// hundred-odd sites per saprotroph was doing.
struct GroundIndex {
    width: usize,
    depth: usize,
    start: Vec<u32>,
    /// `(y, ground index)`.
    entries: Vec<(u32, u32)>,
}

impl GroundIndex {
    fn build(ground: &[Ground], width: u32, depth: u32) -> GroundIndex {
        let (width, depth) = (width as usize, depth as usize);
        let column = |site: Site| {
            let (x, z) = (site.x as usize, site.z as usize);
            (x < width && z < depth).then_some(z * width + x)
        };
        let mut start = vec![0u32; width * depth + 1];
        for g in ground {
            if let Some(c) = column(g.site) {
                start[c + 1] += 1;
            }
        }
        for c in 0..width * depth {
            start[c + 1] += start[c];
        }
        let mut fill = start.clone();
        let mut entries = vec![(0u32, 0u32); start[width * depth] as usize];
        for (gi, g) in ground.iter().enumerate() {
            if let Some(c) = column(g.site) {
                entries[fill[c] as usize] = (g.site.y, gi as u32);
                fill[c] += 1;
            }
        }
        GroundIndex {
            width,
            depth,
            start,
            entries,
        }
    }

    /// Column `(x, z)`'s entries, `(y, ground index)` ascending in `y`.
    fn column(&self, x: usize, z: usize) -> &[(u32, u32)] {
        if x >= self.width || z >= self.depth {
            return &[];
        }
        let c = z * self.width + x;
        &self.entries[self.start[c] as usize..self.start[c + 1] as usize]
    }

    /// The ground index of `site`, as `ground.binary_search_by_key(&site, ..).ok()`.
    fn get(&self, site: Site) -> Option<usize> {
        let (x, z) = (site.x as usize, site.z as usize);
        if x >= self.width || z >= self.depth {
            return None;
        }
        let c = z * self.width + x;
        self.entries[self.start[c] as usize..self.start[c + 1] as usize]
            .iter()
            .find(|e| e.0 == site.y)
            .map(|e| e.1 as usize)
    }
}

/// Which of a site's two dead pools a saprotroph's withdrawal reads. Ordered, because the
/// withdrawals are grouped by `(site, pool)` and the grouping needs a total order;
/// carrion is not a member, being out of a saprotroph's scope this round.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Pool {
    DeadWood,
    Litter,
}

impl Pool {
    fn held(self, g: &Ground) -> f64 {
        match self {
            Pool::DeadWood => g.dead_wood,
            Pool::Litter => g.litter,
        }
    }

    fn take(self, g: &mut Ground, want: f64) -> Option<Taken> {
        match self {
            Pool::DeadWood => crate::take_pool(
                &mut g.dead_wood,
                &mut g.dead_wood_mineral,
                &mut g.dead_wood_energy,
                want,
            ),
            Pool::Litter => crate::take_pool(
                &mut g.litter,
                &mut g.litter_mineral,
                &mut g.litter_energy,
                want,
            ),
        }
    }
}

/// One saprotroph's claim on one pool: the ground entry, which pool of it, the stand, and
/// what that stand asked of it this tick.
#[derive(Clone, Copy, Debug)]
struct Want {
    gi: usize,
    pool: Pool,
    si: usize,
    want: f64,
}

/// What step 5b hands [`grow`] for one stand: everything the withdrawals took, and the
/// part of it that may become tissue.
///
/// The two are separate because the yield is a **per-pool** rule (see [`feed`]): the
/// organic matter, the mineral and the energy sum over the pools drawn on, while the
/// tissue is `yield_of` each withdrawal, summed. With one pool they are the same
/// arithmetic they always were.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Substrate {
    pub taken: Taken,
    pub gross: f64,
}

/// The tissue one withdrawal may become: `substrate_yield` of its organic matter, bounded
/// by what its own energy pays for.
///
/// The `energy / e_v` bound is conservation and not a rule: the tissue this income becomes
/// holds `e_v` per unit, so building more of it than the substrate's own energy pays for
/// would create energy inside the system. At the placeholders it never binds on a declared
/// log — `e_v · yield` is 0.8 against a log's own 2.0 per unit — and it binds exactly when
/// a pool was laid with less energy in it than the tissue it would become: a log with
/// nothing in it to eat, or litter whose `e_d_max` cap threw energy away before the fungus
/// arrived.
fn yield_of(sc: &SpeciesConfig, taken: Taken) -> f64 {
    let e_v = sc.energy_density;
    (sc.substrate_yield.clamp(0.0, 1.0) * taken.organic)
        .min(if e_v > 0.0 {
            taken.energy / e_v
        } else {
            f64::INFINITY
        })
        .max(0.0)
        .min(taken.organic)
}

/// Split one pool's withdrawal among its demanders, proportional to demand in all three
/// currencies, with the **last** demander taking each remainder. The shares come back in
/// `wants` order, in `shares`, which is cleared first.
///
/// The remainder rule is what [`split_proportional`] alone cannot give: water's shares are
/// not in a ledger, and these are, so what the pool lost and what the stands received have
/// to agree to the bit rather than to a few ulps. With one demander — which is the common
/// case — it hands over the whole [`Taken`].
fn split_shares(taken: Taken, wants: &[Want], shares: &mut Vec<Taken>) {
    shares.clear();
    let total: f64 = wants.iter().map(|w| w.want).sum();
    let (mut o, mut m, mut e) = (taken.organic, taken.mineral, taken.energy);
    for (k, w) in wants.iter().enumerate() {
        let share = if k + 1 == wants.len() {
            Taken {
                organic: o.max(0.0),
                mineral: m.max(0.0),
                energy: e.max(0.0),
            }
        } else {
            Taken {
                organic: split_proportional(taken.organic, w.want, total).min(o.max(0.0)),
                mineral: split_proportional(taken.mineral, w.want, total).min(m.max(0.0)),
                energy: split_proportional(taken.energy, w.want, total).min(e.max(0.0)),
            }
        };
        o -= share.organic;
        m -= share.mineral;
        e -= share.energy;
        shares.push(share);
    }
}

// ------------------------------------------- 6. income, growth, senescence, death

/// Multiplier on plant maintenance at `t_c` in a climate whose mean is `t_mean_c`, with
/// `q10` the factor per 10 °C ([`FloraConfig::upkeep_q10`]).
pub fn upkeep_factor(t_c: f64, t_mean_c: f64, q10: f64) -> f64 {
    q10.powf((t_c - t_mean_c) / 10.0)
}

/// **Runner support** ([`SpeciesConfig::runner_support_s`]): each runner daughter still
/// tied to its parent ([`Stand::support`]) is paid
/// [`SpeciesConfig::runner_support_share`] of this tick's shortfall — its upkeep
/// (maintenance at this tick's temperature and night, plus the construction of replacing
/// the foliage it sheds) less its income (this tick's light, daylight, moisture, aeration
/// stress and site mineral) — out of the parent's reserve above the parent's donor floor
/// (`donor_reserve_floor · reserve_cap · W`, the floor propagules are funded above), so the
/// parent never goes below it. The organic matter moves reserve to reserve with its share
/// of the parent's mineral: an internal transfer, booked in
/// [`FloraLedger::runner_support_organic`] / `_mineral`.
///
/// The tie ends when the daughter is established — past its seedling size
/// ([`SpeciesConfig::seedling_below`]) with its own income paying its upkeep at full
/// daylight ([`break_even_cover`] at least one): **weaned** — or when its time runs out or
/// the parent is gone (**ended**). A seedling that pays by day is still carried through
/// the night: its shortfall then is its whole upkeep.
fn support_runners(flora: &mut Flora, light: &[f64], moisture: &[Drink], climate: Climate) {
    let Flora {
        config,
        stands,
        ground,
        ledger,
        ..
    } = flora;
    if light.len() != stands.len() || moisture.len() != stands.len() {
        return;
    }
    let Climate {
        daylight,
        upkeep,
        night,
        dt,
        ..
    } = climate;
    for i in 0..stands.len() {
        let Some(tie) = stands[i].support else {
            continue;
        };
        let d = stands[i];
        let sc = config.species(d.species);
        let slot = d.species.index();
        // The parent by identity on its face: it may hold either plant place there.
        let parent = crate::on_site(stands, tie.parent_site).find(|&j| stands[j].id == tie.parent && j != i);
        let Some(j) = parent else {
            stands[i].support = None;
            ledger.runner_support_ended[slot] += 1;
            continue;
        };
        let n = match ground.binary_search_by_key(&d.site, |g| g.site) {
            Ok(gi) => ground[gi].mineral,
            Err(_) => config.initial_mineral,
        };
        let f = monod(sc, n);
        let mu = moisture[i].moisture * (1.0 - d.aeration_stress).clamp(0.0, 1.0);
        let l = light[i];
        // Established: past its seedling size ([`SpeciesConfig::seedling_below`]) and
        // paying its own upkeep at full daylight.
        if d.wood >= sc.seedling_below * sc.alive_min && break_even_cover(sc, d.wood, d.foliage, l, mu, f) >= 1.0 {
            stands[i].support = None;
            ledger.runner_weaned[slot] += 1;
            continue;
        }
        if !d.cured {
            let need = (sc.maintenance * upkeep * night * d.wood
                + (1.0 + sc.build) * sc.senescence * d.foliage)
                * dt;
            let income = sc.assimilation * l * d.foliage * daylight * mu * f * dt;
            let want = sc.runner_support_share.max(0.0) * (need - income).max(0.0);
            let p = stands[j];
            let psc = config.species(p.species);
            let floor = psc.donor_reserve_floor * psc.reserve_cap * p.wood;
            let pay = want.min((p.reserve - floor).max(0.0));
            if pay > 0.0 {
                let before = stands[j].material();
                stands[j].reserve -= pay;
                let mineral = pull_mineral(&mut stands[j], before, pay);
                stands[i].reserve += pay;
                stands[i].mineral += mineral;
                ledger.runner_support_organic[slot] += pay;
                ledger.runner_support_mineral[slot] += mineral;
            }
        }
        let left = tie.left_s - dt;
        if left <= 0.0 {
            stands[i].support = None;
            ledger.runner_support_ended[slot] += 1;
        } else if let Some(t) = stands[i].support.as_mut() {
            t.left_s = left;
        }
    }
}

/// What the world's clock and air ask of every plant this tick, stands and vines alike.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Climate {
    /// The share of the light it stands in the day lets a plant use: smooth through
    /// twilight, always 1 with the day clock off.
    pub(crate) daylight: f64,
    /// Maintenance's Q10 factor around the climate's mean temperature ([`upkeep_factor`]).
    pub(crate) upkeep: f64,
    /// The plant night rule: maintenance and leaf loss at `plant_night_factor` of their
    /// daytime rate while it is dark (`World::plant_night_scale`).
    pub(crate) night: f64,
    /// Seconds to the ecological dawn while the sun is down (`World::life_to_dawn_s`),
    /// `None` by day and with the clock off: the plants' night budget runs only on `Some`.
    pub(crate) to_dawn_s: Option<f64>,
    /// The world's seed mixed with its tick: the key of this tick's keyed draws that have
    /// no place of their own to key by (a lanternberry's rest between crops).
    pub(crate) draw_key: u64,
    /// The seconds this plant step covers ([`FloraConfig::step_every`] ticks): every rate
    /// below is per second and is applied over it.
    pub(crate) dt: f64,
}

impl Climate {
    pub(crate) fn of(world: &World, upkeep_q10: f64) -> Climate {
        Climate {
            daylight: world.life_daylight(),
            upkeep: upkeep_factor(
                world.life_temperature_c(),
                world.config().climate.temperature_c,
                upkeep_q10,
            ),
            night: world.plant_night_scale(),
            to_dawn_s: world.life_to_dawn_s(),
            draw_key: mix(world.config().seed ^ mix(world.tick())),
            dt: DT,
        }
    }
}

fn grow(
    flora: &mut Flora,
    world: &World,
    light: &[f64],
    climate: Climate,
    drink: &[Drink],
    substrate: &[Substrate],
    threads: usize,
) {
    let Flora {
        config,
        stands,
        ground,
        ledger,
        death_log,
        ..
    } = flora;
    // Every stand's ground slot first — a stand's growth draws on its site's pool and
    // drops its litter there — then each stand's slot index. Both lists are site-sorted,
    // so one walk finds them all, and a stand's slot index rises with its own.
    ensure_slots(ground, stands);
    let slots = slot_indices(ground, stands);

    // Every stand grows on its own stand and its own site's ground, so chunks of stands
    // split cleanly: a chunk's slots are a run of the ground no other chunk touches. Each
    // stand's ledger lines are kept in the order it booked them and folded below in stand
    // order, which is the one-pass loop's order.
    let n = stands.len();
    let mut books = vec![Booked::default(); n];
    let mut dying = vec![None::<DeathCause>; n];
    {
        let config = &*config;
        let chunks = chunk_count(n, threads);
        let per = n.div_ceil(chunks.max(1)).max(1);
        let mut work: Vec<(
            usize,
            &mut [Stand],
            &mut [Ground],
            usize,
            &mut [Booked],
            &mut [Option<DeathCause>],
        )> = Vec::with_capacity(chunks);
        let (mut st, mut gr, mut bk, mut dy) = (
            &mut stands[..],
            &mut ground[..],
            &mut books[..],
            &mut dying[..],
        );
        let mut first = 0;
        let mut ground_base = 0;
        while first < n {
            // A chunk ends between faces, never between a face's plant and its fungus:
            // both grow on the one ground slot, which only one chunk may hold.
            let mut last = (first + per).min(n);
            while last < n && slots[last] == slots[last - 1] {
                last += 1;
            }
            let (s_here, s_rest) = std::mem::take(&mut st).split_at_mut(last - first);
            let (b_here, b_rest) = std::mem::take(&mut bk).split_at_mut(last - first);
            let (d_here, d_rest) = std::mem::take(&mut dy).split_at_mut(last - first);
            // This chunk's ground runs up to the next chunk's first slot.
            let ground_end = if last < n {
                slots[last]
            } else {
                ground_base + gr.len()
            };
            let (g_here, g_rest) = std::mem::take(&mut gr).split_at_mut(ground_end - ground_base);
            work.push((first, s_here, g_here, ground_base, b_here, d_here));
            st = s_rest;
            bk = b_rest;
            dy = d_rest;
            gr = g_rest;
            ground_base = ground_end;
            first = last;
        }
        let slots = &slots[..];
        let run = |(first, stands, ground, base, books, dying): (
            usize,
            &mut [Stand],
            &mut [Ground],
            usize,
            &mut [Booked],
            &mut [Option<DeathCause>],
        )| {
            for (k, stand) in stands.iter_mut().enumerate() {
                let si = first + k;
                dying[k] = grow_stand_cause(
                    config,
                    stand,
                    &mut ground[slots[si] - base],
                    light[si],
                    climate,
                    drink[si],
                    substrate[si],
                    &mut books[k],
                );
            }
        };
        #[cfg(feature = "parallel")]
        if work.len() > 1 {
            let pool = bevy_tasks::ComputeTaskPool::get_or_init(|| {
                bevy_tasks::TaskPoolBuilder::new()
                    .num_threads(threads)
                    .build()
            });
            let run = &run;
            pool.scope(|scope| {
                for w in work.drain(..) {
                    scope.spawn(async move { run(w) });
                }
            });
        }
        for w in work {
            run(w);
        }
    }
    for (b, s) in books.iter().zip(stands.iter()) {
        b.apply(ledger);
        let line = &mut ledger.night_spend[s.species.index()];
        for k in 0..3 {
            line[k] += b.night[k];
        }
        ledger.foliage_built[s.species.index()] += b.built;
        ledger.stand_respired[s.species.index()] += b.respired[..b.respired_n as usize].iter().sum::<f64>();
        ledger.shed_organic[s.species.index()] += b.shed;
        ledger.sun_dried[s.species.index()] += b.sun;
    }
    for (b, s) in books.iter().zip(stands.iter()) {
        let k = s.species.index();
        ledger.cures[k] += u64::from(b.cured);
        ledger.cure_wakes[k] += u64::from(b.woke);
        ledger.cure_shed_organic[k] += b.cure_shed;
    }
    let dead: Vec<usize> = (0..n).filter(|&si| dying[si].is_some()).collect();

    let view = world.view();
    for &si in dead.iter().rev() {
        let cause = dying[si].expect("dying");
        die(config, &view, &stands[si], ground, ledger, cause, death_log);
    }
    remove_at(stands, &dead);
    // Growth moved the crowns and deaths emptied places: each face's plants by height.
    settle_places(config, stands, ledger);
}

/// One stand's ledger lines from [`grow_stand`], in the order it booked them, so that
/// folding them stand by stand adds every number to the ledger exactly where the one-pass
/// loop did.
#[derive(Clone, Copy, Default)]
pub(crate) struct Booked {
    pub(crate) fixed: f64,
    light: f64,
    /// The seep's organic matter, mineral and energy ([`SpeciesConfig::dark_uptake_per_s`]).
    seep: [f64; 3],
    respired: [f64; 2],
    respired_n: u8,
    heat: [f64; 5],
    heat_n: u8,
    /// The night budget's spend on `[reflush, nectar, fruit]` (1b), the reflush with its
    /// construction respiration: `FloraLedger::night_spend`'s line for this stand.
    night: [f64; 3],
    /// Foliage built this tick (`FloraLedger::foliage_built`) and foliage shed to the
    /// litter (`FloraLedger::shed_organic`), this stand's line.
    built: f64,
    shed: f64,
    /// Curing this tick: the stand cured (and dropped `cure_shed` of foliage), or woke.
    cured: bool,
    woke: bool,
    cure_shed: f64,
    /// Cap tissue and mycelium the sun dried to the litter (`FloraLedger::sun_dried`).
    sun: f64,
}

impl Booked {
    fn respired(&mut self, v: f64) {
        self.respired[self.respired_n as usize] = v;
        self.respired_n += 1;
    }

    fn heat(&mut self, v: f64) {
        self.heat[self.heat_n as usize] = v;
        self.heat_n += 1;
    }

    fn apply(&self, ledger: &mut FloraLedger) {
        // A saprotroph books no assimilation: adding its zero changes nothing.
        ledger.fixed_in += self.fixed;
        ledger.light_in += self.light;
        ledger.seep_organic_in += self.seep[0];
        ledger.seep_mineral_in += self.seep[1];
        ledger.seep_energy_in += self.seep[2];
        for &r in &self.respired[..self.respired_n as usize] {
            ledger.respired_out += r;
        }
        for &h in &self.heat[..self.heat_n as usize] {
            ledger.heat_out += h;
        }
    }
}

/// Make a ground slot for every stand's site that has none: a merge of the two
/// site-sorted lists. A plant and a fungus on one face share its one slot.
fn ensure_slots(ground: &mut Vec<Ground>, stands: &[Stand]) {
    let mut missing: Vec<Site> = Vec::new();
    let mut k = 0;
    for s in stands {
        while k < ground.len() && ground[k].site < s.site {
            k += 1;
        }
        if (k >= ground.len() || ground[k].site != s.site) && missing.last() != Some(&s.site) {
            missing.push(s.site);
        }
    }
    if missing.is_empty() {
        return;
    }
    let old = std::mem::take(ground);
    ground.reserve(old.len() + missing.len());
    let mut fresh = missing.into_iter().peekable();
    for g in old {
        while let Some(site) = fresh.next_if(|site| *site < g.site) {
            ground.push(Ground::new(site, 0.0));
        }
        ground.push(g);
    }
    ground.extend(fresh.map(|site| Ground::new(site, 0.0)));
}

/// Every stand's ground slot index, given that every stand has one.
fn slot_indices(ground: &[Ground], stands: &[Stand]) -> Vec<usize> {
    let mut out = Vec::with_capacity(stands.len());
    let mut k = 0;
    for s in stands {
        while ground[k].site < s.site {
            k += 1;
        }
        debug_assert_eq!(ground[k].site, s.site);
        out.push(k);
    }
    out
}

/// **Photosynthesis**, the income function of a stand and of the latticevine:
/// `assimilation` per unit of lit foliage per second, times the day, the root's moisture
/// ramp and whatever else limits it — aeration and the mineral Monod for a stand, the
/// share of its water demand met for the vine. Organic units this tick.
pub(crate) fn photosynthesis(
    assimilation: f64,
    lit_foliage: f64,
    daylight: f64,
    moisture: f64,
    limit: f64,
    dt: f64,
) -> f64 {
    (assimilation * lit_foliage * daylight * moisture * limit * dt).max(0.0)
}

/// **Saprotrophy**, a fungus's income function: the dead wood and litter [`feed`] already
/// took out of the pools for it. Its yield `gross` ([`yield_of`], pool by pool) is income;
/// the rest is respired **at once** — it has left the pool and is not tissue — with the
/// energy the income does not carry as heat. There is no demand cap: what the stand cannot
/// spend is respired at the budget's close. Returns the income and the mineral that
/// arrived with it.
fn saprotrophy(substrate: Substrate, e_v: f64, book: &mut Booked) -> (f64, f64) {
    let Substrate { taken, gross } = substrate;
    let gross = gross.max(0.0).min(taken.organic);
    book.respired((taken.organic - gross).max(0.0));
    book.heat((taken.energy - e_v * gross).max(0.0));
    (gross, taken.mineral)
}

/// §4.1–4.6 for one stand on its own site's ground `g`: the loop body of [`grow`]. Its
/// ledger lines go into `book`. Returns whether the stand dies (§4.7).
#[allow(clippy::too_many_arguments)]
pub(crate) fn grow_stand(
    config: &FloraConfig,
    stand: &mut Stand,
    g: &mut Ground,
    light: f64,
    climate: Climate,
    drink: Drink,
    substrate: Substrate,
    book: &mut Booked,
) -> bool {
    grow_stand_cause(config, stand, g, light, climate, drink, substrate, book).is_some()
}

/// [`grow_stand`], saying what killed the stand.
#[allow(clippy::too_many_arguments)]
pub(crate) fn grow_stand_cause(
    config: &FloraConfig,
    stand: &mut Stand,
    g: &mut Ground,
    light: f64,
    climate: Climate,
    drink: Drink,
    substrate: Substrate,
    book: &mut Booked,
) -> Option<DeathCause> {
    let Climate {
        daylight,
        upkeep,
        night,
        to_dawn_s,
        draw_key,
        dt,
    } = climate;
    let species = stand.species;
    let sc = config.species(species);
    let e_v = sc.energy_density;
    let (w0, p0, q0) = (stand.wood, stand.foliage, stand.reserve);
    let l_eff = light;
    let mu = drink.moisture;
    stand.light = l_eff;
    stand.moisture = mu;
    stand.water_m3 = drink.taken_m3;

    // ---- aeration stress, from the same pre-withdrawal read `μ` came from. It
    // closes on the level the root box's saturated fraction asks for — up at
    // `stress_rate_per_s`, down at `relax_rate_per_s`, both as a fraction of the
    // gap per second — and it is applied to *this* tick's income: the water was read
    // before anything was withdrawn, so the stress and the moisture describe one
    // moment.
    let saturated = drink.saturated;
    let target = aeration_target(saturated, sc);
    let was = stand.aeration_stress;
    // Exponential closure, so the step size does not change where it ends up.
    let stress = if target > was {
        (was + decay_share(sc.stress_rate_per_s, dt) * (target - was)).clamp(0.0, 1.0)
    } else if target < was {
        (was - decay_share(sc.relax_rate_per_s, dt) * (was - target)).clamp(0.0, 1.0)
    } else {
        was
    };
    stand.aeration_stress = stress;

    // ---- 4.0 curing (drought dormancy, [`SpeciesConfig::cure_after_s`]): a green stand
    // dry below `cure_water` for `cure_after_s` cures — drops `cure_shed` of its foliage
    // to the litter and goes dormant — and a cured stand wakes at `cure_resume_water` and
    // regrows by the ordinary rules below (the reflush from its reserve). While cured it
    // runs [`dormant`] instead of the rest of this function.
    //
    // A green stand's clock also runs while its root box is below `cure_resume_water` and
    // the income its water lets it earn cannot pay its upkeep ([`water_short`],
    // [`SpeciesConfig::cure_cover`]): it goes dormant on its reserve instead of starving.
    if sc.cure_after_s > 0.0 && sc.trophic == Trophic::Photo {
        let read = CureRead {
            available: drink.available,
            moisture: mu,
            light_eff: l_eff,
            stress,
            mineral: g.mineral,
            wood: stand.wood,
            foliage: stand.foliage,
        };
        match cure_step(sc, stand.cured, &mut stand.dry_s, &read, dt) {
            CureStep::Green => {}
            CureStep::Wake => {
                stand.cured = false;
                book.woke = true;
            }
            CureStep::Dormant => return dormant(config, stand, g, upkeep * night, book, dt),
            CureStep::Cure => {
                cure(config, stand, g, book);
                return dormant(config, stand, g, upkeep * night, book, dt);
            }
        }
    }

    // ---- 4.1 income, 4.2 demands
    //
    // Income and every demand read the **site's** mineral pool `N`. A photosynthetic
    // stand's income is gated by it as a whole — the part that pays maintenance too — so
    // a stand rich in its own mineral still fixes nothing on a bare pool: `Stand::mineral`
    // is an inventory, not a usable reserve.
    let n0 = g.mineral;
    let p_cap = sc.alpha * w0;
    let q_max = sc.reserve_cap * w0;
    // Maintenance alone follows the temperature; growth respiration does not. A
    // photosynthetic stand's maintenance and senescence also follow the night rule
    // (`World::plant_night_scale`); a saprotroph's do not.
    let night = match sc.trophic {
        Trophic::Photo => night,
        Trophic::Saprotroph => 1.0,
    };
    let m = sc.maintenance * upkeep * night * w0 * dt;
    let d_p = (p_cap - p0).max(0.0).min(sc.foliage_rate * w0 * dt);
    let d_w = (sc.wood_max - w0).max(0.0).min(sc.wood_rate * w0 * dt);
    let d_q = (q_max - q0).max(0.0);
    let (a, arrived_mineral) = match sc.trophic {
        Trophic::Photo => {
            let monod = if n0 + sc.nutrient_half > 0.0 {
                n0 / (n0 + sc.nutrient_half)
            } else {
                0.0
            };
            // Capped by the pool's draw rate, by the tissue the pool can pay `n_tissue`
            // for, and by what the stand can spend: light not captured is never fixed,
            // so there is nothing to respire.
            let a = photosynthesis(
                sc.assimilation,
                l_eff * p0,
                daylight,
                mu,
                (1.0 - stress) * monod,
                dt,
            )
            .min(sc.nutrient_draw_max * n0 * dt)
            .min(tissue_funded(n0, sc.n_tissue))
            .max(0.0)
            .min(m + (1.0 + sc.build) * (d_p + d_w) + d_q);
            book.fixed = a;
            book.light = e_v * a;
            (a, 0.0)
        }
        Trophic::Saprotroph => {
            let (a, mineral) = saprotrophy(substrate, e_v, book);
            // The seep: a dark site's drip water, paid per unit of mycelium at its darkness
            // and `μ`, on top of the substrate, and capped like photosynthesis at what the
            // stand can spend this tick — what it cannot spend never arrives. It brings its
            // own mineral at `n_tissue` and its energy at `e_v`, booked as an input.
            let spend = m + (1.0 + sc.build) * (d_p + d_w) + d_q;
            let seep = (if sc.substrate_required { 0.0 } else { sc.dark_uptake_per_s * w0 * drink.darkness * mu * dt })
                .max(0.0)
                .min((spend - a).max(0.0));
            if seep > 0.0 {
                let n = sc.n_tissue.max(0.0) * seep;
                book.seep = [seep, n, e_v * seep];
                (a + seep, mineral + n)
            } else {
                (a, mineral)
            }
        }
    };

    // ---- 4.3 maintenance from income first, then reserve; 4.4a the mineral budget
    //
    // What may be built this tick is what the mineral that **arrived** with the income
    // (a saprotroph's, out of the log) plus the site's pool can pay `n_tissue` for. For a
    // photosynthetic stand §4.2's cap on `A` already keeps it so; for a saprotroph it is
    // the whole bound. The mineral of the reserve that maintenance burns stays in the
    // stand.
    let mut budget = Budget::new(a, tissue_funded(arrived_mineral + n0.max(0.0), sc.n_tissue));
    let unpaid = budget.upkeep(&mut stand.reserve, m).unpaid;

    // ---- 4.4 growth: reserve share, foliage, wood, then the rest to reserve
    //
    // Income nothing funds is respired at the close, with its energy as heat.
    let dq_s = budget.store((sc.reserve_share * budget.income).min(d_q));
    stand.reserve += dq_s;

    let dp_a = budget.build(d_p, sc.build);
    // **The night budget** (`design/ecology-timescales-2026-10-02.md` 1b): while the sun is
    // down on the ecological clock, a photosynthetic stand spends its reserve on the
    // reflush, then nectar, then fruit, out of one allowance this tick,
    // `max(0, Q − maintenance to dawn − floor) / max(to dawn, night_min_s) · dt`, `Q` the
    // reserve after this tick's maintenance. The maintenance to dawn is this tick's rate
    // carried to dawn, so a stand spending its allowance every tick meets dawn at its
    // floor. By day, and always with the clock off, `None`: today's rules.
    let mut night_left = match (sc.trophic, to_dawn_s) {
        (Trophic::Photo, Some(t)) => {
            let hold = m / dt * t + sc.night_floor * q_max;
            Some((stand.reserve - hold).max(0.0) / t.max(sc.night_min_s) * dt)
        }
        _ => None,
    };
    // By day a photosynthetic stand reflushes from its reserve only as far as the light
    // lets the new leaves earn. At night the light gate gives way to the night budget,
    // which pays the reflush first, construction respiration included. The reflush is
    // reserve turned into foliage, so its mineral is already in the stand.
    let flush_light = match (sc.trophic, night_left) {
        (Trophic::Photo, None) => daylight,
        _ => 1.0,
    };
    // The level it reflushes below and up to is `reflush_below` of [`SpeciesConfig::reflush_cap`]:
    // `alpha · W` once the stand is established, the leaf its seed gave it while it is not,
    // so the adult foliage capacity never spends a seedling's seed reserve.
    let flush_to = sc.reflush_below * sc.reflush_cap(w0);
    let dp_q = if p0 < flush_to {
        let mut want = ((d_p - dp_a) * flush_light).min((flush_to - p0 - dp_a).max(0.0));
        if let Some(left) = night_left {
            want = want.min(left / (1.0 + sc.build));
        }
        let x = budget.build_from(&mut stand.reserve, want, sc.build);
        if let Some(left) = night_left.as_mut() {
            let cost = (1.0 + sc.build) * x;
            *left = (*left - cost).max(0.0);
            book.night[0] += cost;
        }
        x
    } else {
        0.0
    };
    stand.foliage += dp_a + dp_q;
    book.built = dp_a + dp_q;
    // Where the new foliage goes: **bottom-up to each layer's capacity**, in
    // profile order, which is what makes a browsed plant refill its floor tissue
    // first and look browsed from below (decisions §4). The capacities are read at
    // the wood this tick started with, because `dw` below has not been applied yet.
    {
        let sc = config.species(species);
        let caps = sc.layer_capacities(stand.wood);
        let n = caps.len();
        crate::layers::fill_bottom_up(stand.layer_stocks_mut(n), &caps, dp_a + dp_q);
        stand.settle_layers(n);
    }

    let dw = budget.build(d_w, sc.build);
    stand.wood += dw;

    let dq_r = budget.store((d_q - dq_s).max(0.0));
    stand.reserve += dq_r;
    // Maintenance paid, construction on every unit built and the unfunded income: organic
    // matter out of the system, its energy as heat, no mineral.
    let respired = budget.close();
    book.respired(respired);
    book.heat(e_v * respired);

    // ---- 4.4b nectar (F1, predation-and-flight §3.7): an adult stand of a species that
    // makes it moves `nectar_per_s` of its reserve into its flowers in daylight, up to
    // `nectar_cap`. A move inside the stand's material: no ledger line, and the mineral
    // stays in `stand.mineral` until a consumer takes the nectar.
    //
    // At night (1b) it is made at `night_bloom_scale` of the rate out of what the reflush
    // left of the night budget.
    if sc.nectar_per_s > 0.0 && (daylight > 0.0 || night_left.is_some()) && stand.profile_stage >= 1 {
        let room = (sc.nectar_cap - stand.nectar).max(0.0);
        let rate = if night_left.is_some() { sc.nectar_per_s * sc.night_bloom_scale } else { sc.nectar_per_s };
        let make = (rate * dt).min(room).min(stand.reserve.max(0.0)).min(night_left.unwrap_or(f64::INFINITY));
        if make > 0.0 {
            stand.reserve -= make;
            stand.nectar += make;
            if let Some(left) = night_left.as_mut() {
                *left = (*left - make).max(0.0);
                book.night[1] += make;
            }
        }
    }

    // ---- 4.4c fruit (F3, predation-and-flight §3.7): the same move for a species that
    // hangs fruit, up to `fruit_cap`. The crop's clock runs while any fruit hangs; at
    // `fruit_drop_s` the whole uneaten crop falls to this site's litter (with senescence,
    // below), carrying `fruit_mineral_fraction` of itself out of the stand's mineral.
    // At night, as nectar: the night scale, out of what is left of the budget.
    //
    // After an uneaten crop falls the stand rests (plants-flowering-vines 2026-10-03): a
    // random time, exponential with mean `fruit_rest_s`, keyed by the stand and the tick,
    // so the bushes of a grove fruit out of step. A crop eaten out refills at once.
    let resting = stand.fruit_rest_s > 0.0;
    if resting {
        stand.fruit_rest_s = (stand.fruit_rest_s - dt).max(0.0);
    }
    if !resting && sc.fruit_per_s > 0.0 && (daylight > 0.0 || night_left.is_some()) && stand.profile_stage >= 1 {
        let room = (sc.fruit_cap - stand.fruit).max(0.0);
        let rate = if night_left.is_some() { sc.fruit_per_s * sc.night_bloom_scale } else { sc.fruit_per_s };
        let make = (rate * dt).min(room).min(stand.reserve.max(0.0)).min(night_left.unwrap_or(f64::INFINITY));
        if make > 0.0 {
            stand.reserve -= make;
            stand.fruit += make;
            if let Some(left) = night_left.as_mut() {
                *left = (*left - make).max(0.0);
                book.night[2] += make;
            }
        }
    }
    let mut fell = (0.0, 0.0);
    if stand.fruit > 0.0 {
        stand.fruit_age_s += dt;
        if stand.fruit_age_s >= sc.fruit_drop_s {
            let o = std::mem::replace(&mut stand.fruit, 0.0);
            let m = (sc.fruit_mineral_fraction.max(0.0) * o).min(stand.mineral.max(0.0));
            stand.mineral -= m;
            stand.fruit_age_s = 0.0;
            fell = (o, m);
            if sc.fruit_rest_s > 0.0 {
                let u = 1.0 - Rng::keyed(DOMAIN_FRUIT_REST, draw_key, stand.id, 0).unit();
                stand.fruit_rest_s = -u.ln() * sc.fruit_rest_s;
            }
        }
    } else {
        stand.fruit_age_s = 0.0;
    }

    // The tissue built out of this tick's income settles its mineral: `n_tissue` per
    // unit, met from what arrived with the income first, the excess released to the
    // site's pool and a shortfall drawn from it. Only this stand draws on this site.
    let need = (sc.n_tissue * (dq_s + dp_a + dw + dq_r)).max(0.0);
    let settled = settle_mineral(arrived_mineral, need, g.mineral);
    stand.mineral += arrived_mineral - settled.released + settled.drawn;
    g.mineral += settled.released - settled.drawn;

    // ---- 4.5 senescence
    let shed = (sc.senescence * night * stand.foliage * dt).min(stand.foliage);
    book.shed = shed;
    let organic_before = stand.material();
    stand.foliage -= shed;
    // Senescence loses from the **top** foliage layer first: a plant drops its
    // canopy before its rosette, and the rosette persists
    // (`design/organism-anatomy-2026-09-21.md` §3, bloomcrown).
    {
        let n = sc.foliage_layer_count(stand.wood);
        crate::layers::shed_from_top(stand.layer_stocks_mut(n), shed);
        stand.settle_layers(n);
    }
    let shed_mineral = pull_mineral(stand, organic_before, shed);
    {
        let mut heat = FloraLedger::default();
        let (fo, fm) = fell;
        add_litter(config, g, shed + fo, shed_mineral + fm, e_v * (shed + fo), &mut heat);
        if heat.heat_out != 0.0 {
            book.heat(heat.heat_out);
        }
    }

    // ---- 4.6 dieback
    let die_back = stand.wood.min(sc.dieback * unpaid);
    let organic_before = stand.material();
    stand.wood -= die_back;
    let die_back_mineral = pull_mineral(stand, organic_before, die_back);
    if sc.trophic == Trophic::Saprotroph && sc.substrate_required {
        let mut heat = FloraLedger::default();
        add_litter(config, g, die_back, die_back_mineral, e_v * die_back, &mut heat);
        book.heat(heat.heat_out);
    } else {
        g.dead_wood += die_back;
        g.dead_wood_mineral += die_back_mineral;
        g.dead_wood_energy += e_v * die_back;
    }

    // ---- 4.6a the sun (GLOWCAP-2): a saprotroph whose exposure is above its shade dries —
    // its caps and its surface mycelium, each at its own rate times how far the exposure is
    // toward full sun ([`sun_share`]) times the daylight — to its own face's litter, with
    // their mineral and energy. Nothing at night, nothing in shade.
    let mut desiccated = false;
    if sc.trophic == Trophic::Saprotroph
        && (sc.sun_cap_loss_per_s > 0.0 || sc.sun_mycelium_loss_per_s > 0.0)
    {
        // Its own dark habitat shelters it in proportion (SUN-SHELTER).
        let shelter = (1.0 - sc.sun_dark_shelter.clamp(0.0, 1.0) * drink.darkness.clamp(0.0, 1.0)).max(0.0);
        let sun = sun_share(sc, l_eff) * daylight.max(0.0) * shelter;
        if sun > 0.0 {
            let cap = (sc.sun_cap_loss_per_s * sun * dt).clamp(0.0, 1.0) * stand.foliage.max(0.0);
            let myc = (sc.sun_mycelium_loss_per_s * sun * dt).clamp(0.0, 1.0) * stand.wood.max(0.0);
            let wood_was = stand.wood;
            let organic_before = stand.material();
            stand.foliage -= cap;
            {
                let n = sc.foliage_layer_count(stand.wood);
                crate::layers::shed_from_top(stand.layer_stocks_mut(n), cap);
                stand.settle_layers(n);
            }
            stand.wood -= myc;
            let dried = cap + myc;
            let dried_mineral = pull_mineral(stand, organic_before, dried);
            let mut heat = FloraLedger::default();
            add_litter(config, g, dried, dried_mineral, e_v * dried, &mut heat);
            if heat.heat_out != 0.0 {
                book.heat(heat.heat_out);
            }
            book.sun = dried;
            desiccated = wood_was >= sc.alive_min && stand.wood < sc.alive_min;
        }
    }

    // ---- 4.6b the stage, after every change to the wood
    //
    // Growth and dieback both move `W`, and the profile is staged by `W / W_max`.
    // A stand that has crossed a threshold re-bins the tissue it **already holds**
    // into the new stage's layers, bottom-up: a seedling rosette that has grown a
    // stem does not conjure a crown, it lifts what it had. Nothing is created and
    // nothing is destroyed (decisions §4).
    stand.resync_layers(config.species(species));

    // ---- 4.7 death: the sun's when this tick's drying took it under, else starvation
    (stand.wood < sc.alive_min).then_some(if desiccated {
        DeathCause::Desiccated
    } else {
        DeathCause::Starved
    })
}

/// How far a saprotroph's `exposure` ([`Stand::light`]) is from shade toward full sun, for
/// its sun loss ([`SpeciesConfig::sun_shade_max`]): 0 at and below `sun_shade_max`, rising
/// linearly to 1 at full sun.
pub(crate) fn sun_share(sc: &SpeciesConfig, exposure: f64) -> f64 {
    if !(exposure > sc.sun_shade_max) || sc.sun_shade_max >= 1.0 {
        return 0.0;
    }
    ramp(exposure, sc.sun_shade_max, 1.0)
}

/// **A stand cures**: `cure_shed` of its foliage falls to its site's litter, from the top
/// layer down like senescence, with its share of the stand's mineral and its energy —
/// booked as shed foliage — and it is marked cured with its dormancy clock at zero.
fn cure(config: &FloraConfig, stand: &mut Stand, g: &mut Ground, book: &mut Booked) {
    let sc = config.species(stand.species);
    let e_v = sc.energy_density;
    let shed = (sc.cure_shed.clamp(0.0, 1.0) * stand.foliage).max(0.0);
    let before = stand.material();
    stand.foliage -= shed;
    let n = sc.foliage_layer_count(stand.wood);
    crate::layers::shed_from_top(stand.layer_stocks_mut(n), shed);
    stand.settle_layers(n);
    let mineral = pull_mineral(stand, before, shed);
    let mut heat = FloraLedger::default();
    add_litter(config, g, shed, mineral, e_v * shed, &mut heat);
    if heat.heat_out != 0.0 {
        book.heat(heat.heat_out);
    }
    book.shed = shed;
    book.cure_shed = shed;
    book.cured = true;
    stand.cured = true;
    stand.dry_s = 0.0;
}

/// **One dormant tick** of a cured stand: no income at all — no photosynthesis, no
/// growth, no senescence, no reflush — and `cure_upkeep` of its maintenance (`scale` the
/// temperature and night factors) paid from its reserve; maintenance it cannot pay
/// diebacks its wood as usual. Its dormancy clock runs, and past `cure_max_s` the drought
/// kills it, as it does when its reserve and wood run out.
fn dormant(
    config: &FloraConfig,
    stand: &mut Stand,
    g: &mut Ground,
    scale: f64,
    book: &mut Booked,
    dt: f64,
) -> Option<DeathCause> {
    let sc = config.species(stand.species);
    let e_v = sc.energy_density;
    let m = (sc.maintenance * scale * sc.cure_upkeep * stand.wood * dt).max(0.0);
    let mut budget = Budget::new(0.0, 0.0);
    let unpaid = budget.upkeep(&mut stand.reserve, m).unpaid;
    let respired = budget.close();
    book.respired(respired);
    book.heat(e_v * respired);
    let die_back = stand.wood.min(sc.dieback * unpaid);
    let before = stand.material();
    stand.wood -= die_back;
    let die_back_mineral = pull_mineral(stand, before, die_back);
    g.dead_wood += die_back;
    g.dead_wood_mineral += die_back_mineral;
    g.dead_wood_energy += e_v * die_back;
    stand.resync_layers(sc);
    stand.dry_s += dt;
    // A dormant stand whose reserve and crown run out has died of the drought it slept
    // through, as one past `cure_max_s` has.
    if stand.wood < sc.alive_min {
        Some(DeathCause::Drought)
    } else if sc.cure_max_s > 0.0 && stand.dry_s >= sc.cure_max_s {
        Some(DeathCause::Drought)
    } else {
        None
    }
}

/// The mineral belonging to `moved` units of a stand's organic matter, taken out of the
/// stand's mineral stock. Proportional: a stand's mineral is a stock and not
/// `n_tissue · O`, because respiration leaves mineral behind, so a transfer of a
/// fraction of the organic matter has to take the same fraction of the mineral.
/// `before` is the stand's organic matter *including* `moved`.
///
/// `pub(crate)` because a **withdrawal** out of a stand's foliage is the same transfer
/// ([`crate::Flora::take_foliage`]): a consumer taking a fifth of a stand's material takes
/// a fifth of its mineral, exactly as senescence and a leaving propagule package do. One
/// rule, one function.
pub(crate) fn pull_mineral(stand: &mut Stand, before: f64, moved: f64) -> f64 {
    if moved <= 0.0 || before <= 0.0 || stand.mineral <= 0.0 {
        return 0.0;
    }
    let out = if moved >= before {
        stand.mineral
    } else {
        cubarium_voxel::pro_rata(stand.mineral, moved, before)
    };
    stand.mineral -= out;
    out
}

/// §4.7 death: wood to dead wood, foliage, reserve and any saved parcel to litter under the
/// energy cap,
/// and the stand's mineral split between the two in proportion to the organic matter
/// each takes.
///
/// The one death path, whatever killed the stand. For a species with
/// [`crate::SpeciesConfig::falls`] (the vaulttree) the wood, its mineral and
/// its energy are laid in equal shares along its [`fall_line`] instead of on its own site;
/// the litter stays where the stand stood. Every other species' line is its own site.
pub(crate) fn die(
    config: &FloraConfig,
    view: &VoxelView<'_>,
    stand: &Stand,
    ground: &mut Vec<Ground>,
    ledger: &mut FloraLedger,
    cause: DeathCause,
    log: &mut Vec<DeathRecord>,
) {
    let sc = config.species(stand.species);
    let e_v = sc.energy_density;
    let organic = stand.material();
    if sc.trophic == Trophic::Saprotroph && sc.substrate_required {
        let gi = ground_slot(ground, stand.site);
        add_litter(config, &mut ground[gi], organic, stand.mineral, e_v * organic, ledger);
        ledger.deaths += 1;
        ledger.deaths_by_cause[cause.index()][stand.species.index()] += 1;
        log.push(DeathRecord { id: stand.id, site: stand.site, species: stand.species, cause });
        return;
    }
    let wood_mineral = cubarium_voxel::pro_rata(stand.mineral, stand.wood, organic);
    let wood_energy = e_v * stand.wood;
    let line = if sc.falls {
        fall_line(config, view, stand)
    } else {
        vec![(stand.site, 1)]
    };
    let n: usize = line.iter().map(|l| l.1).sum();
    let (mut wood_left, mut mineral_left, mut energy_left) =
        (stand.wood, wood_mineral, wood_energy);
    for (i, &(site, parts)) in line.iter().enumerate() {
        // Equal shares, and the last destination takes exactly what is left, so the
        // line's total is the wood at death to the last bit.
        let (w, m, e) = if i + 1 == line.len() {
            (wood_left, mineral_left, energy_left)
        } else {
            let f = parts as f64 / n as f64;
            (stand.wood * f, wood_mineral * f, wood_energy * f)
        };
        wood_left -= w;
        mineral_left -= m;
        energy_left -= e;
        let gi = if site == stand.site {
            ground_slot(ground, site)
        } else {
            provisioned_slot(config, ground, site, ledger)
        };
        let g = &mut ground[gi];
        g.dead_wood += w;
        g.dead_wood_mineral += m;
        g.dead_wood_energy += e;
    }
    // A parcel dies with its donor: material saved for a package that will never be sent
    // falls where the foliage and the reserve fall, and its mineral goes with it.
    let shed = stand.foliage + stand.reserve + stand.parcel + stand.nectar + stand.fruit;
    let shed_mineral = (stand.mineral - wood_mineral).max(0.0);
    let gi = ground_slot(ground, stand.site);
    add_litter_cap(
        config.litter_energy_cap,
        &mut ground[gi],
        shed,
        shed_mineral,
        e_v * shed,
        ledger,
    );
    ledger.deaths += 1;
    ledger.deaths_by_cause[cause.index()][stand.species.index()] += 1;
    if stand.place == crate::FaceLayer::Under {
        ledger.under_deaths[cause.index()][stand.species.index()] += 1;
    }
    log.push(DeathRecord {
        id: stand.id,
        site: stand.site,
        species: stand.species,
        cause,
    });
}

/// **The fall**: where a falling stand's wood lands, as
/// `(site, shares)` with the shares summing to `round(crown_radius / voxel)`, at least
/// one.
///
/// The line runs from the stand's own site in a direction hashed from the world seed and
/// the site alone — not the terrain, not the tick — one site per step along the major
/// axis of that direction (the minor axis rounded), so share `k` lies `k` sites out. The
/// site in a column is its highest support face no higher than the crown's top; a column
/// with none there is **unsupported**, and its share goes to the nearest supported site on
/// the line (the nearer the origin on a tie), which the origin always is.
///
/// Public so a diagnostic can say where a stand that died would have fallen; `die` is the
/// only caller in the model.
pub fn fall_line(config: &FloraConfig, view: &VoxelView<'_>, stand: &Stand) -> Vec<(Site, usize)> {
    let sc = config.species(stand.species);
    let c = view.config;
    let voxel_m = c.voxel_m;
    let n = ((sc.crown_radius_m_at(stand.wood) / voxel_m).round().max(1.0)) as usize;
    let origin = stand.site;
    let index = c.index(i64::from(origin.x), origin.y, origin.z) as u64;
    let mut rng = Rng::keyed(DOMAIN_FALL, c.seed, index, 0);
    let angle = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64 * std::f64::consts::TAU;
    let (dx, dz) = (angle.cos(), angle.sin());
    let major = dx.abs().max(dz.abs());
    let (sx, sz) = (dx / major, dz / major);
    let top = origin.y.saturating_add(sc.crown_voxels(stand.wood, voxel_m));
    let width = i64::from(c.width);

    // Each share's own site, or `None` where the column holds no support under the crown.
    let own: Vec<Option<Site>> = (0..n)
        .map(|k| {
            if k == 0 {
                return Some(origin);
            }
            let x = (i64::from(origin.x) + (k as f64 * sx).round() as i64).rem_euclid(width);
            let z = i64::from(origin.z) + (k as f64 * sz).round() as i64;
            if z < 0 || z >= i64::from(c.depth) {
                return None;
            }
            let z = z as u32;
            (0..=top.min(c.height.saturating_sub(1)))
                .rev()
                .find(|&y| view.is_support(x, y, z))
                .map(|y| Site { x: x as u32, y, z })
        })
        .collect();
    let mut out: Vec<(Site, usize)> = Vec::new();
    for k in 0..n {
        let site = match own[k] {
            Some(s) => s,
            None => (1..n)
                .flat_map(|d| [k.checked_sub(d), Some(k + d)])
                .flatten()
                .find_map(|j| own.get(j).copied().flatten())
                .unwrap_or(origin),
        };
        match out.iter_mut().find(|e| e.0 == site) {
            Some(e) => e.1 += 1,
            None => out.push((site, 1)),
        }
    }
    out
}

/// The site's ground entry, **provisioned** if the site has none: a fallen log is a
/// landing like a package or a deposit, so an unrepresented site gets its
/// `initial_mineral`, booked as `seeded_mineral_in` ([`crate::Provision::Lazy`]).
pub(crate) fn provisioned_slot(
    config: &FloraConfig,
    ground: &mut Vec<Ground>,
    site: Site,
    ledger: &mut FloraLedger,
) -> usize {
    match ground.binary_search_by_key(&site, |g| g.site) {
        Ok(i) => i,
        Err(i) => {
            ledger.seeded_mineral_in += config.initial_mineral;
            ground.insert(i, Ground::new(site, config.initial_mineral));
            i
        }
    }
}

fn add_litter(
    config: &FloraConfig,
    g: &mut Ground,
    organic: f64,
    mineral: f64,
    energy: f64,
    ledger: &mut FloraLedger,
) {
    add_litter_cap(
        config.litter_energy_cap,
        g,
        organic,
        mineral,
        energy,
        ledger,
    );
}

/// Litter takes the organic matter, the mineral that was in it, and as much of its energy
/// as `e_d_max · D` leaves room for; the rest of the energy is respired. The mineral is
/// never capped: it has nowhere else to be.
///
/// `pub(crate)` because a **litter deposit** ([`crate::Flora::deposit`]) is the same
/// transfer into the same pool under the same cap: one rule, one function.
pub(crate) fn add_litter_cap(
    cap: f64,
    g: &mut Ground,
    organic: f64,
    mineral: f64,
    energy: f64,
    ledger: &mut FloraLedger,
) {
    if organic <= 0.0 && energy <= 0.0 && mineral <= 0.0 {
        return;
    }
    g.litter += organic;
    g.litter_mineral += mineral;
    let room = (cap * g.litter - g.litter_energy).max(0.0);
    let kept = energy.min(room).max(0.0);
    g.litter_energy += kept;
    ledger.heat_out += energy - kept;
}

/// The site's ground entry, created empty if a stand somehow has none. Empty is the
/// only safe creation here: `initial_mineral` is a named inflow, and mineral that is not
/// booked is a residual.
fn ground_slot(ground: &mut Vec<Ground>, site: Site) -> usize {
    match ground.binary_search_by_key(&site, |g| g.site) {
        Ok(i) => i,
        Err(i) => {
            ground.insert(i, Ground::new(site, 0.0));
            i
        }
    }
}

// ------------------------------------------------------------ 7. decomposition

/// Litter and dead wood decompose at their own rates, drawing on what the site held
/// **before** this tick: organic matter this tick deposited is eligible from the next
/// one. Decomposition respires the organic matter out of the system (`respired_out`) and
/// releases the stock's mineral to the site's pool **at the same fraction**, so mineral
/// is conserved exactly and a decomposing stock's mineral density never moves. Energy
/// leaves at the stock's current density for the same reason, which is what lets the
/// `e_d_max` cap survive without a re-clamp.
/// The dead pools' tick alone: the tick-start snapshot and [`decompose`], exactly as
/// [`step`] runs them, and nothing else — no clock, no growth, no litterfall. For a frozen
/// episode that replays a world's production and must still let its dead pools rot at
/// the model's own rates.
pub(crate) fn decompose_only(flora: &mut Flora) {
    let pre: Vec<Pre> = flora
        .ground
        .iter()
        .map(|g| Pre {
            site: g.site,
            litter: g.litter,
            dead_wood: g.dead_wood,
            carrion: g.carrion,
        })
        .collect();
    decompose(flora, &pre, DT);
}

fn decompose(flora: &mut Flora, pre: &[Pre], dt: f64) {
    let Flora {
        config,
        ground,
        ledger,
        ..
    } = flora;
    // Both are in site order, so one walk pairs each site with its tick-start stocks.
    let mut i = 0;
    for g in ground.iter_mut() {
        while i < pre.len() && pre[i].site < g.site {
            i += 1;
        }
        // A site the tick created has nothing eligible yet.
        if i == pre.len() || pre[i].site != g.site {
            continue;
        }
        let (litter0, wood0, carrion0) = (pre[i].litter, pre[i].dead_wood, pre[i].carrion);
        let before = g.litter;
        decompose_pool(
            &mut g.litter,
            &mut g.litter_mineral,
            &mut g.litter_energy,
            &mut g.mineral,
            config.decomposition,
            dt,
            litter0,
            ledger,
        );
        ledger.decomposed_by_pool[0] += before - g.litter;
        let before = g.dead_wood;
        decompose_pool(
            &mut g.dead_wood,
            &mut g.dead_wood_mineral,
            &mut g.dead_wood_energy,
            &mut g.mineral,
            config.wood_decomposition,
            dt,
            wood0,
            ledger,
        );
        ledger.decomposed_by_pool[1] += before - g.dead_wood;
        let before = g.carrion;
        // Carrion, at its own rate, on exactly the two flows the other two pools use: a
        // corpse is not a special case of the ledger, only of the clock.
        decompose_pool(
            &mut g.carrion,
            &mut g.carrion_mineral,
            &mut g.carrion_energy,
            &mut g.mineral,
            config.carrion_decomposition,
            dt,
            carrion0,
            ledger,
        );
        ledger.decomposed_by_pool[2] += before - g.carrion;
        g.clear_spent_carcass(config.carcass_gone_fraction);
    }
}

/// What one site held when this tick started: the stocks [`decompose`] may draw on.
struct Pre {
    site: Site,
    litter: f64,
    dead_wood: f64,
    carrion: f64,
}

/// One dead pool's tick of decomposition: `rate · dt` of the **tick-start** stock `stock0`,
/// capped by what the pool holds now, respired out of the system, with the mineral released
/// into `pool` and the energy leaving as heat **both at the stock's current density** — so
/// neither density moves, mineral is conserved exactly, and litter's `e_d_max` cap needs no
/// re-clamp. Never `0/0`: the guard is the `min` above the division.
fn decompose_pool(
    organic: &mut f64,
    mineral: &mut f64,
    energy: &mut f64,
    pool: &mut f64,
    rate: f64,
    dt: f64,
    stock0: f64,
    ledger: &mut FloraLedger,
) {
    let dec = (rate * dt * stock0).min(*organic).max(0.0);
    if !(dec > 0.0) || !(*organic > 0.0) {
        return;
    }
    let f = dec / *organic;
    let out = *energy * f;
    *energy -= out;
    ledger.heat_out += out;
    let released = (*mineral * f).min(*mineral);
    *mineral -= released;
    *pool += released;
    *organic -= dec;
    ledger.respired_out += dec;
}

// ------------------------------------------------------ 8–9. seed bank, propagules

// `seeds.rs`: the bank's checks, germination, attrition and every dispersal mode.

/// The germination predicate for every **birth a tick makes** — the bank's lottery and a
/// runner's daughter: wet enough for its roots, **aerated** enough for them (the site's
/// saturated fraction *now*, so an intolerant species is shut out of a waterlogged basin
/// rather than merely doing badly there), bright enough for its leaves, something to eat
/// if it eats wood, and not already under water it cannot stand in.
///
/// It also reads the clock: a photosynthetic newborn waits for `daylight` above zero,
/// because one that comes up in the dark has no income until dawn and a reserve sized for
/// minutes. The public habitat predicates ([`establishment_gates`] and its forms) are about
/// the site, not the hour: a host founding a world at night still finds its habitat.
///
/// A site that passes every gate is then asked the **canopy gate**
/// ([`SpeciesConfig::establish_light_margin`]): whether the light that reaches the
/// species' newborn through the taller foliage around it pays the newborn's upkeep
/// ([`canopy_pays`]). The answer says which of the two refused.
#[allow(clippy::too_many_arguments)]
pub(crate) fn establishes(
    view: &VoxelView<'_>,
    sky: &mut Vec<(Site, f64)>,
    ground: &[Ground],
    stands: &[Stand],
    config: &FloraConfig,
    site: Site,
    species: Species,
    daylight: f64,
) -> Establish {
    let sc = config.species(species);
    if sc.trophic == Trophic::Photo && daylight <= 0.0 {
        return Establish::Gates;
    }
    let visibility = sky_at(sky, view, site);
    let (dead_wood, litter) = substrate_pools_in_box(view, ground, site, sc);
    let g = establishment_gates_with_sky(view, site, sc, visibility, dead_wood, litter);
    if !g.passes() {
        return Establish::Gates;
    }
    if !canopy_pays(config, view, ground, stands, site, species, visibility, &g) {
        return Establish::Canopy;
    }
    Establish::Yes
}

/// [`establishes`]'s answer: refused by the site's gates, refused by the canopy gate, or
/// a newborn may come up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Establish {
    Gates,
    Canopy,
    Yes,
}


/// The mineral Monod factor of §4.1's income, `N / (N + K_N)`, on a site pool `n`.
pub(crate) fn monod(sc: &SpeciesConfig, n: f64) -> f64 {
    if n + sc.nutrient_half > 0.0 {
        n / (n + sc.nutrient_half)
    } else {
        0.0
    }
}

/// **A photosynthetic stand's carbon break-even, as a cover ratio**: its income at full
/// daylight over its upkeep,
///
/// `assimilation · L_eff · P · μ · monod / (maintenance · W + (1 + c_g) · senescence · P)`,
///
/// where the upkeep is maintenance plus the construction cost of replacing the foliage it
/// sheds — the break-even [`SpeciesConfig::velvetpad`]'s doc states for a full-foliage
/// stand, here at the stand's own `W` and `P`. Read at the reference temperature, with no
/// aeration stress (a newborn has none) and the night out of it: a statement about the
/// stand's light, moisture and mineral, not about the hour. One or more pays. Infinity for
/// a saprotroph, which is not judged on light, and for a stand with no upkeep.
pub fn break_even_cover(
    sc: &SpeciesConfig,
    wood: f64,
    foliage: f64,
    l_eff: f64,
    moisture: f64,
    monod: f64,
) -> f64 {
    if sc.trophic != Trophic::Photo {
        return f64::INFINITY;
    }
    let upkeep = sc.maintenance * wood.max(0.0) + (1.0 + sc.build) * sc.senescence * foliage.max(0.0);
    let income = sc.assimilation * l_eff * foliage.max(0.0) * moisture * monod;
    if upkeep <= 0.0 {
        return f64::INFINITY;
    }
    (income / upkeep).max(0.0)
}

/// What a curing species' dry clock reads on one tick (package DORMANCY): the root box's
/// mean available water and the stand's moisture `μ` (its ramp of it), its light response,
/// aeration stress, its site's mineral, and its wood and foliage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CureRead {
    pub available: f64,
    pub moisture: f64,
    pub light_eff: f64,
    pub stress: f64,
    pub mineral: f64,
    pub wood: f64,
    pub foliage: f64,
}

/// **Is the stand short on its water?** ([`SpeciesConfig::cure_cover`]): `Some(true)` when
/// the income its water lets it earn, after the reserve's share, covers less than
/// `cure_cover` of its upkeep and leaf turnover while full water would cover it;
/// `Some(false)` when it covers it (or the knob is off); `None` when even full water would
/// not — water is not the question (no light, too little mineral).
pub fn water_short(sc: &SpeciesConfig, r: &CureRead) -> Option<bool> {
    if !(sc.cure_cover > 0.0) {
        return Some(false);
    }
    let usable = (1.0 - sc.reserve_share).max(0.0);
    let n = monod(sc, r.mineral);
    let wet = (1.0 - r.stress).clamp(0.0, 1.0);
    let at = |mu: f64| usable * break_even_cover(sc, r.wood, r.foliage, r.light_eff, mu * wet, n);
    if at(1.0) < sc.cure_cover {
        return None;
    }
    Some(at(r.moisture) < sc.cure_cover)
}

/// What the curing clock does to a stand this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CureStep {
    /// Green, growing by the ordinary rules (its clock ran, held or reset).
    Green,
    /// Green, and its clock reached `cure_after_s`: it cures now.
    Cure,
    /// Cured, and stays dormant this tick.
    Dormant,
    /// Cured, and its water reached `cure_resume_water`: it wakes now.
    Wake,
}

/// **The curing clock** of a curing species ([`SpeciesConfig::cure_after_s`] positive): a
/// cured stand wakes at `cure_resume_water`; a green one's `dry_s` runs while its root box
/// is below `cure_water`, or below `cure_resume_water` and [`water_short`]; holds while
/// below `cure_resume_water` and water is not the question; and resets otherwise. One rule,
/// read by the plant step and by the webmodel's stand law.
pub fn cure_step(sc: &SpeciesConfig, cured: bool, dry_s: &mut f64, r: &CureRead, dt: f64) -> CureStep {
    if cured {
        if r.available >= sc.cure_resume_water {
            *dry_s = 0.0;
            return CureStep::Wake;
        }
        return CureStep::Dormant;
    }
    let dry = if r.available < sc.cure_water {
        Some(true)
    } else if r.available < sc.cure_resume_water {
        water_short(sc, r)
    } else {
        Some(false)
    };
    match dry {
        Some(true) => {
            *dry_s += dt;
            if *dry_s >= sc.cure_after_s {
                return CureStep::Cure;
            }
        }
        Some(false) => *dry_s = 0.0,
        None => {}
    }
    CureStep::Green
}

/// **The canopy gate** ([`SpeciesConfig::establish_light_margin`]): whether a newborn of
/// `species` on `site` — the stand one package builds ([`crate::seeds::newborn_stand`]) —
/// would earn at least the margin times its upkeep under the light reaching it through
/// every other stand's taller foliage ([`canopy_light`]), at the site's moisture (`gates`'
/// root-box reading through the species' ramp) and the site's mineral. `visibility` is
/// the site's terrain sky, as the gates read it. Always true for a saprotroph and for a
/// margin of zero.
#[allow(clippy::too_many_arguments)]
pub(crate) fn canopy_pays(
    config: &FloraConfig,
    view: &VoxelView<'_>,
    ground: &[Ground],
    stands: &[Stand],
    site: Site,
    species: Species,
    visibility: f64,
    gates: &Gates,
) -> bool {
    let sc = config.species(species);
    if sc.trophic != Trophic::Photo || !(sc.establish_light_margin > 0.0) {
        return true;
    }
    let newborn = crate::seeds::newborn_stand(config, site, species);
    let l_eff = canopy_light(config, stands, view, visibility, &newborn);
    let mu = gates.mean_water.map_or(0.0, |m| sc.moisture_at(m));
    let n = match ground.binary_search_by_key(&site, |g| g.site) {
        Ok(gi) => ground[gi].mineral,
        Err(_) => config.initial_mineral,
    };
    break_even_cover(sc, newborn.wood, newborn.foliage, l_eff, mu, monod(sc, n))
        >= sc.establish_light_margin
}

/// The farthest any stand's foliage can reach sideways, in whole cells of `voxel_m`: the
/// widest crown radius any species reaches at any size, times its widest layer.
fn shade_reach(config: &FloraConfig, voxel_m: f64) -> i64 {
    Species::ALL
        .iter()
        .map(|&sp| {
            let sc = config.species(sp);
            let layer = sc
                .profile
                .iter()
                .flat_map(|p| p.layers.iter())
                .filter(|l| l.kind.bears_foliage())
                .map(|l| l.radius)
                .fold(0.0f64, f64::max);
            let r = sc.crown_radius_m[0].max(sc.crown_radius_m[1]).max(0.0) / voxel_m;
            (r * layer).ceil() as i64 + 1
        })
        .max()
        .unwrap_or(1)
}

/// **The light a stand would earn on its site now**, `L_eff`: [`light_per_stand`]'s sweep
/// for one receiver that is not (yet) in `stands` — the open sky over its site times what
/// every foliage layer of another site's stand whose top is strictly above each of its
/// layers and whose footprint holds its column lets through, averaged over its layers by
/// their stock, through the species' light response. A stand on its own site (the cover
/// a seedling comes up through, the glowcap under a plant) shades it not at all.
///
/// The stands are `(site, place)`-sorted, so the shaders are the runs of the columns
/// within [`shade_reach`] in `x`, filtered in `z`.
pub(crate) fn canopy_light(
    config: &FloraConfig,
    stands: &[Stand],
    view: &VoxelView<'_>,
    open: f64,
    receiver: &Stand,
) -> f64 {
    let voxel_m = view.config.voxel_m;
    let width = view.config.width;
    let mut mine: Vec<Shade> = Vec::with_capacity(4);
    shade_layers_into(config, receiver, voxel_m, &mut mine);
    let mut through: Vec<f64> = vec![1.0; mine.len()];
    let lowest = mine.iter().map(|l| l.top).fold(f64::INFINITY, f64::min);
    let reach = shade_reach(config, voxel_m);
    let (x0, z0) = (i64::from(receiver.site.x), i64::from(receiver.site.z));
    let w = i64::from(width);
    let xs: Vec<u32> = if 2 * reach + 1 >= w {
        (0..width).collect()
    } else {
        (-reach..=reach).map(|dx| (x0 + dx).rem_euclid(w) as u32).collect()
    };
    // A newcomer that will share its face — a woody seed coming up beside a ground cover —
    // stands in that face's plant's shade; one that replaces it (a cover over a cover, a
    // runner rooting through one) or lands on a bare face does not read the stand it
    // replaces (CONTEST).
    let shares_face = !config.species(receiver.species).ground_cover;
    let mut theirs: Vec<Shade> = Vec::with_capacity(4);
    for x in xs {
        let a = stands.partition_point(|s| s.site.x < x);
        let b = a + stands[a..].partition_point(|s| s.site.x <= x);
        for s in &stands[a..b] {
            if (s.site == receiver.site && !(shares_face && same_face_shades(s, receiver)))
                || (i64::from(s.site.z) - z0).abs() > reach
            {
                continue;
            }
            let sc = config.species(s.species);
            // The envelope first: a crown whose top is not above the receiver's lowest
            // layer shades none of it.
            if f64::from(s.site.y) + sc.crown_height(s.wood, voxel_m) <= lowest {
                continue;
            }
            theirs.clear();
            shade_layers_into(config, s, voxel_m, &mut theirs);
            for (k, layer) in mine.iter().enumerate() {
                for above in &theirs {
                    if above.top <= layer.top {
                        continue;
                    }
                    let dx = wrapped_delta(above.x, layer.x, f64::from(width));
                    let dz = above.z - layer.z;
                    if dx * dx + dz * dz > above.radius * above.radius {
                        continue;
                    }
                    through[k] *= above.attenuation;
                }
            }
        }
    }
    let total: f64 = mine.iter().map(|l| l.stock).sum();
    let mut l = 0.0;
    let mut weighed = 0.0;
    for (k, layer) in mine.iter().enumerate() {
        let wgt = if total > 0.0 { layer.stock } else { layer.share };
        if !(wgt > 0.0) {
            continue;
        }
        l += wgt * open * through[k];
        weighed += wgt;
    }
    let l = if weighed > 0.0 { l / weighed } else { open };
    light_response(config.species(receiver.species), l)
}

/// What each gate of the germination predicate read at one site, and which of them
/// passed: [`Gates::passes`] is the predicate, so a caller that wants to know **why** a
/// site is refused reads the same numbers the model refused it on.
///
/// The reading is instantaneous: it describes the world at the tick it was taken and
/// nothing else. A count of eligible sites is therefore a statement about *that* moment —
/// after a warm-up, at introduction, at observation — and never about a settled habitat
/// unless the water budget says the stores have settled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gates {
    /// Soil voxels in the species' root box. Zero means there is no soil under this site
    /// at all: rock, or a box that falls outside the world.
    pub soil_voxels: usize,
    /// Mean [`available_water`] of the root box: 0 at the wilting point, 1 at
    /// field capacity. `None` for a box with no plant-available band in it at all. A
    /// dark site's cave damp is in it ([`SpeciesConfig::damp_available`]): it is what the
    /// pore gate read.
    pub mean_water: Option<f64>,
    /// Fraction of the root box's voxels at or above the species' `saturated_pore`.
    pub saturated_fraction: f64,
    /// Free water standing on the support face, metres.
    pub water_depth_m: f64,
    /// Sky visibility of the site: **terrain geometry only**, with no canopy in it, which
    /// is the boundary germination light has in this round.
    pub sky_visibility: f64,
    /// Dead wood in the species' **mycelium box**, as the reading was supplied: zero for a
    /// [`Trophic::Photo`] species, which never asks, and zero for a caller that read the
    /// gates off a `VoxelView` alone, which has no ground stocks in it.
    pub dead_wood: f64,
    /// Litter in the same box, on the same terms. It is the **other half of the substrate**
    /// a saprotroph eats, so the gate reads [`Gates::substrate`] and not this field or
    /// `dead_wood` alone: the two are reported apart only so a diagnosis can say which
    /// food a site holds.
    pub litter: f64,
    /// The pore-water gate: `mean_water >= establish_water_min`, on the available-water
    /// scale, and false for a box with no soil in it unless the site is dark enough for
    /// the cave damp to carry it.
    pub pore_ok: bool,
    /// `saturated_fraction <= establish_saturated_max`.
    pub aeration_ok: bool,
    /// `water_depth_m <= drown_depth_m`.
    pub depth_ok: bool,
    /// `sky_visibility >= establish_light_min` — and **always true for a saprotroph**,
    /// which has no light gate at all, because it does not eat light.
    pub light_ok: bool,
    /// `substrate() >= establish_substrate_min` — and **always true for a `Photo`
    /// species**, whose substrate gate is open by construction.
    pub substrate_ok: bool,
    /// The site's darkness for this species ([`SpeciesConfig::darkness`] of
    /// `sky_visibility`): what the cave damp and the dark substrate gate read. Zero for a
    /// species with no dark habitat.
    pub darkness: f64,
    /// Settled standing water on the site or beside it, metres
    /// ([`standing_water_beside`]). Read only for a species with a
    /// `water_depth_min_m`; zero, unread, for every other.
    pub standing_depth_m: f64,
    /// `standing_depth_m >= water_depth_min_m` — and **always true** for a species with
    /// no standing-water requirement.
    pub standing_ok: bool,
}

impl Gates {
    /// The substrate the box holds: `dead_wood + litter`, which is what
    /// [`SpeciesConfig::establish_substrate_min`] is compared against and what a
    /// saprotroph's income is drawn from. A fungus eats both.
    pub fn substrate(&self) -> f64 {
        self.dead_wood + self.litter
    }

    /// The predicate itself: every gate, and nothing else.
    pub fn passes(&self) -> bool {
        self.pore_ok
            && self.aeration_ok
            && self.depth_ok
            && self.light_ok
            && self.substrate_ok
            && self.standing_ok
    }
}

/// How far a **founder-sized adult**'s light income goes toward its own upkeep at a site
/// with this much sky: `assimilation · L_eff(sky) · α / maintenance`, per unit of wood.
/// Both sides are proportional to the stand's wood, so the ratio is the same whatever
/// size the founder is planted at.
///
/// One or more means an adult can hold its own here; below one it cannot, whatever the
/// **seed** gate says — passing a germination threshold does not establish that a large
/// founder can maintain itself (`design/terrain-generation-plan-2026-09-21.md` §5). Every
/// other term in §4.2's income — the mineral Monod, the moisture multiplier, the stress
/// factor — only reduces it, so this is a necessary condition and never a promise.
///
/// A [`Trophic::Saprotroph`] has no light income at all and is not judged on light: it
/// returns infinity, and its substrate gate is the one that binds.
pub fn adult_light_cover(sc: &SpeciesConfig, sky_visibility: f64) -> f64 {
    match sc.trophic {
        Trophic::Saprotroph => f64::INFINITY,
        Trophic::Photo if sc.maintenance > 0.0 => {
            sc.assimilation * light_response(sc, sky_visibility) * sc.alpha / sc.maintenance
        }
        Trophic::Photo => f64::INFINITY,
    }
}

/// The gates at one site for one species, reading sky visibility straight off the view and
/// the substrate as **nothing**, for a caller outside a tick (a harness picking founders, a
/// presenter shading the sites a species could take).
///
/// A `VoxelView` holds no ground stocks, so for a [`Trophic::Saprotroph`] this reports
/// `substrate_ok: false` and refuses; [`crate::FloraView::establishment_gates`] reads the
/// substrate off the flora's own ground.
pub fn establishment_gates(view: &VoxelView<'_>, site: Site, sc: &SpeciesConfig) -> Gates {
    let sky_visibility = view.sky_visibility(site.x as i64, site.y, site.z);
    establishment_gates_with_sky(view, site, sc, sky_visibility, 0.0, 0.0)
}

/// **The germination predicate**, gate by gate: every other entry point reads its sky
/// visibility and substrate and calls this. `sky_visibility` is
/// [`VoxelView::sky_visibility`] (or a cache of it: [`crate::SkyCache`], or a tick's
/// memoized sky); `dead_wood` and `litter` are the species' mycelium box's two stocks,
/// zero for a caller with no ground.
pub fn establishment_gates_with_sky(
    view: &VoxelView<'_>,
    site: Site,
    sc: &SpeciesConfig,
    sky_visibility: f64,
    dead_wood: f64,
    litter: f64,
) -> Gates {
    #[cfg(feature = "profile")]
    cubarium_voxel::profile::add(cubarium_voxel::profile::Count::GatesEvaluated, 1);
    let box_ = root_box(view, site, sc);
    let darkness = sc.darkness(sky_visibility);
    // The cave damp: a dark site reads at least `dark_damp · darkness`, soil or not.
    let mean_water = sc.damp_available(mean_available(view, &box_), darkness);
    let saturated_fraction = saturated_fraction(view, &box_, sc);
    let water_depth_m = view.water_depth_m(site.x as i64, site.y, site.z);
    let standing_depth_m = if sc.water_depth_min_m > 0.0 {
        standing_water_beside(view, site)
    } else {
        0.0
    };
    Gates {
        soil_voxels: box_.len(),
        mean_water,
        saturated_fraction,
        water_depth_m,
        sky_visibility,
        dead_wood,
        litter,
        pore_ok: mean_water.is_some_and(|mean| mean >= sc.establish_water_min),
        aeration_ok: saturated_fraction <= sc.establish_saturated_max,
        depth_ok: water_depth_m <= sc.drown_depth_m,
        light_ok: match sc.trophic {
            Trophic::Photo => sky_visibility >= sc.establish_light_min,
            // No light gate for a saprotroph: `design/theoretical-biosphere-2026-09-16.md`
            // §6's "no light income" applies on the way in as well. A fungus in the dark
            // under a closed canopy is a fungus in its habitat.
            Trophic::Saprotroph => true,
        },
        substrate_ok: match sc.trophic {
            Trophic::Photo => true,
            // Optional legacy seep can admit a dark face without substrate.
            Trophic::Saprotroph => {
                dead_wood + litter >= sc.establish_substrate_min
                    || (!sc.substrate_required && sc.dark_establish_min > 0.0 && darkness >= sc.dark_establish_min)
            }
        },
        darkness,
        standing_depth_m,
        standing_ok: sc.water_depth_min_m <= 0.0 || standing_depth_m >= sc.water_depth_min_m,
    }
}

/// The deepest **settled standing water** on `site`'s own face or beside it, metres:
/// [`VoxelView::standing_depth_m`] on the site, and on
/// the highest support face of each of the four neighbouring columns **at or below** the
/// site's face, when its standing water reaches up to the site's own support voxel —
/// water lapping at the bank, and not a pool at the foot of a cliff beside it.
pub(crate) fn standing_water_beside(view: &VoxelView<'_>, site: Site) -> f64 {
    let c = view.config;
    let x = site.x as i64;
    let mut best = view.standing_depth_m(x, site.y, site.z);
    let bank_m = f64::from(site.y) * c.voxel_m;
    for (dx, dz) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
        let z = site.z as i64 + dz;
        if z < 0 || z >= c.depth as i64 {
            continue;
        }
        let (nx, nz) = (x + dx, z as u32);
        // The neighbour's highest face at or below the site's: the ground beside the
        // bank. A face under that one is under solid and its water cannot be beside it.
        let Some(y) = (0..=site.y).rev().find(|&y| view.is_support(nx, y, nz)) else {
            continue;
        };
        let depth = view.standing_depth_m(nx, y, nz);
        if depth > 0.0 && f64::from(y + 1) * c.voxel_m + depth >= bank_m {
            best = best.max(depth);
        }
    }
    best
}

#[cfg(test)]
#[path = "night_budget_tests.rs"]
mod night_budget_tests;
#[cfg(test)]
#[path = "runner_support_tests.rs"]
mod runner_support_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Species;

    /// **The column walks find the site list's entries in its order** (SPEED-MATURE):
    /// a mycelium box's ground entries found column by column — through the site-sorted
    /// ground, and through a [`GroundIndex`] — are the entries the site list finds, in its
    /// order, so the sums over them are its sums to the bit. On a hand-built ground: sites
    /// at several rows in and around boxes on a small ring, near its wrap and its ends, a
    /// box radius from 0 to 3 and a reach from 0 to 2.
    #[test]
    fn the_column_walks_find_the_site_lists_entries_in_its_order() {
        let world = cubarium_voxel::World::empty(cubarium_voxel::Config {
            width: 11,
            height: 12,
            depth: 7,
            voxel_m: 0.25,
            seed: 3,
            ..cubarium_voxel::Config::default()
        });
        let view = world.view();
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = |n: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % n
        };
        let mut ground: Vec<Ground> = Vec::new();
        for _ in 0..260 {
            let site = Site { x: next(11) as u32, y: next(12) as u32, z: next(7) as u32 };
            if ground.iter().any(|g| g.site == site) {
                continue;
            }
            let mut g = Ground::new(site, 0.1);
            g.litter = next(1000) as f64 * 1.37e-3;
            g.dead_wood = next(1000) as f64 * 0.71e-3;
            ground.push(g);
        }
        ground.sort_by_key(|g| g.site);
        let index = GroundIndex::build(&ground, 11, 7);
        let mut sc = FloraConfig::default().species(Species::Glowcap).clone();
        let (mut found, mut walked) = (Vec::new(), Vec::new());
        let mut boxes = 0;
        for r in 0..=3u32 {
            for reach in 0..=2u32 {
                sc.rooting_radius = r;
                sc.substrate_reach_up_down = reach;
                for _ in 0..40 {
                    let site = Site { x: next(11) as u32, y: next(12) as u32, z: next(7) as u32 };
                    let want: Vec<usize> = mycelium_sites(&view, site, &sc)
                        .into_iter()
                        .filter_map(|s| ground.binary_search_by_key(&s, |g| g.site).ok())
                        .collect();
                    let b = MyceliumBox::of(&view, site, &sc);
                    assert!(!b.may_repeat());
                    b.ground_sorted(&ground, &mut found);
                    b.ground_indexed(&index, &mut walked);
                    let found: Vec<usize> = found.iter().map(|e| e.1).collect();
                    let walked: Vec<usize> = walked.iter().map(|e| e.1).collect();
                    assert_eq!(found, want, "r {r} reach {reach} at {site:?}, by the sorted ground");
                    assert_eq!(walked, want, "r {r} reach {reach} at {site:?}, by the index");
                    let (mut dead_wood, mut litter) = (0.0, 0.0);
                    for &gi in &want {
                        dead_wood += ground[gi].dead_wood;
                        litter += ground[gi].litter;
                    }
                    let got = substrate_pools_in_box(&view, &ground, site, &sc);
                    assert_eq!((got.0.to_bits(), got.1.to_bits()), (dead_wood.to_bits(), litter.to_bits()));
                    boxes += usize::from(want.len() > 1);
                }
            }
        }
        assert!(boxes > 100, "too few boxes held more than one entry: {boxes}");
    }

    #[test]
    fn a_share_is_proportional_and_never_zero_over_zero() {
        let accepted = 0.6;
        let (a, b) = (0.25, 0.75);
        let total = a + b;
        let sa = split_proportional(accepted, a, total);
        let sb = split_proportional(accepted, b, total);
        assert!(
            (sa + sb - accepted).abs() < 1e-15,
            "{sa} + {sb} != {accepted}"
        );
        assert!(
            (sb / sa - 3.0).abs() < 1e-12,
            "three times the demand, three times the share"
        );
        assert_eq!(split_proportional(1.0, 0.0, 0.0), 0.0);
        assert_eq!(split_proportional(0.0, 1.0, 1.0), 0.0);
        // One demander asking for the whole total takes all of it, exactly.
        assert_eq!(split_proportional(0.4, 1.0, 1.0), 0.4);
    }

    /// The aeration target's two ends and its middle, straight off the two species'
    /// placeholders: zero anywhere a species would germinate, one where there is no air
    /// left at all, and in proportion between. Umbrellafrond's ceiling of 1.0 is the
    /// degenerate case, and it means no saturation stress anywhere.
    #[test]
    fn the_aeration_target_is_zero_up_to_the_tolerance_and_one_at_full_saturation() {
        let bloom = SpeciesConfig::bloomcrown();
        assert_eq!(bloom.establish_saturated_max, 0.25);
        assert_eq!(aeration_target(0.0, &bloom), 0.0);
        assert_eq!(
            aeration_target(2.0 / 18.0, &bloom),
            0.0,
            "two saturated voxels of eighteen"
        );
        assert_eq!(
            aeration_target(0.25, &bloom),
            0.0,
            "at the tolerance, not past it"
        );
        assert!((aeration_target(0.5, &bloom) - 1.0 / 3.0).abs() < 1e-15);
        assert_eq!(aeration_target(1.0, &bloom), 1.0);

        let frond = SpeciesConfig::umbrellafrond();
        assert_eq!(frond.establish_saturated_max, 1.0);
        for f in [0.0, 0.5, 0.999, 1.0] {
            assert_eq!(aeration_target(f, &frond), 0.0, "umbrellafrond at f = {f}");
        }
    }

    /// A world of one soil slab holding `pore` of soil's own pore capacity: bedrock at
    /// `y = 0`, soil at `y = 1..=2`, air above, so every column's support face is `y = 2`
    /// in open sky. Built the way the integration fixtures build one — water into the void
    /// first, then the conversion — because `AddWater` will not push pore water into a
    /// voxel that is already solid.
    fn slab(width: u32, pore: f64) -> World {
        let config = cubarium_voxel::Config {
            width,
            height: 8,
            depth: 1,
            voxel_m: 1.0,
            seed: 5,
            ..cubarium_voxel::Config::default()
        };
        let mut w = World::empty(config);
        for x in 0..width as i64 {
            for y in 1..=2u32 {
                let want = pore
                    * w.config().materials.pore_capacity(Material::Soil)
                    * w.config().voxel_volume();
                let got = w.apply(WorldCommand::AddWater {
                    x,
                    y,
                    z: 0,
                    volume_m3: want,
                });
                assert!((got - want).abs() < 1e-12, "the void took {got} of {want}");
                w.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z: 0,
                    material: Material::Soil,
                });
            }
        }
        w
    }

    /// The gates say which one shut. Read on a slab
    /// that passes everything, then on the three ways to fail it that a world can produce
    /// without touching a config: no soil in the root box, a saturated box, and standing
    /// water over the face.
    #[test]
    fn the_gates_are_the_predicate_and_they_say_which_one_shut() {
        let sc = SpeciesConfig::bloomcrown();
        let mut world = slab(4, 0.6);
        let site = Site { x: 1, y: 2, z: 0 };
        let view = world.view();
        let g = establishment_gates(&view, site, &sc);
        assert!(g.passes(), "{g:?}");
        assert_eq!(g.soil_voxels, 6, "three columns of two soil rows: {g:?}");
        assert_eq!(
            (g.pore_ok, g.aeration_ok, g.depth_ok, g.light_ok),
            (true, true, true, true)
        );
        assert_eq!(g.saturated_fraction, 0.0);
        assert_eq!(g.water_depth_m, 0.0);
        assert_eq!(g.sky_visibility, 1.0, "open sky");
        let a = available_water(&Materials::DEFAULT, Material::Soil, 0.6);
        assert!((g.mean_water.expect("soil") - a).abs() < 1e-12);
        drop(view);

        // A site with no soil under it at all: the pore gate shuts, and the saturated
        // fraction of an empty box is zero rather than waterlogged.
        let mut rock = slab(4, 0.6);
        for x in 0..3i64 {
            for y in 1..=2u32 {
                rock.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z: 0,
                    material: Material::Rock,
                });
            }
        }
        let view = rock.view();
        let g = establishment_gates(&view, site, &sc);
        assert_eq!(g.soil_voxels, 0, "{g:?}");
        assert_eq!(g.mean_water, None);
        assert!(!g.pore_ok && g.aeration_ok, "{g:?}");
        assert!(!g.passes());
        drop(view);

        // A wholly saturated root box: the aeration gate shuts and the pore gate does not.
        let wet = slab(4, 0.98);
        let view = wet.view();
        let g = establishment_gates(&view, site, &sc);
        assert!(g.pore_ok && !g.aeration_ok, "{g:?}");
        assert_eq!(g.saturated_fraction, 1.0);
        assert!(!g.passes());
        drop(view);

        // Standing water deeper than the species tolerates: the depth gate shuts.
        world.apply(WorldCommand::AddWater {
            x: 1,
            y: 3,
            z: 0,
            volume_m3: 0.2,
        });
        let view = world.view();
        let g = establishment_gates(&view, site, &sc);
        assert!(g.water_depth_m > sc.drown_depth_m && !g.depth_ok, "{g:?}");
        assert!(g.pore_ok && g.aeration_ok && g.light_ok, "{g:?}");
        assert!(!g.passes());
    }

    /// One draw is one draw: the same world, site and tick give the same index, a
    /// different tick gives a different sequence, and every candidate is reachable.
    #[test]
    fn a_keyed_draw_is_reproducible_and_covers_its_candidates() {
        let once = Rng::keyed(DOMAIN_DISPERSAL, 7, 1234, 99).below(4);
        let again = Rng::keyed(DOMAIN_DISPERSAL, 7, 1234, 99).below(4);
        assert_eq!(once, again, "the same key drew twice");
        let mut seen = [0usize; 4];
        for tick in 0..400u64 {
            seen[Rng::keyed(DOMAIN_DISPERSAL, 7, 1234, tick).below(4)] += 1;
        }
        assert!(
            seen.iter().all(|&n| n > 60),
            "four candidates over 400 ticks: {seen:?}"
        );
        assert_eq!(seen.iter().sum::<usize>(), 400);
    }

    #[test]
    fn the_ramp_is_a_ramp_and_a_degenerate_one_is_a_step() {
        assert_eq!(ramp(0.1, 0.2, 0.6), 0.0);
        assert_eq!(ramp(0.2, 0.2, 0.6), 0.0);
        assert_eq!(ramp(0.6, 0.2, 0.6), 1.0);
        assert_eq!(ramp(0.9, 0.2, 0.6), 1.0);
        assert!((ramp(0.4, 0.2, 0.6) - 0.5).abs() < 1e-15);
        assert_eq!(ramp(0.3, 0.5, 0.5), 0.0);
        assert_eq!(ramp(0.5, 0.5, 0.5), 1.0);
    }

    /// **Two fungi on one log, with unequal demands.** `feed` itself, not the
    /// scalar arithmetic under it: the collect-then-withdraw rule is what makes two
    /// mycelia *share* a log instead of the earlier one in the sweep eating its fill, and
    /// until now only [`split_proportional`] was tested, which is not `feed` and not its
    /// three-currency remainder.
    ///
    /// Two glowcaps on adjacent faces, `W` 0.02 and 0.06, so their demands are `2e-5` and
    /// `6e-5` — one to three — and one shared log on the first one's face holding `4e-5` of
    /// organic matter, **half** of what the two ask for, with a declared `3e-6` of mineral
    /// and `5e-4` of energy so that the three receipts are distinguishable numbers.
    ///
    /// All three currencies split one to three, the pool is left at exactly zero in all
    /// three with no float dust claiming to be a stock, and the last demander takes each
    /// remainder so that what the pool lost and what the stands received are the same
    /// number and not the same number to a few ulps.
    #[test]
    fn two_fungi_share_one_log_in_proportion_to_their_demand() {
        let world = slab(4, 0.6);
        let mut config = FloraConfig::default();
        config.glowcap.substrate_budgeted = false;
        let mut flora = Flora::new(config);
        for (x, wood) in [(1i64, 0.02), (2, 0.06)] {
            assert!(flora.apply(
                &world,
                crate::Command::Seed {
                    x,
                    z: 0,
                    species: Species::Glowcap,
                    wood
                }
            ));
        }
        let log = Site { x: 1, y: 2, z: 0 };
        assert!(flora.deposit(
            log,
            crate::Deposit {
                kind: crate::DepositKind::DeadWood,
                organic: 4e-5,
                mineral: 3e-6,
                energy: 5e-4,
            }
        ));
        // Both boxes hold the one log, and it is the only dead wood in the world.
        let sc = flora.config().species(Species::Glowcap).clone();
        for site in [Site { x: 1, y: 2, z: 0 }, Site { x: 2, y: 2, z: 0 }] {
            let held = flora.view().dead_wood_in_box(&world.view(), site, &sc);
            assert!((held - 4e-5).abs() < 1e-18, "{site:?} sees {held}");
        }

        // One tick's worth of the moisture `drink` would have read: full, so the demands
        // are the rate's own. `feed` is the step under test and nothing else runs.
        let moisture = vec![
            Drink {
                moisture: 1.0,
                taken_m3: 0.0,
                saturated: 0.0,
                darkness: 0.0,
                available: 1.0,
            };
            2
        ];
        let out = feed(&mut flora, &world, &moisture, 1, DT);

        assert_eq!(out.len(), 2);
        // A quarter and three quarters, in every currency.
        for (i, f) in [(0usize, 0.25), (1, 0.75)] {
            assert!(
                (out[i].taken.organic - f * 4e-5).abs() < 1e-20,
                "{i}: organic {:?}",
                out[i]
            );
            assert!(
                (out[i].taken.mineral - f * 3e-6).abs() < 1e-20,
                "{i}: mineral {:?}",
                out[i]
            );
            assert!(
                (out[i].taken.energy - f * 5e-4).abs() < 1e-18,
                "{i}: energy {:?}",
                out[i]
            );
        }
        // Three times the demand, three times the share, and nothing was created: the two
        // receipts sum to exactly what the log held.
        assert_eq!(out[0].taken.organic + out[1].taken.organic, 4e-5);
        assert_eq!(out[0].taken.mineral + out[1].taken.mineral, 3e-6);
        assert_eq!(out[0].taken.energy + out[1].taken.energy, 5e-4);
        let g = flora.view().ground_at(log).expect("the log's site").clone();
        assert_eq!(
            (g.dead_wood, g.dead_wood_mineral, g.dead_wood_energy),
            (0.0, 0.0, 0.0)
        );
        let uptake = flora.view().ledger.substrate_uptake[Species::Glowcap.index()];
        assert_eq!(uptake, 4e-5, "the diagnostic flux is the whole withdrawal");
    }

    /// The full O(n²) sweep the bucketed light replaced, verbatim: the reference
    /// [`light_per_stand`] must equal to the bit.
    fn light_full_sweep(flora: &mut Flora, world: &World) -> Vec<f64> {
        let Flora {
            config,
            stands,
            sky,
            ..
        } = flora;
        let view = world.view();
        let voxel_m = view.config.voxel_m;
        struct Shade {
            x: f64,
            z: f64,
            top: f64,
            radius: f64,
            stock: f64,
            share: f64,
            porosity: f64,
            area_m2: f64,
        }
        let crowns: Vec<Vec<Shade>> = stands
            .iter()
            .map(|s| {
                crate::layers_of(config, s, voxel_m)
                    .into_iter()
                    .filter(|l| l.kind.bears_foliage())
                    .map(|l| Shade {
                        x: s.site.x as f64,
                        z: s.site.z as f64,
                        top: l.band_v[1],
                        radius: l.radius_v,
                        stock: l.stock,
                        share: l.share,
                        porosity: l.porosity,
                        area_m2: l.area_m2,
                    })
                    .collect()
            })
            .collect();
        let envelope: Vec<(f64, f64)> = crowns
            .iter()
            .map(|layers| {
                layers
                    .iter()
                    .fold((f64::NEG_INFINITY, 0.0f64), |(t, r), l| {
                        (t.max(l.top), r.max(l.radius))
                    })
            })
            .collect();
        let width = view.config.width as f64;
        let mut out = Vec::with_capacity(stands.len());
        for (i, stand) in stands.iter().enumerate() {
            let open = sky_at(sky, &view, stand.site);
            let mine = &crowns[i];
            let total: f64 = mine.iter().map(|l| l.stock).sum();
            let weight_by_stock = total > 0.0;
            let mut l = 0.0;
            let mut weighed = 0.0;
            for layer in mine {
                let w = if weight_by_stock {
                    layer.stock
                } else {
                    layer.share
                };
                if !(w > 0.0) {
                    continue;
                }
                let mut here = open;
                for (j, other) in crowns.iter().enumerate() {
                    // A fungus's exposure has its own face's plant over it (GLOWCAP-2).
                    let fungus = config.species(stand.species).trophic == Trophic::Saprotroph;
                    if j == i || (stands[j].site == stand.site && !fungus && !super::same_face_shades(&stands[j], stand)) {
                        continue;
                    }
                    let (top, radius) = envelope[j];
                    if top <= layer.top {
                        continue;
                    }
                    let sx = wrapped_delta(other.first().map_or(0.0, |o| o.x), layer.x, width);
                    let sz = other.first().map_or(0.0, |o| o.z) - layer.z;
                    if sx * sx + sz * sz > radius * radius {
                        continue;
                    }
                    for above in other {
                        if above.top <= layer.top {
                            continue;
                        }
                        let dx = wrapped_delta(above.x, layer.x, width);
                        let dz = above.z - layer.z;
                        if dx * dx + dz * dz > above.radius * above.radius {
                            continue;
                        }
                        here *= (-config.shade_k_per_m2 * (1.0 - above.porosity) * above.stock
                            / above.area_m2)
                            .exp();
                    }
                }
                l += w * here;
                weighed += w;
            }
            let l = if weighed > 0.0 { l / weighed } else { open };
            out.push(if config.species(stand.species).trophic == Trophic::Saprotroph {
                l
            } else {
                light_response(config.species(stand.species), l)
            });
        }
        out
    }

    /// A soil plain `width × depth` at 0.125 m with a ragged skyline, densely seeded with
    /// every species at keyed sizes between `alive_min` and `wood_max`.
    fn crowded(width: u32, depth: u32, fill: f64, seed: u64) -> (Flora, World) {
        crowded_on(width, depth, fill, seed, 2)
    }

    /// [`crowded`] with the soil `base` rows deep before its random top: a deep one puts
    /// the root boxes in several [`DRINK_BAND`]s.
    fn crowded_on(width: u32, depth: u32, fill: f64, seed: u64, base: u32) -> (Flora, World) {
        let voxel_m = 0.125;
        let mut world = World::empty(cubarium_voxel::Config {
            width,
            height: 40,
            depth,
            voxel_m,
            seed: 5,
            ..cubarium_voxel::Config::default()
        });
        let mut rng = Rng::keyed(99, seed, 0, 0);
        for z in 0..depth {
            for x in 0..width as i64 {
                let top = base + rng.below(4) as u32;
                for y in 1..=top {
                    let pore = 0.05 + 0.9 * rng.unit();
                    let cap = world.config().materials.pore_capacity(Material::Soil)
                        * world.config().voxel_volume();
                    world.apply(WorldCommand::AddWater {
                        x,
                        y,
                        z,
                        volume_m3: pore * cap,
                    });
                    world.apply(WorldCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        let mut flora = Flora::new(FloraConfig::for_voxel_size(voxel_m));
        for z in 0..depth {
            for x in 0..width as i64 {
                if rng.unit() >= fill {
                    continue;
                }
                let species = Species::ALL[rng.below(Species::ALL.len())];
                let sc = flora.config.species(species).clone();
                let wood = sc.alive_min + rng.unit() * (sc.wood_max - sc.alive_min);
                flora.apply(
                    &world,
                    crate::Command::Seed {
                        x,
                        z,
                        species,
                        wood,
                    },
                );
            }
        }
        (flora, world)
    }

    #[test]
    fn the_bucketed_light_is_the_full_sweep() {
        // A wide plain (buckets, the x seam), a narrow one (footprints wider than the
        // world), and a browsed one (layer stocks moved off their capacities).
        // The first plain holds over a thousand stands, so four threads split it.
        for (width, depth, fill, seed) in [(64, 24, 0.7, 1), (6, 5, 0.9, 2), (33, 9, 0.5, 3)] {
            let (mut flora, world) = crowded(width, depth, fill, seed);
            assert!(
                flora.stands.len() > (width * depth) as usize / 3,
                "a crowded plain"
            );
            if seed == 3 {
                let sites: Vec<Site> = flora.stands.iter().map(|s| s.site).step_by(2).collect();
                for site in sites {
                    let _ = flora.take_foliage(site, 0.01);
                }
            }
            let mut reference = flora.clone();
            let full = light_full_sweep(&mut reference, &world);
            for threads in [1, 4] {
                let bucketed = light_per_stand(&flora.config, &flora.stands, &mut flora.sky.clone(), &world, &[], threads).0;
                assert_eq!(bucketed, full, "{width}x{depth}, {threads} threads");
            }
            assert!(
                full.iter()
                    .any(|&l| l < 0.99 * full.iter().cloned().fold(0.0, f64::max))
            );
        }
    }

    /// The sorted sweep the scratch [`drink`] replaced, verbatim.
    fn drink_sorted_sweep(flora: &mut Flora, world: &mut World) -> Vec<Drink> {
        let mut out = vec![Drink::default(); flora.stands.len()];
        let mut wants: Vec<(usize, usize, f64)> = Vec::new();
        {
            let view = world.view();
            for (i, stand) in flora.stands.iter().enumerate() {
                let sc = flora.config.species(stand.species);
                let box_ = root_box(&view, stand.site, sc);
                out[i].moisture = moisture_of(&view, &box_, sc);
                if sc.water_depth_min_m > 0.0
                    && standing_water_beside(&view, stand.site) < sc.water_depth_min_m
                {
                    out[i].moisture = 0.0;
                }
                out[i].saturated = saturated_fraction(&view, &box_, sc);
                let demand =
                    (sc.transpiration_m3_per_s * stand.foliage * out[i].moisture * DT).max(0.0);
                if demand <= 0.0 || box_.is_empty() {
                    continue;
                }
                let stock: f64 = box_.iter().map(|&v| drinkable_m3(&view, v)).sum();
                if stock <= 0.0 {
                    continue;
                }
                for &v in box_.iter() {
                    let share = demand * drinkable_m3(&view, v) / stock;
                    if share > 0.0 {
                        wants.push((v, i, share));
                    }
                }
            }
        }
        wants.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut at = 0;
        while at < wants.len() {
            let voxel = wants[at].0;
            let mut end = at;
            let mut total = 0.0;
            while end < wants.len() && wants[end].0 == voxel {
                total += wants[end].2;
                end += 1;
            }
            let (x, y, z) = world.config().coords(voxel);
            let volume = total.min(drinkable_m3(&world.view(), voxel));
            let accepted = if volume > 0.0 {
                -world.apply(WorldCommand::WithdrawPore {
                    x: x as i64,
                    y,
                    z,
                    volume_m3: volume,
                })
            } else {
                0.0
            };
            flora.ledger.transpired_m3 += accepted;
            for &(_, stand, want) in &wants[at..end] {
                out[stand].taken_m3 += split_proportional(accepted, want, total);
            }
            at = end;
        }
        out
    }

    #[test]
    fn the_scratch_drink_is_the_sorted_sweep() {
        // The first plain holds over a thousand stands, so four threads split it.
        for (width, depth, fill, seed, threads) in [
            (64, 24, 0.7, 4, 4),
            (64, 24, 0.7, 4, 1),
            (6, 5, 0.9, 5, 1),
            (33, 9, 0.5, 6, 2),
        ] {
            let (mut flora, mut world) = crowded(width, depth, fill, seed);
            let (mut ref_flora, mut ref_world) = (flora.clone(), world.clone());
            // Twice, so the second call runs on the scratch the first one left behind.
            for round in 0..2 {
                let got = drink(&mut flora, &mut Target::Now(&mut world), threads, DT);
                let want = drink_sorted_sweep(&mut ref_flora, &mut ref_world);
                assert_eq!(got.len(), want.len());
                assert!(want.iter().any(|d| d.taken_m3 > 0.0), "stands drank");
                for (g, w) in got.iter().zip(&want) {
                    assert_eq!((g.moisture, g.saturated), (w.moisture, w.saturated));
                    assert!(
                        (g.taken_m3 - w.taken_m3).abs() <= 1e-12 * w.taken_m3.abs(),
                        "round {round}: {} vs {}",
                        g.taken_m3,
                        w.taken_m3
                    );
                }
                assert_eq!(flora.ledger.transpired_m3, ref_flora.ledger.transpired_m3);
                assert_eq!(world.view().pore, ref_world.view().pore, "the same withdrawals");
                assert_eq!(
                    world.view().ledger.transpiration_out,
                    ref_world.view().ledger.transpiration_out
                );
            }
        }
    }

    /// The overlapped tick's plan, split over threads, is the one-thread plan: the same
    /// reads, the same per-voxel totals and caps, so the same withdrawals once settled.
    #[test]
    fn the_split_plan_is_the_serial_plan() {
        for (seed, threads) in [(21u64, 2usize), (22, 4)] {
            // Soil thirty rows deep: the root boxes span three bands.
            let (flora, world) = crowded_on(64, 24, 0.7, seed, 30);
            let bands: std::collections::BTreeSet<usize> = flora
                .stands
                .iter()
                .map(|s| {
                    world
                        .config()
                        .index(i64::from(s.site.x), s.site.y, s.site.z)
                        / DRINK_BAND
                })
                .collect();
            assert!(bands.len() >= 2, "the boxes span bands {bands:?}");
            assert!(
                flora.stands.len() >= 2 * MIN_CHUNK,
                "enough stands to split"
            );
            let (mut one, mut split) = (flora.clone(), flora.clone());
            let a = drink(&mut one, &mut Target::Planned(&world), 1, DT);
            let b = drink(&mut split, &mut Target::Planned(&world), threads, DT);
            assert!(a.iter().any(|d| d.taken_m3 > 0.0), "stands drank");
            for (x, y) in a.iter().zip(&b) {
                assert_eq!(
                    (x.moisture, x.saturated, x.taken_m3),
                    (y.moisture, y.saturated, y.taken_m3)
                );
            }
            let (mut wa, mut wb) = (world.clone(), world.clone());
            settle_drink(&mut one, &mut wa);
            settle_drink(&mut split, &mut wb);
            assert_eq!(
                wa.view().pore,
                wb.view().pore,
                "{threads} threads: the same withdrawals"
            );
            assert_eq!(one.ledger.transpired_m3, split.ledger.transpired_m3);
            assert_eq!(
                wa.view().ledger.transpiration_out,
                wb.view().ledger.transpiration_out
            );
        }
    }

    /// The sky read for every stand in one pass fills the cache with exactly the entries,
    /// in exactly the order, that one lookup per stand did — from empty and from a cache
    /// that already holds every other site.
    #[test]
    fn the_sky_pass_fills_the_cache_the_lookups_did() {
        let (flora, world) = crowded(64, 24, 0.7, 31);
        let view = world.view();
        let mut want = Vec::new();
        let open: Vec<f64> = flora
            .stands
            .iter()
            .map(|s| sky_at(&mut want, &view, s.site))
            .collect();
        for partial in [false, true] {
            let mut cache: Vec<(Site, f64)> = if partial {
                want.iter().copied().step_by(2).collect()
            } else {
                Vec::new()
            };
            let got = sky_all(&mut cache, &view, &flora.stands, 4);
            assert_eq!(got, open, "partial {partial}: the values");
            assert_eq!(cache, want, "partial {partial}: the cache");
        }
    }

    /// A crowded plain whose every stand's site holds a log and litter: about one stand in
    /// nine is a glowcap, and over a thousand stands in all, so four threads split the feed.
    fn crowded_with_dead_pools(seed: u64) -> (Flora, World) {
        let (mut flora, world) = crowded(64, 24, 0.7, seed);
        let sites: Vec<Site> = flora.stands.iter().map(|s| s.site).collect();
        let mut rng = Rng::keyed(98, seed, 0, 0);
        for site in sites {
            for kind in [crate::DepositKind::DeadWood, crate::DepositKind::Litter] {
                let organic = 1e-5 * (0.1 + rng.unit());
                assert!(flora.deposit(
                    site,
                    crate::Deposit {
                        kind,
                        organic,
                        mineral: 0.05 * organic,
                        energy: 10.0 * organic,
                    }
                ));
            }
        }
        (flora, world)
    }

    /// The feed's wants gathered over threads and filed by pool are the one-thread sorted
    /// wants: every fungus gets the same substrate, every pool gives up the same amount.
    #[test]
    fn the_split_feed_is_the_serial_feed() {
        for seed in [41u64, 42] {
            let (flora, world) = crowded_with_dead_pools(seed);
            let fungi = flora
                .stands
                .iter()
                .filter(|s| flora.config.species(s.species).trophic == Trophic::Saprotroph)
                .count();
            assert!(
                fungi > 20 && flora.stands.len() >= 2 * MIN_CHUNK,
                "{fungi} fungi"
            );
            let moisture: Vec<Drink> = (0..flora.stands.len())
                .map(|i| Drink {
                    moisture: 0.2 + 0.08 * (i % 10) as f64,
                    ..Drink::default()
                })
                .collect();
            let (mut one, mut split) = (flora.clone(), flora.clone());
            let a = feed(&mut one, &world, &moisture, 1, DT);
            let b = feed(&mut split, &world, &moisture, 4, DT);
            assert!(a.iter().any(|s| s.taken.organic > 0.0), "fungi fed");
            for (x, y) in a.iter().zip(&b) {
                assert_eq!(
                    (x.taken.organic, x.taken.mineral, x.taken.energy, x.gross),
                    (y.taken.organic, y.taken.mineral, y.taken.energy, y.gross)
                );
            }
            for (g, h) in one.ground.iter().zip(&split.ground) {
                assert_eq!(
                    (
                        g.dead_wood,
                        g.dead_wood_mineral,
                        g.dead_wood_energy,
                        g.litter
                    ),
                    (
                        h.dead_wood,
                        h.dead_wood_mineral,
                        h.dead_wood_energy,
                        h.litter
                    )
                );
            }
            assert_eq!(one.ledger.substrate_uptake, split.ledger.substrate_uptake);
        }
    }

    /// Growth split over threads is one-thread growth to the bit: every stand, every ground
    /// stock, every ledger line, and the same deaths — each stand's ledger lines are folded
    /// in stand order, in the order it booked them. Light, water and substrate vary by stand
    /// so every branch runs: rich and starving stands, fungi on logs, some dying.
    #[test]
    fn the_split_growth_is_the_serial_growth() {
        for (seed, threads) in [(51u64, 2usize), (52, 4)] {
            let (mut flora, world) = crowded_with_dead_pools(seed);
            // Some sites with no ground yet, so the growth has slots to make.
            let bare: Vec<Site> = flora.stands.iter().map(|s| s.site).step_by(5).collect();
            flora.ground.retain(|g| !bare.contains(&g.site));
            let n = flora.stands.len();
            assert!(n >= 2 * MIN_CHUNK);
            // One stand in seven on the edge of death with nothing in reserve.
            for s in flora.stands.iter_mut().step_by(7) {
                let alive_min = flora.config.species(s.species).alive_min;
                s.wood = alive_min * (1.0 + 1e-9);
                s.reserve = 0.0;
            }
            let light: Vec<f64> = (0..n).map(|i| [0.0, 0.3, 1.0][i % 3]).collect();
            let drink: Vec<Drink> = (0..n)
                .map(|i| Drink {
                    moisture: [0.0, 0.5, 1.0, 0.9][i % 4],
                    taken_m3: 1e-9 * i as f64,
                    saturated: [0.0, 0.7][i % 2],
                    darkness: 0.0,
                    available: 1.0,
                })
                .collect();
            let substrate = feed(&mut flora, &world, &drink, 1, DT);
            let (mut one, mut split) = (flora.clone(), flora.clone());
            let climate = Climate {
                daylight: 1.0,
                upkeep: 1.0,
                night: 1.0,
                to_dawn_s: None,
                draw_key: 0,
                dt: DT,
            };
            grow(&mut one, &world, &light, climate, &drink, &substrate, 1);
            grow(&mut split, &world, &light, climate, &drink, &substrate, threads);
            assert!(one.stands.len() < n, "some stands died");
            assert_eq!(one.stands.len(), split.stands.len());
            for (a, b) in one.stands.iter().zip(&split.stands) {
                assert_eq!(
                    (
                        a.site,
                        a.wood,
                        a.foliage,
                        a.reserve,
                        a.mineral,
                        a.layer_stock
                    ),
                    (
                        b.site,
                        b.wood,
                        b.foliage,
                        b.reserve,
                        b.mineral,
                        b.layer_stock
                    )
                );
                assert_eq!(
                    (a.light, a.moisture, a.aeration_stress),
                    (b.light, b.moisture, b.aeration_stress)
                );
            }
            assert_eq!(one.ground.len(), split.ground.len());
            for (g, h) in one.ground.iter().zip(&split.ground) {
                assert_eq!(
                    (
                        g.site,
                        g.mineral,
                        g.litter,
                        g.litter_energy,
                        g.litter_mineral
                    ),
                    (
                        h.site,
                        h.mineral,
                        h.litter,
                        h.litter_energy,
                        h.litter_mineral
                    )
                );
                assert_eq!(
                    (
                        g.dead_wood,
                        g.dead_wood_mineral,
                        g.dead_wood_energy,
                        g.carrion
                    ),
                    (
                        h.dead_wood,
                        h.dead_wood_mineral,
                        h.dead_wood_energy,
                        h.carrion
                    )
                );
            }
            let (l, m) = (&one.ledger, &split.ledger);
            assert_eq!(
                (l.fixed_in, l.light_in, l.respired_out, l.heat_out),
                (m.fixed_in, m.light_in, m.respired_out, m.heat_out)
            );
            assert_eq!(format!("{l:?}"), format!("{m:?}"), "every ledger line");
        }
    }

    /// Drowning split over threads kills the same stands, booked the same way.
    #[test]
    fn the_split_drowning_is_the_serial_drowning() {
        let (flora, mut world) = crowded(64, 24, 0.7, 71);
        // Flood one column in three a cell deep over its face.
        let sites: Vec<Site> = flora.stands.iter().map(|s| s.site).step_by(3).collect();
        for s in sites {
            world.apply(WorldCommand::AddWater {
                x: i64::from(s.x),
                y: s.y + 1,
                z: s.z,
                volume_m3: world.config().voxel_volume(),
            });
        }
        let (mut one, mut split) = (flora.clone(), flora.clone());
        drown(&mut one, &world, 1);
        drown(&mut split, &world, 4);
        assert!(one.stands.len() < flora.stands.len(), "some drowned");
        let sites = |f: &Flora| f.stands.iter().map(|s| s.site).collect::<Vec<_>>();
        assert_eq!(sites(&one), sites(&split));
        assert_eq!(format!("{:?}", one.ledger), format!("{:?}", split.ledger));
        assert_eq!(format!("{:?}", one.ground), format!("{:?}", split.ground));
    }

    #[test]
    fn the_root_box_is_the_indexed_walk_across_the_seam() {
        for (width, depth, seed) in [(40, 14, 7), (6, 5, 8), (3, 4, 9)] {
            let (flora, world) = crowded(width, depth, 0.0, seed);
            let view = world.view();
            let c = view.config;
            let mut got = Vec::new();
            for species in Species::ALL {
                let sc = flora.config.species(species);
                for z in 0..depth {
                    for x in 0..width {
                        let Some(site) = crate::highest_support(&view, i64::from(x), z) else {
                            continue;
                        };
                        let span = sc.rooting_depth.min(site.y + 1);
                        let r = sc.rooting_radius as i64;
                        let mut want = Vec::new();
                        for dz in -r..=r {
                            let zz = site.z as i64 + dz;
                            if span == 0 || zz < 0 || zz >= c.depth as i64 {
                                continue;
                            }
                            for y in site.y + 1 - span..=site.y {
                                for dx in -r..=r {
                                    let i = c.index(site.x as i64 + dx, y, zz as u32);
                                    if view.material[i] == Material::Soil {
                                        want.push(i);
                                    }
                                }
                            }
                        }
                        root_box_into(&view, site, sc, &mut got);
                        assert_eq!(got, want, "{species:?} at {site:?} on {width}x{depth}");
                    }
                }
            }
        }
    }

    #[test]
    fn one_compaction_and_one_merge_are_the_removes_and_inserts_they_replace() {
        let (flora, _) = crowded(24, 9, 0.8, 11);
        let all = flora.stands.clone();
        assert!(all.len() > 100);
        // Every third stand dies; the rest keep their order.
        let doomed: Vec<usize> = (0..all.len()).step_by(3).collect();
        let mut one_by_one = all.clone();
        for &i in doomed.iter().rev() {
            one_by_one.remove(i);
        }
        let mut compacted = all.clone();
        remove_at(&mut compacted, &doomed);
        assert_eq!(compacted, one_by_one);

        // The dead are born again, handed over out of order, in batches of one and many.
        let born: Vec<Stand> = doomed.iter().rev().map(|&i| all[i]).collect();
        let mut merged = compacted.clone();
        insert_sorted(&flora.config, &mut merged, born[..1].to_vec());
        insert_sorted(&flora.config, &mut merged, born[1..].to_vec());
        assert_eq!(merged, all);
        insert_sorted(&flora.config, &mut merged, Vec::new());
        assert_eq!(merged, all);
    }

    #[test]
    fn the_ground_index_finds_what_the_binary_search_finds() {
        let (mut flora, world) = crowded(20, 7, 0.5, 12);
        // Ground on stand sites, plus litter dropped on sites with no stand.
        for (x, z) in [(0i64, 0u32), (19, 6), (7, 3), (7, 4)] {
            let site = crate::highest_support(&world.view(), x, z).expect("a face");
            ground_slot(&mut flora.ground, site);
        }
        let c = world.view().config;
        let index = GroundIndex::build(&flora.ground, c.width, c.depth);
        for x in 0..c.width + 1 {
            for z in 0..c.depth + 1 {
                for y in 0..c.height {
                    let site = Site { x, y, z };
                    assert_eq!(
                        index.get(site),
                        flora.ground.binary_search_by_key(&site, |g| g.site).ok(),
                        "{site:?}"
                    );
                }
            }
        }
    }
}

/// One tick of every stand's defence relaxing toward zero at its species' half-life.
fn decay_defences(flora: &mut Flora, dt: f64) {
    for stand in flora.stands.iter_mut().filter(|s| s.defence > 0.0) {
        let half = flora.config.species(stand.species).defence_half_life_s;
        if half > 0.0 {
            stand.defence *= 0.5f64.powf(dt / half);
            if stand.defence < 1e-12 {
                stand.defence = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod clock_tests {
    use super::*;

    /// A per-second decay converted for a step of `n` ticks is the per-tick one applied
    /// `n` times: the expected survivors after any span are the same at any step size.
    #[test]
    fn a_decay_share_over_n_ticks_is_the_per_tick_share_compounded() {
        for rate in [1e-6, 1e-3, 0.05, 2.0] {
            for n in [1u64, 2, 5, 20] {
                let coarse = 1.0 - decay_share(rate, span_dt(n));
                let fine = (1.0 - decay_share(rate, span_dt(1))).powi(n as i32);
                assert!((coarse - fine).abs() <= 1e-12 * fine.max(1e-300), "rate {rate}, n {n}: {coarse} vs {fine}");
            }
        }
        assert_eq!(decay_share(0.0, 1.0), 0.0);
        assert_eq!(decay_share(1.0, 0.0), 0.0);
    }

    /// Over a run, a periodic check is due in exactly as many coarse windows as there are
    /// ticks it falls on (for windows no longer than its period), and the film's steps
    /// add up to the seconds crossed.
    #[test]
    fn periodic_checks_fire_once_per_period_on_any_window() {
        let ticks = 3_000u64;
        for period in [1u64, 3, 7, 20, 600] {
            // Keys whose sum with the tick does not wrap within the run (a wrap breaks the
            // period for the per-tick check too, once in 2^64 ticks).
            for key in [0u64, 1, 5, 0x5EED_1234_5678_9ABC] {
                let per_tick = (1..=ticks).filter(|&t| due_in_window(key, t, 1, period)).count();
                for span in [1u64, 2, 5, 7] {
                    if span > period {
                        continue;
                    }
                    let windows = (1..=ticks / span)
                        .filter(|&k| due_in_window(key, k * span, span, period))
                        .count();
                    let last = ticks / span * span;
                    let per_tick_to_last = (1..=last).filter(|&t| due_in_window(key, t, 1, period)).count();
                    assert_eq!(windows, per_tick_to_last, "period {period} key {key} span {span}");
                }
                assert!(per_tick >= (ticks / period) as usize);
            }
        }
        for span in [1u64, 3, 5, 20, 45] {
            let total: u64 = (1..=600 / span).map(|k| crate::film::film_steps(k * span, span)).sum();
            assert_eq!(total, (600 / span * span) / crate::film::FILM_EVERY, "span {span}");
        }
    }
}

#[cfg(test)]
#[path = "seedling_reflush_tests.rs"]
mod seedling_reflush_tests;
