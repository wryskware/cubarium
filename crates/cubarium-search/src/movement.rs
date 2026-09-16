//! The spatial-coupling measures: how long a body stays, how often it comes back, how often a
//! cell is grazed out and grows back, and who the bodies are.
//!
//! Workstream A measured one number about space — distinct cells per body per window — and it
//! was enough to *name* cheap wide-ranging movement as a hypothesis and not enough to test it
//! (`design/7_Research/ecology-v1-next-review-2026-09-15.md`, finding 4). This module adds the
//! measures that can: **residence time** says how long a body stays where it is, the
//! **revisit interval** says how long a cell gets before that body returns, and the
//! **per-cell crossing counter** says whether any cell ever completed the
//! depletion → recovery cycle ecology v1's §4.4 reflush was written for.
//!
//! Every definition here is the one written into
//! `design/7_Research/ecology-v1-movement-2026-09-16.md` before the campaign ran, and
//! `crates/cubarium-search/tests/movement_measures.rs` checks each of them on hand-built
//! samples. Nothing in this module reads or writes a world; it is arithmetic over a probe
//! sequence and a field series, which is why it is testable without simulating anything.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A cell counts as depleted below this fraction of its own tick-0 foliage, and recovered
/// above [`RECOVERY_FRACTION`] of it. Re-exported from [`crate::evaluate`] rather than
/// redeclared, so the per-cell counter and A's whole-run counter can never drift apart.
pub const DEPLETION_FRACTION: f64 = crate::evaluate::DEPLETION_FRACTION;
/// See [`DEPLETION_FRACTION`].
pub const RECOVERY_FRACTION: f64 = crate::evaluate::RECOVERY_FRACTION;

/// The two diet-bin edges. `diet < 0.35` is the detrital end, `diet ≥ 0.65` the foliage end,
/// and the middle bin is where the founder skimmer (`diet = 0.60`) sits alone.
pub const DIET_BIN_EDGES: [f64; 2] = [0.35, 0.65];

/// Names for the three diet bins, in index order.
pub const DIET_BINS: [&str; 3] = ["0.00-0.35", "0.35-0.65", "0.65-1.00"];

/// The visual form of the founder skimmer (`crates/cubarium-core/src/config.rs:490-519`:
/// lantern 0, sail 1, mossback 2, skimmer 3). Used only to name the skimmer's loss; the
/// founder roster's form mapping is *measured* at tick 0 in every run, never assumed.
pub const SKIMMER_FORM: u8 = 3;

/// Which diet bin a body's `phenotype.diet` falls in. The edges belong to the bin above, so
/// the three bins are exhaustive and disjoint over `[0, 1]`.
pub fn diet_bin(diet: f64) -> usize {
    if diet < DIET_BIN_EDGES[0] {
        0
    } else if diet < DIET_BIN_EDGES[1] {
        1
    } else {
        2
    }
}

// ---------------------------------------------------------------------------------------
// Visits: the batch definition
// ---------------------------------------------------------------------------------------

/// One **visit**: a maximal run of consecutive probes at which a body was observed in the same
/// cell. A probe at which the body was not observed breaks the run, so the same cell either
/// side of a gap is two visits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Visit {
    pub cell: u16,
    /// The index, inside the window, of the first probe of this visit.
    pub start_probe: u64,
    /// How many consecutive probes the body was seen in this cell.
    pub probes: u64,
}

/// Decompose one body's probe sequence into visits. `probes[i]` is the cell the body was in at
/// probe `i`, or `None` if it was not observed then (not yet born, or already dead).
///
/// This is the **definition**; [`BodyTrack`] is the online form the recorder uses, and a test
/// checks the two agree.
pub fn visits(probes: &[Option<u16>]) -> Vec<Visit> {
    let mut out: Vec<Visit> = Vec::new();
    for (i, slot) in probes.iter().enumerate() {
        let i = i as u64;
        match slot {
            None => continue,
            Some(cell) => match out.last_mut() {
                Some(last) if last.cell == *cell && last.start_probe + last.probes == i => {
                    last.probes += 1;
                }
                _ => out.push(Visit { cell: *cell, start_probe: i, probes: 1 }),
            },
        }
    }
    out
}

/// The residence time of each visit, in ticks, at `probe_ticks` ticks per probe.
///
/// A visit seen on `n` consecutive probes occupied its cell for somewhere in
/// `((n − 1)·probe_ticks, (n + 1)·probe_ticks)`; `n·probe_ticks` is the midpoint of that
/// interval and is the estimator. An occupancy shorter than one probe interval is not
/// resolved and reads as one interval; the final visit in a window is truncated by the window
/// boundary and is still counted.
pub fn residence_ticks(visits: &[Visit], probe_ticks: u64) -> Vec<u64> {
    visits.iter().map(|v| v.probes * probe_ticks).collect()
}

