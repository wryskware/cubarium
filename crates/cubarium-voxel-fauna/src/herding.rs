//! **HERD-PULL: the group pull** (Wrysk, 2026-10-09, training plan §6 item 15: "grazers are
//! super spread out today. they should be clustered more in groups for social / protection").
//!
//! An innate pull in the heuristics of the social lineages, as calves follow the herd from
//! birth. It reads only the body's own percept record (its kin tracks, [`crate::Percept`]),
//! never a world query. The heuristic is the v3 brains' teacher (plan item 23), so the rule
//! stays simple:
//!
//! - **Comfort.** While its nearest known kin is within `comfort_m` it is left alone: it
//!   forages as its heuristic asks. No pull, no push: herd-mates compete for the same food,
//!   and that, not a rule, caps how close and how many they crowd.
//! - **The drift back.** Beyond it, its walking turn leans toward the mean point of the kin
//!   it knows: the heuristic's turn and the turn toward that point, mixed by `pull`. Its pace
//!   and its feeding are its heuristic's own, so a body walking on from a cropped patch
//!   (FORAGE-LEAVE) or following food still walks; the herd drifts together.
//! - **Feeding wins.** A body holding still to feed is never pulled; nor one its heuristic is
//!   steering out of the way of something (an escape, a blocked turn), nor one off the ground
//!   (climbing, perched, in flight). The flight, the hunker and the other reflexes after it
//!   still win.
//! - **Herd size.** With more kin known than `herd_max`, the pull is scaled by `herd_max`
//!   over their number: a body that sees a big herd is drawn to it less.
//! - **Memory.** With no kin in sense, the nearest kin of its latest sensing that had one
//!   (the record's best kin track) still draws it for `memory_s`, until it is within
//!   `comfort_m` of where that kin was: it remembers where it last saw the herd.
//! - `young_only` (a switch) pulls only a body under its lineage's adult body: a calf to its
//!   family.
//!
//! Each lineage's numbers are [`herding_of`] (`fauna.founders.<lineage>.herding.*`). The
//! frondgrazer's herd, the seedporter's troop and the loftstrider calf's family use it. The
//! Veilhound is pulled already: it rests at its den, and an idle adult walks back to its
//! breeder ([`crate::Pack::follow_radius_m`]).
//!
//! **The skill.** [`stay_with_group`] is the same pull as a whole action, outside the
//! heuristic's trigger: the "stay with the troop or family" skill a v3 hybrid brain calls
//! (plan item 23). [`group_pull`] is the reading under both.
//!
//! **Never scored.** Herding is no fitness or reward term. The readouts are diagnostics only:
//! [`crate::Fauna::herd_tally`] and [`crate::Fauna::mean_nearest_kin_m`].

use serde::{Deserialize, Serialize};

use crate::controller::Seek;
use crate::perception::class;
use crate::{Actions, Fauna, Founder, MobilityMode};

/// **A lineage's group pull** (module note): its knobs. Off with `comfort_m` 0.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Herding {
    /// A body whose nearest known kin is beyond this drifts back toward the kin it knows,
    /// metres; 0 turns the pull off.
    pub comfort_m: f64,
    /// The pull's share of its walking turn (0: its heuristic's turn, 1: straight at the
    /// kin's mean point). Fraction.
    pub pull: f64,
    /// With more kin known than this, the pull is scaled by this over their number; 0: never
    /// fades. Count.
    pub herd_max: u32,
    /// With no kin in sense, the nearest kin of its latest sensing with one draws it for this
    /// long, seconds; 0: only kin in sense.
    pub memory_s: f64,
    /// Only a body under its lineage's adult body is pulled (a calf to its family). A
    /// switch, no registry row.
    pub young_only: bool,
}

impl Default for Herding {
    /// No pull.
    fn default() -> Herding {
        Herding { comfort_m: 0.0, pull: 0.0, herd_max: 0, memory_s: 0.0, young_only: false }
    }
}

