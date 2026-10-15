//! **YOUNG-ROAM: young animals stray more as they grow** (`design/behaviour-ai-2026-10-11.md`
//! package 6: §0 row 7, §3.5; the brief `design/handoffs/young-roam-2026-10-12.md`). Wrysk:
//! "young loftstriders and non-infant seedporters should have some level of curiosity to stray
//! from their parent and explore though, increasing the older they get".
//!
//! - **Young pace** ([`pace_scale`]). Pace is calibrated in body lengths per second, so a young
//!   body's cruise, and its sprint with it, is its own length's: the lineage's cruise times its
//!   length over an adult's (`(body / birth_body)^(1/3)`, lengths going as the cube root of the
//!   body), never under `pace_floor` of it. A body at or past `birth_body` (an adult) keeps its
//!   lineage's cruise. Before this a calf at a fifth of the adult body walked and fled at the
//!   adult's metres per second.
//! - **The leash loosens** ([`leash`]). A young body that knows its mother (a recorded mother,
//!   sensed through [`crate::nurse::mother_track`]) and is younger than `independence_s` keeps
//!   within a comfort distance of her that grows with its age, from `follow_m` to `roam_m` at
//!   independence: past it the herd pull ([`crate::herding::group_pull`]) leans its walk back
//!   toward her at `follow_pull`, instead of toward the troop's or family's mean; within it,
//!   nothing pulls it. A nursing lineage's young before its `weaning_age_s` keeps its nursing
//!   follow ([`crate::nurse::mother_pull`]) and does not stray; its roam grows from weaning to
//!   independence. It weighs its mother on every remote sensing, as a dependent young does
//!   ([`crate::nurse::weighs_as_mother`]).
//! - **Excursions** ([`excursion`]). A leashed young that is calm (no threat in its record, on
//!   the ground, under its own steering) sets off now and then toward something it senses: a
//!   food or another animal in its record, or an unvisited place `excursion_m` off, away from
//!   its mother. The chance per hour rises linearly with its roam, from `excursion_per_h[0]`
//!   at its start to `excursion_per_h[1]` at independence. While out, nothing pulls it back.
//! - **Coming back.** An excursion ends when the young is alarmed (a threat in its record or an
//!   alarm call heard since it set out), hears its mother's call (a recognised voice), or loses
//!   sight of its family (no kin in its record); then it **bleats** (its lineage's distress
//!   call) and its leash takes it back. It also ends, quietly, at its goal (within
//!   `arrive_m`) or after `excursion_s`.
//! - **Bolder with age** (CALF-ROAM, `design/handoffs/calf-roam-2026-10-15.md`). Losing sight
//!   ends an excursion only once it has sensed no kin for `lost_s`, rising linearly with its
//!   roam from `lost_s[0]` at its start to `lost_s[1]` at independence (0: at once, as before),
//!   so an older young can walk on past its family's sight toward its goal.
//! - **The mother comes** ([`answer`]). A body that hears its own young's distress call (it
//!   recognises the voice) walks to where the call came from until within its calls'
//!   `arrive_m`, while the record is younger than the kind's `heed_s`. A loftstrider's
//!   defence layer still answers any calf's bleat under threat ([`crate::defend`]); this is
//!   the calm-time answer, and the seedporter's.
//!
//! Every excursion is recorded as the menu's **explore** intent (§3.5: the brains copy it
//! later), its subject the goal's point. The reward stays energy only. Session state: an
//! excursion in progress is not saved (a load sends the young back under its leash).

use serde::{Deserialize, Serialize};

use cubarium_voxel::DT;

use crate::perception::class;
use crate::{Fauna, Founder};

