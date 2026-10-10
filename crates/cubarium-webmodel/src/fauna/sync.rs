//! WMSYNC integration kept outside the existing pack provisioning and hunter levers.
use super::{behavior, laws, model::FaunaModel};
use crate::exchange::Returns;
use cubarium_voxel_fauna::{EggSite, Founder};

impl FaunaModel {
    pub(super) fn behavior_response(&mut self, dt: f64, awake: &[Vec<f64>]) {
        let Some(env) = self.habitat.clone() else {
            return;
        };
        let area = env.area.max(f64::MIN_POSITIVE);
        let day = self.daylight_at(self.t_s + dt);
        for f in Founder::ALL {
            if !self.enabled[f.index()] {
                continue;
            }
            let p = *self.phys(f);
            let peers = self.census(f).n + self.extra_bodies[f.index()];
            let mut swept = 0.0;
            let mut threats = 0.0;
            for hf in Founder::ALL {
                if !self.enabled[hf.index()] {
                    continue;
                }
                let hp = self.phys(hf);
                if !hp.active_hunter().is_some_and(|h| h.hunts(f)) {
                    continue;
                }
                for (k, c) in self.pops[hf.index()].iter().enumerate() {
                    if c.host.is_some() || self.is_pup(hf, c) || !c.b.eating {
                        continue;
                    }
                    let n = c.n * awake[hf.index()].get(k).copied().unwrap_or(0.0);
                    threats += n;
                    swept += n * hp.cruise_body_lengths_per_s * hp.body_at(c.b.body).length_m;
                }
            }
            for (k, c) in self.pops[f.index()].iter_mut().enumerate() {
                let a = awake[f.index()].get(k).copied().unwrap_or(0.0);
                behavior::update_response(
                    &p,
                    &c.b,
                    &mut c.response,
                    threats / area,
                    if threats > 0.0 { swept / threats } else { 0.0 },
                    dt,
                );
                let l = &mut self.ledger[f.index()];
                l.hunker_s += c.n * a * c.response.hunker * dt;
                l.flight_s += c.n * a * c.response.flight * dt;
                if f != Founder::RippleSnail || a <= 0.0 || !c.b.eating {
                    continue;
                }
                let Some(am) = p.amphibious else { continue };
                let fill = c.film_patch.get_or_insert(env.film_rich);
                if *fill <= f64::EPSILON && c.film_travel_s == 0.0 {
                    l.refuge_s += c.n * a * dt;
                }
                let total_fill = *fill + env.film_refuge;
                let perceived = if self.edible_perception {
                    *fill
                } else {
                    total_fill
                };
                let density = env.film_faces / area;
                let reachable = std::f64::consts::PI * am.graze_search_m.powi(2) * density;
                let neighbors = 1.0 + peers / env.film_faces.max(1.0);
                let departure =
                    behavior::film_departure(&am, perceived, env.film_rich, reachable, neighbors);
                if c.film_travel_s > 0.0 {
                    c.film_travel_s = (c.film_travel_s - dt).max(0.0);
                    if c.film_travel_s == 0.0 {
                        *fill = env.film_rich;
                    }
                } else if departure > 0.0 {
                    let speed = p.cruise_body_lengths_per_s * p.body_at(c.b.body).length_m;
                    // Mean nearest richer face in a planar Poisson field, capped at search reach.
                    let distance =
                        (0.5 / density.max(f64::MIN_POSITIVE).sqrt()).min(am.graze_search_m);
                    c.film_travel_s = distance / speed.max(f64::MIN_POSITIVE) / departure;
                } else {
                    *fill = (*fill
                        + env.film_growth * day * total_fill * (1.0 - total_fill).max(0.0) * dt)
                        .min(env.film_rich.max(*fill));
                }
            }
        }
    }

