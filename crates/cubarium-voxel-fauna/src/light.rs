//! **Light and flight** (S3 of the senses pass; `design/7_Research/senses-review-2026-10-06.md`
//! §1, §2 and §5 S3, with Astra's design in `design/7_Research/astra-senses-2026-10-06.md` and
//! Wrysk's choices D1–D3): the bright things, the one photoreceptor that reads them, and the
//! two lineages that follow them — the bellwing (no form vision: smell leads, brightness
//! steers) and the capgnawer (the glowcaps' cyan, weakly, when hungry and with no fungal
//! scent near). Every number is [`LightConfig`]'s or the lineage's [`crate::SenseTable`]'s.
//!
//! # Beacons ([`Beacon`])
//!
//! - **Flowers** — a bloomcrown's head holding nectar, a latticevine face in flower holding
//!   nectar: UV reflectors, bright by the light falling on them (`sky × daylight`), dark at
//!   night. A whole plant is never a beacon; only its flower, and only while it holds nectar.
//! - **Glowcaps** — a glowcap with a cap: its own cyan light.
//! - **Lures** — a lanternjaw's lit lure, awake: its own cyan light, at the middle of its
//!   strike capsule inside its jaw band ([`lure_point`]); **moving** when the lanternjaw has
//!   moved within the still window ([`crate::moved_recently`]).
//!
//! Flowers and glowcaps are indexed and rebuilt on the plant clock (when the plant layer's
//! tick or the terrain changes, [`Beacons::refresh`]); the lures are read live.
//!
//! # The photoreceptor ([`refresh_light`])
//!
//! A beacon's **contrast** is its brightness over the light around it plus a floor (the
//! darkest background): a lure or a glowcap is bright in a cave and at night and washed out
//! in the open sun; a flower is bright by day and dark at night. The received **level** is
//! the contrast × `1 / (1 + (d / half_m)²)` (3D distance from the eye). Within the lineage's
//! `light_m`, at its `light_min` or above, on a clear terrain line: the brightest **still**
//! beacon not under a failed-visit mark is its beacon, the brightest **moving** one its
//! repellent. A body without the `uv` channel sees glowcaps and lures only. **Looming** is
//! the nearest moving body of another lineage within `loom_m`.
//!
//! # The bellwing ([`steer_flyer`])
//!
//! Over its hover-sip heuristic, while its table is live:
//! 1. a sip at the mouth stands (and a visit there was rewarded);
//! 2. **a looming shadow repels**: away from it and up;
//! 3. fed, perched, dropping or blocked: its heuristic's answer stands;
//! 4. **a moving light repels** when it outshines the brightest still one;
//! 5. **a still light attracts**: the approach — toward it, down or up to it, slowing near
//!    it, mouth held open; within `visit_m` it inspects; an inspection that brings no sip
//!    within `visit_s` **fails**: the place is marked "no reward here" for `mark_s` and the
//!    bellwing goes back to its search;
//! 6. **smell leads**: up the nectar channel's gradient where it smells it;
//! 7. otherwise its heuristic (the wander, its flower fan).
//!
//! # The capgnawer ([`steer_glow`], D2)
//!
//! Its fungal chemistry first (its forager's `Chem`): only hungry, with the chemistry under
//! [`LightConfig::glow_scent_gate`] and no food at its mouth, does a still cyan light within
//! its short `light_m` draw it; the inspection and the mark are the bellwing's.

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::{FloraView, Species, StandLayer};
use serde::{Deserialize, Serialize};

use crate::perception::{TerrainOnly, Visit};
use crate::{Actions, Animal, Fauna, FaunaConfig, LightConfig, MobilityMode, ScentChannel};

/// What a beacon is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BeaconKind {
    /// A flower holding nectar (UV, reflected).
    Flower,
    /// A glowcap's cap (cyan, emitted).
    Glowcap,
    /// A lanternjaw's lit lure (cyan, emitted).
    Lure,
}

impl BeaconKind {
    pub const COUNT: usize = 3;
    pub const ALL: [BeaconKind; 3] = [BeaconKind::Flower, BeaconKind::Glowcap, BeaconKind::Lure];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(k: u8) -> BeaconKind {
        match k {
            0 => BeaconKind::Flower,
            1 => BeaconKind::Glowcap,
            _ => BeaconKind::Lure,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            BeaconKind::Flower => "flower",
            BeaconKind::Glowcap => "glowcap",
            BeaconKind::Lure => "lure",
        }
    }
}

