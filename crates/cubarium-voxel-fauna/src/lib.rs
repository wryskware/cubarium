//! Consumers on the voxel strip: the founder lineages — the littershredder, the
//! frondgrazer (the browser) and the lanternjaw — living as [`Animal`]s beside a
//! [`cubarium_voxel_flora::Flora`] on a [`cubarium_voxel::World`].
//!
//! This crate never draws, never reads the clock and never touches the world's stores: it
//! reads the world through [`cubarium_voxel::VoxelView`] (support faces, heights, water
//! depth, the `x` wrap) and changes the plant layer only through its bounded transfers
//! ([`cubarium_voxel_flora::Flora::take_foliage_in_layers`], `take_litter`,
//! `take_carrion` and [`cubarium_voxel_flora::Flora::deposit`]).
//!
//! Every quantity is paid out of a stock this layer holds, and every unit that leaves it
//! is a named boundary flow of [`FaunaLedger`], so the plant layer's `consumed_*` and
//! `deposited_*` and this layer's `eaten_*` and `deposited_*` are the same transfers seen
//! from two sides and the two ledgers close together.
//!
//! A body carries a [`Founder`] marker that selects its physiology
//! ([`FounderPhysiology`], looked up through [`effective_config`]) and its controller: a
//! [`Controller`] is sampled at the tick's controller stage ([`step`]'s order), its three
//! bounded actions are held, and the body resolves paid heading motion, contact and taste
//! receptors and local feeding through real transfers — [`body`]'s and [`controller`]'s
//! docs carry the rules. A founder with no controller installed rests.

#![forbid(unsafe_code)]

mod body;
mod controller;
mod encounter;
mod knobs;
mod manifest;
mod pose;
mod predation;
mod registry;
pub use registry::registry;
mod senses;
mod snapshot;
mod step;

#[cfg(test)]
mod cone_speed_tests;
#[cfg(test)]
mod hot_path_tests;
#[cfg(test)]
mod roster_tests;
#[cfg(test)]
mod snail_tests;

use cubarium_voxel::{VoxelView, World};
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraView, Site, Taken};
use serde::{Deserialize, Serialize};

pub use body::{
    Body, Counter, EggSite, Face, FounderPhysiology, Founding, FoundingSite, Hunter, StepLimits, birth_readiness,
    effective_config, has_headroom, step_voxels, wall_ascent, wall_descent,
};
pub use controller::{
    Actions, BlindForager, BrowserForager, Controller, ControllerFactory, FounderControllers,
    FounderFactories, LanternjawAmbush, Response, Scripted, heuristic, resolve_actions,
};
pub use predation::{
    BodyKind, Dormancy, Hit, Rest, Strike, StrikePhase, apply_hit, dodge_chance, dodges,
    egg_hatches, hatch_draw, is_prey_of, kind_of, moved_recently, tremor_of,
};
pub use cubarium_voxel::{DT, TICK_HZ};
pub use encounter::{
    HEADING_SAMPLES, RouteMap, RouteRule, SightMap, band_crown_layers, band_layers, crown_columns, crown_layer,
    crown_slab_m, eye_above_surface_m, eye_origin_m, foliage_stands_in_layers, layer_columns,
    mouth_columns_at, mouth_columns_from_face, mouth_crown_layers_at, ray_direction_deg,
    reachable_layers_of, route_components, standable_faces, surface_m, walkable_components,
};
pub use manifest::{
    ACTION_DEADBAND, Action, BIRTH_READINESS_RULE, BROWSER_RAY_PITCH_OFFSETS_DEG,
    BROWSER_RAY_YAW_OFFSETS_DEG, BROWSER_SECTOR_CENTRES_DEG, BROWSER_VISIBLE_CLASSES,
    Founder, cruise_body_lengths_per_s, HIDDEN, Manifest, Module, OCCLUSION_RULE, SCHEMA_VERSION,
    Transfer, Tunings, gru32_parameter_count,
};
pub use knobs::{CueConfig, FoundingConfig, LifeConfig, PredationConfig, TeacherConfig};
pub use pose::Pose;
pub use senses::{Senses, UPDATE_TICKS};
pub use snapshot::SCHEMA;

/// The browser's three cone sectors as `(foliage_fraction, foliage_proximity)` pairs.
/// This is a read-only diagnostic projection of the same ray result the founder
/// controller receives; it exists for autopsies and does not participate in stepping.
pub fn browser_cone_readings(
    view: &VoxelView<'_>,
    flora: &FloraView<'_>,
    fauna: &FaunaView<'_>,
    animal: &Animal,
) -> Option<[(f64, f64); 3]> {
    if animal.founder != Some(Founder::Browser) {
        return None;
    }
    let occupancy = senses::cone_occupancy(view, flora, fauna);
    let body = fauna.config.founder(Founder::Browser).body_at(animal.body);
    let reading = senses::cone_readings(
        view,
        &occupancy,
        animal.id,
        &animal.pose,
        animal.site.y,
        &Founder::Browser.manifest(),
        &body,
    );
    reading.valid.then_some(
        reading
            .sectors
            .map(|sector| (sector.foliage_fraction, sector.foliage_proximity)),
    )
}

/// The browser's whole `Cone(3, foliage/body)` module — the 19 values its observation
/// carries, in manifest order — from a **full** occupancy build. A read-only diagnostic:
/// the live controller stage builds a window around its due observers instead
/// (`senses::cone_occupancy_window`), and this is what that window must agree with.
pub fn browser_cone_observation(
    view: &VoxelView<'_>,
    flora: &FloraView<'_>,
    fauna: &FaunaView<'_>,
    animal: &Animal,
) -> Option<[f64; 19]> {
    if animal.founder != Some(Founder::Browser) {
        return None;
    }
    let occupancy = senses::cone_occupancy(view, flora, fauna);
    let body = fauna.config.founder(Founder::Browser).body_at(animal.body);
    let reading = senses::cone_readings(
        view,
        &occupancy,
        animal.id,
        &animal.pose,
        animal.site.y,
        &Founder::Browser.manifest(),
        &body,
    );
    let mut out = [0.0; 19];
    for (k, sec) in reading.sectors.iter().enumerate() {
        out[k * 6..k * 6 + 6].copy_from_slice(&[
            sec.clear,
            sec.all_proximity,
            sec.foliage_fraction,
            sec.foliage_proximity,
            sec.body_fraction,
            sec.body_proximity,
        ]);
    }
    out[18] = f64::from(reading.valid);
    Some(out)
}

/// What one cone ray struck first, at the resolution the policy is **not** given.
///
/// [`browser_cone_readings`] reports a sector's `foliage_fraction`; everything that is
/// not foliage and not a body collapses into one occluding class, so a cone that reads
/// no foliage is indistinguishable from a cone facing a slope, a puddle, a shower film,
/// a crown someone already stripped, or a litter pool. This is that same first hit named
/// (`design/handoffs/voxel-cone-autopsy-2026-09-21.md`, deliverable 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConeHit {
    /// Nothing within the cone's range, or the ray left the world.
    Clear,
    /// A solid voxel.
    Terrain,
    /// A cell holding free water.
    Water,
    /// A crown cell of a stand whose foliage is zero.
    StrippedCrown,
    /// A stand's trunk cell: structure, never food.
    Trunk,
    /// A crown cell with foliage standing in it — the only hit the policy reads as food.
    FoliageCrown,
    /// The cell over a ground site holding litter, carrion or dead wood.
    GroundPool,
    /// Another animal's body.
    Body,
}

impl ConeHit {
    /// Its name, for a CSV column.
    pub const fn name(self) -> &'static str {
        match self {
            ConeHit::Clear => "clear",
            ConeHit::Terrain => "terrain",
            ConeHit::Water => "water",
            ConeHit::StrippedCrown => "stripped",
            ConeHit::Trunk => "trunk",
            ConeHit::FoliageCrown => "foliage",
            ConeHit::GroundPool => "pool",
            ConeHit::Body => "body",
        }
    }

    /// Every variant, in the order a census row prints them.
    pub const ALL: [ConeHit; 8] = [
        ConeHit::Clear,
        ConeHit::Terrain,
        ConeHit::Water,
        ConeHit::StrippedCrown,
        ConeHit::Trunk,
        ConeHit::FoliageCrown,
        ConeHit::GroundPool,
        ConeHit::Body,
    ];

    /// Its index in [`ConeHit::ALL`].
    pub const fn index(self) -> usize {
        match self {
            ConeHit::Clear => 0,
            ConeHit::Terrain => 1,
            ConeHit::Water => 2,
            ConeHit::StrippedCrown => 3,
            ConeHit::Trunk => 4,
            ConeHit::FoliageCrown => 5,
            ConeHit::GroundPool => 6,
            ConeHit::Body => 7,
        }
    }
}

/// The one mapping from the ray march's fine class to the named hit. Shared by
/// [`browser_cone_census`] and the encounter query's [`SightMap`].
pub(crate) fn cone_hit_of(fine: senses::Fine) -> ConeHit {
    match fine {
        senses::Fine::Terrain => ConeHit::Terrain,
        senses::Fine::Water => ConeHit::Water,
        senses::Fine::StrippedCrown => ConeHit::StrippedCrown,
        senses::Fine::Trunk => ConeHit::Trunk,
        senses::Fine::FoliageCrown => ConeHit::FoliageCrown,
        senses::Fine::GroundPool => ConeHit::GroundPool,
        senses::Fine::Body => ConeHit::Body,
    }
}

/// One ray of the census: where it was aimed and what it found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConeRay {
    /// Sector index in manifest order, or [`ConeRay::PROBE`] for a caller's bearing.
    pub sector: usize,
    /// Yaw from the body's heading, degrees — sector centre plus the ray's own offset
    /// for a fan ray, the bearing itself for a probe.
    pub yaw_deg: f64,
    /// Pitch from horizontal, degrees.
    pub pitch_deg: f64,
    pub hit: ConeHit,
    /// Distance in metres, or [`f64::INFINITY`] for [`ConeHit::Clear`].
    pub distance_m: f64,
}

impl ConeRay {
    /// The `sector` of a ray the caller aimed rather than one of the fan's.
    pub const PROBE: usize = usize::MAX;
}

/// The browser's cone, ray by ray, at the fine resolution.
#[derive(Clone, Debug, PartialEq)]
pub struct ConeCensus {
    /// The eye, in metres: the body's column, one and a half **voxels** over its
    /// standing face.
    pub origin_m: (f64, f64, f64),
    pub heading_rad: f64,
    pub range_m: f64,
    /// The manifest's own fan, in `cone_readings`' accumulation order: sector, then yaw
    /// offset, then pitch offset.
    pub rays: Vec<ConeRay>,
    /// One ray per bearing the caller asked for, in the order asked.
    pub probes: Vec<ConeRay>,
}

impl ConeCensus {
    /// How many fan rays of one sector struck each class, indexed by
    /// [`ConeHit::index`].
    pub fn counts(&self, sector: usize) -> [u32; ConeHit::ALL.len()] {
        let mut out = [0u32; ConeHit::ALL.len()];
        for ray in self.rays.iter().filter(|r| r.sector == sector) {
            out[ray.hit.index()] += 1;
        }
        out
    }

    /// The mean distance of this sector's [`ConeHit::FoliageCrown`] hits, or `None` when
    /// it has none.
    pub fn nearest_foliage_mean_m(&self, sector: usize) -> Option<f64> {
        let hits: Vec<f64> = self
            .rays
            .iter()
            .filter(|r| r.sector == sector && r.hit == ConeHit::FoliageCrown)
            .map(|r| r.distance_m)
            .collect();
        (!hits.is_empty()).then(|| hits.iter().sum::<f64>() / hits.len() as f64)
    }
}

/// Run the browser's **exact** cone from its exact eye and name every first hit.
///
/// It is the same occupancy map, the same origin, the same ray directions and the same
/// cell-exact traversal the controller's own observation uses — the geometry
/// lives in one place (`senses::cone_origin`, `senses::ray_direction`,
/// `senses::ray_first_hit`) and both callers read it — so a census row explains the
/// reading the policy was given rather than a second opinion about it. What it adds is
/// only resolution: `senses::Fine::coarse` is the one mapping back, and a test asserts
/// the census reproduces [`browser_cone_readings`] exactly.
///
/// `probes` are extra bearings in radians, `(yaw_from_heading, pitch)`, for asking
/// "is the crown over there blocked, and by what" — they are marched with the same ray
/// and returned separately, and they change nothing.
///
/// `None` if the animal is not a browser founder or its eye is invalid where it stands.
pub fn browser_cone_census(
    view: &VoxelView<'_>,
    flora: &FloraView<'_>,
    fauna: &FaunaView<'_>,
    animal: &Animal,
    probes: &[(f64, f64)],
) -> Option<ConeCensus> {
    if animal.founder != Some(Founder::Browser) {
        return None;
    }
    let manifest = Founder::Browser.manifest();
    if !senses::cone_valid(view, &animal.pose, animal.site.y, &manifest) {
        return None;
    }
    let occupancy = senses::cone_occupancy(view, flora, fauna);
    let geometry = fauna.config.founder(Founder::Browser).body_at(animal.body);
    let origin = senses::cone_origin(view, &animal.pose, animal.site.y, &geometry);
    let heading = animal.pose.heading_rad;
    let range = manifest.cone_range_m;
    let march = |sector: usize, yaw_deg: f64, pitch_deg: f64, dir: (f64, f64, f64)| {
        let (hit, distance_m) =
            match senses::ray_first_hit(view, &occupancy, animal.id, origin, dir, range) {
                None => (ConeHit::Clear, f64::INFINITY),
                Some((d, fine)) => (cone_hit_of(fine), d),
            };
        ConeRay {
            sector,
            yaw_deg,
            pitch_deg,
            hit,
            distance_m,
        }
    };
    let mut rays = Vec::with_capacity(
        manifest.sector_centres_deg.len()
            * manifest.ray_yaw_offsets_deg.len()
            * manifest.ray_pitch_offsets_deg.len(),
    );
    for (si, &centre_deg) in manifest.sector_centres_deg.iter().enumerate() {
        for &oyaw in manifest.ray_yaw_offsets_deg {
            for &opitch in manifest.ray_pitch_offsets_deg {
                let dir = senses::ray_direction(heading, centre_deg, oyaw, opitch);
                rays.push(march(si, centre_deg + oyaw, opitch, dir));
            }
        }
    }
    let probes = probes
        .iter()
        .map(|&(yaw, pitch)| {
            let dir = senses::ray_direction(heading, yaw.to_degrees(), 0.0, pitch.to_degrees());
            march(ConeRay::PROBE, yaw.to_degrees(), pitch.to_degrees(), dir)
        })
        .collect();
    Some(ConeCensus {
        origin_m: origin,
        heading_rad: heading,
        range_m: range,
        rays,
        probes,
    })
}

