//! **The phase-one founder body**: paid heading motion on the continuous pose, the
//! contact/taste receptors of the manifest, and the geometry a local bite is allowed to
//! touch (`design/voxel-senses-phase1-plan.md`, "Phase-one body and action contract").
//!
//! **The body is in metres** (`design/handoffs/voxel-body-anchors-2026-09-22.md`,
//! decisions §1, §2, §6). Its dimensions are its lineage's adult dimensions on the
//! [`FounderPhysiology`] times `(body / body_max)^(1/3)`, resolved into one [`Body`]
//! value that every consumer takes. It is an upright disc of radius `width / 2` on the
//! support layer, centred on the [`Pose`](crate::Pose)'s `(x, z)`, standing on the
//! support face one layer below its centre column, and it needs its own **height** of
//! void over that face. The forward mouth is the footprint plus a reach of
//! `0.25 × length` beyond it, and it takes food from the physical band
//! `[0, 1.33 × height]` over the standing surface — not from a whole-voxel layer count.
//! Contract v2 writes the same adult dimensions and anchor fractions into the
//! [`Manifest`](crate::Manifest)'s digest (D1); the geometry is still read from here.
//!
//! # What movement is allowed to do
//!
//! Forward effort sweeps the disc along the heading in bounded sub-steps of at most one
//! footprint radius, so a wall (a solid voxel at the body layer is at least one voxel
//! thick) cannot be tunnelled between endpoint checks. `x` wraps; the strip's `z` ends
//! are hard walls, and since contract v2 (D4) the contact receptors feel them as walls.
//!
//! **The body steps ledges.** A lineage has a step up and a step down in metres on its
//! [`FounderPhysiology`], converted to whole voxels once at the consumer by
//! [`FounderPhysiology::step_limits`], and a sub-step may put the centre column on a
//! support face at most `up` voxels above the body's own standing layer or `down` below
//! it (`design/handoffs/voxel-founder-step-2026-09-22.md`; the split is package
//! mobility's, `design/handoffs/voxel-mobility-2026-09-23.md`). A candidate is refused
//! when the centre column carries no such face — a wall, or a cliff edge: there is still
//! no falling — when the disc would overlap a solid in the layers the body needs, or when
//! the standing water there is deeper than the body can wade (a fraction of its current
//! height) **and** no shallower than the water it already stands in: a body caught in
//! deep water may always step toward the shallows.
//!
//! **A shredder climbs terrain walls** ([`FounderPhysiology::climbs_walls`]). Where its
//! sweep is refused by a riser taller than its step, or by an edge deeper than its step
//! down, it attaches to the face ([`wall_ascent`], [`wall_descent`]) and the step's
//! `founder_act` moves it along the face on later ticks; `step`'s module doc has the
//! rule. The standing layer, and
//! with it `site.y` and the pose's height, follow the destination. A refused step stops
//! the sweep: movement is constrained, the delivered motion is what actually happened,
//! and the requested equivalent displacement is paid for anyway. There is no graph
//! search, no waypoint, no nearest-food target and no turn-to-target anywhere in this
//! path — the controller's turn effort is the only yaw input there is.
//!
//! # Receptors
//!
//! Front/left/right contact receptors probe the footprint's boundary arc in their
//! direction (the arc centre and ±45°, at the footprint radius) for a solid voxel at the
//! layer a body *one climb higher* would occupy — the cell holding `0.5 × height` over
//! the standing surface, plus `climb`. **A solid the
//! body could step onto is not a wall**: a steppable ledge reads as open ground and a
//! cliff face or a taller wall reads exactly as it did before (the receptor is still one
//! layer, and the observation vector's shape is unchanged). The bodies are smaller than a
//! voxel, so a touching solid always covers one of the samples. A sample past the strip's
//! `z` edge reads solid at any height (D4): the edge of the world is a wall. The
//! underside receptor reads the support face under the centre.
//! `Wet` is the standing water at the foot, a valid zero when dry. `Taste` reads only
//! what the mouth region physically contacts: the browser tastes the foliage of a stand
//! whose crown cells reach the mouth region; the blind founder tastes actual litter stock
//! under its ground-level mouth, or valid-zero bare ground. Never a remote field query,
//! never a neighbour outside the reach.
//!
//! The sense modules (`Chem`, `Light`, `Cone`) are not this module's: it leaves them
//! **zero with validity 0** in the observation and fabricates nothing it does not measure.

use cubarium_voxel::{DT, VoxelView};
use cubarium_voxel_flora::{FloraView, Site};
use serde::{Deserialize, Serialize};

use crate::controller::Actions;
use crate::manifest::{Founder, Manifest};
use crate::{Animal, Fauna, Reproduction, SpeciesConfig};

/// Below this much requested equivalent displacement an interval counts as "none
/// requested" for the motor-delivery channel, which then reads 1 by contract.
const NONE_REQUESTED: f64 = 1e-12;

// The anchor fractions, shared by both founders and listed as placeholders in
// `design/backlog.md` §1 (decisions §2 and §6; the brief
// `design/handoffs/voxel-body-anchors-2026-09-22.md`). They live on the physiology, not
// on the `Manifest`, because manifest geometry is part of the trained-policy digest.

/// The eye, as a fraction of body height over the standing surface.
const EYE_HEIGHT_FRACTION: f64 = 0.8;
/// The mouth band's ceiling, as a fraction of body height over the standing surface.
const MOUTH_CEILING_FRACTION: f64 = 1.33;
/// The contact receptors, as a fraction of body height over the standing surface.
const CONTACT_HEIGHT_FRACTION: f64 = 0.5;
/// The mouth's horizontal reach, as a fraction of body length ahead of the footprint.
const MOUTH_REACH_LENGTH_FRACTION: f64 = 0.25;

// The authored assimilation yields per food class (decisions §3; `design/backlog.md`
// §1). They are the two numbers the frozen table already ran on, named so that a food
// class can be re-yielded on its own: the shredder's litter yield and the browser's
// foliage yield. They are equal today, which is why this package moves no number.
/// [`FounderPhysiology::wall_climb_cost_factor`], both lineages: see the field.
const WALL_CLIMB_COST_FACTOR: f64 = 2.0;
/// [`FounderPhysiology::drown_after_s`], both lineages.
const DROWN_AFTER_S: f64 = 60.0;
/// [`FounderPhysiology::sleep_upkeep`] of the endotherm-like lineages (the frondgrazer,
/// loftstrider, bellwing, seedporter and chorister): 0.5, provisional
/// (`design/ecology-timescales-2026-10-02.md` 1a).
const SLEEP_UPKEEP: f64 = 0.5;
/// [`FounderPhysiology::sleep_upkeep`] of the ectotherm-like lineages (the shredder,
/// ripple-snail, capgnawer and lanternjaw): 0.3, provisional (1a).
const SLEEP_UPKEEP_ECTOTHERM: f64 = 0.3;
/// [`FounderPhysiology::wake_hunger`], every lineage: a sleeper wakes for hunger once its
/// reserve is under a tenth of its cap (1a, Wrysk 2026-10-02; 0.5 before).
const WAKE_HUNGER: f64 = 0.9;

/// [`FounderPhysiology::emergency_clear_hunger`], every lineage: the emergency latch
/// clears below this hunger (repair 2 item 7).
const EMERGENCY_CLEAR_HUNGER: f64 = 0.6;
/// [`Gut::s0`], every lineage: a fed body wants food at a nearly empty stomach.
const APPETITE_FLOOR: f64 = 0.15;
/// [`Gut::full_tolerance`], every lineage: the full event (repair 2 item 3).
const FULL_TOLERANCE: f64 = 0.98;
/// [`Gut::refill_gap`], every lineage (repair 3 item 3).
const REFILL_GAP: f64 = 0.1;
/// The ecological day the daily needs and lifespans below are written in, seconds
/// ([`crate::LifeConfig::ecological_day_s`]'s default): a unit for stating defaults.
const DAY_S: f64 = 7200.0;

/// **Daily need `f`**: the share of its adult body a lineage burns per world day awake
/// and at rest (`design/metabolism-meals-2026-10-02.md` §1; `maintenance_per_s = f /
/// 7,200 s`). Plausible art/game defaults, not calibrated biology (Astra M1).
const DAILY_BELLWING: f64 = 0.15;
const DAILY_SEEDPORTER: f64 = 0.08;
const DAILY_FRONDGRAZER: f64 = 0.06;
const DAILY_LOFTSTRIDER: f64 = 0.04;
const DAILY_CHORISTER: f64 = 0.08;
// The detritus web D2 (Wrysk 2026-10-05: "a little more active"): an active small
// predator needs more than the P1 ambusher's 0.04, between the shredder band and the
// grazer (the proposal's 0.06-0.08).
const DAILY_LANTERNJAW: f64 = 0.07;
const DAILY_SHREDDER: f64 = 0.03;
const DAILY_CAPGNAWER: f64 = 0.03;
const DAILY_RIPPLE_SNAIL: f64 = 0.02;

/// **Median lifespans** (repair 2 item 5, with Wrysk's 2026-10-03 change), world days:
/// maturation at most a quarter of the median life. The small lineages (shredder,
/// ripple-snail, capgnawer, bellwing) 6; the frondgrazer and the similar-sized seedporter
/// 8; the predators 12; the loftstrider, largest and longest-lived, 16.
const LIFE_SMALL_DAYS: f64 = 6.0;
const LIFE_GRAZER_DAYS: f64 = 8.0;
const LIFE_PREDATOR_DAYS: f64 = 12.0;
const LIFE_LOFTSTRIDER_DAYS: f64 = 16.0;

/// What a shredder builds out of a unit of litter — and, by the decision, of carrion.
const LITTER_YIELD: f64 = 0.5;
/// What a grazer builds out of a unit of foliage — and, by the decision, what a
/// shredder builds out of a unit of glowcap cap tissue.
const FOLIAGE_YIELD: f64 = 0.5;
/// Nectar's assimilation yield (F1): fresh plant matter's (the foliage yield). A
/// placeholder; at bloomcrown's `nectar_mineral_fraction` (0.01) against the bellwing's
/// `n_tissue` (0.02) the mineral binds at the same 0.5. Kept within 2/3 so the search's
/// ×1.5 yield gene stays a fraction.
const NECTAR_YIELD: f64 = 0.5;

/// The yield of **fruit** (F3, the seedporter): soft, energy-rich flesh digests better
/// than leaf, so a little over the foliage yield; kept under 2/3 so cubarium-search's ×1.5
/// yield gene stays a fraction. A placeholder.
const FRUIT_YIELD: f64 = 0.6;

/// The seedporter's body volume over the frondgrazer's (F3): 0.5 × 0.25 × 0.2 m over
/// 0.75 × 0.375 × 0.375 m, r ≈ 0.237. A derivation, not a knob, as
/// [`LOFTSTRIDER_VOLUME_RATIO`] is.
pub const SEEDPORTER_VOLUME_RATIO: f64 = (0.5 * 0.25 * 0.2) / (0.75 * 0.375 * 0.375);
/// The chorister's body volume over the lanternjaw's (roster R3): 1.4 × 0.35 × 0.6 m over
/// 1.0 × 0.25 × 0.45 m, r ≈ 2.61 (the 0.8 m tail is geometry only). A derivation, not a
/// knob.
pub const CHORISTER_VOLUME_RATIO: f64 = (1.4 * 0.35 * 0.6) / (1.0 * 0.25 * 0.45);
/// The bellwing's mass against the littershredder's (F1): `body_max` 0.001 (x1, the
/// note's number) over the shredder's 0.0125.
pub const BELLWING_MASS_RATIO: f64 = 0.001 / 0.0125;

/// The capgnawer's shipped [`crate::Reproduction::egg_fungus_radius_m`], metres: eggs within
/// half a metre of a glowcap with caps (about its own two body lengths). Plausible, untuned.
const CAPGNAWER_EGG_FUNGUS_RADIUS_M: f64 = 0.5;

/// The capgnawer's body volume over the littershredder's (roster R1): 0.25 × 0.21 ×
/// 0.19 m over 0.375 × 0.125 × 0.125 m. A derivation, not a knob: the physiology fields
/// it scales are the knobs.
pub const CAPGNAWER_VOLUME_RATIO: f64 = (0.25 * 0.21 * 0.19) / (0.375 * 0.125 * 0.125);

/// The ripple-snail's body volume over the littershredder's (roster R2b): 0.19 × 0.19 ×
/// 0.063 m over 0.375 × 0.125 × 0.125 m. A derivation, not a knob, as
/// [`CAPGNAWER_VOLUME_RATIO`] is.
pub const RIPPLE_SNAIL_VOLUME_RATIO: f64 = (0.19 * 0.19 * 0.063) / (0.375 * 0.125 * 0.125);

/// The loftstrider's **hull** volume over the frondgrazer's body (roster R1): 1.5 × 0.55
/// × 0.45 m over 0.75 × 0.375 × 0.375 m. Its 2.2 × 2.2 × 3.4 m envelope is legs and
/// fronds, so it is not used here.
pub const LOFTSTRIDER_VOLUME_RATIO: f64 = (1.5 * 0.55 * 0.45) / (0.75 * 0.375 * 0.375);

/// **A living body's physical geometry**, in metres, resolved from its lineage's
/// [`FounderPhysiology`] and its current structure.
///
/// One value, handed to every consumer, so the animal is the same animal at the tick, in
/// the presenter, in the seeder and in a diagnostic, and the grid appears only where a
/// length is turned into cells (`design/voxel-encounter-contract-2026-09-21.md` §4).
/// Every field is an absolute length: the dimensions are the adult's times
/// `(body / body_max)^(1/3)` (decisions §1) and the anchors are the physiology's
/// fractions of those.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// Nose to tail.
    pub length_m: f64,
    /// Across; the footprint is a disc of half this.
    pub width_m: f64,
    /// Standing surface to the top of the back. The void a body needs is this tall.
    pub height_m: f64,
    /// The eye, metres over the standing surface.
    pub eye_m: f64,
    /// The mouth band is `[mouth_floor_m, mouth_ceiling_m]` over the standing surface.
    pub mouth_floor_m: f64,
    pub mouth_ceiling_m: f64,
    /// The contact receptors, metres over the standing surface.
    pub contact_m: f64,
    /// How far ahead of the footprint the mouth reaches, in metres.
    pub mouth_reach_m: f64,
}

impl Body {
    /// The footprint disc's radius: half the body's width.
    pub fn footprint_radius(&self) -> f64 {
        self.width_m / 2.0
    }

    /// The void this body needs over the face it stands on, in whole voxels:
    /// `ceil(height / voxel)`, never below one. The clearance is the **body**, not the
    /// mouth: `mouth_reach_up_voxels` is no longer read for geometry.
    pub fn headroom_voxels(&self, voxel_m: f64) -> u32 {
        if !(voxel_m > 0.0) || !self.height_m.is_finite() || self.height_m <= 0.0 {
            return 1;
        }
        let cells = (self.height_m / voxel_m).ceil();
        if !cells.is_finite() || cells < 1.0 {
            return 1;
        }
        (cells as u32).max(1)
    }

    /// The crown layers the mouth band reaches from `standing_y`: the cells whose slab
    /// overlaps `[surface + mouth_floor_m, surface + mouth_ceiling_m]` by a positive
    /// amount. A touch at exactly the floor or the ceiling contributes nothing.
    pub fn mouth_layers(&self, standing_y: u32, voxel_m: f64) -> std::ops::RangeInclusive<i64> {
        crate::encounter::band_layers(standing_y, voxel_m, self.mouth_floor_m, self.mouth_ceiling_m)
    }

    /// The layer the contact receptors probe: the cell holding `contact_m` over the
    /// standing surface. For either founder on either shipped grid this is the body's
    /// own layer, which is where the receptors always were — it is now *derived* from
    /// the body rather than assumed.
    pub fn contact_layer(&self, standing_y: u32, voxel_m: f64) -> u32 {
        if !(voxel_m > 0.0) || !self.contact_m.is_finite() || self.contact_m < 0.0 {
            return standing_y + 1;
        }
        let up = (self.contact_m / voxel_m).floor();
        if !up.is_finite() || up < 0.0 {
            return standing_y + 1;
        }
        standing_y + 1 + up.min(f64::from(u32::MAX / 2)) as u32
    }
}

/// A founder's own physiology: the digestive and upkeep numbers in the ordinary
/// [`SpeciesConfig`] shape, plus the two phase-one cost settings the manifest's schema
/// deliberately does not carry (cost coefficients change how good a policy can be, not
/// what a slot means, so they stay out of the digest).
///
/// The physiology table is **frozen** for the first ES pilots
/// (`design/voxel-senses-phase1-plan.md`): genetic variation and cost optimisation are
/// later experiments.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FounderPhysiology {
    /// The numbers the ordinary animal rules run on when the body carries this
    /// founder's marker: upkeep, bite rate, assimilation, thresholds and densities.
    pub core: SpeciesConfig,
    /// The named configurable motor respiration coefficient: organic matter respired
    /// per unit of body per second at **full cruise** (equivalent displacement equal to
    /// `cruise`). Below full cruise the cost scales with the requested equivalent
    /// displacement `|v| + r·|yaw rate|`, so an attempt a wall blocks still pays, and
    /// turning while stopped pays. The frozen default sets full cruise at the same rate
    /// as basal upkeep.
    pub motor_respiration_per_s: f64,
    /// The lineage's **cruise**, in body lengths per second (decisions: pace is calibrated
    /// in body lengths, 1 BL/s; the lanternjaw's 0.6 wins by the lunge and not the
    /// chase, slower than a fleeing grazer). The motion rule's full forward is this ×
    /// the adult length ([`FaunaConfig::manifest`](crate::FaunaConfig::manifest)); the
    /// manifest's digest carries the frozen table's value.
    pub cruise_body_lengths_per_s: f64,
    /// The yaw cap, radians per second: full turn effort turns this fast.
    pub yaw_cap_rad_per_s: f64,
    /// The lineage's **step up and step down in metres**: how far up, and how far down, a
    /// body may put its feet in one sub-step, and therefore what counts as a ledge rather
    /// than a wall or a cliff (`design/handoffs/voxel-founder-step-2026-09-22.md`). The
    /// contract records both (D1). Converted to whole
    /// voxels once at the consumer by [`FounderPhysiology::step_limits`], which rounds
    /// **down**: a riser is a step when its height is within the limit.
    ///
    /// Browser **0.375 m both ways**, its own height (Fable, 2026-09-23). The brief had
    /// 0.75 m down (its own length), but a drop it cannot climb back is a pit, and the
    /// heuristic browsers walked into them and starved (seed 4's browser #15, two
    /// voxels down a hollow at 32 min). A longer drop comes back after the retrain, when
    /// a policy can learn to avoid pits. Shredder **0.25 m both ways**: a long crawler steps what its
    /// front segments reach, two thirds of its 0.375 m length, and taller risers are walls
    /// it climbs. That is the step it effectively had on the 0.25 m presets, where the old
    /// 0.125 m rounded up to a whole voxel. Measured: at a literal 0.125 m every 0.25 m
    /// terrace of the default preset became a wall, and the shredders spent 51 % of their
    /// time on faces (≈ 500 climbs an hour, seeds 1–2) against 8–10 % at 0.25 m.
    pub step_up_m: f64,
    pub step_down_m: f64,
    /// Whether this lineage **climbs vertical terrain**: a
    /// riser taller than its step up is a face it can go up, an edge deeper than its step
    /// down a face it can go down, at half its walking pace. Stands are never faces. The
    /// shredder does ("basically centipedes"); the browser does not.
    pub climbs_walls: bool,
    /// The motor cost of a second on a wall, **× the walking cost at the same effort**.
    ///
    /// **2.0.** Climbing at half pace the legs still cycle as they would walking, and the
    /// body is lifted besides: at the shredder's 0.375 m/s cruise, lifting at 0.19 m/s
    /// costs `g · w / η` ≈ 9.8 × 0.19 / 0.25 ≈ 7.4 W/kg, against ≈ 7–10 W/kg for level
    /// walking at a small arthropod's cost of transport (≈ 20–28 J/kg/m) — so about as
    /// much again. The browser never climbs a wall and never pays it.
    pub wall_climb_cost_factor: f64,
    /// The pace of a wall climb, **× the cruise** (half walking pace, a decision of package
    /// mobility): climbing a riser or a drop of `rise_m` takes `rise_m / (fraction ×
    /// cruise)`. The shredder climbs; the browser never does and never reads it.
    pub climb_speed_fraction: f64,
    /// The deepest standing water the body walks into, **× its current height**: browser
    /// 0.5 (0.19 m for the adult), shredder 0.25 (≈ 3 cm), the brief's numbers. A juvenile
    /// is shorter, so it wades shallower water.
    pub wade_height_fraction: f64,
    /// Standing water deeper than this **× its current height** over its feet is water it
    /// is under: 1.0, its own height.
    pub drown_height_fraction: f64,
    /// Seconds it must be under, **continuously**, before it drowns: 60, the brief's
    /// number. A tick out of the water resets the count.
    pub drown_after_s: f64,
    /// The **adult** body, in metres: length, width and height
    /// (`design/handoffs/voxel-body-anchors-2026-09-22.md`; decisions §1). The ladder's
    /// animal at the sizes — browser 0.75 × 0.375 × 0.375 m, shredder 0.375 ×
    /// 0.125 × 0.125 m. Contract v2 (D1) reads the [`Manifest`](crate::Manifest)'s
    /// `body_length_m` / `body_width_m` / `body_height_m` and its 1 BL/s cruise off these,
    /// so moving them moves the trained-policy digest. A living body's dimensions are
    /// these times `(body / body_max)^(1/3)` ([`FounderPhysiology::body_at`]).
    /// Placeholders, `design/backlog.md` §1.
    pub adult_length_m: f64,
    pub adult_width_m: f64,
    pub adult_height_m: f64,
    /// The eye, as a fraction of body height over the standing surface (decisions §6:
    /// 0.8). A placeholder, `design/backlog.md` §1.
    pub eye_height_fraction: f64,
    /// The mouth band's ceiling, as a fraction of body height over the standing surface
    /// (decisions §2: 1.33, the dossier's raised neck). A placeholder.
    pub mouth_ceiling_fraction: f64,
    /// The mouth band's **floor**, as a fraction of body height over the standing
    /// surface: the band is `[floor, ceiling] × height`. 0 for every lineage so far (a
    /// mouth that reaches the ground); a tall browser that feeds well above it sets more.
    /// Ground pools (litter, carrion) are taken at the standing face whatever the floor.
    pub mouth_floor_fraction: f64,
    /// **The sloppy feeder's spill** (roster R1): this fraction of every bite's withdrawn
    /// tissue goes to the litter pool at its standing face instead of its mouth, a
    /// mass-balance flow (the ledgers close). 0 for every lineage but the loftstrider
    /// (0.25). The plant layer gives the whole bite (its `consumed_*`); the animal books
    /// it eaten and at once deposits this share of it, organic, mineral and energy pro
    /// rata, as litter at its standing face (`deposited_*`), before assimilating the rest.
    pub bite_spill_fraction: f64,
    /// The contact receptors' height, as a fraction of body height over the standing
    /// surface (0.5). A placeholder.
    pub contact_height_fraction: f64,
    /// The mouth's horizontal reach ahead of the footprint, as a fraction of body
    /// length (decisions §2: 0.25, "as today"). A placeholder.
    pub mouth_reach_length_fraction: f64,
    /// **Assimilation yield per [`crate::Food`] class**, in [`crate::Food::ALL`] order
    /// (decisions §3; placeholders, `design/backlog.md` §1).
    ///
    /// The assimilation rule is `min(yield · organic, mineral / n_tissue)`, and this is
    /// the `yield` it reads, chosen by what was eaten rather than by who ate it, because a
    /// detritivore with three foods has no single one.
    ///
    /// Every lineage fills all four honestly, including classes its diet never reaches:
    /// a zero there would be a trap for whoever adds a food later.
    pub yield_by_food: [f64; crate::Food::COUNT],
    /// `f`, the declared share of the body's **total** structure that is sensor/organ
    /// tissue: 5% for the blind founder, 10% for the browser. The body's structure
    /// stock is the total; the sensor share is counted within it once — no second
    /// ledger — and the body's usual maintenance applies to the total. Adding organs
    /// raised paid structure when the founder was sized from a core budget; it came
    /// with no free reserve or energy.
    pub organ_structure_fraction: f64,
    /// When this lineage is awake. RHYTHMS-CADENCE (Wrysk 2026-10-09): every lineage but the
    /// lanternjaw (dark-active) and the ripple-snail (its own night-and-rain rule) leans
    /// ([`crate::Activity::Leaning`], its [`FounderPhysiology::rhythm`]).
    pub active: crate::Activity,
    /// **Its activity window** (LURE, [`crate::ActiveWindow`]): hours after sunset and before
    /// sunrise it is awake on top of [`FounderPhysiology::active`]. Zero for every lineage
    /// but the bellwing (1 h each side: dusk and dawn flights).
    #[serde(default)]
    pub active_window: crate::ActiveWindow,
    /// Switch: hunt and light the flower mimic in daylight as well as darkness.
    #[serde(default)]
    pub lure_daylight: bool,
    /// Metres from a flower at which a daytime mimic holds its ambush.
    #[serde(default)]
    pub flower_ambush_m: f64,
    /// Switch: an opportunistic fruit eater also catches bellwings within reach.
    #[serde(default)]
    pub catch_bellwings: bool,
    /// Switch: its [`FounderPhysiology::hunter`] table is the **opportunistic catch's**
    /// (the seedporter's, MIMIC), so it exists only while [`FounderPhysiology::catch_bellwings`]
    /// is on, and carrion is food only then. A dedicated predator (lanternjaw, Veilhound)
    /// leaves this off: its table has no switch. Behaviour reads the table through
    /// [`FounderPhysiology::active_hunter`] and the mouth's diet through
    /// [`FounderPhysiology::eating_diet`].
    #[serde(default)]
    pub opportunist: bool,
    /// Metres above the hunter's base: maximum bellwing base height for a catch.
    #[serde(default)]
    pub catch_ceiling_m: f64,
    /// Organic units: minimum carrion yield less the expected strikes' upkeep cost.
    #[serde(default)]
    pub catch_gain_min: f64,
    /// Metres: radius in which sensed low bellwing clusters invite a foray (0 disables).
    #[serde(default)]
    pub catch_foray_m: f64,
    /// Bodies: minimum nearby, sensed, profitable low bellwings to leave fruit search.
    #[serde(default)]
    pub catch_foray_min: f64,
    /// Seconds: maximum fruit-search detour without a catch, then avoid its fruitless patch.
    #[serde(default)]
    pub catch_foray_s: f64,
    /// What upkeep costs while it sleeps, **× its basal upkeep**: 0.5 for endotherm-like
    /// lineages, 0.3 for the ectotherm-like shredder, ripple-snail, capgnawer and
    /// lanternjaw (`design/ecology-timescales-2026-10-02.md` 1a; provisional). A flyer
    /// perched while grounded by rain ([`crate::Mobility::rain_grounded`]) pays it too.
    pub sleep_upkeep: f64,
    /// Hunger overrides sleep: a body whose hunger (the reserve's shortfall from its cap,
    /// 0 full to 1 empty) is at or above this stays awake and forages. 0.9 (1a, 2026-10-02;
    /// 0.5 before): a sleeper wakes with a tenth of its reserve, on the emergency latch
    /// ([`Animal::emergency`](crate::Animal::emergency)).
    pub wake_hunger: f64,
    /// **The emergency latch's clear line** (repair 2 item 7): a body whose emergency
    /// latch went on at `wake_hunger` keeps it until its hunger is below this. 0.6.
    pub emergency_clear_hunger: f64,
    /// **The minimal gut** ([`Gut`]): the stomach, its digestion, the bite and the
    /// appetite.
    pub gut: Gut,
    /// `Some` for a **carnivore** (`design/predation-and-flight-2026-09-28.md` §2.1,
    /// §2.3, §2.12): its prey list and size window, the strike and the lure. `None` for
    /// every lineage that eats no animal. Its satiated rest went with the minimal gut: a
    /// fed hunter is one whose eating latch is off.
    pub hunter: Option<Hunter>,
    /// The damage multiplier a hit is reduced by, `1 − armour` (§2.7): the grazer's saddle
    /// shield 0.25, the shredder 0.
    pub armour: f64,
    /// The species factor of the dodge (§2.3's table): frondgrazer 0.5, littershredder
    /// 0.3. The lanternjaw is in nobody's prey list yet, so its 0 is never read.
    pub agility: f64,
    /// The counter-strike seam (§2.7): fired when struck, wounding the attacker. `None`
    /// for every lineage until the loftstrider's kick.
    pub counter: Option<Counter>,
    /// **The org-unit mass scale this physiology was built at** ([`FounderPhysiology::scale_mass`]
    /// multiplies the seven absolute quantities of [`FounderPhysiology::core`] and records
    /// the factor here). The senses read it: the `Self` references and taste's reference
    /// are the manifest's times this (`manifest::SELF_REFERENCE_RULE`,
    /// `TASTE_REFERENCE_RULE`), so a heavier body reads "a fraction of an adult" and a
    /// "full" taste means the same share of its need. The founding gate judges a body as
    /// a x1 one with it (`cubarium_voxel_sim::habitat`).
    #[serde(default = "one")]
    pub mass_scale: f64,
    /// **Taste's reference stock** (roster R2b repair): the food stock per site, organic
    /// units, that reads as a taste cue of 1 before saturation (the mouth's response is
    /// `min(stock / this, 1)` through the chem saturation curve). `None` keeps the
    /// manifest rule every older lineage uses, `emit_reference × mass_scale`
    /// ([`FounderPhysiology::taste_reference_stock`], `manifest::TASTE_REFERENCE_RULE`), so
    /// their cues and digests do not move. The ripple-snail's is the glassfilm's capacity
    /// per 0.125 m site: a film's stock is a few thousandths of a unit, far under the
    /// litter-scaled rule's 1.5 at x30, which no film could ever reach the feed gate on.
    /// Not scaled by [`FounderPhysiology::scale_mass`]: the food's stock does not change
    /// with the eater's mass.
    #[serde(default)]
    pub taste_reference: Option<f64>,
    /// **What it eats** ([`crate::Diet`]): the food classes its mouth takes, each from its
    /// own source. Shredder `[litter, carrion]` (it lost the caps in roster R1), browser
    /// `[foliage]`, lanternjaw `[carrion]`, capgnawer `[cap_tissue, litter]`, loftstrider
    /// `[foliage]`. Not a number, so it has no registry row; set it in TOML as
    /// a list of food names.
    pub diet: crate::Diet,
    /// **How a fresh world founds it** ([`Founding`]): bodies, or the egg bank's clutches.
    pub founding: Founding,
    /// Where a founding body is placed ([`FoundingSite`]); unread for [`Founding::Clutches`].
    pub founding_site: FoundingSite,
    /// **How many** a fresh world founds: bodies, or for [`Founding::Clutches`] clutches.
    /// The code default of a new lineage is 0, so training worlds are unchanged until a
    /// preset turns it on (`[fauna.founders.<lineage>] founding_count = N`). Shredder 8,
    /// browser 8, lanternjaw 3 clutches.
    pub founding_count: usize,
    /// **Founded per world, not per land area**: the founding never scales
    /// [`FounderPhysiology::founding_count`] by the world's dry support area. `true` for
    /// the megafauna that live as one group per world (Wrysk, 2026-10-09: "one veilhound
    /// pack and one loftstrider family per wide d96 world … all those numbers scale
    /// proportionally to wide except for vh/ls"). Not a number, so it has no registry row.
    #[serde(default)]
    pub founding_per_world: bool,
    /// **A water-bound body** (roster R2b, the ripple-snail): it never drowns, pays no
    /// wade refusal, and refuses a step whose destination support face has no standing
    /// water within [`FounderPhysiology::water_margin_m`]. `false` for every other
    /// lineage. Not a number, so it has no registry row.
    pub aquatic: bool,
    /// An aquatic body's reach out of the water, metres: a step's destination face must
    /// have standing water within this distance (0.25 for the ripple-snail). Unread for a
    /// lineage that is not [`FounderPhysiology::aquatic`].
    pub water_margin_m: f64,
    /// `Some` for an **amphibious** body (AMPHIBIOUS-SNAIL, Wrysk 2026-10-06, the
    /// ripple-snail): an [`FounderPhysiology::aquatic`] body that also crawls out onto wet
    /// ground on a drying clock ([`Amphibious`]). `None` keeps the water-bound rule
    /// (`water_margin_m` only), as for every other lineage.
    #[serde(default)]
    pub amphibious: Option<Amphibious>,
    /// **Where an egg-layer's clutch goes** ([`EggSite`]): a face holding litter (the
    /// shredder, the capgnawer) or a face holding film (the ripple-snail). Not a number,
    /// so it has no registry row.
    pub egg_site: EggSite,
    /// `Some` for a **flyer** (F1, predation-and-flight §3.2–3.4, §3.8): its span, band,
    /// flight costs and weather response; the lineage has the `Fly` and `Perched` modes.
    /// `None` for every walker.
    #[serde(default)]
    pub flight: Option<Flight>,
    /// `Some` for a **glider** (F3, predation-and-flight §3.5, the seedporter): it climbs
    /// the stems of tall stands and latticevine-covered faces, perches in crowns and glides
    /// down from height; the lineage has the `Climb`, `Glide` and `Perched` modes. Its
    /// climb pace and cost are [`FounderPhysiology::climb_speed_fraction`] and
    /// [`FounderPhysiology::wall_climb_cost_factor`]. `None` for every other lineage.
    #[serde(default)]
    pub glider: Option<Glider>,
    /// `Some` for a lineage that **carries seeds in its gut** (F3, the seedporter): a fruit
    /// meal records the source species' seeds, voided at the body's face after the
    /// transit time. `None` for every other lineage.
    #[serde(default)]
    pub gut_seeds: Option<GutSeeds>,
    /// **Coat factor for burrs** (springturf spread, 2026-10-04): how readily a seed that
    /// latches on a passing body holds to this lineage's coat, multiplying the plant's
    /// per-contact pickup chance (`flora.*.burr_pickup`). 0 picks up nothing; 1 is a
    /// coat as catchy as the plant's chance assumes. Proposed from each body
    /// (`placeholder`'s `burr_coat_of`), for Wrysk to judge.
    #[serde(default)]
    pub burr_coat: f64,
    /// **Its senses** ([`SenseTable`]; SENSES-CORE): sight, hearing, tremor and smell, each
    /// with its range and floor. Read by [`crate::perception`] for a lineage whose table is
    /// `live` (the Veilhound in S1); inert for the others until S2 and S3.
    #[serde(default)]
    pub senses: SenseTable,
    /// **Its ground response** (S2, [`crate::Vigilance`]): what its heuristics do with what its
    /// live table senses. Inert while its table is not live.
    #[serde(default)]
    pub vigilance: crate::Vigilance,
    /// **Its sprint** (GAIT, [`crate::Sprint`]): the gait above cruise its rush or ground
    /// flight asks for, and the stamina that limits it. None for a lineage that only walks.
    #[serde(default)]
    pub sprint: crate::Sprint,
    /// **Its hunker** (HUNKER, [`crate::Hunker`]): the armoured defence a herd lineage takes
    /// instead of flight. Off for every lineage but the frondgrazer.
    #[serde(default)]
    pub hunker: crate::Hunker,
    /// **Its group pull** (HERD-PULL, [`crate::Herding`]): its heuristic keeps it near the kin
    /// it knows (a herd, a troop, a calf's family). Off for the lineages that do not herd.
    #[serde(default)]
    pub herding: crate::Herding,
    /// **Its jump down when trapped** (DROP, [`crate::DropRule`]): off its route graph with no
    /// walking way back on and no glide, it jumps down past its step, at a wound. No jump for
    /// the bellwing and the ripple-snail.
    #[serde(default)]
    pub drop: crate::DropRule,
    /// **Its daily leaning** (RHYTHMS-CADENCE, [`crate::Rhythm`]): read while
    /// [`FounderPhysiology::active`] is [`crate::Activity::Leaning`].
    #[serde(default)]
    pub rhythm: crate::Rhythm,
    /// **Its stop-and-go pace** (RHYTHMS-CADENCE, [`crate::Cadence`]) while it searches or
    /// forages on its own heuristic. Off for the lineages that are not paced.
    #[serde(default)]
    pub cadence: crate::Cadence,
    /// `Some` for a **pack lineage** (roster R3, the chorister; predation-and-flight
    /// §2.10, §2.14): its pack, den, breeding, provisioning, territory, chase and call
    /// numbers ([`Pack`]). `None` for every other lineage.
    #[serde(default)]
    pub pack: Option<Pack>,
}