    pub(super) fn hunt_synced(
        &mut self,
        dt: f64,
        awake: &[Vec<f64>],
        areas: &[(f64, f64); Founder::COUNT],
        ret: &mut Returns,
    ) {
        let Some(env) = self.habitat.clone() else {
            self.hunt(dt, awake, areas, ret);
            return;
        };
        let old: Vec<_> = self.config.founders.iter().map(|p| p.hunter).collect();
        for p in &mut self.config.founders {
            if let Some(h) = &mut p.hunter {
                h.prey[Founder::Browser.index()] = false;
                h.prey[Founder::Bellwing.index()] = false;
            }
        }
        self.hunt(dt, awake, areas, ret);
        for (p, h) in self.config.founders.iter_mut().zip(old) {
            p.hunter = h;
        }
        let day = self.daylight_at(self.t_s + dt);
        let area = env.area.max(f64::MIN_POSITIVE);
        let bw = Founder::Bellwing;
        let bp = *self.phys(bw);
        let bw_n = self.census(bw).n;
        let bw_awake: f64 = self.pops[bw.index()]
            .iter()
            .enumerate()
            .map(|(k, c)| c.n * awake[bw.index()].get(k).copied().unwrap_or(0.0))
            .sum();
        for hf in Founder::ALL {
            if !self.enabled[hf.index()] {
                continue;
            }
            let hp = *self.phys(hf);
            let Some(h) = hp.active_hunter() else { continue };
            let rule = self.hunt_rule(hf);
            let members = if hp.pack.is_some() {
                self.pops[hf.index()]
                    .iter()
                    .filter(|c| !self.is_pup(hf, c) && c.host.is_none())
                    .map(|c| c.n)
                    .sum::<f64>()
                    / self.packs[hf.index()].len().max(1) as f64
            } else {
                1.0
            };
            let hunters: Vec<_> = self.pops[hf.index()]
                .iter()
                .enumerate()
                .filter(|(_, c)| c.host.is_none() && c.b.eating && !self.is_pup(hf, c))
                .map(|(k, c)| {
                    (
                        k,
                        c.n * awake[hf.index()].get(k).copied().unwrap_or(0.0),
                        c.b.body,
                    )
                })
                .collect();
            for (k, hn, hb) in hunters {
                if hn <= 0.0 {
                    continue;
                }
                let lit = self.lure_lit_override.unwrap_or_else(|| {
                    if env.sky * day < self.config.life.day_line {
                        1.0
                    } else {
                        0.0
                    }
                });
                let lit = if h.lure_power > 0.0 { lit * hn } else { 0.0 };
                self.ledger[hf.index()].lure_lit_s += lit * dt;
                // founder_act pays the lit fraction times lure power and basal upkeep.
                let hc = &mut self.pops[hf.index()][k];
                let cost = h.lure_power * hp.core.maintenance_per_s * hb * lit * dt
                    / hc.n.max(f64::MIN_POSITIVE);
                let (paid, _) = laws::respire(&hp.core, &mut hc.b, cost);
                self.ledger[hf.index()].flows.motor += paid * hc.n;
                let visits = behavior::lure_visits_per_s(
                    &self.config.light,
                    &bp,
                    bp.core.body_max,
                    bw_awake,
                    lit,
                    env.flowers,
                    env.glowcaps,
                    day,
                    area,
                );
                self.ledger[hf.index()].lure_visits += visits * dt;
                let mut kills = 0.0;
                let mut corpses = 0.0;
                let mut meals = 0.0;
                for pf in [Founder::Browser, bw] {
                    if !h.hunts(pf) || !self.enabled[pf.index()] {
                        continue;
                    }
                    let pp = *self.phys(pf);
                    let kin_density = self.census(pf).n / area;
                    for (j, c) in self.pops[pf.index()].iter_mut().enumerate() {
                        if c.host.is_some()
                            || c.n <= 0.0
                            || !laws::in_size_window(&hp, hb * members.max(1.0), &pp, c.b.body)
                        {
                            continue;
                        }
                        let base = laws::strike_share(&hp, hb, &pp, c.b.body);
                        let mut lure_hazard = 0.0;
                        let hazard = if pf == bw {
                            let prey_awake = awake[pf.index()].get(j).copied().unwrap_or(0.0);
                            let reach = h.strike_reach_length_fraction * hp.body_at(hb).length_m;
                            let speed = hp.cruise_body_lengths_per_s * hp.body_at(hb).length_m;
                            // Ground encounters can catch resting flyers; a lure brings awake
                            // flyers into the jaw band. Flight height barriers are a remaining gap.
                            let resting = (1.0 - prey_awake).clamp(0.0, 1.0);
                            let ground = 2.0 * reach * speed / area * hn * resting * base * base;
                            lure_hazard = visits / bw_n.max(f64::MIN_POSITIVE) * base * base;
                            ground + lure_hazard
                        } else {
                            let condition =
                                c.b.reserve / pp.core.reserve_of(c.b.body).max(f64::MIN_POSITIVE);
                            // Conditional on a close attack, a fed hunkerer enters its guard.
                            let guarded =
                                if pp.hunker.enabled && condition >= pp.hunker.min_condition {
                                    1.0
                                } else {
                                    c.response.hunker
                                };
                            let kin = kin_density
                                * std::f64::consts::PI
                                * pp.hunker.ring_m.powi(2)
                                * c.response.hunker;
                            let opened = behavior::pry_open_share(&pp, c.b.body, members, kin);
                            let hit = behavior::guarded_kill_share(
                                &hp, hb, &pp, c.b.body, guarded, opened,
                            );
                            let hs = hp.cruise_body_lengths_per_s * hp.body_at(hb).length_m;
                            let ps = pp.cruise_body_lengths_per_s * pp.body_at(c.b.body).length_m;
                            let escape = if guarded < 1.0 {
                                hs / (hs + ps).max(f64::MIN_POSITIVE)
                            } else {
                                1.0
                            };
                            rule.kill[pf.index()] / areas[pf.index()].0.max(f64::MIN_POSITIVE)
                                * hn
                                * hit
                                * escape
                                * escape
                        };
                        let dead = self.draws.of(c.n, 1.0 - (-hazard * dt).exp()).min(c.n);
                        if dead <= 0.0 {
                            continue;
                        }
                        c.n -= dead;
                        let esc = c.escrow.map_or(0.0, |(x, _)| x);
                        let lost = esc * pp.core.reproduction.gestation_loss_fraction;
                        self.ledger[pf.index()].gestation_loss += dead * lost;
                        let corpse = c.b.organic() + esc - lost;
                        let meal = (rule.reach.clamp(0.0, 1.0) * corpse).min(rule.meal_cap);
                        corpses += dead * corpse;
                        meals += dead * meal;
                        kills += dead;
                        let l = &mut self.ledger[pf.index()];
                        l.killed_by[hf.index()] += dead;
                        if hf == Founder::Chorister {
                            if c.b.body < pp.core.birth_body {
                                l.kills_juvenile += dead;
                            } else {
                                l.kills_adult += dead;
                            }
                        }
                        if pf == bw && hazard > 0.0 {
                            self.ledger[hf.index()].lure_kills += dead * lure_hazard / hazard;
                        }
                    }
                }
                self.pops[hf.index()][k].carcass += meals;
                self.carcass_laid += meals;
                self.carcass_out += corpses - meals;
                ret.carrion += corpses - meals;
                let strikes = kills * rule.strikes_per_kill;
                let hc = &mut self.pops[hf.index()][k];
                let cost = strikes * h.strike_cost_s * laws::upkeep_per_s(&hp, hb, 1.0)
                    / hc.n.max(f64::MIN_POSITIVE);
                let (paid, _) = laws::respire(&hp.core, &mut hc.b, cost);
                let l = &mut self.ledger[hf.index()];
                l.kills += kills;
                l.strikes += strikes;
                l.strike_cost += paid * hc.n;
                l.flows.motor += paid * hc.n;
            }
        }
    }

