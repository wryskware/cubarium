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

#[cfg(test)]
mod mass_rescale_tests;

mod body;
mod controller;
mod encounter;
mod knobs;
mod hunt;
mod manifest;
pub(crate) mod observe_v3;
mod pose;
mod predation;
mod pack;
pub mod perception;
pub mod scent;
pub mod light;
pub use light::{Beacon, BeaconKind};
pub mod vigil;
pub use perception::{Percept, Track};
pub use vigil::{Hunker, Vigil, VigilLedger, VigilMode, Vigilance};
pub mod herding;
pub use herding::{HerdTally, Herding};
#[cfg(test)]
mod herding_tests;
mod gait;
pub use gait::Sprint;
pub mod rhythm;
pub use rhythm::{Cadence, Horizon, Pace, Rhythm};
pub use step::jump::DropRule;
pub use step::forage::ForageTally;
pub use scent::ScentChannel;
mod par;
pub use pack::centre as pack_centre;
mod registry;
pub use registry::registry;
mod senses;
mod snapshot;
mod step;
#[cfg(test)]
mod testpass3_support;
#[cfg(test)]
mod testpass3_state_tests;
pub use step::shore_egg_site;

#[cfg(test)]
mod cone_speed_tests;
#[cfg(test)]
mod descent_tests;
#[cfg(test)]
mod hot_path_tests;
#[cfg(test)]
mod herd_tests;
#[cfg(test)]
mod observe_v3_tests;
#[cfg(test)]
mod tap_tests;
#[cfg(test)]
mod testpass4_herd_tests;
#[cfg(test)]
mod roster_tests;
#[cfg(test)]
mod roster_mechanics_tests;
#[cfg(test)]
mod snail_tests;
#[cfg(test)]
mod snail_land_tests;
#[cfg(test)]
mod flight_tests;
#[cfg(test)]
mod seedporter_tests;
#[cfg(test)]
mod roost_rain_tests;
mod flight;
#[cfg(test)]
mod slide_tests;

use cubarium_voxel::{VoxelView, World};
use rustc_hash::FxHashMap;
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraView, Site, Taken};
use serde::{Deserialize, Serialize};

pub use body::{
    Amphibious, BELLWING_MASS_RATIO, Body, CHORISTER_VOLUME_RATIO, Counter, EggSite, Face, Flight, FounderPhysiology, Founding, FoundingSite, Glider, Gut, GutSeeds, Hunter, Pack, SenseTable, StepLimits, birth_readiness,
    disc_clear_on_face, effective_config, has_headroom, motor_probe, step_voxels, wall_ascent, wall_descent,
};
mod den;
pub use den::{DenQuality, den_quality};

pub use controller::{
    Actions, BlindForager, BrowserForager, Controller, ControllerFactory, FounderControllers,
    FounderFactories, FruitForager, HoverSip, LanternjawAmbush, LanternjawPatrol, PackHunter, Response, Scripted, Seek, heuristic, resolve_actions,
    BodyActions, Resolved, resolve,
};
pub use predation::{
    BodyKind, Dormancy, Hit, Rest, Strike, StrikePhase, apply_hit, dodge_chance, dodge_chance_with, dodges,
    egg_hatches, hatch_draw, is_prey_of, kind_of, moved_recently, tremor_of,
};
pub use cubarium_voxel::{DT, TICK_HZ};
pub use encounter::{
    DropWay, HEADING_SAMPLES, RouteGraph, RouteMap, RouteRule, SightMap, band_crown_layers, band_layers, crown_columns, crown_layer,
    crown_slab_m, eye_above_surface_m, eye_origin_m, foliage_stands_in_layers, layer_columns,
    mouth_columns_at, mouth_columns_from_face, mouth_crown_layers_at, ray_direction_deg,
    reachable_layers_of, route_components, standable_faces, surface_m, walkable_components,
    founding_faces,
};
pub use manifest::{
    ACTION_DEADBAND, Action, BIRTH_READINESS_RULE, BROWSER_RAY_PITCH_OFFSETS_DEG,
    BROWSER_RAY_YAW_OFFSETS_DEG, BROWSER_SECTOR_CENTRES_DEG, BROWSER_VISIBLE_CLASSES,
    Founder, cruise_body_lengths_per_s, HIDDEN, Manifest, Module, OCCLUSION_RULE, SCHEMA_VERSION,
    Transfer, Tunings, gru32_parameter_count, gru_parameter_count, MAX_OUTPUTS, V3_ENCODING_RULE,
    BLIND_INPUTS, BLIND_OUTPUTS, BROWSER_INPUTS, BROWSER_OUTPUTS, SCENT_REFERENCE, TREMOR_REFERENCE,
    HEIGHT_REFERENCE_M,
};
pub use knobs::{CueConfig, FoundingConfig, HuntConfig, LifeConfig, LightConfig, PerceptionConfig, PredationConfig, ScentConfig, TeacherConfig};
pub use hunt::{FruitlessPatch, HuntMemory, HuntMode, accepts, attack_chance, attack_preference, encounter_share, profitability, FRUITLESS_SLOTS};
pub use pose::Pose;
pub use flight::{flyer_fits_on_face, flyer_sheltered};
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
    /// **A bloom** (F1, predation-and-flight §3.6): nectar that can be taken — a
    /// latticevine face in flower, a bloomcrown core with nectar. A flyer's fan class.
    // F1 stub: declared; no ray reports it yet.
    Bloom,
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
            ConeHit::Bloom => "bloom",
        }
    }

    /// Every variant, in the order a census row prints them.
    pub const ALL: [ConeHit; 9] = [
        ConeHit::Clear,
        ConeHit::Terrain,
        ConeHit::Water,
        ConeHit::StrippedCrown,
        ConeHit::Trunk,
        ConeHit::FoliageCrown,
        ConeHit::GroundPool,
        ConeHit::Body,
        ConeHit::Bloom,
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
            ConeHit::Bloom => 8,
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

/// **A flyer's fan, ray by ray** (F1, predation-and-flight §3.6): the downward fan from
/// the body's eye at `y_m`, every first hit named, [`ConeHit::Bloom`] among them, by the
/// same march the flyer's observation reads. `probes` as [`browser_cone_census`]'s. `None`
/// for a body that is not a flyer.
pub fn flyer_fan_census(
    view: &VoxelView<'_>,
    flora: &FloraView<'_>,
    fauna: &FaunaView<'_>,
    animal: &Animal,
    probes: &[(f64, f64)],
) -> Option<ConeCensus> {
    let founder = animal.founder?;
    let phys = fauna.config.founder(founder);
    phys.flight?;
    let manifest = fauna.config.manifest(founder);
    let occupancy = senses::cone_occupancy(view, flora, fauna);
    let blooms = flight::Blooms::of(flora);
    let geometry = phys.body_at(animal.body);
    let origin = flight::eye(animal, &geometry);
    let heading = animal.pose.heading_rad;
    let range = manifest.cone_range_m;
    let march = |sector: usize, yaw_deg: f64, pitch_deg: f64, dir: (f64, f64, f64)| {
        let (hit, distance_m) =
            match flight::fan_first_hit(view, flora, &blooms, &occupancy, animal.id, origin, dir, range) {
                None => (ConeHit::Clear, f64::INFINITY),
                Some((d, hit)) => (hit, d),
            };
        ConeRay {
            sector,
            yaw_deg,
            pitch_deg,
            hit,
            distance_m,
        }
    };
    let mut rays = Vec::new();
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
    /// **Dark-active** (the detritus web, 2026-10-05, §3.1B): awake while its **own face**
    /// is dark — the face's terrain sky visibility × daylight below
    /// [`LifeConfig::day_line`] — so at night, and by day under an overhang, in a cave or a
    /// hall. Photonegative rather than clock-bound, like a woodlouse. In open sky it is
    /// the nocturnal rhythm; read without a face ([`Activity::awake`]) it is too.
    DarkActive,
    /// **A leaning** (RHYTHMS-CADENCE, Wrysk 2026-10-09): awake by its lineage's
    /// [`crate::Rhythm`], a chance by the time of day with rests in random bouts, read per body
    /// in the circadian stage ([`crate::rhythm`]). Read alone, without a body, it is awake.
    Leaning,
}

impl Activity {
    /// Whether this rhythm is awake at `daylight` (0 night, 1 full day), the day's lines
    /// being `life`'s. With the clock off (`clock_on` false) every rhythm is awake.
    pub fn awake(self, clock_on: bool, daylight: f64, life: &LifeConfig) -> bool {
        self.awake_under(clock_on, daylight, 1.0, life)
    }

    /// [`Activity::awake`] on a face whose terrain sky visibility is `sky` (0 closed, 1
    /// open): only [`Activity::DarkActive`] reads it, as the light at the face,
    /// `sky × daylight`.
    pub fn awake_under(self, clock_on: bool, daylight: f64, sky: f64, life: &LifeConfig) -> bool {
        if !clock_on {
            return true;
        }
        match self {
            Activity::Always => true,
            Activity::Diurnal => daylight > life.day_line,
            Activity::Nocturnal => daylight < life.day_line,
            Activity::Crepuscular => daylight > life.twilight_lo && daylight < life.twilight_hi,
            Activity::DarkActive => sky.clamp(0.0, 1.0) * daylight < life.day_line,
            Activity::Leaning => true,
        }
    }

    /// Whether this rhythm reads the light at its face ([`Activity::awake_under`]'s `sky`).
    pub fn reads_face_light(self) -> bool {
        self == Activity::DarkActive
    }

    /// **The rhythm with its lineage's window** (LURE, Wrysk 2026-10-06): awake when
    /// [`Activity::awake_under`] says so, or, with the clock on, inside `window` of the night's
    /// ends ([`ActiveWindow::holds`]). A zero window is the rhythm alone.
    pub fn awake_in(
        self,
        clock_on: bool,
        daylight: f64,
        sky: f64,
        night: NightClock,
        window: ActiveWindow,
        life: &LifeConfig,
    ) -> bool {
        self.awake_under(clock_on, daylight, sky, life) || (clock_on && window.holds(night))
    }
}

/// **Where in the night it is** (LURE): hours since sunset and hours to sunrise on the day's
/// 24-hour clock while the sun is down, both infinite while it is up. Read from the world's
/// sun each tick ([`NightClock::of`]); derived, never saved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NightClock {
    pub since_set_h: f64,
    pub to_rise_h: f64,
}

impl Default for NightClock {
    fn default() -> NightClock {
        NightClock::DAY
    }
}

impl NightClock {
    /// The sun is up (or the clock is off): no night end is near.
    pub const DAY: NightClock = NightClock { since_set_h: f64::INFINITY, to_rise_h: f64::INFINITY };

    /// The night clock of `sun`: from its phase, sunrise and sunset.
    pub fn of(sun: &cubarium_voxel::weather::Sun) -> NightClock {
        let day = (sun.set - sun.rise).rem_euclid(1.0);
        let since_rise = (sun.phase - sun.rise).rem_euclid(1.0);
        if since_rise <= day {
            return NightClock::DAY;
        }
        NightClock {
            since_set_h: 24.0 * (sun.phase - sun.set).rem_euclid(1.0),
            to_rise_h: 24.0 * (sun.rise - sun.phase).rem_euclid(1.0),
        }
    }
}

/// **A lineage's activity window** (LURE, Wrysk 2026-10-06: "dusk/dawn is fine for them to
/// fly as well as day"): how long, in hours on the day's 24-hour clock, it stays awake after
/// sunset and wakes before sunrise on top of its [`Activity`] — a twilight flyer's dusk and
/// dawn flights. Zero (the default) adds nothing. The bellwing's is 1 h each side: crepuscular
/// moths and twilight bees fly in the hour or so around sunset and sunrise (module notes at
/// the bellwing preset).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ActiveWindow {
    /// Hours before sunrise it is awake.
    pub before_rise_h: f64,
    /// Hours after sunset it is awake.
    pub after_set_h: f64,
}

impl ActiveWindow {
    /// Whether `night` is inside this window: strictly under `after_set_h` since sunset or
    /// `before_rise_h` to sunrise.
    pub fn holds(self, night: NightClock) -> bool {
        night.since_set_h < self.after_set_h || night.to_rise_h < self.before_rise_h
    }
}

/// A body's movement state that outlives a tick
/// (`design/handoffs/voxel-mobility-2026-09-23.md`): the face it is on, if any, and how
/// long it has been under water deeper than its height.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Mobility {
    /// The wall it is climbing, or `None` on the ground.
    pub climb: Option<WallClimb>,
    /// **How it is moving** (F1, predation-and-flight §3.1): on the ground, on a wall, in
    /// the air or at rest off the ground. A lineage enables the modes it has (the bellwing
    /// `Fly` + `Perched`). A walker stays `Ground` (its wall climb is [`Mobility::climb`]).
    #[serde(default)]
    pub mode: MobilityMode,
    /// The perch a `Perched` body holds (§3.4): losing it means a takeoff, or a drop when
    /// it cannot pay (a glider's: a forced glide). A glider's `Climb` holds its stem or
    /// vine face here too (F3). `None` in every other mode.
    #[serde(default)]
    pub perch: Option<Perch>,
    /// A flyer **dropping**: it lost a perch it could not pay to leave (or is asleep), so
    /// it falls straight down at the lift speed, paying nothing aloft, onto the first
    /// perch below. Always `false` for a walker. A **glider** (F3) uses it for a climb
    /// **down** (`Climb`), and for an introduced glide whose perch is not yet resolved
    /// (`Glide`, its first tick).
    #[serde(default)]
    pub falling: bool,
    /// Consecutive ticks under water deeper than its drown depth. It drowns when this
    /// reaches [`FounderPhysiology::drown_ticks`]; any tick not under resets it.
    pub submerged_ticks: u64,
    /// **The drying clock** of an amphibious body ([`crate::Amphibious`]), 0 hydrated, 1
    /// full; it dies of [`Departure::Desiccation`] at the lineage's `lethal_fraction`.
    /// Always 0 for every other body.
    #[serde(default)]
    pub dryness: f64,
    /// Out of water on the last tick (an amphibious body beyond its water margin): it
    /// does not fall asleep there.
    #[serde(default)]
    pub ashore: bool,
    /// Ticks left of a retraction ([`crate::Amphibious::retract_hold_s`]): stopped and
    /// closed.
    #[serde(default)]
    pub retract_ticks: u32,
    /// Its drying clock is past the return fraction of the hour (night or rain, or dry
    /// day): it is held to the water-bound rule and its heuristic heads for water.
    #[serde(default)]
    pub thirsty: bool,
    /// **Grounded by rain** (a flyer; backlog §12): set when the rain reaches its
    /// [`crate::Flight::rain_ground_m_per_s`], cleared once it falls under that less the
    /// hysteresis. While set it comes down, does not take off and does not sip. Always
    /// `false` for a body without a flight table.
    #[serde(default)]
    pub rain_grounded: bool,
}

/// A body's locomotion mode ([`Mobility::mode`], predation-and-flight §3.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MobilityMode {
    /// On a support face, walking (today's every body).
    #[default]
    Ground,
    /// On a terrain face, the shredder's climb ([`Mobility::climb`]).
    Wall,
    /// On a plant stem or a latticevine face (the seedporter, F3).
    Climb,
    /// Powered flight: `y_m` moves freely within the band (the bellwing).
    Fly,
    /// Unpowered descent at a glide ratio (the seedporter, F3).
    Glide,
    /// At rest off the ground, or on it, holding a [`Perch`].
    Perched,
}

/// A perch a flyer holds ([`Mobility::perch`], §3.4): what it is, and what holds it up,
/// so a lost perch (a crown eaten, a face uncovered, a ledge dug out) is seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Perch {
    pub kind: PerchKind,
    pub anchor: PerchAnchor,
}

/// **A roost ledge** (backlog §12, Wrysk 2026-10-08: "up top mesas they can reach by
/// climbing latticevine"): one side of one wall column whose latticevine cover runs from
/// the foot to within reach of the lip, with a support face on top. Derived from the
/// terrain and the cover ([`Fauna::ledges`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Ledge {
    /// The lowest covered face on the wall: its column and side.
    pub face: cubarium_voxel_flora::Face,
    /// The support face in front of the wall the climb starts from.
    pub foot: Site,
    /// The support face on top of the wall column: the lip.
    pub top: Site,
    /// The lowest and highest covered rows, and the most uncovered rows between two covered
    /// ones.
    pub low: u32,
    pub high: u32,
    pub gap: u32,
}

