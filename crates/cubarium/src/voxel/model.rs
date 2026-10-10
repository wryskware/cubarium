//! **Voxel organism models** baked by Blender from the simulation's own numbers, and the
//! stamping that puts them in the presenter's occupancy grids
//! (`design/handoffs/voxel-organism-models-2026-09-23.md`, package V).
//!
//! # What a model is
//!
//! A **plant model** is one species at every crown-height step of one voxel, from the
//! seedling to the adult maximum, each step in 2–3 variants (a different random phase of
//! vanes, fronds and lobes) so a meadow is not copies. An **animal model** is one founder
//! in five size bins, newborn to adult, facing +x. Every cell is an offset from the
//! **anchor** — the cell just above the support face in the organism's own column — with a
//! palette material and a [`Tag`] saying which part of the organism it is.
//!
//! The tag is what lets the picture show the simulation's state on a fixed model:
//!
//! - **Foliage thins by layer** on a species without foliage states. A foliage or drape
//!   cell of layer `i` is drawn iff [`keeps`] says so at that layer's `stock / capacity`:
//!   a pure hash of stand, cell and layer against the fraction, so the same half shows
//!   every frame and regrowth only adds cells.
//! - **Foliage states** (`design/handoffs/foliage-states-implementation-2026-10-08.md`): a
//!   species whose file carries more than one state (bloomcrown, umbrellafrond) is drawn
//!   whole in the state its **total** foliage fraction picks ([`FoliageTrack`]), with no
//!   per-layer thinning, and moves between states by a voxel cross-fade ([`fade_shows`]).
//! - **Wilt** tints foliage cells, quantised to [`WILT_LEVELS`] steps.
//! - **Accents** follow their state: a warm bloom only when the parcel is ripe (half a
//!   package), and lanternberry fruit by parcel fullness.
//! - **Starving** tints an animal.
//!
//! The model is placement, not look: every colour is one of the art direction's palette
//! entries ([`PALETTE`]), lit by the existing voxel lighting through the existing plant and
//! animal parts. The old glyphs (`stand::parts_of`, `animal::cells_of`) stay as the dev
//! path and as the fallback for a species the library has no model for.
//!
//! # One source of numbers
//!
//! [`SourceDump`] is the model's own crown ranges, stage profiles and founder bodies as
//! JSON (`examples/voxel_model_source.rs` writes `assets/voxel-models/source.json`). The
//! Blender bake (`scripts/blender/bake_voxel_models.py`) reads that file and nothing else
//! for numbers, so re-running the dump and the bake reproduces the models.
//!
//! # The file format
//!
//! `assets/voxel-models/<voxel_mm>/<name>.cvm`, one per species or founder, plus a
//! `manifest.json` naming them. Compact little-endian binary (5 bytes a cell), because
//! the library is regenerated rather than edited, is read once at start-up, and is a
//! tenth of the size of the same cells in JSON. Version 2 (2026-10-08) adds the **state
//! axis**: a header table of foliage states, and per step and variant one cell list per
//! state. A version 1 file is refused: the library is rebaked, never migrated.
//!
//! ```text
//! b"CVM2"  kind u8 (0 plant, 1 animal)
//! n_materials u8, then per material: len u8, name bytes (a PALETTE name)
//! n_states u8 (>= 1), then per state: len u8, name bytes, start f64
//!     state 0 is the full model, start 1; state k is meant for a total foliage fraction
//!     below its start. The starts are the bake's record of the config it was baked
//!     against (`SpeciesSource::foliage_state_below`); the presenter reads the live
//!     config's thresholds (`SpeciesConfig::foliage_state_below`).
//! n_steps u16, then per step:
//!     height_m f64 (a bin's length for an animal), radius_m f64, n_variants u8,
//!     per variant, per state: n_cells u32, then per cell: dx i8, dy i8, dz i8, material u8, tag u8
//! tag: 0x00 trunk, 0x10|i foliage i, 0x20|i drape i, 0x30 accent, 0x40|i fallen foliage i
//! ```
//!
//! An animal file has one state. A founder with a rig (FEED-RIG: its bones and sampled
//! feeding clip) has a `<name>.rig` beside it, named under the manifest's `rigs`
//! ([`super::rig`] has its layout).

use std::path::Path;

use anyhow::{Context, Result, bail};
use cubarium_voxel::VoxelView;
use cubarium_voxel_fauna::{FaunaConfig, Founder, MobilityMode};
use cubarium_voxel_flora::{FloraConfig, Layer, Species};
use serde::{Deserialize, Serialize};

use super::stand::Cell;

// --- cells ---------------------------------------------------------------------------

/// What a baked cell is. The `u8` is the **foliage index**: the stand's `layer_stock`
/// index, bottom-up among its foliage-bearing layers (`StandLayer::foliage_index`). A drape
/// bears foliage in the model and has its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tag {
    Trunk,
    Foliage(u8),
    Drape(u8),
    Accent,
    /// Foliage of layer `i` **fallen to the ground** beside the plant (a stripped
    /// bloomcrown's crown voxels): painted as that layer's foliage at the top of its ramp,
    /// with no height shading, so it reads in the crown's own colour and not in the dark
    /// base a ground-level cell would take.
    Fallen(u8),
}

impl Tag {
    /// The file's one-byte encoding.
    pub fn from_byte(b: u8) -> Option<Tag> {
        let i = b & 0x0f;
        match b >> 4 {
            0 if i == 0 => Some(Tag::Trunk),
            1 => Some(Tag::Foliage(i)),
            2 => Some(Tag::Drape(i)),
            3 if i == 0 => Some(Tag::Accent),
            4 => Some(Tag::Fallen(i)),
            _ => None,
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            Tag::Trunk => 0x00,
            Tag::Foliage(i) => 0x10 | (i & 0x0f),
            Tag::Drape(i) => 0x20 | (i & 0x0f),
            Tag::Accent => 0x30,
            Tag::Fallen(i) => 0x40 | (i & 0x0f),
        }
    }
}

/// One baked cell. `offset` is `[dx, dy, dz]` in voxels from the anchor; `material`
/// indexes [`PALETTE`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelCell {
    pub offset: [i32; 3],
    pub material: u8,
    pub tag: Tag,
}

/// One crown-height step of a species, and its 2–3 variants.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlantStep {
    pub height_m: f64,
    pub radius_m: f64,
    /// The full model (state 0), per variant.
    pub variants: Vec<Vec<ModelCell>>,
    /// The reduced foliage states `1..`, each per variant: `states[k - 1][v]` is state
    /// `k` of variant `v`. Empty for a species without states.
    pub states: Vec<Vec<Vec<ModelCell>>>,
}

impl PlantStep {
    /// Variant `v` in state `state` (0 is the full model); `None` past either end.
    pub fn cells(&self, v: usize, state: usize) -> Option<&[ModelCell]> {
        match state {
            0 => self.variants.get(v),
            k => self.states.get(k - 1).and_then(|s| s.get(v)),
        }
        .map(Vec::as_slice)
    }
}

