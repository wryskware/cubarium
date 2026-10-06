//! **The lanternjaw's patrol** (the detritus web D2,
//! `design/lanternjaw-detritus-web-proposal-2026-10-05.md` §3.2 with Astra's corrections,
//! `design/7_Research/astra-detritus-web-review-2026-10-05.md`): a mobile sit-and-wait
//! hunter of small ground prey, like a scorpion or a centipede. It replaces the P1 ambush's
//! crown seeking while the world's [`HuntConfig::patrol`] is on, over the lanternjaw's
//! heuristic only ([`crate::LanternjawPatrol`]), as the pack tactic does over the
//! chorister's. Its numbers are [`HuntConfig`]'s, every one a plausible start, untuned.
//!
//! Each controller sample of a hungry lanternjaw (eating latch on):
//!
//! 1. **Senses.** Every size-eligible prey ([`crate::is_prey_of`]: its list, its window,
//!    a seedporter only grounded, a bellwing only at rest) it **sees** — within
//!    [`HuntConfig::sight_m`], ahead of it and not hidden (the flee rule's ray) — or
//!    **feels**: its tremor, `mass × speed / d²` within the tremor radius (predation §2.2),
//!    split left and right. Each lineage met is an **encounter**: it adds the sample's
//!    seconds to the **search image** ([`HuntMemory::image`]), which decays with
//!    [`HuntConfig::search_memory_s`].
//! 2. **Attack preference.** Whether it takes on a prey it meets is drawn once per
//!    [`HuntConfig::attack_decision_s`] per prey ([`accepts`]), at its [`profitability`] (the
//!    share of a kill one strike makes) times [`attack_preference`]. By default it is
//!    opportunistic: the preference is 1 for every listed prey (`attack_explore` 1, share
//!    switching off), so profitability alone decides. With D2b's share switching on, the
//!    preference is the lineage's **share** of its recent eligible encounters
//!    relative to the most-met lineage's, raised to [`HuntConfig::switch_exponent`]. A
//!    lineage that turns rare among the prey it meets is passed over even when met — the
//!    falling per-prey risk at low density Astra asked for — and the image follows what it
//!    meets, not what it caught, so a lineage that recovers is taken up again as it is met.
//!    With [`HuntConfig::strike_chosen_only`] its strike capsule asks the same draw, so a
//!    passed-over prey standing in reach is not struck either.
//! 3. **Engage.** The nearest accepted prey it sees: face it, creep at the teachers'
//!    stalk (it stays a still shape) until it is in reach, feed held — the body turns that
//!    into a strike when the strike capsule holds prey. Failing that, the louder side of
//!    accepted prey's tremor at least [`HuntConfig::tremor_turn`]: turn toward it, feed
//!    held.
//! 4. **Patrol.** Otherwise it waits still on its patch with the lure lit for
//!    [`HuntConfig::patch_wait_s`]; a fruitless wait (or a walk given up) remembers the
//!    patch as fruitless for [`HuntConfig::fruitless_memory_s`], and it chooses the next:
//!    the food of its prey within [`HuntConfig::cue_scan_m`] pooled into
//!    [`HuntConfig::patch_m`] squares, each lineage's cue scaled to its best square and
//!    weighted by its attack preference, the best open square at least
//!    [`HuntConfig::patch_min_hop_m`] away — or, a [`HuntConfig::patrol_explore`] share
//!    of the time, any open square. With no food in reach it walks off in a drawn
//!    direction.
//!
//! A fed lanternjaw (latch off) stands still, lure dark. Eating carrion at its mouth and
//! yielding a carcass stay the ambush's and the carcass yield's.
//!
//! **HUNTER-MEAL** (Astra's open questions §3, 2026-10-05): with [`HuntConfig::meal_first`]
//! a hungry hunter first eats a usable carcass (carrion under a carcass record) at its mouth
//! or walks to one within [`HuntConfig::carcass_scan_m`] ([`meal_step`], the Veilhound's
//! pack tactic too between hunts), and does not strike while its mouth is over one. With
//! [`HuntConfig::reachable_only`] patches and carcasses count only on the hunter's walking
//! route graph (the Veilhound's chase graph, [`crate::encounter::RouteGraph`]) within the
//! route steps it walks before giving up, and it walks there along the graph. A kill marks
//! its place productive ([`HuntMemory::productive`]), which the patch choice favours.

use std::sync::Arc;

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::{FloraView, Site, Trophic};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::encounter::RouteGraph;
use crate::{Actions, Fauna, Food, Founder, HuntConfig};

/// How many fruitless patches one hunter remembers at once (a cache size, not ecology).
pub const FRUITLESS_SLOTS: usize = 6;

/// What a patroller is doing ([`HuntMemory::mode`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HuntMode {
    /// No patch: the next sample chooses one.
    #[default]
    Idle,
    /// Walking to [`HuntMemory::patch`].
    Travel,
    /// Waiting still on it, the lure lit.
    Wait,
    /// Going to a usable carcass, or eating at one (HUNTER-MEAL's meal step).
    Meal,
}

/// One remembered fruitless patch: its centre, metres, and the tick it is forgotten at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FruitlessPatch {
    pub x: f64,
    pub z: f64,
    pub until_tick: u64,
}

