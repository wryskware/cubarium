//! `voxel_export`: baked voxel models to binary glTF (`.glb`), to turn round in the Godot
//! editor, Blender or a three.js viewer.
//!
//! ```text
//! cargo run -p cubarium --release --example voxel_export -- [--lib DIR] [--out DIR]
//!     [--step N] [--variant N|all] [--state N] [--ripe] [--unlit] [--suffix S] [--list]
//!     NAME...
//! ```
//!
//! - `--lib DIR`: a baked library directory holding `manifest.json` (default
//!   `assets/voxel-models/125`); a scratch bake's `runs/.../125` works the same.
//! - `NAME`: a plant species (`vaulttree`), a founder (`loftstrider`, `chorister`) or a
//!   specimen (`lanternjaw-larva`), looked up in that order. Several names are a batch.
//! - `--step N`: the plant's growth step or the animal's size bin, by index (default the
//!   last: the tallest step, the adult bin).
//! - `--variant N|all`: the plant variant (default 0). `all` writes one file per variant.
//! - `--state N`: the plant's foliage state (default 0, the full model).
//! - `--ripe`: accents painted (and glowing) ripe; default unripe, as a stand without a
//!   parcel draws.
//! - `--unlit`: the material is `KHR_materials_unlit` (flat colours); default a plain
//!   matte PBR material, so the faces shade as the model turns.
//! - `--suffix S`: appended to every file name (`vaulttree-A.glb` from `--suffix -A`).
//!
//! Writes `OUT/NAME[-vK][-sN][-stepN][SUFFIX].glb` (`-vK` only for `all` or a non-zero
//! variant). Each cell is a cube of the library's voxel size in metres, y up; only faces
//! with no neighbouring cell are drawn; one mesh per model, its origin at the bottom centre
//! of the model's anchor cell (the cell the renderer stands on the ground). The model's
//! `+x` (an animal's forward) is glTF `+x`; Cubarium's `z` runs into the habitat, so it is
//! glTF `−z` (a rotation-preserving flip, never a mirror).
//!
//! Colours: each cell's **base** colour as the renderer paints it (`colours::plant` /
//! `colours::animal` / `colours::specimen`, the swatch's body at the cell's height band),
//! decoded from sRGB to linear for `COLOR_0` (glTF vertex colours are linear). Emissive
//! cells (`colours::plant_emission`) go in their own primitive whose material's
//! `emissiveFactor` is the glow colour. Not carried: glyph faces and textures, the
//! front-face shadow lean and heart glint stripes, wilt tints, the lighting ladder.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use serde_json::json;

use cubarium::present::srgb_linear;
use cubarium::voxel::colours::{self, Lip};
use cubarium::voxel::model::{self, ModelCell, ModelLibrary, Tag};
use cubarium_voxel_fauna::Founder;
use cubarium_voxel_flora::Species;

