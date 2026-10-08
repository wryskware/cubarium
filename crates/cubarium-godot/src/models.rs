//! **The baked voxel models** for the Godot view: a reader of the `.cvm` files Blender bakes
//! (`assets/voxel-models/<voxel mm>/`, the format in `crates/cubarium/src/voxel/model.rs`)
//! and `VoxelModels`, which turns one step, variant or size bin into an `ArrayMesh`
//! coloured from `game/world/data/palette.json`.
//!
//! Which step a plant shows: the step whose growth (`built_at.t` in the manifest, the wood
//! fraction the bake built it at) is nearest the feed's `growth`; the variant is picked by
//! the stand's id with the presenter's own hash, so a stand keeps its variant. Which bin an
//! animal shows: the bin whose length is nearest `adult length · size^(1/3)` (the feed
//! carries the body's size over its breeding size, not its length).
//!
//! A model mesh is in voxels, cells centred on the anchor column: cell `(dx, dy, dz)` is
//! the box `[dx ± ½] × [dy, dy + 1] × [−dz ± ½]` in Godot's axes, facing `+x`. Faces between
//! two cells of the model, and bottoms, are left out.

use std::collections::{BTreeMap, HashMap};

use godot::classes::{ArrayMesh, IRefCounted};
use godot::prelude::*;
use serde::Deserialize;

use crate::mesher::{DIRS, Dir, MeshData, array_mesh};
use crate::palette::{Linear, PaletteFile, hex_linear};

/// One baked cell: offset from the anchor, a model-palette index and the part tag byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub offset: [i32; 3],
    pub material: u8,
    pub tag: u8,
}

/// A plant's crown-height step, or an animal's size bin (one variant).
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    /// A plant step's height, or an animal bin's length, metres.
    pub size_m: f64,
    pub variants: Vec<Vec<Cell>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub animal: bool,
    pub steps: Vec<Step>,
    /// A plant step's growth (wood fraction) from the manifest, per step.
    pub growth: Vec<f64>,
}

/// Read a `.cvm` file. `palette` is the model palette's names in index order: a file names
/// its materials, and a cell's material becomes that name's index.
pub fn parse_cvm(bytes: &[u8], palette: &[String]) -> Result<Model, String> {
    let mut r = Reader { bytes, at: 0 };
    if r.take(4)? != b"CVM1" {
        return Err("not a CVM1 model file".into());
    }
    let kind = r.take(1)?[0];
    let n_materials = r.take(1)?[0];
    let mut remap = Vec::new();
    for _ in 0..n_materials {
        let len = usize::from(r.take(1)?[0]);
        let name = std::str::from_utf8(r.take(len)?).map_err(|_| "a material name".to_string())?;
        let i = palette
            .iter()
            .position(|p| p == name)
            .ok_or_else(|| format!("material {name:?} is not in the palette"))?;
        remap.push(i as u8);
    }
    let n_steps = u16::from_le_bytes(r.take(2)?.try_into().unwrap());
    let mut steps = Vec::new();
    for _ in 0..n_steps {
        let size_m = f64::from_le_bytes(r.take(8)?.try_into().unwrap());
        let _radius_m = f64::from_le_bytes(r.take(8)?.try_into().unwrap());
        let n_variants = r.take(1)?[0];
        let mut variants = Vec::new();
        for _ in 0..n_variants {
            let n = u32::from_le_bytes(r.take(4)?.try_into().unwrap()) as usize;
            let raw = r.take(n.checked_mul(5).ok_or("cell count")?)?;
            let mut cells = Vec::with_capacity(n);
            for c in raw.chunks_exact(5) {
                let material = *remap.get(usize::from(c[3])).ok_or("cell material")?;
                cells.push(Cell {
                    offset: [
                        i32::from(c[0] as i8),
                        i32::from(c[1] as i8),
                        i32::from(c[2] as i8),
                    ],
                    material,
                    tag: c[4],
                });
            }
            variants.push(cells);
        }
        steps.push(Step { size_m, variants });
    }
    if r.at != bytes.len() {
        return Err(format!("{} trailing bytes", bytes.len() - r.at));
    }
    match kind {
        0 | 1 => Ok(Model {
            animal: kind == 1,
            steps,
            growth: Vec::new(),
        }),
        k => Err(format!("model kind {k}")),
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).ok_or("length")?;
        let s = self
            .bytes
            .get(self.at..end)
            .ok_or_else(|| format!("truncated at byte {}", self.at))?;
        self.at = end;
        Ok(s)
    }
}

