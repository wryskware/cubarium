//! Optional care: bounded Feed / Rain / Clean commands
//! (`design/7_Research/care-contract-2026-09-12.md`, revision 2).
//!
//! Nothing here is ambient ecology. With no command ever admitted the world executes the
//! old operations in the old order with no new RNG draws: [`CareState::default`] is inert,
//! [`CareState::showers`] is empty, and [`crate::water::step`] is handed `None` for the
//! manual rain rate so it runs the pre-care arithmetic unchanged.
//!
//! Doses are *server-defined constants*, not configuration: adding a `WorldConfig` field
//! would change the postcard shape of a nested type and defeat the schema 7 migration.
//! Every dose below is a **total over the footprint**, never a per-cell amount.

use serde::{Deserialize, Serialize};

use cubarium_surface::{CELL_COUNT, CellId, FACE_EXTENT, Face, FieldGraph, SurfacePoint, cell_of};

use crate::DT;
use crate::config::WorldConfig;

/// Total material per feed, summed over the footprint (m).
pub const FEED_MATERIAL: f64 = 3.0;
/// Feed footprint radius: the target cell plus its graph neighbours within one hop.
pub const FEED_HOPS: usize = 1;
/// Cumulative net material a world may be fed (m). A successful clean gives some back.
pub const FEED_ALLOWANCE: f64 = 30.0;

/// Total water depth per shower, summed over the footprint and the whole envelope (d).
pub const RAIN_DEPTH_TOTAL: f64 = 4.0;
/// Rain footprint radius in graph hops.
pub const RAIN_HOPS: usize = 2;
/// Shower duration in ticks: exactly this many discrete samples, `k = 0..=RAIN_TICKS - 1`.
pub const RAIN_TICKS: u32 = 120;
/// `RAIN_TICKS` as a length, for the envelope array.
pub const RAIN_SAMPLES: usize = RAIN_TICKS as usize;

/// Maximum litter removed per clean, summed over the footprint (m).
pub const CLEAN_MATERIAL: f64 = 2.0;
/// At most this fraction of any one cell's `D` may leave in one clean.
pub const CLEAN_FRACTION: f64 = 0.5;
/// Clean footprint radius in graph hops.
pub const CLEAN_HOPS: usize = 1;

/// Weight sums and dose totals are floating point; this is the slack the contract allows
/// (`Σ w_c = 1` and `Σ_k e_k · DT = 1` to within 1e-12).
pub const WEIGHT_TOLERANCE: f64 = 1e-12;

/// Slack on the allowance boundary. The ledger books the *actual* f64 sum of the per-cell
/// deposits, which misses the nominal dose by a few ulps: a rim footprint has weights
/// `0.4, 0.2, 0.2, 0.2` and sums `3 · w_c` to `3.0000000000000004`. Without this slack the
/// nominal 30 m allowance would buy ten interior feeds but only nine rim feeds, which is a
/// rounding artefact and not a rule. The admission bound is the nominal dose; the ledgers
/// stay exact.
pub const ALLOWANCE_TOLERANCE: f64 = 1e-9;

/// How much of a dose one command asks for, in **permille of the standard dose**
/// (`design/7_Research/adjustable-care-dose-handoff-2026-09-13.md`).
///
/// A bounded total multiplier on the nominal amount a command moves — never a per-cell
/// multiplier, and never a way to queue several commands as one. The integer is the identity:
/// two requests carrying 1000 are the same request, with no float-equivalence ambiguity.
///
/// [`CareDose::STANDARD`] is what every command meant before this existed, and it is applied by
/// a documented identity branch, so a standard dose is the **same arithmetic, bit for bit**,
/// as the pre-dose builds performed. Only a nonstandard dose multiplies anything.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct CareDose(u16);

impl CareDose {
    /// Smallest and largest dose a command may carry, in permille.
    pub const MIN_PERMILLE: u16 = 250;
    pub const MAX_PERMILLE: u16 = 2000;
    /// The dose every pre-dose command meant, and what an omitted field means.
    pub const STANDARD_PERMILLE: u16 = 1000;
    /// The wire version of this capability, for a host that advertises it.
    pub const VERSION: u32 = 1;
    /// The standard dose.
    pub const STANDARD: CareDose = CareDose(Self::STANDARD_PERMILLE);

