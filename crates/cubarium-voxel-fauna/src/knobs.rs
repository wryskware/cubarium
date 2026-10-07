//! **The animal layer's knobs that no single lineage owns** (package R2-fauna,
//! `design/handoffs/registry-r2-2026-10-01.md`): the numbers that used to be `const`s and
//! inline literals in the predation, healing, sleep, cue-field, heuristic-controller and
//! founding code. Each is a named field with a unit and a registry row
//! ([`crate::registry`]); the defaults are the values the code ran on before the move.
//!
//! They hang off [`crate::FaunaConfig`] (`predation`, `life`, `cue`, `teachers`,
//! `founding`), so the snapshot carries them and a saved world is refused by a build that
//! reads them differently (schema bump).

use serde::{Deserialize, Serialize};

/// Predation, perception of threats, wounds and the dormant egg bank
/// (`design/predation-and-flight-2026-09-28.md`). Every number here was a `pub const` in
/// `predation.rs` / `lib.rs` or an inline literal in the dodge table, the tremor and the
/// egg draw.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PredationConfig {
    /// §2.2: a body is still once it has not moved faster than `still_speed_bl_per_s` for
    /// this long, seconds.
    pub still_after_s: f64,
    /// §2.2: the ground speed, in the body's own lengths per second, that counts as
    /// moving.
    pub still_speed_bl_per_s: f64,
    /// §2.2: a still threat reads as a threat inside this distance, metres.
    pub still_threat_near_m: f64,
    /// §2.2: tremor is summed over bodies within this radius, metres.
    pub tremor_radius_m: f64,
    /// §2.13: an egg hears prey that pass within this radius, metres.
    pub footfall_radius_m: f64,
    /// §2.13: an adult of the egg's lineage within this distance suppresses its hatching,
    /// metres.
    pub hatch_suppress_m: f64,
    /// The tremor a footstep must make at an egg to be heard (§2.13 pins the rule, not
    /// the number), in the tremor's own units, organic matter × m/s / m². 5e-4 is a
    /// walking littershredder at a quarter of its cruise, 2 m off.
    pub footfall_tremor_min: f64,
    /// Distances below this are read as this in the tremor's `1 / d²`, metres, so two
    /// bodies in one place do not make an infinite tremor.
    pub tremor_near_m: f64,
    /// §2.3 dodge table, the awareness column: walking.
    pub awareness_walking: f64,
    /// Awake and resting.
    pub awareness_resting: f64,
    /// Cropping (feeding) and climbing.
    pub awareness_cropping: f64,
    /// Asleep.
    pub awareness_asleep: f64,
    /// §2.3: the dodge's aspect factor when struck from the rear half (the front is 1).
    pub dodge_rear_aspect: f64,
    /// §2.3: the dodge's condition factor is the reserve fraction clamped to
    /// `[dodge_condition_floor, 1]`.
    pub dodge_condition_floor: f64,
    /// §2.13: a dormant egg's footfall threshold is drawn uniform over this many distinct
    /// prey, lowest, count.
    pub egg_footfall_min: u32,
    /// Highest of that draw, count.
    pub egg_footfall_max: u32,
    /// §2.13: a dormant egg's median viability, seconds; the draw is log-uniform over
    /// `median / spread ..= median × spread`.
    pub egg_viability_median_s: f64,
    /// The viability draw's multiplicative spread each way, a factor of at least one.
    pub egg_viability_spread: f64,
    /// §2.4: the rate a wound heals, wound units per second (0.5 per half hour).
    pub wound_heal_per_s: f64,
    /// §2.4: organic matter a unit of wound costs to heal, per unit of body.
    pub heal_cost: f64,
    /// §2.13: what a dormant egg respires, per unit of its own organic matter per second.
    pub egg_upkeep_per_s: f64,
    /// How long a hit keeps its prey awake, seconds.
    pub alert_s: f64,
    /// §2.4: a wound slows a body: its speed is multiplied by `1 - wound_slow × wound`.
    pub wound_slow: f64,
    /// §2.4: a strike's recovery is walked at this fraction of the speed.
    pub recovery_speed: f64,
    /// §2.7: how far a sighted prey heuristic looks for a threat, metres.
    pub flee_sight_m: f64,
    /// §2.7: the tremor a blind prey heuristic flees, `left + right`, in the tremor's own
    /// units.
    pub flee_tremor: f64,
    /// Roster R3 (Fable): a lanternjaw feeding at a carcass under its heuristic leaves it
    /// when a body that hunts it comes within this, metres. 3.
    pub carcass_yield_radius_m: f64,
    /// ...and walks directly away until none is within this, then resumes, metres. 5.
    pub threat_clear_m: f64,
    /// **D5, the senses as the dodge's awareness** (S2, [`crate::vigil::awareness`]): a prey
    /// of a live lineage whose record holds its striker dodges at this awareness. 1.
    #[serde(default = "awareness_sensed")]
    pub awareness_sensed: f64,
    /// ...and one whose record does not, at this (the review's unaware value, the asleep
    /// column's). 0.
    #[serde(default = "awareness_unaware")]
    pub awareness_unaware: f64,
    /// ...sensed within this long before the lunge's contact, seconds. 1.
    #[serde(default = "aware_within_s")]
    pub aware_within_s: f64,
}

fn awareness_sensed() -> f64 {
    PredationConfig::default().awareness_sensed
}