    pub(super) fn reproduce_synced(
        &mut self,
        dt: f64,
        dusk: bool,
        awake: &[Vec<f64>],
        ret: &mut Returns,
    ) {
        let Some(env) = self.habitat.clone() else {
            self.reproduce(dt, dusk, ret);
            return;
        };
        let mut kept = Vec::with_capacity(self.eggs.len());
        for mut e in std::mem::take(&mut self.eggs) {
            let p = *self.phys(e.lineage);
            if !e.dormant && p.egg_site == EggSite::Shore {
                if let (Some(am), Some(site)) = (p.amphibious, env.shore.first()) {
                    if let Some(drowned) = behavior::egg_loss(
                        &am,
                        &mut e.wet_s,
                        &mut e.dry_s,
                        site.depth > 0.0,
                        site.moisture,
                        dt,
                    ) {
                        ret.litter += e.n * e.organic;
                        let l = &mut self.ledger[e.lineage.index()];
                        if drowned {
                            l.eggs_drowned += e.n;
                        } else {
                            l.eggs_dried += e.n;
                        }
                        continue;
                    }
                }
            }
            kept.push(e);
        }
        self.eggs = kept;
        let snail = Founder::RippleSnail;
        let phys = *self.phys(snail);
        let old_site = self.fits.of(snail).egg_site;
        if phys.egg_site == EggSite::Shore {
            if let Some(am) = phys.amphibious {
                let sites: f64 = env.shore.iter().map(|s| s.faces).sum();
                let density = sites / env.area.max(f64::MIN_POSITIVE);
                self.fits.of_mut(snail).egg_site = old_site
                    * (1.0 - (-std::f64::consts::PI * am.egg_search_m.powi(2) * density).exp());
                for (k, c) in self.pops[snail.index()].iter_mut().enumerate() {
                    if awake[snail.index()].get(k).copied().unwrap_or(0.0) <= 0.0 {
                        c.retry_s = c.retry_s.max(2.0 * dt);
                    }
                }
            }
        }
        self.reproduce(dt, dusk, ret);
        self.fits.of_mut(snail).egg_site = old_site;
    }
}
