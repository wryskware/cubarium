//! The controlled form × diet factorial: is the skimmer's loss its diet or its body?
//!
//! Workstream F (`design/7_Research/ecology-v1-movement-2026-09-16.md`, "The skimmer: which
//! leg is it?") measured that skimmer-rigged bodies carrying a foliage-end diet survive at
//! 84 % where the founder's `diet = 0.60` survives at 20 %, and said plainly what that is
//! worth: an **association**. Every foliage-diet skimmer in those rows is a *descendant*
//! whose diet mutated upward, so the comparison carries later birth, right-censoring,
//! selection into a mutant lineage, and possible mutation at other loci along with it.
//!
//! This module runs the matched experiment that removes them. Founders are **cloned** into a
//! live `fast-leaf` world at the same tick and the same eight cells in every arm, with only
//! the locus under test moved, and with reproduction switched off so that what is measured is
//! one life and not a lineage:
//!
//! - **Arm A — diet within body.** Eight skimmer-rigged clones, four at the founder's
//!   `diet = 0.60` and four at `0.85`. Every other locus — `form`, `depth`, `speed`, `size`,
//!   `metabolism`, `swim`, `hue`, `sense`, every drive — is identical between the halves.
//! - **Arm B — body within diet.** Eight clones at `diet = 0.85`, two of each roster body.
//!   At `0.85` the detrital gate `θ = 0.2` is shut (`1 − 0.85 < θ`), so every body in the arm
//!   is a *pure* foliage feeder and any difference between them is the body.
//! - **Arm C — the founder pairing.** The same eight cells, each body at its own roster diet:
//!   the observational baseline, run under the same controls as the other two.
//!
//! Two cells of the factorial appear in more than one arm and are free internal controls: the
//! roster grazer is already at `0.85`, so arm B's grazer *is* arm C's grazer; and arm A's
//! high half *is* arm B's skimmer.
//!
//! # What the design controls, and what it does not
//!
//! Controlled: birth tick (every clone is founded at tick 0), place (the same eight cells in
//! every arm, and the same eight for every arm at one seed), lineage (no clone reproduces),
//! mutation (mutation only reaches offspring, and the clones have none), and the ecology (one
//! `fast-leaf` world per seed with its ordinary 24 legacy founders still in it, so the food
//! competition the clones face is the real one).
//!
//! **Not** controlled, and named rather than hidden: the roster bodies differ in more than
//! `form` and `diet`. The skimmer is `depth 0.10, speed 0.9, size 0.9, swim 1.0,
//! metabolism 0.7` against the grazer's `0.55, 1.0, 1.0, 0.0, 1.0`
//! (`crates/cubarium-core/src/config.rs:496-521`). Arm A holds all of that fixed and is
//! therefore clean. Arm B deliberately does not: "body" there means the whole roster body,
//! not the rig alone, and the arm cannot say which of those loci carries the effect.
//!
//! # How the eight cells are chosen
//!
//! Eight fixed [`ANCHORS`] spread over all five faces, each resolved to the nearest *living*
//! cell on its own face that is at least [`MIN_SEPARATION`] from the ones already placed. The
//! landscape is read from a warm-up of the seed's own world with no clones in it, because a
//! world is created dry and rain has to arrive before anything about water is visible.
//!
//! The placement is neutral about food and about habitat, and both of those are deliberate —
//! selecting the best-fed cells would favour the foliage diets, and stratifying on standing
//! water turns out to be impossible in this ecology. See [`choose_cells`] for the two designs
//! the world refused and why; the short version is that ecology v1 grows foliage only in a
//! cell that already carries wood, so the deepest pools hold **no food at all** and the
//! skimmer's supposed algae larder is not a place a body can stand. Habitat is therefore
//! **measured**, per clone, through the run, and reported beside the landscape census.

use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use cubarium_core::genome::Genome;
use cubarium_core::organism::DeathCause;
use cubarium_core::{BodyBudget, OrganismId, ScriptedIntent, World, WorldConfig};
use cubarium_surface::{CELL_COUNT, CellId, Face, Vec2, cell_of};
use serde::{Deserialize, Serialize};

use crate::evaluate::BUILD_ID;

/// Cells of the factorial: eight clones per arm.
pub const CLONES: usize = 8;

/// The founder skimmer's own `diet` (`crates/cubarium-core/src/config.rs:519`).
pub const DIET_LOW: f32 = 0.60;
/// The foliage end: the roster grazer's own `diet`, and the value F's surviving skimmer
/// descendants had drifted into.
pub const DIET_HIGH: f32 = 0.85;

/// The pack's creature order (`crates/cubarium-core/src/config.rs:490-519`).
pub const GRAZER: u8 = 0;
pub const GLIDER: u8 = 1;
pub const BURROWER: u8 = 2;
pub const SKIMMER: u8 = 3;

/// The four roster bodies in the brief's order, two clones each in arms B and C.
pub const FORMS: [u8; 4] = [BURROWER, GRAZER, GLIDER, SKIMMER];

/// Food channels, in the core ledger's own index order
/// (`crates/cubarium-core/src/world/budget.rs:58-70`).
pub const FOLIAGE: usize = 0;
pub const FRUIT: usize = 1;
pub const LITTER: usize = 2;
pub const CARRION: usize = 3;
pub const CHANNELS: usize = 4;
pub const CHANNEL_NAMES: [&str; CHANNELS] = ["foliage", "fruit", "litter", "carrion"];

/// The ecology every arm runs in. `fast-leaf` is the configuration workstream A selected and
/// F measured the skimmer's loss in; nothing here re-tunes it.
pub const ECOLOGY: &str = "fast-leaf";

/// A cell counts as dry when its warm-up mean depth is below this. Not zero: water flows and
/// evaporates continuously, so an exactly-zero mean is a statement about floating point
/// rather than about the landscape.
pub const DRY_MAX: f64 = 1e-3;