fn awareness_unaware() -> f64 {
    PredationConfig::default().awareness_unaware
}

fn aware_within_s() -> f64 {
    PredationConfig::default().aware_within_s
}

impl Default for PredationConfig {
    fn default() -> PredationConfig {
        PredationConfig {
            still_after_s: 2.0,
            still_speed_bl_per_s: 0.1,
            still_threat_near_m: 0.5,
            tremor_radius_m: 1.5,
            footfall_radius_m: 2.0,
            hatch_suppress_m: 4.0,
            footfall_tremor_min: 5.0e-4,
            tremor_near_m: 0.1,
            awareness_walking: 1.0,
            awareness_resting: 0.7,
            awareness_cropping: 0.4,
            awareness_asleep: 0.0,
            dodge_rear_aspect: 0.5,
            dodge_condition_floor: 0.5,
            egg_footfall_min: 2,
            egg_footfall_max: 10,
            egg_viability_median_s: 12.0 * 3600.0,
            egg_viability_spread: 2.0,
            wound_heal_per_s: 0.5 / 1800.0,
            heal_cost: 1.0,
            egg_upkeep_per_s: 5.0e-6,
            alert_s: 60.0,
            wound_slow: 0.5,
            recovery_speed: 0.5,
            flee_sight_m: 3.0,
            flee_tremor: 0.015,
            carcass_yield_radius_m: 3.0,
            threat_clear_m: 5.0,
            awareness_sensed: 1.0,
            awareness_unaware: 0.0,
            aware_within_s: 1.0,
        }
    }
}

impl PredationConfig {
    /// The awareness column of the dodge table for a [`crate::State`].
    pub fn awareness(&self, state: crate::State) -> f64 {
        match state {
            crate::State::Walking => self.awareness_walking,
            crate::State::Resting => self.awareness_resting,
            crate::State::Cropping | crate::State::Climbing => self.awareness_cropping,
            crate::State::Asleep => self.awareness_asleep,
        }
    }

    /// Ticks of stillness after which a body is still.
    pub fn still_after_ticks(&self) -> u64 {
        (self.still_after_s / crate::DT).round() as u64
    }
}

/// A body's day and its life: the daylight lines of the circadian rhythm, the lifespan
/// draw and the gut.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LifeConfig {
    /// The daylight (0 night, 1 full day) a diurnal animal wakes above and a nocturnal one
    /// sleeps above.
    pub day_line: f64,
    /// The lower edge of the daylight band that counts as twilight, for a crepuscular
    /// animal.
    pub twilight_lo: f64,
    /// The upper edge of the twilight band.
    pub twilight_hi: f64,
    /// How often an animal voids its gut onto its own face, seconds.
    pub gut_void_s: f64,
    /// A lifespan draw is uniform on `[lifespan_draw_min, lifespan_draw_min +
    /// lifespan_draw_span] × median`: the lowest multiple of the median.
    pub lifespan_draw_min: f64,
    /// The width of that interval, in medians.
    pub lifespan_draw_span: f64,
    /// An **arriving** body (an introduced founder) has lived part of its life: its
    /// remaining span is a uniform `[arriving_share_min, 1)` share of such a draw.
    pub arriving_share_min: f64,
    /// **Wall slide** (crag fix 4, 2026-10-02): the share of a walking request's
    /// along-wall component a wall takes away, `[0, 1]`. 0 slides the disc along the wall
    /// at the full tangential share of the request; 1 is the old motor, which stops at
    /// the wall. The refused remainder is paid for either way.
    #[serde(default = "default_wall_friction")]
    pub wall_friction: f64,
    /// **The shallows reflex's reach** (roster mechanics fix 2), metres: a walker driven by
    /// its own heuristic that stands in water deeper than it wades (dropped in, the water
    /// rose, or a starving body shrank under it) heads for the nearest face within this
    /// horizontal distance whose water it wades. A heuristic world query, not a sense.
    #[serde(default = "default_shallows_search_m")]
    pub shallows_search_m: f64,
    /// **The ecological day**, seconds (the minimal gut, 2026-10-03): the day per-day
    /// physiology is written in — `maintenance_per_s` is a lineage's daily need `f` over
    /// it, and a rider's [`crate::FaunaConfig::parasite_take_per_day`] is spread over it.
    /// 7,200, the default cycle's two hours. Fixed, so the picture clock (a `local` day, a
    /// changed cycle) never changes physiology.
    #[serde(default = "default_ecological_day_s")]
    pub ecological_day_s: f64,
}

fn default_ecological_day_s() -> f64 {
    LifeConfig::default().ecological_day_s
}

fn default_shallows_search_m() -> f64 {
    LifeConfig::default().shallows_search_m
}

fn default_wall_friction() -> f64 {
    LifeConfig::default().wall_friction
}

impl Default for LifeConfig {
    fn default() -> LifeConfig {
        LifeConfig {
            day_line: 0.3,
            twilight_lo: 0.05,
            twilight_hi: 0.95,
            gut_void_s: 30.0,
            lifespan_draw_min: 0.8,
            lifespan_draw_span: 0.4,
            arriving_share_min: 0.5,
            wall_friction: 0.0,
            shallows_search_m: 8.0,
            ecological_day_s: 7200.0,
        }
    }
}

