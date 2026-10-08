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
//!   moved within the still window ([`crate::moved_recently`]). **It mimics a flower**
//!   (BELLWING): it also shows UV ([`LightConfig::lure_uv`] × a flower's reflectance), so an
//!   eye with the `uv` channel sees its UV and its cyan together and reads it as a flower —
//!   the UV class ([`BeaconKind::class_seen`]); an eye without UV sees its cyan alone. It
//!   lays a flower's nectar scent too ([`LightConfig::lure_scent`], on its lanternjaw's
//!   face), so a flyer led by smell comes within sight of it. A visit that finds no nectar
//!   is a failed UV visit — the mimic's cost to the flowers' signal.
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
//! 6. **smell leads**: up the nectar plume's gradient where it smells it, in 3D — the
//!    plume comes from the flower head, so it leads to the flower, at its height
//!    (NECTAR-SCENT, [`crate::scent::nectar_plume`]);
//! 7. otherwise its heuristic (the wander, its flower fan).
//!
//! **LEADS-PAY** (`step::rove`): the approach (5) and the scent (6) are leads. A body that
//! has steered by leads for its `lead_give_up_bl` budget since its last sip gives the lead
//! up: the still light, or the plume's main source, is marked as a failed visit marks it
//! (unlearned), and its heuristic's answer stands. (It was found with nectar laid on the
//! ground at the plant's rooting face, which held hungry bellwings circling a metre or two
//! off the flower all night; NECTAR-SCENT moved the scent into the air at the bloom.)
//!
//! # Learning which light pays (S3b, [`expectation`])
//!
//! A flyer keeps a reward expectation per beacon **class** — UV (flowers) and cyan
//! (glowcaps, lures) — as an offset from the innate value ([`LightConfig::uv_innate`],
//! [`LightConfig::cyan_innate`]) in its percept record. A sip bout moves the class it came to
//! (the inspected beacon's, else UV: it sips at a flower) `learn_rate` of the way to 1; a
//! failed visit moves the visited class the same way to 0; the offset relaxes back with time
//! constant `forget_s`. A still beacon draws it by **level × expectation**, and a class
//! expected to pay under `expect_min` is not approached at all. A naive bellwing (both
//! classes innate) ranks beacons by level alone, as in S3: it falls for a still lure; an
//! experienced one passes it by. The spatial failed-visit mark stays as it is. Off
//! ([`LightConfig::learn`]): by level alone.
//!
//! # Two repairs (S3b)
//!
//! - **A beacon it cannot reach is given up**: a flyer steering for a still beacon short of
//!   it that has not moved for `visit_s` (wedged under a crown, against a wall) marks the
//!   place as a failed visit would, without learning from it ([`crate::FaunaLedger::stalled_visits`]).
//! - **Looming is an approach**: a moving body looms only while it closes on the eye fast
//!   enough to reach it within [`LightConfig::loom_tau_s`] (time to contact); a grazer
//!   cropping past or a capgnawer crossing under a hovering bellwing does not.
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
use crate::{Actions, Animal, Fauna, FaunaConfig, LightConfig, MobilityMode};

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

/// The learned classes ([`BeaconKind::class`]): UV reflectors and cyan emitters.
pub const CLASSES: usize = 2;
/// The UV class (flowers).
pub const UV: usize = 0;
/// The cyan class (glowcaps, lures).
pub const CYAN: usize = 1;

/// A kill's prey counts as **naive** about cyan (the readout
/// [`crate::FaunaLedger::naive_kills`]) while its cyan expectation is at least this fraction
/// of the innate value: a readout cut, not ecology.
pub const NAIVE_FRACTION: f64 = 0.5;

/// **What a flyer's light steer did with a controller sample** (S3b, the time budget,
/// [`crate::FaunaLedger::light_budget`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightMode {
    /// Sipping at its mouth.
    Sip,
    /// Evading a looming body.
    Loom,
    /// Fed, perched, dropping or blocked: its heuristic's answer.
    Held,
    /// Turned from a moving light.
    Repel,
    /// Gave up a beacon it stalled short of.
    Stall,
    ApproachFlower,
    ApproachGlowcap,
    ApproachLure,
    InspectFlower,
    InspectGlowcap,
    InspectLure,
    /// Led up the nectar smell.
    Scent,
    /// Its heuristic's search (wander, its flower fan).
    Search,
}