/// One foliage state in a model file's header: its name, and the total foliage fraction
/// below which the bake meant it to be drawn (the full model, state 0, starts at 1).
#[derive(Clone, Debug, PartialEq)]
pub struct StateInfo {
    pub name: String,
    pub start: f64,
}

/// A species at every step, ascending by height.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlantModel {
    pub steps: Vec<PlantStep>,
    /// The file's state table, state 0 first; empty for a model built by hand without
    /// states.
    pub states: Vec<StateInfo>,
}

impl PlantModel {
    /// The step whose `height_m` is nearest `height_m` (a tie goes to the **taller** step,
    /// as `stand::crown_height_voxels` rounds 2.5 up to 3), and in it the variant
    /// `stand_id` picks. The variant depends on the id alone, so a growing stand keeps its
    /// phase. `None` with no steps, or a step with no variants.
    pub fn select(&self, height_m: f64, stand_id: u64) -> Option<&[ModelCell]> {
        let step = &self.steps[nearest(self.steps.iter().map(|s| s.height_m), height_m)?];
        variant(step, stand_id)
    }

    /// [`PlantModel::select`], and among the steps at the chosen height the one whose
    /// `radius_m` is nearest `radius_m` (a tie to the wider). The bake makes a step
    /// wherever the crown's height crosses a voxel **or** its radius a half voxel, so a
    /// species that grows sideways (velvetpad) has several steps at one height; this is
    /// what the presenter calls. With one step per height it is exactly `select`.
    pub fn select_sized(
        &self,
        height_m: f64,
        radius_m: f64,
        stand_id: u64,
    ) -> Option<&[ModelCell]> {
        let (step, v) = self.pick_sized(height_m, radius_m, stand_id)?;
        self.steps[step].cells(v, 0)
    }

    /// What [`PlantModel::select_sized`] picks, as `(step index, variant index)`, so a
    /// caller can take any of its states ([`PlantStep::cells`]).
    pub fn pick_sized(&self, height_m: f64, radius_m: f64, stand_id: u64) -> Option<(usize, usize)> {
        let h = self.steps[nearest(self.steps.iter().map(|s| s.height_m), height_m)?].height_m;
        // Steps ascend by height, so the ones at `h` are one run.
        let from = self.steps.iter().position(|s| s.height_m == h)?;
        let run = self.steps[from..].iter().take_while(|s| s.height_m == h);
        let i = nearest(run.map(|s| s.radius_m), radius_m)?;
        let v = variant_index(&self.steps[from + i], stand_id)?;
        Some((from + i, v))
    }

    /// How many foliage states every step of the model carries: 1 without states.
    pub fn state_count(&self) -> usize {
        self.steps.iter().map(|s| 1 + s.states.len()).min().unwrap_or(1)
    }
}

/// The variant of `step` that `stand_id` picks, by the id alone.
fn variant(step: &PlantStep, stand_id: u64) -> Option<&[ModelCell]> {
    Some(&step.variants[variant_index(step, stand_id)?])
}

fn variant_index(step: &PlantStep, stand_id: u64) -> Option<usize> {
    let n = step.variants.len() as u64;
    if n == 0 {
        return None;
    }
    Some((mix64(stand_id ^ VARIANT_SALT) % n) as usize)
}

/// One size bin of a founder, facing +x.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimalBin {
    pub length_m: f64,
    pub cells: Vec<ModelCell>,
}

/// A founder in its size bins, ascending by length.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimalModel {
    pub bins: Vec<AnimalBin>,
}

impl AnimalModel {
    /// The bin whose `length_m` is nearest the body's length (a tie goes to the longer
    /// bin). The presenter passes `founder(f).body_at(animal.body).length_m`.
    pub fn select(&self, length_m: f64) -> Option<&[ModelCell]> {
        let i = nearest(self.bins.iter().map(|b| b.length_m), length_m)?;
        Some(&self.bins[i].cells)
    }
}

/// The index of the value nearest `want` among ascending `values`, a tie to the later.
/// A non-finite `want` takes the last.
pub(crate) fn nearest(values: impl Iterator<Item = f64>, want: f64) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, v) in values.enumerate() {
        let d = if want.is_finite() {
            (v - want).abs()
        } else {
            0.0
        };
        match best {
            Some((_, bd)) if d > bd => {}
            _ => best = Some((i, d)),
        }
    }
    best.map(|(i, _)| i)
}

// --- models by mode -------------------------------------------------------------------

/// **Which baked model a lineage wears in a mode** (F3; the seedporter's poses,
/// `design/animal-body-reimagining-2026-09-24.md` §Seedporter "Poses"): a specimen name, or
/// `None` for the lineage's one model. The seedporter climbs in its `seedporter` model,
/// perches in `seedporter-stand` (its z = 0 is the perch under its feet), walks in
/// `seedporter-ground` and glides in `seedporter-glide`. Every other lineage has one pose
/// (a bellwing's perched pose is owed, F1).
pub fn mode_model(founder: Founder, mode: MobilityMode) -> Option<&'static str> {
    match (founder, mode) {
        (Founder::Seedporter, MobilityMode::Climb) => Some("seedporter"),
        (Founder::Seedporter, MobilityMode::Perched) => Some("seedporter-stand"),
        (Founder::Seedporter, MobilityMode::Ground) => Some("seedporter-ground"),
        (Founder::Seedporter, MobilityMode::Glide) => Some("seedporter-glide"),
        _ => None,
    }
}

/// **The baked model a lineage's hunker is drawn toward** (HUNKER, [`super::hunker`]): a
/// specimen baked in the founder's own size bins, or `None` for a lineage with no hunker
/// pose (its hunker, if any, is drawn as the sleep's settle).
pub fn hunker_model(founder: Founder) -> Option<&'static str> {
    match founder {
        Founder::Browser => Some("frondgrazer-hunker"),
        _ => None,
    }
}

// --- the library ---------------------------------------------------------------------

/// Every model the presenter can stamp: at most one per species and one per founder.
#[derive(Clone, Debug, Default)]
pub struct ModelLibrary {
    plants: [Option<PlantModel>; Species::COUNT],
    animals: [Option<AnimalModel>; Founder::COUNT],
    /// The unsimulated animals (model pass MP-A: chorister, ripple-snail, ...), by their
    /// manifest name: one adult bin each, drawn only where a caller places them
    /// (`examples/voxel_specimens.rs`). Not founders, and not in the simulation.
    specimens: std::collections::BTreeMap<String, AnimalModel>,
    /// The founders' rigs (FEED-RIG, [`super::rig`]), by founder name: bones and a sampled
    /// clip baked beside the founder's model, its bins the model's.
    rigs: std::collections::BTreeMap<String, std::sync::Arc<super::rig::AnimalRig>>,
    /// The voxel size the models were baked at, when loaded from disk. A world of another
    /// size draws the glyphs instead: a model is so many cells, not so many metres.
    voxel_m: Option<f64>,
}

impl ModelLibrary {
    pub fn insert_plant(&mut self, species: Species, model: PlantModel) {
        self.plants[species.index()] = Some(model);
    }