/// A pack lineage's table ([`FounderPhysiology::pack`], roster R3; the chorister's numbers
/// are Brief R3's decisions, the rest placeholders).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pack {
    /// **The pack seam**: the size window measures prey against the summed body of the
    /// striker's packmates (itself included) within this, metres. 6.
    pub pack_radius_m: f64,
    /// Adults per pack at founding ([`Founding::Pack`]); the first is the breeder. 4.
    pub pack_size: usize,
    /// A den is covered: solid within this over its face, metres. 3.
    pub den_cover_m: f64,
    /// Nursery and provisioning radius around the den, metres; pups stay here. Adults
    /// resting or sleeping use the smaller `rest_radius_m`.
    pub den_radius_m: f64,
    /// Young in one litter: 2 up to this. 4.
    pub litter_size_max: u32,
    /// The breeder's litters are at least this far apart, seconds. 3 clock-days (the
    /// default day clock's 7,200 s day).
    pub litter_interval_s: f64,
    /// A pup cannot strike and stays at the den until this age, seconds; then it joins the
    /// pack. 1 clock-day.
    pub independence_age_s: f64,
    /// An adult at the den passes reserve to the hungriest pup there while its reserve is
    /// above this fraction of its reserve cap. 0.6.
    pub provision_reserve_fraction: f64,
    /// The provisioning transfer, organic units per second per unit of the giver's body.
    /// A placeholder.
    pub provision_rate_per_s: f64,
    /// The share of a transfer the pup receives; the rest is respired. 0.7.
    pub provision_efficiency: f64,
    /// A pack's heuristic never paths within this of another pack's den, metres. 10.
    pub territory_radius_m: f64,
    /// Flankers go to points this far either side of the pack target, metres. A
    /// placeholder.
    pub flank_offset_m: f64,
    /// A chase is dropped after this long without progress, seconds (CHURN: the pack closed
    /// on its target by `chase_progress_m`, or worked it — a wound or pry gained). 20.
    pub chase_give_up_s: f64,
    /// **CHURN, the progress**: a chase's give-up clock restarts when the pack's nearest
    /// member has come this much closer to the target than its nearest so far, metres.
    #[serde(default = "pack_chase_progress_m")]
    pub chase_progress_m: f64,
    /// **CHURN, the commitment**: in a stalled chase ([`Pack::switch_stall_s`]), another
    /// target takes the hunt only when it is worth at least this many times the current
    /// target (CHURN2: prospect × body, both weighed with the pack's participants and no
    /// awareness; hysteresis: an equal or slightly better prey never steals it).
    #[serde(default = "pack_switch_margin")]
    pub switch_margin: f64,
    /// **CHURN2, the stall**: a chase is open to a switch only after this long without
    /// progress (the give-up's progress: closing in, or working the target), seconds; below
    /// `chase_give_up_s`. A pack closing on or working its target never switches (MacNulty
    /// et al. 2007: wolves test, then commit and stay on that animal).
    #[serde(default = "pack_switch_stall_s")]
    pub switch_stall_s: f64,
    /// **STRIKE, the last metres**: within this of its target (centre to centre), with the
    /// target in its jaw band from where it stands and a straight walk to it, a member leaves
    /// its route and its flank point and steers to its **stand-off**, facing the prey: it turns
    /// in place to a prey to its side or behind it, steps off from one inside its outline, and
    /// never presses into it ([`crate::pack`]'s close-in). Metres; 0 turns it off.
    #[serde(default = "pack_close_in_m")]
    pub close_in_m: f64,
    /// ...the stand-off: the jaw's reach ahead of its centre (`jaw_forward_length_fraction`)
    /// plus this share of the strike's reach (`strike_reach_length_fraction`), from the prey's
    /// centre; 0 stands it at the jaw (where a lunge that cannot move still meets it), 0.5 in
    /// the middle of the strike capsule.
    #[serde(default = "pack_stand_off_reach_fraction")]
    pub stand_off_reach_fraction: f64,
    /// **STRIKE-2, leaving the herd**: a pack that drops an armoured target alive (hunkered,
    /// not turned over: given up late, or broken off) also shuns, for `shun_s`, its armoured
    /// kin within this of it — hunkered, not turned over, and more than one member's work to
    /// open — the herd that stood against it, nearest first, as many as the record holds
    /// beside the target; so it leaves the herd rather than cycling through it (musk oxen in
    /// a ring are left by wolves that cannot break it). Metres; 0 shuns the target alone.
    #[serde(default = "pack_herd_shun_m")]
    pub herd_shun_m: f64,
    /// **PIT, the way out**: a member standing off its route graph (no node within its
    /// footprint of where it stands: a stream channel, a slope ledge, a pocket its disc fits
    /// but the graph's margin does not) walks back onto it by the shortest way its own body can
    /// step — its own disc, step and wade depth, face by face — searched this far round it,
    /// metres, rather than pressing on straight at its goal into a riser. 0: off.
    #[serde(default = "pack_pit_search_m")]
    pub pit_search_m: f64,
    /// **PIT-2, the turn onto a route's next leg**: a member walking its route (home, to a
    /// strike face, behind the draw's leader, or onto the graph) steers at the farthest node
    /// its line reaches on the graph within a controller period's walk, slows to land on it
    /// within the period, and while turning walks no faster than carries it this many voxels
    /// across its line in a period. (At its full pace it ran two or three columns past the
    /// next node at a bend, off the graph onto a ledge it could not leave.) 0: no limit on
    /// the turn.
    #[serde(default = "pack_route_swing_voxels")]
    pub route_swing_voxels: f64,
    /// **PIT-2, the litter at the den**: the breeder carrying a litter goes home and stays
    /// within half the den radius until it gives birth (not in an emergency), so its pups are
    /// born where they live. False: it gives birth wherever it is.
    #[serde(default = "pack_whelp_at_den")]
    pub whelp_at_den: bool,
    /// A pack takes only prey standing in its own walking route component; the
    /// lineage's components are rebuilt from the terrain at most this often, seconds (a
    /// placeholder: the terrain and its water change slowly). 60.
    #[serde(default = "pack_route_refresh_s")]
    pub route_refresh_s: f64,
    /// **DEN-FLOOD**: a den counts as flooded (left for a dry site, and not walked home to)
    /// when its standing water is deeper than this fraction of an adult's wade depth
    /// ([`FounderPhysiology::wade_depth_m`]), dimensionless. Damp ground or a film of float
    /// dust is not a flood: a den held 1.7e-18 m and the pack treated it as one. 0.1 (the
    /// Veilhound wades 0.3 m, so 3 cm: a puddle a body lies in notices, a trace does not).
    #[serde(default = "pack_den_flood_wade_fraction")]
    pub den_flood_wade_fraction: f64,
    /// A pack takes prey only within this share of what it covers in a chase on foot:
    /// route distance at most `cruise × chase_give_up_s × route_reach_fraction`. 0.8.
    #[serde(default = "pack_route_reach_fraction")]
    pub route_reach_fraction: f64,
    /// ...or when a member's reserve falls under this fraction of its reserve cap. A
    /// placeholder.
    pub chase_reserve_floor: f64,
    /// A member's `calling` flag stays set this long after the pack target changes or it
    /// strikes, seconds. 1.
    pub call_s: f64,
    /// Search radius around the founding point or current range, metres. Dry, reachable
    /// roofed sites beat open ground; glowcap darkness then sky visibility break ties.
    pub den_search_m: f64,
    /// An idle adult that is not the breeder walks back to within this of the breeder,
    /// metres: the pack keeps together between hunts. A placeholder.
    pub follow_radius_m: f64,
    /// Metres from the den centre within which an adult rests or sleeps, on the den
    /// level; capped by `den_radius_m` so a small nursery stays small.
    #[serde(default = "pack_rest_radius_m")]
    pub rest_radius_m: f64,
    /// Fraction of reserve above the emergency-wake floor available for the trip home.
    /// The route estimate includes awake basal upkeep and motor cost; 0 permits no trip.
    #[serde(default = "pack_home_reserve_share")]
    pub home_reserve_share: f64,
    /// **The stalk** (S2, [`crate::vigil::stalk`]): closing on its prey beyond `commit_m`, a
    /// member walks no faster than keeps its emission under this fraction of the prey's
    /// hearing floor; 0 does not stalk. 0.7.
    #[serde(default = "pack_stalk_margin")]
    pub stalk_margin: f64,
    /// ...and rushes within this of it, metres. 2.5.
    #[serde(default = "pack_commit_m")]
    pub commit_m: f64,
    /// ...and stands still while a prey it sees is in a scan pause (a switch, no registry
    /// row).
    #[serde(default = "pack_stalk_freeze")]
    pub stalk_freeze: bool,
    /// **The breeding reserve** (LIFE-HISTORY): the breeder conceives only with its reserve at
    /// or above this fraction of its reserve cap. 0: no threshold beyond the ordinary surplus.
    #[serde(default)]
    pub breed_reserve_fraction: f64,
    /// **A litter by condition** (LIFE-HISTORY; a switch, no registry row): the litter is
    /// `1 + floor(c × litter_size_max)`, at most `litter_size_max`, where `c` is the breeder's
    /// reserve fraction above `breed_reserve_fraction` as a share of the rest of the cap
    /// (`(r − f) / (1 − f)`), so a breeder in poor condition has one pup and a full one the
    /// most. Off: the old uniform draw on 2..=`litter_size_max`. Either way never more young
    /// than its reserve above the surplus floor funds.
    #[serde(default)]
    pub litter_by_condition: bool,
    /// **The giver feeds itself first** (PUP): an adult at the den regurgitates its meal to a
    /// pup only while its own reserve is at or above this fraction of its cap; below it, it
    /// keeps the meal (wolves and lions in poor condition eat at the kill and carry home only
    /// a surplus: Mech & Boitani 2003; Packer et al. 1988). 0: the old rule, the giver's
    /// condition never matters.
    #[serde(default)]
    pub regurgitate_reserve_fraction: f64,
    /// **Scramble provisioning** (PUP; a switch, no registry row): food at the den goes to the
    /// strongest begging pup first (the largest body, then the fullest reserve, then the lowest
    /// id) instead of the hungriest, so a short ration feeds the strong and the runt starves
    /// (brood reduction: wolf and lion young compete for what is brought). Off: the old
    /// hungriest-first rule, which shares a short ration out evenly.
    #[serde(default)]
    pub provision_scramble: bool,
    /// **A pup lives on its reserve** (PUP; a switch, no registry row): a pup whose reserve is
    /// spent and whose stomach is empty has starved. A growing young body has little fat and
    /// no structure to spare: it starves within days of its last meal, where an adult lives
    /// for weeks off its own tissue (wolf pups in a lean summer, Mech 1977; Fuller, Mech &
    /// Cochrane 2003; lion cubs, Packer et al. 1988). Off: a pup starves as an adult does,
    /// only when its structure falls under `body_min` — and a newborn's birth stores, built
    /// into structure, outlast its whole dependence (PUP's diagnosis: an unfed newborn lives
    /// 63,000 s, independence is 21,600 s).
    #[serde(default)]
    pub pup_starves_on_empty: bool,
    /// **The prey choice** (PACK-TUNE, [`crate::pack::prospect`]): a pack weighs each target
    /// it could take by its prospect — the chance-like share of a hunt on it that ends in a
    /// kill, from what it can see — times its meat, over its distance, and commits to the best.
    /// The prospect: 0 for a hunkering lineage's body the members present cannot open (its
    /// [`crate::Hunker::members_to_open`] with the kin within its ring), else the share of
    /// their pry left over its resistance; × `select_kin_factor` per kin of its lineage within
    /// `select_kin_m` (at most `select_kin_max`); × `select_aware_factor` when it is aware of
    /// a member; × (1 + `select_wound_gain` × its wound); × `select_airborne_factor` in flight
    /// (in reach). 1 for an isolated, unaware, unarmoured body on the ground.
    #[serde(default = "pack_select_kin_m")]
    pub select_kin_m: f64,
    #[serde(default = "pack_select_kin_factor")]
    pub select_kin_factor: f64,
    #[serde(default = "pack_select_kin_max")]
    pub select_kin_max: u32,
    #[serde(default = "pack_select_aware_factor")]
    pub select_aware_factor: f64,
    #[serde(default = "pack_select_wound_gain")]
    pub select_wound_gain: f64,
    #[serde(default = "pack_select_airborne_factor")]
    pub select_airborne_factor: f64,
    /// The score is prospect × body^`select_body_exp` / (1 + distance / `select_judge_m`).
    #[serde(default = "pack_select_body_exp")]
    pub select_body_exp: f64,
    #[serde(default = "pack_select_judge_m")]
    pub select_judge_m: f64,
    /// **The straggler** (HERD-PULL, Wrysk 2026-10-09: "Veilhound packs single out the most
    /// isolated grazer"): the score is also × (1 + `select_straggler_gain` × min(d,
    /// `select_straggler_m`) / `select_straggler_m`), d the distance from the prey to the
    /// nearest awake body of its lineage (none: the full gain). A choice among prey, not a
    /// prospect: it never makes a pack decline a hunt. Metres, factor; either 0 turns it off.
    #[serde(default = "pack_select_straggler_m")]
    pub select_straggler_m: f64,
    #[serde(default = "pack_select_straggler_gain")]
    pub select_straggler_gain: f64,
    /// It commits only to a prospect of at least this.
    #[serde(default = "pack_select_min_prospect")]
    pub select_min_prospect: f64,
    /// **The test**: in a chase, a target whose prospect falls under this is broken off.
    #[serde(default = "pack_break_off_below")]
    pub break_off_below: f64,
    /// A target dropped alive (broken off, late or lost) is not taken again for this, seconds.
    #[serde(default = "pack_shun_s")]
    pub shun_s: f64,
}

fn pack_select_kin_m() -> f64 {
    Pack::CHORISTER.select_kin_m
}
fn pack_select_kin_factor() -> f64 {
    Pack::CHORISTER.select_kin_factor
}
fn pack_select_kin_max() -> u32 {
    Pack::CHORISTER.select_kin_max
}
fn pack_select_aware_factor() -> f64 {
    Pack::CHORISTER.select_aware_factor
}
fn pack_select_wound_gain() -> f64 {
    Pack::CHORISTER.select_wound_gain
}
fn pack_select_airborne_factor() -> f64 {
    Pack::CHORISTER.select_airborne_factor
}
fn pack_select_body_exp() -> f64 {
    Pack::CHORISTER.select_body_exp
}
fn pack_select_judge_m() -> f64 {
    Pack::CHORISTER.select_judge_m
}
fn pack_select_straggler_m() -> f64 {
    Pack::CHORISTER.select_straggler_m
}
fn pack_select_straggler_gain() -> f64 {
    Pack::CHORISTER.select_straggler_gain
}
fn pack_select_min_prospect() -> f64 {
    Pack::CHORISTER.select_min_prospect
}
fn pack_break_off_below() -> f64 {
    Pack::CHORISTER.break_off_below
}
fn pack_shun_s() -> f64 {
    Pack::CHORISTER.shun_s
}
fn pack_chase_progress_m() -> f64 {
    Pack::CHORISTER.chase_progress_m
}
fn pack_switch_margin() -> f64 {
    Pack::CHORISTER.switch_margin
}
fn pack_switch_stall_s() -> f64 {
    Pack::CHORISTER.switch_stall_s
}
fn pack_close_in_m() -> f64 {
    Pack::CHORISTER.close_in_m
}
fn pack_stand_off_reach_fraction() -> f64 {
    Pack::CHORISTER.stand_off_reach_fraction
}
fn pack_herd_shun_m() -> f64 {
    Pack::CHORISTER.herd_shun_m
}
fn pack_pit_search_m() -> f64 {
    Pack::CHORISTER.pit_search_m
}
fn pack_route_swing_voxels() -> f64 {
    Pack::CHORISTER.route_swing_voxels
}
fn pack_rest_radius_m() -> f64 {
    Pack::CHORISTER.rest_radius_m
}

fn pack_home_reserve_share() -> f64 {
    Pack::CHORISTER.home_reserve_share
}

fn pack_whelp_at_den() -> bool {
    Pack::CHORISTER.whelp_at_den
}

fn pack_stalk_margin() -> f64 {
    Pack::CHORISTER.stalk_margin
}

fn pack_commit_m() -> f64 {
    Pack::CHORISTER.commit_m
}

fn pack_stalk_freeze() -> bool {
    Pack::CHORISTER.stalk_freeze
}

fn pack_route_refresh_s() -> f64 {
    Pack::CHORISTER.route_refresh_s
}

fn pack_den_flood_wade_fraction() -> f64 {
    Pack::CHORISTER.den_flood_wade_fraction
}

fn pack_route_reach_fraction() -> f64 {
    Pack::CHORISTER.route_reach_fraction
}

impl Pack {
    /// **A litter by condition** (LIFE-HISTORY, [`Pack::litter_by_condition`]): a breeder at
    /// reserve fraction `r` has `1 + floor(c × litter_size_max)` young, at most
    /// `litter_size_max`, with `c = (r − f) / (1 − f)` clamped to `[0, 1]` and `f` the
    /// [`Pack::breed_reserve_fraction`].
    pub fn condition_litter(&self, r: f64) -> u32 {
        let hi = self.litter_size_max.max(1);
        let f = self.breed_reserve_fraction.clamp(0.0, 1.0);
        let c = if f < 1.0 { ((r - f) / (1.0 - f)).clamp(0.0, 1.0) } else { 1.0 };
        (1 + (c * f64::from(hi)).floor() as u32).min(hi)
    }

    /// The chorister's table (Brief R3).
    pub const CHORISTER: Pack = Pack {
        pack_radius_m: 6.0,
        pack_size: 4,
        den_cover_m: 3.0,
        den_radius_m: 4.0,
        litter_size_max: 4,
        litter_interval_s: 3.0 * 7200.0,
        independence_age_s: CHORISTER_INDEPENDENCE_S,
        provision_reserve_fraction: 0.6,
        provision_rate_per_s: 0.01,
        provision_efficiency: 0.7,
        territory_radius_m: 10.0,
        flank_offset_m: 1.5,
        chase_give_up_s: 20.0,
        chase_progress_m: 0.5,
        switch_margin: 1.5,
        switch_stall_s: 10.0,
        // STRIKE: the commit distance (`commit_m`, where the rush begins) is where a wolf or a
        // lion leaves its line of approach for the animal itself (MacNulty et al. 2007's
        // attack stage); the stand-off is at the jaw's reach (the probe: from the capsule's
        // middle, a lunge blocked by a ledge or a stem swept short of the prey, 45 of 50
        // strikes on perched bellwings missing by geometry).
        close_in_m: 2.5,
        stand_off_reach_fraction: 0.0,
        // STRIKE-2: the herd a pack leaves, twice the frondgrazer's ring (`Hunker::ring_m`,
        // the kin beside it that stiffen it): a placeholder.
        herd_shun_m: 3.0,
        // PIT: the way out of a pit is looked for within about twice the den radius (a
        // placeholder; the probe's pits were a few metres from the graph).
        pit_search_m: 4.0,
        // PIT-2: a quarter voxel across its line a period while it turns (a placeholder: the
        // bend case's ledge a column off the route held a half voxel's swing, not a quarter's).
        route_swing_voxels: 0.25,
        whelp_at_den: true,
        route_refresh_s: 60.0,
        den_flood_wade_fraction: 0.1,
        route_reach_fraction: 0.8,
        chase_reserve_floor: 0.3,
        call_s: 1.0,
        den_search_m: 8.0,
        follow_radius_m: 3.0,
        rest_radius_m: 0.5,
        home_reserve_share: 0.5,
        stalk_margin: 0.7,
        commit_m: 2.5,
        stalk_freeze: true,
        breed_reserve_fraction: 0.6,
        litter_by_condition: true,
        regurgitate_reserve_fraction: CHORISTER_REGURGITATE_RESERVE,
        provision_scramble: true,
        pup_starves_on_empty: true,
        // PACK-TUNE's prey choice, placeholders at the terrarium's scale. Wolves and lions
        // test prey and quit fast on healthy, defended animals; they take the young, the
        // isolated, the wounded and the unaware far more often than their share (Mech 1970,
        // *The Wolf*; MacNulty, Mech & Smith 2007, J. Mammal. 88:595, wolf hunting stages:
        // most encounters end at the approach or the watch; Packer & Ruttan 1988, Am. Nat.
        // 132:159). Kin within 3 m (about two grazer lengths) each take a fifth off, counting
        // four; an aware target half; a wound of w adds 2w; a body in flight in reach is a
        // quarter of one on the ground. Meat counts as the square root of the body (a big
        // carcass feeds the pack longer, not proportionally better); distance halves the
        // score at 6 m (the pack radius). Committed at 0.1 and broken off under 0.05; a
        // target dropped alive is left for 2 min.
        select_kin_m: 3.0,
        select_kin_factor: 0.8,
        select_kin_max: 4,
        select_aware_factor: 0.5,
        select_wound_gain: 2.0,
        select_airborne_factor: 0.25,
        select_body_exp: 0.5,
        select_judge_m: 6.0,
        // HERD-PULL's straggler: wolves and lions pick the animal that lags or stands apart
        // from its group (Mech 1970; FitzGibbon 1990, *Anim. Behav.* 39:1116, cheetahs
        // selecting the isolated and the less vigilant Thomson's gazelle). A prey with no
        // herd-mate within 6 m (twice the grazer's 3 m comfort, the pack radius) scores
        // double one with a herd-mate beside it; untuned.
        select_straggler_m: 6.0,
        select_straggler_gain: 1.0,
        select_min_prospect: 0.1,
        break_off_below: 0.05,
        shun_s: 120.0,
    };
}

/// **The Veilhound's life history** (LIFE-HISTORY, Wrysk 2026-10-06: "veilhound cubs should
/// be very expensive to birth and to rear. like lions and wolves only have one litter a year").
/// The Cubarium year is the dry-season cycle (5 world days, 10 h at 1×; phase 0 a season's
/// start, the season over phases 0–0.2). Read from wolves and lions:
///
/// - **One litter a year, conceived just after the dry season.** Wolves breed once a year in
///   a short late-winter season and pup in spring, when the ungulates calve (Mech & Boitani
///   2003, *Wolves: Behavior, Ecology, and Conservation*, ch. 2 and 6). The window opens at
///   phase 0.2, as the rains return and the grazers recover, and lasts 0.15 of the year
///   (wolves' oestrus season is about two months).
/// - **Gestation** 0.167 of the year, 6,000 s: the wolf's 63 days of 365 (lions 110).
///   Pregnancy raises upkeep by half ([`CHORISTER_PREGNANCY_UPKEEP`]): late gestation costs
///   a mammal roughly a quarter to a half above maintenance (Robbins 1993, *Wildlife Feeding
///   and Nutrition*).
/// - **Only a breeder in condition conceives**, at 60 % of its reserve cap (the same line it
///   provisions above), and its litter follows that condition, 1 to 4 (wolf and lion litter
///   size and pup survival track prey per adult: Fuller, Mech & Cochrane 2003; Packer et al.
///   1988, "Reproductive success of lions"). A pup's package stays two `body_min` (a litter of
///   four is three quarters of its mother's body).
/// - **Rearing**: a pup stays at the den, fed only by the pack, for 0.6 of the year
///   ([`CHORISTER_INDEPENDENCE_S`], 3 world days): wolf pups travel with the pack from about
///   six months and lion cubs depend on the pride for well over a year. A pup the pack cannot
///   feed starves; wild wolves and lions lose a third to two thirds of their young in a lean
///   year, the provisioning's real cost.
pub const CHORISTER_INDEPENDENCE_S: f64 = 3.0 * 7200.0;
/// The Veilhound's gestation, seconds (0.167 of the 5-day year; see [`CHORISTER_INDEPENDENCE_S`]).
pub const CHORISTER_GESTATION_S: f64 = 6000.0;
/// The Veilhound's pregnancy upkeep factor (see [`CHORISTER_INDEPENDENCE_S`]).
pub const CHORISTER_PREGNANCY_UPKEEP: f64 = 1.5;
/// **The Veilhound's regurgitation floor** (PUP): an adult under half its reserve cap eats its
/// meal itself and carries home only once it is back above half. Wolves in poor condition eat
/// at the kill and provision less, and in a lean year the pack's adults keep the food while
/// pups starve (Mech & Boitani 2003, ch. 2; Mech 1977, "Productivity, mortality and population
/// trends of wolves in northeastern Minnesota"); lionesses in poor condition abandon or
/// fail to feed cubs, which die of starvation in lean seasons (Packer et al. 1988; Packer &
/// Pusey 1997). Half the cap sits under the provisioning line (0.6), so an adult above it
/// regurgitates and gives reserve both, between them only regurgitates its fresh meal, and
/// under it gives nothing. Together with [`Pack::provision_scramble`] (wolf pups compete for
/// regurgitated food and the strongest beggar is fed first; lion cubs are displaced at the kill,
/// the smallest starving first: Packer & Pusey 1995) and [`Pack::pup_starves_on_empty`].
pub const CHORISTER_REGURGITATE_RESERVE: f64 = 0.5;

/// A glider's table ([`FounderPhysiology::glider`], F3; the seedporter's numbers are
/// predation-and-flight §3.5, §3.9 and the roster brief's Brief F3, placeholders).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Glider {
    /// The shortest stand whose stem it climbs: the crown top over the support face,
    /// metres. 1.0.
    pub climb_min_stand_m: f64,
    /// Horizontal metres covered per metre of descent in a glide. 2.5.
    pub glide_ratio: f64,
    /// **The glide's speed, × its ground cruise** (GAIT, Wrysk 2026-10-06: it "can glide
    /// like 2x their base speed"): a glide covers this × the lineage's cruise
    /// (`cruise_body_lengths_per_s` × the adult length) a second across. 2.0.
    pub glide_speed_fraction: f64,
    /// What a second gliding costs, × basal upkeep (the whole, upkeep included). 1.2.
    pub glide_cost_factor: f64,
    /// A glide starts only from at least this height over the landing surface, metres.
    /// 1.0.
    pub glide_min_height_m: f64,
    /// **The roost reflex's reach** (F3): at its rest hour, or in a downpour, a grounded
    /// glider driven by its own heuristic walks to the nearest stand it can climb within
    /// this horizontal distance, metres, climbs it and perches in the crown (under it in
    /// a downpour). Twice the browser cone's 2 m range: 4.0.
    #[serde(default = "roost_search_m")]
    pub roost_search_m: f64,
    /// **The fruit search's reach** (roster mechanics fix 4): a hungry grounded glider
    /// driven by its own heuristic, with no fruit at its mouth, walks to the nearest
    /// fruiting stand within this horizontal distance, metres, and climbs it. A
    /// **heuristic world query, not a sense**: the roost reflex's kind of lookup over the
    /// plant layer, read by the body's reflex only; it enters no observation and no brain
    /// input. 8.0.
    #[serde(default = "fruit_search_m")]
    pub fruit_search_m: f64,
    /// **The preferred roost's reach** (backlog §12, Wrysk 2026-10-08: "seedporters should
    /// prefer to roost in vault trees at night, or up top mesas they can reach by climbing
    /// latticevine"): at its rest hour a glider driven by its own heuristic goes to a
    /// vaulttree crown, else to a ledge top it reaches up a latticevine wall, no farther
    /// than this straight-line distance, metres, before it falls back on the nearest
    /// climbable stand (`roost_search_m`). A heuristic world query, not a sense. 0 turns
    /// the preference off. 12.0 (three roost searches).
    #[serde(default = "roost_budget_m")]
    pub roost_budget_m: f64,
    /// **The roost trip's reserve guard** (den rest's `Pack::home_reserve_share`): a
    /// preferred roost is taken only when the trip — walking there at cruise, then the climb
    /// at the climbing pace and cost — costs at most this share of the reserve above the
    /// emergency line (`wake_hunger`). Fraction. 0.5, as den rest.
    #[serde(default = "roost_reserve_share")]
    pub roost_reserve_share: f64,
    /// **A ledge worth roosting on** stands at least this high over the foot of the
    /// latticevine wall it is climbed by, metres. It must also exceed the mature
    /// Veilhound's strike height. 1.0 (the glide's minimum start).
    #[serde(default = "roost_ledge_min_m")]
    pub roost_ledge_min_m: f64,
}

fn roost_search_m() -> f64 {
    4.0
}

fn roost_budget_m() -> f64 {
    Glider::SEEDPORTER.roost_budget_m
}

fn roost_reserve_share() -> f64 {
    Glider::SEEDPORTER.roost_reserve_share
}

fn roost_ledge_min_m() -> f64 {
    Glider::SEEDPORTER.roost_ledge_min_m
}

fn fruit_search_m() -> f64 {
    8.0
}

impl Glider {
    /// The seedporter's table (predation-and-flight §3.5, §3.9; Brief F3; placeholders):
    /// climbs stands from 1 m, glides from 1 m at 2.5 : 1 and 2 × its cruise for 1.2 × basal.
    pub const SEEDPORTER: Glider = Glider {
        climb_min_stand_m: 1.0,
        glide_ratio: 2.5,
        glide_speed_fraction: 2.0,
        glide_cost_factor: 1.2,
        glide_min_height_m: 1.0,
        roost_search_m: 4.0,
        fruit_search_m: 8.0,
        roost_budget_m: 12.0,
        roost_reserve_share: 0.5,
        roost_ledge_min_m: 1.0,
    };
}

impl GutSeeds {
    /// The seedporter's gut (F3; placeholders): a tenth of a fruit's organic matter is
    /// seed, voided a quarter hour after the meal (small frugivores hold a meal 10–60
    /// min).
    pub const SEEDPORTER: GutSeeds = GutSeeds {
        seeds_per_fruit_organic: 0.1,
        gut_transit_s: 900.0,
    };
}

/// A frugivore's gut seeds ([`FounderPhysiology::gut_seeds`], F3).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GutSeeds {
    /// Seed organic matter per unit of fruit organic matter eaten: the share of a fruit
    /// meal that is the source species' seeds, carried in the gut (counted out of the
    /// fruit, not invented).
    pub seeds_per_fruit_organic: f64,
    /// Seconds from a meal until its seeds are voided at the body's current face.
    pub gut_transit_s: f64,
}

/// **An amphibious body's table** ([`FounderPhysiology::amphibious`], AMPHIBIOUS-SNAIL,
/// Wrysk 2026-10-06): out of water it runs a **drying clock**, 0 hydrated to 1 full, and
/// it may step onto ground wet enough. "In water" is the water-bound rule's own zone:
/// standing water within [`FounderPhysiology::water_margin_m`] of its face. There the clock
/// falls at `rehydrate_per_s`; out of it the clock rises at `1 / T(m)` per second, `T`
/// running linearly from `dry_ground_s` on bone-dry ground (pore fill `m` = 0) to
/// `wet_ground_s` on saturated ground (`m` = 1), times `day_drying_factor` while the
/// sun is up and `rain_drying_factor` while it rains. Past 1 it is **desiccated** (it cannot
/// feed); at `lethal_fraction` it dies ([`crate::Departure::Desiccation`]).
///
/// The defaults are a 19 cm apple-snail-sized body (Pomacea maculata reaches 15 cm and
/// crawls out at night to lay; Lymnaea and land snails tolerate the loss of a third to a
/// half of their water). Real active snails on dry substrate reach that loss in about
/// 8 h, and on saturated ground in about 3 days; they refill in about an hour of water.
/// The world's day is 2 h ([`DAY_S`]), so each real time is taken at the world's day:
/// 8 h → 40 min, 72 h → 6 h, 1 h → 5 min. Plausible, untuned.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Amphibious {
    /// The switch: `false` keeps the water-bound rule (no land, no clock), the arm the
    /// check compares against. Not a number, so it has no registry row.
    pub on_land: bool,
    /// Seconds the clock takes to fill on bone-dry ground (pore fill 0), at night, dry
    /// weather.
    pub dry_ground_s: f64,
    /// Seconds the clock takes to fill on saturated ground (pore fill 1, or a film of
    /// water on the face).
    pub wet_ground_s: f64,
    /// Clock fraction recovered per second in water.
    pub rehydrate_per_s: f64,
    /// The least ground moisture (the face voxel's pore fill, 0..1; a face under any
    /// standing water reads 1) a step onto a face out of water needs.
    pub step_moisture_min: f64,
    /// Its heuristic heads for water once the clock passes this, at night or in rain.
    pub return_fraction: f64,
    /// ... and this by day in dry weather: the activity bias that keeps it near water in
    /// the sun.
    pub day_return_fraction: f64,
    /// The clock runs this many times faster while the sun is up (daylight at or above
    /// the day line).
    pub day_drying_factor: f64,
    /// ... and this many times as fast while rain falls (0: no drying in rain).
    pub rain_drying_factor: f64,
    /// It dies of desiccation when the clock reaches this (between 1 and this it is
    /// desiccated: no feeding, heading for water).
    pub lethal_fraction: f64,
    /// How far its return reflex looks for water, metres (a world query, not a sense).
    pub water_search_m: f64,
    /// A looming shadow or a threat's tremor makes it **retract** for this long, seconds:
    /// it stops and closes, eating nothing.
    pub retract_hold_s: f64,
    /// While retracted a hit on it is reduced by this, on top of its armour: slower to
    /// eat, not immune.
    pub retract_armour: f64,
    /// **The roam drive** (SNAIL-FIX): at night, in rain and for `roam_after_rain_s` after
    /// it, an awake body that is not thirsty and not feeding where it stands walks out
    /// onto wet ground ([`step_moisture_min`](Amphibious::step_moisture_min)) within this
    /// many metres, toward the land face with the most film in reach ([`crate::Food::Film`],
    /// the plant layer's land-wet film), or, in water with none in reach, to the nearest
    /// wet land face. A heuristic world query, as the water reflex is. 0: no roam.
    #[serde(default = "default_roam_search_m")]
    pub roam_search_m: f64,
    /// ... and the roam hours last this long after rain stops, seconds.
    #[serde(default = "default_roam_after_rain_s")]
    pub roam_after_rain_s: f64,
    /// **Eggs on land** (SNAIL-LAND, Wrysk 2026-10-06; [`EggSite::Shore`]): the egg face is
    /// out of standing water and within this horizontal distance, metres, of a column whose
    /// highest support face at or below the egg face holds standing water. Apple snails
    /// (*Pomacea*) lay above the waterline beside it, out of reach of aquatic egg predators.
    #[serde(default = "default_egg_water_max_m")]
    pub egg_water_max_m: f64,
    /// ... at most this far above that water's surface, metres (how far it climbs).
    #[serde(default = "default_egg_climb_max_m")]
    pub egg_climb_max_m: f64,
    /// ... on ground of this pore fill or more (its [`ground_moisture`]).
    #[serde(default = "default_egg_moisture_min")]
    pub egg_moisture_min: f64,
    /// **The egg's risks**: a shore egg under standing water this long, seconds, without a
    /// break drowns (to the litter of its face). *Pomacea* eggs survive a day or two under
    /// water and fail after four or five of a two-week incubation.
    #[serde(default = "default_egg_drown_s")]
    pub egg_drown_s: f64,
    /// ... and one on ground drier than this pore fill ...
    #[serde(default = "default_egg_dry_moisture")]
    pub egg_dry_moisture: f64,
    /// ... for this long, seconds, without a break dries out (to the litter of its face).
    #[serde(default = "default_egg_dry_s")]
    pub egg_dry_s: f64,
    /// **The trip's reach** (SNAIL-LAND-2): how far, metres, a ready layer looks for a shore
    /// egg face it can afford and whose nest is not crowded (a world query, as the water
    /// reflex's). Before SNAIL-LAND-2 the trip looked only as far as `roam_search_m`, and a
    /// layer in mid-pond held its full reserve and its eggs.
    #[serde(default = "default_egg_search_m")]
    pub egg_search_m: f64,
    /// **The giving-up density** (SNAIL-LAND-2, Charnov's marginal value rule): a grazer
    /// that wants food and stands on a face holding less film than this share of one site's
    /// underwater film capacity (`film_capacity_per_m2 · voxel_m²`) walks to a richer face
    /// within `graze_search_m` (in water at any hour; out of water only in its roam hours).
    /// Read on the edible film, above the grazing refuge (TASTE), as the snail's taste and
    /// the graze target's per-snail worth (`graze_worth`) are: a face grazed to its refuge
    /// tastes of nothing and is left in any case; this leaves a patch poorer than richer
    /// ones in reach before it is bare. 0: off (it stays on any film it tastes).
    #[serde(default = "default_graze_give_up")]
    pub graze_give_up: f64,
    /// ... a face worth the walk holds at least this share of a site's capacity per snail
    /// standing on it (itself counted) ...
    #[serde(default = "default_graze_worth")]
    pub graze_worth: f64,
    /// ... within this horizontal distance, metres.
    #[serde(default = "default_graze_search_m")]
    pub graze_search_m: f64,
}

