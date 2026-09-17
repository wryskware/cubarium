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
//! 6. **Per stand**, in site order: §4.1–4.4 income, maintenance and growth, §4.5
//!    senescence, §4.6 dieback, §4.7 death. These three subphases are purely local once
//!    light and water are in hand, so running them per stand in one pass is the same
//!    arithmetic as three passes over every stand.
//! 7. **Decomposition** (§5), on the stocks the site held *before* this tick: litter and
//!    dead wood respire their organic matter out of the system and release their mineral
//!    to the site's pool at the same fraction; litter's energy leaves as heat at its
//!    current density.
//! 8. **The seed bank.** Every arrival bin pays its attrition into litter and falls to
//!    litter whole once its **start** is past its species' `seed_max_age_s`. Then every
//!    site with no stand holds a local lottery among the species whose bank there holds at
//!    least one whole package and which pass that species' establishment predicate,
//!    weighted by the packages each holds; the winner spends exactly **one** package out
//!    of its oldest bins and every other bank stays. This runs *after* the deaths of step
//!    6, so a gap opened this tick can be filled this tick.
//! 9. **Propagules** (§4.8), from one snapshot of donors and recipients. A package lands
//!    as a seed cohort, on a support face within the donor's `hop` whether it is occupied
//!    or not: the bank waits for the gap. A landing joins the arrival bin whose window
//!    covers this tick — creating it if it is not there — so the age of what is already
//!    banked does not move, and the bank is bounded by construction.
//!
//! Dropped from v1 by the brief: fruit (3c), downhill transport of litter (3f) and
//! nutrient diffusion (3g).
//!
//! Dead wood keeps its energy, per §5: `e_v` per unit of it goes into
//! `Ground::dead_wood_energy` when the wood diebacks or the stand dies, and leaves as
//! heat only as the wood decomposes, at the stock's current density. A standing dead
//! trunk is energy-dense and unavailable, which is the point of it being its own stock.
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

use cubarium_voxel::{Command as WorldCommand, Material, VoxelView, World, DT};

use crate::{
    Flora, FloraConfig, FloraLedger, Ground, SeedCohort, Site, Species, SpeciesConfig, Stage,
    Stand,
};

/// Stream keys, so two draws in one tick cannot be the same draw. One per rule that
/// draws.
const DOMAIN_DISPERSAL: u64 = 1;
const DOMAIN_GERMINATION: u64 = 2;

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

/// One stand's crown as the shade model sees it, taken before anything moves.
#[derive(Clone, Copy, Debug)]
struct Crown {
    x: f64,
    z: f64,
    /// Crown top in voxels above `y = 0`: the support face plus the species' crown height.
    top: f64,
    radius: f64,
    foliage: f64,
    /// `π r²` in voxels, at least 1: a sapling's crown is not a point source of shade.
    area: f64,
}

pub(crate) fn step(flora: &mut Flora, world: &mut World) {
    // The tick counter moves **first**, so that `flora.tick` is the tick this step
    // produces: the one whose state the caller will read when the step returns. Everything
    // dated inside a tick — an arrival bin's start, a bin's age, the germination lottery's
    // seed — then uses one clock reading, and a bin's age at the end of the tick it landed
    // in is zero rather than minus one.
    flora.tick += 1;
    // The stocks decomposition is allowed to draw on: what each site held when the tick
    // started, taken before anything at all moves. Litter and dead wood deposited by
    // this tick's drownings, senescence and deaths are eligible from the next tick, per
    // §5. Sites this tick removes are simply never looked up again.
    let pre: Vec<(Site, f64, f64)> =
        flora.ground.iter().map(|g| (g.site, g.litter, g.dead_wood)).collect();

    prune_unsupported(flora, world);
    refresh_sky_cache(flora, world);
    drown(flora, world);

    let light = light_per_stand(flora, world);
    let moisture = drink(flora, world);
    grow(flora, &light, &moisture);
    decompose(flora, &pre);
    seed_bank(flora, world);
    propagate(flora, world);
}

// ------------------------------------------------------------------ 1. terrain

