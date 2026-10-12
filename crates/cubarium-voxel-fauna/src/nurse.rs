//! **SP-NURSE: mothers nurse their young** (Wrysk, 2026-10-11: "sure lets add nursing").
//!
//! A seedporter young eats fruit from birth, but fruit carries a fifth of the nitrogen its
//! tissue needs (mineral 0.01 per organic against `n_tissue` 0.05), so a bite builds at most a
//! fifth of its matter as tissue and the young grows slowly. Real primates solve this with
//! milk: the mother concentrates nitrogen from a large fruit intake into her own tissue and
//! passes it on. Here her **reserve** is that concentrate: mineral-funded organic matter at
//! her body's own density.
//!
//! - **The mother link.** A young born out of a live-bearer's gestation records its mother's
//!   id ([`crate::Animal::mother`]).
//! - **Nursing** ([`nurse`]). While a young is younger than `weaning_age_s` and within
//!   `nurse_radius_m` (3D) of its living mother, she passes reserve to it at up to
//!   `nurse_rate_per_s` × her body, never below `nurse_reserve_floor` × her reserve cap (she
//!   feeds herself first) and never more than the young's reserve room. The young receives
//!   `nurse_efficiency` of it, organic matter, mineral and energy alike; the rest is respired.
//!   The transfer is the Veilhound den's reserve fallback ([`crate::pack::pass_reserve`]), so
//!   there is one conservation path. **Twins share by room**: each tick's milk is split
//!   between her nursing young in proportion to their reserve room, so the emptier one gets
//!   more and neither is fed past its room.
//! - **The follow** ([`mother_pull`]). While dependent, the young's group pull
//!   ([`crate::herding::group_pull`]) targets its mother, when it senses her, instead of the
//!   troop's mean, with the tighter comfort `follow_m` (the leash `leash_m` when that is on)
//!   and the weight `follow_pull`. It reads only the young's own percept record. Real infants
//!   are carried; carrying is a pose, and poses are art, so it is not here.
//! - **The mother as its food source** (SP-FOLLOW, [`mother_goal`]). The drift above only
//!   leans a walking turn, so a young feeding, climbing, perched or on a fruit walk was never
//!   drawn (an 8-seed check: about 1–2 % of dependent time in contact). With `leash_m` on, a
//!   dependent young with reserve room (hungry) goes to its mother as it would go to fruit,
//!   over its own foraging, by the fruit walk's route and climb (`step::glider::to_mother`),
//!   so it reaches her up a tree too; in contact it holds still and nurses. A sated young
//!   forages on its own within `leash_m` of her and goes back past it. Fleeing still wins.
//! - **Its mother in its senses.** It weighs her on every remote sensing on top of its
//!   lineage's `kin_tracks` (the seedporter weighs one: the nearest of its kind, usually its
//!   twin, so its mother was almost never in its record), and it **remembers** where it last
//!   sensed her for `mother_memory_s` ([`crate::Fauna::mother_seen`], session state), heading
//!   there when she is out of its senses, until it gets there. **In contact it feels her**
//!   (SP-FOLLOW-2): a newborn beside a still, silent mother on a vine, out of its field or
//!   past a terrace step, took the spot where it last sensed her for one she had left, forgot
//!   her within seconds of its birth, and never found her again.
//!
//! **Never scored.** Nursing is no fitness or reward term; the ledger's `nursed_*` lines are
//! readouts only.

use serde::{Deserialize, Serialize};

use cubarium_voxel::{DT, VoxelView};

use crate::herding::Pull;
use crate::perception::class;
use crate::{Fauna, Founder};

/// The reference world year the weaning age is stated in, seconds: five 7,200 s ecological
/// days (10 h), the seedporter's calendar's year (SP-TWINS).
const YEAR_S: f64 = 5.0 * 7200.0;