fn default_egg_search_m() -> f64 {
    Amphibious::RIPPLE_SNAIL.egg_search_m
}

fn default_graze_give_up() -> f64 {
    Amphibious::RIPPLE_SNAIL.graze_give_up
}

fn default_graze_worth() -> f64 {
    Amphibious::RIPPLE_SNAIL.graze_worth
}

fn default_graze_search_m() -> f64 {
    Amphibious::RIPPLE_SNAIL.graze_search_m
}

fn default_egg_water_max_m() -> f64 {
    Amphibious::RIPPLE_SNAIL.egg_water_max_m
}

fn default_egg_climb_max_m() -> f64 {
    Amphibious::RIPPLE_SNAIL.egg_climb_max_m
}

fn default_egg_moisture_min() -> f64 {
    Amphibious::RIPPLE_SNAIL.egg_moisture_min
}

fn default_egg_drown_s() -> f64 {
    Amphibious::RIPPLE_SNAIL.egg_drown_s
}

fn default_egg_dry_moisture() -> f64 {
    Amphibious::RIPPLE_SNAIL.egg_dry_moisture
}

fn default_egg_dry_s() -> f64 {
    Amphibious::RIPPLE_SNAIL.egg_dry_s
}

fn default_roam_search_m() -> f64 {
    Amphibious::RIPPLE_SNAIL.roam_search_m
}

fn default_roam_after_rain_s() -> f64 {
    Amphibious::RIPPLE_SNAIL.roam_after_rain_s
}

impl Amphibious {
    /// The ripple-snail's table (see the type's note for the real-snail derivation).
    pub const RIPPLE_SNAIL: Amphibious = Amphibious {
        on_land: true,
        // 8 h real at the 12x world day.
        dry_ground_s: 2400.0,
        // 72 h real at the 12x world day.
        wet_ground_s: 21600.0,
        // A full refill in ~1 h real: 5 min of world time.
        rehydrate_per_s: 1.0 / 300.0,
        // Below about field capacity a big snail's foot dries on the substrate: it keeps to
        // damp ground (a third of pore capacity), and to water films.
        step_moisture_min: 0.33,
        // Real snails turn back well before damage: at half the clock.
        return_fraction: 0.5,
        // By day in the open it barely leaves the water.
        day_return_fraction: 0.1,
        // Sun and low daytime humidity roughly triple evaporation from a mantle.
        day_drying_factor: 3.0,
        // No net loss in rain: snails roam in it.
        rain_drying_factor: 0.0,
        // A quarter of the clock past full is the harm window before death.
        lethal_fraction: 1.25,
        // Its own short world: a few metres of a pond's margin.
        water_search_m: 4.0,
        // A snail stays in its shell some seconds after a shadow passes.
        retract_hold_s: 8.0,
        // A closed shell and operculum halve a hit.
        retract_armour: 0.5,
        // A night's crawl of a big snail is some metres; it looks over a stride of its
        // pond's bank at a time.
        roam_search_m: 1.5,
        // Snails stay out a couple of real hours after rain: 10 min of world time.
        roam_after_rain_s: 600.0,
        // Apple snails lay a few centimetres to about half a metre above the waterline on
        // stems, banks and walls beside the water (Hayes et al. 2015; Cowie 2002): half a
        // metre from the water, up to half a metre above it.
        egg_water_max_m: 0.5,
        egg_climb_max_m: 0.5,
        // Damp ground: over the step minimum, under drained soil's field capacity (0.65).
        egg_moisture_min: 0.5,
        // The clutch incubates 300 s (the shredder's); *Pomacea* eggs (two weeks) take a
        // day or two under water and fail after four or five: about 0.3 of incubation.
        egg_drown_s: 90.0,
        // The calcareous clutch resists drying (it is laid in air) but fails below about
        // 50 % air humidity over days: ground drier than the step minimum for 0.4 of the
        // incubation.
        egg_dry_moisture: 0.33,
        egg_dry_s: 120.0,
        // Its own short world, as the water reflex's: a few metres of a pond's margin.
        egg_search_m: 4.0,
        // A grazed face at under a third of the bed's capacity is left for one holding at
        // least half of it, within a metre (about five body lengths), both read on the film
        // above the grazing refuge (a tenth). Plausible, untuned: below a fresh bed.
        graze_give_up: 0.3,
        graze_worth: 0.5,
        graze_search_m: 1.0,
    };

    /// The clock's rise per second out of water on ground of moisture `m` (0..1), before
    /// the day and rain factors.
    pub fn drying_per_s(&self, m: f64) -> f64 {
        let m = if m.is_finite() { m.clamp(0.0, 1.0) } else { 0.0 };
        let t = self.dry_ground_s + (self.wet_ground_s - self.dry_ground_s) * m;
        if t > 0.0 { 1.0 / t } else { f64::INFINITY }
    }
}

/// A flyer's table ([`FounderPhysiology::flight`], F1; the bellwing's numbers are
/// predation-and-flight §3.3, §3.9, placeholders).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Flight {
    /// Wingspan, metres: the collision sphere's diameter (air is any cell not solid and
    /// not water; the body is a sphere of radius `span_m / 2`). Bellwing 0.375.
    pub span_m: f64,
    /// **The ceiling**: the most the body's base may stand above the support face below
    /// its column, metres. Bellwing 2.5.
    pub band_max_m: f64,
    /// Cost of a second airborne at cruise, × basal upkeep. Bellwing 4.
    pub flight_factor: f64,
    /// Cost of a second hovering (forward 0), × basal upkeep. Bellwing 5.
    pub hover_factor: f64,
    /// Lift work paid going up, organic units per metre climbed per unit of body. Descent
    /// is free.
    pub climb_work_per_m: f64,
    /// A takeoff costs this many seconds of flight. Bellwing 1.
    pub takeoff_s: f64,
    /// The speed cap inside a crown's foliage layers, × the open-air cap. 0.5.
    pub foliage_speed_factor: f64,
    /// `R₀` of the shower cost: flight costs × `(1 + rain rate / this)`, m/s of rain.
    pub shower_rain_reference_m_per_s: f64,
    /// The vertical speed at full `lift` (either way), × the cruise speed. Bellwing 0.5:
    /// half a metre a second, a full band (2.5 m) in five seconds.
    #[serde(default = "half")]
    pub lift_speed_fraction: f64,
    /// **An egg needs room to fly from**: a flyer lays only on a face with at least this
    /// clear over its hatchling's body (the hatchling's disc, `body_min`'s height),
    /// metres. Bellwing 1.0, derived from the hatchling's flight clearance: the hover-sip
    /// teacher treats cover closer than `bellwing_roost_ceiling` (0.4) × `band_max_m`
    /// (2.5) = 1.0 m over its body as low (it neither climbs toward it nor over a wall
    /// under it), so a hatchling with 1.0 m clear sees open sky and climbs out.
    #[serde(default = "egg_headroom")]
    pub egg_headroom_m: f64,
    /// A flyer lays only where the air over the face reaches the **open air** (the
    /// largest connected region of air under its band, a point body); that region is
    /// rebuilt from the terrain at most this often, seconds. A placeholder: 600.
    #[serde(default = "egg_air_refresh")]
    pub egg_air_refresh_s: f64,
    /// **Rain grounds it** (backlog §12, Wrysk 2026-10-08: "rain should ground bellwings"):
    /// at or above this rain rate, m/s, a flyer aloft comes down (up under reachable cover
    /// overhead, else down onto what is below), and a perched one does not take off or sip,
    /// until the rate falls under this less `rain_ground_hysteresis_m_per_s`. A downpour
    /// grounds it whatever the rate (§3.8). 20 mm/h: a shower grounds it, a drizzle does
    /// not.
    #[serde(default = "rain_ground")]
    pub rain_ground_m_per_s: f64,
    /// How far under `rain_ground_m_per_s` the rain must fall before a rain-grounded flyer
    /// flies again, m/s. 5 mm/h.
    #[serde(default = "rain_ground_hysteresis")]
    pub rain_ground_hysteresis_m_per_s: f64,
}

fn rain_ground() -> f64 {
    Flight::BELLWING.rain_ground_m_per_s
}

fn rain_ground_hysteresis() -> f64 {
    Flight::BELLWING.rain_ground_hysteresis_m_per_s
}

fn half() -> f64 {
    0.5
}

fn egg_headroom() -> f64 {
    Flight::BELLWING.egg_headroom_m
}

fn egg_air_refresh() -> f64 {
    Flight::BELLWING.egg_air_refresh_s
}

impl Flight {
    /// The bellwing's table (predation-and-flight §3.3, §3.9; placeholders).
    pub const BELLWING: Flight = Flight {
        span_m: 0.375,
        band_max_m: 2.5,
        flight_factor: 4.0,
        hover_factor: 5.0,
        // Lift work: raising a body 1 m at a muscle efficiency of 0.25 costs g / 0.25 ≈
        // 39 J/kg, against an insect's hover at about 300 W/kg. Scaled to this model's
        // hover (5 × the basal 0.001 body/s), a metre climbed costs 0.005 × 39 / 300 ≈
        // 6.5e-4 of the body: at the full lift (0.5 m/s) about 6 % on top of a hover.
        climb_work_per_m: 6.5e-4,
        takeoff_s: 1.0,
        foliage_speed_factor: 0.5,
        // Heavy rain (10 mm/h) doubles the cost of a second aloft.
        shower_rain_reference_m_per_s: 10.0 / 1000.0 / 3600.0,
        lift_speed_fraction: 0.5,
        egg_headroom_m: 1.0,
        egg_air_refresh_s: 600.0,
        rain_ground_m_per_s: 20.0 / 1000.0 / 3600.0,
        rain_ground_hysteresis_m_per_s: 5.0 / 1000.0 / 3600.0,
    };
}

/// Where an egg-laying lineage lays ([`FounderPhysiology::egg_site`], roster R2b).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EggSite {
    /// On a face whose litter is over the rule's `litter_min`: the litter it lives in.
    Litter,
    /// On a face holding glassfilm (the ripple-snail).
    Film,
    /// **At the fungus** (the capgnawer, 2026-10-05; proposal §3.1E): on a face within
    /// [`crate::Reproduction::egg_fungus_radius_m`] of a fungus holding cap tissue, so the
    /// young hatch at their food and recruitment follows the fungus.
    Fungus,
    /// **On land by the water** (SNAIL-LAND, the ripple-snail; Wrysk 2026-10-06: "snails
    /// should lay eggs on land"): on a face out of standing water near the water's edge,
    /// on moist ground, by its lineage's [`Amphibious`] shore knobs. A lineage with no
    /// amphibious table finds no such face.
    Shore,
}

/// How a fresh world is founded with a lineage ([`FounderPhysiology::founding`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Founding {
    /// `founding_count` living bodies, placed by [`FounderPhysiology::founding_site`].
    Bodies,
    /// `founding_count` **clutches** buried on faces where its prey stand (§2.9, §2.13:
    /// the lanternjaw's egg bank); no adults. A count of 0 is the no-egg-bank control.
    Clutches,
    /// `founding_count` **packs** (roster R3, the chorister): each `pack_size` adults
    /// ([`Pack`]) with one pack id, the first the breeder, around a covered den.
    Pack,
}

/// Where a founding body goes ([`FounderPhysiology::founding_site`]). Both read the
/// lineage's own walkable components and the food its mouth reaches from each
/// (`cubarium_voxel_sim::habitat::lineage_food`), and take founders into a component only
/// while its stock covers the founding hours of upkeep for every one of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoundingSite {
    /// The littershredder's: dry open soil away from the stands, a starter tile of leaf
    /// litter laid under each body first, in a component where a living producer of its
    /// renewable food is rooted.
    LitterBed,
    /// The browser's: a face from which its mouth reaches a stocked layer of a stand its
    /// diet accepts, in a component holding at least two such stands.
    AtFood,
    /// A flyer's (F1, the bellwing): perched on a dry support face within its fan's range
    /// (horizontally) of a nectar source — a bloomcrown past its seedling profile or a
    /// latticevine-covered face. Walkable components do not enter it.
    NearNectar,
}

/// A carnivore's numbers (`design/predation-and-flight-2026-09-28.md` §2.12, every one
/// a placeholder for `design/backlog.md`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hunter {
    /// Its **prey list**, by [`Founder::index`]: the lineages whose bodies it strikes.
    pub prey: [bool; Founder::COUNT],
    /// A hit adds `strike_power × (S_pred / S_prey) × (1 − armour)` to the prey's wound
    /// (§2.4).
    pub strike_power: f64,
    /// The **size window**: a prey body at most this × its own (§2.1). The window follows
    /// its own body, so a juvenile takes juvenile prey.
    pub prey_max_ratio: f64,
    /// The strike capsule's reach ahead of the jaw, × its current length.
    pub strike_reach_length_fraction: f64,
    /// The jaw band `[0, jaw_band_height_fraction × height]` over its standing surface.
    pub jaw_band_height_fraction: f64,
    /// Rearing and jumping reach above the standing surface, metres. The strike uses
    /// the larger of this and the ordinary jaw band. 0 disables the extra reach.
    #[serde(default)]
    pub vertical_reach_m: f64,
    /// The jaw's point: the nose, this × its current length ahead of its centre (half).
    pub jaw_forward_length_fraction: f64,
    /// The three phases, seconds (§2.3): a crouch, the lunge, and the recovery in which it
    /// cannot strike and moves at half speed.
    pub windup_s: f64,
    pub lunge_s: f64,
    pub recovery_s: f64,
    /// The strike's cost, in **seconds of its awake upkeep**, paid at the lunge, hit or
    /// miss.
    pub strike_cost_s: f64,
    /// A lit lure's cost, × its awake upkeep (§2.12: "doubles its burn").
    pub lure_power: f64,
}

impl Hunter {
    /// Vertical strike extent over the hunter's support, metres.
    pub fn strike_height_m(&self, geometry: &Body) -> f64 {
        self.vertical_reach_m.max(self.jaw_band_height_fraction * geometry.height_m)
    }

    /// The lanternjaw's table (§2.12): power 0.7, window 1.5, reach 0.6 L over the jaw
    /// band 0.6 H, windup 0.15 s, lunge 0.25 s, recovery 3 s, a strike at 100 s of awake
    /// upkeep, and the lure at its awake upkeep.
    pub const LANTERNJAW: Hunter = Hunter {
        // Shredder, grazer and (roster R1) the capgnawer; the size window decides. The
        // loftstrider (4) is on no list until the chorister.
        // The ripple-snail (5) and the bellwing (6) since the detritus web D2 (Wrysk
        // 2026-10-05: "lanternjaws can eat snails"; a bellwing "if they catch one on the
        // ground or in reach"): the bellwing only at rest ([`crate::is_prey_of`]).
        // The seedporter (7, F3) is on it, but only while grounded ([`crate::is_prey_of`]).
        // The chorister (8, roster R3) is on no lanternjaw's list.
        prey: [true, true, false, true, false, true, true, true, false],
        strike_power: 0.7,
        prey_max_ratio: 1.5,
        strike_reach_length_fraction: 0.6,
        jaw_band_height_fraction: 0.6,
        vertical_reach_m: 0.0,
        jaw_forward_length_fraction: 0.5,
        windup_s: 0.15,
        lunge_s: 0.25,
        recovery_s: 3.0,
        strike_cost_s: 100.0,
        lure_power: 1.0,
    };

    /// Whether `prey`'s lineage is on its list.
    pub fn hunts(&self, prey: Founder) -> bool {
        self.prey[prey.index()]
    }

    /// Whole ticks for a phase, never below one.
    pub fn ticks(seconds: f64) -> u32 {
        u32::try_from(cubarium_voxel::ticks_at_least_one(seconds)).unwrap_or(u32::MAX)
    }
}

/// **The minimal gut** (`design/metabolism-meals-2026-10-02.md`, repair 2): a stomach of
/// raw food sized to the current body, digested into the reserve's room only, filled by a
/// body-scaled bite, and an appetite that starts a meal near an empty stomach — sooner the
/// emptier the reserve.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gut {
    /// `k`: the stomach's capacity, raw organic matter per unit of **current** body
    /// (`K = k · body`), so a juvenile has a juvenile's stomach.
    pub stomach_k: f64,
    /// `b`: raw food a bite takes, per unit of current body per second at full effort;
    /// one bite per controller interval. Default `2k / τ` of the lineage's own food, so a
    /// meal fills in about `0.69 τ` with digestion running (repair 2 item 4). Independent
    /// of upkeep.
    pub bite_per_body_s: f64,
    /// `τ` per [`crate::Food`] class, seconds: a class's pool empties at `S / τ` while the
    /// reserve has room for what it yields.
    pub digestion_s: [f64; crate::Food::COUNT],
    /// `s0`, the appetite floor: a fed body (reserve hunger at most 0.5) wants food at
    /// `S ≤ s0 · K`; `s_start` rises linearly to 1 as hunger goes from 0.5 to 1.
    pub s0: f64,
    /// The full event: the eating latch goes off at `S ≥ full_tolerance · K`.
    pub full_tolerance: f64,
    /// **The refill gap** (repair 3 item 3), a fraction of `K`: `s_start` is capped at
    /// `full_tolerance − refill_gap`, so the appetite band never overlaps the full event
    /// and a starving body just declared full waits for its stomach to empty this much
    /// before it wants food again. 0.1.
    #[serde(default = "default_refill_gap")]
    pub refill_gap: f64,
    /// **The gut's mass exponent** `e` (package D1b): throughput scales with body as small
    /// animals' does. The stomach stays `k · body`, but a body of `body` digests in
    /// `τ · (body / body_max)^(1 − e)` and bites `b · (body / body_max)^(e − 1)` per unit
    /// body ([`Gut::throughput_factor`]), so its intake at a full stomach goes as
    /// `body^e` and its intake per unit body falls with size (Kleiber's `e` = 0.75; gut
    /// capacity isometric, retention time growing as `body^0.25`). The adult's numbers are
    /// unchanged; a hatchling digests faster per unit of its body. 1: every size the
    /// adult's per-mass gut.
    #[serde(default = "default_mass_exponent")]
    pub mass_exponent: f64,
}

fn default_refill_gap() -> f64 {
    REFILL_GAP
}

fn default_mass_exponent() -> f64 {
    1.0
}

impl Gut {
    /// The design's gut for a lineage of stomach `k` eating food it digests in `τ`
    /// seconds: `b = 2k / τ`, `τ` for every class, `s0` 0.15, full at 0.98.
    pub fn of(stomach_k: f64, tau_s: f64) -> Gut {
        Gut {
            stomach_k,
            bite_per_body_s: 2.0 * stomach_k / tau_s,
            digestion_s: [tau_s; crate::Food::COUNT],
            s0: APPETITE_FLOOR,
            full_tolerance: FULL_TOLERANCE,
            refill_gap: REFILL_GAP,
            mass_exponent: 1.0,
        }
    }

    /// **The throughput factor** of a body of `body` against its lineage's `body_max`:
    /// `(body / body_max)^(e − 1)` ([`Gut::mass_exponent`]), what its digestion rate and
    /// bite per unit body are multiplied by. 1 at `body_max`, for `e` = 1, and for a
    /// degenerate body.
    pub fn throughput_factor(&self, body: f64, body_max: f64) -> f64 {
        if self.mass_exponent == 1.0 || !(body > 0.0) || !(body_max > 0.0) {
            return 1.0;
        }
        (body / body_max).powf(self.mass_exponent - 1.0)
    }

    /// `K`, raw organic matter, for a body of `body`.
    pub fn capacity(&self, body: f64) -> f64 {
        self.stomach_k * body
    }

    /// `s_start` at reserve hunger `hunger`: `s0 + (1 − s0) · max(0, (h − 0.5) / 0.5)`,
    /// capped at `full_tolerance − refill_gap` (repair 3 item 3).
    pub fn s_start(&self, hunger: f64) -> f64 {
        let over = ((hunger - 0.5) / 0.5).clamp(0.0, 1.0);
        (self.s0 + (1.0 - self.s0) * over).min(self.full_tolerance - self.refill_gap)
    }
}

/// The counter-strike seam (§2.7): a struck animal's answer, wounding its attacker. Unused.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Counter {
    pub reach: f64,
    pub damage: f64,
    pub cost: f64,
}

fn one() -> f64 {
    1.0
}

impl FounderPhysiology {
    /// **The hunter table behaviour reads**: [`FounderPhysiology::hunter`], except that an
    /// [`opportunist`](FounderPhysiology::opportunist)'s table is there only while its
    /// [`catch_bellwings`](FounderPhysiology::catch_bellwings) switch is on. Every
    /// behavioural and classification read (who hunts whom, who flees whom, the senses'
    /// prey/threat classes, the strike) goes through this; only the config's own
    /// serialisation and reports read the raw field. With the switch off an opportunist
    /// is the plain forager it was before it learned the catch.
    pub fn active_hunter(&self) -> Option<Hunter> {
        self.hunter.filter(|_| !self.opportunist || self.catch_bellwings)
    }

    /// **The diet at its mouth**: [`FounderPhysiology::diet`], less carrion for an
    /// [`opportunist`](FounderPhysiology::opportunist) whose catch is switched off
    /// (carrion is the catch's food, on the same rule as [`FounderPhysiology::active_hunter`]).
    pub fn eating_diet(&self) -> crate::Diet {
        if self.opportunist && !self.catch_bellwings {
            crate::Diet::of(&self.diet.foods().filter(|&f| f != crate::Food::Carrion).collect::<Vec<_>>())
        } else {
            self.diet
        }
    }

    /// Make this lineage `k` times heavier in org-units: the seven absolute quantities of
    /// its core ([`crate::SpeciesConfig::scale_mass`]) times `k`, and the recorded
    /// [`FounderPhysiology::mass_scale`] with them. Per-unit rates, clocks and metres stay.
    pub fn scale_mass(&mut self, k: f64) {
        self.core.scale_mass(k);
        self.mass_scale *= k;
    }

    /// The frozen phase-one table, one entry per founder.
    ///
    /// The blind littershredder's digestive configuration is set **deliberately, once,
    /// from the real `Taken` composition** of the litter it eats (`voxel-senses-phase1-tests.md`
    /// §2): arena litter is deposited at a 0.02 mineral fraction and
    /// [`take_litter`](cubarium_voxel_flora::Flora::take_litter) returns mineral pro rata
    /// at that fraction with the litter's own retained energy density of 2.0. A tissue
    /// mineral content of 0.02 therefore never binds the mineral budget — the feeder
    /// converts its whole yield share of what it takes — and nothing was
    /// added to the litter and nothing was converted from the carrion or wood pools to
    /// make that so. The bite rate keeps the frondgrazer's placeholder ratio of 0.04
    /// body per second at the founder's smaller body.
    pub fn frozen(founder: Founder) -> FounderPhysiology {
        let mut p = FounderPhysiology::placeholder(founder);
        p.scale_mass(crate::SHIPPED_MASS_SCALE);
        p
    }

    /// The placeholder table before the shipped mass scale: every absolute quantity at
    /// the value its comment derives, `mass_scale` 1. [`FounderPhysiology::frozen`] is
    /// this x [`crate::SHIPPED_MASS_SCALE`].
    pub fn placeholder(founder: Founder) -> FounderPhysiology {
        let mut p = FounderPhysiology::placeholder_table(founder);
        p.burr_coat = burr_coat_of(founder);
        p.senses = senses_of(founder);
        p.vigilance = crate::vigil::vigilance_of(founder);
        p.sprint = crate::gait::sprint_of(founder);
        p.hunker = crate::vigil::hunker_of(founder);
        p.herding = crate::herding::herding_of(founder);
        p.drop = crate::step::jump::drop_of(founder);
        p.rhythm = crate::rhythm::rhythm_of(founder);
        p.cadence = crate::rhythm::cadence_of(founder);
        p
    }