    pub fn insert_animal(&mut self, founder: Founder, model: AnimalModel) {
        self.animals[founder.index()] = Some(model);
    }

    pub fn plant(&self, species: Species) -> Option<&PlantModel> {
        self.plants[species.index()].as_ref()
    }

    /// A live founder's model: its own baked founder model, else the **specimen** of its
    /// lineage name (`"ripple_snail"` finds `"ripple-snail"`), so a new founder draws with
    /// its baked specimen and needs no rebake.
    pub fn animal(&self, founder: Founder) -> Option<&AnimalModel> {
        self.animals[founder.index()]
            .as_ref()
            .or_else(|| self.specimen(founder.name()))
    }

    /// A live founder's model **in its current mode** (F3): its lineage's mode-to-model
    /// table ([`mode_model`]) names a specimen for the mode, and that specimen draws when
    /// the library has it; any other mode, or a missing specimen, is [`Self::animal`].
    pub fn animal_in_mode(&self, founder: Founder, mode: MobilityMode) -> Option<&AnimalModel> {
        mode_model(founder, mode)
            .and_then(|name| self.specimen(name))
            .or_else(|| self.animal(founder))
    }

    pub fn insert_specimen(&mut self, name: &str, model: AnimalModel) {
        self.specimens.insert(name.to_string(), model);
    }

    /// An unsimulated animal's model by its manifest name (`"ripple-snail"`); `_` and `-`
    /// are the same letter here.
    pub fn specimen(&self, name: &str) -> Option<&AnimalModel> {
        self.specimens.get(name).or_else(|| {
            self.specimens
                .iter()
                .find(|(k, _)| same_name(k, name))
                .map(|(_, m)| m)
        })
    }

    /// A founder's rig, when the bake gave it one (FEED-RIG).
    pub fn rig(&self, founder: Founder) -> Option<&std::sync::Arc<super::rig::AnimalRig>> {
        self.rigs.get(founder.name())
    }

    pub fn insert_rig(&mut self, founder: Founder, rig: super::rig::AnimalRig) {
        self.rigs.insert(founder.name().to_string(), std::sync::Arc::new(rig));
    }

    /// Every specimen's name, in name order.
    pub fn specimen_names(&self) -> impl Iterator<Item = &str> {
        self.specimens.keys().map(String::as_str)
    }

    /// The voxel size this library was baked at, if it says.
    pub fn voxel_m(&self) -> Option<f64> {
        self.voxel_m
    }

    /// Whether it serves a world of cells of `voxel_m`: always for a library built by hand,
    /// and for a loaded one only at its own size.
    pub fn serves(&self, voxel_m: f64) -> bool {
        self.voxel_m.is_none_or(|v| (v - voxel_m).abs() <= 1e-9)
    }

    /// How many species, founders and specimens have a model.
    pub fn len(&self) -> usize {
        self.plants.iter().flatten().count()
            + self.animals.iter().flatten().count()
            + self.specimens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Load the models baked for `voxel_m` from `root` (`assets/voxel-models`): the
    /// `<voxel_mm>/manifest.json` there, and each file it names. A species or founder the
    /// manifest does not name has no model and the presenter falls back to its glyph.
    pub fn load(root: &Path, voxel_m: f64) -> Result<ModelLibrary> {
        let dir = root.join(voxel_dir(voxel_m));
        let manifest_path = dir.join("manifest.json");
        let text = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("reading {}", manifest_path.display()))?;
        let manifest: Manifest = serde_json::from_str(&text)
            .with_context(|| format!("parsing {}", manifest_path.display()))?;
        if (manifest.voxel_m - voxel_m).abs() > 1e-9 {
            bail!(
                "{} is baked at {} m, not {voxel_m} m",
                manifest_path.display(),
                manifest.voxel_m
            );
        }
        let mut lib = ModelLibrary {
            voxel_m: Some(voxel_m),
            ..ModelLibrary::default()
        };
        for (name, file) in &manifest.species {
            let Some(sp) = Species::parse(name) else {
                bail!("{}: no species is called {name}", manifest_path.display());
            };
            let bytes = std::fs::read(dir.join(file))
                .with_context(|| format!("reading {}", dir.join(file).display()))?;
            match parse(&bytes).with_context(|| format!("parsing {file}"))? {
                Parsed::Plant(m) => lib.insert_plant(sp, m),
                Parsed::Animal(_) => bail!("{file} is an animal model, listed as a species"),
            }
        }
        for (name, file) in &manifest.founders {
            let Some(f) = Founder::ALL.into_iter().find(|f| f.name() == name) else {
                bail!("{}: no founder is called {name}", manifest_path.display());
            };
            let bytes = std::fs::read(dir.join(file))
                .with_context(|| format!("reading {}", dir.join(file).display()))?;
            match parse(&bytes).with_context(|| format!("parsing {file}"))? {
                Parsed::Animal(m) => lib.insert_animal(f, m),
                Parsed::Plant(_) => bail!("{file} is a plant model, listed as a founder"),
            }
        }
        for (name, file) in &manifest.rigs {
            let Some(f) = Founder::ALL.into_iter().find(|f| f.name() == name) else {
                bail!("{}: no founder is called {name}", manifest_path.display());
            };
            let bytes = std::fs::read(dir.join(file))
                .with_context(|| format!("reading {}", dir.join(file).display()))?;
            let rig = super::rig::AnimalRig::parse(&bytes).with_context(|| format!("parsing {file}"))?;
            let bins = lib.animal(f).map_or(0, |m| m.bins.len());
            if rig.bins.len() != bins {
                bail!("{file}: {} bins for a model of {bins}", rig.bins.len());
            }
            lib.insert_rig(f, rig);
        }
        for (name, file) in &manifest.specimens {
            let bytes = std::fs::read(dir.join(file))
                .with_context(|| format!("reading {}", dir.join(file).display()))?;
            match parse(&bytes).with_context(|| format!("parsing {file}"))? {
                Parsed::Animal(m) => lib.insert_specimen(name, m),
                Parsed::Plant(_) => bail!("{file} is a plant model, listed as a specimen"),
            }
        }
        Ok(lib)
    }
}

/// Whether two lineage or specimen names are the same, `_` and `-` alike.
pub fn same_name(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .all(|(x, y)| x == y || (matches!(x, b'_' | b'-') && matches!(y, b'_' | b'-')))
}

/// `0.125` → `"125"`: the directory a voxel size's models live in.
pub fn voxel_dir(voxel_m: f64) -> String {
    format!("{}", (voxel_m * 1000.0).round() as i64)
}

/// `manifest.json`: which file holds which model. The bake writes more (step heights,
/// cell counts, the palette) for a reader; the loader needs only this.
#[derive(Deserialize)]
struct Manifest {
    voxel_m: f64,
    #[serde(default)]
    species: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    founders: std::collections::BTreeMap<String, String>,
    /// The unsimulated animals, name → file.
    #[serde(default)]
    specimens: std::collections::BTreeMap<String, String>,
    /// The founders' rigs, name → file (FEED-RIG).
    #[serde(default)]
    rigs: std::collections::BTreeMap<String, String>,
}