/// The revisit intervals, in ticks: for each cell, the gap between the **start** of one visit
/// to it and the start of the next visit to the same cell. Consecutive probes in one cell are
/// one visit and produce no interval — residence is not revisiting. Ordered by cell, then by
/// visit start, so the result is deterministic.
pub fn revisit_intervals(visits: &[Visit], probe_ticks: u64) -> Vec<u64> {
    let mut starts: BTreeMap<u16, Vec<u64>> = BTreeMap::new();
    for v in visits {
        starts.entry(v.cell).or_default().push(v.start_probe);
    }
    let mut out = Vec::new();
    for (_, mut s) in starts {
        s.sort_unstable();
        for pair in s.windows(2) {
            out.push((pair[1] - pair[0]) * probe_ticks);
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// Visits: the online form the recorder uses
// ---------------------------------------------------------------------------------------

/// One body's visit statistics inside one window, accumulated **online**.
///
/// The recorder cannot keep 1,800 probe slots per body per window, so it feeds probes in as
/// they happen. This produces exactly what [`visits`] + [`residence_ticks`] +
/// [`revisit_intervals`] produce on the same sequence, which a test asserts.
#[derive(Clone, Debug, Default)]
pub struct BodyTrack {
    /// The visit in progress: `(cell, start probe, probes so far, last probe index)`.
    open: Option<(u16, u64, u64, u64)>,
    /// Per cell, the start probe of that cell's most recent **closed or open** visit.
    last_start: BTreeMap<u16, u64>,
    probes_seen: u64,
    visits: u64,
    /// Σ probes over closed visits, so the mean residence is `probe_sum · probe_ticks / visits`.
    probe_sum: u64,
    /// Σ (start − previous start) over revisits, in probes.
    interval_probe_sum: u64,
    intervals: u64,
}

impl BodyTrack {
    /// Record that the body was in `cell` at probe `probe_index` (window-local, increasing).
    pub fn observe(&mut self, probe_index: u64, cell: u16) {
        self.probes_seen += 1;
        match self.open {
            Some((c, start, probes, last)) if c == cell && last + 1 == probe_index => {
                self.open = Some((c, start, probes + 1, probe_index));
                return;
            }
            Some((_, _, probes, _)) => {
                self.probe_sum += probes;
                self.visits += 1;
            }
            None => {}
        }
        if let Some(previous) = self.last_start.insert(cell, probe_index) {
            self.interval_probe_sum += probe_index - previous;
            self.intervals += 1;
        }
        self.open = Some((cell, probe_index, 1, probe_index));
    }

    /// Close the window. The visit in progress is counted, truncated, as the definition says.
    pub fn finish(&mut self) {
        if let Some((_, _, probes, _)) = self.open.take() {
            self.probe_sum += probes;
            self.visits += 1;
        }
    }

    pub fn probes_seen(&self) -> u64 {
        self.probes_seen
    }

    /// Distinct cells the body stood in during the window — A's `cells_per_body_window`
    /// quantity, recomputed here so the two spatial measures come from one traversal.
    pub fn distinct_cells(&self) -> usize {
        self.last_start.len()
    }

    pub fn visit_count(&self) -> u64 {
        self.visits
    }

    /// Mean residence time over this body's visits, or `None` if it had none.
    pub fn mean_residence_ticks(&self, probe_ticks: u64) -> Option<f64> {
        (self.visits > 0).then(|| (self.probe_sum * probe_ticks) as f64 / self.visits as f64)
    }

    /// Mean revisit interval over this body's returns, or `None` if it never returned to a
    /// cell. `None` is not zero: a body that never comes back has no interval at all, and
    /// averaging it in as a zero would say the opposite of what happened.
    pub fn mean_revisit_ticks(&self, probe_ticks: u64) -> Option<f64> {
        (self.intervals > 0)
            .then(|| (self.interval_probe_sum * probe_ticks) as f64 / self.intervals as f64)
    }
}

// ---------------------------------------------------------------------------------------
// The per-cell depletion/recovery crossing counter
// ---------------------------------------------------------------------------------------

/// Foliage depletion and recovery **per cell**, with A's thresholds and A's hysteresis.
///
/// A cell whose tick-0 foliage exceeds `1e-9` is *watched*. Its state starts `Ok`; it crosses
/// to depleted the first probe its foliage falls **below** `deplete × P₀` and back the first
/// probe it rises **above** `recover × P₀`. A **cycle** is a depletion followed later by a
/// recovery in the same cell.
///
/// The totals this counter produces are the same numbers A's whole-run counters produce — it
/// is the same rule, kept per cell instead of only in aggregate.
/// Which way a cell crossed, reported by [`CrossingCounter::observe_reporting`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrossingKind {
    Depleted,
    Recovered,
}

#[derive(Clone, Debug)]
pub struct CrossingCounter {
    p_ref: Vec<f64>,
    watched: Vec<bool>,
    depleted: Vec<bool>,
    depletions: Vec<u32>,
    recoveries: Vec<u32>,
    deplete: f64,
    recover: f64,
}

impl CrossingCounter {
    pub fn new(p_ref: &[f64], deplete: f64, recover: f64) -> Self {
        let watched: Vec<bool> = p_ref.iter().map(|p| *p > 1e-9).collect();
        CrossingCounter {
            p_ref: p_ref.to_vec(),
            depleted: vec![false; p_ref.len()],
            depletions: vec![0; p_ref.len()],
            recoveries: vec![0; p_ref.len()],
            watched,
            deplete,
            recover,
        }
    }

    /// One probe of the foliage field.
    pub fn observe(&mut self, p: &[f64]) {
        self.observe_reporting(p, |_, _| {});
    }

    /// One probe of the foliage field, reporting each crossing as it is counted.
    ///
    /// [`Self::observe`] is this with a closure that does nothing, so the per-cell record in
    /// [`crate::depletion`] and this counter can never apply two different rules: there is one
    /// rule and one place it is written.
    pub fn observe_reporting(&mut self, p: &[f64], mut on: impl FnMut(usize, CrossingKind)) {
        for i in 0..self.p_ref.len().min(p.len()) {
            if !self.watched[i] {
                continue;
            }
            let reference = self.p_ref[i];
            let now = p[i];
            if self.depleted[i] {
                if now > self.recover * reference {
                    self.depleted[i] = false;
                    self.recoveries[i] += 1;
                    on(i, CrossingKind::Recovered);
                }
            } else if now < self.deplete * reference {
                self.depleted[i] = true;
                self.depletions[i] += 1;
                on(i, CrossingKind::Depleted);
            }
        }
    }

    pub fn watched(&self) -> u32 {
        self.watched.iter().filter(|w| **w).count() as u32
    }

    pub fn depletions(&self) -> u64 {
        self.depletions.iter().map(|c| u64::from(*c)).sum()
    }

    pub fn recoveries(&self) -> u64 {
        self.recoveries.iter().map(|c| u64::from(*c)).sum()
    }

    /// Cells that were depleted at least once.
    pub fn cells_depleted(&self) -> u32 {
        self.depletions.iter().filter(|c| **c > 0).count() as u32
    }

    /// Cells that recovered at least once.
    pub fn cells_recovered(&self) -> u32 {
        self.recoveries.iter().filter(|c| **c > 0).count() as u32
    }

    /// Cells that completed at least one depletion → recovery cycle. Equal to
    /// [`Self::cells_recovered`] by construction — a recovery can only follow a depletion —
    /// and reported separately because the equality is the check, not an assumption.
    pub fn cells_cycled(&self) -> u32 {
        self.depletions
            .iter()
            .zip(&self.recoveries)
            .filter(|(d, r)| **d > 0 && **r > 0)
            .count() as u32
    }

    /// The most cycles any one cell completed.
    pub fn max_cycles(&self) -> u32 {
        self.recoveries.iter().copied().max().unwrap_or(0)
    }

    /// Cells currently below the depletion threshold.
    pub fn depleted_now(&self) -> u32 {
        self.depleted.iter().filter(|d| **d).count() as u32
    }

    /// The whole per-run summary, as it is written into a row.
    pub fn summary(&self) -> Crossings {
        let watched = f64::from(self.watched().max(1));
        Crossings {
            cells_watched: self.watched(),
            depletions: self.depletions(),
            recoveries: self.recoveries(),
            depletions_per_watched_cell: self.depletions() as f64 / watched,
            recoveries_per_watched_cell: self.recoveries() as f64 / watched,
            cells_depleted: self.cells_depleted(),
            cells_recovered: self.cells_recovered(),
            cells_cycled: self.cells_cycled(),
            max_cycles_in_a_cell: self.max_cycles(),
            depleted_at_end: self.depleted_now(),
        }
    }
}

/// The per-run crossing summary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Crossings {
    pub cells_watched: u32,
    pub depletions: u64,
    pub recoveries: u64,
    pub depletions_per_watched_cell: f64,
    pub recoveries_per_watched_cell: f64,
    pub cells_depleted: u32,
    pub cells_recovered: u32,
    pub cells_cycled: u32,
    pub max_cycles_in_a_cell: u32,
    pub depleted_at_end: u32,
}

// ---------------------------------------------------------------------------------------
// Aggregation over bodies and windows
// ---------------------------------------------------------------------------------------

/// The spatial measures for one window, or pooled over a run's windows.
///
/// Every figure is a mean **over qualifying bodies**, weighted equally per body, which is the
/// weighting A's `cells_per_body_window` already uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Spatial {
    /// Bodies that met the 90 % coverage rule in this window (or summed over windows).
    pub bodies: u64,
    pub cells_per_body: f64,
    pub visits_per_body: f64,
    pub mean_residence_ticks: f64,
    pub mean_residence_seconds: f64,
    /// Mean over the bodies that returned to a cell at least once.
    pub mean_revisit_ticks: f64,
    pub mean_revisit_seconds: f64,
    pub bodies_with_revisit: u64,
    pub bodies_without_revisit: u64,
}