    fn placeholder_table(founder: Founder) -> FounderPhysiology {
        match founder {
            Founder::Blind => FounderPhysiology {
                core: SpeciesConfig {
                    maintenance_per_s: DAILY_SHREDDER / DAY_S,
                    // The browser's K scaled by body (0.0125 / 0.05): a fortieth of the
                    // litter cue's reference stock `emit_reference`, so a litter film gives crumbs
                    // and a real pile nearly a whole bite.
                    bite_half_stock: 0.00125,
                    n_tissue: 0.02,
                    body_max: 0.0125,
                    // From body_min to birth_body (0.009375) in ≥ 2,757 s, so that with the
                    // 900 s hold a hatchling's first clutch is ≥ 3,657 s after it hatched.
                    growth_max_per_s: 3.4e-6,
                    body_min: 0.003125,
                    birth_body: 0.0125,
                    birth_cost: 0.00625,
                    reserve_cap: 0.5,
                    // Unused by the founder path: feeding is local and there is no
                    // target search. Recorded at the frondgrazer's placeholders so a
                    // founder body is a complete animal under the same validators.
                    drown_depth_m: 0.2,
                    energy_density: 2.0,
                    // The littershredder lays a clutch on the litter it lives in.
                    reproduction: Reproduction::EGGS_PLACEHOLDER,
                    lifespan_median_s: LIFE_SMALL_DAYS * DAY_S,
                },
                // Full cruise at its basal rate (the motor's tie to upkeep).
                motor_respiration_per_s: DAILY_SHREDDER / DAY_S,
                cruise_body_lengths_per_s: 1.0,
                yaw_cap_rad_per_s: 2.0,
                // Two thirds of its length both ways; everything taller is a wall.
                step_up_m: 0.25,
                step_down_m: 0.25,
                climbs_walls: true,
                wall_climb_cost_factor: WALL_CLIMB_COST_FACTOR,
                climb_speed_fraction: 0.5,
                wade_height_fraction: 0.25,
                drown_height_fraction: 1.0,
                drown_after_s: DROWN_AFTER_S,
                // The ladder's littershredder (package L,
                // `design/handoffs/voxel-ladder-growth-2026-09-23.md` §3): 0.375 m long,
                // a third of that in section — 3 x 1 x 1 cells of 0.125 m.
                adult_length_m: 0.375,
                adult_width_m: 0.125,
                adult_height_m: 0.125,
                eye_height_fraction: EYE_HEIGHT_FRACTION,
                mouth_ceiling_fraction: MOUTH_CEILING_FRACTION,
                mouth_floor_fraction: 0.0,
                bite_spill_fraction: 0.0,
                contact_height_fraction: CONTACT_HEIGHT_FRACTION,
                mouth_reach_length_fraction: MOUTH_REACH_LENGTH_FRACTION,
                // Litter keeps the yield it has; cap tissue takes the foliage yield;
                // carrion takes the litter yield. `Foliage` is the browser's class and
                // the shredder never takes one, so it carries the same number its
                // fungal foliage does.
                yield_by_food: [
                    LITTER_YIELD,
                    FOLIAGE_YIELD,
                    LITTER_YIELD,
                    FOLIAGE_YIELD,
                    // R2b: film is not its food; the foliage yield so it is not a zero.
                    FOLIAGE_YIELD,
                    // F1: nor nectar.
                    FOLIAGE_YIELD,
                    // F3: nor fruit.
                    FOLIAGE_YIELD,
                ],
                organ_structure_fraction: 0.05,
                active: crate::Activity::Leaning,
                active_window: crate::ActiveWindow { before_rise_h: 0.0, after_set_h: 0.0 },
                lure_daylight: false,
                flower_ambush_m: 1.0,
                catch_bellwings: false,
                opportunist: false,
                catch_ceiling_m: 0.4,
                catch_gain_min: 0.0,
                catch_foray_m: 0.0,
                catch_foray_min: 2.0,
                catch_foray_s: 8.0,
                sleep_upkeep: SLEEP_UPKEEP_ECTOTHERM,
                wake_hunger: WAKE_HUNGER,
                emergency_clear_hunger: EMERGENCY_CLEAR_HUNGER,
                // Litter: k 0.02, τ 120 s.
                gut: Gut::of(0.02, 120.0),
                hunter: None,
                armour: 0.0,
                agility: 0.3,
                counter: None,
                mass_scale: 1.0,
                taste_reference: None,
                // Litter and carrion (Wrysk, 2026-10-01: the shredder goes back to litter;
                // the capgnawer owns the glowcap caps, roster R1). Its cap-tissue yield
                // stays as the capgnawer's template.
                diet: crate::Diet::of(&[crate::Food::Litter, crate::Food::Carrion]),
                founding: Founding::Bodies,
                founding_site: FoundingSite::LitterBed,
                founding_count: 8,
                founding_per_world: false,
                aquatic: false,
                water_margin_m: 0.0,
                amphibious: None,
                egg_site: EggSite::Litter,
                flight: None,
                glider: None,
                gut_seeds: None,
                burr_coat: 0.0,
                senses: SenseTable::default(),
                vigilance: crate::Vigilance::default(),
                sprint: crate::Sprint::default(),
                hunker: crate::Hunker::default(),
                herding: crate::Herding::default(),
                drop: crate::DropRule::default(),
                rhythm: crate::Rhythm::default(),
                cadence: crate::Cadence::default(),
                pack: None,
            },
            Founder::Browser => FounderPhysiology {
                // The browser founder *is* the frondgrazer: the landed round-4 numbers
                // stand, including their deliberately mineral-hungry tissue against
                // foliage. Only the cost settings are new.
                core: SpeciesConfig {
                    maintenance_per_s: DAILY_FRONDGRAZER / DAY_S,
                    lifespan_median_s: LIFE_GRAZER_DAYS * DAY_S,
                    ..SpeciesConfig::frondgrazer()
                },
                // The frondgrazer gives live birth out of a gestation escrow, which is
                // in `SpeciesConfig::frondgrazer`'s own table.
                motor_respiration_per_s: DAILY_FRONDGRAZER / DAY_S,
                cruise_body_lengths_per_s: 1.0,
                yaw_cap_rad_per_s: 2.0,
                // Its own height both ways; a longer drop waits for the retrain (see the
                // field).
                step_up_m: 0.375,
                step_down_m: 0.375,
                climbs_walls: false,
                wall_climb_cost_factor: WALL_CLIMB_COST_FACTOR,
                climb_speed_fraction: 0.5,
                wade_height_fraction: 0.5,
                drown_height_fraction: 1.0,
                drown_after_s: DROWN_AFTER_S,
                // The ladder's frondgrazer (package L, same brief §3): 0.75 x 0.375 x
                // 0.375 m, 6 x 3 x 3 cells of 0.125 m.
                adult_length_m: 0.75,
                adult_width_m: 0.375,
                adult_height_m: 0.375,
                eye_height_fraction: EYE_HEIGHT_FRACTION,
                mouth_ceiling_fraction: MOUTH_CEILING_FRACTION,
                mouth_floor_fraction: 0.0,
                bite_spill_fraction: 0.0,
                contact_height_fraction: CONTACT_HEIGHT_FRACTION,
                mouth_reach_length_fraction: MOUTH_REACH_LENGTH_FRACTION,
                // The browser eats foliage and nothing else; the other three are the
                // same number so that a class it cannot reach is never a silent zero.
                yield_by_food: [FOLIAGE_YIELD; crate::Food::COUNT],
                organ_structure_fraction: 0.10,
                active: crate::Activity::Leaning,
                active_window: crate::ActiveWindow { before_rise_h: 0.0, after_set_h: 0.0 },
                lure_daylight: false,
                flower_ambush_m: 1.0,
                catch_bellwings: false,
                opportunist: false,
                catch_ceiling_m: 0.4,
                catch_gain_min: 0.0,
                catch_foray_m: 0.0,
                catch_foray_min: 2.0,
                catch_foray_s: 8.0,
                sleep_upkeep: SLEEP_UPKEEP,
                wake_hunger: WAKE_HUNGER,
                emergency_clear_hunger: EMERGENCY_CLEAR_HUNGER,
                // Foliage: k 0.06, τ 120 s.
                gut: Gut::of(0.06, 120.0),
                hunter: None,
                // The saddle shield (§2.7).
                armour: 0.25,
                agility: 0.5,
                counter: None,
                mass_scale: 1.0,
                taste_reference: None,
                diet: crate::Diet::only(crate::Food::Foliage),
                founding: Founding::Bodies,
                founding_site: FoundingSite::AtFood,
                founding_count: 8,
                founding_per_world: false,
                aquatic: false,
                water_margin_m: 0.0,
                amphibious: None,
                egg_site: EggSite::Litter,
                flight: None,
                glider: None,
                gut_seeds: None,
                burr_coat: 0.0,
                senses: SenseTable::default(),
                vigilance: crate::Vigilance::default(),
                sprint: crate::Sprint::default(),
                hunker: crate::Hunker::default(),
                herding: crate::Herding::default(),
                drop: crate::DropRule::default(),
                rhythm: crate::Rhythm::default(),
                cadence: crate::Cadence::default(),
                pack: None,
            },
            // The lanternjaw (`design/predation-and-flight-2026-09-28.md` §2.12): a slim
            // scorpid ambusher, 1.0 × 0.25 × 0.45 m, its daily need two thirds of the grazer's (the minimal gut).
            // Numbers the note pins are its own; the others are stated where they sit.
            Founder::Lanternjaw => FounderPhysiology {
                core: SpeciesConfig {
                    maintenance_per_s: DAILY_LANTERNJAW / DAY_S,
                    // The browser's K scaled by body (0.005 × 0.035 / 0.05).
                    bite_half_stock: 0.0035,
                    // Set from carrion's own composition, as the shredder's is from
                    // litter's: a corpse carries its dead body's tissue mineral (0.02 to
                    // 0.05), so 0.02 never binds the budget.
                    n_tissue: 0.02,
                    body_max: 0.035,
                    // Hatchling to laying size (0.0034 → 0.021) in about two hours on a
                    // full reserve: a sixth of its 12 h lifespan.
                    growth_max_per_s: 2.5e-6,
                    // A hatchling is 0.46 of the adult's length (§2.13): 0.46³ × 0.035.
                    body_min: 0.0034,
                    // Laying size: 0.6 of body_max, the browser's ratio.
                    birth_body: 0.021,
                    // Unread by an egg-layer; one egg, so the validator's floor holds.
                    birth_cost: 0.005,
                    // The detritus web D2: small frequent meals, not one grazer (P1's 1.0):
                    // 0.6 of the body holds about eight and a half days of its need, and
                    // still pays its whole clutch above the floor
                    // ([`Reproduction::DORMANT_EGGS_PLACEHOLDER`]).
                    reserve_cap: 0.6,
                    drown_depth_m: 0.2,
                    energy_density: 2.0,
                    reproduction: Reproduction::DORMANT_EGGS_PLACEHOLDER,
                    // Repair 2 item 5: the predators' twelve world days.
                    lifespan_median_s: LIFE_PREDATOR_DAYS * DAY_S,
                },
                // Full cruise at its basal rate, both other lineages' rule.
                motor_respiration_per_s: DAILY_LANTERNJAW / DAY_S,
                cruise_body_lengths_per_s: 0.6,
                yaw_cap_rad_per_s: 2.0,
                // Its own height both ways, the browser's rule.
                step_up_m: 0.45,
                step_down_m: 0.45,
                climbs_walls: false,
                wall_climb_cost_factor: WALL_CLIMB_COST_FACTOR,
                climb_speed_fraction: 0.5,
                wade_height_fraction: 0.5,
                drown_height_fraction: 1.0,
                drown_after_s: DROWN_AFTER_S,
                adult_length_m: 1.0,
                adult_width_m: 0.25,
                adult_height_m: 0.45,
                eye_height_fraction: EYE_HEIGHT_FRACTION,
                mouth_ceiling_fraction: MOUTH_CEILING_FRACTION,
                mouth_floor_fraction: 0.0,
                bite_spill_fraction: 0.0,
                contact_height_fraction: CONTACT_HEIGHT_FRACTION,
                mouth_reach_length_fraction: MOUTH_REACH_LENGTH_FRACTION,
                // Carrion is the one food its mouth takes; every class carries the same
                // number so none is a silent zero.
                yield_by_food: [LITTER_YIELD; crate::Food::COUNT],
                organ_structure_fraction: 0.10,
                // S3 (D1, Wrysk 2026-10-06): active wherever it is dark — at night, and by
                // day in caves and deep shade with the daylight-mimic switch off.
                // MIMIC turns that switch on for flower-side daytime ambushes.
                active: crate::Activity::DarkActive,
                active_window: crate::ActiveWindow { before_rise_h: 0.0, after_set_h: 0.0 },
                lure_daylight: true,
                flower_ambush_m: 1.0,
                catch_bellwings: false,
                opportunist: false,
                catch_ceiling_m: 0.4,
                catch_gain_min: 0.0,
                catch_foray_m: 0.0,
                catch_foray_min: 2.0,
                catch_foray_s: 8.0,
                sleep_upkeep: SLEEP_UPKEEP_ECTOTHERM,
                wake_hunger: WAKE_HUNGER,
                emergency_clear_hunger: EMERGENCY_CLEAR_HUNGER,
                // Meat: k 0.15, τ 300 s (an ambusher's rare large meal).
                gut: Gut::of(0.15, 300.0),
                hunter: Some(Hunter::LANTERNJAW),
                armour: 0.0,
                agility: 0.0,
                counter: None,
                mass_scale: 1.0,
                taste_reference: None,
                diet: crate::Diet::only(crate::Food::Carrion),
                // §2.9: a fresh world is founded with a buried egg bank, not adults.
                founding: Founding::Clutches,
                founding_site: FoundingSite::AtFood,
                founding_count: 3,
                founding_per_world: false,
                aquatic: false,
                water_margin_m: 0.0,
                amphibious: None,
                egg_site: EggSite::Litter,
                flight: None,
                glider: None,
                gut_seeds: None,
                burr_coat: 0.0,
                senses: SenseTable::default(),
                vigilance: crate::Vigilance::default(),
                sprint: crate::Sprint::default(),
                hunker: crate::Hunker::default(),
                herding: crate::Herding::default(),
                drop: crate::DropRule::default(),
                rhythm: crate::Rhythm::default(),
                cadence: crate::Cadence::default(),
                pack: None,
            },
            // The capgnawer (roster R1, `design/handoffs/voxel-roster-2026-10-01.md`): the
            // glowcap's grazer, built from the littershredder by **body volume**, the way
            // P1 built the lanternjaw from the grazer (predation §2.12).
            //
            // Volume ratio: its adult body 0.25 × 0.21 × 0.19 m = 0.009975 m³ over the
            // shredder's 0.375 × 0.125 × 0.125 m = 0.005859 m³, **r = 1.7024**
            // ([`CAPGNAWER_VOLUME_RATIO`]). Every stock (body_max, body_min, birth_body,
            // the clutch: egg_organic and surplus_floor) and the absolute growth rate
            // (growth_max_per_s) is the shredder's × r
            // ([`SpeciesConfig::scale_mass`], which scales exactly those). Upkeep and the
            // motor are already per unit of body per second, so they stay and the absolute
            // upkeep scales with the body: specific rates equal the shredder's. The reserve
            // cap is a fraction of body, so the reserve ceiling `reserve_cap · body_max`
            // also scales by r (multiplying the fraction too would scale it by r²). Clocks
            // (hold, interval, incubation, lifespan) are the shredder's: its reproduction
            // mode and timings.
            Founder::Capgnawer => {
                let shredder = FounderPhysiology::placeholder(Founder::Blind);
                let mut core = shredder.core;
                core.scale_mass(CAPGNAWER_VOLUME_RATIO);
                core.maintenance_per_s = DAILY_CAPGNAWER / DAY_S;
                // Its eggs go at the fungus (the capgnawer package, 2026-10-05): within
                // half a metre of a glowcap holding caps, so the young hatch at their food.
                core.reproduction.egg_fungus_radius_m = CAPGNAWER_EGG_FUNGUS_RADIUS_M;
                FounderPhysiology {
                    core,
                    // A gripping six-legged walker on bark: it keeps the shredder's legs
                    // (0.25 m both ways, climbs walls, wades a quarter of its height).
                    adult_length_m: 0.25,
                    adult_width_m: 0.21,
                    adult_height_m: 0.19,
                    // Cap tissue at the shredder's cap yield; litter (conditioned, a weak
                    // fallback) at half the shredder's litter yield. Carrion and foliage
                    // are not its food and carry the cap yield so none is a silent zero.
                    yield_by_food: [
                        0.5 * LITTER_YIELD,
                        FOLIAGE_YIELD,
                        FOLIAGE_YIELD,
                        FOLIAGE_YIELD,
                        FOLIAGE_YIELD,
                        FOLIAGE_YIELD,
                        FOLIAGE_YIELD,
                    ],
                    diet: crate::Diet::of(&[crate::Food::CapTissue, crate::Food::Litter]),
                    // Caps: k 0.02, τ 120 s; the shredder's daily need (its own row).
                    gut: Gut::of(0.02, 120.0),
                    // Near glowcap stands: a face from which its mouth reaches a stocked cap
                    // (the diet-driven food filter).
                    founding: Founding::Bodies,
                    founding_site: FoundingSite::AtFood,
                    founding_count: 0,
                    egg_site: EggSite::Fungus,
                    motor_respiration_per_s: DAILY_CAPGNAWER / DAY_S,
                    ..shredder
                }
            }
            // The loftstrider (roster R1; `design/organism-large-animals-2026-09-23.md` §3):
            // the tall browser, built from the frondgrazer by **body volume**.
            //
            // Volume ratio, from its **hull** (1.5 × 0.55 × 0.45 m = 0.37125 m³, the body
            // that carries the tissue; the 2.2 × 2.2 × 3.4 m leg and frond envelope is its
            // geometry, not its mass) over the grazer's 0.75 × 0.375 × 0.375 m =
            // 0.10547 m³: **r = 3.52** ([`LOFTSTRIDER_VOLUME_RATIO`]). Stocks and absolute
            // rates are the grazer's × r ([`SpeciesConfig::scale_mass`]); upkeep and the
            // motor are per unit of body, so specific rates equal the grazer's; the reserve
            // ceiling scales with the body. Long-lived and slow: one young per birth (a
            // gestation), the gestation 3× the grazer's and the median lifespan 3× the
            // grazer's. The young following its parent is not modelled (owed).
            Founder::Loftstrider => {
                let grazer = FounderPhysiology::placeholder(Founder::Browser);
                let mut core = grazer.core;
                core.scale_mass(LOFTSTRIDER_VOLUME_RATIO);
                // A slow, single-calf browser: half a reference world day carrying,
                // a quarter-day surplus hold, then three days before another conception.
                core.reproduction.gestation_s = 3600.0;
                core.reproduction.surplus_hold_s = 1800.0;
                core.reproduction.birth_interval_s = 21600.0;
                core.lifespan_median_s = LIFE_LOFTSTRIDER_DAYS * DAY_S;
                core.maintenance_per_s = DAILY_LOFTSTRIDER / DAY_S;
                FounderPhysiology {
                    core,
                    // Long legs: a step of a third of its 3.4 m height both ways (Wrysk
                    // 2026-10-02: "on the order of like 1/3 of its height based on its leg
                    // anatomy, so 1+ m"); 1.0 m on a 0.25 m voxel. No wall climbing.
                    step_up_m: 1.15,
                    step_down_m: 1.15,
                    climbs_walls: false,
                    // It wades to a quarter of its height (0.85 m adult, under the hull)
                    // where the grazer drowns at 0.375 m, and is under at the eye ring.
                    wade_height_fraction: 0.25,
                    drown_height_fraction: 0.44,
                    adult_length_m: 2.2,
                    adult_width_m: 2.2,
                    adult_height_m: 3.4,
                    // The eye ring at 1.5 m of 3.4.
                    eye_height_fraction: 0.44,
                    // The mouth band 0.8–3.5 m of 3.4: fronds from shrub height to the
                    // crowns, never turf, rosettes or ground cover.
                    mouth_floor_fraction: 0.235,
                    mouth_ceiling_fraction: 1.03,
                    // The sloppy feeder: a quarter of each bite falls to the litter at
                    // its face.
                    bite_spill_fraction: 0.25,
                    // Its feet meet the terrain, not its hull: contact at the knee, 0.5 m.
                    contact_height_fraction: 0.15,
                    // On no prey list until the chorister (R3); no defence counter.
                    armour: 0.0,
                    agility: 0.0,
                    counter: None,
                    diet: crate::Diet::only(crate::Food::Foliage),
                    // Near the tallest stands: a face from which its banded mouth reaches
                    // a stocked layer.
                    founding: Founding::Bodies,
                    founding_site: FoundingSite::AtFood,
                    founding_count: 0,
                    founding_per_world: true,
                    motor_respiration_per_s: DAILY_LOFTSTRIDER / DAY_S,
                    // Foliage: k 0.05, τ 120 s (one or two long browses a day).
                    gut: Gut::of(0.05, 120.0),
                    ..grazer
                }
            }
            // The ripple-snail (roster R2b, `design/handoffs/voxel-roster-2026-10-01.md`):
            // a slow, water-bound grazer of glassfilm, built from the littershredder by
            // **body volume** exactly as the capgnawer is. Its adult body 0.19 × 0.19 ×
            // 0.063 m = 0.002274 m³ over the shredder's 0.005859 m³: **r = 0.388**
            // ([`RIPPLE_SNAIL_VOLUME_RATIO`]). Stocks and absolute rates scale by r
            // ([`SpeciesConfig::scale_mass`]); upkeep and the motor are per unit of body,
            // so specific rates equal the shredder's. The shredder's eggs, timings,
            // lifespan and legs.
            // The bellwing (F1, predation-and-flight §3.3, §3.9): a bell body 0.125 m long
            // and wide, 0.0625 m tall, on a 0.375 m span; `body_max` 0.001 (x1), pinned by
            // the note. Built from the littershredder **by mass** (k = 0.001 / 0.0125 =
            // 0.08, [`SpeciesConfig::scale_mass`]) as the capgnawer is by volume: a small
            // invertebrate-sized animal, so the shredder's per-unit upkeep, its eggs
            // (on the litter of the face below its perch), timings and lifespan. Upkeep
            // and the motor are per unit of body, so specific rates equal the shredder's.
            Founder::Bellwing => {
                let shredder = FounderPhysiology::placeholder(Founder::Blind);
                let mut core = shredder.core;
                core.scale_mass(BELLWING_MASS_RATIO);
                core.maintenance_per_s = DAILY_BELLWING / DAY_S;
                // Nectar's half-stock: a tenth of a bloomcrown's nectar cap (0.02), so a
                // full flower gives about nine tenths of a sip and a nearly drained one
                // crumbs. Not a mass (the food's stock does not scale with the eater).
                core.bite_half_stock = 0.002;
                // BELLWING (Wrysk 2026-10-06: "bellwing should have fast reproduction"): an
                // r-strategist moth, an income breeder whose eggs are paid from nectar
                // (Boggs 1997; Jervis, Boggs & Ferns 2005), on the egg-layer machinery:
                // - small young: a hatchling of a tenth of the adult (`body_min`; a moth's
                //   egg is ~0.1-2 % of the female, Garcia-Barros 2000 — no caterpillar here,
                //   so the smallest flyer), its egg the hatchling plus a sliver of reserve;
                // - early breeding at half the adult body (`birth_body`; moths lay within
                //   the first days of an adult life of one to two weeks, Fitt 1989);
                // - a clutch from surplus, up to six eggs, waiting at most 300 s to fill
                //   it: fecundity grows with the female's stores (Honek 1993), so a flush
                //   of flowers is laid out as large clutches; a gravid reserve of up to 0.8
                //   of the body (the egg load, Jervis et al. 2005) holds the full clutch;
                // - the surplus judged over 300 s (was 900 s).
                core.body_min = 0.1 * core.body_max;
                core.birth_body = 0.5 * core.body_max;
                core.reserve_cap = 0.8;
                // The egg: the hatchling and a sixth of it again as its reserve.
                core.reproduction.egg_organic = core.body_min * (1.0 + 1.0 / 6.0);
                core.reproduction.clutch_max = 6;
                core.reproduction.clutch_hold_max_s = 300.0;
                core.reproduction.surplus_hold_s = 300.0;
                FounderPhysiology {
                    core,
                    // Cruise 8 BL/s, 1 m/s at 0.125 m (§3.3).
                    cruise_body_lengths_per_s: 8.0,
                    adult_length_m: 0.125,
                    adult_width_m: 0.125,
                    adult_height_m: 0.0625,
                    // It never walks: no step, no wall climb. Wades and drowns at its
                    // height (unread aloft: a flyer never enters water).
                    step_up_m: 0.0,
                    step_down_m: 0.0,
                    climbs_walls: false,
                    // Nectar at [`NECTAR_YIELD`]; every other class carries the foliage
                    // yield so none is a silent zero.
                    yield_by_food: {
                        let mut y = [FOLIAGE_YIELD; crate::Food::COUNT];
                        y[crate::Food::Nectar.index()] = NECTAR_YIELD;
                        y
                    },
                    organ_structure_fraction: 0.10,
                    active: crate::Activity::Leaning,
                    // LURE (Wrysk 2026-10-06: "dusk/dawn is fine for them to fly as well as
                    // day"): a twilight flyer, awake from an hour before sunrise to an hour
                    // after sunset, asleep only in deep night. Many moths and other insects
                    // fly in a dusk peak and a smaller dawn one (Lewis & Taylor 1965); the
                    // crepuscular hawkmoths feed at flowers at dusk (Herrera 1992; Kelber,
                    // Balkenius & Warrant 2002), and the twilight bee Megalopta forages in
                    // about the hour after sunset and before sunrise (Warrant et al. 2004).
                    active_window: crate::ActiveWindow { before_rise_h: 1.0, after_set_h: 1.0 },
                    agility: 0.0,
                    diet: crate::Diet::only(crate::Food::Nectar),
                    founding: Founding::Bodies,
                    founding_site: FoundingSite::NearNectar,
                    founding_count: 0,
                    flight: Some(Flight::BELLWING),
                    // Taste reads nectar against a bloomcrown's full flower (its
                    // `nectar_cap`, 0.02): a full flower tastes 1 before saturation.
                    taste_reference: Some(0.02),
                    // An endotherm-like sleeper, not the shredder's 0.3 (1a).
                    sleep_upkeep: SLEEP_UPKEEP,
                    motor_respiration_per_s: DAILY_BELLWING / DAY_S,
                    // Nectar: k 0.03, τ 30 s (many small sips).
                    gut: Gut::of(0.03, 30.0),
                    ..shredder
                }
            }
            // The seedporter (F3, Brief F3; predation-and-flight §3.5, §3.9): the frondgrazer's
            // table by **body volume**, as R1 built the loftstrider: 0.5 × 0.25 × 0.2 m over
            // the grazer's 0.75 × 0.375 × 0.375 m, r ≈ 0.237 ([`SEEDPORTER_VOLUME_RATIO`]),
            // so `body_max` 0.05 × r ≈ 0.012 (§3.9). Upkeep and the motor are per unit of
            // body, so its specific rates are the grazer's. It walks at 1 BL/s, climbs stems
            // and vine faces at half that for the shredder's climbing cost, glides
            // ([`Glider::SEEDPORTER`]) and carries seeds ([`GutSeeds::SEEDPORTER`]); diet
            // [fruit], diurnal, founded at its food.
            Founder::Seedporter => {
                let grazer = FounderPhysiology::placeholder(Founder::Browser);
                let shredder = FounderPhysiology::placeholder(Founder::Blind);
                let mut core = grazer.core;
                core.scale_mass(SEEDPORTER_VOLUME_RATIO);
                core.maintenance_per_s = DAILY_SEEDPORTER / DAY_S;
                core.lifespan_median_s = LIFE_GRAZER_DAYS * DAY_S;
                FounderPhysiology {
                    core,
                    adult_length_m: 0.5,
                    adult_width_m: 0.25,
                    adult_height_m: 0.2,
                    // GAIT (Wrysk 2026-10-06: "a bit below veilhound when on the ground"):
                    // 0.85 × the Veilhound's 1.4 m/s cruise, 1.19 m/s on its 0.5 m, ≈ 2.4
                    // BL/s. It climbs at `climb_speed_fraction` (0.5) of this, slower than it
                    // walks, and glides at its glider's `glide_speed_fraction` × this.
                    cruise_body_lengths_per_s: 0.85 * 1.4 / 0.5,
                    // Half its length up or down in a stride; it does not climb terrain
                    // walls (stems and vine faces are its climbing, [`Glider`]).
                    step_up_m: 0.25,
                    step_down_m: 0.25,
                    climbs_walls: false,
                    wall_climb_cost_factor: shredder.wall_climb_cost_factor,
                    climb_speed_fraction: 0.5,
                    // Fruit at [`FRUIT_YIELD`]; every other class carries the foliage
                    // yield so none is a silent zero.
                    yield_by_food: {
                        let mut y = [FOLIAGE_YIELD; crate::Food::COUNT];
                        y[crate::Food::Fruit.index()] = FRUIT_YIELD;
                        y
                    },
                    active: crate::Activity::Leaning,
                    diet: crate::Diet::of(&[crate::Food::Fruit, crate::Food::Carrion]),
                    catch_bellwings: true,
                    opportunist: true,
                    catch_foray_m: 5.0,
                    hunter: Some(Hunter {
                        prey: [false, false, false, false, false, false, true, false, false],
                        strike_power: 4.0,
                        prey_max_ratio: 4.0,
                        strike_reach_length_fraction: 0.8,
                        jaw_band_height_fraction: 2.0,
                        vertical_reach_m: 0.0,
                        jaw_forward_length_fraction: 0.5,
                        windup_s: 0.1,
                        lunge_s: 0.15,
                        recovery_s: 1.0,
                        strike_cost_s: 2.0,
                        lure_power: 0.0,
                    }),
                    founding: Founding::Bodies,
                    founding_site: FoundingSite::AtFood,
                    founding_count: 0,
                    // Taste reads fruit against a lanternberry's full crop (its
                    // `fruit_cap`, 0.02): a full crop tastes 1 before saturation.
                    taste_reference: Some(0.02),
                    glider: Some(Glider::SEEDPORTER),
                    gut_seeds: Some(GutSeeds::SEEDPORTER),
                    motor_respiration_per_s: DAILY_SEEDPORTER / DAY_S,
                    // Fruit: k 0.05, τ 60 s (a few meals a day).
                    gut: Gut::of(0.05, 60.0),
                    ..grazer
                }
            }
            // The chorister (roster R3, Brief R3; predation-and-flight §2.10, §2.14;
            // organism-large-animals §5): a diurnal pack hunter on the lanternjaw's hunter
            // table and strike machinery, built from the lanternjaw by **body volume**, as
            // R1 built the capgnawer and the loftstrider.
            //
            // Volume ratio: 1.4 × 0.35 × 0.6 m over the lanternjaw's 1.0 × 0.25 × 0.45 m,
            // **r ≈ 2.61** ([`CHORISTER_VOLUME_RATIO`]; the 0.8 m tail is geometry only).
            // Stocks and absolute rates are the lanternjaw's × r
            // ([`SpeciesConfig::scale_mass`]); the reserve cap is a fraction of body, so it
            // stays. Specific upkeep is **twice** the lanternjaw's (§2.14: "a higher
            // upkeep"), and the motor keeps the lanternjaw's rule (full cruise at its basal
            // rate). Live birth on the grazers' placeholder clocks, the newborn's package
            // twice its `body_min` (the frondgrazer's ratio); the litter, its interval and
            // the breeder are the pack's ([`Pack`]).
            Founder::Chorister => {
                let lj = FounderPhysiology::placeholder(Founder::Lanternjaw);
                let mut core = lj.core;
                core.reproduction = Reproduction::LIVE_BIRTH_PLACEHOLDER;
                // The grazers' surplus floor at the lanternjaw's body (the frondgrazer's
                // 0.005 at its 0.05), then × r with the other stocks.
                core.reproduction.surplus_floor *= lj.core.body_max / 0.05;
                core.birth_cost = 2.0 * core.body_min;
                // The P1 lanternjaw's reserve cap, which the chorister was built on: the
                // detritus web's smaller lanternjaw cap is the lanternjaw's alone.
                core.reserve_cap = 1.0;
                core.scale_mass(CHORISTER_VOLUME_RATIO);
                core.maintenance_per_s = DAILY_CHORISTER / DAY_S;
                // LIFE-HISTORY: one litter a pack a year, conceived just after the dry season,
                // a wolf's gestation at a raised upkeep ([`CHORISTER_INDEPENDENCE_S`]).
                core.reproduction.gestation_s = CHORISTER_GESTATION_S;
                core.reproduction.litters_per_year = 1;
                core.reproduction.breeding_phase = 0.2;
                core.reproduction.breeding_window = 0.15;
                core.reproduction.gestation_upkeep_factor = CHORISTER_PREGNANCY_UPKEEP;
                FounderPhysiology {
                    core,
                    // GAIT: its own cruise, 1 BL/s (1.4 m/s; the standing pace rule), no
                    // longer the lanternjaw's 0.6 (0.84 m/s; Wrysk 2026-10-06: "they
                    // probably should move faster"). Full cruise still costs its basal rate
                    // (the motor tie): a second at cruise costs what it did, a metre less;
                    // its rush at 2.5 × cruise costs 2.5 × a second's walking.
                    cruise_body_lengths_per_s: 1.0,
                    motor_respiration_per_s: core.maintenance_per_s,
                    // Its own height both ways, the lanternjaw's rule.
                    step_up_m: 0.6,
                    step_down_m: 0.6,
                    adult_length_m: 1.4,
                    adult_width_m: 0.35,
                    adult_height_m: 0.6,
                    // RHYTHMS-CADENCE: any hour, long rests ([`crate::rhythm::rhythm_of`]).
                    active: crate::Activity::Leaning,
                    lure_daylight: false,
                    // The lanternjaw's strike table, no lure, its own list (§2.14): the
                    // frondgrazer, the loftstrider (only a pack's window takes an adult),
                    // the lanternjaw, a grounded seedporter, and (the detritus web D2) a
                    // bellwing at rest in reach.
                    hunter: Some(Hunter {
                        prey: [false, true, true, false, true, false, true, true, false],
                        lure_power: 0.0,
                        vertical_reach_m: 2.25,
                        ..Hunter::LANTERNJAW
                    }),
                    diet: crate::Diet::only(crate::Food::Carrion),
                    founding: Founding::Pack,
                    founding_site: FoundingSite::AtFood,
                    founding_count: 0,
                    founding_per_world: true,
                    pack: Some(Pack::CHORISTER),
                    // An endotherm-like sleeper, not the lanternjaw's 0.3 (1a).
                    sleep_upkeep: SLEEP_UPKEEP,
                    // Meat: k 0.20, τ 300 s (Wrysk: one big meal every one or two days).
                    gut: Gut::of(0.20, 300.0),
                    ..lj
                }
            }
            Founder::RippleSnail => {
                let shredder = FounderPhysiology::placeholder(Founder::Blind);
                let grazer = FounderPhysiology::placeholder(Founder::Browser);
                let mut core = shredder.core;
                core.scale_mass(RIPPLE_SNAIL_VOLUME_RATIO);
                core.maintenance_per_s = DAILY_RIPPLE_SNAIL / DAY_S;
                // Film is fresh tissue: the browser's foliage yield. The other foods are
                // not its diet and carry the same number so none is a silent zero.
                let film_yield = grazer.yield_by_food[crate::Food::Foliage.index()];
                FounderPhysiology {
                    core,
                    // Its own rule (night and after rain, `Amphibious`): not a leaning.
                    active: crate::Activity::Nocturnal,
                    adult_length_m: 0.19,
                    adult_width_m: 0.19,
                    adult_height_m: 0.063,
                    // Slow: 0.3 body lengths a second.
                    cruise_body_lengths_per_s: 0.3,
                    yield_by_food: [film_yield; crate::Food::COUNT],
                    diet: crate::Diet::only(crate::Food::Film),
                    // Water-bound: it never drowns, wades anything, and keeps within a
                    // quarter metre of standing water.
                    aquatic: true,
                    water_margin_m: 0.25,
                    // Amphibious (Wrysk, 2026-10-06): it crawls out onto wet ground at
                    // night and after rain, on a drying clock, and goes back to water.
                    amphibious: Some(Amphibious::RIPPLE_SNAIL),
                    // Its clutch goes on land by the water (SNAIL-LAND).
                    egg_site: EggSite::Shore,
                    // Taste reads film against the film's own capacity per site, derived on
                    // the world's cell (`taste_reference_on`): so no fixed stock here.
                    taste_reference: None,
                    // On a film site (the diet-driven food filter).
                    founding: Founding::Bodies,
                    founding_site: FoundingSite::AtFood,
                    founding_count: 0,
                    motor_respiration_per_s: DAILY_RIPPLE_SNAIL / DAY_S,
                    // Film: k 0.012, τ 120 s.
                    gut: Gut::of(0.012, 120.0),
                    ..shredder
                }
            }
        }
    }

    /// Taste's reference stock for a world whose cue config has `emit_reference`: the
    /// lineage's own [`FounderPhysiology::taste_reference`] when it has one, else the
    /// manifest rule `emit_reference × mass_scale`.
    pub fn taste_reference_stock(&self, emit_reference: f64) -> f64 {
        self.taste_reference.unwrap_or(emit_reference * self.mass_scale)
    }

    /// Taste's reference stock **on a world** of cell edge `voxel_m` under the plant layer's
    /// `flora` config (roster mechanics, the coordinator's item 6): a lineage that eats
    /// only film and sets no `taste_reference` tastes against one site's film capacity,
    /// `film_capacity_per_m2 × voxel_m²` (`manifest::TASTE_REFERENCE_FILM_RULE`), so a full
    /// site tastes 1 on any grid; every other lineage is [`Self::taste_reference_stock`].
    pub fn taste_reference_on(&self, emit_reference: f64, voxel_m: f64, flora: &cubarium_voxel_flora::FloraConfig) -> f64 {
        match self.taste_reference {
            None if self.diet == crate::Diet::only(crate::Food::Film) => flora.film_capacity_per_m2 * voxel_m * voxel_m,
            _ => self.taste_reference_stock(emit_reference),
        }
    }

    /// Total structure a `core` non-sensory budget buys, with sensor allocation `f`:
    /// `core / (1 − f)`. A caller sizing a founder body from a core budget introduces
    /// this much; the sensor share is inside it, counted once.
    pub fn total_structure_for_core(core: f64, fraction: f64) -> f64 {
        debug_assert!(fraction.is_finite() && fraction >= 0.0 && fraction < 1.0);
        core / (1.0 - fraction)
    }

    /// The step up and step down in whole voxels of `voxel_m`, each rounded **down**
    /// ([`step_voxels`]): a riser of `k` voxels is a step when `k · voxel_m` is within the
    /// limit. The browser's 0.375 m is three 0.125 m voxels and one 0.25 m voxel, never
    /// two.
    pub fn step_limits(&self, voxel_m: f64) -> StepLimits {
        StepLimits {
            up: step_voxels(self.step_up_m, voxel_m),
            down: step_voxels(self.step_down_m, voxel_m),
        }
    }

    /// The deepest standing water `body` walks into: `wade_height_fraction × height`, or
    /// any depth for an [`FounderPhysiology::aquatic`] body.
    pub fn wade_depth_m(&self, body: &Body) -> f64 {
        // An aquatic body (roster R2b) walks into any depth: no wade refusal.
        if self.aquatic {
            return f64::INFINITY;
        }
        self.wade_height_fraction * body.height_m
    }

    /// Standing water deeper than this over its feet is water `body` is under:
    /// `drown_height_fraction × height`.
    pub fn drown_depth_m(&self, body: &Body) -> f64 {
        self.drown_height_fraction * body.height_m
    }

    /// [`FounderPhysiology::drown_after_s`] in whole ticks, never below one.
    pub fn drown_ticks(&self) -> u64 {
        cubarium_voxel::ticks_at_least_one(self.drown_after_s)
    }

    /// The assimilation yield this lineage gets out of one food class (decisions §3).
    pub fn yield_for(&self, food: crate::Food) -> f64 {
        self.yield_by_food[food.index()]
    }

    /// The sensor/organ share of a total structure stock — a view onto the one body
    /// ledger, never a second stock.
    pub fn sensor_structure(&self, total_body: f64) -> f64 {
        self.organ_structure_fraction * total_body.max(0.0)
    }

    /// The adult's geometry: the authored dimensions, and the anchors as fractions of
    /// them.
    pub fn adult_body(&self) -> Body {
        self.body_scaled(1.0)
    }

    /// The geometry of a body holding `body_organic` of structure: the adult's
    /// dimensions times `(body / body_max)^(1/3)` (decisions §1), so a newborn browser
    /// at `body_min` is 0.46 of the adult's length. Growth changes geometry only; no
    /// rate reads this.
    pub fn body_at(&self, body_organic: f64) -> Body {
        let max = self.core.body_max;
        let fraction = if max.is_finite() && max > 0.0 && body_organic.is_finite() {
            (body_organic / max).clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.body_scaled(fraction.cbrt())
    }

    fn body_scaled(&self, scale: f64) -> Body {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            0.0
        };
        let length_m = self.adult_length_m * scale;
        let width_m = self.adult_width_m * scale;
        let height_m = self.adult_height_m * scale;
        Body {
            length_m,
            width_m,
            height_m,
            eye_m: self.eye_height_fraction * height_m,
            mouth_floor_m: self.mouth_floor_fraction * height_m,
            mouth_ceiling_m: self.mouth_ceiling_fraction * height_m,
            contact_m: self.contact_height_fraction * height_m,
            mouth_reach_m: self.mouth_reach_length_fraction * length_m,
        }
    }
}

/// One tick's resolved motion, for the interval feedback. Attempted and delivered are
/// recorded separately: a wall-constrained attempt is attempted in full and delivered
/// short.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Motion {
    /// Requested equivalent displacement this tick: `(|v| + r·|yaw rate|) · dt`.
    pub attempted_equivalent: f64,
    /// Delivered equivalent displacement this tick: the distance actually covered plus
    /// the footprint radius times the turn actually made.
    pub delivered_equivalent: f64,
    /// Metres actually covered along the heading (unsigned; forward effort has no
    /// reverse).
    pub delivered_forward: f64,
    /// Radians actually turned, signed.
    pub delivered_turn: f64,
    /// A sub-step was refused (wall, drop, water, the strip's ends) or a clamp at the
    /// `z` ends delivered less than its step asked for.
    pub blocked: bool,
}

/// Resolve one tick of held actions against the continuous pose: turn, then sweep the
/// footprint forward in bounded sub-steps. `pose` is advanced in place; nothing here
/// reads a target, a stock or a route.
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_motion(
    view: &VoxelView<'_>,
    pose: &mut crate::Pose,
    standing_y: &mut u32,
    manifest: &Manifest,
    body: &Body,
    wade_depth_m: f64,
    step_limits: StepLimits,
    wall_friction: f64,
    held: Actions,
) -> Motion {
    resolve_motion_in(
        view,
        pose,
        standing_y,
        manifest,
        body,
        wade_depth_m,
        step_limits,
        wall_friction,
        held,
        None,
    )
}

