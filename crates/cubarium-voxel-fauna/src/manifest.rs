//! **Phase-one founder manifests** — the fixed interface a neural controller is authored
//! against.
//!
//! One manifest is one lineage's ordered sensory modules, its three local actions, the
//! cadence it is sampled at, and the physical reference scales those numbers are written
//! in. The module list and the actions **do not move inside a lineage**: adding or removing
//! an organ, a class channel or a sector makes a new founder configuration with a new
//! contract, exactly as `design/voxel-senses.md` says. This module is only the *shape*: it
//! carries no weights, no controller, and no tick. `design/voxel-senses-phase1-plan.md`'s
//! "Exact starting manifests" is the source of the two layouts below.
//!
//! # Two founders
//!
//! - [`Founder::Blind`] — the littershredder: 23 inputs, no eyes.
//! - [`Founder::Browser`] — the frondgrazer founder: 37 inputs, a three-sector
//!   `Cone(3, foliage/body)`.
//!
//! Both carry the same three actions. `Self`, `Contact(4)`, `Wet` and `Taste(1)` occupy the
//! same slots in both, so a policy over a shared prefix has the same meaning in each.
//!
//! # Contract v2 (2026-09-22)
//!
//! Schema 2 (`design/handoffs/voxel-retrain-2026-09-22.md`, D1–D4). The body's meaning
//! changed twice on 2026-09-22 — bodies in metres, then the step rule — without the digest
//! noticing, so the canonical text now carries the **physical anchors** a slot's meaning
//! depends on: the adult's length, width and height, the eye / mouth-band / reach /
//! contact fractions, the climb, wade and drown depths, the cruise (1 BL/s, D2), the
//! five pitches, the cue's name `Chem(detritus)`, the occlusion rule (D3, D4) with the
//! pool bulk density it reads, and the birth-readiness rule. The anchors are **read off
//! [`FounderPhysiology::frozen`](crate::FounderPhysiology::frozen)**, not restated, so the
//! contract and the body it describes are one set of numbers. Every centre trained
//! against schema 1 is refused.
//!
//! # Digest
//!
//! [`Manifest::digest`] is FNV-1a 64 over [`Manifest::canonical_text`], mirroring
//! `cubarium-core`'s `neural::schema_digest`. Two manifests that differ in any declared
//! name, index, channel, action bound, cadence or reference scale have different digests;
//! a policy authored against one is refused against the other. There is deliberately **no
//! pinned golden digest test** — the digest protects meaning, not world reproducibility
//! (`design/voxel-senses-phase1-tests.md`, "GRU shapes").
//!
//! # Parameter count
//!
//! [`Manifest::parameter_count`] is the `Gru32`-compatible count for the manifest's shapes:
//! `3·H·(I + H + 2) + O·(H + 1)` with `H = 32`, which is `cubarium-core`'s
//! `neural::gru::parameter_count`: the number the runtime must accept (5,571 and 6,915).

use std::fmt::Write;

use serde::{Deserialize, Serialize};

/// The fixed hidden width every phase-one founder's GRU uses.
pub const HIDDEN: usize = 32;

/// The schema-version token folded into every manifest's canonical text. A change to the
/// meaning of an existing slot bumps this and invalidates old digests.
pub const SCHEMA_VERSION: u32 = 2;

/// The occlusion rule a cone ray and a contact probe are marched under, as the canonical
/// text records it (D3, D4): terrain stops a ray; free water only below its surface
/// (`fill × cell height`); a ground pool only below its physical height
/// `organic / (cell area × bulk density)`; the strip's `z` edge is solid for rays and
/// probes; above the world is open sky. Changing any clause is a new contract.
pub const OCCLUSION_RULE: &str =
    "terrain;water<surface=fill*cell;pool<organic/(cell_area*density);z_edge=solid;sky=open";

/// How a cone ray is traversed, as the canonical text records it (cone-speed item 3): a
/// cell-exact voxel walk (Amanatides–Woo) that visits every cell the ray passes through,
/// a hit at the ray's entry into the struck cell, and water and pools entered at the
/// plane of their surface. It replaced a fixed quarter-voxel sub-step that could skip a
/// corner-clipped cell and reported half a sub-step short of its sample. Only a manifest
/// with a cone carries it.
pub const CONE_TRAVERSAL: &str = "dda:every_cell;hit=cell_entry;water,pool=surface_plane_entry";

/// What `Self.birth_readiness` reads 1 for, as the canonical text records it: the birth
/// step would act on this body this tick — eligible (structure at `birth_body`, reserve
/// at the package plus the floor), not in its refractory, not gestating, the surplus
/// hold elapsing this tick, and for an egg-layer a face holding litter.
pub const BIRTH_READINESS_RULE: &str =
    "eligible&!refractory&!gestating&hold_elapses_now&eggs_on_litter";

