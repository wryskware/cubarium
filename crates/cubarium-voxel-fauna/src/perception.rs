//! **The perception core** (SENSES-CORE, S1 of the senses pass;
//! `design/7_Research/senses-review-2026-10-06.md` §3 and §5, with Astra's design in
//! `design/7_Research/astra-senses-2026-10-06.md`): one record per body of what it senses,
//! which its heuristics read and nothing else.
//!
//! # The record ([`Percept`])
//!
//! Fixed size, per body, kept in [`crate::Fauna`] by id:
//! - **tracks**: the bodies sensed at its last remote sensing, nearest first (at most
//!   [`MAX_TRACKS`]), each a [`Track`] — its class to the observer (prey, threat, kin), its
//!   position then (the **last-known** point), the 3D distance, bearing and elevation, the
//!   **modality mask** of every sense that detected it ([`SIGHT`], [`HEARING`], [`TREMOR`]),
//!   the loudness and footfall received, its size, and the tick it was sensed;
//! - **best**: per class ([`class`]), the strongest (nearest) item of the latest sensing that
//!   had one, kept with its tick when a later sensing has none — memory, aged by the tick;
//! - **focus**: the one body a behaviour pursues ([`Percept::focus`]), its last-known point
//!   updated only on a sensing that has it; a pursuit walks there and gives up after
//!   [`crate::PerceptionConfig::give_up_s`];
//! - **scent**: per channel, the level and gradient where it stands ([`crate::scent`]);
//! - **tremor**: the footfall felt on its left and right;
//! - **carcasses**: the usable carcasses within the hunt's carcass scan (a hunter's);
//! - **light** (S3, [`crate::light`]): the ambient light, the looming shadow, the brightest
//!   still beacon and the brightest moving one ([`LightReading`]);
//! - **marks** (S3): the failed-visit memory, places marked "no reward here" until they
//!   decay, and the beacon being inspected ([`Visit`]).
//!
//! # Who fills it
//!
//! A lineage whose [`crate::SenseTable`] is **live** (the Veilhound in S1) is filled by the
//! core:
//! - **Remote senses** at [`crate::PerceptionConfig::remote_hz`], staggered by id
//!   ([`sense_pass`]): bodies are binned each tick; an observer weighs at most
//!   `max_candidates` prey and threats, nearest first (and the nearest of its own lineage),
//!   each with **one terrain ray** from its eye to the body's middle that all three senses
//!   share — clear, sight may see and hearing and tremor arrive whole; blocked, no sight and
//!   hearing and tremor times the occlusion knob. Range is the **3D** distance. Hearing's
//!   emission is `speed^a × length^b` over the distance. Tremor reaches only from bodies on
//!   the ground.
//! - **Tick-rate parts** at each of its controller samples ([`refresh`]): the tremor
//!   reading, the scent reading, the carcasses.
//!
//! S2 turns on the frondgrazer's, loftstrider's, seedporter's and littershredder's tables, with
//! two inputs from their responses ([`crate::vigil`]): a grazer with its head down between
//! scans sees [`crate::SenseTable::feeding_sight_fraction`] of its range, and a running body
//! is louder by its [`crate::Vigilance::run_noise`]. An observer weighs up to
//! [`crate::SenseTable::kin_tracks`] of its own lineage.
//!
//! Every other lineage's record is filled by the **interim rules** its reflexes ran on before
//! the senses pass ([`refresh`]), so it behaves as before until S2 and S3 turn its table on:
//! - threats: the flee rule's — the nearest body that hunts it ([`crate::kind_of`]: moving,
//!   or within 0.5 m) within `flee_sight_m`, ahead, on a ray of the predation occupancy;
//! - prey: the lanternjaw patrol's — every body it takes ([`crate::is_prey_of`]) within its
//!   `hunt.sight_m` ahead on that ray, or whose tremor reaches it within `tremor_radius_m`;
//! - tremor: every moving body within `tremor_radius_m` ([`crate::FaunaView::tremor`]).

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::FloraView;
use serde::{Deserialize, Serialize};

use crate::scent::{ScentChannel, ScentReading};
use crate::{Animal, Fauna, FaunaConfig, Founder, SenseTable};

/// The modality bits of a [`Track::modes`].
pub const SIGHT: u8 = 1;
pub const HEARING: u8 = 2;
pub const TREMOR: u8 = 4;
/// Smell is never a body's: it marks a draw up a scent ([`crate::FaunaLedger::pack_draws_by_sense`]).
pub const SMELL: u8 = 8;
/// The ledger's modality order: `[sight, hearing, tremor, smell]`.
pub const MODALITIES: [u8; 4] = [SIGHT, HEARING, TREMOR, SMELL];

/// The tracks one record holds at most (a capacity, not ecology: the core weighs at most
/// [`crate::PerceptionConfig::max_candidates`] plus one).
pub const MAX_TRACKS: usize = 32;
/// The carcasses one record holds at most.
pub const MAX_CARCASSES: usize = 8;
/// The funnel events one record remembers ([`Percept::first`]; a capacity).
pub const FUNNEL_MEMORY: usize = 32;
/// The failed-visit marks one body holds at once (a cache size, not ecology).
pub const MAX_MARKS: usize = 8;

/// What a sensed body is to the observer ([`Track::class`]).
pub mod class {
    /// On its hunter's list.
    pub const PREY: u8 = 0;
    /// Its own lineage on the other's list.
    pub const THREAT: u8 = 1;
    /// Its own lineage.
    pub const KIN: u8 = 2;
    /// Food, beacons and unknowns: S2 and S3's.
    pub const FOOD: u8 = 3;
    pub const BEACON: u8 = 4;
    pub const UNKNOWN: u8 = 5;
    pub const COUNT: usize = 6;
}

/// The hunt funnel's stages ([`crate::FaunaLedger::funnel`]).
pub const SENSED: usize = 0;
pub const APPROACHED: usize = 1;
pub const STRUCK: usize = 2;
pub const KILLED: usize = 3;
pub const EATEN: usize = 4;
pub const FUNNEL_STAGES: [&str; 5] = ["sensed", "approached", "struck", "killed", "eaten"];

