//! **Contract v3's blocks** (`design/brain-contract-v3-proposal-2026-10-08.md` §2.1, P1): the
//! slots after a lineage's v2 layout ([`crate::Module::tail`]), encoded from the body's
//! percept record and its own state by [`crate::manifest::V3_ENCODING_RULE`].
//!
//! # What is read
//!
//! - **The record** ([`Percept`]): per class the best track (threat, kin, prey) and the
//!   latest sensing's counts, filled by the remote senses for every live lineage whatever
//!   drives it. Its tick-rate parts — the tremor, the scent, the light — are refreshed in
//!   a heuristic body's reflexes, so for the encoding they are read **fresh**, read only,
//!   into a copy ([`fresh_record`]): a brain body, which runs no reflex, sees them as a
//!   heuristic one would. Nothing here writes the world.
//! - **The body**: its stomach, latch, wound, stamina, hunker, water state, mode and pack.
//! - **The clock** ([`Clock`]): daylight and the day's phase.
//!
//! # Bearings
//!
//! Every bearing is recomputed at encoding from the body's pose now to the item's
//! last-known point ([`Frame::toward`]): positive to the right, as `sin` (the item's
//! component to the right) and `cos` (its component ahead). Coordinates stay here; only the
//! bearing, the proximity and the flags reach the vector.
//!
//! # Places (P6)
//!
//! Each `Place(kind)` block is five channels (`valid, sin, cos, proximity, fresh`) filled by
//! [`place`]. Item 24 (training plan §6) decided what fills them: the nearest such place the
//! body could perceive, else the one it remembers using, fading if not revisited. Only the
//! den is always known. Other kinds read the reflex queries cached by [`crate::places`]
//! on remote-sense ticks, else the fading memory of a home place the body used.

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::{FloraView, Site};

use crate::manifest::{HEIGHT_REFERENCE_M, Manifest, SCENT_REFERENCE, TREMOR_REFERENCE};
use crate::perception::{HEARING, Percept, Track, class, posture};
use crate::scent::{ScentChannel, ScentReading};
use crate::{Animal, Fauna, MobilityMode};

/// The clock a tail reads: the sky's daylight (with the clock on) and the day's phase (0
/// midnight, 0.5 noon).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clock {
    pub clock_on: bool,
    pub daylight: f64,
    pub day_phase: f64,
}

impl Clock {
    /// The daylight a body reads: the sky's, or 1 with the clock off.
    pub fn daylight(&self) -> f64 {
        if self.clock_on { self.daylight.clamp(0.0, 1.0) } else { 1.0 }
    }
}

/// Where an encoding is taken from: the body's centre on the ring and its heading, the
/// ring's length and the tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub z: f64,
    pub heading: f64,
    pub ring_m: f64,
    pub tick: u64,
}

impl Frame {
    /// The frame of body `a` in a world `ring_m` round at `tick`.
    pub fn of(a: &Animal, ring_m: f64, tick: u64) -> Frame {
        Frame { x: a.pose.x, z: a.pose.z, heading: a.pose.heading_rad, ring_m, tick }
    }

    /// `(sin b, cos b, d)` of the point `(x, z)`: its bearing from the heading (positive to
    /// the right) and its horizontal distance. A point at the body reads straight ahead.
    pub fn toward(&self, x: f64, z: f64) -> (f64, f64, f64) {
        let (dx, dz) = crate::pose::wrapped_delta(self.x, self.z, x, z, self.ring_m);
        let d = (dx * dx + dz * dz).sqrt();
        if !(d > 1e-12) {
            return (0.0, 1.0, 0.0);
        }
        let (fx, fz) = (self.heading.sin(), self.heading.cos());
        let (rx, rz) = (fz, -fx);
        ((dx * rx + dz * rz) / d, (dx * fx + dz * fz) / d, d)
    }
}

fn clamp01(v: f64) -> f64 {
    if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 }
}

fn clamp_sym(v: f64) -> f64 {
    if v.is_finite() { v.clamp(-1.0, 1.0) } else { 0.0 }
}

/// `1 − d / range`, clamped; 0 for a range of 0.
pub fn proximity(d: f64, range_m: f64) -> f64 {
    if range_m > 0.0 { clamp01(1.0 - d / range_m) } else { 0.0 }
}

/// `x / (x + reference)` for a non-negative level.
pub fn level(x: f64, reference: f64) -> f64 {
    let x = x.max(0.0);
    if x + reference > 0.0 { clamp01(x / (x + reference)) } else { 0.0 }
}

fn site_xz(s: Site, v: f64) -> (f64, f64) {
    ((f64::from(s.x) + 0.5) * v, (f64::from(s.z) + 0.5) * v)
}

