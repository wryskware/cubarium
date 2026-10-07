//! **Glassfilm** (`design/handoffs/voxel-roster-2026-10-01.md`, brief R2a): a thin living
//! film on shallow submerged ground, the fourth pool on [`Ground`] (`film`,
//! `film_mineral`, `film_energy`).
//!
//! It steps once per simulated second ([`FILM_EVERY`] flora ticks, `dt` scaled), after
//! decomposition, so what dies to litter this tick decomposes from the next. Per site
//! holding film, from the film it held when the step began:
//!
//! - **Eligible** while the standing water over its support face is at least
//!   `film_wet_min_depth_m` (and above 0) and at most `film_max_depth_m`. Shallower water
//!   is a **residue** and the site is dry: what a drained shower or a stopped film leaves
//!   on a ledge, below the depth the water solver still spreads, often under an overhang
//!   where evaporation never reaches it, so without the threshold such a ledge would read
//!   wet forever.
//! - **Growth** (eligible sites only): `film_growth_per_s · light · film · (1 − film / K)`,
//!   `K = film_capacity_per_m2 · cell area`, `light` = the plants' sky × canopy
//!   transmission × the day's daylight × `exp(−film_extinction_per_m · depth)`. No
//!   construction cost. New tissue draws `film_n_tissue` mineral per unit from the site's
//!   `mineral` and is capped by what the pool can pay for. Booked as plant production is:
//!   `fixed_in`, and `film_energy_density` per unit as `light_in`.
//! - **Upkeep** on every site holding film, wet or dry: `film_maintenance_per_s · film`,
//!   respired (`respired_out`, its energy at the stock's density as heat), its mineral
//!   back to the site's pool.
//! - **Death** on a dry or too-deep site: `film_dry_death_per_s · film` into the site's
//!   own litter, under litter's energy cap. In the step a dry film would fall below
//!   `film_gone_fraction` of the site's capacity, all of it dies (what upkeep does not
//!   respire goes to litter), so a dried site ends at exactly no film and leaves the film
//!   sites rather than keeping an exponential sliver.
//! - **Land-wet ground** (SNAIL-FIX, [`FilmWet::Land`]): a face with no more than a residue
//!   over it whose pore fill is at least `film_land_moisture_min` grows film at
//!   `film_land_growth_per_s · sky light · film · (1 − film / K_land)`,
//!   `K_land = film_land_capacity_per_m2 · cell area`; film above `K_land` there dies back
//!   at `film_dry_death_per_s` to litter; it gives and receives spread (receiving up to
//!   `K_land`). Drier than that it is a dry site. Film is a pool, not a cover slot.
//! - **Wetter ground and the shore** (SNAIL-LAND): a face within `film_shore_band_m` of
//!   standing water at or below it is land-wet from `film_shore_moisture_min`; and a
//!   land-wet face's capacity runs linearly with pore fill from `film_land_capacity_per_m2`
//!   at its floor to `film_land_capacity_wet_per_m2` at saturation ([`land_capacity_per_m2`]).
//! - **Spread** (between eligible and land-wet sites only, so a dry residue neither gives nor receives): an
//!   eligible site with less film gains from each eligible 4-neighbour
//!   `film_spread_per_s · neighbour film` (never more than half the difference, so a pair
//!   does not overshoot and swap), paid by the neighbour at its own densities. A target
//!   with no [`Ground`] is provisioned as a deposit's is.

use cubarium_voxel::{DT, TICK_HZ, VoxelView, World};

use crate::step::{add_litter_cap, provisioned_slot};
use crate::{Flora, FloraConfig, Site, take_pool};

/// Flora ticks per film step: one simulated second. A scheduling numeric, not ecology.
pub(crate) const FILM_EVERY: u64 = TICK_HZ as u64;

/// How many film steps fall in the plant step ending on `tick` and covering `span`
/// ticks: the whole seconds crossed, one per tick that is a multiple of [`FILM_EVERY`].
pub(crate) fn film_steps(tick: u64, span: u64) -> u64 {
    tick / FILM_EVERY - tick.saturating_sub(span) / FILM_EVERY
}