/// **A lineage's young** (module note): its knobs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Young {
    /// A young body's cruise (and sprint) is its own length's, never under this fraction of
    /// the adult's; 1: the adult's pace at any size (before YOUNG-ROAM).
    pub pace_floor: f64,
    /// A young body keeps to its mother, and roams, until this age, seconds; 0: off.
    pub independence_s: f64,
    /// Its comfort distance from its mother at birth (a nursing lineage's: at weaning),
    /// metres; 0: off (the leash and excursions).
    pub follow_m: f64,
    /// ...growing with its age to this at `independence_s`, metres.
    pub roam_m: f64,
    /// The leash's share of its walking turn past the comfort (1: straight at its mother).
    pub follow_pull: f64,
    /// The chance per hour a calm young sets off on an excursion, at the start of its roam
    /// (birth, or weaning) and at independence; linear between.
    pub excursion_per_h: [f64; 2],
    /// An unvisited place it sets off for is this far, metres.
    pub excursion_m: f64,
    /// Within this of its goal, it is there and the excursion ends, metres.
    pub arrive_m: f64,
    /// An excursion ends after this long, seconds.
    pub excursion_s: f64,
    /// Out on an excursion, it gives up and bleats once it has sensed no kin for this long, at
    /// the start of its roam (birth, or weaning) and at independence; linear between, seconds.
    /// 0: as soon as no kin is in its record.
    pub lost_s: [f64; 2],
}

impl Default for Young {
    /// The adult's pace at any size; no roaming.
    fn default() -> Young {
        Young {
            pace_floor: 1.0,
            independence_s: 0.0,
            follow_m: 0.0,
            roam_m: 0.0,
            follow_pull: 0.0,
            excursion_per_h: [0.0; 2],
            excursion_m: 0.0,
            arrive_m: 0.0,
            excursion_s: 0.0,
            lost_s: [0.0; 2],
        }
    }
}

impl Young {
    /// Whether the leash (and with it the excursions) is on.
    pub fn roams(&self) -> bool {
        self.independence_s > 0.0 && self.follow_m > 0.0
    }
}

/// **Each lineage's young** (module note), placeholders, untuned:
/// - `pace_floor` 0.5 for every lineage: a newborn is about 0.55 of a breeding adult's length
///   (its `body_min` over `birth_body`, cube root), so the floor seldom binds; it keeps a body
///   born small walking at half the adult's pace (hoofed calves run with the herd within a day
///   of birth, at about half the adults' speed).
/// - **The loftstrider** (Wrysk's addition on topic 7): a calf keeps within 3 m of its mother at
///   birth (at heel, under two of her lengths) and within 10 m at independence (beyond the
///   family's 6 m comfort, inside its 12 m day sight); independence at 15,000 s, about 0.7 of
///   its 21,600 s birth interval (giraffe calves stay with the mother 12-16 months of a
///   ~20-month interbirth); the leash's pull is the family pull's half turn.
/// - **The seedporter**: from its weaning (its 3 m nursing leash) to 8 m at independence (twice
///   the troop's 4 m comfort, half its 16 m sight); independence at 18,000 s, half its 10 h
///   year (marmoset juveniles forage on their own by five to six months); the troop's half
///   turn.
/// - **Excursions**: a calf sets off about once an hour at birth and four times an hour at
///   independence, for a place 40 m off (BALANCE 2026-10-12, from 20: beyond its 12 m day
///   sight, so it can lose its family, and far enough that a stray meets the pack),
///   arriving within 1.5 m, for at most two minutes; a weaned seedporter twice an hour at
///   weaning and six at independence, 12 m off (inside its 16 m sight), within 0.5 m, two
///   minutes. Balance knobs (§6, topic 8): the pack's calf kills should come from strays.
/// - **Bolder with age** (`lost_s`): 0 for every lineage, an excursion ending as soon as the
///   young loses sight of its family (before CALF-ROAM).
pub fn young_of(founder: Founder) -> Young {
    let d = Young { pace_floor: 0.5, ..Young::default() };
    match founder {
        Founder::Loftstrider => Young {
            independence_s: 15_000.0,
            follow_m: 3.0,
            roam_m: 10.0,
            follow_pull: 0.5,
            excursion_per_h: [1.0, 4.0],
            excursion_m: 40.0,
            arrive_m: 1.5,
            excursion_s: 120.0,
            ..d
        },
        Founder::Seedporter => Young {
            independence_s: 18_000.0,
            follow_m: 3.0,
            roam_m: 8.0,
            follow_pull: 0.5,
            excursion_per_h: [2.0, 6.0],
            excursion_m: 12.0,
            arrive_m: 0.5,
            excursion_s: 120.0,
            ..d
        },
        _ => d,
    }
}