/// **One bright thing** (module note): its kind, where it shines from (ring `x`, `z`,
/// height `y`, metres), the terrain sky visibility there (`0..=1`), its own light's level
/// (a lure's lit fraction; 1 for the others) and whether it moved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beacon {
    pub kind: BeaconKind,
    pub x: f64,
    pub z: f64,
    pub y_m: f64,
    pub sky: f64,
    pub power: f64,
    pub moving: bool,
    /// The lure's body (0 for a flower or a glowcap).
    pub body: u64,
}

impl Beacon {
    /// Its brightness: a flower's reflected light, a glowcap's or a lure's own.
    pub fn brightness(&self, cfg: &LightConfig, daylight: f64) -> f64 {
        match self.kind {
            BeaconKind::Flower => cfg.flower_reflectance.max(0.0) * self.sky.clamp(0.0, 1.0) * daylight.clamp(0.0, 1.0),
            BeaconKind::Glowcap => cfg.glowcap_emission.max(0.0),
            BeaconKind::Lure => cfg.lure_emission.max(0.0) * self.power.clamp(0.0, 1.0),
        }
    }

    /// Its contrast: brightness over the light around it plus the floor.
    pub fn contrast(&self, cfg: &LightConfig, daylight: f64) -> f64 {
        let around = self.sky.clamp(0.0, 1.0) * daylight.clamp(0.0, 1.0);
        let b = self.brightness(cfg, daylight);
        let den = around + cfg.background_floor.max(0.0);
        if den > 0.0 { b / den } else if b > 0.0 { f64::INFINITY } else { 0.0 }
    }
}

/// The level a beacon of `contrast` is received at `d` metres (module note).
pub fn received(cfg: &LightConfig, contrast: f64, d: f64) -> f64 {
    let half = cfg.half_m.max(1e-9);
    contrast / (1.0 + (d / half) * (d / half))
}

/// **The static beacons** (flowers, glowcaps), rebuilt on the plant clock: derived, not saved.
#[derive(Clone, Debug, Default)]
pub struct Beacons {
    /// The plant layer's tick and the terrain version it was built at.
    stamp: Option<(u64, u64)>,
    list: Vec<Beacon>,
}

/// The top foliage-bearing layer of a stand (its flower head, a cap's top).
fn top_layer(fv: &FloraView<'_>, stand: &cubarium_voxel_flora::Stand) -> Option<StandLayer> {
    fv.profile_layers(stand).into_iter().filter(|l| l.kind.bears_foliage()).max_by_key(|l| l.cell)
}

impl Beacons {
    /// Rebuild when the plant layer stepped or the terrain changed since the last build.
    pub(crate) fn refresh(&mut self, view: &VoxelView<'_>, fv: &FloraView<'_>) {
        let stamp = (fv.tick, view.terrain_version);
        if self.stamp == Some(stamp) {
            return;
        }
        self.stamp = Some(stamp);
        self.list = Beacons::build(view, fv);
    }

    /// Every flower holding nectar and every glowcap with a cap (module note).
    pub(crate) fn build(view: &VoxelView<'_>, fv: &FloraView<'_>) -> Vec<Beacon> {
        let c = view.config;
        let v = c.voxel_m;
        let mut out = Vec::new();
        for st in fv.stands {
            let kind = if st.nectar > 0.0 {
                BeaconKind::Flower
            } else if st.species == Species::Glowcap && st.foliage > 0.0 {
                BeaconKind::Glowcap
            } else {
                continue;
            };
            let (row, y_m) = match top_layer(fv, st) {
                Some(l) => (l.cell.max(0) as u32, (l.cell + 1) as f64 * v),
                None => (st.site.y, crate::surface_m(st.site.y, v)),
            };
            let sky = view.sky_visibility(i64::from(st.site.x), row.min(c.height.saturating_sub(1)), st.site.z);
            out.push(Beacon {
                kind,
                x: (f64::from(st.site.x) + 0.5) * v,
                z: (f64::from(st.site.z) + 0.5) * v,
                y_m,
                sky,
                power: 1.0,
                moving: false,
                body: 0,
            });
        }
        for f in fv.cover.flowers() {
            let Some((x, y, z)) = f.face.front(view) else { continue };
            out.push(Beacon {
                kind: BeaconKind::Flower,
                x: (f64::from(x) + 0.5) * v,
                z: (f64::from(z) + 0.5) * v,
                y_m: (f64::from(y) + 0.5) * v,
                sky: view.sky_visibility(i64::from(x), y, z),
                power: 1.0,
                moving: false,
                body: 0,
            });
        }
        out
    }