/// The colour band of a cell `dy` above its anchor in a model whose highest cell is `top`
/// (`voxel::colours::band`, four bands).
pub fn band(dy: i32, top: i32) -> u8 {
    const BANDS: u8 = 4;
    if top <= 0 {
        return BANDS - 1;
    }
    let t = dy.clamp(0, top) as f32 / top as f32;
    (t * f32::from(BANDS - 1)).round() as u8
}

const VARIANT_SALT: u64 = 0x5EED_0F_A11_7A_11E5;

fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The index of the value nearest `want`, a tie to the later.
fn nearest(values: impl Iterator<Item = f64>, want: f64) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, v) in values.enumerate() {
        let d = (v - want).abs();
        match best {
            Some((_, bd)) if d > bd => {}
            _ => best = Some((i, d)),
        }
    }
    best.map(|(i, _)| i)
}

impl Model {
    /// A plant's step and variant for `growth` and a stand id.
    pub fn plant_pick(&self, growth: f64, stand_id: u64) -> Option<(usize, usize)> {
        let n = self.steps.len();
        let step = if self.growth.len() == n {
            nearest(self.growth.iter().copied(), growth)?
        } else {
            nearest((0..n).map(|i| i as f64 / (n.max(2) - 1) as f64), growth)?
        };
        let k = self.steps[step].variants.len() as u64;
        (k > 0).then(|| (step, (mix64(stand_id ^ VARIANT_SALT) % k) as usize))
    }

    /// An animal's bin for its size over its breeding size.
    pub fn animal_pick(&self, size: f64) -> Option<usize> {
        let adult = self.steps.last()?.size_m;
        let s = if size.is_finite() {
            size.clamp(0.0, 2.0)
        } else {
            1.0
        };
        nearest(self.steps.iter().map(|b| b.size_m), adult * s.cbrt())
    }
}

/// A model's cells as a mesh, each cell coloured by `colour(cell, band)`.
pub fn model_mesh(cells: &[Cell], colour: impl Fn(&Cell, u8) -> Linear) -> MeshData {
    let top = cells.iter().map(|c| c.offset[1]).max().unwrap_or(0);
    let occupied: std::collections::HashSet<[i32; 3]> = cells.iter().map(|c| c.offset).collect();
    let mut out = MeshData::default();
    for c in cells {
        let rgb = colour(c, band(c.offset[1], top));
        let rgba = [rgb[0], rgb[1], rgb[2], 1.0];
        let [dx, dy, dz] = c.offset;
        for dir in DIRS {
            if dir == Dir::NegY {
                continue;
            }
            let s = dir.step();
            if occupied.contains(&[dx + s[0] as i32, dy + s[1] as i32, dz + s[2] as i32]) {
                continue;
            }
            out.quad_box_face(c.offset, dir, rgba);
        }
    }
    out
}

#[derive(Deserialize)]
struct Manifest {
    voxel_m: f64,
    #[serde(default)]
    species: BTreeMap<String, String>,
    #[serde(default)]
    founders: BTreeMap<String, String>,
    #[serde(default)]
    specimens: BTreeMap<String, String>,
    #[serde(default)]
    detail: BTreeMap<String, serde_json::Value>,
}

fn same(a: &str) -> String {
    a.replace('_', "-")
}

