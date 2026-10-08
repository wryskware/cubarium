//! **Brain curiosity** (`design/handoffs/brain-curiosity-2026-10-02.md`, Wrysk
//! 2026-10-02): a per-species exploration term in the brains' training reward.
//!
//! A body's window score becomes `delta + given + weight × novelty`. Novelty is ground new
//! to the body: the ground is cut into square cells of `cell_bl` adult body lengths of its
//! lineage; the body keeps a familiarity `f ∈ [0, 1]` per cell it has entered, held at 1
//! while it stands there and halving every `memory_min` minutes after it leaves; entering
//! a cell earns `1 − f`. A window's novelty is its earnings over `n_cruise`, the cells a
//! straight walk at the lineage's cruise would enter in the window, so about 0–1.
//!
//! **Training side only.** The memory ([`Memory`]) is keyed by body id, lives across
//! windows for the body's life in its world, and is dropped when the body dies or is
//! gone. Nothing goes into the sim, `FaunaConfig` or a snapshot; the knobs are training
//! knobs (`config/training/curiosity.toml`, `--curiosity`), not ecology-search genes.

use std::path::Path;

use cubarium_voxel_fauna::{Founder, FounderPhysiology};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use super::window::TICKS_PER_MIN;

/// One lineage's curiosity: a `[<lineage>]` table of the curiosity file.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct LineageCuriosity {
    /// What a window at novelty 1 (fresh ground at cruise all window) is worth, in body
    /// references per window: the novelty's weight in the score. ≥ 0; 0 is today's score.
    pub weight: f64,
    /// The familiarity half-life after the body leaves a cell, simulated minutes. > 0.
    pub memory_min: f64,
    /// The cell side, in adult body lengths of the lineage. > 0.
    pub cell_bl: f64,
}

impl Default for LineageCuriosity {
    /// No curiosity, on the default cells and memory (what the check measures novelty on
    /// without a file).
    fn default() -> LineageCuriosity {
        LineageCuriosity {
            weight: 0.0,
            memory_min: 30.0,
            cell_bl: 1.0,
        }
    }
}

impl LineageCuriosity {
    fn validate(&self, name: &str) -> Result<(), String> {
        if !(self.weight.is_finite() && self.weight >= 0.0) {
            return Err(format!("[{name}] weight must be >= 0, not {}", self.weight));
        }
        if !(self.memory_min.is_finite() && self.memory_min > 0.0) {
            return Err(format!(
                "[{name}] memory_min must be > 0, not {}",
                self.memory_min
            ));
        }
        if !(self.cell_bl.is_finite() && self.cell_bl > 0.0) {
            return Err(format!(
                "[{name}] cell_bl must be > 0, not {}",
                self.cell_bl
            ));
        }
        Ok(())
    }
}

/// Every lineage's curiosity, in [`Founder::ALL`] order. The default (no file) is weight 0
/// everywhere: today's score exactly.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Curiosity {
    pub lineages: [LineageCuriosity; Founder::COUNT],
}

impl Curiosity {
    /// Read a curiosity file's text: one `[<lineage>]` table per lineage by its lineage
    /// name (`frondgrazer`, `littershredder`, `lanternjaw`); a lineage left out keeps the
    /// default (weight 0).
    pub fn parse(text: &str) -> Result<Curiosity, String> {
        let table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut c = Curiosity::default();
        for (name, value) in table {
            let f = Founder::ALL
                .into_iter()
                .find(|f| f.name() == name)
                .ok_or_else(|| {
                    format!(
                        "unknown lineage [{name}]; the lineages are {}",
                        Founder::ALL.map(|f| f.name()).join(", ")
                    )
                })?;
            let l: LineageCuriosity = value
                .try_into()
                .map_err(|e: toml::de::Error| format!("[{name}]: {e}"))?;
            l.validate(&name)?;
            c.lineages[f.index()] = l;
        }
        Ok(c)
    }

    pub fn load(path: &Path) -> Result<Curiosity, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Curiosity::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// `--curiosity FILE`, or the default without one.
    pub fn from_flag(path: Option<&Path>) -> Result<Curiosity, String> {
        path.map_or_else(|| Ok(Curiosity::default()), Curiosity::load)
    }

    pub fn of(&self, f: Founder) -> &LineageCuriosity {
        &self.lineages[f.index()]
    }
}