    /// The static beacons as last built.
    pub fn list(&self) -> &[Beacon] {
        &self.list
    }
}

/// **The lure's point** (module note): over a lanternjaw's standing surface by
/// [`LightConfig::lure_height_fraction`] of its height, ahead of its centre at the middle of
/// its strike capsule; `None` for a body whose table pays no lure.
pub fn lure_point(config: &FaunaConfig, a: &Animal, voxel_m: f64) -> Option<(f64, f64, f64)> {
    let h = config.founder(a.founder?).hunter?;
    if !(h.lure_power > 0.0) {
        return None;
    }
    let g = crate::body_of(config, a)?;
    let ahead = (h.jaw_forward_length_fraction + 0.5 * h.strike_reach_length_fraction) * g.length_m;
    let base = if a.pose.y_m.is_finite() { a.pose.y_m } else { crate::surface_m(a.site.y, voxel_m) };
    Some((
        a.pose.x + ahead * a.pose.heading_rad.sin(),
        a.pose.z + ahead * a.pose.heading_rad.cos(),
        base + config.light.lure_height_fraction.max(0.0) * g.height_m,
    ))
}

/// The lit lures now: every awake, free body with a lure lit ([`crate::FounderState::lure`]).
pub fn lures(fauna: &Fauna, view: &VoxelView<'_>) -> Vec<Beacon> {
    let c = view.config;
    let v = c.voxel_m;
    let depth = f64::from(c.depth) * v;
    fauna
        .animals
        .iter()
        .filter(|a| a.state != crate::State::Asleep && a.attached.is_none() && a.founder_state.lure > 0.0)
        .filter_map(|a| {
            let (x, z, y) = lure_point(&fauna.config, a, v)?;
            let z = z.clamp(0.0, (depth - 1e-6).max(0.0));
            Some(Beacon {
                kind: BeaconKind::Lure,
                x,
                z,
                y_m: y,
                sky: view.sky_visibility(i64::from(a.site.x), a.site.y, a.site.z),
                power: a.founder_state.lure,
                moving: crate::moved_recently(&fauna.config.predation, a),
                body: a.id,
            })
        })
        .collect()
}

/// The bearing of `(dx, dz)` from `heading`, radians in `(−π, π]`, positive to the right.
fn bearing(heading: f64, dx: f64, dz: f64) -> f64 {
    use std::f64::consts::{PI, TAU};
    let d = (dx.atan2(dz) - heading).rem_euclid(TAU);
    if d > PI { d - TAU } else { d }
}

/// A body's eye: its centre column at its eye height over its base.
fn eye_of(config: &FaunaConfig, a: &Animal, v: f64) -> Option<(f64, f64, f64)> {
    let g = crate::body_of(config, a)?;
    Some((a.pose.x, crate::perception::base_m(a, v) + g.eye_m, a.pose.z))
}

/// At most this many beacons are tested for a clear line per reading, brightest first (a
/// cost bound, not ecology).
const MAX_LIGHT_RAYS: usize = 4;