/// **`VoxelModels`**: the baked models and their colours. `load(dir, palette)` once; then
/// `plant_mesh(species, growth, stand_id)` and `animal_mesh(model, size)` return a cached
/// `ArrayMesh` (or null where no model exists: draw a box). Names are the manifest's, `_`
/// and `-` alike: species (`bloomcrown`), lineages (`frondgrazer`, `ripple-snail`) and pose
/// models (`seedporter-glide`, `frondgrazer-hunker`).
#[derive(GodotClass)]
#[class(base = RefCounted)]
pub struct VoxelModels {
    models: BTreeMap<String, Model>,
    plant_colours: HashMap<String, HashMap<(u8, u8, u8), Linear>>,
    animal_colours: HashMap<String, HashMap<(u8, u8), Linear>>,
    cache: HashMap<(String, usize, usize), Gd<ArrayMesh>>,
    voxel_m: f64,
}

#[godot_api]
impl IRefCounted for VoxelModels {
    fn init(_base: Base<RefCounted>) -> Self {
        VoxelModels {
            models: BTreeMap::new(),
            plant_colours: HashMap::new(),
            animal_colours: HashMap::new(),
            cache: HashMap::new(),
            voxel_m: 0.0,
        }
    }
}

#[godot_api]
impl VoxelModels {
    /// Load `dir/manifest.json` and every model it names, coloured from `palette`
    /// (filesystem paths). "" or why not.
    #[func]
    fn load(&mut self, dir: GString, palette: GString) -> GString {
        match self.load_inner(&dir.to_string(), &palette.to_string()) {
            Ok(()) => GString::new(),
            Err(e) => GString::from(e.as_str()),
        }
    }

    /// The voxel size the models were baked at, metres (0 before `load`).
    #[func]
    fn voxel_m(&self) -> f64 {
        self.voxel_m
    }

    /// Whether a model by this name exists.
    #[func]
    fn has_model(&self, name: GString) -> bool {
        self.models.contains_key(&same(&name.to_string()))
    }

    /// Every model's name.
    #[func]
    fn names(&self) -> PackedStringArray {
        self.models
            .keys()
            .map(|k| GString::from(k.as_str()))
            .collect()
    }

    /// A plant's mesh at `growth` (0–1, the feed's), its variant by `stand_id`; null if the
    /// species has no model.
    #[func]
    fn plant_mesh(&mut self, species: GString, growth: f64, stand_id: i64) -> Variant {
        let name = same(&species.to_string());
        let Some(m) = self.models.get(&name).filter(|m| !m.animal) else {
            return Variant::nil();
        };
        let Some((step, variant)) = m.plant_pick(growth, stand_id as u64) else {
            return Variant::nil();
        };
        self.mesh(name, step, variant)
    }

    /// An animal model's mesh for a body of `size` (the feed's size over breeding size);
    /// null if there is no such model.
    #[func]
    fn animal_mesh(&mut self, model: GString, size: f64) -> Variant {
        let name = same(&model.to_string());
        let Some(m) = self.models.get(&name).filter(|m| m.animal) else {
            return Variant::nil();
        };
        let Some(bin) = m.animal_pick(size) else {
            return Variant::nil();
        };
        self.mesh(name, bin, 0)
    }
}