/// A patroller's memory ([`crate::Animal::hunt`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HuntMemory {
    pub mode: HuntMode,
    /// The patch centre it walks to or waits on, metres `(x, z)`.
    pub patch: (f64, f64),
    /// The face it walks to on its route graph (`(x, y, z)`), when the patch was chosen
    /// on it ([`HuntConfig::reachable_only`]).
    pub patch_face: Option<(u32, u32, u32)>,
    /// Controller samples in the current mode.
    pub samples: u32,
    /// Samples of an escape from a refused motor left.
    pub escape: u32,
    pub fruitless: [FruitlessPatch; FRUITLESS_SLOTS],
    /// **Productive places** (HUNTER-MEAL): where its recent kills were, each until the tick
    /// it is forgotten ([`HuntConfig::success_memory_s`]).
    pub productive: [FruitlessPatch; FRUITLESS_SLOTS],
    /// **The search image**: seconds of contact with size-eligible prey per lineage, decayed
    /// with [`HuntConfig::search_memory_s`].
    pub image: [f64; Founder::COUNT],
    /// The tick the image was last decayed to.
    pub image_tick: u64,
}

impl HuntMemory {
    /// Decay the image to `tick` (e-fold [`HuntConfig::search_memory_s`]), then add
    /// `dt_s` of contact to every lineage `met`.
    pub fn meet(&mut self, cfg: &HuntConfig, tick: u64, met: &[bool; Founder::COUNT], dt_s: f64) {
        let elapsed = tick.saturating_sub(self.image_tick) as f64 * crate::DT;
        if elapsed > 0.0 && cfg.search_memory_s > 0.0 {
            let k = (-elapsed / cfg.search_memory_s).exp();
            for v in &mut self.image {
                *v *= k;
            }
        }
        self.image_tick = self.image_tick.max(tick);
        if dt_s > 0.0 && dt_s.is_finite() {
            for (v, &m) in self.image.iter_mut().zip(met) {
                if m {
                    *v += dt_s;
                }
            }
        }
    }

    /// Remember a fruitless patch at `(x, z)` until `until_tick`, over the slot that is
    /// forgotten soonest.
    pub fn remember_fruitless(&mut self, x: f64, z: f64, until_tick: u64) {
        let slot = self
            .fruitless
            .iter_mut()
            .min_by_key(|p| p.until_tick)
            .expect("at least one slot");
        *slot = FruitlessPatch { x, z, until_tick };
    }

    /// Whether `(x, z)` is within [`HuntConfig::fruitless_radius_m`] of a fruitless patch
    /// still remembered at `tick`.
    pub fn is_fruitless(&self, cfg: &HuntConfig, x: f64, z: f64, tick: u64, ring: f64) -> bool {
        near_any(&self.fruitless, cfg.fruitless_radius_m, x, z, tick, ring)
    }

    /// Remember a kill at `(x, z)` as a productive place until `until_tick`, over the slot
    /// forgotten soonest, and forget any fruitless patch there.
    pub fn remember_productive(&mut self, cfg: &HuntConfig, x: f64, z: f64, tick: u64, until_tick: u64, ring: f64) {
        let slot = self
            .productive
            .iter_mut()
            .min_by_key(|p| p.until_tick)
            .expect("at least one slot");
        *slot = FruitlessPatch { x, z, until_tick };
        for p in &mut self.fruitless {
            if near_any(std::slice::from_ref(p), cfg.fruitless_radius_m, x, z, tick, ring) {
                p.until_tick = 0;
            }
        }
    }

    /// Whether `(x, z)` is within [`HuntConfig::fruitless_radius_m`] of a productive place
    /// still remembered at `tick`.
    pub fn is_productive(&self, cfg: &HuntConfig, x: f64, z: f64, tick: u64, ring: f64) -> bool {
        near_any(&self.productive, cfg.fruitless_radius_m, x, z, tick, ring)
    }
}

/// Whether `(x, z)` is within `radius` of a place in `places` still remembered at `tick`.
fn near_any(places: &[FruitlessPatch], radius: f64, x: f64, z: f64, tick: u64, ring: f64) -> bool {
    places.iter().any(|p| {
        p.until_tick > tick && {
            let (dx, dz) = crate::pose::wrapped_delta(p.x, p.z, x, z, ring);
            dx * dx + dz * dz <= radius * radius
        }
    })
}

/// **The encounter share** of `prey` (share switching): its recent contact plus the naive
/// expectation [`HuntConfig::attack_prior_s`], over the same summed across every lineage on
/// the hunter's list, `(m_k + m₀) / Σ_j (m_j + m₀)`. 0 for a lineage not on its list.
pub fn encounter_share(cfg: &HuntConfig, h: &crate::Hunter, image: &[f64; Founder::COUNT], prey: Founder) -> f64 {
    if !h.hunts(prey) {
        return 0.0;
    }
    let prior = if cfg.attack_prior_s.is_finite() { cfg.attack_prior_s.max(0.0) } else { 0.0 };
    let weight = |f: Founder| {
        let m = image[f.index()];
        (if m.is_finite() { m.max(0.0) } else { 0.0 }) + prior
    };
    let listed = Founder::ALL.iter().copied().filter(|&f| h.hunts(f));
    let total: f64 = listed.clone().map(weight).sum();
    if total > 0.0 {
        weight(prey) / total
    } else {
        1.0 / listed.count().max(1) as f64
    }
}