/// Accumulates [`Spatial`] over bodies as their windows close.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpatialAccumulator {
    bodies: u64,
    cells: f64,
    visits: f64,
    residence: f64,
    revisit: f64,
    with_revisit: u64,
}

impl SpatialAccumulator {
    /// Add one qualifying body's window.
    pub fn add(&mut self, track: &BodyTrack, probe_ticks: u64) {
        self.bodies += 1;
        self.cells += track.distinct_cells() as f64;
        self.visits += track.visit_count() as f64;
        self.residence += track.mean_residence_ticks(probe_ticks).unwrap_or(0.0);
        if let Some(r) = track.mean_revisit_ticks(probe_ticks) {
            self.revisit += r;
            self.with_revisit += 1;
        }
    }

    pub fn merge(&mut self, other: &SpatialAccumulator) {
        self.bodies += other.bodies;
        self.cells += other.cells;
        self.visits += other.visits;
        self.residence += other.residence;
        self.revisit += other.revisit;
        self.with_revisit += other.with_revisit;
    }

    pub fn bodies(&self) -> u64 {
        self.bodies
    }

    pub fn finish(&self, dt: f64) -> Spatial {
        let n = self.bodies.max(1) as f64;
        let r = self.with_revisit.max(1) as f64;
        let residence = self.residence / n;
        let revisit = if self.with_revisit == 0 { 0.0 } else { self.revisit / r };
        Spatial {
            bodies: self.bodies,
            cells_per_body: if self.bodies == 0 { 0.0 } else { self.cells / n },
            visits_per_body: if self.bodies == 0 { 0.0 } else { self.visits / n },
            mean_residence_ticks: if self.bodies == 0 { 0.0 } else { residence },
            mean_residence_seconds: if self.bodies == 0 { 0.0 } else { residence * dt },
            mean_revisit_ticks: revisit,
            mean_revisit_seconds: revisit * dt,
            bodies_with_revisit: self.with_revisit,
            bodies_without_revisit: self.bodies - self.with_revisit,
        }
    }
}