/// Drop every site whose support face a terrain edit took away, booking its stocks out.
fn prune_unsupported(flora: &mut Flora, world: &World) {
    let Flora { config, stands, ground, ledger, .. } = flora;
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
            ledger.removed_organic_out += g.litter + g.dead_wood;
            ledger.removed_mineral_out += g.mineral + g.litter_mineral + g.dead_wood_mineral;
            ledger.removed_energy_out += g.litter_energy + g.dead_wood_energy;
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
            let value = view.sky_visibility(site.x as i64, site.y, site.z);
            cache.insert(i, (site, value));
            value
        }
    }
}

// ----------------------------------------------------------------- 3. drowning

fn drown(flora: &mut Flora, world: &World) {
    let Flora { config, stands, ground, ledger, .. } = flora;
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
        let gi = ground_slot(ground, stand.site);
        die(config, &stand, &mut ground[gi], ledger);
        stands.remove(i);
    }
}

// -------------------------------------------------------------------- 4. light

/// Light for every stand, in `stands` order: the site's own sky visibility times the
/// attenuation of every *taller* crown that covers its column, then the species' light
/// response. A stand never shades itself, and a crown level with another's crown top
/// does not shade it — only a strictly higher one does.
fn light_per_stand(flora: &mut Flora, world: &World) -> Vec<f64> {
    let Flora { config, stands, sky, .. } = flora;
    let view = world.view();
    let crowns: Vec<Crown> = stands.iter().map(|s| crown_of(config, s)).collect();
    let width = view.config.width as f64;

    let mut out = Vec::with_capacity(stands.len());
    for (i, stand) in stands.iter().enumerate() {
        let mut l = sky_at(sky, &view, stand.site);
        for (j, other) in crowns.iter().enumerate() {
            if j == i || other.top <= crowns[i].top {
                continue;
            }
            let dx = wrapped_delta(other.x, crowns[i].x, width);
            let dz = other.z - crowns[i].z;
            if dx * dx + dz * dz > other.radius * other.radius {
                continue;
            }
            l *= (-config.shade_k * other.foliage / other.area).exp();
        }
        out.push(light_response(config.species(stand.species), l));
    }
    out
}