/// Which phase-one founder this is: a lineage identity, not a controller.
///
/// # Adding a founder (species as data, roster R0)
///
/// 1. **An arm here**: [`Founder::ALL`], [`Founder::index`], [`Founder::name`] (its
///    lineage name, which is also its model's and its coat's name), [`Founder::role`] and
///    [`Founder::manifest`].
/// 2. **A physiology default** in [`crate::FounderPhysiology::placeholder`]: the body and
///    its anchors, the diet list ([`crate::Diet`]), the mouth band
///    (`mouth_floor_fraction`, `mouth_ceiling_fraction`), the founding mode and site
///    ([`crate::Founding`], [`crate::FoundingSite`]) and `founding_count` **0**, so
///    training worlds are unchanged until a preset turns it on; and its column in every
///    hunter's prey list.
/// 3. **A heuristic** in [`crate::heuristic`], the one constructor per founder.
/// 4. **A model and a coat under its name**: the presenter looks a live body's model up
///    by lineage name among the baked founders and falls back to the specimens, and the
///    coat likewise, so a baked specimen draws without a rebake.
///
/// The step, the founding loop, the search and the presenter loop over [`Founder::ALL`]
/// and read the physiology; none has a per-lineage branch to add. The senses (the
/// manifests, the cone and taste channels) are still per lineage until contract v3 (P2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Founder {
    /// The blind detritus feeder (littershredder): `Self`, `Contact(4)`, `Wet`, `Taste(1)`,
    /// `Chem(detritus)` and `Light` — **23 inputs**.
    Blind,
    /// The sighted foliage browser (frondgrazer founder): the same shared prefix plus
    /// `Cone(3, foliage/body)` — **37 inputs**.
    Browser,
    /// The **lanternjaw**, the ambush carnivore (`design/predation-and-flight-2026-09-28.md`
    /// §2.12): the Browser's 37-input layout with a carrion taste, and a fourth action,
    /// `lure`. An interim contract so a policy file could load; P2's contract v3 replaces
    /// it. No trained centre exists: its heuristic drives it.
    Lanternjaw,
    /// The **capgnawer** (roster R1): the glowcap's grazer. The shredder's blind 23-input
    /// layout with its `Chem` reading a fungal field (saprotroph cap stock) in place of the
    /// detritus field. Diet `[cap_tissue, litter]`.
    Capgnawer,
    /// The **loftstrider** (roster R1): the tall browser, 2.2 × 2.2 × 3.4 m, feeding
    /// 0.8–3.5 m up. The browser's 37-input layout with a cone that counts only foliage
    /// inside its mouth band. Diet `[foliage]`, a sloppy feeder.
    Loftstrider,
}

impl Founder {
    pub const ALL: [Founder; 5] = [
        Founder::Blind,
        Founder::Browser,
        Founder::Lanternjaw,
        Founder::Capgnawer,
        Founder::Loftstrider,
    ];
    pub const COUNT: usize = Founder::ALL.len();

    /// Index into a per-founder array, in [`Founder::ALL`] order: stable, like the
    /// flora's and the fauna's own species index.
    pub fn index(self) -> usize {
        match self {
            Founder::Blind => 0,
            Founder::Browser => 1,
            Founder::Lanternjaw => 2,
            Founder::Capgnawer => 3,
            Founder::Loftstrider => 4,
        }
    }

    /// The lineage name: what a command line or report calls this founder.
    pub fn name(self) -> &'static str {
        match self {
            Founder::Blind => "littershredder",
            Founder::Browser => "frondgrazer",
            Founder::Lanternjaw => "lanternjaw",
            Founder::Capgnawer => "capgnawer",
            Founder::Loftstrider => "loftstrider",
        }
    }

    /// The short role word the plan uses.
    pub fn role(self) -> &'static str {
        match self {
            Founder::Blind => "blind",
            Founder::Browser => "browser",
            Founder::Lanternjaw => "hunter",
            Founder::Capgnawer => "fungivore",
            Founder::Loftstrider => "tall_browser",
        }
    }

    /// This founder's fixed manifest.
    pub fn manifest(self) -> Manifest {
        match self {
            Founder::Blind => Manifest::blind(),
            Founder::Browser => Manifest::browser(),
            Founder::Lanternjaw => Manifest::lanternjaw(),
            Founder::Capgnawer => Manifest::capgnawer(),
            Founder::Loftstrider => Manifest::loftstrider(),
        }
    }
}

/// How a raw network output in `[low, high]` is produced from an unbounded logit. The
/// adapter itself is identical in training and viewing; this only names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Transfer {
    /// `σ(logit)`, for `[0, 1]`.
    Sigmoid,
    /// `tanh(logit)`, for `[-1, 1]`.
    Tanh,
}

impl Transfer {
    pub fn name(self) -> &'static str {
        match self {
            Transfer::Sigmoid => "sigmoid",
            Transfer::Tanh => "tanh",
        }
    }
}

/// One ordered sensory module: a name, the input span it occupies, and the meaning of each
/// scalar in it. `width` includes the module's validity value(s) where the primitive has
/// them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Module {
    pub name: &'static str,
    pub offset: usize,
    pub width: usize,
    pub channels: &'static [&'static str],
}

impl Module {
    /// The last input slot this module occupies, inclusive.
    pub fn end(&self) -> usize {
        self.offset + self.width - 1
    }
}

/// One action, with its resolved bounds and the transfer that produces it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Action {
    pub name: &'static str,
    pub low: f64,
    pub high: f64,
    pub transfer: Transfer,
}

/// A module's encoding tunings: how a physical reading becomes a scalar. These are fixed
/// in a schema and never renormalized by an individual's current range
/// (`design/voxel-senses.md` §1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tunings {
    /// Contact channels are binary geometric contact, not an invented force.
    pub contact_binary: bool,
    /// `Chem` response is `C / (1 + C)` against this saturation reference.
    pub chem_saturation: f64,
    /// The smoothing time constant for `Chem` trend, seconds.
    pub chem_tau_s: f64,
    /// The fixed time scale a smoothed derivative is multiplied by before clamping.
    pub trend_scale_s: f64,
    /// The uniform sky-illumination reference the `Light` response is divided by.
    pub light_reference: f64,
    /// `clamp(1 − distance / proximity_range_m, 0, 1)` for a cone hit. The plan fixes the
    /// encoding reference at 2 m even if the query range is later shortened.
    pub proximity_range_m: f64,
}