/// One lineage's novelty geometry in one world: its cell, its cruise and its memory.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    /// The cell side, metres: `cell_bl` × the adult length (`body_at(body_max)`).
    pub cell_m: f64,
    /// The lineage's cruise, metres per second: its cruise in body lengths per second ×
    /// the adult length.
    pub cruise_m_per_s: f64,
    /// The familiarity half-life, ticks.
    pub half_life_ticks: f64,
}

impl Geometry {
    pub fn of(phys: &FounderPhysiology, c: &LineageCuriosity) -> Geometry {
        let adult = phys.body_at(phys.core.body_max).length_m;
        Geometry {
            cell_m: c.cell_bl * adult,
            cruise_m_per_s: phys.cruise_body_lengths_per_s * adult,
            half_life_ticks: c.memory_min * TICKS_PER_MIN as f64,
        }
    }

    /// Cells a straight walk at cruise enters in a window of `ticks`.
    pub fn n_cruise(&self, ticks: u64) -> f64 {
        ticks as f64 * cubarium_voxel::DT * self.cruise_m_per_s / self.cell_m
    }

    /// A window's novelty: `earned` (cells' worth) over [`Geometry::n_cruise`]; 0 for a
    /// degenerate window or lineage.
    pub fn novelty(&self, earned: f64, ticks: u64) -> f64 {
        let n = self.n_cruise(ticks);
        if n.is_finite() && n > 0.0 {
            earned / n
        } else {
            0.0
        }
    }

    fn cell(&self, x_m: f64, z_m: f64) -> (i64, i64) {
        if !(self.cell_m.is_finite() && self.cell_m > 0.0) {
            return (0, 0);
        }
        (
            (x_m / self.cell_m).floor() as i64,
            (z_m / self.cell_m).floor() as i64,
        )
    }
}

/// One body's familiarity: the cell it stands in, and when it last left each other cell.
#[derive(Clone, Debug, Default)]
struct BodyMemory {
    cell: Option<(i64, i64)>,
    left: FxHashMap<(i64, i64), u64>,
}

/// **The bodies' familiarity**, keyed by body id, on its own tick clock. Carried from one
/// window to the next of a world; a body's entry is dropped when it dies or is gone.
#[derive(Clone, Debug, Default)]
pub struct Memory {
    now: u64,
    bodies: FxHashMap<u64, BodyMemory>,
}

/// JSON-friendly familiarity state; spatial keys remain coordinate pairs.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MemoryState {
    now: u64,
    bodies: Vec<(u64, Option<(i64, i64)>, Vec<((i64, i64), u64)>)>,
}

impl Memory {
    pub fn state(&self) -> MemoryState {
        MemoryState { now: self.now, bodies: self.bodies.iter().map(|(id, b)|
            (*id, b.cell, b.left.iter().map(|(c, t)| (*c, *t)).collect())).collect() }
    }
    pub fn from_state(state: MemoryState) -> Self {
        Self { now: state.now, bodies: state.bodies.into_iter().map(|(id, cell, left)|
            (id, BodyMemory { cell, left: left.into_iter().collect() })).collect() }
    }

    /// One tick passes.
    pub fn advance(&mut self) {
        self.now += 1;
    }

    /// The memory's clock, ticks.
    pub fn now(&self) -> u64 {
        self.now
    }

    /// Body `id` stands at `(x_m, z_m)` now (x already wrapped onto the ring): what it
    /// earns, `1 − f` if its cell changed, else 0. A body's first observation is its spawn,
    /// not an entry: it earns nothing and its cell is familiar.
    pub fn observe(&mut self, id: u64, g: &Geometry, x_m: f64, z_m: f64) -> f64 {
        let now = self.now;
        let cell = g.cell(x_m, z_m);
        let b = self.bodies.entry(id).or_default();
        match b.cell {
            None => {
                b.cell = Some(cell);
                0.0
            }
            Some(c) if c == cell => 0.0,
            Some(c) => {
                b.left.insert(c, now);
                b.cell = Some(cell);
                let f = b.left.get(&cell).map_or(0.0, |&t| {
                    0.5f64.powf(now.saturating_sub(t) as f64 / g.half_life_ticks)
                });
                1.0 - f
            }
        }
    }

    /// Whether `id` has any memory.
    pub fn knows(&self, id: u64) -> bool {
        self.bodies.contains_key(&id)
    }

    /// Drop `id`'s memory (it died or is gone).
    pub fn forget(&mut self, id: u64) {
        self.bodies.remove(&id);
    }

    /// Keep only the ids `keep` accepts.
    pub fn retain(&mut self, mut keep: impl FnMut(u64) -> bool) {
        self.bodies.retain(|id, _| keep(*id));
    }

    /// Bodies remembered.
    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }
}