enum Parsed {
    Plant(PlantModel),
    Animal(AnimalModel),
}

/// Read one `.cvm` file (the module header has the layout).
fn parse(bytes: &[u8]) -> Result<Parsed> {
    let mut r = Reader { bytes, at: 0 };
    if r.take(4)? != b"CVM2" {
        bail!("not a CVM2 model file (a CVM1 library is rebaked, never read)");
    }
    let kind = r.u8()?;
    let n_materials = r.u8()?;
    let mut remap = Vec::with_capacity(usize::from(n_materials));
    for _ in 0..n_materials {
        let len = usize::from(r.u8()?);
        let name = std::str::from_utf8(r.take(len)?).context("a material name")?;
        let Some(i) = palette_index(name) else {
            bail!("material {name:?} is not in the palette");
        };
        remap.push(i);
    }
    let n_states = usize::from(r.u8()?);
    if n_states == 0 {
        bail!("a model with no states");
    }
    let mut states = Vec::with_capacity(n_states);
    for _ in 0..n_states {
        let len = usize::from(r.u8()?);
        let name = std::str::from_utf8(r.take(len)?).context("a state name")?.to_string();
        let start = r.f64()?;
        states.push(StateInfo { name, start });
    }
    let n_steps = r.u16()?;
    let mut steps = Vec::with_capacity(usize::from(n_steps));
    for _ in 0..n_steps {
        let height_m = r.f64()?;
        let radius_m = r.f64()?;
        let n_variants = r.u8()?;
        let mut variants = Vec::with_capacity(usize::from(n_variants));
        let mut reduced = vec![Vec::with_capacity(usize::from(n_variants)); n_states - 1];
        for _ in 0..n_variants {
            for k in 0..n_states {
                let n = r.u32()? as usize;
                let raw = r.take(n.checked_mul(5).context("cell count")?)?;
                let mut cells = Vec::with_capacity(n);
                for c in raw.chunks_exact(5) {
                    let Some(&material) = remap.get(usize::from(c[3])) else {
                        bail!("cell material {} of {n_materials}", c[3]);
                    };
                    let Some(tag) = Tag::from_byte(c[4]) else {
                        bail!("cell tag {:#x}", c[4]);
                    };
                    cells.push(ModelCell {
                        offset: [
                            i32::from(c[0] as i8),
                            i32::from(c[1] as i8),
                            i32::from(c[2] as i8),
                        ],
                        material,
                        tag,
                    });
                }
                match k {
                    0 => variants.push(cells),
                    k => reduced[k - 1].push(cells),
                }
            }
        }
        steps.push(PlantStep {
            height_m,
            radius_m,
            variants,
            states: reduced,
        });
    }
    if r.at != bytes.len() {
        bail!("{} trailing bytes", bytes.len() - r.at);
    }
    let ascending = |v: &[f64]| v.windows(2).all(|w| w[0] <= w[1]);
    Ok(match kind {
        0 => {
            let h: Vec<f64> = steps.iter().map(|s| s.height_m).collect();
            if !ascending(&h) {
                bail!("plant steps are not ascending by height");
            }
            Parsed::Plant(PlantModel { steps, states })
        }
        1 => {
            let bins: Vec<AnimalBin> = steps
                .into_iter()
                .map(|s| AnimalBin {
                    length_m: s.height_m,
                    cells: s.variants.into_iter().next().unwrap_or_default(),
                })
                .collect();
            let l: Vec<f64> = bins.iter().map(|b| b.length_m).collect();
            if !ascending(&l) {
                bail!("animal bins are not ascending by length");
            }
            Parsed::Animal(AnimalModel { bins })
        }
        k => bail!("model kind {k}"),
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(n).context("length")?;
        let Some(s) = self.bytes.get(self.at..end) else {
            bail!("truncated at byte {}", self.at);
        };
        self.at = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.take(8)?.try_into()?))
    }
}

// --- the palette ---------------------------------------------------------------------

/// The model palette: the material names the Blender builders use, which is what a baked
/// cell's `material` indexes. The hex values are the builders' own reference colours
/// (brief §3); the presenter paints from [`super::colours`], by species and height, and
/// uses the material only to tell a part's shades apart.
///
/// The `a_*` names are the animals' **part slots** (model pass MP-A,
/// `scripts/blender/animal_models.py`): what each slot is, and its colour, is per animal
/// ([`super::colours::animal`], [`super::colours::specimen`]); the hex here is only a
/// reference (the frondgrazer's, where it has the slot).
pub const PALETTE: [(&str, u32); 24] = [
    ("plum", 0x002A_0E4A),
    ("violet", 0x003A_1A7A),
    ("detritus", 0x0051_0B6D),
    ("lilac", 0x00B9_9BE6),
    ("p0", 0x001E_2798),
    ("p1", 0x002B_6AD0),
    ("p2", 0x0042_C5F8),
    ("teal", 0x0024_8CA8),
    ("magenta", 0x00FF_2AFC),
    ("warm", 0x00FF_9B50),
    ("starved", 0x007A_2A78),
    ("a_main", 0x00EE_7156),
    ("a_dark", 0x0058_342D),
    ("a_pale", 0x00DF_9F7D),
    ("a_mark", 0x0087_2935),
    ("a_limb", 0x00D9_6862),
    ("a_foot", 0x00C0_927C),
    ("a_head", 0x00FE_D3AC),
    ("a_eye", 0x0030_0919),
    ("a_glow", 0x00F3_70A2),
    ("a_membrane", 0x00DB_DA7B),
    ("a_fruit", 0x00D6_658A),
    ("a_tendril", 0x00EE_B4A7),
    // the lanternjaw's lure orbs (2026-10-01), an accent so each keeps its cell
    ("a_lure", 0x0063_EEFC),
];

/// The index of a palette name.
pub fn palette_index(name: &str) -> Option<u8> {
    PALETTE
        .iter()
        .position(|(n, _)| *n == name)
        .map(|i| i as u8)
}

const fn named(name: &str) -> u8 {
    let mut i = 0;
    while i < PALETTE.len() {
        if const_eq(PALETTE[i].0, name) {
            return i as u8;
        }
        i += 1;
    }
    panic!("not a palette name");
}

const fn const_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// The index of a palette name, for a name this module knows is there.
pub const fn named_index(name: &str) -> u8 {
    named(name)
}

pub const WARM: u8 = named("warm");
pub const MAGENTA: u8 = named("magenta");

/// How many wilt tints a foliage cell can take, `0` (turgid) to `WILT_LEVELS − 1` (at
/// `μ = 0`). Quantised so a meadow shares a handful of styles: the GPU path holds 256.
pub use cubarium_session::view_state::{WILT_LEVELS, wilt_level};

/// The ripe fraction at which a bloom opens: a parcel holding half a package
/// (`design/art-direction/species-dossiers-2026-09-21.md`, "ripe").
pub use cubarium_session::view_state::RIPE_AT;

/// The hash layer lanternberry fruit is thinned under, apart from every foliage index.
pub const FRUIT_LAYER: u8 = 0xff;

// --- thinning ------------------------------------------------------------------------