/// One founder's fixed sensory/action interface.
#[derive(Clone, Copy, Debug)]
pub struct Manifest {
    pub schema_version: u32,
    pub founder: Founder,
    pub modules: &'static [Module],
    pub actions: &'static [Action],
    /// Action outputs below this magnitude are treated as zero (absolute value for turn).
    pub deadband: f64,
    /// Controller period, seconds. Physics remains at the simulation tick.
    pub controller_period_s: f64,
    /// The **adult** body in metres — length, width, height — as the contract records
    /// it (D1): the [`FounderPhysiology`](crate::FounderPhysiology)'s own adult
    /// dimensions. The live geometry is read from the physiology, scaled by
    /// `(body / body_max)^(1/3)`; these are the same numbers, written into the digest so a
    /// centre trained on one body is refused by another.
    pub body_length_m: f64,
    pub body_width_m: f64,
    pub body_height_m: f64,
    /// The anchors, as fractions of the living body (decisions §§1, 2, 6): the eye and the
    /// contact receptors over the standing surface as fractions of height, the mouth
    /// band `[0, mouth_ceiling_fraction × height]`. The mouth's horizontal reach is
    /// [`Manifest::mouth_reach_body_lengths`].
    pub eye_height_fraction: f64,
    pub mouth_ceiling_fraction: f64,
    pub contact_height_fraction: f64,
    /// The lineage's step up and step down, metres: what the step rule treats as a ledge
    /// rather than a wall or a cliff (the contact receptors read the step up), and
    /// whether it climbs terrain walls.
    pub step_up_m: f64,
    pub step_down_m: f64,
    pub climbs_walls: bool,
    /// Standing water the body walks into, and water it is under, as fractions of its
    /// current height, and the seconds under before it drowns.
    pub wade_height_fraction: f64,
    pub drown_height_fraction: f64,
    pub drown_after_s: f64,
    /// Organic-mass reference the founder's `Self` channels are normalized against.
    pub body_reference: f64,
    /// Fixed `Self` normalization references (plan, "Exact starting manifests"): an adult's
    /// energy and reserve, and the total structure the interval's structural loss is
    /// divided by. A slice of the founder's initial stock, not a population statistic.
    pub adult_energy_reference: f64,
    pub adult_reserve_reference: f64,
    pub structural_reference: f64,
    /// Cruise speed in metres per second: [`cruise_body_lengths_per_s`] × the adult
    /// length (D2).
    pub cruise_m_per_s: f64,
    /// Yaw cap in radians per second.
    pub yaw_cap_rad_per_s: f64,
    /// `resolved_forward` and `resolved_turn` are normalized against **reference ×
    /// controller interval** — one controller period of cruise or yaw — not the rate.
    pub forward_reference_m: f64,
    pub turn_reference_rad: f64,
    /// The mouth's horizontal reach ahead of the footprint, in body lengths: the
    /// physiology's `mouth_reach_length_fraction`.
    pub mouth_reach_body_lengths: f64,
    /// How many whole voxels above its own head layer the mouth could take food from
    /// under the rule of 2026-09-21 (`design/handoffs/voxel-browser-reach-2026-09-21.md`).
    ///
    /// **Not part of the contract and not read by the tick.** The mouth is the physical
    /// band `[0, mouth_ceiling_fraction × height]` ([`crate::Body::mouth_layers`];
    /// decisions §2), which the canonical text carries instead. The field survives only
    /// for the arena layouts' own reachability checks, which package 5's arena rebuild
    /// replaces; it is deliberately left out of [`Manifest::canonical_text`].
    pub mouth_reach_up_voxels: u32,
    /// Which cue the mouth's one taste channel means: `litter` for the blind feeder,
    /// `foliage` for the browser.
    pub taste_cue: &'static str,
    /// The fixed material-response resistance mapping the taste resistance channel encodes:
    /// one scalar per contact material class, in `0..=1`. Schema data, so it lives in the
    /// digest.
    pub taste_resistances: &'static [(&'static str, f64)],
    pub tunings: Tunings,
    /// The material classes the eye exposes, for a browser; empty for a blind founder.
    pub visible_classes: &'static [&'static str],
    /// Sector centres, degrees from forward; empty for a blind founder.
    pub sector_centres_deg: &'static [f64],
    /// Per-sector yaw ray offsets, degrees; empty for a blind founder.
    pub ray_yaw_offsets_deg: &'static [f64],
    /// Per-sector pitch ray offsets, degrees; empty for a blind founder.
    pub ray_pitch_offsets_deg: &'static [f64],
    /// Maximum cone ray range, metres; zero for a blind founder.
    pub cone_range_m: f64,
    /// The bulk density a ground pool's physical height is computed with,
    /// `organic / (cell area × density)`, organic units per cubic metre (D3). An authored
    /// placeholder (`design/backlog.md` §1); the cone reads the same constant.
    pub pool_bulk_density: f64,
    /// [`OCCLUSION_RULE`]: the occlusion the eye and the contact receptors are marched
    /// under.
    pub occlusion_rule: &'static str,
    /// [`BIRTH_READINESS_RULE`]: what `Self.birth_readiness` reads 1 for.
    pub birth_readiness_rule: &'static str,
    /// The org-unit **mass scale** the lineage's shipped physiology is built at
    /// (`FounderPhysiology::mass_scale`): a policy trained here is trained for bodies of
    /// this mass, and a world of another mass refuses it.
    pub mass_scale: f64,
    /// [`SELF_REFERENCE_RULE`] and [`TASTE_REFERENCE_RULE`]: how the `Self` and taste
    /// references scale with a body's mass.
    pub self_reference_rule: &'static str,
    pub taste_reference_rule: &'static str,
}

/// The `Self` energy, reserve, structural-loss and intake references are the manifest's
/// times the body's `mass_scale`: each reads a fraction of an adult of that mass.
pub const SELF_REFERENCE_RULE: &str = "self_refs_x_body_mass_scale";

/// Taste's reference stock is `M_EMIT × the body's mass_scale`, so a "full" taste is the
/// same share of the body's need at any mass.
pub const TASTE_REFERENCE_RULE: &str = "taste_ref_m_emit_x_body_mass_scale";

// ---------------------------------------------------------------- static layout

