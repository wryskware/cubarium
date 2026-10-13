//! **LS-DEFEND: calves run to their mothers, adults bunch, face out and kick**
//! (`design/behaviour-ai-2026-10-11.md` package 4: §0 rows 6–8, §3.1–§3.3, §3.5, §11.4–§11.6;
//! the brief `design/handoffs/ls-defend-2026-10-12.md`).
//!
//! The loftstrider's **intent layer under threat** ([`steer`]). With [`Defence::enabled`] (the
//! loftstrider's default; Wrysk's decided behaviour, §0 rows 6, 7 and 8) it runs in the write
//! side after the ground response ([`crate::vigil`]) and, while it holds a defence intent,
//! replaces it (`Steer::Defend`). Today's "an adult flees two or more" (`group_min`) is
//! retired for it: a grown body, and a young one that knows an adult, never starts a flight;
//! a lone young one still runs as before (it has no one to run to).
//!
//! # The menu (§3.1)
//!
//! Each controller sample the body scores the loftstrider menu's threat-time intents from
//! its own percept record and the calls it heeds, takes the best with the shared hold
//! ([`crate::intent::arbitrate`]: `hold_s`, then `switch_margin` on probabilities at the
//! lineage's temperature) and runs its skill. Its proposal and held intent go to the intent
//! record ([`crate::intent::Teach`]): the brains' teachers later.
//! - **watch** (all): a threat seen within `threat_m`, still or beyond reach: stand, face it.
//! - **move off** (all): a threat approaching within `threat_m` but beyond `guard_m` of it and
//!   of every calf it senses: walk away from it at cruise, leaning toward its kin.
//! - **go to adults** (young): a pressing threat sensed, or an alarm or distress call heeded,
//!   and an adult known: run to the far side of its mother (when reachable, §3.5) or the
//!   nearest reachable adult, and settle there.
//! - **go to calf** (adult): a distress call heeded, or a calf it senses nearer a pressing
//!   threat than any adult: run to the guard point between the calf and the threat; its own
//!   calf weighs `mother_weight` more (topic 7).
//! - **guard** (adult): a pressing threat within `guard_m` of it or of the calf it stands by:
//!   hold the guard point facing its hound, kick a hound in reach, charge one that comes
//!   close and return. The aimed kick is guard's alone (§3.3; `kick_any_intent` widens it).
//! - The calm intents (**graze**, **stay with family**, **rest**) are today's skills (the
//!   forager, the herd pull, the rhythm): with none of the above scored, [`steer`] returns
//!   `None` and the rules below it run as before.
//!
//! A hound that looks young (a pup: it cannot strike) is no threat the menu meets unless
//! `meet_young` (VH-KICKED).
//!
//! # What it reads (§2.1)
//!
//! Its own body, its own percept record (threat and kin tracks, with what sight shows of
//! them: size, heading, posture, stance), the calls it heeds ([`crate::calls::heeded`]) and,
//! for the bond, its recognition of its own mother and calf (§3.5). Never another body's
//! record, a true position or a group record.
//!
//! **Never scored.** No fitness or reward term pays for defending (§9).

use serde::{Deserialize, Serialize};

use cubarium_voxel::{DT, VoxelView};

use crate::controller::Actions;
use crate::intent::{LOFTSTRIDER, PlaceKind, Subject, Teach};
use crate::perception::{SIGHT, Track, class};
use crate::vigil::Threat;
use crate::{Animal, Fauna, Founder};

/// **A lineage's defence** (module note): its knobs. Placeholders at the terrarium's scale,
/// untuned: the topic-8 target is reached by turning them, not asserted.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Defence {
    /// The layer runs (a switch, no registry row).
    pub enabled: bool,
    /// A threat sensed within this engages it (watch, move off; a calf runs to its adults),
    /// metres.
    pub threat_m: f64,
    /// A pressing threat within this of an adult, or of a calf it stands by, is met: guard,
    /// metres.
    pub guard_m: f64,
    /// The guard point stands this beyond the calf's and the adult's footprints, toward the
    /// hound; the calf's point this beyond the adult's and its own, away from it. Metres.
    pub guard_gap_m: f64,
    /// At a point within this, it has arrived, metres.
    pub arrive_m: f64,
    /// A hound within this of the guard point, outside the kick's reach, is charged while the
    /// kick is ready, metres; 0 never charges.
    pub charge_m: f64,
    /// ...and the charge turns back this far from the guard point, metres.
    pub leash_m: f64,
    /// A grown body with no calf to stand by keeps within this of its nearest grown kin,
    /// metres (adults bunch).
    pub bunch_m: f64,
    /// A calf's way to an adult is unsafe when a pressing threat stands within this of it,
    /// metres.
    pub cross_m: f64,
    /// A grown kin seen heading within this of a hound faces it, degrees.
    pub facing_deg: f64,
    /// A mother's weight on her own calf: its `go_to_calf` score × this.
    pub mother_weight: f64,
    /// The intents' scores (softmax logits at the intent table's temperature): the calm
    /// intents' (graze, stay with family, rest: today's rules), watch, move off, go to calf,
    /// guard, go to adults.
    pub w_calm: f64,
    pub w_watch: f64,
    pub w_move_off: f64,
    pub w_go_to_calf: f64,
    pub w_guard: f64,
    pub w_go_to_adults: f64,
    /// VH-KICKED: whether a hound that **looks young** (its sensed length under its lineage's
    /// `intent.adult_fraction` of the grown body, [`looks_young`]) is a threat the defence meets.
    /// Off (the loftstrider's): such a hound is no threat to the menu — no calf runs from it, no
    /// adult goes to a calf over it, guards against it, charges it or aims a kick at it. A
    /// Veilhound pup cannot strike (R3) and stays at its den, and a landed kick's size ratio
    /// kills it at once; before this a family grazing by a den charged and kicked the litter
    /// dead. On: every hound is met, as before.
    pub meet_young: bool,
    /// VH-KICKED: whether the aimed kick goes from any defence intent it holds. Off (the
    /// loftstrider's, §3.3 "the kick is an aimed action of an adult in **guard**"): only while
    /// it holds guard; a kick-back answers a bite in any intent. On: from any defence intent
    /// (watch, move off, go to calf too), as LS-DEFEND shipped it — a body running to its calf
    /// kicked a hound walking home past it.
    pub kick_any_intent: bool,
}