/// **A sensed body's slot** (`valid, sin, cos, proximity, moving, still, fresh`): empty for
/// no track or one older than `fresh_s`; else the bearing and proximity from the body now to
/// its last-known point, whether it moved, whether its posture read still (REST or ASLEEP),
/// and `1 − age / fresh_s`.
pub fn slot(t: &Track, frame: &Frame, range_m: f64, fresh_s: f64) -> [f64; 7] {
    if !t.known || !(fresh_s > 0.0) {
        return [0.0; 7];
    }
    let age_s = t.age(frame.tick) as f64 * cubarium_voxel::DT;
    if age_s > fresh_s {
        return [0.0; 7];
    }
    let (s, c, d) = frame.toward(t.x, t.z);
    [
        1.0,
        s,
        c,
        proximity(d, range_m),
        f64::from(t.moving),
        f64::from(t.posture == posture::REST || t.posture == posture::ASLEEP),
        clamp01(1.0 - age_s / fresh_s),
    ]
}

/// `min(n, 4) / 4` of the latest sensing's tracks of `class`.
pub fn count(p: &Percept, class: u8) -> f64 {
    let n = p.tracks().iter().filter(|t| t.class == class).count();
    n.min(4) as f64 / 4.0
}

/// One scent channel along the body: the level, and the gradient forward and right.
pub fn scent(r: &ScentReading, frame: &Frame) -> [f64; 3] {
    let (fx, fz) = (frame.heading.sin(), frame.heading.cos());
    let (rx, rz) = (fz, -fx);
    let along = |g: f64| clamp_sym((g / SCENT_REFERENCE).tanh());
    [level(r.level, SCENT_REFERENCE), along(r.gx * fx + r.gz * fz), along(r.gx * rx + r.gz * rz)]
}

/// The footfall felt left and right.
pub fn tremor(t: [f64; 2]) -> [f64; 2] {
    [level(t[0], TREMOR_REFERENCE), level(t[1], TREMOR_REFERENCE)]
}

/// **The record read fresh** for an encoding (module note): body `i`'s record, with its
/// tick-rate parts — tremor, scent and the photoreceptor's reading — computed now, read
/// only. A lineage whose table is not live reads the interim tremor rule. The light needs
/// the beacons refreshed when due (the step does it before its read side).
pub(crate) fn fresh_record(fauna: &Fauna, i: usize, view: &VoxelView<'_>, clock: Clock) -> Percept {
    let a = &fauna.animals[i];
    let mut p = fauna.percepts.get(&a.id).copied().unwrap_or_default();
    let Some(founder) = a.founder else { return p };
    let table = fauna.config.founder(founder).senses;
    if table.live {
        p.tremor = crate::perception::tremor_reading(fauna, view, a, &table);
        p.scent = fauna.scent.read(view, a);
    } else {
        p.tremor = fauna.view().tremor(view, a.id);
    }
    if let Some(light) = crate::light::light_reading(fauna, a, view, clock.daylight()) {
        p.light = light;
    }
    p
}

