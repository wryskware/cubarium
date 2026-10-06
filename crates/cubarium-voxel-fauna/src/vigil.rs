//! **Ground responses** (S2 of the senses pass, `design/7_Research/senses-review-2026-10-06.md`
//! §1, §2, §4 and §5 S2, with Wrysk's choices of 2026-10-06): what a live prey lineage does
//! with its percept record ([`crate::Percept`]). Each response reads only the record and keeps
//! its own state in the record's [`Vigil`]; its numbers are the lineage's [`Vigilance`]
//! (`fauna.founders.<lineage>.vigilance.*`), every one a plausible start, untuned.
//!
//! - **Herd** (the frondgrazer, the loftstrider):
//!   - a **scan pause** while cropping: every `scan_every_s` of cropping it stands with its
//!     head up for `scan_s`. Between scans its head is down and it sees only within
//!     [`crate::SenseTable::feeding_sight_fraction`] of its range: its blind window;
//!   - **orients** to an approaching threat (seen moving or near, heard, or felt) it has not
//!     faced yet: stands and faces it for `orient_s`, then grazes on while it stays away
//!     (habituated to that one body);
//!   - **flees with hysteresis**: it starts when at least `group_min` threats (one, for a
//!     juvenile) are within `flee_start_m` — seen moving or near, heard, or felt — and runs
//!     straight away from the nearest; it stops only once it is beyond `flee_stop_m` of where
//!     it last knew a threat and none has come within that for `calm_s`;
//!   - **herd alarm by hearing**: a running body is loud (its emission times `run_noise`
//!     while it flees or bolts), so a kin it hears whose emission (loudness times distance)
//!     is at least `alarm_emission_factor` times that kin's walking emission is bolting, and
//!     it bolts too: along that kin's heading for `alarm_bolt_s`, then deaf to alarms for
//!     `alarm_refractory_s`, so a wave crosses the herd once and does not echo.
//! - **Arboreal** (the seedporter): a threat within `flee_start_m` sends it to the nearest
//!   stand it can climb within `refuge_search_m` (walking into the stem climbs it), or
//!   straight away with none; in the crown it holds still until calm (the same hysteresis).
//! - **Freeze** (the littershredder): footfall at least `freeze_tremor` freezes it for
//!   `freeze_s` (no step, no bite); at least `withdraw_tremor` it also turns away in place. It
//!   never flees: it stays on its litter.
//!
//! The awareness the strike's dodge reads ([`awareness`], D5) and the Veilhound's stalk
//! ([`stalk`]) are here too: both read the prey's record.

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::FloraView;
use serde::{Deserialize, Serialize};

use crate::perception::{HEARING, SIGHT, TREMOR, class};
use crate::{Actions, Animal, Fauna, FaunaConfig, Founder, MobilityMode, Percept, Track};

/// Which response a lineage's heuristics run on its record (a switch, no registry row).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VigilMode {
    /// None: its S1 reflexes, whatever its table.
    #[default]
    None,
    Herd,
    Arboreal,
    Freeze,
}

/// **A lineage's ground response** (module note): its knobs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Vigilance {
    /// The response (a switch).
    pub mode: VigilMode,
    /// Flight starts when threats are within this, metres.
    pub flee_start_m: f64,
    /// ...and stops beyond this of the threat it last knew (at least `flee_start_m`), metres.
    pub flee_stop_m: f64,
    /// ...once no threat has come within `flee_stop_m` for this long, seconds.
    pub calm_s: f64,
    /// An adult flees only from at least this many threats within `flee_start_m` (a
    /// juvenile from one): below it it stands its ground, facing them. Count.
    pub group_min: u32,
    /// Its emission (hearing's `speed^a × length^b`) is multiplied by this while it flees or
    /// bolts: a run is loud. Factor.
    pub run_noise: f64,
    /// Cropping between two scan pauses, seconds; 0 never scans.
    pub scan_every_s: f64,
    /// A scan pause, seconds.
    pub scan_s: f64,
    /// It faces a newly sensed threat this long, seconds; 0 never orients.
    pub orient_s: f64,
    /// A kin heard with an emission at least this many times its walking emission (cruise
    /// × length) is bolting, and it bolts too; 0 hears no alarm. Factor.
    pub alarm_emission_factor: f64,
    /// An alarm bolt lasts this long, seconds.
    pub alarm_bolt_s: f64,
    /// After a bolt it hears no alarm for this long, seconds.
    pub alarm_refractory_s: f64,
    /// Footfall (`left + right`, the tremor's units) that freezes it; 0 never freezes.
    pub freeze_tremor: f64,
    /// Footfall at which it also turns away in place.
    pub withdraw_tremor: f64,
    /// A freeze lasts this long after the last tremor that set it, seconds.
    pub freeze_s: f64,
    /// An arboreal body runs to a stand it can climb within this, metres.
    pub refuge_search_m: f64,
}