/// **The pace scale** of a body of `body` on lineage table `phys` (module note): its length
/// over an adult's (`(body / birth_body)^(1/3)`), at least `pace_floor`, at most 1; 1 at or
/// past `birth_body`.
pub fn pace_scale(phys: &crate::FounderPhysiology, body: f64) -> f64 {
    let adult = phys.core.birth_body;
    if !(adult > 0.0) || !body.is_finite() || body >= adult {
        return 1.0;
    }
    let own = (body.max(0.0) / adult).cbrt();
    let floor = if phys.young.pace_floor.is_finite() { phys.young.pace_floor.clamp(0.0, 1.0) } else { 1.0 };
    own.max(floor).min(1.0)
}

/// The age at which body `i`'s roam begins: its lineage's weaning age when it nurses (an
/// infant does not stray), else birth.
fn roam_start_s(fauna: &Fauna, i: usize) -> f64 {
    let Some(f) = fauna.animals.get(i).and_then(|a| a.founder) else { return 0.0 };
    let n = fauna.config.founder(f).nursing;
    if n.on() { n.weaning_age_s.max(0.0) } else { 0.0 }
}

/// **Body `i`'s roam**, 0 at its start (birth, or weaning) to 1 at `independence_s`: `Some`
/// for a young body of a roaming lineage that knows its mother, past the start and short of
/// independence; `None` else (an infant before weaning included).
pub fn progress(fauna: &Fauna, i: usize) -> Option<f64> {
    let a = fauna.animals.get(i)?;
    let y = fauna.config.founder(a.founder?).young;
    if !y.roams() || crate::mother_body(a).is_none() {
        return None;
    }
    let age = a.age_ticks as f64 * DT;
    let start = roam_start_s(fauna, i);
    if age < start || age >= y.independence_s {
        return None;
    }
    let span = y.independence_s - start;
    Some(if span > 0.0 { ((age - start) / span).clamp(0.0, 1.0) } else { 1.0 })
}

/// **The leash** (module note) of body `i`: its comfort distance from its mother at its age
/// and the pull's weight, or `None` when it has none ([`progress`]).
pub fn leash(fauna: &Fauna, i: usize) -> Option<(f64, f64)> {
    let k = progress(fauna, i)?;
    let y = fauna.config.founder(fauna.animals[i].founder?).young;
    let roam = y.roam_m.max(y.follow_m);
    Some((y.follow_m + (roam - y.follow_m) * k, y.follow_pull.clamp(0.0, 1.0)))
}

// ------------------------------------------------------------------ excursions

/// The keyed stream's domain for excursions.
const DOMAIN_ROAM: u64 = 0x0E4C_0125;

/// Why an excursion ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// A threat in its record, or an alarm heard since it set out: it bleats.
    Alarmed,
    /// Its mother's voice heard since it set out: it bleats.
    MotherCalled,
    /// No kin in its record: it bleats.
    LostSight,
    /// At its goal.
    Arrived,
    /// After `excursion_s`.
    TimedOut,
}

impl End {
    pub const ALL: [End; 5] = [End::Alarmed, End::MotherCalled, End::LostSight, End::Arrived, End::TimedOut];

    pub fn index(self) -> usize {
        End::ALL.iter().position(|&e| e == self).unwrap_or(0)
    }

