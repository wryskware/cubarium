//! The opt-in paid hunter extension: a fixed experimental apex lineage.
//!
//! `design/7_Research/astra-fixed-hunter-implementation-plan-2026-09-13.md` is the working
//! recipe; `fixed-hunter-core-handoff-2026-09-13.md` is the work order. **Every number in
//! [`FixedHunterProfile::lanternjaw_trial`] is a trial parameter, not validated balance and
//! not canon.** Nothing here is on by default: a world whose [`HunterState::profile`] is
//! `None` and whose member list is empty executes the pre-hunter tick, operation for
//! operation, with no extra draws (`crate::world::World::step` branches out immediately).
//!
//! What membership means: the organisms named in [`HunterState::members`] are the predators.
//! `genome.form` only picks a rig, so a saved unrelated form-4 organism can never become one,
//! and [`FixedHunterProfile::role`] carries the art direction as an explicit semantic role
//! the renderer maps to a body.
//!
//! The energy model is the world's, unchanged: adult structure carries **no** chemical
//! energy, reserve carries `e_r · R`, the usable battery is `E`, and an escrow carries
//! `e_r · (S_c + R_c) + E_c` until birth turns its structural part into heat. Calling prey
//! "meat" does not create a new energy density: a carried carcass keeps exactly the energy
//! that was removed from the prey, in [`HunterMember::gut_energy`], and that gut is part of
//! the world's stored totals, its invariants and both energy audits.

use serde::{Deserialize, Serialize};

use cubarium_surface::{ChartImage, FACE_EXTENT, Face, MAX_LOCAL_RADIUS, SurfacePoint, Vec2, unfold_with};

use crate::config::WorldConfig;
use crate::genome::Genome;
use crate::ids::{OrganismId, Slots};
use crate::organism::{DeathCause, Organism};

/// Wire version of [`FixedHunterProfile`]. A saved profile with any other version is
/// rejected rather than reinterpreted.
pub const PROFILE_VERSION: u32 = 1;

/// Rounding slack for the persisted gut bound and the adult-structure gate.
pub const TOLERANCE: f64 = 1e-9;

/// The gut residue below which the remainder is flushed as heat rather than carried: the
/// last few ulps of a finished meal, never a hidden discard of real food.
pub const GUT_RESIDUE: f64 = 1e-12;

/// `Stream::Hunt` key of the one founder placement draw (its heading). Every other draw in
/// the stream is keyed by a hunter's packed full ID, which can never be this value because a
/// live generation is never `u32::MAX` in the same slot as `u32::MAX`.
pub const FOUNDER_DRAW_KEY: u64 = u64::MAX;

/// A hunter's full ID packed into a draw key: `(slot << 32) | generation`.
pub fn draw_key(id: OrganismId) -> u64 {
    (u64::from(id.slot) << 32) | u64::from(id.generation)
}

/// Draw counters of one paid attempt: the contested-claim priority, then the capture roll.
/// Two draws per paid attempt and none for anything else (`crate::rng::Stream::Hunt`).
pub fn priority_counter(attack_counter: u64) -> u64 {
    attack_counter.saturating_mul(2)
}

pub fn capture_counter(attack_counter: u64) -> u64 {
    attack_counter.saturating_mul(2) + 1
}

/// The semantic role the art direction selected, independent of `genome.form`. The renderer
/// maps the role to a body; the world never reads it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HunterRole {
    /// Fable's Lanternjaw, the body Wrysk selected.
    Lanternjaw,
}

impl HunterRole {
    pub fn as_str(self) -> &'static str {
        match self {
            HunterRole::Lanternjaw => "lanternjaw",
        }
    }
}

/// Where a hunter founder or its budget-matched control deposit goes: a canonical surface
/// point, `face` in `0..5` and `u`/`v` in `[0, 64)`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HunterTarget {
    pub face: u8,
    pub u: f64,
    pub v: f64,
}

impl HunterTarget {
    /// The surface point named, or `None` when the face or the chart coordinates are out of
    /// range. Never panics on hostile input.
    pub fn resolve(&self) -> Option<SurfacePoint> {
        let face = Face::from_index(self.face)?;
        if !self.u.is_finite() || !self.v.is_finite() {
            return None;
        }
        if self.u < 0.0 || self.u >= FACE_EXTENT || self.v < 0.0 || self.v >= FACE_EXTENT {
            return None;
        }
        Some(SurfacePoint::new(face, self.u, self.v).canonicalize())
    }
}