impl VoxelModels {
    fn load_inner(&mut self, dir: &str, palette: &str) -> Result<(), String> {
        let pal = PaletteFile::load(palette)?;
        let path = format!("{dir}/manifest.json");
        let text = std::fs::read_to_string(&path).map_err(|e| format!("reading {path}: {e}"))?;
        let manifest: Manifest =
            serde_json::from_str(&text).map_err(|e| format!("parsing {path}: {e}"))?;
        let mut models = BTreeMap::new();
        let files = manifest
            .species
            .iter()
            .chain(&manifest.founders)
            .chain(&manifest.specimens);
        for (name, file) in files {
            let bytes = std::fs::read(format!("{dir}/{file}"))
                .map_err(|e| format!("reading {file}: {e}"))?;
            let mut m =
                parse_cvm(&bytes, &pal.model_palette).map_err(|e| format!("{file}: {e}"))?;
            if !m.animal {
                m.growth = manifest
                    .detail
                    .get(name)
                    .and_then(|d| d.get("steps"))
                    .and_then(|s| s.as_array())
                    .map(|steps| {
                        steps
                            .iter()
                            .filter_map(|s| s.get("built_at")?.get("t")?.as_f64())
                            .collect()
                    })
                    .unwrap_or_default();
            }
            models.insert(same(name), m);
        }
        let parse_key =
            |k: &str| -> Option<Vec<u8>> { k.split(':').map(|p| p.parse().ok()).collect() };
        self.plant_colours = pal
            .plants
            .iter()
            .map(|(name, table)| {
                let t = table
                    .iter()
                    .filter_map(|(k, hex)| {
                        let k = parse_key(k)?;
                        Some(((*k.first()?, *k.get(1)?, *k.get(2)?), hex_linear(hex)?))
                    })
                    .collect();
                (same(name), t)
            })
            .collect();
        self.animal_colours = pal
            .animals
            .iter()
            .map(|(name, table)| {
                let t = table
                    .iter()
                    .filter_map(|(k, hex)| {
                        let k = parse_key(k)?;
                        Some(((*k.first()?, *k.get(1)?), hex_linear(hex)?))
                    })
                    .collect();
                (same(name), t)
            })
            .collect();
        self.models = models;
        self.cache.clear();
        self.voxel_m = manifest.voxel_m;
        Ok(())
    }