/// **The photoreceptor's reading** of the body at `i` (module note), written to its record's
/// [`crate::perception::LightReading`]: the looming every call, the beacons at the remote
/// senses' rate (staggered by id) or when it has none yet. `daylight` is the sky's (1 with
/// the clock off).
pub(crate) fn refresh_light(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>, daylight: f64) {
    let a = fauna.animals[i];
    let Some(founder) = a.founder else { return };
    let table = fauna.config.founder(founder).senses;
    if !(table.light_m > 0.0) && !(table.loom_m > 0.0) {
        return;
    }
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let Some(eye) = eye_of(&fauna.config, &a, v) else { return };
    let tick = fauna.tick;
    // Looming: the nearest moving body of another lineage within `loom_m` (3D, to its middle).
    let mut loom: Option<(f64, f64)> = None;
    if table.loom_m > 0.0 {
        for o in &fauna.animals {
            if o.id == a.id || o.founder == Some(founder) || o.attached.is_some() || !crate::moved_recently(&fauna.config.predation, o) {
                continue;
            }
            let Some(og) = crate::body_of(&fauna.config, o) else { continue };
            let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, o.pose.x, o.pose.z, ring);
            if dx.abs() > table.loom_m + og.length_m || dz.abs() > table.loom_m + og.length_m {
                continue;
            }
            let dy = crate::perception::base_m(o, v) + 0.5 * og.height_m - eye.1;
            let d = (dx * dx + dy * dy + dz * dz).sqrt();
            if d <= table.loom_m && loom.is_none_or(|(n, _)| d < n) {
                loom = Some((d, bearing(a.pose.heading_rad, dx, dz)));
            }
        }
    }
    let stagger = fauna.config.perception.stagger_ticks();
    let due = {
        let p = fauna.percepts.get(&a.id);
        p.is_none_or(|p| p.light.tick == 0) || a.id % stagger == tick % stagger
    };
    let ambient = daylight.clamp(0.0, 1.0) * view.sky_visibility(eye.0.div_euclid(v) as i64, a.sense_layer(v), a.site.z);
    let mut reading = fauna.percepts.get(&a.id).map(|p| p.light).unwrap_or_default();
    reading.ambient = ambient;
    reading.looming = loom.map_or(0.0, |(d, _)| (1.0 - d / table.loom_m).clamp(0.0, 1.0).max(1e-9));
    reading.loom_bearing = loom.map_or(0.0, |(_, b)| b);
    if due && table.light_m > 0.0 {
        fauna.beacons.refresh(view, fv);
        let cfg = fauna.config.light;
        let record = fauna.percepts.get(&a.id).copied().unwrap_or_default();
        let lures = lures(fauna, view);
        let mut seen: Vec<(f64, f64, Beacon)> = Vec::new();
        for b in fauna.beacons.list().iter().chain(lures.iter()) {
            if (b.kind == BeaconKind::Lure && b.body == a.id) || (!table.uv && b.kind == BeaconKind::Flower) {
                continue;
            }
            let (dx, dz) = crate::pose::wrapped_delta(eye.0, eye.2, b.x, b.z, ring);
            if dx.abs() > table.light_m || dz.abs() > table.light_m {
                continue;
            }
            let dy = b.y_m - eye.1;
            let d = (dx * dx + dy * dy + dz * dz).sqrt();
            if d > table.light_m {
                continue;
            }
            if !b.moving && record.marked(b.x, b.z, tick, cfg.mark_radius_m, ring) {
                continue;
            }
            let level = received(&cfg, b.contrast(&cfg, daylight), d);
            if level >= table.light_min && level > 0.0 {
                seen.push((level, d, *b));
            }
        }
        seen.sort_by(|p, q| q.0.total_cmp(&p.0).then(p.1.total_cmp(&q.1)));
        let (mut still, mut moving): (Option<(f64, Beacon)>, Option<(f64, Beacon)>) = (None, None);
        for &(level, d, b) in seen.iter().take(MAX_LIGHT_RAYS) {
            if (b.moving && moving.is_some()) || (!b.moving && still.is_some()) {
                continue;
            }
            let (dx, dz) = crate::pose::wrapped_delta(eye.0, eye.2, b.x, b.z, ring);
            let dy = b.y_m - eye.1;
            let clear = d <= 0.0 || {
                let dir = (dx / d, dy / d, dz / d);
                crate::senses::ray_first_hit(view, &TerrainOnly, a.id, eye, dir, d).is_none_or(|(h, _)| h >= d - v)
            };
            if !clear {
                continue;
            }
            if b.moving {
                moving = Some((level, b));
            } else {
                still = Some((level, b));
            }
        }
        reading.beacon = still.is_some();
        if let Some((level, b)) = still {
            let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, b.x, b.z, ring);
            reading.beacon_kind = b.kind.index() as u8;
            reading.beacon_level = level;
            reading.beacon_bearing = bearing(a.pose.heading_rad, dx, dz);
            reading.beacon_x = b.x;
            reading.beacon_z = b.z;
            reading.beacon_y = b.y_m;
        } else {
            reading.beacon_level = 0.0;
        }
        match moving {
            Some((level, b)) => {
                let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, b.x, b.z, ring);
                reading.repel_level = level;
                reading.repel_bearing = bearing(a.pose.heading_rad, dx, dz);
            }
            None => reading.repel_level = 0.0,
        }
        reading.tick = tick.max(1);
    }
    fauna.percepts.entry(a.id).or_default().light = reading;
}