/// What holds a perch ([`Perch::anchor`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerchAnchor {
    /// The top of this support face (ground, a ledge): [`PerchKind::Face`].
    Face(Site),
    /// The foliage-bearing layer `layer` (its index among the stand's foliage layers) of
    /// the stand rooted on `site`: its top ([`PerchKind::CrownTop`]) or its underside.
    Stand { site: Site, layer: u8 },
    /// A latticevine-covered face: [`PerchKind::VineFace`].
    Vine(cubarium_voxel_flora::Face),
    /// The solid cell `(x, y, z)` overhead (an overhang's underside).
    Rock { x: u32, y: u32, z: u32 },
}

/// What a perch is (§3.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerchKind {
    /// The top of a support face: ground, a ledge.
    Face,
    /// The top of a stand's crown slab (a bloomcrown core), from the crown cache.
    CrownTop,
    /// A latticevine-covered face: clinging to the wall.
    VineFace,
    /// The underside of a crown slab or an overhang: hanging (the frond-underside roost).
    Underside,
}

impl PerchKind {
    /// Every kind, in the order [`FaunaLedger::landings_by_perch`] counts them.
    pub const ALL: [PerchKind; 4] = [
        PerchKind::Face,
        PerchKind::CrownTop,
        PerchKind::VineFace,
        PerchKind::Underside,
    ];

    /// Its index in [`PerchKind::ALL`].
    pub const fn index(self) -> usize {
        match self {
            PerchKind::Face => 0,
            PerchKind::CrownTop => 1,
            PerchKind::VineFace => 2,
            PerchKind::Underside => 3,
        }
    }

    /// Its name, for a census column.
    pub const fn name(self) -> &'static str {
        match self {
            PerchKind::Face => "face",
            PerchKind::CrownTop => "crown_top",
            PerchKind::VineFace => "vine_face",
            PerchKind::Underside => "underside",
        }
    }
}

/// The last bloom a body sipped from (F1 §3.7's seam, pollen carriage later).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Bloom {
    /// A stand's nectar stock (a bloomcrown), by its site.
    Stand(Site),
    /// A latticevine face in flower.
    Vine(cubarium_voxel_flora::Face),
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
    /// **The last bloom it took pollen from** (F1 §3.7, the pollen seam): the one it last
    /// sipped from, or (SEEDPORTER-POLLEN, a body with
    /// [`FounderPhysiology::pollen_by_contact`]) the vine flower it last brushed against.
    /// `None` until then.
    #[serde(default)]
    pub last_bloom: Option<Bloom>,
    /// **Seeds in the gut** (F3, a lineage with [`FounderPhysiology::gut_seeds`]): each
    /// fruit meal's seeds, by source, carried out of the fruit's organic matter and voided
    /// at the body's face no sooner than `gut_transit_s` after the meal. Counted in the
    /// stored totals and laid with the corpse at death. Empty for every other lineage.
    #[serde(default)]
    pub gut_seeds: GutSeedLots,
    /// **Pack membership** (roster R3, a lineage with [`FounderPhysiology::pack`]): the
    /// pack it belongs to ([`PackRecord`]), whether it is a pup, and its call. `None` for
    /// every body outside a pack.
    #[serde(default)]
    pub pack: Option<PackMember>,
    /// **Walking home to rest** (roster R3, a pack adult driven by its pack heuristic):
    /// ticks it has wanted a rest (satiated, or out of its activity phase) while away from
    /// its den. Rest begins at [`Pack::rest_radius_m`], or in the field if the route is
    /// unreachable or costs more than [`Pack::home_reserve_share`] allows. Zero otherwise.
    #[serde(default)]
    pub homing_ticks: u32,
    /// **What is left of its rest bout**, ticks (RHYTHMS-CADENCE, [`crate::rhythm`]): a body of
    /// a [`Activity::Leaning`] lineage sleeps while this is above 0 (unless kept up), and an
    /// awake one starts a bout at random. Always 0 for every other body and with the clock off.
    #[serde(default)]
    pub rest_ticks: u32,
    /// **Yielding a carcass** (roster R3, the lanternjaw's heuristic): set when a body that
    /// hunts it came within `carcass_yield_radius_m` while it fed, cleared once none is
    /// within `threat_clear_m` ([`crate::PredationConfig`]); while set it walks directly
    /// away. `false` for every other body.
    #[serde(default)]
    pub yielding: bool,
    /// **Hunkered** (HUNKER, [`crate::Hunker`]): clamped down under its shield, neither moving
    /// nor eating. Its state is [`State::Resting`]; the presenter reads this flag to draw the
    /// hunker pose (`cubarium::voxel::hunker`). False for every other body.
    #[serde(default)]
    pub hunkered: bool,
    /// **The stomach** (the minimal gut, `design/metabolism-meals-2026-10-02.md` repair 2
    /// item 10): raw food eaten and not yet digested, per food class, in all three
    /// currencies. Separate from [`Animal::gut`], which is egesta. Counted in the stored
    /// totals and laid with the corpse at death.
    #[serde(default)]
    pub stomach: Stomach,
    /// **The eating latch** (repair 2 item 3): on when the stomach falls to
    /// `s_start · K` in a state that may eat, off only at a full event (`S ≥
    /// full_tolerance · K`). While on, the body seeks food and bites, and a hunter lures,
    /// targets and strikes. Losing access to food does not clear it.
    #[serde(default)]
    pub eating: bool,
    /// **The emergency latch** (repair 2 item 7b): on at reserve hunger `wake_hunger`, off
    /// below [`FounderPhysiology::emergency_clear_hunger`]. While on, the body is awake
    /// in any phase.
    #[serde(default)]
    pub emergency: bool,
    /// The feeding bout in progress: from the eating latch going on to its full event.
    #[serde(default)]
    pub bout: Option<Bout>,
    /// **The patrol's memory** (the detritus web D2, the lanternjaw's heuristic only): the
    /// patch it is walking to or waiting on, the fruitless patches it avoids, and its
    /// search image, the encounter memory per prey lineage. Default for every other body.
    #[serde(default)]
    pub hunt: HuntMemory,
}

/// **The stomach** (repair 2 item 10): one raw-food pool per [`Food`] class, each in all
/// three currencies. A class digests at its own `τ` and yield; a transfer between
/// stomachs moves every pool by the same fraction, so the food keeps its composition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stomach {
    pub pools: [Taken; Food::COUNT],
}

impl Stomach {
    /// Everything the stomach holds, summed over the classes.
    pub fn total(&self) -> Taken {
        let mut t = Taken::default();
        for p in &self.pools {
            t.organic += p.organic;
            t.mineral += p.mineral;
            t.energy += p.energy;
        }
        t
    }

    /// Raw organic matter held, `S`.
    pub fn organic(&self) -> f64 {
        self.pools.iter().map(|p| p.organic).sum()
    }

    /// Whether nothing is held in any currency.
    pub fn is_empty(&self) -> bool {
        self.pools.iter().all(|p| p.organic <= 0.0 && p.mineral <= 0.0 && p.energy <= 0.0)
    }
}

/// What kind of feeding bout a meal was (repair 2 item 8), decided at its close by the
/// strongest reason that held during it: an emergency, then provisioning (it fed young
/// or carried an escrow), then growth (it was a juvenile), else normal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BoutKind {
    Normal,
    Growth,
    Provisioning,
    Emergency,
}

impl BoutKind {
    pub const ALL: [BoutKind; 4] = [
        BoutKind::Normal,
        BoutKind::Emergency,
        BoutKind::Growth,
        BoutKind::Provisioning,
    ];
    pub const COUNT: usize = 4;

    /// Its index in [`BoutKind::ALL`] (the ledger's order).
    pub const fn index(self) -> usize {
        match self {
            BoutKind::Normal => 0,
            BoutKind::Emergency => 1,
            BoutKind::Growth => 2,
            BoutKind::Provisioning => 3,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            BoutKind::Normal => "normal",
            BoutKind::Emergency => "emergency",
            BoutKind::Growth => "growth",
            BoutKind::Provisioning => "provisioning",
        }
    }
}

/// A feeding bout in progress ([`Animal::bout`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bout {
    /// The tick the eating latch went on.
    pub start_tick: u64,
    /// The strongest kind it has been so far ([`BoutKind`]'s order).
    pub kind: BoutKind,
}

/// A body's place in a pack (roster R3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackMember {
    /// The pack's id ([`PackRecord::id`]).
    pub pack: u64,
    /// **A pup**: born into the pack and younger than its lineage's
    /// [`Pack::independence_age_s`]. It cannot strike and stays within
    /// [`Pack::den_radius_m`] of the den; at its independence age this clears and it joins
    /// the pack's hunts. A founded adult is never a pup.
    pub pup: bool,
    /// Ticks its **call** has left: set to [`Pack::call_s`] when the pack target changes or
    /// it strikes, counted down each tick. Saved, and read by the presenter.
    pub calling_ticks: u32,
}

impl PackMember {
    /// Whether it is calling now.
    pub fn calling(&self) -> bool {
        self.calling_ticks > 0
    }
}

/// The shelter of a pack's den, chosen at founding or after it becomes unusable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DenChoice {
    /// A covered face (solid within [`Pack::den_cover_m`] over it) within
    /// [`Pack::den_search_m`] of its range; darker sites beat brighter ones.
    Covered,
    /// A roof higher than `den_cover_m`; darkness and sky visibility choose the site.
    MostOverhead,
    /// Open ground: no reachable dry roofed site in the search range.
    FoundingPoint,
}

/// An unbounded distance (a serde default).
fn far() -> f64 {
    f64::INFINITY
}

/// A pack (roster R3): its den, its breeder, and the scripted pack tactic's state (its
/// target, its chase clock, its carcass). Saved with the layer.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackRecord {
    /// Its id, never reused.
    pub id: u64,
    /// The lineage whose pack this is.
    pub lineage: Founder,
    /// The den's dry reachable support face, chosen at founding and replaced when
    /// flooded or disconnected. Dark roofed sites are preferred to open ground.
    pub den: Site,
    /// The one member that conceives; `None` only while no adult is left. When it dies the
    /// oldest adult in the pack takes its place.
    pub breeder: Option<u64>,
    /// How the den was chosen; `None` for a pack founded on a given face
    /// ([`Command::FoundPack`] directly).
    #[serde(default)]
    pub den_choice: Option<DenChoice>,
    /// The tick its last litter was born: the next is conceived no sooner than
    /// [`Pack::litter_interval_s`] after it.
    #[serde(default)]
    pub last_litter_tick: Option<u64>,
    /// **The pack target**: the prey every adult member is driving or flanking now.
    #[serde(default)]
    pub target: Option<u64>,
    /// The tick the chase on `target` began.
    #[serde(default)]
    pub chase_since_tick: u64,
    /// **CHURN, the chase's progress** ([`Pack::chase_give_up_s`]): the last tick the pack
    /// made progress on `target`, its nearest member's distance to it so far, and the work
    /// done on it (its wound plus its pry, plus 1 while it is turned over) last read.
    #[serde(default)]
    pub chase_progress_tick: u64,
    #[serde(default = "far")]
    pub chase_best_m: f64,
    #[serde(default)]
    pub chase_work: f64,
    /// The target's face when last seen: where its carcass lies if it was killed.
    #[serde(default)]
    pub target_site: Option<Site>,
    /// **The pack's carcass**: the face of a kill it is eating, until the carcass there is
    /// gone (no carrion, or the plant layer's carcass record cleared at its
    /// `carcass_gone_fraction`). Pups eat at it too when it is within [`Pack::den_radius_m`] of the den.
    #[serde(default)]
    pub feast: Option<Site>,
    /// Carrion the pack's members have eaten at its current feast, organic (the minimal
    /// gut's repair 3 measure): reset when the feast changes.
    #[serde(default)]
    pub feast_eaten: f64,
    /// The last target the pack gave up on (late, spent, or into another territory): not
    /// taken again until the pack has taken another.
    #[serde(default)]
    pub abandoned: Option<u64>,
    /// **Shunned targets** (PACK-TUNE): bodies the pack dropped alive (broken off, late, lost,
    /// spent, trespassing), each with the tick until which it is not taken again
    /// (`Pack::shun_s`); `(0, 0)` is an empty slot. The oldest slot is reused.
    #[serde(default)]
    pub shunned: [(u64, u64); 4],
    /// **The draw** (VEILHOUND-SEARCH, the lineage's [`SenseTable`]): the prey the
    /// pack's leader is walking to by route — sensed by the pack, on its route component,
    /// not yet its target. Remade at each of the leader's samples; cleared when the pack
    /// takes a target.
    #[serde(default)]
    pub drawn: Option<u64>,
    /// **The scent draw** (SENSES-CORE): the leader is walking up its prey's scent, with no
    /// prey sensed on its component. Set and cleared at each of the leader's samples, as
    /// [`PackRecord::drawn`] is.
    #[serde(default)]
    pub drawn_scent: bool,
    /// **The pack's breeding season** (LIFE-HISTORY): the year of its last litter's
    /// conception and the conceptions in it ([`Reproduction::litters_per_year`] per pack).
    #[serde(default)]
    pub season: Season,
}

/// Where a gut seed came from (F3): a stand's fruit (its species, e.g. lanternberry) or a
/// latticevine face's beads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeedPlant {
    Stand(cubarium_voxel_flora::Species),
    Latticevine,
}

impl SeedPlant {
    /// `[stand fruit, latticevine fruit]` order, for the ledger's per-source lines.
    pub const fn source_index(self) -> usize {
        match self {
            SeedPlant::Stand(_) => 0,
            SeedPlant::Latticevine => 1,
        }
    }
}

/// One lot of gut seeds (F3): the seeds of one source plant from meals no further apart
/// than half the transit time, voided together at `due_tick`, which the latest meal in the
/// lot sets (so no meal's seeds leave sooner than the transit time after it).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GutSeedLot {
    /// `None` for an empty slot.
    pub plant: Option<SeedPlant>,
    pub organic: f64,
    pub mineral: f64,
    pub energy: f64,
    /// Where the first meal in the lot was eaten: the stand's site, or the vine face's
    /// support below (for the dispersal census).
    pub source: Site,
    pub opened_tick: u64,
    pub due_tick: u64,
}

impl Default for GutSeedLot {
    fn default() -> GutSeedLot {
        GutSeedLot {
            plant: None,
            organic: 0.0,
            mineral: 0.0,
            energy: 0.0,
            source: Site { x: 0, y: 0, z: 0 },
            opened_tick: 0,
            due_tick: 0,
        }
    }
}

/// A body's gut seed lots: [`GutSeedLots::SLOTS`] fixed slots, at most half of them for
/// one plant, so an animal stays a small `Copy` value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GutSeedLots {
    pub lots: [GutSeedLot; GutSeedLots::SLOTS],
}

impl GutSeedLots {
    pub const SLOTS: usize = 6;

    /// Everything the lots hold.
    pub fn total(&self) -> Taken {
        let mut t = Taken::default();
        for l in self.lots.iter().filter(|l| l.plant.is_some()) {
            t.organic += l.organic;
            t.mineral += l.mineral;
            t.energy += l.energy;
        }
        t
    }

    pub fn is_empty(&self) -> bool {
        self.lots.iter().all(|l| l.plant.is_none())
    }
}

