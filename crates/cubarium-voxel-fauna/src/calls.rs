//! **Calls and hearing** (CONTRACTS, behaviour-AI §2.1 and §11.2): the call substrate.
//!
//! - **Emission** ([`emit`]): a body of a lineage with call kinds ([`kinds_of`]) emits one, at
//!   most once per `cooldown_s` per kind, into a short buffer of events (session state, not
//!   saved). No lineage emits a call yet: a brain's call action or a later package's rules do.
//! - **Hearing** ([`hear`], once a tick before the senses): each event is heard by the bodies
//!   of the caller's lineage within `range_m × loudness` of it — collected by its own range,
//!   not through the nearest-animal candidate cap (`perception.rs`'s `max_candidates`) — through
//!   the pair's terrain ray: blocked, the loudness is multiplied by the perception's
//!   `occlusion`, so its range shrinks the same way.
//! - **A sleeper** records a call only when its loudness after occlusion is at least
//!   `asleep_loudness` (and it is in range); light rest hears as awake (CALLS-REST).
//!   Waking on it is CALLS-REST's one wake rule (`rhythm::wakes`), which marches the ray for a
//!   footfall too.
//! - **The record** ([`CallRecord`]) holds the kind, where the call came from (bearing and
//!   proximity are encoded from it), its age, and the caller's relation when the hearer
//!   recognises the voice (`recognise_voice`: its mother or its own young); never the
//!   caller's id. A hearer keeps at most `records`, a call of a kind from within `dedup_m` of a
//!   record refreshes that record (ignored within `refractory_s` of it), and a record older than
//!   `keep_s` is dropped. Saved with the world (harmless to lose; simpler than not saving).

use serde::{Deserialize, Serialize};

use cubarium_voxel::{DT, VoxelView};

use crate::{Animal, Fauna, Founder};

/// The most call kinds a lineage has.
pub const MAX_CALL_KINDS: usize = 2;

/// A lineage's call kinds (§2.1): the loftstrider's alarm snort and distress bleat, the
/// Veilhound's hunt cue and regroup howl, the seedporter's alarm (later, primate-style), and
/// (CALLS-REST step 2) the frondgrazer's bolt alarm, a sensed kind ([`CallTable::sensed`]).
pub fn kinds_of(founder: Founder) -> &'static [&'static str] {
    match founder {
        Founder::Browser => &["bolt_alarm"],
        Founder::Loftstrider => &["alarm_snort", "distress_bleat"],
        Founder::Chorister => &["hunt_cue", "regroup_howl"],
        Founder::Seedporter => &["alarm"],
        _ => &[],
    }
}

/// **Who heeds a call kind** (CALLS-REST): which of the caller's lineage react to it through
/// their heuristics, and which sleepers it wakes. Every hearer still records it (a brain reads
/// every record it has).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Heed {
    /// No one: recorded only.
    #[default]
    Nobody,
    /// Every hearer of the lineage.
    Everyone,
    /// Hearers at or above the lineage's adult body (`birth_body`).
    Adults,
}