    /// A dose from permille, or the reason it is not one. Out-of-range values are **refused**,
    /// never clamped: a request for twice the maximum is a mistake to report, not a request for
    /// the maximum.
    pub fn new(permille: u16) -> Result<CareDose, String> {
        if !(Self::MIN_PERMILLE..=Self::MAX_PERMILLE).contains(&permille) {
            return Err(format!(
                "dose_permille {permille} is outside {}..={}",
                Self::MIN_PERMILLE,
                Self::MAX_PERMILLE
            ));
        }
        Ok(CareDose(permille))
    }

    pub fn permille(self) -> u16 {
        self.0
    }

    pub fn is_standard(self) -> bool {
        self.0 == Self::STANDARD_PERMILLE
    }

    /// `nominal · permille / 1000`, and **exactly `nominal`** at the standard dose.
    ///
    /// The identity branch is deliberate: `x * 1000.0 / 1000.0` is not bit-identical to `x` for
    /// every `x`, and a world that takes no nonstandard dose must step exactly as it did before
    /// this field existed.
    pub fn scale(self, nominal: f64) -> f64 {
        if self.is_standard() { nominal } else { nominal * f64::from(self.0) / 1000.0 }
    }

    /// Range check for a dose that came off a snapshot rather than through [`CareDose::new`].
    pub fn validate(self, what: &str) -> Result<(), String> {
        CareDose::new(self.0).map(|_| ()).map_err(|e| format!("{what}: {e}"))
    }
}

impl Default for CareDose {
    fn default() -> Self {
        CareDose::STANDARD
    }
}

/// `FEED_ENERGY_DENSITY`: the world's `detritus.energy_cap`, so fed crumbs are fully
/// charged and fully edible to existing scavenging diets while `De ≤ energy_cap · D` is
/// preserved exactly (a cell gains `rho · x` of `De` for `x` of `D`).
pub fn feed_energy_density(config: &WorldConfig) -> f64 {
    config.detritus.energy_cap
}

/// What a care command asks the world to do.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CareKind {
    Feed,
    Rain,
    Clean,
}

impl CareKind {
    /// The lowercase word used in the host's journal and HTTP payloads.
    pub fn as_str(self) -> &'static str {
        match self {
            CareKind::Feed => "feed",
            CareKind::Rain => "rain",
            CareKind::Clean => "clean",
        }
    }
}

/// A canonical surface point: `face` in `0..5`, `u` and `v` in `[0, 64)`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CareTarget {
    pub face: u8,
    pub u: f64,
    pub v: f64,
}

impl CareTarget {
    /// The field cell the target names, or `None` when the face or the chart coordinates
    /// are out of range. Never panics on hostile input.
    pub fn resolve(&self) -> Option<CellId> {
        let face = Face::from_index(self.face)?;
        if !self.u.is_finite() || !self.v.is_finite() {
            return None;
        }
        if self.u < 0.0 || self.u >= FACE_EXTENT || self.v < 0.0 || self.v >= FACE_EXTENT {
            return None;
        }
        Some(cell_of(&SurfacePoint::new(face, self.u, self.v)))
    }
}

/// One admitted command. `apply_after_tick = B` names the boundary with exactly `B`
/// completed ticks: it is applied when `world.tick() == B`, before the step that produces
/// `B + 1`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CareCommand {
    pub seq: u64,
    pub apply_after_tick: u64,
    pub kind: CareKind,
    pub target: CareTarget,
    /// How much of the standard amount to move. Transient: this type is the host→world wire,
    /// never part of a snapshot. A command whose dose is out of range is rejected on
    /// admission, spending its sequence like any other rejection.
    pub dose: CareDose,
}

impl CareCommand {
    /// A command at the standard dose — what every pre-dose caller meant.
    pub fn standard(seq: u64, apply_after_tick: u64, kind: CareKind, target: CareTarget) -> CareCommand {
        CareCommand { seq, apply_after_tick, kind, target, dose: CareDose::STANDARD }
    }
}

/// The contract's `q`: what actually happened, in the world's own units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CareApplied {
    /// Material added to `D` (m), the actual f64 sum over the footprint.
    pub material_in: f64,
    /// Chemical energy added to `De` (e).
    pub energy_in: f64,
    /// Water depth (d). For a rain command this is the *scheduled* total over the whole
    /// shower; the depth actually delivered is booked tick by tick in
    /// [`CareState::rain_depth_in`].
    pub water_depth: f64,
    /// Material removed from `D` (m).
    pub material_out: f64,
    /// Chemical energy exported with it (e). Not heat dissipated inside the world.
    pub energy_out: f64,
    /// Footprint size in cells.
    pub cells: u32,
    /// For rain, the tick at which the last sample has been delivered.
    pub ends_tick: Option<u64>,
}