// ---------------------------------------------------------------------------------------
// The variety census
// ---------------------------------------------------------------------------------------

/// What a body is, fixed when it is first seen and never revised: `diet` and `form` are set at
/// conception, and `form` is immutable under mutation (`crates/cubarium-core/src/genome.rs`),
/// so a census keyed on this moves only through birth and death.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CensusKey {
    pub form: u8,
    pub diet_bin: u8,
    pub guild: u8,
}

/// The stores a body carried at the **last probe before it died** — at most one probe interval
/// stale, and reported as an estimate for that reason.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stores {
    pub energy: f64,
    pub reserve: f64,
    pub structure: f64,
    /// `energy + reserve_energy_density · reserve`: the store the body's bill is drawn against.
    pub usable: f64,
    pub hunger: f64,
}

/// A running sum of [`Stores`], so a mean can be reported per cause and per census cell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StoreSum {
    pub samples: u64,
    pub energy: f64,
    pub reserve: f64,
    pub structure: f64,
    pub usable: f64,
    pub hunger: f64,
}

impl StoreSum {
    pub fn add(&mut self, s: &Stores) {
        self.samples += 1;
        self.energy += s.energy;
        self.reserve += s.reserve;
        self.structure += s.structure;
        self.usable += s.usable;
        self.hunger += s.hunger;
    }

    /// The mean, or all zeros with `samples = 0` if nothing was ever added.
    pub fn mean(&self) -> Stores {
        let n = self.samples.max(1) as f64;
        Stores {
            energy: self.energy / n,
            reserve: self.reserve / n,
            structure: self.structure / n,
            usable: self.usable / n,
            hunger: self.hunger / n,
        }
    }
}

/// One `(form, diet bin, guild)` cell of the census.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CensusCell {
    /// Bodies in this cell at tick 0.
    pub opening: u64,
    pub births: u64,
    pub deaths: u64,
    /// Starvation, age, collapse, predation — [`crate::metrics::DEATH_CAUSES`] order.
    pub deaths_by_cause: [u64; 4],
    /// Σ `age_ticks` over the deaths, from the life event, exact rather than sampled.
    pub lifetime_ticks: u64,
    pub mean_lifetime_seconds: f64,
    pub alive_final: u64,
    pub terminal: StoreSum,
    pub mean_terminal: Stores,
}