/// The stand selected by the browser's actual mouth geometry, if any. Read-only
/// diagnostic access for explaining a bite or a starvation event.
pub fn browser_mouth_foliage(
    view: &VoxelView<'_>,
    flora: &FloraView<'_>,
    config: &FaunaConfig,
    animal: &Animal,
) -> Option<(Site, f64)> {
    let geometry = body_of(config, animal)?;
    if animal.founder != Some(Founder::Browser) {
        return None;
    }
    let cols = body::mouth_columns(view, &animal.pose, &geometry);
    body::mouth_foliage_stand(flora, view, &cols, animal.site.y, &geometry, Diet::only(Food::Foliage))
}

/// The physical geometry of one animal right now, from its lineage's physiology and its
/// structure ([`FounderPhysiology::body_at`]). `None` for a body with no founder marker,
/// which carries no dimensions.
pub fn body_of(config: &FaunaConfig, animal: &Animal) -> Option<Body> {
    Some(config.founder(animal.founder?).body_at(animal.body))
}

/// Every stand whose crown touches the browser's actual mouth probe columns.
/// Read-only diagnostic access for distinguishing a nearby crown from edible contact.
pub fn browser_mouth_candidates(
    view: &VoxelView<'_>,
    flora: &FloraView<'_>,
    config: &FaunaConfig,
    animal: &Animal,
) -> Option<Vec<(Site, f64)>> {
    if animal.founder != Some(Founder::Browser) {
        return None;
    }
    let geometry = body_of(config, animal)?;
    let cols = body::mouth_columns(view, &animal.pose, &geometry);
    Some(body::mouth_foliage_stands(
        flora,
        view,
        &cols,
        animal.site.y,
        &geometry,
        Diet::only(Food::Foliage),
    ))
}

/// The consumers of the voxel ecology. One, so far: see
/// [`SpeciesConfig::frondgrazer`] for the sentence of ecology its numbers encode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Species {
    /// The paid ground browser of the grazed meadow: crops reachable foliage, walks to
    /// better food, drowns in standing water it cannot wade.
    Frondgrazer,
}

impl Species {
    pub const ALL: [Species; 1] = [Species::Frondgrazer];
    pub const COUNT: usize = Species::ALL.len();

    /// Index into a per-species array. Stable, like the flora's.
    pub fn index(self) -> usize {
        match self {
            Species::Frondgrazer => 0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Species::Frondgrazer => "frondgrazer",
        }
    }

    /// The name a command line or a stdin line gives, or `None`.
    pub fn parse(s: &str) -> Option<Species> {
        Species::ALL
            .into_iter()
            .find(|sp| sp.name().eq_ignore_ascii_case(s))
    }
}

/// What an animal did on the tick just stepped. A record of the act, not a mode it is put
/// into: the next tick reads its stocks and its surroundings again from scratch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    /// It had at least one bite within reach and took it.
    Cropping,
    /// It had nothing in reach, something in sense, and a face strictly closer to it that
    /// it could climb and wade. Also the state of an animal that wanted to walk and was
    /// not due a step this tick.
    Walking,
    /// Nothing in reach and nothing in sense, or nowhere closer it could stand.
    Resting,
    /// On a terrain face, going up or down it ([`Mobility::climb`]).
    Climbing,
    /// The rest state: its phase is not now and it is not hungry, so it stands where
    /// it is, its controller is not consulted, it does not move or feed, and it pays
    /// [`FounderPhysiology::sleep_upkeep`] × its basal upkeep. Kept general, a rest with an
    /// upkeep factor and not a thing tied to night: a predator's long rest is the same state.
    Asleep,
}

/// When a lineage is awake, read against the world's `life_daylight`. With the day
/// clock off everyone is awake, so the clock's own switch reaches here as `clock_on`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Activity {
    /// Awake whatever the hour.
    Always,
    /// Awake by day: daylight above [`LifeConfig::day_line`].
    Diurnal,
    /// Awake by night: daylight below [`LifeConfig::day_line`].
    Nocturnal,
    /// Awake in the twilight of either end of the day: daylight strictly inside
    /// [`LifeConfig::twilight_lo`]..[`LifeConfig::twilight_hi`].
    Crepuscular,
}

impl Activity {
    /// Whether this rhythm is awake at `daylight` (0 night, 1 full day), the day's lines
    /// being `life`'s. With the clock off (`clock_on` false) every rhythm is awake.
    pub fn awake(self, clock_on: bool, daylight: f64, life: &LifeConfig) -> bool {
        if !clock_on {
            return true;
        }
        match self {
            Activity::Always => true,
            Activity::Diurnal => daylight > life.day_line,
            Activity::Nocturnal => daylight < life.day_line,
            Activity::Crepuscular => daylight > life.twilight_lo && daylight < life.twilight_hi,
        }
    }
}

/// A body's movement state that outlives a tick
/// (`design/handoffs/voxel-mobility-2026-09-23.md`): the face it is on, if any, and how
/// long it has been under water deeper than its height.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Mobility {
    /// The wall it is climbing, or `None` on the ground.
    pub climb: Option<WallClimb>,
    /// Consecutive ticks under water deeper than its drown depth. It drowns when this
    /// reaches [`FounderPhysiology::drown_ticks`]; any tick not under resets it.
    pub submerged_ticks: u64,
}

/// A body **on a terrain face**: a shredder going up a
/// wall from the face it stood on to the top of the neighbouring column, or down a cliff
/// from the edge it stood on to the foot of the neighbouring column.
///
/// While it climbs, the animal's `site` stays the face it started from — a real support
/// face, so nothing that asks where a body stands is handed a hole — and `progress_m` is
/// how far along the face its feet have gone. Its pose stays beside the face, heading into
/// it; it does not feed. At `rise_m` it stands on `to`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WallClimb {
    /// Up a wall (`true`) or down a cliff.
    pub ascending: bool,
    /// The face it started from, and the one it will stand on at the end.
    pub from: Site,
    pub to: Site,
    /// The unit step, in `(x, z)`, from `from`'s column toward the column of `to`.
    pub dir: (i8, i8),
    /// Metres along the face so far, and the whole height of the face.
    pub progress_m: f64,
    pub rise_m: f64,
}

impl WallClimb {
    /// The feet's height over `from`'s standing surface, signed: up the wall on the way
    /// up, down the cliff on the way down.
    pub fn offset_m(&self) -> f64 {
        if self.ascending {
            self.progress_m
        } else {
            -self.progress_m
        }
    }
}

/// One animal: an identity, the support face it stands on, and the three currencies it
/// holds.
///
/// `body` is structure and `reserve` is what maintenance is paid out of; `mineral` and
/// `energy` are the stocks that came in with its food. Energy is a **stock and not
/// `energy_density · organic`** (the flora's dead pools are kept the same way and for the
/// same reason): a bite carries the energy the plant layer handed over at *its* density,
/// so holding that number is what makes the energy residual close for any food, instead of
/// requiring this species' own density to be no greater than its food's.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Animal {
    /// This animal's identity, from [`FaunaLedger::births`]: every animal this layer has
    /// ever created has its own, introduced ones included, and it never changes or
    /// repeats.
    pub id: u64,
    pub species: Species,
    /// The support face it stands on. Its body is drawn in the void above this.
    pub site: Site,
    /// The continuous place and heading on that support layer. The pose is
    /// authoritative and `site` follows the pose's column at the standing layer.
    pub pose: Pose,
    /// The lineage: which physiology the body runs and whose manifest its controller is
    /// authored against. A body with `None` rests.
    pub founder: Option<Founder>,
    /// The founder's held actions and prior-interval feedback.
    pub founder_state: FounderState,
    /// Where this body is in its own reproductive cycle: the surplus it has held, the
    /// refractory left after its last offspring, and the escrow it is filling. On the
    /// animal and not in [`FounderState`], which is the controller's interval state.
    pub reproduction: ReproductionState,
    /// Organic structure. Below `body_min` it is dead.
    pub body: f64,
    /// Organic matter maintenance is paid from before the body is.
    pub reserve: f64,
    /// The mineral in its tissue: an inventory, moved by the same fraction rule the plant
    /// layer uses for every outflow, never created and never respired.
    pub mineral: f64,
    /// The energy in its tissue, as its food handed it over.
    pub energy: f64,
    /// Waste held until the next void: the undigested share of every bite, the mineral a
    /// bite brought beyond what its tissue needs, and the mineral shed with the organic
    /// matter respiration burns. Emptied onto the ground as litter every
    /// `GUT_VOID_S` (30 s), and into the corpse at death. Counted in the stored totals.
    pub gut: Taken,
    /// Ticks since it was born or introduced. Also its step phase: an animal steps on the
    /// ticks where `age_ticks` is a multiple of its species' step period, so two animals
    /// born on different ticks do not step in lockstep.
    pub age_ticks: u64,
    /// The age, in ticks, at which this body departs [`Departure::Aged`]: drawn once, at
    /// birth or introduction ([`draw_lifespan_ticks`]). Zero is no lifespan.
    #[serde(default)]
    pub lifespan_ticks: u64,
    pub state: State,
    /// On a wall, and how long under water.
    pub mobility: Mobility,
    /// **The wound stock** in `[0, 1)` (`design/predation-and-flight-2026-09-28.md`
    /// §2.4): each hit adds to it, at 1 the body dies [`Departure::Killed`]. It slows the
    /// body (speed cap × `1 − wound_slow · wound`) and heals at [`PredationConfig::wound_heal_per_s`], paying
    /// [`PredationConfig::heal_cost`] out of the reserve; a starving body does not heal.
    pub wound: f64,
    /// A strike in progress: a carnivore only (§2.3).
    pub strike: Option<Strike>,
    /// Why it rests: `Some` exactly while `state` is [`State::Asleep`] (§2.8).
    pub rest: Option<Rest>,
    /// Ticks since it last moved faster than 0.1 of its own lengths a second (§2.2): a
    /// threat is still, and reads as other beyond 0.5 m, once this is past 2 s.
    pub still_ticks: u64,
    /// Its ground speed on the last tick, metres a second: what tremor reads (§2.2).
    pub speed_m_s: f64,
    /// Ticks it is kept awake after a hit woke it ([`PredationConfig::alert_s`]).
    pub alert_ticks: u32,
    /// **Riding a host** (parasite hatchlings,
    /// `design/handoffs/voxel-predation-parasite-2026-10-01.md`): `Some` while this
    /// lanternjaw hatchling is clamped onto a living non-lanternjaw host. A rider has no
    /// motion, controller, sensing or strikes of its own; its position is its host's.
    /// Saved as the host's id.
    pub attached: Option<Attachment>,
}

/// A rider's hold on its host (parasite hatchlings): the host's [`Animal::id`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub host: u64,
}

/// A rider lets go of `host` (parasite hatchlings §4, §5): it stands where the host
/// stands, a free juvenile from this tick, and its controller — made at hatching, never
/// consulted while it rode — starts from an empty interval and a rest.
pub(crate) fn release(rider: &mut Animal, host: &Animal) {
    rider.attached = None;
    rider.site = host.site;
    rider.pose = host.pose;
    rider.founder_state.held = controller::Actions::REST;
    rider.founder_state.feedback = IntervalFeedback::default();
}

impl Animal {
    /// The feet's height over the standing surface of `site`, metres: zero on the ground,
    /// the climb's signed progress on a wall.
    pub fn wall_offset_m(&self) -> f64 {
        self.mobility.climb.map_or(0.0, |c| c.offset_m())
    }

    /// The layer this body's receptors read from and its body is drawn over: `site.y` on
    /// the ground, and on a wall the layer its feet have reached, to the nearest voxel.
    pub fn sense_layer(&self, voxel_m: f64) -> u32 {
        let offset = self.wall_offset_m();
        if offset == 0.0 || !(voxel_m > 0.0) {
            return self.site.y;
        }
        (f64::from(self.site.y) + offset / voxel_m).round().max(0.0) as u32
    }

    /// Organic matter in the animal: structure plus reserve. What a corpse deposits —
    /// a gestation is always resolved before a body dies, so its escrow is back in the
    /// reserve by the time this is read for a corpse.
    pub fn organic(&self) -> f64 {
        self.body + self.reserve
    }

    /// Organic matter this body **holds**, the escrowed offspring included: what the
    /// layer's stored total counts, and what leaves with a body an
    /// [`Command::Remove`] or a collapsing floor takes out of the world.
    ///
    /// The escrow is not the parent's to spend — no rule reads it as reserve, and
    /// maintenance cannot touch it — but it is still in the parent until it is born,
    /// so a conservation check that ignored it would see matter vanish at the first
    /// instalment.
    pub fn stored_organic(&self) -> f64 {
        self.organic() + self.gut.organic + self.reproduction.escrow.map_or(0.0, |e| e.organic)
    }

    /// Mineral in this body, its gut and the escrowed offspring's included.
    pub fn stored_mineral(&self) -> f64 {
        self.mineral + self.gut.mineral + self.reproduction.escrow.map_or(0.0, |e| e.mineral)
    }

    /// Energy in this body, its gut and the escrowed offspring's included.
    pub fn stored_energy(&self) -> f64 {
        self.energy + self.gut.energy + self.reproduction.escrow.map_or(0.0, |e| e.energy)
    }
}

/// Where a body is in its own reproductive cycle ([`step`]'s step 7).
///
/// Three pieces of state, one per clause of the rule: how long the surplus has stood,
/// how long the body must wait after its last offspring, and the package it is
/// currently paying for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReproductionState {
    /// Ticks within the last hold the body has been **eligible** — structure at
    /// `birth_body` and a reserve at or above the offspring package plus `surplus_floor`
    /// ([`SurplusWindow::count_for`]). A body may begin once this reaches
    /// `surplus_share` of the hold and it is eligible now: a sustained surplus, which a
    /// short foraging trip does not reset.
    pub surplus_ticks: u64,
    /// The history `surplus_ticks` is read from.
    #[serde(default)]
    pub window: SurplusWindow,
    /// Whether the body was eligible on its last tick.
    #[serde(default)]
    pub eligible_now: bool,
    /// Ticks of refractory still owed after the last birth or laying. Nothing about
    /// eligibility advances while this is above zero.
    pub refractory_ticks: u64,
    /// The offspring package being filled, for a gestating body. `None` is a body that
    /// is not gestating; an egg-laying body never has one.
    pub escrow: Option<Escrow>,
    /// Organic matter this body has passed to its offspring, cumulative: each birth's
    /// whole package and each clutch it laid. What the brain reward counts as given, not
    /// lost (`design/handoffs/cotrain-stage0-2026-10-01.md` §1).
    #[serde(default)]
    pub given: f64,
}

/// An offspring package the parent has already paid for and cannot spend.
///
/// The three currencies leave the parent's reserve in equal instalments over
/// `gestation_s` and sit here until the birth builds a body out of them. The escrow
/// pays no upkeep of its own and the parent's maintenance is unchanged: it is not a
/// second animal yet, and it is not the parent's reserve any more.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Escrow {
    pub organic: f64,
    pub mineral: f64,
    pub energy: f64,
    /// Instalments paid so far. The birth is at `gestation_ticks`.
    pub ticks: u64,
}

