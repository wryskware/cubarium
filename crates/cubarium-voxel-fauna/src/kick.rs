//! **The kick** (CONTRACTS, behaviour-AI §3.3, §11.4): one state machine with one budget for
//! an aimed kick and a kick-back alike — ready, windup, contact, recovery.
//!
//! - **A request** ([`request`]) starts the windup when the kick is ready; anything else is
//!   refused. A kick-back ([`kick_back`], a bite that landed) is refused too unless the biter
//!   is in the front arc, so an adult cannot answer hounds on four sides at once.
//! - **Windup** `windup_s`: the body turns toward its target within its yaw cap, and steps
//!   only if `step_in_windup`.
//! - **Contact** at the windup's end: the target must be alive, within `reach_m` of the kick's
//!   origin (`origin_forward_fraction` of the body's length ahead of its centre, the bites'
//!   jaw convention) plus its own footprint, inside the front arc, overlapping the kick's
//!   height band (`band_low_fraction..band_high_fraction` of the kicker's height over its
//!   base), with open terrain between. It may dodge at its agility and awareness, as prey
//!   dodges a strike. A landed kick wounds it `damage × size_kicker / size_target`.
//! - **Recovery** `recovery_s`, then ready again.
//!
//! **The loftstrider kicks** (LS-DEFEND, [`kick_of`]: Wrysk's topic 6, "kicks wound"); every
//! other lineage has none (`FounderPhysiology::kick` is `None`). Its defence layer aims the
//! kick ([`crate::defend`]); a bite that lands draws the kick-back on the same budget, and
//! its lineage's reactive counter (`Counter`) never answers besides it.

use serde::{Deserialize, Serialize};

use cubarium_voxel::{DT, VoxelView};

use crate::{Animal, Fauna, Founder};

/// **A lineage's kick** (module note). Placeholders sized from §3.3 and topic 6.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Kick {
    /// Seconds from the request to contact.
    pub windup_s: f64,
    /// Seconds after contact (landed or not) before the next kick.
    pub recovery_s: f64,
    /// The kick's reach from its origin to the target's footprint, metres.
    pub reach_m: f64,
    /// The origin: this fraction of the body's length ahead of its centre.
    pub origin_forward_fraction: f64,
    /// The front arc's half-angle, degrees from the heading.
    pub arc_deg: f64,
    /// The height band, fractions of the kicker's height over its base.
    pub band_low_fraction: f64,
    pub band_high_fraction: f64,
    /// Wound per landed kick at equal size; scaled by `size_kicker / size_target`.
    pub damage: f64,
    /// Whether the body may step during the windup (a switch).
    pub step_in_windup: bool,
    /// Whether a bite that lands while the kick is ready, from inside the arc, draws a
    /// kick-back (a switch).
    pub kick_back: bool,
}

impl Default for Kick {
    fn default() -> Kick {
        Kick::LOFTSTRIDER
    }
}

impl Kick {
    /// §3.3's loftstrider kick: a 0.4 s windup, 2.5 s recovery, about 1.3 m of reach (beyond a
    /// hound's 0.84 m bite, so a hound must step inside it to bite), a front arc, the legs'
    /// band, and 0.18 at equal size — about a third off a grown hound at the size ratio 1.9
    /// (Wrysk, topic 6: kicks wound). The loftstrider's ([`kick_of`]).
    pub const LOFTSTRIDER: Kick = Kick {
        windup_s: 0.4,
        recovery_s: 2.5,
        reach_m: 1.3,
        origin_forward_fraction: 0.5,
        arc_deg: 60.0,
        band_low_fraction: 0.0,
        band_high_fraction: 0.6,
        damage: 0.18,
        step_in_windup: false,
        kick_back: true,
    };

    fn ticks(s: f64) -> u32 {
        if s.is_finite() && s > 0.0 { (s / DT).round().max(1.0) as u32 } else { 1 }
    }
}

/// **Each lineage's kick** (LS-DEFEND): the loftstrider's, none for the rest.
pub fn kick_of(founder: Founder) -> Option<Kick> {
    match founder {
        Founder::Loftstrider => Some(Kick::LOFTSTRIDER),
        _ => None,
    }
}