fn crown_of(config: &FloraConfig, stand: &Stand) -> Crown {
    let sc = config.species(stand.species);
    let radius = sc.crown_radius(stand.wood);
    Crown {
        x: stand.site.x as f64,
        z: stand.site.z as f64,
        top: stand.site.y as f64 + sc.crown_height(stand.wood),
        radius,
        foliage: stand.foliage,
        area: (std::f64::consts::PI * radius * radius).max(1.0),
    }
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
        let accepted =
            -world.apply(WorldCommand::WithdrawPore { x: x as i64, y, z, volume_m3: total });
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
    if capacity <= 0.0 { None } else { Some(water / capacity) }
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
    let n = box_.iter().filter(|&&i| view.pore[i] >= sc.saturated_pore).count();
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

// ------------------------------------------- 6. income, growth, senescence, death

fn grow(flora: &mut Flora, light: &[f64], drink: &[Drink]) {
    let Flora { config, stands, ground, ledger, .. } = flora;
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
        let monod = if n0 + sc.nutrient_half > 0.0 { n0 / (n0 + sc.nutrient_half) } else { 0.0 };
        let mineral_cap = if sc.n_tissue > 0.0 { n0 / sc.n_tissue } else { f64::INFINITY };
        let a_pot = (sc.assimilation * l_eff * mu * (1.0 - stress) * p0 * monod * DT)
            .min(sc.nutrient_draw_max * n0 * DT)
            .min(mineral_cap)
            .max(0.0);
        let p_cap = sc.alpha * w0;
        let q_max = sc.reserve_cap * w0;
        let m = sc.maintenance * w0 * DT;
        let d_p = (p_cap - p0).max(0.0).min(sc.foliage_rate * w0 * DT);
        let d_w = (sc.wood_max - w0).max(0.0).min(sc.wood_rate * w0 * DT);
        let d_q = (q_max - q0).max(0.0);
        let build = 1.0 + sc.build;
        let a = a_pot.min(m + build * (d_p + d_w) + d_q);

        ledger.fixed_in += a;
        ledger.light_in += e_v * a;

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

        // ---- 4.4 growth: reserve share, foliage, wood, then the rest to reserve
        //
        // Every increment is floored at zero as well as capped by its demand. The caps
        // above already keep `rem` non-negative in exact arithmetic; in f64 a
        // `rem - build * (rem / build)` can land a few ulps below it, and a negative
        // "growth" would take a stock down instead of up.
        let dq_s = (sc.reserve_share * rem).min(d_q).max(0.0);
        rem -= dq_s;
        stands[si].reserve += dq_s;

        let dp_a = (rem / build).min(d_p).max(0.0);
        rem -= build * dp_a;
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

        let dw = (rem / build).min(d_w).max(0.0);
        rem -= build * dw;
        stands[si].wood += dw;

        let dq_r = rem.min((d_q - dq_s).max(0.0)).max(0.0);
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
        // foliage — so its mineral is already in the stand and is not drawn again. The
        // `min` is float insurance: `A_pot`'s `mineral / n_tissue` cap already bounds
        // this by the pool, and only this stand draws on this site.
        let built = dq_s + dp_a + dw + dq_r;
        let draw = (sc.n_tissue * built).max(0.0).min(ground[gi].mineral.max(0.0));
        ground[gi].mineral -= draw;
        stands[si].mineral += draw;

        // ---- 4.5 senescence
        let shed = (sc.senescence * stands[si].foliage * DT).min(stands[si].foliage);
        let organic_before = stands[si].material();
        stands[si].foliage -= shed;
        let shed_mineral = pull_mineral(&mut stands[si], organic_before, shed);
        add_litter(config, &mut ground[gi], shed, shed_mineral, e_v * shed, ledger);

        // ---- 4.6 dieback
        let die_back = stands[si].wood.min(sc.dieback * unpaid);
        let organic_before = stands[si].material();
        stands[si].wood -= die_back;
        let die_back_mineral = pull_mineral(&mut stands[si], organic_before, die_back);
        ground[gi].dead_wood += die_back;
        ground[gi].dead_wood_mineral += die_back_mineral;
        ground[gi].dead_wood_energy += e_v * die_back;

        // ---- 4.7 death
        if stands[si].wood < sc.alive_min {
            dead.push(si);
        }
    }

    for &si in dead.iter().rev() {
        let stand = stands[si];
        let gi = ground_slot(ground, stand.site);
        die(config, &stand, &mut ground[gi], ledger);
        stands.remove(si);
    }
}

/// The mineral belonging to `moved` units of a stand's organic matter, taken out of the
/// stand's mineral stock. Proportional: a stand's mineral is a stock and not
/// `n_tissue · O`, because respiration leaves mineral behind, so a transfer of a
/// fraction of the organic matter has to take the same fraction of the mineral.
/// `before` is the stand's organic matter *including* `moved`.
fn pull_mineral(stand: &mut Stand, before: f64, moved: f64) -> f64 {
    if moved <= 0.0 || before <= 0.0 || stand.mineral <= 0.0 {
        return 0.0;
    }
    let out =
        if moved >= before { stand.mineral } else { (stand.mineral * (moved / before)).min(stand.mineral) };
    stand.mineral -= out;
    out
}

/// §4.7 death: wood to dead wood, foliage, reserve and any saved parcel to litter under the
/// energy cap,
/// and the stand's mineral split between the two in proportion to the organic matter
/// each takes.
fn die(config: &FloraConfig, stand: &Stand, g: &mut Ground, ledger: &mut FloraLedger) {
    let e_v = config.species(stand.species).energy_density;
    let organic = stand.material();
    let wood_mineral =
        if organic > 0.0 { (stand.mineral * (stand.wood / organic)).min(stand.mineral) } else { 0.0 };
    g.dead_wood += stand.wood;
    g.dead_wood_mineral += wood_mineral;
    g.dead_wood_energy += e_v * stand.wood;
    // A parcel dies with its donor: material saved for a package that will never be sent
    // falls where the foliage and the reserve fall, and its mineral goes with it.
    let shed = stand.foliage + stand.reserve + stand.parcel;
    let shed_mineral = (stand.mineral - wood_mineral).max(0.0);
    add_litter_cap(config.litter_energy_cap, g, shed, shed_mineral, e_v * shed, ledger);
    ledger.deaths += 1;
}