const VARIANT_SALT: u64 = 0x5EED_0F_A11_7A_11E5;

/// A 64-bit finaliser (splitmix64's): every input bit reaches every output bit.
#[inline]
fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Whether a foliage or drape cell survives thinning when its layer holds `fraction`
/// (= stock / capacity) of its capacity: a pure hash of `(stand_id, offset, layer)`
/// compared against the fraction. So it gives the same answer every frame, keeps
/// everything at 1 (or more) and nothing at 0 (or less), and the cells kept at a lower
/// fraction are a subset of the cells kept at a higher one.
#[inline]
pub fn keeps(stand_id: u64, offset: [i32; 3], layer: u8, fraction: f64) -> bool {
    if !(fraction > 0.0) {
        return false;
    }
    if fraction >= 1.0 {
        return true;
    }
    keep_threshold(stand_id, offset, layer) < fraction
}

/// The fraction above which [`keeps`] keeps this cell: 24 bits of the cell's hash as a
/// fraction in `[0, 1)`. Any fraction within 2^-24 of 1 keeps every cell, so a full layer
/// whose stock is one ulp short of its capacity is full.
#[inline]
pub fn keep_threshold(stand_id: u64, offset: [i32; 3], layer: u8) -> f64 {
    let key = (offset[0] as u16 as u64)
        | ((offset[1] as u16 as u64) << 16)
        | ((offset[2] as u16 as u64) << 32)
        | (u64::from(layer) << 48);
    let h = mix64(mix64(stand_id) ^ key);
    ((h >> 40) as f64) * (1.0 / (1u64 << 24) as f64)
}

// --- foliage states ------------------------------------------------------------------

/// The hash layer a state cross-fade reads ([`fade_shows`]), apart from every foliage
/// index and [`FRUIT_LAYER`]. One hash per cell offset, so an outgoing and an incoming
/// cell at the same offset never show together or leave a gap.
pub const FADE_LAYER: u8 = 0xfe;

/// The state a stand's **total** foliage fraction picks, without hysteresis: how many of
/// the thresholds `below` (descending, state `k` is drawn below `below[k - 1]`) the
/// fraction lies below. A NaN fraction counts as bare.
pub fn state_at(fraction: f64, below: &[f64]) -> usize {
    let f = if fraction.is_nan() { 0.0 } else { fraction };
    below.iter().filter(|&&t| f < t).count()
}

/// The state a stand in state `current` moves to at `fraction`, with `hysteresis`: it
/// goes down (to a barer state) as soon as the fraction is below a threshold, and back
/// up only once the fraction is at or above that threshold plus `hysteresis`. So a
/// fraction hovering on a threshold does not flicker between the two states.
pub fn next_state(current: usize, fraction: f64, below: &[f64], hysteresis: f64) -> usize {
    let down = state_at(fraction, below);
    if down >= current {
        return down;
    }
    let f = if fraction.is_nan() { 0.0 } else { fraction };
    let up = below.iter().filter(|&&t| f < t + hysteresis.max(0.0)).count();
    up.clamp(down, current)
}

/// **A stand's drawn foliage state and its cross-fade**: it shows state `from` blended
/// toward state `to` by `blend` in `[0, 1]`; settled, `from == to`. The blend moves on the
/// presenter's frame clock ([`FoliageTrack::advance`]) and never jumps, so a stand never
/// pops between states.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FoliageTrack {
    pub from: u8,
    pub to: u8,
    pub blend: f64,
}

impl FoliageTrack {
    /// Settled in `state`, no fade: what a stand seen for the first time draws.
    pub fn settled(state: u8) -> FoliageTrack {
        FoliageTrack {
            from: state,
            to: state,
            blend: 1.0,
        }
    }

    pub fn is_fading(&self) -> bool {
        self.from != self.to
    }

    /// The state it shows once the fade is done.
    pub fn state(&self) -> u8 {
        self.to
    }

    /// Move `dt` seconds toward state `aim`, a whole fade taking `fade_s` (0: at once).
    /// Settled elsewhere, it starts fading to `aim` directly, however many states apart.
    /// Asked back to the state it is fading from, it runs the same fade backward, so the
    /// cells it already swapped swap back one by one. Asked somewhere else mid-fade, it
    /// first finishes the fade it is in and then starts the next.
    pub fn advance(&mut self, aim: u8, dt: f64, fade_s: f64) {
        let step = if fade_s > 0.0 {
            (dt.max(0.0) / fade_s).min(1.0)
        } else {
            1.0
        };
        if self.from == self.to {
            if aim == self.to {
                return;
            }
            self.from = self.to;
            self.to = aim;
            self.blend = 0.0;
        }
        if aim == self.from {
            self.blend -= step;
            if self.blend <= 0.0 {
                *self = FoliageTrack::settled(self.from);
            }
        } else {
            self.blend += step;
            if self.blend >= 1.0 {
                *self = FoliageTrack::settled(self.to);
            }
        }
    }
}

/// Whether a cell **only one** of the two states holds shows at `blend` of the fade
/// between them: an incoming cell once its [`FADE_LAYER`] hash is below the blend, an
/// outgoing one while its hash is at or above it. A cell both states hold always shows
/// ([`fade_cells`]). Pure, so a stand fades the same way every time.
#[inline]
pub fn fade_shows(stand_id: u64, offset: [i32; 3], incoming: bool, blend: f64) -> bool {
    let h = keep_threshold(stand_id, offset, FADE_LAYER);
    if incoming { h < blend } else { h >= blend }
}

/// The cells drawn at `blend` of the fade from `from` to `to`: every cell both hold, and
/// of the rest those [`fade_shows`] keeps. At blend 0 it is `from`, at 1 it is `to`.
pub fn fade_cells(from: &[ModelCell], to: &[ModelCell], stand_id: u64, blend: f64) -> Vec<ModelCell> {
    let mut out = Vec::with_capacity(from.len().max(to.len()));
    for m in to {
        if from.contains(m) || fade_shows(stand_id, m.offset, true, blend) {
            out.push(*m);
        }
    }
    for m in from {
        if !to.contains(m) && fade_shows(stand_id, m.offset, false, blend) {
            out.push(*m);
        }
    }
    out
}

// --- stamping ------------------------------------------------------------------------

/// A plant model at `anchor`, the cell just above the stand's support face in its own
/// column. Each cell goes to `anchor + offset`: `x` wraps with the ring (the returned `x`
/// is unwrapped), a `y` or `z` outside the world is dropped, a cell in solid terrain is
/// dropped. A `Foliage(i)` or `Drape(i)` cell is kept iff `keeps(stand_id, offset, i,
/// foliage[i])`, a layer past the end of `foliage` counting as full. Trunk and accent
/// cells are never thinned by foliage.
pub fn stamp_plant(
    cells: &[ModelCell],
    anchor: Cell,
    stand_id: u64,
    foliage: &[f64],
    view: &VoxelView<'_>,
) -> Vec<(Cell, ModelCell)> {
    let mut out = Vec::with_capacity(cells.len());
    each_plant_cell(cells, anchor, stand_id, foliage, view, |c, m| {
        out.push((c, m))
    });
    out
}