impl LightMode {
    pub const COUNT: usize = 13;
    pub const NAMES: [&'static str; 13] = [
        "sip", "loom", "held", "repel", "stall", "approach_flower", "approach_glowcap", "approach_lure", "inspect_flower", "inspect_glowcap", "inspect_lure", "scent", "search",
    ];

    fn approach(kind: u8) -> LightMode {
        match BeaconKind::from_index(kind) {
            BeaconKind::Flower => LightMode::ApproachFlower,
            BeaconKind::Glowcap => LightMode::ApproachGlowcap,
            BeaconKind::Lure => LightMode::ApproachLure,
        }
    }

    fn inspect(kind: u8) -> LightMode {
        match BeaconKind::from_index(kind) {
            BeaconKind::Flower => LightMode::InspectFlower,
            BeaconKind::Glowcap => LightMode::InspectGlowcap,
            BeaconKind::Lure => LightMode::InspectLure,
        }
    }
}

impl BeaconKind {
    pub const COUNT: usize = 3;

    /// Its learned class to an eye without UV: [`UV`] for a flower, [`CYAN`] for a glowcap
    /// or a lure ([`BeaconKind::class_seen`] for an eye with it).
    pub fn class(self) -> usize {
        match self {
            BeaconKind::Flower => UV,
            BeaconKind::Glowcap | BeaconKind::Lure => CYAN,
        }
    }
    /// Its class to an eye with (`uv`) or without the UV channel: a lure that shows UV
    /// ([`LightConfig::lure_uv`] > 0) is a flower — [`UV`] — to a UV eye; otherwise
    /// [`BeaconKind::class`].
    pub fn class_seen(self, cfg: &LightConfig, uv: bool) -> usize {
        match self {
            BeaconKind::Lure if uv && cfg.lure_uv > 0.0 => UV,
            k => k.class(),
        }
    }

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
    /// Its brightness to an eye without UV: a flower's reflected light, a glowcap's or a
    /// lure's own cyan.
    pub fn brightness(&self, cfg: &LightConfig, daylight: f64) -> f64 {
        self.brightness_seen(cfg, daylight, false)
    }

    /// Its brightness to an eye with (`uv`) or without the UV channel: a lure shows a UV eye
    /// its UV ([`LightConfig::lure_uv`] × `flower_reflectance`) on top of its cyan.
    pub fn brightness_seen(&self, cfg: &LightConfig, daylight: f64, uv: bool) -> f64 {
        match self.kind {
            BeaconKind::Flower => cfg.flower_reflectance.max(0.0) * self.sky.clamp(0.0, 1.0) * daylight.clamp(0.0, 1.0),
            BeaconKind::Glowcap => cfg.glowcap_emission.max(0.0),
            BeaconKind::Lure => {
                let mimic = if uv { cfg.lure_uv.max(0.0) * cfg.flower_reflectance.max(0.0) } else { 0.0 };
                (cfg.lure_emission.max(0.0) + mimic) * self.power.clamp(0.0, 1.0)
            }
        }
    }

    /// Its contrast to an eye without UV: brightness over the light around it plus the floor.
    pub fn contrast(&self, cfg: &LightConfig, daylight: f64) -> f64 {
        self.contrast_seen(cfg, daylight, false)
    }

    /// Its contrast to an eye with (`uv`) or without the UV channel.
    pub fn contrast_seen(&self, cfg: &LightConfig, daylight: f64, uv: bool) -> f64 {
        let around = self.sky.clamp(0.0, 1.0) * daylight.clamp(0.0, 1.0);
        let b = self.brightness_seen(cfg, daylight, uv);
        let den = around + cfg.background_floor.max(0.0);
        if den > 0.0 { b / den } else if b > 0.0 { f64::INFINITY } else { 0.0 }
    }
}

/// **The reward expectation** of `kind`'s class in the record `p` at tick `now` (module note,
/// "Learning"): the innate value plus the learned offset, relaxed over `forget_s`.
pub fn expectation(cfg: &LightConfig, p: &crate::perception::Percept, kind: BeaconKind, now: u64) -> f64 {
    expectation_of(cfg, p, kind.class(), now)
}