/// The kick's phase.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    #[default]
    Ready,
    Windup,
    Recovery,
}

/// Who asked for the kick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    #[default]
    Aimed,
    Back,
}

/// **A body's kick state** (saved: a mid-recovery save continues).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KickState {
    pub phase: Phase,
    pub ticks_left: u32,
    pub target: u64,
    pub source: Source,
}

/// Why a request was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Its lineage has no kick.
    NoKick,
    /// Winding up or recovering: one budget.
    Busy,
    /// A kick-back from outside the front arc.
    OutOfArc,
}

fn table(fauna: &Fauna, a: &Animal) -> Option<Kick> {
    a.founder.and_then(|f| fauna.config.founder(f).kick)
}

/// The bearing of `(x, z)` from `a`'s heading, radians in `[-π, π]`.
fn bearing(a: &Animal, x: f64, z: f64, ring: f64) -> f64 {
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, x, z, ring);
    let d = (dx.atan2(dz) - a.pose.heading_rad).rem_euclid(std::f64::consts::TAU);
    if d > std::f64::consts::PI { d - std::f64::consts::TAU } else { d }
}

fn ring_of(fauna: &Fauna) -> f64 {
    fauna.world_geometry.map_or(f64::INFINITY, |(r, _)| r)
}

/// Whether body `t` is in kicker `a`'s front arc.
pub fn in_arc(k: &Kick, a: &Animal, t: &Animal, ring: f64) -> bool {
    bearing(a, t.pose.x, t.pose.z, ring).abs() <= k.arc_deg.to_radians() + 1e-12
}

/// **Request a kick** by body `i` at body `target` ([`Source::Aimed`] tracks it through the
/// windup; [`Source::Back`] only from inside the arc).
pub fn request(fauna: &mut Fauna, i: usize, target: u64, source: Source) -> Result<(), Refused> {
    let a = &fauna.animals[i];
    let Some(k) = table(fauna, a) else { return Err(Refused::NoKick) };
    if a.kick.phase != Phase::Ready {
        return Err(Refused::Busy);
    }
    if source == Source::Back {
        let ring = ring_of(fauna);
        let inside = fauna.animals.binary_search_by_key(&target, |o| o.id).ok().is_some_and(|j| in_arc(&k, a, &fauna.animals[j], ring));
        if !inside {
            return Err(Refused::OutOfArc);
        }
    }
    fauna.animals[i].kick = KickState { phase: Phase::Windup, ticks_left: Kick::ticks(k.windup_s), target, source };
    Ok(())
}

/// **A bite landed** on body `bitten` by `biter`: the kick-back hook. With its lineage's
/// `kick_back` on, the kick ready and the biter in the arc, the one kick winds up at it.
pub(crate) fn kick_back(fauna: &mut Fauna, bitten: u64, biter: u64) -> bool {
    let Ok(i) = fauna.animals.binary_search_by_key(&bitten, |a| a.id) else { return false };
    if !table(fauna, &fauna.animals[i]).is_some_and(|k| k.kick_back) {
        return false;
    }
    let ok = request(fauna, i, biter, Source::Back).is_ok();
    fauna.ledger.kick_backs += u64::from(ok);
    ok
}

/// Whether kicker `a`'s kick reaches body `t` now (module note's contact, without the dodge).
pub fn lands(fauna: &Fauna, view: &VoxelView<'_>, k: &Kick, a: &Animal, t: &Animal) -> bool {
    let (Some(ag), Some(tg)) = (crate::body_of(&fauna.config, a), crate::body_of(&fauna.config, t)) else { return false };
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    if !in_arc(k, a, t, ring) {
        return false;
    }
    let origin = crate::step::jaw_of(a.pose.x, a.pose.z, a.pose.heading_rad, ag.length_m, k.origin_forward_fraction);
    let (dx, dz) = crate::pose::wrapped_delta(origin.0, origin.1, t.pose.x, t.pose.z, ring);
    if (dx * dx + dz * dz).sqrt() > k.reach_m + tg.footprint_radius() {
        return false;
    }
    let base_a = crate::perception::base_m(a, v);
    let base_t = crate::perception::base_m(t, v);
    let (lo, hi) = (base_a + k.band_low_fraction * ag.height_m, base_a + k.band_high_fraction * ag.height_m);
    if base_t > hi || base_t + tg.height_m < lo {
        return false;
    }
    let mid = 0.5 * (lo.max(base_t) + hi.min(base_t + tg.height_m));
    crate::perception::line_clear(view, a.id, (origin.0, mid, origin.1), (t.pose.x, mid, t.pose.z))
}