/// One serialised census row: the key, spelled out, and its cell.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CensusRow {
    pub form: u8,
    pub diet_bin: u8,
    pub guild: u8,
    #[serde(flatten)]
    pub cell: CensusCell,
}

/// The whole census, plus what it is asked to answer about the skimmer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Census {
    /// Only cells that were ever occupied.
    pub cells: Vec<CensusRow>,
    /// The founder roster as measured at tick 0: `form -> count`, so the note's form mapping
    /// is a measurement of this build rather than a reading of the config.
    pub opening_by_form: [u64; 5],
    pub opening_by_diet_bin: [u64; 3],
    /// The last tick at which a form-3 body was observed alive, or `None` if one is alive at
    /// the horizon.
    pub skimmer_last_alive_tick: Option<u64>,
    pub skimmer_alive_final: u64,
    /// The cause of the last form-3 death, if any form-3 body died.
    pub skimmer_last_death_cause: Option<String>,
    pub skimmer_deaths_by_cause: [u64; 4],
}

/// Accumulates the census while a run proceeds.
#[derive(Clone, Debug, Default)]
pub struct CensusBuilder {
    cells: BTreeMap<CensusKey, CensusCell>,
    opening_by_form: [u64; 5],
    opening_by_diet_bin: [u64; 3],
    skimmer_last_alive_tick: Option<u64>,
    skimmer_last_death_cause: Option<String>,
    skimmer_deaths_by_cause: [u64; 4],
}

impl CensusBuilder {
    /// A founder, alive at tick 0.
    pub fn found(&mut self, key: CensusKey) {
        self.cells.entry(key).or_default().opening += 1;
        self.opening_by_form[usize::from(key.form).min(4)] += 1;
        self.opening_by_diet_bin[usize::from(key.diet_bin).min(2)] += 1;
    }

    /// A body born during the run.
    pub fn born(&mut self, key: CensusKey) {
        self.cells.entry(key).or_default().births += 1;
    }

    /// A body's death: its key, the cause slot, its exact age in ticks, and the stores it was
    /// last observed with.
    pub fn died(&mut self, key: CensusKey, cause: usize, cause_name: &str, age_ticks: u64, stores: Option<&Stores>) {
        let cell = self.cells.entry(key).or_default();
        cell.deaths += 1;
        cell.deaths_by_cause[cause.min(3)] += 1;
        cell.lifetime_ticks += age_ticks;
        if let Some(s) = stores {
            cell.terminal.add(s);
        }
        if key.form == SKIMMER_FORM {
            self.skimmer_deaths_by_cause[cause.min(3)] += 1;
            self.skimmer_last_death_cause = Some(cause_name.to_string());
        }
    }

    /// A probe at which at least one form-3 body was alive.
    pub fn skimmer_seen(&mut self, tick: u64) {
        self.skimmer_last_alive_tick = Some(tick);
    }

    /// Close the census with the live bodies at the horizon.
    pub fn finish(mut self, alive: &[CensusKey], dt: f64) -> Census {
        let mut skimmer_alive_final = 0;
        for key in alive {
            self.cells.entry(*key).or_default().alive_final += 1;
            if key.form == SKIMMER_FORM {
                skimmer_alive_final += 1;
            }
        }
        let cells = self
            .cells
            .into_iter()
            .map(|(key, mut cell)| {
                cell.mean_lifetime_seconds = if cell.deaths == 0 {
                    0.0
                } else {
                    cell.lifetime_ticks as f64 * dt / cell.deaths as f64
                };
                cell.mean_terminal = cell.terminal.mean();
                CensusRow { form: key.form, diet_bin: key.diet_bin, guild: key.guild, cell }
            })
            .collect();
        Census {
            cells,
            opening_by_form: self.opening_by_form,
            opening_by_diet_bin: self.opening_by_diet_bin,
            skimmer_last_alive_tick: if skimmer_alive_final > 0 {
                None
            } else {
                self.skimmer_last_alive_tick
            },
            skimmer_alive_final,
            skimmer_last_death_cause: self.skimmer_last_death_cause,
            skimmer_deaths_by_cause: self.skimmer_deaths_by_cause,
        }
    }
}

// ---------------------------------------------------------------------------------------
// Workstream I: the founder grazer's brood, and the net energy margin from E's ledger
// ---------------------------------------------------------------------------------------

/// How many visual forms the founder roster can carry, so a per-form array is a fixed size.
pub const FORMS: usize = 5;