/// [`expectation`] of the class `c` ([`UV`] or [`CYAN`]).
pub fn expectation_of(cfg: &LightConfig, p: &crate::perception::Percept, c: usize, now: u64) -> f64 {
    let innate = if c == UV { cfg.uv_innate } else { cfg.cyan_innate };
    let offset = p.expect[c];
    if offset == 0.0 {
        return innate;
    }
    let tau = cfg.forget_s * f64::from(crate::TICK_HZ);
    let dt = now.saturating_sub(p.expect_tick) as f64;
    let decay = if tau > 0.0 { (-dt / tau).exp() } else { 0.0 };
    innate + offset * decay
}

/// Move the class `kind` has to the eye of `founder` ([`BeaconKind::class_seen`]) in the
/// record of `id` toward `outcome` (1 a sip, 0 a failed visit) by `learn_rate` (module
/// note); both offsets are brought up to now first.
fn learn(fauna: &mut Fauna, id: u64, founder: crate::Founder, kind: BeaconKind, outcome: f64) {
    let cfg = fauna.config.light;
    if !cfg.learn {
        return;
    }
    let c = kind.class_seen(&cfg, fauna.config.founder(founder).senses.uv);
    let now = fauna.tick;
    let p = fauna.percepts.entry(id).or_default();
    let now_e = [expectation_of(&cfg, p, UV, now), expectation_of(&cfg, p, CYAN, now)];
    let innate = [cfg.uv_innate, cfg.cyan_innate];
    let mut e = now_e;
    e[c] += cfg.learn_rate.clamp(0.0, 1.0) * (outcome - e[c]);
    p.expect = [e[0] - innate[0], e[1] - innate[1]];
    p.expect_tick = now;
}

/// Whether the body `id` of `founder` is naive about the lure's class to its eye now (cyan,
/// or UV for a UV eye when the lure mimics a flower; the readout cut [`NAIVE_FRACTION`]): a
/// body with no record is.
pub(crate) fn naive(fauna: &Fauna, id: u64, founder: crate::Founder) -> bool {
    let cfg = fauna.config.light;
    let c = BeaconKind::Lure.class_seen(&cfg, fauna.config.founder(founder).senses.uv);
    let innate = if c == UV { cfg.uv_innate } else { cfg.cyan_innate };
    fauna.percepts.get(&id).is_none_or(|p| expectation_of(&cfg, p, c, fauna.tick) >= NAIVE_FRACTION * innate)
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
    /// NECTAR-SCENT: per entry of `list`, the nectar a flower holds over a full bloomcrown
    /// head's (`flora.bloomcrown.nectar_cap`): the share of a full flower's scent it gives
    /// off. 0 for a glowcap; 1 for a hand-placed (test) beacon.
    scent: Vec<f64>,
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
        (self.list, self.scent) = Beacons::build(view, fv);
    }

    /// The flowers as smell sources: each with its scent share ([`Beacons::scent_shares`]).
    pub(crate) fn flowers_by_scent(&self) -> impl Iterator<Item = (&Beacon, f64)> + '_ {
        self.list.iter().zip(self.scent.iter().copied().chain(std::iter::repeat(1.0))).filter(|(b, _)| b.kind == BeaconKind::Flower)
    }

    /// Every flower holding nectar and every glowcap with a cap (module note).
    /// With each its scent share (NECTAR-SCENT): a flower's nectar over a full bloomcrown
    /// head's (`flora.bloomcrown.nectar_cap`; 1 when that is not positive), 0 for a glowcap.
    pub(crate) fn build(view: &VoxelView<'_>, fv: &FloraView<'_>) -> (Vec<Beacon>, Vec<f64>) {
        let c = view.config;
        let v = c.voxel_m;
        let mut out = Vec::new();
        let mut scent = Vec::new();
        let full = fv.config.species(Species::Bloomcrown).nectar_cap;
        let share = |n: f64| if full > 0.0 { (n / full).max(0.0) } else { 1.0 };
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
            scent.push(if kind == BeaconKind::Flower { share(st.nectar) } else { 0.0 });
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
            scent.push(share(f.nectar));
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
        (out, scent)
    }

    /// The static beacons as last built.
    pub fn list(&self) -> &[Beacon] {
        &self.list
    }

    /// A test's hand-placed beacons, held until the plant layer or the terrain changes.
    #[cfg(test)]
    pub(crate) fn fixed(&mut self, view: &VoxelView<'_>, fv: &FloraView<'_>, list: Vec<Beacon>) {
        self.stamp = Some((fv.tick, view.terrain_version));
        self.scent = vec![1.0; list.len()];
        self.list = list;
    }

    /// [`Beacons::fixed`] with each beacon's scent share given.
    #[cfg(test)]
    pub(crate) fn fixed_with_scent(&mut self, view: &VoxelView<'_>, fv: &FloraView<'_>, list: Vec<(Beacon, f64)>) {
        self.stamp = Some((fv.tick, view.terrain_version));
        (self.list, self.scent) = list.into_iter().unzip();
    }
}