/// **A lineage's calls and hearing of them** (module note). Placeholders, untuned.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CallTable {
    /// A call of loudness 1 is heard this far on open ground, metres (0: no calls).
    pub range_m: f64,
    /// Each kind's loudness ([`kinds_of`] order).
    pub loudness: [f64; MAX_CALL_KINDS],
    /// A caller emits each kind at most once per this, seconds ([`kinds_of`] order).
    pub cooldown_s: [f64; MAX_CALL_KINDS],
    /// Call records a hearer keeps.
    pub records: u32,
    /// A record is dropped this long after its call, seconds.
    pub keep_s: f64,
    /// A call of a kind within `dedup_m` of a record younger than this is ignored, seconds.
    pub refractory_s: f64,
    /// Calls of a kind from within this of a record's point are the same caller's, metres.
    pub dedup_m: f64,
    /// A sleeper records a call only at least this loud after occlusion.
    pub asleep_loudness: f64,
    /// Whether a hearer knows its mother's and its own young's voices (a switch).
    pub recognise_voice: bool,
    /// **CALLS-REST: who heeds each kind** ([`Heed`], a switch per kind, no registry row):
    /// who reacts to it awake and whom it wakes.
    pub heed: [Heed; MAX_CALL_KINDS],
    /// **CALLS-REST: the heuristic's calls.** A body calls a kind when it senses a pressing
    /// threat within this, metres ([`kinds_of`] order; 0: its heuristic never calls that
    /// kind). The loftstrider's first kind (the snort) needs the threat seen; its second (the
    /// bleat) is a calf's, chased within this.
    pub trigger_m: [f64; MAX_CALL_KINDS],
    /// A young body under this share of its lineage's adult body (`birth_body`) is a **calf**:
    /// it bleats (the second kind) and does not snort; an adult or a juvenile snorts. Fraction.
    pub calf_below: f64,
    /// A calf with a pressing threat and no adult-sized kin known within this is **alone with
    /// a threat**, and bleats, metres; 0 never.
    pub alone_m: f64,
    /// **CALLS-REST: the hearer's reaction** to a heeded call holds while its record is younger
    /// than this, seconds ([`kinds_of`] order; 0: no reaction).
    pub heed_s: [f64; MAX_CALL_KINDS],
    /// A hearer that does not recognise the caller's voice begins its reaction this long after
    /// it first heard the call, seconds ([`kinds_of`] order): the mother's head start on her
    /// calf's bleat.
    pub stranger_delay_s: [f64; MAX_CALL_KINDS],
    /// A hearer of the first kind (an alarm) walks to the nearest kin it knows until within
    /// this, then stands head up facing the call, metres.
    pub gather_m: f64,
    /// A hearer of the second kind (a distress call) walks to where it came from until within
    /// this, metres.
    pub arrive_m: f64,
    /// **CALLS-REST: a sensed kind** (a switch per kind, no registry row): the call is the
    /// caller's loud run itself, heard through the hearer's own hearing at its remote sensing
    /// and not by range: the frondgrazer's bolt alarm ([`sense_alarms`]). Never emitted into
    /// the buffer.
    pub sensed: [bool; MAX_CALL_KINDS],
}

impl Default for CallTable {
    /// No calls.
    fn default() -> CallTable {
        CallTable {
            range_m: 0.0,
            loudness: [1.0; MAX_CALL_KINDS],
            cooldown_s: [5.0; MAX_CALL_KINDS],
            records: 4,
            keep_s: 10.0,
            refractory_s: 2.0,
            dedup_m: 2.0,
            asleep_loudness: 0.5,
            recognise_voice: false,
            heed: [Heed::Nobody; MAX_CALL_KINDS],
            trigger_m: [0.0; MAX_CALL_KINDS],
            calf_below: 0.5,
            alone_m: 0.0,
            heed_s: [0.0; MAX_CALL_KINDS],
            stranger_delay_s: [0.0; MAX_CALL_KINDS],
            gather_m: 0.0,
            arrive_m: 0.0,
            sensed: [false; MAX_CALL_KINDS],
        }
    }
}

impl CallTable {
    /// Whether kind `k` is heard through the hearer's own senses rather than by range
    /// ([`CallTable::sensed`]).
    pub fn sensed_kind(&self, k: usize) -> bool {
        self.sensed.get(k).copied().unwrap_or(false)
    }
}