    pub fn name(self) -> &'static str {
        match self {
            End::Alarmed => "alarmed",
            End::MotherCalled => "mother_called",
            End::LostSight => "lost_sight",
            End::Arrived => "arrived",
            End::TimedOut => "timed_out",
        }
    }

    /// Whether the young bleats at this end.
    pub fn bleats(self) -> bool {
        matches!(self, End::Alarmed | End::MotherCalled | End::LostSight)
    }
}

/// What an excursion makes for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    /// A food in its record (class food).
    Food,
    /// Another animal in its record (not kin, not a threat).
    Animal,
    /// An unvisited place.
    Place,
}

/// **An excursion under way** (session state, not saved).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Excursion {
    pub goal: Goal,
    /// The track it heads for (a food or an animal), 0 for a place.
    pub track: u64,
    /// Where it heads, metres on the ring.
    pub x: f64,
    pub z: f64,
    pub y_m: f64,
    /// The tick it set out.
    pub since: u64,
    /// The last tick it sensed kin (from `since`): `lost_s` counts from here.
    pub kin_tick: u64,
}

/// **The excursions' readout** per lineage (diagnostics only; session state, not saved).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RoamTally {
    /// Excursions started, by goal ([`Goal`] order: food, animal, place).
    pub started: [u64; 3],
    /// Excursions ended, by [`End`] order.
    pub ended: [u64; 5],
    /// Distress calls emitted at an excursion's end.
    pub bleats: u64,
    /// Controller samples a mother spent answering her own young's distress call.
    pub answer_samples: u64,
}

/// **A young of its mother** (CALF-PLACE's readout): a body that knows its mother
/// ([`crate::mother_body`], living or not) and is younger than its lineage's
/// `young.independence_s` or nursing `weaning_age_s`, whichever is later; never for a
/// lineage with neither. Readouts only.
pub fn young_of_mother(config: &crate::FaunaConfig, a: &crate::Animal) -> bool {
    let Some(f) = a.founder else { return false };
    let phys = config.founder(f);
    let until = phys.young.independence_s.max(if phys.nursing.on() { phys.nursing.weaning_age_s } else { 0.0 });
    crate::mother_body(a).is_some() && until > 0.0 && (a.age_ticks as f64) * DT < until
}

/// **Harm by lineage** (CALF-PLACE's readout; session state, not saved, diagnostics only),
/// indexed `[by][prey]` in [`Founder::ALL`] order: every wounding hit (a strike landed or a
/// kick), and the hits on and kills of a young of its mother ([`young_of_mother`], read at
/// the hit).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HarmTally {
    pub wounds: [[u64; Founder::COUNT]; Founder::COUNT],
    pub young_wounds: [[u64; Founder::COUNT]; Founder::COUNT],
    pub young_kills: [[u64; Founder::COUNT]; Founder::COUNT],
}

/// Whether body `i`'s record holds a fresh track of `cls`.
fn senses_class(fauna: &Fauna, id: u64, cls: u8) -> bool {
    let fresh = 2 * fauna.config.perception.stagger_ticks();
    let now = fauna.tick;
    fauna.percept(id).is_some_and(|p| p.tracks().iter().any(|t| t.known && t.class == cls && now.saturating_sub(t.tick) <= fresh))
}

/// The time out of sight of kin that ends an excursion at roam `k` (`lost_s`, linear), ticks.
fn lost_ticks(y: &Young, k: f64) -> u64 {
    let s = y.lost_s[0] + (y.lost_s[1] - y.lost_s[0]) * k.clamp(0.0, 1.0);
    if s.is_finite() && s > 0.0 { (s / DT).round() as u64 } else { 0 }
}