/// **The lure's point** (module note): over a lanternjaw's standing surface by
/// [`LightConfig::lure_height_fraction`] of its height, ahead of its centre at the middle of
/// its strike capsule; `None` for a body whose table pays no lure.
pub fn lure_point(config: &FaunaConfig, a: &Animal, voxel_m: f64) -> Option<(f64, f64, f64)> {
    let h = config.founder(a.founder?).active_hunter()?;
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
            // S3b: only a body closing in, to reach the eye within `loom_tau_s` (its ground
            // velocity toward the eye; `dx, dz` run from the eye to it).
            let tau = fauna.config.light.loom_tau_s;
            let closing = || {
                let h = (dx * dx + dz * dz).sqrt();
                let (vx, vz) = (o.speed_m_s * o.pose.heading_rad.sin(), o.speed_m_s * o.pose.heading_rad.cos());
                let toward = if h > 0.0 { -(vx * dx + vz * dz) / h } else { 0.0 };
                toward > 0.0 && d < tau * toward
            };
            if d <= table.loom_m && (tau <= 0.0 || closing()) && loom.is_none_or(|(n, _)| d < n) {
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
        // S3b: a still beacon draws by level × its class's expectation; a class expected to
        // pay under `expect_min` is passed by. Moving lights repel by level.
        // BELLWING: the class and the level are the ones its eye sees (a lure is a flower to a
        // UV eye when it mimics one).
        let weight = |k: BeaconKind| if cfg.learn { expectation_of(&cfg, &record, k.class_seen(&cfg, table.uv), tick) } else { 1.0 };
        let mut seen: Vec<(f64, f64, f64, Beacon)> = Vec::new();
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
            let level = received(&cfg, b.contrast_seen(&cfg, daylight, table.uv), d);
            if level >= table.light_min && level > 0.0 {
                let score = if b.moving { level } else { level * weight(b.kind) };
                if !b.moving && cfg.learn && weight(b.kind) < cfg.expect_min {
                    continue;
                }
                seen.push((score, level, d, *b));
            }
        }
        seen.sort_by(|p, q| q.0.total_cmp(&p.0).then(p.2.total_cmp(&q.2)));
        let (mut still, mut moving): (Option<(f64, Beacon)>, Option<(f64, Beacon)>) = (None, None);
        for &(_, level, d, b) in seen.iter().take(MAX_LIGHT_RAYS) {
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

/// The open inspection of body `id`, at `(x, z)`, closed as failed when the body is farther
/// than `visit_m` + `visit_leave_m` from it (horizontally): marked for `mark_s`, counted in
/// `failed_visits` and `left_visits`, and its class taught 0.
fn left_visit(fauna: &mut Fauna, id: u64, x: f64, z: f64, founder: crate::Founder, ring: f64) {
    let cfg = fauna.config.light;
    let tick = fauna.tick;
    let Some(p) = fauna.percepts.get_mut(&id) else { return };
    if !p.visit.active {
        return;
    }
    let (dx, dz) = crate::pose::wrapped_delta(x, z, p.visit.x, p.visit.z, ring);
    let r = cfg.visit_m + cfg.visit_leave_m.max(0.0);
    if dx * dx + dz * dz <= r * r {
        return;
    }
    let (vx, vz, kind) = (p.visit.x, p.visit.z, p.visit.kind);
    p.mark(vx, vz, tick + cubarium_voxel::ticks_at_least_one(cfg.mark_s));
    p.visit = Visit::default();
    let k = usize::from(kind).min(BeaconKind::COUNT - 1);
    fauna.ledger.failed_visits[founder.index()][k] += 1;
    fauna.ledger.left_visits[founder.index()][k] += 1;
    learn(fauna, id, founder, BeaconKind::from_index(kind), 0.0);
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
    let budget = |fauna: &mut Fauna, m: LightMode| fauna.ledger.light_budget[founder.index()][m as usize] += 1;
    // Its controller period: one sample of a lead (LEADS-PAY).
    let period_s = fauna.config.manifest(founder).cadence_ticks().max(1) as f64 * cubarium_voxel::DT;
    // 1. A sip at its mouth stands: an inspection that ends in a sip is rewarded, and (S3b) a
    //    new sip bout teaches its class — the inspected beacon's, else UV (a flower).
    if asked.feed > 0.5 && asked.forward == 0.0 && asked.turn == 0.0 && asked_lift == 0.0 {
        budget(fauna, LightMode::Sip);
        let tick = fauna.tick;
        let gap = cubarium_voxel::ticks_at_least_one(fauna.config.light.visit_s);
        let p = fauna.percepts.entry(a.id).or_default();
        let bout = p.last_sip == 0 || tick.saturating_sub(p.last_sip) > gap;
        p.last_sip = tick;
        // FINDINGS-FIX: the sip credits the inspected beacon only at it (within `visit_m`);
        // elsewhere it is a flower's, and the far visit closes with the sip, unlearned.
        let at_visit = {
            let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, p.visit.x, p.visit.z, ring);
            dx * dx + dz * dz <= fauna.config.light.visit_m * fauna.config.light.visit_m
        };
        let visited = (p.visit.active && at_visit).then_some(p.visit.kind);
        if bout {
            let kind = visited.map_or(BeaconKind::Flower, BeaconKind::from_index);
            if let Some(k) = visited {
                fauna.ledger.sipped_visits[founder.index()][usize::from(k).min(BeaconKind::COUNT - 1)] += 1;
            }
            learn(fauna, a.id, founder, kind, 1.0);
        }
        rewarded(fauna, a.id);
        crate::step::rove::tasted(fauna, a.id);
        return None;
    }
    // FINDINGS-FIX: an open inspection it has flown away from (beyond `visit_m` +
    // `visit_leave_m`) is closed as failed: marked, counted, its class taught 0.
    left_visit(fauna, a.id, a.pose.x, a.pose.z, founder, ring);
    refresh_light(fauna, i, view, fv, daylight);
    let p = *fauna.percepts.get(&a.id)?;
    // 2. A looming shadow repels: away and up.
    if p.light.looming > 0.0 {
        fauna.ledger.loom_samples[founder.index()] += 1;
        budget(fauna, LightMode::Loom);
        rewarded(fauna, a.id);
        return Some((Actions { forward: 1.0, turn: away(p.light.loom_bearing), feed: 0.0 }, 1.0));
    }
    // 3. Fed, perched, dropping or blocked: its heuristic's.
    let fb = a.founder_state.feedback;
    let refused = fb.attempted_equivalent > 0.0 && fb.delivered_equivalent / fb.attempted_equivalent < fauna.config.teachers.motor_refused;
    if !a.eating || a.mobility.mode != MobilityMode::Fly || a.mobility.falling || refused {
        budget(fauna, LightMode::Held);
        return None;
    }
    let cfg = fauna.config.light;
    // 4. A moving light repels when it outshines the brightest still one.
    if p.light.repel_level > 0.0 && p.light.repel_level >= p.light.beacon_level {
        fauna.ledger.repel_samples[founder.index()] += 1;
        budget(fauna, LightMode::Repel);
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
            let kind = p.light.beacon_kind;
            if let Inspection::Failed = inspect(fauna, i, ring) {
                learn(fauna, a.id, founder, BeaconKind::from_index(kind), 0.0);
                budget(fauna, LightMode::inspect(kind));
                return None;
            }
            fauna.ledger.beacon_samples[founder.index()] += 1;
            budget(fauna, LightMode::inspect(kind));
            let mut act = crate::hunt::head(a.pose.heading_rad, want, 0.0, 1.0);
            act.forward = 0.0;
            return Some((act, lift));
        }
        // S3b: short of it and not moving for `visit_s` — wedged — it gives the beacon up;
        // LEADS-PAY: so it does when its leads have run past their budget without a sip.
        let spent = crate::step::rove::lead_spent(fauna, i, period_s);
        if spent || a.still_ticks >= cubarium_voxel::ticks_at_least_one(cfg.visit_s) {
            let until = fauna.tick + cubarium_voxel::ticks_at_least_one(cfg.mark_s);
            let rec = fauna.percepts.entry(a.id).or_default();
            rec.mark(p.light.beacon_x, p.light.beacon_z, until);
            rec.visit = Visit::default();
            rec.light.beacon = false;
            if spent {
                budget(fauna, LightMode::Search);
            } else {
                fauna.ledger.stalled_visits[founder.index()] += 1;
                budget(fauna, LightMode::Stall);
            }
            return None;
        }
        fauna.ledger.beacon_samples[founder.index()] += 1;
        budget(fauna, LightMode::approach(p.light.beacon_kind));
        let forward = if dh <= cfg.near_m { cfg.near_forward } else { 1.0 };
        return Some((crate::hunt::head(a.pose.heading_rad, want, forward, 1.0), lift));
    }
    // 6. Smell leads: up the nectar plume's gradient (NECTAR-SCENT: in the air, around the
    //    bloom, so up it in 3D leads to the flower head itself).
    if table.smell_min > 0.0 {
        let s = smell_nectar(fauna, &a, view, fv, table.smell_min);
        let gh = (s.gx * s.gx + s.gz * s.gz).sqrt();
        if s.level >= table.smell_min && (gh > 0.0 || s.gy != 0.0) && !scent_from_marked(fauna, &a, view, s.source) {
            // LEADS-PAY: a scent followed past the budget without a sip: its source is
            // marked, and the scent from it passed by.
            if crate::step::rove::lead_spent(fauna, i, period_s) {
                if let Some((x, _, z)) = s.source {
                    let until = fauna.tick + cubarium_voxel::ticks_at_least_one(cfg.mark_s);
                    fauna.percepts.entry(a.id).or_default().mark(x, z, until);
                }
                budget(fauna, LightMode::Search);
                return None;
            }
            fauna.ledger.scent_led_samples[founder.index()] += 1;
            budget(fauna, LightMode::Scent);
            // Along the gradient: its level part sets the heading and the forward effort, its
            // height part the lift, in proportion (full lift is the flight's
            // `lift_speed_fraction` of the cruise), the larger of the two at full effort.
            let lsf = phys.flight.map_or(1.0, |f| f.lift_speed_fraction).max(1e-9);
            let m = gh.max(s.gy.abs() / lsf);
            let want = if gh > 0.0 { s.gx.atan2(s.gz) } else { a.pose.heading_rad };
            let lift = (s.gy / (lsf * m)).clamp(-1.0, 1.0);
            return Some((crate::hunt::head(a.pose.heading_rad, want, (gh / m).clamp(0.0, 1.0), 0.0), lift));
        }
    }
    budget(fauna, LightMode::Search);
    None
}