/// A landed kick: who, on whom, how hard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct KickHit {
    pub target: u64,
    pub damage: f64,
    pub by: Founder,
}

/// **One tick of every kick** (after the act, before the strikes land): windups turn the body
/// toward its target and count down, contacts resolve, recoveries count down. The landed
/// kicks, for the step to wound.
pub(crate) fn advance(fauna: &mut Fauna, view: &VoxelView<'_>, seed: u64) -> Vec<KickHit> {
    let mut out = Vec::new();
    if !Founder::ALL.iter().any(|&f| fauna.config.founder(f).kick.is_some()) {
        return out;
    }
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let tick = fauna.tick;
    for i in 0..fauna.animals.len() {
        let a = fauna.animals[i];
        let Some(k) = table(fauna, &a) else { continue };
        let mut s = a.kick;
        match s.phase {
            Phase::Ready => continue,
            Phase::Recovery => {
                s.ticks_left = s.ticks_left.saturating_sub(1);
                if s.ticks_left == 0 {
                    s = KickState::default();
                }
            }
            Phase::Windup => {
                let target = fauna.animals.binary_search_by_key(&s.target, |o| o.id).ok().map(|j| fauna.animals[j]);
                // Tracking: turn toward the target within the yaw cap.
                if let (Some(t), Some(f)) = (target, a.founder) {
                    let cap = fauna.config.founder(f).yaw_cap_rad_per_s * DT;
                    let b = bearing(&a, t.pose.x, t.pose.z, ring);
                    fauna.animals[i].pose.heading_rad = (a.pose.heading_rad + b.clamp(-cap, cap)).rem_euclid(std::f64::consts::TAU);
                }
                if !k.step_in_windup {
                    fauna.animals[i].founder_state.held.forward = 0.0;
                }
                s.ticks_left = s.ticks_left.saturating_sub(1);
                if s.ticks_left == 0 {
                    let a = fauna.animals[i];
                    if let Some(t) = target.filter(|t| lands(fauna, view, &k, &a, t)) {
                        let tf = t.founder;
                        let pp = tf.map(|f| *fauna.config.founder(f));
                        let chance = pp.map_or(0.0, |pp| {
                            let (dx, dz) = crate::pose::wrapped_delta(t.pose.x, t.pose.z, a.pose.x, a.pose.z, ring);
                            let (fx, fz) = t.pose.forward();
                            let from_rear = dx * fx + dz * fz < 0.0;
                            let cap = pp.core.reserve_of(t.body);
                            let condition = if cap > 0.0 { t.reserve / cap } else { 0.0 };
                            let awareness = crate::vigil::awareness(&fauna.config, &t, fauna.percept(t.id), a.id, tick);
                            crate::dodge_chance_with(&fauna.config.predation, pp.agility, awareness, from_rear, condition)
                        });
                        let dodged = crate::dodges(chance, seed ^ 0x6b69_636b, a.id, t.id, tick);
                        let by = a.founder.expect("a kicker has a lineage");
                        let size = |b: &Animal| b.founder.map_or(0.0, |f| fauna.config.founder(f).size(b.body));
                        let (sa, st) = (size(&a), size(&t));
                        let damage = if st > 0.0 { k.damage * sa / st } else { 0.0 };
                        if !dodged && damage.is_finite() && damage > 0.0 {
                            out.push(KickHit { target: t.id, damage, by });
                        }
                    }
                    s = KickState { phase: Phase::Recovery, ticks_left: Kick::ticks(k.recovery_s), ..s };
                }
            }
        }
        fauna.animals[i].kick = s;
    }
    out
}
