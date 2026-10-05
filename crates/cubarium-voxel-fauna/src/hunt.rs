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
//! 2. **Attack preference** (D2b: switching on encounter share). Whether it takes on a
//!    prey it meets is drawn once per [`HuntConfig::attack_decision_s`] per prey ([`accepts`]),
//!    at its [`profitability`] (the share of a kill one strike makes) times
//!    [`attack_preference`]: the lineage's **share** of its recent eligible encounters
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

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::{FloraView, Trophic};
use serde::{Deserialize, Serialize};

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
    /// Controller samples in the current mode.
    pub samples: u32,
    /// Samples of an escape from a refused motor left.
    pub escape: u32,
    pub fruitless: [FruitlessPatch; FRUITLESS_SLOTS],
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
        self.fruitless.iter().any(|p| {
            p.until_tick > tick && {
                let (dx, dz) = crate::pose::wrapped_delta(p.x, p.z, x, z, ring);
                dx * dx + dz * dz <= cfg.fruitless_radius_m * cfg.fruitless_radius_m
            }
        })
    }
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
fn head(heading: f64, want: f64, forward: f64, feed: f64) -> Actions {
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
/// choice. `None` when nothing is open at all.
fn choose_patch(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>, h: &crate::Hunter) -> (f64, f64) {
    let cfg = fauna.config.hunt;
    let a = fauna.animals[i];
    let tick = fauna.tick;
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let pm = cfg.patch_m.max(v);
    let scan = cfg.cue_scan_m.max(0.0);
    // Which foods mark which prey lineage's habitat.
    let lineages: Vec<Founder> = Founder::ALL.iter().copied().filter(|&f| h.hunts(f)).collect();
    let weight: Vec<f64> = lineages.iter().map(|&f| attack_preference(&cfg, h, &a.hunt.image, f)).collect();
    // Squares keyed by their absolute index; each holds the cue per lineage.
    let mut squares: rustc_hash::FxHashMap<(i64, i64), Vec<f64>> = rustc_hash::FxHashMap::default();
    let add = |x: f64, z: f64, k: usize, cue: f64, squares: &mut rustc_hash::FxHashMap<(i64, i64), Vec<f64>>| {
        if !(cue > 0.0) {
            return;
        }
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, x, z, ring);
        if dx * dx + dz * dz > scan * scan {
            return;
        }
        let key = ((x.rem_euclid(ring) / pm).floor() as i64, (z / pm).floor() as i64);
        squares.entry(key).or_insert_with(|| vec![0.0; lineages.len()])[k] += cue;
    };
    let cell_m2 = v * v;
    for g in fv.ground.iter() {
        if g.litter <= 0.0 && g.carrion <= 0.0 && g.film <= 0.0 {
            continue;
        }
        let (x, z) = ((f64::from(g.site.x) + 0.5) * v, (f64::from(g.site.z) + 0.5) * v);
        for (k, f) in lineages.iter().enumerate() {
            let diet = fauna.config.founder(*f).diet;
            let cue: f64 = diet.foods().map(|food| ground_cue(fv, fauna, g, food, cell_m2)).sum();
            add(x, z, k, cue, &mut squares);
        }
    }
    for st in fv.stands.iter() {
        let (x, z) = ((f64::from(st.site.x) + 0.5) * v, (f64::from(st.site.z) + 0.5) * v);
        for (k, f) in lineages.iter().enumerate() {
            let diet = fauna.config.founder(*f).diet;
            let cue: f64 = diet.foods().map(|food| stand_cue(fv, st, food)).sum();
            add(x, z, k, cue, &mut squares);
        }
    }
    // Each lineage's cue against its own best square, weighted by its preference.
    let mut best = vec![0.0f64; lineages.len()];
    for cues in squares.values() {
        for (b, &c) in best.iter_mut().zip(cues) {
            *b = b.max(c);
        }
    }
    let centre = |(kx, kz): (i64, i64)| (((kx as f64) + 0.5) * pm, ((kz as f64) + 0.5) * pm);
    let mut open: Vec<((f64, f64), f64, f64)> = Vec::new();
    let mut fruitless: Vec<((f64, f64), f64, f64)> = Vec::new();
    for (&key, cues) in &squares {
        let (x, z) = centre(key);
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, x, z, ring);
        let d = (dx * dx + dz * dz).sqrt();
        if d < cfg.patch_min_hop_m {
            continue;
        }
        let score: f64 = cues
            .iter()
            .zip(&best)
            .zip(&weight)
            .map(|((&c, &b), &w)| if b > 0.0 { w * c / b } else { 0.0 })
            .sum();
        if !(score > 0.0) {
            continue;
        }
        let entry = ((x, z), score, d);
        if a.hunt.is_fruitless(&cfg, x, z, tick, ring) {
            fruitless.push(entry);
        } else {
            open.push(entry);
        }
    }
    // Every square remembered fruitless: the memory gives way rather than strand it.
    let pool = if open.is_empty() { &mut fruitless } else { &mut open };
    // A stable order, so a draw picks the same square for the same world.
    pool.sort_by(|p, q| p.0.0.total_cmp(&q.0.0).then(p.0.1.total_cmp(&q.0.1)));
    let explore = draw(DOMAIN_EXPLORE, a.id, tick, 0) < cfg.patrol_explore;
    let pick = if pool.is_empty() {
        None
    } else if explore {
        let k = ((draw(DOMAIN_EXPLORE, a.id, tick, 1) * pool.len() as f64) as usize).min(pool.len() - 1);
        Some(pool[k].0)
    } else {
        pool.iter()
            .max_by(|p, q| p.1.total_cmp(&q.1).then(q.2.total_cmp(&p.2)))
            .map(|p| p.0)
    };
    match pick {
        Some(p) => {
            if explore {
                fauna.ledger.hunt_explored += 1;
            } else {
                fauna.ledger.hunt_chosen += 1;
            }
            (p.0.rem_euclid(ring), p.1)
        }
        None => {
            // No food in reach: walk off half the scan in a drawn direction.
            fauna.ledger.hunt_explored += 1;
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

/// Whether the hunter at `a` sees `o`, `(dx, dz)` away: ahead of it (the cone looks
/// forward) and not hidden, by the flee rule's ray against the predation occupancy.
fn sees(
    fauna: &Fauna,
    view: &VoxelView<'_>,
    occupancy: Option<&crate::senses::ConeOccupancy>,
    a: &crate::Animal,
    o: &crate::Animal,
    (dx, dz): (f64, f64),
) -> bool {
    let (fx, fz) = a.pose.forward();
    if dx * fx + dz * fz < 0.0 {
        return false;
    }
    let Some(occupancy) = occupancy else {
        return true;
    };
    let config = &fauna.config;
    let (Some(geometry), Some(og)) = (crate::body_of(config, a), crate::body_of(config, o)) else {
        return false;
    };
    let v = view.config.voxel_m;
    let origin = crate::senses::cone_origin(view, &a.pose, a.sense_layer(v), &geometry);
    let base = if o.mobility.mode != crate::MobilityMode::Ground && o.pose.y_m.is_finite() {
        o.pose.y_m
    } else {
        crate::surface_m(o.sense_layer(v), v)
    };
    let ty = base + 0.5 * og.height_m;
    let (ex, ey, ez) = (dx, ty - origin.1, dz);
    let dist = (ex * ex + ey * ey + ez * ez).sqrt();
    if !(dist > 0.0) {
        return true;
    }
    let dir = (ex / dist, ey / dist, ez / dist);
    matches!(
        crate::senses::ray_first_hit(view, occupancy, a.id, origin, dir, dist + og.width_m),
        Some((hit, crate::senses::Fine::Body)) if hit >= dist - og.width_m - v
    )
}

/// The patrol's answer for the due body at `i` — its held actions and its lure — or
/// `None` to leave its controller's (the patrol off, no hunter table, riding, striking).
pub(crate) fn steer(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    occupancy: Option<&crate::senses::ConeOccupancy>,
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

    // 1. Senses: the size-eligible prey it sees or feels, and its encounters.
    let pc = fauna.config.predation;
    let (fx, fz) = a.pose.forward();
    let (rx, rz) = (fz, -fx);
    let mut met = [false; Founder::COUNT];
    let mut declined = [false; Founder::COUNT];
    let mut target: Option<(f64, f64, f64)> = None;
    let mut tremor = [0.0f64; 2];
    let reach = cfg.sight_m.max(pc.tremor_radius_m);
    for o in &fauna.animals {
        if o.id == a.id || !crate::is_prey_of(&fauna.config, &a, o) {
            continue;
        }
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, o.pose.x, o.pose.z, ring);
        let d = (dx * dx + dz * dz).sqrt();
        if d > reach {
            continue;
        }
        let felt = if d <= pc.tremor_radius_m { crate::tremor_of(o, d, pc.tremor_near_m) } else { 0.0 };
        let seen = d <= cfg.sight_m && sees(fauna, view, occupancy, &a, o, (dx, dz));
        if !(seen || felt > 0.0) {
            continue;
        }
        let Some(pf) = o.founder else { continue };
        met[pf.index()] = true;
        if !accepts(&fauna.config, &a, o, tick) {
            declined[pf.index()] = true;
            continue;
        }
        if seen && target.is_none_or(|t| d < t.0) {
            target = Some((d, dx, dz));
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
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, m.patch.0, m.patch.1, ring);
        head(a.pose.heading_rad, dx.atan2(dz), 1.0, 0.0)
    };
    fauna.animals[i].hunt = m;
    fauna.ledger.hunt_travel_samples += 1;
    Some((act, 0.0))
}