/// A clutch of eggs: a **stationary paid package in the world**, on the support face
/// the parent laid it on.
///
/// It is not an animal — it has no body, no controller, no pose and no upkeep — and it
/// is not a plant-layer pool either: the material is still the animal layer's, and the
/// layer's stored totals count it, until the eggs hatch into bodies or the site takes
/// them. Nothing eats a clutch yet; the record carries its site and its three
/// currencies so that a consumer can be added later without changing what an egg *is*.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clutch {
    /// The support face the eggs lie on.
    pub site: Site,
    /// Eggs still in the clutch. They hatch together.
    pub count: u32,
    /// The clutch's organic matter, all eggs together. One egg is `organic / count`.
    pub organic: f64,
    pub mineral: f64,
    pub energy: f64,
    /// The tick the clutch was laid; incubation is counted from it.
    pub laid_tick: u64,
    /// The lineage that laid it, which is the lineage its hatchlings carry and the
    /// factory their controllers come from.
    pub lineage: Founder,
    /// `Some` for a **dormant** egg (§2.13): a lanternjaw's clutch is laid or buried as
    /// one `Clutch` per egg, each with its own footfall threshold and viability; it
    /// hatches by [`egg_hatches`], not by incubation, pays [`PredationConfig::egg_upkeep_per_s`], and dies
    /// to the litter when its viability runs out.
    pub dormancy: Option<Dormancy>,
}

impl Clutch {
    /// What one egg holds: the clutch split `count` ways.
    pub fn egg(&self) -> Taken {
        let n = f64::from(self.count.max(1));
        Taken {
            organic: self.organic / n,
            mineral: self.mineral / n,
            energy: self.energy / n,
        }
    }
}

/// How a lineage puts its offspring into the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BirthMode {
    /// Live birth out of a gestation escrow: one offspring at a time, at the parent's
    /// pose, at term.
    Gestation,
    /// A clutch of eggs laid on the litter the parent lives in, hatching where they lie.
    Eggs,
}

/// A lineage's reproduction: **every number here is an untuned placeholder**
/// (`design/backlog.md` §1, `design/handoffs/voxel-reproduction-2026-09-21.md`), chosen
/// to encode "births after a sustained surplus, one at a time, with an interval" at the
/// scale the frozen physiology already sets. Nothing measured any of them and there was
/// no tuning loop.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reproduction {
    pub mode: BirthMode,
    /// Reserve a body must hold **above** the offspring package's price before it counts
    /// as being in surplus.
    pub surplus_floor: f64,
    /// The window a surplus is judged over, seconds: a body may begin once it has been in
    /// surplus for [`Reproduction::surplus_share`] of the last `surplus_hold_s` (and is in
    /// surplus now).
    pub surplus_hold_s: f64,
    /// Seconds a gestation takes, over which the escrow is filled in equal instalments.
    /// [`BirthMode::Eggs`] does not read it.
    pub gestation_s: f64,
    /// The share of an interrupted gestation's escrowed **organic matter** that is
    /// respired; the rest returns to the parent, and the mineral all returns, because
    /// respiration in this layer never takes mineral.
    pub gestation_loss_fraction: f64,
    /// Seconds of refractory after a birth or a laying, before a surplus can start
    /// accumulating again.
    pub birth_interval_s: f64,
    /// Eggs in one clutch. [`BirthMode::Gestation`] does not read it.
    pub clutch_size: u32,
    /// Organic matter in **one** egg, which must be at least the lineage's `body_min`:
    /// a hatchling is a `body_min` juvenile with what is left over as its reserve.
    pub egg_organic: f64,
    /// Seconds a clutch incubates before it hatches.
    pub incubation_s: f64,
    /// **Dormant eggs** (§2.13): each egg is buried as its own clutch on the face the
    /// parent stands on — soil or litter, no litter required — and waits for its
    /// hatching trigger instead of an incubation.
    pub dormant: bool,
    /// The share of the last `surplus_hold_s` a body must have spent in surplus: 0.8
    /// shipped, so a foraging trip of up to a fifth of the hold is not a reset. 1.0 is an
    /// unbroken hold.
    #[serde(default = "default_surplus_share")]
    pub surplus_share: f64,
    /// An egg-layer that lays in litter (not [`Reproduction::dormant`]) needs the litter on
    /// its face to exceed this, organic units: 0, "any litter at all".
    #[serde(default)]
    pub litter_min: f64,
}

fn default_surplus_share() -> f64 {
    SURPLUS_SHARE
}

/// The shipped [`Reproduction::surplus_share`].
pub const SURPLUS_SHARE: f64 = 0.8;

/// How many buckets a [`SurplusWindow`] keeps (serde's fixed arrays stop at 32).
pub const SURPLUS_BUCKETS: usize = 30;

/// **Where a body's surplus has been over the last hold**: a ring of
/// [`SURPLUS_BUCKETS`] buckets of `ceil(hold / 30)` ticks, each the number of ticks in it
/// the body was in surplus, and the bucket being filled. Exact per tick for a hold of up
/// to 30 ticks; otherwise the window reads the last `ceil(hold / bucket)` buckets
/// including the current one, within one bucket (1/30 of the hold) of the true last hold.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SurplusWindow {
    buckets: [u32; SURPLUS_BUCKETS],
    /// Completed buckets so far, up to [`SURPLUS_BUCKETS`].
    filled: u32,
    /// The next bucket to complete, and what the current one holds.
    pos: u32,
    cur_eligible: u32,
    cur_ticks: u32,
}

impl SurplusWindow {
    fn bucket_ticks(hold: u64) -> u64 {
        hold.div_ceil(SURPLUS_BUCKETS as u64).max(1)
    }

    fn span_buckets(hold: u64) -> usize {
        (hold.div_ceil(SurplusWindow::bucket_ticks(hold)) as usize).clamp(1, SURPLUS_BUCKETS)
    }

    /// One tick of the body's life: in surplus or not.
    pub fn tick(&mut self, eligible: bool, hold: u64) {
        self.cur_ticks += 1;
        if eligible {
            self.cur_eligible += 1;
        }
        if u64::from(self.cur_ticks) >= SurplusWindow::bucket_ticks(hold) {
            self.buckets[self.pos as usize] = self.cur_eligible;
            self.pos = (self.pos + 1) % SURPLUS_BUCKETS as u32;
            self.filled = (self.filled + 1).min(SURPLUS_BUCKETS as u32);
            self.cur_ticks = 0;
            self.cur_eligible = 0;
        }
    }

    /// Ticks in surplus within the last hold (the current bucket and the completed ones
    /// before it).
    pub fn count_for(&self, hold: u64) -> u64 {
        let span = SurplusWindow::span_buckets(hold);
        // A partly filled current bucket counts as one of the span.
        let completed = if self.cur_ticks > 0 { span - 1 } else { span };
        let n = completed.min(self.filled as usize);
        let mut sum = u64::from(self.cur_eligible);
        for k in 1..=n {
            let i = (self.pos as usize + SURPLUS_BUCKETS - k) % SURPLUS_BUCKETS;
            sum += u64::from(self.buckets[i]);
        }
        sum
    }

    /// [`SurplusWindow::count_for`] over a full 30-bucket window.
    pub fn count(&self) -> u64 {
        self.count_for(u64::MAX / 2)
    }

    /// Whether a body in surplus now (`eligible`) has been in surplus for `share` of the
    /// last `hold` ticks.
    pub fn met(&self, eligible: bool, hold: u64, share: f64) -> bool {
        eligible && self.count_for(hold) as f64 >= (share * hold as f64).ceil()
    }

    /// Forget the history (a birth, a laying, a refractory tick).
    pub fn reset(&mut self) {
        *self = SurplusWindow::default();
    }
}

impl Reproduction {
    /// The frondgrazer's gestation placeholders, and the table a body with **no**
    /// lineage runs.
    ///
    /// Against the frondgrazer's frozen physiology (`birth_cost` 0.01, a reserve
    /// ceiling of 0.025 at `body_max`): eligibility at 0.015 is 60 % of a full reserve,
    /// so it is a real surplus and not a full tank; the escrow fills at
    /// `0.01 / 180 s` = 5.6e-5 /s against a basal upkeep of 5e-5 /s at `body_max`, so
    /// gestating roughly doubles what the parent is paying out.
    ///
    /// **On plant time**: `surplus_hold_s` 900 and
    /// `birth_interval_s` 1,800 put hold + gestation + interval = 2,880 s (48 min)
    /// between one parent's offspring — the brief's "at least ~30 min apart" — and, with
    /// `growth_max_per_s`, a newborn's first birth an hour after its own. A founder that
    /// arrives full gives birth at ~18 min and not again before ~66 min, so a meadow of
    /// founders is at most doubled by the hour.
    pub const LIVE_BIRTH_PLACEHOLDER: Reproduction = Reproduction {
        mode: BirthMode::Gestation,
        surplus_floor: 0.005,
        surplus_hold_s: 900.0,
        gestation_s: 180.0,
        gestation_loss_fraction: 0.25,
        birth_interval_s: 1800.0,
        clutch_size: 0,
        egg_organic: 0.0,
        incubation_s: 0.0,
        dormant: false,
        surplus_share: SURPLUS_SHARE,
        litter_min: 0.0,
    };

    /// The littershredder's egg placeholders.
    ///
    /// `clutch_size` is **1** and that is a consequence, not a preference: the blind
    /// founder's frozen physiology puts `body_min` at 0.003125 and caps its reserve at
    /// `reserve_cap · body_max` = 0.00625, so a viable egg costs at least a body_min
    /// plus something to hatch with and two of them cannot be paid for out of one
    /// reserve at all. Raising it needs the blind body's reserve ceiling or its
    /// `body_min` revisited, which is a physiology decision and not this round's
    /// (`design/backlog.md` §1). The rule itself is written for any count.
    ///
    /// **On plant time**:
    /// `surplus_hold_s` 900 and `birth_interval_s` 1,800 put 2,700 s (45 min) between one
    /// parent's clutches, and with the shredder's `growth_max_per_s` a hatchling lays its
    /// first clutch no sooner than an hour after it hatched. A founder that arrives full
    /// lays at ~15 min (hatching at ~20) and not again before ~60 min.
    pub const EGGS_PLACEHOLDER: Reproduction = Reproduction {
        mode: BirthMode::Eggs,
        surplus_floor: 0.000_625,
        surplus_hold_s: 900.0,
        gestation_s: 0.0,
        gestation_loss_fraction: 0.25,
        birth_interval_s: 1800.0,
        clutch_size: 1,
        egg_organic: 0.004,
        incubation_s: 300.0,
        dormant: false,
        surplus_share: SURPLUS_SHARE,
        litter_min: 0.0,
    };

    /// The lanternjaw's **dormant egg bank** (§2.13): a clutch of 3 buried eggs laid
    /// after a sustained surplus, each `egg_organic` 0.005 — a `body_min` hatchling
    /// (0.0034) and a small reserve (0.0016), "each egg's organic matter plus a small
    /// reserve". The floor 0.0125 puts the surplus at 0.0275, 0.79 of a full reserve: a
    /// lanternjaw lays after a real meal, not on an ordinary one. Hold and interval are
    /// the other egg-layer's.
    pub const DORMANT_EGGS_PLACEHOLDER: Reproduction = Reproduction {
        mode: BirthMode::Eggs,
        surplus_floor: 0.0125,
        surplus_hold_s: 900.0,
        gestation_s: 0.0,
        gestation_loss_fraction: 0.25,
        birth_interval_s: 1800.0,
        clutch_size: 3,
        egg_organic: 0.005,
        incubation_s: 0.0,
        dormant: true,
        surplus_share: SURPLUS_SHARE,
        litter_min: 0.0,
    };

    /// What one offspring package costs the parent's reserve: a gestation's `birth_cost`
    /// or a whole clutch.
    pub fn package_cost(&self, sc: &SpeciesConfig) -> f64 {
        match self.mode {
            BirthMode::Gestation => sc.birth_cost,
            BirthMode::Eggs => f64::from(self.clutch_size) * self.egg_organic,
        }
    }

    /// The reserve a body has to hold to be in surplus: the package plus the floor.
    pub fn surplus_reserve(&self, sc: &SpeciesConfig) -> f64 {
        self.package_cost(sc) + self.surplus_floor
    }


    pub fn hold_ticks(&self) -> u64 {
        cubarium_voxel::ticks_at_least_one(self.surplus_hold_s)
    }

    /// The ticks in surplus within the last hold that let a body begin:
    /// `ceil(surplus_share × hold)`.
    pub fn hold_required(&self) -> u64 {
        (self.surplus_share.clamp(0.0, 1.0) * self.hold_ticks() as f64).ceil() as u64
    }

    /// Whether a body's surplus has stood long enough to begin: eligible now, and in
    /// surplus for [`Reproduction::surplus_share`] of the last hold.
    pub fn hold_met(&self, state: &ReproductionState) -> bool {
        state.eligible_now && state.surplus_ticks >= self.hold_required()
    }

    pub fn gestation_ticks(&self) -> u64 {
        cubarium_voxel::ticks_at_least_one(self.gestation_s)
    }

    pub fn interval_ticks(&self) -> u64 {
        cubarium_voxel::ticks_at_least_one(self.birth_interval_s)
    }

    pub fn incubation_ticks(&self) -> u64 {
        cubarium_voxel::ticks_at_least_one(self.incubation_s)
    }
}

/// A phase-one founder's held actions and prior-interval feedback. The heuristic bodies
/// carry it all-default and never read it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FounderState {
    /// The actions held from the last due controller sampling, already through the
    /// shared adapter — bounded and deadbanded. Rest until the first sampling: an
    /// unsampled founder holds no action.
    pub held: Actions,
    /// What the last completed interval actually did, consumed and zeroed once at the
    /// next sampling. The `Self` channels are built from this; it never carries a
    /// prediction of success.
    pub feedback: IntervalFeedback,
    /// The held **lure** effort, a carnivore's fourth action (predation §2.2): lit while
    /// above zero, costing its `lure_power`. Zero for every other lineage.
    pub lure: f64,
}

/// What one controller interval actually did, from the body's own ledger events and
/// resolved motion — the prior-interval feedback the observation's `Self` channels are
/// built from, and zeroed once when it is consumed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IntervalFeedback {
    /// Organic matter assimilated into body and reserve over the interval.
    pub intake: f64,
    /// Internal tissue lost over the interval: the share of upkeep and motor
    /// respiration that came out of the body itself once the reserve was empty.
    pub structural_loss: f64,
    /// Requested equivalent displacement over the interval, `|v| + r·|yaw rate|`
    /// summed over the ticks it was held. A wall-constrained attempt is attempted in
    /// full.
    pub attempted_equivalent: f64,
    /// Delivered equivalent displacement over the interval: what the sweep actually
    /// covered plus the turn the body actually made.
    pub delivered_equivalent: f64,
    /// Metres actually covered along the heading over the interval.
    pub delivered_forward: f64,
    /// Radians actually turned over the interval, signed.
    pub delivered_turn: f64,
    /// Organic matter the interval's paid motion respired, reserve and body together —
    /// the per-interval motor-respiration counter, alongside the ledger's cumulative
    /// `respired_motor_out`. Zero for a resting interval.
    pub motor_respiration: f64,
}