/// **The attack preference** for `prey` given the search image `image`, in `[0, 1]`.
///
/// Share switching on (D2b): `(s_k / max_j s_j)^γ` with `s` the [`encounter_share`]s over
/// the hunter's list and `γ` [`HuntConfig::switch_exponent`]. The lineage it meets most is
/// attacked at its full profitability; one met half as often, at `2^−γ` of it; a lineage
/// whose share falls loses preference faster than its share for `γ > 1`. With nothing met
/// lately (all at the prior, or no prior and no contact) every listed lineage is at 1.
///
/// Off (D2): `explore + (1 − explore) · m / (m + half)` on the lineage's absolute contact
/// `m`, from the floor [`HuntConfig::attack_explore`] toward 1, halfway at
/// [`HuntConfig::attack_half_s`].
pub fn attack_preference(cfg: &HuntConfig, h: &crate::Hunter, image: &[f64; Founder::COUNT], prey: Founder) -> f64 {
    if !cfg.share_switching {
        let explore = cfg.attack_explore.clamp(0.0, 1.0);
        let m = image[prey.index()];
        let m = if m.is_finite() { m.max(0.0) } else { 0.0 };
        let half = cfg.attack_half_s.max(1e-9);
        return explore + (1.0 - explore) * m / (m + half);
    }
    let share = encounter_share(cfg, h, image, prey);
    let top = Founder::ALL
        .iter()
        .map(|&f| encounter_share(cfg, h, image, f))
        .fold(0.0f64, f64::max);
    if !(top > 0.0) {
        return 0.0;
    }
    let gamma = if cfg.switch_exponent.is_finite() { cfg.switch_exponent.max(0.0) } else { 1.0 };
    (share / top).clamp(0.0, 1.0).powf(gamma)
}

/// **The attack chance** of hunter `a` on prey `o` this decision window:
/// [`profitability`] × [`attack_preference`] from `a`'s search image. 0 for a body that is
/// not a hunter or not a founder.
pub fn attack_chance(config: &crate::FaunaConfig, a: &crate::Animal, o: &crate::Animal) -> f64 {
    let (Some(hf), Some(pf)) = (a.founder, o.founder) else {
        return 0.0;
    };
    let Some(h) = config.founder(hf).hunter else {
        return 0.0;
    };
    profitability(&h, a.body, o.body, config.founder(pf).armour) * attack_preference(&config.hunt, &h, &a.hunt.image, pf)
}

/// **Whether hunter `a` takes on prey `o`** at `tick`: one draw per pair per
/// [`HuntConfig::attack_decision_s`] window against [`attack_chance`]. The patrol's choice
/// and, with [`HuntConfig::strike_chosen_only`], its strike capsule both ask this, so they
/// agree within a window.
pub fn accepts(config: &crate::FaunaConfig, a: &crate::Animal, o: &crate::Animal, tick: u64) -> bool {
    let window = u64::try_from(cubarium_voxel::ticks_at_least_one(config.hunt.attack_decision_s)).unwrap_or(u64::MAX).max(1);
    draw(DOMAIN_ATTACK, a.id, o.id, tick / window) < attack_chance(config, a, o)
}

/// **Profitability**: the share of a kill one strike makes on a prey of `prey_body` with
/// `armour`, `min(1, power × (S_pred / S_prey) × (1 − armour))` — the wound formula
/// (predation §2.4). A prey it kills in one strike is worth 1; an adult grazer that takes
/// three hits about a third. It scales the attack preference, so the small prey are its
/// living and the big ones a chance (proposal §3.2: adult grazers "are not its living").
pub fn profitability(h: &crate::Hunter, hunter_body: f64, prey_body: f64, armour: f64) -> f64 {
    if !(prey_body > 0.0) {
        return 1.0;
    }
    let hit = h.strike_power * (hunter_body / prey_body) * (1.0 - armour);
    if hit.is_finite() { hit.clamp(0.0, 1.0) } else { 1.0 }
}

