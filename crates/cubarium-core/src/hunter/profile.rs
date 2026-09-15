use serde::{Deserialize, Serialize};

use cubarium_surface::{MAX_LOCAL_RADIUS, Vec2};

use crate::config::{OrganismConfig, WorldConfig};
use crate::genome::Genome;

use super::*;

/// Wire version of [`FixedHunterProfile`] the **default constructor** writes, and the version
/// every existing trial carries. A saved profile whose version is not in
/// [`SUPPORTED_PROFILE_VERSIONS`] is rejected rather than reinterpreted — including one whose
/// *shape* still matches.
///
/// - **1** (schema 10): a single forward `jaw_offset_px` placeholder.
/// - **2**: replaced it with the measured two-component capture effector, a separate ingestion
///   mouth, the visual query extent and the body-scale mapping
///   (`design/7_Research/lanternjaw-ecology-animation-contract-2026-09-13.md`). The shape
///   changed, so schema 10 payloads carrying a version 1 profile are refused
///   (`crate::snapshot::v10`).
/// - **3**: the capture effector's side coordinate is now Fable's *authored*
///   `Lanternjaw::effectors(1.0).near_claw`, `1.1`, in place of the `1.162368` the contract's
///   earlier decorated-study measurement reported ([`CAPTURE_OFFSET_BODY`]). The shape is
///   unchanged, but a frozen experimental constant moved, so the version moves with it: a
///   saved version 2 trial is refused by name rather than quietly re-measured. Schema 11 was
///   never deployed, so no live world carries one.
/// - **4**: [`PROFILE_VERSION_CHARGE80`], the paid-charging experiment. Every field, including
///   all geometry, means exactly what version 3 means; the *only* difference is the
///   [`OxidationPolicy`] a member carrying it runs under
///   (`design/7_Research/astra-hunter-paid-charging-proposal-2026-09-13.md`). It is **not**
///   the default: version 3 remains what [`FixedHunterProfile::lanternjaw_trial`] writes, so
///   every saved baseline trial is still read as itself.
pub const PROFILE_VERSION: u32 = 3;

/// The semantic version of the one fixed paid-charging policy, [`OxidationPolicy::Fixed`] at
/// [`CHARGE80_OXIDATION_THRESHOLD`].
///
/// It shares version 3's serialized shape exactly, which is why it is a *semantic* version and
/// not a schema change: an old reader decodes the bytes and then refuses them at
/// [`FixedHunterProfile::validate`], rather than resuming a version 4 experiment as though it
/// were version 3. That refusal is the whole safety argument for reusing the shape, so it is
/// verified against an actual frozen pre-change executable, not asserted.
///
/// This scheme suits **one frozen experimental constant**. An extensible matrix of tunable
/// policies would need an explicitly persisted field and an honest frozen old-shape migration,
/// not more combinations encoded in version numbers.
pub const PROFILE_VERSION_CHARGE80: u32 = 4;

/// Every profile version this build will load. Anything else is refused by name.
pub const SUPPORTED_PROFILE_VERSIONS: [u32; 2] = [PROFILE_VERSION, PROFILE_VERSION_CHARGE80];

/// The fixed oxidation activation threshold, as a fraction of `E_max`, that version 4 carries.
///
/// A trial constant, not a balanced or accepted value: the proposal picked 0.80 because it
/// leaves a 0.20-energy margin above this trial's 3.0 battery reproduction gate. It is not a
/// guarantee against future expenditure and says nothing about lineage viability.
pub const CHARGE80_OXIDATION_THRESHOLD: f64 = 0.80;