const SELF_CHANNELS: [&str; 8] = [
    "energy",
    "reserve",
    "birth_readiness",
    "structural_loss",
    "assimilated_intake",
    "resolved_forward",
    "resolved_turn",
    "motor_delivery",
];

const MODULE_SELF: Module = Module {
    name: "Self",
    offset: 0,
    width: 8,
    channels: &SELF_CHANNELS,
};

const CONTACT4_CHANNELS: [&str; 5] = ["front", "left", "right", "underside", "valid"];

const MODULE_CONTACT4: Module = Module {
    name: "Contact(4)",
    offset: 8,
    width: 5,
    channels: &CONTACT4_CHANNELS,
};

const WET_CHANNELS: [&str; 2] = ["response", "valid"];

const MODULE_WET: Module = Module {
    name: "Wet",
    offset: 13,
    width: 2,
    channels: &WET_CHANNELS,
};

const TASTE1_CHANNELS: [&str; 3] = ["cue", "resistance", "valid"];

const MODULE_TASTE1: Module = Module {
    name: "Taste(1)",
    offset: 15,
    width: 3,
    channels: &TASTE1_CHANNELS,
};

const CHEM_DETRITUS_CHANNELS: [&str; 3] = ["response", "trend", "valid"];

/// The shredder's one diffused cue channel: the **detritus** field, litter plus carrion
/// (decisions §3 as amended 2026-09-22; the field is [`crate::senses`]'s
/// `DetritusField`). Called `Chem(litter)` under schema 1, when the field carried litter
/// alone; renamed with contract v2. Name and canonical text only: the slot,
/// the width and the channel order are unchanged.
const MODULE_CHEM_DETRITUS: Module = Module {
    name: "Chem(detritus)",
    offset: 18,
    width: 3,
    channels: &CHEM_DETRITUS_CHANNELS,
};

const LIGHT_CHANNELS: [&str; 2] = ["response", "valid"];

const MODULE_LIGHT: Module = Module {
    name: "Light",
    offset: 21,
    width: 2,
    channels: &LIGHT_CHANNELS,
};

const CONE3_CHANNELS: [&str; 19] = [
    "s0.clear",
    "s0.all_proximity",
    "s0.foliage",
    "s0.foliage_proximity",
    "s0.body",
    "s0.body_proximity",
    "s1.clear",
    "s1.all_proximity",
    "s1.foliage",
    "s1.foliage_proximity",
    "s1.body",
    "s1.body_proximity",
    "s2.clear",
    "s2.all_proximity",
    "s2.foliage",
    "s2.foliage_proximity",
    "s2.body",
    "s2.body_proximity",
    "valid",
];

const MODULE_CONE3: Module = Module {
    name: "Cone(3, foliage/body)",
    offset: 18,
    width: 19,
    channels: &CONE3_CHANNELS,
};

const BLIND_MODULES: [Module; 6] = [
    MODULE_SELF,
    MODULE_CONTACT4,
    MODULE_WET,
    MODULE_TASTE1,
    MODULE_CHEM_DETRITUS,
    MODULE_LIGHT,
];
const BROWSER_MODULES: [Module; 5] = [
    MODULE_SELF,
    MODULE_CONTACT4,
    MODULE_WET,
    MODULE_TASTE1,
    MODULE_CONE3,
];

/// The three local actions, identical for both founders (`design/voxel-senses-phase1-plan.md`,
/// "Phase-one body and action contract"). Indices are the array order: 0 forward, 1 turn,
/// 2 feed. Zero movement is rest; turning while stopped is permitted and paid.
const ACTIONS: [Action; 3] = [
    Action {
        name: "forward_effort",
        low: 0.0,
        high: 1.0,
        transfer: Transfer::Sigmoid,
    },
    Action {
        name: "turn_effort",
        low: -1.0,
        high: 1.0,
        transfer: Transfer::Tanh,
    },
    Action {
        name: "feed_effort",
        low: 0.0,
        high: 1.0,
        transfer: Transfer::Sigmoid,
    },
];

/// The lanternjaw's four actions: the three above, and `lure_effort` (§2.2): its lure is
/// lit while it is held. Resolved by [`crate::resolve_actions`] for the first three; the
/// fourth reaches the body through [`crate::Controller::lure`] until P2's contract v3
/// carries it in the response.
const LANTERNJAW_ACTIONS: [Action; 4] = [
    ACTIONS[0],
    ACTIONS[1],
    ACTIONS[2],
    Action {
        name: "lure_effort",
        low: 0.0,
        high: 1.0,
        transfer: Transfer::Sigmoid,
    },
];

/// Fixed action deadband. `abs(turn)` is compared, so it is symmetric.
pub const ACTION_DEADBAND: f64 = 0.05;

/// Browser sector centres, degrees from forward.
pub const BROWSER_SECTOR_CENTRES_DEG: [f64; 3] = [-60.0, 0.0, 60.0];
/// Per-sector ray yaw offsets, degrees.
pub const BROWSER_RAY_YAW_OFFSETS_DEG: [f64; 3] = [-30.0, 0.0, 30.0];
/// Per-sector pitch ray offsets, degrees: the decided fan, −40..+40 (decisions §6), so
/// the eye sees basal food at its feet, a drop ahead, and a crown above. 45 rays; the
/// sector values are fractions of the ray count, so the channel count does not move.
pub const BROWSER_RAY_PITCH_OFFSETS_DEG: [f64; 5] = [-40.0, -20.0, 0.0, 20.0, 40.0];
/// The classes a browser's cone exposes. Terrain and wood still occlude.
pub const BROWSER_VISIBLE_CLASSES: [&str; 2] = ["foliage", "body"];

const BLIND_TUNINGS: Tunings = Tunings {
    contact_binary: true,
    chem_saturation: 1.0,
    chem_tau_s: 1.0,
    trend_scale_s: 1.0,
    light_reference: 1.0,
    proximity_range_m: 2.0,
};