/// The standing water over support face `(x, y, z)`, if the face is wet-eligible: above 0,
/// at least `film_wet_min_depth_m` (shallower is a residue, dry) and at most
/// `film_max_depth_m`.
pub(crate) fn eligible_depth(
    view: &VoxelView<'_>,
    c: &FloraConfig,
    x: i64,
    y: u32,
    z: u32,
) -> Option<f64> {
    let d = view.standing_depth_m(x, y, z);
    (d > 0.0 && d >= c.film_wet_min_depth_m && d <= c.film_max_depth_m).then_some(d)
}

/// [`eligible_depth`] as a yes or no.
pub(crate) fn eligible(view: &VoxelView<'_>, c: &FloraConfig, x: i64, y: u32, z: u32) -> bool {
    eligible_depth(view, c, x, y, z).is_some()
}

/// **Where a site's film stands** (SNAIL-FIX): under water it is wet-eligible under
/// ([`eligible_depth`]), on **land-wet** ground (no deeper water than a residue, and the
/// face voxel's pore fill at least [`FloraConfig::film_land_moisture_min`]), or dry
/// (including too deep).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilmWet {
    /// Wet-eligible under this much standing water, metres.
    Under(f64),
    /// Land-wet ground.
    Land,
    /// Dry or too deep: the film dies.
    Dry,
}

/// The [`FilmWet`] class of support face `(x, y, z)`.
pub fn film_wet(view: &VoxelView<'_>, c: &FloraConfig, x: i64, y: u32, z: u32) -> FilmWet {
    classify(view, c, x, y, z).0
}

/// [`film_wet`] with, for a land-wet face, its capacity per square metre
/// ([`land_capacity_per_m2`]); 0 otherwise.
pub(crate) fn classify(view: &VoxelView<'_>, c: &FloraConfig, x: i64, y: u32, z: u32) -> (FilmWet, f64) {
    let d = view.standing_depth_m(x, y, z);
    if d > 0.0 && d >= c.film_wet_min_depth_m && d <= c.film_max_depth_m {
        return (FilmWet::Under(d), 0.0);
    }
    let cfg = view.config;
    if d > c.film_max_depth_m || z >= cfg.depth || y >= cfg.height || view.pore.is_empty() {
        return (FilmWet::Dry, 0.0);
    }
    let m = view.pore_at(x, y, z);
    if !m.is_finite() {
        return (FilmWet::Dry, 0.0);
    }
    // SNAIL-LAND: in the shore band the floor is the shore's; the band is only searched
    // for a face that is under the land floor and could clear the shore's.
    let floor = if m >= c.film_land_moisture_min {
        c.film_land_moisture_min
    } else if m >= c.film_shore_moisture_min && shore_band(view, c.film_shore_band_m, x, y, z) {
        c.film_shore_moisture_min
    } else {
        return (FilmWet::Dry, 0.0);
    };
    (FilmWet::Land, land_capacity_per_m2(c, m, floor))
}

/// **Land film capacity** (SNAIL-LAND), organic matter per square metre, on a face of
/// pore fill `m` whose moisture floor is `floor`: `film_land_capacity_per_m2` at the floor,
/// rising linearly to `film_land_capacity_wet_per_m2` at saturation (pore fill 1).
pub fn land_capacity_per_m2(c: &FloraConfig, m: f64, floor: f64) -> f64 {
    let span = 1.0 - floor;
    let t = if span > 0.0 { ((m - floor) / span).clamp(0.0, 1.0) } else { 1.0 };
    let (lo, hi) = (c.film_land_capacity_per_m2, c.film_land_capacity_wet_per_m2);
    (lo + (hi - lo) * t).max(0.0)
}

/// **The shore band** (SNAIL-LAND, [`FloraConfig::film_shore_band_m`]): whether a column
/// within `band_m` (horizontal, centre to centre; `x` wraps) of face `(x, y, z)` has its
/// highest support face at or below row `y` under standing water. The face's own column
/// counts. `false` for a band of 0.
pub fn shore_band(view: &VoxelView<'_>, band_m: f64, x: i64, y: u32, z: u32) -> bool {
    let cfg = view.config;
    let v = cfg.voxel_m;
    if !(band_m > 0.0) || !(v > 0.0) {
        return false;
    }
    let r = band_m / v;
    let k = (r + 1e-9).floor() as i64;
    for dz in -k..=k {
        let zz = i64::from(z) + dz;
        if zz < 0 || zz >= i64::from(cfg.depth) {
            continue;
        }
        for dx in -k..=k {
            if ((dx * dx + dz * dz) as f64) > r * r + 1e-9 {
                continue;
            }
            let xx = (x + dx).rem_euclid(i64::from(cfg.width));
            if let Some(yy) = view.highest_support_in(xx, zz as u32, 0, y)
                && view.standing_depth_m(xx, yy, zz as u32) > 0.0
            {
                return true;
            }
        }
    }
    false
}