/// Why body `i`'s excursion `e` ends now, if it does (module note), from its own record and
/// calls; `k` its roam.
fn ends(fauna: &Fauna, i: usize, e: &Excursion, y: &Young, k: f64, ring: f64) -> Option<End> {
    let a = &fauna.animals[i];
    let heard = crate::calls::heard(fauna, a.id);
    if heard.iter().any(|r| r.relation == 1 && r.tick >= e.since) {
        return Some(End::MotherCalled);
    }
    if senses_class(fauna, a.id, class::THREAT) || heard.iter().any(|r| r.kind == 0 && r.tick >= e.since) {
        return Some(End::Alarmed);
    }
    if !senses_class(fauna, a.id, class::KIN) && fauna.tick.saturating_sub(e.kin_tick) >= lost_ticks(y, k) {
        return Some(End::LostSight);
    }
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, e.x, e.z, ring);
    if dx.hypot(dz) <= y.arrive_m.max(0.0) {
        return Some(End::Arrived);
    }
    if fauna.tick.saturating_sub(e.since) as f64 * DT >= y.excursion_s.max(0.0) {
        return Some(End::TimedOut);
    }
    None
}

/// **Set off** (module note): the goal body `i` picks, from its own record — a food or another
/// animal it senses, or an unvisited place `excursion_m` off away from its mother (where it
/// senses her; else along a heading drawn at random) — each candidate alike.
fn pick(fauna: &Fauna, i: usize, y: &Young, rng: &mut cubarium_voxel::KeyedRng, ring: f64) -> Excursion {
    let a = &fauna.animals[i];
    let now = fauna.tick;
    let fresh = 2 * fauna.config.perception.stagger_ticks();
    let mut options: Vec<(Goal, u64, f64, f64, f64)> = Vec::new();
    if let Some(p) = fauna.percept(a.id) {
        for t in p.tracks().iter().filter(|t| t.known && now.saturating_sub(t.tick) <= fresh) {
            match t.class {
                class::FOOD => options.push((Goal::Food, t.id, t.x, t.z, t.y_m)),
                class::UNKNOWN | class::PREY if t.id != 0 => options.push((Goal::Animal, t.id, t.x, t.z, t.y_m)),
                _ => {}
            }
        }
    }
    // An unvisited place: away from its mother, give or take a quarter turn.
    let away = crate::nurse::mother_track(fauna, i).map(|m| {
        let (dx, dz) = crate::pose::wrapped_delta(m.x, m.z, a.pose.x, a.pose.z, ring);
        dx.atan2(dz)
    });
    let u = rng.unit();
    let heading = match away {
        Some(h) => h + (u - 0.5) * std::f64::consts::FRAC_PI_2,
        None => u * std::f64::consts::TAU,
    };
    let m = y.excursion_m.max(0.0);
    let px = (a.pose.x + m * heading.sin()).rem_euclid(ring.max(f64::EPSILON));
    options.push((Goal::Place, 0, px, a.pose.z + m * heading.cos(), a.pose.y_m));
    let k = ((rng.unit() * options.len() as f64) as usize).min(options.len() - 1);
    let (goal, track, x, z, y_m) = options[k];
    Excursion { goal, track, x, z, y_m, since: now, kin_tick: now }
}

/// Body `i`'s excursion as its intent subject: the goal's point.
pub(crate) fn subject(e: &Excursion) -> crate::intent::Subject {
    crate::intent::Subject::Place { kind: crate::intent::PlaceKind::ExcursionGoal, x: e.x, z: e.z, y_m: e.y_m, seen_tick: e.since }
}