/// Every persisted trial parameter of the fixed lineage, including the genome its founder and
/// every descendant carry. Saved with the world so a resumed experiment is the experiment
/// that was started.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FixedHunterProfile {
    pub version: u32,
    pub role: HunterRole,
    /// The fixed genome. Copied exactly to every descendant: hunter mutation is outside this
    /// experiment even when ordinary prey mutation is on.
    pub genome: Genome,
    /// With attacks off the same living hunter is still maintained, moves, may scavenge and
    /// may reproduce: the control that isolates predation from mere presence.
    pub attacks_enabled: bool,

    /// Collision and crowding extent of the assembled body, in pixels. Overrides the decoded
    /// lobe extent for members only; every other creature keeps its own.
    pub body_extent_px: f64,
    /// Jaw anchor: pixels forward of the body center along the heading, transported over the
    /// surface like any other offset.
    pub jaw_offset_px: f64,
    /// Contact radius at that anchor, in pixels.
    pub jaw_reach_px: f64,

    /// Founder inventory as fractions of the decoded maxima: `S = S_adult`,
    /// `R = fraction · R_max`, `E = fraction · E_max`. Derived from the world's own config,
    /// never a hardcoded inventory.
    pub founder_reserve_fraction: f64,
    pub founder_energy_fraction: f64,

    /// Perch (rest) while the reserve is above this fraction of `R_max`; start hunting below
    /// [`FixedHunterProfile::seek_reserve_fraction`]. The gap is the hysteresis.
    pub perch_reserve_fraction: f64,
    pub seek_reserve_fraction: f64,
    /// Eligible prey structure: at least `prey_structure_min`, at most
    /// `prey_structure_fraction_max · hunter.S`. Juveniles count with their actual `S`.
    pub prey_structure_min: f64,
    pub prey_structure_fraction_max: f64,
    /// Abandon a stalk after this long without reaching contact.
    pub stalk_timeout_seconds: f64,
    /// The windup gesture grants no capture; it only costs time and ordinary movement.
    pub windup_seconds: f64,
    pub strike_seconds: f64,
    /// Burst speed ceiling during a strike (px/s), still capped by affordable movement energy.
    pub strike_speed_px_s: f64,
    /// Paid in full at strike entry, before the outcome is known. A hunter that cannot afford
    /// it does not attempt.
    pub strike_energy_cost: f64,
    pub recovery_seconds: f64,
    /// `clamp(capture_base · S_h / (S_h + S_p), capture_min, capture_max)`: a bounded first
    /// size hypothesis, not an evolved defense model.
    pub capture_base: f64,
    pub capture_min: f64,
    pub capture_max: f64,

    /// A threatened prey may briefly move up to this multiple of its own maximum speed, and
    /// may turn at this rate, both limited by the movement energy it actually has.
    pub escape_speed_multiple: f64,
    pub escape_turn_rate_deg: f64,

    /// One bounded carried carcass pool per hunter. A prey whose whole inventory does not fit
    /// the remaining capacity is not attempted.
    pub gut_capacity_material: f64,
    /// Paid while a meal is carried, on top of maintenance and sensing. If it cannot be paid
    /// in full, no digestion happens that tick.
    pub handling_cost_per_second: f64,
    pub digest_rate: f64,
    /// Pause after a meal finishes.
    pub meal_recovery_seconds: f64,
    /// Facultative scavenging: this fraction of the decoded scavenge rate, and only with no
    /// target and an empty gut. Zero makes a specialist. Producer and fruit intake are always
    /// off for a member.
    pub scavenge_fraction: f64,

    /// One paid offspring, gated only on local parent state: full adult structure, this age,
    /// these reserve/energy fractions, no gut and no hunt in progress, and this interval since
    /// the previous birth. Never gated on a world population count.
    pub reproduce_min_age_seconds: f64,
    pub reproduce_reserve_fraction: f64,
    pub reproduce_energy_fraction: f64,
    pub reproduce_interval_seconds: f64,
    pub gestation_seconds: f64,
    /// Structural growth ceiling for a juvenile member (m/s), inside the existing reserve and
    /// build-cost constraints.
    pub juvenile_growth_rate: f64,
}

impl FixedHunterProfile {
    /// The candidate Lanternjaw trial profile of the implementation plan. **Trial parameters
    /// only**: the six-pixel jaw offset and 1.5 px reach are placeholders the art worker must
    /// confirm against the visible jaw, and no number here is validated balance.
    pub fn lanternjaw_trial(config: &WorldConfig) -> FixedHunterProfile {
        let mut genome = Genome::founder(0.08, &config.drives);
        genome.size = 2.0;
        genome.reserve = 2.0;
        genome.mouth = 1.0;
        genome.speed = 1.0;
        genome.sense = 8.0;
        genome.metabolism = 0.5;
        genome.depth = 0.85;
        genome.swim = 0.1;
        genome.diet = 0.0;
        genome.form = 4;
        genome.clamp();
        FixedHunterProfile {
            version: PROFILE_VERSION,
            role: HunterRole::Lanternjaw,
            genome,
            attacks_enabled: true,
            body_extent_px: 9.0,
            jaw_offset_px: 6.0,
            jaw_reach_px: 1.5,
            founder_reserve_fraction: 0.5,
            founder_energy_fraction: 0.75,
            perch_reserve_fraction: 0.65,
            seek_reserve_fraction: 0.35,
            prey_structure_min: 0.15,
            prey_structure_fraction_max: 0.75,
            stalk_timeout_seconds: 8.0,
            windup_seconds: 0.6,
            strike_seconds: 1.0,
            strike_speed_px_s: 1.0,
            strike_energy_cost: 0.08,
            recovery_seconds: 5.0,
            capture_base: 0.65,
            capture_min: 0.1,
            capture_max: 0.75,
            escape_speed_multiple: 2.0,
            escape_turn_rate_deg: 240.0,
            gut_capacity_material: 4.0,
            handling_cost_per_second: 0.002,
            digest_rate: 0.1,
            meal_recovery_seconds: 20.0,
            scavenge_fraction: 0.0,
            reproduce_min_age_seconds: 1200.0,
            reproduce_reserve_fraction: 0.8,
            reproduce_energy_fraction: 0.75,
            reproduce_interval_seconds: 1800.0,
            gestation_seconds: 120.0,
            juvenile_growth_rate: 0.002,
        }
    }

    /// The facultative variant: the same hunter, allowed to scavenge detritus at a quarter of
    /// its decoded rate when it has no target and an empty gut.
    pub fn facultative(mut self) -> FixedHunterProfile {
        self.scavenge_fraction = 0.25;
        self
    }

    /// The attack-disabled control: the same living hunter, maintained and fed the same way,
    /// that never attempts a capture.
    pub fn without_attacks(mut self) -> FixedHunterProfile {
        self.attacks_enabled = false;
        self
    }

