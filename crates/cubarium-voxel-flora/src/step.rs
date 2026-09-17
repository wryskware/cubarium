//! One tick of the plant layer: the `design/ecology-v1-contract.md` §4 plant model with
//! light from the terrain's own geometry and water from the core's pore store.
//!
//! # Order within a tick
//!
//! 1. **Terrain.** A site whose support face is no longer a support — buried, dug out,
//!    turned to air — loses its stand and its ground, booked as `removed_*_out`.
//! 2. **Sky cache.** Dropped whole when `VoxelView::terrain_version` moved, refilled
//!    lazily per site.
//! 3. **Drowning.** An alive stand standing in water deeper than its species tolerates
//!    dies the §4.7 death. Establishing stands are frozen (§3.1) and do not drown.
//! 4. **Light**, from one snapshot of every stand's pre-tick crown: nothing a stand
//!    grows this tick shades anything this tick.
//! 5. **Water.** Every stand's demand per voxel is collected first; then each voxel
//!    issues exactly **one** `Command::WithdrawPore` for the total and the accepted
//!    volume is split among its demanders proportional to demand. `μ` for income is the
//!    one read taken before any withdrawal: a stand that got less than it asked for does
//!    not get a second read.
//! 6. **Per stand**, in site order: §4.1–4.4 income, maintenance and growth, §4.5
//!    senescence, §4.6 dieback, §4.7 death. These three subphases are purely local once
//!    light and water are in hand, so running them per stand in one pass is the same
//!    arithmetic as three passes over every stand.
//! 7. **Decomposition** (§5), on the stocks the site held *before* this tick: litter and
//!    dead wood return to nutrient, litter's energy leaves as heat at its current
//!    density.
//! 8. **Propagules** (§4.8), from one snapshot of donors and recipients.
//!
//! Dropped from v1 by the brief: fruit (3c), downhill transport of litter (3f) and
//! nutrient diffusion (3g).
//!
//! # Where this crate's accounting differs from the contract
//!
//! `FloraView::energy` counts the energy of living tissue and of litter, and there is no
//! term in it for dead wood — `Ground` has no species, so it has no `e_v` to count with.
//! So dead wood carries **no** energy here: the energy of wood that diebacks or dies is
//! respired to `heat_out` at the moment it becomes dead wood, and the decomposition of
//! dead wood moves material to nutrient and books no further heat. Booking it twice, or
//! not at all, would show up as a residual.
//!
//! Every clamp is a `min` against the stock it reads, so nothing here can go negative.

use cubarium_voxel::{Command as WorldCommand, Material, VoxelView, World, DT};

use crate::{Flora, FloraConfig, FloraLedger, Ground, Site, Species, SpeciesConfig, Stage, Stand};

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
    prune_unsupported(flora, world);
    refresh_sky_cache(flora, world);
    drown(flora, world);

    // The stocks decomposition is allowed to draw on: what the site held when the tick
    // started. Litter and dead wood deposited by this tick's senescence and deaths are
    // eligible from the next tick, per §5.
    let pre: Vec<(Site, f64, f64)> =
        flora.ground.iter().map(|g| (g.site, g.litter, g.dead_wood)).collect();

    let light = light_per_stand(flora, world);
    let moisture = drink(flora, world);
    grow(flora, &light, &moisture);
    decompose(flora, &pre);
    propagate(flora, world);

    flora.tick += 1;
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
            let material = stand.wood + stand.foliage + stand.reserve;
            ledger.removed_material_out += material;
            ledger.removed_energy_out += config.species(stand.species).energy_density * material;
        }
    }
    if kept.len() != stands.len() {
        *stands = kept;
    }

    let mut kept = Vec::with_capacity(ground.len());
    for g in ground.iter() {
        if supported(&g.site) {
            kept.push(*g);
        } else {
            ledger.removed_material_out += g.nutrient + g.litter + g.dead_wood;
            ledger.removed_energy_out += g.litter_energy;
        }
    }
    if kept.len() != ground.len() {
        *ground = kept;
    }
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
        if stand.stage != Stage::Alive {
            continue;
        }
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