/// One void of gut seeds (F3), for a census: what, from where, to where. Not world state;
/// [`Fauna::take_seed_voids`] drains them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeedVoid {
    pub tick: u64,
    pub animal: u64,
    pub plant: SeedPlant,
    pub organic: f64,
    pub source: Site,
    pub landed: Site,
    /// Whether the plant layer banked the seeds (a lanternberry's into the seed bank); a
    /// latticevine's are voided to litter, lost (no path founds a vine from a seed yet).
    pub banked: bool,
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
    /// the ground, and on a wall the layer its feet have reached, to the nearest voxel. A
    /// flyer's (F1) is the layer under the cell its base is in, so its body is indexed and
    /// drawn at `y_m`.
    pub fn sense_layer(&self, voxel_m: f64) -> u32 {
        if matches!(
            self.mobility.mode,
            MobilityMode::Fly | MobilityMode::Perched | MobilityMode::Climb | MobilityMode::Glide
        ) && voxel_m > 0.0
            && self.pose.y_m.is_finite()
        {
            return ((self.pose.y_m / voxel_m + 1e-9).floor() - 1.0).max(0.0) as u32;
        }
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
        self.organic()
            + self.gut.organic
            + self.reproduction.escrow.map_or(0.0, |e| e.organic)
            + self.gut_seeds.total().organic
            + self.stomach.organic()
    }

    /// Mineral in this body, its gut and the escrowed offspring's included.
    pub fn stored_mineral(&self) -> f64 {
        self.mineral
            + self.gut.mineral
            + self.reproduction.escrow.map_or(0.0, |e| e.mineral)
            + self.gut_seeds.total().mineral
            + self.stomach.total().mineral
    }

    /// Energy in this body, its gut and the escrowed offspring's included.
    pub fn stored_energy(&self) -> f64 {
        self.energy
            + self.gut.energy
            + self.reproduction.escrow.map_or(0.0, |e| e.energy)
            + self.gut_seeds.total().energy
            + self.stomach.total().energy
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
    /// **Born in the world** (from a clutch, a gestation or the egg bank), not introduced
    /// or founded: the bodies whose first clutch counts as replacement.
    #[serde(default)]
    pub native: bool,
    /// Ticks before a body refused a nest site, or held by the body ceiling, tries to
    /// lay again ([`Reproduction::nest_retry_s`]).
    #[serde(default)]
    pub retry_ticks: u64,
    /// Ticks a body whose hold is met has waited for a **full clutch** since its last
    /// laying ([`Reproduction::clutch_hold_max_s`]): it lays at once when its reserve pays
    /// for its full clutch ([`Reproduction::full_clutch`] of its reserve cap), and lays what
    /// it can when this reaches the hold's maximum.
    #[serde(default)]
    pub clutch_wait_ticks: u64,
    /// **The brood**, organic units: the part of the reserve a mature body has earmarked
    /// for its next clutch ([`Reproduction::brood_share`]). A label inside the reserve, not
    /// a pool of its own: growth draws only on the reserve above it, hunger does not count
    /// it, a laying pays from it first, and wherever it is read it is at most the reserve
    /// ([`ReproductionState::brood_held`]).
    #[serde(default)]
    pub brood: f64,
    /// **This body's breeding season** (LIFE-HISTORY, [`Reproduction::litters_per_year`]):
    /// the year of its last conception and how many it has had in it. A pack lineage counts
    /// on its [`PackRecord::season`] instead.
    #[serde(default)]
    pub season: Season,
}

/// **A breeding season's count** (LIFE-HISTORY): the Cubarium year of the last conception and
/// the conceptions in it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Season {
    pub year: i64,
    pub litters: u32,
}

impl Season {
    /// The conceptions already had in `year`.
    pub fn used_in(&self, year: i64) -> u32 {
        if self.year == year { self.litters } else { 0 }
    }

    /// Count one conception in `year`.
    pub fn count(&mut self, year: i64) {
        let used = self.used_in(year);
        *self = Season { year, litters: used + 1 };
    }
}

impl ReproductionState {
    /// The brood the reserve actually holds: [`ReproductionState::brood`], at most
    /// `reserve` and never negative.
    pub fn brood_held(&self, reserve: f64) -> f64 {
        self.brood.min(reserve).max(0.0)
    }
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
    /// The young it carries (roster R3, a pack breeder's litter); 0 reads as one.
    #[serde(default)]
    pub young: u32,
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
    /// **A shore egg's clocks** (SNAIL-LAND, [`EggSite::Shore`]): ticks in a row under
    /// standing water, and on ground drier than its lineage's `egg_dry_moisture`. 0 for
    /// every other clutch.
    #[serde(default)]
    pub wet_ticks: u32,
    #[serde(default)]
    pub dry_ticks: u32,
    /// **A clutch in the vines** (VINE-C, [`EggSite::Vine`]): the latticevine-covered wall
    /// face the eggs lie on; `site` is then the support face below them, where they would
    /// fall. `None` for a clutch on its support face.
    #[serde(default)]
    pub face: Option<cubarium_voxel_flora::Face>,
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
    /// **The clutch from surplus** (the detritus web, §3.1C with Astra's correction): the
    /// most eggs one clutch can hold, count. **0 keeps the fixed clutch** of
    /// [`Reproduction::clutch_size`]. Above 0, an egg-layer is eligible on **one affordable
    /// egg** (`egg_organic + surplus_floor`), and at laying its clutch is what the reserve
    /// above the floor pays for, `floor((reserve − surplus_floor) / egg_organic)`, at most
    /// this ([`Reproduction::clutch_for`]); a clutch of none is not laid. Fecundity then
    /// follows condition and, through the reserve cap, body size.
    #[serde(default)]
    pub clutch_max: u32,
    /// **Nest sites** (§3.1D, a crowding brake on recruitment): the radius, metres, around
    /// the laying face within which an egg-layer counts its own lineage — living bodies of
    /// any age and the eggs still waiting to hatch. 0 turns the brake off.
    #[serde(default)]
    pub nest_radius_m: f64,
    /// The most of its own lineage (bodies and unhatched eggs, itself not counted) a body
    /// may lay among within [`Reproduction::nest_radius_m`], count: more refuses the
    /// clutch, which costs nothing, and the body tries again after
    /// [`Reproduction::nest_retry_s`]. Hatchlings stay counted while they grow up near the
    /// site, so the brake holds through recruitment, not just through incubation.
    #[serde(default)]
    pub nest_crowd_max: u32,
    /// Seconds a body refused a nest site (or held by the body ceiling compute guard)
    /// waits before it tries to lay again; its surplus keeps standing.
    #[serde(default = "default_nest_retry_s")]
    pub nest_retry_s: f64,
    /// **A full clutch first** (the capgnawer package, 2026-10-05), seconds: with a clutch
    /// from surplus ([`Reproduction::clutch_max`] above 0), a body whose hold is met waits
    /// up to this long for its reserve to pay for its **full clutch** and lays it the tick
    /// it does; at the end of the wait it lays what the reserve pays for. So a fed body lays
    /// a full clutch and a hungry one a small one, and the clutch follows food rather than
    /// the moment one egg became affordable. 0 lays at once (the clutch is whatever one
    /// affordable egg's hold left). The full clutch is `clutch_max` eggs or what the body's
    /// own reserve cap can pay for, the fewer ([`Reproduction::full_clutch`], D1b): a small
    /// layer whose cap holds one egg does not wait out the hold for three it can never
    /// carry.
    #[serde(default)]
    pub clutch_hold_max_s: f64,
    /// **Fungal egg sites** ([`crate::EggSite::Fungus`]): an egg goes on a face within this
    /// many metres (around the ring and across, one row up or down) of a fungus holding
    /// cap tissue, so recruitment sits where the food is. 0: the fungus's own face only.
    #[serde(default)]
    pub egg_fungus_radius_m: f64,
    /// **The brood** (package D1b, the κ-rule of dynamic energy budgets): the share of a
    /// **mature** egg-layer's net income — assimilation less the respiration its reserve pays
    /// — earmarked for its next clutch, `1 − κ`. A body at or past `birth_body` puts this
    /// share into its brood ([`ReproductionState::brood`]) until the brood holds
    /// [`Reproduction::brood_target`], and builds structure only out of the reserve above
    /// the brood; the rest of its income grows it. So a growing adult saves for eggs
    /// instead of building every surplus into structure (which otherwise drains the reserve
    /// to a few per cent of its cap and puts off the first clutch until `body_max`). The
    /// brood also does not sate: hunger counts only the reserve above it. 0 turns it off.
    #[serde(default)]
    pub brood_share: f64,
    /// **Breeding seasons** (LIFE-HISTORY, Wrysk 2026-10-06): at most this many conceptions —
    /// a gestation opened, or a clutch laid — per Cubarium year
    /// ([`cubarium_voxel::weather::Climate::year`], the dry-season cycle), each inside the
    /// window [`Reproduction::breeding_phase`] + [`Reproduction::breeding_window`]. A pack
    /// lineage counts them per pack (the pack breeds, whoever its breeder is). **0: no season**
    /// — the lineage breeds exactly as without these knobs; so does a seasonal lineage in a
    /// world with no year (no season period).
    #[serde(default)]
    pub litters_per_year: u32,
    /// The year phase, `[0, 1)`, at which the breeding window opens (phase 0 is a dry
    /// season's start).
    #[serde(default)]
    pub breeding_phase: f64,
    /// The breeding window's width, a share of the year (wrapping past its end; 1 or more is
    /// the whole year).
    #[serde(default)]
    pub breeding_window: f64,
    /// **Pregnancy upkeep**: a gestating body's basal upkeep is multiplied by this while its
    /// escrow is open. 1 is no raise.
    #[serde(default = "default_gestation_upkeep_factor")]
    pub gestation_upkeep_factor: f64,
}

fn default_gestation_upkeep_factor() -> f64 {
    1.0
}

fn default_surplus_share() -> f64 {
    SURPLUS_SHARE
}

fn default_nest_retry_s() -> f64 {
    NEST_RETRY_S
}

/// The shipped [`Reproduction::nest_retry_s`]: a minute, long beside a step and short
/// beside the hold.
pub const NEST_RETRY_S: f64 = 60.0;

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
        clutch_max: 0,
        nest_radius_m: 0.0,
        nest_crowd_max: 0,
        nest_retry_s: NEST_RETRY_S,
        clutch_hold_max_s: 0.0,
        egg_fungus_radius_m: 0.0,
        brood_share: 0.0,
        litters_per_year: 0,
        breeding_phase: 0.0,
        breeding_window: 0.0,
        gestation_upkeep_factor: 1.0,
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
        clutch_max: 0,
        nest_radius_m: 0.0,
        nest_crowd_max: 0,
        nest_retry_s: NEST_RETRY_S,
        clutch_hold_max_s: 0.0,
        egg_fungus_radius_m: 0.0,
        brood_share: 0.0,
        litters_per_year: 0,
        breeding_phase: 0.0,
        breeding_window: 0.0,
        gestation_upkeep_factor: 1.0,
    };

    /// The lanternjaw's **dormant egg bank** (§2.13), revised with its metabolism in the
    /// detritus web D2: each egg `egg_organic` 0.005 — a `body_min` hatchling (0.0034) and a
    /// small reserve (0.0016) — buried as its own clutch, the clutch **from surplus**
    /// (eligible on one affordable egg, up to `clutch_max` 3 at laying, the D1 mechanism),
    /// over a floor of 0.005, one egg. At the shipped mass (×30, adult 1.05, reserve cap
    /// 0.6 → 0.63): the floor 0.15 is two days of the adult's 0.07 daily need; one egg needs
    /// a reserve of 0.30, the whole clutch 0.60, which a full adult (0.63) holds and lays,
    /// keeping 0.18 above the floor. Hold and interval are the other egg-layers'.
    pub const DORMANT_EGGS_PLACEHOLDER: Reproduction = Reproduction {
        mode: BirthMode::Eggs,
        surplus_floor: 0.005,
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
        clutch_max: 3,
        nest_radius_m: 0.0,
        nest_crowd_max: 0,
        nest_retry_s: NEST_RETRY_S,
        clutch_hold_max_s: 0.0,
        egg_fungus_radius_m: 0.0,
        brood_share: 0.0,
        litters_per_year: 0,
        breeding_phase: 0.0,
        breeding_window: 0.0,
        gestation_upkeep_factor: 1.0,
    };

    /// What the **smallest** offspring package costs the parent's reserve, the price
    /// eligibility asks for: a gestation's `birth_cost`, a fixed clutch, or — with a clutch
    /// from surplus ([`Reproduction::clutch_max`] above 0) — one egg.
    pub fn package_cost(&self, sc: &SpeciesConfig) -> f64 {
        match self.mode {
            BirthMode::Gestation => sc.birth_cost,
            BirthMode::Eggs if self.clutch_max > 0 => self.egg_organic,
            BirthMode::Eggs => f64::from(self.clutch_size) * self.egg_organic,
        }
    }

    /// **The eggs a clutch holds** for a parent with `reserve`: the fixed
    /// [`Reproduction::clutch_size`], or with a clutch from surplus the eggs the reserve
    /// above the floor pays for, `floor((reserve − surplus_floor) / egg_organic)`, between
    /// 0 (not one affordable) and [`Reproduction::clutch_max`].
    pub fn clutch_for(&self, reserve: f64) -> u32 {
        if self.clutch_max == 0 {
            return self.clutch_size;
        }
        if !(self.egg_organic > 0.0) || !(reserve.is_finite()) {
            return 0;
        }
        let paid = ((reserve - self.surplus_floor) / self.egg_organic).floor();
        if !(paid >= 1.0) {
            return 0;
        }
        (paid.min(f64::from(self.clutch_max))) as u32
    }

    /// **The full clutch a reserve of `cap` can pay for**: the fixed clutch if `cap` holds it
    /// and its floor (else 0), or with a clutch from surplus [`Reproduction::clutch_for`]
    /// of `cap` (at most `clutch_max`). With `cap` the body's reserve cap, the most this
    /// body could ever lay at once: what a full-clutch wait waits for and what its brood
    /// saves for.
    pub fn full_clutch(&self, cap: f64) -> u32 {
        if self.clutch_max == 0 {
            let fits = f64::from(self.clutch_size) * self.egg_organic + self.surplus_floor <= cap;
            return if fits { self.clutch_size } else { 0 };
        }
        self.clutch_for(cap)
    }

    /// **What the brood saves for** at structure `body`: the full clutch its reserve cap
    /// can pay for ([`Reproduction::full_clutch`]) plus the floor, organic units; 0 with no
    /// brood share, for a live-bearer, or when the cap pays for no egg.
    pub fn brood_target(&self, sc: &SpeciesConfig, body: f64) -> f64 {
        if !(self.brood_share > 0.0) || self.mode != BirthMode::Eggs {
            return 0.0;
        }
        let n = self.full_clutch(sc.reserve_of(body));
        if n == 0 {
            return 0.0;
        }
        f64::from(n) * self.egg_organic + self.surplus_floor
    }

    /// Ticks a body waits for a full clutch ([`Reproduction::clutch_hold_max_s`]); 0 lays
    /// at once.
    pub fn clutch_hold_max_ticks(&self) -> u64 {
        if self.clutch_hold_max_s > 0.0 {
            cubarium_voxel::ticks_at_least_one(self.clutch_hold_max_s)
        } else {
            0
        }
    }

    /// Ticks a refused laying waits ([`Reproduction::nest_retry_s`]).
    pub fn retry_ticks(&self) -> u64 {
        cubarium_voxel::ticks_at_least_one(self.nest_retry_s)
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

    /// **The breeding season's gate** (LIFE-HISTORY): whether a conception may happen at
    /// `year` with `season` counted so far — always, for a lineage with no season
    /// (`litters_per_year` 0) or in a world with no year; else only inside the window and
    /// while the year's conceptions are under `litters_per_year`.
    pub fn season_allows(&self, year: Option<cubarium_voxel::weather::Year>, season: Season) -> bool {
        if self.litters_per_year == 0 {
            return true;
        }
        let Some(y) = year else {
            return true;
        };
        y.in_window(self.breeding_phase, self.breeding_window) && season.used_in(y.number) < self.litters_per_year
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
    /// The held **lift** effort, a flyer's fourth action (F1, §3.3), in `[-1, 1]`. Zero for
    /// every walker.
    #[serde(default)]
    pub lift: f64,
    /// **Stamina spent** (GAIT, [`crate::Sprint`]): 0 rested, 1 spent. A sprint spends it at
    /// `1 / endurance_s` a second at top speed, and time off the sprint rests it at
    /// `1 / recovery_s`.
    #[serde(default)]
    pub sprint_spent: f64,
    /// Sprinted out: no sprint until rested to the sprint's `resume_fraction`.
    #[serde(default)]
    pub winded: bool,
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

    /// Multiply the **seven absolute quantities** — `body_max`, `body_min`, `birth_body`,
    /// `birth_cost`, the reproduction `surplus_floor` and `egg_organic`, and
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
// A knob a saved or authored config lacks takes its default (SAVE-EXTENSIBLE).
#[serde(default)]
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
    /// **A rider's take** (the minimal gut, repair 2 item 9): host body per world day
    /// ([`LifeConfig::ecological_day_s`]) a rider takes out of its host — reserve first,
    /// then structure, all three currencies at the host's density — straight into its
    /// own reserve. Default 0.09: on a 1.5-ou grazer a hatchling reaches drop-off in about
    /// 1.19 world days (Astra's integration). Replaces the basal-tied drain, which the
    /// cut to basal upkeep left unable to grow a rider.
    pub parasite_take_per_day: f64,
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
    /// **The body ceiling, a compute guard and not ecology** (the detritus web, §3.5;
    /// Wrysk 2026-10-05): living bodies one lineage may hold, count. At it, a body that
    /// would lay or conceive does not, the refusal is booked
    /// ([`FaunaLedger::ceiling_refusals_by_founder`]) and logged on stderr, and it tries
    /// again after its lineage's [`Reproduction::nest_retry_s`]. Set far above any
    /// population the ecology should hold: every activation means the ecological brakes
    /// (crowding, predation, food) failed. 0 turns the guard off.
    #[serde(default = "default_lineage_body_ceiling")]
    pub lineage_body_ceiling: u32,
    /// **The lanternjaw's patrol** (the detritus web D2, [`HuntConfig`]): its heuristic's
    /// patch-to-patch search, tremor sense and prey switching.
    #[serde(default)]
    pub hunt: HuntConfig,
    /// **The perception core** (SENSES-CORE, [`PerceptionConfig`]): the pairwise senses'
    /// cadence, candidates, occlusion, hearing's emission and the pursuit's memory.
    #[serde(default)]
    pub perception: PerceptionConfig,
    /// **The scent field** (SENSES-CORE, [`ScentConfig`]).
    #[serde(default)]
    pub scent: ScentConfig,
    /// **Light** (S3, [`LightConfig`]): the beacons' brightness, the photoreceptor, the lure's
    /// point, inspections and the failed-visit mark.
    #[serde(default)]
    pub light: LightConfig,
}

fn default_lineage_body_ceiling() -> u32 {
    LINEAGE_BODY_CEILING
}

/// The shipped [`FaunaConfig::lineage_body_ceiling`]: ten times the largest lineage the
/// desk terrarium has carried (the bellwing's ~250, 2026-10-05).
pub const LINEAGE_BODY_CEILING: u32 = 2500;

/// **The shipped org-unit mass scale** (Wrysk, 2026-09-29; audit
/// `design/7_Research/ecology-parameter-audit-2026-09-28.md` §1.4): both lineages are
/// built this many times heavier against the plants than the placeholder tables
/// ([`SpeciesConfig::scale_mass`]: the seven absolute quantities, nothing per unit), so one
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
            parasite_take_per_day: 0.09,
            parasite_detach_body_fraction: 0.24,
            predation: PredationConfig::default(),
            life: LifeConfig::default(),
            cue: CueConfig::default(),
            teachers: TeacherConfig::default(),
            founding: FoundingConfig::default(),
            lineage_body_ceiling: LINEAGE_BODY_CEILING,
            hunt: HuntConfig::default(),
            perception: PerceptionConfig::default(),
            scent: ScentConfig::default(),
            light: LightConfig::default(),
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
    /// An amphibious body's drying clock reached its lineage's `lethal_fraction`
    /// ([`crate::Amphibious`], AMPHIBIOUS-SNAIL). A corpse.
    Desiccation,
    /// DROP: its wound reached 1 on a jump down ([`crate::DropRule`]). A corpse.
    Fell,
}