/// One species' numbers. **Every one of them is an untuned placeholder**; see
/// [`SpeciesConfig::frondgrazer`] and `design/backlog.md` §1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpeciesConfig {
    /// Organic matter respired per unit of body per second, standing still. Paid from the
    /// reserve, then from the body.
    pub maintenance_per_s: f64,
    /// Organic matter a mouth can take per second, spread over the stands in reach in site
    /// order. The bound on what a bite *asks* for; the plant layer bounds what it gets.
    ///
    /// It is the *hungry* rate: a bite asks for `bite_per_s · period · effort · hunger`, where
    /// hunger is the reserve's shortfall from `reserve_cap · body`, then takes
    /// `E / (E + bite_half_stock)` of that from food holding `E`, and never more than
    /// `room / yield`, the reserve's room after yield — so nothing it eats is respired as
    /// surplus.
    pub bite_per_s: f64,
    /// **`K`, the stock at which a bite is half of what the mouth asked for** (package G,
    /// decision 2): a bite from food holding `E` at the mouth — a plant's edible foliage
    /// in the mouth band, or a detritus pool's stock — is `want · E / (E + K)`. A stripped
    /// plant gives crumbs, so moving on pays.
    pub bite_half_stock: f64,
    /// Mineral per unit of tissue built — the density of what it builds, and therefore the
    /// **budget** its growth fits inside: a bite can build at most `mineral / n_tissue` of
    /// tissue, whatever its food's yield would otherwise have assimilated, and the
    /// unfunded organic matter is respired with its energy. Mineral a bite
    /// carries in excess of what was built is excreted as a litter deposit.
    ///
    /// There is **no mineral reserve**: [`Animal::mineral`] is an inventory of the tissue's
    /// own content and growth never draws on it, so a mineral-free bite builds nothing
    /// however much mineral the animal is carrying. At the placeholders the budget binds
    /// on every bite — a plant's foliage holds 0.02 against this 0.05 — so a frondgrazer
    /// turns 40 % of what it eats into tissue and not the 50 % its yield names.
    pub n_tissue: f64,
    /// Structure it grows to and no further.
    pub body_max: f64,
    /// **The fastest structure can grow**, organic matter per second (package G,
    /// decision 4). Intake goes to the reserve; each tick the reserve pays for
    /// `growth_max_per_s · DT · fullness` of new structure, `fullness` being the reserve
    /// over `reserve_cap · body` (at most 1), until `body_max`. A juvenile cannot outgrow
    /// it however much it eats, and a hungry one grows slower rather than spending the
    /// buffer it needs to live on.
    pub growth_max_per_s: f64,
    /// Structure below which it is dead. Also a newborn's body.
    pub body_min: f64,
    /// Structure at which it can pay for a birth.
    pub birth_body: f64,
    /// Organic matter a birth costs its parent, all of it out of the reserve. At least
    /// `body_min`, because that is the newborn's body; the remainder is the newborn's
    /// reserve.
    pub birth_cost: f64,
    /// The ceiling on **new intake** into the reserve, per unit of body: assimilated
    /// matter stops going to the reserve at `reserve_cap · body` and is respired instead
    /// ([`step`]'s assimilation). It is deliberately **not** a universal storage bound.
    ///
    /// **The one exception, named.** A paid birth endowment may sit above it. A newborn is
    /// `body_min` of body and `birth_cost − body_min` of reserve, which at the
    /// placeholders is `0.005` against a cap of `reserve_cap · body_min` = `0.0025`: the
    /// parcel is what the parent actually paid for out of its own reserve, so clamping it
    /// would either destroy organic matter or need a conserving birth allocation, which is
    /// a life-history decision and not a repair. While a reserve is above the ceiling no
    /// intake raises it — `room` is zero and the surplus is respired — and maintenance
    /// spends it exactly as it spends any other reserve, so the excess is transient and
    /// never grows.
    pub reserve_cap: f64,
    /// The depth of standing water on its own face that kills it.
    pub drown_depth_m: f64,
    /// Energy per unit of organic matter an **introduced** animal arrives with. Used for
    /// nothing else: every other unit of energy in this layer came in with a bite at the
    /// food's own density, and a newborn takes its parent's pro rata.
    pub energy_density: f64,
    /// How a body of this kind puts offspring into the world: the gestation escrow or a
    /// clutch of eggs, with the thresholds and intervals both rules run on
    /// ([`Reproduction`], `design/handoffs/voxel-reproduction-2026-09-21.md`).
    ///
    /// It rides here, inside the founder's own physiology and inside the placeholder
    /// species', because [`effective_config`] is already the one lookup every rule uses
    /// to ask what numbers a body runs on, and reproduction is not a different kind of
    /// question. A body with no lineage therefore reproduces by its species' table, and
    /// the browser founder — which *is* the frondgrazer — shares that table with it.
    pub reproduction: Reproduction,
    /// **Median lifespan**, seconds of simulated time: every body of this kind draws its
    /// own lifespan around it at birth ([`draw_lifespan_ticks`]) and departs
    /// [`Departure::Aged`] when its age reaches it. Zero is no lifespan.
    ///
    /// Four simulated hours for both current lineages
    /// (`design/handoffs/voxel-lifespan-defence-2026-09-25.md`): long against a shredder
    /// hatchling's hour to its first clutch and a browser's first birth, short against the
    /// 6 h a census runs, so a world turns over within one run.
    #[serde(default)]
    pub lifespan_median_s: f64,
}

impl SpeciesConfig {
    /// The paid ground browser of the biosphere's grazed meadow
    /// (`design/theoretical-biosphere-2026-09-16.md` §5, branch 1): a small body that
    /// eats low foliage it can stand under, walks to the next patch when its own is bare,
    /// cannot climb a step taller than itself, wades a film and drowns in a pool.
    ///
    /// **Every number is a placeholder and nothing measured any of them.** They are
    /// chosen to encode that sentence at the scale of the small plant presets: a
    /// springturf founder carries `α · W` = 0.06 of foliage, so a body of 0.05 and a
    /// mouth of 0.002 /s put one animal and one turf in the same order of magnitude.
    pub fn frondgrazer() -> SpeciesConfig {
        SpeciesConfig {
            maintenance_per_s: 0.001,
            bite_per_s: 0.002,
            // A turf grazed nearly to its floor: 0.005 is 6 % of a full-grown springturf's
            // edible foliage (0.7 · 2 · 0.06 = 0.084), so any plant worth the name gives
            // ~90 % of a bite and only one stripped to within crumbs of its refuge gives
            // half. Measured on the default preset, seed 2: 4 of 8 founders alive at
            // 60 min at K = 0.02, 6 at 0.005, with offtake under 10 % either way.
            bite_half_stock: 0.005,
            n_tissue: 0.05,
            body_max: 0.05,
            // From body_min to birth_body (0.025) in ≥ 2,632 s, so that with the 900 s hold
            // and 180 s gestation a newborn's first birth is ≥ 3,712 s — the brief's "no
            // sooner than ~1 h after being born".
            growth_max_per_s: 9.5e-6,
            body_min: 0.005,
            birth_body: 0.03,
            birth_cost: 0.01,
            reserve_cap: 0.5,
            drown_depth_m: 0.2,
            energy_density: 2.0,
            reproduction: Reproduction::LIVE_BIRTH_PLACEHOLDER,
            lifespan_median_s: LIFESPAN_MEDIAN_S,
        }
    }

    /// Multiply the **eight absolute quantities** — `body_max`, `body_min`, `birth_body`,
    /// `birth_cost`, the reproduction `surplus_floor` and `egg_organic`, `bite_per_s` and
    /// `growth_max_per_s` — by `k` (audit §1.4): the animal is `k` times heavier against
    /// the plants, and every per-unit rate, clock and metre stays. The bite's
    /// half-saturation `bite_half_stock` stays too: it is measured in the food's stock at
    /// the mouth, and the food is not scaled (units correction, 2026-10-01; at x30 it had
    /// cut a browser's bite to ~0.17 of its want).
    pub fn scale_mass(&mut self, k: f64) {
        self.body_max *= k;
        self.body_min *= k;
        self.birth_body *= k;
        self.birth_cost *= k;
        self.reproduction.surplus_floor *= k;
        self.reproduction.egg_organic *= k;
        self.bite_per_s *= k;
        self.growth_max_per_s *= k;
    }

    /// The reserve intake into a body of this size stops at: `reserve_cap · body`. A
    /// ceiling on new intake and not a storage bound — a newborn's paid endowment starts
    /// above it (see [`SpeciesConfig::reserve_cap`]) — so a caller must not read this as
    /// "what it is holding" or clamp to it.
    pub fn reserve_of(&self, body: f64) -> f64 {
        self.reserve_cap * body
    }
}

/// The whole layer's numbers: one [`SpeciesConfig`] per species, and nothing else. There is
/// no global constant in this layer — no decomposition, no cap, no lottery — because every
/// rule it has belongs to an animal.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FaunaConfig {
    pub frondgrazer: SpeciesConfig,
    /// The two phase-one founders' own physiology and cost settings
    /// ([`FounderPhysiology`], frozen for the first pilots). A body carrying a
    /// [`Founder`] marker runs its founder's numbers instead of its placeholder
    /// species'; [`effective_config`] is the one lookup every rule uses.
    pub founders: [FounderPhysiology; Founder::COUNT],
    /// The chance a dormant egg past its footfall gate hatches at one dusk
    /// ([`hatch_draw`]; default a third: the bank bet-hedges
    /// across nights).
    pub hatch_chance_per_dusk: f64,
    /// **Parasite hatchlings** (`design/handoffs/voxel-predation-parasite-2026-10-01.md`):
    /// metres from a hatching egg's column centre within which the nearest living
    /// non-lanternjaw animal with room is taken as its host. Default one grazer body
    /// length (the browser's `adult_length_m`, 0.75 m; metres do not scale with mass).
    pub parasite_attach_radius_m: f64,
    /// Riders one host can carry at once, count. Default 1.
    pub parasite_max_per_host: u32,
    /// A rider's drain, as a fraction of its host's awake basal upkeep
    /// (`maintenance_per_s × body`, organic units per second, whatever the host's activity),
    /// moved host → rider each tick through the three-currency pro-rata path (reserve
    /// first, then structure; mineral and energy at the host's own density). Default
    /// 0.05: at the brief's 0.01 a hatchling's take (about 1.5e-5/s on a shipped grazer)
    /// is below its own upkeep (about 2e-5/s), so no rider would ever reach drop-off.
    pub parasite_drain_of_host_basal: f64,
    /// A rider drops off when its body reaches this fraction of its lineage's adult
    /// `body_max`, dimensionless within `(0, 1]`. Default 0.24.
    pub parasite_detach_body_fraction: f64,
    /// Predation, threat perception, wounds and the egg bank ([`PredationConfig`]).
    pub predation: PredationConfig,
    /// The day's lines, the lifespan draw, the gut and the wall climb ([`LifeConfig`]).
    pub life: LifeConfig,
    /// The detritus cue field and the references the senses read through ([`CueConfig`]).
    pub cue: CueConfig,
    /// The heuristic controllers' reflexes ([`TeacherConfig`]).
    pub teachers: TeacherConfig,
    /// How a fresh world is founded ([`FoundingConfig`]).
    pub founding: FoundingConfig,
}

/// **The shipped org-unit mass scale** (Wrysk, 2026-09-29; audit
/// `design/7_Research/ecology-parameter-audit-2026-09-28.md` §1.4): both lineages are
/// built this many times heavier against the plants than the placeholder tables
/// ([`SpeciesConfig::scale_mass`]: the eight absolute quantities, nothing per unit), so one
/// plant no longer feeds a parked animal for ever. Plants are to be re-tuned for day and
/// night later; this is not a measured optimum.
pub const SHIPPED_MASS_SCALE: f64 = 30.0;

impl Default for FaunaConfig {
    fn default() -> FaunaConfig {
        let mut frondgrazer = SpeciesConfig::frondgrazer();
        frondgrazer.scale_mass(SHIPPED_MASS_SCALE);
        FaunaConfig {
            frondgrazer,
            founders: Founder::ALL.map(FounderPhysiology::frozen),
            hatch_chance_per_dusk: 1.0 / 3.0,
            parasite_attach_radius_m: FounderPhysiology::placeholder(Founder::Browser)
                .adult_length_m,
            parasite_max_per_host: 1,
            parasite_drain_of_host_basal: 0.05,
            parasite_detach_body_fraction: 0.24,
            predation: PredationConfig::default(),
            life: LifeConfig::default(),
            cue: CueConfig::default(),
            teachers: TeacherConfig::default(),
            founding: FoundingConfig::default(),
        }
    }
}

impl FaunaConfig {
    /// The placeholder tables before [`SHIPPED_MASS_SCALE`]: every lineage at mass 1. For
    /// fixtures whose numbers were derived from the placeholders; the shipped world is
    /// [`FaunaConfig::default`].
    pub fn placeholder() -> FaunaConfig {
        FaunaConfig {
            frondgrazer: SpeciesConfig::frondgrazer(),
            founders: Founder::ALL.map(FounderPhysiology::placeholder),
            ..FaunaConfig::default()
        }
    }

    pub fn species(&self, s: Species) -> &SpeciesConfig {
        match s {
            Species::Frondgrazer => &self.frondgrazer,
        }
    }

    pub fn species_mut(&mut self, s: Species) -> &mut SpeciesConfig {
        match s {
            Species::Frondgrazer => &mut self.frondgrazer,
        }
    }

    /// One founder's physiology, by lineage.
    pub fn founder(&self, founder: Founder) -> &FounderPhysiology {
        &self.founders[founder.index()]
    }

    /// **How many each lineage founds** in a fresh world, in [`Founder::ALL`] order: each
    /// physiology's [`FounderPhysiology::founding_count`] — bodies, or for
    /// [`Founding::Clutches`] the egg bank's clutches.
    pub fn founder_counts(&self) -> [usize; Founder::COUNT] {
        Founder::ALL.map(|f| self.founder(f).founding_count)
    }

    /// [`FaunaConfig::founder_counts`] with **no egg bank**: every [`Founding::Clutches`]
    /// lineage at 0. The predation measurement's control arm, and the training worlds
    /// until the lanternjaw has a brain (P2).
    pub fn founder_counts_without_egg_bank(&self) -> [usize; Founder::COUNT] {
        Founder::ALL.map(|f| {
            let p = self.founder(f);
            if p.founding == Founding::Clutches { 0 } else { p.founding_count }
        })
    }

