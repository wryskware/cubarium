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
//! # Contract v3 (2026-10-09)
//!
//! Schema 3 (`design/brain-contract-v3-proposal-2026-10-08.md` §2.1, §2.3 B, §4 P1, with
//! Wrysk's answers in `design/training-plan-2026-10-08.md` §6 items 12, 14, 23–25). Every
//! lineage keeps its v2 layout **in place** and appends its v3 blocks ([`Module::tail`]):
//! the body state the v2 `Self` lacks, the threat, kin and prey slots read off the percept
//! record, carcass, scents, tremor, the photoreceptor, the lineage's own state (hunker,
//! water, mode, pack) and its `Place(…)` blocks, encoded by [`V3_ENCODING_RULE`]
//! (`crate::observe_v3`). A lineage's actions are the three local ones and then its body's
//! own (`sprint`, `hunker`, `retract`, `lure`, `lift`), declared here and wired in P2; an
//! option head ([`Manifest::option_head`], 0 until P8) may follow them. The hidden width is
//! per lineage ([`Manifest::hidden`]). All of it is in the canonical text, so every v2
//! centre is refused.
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
//! [`Manifest::parameter_count`] is the GRU count for the manifest's shapes:
//! `3·H·(I + H + 2) + O·(H + 1)` with `H` its hidden width and `O` its outputs (actions
//! plus option head), which at `H = 32` is `cubarium-core`'s `neural::gru::parameter_count`:
//! the number the runtime must accept.

use std::fmt::Write;

use serde::{Deserialize, Serialize};

/// The default hidden width of a lineage's GRU ([`Manifest::hidden`]). Contract v3 makes
/// it a per-lineage manifest field folded into the digest, so a bigger brain for one
/// lineage (the proposal's Q5) is a manifest change, not a runtime one.
pub const HIDDEN: usize = 32;

/// The schema-version token folded into every manifest's canonical text. A change to the
/// meaning of an existing slot bumps this and invalidates old digests. **3**: contract v3
/// (the v3 blocks, per-lineage hidden width, the body actions and the option head).
pub const SCHEMA_VERSION: u32 = 3;

/// The most network outputs one response carries ([`crate::Response::Logits`]): a
/// lineage's actions plus its option head ([`Manifest::outputs`]). A capacity, not a shape:
/// a manifest's own count says how many are read.
pub const MAX_OUTPUTS: usize = 16;

/// How the v3 blocks are encoded, as the canonical text records it. **Bearing**: the body
/// frame, positive to the right, as `sin` and `cos`, recomputed at encoding from the body's
/// pose to the item's last-known point (no coordinate reaches the vector). **Proximity**:
/// `1 − d / range`, `d` horizontal, clamped. **Fresh**: `1 − age / fresh_s`; a slot older
/// than `fresh_s` is empty. **Still**: the item's FEAR posture is REST or ASLEEP. **Count**:
/// `min(n, 4) / 4`. **Level** responses: `x / (x + reference)`. **Gradient**: `tanh(g · 1 m
/// / reference)` along the body's forward and right. Every block carries its validity, and
/// an empty one reads all zero.
pub const V3_ENCODING_RULE: &str = "bearing=body_right_pos:sin,cos@now;proximity=1-d_h/range;fresh=1-age/fresh_s;still=rest|asleep;count=min(n,4)/4;level=x/(x+ref);gradient=tanh(g*1m/ref);empty=0";

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
    /// The **ripple-snail** (roster R2b): a slow, water-bound film grazer on glassfilm.
    /// The shredder's blind 23-input layout with its `Chem` reading a film field (the
    /// film stock per site) in place of the detritus field. Diet `[film]`.
    RippleSnail,
    /// The **bellwing** (F1, `design/handoffs/voxel-flight-2026-09-28.md` Brief F1): the
    /// first flyer, a small diurnal nectar feeder that flies within its band, hovers at
    /// flowers and perches on crowns, vine faces and undersides. Its own manifest with a
    /// fourth action, `lift`. Diet `[nectar]`.
    Bellwing,
    /// The **seedporter** (F3, `design/handoffs/voxel-roster-2026-10-01.md` Brief F3): a
    /// diurnal frugivore that climbs stems and vine-covered faces, glides from grove to
    /// grove and carries seeds in its gut. The browser's 37-input layout with a cone that
    /// counts fruit in place of foliage. Diet `[fruit]`.
    Seedporter,
    /// The **chorister** (roster R3, `design/handoffs/voxel-roster-2026-10-01.md` Brief R3;
    /// predation-and-flight §2.10, §2.14): a diurnal pack hunter, 1.4 × 0.35 × 0.6 m, built
    /// on the lanternjaw's hunter table and strike machinery with no lure. Packs with a den,
    /// one breeder, provisioned pups and a territory. Diet `[carrion]`.
    Chorister,
}

impl Founder {
    pub const ALL: [Founder; 9] = [
        Founder::Blind,
        Founder::Browser,
        Founder::Lanternjaw,
        Founder::Capgnawer,
        Founder::Loftstrider,
        Founder::RippleSnail,
        Founder::Bellwing,
        Founder::Seedporter,
        Founder::Chorister,
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
            Founder::RippleSnail => 5,
            Founder::Bellwing => 6,
            Founder::Seedporter => 7,
            Founder::Chorister => 8,
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
            Founder::RippleSnail => "ripple-snail",
            Founder::Bellwing => "bellwing",
            Founder::Seedporter => "seedporter",
            Founder::Chorister => "chorister",
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
            Founder::RippleSnail => "film_grazer",
            Founder::Bellwing => "pollinator",
            Founder::Seedporter => "glider",
            Founder::Chorister => "pack_hunter",
        }
    }

    /// This founder's fixed manifest.
    pub fn manifest(self) -> Manifest {
        // The interface is fixed; rebuilding its default physiology for every cadence
        // check and observation costs more than copying the resulting manifest.
        static FIXED: std::sync::OnceLock<[Manifest; Founder::COUNT]> = std::sync::OnceLock::new();
        FIXED.get_or_init(|| Founder::ALL.map(|founder| match founder {
            Founder::Blind => Manifest::blind(),
            Founder::Browser => Manifest::browser(),
            Founder::Lanternjaw => Manifest::lanternjaw(),
            Founder::Capgnawer => Manifest::capgnawer(),
            Founder::Loftstrider => Manifest::loftstrider(),
            Founder::RippleSnail => Manifest::ripple_snail(),
            Founder::Bellwing => Manifest::bellwing(),
            Founder::Seedporter => Manifest::seedporter(),
            Founder::Chorister => Manifest::chorister(),
        }))[self.index()]
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
    /// **A contract-v3 block** (`design/brain-contract-v3-proposal-2026-10-08.md` §2.1),
    /// appended after the lineage's v2 layout. The v2 modules keep their slots, so the
    /// observation-only heuristics read exactly what they read under v2; the v3 blocks are
    /// encoded only for a body whose controller reads them (a brain) or while the
    /// executed-action tap records it ([`crate::body`]'s tail encoder).
    pub tail: bool,
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
    /// The mouth band's floor, a fraction of height (roster R1): 0 for every lineage but
    /// the loftstrider, whose band is `[0.235, 1.03] × height`. The canonical text names it
    /// only when it is not 0, so the older lineages' digests do not move.
    pub mouth_floor_fraction: f64,
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
    /// **The cone sees only the foliage its mouth band reaches** (roster R1, the
    /// loftstrider's interim cone until P2): a foliage crown cell outside
    /// [`crate::Body::mouth_layers`] from the standing face reads as an occluder, not as
    /// foliage. `false` for every other lineage; named in the canonical text only when set.
    pub cone_banded_to_mouth: bool,
    /// **A fruit cone** (F3, the seedporter): the cone's class slot (the browser's
    /// foliage fraction and proximity) counts a ray's first hit only when it is **fruit**
    /// — a crown cell of a stand hanging fruit, or a latticevine face in fruit the ray
    /// entered; plain foliage is an occluder. Interim until P2.
    pub cone_counts_fruit: bool,
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
    /// references are computed (only the `Self` references follow body mass).
    pub self_reference_rule: &'static str,
    pub taste_reference_rule: &'static str,
    /// The effective food-unit taste reference (`taste_reference` override or
    /// `taste_reference_food`), recorded in the digest. `None` means the film-only
    /// rule derives the reference from the world's cell capacity.
    pub taste_reference: Option<f64>,
    /// **The hidden width** of this lineage's GRU (contract v3): [`HIDDEN`] for every lineage
    /// today. Folded into the digest and the parameter count, so a bigger brain for one
    /// lineage is a manifest change.
    pub hidden: usize,
    /// **The option head's width** (the B/C hybrid, training plan item 23): option logits the
    /// network emits after its actions, option 0 being its own steering. 0 for every lineage
    /// until P8 writes the skills; the response, the adapter and the parameter count already
    /// admit it ([`Manifest::outputs`]).
    pub option_head: usize,
    /// The v3 slots' proximity range, metres: the lineage's longest remote sense (sight,
    /// hearing or tremor) in its frozen table.
    pub slot_range_m: f64,
    /// A v3 slot's freshness horizon, seconds: the perception's `give_up_s` as shipped. A
    /// slot older than this reads empty.
    pub fresh_s: f64,
    /// The `Carcass` block's range, metres: the hunt's carcass scan as shipped.
    pub carcass_range_m: f64,
    /// **The intent menu** (CONTRACTS, [`crate::intent`]): its named intents and masked
    /// spares; a one-item menu for a plain lineage, whose canonical text then does not
    /// change.
    pub menu: &'static crate::Menu,
    /// The subject candidate rows (§11.1): kin, threat, prey and place slots, each row
    /// [`crate::subjects::ROW_WIDTH`] wide. All 0 for a plain lineage.
    pub kin_slots: usize,
    pub threat_slots: usize,
    pub prey_slots: usize,
    pub place_slots: usize,
}

