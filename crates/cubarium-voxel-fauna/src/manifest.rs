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
//! `neural::gru::parameter_count`. The runtime that accepts these shapes is P1-D; this is
//! the number it must accept (5,571 and 6,915).

use std::fmt::Write;

use serde::{Deserialize, Serialize};

/// The fixed hidden width every phase-one founder's GRU uses.
pub const HIDDEN: usize = 32;

/// The schema-version token folded into every manifest's canonical text. A change to the
/// meaning of an existing slot bumps this and invalidates old digests.
pub const SCHEMA_VERSION: u32 = 1;

/// Which phase-one founder this is: a lineage identity, not a controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Founder {
    /// The blind litter feeder (littershredder): `Self`, `Contact(4)`, `Wet`, `Taste(1)`,
    /// `Chem(litter)` and `Light` — **23 inputs**.
    Blind,
    /// The sighted foliage browser (frondgrazer founder): the same shared prefix plus
    /// `Cone(3, foliage/body)` — **37 inputs**.
    Browser,
}

impl Founder {
    pub const ALL: [Founder; 2] = [Founder::Blind, Founder::Browser];
    pub const COUNT: usize = Founder::ALL.len();

    /// Index into a per-founder array, in [`Founder::ALL`] order: stable, like the
    /// flora's and the fauna's own species index.
    pub fn index(self) -> usize {
        match self {
            Founder::Blind => 0,
            Founder::Browser => 1,
        }
    }

    /// The lineage name: what a command line or report calls this founder.
    pub fn name(self) -> &'static str {
        match self {
            Founder::Blind => "littershredder",
            Founder::Browser => "frondgrazer",
        }
    }

    /// The short role word the plan uses.
    pub fn role(self) -> &'static str {
        match self {
            Founder::Blind => "blind",
            Founder::Browser => "browser",
        }
    }

    /// This founder's fixed manifest.
    pub fn manifest(self) -> Manifest {
        match self {
            Founder::Blind => Manifest::blind(),
            Founder::Browser => Manifest::browser(),
        }
    }
}

/// How a raw network output in `[low, high]` is produced from an unbounded logit. The
/// adapter itself (P1-B) is identical in training and viewing; this only names it.
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
    /// Body length in metres, **as the trained contract recorded it**. The cruise speed
    /// below was set at `1 BL/s` against it.
    ///
    /// **Not read for geometry since 2026-09-22**
    /// (`design/handoffs/voxel-body-anchors-2026-09-22.md`, decisions §1). The animal's
    /// real dimensions are its lineage's adult dimensions on the
    /// [`FounderPhysiology`](crate::FounderPhysiology), scaled by
    /// `(body / body_max)^(1/3)`, and they live there precisely because this field is
    /// in [`Manifest::canonical_text`]: moving it would refuse the shipped centres. It
    /// stays as the record of what the centres in `crates/cubarium/assets/policies`
    /// were trained against, and package 5's retrain is where the two are reconciled.
    pub body_length_m: f64,
    /// Footprint width in metres as the trained contract recorded it: half the body
    /// length for these two founders. **Not read for geometry** — see
    /// [`Manifest::body_length_m`].
    pub body_width_m: f64,
    /// Organic-mass reference the founder's `Self` channels are normalized against.
    pub body_reference: f64,
    /// Fixed `Self` normalization references (plan, "Exact starting manifests"): an adult's
    /// energy and reserve, and the total structure the interval's structural loss is
    /// divided by. A slice of the founder's initial stock, not a population statistic.
    pub adult_energy_reference: f64,
    pub adult_reserve_reference: f64,
    pub structural_reference: f64,
    /// Cruise speed in metres per second (`1 BL/s`).
    pub cruise_m_per_s: f64,
    /// Yaw cap in radians per second.
    pub yaw_cap_rad_per_s: f64,
    /// `resolved_forward` and `resolved_turn` are normalized against **reference ×
    /// controller interval** — one controller period of cruise or yaw — not the rate.
    pub forward_reference_m: f64,
    pub turn_reference_rad: f64,
    /// Mouth/feed reach, in body lengths, as the trained contract recorded it. **Not
    /// read for geometry** — the live reach is `mouth_reach_length_fraction × length`
    /// on the physiology, at the same 0.25 — see [`Manifest::body_length_m`].
    pub mouth_reach_body_lengths: f64,
    /// How many whole voxels above its own head layer the mouth could take food from
    /// under the rule of 2026-09-21 (`design/handoffs/voxel-browser-reach-2026-09-21.md`).
    ///
    /// **Not read since 2026-09-22.** The mouth is a physical band
    /// `[0, 1.33 × body height]` over the standing surface
    /// ([`crate::Body::mouth_layers`]; decisions §2), which for the adult browser is
    /// 0.249375 m — less than the one 0.25 m voxel this field claimed, and two cells on
    /// a 0.125 m grid, which is the same air. The field stays because it is in
    /// [`Manifest::canonical_text`] whenever it is non-zero: removing it would move the
    /// browser's digest and refuse its shipped centre for nothing.
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
}

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