/// One sensed body (module note).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// Holds a body.
    pub known: bool,
    pub id: u64,
    /// Its lineage's [`Founder::index`].
    pub lineage: u8,
    /// [`class`].
    pub class: u8,
    /// [`SIGHT`] | [`HEARING`] | [`TREMOR`]: every sense that detected it.
    pub modes: u8,
    /// Where it was: its centre on the ring and its base height, metres, and its face.
    pub x: f64,
    pub z: f64,
    pub y_m: f64,
    pub site: (u32, u32, u32),
    /// Its base over the support under it, metres (0 on the ground).
    pub lift_m: f64,
    /// On the ground (walking, not climbing), and climbing.
    pub grounded: bool,
    pub climbing: bool,
    /// Moved within the still window ([`crate::moved_recently`]).
    pub moving: bool,
    /// From the observer's eye to its middle, metres (3D), and the bearing from the
    /// observer's heading (radians, positive to the right) and elevation.
    pub d_m: f64,
    pub bearing: f64,
    pub elevation: f64,
    /// The loudness heard and the footfall felt from it.
    pub loud: f64,
    pub felt: f64,
    /// Its length, metres.
    pub size_m: f64,
    /// The tick it was sensed.
    pub tick: u64,
    /// Its heading then, radians (S2: a herd bolts the way a neighbour runs).
    pub heading: f64,
    /// **FEAR**: its posture as seen ([`posture`]); [`posture::ACTIVE`] unless sight detected
    /// it (a posture is read by eye: a still body is neither heard nor felt).
    pub posture: u8,
}

/// **A body's posture** as a sighted observer reads it (FEAR, [`posture_of`]).
pub mod posture {
    /// Up and about, or not seen: whatever it is doing, it may come.
    pub const ACTIVE: u8 = 0;
    /// Awake and still: no strike, not moving within the still window, not feeding (lying
    /// or standing, whatever its state: FEAR-2).
    pub const REST: u8 = 1;
    /// Asleep.
    pub const ASLEEP: u8 = 2;
}

/// **FEAR**: `o`'s posture ([`posture`]): asleep; still (awake, no strike under way, not
/// feeding, still for the predation table's `still_after_s`); else active. **FEAR-2**: still in
/// any awake state, not only [`crate::State::Resting`]: a Veilhound standing idle (its state
/// `Walking`, not due a step) reads the same to an eye as one lying awake, and FEAR's
/// `Resting` test left those idle hunters active. A body feeding ([`crate::State::Cropping`],
/// a hunter at a carcass) is active: FEAR's probe found prey fleeing a feeding Veilhound in at
/// most 1 % of the seedporter's flight ticks (0–8 % of the loftstrider's), too little to rule on.
pub fn posture_of(config: &FaunaConfig, o: &Animal) -> u8 {
    if o.state == crate::State::Asleep {
        posture::ASLEEP
    } else if o.state != crate::State::Cropping && o.strike.is_none() && !crate::moved_recently(&config.predation, o) {
        posture::REST
    } else {
        posture::ACTIVE
    }
}

impl Track {
    /// Its last-known point as a [`cubarium_voxel_flora::Site`].
    pub fn face(&self) -> cubarium_voxel_flora::Site {
        cubarium_voxel_flora::Site { x: self.site.0, y: self.site.1, z: self.site.2 }
    }

    /// Ticks since it was sensed.
    pub fn age(&self, now: u64) -> u64 {
        now.saturating_sub(self.tick)
    }
}

/// **The light reading** (S3, [`crate::light`]): what its photoreceptor reads — one
/// receptor, so a direction and a level, never a shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LightReading {
    /// The light where its eye is: `sky × daylight`.
    pub ambient: f64,
    /// A shadow falling on it: `1 − d / loom_m` of the nearest moving body within its
    /// `loom_m` (0 none), and its bearing from the heading (radians, positive right).
    pub looming: f64,
    pub loom_bearing: f64,
    /// The brightest **still** beacon it sees (a clear line, not under a failed-visit mark):
    /// whether there is one, its kind ([`crate::BeaconKind`] as `u8`), its bearing and level,
    /// and where it is (ring `x`, `z`, height `y`, metres).
    pub beacon: bool,
    pub beacon_kind: u8,
    pub beacon_bearing: f64,
    pub beacon_level: f64,
    pub beacon_x: f64,
    pub beacon_z: f64,
    pub beacon_y: f64,
    /// The brightest **moving** light it sees (a lure on a body that moved): level (0 none)
    /// and bearing. A moving light repels.
    pub repel_level: f64,
    pub repel_bearing: f64,
    /// The tick the beacons were last read (they are read at the remote senses' rate).
    pub tick: u64,
}

/// **A failed-visit mark** (S3): "no reward here" around `(x, z)` (ring metres) until the
/// tick; 0 is an empty slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Mark {
    pub x: f64,
    pub z: f64,
    pub until_tick: u64,
}

/// **The beacon being inspected** (S3): where, its kind, and the tick the inspection began.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Visit {
    pub active: bool,
    pub kind: u8,
    pub x: f64,
    pub z: f64,
    pub since: u64,
}

/// **A body's percept record** (module note).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Percept {
    /// The tick of its last remote sensing (or interim fill).
    pub tick: u64,
    pub n: u8,
    pub tracks: [Track; MAX_TRACKS],
    /// Per [`class`], the nearest item of the latest sensing that had one.
    pub best: [Track; class::COUNT],
    /// The body a behaviour pursues, at its last-known point.
    pub focus: Track,
    pub scent: [ScentReading; ScentChannel::COUNT],
    pub tremor: [f64; 2],
    pub n_carcasses: u8,
    pub carcasses: [(u32, u32, u32); MAX_CARCASSES],
    pub light: LightReading,
    /// **The failed-visit memory** (S3): places marked "no reward here", each until it
    /// decays ([`crate::LightConfig::mark_s`]).
    pub marks: [Mark; MAX_MARKS],
    /// The beacon it is inspecting.
    pub visit: Visit,
    /// **The funnel's memory**: `(stage, key, tick)` of the events it counted lately — a
    /// prey sensed or approached (by id), a carcass eaten (by face). An event counts once
    /// per episode: again only after [`crate::PerceptionConfig::give_up_s`] without it.
    pub recent: [(u8, u64, u64); FUNNEL_MEMORY],
    /// **Its ground response's state** (S2, [`crate::vigil`]): scans, flight, alarm,
    /// orientation, freeze.
    pub vigil: crate::Vigil,
    /// **Which light pays** (S3b, [`crate::light::expectation`]): per beacon class (near-IR,
    /// cyan) the learned offset from the innate reward expectation as of `expect_tick`; it
    /// relaxes back over `forget_s`. All zero: naive.
    pub expect: [f64; crate::light::CLASSES],
    pub expect_tick: u64,
    /// The tick of its last sip (0: none yet): a sip bout teaches once.
    pub last_sip: u64,
}