/// Completed broods by **founder** parents, per visual form.
///
/// A *completed brood* is a `LifeEvent::Birth` whose parent is a tick-0 founder: the core emits
/// that event when gestation completes and the child is committed, so a birth event **is** a
/// completed brood and nothing is inferred from a reserve or an age. The founder grazer
/// (form 0) is the ladder's gate; the glider (form 1) is reported beside it because
/// `fast-leaf` x 0.0018 already satisfied a "first herbivore brood" gate through the glider
/// alone (Astra's finding 1).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FounderBroods {
    /// Founders alive at tick 0, per form. Measured, never assumed.
    pub founders_by_form: [u64; FORMS],
    /// Births whose parent is a founder of that form.
    pub broods_by_form: [u64; FORMS],
    /// The tick of the first such birth, per form.
    pub first_brood_tick_by_form: [Option<u64>; FORMS],
    /// How many distinct founders of that form produced at least one.
    pub parents_by_form: [u64; FORMS],
}

impl Default for FounderBroods {
    fn default() -> Self {
        FounderBroods {
            founders_by_form: [0; FORMS],
            broods_by_form: [0; FORMS],
            first_brood_tick_by_form: [None; FORMS],
            parents_by_form: [0; FORMS],
        }
    }
}

impl FounderBroods {
    /// A founder of `form` was alive at tick 0.
    pub fn found(&mut self, form: u8) {
        let f = usize::from(form).min(FORMS - 1);
        self.founders_by_form[f] += 1;
    }

    /// A founder of `form` completed a brood at `tick`. `first_parent` says whether this is
    /// the first brood by that particular founder, so parents are counted once.
    pub fn brood(&mut self, form: u8, tick: u64, first_parent: bool) {
        let f = usize::from(form).min(FORMS - 1);
        self.broods_by_form[f] += 1;
        if self.first_brood_tick_by_form[f].is_none() {
            self.first_brood_tick_by_form[f] = Some(tick);
        }
        if first_parent {
            self.parents_by_form[f] += 1;
        }
    }

    /// The ladder's gate quantity: did the founder grazer rig complete a brood at all?
    pub fn grazer_bred(&self) -> bool {
        self.broods_by_form[0] > 0
    }
}

/// The net energy margin of one `(form, diet bin)` group, from E's per-body ledger.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MarginBin {
    pub form: u8,
    pub diet_bin: u8,
    pub bodies: u64,
    pub deaths: u64,
    pub alive: u64,
    /// `e_food_in - e_owed` (e), meaned over bodies. See [`MarginAccumulator::add`].
    pub margin_mean: f64,
    /// The same divided by each body's own recorded seconds, then meaned (e/s).
    pub margin_rate_mean: f64,
    pub food_energy_in_mean: f64,
    /// What the body **owed**: `bill_total + other + growth + reproduction` (e).
    pub energy_owed_mean: f64,
    pub bill_total_mean: f64,
    /// `bill_total - bill_paid`: what it could not raise (e).
    pub bill_unpaid_mean: f64,
    /// `(translation + turn) / bill_total`, meaned over bodies with a positive bill.
    pub motor_share_mean: f64,
    pub served_total_mean: f64,
    pub recorded_seconds_mean: f64,
}

/// Every group's margin, plus what the ledger itself reported about its own completeness.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Margins {
    pub ledger_on: bool,
    pub bodies: u64,
    /// Closed records the ledger dropped because more than its cap accumulated undrained.
    /// Never silently zero.
    pub records_dropped: u64,
    /// The mean margin over every body, whatever its group.
    pub margin_mean: f64,
    pub bins: Vec<MarginBin>,
}

/// Accumulates [`Margins`] as bodies' ledger records close and at the horizon.
#[derive(Clone, Debug, Default)]
pub struct MarginAccumulator {
    bins: BTreeMap<(u8, u8), MarginSum>,
    bodies: u64,
    margin_sum: f64,
    dropped: u64,
}

#[derive(Clone, Copy, Debug, Default)]
struct MarginSum {
    bodies: u64,
    deaths: u64,
    alive: u64,
    margin: f64,
    margin_rate: f64,
    food_in: f64,
    owed: f64,
    bill_total: f64,
    bill_unpaid: f64,
    motor_share: f64,
    motor_share_bodies: u64,
    served: f64,
    seconds: f64,
}