impl Departure {
    pub const ALL: [Departure; 6 + Founder::COUNT] = {
        let mut all = [Departure::Starved; 6 + Founder::COUNT];
        all[4 + Founder::COUNT] = Departure::Desiccation;
        all[5 + Founder::COUNT] = Departure::Fell;
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
            Departure::Desiccation => 4 + Founder::COUNT,
            Departure::Fell => 5 + Founder::COUNT,
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
            Departure::Desiccation => "desiccated",
            Departure::Fell => "fell",
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
    /// **Nectar** (F1, predation-and-flight §3.7, decision 6): a flower product, not the
    /// seed parcel — a latticevine face in flower
    /// ([`cubarium_voxel_flora::Flora::take_nectar`]) or a bloomcrown's own nectar stock
    /// ([`cubarium_voxel_flora::Flora::take_stand_nectar`]). Energy-rich and mineral-poor.
    /// The bellwing's food: a sip is `feed` with a bloom inside the mouth reach while
    /// hovering at it or perched on it.
    Nectar,
    /// **Fruit** (F3, the seedporter's; predation-and-flight §3.7): a product, not the
    /// seed parcel — a latticevine face in fruit
    /// ([`cubarium_voxel_flora::Flora::take_fruit`]) or a lanternberry's hanging fruit
    /// stock ([`cubarium_voxel_flora::Flora::take_stand_fruit`]). Energy-rich, a little
    /// mineral. A fruit meal records the source species' seeds in the gut.
    Fruit,
}

impl Food {
    pub const ALL: [Food; 7] = [
        Food::Litter,
        Food::CapTissue,
        Food::Carrion,
        Food::Foliage,
        Food::Film,
        Food::Nectar,
        Food::Fruit,
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
            Food::Nectar => 5,
            Food::Fruit => 6,
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

    /// The place on a face whose stand this food comes off ([`cubarium_voxel_flora::FaceLayer`]):
    /// cap tissue is the face's fungus, every other stand food its plant. A face may hold
    /// both, so a stand bite names its place by its food.
    pub const fn face_layer(self) -> cubarium_voxel_flora::FaceLayer {
        match self {
            Food::CapTissue => cubarium_voxel_flora::FaceLayer::Fungus,
            _ => cubarium_voxel_flora::FaceLayer::Plant,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Food::Litter => "litter",
            Food::CapTissue => "cap_tissue",
            Food::Carrion => "carrion",
            Food::Foliage => "foliage",
            Food::Film => "film",
            Food::Nectar => "nectar",
            Food::Fruit => "fruit",
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
    /// Living fungal mycelium withdrawn by lineage, included in CapTissue intake.
    pub eaten_mycelium_by_founder: [f64; Founder::COUNT],
    /// Raw organic matter withdrawn by founder and food, before any bite spillage.
    pub eaten_by_founder_food: [[f64; Food::COUNT]; Founder::COUNT],
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
    /// GAIT: ticks spent sprinting (a forward above cruise delivered), and the times a body
    /// sprinted itself out (winded), by lineage.
    #[serde(default)]
    pub sprint_ticks_by_founder: [u64; Founder::COUNT],
    #[serde(default)]
    pub winded_by_founder: [u64; Founder::COUNT],
    /// DROP: jumps down past the step by a trapped body, their metres summed, and the wound
    /// they added summed, by lineage.
    #[serde(default)]
    pub drops_by_founder: [u64; Founder::COUNT],
    #[serde(default)]
    pub drop_m_by_founder: [f64; Founder::COUNT],
    #[serde(default)]
    pub drop_wound_by_founder: [f64; Founder::COUNT],
    /// Predation (`design/predation-and-flight-2026-09-28.md` §2.3), by the **striker's**
    /// lineage: strikes begun, and contacts resolved as a hit or a dodge.
    pub strikes_by_founder: [u64; Founder::COUNT],
    pub hits_by_founder: [u64; Founder::COUNT],
    pub dodges_by_founder: [u64; Founder::COUNT],
    /// **The patrol's readouts** (the detritus web D2; counters only, no rule reads them).
    /// Strikes begun, by the **prey's** lineage (any striker).
    #[serde(default)]
    pub strikes_on_by_founder: [u64; Founder::COUNT],
    /// Seconds of contact a patrolling hunter had with size-eligible prey of each lineage
    /// (seen or felt): the locally available prey its kills are read against.
    #[serde(default)]
    pub hunt_encounter_s_by_prey: [f64; Founder::COUNT],
    /// Controller samples a patroller spent waiting still on a patch, walking to one, and
    /// engaged with a prey it saw or felt.
    #[serde(default)]
    pub hunt_wait_samples: u64,
    #[serde(default)]
    pub hunt_travel_samples: u64,
    #[serde(default)]
    pub hunt_engaged_samples: u64,
    /// Patches left fruitless (a wait with nothing, or a walk given up), and patches
    /// chosen at random (exploration) or by the search image.
    #[serde(default)]
    pub hunt_fruitless: u64,
    #[serde(default)]
    pub hunt_explored: u64,
    #[serde(default)]
    pub hunt_chosen: u64,
    /// (HUNTER-MEAL) Controller samples a hungry hunter spent going to a usable carcass out
    /// of its mouth's reach, and at one (the meal step's answers).
    pub hunt_meal_walk_samples: u64,
    pub hunt_meal_eat_samples: u64,
    /// Strikes withheld because the striker's mouth was over a usable carcass.
    pub hunt_strikes_withheld: u64,
    /// Patch squares with food a patroller passed over as off its walking route.
    pub hunt_unreachable_squares: u64,
    /// Patch choices made with the patroller off its route graph (main's rule then).
    pub hunt_off_route: u64,
    /// Kills a patroller remembered as a productive place.
    pub hunt_productive_kills: u64,
    /// (D2b) Seconds of contact a patroller had with prey of each lineage that its attack
    /// choice declined (met, seen or felt, and passed over), and strikes begun by a
    /// patroller on a prey its choice did not accept (only with
    /// [`HuntConfig::strike_chosen_only`] off: the opportunistic strike).
    #[serde(default)]
    pub hunt_declined_s_by_prey: [f64; Founder::COUNT],
    #[serde(default)]
    pub strikes_unchosen_by_prey: [u64; Founder::COUNT],
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
    /// **Flight** (F1): nectar sips by source, `[bloomcrown stand, latticevine face]`.
    #[serde(default)]
    pub sips_by_bloom: [u64; 2],
    /// Landings by perch kind, in [`PerchKind::ALL`] order.
    #[serde(default)]
    pub landings_by_perch: [u64; 4],
    /// Paid takeoffs, and drops: a lost perch a flyer could not pay to leave.
    #[serde(default)]
    pub takeoffs: u64,
    #[serde(default)]
    pub drops: u64,
    /// The `respired_motor_out` share that was **flight**: seconds aloft beyond basal,
    /// lift work and takeoffs.
    #[serde(default)]
    pub respired_flight_out: f64,
    /// **Gliders** (F3): fruit eaten by source, organic, `[stand fruit, latticevine
    /// fruit]` (a share of `eaten_by_food[fruit]`), and the bites.
    #[serde(default)]
    pub fruit_eaten_by_source: [f64; 2],
    #[serde(default)]
    pub fruit_bites_by_source: [u64; 2],
    /// Glides started by choice (off a ledge, a crown or a vine), and forced ones (a lost
    /// perch or hold).
    #[serde(default)]
    pub glides: u64,
    #[serde(default)]
    pub glides_forced: u64,
    /// Stem and vine climbs started.
    #[serde(default)]
    pub climbs: u64,
    /// Gut seeds voided, organic, by source (`[stand, latticevine]`), and the voids. A
    /// latticevine's seeds are voided to litter (lost: no seed path founds a vine yet).
    #[serde(default)]
    pub seeds_voided_by_source: [f64; 2],
    #[serde(default)]
    pub seed_voids: u64,
    /// **Provisioning** (roster R3): organic matter adults at a den passed out of their
    /// reserves to pups, and what the pups received of it (`provision_efficiency` of it;
    /// the rest is respired, inside `respired_out`).
    #[serde(default)]
    pub provision_given_organic: f64,
    #[serde(default)]
    pub provision_received_organic: f64,
    /// **The pack tactic** (roster R3): pack chases begun (a member set a pack target),
    /// and those that ended with the target dead and its carrion on its face.
    #[serde(default)]
    pub pack_chases: u64,
    #[serde(default)]
    pub pack_chase_kills: u64,
    /// **The pack's senses** (VEILHOUND-SEARCH, SENSES-CORE): chases begun on a prey sensed
    /// by each of `[sight, hearing, tremor, smell]` ([`crate::perception::MODALITIES`]) — a
    /// prey seen and heard counts under both, so the entries need not sum to the chases;
    /// draws begun (the leader set out toward a new prey beyond its reach, or up its prey's
    /// scent, counted under smell) by the same; chases begun while drawn; controller samples
    /// of a hungry adult (wanting food or in an emergency) under the pack tactic, and of
    /// those the samples spent leading a draw and following its leader.
    #[serde(default)]
    pub pack_chases_by_sense: [u64; 4],
    #[serde(default)]
    pub pack_draws_by_sense: [u64; 4],
    /// **The hunt funnel** (SENSES-CORE), per `[hunter][prey]` lineage and stage
    /// ([`crate::perception::FUNNEL_STAGES`]: sensed, approached, struck, killed, eaten):
    /// a hunter's detections of a prey, its setting out after one (a pack target or a draw,
    /// a patrol's engage), each strike begun, each kill, and its first bite of a carcass.
    /// Sensed, approached and eaten count **episodes**: once per hunter and prey (carcass),
    /// again only after the pursuit's memory without it ([`crate::perception::Percept::first`]).
    #[serde(default)]
    pub funnel: [[[u64; 5]; Founder::COUNT]; Founder::COUNT],
    /// The perception core's cost (SENSES-CORE): remote sensings run, pairs weighed (one
    /// terrain ray each) and the cells those rays crossed.
    #[serde(default)]
    pub percept_sensings: u64,
    #[serde(default)]
    pub percept_pairs: u64,
    #[serde(default)]
    pub percept_ray_cells: u64,
    /// Pack targets given up because no member had sensed them within the pursuit's memory
    /// ([`PerceptionConfig::give_up_s`]).
    #[serde(default)]
    pub pack_targets_lost: u64,
    /// **The prey choice's readouts** (PACK-TUNE): pack targets broken off in a chase (their
    /// prospect under `Pack::break_off_below`), and sensed takeable prey a pack declined at a
    /// choice for a prospect under `Pack::select_min_prospect` (one count per body per choice).
    #[serde(default)]
    pub pack_break_offs: u64,
    #[serde(default)]
    pub pack_declined: u64,
    /// **CHURN**: pack targets given up for a better one (past `Pack::switch_margin`).
    #[serde(default)]
    pub pack_switches: u64,
    /// **STRIKE**: member samples steered by the close-in ([`Pack::close_in_m`]); of them,
    /// steps off from a prey inside the outline and pivots to face one; and chases broken off
    /// as out of reach (no face beside the target puts it in the jaw band).
    #[serde(default)]
    pub pack_close_in_samples: u64,
    #[serde(default)]
    pub pack_step_offs: u64,
    #[serde(default)]
    pub pack_pivots: u64,
    #[serde(default)]
    pub pack_out_of_reach: u64,
    /// **The ground responses' readouts** (S2, [`vigil::VigilLedger`]).
    #[serde(default)]
    pub vigil: VigilLedger,
    #[serde(default)]
    pub pack_draw_chases: u64,
    #[serde(default)]
    pub pack_hungry_samples: u64,
    #[serde(default)]
    pub pack_draw_lead_samples: u64,
    #[serde(default)]
    pub pack_draw_follow_samples: u64,
    /// **Light and flight** (S3, [`crate::light`]), per lineage: inspections begun by
    /// beacon kind (`[flower, glowcap, lure]`, [`BeaconKind`]) and those that failed (marked
    /// "no reward here"); controller samples spent evading a looming shadow, turned from a
    /// moving light, steered by a beacon, and led up the nectar smell.
    #[serde(default)]
    pub beacon_visits: [[u64; BeaconKind::COUNT]; Founder::COUNT],
    #[serde(default)]
    pub failed_visits: [[u64; BeaconKind::COUNT]; Founder::COUNT],
    #[serde(default)]
    pub loom_samples: [u64; Founder::COUNT],
    #[serde(default)]
    pub repel_samples: [u64; Founder::COUNT],
    #[serde(default)]
    pub beacon_samples: [u64; Founder::COUNT],
    #[serde(default)]
    pub scent_led_samples: [u64; Founder::COUNT],
    /// Kills of a prey drawn to a lure when struck (inspecting one, or a lure its brightest
    /// still beacon), by prey lineage.
    #[serde(default)]
    pub lure_kills: [u64; Founder::COUNT],
    /// **S3b readouts** ([`crate::light`]): a flyer's controller samples by what its light
    /// steer did ([`crate::light::LightMode`]), the time budget; approaches given up because
    /// the body stalled short of the beacon; inspections that ended in a sip, by beacon kind;
    /// and kills, `[killer][prey]`, of a prey still naive about cyan (its cyan expectation at
    /// least half its innate value) when it died.
    #[serde(default)]
    pub light_budget: [[u64; light::LightMode::COUNT]; Founder::COUNT],
    #[serde(default)]
    pub stalled_visits: [u64; Founder::COUNT],
    /// FINDINGS-FIX: open inspections closed as failed because the body flew away from them
    /// (beyond `visit_m` + `visit_leave_m`), by beacon kind; also counted in `failed_visits`.
    #[serde(default)]
    pub left_visits: [[u64; BeaconKind::COUNT]; Founder::COUNT],
    #[serde(default)]
    pub sipped_visits: [[u64; BeaconKind::COUNT]; Founder::COUNT],
    #[serde(default)]
    pub naive_kills: [[u64; Founder::COUNT]; Founder::COUNT],
    /// Lanternjaws that left a carcass to a body that hunts them (roster R3, the yield).
    #[serde(default)]
    pub carcass_yields: u64,
    /// A pack breeder's litters born, and the pups in them (roster R3).
    #[serde(default)]
    pub litters: u64,
    #[serde(default)]
    pub pups_born: u64,
    /// Pups that came of age (reached [`Pack::independence_age_s`] alive) and joined their
    /// pack (LIFE-HISTORY).
    #[serde(default)]
    pub pups_independent: u64,
    /// Conceptions a breeding season refused (outside the window, or the year's litters
    /// used), counted once per refused tick a ready body waited (LIFE-HISTORY).
    #[serde(default)]
    pub season_waits: u64,
    /// **A pup's income by source** (PUP), organic units: raw food pups swallowed by their own
    /// bites (a feast at the den), and raw food regurgitated into pups by a giver at or above
    /// its lineage's `provision_reserve_fraction` (`_surplus`) or below it (`_lean`). The
    /// reserve fallback is `provision_received_organic`; the birth stores are the litters'
    /// parcels (`pups_born` × the package).
    #[serde(default)]
    pub pup_swallowed_organic: f64,
    #[serde(default)]
    pub pup_regurgitated_surplus: f64,
    #[serde(default)]
    pub pup_regurgitated_lean: f64,
    /// Basal upkeep the pups paid (PUP), organic units.
    #[serde(default)]
    pub pup_upkeep_organic: f64,
    /// Pups that died as pups (PUP): starved, and by any other cause.
    #[serde(default)]
    pub pups_starved: u64,
    #[serde(default)]
    pub pups_lost_other: u64,
    /// Ticks an adult at the den held raw food and a pup there begged, but its condition kept
    /// the meal (PUP, [`Pack::regurgitate_reserve_fraction`]).
    #[serde(default)]
    pub provision_withheld_ticks: u64,
    /// **The minimal gut's books** (repair 2 item 10). Raw food into stomachs (every bite,
    /// after any spill), out of them by digestion (into assimilation, egesta and the
    /// unfunded share's respiration), moved stomach to stomach by regurgitation (inside
    /// the layer, so it nets to zero), and leaving with a body that died or was removed.
    /// `swallowed − digested − stomach_out` is the change in what stomachs hold over the
    /// layer's life **less the founding stock**: a founder's `s0` stomach and any stomach
    /// food a test or tool introduces are booked `introduced_*_in`, not swallowed, so a
    /// diagnostic of the stomach stock adds them (or, like the books test, starts from a
    /// measured baseline).
    #[serde(default)]
    pub swallowed: Taken,
    #[serde(default)]
    pub digested: Taken,
    #[serde(default)]
    pub regurgitated: Taken,
    #[serde(default)]
    pub stomach_out: Taken,
    /// What riders took out of their hosts ([`FaunaConfig::parasite_take_per_day`]),
    /// organic: a transfer inside the layer.
    #[serde(default)]
    pub parasite_taken_organic: f64,
    /// **Meals**: feeding bouts completed (the eating latch's full event), by lineage and
    /// [`BoutKind`] in [`BoutKind::ALL`] order, and their summed durations in ticks.
    #[serde(default)]
    pub meals_by_founder_kind: [[u64; BoutKind::COUNT]; Founder::COUNT],
    #[serde(default)]
    pub meal_ticks_by_founder_kind: [[u64; BoutKind::COUNT]; Founder::COUNT],
    /// Sleepers the emergency latch woke (it went on while the body was asleep), by lineage.
    #[serde(default)]
    pub emergency_wakes_by_founder: [u64; Founder::COUNT],
    /// Sleep flips — a change of [`State::Asleep`] either way — by lineage.
    #[serde(default)]
    pub sleep_flips_by_founder: [u64; Founder::COUNT],
    /// Bodies that grew through their lineage's `birth_body`, and their summed ages then,
    /// ticks: the measured maturation time.
    #[serde(default)]
    pub matured_by_founder: [u64; Founder::COUNT],
    #[serde(default)]
    pub maturation_ticks_by_founder: [u64; Founder::COUNT],
    /// Raw food swallowed and digested (organic), by lineage (repair 3's measure).
    #[serde(default)]
    pub swallowed_by_founder: [f64; Founder::COUNT],
    #[serde(default)]
    pub digested_by_founder: [f64; Founder::COUNT],
    /// Sip attempts with the eating latch on and `feed` held but no bloom in the mouth's
    /// reach, by lineage (repair 3's measure: a flyer's reach failures).
    #[serde(default)]
    pub sip_misses_by_founder: [u64; Founder::COUNT],
    /// **Spend by lineage** (the search rebase's balance measures; counters only, no
    /// rule reads them), organic. Upkeep paid, the sleep factor applied.
    #[serde(default)]
    pub upkeep_by_founder: [f64; Founder::COUNT],
    /// Motor paid: walking, flying, climbing, and a hunter's lure and strike costs.
    #[serde(default)]
    pub motor_by_founder: [f64; Founder::COUNT],
    /// Structure built out of the reserve (growth).
    #[serde(default)]
    pub growth_by_founder: [f64; Founder::COUNT],
    /// Reproduction invested: gestation instalments into an escrow, and clutches laid.
    #[serde(default)]
    pub reproduction_by_founder: [f64; Founder::COUNT],
    /// Provisioning given by the giver's lineage: reserve transferred to young, and raw
    /// food regurgitated into their stomachs (raw organic).
    #[serde(default)]
    pub provision_by_founder: [f64; Founder::COUNT],
    /// What riders took out of hosts of this lineage (the host's loss).
    #[serde(default)]
    pub rider_take_lost_by_founder: [f64; Founder::COUNT],
    /// Ticks a free founder body was awake with its eating latch on ("wanting"), and the
    /// ticks it spent eating: each bite swallowed credits its controller interval's
    /// ticks (a bite covers one interval).
    #[serde(default)]
    pub wanting_ticks_by_founder: [u64; Founder::COUNT],
    #[serde(default)]
    pub eating_ticks_by_founder: [u64; Founder::COUNT],
    /// Ticks a free founder body was awake, latch on or off (the sated share's
    /// denominator).
    #[serde(default)]
    pub awake_ticks_by_founder: [u64; Founder::COUNT],
    /// **The detritus web's readouts** (2026-10-05; counters only, no rule reads them).
    /// Ticks a free founder body was alive, asleep or awake: the awake share's denominator.
    #[serde(default)]
    pub alive_ticks_by_founder: [u64; Founder::COUNT],
    /// Awake ticks a dark-active body owed to the dark at its face: the daylight was at or
    /// above the day line, so a nocturnal body would have slept.
    #[serde(default)]
    pub dark_awake_ticks_by_founder: [u64; Founder::COUNT],
    /// **RHYTHMS-CADENCE readouts** (counters only, no rule reads them): ticks a free founder
    /// body was alive with the sun up, and awake with the sun up (the night's are the totals
    /// above less these); rest bouts begun (a leaning); sleepers woken by an alarm.
    #[serde(default)]
    pub alive_day_ticks_by_founder: [u64; Founder::COUNT],
    #[serde(default)]
    pub awake_day_ticks_by_founder: [u64; Founder::COUNT],
    #[serde(default)]
    pub rest_bouts_by_founder: [u64; Founder::COUNT],
    #[serde(default)]
    pub alarm_wakes_by_founder: [u64; Founder::COUNT],
    /// Ticks a heuristic walker was paced (searching or foraging on its own steering, moving
    /// faster than a stay on food, not fleeing) and, of them, paused ([`crate::Cadence`]).
    #[serde(default)]
    pub paced_ticks_by_founder: [u64; Founder::COUNT],
    #[serde(default)]
    pub paused_ticks_by_founder: [u64; Founder::COUNT],
    /// Controller samples in which a body driven by its heuristic held still to feed (feed
    /// on, forward at most the teachers' `forage_stay_forward`): the stay-on-food reflex.
    #[serde(default)]
    pub feed_still_ticks_by_founder: [u64; Founder::COUNT],
    /// Clutches laid, by lineage (eggs over clutches is the mean clutch).
    #[serde(default)]
    pub clutches_by_founder: [u64; Founder::COUNT],
    /// **Replacement**: bodies born in the world that laid their first clutch (or bore
    /// their first young), and the ages they did it at, summed, ticks — the hatch to
    /// first clutch time.
    #[serde(default)]
    pub first_clutches_by_founder: [u64; Founder::COUNT],
    #[serde(default)]
    pub first_clutch_age_ticks_by_founder: [u64; Founder::COUNT],
    /// Layings the nest-site brake refused ([`Reproduction::nest_crowd_max`]), events.
    #[serde(default)]
    pub nest_refusals_by_founder: [u64; Founder::COUNT],
    /// VINE-C: clutches laid on a latticevine face ([`EggSite::Vine`]), by lineage.
    #[serde(default)]
    pub vine_clutches_by_founder: [u64; Founder::COUNT],
    /// SEEDPORTER-POLLEN: vine flower visits by contact — open flowers a body that carries
    /// pollen ([`FounderPhysiology::pollen_by_contact`]) touched as it moved, bringing pollen
    /// from another flower — by lineage. Each is also one of the plant layer's
    /// `flower_visits`; the rest of those are sip visits.
    #[serde(default)]
    pub contact_visits_by_founder: [u64; Founder::COUNT],
    /// VINE-C: controller samples a vine nester, ready to lay, held at a nest site, and flew
    /// toward one it found ([`crate::Flight::nest_search_m`]), by lineage. Diagnostics.
    #[serde(default)]
    pub nest_holds_by_founder: [u64; Founder::COUNT],
    #[serde(default)]
    pub nest_searches_by_founder: [u64; Founder::COUNT],
    /// Layings or conceptions the body ceiling compute guard refused
    /// ([`FaunaConfig::lineage_body_ceiling`]), events. Any is a failure of the brakes.
    #[serde(default)]
    pub ceiling_refusals_by_founder: [u64; Founder::COUNT],
    /// Eggs lost before hatching (face gone, flooded), by lineage.
    #[serde(default)]
    pub eggs_lost_by_founder: [u64; Founder::COUNT],
    /// **Shore eggs drowned** (SNAIL-LAND): under standing water past `egg_drown_s`, to
    /// the litter of their face, by lineage.
    #[serde(default)]
    pub eggs_drowned_by_founder: [u64; Founder::COUNT],
    /// ... and dried: on ground drier than `egg_dry_moisture` past `egg_dry_s`.
    #[serde(default)]
    pub eggs_dried_by_founder: [u64; Founder::COUNT],
    /// **AMPHIBIOUS-SNAIL readouts** (counters only): ticks an amphibious body was alive,
    /// `[day, night][in water, ashore]` (day: daylight at or above the day line, or the
    /// clock off), and retractions begun.
    #[serde(default)]
    pub amphibious_ticks: [[u64; 2]; 2],
    #[serde(default)]
    pub retracts: u64,
    /// Organic matter an amphibious body assimilated while ashore (its share of
    /// `assimilated_by_founder`).
    #[serde(default)]
    pub ashore_assimilated: f64,
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
    /// Put a **flyer** founder at exactly `pose`, its `y_m` the body's base height in
    /// metres (F1): airborne ([`MobilityMode::Fly`]) over the support face below its
    /// column, or perched where its base meets a perch. Booked as
    /// [`Command::IntroduceFounder`] is. Refused as that is, and also for a lineage that
    /// cannot fly, a pose whose body would occupy a solid or water cell, and a `y_m` above
    /// the lineage's ceiling over the support face below.
    ///
    /// **A glider** (F3, the seedporter) is taken too: gliding ([`MobilityMode::Glide`])
    /// when its base is in the air at least its `glide_min_height_m` over the landing
    /// surface below, perched where its base meets the crown top of a stand it could climb,
    /// walking where it meets the ground; refused in between.
    IntroduceAloft {
        pose: Pose,
        founder: Founder,
        stores: StartingStores,
    },
    /// **Found a pack** (roster R3) of a lineage with a [`FounderPhysiology::pack`] table:
    /// a new [`PackRecord`] with its den on the support face `den`, and its `pack_size`
    /// adults introduced on that face (as [`Command::IntroduceFounderOnFace`] with
    /// `stores` and `heading_rad`), each a member of it and none a pup; the first is the
    /// breeder. Refused off a support face, for a lineage with no pack table, and when the
    /// stores are refused. The caller (the habitat seeder, `--introduce chorister:N`)
    /// picks the covered den.
    FoundPack {
        den: Site,
        founder: Founder,
        stores: StartingStores,
        heading_rad: f64,
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

/// An animal tick between its parts ([`Fauna::begin_tick`] → [`Fauna::read_tick`] →
/// [`Fauna::finish_tick`]): the tick it produces and the clock and weather it began
/// with. Not state: it lives for one tick.
#[derive(Debug)]
pub struct FaunaTick(step::TickStart);

/// The animal layer. Owns its animals; borrows the world and the plant layer per call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fauna {
    /// Saved beside the state as a named-field document (`cubarium_voxel::savedoc`), not in
    /// its positional bytes: a new knob is not a new state layout.
    #[serde(skip)]
    config: FaunaConfig,
    /// Last world's (ring circumference, voxel size), rebuilt on introduction/step for
    /// exact death readouts that need cover geometry. Runtime cache, never saved.
    #[serde(skip)]
    world_geometry: Option<(f64, f64)>,
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
    /// Exact paid body creations since construction/load; runtime instrumentation only.
    #[serde(skip)]
    born_by_founder: [u64; Founder::COUNT],
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
    /// The controller stage's occupancy grids, kept warm between stages. Session state:
    /// not saved and not cloned.
    #[serde(skip)]
    grid_scratch: senses::LiveGrid,
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
    /// **Where in the night this tick is** (LURE, [`NightClock`]): set from the world's sun
    /// at the start of every tick, read by the activity windows. Derived, so not saved.
    #[serde(skip)]
    pub(crate) night: NightClock,
    /// **Where in the day this tick is** (RHYTHMS-CADENCE, [`Horizon`]): set from the world's
    /// sun at the start of every tick, read by the leanings. Derived, so not saved.
    #[serde(skip)]
    pub(crate) horizon: Horizon,
    /// **The day's phase this tick** (0 midnight, 0.5 noon; 0.5 with the clock off): set from
    /// the world's sun at the start of every tick, read by the v3 observation. Derived, so
    /// not saved.
    #[serde(skip)]
    pub(crate) day_phase: f64,
    /// **The executed-action tap** (contract v3, P0): while `Some`, every controller sample
    /// records its v3 observation and both labels ([`TapSample`]). Session state: not saved.
    #[serde(skip)]
    pub(crate) tap: Option<ActionTap>,
    /// The last tick rain fell (SNAIL-FIX: an amphibious body's roam hours run on for
    /// [`Amphibious::roam_after_rain_s`] after it). `None` before any rain.
    #[serde(default)]
    last_rain_tick: Option<u64>,
    /// Gut-seed voids since the last [`Fauna::take_seed_voids`] (F3): a census's events,
    /// not world state, so not saved; bounded at [`SEED_VOID_LOG_MAX`] (the oldest go).
    #[serde(skip)]
    seed_voids: Vec<SeedVoid>,
    /// Pack hunt and feast events since the last [`Fauna::take_pack_events`] (repair 3's
    /// measure): a census's events, not world state, so not saved; bounded like the seed
    /// voids.
    #[serde(skip)]
    pack_events: Vec<PackEvent>,
    /// The open air a flyer's egg site must reach ([`flight::OpenAir`]) and the tick it was
    /// built, refreshed after [`Flight::egg_air_refresh_s`]. Derived from the terrain, so
    /// not saved.
    #[serde(skip)]
    open_air: Option<(u64, std::sync::Arc<flight::OpenAir>)>,
    /// Per pack lineage ([`Founder::index`]): its walking route graph
    /// ([`encounter::RouteGraph`]) and the tick it was built, refreshed after
    /// [`Pack::route_refresh_s`]. Derived from the terrain, so not saved.
    #[serde(skip)]
    pack_routes: FxHashMap<usize, (u64, std::sync::Arc<encounter::RouteGraph>)>,
    /// Per pack id: the route field to its target's face (the graph it was walked on,
    /// that face, the distances from it), recomputed when the graph is rebuilt or the
    /// target changes face. Not saved.
    #[serde(skip)]
    pack_paths: FxHashMap<u64, PackPath>,
    /// Per pack id (PIT): the route field to its den's node, as `pack_paths` (a member walks
    /// home on it, not straight at the den). Not saved.
    #[serde(skip)]
    den_paths: FxHashMap<u64, PackPath>,
    /// Per hunter id (HUNTER-MEAL): the route field to the face it walks to, as
    /// `pack_paths`. Not saved.
    #[serde(skip)]
    hunt_paths: FxHashMap<u64, PackPath>,
    /// **Each body's percept record** (SENSES-CORE, [`Percept`]), by id: what its heuristics
    /// read of the other bodies, the scent and the tremor. Saved with the world (it holds the
    /// last-known positions a pursuit walks to); a dead body's is dropped.
    #[serde(default)]
    pub(crate) percepts: FxHashMap<u64, Percept>,
    /// **The scent field** (SENSES-CORE, [`scent::ScentField`]). Not saved: it refills
    /// within a few half-lives of a load.
    #[serde(skip)]
    pub(crate) scent: scent::ScentField,
    /// **The static beacons** (S3, [`light::Beacons`]): flowers and glowcaps, rebuilt on the
    /// plant clock. Not saved.
    #[serde(skip)]
    pub(crate) beacons: light::Beacons,
    /// The faces holding a usable carcass (carrion under a carcass record) and the tick the
    /// list was read from the plant layer (HUNTER-MEAL's meal step). Not saved.
    #[serde(skip)]
    carcass_sites: (Option<u64>, Vec<cubarium_voxel_flora::Site>),
    /// The packs standing in the world (roster R3), by id.
    #[serde(default)]
    packs: Vec<PackRecord>,
    /// The next pack's id: ids are never reused.
    #[serde(default)]
    next_pack: u64,
    /// The terrain sky visibility of the faces dark-active bodies stand on
    /// ([`Activity::DarkActive`]), keyed by face, and the `terrain_version` it was read
    /// against: pure terrain geometry, so derived and not saved.
    #[serde(skip)]
    face_sky: (Option<u64>, FxHashMap<(i64, u32, u32), f64>),
    /// EDGE's readout: per founder id, which layer of the write side last set its held
    /// actions ([`Steer`]). Session state for probes and studies: not saved.
    #[serde(skip)]
    steer: FxHashMap<u64, Steer>,
    /// EDGE: per body, the target a reflex walk is taking it to ([`Fauna::walk_key`]: a
    /// stand's id, or a face), the tick it set out (`teachers.press_replan_s` runs from there)
    /// and the last tick it walked there. Session state: not saved.
    #[serde(skip)]
    pub(crate) walk_targets: FxHashMap<u64, (u64, u64, u64)>,
    /// EDGE: per body, the target the last reflex walk to succeed this sample took and the
    /// layer it walks for; committed to `walk_targets` at the end of the body's write when
    /// that layer is the one held ([`Fauna::walk_commit`]). Not saved.
    #[serde(skip)]
    pub(crate) walk_pending: FxHashMap<u64, (u64, Steer)>,
    /// EDGE: per grounded founder, the ticks it has **pressed** without a break: controller
    /// intervals in which it held forward and its motor delivered under
    /// `teachers.motor_refused` of what it attempted. Session state: not saved.
    #[serde(skip)]
    pub(crate) pressed: FxHashMap<u64, u64>,
    /// EDGE: per body, the targets it gave up for pressing and the tick it may try each
    /// again, at most [`WALK_SHUN_SLOTS`]. Session state: not saved.
    #[serde(skip)]
    pub(crate) walk_shun: FxHashMap<u64, Vec<(u64, u64)>>,
    /// EDGE: per body, the flight (its `Vigil::flee_tick`) in which it was cornered: the rest
    /// of that flight goes by its route graph. Session state: not saved.
    #[serde(skip)]
    pub(crate) route_flee: FxHashMap<u64, u64>,
    /// FORAGE-LEAVE: per heuristic body, its patch record — the stay under way and the
    /// patch it gave up (`step::forage`). Session state: not saved.
    #[serde(skip)]
    pub(crate) forage: FxHashMap<u64, step::forage::Patch>,
    /// HUNGRY-SEARCH: per heuristic forager, its visited-ground memory, its patience and
    /// the search leg it is on (`step::rove`). Session state: not saved.
    #[serde(skip)]
    pub(crate) rove: FxHashMap<u64, step::rove::Rove>,
    /// FORAGE-LEAVE's readout per lineage ([`ForageTally`]). Session state: not saved.
    #[serde(skip)]
    pub(crate) forage_tally: [ForageTally; Founder::COUNT],
    /// LEADS-PAY's readout: leads given up per lineage (`step::rove`). Session state: not
    /// saved.
    #[serde(skip)]
    pub(crate) lead_give_ups: [u64; Founder::COUNT],
    /// HERD-PULL's readout per lineage ([`HerdTally`]). Session state: not saved.
    #[serde(skip)]
    pub(crate) herd_tally: [HerdTally; Founder::COUNT],
    /// ROOSTS (backlog §12): the latticevine ledges a glider may roost on ([`Ledge`]) and
    /// the tick they were read, re-read at the teachers' `route_refresh_s`. Derived from the
    /// terrain and the cover: not saved.
    #[serde(skip)]
    pub(crate) ledges: (Option<u64>, Vec<Ledge>),
    /// DROP: per grounded body, its trapped clock (`step::jump::Trapped`). Session state:
    /// not saved.
    #[serde(skip)]
    pub(crate) trapped: FxHashMap<u64, step::jump::Trapped>,
    /// DROP: per trapped body, the jump down it is taking (`step::jump::DropPlan`). Session
    /// state: not saved.
    #[serde(skip)]
    pub(crate) drop_plans: FxHashMap<u64, step::jump::DropPlan>,
}

/// **Which layer set a founder's held actions** on its last controller sample (EDGE's
/// readout, [`Fauna::steer_of`]): its own heuristic, or the reflex that overrode it last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Steer {
    /// Not driven (no controller, or a non-finite observation): rest.
    Rest,
    /// Its controller's own actions.
    Heuristic,
    /// A glider's fruit search (`glider::fruit_reflex`).
    Fruit,
    /// The flee rule or the ground response (herd, arboreal refuge or away, freeze).
    Flee,
    /// A glider's roost.
    Roost,
    /// Fed perching.
    FedRest,
    /// The shallows reflex.
    Shallows,
    /// An amphibious body's roam or water reflex.
    Roam,
    /// The lanternjaw's patrol.
    Patrol,
    /// A carcass yield.
    Yield,
    /// The pack tactic.
    Pack,
    /// The light, loom or glow steering.
    Light,
    /// DROP: a trapped body's jump down.
    Drop,
    /// FORAGE-LEAVE: the walk off a patch given up.
    Leave,
    /// HUNGRY-SEARCH: a hungry forager's search leg.
    Search,
    /// VINE-C: a vine nester's hold at, or flight to, a nest site.
    Nest,
}

impl Steer {
    /// Every source, in a fixed order for tables.
    pub const ALL: [Steer; 16] = [
        Steer::Rest,
        Steer::Heuristic,
        Steer::Fruit,
        Steer::Flee,
        Steer::Roost,
        Steer::FedRest,
        Steer::Shallows,
        Steer::Roam,
        Steer::Patrol,
        Steer::Yield,
        Steer::Pack,
        Steer::Light,
        Steer::Drop,
        Steer::Leave,
        Steer::Search,
        Steer::Nest,
    ];

    /// A short name for tables.
    pub fn name(self) -> &'static str {
        match self {
            Steer::Rest => "rest",
            Steer::Heuristic => "heuristic",
            Steer::Fruit => "fruit",
            Steer::Flee => "flee",
            Steer::Roost => "roost",
            Steer::FedRest => "fed_rest",
            Steer::Shallows => "shallows",
            Steer::Roam => "roam",
            Steer::Patrol => "patrol",
            Steer::Yield => "yield",
            Steer::Pack => "pack",
            Steer::Light => "light",
            Steer::Drop => "drop",
            Steer::Leave => "leave",
            Steer::Search => "search",
            Steer::Nest => "nest",
        }
    }
}

/// **What a body held**, as the executed-action tap records it (contract v3, P0): the three
/// local actions and the body's own ([`BodyActions`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Held {
    pub actions: Actions,
    pub body: BodyActions,
}

/// **One sample of the executed-action tap** (contract v3, P0;
/// `design/brain-contract-v3-proposal-2026-10-08.md` §2.3 "Seeding" and §4 P0): at one of
/// a body's controller samples, the v3 observation it was sampled on, the two labels, and
/// which layer set the held actions.
///
/// - `own` is the **pre-override** label: its controller's own answer through the shared
///   adapter, before any reflex (the hybrid's option 0 clones this).
/// - `executed` is what the write side **held** after every layer (B's clone fits this):
///   the actions, the lure and the lift as held, and the hunker and retraction the body is
///   in. The sprint is P2's (the gait decides it after this stage) and reads 0.
/// - `steer` is the layer that set it ([`Steer`]); [`Steer::Heuristic`] means no override,
///   and then `own == executed` but for the stop-and-go pace and the body's states.
#[derive(Clone, Debug, PartialEq)]
pub struct TapSample {
    pub tick: u64,
    pub observation: Vec<f64>,
    pub own: Held,
    pub executed: Held,
    pub steer: Steer,
}

/// The tap's recording: one stream per body, by id ([`Fauna::tap`]).
#[derive(Clone, Debug, Default)]
pub struct ActionTap {
    streams: std::collections::BTreeMap<u64, (Founder, Vec<TapSample>)>,
}

impl ActionTap {
    /// Body `id`'s stream so far.
    pub fn stream(&self, id: u64) -> Option<&[TapSample]> {
        self.streams.get(&id).map(|(_, s)| s.as_slice())
    }