impl Percept {
    /// Its current tracks.
    pub fn tracks(&self) -> &[Track] {
        &self.tracks[..usize::from(self.n).min(MAX_TRACKS)]
    }

    /// The track of body `id`, if sensed at its last sensing.
    pub fn track(&self, id: u64) -> Option<&Track> {
        self.tracks().iter().find(|t| t.id == id)
    }

    /// The usable carcasses it knows.
    pub fn carcass_sites(&self) -> impl Iterator<Item = cubarium_voxel_flora::Site> + '_ {
        self.carcasses[..usize::from(self.n_carcasses).min(MAX_CARCASSES)]
            .iter()
            .map(|&(x, y, z)| cubarium_voxel_flora::Site { x, y, z })
    }

    /// Whether the funnel event `(stage, key)` begins a new episode at `now` — not seen
    /// within `window` ticks; it is remembered (or its tick refreshed) either way.
    pub fn first(&mut self, stage: usize, key: u64, now: u64, window: u64) -> bool {
        let stage = stage as u8;
        if let Some(e) = self.recent.iter_mut().find(|e| e.0 == stage && e.1 == key && e.2 != 0) {
            let new = now.saturating_sub(e.2) > window;
            e.2 = now.max(1);
            return new;
        }
        let k = (0..FUNNEL_MEMORY).min_by_key(|&k| self.recent[k].2).unwrap_or(0);
        self.recent[k] = (stage, key, now.max(1));
        true
    }

    /// Whether `(x, z)` is under a mark that has not decayed at `now` (within `radius` of
    /// its centre on a ring `ring` metres round).
    pub fn marked(&self, x: f64, z: f64, now: u64, radius: f64, ring: f64) -> bool {
        self.marks.iter().any(|m| {
            if m.until_tick <= now {
                return false;
            }
            let (dx, dz) = crate::pose::wrapped_delta(m.x, m.z, x, z, ring);
            dx * dx + dz * dz <= radius * radius
        })
    }

    /// Mark `(x, z)` "no reward here" until `until_tick`, over the slot that decays first.
    pub fn mark(&mut self, x: f64, z: f64, until_tick: u64) {
        let k = (0..MAX_MARKS).min_by_key(|&k| self.marks[k].until_tick).unwrap_or(0);
        self.marks[k] = Mark { x, z, until_tick };
    }

    fn set_tracks(&mut self, tracks: &[Track]) {
        let n = tracks.len().min(MAX_TRACKS);
        self.tracks = [Track::default(); MAX_TRACKS];
        self.tracks[..n].copy_from_slice(&tracks[..n]);
        self.n = n as u8;
    }

    /// Point the focus at body `id` (a pursuit begins): kept as it is when it already
    /// holds that body; else taken from its tracks, or unknown (aged from `now`).
    pub fn set_focus(&mut self, id: u64, now: u64) {
        if self.focus.known && self.focus.id == id {
            return;
        }
        self.focus = match self.track(id) {
            Some(t) => *t,
            None => Track { known: false, id, tick: now, ..Track::default() },
        };
    }
}

/// What one body is to another, for the record: prey on the observer's list, threat when
/// the observer is on the other's, kin of its own lineage; `None` for anything else (S1
/// weighs no other body).
pub(crate) fn class_of(config: &FaunaConfig, observer: &Animal, o: &Animal) -> Option<u8> {
    let (bf, of) = (observer.founder?, o.founder?);
    if bf == of {
        return Some(class::KIN);
    }
    if config.founder(bf).active_hunter().is_some_and(|h| h.hunts(of)) {
        return Some(class::PREY);
    }
    if config.founder(of).active_hunter().is_some_and(|h| h.hunts(bf)) {
        return Some(class::THREAT);
    }
    None
}

/// The occupancy the core's ray is marched under: terrain (and the world's edges and free
/// water) only — no body or stand blocks it.
pub(crate) struct TerrainOnly;

impl crate::senses::CellOccupancy for TerrainOnly {
    fn any(&self, _cell: usize) -> bool {
        false
    }
    fn other_body(&self, _cell: usize, _observer_id: u64) -> bool {
        false
    }
    fn environment(&self, _cell: usize) -> Option<crate::senses::Fine> {
        None
    }
    fn pool_top(&self, _cell: usize) -> Option<f64> {
        None
    }
}

/// A body's base height, metres: its pose's off the ground, its sensing layer's top on it.
pub(crate) fn base_m(o: &Animal, v: f64) -> f64 {
    if o.mobility.mode != crate::MobilityMode::Ground && o.pose.y_m.is_finite() {
        o.pose.y_m
    } else {
        crate::surface_m(o.sense_layer(v), v)
    }
}

/// Hearing's emission ([`crate::PerceptionConfig`]): `speed^a × length^b`.
pub(crate) fn emission(config: &FaunaConfig, o: &Animal) -> f64 {
    let pc = &config.perception;
    let speed = if o.speed_m_s.is_finite() { o.speed_m_s.max(0.0) } else { 0.0 };
    if !(speed > 0.0) {
        return 0.0;
    }
    let length = crate::body_of(config, o).map_or(0.0, |g| g.length_m);
    speed.powf(pc.hearing_speed_exp) * length.max(0.0).powf(pc.hearing_size_exp)
}

/// One pair's cost readout.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Cost {
    pub pairs: u64,
    pub cells: u64,
}