/// **A lineage's nursing** (module note): its knobs. Off with `weaning_age_s` 0 or
/// `nurse_rate_per_s` 0.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Nursing {
    /// A young is nursed, and follows its mother, while younger than this, seconds; 0: off.
    pub weaning_age_s: f64,
    /// A young within this of its mother (3D, centre to centre) is nursed, metres.
    pub nurse_radius_m: f64,
    /// The most a mother passes per second, organic per unit of her body, split between her
    /// nursing young; 0: off.
    pub nurse_rate_per_s: f64,
    /// She never nurses below this fraction of her reserve cap (`reserve_cap · body`).
    pub nurse_reserve_floor: f64,
    /// The share of what she passes the young receives; the rest is respired.
    pub nurse_efficiency: f64,
    /// A dependent young that senses its mother beyond this drifts back toward her, metres;
    /// 0: the young keeps to the troop's pull.
    pub follow_m: f64,
    /// That drift's share of its walking turn (1: straight at its mother).
    pub follow_pull: f64,
    /// **SP-FOLLOW**: a sated dependent young forages on its own within this of its mother
    /// (3D), and goes back to her past it; a hungry one (reserve room) goes to her whatever
    /// the distance. Metres; 0: off (the drift only).
    pub leash_m: f64,
    /// Out of its senses, the young heads for where it last sensed its mother for this long,
    /// seconds; 0: only while it senses her.
    pub mother_memory_s: f64,
}

impl Default for Nursing {
    /// No nursing, no follow.
    fn default() -> Nursing {
        Nursing {
            weaning_age_s: 0.0,
            nurse_radius_m: 0.0,
            nurse_rate_per_s: 0.0,
            nurse_reserve_floor: 0.0,
            nurse_efficiency: 0.0,
            follow_m: 0.0,
            follow_pull: 0.0,
            leash_m: 0.0,
            mother_memory_s: 0.0,
        }
    }
}

impl Nursing {
    /// Whether nursing is on at all.
    pub fn on(&self) -> bool {
        self.weaning_age_s > 0.0 && self.nurse_rate_per_s > 0.0
    }

    /// Whether a dependent young keeps to its mother at all: nursing on, and the drift or the
    /// leash on.
    pub fn follows(&self) -> bool {
        self.on() && (self.follow_m > 0.0 || self.leash_m > 0.0)
    }
}

/// **Each lineage's nursing** (SP-NURSE): the seedporter's, the rest off. Its numbers, each a
/// knob (`fauna.founders.<lineage>.nursing.*`):
/// - `weaning_age_s` 7,200 s, 0.2 of the 10 h year: a common marmoset weans at about 2–3
///   months of its year;
/// - `nurse_rate_per_s` 2e-4 per unit of her body: a lactating marmoset's milk, about 2 % of
///   her dry mass a day for twins (≈ 100 kJ/day of milk against a ≈ 300 kJ/day field
///   metabolism at 0.4 kg), with a real day ≈ 96 world seconds (75 days to the 7,200 s
///   weaning age). Far above what fruit's nitrogen lets her store, so in practice her income
///   above the floor, not this, sets the milk;
/// - `nurse_reserve_floor` 0.25 of her reserve cap: a mother in poor condition cuts milk, and
///   she keeps a quarter of a full reserve for herself, clear of the emergency at hunger 0.9;
/// - `nurse_efficiency` 0.8: mammary milk synthesis is about 80 % efficient;
/// - `nurse_radius_m` 0.5: contact, one of its 0.5 m adult lengths centre to centre;
/// - `follow_m` 0.4, inside the contact radius, so a young drifts back before it loses the
///   teat; `follow_pull` 0.8: an infant's keeping to its mother outweighs its own foraging
///   turn (the troop's pull is 0.5);
/// - `leash_m` 3 (SP-FOLLOW): young primates forage within a few metres of their mother
///   (six of its 0.5 m adult lengths, under the troop's 4 m comfort);
/// - `mother_memory_s` 120: a separated infant searches where it lost its mother for
///   minutes (twice the troop's one-minute memory: its mother is its whole food supply).
///
/// **Budget** (SP-NEWBORN's measured numbers, before these were set): a mother conceives only
/// full grown (a growing adult's reserve sits near 5 % of its cap, under the surplus floor), so
/// she is 0.71 with a 0.36 cap and a 0.09 floor. Fruit at 0.36–0.48 body/h, a fifth of it
/// nitrogen-funded, stores 0.05–0.07 organic/h: far under the 0.51/h rate, so she drains to
/// her floor within minutes and then passes her income; twins receive 0.04–0.055/h, 0.02–0.027
/// each, against a young's own 0.015/h at 0.145 (relative growth about 0.1/h → 0.25–0.3/h).
/// Re-conceived about 33 min after a birth (refractory plus surplus hold), her gestation draws
/// about 0.07/h, near all her income, so milk then falls to little (a marmoset is pregnant and
/// lactating at once too).
///
/// The loftstrider is a live-bearer too, but its calves are another design thread's: off.
pub fn nursing_of(founder: Founder) -> Nursing {
    match founder {
        Founder::Seedporter => Nursing {
            weaning_age_s: 0.2 * YEAR_S,
            nurse_radius_m: 0.5,
            nurse_rate_per_s: 2.0e-4,
            nurse_reserve_floor: 0.25,
            nurse_efficiency: 0.8,
            follow_m: 0.4,
            follow_pull: 0.8,
            leash_m: 3.0,
            mother_memory_s: 120.0,
        },
        _ => Nursing::default(),
    }
}