fn add_litter(
    config: &FloraConfig,
    g: &mut Ground,
    organic: f64,
    mineral: f64,
    energy: f64,
    ledger: &mut FloraLedger,
) {
    add_litter_cap(config.litter_energy_cap, g, organic, mineral, energy, ledger);
}

/// Litter takes the organic matter, the mineral that was in it, and as much of its energy
/// as `e_d_max · D` leaves room for; the rest of the energy is respired. The mineral is
/// never capped: it has nowhere else to be.
fn add_litter_cap(
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
fn decompose(flora: &mut Flora, pre: &[(Site, f64, f64)]) {
    let Flora { config, ground, ledger, .. } = flora;
    for g in ground.iter_mut() {
        // A site the tick created has nothing eligible yet.
        let (litter0, wood0) = match pre.binary_search_by_key(&g.site, |e| e.0) {
            Ok(i) => (pre[i].1, pre[i].2),
            Err(_) => continue,
        };
        let dec = (config.decomposition * DT * litter0).min(g.litter).max(0.0);
        if dec > 0.0 && g.litter > 0.0 {
            // Energy and mineral both leave at the stock's current density, so neither
            // density moves and the `e_d_max` cap needs no re-clamp. Never 0/0: guarded
            // above.
            let f = dec / g.litter;
            let out = g.litter_energy * f;
            g.litter_energy -= out;
            ledger.heat_out += out;
            let mineral = (g.litter_mineral * f).min(g.litter_mineral);
            g.litter_mineral -= mineral;
            g.mineral += mineral;
            g.litter -= dec;
            ledger.respired_out += dec;
        }
        let dec_w = (config.wood_decomposition * DT * wood0).min(g.dead_wood).max(0.0);
        if dec_w > 0.0 && g.dead_wood > 0.0 {
            let f = dec_w / g.dead_wood;
            let out = g.dead_wood_energy * f;
            g.dead_wood_energy -= out;
            ledger.heat_out += out;
            let mineral = (g.dead_wood_mineral * f).min(g.dead_wood_mineral);
            g.dead_wood_mineral -= mineral;
            g.mineral += mineral;
            g.dead_wood -= dec_w;
            ledger.respired_out += dec_w;
        }
    }
}

// ----------------------------------------------------------------- 8. seed bank

/// The seed bank: take every bin's attrition, cull the bins whose start is over-age, then
/// germinate where a bank can build a living stand.
fn seed_bank(flora: &mut Flora, world: &World) {
    let Flora { config, tick, stands, ground, ledger, sky, .. } = flora;
    let tick = *tick;
    let view = world.view();
    for g in ground.iter_mut() {
        age_cohorts(config, g, ledger, tick);
    }
    // In site order. Which species takes a bare site is a **local lottery** among the
    // banks that can build a stand here, weighted by the whole packages each holds, drawn
    // from a stream keyed by the world, the site and the tick. The winner spends exactly
    // one package out of its oldest bins; its own remainder and every loser's bank stay
    // where they are and go on ageing.
    let world_seed = view.config.seed;
    for g in ground.iter_mut() {
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
            if !establishes(&view, sky, g.site, sc) {
                continue;
            }
            candidates.push((species, packages.min(u32::MAX as f64) as u64));
        }
        let site_index = view.config.index(g.site.x as i64, g.site.y, g.site.z) as u64;
        let Some(species) = lottery(world_seed, site_index, tick, &candidates) else { continue };
        let sc = config.species(species);
        let [w_frac, p_frac, q_frac] = sc.propagule_split;
        // Exactly one package, oldest bins first, with each bin's own mineral in
        // proportion to what it gave up.
        let (organic, mineral) = spend_bank(g, species, package_of(sc));
        let at = match stands.binary_search_by_key(&g.site, |s| s.site) {
            Ok(_) => continue,
            Err(at) => at,
        };
        let id = ledger.births;
        ledger.births += 1;
        stands.insert(
            at,
            Stand {
                id,
                site: g.site,
                species,
                stage: Stage::Alive,
                wood: w_frac * organic,
                foliage: p_frac * organic,
                reserve: q_frac * organic,
                light: 0.0,
                moisture: 0.0,
                water_m3: 0.0,
                mineral,
                aeration_stress: 0.0,
                parcel: 0.0,
            },
        );
        ledger.establishments += 1;
    }
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
        let loss = (sc.seed_attrition_per_s * DT * c.organic).min(c.organic).max(0.0);
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

/// The width of one arrival bin in ticks: `seed_max_age_s / seed_cohorts_max`, floored at
/// one tick. Derived, not a knob of its own — the placeholders' 600 s over 4 bins is 150 s,
/// which is 3,000 ticks.
fn bin_ticks(sc: &SpeciesConfig) -> u64 {
    let ticks = sc.seed_max_age_s / sc.seed_cohorts_max.max(1) as f64 / DT;
    if ticks.is_finite() && ticks >= 1.0 { ticks as u64 } else { 1 }
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
    match g.seeds.binary_search_by(|c| (c.species, c.bin_start_tick).cmp(&(species, start))) {
        Ok(i) => {
            g.seeds[i].organic += organic;
            g.seeds[i].mineral += mineral;
        }
        Err(i) => {
            g.seeds.insert(i, SeedCohort { species, organic, mineral, bin_start_tick: start })
        }
    }
}

// ---------------------------------------------------------------- 9. propagules

/// The smallest package worth sending: the seed-bank organic matter a germination needs to
/// build a stand at exactly `alive_min` of wood, `alive_min / w_frac` — 0.05 at the
/// placeholders. Stated as a division guarded by the caller, never a tuned constant.
///
/// Zero for a species whose `w_frac` is zero, which can never germinate anything.
fn package_of(sc: &SpeciesConfig) -> f64 {
    let w_frac = sc.propagule_split[0];
    if w_frac > 0.0 { sc.alive_min / w_frac } else { 0.0 }
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
    let Flora { config, tick, stands, ground, ledger, .. } = flora;
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
            let take = (donor.reserve - floor).max(0.0).min(ask).min(donor.reserve.max(0.0));
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
        let Some(site) = dispersal_target(&view, sc, &donor, world_seed, tick) else { continue };
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
            let Some(site) = crate::highest_support(view, x, z as u32) else { continue };
            if site == donor.site || targets.contains(&site) {
                continue;
            }
            targets.push(site);
        }
    }
    if targets.is_empty() {
        return None;
    }
    let home = view.config.index(donor.site.x as i64, donor.site.y, donor.site.z) as u64;
    let mut rng = Rng::keyed(DOMAIN_DISPERSAL, world_seed, home, tick);
    Some(targets[rng.below(targets.len())])
}