/// **The pairwise senses for one pair** (module note): which of `b`'s senses (`table`, at
/// this light, its sight ranges times `sight_scale` — S2's head-down blind window) detect
/// `o` (its emission times `noise` — S2's loud run), sharing one terrain ray; `None` when
/// none does (no ray is marched when no sense could reach it).
#[allow(clippy::too_many_arguments)]
pub(crate) fn sense_pair(
    config: &FaunaConfig,
    view: &VoxelView<'_>,
    table: &SenseTable,
    day: bool,
    b: &Animal,
    o: &Animal,
    class: u8,
    tick: u64,
    cost: &mut Cost,
    sight_scale: f64,
    noise: f64,
) -> Option<Track> {
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let (geometry, og) = (crate::body_of(config, b)?, crate::body_of(config, o)?);
    let origin = crate::senses::cone_origin(view, &b.pose, b.sense_layer(v), &geometry);
    let base = base_m(o, v);
    let mid = base + 0.5 * og.height_m;
    let (dx, dz) = crate::pose::wrapped_delta(b.pose.x, b.pose.z, o.pose.x, o.pose.z, ring);
    let ey = mid - origin.1;
    let dh = (dx * dx + dz * dz).sqrt();
    let d3 = (dh * dh + ey * ey).sqrt();
    let pc = &config.perception;
    let moving = crate::moved_recently(&config.predation, o);
    // What could reach it before the ray.
    let (fx, fz) = b.pose.forward();
    let cos = if dh > 0.0 { (dx * fx + dz * fz) / dh } else { 1.0 };
    let in_field = dh <= 0.0 || cos >= table.sight_half_angle_deg.to_radians().cos() - 1e-12;
    let range = table.sight_range_m(day) * if moving { 1.0 } else { table.sight_still_fraction.max(0.0) } * sight_scale.max(0.0);
    let may_see = range > 0.0 && d3 <= range && in_field;
    let e = emission(config, o) * noise.max(0.0);
    let may_hear = table.hearing_m > 0.0 && d3 <= table.hearing_m && e > 0.0;
    let grounded = o.mobility.mode == crate::MobilityMode::Ground && o.attached.is_none();
    let may_feel = table.tremor_m > 0.0 && d3 <= table.tremor_m && grounded && o.speed_m_s > 0.0;
    if !(may_see || may_hear || may_feel) {
        return None;
    }
    // The one ray.
    cost.pairs += 1;
    let blocked = if d3 > 0.0 {
        let dir = (dx / d3, ey / d3, dz / d3);
        let hit = crate::senses::ray_first_hit(view, &TerrainOnly, b.id, origin, dir, d3).map(|(h, _)| h);
        let walked = hit.unwrap_or(d3).min(d3);
        cost.cells += ((dx.abs() + ey.abs() + dz.abs()) * walked / d3 / v).ceil() as u64 + 1;
        hit.is_some_and(|h| h < d3 - v)
    } else {
        false
    };
    let through = if blocked { pc.occlusion.max(0.0) } else { 1.0 };
    let mut modes = 0u8;
    if may_see && !blocked {
        modes |= SIGHT;
    }
    let loud = if may_hear { e / d3.max(pc.hearing_near_m.max(1e-9)) * through } else { 0.0 };
    if may_hear && loud > 0.0 && loud >= table.hearing_min {
        modes |= HEARING;
    }
    let felt = if may_feel { crate::tremor_of(o, d3, config.predation.tremor_near_m) * through } else { 0.0 };
    if may_feel && felt > 0.0 && felt >= table.tremor_min {
        modes |= TREMOR;
    }
    if modes == 0 {
        return None;
    }
    let lift_m = if grounded {
        0.0
    } else {
        crate::flight::support_below(view, o.pose.x, o.pose.z, base)
            .map_or(0.0, |s| (base - crate::surface_m(s.y, v)).max(0.0))
    };
    let heading = b.pose.heading_rad;
    let bearing = {
        let d = (dx.atan2(dz) - heading).rem_euclid(std::f64::consts::TAU);
        if d > std::f64::consts::PI { d - std::f64::consts::TAU } else { d }
    };
    Some(Track {
        known: true,
        id: o.id,
        lineage: o.founder.map_or(u8::MAX, |f| f.index() as u8),
        class,
        modes,
        x: o.pose.x,
        z: o.pose.z,
        y_m: base,
        site: (o.site.x, o.site.y, o.site.z),
        lift_m,
        grounded: grounded && o.mobility.climb.is_none(),
        climbing: o.mobility.climb.is_some(),
        moving,
        d_m: d3,
        bearing,
        elevation: if dh > 0.0 || ey != 0.0 { ey.atan2(dh) } else { 0.0 },
        loud,
        felt,
        size_m: og.length_m,
        tick,
        heading: o.pose.heading_rad,
        posture: if modes & SIGHT != 0 { posture_of(config, o) } else { posture::ACTIVE },
    })
}

/// Whether the world's light is day for sight: the clock off, or daylight at the day line
/// or above.
pub(crate) fn is_day(config: &FaunaConfig, clock_on: bool, daylight: f64) -> bool {
    !clock_on || daylight >= config.life.day_line
}

/// Bodies binned by `(x, z)` on the ring ([`crate::PerceptionConfig::bin_m`]).
struct Bins {
    nx: usize,
    nz: usize,
    bin: f64,
    cells: Vec<Vec<u32>>,
}

impl Bins {
    fn build(fauna: &Fauna, view: &VoxelView<'_>) -> Bins {
        let v = view.config.voxel_m;
        let ring = f64::from(view.config.width) * v;
        let depth = f64::from(view.config.depth) * v;
        let bin = if fauna.config.perception.bin_m.is_finite() && fauna.config.perception.bin_m > 0.0 {
            fauna.config.perception.bin_m.max(v)
        } else {
            ring.max(depth)
        };
        let nx = ((ring / bin).ceil() as usize).max(1);
        let nz = ((depth / bin).ceil() as usize).max(1);
        let mut cells = vec![Vec::new(); nx * nz];
        for (k, a) in fauna.animals.iter().enumerate() {
            if a.attached.is_some() {
                continue;
            }
            let (ix, iz) = Bins::cell(a.pose.x, a.pose.z, bin, nx, nz);
            cells[iz * nx + ix].push(k as u32);
        }
        Bins { nx, nz, bin, cells }
    }