    /// **The manifest the live body runs**: the founder's fixed [`Manifest`] with the
    /// motor numbers of its physiology in the config — cruise (`cruise_body_lengths_per_s`
    /// × the adult length), the yaw cap, and the forward reference that follows cruise.
    /// The sensor layout, references and tunings stay the contract's, so a default config
    /// returns exactly [`Founder::manifest`] and a moved pace changes the digest.
    pub fn manifest(&self, founder: Founder) -> Manifest {
        let mut m = founder.manifest();
        let p = self.founder(founder);
        m.cruise_m_per_s = p.cruise_body_lengths_per_s * p.adult_length_m;
        m.yaw_cap_rad_per_s = p.yaw_cap_rad_per_s;
        m.forward_reference_m = m.cruise_m_per_s * m.controller_period_s;
        m
    }

    /// Every number inside its row's range in the parameter registry ([`registry`]),
    /// and the cross-field rules: `body_min <= birth_body <= body_max`, a birth that can
    /// build a newborn, an egg-layer's clutch of at least one egg that can build a
    /// hatchling. **Not** a plausibility check: the placeholders are known to be
    /// arbitrary, and refusing them here would be tuning in a validator's clothes.
    pub fn validate(&self) -> Result<(), String> {
        registry().check(self)?;
        registry::cross_rules(self)
    }
}

/// Why a body left the world, as [`FaunaLedger`] books it.
///
/// Two of the three are **deaths** under [`step`]'s step 8 and leave a corpse; the third
/// is not a death at all and leaves none. Keeping all three in one enum is what lets a
/// census say "sixty-five bodies left, and here is the split" without a second bookkeeping
/// path: the two death causes sum to [`FaunaLedger::deaths`], and `Removed` is stated
/// beside them rather than hidden in the difference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Departure {
    /// `body` fell below the effective `body_min`: the reserve ran out and dieback took
    /// the structure past the floor. A corpse.
    Starved,
    /// Standing water deeper than the effective `drown_depth_m` on its own face. A
    /// corpse.
    Drowned,
    /// Old age: the body reached its own drawn lifespan ([`Animal::lifespan_ticks`],
    /// `design/handoffs/voxel-lifespan-defence-2026-09-25.md`). A corpse.
    Aged,
    /// No corpse: a [`Command::Remove`], or a body whose support face stopped being one
    /// ([`step`]'s step 1). Booked `removed_*_out` and **not** counted in
    /// [`FaunaLedger::deaths`].
    Removed,
    /// Its wound reached 1 (`design/predation-and-flight-2026-09-28.md` §2.5): a corpse,
    /// with a carcass record, killed by a body of lineage `by`.
    Killed { by: Founder },
}

impl Departure {
    pub const ALL: [Departure; 4 + Founder::COUNT] = {
        let mut all = [Departure::Starved; 4 + Founder::COUNT];
        all[1] = Departure::Drowned;
        all[2] = Departure::Aged;
        all[3] = Departure::Removed;
        let mut k = 0;
        while k < Founder::COUNT {
            all[4 + k] = Departure::Killed { by: Founder::ALL[k] };
            k += 1;
        }
        all
    };
    pub const COUNT: usize = Departure::ALL.len();

    /// Index into a per-cause array, in [`Departure::ALL`] order.
    pub fn index(self) -> usize {
        match self {
            Departure::Starved => 0,
            Departure::Drowned => 1,
            Departure::Aged => 2,
            Departure::Removed => 3,
            Departure::Killed { by } => 4 + by.index(),
        }
    }

    /// Whether this departure left a corpse and is counted in [`FaunaLedger::deaths`].
    pub fn is_death(self) -> bool {
        !matches!(self, Departure::Removed)
    }

    pub fn name(self) -> &'static str {
        match self {
            Departure::Starved => "starved",
            Departure::Drowned => "drowned",
            Departure::Aged => "aged",
            Departure::Removed => "removed",
            Departure::Killed { by } => {
                static NAMES: std::sync::OnceLock<[&'static str; Founder::COUNT]> =
                    std::sync::OnceLock::new();
                NAMES.get_or_init(|| {
                    Founder::ALL.map(|f| -> &'static str {
                        Box::leak(format!("killed_by_{}", f.name()).into_boxed_str())
                    })
                })[by.index()]
            }
        }
    }
}

/// The shipped median lifespan of every body kind: four simulated hours.
pub const LIFESPAN_MEDIAN_S: f64 = 4.0 * 3600.0;

/// A body's lifespan in ticks, from its kind's median (seconds): **uniform on
/// `[0.8, 1.2] × median`** for a newborn (`life.lifespan_draw_min` and `_span`) — bounded,
/// so no body is immortal or dies in infancy, and ±20 % around the median as the brief
/// asks. An **arriving** body (an introduced founder) has already lived part of its
/// life: its remaining span is a uniform `[0.5, 1.0)` share (`life.arriving_share_min`)
/// of such a draw, so a founding cohort ages out over half a
/// lifespan rather than all at once. Deterministic in `(id, salt)`. Zero median: zero,
/// no lifespan.
pub fn draw_lifespan_ticks(
    life: &LifeConfig,
    median_s: f64,
    id: u64,
    salt: u64,
    arriving: bool,
) -> u64 {
    if !(median_s.is_finite() && median_s > 0.0) {
        return 0;
    }
    let unit = |k: u64| {
        let z = id.wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ salt.rotate_left(29)
            ^ k.wrapping_mul(0xD1B5_4A32_D192_ED03)
            ^ 0x11FE_5BA2_A6ED_0001;
        cubarium_voxel::unit_from_bits(cubarium_voxel::mix(z))
    };
    let mut span = median_s * (life.lifespan_draw_min + life.lifespan_draw_span * unit(1));
    if arriving {
        span *= life.arriving_share_min + (1.0 - life.arriving_share_min) * unit(2);
    }
    cubarium_voxel::ticks_at_least_one(span)
}

/// The per-body salt a lifespan draw mixes with the id: where and when the body appeared.
pub(crate) fn lifespan_salt(site: Site, tick: u64) -> u64 {
    (u64::from(site.x) << 40) ^ (u64::from(site.z) << 20) ^ u64::from(site.y) ^ tick.rotate_left(13)
}

/// Which **food class** a bite came out of (decisions §3,
/// `design/handoffs/voxel-organism-decisions-2026-09-21.md`).
///
/// A food class is not a species and not a pool: it is the kind of tissue a mouth took,
/// which is what the diet rules and the per-class assimilation yields are written
/// against. The browser has one, `Foliage`; the shredder has three, and its three are
/// the detritus the cue field now sums.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Food {
    /// The ground litter pool at the standing face — the shredder's original and only
    /// food ([`cubarium_voxel_flora::Flora::take_litter`]).
    Litter,
    /// A glowcap cap: the foliage of a [`cubarium_voxel_flora::Trophic::Saprotroph`]
    /// stand whose layer intersects the mouth band, taken as an ordinary foliage
    /// withdrawal. Fungal tissue, which is why it is not the browser's.
    CapTissue,
    /// The ground carrion pool at the standing face
    /// ([`cubarium_voxel_flora::Flora::take_carrion`]). Carrion had decomposition and no
    /// consumer before this.
    Carrion,
    /// The foliage of a vascular ([`cubarium_voxel_flora::Trophic::Photo`]) stand: the
    /// browser's whole diet, and nothing the shredder eats.
    Foliage,
    /// **Glassfilm** (roster R2b): the ground film pool at the standing face
    /// ([`cubarium_voxel_flora::Flora::take_film`]), the ripple-snail's food. Fresh tissue,
    /// not detritus.
    Film,
}

impl Food {
    pub const ALL: [Food; 5] = [
        Food::Litter,
        Food::CapTissue,
        Food::Carrion,
        Food::Foliage,
        Food::Film,
    ];
    pub const COUNT: usize = Food::ALL.len();

    /// Index into a per-food array, in [`Food::ALL`] order.
    pub const fn index(self) -> usize {
        match self {
            Food::Litter => 0,
            Food::CapTissue => 1,
            Food::Carrion => 2,
            Food::Foliage => 3,
            Food::Film => 4,
        }
    }

    /// Whether a mouth takes this food from a **ground pool** at its standing face
    /// (litter, carrion, film) rather than from a stand's tissue (cap tissue, foliage).
    pub const fn is_pool(self) -> bool {
        matches!(self, Food::Litter | Food::Carrion | Food::Film)
    }

    /// The food class of a stand's tissue, by its species' income rule: a saprotroph's is
    /// cap tissue, a plant's foliage. The stand decides, not whose mouth took it.
    pub const fn of_stand(trophic: cubarium_voxel_flora::Trophic) -> Food {
        match trophic {
            cubarium_voxel_flora::Trophic::Photo => Food::Foliage,
            cubarium_voxel_flora::Trophic::Saprotroph => Food::CapTissue,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Food::Litter => "litter",
            Food::CapTissue => "cap_tissue",
            Food::Carrion => "carrion",
            Food::Foliage => "foliage",
            Food::Film => "film",
        }
    }
}

/// **What a lineage eats**: a set of [`Food`] classes, the lineage's
/// [`FounderPhysiology::diet`] (a knob; in TOML a list of food names,
/// `diet = ["litter", "cap_tissue", "carrion"]`).
///
/// The mouth offers each food in the set from its own source ([`body`]'s generalised
/// mouth): litter and carrion from the ground pool at the standing face, cap tissue from a
/// [`cubarium_voxel_flora::Trophic::Saprotroph`] stand, foliage from a
/// [`cubarium_voxel_flora::Trophic::Photo`] stand. The order of the list does not matter:
/// when several foods are at the mouth the richest offer wins, ties in [`Food::ALL`]
/// order.
///
/// Reach and permission are separate tests (audit §2): a glowcap cap standing in a
/// browser's band is reachable and is not food. [`Diet::accepts`] is the permission,
/// applied inside the one scan every mouth runs, so a stand a mouth may not eat is never
/// offered to it as taste either. [`Diet::ANY`] is the diagnostic and observer scan of
/// what physically stands in a band, a geometry question and not a diet one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "Vec<Food>", into = "Vec<Food>")]
pub struct Diet {
    foods: [bool; Food::COUNT],
}

impl Diet {
    /// Every food class: accepts every stand.
    pub const ANY: Diet = Diet { foods: [true; Food::COUNT] };
    /// Nothing at all.
    pub const NONE: Diet = Diet { foods: [false; Food::COUNT] };

    /// The diet of one food.
    pub const fn only(food: Food) -> Diet {
        let mut foods = [false; Food::COUNT];
        foods[food.index()] = true;
        Diet { foods }
    }

    /// The diet of these foods.
    pub fn of(foods: &[Food]) -> Diet {
        let mut d = Diet::NONE;
        for f in foods {
            d.foods[f.index()] = true;
        }
        d
    }

    /// Whether `food` is in it.
    pub const fn eats(self, food: Food) -> bool {
        self.foods[food.index()]
    }

    /// Its foods, in [`Food::ALL`] order.
    pub fn foods(self) -> impl Iterator<Item = Food> {
        Food::ALL.into_iter().filter(move |f| self.eats(*f))
    }

    /// Whether this diet accepts a stand of a species with that income rule: a
    /// photosynthetic stand's tissue is [`Food::Foliage`], a saprotroph's
    /// [`Food::CapTissue`] ([`Food::of_stand`]).
    pub fn accepts(self, trophic: cubarium_voxel_flora::Trophic) -> bool {
        self.eats(Food::of_stand(trophic))
    }

    /// Whether any of its foods is a stand's tissue rather than a ground pool.
    pub fn eats_stands(self) -> bool {
        self.foods().any(|f| !f.is_pool())
    }
}

impl From<Vec<Food>> for Diet {
    fn from(foods: Vec<Food>) -> Diet {
        Diet::of(&foods)
    }
}

impl From<Diet> for Vec<Food> {
    fn from(d: Diet) -> Vec<Food> {
        d.foods().collect()
    }
}

