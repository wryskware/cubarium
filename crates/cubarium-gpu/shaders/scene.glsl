// Shared by the two full-screen field passes and the sprite pass: the uniform
// block `crate::palette::SceneUniforms` fills, and the field sampling rule.
//
// `fieldA` is (producer, water, detritus, rain) and `fieldB` is (growth, tall, -, -),
// both `cells_x x cells_y` R16G16B16A16_SFLOAT with NEAREST filtering, REPEAT in x
// (the ring wraps) and CLAMP_TO_EDGE in y (the rims do not).
#ifndef SCENE_GLSL
#define SCENE_GLSL

layout(set = 0, binding = 0, std140) uniform Scene {
    vec4 raster;        // w, h, 1/w, 1/h
    vec4 grid;          // cells_x, cells_y, S, producer_max
    vec4 time;          // seconds, f, bilinear filter, art scale
    vec4 floorColour;   // pre-scaled by FLOOR_BRIGHTNESS
    vec4 producerLow;   // rgb, RAMP_MIN_BRIGHTNESS
    vec4 producerHigh;  // rgb, RAMP_MAX_BRIGHTNESS
    vec4 detritus;      // rgb, DETRITUS_SCALE
    vec4 soilLow;       // rgb, SOIL_MIN_BRIGHTNESS
    vec4 soilHigh;      // rgb, SOIL_MAX_BRIGHTNESS
    vec4 waterLow;      // rgb, WATER_FILM
    vec4 waterHigh;     // rgb, WATER_BRIGHT
    vec4 algae;         // rgb, ALGAE_TINT
    vec4 bands;         // SOIL_TOP, HORIZON, DETRITUS_THRESHOLD, SOIL_SCALE
    vec4 knobs;         // PRODUCER_SATURATION, WATER_SHIMMER, WATER_SHIMMER_SECONDS, -
} u;

layout(set = 0, binding = 1) uniform sampler2D fieldA;
layout(set = 0, binding = 2) uniform sampler2D fieldB;

// The cell a raster pixel belongs to. On the ring this is an integer division —
// no chart, no seam-aware pixel->cell table (presenter-budget W1 is simply absent).
ivec2 cellOf(vec2 px) {
    float cell = 4.0 * u.grid.z;
    return ivec2(floor(px / cell));
}

// One field channel at a raster pixel, without filtering: the pixel's own cell.
float cellValue(sampler2D field, int channel, ivec2 cell) {
    int nx = int(u.grid.x);
    int ny = int(u.grid.y);
    ivec2 c = ivec2((cell.x % nx + nx) % nx, clamp(cell.y, 0, ny - 1));
    return texelFetch(field, c, 0)[channel];
}

// `cubarium_render::draw_field`'s seam-aware one-pixel box filter, transcribed:
// the pixel's own cell at weight 4, each *existing* pixel neighbour's cell at 1,
// normalised over what exists so the rims do not darken. The ring wraps in x, so
// the left and right neighbours always exist; the top and bottom rows lose one.
float filteredAt(sampler2D field, int channel, vec2 px) {
    float sum = cellValue(field, channel, cellOf(px)) * 4.0;
    float weight = 4.0;
    sum += cellValue(field, channel, cellOf(px + vec2(1.0, 0.0)));
    sum += cellValue(field, channel, cellOf(px - vec2(1.0, 0.0)));
    weight += 2.0;
    if (px.y + 1.0 < u.raster.y) { sum += cellValue(field, channel, cellOf(px + vec2(0.0, 1.0))); weight += 1.0; }
    if (px.y - 1.0 >= 0.0)       { sum += cellValue(field, channel, cellOf(px - vec2(0.0, 1.0))); weight += 1.0; }
    return sum / weight;
}

// `Topology::height` on the ring: +1 at the top row, -1 at the bottom.
float heightAt(float y) {
    return 1.0 - 2.0 * (y / u.raster.y);
}

// `art_present::habitat::soil_weight`: how much of a pixel is soil.
// 1 - smoothstep(SOIL_TOP - HORIZON, SOIL_TOP + HORIZON, h).
float soilWeight(float y) {
    float h = heightAt(y);
    return 1.0 - smoothstep(u.bands.x - u.bands.y, u.bands.x + u.bands.y, h);
}

// A stable per-pixel phase in [0, 2pi). The CPU hashes with SplitMix64; any stable
// hash gives the same *kind* of fixed glint pattern, which is all this value is.
float hashPhase(ivec2 p) {
    uint h = uint(p.x) * 0x9E3779B9u ^ uint(p.y) * 0x85EBCA6Bu;
    h ^= h >> 16; h *= 0x7FEB352Du; h ^= h >> 15; h *= 0x846CA68Bu; h ^= h >> 16;
    return float(h & 0xFFFFFFu) / float(0x1000000u) * 6.28318530718;
}

#endif