    fn cell(x: f64, z: f64, bin: f64, nx: usize, nz: usize) -> (usize, usize) {
        let ix = ((x / bin).floor() as i64).rem_euclid(nx as i64) as usize;
        let iz = ((z / bin).floor().max(0.0) as usize).min(nz - 1);
        (ix, iz)
    }

    /// Every body index in the bins within `reach` of `(x, z)`, each once.
    fn around(&self, x: f64, z: f64, reach: f64, out: &mut Vec<u32>) {
        out.clear();
        let r = (reach / self.bin).ceil() as i64 + 1;
        let (cx, cz) = Bins::cell(x, z, self.bin, self.nx, self.nz);
        let span_x = (2 * r + 1).min(self.nx as i64);
        let x0 = cx as i64 - if span_x == self.nx as i64 { cx as i64 } else { r };
        for ox in 0..span_x {
            let ix = (x0 + ox).rem_euclid(self.nx as i64) as usize;
            let z0 = (cz as i64 - r).max(0) as usize;
            let z1 = ((cz as i64 + r) as usize).min(self.nz - 1);
            for iz in z0..=z1 {
                out.extend_from_slice(&self.cells[iz * self.nx + ix]);
            }
        }
    }
}

/// **One remote sensing** of the body at `i` (live), from scratch: its candidates, one ray
/// each, its tracks nearest first.
fn sense_one(fauna: &Fauna, view: &VoxelView<'_>, bins: &Bins, day: bool, i: usize, cost: &mut Cost, scratch: &mut Vec<u32>) -> Vec<Track> {
    let b = &fauna.animals[i];
    let Some(f) = b.founder else { return Vec::new() };
    // RHYTHMS-CADENCE: a body that has stood still senses farther (`SenseTable::still_gain`).
    let table = fauna.config.founder(f).senses.sensing(b.still_ticks);
    let reach = table.reach_m(day);
    if !(reach > 0.0) {
        return Vec::new();
    }
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    bins.around(b.pose.x, b.pose.z, reach, scratch);
    let mut cand: Vec<(f64, u64, usize, u8)> = Vec::new();
    for &k in scratch.iter() {
        let o = &fauna.animals[k as usize];
        if o.id == b.id {
            continue;
        }
        let Some(c) = class_of(&fauna.config, b, o) else { continue };
        let (dx, dz) = crate::pose::wrapped_delta(b.pose.x, b.pose.z, o.pose.x, o.pose.z, ring);
        let dh = (dx * dx + dz * dz).sqrt();
        if dh > reach {
            continue;
        }
        cand.push((dh, o.id, k as usize, c));
    }
    cand.sort_by(|p, q| p.0.total_cmp(&q.0).then(p.1.cmp(&q.1)));
    let cap = fauna.config.perception.max_candidates as usize;
    let kin_cap = table.kin_tracks as usize;
    let mut weighed = 0usize;
    let mut kin = 0usize;
    let mut out: Vec<Track> = Vec::new();
    // S2: head down between scans, it sees a fraction of its range.
    let now = fauna.tick;
    let sight_scale = if fauna.percepts.get(&b.id).is_some_and(|p| p.vigil.head_down(now)) { table.feeding_sight_fraction } else { 1.0 };
    for &(_, _, k, c) in &cand {
        if c == class::KIN {
            if kin >= kin_cap {
                continue;
            }
            kin += 1;
        } else {
            if weighed >= cap {
                continue;
            }
            weighed += 1;
        }
        let o = &fauna.animals[k];
        let noise = run_noise(fauna, o, now);
        if let Some(t) = sense_pair(&fauna.config, view, &table, day, b, o, c, now, cost, sight_scale, noise) {
            out.push(t);
        }
        if weighed >= cap && kin >= kin_cap {
            break;
        }
    }
    out.sort_by(|p, q| p.d_m.total_cmp(&q.d_m).then(p.id.cmp(&q.id)));
    out
}

/// A body's emission factor (S2): its lineage's [`crate::Vigilance::run_noise`] while it runs
/// (flees or bolts), else 1.
pub(crate) fn run_noise(fauna: &Fauna, o: &Animal, now: u64) -> f64 {
    let running = fauna.percepts.get(&o.id).is_some_and(|p| p.vigil.running(now));
    match o.founder {
        Some(f) if running => fauna.config.founder(f).vigilance.run_noise,
        _ => 1.0,
    }
}

/// Whether terrain leaves the straight line from `from` to `to` (metres) open (S2's still
/// fruit seen across the ground): the core's one-ray rule.
pub(crate) fn line_clear(view: &VoxelView<'_>, id: u64, from: (f64, f64, f64), to: (f64, f64, f64)) -> bool {
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let (dx, dz) = crate::pose::wrapped_delta(from.0, from.2, to.0, to.2, ring);
    let dy = to.1 - from.1;
    let d = (dx * dx + dy * dy + dz * dz).sqrt();
    if !(d > 0.0) {
        return true;
    }
    let dir = (dx / d, dy / d, dz / d);
    crate::senses::ray_first_hit(view, &TerrainOnly, id, from, dir, d).is_none_or(|(h, _)| h >= d - view.config.voxel_m)
}

/// Fold a sensing's tracks into a record: the tracks, the best per class (kept when this
/// sensing has none of a class), the focus's last-known point; and the funnel's newly
/// sensed prey (a prey not in the record's previous tracks).
fn fold(fauna: &mut Fauna, id: u64, founder: Founder, tracks: &[Track], classes: Option<&[u8]>) {
    let tick = fauna.tick;
    let p = fauna.percepts.entry(id).or_default();
    // A class not refreshed keeps its tracks.
    let mut all: Vec<Track> = match classes {
        Some(cs) => p.tracks().iter().filter(|t| !cs.contains(&t.class)).copied().collect(),
        None => Vec::new(),
    };
    all.extend_from_slice(tracks);
    p.set_tracks(&all);
    p.tick = tick;
    for c in 0..class::COUNT {
        if let Some(t) = tracks.iter().filter(|t| usize::from(t.class) == c).min_by(|a, b| a.d_m.total_cmp(&b.d_m)) {
            p.best[c] = *t;
        }
    }
    if let Some(t) = tracks.iter().find(|t| t.id == p.focus.id) {
        p.focus = *t;
    }
    let window = funnel_window(&fauna.config);
    let mut new: Vec<usize> = Vec::new();
    for t in tracks.iter().filter(|t| t.class == class::PREY) {
        if p.first(SENSED, t.id, tick, window) && usize::from(t.lineage) < Founder::COUNT {
            new.push(usize::from(t.lineage));
        }
    }
    for prey in new {
        fauna.ledger.funnel[founder.index()][prey][SENSED] += 1;
    }
}