/// The detritus cue field the blind lineage follows, and the references it and the mouth
/// read through (`design/voxel-senses.md` §1, §3). The update cadence
/// ([`crate::UPDATE_TICKS`]) is a numeric step of the field and stays a `const`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CueConfig {
    /// The detritus stock at which a source emits at full strength, organic units; also
    /// the taste reference (`× the body's mass_scale`).
    pub emit_reference: f64,
    /// Cue half-life, seconds.
    pub half_life_s: f64,
    /// **The fungal field's own half-life** (the capgnawer package, 2026-10-05), seconds:
    /// the capgnawer's `Chem(fungal)` with the same transport but a longer-lived scent, so
    /// a grove is smelled metres off rather than only from the next face (the field
    /// reaches about `sqrt(0.29 · this)` faces before it falls by `e`, with the shipped
    /// diffusion). The detritus and film fields keep [`CueConfig::half_life_s`].
    #[serde(default = "default_fungal_half_life_s")]
    pub fungal_half_life_s: f64,
    /// The convex nearest-neighbour mix of one diffusion step, a fraction.
    pub diffuse_fraction: f64,
    /// Cue values below this are dropped after an update (cue units).
    pub threshold: f64,
    /// The bulk density a ground pool's physical height is computed with, organic units
    /// per cubic metre: a pool of organic `m` on a face of area `A` stands `m / (A ·
    /// density)` tall.
    pub pool_bulk_density: f64,
}

impl Default for CueConfig {
    fn default() -> CueConfig {
        CueConfig {
            emit_reference: 0.05,
            half_life_s: 2.0,
            fungal_half_life_s: default_fungal_half_life_s(),
            diffuse_fraction: 0.4,
            threshold: 1e-5,
            pool_bulk_density: 80.0,
        }
    }
}

fn default_fungal_half_life_s() -> f64 {
    FUNGAL_HALF_LIFE_S
}

/// The code default of [`CueConfig::fungal_half_life_s`]: the shared field's 2 s, so a
/// world that does not ask for the long scent is unchanged.
pub const FUNGAL_HALF_LIFE_S: f64 = 2.0;

/// The heuristic ("teacher") controllers' reflexes: how the shredder, the grazer and the
/// lanternjaw's ambush behave when a policy is not driving them. These are what the live
/// animals run on until a policy is installed ([`crate::BlindForager::with_config`] and
/// its siblings read this).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TeacherConfig {
    /// `Self.motor_delivery` below this is a refused motor (a wall, a drop, water, the
    /// strip's edge), a fraction.
    pub motor_refused: f64,
    /// Controller samples an escape lasts once triggered, count.
    pub escape_samples: u32,
    /// Forward effort while turning away, after the first sample of an escape.
    pub escape_creep: f64,
    /// The share of a sector's rays that are clear on flat open ground (the two downward
    /// pitch rows hit the floor; the other nine of fifteen do not). Fraction of rays.
    pub flat_clear: f64,
    /// Half a ray's share of a sector (a drop is more clear than flat ground by this).
    pub half_ray: f64,
    /// A forward sector whose only hits are the floor rows, reading this far on average
    /// (`1 - d / range`), is looking over an edge.
    pub drop_proximity: f64,
    /// Samples a wander holds its heading, drawn uniform from this lowest, count.
    pub wander_hold_min: u32,
    /// Highest of that draw, count.
    pub wander_hold_max: u32,
    /// Samples a wander turn lasts, drawn uniform from this lowest, count.
    pub wander_turn_samples_min: u32,
    /// Highest of that draw, count.
    pub wander_turn_samples_max: u32,
    /// Turn effort while a wander turn runs.
    pub wander_turn: f64,
    /// The heuristics feed when the mouth's taste cue is above this.
    pub feed_gate: f64,
    /// Forward effort when the front contact is blocked.
    pub blocked_forward: f64,
    /// The blind heuristic follows a chem signal whose response is at least this.
    pub chem_response_gate: f64,
    /// A chem trend above this is rising, below its negative falling.
    pub chem_trend_gate: f64,
    /// Turn effort of the blind heuristic's turn away from a wall and a falling signal.
    pub blind_turn: f64,
    /// The cone heuristics (browser and ambush) see foliage in a sector above this
    /// fraction of its rays.
    pub browser_sector_gate: f64,
    /// Turn effort of the browser's gaze at the richest sector.
    pub browser_gaze_turn: f64,
    /// Turn effort of the browser's no-signal wander.
    pub browser_wander_turn: f64,
    /// Samples the browser's no-signal wander holds a side before flipping, count.
    pub browser_wander_flip: u32,
    /// The ambush: a body this close (`1 - d / range`) is inside or at the edge of the
    /// strike capsule.
    pub strike_proximity: f64,
    /// The ambush's forward effort while stalking a body.
    pub stalk: f64,
    /// A crown this near ahead (`foliage_proximity`) is a waiting spot.
    pub near_crown: f64,
    /// Samples it waits at a spot with nothing in sight before it leaves, count.
    pub wait_samples: u32,
    /// Samples it walks off after a fruitless wait, count.
    pub leave_samples: u32,
    /// The ambush's turn effort toward a body it sees off-centre.
    pub ambush_face_turn: f64,
    /// The ambush's turn effort leaving a spot.
    pub ambush_leave_turn: f64,
    /// The ambush's turn effort toward foliage while seeking a crown.
    pub ambush_gaze_turn: f64,
    /// The ambush's no-signal wander turn effort.
    pub ambush_wander_turn: f64,
    /// Samples the ambush's wander holds a side before flipping, count.
    pub ambush_wander_flip: u32,
    /// The height it searches at, a fraction of its band (`Self.altitude`).
    #[serde(default = "bw_altitude")]
    pub bellwing_cruise_altitude: f64,
    /// Lift effort per unit of altitude error (fraction of the band), clamped to ±1.
    #[serde(default = "bw_gain")]
    pub bellwing_altitude_gain: f64,
    /// A fan sector sees blooms above this fraction of its rays.
    #[serde(default = "bw_bloom_gate")]
    pub bellwing_bloom_gate: f64,
    /// A bloom this near (`1 - d / range`) is close: it slows and comes down to it.
    #[serde(default = "bw_bloom_near")]
    pub bellwing_bloom_near: f64,
    /// Turn effort toward the sector with the most bloom.
    #[serde(default = "bw_gaze")]
    pub bellwing_gaze_turn: f64,
    /// Forward effort while it wanders with nothing in sight.
    #[serde(default = "bw_wander_forward")]
    pub bellwing_wander_forward: f64,
    /// Cover overhead closer than this (`Air.overhead`, a fraction of the band) is low: going
    /// to rest it rises into it (an underside), otherwise it comes down onto what is
    /// below; searching it never climbs toward it; taking off under it, it goes out level.
    #[serde(default = "bw_roost_ceiling")]
    pub bellwing_roost_ceiling: f64,
    /// **Stay on food** (the detritus web, §3.1A): the blind forager's forward effort
    /// while its mouth tastes food over [`TeacherConfig::feed_gate`]. 0 stops and feeds
    /// until the face under its mouth falls below the gate (an exhausted patch), then
    /// walks on; 1 is the old forager, which crossed a pile at full walk. A refused motor's
    /// escape and the prey flee rule still come first.
    #[serde(default = "stay_forward")]
    pub forage_stay_forward: f64,
    /// Samples the blind forager turns, the way its remembered preference says, right
    /// after it leaves an exhausted patch (area-restricted search: stay near where food
    /// was), count. 0 walks straight on.
    #[serde(default = "turn_back_samples")]
    pub forage_turn_back_samples: u32,
}