/// **Each lineage's group pull** (HERD-PULL), placeholders at the terrarium's scale, untuned:
/// - the frondgrazer: comfortable within 3 m of a herd-mate (four of its 0.75 m lengths; the
///   pack's `select_kin_m`, so a herd-mate inside it already lowers a pack's prospect on it),
///   inside the 5 m it sees a still body at; half its turn leans back beyond it; a herd of more
///   than four known draws it less; it remembers where it last saw one for a minute;
/// - the loftstrider: a calf only, within 6 m of its family (half its 12 m sight, under three
///   adult lengths), the same half turn and minute;
/// - the seedporter: a troop within 4 m (eight of its 0.5 m lengths, a quarter of its 16 m
///   sight), on the ground, the same half turn and minute.
pub fn herding_of(founder: Founder) -> Herding {
    match founder {
        Founder::Browser => Herding { comfort_m: 3.0, pull: 0.5, herd_max: 4, memory_s: 60.0, young_only: false },
        Founder::Loftstrider => Herding { comfort_m: 6.0, pull: 0.5, herd_max: 0, memory_s: 60.0, young_only: true },
        Founder::Seedporter => Herding { comfort_m: 4.0, pull: 0.5, herd_max: 0, memory_s: 60.0, young_only: false },
        _ => Herding::default(),
    }
}

/// **The group pull's reading** for one body ([`group_pull`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pull {
    /// From the body to the kin's mean point (or the remembered kin), metres on the ring.
    pub dx: f64,
    pub dz: f64,
    /// Kin in sense (0: the remembered one).
    pub kin: usize,
    /// The nearest of them (or the remembered one), metres.
    pub nearest_m: f64,
    /// The pull's weight after the herd size: `pull` × min(1, `herd_max` / kin).
    pub weight: f64,
}

/// Seconds as ticks; 0 for none.
fn ticks(s: f64) -> u64 {
    if s.is_finite() && s > 0.0 { cubarium_voxel::ticks_at_least_one(s) } else { 0 }
}

/// **The group pull's reading** (module note) for the body at `i`, on its lineage's table but
/// `comfort_m`: where the kin it knows are, when the nearest is farther than `comfort_m`.
/// `None` within it, or knowing no kin (none in sense and none remembered). `ring` is the
/// world's x circumference, metres. Reads its own record only.
///
/// SP-NURSE: a dependent young that senses its mother is pulled toward her instead, on its
/// lineage's nursing table's `follow_m` and `follow_pull` and not `comfort_m`
/// ([`crate::nurse::mother_pull`]).
pub fn group_pull(fauna: &Fauna, i: usize, ring: f64, comfort_m: f64) -> Option<Pull> {
    if let Some(follow) = crate::nurse::mother_pull(fauna, i, ring) {
        return follow;
    }
    let a = fauna.animals.get(i)?;
    let h = fauna.config.founder(a.founder?).herding;
    let p = fauna.percept(a.id)?;
    let now = fauna.tick;
    let fresh = 2 * fauna.config.perception.stagger_ticks();
    let comfort = comfort_m.max(0.0);
    let (mut sx, mut sz, mut n, mut nearest) = (0.0, 0.0, 0usize, f64::INFINITY);
    for t in p.tracks().iter().filter(|t| t.class == class::KIN && t.id != a.id && now.saturating_sub(t.tick) <= fresh) {
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, t.x, t.z, ring);
        sx += dx;
        sz += dz;
        n += 1;
        nearest = nearest.min(dx.hypot(dz));
    }
    if n > 0 {
        if nearest <= comfort {
            return None;
        }
        let fade = if h.herd_max > 0 && n > h.herd_max as usize { f64::from(h.herd_max) / n as f64 } else { 1.0 };
        return Some(Pull { dx: sx / n as f64, dz: sz / n as f64, kin: n, nearest_m: nearest, weight: h.pull.clamp(0.0, 1.0) * fade });
    }
    // Memory: where it last sensed kin, while that is recent.
    let best = p.best[usize::from(class::KIN)];
    if !best.known || best.id == a.id || now.saturating_sub(best.tick) > ticks(h.memory_s) || ticks(h.memory_s) == 0 {
        return None;
    }
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, best.x, best.z, ring);
    let d = dx.hypot(dz);
    (d > comfort).then(|| Pull { dx, dz, kin: 0, nearest_m: d, weight: h.pull.clamp(0.0, 1.0) })
}

/// The signed angle from `heading` to `want`, radians in (−π, π].
fn angle_to(heading: f64, want: f64) -> f64 {
    let d = (want - heading).rem_euclid(std::f64::consts::TAU);
    if d > std::f64::consts::PI { d - std::f64::consts::TAU } else { d }
}

/// The turn effort toward heading `want` from the body's heading: the angle over 45°,
/// clamped (the reflexes' steering rule).
fn turn_to(heading: f64, want: f64) -> f64 {
    (angle_to(heading, want) / std::f64::consts::FRAC_PI_4).clamp(-1.0, 1.0)
}