/// The nursing table of body `i`'s lineage, when on.
fn table_of(fauna: &Fauna, i: usize) -> Option<Nursing> {
    let t = fauna.config.founder(fauna.animals.get(i)?.founder?).nursing;
    t.on().then_some(t)
}

/// Whether body `i` is a **dependent young**: it knows its mother, its lineage nurses, and it
/// is younger than the weaning age. Says nothing of whether its mother is alive or near.
pub fn dependent(fauna: &Fauna, i: usize) -> bool {
    let Some(a) = fauna.animals.get(i) else {
        return false;
    };
    crate::mother_body(a).is_some() && table_of(fauna, i).is_some_and(|t| (a.age_ticks as f64) * DT < t.weaning_age_s)
}

/// The 3D distance between bodies `a` and `b`, metres, x wrapping on the ring.
fn distance_3d(fauna: &Fauna, a: usize, b: usize, ring: f64) -> f64 {
    let (p, q) = (fauna.animals[a].pose, fauna.animals[b].pose);
    let (dx, dz) = crate::pose::wrapped_delta(p.x, p.z, q.x, q.z, ring);
    (dx * dx + dz * dz + (q.y_m - p.y_m).powi(2)).sqrt()
}

/// Body `i`'s reserve room: its cap less its reserve (negative when over).
fn reserve_room(fauna: &Fauna, i: usize) -> f64 {
    let a = &fauna.animals[i];
    crate::body::effective_config(&fauna.config, a).reserve_of(a.body) - a.reserve
}

/// **Nursing** (module note), once a tick after maintenance: every living mother with a
/// nursing table passes reserve to her dependent young within her radius, split by their
/// reserve room. A mother's table sets the numbers; a young that is not hers, past weaning,
/// out of reach or without room gets nothing, and a mother who has died is not in the layer,
/// so her young go unfed.
pub(crate) fn nurse(fauna: &mut Fauna, view: &VoxelView<'_>) {
    if !Founder::ALL.iter().any(|&f| fauna.config.founder(f).nursing.on()) {
        return;
    }
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    // SP-FOLLOW: the memories of the departed go (a young's own is checked as it reads it).
    if fauna.tick % 64 == 0 && !fauna.mother_seen.is_empty() {
        let animals = &fauna.animals;
        fauna.mother_seen.retain(|id, _| animals.binary_search_by_key(id, |a| a.id).is_ok());
    }
    // (mother id, young index) for every dependent young, mothers grouped in id order.
    let mut young: Vec<(u64, usize)> = (0..fauna.animals.len())
        .filter(|&k| dependent(fauna, k))
        .filter_map(|k| crate::mother_body(&fauna.animals[k]).map(|m| (m, k)))
        .collect();
    if young.is_empty() {
        return;
    }
    young.sort_unstable();
    let mut start = 0;
    while start < young.len() {
        let mother = young[start].0;
        let end = start + young[start..].iter().take_while(|(m, _)| *m == mother).count();
        let brood = &young[start..end];
        start = end;
        // The layer keeps its bodies in id order (as `carry_riders` relies on).
        let Ok(g) = fauna.animals.binary_search_by_key(&mother, |a| a.id) else {
            continue; // she has died
        };
        let Some(t) = table_of(fauna, g) else {
            continue;
        };
        let eff = t.nurse_efficiency.clamp(0.0, 1.0);
        if !(eff > 0.0) {
            continue;
        }
        // Her young in reach with room, and their rooms.
        let fed: Vec<(usize, f64)> = brood
            .iter()
            .filter(|&&(_, k)| distance_3d(fauna, g, k, ring) <= t.nurse_radius_m)
            .map(|&(_, k)| (k, reserve_room(fauna, k)))
            .filter(|&(_, room)| room > 0.0)
            .collect();
        let rooms: f64 = fed.iter().map(|&(_, r)| r).sum();
        if fed.is_empty() || !(rooms > 0.0) {
            continue;
        }
        let a = &fauna.animals[g];
        let floor = t.nurse_reserve_floor.max(0.0) * crate::body::effective_config(&fauna.config, a).reserve_of(a.body);
        let give = (t.nurse_rate_per_s * a.body * DT).min(a.reserve - floor).min(rooms / eff);
        if !(give > 0.0) {
            continue;
        }
        let f = a.founder;
        for (k, room) in fed {
            let (given, received) = crate::pack::pass_reserve(fauna, g, k, give * (room / rooms), eff);
            if let Some(f) = f {
                let l = &mut fauna.ledger;
                l.nursed_given_by_founder[f.index()] += given;
                l.nursed_received_by_founder[f.index()] += received;
                l.provision_by_founder[f.index()] += given;
            }
        }
    }
}