/// [`stamp_plant`] without the allocation: what the presenter calls every tick.
#[inline]
pub(crate) fn each_plant_cell(
    cells: &[ModelCell],
    anchor: Cell,
    stand_id: u64,
    foliage: &[f64],
    view: &VoxelView<'_>,
    mut f: impl FnMut(Cell, ModelCell),
) {
    let c = view.config;
    let (h, d) = (i64::from(c.height), i64::from(c.depth));
    let (ay, az) = (i64::from(anchor.y), i64::from(anchor.z));
    for m in cells {
        let y = ay + i64::from(m.offset[1]);
        let z = az + i64::from(m.offset[2]);
        if y < 0 || y >= h || z < 0 || z >= d {
            continue;
        }
        if let Tag::Foliage(i) | Tag::Drape(i) = m.tag {
            let fraction = foliage.get(usize::from(i)).copied().unwrap_or(1.0);
            if !keeps(stand_id, m.offset, i, fraction) {
                continue;
            }
        }
        let x = anchor.x + i64::from(m.offset[0]);
        let (y, z) = (y as u32, z as u32);
        if view.material_at(x, y, z).is_solid() {
            continue;
        }
        f(Cell { x, y, z }, *m);
    }
}

/// Which of the four headings `heading_rad` is nearest, as quarter turns from the model's
/// own +x (heading π/2; `Pose::forward` is `(sin h, cos h)`): 0 faces +x, 1 −z, 2 −x, 3 +z.
pub fn quarter_turns(heading_rad: f64) -> u8 {
    if !heading_rad.is_finite() {
        return 0;
    }
    let q = ((heading_rad - std::f64::consts::FRAC_PI_2) / std::f64::consts::FRAC_PI_2).round();
    q.rem_euclid(4.0) as u8 % 4
}

/// `(dx, dz)` turned by `q` quarter turns: `(dz, −dx)` facing −z, `(−dx, −dz)` facing
/// −x, `(−dz, dx)` facing +z. A rotation, never a mirror.
#[inline]
pub fn turn(q: u8, dx: i32, dz: i32) -> (i32, i32) {
    match q & 3 {
        0 => (dx, dz),
        1 => (dz, -dx),
        2 => (-dx, -dz),
        _ => (-dz, dx),
    }
}

/// An animal model at `anchor = (floor(pose.x / voxel_m), site.y + 1, floor(pose.z /
/// voxel_m))`, turned to the nearest of four headings ([`quarter_turns`], [`turn`]).
/// Clipping and burial work as for a plant, and nothing is thinned.
pub fn stamp_animal(
    cells: &[ModelCell],
    anchor: Cell,
    heading_rad: f64,
    view: &VoxelView<'_>,
) -> Vec<(Cell, ModelCell)> {
    let mut out = Vec::with_capacity(cells.len());
    each_animal_cell(cells, anchor, heading_rad, view, |c, m| out.push((c, m)));
    out
}

/// [`stamp_animal`] without the allocation.
#[inline]
pub(crate) fn each_animal_cell(
    cells: &[ModelCell],
    anchor: Cell,
    heading_rad: f64,
    view: &VoxelView<'_>,
    mut f: impl FnMut(Cell, ModelCell),
) {
    let c = view.config;
    let (h, d) = (i64::from(c.height), i64::from(c.depth));
    let q = quarter_turns(heading_rad);
    for m in cells {
        let (dx, dz) = turn(q, m.offset[0], m.offset[2]);
        let y = i64::from(anchor.y) + i64::from(m.offset[1]);
        let z = i64::from(anchor.z) + i64::from(dz);
        if y < 0 || y >= h || z < 0 || z >= d {
            continue;
        }
        let x = anchor.x + i64::from(dx);
        let (y, z) = (y as u32, z as u32);
        if view.material_at(x, y, z).is_solid() {
            continue;
        }
        f(Cell { x, y, z }, *m);
    }
}

/// [`each_animal_cell`] for a body **on a wall** (package mobility, decision 5; an
/// interim stage-1 look, not an art decision): the model pitched flat against the face,
/// its belly on it, head up the face when `head_up` and down it otherwise. `toward` is
/// the unit `(dx, dz)` from the body's column into the face; `anchor` is the air cell
/// beside the face at the feet's layer.
///
/// The model's own axes — forward `+x`, back `+y`, side `+z` — go to up (or down) the
/// face, straight out of it, and their cross product, so it is a rotation and never a
/// mirror.
pub(crate) fn each_animal_cell_on_wall(
    cells: &[ModelCell],
    anchor: Cell,
    toward: (i32, i32),
    head_up: bool,
    view: &VoxelView<'_>,
    mut f: impl FnMut(Cell, ModelCell),
) {
    let c = view.config;
    let (h, d) = (i64::from(c.height), i64::from(c.depth));
    let fy: i64 = if head_up { 1 } else { -1 };
    // Back: out of the face. Side: forward × back.
    let (ux, uz) = (-i64::from(toward.0), -i64::from(toward.1));
    let (sx, sy, sz) = (fy * uz, 0i64, -fy * ux);
    for m in cells {
        let (ox, oy, oz) = (
            i64::from(m.offset[0]),
            i64::from(m.offset[1]),
            i64::from(m.offset[2]),
        );
        let x = anchor.x + oy * ux + oz * sx;
        let y = i64::from(anchor.y) + ox * fy + oz * sy;
        let z = i64::from(anchor.z) + oy * uz + oz * sz;
        if y < 0 || y >= h || z < 0 || z >= d {
            continue;
        }
        let (y, z) = (y as u32, z as u32);
        if view.material_at(x, y, z).is_solid() {
            continue;
        }
        f(Cell { x, y, z }, *m);
    }
}

// --- the source dump -----------------------------------------------------------------

/// The model's own numbers for the Blender bake (§1 of the brief): every species' crown
/// ranges in metres and stage profiles, every founder's adult dimensions, and the voxel
/// sizes the shipped presets use. `examples/voxel_model_source.rs` writes it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceDump {
    /// Every distinct `voxel_m` of `cubarium_voxel::recipe::PRESETS` and the terrarium,
    /// ascending: the sizes the bake voxelizes at.
    pub voxel_sizes_m: Vec<f64>,
    pub species: Vec<SpeciesSource>,
    pub founders: Vec<FounderSource>,
}

/// One per `Species::ALL`, `name` = `Species::name()`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpeciesSource {
    pub name: String,
    pub crown_height_m: [f64; 2],
    pub crown_radius_m: [f64; 2],
    pub profile: Vec<Stage>,
    /// The species' foliage-state thresholds ([`cubarium_voxel_flora::SpeciesConfig::foliage_state_below`]):
    /// the bake writes them into the model's state table as each reduced state's start.
    #[serde(default)]
    pub foliage_state_below: Vec<f64>,
}