/// The species' establishment predicate: wet enough for its roots, **aerated** enough for
/// them, bright enough for its leaves, and not already under water it cannot stand in.
///
/// The aeration bound reads the site's saturated fraction *now*, not a stress that a
/// cohort has no way to carry. It is the half of correction 2 that makes saturation cost
/// something on the way in: the intolerant species is shut out of the basin instead of
/// merely doing badly there, which is what Chesson's test needs to have anywhere to bite.
fn establishes(
    view: &VoxelView<'_>,
    sky: &mut Vec<(Site, f64)>,
    site: Site,
    sc: &SpeciesConfig,
) -> bool {
    establishes_but_for_light(view, site, sc)
        && sky_at(sky, view, site) >= sc.establish_light_min
}

/// The same predicate for a caller outside a tick — a harness picking founders, a
/// presenter shading the sites a species could take — reading sky visibility straight off
/// the view instead of off `step`'s cache. The cache is memoized geometry, so the two
/// agree by construction, and there is exactly **one** germination predicate in the crate
/// for anything to agree with.
///
/// Package I had to replicate the private one line for line in `examples/two_producers.rs`
/// to say which germination gate was shut, and the harness's own founder-selection
/// predicate had drifted from it: it read the support voxel's own pore fraction and a
/// binary saturation test where this reads the capacity-weighted mean and the saturated
/// *fraction* over the whole root box, which on a slope reaches sideways into neighbouring
/// columns.
pub fn can_establish(view: &VoxelView<'_>, site: Site, sc: &SpeciesConfig) -> bool {
    establishes_but_for_light(view, site, sc)
        && view.sky_visibility(site.x as i64, site.y, site.z) >= sc.establish_light_min
}