/// Every unit of organic matter, mineral and energy that has crossed this layer's
/// boundary, and the counters of what happened.
///
/// The three currencies are the plant layer's ([`cubarium_voxel_flora::FloraLedger`]) and
/// mean the same thing. Internal moves — a bite becoming tissue, a reserve paying
/// maintenance, a parent paying for a newborn — never appear here.
///
/// **Two pairs of terms the brief did not name are here because the residuals do not close
/// without them.** An [`Command::IntroduceFounder`] creates an animal out of nothing, exactly as
/// the plant layer's `Seed` creates a founder stand, so it is booked `introduced_*_in`
/// the way that is booked `seeded_*_in`; and [`Command::Remove`] takes one out of the
/// world without a corpse, so it is booked `removed_*_out`. Both are named boundary flows
/// and neither is hidden.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FaunaLedger {
    /// Organic matter, mineral and energy taken out of the plant layer by
    /// [`cubarium_voxel_flora::Flora::take_foliage`], cumulative over every bite. The
    /// sum of the [`cubarium_voxel_flora::Taken`]s the withdrawals returned, to the bit,
    /// so these three **equal** the plant layer's `consumed_*_out` for a run in which
    /// this layer is the only consumer.
    pub eaten_organic_in: f64,
    pub eaten_mineral_in: f64,
    pub eaten_energy_in: f64,
    /// Organic matter respired: maintenance, the undigested fraction of a bite, the
    /// founder's paid motor respiration, and anything assimilated that a full body and a
    /// full reserve could not hold. Mineral never crosses this boundary.
    ///
    /// The four split counters below sum to this **to the bit** — every booking adds the
    /// same addend to the total and to one split at the same point — so a driver reads
    /// an interval's or an episode's upkeep, motor, digestion and failed-gestation
    /// charges apart without a second ledger. The ES pilot's score needs the motor term
    /// apart from upkeep (`design/voxel-senses-phase1-tests.md` §2).
    pub respired_out: f64,
    /// The `respired_out` share that was basal upkeep.
    pub respired_maintenance_out: f64,
    /// The `respired_out` share that was the founder's paid motion.
    pub respired_motor_out: f64,
    /// The `respired_out` share that was digestion: the undigested fraction of a bite and
    /// assimilated matter neither the body nor the reserve could hold.
    pub respired_digestion_out: f64,
    /// The `respired_out` share that was an **interrupted gestation**:
    /// `gestation_loss_fraction` of the escrow of a parent that could not pay its next
    /// instalment or died carrying one. The fourth split, and the only one that is not
    /// something a living body spent on itself.
    pub respired_gestation_out: f64,
    /// The energy that left with it, at the density of the stock it left.
    pub heat_out: f64,
    /// Organic matter, mineral and energy handed back to the plant layer by
    /// [`cubarium_voxel_flora::Flora::deposit`]: a corpse as carrion, and excreted mineral
    /// as litter. Equal to the plant layer's `deposited_*_in` to the bit, for the same
    /// reason the `eaten_*` are.
    pub deposited_organic_out: f64,
    pub deposited_mineral_out: f64,
    pub deposited_energy_out: f64,
    /// What an introduction ([`Command::IntroduceFounder`], [`Command::BuryClutch`])
    /// created out of nothing.
    pub introduced_organic_in: f64,
    pub introduced_mineral_in: f64,
    pub introduced_energy_in: f64,
    /// What a [`Command::Remove`] — or an animal whose support face the terrain took away
    /// — removed without a corpse.
    pub removed_organic_out: f64,
    pub removed_mineral_out: f64,
    pub removed_energy_out: f64,
    /// Every animal this layer has created, introduced ones included: a counter, not a
    /// flux, and the source of [`Animal::id`]. `births − introduced` is how many were
    /// born here.
    pub births: u64,
    /// Animals born of a parent's reserve: a live birth out of a gestation escrow, and
    /// a hatchling out of an egg the parent paid for. `births − introduced == born`.
    pub born: u64,
    /// The `born` share that came out of an egg.
    pub hatched: u64,
    /// Gestations begun, and the ones that ended without a birth because the parent
    /// could not pay an instalment or died carrying the escrow. `opened − failed` is
    /// the number that reached term, once no gestation is still running.
    pub gestations_opened: u64,
    pub gestations_failed: u64,
    /// Clutches laid, and the eggs in them.
    pub clutches_laid: u64,
    pub eggs_laid: u64,
    /// Eggs lost with their site: the face was taken away or drowned, and the clutch
    /// went to carrion where it lay.
    pub eggs_lost: u64,
    /// Animals an [`Command::IntroduceFounder`] put into the world.
    pub introduced: u64,
    /// Animals that died: starved below `body_min`, or drowned.
    pub deaths: u64,
    /// The **cause** each of those deaths was booked under, by [`Departure::index`]:
    /// `Starved`, `Drowned`, then `Removed`.
    ///
    /// Instrumentation, not a rule: nothing here changes when a body dies, only what the
    /// ledger can be asked afterwards. `deaths_by_cause[Starved] +
    /// deaths_by_cause[Drowned]` is exactly [`FaunaLedger::deaths`]
    /// ([`FaunaLedger::deaths_accounted`] asserts it), because every death books both at
    /// the same point. `deaths_by_cause[Removed]` is **not** in `deaths` and never was: a
    /// body an [`Command::Remove`] took, or one whose support face the terrain took away,
    /// left the world without dying and without a corpse ([`step`]'s step 1), and the
    /// census could not tell that apart from a death before this counter existed.
    ///
    /// A body that is at once below `body_min` and under drowning water is booked
    /// `Starved`: the starvation clause is the one the death rule reads first, and a
    /// cause counter must name the clause that fired, not adjudicate between two.
    pub deaths_by_cause: [u64; Departure::COUNT],
    /// The same three counters restricted to bodies that carry a lineage, indexed
    /// `[founder][cause]` in [`Founder::index`] and [`Departure::index`] order. A
    /// body with no lineage appears only in the totals above.
    pub deaths_by_founder_cause: [[u64; Departure::COUNT]; Founder::COUNT],
    /// Withdrawals that returned something.
    pub bites: u64,
    /// Bites and organic matter **by the plant species they came off**, indexed by
    /// [`cubarium_voxel_flora::Species::index`].
    ///
    /// The plant layer's `consumed_*` have no species in them; what a body ate depends on
    /// where its mouth was, so this is a measurement taken where the bite happens.
    pub bites_by_plant: [u64; cubarium_voxel_flora::Species::COUNT],
    pub eaten_by_plant: [f64; cubarium_voxel_flora::Species::COUNT],
    /// Local founder bites and the organic matter they actually **placed** into body
    /// and reserve, by lineage, indexed by [`Founder::index`].
    ///
    /// The same kind of measurement as `bites_by_plant`, taken at the other end of the
    /// transfer: what a lineage got, not what a species lost. `assimilated_by_founder`
    /// is the placed figure and not the bite's gross organic matter, because that is
    /// what became tissue — it is the same number the body's own `Self`
    /// `assimilated_intake` channel reports.
    pub bites_by_founder: [u64; Founder::COUNT],
    pub assimilated_by_founder: [f64; Founder::COUNT],
    /// Bites and the **gross** organic matter they took, by the [`Food`] class the
    /// mouth took them from.
    ///
    /// The third view of the same transfer, and the one the diets package needed: a
    /// shredder's bites split three ways say whether carrion and cap tissue are food it
    /// actually finds or only food it is permitted. `eaten_by_food` is gross, like
    /// `eaten_by_plant` and unlike `assimilated_by_founder`, so it sums to
    /// `eaten_organic_in` over every bite this layer took.
    pub bites_by_food: [u64; Food::COUNT],
    pub eaten_by_food: [f64; Food::COUNT],
    /// A founder body's bites by lineage **and** food class (roster R1's measure: which
    /// of its foods a lineage actually takes, when two lineages share one). Founder
    /// bites only, so each row sums to [`FaunaLedger::bites_by_founder`].
    pub bites_by_founder_food: [[u64; Food::COUNT]; Founder::COUNT],
    /// Steps taken, one voxel each.
    pub steps: u64,
    /// Movement counters, per lineage: walls climbed to the top and
    /// cliffs climbed to the foot (each counted once, on arrival); ticks spent on a face
    /// and ticks alive, so the share of time on walls is the one over the other; and
    /// sweeps that moved the standing layer up or down a ledge.
    pub wall_ascents_by_founder: [u64; Founder::COUNT],
    pub wall_descents_by_founder: [u64; Founder::COUNT],
    pub wall_ticks_by_founder: [u64; Founder::COUNT],
    pub founder_ticks_by_founder: [u64; Founder::COUNT],
    pub ledges_up_by_founder: [u64; Founder::COUNT],
    pub ledges_down_by_founder: [u64; Founder::COUNT],
    /// Predation (`design/predation-and-flight-2026-09-28.md` §2.3), by the **striker's**
    /// lineage: strikes begun, and contacts resolved as a hit or a dodge.
    pub strikes_by_founder: [u64; Founder::COUNT],
    pub hits_by_founder: [u64; Founder::COUNT],
    pub dodges_by_founder: [u64; Founder::COUNT],
    /// Wound units healed, by the healed body's lineage (§2.4).
    pub healed_by_founder: [f64; Founder::COUNT],
    /// Carrion taken by each lineage's mouths, gross organic matter: who ate the carcasses.
    pub eaten_carrion_by_founder: [f64; Founder::COUNT],
    /// The `respired_out` share that was a **hunt**: strikes and a lit lure.
    pub respired_hunt_out: f64,
    /// The `respired_out` share that was **healing** a wound.
    pub respired_heal_out: f64,
    /// Eggs laid and hatched, by lineage.
    pub eggs_laid_by_founder: [u64; Founder::COUNT],
    pub eggs_hatched_by_founder: [u64; Founder::COUNT],
    /// Dormant eggs whose viability ran out (or whose upkeep took them below a hatchling),
    /// gone to the litter of their face (§2.13).
    pub eggs_died: u64,
    /// Eggs a [`Command::BuryClutch`] put into the world (a founding bank or the viewer's
    /// key), booked `introduced_*_in`.
    pub eggs_buried: u64,
    /// Parasite hatchlings: eggs that hatched onto a host (riders attached, cumulative),
    /// riders that dropped off at their detach size, and riders freed by their host's
    /// departure (any cause).
    pub riders_attached: u64,
    pub rider_dropoffs: u64,
    pub riders_freed: u64,
}

impl FaunaLedger {
    /// How many bodies left this way, whatever lineage they carried.
    pub fn departed(&self, cause: Departure) -> u64 {
        self.deaths_by_cause[cause.index()]
    }

    /// How many bodies of one lineage left this way.
    pub fn departed_founder(&self, founder: Founder, cause: Departure) -> u64 {
        self.deaths_by_founder_cause[founder.index()][cause.index()]
    }

    /// The death causes, summed: equal to [`FaunaLedger::deaths`] for any ledger this
    /// layer produced, which is the instrumentation's own invariant.
    pub fn deaths_accounted(&self) -> u64 {
        Departure::ALL
            .iter()
            .filter(|d| d.is_death())
            .map(|d| self.departed(*d))
            .sum()
    }

    pub fn expected_organic(&self) -> f64 {
        self.introduced_organic_in + self.eaten_organic_in
            - self.respired_out
            - self.deposited_organic_out
            - self.removed_organic_out
    }

    pub fn expected_mineral(&self) -> f64 {
        self.introduced_mineral_in + self.eaten_mineral_in
            - self.deposited_mineral_out
            - self.removed_mineral_out
    }

    pub fn expected_energy(&self) -> f64 {
        self.introduced_energy_in + self.eaten_energy_in
            - self.heat_out
            - self.deposited_energy_out
            - self.removed_energy_out
    }
}

/// Read-only access for drawing, inspection and a harness.
#[derive(Clone, Copy, Debug)]
pub struct FaunaView<'a> {
    pub config: &'a FaunaConfig,
    pub tick: u64,
    /// Sorted by [`Animal::id`], which never changes: an animal keeps its place in this
    /// slice for its whole life, whatever it walks over.
    pub animals: &'a [Animal],
    /// The clutches standing in the world, in the order they were laid. Not animals:
    /// stationary paid packages this layer still owns until they hatch.
    pub clutches: &'a [Clutch],
    pub ledger: &'a FaunaLedger,
}

impl<'a> FaunaView<'a> {
    /// The animal with this identity, if it is still alive.
    pub fn animal(&self, id: u64) -> Option<&'a Animal> {
        self.animals
            .binary_search_by_key(&id, |a| a.id)
            .ok()
            .map(|i| &self.animals[i])
    }

    /// Every animal standing on this face, in id order. More than one is allowed: a
    /// newborn appears on its parent's face.
    pub fn animals_at(&self, site: Site) -> impl Iterator<Item = &'a Animal> {
        self.animals.iter().filter(move |a| a.site == site)
    }

    /// Organic matter this layer holds: every animal's structure and reserve, the
    /// escrowed offspring inside gestating parents, and the eggs standing in the world.
    /// This is the stock the ledger's `expected_organic` is checked against, so every
    /// package the layer owns has to be in it or a paid offspring would read as matter
    /// created or destroyed.
    pub fn organic(&self) -> f64 {
        self.animals.iter().map(Animal::stored_organic).sum::<f64>()
            + self.clutches.iter().map(|c| c.organic).sum::<f64>()
    }

    /// Mineral in every animal's tissue, its escrow and every standing clutch.
    pub fn mineral(&self) -> f64 {
        self.animals.iter().map(Animal::stored_mineral).sum::<f64>()
            + self.clutches.iter().map(|c| c.mineral).sum::<f64>()
    }

    /// Energy in every animal's tissue, its escrow and every standing clutch.
    pub fn energy(&self) -> f64 {
        self.animals.iter().map(Animal::stored_energy).sum::<f64>()
            + self.clutches.iter().map(|c| c.energy).sum::<f64>()
    }

    /// Eggs of one lineage standing in the world, over every clutch.
    pub fn eggs_by_founder(&self, founder: Founder) -> u64 {
        self.clutches
            .iter()
            .filter(|c| c.lineage == founder)
            .map(|c| u64::from(c.count))
            .sum()
    }

    /// Bodies of one lineage currently carrying a gestation escrow.
    pub fn gestating_by_founder(&self, founder: Founder) -> u64 {
        self.animals
            .iter()
            .filter(|a| a.founder == Some(founder) && a.reproduction.escrow.is_some())
            .count() as u64
    }

    /// Every clutch on this face, in the order they were laid.
    pub fn clutches_at(&self, site: Site) -> impl Iterator<Item = &'a Clutch> {
        self.clutches.iter().filter(move |c| c.site == site)
    }

    /// **Tremor** at `observer` (§2.2): the sum over the other bodies within the tremor radius of
    /// `mass × speed / distance²`, split into its left and right halves by the observer's
    /// heading — `[left, right]`. A still body makes none. Zero for an observer that is
    /// not alive.
    pub fn tremor(&self, view: &VoxelView<'_>, observer: u64) -> [f64; 2] {
        let cfg = &self.config.predation;
        let Some(o) = self.animal(observer) else {
            return [0.0, 0.0];
        };
        let ring = f64::from(view.config.width) * view.config.voxel_m;
        let (fx, fz) = o.pose.forward();
        let (rx, rz) = (fz, -fx);
        let mut out = [0.0, 0.0];
        for a in self.animals {
            if a.id == observer || !(a.speed_m_s > 0.0) {
                continue;
            }
            let (dx, dz) = pose::wrapped_delta(o.pose.x, o.pose.z, a.pose.x, a.pose.z, ring);
            let d = (dx * dx + dz * dz).sqrt();
            if d > cfg.tremor_radius_m {
                continue;
            }
            let t = tremor_of(a, d, cfg.tremor_near_m);
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
}

/// A frontend, a harness or a test changes this layer only through these.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// Put a **founder** body on the highest support face of column `(x, z)`, at that
    /// face's centre and the given `heading_rad`, with its lineage's `n_tissue` of mineral
    /// and `energy_density` of energy — a body grown outside the system, booked
    /// `introduced_*_in`. A body that is not finite, a heading that is not finite, a
    /// column with no support, or a body below the founder's own `body_min` is refused.
    /// More than one animal may stand on a face, so this is never refused for being
    /// occupied.
    ///
    /// The founder marker selects which manifest's controller the body belongs to and
    /// which physiology it runs. The species tag is the shared placeholder
    /// (`Species::Frondgrazer`); every rule that reads a founder's numbers goes through
    /// [`effective_config`], which substitutes the founder table for it.
    IntroduceFounder {
        x: i64,
        z: u32,
        founder: Founder,
        /// How full the body arrives ([`StartingStores`]): a founder may be placed
        /// hungry, which is what makes eating worth anything to it.
        stores: StartingStores,
        heading_rad: f64,
    },
    /// [`Command::IntroduceFounder`] on **exactly this support face** — a hollow's floor,
    /// the ground under a shelf — rather than the skyline of its column. The seeder uses
    /// it so that the face it checked is the face the body stands on
    /// (`design/handoffs/voxel-startup-acceptance-2026-09-22.md` item 2). Refused as
    /// `IntroduceFounder` is, and also when `site` is not a support face.
    IntroduceFounderOnFace {
        site: Site,
        founder: Founder,
        stores: StartingStores,
        heading_rad: f64,
    },
    /// Take every animal off the highest support face of column `(x, z)`, booking their
    /// material as `removed_*_out`. No corpse: this is a frontend's undo, not a death.
    /// Refused if there is none.
    Remove { x: i64, z: u32 },
    /// **Bury `eggs` dormant eggs** of `lineage` on the support face `site`
    /// (`design/predation-and-flight-2026-09-28.md` §2.9, §2.13): a world's founding bank
    /// and the viewer's key. Each egg is its own clutch, its lineage's `egg_organic` with
    /// its tissue mineral and energy, booked `introduced_*_in`. Refused off a support face,
    /// for zero eggs, and for a lineage whose eggs are not dormant.
    BuryClutch {
        site: Site,
        lineage: Founder,
        eggs: u32,
    },
}