impl MarginAccumulator {
    /// One body's complete ledger record.
    ///
    /// ```text
    /// e_food_in = sum_channels ( battery_credit + e_r * reserve_credit )
    ///             + gut_battery_credit + e_r * gut_reserve_credit
    /// e_owed    = bill_total + other_energy_paid + growth_energy + reproduction_energy
    /// margin    = e_food_in - e_owed
    /// ```
    ///
    /// `bill_total` and not `bill_paid`: what the body **owed** is the question, and a
    /// starving body's unpaid bill is exactly its deficit. Reserve material is valued at its
    /// stated density `e_r` rather than at the 0.8 the later oxidation returns, so `margin` is
    /// an **upper bound** on the energy the body could actually have spent.
    pub fn add(
        &mut self,
        key: CensusKey,
        budget: &cubarium_core::BodyBudget,
        reserve_energy_density: f64,
        now_tick: u64,
        dt: f64,
        alive: bool,
    ) {
        let e_r = reserve_energy_density;
        let food_in = budget.battery_credit_total()
            + e_r * budget.reserve_credit_total()
            + budget.gut_battery_credit
            + e_r * budget.gut_reserve_credit;
        let owed = budget.bill_total
            + budget.other_energy_paid
            + budget.growth_energy
            + budget.reproduction_energy;
        let margin = food_in - owed;
        let closed = budget.closed_tick.unwrap_or(now_tick);
        let seconds = closed.saturating_sub(budget.opened_tick) as f64 * dt;

        let bin = self.bins.entry((key.form, key.diet_bin)).or_default();
        bin.bodies += 1;
        if alive {
            bin.alive += 1;
        } else {
            bin.deaths += 1;
        }
        bin.margin += margin;
        bin.margin_rate += if seconds > 0.0 { margin / seconds } else { 0.0 };
        bin.food_in += food_in;
        bin.owed += owed;
        bin.bill_total += budget.bill_total;
        bin.bill_unpaid += budget.bill_total - budget.bill_paid;
        if budget.bill_total > 0.0 {
            bin.motor_share +=
                (budget.motor_translation_billed + budget.motor_turn_billed) / budget.bill_total;
            bin.motor_share_bodies += 1;
        }
        bin.served += budget.served_total();
        bin.seconds += seconds;

        self.bodies += 1;
        self.margin_sum += margin;
    }

    /// Closed ledger records the world reported dropping.
    pub fn note_dropped(&mut self, dropped: u64) {
        self.dropped += dropped;
    }

    pub fn finish(self, ledger_on: bool) -> Margins {
        let bins = self
            .bins
            .into_iter()
            .map(|((form, diet_bin), sum)| {
                let n = sum.bodies.max(1) as f64;
                let m = sum.motor_share_bodies.max(1) as f64;
                MarginBin {
                    form,
                    diet_bin,
                    bodies: sum.bodies,
                    deaths: sum.deaths,
                    alive: sum.alive,
                    margin_mean: sum.margin / n,
                    margin_rate_mean: sum.margin_rate / n,
                    food_energy_in_mean: sum.food_in / n,
                    energy_owed_mean: sum.owed / n,
                    bill_total_mean: sum.bill_total / n,
                    bill_unpaid_mean: sum.bill_unpaid / n,
                    motor_share_mean: if sum.motor_share_bodies == 0 {
                        0.0
                    } else {
                        sum.motor_share / m
                    },
                    served_total_mean: sum.served / n,
                    recorded_seconds_mean: sum.seconds / n,
                }
            })
            .collect();
        Margins {
            ledger_on,
            bodies: self.bodies,
            records_dropped: self.dropped,
            margin_mean: if self.bodies == 0 {
                0.0
            } else {
                self.margin_sum / self.bodies as f64
            },
            bins,
        }
    }
}

// ---------------------------------------------------------------------------------------
// The row
// ---------------------------------------------------------------------------------------

/// Late-window range and residence for one visual form, so "is the skimmer lost through its
/// habitat?" has a spatial number beside it rather than only a demographic one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FormSpatial {
    pub form: u8,
    #[serde(flatten)]
    pub spatial: Spatial,
}