/// **The skill: stay with the troop or family** (module note) for the body at `i`, outside
/// the heuristic's trigger: with its nearest known kin beyond its lineage's `comfort_m`, a
/// walk at full forward toward the kin's mean point (turning in place when it lies more than
/// 60° off), not feeding; `None` within it, or knowing no kin. Any lineage may call it; a
/// lineage with no table is comfortable nowhere but beside its kin (`comfort_m` 0).
pub fn stay_with_group(fauna: &Fauna, i: usize, ring: f64) -> Option<Actions> {
    let a = fauna.animals.get(i)?;
    let comfort = fauna.config.founder(a.founder?).herding.comfort_m;
    let g = group_pull(fauna, i, ring, comfort)?;
    if g.dx == 0.0 && g.dz == 0.0 {
        return None;
    }
    let want = g.dx.atan2(g.dz);
    let off = angle_to(a.pose.heading_rad, want).abs() > std::f64::consts::FRAC_PI_3;
    Some(Actions { forward: if off { 0.0 } else { 1.0 }, turn: turn_to(a.pose.heading_rad, want), feed: 0.0 })
}

/// **The heuristic's drift back** (module note) at the body at `i`'s controller sample:
/// `actions` is what its heuristic and the layers before this one asked, `seek` what its
/// heuristic's answer was for. `Some` replaces them: its pace and feeding, its turn leaning
/// toward its kin by the pull's weight. `None` leaves them: within comfort, knowing no kin,
/// holding still to feed, steering out of the way of something, off the ground, a grown body
/// under a `young_only` table, or no table.
pub(crate) fn steer(fauna: &mut Fauna, i: usize, actions: Actions, seek: Seek, ring: f64) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = fauna.config.founder(founder);
    let h = phys.herding;
    // SP-NURSE: a dependent young follows its mother whatever its lineage's herd table.
    let follows = crate::nurse::dependent(fauna, i);
    if !follows && (!(h.comfort_m > 0.0) || !(h.pull > 0.0)) {
        return None;
    }
    if !follows && h.young_only && a.body >= phys.core.birth_body {
        return None;
    }
    let cropping = actions.feed > 0.5 && actions.forward <= fauna.config.teachers.forage_stay_forward;
    if cropping || seek == Seek::Obstacle || a.mobility.mode != MobilityMode::Ground || !(actions.forward > 0.0) {
        return None;
    }
    let g = group_pull(fauna, i, ring, h.comfort_m)?;
    if g.dx == 0.0 && g.dz == 0.0 || !(g.weight > 0.0) {
        return None;
    }
    let w = g.weight.clamp(0.0, 1.0);
    let toward = turn_to(a.pose.heading_rad, g.dx.atan2(g.dz));
    fauna.herd_tally[founder.index()].pulled_samples += 1;
    Some(Actions { forward: actions.forward, turn: ((1.0 - w) * actions.turn + w * toward).clamp(-1.0, 1.0), feed: actions.feed })
}

/// **The herding readout** per lineage (diagnostics only, no rule or score reads it;
/// session state, not saved).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HerdTally {
    /// Controller samples the drift back leaned a heuristic's turn.
    pub pulled_samples: u64,
    /// Herd-response samples a body asked to crop (scanning or not), and of them the samples
    /// it stood in a scan pause.
    pub crop_samples: u64,
    pub scan_samples: u64,
}

impl Fauna {
    /// The herding readout per lineage ([`HerdTally`]).
    pub fn herd_tally(&self) -> [HerdTally; Founder::COUNT] {
        self.herd_tally
    }

    /// **Herd spacing** (a diagnostic, never scored): the mean over the lineage's bodies of
    /// the distance to its nearest other body of the lineage, metres, on the ground plane with
    /// x wrapping on the ring; `None` with fewer than two. `ring` is the world's x
    /// circumference, metres.
    pub fn mean_nearest_kin_m(&self, founder: Founder, ring: f64) -> Option<f64> {
        let pts: Vec<(f64, f64)> = self.animals.iter().filter(|a| a.founder == Some(founder)).map(|a| (a.pose.x, a.pose.z)).collect();
        if pts.len() < 2 {
            return None;
        }
        let sum: f64 = pts
            .iter()
            .enumerate()
            .map(|(k, &(x, z))| {
                pts.iter()
                    .enumerate()
                    .filter(|&(j, _)| j != k)
                    .map(|(_, &(x2, z2))| {
                        let (dx, dz) = crate::pose::wrapped_delta(x, z, x2, z2, ring);
                        dx.hypot(dz)
                    })
                    .fold(f64::INFINITY, f64::min)
            })
            .sum();
        Some(sum / pts.len() as f64)
    }
}