/// **Encode body `i`'s v3 blocks** into `obs` (its manifest's width), from the fresh record
/// ([`fresh_record`]). Each tail module is written at its own slot; a block a body cannot
/// fill reads all zero, validity included.
pub(crate) fn encode(
    fauna: &Fauna,
    i: usize,
    view: &VoxelView<'_>,
    _fv: &FloraView<'_>,
    manifest: &Manifest,
    clock: Clock,
    obs: &mut [f64],
) {
    let a = &fauna.animals[i];
    let Some(founder) = a.founder else { return };
    let phys = fauna.config.founder(founder);
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let frame = Frame::of(a, ring, fauna.tick);
    let p = fresh_record(fauna, i, view, clock);
    let (range, fresh) = (manifest.slot_range_m, manifest.fresh_s);
    for m in manifest.modules.iter().filter(|m| m.tail) {
        let out = &mut obs[m.offset..m.offset + m.width];
        match m.name {
            "SelfV3" => {
                let k = phys.gut.capacity(a.body);
                let s = a.stomach.total().organic;
                let phase = std::f64::consts::TAU * clock.day_phase;
                out.copy_from_slice(&[
                    if k > 0.0 { clamp01(s / k) } else { 0.0 },
                    f64::from(a.eating),
                    clamp01(a.wound),
                    clamp01(1.0 - a.founder_state.sprint_spent),
                    clock.daylight(),
                    phase.sin(),
                    phase.cos(),
                ]);
            }
            "Threat" => {
                out[..7].copy_from_slice(&slot(&p.best[usize::from(class::THREAT)], &frame, range, fresh));
                out[7] = count(&p, class::THREAT);
            }
            "Kin" => {
                out[..7].copy_from_slice(&slot(&p.best[usize::from(class::KIN)], &frame, range, fresh));
                out[7] = kin_alarm(fauna, a, &p);
                out[8] = count(&p, class::KIN);
            }
            "Prey" => out.copy_from_slice(&slot(&p.best[usize::from(class::PREY)], &frame, range, fresh)),
            "Carcass" => out.copy_from_slice(&nearest_carcass(fauna.carcass_list(), &frame, v, manifest.carcass_range_m)),
            "Tremor" => out.copy_from_slice(&tremor(p.tremor)),
            "Scent(hunter)" => out.copy_from_slice(&scent(&p.scent[ScentChannel::Hunter.index()], &frame)),
            "Scent(grazer)" => out.copy_from_slice(&scent(&p.scent[ScentChannel::Grazer.index()], &frame)),
            "Scent(small-prey)" => out.copy_from_slice(&scent(&p.scent[ScentChannel::SmallPrey.index()], &frame)),
            "Scent(carrion)" => out.copy_from_slice(&scent(&p.scent[ScentChannel::Carrion.index()], &frame)),
            "Scent(nectar)" => out.copy_from_slice(&nectar(fauna, a, view, &frame)),
            "LightV3" => out.copy_from_slice(&light_v3(fauna, a, &p, &frame)),
            "Glow" => out.copy_from_slice(&glow(fauna, &p, &frame)),
            "Hunker" => out[0] = f64::from(a.hunkered),
            "Amphibious" => out.copy_from_slice(&[
                clamp01(a.mobility.dryness),
                f64::from(a.mobility.thirsty),
                f64::from(a.mobility.ashore),
                f64::from(a.mobility.retract_ticks > 0),
            ]),
            "Mode" => out.copy_from_slice(&mode(a, view)),
            "FaceLight" => {
                out[0] = a.pose.column(v, view.config.depth).map_or(0.0, |(cx, cz)| {
                    let wx = cx.rem_euclid(i64::from(view.config.width));
                    clamp01(view.sky_visibility(wx, a.sense_layer(v), cz) * clock.daylight())
                });
            }
            "Social" => out.copy_from_slice(&social(fauna, a, &frame, v, range)),
            "Calls" => crate::calls::encode(fauna, a, &frame, out),
            name => match name.strip_prefix("Place(").and_then(|r| r.strip_suffix(')')) {
                Some(kind) => out.copy_from_slice(&place(fauna, a, kind, &frame, v)),
                None => panic!("the {} manifest's v3 block {name} has no encoder", founder.name()),
            },
        }
    }
}

/// **The herd alarm** as the vigil hears it: a kin track heard in the latest sensings whose
/// loudness at its distance is at least the lineage's `alarm_emission_factor` times a
/// walking kin's. 0 for a lineage that hears no alarm.
fn kin_alarm(fauna: &Fauna, a: &Animal, p: &Percept) -> f64 {
    let Some(founder) = a.founder else { return 0.0 };
    let phys = fauna.config.founder(founder);
    let vg = phys.vigilance;
    if !(vg.alarm_emission_factor > 0.0) {
        return 0.0;
    }
    let pc = fauna.config.perception;
    let now = fauna.tick;
    let walk = |size: f64| {
        (phys.cruise_body_lengths_per_s * size).max(0.0).powf(pc.hearing_speed_exp) * size.max(0.0).powf(pc.hearing_size_exp)
    };
    let heard = p.tracks().iter().any(|t| {
        t.class == class::KIN
            && t.modes & HEARING != 0
            && t.tick + 2 * pc.stagger_ticks() >= now
            && t.loud * t.d_m.max(pc.hearing_near_m) >= vg.alarm_emission_factor * walk(t.size_m)
    });
    f64::from(heard)
}

/// The nearest carcass face within `range_m` (`valid, sin, cos, proximity`).
pub fn nearest_carcass(sites: &[Site], frame: &Frame, v: f64, range_m: f64) -> [f64; 4] {
    let best = sites
        .iter()
        .map(|&s| {
            let (x, z) = site_xz(s, v);
            frame.toward(x, z)
        })
        .filter(|&(_, _, d)| d <= range_m)
        .min_by(|p, q| p.2.total_cmp(&q.2));
    match best {
        Some((s, c, d)) => [1.0, s, c, proximity(d, range_m)],
        None => [0.0; 4],
    }
}

/// The bellwing's nectar plume at its eye, as its light steering smells it.
fn nectar(fauna: &Fauna, a: &Animal, view: &VoxelView<'_>, frame: &Frame) -> [f64; 3] {
    let Some(founder) = a.founder else { return [0.0; 3] };
    let floor = fauna.config.founder(founder).senses.smell_min;
    let Some(eye) = crate::light::eye_of(&fauna.config, a, view.config.voxel_m) else { return [0.0; 3] };
    let plume = crate::scent::nectar_plume_indexed(fauna, view, eye, a.id, floor);
    scent(&ScentReading { level: plume.level, gx: plume.gx, gz: plume.gz }, frame)
}