    /// Every body's stream, in id order: `(id, founder, samples)`.
    pub fn streams(&self) -> impl Iterator<Item = (u64, Founder, &[TapSample])> + '_ {
        self.streams.iter().map(|(&id, (f, s))| (id, *f, s.as_slice()))
    }

    /// A recording from per-body streams (tools and tests that read a saved one).
    pub fn from_streams(streams: impl IntoIterator<Item = (u64, Founder, Vec<TapSample>)>) -> ActionTap {
        ActionTap { streams: streams.into_iter().map(|(id, f, s)| (id, (f, s))).collect() }
    }

    pub(crate) fn push(&mut self, id: u64, founder: Founder, sample: TapSample) {
        self.streams.entry(id).or_insert_with(|| (founder, Vec::new())).1.push(sample);
    }
}

/// The most targets a body keeps given up at once ([`Fauna::walk_shun`]): a memory bound,
/// not a knob; the soonest to lapse goes first.
pub(crate) const WALK_SHUN_SLOTS: usize = 8;

/// The most gut-seed void events [`Fauna`] keeps undrained.
pub const SEED_VOID_LOG_MAX: usize = 65_536;

/// A pack's cached route field to its target ([`Fauna::pack_path`]).
#[derive(Clone, Debug)]
pub(crate) struct PackPath {
    graph: std::sync::Arc<encounter::RouteGraph>,
    goal: usize,
    /// The steps the field reaches out to.
    steps: u32,
    field: std::sync::Arc<FxHashMap<usize, u32>>,
}

/// One pack hunt or feast event (the minimal gut's repair 3 measure); not world state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PackEvent {
    /// A pack took a target, with this many participants (the taker and its
    /// participating mates within `pack_radius_m`).
    /// `taker` is the member that chose `target` and `score` its choice score; `prev` is the
    /// pack's last target dropped alive with the score the taker gave it now (`None` when it
    /// does not sense it), the re-target's comparison (CHURN's readout).
    ChaseStart { tick: u64, pack: u64, participants: u32, taker: u64, target: u64, score: f64, prev: Option<u64>, prev_score: Option<f64> },
    /// A pack's target was dropped, and why; `by` is the member whose sampling dropped it.
    ChaseEnd { tick: u64, pack: u64, why: ChaseEnd, target: u64, by: u64 },
    /// A pack's feast ended, with the carrion its members ate there, and why.
    FeastEnd { tick: u64, pack: u64, site: Site, eaten: f64, why: FeastEnd },
}

