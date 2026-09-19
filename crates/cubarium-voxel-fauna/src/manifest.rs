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
    /// Body length in metres. Cruise speed is `1 BL/s`.
    pub body_length_m: f64,
    /// Footprint width in metres: half the body length for these two founders.
    pub body_width_m: f64,
    /// Organic-mass reference the founder's `Self` channels are normalized against.
    pub body_reference: f64,
    /// Cruise speed in metres per second (`1 BL/s`).
    pub cruise_m_per_s: f64,
    /// Yaw cap in radians per second.
    pub yaw_cap_rad_per_s: f64,
    /// Mouth/feed reach, in body lengths.
    pub mouth_reach_body_lengths: f64,
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
            cruise_m_per_s: 0.125,
            yaw_cap_rad_per_s: 2.0,
            mouth_reach_body_lengths: 0.25,
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
            cruise_m_per_s: 0.25,
            yaw_cap_rad_per_s: 2.0,
            mouth_reach_body_lengths: 0.25,
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
        self.sector_centres_deg.len() * self.ray_yaw_offsets_deg.len()
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
        let t = &self.tunings;
        let _ = write!(
            s,
            "|tune:contact_binary={},chem_sat={},chem_tau={},trend_scale={},light_ref={},proximity={}",
            t.contact_binary, t.chem_saturation, t.chem_tau_s, t.trend_scale_s, t.light_reference,
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
        assert_eq!((MODULE_CHEM_LITTER.offset, MODULE_CHEM_LITTER.end()), (18, 20));
        assert_eq!((MODULE_LIGHT.offset, MODULE_LIGHT.end()), (21, 22));

        let browser = Manifest::browser();
        assert_eq!(browser.inputs(), 37, "browser manifest width");
        assert_eq!(browser.modules.len(), 5);
        assert_eq!((MODULE_CONE3.offset, MODULE_CONE3.end()), (18, 36));
        assert_eq!(browser.ray_count(), 27, "3 sectors x 3 x 3 rays");
        assert_eq!(blind.ray_count(), 0);
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
            assert_eq!((m.actions[1].name, m.actions[1].transfer), ("turn_effort", Transfer::Tanh));
            assert_eq!((m.actions[2].name, m.actions[2].transfer), ("feed_effort", Transfer::Sigmoid));
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
    /// separates two founders that differ in a single module.
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
    }
}