/// [`resolve_motion`] for a body that may be **water-bound** (roster R2b) or
/// **amphibious** (AMPHIBIOUS-SNAIL): with a [`WaterRule`] set, a sub-step onto another
/// support face is refused, the way a wall refuses one, unless the rule admits that face
/// (standing water within the margin, or for an amphibious body ground wet enough). A
/// body already stranded on a face the rule refuses is not held: it may walk anywhere, so
/// it can find its way back.
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_motion_in(
    view: &VoxelView<'_>,
    pose: &mut crate::Pose,
    standing_y: &mut u32,
    manifest: &Manifest,
    body: &Body,
    wade_depth_m: f64,
    step_limits: StepLimits,
    wall_friction: f64,
    held: Actions,
    water_rule: Option<WaterRule>,
) -> Motion {
    // A stranded body is not held to the margin (it would never move again).
    let water_rule = water_rule.filter(|rule| {
        pose.column(view.config.voxel_m, view.config.depth)
            .is_some_and(|(cx, cz)| {
                let cx = cx.rem_euclid(i64::from(view.config.width));
                rule.admits(view, cx, *standing_y, cz)
            })
    });
    let r = body.footprint_radius();
    let headroom = body.headroom_voxels(view.config.voxel_m);
    let v_req = held.forward * manifest.cruise_m_per_s;
    let yaw_rate = held.turn * manifest.yaw_cap_rad_per_s;
    let attempted = (v_req.abs() + r * yaw_rate.abs()) * DT;

    // Turn first; the sweep then runs along the heading the body now faces.
    let dyaw = yaw_rate * DT;
    pose.heading_rad = (pose.heading_rad + dyaw).rem_euclid(std::f64::consts::TAU);

    // Bounded sub-steps of at most one footprint radius: the overlap window of a
    // one-voxel wall is `voxel + 2r` wide and the sub-step is `r`, so no endpoint can
    // skip past it. A refused step is followed by a bisection that presses the body up
    // to the obstruction — a body stands against a wall at the wall's face, not a
    // sub-step short of it — and the sweep stops there: the rest of the request was
    // attempted and is paid for, but is not delivered.
    let mut moved = 0.0;
    let mut blocked = false;
    let mut left = v_req * DT;
    // The heading is fixed for the sweep: its unit vector is taken once, not per probe.
    let dir = (pose.heading_rad.sin(), pose.heading_rad.cos());
    while left > 1e-12 {
        let step = left.min(r);
        match step_advance(
            view,
            pose,
            standing_y,
            dir,
            step,
            r,
            wade_depth_m,
            headroom,
            step_limits,
            water_rule,
        ) {
            Some(actual) => {
                moved += actual;
                left -= step;
                if actual < step - 1e-9 {
                    // A clamp at the strip's `z` ends: the world edge is a wall too.
                    blocked = true;
                }
            }
            None => {
                blocked = true;
                // Press the body up to the obstruction: bisect the largest still-valid
                // advance from where the sweep stopped, without committing the probes,
                // then take it once. The gap left under a wall is a few hundredths of
                // the footprint radius.
                let (px, pz) = (pose.x, pose.z);
                let y0 = *standing_y;
                let mut lo = 0.0;
                let mut hi = step;
                for _ in 0..24 {
                    let mid = (lo + hi) / 2.0;
                    if mid <= 1e-12 {
                        break;
                    }
                    if advance_candidate(
                        view,
                        px,
                        pz,
                        dir,
                        y0,
                        mid,
                        r,
                        wade_depth_m,
                        headroom,
                        step_limits,
                        water_rule,
                    )
                    .is_some()
                    {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                if lo > 1e-12 {
                    let (nx, nz, actual, ny) = advance_candidate(
                        view,
                        px,
                        pz,
                        dir,
                        y0,
                        lo,
                        r,
                        wade_depth_m,
                        headroom,
                        step_limits,
                        water_rule,
                    )
                    .expect("the bisection's best advance is valid");
                    pose.x = nx;
                    pose.z = nz;
                    *standing_y = ny;
                    moved += actual;
                }
                moved += wall_slide(
                    view,
                    pose,
                    standing_y,
                    dir,
                    (left - lo) * (1.0 - wall_friction.clamp(0.0, 1.0)),
                    r,
                    wade_depth_m,
                    headroom,
                    step_limits,
                    water_rule,
                );
                break;
            }
        }
    }
    Motion {
        attempted_equivalent: attempted,
        delivered_equivalent: moved + r * dyaw.abs(),
        delivered_forward: moved,
        delivered_turn: dyaw,
        blocked,
    }
}

/// **Wall slide** (crag fix 4): after a refused sub-step has pressed the body up to the
/// obstruction, the rest of the request, `budget` metres (already scaled by
/// `1 − wall_friction`), goes along the wall instead of being lost. Voxel walls are
/// axis-aligned, so the heading's two axis components are the candidate tangents: the
/// larger is tried first and the first one that advances at all takes the slide, swept
/// in sub-steps of at most `r` until it is spent or refused. A head-on walk has no
/// tangential component and does not move. The return is the slide's distance projected
/// onto the heading — the forward progress it delivered.
#[allow(clippy::too_many_arguments)]
fn wall_slide(
    view: &VoxelView<'_>,
    pose: &mut crate::Pose,
    standing_y: &mut u32,
    (fx, fz): (f64, f64),
    budget: f64,
    r: f64,
    wade_depth_m: f64,
    headroom: u32,
    step_limits: StepLimits,
    water_rule: Option<WaterRule>,
) -> f64 {
    if budget <= 1e-12 {
        return 0.0;
    }
    let mut axes = [((fx.signum(), 0.0), fx.abs()), ((0.0, fz.signum()), fz.abs())];
    if axes[1].1 > axes[0].1 {
        axes.swap(0, 1);
    }
    for (axis, share) in axes {
        let mut left = budget * share;
        if left <= 1e-12 {
            continue;
        }
        let mut slid = 0.0;
        while left > 1e-12 {
            let step = left.min(r);
            let Some(actual) =
                step_advance(
                view,
                pose,
                standing_y,
                axis,
                step,
                r,
                wade_depth_m,
                headroom,
                step_limits,
                water_rule,
            )
            else {
                break;
            };
            slid += actual;
            left -= step;
            if actual < step - 1e-9 {
                break;
            }
        }
        if slid > 0.0 {
            return slid * share;
        }
    }
    0.0
}

/// One bounded sub-step. `Some(actual)` hands back the distance actually covered — a
/// clamp at the strip's `z` ends delivers the short move it is — and `None` leaves the
/// pose untouched.
#[allow(clippy::too_many_arguments)]
fn step_advance(
    view: &VoxelView<'_>,
    pose: &mut crate::Pose,
    standing_y: &mut u32,
    dir: (f64, f64),
    step: f64,
    r: f64,
    wade_depth_m: f64,
    headroom: u32,
    step_limits: StepLimits,
    water_rule: Option<WaterRule>,
) -> Option<f64> {
    let (nx, nz, actual, ny) = advance_candidate(
        view,
        pose.x,
        pose.z,
        dir,
        *standing_y,
        step,
        r,
        wade_depth_m,
        headroom,
        step_limits,
        water_rule,
    )?;
    pose.x = nx;
    pose.z = nz;
    *standing_y = ny;
    Some(actual)
}

/// Whether the support face `(x, y, z)` has **standing water within `margin_m`** of it
/// (roster R2b, an aquatic body's margin): any support face whose centre is within
/// `margin_m` of this one's, itself included, holding standing water
/// ([`VoxelView::standing_depth_m`]) over it. Distances are centre to centre between
/// support faces, in metres; `x` wraps, `z` and `y` stop at the world's ends.
/// **Where a water-bound or amphibious body may step** ([`resolve_motion_in`]): a face with
/// standing water within `margin_m` ([`water_within`]), or, for an amphibious body out on
/// land (`land_moisture_min` set), a face whose [`ground_moisture`] is at least that.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WaterRule {
    pub margin_m: f64,
    pub land_moisture_min: Option<f64>,
}

impl WaterRule {
    /// The rule of a lineage's physiology: `None` for a body that is not aquatic.
    pub(crate) fn of(phys: &FounderPhysiology) -> Option<WaterRule> {
        phys.aquatic.then(|| WaterRule {
            margin_m: phys.water_margin_m,
            land_moisture_min: phys.amphibious.filter(|a| a.on_land).map(|a| a.step_moisture_min),
        })
    }

    /// Whether the face `(x, y, z)` is one the body may step onto.
    pub(crate) fn admits(&self, view: &VoxelView<'_>, x: i64, y: u32, z: u32) -> bool {
        water_within(view, x, y, z, self.margin_m)
            || self.land_moisture_min.is_some_and(|m| ground_moisture(view, x, y, z) >= m)
    }
}

/// **The ground's moisture at a support face**, 0..1: 1 under any water on the face (a
/// shower's film included), else the face voxel's pore fill (0 for bedrock and rock with
/// no pores, and on a view with no water).
pub(crate) fn ground_moisture(view: &VoxelView<'_>, x: i64, y: u32, z: u32) -> f64 {
    let c = view.config;
    if z >= c.depth || y >= c.height || view.pore.is_empty() {
        return 0.0;
    }
    if view.water_depth_m(x, y, z) > 0.0 {
        return 1.0;
    }
    view.pore_at(x, y, z).clamp(0.0, 1.0)
}

pub(crate) fn water_within(view: &VoxelView<'_>, x: i64, y: u32, z: u32, margin_m: f64) -> bool {
    let c = view.config;
    let v = c.voxel_m;
    if view.standing_depth_m(x, y, z) > 0.0 {
        return true;
    }
    if !(margin_m > 0.0) || !(v > 0.0) {
        return false;
    }
    let k = (margin_m / v + 1e-9).floor() as i64;
    let reach2 = (margin_m / v) * (margin_m / v) + 1e-9;
    let lo = (i64::from(y) - k).max(0) as u32;
    let hi = (i64::from(y) + k).min(i64::from(c.height) - 1) as u32;
    // Standing water can occur only on support faces. Enumerate those with the
    // terrain's column bits instead of visiting every solid and air voxel in the sphere.
    for dz in -k..=k {
        let zz = i64::from(z) + dz;
        if zz < 0 || zz >= i64::from(c.depth) {
            continue;
        }
        for dx in -k..=k {
            let horizontal = dx * dx + dz * dz;
            if horizontal as f64 > reach2 {
                continue;
            }
            let xx = (x + dx).rem_euclid(i64::from(c.width));
            let mut from = lo;
            while let Some(yy) = view.lowest_support_in(xx, zz as u32, from, hi) {
                let dy = i64::from(yy) - i64::from(y);
                if (horizontal + dy * dy) as f64 <= reach2 && (dx, dy, dz) != (0, 0, 0)
                    && view.standing_depth_m(xx, yy, zz as u32) > 0.0
                {
                    return true;
                }
                if yy == hi {
                    break;
                }
                from = yy + 1;
            }
        }
    }
    false
}

/// The candidate one sub-step would move the body to, as a pure query: the new
/// position, the distance actually covered and the **standing layer it would leave the
/// body on**, or `None` when the position is refused.
///
/// The centre column must carry a support face at most `up` voxels above `standing_y` or
/// `down` below it — the nearest one, ties to the higher — and that face is the standing
/// layer the body ends on. It is the *centre* column's, not a neighbour's, because
/// `site` has to stay a support face: `step::terrain` removes an animal whose site is
/// not one.
///
/// The disc's clearance is checked at the `headroom` layers ([`headroom_voxels`]) over
/// the **highest ground under the disc** — the greatest within-climb support face of any
/// column the disc actually overlaps, never below `standing_y` or the destination. A
/// disc body straddling a riser rests on the upper surface, and this is not a nicety:
/// checking the clearance at the body's own layer alone deadlocks both the ascent and
/// the descent, because the centre has to cross the column boundary while the disc still
/// overlaps the riser, and a sub-step is capped at one footprint radius. A column whose
/// only faces are outside the climb — a wall, a cliff — contributes nothing, so its
/// solid is checked and refuses exactly as before.
///
/// **A step down is also checked as the next sub-step will see it** (fix 1,
/// `design/handoffs/browser-stuck-in-crags-2026-10-01.md`): when the destination's layer
/// is below `standing_y`, the disc's ground is taken again from the landing layer —
/// [`disc_ground_layer`] from `ny`, never below `ny` — and the disc must be clear over
/// the `headroom` layers above that too ([`pose_clear`]). Otherwise a column within the
/// climb of the upper layer but out of it from the lower one counts as ground on the way
/// down and as a wall once there: the landed body could move nowhere, only turn. The
/// ledge it came down from is within the climb of the landing layer (a step down is at
/// most `down`, and every lineage's `up == down`), so a plain riser still descends. Only
/// descents run the second pass.
///
/// The standing water at the destination must be within the body's wade depth, **or no
/// deeper than the water at the column the sub-step starts from** (within
/// [`LEVEL_TOLERANCE_M`]): a body caught deeper
/// than it can wade may always step toward the shallows or across a pool's level floor,
/// and never deeper; it may also move about the face it stands on.
/// Water this little deeper than where a body stands is the same level: a flowing stream's
/// surface slopes by millimetres a column, and "no deeper" must not read that as deeper. A
/// tolerance, not a knob: a real step deeper is a layer (`voxel_m`) of water.
pub(crate) const LEVEL_TOLERANCE_M: f64 = 0.01;

#[allow(clippy::too_many_arguments)]
fn advance_candidate(
    view: &VoxelView<'_>,
    px: f64,
    pz: f64,
    (fx, fz): (f64, f64),
    standing_y: u32,
    step: f64,
    r: f64,
    wade_depth_m: f64,
    headroom: u32,
    step_limits: StepLimits,
    water_rule: Option<WaterRule>,
) -> Option<(f64, f64, f64, u32)> {
    let c = view.config;
    let v = c.voxel_m;
    let width_m = f64::from(c.width) * v;
    let depth_m = f64::from(c.depth) * v;
    let mut nx = px + fx * step;
    let mut nz = pz + fz * step;
    // Wrap on x; the strip's z ends are walls, not wraps.
    nx = nx.rem_euclid(width_m);
    if nz < r {
        nz = r;
    } else if nz > depth_m - r {
        nz = depth_m - r;
    }
    let cz = (nz / v).floor();
    if cz < 0.0 || cz >= f64::from(c.depth) {
        return None;
    }
    let cx = ((nx / v).floor() as i64).rem_euclid(i64::from(c.width));
    // The step rule: the centre column's support face nearest the body's own standing
    // layer, within the step. No such face is a wall or a cliff edge, and refuses.
    let ny = step_target_layer(view, cx, cz as u32, standing_y, step_limits)?;
    // The body rests on the highest ground under its disc while it straddles a riser,
    // so the clearance is checked from there: every one of the `headroom` layers over
    // it, in one pass over the columns the disc can overlap.
    let cells = disc_box(v, nx, nz, r);
    let clearance = disc_ground_layer(view, cells, nx, nz, r, standing_y, step_limits)
        .max(standing_y.max(ny));
    let hits = headroom > 0 && disc_hits_solid(view, cells, nx, nz, clearance + 1, clearance + headroom, r);
    if hits || (ny < standing_y && !pose_clear(view, nx, nz, ny, r, headroom, step_limits)) {
        // EDGE-2, out of a wall: a body whose disc is already caught in solid where it
        // stands (it grew against a wall, or came down a stem or a vine beside one) may take
        // a step that leaves it no deeper in it; a clear body never enters. TRAPS: "no
        // deeper", not "less deep": along a wall it is caught in (a juvenile grown wider than
        // its ledge, a loftstrider grown against an overhang's side) a step keeps the same
        // depth, and "less deep" refused every heading but the one straight out.
        let here = pose_penetration(view, px, pz, standing_y, r, headroom, step_limits);
        if !(here > 0.0) {
            return None;
        }
        let mut there = if hits { disc_penetration(view, cells, nx, nz, clearance + 1, clearance + headroom, r) } else { 0.0 };
        if ny < standing_y {
            there = there.max(pose_penetration(view, nx, nz, ny, r, headroom, step_limits));
        }
        if !(there <= here + PENETRATION_EPS_M) {
            return None;
        }
    }
    // A water-bound body (roster R2b): another face must have standing water within its
    // margin, or the step is refused as a wall's is.
    if let Some(rule) = water_rule {
        let hz = ((pz / v).floor().max(0.0) as u32).min(c.depth.saturating_sub(1));
        let hx = ((px / v).floor() as i64).rem_euclid(i64::from(c.width));
        let same_face = (hx, hz) == (cx, cz as u32) && ny == standing_y;
        if !same_face && !rule.admits(view, cx, ny, cz as u32) {
            return None;
        }
    }
    let there = view.water_depth_m(cx, ny, cz as u32);
    if there > wade_depth_m {
        // The escape: within the face it stands on, or to a face shallower than it.
        let hz = ((pz / v).floor().max(0.0) as u32).min(c.depth.saturating_sub(1));
        let hx = ((px / v).floor() as i64).rem_euclid(i64::from(c.width));
        let same_face = (hx, hz) == (cx, cz as u32) && ny == standing_y;
        // No deeper (roster mechanics fix 2): across a flat-bottomed pool every face is as
        // deep as the next, and "strictly shallower" held a body in place until it drowned.
        // EDGE: within `LEVEL_TOLERANCE_M`, as a stream's surface slopes by millimetres a
        // column (a seedporter stood 30 min in a stream, the face upstream 2 mm deeper).
        if !same_face && !(there <= view.water_depth_m(hx, standing_y, hz) + LEVEL_TOLERANCE_M) {
            return None;
        }
    }
    // The projection of the move onto the heading, wrap-aware, so a clamp at the z
    // ends is delivered as the short move it is.
    let mut dx = nx - px;
    if dx > width_m / 2.0 {
        dx -= width_m;
    } else if dx < -width_m / 2.0 {
        dx += width_m;
    }
    Some((nx, nz, (dx * fx + (nz - pz) * fz).max(0.0), ny))
}

/// How much deeper in solid a step along a wall it is caught in may leave a body
/// ([`advance_candidate`]), metres: a float tolerance, not a knob.
const PENETRATION_EPS_M: f64 = 1e-9;

/// How deep the disc of radius `r` at `(cx, cz)`, standing on `standing_y`, is in solid over
/// its `headroom` layers above the highest within-step ground under it ([`pose_clear`]'s
/// rows): the most by which any solid voxel's square reaches inside the disc, metres; 0 when
/// the pose is clear.
fn pose_penetration(view: &VoxelView<'_>, cx: f64, cz: f64, standing_y: u32, r: f64, headroom: u32, step: StepLimits) -> f64 {
    if headroom == 0 {
        return 0.0;
    }
    let cells = disc_box(view.config.voxel_m, cx, cz, r);
    let ground = disc_ground_layer(view, cells, cx, cz, r, standing_y, step).max(standing_y);
    disc_penetration(view, cells, cx, cz, ground + 1, ground + headroom, r)
}

/// [`disc_hits_solid`]'s overlap as a depth: over the voxels of layers `lo..=hi` in `cells`,
/// the most by which a solid one's square reaches inside the disc, `r` less its distance from
/// the centre, metres; 0 when none does.
fn disc_penetration(view: &VoxelView<'_>, (x0, x1, z0, z1): (i64, i64, i64, i64), cx: f64, cz: f64, lo: u32, hi: u32, r: f64) -> f64 {
    let c = view.config;
    let v = c.voxel_m;
    if lo >= c.height || lo > hi {
        return 0.0;
    }
    let mut deepest: f64 = 0.0;
    for x in x0..=x1 {
        for z in z0..=z1 {
            if z < 0 || z >= i64::from(c.depth) || !view.solid_in(x, z as u32, lo, hi) {
                continue;
            }
            let qx = cx.clamp(x as f64 * v, (x as f64 + 1.0) * v);
            let qz = cz.clamp(z as f64 * v, (z as f64 + 1.0) * v);
            let d = ((cx - qx).powi(2) + (cz - qz).powi(2)).sqrt();
            if d < r {
                deepest = deepest.max(r - d);
            }
        }
    }
    deepest
}

/// A probe's readout (EDGE-2): whether the motor would take a sub-step of `step` metres
/// along `dir` from `(px, pz)` on `standing_y` for `body` ([`advance_candidate`], no water
/// rule), and whether the pose it stands in now is clear ([`pose_clear`]).
#[allow(clippy::too_many_arguments)]
pub fn motor_probe(
    view: &VoxelView<'_>,
    px: f64,
    pz: f64,
    dir: (f64, f64),
    standing_y: u32,
    step: f64,
    body: &Body,
    wade_depth_m: f64,
    step_limits: StepLimits,
) -> (bool, bool) {
    let v = view.config.voxel_m;
    let (r, headroom) = (body.footprint_radius(), body.headroom_voxels(v));
    (
        advance_candidate(view, px, pz, dir, standing_y, step, r, wade_depth_m, headroom, step_limits, None).is_some(),
        pose_clear(view, px, pz, standing_y, r, headroom, step_limits),
    )
}

/// **DROP: the motor's own walk** of a disc of radius `r` from `(fx, fz)` on layer `standing_y`
/// straight to `(tx, tz)` (no wrap: the caller's local coordinates, `tx` may run past the
/// ring), in sub-steps of at most a quarter voxel and the radius, each one [`advance_candidate`]
/// admits (no water rule): the layer it ends on, `None` when a sub-step is refused or falls
/// short.
#[allow(clippy::too_many_arguments)]
pub(crate) fn motor_walk(
    view: &VoxelView<'_>,
    (fx, fz): (f64, f64),
    (tx, tz): (f64, f64),
    standing_y: u32,
    r: f64,
    wade_depth_m: f64,
    headroom: u32,
    step_limits: StepLimits,
) -> Option<u32> {
    let (dx, dz) = (tx - fx, tz - fz);
    let len = (dx * dx + dz * dz).sqrt();
    if !(len > 0.0) {
        return Some(standing_y);
    }
    let dir = (dx / len, dz / len);
    let sub = (0.25 * view.config.voxel_m).min(r.max(1e-3));
    let n = (len / sub).ceil().max(1.0) as u32;
    let width_m = f64::from(view.config.width) * view.config.voxel_m;
    let (mut px, mut pz, mut y) = (fx.rem_euclid(width_m), fz, standing_y);
    for _ in 0..n {
        let (nx, nz, moved, ny) = advance_candidate(view, px, pz, dir, y, len / f64::from(n), r, wade_depth_m, headroom, step_limits, None)?;
        if moved < 0.5 * len / f64::from(n) {
            return None;
        }
        (px, pz, y) = (nx, nz, ny);
    }
    Some(y)
}

/// Whether a disc of radius `r` centred at `(cx, cz)`, standing on `standing_y`, is clear
/// of solid over its `headroom` layers above the highest within-step ground under it:
/// the clearance [`advance_candidate`] asks of a sub-step that stays on its layer, asked
/// of the pose itself.
pub(crate) fn pose_clear(
    view: &VoxelView<'_>,
    cx: f64,
    cz: f64,
    standing_y: u32,
    r: f64,
    headroom: u32,
    step: StepLimits,
) -> bool {
    if headroom == 0 {
        return true;
    }
    let cells = disc_box(view.config.voxel_m, cx, cz, r);
    let ground = disc_ground_layer(view, cells, cx, cz, r, standing_y, step).max(standing_y);
    !disc_hits_solid(view, cells, cx, cz, ground + 1, ground + headroom, r)
}

/// Whether `body`, placed on `face` as the model places one ([`crate::Pose::at_site`]),
/// stands with its whole disc clear of solid over its headroom ([`pose_clear`]). The
/// column rule ([`has_headroom`]) checks the face's own column only.
pub fn disc_clear_on_face(view: &VoxelView<'_>, body: &Body, face: Site, step: StepLimits) -> bool {
    let pose = crate::Pose::at_site(face, view.config.voxel_m);
    pose_clear(
        view,
        pose.x,
        pose.z,
        face.y,
        body.footprint_radius(),
        body.headroom_voxels(view.config.voxel_m),
        step,
    )
}

/// The support face of column `(x, z)` a body standing on `standing_y` could put its
/// feet on: the one nearest its own layer, at most `up` above it or `down` below, ties
/// to the higher.
///
/// A column cannot hold two support faces one layer apart — a support face is a solid
/// with void over it — so ties only arise for a step of two or more, and the higher
/// face is the ground a body walking over the terrain meets first.
fn step_target_layer(
    view: &VoxelView<'_>,
    x: i64,
    z: u32,
    standing_y: u32,
    step: StepLimits,
) -> Option<u32> {
    // The column's support mask answers it whole: the nearest face at or above and the
    // nearest below, ties to the higher (`VoxelView::nearest_support`).
    view.nearest_support(x, z, standing_y, step.down, step.up)
}

/// The highest within-step support face under the disc at `(cx, cz)`: the surface a
/// disc body straddling a riser actually rests on. `standing_y` when nothing under the
/// disc is higher and reachable.
///
/// The disc's overlap test is [`disc_hits_solid`]'s, to the same strictness, so a column
/// the body merely grazes at exactly the radius does not lift it.
fn disc_ground_layer(
    view: &VoxelView<'_>,
    (x0, x1, z0, z1): (i64, i64, i64, i64),
    cx: f64,
    cz: f64,
    r: f64,
    standing_y: u32,
    step: StepLimits,
) -> u32 {
    if step.up == 0 {
        return standing_y;
    }
    let c = view.config;
    let v = c.voxel_m;
    let mut best = standing_y;
    for x in x0..=x1 {
        for z in z0..=z1 {
            if z < 0 || z >= i64::from(c.depth) {
                continue;
            }
            let qx = cx.clamp(x as f64 * v, (x as f64 + 1.0) * v);
            let qz = cz.clamp(z as f64 * v, (z as f64 + 1.0) * v);
            let (ddx, ddz) = (cx - qx, cz - qz);
            if ddx * ddx + ddz * ddz >= r * r {
                continue;
            }
            // `x` is wrapped by the lookup itself.
            if let Some(y) = step_target_layer(view, x, z as u32, standing_y, step) {
                best = best.max(y);
            }
        }
    }
    best
}

/// The columns a disc of radius `r` at `(cx, cz)` can overlap, `(x0, x1, z0, z1)`
/// inclusive, `x` unwrapped.
fn disc_box(v: f64, cx: f64, cz: f64, r: f64) -> (i64, i64, i64, i64) {
    (
        ((cx - r) / v).floor() as i64,
        ((cx + r) / v).floor() as i64,
        ((cz - r) / v).floor() as i64,
        ((cz + r) / v).floor() as i64,
    )
}

/// Whether the disc at `(cx, cz)` overlaps a solid voxel at any layer in `lo..=hi`
/// (layers past the world's top hold nothing). Strict: a grazing touch at exactly the
/// radius does not block, it contacts. `cells` is the disc's [`disc_box`].
///
/// One pass over the columns for the whole run of layers — a column's layers read as
/// one mask ([`VoxelView::solid_in`]): whether a column overlaps the disc does not depend
/// on the layer.
fn disc_hits_solid(
    view: &VoxelView<'_>,
    (x0, x1, z0, z1): (i64, i64, i64, i64),
    cx: f64,
    cz: f64,
    lo: u32,
    hi: u32,
    r: f64,
) -> bool {
    let c = view.config;
    let v = c.voxel_m;
    if lo >= c.height || lo > hi {
        return false;
    }
    for x in x0..=x1 {
        for z in z0..=z1 {
            if z < 0 || z >= i64::from(c.depth) {
                continue;
            }
            if !view.solid_in(x, z as u32, lo, hi) {
                continue;
            }
            // Nearest point of the voxel's square to the disc centre.
            let qx = cx.clamp(x as f64 * v, (x as f64 + 1.0) * v);
            let qz = cz.clamp(z as f64 * v, (z as f64 + 1.0) * v);
            let (ddx, ddz) = (cx - qx, cz - qz);
            if ddx * ddx + ddz * ddz < r * r {
                return true;
            }
        }
    }
    false
}

/// What a face a climbing lineage has run into offers ([`wall_ascent`], [`wall_descent`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    /// Not a wall: level ground, a step within the lineage's limits, open air with no
    /// ground under it, or the strip's end. Nothing to climb.
    Not,
    /// A wall, but the way along it is shut: an overhang or a roof over the path, or no
    /// room for the body at the far end. The body turns back.
    Blocked,
    /// A climbable face ending on this standing layer of the neighbouring column (the top
    /// of a wall, or the foot of a cliff).
    Climb(u32),
}

/// **The wall rule, going up**: a body standing on
/// `(x, y, z)` faces the column one step along `dir` (`(±1, 0)` or `(0, ±1)`).
///
/// It is a wall when that column is **solid at the body's layer** `y + 1` and its first
/// support face above `y` — the top of the wall, every layer from `y + 1` to it solid —
/// is more than `up` layers higher (a lower one is a step, not a wall). The way up is
/// shut ([`Face::Blocked`]) when anything solid stands in the body's **own column**
/// between its feet and `headroom` layers over the top (an overhang: ceilings are a later
/// package), or when the top has no `headroom` of void over it. Terrain only: a stand is
/// not a voxel and is never a face.
///
/// Column-level, so the route maps can ask it of faces as well as bodies; the live body
/// also checks its disc along the way (`step`'s attach).
pub fn wall_ascent(
    view: &VoxelView<'_>,
    x: i64,
    y: u32,
    z: u32,
    dir: (i64, i64),
    headroom: u32,
    up: u32,
) -> Face {
    let c = view.config;
    let wz = i64::from(z) + dir.1;
    if wz < 0 || wz >= i64::from(c.depth) || y + 1 >= c.height {
        return Face::Not;
    }
    let (wx, wz) = ((x + dir.0).rem_euclid(i64::from(c.width)), wz as u32);
    if !view.is_solid(wx, y + 1, wz) {
        return Face::Not;
    }
    let Some(top) = view.lowest_support_in(wx, wz, y + 1, c.height - 1) else {
        return Face::Not;
    };
    if top - y <= up {
        return Face::Not;
    }
    let ceiling = (top + headroom).min(c.height - 1);
    if (y + 1..=ceiling).any(|yy| view.is_solid(x, yy, z))
        || !has_headroom(view, wx, top, wz, headroom)
    {
        return Face::Blocked;
    }
    Face::Climb(top)
}

/// **The wall rule, going down**: a body standing on `(x, y, z)` at the edge toward the
/// column along `dir`. It is a cliff when that column is **open at the body's own
/// standing layer** `y` and its highest support face below — the foot of the cliff — is
/// more than `down` layers lower. The face is the body's own column's side, so it must be
/// **solid from the foot up to `y`** (a ledge with air under it is an overhang); the way
/// down must be open up to `headroom` layers over `y` (no roof over the lip), and the
/// foot must have `headroom` of void over it. A column with no ground under it at all is
/// not a cliff, it is a hole: [`Face::Not`], refused as before.
///
/// The exact mirror of [`wall_ascent`] from the foot, so a face that can be climbed up
/// can be climbed down and the route maps may join the two ends as one undirected edge.
pub fn wall_descent(
    view: &VoxelView<'_>,
    x: i64,
    y: u32,
    z: u32,
    dir: (i64, i64),
    headroom: u32,
    down: u32,
) -> Face {
    let c = view.config;
    let az = i64::from(z) + dir.1;
    if az < 0 || az >= i64::from(c.depth) {
        return Face::Not;
    }
    let (ax, az) = ((x + dir.0).rem_euclid(i64::from(c.width)), az as u32);
    if view.is_solid(ax, y, az) {
        return Face::Not;
    }
    let Some(foot) = y
        .checked_sub(1)
        .and_then(|below| view.highest_support_in(ax, az, 0, below))
    else {
        return Face::Not;
    };
    if y - foot <= down {
        return Face::Not;
    }
    let ceiling = (y + headroom).min(c.height - 1);
    if !(foot + 1..=y).all(|yy| view.is_solid(x, yy, z))
        || (y + 1..=ceiling).any(|yy| view.is_solid(ax, yy, az))
        || !has_headroom(view, ax, foot, az, headroom)
    {
        return Face::Blocked;
    }
    Face::Climb(foot)
}

/// Whether the disc at `(cx, cz)` is clear of solid over every layer in `layers`: the
/// live body's own check along a face, on top of the column rule.
pub(crate) fn disc_clear(
    view: &VoxelView<'_>,
    cx: f64,
    cz: f64,
    r: f64,
    layers: std::ops::RangeInclusive<u32>,
) -> bool {
    let (lo, hi) = (*layers.start(), *layers.end());
    !disc_hits_solid(view, disc_box(view.config.voxel_m, cx, cz, r), cx, cz, lo, hi, r)
}

/// The contact and wet readings of one body, from its geometry right now. `resolved`
/// is false only when the pose cannot be mapped onto the strip at all; every channel
/// that was evaluated is a valid reading, a zero included.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ContactReading {
    pub front: f64,
    pub left: f64,
    pub right: f64,
    pub underside: f64,
    pub wet: f64,
    pub resolved: bool,
}

impl ContactReading {
    const UNRESOLVED: ContactReading = ContactReading {
        front: 0.0,
        left: 0.0,
        right: 0.0,
        underside: 0.0,
        wet: 0.0,
        resolved: false,
    };
}

/// Read the four contact receptors and the wet response from the body's actual
/// geometry. Front/left/right probe the footprint's boundary arc in their direction
/// (centre and ±45°); the underside reads the support face under the centre; wet is the
/// standing water at the foot, a valid zero when dry.
///
/// **Semantic change, 2026-09-22** (`design/handoffs/voxel-founder-step-2026-09-22.md`):
/// the arcs are probed at `standing_y + 1 + climb` (the step **up**, since package
/// mobility), not at the body layer, because a
/// solid the body could step onto is not a wall. A ledge within the lineage's climb now
/// reads as open ground; a cliff face, a taller rise and a wall read exactly as they did.
/// The receptor is still one layer and one channel — the observation vector's shape and
/// its five Contact slots are untouched — and `climb = 0` reproduces the old reading
/// exactly. A body that stands still while embedded in the riser it just stepped off is
/// the one case the receptor does not describe (see `advance_candidate`); it reads clear
/// there, which is what a body that can step away from it should read.
pub(crate) fn contact_readings(
    view: &VoxelView<'_>,
    pose: &crate::Pose,
    standing_y: u32,
    body: &Body,
    climb: u32,
) -> ContactReading {
    let c = view.config;
    let Some((cx, cz)) = pose.column(c.voxel_m, c.depth) else {
        return ContactReading::UNRESOLVED;
    };
    if standing_y + 1 >= c.height {
        return ContactReading::UNRESOLVED;
    }
    // A probe above the world is open sky, not a wall; one past the strip's `z` edge is
    // a wall at any height (D4).
    let layer = body.contact_layer(standing_y, c.voxel_m) + climb;
    let r = body.footprint_radius();
    let wx = cx.rem_euclid(i64::from(c.width));
    let arc = |centre: f64| boundary_arc(view, pose, r, layer, centre);
    ContactReading {
        front: arc(pose.heading_rad),
        left: arc(pose.heading_rad - std::f64::consts::FRAC_PI_2),
        right: arc(pose.heading_rad + std::f64::consts::FRAC_PI_2),
        underside: f64::from(view.is_support(wx, standing_y, cz)),
        wet: f64::from(view.water_depth_m(wx, standing_y, cz) > 0.0),
        resolved: true,
    }
}

/// One boundary arc: probe at the arc's centre and ±45°, a hair beyond the footprint
/// radius so a body pressed against a wall by the sweep's snap reads the wall it is
/// standing on. A body smaller than a voxel that touches a solid covers at least one
/// probe. A probe past the strip's `z` edge is solid (D4) — the sweep's clamp holds the
/// disc exactly at the edge, so a body pressed there reads it the way it reads a wall; a
/// probe above the world is open.
fn boundary_arc(view: &VoxelView<'_>, pose: &crate::Pose, r: f64, layer: u32, centre: f64) -> f64 {
    let c = view.config;
    let probe = r * (1.0 + 1e-6);
    for da in [
        0.0,
        -std::f64::consts::FRAC_PI_4,
        std::f64::consts::FRAC_PI_4,
    ] {
        let a = centre + da;
        let px = pose.x + probe * a.sin();
        let pz = pose.z + probe * a.cos();
        let z = (pz / c.voxel_m).floor();
        if z < 0.0 || z >= f64::from(c.depth) {
            return 1.0;
        }
        if layer >= c.height {
            continue;
        }
        let wx = ((px / c.voxel_m).floor() as i64).rem_euclid(i64::from(c.width));
        if view.material_at(wx, layer, z as u32).is_solid() {
            return 1.0;
        }
    }
    0.0
}