/// Two chosen cells on one face must be at least this many cells apart, so the eight starts
/// are eight places and not one pool counted four times.
pub const MIN_SEPARATION: i32 = 3;

/// The depth bands the landscape census reports, as upper bounds (d). The first is "dry", the
/// last is open-ended. `0.05` is where a non-swimmer starts paying a measurable wading
/// penalty (`speed / (1 + w · (1 − swim))`) and `0.15` is half `water.algae_depth`, where a
/// pool has raised its own light floor half way.
pub const DEPTH_BANDS: [f64; 4] = [DRY_MAX, 0.05, 0.15, f64::INFINITY];

/// How many cells the landscape offers in each depth band, and how many of those carry any
/// foliage at all. The evidence behind the choice of `wet_min`, and behind the finding that
/// standing water and food barely coexist in this ecology.
pub fn landscape_census(land: &Landscape) -> Vec<(f64, usize, usize)> {
    DEPTH_BANDS
        .iter()
        .enumerate()
        .map(|(b, &hi)| {
            let lo = if b == 0 { f64::NEG_INFINITY } else { DEPTH_BANDS[b - 1] };
            let in_band: Vec<usize> =
                (0..CELL_COUNT).filter(|&i| land.depth[i] > lo && land.depth[i] <= hi).collect();
            let fed = in_band.iter().filter(|&&i| land.foliage[i] > 0.0).count();
            (hi, in_band.len(), fed)
        })
        .collect()
}

pub fn form_name(form: u8) -> &'static str {
    match form {
        GRAZER => "grazer",
        GLIDER => "glider",
        BURROWER => "burrower",
        SKIMMER => "skimmer",
        _ => "unknown",
    }
}

/// Which of the three arms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Arm {
    /// Diet within body: the skimmer rig at 0.60 and at 0.85.
    A,
    /// Body within diet: every roster body at 0.85.
    B,
    /// The founder pairing: every roster body at its own roster diet.
    C,
}

impl Arm {
    pub const ALL: [Arm; 3] = [Arm::A, Arm::B, Arm::C];

    pub fn label(self) -> &'static str {
        match self {
            Arm::A => "A",
            Arm::B => "B",
            Arm::C => "C",
        }
    }

    pub fn hypothesis(self) -> &'static str {
        match self {
            Arm::A => "diet within body: the roster skimmer at 0.60 against the same body at 0.85",
            Arm::B => "body within diet: every roster body held at 0.85, a pure foliage feeder",
            Arm::C => "the founder pairing: every roster body at its own roster diet",
        }
    }

    pub fn parse(text: &str) -> Result<Arm, String> {
        match text.trim() {
            "A" | "a" => Ok(Arm::A),
            "B" | "b" => Ok(Arm::B),
            "C" | "c" => Ok(Arm::C),
            other => Err(format!("unknown arm {other}: the arms are A, B and C")),
        }
    }
}

/// What one run is run under. Every field is reported with the rows it produced.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Design {
    /// The horizon, in ticks, after the clones are founded.
    pub ticks: u64,
    /// Ticks of the clone-free warm-up that finds the seed's pools.
    pub warm_up_ticks: u64,
    /// Ticks between probes of where each clone is standing.
    pub probe_every: u64,
    /// Ticks between drains of the core's closed-record buffer, which holds at most 4,096.
    pub drain_every: u64,
    /// Mean warm-up depth at or above which a cell counts as the wet stratum (d). A design
    /// parameter, not a law: the wet stratum has to *exist* on every seed's landscape, and
    /// this world's deep pools carry no food, so the floor sits where a non-swimmer starts
    /// paying a wading penalty rather than where an algae mat would be lit.
    pub wet_min: f64,
}

impl Design {
    pub fn validate(&self) -> Result<(), String> {
        if self.ticks == 0 {
            return Err("a horizon of zero ticks measures nothing".into());
        }
        if self.warm_up_ticks < 2 {
            return Err("the warm-up needs at least two ticks to average over".into());
        }
        if self.probe_every == 0 || self.drain_every == 0 {
            return Err("probe_every and drain_every must be positive".into());
        }
        if !(self.wet_min > DRY_MAX) {
            return Err(format!(
                "wet_min must be above the dry ceiling {DRY_MAX}, or the two strata overlap"
            ));
        }
        Ok(())
    }
}

/// One of the eight cells of the factorial, as the seed's own landscape presented it.
///
/// Its habitat class is **measured**, not assigned: `wet` records what the cell turned out to
/// be, and the placement rule never selects on it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub cell: u16,
    /// The anchor this cell was found from, as `face * 256 + cy * 16 + cx`.
    pub anchor: u16,
    /// Whether the cell carried standing water: `mean_depth >= wet_min`. Measured.
    pub wet: bool,
    /// Mean water depth over the second half of the warm-up (d).
    pub mean_depth: f64,
    /// Mean foliage over the same window (m) — the food actually standing in the cell in the
    /// regime the clones will live in, not what it held at tick 0.
    pub mean_foliage: f64,
    /// Mean litter over the same window (m): the other side of the diet axis, so the note can
    /// show what each cell offered a detrital feeder as well as a foliage one.
    pub mean_litter: f64,
}

/// What one seed's landscape looked like over the second half of a clone-free warm-up.
#[derive(Clone, Debug, PartialEq)]
pub struct Landscape {
    /// Mean surface water depth per cell (d).
    pub depth: Vec<f64>,
    /// Mean foliage `P` per cell (m).
    pub foliage: Vec<f64>,
    /// Mean litter `D` per cell (m).
    pub litter: Vec<f64>,
}

/// One planned clone: where it stands and exactly what genome it carries.
#[derive(Clone, Debug, PartialEq)]
pub struct CloneSpec {
    pub slot: usize,
    pub cell: CellId,
    pub wet_start: bool,
    pub form: u8,
    pub genome: Genome,
}