/// **Land film census** (SNAIL-FIX readout): over every column's top support face, the
/// number of land-wet faces ([`FilmWet::Land`]); and over every site holding film, the
/// film on land-wet faces and the number of them holding any.
pub fn land_census(flora: &Flora, world: &World) -> (f64, usize, usize) {
    let view = world.view();
    let c = &flora.config;
    let cfg = view.config;
    let mut wet_faces = 0usize;
    for z in 0..cfg.depth {
        for x in 0..i64::from(cfg.width) {
            if let Some(y) = view.highest_support_in(x, z, 0, cfg.height.saturating_sub(1))
                && film_wet(&view, c, x, y, z) == FilmWet::Land
            {
                wet_faces += 1;
            }
        }
    }
    let (mut film, mut filmed) = (0.0, 0usize);
    for g in flora.ground.iter().filter(|g| g.film > 0.0) {
        let s = g.site;
        if film_wet(&view, c, i64::from(s.x), s.y, s.z) == FilmWet::Land {
            film += g.film;
            filmed += 1;
        }
    }
    (film, filmed, wet_faces)
}

/// Every site holding film, in site order: the receivers the light phase computes sky and
/// canopy transmission for on a film tick.
pub(crate) fn receivers(flora: &Flora) -> Vec<Site> {
    flora
        .ground
        .iter()
        .filter(|g| g.film > 0.0)
        .map(|g| g.site)
        .collect()
}