/// The columns the mouth region covers: the footprint and its short forward reach —
/// the axis from the body's centre to the reach tip's forward extreme, and the tip's
/// lateral extent — probed at the capsule's extreme points (centre, reach tip, the
/// tip's forward and lateral extremes) **and at no more than half a voxel between
/// them**. The five extreme probes alone were enough while a body fitted in about a
/// cell; the 0.75 m browser spans more than two 0.25 m cells from centre to
/// tip, and five probes skipped the column in the middle — the one a crown beside the
/// head stands in.
pub(crate) fn mouth_columns(
    view: &VoxelView<'_>,
    pose: &crate::Pose,
    body: &Body,
) -> Vec<(i64, u32)> {
    let mut cols: Vec<(i64, u32)> = Vec::new();
    mouth_columns_each(view, pose, body, |entry| {
        if !cols.contains(&entry) {
            cols.push(entry);
        }
    });
    cols
}

/// `q.floor() as i64`, without the library call: the truncation, one lower for a
/// negative non-integer. The same value for every `q` a world coordinate can be.
#[inline]
fn floor_i64(q: f64) -> i64 {
    let t = q as i64;
    if (t as f64) > q { t - 1 } else { t }
}

/// `x.rem_euclid(width)` for an `x` within a ring's width of it, without the division.
#[inline]
pub(crate) fn wrap_x(x: i64, width: i64) -> i64 {
    if x < 0 {
        if x >= -width { x + width } else { x.rem_euclid(width) }
    } else if x >= width {
        if x < 2 * width { x - width } else { x.rem_euclid(width) }
    } else {
        x
    }
}

/// [`mouth_columns`] **with repeats**: every probe's column in probe order, for a caller
/// that deduplicates a union over many poses itself (`encounter::mouth_columns_from_face`).
/// Its first occurrences are exactly [`mouth_columns`], in the same order.
pub(crate) fn mouth_columns_each(view: &VoxelView<'_>, pose: &crate::Pose, body: &Body, mut each: impl FnMut((i64, u32))) {
    let c = view.config;
    let v = c.voxel_m;
    let r = body.footprint_radius();
    let reach = body.mouth_reach_m;
    let (fx, fz) = pose.forward();
    let (rx, rz) = (fz, -fx);
    let tip = (pose.x + (r + reach) * fx, pose.z + (r + reach) * fz);
    // Along the axis, centre to the tip's forward extreme; then across the tip, one
    // lateral extreme to the other. Each run's endpoints are the old extreme probes.
    let axis = 2.0 * r + reach;
    let axis_steps = ((axis / (0.5 * v)).ceil() as usize).max(1);
    let side_steps = ((2.0 * r / (0.5 * v)).ceil() as usize).max(1);
    let (width, depth) = (i64::from(c.width), i64::from(c.depth));
    let mut probe = |px: f64, pz: f64| {
        let z = floor_i64(pz / v);
        if z < 0 || z >= depth {
            return;
        }
        each((wrap_x(floor_i64(px / v), width), z as u32));
    };
    for k in 0..=axis_steps {
        let d = axis * (k as f64) / (axis_steps as f64);
        probe(pose.x + d * fx, pose.z + d * fz);
    }
    for k in 0..=side_steps {
        let d = -r + 2.0 * r * (k as f64) / (side_steps as f64);
        probe(tip.0 + d * rx, tip.1 + d * rz);
    }
}

/// The stock of one ground food class at a site: what a mouth over it could take.
fn pool_stock(fv: &FloraView<'_>, site: Site, food: crate::Food) -> f64 {
    fv.ground_at(site).map_or(0.0, |g| match food {
        crate::Food::Litter => g.litter,
        crate::Food::Carrion => g.carrion,
        // Glassfilm (roster R2b): the film pool at the face above its grazing refuge
        // (TASTE): a face grazed to the refuge tastes of nothing, as `take_film` gives
        // nothing there.
        crate::Food::Film => fv.edible_film(g),
        // Neither is a ground pool; a mouth never asks this of them. Nectar (F1) is a
        // flower product, not a pool, and fruit (F3) a hanging one.
        crate::Food::CapTissue | crate::Food::Foliage | crate::Food::Nectar | crate::Food::Fruit => 0.0,
    })
}

/// The ground site holding the most of `food` under the mouth region, and how much, or
/// `None` when the mouth touches none of it at all. Ties go to the smallest site, so
/// the answer is a pure function of the state and never of storage order.
///
/// Since decisions §3 the shredder has two ground foods rather than one — litter and
/// carrion — and this is the one scan, asked twice.
pub(crate) fn mouth_pool_site(
    fv: &FloraView<'_>,
    cols: &[(i64, u32)],
    standing_y: u32,
    food: crate::Food,
) -> Option<(Site, f64)> {
    let mut best: Option<(Site, f64)> = None;
    let mut best_amount = 0.0;
    for &(x, z) in cols {
        let site = Site {
            x: x as u32,
            y: standing_y,
            z,
        };
        let amount = pool_stock(fv, site, food);
        if amount > best_amount
            || (amount > 0.0 && amount == best_amount && best.is_some_and(|(b, _)| site < b))
        {
            best = Some((site, amount));
            best_amount = amount;
        }
    }
    best
}

/// Where one bite would come from: a ground pool at a site, or the tissue of a stand
/// rooted at one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Meal {
    /// A withdrawal from `site`'s [`crate::Food::Litter`] or [`crate::Food::Carrion`]
    /// pool.
    Pool { site: Site, food: crate::Food },
    /// A foliage withdrawal from the stand rooted at `root` in `place` on its face, bounded
    /// to the mouth's own layer range. Its food class is the stand's own, by its species'
    /// income rule ([`crate::Food::of_stand`]): a saprotroph's cap tissue, a plant's
    /// foliage — not whose mouth took it. The place is the stand's own: a face may hold a
    /// plant, a fungus and a plant beneath the first (a woody recruit in its sward,
    /// `FaceLayer::Under`), and the bite takes the one the mouth chose.
    Stand { root: Site, place: cubarium_voxel_flora::FaceLayer, food: crate::Food },
    /// Exposed living mycelium on a mouth column, after its edible caps are cropped.
    Mycelium { root: Site },
}

impl Meal {
    /// The food class this bite is.
    pub(crate) fn food(self) -> crate::Food {
        match self {
            Meal::Pool { food, .. } | Meal::Stand { food, .. } => food,
            Meal::Mycelium { .. } => crate::Food::CapTissue,
        }
    }
}

/// **A lineage's foods at one mouth, and which of them it would take** (decisions §3,
/// generalised over the diet list): for each food in `diet`, litter and carrion from the
/// ground pool at the standing face, cap tissue from a saprotroph stand and foliage from a
/// photosynthetic one whose layers the mouth band reaches.
///
/// There is no new action: `bite` still means "take what is at the mouth", so when more
/// than one food is there this picks the **richest available** — the shredder's rule,
/// stated once for every diet. Ties go in [`crate::Food::ALL`] order — litter, cap
/// tissue, carrion, foliage — so the answer is a pure function of the state.
///
/// It reports two amounts: the **stock** a bite diminishes on (a pool's whole stock at
/// the site, or the stand's stock in the layers the band reaches, not its whole cap), and
/// the **offer** the comparison is made on and the taste channel encodes — the stock, or
/// for a stand `(1 − defence) ×` its stock (audit round §4: the defence is readable). A
/// pool has no defence, so its offer is its stock.
pub(crate) fn mouth_meal(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    cols: &[(i64, u32)],
    standing_y: u32,
    body: &Body,
    diet: crate::Diet,
) -> Option<(Meal, f64, f64)> {
    let mut best: Option<(Meal, f64, f64)> = None;
    for food in diet.foods() {
        let found = if food.is_pool() {
            mouth_pool_site(fv, cols, standing_y, food)
                .map(|(site, stock)| (Meal::Pool { site, food }, stock, stock))
        } else {
            mouth_foliage_offer(fv, view, cols, standing_y, body, crate::Diet::only(food))
                .map(|(root, place, stock, offer)| {
                    // A bite may finish the caps and continue into the exposed root
                    // tissue. Include that food in its diminishing-stock budget, or
                    // tiny regrown caps can prevent it ever reaching the mycelium.
                    let mycelium = if food == crate::Food::CapTissue
                        && root.y == standing_y
                        && mouth_crown_layers(standing_y, body, view.config.voxel_m).contains(&(i64::from(standing_y)+1)) {
                        fv.stand_in(root, place).filter(|s| fv.edible_foliage(s) <= stock).map_or(0.0, |s|
                            fv.config.species(s.species).mycelium_graze_fraction.clamp(0.0,1.0)*s.wood.max(0.0))
                    } else { 0.0 };
                    (Meal::Stand { root, place, food }, stock+mycelium, offer+mycelium)
                })
                .or_else(|| {
                    if food != crate::Food::CapTissue
                        || !mouth_crown_layers(standing_y, body, view.config.voxel_m).contains(&(i64::from(standing_y) + 1)) {
                        return None;
                    }
                    // Mycelium is at its root face, rather than in a crown. A mouth
                    // cannot eat the stand on another ledge just because it smells it.
                    cols.iter().filter_map(|&(x,z)| {
                        let root = Site { x: x.rem_euclid(i64::from(view.config.width)) as u32, y: standing_y, z };
                        let s = fv.fungus_at(root)?;
                        let stock = fv.edible_mycelium(s);
                        (stock > 0.0).then_some((Meal::Mycelium { root }, stock, stock))
                    }).max_by(|a,b| a.2.total_cmp(&b.2).then_with(|| {
                        let root = |m: Meal| match m { Meal::Mycelium { root } => root, _ => unreachable!() };
                        root(b.0).cmp(&root(a.0))
                    }))
                })
        };
        if let Some((meal, stock, offer)) = found
            && stock > 0.0
            && best.is_none_or(|(_, _, b)| offer > b)
        {
            best = Some((meal, stock, offer));
        }
    }
    best
}

/// A lineage's step in whole voxels: how many layers up, and how many down, one
/// sub-step may move its standing face ([`FounderPhysiology::step_limits`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StepLimits {
    pub up: u32,
    pub down: u32,
}

impl StepLimits {
    /// The largest rise a body can take **both ways** between two faces: the undirected
    /// adjacency a component or an acceptance check needs is mutual reachability, and a
    /// face `d` layers above another is reachable from it and back only when `d` is
    /// within both limits.
    pub fn mutual(&self) -> u32 {
        self.up.min(self.down)
    }
}

/// A length in metres as whole voxels, **rounded down** (with a hair of tolerance for a
/// limit that is an exact multiple): a riser of `k` voxels is within a step of `m` metres
/// when `k · voxel_m <= m`. A non-finite or negative length is no step at all.
pub fn step_voxels(m: f64, voxel_m: f64) -> u32 {
    if !m.is_finite() || m <= 0.0 || !voxel_m.is_finite() || voxel_m <= 0.0 {
        return 0;
    }
    (m / voxel_m + 1e-9).floor().clamp(0.0, f64::from(u32::MAX)) as u32
}

/// Whether `headroom` whole voxels of void stand over the face `(x, y, z)`.
pub fn has_headroom(view: &VoxelView<'_>, x: i64, y: u32, z: u32, headroom: u32) -> bool {
    let c = view.config;
    (1..=headroom).all(|d| {
        let yy = y + d;
        yy < c.height && !view.is_solid(x, yy, z)
    })
}

/// The crown layers one mouth can take food from, standing on `standing_y`: the cells
/// the physical band `[0, 1.33 × body height]` over the standing surface overlaps
/// (decisions §2). [`Body::mouth_layers`], named here because this is where the live
/// mouth asks for it.
pub(crate) fn mouth_crown_layers(
    standing_y: u32,
    body: &Body,
    voxel_m: f64,
) -> std::ops::RangeInclusive<i64> {
    body.mouth_layers(standing_y, voxel_m)
}

/// The stand whose crown cells the mouth region physically touches **and whose tissue
/// this `diet` accepts**: a crown cell of `stand.site` in a mouth column, at a layer
/// inside the physical mouth band ([`Body::mouth_layers`]). The most foliage wins, ties
/// to the smallest root site. `None` when the mouth is in air — a neighbouring stand
/// whose crown does not reach the mouth is not mouth input.
///
/// Reach and permission are separate tests (audit §2), and the permission is applied
/// here rather than at the withdrawal so that a stand a mouth may not eat is not
/// offered to it as taste either: since decisions §3 a glowcap cap standing in a
/// browser's band is reachable fungal tissue and not food, and a shredder's mouth sees
/// the cap and no leaf ([`crate::Diet`]).
pub(crate) fn mouth_foliage_stand(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    cols: &[(i64, u32)],
    standing_y: u32,
    body: &Body,
    diet: crate::Diet,
) -> Option<(Site, f64)> {
    mouth_foliage_offer(fv, view, cols, standing_y, body, diet).map(|(s, _, stock, _)| (s, stock))
}

/// [`mouth_foliage_stand`]'s rule with the **offer** it chose by: among the stands the
/// mouth touches, the one with the most `(1 − defence) × reachable stock` — what a bite
/// would actually get, since the plant layer takes `(1 − defence)` of the ask. Returns the stand, its
/// reachable stock (the `E` a bite diminishes on: the defence shrinks the bite once, at
/// the withdrawal, and not again here) and its offer (what taste encodes).
pub(crate) fn mouth_foliage_offer(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    cols: &[(i64, u32)],
    standing_y: u32,
    body: &Body,
    diet: crate::Diet,
) -> Option<(Site, cubarium_voxel_flora::FaceLayer, f64, f64)> {
    let layers = mouth_crown_layers(standing_y, body, view.config.voxel_m);
    let mut best: Option<(Site, cubarium_voxel_flora::FaceLayer, f64, f64)> = None;
    let mut consider = |stand: &cubarium_voxel_flora::Stand| {
        if !(stand.foliage > 0.0 && diet.accepts(fv.config.species(stand.species).trophic)) {
            return;
        }
        let reachable = reachable_layer_stock(fv, view, stand, cols, &layers);
        if !(reachable > 0.0) {
            return;
        }
        let offer = (1.0 - stand.defence.clamp(0.0, 1.0)) * reachable;
        let better = match best {
            None => true,
            Some((site, place, _, o)) => offer > o || (offer == o && (stand.site, stand.place) < (site, place)),
        };
        if better {
            best = Some((stand.site, stand.place, reachable, offer));
        }
    };
    // The best offer wins, ties to the smallest site: a total order, so visiting only
    // the stands near the mouth picks what the scan of every stand picked.
    match stands_near_mouth(fv, view, cols) {
        Some(near) => near.iter().for_each(|&i| consider(&fv.stands[i as usize])),
        None => fv.stands.iter().for_each(&mut consider),
    }
    best
}

/// The stands whose crowns could reach any of the mouth columns `cols`, as ascending
/// indices into `fv.stands`: the flora's reach index ([`FloraView::stands_near`]) for
/// each column, merged. `None` when the view holds no current index, and the caller
/// visits every stand.
///
/// A stand left out cannot touch the mouth: the index lists every stand at every column
/// its widest layer disc boxes, and [`each_reachable_layer`] needs a layer disc to cover
/// a mouth column.
fn stands_near_mouth(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    cols: &[(i64, u32)],
) -> Option<Vec<u32>> {
    let (w, d) = (view.config.width, view.config.depth);
    let mut near: Vec<u32> = Vec::new();
    for &(cx, cz) in cols {
        near.extend_from_slice(fv.stands_near(cx, cz, w, d)?);
    }
    near.sort_unstable();
    near.dedup();
    #[cfg(debug_assertions)]
    {
        // The index against the scan it replaces: every stand that touches the mouth at
        // any band is in it.
        let every = 0..=i64::from(view.config.height);
        for (i, stand) in fv.stands.iter().enumerate() {
            if !reachable_layers(fv, view, stand, cols, &every).is_empty() {
                debug_assert!(
                    near.binary_search(&(i as u32)).is_ok(),
                    "the reach index missed {:?} at {:?} for {cols:?}",
                    stand.species,
                    stand.site
                );
            }
        }
    }
    Some(near)
}

/// What a mouth over `cols` whose band selects the cell range `layers` can take from
/// **one stand**: the sum of the edible stocks ([`cubarium_voxel_flora::StandLayer::edible`],
/// above the grazing floor) of its foliage layers whose disc cell is in range and whose
/// own disc — its own radius, which is a fraction of the crown's — covers one of the
/// columns.
///
/// This is the whole of what layers changed about reach. Before, a stand was in reach
/// or it was not and the answer was its whole `foliage`; now an adult bloomcrown offers
/// a low mouth its basal rosette and keeps its crown three cells up.
pub(crate) fn reachable_layer_stock(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    stand: &cubarium_voxel_flora::Stand,
    cols: &[(i64, u32)],
    layers: &std::ops::RangeInclusive<i64>,
) -> f64 {
    let mut sum = 0.0;
    each_reachable_layer(fv, view, stand, cols, layers, |_, stock| sum += stock);
    sum
}

/// The same, **per layer**: `(index among the stand's foliage layers, stock)`, which is
/// what an observer reporting a per-layer split reads.
pub(crate) fn reachable_layers(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    stand: &cubarium_voxel_flora::Stand,
    cols: &[(i64, u32)],
    layers: &std::ops::RangeInclusive<i64>,
) -> Vec<(usize, f64)> {
    let mut out = Vec::new();
    each_reachable_layer(fv, view, stand, cols, layers, |i, stock| out.push((i, stock)));
    out
}

/// [`reachable_layers`] one layer at a time, in profile order, with no vector: the scan
/// every mouth runs over every stand on every tick.
///
/// The stand's geometry — its `crown_voxels` and its layers' disc cells and radii — is
/// read through the flora's cached crown ([`FloraView::crown_voxels`],
/// [`FloraView::layers`]), which is a fresh computation to the bit
/// (`design/handoffs/voxel-hot-path-geometry-2026-09-24.md`, item 4); only the stocks move
/// on a bite. Whether a layer's disc covers a mouth column is asked of each column
/// ([`disc_covers`]) instead of by walking the disc's cells against the column list.
fn each_reachable_layer(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    stand: &cubarium_voxel_flora::Stand,
    cols: &[(i64, u32)],
    layers: &std::ops::RangeInclusive<i64>,
    mut f: impl FnMut(usize, f64),
) {
    // A cheap bound before the layers are read at all: every layer of a stand sits
    // between the first cell over its support face and its crown cell, so a mouth whose
    // range misses that span can skip the stand. This scan runs over every stand for
    // every mouth on every tick.
    let crown = fv.cached_crown(stand);
    let crown_voxels = match crown {
        Some(c) => c.crown_voxels(),
        None => fv.crown_voxels(stand),
    };
    let lowest = i64::from(stand.site.y) + 1;
    let highest = i64::from(stand.site.y) + i64::from(crown_voxels);
    if *layers.end() < lowest || *layers.start() > highest {
        return;
    }
    let width = i64::from(view.config.width);
    let depth = i64::from(view.config.depth);
    let (sx, sz) = (i64::from(stand.site.x), i64::from(stand.site.z));
    // The same bound across: no layer's disc reaches past the crown's widest span, so a
    // mouth with no column that near is skipped without reading a layer.
    if let Some(c) = crown {
        let span = c.max_disc_span();
        if !cols
            .iter()
            .any(|&(cx, cz)| within_span(sx, sz, span, width, depth, cx, cz))
        {
            return;
        }
    }
    let every = match crown {
        Some(c) => c.layers(fv.config, stand),
        None => cubarium_voxel_flora::layer_iter(fv.config, stand, fv.config.voxel_m),
    };
    for layer in every.filter(|l| l.kind.bears_foliage()) {
        // What a mouth could take is what stands above the grazing floor: the flora
        // layer's one reading of food.
        let edible = layer.edible();
        if !(edible > 0.0) || !layers.contains(&layer.cell) {
            continue;
        }
        let span = layer.radius_v.floor() as i64;
        let r2 = layer.radius_v * layer.radius_v;
        if cols
            .iter()
            .any(|&(cx, cz)| disc_covers(sx, sz, span, r2, width, depth, cx, cz))
        {
            f(layer.foliage_index.unwrap_or(0), edible);
        }
    }
}

/// Whether the integer disc of cells a layer of radius² `r2` covers around the column
/// `(sx, sz)` — every offset `(dx, dz)` within `±span` whose `dx² + dz²` is not over
/// `r2`, `x` wrapped, `z` inside the strip — holds the column `(cx, cz)`.
///
/// The disc walk it replaces asked, cell by cell, whether the mouth's column list held
/// the cell; this asks the one question per mouth column. They agree exactly: the offsets
/// that land on `cx` are `d`, `d − width`, … for `d = (cx − sx) mod width`, the nearest
/// of them to the stand has the least `dx²`, and the disc test only gets harder as `|dx|`
/// grows — so the column is covered exactly when that nearest offset is within the span
/// and passes the disc test. A column the walk could never produce (an unwrapped `x`, a
/// `z` off the strip) is covered by nothing, as before.
#[inline]
#[allow(clippy::too_many_arguments)]
fn disc_covers(sx: i64, sz: i64, span: i64, r2: f64, width: i64, depth: i64, cx: i64, cz: u32) -> bool {
    let Some((dx, dz)) = nearest_offset(sx, sz, width, depth, cx, cz) else {
        return false;
    };
    dx <= span && dz.abs() <= span && !((dx * dx + dz * dz) as f64 > r2)
}

/// Whether the column `(cx, cz)` is within `span` cells of `(sx, sz)` in wrapped `x` and
/// in `z`: the box every disc of that span lies in ([`disc_covers`] without the disc).
#[inline]
fn within_span(sx: i64, sz: i64, span: i64, width: i64, depth: i64, cx: i64, cz: u32) -> bool {
    nearest_offset(sx, sz, width, depth, cx, cz).is_some_and(|(dx, dz)| dx <= span && dz.abs() <= span)
}

/// The offset of the column `(cx, cz)` from `(sx, sz)` with the least `|dx|` of the
/// `x` offsets that land on it, as `(|dx|, dz)`; `None` for a column no cell walk could
/// produce (an unwrapped `x`, a `z` off the strip).
#[inline]
fn nearest_offset(sx: i64, sz: i64, width: i64, depth: i64, cx: i64, cz: u32) -> Option<(i64, i64)> {
    let cz = i64::from(cz);
    if cx < 0 || cx >= width || cz >= depth {
        return None;
    }
    // `(cx - sx) mod width`: both are on the strip, so one correction does it.
    let d = cx - sx;
    let d = if (0..width).contains(&d) {
        d
    } else if (-width..0).contains(&d) {
        d + width
    } else {
        d.rem_euclid(width)
    };
    Some((d.min(width - d), cz - sz))
}

/// Every foliage-bearing stand whose crown touches the actual mouth probe columns.
/// The public fauna wrapper exposes this only as a read-only autopsy diagnostic.
pub(crate) fn mouth_foliage_stands(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    cols: &[(i64, u32)],
    standing_y: u32,
    body: &Body,
    diet: crate::Diet,
) -> Vec<(Site, f64)> {
    foliage_stands_touching(
        fv,
        view,
        cols,
        &mouth_crown_layers(standing_y, body, view.config.voxel_m),
        diet,
    )
}

/// Every foliage-bearing stand whose crown **cells** sit at a layer in `layers`,
/// intersect `cols`, and whose tissue `diet` accepts.
///
/// This is [`mouth_foliage_stands`]' own scan with the layer range handed in instead of
/// derived from the manifest, because the layer range is the *only* thing that differs
/// between today's whole-voxel mouth and a metre band measured from the standing surface
/// (`design/voxel-encounter-contract-2026-09-21.md`, "Discretisation"): a crown is one
/// cell thick, so any band over the surface selects a contiguous run of layers. Nothing
/// in the live tick calls this with anything but the mouth's own range.
pub(crate) fn foliage_stands_touching(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    cols: &[(i64, u32)],
    layers: &std::ops::RangeInclusive<i64>,
    diet: crate::Diet,
) -> Vec<(Site, f64)> {
    let mut out = Vec::new();
    let mut consider = |stand: &cubarium_voxel_flora::Stand| {
        if !(stand.foliage > 0.0 && diet.accepts(fv.config.species(stand.species).trophic)) {
            return;
        }
        let reachable = reachable_layer_stock(fv, view, stand, cols, layers);
        if reachable > 0.0 {
            out.push((stand.site, reachable));
        }
    };
    // In stand order either way: the index lists ascending stand indices.
    match stands_near_mouth(fv, view, cols) {
        Some(near) => near.iter().for_each(|&i| consider(&fv.stands[i as usize])),
        None => fv.stands.iter().for_each(&mut consider),
    }
    out
}

/// A taste reading: the contact's cue response, the manifest's fixed resistance for the
/// material actually touched, and whether the mouth contacted anything at all.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TasteReading {
    pub cue: f64,
    pub resistance: f64,
    pub valid: bool,
}

impl TasteReading {
    /// Zero data, validity zero: no contact, or no channel to read.
    const INVALID: TasteReading = TasteReading {
        cue: 0.0,
        resistance: 0.0,
        valid: false,
    };
}

/// Read chemistry and resistance at the mouth's actual contact. The blind founder's
/// mouth roots at the ground: **detritus** under its mouth produces a cue response,
/// while bare ground is a valid ground contact with zero response. The diffused
/// `Chem(detritus)` field is deliberately not consulted. The browser's mouth is at body
/// height and is invalid in air.
///
/// # What decisions §3 changed here, and what it did not
///
/// The shredder now has three foods, so "what is at the mouth" is the richest of the
/// three at the mouth and not the litter alone: a shredder standing on a corpse tastes
/// the corpse. The **channel** is untouched — one cue, one resistance, one validity, at
/// the manifest's own slot — and so is the resistance mapping: all three foods are the
/// one soft class the schema calls `litter`, because the taste resistances are inside
/// the manifest digest; contract v2 left them as they were.
///
/// The browser's taste moves the other way: a glowcap cap is no longer offered to its
/// mouth at all, so it no longer tastes one.
#[cfg(test)]
pub(crate) fn taste_reading(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    pose: &crate::Pose,
    standing_y: u32,
    manifest: &Manifest,
    body: &Body,
    founder: Founder,
) -> TasteReading {
    taste_reading_at(
        fv,
        view,
        pose,
        standing_y,
        manifest,
        body,
        FounderPhysiology::placeholder(founder).diet,
        // The unit tests' bodies are the placeholder tables' (mass 1).
        FounderPhysiology::placeholder(founder).taste_reference_on(
            crate::CueConfig::default().emit_reference,
            view.config.voxel_m,
            fv.config,
        ),
    )
}

/// [`taste_reading`] against a reference stock `reference`
/// ([`FounderPhysiology::taste_reference_stock`]: the lineage's own `taste_reference`, else
/// `emit_reference × mass_scale`, `manifest::TASTE_REFERENCE_RULE`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn taste_reading_at(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    pose: &crate::Pose,
    standing_y: u32,
    manifest: &Manifest,
    body: &Body,
    diet: crate::Diet,
    reference: f64,
) -> TasteReading {
    let sat = manifest.tunings.chem_saturation;
    // The contact-material response: `amount / reference` cue units through the same
    // curve the field's concentration reads through. The browser's foliage cue is still
    // this — there is no foliage field to sample.
    let response = |amount: f64| {
        let c = (amount / reference).min(1.0);
        c / (c + sat)
    };
    let cols = mouth_columns(view, pose, body);
    // What is at the mouth is the lineage's own diet's richest offer ([`mouth_meal`]).
    // The **channel's** class and its reading in air are the manifest's (P2 owns them):
    // the blind layout reads every food as its one soft class `litter`, and bare ground
    // as a valid zero; the lanternjaw's reads `carrion`, ground likewise; the browser's
    // reads `foliage`, and a mouth in air is no contact and no taste.
    let offer = mouth_meal(fv, view, &cols, standing_y, body, diet).map(|(_, _, offer)| offer);
    // The class is the manifest's taste cue; a mouth on bare ground is a valid zero for
    // every layout but the foliage-tasting ones (the browser's and the loftstrider's).
    // The capgnawer keeps the blind layout's `litter` class: its caps and litter are both
    // soft, as the shredder's three foods are.
    let class = manifest.taste_cue;
    let ground_valid = class != "foliage";
    match offer {
        Some(offer) => TasteReading {
            cue: response(offer),
            resistance: resistance_of(manifest, class),
            valid: true,
        },
        None if ground_valid => TasteReading {
            cue: 0.0,
            resistance: resistance_of(manifest, "ground"),
            valid: true,
        },
        None => TasteReading::INVALID,
    }
}

/// The manifest's fixed resistance for a material class; a class the schema does not
/// map reads 0 rather than an invented number (the manifest's own tests pin the
/// classes each founder maps).
fn resistance_of(manifest: &Manifest, class: &str) -> f64 {
    manifest
        .taste_resistances
        .iter()
        .find(|(c, _)| *c == class)
        .map_or(0.0, |&(_, r)| r)
}

/// The observation vector for one founder body, built from the **pre-action state of
/// this tick**: the body's own stocks and prior-interval feedback, and the receptors'
/// geometry as it stands before anything moves or eats. Everything is clamped; a
/// non-finite input is rejected to zero rather than trained through. `senses` carries the
/// per-arena litter field and trend stores; without it (the live schedule) `Chem` reads
/// zero with validity 0. Taste remains contact-local, while `Light` and the `Cone` are
/// pure geometry. `cone_occupancy` is prepared once for all observations in a controller
/// stage so each browser does not rescan the world.
#[cfg(test)]
pub(crate) fn observation(
    fauna: &Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    manifest: &Manifest,
    senses: Option<&mut crate::Senses>,
    cone_occupancy: Option<crate::senses::Occ<'_>>,
) -> Vec<f64> {
    let (obs, writes) = observation_read(fauna, i, view, fv, manifest, senses.as_deref(), cone_occupancy, true, None);
    if let Some(s) = senses {
        writes.apply(s, view);
    }
    obs
}

/// [`observation`] reading only: the senses handle's writes (the body's chem-trend store,
/// a light-memo miss) come back beside the vector for the caller to apply. A body's
/// observation reads nothing another body's observation writes, so a stage's
/// observations can be built side by side and their writes applied afterwards.
pub(crate) fn observation_read(
    fauna: &Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    manifest: &Manifest,
    senses: Option<&crate::Senses>,
    cone_occupancy: Option<crate::senses::Occ<'_>>,
    full_observation: bool,
    blooms: Option<&crate::flight::Blooms>,
) -> (Vec<f64>, crate::senses::SenseWrites) {
    let mut writes = crate::senses::SenseWrites::default();
    let a = &fauna.animals[i];
    let founder = a
        .founder
        .expect("an observation is built for a founder body");
    let phys = fauna.config.founder(founder);
    if let Some(flight) = phys.flight {
        return (flyer_observation(fauna, i, view, fv, manifest, cone_occupancy, &flight, full_observation, blooms), writes);
    }
    // The body this animal actually has right now: the anchors below are fractions of
    // it, so a growing founder's eye, mouth and receptors rise with it (decisions §1).
    let body = phys.body_at(a.body);
    let mut obs = vec![0.0; manifest.inputs()];
    // Where the receptors are: the standing face, or on a wall the layer the feet have
    // reached (package mobility: "its senses read from where it is").
    let standing_y = a.sense_layer(view.config.voxel_m);

    // Self: real body state against the manifest's fixed references (times the
    // physiology's `self_reference_scale`, 1 under the contract).
    let fb = &a.founder_state.feedback;
    let k = phys.mass_scale;
    obs[0] = clamp01(a.energy / (manifest.adult_energy_reference * k));
    obs[1] = clamp01(a.reserve / (manifest.adult_reserve_reference * k));
    obs[2] = f64::from(birth_readiness(&fauna.config, a, fv));
    obs[3] = clamp01(fb.structural_loss / (manifest.structural_reference * k));
    obs[4] = clamp01(fb.intake / (manifest.body_reference * k));
    obs[5] = clamp01(fb.delivered_forward / manifest.forward_reference_m);
    obs[6] = clamp_sym(fb.delivered_turn / manifest.turn_reference_rad);
    obs[7] = if fb.attempted_equivalent > NONE_REQUESTED {
        clamp01(fb.delivered_equivalent / fb.attempted_equivalent)
    } else {
        1.0
    };

    // Contact(4), Wet and Taste: the body's geometry right now.
    let cm = module(manifest, "Contact(4)");
    let wm = module(manifest, "Wet");
    let tm = module(manifest, "Taste(1)");
    let reading = contact_readings(
        view,
        &a.pose,
        standing_y,
        &body,
        phys.step_limits(view.config.voxel_m).up,
    );
    let valid = f64::from(reading.resolved);
    obs[cm.offset] = reading.front;
    obs[cm.offset + 1] = reading.left;
    obs[cm.offset + 2] = reading.right;
    obs[cm.offset + 3] = reading.underside;
    obs[cm.offset + 4] = valid;
    obs[wm.offset] = reading.wet;
    obs[wm.offset + 1] = valid;
    let taste = taste_reading_at(
        fv,
        view,
        &a.pose,
        standing_y,
        manifest,
        &body,
        phys.diet,
        phys.taste_reference_on(fauna.config.cue.emit_reference, view.config.voxel_m, fv.config),
    );
    // A fruit taste (F3, the seedporter): the fruit in the mouth's reach — the stand it
    // climbs or perches on, else the nearest fruiting crown cell or vine face in fruit —
    // against the lineage's reference; nothing in reach is a valid zero.
    let taste = if manifest.taste_cue == "fruit" {
        let reference = phys.taste_reference_on(fauna.config.cue.emit_reference, view.config.voxel_m, fv.config);
        let sat = manifest.tunings.chem_saturation;
        match crate::step::glider_fruit_in_reach(view, fv, a, phys) {
            Some((_, stock)) => {
                let c = (stock / reference).min(1.0);
                TasteReading {
                    cue: c / (c + sat),
                    resistance: resistance_of(manifest, "fruit"),
                    valid: true,
                }
            }
            None => TasteReading {
                cue: 0.0,
                resistance: resistance_of(manifest, "ground"),
                valid: true,
            },
        }
    } else {
        taste
    };
    obs[tm.offset] = taste.cue;
    obs[tm.offset + 1] = taste.resistance;
    obs[tm.offset + 2] = f64::from(taste.valid);

    // Chem(detritus): the arena's detritus field at the receptor, response then trend
    // then validity. Blind founder only. Without a senses handle the module reads zero
    // with validity 0.
    // The capgnawer's `Chem(fungal)` (roster R1) reads the fungal field the same way, in
    // the same slot.
    // The ripple-snail's `Chem(film)` (roster R2b) reads the film field, in the same slot.
    #[derive(Clone, Copy)]
    enum ChemField {
        Detritus,
        Fungal,
        Film,
    }
    let chem = module_opt(manifest, "Chem(detritus)")
        .map(|m| (m, ChemField::Detritus))
        .or_else(|| module_opt(manifest, "Chem(fungal)").map(|m| (m, ChemField::Fungal)))
        .or_else(|| module_opt(manifest, "Chem(film)").map(|m| (m, ChemField::Film)));
    if let Some((cm, field)) = chem {
        if let Some(senses) = senses {
            let sampled = match field {
                ChemField::Detritus => senses.sample_cue(view, &a.pose, standing_y),
                ChemField::Fungal => senses.sample_fungal(view, &a.pose, standing_y),
                ChemField::Film => senses.sample_film(view, &a.pose, standing_y),
            };
            if let Some(cue) = sampled {
                let sat = manifest.tunings.chem_saturation;
                let q = cue / (cue + sat);
                obs[cm.offset] = clamp01(q);
                let (trend, smoothed) = senses.peek_chem_trend(a.id, q, manifest);
                obs[cm.offset + 1] = trend;
                writes.chem = Some((a.id, smoothed));
                obs[cm.offset + 2] = 1.0;
            }
        }
    }

    // Light: uniform sky illumination × terrain exposure. Canopy shading and emission are
    // deferred, so a glowcap is invisible. Valid when the pose resolves to a support.
    if let Some(lm) = module_opt(manifest, "Light") {
        if let Some((cx, cz)) = a.pose.column(view.config.voxel_m, view.config.depth) {
            let wx = cx.rem_euclid(i64::from(view.config.width));
            if view.is_support(wx, standing_y, cz) {
                // A static episode's senses memo the reading per face (frozen terrain).
                let sky = match senses.and_then(|s| s.peek_sky(view, wx, standing_y, cz)) {
                    Some(Some(held)) => held,
                    // Held but not computed yet: compute it, and remember it afterwards.
                    Some(None) => {
                        let v = view.sky_visibility(wx, standing_y, cz);
                        writes.sky = Some((wx, standing_y, cz, v));
                        v
                    }
                    None => view.sky_visibility(wx, standing_y, cz),
                };
                obs[lm.offset] = clamp01(sky);
                obs[lm.offset + 1] = 1.0;
            }
        }
    }

    // Cone(3, foliage/body): the fixed ray fan, a fresh reading each observation. No
    // memory, no expansion, no body identity. Geometry alone, so it reads with or without
    // a senses handle.
    if let Some(cn) = module_opt(manifest, "Cone(3, foliage/body)") {
        let occupancy = &cone_occupancy.expect("browser observations prepare cone occupancy");
        let fruit = manifest.cone_counts_fruit.then_some(fv);
        let cone = crate::senses::cone_readings_in(
            view, fruit, occupancy, a.id, &a.pose, standing_y, manifest, &body,
        );
        let base = cn.offset;
        for (k, sec) in cone.sectors.iter().enumerate() {
            let o = base + k * 6;
            obs[o] = sec.clear;
            obs[o + 1] = sec.all_proximity;
            obs[o + 2] = sec.foliage_fraction;
            obs[o + 3] = sec.foliage_proximity;
            obs[o + 4] = sec.body_fraction;
            obs[o + 5] = sec.body_proximity;
        }
        obs[base + 18] = f64::from(cone.valid);
    }

    (obs, writes)
}