const CHEM_LITTER_CHANNELS: [&str; 3] = ["response", "trend", "valid"];

/// The shredder's one diffused cue channel.
///
/// **The id says `litter`; since 2026-09-22 the field carries detritus** — litter,
/// carrion and glowcap cap tissue, the shredder's three foods (decisions §3,
/// `design/handoffs/voxel-diets-2026-09-22.md`; the field is
/// [`crate::senses`]'s `DetritusField`). The name, the slot, the width and the channel
/// order are unchanged and stay unchanged, because they are inside the trained-policy
/// digest and the shipped centres must keep loading: renaming the module would refuse
/// them, and this package's rule is that no sensing *shape* moves.
///
/// So this is a **meaning change on an unchanged channel**, and the scheduled retrain
/// (package 5) owns it. On a world holding only litter the value is what it always was.
/// `crates/cubarium/assets/policies/README.md` records it beside the other two.
const MODULE_CHEM_LITTER: Module = Module {
    name: "Chem(litter)",
    offset: 18,
    width: 3,
    channels: &CHEM_LITTER_CHANNELS,
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
    MODULE_CHEM_LITTER,
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

/// Fixed action deadband. `abs(turn)` is compared, so it is symmetric.
pub const ACTION_DEADBAND: f64 = 0.05;

/// Browser sector centres, degrees from forward.
pub const BROWSER_SECTOR_CENTRES_DEG: [f64; 3] = [-60.0, 0.0, 60.0];
/// Per-sector ray yaw offsets, degrees.
pub const BROWSER_RAY_YAW_OFFSETS_DEG: [f64; 3] = [-30.0, 0.0, 30.0];
/// Per-sector ray pitch offsets, degrees.
pub const BROWSER_RAY_PITCH_OFFSETS_DEG: [f64; 3] = [-20.0, 0.0, 20.0];
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

/// The browser's mouth mapping: pliable foliage, stiff wood, firm ground.
const BROWSER_TASTE_RESISTANCES: [(&str, f64); 3] =
    [("foliage", 0.3), ("wood", 0.8), ("ground", 0.5)];

impl Manifest {
    /// The blind littershredder: 23 inputs, no eyes. Body 0.125 m, cruise 0.125 m/s.
    pub fn blind() -> Manifest {
        Manifest {
            schema_version: SCHEMA_VERSION,
            founder: Founder::Blind,
            modules: &BLIND_MODULES,
            actions: &ACTIONS,
            deadband: ACTION_DEADBAND,
            controller_period_s: 0.25,
            body_length_m: 0.125,
            body_width_m: 0.0625,
            body_reference: 0.0125,
            adult_energy_reference: 0.0375,
            adult_reserve_reference: 0.00625,
            structural_reference: 0.0125,
            cruise_m_per_s: 0.125,
            yaw_cap_rad_per_s: 2.0,
            forward_reference_m: 0.03125,
            turn_reference_rad: 0.5,
            mouth_reach_body_lengths: 0.25,
            mouth_reach_up_voxels: 0,
            taste_cue: "litter",
            taste_resistances: &BLIND_TASTE_RESISTANCES,
            tunings: BLIND_TUNINGS,
            visible_classes: &[],
            sector_centres_deg: &[],
            ray_yaw_offsets_deg: &[],
            ray_pitch_offsets_deg: &[],
            cone_range_m: 0.0,
        }
    }

    /// The sighted browser: 37 inputs with a three-sector cone. Body 0.25 m, cruise
    /// 0.25 m/s, 2 m ray range and 27 rays.
    pub fn browser() -> Manifest {
        Manifest {
            schema_version: SCHEMA_VERSION,
            founder: Founder::Browser,
            modules: &BROWSER_MODULES,
            actions: &ACTIONS,
            deadband: ACTION_DEADBAND,
            controller_period_s: 0.25,
            body_length_m: 0.25,
            body_width_m: 0.125,
            body_reference: 0.05,
            adult_energy_reference: 0.15,
            adult_reserve_reference: 0.025,
            structural_reference: 0.05,
            cruise_m_per_s: 0.25,
            yaw_cap_rad_per_s: 2.0,
            forward_reference_m: 0.0625,
            turn_reference_rad: 0.5,
            mouth_reach_body_lengths: 0.25,
            mouth_reach_up_voxels: 1,
            taste_cue: "foliage",
            taste_resistances: &BROWSER_TASTE_RESISTANCES,
            tunings: BROWSER_TUNINGS,
            visible_classes: &BROWSER_VISIBLE_CLASSES,
            sector_centres_deg: &BROWSER_SECTOR_CENTRES_DEG,
            ray_yaw_offsets_deg: &BROWSER_RAY_YAW_OFFSETS_DEG,
            ray_pitch_offsets_deg: &BROWSER_RAY_PITCH_OFFSETS_DEG,
            cone_range_m: 2.0,
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
            "|deadband:{}|cadence:{}|body_len:{}|body_width:{}|body_ref:{}|cruise:{}|yaw:{}|mouth_bl:{}",
            self.deadband,
            self.controller_period_s,
            self.body_length_m,
            self.body_width_m,
            self.body_reference,
            self.cruise_m_per_s,
            self.yaw_cap_rad_per_s,
            self.mouth_reach_body_lengths
        );
        // Written only when the mouth actually lifts: a zero reach is the behaviour every
        // earlier build had, so its canonical text — and its digest — stay exactly as they
        // were. A lineage that can lift declares it, and its old centres are refused.
        if self.mouth_reach_up_voxels > 0 {
            let _ = write!(s, "|mouth_up:{}", self.mouth_reach_up_voxels);
        }
        let _ = write!(
            s,
            "|self_ref:energy={},reserve={},structural={}|move_ref:forward={},turn={}",
            self.adult_energy_reference,
            self.adult_reserve_reference,
            self.structural_reference,
            self.forward_reference_m,
            self.turn_reference_rad
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
        let t = (self.controller_period_s * f64::from(cubarium_voxel::TICK_HZ)).round();
        if !t.is_finite() || t < 1.0 {
            1
        } else {
            t as u64
        }
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
            (MODULE_CHEM_LITTER.offset, MODULE_CHEM_LITTER.end()),
            (18, 20)
        );
        assert_eq!((MODULE_LIGHT.offset, MODULE_LIGHT.end()), (21, 22));

        let browser = Manifest::browser();
        assert_eq!(browser.inputs(), 37, "browser manifest width");
        assert_eq!(browser.modules.len(), 5);
        assert_eq!((MODULE_CONE3.offset, MODULE_CONE3.end()), (18, 36));
        assert_eq!(browser.ray_count(), 27, "3 sectors x 3 x 3 rays");
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
        for m in Founder::ALL.into_iter().map(Founder::manifest) {
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

    /// The counts P1-D must accept: 5,571 and 6,915.
    #[test]
    fn the_parameter_counts_are_the_contracts() {
        assert_eq!(Manifest::blind().parameter_count(), 5_571);
        assert_eq!(Manifest::browser().parameter_count(), 6_915);
        assert_eq!(gru32_parameter_count(23, 3), 5_571);
        assert_eq!(gru32_parameter_count(37, 3), 6_915);
    }

    /// The digest is stable across calls, is exactly FNV-1a over the canonical text, and
    /// separates two founders that differ in a single module. The schema-significant
    /// constants Astra named — Self references, the movement references, the taste cue and
    /// its resistance mapping — are all part of the canonical text.
    #[test]
    fn the_digest_is_stable_and_separates_the_founders() {
        let blind = Manifest::blind();
        let browser = Manifest::browser();
        assert_eq!(blind.digest(), blind.digest());
        assert_eq!(browser.digest(), browser.digest());
        assert_eq!(blind.digest(), fnv1a(blind.canonical_text().as_bytes()));
        assert_ne!(blind.digest(), browser.digest());
        assert!(blind.canonical_text().contains("Chem(litter)"));
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
    /// protects meaning, so P1-C cannot invent a constant the digest does not cover.
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
