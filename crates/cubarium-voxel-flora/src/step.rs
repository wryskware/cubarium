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
//!    one read, for step 6's aeration stress.
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
//!    arithmetic as three passes over every stand.
//! 7. **Decomposition** (§5), on the stocks the site held *before* this tick: litter,
//!    dead wood and carrion respire their organic matter out of the system and release
//!    their mineral to the site's pool at the same fraction; each pool's energy leaves as
//!    heat at its own current density.
//! 8. **The seed bank.** Every site with no stand holds a local lottery among the species
//!    whose bank there holds at least one whole package and which pass that species'
//!    establishment predicate, weighted by the packages each holds; the winner spends
//!    exactly **one** package out of its oldest bins and every other bank stays. This runs
//!    *after* the deaths of step 6, so a gap opened this tick can be filled this tick.
//!    **Then** every arrival bin still banked pays its attrition into litter and falls to
//!    litter whole once its **start** is past its species' `seed_max_age_s`. Germination
//!    before decay, so that one package — exactly one minimum viable stand's material —
//!    is one recruit and not 0.1 % short of one.
//! 9. **Propagules** (§4.8), from one snapshot of donors and recipients. A package lands
//!    as a seed cohort, on a support face within the donor's `hop` whether it is occupied
//!    or not: the bank waits for the gap. A landing joins the arrival bin whose window
//!    covers this tick — creating it if it is not there — so the age of what is already
//!    banked does not move, and the bank is bounded by construction.
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
//! **What waits is the organic throughput, and not every currency** (Astra R8.5). The
//! snapshot stores organic *amounts*, and [`decompose_pool`] then takes the mineral and the
//! energy at the pool's **current** density, so material parcels are not age-isolated:
//! one old unit holding no mineral plus one unit shed inside the tick holding one, at a
//! decomposition step of half the old stock, releases `1.0 · 0.5/2.0` = **0.25 of mineral
//! immediately**. That is the inherited well-mixed-pool rule, it conserves every currency
//! exactly, and it is pinned by
//! `tests/round5b.rs::decomposition_delays_organic_matter_and_not_the_mineral_of_a_mixed_pool`
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
//! Round 3's first correction: the two are separate currencies (`lib.rs`'s `FloraLedger`).
//! Assimilation creates **organic matter** from light (`fixed_in`); every respiration —
//! maintenance, construction `c_g`, reflush, a capped income's leftover, decomposition —
//! destroys organic matter and books it out (`respired_out`, energy to heat) and moves
//! **no** mineral. Mineral only ever moves between stocks: the site's pool pays
//! `n_tissue` per unit of new tissue, tissue that dies or falls carries its mineral into
//! litter and dead wood, and decomposition hands it back to the pool. So respiring a
//! kilogram of wood no longer produces a kilogram of fertilizer.
//!
//! A stand's mineral is its own stock and not `n_tissue · O`: respiration takes organic
//! matter and leaves mineral behind. Every transfer out of a stand therefore takes the
//! same *fraction* of its mineral as of its organic matter ([`pull_mineral`]).
//!
//! Every clamp is a `min` against the stock it reads, so nothing here can go negative.

use cubarium_voxel::{Command as WorldCommand, DT, Material, VoxelView, World};

use crate::{
    Flora, FloraConfig, FloraLedger, Ground, SeedCohort, Site, Species, SpeciesConfig, Stage,
    Stand, Taken, Trophic,
};

/// Stream keys, so two draws in one tick cannot be the same draw. One per rule that
/// draws.
const DOMAIN_DISPERSAL: u64 = 1;
const DOMAIN_GERMINATION: u64 = 2;
const DOMAIN_FALL: u64 = 3;

/// A deterministic scalar stream (splitmix64), keyed by the values that **identify** a
/// draw rather than seeded from stored state: the same world, the same place and the same
/// tick always produce the same numbers, and nothing about iteration or storage order can
/// reach them. No clock and no thread state, like the core generator's own stream.
struct Rng(u64);