/// **The follow** (module note) for body `i`: `Some` while it is a dependent young that senses
/// its mother (a fresh kin track of her id) — `Some(None)` within `follow_m` of her (no pull,
/// not even the troop's), else `Some(Some(pull))` toward her at `follow_pull`. `None` leaves
/// the body to the troop's pull: not dependent, the follow off, or its mother not in sense.
pub fn mother_pull(fauna: &Fauna, i: usize, ring: f64) -> Option<Option<Pull>> {
    if !dependent(fauna, i) {
        return None;
    }
    let a = &fauna.animals[i];
    let t = table_of(fauna, i)?;
    if !(t.follow_m > 0.0) {
        return None;
    }
    // SP-FOLLOW: with the leash on, the young is left to itself within it ([`mother_goal`]).
    let comfort = if t.leash_m > 0.0 { t.leash_m.max(t.follow_m) } else { t.follow_m };
    let mother = crate::mother_body(&a)?;
    let p = fauna.percept(a.id)?;
    let fresh = 2 * fauna.config.perception.stagger_ticks();
    let track = p
        .tracks()
        .iter()
        .find(|tr| tr.class == class::KIN && tr.id == mother && fauna.tick.saturating_sub(tr.tick) <= fresh)?;
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, track.x, track.z, ring);
    let d = dx.hypot(dz);
    Some((d > comfort).then(|| Pull { dx, dz, kin: 1, nearest_m: d, weight: t.follow_pull.clamp(0.0, 1.0) }))
}

/// Whether the body at `i` weighs body `o` as its mother on a remote sensing over its
/// lineage's kin cap ([`crate::perception`]): a dependent young whose lineage follows
/// ([`Nursing::follows`]), and `o` its mother.
pub(crate) fn weighs_as_mother(fauna: &Fauna, i: usize, o: u64) -> bool {
    fauna.animals.get(i).is_some_and(|a| crate::mother_body(a) == Some(o))
        && dependent(fauna, i)
        && table_of(fauna, i).is_some_and(|t| t.follows())
}

/// **What a dependent young does about its mother** this controller sample ([`mother_goal`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MotherGoal {
    /// In contact and hungry: it holds still beside her and nurses.
    Hold,
    /// It goes to her: where it senses her, or where it last did. Her centre on the ring and
    /// her base height, metres, her base over the support under her, and that support face.
    Go { x: f64, z: f64, y_m: f64, lift_m: f64, site: (u32, u32, u32) },
}