/// Why a chase ended ([`PackEvent::ChaseEnd`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChaseEnd {
    /// The target is gone and carrion lies where it was last seen.
    Killed,
    /// The target is gone with no carrion there (it left the world otherwise).
    Gone,
    /// `chase_give_up_s` passed.
    Late,
    /// A member that wants no food was under the chase floor.
    Spent,
    /// The target stood in another pack's territory.
    Trespass,
    /// No member had sensed it within the pursuit's memory (SENSES-CORE,
    /// [`PerceptionConfig::give_up_s`]).
    Lost,
    /// Its prospect fell under `Pack::break_off_below` (PACK-TUNE: the test went badly).
    BrokeOff,
    /// Another target scored past `Pack::switch_margin` of it and took the hunt (CHURN).
    Switched,
}

/// Why a feast ended ([`PackEvent::FeastEnd`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeastEnd {
    /// Its carcass was gone: eaten or decayed to the plant layer's `carcass_gone_fraction`.
    Emptied,
    /// A new kill elsewhere became the pack's feast.
    Replaced,
}

impl Fauna {
    /// Lineage `founder`'s walking route components by face, rebuilt from `view` when
    /// older than its pack's `route_refresh_s` (or never built).
    pub(crate) fn pack_routes(
        &mut self,
        view: &VoxelView<'_>,
        founder: Founder,
    ) -> std::sync::Arc<encounter::RouteGraph> {
        let every = self.config.founder(founder).pack.map_or(0.0, |p| p.route_refresh_s);
        self.routes(view, founder, every)
    }

    /// Lineage `founder`'s walking route graph, rebuilt when older than `every` seconds
    /// (the pack's [`Pack::route_refresh_s`], the patrol's [`HuntConfig::route_refresh_s`]).
    /// One cache per lineage, whichever asks.
    pub(crate) fn routes(
        &mut self,
        view: &VoxelView<'_>,
        founder: Founder,
        every: f64,
    ) -> std::sync::Arc<encounter::RouteGraph> {
        let phys = *self.config.founder(founder);
        let tick = self.tick;
        if let Some((built, map)) = self.pack_routes.get(&founder.index()) {
            if tick.saturating_sub(*built) < cubarium_voxel::ticks_at_least_one(every) {
                return map.clone();
            }
        }
        let map = std::sync::Arc::new(encounter::RouteGraph::for_founder(view, &phys, self.config.founding.clearance_margin_m));
        self.pack_routes.insert(founder.index(), (tick, map.clone()));
        map
    }