/// A uniform draw in `[0, 1)` from three keys and a domain (SplitMix64).
fn draw(domain: u64, a: u64, b: u64, c: u64) -> f64 {
    fn mix(mut z: u64) -> u64 {
        z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    let h = mix(mix(mix(domain ^ a) ^ b) ^ c);
    (h >> 11) as f64 / (1u64 << 53) as f64
}

const DOMAIN_ATTACK: u64 = 0xA77AC;
const DOMAIN_EXPLORE: u64 = 0xE4910;
const DOMAIN_ESCAPE: u64 = 0xE5CA9;

/// Steer toward a heading: turn in place while it is more than 60° off, else go at
/// `forward` turning toward it (full turn beyond 45°).
pub(crate) fn head(heading: f64, want: f64, forward: f64, feed: f64) -> Actions {
    use std::f64::consts::{FRAC_PI_3, FRAC_PI_4, PI, TAU};
    let d = (want - heading).rem_euclid(TAU);
    let diff = if d > PI { d - TAU } else { d };
    Actions {
        forward: if diff.abs() > FRAC_PI_3 { 0.0 } else { forward },
        turn: (diff / FRAC_PI_4).clamp(-1.0, 1.0),
        feed,
    }
}

/// How strongly one ground entry or stand offers `food`, `[0, 1]`: the detritus field's
/// emission curve for litter and carrion, the film's against its capacity, and presence
/// for stand tissue, nectar and fruit.
fn ground_cue(fv: &FloraView<'_>, fauna: &Fauna, g: &cubarium_voxel_flora::Ground, food: Food, cell_m2: f64) -> f64 {
    let reference = fauna.config.cue.emit_reference;
    let sat = |x: f64, r: f64| if r > 0.0 && x > 0.0 { (x / r).min(1.0) } else { 0.0 };
    match food {
        Food::Litter => sat(g.litter, reference),
        Food::Carrion => sat(g.carrion, reference),
        Food::Film => sat(g.film, fv.config.film_capacity_per_m2 * cell_m2),
        _ => 0.0,
    }
}

fn stand_cue(fv: &FloraView<'_>, st: &cubarium_voxel_flora::Stand, food: Food) -> f64 {
    let trophic = fv.config.species(st.species).trophic;
    let on = match food {
        Food::CapTissue => trophic == Trophic::Saprotroph && st.foliage > 0.0,
        Food::Foliage => trophic != Trophic::Saprotroph && st.foliage > 0.0,
        Food::Nectar => st.nectar > 0.0,
        Food::Fruit => st.fruit > 0.0,
        _ => false,
    };
    if on { 1.0 } else { 0.0 }
}

/// **Choose the next patch** for the hunter at `i`: the centre of the best open square
/// of its prey's food within the scan, or (exploring, or with none) a drawn one. Books the
/// choice.
///
/// HUNTER-MEAL: with [`HuntConfig::reachable_only`] and the hunter on its route graph, a
/// square counts only when one of its food faces can be taken from a node on its route
/// field (the walk it makes before giving up), and the patch it walks to is that node (the
/// nearest by route); with no food in reach, a drawn node of the field. A square by a
/// remembered kill ([`HuntMemory::is_productive`]) scores `1 + success_boost` times its
/// cue and is not held fruitless. The face walked to goes to [`HuntMemory::patch_face`].
pub(crate) fn choose_patch(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>, h: &crate::Hunter) -> (f64, f64) {
    let cfg = fauna.config.hunt;
    let tick = fauna.tick;
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let pm = cfg.patch_m.max(v);
    let scan = cfg.cue_scan_m.max(0.0);
    let route = route_from(fauna, i, view);
    if cfg.reachable_only && route.is_none() {
        fauna.ledger.hunt_off_route += 1;
    }
    let a = fauna.animals[i];
    // Which foods mark which prey lineage's habitat.
    let lineages: Vec<Founder> = Founder::ALL.iter().copied().filter(|&f| h.hunts(f)).collect();
    let weight: Vec<f64> = lineages.iter().map(|&f| attack_preference(&cfg, h, &a.hunt.image, f)).collect();
    /// One square: the cue per lineage, and its nearest reachable food node `(steps, node)`.
    struct Square {
        cues: Vec<f64>,
        node: Option<(u32, usize)>,
    }
    // Squares keyed by their absolute index.
    let mut squares: FxHashMap<(i64, i64), Square> = FxHashMap::default();
    let add = |site: Site, cues: &[f64], squares: &mut FxHashMap<(i64, i64), Square>| {
        if !cues.iter().any(|&c| c > 0.0) {
            return;
        }
        let (x, z) = ((f64::from(site.x) + 0.5) * v, (f64::from(site.z) + 0.5) * v);
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, x, z, ring);
        if dx * dx + dz * dz > scan * scan {
            return;
        }
        let reach = route.as_ref().and_then(|r| r.reach(site, v).map(|(j, d)| (d, j)));
        let key = ((x.rem_euclid(ring) / pm).floor() as i64, (z / pm).floor() as i64);
        let sq = squares.entry(key).or_insert_with(|| Square { cues: vec![0.0; lineages.len()], node: None });
        for (c, &cue) in sq.cues.iter_mut().zip(cues) {
            *c += cue;
        }
        if let Some(n) = reach {
            if sq.node.is_none_or(|b| n < b) {
                sq.node = Some(n);
            }
        }
    };
    let cell_m2 = v * v;
    let mut cues = vec![0.0; lineages.len()];
    for g in fv.ground.iter() {
        if g.litter <= 0.0 && g.carrion <= 0.0 && g.film <= 0.0 {
            continue;
        }
        for (k, f) in lineages.iter().enumerate() {
            let diet = fauna.config.founder(*f).diet;
            cues[k] = diet.foods().map(|food| ground_cue(fv, fauna, g, food, cell_m2)).sum();
        }
        add(g.site, &cues, &mut squares);
    }
    for st in fv.stands.iter() {
        for (k, f) in lineages.iter().enumerate() {
            let diet = fauna.config.founder(*f).diet;
            cues[k] = diet.foods().map(|food| stand_cue(fv, st, food)).sum();
        }
        add(st.site, &cues, &mut squares);
    }
    // Each lineage's cue against its own best square, weighted by its preference.
    let mut best = vec![0.0f64; lineages.len()];
    for sq in squares.values() {
        for (b, &c) in best.iter_mut().zip(&sq.cues) {
            *b = b.max(c);
        }
    }
    let centre = |(kx, kz): (i64, i64)| (((kx as f64) + 0.5) * pm, ((kz as f64) + 0.5) * pm);
    // (square centre, score, distance, the node walked to)
    type Entry = ((f64, f64), f64, f64, Option<usize>);
    let mut open: Vec<Entry> = Vec::new();
    let mut fruitless: Vec<Entry> = Vec::new();
    let mut unreachable = 0u64;
    for (&key, sq) in &squares {
        let (x, z) = centre(key);
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, x, z, ring);
        let d = (dx * dx + dz * dz).sqrt();
        if d < cfg.patch_min_hop_m {
            continue;
        }
        let mut score: f64 = sq
            .cues
            .iter()
            .zip(&best)
            .zip(&weight)
            .map(|((&c, &b), &w)| if b > 0.0 { w * c / b } else { 0.0 })
            .sum();
        if !(score > 0.0) {
            continue;
        }
        if route.is_some() && sq.node.is_none() {
            unreachable += 1;
            continue;
        }
        let productive = a.hunt.is_productive(&cfg, x, z, tick, ring);
        if productive {
            score *= 1.0 + cfg.success_boost.max(0.0);
        }
        let entry = ((x, z), score, d, sq.node.map(|n| n.1));
        if !productive && a.hunt.is_fruitless(&cfg, x, z, tick, ring) {
            fruitless.push(entry);
        } else {
            open.push(entry);
        }
    }
    fauna.ledger.hunt_unreachable_squares += unreachable;
    // Every square remembered fruitless: the memory gives way rather than strand it.
    let pool = if open.is_empty() { &mut fruitless } else { &mut open };
    // A stable order, so a draw picks the same square for the same world.
    pool.sort_by(|p, q| p.0.0.total_cmp(&q.0.0).then(p.0.1.total_cmp(&q.0.1)));
    let explore = draw(DOMAIN_EXPLORE, a.id, tick, 0) < cfg.patrol_explore;
    let pick = if pool.is_empty() {
        None
    } else if explore {
        let k = ((draw(DOMAIN_EXPLORE, a.id, tick, 1) * pool.len() as f64) as usize).min(pool.len() - 1);
        Some(pool[k])
    } else {
        pool.iter().copied().max_by(|p, q| p.1.total_cmp(&q.1).then(q.2.total_cmp(&p.2)))
    };
    let face_of = |r: &Route, j: usize| {
        let f = r.graph.faces[j];
        (f.x, f.y, f.z)
    };
    match pick {
        Some(p) => {
            if explore {
                fauna.ledger.hunt_explored += 1;
            } else {
                fauna.ledger.hunt_chosen += 1;
            }
            match (p.3, &route) {
                (Some(j), Some(r)) => {
                    fauna.animals[i].hunt.patch_face = Some(face_of(r, j));
                    r.graph.centre(j)
                }
                _ => {
                    fauna.animals[i].hunt.patch_face = None;
                    (p.0.0.rem_euclid(ring), p.0.1)
                }
            }
        }
        None => {
            // No food in reach: walk off in a drawn direction — on its route, a drawn node of
            // the field at least the minimum hop away.
            fauna.ledger.hunt_explored += 1;
            if let Some(r) = &route {
                let mut far: Vec<usize> = r
                    .field
                    .keys()
                    .copied()
                    .filter(|&j| {
                        let (x, z) = r.graph.centre(j);
                        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, x, z, ring);
                        let d = (dx * dx + dz * dz).sqrt();
                        d >= cfg.patch_min_hop_m && d <= scan.max(cfg.patch_min_hop_m)
                    })
                    .collect();
                if !far.is_empty() {
                    far.sort_unstable();
                    let k = ((draw(DOMAIN_EXPLORE, a.id, tick, 2) * far.len() as f64) as usize).min(far.len() - 1);
                    fauna.animals[i].hunt.patch_face = Some(face_of(r, far[k]));
                    return r.graph.centre(far[k]);
                }
            }
            fauna.animals[i].hunt.patch_face = None;
            let angle = draw(DOMAIN_EXPLORE, a.id, tick, 2) * std::f64::consts::TAU;
            let r = 0.5 * scan.max(cfg.patch_min_hop_m);
            let depth_m = f64::from(view.config.depth) * v;
            (
                (a.pose.x + r * angle.sin()).rem_euclid(ring),
                (a.pose.z + r * angle.cos()).clamp(0.5 * v, (depth_m - 0.5 * v).max(0.5 * v)),
            )
        }
    }
}