/// Every gate of the predicate but the light one, which needs a sky reading its caller
/// supplies.
fn establishes_but_for_light(view: &VoxelView<'_>, site: Site, sc: &SpeciesConfig) -> bool {
    let box_ = root_box(view, site, sc);
    match mean_pore(view, &box_) {
        None => return false,
        Some(mean) if mean < sc.establish_pore_min => return false,
        Some(_) => {}
    }
    if saturated_fraction(view, &box_, sc) > sc.establish_saturated_max {
        return false;
    }
    view.water_depth_m(site.x as i64, site.y, site.z) <= sc.drown_depth_m
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
        assert!((sa + sb - accepted).abs() < 1e-15, "{sa} + {sb} != {accepted}");
        assert!((sb / sa - 3.0).abs() < 1e-12, "three times the demand, three times the share");
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
        assert_eq!(aeration_target(2.0 / 18.0, &bloom), 0.0, "two saturated voxels of eighteen");
        assert_eq!(aeration_target(0.25, &bloom), 0.0, "at the tolerance, not past it");
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
    /// tick. It is exact at both species' placeholders, and this is the test a new preset
    /// has to keep passing.
    #[test]
    fn one_package_builds_a_stand_at_exactly_alive_min() {
        for sc in [SpeciesConfig::bloomcrown(), SpeciesConfig::umbrellafrond()] {
            let package = package_of(&sc);
            assert!((package - 0.05).abs() < 1e-15, "a {package} package at the placeholders");
            let wood = sc.propagule_split[0] * package;
            assert!(wood >= sc.alive_min, "a package builds {wood} of wood, under {}", sc.alive_min);
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
        assert_eq!(swapped_disagreements, 0, "the order the candidates came in moved the winner");
        assert!(wins[0] > 0 && wins[1] > 0, "one species never won: {wins:?}");
        // 1:3 over 200 draws is 50 against 150; anything inside 35..65 is the weights and
        // not the enum order, which would be 200 against 0.
        assert!((35..=65).contains(&wins[0]), "weights 1 and 3 gave {wins:?}");
        assert_eq!(wins[0] + wins[1], 200);

        // Degenerate cases: nothing to draw among, and one candidate that always wins
        // whatever its weight.
        assert_eq!(lottery(11, 0, 0, &[]), None);
        assert_eq!(lottery(11, 0, 0, &[(u, 1)]), Some(u));
        assert_eq!(lottery(11, 0, 0, &[(b, 0), (u, 0)]), None, "no packages, no winner");
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
        assert!((mineral - 0.02 * 0.05).abs() < 1e-15, "took {mineral} of mineral");
        // The 0.02 bin is gone whole and the 0.04 one is down to 0.01, still on tick 10.
        let bloom: Vec<&SeedCohort> =
            g.seeds.iter().filter(|c| c.species == Species::Bloomcrown).collect();
        assert_eq!(bloom.len(), 2, "{:?}", g.seeds);
        assert_eq!(bloom[0].bin_start_tick, 10);
        assert!((bloom[0].organic - 0.01).abs() < 1e-15, "{:?}", bloom[0]);
        assert_eq!(bloom[1].bin_start_tick, 20);
        assert!((bloom[1].organic - 0.06).abs() < 1e-15, "{:?}", bloom[1]);
        // The other species is untouched, and a bank with less than is asked for gives
        // what it has rather than going negative.
        let frond = g.seeds.iter().find(|c| c.species == Species::Umbrellafrond).expect("kept");
        assert_eq!((frond.organic, frond.mineral), (1.0, 0.02));
        let (rest, _) = spend_bank(&mut g, Species::Bloomcrown, 1.0);
        assert!((rest - 0.07).abs() < 1e-15, "a short bank gave {rest}");
        assert!(
            g.seeds.iter().all(|c| c.species == Species::Umbrellafrond),
            "{:?}",
            g.seeds
        );
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
        assert!(seen.iter().all(|&n| n > 60), "four candidates over 400 ticks: {seen:?}");
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
}