impl Default for Vigilance {
    /// No response.
    fn default() -> Vigilance {
        Vigilance {
            mode: VigilMode::None,
            flee_start_m: 0.0,
            flee_stop_m: 0.0,
            calm_s: 0.0,
            group_min: 1,
            run_noise: 1.0,
            scan_every_s: 0.0,
            scan_s: 0.0,
            orient_s: 0.0,
            alarm_emission_factor: 0.0,
            alarm_bolt_s: 0.0,
            alarm_refractory_s: 0.0,
            freeze_tremor: 0.0,
            withdraw_tremor: 0.0,
            freeze_s: 0.0,
            refuge_search_m: 0.0,
        }
    }
}

/// **Each lineage's ground response** (review §1 and §4, Astra's per-species rows),
/// placeholders at the terrarium's scale.
pub fn vigilance_of(founder: Founder) -> Vigilance {
    let none = Vigilance::default();
    match founder {
        // Astra: alternate cropping with brief scans; orient to uncertain sounds, flee a
        // confirmed approaching threat (start 4 m, half its 8 m sight), then reassess with
        // hysteresis (stop at its sight range, 10 s calm). A run is three times as loud as a
        // walk; a bolt of 3 s, deaf to alarms 20 s after.
        Founder::Browser => Vigilance {
            mode: VigilMode::Herd,
            flee_start_m: 4.0,
            flee_stop_m: 8.0,
            calm_s: 10.0,
            group_min: 1,
            run_noise: 3.0,
            scan_every_s: 6.0,
            scan_s: 1.5,
            orient_s: 3.0,
            alarm_emission_factor: 2.0,
            alarm_bolt_s: 3.0,
            alarm_refractory_s: 20.0,
            ..none
        },
        // Panoramic from its eye ring, so no scan pause; an adult stands its ground against
        // a lone hunter and moves off before a converging group (two or more); no herd alarm.
        Founder::Loftstrider => Vigilance {
            mode: VigilMode::Herd,
            flee_start_m: 6.0,
            flee_stop_m: 12.0,
            calm_s: 10.0,
            group_min: 2,
            run_noise: 3.0,
            orient_s: 3.0,
            ..none
        },
        // Flight to the trees: a threat within 8 m (half its daylight sight) on the ground
        // sends it to a stand within twice its roost reach (8 m); no alarm calls (D6, later).
        Founder::Seedporter => Vigilance {
            mode: VigilMode::Arboreal,
            flee_start_m: 8.0,
            flee_stop_m: 12.0,
            calm_s: 10.0,
            group_min: 1,
            run_noise: 3.0,
            refuge_search_m: 8.0,
            ..none
        },
        // Freeze at the old flee line (predation's `flee_tremor`, 0.015), turn away at four
        // times it, 3 s.
        Founder::Blind => Vigilance {
            mode: VigilMode::Freeze,
            freeze_tremor: 0.015,
            withdraw_tremor: 0.06,
            freeze_s: 3.0,
            ..none
        },
        _ => none,
    }
}