impl Default for Defence {
    /// Off.
    fn default() -> Defence {
        Defence { enabled: false, ..Defence::LOFTSTRIDER }
    }
}

impl Defence {
    /// §3.1's loftstrider, on (Wrysk's decided behaviour). Placeholders:
    /// - it engages a threat within its 12 m day sight and meets one within 6 m (its old flee
    ///   start, about three of its lengths);
    /// - guard and calf points 0.25 m clear of the two footprints (shoulder to shoulder),
    ///   arrival within 0.75 m;
    /// - it charges a hound within 4 m of its post, at most 2.5 m out (a short rush and back,
    ///   as cows and bulls charge wolves, *eth §1.3*);
    /// - adults keep within 4 m of each other, under two of their lengths;
    /// - a calf's way is unsafe with a hound within 2.5 m of it (a rush's last bound);
    /// - a grown kin heading within 30° of a hound faces it;
    /// - a mother weighs her own calf 1.5 × another's (allomothers help, mothers most:
    ///   *eth §1.1–1.2*);
    /// - scores: calm 1, watch 1.5, move off 2, go to calf 2.5, guard 3, go to adults 3.
    pub const LOFTSTRIDER: Defence = Defence {
        enabled: true,
        threat_m: 12.0,
        guard_m: 6.0,
        guard_gap_m: 0.25,
        arrive_m: 0.75,
        charge_m: 4.0,
        leash_m: 2.5,
        bunch_m: 4.0,
        cross_m: 2.5,
        facing_deg: 30.0,
        mother_weight: 1.5,
        w_calm: 1.0,
        w_watch: 1.5,
        w_move_off: 2.0,
        w_go_to_calf: 2.5,
        w_guard: 3.0,
        w_go_to_adults: 3.0,
        meet_young: false,
        kick_any_intent: false,
    };
}

/// Each lineage's defence: the loftstrider's on, the rest off.
pub fn defence_of(founder: Founder) -> Defence {
    match founder {
        Founder::Loftstrider => Defence::LOFTSTRIDER,
        _ => Defence::default(),
    }
}

/// Whether lineage `founder` runs the defence layer.
pub(crate) fn on(config: &crate::FaunaConfig, founder: Founder) -> bool {
    config.founder(founder).defence.enabled
}

// ------------------------------------------------------------------ geometry

fn gap(ax: f64, az: f64, bx: f64, bz: f64, ring: f64) -> (f64, f64, f64) {
    let (dx, dz) = crate::pose::wrapped_delta(ax, az, bx, bz, ring);
    (dx, dz, dx.hypot(dz))
}

fn on_ring(x: f64, ring: f64) -> f64 {
    if ring.is_finite() && ring > 0.0 { x.rem_euclid(ring) } else { x }
}

fn angle_between(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(std::f64::consts::TAU);
    if d > std::f64::consts::PI { std::f64::consts::TAU - d } else { d }
}

/// The point `m` metres from `(ax, az)` along the way to `(bx, bz)` (negative: away).
fn along(ax: f64, az: f64, bx: f64, bz: f64, m: f64, ring: f64) -> (f64, f64) {
    let (dx, dz, d) = gap(ax, az, bx, bz, ring);
    if d <= 1e-9 {
        return (ax, az);
    }
    (on_ring(ax + dx / d * m, ring), az + dz / d * m)
}