    fn mesh(&mut self, name: String, step: usize, variant: usize) -> Variant {
        let key = (name, step, variant);
        if let Some(m) = self.cache.get(&key) {
            return m.to_variant();
        }
        let Some(model) = self.models.get(&key.0) else {
            return Variant::nil();
        };
        let Some(cells) = model.steps.get(step).and_then(|s| s.variants.get(variant)) else {
            return Variant::nil();
        };
        const MISSING: Linear = [0.25, 0.25, 0.25];
        let data = if model.animal {
            let table = self.animal_colours.get(&key.0);
            model_mesh(cells, |c, band| {
                table
                    .and_then(|t| t.get(&(c.material, band)))
                    .copied()
                    .unwrap_or(MISSING)
            })
        } else {
            let table = self.plant_colours.get(&key.0);
            model_mesh(cells, |c, band| {
                table
                    .and_then(|t| t.get(&(c.tag, c.material, band)))
                    .copied()
                    .unwrap_or(MISSING)
            })
        };
        match array_mesh(&data) {
            Some(mesh) => {
                self.cache.insert(key, mesh.clone());
                mesh.to_variant()
            }
            None => Variant::nil(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> Vec<String> {
        ["plum", "violet", "a_main"].map(String::from).to_vec()
    }

    /// A hand-built file: `kind`, the materials it names, and steps of
    /// `(size, [variant cells])`.
    fn cvm(kind: u8, materials: &[&str], steps: &[(f64, Vec<Vec<[u8; 5]>>)]) -> Vec<u8> {
        let mut b = b"CVM1".to_vec();
        b.push(kind);
        b.push(materials.len() as u8);
        for m in materials {
            b.push(m.len() as u8);
            b.extend(m.as_bytes());
        }
        b.extend((steps.len() as u16).to_le_bytes());
        for (size, variants) in steps {
            b.extend(size.to_le_bytes());
            b.extend(0.5f64.to_le_bytes());
            b.push(variants.len() as u8);
            for cells in variants {
                b.extend((cells.len() as u32).to_le_bytes());
                for c in cells {
                    b.extend(c);
                }
            }
        }
        b
    }

    #[test]
    fn a_file_reads_back_its_cells_with_materials_mapped_to_the_palette() {
        let bytes = cvm(
            0,
            &["violet", "plum"],
            &[(0.25, vec![vec![[0, 0, 0, 0, 0x00], [255, 1, 2, 1, 0x11]]])],
        );
        let m = parse_cvm(&bytes, &palette()).unwrap();
        assert!(!m.animal);
        let cells = &m.steps[0].variants[0];
        assert_eq!(
            cells[0],
            Cell {
                offset: [0, 0, 0],
                material: 1,
                tag: 0x00
            }
        );
        assert_eq!(
            cells[1],
            Cell {
                offset: [-1, 1, 2],
                material: 0,
                tag: 0x11
            }
        );
        // A name outside the palette, a truncated file and trailing bytes are refused.
        assert!(parse_cvm(&cvm(0, &["nope"], &[]), &palette()).is_err());
        assert!(parse_cvm(&bytes[..bytes.len() - 1], &palette()).is_err());
        let mut long = bytes.clone();
        long.push(0);
        assert!(parse_cvm(&long, &palette()).is_err());
    }

    #[test]
    fn a_plant_shows_the_step_nearest_its_growth_and_keeps_its_variant() {
        let step = |s| (s, vec![vec![[0, 0, 0, 0, 0]], vec![[0, 1, 0, 0, 0]]]);
        let mut m = parse_cvm(
            &cvm(0, &["plum"], &[step(0.1), step(0.2), step(0.3)]),
            &palette(),
        )
        .unwrap();
        m.growth = vec![0.1, 0.5, 0.9];
        assert_eq!(m.plant_pick(0.0, 7).unwrap().0, 0);
        assert_eq!(m.plant_pick(0.45, 7).unwrap().0, 1);
        assert_eq!(m.plant_pick(1.0, 7).unwrap().0, 2);
        let v = m.plant_pick(0.1, 99).unwrap().1;
        assert!(
            (0..=1).all(|_| m.plant_pick(0.9, 99).unwrap().1 == v),
            "the variant follows the id alone"
        );
    }

    #[test]
    fn a_bigger_body_never_shows_a_smaller_bin() {
        let bin = |s| (s, vec![vec![[0, 0, 0, 0, 0]]]);
        let m = parse_cvm(
            &cvm(1, &["a_main"], &[bin(0.3), bin(0.5), bin(0.75)]),
            &palette(),
        )
        .unwrap();
        assert!(m.animal);
        let picks: Vec<usize> = (0..=20)
            .map(|i| m.animal_pick(i as f64 / 10.0).unwrap())
            .collect();
        assert!(picks.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(
            m.animal_pick(1.0),
            Some(2),
            "a breeding-size body is the adult"
        );
    }

    #[test]
    fn a_model_mesh_hides_the_faces_between_its_cells() {
        let cells = [
            Cell {
                offset: [0, 0, 0],
                material: 0,
                tag: 0,
            },
            Cell {
                offset: [1, 0, 0],
                material: 0,
                tag: 0,
            },
        ];
        // Two cells side by side: 2 × 5 faces (no bottoms) less the 2 they share.
        assert_eq!(model_mesh(&cells, |_, _| [0.5; 3]).quads(), 8);
    }

    /// Every shipped model reads with the exported palette (the bake and the export agree
    /// on the material names).
    #[test]
    fn the_shipped_models_read_with_the_exported_palette() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let pal = PaletteFile::load(&format!("{root}/game/world/data/palette.json")).unwrap();
        let dir = format!("{root}/assets/voxel-models/125");
        let mut lib = VoxelModels {
            models: BTreeMap::new(),
            plant_colours: HashMap::new(),
            animal_colours: HashMap::new(),
            cache: HashMap::new(),
            voxel_m: 0.0,
        };
        lib.load_inner(&dir, &format!("{root}/game/world/data/palette.json"))
            .unwrap();
        assert!(!lib.models.is_empty());
        for (name, m) in &lib.models {
            if !m.animal {
                assert_eq!(m.growth.len(), m.steps.len(), "{name}: a growth per step");
                assert!(pal.plants.contains_key(name), "{name}: colours");
            } else {
                assert!(lib.animal_colours.contains_key(name), "{name}: colours");
            }
        }
    }
}