/// **The core's remote pass** for the tick (module note): every awake, free body of a live
/// lineage whose id falls on this tick of the stagger senses; the records are written in
/// body order. Records of the dead are dropped.
pub(crate) fn sense_pass(fauna: &mut Fauna, view: &VoxelView<'_>, clock_on: bool, daylight: f64, threads: usize) {
    #[cfg(feature = "profile")]
    let _timer = cubarium_voxel::profile::start(cubarium_voxel::profile::Phase::FaunaRemote);
    if fauna.tick % 64 == 0 {
        let alive: rustc_hash::FxHashSet<u64> = fauna.animals.iter().map(|a| a.id).collect();
        fauna.percepts.retain(|id, _| alive.contains(id));
        fauna.steer.retain(|id, _| alive.contains(id));
        fauna.walk_targets.retain(|id, _| alive.contains(id));
        fauna.pressed.retain(|id, _| alive.contains(id));
        fauna.walk_shun.retain(|id, _| alive.contains(id));
        fauna.route_flee.retain(|id, _| alive.contains(id));
        fauna.forage.retain(|id, _| alive.contains(id));
        fauna.nest_trips.retain(|id, _| alive.contains(id));
    }
    let stagger = fauna.config.perception.stagger_ticks();
    let tick = fauna.tick;
    let sensing: Vec<usize> = fauna
        .animals
        .iter()
        .enumerate()
        .filter(|(_, a)| {
            a.founder.is_some_and(|f| fauna.config.founder(f).senses.live)
                && a.state != crate::State::Asleep
                && a.attached.is_none()
                && a.id % stagger == tick % stagger
        })
        .map(|(i, _)| i)
        .collect();
    if sensing.is_empty() {
        return;
    }
    let day = is_day(&fauna.config, clock_on, daylight);
    let bins = Bins::build(fauna, view);
    let results: Vec<(usize, Vec<Track>, Cost)> = {
        let fauna: &Fauna = fauna;
        let bins = &bins;
        let one = move |i: usize| {
            let mut cost = Cost::default();
            let mut scratch = Vec::new();
            let t = sense_one(fauna, view, bins, day, i, &mut cost, &mut scratch);
            (i, t, cost)
        };
        let ranges = crate::par::ranges(sensing.len(), threads.max(1) * 2, 4);
        if ranges.len() <= 1 {
            sensing.iter().map(|&i| one(i)).collect()
        } else {
            let sensing = &sensing;
            let jobs: Vec<crate::par::Job<'_, Vec<(usize, Vec<Track>, Cost)>>> = ranges
                .into_iter()
                .map(|r| -> crate::par::Job<'_, Vec<(usize, Vec<Track>, Cost)>> {
                    Box::new(move || sensing[r].iter().map(|&i| one(i)).collect())
                })
                .collect();
            crate::par::run(threads, jobs).into_iter().flatten().collect()
        }
    };
    for (i, tracks, cost) in results {
        let (id, founder) = (fauna.animals[i].id, fauna.animals[i].founder.expect("a live lineage"));
        fold(fauna, id, founder, &tracks, None);
        let l = &mut fauna.ledger;
        l.percept_sensings += 1;
        l.percept_pairs += cost.pairs;
        l.percept_ray_cells += cost.cells;
    }
}

/// What a reflex is about to read of its record ([`refresh`]).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Want {
    pub threats: bool,
    pub prey: bool,
    pub tremor: bool,
    pub scent: bool,
    pub carcasses: bool,
}

/// The record of body `id`.
pub(crate) fn percept(fauna: &Fauna, id: u64) -> Option<&Percept> {
    fauna.percepts.get(&id)
}

/// **The tick-rate parts and the interim rules** (module note) of the body at `i`, before a
/// reflex reads its record: for a live lineage the tremor and scent readings and the
/// carcasses; for any other the parts `want` asks for, by the rules its reflexes ran on
/// before the senses pass.
pub(crate) fn refresh(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    occupancy: Option<crate::senses::Occ<'_>>,
    want: Want,
) {
    let a = fauna.animals[i];
    let Some(founder) = a.founder else { return };
    let table = fauna.config.founder(founder).senses;
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    if want.carcasses {
        let scan = fauna.config.hunt.carcass_scan_m.max(0.0);
        let mut near: Vec<(f64, (u32, u32, u32))> = fauna
            .carcass_sites(fv)
            .iter()
            .filter_map(|s| {
                let (x, z) = ((f64::from(s.x) + 0.5) * v, (f64::from(s.z) + 0.5) * v);
                let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, x, z, ring);
                let d = (dx * dx + dz * dz).sqrt();
                (d <= scan).then_some((d, (s.x, s.y, s.z)))
            })
            .collect();
        near.sort_by(|p, q| p.0.total_cmp(&q.0).then(p.1.cmp(&q.1)));
        near.truncate(MAX_CARCASSES);
        // The carcass list's order is the world's (the meal step chooses by route or by
        // distance with its own tie-break).
        near.sort_by(|p, q| p.1.cmp(&q.1));
        let p = fauna.percepts.entry(a.id).or_default();
        p.n_carcasses = near.len() as u8;
        for (k, (_, s)) in near.into_iter().enumerate() {
            p.carcasses[k] = s;
        }
    }
    if table.live {
        if want.tremor || want.prey || want.threats {
            let t = tremor_reading(fauna, view, &a, &table);
            fauna.percepts.entry(a.id).or_default().tremor = t;
        }
        if want.scent || want.prey {
            let s = fauna.scent.read(view, &a);
            fauna.percepts.entry(a.id).or_default().scent = s;
        }
        return;
    }
    if want.tremor {
        let t = fauna.view().tremor(view, a.id);
        fauna.percepts.entry(a.id).or_default().tremor = t;
    }
    if want.threats {
        let t = interim_threat(fauna, view, &a, occupancy);
        fold(fauna, a.id, founder, t.as_slice(), Some(&[class::THREAT]));
    }
    if want.prey {
        let t = interim_prey(fauna, view, &a, occupancy);
        fold(fauna, a.id, founder, &t, Some(&[class::PREY]));
    }
}

