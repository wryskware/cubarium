// The plant pass's fragment stages (`crates/cubarium-gpu/src/plants.rs`), built on the
// walk's own face functions (`voxel.wgsl`): `plant_vis.frag.wgsl` and `plant_shade.frag.wgsl`.
//
// **Visibility** (`plant_vis.frag.wgsl`): each layer sample a plant cell's face covers
// records which cell, which face and which of the stamped face's texels it shows, or
// nothing where the walk would have drawn nothing (a glyph's empty texel, a leaf cutout's
// hole). The depth test keeps the nearest. Nothing is lit here, so a canopy's hidden cells
// cost a few fetches.
//
// **Shade** (`plant_shade.frag.wgsl`), a full-screen pass before the figures: each raster
// pixel takes its layer samples in front of the walk's own end depth, lights every distinct
// texel among them once with `glyphFrontAt` / `glyphCapAt` at the cell's resting place (so a
// cell shows exactly the stamped cell's colours, light and shadow), and lays their mean over
// the raster by coverage. It writes the **occluder** for every pixel: the walk's depth, or
// the nearest plant sample in front of it, which the figures test their texels against as
// they tested them against the walk's depth when the plants were in it.

// #import voxel.wgsl
// #import plant.wgsl

// The plant pass tests its drape cords itself, where the swaying cell puts them
// (`plantCordShows`), so the walk's face functions take every cord texel as shown.
const PLANT_PASS = true;

// Which voxel a cell draws: a leaving cell its carried texel; an arriving or settled one the
// volume's own, while it is still a dry block plant cell there (a cell flooded, or taken by
// an animal, is the walk's: `plantHidden`).
fn plantVoxel(c: vec4<u32>, v: ptr<function, vec4<u32>>) -> bool {
    if (plantDying(c)) {
        *v = plantCarried(c);
        return true;
    }
    let p = plantCellAt(c);
    *v = at(p.x, p.y, p.z);
    return plantHidden(*v);
}

// Whether the walk draws texel (dx, row) of this face of cell (x, y, z) holding v: the
// glyph has a texel there (a front face's tone 63 is empty) and no cutout takes it, as
// `glyphFrontAt` and `glyphCapAt` decide. A cap's row counts from its back edge.
fn plantFaceShows(x: i32, y: i32, z: i32, v: vec4<u32>, cap: bool, row: i32, dx: i32) -> bool {
    let q = cordGlyphTexel(v, dx, select(row, S + row, cap));
    let pigment = textureLoad(styleTex, vec2(i32(q & 3u), i32(v.a)), 0);
    let tone = i32(q >> 2u);
    if (!cap && tone == 63) { return false; }
    if (LIT && (tone == TONE_EMIT || emitsWhole(pigment.a))
        && any(styleEmit(i32(v.a)) > vec3(0.0))) {
        return true;
    }
    var base = pigment.rgb;
    return plantTexel(x, y, z, v, cap, dx, row, &base);
}