// ------------------------------------------------------------------ routes (HUNTER-MEAL)

/// A hunter's place on its lineage's walking route graph ([`HuntConfig::reachable_only`]):
/// the graph, its node, the route field from that node out to the steps it walks before it
/// gives a walk up, and how near a node must be to a food face for its mouth to take it.
pub(crate) struct Route {
    pub graph: Arc<RouteGraph>,
    pub from: usize,
    pub field: FxHashMap<usize, u32>,
    pub steps: u32,
    pub food_r: u32,
}

impl Route {
    /// The node a body takes `site` from (within its mouth's reach of the face) if it is
    /// on the field, and its route distance.
    pub fn reach(&self, site: Site, v: f64) -> Option<(usize, u32)> {
        let centre = ((f64::from(site.x) + 0.5) * v, (f64::from(site.z) + 0.5) * v);
        let j = self.graph.node_near(site, centre, self.food_r)?;
        self.field.get(&j).map(|&d| (j, d))
    }
}

/// The route steps (one voxel each) a hunter walks before it gives a walk up:
/// `cruise × patrol_give_up_s / voxel`.
fn reach_steps(fauna: &Fauna, founder: Founder, v: f64) -> u32 {
    let m = fauna.config.manifest(founder).cruise_m_per_s * fauna.config.hunt.patrol_give_up_s;
    if m.is_finite() && m > 0.0 && v > 0.0 { (m / v).floor().min(f64::from(u32::MAX)) as u32 } else { 0 }
}

/// The hunter at `i` on its walking route graph, `None` when [`HuntConfig::reachable_only`]
/// is off or it stands off the graph (the graph is the adult disc's; a juvenile in a gap
/// may be off it): the callers then take main's rule.
pub(crate) fn route_from(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>) -> Option<Route> {
    let cfg = fauna.config.hunt;
    if !cfg.reachable_only {
        return None;
    }
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let v = view.config.voxel_m;
    let graph = fauna.routes(view, founder, cfg.route_refresh_s);
    let geometry = phys.body_at(a.body);
    let cols = |m: f64| if m.is_finite() && v > 0.0 { (m / v).ceil().max(0.0) as u32 } else { 0 };
    let own_r = cols(geometry.footprint_radius());
    let food_r = cols(geometry.footprint_radius() + geometry.mouth_reach_m);
    let from = graph.node_near(a.site, (a.pose.x, a.pose.z), own_r)?;
    let steps = reach_steps(fauna, founder, v);
    let field = graph.distances(from, steps);
    Some(Route { graph, from, field, steps, food_r })
}

/// Where the hunter at `i` heads on its way to node `goal` at `goal_xy`: straight at it when
/// the line is walkable on the graph, else the next node of its route there.
fn waypoint(fauna: &mut Fauna, i: usize, graph: &Arc<RouteGraph>, from: usize, goal: usize, steps: u32) -> (f64, f64) {
    let a = fauna.animals[i];
    let goal_xy = graph.centre(goal);
    if from == goal || graph.walk_line(from, (a.pose.x, a.pose.z), goal_xy) {
        return goal_xy;
    }
    let field = fauna.hunt_path(graph, a.id, goal, steps);
    graph.next_toward(&field, from).map_or(goal_xy, |j| graph.centre(j))
}