/// The photoreceptor (`ambient, loom, loom_sin, loom_cos, beacon, beacon_level, beacon_sin,
/// beacon_cos, beacon_cyan, moving_level, moving_sin, moving_cos`).
fn light_v3(fauna: &Fauna, a: &Animal, p: &Percept, frame: &Frame) -> [f64; 12] {
    let r = &p.light;
    let mut out = [0.0; 12];
    out[0] = clamp01(r.ambient);
    if r.looming > 0.0 {
        out[1] = clamp01(r.looming);
        out[2] = r.loom_bearing.sin();
        out[3] = r.loom_bearing.cos();
    }
    out[4..9].copy_from_slice(&beacon(fauna, a, p, frame));
    if r.repel_level > 0.0 {
        out[9] = clamp01(r.repel_level);
        out[10] = r.repel_bearing.sin();
        out[11] = r.repel_bearing.cos();
    }
    out
}

/// The brightest still beacon (`valid, level, sin, cos, cyan`).
fn beacon(fauna: &Fauna, a: &Animal, p: &Percept, frame: &Frame) -> [f64; 5] {
    let r = &p.light;
    if !r.beacon || !(r.beacon_level > 0.0) {
        return [0.0; 5];
    }
    let nir = a.founder.is_some_and(|f| fauna.config.founder(f).senses.nir);
    let kind = match r.beacon_kind {
        0 => crate::BeaconKind::Flower,
        1 => crate::BeaconKind::Glowcap,
        _ => crate::BeaconKind::Lure,
    };
    let (s, c, _) = frame.toward(r.beacon_x, r.beacon_z);
    let cyan = kind.class_seen(&fauna.config.light, nir) == crate::light::CYAN;
    [1.0, clamp01(r.beacon_level), s, c, f64::from(cyan)]
}

/// The capgnawer's glow (`beacon, beacon_level, beacon_sin, beacon_cos, marked_here`).
fn glow(fauna: &Fauna, p: &Percept, frame: &Frame) -> [f64; 5] {
    let r = &p.light;
    let mut out = [0.0; 5];
    if r.beacon && r.beacon_level > 0.0 {
        let (s, c, _) = frame.toward(r.beacon_x, r.beacon_z);
        out[..4].copy_from_slice(&[1.0, clamp01(r.beacon_level), s, c]);
    }
    out[4] = f64::from(p.marked(frame.x, frame.z, frame.tick, fauna.config.light.mark_radius_m, frame.ring_m));
    out
}

/// The glider's mode and its base over the support below it.
fn mode(a: &Animal, view: &VoxelView<'_>) -> [f64; 5] {
    let m = a.mobility.mode;
    let v = view.config.voxel_m;
    let height = if m == MobilityMode::Ground {
        0.0
    } else {
        let base = crate::perception::base_m(a, v);
        crate::flight::support_below(view, a.pose.x, a.pose.z, base).map_or(0.0, |s| (base - crate::surface_m(s.y, v)).max(0.0))
    };
    [
        f64::from(matches!(m, MobilityMode::Ground | MobilityMode::Wall)),
        f64::from(m == MobilityMode::Climb),
        f64::from(m == MobilityMode::Perched),
        f64::from(matches!(m, MobilityMode::Glide | MobilityMode::Fly)),
        clamp01(height / HEIGHT_REFERENCE_M),
    ]
}

/// The Veilhound's pack (`breeder, pup, target ×4, feast ×4`); all zero outside a pack.
fn social(fauna: &Fauna, a: &Animal, frame: &Frame, v: f64, range_m: f64) -> [f64; 10] {
    let mut out = [0.0; 10];
    let Some(member) = a.pack else { return out };
    let Some(pack) = fauna.packs.iter().find(|p| p.id == member.pack) else { return out };
    out[0] = f64::from(pack.breeder == Some(a.id));
    out[1] = f64::from(member.pup);
    if let Some(t) = pack.target.and_then(|id| fauna.animals.iter().find(|o| o.id == id)) {
        let (s, c, d) = frame.toward(t.pose.x, t.pose.z);
        out[2..6].copy_from_slice(&[1.0, s, c, proximity(d, range_m)]);
    }
    if let Some(f) = pack.feast {
        let (x, z) = site_xz(f, v);
        let (s, c, d) = frame.toward(x, z);
        out[6..10].copy_from_slice(&[1.0, s, c, proximity(d, range_m)]);
    }
    out
}

/// **One place** (P6): `kind` is the name inside `Place(…)`. The current pack den
/// is always known (item 24); other kinds read the cached reflex query within the
/// body's sight, else the fading memory of a home place it physically used.
pub(crate) fn place(fauna: &Fauna, a: &Animal, kind: &str, frame: &Frame, v: f64) -> [f64; 5] {
    crate::places::encode(fauna, a, kind, frame, v)
}