/// When an **authoritative hunter member** converts reserve material into battery charge.
///
/// This is the single policy the paid-charging family varies, and it varies nothing else: the
/// conversion block itself — burn ceiling, reserve-to-`N` return, `e_r` release, efficiency,
/// headroom cap and conversion heat — is identical under both, and so is every cost, gate,
/// target, geometry and intake. Only the *condition under which that block runs* differs.
///
/// Ordinary organisms are never subject to this. They are not members, and the world's
/// configured threshold is theirs whatever any hunter profile says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OxidationPolicy {
    /// Version 3: the world's own `organism.oxidation_threshold`, exactly as every creature in
    /// the world uses it. A member under this policy is arithmetically indistinguishable from
    /// the pre-policy build.
    Configured,
    /// Version 4: this fixed fraction of `E_max`, at **every age and phase**, descendants
    /// included. No age switch, reserve floor, hysteresis, reproduction condition or phase
    /// exception — those would be additional interventions, not this one.
    Fixed(f64),
}

impl OxidationPolicy {
    /// The activation threshold as a fraction of `E_max`, resolved against the world whose
    /// config it will be applied in.
    pub fn threshold(self, config: &OrganismConfig) -> f64 {
        match self {
            OxidationPolicy::Configured => config.oxidation_threshold,
            OxidationPolicy::Fixed(fraction) => fraction,
        }
    }

    /// A stable label for a manifest, so a recorded experiment states its policy in words as
    /// well as in a number.
    pub fn as_str(self) -> &'static str {
        match self {
            OxidationPolicy::Configured => "configured-world-threshold",
            OxidationPolicy::Fixed(_) => "fixed-member-threshold",
        }
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

    /// **Physical crowding extent** of the assembled body, in pixels: what the pair pass and
    /// the repulsion term use. Overrides the decoded lobe extent for members only; every other
    /// creature keeps its own. This is *not* the artwork's query radius and *not* the capture
    /// region — the contract keeps the three separate.
    pub body_extent_px: f64,
    /// **Visual query support** the renderer needs around the root to sample the whole
    /// assembled rig, in pixels, at scale 1. Published for the art adapter; the world never
    /// senses or collides with it.
    pub visual_query_extent_px: f64,
    /// **Capture effector**: the body-local centre of the grasp, `(forward, side)` in the same
    /// basis `stamp_rig` uses (+x along the heading, +y its clockwise side). The trial value is
    /// the measured centre of Fable's fully extended near claw, `(13.2794, 1.1624)`; the arms
    /// are never shortened to keep an unconfirmed placeholder.
    pub capture_offset_body: Vec2,
    /// Contact tolerance around that centre, in pixels at scale 1. A trial tolerance, not a
    /// balanced value.
    pub capture_reach_px: f64,
    /// **Ingestion mouth**, body-local, where a captured body is swallowed — the head, not the
    /// claws. The art's mouth runs from `(8.5, 0)` at rest to `(9.6, 0)` at full extension;
    /// this is the extended position, the one that exists at settlement.
    pub ingestion_offset_body: Vec2,
    /// Body scale mapping, owned here so core and art cannot disagree:
    /// `scale = max(body_scale_min, (S / S_adult)^body_scale_exponent)`. The trial exponent
    /// `0.5` is the area-preserving candidate of the animation contract.
    ///
    /// Both are validated finite as well as in range: a NaN passes every ordering test, so an
    /// ordering test alone is not a bound.
    pub body_scale_exponent: f64,
    pub body_scale_min: f64,

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
        // Sensing reconciled with the capture effector: the claws close about 14.8 px from the
        // root (`|capture_offset| + capture_reach`), so an eight-pixel gene could neither
        // acquire nor retain prey it could actually grasp. Twelve is the genome's own maximum,
        // and the extra sensing is paid for like anyone else's (`sense_cost · sense_radius`
        // every tick) — increased in the world, never in the presenter.
        genome.sense = 12.0;
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
            visual_query_extent_px: 16.0,
            capture_offset_body: CAPTURE_OFFSET_BODY,
            capture_reach_px: 1.5,
            ingestion_offset_body: INGESTION_OFFSET_BODY,
            body_scale_exponent: 0.5,
            body_scale_min: 0.2,
            founder_reserve_fraction: 0.5,
            founder_energy_fraction: 0.75,
            perch_reserve_fraction: 0.65,
            seek_reserve_fraction: 0.35,
            prey_structure_min: 0.15,
            prey_structure_fraction_max: 0.75,
            stalk_timeout_seconds: 8.0,
            windup_seconds: 0.6,
            strike_seconds: 1.0,
            // Scaled with the world's cruise calibration (`organism.speed_max` 0.3 → 5.0
            // px/s, i.e. 0.06 → 1.0 BL/s of the unit adult) by the same 16.667×, so the
            // strike keeps the *multiple* of cruise it always had. The apex adult cruises
            // about 4.2 px/s (`speed · speed_max · size^-0.25` at size 2), so a 16.67 px/s
            // strike is still ~4× cruise, and `strike_closing_px` is 16.67 px over the 1.0 s
            // strike — a lunge an observer can see at 64×64.
            strike_speed_px_s: 16.667,
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