const BROWSER_TUNINGS: Tunings = Tunings {
    contact_binary: true,
    chem_saturation: 1.0,
    chem_tau_s: 1.0,
    trend_scale_s: 1.0,
    light_reference: 1.0,
    proximity_range_m: 2.0,
};

/// The fixed material-response resistance mapping a probing littershredder mouth feels:
/// soft detritus, firmer ground.
///
/// The soft class is still called `litter` and still reads 0.2 — the mapping is inside
/// the manifest digest — but since decisions §3 it is the class of all three detritus
/// foods, because a mouth on a corpse or a cap is not on bare ground. Adding a class
/// would have moved the digest and refused the shipped centre.
const BLIND_TASTE_RESISTANCES: [(&str, f64); 2] = [("litter", 0.2), ("ground", 0.5)];

/// The lanternjaw's mouth mapping: soft carrion, firm ground.
const LANTERNJAW_TASTE_RESISTANCES: [(&str, f64); 2] = [("carrion", 0.2), ("ground", 0.5)];

/// The browser's mouth mapping: pliable foliage, stiff wood, firm ground.
const BROWSER_TASTE_RESISTANCES: [(&str, f64); 3] =
    [("foliage", 0.3), ("wood", 0.8), ("ground", 0.5)];

/// The controller period every founder is sampled at, seconds.
const CONTROLLER_PERIOD_S: f64 = 0.25;

/// The fields of a manifest that are the founder's **physical anchors**, read off its
/// frozen physiology (D1) so the contract and the body cannot drift apart.
struct Anchors {
    length: f64,
    width: f64,
    height: f64,
    eye: f64,
    mouth_ceiling: f64,
    contact: f64,
    reach: f64,
    step_up: f64,
    step_down: f64,
    walls: bool,
    wade: f64,
    drown: f64,
    drown_after: f64,
    cruise: f64,
    yaw: f64,
    mass: f64,
}

fn anchors(founder: Founder) -> Anchors {
    let p = crate::FounderPhysiology::frozen(founder);
    Anchors {
        length: p.adult_length_m,
        width: p.adult_width_m,
        height: p.adult_height_m,
        eye: p.eye_height_fraction,
        mouth_ceiling: p.mouth_ceiling_fraction,
        contact: p.contact_height_fraction,
        reach: p.mouth_reach_length_fraction,
        step_up: p.step_up_m,
        step_down: p.step_down_m,
        walls: p.climbs_walls,
        wade: p.wade_height_fraction,
        drown: p.drown_height_fraction,
        drown_after: p.drown_after_s,
        cruise: p.cruise_body_lengths_per_s * p.adult_length_m,
        yaw: p.yaw_cap_rad_per_s,
        mass: p.mass_scale,
    }
}

/// A lineage's cruise in body lengths per second, as the frozen physiology has it
/// ([`FounderPhysiology::cruise_body_lengths_per_s`](crate::FounderPhysiology)): 1 for the
/// two grazing lineages, 0.6 for the lanternjaw (§2.12), which wins by the lunge and not
/// the chase — slower than a fleeing grazer.
pub fn cruise_body_lengths_per_s(founder: Founder) -> f64 {
    crate::FounderPhysiology::frozen(founder).cruise_body_lengths_per_s
}

impl Manifest {
    /// The blind littershredder: 23 inputs, no eyes. The ladder's 0.375 m body (package
    /// L), cruising at one body length per second.
    // WX4: the manifests carry no phase input. An asleep body skips its controller, so a
    // heuristic or trained brain never sees a night it cannot act in; a phase channel
    // (the day's place, `World::day_phase`) is the next training round's addition, and it
    // is a new contract when it lands.
    pub fn blind() -> Manifest {
        let a = anchors(Founder::Blind);
        Manifest {
            schema_version: SCHEMA_VERSION,
            founder: Founder::Blind,
            modules: &BLIND_MODULES,
            actions: &ACTIONS,
            deadband: ACTION_DEADBAND,
            controller_period_s: CONTROLLER_PERIOD_S,
            body_length_m: a.length,
            body_width_m: a.width,
            body_height_m: a.height,
            eye_height_fraction: a.eye,
            mouth_ceiling_fraction: a.mouth_ceiling,
            contact_height_fraction: a.contact,
            step_up_m: a.step_up,
            step_down_m: a.step_down,
            climbs_walls: a.walls,
            wade_height_fraction: a.wade,
            drown_height_fraction: a.drown,
            drown_after_s: a.drown_after,
            body_reference: 0.0125,
            adult_energy_reference: 0.0375,
            adult_reserve_reference: 0.00625,
            structural_reference: 0.0125,
            cruise_m_per_s: a.cruise,
            yaw_cap_rad_per_s: a.yaw,
            forward_reference_m: a.cruise * CONTROLLER_PERIOD_S,
            turn_reference_rad: 0.5,
            mouth_reach_body_lengths: a.reach,
            mouth_reach_up_voxels: 0,
            taste_cue: "litter",
            taste_resistances: &BLIND_TASTE_RESISTANCES,
            tunings: BLIND_TUNINGS,
            visible_classes: &[],
            sector_centres_deg: &[],
            ray_yaw_offsets_deg: &[],
            ray_pitch_offsets_deg: &[],
            cone_range_m: 0.0,
            pool_bulk_density: crate::CueConfig::default().pool_bulk_density,
            occlusion_rule: OCCLUSION_RULE,
            birth_readiness_rule: BIRTH_READINESS_RULE,
            mass_scale: a.mass,
            self_reference_rule: SELF_REFERENCE_RULE,
            taste_reference_rule: TASTE_REFERENCE_RULE,
        }
    }