    /// Hunter `id`'s route field to node `goal` of `graph`, out to `max_steps`: cached
    /// until the graph is rebuilt or the goal changes (HUNTER-MEAL).
    pub(crate) fn hunt_path(
        &mut self,
        graph: &std::sync::Arc<encounter::RouteGraph>,
        id: u64,
        goal: usize,
        max_steps: u32,
    ) -> std::sync::Arc<FxHashMap<usize, u32>> {
        if let Some(p) = self.hunt_paths.get(&id) {
            if std::sync::Arc::ptr_eq(&p.graph, graph) && p.goal == goal && p.steps >= max_steps {
                return p.field.clone();
            }
        }
        let field = std::sync::Arc::new(graph.distances(goal, max_steps));
        self.hunt_paths.insert(id, PackPath { graph: graph.clone(), goal, steps: max_steps, field: field.clone() });
        field
    }

    /// The faces holding a usable carcass — carrion under the plant layer's carcass record —
    /// read once a tick (HUNTER-MEAL).
    /// Whether the scent field runs: a body of a live lineage ([`SenseTable::live`]) that
    /// smells is alive.
    pub(crate) fn scent_wanted(&self) -> bool {
        // S2: a patrolling hunter's patch choice reads its prey's scent too.
        let patrol_smells = self.config.hunt.patrol && self.config.hunt.patch_scent_weight > 0.0;
        self.animals.iter().any(|a| a.founder.is_some_and(|f| {
            let t = self.config.founder(f).senses;
            (t.live && t.smell_min > 0.0) || (patrol_smells && self.config.founder(f).active_hunter().is_some() && self.config.founder(f).pack.is_none())
        }))
    }

    /// **A body's percept record** (SENSES-CORE, [`Percept`]): what its heuristics read.
    pub fn percept(&self, id: u64) -> Option<&Percept> {
        self.percepts.get(&id)
    }

    /// The scent level of channel `ch` on the support face `site` (a readout; 0 off the
    /// field).
    pub fn scent_level(&self, view: &VoxelView<'_>, ch: ScentChannel, site: Site) -> f64 {
        self.scent.level(view, ch, i64::from(site.x), site.y, site.z)
    }

    /// The carcass faces as [`Fauna::carcass_sites`] last read them, if it read them this
    /// tick; else none (the v3 observation's read side, which cannot refresh the list).
    pub(crate) fn carcass_list(&self) -> &[cubarium_voxel_flora::Site] {
        if self.carcass_sites.0 == Some(self.tick) { &self.carcass_sites.1 } else { &[] }
    }

    pub(crate) fn carcass_sites(&mut self, fv: &cubarium_voxel_flora::FloraView<'_>) -> &[cubarium_voxel_flora::Site] {
        if self.carcass_sites.0 != Some(self.tick) {
            let list = &mut self.carcass_sites.1;
            list.clear();
            list.extend(fv.ground.iter().filter(|g| g.carcass.is_some() && g.carrion > 0.0).map(|g| g.site));
            self.carcass_sites.0 = Some(self.tick);
        }
        &self.carcass_sites.1
    }

    /// Pack `pack`'s route field to face `goal` on lineage `founder`'s graph, out to
    /// `max_steps`: cached until the graph is rebuilt or the goal changes.
    pub(crate) fn pack_path(
        &mut self,
        view: &VoxelView<'_>,
        founder: Founder,
        pack: u64,
        goal: usize,
        max_steps: u32,
    ) -> (std::sync::Arc<encounter::RouteGraph>, std::sync::Arc<FxHashMap<usize, u32>>) {
        let graph = self.pack_routes(view, founder);
        if let Some(p) = self.pack_paths.get(&pack) {
            if std::sync::Arc::ptr_eq(&p.graph, &graph) && p.goal == goal && p.steps >= max_steps {
                return (graph, p.field.clone());
            }
        }
        let field = std::sync::Arc::new(graph.distances(goal, max_steps));
        self.pack_paths.insert(pack, PackPath { graph: graph.clone(), goal, steps: max_steps, field: field.clone() });
        (graph, field)
    }

    /// Pack `pack`'s route field to its den's node `goal` on lineage `founder`'s graph (PIT),
    /// over the whole graph: cached until the graph is rebuilt or the node changes.
    pub(crate) fn den_path(
        &mut self,
        view: &VoxelView<'_>,
        founder: Founder,
        pack: u64,
        goal: usize,
    ) -> (std::sync::Arc<encounter::RouteGraph>, std::sync::Arc<FxHashMap<usize, u32>>) {
        let graph = self.pack_routes(view, founder);
        if let Some(p) = self.den_paths.get(&pack) {
            if std::sync::Arc::ptr_eq(&p.graph, &graph) && p.goal == goal {
                return (graph, p.field.clone());
            }
        }
        let field = std::sync::Arc::new(graph.distances(goal, u32::MAX));
        self.den_paths.insert(pack, PackPath { graph: graph.clone(), goal, steps: u32::MAX, field: field.clone() });
        (graph, field)
    }