fn stay_forward() -> f64 {
    0.0
}
fn turn_back_samples() -> u32 {
    2
}

fn bw_altitude() -> f64 {
    0.4
}
fn bw_gain() -> f64 {
    4.0
}
fn bw_bloom_gate() -> f64 {
    0.02
}
fn bw_bloom_near() -> f64 {
    0.6
}
fn bw_gaze() -> f64 {
    0.8
}
fn bw_wander_forward() -> f64 {
    0.7
}
fn bw_roost_ceiling() -> f64 {
    0.4
}

impl Default for TeacherConfig {
    fn default() -> TeacherConfig {
        TeacherConfig {
            motor_refused: 0.5,
            escape_samples: 3,
            escape_creep: 0.3,
            flat_clear: 9.0 / 15.0,
            half_ray: 0.5 / 15.0,
            drop_proximity: 0.55,
            wander_hold_min: 8,
            wander_hold_max: 24,
            wander_turn_samples_min: 1,
            wander_turn_samples_max: 4,
            wander_turn: 0.8,
            feed_gate: 0.02,
            blocked_forward: 0.1,
            chem_response_gate: 0.02,
            chem_trend_gate: 0.05,
            blind_turn: 0.8,
            browser_sector_gate: 0.02,
            browser_gaze_turn: 0.8,
            browser_wander_turn: 0.6,
            browser_wander_flip: 3,
            strike_proximity: 0.45,
            stalk: 0.07,
            near_crown: 0.6,
            wait_samples: 240,
            leave_samples: 20,
            ambush_face_turn: 0.4,
            ambush_leave_turn: 0.2,
            ambush_gaze_turn: 0.8,
            ambush_wander_turn: 0.4,
            ambush_wander_flip: 8,
            bellwing_cruise_altitude: bw_altitude(),
            bellwing_altitude_gain: bw_gain(),
            bellwing_bloom_gate: bw_bloom_gate(),
            bellwing_bloom_near: bw_bloom_near(),
            bellwing_gaze_turn: bw_gaze(),
            bellwing_wander_forward: bw_wander_forward(),
            bellwing_roost_ceiling: bw_roost_ceiling(),
            forage_stay_forward: stay_forward(),
            forage_turn_back_samples: turn_back_samples(),
        }
    }
}