/// What an inspection came to ([`inspect`]).
enum Inspection {
    /// Still inspecting (or just begun).
    Going,
    /// It failed: the place is marked.
    Failed,
}

/// The inspection of the beacon in the record of the body at `i`, which is within
/// `visit_m` of it: begun now (counted), going on, or failed after `visit_s` (marked and
/// counted).
fn inspect(fauna: &mut Fauna, i: usize, ring: f64) -> Inspection {
    let a = fauna.animals[i];
    let Some(founder) = a.founder else { return Inspection::Going };
    let cfg = fauna.config.light;
    let tick = fauna.tick;
    let p = fauna.percepts.entry(a.id).or_default();
    let (bx, bz, kind) = (p.light.beacon_x, p.light.beacon_z, p.light.beacon_kind);
    let same = p.visit.active && {
        let (dx, dz) = crate::pose::wrapped_delta(p.visit.x, p.visit.z, bx, bz, ring);
        dx * dx + dz * dz <= cfg.mark_radius_m * cfg.mark_radius_m
    };
    if !same {
        p.visit = Visit { active: true, kind, x: bx, z: bz, since: tick };
        fauna.ledger.beacon_visits[founder.index()][usize::from(kind).min(BeaconKind::COUNT - 1)] += 1;
        return Inspection::Going;
    }
    if tick.saturating_sub(p.visit.since) < cubarium_voxel::ticks_at_least_one(cfg.visit_s) {
        return Inspection::Going;
    }
    let until = tick + cubarium_voxel::ticks_at_least_one(cfg.mark_s);
    p.mark(bx, bz, until);
    p.visit = Visit::default();
    p.light.beacon = false;
    fauna.ledger.failed_visits[founder.index()][usize::from(kind).min(BeaconKind::COUNT - 1)] += 1;
    Inspection::Failed
}

/// The inspection of the body `id` ended in a meal (its mouth took food): no mark.
fn rewarded(fauna: &mut Fauna, id: u64) {
    if let Some(p) = fauna.percepts.get_mut(&id) {
        p.visit = Visit::default();
    }
}

/// Away from a bearing (radians from the heading, positive right): a full turn to the
/// other side.
fn away(bearing: f64) -> f64 {
    if bearing > 0.0 { -1.0 } else { 1.0 }
}