/// **A flyer's observation** (F1, predation-and-flight §3.6; [`Manifest::bellwing`]'s
/// layout): `Self` as every lineage's, plus `altitude` (the base's height over the support
/// face below, over `band_max_m`) and `ceiling` (the clear height over the base to the
/// first solid or water cell in its column, over `band_max_m`, capped at 1); `Air`
/// (perched, hanging, the clear height over its whole span, the rain rate over the
/// flight's `R₀`, grounded by the weather: a downpour or its rain latch); `Taste(1)`, the nearest
/// bloom's nectar in the mouth's reach; and the downward fan, sector by sector, with
/// `bloom` beside foliage and bodies. Without a prepared occupancy (a diagnostic) it
/// builds its own window.
fn flyer_observation(
    fauna: &Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    manifest: &Manifest,
    cone_occupancy: Option<crate::senses::Occ<'_>>,
    flight: &crate::Flight,
    full_observation: bool,
    blooms: Option<&crate::flight::Blooms>,
) -> Vec<f64> {
    let a = &fauna.animals[i];
    let phys = fauna.config.founder(a.founder.expect("a founder body"));
    let body = phys.body_at(a.body);
    let v = view.config.voxel_m;
    let mut obs = vec![0.0; manifest.inputs()];
    let shape = crate::flight::Shape::of(phys, flight, a.body);
    let tm = module(manifest, "Taste(1)");
    let reach = shape.r + body.mouth_reach_m;
    if let Some((_, stock)) =
        crate::flight::bloom_in_reach(view, fv, crate::flight::mouth_point(a, shape), reach)
    {
        let reference = phys.taste_reference_on(fauna.config.cue.emit_reference, view.config.voxel_m, fv.config);
        let c = if reference > 0.0 { (stock / reference).min(1.0) } else { 1.0 };
        let sat = manifest.tunings.chem_saturation;
        obs[tm.offset] = clamp01(c / (c + sat));
        obs[tm.offset + 1] = resistance_of(manifest, "nectar");
        obs[tm.offset + 2] = 1.0;
    }
    if !full_observation {
        return obs;
    }
    let fb = &a.founder_state.feedback;
    let k = phys.mass_scale;
    obs[0] = clamp01(a.energy / (manifest.adult_energy_reference * k));
    obs[1] = clamp01(a.reserve / (manifest.adult_reserve_reference * k));
    obs[2] = f64::from(birth_readiness(&fauna.config, a, fv));
    obs[3] = clamp01(fb.structural_loss / (manifest.structural_reference * k));
    obs[4] = clamp01(fb.intake / (manifest.body_reference * k));
    obs[5] = clamp01(fb.delivered_forward / manifest.forward_reference_m);
    obs[6] = clamp_sym(fb.delivered_turn / manifest.turn_reference_rad);
    obs[7] = if fb.attempted_equivalent > NONE_REQUESTED {
        clamp01(fb.delivered_equivalent / fb.attempted_equivalent)
    } else {
        1.0
    };
    let band = flight.band_max_m;
    if band > 0.0 {
        obs[8] = clamp01((a.pose.y_m - crate::surface_m(a.site.y, v)) / band);
        obs[9] = clamp01(crate::flight::clear_height(view, a.pose.x, a.pose.z, a.pose.y_m, band) / band);
    }

    let am = module(manifest, "Air");
    obs[am.offset] = f64::from(a.mobility.mode == crate::MobilityMode::Perched);
    obs[am.offset + 1] = f64::from(
        a.mobility.mode == crate::MobilityMode::Perched
            && a.mobility.perch.is_some_and(|p| p.kind == crate::PerchKind::Underside),
    );
    if band > 0.0 {
        obs[am.offset + 2] = clamp01(
            crate::flight::overhead_clear(view, shape, a.pose.x, a.pose.z, a.pose.y_m, band) / band,
        );
    }
    if flight.shower_rain_reference_m_per_s > 0.0 {
        obs[am.offset + 3] =
            clamp01(f64::from(view.weather.rain_m_per_s) / flight.shower_rain_reference_m_per_s);
    }
    // The weather grounds it: a downpour, or its rain latch (backlog §12).
    obs[am.offset + 4] = f64::from(
        view.weather.mode == cubarium_voxel::weather::RainMode::Downpour || a.mobility.rain_grounded,
    );

    if let Some(fm) = module_opt(manifest, "Fan(3, bloom/foliage/body)") {
        let owned;
        let occupancy = match cone_occupancy {
            Some(o) => o,
            None => {
                owned = crate::senses::cone_occupancy_window(
                    view,
                    fv,
                    &fauna.view(),
                    [a.pose.x],
                    manifest.cone_range_m,
                );
                crate::senses::Occ::Grid(&owned)
            }
        };
        let origin = crate::flight::eye(a, &body);
        let range = manifest.cone_range_m;
        let rays = (manifest.ray_yaw_offsets_deg.len() * manifest.ray_pitch_offsets_deg.len()) as f64;
        let owned_blooms;
        let blooms = match blooms {
            Some(blooms) => blooms,
            None => {
                owned_blooms = crate::flight::Blooms::near(view, fv, origin, range);
                &owned_blooms
            }
        };
        for (si, &centre) in manifest.sector_centres_deg.iter().enumerate().take(3) {
            let (mut clear, mut hit, mut hit_prox) = (0u32, 0u32, 0.0f64);
            let mut count = [0u32; 3];
            let mut prox = [0.0f64; 3];
            for &oyaw in manifest.ray_yaw_offsets_deg {
                for &opitch in manifest.ray_pitch_offsets_deg {
                    let dir = crate::senses::ray_direction(a.pose.heading_rad, centre, oyaw, opitch);
                    match crate::flight::fan_first_hit(view, fv, blooms, &occupancy, a.id, origin, dir, range) {
                        None => clear += 1,
                        Some((d, h)) => {
                            let p = (1.0 - d / range).clamp(0.0, 1.0);
                            hit += 1;
                            hit_prox += p;
                            let class = match h {
                                crate::ConeHit::Bloom => Some(0),
                                crate::ConeHit::FoliageCrown => Some(1),
                                crate::ConeHit::Body => Some(2),
                                _ => None,
                            };
                            if let Some(c) = class {
                                count[c] += 1;
                                prox[c] += p;
                            }
                        }
                    }
                }
            }
            let o = fm.offset + si * 8;
            obs[o] = f64::from(clear) / rays;
            obs[o + 1] = if hit > 0 { hit_prox / f64::from(hit) } else { 0.0 };
            for c in 0..3 {
                obs[o + 2 + 2 * c] = f64::from(count[c]) / rays;
                obs[o + 3 + 2 * c] = if count[c] > 0 {
                    prox[c] / f64::from(count[c])
                } else {
                    0.0
                };
            }
        }
        obs[fm.offset + 24] = 1.0;
    }
    obs
}

/// **`Self.birth_readiness`** (contract v2; [`crate::manifest::BIRTH_READINESS_RULE`]):
/// whether this tick's reproduction step would act on `animal` — start a gestation or
/// lay a clutch — read off the rules the step applies, from the pre-action state.
///
/// It is 1 only when the body is **eligible** (structure at `birth_body`, reserve at
/// the offspring package plus `surplus_floor`, a package that costs something), **not in
/// its refractory**, **not already gestating**, the **surplus hold elapses this tick**
/// (the counter the step is about to advance reaches `hold_ticks`), and — for an
/// egg-layer — the face it stands on **holds litter**. Schema 1 read 1 for any body at
/// `birth_body` holding `birth_cost`, which was true through the whole hold, the whole
/// refractory and every tick of a gestation.
///
/// A gestating body's step opens the escrow on the very tick the hold elapses, so for a
/// browser this is a one-tick pulse; an egg-layer holding its surplus off litter reads 1
/// on every tick until it stands on some.
pub fn birth_readiness(
    config: &crate::FaunaConfig,
    animal: &Animal,
    flora: &FloraView<'_>,
) -> bool {
    let sc = effective_config(config, animal);
    let rule = sc.reproduction;
    let state = &animal.reproduction;
    if state.refractory_ticks > 0 || state.escrow.is_some() {
        return false;
    }
    let cost = rule.package_cost(&sc);
    let eligible =
        cost > 0.0 && animal.body >= sc.birth_body && animal.reserve >= rule.surplus_reserve(&sc);
    if !eligible || state.surplus_ticks + 1 < rule.hold_required() {
        return false;
    }
    match rule.mode {
        crate::BirthMode::Gestation => true,
        crate::BirthMode::Eggs => {
            animal.founder.is_some()
                && (rule.dormant || flora.ground_at(animal.site).is_some_and(|g| g.litter > 0.0))
        }
    }
}

/// [`module`] without the panic: `None` when this manifest has no such module.
fn module_opt(manifest: &Manifest, name: &str) -> Option<crate::manifest::Module> {
    manifest.modules.iter().find(|m| m.name == name).copied()
}

fn module(manifest: &Manifest, name: &str) -> crate::manifest::Module {
    *manifest
        .modules
        .iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("manifest has no {name} module"))
}

fn clamp01(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn clamp_sym(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

/// The physiology an animal actually runs: its own founder's when it carries the marker,
/// its species' placeholder otherwise.
pub fn effective_config(config: &crate::FaunaConfig, animal: &Animal) -> SpeciesConfig {
    match animal.founder {
        Some(founder) => config.founder(founder).core,
        None => *config.species(animal.species),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Founder;
    use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
    use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig};

    /// A small flat world: 8 × 6 × 6 voxels at 0.25 m, soil 1..=2, so the ground is a
    /// support face at y = 2 and bodies stand in layer 3.
    fn flat_world() -> World {
        let mut world = World::empty(VoxelConfig {
            width: 8,
            height: 6,
            depth: 6,
            voxel_m: 0.25,
            ..VoxelConfig::default()
        });
        for z in 0..6 {
            for x in 0..8 {
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
        world
    }

    fn site(x: u32, z: u32) -> Site {
        Site { x, y: 2, z }
    }

    fn pose_at(x_m: f64, z_m: f64, heading: f64) -> crate::Pose {
        crate::Pose {
            x: x_m,
            z: z_m,
            heading_rad: heading,
            y_m: 0.0,
        }
    }

    const H: f64 = std::f64::consts::FRAC_PI_2; // heading east (+x)

    /// One lineage's adult geometry, the body every fixture below places.
    pub(super) fn adult(founder: Founder) -> Body {
        crate::FounderPhysiology::placeholder(founder).adult_body()
    }

    /// Full cruise over one controller period (five ticks of held action) is exactly
    /// the manifest's forward reference, and the pose follows the heading it was given
    /// — including a wrap across the seam, which is an ordinary move and not a turn.
    #[test]
    fn full_cruise_moves_the_reference_distance_along_the_heading() {
        let world = flat_world();
        let view = world.view();
        let manifest = Founder::Blind.manifest();
        let body = adult(Founder::Blind);
        let mut standing = 2u32;
        let mut pose = pose_at(0.5, 0.5, H);
        let mut total = 0.0;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                &mut standing,
                &manifest,
                &body,
                0.05,
                StepLimits { up: 1, down: 1 },
                crate::LifeConfig::default().wall_friction,
                Actions {
                    forward: 1.0,
                    turn: 0.0,
                    feed: 0.0,
                },
            );
            total += motion.delivered_forward;
        }
        assert!(
            (total - manifest.forward_reference_m).abs() < 1e-12,
            "one period at full cruise = {}",
            total
        );
        assert!((pose.x - (0.5 + manifest.forward_reference_m)).abs() < 1e-12);
        // Wrap on x: starting just before the seam, the same period lands past it, one
        // forward reference on from the start less the ring's circumference.
        let ring_m = 8.0 * 0.25;
        let mut seam = pose_at(ring_m - 0.01, 0.5, H);
        for _ in 0..manifest.cadence_ticks() {
            resolve_motion(
                &view,
                &mut seam,
                &mut standing,
                &manifest,
                &body,
                0.05,
                StepLimits { up: 1, down: 1 },
                crate::LifeConfig::default().wall_friction,
                Actions {
                    forward: 1.0,
                    turn: 0.0,
                    feed: 0.0,
                },
            );
        }
        // Derived, not pinned: package L's 0.375 m shredder cruises 0.375 m/s (1 BL/s),
        // so the landing point moves with the body length.
        let landing = ring_m - 0.01 + manifest.forward_reference_m - ring_m;
        assert!(
            landing > 0.0 && (seam.x - landing).abs() < 1e-12,
            "the pose wrapped past the seam to {}, not {landing}: it turned or stopped",
            seam.x
        );
    }

    /// Turning while stopped is permitted and paid, and the yaw cap is real: one
    /// controller period's turn is exactly the turn reference.
    #[test]
    fn turning_while_stopped_moves_only_the_heading() {
        let world = flat_world();
        let view = world.view();
        let manifest = Founder::Blind.manifest();
        let body = adult(Founder::Blind);
        let mut standing = 2u32;
        let mut pose = pose_at(0.5, 0.5, 0.0);
        let mut total = 0.0;
        let mut paid = 0.0;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                &mut standing,
                &manifest,
                &body,
                0.05,
                StepLimits { up: 1, down: 1 },
                crate::LifeConfig::default().wall_friction,
                Actions {
                    forward: 0.0,
                    turn: 1.0,
                    feed: 0.0,
                },
            );
            total += motion.delivered_turn;
            paid += motion.delivered_equivalent;
        }
        assert!(
            (total - manifest.turn_reference_rad).abs() < 1e-12,
            "one period at the yaw cap = {}",
            total
        );
        assert!(
            (pose.heading_rad - manifest.turn_reference_rad).abs() < 1e-12,
            "the heading turned to {}",
            pose.heading_rad
        );
        assert_eq!(
            (pose.x, pose.z),
            (0.5, 0.5),
            "a stopped turn does not translate"
        );
        assert!(
            paid > 0.0,
            "the turn was paid in equivalent displacement: r·|yaw| over the period"
        );
    }

    /// A wall constrains the sweep: the delivered motion stops at the wall, the block
    /// is flagged, and the contact receptors report the wall on the right side.
    #[test]
    fn a_wall_constrains_motion_and_reports_on_the_contacted_side() {
        let mut world = flat_world();
        // A two-voxel wall on column x = 4, standing proud at y = 3 and y = 4 — over
        // the founder's one-voxel climb, so it is a wall and not a ledge.
        for y in 3..=4 {
            world.apply(WorldCommand::SetMaterial {
                x: 4,
                y,
                z: 3,
                material: Material::Soil,
            });
        }
        let view = world.view();
        let manifest = Founder::Blind.manifest();
        let body = adult(Founder::Blind);
        // Heading east at z row 3, close enough to the wall that the period would
        // cross into it: the disc's front 0.02 m short of the face (package L's
        // shredder is 0.125 m wide, so the old fixed 0.95 m start was already inside).
        let mut standing = 2u32;
        let mut pose = pose_at(
            4.0 * 0.25 - body.footprint_radius() - 0.02,
            3.0 * 0.25 + 0.125,
            H,
        );
        let mut blocked = false;
        let mut attempted = 0.0;
        let mut delivered = 0.0;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                &mut standing,
                &manifest,
                &body,
                0.05,
                StepLimits { up: 1, down: 1 },
                crate::LifeConfig::default().wall_friction,
                Actions {
                    forward: 1.0,
                    turn: 0.0,
                    feed: 0.0,
                },
            );
            blocked |= motion.blocked;
            attempted += motion.attempted_equivalent;
            delivered += motion.delivered_forward;
        }
        assert!(blocked, "the wall stopped the sweep within the period");
        assert!(
            delivered < attempted,
            "attempted {} against delivered {}",
            attempted,
            delivered
        );
        assert!(
            pose.x + body.footprint_radius() <= 4.0 * 0.25 + 1e-9,
            "the disc stopped at the wall's face, at x = {}",
            pose.x
        );

        // The wall is dead ahead: front contact, no side contact.
        let contacts = contact_readings(&view, &pose, 2, &body, 1);
        assert_eq!(contacts.front, 1.0);
        assert_eq!(contacts.left, 0.0);
        assert_eq!(contacts.right, 0.0);
        assert_eq!(contacts.underside, 1.0);
        assert_eq!(contacts.wet, 0.0, "dry is a valid zero");

        // Turn to face north (a −90° turn from east): the wall is now on the body's
        // right, and the contact moved with it — contact location follows the body,
        // not the world's axes.
        let mut turned = pose;
        turned.heading_rad =
            (turned.heading_rad - std::f64::consts::FRAC_PI_2).rem_euclid(std::f64::consts::TAU);
        let contacts = contact_readings(&view, &turned, 2, &body, 1);
        assert_eq!(
            contacts.right, 1.0,
            "the wall the body faces is its right after a −90° turn"
        );
        assert_eq!(contacts.front, 0.0);
    }

    /// A body needs **its own height** of void over the face it stands on
    /// (`design/handoffs/voxel-body-anchors-2026-09-22.md`), not the voxels its mouth
    /// lifts through: the 0.375 m browser asks for two cells at 0.25 m and
    /// three at 0.125 m, and the 0.125 m shredder asks for one at either. A void one
    /// cell shorter is not a place to stand and not a place to walk to.
    #[test]
    fn a_slot_shorter_than_the_body_is_not_a_place_to_stand() {
        let browser = adult(Founder::Browser);
        assert_eq!(browser.headroom_voxels(0.25), 2);
        assert_eq!(browser.headroom_voxels(0.125), 3);
        assert_eq!(
            adult(Founder::Blind).headroom_voxels(0.125),
            1,
            "a ground feeder asks for the voxel it stands in"
        );

        // A 0.125 m world: ground at y = 2, then a roof over two columns — x = 4 at
        // y = 6, leaving three cells of room, and x = 5 at y = 5, leaving two.
        let mut world = World::empty(VoxelConfig {
            width: 8,
            height: 8,
            depth: 6,
            voxel_m: 0.125,
            ..VoxelConfig::default()
        });
        for z in 0..6 {
            for x in 0..8 {
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
        for (x, roof) in [(4i64, 6u32), (5, 5)] {
            world.apply(WorldCommand::SetMaterial {
                x,
                y: roof,
                z: 2,
                material: Material::Soil,
            });
        }
        let view = world.view();
        let need = browser.headroom_voxels(view.config.voxel_m);
        assert_eq!(need, 3);
        assert!(
            has_headroom(&view, 4, 2, 2, need),
            "three cells under the roof is the browser's height"
        );
        assert!(
            !has_headroom(&view, 5, 2, 2, need),
            "one cell less is a slot, not a floor"
        );
        // The shredder is a third of the browser's height and the same slot is a corridor
        // for it: the clearance is the animal's, not the grid's.
        assert!(has_headroom(
            &view,
            5,
            2,
            2,
            adult(Founder::Blind).headroom_voxels(view.config.voxel_m)
        ));
    }

    /// A rise beyond the climb is a wall and a drop beyond it is a cliff edge: the
    /// founder's centre stays on its standing layer whatever the sweep asks for. The
    /// blind founder's climb is one voxel here, so the fixture is two.
    #[test]
    fn a_drop_and_a_higher_face_constrain_the_centre() {
        let mut world = flat_world();
        // A step up: column x = 5 carries two extra layers, so its support face is at
        // y = 4 — two voxels over the founder's one-voxel climb — and at y = 2 and
        // y = 3 it has no support at all.
        for y in 3..=4 {
            world.apply(WorldCommand::SetMaterial {
                x: 5,
                y,
                z: 2,
                material: Material::Soil,
            });
        }
        let view = world.view();
        let manifest = Founder::Blind.manifest();
        let body = adult(Founder::Blind);
        assert!(view.is_support(5, 4, 2));
        assert!(!view.is_support(5, 2, 2));
        assert!(!view.is_support(5, 3, 2));

        let mut pose = pose_at(5.0 * 0.25 - 0.06, 2.0 * 0.25 + 0.125, H);
        let mut blocked = false;
        let mut standing = 2u32;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                &mut standing,
                &manifest,
                &body,
                0.05,
                StepLimits { up: 1, down: 1 },
                crate::LifeConfig::default().wall_friction,
                Actions {
                    forward: 1.0,
                    turn: 0.0,
                    feed: 0.0,
                },
            );
            blocked |= motion.blocked;
        }
        assert!(blocked, "the higher face is not steppable");
        let contacts = contact_readings(&view, &pose, 2, &body, 1);
        assert_eq!(
            contacts.front, 1.0,
            "the step-up's wall is at the body layer: contact"
        );
        assert_eq!(contacts.underside, 1.0);

        // A drop: dig column x = 3 at row 4 out entirely, so there is no support face
        // within the founder's one-voxel climb — a pit the body must not walk into.
        for y in 1..=2 {
            world.apply(WorldCommand::SetMaterial {
                x: 3,
                y,
                z: 4,
                material: Material::Air,
            });
        }
        let view = world.view();
        let body = adult(Founder::Blind);
        assert!(!view.is_support(3, 2, 4));
        assert!(!view.is_support(3, 1, 4));
        let mut pose = pose_at(3.0 * 0.25 - 0.025, 4.0 * 0.25 + 0.125, H);
        let mut blocked = false;
        let mut standing = 2u32;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                &mut standing,
                &manifest,
                &body,
                0.05,
                StepLimits { up: 1, down: 1 },
                crate::LifeConfig::default().wall_friction,
                Actions {
                    forward: 1.0,
                    turn: 0.0,
                    feed: 0.0,
                },
            );
            blocked |= motion.blocked;
        }
        assert!(blocked, "the pit is not steppable either");
        let contacts = contact_readings(&view, &pose, 2, &body, 1);
        assert_eq!(
            contacts.front, 0.0,
            "a drop is open air at body level: the delivery ratio is what reports it"
        );
        assert_eq!(contacts.underside, 1.0);
    }

    /// Standing water deeper than the founder can wade stops the sweep before it.
    #[test]
    fn deep_water_stops_the_sweep() {
        let mut world = flat_world();
        // A pond on column x = 2, row 2: one voxel of free water on the face.
        let volume = 0.25 * 0.25 * 0.25;
        world.apply(WorldCommand::AddWater {
            x: 2,
            y: 3,
            z: 2,
            volume_m3: volume,
        });
        let view = world.view();
        assert!(
            view.water_depth_m(2, 2, 2) > 0.05,
            "the pond is {} deep",
            view.water_depth_m(2, 2, 2)
        );
        let manifest = Founder::Blind.manifest();
        let body = adult(Founder::Blind);
        let mut pose = pose_at(2.0 * 0.25 - 0.01, 2.0 * 0.25 + 0.125, H);
        let mut blocked = false;
        let mut standing = 2u32;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                &mut standing,
                &manifest,
                &body,
                0.05,
                StepLimits { up: 1, down: 1 },
                crate::LifeConfig::default().wall_friction,
                Actions {
                    forward: 1.0,
                    turn: 0.0,
                    feed: 0.0,
                },
            );
            blocked |= motion.blocked;
        }
        assert!(blocked);
        assert!(
            pose.x < 2.0 * 0.25,
            "the body never entered the pond column"
        );
    }

    /// A remote stock may create a local `Chem` field, but it cannot become Taste until
    /// the mouth physically reaches litter.
    #[test]
    fn blind_taste_reads_contact_stock_not_the_diffused_field() {
        let world = flat_world();
        let view = world.view();
        let mut flora = Flora::new(FloraConfig::default());
        flora.deposit(
            site(4, 2),
            Deposit {
                kind: DepositKind::Litter,
                organic: 0.2,
                mineral: 0.2 * 0.02,
                energy: 0.2 * 2.0,
            },
        );
        let mut senses = crate::Senses::new();
        let (updates, converged) = senses.settle(&view, &flora.view());
        assert!(converged, "the field settled in {updates} updates");
        let manifest = Founder::Blind.manifest();
        let body = adult(Founder::Blind);
        let pose = pose_at(2.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125, -H);
        let field = senses.sample_cue(&view, &pose, 2).expect("supported");
        assert!(
            field > 0.0,
            "the remote litter produces a local diffused cue"
        );
        let t = taste_reading(
            &flora.view(),
            &view,
            &pose,
            2,
            &manifest,
            &body,
            Founder::Blind,
        );
        assert_eq!(t.cue, 0.0, "remote litter is not mouth chemistry");
        assert!(t.valid, "bare ground is still an actual mouth contact");
        assert!((t.resistance - 0.5).abs() < 1e-12);

        flora.deposit(
            site(2, 2),
            Deposit {
                kind: DepositKind::Litter,
                organic: 0.2,
                mineral: 0.2 * 0.02,
                energy: 0.2 * 2.0,
            },
        );
        let t = taste_reading(
            &flora.view(),
            &view,
            &pose,
            2,
            &manifest,
            &body,
            Founder::Blind,
        );
        assert!(
            t.cue > 0.2,
            "contact with litter reads its stock: {}",
            t.cue
        );
        assert!((t.resistance - 0.2).abs() < 1e-12, "the litter resistance");
    }

    /// Taste is a contact sensor and remains available without the remote cue field.
    #[test]
    fn blind_taste_on_bare_ground_is_valid_zero() {
        let world = flat_world();
        let view = world.view();
        let flora = Flora::new(FloraConfig::default());
        let manifest = Founder::Blind.manifest();
        let body = adult(Founder::Blind);
        let pose = pose_at(2.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125, 0.0);
        let t = taste_reading(
            &flora.view(),
            &view,
            &pose,
            2,
            &manifest,
            &body,
            Founder::Blind,
        );
        assert_eq!(t.cue, 0.0);
        assert_eq!(t.resistance, 0.5);
        assert!(t.valid);
    }

    /// The browser's mouth is at body height: a stand's crown cell in the mouth region
    /// is a valid contact, and the same stand one voxel beyond the reach is not mouth
    /// input.
    #[test]
    fn browser_taste_is_invalid_until_a_crown_reaches_the_mouth() {
        let world = flat_world();
        let view = world.view();
        let mut flora = Flora::new(FloraConfig::default());
        // A springturf on column (2, 2): its crown cell is one voxel above the face —
        // exactly the standing body's layer.
        let wood = 0.5
            * flora
                .config()
                .species(cubarium_voxel_flora::Species::Springturf)
                .wood_max;
        assert!(flora.apply(
            &world,
            cubarium_voxel_flora::Command::Seed {
                x: 2,
                z: 2,
                species: cubarium_voxel_flora::Species::Springturf,
                wood,
            },
        ));
        let fv = flora.view();
        let manifest = Founder::Browser.manifest();
        let body = adult(Founder::Browser);

        // Standing in the turf itself: contact.
        let pose = pose_at(2.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125, 0.0);
        let t = taste_reading(&fv, &view, &pose, 2, &manifest, &body, Founder::Browser);
        assert!(
            t.valid,
            "the crown cell is at the body's own column and layer"
        );
        assert!(t.cue > 0.0 && (t.resistance - 0.3).abs() < 1e-12);

        // Standing one column west facing east: the crown is within the mouth reach.
        let near = pose_at(1.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125, H);
        let t = taste_reading(&fv, &view, &near, 2, &manifest, &body, Founder::Browser);
        assert!(t.valid, "the reach reaches the neighbouring crown cell");

        // Facing away, the crown is behind the mouth: no contact, invalid.
        let mut away = near;
        away.heading_rad = (H + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU);
        let t = taste_reading(&fv, &view, &away, 2, &manifest, &body, Founder::Browser);
        assert!(
            !t.valid,
            "a stock behind the mouth is unreachable and stays unmouthed"
        );
        assert_eq!(t.cue, 0.0);
    }

    /// A controller that records what it was shown and holds rest.
    struct Recorder(std::sync::Arc<std::sync::Mutex<Vec<Vec<f64>>>>);

    impl crate::Controller for Recorder {
        fn drive(&mut self, observation: &[f64]) -> crate::Response {
            self.0.lock().expect("log").push(observation.to_vec());
            crate::Response::Bounded(crate::Actions::REST)
        }

        fn reset(&mut self) {
            self.0.lock().expect("log").clear();
        }
    }

    /// "Initial intake/loss/motion feedback is zero", from a **depleted** start.
    ///
    /// The ticks between an introduction and the first sampling are not an interval the
    /// controller acted in, so channels 3..6 read zero and motor delivery reads 1 — even
    /// though a hungry founder has been paying upkeep out of its own structure since
    /// tick one.
    #[test]
    fn the_first_sample_reports_no_prior_interval_from_a_depleted_start() {
        for founder in [Founder::Blind, Founder::Browser] {
            let world = flat_world();
            // The minimal gut: an empty founding stomach (`s0` 0), so nothing digests
            // into the empty reserve and upkeep really comes out of structure.
            let mut config = crate::FaunaConfig::placeholder();
            config.founders[founder.index()].gut.s0 = 0.0;
            let mut fauna = Fauna::new(config);
            assert!(fauna.apply(
                &world,
                crate::Command::IntroduceFounder {
                    x: 2,
                    z: 2,
                    founder,
                    stores: crate::StartingStores::HUNGRY,
                    heading_rad: H,
                },
            ));
            let id = fauna.view().ledger.births - 1;
            let start_body = fauna.view().animal(id).expect("placed").body;
            let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            assert!(fauna.set_controller(id, Box::new(Recorder(log.clone()))));
            let mut flora = Flora::new(FloraConfig::default());
            let manifest = founder.manifest();
            for _ in 0..manifest.cadence_ticks() {
                fauna.step(&world, &mut flora);
            }
            let log = log.lock().expect("log");
            assert_eq!(log.len(), 1, "{founder:?}: exactly one sampling");
            let obs = &log[0];

            // The premise: with no reserve, those ticks' upkeep really did come out of
            // structure, so there was something for the old rule to leak.
            let a = *fauna.view().animal(id).expect("alive");
            assert_eq!(a.reserve, 0.0, "{founder:?}");
            assert!(
                a.body < start_body,
                "{founder:?}: upkeep must have eaten structure"
            );

            for c in 3..=6 {
                assert_eq!(
                    obs[c], 0.0,
                    "{founder:?}: channel {c} is not a prior interval"
                );
            }
            assert_eq!(
                obs[7], 1.0,
                "{founder:?}: nothing was requested, so delivery is 1"
            );
            // And the *stocks* are not zeroed: they are current state, and a hungry
            // founder's reserve channel says so.
            assert_eq!(obs[1], 0.0, "{founder:?}: an empty reserve reads empty");
            assert!(obs[0] > 0.0, "{founder:?}: it is still alive");
        }
    }

    /// The observation vector: real Self channels against the manifest's references,
    /// real contacts, and the sense spans left zero with validity 0.
    #[test]
    fn the_observation_is_real_body_state_with_p1c_spans_zero() {
        let mut world = flat_world();
        world.apply(WorldCommand::SetMaterial {
            x: 4,
            y: 3,
            z: 2,
            material: Material::Soil,
        });
        let mut fauna = Fauna::new(crate::FaunaConfig::placeholder());
        assert!(fauna.apply(
            &world,
            crate::Command::IntroduceFounder {
                x: 2,
                z: 2,
                founder: Founder::Blind,
                stores: crate::StartingStores::FULL,
                heading_rad: H,
            },
        ));
        let mut flora = Flora::new(FloraConfig::default());
        // Litter one column west of the body, behind it, so the settled field has a
        // gradient the cue channels can read and the mouth is not over it. (It was one
        // column east until package L: the 0.375 m shredder's mouth reaches that column.)
        flora.deposit(
            site(1, 2),
            Deposit {
                kind: DepositKind::Litter,
                organic: 0.2,
                mineral: 0.2 * 0.02,
                energy: 0.2 * 2.0,
            },
        );
        let view = world.view();
        let fv = flora.view();
        let manifest = Founder::Blind.manifest();
        let obs = observation(&fauna, 0, &view, &fv, &manifest, None, None);
        assert_eq!(obs.len(), 23);
        // Introduced at the adult reference: energy and reserve read 1. Birth readiness
        // is the step's own rule (contract v2): an adult in surplus that has held none
        // of it yet is not about to lay.
        assert!((obs[0] - 1.0).abs() < 1e-9, "energy {}", obs[0]);
        assert!((obs[1] - 1.0).abs() < 1e-9, "reserve {}", obs[1]);
        assert_eq!(obs[2], 0.0);
        // Initial feedback is zero, so no delivery was requested: motor delivery 1.
        assert_eq!(obs[3], 0.0);
        assert_eq!(obs[4], 0.0);
        assert_eq!(obs[5], 0.0);
        assert_eq!(obs[6], 0.0);
        assert_eq!(obs[7], 1.0);
        // The wall two columns east is beyond the footprint+reach: no contact.
        assert_eq!((obs[8], obs[9], obs[10]), (0.0, 0.0, 0.0));
        assert_eq!(obs[11], 1.0, "supported underside");
        assert_eq!(obs[12], 1.0, "contact validity");
        assert_eq!(obs[13], 0.0, "dry");
        assert_eq!(obs[14], 1.0, "wet validity");
        // The litter one column behind is outside the forward mouth reach: bare-ground
        // Taste is valid zero. Chem remains unavailable; Light reads the real sky.
        assert_eq!(obs[15], 0.0, "unreached litter is not taste");
        assert_eq!(obs[16], 0.5, "ground resistance");
        assert_eq!(obs[17], 1.0, "ground contact is valid");
        assert_eq!(&obs[18..21], &[0.0, 0.0, 0.0], "chem stays invalid");
        assert_eq!(obs[22], 1.0, "light validity");
        assert!(
            (0.0..=1.0).contains(&obs[21]),
            "light response is a sky fraction, got {}",
            obs[21]
        );

        // With the arena's field the blind observation also reads remote Chem. Taste
        // remains the independent contact-stock response.
        let mut senses = crate::Senses::new();
        let (updates, converged) = senses.settle(&view, &fv);
        assert!(converged, "settled in {updates}");
        let obs = observation(&fauna, 0, &view, &fv, &manifest, Some(&mut senses), None);
        assert_eq!(obs[20], 1.0, "chem validity");
        assert_eq!(obs[17], 1.0, "ground taste contact remains valid");
        assert_eq!(obs[15], 0.0, "the field does not leak into Taste");
        assert!(obs[18] > 0.0, "a cue in the field here, got {}", obs[18]);
        assert!((-1.0..=1.0).contains(&obs[19]), "trend in range");

        // The browser's observation is 37 wide and its Cone reads the real world: valid,
        // and the wall two columns east shades the front sector at the eye's layer.
        let mut browser = Fauna::new(crate::FaunaConfig::placeholder());
        assert!(browser.apply(
            &world,
            crate::Command::IntroduceFounder {
                x: 2,
                z: 2,
                founder: Founder::Browser,
                stores: crate::StartingStores::FULL,
                heading_rad: H,
            },
        ));
        let cone_occupancy = crate::senses::cone_occupancy(&view, &fv, &browser.view());
        let obs = observation(
            &browser,
            0,
            &view,
            &fv,
            &Founder::Browser.manifest(),
            None,
            Some((&cone_occupancy).into()),
        );
        assert_eq!(obs.len(), 37);
        assert_eq!(obs[36], 1.0, "the cone reads the real world");
        assert!(
            obs[24] < 1.0,
            "the front sector is occluded by the wall, clear = {}",
            obs[24]
        );
        assert!(obs[18..36].iter().all(|v| (-1.0..=1.0).contains(v)));
    }

    /// The organ-allocation rule: total structure for a core budget is core/(1−f), the
    /// sensor share is inside that total once, and the frozen table carries the plan's
    /// 5% and 10%.
    #[test]
    fn the_organ_allocation_counts_the_sensor_share_once() {
        let blind = FounderPhysiology::placeholder(Founder::Blind);
        let browser = FounderPhysiology::placeholder(Founder::Browser);
        assert_eq!(blind.organ_structure_fraction, 0.05);
        assert_eq!(browser.organ_structure_fraction, 0.10);
        let core = 0.011875;
        let total =
            FounderPhysiology::total_structure_for_core(core, blind.organ_structure_fraction);
        assert!((total - 0.0125).abs() < 1e-12);
        assert!((blind.sensor_structure(total) - 0.05 * total).abs() < 1e-15);
        // The rule does not create matter: sensor + core is the total, not more.
        assert!((blind.sensor_structure(total) + core - total).abs() < 1e-12);
    }

    /// The mouth band is a **length**, so the same animal selects the same physical
    /// layers on either grid: the adult browser's `1.33 × 0.375 = 0.49875` m
    /// ceiling is two 0.25 m cells and four 0.125 m cells, which are the same 0.5 m of
    /// air. The shredder's `1.33 × 0.125 = 0.16625` m ceiling is the standing layer
    /// alone on the 0.25 m grid and — being over one 0.125 m cell — the standing layer
    /// and the one above it on the 0.125 m grid: a band that is not a whole number of
    /// cells selects every cell it overlaps.
    #[test]
    fn the_mouth_band_selects_the_same_physical_layers_on_both_grids() {
        let browser = adult(Founder::Browser);
        let blind = adult(Founder::Blind);
        assert!((browser.mouth_ceiling_m - 0.49875).abs() < 1e-12);
        assert_eq!(mouth_crown_layers(2, &browser, 0.25), 3..=4);
        assert_eq!(mouth_crown_layers(2, &browser, 0.125), 3..=6);
        assert_eq!(mouth_crown_layers(2, &blind, 0.25), 3..=3);
        assert_eq!(mouth_crown_layers(2, &blind, 0.125), 3..=4);
        // A touch at exactly the ceiling contributes nothing: a body whose ceiling is
        // two whole cells reaches those cells and not the one starting there.
        let mut exact = browser;
        exact.mouth_ceiling_m = 0.5;
        assert_eq!(mouth_crown_layers(2, &exact, 0.25), 3..=4);
    }

    /// The counterfactual behind the band: the **same** fixture, the same mouth columns
    /// and the same two crowns, read with a 0.49875 m mouth ceiling and again with a
    /// 0.75 m one. The crown two 0.25 m voxels above the head layer is out of the
    /// adult's physical band, and the crown at the head layer is in it either way. The
    /// band is what decides, and the manifest's `mouth_reach_up_voxels` is not read at
    /// all.
    #[test]
    fn the_band_and_not_a_voxel_count_decides_what_the_mouth_takes() {
        use cubarium_voxel_flora::{Command as FloraCommand, Species as Plant};

        let world = flat_world();
        let view = world.view();
        // The subject is the **band**, so the plants are the one-disc lollipops this
        // test was written against: what an adult bloomcrown's basal rosette does under
        // the same band is `tests/plant_layers.rs`'s claim.
        // The crowns are the test's own: a bloomcrown three 0.25 m voxels tall, whose cell
        // is the body's head layer plus two, and a springturf one voxel tall at the head.
        let mut config = FloraConfig::default().one_layer_species();
        config.bloomcrown.crown_height_m = [0.75, 0.75];
        config.springturf.crown_height_m = [0.25, 0.25];
        let mut flora = Flora::new(config);
        let bloom = flora.config().bloomcrown.wood_max;
        let turf = flora.config().springturf.wood_max;
        for (x, species, wood) in [
            (2i64, Plant::Bloomcrown, bloom),
            (5, Plant::Springturf, turf),
        ] {
            assert!(flora.apply(
                &world,
                FloraCommand::Seed {
                    x,
                    z: 2,
                    species,
                    wood,
                },
            ));
        }
        let v = flora.config().voxel_m;
        assert_eq!(flora.config().bloomcrown.crown_voxels(bloom, v), 3);
        assert_eq!(flora.config().springturf.crown_voxels(turf, v), 1);
        let fv = flora.view();
        // The band is the test's own too: an adult-browser-sized 0.49875 m ceiling.
        let mut browser = adult(Founder::Browser);
        browser.mouth_ceiling_m = 0.49875;

        let cols_over = mouth_columns(&view, &pose_at(2.5 * 0.25, 2.5 * 0.25, 0.0), &browser);
        let cols_head = mouth_columns(&view, &pose_at(5.5 * 0.25, 2.5 * 0.25, 0.0), &browser);

        assert_eq!(
            mouth_foliage_stand(&fv, &view, &cols_over, 2, &browser, crate::Diet::only(crate::Food::Foliage)),
            None,
            "a crown cell starting 0.5 m over the face is above a 0.49875 m ceiling"
        );
        assert_eq!(
            mouth_foliage_stand(&fv, &view, &cols_head, 2, &browser, crate::Diet::only(crate::Food::Foliage))
                .map(|(s, _)| s),
            Some(site(5, 2)),
            "the crown at the head layer is inside the band"
        );

        // A taller animal, same fixture: the band rises with the body and the crown
        // over the head comes into reach. Nothing else about the mouth changed.
        let mut tall = browser;
        tall.mouth_ceiling_m = 0.75;
        assert_eq!(
            mouth_foliage_stand(&fv, &view, &cols_over, 2, &tall, crate::Diet::only(crate::Food::Foliage))
                .map(|(s, _)| s),
            Some(site(2, 2)),
            "a 0.75 m band reaches the crown two voxels up"
        );
        assert_eq!(
            mouth_foliage_stand(&fv, &view, &cols_head, 2, &tall, crate::Diet::only(crate::Food::Foliage))
                .map(|(s, _)| s),
            Some(site(5, 2)),
            "and still takes the one at its feet"
        );
    }
}