/// How a fresh world is founded: the stores a founder arrives with, the litter laid under
/// the shredders and the acceptance bar (`cubarium_voxel_sim::habitat`'s seeder). The
/// founder **counts** are each lineage's own
/// [`crate::FounderPhysiology::founding_count`] ([`crate::FaunaConfig::founder_counts`]).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FoundingConfig {
    /// A seeded founder arrives at this fraction of its adult body (`StartingStores`).
    pub arrival_body: f64,
    /// A seeded founder arrives with this fraction of its reserve cap.
    pub arrival_reserve: f64,
    /// Leaf litter laid under each seeded littershredder, organic units.
    pub starter_litter_organic: f64,
    /// Mineral share of that litter, fraction of organic.
    pub starter_litter_mineral_fraction: f64,
    /// Retained energy density of that litter, energy per organic unit.
    pub starter_litter_energy_density: f64,
    /// The acceptance bar: a lineage's component must hold at least this many
    /// founder-hours of adult basal upkeep in reachable edible stock for every founder
    /// placed in it (decisions §8), hours.
    pub accept_founder_hours: f64,
    /// Founders go only where the disc, this much wider than the body's footprint, clears
    /// its headroom at a face and between faces, in the largest such component
    /// (`encounter::founding_faces`), metres. A slack against corridors exactly as wide
    /// as the body, which the live body passes only on its exact centre line.
    #[serde(default = "default_clearance_margin_m")]
    pub clearance_margin_m: f64,
    /// **The founding table's land area**, square metres: the dry support-face area the
    /// lineages' `founding_count`s are written for. When positive, a founding scales every
    /// lineage's count (and each plant species' founder cap) by the world's own dry
    /// support area over this, so a smaller or larger map keeps the table's densities; a
    /// lineage asked for at all keeps at least one. Zero, the default, takes the counts as
    /// they are written. A map leaves a lineage out with `founding_count = 0`, never by
    /// letting its placement fail (scenario maps, 2026-10-04).
    #[serde(default)]
    pub reference_land_m2: f64,
}

fn default_clearance_margin_m() -> f64 {
    FoundingConfig::default().clearance_margin_m
}

impl Default for FoundingConfig {
    fn default() -> FoundingConfig {
        FoundingConfig {
            arrival_body: 1.0,
            // Night survival 1a (2026-10-02): founders arrive full, so each enters its
            // first sleep at f0 = 1 (was 0.5).
            arrival_reserve: 1.0,
            starter_litter_organic: 0.2,
            starter_litter_mineral_fraction: 0.02,
            starter_litter_energy_density: 2.0,
            accept_founder_hours: 1.0,
            clearance_margin_m: 0.03,
            reference_land_m2: 0.0,
        }
    }
}