/// Whether a mouth column of `a` (standing as it is) holds a **usable carcass**: carrion
/// under the plant layer's carcass record, at its standing face's height (where a mouth
/// takes a pool).
pub(crate) fn mouth_on_carcass(fv: &FloraView<'_>, view: &VoxelView<'_>, a: &crate::Animal, geometry: &crate::Body) -> bool {
    crate::body::mouth_columns(view, &a.pose, geometry).into_iter().any(|(x, z)| {
        let site = Site { x: x.rem_euclid(i64::from(view.config.width)) as u32, y: a.site.y, z };
        fv.ground_at(site).is_some_and(|g| g.carcass.is_some() && g.carrion > 0.0)
    })
}

/// **The meal step** (HUNTER-MEAL, [`HuntConfig::meal_first`]) for the hungry hunter at `i`
/// that eats carrion: with its mouth over a usable carcass, eat (feed held, still); else,
/// with one it can walk to within [`HuntConfig::carcass_scan_m`] (on its route field when
/// [`HuntConfig::reachable_only`], the nearest by route; else the nearest), go to it, feed
/// off so nothing is struck on the way. A walk that has not reached its carcass after
/// [`HuntConfig::patrol_give_up_s`] remembers it fruitless. `None`: no meal to go to.
pub(crate) fn meal_step(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>) -> Option<Actions> {
    meal_step_among(fauna, i, view, fv, None)
}

/// [`meal_step`] to one carcass, `site` (the Veilhound pack's feast), at any distance: the
/// same walk — on the route field when [`HuntConfig::reachable_only`], given up and
/// remembered fruitless after [`HuntConfig::patrol_give_up_s`]. `None`: it is not on the
/// hunter's route field, or remembered fruitless, or the meal step does not apply.
pub(crate) fn meal_step_to(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>, site: Site) -> Option<Actions> {
    meal_step_among(fauna, i, view, fv, Some(site))
}

fn meal_step_among(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>, only: Option<Site>) -> Option<Actions> {
    let cfg = fauna.config.hunt;
    let a = fauna.animals[i];
    if !cfg.meal_first || !a.eating || a.attached.is_some() || a.strike.is_some() {
        return None;
    }
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    phys.hunter?;
    if !phys.diet.foods().any(|f| f == Food::Carrion) {
        return None;
    }
    let tick = fauna.tick;
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let geometry = phys.body_at(a.body);
    let mut m = a.hunt;
    if mouth_on_carcass(fv, view, &a, &geometry) {
        m.mode = HuntMode::Meal;
        m.samples = 0;
        fauna.animals[i].hunt = m;
        fauna.ledger.hunt_meal_eat_samples += 1;
        return Some(Actions { forward: 0.0, turn: 0.0, feed: 1.0 });
    }
    let scan = cfg.carcass_scan_m.max(0.0);
    let centre = |s: Site| ((f64::from(s.x) + 0.5) * v, (f64::from(s.z) + 0.5) * v);
    // SENSES-CORE: the carcasses it knows of are its record's (the carcass scan).
    let sites: Vec<Site> = match only {
        Some(s) => vec![s],
        None => {
            let want = crate::perception::Want { carcasses: true, ..Default::default() };
            crate::perception::refresh(fauna, i, view, fv, None, want);
            fauna.percept(a.id).map(|p| p.carcass_sites().collect()).unwrap_or_default()
        }
    };
    let near: Vec<(Site, f64)> = sites
        .iter()
        .filter_map(|&s| {
            let (x, z) = centre(s);
            let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, x, z, ring);
            let d = (dx * dx + dz * dz).sqrt();
            ((only.is_some() || d <= scan) && !a.hunt.is_fruitless(&cfg, x, z, tick, ring)).then_some((s, d))
        })
        .collect();
    let leave = |fauna: &mut Fauna, mut m: HuntMemory| {
        if m.mode == HuntMode::Meal {
            m.mode = HuntMode::Idle;
            m.samples = 0;
        }
        fauna.animals[i].hunt = m;
    };
    if near.is_empty() {
        leave(fauna, m);
        return None;
    }
    // The carcass to go to, and the point to head for.
    let route = route_from(fauna, i, view);
    let (site, go) = match &route {
        Some(r) => {
            let best = near
                .iter()
                .filter_map(|&(s, _)| r.reach(s, v).map(|(j, d)| (d, s, j)))
                .min_by(|p, q| p.0.cmp(&q.0).then(p.1.cmp(&q.1)));
            let Some((_, s, j)) = best else {
                leave(fauna, m);
                return None;
            };
            let go = waypoint(fauna, i, &r.graph, r.from, j, r.steps);
            (s, go)
        }
        None => {
            let (s, _) = near
                .iter()
                .copied()
                .min_by(|p, q| p.1.total_cmp(&q.1).then(p.0.cmp(&q.0)))
                .expect("not empty");
            (s, centre(s))
        }
    };
    let cadence_s = fauna.config.manifest(founder).controller_period_s;
    if m.mode == HuntMode::Meal {
        m.samples += 1;
        if m.samples >= samples_for(cfg.patrol_give_up_s, cadence_s) {
            let (x, z) = centre(site);
            m.remember_fruitless(x, z, tick + cubarium_voxel::ticks_at_least_one(cfg.fruitless_memory_s));
            fauna.ledger.hunt_fruitless += 1;
            m.mode = HuntMode::Idle;
            m.samples = 0;
            fauna.animals[i].hunt = m;
            return None;
        }
    } else {
        m.mode = HuntMode::Meal;
        m.samples = 0;
        m.escape = 0;
    }
    // Walking: the teachers' escape from a refused motor, else toward the carcass.
    let t = fauna.config.teachers;
    let fb = a.founder_state.feedback;
    let refused = fb.attempted_equivalent > 0.0 && fb.delivered_equivalent / fb.attempted_equivalent < t.motor_refused;
    if refused && m.escape == 0 {
        m.escape = t.escape_samples.max(1);
    }
    let act = if m.escape > 0 {
        m.escape -= 1;
        let side = if draw(DOMAIN_ESCAPE, a.id, tick / u64::from(t.escape_samples.max(1)), 1) < 0.5 { -1.0 } else { 1.0 };
        Actions { forward: t.escape_creep, turn: side, feed: 0.0 }
    } else {
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, go.0, go.1, ring);
        head(a.pose.heading_rad, dx.atan2(dz), 1.0, 0.0)
    };
    fauna.animals[i].hunt = m;
    fauna.ledger.hunt_meal_walk_samples += 1;
    Some(act)
}

