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
//! - **Foliage thins by layer.** A foliage or drape cell of layer `i` is drawn iff
//!   [`keeps`] says so at that layer's `stock / capacity`: a pure hash of stand, cell and
//!   layer against the fraction, so a browsed rosette visibly empties while the crown
//!   above it stays full, the same half shows every frame, and regrowth only adds cells.
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
//! tenth of the size of the same cells in JSON:
//!
//! ```text
//! b"CVM1"  kind u8 (0 plant, 1 animal)
//! n_materials u8, then per material: len u8, name bytes (a PALETTE name)
//! n_steps u16, then per step:
//!     height_m f64 (a bin's length for an animal), radius_m f64, n_variants u8,
//!     per variant: n_cells u32, then per cell: dx i8, dy i8, dz i8, material u8, tag u8
//! tag: 0x00 trunk, 0x10|i foliage i, 0x20|i drape i, 0x30 accent
//! ```

use std::path::Path;

use anyhow::{Context, Result, bail};
use cubarium_voxel::VoxelView;
use cubarium_voxel_fauna::{FaunaConfig, Founder};
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
            _ => None,
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            Tag::Trunk => 0x00,
            Tag::Foliage(i) => 0x10 | (i & 0x0f),
            Tag::Drape(i) => 0x20 | (i & 0x0f),
            Tag::Accent => 0x30,
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
#[derive(Clone, Debug, PartialEq)]
pub struct PlantStep {
    pub height_m: f64,
    pub radius_m: f64,
    pub variants: Vec<Vec<ModelCell>>,
}

/// A species at every step, ascending by height.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlantModel {
    pub steps: Vec<PlantStep>,
}

impl PlantModel {
    /// The step whose `height_m` is nearest `height_m` (a tie goes to the **taller** step,
    /// as `stand::crown_height_voxels` rounds 2.5 up to 3), and in it the variant
    /// `stand_id` picks. The variant depends on the id alone, so a growing stand keeps its
    /// phase. `None` with no steps, or a step with no variants.
    pub fn select(&self, height_m: f64, stand_id: u64) -> Option<&[ModelCell]> {
        let step = &self.steps[nearest(self.steps.iter().map(|s| s.height_m), height_m)?];
        let n = step.variants.len() as u64;
        if n == 0 {
            return None;
        }
        Some(&step.variants[(mix64(stand_id ^ VARIANT_SALT) % n) as usize])
    }
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
fn nearest(values: impl Iterator<Item = f64>, want: f64) -> Option<usize> {
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

// --- the library ---------------------------------------------------------------------

/// Every model the presenter can stamp: at most one per species and one per founder.
#[derive(Clone, Debug, Default)]
pub struct ModelLibrary {
    plants: [Option<PlantModel>; Species::COUNT],
    animals: [Option<AnimalModel>; Founder::COUNT],
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

    pub fn animal(&self, founder: Founder) -> Option<&AnimalModel> {
        self.animals[founder.index()].as_ref()
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

    /// How many species and founders have a model.
    pub fn len(&self) -> usize {
        self.plants.iter().flatten().count() + self.animals.iter().flatten().count()
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
        Ok(lib)
    }
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
}

enum Parsed {
    Plant(PlantModel),
    Animal(AnimalModel),
}

/// Read one `.cvm` file (the module header has the layout).
fn parse(bytes: &[u8]) -> Result<Parsed> {
    let mut r = Reader { bytes, at: 0 };
    if r.take(4)? != b"CVM1" {
        bail!("not a CVM1 model file");
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
    let n_steps = r.u16()?;
    let mut steps = Vec::with_capacity(usize::from(n_steps));
    for _ in 0..n_steps {
        let height_m = r.f64()?;
        let radius_m = r.f64()?;
        let n_variants = r.u8()?;
        let mut variants = Vec::with_capacity(usize::from(n_variants));
        for _ in 0..n_variants {
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
            variants.push(cells);
        }
        steps.push(PlantStep {
            height_m,
            radius_m,
            variants,
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
            Parsed::Plant(PlantModel { steps })
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

/// The model palette: brief §3's colours, which are the art direction's
/// (`design/art-direction/Cubarium_Art_Direction_v0.1.md`), by the names the Blender
/// builders use. `starved` is D3/D4's desaturated sense patch.
pub const PALETTE: [(&str, u32); 11] = [
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

pub const PLUM: u8 = named("plum");
pub const P1: u8 = named("p1");
pub const WARM: u8 = named("warm");
pub const MAGENTA: u8 = named("magenta");
pub const STARVED: u8 = named("starved");

/// A palette colour as sRGB hex. A material the palette does not have (a hand-built test
/// model's arbitrary numbers) paints in the first entry rather than failing.
pub fn palette_srgb(material: u8) -> u32 {
    PALETTE
        .get(usize::from(material))
        .map_or(PALETTE[0].1, |(_, c)| *c)
}

/// How many wilt tints a foliage cell can take, `0` (turgid) to `WILT_LEVELS − 1` (at
/// `μ = 0`). Quantised so a meadow shares a handful of styles: the GPU path holds 256.
pub const WILT_LEVELS: u8 = 5;

/// The wilt level of a stand at moisture `μ`.
pub fn wilt_level(moisture: f64) -> u8 {
    let wilt = 1.0 - moisture.clamp(0.0, 1.0);
    let top = f64::from(WILT_LEVELS - 1);
    if wilt.is_finite() {
        (wilt * top).round().clamp(0.0, top) as u8
    } else {
        0
    }
}

/// The ripe fraction at which a bloom opens: a parcel holding half a package
/// (`design/art-direction/species-dossiers-2026-09-21.md`, "ripe").
pub const RIPE_AT: f64 = 0.5;

/// What a warm accent paints while it is not ripe: the bloomcrown's core is `p1` until it
/// turns warm (D9), and a lanternberry bell is closed plum (D12). Other species' warm
/// accents take the core colour.
pub fn unripe_material(species: Species) -> u8 {
    match species {
        Species::Lanternberry => PLUM,
        _ => P1,
    }
}

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
    let key = (offset[0] as u16 as u64)
        | ((offset[1] as u16 as u64) << 16)
        | ((offset[2] as u16 as u64) << 32)
        | (u64::from(layer) << 48);
    let h = mix64(mix64(stand_id) ^ key);
    // 24 bits of the hash as a fraction in [0, 1): any fraction within 2^-24 of 1 keeps
    // every cell, so a full layer whose stock is one ulp short of its capacity is full.
    ((h >> 40) as f64) * (1.0 / (1u64 << 24) as f64) < fraction
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
    fn a_tag_round_trips_its_byte() {
        for t in [
            Tag::Trunk,
            Tag::Foliage(0),
            Tag::Foliage(3),
            Tag::Drape(1),
            Tag::Accent,
        ] {
            assert_eq!(Tag::from_byte(t.to_byte()), Some(t));
        }
        assert_eq!(Tag::from_byte(0x40), None);
    }

    #[test]
    fn a_written_file_parses_back() {
        // One plant step with two variants, written the way the bake writes it.
        let mut b = b"CVM1".to_vec();
        b.push(0);
        b.push(2);
        for n in ["violet", "warm"] {
            b.push(n.len() as u8);
            b.extend_from_slice(n.as_bytes());
        }
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
}