/// What one stand got, and the moisture factor its income reads.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Drink {
    pub(crate) moisture: f64,
    pub(crate) taken_m3: f64,
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
            if stand.stage != Stage::Alive {
                continue;
            }
            let sc = flora.config.species(stand.species);
            let box_ = root_box(&view, stand.site, sc);
            out[i].moisture = moisture_of(&view, &box_, sc);
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
        if stands[si].stage != Stage::Alive {
            continue;
        }
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

        // ---- 4.1 potential income, 4.2 demands
        let n0 = ground[gi].nutrient;
        let monod = if n0 + sc.nutrient_half > 0.0 { n0 / (n0 + sc.nutrient_half) } else { 0.0 };
        let a_pot = (sc.assimilation * l_eff * mu * p0 * monod * DT)
            .min(sc.nutrient_draw_max * n0 * DT)
            .min(n0)
            .max(0.0);
        let p_cap = sc.alpha * w0;
        let q_max = sc.reserve_cap * w0;
        let m = sc.maintenance * w0 * DT;
        let d_p = (p_cap - p0).max(0.0).min(sc.foliage_rate * w0 * DT);
        let d_w = (sc.wood_max - w0).max(0.0).min(sc.wood_rate * w0 * DT);
        let d_q = (q_max - q0).max(0.0);
        let build = 1.0 + sc.build;
        let a = a_pot.min(m + build * (d_p + d_w) + d_q);

        ground[gi].nutrient -= a;
        ledger.light_in += e_v * a;

        // ---- 4.3 maintenance from income first, then reserve
        let paid_a = a.min(m);
        let mut rem = a - paid_a;
        let short = m - paid_a;
        let paid_q = stands[si].reserve.min(short);
        stands[si].reserve -= paid_q;
        let unpaid = short - paid_q;
        ground[gi].nutrient += paid_a + paid_q;
        ledger.heat_out += e_v * (paid_a + paid_q);

        // ---- 4.4 growth: reserve share, foliage, wood, then the rest to reserve
        let dq_s = (sc.reserve_share * rem).min(d_q);
        rem -= dq_s;
        stands[si].reserve += dq_s;

        let dp_a = (rem / build).min(d_p);
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
        ground[gi].nutrient += sc.build * (dp_a + dp_q);
        ledger.heat_out += e_v * sc.build * (dp_a + dp_q);

        let dw = (rem / build).min(d_w);
        rem -= build * dw;
        stands[si].wood += dw;
        ground[gi].nutrient += sc.build * dw;
        ledger.heat_out += e_v * sc.build * dw;

        let dq_r = rem.min((d_q - dq_s).max(0.0));
        stands[si].reserve += dq_r;
        rem -= dq_r;
        // Whatever the caps left over goes straight back: material to nutrient, its
        // energy to heat. With the §4.2 cap on `A` this is float residue only.
        ground[gi].nutrient += rem;
        ledger.heat_out += e_v * rem;

        // ---- 4.5 senescence
        let shed = (sc.senescence * stands[si].foliage * DT).min(stands[si].foliage);
        stands[si].foliage -= shed;
        add_litter(config, &mut ground[gi], shed, e_v * shed, ledger);

        // ---- 4.6 dieback
        let die_back = stands[si].wood.min(sc.dieback * unpaid);
        stands[si].wood -= die_back;
        ground[gi].dead_wood += die_back;
        ledger.heat_out += e_v * die_back;

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

/// §4.7 death: wood to dead wood, foliage and reserve to litter under the energy cap.
fn die(config: &FloraConfig, stand: &Stand, g: &mut Ground, ledger: &mut FloraLedger) {
    let e_v = config.species(stand.species).energy_density;
    g.dead_wood += stand.wood;
    // Dead wood carries no energy in this crate's accounting: see the module doc.
    ledger.heat_out += e_v * stand.wood;
    let shed = stand.foliage + stand.reserve;
    add_litter_cap(config.litter_energy_cap, g, shed, e_v * shed, ledger);
    ledger.deaths += 1;
}

fn add_litter(
    config: &FloraConfig,
    g: &mut Ground,
    material: f64,
    energy: f64,
    ledger: &mut FloraLedger,
) {
    add_litter_cap(config.litter_energy_cap, g, material, energy, ledger);
}

