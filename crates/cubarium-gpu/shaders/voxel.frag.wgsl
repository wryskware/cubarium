// The voxel strip, drawn per pixel by a slab walk (`voxel_main.wgsl`; its faces, water,
// sky and weather in `voxel.wgsl`). The same module, overridden, is the lit tier's sky pass
// (`SKY`); the weather pass is `voxel_weather.frag.wgsl`.

// #import voxel_main.wgsl

// The walk's own drape cords (`cordShows`): it tests a cord where the cell stands.
const PLANT_PASS = false;

// What the weather lays over the raster at pixel px, down to the walk's end depth: the slab
// its ray ended in, from the emission's alpha (`WEATHER_DEPTH`).
fn weatherOver(px: vec2<i32>) -> vec4<f32> {
    let zEnd = floor(textureLoad(depthTex, px, 0).a * 255.0 + 0.5);
    return weatherAt(px, zEnd);
}