/// Each lineage's table: a range for the lineages with kinds (none emits yet).
///
/// **CALLS-REST, the loftstrider** (Wrysk's topic 1 and 7, behaviour-AI §2.1), placeholders,
/// untuned:
/// - **the alarm snort**: an adult or juvenile that sees a pressing threat within 8 m (its
///   flee start is 6, its day sight 12) snorts at most every 5 s; every hearer heeds it for
///   10 s: head up, it walks to its nearest known kin until within 2.5 m (about a body length
///   and a half), then stands facing the call;
/// - **the distress bleat**: a calf (under half the adult body) chased within its 6 m flee
///   start, struck, or with a threat known and no adult within 6 m bleats; adults heed it for
///   15 s and walk to it until within 1.5 m. Its mother knows its voice and goes at once; any
///   other adult 3 s later (giraffe and elephant mothers answer their own calf's call first;
///   allomothers follow);
/// - records keep 20 s, so the slowest stranger's reaction (3 + 15 s) fits.
pub fn calls_of(founder: Founder) -> CallTable {
    let d = CallTable::default();
    match founder {
        // An alarm snort carries; a calf's bleat less. Mother and calf know each other.
        Founder::Loftstrider => CallTable {
            range_m: 40.0,
            loudness: [1.0, 0.7],
            recognise_voice: true,
            keep_s: 20.0,
            heed: [Heed::Everyone, Heed::Adults],
            trigger_m: [8.0, 6.0],
            calf_below: 0.5,
            alone_m: 6.0,
            heed_s: [10.0, 15.0],
            stranger_delay_s: [0.0, 3.0],
            gather_m: 2.5,
            arrive_m: 1.5,
            ..d
        },
        // CALLS-REST step 2: today's bolt alarm on the substrate, unchanged: a sensed kind,
        // heard by every hearer (§5).
        Founder::Browser => CallTable { sensed: [true, false], heed: [Heed::Everyone, Heed::Nobody], ..d },
        // A hunt cue is quiet; a howl carries far.
        Founder::Chorister => CallTable { range_m: 60.0, loudness: [0.3, 1.0], ..d },
        Founder::Seedporter => CallTable { range_m: 30.0, loudness: [1.0, 0.0], recognise_voice: true, ..d },
        _ => d,
    }
}

/// One call in the buffer (session state).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CallEvent {
    pub caller: u64,
    pub lineage: Founder,
    pub kind: u8,
    pub x: f64,
    pub y_m: f64,
    pub z: f64,
    pub tick: u64,
}

/// What a hearer knows of a call (module note).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CallRecord {
    pub kind: u8,
    /// Where the call came from.
    pub x: f64,
    pub z: f64,
    pub y_m: f64,
    /// Its loudness as heard (after occlusion), and `1 − d / reach`.
    pub loudness: f64,
    pub proximity: f64,
    pub tick: u64,
    /// 0 not recognised, 1 its mother, 2 its own young.
    pub relation: u8,
    /// Heard asleep.
    pub asleep: bool,
    /// CALLS-REST: a sensed kind's caller's heading as heard (the frondgrazer bolts along it),
    /// radians; 0 for a ranged call.
    pub heading: f64,
}

/// A body's call records.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Heard {
    pub records: Vec<CallRecord>,
}

/// **Emit** call kind `kind` from body `i`: `false` when its lineage has no such kind, no
/// range, or the kind is cooling down.
pub fn emit(fauna: &mut Fauna, i: usize, kind: usize) -> bool {
    let a = &fauna.animals[i];
    let Some(f) = a.founder else { return false };
    let t = fauna.config.founder(f).calls;
    if kind >= kinds_of(f).len() || !(t.range_m > 0.0) || t.sensed_kind(kind) {
        return false;
    }
    let key = (a.id, kind as u8);
    let cool = (t.cooldown_s[kind] / DT).round() as u64;
    if fauna.last_call.get(&key).is_some_and(|&at| fauna.tick < at + cool.max(1)) {
        return false;
    }
    let v = fauna.world_geometry.map_or(1.0, |(_, v)| v);
    let head = crate::perception::base_m(a, v) + crate::body_of(&fauna.config, a).map_or(0.0, |g| g.height_m);
    fauna.calls.push(CallEvent { caller: a.id, lineage: f, kind: kind as u8, x: a.pose.x, y_m: head, z: a.pose.z, tick: fauna.tick });
    fauna.last_call.insert(key, fauna.tick);
    true
}

fn relation(t: &CallTable, hearer: &Animal, caller: &Animal) -> u8 {
    if !t.recognise_voice {
        0
    } else if crate::mother_body(hearer) == Some(caller.id) {
        1
    } else if crate::mother_body(caller) == Some(hearer.id) {
        2
    } else {
        0
    }
}

