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
//!   troop's mean, with the tighter comfort `follow_m` and the weight `follow_pull`. It reads
//!   only the young's own percept record. Real infants are carried; carrying is a pose, and
//!   poses are art, so it is not here.
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
        }
    }
}

impl Nursing {
    /// Whether nursing is on at all.
    pub fn on(&self) -> bool {
        self.weaning_age_s > 0.0 && self.nurse_rate_per_s > 0.0
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
///   turn (the troop's pull is 0.5).
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
    a.mother.is_some() && table_of(fauna, i).is_some_and(|t| (a.age_ticks as f64) * DT < t.weaning_age_s)
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
    // (mother id, young index) for every dependent young, mothers grouped in id order.
    let mut young: Vec<(u64, usize)> = (0..fauna.animals.len())
        .filter(|&k| dependent(fauna, k))
        .filter_map(|k| fauna.animals[k].mother.map(|m| (m, k)))
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
    let mother = a.mother?;
    let p = fauna.percept(a.id)?;
    let fresh = 2 * fauna.config.perception.stagger_ticks();
    let track = p
        .tracks()
        .iter()
        .find(|tr| tr.class == class::KIN && tr.id == mother && fauna.tick.saturating_sub(tr.tick) <= fresh)?;
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, track.x, track.z, ring);
    let d = dx.hypot(dz);
    Some((d > t.follow_m).then(|| Pull { dx, dz, kin: 1, nearest_m: d, weight: t.follow_pull.clamp(0.0, 1.0) }))
}