/// **The lanternjaw's patrol** (the detritus web D2,
/// `design/lanternjaw-detritus-web-proposal-2026-10-05.md` §3.2 with Astra's corrections in
/// `design/7_Research/astra-detritus-web-review-2026-10-05.md`): a mobile sit-and-wait
/// hunter of small ground prey. It walks between patches its prey's food marks, waits
/// still on each with the lure lit, leaves a fruitless one and remembers it, turns to prey
/// tremor, and attacks the prey it meets often more readily than the prey it meets rarely.
/// Every number is a plausible starting value, untuned ([`crate::hunt`]).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HuntConfig {
    /// The patrol on. Off, the lanternjaw's heuristic is the P1 crown-seeking ambush
    /// ([`crate::LanternjawAmbush`]) unchanged: the baseline arm.
    pub patrol: bool,
    /// How far around itself a patroller reads its prey's food (litter, film, caps, crowns,
    /// blooms) to choose the next patch, metres. It stands for the reach of the cue fields
    /// it follows; a world query, not a sense in its manifest. 6 m, ten seconds' walk.
    pub cue_scan_m: f64,
    /// Food cells are pooled into square patches this wide, metres: about one prey's
    /// feeding spot plus a strike's reach. 1 m.
    pub patch_m: f64,
    /// The nearest a next patch may be, metres: beyond its own tremor range, so a new patch
    /// is new ground. 1.5 m.
    pub patch_min_hop_m: f64,
    /// It has arrived on a patch within this of its centre, metres. 0.4 m.
    pub patch_arrive_m: f64,
    /// How long it waits still on a patch before leaving it as fruitless, seconds (the
    /// proposal's 20–30 s, against the ambush's minute). 25 s.
    pub patch_wait_s: f64,
    /// A walk to a patch is given up after this long, and the patch is remembered as
    /// fruitless (it could not get there), seconds. 60 s.
    pub patrol_give_up_s: f64,
    /// How long a fruitless patch is avoided, seconds: a quarter of a world day. 1,800 s.
    pub fruitless_memory_s: f64,
    /// A patch whose centre is within this of a remembered fruitless one is that one,
    /// metres. 1 m.
    pub fruitless_radius_m: f64,
    /// The share of patch choices made uniformly among the open patches instead of by its
    /// search image (exploration: a food cue marks habitat, not prey). 0.2.
    pub patrol_explore: f64,
    /// How far it sees a prey body, metres, ahead of it and not hidden (the interim cone's
    /// 2 m range). 2 m.
    pub sight_m: f64,
    /// **The tremor sense** (predation §2.2, two channels): the faintest prey tremor,
    /// `left + right` over size-eligible prey within the tremor radius, that turns it toward
    /// the louder side, in the tremor's own units. 0.003: a walking ripple-snail at the
    /// tremor radius.
    pub tremor_turn: f64,
    /// **The search image's memory**: the e-fold time of its encounter memory per prey
    /// lineage, seconds. Shorter than a prey's crash, so the image follows a collapse within
    /// the night it happens: a night on the 120-minute day is about 40 minutes, and the small
    /// prey crashed on `patch-hunters` within one (2026-10-05). 600 s, a quarter of a night.
    /// (D2 had the proposal's one world day, 7,200 s.)
    pub search_memory_s: f64,
    /// **Switching on encounter share** (the detritus web D2b). On, the attack preference
    /// for a lineage is its share of the recent eligible encounters relative to the most
    /// met lineage's, raised to [`HuntConfig::switch_exponent`] ([`crate::attack_preference`]):
    /// a lineage that turns rare among the prey it meets is passed over even when met. Off,
    /// D2's rule on absolute contact over the exploration floor (`attack_half_s`,
    /// `attack_explore`). Off by default (2026-10-05, Wrysk: a generalist hunter is
    /// opportunistic, not loyal to a species; the 2 h arms showed no measurable refuge from
    /// switching): with `attack_explore` 1 it takes whatever profitable prey it meets.
    pub share_switching: bool,
    /// **The switching exponent** `γ` (share switching): the preference is the relative
    /// encounter share to this power. Above 1 the preference falls faster than the share (a
    /// type III response in the prey's relative abundance, Murdoch's switching); 1 is
    /// proportional, no switching. 2.
    pub switch_exponent: f64,
    /// **The naive hunter's expectation** (share switching): seconds of contact credited to
    /// every lineage on its list before the shares are taken. With little met lately every
    /// lineage stays near an equal share, so a hunter in a lean spell takes whatever it
    /// meets (its exploration); once one lineage dominates its recent encounters, this fades
    /// and a rare lineage's preference goes toward 0 — no floor. 15 s.
    pub attack_prior_s: f64,
    /// (D2's rule, share switching off.) The encounter memory, in seconds of contact, at
    /// which its attack preference for a lineage is halfway between `attack_explore` and 1.
    /// 60 s.
    pub attack_half_s: f64,
    /// (D2's rule, share switching off.) The attack preference for a prey it has not met
    /// lately: the floor. 1, opportunistic: every listed prey at full preference, so the
    /// attack chance is its [`crate::profitability`] alone and the search image changes
    /// nothing. (D2 had 0.15.)
    pub attack_explore: f64,
    /// One attack decision on one prey stands this long, seconds, so a prey it declined is
    /// not re-drawn every sample. 20 s.
    pub attack_decision_s: f64,
    /// **Strike only what it chose** (D2b). On, a patroller's strike capsule takes only a
    /// prey its attack choice accepts this decision window (the same draw the patrol makes),
    /// so a prey it passes over is not struck because it stood in reach while the hunter
    /// went for another. Off (the default, opportunistic), any size-eligible prey in the
    /// capsule, as D2 did.
    pub strike_chosen_only: bool,
    /// **Eat the kill first** (HUNTER-MEAL, Astra's open questions §3). On, a hungry hunter
    /// (the lanternjaw's patrol, and a Veilhound between its pack's hunts) with a usable
    /// carcass — carrion under the plant layer's carcass record — that it can walk to within
    /// [`HuntConfig::carcass_scan_m`] goes to it and eats until its stomach is full or the
    /// carcass is gone, taking no new prey on the way; and a hunter whose mouth is over a
    /// usable carcass eats it instead of striking a prey in its capsule. Off, main's rule:
    /// a carcass is eaten only where the mouth already tastes it, and a strike wins over it.
    pub meal_first: bool,
    /// How far a hungry hunter looks for a usable carcass to eat (its own kill's or any
    /// other's), metres, from its centre to the carcass's face. 3 m: a lunge and a turn
    /// past the 2 m it sees.
    pub carcass_scan_m: f64,
    /// **Reachable destinations only** (HUNTER-MEAL). On, the patrol's next patch and a
    /// carcass to eat are chosen only among places on the hunter's walking route graph
    /// (the Veilhound's chase graph: faces its adult disc stands clear on, joined within
    /// its mutual step where the disc passes, walls for a climber) within the route steps
    /// it walks before [`HuntConfig::patrol_give_up_s`]; and it walks there along the graph
    /// where the straight line is not walkable. Off, main's rule: squares by horizontal
    /// distance, walked straight at.
    pub reachable_only: bool,
    /// How long a lineage's walking route graph stands before it is rebuilt (terrain and
    /// water change slowly), seconds. 60 s, the Veilhound's.
    pub route_refresh_s: f64,
    /// **Kills raise a place's appeal** (HUNTER-MEAL): a kill marks its place productive in
    /// the killer's memory for this long, seconds, and the patrol's choice favours it (and
    /// no longer holds it fruitless). 1,800 s, the fruitless memory's.
    pub success_memory_s: f64,
    /// A patch within [`HuntConfig::fruitless_radius_m`] of a remembered productive place
    /// scores `1 + success_boost` times its food cue. 1: twice as appealing. Exploration
    /// ([`HuntConfig::patrol_explore`]) still draws uniformly among the open patches.
    pub success_boost: f64,
    /// **Scent in the patch choice** (S2, review §4 item 3): the scent of the channels its
    /// prey lay ([`crate::perception::prey_channels`]: the lanternjaw's small-prey channel)
    /// on a square's food faces is one more cue, scaled to its best square like the food
    /// cues and weighted by this, so a boom's scent draws it. 0 reads no scent. 1.
    #[serde(default = "patch_scent_weight")]
    pub patch_scent_weight: f64,
}

fn patch_scent_weight() -> f64 {
    HuntConfig::default().patch_scent_weight
}