/// How full a founder body arrives, as fractions of what its physiology lets it hold.
///
/// A body introduced at [`StartingStores::FULL`] has no headroom: settled intake can only
/// replace the upkeep it already burned, so an episode's score is capped near standing
/// still. A body placed at [`StartingStores::HUNGRY`] has somewhere to put what it eats.
///
/// The manifest's `body_reference` is **not** this: it is the schema's fixed normaliser
/// and does not move with the start state.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StartingStores {
    /// Structure at introduction, as a fraction of the founder's `body_max`.
    pub body: f64,
    /// Reserve at introduction, as a fraction of the full reserve that much structure
    /// carries (`reserve_cap · body`).
    pub reserve: f64,
}

impl StartingStores {
    /// Everything the body can hold.
    pub const FULL: StartingStores = StartingStores {
        body: 1.0,
        reserve: 1.0,
    };

    /// The arenas' hungry founder: half its structure and no reserve, so a whole
    /// reference body of headroom is there to be eaten into.
    pub const HUNGRY: StartingStores = StartingStores {
        body: 0.5,
        reserve: 0.0,
    };

    /// The absolute `(body, reserve)` these fractions mean for `sc`, or `None` when they
    /// are not finite, not within `[0, 1]`, or the structure they ask for is below the
    /// physiology's `body_min` — a body that cannot live is refused, not clamped.
    pub fn resolve(&self, sc: &SpeciesConfig) -> Option<(f64, f64)> {
        if !(self.body.is_finite() && (0.0..=1.0).contains(&self.body)) {
            return None;
        }
        if !(self.reserve.is_finite() && (0.0..=1.0).contains(&self.reserve)) {
            return None;
        }
        let body = self.body * sc.body_max;
        if !(body >= sc.body_min) {
            return None;
        }
        Some((body, self.reserve * sc.reserve_of(body)))
    }
}

/// The animal layer. Owns its animals; borrows the world and the plant layer per call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fauna {
    config: FaunaConfig,
    tick: u64,
    /// Sorted by id, so every pass over them is in one stable order however they move.
    /// A `Vec` with a binary search, never a `HashMap`: this layer iterates nothing
    /// unordered.
    animals: Vec<Animal>,
    /// The clutches standing in the world, in the order they were laid — which is
    /// parent id order within a tick and tick order between ticks, so every pass over
    /// them is the same order however the world runs.
    clutches: Vec<Clutch>,
    ledger: FaunaLedger,
    /// Whether paid births run in [`step`]. Defaults to **true**, which is the live world's
    /// rule and changes nothing. An isolated arena sets it false
    /// (`design/voxel-senses-phase1-plan.md`, "Frozen arena contract"); a fresh episode
    /// sets it back.
    births_enabled: bool,
    /// The controllers the founder bodies are driven by, keyed by animal id. Driver-owned
    /// session state, not world state: skipped by the snapshot and not cloned
    /// ([`FounderControllers`]).
    #[serde(skip)]
    controllers: FounderControllers,
    /// How to build a fresh controller for a founder **born** here, one recipe per
    /// lineage. Skipped by the snapshot for the same reason the controllers are, but
    /// cloned with the layer: see [`FounderFactories`].
    #[serde(skip)]
    factories: FounderFactories,
    /// Which lineages were driven by a **saved policy** rather than their own
    /// heuristic, by [`Founder::index`]. This one **is** world state and the snapshot
    /// carries it.
    ///
    /// The controllers themselves are not saved — a `dyn Controller` does not serialize
    /// and a hidden state is session state — so on its own a loaded layer cannot tell a
    /// trained lineage from a heuristic one, and a loader would put the heuristic back
    /// without saying anything. That is a different world wearing the same bytes. This
    /// flag is the one thing a loader cannot re-derive, so it is recorded: a driver
    /// reading it either supplies the policy again or refuses the load.
    policy_driven: [bool; Founder::COUNT],
    /// The `weights_fnv1a` digest of the centre driving this lineage, by
    /// [`Founder::index`], or `0` for a heuristic or an untouched lineage. Recorded
    /// beside [`Fauna::policy_driven`] for the same reason and by the same installer:
    /// the bool alone can tell a trained lineage from a heuristic one, but not one
    /// trained centre from another, so a loaded world whose lineage was driven by one
    /// centre and is handed a *different* one could not be refused — only demoted or
    /// silently swapped, both of which `always-fresh-never-migrate` forbids. This is
    /// the identity a loader compares against.
    policy_digest: [u64; Founder::COUNT],
    /// The world's `life_daylight` on the last tick stepped, so a tick can tell **dusk**
    /// — daylight falling below [`LifeConfig::day_line`] — for the egg bank's trigger
    /// (§2.13). `None` before the first tick.
    last_daylight: Option<f64>,
}

impl Fauna {
    /// An animal layer on a **valid** config. Panics with the offending field named if
    /// [`FaunaConfig::validate`] refuses it, exactly as
    /// [`cubarium_voxel_flora::Flora::new`] does and for the same reason: a preset that
    /// cannot build a living animal is a programming error in the preset.
    pub fn new(config: FaunaConfig) -> Fauna {
        match Fauna::try_new(config) {
            Ok(fauna) => fauna,
            Err(e) => panic!("invalid FaunaConfig — {e}"),
        }
    }

    /// [`Fauna::new`] without the panic.
    pub fn try_new(config: FaunaConfig) -> Result<Fauna, String> {
        config.validate()?;
        Ok(Fauna {
            config,
            tick: 0,
            animals: Vec::new(),
            clutches: Vec::new(),
            ledger: FaunaLedger::default(),
            births_enabled: true,
            controllers: FounderControllers::default(),
            factories: FounderFactories::default(),
            policy_driven: [false; Founder::COUNT],
            policy_digest: [0; Founder::COUNT],
            last_daylight: None,
        })
    }

    /// Give the founder body `id` a controller: the diagnostic heuristic and the GRU
    /// policy are interchangeable behind [`Controller`], and this table is what the
    /// tick's controller stage drives. Accepted only for a body that exists; `true`
    /// when installed. A fresh episode clears a controller's own memory through
    /// [`Controller::reset`] — reinstalling is not what resets hidden state.
    pub fn set_controller(&mut self, animal_id: u64, controller: Box<dyn Controller>) -> bool {
        if self.view().animal(animal_id).is_none() {
            return false;
        }
        self.controllers.set(animal_id, controller);
        true
    }

    /// Take a body's controller back, if it has one: a driver that wants to replace or
    /// drop one between episodes.
    pub fn take_controller(&mut self, animal_id: u64) -> Option<Box<dyn Controller>> {
        self.controllers.take(animal_id)
    }

    /// Register how to build a controller for a founder **born** in this layer.
    ///
    /// A newborn founder inherits its parent's lineage and pays for its own body, but it
    /// inherits no mind: [`step`]'s birth rule asks this factory for a fresh controller
    /// of the parent's kind, so a lineage that breeds keeps being driven instead of
    /// filling the world with resting bodies that still pay upkeep. One factory per
    /// lineage, never a shared controller instance — two bodies sharing one controller
    /// would share its memory.
    ///
    /// Introduced bodies are **not** touched by this: a driver installs their controllers
    /// itself with [`Fauna::set_controller`], which is also how it chooses something
    /// other than the lineage default for a particular body.
    pub fn set_founder_factory(
        &mut self,
        founder: Founder,
        factory: std::sync::Arc<dyn ControllerFactory>,
    ) {
        self.factories.set(founder, factory);
    }

    /// Whether this lineage's bodies are driven by a saved policy rather than their own
    /// heuristic. Snapshot state: see the field's own note for why it is recorded.
    pub fn policy_driven(&self, founder: Founder) -> bool {
        self.policy_driven[founder.index()]
    }

    /// Record that this lineage is (or is no longer) driven by a saved policy. A driver
    /// sets it when it installs one; a loader reads it to decide whether it may run this
    /// world at all.
    pub fn set_policy_driven(&mut self, founder: Founder, driven: bool) {
        self.policy_driven[founder.index()] = driven;
    }

    /// The `weights_fnv1a` digest of the centre this lineage is recorded as running, or
    /// `0` for a heuristic or an untouched lineage. Snapshot state: see the field's own
    /// note for why it is recorded beside [`Fauna::policy_driven`].
    pub fn policy_digest(&self, founder: Founder) -> u64 {
        self.policy_digest[founder.index()]
    }

    /// Record the `weights_fnv1a` digest of the centre now driving this lineage. A
    /// driver sets it whenever it sets [`Fauna::set_policy_driven`]; a loader reads it
    /// to decide whether the centre it was handed is the one this world remembers.
    pub fn set_policy_digest(&mut self, founder: Founder, digest: u64) {
        self.policy_digest[founder.index()] = digest;
    }

    /// Forget a lineage's factory. Founders of that kind born afterwards rest.
    pub fn clear_founder_factory(&mut self, founder: Founder) {
        self.factories.clear(founder);
    }

    /// Whether a lineage has a birth factory registered.
    pub fn has_founder_factory(&self, founder: Founder) -> bool {
        self.factories.has(founder)
    }

    /// A fresh controller of a lineage's registered kind, installed on an existing body.
    /// `false` when the body does not exist or the lineage has no factory. This is how a
    /// driver re-installs controllers over a whole layer — after a load, or after
    /// changing what a lineage is driven by.
    pub fn install_founder_controller(&mut self, animal_id: u64, founder: Founder) -> bool {
        let Some(controller) = self.factories.make(founder) else {
            return false;
        };
        self.set_controller(animal_id, controller)
    }

    pub fn config(&self) -> &FaunaConfig {
        &self.config
    }
    /// Whether paid births run. Live-world default: `true`.
    pub fn births_enabled(&self) -> bool {
        self.births_enabled
    }

    /// Turn births on or off. An isolated arena turns them off; the live world never does.
    pub fn set_births_enabled(&mut self, enabled: bool) {
        self.births_enabled = enabled;
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn view(&self) -> FaunaView<'_> {
        FaunaView {
            config: &self.config,
            tick: self.tick,
            animals: &self.animals,
            clutches: &self.clutches,
            ledger: &self.ledger,
        }
    }

    /// Advance one tick against the world and the plant layer as they stand. Call **after**
    /// `World::step` and after `Flora::step`: the plant layer reads the water the core has
    /// just moved and grows, and then the animals eat what is there. The order within the
    /// tick is [`step`]'s module doc, and it is written down in exactly one place.
    pub fn step(&mut self, world: &World, flora: &mut Flora) {
        step::step(self, world, flora);
    }

    /// [`Fauna::step`], taking the host's thread count. The animal tick has no parallel
    /// phase, so `threads` is not read; the signature matches the other layers' `step_with`.
    pub fn step_with(&mut self, world: &World, flora: &mut Flora, threads: usize) {
        let _ = threads;
        step::step(self, world, flora);
    }

    /// The **static arena's** stepping: [`Fauna::step_with`] with the arena's per-arena
    /// [`Senses`] handle threaded into the tick, so the controller stage reads the settled
    /// litter field its caller prepared (`design/voxel-senses-phase1-plan.md`, "Initial cue
    /// field settings"). The field updates itself inside the tick at its own cadence
    /// ([`UPDATE_TICKS`]); a caller never calls this for the live world. [`Fauna::step`] and
    /// [`Fauna::step_with`] keep the live signature and stay senses-free.
    pub fn step_with_senses(
        &mut self,
        world: &World,
        flora: &mut Flora,
        threads: usize,
        senses: &mut Senses,
    ) {
        let _ = threads;
        step::step_with_senses(self, world, flora, senses);
    }

    /// Apply a command now, between ticks, the way the plant layer's are applied. Returns
    /// whether it was accepted.
    pub fn apply(&mut self, world: &World, command: Command) -> bool {
        match command {
            Command::IntroduceFounder {
                x,
                z,
                founder,
                stores,
                heading_rad,
            } => {
                let core = self.config.founder(founder).core;
                let Some((body, reserve)) = stores.resolve(&core) else {
                    return false;
                };
                let view = world.view();
                let Some(site) = cubarium_voxel_flora::highest_support(&view, x, z) else {
                    return false;
                };
                self.introduce_body(
                    world,
                    site,
                    Species::Frondgrazer,
                    Some(founder),
                    body,
                    Some(reserve),
                    heading_rad,
                )
            }
            Command::IntroduceFounderOnFace {
                site,
                founder,
                stores,
                heading_rad,
            } => {
                let view = world.view();
                let c = view.config;
                if site.x >= c.width
                    || site.z >= c.depth
                    || site.y + 1 >= c.height
                    || !view.is_support(i64::from(site.x), site.y, site.z)
                {
                    return false;
                }
                let core = self.config.founder(founder).core;
                let Some((body, reserve)) = stores.resolve(&core) else {
                    return false;
                };
                self.introduce_body(
                    world,
                    site,
                    Species::Frondgrazer,
                    Some(founder),
                    body,
                    Some(reserve),
                    heading_rad,
                )
            }
            Command::BuryClutch {
                site,
                lineage,
                eggs,
            } => self.bury_clutch(world, site, lineage, eggs),
            Command::Remove { x, z } => {
                let view = world.view();
                let Some(site) = cubarium_voxel_flora::highest_support(&view, x, z) else {
                    return false;
                };
                let before = self.animals.len();
                let mut removed = Vec::new();
                // A rider is not removed with its host: it is freed (parasite §5).
                self.animals.retain(|a| {
                    if a.site == site && a.attached.is_none() {
                        removed.push(*a);
                        false
                    } else {
                        true
                    }
                });
                for a in removed {
                    // The body is gone, so its controller is too: ids are never reused,
                    // and a table that only grows is a leak in a world that breeds.
                    self.controllers.take(a.id);
                    self.book_removed(&a);
                    self.free_riders(&a);
                }
                self.animals.len() != before
            }
        }
    }

    /// [`Command::BuryClutch`]'s placement: `eggs` dormant eggs, one clutch each.
    fn bury_clutch(&mut self, world: &World, site: Site, lineage: Founder, eggs: u32) -> bool {
        let view = world.view();
        let c = view.config;
        if eggs == 0
            || site.x >= c.width
            || site.z >= c.depth
            || site.y + 1 >= c.height
            || !view.is_support(i64::from(site.x), site.y, site.z)
        {
            return false;
        }
        let sc = self.config.founder(lineage).core;
        let rule = sc.reproduction;
        if !(rule.mode == BirthMode::Eggs && rule.dormant && rule.egg_organic > 0.0) {
            return false;
        }
        for _ in 0..eggs {
            let organic = rule.egg_organic;
            let mineral = sc.n_tissue * organic;
            let energy = sc.energy_density * organic;
            let serial = self.ledger.eggs_laid + self.ledger.eggs_buried;
            self.clutches.push(Clutch {
                site,
                count: 1,
                organic,
                mineral,
                energy,
                laid_tick: self.tick,
                lineage,
                dormancy: Some(Dormancy::draw(&self.config.predation, c.seed, serial, self.tick)),
            });
            self.ledger.eggs_buried += 1;
            self.ledger.introduced_organic_in += organic;
            self.ledger.introduced_mineral_in += mineral;
            self.ledger.introduced_energy_in += energy;
        }
        true
    }