/// The `Self` energy, reserve, structural-loss and intake references are the manifest's
/// times the body's `mass_scale`: each reads a fraction of an adult of that mass.
pub const SELF_REFERENCE_RULE: &str = "self_refs_x_body_mass_scale";

/// Taste's reference stock is a per-lineage food quantity, independent of body mass
/// and scent emission. The effective stock is also recorded in the manifest digest.
pub const TASTE_REFERENCE_RULE: &str = "taste_ref_food_units";

/// A film eater's taste reference (roster mechanics, the coordinator's item 6): one
/// site's film capacity, `film_capacity_per_m2 × cell area`, on the world's own cell.
pub const TASTE_REFERENCE_FILM_RULE: &str = "taste_ref_film_capacity_x_cell_area";

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
    tail: false,
};

const CONTACT4_CHANNELS: [&str; 5] = ["front", "left", "right", "underside", "valid"];

const MODULE_CONTACT4: Module = Module {
    name: "Contact(4)",
    offset: 8,
    width: 5,
    channels: &CONTACT4_CHANNELS,
    tail: false,
};

const WET_CHANNELS: [&str; 2] = ["response", "valid"];

const MODULE_WET: Module = Module {
    name: "Wet",
    offset: 13,
    width: 2,
    channels: &WET_CHANNELS,
    tail: false,
};

const TASTE1_CHANNELS: [&str; 3] = ["cue", "resistance", "valid"];

const MODULE_TASTE1: Module = Module {
    name: "Taste(1)",
    offset: 15,
    width: 3,
    channels: &TASTE1_CHANNELS,
    tail: false,
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
    tail: false,
};

/// The capgnawer's diffused cue channel (roster R1): the **fungal** field, whose sources
/// are the cap stock of saprotroph stands ([`crate::senses`]), in the shredder's
/// `Chem(detritus)` slot and width.
const MODULE_CHEM_FUNGAL: Module = Module {
    name: "Chem(fungal)",
    offset: 18,
    width: 3,
    channels: &CHEM_DETRITUS_CHANNELS,
    tail: false,
};

// Roster R2b: the ripple-snail's `Chem(film)`, the film field in the `Chem(detritus)` slot.
const MODULE_CHEM_FILM: Module = Module {
    name: "Chem(film)",
    offset: 18,
    width: 3,
    channels: &CHEM_DETRITUS_CHANNELS,
    tail: false,
};

const LIGHT_CHANNELS: [&str; 2] = ["response", "valid"];

const MODULE_LIGHT: Module = Module {
    name: "Light",
    offset: 21,
    width: 2,
    channels: &LIGHT_CHANNELS,
    tail: false,
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
    tail: false,
};

#[cfg(test)]
const BLIND_MODULES_V2: [Module; 6] = [
    MODULE_SELF,
    MODULE_CONTACT4,
    MODULE_WET,
    MODULE_TASTE1,
    MODULE_CHEM_DETRITUS,
    MODULE_LIGHT,
];
#[cfg(test)]
const CAPGNAWER_MODULES_V2: [Module; 6] = [
    MODULE_SELF,
    MODULE_CONTACT4,
    MODULE_WET,
    MODULE_TASTE1,
    MODULE_CHEM_FUNGAL,
    MODULE_LIGHT,
];
#[cfg(test)]
const RIPPLE_SNAIL_MODULES_V2: [Module; 6] = [
    MODULE_SELF,
    MODULE_CONTACT4,
    MODULE_WET,
    MODULE_TASTE1,
    MODULE_CHEM_FILM,
    MODULE_LIGHT,
];
// F1: the flyer's layout (predation-and-flight §3.6). `Self` gains `altitude` (the base's
// height over the support face below / `band_max_m`) and `ceiling` (the clear height over
// the base to the first solid or water cell in its column / `band_max_m`, capped at 1).
const FLYER_SELF_CHANNELS: [&str; 10] = [
    "energy",
    "reserve",
    "birth_readiness",
    "structural_loss",
    "assimilated_intake",
    "resolved_forward",
    "resolved_turn",
    "motor_delivery",
    "altitude",
    "ceiling",
];

const MODULE_FLYER_SELF: Module = Module {
    name: "Self",
    offset: 0,
    width: 10,
    channels: &FLYER_SELF_CHANNELS,
    tail: false,
};

/// A flyer's state in the air: perched (1) or aloft (0); hanging (perched under an
/// underside); `overhead`, the clear height over the body's top across its whole span to
/// the first solid, over the band (capped at 1); the rain rate over the flight's `R₀`
/// (capped at 1); and whether the weather grounds it (the `downpour` channel: a downpour,
/// or rain over its `Flight::rain_ground_m_per_s` latch, backlog §12).
const AIR_CHANNELS: [&str; 5] = ["perched", "hanging", "overhead", "rain", "downpour"];

const MODULE_AIR: Module = Module {
    name: "Air",
    offset: 10,
    width: 5,
    channels: &AIR_CHANNELS,
    tail: false,
};

/// A flyer's mouth: the nearest bloom's nectar in reach.
const MODULE_FLYER_TASTE: Module = Module {
    name: "Taste(1)",
    offset: 15,
    width: 3,
    channels: &TASTE1_CHANNELS,
    tail: false,
};

const FAN3_CHANNELS: [&str; 25] = [
    "s0.clear",
    "s0.all_proximity",
    "s0.bloom",
    "s0.bloom_proximity",
    "s0.foliage",
    "s0.foliage_proximity",
    "s0.body",
    "s0.body_proximity",
    "s1.clear",
    "s1.all_proximity",
    "s1.bloom",
    "s1.bloom_proximity",
    "s1.foliage",
    "s1.foliage_proximity",
    "s1.body",
    "s1.body_proximity",
    "s2.clear",
    "s2.all_proximity",
    "s2.bloom",
    "s2.bloom_proximity",
    "s2.foliage",
    "s2.foliage_proximity",
    "s2.body",
    "s2.body_proximity",
    "valid",
];