    /// Internal consistency: version, finite numbers, ranges, ordered thresholds, and a
    /// genome inside the bounds `crate::genome` documents. Config-dependent limits are
    /// checked by the initializer, which can see the world.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != PROFILE_VERSION {
            return Err(format!("hunter profile version {} is not {PROFILE_VERSION}", self.version));
        }
        for (name, v) in self.positive_numbers() {
            if !v.is_finite() || v <= 0.0 {
                return Err(format!("hunter profile {name} = {v}, expected a positive number"));
            }
        }
        for (name, v) in self.nonnegative_numbers() {
            if !v.is_finite() || v < 0.0 {
                return Err(format!("hunter profile {name} = {v}, expected a nonnegative number"));
            }
        }
        for (name, v) in [
            ("founder_reserve_fraction", self.founder_reserve_fraction),
            ("founder_energy_fraction", self.founder_energy_fraction),
            ("perch_reserve_fraction", self.perch_reserve_fraction),
            ("seek_reserve_fraction", self.seek_reserve_fraction),
            ("prey_structure_fraction_max", self.prey_structure_fraction_max),
            ("capture_base", self.capture_base),
            ("capture_min", self.capture_min),
            ("capture_max", self.capture_max),
            ("scavenge_fraction", self.scavenge_fraction),
            ("reproduce_reserve_fraction", self.reproduce_reserve_fraction),
            ("reproduce_energy_fraction", self.reproduce_energy_fraction),
        ] {
            if !(0.0..=1.0).contains(&v) {
                return Err(format!("hunter profile {name} = {v}, expected [0, 1]"));
            }
        }
        if self.seek_reserve_fraction >= self.perch_reserve_fraction {
            return Err(format!(
                "hunter profile seek_reserve_fraction {} is not below perch_reserve_fraction {}",
                self.seek_reserve_fraction, self.perch_reserve_fraction
            ));
        }
        if self.capture_min > self.capture_max {
            return Err(format!(
                "hunter profile capture_min {} exceeds capture_max {}",
                self.capture_min, self.capture_max
            ));
        }
        if self.escape_speed_multiple < 1.0 {
            return Err(format!(
                "hunter profile escape_speed_multiple {} would slow threatened prey down",
                self.escape_speed_multiple
            ));
        }
        if self.jaw_offset_px + self.jaw_reach_px + self.body_extent_px >= MAX_LOCAL_RADIUS {
            return Err(format!(
                "hunter profile jaw reach {} + {} and extent {} do not fit the local unfolding limit {MAX_LOCAL_RADIUS}",
                self.jaw_offset_px, self.jaw_reach_px, self.body_extent_px
            ));
        }
        crate::world::check_genome(&self.genome, "hunter profile")?;
        Ok(())
    }

    fn positive_numbers(&self) -> [(&'static str, f64); 12] {
        [
            ("body_extent_px", self.body_extent_px),
            ("jaw_reach_px", self.jaw_reach_px),
            ("stalk_timeout_seconds", self.stalk_timeout_seconds),
            ("windup_seconds", self.windup_seconds),
            ("strike_seconds", self.strike_seconds),
            ("strike_speed_px_s", self.strike_speed_px_s),
            ("recovery_seconds", self.recovery_seconds),
            ("gut_capacity_material", self.gut_capacity_material),
            ("digest_rate", self.digest_rate),
            ("reproduce_interval_seconds", self.reproduce_interval_seconds),
            ("gestation_seconds", self.gestation_seconds),
            ("juvenile_growth_rate", self.juvenile_growth_rate),
        ]
    }

    fn nonnegative_numbers(&self) -> [(&'static str, f64); 7] {
        [
            ("jaw_offset_px", self.jaw_offset_px),
            ("strike_energy_cost", self.strike_energy_cost),
            ("prey_structure_min", self.prey_structure_min),
            ("escape_turn_rate_deg", self.escape_turn_rate_deg),
            ("handling_cost_per_second", self.handling_cost_per_second),
            ("meal_recovery_seconds", self.meal_recovery_seconds),
            ("reproduce_min_age_seconds", self.reproduce_min_age_seconds),
        ]
    }
}

/// Where one hunter is in its local hunt. Phases are per member and never global.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HunterPhase {
    /// Resting in place: fed, recovering from nothing, or with attacks disabled.
    Perched,
    /// Moving toward one locally sensed eligible prey.
    Stalking,
    /// The folded-to-open gesture. Grants no capture.
    Windup,
    /// The paid burst. The attempt resolves after both creatures have moved.
    Strike,
    /// Paid for an attempt and failed, or just finished a meal.
    Recovering,
    /// Carrying and digesting a carcass. No attacks and no intake from the fields.
    Handling,
}

impl HunterPhase {
    /// Phases that end at a stored tick. The others end on a condition (satiety, contact, an
    /// empty gut) and store `phase_ends_tick == phase_started_tick`.
    pub fn is_timed(self) -> bool {
        matches!(self, HunterPhase::Windup | HunterPhase::Strike | HunterPhase::Recovering)
    }

    /// Phases in which the hunter is pursuing a specific prey.
    pub fn hunting(self) -> bool {
        matches!(self, HunterPhase::Stalking | HunterPhase::Windup | HunterPhase::Strike)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            HunterPhase::Perched => "perched",
            HunterPhase::Stalking => "stalking",
            HunterPhase::Windup => "windup",
            HunterPhase::Strike => "strike",
            HunterPhase::Recovering => "recovering",
            HunterPhase::Handling => "handling",
        }
    }
}

/// One member of the lineage. The organism itself is an ordinary [`Organism`] in the arena;
/// this record is everything the hunt adds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HunterMember {
    /// Slot **and** generation: a stale handle never resolves to a reused slot.
    pub id: OrganismId,
    pub phase: HunterPhase,
    pub phase_started_tick: u64,
    /// End tick of a timed phase; equal to `phase_started_tick` for the others.
    pub phase_ends_tick: u64,
    /// The prey being pursued, by full ID. Cleared deterministically when it goes stale.
    pub target: Option<OrganismId>,
    /// Paid attempts this member has made: the counter of its `Stream::Hunt` draws.
    pub attack_counter: u64,
    /// Earliest tick a new gestation may start.
    pub next_reproduction_tick: u64,
    /// The carried carcass: material and the energy actually removed with it.
    pub gut_material: f64,
    pub gut_energy: f64,
}

impl HunterMember {
    /// A newly placed member: perched, no target, empty gut.
    pub fn new(id: OrganismId, tick: u64) -> HunterMember {
        HunterMember {
            id,
            phase: HunterPhase::Perched,
            phase_started_tick: tick,
            phase_ends_tick: tick,
            target: None,
            attack_counter: 0,
            next_reproduction_tick: 0,
            gut_material: 0.0,
            gut_energy: 0.0,
        }
    }

    /// Enter `phase` at `tick`, ending at `ends` (pass `tick` for an untimed phase).
    pub fn enter(&mut self, phase: HunterPhase, tick: u64, ends: u64) {
        self.phase = phase;
        self.phase_started_tick = tick;
        self.phase_ends_tick = ends.max(tick);
        if !phase.hunting() {
            self.target = None;
        }
    }

    /// Progress through a timed phase in `[0, 1]`, `None` for the others.
    pub fn progress(&self, tick: u64) -> Option<f32> {
        if !self.phase.is_timed() {
            return None;
        }
        let span = self.phase_ends_tick.saturating_sub(self.phase_started_tick);
        if span == 0 {
            return Some(1.0);
        }
        let done = tick.saturating_sub(self.phase_started_tick).min(span);
        Some((done as f64 / span as f64) as f32)
    }

