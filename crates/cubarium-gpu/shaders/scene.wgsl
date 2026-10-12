// Shared by the two full-screen field passes and the sprite pass: the uniform
// block `crate::palette::SceneUniforms` fills, and the field sampling rule.
//
// `fieldA` is (producer, water, detritus, rain) and `fieldB` is (growth, tall, -, -),
// both `cells_x x cells_y` R16G16B16A16_SFLOAT read texel by texel, wrapping in x (the
// ring wraps) and clamped in y (the rims do not).

struct Scene {
    raster: vec4<f32>,        // w, h, 1/w, 1/h
    grid: vec4<f32>,          // cells_x, cells_y, S, producer_max
    time: vec4<f32>,          // seconds, f, bilinear filter, art scale
    floorColour: vec4<f32>,   // pre-scaled by FLOOR_BRIGHTNESS
    producerLow: vec4<f32>,   // rgb, RAMP_MIN_BRIGHTNESS
    producerHigh: vec4<f32>,  // rgb, RAMP_MAX_BRIGHTNESS
    detritus: vec4<f32>,      // rgb, DETRITUS_SCALE
    soilLow: vec4<f32>,       // rgb, SOIL_MIN_BRIGHTNESS
    soilHigh: vec4<f32>,      // rgb, SOIL_MAX_BRIGHTNESS
    waterLow: vec4<f32>,      // rgb, WATER_FILM
    waterHigh: vec4<f32>,     // rgb, WATER_BRIGHT
    algae: vec4<f32>,         // rgb, ALGAE_TINT
    bands: vec4<f32>,         // SOIL_TOP, HORIZON, DETRITUS_THRESHOLD, SOIL_SCALE
    knobs: vec4<f32>,         // PRODUCER_SATURATION, WATER_SHIMMER, WATER_SHIMMER_SECONDS, -
}

@group(0) @binding(0) var<uniform> u: Scene;
@group(0) @binding(1) var fieldA: texture_2d<f32>;
@group(0) @binding(2) var fieldB: texture_2d<f32>;

// The cell a raster pixel belongs to. On the ring this is an integer division —
// no chart, no seam-aware pixel->cell table (presenter-budget W1 is simply absent).
fn cellOf(px: vec2<f32>) -> vec2<i32> {
    let cell = 4.0 * u.grid.z;
    return vec2<i32>(floor(px / cell));
}

// One field channel at a raster pixel, without filtering: the pixel's own cell.
fn cellValue(field: texture_2d<f32>, channel: i32, cell: vec2<i32>) -> f32 {
    let nx = i32(u.grid.x);
    let ny = i32(u.grid.y);
    // Wrap without a `%` of a negative.
    let cx = select(nx - 1 - (-1 - cell.x) % nx, cell.x % nx, cell.x >= 0);
    let c = vec2<i32>(cx, clamp(cell.y, 0, ny - 1));
    return textureLoad(field, c, 0)[channel];
}

// `cubarium_render::draw_field`'s seam-aware one-pixel box filter, transcribed:
// the pixel's own cell at weight 4, each *existing* pixel neighbour's cell at 1,
// normalised over what exists so the rims do not darken. The ring wraps in x, so
// the left and right neighbours always exist; the top and bottom rows lose one.
fn filteredAt(field: texture_2d<f32>, channel: i32, px: vec2<f32>) -> f32 {
    var sum = cellValue(field, channel, cellOf(px)) * 4.0;
    var weight = 4.0;
    sum += cellValue(field, channel, cellOf(px + vec2(1.0, 0.0)));
    sum += cellValue(field, channel, cellOf(px - vec2(1.0, 0.0)));
    weight += 2.0;
    if (px.y + 1.0 < u.raster.y) { sum += cellValue(field, channel, cellOf(px + vec2(0.0, 1.0))); weight += 1.0; }
    if (px.y - 1.0 >= 0.0)       { sum += cellValue(field, channel, cellOf(px - vec2(0.0, 1.0))); weight += 1.0; }
    return sum / weight;
}

// `Topology::height` on the ring: +1 at the top row, -1 at the bottom.
fn heightAt(y: f32) -> f32 {
    return 1.0 - 2.0 * (y / u.raster.y);
}

// `art_present::habitat::soil_weight`: how much of a pixel is soil.
// 1 - smoothstep(SOIL_TOP - HORIZON, SOIL_TOP + HORIZON, h).
fn soilWeight(y: f32) -> f32 {
    let h = heightAt(y);
    return 1.0 - smoothstep(u.bands.x - u.bands.y, u.bands.x + u.bands.y, h);
}

// A stable per-pixel phase in [0, 2pi). The CPU hashes with SplitMix64; any stable
// hash gives the same *kind* of fixed glint pattern, which is all this value is.
fn hashPhase(p: vec2<i32>) -> f32 {
    var h = (u32(p.x) * 0x9E3779B9u) ^ (u32(p.y) * 0x85EBCA6Bu);
    h ^= h >> 16u; h *= 0x7FEB352Du; h ^= h >> 15u; h *= 0x846CA68Bu; h ^= h >> 16u;
    return f32(h & 0xFFFFFFu) / f32(0x1000000u) * 6.28318530718;
}