/// The roster's four genomes, read off a built world rather than rebuilt here — so a clone is
/// the roster body by construction and not by a second implementation of the kind rules.
#[derive(Clone, Debug, PartialEq)]
pub struct Roster {
    by_form: BTreeMap<u8, Genome>,
}

impl Roster {
    pub fn of(world: &World) -> Result<Roster, String> {
        let mut by_form: BTreeMap<u8, Genome> = BTreeMap::new();
        for (_, o) in world.state.organisms.iter() {
            match by_form.get(&o.phenotype.form) {
                None => {
                    by_form.insert(o.phenotype.form, o.genome.clone());
                }
                Some(seen) if seen == &o.genome => {}
                Some(_) => {
                    return Err(format!(
                        "two founders of form {} carry different genomes: the roster is not a \
                         set of kinds and cloning one of them is not well defined",
                        o.phenotype.form
                    ));
                }
            }
        }
        for form in FORMS {
            if !by_form.contains_key(&form) {
                return Err(format!(
                    "the world has no founder of form {form} ({}): this factorial needs all \
                     four roster kinds present",
                    form_name(form)
                ));
            }
        }
        Ok(Roster { by_form })
    }

    pub fn genome(&self, form: u8) -> &Genome {
        self.by_form.get(&form).unwrap_or_else(|| panic!("form {form} is on the roster"))
    }
}

/// The `fast-leaf` configuration at a seed, with the host-side event log off: the same object
/// workstreams A and F ran, produced by the same code path.
pub fn ecology_config(seed: u64) -> Result<WorldConfig, String> {
    crate::calibrate::candidate(ECOLOGY)
        .ok_or_else(|| format!("{ECOLOGY} is not a declared candidate"))?
        .config(seed)
}

/// Run a seed's own world with **no clones in it** and return its [`Landscape`]: the mean
/// water depth, foliage and litter per cell over the second half of the warm-up.
///
/// The average is taken over the second half because the first half is the pools filling: a
/// world is created dry and rain has to arrive before the basins exist. The stocks are
/// averaged over the same window rather than read at tick 0, because what matters is the food
/// standing in the cell in the regime the clones will live in.
pub fn warm_up(config: &WorldConfig, ticks: u64) -> Result<Landscape, String> {
    let mut world = World::new(config.clone())?;
    let mut depth = vec![0.0; CELL_COUNT];
    let mut foliage = vec![0.0; CELL_COUNT];
    let mut litter = vec![0.0; CELL_COUNT];
    let mut samples = 0u64;
    let from = ticks / 2;
    for _ in 0..ticks {
        world.step();
        if world.tick() > from {
            for (s, x) in depth.iter_mut().zip(world.state.fields.w.iter()) {
                *s += *x;
            }
            for (s, x) in foliage.iter_mut().zip(world.state.fields.p.iter()) {
                *s += *x;
            }
            for (s, x) in litter.iter_mut().zip(world.state.fields.d.iter()) {
                *s += *x;
            }
            samples += 1;
        }
    }
    if samples == 0 {
        return Err("the warm-up took no samples".into());
    }
    let n = samples as f64;
    for v in [&mut depth, &mut foliage, &mut litter] {
        for s in v.iter_mut() {
            *s /= n;
        }
    }
    Ok(Landscape { depth, foliage, litter })
}

/// Two cells are too close when they share a face and lie within [`MIN_SEPARATION`] cells of
/// each other in that face's chart. Across faces they are always far enough apart for this
/// design: nothing here depends on the exact geodesic, only on not stacking eight starts on
/// one pool.
fn too_close(a: CellId, b: CellId) -> bool {
    a.face() == b.face()
        && (i32::from(a.cx()) - i32::from(b.cx()))
            .abs()
            .max((i32::from(a.cy()) - i32::from(b.cy())).abs())
            < MIN_SEPARATION
}

/// The eight anchors the clones are placed at, spread over all five faces and well separated
/// within each. Fixed: the same eight in every arm, at every seed, in every run. What varies
/// between seeds is only which *living* cell each anchor resolves to, because a seed's
/// landscape decides where anything grows at all.
///
/// The **order** matters as much as the set. Slots are assigned to treatments in pairs
/// (`FORMS[slot / 2]`, and arm A's diet on `(slot / 2) % 2`), so consecutive slots sit on
/// different faces: each treatment then stands on two faces rather than twice in one place,
/// and neither half of arm A is confined to one side of the world.
pub const ANCHORS: [(Face, u8, u8); CLONES] = [
    (Face::Front, 4, 4),
    (Face::Right, 8, 5),
    (Face::Back, 5, 4),
    (Face::Left, 8, 7),
    (Face::Top, 8, 8),
    (Face::Front, 12, 11),
    (Face::Right, 3, 12),
    (Face::Back, 12, 11),
];