    /// The open air under `flight`'s band, rebuilt from `view` when older than its
    /// `egg_air_refresh_s` (or never built).
    pub(crate) fn open_air(&mut self, view: &VoxelView<'_>, flight: &Flight) -> std::sync::Arc<flight::OpenAir> {
        let tick = self.tick;
        if let Some((built, air)) = &self.open_air {
            if tick.saturating_sub(*built) < cubarium_voxel::ticks_at_least_one(flight.egg_air_refresh_s) {
                return air.clone();
            }
        }
        let air = std::sync::Arc::new(flight::OpenAir::build(view, flight.band_max_m));
        self.open_air = Some((tick, air.clone()));
        air
    }

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
            world_geometry: None,
            tick: 0,
            animals: Vec::new(),
            clutches: Vec::new(),
            ledger: FaunaLedger::default(),
            born_by_founder: [0; Founder::COUNT],
            births_enabled: true,
            controllers: FounderControllers::default(),
            grid_scratch: senses::LiveGrid::default(),
            factories: FounderFactories::default(),
            policy_driven: [false; Founder::COUNT],
            policy_digest: [0; Founder::COUNT],
            last_daylight: None,
            night: NightClock::DAY,
            horizon: Horizon::NOON,
            day_phase: 0.5,
            tap: None,
            last_rain_tick: None,
            seed_voids: Vec::new(),
            pack_events: Vec::new(),
            pack_routes: FxHashMap::default(),
            open_air: None,
            pack_paths: FxHashMap::default(),
            den_paths: FxHashMap::default(),
            percepts: FxHashMap::default(),
            scent: scent::ScentField::default(),
            beacons: light::Beacons::default(),
            hunt_paths: FxHashMap::default(),
            carcass_sites: (None, Vec::new()),
            packs: Vec::new(),
            next_pack: 0,
            face_sky: (None, FxHashMap::default()),
            steer: FxHashMap::default(),
            walk_targets: FxHashMap::default(),
            walk_pending: FxHashMap::default(),
            pressed: FxHashMap::default(),
            trapped: FxHashMap::default(),
            drop_plans: FxHashMap::default(),
            walk_shun: FxHashMap::default(),
            route_flee: FxHashMap::default(),
            forage: FxHashMap::default(),
            rove: FxHashMap::default(),
            forage_tally: [ForageTally::default(); Founder::COUNT],
            lead_give_ups: [0; Founder::COUNT],
            herd_tally: [HerdTally::default(); Founder::COUNT],
            ledges: (None, Vec::new()),
        })
    }

    /// **Start the executed-action tap** (contract v3, P0), empty: from the next tick every
    /// founder's controller sample records its v3 observation, its controller's own answer
    /// and what was held ([`TapSample`]). Recording reads only: a tapped world steps exactly
    /// as an untapped one.
    pub fn start_tap(&mut self) {
        self.tap = Some(ActionTap::default());
    }

    /// Stop the tap and hand back what it recorded (`None` if it was not running).
    pub fn take_tap(&mut self) -> Option<ActionTap> {
        self.tap.take()
    }

    /// The tap's recording so far, if it is running.
    pub fn tap(&self) -> Option<&ActionTap> {
        self.tap.as_ref()
    }

    /// EDGE's readout: which layer set the founder `id`'s held actions on its last
    /// controller sample, `None` before its first.
    pub fn steer_of(&self, id: u64) -> Option<Steer> {
        self.steer.get(&id).copied()
    }

    /// EDGE's readout: the ticks founder `id` has pressed without a break, the target its
    /// reflex walk last took (key, set out, last walked) and the targets it has given up.
    pub fn press_of(&self, id: u64) -> (u64, Option<(u64, u64, u64)>, Vec<(u64, u64)>) {
        (
            self.pressed.get(&id).copied().unwrap_or(0),
            self.walk_targets.get(&id).copied(),
            self.walk_shun.get(&id).cloned().unwrap_or_default(),
        )
    }

    /// DROP's readout: founder `id`'s trapped clock, `(since, cornered)`: the tick it was
    /// first found cornered off its route graph and whether its last reading was. `None`
    /// when it was never read (a flyer, or a body whose jump reach is within its step).
    pub fn trapped_of(&self, id: u64) -> Option<(u64, bool)> {
        self.trapped.get(&id).map(|t| (t.since, t.cornered))
    }

    /// EDGE: the key a reflex walk's target goes by: a stand by its id, a face by its
    /// coordinates under the top bit.
    pub(crate) fn walk_key_face(site: Site) -> u64 {
        (1 << 63) | u64::from(site.x) | (u64::from(site.y) << 21) | (u64::from(site.z) << 42)
    }

    /// **EDGE, a pressed walk re-plans** — call first in a reflex walk's sample for body
    /// `id` of `founder`: when the target it has walked to for `teachers.press_replan_s`
    /// (taken again at least every two controller periods) has had it pressing that long
    /// without a break ([`Fauna::pressed`]), that target is given up for
    /// `teachers.press_shun_s`. Returns the targets given up now.
    pub(crate) fn walk_shunned(&mut self, id: u64, founder: Founder) -> Vec<u64> {
        let now = self.tick;
        let tc = self.config.teachers;
        let ticks = |s: f64| if s.is_finite() && s > 0.0 { cubarium_voxel::ticks_at_least_one(s) } else { u64::MAX };
        let press = ticks(tc.press_replan_s);
        let gap = 2 * founder.manifest().cadence_ticks().max(1);
        if let Some(&(t, since, last)) = self.walk_targets.get(&id) {
            let pressed = self.pressed.get(&id).copied().unwrap_or(0);
            if now.saturating_sub(last) <= gap && now.saturating_sub(since) >= press && pressed >= press {
                let until = now.saturating_add(ticks(tc.press_shun_s).min(u64::MAX / 2));
                let list = self.walk_shun.entry(id).or_default();
                list.retain(|&(k, u)| k != t && now < u);
                if list.len() >= WALK_SHUN_SLOTS {
                    list.sort_by_key(|&(_, u)| std::cmp::Reverse(u));
                    list.truncate(WALK_SHUN_SLOTS - 1);
                }
                list.push((t, until));
                self.walk_targets.remove(&id);
            }
        }
        self.walk_shun.get(&id).map_or(Vec::new(), |l| l.iter().filter(|&&(_, u)| now < u).map(|&(k, _)| k).collect())
    }

    /// FORAGE-LEAVE: body `id` gives up target `key` (a stand it fed from) until tick
    /// `until`, in the same list a pressed walk's targets go to ([`Fauna::walk_shunned`]).
    pub(crate) fn shun_target(&mut self, id: u64, key: u64, until: u64) {
        let now = self.tick;
        let list = self.walk_shun.entry(id).or_default();
        list.retain(|&(k, u)| k != key && now < u);
        if list.len() >= WALK_SHUN_SLOTS {
            list.sort_by_key(|&(_, u)| std::cmp::Reverse(u));
            list.truncate(WALK_SHUN_SLOTS - 1);
        }
        list.push((key, until));
    }

    /// EDGE: a reflex walk for layer `kind` takes body `id` to target `key` this sample.
    pub(crate) fn walking_to(&mut self, id: u64, key: u64, kind: Steer) {
        self.walk_pending.insert(id, (key, kind));
    }

    /// EDGE: the end of body `id`'s write, `held` the layer whose actions it holds: a walk
    /// that layer took starts or continues [`Fauna::walk_shunned`]'s clock.
    pub(crate) fn walk_commit(&mut self, id: u64, founder: Founder, held: Steer) {
        let Some((key, kind)) = self.walk_pending.remove(&id) else { return };
        if kind != held {
            return;
        }
        let now = self.tick;
        let gap = 2 * founder.manifest().cadence_ticks().max(1);
        let since = match self.walk_targets.get(&id) {
            Some(&(t, since, last)) if t == key && now.saturating_sub(last) <= gap => since,
            _ => now,
        };
        self.walk_targets.insert(id, (key, since, now));
    }

    /// The terrain sky visibility of face `(x, y, z)` ([`VoxelView::sky_visibility`]),
    /// cached per face until the terrain moves.
    pub(crate) fn face_sky(&mut self, view: &VoxelView<'_>, x: i64, y: u32, z: u32) -> f64 {
        let version = view.terrain_version;
        if self.face_sky.0 != Some(version) {
            self.face_sky.0 = Some(version);
            self.face_sky.1.clear();
        }
        *self.face_sky.1.entry((x, y, z)).or_insert_with(|| view.sky_visibility(x, y, z))
    }

    /// The gut-seed voids since the last call (F3), oldest first.
    pub fn take_seed_voids(&mut self) -> Vec<SeedVoid> {
        std::mem::take(&mut self.seed_voids)
    }

    /// Drain the pack hunt and feast events ([`PackEvent`]).
    pub fn take_pack_events(&mut self) -> Vec<PackEvent> {
        std::mem::take(&mut self.pack_events)
    }

    pub(crate) fn log_pack_event(&mut self, e: PackEvent) {
        if self.pack_events.len() >= SEED_VOID_LOG_MAX {
            self.pack_events.drain(..SEED_VOID_LOG_MAX / 2);
        }
        self.pack_events.push(e);
    }

    pub(crate) fn log_seed_void(&mut self, v: SeedVoid) {
        if self.seed_voids.len() >= SEED_VOID_LOG_MAX {
            self.seed_voids.drain(..SEED_VOID_LOG_MAX / 2);
        }
        self.seed_voids.push(v);
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

    /// Whether body `animal_id` has a controller: a founder body without one rests where
    /// it stands (a loader that forgot to put them back; the staged runs' liveness check).
    pub fn has_controller(&self, animal_id: u64) -> bool {
        self.controllers.has(animal_id)
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

    /// **A loaded layer's configuration replacement** (the episode initializer,
    /// `design/handoffs/assembly-step1-2026-10-03.md`): `config` replaces the saved one
    /// whole, after [`FaunaConfig::validate`], and the route and air caches built from the
    /// old one are dropped. A changed lineage mass scale rescales its held matter, including
    /// eggs, and books the signed difference as introduced matter. Per-unit rates, clocks,
    /// positions and historical flows stay as saved. The controllers'
    /// recipes read the config when they are installed, so a caller reinstalls them after.
    #[doc(hidden)]
    pub fn replace_config(&mut self, config: FaunaConfig) -> Result<(), String> {
        config.validate()?;
        for a in &mut self.animals {
            let Some(f) = a.founder else { continue };
            let k = config.founder(f).mass_scale / self.config.founder(f).mass_scale;
            if k == 1.0 {
                continue;
            }
            let before = [a.stored_organic(), a.stored_mineral(), a.stored_energy()];
            a.body *= k;
            a.reserve *= k;
            a.mineral *= k;
            a.energy *= k;
            // Brood is a label inside the reserve, not additional matter.
            a.reproduction.brood *= k;
            for t in std::iter::once(&mut a.gut).chain(a.stomach.pools.iter_mut()) {
                t.organic *= k;
                t.mineral *= k;
                t.energy *= k;
            }
            if let Some(e) = &mut a.reproduction.escrow {
                e.organic *= k;
                e.mineral *= k;
                e.energy *= k;
            }
            for lot in a.gut_seeds.lots.iter_mut().filter(|l| l.plant.is_some()) {
                lot.organic *= k;
                lot.mineral *= k;
                lot.energy *= k;
            }
            self.ledger.introduced_organic_in += a.stored_organic() - before[0];
            self.ledger.introduced_mineral_in += a.stored_mineral() - before[1];
            self.ledger.introduced_energy_in += a.stored_energy() - before[2];
        }
        // egg_organic follows SpeciesConfig::scale_mass; a clutch is a paid package
        // of that lineage's future bodies. Keep egg counts and incubation clocks.
        for c in &mut self.clutches {
            let k = config.founder(c.lineage).mass_scale / self.config.founder(c.lineage).mass_scale;
            if k == 1.0 {
                continue;
            }
            let before = [c.organic, c.mineral, c.energy];
            c.organic *= k;
            c.mineral *= k;
            c.energy *= k;
            self.ledger.introduced_organic_in += c.organic - before[0];
            self.ledger.introduced_mineral_in += c.mineral - before[1];
            self.ledger.introduced_energy_in += c.energy - before[2];
        }
        self.config = config;
        self.pack_routes.clear();
        self.pack_paths.clear();
        self.den_paths.clear();
        self.open_air = None;
        Ok(())
    }
    /// Paid live births and hatchlings by lineage since construction/load. Introductions
    /// are excluded; callers take interval differences. This counter is never saved.
    pub fn born_by_founder(&self) -> [u64; Founder::COUNT] {
        self.born_by_founder
    }

    /// Whether paid births run. Live-world default: `true`.
    pub fn births_enabled(&self) -> bool {
        self.births_enabled
    }

    /// Turn births on or off. An isolated arena turns them off; the live world never does.
    pub fn set_births_enabled(&mut self, enabled: bool) {
        self.births_enabled = enabled;
    }

    /// DROP's readout for body `id`: the tick its trapped clock started (if it is cornered now),
    /// and the jump it is taking (`from`, `land`).
    pub fn drop_state(&self, id: u64) -> (Option<u64>, Option<(Site, Site)>) {
        (
            self.trapped.get(&id).filter(|t| t.cornered).map(|t| t.since),
            self.drop_plans.get(&id).map(|p| (p.from, p.land)),
        )
    }

    /// **A probe's placement** (DROP's `hunter_study drop`): the grounded body `id` stands on
    /// `site`, centred on its column, its trapped clock and jump cleared. No check of the face:
    /// the probe picks it. `false` when there is no such body.
    pub fn probe_place(&mut self, id: u64, site: Site, voxel_m: f64) -> bool {
        let Ok(i) = self.animals.binary_search_by_key(&id, |a| a.id) else { return false };
        let a = &mut self.animals[i];
        a.site = site;
        a.pose.x = (f64::from(site.x) + 0.5) * voxel_m;
        a.pose.z = (f64::from(site.z) + 0.5) * voxel_m;
        a.pose.y_m = surface_m(site.y, voxel_m);
        a.mobility = Mobility::default();
        self.trapped.remove(&id);
        self.drop_plans.remove(&id);
        true
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// The packs standing in the world (roster R3), by id.
    pub fn packs(&self) -> &[PackRecord] {
        &self.packs
    }

    /// One pack, by id.
    pub fn pack(&self, id: u64) -> Option<&PackRecord> {
        self.packs.iter().find(|p| p.id == id)
    }

    pub(crate) fn pack_mut(&mut self, id: u64) -> Option<&mut PackRecord> {
        self.packs.iter_mut().find(|p| p.id == id)
    }

    pub(crate) fn packs_mut(&mut self) -> &mut Vec<PackRecord> {
        &mut self.packs
    }

    /// **A litter born now** (PUP, a scenario control): pack `pack`'s breeder pays the
    /// lineage's package for `young` pups out of its reserve, exactly as a gestation would
    /// have paid it, and they are born at the den, each with an even share; the litter counts
    /// against the breeder's interval and the year's season as a born litter would. Refused
    /// (`Err`) for a pack with no breeder, or a breeder whose reserve cannot pay. Returns the
    /// pups' ids.
    pub fn whelp(&mut self, view: &VoxelView<'_>, pack: u64, young: u32) -> Result<Vec<u64>, String> {
        step::whelp(self, view, pack, young)
    }

    /// **A body's reserve set to `fraction` of its cap** (PUP, a scenario control: a lean year's
    /// thin adults). Organic matter, mineral and energy move pro rata with the reserve's own
    /// composition (the body's, for a body with none); a cut is booked `removed_*_out` and a
    /// rise `introduced_*_in`, so the books close. `false` for no such body.
    pub fn set_reserve_fraction(&mut self, id: u64, fraction: f64) -> bool {
        let Some(i) = self.animals.iter().position(|a| a.id == id) else {
            return false;
        };
        let sc = body::effective_config(&self.config, &self.animals[i]);
        let a = &mut self.animals[i];
        let want = fraction.max(0.0) * sc.reserve_of(a.body);
        let held = a.organic();
        let d = want - a.reserve;
        let (dm, de) = if held > 0.0 { (a.mineral * d / held, a.energy * d / held) } else { (0.0, 0.0) };
        a.reserve = want;
        a.mineral += dm;
        a.energy += de;
        let l = &mut self.ledger;
        if d >= 0.0 {
            l.introduced_organic_in += d;
            l.introduced_mineral_in += dm;
            l.introduced_energy_in += de;
        } else {
            l.removed_organic_out -= d;
            l.removed_mineral_out -= dm;
            l.removed_energy_out -= de;
        }
        true
    }

    /// The breeder's den pick (the habitat seeder's founding search): the pack's den, and
    /// how it was chosen. The pack may have been founded on a face beside it (a covered
    /// pocket its adults cannot stand in); it lives within `den_radius_m` of it.
    pub fn set_den(&mut self, pack: u64, den: Site, choice: DenChoice) {
        if let Some(p) = self.pack_mut(pack) {
            p.den = den;
            p.den_choice = Some(choice);
        }
    }

    /// [`Command::FoundPack`]: a new pack on `den`, its `pack_size` adults introduced on
    /// that face, the first the breeder. The new pack's id, or `None` (nothing changed)
    /// for a lineage with no pack table, a face that is not one, or refused stores.
    fn found_pack(
        &mut self,
        world: &World,
        den: Site,
        founder: Founder,
        stores: StartingStores,
        heading_rad: f64,
    ) -> Option<u64> {
        let size = self.config.founder(founder).pack?.pack_size;
        self.found_pack_sized(world, den, founder, stores, heading_rad, size)
    }

    /// [`Command::FoundPack`] with `size` adults instead of the table's `pack_size`: a
    /// constructed start's pack sized to the prey present
    /// (`design/handoffs/assembly-step1-2026-10-03.md`). Booked as every introduced adult
    /// is; `None` (nothing changed) as [`Command::FoundPack`] refuses, or for size 0.
    pub fn found_pack_sized(
        &mut self,
        world: &World,
        den: Site,
        founder: Founder,
        stores: StartingStores,
        heading_rad: f64,
        size: usize,
    ) -> Option<u64> {
        self.config.founder(founder).pack?;
        let id = self.next_pack;
        let mut members = Vec::new();
        for _ in 0..size {
            if !self.apply(
                world,
                Command::IntroduceFounderOnFace {
                    site: den,
                    founder,
                    stores,
                    heading_rad,
                },
            ) {
                break;
            }
            members.push(self.ledger.births - 1);
        }
        if members.is_empty() {
            return None;
        }
        for &m in &members {
            if let Ok(i) = self.animals.binary_search_by_key(&m, |a| a.id) {
                self.animals[i].pack = Some(PackMember {
                    pack: id,
                    pup: false,
                    calling_ticks: 0,
                });
            }
        }
        self.next_pack += 1;
        self.packs.push(PackRecord {
            id,
            lineage: founder,
            den,
            breeder: members.first().copied(),
            den_choice: None,
            last_litter_tick: None,
            target: None,
            chase_since_tick: 0,
            chase_progress_tick: 0,
            chase_best_m: f64::INFINITY,
            chase_work: 0.0,
            target_site: None,
            feast: None,
            feast_eaten: 0.0,
            abandoned: None,
            shunned: [(0, 0); 4],
            drawn: None,
            drawn_scent: false,
            season: Season::default(),
        });
        Some(id)
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
        step::step(self, world, flora, 1);
    }

    /// [`Fauna::step`], taking the host's thread count: the controller stage's read side
    /// (occupancy windows, observations, controllers) splits across the compute pool.
    pub fn step_with(&mut self, world: &World, flora: &mut Flora, threads: usize) {
        step::step(self, world, flora, threads);
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
        step::step_with_senses(self, world, flora, senses, threads);
    }

    /// **One tick in three parts**, for a schedule that runs the read side beside the
    /// plant layer's own reads: [`Fauna::begin_tick`], [`Fauna::read_tick`],
    /// [`Fauna::finish_tick`], in that order and nothing of this layer's in between, are
    /// [`Fauna::step_with_senses`] (or [`Fauna::step_with`] with no senses).
    ///
    /// The first part: the clock, the terrain and maintenance. Its voiding (dung, gut
    /// seeds) writes the plant layer's ground.
    pub fn begin_tick(&mut self, world: &World, flora: &mut Flora) -> FaunaTick {
        cubarium_voxel::voxel_phase!(FaunaStep, { FaunaTick(step::begin(self, world, flora)) })
    }

    /// The read side: the litter field's due update and the controller stage. Reads the
    /// plant layer and writes nothing of it.
    pub fn read_tick(
        &mut self,
        tick: &FaunaTick,
        world: &World,
        flora: &Flora,
        senses: Option<&mut Senses>,
        threads: usize,
    ) {
        cubarium_voxel::voxel_phase!(FaunaStep, {
            step::read(self, world, flora, senses, &tick.0, threads)
        });
    }

    /// The writes: the act (feeding through the plant layer's `take_*`), births, deaths
    /// and the burr pass.
    pub fn finish_tick(&mut self, tick: FaunaTick, world: &World, flora: &mut Flora) {
        cubarium_voxel::voxel_phase!(FaunaStep, { step::finish(self, world, flora, tick.0) });
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
                // A flyer is refused where its disc does not fit on the face (F1).
                let phys = self.config.founder(founder);
                if phys.flight.is_some()
                    && !flight::flyer_fits_on_face(&world.view(), phys, body, site, 0.0)
                {
                    return false;
                }
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
                // A flyer is refused where its disc does not fit on the face (F1).
                let phys = self.config.founder(founder);
                if phys.flight.is_some()
                    && !flight::flyer_fits_on_face(&world.view(), phys, body, site, 0.0)
                {
                    return false;
                }
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
            Command::FoundPack {
                den,
                founder,
                stores,
                heading_rad,
            } => self.found_pack(world, den, founder, stores, heading_rad).is_some(),
            Command::IntroduceAloft {
                pose,
                founder,
                stores,
            } => {
                let phys = *self.config.founder(founder);
                if phys.glider.is_some() {
                    return self.introduce_glider_aloft(world, pose, founder, stores);
                }
                let Some(flight) = phys.flight else {
                    return false;
                };
                let Some((body, reserve)) = stores.resolve(&phys.core) else {
                    return false;
                };
                let view = world.view();
                let Some((site, perch)) =
                    flight::placement(&view, &phys, &flight, body, &pose)
                else {
                    return false;
                };
                if !self.introduce_body(
                    world,
                    site,
                    Species::Frondgrazer,
                    Some(founder),
                    body,
                    Some(reserve),
                    pose.heading_rad,
                ) {
                    return false;
                }
                let id = self.ledger.births - 1;
                let i = self
                    .animals
                    .binary_search_by_key(&id, |a| a.id)
                    .expect("just placed");
                let a = &mut self.animals[i];
                a.pose = pose;
                match perch {
                    Some(p) => {
                        a.mobility.mode = MobilityMode::Perched;
                        a.mobility.perch = Some(p);
                    }
                    None => {
                        a.mobility.mode = MobilityMode::Fly;
                        a.mobility.perch = None;
                    }
                }
                true
            }
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
                wet_ticks: 0,
                dry_ticks: 0,
                face: None,
            });
            self.ledger.eggs_buried += 1;
            self.ledger.introduced_organic_in += organic;
            self.ledger.introduced_mineral_in += mineral;
            self.ledger.introduced_energy_in += energy;
        }
        true
    }

    /// **A founder's stomach at arrival** (the founding fix, 2026-10-08): body `id`'s
    /// stomach becomes `share · K` of its lineage's first food (`share` clamped to
    /// `[0, 1]`), at its own tissue's mineral and energy densities, the change booked
    /// introduced. A body introduced through [`Command::IntroduceFounder`] arrives at
    /// `s0 · K` (repair 2 item 3), on its appetite floor; the seeder calls this with
    /// [`FoundingConfig::arrival_stomach`] so a founded world's founders arrive fed. The
    /// eating latch is left as it is. False if `id` is no living founder.
    pub fn set_arrival_stomach(&mut self, id: u64, share: f64) -> bool {
        let Some(i) = self.animals.iter().position(|a| a.id == id) else {
            return false;
        };
        let Some(f) = self.animals[i].founder else {
            return false;
        };
        let p = *self.config.founder(f);
        let a = &mut self.animals[i];
        let before = a.stomach.total();
        a.stomach = Stomach::default();
        if let Some(food) = p.diet.foods().next() {
            let organic = share.clamp(0.0, 1.0) * p.gut.capacity(a.body);
            if organic > 0.0 {
                a.stomach.pools[food.index()] = Taken {
                    organic,
                    mineral: p.core.n_tissue * organic,
                    energy: p.core.energy_density * organic,
                };
            }
        }
        let after = a.stomach.total();
        self.ledger.introduced_organic_in += after.organic - before.organic;
        self.ledger.introduced_mineral_in += after.mineral - before.mineral;
        self.ledger.introduced_energy_in += after.energy - before.energy;
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
        self.world_geometry = Some((f64::from(view.config.width) * view.config.voxel_m, view.config.voxel_m));
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
            last_bloom: None,
            gut_seeds: GutSeedLots::default(),
            pack: None,
            homing_ticks: 0,
            rest_ticks: 0,
            yielding: false,
            hunkered: false,
            stomach: Stomach::default(),
            eating: false,
            emergency: false,
            bout: None,
            hunt: crate::HuntMemory::default(),
        };
        let mut animal = animal;
        // A founder arrives with its eating latch off and a stomach at `s0 · K` of its own
        // first food (repair 2 item 3), at its own tissue's mineral and energy densities
        // (so no yield up to 1 is mineral-limited), booked introduced with the rest of it.
        if let Some(f) = founder {
            let p = self.config.founder(f);
            if let Some(food) = p.diet.foods().next() {
                let organic = p.gut.s0 * p.gut.capacity(body);
                if organic > 0.0 {
                    animal.stomach.pools[food.index()] = Taken {
                        organic,
                        mineral: sc.n_tissue * organic,
                        energy: sc.energy_density * organic,
                    };
                }
            }
        }
        // A flyer on a face is perched on it (F1).
        if founder.is_some_and(|f| self.config.founder(f).flight.is_some()) {
            flight::perch_on_face(&mut animal, site);
        }
        self.ledger.births += 1;
        self.ledger.introduced += 1;
        let fed = animal.stomach.total();
        self.ledger.introduced_organic_in += organic + fed.organic;
        self.ledger.introduced_mineral_in += animal.mineral + fed.mineral;
        self.ledger.introduced_energy_in += animal.energy + fed.energy;
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
    /// face, and it is not a death, so it is not carrion either. **Removal is export**
    /// (the minimal gut's repair 3 item 7): its stomach leaves with it, booked
    /// `removed_*` (and `stomach_out`); only a death deposits a stomach.
    fn book_removed(&mut self, a: &Animal) {
        let fed = a.stomach.total();
        self.ledger.stomach_out.organic += fed.organic;
        self.ledger.stomach_out.mineral += fed.mineral;
        self.ledger.stomach_out.energy += fed.energy;
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
                // The chorister (roster R3) keeps the lanternjaw's reserve cap; the bellwing
                // (BELLWING) carries an egg load.
                matches!(founder, Founder::Lanternjaw | Founder::Chorister | Founder::Bellwing) ||
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