/// **A body's response state**, kept in its record ([`Percept::vigil`]). Ticks are the
/// world's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vigil {
    /// At its last sample it was cropping (head down unless scanning).
    pub cropping: bool,
    /// A scan pause runs until this tick.
    pub scan_until: u64,
    /// The next scan pause begins at this tick of cropping (0: not yet scheduled).
    pub next_scan: u64,
    /// In flight, from the point it last knew a threat.
    pub fleeing: bool,
    pub from_x: f64,
    pub from_z: f64,
    /// The last tick a threat was within `flee_stop_m`.
    pub close_tick: u64,
    /// An alarm bolt runs until this tick, along this heading.
    pub bolt_until: u64,
    pub bolt_heading: f64,
    /// It hears alarms again from this tick.
    pub alarm_ready: u64,
    /// The threat it oriented to (habituated to after), and until when it faces it.
    pub orient_id: u64,
    pub orient_until: u64,
    /// A freeze runs until this tick, turning `withdraw_turn` (0 still).
    pub freeze_until: u64,
    pub withdraw_turn: f64,
}

impl Vigil {
    /// In a scan pause at `now`.
    pub fn scanning(&self, now: u64) -> bool {
        now < self.scan_until
    }

    /// Running (fleeing or bolting) at `now`: loud.
    pub fn running(&self, now: u64) -> bool {
        self.fleeing || now < self.bolt_until
    }

    /// Head down at `now`: cropping, not scanning and not running.
    pub fn head_down(&self, now: u64) -> bool {
        self.cropping && !self.scanning(now) && !self.running(now)
    }
}

fn ticks(s: f64) -> u64 {
    if s.is_finite() && s > 0.0 { cubarium_voxel::ticks_at_least_one(s) } else { 0 }
}

/// Steer toward the heading `want`: turn in place beyond 60°, else walk at `forward`.
fn head(a: &Animal, want: f64, forward: f64, feed: f64) -> Actions {
    let d = (want - a.pose.heading_rad).rem_euclid(std::f64::consts::TAU);
    let diff = if d > std::f64::consts::PI { d - std::f64::consts::TAU } else { d };
    Actions {
        forward: if diff.abs() > std::f64::consts::FRAC_PI_3 { 0.0 } else { forward },
        turn: (diff / std::f64::consts::FRAC_PI_4).clamp(-1.0, 1.0),
        feed,
    }
}

/// A threat in the record, from where it stands now.
#[derive(Clone, Copy, Debug)]
struct Threat {
    d: f64,
    x: f64,
    z: f64,
    id: u64,
    /// Seen moving or near, heard, or felt: an approaching body, not a still shape.
    pressing: bool,
}

fn threats(config: &FaunaConfig, a: &Animal, p: &Percept, now: u64, ring: f64) -> Vec<Threat> {
    let fresh = 2 * config.perception.stagger_ticks();
    let mut out: Vec<Threat> = p
        .tracks()
        .iter()
        .filter(|t| t.class == class::THREAT && now.saturating_sub(t.tick) <= fresh)
        .map(|t| {
            let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, t.x, t.z, ring);
            let d = (dx * dx + dz * dz).sqrt();
            let seen_pressing = t.modes & SIGHT != 0 && (t.moving || d <= config.predation.still_threat_near_m);
            Threat { d, x: t.x, z: t.z, id: t.id, pressing: seen_pressing || t.modes & (HEARING | TREMOR) != 0 }
        })
        .collect();
    out.sort_by(|p, q| p.d.total_cmp(&q.d).then(p.id.cmp(&q.id)));
    out
}

/// The flight's hysteresis, shared by the herd and the arboreal: starts at `need` pressing
/// threats within `flee_start_m`, follows the nearest threat it knows, stops beyond
/// `flee_stop_m` of it after `calm_s` with none within. Returns whether it started now.
fn flight(v: &mut Vigil, vg: &Vigilance, a: &Animal, th: &[Threat], need: usize, now: u64, ring: f64) -> bool {
    let start = vg.flee_start_m.max(0.0);
    let stop = vg.flee_stop_m.max(start);
    let mut started = false;
    if !v.fleeing && th.iter().filter(|t| t.pressing && t.d <= start).count() >= need.max(1) {
        v.fleeing = true;
        v.close_tick = now;
        started = true;
    }
    if v.fleeing {
        if let Some(t) = th.first() {
            v.from_x = t.x;
            v.from_z = t.z;
            if t.d <= stop {
                v.close_tick = now;
            }
        }
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, v.from_x, v.from_z, ring);
        if (dx * dx + dz * dz).sqrt() > stop && now.saturating_sub(v.close_tick) >= ticks(vg.calm_s) {
            v.fleeing = false;
        }
    }
    started
}

