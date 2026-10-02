//! **Glassfilm** (`design/handoffs/voxel-roster-2026-10-01.md`, brief R2a): a thin living
//! film on shallow submerged ground, the fourth pool on [`Ground`] (`film`,
//! `film_mineral`, `film_energy`).
//!
//! It steps once per simulated second ([`FILM_EVERY`] flora ticks, `dt` scaled), after
//! decomposition, so what dies to litter this tick decomposes from the next. Per site
//! holding film, from the film it held when the step began:
//!
//! - **Eligible** while the standing water over its support face is above 0 and at most
//!   `film_max_depth_m`.
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
//!   own litter, under litter's energy cap.
//! - **Spread**: an eligible site with less film gains from each eligible 4-neighbour
//!   `film_spread_per_s · neighbour film` (never more than half the difference, so a pair
//!   does not overshoot and swap), paid by the neighbour at its own densities. A target
//!   with no [`Ground`] is provisioned as a deposit's is.

use cubarium_voxel::{DT, TICK_HZ, VoxelView, World};

use crate::step::{add_litter_cap, provisioned_slot};
use crate::{Flora, FloraConfig, Site, take_pool};

/// Flora ticks per film step: one simulated second. A scheduling numeric, not ecology.
pub(crate) const FILM_EVERY: u64 = TICK_HZ as u64;

/// Whether the tick `tick` steps the film.
pub(crate) fn is_film_tick(tick: u64) -> bool {
    tick.is_multiple_of(FILM_EVERY)
}

/// The standing water over support face `(x, y, z)`, if the face is wet-eligible: above 0
/// and at most `film_max_depth_m`.
pub(crate) fn eligible_depth(
    view: &VoxelView<'_>,
    c: &FloraConfig,
    x: i64,
    y: u32,
    z: u32,
) -> Option<f64> {
    let d = view.standing_depth_m(x, y, z);
    (d > 0.0 && d <= c.film_max_depth_m).then_some(d)
}

/// [`eligible_depth`] as a yes or no.
pub(crate) fn eligible(view: &VoxelView<'_>, c: &FloraConfig, x: i64, y: u32, z: u32) -> bool {
    eligible_depth(view, c, x, y, z).is_some()
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
pub(crate) fn step(flora: &mut Flora, world: &World, open: &[(Site, f64)], daylight: f64) {
    let view = world.view();
    let dt = FILM_EVERY as f64 * DT;
    let Flora {
        config: c,
        ground,
        ledger,
        ..
    } = flora;
    let voxel_m = view.config.voxel_m;
    let cap = c.film_capacity_per_m2 * voxel_m * voxel_m;
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
        let depth = eligible_depth(&view, c, i64::from(s.x), s.y, s.z);
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
            _ => 0.0,
        };
        if depth.is_some() {
            sources.push((s, f));
        }

        // ---- upkeep everywhere, death where dry or too deep
        let mut respired = c.film_maintenance_per_s * dt * f;
        let mut died = if depth.is_none() {
            c.film_dry_death_per_s * dt * f
        } else {
            0.0
        };
        if respired + died > f {
            respired *= f / (respired + died);
            died = f - respired;
        }
        let lost = respired + died;
        if lost > 0.0 {
            // Pro rata out of the stock, then split between the two sinks.
            let all = lost >= f;
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
            if to == from || !eligible(&view, c, i64::from(to.x), to.y, to.z) {
                continue;
            }
            let there = before
                .binary_search_by_key(&to, |e| e.0)
                .map_or(0.0, |i| before[i].1);
            if there < f {
                let amount = (rate * f).min(0.5 * (f - there));
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