    pub fn carrying(&self) -> bool {
        self.gut_material > 0.0
    }
}

/// The whole hunter extension, appended to `WorldState` at schema 10. Default is empty and
/// inert: no profile, no members, no imports, no counters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HunterState {
    /// `None` until an experiment initializes one. Present with an empty member list after
    /// every member has died, or in a budget-matched control arm.
    pub profile: Option<FixedHunterProfile>,
    /// Members sorted by full ID.
    pub members: Vec<HunterMember>,
    /// Material and energy admitted from outside for hunter founders, booked once here and
    /// never again in `external_material_in`.
    pub founder_material_in: f64,
    pub founder_energy_in: f64,
    /// The budget-matched control deposit: the same derived founder inventory placed as local
    /// `D`/`De` with no hunter.
    pub control_material_in: f64,
    pub control_energy_in: f64,
    pub control_deposited: bool,
    /// Founders placed by [`crate::World::start_hunter_trial`]; the initializer refuses a
    /// second one.
    pub founders_placed: u32,
    /// Paid attempts, successful captures, and prey consumed. Separate from the three
    /// persisted natural-death counters, which keep their shape and their wire position.
    pub attacks_total: u64,
    pub captures_total: u64,
    pub predation_deaths_total: u64,
    pub hunter_deaths_total: u64,
    pub hunter_births_total: u64,
}

impl HunterState {
    /// True when this world has hunters to run. An empty extension is skipped whole.
    pub fn active(&self) -> bool {
        self.profile.is_some() && !self.members.is_empty()
    }

    /// The profile of an active extension.
    pub fn profile(&self) -> Option<&FixedHunterProfile> {
        self.profile.as_ref()
    }

    /// Index of a member by full ID (binary search: members are sorted).
    pub fn index_of(&self, id: OrganismId) -> Option<usize> {
        self.members.binary_search_by(|m| m.id.cmp(&id)).ok()
    }

    pub fn contains(&self, id: OrganismId) -> bool {
        self.index_of(id).is_some()
    }

    pub fn member(&self, id: OrganismId) -> Option<&HunterMember> {
        self.index_of(id).map(|i| &self.members[i])
    }

    pub fn member_mut(&mut self, id: OrganismId) -> Option<&mut HunterMember> {
        self.index_of(id).map(|i| &mut self.members[i])
    }

    /// Insert a member keeping the list sorted; returns false if the ID is already a member.
    pub fn insert_member(&mut self, member: HunterMember) -> bool {
        match self.members.binary_search_by(|m| m.id.cmp(&member.id)) {
            Ok(_) => false,
            Err(at) => {
                self.members.insert(at, member);
                true
            }
        }
    }

    /// Remove a member and every other member's reference to it, returning the record.
    pub fn remove_member(&mut self, id: OrganismId) -> Option<HunterMember> {
        let at = self.index_of(id)?;
        let gone = self.members.remove(at);
        for m in &mut self.members {
            if m.target == Some(id) {
                m.target = None;
            }
        }
        Some(gone)
    }

    /// Clear every member's reference to a prey that has left the arena.
    pub fn forget_target(&mut self, id: OrganismId) {
        for m in &mut self.members {
            if m.target == Some(id) {
                m.target = None;
            }
        }
    }

    pub fn gut_material_total(&self) -> f64 {
        self.members.iter().map(|m| m.gut_material).sum()
    }

    pub fn gut_energy_total(&self) -> f64 {
        self.members.iter().map(|m| m.gut_energy).sum()
    }

    /// Material and energy this extension has admitted from outside the world.
    pub fn imported_material(&self) -> f64 {
        self.founder_material_in + self.control_material_in
    }

    pub fn imported_energy(&self) -> f64 {
        self.founder_energy_in + self.control_energy_in
    }

    /// Range checks after decode, against the arena the extension belongs to.
    ///
    /// A **stale target is allowed**: a snapshot can be written on the tick its prey died, and
    /// a generation-checked handle to a dead organism is harmless — the tick clears it. What is
    /// refused is a member that does not exist, a duplicate or out-of-order member list, a
    /// gut past its bound, energy in an empty gut, a hunter targeting itself or another
    /// hunter, phases that contradict their stored ticks, and any import or counter that is
    /// not a finite nonnegative number.
    pub fn validate(
        &self,
        tick: u64,
        organisms: &Slots<Organism>,
        config: &WorldConfig,
    ) -> Result<(), String> {
        for (name, v) in [
            ("founder_material_in", self.founder_material_in),
            ("founder_energy_in", self.founder_energy_in),
            ("control_material_in", self.control_material_in),
            ("control_energy_in", self.control_energy_in),
        ] {
            if !v.is_finite() || v < 0.0 {
                return Err(format!("hunter {name} = {v}"));
            }
        }
        let Some(profile) = &self.profile else {
            if !self.members.is_empty() {
                return Err("hunter members exist without a profile".into());
            }
            if self.founders_placed > 0 || self.control_deposited {
                return Err("hunter imports were booked without a profile".into());
            }
            return Ok(());
        };
        profile.validate()?;
        if self.control_deposited && !self.members.is_empty() {
            return Err("a budget-matched control world must not hold hunter members".into());
        }
        if self.members.len() > config.capacity.max_organisms as usize {
            return Err(format!("hunter members {} exceed the organism cap", self.members.len()));
        }
        let mut previous: Option<OrganismId> = None;
        for m in &self.members {
            if let Some(p) = previous
                && m.id <= p
            {
                return Err(format!(
                    "hunter members are not sorted and unique: {:?} follows {:?}",
                    m.id, p
                ));
            }
            previous = Some(m.id);
            let who = format!("hunter {}:{}", m.id.slot, m.id.generation);
            let Some(_) = organisms.get(m.id) else {
                return Err(format!("{who} is not a live organism"));
            };
            for (name, v) in [("gut_material", m.gut_material), ("gut_energy", m.gut_energy)] {
                if !v.is_finite() || v < 0.0 {
                    return Err(format!("{who}: {name} = {v}"));
                }
            }
            if m.gut_material > profile.gut_capacity_material + TOLERANCE {
                return Err(format!(
                    "{who}: gut_material {} exceeds the profile capacity {}",
                    m.gut_material, profile.gut_capacity_material
                ));
            }
            if m.gut_material <= 0.0 && m.gut_energy > TOLERANCE {
                return Err(format!("{who}: an empty gut carries {} energy", m.gut_energy));
            }
            if m.phase_started_tick > tick {
                return Err(format!("{who}: phase started at {} after tick {tick}", m.phase_started_tick));
            }
            if m.phase_ends_tick < m.phase_started_tick {
                return Err(format!("{who}: phase ends before it started"));
            }
            if m.phase.is_timed() && m.phase_ends_tick == m.phase_started_tick {
                return Err(format!("{who}: timed phase {} has no duration", m.phase.as_str()));
            }
            if !m.phase.is_timed() && m.phase_ends_tick != m.phase_started_tick {
                return Err(format!("{who}: untimed phase {} stores an end tick", m.phase.as_str()));
            }
            if m.phase == HunterPhase::Handling && !m.carrying() {
                return Err(format!("{who}: handling nothing"));
            }
            if m.phase == HunterPhase::Stalking && m.target.is_none() {
                return Err(format!("{who}: stalking no one"));
            }
            if !m.phase.hunting() && m.target.is_some() {
                return Err(format!("{who}: phase {} holds a target", m.phase.as_str()));
            }
            if let Some(target) = m.target {
                if target == m.id {
                    return Err(format!("{who} is hunting itself"));
                }
                // A stale handle is fine; a live one must not be another predator.
                if organisms.get(target).is_some() && self.contains(target) {
                    return Err(format!("{who} is hunting another hunter"));
                }
            }
        }
        Ok(())
    }
}