    /// The sighted browser: 37 inputs with a three-sector cone. The ladder's 0.75 m body,
    /// cruising at one body length per second; 2 m ray range and 45 rays.
    pub fn browser() -> Manifest {
        let a = anchors(Founder::Browser);
        Manifest {
            schema_version: SCHEMA_VERSION,
            founder: Founder::Browser,
            modules: &BROWSER_MODULES,
            actions: &ACTIONS,
            deadband: ACTION_DEADBAND,
            controller_period_s: CONTROLLER_PERIOD_S,
            body_length_m: a.length,
            body_width_m: a.width,
            body_height_m: a.height,
            eye_height_fraction: a.eye,
            mouth_ceiling_fraction: a.mouth_ceiling,
            contact_height_fraction: a.contact,
            step_up_m: a.step_up,
            step_down_m: a.step_down,
            climbs_walls: a.walls,
            wade_height_fraction: a.wade,
            drown_height_fraction: a.drown,
            drown_after_s: a.drown_after,
            body_reference: 0.05,
            adult_energy_reference: 0.15,
            adult_reserve_reference: 0.025,
            structural_reference: 0.05,
            cruise_m_per_s: a.cruise,
            yaw_cap_rad_per_s: a.yaw,
            forward_reference_m: a.cruise * CONTROLLER_PERIOD_S,
            turn_reference_rad: 0.5,
            mouth_reach_body_lengths: a.reach,
            mouth_reach_up_voxels: 1,
            taste_cue: "foliage",
            taste_resistances: &BROWSER_TASTE_RESISTANCES,
            tunings: BROWSER_TUNINGS,
            visible_classes: &BROWSER_VISIBLE_CLASSES,
            sector_centres_deg: &BROWSER_SECTOR_CENTRES_DEG,
            ray_yaw_offsets_deg: &BROWSER_RAY_YAW_OFFSETS_DEG,
            ray_pitch_offsets_deg: &BROWSER_RAY_PITCH_OFFSETS_DEG,
            cone_range_m: 2.0,
            pool_bulk_density: crate::CueConfig::default().pool_bulk_density,
            occlusion_rule: OCCLUSION_RULE,
            birth_readiness_rule: BIRTH_READINESS_RULE,
            mass_scale: a.mass,
            self_reference_rule: SELF_REFERENCE_RULE,
            taste_reference_rule: TASTE_REFERENCE_RULE,
        }
    }

    /// The lanternjaw (interim): the Browser's layout and cone, its own 1.0 × 0.25 ×
    /// 0.45 m body cruising at 0.6 BL/s, a carrion taste, and the `lure` action. P2's
    /// contract v3 replaces it.
    pub fn lanternjaw() -> Manifest {
        let a = anchors(Founder::Lanternjaw);
        Manifest {
            schema_version: SCHEMA_VERSION,
            founder: Founder::Lanternjaw,
            modules: &BROWSER_MODULES,
            actions: &LANTERNJAW_ACTIONS,
            deadband: ACTION_DEADBAND,
            controller_period_s: CONTROLLER_PERIOD_S,
            body_length_m: a.length,
            body_width_m: a.width,
            body_height_m: a.height,
            eye_height_fraction: a.eye,
            mouth_ceiling_fraction: a.mouth_ceiling,
            contact_height_fraction: a.contact,
            step_up_m: a.step_up,
            step_down_m: a.step_down,
            climbs_walls: a.walls,
            wade_height_fraction: a.wade,
            drown_height_fraction: a.drown,
            drown_after_s: a.drown_after,
            body_reference: 0.035,
            // `energy_density · (body_max + reserve_cap · body_max)`, the other two's rule.
            adult_energy_reference: 0.14,
            adult_reserve_reference: 0.035,
            structural_reference: 0.035,
            cruise_m_per_s: a.cruise,
            yaw_cap_rad_per_s: a.yaw,
            forward_reference_m: a.cruise * CONTROLLER_PERIOD_S,
            turn_reference_rad: 0.5,
            mouth_reach_body_lengths: a.reach,
            mouth_reach_up_voxels: 1,
            taste_cue: "carrion",
            taste_resistances: &LANTERNJAW_TASTE_RESISTANCES,
            tunings: BROWSER_TUNINGS,
            visible_classes: &BROWSER_VISIBLE_CLASSES,
            sector_centres_deg: &BROWSER_SECTOR_CENTRES_DEG,
            ray_yaw_offsets_deg: &BROWSER_RAY_YAW_OFFSETS_DEG,
            ray_pitch_offsets_deg: &BROWSER_RAY_PITCH_OFFSETS_DEG,
            cone_range_m: 2.0,
            pool_bulk_density: crate::CueConfig::default().pool_bulk_density,
            occlusion_rule: OCCLUSION_RULE,
            birth_readiness_rule: BIRTH_READINESS_RULE,
            mass_scale: a.mass,
            self_reference_rule: SELF_REFERENCE_RULE,
            taste_reference_rule: TASTE_REFERENCE_RULE,
        }
    }

    /// The capgnawer (roster R1): the blind layout, its `Chem` reading the fungal field.
    // R1 stub: the shredder's manifest under the capgnawer's name. The implementer gives
    // it its own module list (`Chem(fungal)` in the shredder's `Chem(detritus)` slot) and
    // its own anchors and references.
    pub fn capgnawer() -> Manifest {
        Manifest {
            founder: Founder::Capgnawer,
            ..Manifest::blind()
        }
    }

    /// The loftstrider (roster R1): the browser's layout, its cone banded to its mouth.
    // R1 stub: the browser's manifest under the loftstrider's name. The implementer gives
    // it its own anchors and references (and the band-limited cone).
    pub fn loftstrider() -> Manifest {
        Manifest {
            founder: Founder::Loftstrider,
            ..Manifest::browser()
        }
    }