/// Away from the point it flees, at full forward.
fn away(a: &Animal, v: &Vigil, ring: f64) -> Actions {
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, v.from_x, v.from_z, ring);
    let want = if dx == 0.0 && dz == 0.0 { a.pose.heading_rad } else { (-dx).atan2(-dz) };
    head(a, want, 1.0, 0.0)
}

/// **The ground response** of the body at `i` (module note), for a live lineage with a mode;
/// `None` leaves its heuristic's actions. `held` is what its heuristic asked this sample.
pub(crate) fn respond(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>, held: Actions) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    if !phys.senses.live {
        return None;
    }
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    match phys.vigilance.mode {
        VigilMode::None => None,
        VigilMode::Herd => herd(fauna, i, ring, held),
        VigilMode::Arboreal => arboreal(fauna, i, view, fv, ring, held),
        VigilMode::Freeze => freeze(fauna, i),
    }
}

fn herd(fauna: &mut Fauna, i: usize, ring: f64, held: Actions) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let vg = phys.vigilance;
    let now = fauna.tick;
    let p = fauna.percepts.get(&a.id).copied().unwrap_or_default();
    let mut v = p.vigil;
    let th = threats(&fauna.config, &a, &p, now, ring);
    let adult = a.body >= phys.core.birth_body;
    let need = if adult { vg.group_min.max(1) as usize } else { 1 };
    let k = founder.index();
    if flight(&mut v, &vg, &a, &th, need, now, ring) {
        fauna.ledger.vigil.flees[k] += 1;
    }
    // The herd alarm: a kin heard running.
    if !v.fleeing && now >= v.alarm_ready && vg.alarm_emission_factor > 0.0 && now >= v.bolt_until {
        let pc = fauna.config.perception;
        let walk = |size: f64| (phys.cruise_body_lengths_per_s * size).max(0.0).powf(pc.hearing_speed_exp) * size.max(0.0).powf(pc.hearing_size_exp);
        let bolting = p.tracks().iter().filter(|t| t.class == class::KIN && t.modes & HEARING != 0 && t.tick + 2 * pc.stagger_ticks() >= now).find(|t| {
            let heard = t.loud * t.d_m.max(pc.hearing_near_m);
            heard >= vg.alarm_emission_factor * walk(t.size_m)
        });
        if let Some(t) = bolting {
            v.bolt_until = now + ticks(vg.alarm_bolt_s).max(1);
            v.bolt_heading = t.heading;
            v.alarm_ready = v.bolt_until + ticks(vg.alarm_refractory_s);
            fauna.ledger.vigil.alarm_bolts[k] += 1;
        }
    }
    let mut out = None;
    if v.fleeing {
        out = Some(away(&a, &v, ring));
    } else if now < v.bolt_until {
        out = Some(head(&a, v.bolt_heading, 1.0, 0.0));
    } else if vg.orient_s > 0.0 {
        // A threat it has not faced: face it. Below its group it stands its ground, facing
        // the nearest pressing one.
        let standing = th.iter().find(|t| t.pressing && t.d <= vg.flee_start_m);
        if let Some(t) = th.iter().find(|t| t.pressing && t.id != v.orient_id).filter(|_| now >= v.orient_until) {
            v.orient_id = t.id;
            v.orient_until = now + ticks(vg.orient_s);
            fauna.ledger.vigil.orients[k] += 1;
        }
        let facing = if now < v.orient_until { th.iter().find(|t| t.id == v.orient_id) } else { None };
        if let Some(t) = facing.or(standing) {
            let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, t.x, t.z, ring);
            let mut act = head(&a, dx.atan2(dz), 0.0, 0.0);
            act.forward = 0.0;
            out = Some(act);
        } else if th.iter().all(|t| t.id != v.orient_id) {
            v.orient_id = 0;
        }
    }
    // The scan pause, while cropping and nothing else holds it.
    let cropping = held.feed > 0.5 && held.forward <= fauna.config.teachers.forage_stay_forward;
    v.cropping = out.is_none() && cropping;
    if out.is_none() && vg.scan_every_s > 0.0 && vg.scan_s > 0.0 {
        let every = ticks(vg.scan_every_s).max(1);
        if cropping && !v.scanning(now) {
            if v.next_scan == 0 {
                v.next_scan = now + 1 + a.id % every;
            }
            if now >= v.next_scan {
                v.scan_until = now + ticks(vg.scan_s).max(1);
                v.next_scan = v.scan_until + every;
                fauna.ledger.vigil.scans[k] += 1;
            }
        }
        if v.scanning(now) {
            v.cropping = false;
            out = Some(Actions { forward: 0.0, turn: 0.0, feed: 0.0 });
        }
    }
    fauna.percepts.entry(a.id).or_default().vigil = v;
    out
}