/// What [`crate::World::start_hunter_trial`] actually did: the full founder ID, the inventory
/// derived from this world's own config, the amounts booked, and the jaw geometry the art
/// worker has to confirm.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct HunterFounderReceipt {
    pub id: OrganismId,
    pub tick: u64,
    pub pos: SurfacePoint,
    pub structure: f64,
    pub reserve: f64,
    pub energy: f64,
    /// `S + R`, booked in [`HunterState::founder_material_in`].
    pub material_in: f64,
    /// `E + e_r · R`, booked in [`HunterState::founder_energy_in`].
    pub energy_in: f64,
    pub extent: f64,
    pub sense_radius: f64,
    pub jaw_offset_px: f64,
    pub jaw_reach_px: f64,
}

/// What [`crate::World::deposit_hunter_budget_control`] actually did: the same derived
/// inventory, placed as local detritus instead of a hunter.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct HunterControlReceipt {
    pub tick: u64,
    pub cell: u16,
    /// The founder-equivalent material added to `D`.
    pub material_in: f64,
    /// The founder-equivalent energy admitted; `energy_stored` of it fits the detritus cap
    /// and `energy_heat` left as heat immediately.
    pub energy_in: f64,
    pub energy_stored: f64,
    pub energy_heat: f64,
}

/// Why a paid attempt ended the way it did. `Unaffordable` is the one refusal that happens
/// *before* payment: it consumes no energy, no draw and no attack counter.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum AttemptOutcome {
    Captured,
    /// In contact, paid, and the capture roll failed.
    Missed,
    /// The jaw was not in reach of the target after both creatures moved.
    OutOfReach,
    /// The target died or its slot was reused before the attempt resolved.
    TargetLost,
    /// Another hunter's claim on the same prey won.
    TargetClaimed,
    /// The target no longer fits the eligibility window or the remaining gut.
    Ineligible,
    /// Refused before payment: the full strike cost was not available.
    Unaffordable,
}

impl AttemptOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            AttemptOutcome::Captured => "captured",
            AttemptOutcome::Missed => "missed",
            AttemptOutcome::OutOfReach => "out_of_reach",
            AttemptOutcome::TargetLost => "target_lost",
            AttemptOutcome::TargetClaimed => "target_claimed",
            AttemptOutcome::Ineligible => "ineligible",
            AttemptOutcome::Unaffordable => "unaffordable",
        }
    }
}

/// Transient hunter records, drained by the observer like [`crate::LifeEvent`]. Never
/// checkpointed, never hashed, never read back by the tick. Each consumed prey also produces
/// exactly one ordinary `LifeEvent::Death` with [`DeathCause::Predation`].
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HunterEvent {
    /// One resolved attempt, paid or refused.
    Attempt {
        tick: u64,
        hunter: OrganismId,
        target: Option<OrganismId>,
        outcome: AttemptOutcome,
        /// Strike cost actually charged (zero for `Unaffordable`).
        energy_paid: f64,
    },
    /// A capture, with the material and energy actually transferred into the gut.
    Capture { tick: u64, hunter: OrganismId, prey: OrganismId, material: f64, energy: f64 },
    /// A funded descendant was placed.
    Offspring { tick: u64, parent: OrganismId, child: OrganismId },
    /// A member died; its carried gut went to the local fields.
    Death {
        tick: u64,
        id: OrganismId,
        cause: DeathCause,
        gut_material: f64,
        gut_energy: f64,
        /// Energy the detritus cap allowed the gut to keep; the rest became heat.
        gut_energy_stored: f64,
    },
}

/// One hunter as the renderer sees it, keyed by full ID. Transient and published separately
/// from `RenderView` so the existing view structs keep their fields while the art package
/// lands (`fixed-hunter-core-handoff-2026-09-13.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HunterView {
    pub id: OrganismId,
    /// The art direction's semantic role, not `genome.form`.
    pub role: HunterRole,
    pub phase: HunterPhase,
    /// Progress through a timed phase; `None` while perched, stalking or handling.
    pub phase_progress: Option<f32>,
    pub pos: SurfacePoint,
    pub heading: Vec2,
    /// The transported jaw anchor: where contact is actually tested.
    pub mouth: SurfacePoint,
    pub mouth_reach_px: f64,
    /// The prey being pursued, when the handle still resolves.
    pub target: Option<OrganismId>,
    pub structure: f64,
    pub extent: f64,
    pub juvenile: bool,
    pub gut_material: f64,
    pub gut_energy: f64,
    /// `gut_material / gut_capacity_material`.
    pub gut_fraction: f32,
    /// Gestation progress while an escrow is held, on the hunter's own gestation time.
    pub gestation: Option<f32>,
}