/// The result of one command. A rejection carries the reason verbatim.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CareOutcome {
    Applied(CareApplied),
    Partial(CareApplied),
    Rejected(String),
}

impl CareOutcome {
    /// `"applied" | "partial" | "rejected"`, for the host's journal.
    pub fn as_str(&self) -> &'static str {
        match self {
            CareOutcome::Applied(_) => "applied",
            CareOutcome::Partial(_) => "partial",
            CareOutcome::Rejected(_) => "rejected",
        }
    }

    pub fn applied(&self) -> Option<&CareApplied> {
        match self {
            CareOutcome::Applied(q) | CareOutcome::Partial(q) => Some(q),
            CareOutcome::Rejected(_) => None,
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            CareOutcome::Rejected(r) => Some(r),
            _ => None,
        }
    }
}

/// What the world reports back for one command. `tick` is the world tick the command was
/// evaluated at, which equals `apply_after_tick` on every path but a wrong-boundary refusal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CareReceipt {
    pub seq: u64,
    pub tick: u64,
    pub outcome: CareOutcome,
}

/// A shower in progress. Cells and weights are resolved once, at admission, so a reload
/// delivers exactly the samples the original run had left.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActiveShower {
    /// The command that started it.
    pub seq: u64,
    /// The boundary it was admitted at; sample `k` falls in the step producing `B + 1 + k`.
    pub apply_after_tick: u64,
    /// Footprint cells as raw `CellId` indices.
    pub cells: Vec<u16>,
    /// The matching normalized footprint weights, `Σ = 1` to within `WEIGHT_TOLERANCE`.
    pub weights: Vec<f64>,
    /// Samples already delivered; the next sample is `envelope[delivered]`.
    pub delivered: u32,
    /// The dose this shower was admitted with, in permille, **persisted** so a restart
    /// delivers its remaining samples at the amount that was actually asked for. Changing a
    /// panel selection cannot change a shower already falling. A world migrated from a
    /// pre-dose schema opens its in-flight rain at [`CareDose::STANDARD`], which is what it
    /// was.
    pub dose_permille: u16,
}

impl ActiveShower {
    /// The dose this shower is delivering.
    pub fn dose(&self) -> CareDose {
        CareDose(self.dose_permille)
    }
}

/// Everything care adds to `WorldState`. Appended last; nothing else is reordered.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CareState {
    /// The highest contiguous sequence number the world has consumed.
    pub admitted_seq: u64,
    /// At most one entry (the contract allows one active shower).
    pub showers: Vec<ActiveShower>,
    /// Cumulative ledgers, all in the identities of `design/m2-world-spec.md` as extended
    /// by the care contract.
    pub feed_material_in: f64,
    pub feed_energy_in: f64,
    pub rain_depth_in: f64,
    pub clean_material_out: f64,
    pub clean_energy_out: f64,
    /// Net material fed so far: feeding adds, a successful clean gives back, never below 0.
    pub allowance_used: f64,
}