impl Rng {
    fn keyed(domain: u64, a: u64, b: u64, c: u64) -> Rng {
        let mut state = 0u64;
        for part in [domain, a, b, c] {
            state = mix(state ^ part);
        }
        Rng(state)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.0)
    }

    /// A uniform index below `n`, which must be positive. Modulo, whose bias against
    /// `2^64` is 1e-18 for the handful of candidates anything here draws among.
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// splitmix64's finalizer: the avalanche that makes neighbouring keys independent.
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub(crate) fn step(flora: &mut Flora, world: &mut World) {
    // The tick counter moves **first**, so that `flora.tick` is the tick this step
    // produces: the one whose state the caller will read when the step returns. Everything
    // dated inside a tick — an arrival bin's start, a bin's age, the germination lottery's
    // seed — then uses one clock reading, and a bin's age at the end of the tick it landed
    // in is zero rather than minus one.
    flora.tick += 1;
    // This tick's delivery receipts start empty: they are **observation and not state**, they
    // describe only the tick that is about to run, and a caller that never reads them must not
    // accumulate them (Astra R10.3, `DeliveryReceipt`).
    flora.deliveries.clear();
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

    cubarium_voxel::voxel_phase!(FloraStep, {
        cubarium_voxel::voxel_phase!(Prune, { prune_unsupported(flora, world) });
        cubarium_voxel::voxel_phase!(SkyCache, { refresh_sky_cache(flora, world) });
        cubarium_voxel::voxel_phase!(Drown, { drown(flora, world) });

        let light = cubarium_voxel::voxel_phase!(Light, { light_per_stand(flora, world) });
        let moisture = cubarium_voxel::voxel_phase!(Drink, { drink(flora, world) });
        let substrate = cubarium_voxel::voxel_phase!(Feed, { feed(flora, world, &moisture) });
        cubarium_voxel::voxel_phase!(Grow, {
            grow(flora, world, &light, &moisture, &substrate)
        });
        cubarium_voxel::voxel_phase!(Decompose, { decompose(flora, &pre) });
        cubarium_voxel::voxel_phase!(SeedBank, { seed_bank(flora, world) });
        cubarium_voxel::voxel_phase!(Propagate, { propagate(flora, world) });
        #[cfg(feature = "profile")]
        {
            use cubarium_voxel::profile::{Count, add};
            add(Count::Stands, flora.stands.len() as u64);
            add(Count::GroundSites, flora.ground.len() as u64);
        }
    });
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

    let mut kept = Vec::with_capacity(stands.len());
    for stand in stands.iter() {
        if supported(&stand.site) {
            kept.push(*stand);
        } else {
            // The parcel is organic matter this layer holds, so it is booked out with the
            // stand that was saving it.
            let organic = stand.material();
            ledger.removed_organic_out += organic;
            ledger.removed_mineral_out += stand.mineral;
            ledger.removed_energy_out += config.species(stand.species).energy_density * organic;
        }
    }
    if kept.len() != stands.len() {
        *stands = kept;
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
            ledger.removed_organic_out += g.litter + g.dead_wood + g.carrion;
            ledger.removed_mineral_out +=
                g.mineral + g.litter_mineral + g.dead_wood_mineral + g.carrion_mineral;
            ledger.removed_energy_out += g.litter_energy + g.dead_wood_energy + g.carrion_energy;
            // The seed bank goes out with the ground it sat on: a cohort whose support
            // face is gone has nowhere to germinate, and it is booked as removed rather
            // than dropped.
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

fn sky_at(cache: &mut Vec<(Site, f64)>, view: &VoxelView<'_>, site: Site) -> f64 {
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

// ----------------------------------------------------------------- 3. drowning

fn drown(flora: &mut Flora, world: &World) {
    let Flora {
        config,
        stands,
        ground,
        ledger,
        ..
    } = flora;
    let view = world.view();
    let mut doomed: Vec<usize> = Vec::new();
    for (i, stand) in stands.iter().enumerate() {
        let limit = config.species(stand.species).drown_depth_m;
        if view.water_depth_m(stand.site.x as i64, stand.site.y, stand.site.z) > limit {
            doomed.push(i);
        }
    }
    for &i in doomed.iter().rev() {
        let stand = stands[i];
        die(config, &view, &stand, ground, ledger);
        stands.remove(i);
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
fn light_per_stand(flora: &mut Flora, world: &World) -> Vec<f64> {
    let Flora {
        config,
        stands,
        sky,
        ..
    } = flora;
    let view = world.view();
    let voxel_m = view.config.voxel_m;
    let crowns: Vec<Vec<Shade>> = stands
        .iter()
        .map(|s| shade_layers(config, s, voxel_m))
        .collect();
    // A conservative bound per **stand**, so the pair loop below stays the O(n²) it was
    // before layers instead of O(n² · layers²): no layer of a stand can shade above the
    // stand's own highest layer top, or outside its widest layer's footprint. Skipping
    // on the bound changes no result — it only avoids opening a stand whose every layer
    // would have been rejected.
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
            for (j, other) in crowns.iter().enumerate() {
                if j == i {
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
        out.push(light_response(config.species(stand.species), l));
    }
    out
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
    porosity: f64,
    area_m2: f64,
}

fn shade_layers(config: &FloraConfig, stand: &Stand, voxel_m: f64) -> Vec<Shade> {
    crate::layers_of(config, stand, voxel_m)
        .into_iter()
        .filter(|l| l.kind.bears_foliage())
        .map(|l| Shade {
            x: stand.site.x as f64,
            z: stand.site.z as f64,
            top: l.band_v[1],
            radius: l.radius_v,
            stock: l.stock,
            share: l.share,
            porosity: l.porosity,
            area_m2: l.area_m2,
        })
        .collect()
}

/// `L_eff = L (1 + light_half) / (L + light_half)`: a species with a small `light_half`
/// earns nearly its full income in shade, one with a large value needs open sky.
fn light_response(sc: &SpeciesConfig, l: f64) -> f64 {
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
}

/// Collect every stand's demand per voxel, then withdraw once per voxel and split what
/// the core accepted proportional to demand.
fn drink(flora: &mut Flora, world: &mut World) -> Vec<Drink> {
    let mut out = vec![Drink::default(); flora.stands.len()];
    // (voxel index, stand, wanted) — sorted, so the withdrawal order is the world's own
    // index order and nothing depends on how the stands were reached.
    let mut wants: Vec<(usize, usize, f64)> = Vec::new();
    {
        let view = world.view();
        for (i, stand) in flora.stands.iter().enumerate() {
            let sc = flora.config.species(stand.species);
            let box_ = root_box(&view, stand.site, sc);
            out[i].moisture = moisture_of(&view, &box_, sc);
            // A species that needs standing water (package N) and has lost it reads
            // `μ = 0`: it earns nothing and pays its upkeep out of reserve.
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
            let stock: f64 = box_.iter().map(|&v| pore_m3(&view, v)).sum();
            if stock <= 0.0 {
                continue;
            }
            for &v in box_.iter() {
                let share = demand * pore_m3(&view, v) / stock;
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
        // One bounded operation for the whole voxel, whatever asked for it.
        let accepted = -world.apply(WorldCommand::WithdrawPore {
            x: x as i64,
            y,
            z,
            volume_m3: total,
        });
        flora.ledger.transpired_m3 += accepted;
        for &(_, stand, want) in &wants[at..end] {
            out[stand].taken_m3 += split_proportional(accepted, want, total);
        }
        at = end;
    }
    out
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
    #[cfg(feature = "profile")]
    cubarium_voxel::profile::add(
        cubarium_voxel::profile::Count::BoxVoxels,
        u64::from(sc.rooting_depth.min(site.y + 1))
            * u64::from(2 * sc.rooting_radius + 1)
            * u64::from(2 * sc.rooting_radius + 1),
    );
    let c = view.config;
    let span = sc.rooting_depth.min(site.y + 1);
    if span == 0 {
        return Vec::new();
    }
    let y_lo = site.y + 1 - span;
    let r = sc.rooting_radius as i64;
    let mut out = Vec::new();
    for dz in -r..=r {
        let z = site.z as i64 + dz;
        if z < 0 || z >= c.depth as i64 {
            continue;
        }
        for y in y_lo..=site.y {
            for dx in -r..=r {
                let i = c.index(site.x as i64 + dx, y, z as u32);
                if view.material[i] == Material::Soil {
                    out.push(i);
                }
            }
        }
    }
    out
}

fn pore_m3(view: &VoxelView<'_>, i: usize) -> f64 {
    view.pore[i] * view.material[i].pore_capacity() * view.config.voxel_volume()
}

/// The capacity-weighted mean pore fraction of a root box, `None` for a box with no
/// pore space in it at all. Weighted, not a plain mean, so a box that mixes materials
/// reads as the water it could actually hold.
fn mean_pore(view: &VoxelView<'_>, box_: &[usize]) -> Option<f64> {
    let mut water = 0.0;
    let mut capacity = 0.0;
    for &i in box_ {
        let cap = view.material[i].pore_capacity();
        water += view.pore[i] * cap;
        capacity += cap;
    }
    if capacity <= 0.0 {
        None
    } else {
        Some(water / capacity)
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
/// say how fast it gets there rather than which boundary it ramps to. Package I measured
/// what the increment rule did instead (`design/7_Research/voxel-round3-experiment-2026-09-16.md`,
/// findings 1 and 2): `f* = relax / (rate + relax)` was a threshold, and two saturated
/// voxels of an eighteen-voxel box pinned a bloomcrown at stress 1 forever.
fn aeration_target(saturated: f64, sc: &SpeciesConfig) -> f64 {
    let tol = sc.establish_saturated_max;
    if tol >= 1.0 {
        return 0.0;
    }
    ((saturated - tol) / (1.0 - tol)).clamp(0.0, 1.0)
}

/// `μ`: a linear ramp of the root box's mean pore fraction between `wilt_pore` and
/// `sat_pore`. An empty box is a wilting one.
fn moisture_of(view: &VoxelView<'_>, box_: &[usize], sc: &SpeciesConfig) -> f64 {
    match mean_pore(view, box_) {
        None => 0.0,
        Some(mean) => ramp(mean, sc.wilt_pore, sc.sat_pore),
    }
}

/// Zero at or below `lo`, one at or above `hi`, linear between. A `hi` at or below `lo`
/// is a step at `hi`.
fn ramp(value: f64, lo: f64, hi: f64) -> f64 {
    if hi <= lo {
        return if value >= hi { 1.0 } else { 0.0 };
    }
    ((value - lo) / (hi - lo)).clamp(0.0, 1.0)
}

// --------------------------------------------------------------- 5b. substrate

/// The **mycelium box** of a saprotroph on `site`: `|dx| <= rooting_radius`, `|dz| <=
/// rooting_radius`, `|dy| <= `[`SpeciesConfig::substrate_reach_up_down`], `x` wrapped and
/// `z` and `y` clipped at the walls, read as support **sites** rather than as soil voxels,
/// in a fixed geometric order, deduplicated.
///
/// Sites, not voxels, because what a fungus eats is a *stock on the ground* and the ground
/// stocks live one per support face. A site in the box that has never held anything has no
/// [`Ground`] and therefore no dead wood, which is the same thing as holding none.
///
/// **The vertical reach is its own field and symmetric (Astra R9.3).** It used to be
/// [`root_box`]'s geometry, which reaches `rooting_depth` **down** and never up, so at
/// glowcap's `rooting_depth` 1 the box was the stand's own row alone and a spore one voxel
/// above or below a full log found nothing to eat — which is what refused all three
/// landings of the round-5b `community` run. Mycelium in a log is not a root in soil, so
/// substrate access is now decided by `substrate_reach_up_down` and the soil-water box is
/// untouched.
///
/// Still a box and not a path: a log across a one-voxel wall is in reach of a mycelium that
/// could not actually grow through it, exactly as [`crate::Reach`] is a box with no line of
/// sight in it.
fn mycelium_sites(view: &VoxelView<'_>, site: Site, sc: &SpeciesConfig) -> Vec<Site> {
    let c = view.config;
    let up_down = i64::from(sc.substrate_reach_up_down);
    let y_lo = (i64::from(site.y) - up_down).max(0) as u32;
    let y_hi = (i64::from(site.y) + up_down).min(i64::from(c.height.max(1) - 1)) as u32;
    let r = sc.rooting_radius as i64;
    let width = c.width.max(1) as i64;
    let mut out: Vec<Site> = Vec::new();
    for dz in -r..=r {
        let z = site.z as i64 + dz;
        if z < 0 || z >= c.depth as i64 {
            continue;
        }
        for y in y_lo..=y_hi {
            for dx in -r..=r {
                let x = (site.x as i64 + dx).rem_euclid(width) as u32;
                let s = Site { x, y, z: z as u32 };
                if !out.contains(&s) {
                    out.push(s);
                }
            }
        }
    }
    out
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
    for gi in mycelium_sites(view, site, sc)
        .into_iter()
        .filter_map(|s| ground.binary_search_by_key(&s, |g| g.site).ok())
    {
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
/// `1 − aeration_stress`: a saprotroph's only water term is `μ`, which is the round-5b
/// brief's rule, and the consequence is stated in the glowcap preset's own doc (a spore
/// will not start on a waterlogged log, while the mycelium already in one pays nothing).
///
/// Nothing is booked at the layer's boundary here. The material moves from a ground stock
/// to a stand, both inside this layer, so the only ledger term it touches is the
/// per-species diagnostic [`FloraLedger::substrate_uptake`]; the organic matter the fungus
/// does **not** keep is respired in [`grow`], where every other respiration is.
fn feed(flora: &mut Flora, world: &World, drink: &[Drink]) -> Vec<Substrate> {
    let mut out = vec![Substrate::default(); flora.stands.len()];
    let Flora {
        config,
        stands,
        ground,
        ledger,
        ..
    } = flora;
    let view = world.view();
    // Sorted, so the withdrawal order is the ground's own site order, dead wood before
    // litter at a site, and nothing depends on how the stands were reached.
    let mut wants: Vec<Want> = Vec::new();
    for (si, stand) in stands.iter().enumerate() {
        let sc = config.species(stand.species);
        if sc.trophic != Trophic::Saprotroph {
            continue;
        }
        let want = (sc.substrate_uptake_per_s * stand.wood * drink[si].moisture * DT).max(0.0);
        if !(want > 0.0) {
            continue;
        }
        let mut pools: Vec<(usize, Pool, f64)> = Vec::new();
        for s in mycelium_sites(&view, stand.site, sc) {
            if let Ok(gi) = ground.binary_search_by_key(&s, |g| g.site) {
                for pool in [Pool::DeadWood, Pool::Litter] {
                    let held = pool.held(&ground[gi]);
                    if held > 0.0 {
                        pools.push((gi, pool, held));
                    }
                }
            }
        }
        let total: f64 = pools.iter().map(|&(_, _, w)| w).sum();
        if !(total > 0.0) {
            continue;
        }
        for (gi, pool, held) in pools {
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
    wants.sort_unstable_by_key(|w| (w.gi, w.pool, w.si));

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

fn grow(
    flora: &mut Flora,
    world: &World,
    light: &[f64],
    drink: &[Drink],
    substrate: &[Substrate],
) {
    let Flora {
        config,
        stands,
        ground,
        ledger,
        ..
    } = flora;
    let mut dead: Vec<usize> = Vec::new();

    for si in 0..stands.len() {
        let site = stands[si].site;
        let species = stands[si].species;
        let sc = config.species(species);
        let e_v = sc.energy_density;
        let gi = ground_slot(ground, site);

        let (w0, p0, q0) = (stands[si].wood, stands[si].foliage, stands[si].reserve);
        let l_eff = light[si];
        let mu = drink[si].moisture;
        stands[si].light = l_eff;
        stands[si].moisture = mu;
        stands[si].water_m3 = drink[si].taken_m3;

        // ---- aeration stress, from the same pre-withdrawal read `μ` came from. It
        // closes on the level the root box's saturated fraction asks for — up at
        // `stress_rate_per_s`, down at `relax_rate_per_s`, both as a fraction of the
        // gap per second — and it is applied to *this* tick's income: the water was read
        // before anything was withdrawn, so the stress and the moisture describe one
        // moment.
        let saturated = drink[si].saturated;
        let target = aeration_target(saturated, sc);
        let was = stands[si].aeration_stress;
        let stress = if target > was {
            (was + sc.stress_rate_per_s * DT * (target - was)).clamp(0.0, 1.0)
        } else if target < was {
            (was - sc.relax_rate_per_s * DT * (was - target)).clamp(0.0, 1.0)
        } else {
            was
        };
        stands[si].aeration_stress = stress;

        // ---- 4.1 potential income, 4.2 demands
        //
        // The mineral cap replaces v1's `.min(n0)`: income is organic matter, and what
        // the pool limits is the *tissue* it can pay `n_tissue` per unit for, so the
        // pool's own units bound `A` through `mineral / n_tissue`. The
        // Michaelis-Menten and the `f_max` rate cap are unchanged.
        //
        // All three read the **site's** pool, and they gate the whole of `A` — including
        // the part that pays maintenance. So a stand whose own tissue is rich in mineral
        // still fixes nothing on a bare pool, which is stronger than "mineral caps new
        // tissue" and is a stated limitation of this round rather than a physiological
        // claim: `Stand::mineral` is an inventory, not a usable reserve (Astra R4.3, and
        // the doc on that field). At the placeholders the `f_max` rate cap is the binding
        // one by five orders of magnitude — `0.0005 · N` against the stock cap's
        // `50 · N` — so moving `n_tissue` would not change which limit bites.
        let n0 = ground[gi].mineral;
        let monod = if n0 + sc.nutrient_half > 0.0 {
            n0 / (n0 + sc.nutrient_half)
        } else {
            0.0
        };
        let mineral_cap = if sc.n_tissue > 0.0 {
            n0 / sc.n_tissue
        } else {
            f64::INFINITY
        };
        let p_cap = sc.alpha * w0;
        let q_max = sc.reserve_cap * w0;
        let m = sc.maintenance * w0 * DT;
        let d_p = (p_cap - p0).max(0.0).min(sc.foliage_rate * w0 * DT);
        let d_w = (sc.wood_max - w0).max(0.0).min(sc.wood_rate * w0 * DT);
        let d_q = (q_max - q0).max(0.0);
        let build = 1.0 + sc.build;
        // The income, by trophic mode. Everything below this is one set of rules.
        let (a, arrived_mineral) = match sc.trophic {
            Trophic::Photo => {
                let a_pot = (sc.assimilation * l_eff * mu * (1.0 - stress) * p0 * monod * DT)
                    .min(sc.nutrient_draw_max * n0 * DT)
                    .min(mineral_cap)
                    .max(0.0);
                // Capped by what the stand can actually spend: light not captured is
                // simply never fixed, so there is nothing to respire.
                let a = a_pot.min(m + build * (d_p + d_w) + d_q);
                ledger.fixed_in += a;
                ledger.light_in += e_v * a;
                (a, 0.0)
            }
            // A saprotroph's income is the dead wood and litter step 5b already took out
            // of the pools for it. `substrate_yield` of it is income and the rest is
            // respired **at once**: it has left the pool and it is not tissue, so it
            // cannot be left unaccounted the way an uncaptured photon is, and there is no
            // demand cap here for the same reason — anything the stand cannot spend falls
            // through to the leftover `rem` below and is respired there.
            //
            // The tissue was computed pool by pool in [`feed`] ([`yield_of`]), because the
            // two pools carry their own energy per unit; here it is only spent. It is
            // bounded again by `taken.organic` against float dust, and the energy bound it
            // already obeys per pool keeps `taken.energy - e_v · gross` non-negative.
            Trophic::Saprotroph => {
                let Substrate { taken, gross } = substrate[si];
                let gross = gross.max(0.0).min(taken.organic);
                let waste = (taken.organic - gross).max(0.0);
                ledger.respired_out += waste;
                ledger.heat_out += (taken.energy - e_v * gross).max(0.0);
                (gross, taken.mineral)
            }
        };

        // ---- 4.3 maintenance from income first, then reserve
        let paid_a = a.min(m);
        let mut rem = a - paid_a;
        let short = m - paid_a;
        let paid_q = stands[si].reserve.min(short);
        stands[si].reserve -= paid_q;
        let unpaid = short - paid_q;
        // Both are respiration: the organic matter leaves the system as heat, and no
        // mineral moves. `paid_a` was never tissue, and the mineral of the reserve
        // `paid_q` burned stays in the stand — what is still standing keeps its mineral.
        ledger.respired_out += paid_a + paid_q;
        ledger.heat_out += e_v * (paid_a + paid_q);

        // ---- 4.4a the mineral budget, **before** a unit of tissue is allocated
        //
        // Astra R9.1. The mineral used to be netted against the tissue *after* it was
        // built, which capped the **debit** and not the growth: a moist half-grown glowcap
        // on an energy-bearing, mineral-free log with a bare site pool still grew
        // `5e-6` of wood whose `1e-7` of mineral did not exist, so the tissue it built
        // held less than `n_tissue` and the promise that `n_tissue` is the density of what
        // is built was only true for the five plants. The budget now comes first and
        // bounds what may be built at all.
        //
        // What is spendable this tick is what **arrived** with the income plus what the
        // site's pool can give up: `arrived_mineral` — the mineral that came out of the
        // log with the wood, zero for a `Photo` stand — plus the pool `n0` itself. The
        // pool is the bound a plant's own draw obeys: §4.2's stock cap `N / n_tissue` is
        // exactly `N` of mineral read in tissue units, and `nutrient_draw_max` bounds
        // **assimilation**, which a saprotroph does not do. A *rate* cap on a fungal pool
        // draw is a rule decision and a `design/backlog.md` row, not one this round
        // invents.
        //
        // For a `Photo` stand this cap is provably non-binding and is written once for
        // both modes rather than branched: §4.2 caps `A` by `mineral_cap = N / n_tissue`
        // and every unit built costs at least one unit of `rem <= A`, so
        // `n_tissue · built <= N` already. For a saprotroph it is the whole of R9.1.
        let mineral_budget = arrived_mineral + n0.max(0.0);
        let mut tissue_left = if sc.n_tissue > 0.0 {
            (mineral_budget / sc.n_tissue).max(0.0)
        } else {
            f64::INFINITY
        };

        // ---- 4.4 growth: reserve share, foliage, wood, then the rest to reserve
        //
        // Every increment is floored at zero as well as capped by its demand **and by the
        // mineral budget left**. Unfunded income is not held anywhere: it falls through to
        // the leftover `rem` below and is respired there, with its energy as heat, which
        // is where a capped income has always gone. The caps above already keep `rem`
        // non-negative in exact arithmetic; in f64 a `rem - build * (rem / build)` can
        // land a few ulps below it, and a negative "growth" would take a stock down
        // instead of up.
        let dq_s = (sc.reserve_share * rem).min(d_q).min(tissue_left).max(0.0);
        rem -= dq_s;
        tissue_left -= dq_s;
        stands[si].reserve += dq_s;

        let dp_a = (rem / build).min(d_p).min(tissue_left).max(0.0);
        rem -= build * dp_a;
        tissue_left -= dp_a;
        let dp_q = if p0 < sc.reflush_below * p_cap {
            (stands[si].reserve / build)
                .min(d_p - dp_a)
                .min((sc.reflush_below * p_cap - p0 - dp_a).max(0.0))
                .max(0.0)
        } else {
            0.0
        };
        stands[si].reserve -= build * dp_q;
        stands[si].foliage += dp_a + dp_q;
        // Where the new foliage goes: **bottom-up to each layer's capacity**, in
        // profile order, which is what makes a browsed plant refill its floor tissue
        // first and look browsed from below (decisions §4). The capacities are read at
        // the wood this tick started with, because `dw` below has not been applied yet.
        {
            let sc = config.species(species);
            let caps = sc.layer_capacities(stands[si].wood);
            let n = caps.len();
            crate::layers::fill_bottom_up(stands[si].layer_stocks_mut(n), &caps, dp_a + dp_q);
            stands[si].settle_layers(n);
        }

        let dw = (rem / build).min(d_w).min(tissue_left).max(0.0);
        rem -= build * dw;
        tissue_left -= dw;
        stands[si].wood += dw;

        let dq_r = rem.min((d_q - dq_s).max(0.0)).min(tissue_left).max(0.0);
        stands[si].reserve += dq_r;
        rem -= dq_r;
        // The leftover itself: never respire a negative residue. With the §4.2 cap on
        // `A` this is float residue only.
        let rem = rem.max(0.0);
        // Construction respiration on every unit built, whichever stock paid for it, and
        // the leftover of a capped income: organic matter out of the system, energy to
        // heat, no mineral.
        let respired = sc.build * (dp_a + dp_q + dw) + rem;
        ledger.respired_out += respired;
        ledger.heat_out += e_v * respired;

        // New tissue built out of this tick's income draws `n_tissue` per unit from the
        // site's pool. The reflush `dp_q` is not new tissue — it is reserve turned into
        // foliage — so its mineral is already in the stand and is not drawn again. This is
        // now a **settlement** of §4.4a's budget and not a cap of its own: `need` is at
        // most `mineral_budget` by construction, so the draw below is always fully funded
        // and the `min`s are float insurance. Only this stand draws on this site.
        let built = dq_s + dp_a + dw + dq_r;
        let need = (sc.n_tissue * built).max(0.0);
        // A saprotroph's mineral arrives **with the wood**, at the log's own density, and
        // is netted against what the tissue it built actually needs: the excess is released
        // to the site's pool and a shortfall is drawn from the pool the budget counted.
        // Mineral only ever moves between stocks, so both directions conserve it exactly.
        // A `Photo` stand has no arrival and this is the draw it always was.
        stands[si].mineral += arrived_mineral;
        if arrived_mineral > need {
            let release = (arrived_mineral - need).min(stands[si].mineral.max(0.0));
            stands[si].mineral -= release;
            ground[gi].mineral += release;
        } else {
            let draw = (need - arrived_mineral).min(ground[gi].mineral.max(0.0));
            ground[gi].mineral -= draw;
            stands[si].mineral += draw;
        }

        // ---- 4.5 senescence
        let shed = (sc.senescence * stands[si].foliage * DT).min(stands[si].foliage);
        let organic_before = stands[si].material();
        stands[si].foliage -= shed;
        // Senescence loses from the **top** foliage layer first: a plant drops its
        // canopy before its rosette, and the rosette persists
        // (`design/organism-anatomy-2026-09-21.md` §3, bloomcrown).
        {
            let n = sc.foliage_layer_count(stands[si].wood);
            crate::layers::shed_from_top(stands[si].layer_stocks_mut(n), shed);
            stands[si].settle_layers(n);
        }
        let shed_mineral = pull_mineral(&mut stands[si], organic_before, shed);
        add_litter(
            config,
            &mut ground[gi],
            shed,
            shed_mineral,
            e_v * shed,
            ledger,
        );

        // ---- 4.6 dieback
        let die_back = stands[si].wood.min(sc.dieback * unpaid);
        let organic_before = stands[si].material();
        stands[si].wood -= die_back;
        let die_back_mineral = pull_mineral(&mut stands[si], organic_before, die_back);
        ground[gi].dead_wood += die_back;
        ground[gi].dead_wood_mineral += die_back_mineral;
        ground[gi].dead_wood_energy += e_v * die_back;

        // ---- 4.6b the stage, after every change to the wood
        //
        // Growth and dieback both move `W`, and the profile is staged by `W / W_max`.
        // A stand that has crossed a threshold re-bins the tissue it **already holds**
        // into the new stage's layers, bottom-up: a seedling rosette that has grown a
        // stem does not conjure a crown, it lifts what it had. Nothing is created and
        // nothing is destroyed (decisions §4).
        stands[si].resync_layers(config.species(species));

        // ---- 4.7 death
        if stands[si].wood < sc.alive_min {
            dead.push(si);
        }
    }

    let view = world.view();
    for &si in dead.iter().rev() {
        let stand = stands[si];
        die(config, &view, &stand, ground, ledger);
        stands.remove(si);
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
        (stand.mineral * (moved / before)).min(stand.mineral)
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
/// [`crate::SpeciesConfig::falls`] (the vaulttree, package N) the wood, its mineral and
/// its energy are laid in equal shares along its [`fall_line`] instead of on its own site;
/// the litter stays where the stand stood. Every other species' line is its own site.
fn die(
    config: &FloraConfig,
    view: &VoxelView<'_>,
    stand: &Stand,
    ground: &mut Vec<Ground>,
    ledger: &mut FloraLedger,
) {
    let sc = config.species(stand.species);
    let e_v = sc.energy_density;
    let organic = stand.material();
    let wood_mineral = if organic > 0.0 {
        (stand.mineral * (stand.wood / organic)).min(stand.mineral)
    } else {
        0.0
    };
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
    let shed = stand.foliage + stand.reserve + stand.parcel;
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
}

/// **The fall** (package N, model addition 1): where a falling stand's wood lands, as
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
fn provisioned_slot(
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
fn decompose(flora: &mut Flora, pre: &[Pre]) {
    let Flora {
        config,
        ground,
        ledger,
        ..
    } = flora;
    for g in ground.iter_mut() {
        // A site the tick created has nothing eligible yet.
        let Ok(i) = pre.binary_search_by_key(&g.site, |e| e.site) else {
            continue;
        };
        let (litter0, wood0, carrion0) = (pre[i].litter, pre[i].dead_wood, pre[i].carrion);
        decompose_pool(
            &mut g.litter,
            &mut g.litter_mineral,
            &mut g.litter_energy,
            &mut g.mineral,
            config.decomposition,
            litter0,
            ledger,
        );
        decompose_pool(
            &mut g.dead_wood,
            &mut g.dead_wood_mineral,
            &mut g.dead_wood_energy,
            &mut g.mineral,
            config.wood_decomposition,
            wood0,
            ledger,
        );
        // Carrion, at its own rate, on exactly the two flows the other two pools use: a
        // corpse is not a special case of the ledger, only of the clock.
        decompose_pool(
            &mut g.carrion,
            &mut g.carrion_mineral,
            &mut g.carrion_energy,
            &mut g.mineral,
            config.carrion_decomposition,
            carrion0,
            ledger,
        );
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
    stock0: f64,
    ledger: &mut FloraLedger,
) {
    let dec = (rate * DT * stock0).min(*organic).max(0.0);
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

// ----------------------------------------------------------------- 8. seed bank

/// The seed bank: **germinate first**, out of the bank as it stands at the start of the
/// tick, and then charge attrition and expiry on whatever is still banked.
///
/// The order matters because a package is exactly one minimum viable stand's material
/// (`alive_min / w_frac`). Charging attrition first left a single package 0.1 % short of
/// the germination threshold for ever, so every recruit cost **two** packages and one
/// package's worth of paid material could never become a stand at all — which is not the
/// rule the package size states. Germination therefore reads the bank before anything
/// decays out of it, and attrition applies to what stays.
///
/// Two consequences, both deliberate. A package that lands on tick `t` on a passing site
/// is born at tick `t + 1` with the whole package. And a bin on the tick its own
/// `seed_max_age_s` runs out gets one last chance to germinate before it falls to litter:
/// it is paid material on a site that passes the predicate, and throwing it away in the
/// same tick that could have used it would be the same arbitrariness one step further out.
///
/// # The expiry boundary, exactly (Astra R5.5)
///
/// [`age_cohorts`] removes a bin on the first tick whose age is **greater than**
/// `seed_max_age_s`, and it runs after the lottery. So for a lifetime of `L` ticks and a
/// bin that opened at tick `b`:
///
/// - ticks `b..=b + L` — the bin is in the bank and can be drawn on.
/// - tick `b + L + 1` — the lottery still sees it: **this is its last chance**, and a site
///   whose gate opens on exactly this tick recruits out of it. Whatever is left of it
///   afterwards goes to litter whole, with its mineral and its energy.
/// - tick `b + L + 2` and after — there is nothing there. A gate that opens one tick too
///   late finds an empty bank, and the material is in the litter.
///
/// A bin therefore cannot linger: it recruits once on its removal tick or it is gone. The
/// rule is the `>` in [`age_cohorts`] plus this phase order, and changing either is a rule
/// decision and not a tidy-up. Pinned by
/// `tests/round3.rs::an_expiring_bin_gets_one_last_germination_and_then_goes_to_litter`.
fn seed_bank(flora: &mut Flora, world: &World) {
    let Flora {
        config,
        tick,
        stands,
        ground,
        ledger,
        sky,
        ..
    } = flora;
    let tick = *tick;
    let view = world.view();
    // In site order. Which species takes a bare site is a **local lottery** among the
    // banks that can build a stand here, weighted by the whole packages each holds, drawn
    // from a stream keyed by the world, the site and the tick. The winner spends exactly
    // one package out of its oldest bins; its own remainder and every loser's bank stay
    // where they are and go on ageing.
    let world_seed = view.config.seed;
    // The lottery is drawn in a **read-only** pass, and only then are the winners' banks
    // spent. Round 5b's reason: a saprotroph's substrate gate reads the dead wood of
    // *other* sites in its mycelium box, which cannot be looked up while this site is
    // borrowed for mutation. The order, the keys and the draw are unchanged — the sweep is
    // still `ground`'s own site order — so no `Photo` species' behaviour moves.
    let mut winners: Vec<(usize, Species)> = Vec::new();
    for (gi, g) in ground.iter().enumerate() {
        if g.seeds.is_empty() || stands.binary_search_by_key(&g.site, |s| s.site).is_ok() {
            continue;
        }
        // Candidates in `Species::ALL` order — a fixed order, and the lottery sorts them
        // again so that not even a caller's order can reach the draw.
        let mut candidates: Vec<(Species, u64)> = Vec::new();
        for species in Species::ALL {
            let sc = config.species(species);
            let package = package_of(sc);
            if package <= 0.0 {
                continue;
            }
            let packages = (g.seed_organic(species) / package).floor();
            if packages < 1.0 {
                continue;
            }
            if !establishes(&view, sky, ground, g.site, sc) {
                continue;
            }
            candidates.push((species, packages.min(u32::MAX as f64) as u64));
        }
        let site_index = view.config.index(g.site.x as i64, g.site.y, g.site.z) as u64;
        if let Some(species) = lottery(world_seed, site_index, tick, &candidates) {
            winners.push((gi, species));
        }
    }
    for (gi, species) in winners {
        let g = &mut ground[gi];
        let sc = config.species(species);
        // The slot first and the bank second: a bank must never be spent on a birth that
        // does not happen. The pass above skipped occupied sites and nothing inserts a
        // stand on *this* site in between, so this is unreachable rather than a real
        // branch — and it is in this order so that it stays harmless if that ever changes.
        let at = match stands.binary_search_by_key(&g.site, |s| s.site) {
            Ok(_) => continue,
            Err(at) => at,
        };
        // Exactly one package, oldest bins first, with each bin's own mineral in
        // proportion to what it gave up.
        let (organic, mineral) = spend_bank(g, species, package_of(sc));
        let (wood, foliage, reserve) = newborn_stocks(sc, organic);
        let id = ledger.births;
        ledger.births += 1;
        let mut born = Stand {
            id,
            site: g.site,
            species,
            stage: Stage::Alive,
            wood,
            foliage,
            reserve,
            light: 0.0,
            moisture: 0.0,
            water_m3: 0.0,
            mineral,
            aeration_stress: 0.0,
            parcel: 0.0,
            layer_stock: [0.0; crate::MAX_FOLIAGE_LAYERS],
            profile_stage: 0,
        };
        // A newborn's foliage goes into its seedling profile bottom-up, like any other
        // tissue: the stocks sum to `foliage` from the first tick of its life.
        born.bin_foliage(sc, config.voxel_m);
        stands.insert(at, born);
        ledger.establishments += 1;
    }
    // Then the decay, on what is left: a bin the germination above emptied pays nothing,
    // because there is nothing left of it to pay with.
    for g in ground.iter_mut() {
        age_cohorts(config, g, ledger, tick);
    }
}

/// The three stocks a funded newborn is built out of `organic` units of banked material:
/// wood at **exactly** `alive_min`, foliage at the preset's `p_frac` of the material, and
/// the remainder — including every unit of floating-point difference — in the reserve.
///
/// Astra's R5.1. `spend_bank` accumulates the material it takes bin by bin, so across
/// several bins the sum is the intended package to within an ulp or two rather than to the
/// bit: Astra's case is bins of `0.001, 0.009, 0.04` against the default package
/// `0.02 / 0.4 = 0.049999999999999996`, which spends `0.04999999999999999` — short by
/// 6.9e-18. `w_frac · that` is `0.019999999999999997`, which is **below** `alive_min` by
/// 3.5e-18, so the next growth pass killed a newborn that had just been paid for in full,
/// with no loss of wood anywhere. A funded package buys a stand that is alive: the wood is
/// the threshold itself, and the rounding difference goes where it can do no harm.
///
/// The reserve is the compartment that absorbs it because it is the buffer — it feeds no
/// income and no death test. The paid organic total and the transferred mineral are
/// preserved: `wood + foliage + reserve` re-sums to `organic` (exactly, in Astra's case),
/// and `mineral` is untouched by this. Every stock is non-negative by construction, even
/// for a preset whose `q_frac` is zero, because `foliage` is capped at what is left after
/// the wood and the reserve is that remainder.
fn newborn_stocks(sc: &SpeciesConfig, organic: f64) -> (f64, f64, f64) {
    let wood = sc.alive_min.min(organic.max(0.0));
    let left = (organic - wood).max(0.0);
    let foliage = (sc.propagule_split[1] * organic).clamp(0.0, left);
    (wood, foliage, left - foliage)
}

/// Which species takes a gap, among the banks that can build a stand on it: a draw
/// weighted by the **whole packages** each holds, from a stream keyed by the world's seed,
/// the site's voxel index and the tick. `None` when nothing qualifies.
///
/// Astra's R4.5: the old rule gave the gap to the first qualifying species in
/// `Species::ALL`, so bloomcrown pre-empted umbrellafrond in every contested gap in the
/// world whatever the two banks held, and adding three species after it would have built
/// that precedence into the whole ecology. A fixed enum order is not a `HashMap` iteration,
/// but it is not an ecological rule either.
///
/// The candidates are sorted by species before the walk, so the caller's order — which is
/// `Species::ALL` today and could be a storage order tomorrow — cannot select the winner.
/// Weights are integer package counts, so the draw is one bounded integer and there is no
/// float comparison in it.
fn lottery(
    world_seed: u64,
    site_index: u64,
    tick: u64,
    candidates: &[(Species, u64)],
) -> Option<Species> {
    match candidates {
        [] => return None,
        [(one, _)] => return Some(*one),
        _ => {}
    }
    let mut sorted: Vec<(Species, u64)> = candidates.to_vec();
    sorted.sort_unstable_by_key(|&(species, _)| species);
    let total: u64 = sorted.iter().map(|&(_, w)| w).sum();
    if total == 0 {
        return None;
    }
    let mut draw =
        Rng::keyed(DOMAIN_GERMINATION, world_seed, site_index, tick).below(total as usize) as u64;
    for (species, weight) in sorted {
        if draw < weight {
            return Some(species);
        }
        draw -= weight;
    }
    None
}

/// Spend `want` of one species' banked organic matter, **oldest bin first**, taking each
/// bin's mineral in proportion to the organic matter taken from it. Returns what was
/// actually spent, which is `want` unless the bank held less.
///
/// A bin that is emptied is removed whole, mineral included, so no float dust is left
/// behind claiming to be a cohort; a bin that is partly spent keeps its `bin_start_tick`
/// and goes on ageing toward its own expiry. Nothing is deleted: what is not spent stays
/// banked (Astra's R4.5 — germination used to spend the whole bank, however large, and
/// build an oversized "small" stand out of it).
fn spend_bank(g: &mut Ground, species: Species, want: f64) -> (f64, f64) {
    let mut left = want;
    let (mut organic, mut mineral) = (0.0, 0.0);
    let mut i = 0;
    while i < g.seeds.len() && left > 0.0 {
        if g.seeds[i].species != species {
            i += 1;
            continue;
        }
        let c = g.seeds[i];
        if c.organic <= left {
            organic += c.organic;
            mineral += c.mineral;
            left -= c.organic;
            g.seeds.remove(i);
            continue;
        }
        let m = (c.mineral * (left / c.organic)).min(c.mineral);
        g.seeds[i].organic -= left;
        g.seeds[i].mineral -= m;
        organic += left;
        mineral += m;
        left = 0.0;
    }
    (organic, mineral)
}

/// One tick of decay for one site's bank. A bin whose **start** is past `seed_max_age_s`
/// falls to litter whole; every other bin pays `seed_attrition_per_s · dt` of itself into
/// litter, with the matching fraction of its mineral and its energy. Paid decay, never
/// deletion.
///
/// Nothing here writes an age: a bin's age is `tick - bin_start_tick`, so it rises by one
/// every tick on its own and a landing cannot lower it.
fn age_cohorts(config: &FloraConfig, g: &mut Ground, ledger: &mut FloraLedger, tick: u64) {
    if g.seeds.is_empty() {
        return;
    }
    let cap = config.litter_energy_cap;
    let taken = std::mem::take(&mut g.seeds);
    let mut kept: Vec<SeedCohort> = Vec::with_capacity(taken.len());
    for mut c in taken {
        let sc = config.species(c.species);
        let e_v = sc.energy_density;
        if c.age_s(tick) > sc.seed_max_age_s {
            add_litter_cap(cap, g, c.organic, c.mineral, e_v * c.organic, ledger);
            continue;
        }
        let loss = (sc.seed_attrition_per_s * DT * c.organic)
            .min(c.organic)
            .max(0.0);
        if loss > 0.0 {
            let mineral = if c.organic > 0.0 {
                (c.mineral * (loss / c.organic)).min(c.mineral)
            } else {
                0.0
            };
            c.organic -= loss;
            c.mineral -= mineral;
            add_litter_cap(cap, g, loss, mineral, e_v * loss, ledger);
        }
        if c.organic > 0.0 || c.mineral > 0.0 {
            kept.push(c);
        }
    }
    g.seeds = kept;
}

/// The width of one arrival bin in ticks: `seed_max_age_s / seed_cohorts_max`, **rounded
/// up**, at least one tick. Derived, not a knob of its own — the placeholders' 600 s over 4
/// bins is 150 s, which is 3,000 ticks exactly.
///
/// Rounded up rather than truncated so that the bound on the bank is exact: a lifetime of
/// `life` ticks in bins of `ceil(life / n)` can hold at most `n + 1` live bins, where
/// truncating could fit one more (a 2 s lifetime over 7 bins truncates to 5 ticks, and
/// 40 ticks of lifetime is nine such bins). Rounding up can only make a bin coarser than
/// asked for, never let a bank hold more of them.
fn bin_ticks(sc: &SpeciesConfig) -> u64 {
    let ticks = sc.seed_max_age_s / sc.seed_cohorts_max.max(1) as f64 / DT;
    if ticks.is_finite() && ticks >= 1.0 {
        ticks.ceil() as u64
    } else {
        1
    }
}

/// The first tick of the bin that `tick` falls in.
fn bin_start(tick: u64, sc: &SpeciesConfig) -> u64 {
    let w = bin_ticks(sc);
    tick - tick % w
}

/// Land a fresh package on a site's bank, in the bin whose window covers `tick`: a bin
/// that is already there sums the organic matter and the mineral and keeps its start, and
/// one that is not is inserted. `seeds` stays sorted by species then `bin_start_tick`,
/// oldest first, so one species' bins are a contiguous run and germination can spend the
/// oldest of them first.
///
/// No cap and no merge: a bin's age never decreases, so `seed_max_age_s` bounds how many
/// bins of one species can be alive at `seed_cohorts_max + 1` by construction.
fn add_cohort(
    g: &mut Ground,
    species: Species,
    organic: f64,
    mineral: f64,
    tick: u64,
    sc: &SpeciesConfig,
) {
    if organic <= 0.0 && mineral <= 0.0 {
        return;
    }
    let start = bin_start(tick, sc);
    match g
        .seeds
        .binary_search_by(|c| (c.species, c.bin_start_tick).cmp(&(species, start)))
    {
        Ok(i) => {
            g.seeds[i].organic += organic;
            g.seeds[i].mineral += mineral;
        }
        Err(i) => g.seeds.insert(
            i,
            SeedCohort {
                species,
                organic,
                mineral,
                bin_start_tick: start,
            },
        ),
    }
}

// ---------------------------------------------------------------- 9. propagules

/// The smallest package worth sending: the seed-bank organic matter a germination needs to
/// build a stand at exactly `alive_min` of wood, `alive_min / w_frac` — 0.05 at the
/// placeholders. Stated as a division guarded by the caller, never a tuned constant.
///
/// Zero for a species whose `w_frac` is zero, which can never germinate anything.
pub(crate) fn package_of(sc: &SpeciesConfig) -> f64 {
    let w_frac = sc.propagule_split[0];
    if w_frac > 0.0 {
        sc.alive_min / w_frac
    } else {
        0.0
    }
}

/// §4.8, round 3b: a stand doing well **saves** for one neighbour at a time, and nothing
/// arrives from outside.
///
/// Every tick, a stand over `donor_min` asks for one recipient's worth of material —
/// `propagule_rate · dt`, gross — and is funded out of whatever its reserve holds above
/// its own `donor_reserve_floor`. What it can pay is respired for construction (`c_g`) and
/// the rest is saved in [`Stand::parcel`]. When the parcel holds one whole minimum package
/// ([`package_of`], `alive_min / w_frac`), that package lands on **one** support face
/// within the donor's `hop`, and the remainder keeps saving.
///
/// Astra's R4.4 is the reason: the old rule multiplied the budget by the recipient count
/// and then split it, so `propagule_rate` was a rate *per recipient* paid out of one
/// reserve. A bloomcrown with 24 recipients in `hop` 2 gave each of them a
/// twenty-fourth of what it could afford, and no bank came within a seventh of the
/// germination threshold in 2,000 s. The rate is unchanged; what changed is that the donor
/// pays for one package instead of pretending to pay for two dozen.
///
/// The recipient is drawn from the donor's own deterministic stream, keyed by the world's
/// seed, the donor's site and the tick — never its own site, and with **no habitat
/// screening**: the predicate is germination's test, not landing's, so a package can land
/// on an occupied site and wait for the gap, or on a site that will never germinate it and
/// decay there. Two species' cohorts can share one site, and there is no contest to
/// settle until germination.
fn propagate(flora: &mut Flora, world: &World) {
    let Flora {
        config,
        tick,
        stands,
        ground,
        ledger,
        deliveries,
        ..
    } = flora;
    let tick = *tick;
    let view = world.view();
    let world_seed = view.config.seed;

    for si in 0..stands.len() {
        let donor = stands[si];
        let sc = config.species(donor.species);
        let e_v = sc.energy_density;
        let slot = donor.species.index();
        let build = 1.0 + sc.build;

        // ---- what the rate asks for, and what the reserve can fund.
        if donor.wood >= sc.donor_min {
            let ask = (sc.propagule_rate * DT).max(0.0);
            ledger.propagule_requested[slot] += ask / build;
            let floor = sc.donor_reserve_floor * sc.reserve_cap * donor.wood;
            let take = (donor.reserve - floor)
                .max(0.0)
                .min(ask)
                .min(donor.reserve.max(0.0));
            if take > 0.0 {
                let net = take / build;
                stands[si].reserve -= take;
                stands[si].parcel += net;
                ledger.propagule_funded[slot] += net;
                // Construction respiration, paid when the material is set aside: organic
                // matter out of the system, energy to heat, and no mineral moves — the
                // parcel's mineral is still in the stand and travels only when the package
                // does.
                ledger.respired_out += take - net;
                ledger.heat_out += e_v * (take - net);
            }
        }

        // ---- one whole package or nothing. A stand that has fallen below `donor_min`
        // still delivers material it has already paid for; nothing that is paid for is
        // stranded.
        let package = package_of(sc);
        if package <= 0.0 || stands[si].parcel < package {
            continue;
        }
        let Some(site) = dispersal_target(&view, sc, &donor, world_seed, tick) else {
            continue;
        };
        let before = stands[si].material();
        stands[si].parcel -= package;
        // The package takes the same fraction of the donor's mineral as it is of the
        // donor's whole material, parcel included: the fraction rule, with the parcel
        // counted because its mineral never left the stand.
        let mineral = pull_mineral(&mut stands[si], before, package);
        ledger.propagule_landed[slot] += package;

        let gi = match ground.binary_search_by_key(&site, |g| g.site) {
            Ok(i) => i,
            Err(i) => {
                ledger.seeded_mineral_in += config.initial_mineral;
                ground.insert(i, Ground::new(site, config.initial_mineral));
                i
            }
        };
        add_cohort(&mut ground[gi], donor.species, package, mineral, tick, sc);
        // The receipt, written where the destination is actually known. Nothing downstream
        // reads it and no total includes it (Astra R10.3).
        deliveries.push(crate::DeliveryReceipt {
            tick,
            donor: donor.id,
            species: donor.species,
            recipient: site,
            organic: package,
            mineral,
        });
    }
}

/// The one site this tick's package lands on: the **highest** support face of each column
/// within `hop` of the donor in `x` and `z` is a candidate — one per column, so a stand
/// cannot seed the terraces below its own — and one of them is drawn uniformly from a
/// stream keyed by the world's seed, the donor's own site and the tick. Never the donor's
/// own site, and no habitat screening.
///
/// The candidate list is built in a fixed geometric order and the draw is an index into
/// it, so nothing about how the stands were reached or how the ground is stored can move
/// the choice; the same world, donor and tick always choose the same site.
fn dispersal_target(
    view: &VoxelView<'_>,
    sc: &SpeciesConfig,
    donor: &Stand,
    world_seed: u64,
    tick: u64,
) -> Option<Site> {
    let mut targets: Vec<Site> = Vec::new();
    let hop = sc.hop as i64;
    for dz in -hop..=hop {
        let z = donor.site.z as i64 + dz;
        if z < 0 || z >= view.config.depth as i64 {
            continue;
        }
        for dx in -hop..=hop {
            let x = donor.site.x as i64 + dx;
            let Some(site) = crate::highest_support(view, x, z as u32) else {
                continue;
            };
            if site == donor.site || targets.contains(&site) {
                continue;
            }
            targets.push(site);
        }
    }
    if targets.is_empty() {
        return None;
    }
    let home = view
        .config
        .index(donor.site.x as i64, donor.site.y, donor.site.z) as u64;
    let mut rng = Rng::keyed(DOMAIN_DISPERSAL, world_seed, home, tick);
    Some(targets[rng.below(targets.len())])
}

/// The species' establishment predicate: wet enough for its roots, **aerated** enough for
/// them, bright enough for its leaves, something to eat if it eats wood, and not already
/// under water it cannot stand in.
///
/// The aeration bound reads the site's saturated fraction *now*, not a stress that a
/// cohort has no way to carry. It is the half of correction 2 that makes saturation cost
/// something on the way in: the intolerant species is shut out of the basin instead of
/// merely doing badly there, which is what Chesson's test needs to have anywhere to bite.
fn establishes(
    view: &VoxelView<'_>,
    sky: &mut Vec<(Site, f64)>,
    ground: &[Ground],
    site: Site,
    sc: &SpeciesConfig,
) -> bool {
    let visibility = sky_at(sky, view, site);
    let (dead_wood, litter) = substrate_pools_in_box(view, ground, site, sc);
    gates(view, site, sc, visibility, dead_wood, litter).passes()
}

/// The same predicate for a caller outside a tick — a harness picking founders, a
/// presenter shading the sites a species could take — reading sky visibility straight off
/// the view instead of off `step`'s cache. The cache is memoized geometry, so the two
/// agree by construction, and there is exactly **one** germination predicate in the crate
/// for anything to agree with.
///
/// It reads **no substrate**, because a `VoxelView` holds none: for a
/// [`Trophic::Saprotroph`] species this therefore refuses every site, and the caller that
/// means to ask about a fungus wants [`crate::FloraView::can_establish`], which supplies
/// the dead wood out of the flora's own ground. Still one predicate: the substrate is an
/// input to it, exactly as sky visibility is.
///
/// Package I had to replicate the private one line for line in `examples/two_producers.rs`
/// to say which germination gate was shut, and the harness's own founder-selection
/// predicate had drifted from it: it read the support voxel's own pore fraction and a
/// binary saturation test where this reads the capacity-weighted mean and the saturated
/// *fraction* over the whole root box, which on a slope reaches sideways into neighbouring
/// columns.
pub fn can_establish(view: &VoxelView<'_>, site: Site, sc: &SpeciesConfig) -> bool {
    establishment_gates(view, site, sc).passes()
}

/// What each gate of the predicate read at one site, and which of them passed:
/// [`can_establish`] is `passes()` on this, so a caller that wants to know **why** a site
/// is refused reads the same numbers the model refused it on (Astra R5.2 — a second
/// approximate predicate in a harness is what package J deleted, and this is the thing to
/// use instead).
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
    /// Capacity-weighted mean pore fraction of the root box, `None` for a box with no pore
    /// space in it at all.
    pub mean_pore: Option<f64>,
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
    /// `mean_pore >= establish_pore_min`, and false for a box with no soil in it.
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
/// the substrate as **nothing**. The one predicate, in the form that says which gate shut.
///
/// A `VoxelView` holds no ground stocks, so this is the form for the five plants; for a
/// [`Trophic::Saprotroph`] it reports `substrate_ok: false` and refuses, which is what
/// "there is no log here as far as I can see" means. [`establishment_gates_on_substrate`]
/// is the same function with the reading supplied, and
/// [`crate::FloraView::establishment_gates`] takes it off the flora's own ground.
pub fn establishment_gates(view: &VoxelView<'_>, site: Site, sc: &SpeciesConfig) -> Gates {
    establishment_gates_on_substrate(view, site, sc, 0.0, 0.0)
}

/// [`establishment_gates`] with the substrate of the species' mycelium box handed in —
/// its dead wood and its litter, the two stocks a saprotroph eats, reported apart and
/// gated on their sum: the substrate is an **input** to the one predicate, the way sky
/// visibility is, and not a second predicate.
pub fn establishment_gates_on_substrate(
    view: &VoxelView<'_>,
    site: Site,
    sc: &SpeciesConfig,
    dead_wood: f64,
    litter: f64,
) -> Gates {
    let visibility = view.sky_visibility(site.x as i64, site.y, site.z);
    gates(view, site, sc, visibility, dead_wood, litter)
}

/// [`establishment_gates_on_substrate`] with the geometric sky reading handed in by a
/// caller that already has it. The **same** predicate, the same thresholds and the same
/// readings of pore water, saturation, standing water and substrate — only the hemisphere
/// ray's result is supplied, and it is exactly [`VoxelView::sky_visibility`].
///
/// A study sweeping one skyline for all six species caches one reading per site and shares
/// it through [`crate::SkyCache`], so the gate fields and the verdict are identical to the
/// uncached call and the ray is cast once per site instead of once per species.
pub fn establishment_gates_with_sky(
    view: &VoxelView<'_>,
    site: Site,
    sc: &SpeciesConfig,
    sky_visibility: f64,
    dead_wood: f64,
    litter: f64,
) -> Gates {
    gates(view, site, sc, sky_visibility, dead_wood, litter)
}

/// The same, for a caller that already has a sky reading — `step`'s memoized cache, which
/// is the same geometry by construction.
fn gates(
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
    let mean_pore = mean_pore(view, &box_);
    let saturated_fraction = saturated_fraction(view, &box_, sc);
    let water_depth_m = view.water_depth_m(site.x as i64, site.y, site.z);
    let standing_depth_m = if sc.water_depth_min_m > 0.0 {
        standing_water_beside(view, site)
    } else {
        0.0
    };
    Gates {
        soil_voxels: box_.len(),
        mean_pore,
        saturated_fraction,
        water_depth_m,
        sky_visibility,
        dead_wood,
        litter,
        pore_ok: mean_pore.is_some_and(|mean| mean >= sc.establish_pore_min),
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
            Trophic::Saprotroph => dead_wood + litter >= sc.establish_substrate_min,
        },
        standing_depth_m,
        standing_ok: sc.water_depth_min_m <= 0.0 || standing_depth_m >= sc.water_depth_min_m,
    }
}

/// The deepest **settled standing water** on `site`'s own face or beside it, metres
/// (package N, model addition 2): [`VoxelView::standing_depth_m`] on the site, and on
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
mod tests {
    use super::*;

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

    /// The package a donor saves for is the material a germination needs to build a stand
    /// at exactly `alive_min` of wood, so `w_frac · package` must not land **under**
    /// `alive_min` in f64 — a newborn a hair under it would be born and die on its first
    /// tick. It is exact at every preset's placeholders, and this is the test a new preset
    /// has to keep passing: round 4 added three, whose packages are 0.015, 0.025 and 0.0375
    /// against the original pair's 0.05.
    #[test]
    fn one_package_builds_a_stand_at_exactly_alive_min() {
        let config = FloraConfig::default();
        for species in Species::ALL {
            let sc = config.species(species).clone();
            let package = package_of(&sc);
            let want = sc.alive_min / 0.4;
            assert!(
                (package - want).abs() < 1e-15,
                "{}: a {package} package for a {want} split",
                species.name()
            );
            let wood = sc.propagule_split[0] * package;
            assert!(
                wood >= sc.alive_min,
                "a package builds {wood} of wood, under {}",
                sc.alive_min
            );
            assert!(
                wood - sc.alive_min <= 1e-15,
                "a package builds {wood}, which is not alive_min {}",
                sc.alive_min
            );
        }
        // A species that cannot put anything into wood has no package at all, and
        // `propagate` and germination both skip it rather than dividing by zero.
        let mut odd = SpeciesConfig::bloomcrown();
        odd.propagule_split = [0.0, 0.5, 0.5];
        assert_eq!(package_of(&odd), 0.0);
    }

    /// The gap lottery: weighted by whole packages, reproducible per (world, site, tick),
    /// and **blind to the order the candidates arrive in** — which is R4.5's requirement,
    /// since that order is `Species::ALL` today and could be a storage order tomorrow.
    ///
    /// Two hundred seeded draws at weights 1 and 3: both species win somewhere, the counts
    /// sit near the weights, and swapping the two candidates round gives the identical
    /// winner on all two hundred.
    #[test]
    fn the_gap_lottery_follows_the_weights_and_not_the_order_it_is_handed() {
        let (b, u) = (Species::Bloomcrown, Species::Umbrellafrond);
        let mut wins = [0usize; 2];
        let mut swapped_disagreements = 0;
        for site in 0..200u64 {
            let one = lottery(11, site, 3, &[(b, 1), (u, 3)]).expect("two candidates");
            let other = lottery(11, site, 3, &[(u, 3), (b, 1)]).expect("two candidates");
            if one != other {
                swapped_disagreements += 1;
            }
            wins[one.index()] += 1;
        }
        assert_eq!(
            swapped_disagreements, 0,
            "the order the candidates came in moved the winner"
        );
        assert!(
            wins[0] > 0 && wins[1] > 0,
            "one species never won: {wins:?}"
        );
        // 1:3 over 200 draws is 50 against 150; anything inside 35..65 is the weights and
        // not the enum order, which would be 200 against 0.
        assert!(
            (35..=65).contains(&wins[0]),
            "weights 1 and 3 gave {wins:?}"
        );
        assert_eq!(wins[0] + wins[1], 200);

        // Degenerate cases: nothing to draw among, and one candidate that always wins
        // whatever its weight.
        assert_eq!(lottery(11, 0, 0, &[]), None);
        assert_eq!(lottery(11, 0, 0, &[(u, 1)]), Some(u));
        assert_eq!(
            lottery(11, 0, 0, &[(b, 0), (u, 0)]),
            None,
            "no packages, no winner"
        );
    }

    /// Spending a bank takes the **oldest** bin first, leaves a part-spent bin ageing on
    /// its own start tick, and removes an emptied one whole with its mineral.
    #[test]
    fn spending_a_bank_empties_its_oldest_bins_first() {
        let mut g = Ground::new(Site { x: 0, y: 1, z: 0 }, 0.0);
        for (start, organic) in [(0u64, 0.02), (10, 0.04), (20, 0.06)] {
            g.seeds.push(SeedCohort {
                species: Species::Bloomcrown,
                organic,
                mineral: 0.02 * organic,
                bin_start_tick: start,
            });
        }
        // Another species' bin, to be left strictly alone.
        g.seeds.push(SeedCohort {
            species: Species::Umbrellafrond,
            organic: 1.0,
            mineral: 0.02,
            bin_start_tick: 0,
        });

        let (organic, mineral) = spend_bank(&mut g, Species::Bloomcrown, 0.05);
        assert!((organic - 0.05).abs() < 1e-15, "spent {organic}");
        // Density is uniform here, so the mineral is the same fraction.
        assert!(
            (mineral - 0.02 * 0.05).abs() < 1e-15,
            "took {mineral} of mineral"
        );
        // The 0.02 bin is gone whole and the 0.04 one is down to 0.01, still on tick 10.
        let bloom: Vec<&SeedCohort> = g
            .seeds
            .iter()
            .filter(|c| c.species == Species::Bloomcrown)
            .collect();
        assert_eq!(bloom.len(), 2, "{:?}", g.seeds);
        assert_eq!(bloom[0].bin_start_tick, 10);
        assert!((bloom[0].organic - 0.01).abs() < 1e-15, "{:?}", bloom[0]);
        assert_eq!(bloom[1].bin_start_tick, 20);
        assert!((bloom[1].organic - 0.06).abs() < 1e-15, "{:?}", bloom[1]);
        // The other species is untouched, and a bank with less than is asked for gives
        // what it has rather than going negative.
        let frond = g
            .seeds
            .iter()
            .find(|c| c.species == Species::Umbrellafrond)
            .expect("kept");
        assert_eq!((frond.organic, frond.mineral), (1.0, 0.02));
        let (rest, _) = spend_bank(&mut g, Species::Bloomcrown, 1.0);
        assert!((rest - 0.07).abs() < 1e-15, "a short bank gave {rest}");
        assert!(
            g.seeds.iter().all(|c| c.species == Species::Umbrellafrond),
            "{:?}",
            g.seeds
        );
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
                let want = pore * Material::Soil.pore_capacity() * w.config().voxel_volume();
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

    /// The three residuals, as the integration fixtures compute them.
    fn residuals(flora: &Flora) -> (f64, f64, f64) {
        let v = flora.view();
        (
            v.organic() - v.ledger.expected_organic(),
            v.mineral() - v.ledger.expected_mineral(),
            v.energy() - v.ledger.expected_energy(),
        )
    }

    /// **Astra's R5.1 case, end to end.** A bank of three oldest-first bins holding
    /// `0.001, 0.009, 0.04` is one whole default package — and spending it across the three
    /// of them returns `0.04999999999999999`, 6.9e-18 short of the package itself, so the
    /// old `w_frac · organic` built wood of `0.019999999999999997`: **below** `alive_min`
    /// by 3.5e-18. Frozen (`assimilation`, `maintenance` and `senescence` all zero, so
    /// nothing can take a unit of wood off it), the next growth pass killed that newborn on
    /// the §4.7 death test. Now the wood is `alive_min` exactly and the difference is in
    /// the reserve.
    ///
    /// The bank is injected rather than donated because no donor can produce these three
    /// amounts: a landing is always exactly one package. It is booked in as seeded material
    /// the way `Command::Seed` books a founder, so the three residuals still mean something.
    #[test]
    fn a_funded_birth_across_three_bins_is_born_alive_and_stays_alive() {
        let mut config = FloraConfig::default();
        config.bloomcrown.assimilation = 0.0;
        config.bloomcrown.maintenance = 0.0;
        config.bloomcrown.senescence = 0.0;
        let sc = config.bloomcrown.clone();
        let e_v = sc.energy_density;
        let n_tissue = sc.n_tissue;
        let package = package_of(&sc);
        assert_eq!(
            package, 0.049999999999999996,
            "the default package this case is about"
        );

        let mut world = slab(4, 0.6);
        let mut flora = Flora::new(config);
        let site = Site { x: 1, y: 2, z: 0 };

        // The bank: three bins, oldest first, summing to exactly 0.05 of organic matter.
        // Their start ticks are hand-set one tick apart to force the three-bin spend; the
        // placeholders' bin is 3,000 ticks wide, so a donor would have put all of this in
        // one bin.
        let mut g = Ground::new(site, 1.0);
        let mut banked_organic = 0.0;
        let mut banked_mineral = 0.0;
        for (start, organic) in [(0u64, 0.001), (1, 0.009), (2, 0.04)] {
            let mineral = n_tissue * organic;
            g.seeds.push(SeedCohort {
                species: Species::Bloomcrown,
                organic,
                mineral,
                bin_start_tick: start,
            });
            banked_organic += organic;
            banked_mineral += mineral;
        }
        assert_eq!(
            banked_organic, 0.05,
            "the bank is one package, and a hair over it"
        );
        assert!(
            banked_organic > package,
            "it has to be able to buy the package"
        );
        flora.ground.push(g);
        flora.ledger.seeded_organic_in += banked_organic;
        flora.ledger.seeded_mineral_in += banked_mineral + 1.0; // the site's own pool
        flora.ledger.seeded_energy_in += e_v * banked_organic;
        let (o, n, e) = residuals(&flora);
        assert!(
            o.abs() < 1e-18 && n.abs() < 1e-18 && e.abs() < 1e-18,
            "{o} {n} {e}"
        );

        // The birth.
        flora.step(&mut world);
        assert_eq!(
            flora.view().ledger.establishments,
            1,
            "the bank did not germinate"
        );
        let born = *flora.view().stand_at(site).expect("nothing stands here");
        assert_eq!(born.wood, sc.alive_min, "born with {} of wood", born.wood);
        assert!(
            born.wood >= sc.alive_min,
            "born under the death threshold: {}",
            born.wood
        );
        // What it holds is what was spent, to the bit in this case.
        let spent = 0.04999999999999999;
        assert_eq!(born.organic(), spent, "born holding {}", born.organic());
        assert!(
            born.organic() < package,
            "the fixture's premise: the spend is short"
        );
        // The intended split is recovered to a hair, and the difference is in the reserve.
        assert!(
            (born.foliage - sc.propagule_split[1] * spent).abs() < 1e-17,
            "{born:?}"
        );
        assert!(
            (born.reserve - sc.propagule_split[2] * spent).abs() < 1e-17,
            "{born:?}"
        );
        assert!(born.reserve > 0.0 && born.foliage > 0.0, "{born:?}");
        // Its mineral is the consumed bins' own, at the bank's density.
        assert!(
            (born.mineral / born.organic() - n_tissue).abs() < 1e-15,
            "mineral density {} against the bank's {n_tissue}",
            born.mineral / born.organic()
        );
        // What is left of the third bin is the 6.9e-18 the spend could not take, with its
        // mineral: real material, not deleted, and it ages out on its own bin's schedule.
        let left = flora
            .view()
            .ground_at(site)
            .expect("ground")
            .seed_organic(Species::Bloomcrown);
        assert!(left > 0.0 && left < 1e-17, "the third bin left {left}");

        // One frozen growth tick: nothing can take wood off it, so the only thing that
        // could kill it is the death test reading a rounded-down wood.
        flora.step(&mut world);
        assert_eq!(flora.view().ledger.deaths, 0, "the paid newborn was killed");
        let still = *flora
            .view()
            .stand_at(site)
            .expect("it died on its first growth tick");
        assert_eq!(still.wood, sc.alive_min, "its wood moved: {}", still.wood);
        assert_eq!(still.id, born.id, "a different stand is standing here");
        let (o, n, e) = residuals(&flora);
        let v = flora.view();
        assert!(
            o.abs() <= 1e-9 * v.organic().max(1.0),
            "organic residual {o}"
        );
        assert!(
            n.abs() <= 1e-9 * v.mineral().max(1.0),
            "mineral residual {n}"
        );
        assert!(e.abs() <= 1e-9 * v.energy().max(1.0), "energy residual {e}");
    }

    /// The same allocation against **every one of the five presets' own splits**, on the value Astra's three
    /// bins produce and on an exact package: the wood is `alive_min` on the nose, no stock
    /// is negative, the total is preserved, and the foliage and reserve are the intended
    /// fractions to within the rounding that is being corrected. The last two cases are the
    /// degenerate splits a future preset could bring — no reserve at all, and a bank that
    /// somehow holds less than `alive_min`.
    #[test]
    fn a_newborn_s_wood_is_exactly_alive_min_for_every_split() {
        let config = FloraConfig::default();
        for species in Species::ALL {
            let sc = config.species(species).clone();
            let [w_frac, p_frac, q_frac] = sc.propagule_split;
            assert!(
                (w_frac + p_frac + q_frac - 1.0).abs() < 1e-15,
                "the split sums to one"
            );
            assert!(sc.alive_min <= sc.wood_max, "alive_min over wood_max");
            for organic in [0.04999999999999999, package_of(&sc), 0.2] {
                let (wood, foliage, reserve) = newborn_stocks(&sc, organic);
                assert_eq!(wood, sc.alive_min, "wood {wood} for {organic} of material");
                assert!(
                    foliage >= 0.0 && reserve >= 0.0,
                    "{wood} {foliage} {reserve}"
                );
                assert!(
                    ((wood + foliage + reserve) - organic).abs() <= 4e-18,
                    "{wood} + {foliage} + {reserve} against {organic} paid"
                );
                assert!(
                    (foliage - p_frac * organic).abs() <= 1e-17,
                    "foliage {foliage}"
                );
                if organic <= package_of(&sc) {
                    assert!(
                        (reserve - q_frac * organic).abs() <= 1e-17,
                        "reserve {reserve}"
                    );
                }
            }
        }
        // A split with nothing in the reserve: the remainder is zero and never negative.
        let mut dry = SpeciesConfig::bloomcrown();
        dry.propagule_split = [0.5, 0.5, 0.0];
        let (wood, foliage, reserve) = newborn_stocks(&dry, package_of(&dry));
        assert_eq!(wood, dry.alive_min);
        assert!(reserve >= 0.0 && reserve < 1e-17, "reserve {reserve}");
        assert!((wood + foliage + reserve - package_of(&dry)).abs() <= 4e-18);
        // And a bank under `alive_min`, which the candidate filter cannot produce: the wood
        // takes all of it and the stand is born dying rather than born rich.
        let (wood, foliage, reserve) = newborn_stocks(&dry, 0.01);
        assert_eq!((wood, foliage, reserve), (0.01, 0.0, 0.0));
    }

    /// `can_establish` is `Gates::passes()` and nothing else, so there is still exactly one
    /// predicate in the crate — now in a form that says which gate shut. Read on a slab
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
        assert!(g.passes() && can_establish(&view, site, &sc), "{g:?}");
        assert_eq!(g.soil_voxels, 6, "three columns of two soil rows: {g:?}");
        assert_eq!(
            (g.pore_ok, g.aeration_ok, g.depth_ok, g.light_ok),
            (true, true, true, true)
        );
        assert_eq!(g.saturated_fraction, 0.0);
        assert_eq!(g.water_depth_m, 0.0);
        assert_eq!(g.sky_visibility, 1.0, "open sky");
        assert!((g.mean_pore.expect("soil") - 0.6).abs() < 1e-12);
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
        assert_eq!(g.mean_pore, None);
        assert!(!g.pore_ok && g.aeration_ok, "{g:?}");
        assert_eq!(g.passes(), can_establish(&view, site, &sc));
        assert!(!g.passes());
        drop(view);

        // A wholly saturated root box: the aeration gate shuts and the pore gate does not.
        let wet = slab(4, 0.98);
        let view = wet.view();
        let g = establishment_gates(&view, site, &sc);
        assert!(g.pore_ok && !g.aeration_ok, "{g:?}");
        assert_eq!(g.saturated_fraction, 1.0);
        assert_eq!(g.passes(), can_establish(&view, site, &sc));
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
        assert_eq!(g.passes(), can_establish(&view, site, &sc));
    }

    /// The bank's size bound is by construction and not by a cap: a lifetime in bins of
    /// `ceil(lifetime / seed_cohorts_max)` holds at most `seed_cohorts_max + 1` of them,
    /// whatever the two numbers are and whether or not they divide.
    #[test]
    fn the_bin_width_bounds_a_bank_at_the_cohort_cap_plus_one() {
        for cap in 1..=12usize {
            for &life_s in &[0.05, 0.1, 1.0, 2.0, 7.0, 600.0] {
                let mut sc = SpeciesConfig::bloomcrown();
                sc.seed_max_age_s = life_s;
                sc.seed_cohorts_max = cap;
                let w = bin_ticks(&sc);
                assert!(w >= 1, "a zero-width bin at cap {cap}, life {life_s}");
                // Every bin start a landing can produce, over one lifetime of ticks: the
                // bins alive at once are the distinct starts inside the lifetime, plus the
                // one the oldest is expiring out of.
                let life_ticks = (life_s / DT) as u64;
                let live = life_ticks / w + 1;
                assert!(
                    live <= cap as u64 + 1,
                    "cap {cap}, life {life_s}: {live} live bins of {w} ticks"
                );
            }
        }
        // The placeholders themselves: 600 s over 4 bins is 3,000 ticks, exactly.
        assert_eq!(bin_ticks(&SpeciesConfig::bloomcrown()), 3000);
        assert_eq!(bin_start(0, &SpeciesConfig::bloomcrown()), 0);
        assert_eq!(bin_start(2999, &SpeciesConfig::bloomcrown()), 0);
        assert_eq!(bin_start(3000, &SpeciesConfig::bloomcrown()), 3000);
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

    /// **Two fungi on one log, with unequal demands (Astra R9.3).** `feed` itself, not the
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
        let mut flora = Flora::new(FloraConfig::default());
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
                saturated: 0.0
            };
            2
        ];
        let out = feed(&mut flora, &world, &moisture);

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
}