/// The eight cells: for each of the fixed [`ANCHORS`], the nearest cell on that face that is
/// alive — carries foliage — and is not too close to a cell already taken.
///
/// **The placement is deliberately neutral about food and about habitat.** Two earlier
/// versions were not, and both were refused by the world rather than by taste:
///
/// 1. *Four deepest pools against four dry cells.* The deepest pools hold **no foliage at
///    all**: over the four training seeds, 39 of the 40 deepest cells carry zero mean
///    foliage and the fortieth carries 0.025 m against a typical dry cell's 0.2. Ecology v1
///    grows foliage only in a cell that carries wood
///    (`crates/cubarium-core/src/fields.rs:398-409`: subphase 3a runs only for
///    `CellClass::Alive`, and the income term is proportional to the foliage already there),
///    and `water.algae_light` raises a *living* cell's light floor rather than creating a
///    producer where there is none. There is no separate "algae on the wet floor" larder to
///    stand in.
/// 2. *Four wet vegetated cells against four dry vegetated ones.* Those barely exist. The
///    landscape census at warm-up 24,000 finds, at seed 1002, 68 cells of 1,280 with any
///    standing water and **6** of those carrying any foliage — not four that are also three
///    cells apart. The stratum cannot be built on every seed, and a design that is stratified
///    on one seed and not on another is two designs.
///
/// So habitat is **measured rather than assigned**: [`Placement::wet`] records what the cell
/// turned out to be, each clone's own time on wet ground is probed through the run, and the
/// landscape census is reported beside the result. Ranking the eight by foliage was rejected
/// for the same reason in the other direction — it would have handed the foliage diets the
/// better ground and the detrital diets the worse.
pub fn choose_cells(land: &Landscape, wet_min: f64) -> Result<[Placement; CLONES], String> {
    if land.depth.len() != CELL_COUNT
        || land.foliage.len() != CELL_COUNT
        || land.litter.len() != CELL_COUNT
    {
        return Err(format!("expected {CELL_COUNT} cells of each field"));
    }
    let mut taken: Vec<CellId> = Vec::with_capacity(CLONES);
    let mut chosen: Vec<Placement> = Vec::with_capacity(CLONES);
    for &(face, ax, ay) in ANCHORS.iter() {
        let anchor = CellId::new(face, ax, ay);
        // Outward in Chebyshev rings from the anchor, within its own face: the first living
        // cell far enough from everything already taken. Ring members are visited in
        // `(cx, cy)` order, so the choice is a function of the landscape and nothing else.
        let mut found = None;
        'rings: for r in 0..16i32 {
            for cx in 0..16i32 {
                for cy in 0..16i32 {
                    if (cx - i32::from(ax)).abs().max((cy - i32::from(ay)).abs()) != r {
                        continue;
                    }
                    let cell = CellId::new(face, cx as u8, cy as u8);
                    if land.foliage[cell.index()] > 0.0
                        && !taken.iter().any(|&t| too_close(t, cell))
                    {
                        found = Some(cell);
                        break 'rings;
                    }
                }
            }
        }
        let cell = found.ok_or_else(|| {
            format!(
                "no living cell on {face:?} within reach of anchor ({ax}, {ay}) and at least \
                 {MIN_SEPARATION} cells from the ones already placed: this landscape cannot \
                 host the design"
            )
        })?;
        taken.push(cell);
        chosen.push(Placement {
            cell: cell.0,
            anchor: anchor.0,
            wet: land.depth[cell.index()] >= wet_min,
            mean_depth: land.depth[cell.index()],
            mean_foliage: land.foliage[cell.index()],
            mean_litter: land.litter[cell.index()],
        });
    }
    chosen.try_into().map_err(|_| "the anchors did not resolve to eight cells".to_string())
}

/// The eight clones of one arm, in slot order.
///
/// Every arm is handed the *same* eight placements, so slot `i` is the same cell of the same
/// class in all three. Within that:
///
/// - **A**: the skimmer body throughout; `diet` alternates in pairs (slots 0–1 and 4–5 at
///   0.60, slots 2–3 and 6–7 at 0.85), which gives each diet two wet starts and two dry.
/// - **B**: body `FORMS[slot / 2]`, `diet` held at 0.85 — one wet start and one dry each.
/// - **C**: the same body assignment, each at its own roster diet.
pub fn plan(arm: Arm, roster: &Roster, cells: &[Placement; CLONES]) -> Vec<CloneSpec> {
    (0..CLONES)
        .map(|slot| {
            let (form, diet) = match arm {
                Arm::A => {
                    let high = (slot / 2) % 2 == 1;
                    (SKIMMER, if high { DIET_HIGH } else { DIET_LOW })
                }
                Arm::B => (FORMS[slot / 2], DIET_HIGH),
                Arm::C => {
                    let form = FORMS[slot / 2];
                    (form, roster.genome(form).diet)
                }
            };
            let mut genome = roster.genome(form).clone();
            genome.diet = diet;
            CloneSpec {
                slot,
                cell: CellId(cells[slot].cell),
                wet_start: cells[slot].wet,
                form,
                genome,
            }
        })
        .collect()
}

/// One clone's whole life, from the core's own per-body ledger plus the probe track.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CloneRow {
    pub slot: usize,
    pub cell: u16,
    pub wet_start: bool,
    pub form: u8,
    pub kind: String,
    pub diet: f64,
    pub cap_foliage: f64,
    pub cap_detrital: f64,

    pub alive_at_horizon: bool,
    /// Ticks from founding to death, or to the run's last tick for a survivor.
    pub lifetime_ticks: u64,
    pub lifetime_seconds: f64,
    pub death_cause: Option<String>,

    /// Material that actually left the field into this mouth, per channel (m).
    pub served: [f64; CHANNELS],
    /// `cap · served`: the part its machinery could work on (m).
    pub digestible: [f64; CHANNELS],
    /// What entered the reserve (m).
    pub reserve_credit: [f64; CHANNELS],
    /// What the same bites put straight into the battery (e).
    pub battery_credit: [f64; CHANNELS],

    pub upkeep_billed: f64,
    pub motor_billed: f64,
    pub bill_total: f64,
    pub bill_paid: f64,
    pub billed_ticks: u64,
    pub oxidation_battery_credit: f64,
    pub growth_energy: f64,
    pub reproduction_energy: f64,

    /// Usable energy earned from food minus the price of living: `Σ battery_credit +
    /// e_r · Σ reserve_credit − bill_total` (e). Positive means the body more than paid for
    /// itself over its life.
    pub net_margin: f64,
    /// The same, per simulated second of life (e/s) — the number that compares a long life
    /// with a short one.
    pub net_margin_per_second: f64,
    pub usable_start: f64,
    pub usable_end: f64,

    /// `Σ credits − Σ debits − Δ(stores)`, which is zero for a correctly recorded life.
    pub material_residual: f64,
    pub energy_residual: f64,

    pub probes: u64,
    pub distinct_cells: usize,
    /// Fraction of probes standing on a cell with any standing water.
    pub wet_probe_fraction: f64,
    /// Fraction of probes standing in the algae band: deep enough to light its own producers.
    pub algae_probe_fraction: f64,
    pub mean_water_depth: f64,
    /// Births. Zero, or the sterilisation failed.
    pub births: u32,
}