    /// The paid-charging candidate: this profile, **field for field**, under semantic version
    /// [`PROFILE_VERSION_CHARGE80`].
    ///
    /// The only thing that changes is [`FixedHunterProfile::oxidation_policy`]. Geometry,
    /// genome, costs, gates, reserve targets, founder stocks and every other number are the
    /// ones this profile already carried, which is what makes the resulting experiment a
    /// single-family one.
    pub fn charge80(mut self) -> FixedHunterProfile {
        self.version = PROFILE_VERSION_CHARGE80;
        self
    }

    /// When a member carrying this profile converts reserve into battery charge.
    ///
    /// Version 3 defers to the world; version 4 is the fixed [`CHARGE80_OXIDATION_THRESHOLD`].
    /// A version outside [`SUPPORTED_PROFILE_VERSIONS`] cannot reach here: every path that
    /// installs a profile validates it first (`HunterState::validate`, which
    /// `WorldState::validate` and therefore `decode_snapshot`, `World::new` and
    /// `World::from_state` all run). The fallback is the configured threshold regardless, so an
    /// unvalidated profile could only ever behave like the world's own creatures.
    pub fn oxidation_policy(&self) -> OxidationPolicy {
        match self.version {
            PROFILE_VERSION_CHARGE80 => OxidationPolicy::Fixed(CHARGE80_OXIDATION_THRESHOLD),
            _ => OxidationPolicy::Configured,
        }
    }

    /// [`FixedHunterProfile::oxidation_policy`] resolved against a world's organism config.
    pub fn oxidation_threshold(&self, config: &OrganismConfig) -> f64 {
        self.oxidation_policy().threshold(config)
    }