impl Default for HuntConfig {
    fn default() -> HuntConfig {
        HuntConfig {
            patrol: true,
            cue_scan_m: 6.0,
            patch_m: 1.0,
            patch_min_hop_m: 1.5,
            patch_arrive_m: 0.4,
            patch_wait_s: 25.0,
            patrol_give_up_s: 60.0,
            fruitless_memory_s: 1800.0,
            fruitless_radius_m: 1.0,
            patrol_explore: 0.2,
            sight_m: 2.0,
            tremor_turn: 0.003,
            search_memory_s: 600.0,
            share_switching: false,
            switch_exponent: 2.0,
            attack_prior_s: 15.0,
            attack_half_s: 60.0,
            attack_explore: 1.0,
            attack_decision_s: 20.0,
            strike_chosen_only: false,
            meal_first: true,
            carcass_scan_m: 3.0,
            reachable_only: true,
            route_refresh_s: 60.0,
            success_memory_s: 1800.0,
            success_boost: 1.0,
            patch_scent_weight: 1.0,
        }
    }
}

/// **The perception core's knobs** (SENSES-CORE, S1; `design/7_Research/senses-review-2026-10-06.md`
/// §3): how often and how widely the pairwise senses (sight, hearing, tremor) look, the one
/// terrain ray's occlusion, hearing's emission rule and the pursuit's memory. Per-lineage
/// ranges are each lineage's [`crate::SenseTable`]. Every number a plausible start, untuned.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PerceptionConfig {
    /// Remote sensing (sight, hearing and the tremor mark of a pair) runs this often per
    /// body, Hz: on the ticks where `tick ≡ id (mod TICK_HZ / remote_hz)`, staggered by id.
    /// Contact, the tremor reading and flight run at tick rate. 4 Hz.
    pub remote_hz: f64,
    /// At most this many bodies (prey and threats, nearest first) are weighed per observer
    /// per remote sensing, each with one terrain ray; the nearest of its own lineage besides.
    /// 8.
    pub max_candidates: u32,
    /// The spatial bins bodies are sorted into each tick, metres on a side. 2 m.
    pub bin_m: f64,
    /// Hearing and tremor through terrain (the ray blocked) are multiplied by this; sight
    /// is stopped. 0.25.
    pub occlusion: f64,
    /// Hearing's emission is `speed^speed_exp × length^size_exp` (speed in m/s, length in
    /// m). 1.
    pub hearing_speed_exp: f64,
    /// 1.
    pub hearing_size_exp: f64,
    /// Received loudness is emission over the distance, read as at least this, metres.
    /// 0.5.
    pub hearing_near_m: f64,
    /// **The pursuit's memory**: a body not sensed again within this long is given up, and
    /// a pursuit walks to its last-known point until then, seconds. 10 s.
    pub give_up_s: f64,
}

impl Default for PerceptionConfig {
    fn default() -> PerceptionConfig {
        PerceptionConfig {
            remote_hz: 4.0,
            max_candidates: 8,
            bin_m: 2.0,
            occlusion: 0.25,
            hearing_speed_exp: 1.0,
            hearing_size_exp: 1.0,
            hearing_near_m: 0.5,
            give_up_s: 10.0,
        }
    }
}

impl PerceptionConfig {
    /// Ticks between a body's remote sensings, at least one.
    pub fn stagger_ticks(&self) -> u64 {
        if self.remote_hz.is_finite() && self.remote_hz > 0.0 {
            (f64::from(crate::TICK_HZ) / self.remote_hz).round().max(1.0) as u64
        } else {
            1
        }
    }
}

/// **The scent field's knobs** (SENSES-CORE, [`crate::scent`]): animals lay scent on the
/// surface where they stand, by mass; carrion by its stock; it decays and spreads only
/// between connected surface cells.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScentConfig {
    /// The field's cell, metres on a side (rounded to whole voxels): surface faces in one
    /// cell's columns that connect are one node. 0.5 m.
    pub cell_m: f64,
    /// Scent's half-life, seconds. 60 s.
    pub half_life_s: f64,
    /// The share of a node's difference to its neighbours mixed in per update (every
    /// 0.5 s), at most 1. 0.5.
    pub diffuse_fraction: f64,
    /// Levels below this are dropped after an update. 1e-6.
    pub threshold: f64,
    /// A standing body lays this much per unit of its organic matter per second. 1.
    pub deposit_per_s: f64,
    /// Carrion lays this much per unit of its stock per second. 1.
    pub carrion_per_s: f64,
    /// Two neighbouring surface faces connect when their heights differ by at most this,
    /// metres, with open air over the lower face up to the higher (a ledge connects, a
    /// wall does not). 0.5 m.
    pub step_m: f64,
    /// **The nectar channel** (S3): a flower holding nectar (a bloomcrown's head, a
    /// latticevine face in flower) lays this much a second on the surface under it, whatever
    /// its stock: a flower smells while it holds nectar. 2.25 (an adult frondgrazer's
    /// organic matter at `deposit_per_s`, so the smell floors' calibration reads in metres).
    pub nectar_per_s: f64,
}

impl Default for ScentConfig {
    fn default() -> ScentConfig {
        ScentConfig {
            cell_m: 0.5,
            half_life_s: 60.0,
            diffuse_fraction: 0.5,
            threshold: 1e-6,
            deposit_per_s: 1.0,
            carrion_per_s: 1.0,
            step_m: 0.5,
            nectar_per_s: 2.25,
        }
    }
}