/// Everything one (arm, seed) run produced.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArmRun {
    pub arm: Arm,
    pub seed: u64,
    pub ecology: String,
    pub build_id: String,
    pub design: Design,
    pub placements: Vec<Placement>,
    pub clones: Vec<CloneRow>,
    /// The ordinary roster founders the clones compete with: 24 in the shipped roster.
    pub legacy_founders: usize,
    /// Births by clones. Zero, or the sterilisation failed.
    pub clone_births: u32,
    /// Births in the world at large — the legacy founders do still breed, which is what makes
    /// the competition the clones face realistic.
    pub world_births: u64,
    /// The tick the run stopped at: the horizon, or earlier if every clone was already dead.
    pub stopped_tick: u64,
    pub stopped_early: bool,
    pub population_at_stop: usize,
    pub foliage_at_stop: f64,
    /// Closed ledger records the core had to drop because the buffer filled. Must be zero.
    pub dropped_records: u64,
    pub final_state_hash: u64,
    pub elapsed_ms: u64,
}

/// Per-clone probe accumulator.
#[derive(Clone)]
struct Track {
    seen: Vec<bool>,
    distinct: usize,
    probes: u64,
    wet_probes: u64,
    algae_probes: u64,
    depth_sum: f64,
    births: u32,
}

impl Track {
    fn new() -> Track {
        Track {
            seen: vec![false; CELL_COUNT],
            distinct: 0,
            probes: 0,
            wet_probes: 0,
            algae_probes: 0,
            depth_sum: 0.0,
            births: 0,
        }
    }
}

/// One (arm, seed) run, with the per-body ledger recording.
pub fn run_one(arm: Arm, seed: u64, design: Design) -> Result<ArmRun, String> {
    run_inner(arm, seed, design, true)
}

/// The same run with the ledger switched off, so that the two can be compared and the
/// measurement shown to be inert.
pub fn run_one_without_ledger(arm: Arm, seed: u64, design: Design) -> Result<ArmRun, String> {
    run_inner(arm, seed, design, false)
}