/// **The bellwing's light and smell** over its heuristic (module note): `asked` and
/// `asked_lift` are its heuristic's answer this sample; `Some((actions, lift))` replaces
/// them, `None` leaves them. Only a live table with a light sense on a flyer.
pub(crate) fn steer_flyer(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    daylight: f64,
    asked: Actions,
    asked_lift: f64,
) -> Option<(Actions, f64)> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let table = phys.senses;
    phys.flight?;
    if !table.live || a.attached.is_some() {
        return None;
    }
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    // 1. A sip at its mouth stands: an inspection that ends in a sip is rewarded.
    if asked.feed > 0.5 && asked.forward == 0.0 && asked.turn == 0.0 && asked_lift == 0.0 {
        rewarded(fauna, a.id);
        return None;
    }
    refresh_light(fauna, i, view, fv, daylight);
    let p = *fauna.percepts.get(&a.id)?;
    // 2. A looming shadow repels: away and up.
    if p.light.looming > 0.0 {
        fauna.ledger.loom_samples[founder.index()] += 1;
        rewarded(fauna, a.id);
        return Some((Actions { forward: 1.0, turn: away(p.light.loom_bearing), feed: 0.0 }, 1.0));
    }
    // 3. Fed, perched, dropping or blocked: its heuristic's.
    let fb = a.founder_state.feedback;
    let refused = fb.attempted_equivalent > 0.0 && fb.delivered_equivalent / fb.attempted_equivalent < fauna.config.teachers.motor_refused;
    if !a.eating || a.mobility.mode != MobilityMode::Fly || a.mobility.falling || refused {
        return None;
    }
    let cfg = fauna.config.light;
    // 4. A moving light repels when it outshines the brightest still one.
    if p.light.repel_level > 0.0 && p.light.repel_level >= p.light.beacon_level {
        fauna.ledger.repel_samples[founder.index()] += 1;
        return Some((Actions { forward: 1.0, turn: away(p.light.repel_bearing), feed: 0.0 }, asked_lift));
    }
    // 5. A still light attracts.
    if p.light.beacon {
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, p.light.beacon_x, p.light.beacon_z, ring);
        let dh = (dx * dx + dz * dz).sqrt();
        let h = phys.body_at(a.body).height_m;
        let lift = (cfg.lift_per_m * (p.light.beacon_y - 0.5 * h - a.pose.y_m)).clamp(-1.0, 1.0);
        let want = dx.atan2(dz);
        if dh <= cfg.visit_m {
            if let Inspection::Failed = inspect(fauna, i, ring) {
                return None;
            }
            fauna.ledger.beacon_samples[founder.index()] += 1;
            let mut act = crate::hunt::head(a.pose.heading_rad, want, 0.0, 1.0);
            act.forward = 0.0;
            return Some((act, lift));
        }
        fauna.ledger.beacon_samples[founder.index()] += 1;
        let forward = if dh <= cfg.near_m { cfg.near_forward } else { 1.0 };
        return Some((crate::hunt::head(a.pose.heading_rad, want, forward, 1.0), lift));
    }
    // 6. Smell leads: up the nectar gradient.
    if table.smell_min > 0.0 {
        let s = fauna.scent.read_below(view, a.pose.x, a.pose.z, a.pose.y_m)[ScentChannel::Nectar.index()];
        if s.level >= table.smell_min && (s.gx != 0.0 || s.gz != 0.0) {
            fauna.ledger.scent_led_samples[founder.index()] += 1;
            return Some((crate::hunt::head(a.pose.heading_rad, s.gx.atan2(s.gz), 1.0, 0.0), asked_lift));
        }
    }
    None
}

/// **The capgnawer's cyan** (D2, module note) over its forager: `asked` is its forager's
/// answer, `fungal` the response its `Chem` reads (`None` when it reads none). `Some`
/// replaces the answer. Only a live table with a light sense on a body that walks.
pub(crate) fn steer_glow(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    daylight: f64,
    asked: Actions,
    fungal: Option<f64>,
) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let table = phys.senses;
    if !table.live || !(table.light_m > 0.0) || phys.flight.is_some() || a.attached.is_some() || a.mobility.climb.is_some() {
        return None;
    }
    // Food at its mouth: its forager eats, and an inspection that ends here is rewarded.
    if asked.feed > 0.5 {
        rewarded(fauna, a.id);
        return None;
    }
    let cfg = fauna.config.light;
    // Its chemistry leads; fed, it does not look.
    if !a.eating || fungal.is_some_and(|q| q >= cfg.glow_scent_gate) {
        return None;
    }
    refresh_light(fauna, i, view, fv, daylight);
    let p = *fauna.percepts.get(&a.id)?;
    if !p.light.beacon {
        return None;
    }
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, p.light.beacon_x, p.light.beacon_z, ring);
    let dh = (dx * dx + dz * dz).sqrt();
    let want = dx.atan2(dz);
    fauna.ledger.beacon_samples[founder.index()] += 1;
    if dh <= cfg.visit_m {
        if let Inspection::Failed = inspect(fauna, i, ring) {
            return None;
        }
        let mut act = crate::hunt::head(a.pose.heading_rad, want, 0.0, 1.0);
        act.forward = 0.0;
        return Some(act);
    }
    let forward = if dh <= cfg.near_m { cfg.near_forward } else { 1.0 };
    Some(crate::hunt::head(a.pose.heading_rad, want, forward, 1.0))
}

/// Whether the body `id` is drawn to a lure now — inspecting one, or with a lure its
/// brightest still beacon (the lure-kill readout).
pub(crate) fn drawn_to_lure(fauna: &Fauna, id: u64) -> bool {
    let lure = BeaconKind::Lure.index() as u8;
    fauna.percepts.get(&id).is_some_and(|p| (p.visit.active && p.visit.kind == lure) || (p.light.beacon && p.light.beacon_kind == lure))
}