/// **Light and the bright things** (S3 LIGHT AND FLIGHT; `design/7_Research/senses-review-2026-10-06.md`
/// §1, §2 and §5 S3, [`crate::light`]): the beacons' brightness, the photoreceptor's falloff,
/// the lanternjaw's lure point, the inspection of a beacon and the failed-visit mark. Ranges
/// and floors are each lineage's [`crate::SenseTable`]. Every number a plausible start, untuned.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LightConfig {
    /// A nectar flower's brightness (its UV) per unit of the light falling on it (`sky ×
    /// daylight` at its head): a reflector, dark at night. 1.
    pub flower_reflectance: f64,
    /// A glowcap's own light, in the same units (it needs no light to be seen). 0.3.
    pub glowcap_emission: f64,
    /// A fully lit lure's own light. 1.
    pub lure_emission: f64,
    /// A beacon is seen by its **contrast**: its brightness over the light around it (`sky ×
    /// daylight` where it is) plus this floor, the darkest background. 0.05.
    pub background_floor: f64,
    /// The received level is the contrast × `1 / (1 + (d / half_m)²)`, metres. 1 m.
    pub half_m: f64,
    /// The lure hangs this far over the lanternjaw's standing surface, × its body height —
    /// inside its jaw band — at the middle of its strike capsule. 0.4.
    pub lure_height_fraction: f64,
    /// A body this close (horizontally, metres) to a beacon is **inspecting** it. 0.25 m.
    pub visit_m: f64,
    /// An inspection that brings no meal within this long fails, seconds. 3 s.
    pub visit_s: f64,
    /// **The left visit** (FINDINGS-FIX): an open inspection whose body has flown farther
    /// than `visit_m` + this (horizontally, metres) from its beacon is closed as failed
    /// (marked, counted, its class taught 0), so a later sip elsewhere cannot credit it.
    /// 0.5 m.
    pub visit_leave_m: f64,
    /// **The failed-visit mark**: a failed inspection marks the place "no reward here" for
    /// this long, seconds; a marked beacon is not approached until it decays. 120 s.
    pub mark_s: f64,
    /// A mark covers beacons within this of its centre, metres. 0.5 m.
    pub mark_radius_m: f64,
    /// A flyer approaching a beacon climbs or drops toward its height at this lift per metre
    /// of difference (clamped to ±1). 4 per m.
    pub lift_per_m: f64,
    /// Inside this (horizontally, metres) of the beacon, a flyer slows to `near_forward`. 1 m.
    pub near_m: f64,
    /// A flyer's forward effort near a beacon. 0.3.
    pub near_forward: f64,
    /// **The glow's pull without scent** (D2): a hungry ground body with a light sense is
    /// drawn to a still cyan light only while its food chemistry — the response its `Chem`
    /// reads, `cue / (cue + chem_saturation)`, of the capgnawer's fungal field at its receptor
    /// — is below this. 0.05.
    pub glow_scent_gate: f64,
    /// **Which light pays** (S3b, [`crate::light`] "Learning"): a flyer with a light sense
    /// keeps a reward expectation per beacon class — UV (flowers) and cyan (glowcaps, lures)
    /// — and is drawn to a still beacon by its level × its class's expectation. Off: by its
    /// level alone (S3). A switch, no registry row. On.
    pub learn: bool,
    /// The **innate** expectation (a probability of reward, `0..=1`) of a UV reflector: what
    /// a naive body expects, and what a learned one relaxes back to. 0.5: either colour may pay
    /// (pollinators come with a colour bias but are drawn to both; Giurfa et al. 1995).
    pub uv_innate: f64,
    /// The innate expectation of a cyan emitter. 0.5, like UV: a naive bellwing is drawn to
    /// any bright thing, so the still lure works on the young and the inexperienced.
    pub cyan_innate: f64,
    /// Each outcome moves its class's expectation this fraction of the way to it: 1 after a
    /// sip, 0 after a failed visit (Rescorla–Wagner). 0.3: bees and bumblebees learn a
    /// colour–reward pairing within a few visits (Menzel 1985; Dukas & Real 1991).
    pub learn_rate: f64,
    /// A learned expectation relaxes back toward innate with this time constant, seconds
    /// (the offset shrinks by e each `forget_s`). 3600 s: half the 2 h world day — a colour
    /// memory that lasts days at life scale (Menzel 1999), compressed like the day.
    pub forget_s: f64,
    /// A still beacon whose class is expected to pay less than this is not approached at all.
    /// 0.1.
    pub expect_min: f64,
    /// **Looming is an approach** (S3b): a moving body within `loom_m` looms only while it
    /// closes in fast enough to reach the eye within this, seconds — distance over closing
    /// speed, the time-to-contact an insect's looming detector reads (Gabbiani et al. 1999;
    /// Card & Dickinson 2008). 0: any moving body within `loom_m` looms (S3). 1 s.
    pub loom_tau_s: f64,
}

impl Default for LightConfig {
    fn default() -> LightConfig {
        LightConfig {
            flower_reflectance: 1.0,
            glowcap_emission: 0.3,
            lure_emission: 1.0,
            background_floor: 0.05,
            half_m: 1.0,
            lure_height_fraction: 0.4,
            visit_m: 0.25,
            visit_s: 3.0,
            visit_leave_m: 0.5,
            mark_s: 120.0,
            mark_radius_m: 0.5,
            lift_per_m: 4.0,
            near_m: 1.0,
            near_forward: 0.3,
            glow_scent_gate: 0.05,
            learn: true,
            uv_innate: 0.5,
            cyan_innate: 0.5,
            learn_rate: 0.3,
            forget_s: 3600.0,
            expect_min: 0.1,
            loom_tau_s: 1.0,
        }
    }
}