fn arboreal(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>, ring: f64, held: Actions) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let vg = phys.vigilance;
    let now = fauna.tick;
    let p = fauna.percepts.get(&a.id).copied().unwrap_or_default();
    let mut v = p.vigil;
    let th = threats(&fauna.config, &a, &p, now, ring);
    let k = founder.index();
    // Any threat it senses counts: it is a small body on the ground.
    let sensed: Vec<Threat> = th.iter().map(|t| Threat { pressing: true, ..*t }).collect();
    let grounded = a.mobility.mode == MobilityMode::Ground;
    if flight(&mut v, &vg, &a, &sensed, 1, now, ring) && grounded {
        fauna.ledger.vigil.flees[k] += 1;
    }
    let mut out = None;
    if v.fleeing {
        out = match a.mobility.mode {
            MobilityMode::Ground => {
                match crate::step::refuge_step(fauna, i, view, fv, vg.refuge_search_m) {
                    Some(go) => {
                        fauna.ledger.vigil.refuges[k] += 1;
                        Some(crate::gait::dash(&fauna.config, founder, v.close_tick == now, go))
                    }
                    None => Some(crate::gait::dash(&fauna.config, founder, v.close_tick == now, away(&a, &v, ring))),
                }
            }
            // Up the stem, and still in the crown.
            MobilityMode::Climb if !a.mobility.falling => Some(Actions { forward: 1.0, turn: 0.0, feed: 0.0 }),
            MobilityMode::Perched => Some(Actions { forward: 0.0, turn: 0.0, feed: held.feed }),
            _ => None,
        };
    }
    fauna.percepts.entry(a.id).or_default().vigil = v;
    out
}

fn freeze(fauna: &mut Fauna, i: usize) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let vg = fauna.config.founder(founder).vigilance;
    let now = fauna.tick;
    let p = fauna.percepts.get(&a.id).copied().unwrap_or_default();
    let mut v = p.vigil;
    let [left, right] = p.tremor;
    let felt = left + right;
    if vg.freeze_tremor > 0.0 && felt >= vg.freeze_tremor {
        if now >= v.freeze_until {
            fauna.ledger.vigil.freezes[founder.index()] += 1;
        }
        v.freeze_until = now + ticks(vg.freeze_s).max(1);
        v.withdraw_turn = if vg.withdraw_tremor > 0.0 && felt >= vg.withdraw_tremor {
            if left > right { 1.0 } else { -1.0 }
        } else {
            0.0
        };
    }
    let out = (now < v.freeze_until).then_some(Actions { forward: 0.0, turn: v.withdraw_turn, feed: 0.0 });
    fauna.percepts.entry(a.id).or_default().vigil = v;
    out
}