/// **Hear** every call in the buffer (module note), then empty it; drop old records.
pub(crate) fn hear(fauna: &mut Fauna, view: &VoxelView<'_>) {
    let now = fauna.tick;
    if !fauna.heard.is_empty() && now % 16 == 0 {
        let alive: rustc_hash::FxHashSet<u64> = fauna.animals.iter().map(|a| a.id).collect();
        fauna.heard.retain(|id, _| alive.contains(id));
        let keep = |f: Option<Founder>| f.map_or(0.0, |f| fauna.config.founder(f).calls.keep_s);
        let keeps: rustc_hash::FxHashMap<u64, f64> = fauna.animals.iter().map(|a| (a.id, keep(a.founder))).collect();
        for (id, h) in fauna.heard.iter_mut() {
            let k = keeps.get(id).copied().unwrap_or(0.0);
            h.records.retain(|r| now.saturating_sub(r.tick) as f64 * DT <= k);
        }
        fauna.heard.retain(|_, h| !h.records.is_empty());
    }
    if fauna.calls.is_empty() {
        return;
    }
    let events = std::mem::take(&mut fauna.calls);
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let occlusion = fauna.config.perception.occlusion.max(0.0);
    for e in events {
        let t = fauna.config.founder(e.lineage).calls;
        let loud = t.loudness.get(usize::from(e.kind)).copied().unwrap_or(0.0).max(0.0);
        let reach_open = t.range_m * loud;
        if !(reach_open > 0.0) {
            continue;
        }
        let Ok(c) = fauna.animals.binary_search_by_key(&e.caller, |a| a.id) else { continue };
        let caller = fauna.animals[c];
        let mut got: Vec<(u64, CallRecord)> = Vec::new();
        for h in &fauna.animals {
            if h.id == e.caller || h.founder != Some(e.lineage) || h.attached.is_some() {
                continue;
            }
            let hy = crate::perception::base_m(h, v) + crate::body_of(&fauna.config, h).map_or(0.0, |g| g.height_m);
            let (dx, dz) = crate::pose::wrapped_delta(e.x, e.z, h.pose.x, h.pose.z, ring);
            let dy = hy - e.y_m;
            let d = (dx * dx + dy * dy + dz * dz).sqrt();
            if d > reach_open {
                continue;
            }
            let open = crate::perception::line_clear(view, e.caller, (e.x, e.y_m, e.z), (h.pose.x, hy, h.pose.z));
            let heard_loud = if open { loud } else { loud * occlusion };
            let reach = t.range_m * heard_loud;
            if !(d <= reach) {
                continue;
            }
            // CALLS-REST: only deep sleep is hard of hearing; light rest hears as awake.
            let asleep = crate::sleep::deep_asleep(h);
            if asleep && heard_loud < t.asleep_loudness {
                continue;
            }
            got.push((
                h.id,
                CallRecord {
                    kind: e.kind,
                    x: e.x,
                    z: e.z,
                    y_m: e.y_m,
                    loudness: heard_loud,
                    proximity: if reach > 0.0 { (1.0 - d / reach).clamp(0.0, 1.0) } else { 0.0 },
                    tick: e.tick,
                    relation: relation(&t, h, &caller),
                    asleep,
                    heading: 0.0,
                },
            ));
        }
        for (id, r) in got {
            let rec = fauna.heard.entry(id).or_default();
            let same = rec.records.iter().position(|o| {
                let (dx, dz) = crate::pose::wrapped_delta(o.x, o.z, r.x, r.z, ring);
                o.kind == r.kind && (dx * dx + dz * dz).sqrt() <= t.dedup_m
            });
            match same {
                Some(k) if r.tick.saturating_sub(rec.records[k].tick) as f64 * DT < t.refractory_s => {}
                Some(k) => rec.records[k] = r,
                None if rec.records.len() < t.records as usize => rec.records.push(r),
                None => {
                    if let Some(k) = (0..rec.records.len()).min_by_key(|&k| rec.records[k].tick) {
                        rec.records[k] = r;
                    }
                }
            }
        }
    }
}

/// Body `id`'s call records.
pub fn heard(fauna: &Fauna, id: u64) -> &[CallRecord] {
    fauna.heard.get(&id).map_or(&[], |h| h.records.as_slice())
}