/// The flyer's downward fan (§3.6): the browser's three sectors and yaw offsets, pitches
/// −80 … +40, 4 m, with a `bloom` class beside foliage and bodies.
const MODULE_FAN3: Module = Module {
    name: "Fan(3, bloom/foliage/body)",
    offset: 18,
    width: 25,
    channels: &FAN3_CHANNELS,
    tail: false,
};

#[cfg(test)]
const BELLWING_MODULES_V2: [Module; 4] = [MODULE_FLYER_SELF, MODULE_AIR, MODULE_FLYER_TASTE, MODULE_FAN3];

/// The flyer fan's pitch offsets, degrees (§3.6): it looks down.
pub const FLYER_RAY_PITCH_OFFSETS_DEG: [f64; 5] = [-80.0, -50.0, -20.0, 10.0, 40.0];
/// The classes a flyer's fan exposes.
pub const FLYER_VISIBLE_CLASSES: [&str; 3] = ["bloom", "foliage", "body"];
/// The flyer's mouth mapping: a soft flower; no ground contact (a mouth in air reads
/// nothing).
const BELLWING_TASTE_RESISTANCES: [(&str, f64); 1] = [("nectar", 0.1)];

/// The seedporter's mouth mapping (F3): soft fruit, firm ground.
const SEEDPORTER_TASTE_RESISTANCES: [(&str, f64); 2] = [("fruit", 0.1), ("ground", 0.5)];

#[cfg(test)]
const BROWSER_MODULES_V2: [Module; 5] = [
    MODULE_SELF,
    MODULE_CONTACT4,
    MODULE_WET,
    MODULE_TASTE1,
    MODULE_CONE3,
];

// ---------------------------------------------------------------- contract v3 blocks
//
// `design/brain-contract-v3-proposal-2026-10-08.md` §2.1, reconciled with the code of
// 2026-10-09 (`design/training-plan-2026-10-08.md` §6 items 12, 14, 23–25). Each lineage's v3
// layout is its v2 layout, unchanged and in place, followed by its v3 blocks: the order is
// not meaning to a network, and keeping the v2 slots lets the observation-only heuristics
// read exactly what they read under v2.

/// Lays a module list out contiguously from slot 0, in order.
const fn layout<const N: usize>(mut mods: [Module; N]) -> [Module; N] {
    let mut next = 0;
    let mut k = 0;
    while k < N {
        mods[k].offset = next;
        next += mods[k].width;
        k += 1;
    }
    mods
}