impl CareState {
    /// Range checks after decode, called from `WorldState::validate`. `tick` is the state's
    /// own tick, which no shower may have been admitted after.
    pub fn validate(&self, tick: u64) -> Result<(), String> {
        for (name, v) in [
            ("feed_material_in", self.feed_material_in),
            ("feed_energy_in", self.feed_energy_in),
            ("rain_depth_in", self.rain_depth_in),
            ("clean_material_out", self.clean_material_out),
            ("clean_energy_out", self.clean_energy_out),
            ("allowance_used", self.allowance_used),
        ] {
            if !v.is_finite() {
                return Err(format!("care {name} is not finite"));
            }
            if v < 0.0 {
                return Err(format!("care {name} is negative: {v}"));
            }
        }
        // The booked allowance is the actual f64 sum of the per-cell deposits, which may
        // miss the nominal dose by rounding; the bound is the dose, not the sum.
        if self.allowance_used > FEED_ALLOWANCE + ALLOWANCE_TOLERANCE {
            return Err(format!("care allowance_used {} exceeds {FEED_ALLOWANCE}", self.allowance_used));
        }
        if self.showers.len() > 1 {
            return Err(format!("care holds {} showers, at most one is allowed", self.showers.len()));
        }
        for s in &self.showers {
            if s.cells.is_empty() || s.cells.len() != s.weights.len() {
                return Err(format!(
                    "care shower {} has {} cells and {} weights",
                    s.seq,
                    s.cells.len(),
                    s.weights.len()
                ));
            }
            if s.cells.len() > CELL_COUNT {
                return Err(format!("care shower {} covers more cells than the surface has", s.seq));
            }
            // The persisted dose is range-checked like every other decoded value: a shower
            // whose dose is outside the documented bounds is refused, never clamped.
            s.dose().validate(&format!("care shower {}", s.seq))?;
            // A footprint is a set: a repeated cell would take its share twice.
            let mut seen = vec![false; CELL_COUNT];
            for &c in &s.cells {
                let i = usize::from(c);
                if i >= CELL_COUNT {
                    return Err(format!("care shower {} names cell {c}, out of range", s.seq));
                }
                if seen[i] {
                    return Err(format!("care shower {} names cell {c} twice", s.seq));
                }
                seen[i] = true;
            }
            let mut sum = 0.0;
            for &w in &s.weights {
                if !w.is_finite() || w <= 0.0 || w > 1.0 {
                    return Err(format!("care shower {} has weight {w}", s.seq));
                }
                sum += w;
            }
            // The contract's tolerance, not a looser one: a crafted shower must not be able
            // to deliver more than `RAIN_DEPTH_TOTAL`.
            if (sum - 1.0).abs() > WEIGHT_TOLERANCE {
                return Err(format!("care shower {} weights sum to {sum}", s.seq));
            }
            if s.delivered >= RAIN_TICKS {
                return Err(format!("care shower {} has delivered {} of {RAIN_TICKS}", s.seq, s.delivered));
            }
            if s.apply_after_tick > tick {
                return Err(format!("care shower {} starts at {} after tick {tick}", s.seq, s.apply_after_tick));
            }
            // Progress is not free-standing: one sample is delivered per step, so a shower
            // admitted at `B` and alive at `tick` has delivered exactly `tick − B`. A
            // snapshot claiming otherwise would replay rain the water state already has.
            let elapsed = tick - s.apply_after_tick;
            if u64::from(s.delivered) != elapsed {
                return Err(format!(
                    "care shower {} has delivered {} samples but {elapsed} ticks have passed since {}",
                    s.seq, s.delivered, s.apply_after_tick
                ));
            }
            if s.seq == 0 || s.seq > self.admitted_seq {
                return Err(format!("care shower {} was never admitted (cursor {})", s.seq, self.admitted_seq));
            }
        }
        Ok(())
    }
}

/// The footprint of a target: the cell and every cell reachable within `hops` hops on the
/// field graph, breadth-first, each cell exactly once (so a seam target never doubles a
/// dose and the open rim simply has fewer cells).
///
/// Raw weight by hop distance is `1 / (1 + hop)`, normalized over the cells that exist so
/// `Σ w_c = 1` to within [`WEIGHT_TOLERANCE`]. Returned in `CellId` index order, so the
/// order the doses are summed in is a property of the surface and not of the search.
pub fn footprint(graph: &FieldGraph, center: CellId, hops: usize) -> Vec<(CellId, f64)> {
    let mut seen = vec![false; CELL_COUNT];
    seen[center.index()] = true;
    let mut raw: Vec<(CellId, f64)> = vec![(center, 1.0)];
    let mut frontier = vec![center];
    for hop in 1..=hops {
        let mut next: Vec<CellId> = Vec::new();
        for cell in &frontier {
            for n in graph.neighbors(*cell).iter().flatten() {
                if !seen[n.index()] {
                    seen[n.index()] = true;
                    next.push(*n);
                }
            }
        }
        next.sort_unstable_by_key(|c| c.index());
        let w = 1.0 / (1.0 + hop as f64);
        raw.extend(next.iter().map(|c| (*c, w)));
        frontier = next;
    }
    raw.sort_unstable_by_key(|(c, _)| c.index());
    let total: f64 = raw.iter().map(|(_, w)| *w).sum();
    raw.into_iter().map(|(c, w)| (c, w / total)).collect()
}