/// The `Calls` block's channels per kind.
pub const PER_KIND: usize = 6;

/// **Encode** body `a`'s freshest record of each kind as `valid, sin, cos, proximity, fresh,
/// mine` (mine: its mother's or its own young's voice).
pub(crate) fn encode(fauna: &Fauna, a: &Animal, frame: &crate::observe_v3::Frame, out: &mut [f64]) {
    let Some(f) = a.founder else { return };
    let keep = fauna.config.founder(f).calls.keep_s;
    for (k, chunk) in out.chunks_mut(PER_KIND).enumerate() {
        chunk.fill(0.0);
        let best = heard(fauna, a.id).iter().filter(|r| usize::from(r.kind) == k).max_by_key(|r| r.tick);
        let Some(r) = best else { continue };
        let age_s = frame.tick.saturating_sub(r.tick) as f64 * DT;
        if !(keep > 0.0) || age_s > keep {
            continue;
        }
        let (s, c, _) = frame.toward(r.x, r.z);
        chunk.copy_from_slice(&[1.0, s, c, r.proximity, (1.0 - age_s / keep).clamp(0.0, 1.0), f64::from(r.relation != 0)]);
    }
}

// ---- CALLS-REST: the heuristics' calls and reactions ----

/// What the herd response knows of its threats this sample, from its own record.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ThreatSense {
    /// The nearest pressing threat it senses (seen moving or near, heard, felt), metres.
    pub pressing_m: Option<f64>,
    /// The nearest pressing threat it sees, metres.
    pub seen_m: Option<f64>,
}

/// Whether body `a` of lineage `f` is a calf for the calls ([`CallTable::calf_below`]).
pub fn is_calf(config: &crate::FaunaConfig, f: Founder, a: &Animal) -> bool {
    let p = config.founder(f);
    a.body < p.calls.calf_below * p.core.birth_body
}

/// Whether body `a` heeds kind `k` of its lineage `f` ([`CallTable::heed`]).
pub fn heeds(config: &crate::FaunaConfig, f: Founder, a: &Animal, k: usize) -> bool {
    let p = config.founder(f);
    match p.calls.heed.get(k).copied().unwrap_or_default() {
        Heed::Nobody => false,
        Heed::Everyone => true,
        Heed::Adults => a.body >= p.core.birth_body,
    }
}

/// **The heuristic's calls** (module note, §2.1) for body `i` under its herd response: an
/// adult or juvenile that sees a pressing threat within the first kind's `trigger_m` calls it
/// (the alarm); a calf chased within the second kind's `trigger_m`, `struck` since its last
/// sample, or with a pressing threat known and no adult-sized kin known within `alone_m` calls
/// the second (its distress). Reads only its record. Returns the kind emitted.
pub(crate) fn heuristic_calls(fauna: &mut Fauna, i: usize, ts: ThreatSense, struck: bool, ring: f64) -> Option<usize> {
    let a = fauna.animals[i];
    let f = a.founder?;
    let phys = *fauna.config.founder(f);
    let t = phys.calls;
    let n = kinds_of(f).len();
    if !(t.range_m > 0.0) || n == 0 {
        return None;
    }
    if !is_calf(&fauna.config, f, &a) {
        let alarm = t.trigger_m[0] > 0.0 && ts.seen_m.is_some_and(|d| d <= t.trigger_m[0]);
        return (alarm && emit(fauna, i, 0)).then_some(0);
    }
    if n < 2 {
        return None;
    }
    let chased = t.trigger_m[1] > 0.0 && ts.pressing_m.is_some_and(|d| d <= t.trigger_m[1]);
    let alone = t.alone_m > 0.0 && ts.pressing_m.is_some() && {
        let fresh = 2 * fauna.config.perception.stagger_ticks();
        let adult_len = phys.body_at(phys.core.birth_body).length_m;
        let now = fauna.tick;
        !fauna.percepts.get(&a.id).is_some_and(|p| {
            p.tracks().iter().any(|k| {
                k.class == crate::perception::class::KIN && now.saturating_sub(k.tick) <= fresh && k.size_m >= adult_len * (1.0 - 1e-9) && {
                    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, k.x, k.z, ring);
                    dx.hypot(dz) <= t.alone_m
                }
            })
        })
    };
    ((chased || struck || alone) && emit(fauna, i, 1)).then_some(1)
}