    /// The placement itself. `reserve` `None` is the full reserve `body` can carry;
    /// `Some(r)` is an explicit starting reserve.
    #[allow(clippy::too_many_arguments)]
    fn introduce_body(
        &mut self,
        world: &World,
        site: Site,
        species: Species,
        founder: Option<Founder>,
        body: f64,
        reserve: Option<f64>,
        heading_rad: f64,
    ) -> bool {
        let view = world.view();
        // A founder body is introduced under its **own** physiology: thresholds,
        // densities and tissue mineral content come from the founder table, not from
        // the placeholder species it shares a [`Species`] tag with.
        let sc = match founder {
            Some(f) => self.config.founder(f).core,
            None => *self.config.species(species),
        };
        if !(body.is_finite() && body >= sc.body_min) {
            return false;
        }
        if !heading_rad.is_finite() {
            return false;
        }
        let reserve = reserve.unwrap_or_else(|| sc.reserve_of(body));
        if !(reserve.is_finite() && (0.0..=sc.reserve_of(body)).contains(&reserve)) {
            return false;
        }
        let organic = body + reserve;
        let pose = Pose {
            heading_rad,
            ..Pose::at_site(site, view.config.voxel_m)
        };
        let animal = Animal {
            id: self.ledger.births,
            species,
            site,
            pose,
            founder,
            founder_state: FounderState::default(),
            reproduction: ReproductionState::default(),
            body,
            reserve,
            mineral: sc.n_tissue * organic,
            energy: sc.energy_density * organic,
            age_ticks: 0,
            // An introduced body arrives part way through a life, so a founding cohort
            // does not age out together.
            lifespan_ticks: draw_lifespan_ticks(
                &self.config.life,
                sc.lifespan_median_s,
                self.ledger.births,
                lifespan_salt(site, self.tick),
                true,
            ),
            gut: Taken::default(),
            state: State::Resting,
            mobility: crate::Mobility::default(),
            wound: 0.0,
            strike: None,
            rest: None,
            // It arrives still: it has not moved.
            still_ticks: self.config.predation.still_after_ticks(),
            speed_m_s: 0.0,
            alert_ticks: 0,
            attached: None,
        };
        self.ledger.births += 1;
        self.ledger.introduced += 1;
        self.ledger.introduced_organic_in += organic;
        self.ledger.introduced_mineral_in += animal.mineral;
        self.ledger.introduced_energy_in += animal.energy;
        self.insert(animal);
        true
    }

    /// Save as postcard bytes behind this layer's own schema tag. A different tag is
    /// refused by [`Fauna::load`]; there is no migration, ever
    /// (`always-fresh-never-migrate`).
    pub fn save(&self) -> Vec<u8> {
        snapshot::encode(self)
    }

    /// Read a layer back. Refuses another schema tag, corrupt bytes, and a config no
    /// animal could live under.
    pub fn load(bytes: &[u8]) -> anyhow::Result<Fauna> {
        snapshot::decode(bytes)
    }

    /// Insert keeping the id order. An id is never reused, so this is always an insert.
    fn insert(&mut self, animal: Animal) {
        match self.animals.binary_search_by_key(&animal.id, |a| a.id) {
            Ok(_) => unreachable!("an animal id is never reused"),
            Err(at) => self.animals.insert(at, animal),
        }
    }

    /// One withdrawal on the boundary: the three `eaten_*` flows, and the same
    /// [`Taken`] back, so what this ledger books and what the animal received are one
    /// value and cannot drift from the plant layer's `consumed_*`.
    fn book_eaten(&mut self, taken: Taken, food: Food) -> Taken {
        self.ledger.eaten_organic_in += taken.organic;
        self.ledger.eaten_mineral_in += taken.mineral;
        self.ledger.eaten_energy_in += taken.energy;
        self.ledger.bites += 1;
        // The same addend, at the same point, into the per-class split: so
        // `sum(eaten_by_food) == eaten_organic_in` and `sum(bites_by_food) == bites`
        // hold for every run, the way the respiration splits do.
        self.ledger.bites_by_food[food.index()] += 1;
        self.ledger.eaten_by_food[food.index()] += taken.organic;
        taken
    }

    /// Material out of this layer and into the plant layer's pools, booked whichever way
    /// it went.
    ///
    /// A triplet whose three numbers are all zero is not a deposit and the plant layer
    /// refuses it; there is nothing to lose and nothing is booked. A triplet it refuses
    /// for any *other* reason would otherwise strand material on a dead animal, so it is
    /// booked out as `removed_*` instead — this layer never keeps an inert remainder.
    fn book_deposit(&mut self, flora: &mut Flora, site: Site, kind: DepositKind, d: Taken) -> bool {
        if d.organic <= 0.0 && d.mineral <= 0.0 && d.energy <= 0.0 {
            return false;
        }
        let deposit = Deposit {
            kind,
            organic: d.organic.max(0.0),
            mineral: d.mineral.max(0.0),
            energy: d.energy.max(0.0),
        };
        if flora.deposit(site, deposit) {
            self.ledger.deposited_organic_out += deposit.organic;
            self.ledger.deposited_mineral_out += deposit.mineral;
            self.ledger.deposited_energy_out += deposit.energy;
            true
        } else {
            self.ledger.removed_organic_out += deposit.organic;
            self.ledger.removed_mineral_out += deposit.mineral;
            self.ledger.removed_energy_out += deposit.energy;
            false
        }
    }

    /// An animal leaving the world without a corpse takes **everything it holds**, the
    /// escrowed offspring included: an unborn package is not left behind on an empty
    /// face, and it is not a death, so it is not carrion either.
    fn book_removed(&mut self, a: &Animal) {
        self.ledger.removed_organic_out += a.stored_organic();
        self.ledger.removed_mineral_out += a.stored_mineral();
        self.ledger.removed_energy_out += a.stored_energy();
        self.book_departure(a, Departure::Removed);
    }

    /// A departed host's riders, all of them, whatever their size, let go where it last
    /// stood (parasite hatchlings §5), booked `riders_freed`. Every path a body leaves the
    /// world by calls this once the body is out of `animals`.
    pub(crate) fn free_riders(&mut self, host: &Animal) {
        for rider in self
            .animals
            .iter_mut()
            .filter(|a| a.attached == Some(Attachment { host: host.id }))
        {
            release(rider, host);
            self.ledger.riders_freed += 1;
        }
    }

    /// The one place a departure's cause is counted, so the totals and the per-lineage
    /// split cannot drift: the same body books the same addend into both at the same
    /// point, exactly as [`FaunaLedger::respired_out`]'s three splits do.
    pub(crate) fn book_departure(&mut self, a: &Animal, cause: Departure) {
        self.ledger.deaths_by_cause[cause.index()] += 1;
        if let Some(founder) = a.founder {
            self.ledger.deaths_by_founder_cause[founder.index()][cause.index()] += 1;
        }
    }
}

#[cfg(test)]
mod starting_stores_tests {
    use super::*;

    /// The fractions mean what they say against a founder's own physiology, and a body
    /// that could not live is refused rather than clamped up to `body_min`.
    #[test]
    fn starting_stores_resolve_against_the_founders_own_physiology() {
        for founder in Founder::ALL {
            let sc = FounderPhysiology::frozen(founder).core;

            let (body, reserve) = StartingStores::FULL.resolve(&sc).expect("full is valid");
            assert_eq!(body, sc.body_max);
            assert_eq!(reserve, sc.reserve_of(sc.body_max), "a full reserve");

            let (body, reserve) = StartingStores::HUNGRY
                .resolve(&sc)
                .expect("hungry is valid");
            assert_eq!(body, 0.5 * sc.body_max);
            assert_eq!(reserve, 0.0, "hungry means no reserve at all");
            // The headroom that buys: a whole body_max of organic matter to eat into (at
            // the grazing lineages' reserve cap of 0.5; the lanternjaw's is 1.0).
            let full = sc.body_max + sc.reserve_of(sc.body_max);
            assert!(
                founder == Founder::Lanternjaw ||
                ((full - body) - sc.body_max).abs() < 1e-15,
                "{founder:?}: headroom {} is not one body_max",
                full - body
            );

            // Below body_min, out of range, and non-finite are all refusals.
            let too_small = StartingStores {
                body: 0.5 * sc.body_min / sc.body_max,
                reserve: 0.0,
            };
            assert!(too_small.resolve(&sc).is_none(), "a body under body_min");
            for bad in [
                StartingStores {
                    body: 1.5,
                    reserve: 0.0,
                },
                StartingStores {
                    body: -0.1,
                    reserve: 0.0,
                },
                StartingStores {
                    body: 1.0,
                    reserve: 1.5,
                },
                StartingStores {
                    body: 1.0,
                    reserve: -0.5,
                },
                StartingStores {
                    body: f64::NAN,
                    reserve: 0.0,
                },
                StartingStores {
                    body: 1.0,
                    reserve: f64::INFINITY,
                },
            ] {
                assert!(bad.resolve(&sc).is_none(), "{bad:?} must be refused");
            }
        }
    }
}

#[cfg(test)]
mod cone_census_tests {
    use super::*;
    use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material};
    use cubarium_voxel_flora::{Command as FloraCommand, FloraConfig, Species as Plant};

    /// A 16 × 8 × 8 world at 0.25 m with soil to y = 2, a rock step, a pool of free
    /// water and four stands — so the census has a terrain hit, a water hit, a stripped
    /// crown and a standing crown to tell apart in the same run.
    fn wall_and_water_world() -> (World, Flora, Fauna) {
        let mut world = World::empty(VoxelConfig {
            width: 16,
            height: 8,
            depth: 8,
            voxel_m: 0.25,
            ..VoxelConfig::default()
        });
        for z in 0..8 {
            for x in 0..16 {
                for y in 1..=2 {
                    world.apply(WorldCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        // A rock step two columns east of the middle: a terrain first hit.
        for z in 4..=6 {
            world.apply(WorldCommand::SetMaterial {
                x: 11,
                y: 3,
                z,
                material: Material::Rock,
            });
        }
        // A puddle two columns west: a water first hit at the eye's own layer is what a
        // shower film looks like to the ray.
        for z in 2..=4u32 {
            world.apply(WorldCommand::AddWater {
                x: 5,
                y: 3,
                z,
                volume_m3: 0.01,
            });
        }
        let mut flora = Flora::new(FloraConfig::default());
        for (x, z, species) in [
            (9u32, 2u32, Plant::Springturf),
            (8, 6, Plant::Springturf),
            (12, 3, Plant::Stonecushion),
            (6, 5, Plant::Stonecushion),
        ] {
            let wood = 0.5 * flora.config().species(species).wood_max;
            flora.apply(
                &world,
                FloraCommand::Seed {
                    x: i64::from(x),
                    z,
                    species,
                    wood,
                },
            );
        }
        let mut fauna = Fauna::new(FaunaConfig::default());
        for (x, z, heading) in [
            (8u32, 4u32, 0.0),
            (9, 4, std::f64::consts::FRAC_PI_2),
            (7, 3, std::f64::consts::PI),
        ] {
            assert!(fauna.apply(
                &world,
                Command::IntroduceFounder {
                    x: i64::from(x),
                    z,
                    founder: Founder::Browser,
                    stores: StartingStores::FULL,
                    heading_rad: heading,
                },
            ));
        }
        (world, flora, fauna)
    }

    /// **The census is the reading.** Deliverable 2's contract: aggregating the fine
    /// per-ray classes back through `Fine::coarse` reproduces the policy's own
    /// `foliage_fraction` and `foliage_proximity` bit for bit, on a world that is
    /// changing under the bodies — stands cropped, water moving, three browsers walking
    /// into and out of each other's cones.
    #[test]
    fn the_fine_census_maps_onto_the_coarse_reading_exactly() {
        let (mut world, mut flora, mut fauna) = wall_and_water_world();
        let mut checked = 0usize;
        let mut saw = [0usize; ConeHit::ALL.len()];
        for tick in 0..=200u64 {
            if tick > 0 {
                world.step();
                fauna.step(&world, &mut flora);
            }
            if tick % 20 != 0 {
                continue;
            }
            let view = world.view();
            let fv = flora.view();
            let av = fauna.view();
            for animal in av.animals {
                let Some(census) = browser_cone_census(&view, &fv, &av, animal, &[]) else {
                    continue;
                };
                let reading = browser_cone_readings(&view, &fv, &av, animal).expect("a valid cone");
                let rays_per_sector = (Founder::Browser.manifest().ray_yaw_offsets_deg.len()
                    * Founder::Browser.manifest().ray_pitch_offsets_deg.len())
                    as f64;
                for (sector, (fraction, proximity)) in reading.into_iter().enumerate() {
                    let counts = census.counts(sector);
                    for (i, n) in counts.iter().enumerate() {
                        saw[i] += *n as usize;
                    }
                    let foliage: Vec<f64> = census
                        .rays
                        .iter()
                        .filter(|r| r.sector == sector && r.hit == ConeHit::FoliageCrown)
                        .map(|r| (1.0 - r.distance_m / census.range_m).clamp(0.0, 1.0))
                        .collect();
                    assert_eq!(
                        f64::from(counts[ConeHit::FoliageCrown.index()]) / rays_per_sector,
                        fraction,
                        "tick {tick} body {} sector {sector}: the census's foliage count \
                         is not the reading's fraction",
                        animal.id,
                    );
                    let want = if foliage.is_empty() {
                        0.0
                    } else {
                        foliage.iter().sum::<f64>() / foliage.len() as f64
                    };
                    assert_eq!(
                        want, proximity,
                        "tick {tick} body {} sector {sector}: the census's foliage \
                         distances are not the reading's proximity",
                        animal.id,
                    );
                    assert_eq!(
                        counts.iter().sum::<u32>() as f64,
                        rays_per_sector,
                        "every ray of the sector is classed exactly once"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 0, "the test never sampled a cone");
        // The fixture has to actually exercise the classes the coarse reading hides, or
        // the assertion above is vacuous for them.
        for hit in [
            ConeHit::Clear,
            ConeHit::Terrain,
            ConeHit::Water,
            ConeHit::FoliageCrown,
        ] {
            assert!(
                saw[hit.index()] > 0,
                "the fixture never produced a {} first hit",
                hit.name()
            );
        }
    }

    /// A probe bearing is marched by the same ray as the fan: aimed down the heading
    /// with no pitch, it agrees with the fan's own centre-sector, centre-offset ray.
    #[test]
    fn a_probe_bearing_is_the_same_ray_as_the_fan() {
        let (world, flora, fauna) = wall_and_water_world();
        let view = world.view();
        let fv = flora.view();
        let av = fauna.view();
        let animal = av.animals.first().expect("a founder was introduced");
        let census =
            browser_cone_census(&view, &fv, &av, animal, &[(0.0, 0.0)]).expect("a valid cone");
        let straight = census
            .rays
            .iter()
            .find(|r| r.sector == 1 && r.yaw_deg == 0.0 && r.pitch_deg == 0.0)
            .expect("the fan has a ray dead ahead");
        let probe = census.probes.first().expect("one probe was asked for");
        assert_eq!(
            (probe.hit, probe.distance_m),
            (straight.hit, straight.distance_m)
        );
    }
}