/// The live tremor reading: footfall `mass × speed / d²` of every moving body on the ground
/// within its `tremor_m`, split left and right of its heading (no ray: the reading runs at
/// tick rate).
pub(crate) fn tremor_reading(fauna: &Fauna, view: &VoxelView<'_>, a: &Animal, table: &SenseTable) -> [f64; 2] {
    let mut out = [0.0, 0.0];
    if !(table.tremor_m > 0.0) {
        return out;
    }
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let (fx, fz) = a.pose.forward();
    let (rx, rz) = (fz, -fx);
    let near = fauna.config.predation.tremor_near_m;
    for o in &fauna.animals {
        if o.id == a.id || !(o.speed_m_s > 0.0) || o.mobility.mode != crate::MobilityMode::Ground || o.attached.is_some() {
            continue;
        }
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, o.pose.x, o.pose.z, ring);
        let d = (dx * dx + dz * dz).sqrt();
        if d > table.tremor_m {
            continue;
        }
        let t = crate::tremor_of(o, d, near);
        let side = dx * rx + dz * rz;
        if side > 0.0 {
            out[1] += t;
        } else if side < 0.0 {
            out[0] += t;
        } else {
            out[0] += 0.5 * t;
            out[1] += 0.5 * t;
        }
    }
    out
}

/// A track of `o` by the interim rules: position now, 2D distance in `d_m`.
fn interim_track(fauna: &Fauna, view: &VoxelView<'_>, a: &Animal, o: &Animal, class: u8, modes: u8, felt: f64) -> Track {
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, o.pose.x, o.pose.z, ring);
    let bearing = {
        let d = (dx.atan2(dz) - a.pose.heading_rad).rem_euclid(std::f64::consts::TAU);
        if d > std::f64::consts::PI { d - std::f64::consts::TAU } else { d }
    };
    Track {
        known: true,
        id: o.id,
        lineage: o.founder.map_or(u8::MAX, |f| f.index() as u8),
        class,
        modes,
        x: o.pose.x,
        z: o.pose.z,
        y_m: base_m(o, v),
        site: (o.site.x, o.site.y, o.site.z),
        lift_m: 0.0,
        grounded: o.mobility.mode == crate::MobilityMode::Ground && o.mobility.climb.is_none(),
        climbing: o.mobility.climb.is_some(),
        moving: crate::moved_recently(&fauna.config.predation, o),
        d_m: (dx * dx + dz * dz).sqrt(),
        bearing,
        elevation: 0.0,
        loud: 0.0,
        felt,
        size_m: crate::body_of(&fauna.config, o).map_or(0.0, |g| g.length_m),
        tick: fauna.tick,
        heading: o.pose.heading_rad,
        posture: if modes & SIGHT != 0 { posture_of(&fauna.config, o) } else { posture::ACTIVE },
    }
}

/// The flee rule's threat (predation §2.7), unchanged: the nearest body within
/// `flee_sight_m` that is a threat by [`crate::kind_of`], ahead of it, on a ray of the
/// predation occupancy that meets the body.
fn interim_threat(fauna: &Fauna, view: &VoxelView<'_>, a: &Animal, occupancy: Option<crate::senses::Occ<'_>>) -> Option<Track> {
    let occupancy = occupancy?;
    let config = &fauna.config;
    let geometry = crate::body_of(config, a)?;
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let origin = crate::senses::cone_origin(view, &a.pose, a.sense_layer(v), &geometry);
    let (fx, fz) = a.pose.forward();
    let mut nearest: Option<(f64, usize)> = None;
    for (k, o) in fauna.animals.iter().enumerate() {
        if o.id == a.id {
            continue;
        }
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, o.pose.x, o.pose.z, ring);
        let d = (dx * dx + dz * dz).sqrt();
        if d > config.predation.flee_sight_m || nearest.is_some_and(|(n, _)| d >= n) {
            continue;
        }
        if crate::kind_of(config, a, o, d) != crate::BodyKind::Threat {
            continue;
        }
        if dx * fx + dz * fz < 0.0 {
            continue;
        }
        let Some(og) = crate::body_of(config, o) else { continue };
        let ty = crate::surface_m(o.sense_layer(v), v) + 0.5 * og.height_m;
        let (ex, ey, ez) = (dx, ty - origin.1, dz);
        let dist = (ex * ex + ey * ey + ez * ez).sqrt();
        if !(dist > 0.0) {
            continue;
        }
        let dir = (ex / dist, ey / dist, ez / dist);
        let seen = matches!(
            crate::senses::ray_first_hit(view, &occupancy, a.id, origin, dir, dist + og.width_m),
            Some((hit, crate::senses::Fine::Body)) if hit >= dist - og.width_m - v
        );
        if seen {
            nearest = Some((d, k));
        }
    }
    let (_, k) = nearest?;
    Some(interim_track(fauna, view, a, &fauna.animals[k], class::THREAT, SIGHT, 0.0))
}