#[cfg(test)]
mod step_rule_tests {
    //! The founder step rule (`design/handoffs/voxel-founder-step-2026-09-22.md`).
    //!
    //! Written from the brief before the implementation: a lineage has a climb height
    //! in metres, converted to whole voxels at the consumer, and a sub-step may move
    //! the centre column to a support face within that many layers of the body's own,
    //! up or down. Contact reports only solids the body could **not** step onto.
    //!
    //! Every case here is a handful of ticks on an 8-column fixture.

    use super::tests::adult;
    use super::*;
    use crate::manifest::Founder;
    use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};

    /// A terraced strip: 16 × `height` × 6 at `voxel_m`, soil in `1..=base`, and the
    /// columns in `raised` carrying `rise` layers more. The support face of the low
    /// ground is `base`; of the terrace, `base + rise`.
    fn terrace(
        voxel_m: f64,
        height: u32,
        base: u32,
        raised: std::ops::Range<i64>,
        rise: u32,
    ) -> World {
        let mut world = World::empty(VoxelConfig {
            width: 16,
            height,
            depth: 6,
            voxel_m,
            ..VoxelConfig::default()
        });
        for z in 0..6 {
            for x in 0..16 {
                for y in 1..=base {
                    world.apply(WorldCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        for z in 0..6 {
            for x in raised.clone() {
                for y in base + 1..=base + rise {
                    world.apply(WorldCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        world
    }

    /// A pit: soil in `1..=base` everywhere, then the columns in `sunk` dug down by
    /// `drop` layers, so their support face is `base - drop`.
    fn pit(voxel_m: f64, height: u32, base: u32, sunk: std::ops::Range<i64>, drop: u32) -> World {
        let mut world = terrace(voxel_m, height, base, 0..0, 0);
        for z in 0..6 {
            for x in sunk.clone() {
                for y in base - drop + 1..=base {
                    world.apply(WorldCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Air,
                    });
                }
            }
        }
        world
    }

    /// Walk a body east at full cruise and report where it got to: the standing layer
    /// after each tick, in order, and the final pose. `ticks` stays small.
    #[allow(clippy::too_many_arguments)]
    fn walk_east(
        world: &World,
        manifest: &Manifest,
        body: &Body,
        climb: u32,
        start_x_m: f64,
        y0: u32,
        ticks: u32,
    ) -> (crate::Pose, Vec<u32>) {
        let view = world.view();
        let v = view.config.voxel_m;
        let mut pose = crate::Pose {
            x: start_x_m,
            z: 2.5 * v,
            heading_rad: std::f64::consts::FRAC_PI_2,
            y_m: 0.0,
        };
        let mut y = y0;
        let mut layers = Vec::new();
        for _ in 0..ticks {
            resolve_motion(
                &view,
                &mut pose,
                &mut y,
                manifest,
                body,
                0.05,
                StepLimits {
                    up: climb,
                    down: climb,
                },
                crate::LifeConfig::default().wall_friction,
                Actions {
                    forward: 1.0,
                    turn: 0.0,
                    feed: 0.0,
                },
            );
            layers.push(y);
        }
        (pose, layers)
    }

    /// The conversion rounds a physical step **down**: the browser's
    /// 0.375 m both ways is 3 voxels on the 0.125 m preset and 1 on the
    /// 0.25 m presets; the shredder's 0.25 m is two voxels on the fine grid and one on
    /// the coarse one.
    #[test]
    fn the_step_converts_to_whole_voxels_rounding_down() {
        let browser = crate::FounderPhysiology::placeholder(Founder::Browser);
        let shredder = crate::FounderPhysiology::placeholder(Founder::Blind);
        assert_eq!((browser.step_up_m, browser.step_down_m), (0.375, 0.375));
        assert_eq!((shredder.step_up_m, shredder.step_down_m), (0.25, 0.25));
        let l = |p: &crate::FounderPhysiology, v| {
            let s = p.step_limits(v);
            (s.up, s.down)
        };
        assert_eq!(l(&browser, 0.125), (3, 3));
        assert_eq!(l(&browser, 0.25), (1, 1));
        assert_eq!(l(&shredder, 0.125), (2, 2));
        assert_eq!(l(&shredder, 0.25), (1, 1));
    }

    /// One voxel up onto a terrace and one voxel back down off it, on the 0.25 m
    /// reference grid, for a browser whose 0.25 m climb is one voxel there. The
    /// standing layer follows the ground under the body.
    #[test]
    fn a_browser_steps_up_and_down_one_voxel_on_the_reference_grid() {
        let world = terrace(0.25, 10, 2, 5..8, 1);
        let manifest = Founder::Browser.manifest();
        let body = adult(Founder::Browser);
        let (pose, layers) = walk_east(&world, &manifest, &body, 1, 3.5 * 0.25, 2, 160);
        assert!(
            layers.contains(&3),
            "the body never got onto the terrace: {layers:?}"
        );
        assert_eq!(
            *layers.last().unwrap(),
            2,
            "and never got off it again: {layers:?}"
        );
        assert!(
            pose.x > 8.0 * 0.25,
            "the body is past the terrace at {}",
            pose.x
        );
    }

    /// The shredder's 0.25 m step is two voxels on the 0.125 m grid and one on the
    /// 0.25 m grid: a one-voxel terrace is a step on both, and a 0.5 m (two coarse
    /// voxels) terrace is a wall the sweep refuses (the founder path climbs it instead:
    /// `tests/mobility.rs`).
    #[test]
    fn a_shredder_steps_a_quarter_metre_and_meets_half_a_metre_as_a_wall() {
        let phys = crate::FounderPhysiology::placeholder(Founder::Blind);
        for (v, rise, steps) in [(0.125, 2, true), (0.25, 1, true), (0.25, 2, false)] {
            let world = terrace(v, 12, 2, 5..8, rise);
            let manifest = Founder::Blind.manifest();
            let body = adult(Founder::Blind);
            let up = phys.step_limits(v).up;
            let (_, layers) = walk_east(&world, &manifest, &body, up, 3.5 * v, 2, 60);
            assert_eq!(
                layers.contains(&(2 + rise)),
                steps,
                "a {rise}-voxel terrace at {v} m: {layers:?}"
            );
        }
    }

    /// Two voxels is over the browser's climb on the 0.25 m grid (0.5 m of rise
    /// against a 0.25 m climb) and exactly at it on the 0.125 m grid. The same animal,
    /// the same physical rise of 0.25 m, refused on one grid and taken on the other is
    /// the discretisation working, not the rule changing.
    #[test]
    fn a_two_voxel_rise_refuses_on_the_reference_grid_and_passes_on_the_fine_one() {
        let manifest = Founder::Browser.manifest();
        let body = adult(Founder::Browser);

        let coarse = terrace(0.25, 10, 2, 5..8, 2);
        let (pose, layers) = walk_east(&coarse, &manifest, &body, 1, 3.5 * 0.25, 2, 40);
        assert!(
            layers.iter().all(|&y| y == 2),
            "a 0.5 m rise is a wall for a 0.25 m climb: {layers:?}"
        );
        assert!(
            pose.x < 5.0 * 0.25,
            "the body stopped at the wall, not on it: {}",
            pose.x
        );

        let fine = terrace(0.125, 14, 4, 5..8, 2);
        let (_, layers) = walk_east(&fine, &manifest, &body, 2, 3.5 * 0.125, 4, 60);
        assert!(
            layers.contains(&6),
            "the same 0.25 m rise is two voxels of climb here: {layers:?}"
        );
    }

    /// A drop deeper than the climb is a cliff edge: the body stops on it and does not
    /// fall. A drop within the climb is a step (the terrace test's descent).
    #[test]
    fn a_drop_beyond_the_climb_refuses() {
        let manifest = Founder::Browser.manifest();
        let body = adult(Founder::Browser);
        let world = pit(0.25, 10, 4, 5..9, 2);
        let (pose, layers) = walk_east(&world, &manifest, &body, 1, 3.5 * 0.25, 4, 40);
        assert!(
            layers.iter().all(|&y| y == 4),
            "the body walked off a 0.5 m cliff: {layers:?}"
        );
        assert!(
            pose.x < 5.0 * 0.25,
            "the body should stand on the rim at {}",
            pose.x
        );

        // One voxel of the same pit is a step down.
        let shallow = pit(0.25, 10, 4, 5..9, 1);
        let (_, layers) = walk_east(&shallow, &manifest, &body, 1, 3.5 * 0.25, 4, 40);
        assert!(
            layers.contains(&3),
            "a 0.25 m drop is within the climb: {layers:?}"
        );
    }

    /// Contact is about walls, not about ground: a ledge the body could step onto
    /// reads as open, a ledge it could not reads as solid. The channel count does not
    /// change and the underside still reads the face the body is standing on.
    #[test]
    fn contact_reads_a_steppable_ledge_as_clear_and_a_taller_one_as_solid() {
        let body = adult(Founder::Browser);
        // Standing in column 4, facing east, pressed up against the terrace at x = 5.
        let pose = crate::Pose {
            x: 5.0 * 0.25 - 0.0624,
            z: 2.5 * 0.25,
            heading_rad: std::f64::consts::FRAC_PI_2,
            y_m: 0.0,
        };

        let ledge = terrace(0.25, 10, 2, 5..8, 1);
        let reading = contact_readings(&ledge.view(), &pose, 2, &body, 1);
        assert_eq!(
            reading.front, 0.0,
            "a one-voxel ledge within the climb is ground, not a wall"
        );
        assert_eq!(reading.underside, 1.0);
        assert!(reading.resolved);

        let wall = terrace(0.25, 10, 2, 5..8, 2);
        let reading = contact_readings(&wall.view(), &pose, 2, &body, 1);
        assert_eq!(
            reading.front, 1.0,
            "a two-voxel rise is over the climb: a wall, as before"
        );
        assert_eq!(reading.underside, 1.0);

        // A climb of zero is the old receptor exactly: the ledge is a wall again.
        let reading = contact_readings(&ledge.view(), &pose, 2, &body, 0);
        assert_eq!(reading.front, 1.0);
    }
}

/// **A lineage's senses** (SENSES-CORE, S1 of the senses pass; VEILHOUND-SEARCH before it):
/// what the perception core ([`crate::perception`]) senses for it. Every number is a knob
/// (`fauna.founders.<lineage>.senses.*`), set from the senses review's §1
/// (`design/7_Research/senses-review-2026-10-06.md`, which keeps Astra's ranges where it
/// agrees), the same in every preset. 0 turns a range off.
///
/// - **Sight**: a body within `sight_m` by day (`sight_night_m` below the day line), at
///   `sight_still_fraction` of that for a still body, inside `sight_half_angle_deg` of its
///   heading, on a terrain ray from its eye to the body's middle that terrain does not block.
///   3D distance.
/// - **Hearing**: a body within `hearing_m` whose received loudness — its emission
///   `speed^a × length^b` ([`crate::PerceptionConfig`]'s exponents) over the 3D distance
///   (at least `hearing_near_m`), times the occlusion factor when terrain blocks the ray —
///   is at least `hearing_min`. All around.
/// - **Tremor**: a grounded body within `tremor_m` whose footfall at it (the tremor rule,
///   `mass × speed / d²`, times the occlusion factor through terrain) is at least
///   `tremor_min`. All around.
/// - **Smell**: the scent field ([`crate::scent`]), never a body: it reads a channel's level
///   and gradient where it stands, and smells a channel at `smell_min` or above.
///   `scent` is the channel its own body lays.
///
/// **`live`** says whether its heuristics read the core's record. Off (every lineage but the
/// Veilhound in S1), its record is filled by the interim rules its reflexes ran on before the
/// senses pass ([`crate::perception::refresh`]): these values are inert until the packages
/// that give it behaviour (S2 ground responses, S3 light and flight) turn it on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SenseTable {
    /// The core fills its record (a switch, no registry row).
    pub live: bool,
    /// Sight's range by day for a moving body, metres; 0 is blind.
    pub sight_m: f64,
    /// Sight's range below the day line, metres.
    pub sight_night_m: f64,
    /// A still body is seen within this fraction of the range.
    pub sight_still_fraction: f64,
    /// Sight's half-angle from the heading, degrees (180 all around).
    pub sight_half_angle_deg: f64,
    /// Hearing's range, metres; 0 is deaf.
    pub hearing_m: f64,
    /// The faintest received loudness heard (emission / distance, m/s at the default
    /// exponents).
    pub hearing_min: f64,
    /// Tremor's range, metres; 0 feels none.
    pub tremor_m: f64,
    /// The faintest footfall felt, in the tremor's units (organic × m/s / m²).
    pub tremor_min: f64,
    /// The faintest scent level smelled, in the field's units; 0 smells nothing.
    pub smell_min: f64,
    /// The scent channel its body lays ([`crate::ScentChannel`]); `None` lays none (a switch,
    /// no registry row).
    pub scent: Option<crate::ScentChannel>,
    /// **Light** (S3, [`crate::light`]): the photoreceptor's range for a beacon, metres (3D);
    /// 0 has none.
    pub light_m: f64,
    /// The faintest beacon level it reads ([`crate::LightConfig`]'s contrast × falloff).
    pub light_min: f64,
    /// It sees the flowers' near-IR (the bellwing), not only the cyan of glowcaps and lures (a
    /// switch, no registry row).
    #[serde(alias = "uv")]
    pub nir: bool,
    /// **Cyan only up close** (FLOWER-FRUIT, Wrysk 2026-10-09): a still light of the cyan
    /// class to its eye (a glowcap; a lure to an eye without near-IR) draws it only within
    /// this, metres (3D); farther it is passed by. 0: `light_m` alone bounds it.
    pub cyan_m: f64,
    /// **Looming**: a moving body (any but its own lineage) within this, metres (3D), is a
    /// shadow falling on it; 0 reads none.
    pub loom_m: f64,
    /// **Head down** (S2): while it crops between scan pauses ([`crate::Vigil::head_down`])
    /// sight reaches this fraction of its range: the blind window. 1 is no change.
    pub feeding_sight_fraction: f64,
    /// At most this many of its own lineage are weighed per remote sensing, nearest first
    /// (S2: a herd hears its neighbours bolt). 1.
    pub kin_tracks: u32,
    /// **Still, it senses farther** (RHYTHMS-CADENCE, [`crate::rhythm`]): while the body itself
    /// has been still for `still_settle_s`, its sight and hearing and tremor ranges are this ×
    /// theirs and its hearing and tremor floors this ÷ theirs (saltatory search: a pause to look
    /// and listen detects more; a moving body's own footfall masks what it hears and feels).
    /// 1 is no change.
    pub still_gain: f64,
    /// Seconds the body has been still before [`SenseTable::still_gain`] applies.
    pub still_settle_s: f64,
}

impl Default for SenseTable {
    /// Senseless: every range off, read by no one.
    fn default() -> SenseTable {
        SenseTable {
            live: false,
            sight_m: 0.0,
            sight_night_m: 0.0,
            sight_still_fraction: 1.0,
            sight_half_angle_deg: 90.0,
            hearing_m: 0.0,
            hearing_min: 0.0,
            tremor_m: 0.0,
            tremor_min: 0.0,
            smell_min: 0.0,
            scent: None,
            light_m: 0.0,
            light_min: 0.0,
            nir: false,
            cyan_m: 0.0,
            loom_m: 0.0,
            feeding_sight_fraction: 1.0,
            kin_tracks: 1,
            still_gain: 1.0,
            still_settle_s: 0.0,
        }
    }
}

impl SenseTable {
    /// The longest of its pairwise ranges at this light, metres.
    pub fn reach_m(&self, day: bool) -> f64 {
        self.sight_range_m(day).max(self.hearing_m).max(self.tremor_m)
    }

    /// This table as a body that has been still for `still_ticks` senses
    /// ([`SenseTable::still_gain`]): itself while it moves or the gain is at most 1.
    pub fn sensing(&self, still_ticks: u64) -> SenseTable {
        let g = self.still_gain;
        if !(g.is_finite() && g > 1.0) {
            return *self;
        }
        let settle = if self.still_settle_s > 0.0 { cubarium_voxel::ticks_at_least_one(self.still_settle_s) } else { 1 };
        if still_ticks < settle {
            return *self;
        }
        SenseTable {
            sight_m: self.sight_m * g,
            sight_night_m: self.sight_night_m * g,
            hearing_m: self.hearing_m * g,
            hearing_min: self.hearing_min / g,
            tremor_m: self.tremor_m * g,
            tremor_min: self.tremor_min / g,
            ..*self
        }
    }

    /// Sight's range for a moving body at this light, metres.
    pub fn sight_range_m(&self, day: bool) -> f64 {
        if day { self.sight_m } else { self.sight_night_m }
    }
}

/// **Each lineage's senses** ([`SenseTable`]), from the senses review's §1 and Astra's per-species
/// values (`design/7_Research/astra-senses-2026-10-06.md`), at the terrarium's scale (a 32 m ring,
/// 6 m deep, 16 m tall). Hearing thresholds are set from the review's distances for a
/// representative source at cruise (1 body length a second), emission `speed × length`:
/// a walking Veilhound emits 1.4 × 1.4 = 1.96. Tremor's floor is the patrol's `tremor_turn`
/// (a walking ripple-snail at the tremor radius), for every lineage alike. Smell floors are
/// the levels the scent field holds at the review's distances from a still adult source at
/// steady state on open ground ([`crate::scent`]'s calibration note).
///
/// The Veilhound's table is **live** since S1; S2 turns on the frondgrazer's, the loftstrider's,
/// the seedporter's and the littershredder's ([`crate::vigil`]), S3 the bellwing's and the
/// capgnawer's (their light sense, [`crate::light`]); the others are read by no heuristic yet.
pub fn senses_of(founder: Founder) -> SenseTable {
    use crate::ScentChannel::{Grazer, Hunter, SmallPrey};
    use crate::scent::{SMELL_AT_2_5_M, SMELL_AT_2_M, SMELL_AT_3_5_M, SMELL_AT_6_M};
    // RHYTHMS-CADENCE: every lineage senses a quarter farther after half a second still (a
    // modest gain; Kramer & McLaughlin 2001 on intermittent locomotion), untuned.
    let t = SenseTable { still_gain: 1.25, still_settle_s: 0.5, ..SenseTable::default() };
    match founder {
        // Eyeless (dossier): substrate tremor ~1 m, 2 m for heavy steps; litter chemistry is
        // its own detritus field.
        Founder::Blind => SenseTable { live: true, tremor_m: 2.0, tremor_min: 0.003, scent: Some(SmallPrey), ..t },
        // Coarse silhouettes at 1 m by day and 0.5 m dim over 240°, motion over shape;
        // substrate vibration 1–1.5 m; fungal chemistry is its own field.
        // S3 (D2): live; its fungal chemistry leads (its forager's `Chem`), and a weak cyan
        // light sense (glowcaps, lures) to 1.5 m pulls a hungry one with no fungal scent near.
        Founder::Capgnawer => SenseTable {
            live: true,
            sight_m: 1.0,
            sight_night_m: 0.5,
            sight_still_fraction: 0.5,
            sight_half_angle_deg: 120.0,
            tremor_m: 1.5,
            tremor_min: 0.003,
            scent: Some(SmallPrey),
            light_m: 1.5,
            light_min: 0.05,
            ..t
        },
        // Shadow and looming at 0.5 m by day, 0.25 m dim over 180°; a large walker felt at
        // ~1 m; film chemistry is its own field.
        // Live (AMPHIBIOUS-SNAIL): its retraction reads the record.
        Founder::RippleSnail => SenseTable {
            live: true,
            sight_m: 0.5,
            sight_night_m: 0.25,
            sight_still_fraction: 0.5,
            sight_half_angle_deg: 90.0,
            tremor_m: 1.0,
            tremor_min: 0.003,
            scent: Some(SmallPrey),
            ..t
        },
        // Moving bodies at 8 m by day and 4 m at night, static at 5 and 2 (0.625); 300° from
        // its eye stalks; a walking Veilhound heard at 6 m, a running one to 8 m; predator
        // odour at 3–4 m; ~1 m of footfall.
        // S2: live; head down while cropping it sees a quarter of its range (Astra's "reduced
        // clarity while cropping"); it weighs its six nearest herd-mates (HERD-PULL: three
        // until 2026-10-09; its many eyes count up to four and its pull fades past four).
        Founder::Browser => SenseTable {
            live: true,
            feeding_sight_fraction: 0.25,
            kin_tracks: 6,
            sight_m: 8.0,
            sight_night_m: 4.0,
            sight_still_fraction: 0.625,
            sight_half_angle_deg: 150.0,
            hearing_m: 8.0,
            hearing_min: 1.96 / 6.0,
            tremor_m: 1.0,
            tremor_min: 0.003,
            smell_min: SMELL_AT_3_5_M,
            scent: Some(Grazer),
            ..t
        },
        // 360° from the eye ring: moving bodies at 12 m by day, 6 at night, static at 8 and
        // 4; large-body movement heard at 8 m, louder to 10; ~2 m of ground disturbance;
        // animal odour at 2–3 m.
        Founder::Loftstrider => SenseTable {
            live: true,
            sight_m: 12.0,
            sight_night_m: 6.0,
            sight_still_fraction: 8.0 / 12.0,
            sight_half_angle_deg: 180.0,
            hearing_m: 10.0,
            hearing_min: 1.96 / 8.0,
            tremor_m: 2.0,
            tremor_min: 0.003,
            smell_min: SMELL_AT_2_5_M,
            scent: Some(Grazer),
            ..t
        },
        // D3: no form vision. S3: live; one photoreceptor (near-IR and cyan) reads the brightest still
        // beacon to 4 m (Astra: flowers ~4 m by day, a lure 2–3 m against the dark) and a
        // looming shadow within 1 m; floral smell on the nectar channel to ~6 m (Astra 4–6);
        // perch vibration over ~0.5 m.
        Founder::Bellwing => SenseTable {
            live: true,
            tremor_m: 0.5,
            tremor_min: 0.003,
            smell_min: SMELL_AT_6_M,
            scent: Some(SmallPrey),
            light_m: 4.0,
            light_min: 0.05,
            nir: true,
            // FLOWER-FRUIT: a glowcap draws it within about a metre, not across a cave.
            cyan_m: 1.0,
            loom_m: 1.0,
            ..t
        },
        // The review raises it to the Veilhound's 16 m by day (3 m at night), the best still
        // recognition in the community, about 180° with a forward binocular centre; predator
        // movement heard around 6 m; branch movement ~0.5 m; odour ~2 m.
        Founder::Seedporter => SenseTable {
            live: true,
            sight_m: 16.0,
            sight_night_m: 3.0,
            sight_still_fraction: 1.0,
            sight_half_angle_deg: 90.0,
            hearing_m: 8.0,
            hearing_min: 1.96 / 6.0,
            tremor_m: 0.5,
            tremor_min: 0.003,
            smell_min: SMELL_AT_2_M,
            scent: Some(Grazer),
            ..t
        },
        // Moving small prey at 4 m by day and 3 at night, still at half; ~220°; local rustling
        // at 1–2 m (a walking littershredder, 0.375 × 0.375, at 1.5 m) and a Veilhound's
        // footfalls to 3 m; strong substrate tremor 2–3 m; live-prey odour 2–3 m (carrion
        // carries farther by its source).
        Founder::Lanternjaw => SenseTable {
            sight_m: 4.0,
            sight_night_m: 3.0,
            sight_still_fraction: 0.5,
            sight_half_angle_deg: 110.0,
            hearing_m: 3.0,
            hearing_min: 0.375 * 0.375 / 1.5,
            tremor_m: 3.0,
            tremor_min: 0.003,
            smell_min: SMELL_AT_2_5_M,
            scent: Some(Hunter),
            ..t
        },
        // The Veilhound (`chorister`), from wolves and lions at the terrarium's scale, the branch's
        // values (VEILHOUND-SEARCH) kept: sight 16 m by day over 240° (half the ring: it sees
        // across the map in the open, terraces still hide what lies below them), a still body as
        // far as a moving one; 9 m at night (Astra's 8–10; it stays a day hunter, D4); hearing
        // 10 m, the territory radius, of a frondgrazer moving at 0.05 m/s (the branch's
        // threshold for its main prey: 0.05 × 0.75 / 10); ~1 m of footfall; smell ~6 m.
        Founder::Chorister => SenseTable {
            live: true,
            sight_m: 16.0,
            sight_night_m: 9.0,
            sight_still_fraction: 1.0,
            sight_half_angle_deg: 120.0,
            hearing_m: 10.0,
            hearing_min: 0.05 * 0.75 / 10.0,
            tremor_m: 1.0,
            tremor_min: 0.003,
            smell_min: SMELL_AT_6_M,
            scent: Some(Hunter),
            ..t
        },
    }
}

/// **The coat factors, proposed from each body** (`design/animal-body-reimagining-2026-09-24.md`
/// and the roster briefs): how readily an awned grass seed holds to the coat. Plates and
/// shells shed burrs; bristles, folds and legs that wade through a sward catch them.
/// Wrysk judges these; they are knobs (`fauna.founders.<lineage>.burr_coat`).
pub fn burr_coat_of(founder: Founder) -> f64 {
    match founder {
        // The littershredder: a low segmented body under smooth plates, but six legs and
        // plate seams in the litter: a little.
        Founder::Blind => 0.3,
        // The frondgrazer: a sward grazer whose legs and belly brush the turf all day; the
        // saddle shield is smooth, the rest is not.
        Founder::Browser => 0.6,
        // The lanternjaw: a low scorpid of overlapping plates, still for long ambushes.
        Founder::Lanternjaw => 0.3,
        // The capgnawer: bristle tufts on its back, the catchiest coat on the roster.
        Founder::Capgnawer => 0.8,
        // The loftstrider: long legs wading through grass, the body far above it.
        Founder::Loftstrider => 0.4,
        // The ripple-snail: a shell and a wet mantle, and it lives in the water.
        Founder::RippleSnail => 0.1,
        // The bellwing: a small flyer; only when grounded (the burr pass reads that).
        Founder::Bellwing => 0.3,
        // The seedporter: skin membranes and grasping limbs, mostly up in the crowns.
        Founder::Seedporter => 0.5,
        // The Veilhound: a six-footed runner under back plates that crosses the open
        // hills at a run; legs and the call membrane's folds catch.
        Founder::Chorister => 0.5,
    }
}