fn main() -> Result<()> {
    let mut lib_dir = PathBuf::from("assets/voxel-models/125");
    let mut out_dir = PathBuf::from(".");
    let mut step: Option<usize> = None;
    let mut variant = Variant::One(0);
    let mut state = 0usize;
    let mut ripe = false;
    let mut unlit = false;
    let mut suffix = String::new();
    let mut list = false;
    let mut names = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--lib" => lib_dir = PathBuf::from(args.next().context("--lib DIR")?),
            "--out" => out_dir = PathBuf::from(args.next().context("--out DIR")?),
            "--step" => step = Some(args.next().context("--step N")?.parse()?),
            "--variant" => {
                let v = args.next().context("--variant N|all")?;
                variant = if v == "all" { Variant::All } else { Variant::One(v.parse()?) };
            }
            "--state" => state = args.next().context("--state N")?.parse()?,
            "--ripe" => ripe = true,
            "--unlit" => unlit = true,
            "--suffix" => suffix = args.next().context("--suffix S")?,
            "--list" => list = true,
            s if s.starts_with("--") => bail!("unknown option {s}"),
            _ => names.push(a),
        }
    }
    let (lib, voxel_m) = load_library(&lib_dir)?;
    if list {
        let plants: Vec<_> = Species::ALL.into_iter().filter(|s| lib.plant(*s).is_some()).map(Species::name).collect();
        let founders: Vec<_> = Founder::ALL.into_iter().filter(|f| lib.animal(*f).is_some()).map(Founder::name).collect();
        println!("plants: {}", plants.join(" "));
        println!("founders: {}", founders.join(" "));
        println!("specimens: {}", lib.specimen_names().collect::<Vec<_>>().join(" "));
        if names.is_empty() {
            return Ok(());
        }
    }
    ensure!(!names.is_empty(), "usage: voxel_export [--lib DIR] [--out DIR] [options] NAME...");
    std::fs::create_dir_all(&out_dir).with_context(|| format!("{}", out_dir.display()))?;
    let opts = Paint { ripe };
    for name in &names {
        for (file_tag, cells) in resolve(&lib, name, step, variant, state, opts)? {
            let mesh = build_mesh(&cells, voxel_m as f32);
            let glb = write_glb(&mesh, &format!("{name}{file_tag}{suffix}"), unlit, voxel_m)?;
            let path = out_dir.join(format!("{name}{file_tag}{suffix}.glb"));
            std::fs::write(&path, &glb).with_context(|| format!("{}", path.display()))?;
            println!(
                "{}: {} cells, {} vertices, {} triangles, {} bytes",
                path.display(),
                cells.len(),
                mesh.vertex_count(),
                mesh.triangle_count(),
                glb.len()
            );
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Variant {
    One(usize),
    All,
}

#[derive(Clone, Copy)]
struct Paint {
    ripe: bool,
}

/// A library directory (`.../125`, holding `manifest.json`) and the voxel size it states.
fn load_library(dir: &Path) -> Result<(ModelLibrary, f64)> {
    let manifest = dir.join("manifest.json");
    let text = std::fs::read_to_string(&manifest).with_context(|| format!("reading {}", manifest.display()))?;
    let v: serde_json::Value = serde_json::from_str(&text)?;
    let voxel_m = v["voxel_m"].as_f64().context("manifest has no voxel_m")?;
    let leaf = dir.file_name().and_then(|s| s.to_str()).unwrap_or("");
    ensure!(
        leaf == model::voxel_dir(voxel_m),
        "{} is baked at {voxel_m} m and should sit in a directory named {}",
        dir.display(),
        model::voxel_dir(voxel_m)
    );
    let root = dir.parent().context("the library's parent directory")?;
    Ok((ModelLibrary::load(root, voxel_m)?, voxel_m))
}

/// One coloured cell for the mesher: its offset, its base colour (linear) and its glow
/// (linear), if it emits.
#[derive(Clone, Copy, Debug)]
struct Painted {
    offset: [i32; 3],
    base: [f32; 3],
    glow: Option<[f32; 3]>,
}

/// The models `name` asks for, each with its file-name tag, painted.
fn resolve(
    lib: &ModelLibrary,
    name: &str,
    step: Option<usize>,
    variant: Variant,
    state: usize,
    paint: Paint,
) -> Result<Vec<(String, Vec<Painted>)>> {
    let step_tag = step.map(|s| format!("-step{s}")).unwrap_or_default();
    if let Some(sp) = Species::parse(name) {
        let m = lib.plant(sp).with_context(|| format!("the library has no {name} model"))?;
        let si = step.unwrap_or(m.steps.len().saturating_sub(1));
        let st = m.steps.get(si).with_context(|| format!("{name} has no step {si} (of {})", m.steps.len()))?;
        let vs: Vec<usize> = match variant {
            Variant::One(v) => vec![v],
            Variant::All => (0..st.variants.len()).collect(),
        };
        let mut out = Vec::new();
        for v in vs {
            let cells = st
                .cells(v, state)
                .with_context(|| format!("{name} step {si} has no variant {v} state {state}"))?;
            let vt = match variant {
                Variant::All => format!("-v{v}"),
                Variant::One(0) => String::new(),
                Variant::One(v) => format!("-v{v}"),
            };
            let s_tag = if state > 0 { format!("-s{state}") } else { String::new() };
            out.push((format!("{vt}{s_tag}{step_tag}"), paint_plant(sp, cells, paint)));
        }
        return Ok(out);
    }
    let (model, coat): (_, Box<dyn Fn(u8, u8, bool) -> Option<u32>>) =
        if let Some(f) = Founder::ALL.into_iter().find(|f| f.name() == name) {
            let m = lib.animal(f).with_context(|| format!("the library has no {name} model"))?;
            (m, Box::new(move |mat, band, accent| Some(colours::animal(f, mat, band, accent, 0.0).body)))
        } else if let Some(m) = lib.specimen(name) {
            let n = name.to_string();
            (m, Box::new(move |mat, band, _| colours::specimen(&n, mat, band).map(|s| s.body)))
        } else {
            bail!("{name} is no species, founder or specimen in this library (try --list)");
        };
    let bi = step.unwrap_or(model.bins.len().saturating_sub(1));
    let bin = model.bins.get(bi).with_context(|| format!("{name} has no bin {bi} (of {})", model.bins.len()))?;
    let top = bin.cells.iter().map(|m| m.offset[1]).max().unwrap_or(0);
    let mut cells = Vec::with_capacity(bin.cells.len());
    for m in &bin.cells {
        let band = colours::band(m.offset[1], top);
        let rgb = coat(m.material, band, m.tag == Tag::Accent).with_context(|| format!("{name} has no coat"))?;
        cells.push(Painted { offset: m.offset, base: srgb_linear(rgb), glow: None });
    }
    Ok(vec![(step_tag, cells)])
}

/// A plant's cells in the presenter's paint (`stand.rs`'s `model_style`), at full moisture.
fn paint_plant(sp: Species, cells: &[ModelCell], paint: Paint) -> Vec<Painted> {
    let top = cells.iter().map(|m| m.offset[1]).max().unwrap_or(0);
    cells
        .iter()
        .map(|m| {
            let (tag, band) = match m.tag {
                Tag::Fallen(i) => (Tag::Foliage(i), colours::BANDS - 1),
                t => (t, colours::band(m.offset[1], top)),
            };
            let sw = colours::plant(sp, tag, m.material, band, paint.ripe);
            let lip = if paint.ripe && matches!(tag, Tag::Foliage(_)) && m.material == model::named_index("p2") {
                Lip::Ripe
            } else {
                Lip::None
            };
            let glow = colours::plant_emission(sp, tag, m.material, paint.ripe, lip).map(|g| srgb_linear(g.rgb));
            Painted { offset: m.offset, base: srgb_linear(sw.body), glow }
        })
        .collect()
}

// --- the mesher ----------------------------------------------------------------------

/// One primitive's flat-shaded geometry: four vertices per face, two triangles.
#[derive(Default, Debug)]
struct Prim {
    pos: Vec<[f32; 3]>,
    normal: Vec<[f32; 3]>,
    colour: Vec<[f32; 3]>,
    index: Vec<u32>,
    /// The glow (linear) of an emissive primitive; `None` for the plain one.
    glow: Option<[f32; 3]>,
}

#[derive(Default, Debug)]
struct Mesh {
    prims: Vec<Prim>,
}

impl Mesh {
    fn vertex_count(&self) -> usize {
        self.prims.iter().map(|p| p.pos.len()).sum()
    }
    fn triangle_count(&self) -> usize {
        self.prims.iter().map(|p| p.index.len() / 3).sum()
    }
}

/// The six neighbours, in Cubarium cell axes.
const DIRS: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

/// Every cell a cube of `size` metres; only faces with no neighbouring cell. A cell listed
/// twice is drawn once (the first). Cubarium `(x, y, z)` → glTF `(x, y, −z)`; the origin is
/// the bottom centre of the cell at offset `(0, 0, 0)`.
fn build_mesh(cells: &[Painted], size: f32) -> Mesh {
    let mut seen = HashSet::with_capacity(cells.len());
    let cells: Vec<&Painted> = cells.iter().filter(|c| seen.insert(c.offset)).collect();
    let occupied = seen;
    // One primitive per glow colour (the plain one first), keyed by the colour's bits.
    let mut by_glow: BTreeMap<Option<[u32; 3]>, Prim> = BTreeMap::new();
    for c in cells {
        let key = c.glow.map(|g| g.map(f32::to_bits));
        let prim = by_glow.entry(key).or_insert_with(|| Prim { glow: c.glow, ..Prim::default() });
        let [x, y, z] = c.offset;
        // The cube in glTF axes, before scaling: min corner and unit size.
        let lo = [x as f32 - 0.5, y as f32, -(z as f32) - 0.5];
        for d in DIRS {
            if occupied.contains(&[x + d[0], y + d[1], z + d[2]]) {
                continue;
            }
            let n = [d[0] as f32, d[1] as f32, -(d[2] as f32)];
            let axis = d.iter().position(|&v| v != 0).unwrap_or(0);
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            let mut corners = [lo; 4];
            for (k, (du, dv)) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)].into_iter().enumerate() {
                corners[k][axis] += if n[axis] > 0.0 { 1.0 } else { 0.0 };
                corners[k][u] += du;
                corners[k][v] += dv;
            }
            // Counter-clockwise seen from outside: the winding's normal along `n`.
            let e1 = sub(corners[1], corners[0]);
            let e2 = sub(corners[2], corners[0]);
            if dot(cross(e1, e2), n) < 0.0 {
                corners.swap(1, 3);
            }
            let base = prim.pos.len() as u32;
            for p in corners {
                prim.pos.push(p.map(|v| v * size));
                prim.normal.push(n);
                prim.colour.push(c.base);
            }
            prim.index.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    Mesh { prims: by_glow.into_values().filter(|p| !p.index.is_empty()).collect() }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

// --- the writer ----------------------------------------------------------------------

const GLB_MAGIC: u32 = 0x4654_6C67; // "glTF"
const CHUNK_JSON: u32 = 0x4E4F_534A;
const CHUNK_BIN: u32 = 0x004E_4942;
const FLOAT: u32 = 5126;
const UNSIGNED_INT: u32 = 5125;
const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;

/// The mesh as a binary glTF 2.0 file: one node, one mesh, one primitive per material.
fn write_glb(mesh: &Mesh, name: &str, unlit: bool, voxel_m: f64) -> Result<Vec<u8>> {
    let mut bin: Vec<u8> = Vec::new();
    let mut views = Vec::new();
    let mut accessors = Vec::new();
    let mut materials = Vec::new();
    let mut prims = Vec::new();
    let mut push_view = |bin: &mut Vec<u8>, bytes: Vec<u8>, target: u32| -> usize {
        let offset = bin.len();
        bin.extend_from_slice(&bytes);
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        views.push(json!({"buffer": 0, "byteOffset": offset, "byteLength": bytes.len(), "target": target}));
        views.len() - 1
    };
    let vec3_bytes = |v: &[[f32; 3]]| v.iter().flatten().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>();
    for p in &mesh.prims {
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for q in &p.pos {
            for k in 0..3 {
                lo[k] = lo[k].min(q[k]);
                hi[k] = hi[k].max(q[k]);
            }
        }
        let n = p.pos.len();
        let vp = push_view(&mut bin, vec3_bytes(&p.pos), ARRAY_BUFFER);
        accessors.push(json!({"bufferView": vp, "componentType": FLOAT, "count": n, "type": "VEC3", "min": lo, "max": hi}));
        let a_pos = accessors.len() - 1;
        let vn = push_view(&mut bin, vec3_bytes(&p.normal), ARRAY_BUFFER);
        accessors.push(json!({"bufferView": vn, "componentType": FLOAT, "count": n, "type": "VEC3"}));
        let a_nrm = accessors.len() - 1;
        let vc = push_view(&mut bin, vec3_bytes(&p.colour), ARRAY_BUFFER);
        accessors.push(json!({"bufferView": vc, "componentType": FLOAT, "count": n, "type": "VEC3"}));
        let a_col = accessors.len() - 1;
        let idx: Vec<u8> = p.index.iter().flat_map(|i| i.to_le_bytes()).collect();
        let vi = push_view(&mut bin, idx, ELEMENT_ARRAY_BUFFER);
        accessors.push(json!({"bufferView": vi, "componentType": UNSIGNED_INT, "count": p.index.len(), "type": "SCALAR"}));
        let a_idx = accessors.len() - 1;
        let mut mat = json!({
            "name": if p.glow.is_some() { "voxel-emissive" } else { "voxel" },
            "pbrMetallicRoughness": {"baseColorFactor": [1.0, 1.0, 1.0, 1.0], "metallicFactor": 0.0, "roughnessFactor": 1.0},
        });
        if let Some(g) = p.glow {
            mat["emissiveFactor"] = json!(g);
        }
        if unlit {
            mat["extensions"] = json!({"KHR_materials_unlit": {}});
        }
        materials.push(mat);
        prims.push(json!({
            "attributes": {"POSITION": a_pos, "NORMAL": a_nrm, "COLOR_0": a_col},
            "indices": a_idx,
            "material": materials.len() - 1,
            "mode": 4,
        }));
    }
    let mut doc = json!({
        "asset": {"version": "2.0", "generator": "cubarium voxel_export"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"name": name, "mesh": 0}],
        "meshes": [{"name": name, "primitives": prims}],
        "materials": materials,
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [{"byteLength": bin.len()}],
        "extras": {"voxel_m": voxel_m, "colour_space": "COLOR_0 linear (sRGB-decoded renderer base colours)"},
    });
    if unlit {
        doc["extensionsUsed"] = json!(["KHR_materials_unlit"]);
    }
    let mut js = serde_json::to_vec(&doc)?;
    while js.len() % 4 != 0 {
        js.push(b' ');
    }
    let total = 12 + 8 + js.len() + 8 + bin.len();
    let total = u32::try_from(total).context("a .glb over 4 GiB")?;
    let mut out = Vec::with_capacity(total as usize);
    out.extend(GLB_MAGIC.to_le_bytes());
    out.extend(2u32.to_le_bytes());
    out.extend(total.to_le_bytes());
    out.extend((js.len() as u32).to_le_bytes());
    out.extend(CHUNK_JSON.to_le_bytes());
    out.extend(js);
    out.extend((bin.len() as u32).to_le_bytes());
    out.extend(CHUNK_BIN.to_le_bytes());
    out.extend(bin);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(offset: [i32; 3]) -> Painted {
        Painted { offset, base: [0.5, 0.25, 0.75], glow: None }
    }

    /// Every face's four corners as a sorted, scale-free key.
    fn faces(m: &Mesh) -> Vec<Vec<[i64; 3]>> {
        let mut out = Vec::new();
        for p in &m.prims {
            for q in p.pos.chunks(4) {
                let mut k: Vec<[i64; 3]> = q.iter().map(|v| v.map(|f| (f * 2.0).round() as i64)).collect();
                k.sort();
                out.push(k);
            }
        }
        out
    }

    #[test]
    fn a_lone_cube_has_six_outward_faces() {
        let m = build_mesh(&[cell([0, 0, 0])], 1.0);
        assert_eq!(m.triangle_count(), 12);
        assert_eq!(m.vertex_count(), 24);
        // Each triangle winds counter-clockwise seen from outside: its normal points away
        // from the cube's centre, and agrees with the stored vertex normal.
        let p = &m.prims[0];
        let centre = [0.0, 0.5, 0.0];
        for t in p.index.chunks(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| p.pos[i as usize]);
            let n = cross(sub(b, a), sub(c, a));
            assert!(dot(n, sub(a, centre)) > 0.0, "inward triangle {t:?}");
            assert!(dot(n, p.normal[t[0] as usize]) > 0.0);
        }
    }

    #[test]
    fn two_adjacent_cells_share_no_face() {
        for d in DIRS {
            let m = build_mesh(&[cell([0, 0, 0]), cell(d)], 1.0);
            assert_eq!(m.triangle_count(), 20, "two cells along {d:?}: 10 faces");
            let f = faces(&m);
            let unique: HashSet<_> = f.iter().cloned().collect();
            assert_eq!(unique.len(), f.len(), "no face drawn twice");
        }
        // apart, each keeps all six
        let m = build_mesh(&[cell([0, 0, 0]), cell([2, 0, 0])], 1.0);
        assert_eq!(m.triangle_count(), 24);
    }

    #[test]
    fn origin_is_the_anchor_cells_foot_centre_and_z_turns_toward_the_viewer() {
        let m = build_mesh(&[cell([0, 0, 0]), cell([0, 0, 3])], 0.5);
        let p = &m.prims[0];
        let ys = p.pos.iter().map(|v| v[1]);
        assert_eq!(ys.fold(f32::INFINITY, f32::min), 0.0);
        // the anchor cell spans −0.25..0.25 in x; the cell deeper in the habitat is at −z
        assert!(p.pos.iter().any(|v| v[0] == -0.25) && p.pos.iter().any(|v| v[0] == 0.25));
        let zmin = p.pos.iter().map(|v| v[2]).fold(f32::INFINITY, f32::min);
        assert_eq!(zmin, -1.75);
    }

    #[test]
    fn emissive_cells_get_their_own_primitive() {
        let mut glowing = cell([1, 0, 0]);
        glowing.glow = Some([1.0, 0.5, 0.0]);
        let m = build_mesh(&[cell([0, 0, 0]), glowing], 1.0);
        assert_eq!(m.prims.len(), 2);
        assert_eq!(m.triangle_count(), 20, "emission does not change which faces are hidden");
    }

    #[test]
    fn the_glb_parses_back() {
        let m = build_mesh(&[cell([0, 0, 0]), cell([0, 1, 0])], 0.125);
        let glb = write_glb(&m, "t", true, 0.125).unwrap();
        let u = |o: usize| u32::from_le_bytes(glb[o..o + 4].try_into().unwrap()) as usize;
        assert_eq!(u(0) as u32, GLB_MAGIC);
        assert_eq!(u(4), 2);
        assert_eq!(u(8), glb.len());
        let jl = u(12);
        assert_eq!(u(16) as u32, CHUNK_JSON);
        assert_eq!(jl % 4, 0);
        let doc: serde_json::Value = serde_json::from_slice(&glb[20..20 + jl]).unwrap();
        let bl = u(20 + jl);
        assert_eq!(u(24 + jl) as u32, CHUNK_BIN);
        assert_eq!(28 + jl + bl, glb.len());
        assert_eq!(doc["buffers"][0]["byteLength"].as_u64().unwrap() as usize, bl);
        // every accessor fits its view, and the view the buffer
        for a in doc["accessors"].as_array().unwrap() {
            let v = &doc["bufferViews"][a["bufferView"].as_u64().unwrap() as usize];
            let per = if a["type"] == "VEC3" { 12 } else { 4 };
            assert_eq!(a["count"].as_u64().unwrap() * per, v["byteLength"].as_u64().unwrap());
            assert!(v["byteOffset"].as_u64().unwrap() + v["byteLength"].as_u64().unwrap() <= bl as u64);
        }
        let n = doc["accessors"][0]["count"].as_u64().unwrap() as usize;
        assert_eq!(n, m.vertex_count());
    }
}