/// The point a travelling patroller heads for on its way to [`HuntMemory::patch_face`]
/// ([`waypoint`]); `None` off the route rule, off the graph, or with no face (straight at
/// the patch, main's rule).
fn travel_point(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>) -> Option<(f64, f64)> {
    let cfg = fauna.config.hunt;
    if !cfg.reachable_only {
        return None;
    }
    let a = fauna.animals[i];
    let (x, y, z) = a.hunt.patch_face?;
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let v = view.config.voxel_m;
    let graph = fauna.routes(view, founder, cfg.route_refresh_s);
    let goal = graph.face(Site { x, y, z })?;
    let own_r = (phys.body_at(a.body).footprint_radius() / v).ceil().max(0.0) as u32;
    let from = graph.node_near(a.site, (a.pose.x, a.pose.z), own_r)?;
    let steps = reach_steps(fauna, founder, v);
    Some(waypoint(fauna, i, &graph, from, goal, steps))
}

/// Controller samples in `s` seconds at `cadence_s` a sample, at least one.
fn samples_for(s: f64, cadence_s: f64) -> u32 {
    if cadence_s > 0.0 && s.is_finite() { (s / cadence_s).ceil().clamp(1.0, f64::from(u32::MAX)) as u32 } else { 1 }
}

/// A kill by `striker` of a prey that stood at `(x, z)` (HUNTER-MEAL): the place is
/// remembered productive in the striker's memory for [`HuntConfig::success_memory_s`].
pub(crate) fn remember_kill(fauna: &mut Fauna, striker: u64, x: f64, z: f64, ring: f64) {
    let cfg = fauna.config.hunt;
    if !(cfg.success_memory_s > 0.0) {
        return;
    }
    let tick = fauna.tick;
    let until = tick + cubarium_voxel::ticks_at_least_one(cfg.success_memory_s);
    if let Ok(j) = fauna.animals.binary_search_by_key(&striker, |a| a.id) {
        fauna.animals[j].hunt.remember_productive(&cfg, x, z, tick, until, ring);
        fauna.ledger.hunt_productive_kills += 1;
    }
}

