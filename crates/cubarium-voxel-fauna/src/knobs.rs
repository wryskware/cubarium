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
            diffuse_fraction: 0.4,
            threshold: 1e-5,
            pool_bulk_density: 80.0,
        }
    }
}

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
        }
    }
}

/// How a fresh world is founded: the stores a founder arrives with, the litter laid under
/// the shredders and the acceptance bar (`cubarium_voxel_sim::habitat`'s seeder). The
/// founder **counts** (`habitat::FOUNDER_COUNTS`) stay `const`s there: the CLI and the
/// examples of other crates pass them in by name.
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
}

fn default_clearance_margin_m() -> f64 {
    FoundingConfig::default().clearance_margin_m
}

impl Default for FoundingConfig {
    fn default() -> FoundingConfig {
        FoundingConfig {
            arrival_body: 1.0,
            arrival_reserve: 0.5,
            starter_litter_organic: 0.2,
            starter_litter_mineral_fraction: 0.02,
            starter_litter_energy_density: 2.0,
            accept_founder_hours: 1.0,
            clearance_margin_m: 0.03,
        }
    }
}