/// **A hearer's reaction** to a heeded call (module note), from its own records.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Reaction {
    /// Walk this way (metres on the ring, from it).
    Walk { dx: f64, dz: f64 },
    /// Stand, head up, facing this heading (radians).
    Face { heading: f64 },
}

/// **The heeded call** of body `i` (LS-DEFEND: shared by [`reaction`] and the loftstrider's
/// defence): from its own records, the one it acts on — a recognised voice first, then the
/// later kind (a distress call over an alarm), then the freshest — younger than its kind's
/// `heed_s` and, for a voice it does not recognise, heard at least `stranger_delay_s` ago
/// (`heard_from`: the tick its current heeding began; 0 none). Returns it and the new
/// `heard_from` (0 when it heeds nothing).
pub(crate) fn heeded(fauna: &Fauna, i: usize, heard_from: u64) -> (Option<CallRecord>, u64) {
    let a = &fauna.animals[i];
    let Some(f) = a.founder else { return (None, 0) };
    let t = fauna.config.founder(f).calls;
    let now = fauna.tick;
    let age = |r: &CallRecord| now.saturating_sub(r.tick) as f64 * DT;
    let live: Vec<&CallRecord> = heard(fauna, a.id)
        .iter()
        .filter(|r| {
            let k = usize::from(r.kind);
            k < MAX_CALL_KINDS && !t.sensed_kind(k) && t.heed_s[k] > 0.0 && heeds(&fauna.config, f, a, k) && age(r) <= t.heed_s[k]
        })
        .collect();
    if live.is_empty() {
        return (None, 0);
    }
    let from = if heard_from == 0 { now } else { heard_from };
    let since = now.saturating_sub(from) as f64 * DT;
    let best = live
        .into_iter()
        .filter(|r| r.relation != 0 || since >= t.stranger_delay_s[usize::from(r.kind)])
        .max_by(|p, q| (p.relation != 0).cmp(&(q.relation != 0)).then(p.kind.cmp(&q.kind)).then(p.tick.cmp(&q.tick)));
    (best.copied(), from)
}

/// **The reaction** of body `i` to the calls it has heard (module note): the heeded record it
/// acts on — a recognised voice first, then the later kind (a distress call over an alarm), then
/// the freshest — younger than its kind's `heed_s`, and, for a voice it does not recognise,
/// heard at least `stranger_delay_s` ago (`heard_from`: the tick its current heeding began, kept
/// by the caller; 0 none). An alarm walks it to its nearest known kin, then stands it facing
/// the call; a distress call walks it to where the call came from, until within `arrive_m`.
/// Returns the reaction and the new `heard_from`.
pub(crate) fn reaction(fauna: &Fauna, i: usize, heard_from: u64, ring: f64) -> (Option<Reaction>, u64) {
    let a = &fauna.animals[i];
    let Some(f) = a.founder else { return (None, 0) };
    let t = fauna.config.founder(f).calls;
    let now = fauna.tick;
    let (best, from) = heeded(fauna, i, heard_from);
    let Some(r) = best else { return (None, from) };
    let (cx, cz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, r.x, r.z, ring);
    let out = if r.kind == 0 {
        // The alarm: to the nearest kin it knows, then head up facing the call.
        let fresh = 2 * fauna.config.perception.stagger_ticks();
        let kin = fauna.percepts.get(&a.id).and_then(|p| {
            p.tracks()
                .iter()
                .filter(|k| k.class == crate::perception::class::KIN && k.id != a.id && now.saturating_sub(k.tick) <= fresh)
                .map(|k| crate::pose::wrapped_delta(a.pose.x, a.pose.z, k.x, k.z, ring))
                .min_by(|p, q| p.0.hypot(p.1).total_cmp(&q.0.hypot(q.1)))
        });
        match kin {
            Some((dx, dz)) if dx.hypot(dz) > t.gather_m => Some(Reaction::Walk { dx, dz }),
            _ => Some(Reaction::Face { heading: cx.atan2(cz) }),
        }
    } else if cx.hypot(cz) > t.arrive_m {
        Some(Reaction::Walk { dx: cx, dz: cz })
    } else {
        None
    };
    (out, from)
}