/// A mirror of flora's `Profile` whose threshold survives JSON: the last stage's
/// `wood_fraction_max` is infinite, which `serde_json` would write as `null`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stage {
    #[serde(with = "non_finite")]
    pub wood_fraction_max: f64,
    /// The seedling cap, metres.
    pub height_m_max: Option<f64>,
    pub layers: Vec<Layer>,
}

/// One per `Founder::ALL`, `name` = `Founder::name()`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FounderSource {
    pub name: String,
    pub adult_length_m: f64,
    pub adult_width_m: f64,
    pub adult_height_m: f64,
    /// The newborn's and the adult's structure: a body's length is the adult's times
    /// `(body / body_max)^(1/3)`, which is where the size bins come from.
    pub body_min: f64,
    pub body_max: f64,
}

impl SourceDump {
    pub fn from_model(flora: &FloraConfig, fauna: &FaunaConfig) -> SourceDump {
        let mut voxel_sizes_m: Vec<f64> = cubarium_voxel::recipe::PRESETS
            .iter()
            .map(|p| p.voxel_m)
            .collect();
        voxel_sizes_m.sort_by(f64::total_cmp);
        voxel_sizes_m.dedup();
        SourceDump {
            voxel_sizes_m,
            species: Species::ALL
                .into_iter()
                .map(|sp| {
                    let sc = flora.species(sp);
                    SpeciesSource {
                        name: sp.name().to_string(),
                        crown_height_m: sc.crown_height_m,
                        crown_radius_m: sc.crown_radius_m,
                        profile: sc
                            .profile
                            .iter()
                            .map(|p| Stage {
                                wood_fraction_max: p.wood_fraction_max,
                                height_m_max: p.height_m_max,
                                layers: p.layers.clone(),
                            })
                            .collect(),
                        foliage_state_below: sc.foliage_state_below.clone(),
                    }
                })
                .collect(),
            founders: Founder::ALL
                .into_iter()
                .map(|f| {
                    let phys = fauna.founder(f);
                    FounderSource {
                        name: f.name().to_string(),
                        adult_length_m: phys.adult_length_m,
                        adult_width_m: phys.adult_width_m,
                        adult_height_m: phys.adult_height_m,
                        body_min: phys.core.body_min,
                        body_max: phys.core.body_max,
                    }
                })
                .collect(),
        }
    }
}

/// An `f64` that may be infinite or NaN: a number when finite, else `"inf"`, `"-inf"` or
/// `"nan"`. Python's `float()` reads all three.
mod non_finite {
    use serde::de::{self, Visitor};
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
        if v.is_finite() {
            s.serialize_f64(*v)
        } else if v.is_nan() {
            s.serialize_str("nan")
        } else if *v > 0.0 {
            s.serialize_str("inf")
        } else {
            s.serialize_str("-inf")
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = f64;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a number, \"inf\", \"-inf\" or \"nan\"")
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<f64, E> {
                Ok(v)
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<f64, E> {
                Ok(v as f64)
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<f64, E> {
                Ok(v as f64)
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<f64, E> {
                match v {
                    "inf" => Ok(f64::INFINITY),
                    "-inf" => Ok(f64::NEG_INFINITY),
                    "nan" => Ok(f64::NAN),
                    _ => Err(E::invalid_value(de::Unexpected::Str(v), &self)),
                }
            }
        }
        d.deserialize_any(V)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loftstrider_bins_follow_the_founder_length_rule_at_each_voxel_size() {
        let fauna = FaunaConfig::default();
        let phys = fauna.founder(Founder::Loftstrider);
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/voxel-models");
        for vox in SourceDump::from_model(&FloraConfig::default(), &fauna).voxel_sizes_m {
            let lib = ModelLibrary::load(&root, vox).unwrap();
            let model = lib.animals[Founder::Loftstrider.index()]
                .as_ref()
                .expect("founder bake");
            assert_eq!(model.bins.len(), 5);
            let s0 = (phys.core.body_min / phys.core.body_max).cbrt();
            for (n, bin) in model.bins.iter().enumerate() {
                let s = s0 + (1.0 - s0) * n as f64 / (model.bins.len() - 1) as f64;
                let length = phys.adult_length_m * s;
                assert!((bin.length_m - length).abs() < 1e-12);
                assert!(!bin.cells.is_empty());
                assert!(std::ptr::eq(
                    model.select(length).unwrap().as_ptr(),
                    bin.cells.as_ptr()
                ));
            }
            assert_ne!(
                model.bins.first().unwrap().cells,
                model.bins.last().unwrap().cells
            );
            assert!(std::ptr::eq(
                model
                    .select(phys.body_at(phys.core.body_min).length_m)
                    .unwrap()
                    .as_ptr(),
                model.bins[0].cells.as_ptr(),
            ));
        }
    }

    /// One animal file, one bin of one cell in `material`, as the bake writes a specimen.
    fn one_bin_animal(material: &str, length_m: f64) -> Vec<u8> {
        let mut b = b"CVM2".to_vec();
        b.extend_from_slice(&[1, 1, material.len() as u8]);
        b.extend_from_slice(material.as_bytes());
        push_states(&mut b, &[("full", 1.0)]);
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&length_m.to_le_bytes());
        b.extend_from_slice(&0.1f64.to_le_bytes());
        b.push(1);
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&[0, 1, 0, 0, 0x00]);
        b
    }

    #[test]
    fn specimens_load_by_name_beside_the_founders() {
        let root = std::env::temp_dir().join(format!("cubarium-specimens-{}", std::process::id()));
        let dir = root.join(voxel_dir(0.125));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ripple-snail.cvm"), one_bin_animal("a_main", 0.19)).unwrap();
        std::fs::write(dir.join("grazer.cvm"), one_bin_animal("a_mark", 0.75)).unwrap();
        std::fs::write(
            dir.join("manifest.json"),
            r#"{"voxel_m": 0.125, "founders": {"frondgrazer": "grazer.cvm"},
                "specimens": {"ripple-snail": "ripple-snail.cvm"}}"#,
        )
        .unwrap();
        let lib = ModelLibrary::load(&root, 0.125).unwrap();
        let snail = lib.specimen("ripple-snail").expect("the specimen, by name");
        let cells = snail.select(f64::INFINITY).unwrap();
        assert_eq!(cells[0].material, named_index("a_main"));
        assert_eq!(cells[0].offset, [0, 1, 0]);
        assert!(lib.specimen("chorister").is_none(), "an unlisted name has no model");
        assert!(lib.animal(Founder::Browser).is_some(), "the founder still loads");
        assert_eq!(lib.specimen_names().collect::<Vec<_>>(), ["ripple-snail"]);
        assert_eq!(lib.len(), 2);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// Roster R0: a live founder with no founder model of its own draws with the specimen
    /// of its lineage name (`_` and `-` alike), so a new founder needs no rebake; a
    /// baked founder model still wins over a specimen of the same name.
    #[test]
    fn a_founder_with_no_founder_model_draws_with_its_specimen() {
        let parse_animal = |bytes: Vec<u8>| match parse(&bytes).unwrap() {
            Parsed::Animal(m) => m,
            Parsed::Plant(_) => unreachable!("an animal file"),
        };
        let mut lib = ModelLibrary::default();
        assert!(lib.animal(Founder::Lanternjaw).is_none(), "nothing to draw yet");
        lib.insert_specimen("lanternjaw", parse_animal(one_bin_animal("a_main", 1.0)));
        let drawn = lib.animal(Founder::Lanternjaw).expect("the specimen stands in");
        assert_eq!(drawn.select(1.0).unwrap()[0].material, named_index("a_main"));
        lib.insert_animal(Founder::Lanternjaw, parse_animal(one_bin_animal("a_mark", 1.0)));
        let own = lib.animal(Founder::Lanternjaw).unwrap();
        assert_eq!(own.select(1.0).unwrap()[0].material, named_index("a_mark"), "its own first");
        assert!(same_name("ripple_snail", "ripple-snail") && !same_name("ripple", "ripple-snail"));
        assert!(super::super::colours::specimen_coat("ripple_snail").is_some(), "the coat too");
    }