    /// Input width, read off the module list: the last module's end plus one. Panics if
    /// the module list is not contiguous from zero — a programming error in this file.
    pub fn inputs(&self) -> usize {
        let mut next = 0;
        for m in self.modules {
            assert_eq!(m.offset, next, "module {} is not contiguous", m.name);
            assert_eq!(
                m.channels.len(),
                m.width,
                "module {} declares {} channels for a width of {}",
                m.name,
                m.channels.len(),
                m.width
            );
            next = m.end() + 1;
        }
        next
    }

    /// Total rays per controller observation: sectors × yaw offsets × pitch offsets.
    pub fn ray_count(&self) -> usize {
        self.sector_centres_deg.len()
            * self.ray_yaw_offsets_deg.len()
            * self.ray_pitch_offsets_deg.len()
    }

    /// The `Gru32`-compatible parameter count this manifest's shapes need:
    /// `3·H·(I + H + 2) + O·(H + 1)`.
    pub fn parameter_count(&self) -> usize {
        gru32_parameter_count(self.inputs(), self.actions.len())
    }

    /// The deterministic canonical text the digest is taken over. Every declared quantity
    /// that changes what a slot means is named here; coefficients that only change how good
    /// a policy is (costs, rates outside the schema) are not.
    pub fn canonical_text(&self) -> String {
        let mut s = String::new();
        let _ = write!(
            s,
            "voxel-senses|v{}|founder:{}|role:{}",
            self.schema_version,
            self.founder.name(),
            self.founder.role()
        );
        for m in self.modules {
            let _ = write!(s, "|mod:{}[{}..{}]", m.name, m.offset, m.end());
            for c in m.channels {
                let _ = write!(s, ":{c}");
            }
        }
        for a in self.actions {
            let _ = write!(
                s,
                "|act:{}[{},{}]:{}",
                a.name,
                a.low,
                a.high,
                a.transfer.name()
            );
        }
        let _ = write!(
            s,
            "|deadband:{}|cadence:{}|body_len:{}|body_width:{}|body_height:{}|body_ref:{}|cruise:{}|yaw:{}|mouth_bl:{}",
            self.deadband,
            self.controller_period_s,
            self.body_length_m,
            self.body_width_m,
            self.body_height_m,
            self.body_reference,
            self.cruise_m_per_s,
            self.yaw_cap_rad_per_s,
            self.mouth_reach_body_lengths
        );
        // The physical anchors (D1): where the eye, the mouth band and the receptors sit
        // on the living body, what the legs can step, and the water it wades and drowns in.
        let _ = write!(
            s,
            "|anchor:eye={}H,mouth=[0,{}H],contact={}H|step:up={},down={},walls={}|wade:{}H|drown:{}H/{}s",
            self.eye_height_fraction,
            self.mouth_ceiling_fraction,
            self.contact_height_fraction,
            self.step_up_m,
            self.step_down_m,
            self.climbs_walls,
            self.wade_height_fraction,
            self.drown_height_fraction,
            self.drown_after_s
        );
        let _ = write!(
            s,
            "|occlusion:{}|pool_density:{}|birth_ready:{}",
            self.occlusion_rule, self.pool_bulk_density, self.birth_readiness_rule
        );
        let _ = write!(
            s,
            "|self_ref:energy={},reserve={},structural={}|move_ref:forward={},turn={}",
            self.adult_energy_reference,
            self.adult_reserve_reference,
            self.structural_reference,
            self.forward_reference_m,
            self.turn_reference_rad
        );
        let _ = write!(
            s,
            "|mass:{}|self_ref:{}|taste_ref:{}",
            self.mass_scale, self.self_reference_rule, self.taste_reference_rule
        );
        let _ = write!(s, "|taste:{}", self.taste_cue);
        for (class, r) in self.taste_resistances {
            let _ = write!(s, ":{}={}", class, r);
        }
        let t = &self.tunings;
        let _ = write!(
            s,
            "|tune:contact_binary={},chem_sat={},chem_tau={},trend_scale={},light_ref={},proximity={}",
            t.contact_binary,
            t.chem_saturation,
            t.chem_tau_s,
            t.trend_scale_s,
            t.light_reference,
            t.proximity_range_m
        );
        if !self.sector_centres_deg.is_empty() {
            let _ = write!(s, "|visible:");
            for c in self.visible_classes {
                let _ = write!(s, "{c},");
            }
            let _ = write!(s, "|sectors:");
            for d in self.sector_centres_deg {
                let _ = write!(s, "{d},");
            }
            let _ = write!(s, "|ray_yaw:");
            for d in self.ray_yaw_offsets_deg {
                let _ = write!(s, "{d},");
            }
            let _ = write!(s, "|ray_pitch:");
            for d in self.ray_pitch_offsets_deg {
                let _ = write!(s, "{d},");
            }
            let _ = write!(s, "|range:{}", self.cone_range_m);
            let _ = write!(s, "|traversal:{CONE_TRAVERSAL}");
        } else {
            let _ = write!(s, "|sectors:none");
        }
        s
    }

    /// FNV-1a 64 over [`Manifest::canonical_text`]. A policy whose stored digest differs is
    /// refused rather than reinterpreted.
    pub fn digest(&self) -> u64 {
        fnv1a(self.canonical_text().as_bytes())
    }

    /// Controller period quantised to whole ticks at the simulation rate, never below one.
    pub fn cadence_ticks(&self) -> u64 {
        cubarium_voxel::ticks_at_least_one(self.controller_period_s)
    }
}