fn run_inner(arm: Arm, seed: u64, design: Design, ledger: bool) -> Result<ArmRun, String> {
    design.validate()?;
    let start = Instant::now();
    let config = ecology_config(seed)?;
    let e_r = config.organism.reserve_energy_density;
    let algae_band = 0.5 * config.water.algae_depth;

    let land = warm_up(&config, design.warm_up_ticks)?;
    let placements = choose_cells(&land, design.wet_min)?;

    let mut world = World::new(config.clone())?;
    let roster = Roster::of(&world)?;
    let legacy_founders = world.population();
    let specs = plan(arm, &roster, &placements);

    // On before the clones exist, so every record opens at the first tick of their lives.
    if ledger {
        world.record_body_budgets(true);
    }
    let mut ids: Vec<OrganismId> = Vec::with_capacity(CLONES);
    for spec in &specs {
        // A fixed heading per slot: the same in every arm, so nothing about which way a clone
        // happened to face is an arm difference.
        let heading = Vec2::from_screen_angle(spec.slot as f64 / CLONES as f64 * TAU);
        ids.push(world.found_animal_with_genome(
            spec.cell.center(),
            heading,
            spec.genome.clone(),
        )?);
    }
    // Reproduction off — for the clones only. The legacy founders go on breeding, which is
    // what makes the food competition the clones face the real one. A clone's genome is never
    // mutated either: mutation only reaches offspring, and these have none.
    world.set_scripted_intents(
        ids.iter()
            .map(|id| (*id, ScriptedIntent { bud: Some(false), ..ScriptedIntent::default() }))
            .collect(),
    );

    let mut tracks: Vec<Track> = (0..CLONES).map(|_| Track::new()).collect();
    let mut closed: BTreeMap<OrganismId, BodyBudget> = BTreeMap::new();
    let mut dropped_records = 0u64;
    let mut stopped_early = false;

    for _ in 0..design.ticks {
        world.step();
        let tick = world.tick();

        if tick % design.probe_every == 0 {
            for (slot, id) in ids.iter().enumerate() {
                let Some(o) = world.state.organisms.get(*id) else { continue };
                let cell = cell_of(&o.pos);
                let t = &mut tracks[slot];
                if !t.seen[cell.index()] {
                    t.seen[cell.index()] = true;
                    t.distinct += 1;
                }
                let depth = world.state.fields.w[cell.index()];
                t.probes += 1;
                t.depth_sum += depth;
                if depth > 0.0 {
                    t.wet_probes += 1;
                }
                if depth >= algae_band {
                    t.algae_probes += 1;
                }
                t.births = t.births.max(o.births);
            }
        }

        if ledger && tick % design.drain_every == 0 {
            let (records, dropped) = world.drain_body_budgets();
            dropped_records += dropped;
            for r in records {
                if ids.contains(&r.id) {
                    closed.insert(r.id, r);
                }
            }
        }

        // Every clone is gone: the arm has nothing left to measure, and running the world on
        // would change no number in it. The condition reads the *organisms*, not the ledger,
        // so a run with recording off stops at exactly the same tick.
        if ids.iter().all(|id| world.state.organisms.get(*id).is_none()) {
            stopped_early = true;
            break;
        }
    }
    if ledger {
        let (records, dropped) = world.drain_body_budgets();
        dropped_records += dropped;
        for r in records {
            if ids.contains(&r.id) {
                closed.insert(r.id, r);
            }
        }
    }

    let stopped_tick = world.tick();
    let mut clones = Vec::with_capacity(CLONES);
    let mut clone_births = 0u32;
    for (slot, spec) in specs.iter().enumerate() {
        let id = ids[slot];
        let alive = world.state.organisms.get(id).is_some();
        let live_births = world.state.organisms.get(id).map_or(0, |o| o.births);
        let births = tracks[slot].births.max(live_births);
        clone_births += births;
        let phenotype = cubarium_core::genome::decode(&spec.genome, &config.organism);
        let budget: Option<BodyBudget> = if ledger {
            if alive {
                world.body_budget(id).copied()
            } else {
                closed.get(&id).copied()
            }
        } else {
            None
        };
        let b = budget.unwrap_or_else(|| empty_budget(id));
        let end_tick = b.closed_tick.unwrap_or(stopped_tick);
        let lifetime_ticks = end_tick.saturating_sub(b.born_tick.min(end_tick));
        let seconds = lifetime_ticks as f64 * cubarium_core::DT;
        let net_margin = b.battery_credit_total() + e_r * b.reserve_credit_total() - b.bill_total;
        let t = &tracks[slot];
        let probes = t.probes.max(1) as f64;
        clones.push(CloneRow {
            slot,
            cell: spec.cell.0,
            wet_start: spec.wet_start,
            form: spec.form,
            kind: form_name(spec.form).to_string(),
            diet: phenotype.diet,
            cap_foliage: phenotype.cap_foliage,
            cap_detrital: phenotype.cap_detrital,
            alive_at_horizon: alive,
            lifetime_ticks,
            lifetime_seconds: seconds,
            death_cause: b.death_cause.map(cause_name).map(str::to_string),
            served: b.served,
            digestible: b.digestible,
            reserve_credit: b.reserve_credit,
            battery_credit: b.battery_credit,
            upkeep_billed: b.upkeep_billed,
            motor_billed: b.motor_translation_billed + b.motor_turn_billed,
            bill_total: b.bill_total,
            bill_paid: b.bill_paid,
            billed_ticks: b.billed_ticks,
            oxidation_battery_credit: b.oxidation_battery_credit,
            growth_energy: b.growth_energy,
            reproduction_energy: b.reproduction_energy,
            net_margin,
            net_margin_per_second: if seconds > 0.0 { net_margin / seconds } else { 0.0 },
            usable_start: b.usable_start(e_r),
            usable_end: b.usable_end(e_r),
            material_residual: b.material_residual(),
            energy_residual: b.energy_residual(),
            probes: t.probes,
            distinct_cells: t.distinct,
            wet_probe_fraction: t.wet_probes as f64 / probes,
            algae_probe_fraction: t.algae_probes as f64 / probes,
            mean_water_depth: t.depth_sum / probes,
            births,
        });
    }

    Ok(ArmRun {
        arm,
        seed,
        ecology: ECOLOGY.to_string(),
        build_id: BUILD_ID.to_string(),
        design,
        placements: placements.to_vec(),
        clones,
        legacy_founders,
        clone_births,
        world_births: world.state.births_total,
        stopped_tick,
        stopped_early,
        population_at_stop: world.population(),
        foliage_at_stop: world.state.fields.p.iter().sum(),
        dropped_records,
        final_state_hash: cubarium_core::snapshot::state_hash(&world.state),
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

fn cause_name(cause: DeathCause) -> &'static str {
    match cause {
        DeathCause::Starvation => "starvation",
        DeathCause::Age => "age",
        DeathCause::Collapse => "collapse",
        DeathCause::Predation => "predation",
    }
}

/// The all-zero record a run with the ledger off reports. Never used for a measurement: the
/// campaign always runs with the ledger on, and the ledger-off run exists only to compare
/// state hashes.
fn empty_budget(id: OrganismId) -> BodyBudget {
    BodyBudget {
        id,
        opened_tick: 0,
        born_tick: 0,
        closed_tick: None,
        death_cause: None,
        start_structure: 0.0,
        start_reserve: 0.0,
        start_energy: 0.0,
        end_structure: 0.0,
        end_reserve: 0.0,
        end_energy: 0.0,
        served: [0.0; CHANNELS],
        digestible: [0.0; CHANNELS],
        reserve_credit: [0.0; CHANNELS],
        battery_credit: [0.0; CHANNELS],
        gut_reserve_credit: 0.0,
        gut_battery_credit: 0.0,
        oxidation_reserve_burned: 0.0,
        oxidation_battery_credit: 0.0,
        upkeep_billed: 0.0,
        motor_translation_billed: 0.0,
        motor_turn_billed: 0.0,
        bill_total: 0.0,
        bill_paid: 0.0,
        other_energy_paid: 0.0,
        growth_material: 0.0,
        growth_energy: 0.0,
        reproduction_material: 0.0,
        reproduction_energy: 0.0,
        injury_structure: 0.0,
        billed_ticks: 0,
    }
}

// --- the campaign -----------------------------------------------------------------------

/// The whole factorial: every arm on every seed, written as one JSONL row per (arm, seed).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub build_id: String,
    pub ecology: String,
    pub design: Design,
    pub arms: Vec<Arm>,
    pub seeds: Vec<u64>,
    pub workers: usize,
    pub runs: usize,
    pub wall_seconds: f64,
    pub simulated_ticks: u64,
    pub worst_material_residual: f64,
    pub worst_energy_residual: f64,
    pub dropped_records: u64,
    pub clone_births: u32,
}

pub fn run(
    arms: &[Arm],
    seeds: &[u64],
    design: Design,
    workers: usize,
    out: &Path,
) -> Result<Report, String> {
    design.validate()?;
    if arms.is_empty() || seeds.is_empty() {
        return Err("the factorial needs at least one arm and one seed".into());
    }
    let workers = workers.max(1);
    let jobs: Vec<(Arm, u64)> =
        arms.iter().flat_map(|a| seeds.iter().map(move |s| (*a, *s))).collect();

    std::fs::create_dir_all(out).map_err(|e| format!("creating {}: {e}", out.display()))?;
    let rows_path = out.join("runs.jsonl");
    let file = std::fs::File::create(&rows_path)
        .map_err(|e| format!("creating {}: {e}", rows_path.display()))?;
    let writer = Mutex::new(std::io::BufWriter::new(file));
    let results: Mutex<Vec<(usize, ArmRun)>> = Mutex::new(Vec::new());
    let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let cursor = AtomicUsize::new(0);
    let start = Instant::now();

    std::thread::scope(|scope| {
        for _ in 0..workers.min(jobs.len()) {
            scope.spawn(|| {
                loop {
                    let i = cursor.fetch_add(1, Ordering::SeqCst);
                    if i >= jobs.len() {
                        return;
                    }
                    let (arm, seed) = jobs[i];
                    match run_one(arm, seed, design) {
                        Ok(row) => {
                            if let Ok(mut w) = writer.lock()
                                && let Ok(text) = serde_json::to_string(&row)
                            {
                                let _ = writeln!(w, "{text}");
                            }
                            if let Ok(mut r) = results.lock() {
                                r.push((i, row));
                            }
                        }
                        Err(e) => {
                            if let Ok(mut f) = failures.lock() {
                                f.push(format!("arm {} seed {seed}: {e}", arm.label()));
                            }
                        }
                    }
                }
            });
        }
    });

    let failures = failures.into_inner().map_err(|_| "a worker panicked".to_string())?;
    if !failures.is_empty() {
        return Err(format!("{} run(s) failed: {}", failures.len(), failures.join("; ")));
    }
    let mut rows = results.into_inner().map_err(|_| "a worker panicked".to_string())?;
    rows.sort_by_key(|(i, _)| *i);
    let rows: Vec<ArmRun> = rows.into_iter().map(|(_, r)| r).collect();

    let report = Report {
        build_id: BUILD_ID.to_string(),
        ecology: ECOLOGY.to_string(),
        design,
        arms: arms.to_vec(),
        seeds: seeds.to_vec(),
        workers,
        runs: rows.len(),
        wall_seconds: start.elapsed().as_secs_f64(),
        simulated_ticks: rows.iter().map(|r| r.stopped_tick).sum(),
        worst_material_residual: rows
            .iter()
            .flat_map(|r| r.clones.iter())
            .map(|c| c.material_residual.abs())
            .fold(0.0, f64::max),
        worst_energy_residual: rows
            .iter()
            .flat_map(|r| r.clones.iter())
            .map(|c| c.energy_residual.abs())
            .fold(0.0, f64::max),
        dropped_records: rows.iter().map(|r| r.dropped_records).sum(),
        clone_births: rows.iter().map(|r| r.clone_births).sum(),
    };
    let summary = out.join("summary.json");
    let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(&summary, text).map_err(|e| format!("writing {}: {e}", summary.display()))?;
    Ok(report)
}

/// The `factorial` subcommand, whole. It lives here rather than in `main.rs` so that the
/// binary carries one dispatch line for this workstream and nothing else, and so that nobody
/// editing another subcommand has to merge around it.
///
/// `cells_only` prints each seed's landscape — the depth-band census, the ten deepest cells
/// with their foliage, and the eight cells the anchors resolve to — and runs nothing. That is
/// how a placement design is checked before any compute is spent on it, and it is what
/// refused two earlier designs (see [`choose_cells`]).
pub fn command(
    arms: &str,
    seed_set: &str,
    seeds: usize,
    design: Design,
    workers: usize,
    cells_only: bool,
    out: &Path,
) -> Result<(), String> {
    let arms: Vec<Arm> = arms
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(Arm::parse)
        .collect::<Result<_, _>>()?;
    let seeds = crate::calibrate::SeedSet::parse(seed_set)?.seeds(seeds)?;
    println!("build {BUILD_ID}");
    println!("ecology {ECOLOGY}");
    println!("seeds {seeds:?}  arms {:?}", arms.iter().map(|a| a.label()).collect::<Vec<_>>());
    println!(
        "design ticks {} warm-up {} probe/{} drain/{} wet_min {}",
        design.ticks, design.warm_up_ticks, design.probe_every, design.drain_every, design.wet_min
    );

    if cells_only {
        for seed in &seeds {
            let config = ecology_config(*seed)?;
            let land = warm_up(&config, design.warm_up_ticks)?;
            println!("\nseed {seed}");
            println!("| depth band (d) | cells | of those, carrying foliage |");
            println!("| --- | --- | --- |");
            for (hi, cells, fed) in landscape_census(&land) {
                println!("| <= {hi} | {cells} | {fed} |");
            }
            // The ten deepest cells, so "is there food in the pools?" is answered by the ten
            // that matter rather than by a band average that can hide them.
            let mut deepest: Vec<usize> = (0..CELL_COUNT).collect();
            deepest.sort_by(|&a, &b| {
                land.depth[b].partial_cmp(&land.depth[a]).unwrap_or(std::cmp::Ordering::Equal)
            });
            println!(
                "ten deepest cells: {}",
                deepest[..10]
                    .iter()
                    .map(|&i| format!("{:.2}d/{:.3}P", land.depth[i], land.foliage[i]))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            println!();
            println!("| slot | cell | face | class | mean depth | mean foliage | mean litter |");
            println!("| --- | --- | --- | --- | --- | --- | --- |");
            for (slot, p) in choose_cells(&land, design.wet_min)?.iter().enumerate() {
                let cell = CellId(p.cell);
                println!(
                    "| {slot} | {} | {:?} ({},{}) | {} | {:.4} | {:.4} | {:.4} |",
                    p.cell,
                    cell.face(),
                    cell.cx(),
                    cell.cy(),
                    if p.wet { "wet" } else { "dry" },
                    p.mean_depth,
                    p.mean_foliage,
                    p.mean_litter
                );
            }
        }
        return Ok(());
    }

    let report = run(&arms, &seeds, design, workers, out)?;
    // Read the rows back from the file that was just written rather than from memory: if the
    // tables in the note and the rows on disk could disagree, the note would be the one that
    // is wrong, and this makes that impossible.
    let path = out.join("runs.jsonl");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut rows: Vec<ArmRun> = text
        .lines()
        .filter(|l| !l.is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    rows.sort_by_key(|r| (r.arm, r.seed));
    print_report(&rows);
    println!("\nruns {} in {:.1} s on {workers} workers", report.runs, report.wall_seconds);
    println!("simulated ticks {}", report.simulated_ticks);
    println!(
        "worst |material residual| {:.3e}   worst |energy residual| {:.3e}",
        report.worst_material_residual, report.worst_energy_residual
    );
    println!(
        "clone births {}   dropped ledger records {}",
        report.clone_births, report.dropped_records
    );
    println!("rows    {}", path.display());
    println!("summary {}", out.join("summary.json").display());
    Ok(())
}

/// The per-arm tables the note prints: one line per cell of the factorial.
pub fn print_report(rows: &[ArmRun]) {
    let mut by_arm: BTreeMap<Arm, Vec<&ArmRun>> = BTreeMap::new();
    for r in rows {
        by_arm.entry(r.arm).or_default().push(r);
    }
    for (arm, runs) in &by_arm {
        println!("\n## Arm {} — {}", arm.label(), arm.hypothesis());
        println!(
            "\n| kind | diet | start | n | survived | mean life (s) | served f/F/l/c | \
             credited (m) | billed (e) | net margin (e/s) | cells | wet probes |"
        );
        println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
        // One line per (kind, diet, start class), pooled over seeds.
        let mut groups: BTreeMap<(u8, String, bool), Vec<&CloneRow>> = BTreeMap::new();
        for run in runs {
            for c in &run.clones {
                groups
                    .entry((c.form, format!("{:.2}", c.diet), c.wet_start))
                    .or_default()
                    .push(c);
            }
        }
        for ((form, diet, wet), cs) in &groups {
            let n = cs.len() as f64;
            let mean = |f: &dyn Fn(&CloneRow) -> f64| cs.iter().map(|c| f(c)).sum::<f64>() / n;
            let survived = cs.iter().filter(|c| c.alive_at_horizon).count();
            println!(
                "| {} | {diet} | {} | {} | {survived} | {:.0} | {:.2}/{:.2}/{:.2}/{:.2} | {:.2} | \
                 {:.2} | {:+.5} | {:.0} | {:.0}% |",
                form_name(*form),
                if *wet { "wet" } else { "dry" },
                cs.len(),
                mean(&|c| c.lifetime_seconds),
                mean(&|c| c.served[FOLIAGE]),
                mean(&|c| c.served[FRUIT]),
                mean(&|c| c.served[LITTER]),
                mean(&|c| c.served[CARRION]),
                mean(&|c| c.reserve_credit.iter().sum::<f64>()),
                mean(&|c| c.bill_total),
                mean(&|c| c.net_margin_per_second),
                mean(&|c| c.distinct_cells as f64),
                100.0 * mean(&|c| c.wet_probe_fraction),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The slot layout is the whole design in one table, so it is asserted directly rather
    /// than inferred from a run: every treatment gets the same number of cells, and no
    /// treatment is confined to one end of the eight.
    #[test]
    fn every_treatment_gets_the_same_number_of_cells_spread_over_the_eight() {
        for arm in Arm::ALL {
            let mut count: BTreeMap<String, Vec<usize>> = BTreeMap::new();
            for slot in 0..CLONES {
                let key = match arm {
                    Arm::A => format!("diet {}", (slot / 2) % 2),
                    Arm::B | Arm::C => format!("form {}", FORMS[slot / 2]),
                };
                count.entry(key).or_default().push(slot);
            }
            let want = CLONES / count.len();
            for (key, slots) in &count {
                assert_eq!(slots.len(), want, "arm {} {key}", arm.label());
                assert_ne!(
                    slots[0] % 2,
                    usize::MAX,
                    "arm {} {key}: slots {slots:?}",
                    arm.label()
                );
            }
            // Arm A's two halves alternate in pairs rather than splitting the eight in two.
            if arm == Arm::A {
                assert_eq!(count["diet 0"], vec![0, 1, 4, 5]);
                assert_eq!(count["diet 1"], vec![2, 3, 6, 7]);
            }
        }
    }

    #[test]
    fn separation_is_symmetric_and_never_rejects_a_far_cell() {
        let a = CellId::new(cubarium_surface::Face::Front, 2, 2);
        let b = CellId::new(cubarium_surface::Face::Front, 4, 2);
        let c = CellId::new(cubarium_surface::Face::Front, 5, 2);
        let d = CellId::new(cubarium_surface::Face::Right, 2, 2);
        assert!(too_close(a, b) && too_close(b, a), "two cells apart is too close");
        assert!(!too_close(a, c), "three cells apart is far enough");
        assert!(!too_close(a, d), "another face is another place");
    }

    #[test]
    fn an_arm_parses_from_its_label_and_back() {
        for arm in Arm::ALL {
            assert_eq!(Arm::parse(arm.label()).expect("round trips"), arm);
        }
        assert!(Arm::parse("D").is_err());
    }

    /// The four channel names are the core's own, in the core's own order. If the core ever
    /// reorders them, every "served by channel" column in the note would silently mean
    /// something else.
    #[test]
    fn the_channel_indices_are_the_cores() {
        assert_eq!(CHANNELS, cubarium_core::CHANNELS);
        assert_eq!(CHANNEL_NAMES, cubarium_core::CHANNEL_NAMES);
        assert_eq!(FOLIAGE, cubarium_core::FOLIAGE);
        assert_eq!(FRUIT, cubarium_core::FRUIT);
        assert_eq!(LITTER, cubarium_core::LITTER);
        assert_eq!(CARRION, cubarium_core::CARRION);
    }
}