/// **The strike's awareness** (D5): for a prey whose lineage is live, the predation table's
/// `awareness_sensed` when its record holds the striker sensed within `aware_within_s`, else
/// `awareness_unaware` (asleep, the table's asleep value); for any other prey the table's
/// state column, as before.
pub fn awareness(config: &FaunaConfig, prey: &Animal, record: Option<&Percept>, striker: u64, now: u64) -> f64 {
    let cfg = &config.predation;
    let live = prey.founder.is_some_and(|f| config.founder(f).senses.live);
    if !live || prey.state == crate::State::Asleep {
        return cfg.awareness(prey.state);
    }
    if aware_of(config, record, striker, now) { cfg.awareness_sensed } else { cfg.awareness_unaware }
}

/// Whether a record holds `striker` sensed within the predation table's `aware_within_s`.
pub fn aware_of(config: &FaunaConfig, record: Option<&Percept>, striker: u64, now: u64) -> bool {
    let within = ticks(config.predation.aware_within_s);
    let fresh = |t: &Track| t.known && t.id == striker && now.saturating_sub(t.tick) <= within;
    record.is_some_and(|p| p.tracks().iter().any(fresh) || fresh(&p.best[usize::from(class::THREAT)]))
}

/// **The stalk** (the Veilhound's gait, review §1 and §4): the forward a pack member at `a`
/// closing on the prey it knows as `t`, `d` metres off, may use. Beyond its pack's
/// `commit_m` it stalks: no faster than keeps its emission under `stalk_margin` of the
/// prey's hearing floor at that distance, and still while the prey looks up (a scan it
/// sees, with `stalk_freeze`); within it, or once the prey runs, it rushes (`asked`).
/// `None` when the pack does not stalk.
pub(crate) fn stalk(fauna: &mut Fauna, a: &Animal, t: &Track, d: f64, asked: f64) -> Option<f64> {
    let pk = fauna.config.founder(a.founder?).pack?;
    if !(pk.stalk_margin > 0.0) {
        return None;
    }
    let now = fauna.tick;
    let prey = fauna.view().animal(t.id).copied()?;
    let pv = fauna.percept(prey.id).map(|p| p.vigil).unwrap_or_default();
    if d <= pk.commit_m || pv.running(now) {
        fauna.ledger.vigil.rush_samples += 1;
        // GAIT: the rush is its sprint.
        return Some(asked * crate::gait::sprint_forward(&fauna.config, a.founder?));
    }
    if pk.stalk_freeze && t.modes & SIGHT != 0 && pv.scanning(now) {
        fauna.ledger.vigil.stalk_freezes += 1;
        return Some(0.0);
    }
    let pt = fauna.config.founder(prey.founder?).senses;
    let pc = fauna.config.perception;
    let geometry = crate::body_of(&fauna.config, a)?;
    let cruise = fauna.config.manifest(a.founder?).cruise_m_per_s;
    if !(pt.hearing_m > 0.0) || d > pt.hearing_m || !(cruise > 0.0) || !(pc.hearing_speed_exp > 0.0) {
        return Some(asked);
    }
    fauna.ledger.vigil.stalk_samples += 1;
    let emission = pk.stalk_margin * pt.hearing_min * d.max(pc.hearing_near_m) / geometry.length_m.max(1e-9).powf(pc.hearing_size_exp);
    let speed = emission.max(0.0).powf(1.0 / pc.hearing_speed_exp);
    Some(asked.min(speed / cruise))
}

/// **The response readouts** (S2): counters only, no rule reads them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VigilLedger {
    /// Scan pauses, flights begun, alarm bolts, orientations, freezes and refuge samples
    /// (an arboreal body walking to a tree in flight), by lineage.
    pub scans: [u64; Founder::COUNT],
    pub flees: [u64; Founder::COUNT],
    pub alarm_bolts: [u64; Founder::COUNT],
    pub orients: [u64; Founder::COUNT],
    pub freezes: [u64; Founder::COUNT],
    pub refuges: [u64; Founder::COUNT],
    /// Strikes that met a prey of a live lineage that had sensed the striker, and that had
    /// not, by the prey's lineage.
    pub aware_strikes: [u64; Founder::COUNT],
    pub unaware_strikes: [u64; Founder::COUNT],
    /// Pack chase samples stalking, rushing, and frozen by a prey's scan.
    pub stalk_samples: u64,
    pub rush_samples: u64,
    pub stalk_freezes: u64,
}