    /// The state table, as the bake writes it.
    fn push_states(b: &mut Vec<u8>, states: &[(&str, f64)]) {
        b.push(states.len() as u8);
        for (name, start) in states {
            b.push(name.len() as u8);
            b.extend_from_slice(name.as_bytes());
            b.extend_from_slice(&start.to_le_bytes());
        }
    }

    #[test]
    fn a_tag_round_trips_its_byte() {
        for t in [
            Tag::Trunk,
            Tag::Foliage(0),
            Tag::Foliage(3),
            Tag::Drape(1),
            Tag::Accent,
            Tag::Fallen(1),
        ] {
            assert_eq!(Tag::from_byte(t.to_byte()), Some(t));
        }
        assert_eq!(Tag::from_byte(0x41), Some(Tag::Fallen(1)), "the bake's fallen byte");
        assert_eq!(Tag::from_byte(0x50), None);
    }

    /// A version 1 file is refused: the library is rebaked, never read across versions.
    #[test]
    fn a_version_1_file_is_refused() {
        let mut b = one_bin_animal("a_main", 0.5);
        b[..4].copy_from_slice(b"CVM1");
        assert!(parse(&b).is_err());
    }

    /// A plant file with three states reads back each variant's cells per state, in the
    /// file's order, the full model as `variants` and the rest as `states`, and the state
    /// table's names and starts.
    #[test]
    fn a_file_with_foliage_states_parses_each_state() {
        let mut b = b"CVM2".to_vec();
        b.extend_from_slice(&[0, 1, 4]);
        b.extend_from_slice(b"plum");
        push_states(&mut b, &[("full", 1.0), ("browsed", 0.6), ("stripped", 0.2)]);
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&0.5f64.to_le_bytes());
        b.extend_from_slice(&0.25f64.to_le_bytes());
        b.push(2);
        // Variant v, state k: k + 1 cells at height 10 v + k; the last one fallen.
        for v in 0..2u8 {
            for k in 0..3u8 {
                b.extend_from_slice(&u32::from(k + 1).to_le_bytes());
                for c in 0..=k {
                    let tag = if c == k && k == 2 { 0x41 } else { 0x10 };
                    b.extend_from_slice(&[c, 10 * v + k, 0, 0, tag]);
                }
            }
        }
        let Parsed::Plant(m) = parse(&b).unwrap() else {
            panic!("a plant");
        };
        assert_eq!(m.state_count(), 3);
        assert_eq!(
            m.states.iter().map(|s| (s.name.as_str(), s.start)).collect::<Vec<_>>(),
            [("full", 1.0), ("browsed", 0.6), ("stripped", 0.2)]
        );
        let step = &m.steps[0];
        for v in 0..2usize {
            for k in 0..3usize {
                let cells = step.cells(v, k).expect("every variant in every state");
                assert_eq!(cells.len(), k + 1);
                assert!(cells.iter().all(|c| c.offset[1] == 10 * v as i32 + k as i32));
            }
            assert!(step.cells(v, 3).is_none(), "no fourth state");
        }
        assert_eq!(step.cells(1, 2).unwrap()[2].tag, Tag::Fallen(1));
        assert_eq!(step.cells(0, 0), Some(&step.variants[0][..]), "state 0 is the full model");
    }

    #[test]
    fn a_written_file_parses_back() {
        // One plant step with two variants, written the way the bake writes it.
        let mut b = b"CVM2".to_vec();
        b.push(0);
        b.push(2);
        for n in ["violet", "warm"] {
            b.push(n.len() as u8);
            b.extend_from_slice(n.as_bytes());
        }
        push_states(&mut b, &[("full", 1.0)]);
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&0.25f64.to_le_bytes());
        b.extend_from_slice(&0.125f64.to_le_bytes());
        b.push(2);
        for cells in [
            vec![[0i8, 0, 0, 0, 0x00]],
            vec![[1, 2, -1, 1, 0x30], [0, 1, 0, 0, 0x11]],
        ] {
            b.extend_from_slice(&(cells.len() as u32).to_le_bytes());
            for c in cells {
                b.extend(c.iter().map(|&v| v as u8));
            }
        }
        let Parsed::Plant(m) = parse(&b).unwrap() else {
            panic!("a plant");
        };
        assert_eq!(m.steps.len(), 1);
        assert_eq!(m.steps[0].height_m, 0.25);
        assert_eq!(
            m.steps[0].variants[1][0],
            ModelCell {
                offset: [1, 2, -1],
                material: WARM,
                tag: Tag::Accent
            }
        );
        assert_eq!(m.steps[0].variants[1][1].tag, Tag::Foliage(1));
        assert!(
            parse(&b[..b.len() - 1]).is_err(),
            "a truncated file is refused"
        );
    }

    /// Package mobility's interim wall pose: a body three cells long with a one-cell back
    /// is stood on end beside the face — its length up the face (or down it), its back
    /// out of it, never into the solid.
    #[test]
    fn a_body_on_a_wall_is_pitched_flat_against_the_face() {
        let world = cubarium_voxel::World::empty(cubarium_voxel::Config {
            width: 8,
            height: 12,
            depth: 8,
            voxel_m: 0.25,
            ..cubarium_voxel::Config::default()
        });
        let view = world.view();
        let cell = |x: i32, y: i32| ModelCell {
            offset: [x, y, 0],
            material: 0,
            tag: Tag::Trunk,
        };
        let model = [cell(0, 0), cell(1, 0), cell(2, 0), cell(1, 1)];
        let anchor = Cell { x: 3, y: 5, z: 3 };
        for (head_up, dy) in [(true, 1i64), (false, -1)] {
            let mut got = Vec::new();
            // The face is at +x of the body's column.
            each_animal_cell_on_wall(&model, anchor, (1, 0), head_up, &view, |c, m| {
                got.push((c, m.offset))
            });
            assert_eq!(got.len(), 4);
            for (c, o) in &got {
                assert_eq!(
                    i64::from(c.y),
                    5 + dy * i64::from(o[0]),
                    "length along the face"
                );
                assert_eq!(c.x, 3 - i64::from(o[1]), "the back points out of the face");
                assert_eq!(c.z, 3);
            }
        }
    }
}