/// A v3 block: its slot is set by [`layout`].
const fn tail(name: &'static str, channels: &'static [&'static str]) -> Module {
    Module { name, offset: 0, width: channels.len(), channels, tail: true }
}

/// `SelfV3`: the body state the v2 `Self` lacks — the stomach's fill over its capacity
/// `K`, the eating latch, the wound stock, stamina (`1 − sprint_spent`; 1 for a lineage
/// with no sprint), the daylight at the sky (1 with the clock off) and the day's phase as
/// `sin`/`cos` (0 midnight).
const SELF_V3_CHANNELS: [&str; 7] = ["stomach_fill", "eating", "wound", "stamina", "daylight", "day_sin", "day_cos"];
/// A sensed body's slot ([`V3_ENCODING_RULE`]), read off the record's per-class best
/// track (`Percept::best`).
const PREY_CHANNELS: [&str; 7] = ["valid", "sin", "cos", "proximity", "moving", "still", "fresh"];
/// The threat slot and the number of threats in the latest sensing (the loftstrider's
/// group rule reads it).
const THREAT_CHANNELS: [&str; 8] = ["valid", "sin", "cos", "proximity", "moving", "still", "fresh", "count"];
/// The kin slot, the herd alarm (a kin heard running, the vigil's own test) and the number
/// of kin in the latest sensing (the herding pull and the many eyes read it).
const KIN_CHANNELS: [&str; 9] = ["valid", "sin", "cos", "proximity", "moving", "still", "fresh", "alarm", "count"];
/// The nearest usable carcass within the manifest's carcass range.
const CARCASS_CHANNELS: [&str; 4] = ["valid", "sin", "cos", "proximity"];
/// One scent channel where the body stands: the level and its gradient along the body.
const SCENT_CHANNELS: [&str; 3] = ["level", "forward", "right"];
/// The footfall felt on its left and right.
const TREMOR_CHANNELS: [&str; 2] = ["left", "right"];
/// The flyer's photoreceptor (S3): ambient light, the looming shadow, the brightest still
/// beacon (its kind as `cyan`: 1 a cyan emitter, 0 a near-IR reflector, as its eye sees
/// it) and the brightest moving light.
const LIGHT_V3_CHANNELS: [&str; 12] = [
    "ambient",
    "loom",
    "loom_sin",
    "loom_cos",
    "beacon",
    "beacon_level",
    "beacon_sin",
    "beacon_cos",
    "beacon_cyan",
    "moving_level",
    "moving_sin",
    "moving_cos",
];
/// The capgnawer's cyan sense (S3): the brightest still glow it sees, and whether it stands
/// under one of its own failed-visit marks.
const GLOW_CHANNELS: [&str; 5] = ["beacon", "beacon_level", "beacon_sin", "beacon_cos", "marked_here"];
/// The frondgrazer's hunker state.
const HUNKER_CHANNELS: [&str; 1] = ["hunkered"];
/// The ripple-snail's water state: its drying clock, thirst, ashore, retracted.
const AMPHIBIOUS_CHANNELS: [&str; 4] = ["dry", "thirst", "ashore", "retracted"];
/// The seedporter's mode (on the ground or a wall, climbing, perched, gliding) and its base
/// over the support below it.
const MODE_CHANNELS: [&str; 5] = ["ground", "climb", "perched", "glide", "height"];
/// The lanternjaw's light at its face (`sky × daylight`): the lure pays only in the dark.
const FACE_LIGHT_CHANNELS: [&str; 1] = ["level"];
/// The Veilhound's pack: breeder, pup, the pack target (known to every member by call) and
/// the pack's feast.
const SOCIAL_CHANNELS: [&str; 10] = [
    "breeder",
    "pup",
    "target.valid",
    "target.sin",
    "target.cos",
    "target.proximity",
    "feast.valid",
    "feast.sin",
    "feast.cos",
    "feast.proximity",
];
/// **One place** (item 24): the nearest such place the body could perceive, else the one it
/// remembers using, which fades if not revisited — `fresh` is 1 for a place perceived now
/// and falls as a memory ages. P6 fills them; until then every place reads empty but the
/// den, which a pack member always knows.
const PLACE_CHANNELS: [&str; 5] = ["valid", "sin", "cos", "proximity", "fresh"];

const MODULE_SELF_V3: Module = tail("SelfV3", &SELF_V3_CHANNELS);
const MODULE_THREAT: Module = tail("Threat", &THREAT_CHANNELS);
const MODULE_KIN: Module = tail("Kin", &KIN_CHANNELS);
const MODULE_PREY: Module = tail("Prey", &PREY_CHANNELS);
const MODULE_CARCASS: Module = tail("Carcass", &CARCASS_CHANNELS);
const MODULE_SCENT_HUNTER: Module = tail("Scent(hunter)", &SCENT_CHANNELS);
const MODULE_SCENT_GRAZER: Module = tail("Scent(grazer)", &SCENT_CHANNELS);
const MODULE_SCENT_SMALL_PREY: Module = tail("Scent(small-prey)", &SCENT_CHANNELS);
const MODULE_SCENT_CARRION: Module = tail("Scent(carrion)", &SCENT_CHANNELS);
const MODULE_SCENT_NECTAR: Module = tail("Scent(nectar)", &SCENT_CHANNELS);
const MODULE_TREMOR: Module = tail("Tremor", &TREMOR_CHANNELS);
const MODULE_LIGHT_V3: Module = tail("LightV3", &LIGHT_V3_CHANNELS);
const MODULE_GLOW: Module = tail("Glow", &GLOW_CHANNELS);
const MODULE_HUNKER: Module = tail("Hunker", &HUNKER_CHANNELS);
const MODULE_AMPHIBIOUS: Module = tail("Amphibious", &AMPHIBIOUS_CHANNELS);
const MODULE_MODE: Module = tail("Mode", &MODE_CHANNELS);
const MODULE_FACE_LIGHT: Module = tail("FaceLight", &FACE_LIGHT_CHANNELS);
const MODULE_SOCIAL: Module = tail("Social", &SOCIAL_CHANNELS);
const MODULE_PLACE_SHORE: Module = tail("Place(shore)", &PLACE_CHANNELS);
const MODULE_PLACE_WATER: Module = tail("Place(water)", &PLACE_CHANNELS);
const MODULE_PLACE_SHORE_EGG: Module = tail("Place(shore_egg)", &PLACE_CHANNELS);
const MODULE_PLACE_FILM: Module = tail("Place(film)", &PLACE_CHANNELS);
const MODULE_PLACE_ROOST: Module = tail("Place(roost)", &PLACE_CHANNELS);
const MODULE_PLACE_FRUIT: Module = tail("Place(fruit)", &PLACE_CHANNELS);
const MODULE_PLACE_NEST: Module = tail("Place(nest)", &PLACE_CHANNELS);
const MODULE_PLACE_DEN: Module = tail("Place(den)", &PLACE_CHANNELS);

/// The littershredder: v2's 23, then its body state, the tremor it freezes on, the threat
/// it flees, and the shore its shallows reflex walks to.
const BLIND_MODULES: [Module; 10] = layout([
    MODULE_SELF,
    MODULE_CONTACT4,
    MODULE_WET,
    MODULE_TASTE1,
    MODULE_CHEM_DETRITUS,
    MODULE_LIGHT,
    MODULE_SELF_V3,
    MODULE_TREMOR,
    MODULE_THREAT,
    MODULE_PLACE_SHORE,
]);
/// The capgnawer: as the shredder, plus the cyan glow its light steering follows.
const CAPGNAWER_MODULES: [Module; 11] = layout([
    MODULE_SELF,
    MODULE_CONTACT4,
    MODULE_WET,
    MODULE_TASTE1,
    MODULE_CHEM_FUNGAL,
    MODULE_LIGHT,
    MODULE_SELF_V3,
    MODULE_TREMOR,
    MODULE_THREAT,
    MODULE_GLOW,
    MODULE_PLACE_SHORE,
]);
/// The ripple-snail: as the shredder, plus its water state and the three places its
/// reflexes walk to (water when thirsty, a shore egg face when ready to lay, a richer film
/// face when its own is grazed down).
const RIPPLE_SNAIL_MODULES: [Module; 13] = layout([
    MODULE_SELF,
    MODULE_CONTACT4,
    MODULE_WET,
    MODULE_TASTE1,
    MODULE_CHEM_FILM,
    MODULE_LIGHT,
    MODULE_SELF_V3,
    MODULE_TREMOR,
    MODULE_THREAT,
    MODULE_AMPHIBIOUS,
    MODULE_PLACE_WATER,
    MODULE_PLACE_SHORE_EGG,
    MODULE_PLACE_FILM,
]);
/// The frondgrazer: v2's 37, then its body state, threat and kin (flight, the herd alarm,
/// herding, many eyes), hunter scent, tremor, its hunker state and the shore.
const BROWSER_MODULES: [Module; 12] = layout([
    MODULE_SELF,
    MODULE_CONTACT4,
    MODULE_WET,
    MODULE_TASTE1,
    MODULE_CONE3,
    MODULE_SELF_V3,
    MODULE_THREAT,
    MODULE_KIN,
    MODULE_SCENT_HUNTER,
    MODULE_TREMOR,
    MODULE_HUNKER,
    MODULE_PLACE_SHORE,
]);
/// The loftstrider: as the frondgrazer without the hunker (its calves' following pull reads
/// the kin slot).
const LOFTSTRIDER_MODULES: [Module; 11] = layout([
    MODULE_SELF,
    MODULE_CONTACT4,
    MODULE_WET,
    MODULE_TASTE1,
    MODULE_CONE3,
    MODULE_SELF_V3,
    MODULE_THREAT,
    MODULE_KIN,
    MODULE_SCENT_HUNTER,
    MODULE_TREMOR,
    MODULE_PLACE_SHORE,
]);
/// The bellwing: v2's 43, then its body state (the latch fed rest reads), the
/// photoreceptor its light steering follows, the nectar plume, perch tremor and the vine
/// nest site its nest reflex flies to (VINE-C).
const BELLWING_MODULES: [Module; 9] = layout([
    MODULE_FLYER_SELF,
    MODULE_AIR,
    MODULE_FLYER_TASTE,
    MODULE_FAN3,
    MODULE_SELF_V3,
    MODULE_LIGHT_V3,
    MODULE_SCENT_NECTAR,
    MODULE_TREMOR,
    MODULE_PLACE_NEST,
]);
/// The seedporter: v2's 37, then its body state, threat, kin (the troop pull), prey (the
/// insect catch), carcass, hunter and carrion scent, tremor, its mode and height, and the
/// roost, fruiting stand and shore its reflexes walk to.
const SEEDPORTER_MODULES: [Module; 17] = layout([
    MODULE_SELF,
    MODULE_CONTACT4,
    MODULE_WET,
    MODULE_TASTE1,
    MODULE_CONE3,
    MODULE_SELF_V3,
    MODULE_THREAT,
    MODULE_KIN,
    MODULE_PREY,
    MODULE_CARCASS,
    MODULE_SCENT_HUNTER,
    MODULE_SCENT_CARRION,
    MODULE_TREMOR,
    MODULE_MODE,
    MODULE_PLACE_ROOST,
    MODULE_PLACE_FRUIT,
    MODULE_PLACE_SHORE,
]);
/// The lanternjaw: v2's 37, then its body state, prey, threat, carcass, the scents of its
/// prey and of carrion, tremor, the light at its face (the lure pays only in the dark) and
/// the shore. Its senses table is not live yet: its record holds only what its patrol's
/// interim rules put there (a gap until its table turns on).
const LANTERNJAW_MODULES: [Module; 15] = layout([
    MODULE_SELF,
    MODULE_CONTACT4,
    MODULE_WET,
    MODULE_TASTE1,
    MODULE_CONE3,
    MODULE_SELF_V3,
    MODULE_PREY,
    MODULE_THREAT,
    MODULE_CARCASS,
    MODULE_SCENT_SMALL_PREY,
    MODULE_SCENT_GRAZER,
    MODULE_SCENT_CARRION,
    MODULE_TREMOR,
    MODULE_FACE_LIGHT,
    MODULE_PLACE_SHORE,
]);
/// The Veilhound: v2's 37, then its body state, prey, kin (packmates), carcass, four
/// scents, tremor, its pack (breeder, pup, target by call, feast), its den (always known)
/// and the shore.
const CHORISTER_MODULES: [Module; 17] = layout([
    MODULE_SELF,
    MODULE_CONTACT4,
    MODULE_WET,
    MODULE_TASTE1,
    MODULE_CONE3,
    MODULE_SELF_V3,
    MODULE_PREY,
    MODULE_KIN,
    MODULE_CARCASS,
    MODULE_SCENT_GRAZER,
    MODULE_SCENT_SMALL_PREY,
    MODULE_SCENT_HUNTER,
    MODULE_SCENT_CARRION,
    MODULE_TREMOR,
    MODULE_SOCIAL,
    MODULE_PLACE_DEN,
    MODULE_PLACE_SHORE,
]);

/// The two shapes the search runtime is compiled for until P3 makes it generic: the
/// littershredder's and the frondgrazer's inputs and outputs. A test holds them to the
/// manifests.
pub const BLIND_INPUTS: usize = 45;
pub const BLIND_OUTPUTS: usize = 3;
pub const BROWSER_INPUTS: usize = 72;
pub const BROWSER_OUTPUTS: usize = 4;

/// The fixed references the v3 levels are written against ([`V3_ENCODING_RULE`]): a scent
/// level (the grazer channel's steady level 3.5 m from a still adult frondgrazer,
/// [`crate::scent`]'s calibration), a footfall (the shared tremor floor), and a height over
/// the support below, metres.
pub const SCENT_REFERENCE: f64 = crate::scent::SMELL_AT_3_5_M;
pub const TREMOR_REFERENCE: f64 = 0.003;
pub const HEIGHT_REFERENCE_M: f64 = 8.0;

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

/// The bellwing's four actions (F1, predation-and-flight §3.3): the three above and
/// `lift_effort`, vertical effort in `[-1, 1]`. Forward 0 and lift 0 is a hover. The
/// fourth reaches the body through [`crate::Controller::lift`], as the lanternjaw's lure
/// does, until P2's contract v3 carries it in the response.
const BELLWING_ACTIONS: [Action; 4] = [
    ACTIONS[0],
    ACTIONS[1],
    ACTIONS[2],
    Action {
        name: "lift_effort",
        low: -1.0,
        high: 1.0,
        transfer: Transfer::Tanh,
    },
];

/// A `[0, 1]` body action (contract v3, proposal §2.3 B). Declared here; P2 wires each to
/// the body (until then the step reads only forward, turn and feed, and the lure and lift
/// through the interim seams).
const fn body_action(name: &'static str) -> Action {
    Action { name, low: 0.0, high: 1.0, transfer: Transfer::Sigmoid }
}

/// The frondgrazer's four: the three and `hunker_effort` (enter or hold the vigil hunker).
const HUNKER_ACTIONS: [Action; 4] = [ACTIONS[0], ACTIONS[1], ACTIONS[2], body_action("hunker_effort")];
/// The ripple-snail's four: the three and `retract_effort` (close, held at least
/// `retract_hold_s`).
const RETRACT_ACTIONS: [Action; 4] = [ACTIONS[0], ACTIONS[1], ACTIONS[2], body_action("retract_effort")];
/// The seedporter's and the Veilhound's four: the three and `sprint_effort` (forward scaled
/// up to the lineage's top speed through the stamina gate).
const SPRINT_ACTIONS: [Action; 4] = [ACTIONS[0], ACTIONS[1], ACTIONS[2], body_action("sprint_effort")];

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
/// A flyer's controller period (F1): one tick.
const FLYER_CONTROLLER_PERIOD_S: f64 = cubarium_voxel::DT;

/// The fields of a manifest that are the founder's **physical anchors**, read off its
/// frozen physiology (D1) so the contract and the body cannot drift apart.
struct Anchors {
    length: f64,
    width: f64,
    height: f64,
    eye: f64,
    mouth_ceiling: f64,
    mouth_floor: f64,
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
    taste_ref: Option<f64>,
    slot_range: f64,
    /// The intent slots: kin, threat, prey, place.
    slots: [usize; 4],
}

fn anchors(founder: Founder) -> Anchors {
    let p = crate::FounderPhysiology::frozen(founder);
    Anchors {
        length: p.adult_length_m,
        width: p.adult_width_m,
        height: p.adult_height_m,
        eye: p.eye_height_fraction,
        mouth_ceiling: p.mouth_ceiling_fraction,
        mouth_floor: p.mouth_floor_fraction,
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
        taste_ref: if p.taste_reference.is_none() && p.diet == crate::Diet::only(crate::Food::Film) {
            None
        } else {
            Some(p.taste_reference_stock(crate::CueConfig::default().emit_reference))
        },
        slot_range: p.senses.sight_m.max(p.senses.hearing_m).max(p.senses.tremor_m).max(0.0),
        slots: [
            p.intent.kin_slots as usize,
            p.intent.threat_slots as usize,
            p.intent.prey_slots as usize,
            p.intent.place_slots as usize,
        ],
    }
}

/// The `Self` references of a lineage's manifest, read off its placeholder (x1)
/// physiology by the rule the three older manifests' hand-written numbers follow: the
/// body and structural references are `body_max`, the reserve `reserve_cap · body_max`,
/// the energy `energy_density · (body_max + reserve_cap · body_max)`. Used by the roster
/// lineages (R1); the older manifests keep their literals so their digests do not move.
struct SelfReferences {
    body: f64,
    reserve: f64,
    energy: f64,
}

impl SelfReferences {
    fn of(founder: Founder) -> SelfReferences {
        let c = crate::FounderPhysiology::placeholder(founder).core;
        let reserve = c.reserve_cap * c.body_max;
        SelfReferences {
            body: c.body_max,
            reserve,
            energy: c.energy_density * (c.body_max + reserve),
        }
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
            mouth_floor_fraction: a.mouth_floor,
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
            cone_banded_to_mouth: false,
            cone_counts_fruit: false,
            pool_bulk_density: crate::CueConfig::default().pool_bulk_density,
            occlusion_rule: OCCLUSION_RULE,
            birth_readiness_rule: BIRTH_READINESS_RULE,
            mass_scale: a.mass,
            self_reference_rule: SELF_REFERENCE_RULE,
            taste_reference_rule: TASTE_REFERENCE_RULE,
            taste_reference: a.taste_ref,
            hidden: HIDDEN,
            option_head: 0,
            slot_range_m: a.slot_range,
            fresh_s: crate::PerceptionConfig::default().give_up_s,
            carcass_range_m: crate::HuntConfig::default().carcass_scan_m,
            menu: &crate::intent::PLAIN,
            kin_slots: 0,
            threat_slots: 0,
            prey_slots: 0,
            place_slots: 0,
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
            actions: &HUNKER_ACTIONS,
            deadband: ACTION_DEADBAND,
            controller_period_s: CONTROLLER_PERIOD_S,
            body_length_m: a.length,
            body_width_m: a.width,
            body_height_m: a.height,
            eye_height_fraction: a.eye,
            mouth_ceiling_fraction: a.mouth_ceiling,
            mouth_floor_fraction: a.mouth_floor,
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
            cone_banded_to_mouth: false,
            cone_counts_fruit: false,
            pool_bulk_density: crate::CueConfig::default().pool_bulk_density,
            occlusion_rule: OCCLUSION_RULE,
            birth_readiness_rule: BIRTH_READINESS_RULE,
            mass_scale: a.mass,
            self_reference_rule: SELF_REFERENCE_RULE,
            taste_reference_rule: TASTE_REFERENCE_RULE,
            taste_reference: a.taste_ref,
            hidden: HIDDEN,
            option_head: 0,
            slot_range_m: a.slot_range,
            fresh_s: crate::PerceptionConfig::default().give_up_s,
            carcass_range_m: crate::HuntConfig::default().carcass_scan_m,
            menu: &crate::intent::PLAIN,
            kin_slots: 0,
            threat_slots: 0,
            prey_slots: 0,
            place_slots: 0,
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
            modules: &LANTERNJAW_MODULES,
            menu: crate::intent::menu_of(Founder::Lanternjaw),
            kin_slots: a.slots[0],
            threat_slots: a.slots[1],
            prey_slots: a.slots[2],
            place_slots: a.slots[3],
            actions: &LANTERNJAW_ACTIONS,
            deadband: ACTION_DEADBAND,
            controller_period_s: CONTROLLER_PERIOD_S,
            body_length_m: a.length,
            body_width_m: a.width,
            body_height_m: a.height,
            eye_height_fraction: a.eye,
            mouth_ceiling_fraction: a.mouth_ceiling,
            mouth_floor_fraction: a.mouth_floor,
            contact_height_fraction: a.contact,
            step_up_m: a.step_up,
            step_down_m: a.step_down,
            climbs_walls: a.walls,
            wade_height_fraction: a.wade,
            drown_height_fraction: a.drown,
            drown_after_s: a.drown_after,
            body_reference: 0.035,
            // `energy_density · (body_max + reserve_cap · body_max)`, the other two's rule,
            // at the detritus web's reserve cap of 0.6.
            adult_energy_reference: 0.112,
            adult_reserve_reference: 0.021,
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
            cone_banded_to_mouth: false,
            cone_counts_fruit: false,
            pool_bulk_density: crate::CueConfig::default().pool_bulk_density,
            occlusion_rule: OCCLUSION_RULE,
            birth_readiness_rule: BIRTH_READINESS_RULE,
            mass_scale: a.mass,
            self_reference_rule: SELF_REFERENCE_RULE,
            taste_reference_rule: TASTE_REFERENCE_RULE,
            taste_reference: a.taste_ref,
            hidden: HIDDEN,
            option_head: 0,
            slot_range_m: a.slot_range,
            fresh_s: crate::PerceptionConfig::default().give_up_s,
            carcass_range_m: crate::HuntConfig::default().carcass_scan_m,
        }
    }

    /// The capgnawer (roster R1): the blind 23-input layout with `Chem(fungal)` — the
    /// fungal field, sourced by saprotroph cap stock — in the shredder's `Chem(detritus)`
    /// slot; its own anchors (0.25 × 0.21 × 0.19 m) and `Self` references.
    pub fn capgnawer() -> Manifest {
        let a = anchors(Founder::Capgnawer);
        let r = SelfReferences::of(Founder::Capgnawer);
        Manifest {
            founder: Founder::Capgnawer,
            modules: &CAPGNAWER_MODULES,
            body_length_m: a.length,
            body_width_m: a.width,
            body_height_m: a.height,
            eye_height_fraction: a.eye,
            mouth_ceiling_fraction: a.mouth_ceiling,
            mouth_floor_fraction: a.mouth_floor,
            contact_height_fraction: a.contact,
            step_up_m: a.step_up,
            step_down_m: a.step_down,
            climbs_walls: a.walls,
            wade_height_fraction: a.wade,
            drown_height_fraction: a.drown,
            drown_after_s: a.drown_after,
            body_reference: r.body,
            adult_energy_reference: r.energy,
            adult_reserve_reference: r.reserve,
            structural_reference: r.body,
            cruise_m_per_s: a.cruise,
            yaw_cap_rad_per_s: a.yaw,
            forward_reference_m: a.cruise * CONTROLLER_PERIOD_S,
            mouth_reach_body_lengths: a.reach,
            mass_scale: a.mass,
            slot_range_m: a.slot_range,
            ..Manifest::blind()
        }
    }

    /// The loftstrider (roster R1): the browser's 37-input layout and fan, its own anchors
    /// (2.2 × 2.2 × 3.4 m, the eye ring at 1.5 m, the mouth band 0.8–3.5 m) and `Self`
    /// references, and a cone that counts only foliage inside its mouth band
    /// ([`Manifest::cone_banded_to_mouth`]). Interim until P2.
    pub fn loftstrider() -> Manifest {
        let a = anchors(Founder::Loftstrider);
        let r = SelfReferences::of(Founder::Loftstrider);
        Manifest {
            founder: Founder::Loftstrider,
            body_length_m: a.length,
            body_width_m: a.width,
            body_height_m: a.height,
            eye_height_fraction: a.eye,
            mouth_ceiling_fraction: a.mouth_ceiling,
            mouth_floor_fraction: a.mouth_floor,
            contact_height_fraction: a.contact,
            step_up_m: a.step_up,
            step_down_m: a.step_down,
            climbs_walls: a.walls,
            wade_height_fraction: a.wade,
            drown_height_fraction: a.drown,
            drown_after_s: a.drown_after,
            body_reference: r.body,
            adult_energy_reference: r.energy,
            adult_reserve_reference: r.reserve,
            structural_reference: r.body,
            cruise_m_per_s: a.cruise,
            yaw_cap_rad_per_s: a.yaw,
            forward_reference_m: a.cruise * CONTROLLER_PERIOD_S,
            mouth_reach_body_lengths: a.reach,
            mass_scale: a.mass,
            slot_range_m: a.slot_range,
            modules: &LOFTSTRIDER_MODULES,
            menu: crate::intent::menu_of(Founder::Loftstrider),
            kin_slots: a.slots[0],
            threat_slots: a.slots[1],
            prey_slots: a.slots[2],
            place_slots: a.slots[3],
            actions: &ACTIONS,
            cone_banded_to_mouth: true,
            ..Manifest::browser()
        }
    }

    /// The ripple-snail (roster R2b): the blind 23-input layout with `Chem(film)` — the
    /// film field, sourced by the film stock per site — in the shredder's
    /// `Chem(detritus)` slot; its own anchors and `Self` references.
    pub fn ripple_snail() -> Manifest {
        let a = anchors(Founder::RippleSnail);
        let r = SelfReferences::of(Founder::RippleSnail);
        Manifest {
            founder: Founder::RippleSnail,
            modules: &RIPPLE_SNAIL_MODULES,
            body_length_m: a.length,
            body_width_m: a.width,
            body_height_m: a.height,
            eye_height_fraction: a.eye,
            mouth_ceiling_fraction: a.mouth_ceiling,
            mouth_floor_fraction: a.mouth_floor,
            contact_height_fraction: a.contact,
            step_up_m: a.step_up,
            step_down_m: a.step_down,
            climbs_walls: a.walls,
            wade_height_fraction: a.wade,
            drown_height_fraction: a.drown,
            drown_after_s: a.drown_after,
            body_reference: r.body,
            adult_energy_reference: r.energy,
            adult_reserve_reference: r.reserve,
            structural_reference: r.body,
            cruise_m_per_s: a.cruise,
            yaw_cap_rad_per_s: a.yaw,
            forward_reference_m: a.cruise * CONTROLLER_PERIOD_S,
            mouth_reach_body_lengths: a.reach,
            mass_scale: a.mass,
            slot_range_m: a.slot_range,
            actions: &RETRACT_ACTIONS,
            taste_reference: a.taste_ref,
            // Its taste reads film against one site's film capacity on the world's own
            // cell (`FounderPhysiology::taste_reference_on`), not `M_EMIT × mass`.
            taste_reference_rule: TASTE_REFERENCE_FILM_RULE,
            ..Manifest::blind()
        }
    }

    /// The bellwing (F1): its own 4-action manifest (`forward`, `turn`, `feed`, `lift`)
    /// and the flyer's layout — `Self` with `altitude` and `ceiling`, `Air`, a nectar
    /// `Taste(1)`, and the downward fan (pitches −80, −50, −20, +10, +40, range 4 m, the
    /// three sectors) with a `bloom` class (predation-and-flight §3.6). 43 inputs.
    pub fn bellwing() -> Manifest {
        let a = anchors(Founder::Bellwing);
        let r = SelfReferences::of(Founder::Bellwing);
        Manifest {
            founder: Founder::Bellwing,
            actions: &BELLWING_ACTIONS,
            body_length_m: a.length,
            body_width_m: a.width,
            body_height_m: a.height,
            eye_height_fraction: a.eye,
            mouth_ceiling_fraction: a.mouth_ceiling,
            mouth_floor_fraction: a.mouth_floor,
            contact_height_fraction: a.contact,
            step_up_m: a.step_up,
            step_down_m: a.step_down,
            climbs_walls: a.walls,
            wade_height_fraction: a.wade,
            drown_height_fraction: a.drown,
            drown_after_s: a.drown_after,
            body_reference: r.body,
            adult_energy_reference: r.energy,
            adult_reserve_reference: r.reserve,
            structural_reference: r.body,
            cruise_m_per_s: a.cruise,
            yaw_cap_rad_per_s: a.yaw,
            mouth_reach_body_lengths: a.reach,
            mass_scale: a.mass,
            slot_range_m: a.slot_range,
            taste_reference: a.taste_ref,
            modules: &BELLWING_MODULES,
            // A flyer's controller is sampled every tick: at the walkers' 0.25 s a 1 m/s
            // flyer would go a quarter metre past every decision (a hover at a flower is
            // a centimetre-scale act).
            controller_period_s: FLYER_CONTROLLER_PERIOD_S,
            forward_reference_m: a.cruise * FLYER_CONTROLLER_PERIOD_S,
            taste_cue: "nectar",
            taste_resistances: &BELLWING_TASTE_RESISTANCES,
            visible_classes: &FLYER_VISIBLE_CLASSES,
            ray_pitch_offsets_deg: &FLYER_RAY_PITCH_OFFSETS_DEG,
            cone_range_m: 4.0,
            ..Manifest::browser()
        }
    }

    /// The seedporter (F3): the browser's 37-input layout and three actions, its own
    /// anchors and `Self` references, and a cone that counts the fruit stock in view
    /// (lanternberry stands and latticevine faces in fruit) where the browser's counts
    /// foliage. Interim until P2. Its taste reads fruit in the mouth's reach.
    pub fn seedporter() -> Manifest {
        let a = anchors(Founder::Seedporter);
        let r = SelfReferences::of(Founder::Seedporter);
        Manifest {
            founder: Founder::Seedporter,
            body_length_m: a.length,
            body_width_m: a.width,
            body_height_m: a.height,
            eye_height_fraction: a.eye,
            mouth_ceiling_fraction: a.mouth_ceiling,
            mouth_floor_fraction: a.mouth_floor,
            contact_height_fraction: a.contact,
            step_up_m: a.step_up,
            step_down_m: a.step_down,
            climbs_walls: a.walls,
            wade_height_fraction: a.wade,
            drown_height_fraction: a.drown,
            drown_after_s: a.drown_after,
            body_reference: r.body,
            adult_energy_reference: r.energy,
            adult_reserve_reference: r.reserve,
            structural_reference: r.body,
            cruise_m_per_s: a.cruise,
            yaw_cap_rad_per_s: a.yaw,
            forward_reference_m: a.cruise * CONTROLLER_PERIOD_S,
            mouth_reach_body_lengths: a.reach,
            mass_scale: a.mass,
            slot_range_m: a.slot_range,
            modules: &SEEDPORTER_MODULES,
            menu: crate::intent::menu_of(Founder::Seedporter),
            kin_slots: a.slots[0],
            threat_slots: a.slots[1],
            prey_slots: a.slots[2],
            place_slots: a.slots[3],
            actions: &SPRINT_ACTIONS,
            taste_reference: a.taste_ref,
            taste_cue: "fruit",
            taste_resistances: &SEEDPORTER_TASTE_RESISTANCES,
            cone_counts_fruit: true,
            ..Manifest::browser()
        }
    }

    /// The chorister (roster R3): the lanternjaw's layout on its own anchors and `Self`
    /// references, **without the lure** (Brief R3: the lanternjaw's strike machinery, no
    /// lure). Interim until P2.
    // The lanternjaw's manifest with the walkers' three actions, on the chorister's
    // physiology.
    pub fn chorister() -> Manifest {
        let a = anchors(Founder::Chorister);
        let r = SelfReferences::of(Founder::Chorister);
        Manifest {
            founder: Founder::Chorister,
            body_length_m: a.length,
            body_width_m: a.width,
            body_height_m: a.height,
            eye_height_fraction: a.eye,
            mouth_ceiling_fraction: a.mouth_ceiling,
            mouth_floor_fraction: a.mouth_floor,
            contact_height_fraction: a.contact,
            step_up_m: a.step_up,
            step_down_m: a.step_down,
            climbs_walls: a.walls,
            wade_height_fraction: a.wade,
            drown_height_fraction: a.drown,
            drown_after_s: a.drown_after,
            body_reference: r.body,
            adult_energy_reference: r.energy,
            adult_reserve_reference: r.reserve,
            structural_reference: r.body,
            cruise_m_per_s: a.cruise,
            yaw_cap_rad_per_s: a.yaw,
            forward_reference_m: a.cruise * CONTROLLER_PERIOD_S,
            mouth_reach_body_lengths: a.reach,
            mass_scale: a.mass,
            slot_range_m: a.slot_range,
            modules: &CHORISTER_MODULES,
            menu: crate::intent::menu_of(Founder::Chorister),
            kin_slots: a.slots[0],
            threat_slots: a.slots[1],
            prey_slots: a.slots[2],
            place_slots: a.slots[3],
            actions: &SPRINT_ACTIONS,
            ..Manifest::lanternjaw()
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

    /// The GRU parameter count this manifest's shapes need: `3·H·(I + H + 2) + O·(H + 1)`,
    /// with `H` its [`Manifest::hidden`] and `O` its [`Manifest::outputs`].
    pub fn parameter_count(&self) -> usize {
        gru_parameter_count(self.inputs(), self.hidden, self.outputs())
    }

    /// The network outputs a response carries for this manifest: its actions, then its
    /// option head's logits.
    pub fn outputs(&self) -> usize {
        self.actions.len() + self.option_head
    }

    /// The first v3 slot: everything before it is the lineage's v2 layout, which the
    /// observation-only heuristics read.
    pub fn tail_offset(&self) -> usize {
        self.modules.iter().find(|m| m.tail).map_or_else(|| self.inputs(), |m| m.offset)
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
            "|anchor:eye={}H,mouth=[{},{}H],contact={}H|step:up={},down={},walls={}|wade:{}H|drown:{}H/{}s",
            self.eye_height_fraction,
            // A floor of 0 keeps the text it always had (`[0,…]`).
            if self.mouth_floor_fraction == 0.0 {
                "0".to_string()
            } else {
                format!("{}H", self.mouth_floor_fraction)
            },
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
        // Fixed food references are digest inputs; film capacity is a world rule.
        if let Some(stock) = self.taste_reference {
            let _ = write!(s, "|taste_ref_stock:{}", stock);
        }
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
            if self.cone_banded_to_mouth {
                let _ = write!(s, "|cone_band:mouth");
            }
            if self.cone_counts_fruit {
                let _ = write!(s, "|cone_class:fruit");
            }
        } else {
            let _ = write!(s, "|sectors:none");
        }
        // Contract v3: the network's shape and the blocks' encoding and references.
        let _ = write!(
            s,
            "|hidden:{}|options:{}|v3:{}|v3_ref:slot={},fresh={},carcass={},scent={},tremor={},height={}",
            self.hidden,
            self.option_head,
            V3_ENCODING_RULE,
            self.slot_range_m,
            self.fresh_s,
            self.carcass_range_m,
            SCENT_REFERENCE,
            TREMOR_REFERENCE,
            HEIGHT_REFERENCE_M
        );
        // CONTRACTS: the intent menu and the subject candidates, named only for a menu
        // lineage, so a plain lineage's text (and digest) is what it was.
        if !self.menu.plain() {
            let _ = write!(s, "|intent:v1|menu:");
            for k in 0..self.menu.len() {
                let _ = write!(s, "{},", self.menu.name(k));
            }
            let _ = write!(
                s,
                "|slots:kin={},threat={},prey={},place={}|row:",
                self.kin_slots, self.threat_slots, self.prey_slots, self.place_slots
            );
            for c in crate::subjects::ROW_CHANNELS {
                let _ = write!(s, "{c},");
            }
            let _ = write!(s, "|rows:{}", crate::subjects::CANDIDATE_RULE);
        }
        s
    }

    /// Subject candidate rows `K` per decision sample (0 for a plain lineage).
    pub fn candidates(&self) -> usize {
        self.kin_slots + self.threat_slots + self.prey_slots + self.place_slots
    }

    /// A candidate row's width `F` (0 for a plain lineage).
    pub fn row_width(&self) -> usize {
        if self.menu.plain() { 0 } else { crate::subjects::ROW_WIDTH }
    }

    /// **Why a stored policy digest is refused**, for a loader's error: a menu lineage's
    /// policy made before the intent contract (CONTRACTS) is named as that. No migration.
    pub fn digest_refusal(&self, stored: u64) -> String {
        let mut before = *self;
        before.menu = &crate::intent::PLAIN;
        let pre = fnv1a(before.canonical_text().as_bytes());
        if !self.menu.plain() && stored == pre {
            format!(
                "{} policy digest {stored:#018x} was made before the intent contract (CONTRACTS, \
                 behaviour-AI §11: menus, subject candidates, calls); it is not this build's \
                 {:#018x} and is refused, not migrated: retrain or re-clone it",
                self.founder.name(),
                self.digest()
            )
        } else {
            format!(
                "{} policy digest {stored:#018x} is not this build's {:#018x}: the schemas differ",
                self.founder.name(),
                self.digest()
            )
        }
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
#[cfg(test)]
pub(crate) fn fnv1a_pub(bytes: &[u8]) -> u64 {
    fnv1a(bytes)
}

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
    gru_parameter_count(inputs, HIDDEN, actions)
}

/// The GRU parameter count for `I` inputs, hidden width `H` and `O` outputs:
/// `3·H·(I + H + 2) + O·(H + 1)`.
pub const fn gru_parameter_count(inputs: usize, hidden: usize, outputs: usize) -> usize {
    3 * hidden * (inputs + hidden + 2) + outputs * (hidden + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The v2 layouts the heuristics read, and which every v3 manifest keeps in place.
    fn v2_prefix(founder: Founder) -> &'static [Module] {
        match founder {
            Founder::Blind => &BLIND_MODULES_V2,
            Founder::Capgnawer => &CAPGNAWER_MODULES_V2,
            Founder::RippleSnail => &RIPPLE_SNAIL_MODULES_V2,
            Founder::Bellwing => &BELLWING_MODULES_V2,
            _ => &BROWSER_MODULES_V2,
        }
    }

    /// Contract v3 keeps every lineage's v2 slots where they were and appends its v3 blocks
    /// after them, contiguously: a heuristic reading a v2 slot by name reads the same index.
    #[test]
    fn every_v3_layout_keeps_its_v2_prefix_in_place() {
        for founder in Founder::ALL {
            let m = founder.manifest();
            let prefix = v2_prefix(founder);
            let n = prefix.len();
            assert_eq!(&m.modules[..n], prefix, "{}: the v2 prefix moved", founder.name());
            assert!(m.modules[..n].iter().all(|x| !x.tail));
            assert!(m.modules[n..].iter().all(|x| x.tail), "{}: a v2 module after the tail", founder.name());
            assert_eq!(m.tail_offset(), prefix.last().map_or(0, |x| x.end() + 1));
            assert!(m.inputs() > m.tail_offset(), "{}: no v3 block", founder.name());
            assert!(m.modules.iter().any(|x| x.name == "SelfV3"), "{}", founder.name());
        }
    }

    /// The two shapes the search runtime is compiled for are the manifests' own.
    #[test]
    fn the_search_shapes_are_the_manifests() {
        assert_eq!(Manifest::blind().inputs(), BLIND_INPUTS);
        assert_eq!(Manifest::blind().outputs(), BLIND_OUTPUTS);
        assert_eq!(Manifest::browser().inputs(), BROWSER_INPUTS);
        assert_eq!(Manifest::browser().outputs(), BROWSER_OUTPUTS);
    }

    /// Every lineage's actions start with forward, turn and feed in the manifest's transfer
    /// pair and deadband; the body actions follow, each a lineage's own.
    #[test]
    fn the_actions_are_the_three_then_the_bodys_own() {
        let body = |f: Founder| f.manifest().actions.get(3).map(|a| a.name);
        assert_eq!(body(Founder::Lanternjaw), Some("lure_effort"));
        assert_eq!(body(Founder::Bellwing), Some("lift_effort"));
        assert_eq!(body(Founder::Browser), Some("hunker_effort"));
        assert_eq!(body(Founder::RippleSnail), Some("retract_effort"));
        assert_eq!(body(Founder::Seedporter), Some("sprint_effort"));
        assert_eq!(body(Founder::Chorister), Some("sprint_effort"));
        for f in [Founder::Blind, Founder::Capgnawer, Founder::Loftstrider] {
            assert_eq!(f.manifest().actions.len(), 3, "{}", f.name());
        }
        for m in Founder::ALL.into_iter().map(Founder::manifest) {
            assert!(m.outputs() <= MAX_OUTPUTS);
            assert_eq!(&m.actions[..3], &ACTIONS[..]);
            assert_eq!(m.deadband, ACTION_DEADBAND);
            assert_eq!(m.hidden, HIDDEN);
            assert_eq!(m.option_head, 0);
        }
        assert_eq!(Manifest::blind().cadence_ticks(), 5, "0.25 s at 20 Hz");
    }

    /// The parameter count is `3H(I + H + 2) + O(H + 1)` for every founder's manifest, at its
    /// own hidden width and output count.
    #[test]
    fn the_parameter_count_is_the_gru_formula_for_every_founder() {
        for m in Founder::ALL.into_iter().map(Founder::manifest) {
            let (i, h, o) = (m.inputs(), m.hidden, m.outputs());
            assert_eq!(m.parameter_count(), 3 * h * (i + h + 2) + o * (h + 1), "{}", m.founder.name());
        }
        // The formula reproduces v2's two shapes.
        assert_eq!(gru32_parameter_count(23, 3), 5_571);
        assert_eq!(gru32_parameter_count(37, 3), 6_915);
        let mut wide = Manifest::browser();
        wide.hidden = 64;
        wide.option_head = 5;
        assert_eq!(wide.parameter_count(), 3 * 64 * (72 + 64 + 2) + (4 + 5) * 65);
    }

    /// The digest is FNV-1a over the canonical text, stable, and distinct per founder; the
    /// text names schema 3.
    #[test]
    fn the_digest_is_stable_and_separates_the_founders() {
        let blind = Manifest::blind();
        let browser = Manifest::browser();
        assert_eq!(blind.digest(), blind.digest());
        assert_eq!(blind.digest(), fnv1a(blind.canonical_text().as_bytes()));
        assert!(blind.canonical_text().starts_with("voxel-senses|v3|"));
        let mut seen = std::collections::BTreeSet::new();
        for m in Founder::ALL.into_iter().map(Founder::manifest) {
            assert!(seen.insert(m.digest()), "{} shares a digest", m.founder.name());
        }
        assert_ne!(blind.digest(), browser.digest());
        assert!(blind.canonical_text().contains("|taste:litter:litter=0.2:ground=0.5"));
        assert!(blind.canonical_text().contains(V3_ENCODING_RULE));
    }

    /// A single changed module, action, hidden width, option head or encoding reference
    /// moves the digest.
    #[test]
    fn a_single_changed_manifest_property_moves_the_digest() {
        let baseline = Manifest::browser();
        let before = baseline.digest();
        let moved = |m: Manifest, what: &str| assert_ne!(m.digest(), before, "{what} did not move the digest");

        let mut m = baseline;
        m.modules = &LOFTSTRIDER_MODULES;
        moved(m, "a dropped module");
        let mut m = baseline;
        m.actions = &ACTIONS;
        moved(m, "a dropped action");
        let mut m = baseline;
        m.actions = &SPRINT_ACTIONS;
        moved(m, "a renamed action");
        let mut m = baseline;
        m.hidden = 64;
        moved(m, "the hidden width");
        let mut m = baseline;
        m.option_head = 3;
        moved(m, "the option head");
        let mut m = baseline;
        m.slot_range_m += 1.0;
        moved(m, "the slot range");
        let mut m = baseline;
        m.fresh_s += 1.0;
        moved(m, "the freshness horizon");
        let mut m = baseline;
        m.tunings.chem_tau_s = 2.0;
        moved(m, "a chem tuning");
        let mut m = baseline;
        m.adult_energy_reference = 0.1;
        moved(m, "a Self reference");
        let mut m = baseline;
        m.forward_reference_m = 0.01;
        moved(m, "a movement reference");
    }
}