// ---- CALLS-REST step 2: the frondgrazer's bolt alarm, a sensed kind ----

/// Whether track `t` in body `a`'s record (lineage `f`) is a kin heard running: its received
/// loudness times its distance (its emission, at least `hearing_near_m`) at least
/// `alarm_emission_factor` times that kin's walking emission (cruise × length). Today's herd
/// alarm rule (S2), unchanged.
fn bolting_kin(config: &crate::FaunaConfig, f: Founder, t: &crate::Track, now: u64) -> bool {
    let phys = config.founder(f);
    let vg = phys.vigilance;
    let pc = config.perception;
    let walk = |size: f64| (phys.cruise_body_lengths_per_s * size).max(0.0).powf(pc.hearing_speed_exp) * size.max(0.0).powf(pc.hearing_size_exp);
    t.class == crate::perception::class::KIN
        && t.modes & crate::perception::HEARING != 0
        && t.tick + 2 * pc.stagger_ticks() >= now
        && t.loud * t.d_m.max(pc.hearing_near_m) >= vg.alarm_emission_factor * walk(t.size_m)
}

/// **Hear the sensed kinds** at body `id`'s remote sensing (CALLS-REST step 2): for each of its
/// lineage's sensed kinds ([`CallTable::sensed`]) the record of that kind is replaced by the
/// sensing's: the nearest kin it heard running loud enough ([`bolting_kin`]), at the runner's
/// point and heading, or none. So the record says exactly what the sensing's tracks say, as
/// the herd alarm read them before (S2).
pub(crate) fn sense_alarms(fauna: &mut Fauna, id: u64, f: Founder) {
    let t = fauna.config.founder(f).calls;
    let n = kinds_of(f).len();
    if !(0..n).any(|k| t.sensed_kind(k)) {
        return;
    }
    let now = fauna.tick;
    let Some(p) = fauna.percepts.get(&id) else { return };
    let found = p.tracks().iter().find(|tr| bolting_kin(&fauna.config, f, tr, now)).copied();
    for k in (0..n).filter(|&k| t.sensed_kind(k)) {
        let kind = k as u8;
        if let Some(h) = fauna.heard.get_mut(&id) {
            h.records.retain(|r| r.kind != kind);
        }
        if let Some(tr) = found {
            let r = CallRecord {
                kind,
                x: tr.x,
                z: tr.z,
                y_m: tr.y_m,
                loudness: tr.loud,
                proximity: 0.0,
                tick: tr.tick,
                relation: 0,
                asleep: false,
                heading: tr.heading,
            };
            fauna.heard.entry(id).or_default().records.push(r);
        }
    }
    if fauna.heard.get(&id).is_some_and(|h| h.records.is_empty()) {
        fauna.heard.remove(&id);
    }
}

/// **A sensed alarm heard** by body `a` (CALLS-REST step 2): its freshest record of its
/// lineage's first sensed kind, heard at a sensing within the last two staggers (as the herd
/// alarm read a track), when it heeds that kind; `None` else. The frondgrazer's herd alarm
/// and its observation's kin-alarm channel read this.
pub fn sensed_alarm(fauna: &Fauna, a: &Animal, now: u64) -> Option<CallRecord> {
    let f = a.founder?;
    let t = fauna.config.founder(f).calls;
    let k = (0..kinds_of(f).len()).find(|&k| t.sensed_kind(k))?;
    if !heeds(&fauna.config, f, a, k) {
        return None;
    }
    let fresh = 2 * fauna.config.perception.stagger_ticks();
    heard(fauna, a.id).iter().filter(|r| usize::from(r.kind) == k && r.tick + fresh >= now).max_by_key(|r| r.tick).copied()
}