/// The shower's temporal envelope: exactly [`RAIN_TICKS`] samples with
/// `e_k = (1 − cos(2π (k + 0.5) / 120)) / S`, `S = Σ_k (1 − cos(2π (k + 0.5) / 120)) · DT`,
/// so `Σ_k e_k · DT = 1` to within [`WEIGHT_TOLERANCE`]. Raised cosine: the shower starts
/// and ends at (almost) nothing rather than switching on.
pub fn rain_envelope() -> [f64; RAIN_SAMPLES] {
    let mut e = [0.0f64; RAIN_SAMPLES];
    let mut raw_sum = 0.0;
    for (k, slot) in e.iter_mut().enumerate() {
        *slot = 1.0 - (std::f64::consts::TAU * (k as f64 + 0.5) / RAIN_SAMPLES as f64).cos();
        raw_sum += *slot;
    }
    let s = raw_sum * DT;
    for slot in e.iter_mut() {
        *slot /= s;
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_envelope_integrates_to_one_second_and_starts_and_ends_small() {
        let e = rain_envelope();
        assert_eq!(e.len(), 120);
        let total: f64 = e.iter().sum::<f64>() * DT;
        assert!((total - 1.0).abs() < WEIGHT_TOLERANCE, "envelope integrates to {total}");
        assert!(e.iter().all(|&x| x > 0.0 && x.is_finite()));
        // A raised cosine: the ends are the smallest samples and the middle the largest.
        assert!(e[0] < e[60] && e[119] < e[60]);
        assert!((e[0] - e[119]).abs() < 1e-12, "the envelope is not symmetric");
    }

    #[test]
    fn an_interior_footprint_has_the_documented_cells_and_weights() {
        let graph = FieldGraph::new();
        let center = CellId::new(Face::Front, 8, 8);
        let fp = footprint(&graph, center, FEED_HOPS);
        assert_eq!(fp.len(), 5, "an interior 1-hop footprint is the cell and four neighbours");
        let sum: f64 = fp.iter().map(|(_, w)| w).sum();
        assert!((sum - 1.0).abs() < WEIGHT_TOLERANCE, "weights sum to {sum}");
        let w_center = fp.iter().find(|(c, _)| *c == center).expect("the centre is in it").1;
        // Raw weights 1 + 4·(1/2) = 3, so the centre keeps a third.
        assert!((w_center - 1.0 / 3.0).abs() < 1e-15, "{w_center}");

        let fp2 = footprint(&graph, center, RAIN_HOPS);
        assert_eq!(fp2.len(), 13, "an interior 2-hop footprint is a diamond of 13 cells");
        let sum2: f64 = fp2.iter().map(|(_, w)| w).sum();
        assert!((sum2 - 1.0).abs() < WEIGHT_TOLERANCE, "weights sum to {sum2}");
        // Index order, so the same surface always sums the doses in the same order.
        assert!(fp2.windows(2).all(|p| p[0].0.index() < p[1].0.index()));
    }

    #[test]
    fn a_footprint_never_repeats_a_cell_across_a_seam_and_always_normalizes() {
        let graph = FieldGraph::new();
        for (name, center) in [
            ("seam", CellId::new(Face::Front, 15, 8)),
            ("rim", CellId::new(Face::Front, 8, 15)),
            ("corner", CellId::new(Face::Front, 15, 15)),
            ("top", CellId::new(Face::Top, 0, 0)),
        ] {
            for hops in [FEED_HOPS, RAIN_HOPS] {
                let fp = footprint(&graph, center, hops);
                let mut ids: Vec<u16> = fp.iter().map(|(c, _)| c.0).collect();
                ids.sort_unstable();
                let before = ids.len();
                ids.dedup();
                assert_eq!(ids.len(), before, "{name} at {hops} hops repeats a cell");
                let sum: f64 = fp.iter().map(|(_, w)| w).sum();
                assert!((sum - 1.0).abs() < WEIGHT_TOLERANCE, "{name} at {hops} hops sums to {sum}");
            }
        }
    }

    #[test]
    fn a_target_resolves_only_inside_the_charts() {
        assert_eq!(CareTarget { face: 0, u: 0.0, v: 0.0 }.resolve(), Some(CellId::new(Face::Front, 0, 0)));
        assert_eq!(CareTarget { face: 4, u: 63.9, v: 63.9 }.resolve(), Some(CellId::new(Face::Top, 15, 15)));
        assert_eq!(CareTarget { face: 5, u: 1.0, v: 1.0 }.resolve(), None);
        assert_eq!(CareTarget { face: 0, u: 64.0, v: 1.0 }.resolve(), None);
        assert_eq!(CareTarget { face: 0, u: -1e-9, v: 1.0 }.resolve(), None);
        assert_eq!(CareTarget { face: 0, u: f64::NAN, v: 1.0 }.resolve(), None);
    }

    #[test]
    fn a_default_care_state_is_inert_and_valid() {
        let care = CareState::default();
        assert_eq!(care.admitted_seq, 0);
        assert!(care.showers.is_empty());
        assert_eq!(care.allowance_used, 0.0);
        care.validate(0).expect("the default state is valid");
    }
}