/// One film step. `open` is each [`receivers`] site's sky × canopy transmission, in the
/// same (site) order; `daylight` the day's share of light the plants use.
pub(crate) fn step(flora: &mut Flora, world: &World, open: &[(Site, f64)], daylight: f64, steps: u64) {
    let view = world.view();
    let dt = (steps * FILM_EVERY) as f64 * DT;
    let Flora {
        config: c,
        ground,
        ledger,
        ..
    } = flora;
    let voxel_m = view.config.voxel_m;
    let cap = c.film_capacity_per_m2 * voxel_m * voxel_m;
    let area = voxel_m * voxel_m;
    let (n_t, e_t) = (c.film_n_tissue, c.film_energy_density);

    // The step-start film of every site holding any, and the eligible ones' sites: what
    // spread reads, so its order of application changes no amount.
    let mut before: Vec<(Site, f64)> = Vec::new();
    let mut sources: Vec<(Site, f64)> = Vec::new();

    let mut k = 0;
    for g in ground.iter_mut() {
        let f = g.film;
        if !(f > 0.0) {
            continue;
        }
        let s = g.site;
        before.push((s, f));
        let (wet, land_k) = classify(&view, c, i64::from(s.x), s.y, s.z);
        let land_cap = land_k * area;
        let depth = match wet {
            FilmWet::Under(d) => Some(d),
            _ => None,
        };
        let land = wet == FilmWet::Land;
        while k < open.len() && open[k].0 < s {
            k += 1;
        }
        let sky = if k < open.len() && open[k].0 == s {
            open[k].1
        } else {
            0.0
        };

        // ---- growth, from the step-start film and the step-start pool
        let grown = match depth {
            Some(d) if cap > 0.0 => {
                let light = sky * daylight * (-c.film_extinction_per_m * d).exp();
                (c.film_growth_per_s * light * f * (1.0 - f / cap) * dt)
                    .max(0.0)
                    .min(cubarium_voxel::organism::tissue_funded(g.mineral, n_t))
            }
            // Land-wet ground (SNAIL-FIX): full sky light, its own rate and capacity.
            None if land && land_cap > 0.0 => {
                let light = sky * daylight;
                (c.film_land_growth_per_s * light * f * (1.0 - f / land_cap) * dt)
                    .max(0.0)
                    .min(cubarium_voxel::organism::tissue_funded(g.mineral, n_t))
            }
            _ => 0.0,
        };
        let dry = wet == FilmWet::Dry;
        if !dry {
            sources.push((s, f));
        }

        // ---- upkeep everywhere, death where dry or too deep, and on land-wet ground
        // what stands above the land capacity
        let mut respired = c.film_maintenance_per_s * dt * f;
        let mut died = if dry {
            c.film_dry_death_per_s * dt * f
        } else if land && f > land_cap {
            (c.film_dry_death_per_s * dt * (f - land_cap)).min(f - land_cap)
        } else {
            0.0
        };
        if respired + died > f {
            respired *= f / (respired + died);
            died = f - respired;
        }
        // A dry film that would fall under the floor dies whole this step: a dried site
        // ends at exactly nothing, not an exponential sliver.
        let gone = dry && f - respired - died < c.film_gone_fraction * cap;
        if gone {
            died = f - respired;
        }
        let lost = respired + died;
        if lost > 0.0 {
            // Pro rata out of the stock, then split between the two sinks.
            let all = gone || lost >= f;
            let lost_m = if all {
                g.film_mineral
            } else {
                g.film_mineral * (lost / f)
            };
            let lost_e = if all {
                g.film_energy
            } else {
                g.film_energy * (lost / f)
            };
            let (resp_m, resp_e) = (lost_m * respired / lost, lost_e * respired / lost);
            let (died_m, died_e) = (lost_m - resp_m, lost_e - resp_e);
            if all {
                g.film = 0.0;
                g.film_mineral = 0.0;
                g.film_energy = 0.0;
            } else {
                g.film -= lost;
                g.film_mineral -= lost_m;
                g.film_energy -= lost_e;
            }
            g.mineral += resp_m;
            ledger.respired_out += respired;
            ledger.heat_out += resp_e;
            ledger.film_respired += respired;
            if died > 0.0 {
                add_litter_cap(c.litter_energy_cap, g, died, died_m, died_e, ledger);
                ledger.film_to_litter += died;
            }
        }

        // ---- the new tissue, paid from the site's pool
        if grown > 0.0 {
            let drawn = (n_t * grown).min(g.mineral).max(0.0);
            g.mineral -= drawn;
            g.film += grown;
            g.film_mineral += drawn;
            g.film_energy += e_t * grown;
            ledger.fixed_in += grown;
            ledger.light_in += e_t * grown;
            ledger.film_produced += grown;
        }
    }

    // ---- spread, from the step-start film
    if !(c.film_spread_per_s > 0.0) || sources.is_empty() {
        return;
    }
    let rate = c.film_spread_per_s * dt;
    let depth_n = view.config.depth;
    let mut moves: Vec<(Site, Site, f64)> = Vec::new();
    for &(from, f) in &sources {
        for (dx, dz) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
            let z = i64::from(from.z) + dz;
            if z < 0 || z >= i64::from(depth_n) {
                continue;
            }
            let Some(to) =
                crate::seeds::support_near(&view, i64::from(from.x) + dx, z as u32, from.y)
            else {
                continue;
            };
            if to == from {
                continue;
            }
            // Onto wet-eligible ground, or land-wet ground up to its own capacity.
            let room = match classify(&view, c, i64::from(to.x), to.y, to.z) {
                (FilmWet::Under(_), _) => f64::INFINITY,
                (FilmWet::Land, k) => k * area,
                (FilmWet::Dry, _) => continue,
            };
            let there = before
                .binary_search_by_key(&to, |e| e.0)
                .map_or(0.0, |i| before[i].1);
            if there < f && there < room {
                let amount = (rate * f).min(0.5 * (f - there)).min(room - there);
                if amount > 0.0 {
                    moves.push((from, to, amount));
                }
            }
        }
    }
    for &(from, to, amount) in &moves {
        let t = provisioned_slot(c, ground, to, ledger);
        let Ok(s) = ground.binary_search_by_key(&from, |g| g.site) else {
            continue;
        };
        let src = &mut ground[s];
        let Some(moved) = take_pool(
            &mut src.film,
            &mut src.film_mineral,
            &mut src.film_energy,
            amount,
        ) else {
            continue;
        };
        let dst = &mut ground[t];
        dst.film += moved.organic;
        dst.film_mineral += moved.mineral;
        dst.film_energy += moved.energy;
    }
}
