// The lit tier's weather pass (`WEATHER`, WX2 checkpoint 2, FOG-OCCLUDE): the slab walk's
// module again (`voxel_main.wgsl`), its weather stopping in front of the model plants and
// the figures, which are drawn after the walk and so are not in its end depth.

// #import voxel_main.wgsl

const PLANT_PASS = false;

// Only the weather pass reads these. The walk, sky and flat modules keep their
// original shader interface and arithmetic, including their no-weather frame path.
@group(0) @binding(15) var weatherPlantZ: texture_2d<f32>;
@group(0) @binding(16) var weatherFigureZ: texture_depth_2d;
@group(0) @binding(17) var weatherPlantSurface: texture_2d<f32>;
@group(0) @binding(18) var weatherFigureSurface: texture_2d<f32>;

// What the weather lays over the raster at pixel px, premultiplied (colour, coverage).
fn weatherOver(px: vec2<i32>) -> vec4<f32> {
    // The walk already clamps to wEntry (the first water surface). Its emission alpha
    // encodes an integer slab; plant shade resolves front/cap z in the same slab units.
    // The figure depth is the inverse of composite.frag.wgsl's frag depth, without rounding.
    let walkZ = floor(textureLoad(depthTex, px, 0).a * 255.0 + 0.5);
    var plant = vec4(0.0);
    var figure = vec4(0.0);
    var plantZ = walkZ;
    var figureZ = walkZ;
    if ((u.camera.z & 1) != 0) {
        plant = textureLoad(weatherPlantSurface, px, 0);
        if (plant.a > 0.0) { plantZ = max(0.0, min(walkZ, textureLoad(weatherPlantZ, px, 0).r)); }
    }
    if ((u.camera.z & 2) != 0) {
        figure = textureLoad(weatherFigureSurface, px, 0);
        if (figure.a > 0.0) {
            figureZ = max(0.0, min(plantZ,
                textureLoad(weatherFigureZ, px, 0) * (f32(D) + 32.0) - 16.0));
        }
    }
    // Opaque interiors do just one integration, to the nearest of the three ends. Empty
    // pixels retain the original arithmetic, as do objects behind the walk/water end.
    let zEnd = min(walkZ, min(plantZ, figureZ));
    if (figure.a >= 1.0 || (figure.a == 0.0 && plant.a >= 1.0) || zEnd == walkZ) {
        return weatherAt(px, zEnd);
    }
    let farWeather = weatherAt(px, walkZ);
    var plantWeather = farWeather;
    if (plantZ != walkZ) { plantWeather = weatherAt(px, plantZ); }
    var figureWeather = plantWeather;
    if (figureZ != plantZ) { figureWeather = weatherAt(px, figureZ); }
    // Apply weather separately to the foreground and the visible background, then mix
    // by the ORIGINAL coverage. Interpolating depth (or just fog alpha) would attenuate
    // the already-mixed foreground/background colour and leave a fringe.
    // Raster = figure.rgb + (1-figure.a)*(plant.rgb + (1-plant.a)*background).
    let behindFigure = 1.0 - figure.a;
    var light = behindFigure * mix(farWeather.rgb, plantWeather.rgb, plant.a)
        + figure.a * figureWeather.rgb;
    light += behindFigure * plant.rgb * (farWeather.a - plantWeather.a)
        + figure.rgb * (farWeather.a - figureWeather.a);
    return vec4(light, farWeather.a);
}