    /// Internal consistency: version, finite numbers, ranges, ordered thresholds, and a
    /// genome inside the bounds `crate::genome` documents. Config-dependent limits are
    /// checked by the initializer, which can see the world.
    pub fn validate(&self) -> Result<(), String> {
        // Versions 3 and 4 share a shape *and* every field meaning; they differ only in
        // `oxidation_policy`. Any other version is refused by name rather than reinterpreted —
        // which is exactly what makes an old reader safe against a version 4 payload it can
        // decode but must not resume.
        if !SUPPORTED_PROFILE_VERSIONS.contains(&self.version) {
            return Err(format!(
                "hunter profile version {} is not one of {SUPPORTED_PROFILE_VERSIONS:?}",
                self.version
            ));
        }
        for (name, v) in self.positive_numbers() {
            if !v.is_finite() || v <= 0.0 {
                return Err(format!(
                    "hunter profile {name} = {v}, expected a positive number"
                ));
            }
        }
        for (name, v) in self.nonnegative_numbers() {
            if !v.is_finite() || v < 0.0 {
                return Err(format!(
                    "hunter profile {name} = {v}, expected a nonnegative number"
                ));
            }
        }
        for (name, v) in [
            ("founder_reserve_fraction", self.founder_reserve_fraction),
            ("founder_energy_fraction", self.founder_energy_fraction),
            ("perch_reserve_fraction", self.perch_reserve_fraction),
            ("seek_reserve_fraction", self.seek_reserve_fraction),
            (
                "prey_structure_fraction_max",
                self.prey_structure_fraction_max,
            ),
            ("capture_base", self.capture_base),
            ("capture_min", self.capture_min),
            ("capture_max", self.capture_max),
            ("scavenge_fraction", self.scavenge_fraction),
            (
                "reproduce_reserve_fraction",
                self.reproduce_reserve_fraction,
            ),
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
        // Finiteness first, and explicitly: an ordering test alone lets a NaN through, because
        // every comparison against NaN is false (`astra-hunter-geometry-review-2026-09-13.md`).
        for (name, v) in [
            ("escape_speed_multiple", self.escape_speed_multiple),
            ("body_scale_min", self.body_scale_min),
            ("body_scale_exponent", self.body_scale_exponent),
        ] {
            if !v.is_finite() {
                return Err(format!(
                    "hunter profile {name} = {v}, expected a finite number"
                ));
            }
        }
        if self.escape_speed_multiple < 1.0 {
            return Err(format!(
                "hunter profile escape_speed_multiple {} would slow threatened prey down",
                self.escape_speed_multiple
            ));
        }
        for (name, v) in [
            ("capture_offset_body.x", self.capture_offset_body.x),
            ("capture_offset_body.y", self.capture_offset_body.y),
            ("ingestion_offset_body.x", self.ingestion_offset_body.x),
            ("ingestion_offset_body.y", self.ingestion_offset_body.y),
        ] {
            if !v.is_finite() {
                return Err(format!("hunter profile {name} = {v}"));
            }
        }
        if self.body_scale_min <= 0.0 || self.body_scale_min > 1.0 {
            return Err(format!(
                "hunter profile body_scale_min {} is not in (0, 1]",
                self.body_scale_min
            ));
        }
        if !(0.0..=2.0).contains(&self.body_scale_exponent) {
            return Err(format!(
                "hunter profile body_scale_exponent {} is not in [0, 2]",
                self.body_scale_exponent
            ));
        }
        if self.visual_query_extent_px < self.body_extent_px {
            return Err(format!(
                "hunter profile visual_query_extent_px {} is smaller than its physical extent {}",
                self.visual_query_extent_px, self.body_extent_px
            ));
        }
        // Everything the local unfolding has to reach at scale 1: the grasp centre, its
        // tolerance, and the artwork the renderer queries around the same root.
        let reach = self.capture_offset_body.length() + self.capture_reach_px;
        if reach.max(self.visual_query_extent_px) >= MAX_LOCAL_RADIUS {
            return Err(format!(
                "hunter profile capture reach {reach} / query extent {} do not fit the local unfolding limit {MAX_LOCAL_RADIUS}",
                self.visual_query_extent_px
            ));
        }
        crate::world::check_genome(&self.genome, "hunter profile")?;
        Ok(())
    }

    fn positive_numbers(&self) -> [(&'static str, f64); 13] {
        [
            ("body_extent_px", self.body_extent_px),
            ("capture_reach_px", self.capture_reach_px),
            ("visual_query_extent_px", self.visual_query_extent_px),
            ("stalk_timeout_seconds", self.stalk_timeout_seconds),
            ("windup_seconds", self.windup_seconds),
            ("strike_seconds", self.strike_seconds),
            ("strike_speed_px_s", self.strike_speed_px_s),
            ("recovery_seconds", self.recovery_seconds),
            ("gut_capacity_material", self.gut_capacity_material),
            ("digest_rate", self.digest_rate),
            (
                "reproduce_interval_seconds",
                self.reproduce_interval_seconds,
            ),
            ("gestation_seconds", self.gestation_seconds),
            ("juvenile_growth_rate", self.juvenile_growth_rate),
        ]
    }

    fn nonnegative_numbers(&self) -> [(&'static str, f64); 6] {
        [
            ("strike_energy_cost", self.strike_energy_cost),
            ("prey_structure_min", self.prey_structure_min),
            ("escape_turn_rate_deg", self.escape_turn_rate_deg),
            ("handling_cost_per_second", self.handling_cost_per_second),
            ("meal_recovery_seconds", self.meal_recovery_seconds),
            ("reproduce_min_age_seconds", self.reproduce_min_age_seconds),
        ]
    }
}