/// FNV-1a 64, field for field `cubarium-core`'s `neural::fnv1a`.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The `Gru32`-compatible parameter count for `I` inputs and `O` outputs, hidden width 32:
/// `3·32·(I + 32 + 2) + O·(32 + 1)`. The same arithmetic as
/// `cubarium_core::neural::gru::parameter_count`, written here so the manifest does not
/// depend on the neural crate.
pub const fn gru32_parameter_count(inputs: usize, actions: usize) -> usize {
    3 * HIDDEN * (inputs + HIDDEN + 2) + actions * (HIDDEN + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two exact scalar layouts the plan fixes. A wrong offset or width fails here
    /// before any runtime does.
    #[test]
    fn the_two_manifests_have_the_plans_exact_shapes() {
        let blind = Manifest::blind();
        assert_eq!(blind.inputs(), 23, "blind manifest width");
        assert_eq!(blind.modules.len(), 6);
        assert_eq!((MODULE_SELF.offset, MODULE_SELF.end()), (0, 7));
        assert_eq!((MODULE_CONTACT4.offset, MODULE_CONTACT4.end()), (8, 12));
        assert_eq!((MODULE_WET.offset, MODULE_WET.end()), (13, 14));
        assert_eq!((MODULE_TASTE1.offset, MODULE_TASTE1.end()), (15, 17));
        assert_eq!(
            (MODULE_CHEM_DETRITUS.offset, MODULE_CHEM_DETRITUS.end()),
            (18, 20)
        );
        assert_eq!((MODULE_LIGHT.offset, MODULE_LIGHT.end()), (21, 22));

        let browser = Manifest::browser();
        assert_eq!(browser.inputs(), 37, "browser manifest width");
        assert_eq!(browser.modules.len(), 5);
        assert_eq!((MODULE_CONE3.offset, MODULE_CONE3.end()), (18, 36));
        assert_eq!(browser.ray_count(), 45, "3 sectors x 3 yaws x 5 pitches");
        assert_eq!(blind.ray_count(), 0);
        // The plan's named taste cue per founder and its fixed resistance mapping.
        assert_eq!(blind.taste_cue, "litter");
        assert_eq!(browser.taste_cue, "foliage");
        assert!(blind.taste_resistances.iter().any(|(c, _)| *c == "litter"));
        assert!(
            browser
                .taste_resistances
                .iter()
                .any(|(c, _)| *c == "foliage")
        );
    }

    /// The actions are the plan's three, in order, with its transfer pair and the fixed
    /// 0.05 deadband.
    #[test]
    fn the_actions_are_the_plans_three() {
        let lj = Manifest::lanternjaw();
        assert_eq!(lj.actions.len(), 4, "the lanternjaw adds `lure`");
        assert_eq!(&lj.actions[..3], &ACTIONS[..]);
        assert_eq!(lj.actions[3].name, "lure_effort");
        for m in [Founder::Blind, Founder::Browser].into_iter().map(Founder::manifest) {
            assert_eq!(m.actions.len(), 3);
            assert_eq!(m.deadband, 0.05);
            assert_eq!(m.actions[0].name, "forward_effort");
            assert_eq!(m.actions[0].transfer, Transfer::Sigmoid);
            assert_eq!(
                (m.actions[1].name, m.actions[1].transfer),
                ("turn_effort", Transfer::Tanh)
            );
            assert_eq!(
                (m.actions[2].name, m.actions[2].transfer),
                ("feed_effort", Transfer::Sigmoid)
            );
            assert_eq!(m.cadence_ticks(), 5, "0.25 s at 20 Hz");
        }
    }

    /// The counts the GRU runtime must accept: 5,571 and 6,915.
    #[test]
    fn the_parameter_counts_are_the_contracts() {
        assert_eq!(Manifest::blind().parameter_count(), 5_571);
        assert_eq!(Manifest::browser().parameter_count(), 6_915);
        assert_eq!(gru32_parameter_count(23, 3), 5_571);
        assert_eq!(gru32_parameter_count(37, 3), 6_915);
    }

    /// The digest is stable across calls, is exactly FNV-1a over the canonical text, and
    /// separates two founders that differ in a single module. The schema-significant
    /// constants — Self references, the movement references, the taste cue and
    /// its resistance mapping — are all part of the canonical text.
    #[test]
    fn the_digest_is_stable_and_separates_the_founders() {
        let blind = Manifest::blind();
        let browser = Manifest::browser();
        assert_eq!(blind.digest(), blind.digest());
        assert_eq!(browser.digest(), browser.digest());
        assert_eq!(blind.digest(), fnv1a(blind.canonical_text().as_bytes()));
        assert_ne!(blind.digest(), browser.digest());
        assert!(blind.canonical_text().contains("Chem(detritus)"));
        assert!(browser.canonical_text().contains("Cone(3, foliage/body)"));
        assert!(
            blind
                .canonical_text()
                .contains("|taste:litter:litter=0.2:ground=0.5")
        );
        assert!(browser.canonical_text().contains("|taste:foliage"));
        assert!(blind.canonical_text().contains("|self_ref:energy="));
        assert!(blind.canonical_text().contains("|move_ref:forward="));
    }

    /// A single changed manifest property moves the digest: the schema text is what
    /// protects meaning, so no constant can be invented that the digest does not cover.
    #[test]
    fn a_single_changed_manifest_property_moves_the_digest() {
        let baseline = Manifest::blind();
        let before = baseline.digest();

        let mut tau = baseline;
        tau.tunings.chem_tau_s = 2.0;
        assert_ne!(
            tau.digest(),
            before,
            "a chem tuning move did not move the digest"
        );

        let mut energy = baseline;
        energy.adult_energy_reference = 0.1;
        assert_ne!(
            energy.digest(),
            before,
            "a Self energy reference move did not move the digest"
        );

        let mut resistance = baseline;
        resistance.taste_resistances = &[("litter", 0.9), ("ground", 0.5)];
        assert_ne!(
            resistance.digest(),
            before,
            "a taste resistance move did not move the digest"
        );

        let mut reference = baseline;
        reference.forward_reference_m = 0.01;
        assert_ne!(
            reference.digest(),
            before,
            "a movement reference move did not move the digest"
        );
    }
}
