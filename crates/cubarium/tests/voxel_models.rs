//! Package V, authored first and by a separate pass: the voxel organism models the
//! presenter stamps in place of today's glyphs
//! (`design/handoffs/voxel-organism-models-2026-09-23.md` §3 and §4).
//!
//! **Nothing here compiles until the API below exists.** It is the smallest API the
//! brief needs. The implementer matches it, or renames it here and keeps every
//! assertion. Every model is built by hand in the test: small and exact, with no Blender
//! and no baked asset file.
//!
//! # The API these tests assume: `cubarium::voxel::model`
//!
//! ```ignore
//! /// What a baked cell is. The `u8` is the **foliage index**: the stand's
//! /// `layer_stock` index, bottom-up among its foliage-bearing layers
//! /// (`StandLayer::foliage_index`). A drape bears foliage in the model and has its own.
//! #[derive(Clone, Copy, Debug, PartialEq, Eq)]
//! pub enum Tag { Trunk, Foliage(u8), Drape(u8), Accent }
//!
//! /// One baked cell. `offset` is `[dx, dy, dz]` in voxels from the anchor. `material`
//! /// indexes the model palette; these tests carry it through but never read its meaning.
//! #[derive(Clone, Copy, Debug, PartialEq, Eq)]
//! pub struct ModelCell { pub offset: [i32; 3], pub material: u8, pub tag: Tag }
//!
//! /// One crown-height step of a species, and its 2–3 variants.
//! pub struct PlantStep { pub height_m: f64, pub radius_m: f64, pub variants: Vec<Vec<ModelCell>> }
//! /// Steps ascending by height.
//! pub struct PlantModel { pub steps: Vec<PlantStep> }
//! impl PlantModel {
//!     /// The step whose `height_m` is nearest `height_m` (**a tie goes to the taller
//!     /// step**, as `stand::crown_height_voxels` rounds 2.5 up to 3), and in it the
//!     /// variant `stand_id` picks. The variant depends on the id alone. `None` with no steps.
//!     pub fn select(&self, height_m: f64, stand_id: u64) -> Option<&[ModelCell]>;
//! }
//!
//! /// One size bin of a founder, facing +x. Bins ascending by length.
//! pub struct AnimalBin { pub length_m: f64, pub cells: Vec<ModelCell> }
//! pub struct AnimalModel { pub bins: Vec<AnimalBin> }
//! impl AnimalModel {
//!     /// The bin whose `length_m` is nearest the body's length (a tie goes to the
//!     /// longer bin). The presenter passes `founder(f).body_at(animal.body).length_m`.
//!     pub fn select(&self, length_m: f64) -> Option<&[ModelCell]>;
//! }
//!
//! #[derive(Default)]
//! pub struct ModelLibrary { /* per species, per founder */ }
//! impl ModelLibrary {
//!     pub fn insert_plant(&mut self, species: cubarium_voxel_flora::Species, model: PlantModel);
//!     pub fn insert_animal(&mut self, founder: cubarium_voxel_fauna::Founder, model: AnimalModel);
//! }
//!
//! /// Whether a foliage or drape cell survives thinning when its layer holds `fraction`
//! /// (= stock / capacity, clamped to [0, 1]) of its capacity: a pure hash of
//! /// `(stand_id, offset, layer)` compared against the fraction. So it gives the same
//! /// answer every frame, keeps everything at 1 and nothing at 0, and the cells kept at
//! /// a lower fraction are a subset of the cells kept at a higher one.
//! pub fn keeps(stand_id: u64, offset: [i32; 3], layer: u8, fraction: f64) -> bool;
//!
//! /// A plant model at `anchor`, which is the cell just above the stand's support face
//! /// in the stand's own column: `(site.x, site.y + 1, site.z)`. Each cell goes to
//! /// `anchor + offset`. `x` wraps with the ring; the returned `x` may be wrapped or not,
//! /// and these tests compare it modulo the width. A `y` or `z` outside the world is
//! /// dropped and never wrapped. A cell in solid terrain is dropped. A `Foliage(i)` or
//! /// `Drape(i)` cell is kept iff `keeps(stand_id, offset, i, foliage[i])`. Trunk and
//! /// accent cells are never thinned by foliage stock. Each output carries the model
//! /// cell it came from.
//! pub fn stamp_plant(cells: &[ModelCell], anchor: Cell, stand_id: u64, foliage: &[f64],
//!     view: &VoxelView<'_>) -> Vec<(Cell, ModelCell)>;
//!
//! /// An animal model at `anchor = (floor(pose.x / voxel_m), site.y + 1,
//! /// floor(pose.z / voxel_m))`, turned to the nearest of four headings. The model faces
//! /// +x, which is heading π/2 (`Pose::forward` is `(sin h, cos h)`). A turn maps
//! /// `(dx, dz)` to `(−dz, dx)` facing +z, `(dz, −dx)` facing −z and `(−dx, −dz)` facing
//! /// −x: a rotation, never a mirror, with the lateral convention `animal::cells_of`
//! /// uses today. Clipping and burial work as for a plant, and nothing is thinned.
//! pub fn stamp_animal(cells: &[ModelCell], anchor: Cell, heading_rad: f64,
//!     view: &VoxelView<'_>) -> Vec<(Cell, ModelCell)>;
//! ```
//!
//! On the grids: `Stands::rebuild_with(&mut self, view, flora, &ModelLibrary)` and
//! `Animals::rebuild_with(&mut self, view, fauna, &ModelLibrary)` are the model path. For a
//! species or founder the library has no model, each falls back to `parts_of` or
//! `cells_of`. `rebuild` stays the dev-mode glyph path, unchanged. A model cell's part in
//! the grid is the implementer's choice; these tests only ask whether a cell is drawn.

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use cubarium::voxel::animal::{AnimalPart, Animals, cells_of};
use cubarium::voxel::model::{
    AnimalBin, AnimalModel, ModelCell, ModelLibrary, PlantModel, PlantStep, Tag, keeps,
    stamp_animal, stamp_plant,
};
use cubarium::voxel::stand::{Cell, Part, Stands, Style, parts_of, style_of};
use cubarium_voxel::{Command as VoxelCommand, Config, Material, World};
use cubarium_voxel_fauna::{Command as FaunaCommand, Fauna, FaunaConfig, Founder, StartingStores};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Species};

