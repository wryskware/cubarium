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
//!   `asleep_loudness` (and it is in range). Recording is all: waking on it is CALLS-REST's.
//!   Unlike today's alarm wake (`rhythm::alarm_wakes`: a running body's emission over the
//!   3D distance, no terrain ray), this path marches the ray.
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
/// Veilhound's hunt cue and regroup howl, the seedporter's alarm (later, primate-style). The
/// frondgrazer's bolt alarm stays where it is until CALLS-REST ports it.
pub fn kinds_of(founder: Founder) -> &'static [&'static str] {
    match founder {
        Founder::Loftstrider => &["alarm_snort", "distress_bleat"],
        Founder::Chorister => &["hunt_cue", "regroup_howl"],
        Founder::Seedporter => &["alarm"],
        _ => &[],
    }
}

/// **A lineage's calls and hearing of them** (module note). Placeholders, untuned.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CallTable {
    /// A call of loudness 1 is heard this far on open ground, metres (0: no calls).
    pub range_m: f64,
    /// Each kind's loudness ([`kinds_of`] order).
    pub loudness: [f64; MAX_CALL_KINDS],
    /// A caller emits a kind at most once per this, seconds.
    pub cooldown_s: f64,
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
}

impl Default for CallTable {
    /// No calls.
    fn default() -> CallTable {
        CallTable {
            range_m: 0.0,
            loudness: [1.0; MAX_CALL_KINDS],
            cooldown_s: 5.0,
            records: 4,
            keep_s: 10.0,
            refractory_s: 2.0,
            dedup_m: 2.0,
            asleep_loudness: 0.5,
            recognise_voice: false,
        }
    }
}

/// Each lineage's table: a range for the lineages with kinds (none emits yet).
pub fn calls_of(founder: Founder) -> CallTable {
    let d = CallTable::default();
    match founder {
        // An alarm snort carries; a calf's bleat less. Mother and calf know each other.
        Founder::Loftstrider => CallTable { range_m: 40.0, loudness: [1.0, 0.7], recognise_voice: true, ..d },
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
    if kind >= kinds_of(f).len() || !(t.range_m > 0.0) {
        return false;
    }
    let key = (a.id, kind as u8);
    let cool = (t.cooldown_s / DT).round() as u64;
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
            let asleep = h.state == crate::State::Asleep;
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