/// Litter takes the material and as much of its energy as `e_d_max · D` leaves room
/// for; the rest is respired.
fn add_litter_cap(
    cap: f64,
    g: &mut Ground,
    material: f64,
    energy: f64,
    ledger: &mut FloraLedger,
) {
    if material <= 0.0 && energy <= 0.0 {
        return;
    }
    g.litter += material;
    let room = (cap * g.litter - g.litter_energy).max(0.0);
    let kept = energy.min(room).max(0.0);
    g.litter_energy += kept;
    ledger.heat_out += energy - kept;
}

/// The site's ground entry, created empty if a stand somehow has none. Empty is the
/// only safe creation here: `initial_nutrient` is material, and material that is not
/// booked is a residual.
fn ground_slot(ground: &mut Vec<Ground>, site: Site) -> usize {
    match ground.binary_search_by_key(&site, |g| g.site) {
        Ok(i) => i,
        Err(i) => {
            ground.insert(
                i,
                Ground { site, nutrient: 0.0, litter: 0.0, litter_energy: 0.0, dead_wood: 0.0 },
            );
            i
        }
    }
}

// ------------------------------------------------------------ 7. decomposition

/// Litter and dead wood return to nutrient at their own rates, drawing on what the site
/// held **before** this tick: material this tick deposited is eligible from the next
/// one. Litter's energy leaves at the stock's current density, so decomposition never
/// changes that density and the `e_d_max` cap survives without a re-clamp.
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
            // Energy leaves at the stock's current density, so the density does not move
            // and the `e_d_max` cap needs no re-clamp. Never 0/0: guarded above.
            let out = g.litter_energy * (dec / g.litter);
            g.litter_energy -= out;
            ledger.heat_out += out;
            g.litter -= dec;
            g.nutrient += dec;
        }
        let dec_w = (config.wood_decomposition * DT * wood0).min(g.dead_wood).max(0.0);
        if dec_w > 0.0 {
            // No energy term: dead wood's energy was respired when it died.
            g.dead_wood -= dec_w;
            g.nutrient += dec_w;
        }
    }
}

// ---------------------------------------------------------------- 8. propagules

/// What one donor offers one recipient site this tick.
#[derive(Clone, Copy, Debug)]
struct Offer {
    site: Site,
    species: Species,
    donor: Site,
    material: f64,
}

/// §4.8: a stand doing well spends reserve on its neighbours, and nothing arrives from
/// outside.
///
/// Recipients are the **highest** support face of each column within `hop` of the donor
/// in `x` and `z` — one candidate per column for now, so a stand cannot seed the
/// terraces below its own. A candidate qualifies if it is bare, or holds an establishing
/// stand of the donor's own species, and passes the species' establishment predicate.
/// Where two species offer for the same bare site in one tick the larger total takes it
/// and the other donors keep their reserve, so nothing is created or lost by the tie.
fn propagate(flora: &mut Flora, world: &World) {
    let mut offers: Vec<Offer> = Vec::new();
    {
        let Flora { config, stands, sky, .. } = flora;
        let view = world.view();
        let donors: Vec<Stand> = stands
            .iter()
            .copied()
            .filter(|s| {
                let sc = config.species(s.species);
                s.stage == Stage::Alive
                    && s.wood >= sc.donor_min
                    && s.reserve > sc.donor_reserve_floor * sc.reserve_cap * s.wood
            })
            .collect();
        for donor in donors {
            let sc = config.species(donor.species);
            let mut targets: Vec<Site> = Vec::new();
            let hop = sc.hop as i64;
            for dz in -hop..=hop {
                let z = donor.site.z as i64 + dz;
                if z < 0 || z >= view.config.depth as i64 {
                    continue;
                }
                for dx in -hop..=hop {
                    let x = donor.site.x as i64 + dx;
                    let Some(site) = crate::highest_support(&view, x, z as u32) else { continue };
                    if targets.contains(&site) {
                        continue;
                    }
                    if !receptive(stands, site, donor.species) {
                        continue;
                    }
                    if !establishes(&view, sky, site, sc) {
                        continue;
                    }
                    targets.push(site);
                }
            }
            if targets.is_empty() {
                continue;
            }
            let floor = sc.donor_reserve_floor * sc.reserve_cap * donor.wood;
            let budget =
                (donor.reserve - floor).max(0.0).min(sc.propagule_rate * DT * targets.len() as f64);
            if budget <= 0.0 {
                continue;
            }
            let each = budget / targets.len() as f64;
            for site in targets {
                offers.push(Offer { site, species: donor.species, donor: donor.site, material: each });
            }
        }
    }
    if offers.is_empty() {
        return;
    }
    offers.sort_unstable_by(|a, b| {
        a.site.cmp(&b.site).then(a.species.cmp(&b.species)).then(a.donor.cmp(&b.donor))
    });

    let mut at = 0;
    while at < offers.len() {
        let site = offers[at].site;
        let mut end = at;
        while end < offers.len() && offers[end].site == site {
            end += 1;
        }
        let winner = winning_species(&offers[at..end]);
        commit_propagules(flora, site, winner, &offers[at..end]);
        at = end;
    }
}