/// The patrol's answer for the due body at `i` — its held actions and its lure — or
/// `None` to leave its controller's (the patrol off, no hunter table, riding, striking).
pub(crate) fn steer(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    occupancy: Option<crate::senses::Occ<'_>>,
) -> Option<(Actions, f64)> {
    let cfg = fauna.config.hunt;
    if !cfg.patrol {
        return None;
    }
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let h = phys.hunter?;
    if a.attached.is_some() || a.strike.is_some() {
        return None;
    }
    let tick = fauna.tick;
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let cadence_s = fauna.config.manifest(founder).controller_period_s;
    let samples_of = |s: f64| -> u32 {
        if cadence_s > 0.0 && s.is_finite() {
            (s / cadence_s).ceil().clamp(1.0, f64::from(u32::MAX)) as u32
        } else {
            1
        }
    };
    let lit = if h.lure_power > 0.0 { 1.0 } else { 0.0 };
    let still = Actions { forward: 0.0, turn: 0.0, feed: 0.0 };
    // Fed: digest where it stands.
    if !a.eating {
        fauna.animals[i].hunt.mode = HuntMode::Idle;
        return Some((still, 0.0));
    }
    // HUNTER-MEAL: a usable carcass at its mouth or one it can walk to comes first.
    if let Some(go) = meal_step(fauna, i, view, fv) {
        return Some((go, 0.0));
    }

    // 1. Senses: the size-eligible prey it sees or feels, and its encounters.
    let pc = fauna.config.predation;
    let (fx, fz) = a.pose.forward();
    let (rx, rz) = (fz, -fx);
    let mut met = [false; Founder::COUNT];
    let mut declined = [false; Founder::COUNT];
    let mut target: Option<(f64, f64, f64)> = None;
    let mut tremor = [0.0f64; 2];
    let reach = cfg.sight_m.max(pc.tremor_radius_m);
    // SENSES-CORE: the prey in its record — by the patrol's own rule while its lineage's
    // table is not live ([`crate::perception::refresh`]); a live lineage's core tracks read
    // through the same reach, sight range and tremor radius.
    crate::perception::refresh(fauna, i, view, fv, occupancy, crate::perception::Want { prey: true, ..Default::default() });
    let tracks: Vec<crate::Track> = fauna
        .percept(a.id)
        .map(|p| p.tracks().iter().filter(|t| t.class == crate::perception::class::PREY).copied().collect())
        .unwrap_or_default();
    let mut target_id: Option<u64> = None;
    for t in &tracks {
        let Some(o) = fauna.view().animal(t.id).copied() else { continue };
        if !crate::is_prey_of(&fauna.config, &a, &o) {
            continue;
        }
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, t.x, t.z, ring);
        let d = (dx * dx + dz * dz).sqrt();
        if d > reach {
            continue;
        }
        let felt = if t.modes & crate::perception::TREMOR != 0 && d <= pc.tremor_radius_m { t.felt } else { 0.0 };
        let seen = t.modes & crate::perception::SIGHT != 0 && d <= cfg.sight_m && dx * fx + dz * fz >= 0.0;
        if !(seen || felt > 0.0) {
            continue;
        }
        let Some(pf) = o.founder else { continue };
        met[pf.index()] = true;
        if !accepts(&fauna.config, &a, &o, tick) {
            declined[pf.index()] = true;
            continue;
        }
        if seen && target.is_none_or(|t| d < t.0) {
            target = Some((d, dx, dz));
            target_id = Some(o.id);
        }
        if felt > 0.0 {
            let side = dx * rx + dz * rz;
            if side > 0.0 {
                tremor[1] += felt;
            } else if side < 0.0 {
                tremor[0] += felt;
            } else {
                tremor[0] += 0.5 * felt;
                tremor[1] += 0.5 * felt;
            }
        }
    }
    fauna.animals[i].hunt.meet(&cfg, tick, &met, cadence_s);
    for (k, &m) in met.iter().enumerate() {
        if m {
            fauna.ledger.hunt_encounter_s_by_prey[k] += cadence_s;
        }
        if declined[k] {
            fauna.ledger.hunt_declined_s_by_prey[k] += cadence_s;
        }
    }

    // 2. Engage: a prey in sight, then prey tremor.
    if let Some((d, dx, dz)) = target {
        let geometry = phys.body_at(a.body);
        let in_reach = (h.jaw_forward_length_fraction + h.strike_reach_length_fraction) * geometry.length_m;
        let forward = if d <= in_reach { 0.0 } else { fauna.config.teachers.stalk };
        let m = &mut fauna.animals[i].hunt;
        m.mode = HuntMode::Wait;
        m.patch = (a.pose.x, a.pose.z);
        m.samples = 0;
        fauna.ledger.hunt_engaged_samples += 1;
        if let Some(id) = target_id {
            crate::perception::approach(fauna, a.id, id);
        }
        return Some((head(a.pose.heading_rad, dx.atan2(dz), forward, 1.0), lit));
    }
    if tremor[0] + tremor[1] >= cfg.tremor_turn && tremor[0] + tremor[1] > 0.0 {
        let turn = if tremor[0] > tremor[1] { -1.0 } else { 1.0 };
        let m = &mut fauna.animals[i].hunt;
        m.mode = HuntMode::Wait;
        m.patch = (a.pose.x, a.pose.z);
        m.samples = 0;
        fauna.ledger.hunt_engaged_samples += 1;
        return Some((Actions { forward: 0.0, turn, feed: 1.0 }, lit));
    }

    // 3. The patrol.
    let forget_at = tick + cubarium_voxel::ticks_at_least_one(cfg.fruitless_memory_s);
    let mut m = fauna.animals[i].hunt;
    if m.mode == HuntMode::Wait {
        m.samples += 1;
        if m.samples < samples_of(cfg.patch_wait_s) {
            fauna.animals[i].hunt = m;
            fauna.ledger.hunt_wait_samples += 1;
            return Some((still, lit));
        }
        // A fruitless wait: remember the patch and choose the next.
        m.remember_fruitless(m.patch.0, m.patch.1, forget_at);
        fauna.ledger.hunt_fruitless += 1;
        m.mode = HuntMode::Idle;
    }
    if m.mode == HuntMode::Travel {
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, m.patch.0, m.patch.1, ring);
        let d = (dx * dx + dz * dz).sqrt();
        if d <= cfg.patch_arrive_m {
            m.mode = HuntMode::Wait;
            m.samples = 0;
            fauna.animals[i].hunt = m;
            fauna.ledger.hunt_wait_samples += 1;
            return Some((still, lit));
        }
        m.samples += 1;
        if m.samples >= samples_of(cfg.patrol_give_up_s) {
            m.remember_fruitless(m.patch.0, m.patch.1, forget_at);
            fauna.ledger.hunt_fruitless += 1;
            m.mode = HuntMode::Idle;
        }
    }
    if m.mode == HuntMode::Idle {
        fauna.animals[i].hunt = m;
        let patch = choose_patch(fauna, i, view, fv, &h);
        m = fauna.animals[i].hunt;
        m.patch = patch;
        m.mode = HuntMode::Travel;
        m.samples = 0;
        m.escape = 0;
    }
    // Walking: the teachers' escape from a refused motor, else straight for the patch.
    let t = fauna.config.teachers;
    let fb = a.founder_state.feedback;
    let refused = fb.attempted_equivalent > 0.0 && fb.delivered_equivalent / fb.attempted_equivalent < t.motor_refused;
    if refused && m.escape == 0 {
        m.escape = t.escape_samples.max(1);
    }
    let act = if m.escape > 0 {
        m.escape -= 1;
        let side = if draw(DOMAIN_ESCAPE, a.id, tick / u64::from(t.escape_samples.max(1)), 0) < 0.5 { -1.0 } else { 1.0 };
        Actions { forward: t.escape_creep, turn: side, feed: 0.0 }
    } else {
        // HUNTER-MEAL: along its route graph where the straight line is not walkable.
        fauna.animals[i].hunt = m;
        let go = travel_point(fauna, i, view).unwrap_or(m.patch);
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, go.0, go.1, ring);
        head(a.pose.heading_rad, dx.atan2(dz), 1.0, 0.0)
    };
    fauna.animals[i].hunt = m;
    fauna.ledger.hunt_travel_samples += 1;
    Some((act, 0.0))
}