// ---------------------------------------------------------------- pure rules

/// The metabolic constants digestion reads out of the world config: reserve energy density,
/// and the two assimilation efficiencies. Exactly the ones ordinary feeding uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metabolism {
    pub e_r: f64,
    pub eta_m: f64,
    pub eta_e: f64,
}

impl Metabolism {
    pub fn of(config: &WorldConfig) -> Metabolism {
        let o = &config.organism;
        Metabolism {
            e_r: o.reserve_energy_density,
            eta_m: o.assimilation_material,
            eta_e: o.assimilation_energy,
        }
    }
}

/// One creature's per-tick movement bill, in the world's own terms: the part that does not
/// depend on speed (maintenance and sensing) and the part that does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveBill {
    pub structure: f64,
    pub maintenance: f64,
    pub sense_radius: f64,
    pub move_cost: f64,
    pub sense_cost: f64,
}

impl MoveBill {
    pub fn of(o: &Organism, config: &WorldConfig) -> MoveBill {
        MoveBill {
            structure: o.structure,
            maintenance: o.phenotype.maintenance,
            sense_radius: o.phenotype.sense_radius,
            move_cost: config.organism.move_cost,
            sense_cost: config.organism.sense_cost,
        }
    }

    /// The fastest speed this creature can pay for out of `energy` in one tick, never below
    /// `ordinary_speed`: a boost is limited by the movement energy actually available, and
    /// ordinary movement keeps its existing semantics (pay what you can, then run out).
    pub fn affordable_speed(&self, energy: f64, dt: f64, ordinary_speed: f64, wanted_speed: f64) -> f64 {
        if wanted_speed <= ordinary_speed {
            return wanted_speed;
        }
        let fixed = (self.maintenance * self.structure + self.sense_cost * self.sense_radius) * dt;
        let per_speed = self.move_cost * self.structure * dt;
        if per_speed <= 0.0 {
            return wanted_speed;
        }
        let budget = (energy - fixed).max(0.0) / per_speed;
        wanted_speed.min(budget).max(ordinary_speed)
    }
}

/// The prey inventory a capture would transfer: `M = S + R (+ escrow S + R)` and
/// `Q = E + e_r · R (+ escrow E + e_r · (S + R))`. Structure carries no energy.
pub fn prey_inventory(prey: &Organism, e_r: f64) -> (f64, f64) {
    let mut material = prey.structure + prey.reserve;
    let mut energy = prey.energy + e_r * prey.reserve;
    if let Some(es) = &prey.escrow {
        material += es.structure + es.reserve;
        energy += es.energy + e_r * (es.structure + es.reserve);
    }
    (material, energy)
}

/// `clamp(capture_base · S_h / (S_h + S_p), capture_min, capture_max)`.
pub fn capture_probability(profile: &FixedHunterProfile, hunter_structure: f64, prey_structure: f64) -> f64 {
    let total = hunter_structure + prey_structure;
    if !total.is_finite() || total <= 0.0 {
        return profile.capture_min;
    }
    (profile.capture_base * hunter_structure / total).clamp(profile.capture_min, profile.capture_max)
}

/// Is this prey inside the eligibility window, and does its whole inventory fit the gut
/// headroom? Membership is checked by the caller, which owns the member list.
pub fn prey_is_eligible(
    profile: &FixedHunterProfile,
    hunter: &Organism,
    prey: &Organism,
    gut_headroom: f64,
    e_r: f64,
) -> bool {
    if prey.structure < profile.prey_structure_min {
        return false;
    }
    if prey.structure > profile.prey_structure_fraction_max * hunter.structure {
        return false;
    }
    let (material, _) = prey_inventory(prey, e_r);
    material <= gut_headroom + TOLERANCE
}

/// The surface distance from `from` to `to` through the shortest valid unfolding, or `None`
/// when the target is past `max_distance` or not reachable without leaving the surface.
///
/// This is the same local unfolding the pair pass and the controller use: it crosses seams,
/// never a face-local straight line that would miss one, and never the open rim.
pub fn surface_reach(
    images: &[Vec<ChartImage>; 5],
    from: SurfacePoint,
    to: SurfacePoint,
    max_distance: f64,
) -> Option<f64> {
    let max = max_distance.min(MAX_LOCAL_RADIUS);
    if max <= 0.0 {
        return None;
    }
    unfold_with(&images[from.face.index()], from, to, max).map(|u| u.distance)
}

/// True when a hunter's transported jaw anchor is within reach of `prey`: the distance from
/// the anchor to the prey's position is at most `jaw_reach_px` plus the prey's own extent.
///
/// `mouth` is the anchor the caller transported (`jaw_offset_px` forward along the heading),
/// so a jaw that crossed a seam is tested from where it actually ended up.
pub fn jaw_in_reach(
    profile: &FixedHunterProfile,
    mouth: SurfacePoint,
    prey: &Organism,
    images: &[Vec<ChartImage>; 5],
) -> bool {
    let reach = profile.jaw_reach_px + prey.phenotype.extent;
    // A small slack on the unfolding limit so a target exactly at the edge is still found.
    matches!(surface_reach(images, mouth, prey.pos, reach + 1.0), Some(d) if d <= reach)
}

/// The one paid-offspring gate, on **local parent state only**: no world population is
/// consulted, and the ordinary organism thresholds are overridden by the saved profile rather
/// than changed for every creature.
pub fn may_reproduce(
    profile: &FixedHunterProfile,
    parent: &Organism,
    member: &HunterMember,
    now: u64,
    dt: f64,
) -> bool {
    parent.escrow.is_none()
        && !member.carrying()
        && member.target.is_none()
        && !member.phase.hunting()
        && parent.structure >= parent.phenotype.structure_adult - TOLERANCE
        && parent.reserve >= profile.reproduce_reserve_fraction * parent.phenotype.reserve_max
        && parent.energy >= profile.reproduce_energy_fraction * parent.phenotype.energy_max
        && parent.age_ticks(now) as f64 * dt >= profile.reproduce_min_age_seconds
        && now >= member.next_reproduction_tick
}