/// The distance from `p` to the segment `a`–`b`, metres.
fn to_segment(a: (f64, f64), b: (f64, f64), p: (f64, f64), ring: f64) -> f64 {
    let (bx, bz, _) = gap(a.0, a.1, b.0, b.1, ring);
    let (px, pz, _) = gap(a.0, a.1, p.0, p.1, ring);
    let l2 = bx * bx + bz * bz;
    let t = if l2 > 1e-12 { ((px * bx + pz * bz) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (px - t * bx).hypot(pz - t * bz)
}

/// Stand turning toward `(x, z)`.
fn face(a: &Animal, x: f64, z: f64, ring: f64) -> Actions {
    let (dx, dz, _) = gap(a.pose.x, a.pose.z, x, z, ring);
    let mut act = crate::vigil::head(a, dx.atan2(dz), 0.0, 0.0);
    act.forward = 0.0;
    act
}

/// Walk toward `(x, z)` at `forward` (turning in place when it lies far off), not feeding.
fn walk(a: &Animal, x: f64, z: f64, forward: f64, ring: f64) -> Actions {
    let (dx, dz, _) = gap(a.pose.x, a.pose.z, x, z, ring);
    crate::vigil::head(a, dx.atan2(dz), forward, 0.0)
}

// ------------------------------------------------------------------ what it senses

/// Whether a track looks young for the menu: shorter than an adult of its lineage at the
/// intent table's `adult_fraction` of the adult body (length goes as the cube root).
pub(crate) fn looks_young(config: &crate::FaunaConfig, t: &Track) -> bool {
    let Some(&f) = Founder::ALL.get(usize::from(t.lineage)) else { return false };
    let p = config.founder(f);
    t.size_m < p.intent.adult_fraction.clamp(0.0, 1.0).cbrt() * p.adult_length_m * (1.0 - 1e-9)
}

/// The footprint radius a track's size stands for (its lineage's body at that length).
fn radius_est(config: &crate::FaunaConfig, t: &Track) -> f64 {
    let Some(&f) = Founder::ALL.get(usize::from(t.lineage)) else { return 0.5 * t.size_m };
    let p = config.founder(f);
    let r = if p.adult_length_m > 0.0 { (t.size_m / p.adult_length_m).clamp(0.0, 1.0) } else { 1.0 };
    p.body_at(p.core.body_max * r * r * r).footprint_radius()
}

/// What one body senses at a sample.
pub(crate) struct Scene {
    pub(crate) me: Animal,
    pub(crate) young: bool,
    pub(crate) radius: f64,
    pub(crate) length: f64,
    /// Threats in its record, nearest first ([`crate::vigil::threats`]); pressing ones are
    /// approaching (seen moving or near, heard or felt), not watched.
    pub(crate) threats: Vec<Threat>,
    /// Its own lineage's kin tracks, fresh: grown and young apart.
    pub(crate) adults: Vec<Track>,
    pub(crate) calves: Vec<Track>,
    /// The call it heeds, if any (kind 0 an alarm, 1 a distress call).
    pub(crate) call: Option<crate::calls::CallRecord>,
    /// Its mother, when it senses her ([`crate::nurse::mother_track`]), and the calves it
    /// senses that it knows as its own (recognition, §3.5).
    pub(crate) mother: Option<u64>,
    pub(crate) own: Vec<u64>,
    /// Its [`Defence::meet_young`]: whether a young-looking hound is a threat it meets.
    pub(crate) meet_young: bool,
}

/// **The scene** of body `i` (module note): read from its own record and calls only.
pub(crate) fn scene(fauna: &Fauna, i: usize, view: &VoxelView<'_>) -> Option<Scene> {
    let a = fauna.animals[i];
    let f = a.founder?;
    let phys = fauna.config.founder(f);
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let now = fauna.tick;
    let p = fauna.percept(a.id).copied().unwrap_or_default();
    let meet_young = phys.defence.meet_young;
    let threats: Vec<Threat> = crate::vigil::threats(&fauna.config, &a, &p, now, ring, view.config.voxel_m)
        .into_iter()
        .filter(|t| meet_young || !p.track(t.id).is_some_and(|k| looks_young(&fauna.config, k)))
        .collect();
    let fresh = 2 * fauna.config.perception.stagger_ticks();
    let kin: Vec<Track> = p
        .tracks()
        .iter()
        .filter(|t| t.known && t.class == class::KIN && t.id != a.id && t.lineage == f.index() as u8 && now.saturating_sub(t.tick) <= fresh)
        .copied()
        .collect();
    let (calves, adults): (Vec<Track>, Vec<Track>) = kin.into_iter().partition(|t| looks_young(&fauna.config, t));
    let (call, _) = crate::calls::heeded(fauna, i, p.vigil.heard_from);
    let body = phys.body_at(a.body);
    let mother = crate::nurse::mother_track(fauna, i).map(|t| t.id);
    let own = calves
        .iter()
        .filter(|c| fauna.animals.binary_search_by_key(&c.id, |o| o.id).ok().is_some_and(|j| crate::mother_body(&fauna.animals[j]) == Some(a.id)))
        .map(|c| c.id)
        .collect();
    Some(Scene { me: a, young: crate::intent::young(fauna, &a, f), radius: body.footprint_radius(), length: body.length_m, threats, adults, calves, call, mother, own, meet_young })
}

/// Whether lineage `f`'s body `a` knows a grown kin (a fresh kin track that looks grown):
/// a young one that does runs to it, never off on its own.
pub(crate) fn knows_adult(fauna: &Fauna, a: &Animal) -> bool {
    let Some(f) = a.founder else { return false };
    let now = fauna.tick;
    let fresh = 2 * fauna.config.perception.stagger_ticks();
    fauna.percept(a.id).is_some_and(|p| {
        p.tracks().iter().any(|t| {
            t.known && t.class == class::KIN && t.id != a.id && t.lineage == f.index() as u8 && now.saturating_sub(t.tick) <= fresh && !looks_young(&fauna.config, t)
        })
    })
}

// ------------------------------------------------------------------ the plans

/// A defence intent's plan for this sample: its score, subject, actions and kick target.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Plan {
    pub(crate) name: &'static str,
    pub(crate) score: f64,
    pub(crate) subject: Subject,
    pub(crate) act: Actions,
}

fn threat_subject(t: &Threat, now: u64) -> Subject {
    Subject::Body { id: t.id, seen_tick: now, at: None }
}

fn track_subject(t: &Track) -> Subject {
    Subject::of_track(t)
}

/// **The calf point** (§3.2): the far side of adult `k` from threat `(hx, hz)` (none: the
/// adult itself), clear of both footprints by `guard_gap_m`.
fn calf_point(config: &crate::FaunaConfig, d: &Defence, s: &Scene, k: &Track, h: Option<(f64, f64)>, ring: f64) -> (f64, f64) {
    match h {
        Some((hx, hz)) => along(k.x, k.z, hx, hz, -(radius_est(config, k) + s.radius + d.guard_gap_m), ring),
        None => (k.x, k.z),
    }
}

/// The nearest pressing threat to `(x, z)` in the scene.
fn nearest_pressing(s: &Scene, x: f64, z: f64, ring: f64) -> Option<(Threat, f64)> {
    s.threats.iter().filter(|t| t.pressing).map(|t| (*t, gap(x, z, t.x, t.z, ring).2)).min_by(|p, q| p.1.total_cmp(&q.1).then(p.0.id.cmp(&q.0.id)))
}

/// **Go to adults** (young, §3.1): run to the far side of an adult from the threat it fears
/// and settle there. The adult: its mother first, the nearest after.
fn go_to_adults(fauna: &Fauna, view: &VoxelView<'_>, d: &Defence, s: &Scene, ring: f64) -> Option<Plan> {
    if !s.young || s.adults.is_empty() {
        return None;
    }
    let me = &s.me;
    let near = nearest_pressing(s, me.pose.x, me.pose.z, ring);
    let threatened = near.is_some_and(|(_, dd)| dd <= d.threat_m) || s.call.is_some();
    if !threatened {
        return None;
    }
    // The bond (§3.5, topic 7): its mother first, when it senses her; the nearest after.
    let mother = s.mother;
    let mut order: Vec<&Track> = s.adults.iter().collect();
    order.sort_by(|p, q| {
        (Some(q.id) == mother).cmp(&(Some(p.id) == mother))
            .then(gap(me.pose.x, me.pose.z, p.x, p.z, ring).2.total_cmp(&gap(me.pose.x, me.pose.z, q.x, q.z, ring).2))
            .then(p.id.cmp(&q.id))
    });
    // Each adult's far point; the first reachable and safe one (§11.5), else the first.
    let point = |k: &Track| {
        let h = nearest_pressing(s, k.x, k.z, ring).map(|(t, _)| (t.x, t.z));
        calf_point(&fauna.config, d, s, k, h, ring)
    };
    let ok = |k: &Track| {
        let p = point(k);
        reachable(fauna, view, me, p, ring) && segment_safe(s, (me.pose.x, me.pose.z), p, d.cross_m, ring)
    };
    let k = *order.iter().find(|k| ok(k)).or(order.first())?;
    let (tx, tz) = point(k);
    let (_, _, dist) = gap(me.pose.x, me.pose.z, tx, tz, ring);
    let act = if dist <= d.arrive_m { Actions { forward: 0.0, turn: 0.0, feed: 0.0 } } else { walk(me, tx, tz, 1.0, ring) };
    Some(Plan { name: "go_to_adults", score: d.w_go_to_adults, subject: track_subject(k), act })
}

/// The calf an adult goes to or stands by, if any: one it senses nearer a pressing threat
/// than any grown body it knows (itself included), or one whose distress call it heeds.
/// Returns where it is, its body (when sensed) and the threat on it.
struct CalfTarget {
    x: f64,
    z: f64,
    calf: Option<Track>,
    threat: Option<Threat>,
    /// Its own calf (topic 7: a mother guards hers hardest).
    mine: bool,
}

fn calf_target(d: &Defence, s: &Scene, ring: f64) -> Option<CalfTarget> {
    let me = &s.me;
    // Its own calf first, then the one nearest its threat.
    let mut best: Option<(CalfTarget, (bool, f64))> = None;
    for c in &s.calves {
        let Some((h, dch)) = nearest_pressing(s, c.x, c.z, ring) else { continue };
        if dch > d.threat_m {
            continue;
        }
        let adult_d = s.adults.iter().map(|k| gap(k.x, k.z, h.x, h.z, ring).2).fold(gap(me.pose.x, me.pose.z, h.x, h.z, ring).2, f64::min);
        let mine = s.own.contains(&c.id);
        let key = (!mine, dch);
        if dch < adult_d && best.as_ref().is_none_or(|(_, b)| key.0 < b.0 || (key.0 == b.0 && key.1 < b.1)) {
            best = Some((CalfTarget { x: c.x, z: c.z, calf: Some(*c), threat: Some(h), mine }, key));
        }
    }
    if let Some((t, _)) = best {
        return Some(t);
    }
    // A distress call heeded: where it came from, the calf there if it senses it; its own
    // when it knows the voice (a recognised call is heeded first, `calls::heeded`).
    let r = s.call.filter(|r| r.kind == 1)?;
    let calf = s.calves.iter().filter(|c| gap(c.x, c.z, r.x, r.z, ring).2 <= d.cross_m.max(d.arrive_m)).min_by(|p, q| gap(p.x, p.z, r.x, r.z, ring).2.total_cmp(&gap(q.x, q.z, r.x, r.z, ring).2)).copied();
    let (x, z) = calf.map_or((r.x, r.z), |c| (c.x, c.z));
    let threat = nearest_pressing(s, x, z, ring).filter(|(_, dd)| *dd <= d.threat_m).map(|(t, _)| t);
    let mine = r.relation == 2 || calf.is_some_and(|c| s.own.contains(&c.id));
    Some(CalfTarget { x, z, calf, threat, mine })
}

/// **The guard point** between a calf at `(cx, cz)` (footprint `rc`) and a threat at
/// `(hx, hz)`: the adult's footprint and the calf's apart, plus `guard_gap_m`, toward it.
fn guard_point(d: &Defence, s: &Scene, (cx, cz): (f64, f64), rc: f64, h: Option<(f64, f64)>, ring: f64) -> (f64, f64) {
    match h {
        Some((hx, hz)) => along(cx, cz, hx, hz, rc + s.radius + d.guard_gap_m, ring),
        None => (cx, cz),
    }
}

/// **Go to calf** (adult, §3.1): run to the guard point between the calf and its threat.
fn go_to_calf(fauna: &Fauna, d: &Defence, s: &Scene, ring: f64) -> Option<Plan> {
    if s.young {
        return None;
    }
    let me = &s.me;
    let t = calf_target(d, s, ring)?;
    let rc = t.calf.map_or(0.0, |c| radius_est(&fauna.config, &c));
    let (gx, gz) = guard_point(d, s, (t.x, t.z), rc, t.threat.map(|h| (h.x, h.z)), ring);
    let (_, _, dist) = gap(me.pose.x, me.pose.z, gx, gz, ring);
    if dist <= d.arrive_m + if t.threat.is_some() { 0.0 } else { rc + s.radius } {
        return None;
    }
    let subject = t.calf.map_or(Subject::Place { kind: PlaceKind::GuardPoint, x: gx, z: gz, y_m: me.pose.y_m, seen_tick: fauna.tick }, |c| track_subject(&c));
    let score = d.w_go_to_calf * if t.mine { d.mother_weight } else { 1.0 };
    Some(Plan { name: "go_to_calf", score, subject, act: walk(me, gx, gz, 1.0, ring) })
}

/// **Guard** (adult, §3.1): a pressing threat within `guard_m` of it, or of the calf it
/// stands by (the nearest it senses within `guard_m`): hold the guard point facing it.
fn guard(fauna: &Fauna, view: &VoxelView<'_>, d: &Defence, s: &Scene, ring: f64) -> Option<Plan> {
    if s.young {
        return None;
    }
    let me = &s.me;
    // The calf it stands by: its own first (topic 7), then the nearest, within `guard_m`.
    let calf = s
        .calves
        .iter()
        .map(|c| (c, gap(me.pose.x, me.pose.z, c.x, c.z, ring).2))
        .filter(|(_, dd)| *dd <= d.guard_m)
        .min_by(|p, q| (!s.own.contains(&p.0.id)).cmp(&!s.own.contains(&q.0.id)).then(p.1.total_cmp(&q.1)).then(p.0.id.cmp(&q.0.id)))
        .map(|(c, _)| *c);
    let near = |t: &Threat| t.pressing && (t.d <= d.guard_m || calf.is_some_and(|c| gap(c.x, c.z, t.x, t.z, ring).2 <= d.guard_m));
    let (rx, rz) = calf.map_or((me.pose.x, me.pose.z), |c| (c.x, c.z));
    let mut hounds: Vec<&Threat> = s.threats.iter().filter(|t| near(t)).collect();
    hounds.sort_by(|p, q| gap(rx, rz, p.x, p.z, ring).2.total_cmp(&gap(rx, rz, q.x, q.z, ring).2).then(p.id.cmp(&q.id)));
    // Sector spread (§11.5): a hound that a grown kin is seen facing, and stands nearer to
    // than it does, is that kin's; it faces the nearest one no other adult faces.
    let taken = |h: &Threat| {
        s.adults.iter().any(|k| {
            let dk = gap(k.x, k.z, h.x, h.z, ring).2;
            faces(k, h.x, h.z, d.facing_deg, ring) && (dk < h.d || (dk == h.d && k.id < me.id))
        })
    };
    let h = *hounds.iter().find(|h| !taken(h)).or(hounds.first())?;
    let (gx, gz) = match calf {
        Some(c) => {
            // Between the calf and its hound, never past the hound; where it cannot walk, it
            // stands where it is.
            let rc = radius_est(&fauna.config, &c);
            let room = gap(c.x, c.z, h.x, h.z, ring).2 - s.radius;
            let g = along(c.x, c.z, h.x, h.z, (rc + s.radius + d.guard_gap_m).min(room.max(0.0)), ring);
            if reachable(fauna, view, me, g, ring) { g } else { (me.pose.x, me.pose.z) }
        }
        None => bunch_point(d, s, ring).filter(|g| reachable(fauna, view, me, *g, ring)).unwrap_or((me.pose.x, me.pose.z)),
    };
    let (_, _, off) = gap(me.pose.x, me.pose.z, gx, gz, ring);
    // Charge and return (§3.1): with its kick ready, a hound that comes within `charge_m` of
    // its post but out of the kick's reach is charged, at most `leash_m` out; then back.
    let ready = me.kick.phase == crate::kick::Phase::Ready && me.founder.is_some_and(|f| fauna.config.founder(f).kick.is_some());
    let charge = ready && d.charge_m > 0.0 && gap(gx, gz, h.x, h.z, ring).2 <= d.charge_m && off <= d.leash_m && !in_kick_reach(fauna, s, h, ring);
    let act = if charge {
        walk(me, h.x, h.z, 1.0, ring)
    } else if off > d.arrive_m {
        walk(me, gx, gz, 1.0, ring)
    } else {
        face(me, h.x, h.z, ring)
    };
    Some(Plan { name: "guard", score: d.w_guard, subject: threat_subject(h, fauna.tick), act })
}

/// **Adults bunch** (§3.1): a grown body with no calf to stand by keeps within `bunch_m` of its
/// nearest grown kin: the point `bunch_m / 2` from that kin toward it, when farther.
fn bunch_point(d: &Defence, s: &Scene, ring: f64) -> Option<(f64, f64)> {
    let me = &s.me;
    let (k, dk) = s.adults.iter().map(|k| (k, gap(me.pose.x, me.pose.z, k.x, k.z, ring).2)).min_by(|p, q| p.1.total_cmp(&q.1).then(p.0.id.cmp(&q.0.id)))?;
    (dk > d.bunch_m).then(|| along(k.x, k.z, me.pose.x, me.pose.z, 0.5 * d.bunch_m, ring))
}

/// **Watch** (§3.1): a threat seen within `threat_m`: stand, face it.
fn watch(fauna: &Fauna, d: &Defence, s: &Scene, ring: f64) -> Option<Plan> {
    let t = s.threats.iter().find(|t| !t.calm && t.seen && t.d <= d.threat_m)?;
    Some(Plan { name: "watch", score: d.w_watch, subject: threat_subject(t, fauna.tick), act: face(&s.me, t.x, t.z, ring) })
}

/// **Move off** (§3.1): a threat approaching within `threat_m`, beyond `guard_m` of it and of
/// every calf it senses: walk away at cruise, leaning toward the kin it knows.
fn move_off(fauna: &Fauna, d: &Defence, s: &Scene, ring: f64) -> Option<Plan> {
    let me = &s.me;
    let (h, dh) = nearest_pressing(s, me.pose.x, me.pose.z, ring)?;
    if dh > d.threat_m || dh <= d.guard_m {
        return None;
    }
    if s.calves.iter().any(|c| s.threats.iter().any(|t| t.pressing && gap(c.x, c.z, t.x, t.z, ring).2 <= d.guard_m)) {
        return None;
    }
    let (ax, az, ad) = gap(h.x, h.z, me.pose.x, me.pose.z, ring);
    let (mut wx, mut wz) = if ad > 1e-9 { (ax / ad, az / ad) } else { (me.pose.heading_rad.sin(), me.pose.heading_rad.cos()) };
    let kin: Vec<&Track> = s.adults.iter().chain(s.calves.iter()).collect();
    if !kin.is_empty() {
        let (mut kx, mut kz) = (0.0, 0.0);
        for k in &kin {
            let (dx, dz, _) = gap(me.pose.x, me.pose.z, k.x, k.z, ring);
            kx += dx;
            kz += dz;
        }
        let (kx, kz) = (kx / kin.len() as f64, kz / kin.len() as f64);
        let kd = kx.hypot(kz);
        if kd > d.bunch_m {
            wx += 0.5 * kx / kd;
            wz += 0.5 * kz / kd;
        }
    }
    let act = crate::vigil::head(me, wx.atan2(wz), 1.0, 0.0);
    Some(Plan { name: "move_off", score: d.w_move_off, subject: threat_subject(&h, fauna.tick), act })
}

/// Every defence plan scored this sample.
pub(crate) fn plans(fauna: &Fauna, view: &VoxelView<'_>, d: &Defence, s: &Scene, ring: f64) -> Vec<Plan> {
    // A young body that knows no adult has no one to run to: its flight stands.
    if s.young && s.adults.is_empty() {
        return Vec::new();
    }
    [go_to_adults(fauna, view, d, s, ring), go_to_calf(fauna, d, s, ring), guard(fauna, view, d, s, ring), move_off(fauna, d, s, ring), watch(fauna, d, s, ring)]
        .into_iter()
        .flatten()
        .collect()
}

// ------------------------------------------------------------------ the layer

/// The loftstrider menu's index of intent `name`.
fn ix(name: &str) -> usize {
    LOFTSTRIDER.index(name).expect("a loftstrider intent")
}

/// Whether menu intent `k` is a calm one (today's rules run it).
fn calm(k: usize) -> bool {
    matches!(LOFTSTRIDER.name(k), "graze" | "stay_with_family" | "rest")
}

/// **The defence layer** (module note) for body `i`, after its ground response: `Some` holds
/// a defence intent and replaces the actions; `None` leaves them (a calm intent held). Its
/// teacher labels go to [`Fauna::ls_teach`] for its intent record.
pub(crate) fn steer(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>) -> Option<(Actions, Option<u64>)> {
    let a = fauna.animals[i];
    let f = a.founder?;
    let phys = *fauna.config.founder(f);
    let d = phys.defence;
    fauna.ls_teach.remove(&a.id);
    if !d.enabled || crate::intent::menu_of(f) != &LOFTSTRIDER {
        return None;
    }
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let s = scene(fauna, i, view)?;
    let ps = plans(fauna, view, &d, &s, ring);
    let knobs = phys.intent;
    let n = LOFTSTRIDER.len();
    let now = fauna.tick;
    let mem = fauna.percept(a.id).map(|p| p.vigil).unwrap_or_default();
    let held = usize::from(mem.defend_held).min(n - 1);
    // The calm intents stand as one: the one held, else graze.
    let calm_k = if calm(held) { held } else { ix("graze") };
    let mut logits = vec![f64::NEG_INFINITY; n];
    let mut valid = vec![false; n];
    logits[calm_k] = d.w_calm;
    valid[calm_k] = true;
    for p in &ps {
        let k = ix(p.name);
        if p.score > logits[k] || !valid[k] {
            logits[k] = p.score;
            valid[k] = true;
        }
    }
    // A defence intent held keeps its own plan's validity: masked when it has none.
    let probs = crate::intent::probabilities(&logits, &valid, knobs.temperature);
    let first = mem.defend_since == 0;
    let age_s = now.saturating_sub(mem.defend_since) as f64 * DT;
    let k = crate::intent::arbitrate(&knobs, &valid, &probs, held, age_s, first);
    let proposal = (0..n).filter(|&j| valid[j]).fold(calm_k, |b, j| if probs[j] > probs[b] { j } else { b });
    if k != held || first {
        let v = &mut fauna.percepts.entry(a.id).or_default().vigil;
        v.defend_held = k as u8;
        v.defend_since = now.max(1);
    }
    if calm(k) {
        if calm(proposal) {
            return None;
        }
        // Held calm under the hold while a defence intent is proposed: today's rules run,
        // the proposal recorded.
        let p = ps.iter().find(|p| ix(p.name) == proposal)?;
        fauna.ls_teach.insert(a.id, Teach { name: Some(p.name), subject: p.subject, held: None, call: 0, kick: false });
        return None;
    }
    let plan = ps.iter().find(|p| ix(p.name) == k)?;
    let prop = ps.iter().find(|p| ix(p.name) == proposal).copied().unwrap_or(*plan);
    let call = called_now(fauna, a.id, f);
    // The aimed kick (§3.3): a grown body in guard (any defence intent with
    // `kick_any_intent`) whose kick is ready kicks a hound it sees in reach and in its front
    // arc; it stands for the windup. The write side starts it ([`kick_now`]) once no later
    // layer has replaced the steering.
    let mut act = plan.act;
    let kicks_now = d.kick_any_intent || plan.name == "guard";
    let target = match (s.young, phys.kick) {
        (false, Some(kk)) if kicks_now && a.kick.phase == crate::kick::Phase::Ready => kick_target(fauna, &kk, &s, ring),
        _ => None,
    };
    if target.is_some() {
        act.forward = 0.0;
        act.feed = 0.0;
    }
    fauna.ls_teach.insert(a.id, Teach { name: Some(prop.name), subject: plan.subject, held: Some(plan.name), call, kick: false });
    fauna.ledger.defence_samples[ix(plan.name)] += 1;
    Some((act, target))
}

/// **Start the aimed kick** [`steer`] chose for body `i` at `target` (the write side, after
/// the layers that may replace its steering); recorded on its teacher label.
pub(crate) fn kick_now(fauna: &mut Fauna, i: usize, target: u64) -> bool {
    let ok = crate::kick::request(fauna, i, target, crate::kick::Source::Aimed).is_ok();
    let id = fauna.animals[i].id;
    if let Some(t) = fauna.ls_teach.get_mut(&id) {
        t.kick |= ok;
    }
    ok
}

/// **The hound to kick** (§3.3), from what body `s.me` sees: the nearest threat it sees whose
/// footprint (estimated from its size) its kick reaches from its origin, inside the front arc.
fn kick_target(fauna: &Fauna, k: &crate::kick::Kick, s: &Scene, ring: f64) -> Option<u64> {
    let me = &s.me;
    let now = fauna.tick;
    let fresh = 2 * fauna.config.perception.stagger_ticks();
    let (ox, oz) = crate::step::jaw_of(me.pose.x, me.pose.z, me.pose.heading_rad, s.length, k.origin_forward_fraction);
    fauna
        .percept(me.id)?
        .tracks()
        .iter()
        .filter(|t| t.known && t.class == class::THREAT && t.modes & SIGHT != 0 && now.saturating_sub(t.tick) <= fresh)
        .filter(|t| s.meet_young || !looks_young(&fauna.config, t))
        .filter(|t| {
            let (dx, dz, dd) = gap(me.pose.x, me.pose.z, t.x, t.z, ring);
            dd > 1e-9 && angle_between(me.pose.heading_rad, dx.atan2(dz)) <= k.arc_deg.to_radians() && gap(ox, oz, t.x, t.z, ring).2 <= k.reach_m + radius_est(&fauna.config, t)
        })
        .min_by(|p, q| gap(me.pose.x, me.pose.z, p.x, p.z, ring).2.total_cmp(&gap(me.pose.x, me.pose.z, q.x, q.z, ring).2).then(p.id.cmp(&q.id)))
        .map(|t| t.id)
}

/// Whether body `s.me`'s kick would reach threat `h` now, from what it sees (the charge
/// stops there; [`kick_target`]'s test).
fn in_kick_reach(fauna: &Fauna, s: &Scene, h: &Threat, ring: f64) -> bool {
    let Some(k) = s.me.founder.and_then(|f| fauna.config.founder(f).kick) else { return false };
    let Some(t) = fauna.percept(s.me.id).and_then(|p| p.track(h.id)) else { return false };
    let (ox, oz) = crate::step::jaw_of(s.me.pose.x, s.me.pose.z, s.me.pose.heading_rad, s.length, k.origin_forward_fraction);
    gap(ox, oz, t.x, t.z, ring).2 <= k.reach_m + radius_est(&fauna.config, t)
}

/// The call kind body `id` emitted this tick (0 none, `k + 1` its lineage's kind `k`).
fn called_now(fauna: &Fauna, id: u64, f: Founder) -> u8 {
    (0..crate::calls::kinds_of(f).len()).find(|&k| fauna.last_call.get(&(id, k as u8)) == Some(&fauna.tick)).map_or(0, |k| k as u8 + 1)
}

/// Whether sight shows track `t` heading at `(x, z)` within `deg`.
fn faces(t: &Track, x: f64, z: f64, deg: f64, ring: f64) -> bool {
    let (dx, dz, dd) = gap(t.x, t.z, x, z, ring);
    t.modes & SIGHT != 0 && dd > 1e-9 && angle_between(t.heading, dx.atan2(dz)) <= deg.to_radians()
}

/// **A point checked as reachable** (§11.5): body `a` walks straight to `(x, z)` by its own
/// motor's rule (footprint, steps, headroom, water) from where it stands.
fn reachable(fauna: &Fauna, view: &VoxelView<'_>, a: &Animal, (x, z): (f64, f64), ring: f64) -> bool {
    let (dx, dz, dd) = gap(a.pose.x, a.pose.z, x, z, ring);
    dd <= 1e-6 || crate::vigil::walk_clear(fauna, view, a, (dx, dz))
}

/// **A way checked as safe** (§3.5, §11.5): the walk from `a` to `b` passes no pressing threat
/// closer than `cross_m`, unless that threat is already as close to `a` (a way that runs
/// from a chaser is not a way across a hunter).
fn segment_safe(s: &Scene, a: (f64, f64), b: (f64, f64), cross_m: f64, ring: f64) -> bool {
    !s.threats.iter().any(|t| {
        let to_way = to_segment(a, b, (t.x, t.z), ring);
        t.pressing && to_way <= cross_m && to_way < gap(a.0, a.1, t.x, t.z, ring).2 - 1e-9
    })
}

#[cfg(test)]
#[path = "defend_tests.rs"]
mod defend_tests;