/// **Excursions** (module note) for heuristic body `i` at its controller sample, when its
/// steering is still its own (`calm`: its own heuristic, a search leg or the walk off a patch,
/// on the ground). Ends an excursion under way when it must (and bleats); starts one at the
/// chance for its roam; returns the walk toward its goal while one is under way. `period_s`
/// is its controller period.
pub(crate) fn excursion(fauna: &mut Fauna, i: usize, calm: bool, period_s: f64, seed: u64, ring: f64) -> Option<crate::Actions> {
    let a = fauna.animals[i];
    let f = a.founder?;
    let y = fauna.config.founder(f).young;
    let under_way = fauna.excursions.get(&a.id).copied();
    let roam = progress(fauna, i).filter(|_| crate::intent::young(fauna, &a, f));
    let Some(k) = roam else {
        fauna.excursions.remove(&a.id);
        return None;
    };
    if let Some(mut e) = under_way {
        // Its goal moves with the body it makes for, while it senses it.
        if e.track != 0 {
            if let Some(t) = fauna.percept(a.id).and_then(|p| p.track(e.track)) {
                (e.x, e.z, e.y_m) = (t.x, t.z, t.y_m);
            }
        }
        if senses_class(fauna, a.id, class::KIN) {
            e.kin_tick = fauna.tick;
        }
        if let Some(end) = ends(fauna, i, &e, &y, k, ring) {
            fauna.excursions.remove(&a.id);
            let tally = &mut fauna.roam_tally[f.index()];
            tally.ended[end.index()] += 1;
            if end.bleats() && crate::calls::kinds_of(f).len() > 1 && crate::calls::emit(fauna, i, 1) {
                fauna.roam_tally[f.index()].bleats += 1;
            }
            return None;
        }
        if !calm {
            // A later or earlier layer has it (feeding still, resting, off the ground): the
            // excursion waits, its clock running.
            fauna.excursions.insert(a.id, e);
            return None;
        }
        fauna.excursions.insert(a.id, e);
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, e.x, e.z, ring);
        return Some(crate::vigil::head(&a, dx.atan2(dz), 1.0, 0.0));
    }
    // Setting off: calm, resting not, kin in sense and no threat.
    if !calm || a.rest_ticks > 0 || !senses_class(fauna, a.id, class::KIN) || senses_class(fauna, a.id, class::THREAT) {
        return None;
    }
    let rate = y.excursion_per_h[0] + (y.excursion_per_h[1] - y.excursion_per_h[0]) * k;
    let p = (rate.max(0.0) * period_s.max(0.0) / 3600.0).min(1.0);
    let mut rng = cubarium_voxel::KeyedRng::keyed(DOMAIN_ROAM, seed, a.id, fauna.tick);
    if !(rng.unit() < p) {
        return None;
    }
    let e = pick(fauna, i, &y, &mut rng, ring);
    fauna.roam_tally[f.index()].started[e.goal as usize] += 1;
    fauna.excursions.insert(a.id, e);
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, e.x, e.z, ring);
    Some(crate::vigil::head(&a, dx.atan2(dz), 1.0, 0.0))
}

/// **The mother comes** (module note) for heuristic body `i`: the walk to where its own
/// young's distress call came from, while the record is younger than the kind's `heed_s` and
/// it is farther than its calls' `arrive_m`; `None` else. Reads its own call records only.
pub(crate) fn answer(fauna: &mut Fauna, i: usize, ring: f64) -> Option<(crate::Actions, crate::calls::CallRecord)> {
    let a = fauna.animals[i];
    let f = a.founder?;
    let t = fauna.config.founder(f).calls;
    if crate::calls::kinds_of(f).len() < 2 || !(t.heed_s[1] > 0.0) || !crate::calls::heeds(&fauna.config, f, &a, 1) {
        return None;
    }
    let now = fauna.tick;
    let r = crate::calls::heard(fauna, a.id)
        .iter()
        .filter(|r| r.kind == 1 && r.relation == 2 && now.saturating_sub(r.tick) as f64 * DT <= t.heed_s[1])
        .max_by_key(|r| r.tick)
        .copied()?;
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, r.x, r.z, ring);
    if dx.hypot(dz) <= t.arrive_m.max(0.0) {
        return None;
    }
    fauna.roam_tally[f.index()].answer_samples += 1;
    Some((crate::vigil::head(&a, dx.atan2(dz), 1.0, 0.0), r))
}

#[cfg(test)]
#[path = "roam_tests.rs"]
mod roam_tests;