/// The species that offered this site the most material; ties go to the earlier species
/// in `Species::ALL`, which is a fixed order, not an iteration order.
fn winning_species(offers: &[Offer]) -> Species {
    let mut best = offers[0].species;
    let mut best_total = 0.0;
    for species in Species::ALL {
        let total: f64 =
            offers.iter().filter(|o| o.species == species).map(|o| o.material).sum();
        if total > best_total {
            best_total = total;
            best = species;
        }
    }
    best
}

fn commit_propagules(flora: &mut Flora, site: Site, species: Species, offers: &[Offer]) {
    let Flora { config, stands, ground, ledger, .. } = flora;
    let sc = config.species(species);
    let e_v = sc.energy_density;

    // Debit the donors that actually sent, and nothing else.
    let mut sent = 0.0;
    for offer in offers.iter().filter(|o| o.species == species) {
        let Ok(di) = stands.binary_search_by_key(&offer.donor, |s| s.site) else { continue };
        let take = stands[di].reserve.min(offer.material);
        stands[di].reserve -= take;
        sent += take;
    }
    if sent <= 0.0 {
        return;
    }
    let net = sent / (1.0 + sc.build);

    let gi = match ground.binary_search_by_key(&site, |g| g.site) {
        Ok(i) => i,
        Err(i) => {
            ledger.seeded_material_in += config.initial_nutrient;
            ground.insert(
                i,
                Ground {
                    site,
                    nutrient: config.initial_nutrient,
                    litter: 0.0,
                    litter_energy: 0.0,
                    dead_wood: 0.0,
                },
            );
            i
        }
    };
    ground[gi].nutrient += sc.build * net;
    ledger.heat_out += e_v * sc.build * net;

    let [w_frac, p_frac, q_frac] = sc.propagule_split;
    let si = match stands.binary_search_by_key(&site, |s| s.site) {
        Ok(i) => i,
        Err(i) => {
            stands.insert(
                i,
                Stand {
                    site,
                    species,
                    stage: Stage::Establishing,
                    wood: 0.0,
                    foliage: 0.0,
                    reserve: 0.0,
                    light: 0.0,
                    moisture: 0.0,
                    water_m3: 0.0,
                },
            );
            i
        }
    };
    stands[si].wood += w_frac * net;
    stands[si].foliage += p_frac * net;
    stands[si].reserve += q_frac * net;
    if stands[si].stage == Stage::Establishing && stands[si].wood >= sc.alive_min {
        stands[si].stage = Stage::Alive;
        ledger.establishments += 1;
    }
}

/// Bare, or establishing in the donor's own species: a propagule of one species never
/// feeds another's stand.
fn receptive(stands: &[Stand], site: Site, species: Species) -> bool {
    match stands.binary_search_by_key(&site, |s| s.site) {
        Err(_) => true,
        Ok(i) => stands[i].stage == Stage::Establishing && stands[i].species == species,
    }
}

/// The species' establishment predicate: wet enough for its roots, bright enough for
/// its leaves, and not already under water it cannot stand in.
fn establishes(
    view: &VoxelView<'_>,
    sky: &mut Vec<(Site, f64)>,
    site: Site,
    sc: &SpeciesConfig,
) -> bool {
    let box_ = root_box(view, site, sc);
    match mean_pore(view, &box_) {
        None => return false,
        Some(mean) if mean < sc.establish_pore_min => return false,
        Some(_) => {}
    }
    if view.water_depth_m(site.x as i64, site.y, site.z) > sc.drown_depth_m {
        return false;
    }
    sky_at(sky, view, site) >= sc.establish_light_min
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