/// **The mother as its food source** (module note) for the body at `i`, `ring` the world's
/// x circumference: `None` leaves it to its heuristic — not a dependent young, the leash
/// off, its mother dead, out of its senses longer than `mother_memory_s`, or sated within
/// `leash_m` of her (and sated in contact). It reads its own percept record and keeps the
/// memory of her ([`crate::Fauna::mother_seen`]); reaching where it last sensed her without
/// sensing her ends that memory. In contact it feels her (SP-FOLLOW-2, [`felt`]), so a
/// mother it touches is never taken for one who has left.
pub(crate) fn mother_goal(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>) -> Option<MotherGoal> {
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let id = fauna.animals.get(i)?.id;
    let t = match table_of(fauna, i) {
        Some(t) if dependent(fauna, i) && t.leash_m > 0.0 => t,
        _ => {
            fauna.mother_seen.remove(&id);
            return None;
        }
    };
    let a = fauna.animals[i];
    let mother = crate::mother_body(&a)?;
    if fauna.animals.binary_search_by_key(&mother, |b| b.id).is_err() {
        fauna.mother_seen.remove(&id);
        return None;
    }
    let now = fauna.tick;
    let fresh = 2 * fauna.config.perception.stagger_ticks();
    let seen = fauna
        .percept(id)
        .and_then(|p| p.tracks().iter().find(|tr| tr.class == class::KIN && tr.id == mother && now.saturating_sub(tr.tick) <= fresh).copied());
    // SP-FOLLOW-2: in contact it feels her, whatever its eyes and ears give (a still, silent
    // mother behind it on a vine or past a terrace step is in none of its senses).
    let seen = seen.or_else(|| felt(fauna, i, mother, ring, t.nurse_radius_m, view));
    if let Some(tr) = seen {
        fauna.mother_seen.insert(id, tr);
    }
    let memory = if t.mother_memory_s.is_finite() && t.mother_memory_s > 0.0 { cubarium_voxel::ticks_at_least_one(t.mother_memory_s) } else { 0 };
    let m = *fauna.mother_seen.get(&id)?;
    let sensed = now.saturating_sub(m.tick) <= fresh;
    if !sensed && now.saturating_sub(m.tick) > memory {
        fauna.mother_seen.remove(&id);
        return None;
    }
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, m.x, m.z, ring);
    let d = dx.hypot(dz).hypot(m.y_m - a.pose.y_m);
    let hungry = reserve_room(fauna, i) > 0.0;
    if d <= t.nurse_radius_m {
        if sensed {
            return hungry.then_some(MotherGoal::Hold);
        }
        // Where it last sensed her, and she is not there.
        fauna.mother_seen.remove(&id);
        return None;
    }
    if !hungry && d <= t.leash_m {
        return None;
    }
    Some(MotherGoal::Go { x: m.x, z: m.z, y_m: m.y_m, lift_m: m.lift_m, site: m.site })
}

/// **Touch** (SP-FOLLOW-2): the track of its mother, by id, that the body at `i` feels in
/// contact with her (within `radius_m`, 3D, as nursing takes it), else `None`. Her centre,
/// base height, support face and lift, as a sensing records them.
fn felt(fauna: &Fauna, i: usize, mother: u64, ring: f64, radius_m: f64, view: &VoxelView<'_>) -> Option<crate::Track> {
    let g = fauna.animals.binary_search_by_key(&mother, |b| b.id).ok()?;
    if !(distance_3d(fauna, i, g, ring) <= radius_m) {
        return None;
    }
    let (a, o) = (&fauna.animals[i], &fauna.animals[g]);
    let v = view.config.voxel_m;
    let base = crate::perception::base_m(o, v);
    let grounded = o.mobility.mode == crate::MobilityMode::Ground && o.attached.is_none();
    let lift_m = if grounded {
        0.0
    } else {
        crate::flight::support_below(view, o.pose.x, o.pose.z, base).map_or(0.0, |s| (base - crate::surface_m(s.y, v)).max(0.0))
    };
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, o.pose.x, o.pose.z, ring);
    Some(crate::Track {
        known: true,
        id: o.id,
        lineage: o.founder.map_or(u8::MAX, |f| f.index() as u8),
        class: class::KIN,
        x: o.pose.x,
        z: o.pose.z,
        y_m: base,
        site: (o.site.x, o.site.y, o.site.z),
        lift_m,
        grounded: grounded && o.mobility.climb.is_none(),
        climbing: o.mobility.climb.is_some(),
        d_m: dx.hypot(dz).hypot(base - a.pose.y_m),
        tick: fauna.tick,
        heading: o.pose.heading_rad,
        ..crate::Track::default()
    })
}