/// One tick of digestion of homogeneous gut contents, in the world's own currencies.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DigestStep {
    /// Material removed from the gut this tick (`q`).
    pub material: f64,
    /// Energy removed with it (`carried = gut_energy · q / gut_material`).
    pub carried: f64,
    /// Material stored as reserve (`a`).
    pub to_reserve: f64,
    /// Material rejected into the cell's detritus, carrying zero energy (`q − a`).
    pub to_detritus: f64,
    /// Usable energy gained.
    pub energy_gain: f64,
    /// Everything else the meal released.
    pub heat: f64,
}

/// The energy-honest digestion step of the implementation plan.
///
/// `a = eta_m · min(q, carried / e_r)`, so prey that is mostly structure and almost no
/// reserve stores proportionally less material — the same `min(1, ρ / e_r)` factor ordinary
/// scavenging uses. `q` is then reduced consistently rather than clamping `a`, so the
/// material and energy leaving the gut always describe the same portion.
pub fn digest_step(
    profile: &FixedHunterProfile,
    dt: f64,
    gut_material: f64,
    gut_energy: f64,
    metabolism: Metabolism,
    reserve_headroom: f64,
    energy_headroom: f64,
) -> DigestStep {
    let Metabolism { e_r, eta_m, eta_e } = metabolism;
    if gut_material <= 0.0 || reserve_headroom <= 0.0 {
        return DigestStep::default();
    }
    let mut q = (profile.digest_rate * dt).min(gut_material);
    if q <= 0.0 {
        return DigestStep::default();
    }
    let density = gut_energy / gut_material;
    let eta = if e_r > 0.0 { eta_m * (density / e_r).min(1.0) } else { eta_m };
    if eta <= 0.0 {
        return DigestStep::default();
    }
    let mut to_reserve = eta * q;
    if to_reserve > reserve_headroom {
        // Reduce the portion, not the assimilation: material and energy must describe the
        // same bite.
        q *= reserve_headroom / to_reserve;
        to_reserve = eta * q;
    }
    let carried = density * q;
    // `eta <= eta_m · ρ / e_r` keeps this nonnegative analytically; only rounding-scale
    // residue is clamped.
    let spare = carried - e_r * to_reserve;
    debug_assert!(spare > -1e-9, "digestion spare energy {spare} is negative");
    let spare = spare.max(0.0);
    let energy_gain = (eta_e * spare).clamp(0.0, energy_headroom.max(0.0));
    DigestStep {
        material: q,
        carried,
        to_reserve,
        to_detritus: q - to_reserve,
        energy_gain,
        heat: spare - energy_gain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DT;

    fn profile() -> FixedHunterProfile {
        FixedHunterProfile::lanternjaw_trial(&WorldConfig::default())
    }

    /// The default world's metabolic constants: `e_r = 2`, `eta_m = 0.6`, `eta_e = 0.5`.
    fn metabolism() -> Metabolism {
        Metabolism::of(&WorldConfig::default())
    }

    /// The founder's own movement bill, spelled out rather than read off an organism.
    fn bill() -> MoveBill {
        MoveBill { structure: 1.0, maintenance: 0.005, sense_radius: 6.0, move_cost: 0.006, sense_cost: 0.0002 }
    }

    /// One way to make a valid profile invalid.
    type Break = fn(&mut FixedHunterProfile);

    #[test]
    fn the_trial_profile_is_valid_and_its_variants_stay_valid() {
        let p = profile();
        p.validate().expect("the trial profile is valid");
        p.clone().facultative().validate().expect("facultative");
        p.clone().without_attacks().validate().expect("attack-disabled");
        assert_eq!(p.role.as_str(), "lanternjaw");
        assert_eq!(p.version, PROFILE_VERSION);
    }

    #[test]
    fn an_invalid_profile_is_rejected_one_property_at_a_time() {
        let cases: [(&str, Break); 10] = [
            ("version", |p| p.version = 2),
            ("gut_capacity_material", |p| p.gut_capacity_material = 0.0),
            ("digest_rate", |p| p.digest_rate = f64::NAN),
            ("strike_energy_cost", |p| p.strike_energy_cost = -1.0),
            ("capture_min", |p| p.capture_min = 0.9),
            ("seek_reserve_fraction", |p| p.seek_reserve_fraction = 0.9),
            ("escape_speed_multiple", |p| p.escape_speed_multiple = 0.5),
            ("jaw", |p| p.jaw_offset_px = 40.0),
            ("genome", |p| p.genome.size = 9.0),
            ("scavenge_fraction", |p| p.scavenge_fraction = 2.0),
        ];
        for (what, break_it) in cases {
            let mut p = profile();
            break_it(&mut p);
            let err = match p.validate() {
                Ok(()) => panic!("an invalid {what} must be rejected"),
                Err(e) => e,
            };
            assert!(!err.is_empty(), "{what} was rejected without saying why");
        }
    }

    #[test]
    fn capture_probability_is_bounded_and_falls_with_prey_size() {
        let p = profile();
        let big = capture_probability(&p, 2.0, 0.2);
        let small = capture_probability(&p, 2.0, 1.5);
        assert!(big > small, "{big} vs {small}");
        for prey in [0.0, 0.01, 0.5, 1.5, 100.0] {
            let q = capture_probability(&p, 2.0, prey);
            assert!((p.capture_min..=p.capture_max).contains(&q), "prey {prey} gave {q}");
        }
        // The documented formula, exactly.
        assert_eq!(capture_probability(&p, 2.0, 1.0), (0.65 * 2.0 / 3.0f64).clamp(0.1, 0.75));
    }

    #[test]
    fn digestion_conserves_material_and_energy_exactly() {
        let p = profile();
        let m = metabolism();
        let e_r = m.e_r;
        for (gut_m, gut_q) in [(4.0, 7.0), (1.0, 0.1), (0.001, 0.002), (2.0, 12.0)] {
            let step = digest_step(&p, DT, gut_m, gut_q, m, 10.0, 10.0);
            assert!(step.material > 0.0, "no digestion of {gut_m}/{gut_q}");
            // Material: what leaves the gut is reserve plus rejected detritus.
            assert!((step.material - step.to_reserve - step.to_detritus).abs() < 1e-15);
            // Energy: what leaves the gut is stored in the reserve, gained, or heat.
            let booked = e_r * step.to_reserve + step.energy_gain + step.heat;
            assert!((step.carried - booked).abs() < 1e-12, "{:?}", step);
            assert!(step.heat >= 0.0 && step.to_detritus >= -1e-15, "{:?}", step);
        }
    }

    #[test]
    fn energy_poor_prey_stores_proportionally_less_material() {
        let p = profile();
        let m = metabolism();
        let eta_m = m.eta_m;
        // Density 2.0 (as rich as reserve material) versus 0.5 (structure-heavy prey).
        let rich = digest_step(&p, DT, 4.0, 8.0, m, 10.0, 10.0);
        let poor = digest_step(&p, DT, 4.0, 2.0, m, 10.0, 10.0);
        assert_eq!(rich.material, poor.material, "the same portion is taken");
        assert!((rich.to_reserve - eta_m * rich.material).abs() < 1e-15, "rich prey stores eta_m");
        assert!((poor.to_reserve - eta_m * 0.25 * poor.material).abs() < 1e-15, "poor prey stores rho/e_r of it");
        assert!(poor.to_detritus > rich.to_detritus);
    }

    #[test]
    fn reserve_headroom_reduces_the_portion_rather_than_the_assimilation() {
        let p = profile();
        let m = metabolism();
        let room = 0.0004;
        let step = digest_step(&p, DT, 4.0, 8.0, m, room, 10.0);
        assert!((step.to_reserve - room).abs() < 1e-15, "{:?}", step);
        // Still the same identity, on the reduced portion.
        assert!((step.carried - (2.0 * step.to_reserve + step.energy_gain + step.heat)).abs() < 1e-12);
        assert!(step.material < p.digest_rate * DT, "the portion shrank");
        // No headroom at all digests nothing and keeps the gut.
        assert_eq!(digest_step(&p, DT, 4.0, 8.0, m, 0.0, 10.0), DigestStep::default());
        assert_eq!(digest_step(&p, DT, 0.0, 0.0, m, 1.0, 1.0), DigestStep::default());
    }

    #[test]
    fn a_full_battery_sends_the_spare_energy_to_heat() {
        let p = profile();
        let step = digest_step(&p, DT, 4.0, 8.0, metabolism(), 10.0, 0.0);
        assert_eq!(step.energy_gain, 0.0);
        assert!(step.heat > 0.0);
        assert!((step.carried - (2.0 * step.to_reserve + step.heat)).abs() < 1e-12);
    }

    #[test]
    fn affordable_speed_never_slows_ordinary_movement_and_bounds_a_boost() {
        // A creature with plenty of energy gets the whole boost.
        let fast = bill().affordable_speed(10.0, DT, 0.3, 1.0);
        assert_eq!(fast, 1.0);
        // An empty battery still moves at its ordinary speed, never slower.
        let broke = bill().affordable_speed(0.0, DT, 0.3, 1.0);
        assert_eq!(broke, 0.3);
        // In between, the boost is exactly what the energy pays for, and no more.
        let energy = 0.0005;
        let some = bill().affordable_speed(energy, DT, 0.3, 5.0);
        let fixed = (0.005 * 1.0 + 0.0002 * 6.0) * DT;
        let budget = (energy - fixed) / (0.006 * 1.0 * DT);
        assert!(budget > 0.3 && budget < 5.0, "the budget must bind for this to prove anything: {budget}");
        assert!((some - budget).abs() < 1e-12, "{some} vs {budget}");
        // A request at or below the ordinary speed is returned untouched.
        assert_eq!(bill().affordable_speed(0.0, DT, 0.3, 0.2), 0.2);
    }

    #[test]
    fn member_bookkeeping_is_sorted_and_forgets_removed_ids() {
        let mut state = HunterState::default();
        assert!(!state.active());
        let a = OrganismId { slot: 5, generation: 1 };
        let b = OrganismId { slot: 2, generation: 3 };
        assert!(state.insert_member(HunterMember::new(a, 10)));
        assert!(state.insert_member(HunterMember::new(b, 10)));
        assert!(!state.insert_member(HunterMember::new(a, 10)), "a repeat is refused");
        assert_eq!(state.members.iter().map(|m| m.id).collect::<Vec<_>>(), vec![b, a]);
        assert!(state.contains(a) && state.contains(b));
        assert!(!state.contains(OrganismId { slot: 5, generation: 2 }), "generation is part of the ID");

        state.member_mut(b).expect("member").target = Some(a);
        let gone = state.remove_member(a).expect("removed");
        assert_eq!(gone.id, a);
        assert_eq!(state.member(b).expect("member").target, None, "a removed hunter is forgotten");
        state.member_mut(b).expect("member").target = Some(OrganismId { slot: 9, generation: 1 });
        state.forget_target(OrganismId { slot: 9, generation: 1 });
        assert_eq!(state.member(b).expect("member").target, None);
    }

    #[test]
    fn phase_bookkeeping_reports_progress_and_drops_targets() {
        let mut m = HunterMember::new(OrganismId { slot: 0, generation: 1 }, 100);
        m.target = Some(OrganismId { slot: 1, generation: 1 });
        m.enter(HunterPhase::Windup, 100, 112);
        assert_eq!(m.target, Some(OrganismId { slot: 1, generation: 1 }), "a hunting phase keeps it");
        assert_eq!(m.progress(100), Some(0.0));
        assert_eq!(m.progress(106), Some(0.5));
        assert_eq!(m.progress(999), Some(1.0));
        m.enter(HunterPhase::Recovering, 112, 212);
        assert_eq!(m.target, None, "a non-hunting phase drops the target");
        m.enter(HunterPhase::Perched, 212, 212);
        assert_eq!(m.progress(300), None);
    }

    #[test]
    fn a_target_resolves_only_inside_the_charts() {
        assert!(HunterTarget { face: 0, u: 1.0, v: 2.0 }.resolve().is_some());
        for bad in [
            HunterTarget { face: 9, u: 1.0, v: 1.0 },
            HunterTarget { face: 0, u: -1.0, v: 1.0 },
            HunterTarget { face: 0, u: 64.0, v: 1.0 },
            HunterTarget { face: 0, u: f64::NAN, v: 1.0 },
        ] {
            assert!(bad.resolve().is_none(), "{bad:?} resolved");
        }
    }
}