/// Whether the hunter at `a` sees `o` by the patrol's rule: ahead of it and not hidden, by
/// the flee rule's ray against the predation occupancy (every body seen without one).
fn interim_sees(fauna: &Fauna, view: &VoxelView<'_>, occupancy: Option<crate::senses::Occ<'_>>, a: &Animal, o: &Animal, (dx, dz): (f64, f64)) -> bool {
    let (fx, fz) = a.pose.forward();
    if dx * fx + dz * fz < 0.0 {
        return false;
    }
    let Some(occupancy) = occupancy else {
        return true;
    };
    let config = &fauna.config;
    let (Some(geometry), Some(og)) = (crate::body_of(config, a), crate::body_of(config, o)) else {
        return false;
    };
    let v = view.config.voxel_m;
    let origin = crate::senses::cone_origin(view, &a.pose, a.sense_layer(v), &geometry);
    let ty = base_m(o, v) + 0.5 * og.height_m;
    let (ex, ey, ez) = (dx, ty - origin.1, dz);
    let dist = (ex * ex + ey * ey + ez * ez).sqrt();
    if !(dist > 0.0) {
        return true;
    }
    let dir = (ex / dist, ey / dist, ez / dist);
    matches!(
        crate::senses::ray_first_hit(view, &occupancy, a.id, origin, dir, dist + og.width_m),
        Some((hit, crate::senses::Fine::Body)) if hit >= dist - og.width_m - v
    )
}

/// The patrol's prey (the detritus web D2), unchanged: every body it takes
/// ([`crate::is_prey_of`]) within `max(hunt.sight_m, tremor_radius_m)` that it sees within
/// `hunt.sight_m` or whose tremor reaches it within `tremor_radius_m`; in body order.
fn interim_prey(fauna: &Fauna, view: &VoxelView<'_>, a: &Animal, occupancy: Option<crate::senses::Occ<'_>>) -> Vec<Track> {
    let cfg = fauna.config.hunt;
    let pc = fauna.config.predation;
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let reach = cfg.sight_m.max(pc.tremor_radius_m);
    let mut out: Vec<(f64, Track)> = Vec::new();
    for o in &fauna.animals {
        if o.id == a.id || !crate::is_prey_of(&fauna.config, a, o) {
            continue;
        }
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, o.pose.x, o.pose.z, ring);
        let d = (dx * dx + dz * dz).sqrt();
        if d > reach {
            continue;
        }
        let felt = if d <= pc.tremor_radius_m { crate::tremor_of(o, d, pc.tremor_near_m) } else { 0.0 };
        let seen = d <= cfg.sight_m && interim_sees(fauna, view, occupancy, a, o, (dx, dz));
        if !(seen || felt > 0.0) {
            continue;
        }
        let modes = if seen { SIGHT } else { 0 } | if felt > 0.0 { TREMOR } else { 0 };
        out.push((d, interim_track(fauna, view, a, o, class::PREY, modes, felt)));
    }
    if out.len() > MAX_TRACKS {
        out.sort_by(|p, q| p.0.total_cmp(&q.0).then(p.1.id.cmp(&q.1.id)));
        out.truncate(MAX_TRACKS);
        out.sort_by_key(|p| p.1.id);
    }
    out.into_iter().map(|(_, t)| t).collect()
}

/// **The funnel's approached stage**: hunter `a` sets out after prey `prey` (a pack target,
/// a draw, a patrol's engage); counted once per episode ([`Percept::first`]).
pub(crate) fn approach(fauna: &mut Fauna, hunter: u64, prey: u64) {
    let Some(hf) = fauna.view().animal(hunter).and_then(|a| a.founder) else { return };
    let Some(pf) = fauna.view().animal(prey).and_then(|a| a.founder) else { return };
    let (now, window) = (fauna.tick, funnel_window(&fauna.config));
    if fauna.percepts.entry(hunter).or_default().first(APPROACHED, prey, now, window) {
        fauna.ledger.funnel[hf.index()][pf.index()][APPROACHED] += 1;
    }
}

/// The funnel's episode, ticks: the pursuit's memory ([`crate::PerceptionConfig::give_up_s`]).
pub(crate) fn funnel_window(config: &FaunaConfig) -> u64 {
    cubarium_voxel::ticks_at_least_one(config.perception.give_up_s)
}

/// **The funnel's eaten stage**: hunter `hunter` bites the carcass of a `prey` on `face`;
/// counted once per carcass and hunter.
pub(crate) fn eaten(fauna: &mut Fauna, hunter: u64, hf: Founder, prey: usize, face: cubarium_voxel_flora::Site) {
    if prey >= Founder::COUNT {
        return;
    }
    let key = (u64::from(face.x) << 42) | (u64::from(face.y) << 21) | u64::from(face.z);
    let (now, window) = (fauna.tick, funnel_window(&fauna.config));
    if fauna.percepts.entry(hunter).or_default().first(EATEN, key, now, window) {
        fauna.ledger.funnel[hf.index()][prey][EATEN] += 1;
    }
}

/// Whether hunter `h` (its table `hunter`, current body) could take a sensed body where it
/// was: on the ground, or with its elevated base within the hunter's strike height.
/// A rearing hunter also reaches a body clinging to a wall within that band.
pub fn in_reach(config: &FaunaConfig, h: &Animal, t: &Track) -> bool {
    // A ground prey can be pursued over walkable terrain; the route finds a strike
    // surface at its elevation. An elevated perch needs the vertical reach check.
    if t.grounded {
        return true;
    }
    let (Some(geometry), Some(table)) = (crate::body_of(config, h), h.founder.and_then(|f| config.founder(f).active_hunter())) else {
        return false;
    };
    if t.climbing && table.vertical_reach_m <= 0.0 {
        return false;
    }
    // Compare with this hunter's base, not the terrain directly under the prey: a
    // mesa-top body can stand on terrain while still being above a hunter below.
    t.y_m < h.pose.y_m + table.strike_height_m(&geometry)
}

/// The levels and gradients of the channels hunter lineage `f` follows for its prey: every
/// channel whose layers are all on its list, and which its own lineage does not lay.
pub fn prey_channels(config: &FaunaConfig, f: Founder) -> [bool; ScentChannel::COUNT] {
    let mut out = [false; ScentChannel::COUNT];
    let Some(h) = config.founder(f).active_hunter() else { return out };
    let own = config.founder(f).senses.scent;
    for ch in ScentChannel::ALL {
        if ch == ScentChannel::Carrion || own == Some(ch) {
            continue;
        }
        let mut layers = Founder::ALL.iter().filter(|&&g| config.founder(g).senses.scent == Some(ch)).peekable();
        out[ch.index()] = layers.peek().is_some() && layers.all(|&g| h.hunts(g));
    }
    out
}