/// FINDINGS-FIX-2: whether the nectar scent bellwing `a` smells comes from a place it has
/// marked "no reward here": the plume's main source (the flower or lit lure giving the most
/// of it, [`crate::scent::Plume::source`]) under one of its marks, by the light rule's own
/// mark test (`mark_radius_m`, for the mark's `mark_s`). Such a scent is passed by as the
/// marked beacon is.
fn scent_from_marked(fauna: &Fauna, a: &Animal, view: &VoxelView<'_>, source: Option<(f64, f64, f64)>) -> bool {
    let Some(record) = fauna.percepts.get(&a.id) else { return false };
    let Some((x, _, z)) = source else { return false };
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    record.marked(x, z, fauna.tick, fauna.config.light.mark_radius_m, ring)
}

/// **The nectar plume bellwing `a` smells** (NECTAR-SCENT, [`crate::scent::nectar_plume`]),
/// at its eye: every flower holding nectar and every lit lure but its own, by 3D distance
/// from the bloom; nothing under `floor`.
pub(crate) fn smell_nectar(fauna: &mut Fauna, a: &Animal, view: &VoxelView<'_>, fv: &FloraView<'_>, floor: f64) -> crate::scent::Plume {
    let v = view.config.voxel_m;
    let Some(eye) = eye_of(&fauna.config, a, v) else { return crate::scent::Plume::default() };
    fauna.beacons.refresh(view, fv);
    let sources = crate::scent::nectar_sources(fauna, view, Some(a.id));
    crate::scent::nectar_plume(view, &fauna.config, &sources, eye, a.id, floor)
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
        reset_glow_approach(fauna, i);
        return None;
    }
    let cfg = fauna.config.light;
    // Its chemistry leads; fed, it does not look.
    if !a.eating || fungal.is_some_and(|q| q >= cfg.glow_scent_gate) {
        reset_glow_approach(fauna, i);
        return None;
    }
    refresh_light(fauna, i, view, fv, daylight);
    let p = *fauna.percepts.get(&a.id)?;
    if !p.light.beacon {
        reset_glow_approach(fauna, i);
        return None;
    }
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, p.light.beacon_x, p.light.beacon_z, ring);
    let dh = (dx * dx + dz * dz).sqrt();
    let want = dx.atan2(dz);
    // Capgnawers do not patrol as hunters, so their existing hunt memory can hold
    // a bounded light approach. Circling counts as movement: still_ticks cannot
    // catch it. Allow one sensed-range traversal at cruise plus visit_s, then mark
    // the unrewarded place just as a stalled bellwing does. No new saved state.
    if founder == crate::Founder::Capgnawer {
        let m = &mut fauna.animals[i].hunt;
        let (px, pz) = crate::pose::wrapped_delta(m.patch.0, m.patch.1,
            p.light.beacon_x, p.light.beacon_z, ring);
        if m.mode != crate::HuntMode::Travel || px.hypot(pz) > cfg.mark_radius_m {
            m.mode = crate::HuntMode::Travel;
            m.samples = 0;
            m.patch = (p.light.beacon_x, p.light.beacon_z);
        }
        let manifest = fauna.config.manifest(founder);
        let travel_s = table.light_m / manifest.cruise_m_per_s.max(1e-9);
        let cadence_s = manifest.cadence_ticks() as f64 / f64::from(crate::TICK_HZ);
        let budget = ((travel_s + cfg.visit_s) / cadence_s).ceil().max(1.0);
        if f64::from(m.samples) >= budget {
            let rec = fauna.percepts.entry(a.id).or_default();
            rec.mark(p.light.beacon_x, p.light.beacon_z,
                fauna.tick + cubarium_voxel::ticks_at_least_one(cfg.mark_s));
            rec.visit = Visit::default();
            rec.light.beacon = false;
            fauna.ledger.stalled_visits[founder.index()] += 1;
            reset_glow_approach(fauna, i);
            return None;
        }
        m.samples = m.samples.saturating_add(1);
    }
    fauna.ledger.beacon_samples[founder.index()] += 1;
    // A walking body's centre need not reach the light: its footprint or turning
    // circle can hold it just outside visit_m. Start the existing no-reward timer
    // in the near band while it keeps approaching, so this cannot orbit forever.
    if dh <= cfg.near_m.max(cfg.visit_m) {
        if let Inspection::Failed = inspect(fauna, i, ring) {
            reset_glow_approach(fauna, i);
            return None;
        }
    }
    if dh <= cfg.visit_m {
        let mut act = crate::hunt::head(a.pose.heading_rad, want, 0.0, 1.0);
        act.forward = 0.0;
        return Some(act);
    }
    let forward = if dh <= cfg.near_m { cfg.near_forward } else { 1.0 };
    Some(crate::hunt::head(a.pose.heading_rad, want, forward, 1.0))
}

fn reset_glow_approach(fauna: &mut Fauna, i: usize) {
    if fauna.animals[i].founder == Some(crate::Founder::Capgnawer) {
        fauna.animals[i].hunt.mode = crate::HuntMode::Idle;
        fauna.animals[i].hunt.samples = 0;
    }
}

/// Whether the body `id` is drawn to a lure now — inspecting one, or with a lure its
/// brightest still beacon (the lure-kill readout).
pub(crate) fn drawn_to_lure(fauna: &Fauna, id: u64) -> bool {
    let lure = BeaconKind::Lure.index() as u8;
    fauna.percepts.get(&id).is_some_and(|p| (p.visit.active && p.visit.kind == lure) || (p.light.beacon && p.light.beacon_kind == lure))
}