// --- fixtures ----------------------------------------------------------------------

/// A `w × h × d` world with soil in rows `0..=soil_top` and air above.
fn world(w: u32, h: u32, d: u32, soil_top: u32) -> World {
    let mut world = World::empty(Config {
        width: w,
        height: h,
        depth: d,
        ..Config::default()
    });
    for z in 0..d {
        for x in 0..i64::from(w) {
            for y in 0..=soil_top {
                world.apply(VoxelCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    world
}

fn mc(offset: [i32; 3], material: u8, tag: Tag) -> ModelCell {
    ModelCell {
        offset,
        material,
        tag,
    }
}

/// Where an output cell is: `(x mod width, y, z)`.
fn key(c: Cell, width: u32) -> (u32, u32, u32) {
    (c.x.rem_euclid(i64::from(width)) as u32, c.y, c.z)
}

/// The stamp as a map from wrapped position to the model cell drawn there.
fn placed(out: &[(Cell, ModelCell)], width: u32) -> BTreeMap<(u32, u32, u32), ModelCell> {
    out.iter().map(|&(c, m)| (key(c, width), m)).collect()
}

/// A tag as an orderable key: `(kind, layer)`.
fn tag_key(t: Tag) -> (u8, u8) {
    match t {
        Tag::Trunk => (0, 0),
        Tag::Foliage(i) => (1, i),
        Tag::Drape(i) => (2, i),
        Tag::Accent => (3, 0),
    }
}

/// The model offsets a stamp kept, grouped by tag.
fn kept_by_tag(out: &[(Cell, ModelCell)]) -> BTreeMap<(u8, u8), BTreeSet<[i32; 3]>> {
    let mut m: BTreeMap<(u8, u8), BTreeSet<[i32; 3]>> = BTreeMap::new();
    for (_, c) in out {
        m.entry(tag_key(c.tag)).or_default().insert(c.offset);
    }
    m
}

/// A square of offsets `dx, dz ∈ −r..=r` at height `dy`.
fn square(r: i32, dy: i32) -> Vec<[i32; 3]> {
    let mut v = Vec::new();
    for dz in -r..=r {
        for dx in -r..=r {
            v.push([dx, dy, dz]);
        }
    }
    v
}

/// What a grid cell draws, independent of style numbering: the part's shape and its
/// resolved colours. Style indices may be numbered differently by the model path.
fn drawn_plant(s: &Stands, x: i64, y: i64, z: u32) -> Option<(String, Style)> {
    let part = s.at(x, y, z);
    let shape = match part {
        Part::None => return None,
        Part::Trunk(_) => "trunk".to_string(),
        Part::Crown { heart, .. } => format!("crown heart={heart}"),
        Part::Sprout(_) => "sprout".to_string(),
        Part::Log(_) => "log".to_string(),
        Part::Litter(_) => "litter".to_string(),
        Part::Carrion(_) => "carrion".to_string(),
        #[allow(unreachable_patterns)]
        other => format!("{other:?}"),
    };
    Some((shape, s.style(part).expect("a drawn part paints")))
}

fn drawn_animal(a: &Animals, x: i64, y: i64, z: u32) -> Option<(String, Style)> {
    let part = a.at(x, y, z);
    let shape = match part {
        AnimalPart::None => return None,
        AnimalPart::Interim(_) => "interim".to_string(),
        AnimalPart::Body {
            head, facing_right, ..
        } => format!("body head={head} right={facing_right}"),
        AnimalPart::Head { facing_right, .. } => format!("head right={facing_right}"),
        #[allow(unreachable_patterns)]
        other => format!("{other:?}"),
    };
    Some((shape, a.style(part).expect("a drawn part paints")))
}

// --- 1. placement --------------------------------------------------------------------

/// Every model cell lands at `anchor + offset`, once, carrying the model cell it came from.
#[test]
fn a_model_cell_lands_at_its_anchor_plus_its_offset() {
    let w = world(8, 6, 3, 1);
    let view = w.view();
    let model = [
        mc([0, 0, 0], 1, Tag::Trunk),
        mc([0, 1, 0], 1, Tag::Trunk),
        mc([1, 2, -1], 2, Tag::Foliage(0)),
        mc([-1, 2, 1], 2, Tag::Foliage(0)),
        mc([0, 3, 0], 3, Tag::Accent),
    ];
    // The support face is the soil skyline at y = 1, so the anchor is y = 2.
    let anchor = Cell { x: 3, y: 2, z: 1 };
    let out = stamp_plant(&model, anchor, 7, &[1.0], &view);
    assert_eq!(out.len(), model.len(), "one output per model cell: {out:?}");
    let want = BTreeMap::from([
        ((3, 2, 1), model[0]),
        ((3, 3, 1), model[1]),
        ((4, 4, 0), model[2]),
        ((2, 4, 2), model[3]),
        ((3, 5, 1), model[4]),
    ]);
    assert_eq!(placed(&out, 8), want);
}

/// Through the presenter, the anchor is the cell just above the stand's support face in
/// its own column. The step is the one nearest the stand's own crown height
/// (`crown_height_m_at(wood)`). A species with a model draws the model and not the old glyph.
#[test]
fn a_stand_stamps_its_model_on_the_cell_above_its_support_face() {
    let w = world(32, 16, 4, 3);
    let sp = Species::Umbrellafrond;
    let mut flora = Flora::new(FloraConfig::default());
    let wood_max = flora.config().species(sp).wood_max;
    assert!(flora.apply(
        &w,
        Command::Seed {
            x: 10,
            z: 2,
            species: sp,
            wood: wood_max
        }
    ));
    assert!(flora.apply(
        &w,
        Command::Seed {
            x: 24,
            z: 2,
            species: sp,
            wood: 0.5 * wood_max
        }
    ));

    // Steps every 0.25 m up to 2 m. Step k is a trunk column k cells tall plus one
    // marker cell off the axis at (2, k − 1, −1), in material 10 + k.
    const STEP_M: f64 = 0.25;
    let steps: Vec<PlantStep> = (1..=8)
        .map(|k: i32| {
            let mut cells: Vec<ModelCell> = (0..k).map(|j| mc([0, j, 0], 1, Tag::Trunk)).collect();
            cells.push(mc([2, k - 1, -1], 10 + k as u8, Tag::Trunk));
            PlantStep {
                height_m: STEP_M * f64::from(k),
                radius_m: 0.5,
                variants: vec![cells],
            }
        })
        .collect();
    let mut lib = ModelLibrary::default();
    lib.insert_plant(sp, PlantModel { steps });

    let view = w.view();
    let mut stands = Stands::empty(32, 16, 4);
    stands.rebuild_with(&view, flora.view(), &lib);

    let fv = flora.view();
    assert_eq!(fv.stands.len(), 2);
    let mut chosen = Vec::new();
    let mut total = 0;
    for stand in fv.stands {
        assert_eq!(stand.site.y, 3, "the support is the soil skyline");
        let h = fv.config.species(sp).crown_height_m_at(stand.wood);
        let k = ((h / STEP_M).round() as i64).clamp(1, 8);
        chosen.push(k);
        let (x, y0, z) = (
            i64::from(stand.site.x),
            i64::from(stand.site.y) + 1,
            stand.site.z,
        );
        for j in 0..k {
            assert_ne!(
                stands.at(x, y0 + j, z),
                Part::None,
                "stand at x = {x}: trunk offset {j} of step {k} (height {h} m)"
            );
        }
        assert_eq!(
            stands.at(x, y0 + k, z),
            Part::None,
            "step {k} stops at its top"
        );
        assert_ne!(
            stands.at(x + 2, y0 + k - 1, z - 1),
            Part::None,
            "stand at x = {x}: step {k}'s marker at anchor + (2, {}, −1)",
            k - 1
        );
        for other in (1..=8).filter(|&o| o != k) {
            assert_eq!(
                stands.at(x + 2, y0 + other - 1, z - 1),
                Part::None,
                "stand at x = {x}: only the nearest step is drawn, not step {other}"
            );
        }
        total += k as usize + 1;
    }
    assert_eq!(
        chosen[0], 8,
        "a full-grown umbrellafrond is 2 m, the last step"
    );
    assert!(
        chosen[1] < 8,
        "the half-grown stand picks a lower step: {chosen:?}"
    );
    assert_eq!(
        stands.cells().count(),
        total,
        "the model's cells and nothing of the old glyph"
    );
}

// --- 2. wrap and clip ----------------------------------------------------------------

/// `x` wraps across the ring's seam both ways. A cell past the back wall, in front of the
/// front, above the top or below the floor is dropped, never wrapped. Each dropped cell's
/// wrapped position would be air in this world, so a wrap would show up in the output.
#[test]
fn cells_wrap_in_x_and_clip_at_the_back_wall_the_front_and_the_top() {
    let w = world(8, 6, 3, 1);
    let view = w.view();
    let seam = mc([1, 0, 0], 1, Tag::Trunk); // x 8 → 0
    let far = mc([-10, 0, 0], 2, Tag::Trunk); // x −3 → 5
    let back = mc([0, 0, 1], 3, Tag::Trunk); // z 3 = depth: dropped
    let front = mc([0, 0, -3], 4, Tag::Trunk); // z −1: dropped
    let top = mc([0, 4, 0], 5, Tag::Trunk); // y 8 ≥ height: dropped
    let up = mc([0, 1, 0], 6, Tag::Trunk); // y 5, the top row: kept
    let floor = mc([0, -6, 0], 7, Tag::Trunk); // y −2: dropped
    let model = [seam, far, back, front, top, up, floor];
    let anchor = Cell { x: 7, y: 4, z: 2 };
    let out = stamp_plant(&model, anchor, 1, &[], &view);
    let want = BTreeMap::from([((0, 4, 2), seam), ((5, 4, 2), far), ((7, 5, 2), up)]);
    assert_eq!(placed(&out, 8), want);
    assert_eq!(out.len(), 3, "{out:?}");
}

// --- 3. buried cells -----------------------------------------------------------------

/// A cell that falls inside solid terrain (rock, or the soil under the anchor) is not
/// drawn, whatever its tag. The cells beside it still are.
#[test]
fn a_cell_inside_solid_terrain_is_skipped() {
    let mut w = world(8, 6, 3, 1);
    w.apply(VoxelCommand::SetMaterial {
        x: 4,
        y: 3,
        z: 1,
        material: Material::Rock,
    });
    let view = w.view();
    let in_rock = mc([1, 1, 0], 1, Tag::Foliage(0)); // (4, 3, 1): rock
    let in_soil = mc([0, -1, 0], 2, Tag::Trunk); // (3, 1, 1): soil
    let beside = mc([1, 0, 0], 3, Tag::Foliage(0)); // (4, 2, 1): air
    let base = mc([0, 0, 0], 4, Tag::Trunk); // (3, 2, 1): air
    let out = stamp_plant(
        &[in_rock, in_soil, beside, base],
        Cell { x: 3, y: 2, z: 1 },
        3,
        &[1.0],
        &view,
    );
    let want = BTreeMap::from([((4, 2, 1), beside), ((3, 2, 1), base)]);
    assert_eq!(placed(&out, 8), want);
    assert_eq!(out.len(), 2);
}

// --- 4. thinning ---------------------------------------------------------------------

const IDS: [u64; 6] = [0, 1, 2, 17, 1 << 40, u64::MAX];

/// `keeps` is a pure function of stand, cell, layer and stock: asked twice, it answers
/// the same. So the picture does not shimmer between frames.
#[test]
fn thinning_is_a_pure_function_of_stand_cell_layer_and_stock() {
    let cells = square(10, 3);
    for id in IDS {
        for layer in 0..3u8 {
            for f in [0.1, 0.25, 0.5, 0.9] {
                for &o in &cells {
                    assert_eq!(
                        keeps(id, o, layer, f),
                        keeps(id, o, layer, f),
                        "id {id} layer {layer} f {f} offset {o:?}"
                    );
                }
            }
        }
    }
}

/// A layer at stock fraction `f` shows about `f` of its cells. Of 441 cells, the count
/// stays within four binomial standard deviations of `f · 441` (±42 at one half). Full
/// shows every cell and empty shows none, including when the stock is a little over
/// capacity or below zero. Regrowth only adds cells: the cells shown at a lower stock are
/// a subset of the cells shown at a higher one, so a filling layer never swaps cells.
#[test]
fn a_layer_shows_its_stock_fraction_of_its_cells_and_the_ends_are_exact() {
    let cells = square(10, 3);
    let n = cells.len() as f64;
    for id in IDS {
        for layer in 0..3u8 {
            let kept = |f: f64| -> BTreeSet<[i32; 3]> {
                cells
                    .iter()
                    .copied()
                    .filter(|&o| keeps(id, o, layer, f))
                    .collect()
            };
            assert_eq!(
                kept(0.0).len(),
                0,
                "empty shows nothing (id {id}, layer {layer})"
            );
            assert_eq!(kept(-0.5).len(), 0, "below empty shows nothing");
            assert_eq!(kept(1.0).len(), cells.len(), "full shows every cell");
            assert_eq!(kept(1.25).len(), cells.len(), "over full shows every cell");
            for f in [0.25, 0.5, 0.75] {
                let got = kept(f).len() as f64;
                let tol = 4.0 * (n * f * (1.0 - f)).sqrt();
                assert!(
                    (got - f * n).abs() <= tol,
                    "id {id} layer {layer}: {got} of {n} kept at {f}, want {} ± {tol:.0}",
                    f * n
                );
            }
            let (a, b, c) = (kept(0.3), kept(0.6), kept(0.9));
            assert!(
                a.is_subset(&b) && b.is_subset(&c),
                "regrowth only adds cells"
            );
        }
    }
}

/// Through the stamp, each foliage or drape layer is thinned by its own stock alone. The
/// kept cells are exactly the ones `keeps` names. Trunk and accent cells are never
/// thinned by foliage, even when every layer is empty. The same stand gives the same
/// kept set every frame, and a different stand thins differently.
#[test]
fn a_layer_thins_by_its_own_stock_and_trunk_and_accents_never_do() {
    let w = world(32, 12, 16, 0);
    let view = w.view();
    let anchor = Cell { x: 16, y: 1, z: 8 };
    let trunk: Vec<[i32; 3]> = (0..3).map(|j| [0, j, 0]).collect();
    let l0 = square(6, 3); // 169 cells
    let drape = square(6, 4); // 169 cells, foliage index 2
    let l1 = square(6, 6); // 169 cells
    let accent = [0, 7, 0];
    let mut model = Vec::new();
    model.extend(trunk.iter().map(|&o| mc(o, 1, Tag::Trunk)));
    model.extend(l0.iter().map(|&o| mc(o, 2, Tag::Foliage(0))));
    model.extend(drape.iter().map(|&o| mc(o, 3, Tag::Drape(2))));
    model.extend(l1.iter().map(|&o| mc(o, 4, Tag::Foliage(1))));
    model.push(mc(accent, 5, Tag::Accent));

    let set = |v: &[[i32; 3]]| -> BTreeSet<[i32; 3]> { v.iter().copied().collect() };
    let expect = |v: &[[i32; 3]], id: u64, layer: u8, f: f64| -> BTreeSet<[i32; 3]> {
        v.iter()
            .copied()
            .filter(|&o| keeps(id, o, layer, f))
            .collect()
    };
    let id = 41;
    let kept = |foliage: &[f64]| kept_by_tag(&stamp_plant(&model, anchor, id, foliage, &view));
    let get = |m: &BTreeMap<(u8, u8), BTreeSet<[i32; 3]>>, k: (u8, u8)| {
        m.get(&k).cloned().unwrap_or_default()
    };

    // The rosette half browsed, the crown and the drape full.
    let a = kept(&[0.5, 1.0, 1.0]);
    let half = expect(&l0, id, 0, 0.5);
    assert_eq!(
        get(&a, (1, 0)),
        half,
        "layer 0 keeps exactly what `keeps` names"
    );
    let tol = 4.0 * (169.0f64 * 0.25).sqrt();
    assert!(
        (half.len() as f64 - 84.5).abs() <= tol,
        "about half of 169: {}",
        half.len()
    );
    assert_eq!(
        get(&a, (1, 1)),
        set(&l1),
        "layer 1 is untouched by layer 0's stock"
    );
    assert_eq!(get(&a, (2, 2)), set(&drape), "the drape is untouched too");
    assert_eq!(get(&a, (0, 0)), set(&trunk), "the trunk is never thinned");
    assert_eq!(get(&a, (3, 0)), BTreeSet::from([accent]), "nor the accent");

    // The crown eaten, the drape half, the rosette full.
    let b = kept(&[1.0, 0.0, 0.5]);
    assert_eq!(get(&b, (1, 0)), set(&l0));
    assert_eq!(
        get(&b, (1, 1)),
        BTreeSet::new(),
        "an empty layer shows nothing"
    );
    assert_eq!(
        get(&b, (2, 2)),
        expect(&drape, id, 2, 0.5),
        "the drape by its own stock"
    );
    assert_eq!(get(&b, (0, 0)), set(&trunk));
    assert_eq!(get(&b, (3, 0)), BTreeSet::from([accent]));

    // Everything eaten: wood and accent stand.
    let c = kept(&[0.0, 0.0, 0.0]);
    assert_eq!(
        get(&c, (1, 0)).len() + get(&c, (1, 1)).len() + get(&c, (2, 2)).len(),
        0
    );
    assert_eq!(get(&c, (0, 0)), set(&trunk));
    assert_eq!(get(&c, (3, 0)), BTreeSet::from([accent]));

    // Frame after frame, the same half.
    for _ in 0..3 {
        assert_eq!(
            kept(&[0.5, 1.0, 1.0]),
            a,
            "the same stand thins the same way every frame"
        );
    }
    // A meadow of half-browsed stands does not thin in lockstep.
    let other = kept_by_tag(&stamp_plant(
        &model,
        anchor,
        id + 1,
        &[0.5, 1.0, 1.0],
        &view,
    ));
    assert_ne!(get(&other, (1, 0)), half, "the stand id feeds the hash");
}

/// Through the presenter, the stock is the model's own. A bite that empties an adult
/// bloomcrown's lowest layer empties that layer's cells. The crown above keeps every
/// cell, and so do the trunk and the accent.
#[test]
fn a_browsed_layer_empties_on_the_model_while_the_crown_stays_full() {
    let w = world(32, 16, 8, 3);
    let sp = Species::Bloomcrown;
    let mut flora = Flora::new(FloraConfig::default());
    let wood_max = flora.config().species(sp).wood_max;
    let height_m = flora.config().species(sp).crown_height_m_at(wood_max);
    assert!(flora.apply(
        &w,
        Command::Seed {
            x: 10,
            z: 4,
            species: sp,
            wood: wood_max
        }
    ));
    let (site, low_cell) = {
        let fv = flora.view();
        let stand = &fv.stands[0];
        let foliage: Vec<_> = fv
            .layers(stand)
            .filter(|l| l.foliage_index.is_some())
            .collect();
        assert!(
            foliage.len() >= 2,
            "an adult bloomcrown has a rosette and a crown"
        );
        assert_ne!(foliage[0].cell, foliage[1].cell, "at different heights");
        assert!(
            foliage.iter().all(|l| l.stock >= l.capacity * 0.999),
            "a seeded stand starts full"
        );
        (stand.site, foliage[0].cell)
    };

    let trunk: Vec<[i32; 3]> = (0..4).map(|j| [0, j, 0]).collect();
    let ring: Vec<[i32; 3]> = square(1, 1)
        .into_iter()
        .filter(|o| o[0] != 0 || o[2] != 0)
        .collect();
    let crown = square(1, 5);
    let accent = [0, 6, 0];
    let mut cells = Vec::new();
    cells.extend(trunk.iter().map(|&o| mc(o, 1, Tag::Trunk)));
    cells.extend(ring.iter().map(|&o| mc(o, 2, Tag::Foliage(0))));
    cells.extend(crown.iter().map(|&o| mc(o, 3, Tag::Foliage(1))));
    cells.push(mc(accent, 4, Tag::Accent));
    let mut lib = ModelLibrary::default();
    lib.insert_plant(
        sp,
        PlantModel {
            steps: vec![PlantStep {
                height_m,
                radius_m: 0.25,
                variants: vec![cells],
            }],
        },
    );

    let (x, y0, z) = (i64::from(site.x), i64::from(site.y) + 1, site.z);
    let drawn = |stands: &Stands, v: &[[i32; 3]]| {
        v.iter()
            .filter(|o| {
                stands.at(
                    x + i64::from(o[0]),
                    y0 + i64::from(o[1]),
                    (z as i64 + i64::from(o[2])) as u32,
                ) != Part::None
            })
            .count()
    };
    let mut stands = Stands::empty(32, 16, 8);
    stands.rebuild_with(&w.view(), flora.view(), &lib);
    assert_eq!(
        drawn(&stands, &ring),
        ring.len(),
        "a full rosette shows every cell"
    );
    assert_eq!(drawn(&stands, &crown), crown.len());

    assert!(
        flora
            .take_foliage_in_layers(site, 1.0e9, &(low_cell..=low_cell))
            .is_some(),
        "the bite takes from the rosette"
    );
    stands.rebuild_with(&w.view(), flora.view(), &lib);
    assert_eq!(drawn(&stands, &ring), 0, "the browsed rosette is empty");
    assert_eq!(drawn(&stands, &crown), crown.len(), "the crown stays full");
    assert_eq!(drawn(&stands, &trunk), trunk.len());
    assert_eq!(drawn(&stands, &[accent]), 1);
}

// --- 5. nearest step and variant -----------------------------------------------------

/// Four steps a voxel apart, three variants each. Cell material `10 · step + variant`
/// says which one `select` returned.
fn stepped() -> PlantModel {
    PlantModel {
        steps: (0..4u8)
            .map(|s| PlantStep {
                height_m: 0.125 * f64::from(s + 1),
                radius_m: 0.1,
                variants: (0..3u8)
                    .map(|v| vec![mc([0, 0, 0], 10 * s + v, Tag::Trunk)])
                    .collect(),
            })
            .collect(),
    }
}

/// The step whose height is nearest the stand's is chosen, clamped at both ends. An exact
/// tie between two steps goes to the **taller** one, as `crown_height_voxels` rounds
/// 2.5 up to 3. The tie heights here (0.1875, 0.3125, 0.4375) are exact in binary, so
/// both distances are exactly equal.
#[test]
fn the_nearest_height_step_is_chosen_and_a_tie_goes_to_the_taller_step() {
    let model = stepped();
    let step_of = |h: f64| model.select(h, 5).expect("a model with steps")[0].material / 10;
    for (h, want) in [
        (0.0, 0),
        (0.1, 0),
        (0.125, 0),
        (0.25, 1),
        (0.30, 1),
        (0.34, 2),
        (0.375, 2),
        (0.5, 3),
        (9.0, 3),
        // Exact ties.
        (0.1875, 1),
        (0.3125, 2),
        (0.4375, 3),
    ] {
        assert_eq!(step_of(h), want, "height {h} m");
    }
    assert!(
        PlantModel { steps: Vec::new() }.select(0.3, 5).is_none(),
        "no steps, no model: the presenter falls back"
    );
}

/// The variant comes from the stand id: the same id gets the same variant every time
/// and at every step (so a growing stand keeps its phase). Across a meadow of ids, every
/// variant appears.
#[test]
fn the_variant_comes_from_the_stand_id_and_holds_as_the_stand_grows() {
    let model = stepped();
    let pick = |h: f64, id: u64| model.select(h, id).unwrap()[0].material;
    let mut seen = BTreeSet::new();
    for id in 0..300u64 {
        let v = pick(0.125, id) % 10;
        assert!(v < 3);
        assert_eq!(pick(0.125, id), pick(0.125, id), "stable for id {id}");
        for (s, h) in [(1u8, 0.25), (2, 0.375), (3, 0.5)] {
            assert_eq!(
                pick(h, id),
                10 * s + v,
                "id {id} keeps variant {v} at step {s}"
            );
        }
        seen.insert(v);
    }
    assert_eq!(seen.len(), 3, "a meadow is not copies: {seen:?}");
}

// --- 6. fallback ---------------------------------------------------------------------

/// A species the library has no model for draws exactly what `parts_of` draws today,
/// cell for cell and colour for colour. The dev-mode `rebuild` is that path, so the
/// model path with only other species' models must match it.
#[test]
fn a_species_with_no_model_draws_exactly_what_parts_of_draws() {
    let w = world(32, 16, 4, 3);
    let mut flora = Flora::new(FloraConfig::default());
    for (x, z, sp) in [(6, 1, Species::Bloomcrown), (20, 2, Species::Springturf)] {
        let wood = flora.config().species(sp).wood_max;
        assert!(flora.apply(
            &w,
            Command::Seed {
                x,
                z,
                species: sp,
                wood
            }
        ));
    }
    // A model for a species that is not in this flora.
    let mut lib = ModelLibrary::default();
    lib.insert_plant(
        Species::Umbrellafrond,
        PlantModel {
            steps: vec![PlantStep {
                height_m: 2.0,
                radius_m: 0.75,
                variants: vec![vec![mc([0, 0, 0], 1, Tag::Trunk)]],
            }],
        },
    );

    let view = w.view();
    let fv = flora.view();
    let mut old = Stands::empty(32, 16, 4);
    old.rebuild(&view, fv);
    let mut new = Stands::empty(32, 16, 4);
    new.rebuild_with(&view, fv, &lib);

    let old_cells: BTreeSet<_> = old.cells().collect();
    let new_cells: BTreeSet<_> = new.cells().collect();
    assert!(!old_cells.is_empty());
    assert_eq!(new_cells, old_cells, "the same cells");
    for &(x, y, z) in &old_cells {
        let (x, y) = (i64::from(x), i64::from(y));
        assert_eq!(
            drawn_plant(&new, x, y, z),
            drawn_plant(&old, x, y, z),
            "the same part and colours at ({x}, {y}, {z})"
        );
    }
    // And that is `parts_of`, cell for cell, for every stand, in the stand's own colours.
    for stand in fv.stands {
        for (c, part) in parts_of(fv, stand, 0) {
            if c.y >= 16 || c.z >= 4 || view.material_at(c.x, c.y, c.z).is_solid() {
                continue;
            }
            let got = new.at(c.x, i64::from(c.y), c.z);
            assert_ne!(got, Part::None, "{:?}'s {part:?} at {c:?}", stand.species);
            assert_eq!(
                new.style(got),
                Some(style_of(fv, stand)),
                "{:?} at {c:?} paints in its own style",
                stand.species
            );
        }
    }
}

// --- 7. animals ----------------------------------------------------------------------

/// Five bins from newborn to adult, evenly spaced in length (length ∝ body^(1/3)), cell
/// material = bin index, each a single cell at `(0, bin, 0)`. That offset does not change
/// under a turn, so a stamped cell's height says which bin was drawn.
fn browser_bins(fauna: &FaunaConfig) -> AnimalModel {
    let phys = fauna.founder(Founder::Browser);
    let newborn = phys.body_at(phys.core.body_min).length_m;
    let adult = phys.body_at(phys.core.body_max).length_m;
    AnimalModel {
        bins: (0..5u8)
            .map(|i| AnimalBin {
                length_m: newborn + (adult - newborn) * f64::from(i) / 4.0,
                cells: vec![mc([0, i32::from(i), 0], i, Tag::Trunk)],
            })
            .collect(),
    }
}

/// The size bin is the one whose length is nearest the body's, and the body's length
/// is `body_at(body)`, the model's own geometry. A newborn draws bin 0 and an adult bin 4.
/// A body whose length is bin 2's draws bin 2. Growing never goes down a bin.
#[test]
fn the_size_bin_is_the_one_nearest_the_body_length() {
    let fauna = FaunaConfig::default();
    let model = browser_bins(&fauna);
    let phys = fauna.founder(Founder::Browser);
    let (min, max) = (phys.core.body_min, phys.core.body_max);
    let bin = |body: f64| model.select(phys.body_at(body).length_m).unwrap()[0].material;
    assert_eq!(bin(min), 0, "a newborn");
    assert_eq!(bin(max), 4, "an adult");
    let mid_len = model.bins[2].length_m;
    let mid_body = max * (mid_len / phys.body_at(max).length_m).powi(3);
    assert_eq!(bin(mid_body), 2, "the body whose length is bin 2's");
    let mut last = 0;
    for k in 0..=100 {
        let b = bin(min + (max - min) * f64::from(k) / 100.0);
        assert!(b >= last, "growing never shrinks the drawing");
        last = b;
    }
    assert_eq!(
        model.select(0.0).unwrap()[0].material,
        0,
        "below the newborn"
    );
    assert_eq!(
        model.select(10.0).unwrap()[0].material,
        4,
        "beyond the adult"
    );
    assert!(AnimalModel { bins: Vec::new() }.select(0.3).is_none());
}

/// The model faces +x (heading π/2). The heading rounds to the nearest quarter turn
/// (headings wrap by 2π), and the turn rotates the horizontal offsets: `(dx, dz)` →
/// `(−dz, dx)` facing +z, `(dz, −dx)` facing −z, `(−dx, −dz)` facing −x. An offset
/// straight up is unchanged.
#[test]
fn the_heading_rounds_to_the_nearest_quarter_turn_and_turns_the_offsets() {
    let w = world(32, 8, 16, 1);
    let view = w.view();
    let p = mc([2, 0, 1], 1, Tag::Trunk);
    let q = mc([0, 1, 0], 2, Tag::Trunk);
    let r = mc([-1, 0, 0], 3, Tag::Trunk);
    let model = [p, q, r];
    let anchor = Cell { x: 8, y: 2, z: 8 };
    let by_material = |h: f64| -> BTreeMap<u8, (u32, u32, u32)> {
        stamp_animal(&model, anchor, h, &view)
            .into_iter()
            .map(|(c, m)| (m.material, key(c, 32)))
            .collect()
    };
    let facing_px = BTreeMap::from([(1, (10, 2, 9)), (2, (8, 3, 8)), (3, (7, 2, 8))]);
    let facing_pz = BTreeMap::from([(1, (7, 2, 10)), (2, (8, 3, 8)), (3, (8, 2, 7))]);
    let facing_nz = BTreeMap::from([(1, (9, 2, 6)), (2, (8, 3, 8)), (3, (8, 2, 9))]);
    let facing_nx = BTreeMap::from([(1, (6, 2, 7)), (2, (8, 3, 8)), (3, (9, 2, 8))]);
    let tau = 2.0 * PI;
    for (name, want, headings) in [
        (
            "+x",
            &facing_px,
            vec![
                FRAC_PI_2,
                FRAC_PI_2 + 0.6,
                FRAC_PI_2 - 0.6,
                FRAC_PI_2 + tau,
                FRAC_PI_4 + 0.05,
            ],
        ),
        ("+z", &facing_pz, vec![0.0, 0.7, -0.7, tau, tau - 0.3]),
        ("-z", &facing_nz, vec![PI, PI + 0.7, PI - 0.7, -PI]),
        (
            "-x",
            &facing_nx,
            vec![
                -FRAC_PI_2,
                3.0 * FRAC_PI_2,
                3.0 * FRAC_PI_2 - 0.7,
                -FRAC_PI_2 + 0.7,
            ],
        ),
    ] {
        for h in headings {
            assert_eq!(&by_material(h), want, "heading {h:.3} rad faces {name}");
        }
    }
}

/// A turned animal clips and wraps like a plant. At the back-right corner facing +x, a
/// cell wraps over the seam and one past the back wall is dropped. At the front facing
/// −z, a cell turned in front of the front is dropped. A cell in the soil is dropped.
#[test]
fn a_turned_animal_wraps_clips_and_skips_buried_cells() {
    let w = world(32, 8, 16, 1);
    let view = w.view();
    let p = mc([2, 0, 1], 1, Tag::Trunk);
    let q = mc([0, 1, 0], 2, Tag::Trunk);
    let r = mc([-1, 0, 0], 3, Tag::Trunk);
    let s = mc([0, -1, 0], 4, Tag::Trunk); // into the soil
    let t = mc([1, 0, 0], 5, Tag::Trunk);
    let model = [p, q, r, s, t];
    let at = |anchor: Cell, h: f64| -> BTreeMap<u8, (u32, u32, u32)> {
        stamp_animal(&model, anchor, h, &view)
            .into_iter()
            .map(|(c, m)| (m.material, key(c, 32)))
            .collect()
    };
    // Back-right corner, facing +x: p reaches z = 16 (dropped); t crosses the seam.
    assert_eq!(
        at(Cell { x: 31, y: 2, z: 15 }, FRAC_PI_2),
        BTreeMap::from([(2, (31, 3, 15)), (3, (30, 2, 15)), (5, (0, 2, 15))])
    );
    // Front row, facing −z: p turns to (1, −2) and is dropped; r turns to (0, +1); t to (0, −1), dropped.
    assert_eq!(
        at(Cell { x: 0, y: 2, z: 0 }, PI),
        BTreeMap::from([(2, (0, 3, 0)), (3, (0, 2, 1))])
    );
}

/// Through the presenter, a browser draws its founder's model at the bin nearest its
/// body's length, at `(floor(pose.x / voxel_m), site.y + 1, floor(pose.z / voxel_m))`.
/// A founder with no model (the littershredder here) draws exactly `cells_of`.
#[test]
fn animals_stamp_their_founder_model_by_size_bin_and_fall_back_to_cells_of() {
    let voxel_m = 0.125;
    let c = Config {
        voxel_m,
        ..Config::default()
    };
    let mut w = World::empty(c.clone());
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            w.apply(VoxelCommand::SetMaterial {
                x,
                y: 0,
                z,
                material: Material::Soil,
            });
        }
    }
    let mut fauna = Fauna::new(FaunaConfig::default());
    for (x, z, founder, stores) in [
        (6, 4, Founder::Browser, StartingStores::FULL),
        (40, 10, Founder::Browser, StartingStores::HUNGRY),
        (80, 12, Founder::Blind, StartingStores::FULL),
    ] {
        assert!(fauna.apply(
            &w,
            FaunaCommand::IntroduceFounder {
                x,
                z,
                founder,
                stores,
                heading_rad: FRAC_PI_2,
            }
        ));
    }
    let mut lib = ModelLibrary::default();
    lib.insert_animal(Founder::Browser, browser_bins(fauna.config()));
    let model = browser_bins(fauna.config()); // the same bins, to read their lengths

    let view = w.view();
    let av = fauna.view();
    let mut old = Animals::empty(0, 0, 0);
    old.rebuild(&view, Some(av));
    let mut new = Animals::empty(0, 0, 0);
    new.rebuild_with(&view, Some(av), &lib);

    let mut expected_cells = 0;
    let mut bins = Vec::new();
    for animal in av.animals {
        let founder = animal.founder.expect("founders");
        match founder {
            Founder::Browser => {
                let length = av.config.founder(founder).body_at(animal.body).length_m;
                let bin = (0..5)
                    .min_by(|&a, &b| {
                        let da = (model.bins[a].length_m - length).abs();
                        let db = (model.bins[b].length_m - length).abs();
                        da.partial_cmp(&db).unwrap().then(b.cmp(&a))
                    })
                    .unwrap() as i64;
                bins.push(bin);
                let ax = (animal.pose.x / voxel_m).floor() as i64;
                let az = (animal.pose.z / voxel_m).floor() as u32;
                let y0 = i64::from(animal.site.y) + 1;
                for j in 0..5 {
                    let got = new.at(ax, y0 + j, az);
                    if j == bin {
                        assert_ne!(
                            got,
                            AnimalPart::None,
                            "bin {bin} drawn at ({ax}, {}, {az})",
                            y0 + j
                        );
                    } else {
                        assert_eq!(got, AnimalPart::None, "only bin {bin}, not {j}");
                    }
                }
                expected_cells += 1;
            }
            Founder::Blind => {
                let mut seen = BTreeSet::new();
                for (cell, _) in cells_of(animal, av.config, 0, voxel_m) {
                    if cell.y >= c.height || cell.z >= c.depth {
                        continue;
                    }
                    let (x, y) = (cell.x, i64::from(cell.y));
                    assert!(
                        drawn_animal(&new, x, y, cell.z).is_some(),
                        "cells_of's {cell:?}"
                    );
                    assert_eq!(
                        drawn_animal(&new, x, y, cell.z),
                        drawn_animal(&old, x, y, cell.z),
                        "the fallback draws what the glyph path draws at {cell:?}"
                    );
                    seen.insert(key(cell, c.width));
                }
                expected_cells += seen.len();
            }
        }
    }
    assert_eq!(bins[0], 4, "a full-grown browser draws the adult bin");
    assert!(bins[1] < 4, "a hungry browser is smaller: {bins:?}");
    assert_eq!(new.cells().count(), expected_cells, "nothing else is drawn");
}