/// Everything this workstream adds to one evaluated run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Movement {
    /// The price this run was charged, echoed at the top level so a row is self-describing.
    pub move_cost: f64,
    pub probe_ticks: u64,
    /// One per closed window, in order.
    pub windows: Vec<Spatial>,
    /// The last closed window: the same interval as A's late window.
    pub late: Option<Spatial>,
    /// All windows pooled, weighted equally per body-window.
    pub whole_run: Spatial,
    /// Late-window range and residence by visual form.
    pub late_by_form: Vec<FormSpatial>,
    pub crossings: Crossings,
    pub census: Census,
    /// Terminal stores by death cause, in [`crate::metrics::DEATH_CAUSES`] order.
    pub terminal_by_cause: [StoreSum; 4],
    /// The mean of [`Margins::margin_mean`] over every prey body, or `None` on a build or a
    /// run whose ledger was off. F's rows carry `null` here because workstream E's per-organism
    /// budget accumulator had not landed on `main` when F ran; workstream I turns it on.
    pub net_energy_margin_per_body: Option<f64>,
    /// Whether this run recorded per-body budgets (workstream I).
    #[serde(default)]
    pub ledger_on: bool,
    /// Completed broods by tick-0 founders, per form: the ladder's gate (workstream I).
    #[serde(default)]
    pub founder_broods: FounderBroods,
    /// Net energy margin by `(form, diet bin)` from E's ledger, absent when it was off.
    #[serde(default)]
    pub margins: Option<Margins>,
    /// The per-depleted-cell record and its four-way classification (workstream I).
    #[serde(default)]
    pub depletion: Option<crate::depletion::DepletionSummary>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The per-cell counter must reproduce the aggregate rule A already ships, because it is
    /// the same rule: a whole-run total from this counter equals the total a single pair of
    /// scalars would produce on the same series.
    #[test]
    fn the_per_cell_counter_totals_match_a_single_aggregate_counter() {
        let reference = vec![1.0, 2.0, 0.0, 0.5];
        let series = vec![
            vec![1.0, 2.0, 0.0, 0.5],
            vec![0.1, 1.9, 3.0, 0.5],
            vec![0.9, 0.4, 0.0, 0.1],
            vec![0.9, 1.5, 0.0, 0.3],
            vec![0.2, 0.2, 0.0, 0.4],
        ];

        let mut counter = CrossingCounter::new(&reference, DEPLETION_FRACTION, RECOVERY_FRACTION);
        for p in &series {
            counter.observe(p);
        }

        // The aggregate form, written out exactly as `evaluate::Recorder::probe` writes it.
        let mut depleted = vec![false; reference.len()];
        let (mut depletions, mut recoveries) = (0u64, 0u64);
        for p in &series {
            for i in 0..reference.len() {
                if reference[i] <= 1e-9 {
                    continue;
                }
                if depleted[i] {
                    if p[i] > RECOVERY_FRACTION * reference[i] {
                        depleted[i] = false;
                        recoveries += 1;
                    }
                } else if p[i] < DEPLETION_FRACTION * reference[i] {
                    depleted[i] = true;
                    depletions += 1;
                }
            }
        }
        assert_eq!(counter.depletions(), depletions);
        assert_eq!(counter.recoveries(), recoveries);
        assert_eq!(counter.depleted_now(), depleted.iter().filter(|d| **d).count() as u32);
    }

    /// A cell that recovered must have been depleted first, so the two counts agree by
    /// construction. Stated as a test rather than assumed by the reporting.
    #[test]
    fn a_recovery_implies_an_earlier_depletion_in_the_same_cell() {
        let mut c = CrossingCounter::new(&[1.0, 1.0, 1.0], DEPLETION_FRACTION, RECOVERY_FRACTION);
        c.observe(&[0.1, 1.0, 0.1]);
        c.observe(&[0.9, 1.0, 0.1]);
        assert_eq!(c.cells_recovered(), c.cells_cycled());
        assert_eq!(c.cells_depleted(), 2);
        assert_eq!(c.cells_cycled(), 1);
    }

    /// The accumulator's "no return" case is a missing observation, not a zero interval.
    #[test]
    fn bodies_without_a_revisit_do_not_pull_the_mean_interval_to_zero() {
        let dt = 0.05;
        let mut acc = SpatialAccumulator::default();

        let mut returner = BodyTrack::default();
        returner.observe(0, 1);
        returner.observe(1, 2);
        returner.observe(2, 1); // returns to cell 1 after 2 probes = 40 ticks
        returner.finish();

        let mut wanderer = BodyTrack::default();
        for i in 0..3u64 {
            wanderer.observe(i, 10 + i as u16);
        }
        wanderer.finish();

        acc.add(&returner, 20);
        acc.add(&wanderer, 20);
        let s = acc.finish(dt);
        assert_eq!(s.bodies, 2);
        assert_eq!(s.bodies_with_revisit, 1);
        assert_eq!(s.bodies_without_revisit, 1);
        assert_eq!(s.mean_revisit_ticks, 40.0, "averaged over the one body that returned");
        assert_eq!(s.cells_per_body, 2.5, "2 cells and 3 cells");
    }

    /// The census keys on what a body *was*, and the skimmer's loss tick is only reported when
    /// no skimmer survived.
    #[test]
    fn a_surviving_skimmer_has_no_loss_tick() {
        let skimmer = CensusKey { form: SKIMMER_FORM, diet_bin: 1, guild: 2 };
        let mut b = CensusBuilder::default();
        b.found(skimmer);
        b.skimmer_seen(1_000);
        let census = b.finish(&[skimmer], 0.05);
        assert_eq!(census.skimmer_alive_final, 1);
        assert_eq!(census.skimmer_last_alive_tick, None, "it is not lost, so it has no loss tick");
        assert_eq!(census.opening_by_form[usize::from(SKIMMER_FORM)], 1);

        let mut b = CensusBuilder::default();
        b.found(skimmer);
        b.skimmer_seen(1_000);
        b.died(skimmer, 0, "starvation", 1_020, Some(&Stores::default()));
        let census = b.finish(&[], 0.05);
        assert_eq!(census.skimmer_alive_final, 0);
        assert_eq!(census.skimmer_last_alive_tick, Some(1_000));
        assert_eq!(census.skimmer_last_death_cause.as_deref(), Some("starvation"));
        assert_eq!(census.cells[0].cell.mean_lifetime_seconds, 1_020.0 * 0.05);
    }
}
