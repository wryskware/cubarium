// The voxel strip's faces, water, sky and weather: everything the slab walk
// (`voxel_main.wgsl`) draws with, shared with the plant pass (`plant_vis.frag.wgsl`,
// `plant_shade.frag.wgsl`), which calls the walk's own face functions.
//
// `crates/cubarium/src/voxel/present.rs` is the definition of this picture and draws it
// on the CPU far to near, rectangle by rectangle. The walk goes through the depth slabs
// **near to far** for one raster pixel and asks which voxel and which face owns it in each
// slab. The first opaque face ends the walk; water blends front to back on the way.
//
// Everything here is the CPU presenter's colour rules, ported one for one. Two of its
// ownership tests are genuinely local functions of a cell and its nearer neighbour and are
// ported as well: the water body's `stop` row and `nearerOwns` for a water top. The rest of
// the CPU's bookkeeping — `front_hidden`, `top_hidden`, the y-descending clipping — is what
// walking front to back replaces.
//
// Whoever imports this declares `const PLANT_PASS: bool`: true in the plant pass's two
// stages, false in the walk's.

// #import world.wgsl

// --- the faces ------------------------------------------------------------------------

// Micro-dithering grain across solid voxel faces to break up flat surfaces
fn faceGrain(x: i32, y: i32, z: i32, dx: i32, dy: i32) -> f32 {
    var h = u32((x * 73856093) ^ (y * 19349663) ^ (z * 83492791)) ^ (u32(dx) * 2654435761u)
        ^ u32(dy * 38291);
    h = (h ^ (h >> 13u)) * 1274126177u;
    return f32(i32(h & 15u) - 7) * 0.005;
}

// The lit tier's front rectangle: `blockFront`'s rules with its two tones lit. The face's
// own tone is the base under this face's light; where the flat tier leans toward the top
// face's light (the rim row, the chamfer, the riser's head) this leans toward the base
// under the top face's light at its front edge.
fn blockFrontLit(x: i32, y: i32, z: i32, v: vec4<u32>, r: i32, dx: i32) -> vec3<f32> {
    let dy = S - 1 - r;
    let openUp = !solidAt(x, y + 1, z);
    let openLeft = !solidAt(x - 1, y, z);
    let openRight = !solidAt(x + 1, y, z);
    let riser = openUp && z > 0 && !solidAt(x, y, z - 1) && solidAt(x, y - 1, z - 1);
    let onSide = (dx == 0 && openLeft) || (dx + 1 == S && openRight);

    let m = matOf(v);
    let slot = select(2 * (m - 1), TEX_TURF_SIDE, m == 3 && openUp && texOn(TEX_TURF_SIDE));
    var body = blockBody(v, faceBase(m, slot, x, y, z, 0, dx, dy));
    if (m != 0 && u.roofK.z > 0.0) {
        body = body * (1.0 + faceGrain(x, y, z, dx, dy) * (u.roofK.z / 0.04));
    }
    let own = shadeFront(body, x, y, z, dx, dy, canopyAt(x, z - 1, y));
    if (riser) {
        let lit = shadeTop(body, x, y, z, dx, RISE - 1, canopyAt(x, z, y));
        let t = select(0.0, f32(dy) / f32(S - 1), S > 1);
        let slope = mix(lit, own, u.shadeB.z * t);
        let hz = hazeAt(f32(z) - t);
        return hazed(select(slope, slope * u.shadeB.x, onSide), hz);
    }
    let haze = hazeAt(f32(z));
    var c: vec3<f32>;
    if (dy == 0 && openUp) {
        let lit = shadeTop(body, x, y, z, dx, RISE - 1, canopyAt(x, z, y));
        c = select(mix(own, lit, u.shadeA.w), lit, onSide);
    } else {
        c = select(own, own * u.shadeB.x, onSide);
    }
    return hazed(c, haze);
}

// `VoxelPresenter::block`'s front rectangle: rim row, side bevel, chamfered corner, or
// — where the ground steps one voxel into depth — the riser's lean with no rim at all.
fn blockFront(x: i32, y: i32, z: i32, v: vec4<u32>, r: i32, dx: i32) -> vec3<f32> {
    if (LIT) { return blockFrontLit(x, y, z, v, r, dx); }
    let dy = S - 1 - r;
    let openUp = !solidAt(x, y + 1, z);
    let openLeft = !solidAt(x - 1, y, z);
    let openRight = !solidAt(x + 1, y, z);
    let riser = openUp && z > 0 && !solidAt(x, y, z - 1) && solidAt(x, y - 1, z - 1);
    let onSide = (dx == 0 && openLeft) || (dx + 1 == S && openRight);

    let m = matOf(v);
    // Soil under open sky wears the turf: the same soil, with a fringe over its top rows.
    let slot = select(2 * (m - 1), TEX_TURF_SIDE, m == 3 && openUp && texOn(TEX_TURF_SIDE));
    var body = blockBody(v, faceBase(m, slot, x, y, z, 0, dx, dy));
    if (m != 0 && u.roofK.z > 0.0) {
        body = body * (1.0 + faceGrain(x, y, z, dx, dy) * (u.roofK.z / 0.04));
    }
    let lit = blockLit(body, roofShade(roofGap(x, y, z)));
    if (riser) {
        let t = select(0.0, f32(dy) / f32(S - 1), S > 1);
        let slope = mix(lit, body, u.shadeB.z * t);
        let hz = hazeAt(f32(z) - t);
        return hazed(select(slope, slope * u.shadeB.x, onSide), hz);
    }
    let haze = hazeAt(f32(z));
    let onCap = dy == 0 && openUp;
    var c: vec3<f32>;
    if (onCap) {
        c = select(mix(body, lit, u.shadeA.w), lit, onSide);   // the chamfer, then the rim
    } else {
        c = select(body, body * u.shadeB.x, onSide);
    }
    return hazed(c, haze);
}

// The lit tier's top rectangle: `blockTop`'s rules with its two tones lit. The face's own
// tone is the base under this face's light; where the flat tier leans back toward the
// unlit body (the contour row, the bevel at a drop) this leans toward the own tone over
// TOP_GAIN, the flat tier's own ratio of a top to the front it caps, so both keep their
// flat-tier contrast at every rung of the ladder.
fn blockTopLit(x: i32, y: i32, z: i32, v: vec4<u32>, r: i32, dx: i32) -> vec3<f32> {
    let dy = RISE - 1 - r;
    let openLeft = !solidAt(x - 1, y, z);
    let openRight = !solidAt(x + 1, y, z);
    let dropLeft = openLeft && !solidAt(x - 1, y - 1, z);
    let dropRight = openRight && !solidAt(x + 1, y - 1, z);
    let backContinues = z + 1 < D && solidAt(x, y, z + 1) && !solidAt(x, y + 1, z + 1);

    let m = matOf(v);
    var body = blockBody(v, faceBase(m, 2 * (m - 1) + 1, x, y, z, 1, dx, dy));
    if (m != 0 && u.roofK.z > 0.0) {
        body = body * (1.0 + faceGrain(x, y, z, dx, dy + 100) * (u.roofK.z / 0.04));
    }
    let own = shadeTop(body, x, y, z, dx, dy, canopyAt(x, z, y));
    let dark = own / u.shadeA.x;
    let hz = hazeAt(f32(z) + f32(r) / f32(RISE));
    let plane = select(own, mix(own, dark, u.shadeA.z), dy == 0 && RISE > 2 && !backContinues);
    let onDrop = (dx == 0 && dropLeft) || (dx + 1 == S && dropRight);
    return hazed(select(plane, mix(plane, dark, u.shadeB.y), onDrop), hz);
}

// `VoxelPresenter::block`'s top rectangle: the contour row only where the ground really
// ends going back, and a bevel only at a real drop.
fn blockTop(x: i32, y: i32, z: i32, v: vec4<u32>, r: i32, dx: i32) -> vec3<f32> {
    if (LIT) { return blockTopLit(x, y, z, v, r, dx); }
    let dy = RISE - 1 - r;
    let openLeft = !solidAt(x - 1, y, z);
    let openRight = !solidAt(x + 1, y, z);
    let dropLeft = openLeft && !solidAt(x - 1, y - 1, z);
    let dropRight = openRight && !solidAt(x + 1, y - 1, z);
    let backContinues = z + 1 < D && solidAt(x, y, z + 1) && !solidAt(x, y + 1, z + 1);

    let m = matOf(v);
    var body = blockBody(v, faceBase(m, 2 * (m - 1) + 1, x, y, z, 1, dx, dy));
    if (m != 0 && u.roofK.z > 0.0) {
        body = body * (1.0 + faceGrain(x, y, z, dx, dy + 100) * (u.roofK.z / 0.04));
    }
    let lit = blockLit(body, roofShade(roofGap(x, y, z)));
    let hz = hazeAt(f32(z) + f32(r) / f32(RISE));
    let plane = select(lit, mix(lit, body, u.shadeA.z), dy == 0 && RISE > 2 && !backContinues);
    let onDrop = (dx == 0 && dropLeft) || (dx + 1 == S && dropRight);
    return hazed(select(plane, mix(plane, body, u.shadeB.y), onDrop), hz);
}

fn glyphPigment(v: vec4<u32>, q: u32) -> vec3<f32> {
    return styleAt(i32(v.a), i32(q & 3u));
}

// The texture slot a style's cells draw with on this face, or -1: only a baked model's
// trunk, foliage and drape have one.
fn roleSlot(v: vec4<u32>, top: bool) -> i32 {
    let role = i32(textureLoad(styleTex, vec2(0, i32(v.a)), 0).a + 0.5);
    if (role < ROLE_BARK || role > ROLE_DRAPE) { return -1; }
    return TEX_BARK_SIDE + 2 * (role - ROLE_BARK) + select(0, 1, top);
}

// A style's species face slot on this face (`VoxelStyle::with_faces`: the alpha of its
// crown column for the side, of its heart column for the top, slot + 1), or -1.
fn speciesSlot(v: vec4<u32>, top: bool) -> i32 {
    return i32(textureLoad(styleTex, vec2(select(1, 2, top), i32(v.a)), 0).a + 0.5) - 1;
}

// A drape cord's texel (`drape_cord.wgsl`): never its cap; on its front face, the cord's
// pixels in the walk, and every pixel in the plant pass, which tested the cord itself where
// the swaying cell put it (`plantCordShows`). The cord is the cell's whole cutout: no drape
// texture over it.
fn cordShows(x: i32, y: i32, z: i32, top: bool, dx: i32, dy: i32) -> bool {
    if (PLANT_PASS) { return !top; }
    return !top && cordTexel(x, y, z, dx, dy);
}

// A model cell's texture over its pigment. A species face (`species/<species>/`) is
// direct colour and replaces the pigment; a generic one's `r / 128` multiplies the style
// colour, so the colour pass still decides the hue. False where a leaf or drape cutout
// has a hole: that texel is not this cell's, and the walk goes on to whatever is behind
// it.
fn plantTexel(x: i32, y: i32, z: i32, v: vec4<u32>, top: bool, dx: i32, dy: i32,
              base: ptr<function, vec3<f32>>) -> bool {
    if (cordCell(v)) { return cordShows(x, y, z, top, dx, dy); }
    let own = speciesSlot(v, top);
    if (own >= 0) {
        let t = faceTexel(own, x, y, z, select(0, 1, top), dx, dy);
        let role = i32(textureLoad(styleTex, vec2(0, i32(v.a)), 0).a + 0.5);
        if (role >= ROLE_BARK + 1 && role <= ROLE_DRAPE && t.a < 0.5) { return false; }
        *base = srgbToLinear(t.rgb);
        return true;
    }
    let slot = roleSlot(v, top);
    if (!texOn(slot)) { return true; }
    let t = faceTexel(slot, x, y, z, select(0, 1, top), dx, dy);
    if (slot > TEX_BARK_SIDE + 1 && t.a < 0.5) { return false; }
    *base *= t.r * (255.0 / 128.0);
    return true;
}

// Organism anatomy and markings are already resolved in glyphTex by the shared
// appearance layer. This is deliberately generic: the shader knows only pigment slots
// and treatments, never bodies, heads, eyes or facing.
// `row` is the face's row from its top (S - 1 - r for a cell where it stands); a row from
// S on is row `row - S` of its cap, from the back edge, as the atlas lays them out. A
// posing body's cap (WX4) is drawn through here from the band's own voxel, so the walk
// still calls each glyph function from one place.
fn glyphFrontAt(x: i32, y: i32, z: i32, v: vec4<u32>, row: i32, dx: i32,
                rgb: ptr<function, vec3<f32>>, glow: ptr<function, vec3<f32>>) -> bool {
    *glow = vec3(0.0);
    let q = cordGlyphTexel(v, dx, row);
    let pigment = textureLoad(styleTex, vec2(i32(q & 3u), i32(v.a)), 0);
    var base = pigment.rgb;
    let tone = i32(q >> 2u);
    let top = POSE && row >= S;
    let localY = select(row, row - S, top);
    let hz = hazeAt(f32(z) + select(0.0, f32(RISE - 1 - localY) / f32(RISE), top));
    if (tone == 63 && !top) { return false; }
    // GLOW-EASE: an emitter part way on adds its colour over its pigment's share.
    var eAdd = vec3(0.0);
    var keep = 1.0;
    if (LIT && (tone == TONE_EMIT || emitsWhole(pigment.a))) {
        // An emitter: its style's emissive colour (or the host's for this cell this
        // frame), unshadowed and at full value.
        var e = styleEmit(i32(v.a));
        var on = 1.0;
        cellEmit(x, y, z, &e, &on);
        if (any(e > vec3(0.0))) {
            // `glow`, the emission alone, is what the bloom and the walk's emitter test read.
            *glow = hazed(e, hz);
            if (on >= 1.0) {
                *rgb = *glow;
                return true;
            }
            eAdd = e;
            keep = 1.0 - on;
        }
    }
    if (!plantTexel(x, y, z, v, top, dx, localY, &base)) { return false; }
    if (LIT) {
        let coveredUp = !top && (solidAt(x, y + 1, z)
            || (inY(y + 1) && isBlockPart(partOf(at(x, y + 1, z))) && !softMark(at(x, y + 1, z))));
        // The atlas's tones on the lit base: the rim and the lit column lean toward the
        // base under the cell's top light, every other tone keeps its pigment rule under
        // this face's light. A cap is the base under its top light, its edge tones leaning
        // toward it over PLANT_TOP_GAIN. One top light and one front light at most, so
        // each shading function is inlined once.
        // A plant cell's own stand does not shade it; an animal reads the column.
        let animal = partOf(v) == ANIMAL_INTERIM;
        var cf = plantCanopy(v);
        var ct = cf;
        if (animal) {
            cf = canopyAt(x, z - 1, y);
            ct = canopyAt(x, z, y);
        }
        let rim = !top && tone == 2 && !coveredUp;
        var tl = vec3(0.0);
        if (top || rim || tone == 3) {
            tl = shadeTop(base, x, y, z, dx, select(RISE - 1, localY, top), ct);
        }
        var c = tl;
        if (top) {
            if (tone == 1) {
                c *= u.plantA.w;
            } else if (tone >= 32 && tone <= 47) {
                let gain = 0.5 + f32(tone & 15) / 16.0;
                c = mix(c, tl / u.plantA.x * gain, u.shadeB.y);
            }
        } else if (tone != 3) {
            if (tone == 1) {
                base *= u.shadeB.x;
            } else if (tone == 4 || tone == 5) {
                base = mix(base, styleAt(i32(v.a), 0), u.plantB.x);
                if (tone == 5) { base *= mix(1.0, u.plantA.w, crownEdge(x, y, z, dx)); }
            } else if (tone >= 16 && tone <= 31) {
                base *= 0.5 + f32(tone & 15) / 16.0;
            }
            let f = shadeFront(base, x, y, z, dx, localY, cf);
            c = select(f, mix(f, tl, u.plantA.z), rim);
        }
        *rgb = hazed(c * keep + eAdd, hz);
        return true;
    }
    if (top) {
        // A cap, as `glyphCapAt` draws it.
        var c = plantLit(base, roofShade(roofGap(x, y, z)));
        if (tone == 1) {
            c *= u.plantA.w;
        } else if (tone >= 32 && tone <= 47) {
            let gain = 0.5 + f32(tone & 15) / 16.0;
            c = mix(c, base * gain, u.shadeB.y);
        }
        *rgb = hazed(c, hz);
        return true;
    }
    let coveredUp = solidAt(x, y + 1, z)
        || (inY(y + 1) && isBlockPart(partOf(at(x, y + 1, z))) && !softMark(at(x, y + 1, z)));
    if (tone == 1) {
        base *= u.shadeB.x;
    } else if (tone == 2 && !coveredUp) {
        base = mix(base, plantLit(base, roofShade(roofGap(x, y, z))), u.plantA.z);
    } else if (tone == 3) {
        base = plantLit(base, roofShade(roofGap(x, y, z)));
    } else if (tone == 4 || tone == 5) {
        base = mix(base, styleAt(i32(v.a), 0), u.plantB.x);
        if (tone == 5) { base *= mix(1.0, u.plantA.w, crownEdge(x, y, z, dx)); }
    } else if (tone >= 16 && tone <= 31) {
        base *= 0.5 + f32(tone & 15) / 16.0;
    }
    *rgb = hazed(base, hz);
    return true;
}

// `localY` is the cap's row from its back edge (RISE - 1 - r for a cell where it stands).
fn glyphCapAt(x: i32, y: i32, z: i32, v: vec4<u32>, localY: i32, dx: i32,
              rgb: ptr<function, vec3<f32>>, glow: ptr<function, vec3<f32>>) -> bool {
    *glow = vec3(0.0);
    let r = RISE - 1 - localY;
    let atlasY = (partOf(v) * 8 + glyphOf(v)) * (S + RISE) + S + localY;
    let q = textureLoad(glyphTex, vec2(dx, atlasY), 0).r;
    let pigment = textureLoad(styleTex, vec2(i32(q & 3u), i32(v.a)), 0);
    var base = pigment.rgb;
    let tone = i32(q >> 2u);
    var eAdd = vec3(0.0);
    var keep = 1.0;
    if (LIT && (tone == TONE_EMIT || emitsWhole(pigment.a))) {
        var e = styleEmit(i32(v.a));
        var on = 1.0;
        cellEmit(x, y, z, &e, &on);
        if (any(e > vec3(0.0))) {
            *glow = hazed(e, hazeAt(f32(z) + f32(r) / f32(RISE)));
            if (on >= 1.0) {
                *rgb = *glow;
                return true;
            }
            eAdd = e;
            keep = 1.0 - on;
        }
    }
    if (!plantTexel(x, y, z, v, true, dx, localY, &base)) { return false; }
    if (LIT) {
        // The cap's own tone is the base under its light; the edge tones lean toward it
        // over PLANT_TOP_GAIN, as the flat tier leans toward the unlit base.
        var ct = plantCanopy(v);
        if (partOf(v) == ANIMAL_INTERIM) { ct = canopyAt(x, z, y); }
        let own = shadeTop(base, x, y, z, dx, localY, ct);
        var lc = own;
        if (tone == 1) {
            lc *= mix(1.0, u.plantA.w, crownEdge(x, y, z, dx));
        } else if (tone >= 32 && tone <= 47) {
            let gain = 0.5 + f32(tone & 15) / 16.0;
            lc = mix(lc, own / u.plantA.x * gain, u.shadeB.y);
        }
        *rgb = hazed(lc * keep + eAdd, hazeAt(f32(z) + f32(r) / f32(RISE)));
        return true;
    }
    let shade = roofShade(roofGap(x, y, z));
    var cap = plantLit(base, shade);
    if (tone == 1) {
        cap *= mix(1.0, u.plantA.w, crownEdge(x, y, z, dx));
    } else if (tone >= 32 && tone <= 47) {
        let gain = 0.5 + f32(tone & 15) / 16.0;
        cap = mix(cap, base * gain, u.shadeB.y);
    }
    *rgb = hazed(cap, hazeAt(f32(z) + f32(r) / f32(RISE)));
    return true;
}

// A dry plant cell the plant pass draws instead of the walk (`PLANT_MODELS`). A cell with
// water in it stays the walk's: the water of its own cell blends over it.
fn plantHidden(v: vec4<u32>) -> bool { return PLANT_MODELS && v.g == 0u && plantBlockPart(partOf(v)); }

// --- the sleep and idle poses (WX4, `cubarium::voxel::sleep`, `cubarium::voxel::idle`) ------
//
// A sleeping body is stamped where it stands awake. Each of its cells carries its class
// (b's top two bits) and its pose entry (b's low six; b = 0 is a body not posing, drawn as
// it always was), and is drawn displaced by a fraction of a voxel recomputed from the
// frame's clock: a torso cell down by the settle, a breathing cell's top also down by the
// breath's dip, a leg cut off at the settling torso's bottom. So the settle and the breath
// move a pixel row at a time and no cell ever appears or goes. The host evaluates each
// pose on the frame's clock (`pose_rows`), so the walk only reads two integers. The empty voxels under a
// settling torso are faceless leg cells (glyph 0): they draw nothing and only send the walk
// looking for the torso. Nothing is drawn above its own voxel's top, so the walk's own
// voxel and the cap under it are the only places a search starts.
//
// An idle body (awake and standing still) breathes the same way, and now and then dips its
// head: the head cells take an entry of their own whose settle is the dip. The empty voxels
// a dipping head reaches into are faceless torso-class cells (`softMark`).

fn isPosed(v: vec4<u32>) -> bool { return POSE && partOf(v) == ANIMAL_INTERIM && v.b != 0u; }

// Where posing cell c of row sy is drawn: its front face's bottom and top, in the walk's
// q (y * S plus rows up). `sleep::span`.
fn posedSpan(c: vec4<u32>, sy: i32) -> vec2<i32> {
    let cls = i32(c.b >> 6u);
    let db = vec2<i32>(u.poseK[i32(c.b & 63u)].xy);
    if (cls >= 2) {
        return vec2(sy * S, max(sy * S, min((sy + 1) * S, (sy + cls - 1) * S - db.x)));
    }
    return vec2(sy * S - db.x, (sy + 1) * S - db.x - select(0, db.y, cls == 1));
}

// The face row of posing cell c at q, given its span: a leg is cut at its top, its face
// where it was; a torso cell's face moves with it, and a breathing cell brought down by a
// dip keeps its rim row on top and loses the rows under it.
fn posedRow(c: vec4<u32>, sy: i32, sp: vec2<i32>, q: i32) -> i32 {
    if (i32(c.b >> 6u) >= 2) { return (sy + 1) * S - 1 - q; }
    let t = sp.y - 1 - q;
    return select(0, t + S - (sp.y - sp.x), t > 0);
}

// Which posing cell owns row r of band `level`, and which of its rows (a cap's from S on,
// for `glyphFrontAt`): from the rows a settling cell can reach the band from, two over it
// (a torso settles up to two rows) and the band's own. A front face wins, since they all
// lie in the slab's front plane with every cap behind; else the highest cap. The cell
// under the band shows only its cap, which the walk's cap site draws.
fn posedFind(x: i32, level: i32, z: i32, r: i32, gy: ptr<function, i32>, row: ptr<function, i32>) -> bool {
    let q = level * S + r;
    *gy = level;
    *row = -1;
    for (var i = 0; i < 3; i++) {
        let sy = level + 2 - i;
        if (!inY(sy)) { continue; }
        let c = at(x, sy, z);
        if (!isPosed(c) || glyphOf(c) == 0) { continue; }
        let sp = posedSpan(c, sy);
        // A cell cut to nothing is folded away, cap and all (the hunker's neck; a sleeper's
        // leg is under its torso anyway).
        if (sp.y <= sp.x) { continue; }
        if (q >= sp.x && q < sp.y) {
            *gy = sy;
            *row = posedRow(c, sy, sp, q);
            return true;
        }
        if (*row < 0 && q >= sp.y && q < sp.y + RISE) {
            *gy = sy;
            *row = S + RISE - 1 - (q - sp.y);
        }
    }
    return *row >= 0;
}

// The cap row of posing cell v under band `level` at row r, or -1: where its settle and
// breath put its cap.
fn posedCapRow(v: vec4<u32>, level: i32, r: i32) -> i32 {
    if (glyphOf(v) == 0) { return -1; }
    let sp = posedSpan(v, level - 1);
    if (sp.y <= sp.x) { return -1; }
    let q = level * S + r;
    return select(-1, RISE - 1 - (q - sp.y), q >= sp.y && q < sp.y + RISE);
}

// --- water ----------------------------------------------------------------------------

// `VoxelPresenter::nearer_owns`: does the slab one step nearer already own this screen
// row, in the pixel columns of voxel (x, y, z)? The bands are the unclipped ones on
// purpose — where the nearer slab's own face is itself culled, whatever culls it is
// nearer still.
fn nearerOwns(x: i32, y: i32, z: i32, row: i32) -> bool {
    if (z == 0) { return false; }
    let R = frontRow(y, z);
    for (var dy = 0; dy < 3; dy++) {
        let yn = y + dy;
        let f = R + RISE - dy * S;
        var top: i32;
        if (solidAt(x, yn, z - 1)) {
            top = f - RISE;
        } else {
            let fill = fillPxAt(x, yn, z - 1);
            if (fill == 0) { continue; }
            let skin = f + S - fill;
            top = select(skin, skin - RISE, waterOpenUp(x, yn, z - 1));
        }
        if (row >= top && row < f + S) { return true; }
    }
    return false;
}

// --- water, the lit tier -------------------------------------------------------------------
//
// The boundary is the flat tier's, exactly: `waterAtLit` runs the same row tests
// (`fillPxQ`, the skin rows, `nearerOwns`, `stop`), and a pixel is water in the lit tier
// if and only if one of them marks it. Only the colour inside changes:
//
// - Depth absorption: from the slab that first marks the pixel on, the walk's ray is
//   followed through the water it really crosses, cell by cell (the part of each slab's
//   segment, √(1 + (RISE/S)²) voxels long, under the cell's water line), and each piece
//   absorbs per channel by Beer–Lambert with σ = `absorb` × −ln(deep colour): after
//   1 / `absorb` voxels of water what is left of the light behind is the palette's deep
//   colour itself. What the water scatters in runs from the palette's surface colour at
//   the surface to its deep colour at depth under it (the same `absorb` sets that ramp,
//   over the view ray's path to the depth). The
//   in-scatter is lit by the ladder at the water's entry, as a ratio to open sunlit sky,
//   so open water keeps the palette's colours and water under cover darkens with it.
// - The surface (a pixel whose first mark is a skin top-face row): an animated, quantised
//   normal and a Fresnel mix with a reflection marched through the volume.
// - Falling water (a water cell over a cell that is neither solid nor full) has no ripple
//   and no reflection, but streaks that scroll down.
//
// Everything that needs more than a few texel fetches (the light, the flow, the normal,
// the reflection, the streaks) runs once per pixel after the walk, from what the entry
// recorded: front to back is linear, so the surface can be put in front afterwards.
// Nothing reaches the pixel before its first water (every other contribution ends the
// walk), so in front of the entry the walk has gathered nothing and let everything
// through. **The walk carries as little as it can**: one int for the entry, from which
// everything else about it is recomputed afterwards (state carried through the loop
// costs every pixel occupancy, water or not: +1.7 ms at 13 px with the entry point, its
// open cell and its canopy carried).

var<private> wEntry: i32;         // the first water: y | z << 12 | top-row << 24 | flat << 25 | falling << 26,
                                  // or -1 for none
var<private> wScatter: vec3<f32>; // in-scatter so far, unlit and unhazed, weighted by transmission
var<private> wHaze: f32;          // the haze share of that in-scatter (its weight, averaged over the channels)
var<private> wSurf: f32;          // the height of the entered water's surface
var<private> PX: i32;             // this pixel's raster column
var<private> PY: i32;             // and row

fn waterLitReset() {
    wEntry = -1;
    wScatter = vec3(0.0);
    wHaze = 0.0;
    wSurf = 0.0;
}

// Per-channel absorption per voxel of water path.
fn waterSigma() -> vec3<f32> { return -log(max(u.waterDeepC.rgb, vec3(1.0 / 4096.0))) * u.waterL.x; }

// Entry flags: the pixel's water is drawn by the flat tier's rule (`waterAt`), and its
// first water is falling.
const ENTRY_FLAT = 1 << 25u;
const ENTRY_FALLING = 1 << 26u;

// Whether the water in cell (x, y, z) is falling: the cell under it is neither solid nor
// full (in drawn pixels). `flowAt` reads the same.
fn fallingAt(x: i32, y: i32, z: i32) -> bool {
    return y > 0 && !solidAt(x, y - 1, z) && fillPxAt(x, y - 1, z) < S;
}

// The lit water for one screen row. A pixel whose first water is the lake's cut face at
// the world's front edge, or falling water, takes the flat tier's rule for all of its
// water (`waterAt`'s colours and opacities, off the ladder), carried in the lit tier's
// own accumulators: running the flat code itself in the walk costs +0.8 ms at 13 px in
// occupancy.
fn waterAtLit(x: i32, y: i32, z: i32, v: vec4<u32>, row: i32,
              acc: ptr<function, vec3<f32>>, trans: ptr<function, vec3<f32>>) {
    let fill = fillPxQ(i32(v.g));
    let R = frontRow(y, z);
    let bottom = R + S;
    let skinRow = bottom - fill;
    let openUp = waterOpenUp(x, y, z);
    let topRow = openUp && row >= skinRow - RISE && row < skinRow && !nearerOwns(x, y, z, row);
    let nearSolid = z > 0 && solidAt(x, y, z - 1);
    var nearFill = 0;
    if (z > 0) { nearFill = fillPxAt(x, y, z - 1); }
    let nearSkin = R + RISE + S - nearFill;
    var stop = bottom;
    if (nearSolid) {
        stop = skinRow;
    } else if (nearFill > 0) {
        let nearOpenUp = waterOpenUp(x, y, z - 1);
        stop = min(bottom, select(nearSkin, nearSkin - RISE, nearOpenUp));
    }
    let bodyRow = row >= skinRow && row < stop;

    let k = f32(RISE) / f32(S);
    // The pixel's height on this slab's front plane; the ray falls k voxels a slab.
    let yf = (f32(BASE - z * RISE - row - 1) + 0.5) / f32(S);
    let h = hazeAt(f32(z) + 0.5);
    if (wEntry < 0) {
        if (!topRow && !bodyRow) { return; }
        wEntry = y | (z << 12u) | select(0, 1 << 24u, topRow);
        // The lake's cut face at the world's front edge keeps the flat tier's deep indigo,
        // and falling water its bright surface colour: both by the flat rule, off the
        // ladder (the finish adds the fall's streaks).
        let falling = fallingAt(x, y, z);
        if ((z == 0 && !topRow) || falling) {
            wEntry |= ENTRY_FLAT | select(0, ENTRY_FALLING, falling);
        } else {
            // The surface over the entry: its own top face, or up the column a front face
            // belongs to.
            var ys = y;
            for (var i = 0; i < 8 && !topRow; i++) {
                if (solidAt(x, ys + 1, z) || freeQ(x, ys + 1, z) == 0) { break; }
                ys++;
            }
            var surfaceFill = fill;
            if (!topRow) { surfaceFill = fillPxAt(x, ys, z); }
            wSurf = f32(ys) + f32(surfaceFill) / f32(S);
            if (bodyRow && row == skinRow && openUp) {
                // The front's top row: the flat tier's skin line, in the surface colour.
                let a = min(clamp(u.knobs.y, 0.0, 1.0) * u.waterK.x, 0.95);
                wScatter += *trans * a * (1.0 - h) * u.waterSurfaceC.rgb;
                wHaze += dot(*trans, vec3(a * h / 3.0));
                *trans *= 1.0 - a;
            }
        }
    }
    if ((wEntry & ENTRY_FLAT) != 0) {
        let alpha = clamp(u.knobs.y, 0.0, 1.0);
        let skinAlpha = min(alpha * u.waterK.x, 0.95);
        var a: f32;
        var c: vec3<f32>;
        var hf: f32;
        if (topRow) {
            a = skinAlpha * u.waterK.y;
            c = u.waterSurfaceC.rgb;
            hf = hazeAt(f32(z) + f32(RISE - 1 - (row - (skinRow - RISE))) / f32(RISE));
        } else if (bodyRow) {
            let isSkin = row == skinRow && openUp;
            a = select(alpha, skinAlpha, isSkin);
            c = select(u.waterDeepC.rgb, u.waterSurfaceC.rgb, isSkin);
            hf = hazeAt(f32(z));
        } else {
            return;
        }
        wScatter += *trans * a * (1.0 - hf) * c;
        wHaze += dot(*trans, vec3(a * hf / 3.0));
        *trans *= 1.0 - a;
        return;
    }
    // The ray's piece in this cell's water, in this slab.
    let top = f32(y) + f32(fill) / f32(S);
    let hi = min(yf, top);
    let lo = max(yf - k, f32(y));
    let l = max(hi - lo, 0.0) / k * sqrt(1.0 + k * k);
    if (l <= 0.0) { return; }
    let T = exp(-waterSigma() * l);
    // The in-scatter ramp runs on depth under the surface, as the view ray's path to
    // that depth (l / (hi - lo) voxels of path a voxel of depth).
    let depth = max(wSurf - 0.5 * (hi + lo), 0.0) * sqrt(1.0 + k * k) / k;
    let cin = mix(u.waterSurfaceC.rgb, u.waterDeepC.rgb, 1.0 - exp(-u.waterL.x * depth));
    wScatter += *trans * (1.0 - T) * (1.0 - h) * cin;
    wHaze += dot(*trans * (1.0 - T), vec3(h / 3.0));
    *trans *= T;
}

// `VoxelPresenter::water`, for one screen row: the surface's own receding top face and
// the body below it, each blended at most once.
fn waterAt(x: i32, y: i32, z: i32, v: vec4<u32>, row: i32,
           acc: ptr<function, vec3<f32>>, trans: ptr<function, vec3<f32>>) {
    if (LIT) { waterAtLit(x, y, z, v, row, acc, trans); return; }
    let fill = fillPxQ(i32(v.g));
    let R = frontRow(y, z);
    let bottom = R + S;
    let skinRow = bottom - fill;
    let alpha = clamp(u.knobs.y, 0.0, 1.0);
    let skinAlpha = min(alpha * u.waterK.x, 0.95);
    let openUp = waterOpenUp(x, y, z);

    if (openUp && row >= skinRow - RISE && row < skinRow && !nearerOwns(x, y, z, row)) {
        let dy = row - (skinRow - RISE);
        let c = hazed(u.waterSurfaceC.rgb, hazeAt(f32(z) + f32(RISE - 1 - dy) / f32(RISE)));
        let a = skinAlpha * u.waterK.y;
        *acc += *trans * a * c;
        *trans *= 1.0 - a;
    }

    let nearSolid = z > 0 && solidAt(x, y, z - 1);
    var nearFill = 0;
    if (z > 0) { nearFill = fillPxAt(x, y, z - 1); }
    let nearSkin = R + RISE + S - nearFill;
    var stop = bottom;
    if (nearSolid) {
        stop = skinRow;
    } else if (nearFill > 0) {
        let nearOpenUp = waterOpenUp(x, y, z - 1);
        stop = min(bottom, select(nearSkin, nearSkin - RISE, nearOpenUp));
    }
    if (row >= skinRow && row < stop) {
        let isSkin = row == skinRow && openUp;
        let haze = hazeAt(f32(z));
        let c = hazed(select(u.waterDeepC.rgb, u.waterSurfaceC.rgb, isSkin), haze);
        let a = select(alpha, skinAlpha, isSkin);
        *acc += *trans * a * c;
        *trans *= 1.0 - a;
    }
}

// --- the sky ----------------------------------------------------------------------------

// The twilight band (`dayK.w`, zero away from dawn and dusk), `t` of the way down the
// raster: purple fading in from above, magenta, then the palette's warm orange low down,
// where the sky meets the far ridge.
fn bandAt(t: f32) -> vec3<f32> {
    let b = smoothstep(0.04, 0.56, t);
    var c = mix(u.bandA.rgb, u.bandB.rgb, smoothstep(0.0, 0.5, b));
    c = mix(c, u.bandC.rgb, smoothstep(0.5, 1.0, b));
    return c * (b * b * u.dayK.w);
}

// Vertical sky gradient from zenith down to horizon (the time of day's: the palette's by
// day), with the twilight band over it at dawn and dusk.
fn skyAt(py: i32) -> vec3<f32> {
    if (u.roofK.w > 0.5) {
        let t = clamp(f32(py + u.camera.x) / f32(max(u.camera.y, 1)), 0.0, 1.0);
        var c = mix(u.skyC.rgb, u.skyHorizon.rgb, t);
        if (u.dayK.w > 0.0) { c += bandAt(t); }
        return c;
    }
    return u.skyC.rgb;
}

// Atmospheric sky with drifting moisture cloud wisps. The lit tier's is the sky pass's
// (stars and clouds instead of wisps).
fn skyColor(px_x: i32, px_y: i32) -> vec3<f32> {
    if (LIT) { return textureLoad(backdropTex, vec2(px_x, px_y), 0).rgb; }
    var sky = skyAt(px_y);
    let moisture = clamp(u.knobs.z, 0.0, 1.0);
    let mistRows = min(u.extent.w / 3, 56);
    if (u.roofK.w > 0.5 && moisture > 0.0 && px_y < mistRows) {
        let verticalT = 1.0 - (f32(px_y) / f32(mistRows));
        let rowFactor = verticalT * verticalT * moisture;
        let driftX = i32(u.knobs.w) / 2;
        let wave1 = sin(f32(px_x + driftX + px_y * 4) * 0.045) * 0.5 + 0.5;
        let wave2 = cos(f32(px_x * 2 - driftX + 37) * 0.025) * 0.5 + 0.5;
        let density = wave1 * wave2 * rowFactor;
        if (density > 0.10) {
            let a = min((density - 0.10) * 1.8, 0.70);
            let cloudC = vec3(0.038, 0.020, 0.102);
            let cloudEdge = vec3(0.113, 0.171, 0.354);
            let col = mix(cloudC, cloudEdge, wave1 * 0.5);
            sky = mix(sky, col, a);
        }
    }
    return sky;
}

// --- water after the walk (the lit tier) --------------------------------------------------

// The flow field, derived from the water state alone (the solver hands the renderer no
// velocity): lateral flow runs down the gradient of the free-surface height y + free
// against the four lateral neighbours' surfaces, and a water cell over a cell that is
// neither solid nor full is falling. Computed here, per water pixel, from the free bytes
// the walk already reads: at most 17 texel fetches, once per pixel.

// A neighbour that is a wall (solid, or outside the world in z): no flow across it.
const WALL: f32 = -1.0e6;
// The surface drop per voxel that is full speed, and the steps the speed is quantised to
// (0, 1/3, 2/3, 1: a drop under a sixth of FLOW_FULL per voxel is still water).
const FLOW_FULL: f32 = 0.5;
const FLOW_STEPS: f32 = 3.0;

// The free surface near level y in column (x, z): the water over it, in it, or the water
// or ground at most two cells under it.
fn surfaceNear(x: i32, y: i32, z: i32) -> f32 {
    if (z < 0 || z >= D || solidAt(x, y, z)) { return WALL; }
    var ga = 0;
    if (!solidAt(x, y + 1, z)) { ga = freeQ(x, y + 1, z); }
    if (ga > 0) { return f32(y + 1) + f32(ga) / 255.0; }
    let g = freeQ(x, y, z);
    if (g > 0) { return f32(y) + f32(g) / 255.0; }
    for (var yy = y - 1; yy >= max(y - 2, 0); yy--) {
        if (solidAt(x, yy, z)) { return f32(yy + 1); }
        let gg = freeQ(x, yy, z);
        if (gg > 0) { return f32(yy) + f32(gg) / 255.0; }
    }
    return f32(max(y - 2, 0));
}

// The surface's slope along one axis from its two neighbours: central where both are
// water or open, one-sided against a wall, flat between two walls.
fn slope1(a: f32, h0: f32, b: f32) -> f32 {
    let wa = a < WALL * 0.5;
    let wb = b < WALL * 0.5;
    if (wa && wb) { return 0.0; }
    if (wa) { return b - h0; }
    if (wb) { return h0 - a; }
    return 0.5 * (b - a);
}

// xy: the flow's direction (world x, z) times its quantised speed; z: 1 where falling.
fn flowAt(c: vec3<i32>) -> vec3<f32> {
    let x = c.x;
    let y = c.y;
    let z = c.z;
    if (y > 0 && !solidAt(x, y - 1, z) && fillPxAt(x, y - 1, z) < S) {
        return vec3(0.0, 0.0, 1.0);
    }
    // The cell's own column by the same rule as its neighbours': a submerged cell (a
    // front face's lower rows) reads the surface over it, as they do.
    let h0 = surfaceNear(x, y, z);
    let gx = slope1(surfaceNear(x - 1, y, z), h0, surfaceNear(x + 1, y, z));
    let gz = slope1(surfaceNear(x, y, z - 1), h0, surfaceNear(x, y, z + 1));
    let f = -vec2(gx, gz);
    let m = length(f);
    let q = floor(clamp(m / FLOW_FULL, 0.0, 1.0) * FLOW_STEPS + 0.5) / FLOW_STEPS;
    return vec3(select(vec2(0.0), f / m * q, m > 0.0), 0.0);
}

fn wrapI(a: i32, m: i32) -> i32 { return wrapMod(a, m); }

// Steps in one flow-map cycle, and voxels the ripples move a step at full speed (one
// voxel a second at 12 Hz). Two copies half a cycle apart take turns: a ripple line
// appears, drifts along the flow and fades within its copy's cycle, so the advection never
// runs away and a restart is never seen.
const FLOW_CYCLE: f32 = 64.0;
const FLOW_ADVECT: f32 = 1.0 / 12.0;
// The ripple lines: segments this many voxels long along x (a line takes part of one),
// and the share of (row, segment) places that carry a line some time in its cycle.
const LINE_SEG: f32 = 2.0;
const LINE_DENSITY: f32 = 0.16;
// A line lives this share of its copy's cycle (between the two), starting at a random
// point in it; it fades in and out over its life (sin²) and grows from its middle.
const LINE_LIFE_MIN: f32 = 0.3;
const LINE_LIFE_MAX: f32 = 0.6;
// A breeze: each row of lines slides along +x at its own speed, between these voxels a
// second at full water speed, on still and moving water alike.
const BREEZE_MIN: f32 = 0.1;
const BREEZE_MAX: f32 = 0.3;

// One copy of the ripple lines at world (x, z), `phase` of the way through cycle `cycle`:
// x is 0 for none, else the tilt along z, signed, times the line's fade (0 to 1); y is the
// line's glint weight, its fade times a taper from the line's middle (1) to its ends (0).
// A line is one raster row of a top face (RISE rows a voxel of z) and part of a
// LINE_SEG-voxel segment.
fn rippleLine(w: vec2<f32>, flow: vec2<f32>, phase: f32, cycle: i32, salt: i32) -> vec2<f32> {
    let t = phase * FLOW_CYCLE * FLOW_ADVECT;
    var a = w - flow * t;
    let row = i32(floor(a.y * f32(RISE)));
    let hr = cellHash(row, 0, cycle, salt);
    a.x -= mix(BREEZE_MIN, BREEZE_MAX, f32((hr >> 8u) & 0xFFu) / 255.0) * t;
    // Segments across the ring: a whole number, so the pattern meets itself at the seam.
    let segs = max(1, i32(f32(W) / LINE_SEG + 0.5));
    let sx = a.x * f32(segs) / f32(W) + f32(hr & 0xFFu) / 256.0;
    let seg = i32(floor(sx));
    let h = cellHash(wrapI(seg, segs), row, cycle, salt + 1);
    if (f32(h & 0xFFFFu) / 65535.0 >= LINE_DENSITY) { return vec2(0.0); }
    let hl = cellHash(wrapI(seg, segs), row, cycle, salt + 2);
    let life = mix(LINE_LIFE_MIN, LINE_LIFE_MAX, f32(hl & 0xFFu) / 255.0);
    let age = (phase - f32((hl >> 8u) & 0xFFu) / 255.0 * (1.0 - life)) / life;
    if (age <= 0.0 || age >= 1.0) { return vec2(0.0); }
    var fade = sin(3.14159265 * age);
    fade *= fade;
    let f = sx - f32(seg);
    let mid = 0.25 + f32((h >> 16u) & 0xFFu) / 255.0 * 0.5;
    let halfLen = (0.1 + f32((h >> 24u) & 0x7Fu) / 127.0 * 0.15) * sqrt(fade);
    if (abs(f - mid) >= halfLen) { return vec2(0.0); }
    let along = (f - mid) / halfLen;
    let taper = 1.0 - along * along;
    return vec2(select(-1.0, 1.0, (h & 0x80000000u) != 0u) * fade, fade * taper * taper);
}

// The surface normal at world (x, z) under flow `flow`: straight up, or, on a ripple line,
// tilted along z (toward or away from the camera) by up to the ripple knob. Lines are
// sparse, thin and horizontal; they fade in, glide with the breeze (and the flow, on
// moving water) and fade out. `glint` is the line's glint weight where it tilts toward
// the sun (the normal leans the sun's way along z), else 0.
fn rippleNormal(w: vec2<f32>, flow: vec2<f32>, glint: ptr<function, f32>) -> vec3<f32> {
    let st = u.clock.y;
    let c1 = st / FLOW_CYCLE;
    let c2 = c1 + 0.5;
    // The step wraps at 2048 (`VoxelRenderer::set_clock`): 32 cycles.
    var s = rippleLine(w, flow, fract(c1), wrapI(i32(floor(c1)), 32), 60);
    if (s.x == 0.0) {
        s = rippleLine(w + vec2(0.37, 0.0), flow, fract(c2), wrapI(i32(floor(c2)), 32), 62);
    }
    *glint = 0.0;
    if (s.x == 0.0) { return vec3(0.0, 1.0, 0.0); }
    let n = normalize(vec3(0.0, 1.0, -s.x * u.waterL.z));
    if (n.z * u.sunK.z > 0.0) { *glint = s.y; }
    return n;
}

// --- the surface's details (W checkpoint 2) ------------------------------------------------
//
// Everything here moves on the water's clock (`u.clock.y`, every frame by default) and
// fades or moves continuously: nothing appears or vanishes between two frames.

fn hash01(x: i32, y: i32, z: i32, salt: i32) -> f32 {
    return f32(cellHash(x, y, z, salt) & 0xFFFFu) / 65535.0;
}

// Smooth value noise over the world's (x, z) plane at about `freq` lattice cells a voxel
// (a whole number of them round the ring, so it meets itself at the seam), on noise
// layer `layer`.
fn vnoise(w: vec2<f32>, freq: f32, layer: i32, salt: i32) -> f32 {
    let period = max(1, i32(f32(W) * freq + 0.5));
    let q = vec2(w.x * f32(period) / f32(W), w.y * freq);
    let i = floor(q);
    var f = q - i;
    f = f * f * (3.0 - 2.0 * f);
    let x0 = wrapI(i32(i.x), period);
    let x1 = wrapI(i32(i.x) + 1, period);
    let y0 = i32(i.y);
    let a = hash01(x0, y0, layer, salt);
    let b = hash01(x1, y0, layer, salt);
    let c = hash01(x0, y0 + 1, layer, salt);
    let d = hash01(x1, y0 + 1, layer, salt);
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

// The same noise evolving in time: `t` layers on from the first, cross-faded (smoothstep)
// from each layer to the next. Layers wrap at 256, so `t` must too.
fn vnoiseT(w: vec2<f32>, freq: f32, t: f32, salt: i32) -> f32 {
    let lt = floor(t);
    var ft = t - lt;
    ft = ft * ft * (3.0 - 2.0 * ft);
    let l0 = wrapI(i32(lt), 256);
    let l1 = wrapI(i32(lt) + 1, 256);
    return mix(vnoise(w, freq, l0, salt), vnoise(w, freq, l1, salt), ft);
}


// --- the sky (the lit tier's sky pass, `SKY`; package WX2) ----------------------------------
//
// Interim look. Everything moves on the every-frame clock (the drift and the twinkle clock
// come from `cubarium_gpu::weather`, eased there), so nothing pops.

// Stars (WX2 checkpoint 1b): at most one in each square of STAR_CELL voxels of the
// raster, STAR_SHARE of them. Brightness is a power-law draw (many faint, a few bright:
// the share brighter than b falls as b^-STAR_POWER), from STAR_MIN up to STAR_MAX; the
// colour a temperature from bluish white to warm, a rare few in the palette's cyan or
// magenta. One pixel, two from 10 px a voxel; the brightest also get a soft cross. Each
// twinkles smoothly on its own two rates and phases (whole periods in the twinkle
// clock's 3600 s wrap), deeper for the faint ones and near the horizon (`t`, 0 at the
// top of the raster). `.a` is how much of it is bright enough to feed the bloom.
const STAR_CELL: f32 = 1.0;
const STAR_SHARE: f32 = 0.16;
const STAR_GAIN: f32 = 0.9;
const STAR_MIN: f32 = 0.09;
const STAR_MAX: f32 = 3.0;
const STAR_POWER: f32 = 1.5;
// Above this brightness a star has a cross and feeds the bloom.
const STAR_BRIGHT: f32 = 0.9;

fn starAt(px: i32, py: i32, t: f32) -> vec4<f32> {
    let C = max(4, i32(f32(S) * STAR_CELL + 0.5));
    let cell = vec2(px / C, py / C);
    let h = cellHash(cell.x, cell.y, 0, 71);
    if (f32(h & 0xFFFFu) / 65536.0 >= STAR_SHARE) { return vec4(0.0); }
    let g = cellHash(cell.x, cell.y, 1, 72);
    let f = cellHash(cell.x, cell.y, 2, 73);
    // r in (0, 1]: the power law's draw.
    let r = (f32(f & 0xFFFFu) + 1.0) / 65536.0;
    let b = min(STAR_MIN * pow(r, -1.0 / STAR_POWER), STAR_MAX);
    let sz = select(1, 2, S >= 10);
    // Kept a pixel in from the cell's edge, so a cross stays inside it.
    let span = max(C - sz - 1, 1);
    let spot = vec2(1 + i32(g % u32(span)), 1 + i32((g >> 8u) % u32(span)));
    let d = vec2(px, py) - cell * C - spot;
    var k: f32;
    if (d.x >= 0 && d.y >= 0 && d.x < sz && d.y < sz) {
        k = 1.0;
    } else if (b > STAR_BRIGHT && (((d.x == -1 || d.x == sz) && d.y >= 0 && d.y < sz)
                                || ((d.y == -1 || d.y == sz) && d.x >= 0 && d.x < sz))) {
        k = 0.22 * min((b - STAR_BRIGHT) / STAR_BRIGHT, 1.0);
    } else {
        return vec4(0.0);
    }
    // Twinkle: two sines, each a whole number of periods in the 3600 s wrap (0.2-0.9 Hz
    // and 0.07-0.3 Hz), deeper for a faint star and toward the horizon.
    let w1 = 6.28318530718 * f32(720u + (g >> 16u) % 2520u) / 3600.0;
    let w2 = 6.28318530718 * f32(250u + (f >> 16u) % 830u) / 3600.0;
    let ph1 = f32(h >> 16u) * 0.0000958738;
    let ph2 = f32(g >> 24u) * 0.0245436926;
    let wave = 0.5 + 0.5 * (0.65 * sin(u.cloudK.w * w1 + ph1) + 0.35 * sin(u.cloudK.w * w2 + ph2));
    let faint = 1.0 - smoothstep(STAR_MIN, STAR_BRIGHT, b);
    let depth = clamp(mix(0.18, 0.55, faint) * (1.0 + 1.2 * smoothstep(0.1, 0.5, t)), 0.0, 0.9);
    let tw = 1.0 - depth * wave;
    // Colour: a temperature, and a rare palette tint.
    let temp = f32((f >> 16u) & 0xFFu) / 255.0;
    var c = select(mix(vec3(1.0), vec3(1.0, 0.82, 0.62), (temp - 0.55) / 0.45),
                   mix(vec3(0.72, 0.84, 1.0), vec3(1.0), temp / 0.55), temp < 0.55);
    let rare = (f >> 24u) % 16u;
    if (rare == 0u) { c = mix(c, u.lightC.rgb / max(max(u.lightC.r, u.lightC.g), max(u.lightC.b, 1e-3)), 0.5); }
    if (rare == 1u) { c = mix(c, u.bandB.rgb / max(max(u.bandB.r, u.bandB.g), max(u.bandB.b, 1e-3)), 0.4); }
    let lum = b * tw * k;
    return vec4(c * lum, k * max(b - STAR_BRIGHT, 0.0) / (STAR_MAX - STAR_BRIGHT));
}

// The cloud layer's noise: value-noise fbm over the raster in voxel units, x round the ring
// (so it meets itself at the seam), y stretched CLOUD_FLAT times so clouds are wide and
// flat, domain-warped by a slower field that drifts at half the clouds' speed (so shapes
// change as they go).
const CLOUD_FREQ: f32 = 1.0 / 36.0;
const CLOUD_FLAT: f32 = 3.0;
const CLOUD_WARP: f32 = 20.0;

fn cloudFbm(q: vec2<f32>, octaves: i32, salt: i32) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var norm = 0.0;
    var freq = CLOUD_FREQ;
    for (var i = 0; i < octaves; i++) {
        sum += amp * vnoise(q, freq, i, salt);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    return sum / norm;
}

fn cloudNoise(q: vec2<f32>) -> f32 {
    let wq = q - vec2(0.5 * u.cloudK.y, 0.0);
    let warp = vec2(cloudFbm(wq, 3, 120), cloudFbm(wq + vec2(64.0, 23.0), 3, 121)) - 0.5;
    return cloudFbm(q + warp * CLOUD_WARP, 5, 122);
}

// The day sky (WX2 checkpoint 1b), `t` of the way down the raster: zenith, middle, and the
// lightest at the horizon, which sits about where the far ridge does (the twilight band's
// own reach).
const DAY_HORIZON_T: f32 = 0.55;

fn daySkyAt(t: f32) -> vec3<f32> {
    let g = smoothstep(0.0, DAY_HORIZON_T, t);
    let c = mix(u.dayA.rgb, u.dayB.rgb, smoothstep(0.0, 0.55, g));
    return mix(c, u.dayC.rgb, smoothstep(0.45, 1.0, g));
}

// The lit sky: the gradient (the day sky's over the palette's by `dayA.w`), the twilight
// band, the stars, the clouds. `.a` is the light the pixel sends the bloom (a bright
// star's, times `dayB.w`).
fn litSky(px: i32, rasterY: i32) -> vec4<f32> {
    let py = rasterY + u.camera.x;
    let t = clamp(f32(py) / f32(max(u.camera.y, 1)), 0.0, 1.0);
    let gradient = u.roofK.w > 0.5;
    var sky = select(u.skyC.rgb, mix(u.skyC.rgb, u.skyHorizon.rgb, t), gradient);
    let day = u.dayA.w;
    if (day > 0.0) {
        // Coming in (and going out) through the twilight band's own colours, so the sky
        // brightens rose and orange into the day's rather than through a grey: a linear
        // mix of near-black and a pale colour is a grey.
        let b = smoothstep(0.04, 0.56, t);
        let warm = mix(mix(u.bandA.rgb, u.bandB.rgb, smoothstep(0.0, 0.5, b)), u.bandC.rgb,
                       smoothstep(0.5, 1.0, b));
        sky = mix(sky, mix(warm, daySkyAt(t), smoothstep(0.0, 1.0, day)), day);
    }
    if (gradient && u.dayK.w > 0.0) {
        // The band, a little brighter on the sun's side.
        let xs = f32(px) / f32(max(W * S, 1)) * 2.0 - 1.0;
        sky += bandAt(t) * (1.0 + 0.4 * u.sunLean.w * xs);
    }
    var glow = 0.0;
    if (u.cloudBody.w > 0.001) {
        let st = starAt(px, py, t) * (u.cloudBody.w * (1.0 - smoothstep(0.25, 0.6, t)));
        sky += st.rgb * STAR_GAIN;
        glow = st.a * u.dayB.w;
    }
    // A lightning flash lifts the sky a little everywhere, and lights the clouds from
    // within (below), most over the strike.
    let flash = u.flashC.w;
    var nearStrike = 0.0;
    if (flash > 0.0) {
        let ringPx = f32(W * S);
        var dx = abs(f32(px) + 0.5 - u.strikeK.x * f32(S));
        dx = min(dx, ringPx - dx) / (28.0 * f32(S));
        nearStrike = exp(-dx * dx);
        sky += u.flashC.rgb * (0.04 + 0.06 * nearStrike);
    }
    let cover = u.cloudK.x;
    if (cover < 0.001) { return vec4(sky, glow); }
    let storm = u.cloudK.z;
    // The layer: down from the top of the sky, thinning toward the horizon; a storm brings
    // it lower.
    let env = 1.0 - smoothstep(mix(0.22, 0.5, storm), mix(0.52, 0.8, storm), t);
    if (env <= 0.0) { return vec4(sky, glow); }
    let q = vec2((f32(px) + 0.5) / f32(S) + u.cloudK.y,
                 (f32(py) + 0.5) / f32(S) * CLOUD_FLAT);
    let n = cloudNoise(q);
    let thr = mix(0.74, 0.36, cover) - 0.16 * storm;
    let d = smoothstep(thr, thr + 0.08, n) * env * smoothstep(0.0, 0.2, cover);
    if (d <= 0.0) { return vec4(sky, glow); }
    // Lit where the cloud thins toward the sun: the sun is on the camera's side, so on the
    // screen its light comes from above and from its own side.
    let toSun = normalize(vec2(u.sunK.x, -max(u.sunK.y, 0.25) * CLOUD_FLAT));
    let n2 = cloudNoise(q + toSun * 2.0);
    let edge = clamp((n - n2) * 10.0 + 0.2, 0.0, 1.0);
    let lit = edge * u.cloudLit.w;
    // Thick cores and the shaded side darker.
    let body = u.cloudBody.rgb * (1.15 - 0.45 * d * (1.0 - lit)) * (1.0 - 0.5 * storm);
    var col = body + u.cloudLit.rgb * lit;
    if (day > 0.0) {
        // By day: near white where the sun reaches and through the body, violet on the
        // shadowed underside (where the cloud thins away from the sun); heavy cover and a
        // storm push toward the shade.
        // A storm's clouds are mostly underside, and dark (checkpoint 2).
        let sunward = clamp(edge * 2.0 + 0.3 - 0.4 * cover * cover, 0.0, 1.0) * (1.0 - 0.7 * storm);
        let dayCol = mix(u.cloudShade.rgb, u.cloudDay.rgb, sunward) * (1.0 - 0.72 * storm);
        col = mix(col, dayCol, day);
    }
    if (flash > 0.0) {
        // Lit from within: thick cores glow most, the strike's own clouds far more.
        col += u.flashC.rgb * (0.12 + 0.5 * nearStrike) * (0.4 + 0.6 * d);
    }
    return vec4(mix(sky, col, d * 0.92), glow * (1.0 - d));
}

// Foam: a band about FOAM_WIDTH voxels wide along a shore (breathing between FOAM_BREATHE
// and 1 + FOAM_BREATHE of that), and a disc PLUNGE_RADIUS voxels round where a fall meets
// the pool. Its noise moves a layer every FOAM_STEPS animation steps (PLUNGE_STEPS where a
// fall churns it); both divide the step's wrap of 2048 into a whole multiple of 256 layers.
const FOAM_WIDTH: f32 = 0.4;
const FOAM_BREATHE: f32 = 0.45;
const FOAM_STEPS: f32 = 8.0;
const PLUNGE_RADIUS: f32 = 1.4;
const PLUNGE_STEPS: f32 = 4.0;

// The distance (voxels, in x and z) from surface point `p` in water cell `c` to the
// nearest solid beside it at the surface's level (and the level over it for a brim-full
// cell), or a large number for none: the four sides by the face between, the four corners
// by the corner point.
fn shoreDist(c: vec3<i32>, p: vec3<f32>, brim: bool) -> f32 {
    let fx = p.x - f32(c.x);
    let fz = p.z - f32(c.z);
    var d = 9.0;
    for (var dz = -1; dz <= 1; dz++) {
        for (var dx = -1; dx <= 1; dx++) {
            if (dx == 0 && dz == 0) { continue; }
            let nx = c.x + dx;
            let nz = c.z + dz;
            if (nz < 0 || nz >= D) { continue; }
            if (!solidAt(nx, c.y, nz) && !(brim && solidAt(nx, c.y + 1, nz))) { continue; }
            let ex = select(select(0.0, 1.0 - fx, dx > 0), fx, dx < 0);
            let ez = select(select(0.0, 1.0 - fz, dz > 0), fz, dz < 0);
            d = min(d, length(vec2(ex, ez)));
        }
    }
    return d;
}

// The distance (voxels, in x and z) from surface point `p` to the nearest fall landing
// on the pool around column (x, z) of cell `c`: a water cell at level `yp` over a cell
// that is neither solid nor full (`fallingAt`), measured to its column's centre; a large
// number for none.
fn plungeDist(c: vec3<i32>, p: vec3<f32>, yp: i32) -> f32 {
    var d = 9.0;
    for (var dz = -1; dz <= 1; dz++) {
        for (var dx = -1; dx <= 1; dx++) {
            let nx = c.x + dx;
            let nz = c.z + dz;
            if (nz < 0 || nz >= D || freeQ(nx, yp, nz) == 0 || !fallingAt(nx, yp, nz)) {
                continue;
            }
            d = min(d, length(p.xz - vec2(f32(nx) + 0.5, f32(nz) + 0.5)));
        }
    }
    return d;
}

// The foam's cover (0 to 1) at surface point `p` of water cell `c`, `fill` pixels full.
fn foamAt(c: vec3<i32>, p: vec3<f32>, fill: i32) -> f32 {
    let brim = fill >= S;
    let st = u.clock.y;
    var cover = 0.0;
    let ds = shoreDist(c, p, brim);
    if (ds < FOAM_WIDTH * (1.0 + FOAM_BREATHE)) {
        let t = st / FOAM_STEPS;
        let w = FOAM_WIDTH * (FOAM_BREATHE + vnoiseT(p.xz, 1.0, t, 80));
        let lace = smoothstep(0.3, 0.7, vnoiseT(p.xz, 3.0, t, 81));
        cover = (1.0 - smoothstep(0.35 * w, w, ds)) * mix(0.45, 1.0, lace);
    }
    let dp = plungeDist(c, p, select(c.y, c.y + 1, brim) + 1);
    if (dp < PLUNGE_RADIUS) {
        let churn = vnoiseT(p.xz, 3.0, st / PLUNGE_STEPS, 82);
        cover = max(cover, (1.0 - smoothstep(0.25, PLUNGE_RADIUS, dp)) * mix(0.5, 1.0, churn));
    }
    return cover;
}

// Rain rings: the surface is cut into cells about RAIN_CELL voxels on a side, and in each
// time slot of RAIN_SLOT animation steps a cell holds a drop with the chance `rainK.w` (the rain rate's), at a
// random place and moment in the slot. Its ring grows to RING_RADIUS voxels over RING_LIFE
// of the slot, easing out, fading in fast and out slowly. Two copies, offset half a cell
// and half a slot, overlap. A ring stays inside its cell, so a pixel asks only its own.
const RAIN_CELL: f32 = 2.0;
const RAIN_SLOT: f32 = 8.0;
const RING_RADIUS: f32 = 0.6;
const RING_LIFE: f32 = 0.7;

fn rainRing(w: vec2<f32>, copy: i32) -> f32 {
    let tt = u.clock.y / RAIN_SLOT + 0.5 * f32(copy);
    // The step wraps at 2048: 256 slots.
    let slot = wrapI(i32(floor(tt)), 256);
    let phase = fract(tt);
    let cellsX = max(1, i32(f32(W) / RAIN_CELL + 0.5));
    let cellW = f32(W) / f32(cellsX);
    let g = vec2(w.x / cellW, w.y / RAIN_CELL) + 0.5 * f32(copy);
    let cell = vec2<i32>(floor(g));
    let h = cellHash(wrapI(cell.x, cellsX), cell.y, slot, 90 + copy);
    // The density follows the rain rate (`rainK.w`: W-2's 0.55 in a shower, more in a
    // downpour, few in a drizzle). A drop whose draw is near the density is faint, so as the
    // rate eases a ring fades rather than appearing or vanishing mid-life.
    let chance = min(u.rainK.w, 0.98);
    let hv = f32(h & 0xFFFFu) / 65535.0;
    if (hv >= chance) { return 0.0; }
    let margin = clamp((chance - hv) / 0.06, 0.0, 1.0);
    let age = (phase - f32((h >> 16u) & 0xFFu) / 255.0 * (1.0 - RING_LIFE)) / RING_LIFE;
    if (age <= 0.0 || age >= 1.0) { return 0.0; }
    // The drop's place in its cell, far enough in that its ring stays there.
    let m = vec2(RING_RADIUS / cellW, RING_RADIUS / RAIN_CELL);
    let spot = vec2(f32((h >> 24u) & 0xFu), f32((h >> 28u) & 0xFu)) / 15.0;
    let centre = m + spot * (1.0 - 2.0 * m);
    // Voxels from the drop, and the ring's distance in raster pixels (a voxel is S pixels
    // across and RISE rows deep), so it is as thin at its top as at its sides.
    let e = (g - floor(g) - centre) * vec2(cellW, RAIN_CELL);
    let l = length(e);
    let r = RING_RADIUS * (1.0 - (1.0 - age) * (1.0 - age));
    let grad = select(vec2(1.0 / f32(S), 0.0), e / l / vec2(f32(S), f32(RISE)), l > 1e-4);
    let px = abs(l - r) / max(length(grad), 1e-6);
    let edge = max(1.0 - px / 0.8, 0.0);
    let fade = smoothstep(0.0, 0.12, age) * (1.0 - age) * (1.0 - age);
    return edge * fade * margin;
}

// A ceiling seen in a reflection: the underside of a solid, which the picture never
// draws; the ambient ladder's floor rung and whatever local light reaches it.
fn undersideLit(c: vec3<i32>, v: vec4<u32>, hp: vec3<f32>) -> vec3<f32> {
    let body = blockBody(v, strataOf(matOf(v)));
    return hazed(body * ladderLight(0.0, 0.0, glowAt(hp - vec3(0.0, 0.5, 0.0))),
                 hazeAt(f32(c.z)));
}

// The front-face texel (r rows up from its bottom, dx across) a reflected ray entering
// cell `c` through `axis` at `hp` lands on: a side face (axis 0) is drawn as the front's
// edge column on that side, an underside (axis 1) as its bottom row.
fn reflectTexel(hp: vec3<f32>, c: vec3<i32>, axis: i32, dirX: f32) -> vec2<i32> {
    var dx = clamp(i32(floor(fract(hp.x) * f32(S))), 0, S - 1);
    var dy = clamp(i32(floor((f32(c.y + 1) - hp.y) * f32(S))), 0, S - 1);
    if (axis == 0) { dx = select(S - 1, 0, dirX > 0.0); }
    if (axis == 1) { dy = S - 1; }
    return vec2(S - 1 - dy, dx);
}

fn glyphFilled(base: i32, col: i32, row: i32) -> bool {
    if (col < 0 || col >= S || row < 0 || row >= S) { return true; }
    return (textureLoad(glyphTex, vec2(col, base + row), 0).r >> 2u) != 63u;
}

// Whether a reflected ray stops at a block part's face texel: the texel and its four
// neighbours on the face all drawn (past the face's edge counts as drawn, so a body of
// several cells keeps its edges). Thin parts (stems, sprigs, a berry's pixel) are passed
// through: in a rippled mirror they are specks that blink.
fn glyphTexelAt(v: vec4<u32>, hp: vec3<f32>, c: vec3<i32>, axis: i32, dirX: f32) -> bool {
    let t = reflectTexel(hp, c, axis, dirX);
    let base = (partOf(v) * 8 + glyphOf(v)) * (S + RISE);
    let row = S - 1 - t.x;
    let col = t.y;
    return glyphFilled(base, col, row) && glyphFilled(base, col - 1, row)
        && glyphFilled(base, col + 1, row) && glyphFilled(base, col, row - 1)
        && glyphFilled(base, col, row + 1);
}

// What the surface at `p` mirrors along `r`: a DDA through the volume, at most
// `reflect_cells` cells, to the first terrain or block part (`hit`, entered through
// `axis` at `hp`); false when the ray leaves the world or is still going at the cap
// (`hp` is then where it went). The march only finds the hit; `reflectionShade` shades
// it once the march's own state is dead, which keeps this pass's register count (and so
// the whole shader's occupancy) near the walk's own.
fn reflectMarch(p: vec3<f32>, r: vec3<f32>, hit: ptr<function, vec3<i32>>,
                hp: ptr<function, vec3<f32>>, axis: ptr<function, i32>) -> bool {
    var c = vec3<i32>(floor(p + r * 1e-3));
    let dir = vec3<i32>(sign(r));
    let inv = vec3(
        select(1e30, 1.0 / abs(r.x), abs(r.x) > 1e-6),
        select(1e30, 1.0 / abs(r.y), abs(r.y) > 1e-6),
        select(1e30, 1.0 / abs(r.z), abs(r.z) > 1e-6));
    var next = vec3(
        select(p.x - f32(c.x), f32(c.x + 1) - p.x, dir.x > 0) * inv.x,
        select(p.y - f32(c.y), f32(c.y + 1) - p.y, dir.y > 0) * inv.y,
        select(p.z - f32(c.z), f32(c.z + 1) - p.z, dir.z > 0) * inv.z);
    let cap = i32(u.waterL.w);
    var tIn = 0.0;
    *axis = -1;
    *hit = c;
    for (var i = 0; i < cap; i++) {
        if (c.y >= H || c.y < 0 || c.z < 0 || c.z >= D) { break; }
        if (i > 0) {
            let v = at(c.x, c.y, c.z);
            // A block part counts where its glyph has a texel (the atlas's own holes let
            // the ray on); a texture's leaf cutout does not (`reflectionShade`).
            if (solidV(v) || (isBlockPart(partOf(v)) && glyphTexelAt(v, p + r * tIn, c, *axis, r.x))) {
                *hit = c;
                *hp = p + r * tIn;
                return true;
            }
        }
        *axis = select(select(2, 1, next.y < next.z), 0, next.x < next.y && next.x < next.z);
        if (*axis == 0) { tIn = next.x; c.x += dir.x; next.x += inv.x; }
        else if (*axis == 1) { tIn = next.y; c.y += dir.y; next.y += inv.y; }
        else { tIn = next.z; c.z += dir.z; next.z += inv.z; }
    }
    *hp = p + r * tIn;
    return false;
}

// The reflection's colour: the hit's face under L's light (`shadeFront`: the ladder, AO,
// the sun and its shadows, the glow), from its base colour (terrain: the material or its
// texture, wet; a block part: its glyph texel's pigment or texture, and an emitter at
// full value), without the picture's edge treatments (rims, bevels, tones), which a
// ripple-broken mirror image at 40 % would not show and which cost the whole shader
// occupancy to inline a second time; a solid's underside at the ladder's floor; or the
// sky gradient at the raster row where the ray left.
fn reflectionShade(found: bool, c: vec3<i32>, hp: vec3<f32>, axis: i32, r: vec3<f32>) -> vec3<f32> {
    if (!found) {
        let row = i32(floor(f32(BASE) - hp.y * f32(S) - hp.z * f32(RISE)));
        let col = wrapI(i32(floor(hp.x * f32(S))), max(W * S, 1));
        return textureLoad(backdropTex, vec2(col, clamp(row, 0, u.extent.w - 1)), 0).rgb;
    }
    let v = at(c.x, c.y, c.z);
    let t = reflectTexel(hp, c, axis, r.x);
    let dy = S - 1 - t.x;
    let solid = solidV(v);
    if (solid && axis == 1) { return undersideLit(c, v, hp); }
    var base: vec3<f32>;
    var canopy: f32;
    if (solid) {
        let m = matOf(v);
        base = blockBody(v, faceBase(m, 2 * (m - 1), c.x, c.y, c.z, 0, t.y, dy));
        canopy = canopyAt(c.x, c.z - 1, c.y);
    } else {
        let atlasY = (partOf(v) * 8 + glyphOf(v)) * (S + RISE) + dy;
        let q = textureLoad(glyphTex, vec2(t.y, atlasY), 0).r;
        let pigment = textureLoad(styleTex, vec2(i32(q & 3u), i32(v.a)), 0);
        var e = styleEmit(i32(v.a));
        let emits = i32(q >> 2u) == TONE_EMIT || emitsWhole(pigment.a);
        var on = 1.0;
        if (emits) { cellEmit(c.x, c.y, c.z, &e, &on); }
        if (emits && any(e > vec3(0.0))) {
            return hazed(e, hazeAt(f32(c.z)));
        }
        base = pigment.rgb;
        // A texture's leaf hole: the cell's crown pigment.
        if (!plantTexel(c.x, c.y, c.z, v, false, t.y, dy, &base)) { base = styleAt(i32(v.a), 1); }
        if (partOf(v) == ANIMAL_INTERIM) {
            canopy = canopyAt(c.x, c.z - 1, c.y);
        } else {
            canopy = plantCanopy(v);
        }
    }
    return hazed(shadeFront(base, c.x, c.y, c.z, t.y, dy, canopy), hazeAt(f32(c.z)));
}

// A falling-water streak at raster pixel (px, py): a third of the pixel columns carry
// one, S/3 rows long (at least 2) once every 16 rows (32 from 8 px a voxel up), moving
// down S/4 rows (at least 1) an animation step. The periods divide the step's wrap.
fn fallStreak(px: i32, py: i32, st: f32) -> bool {
    let h = cellHash(wrapI(px, W * S), 0, 0, 50);
    if (h % 3u != 0u) { return false; }
    let period = select(16, 32, S >= 8);
    let len = max(2, S / 3) + i32((h >> 8u) & 1u);
    let speed = max(1, S / 4);
    let ph = wrapI(py - i32(floor(st * f32(speed))) + i32(h >> 16u), period);
    return ph < len;
}

// The capture-only flow overlay (`VoxelParams::debug_flow`): still water dark blue,
// moving water hued by its direction (+x red, +z green, -x cyan, -z violet) and brighter
// with speed, falling water white-on-violet stripes; from 8 px a voxel, a white line (its
// head yellow) from each top face's centre along its flow.
fn flowDebug(fl: vec3<f32>, top: bool, p: vec3<f32>) -> vec3<f32> {
    if (fl.z > 0.5) {
        return select(vec3(0.35, 0.05, 0.5), vec3(1.0), fallStreak(PX, PY, u.clock.y));
    }
    let q = length(fl.xy);
    if (q < 0.01) { return vec3(0.01, 0.02, 0.2); }
    let a = atan2(fl.y, fl.x);
    let hue = 0.5 + 0.5 * vec3(cos(a), cos(a - 2.0944), cos(a + 2.0944));
    var c = hue * (0.25 + 0.75 * q);
    if (top && S >= 8) {
        let lp = vec2(fract(p.x) * f32(S), (1.0 - fract(p.z)) * f32(RISE));
        let o = vec2(0.5 * f32(S), 0.5 * f32(RISE));
        let d = normalize(vec2(fl.x * f32(S), -fl.y * f32(RISE)));
        let e = o + d * 0.45 * f32(S) * vec2(1.0, f32(RISE) / f32(S));
        let pa = lp - o;
        let ba = e - o;
        let t = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-4), 0.0, 1.0);
        if (length(pa - ba * t) < 0.75) { c = select(vec3(1.0), vec3(1.0, 1.0, 0.3), t > 0.6); }
    }
    return c;
}

// The lit water, put together once the walk is over: `walked` is what the walk made of
// the pixel with the water's absorption in it and its in-scatter still out. The entry is
// recomputed from `wEntry`: its cell, whether it is the surface, the point where the ray
// entered, and the open cell whose light it reads.
fn litWaterFinish(walked: vec3<f32>) -> vec3<f32> {
    var colour = walked;
    if ((wEntry & ENTRY_FLAT) != 0) {
        colour += wScatter * u.dayL.x + wHaze * u.hazeC.rgb;
        if ((wEntry & ENTRY_FALLING) == 0) { return colour; }
        if (u.clock.z > 0.5) {
            return select(vec3(0.35, 0.05, 0.5), vec3(1.0), fallStreak(PX, PY, u.clock.y));
        }
        // A fall's streaks over the flat tier's sheet: the surface colour itself (opaque,
        // hazed), a third of the columns, moving down. The palette has nothing brighter,
        // and the sheet between them keeps the flat tier's brightness.
        let zf = (wEntry >> 12u) & 0xFFF;
        if (fallStreak(PX, PY, u.clock.y)) {
            return hazed(u.waterSurfaceC.rgb * u.dayL.x, hazeAt(f32(zf)));
        }
        return colour;
    }
    let y = wEntry & 0xFFF;
    let z = (wEntry >> 12u) & 0xFFF;
    let top = (wEntry >> 24u) != 0;
    let x = PX / S;
    let fill = fillPxAt(x, y, z);
    let k = f32(RISE) / f32(S);
    let yf = (f32(BASE - z * RISE - PY - 1) + 0.5) / f32(S);
    let xw = (f32(PX) + 0.5) / f32(S);
    let ys = f32(y) + f32(fill) / f32(S);
    let p = select(vec3(xw, yf, f32(z)), vec3(xw, ys, f32(z) + clamp((yf - ys) / k, 0.0, 1.0)), top);
    let open = select(vec3(x, y, z - 1), vec3(x, select(y + 1, y, fill < S), z), top);
    var canopy: f32;
    if (top) {
        canopy = canopyAt(x, z, y);
    } else {
        canopy = canopyAt(x, z - 1, y);
    }

    let fl = flowAt(vec3(x, y, z));
    if (u.clock.z > 0.5) { return flowDebug(fl, top, p); }
    // The ladder's light at the entry, as a ratio to open sunlit sky.
    let sunOn = select(0.0, 1.0, u.sunK.y > 0.0);
    let ndl = select(-u.sunK.z, u.sunK.y, top);
    var sun = 0.0;
    if (sunOn > 0.0 && ndl > 0.0) { sun = sunReaches(p, open); }
    let glow = glowAt(select(p - vec3(0.0, 0.0, 0.5), p + vec3(0.0, 0.5, 0.0), top));
    let here = ladderLight(skyOpen(open.x, open.y, open.z) * canopy, sun, glow);
    let ratio = here / max(ladderRef(sunOn), vec3(1e-4));
    colour += wScatter * ratio + wHaze * u.hazeC.rgb;
    if (!top) { return colour; }
    var glint = 0.0;
    let n = rippleNormal(p.xz, fl.xy, &glint);
    let d = normalize(vec3(0.0, -k, 1.0));
    let cosi = clamp(-dot(d, n), 0.0, 1.0);
    let f = clamp(u.waterL.y * (0.02 + 0.98 * pow(1.0 - cosi, 5.0)), 0.0, 1.0);
    var r = reflect(d, n);
    r.y = max(r.y, 0.05);
    r = normalize(r);
    // What is behind the surface first, so the march and the shading carry little state.
    if (REFLECTIONS) {
        let under = (1.0 - f) * colour;
        var hc = vec3(0);
        var hp = vec3(0.0);
        var axis = 0;
        let found = reflectMarch(p, r, &hc, &hp, &axis);
        colour = under + f * reflectionShade(found, hc, hp, axis, r);
    }
    // The surface's details over the reflection: rain rings and foam in the palette's
    // surface colour, off the light as the falls' streaks are (lit, foam in a shaded
    // crevice went a muddy grey), and glints in the palette's light. Foam and rings only
    // lighten: where the water is already brighter they leave it be. Rings and glints lean
    // from their palette colour toward white by the highlight knob (waterM.w).
    let hz = hazeAt(f32(z) + 0.5);
    let surf = u.waterSurfaceC.rgb * u.dayL.x;
    let froth = max(colour, hazed(surf, hz));
    if (u.waterM.z > 0.0 && u.rainK.w > 0.0 && skyOpen(open.x, open.y, open.z) > 0.0) {
        let ring = max(rainRing(p.xz, 0), rainRing(p.xz, 1));
        let ringC = max(colour, hazed(mix(surf, vec3(u.dayL.x), u.waterM.w), hz));
        colour = mix(colour, ringC, clamp(u.waterM.z * ring, 0.0, 1.0));
    }
    if (u.waterM.x > 0.0) {
        let foam = foamAt(vec3(x, y, z), p, fill);
        colour = mix(colour, froth, clamp(u.waterM.x * foam, 0.0, 1.0));
    }
    if (u.waterM.y > 0.0 && glint > 0.0 && sun > 0.0) {
        let glintC = hazed(mix(u.sunLean.rgb, vec3(1.0), u.waterM.w), hz);
        colour = mix(colour, glintC, clamp(u.waterM.y * glint * u.dayL.y, 0.0, 1.0));
    }
    return colour;
}


// --- the weather over the world (WX2 checkpoint 2) -----------------------------------------
//
// Interim look. Rain, fog and lightning, drawn by the lit tier's weather pass (`WEATHER`)
// over the raster, with the walk's end depth from `depthTex`; the flat tier's rain and
// flash are the cheap stand-ins in the walk itself (`flatRain`, the tint). Everything moves
// on the every-frame clock (`cloudK.w`, sim seconds wrapped at 3600 with every period here
// whole in it) and eases in and out through the look's weights, so nothing pops.

// The weather clock, seconds.
fn wxTime() -> f32 { return u.cloudK.w; }

// A speed (units a second) nudged so that it moves a whole number of `period`s in the
// clock's 3600 s wrap: the pattern is the same on both sides of the wrap.
fn wholeIn3600(speed: f32, period: f32) -> f32 {
    return max(1.0, floor(speed * 3600.0 / period + 0.5)) * period / 3600.0;
}

// --- fog ---
//
// Height fog over the low ground: full density in a band `fogK.z` voxels deep over its
// floor (`fogK.y`, the sink's low ground: basins and the lake), thinning by e every
// `fogK.w` voxels above. A drifting noise over the ground plane thickens and thins it, one
// value per pixel (where its ray reaches the fog), so it drifts in banks without costing a
// march.

// The fog's shape at height y: 1 in its band, falling off above.
fn fogShape(y: f32) -> f32 {
    let h = y - (u.fogK.y + u.fogK.z);
    return select(exp(-h / max(u.fogK.w, 1e-3)), 1.0, h <= 0.0);
}

// The noise's layers go by at this rate (a whole 256 of them in the 3600 s wrap), and it
// drifts with the clouds.
const FOG_EVOLVE: f32 = 256.0 / 3600.0;
const FOG_FREQ: f32 = 1.0 / 9.0;

// The banks' size for the breathing (FOG-MOTION): about 32 voxels across.
const FOG_BANK_FREQ: f32 = 1.0 / 32.0;

// The drifting density at ground point (x, z): 0.35 to 1.65, about 1, times the banks'
// breath when the fog breathes (`fogM`, FOG-MOTION's trial):
//   0 fixed: the pattern alone;
//   1 breathing: two still noise fields over the ground, turned through each other on the
//     breath's period (a cos + b sin), so each bank swells and thins in place on its own
//     phase and the whole never pulses together;
//   2 flowing: that, the banks and the pattern in them carried along +x at the flow;
//   3 deep: flowing, drawn by `fogDepthDeep` (candidate d).
fn fogNoise(x: f32, z: f32) -> f32 {
    let mode = i32(u.fogM.x + 0.5);
    var flow = 0.0;
    if (mode >= 2) {
        // GLSL's mod: what is left of the drift after whole turns of the ring.
        let drift = wxTime() * wholeIn3600(u.fogM.w, f32(W));
        flow = drift - floor(drift / f32(W)) * f32(W);
    }
    let w = vec2(x - u.cloudK.y - flow, z * 1.6);
    let n = 0.65 * vnoiseT(w, FOG_FREQ, wxTime() * FOG_EVOLVE, 140)
          + 0.35 * vnoise(w, FOG_FREQ * 2.3, 7, 141);
    var d = 0.35 + 1.3 * n;
    if (mode >= 1) {
        let wb = vec2(x - flow, z * 1.6);
        let a = 2.0 * vnoise(wb, FOG_BANK_FREQ, 11, 142) - 1.0;
        let b = 2.0 * vnoise(wb, FOG_BANK_FREQ, 12, 143) - 1.0;
        // Whole periods in the clock's 3600 s wrap.
        let per = 3600.0 / max(1.0, floor(3600.0 / max(u.fogM.z, 1.0) + 0.5));
        let ph = 6.2831853 * wxTime() / per;
        d *= max(0.0, 1.0 + u.fogM.y * (a * cos(ph) + b * sin(ph)));
    }
    return d;
}

// The slab the pixel's ray reaches the fog's band in, and the slabs of band it crosses up
// to `zEnd`: the ray falls k voxels a slab from height y0 at z = 0.
fn fogSpan(y0: f32, k: f32, zEnd: f32) -> vec2<f32> {
    let top = u.fogK.y + u.fogK.z;
    let zb = clamp((y0 - top) / k, 0.0, zEnd);
    return vec2(zb, zEnd - zb);
}

// The fog's optical depth along the ray from z = 0 to `zEnd` slabs, at the pixel's noise:
// the band part, plus the exponential tail above it, integrated in closed form.
fn fogDepth(y0: f32, k: f32, zEnd: f32, noise: f32) -> f32 {
    let span = fogSpan(y0, k, zEnd);
    let top = u.fogK.y + u.fogK.z;
    let fw = max(u.fogK.w, 1e-3);
    var above = 0.0;
    if (y0 > top) {
        above = (fw / k) * (exp(-(y0 - k * span.x - top) / fw) - exp(-(y0 - top) / fw));
    }
    return u.fogK.x * noise * (above + span.y) * sqrt(1.0 + k * k);
}

// The pixel's fog noise, where its ray first reaches the band (the middle of what it
// crosses there).
fn fogNoiseFor(xw: f32, y0: f32, k: f32, zEnd: f32) -> f32 {
    let span = fogSpan(y0, k, zEnd);
    return fogNoise(xw, span.x + 0.5 * span.y);
}

// FOG-MOTION candidate d (`fogM.x` 3, "deep": flowing, and less of a flat sheet): the
// band's top billows with the noise (FOG_BILLOW voxels for each unit of it off 1, found in
// one refinement: the flat top's crossing, then the billowed top there), and the density
// is taken at two depths along the ray, where it reaches the band and where it ends, each
// for half of the band it crosses. Its optical depth, as `fogDepth`'s.
const FOG_BILLOW: f32 = 3.0;
fn fogDepthDeep(xw: f32, y0: f32, k: f32, zEnd: f32) -> f32 {
    let base = u.fogK.y + u.fogK.z;
    let nb = fogNoise(xw, clamp((y0 - base) / k, 0.0, zEnd));
    let top = base + FOG_BILLOW * (nb - 1.0);
    let zb = clamp((y0 - top) / k, 0.0, zEnd);
    let ne = fogNoise(xw, zEnd);
    let band = zEnd - zb;
    let fw = max(u.fogK.w, 1e-3);
    var above = 0.0;
    if (y0 > top) {
        above = (fw / k) * (exp(-(y0 - k * zb - top) / fw) - exp(-(y0 - top) / fw));
    }
    return u.fogK.x * (nb * (above + 0.5 * band) + ne * 0.5 * band) * sqrt(1.0 + k * k);
}

// --- rain ---
//
// Three looks, one per mode, cross-faded by the look's weights (`rainK.xyz`):
//   drizzle: fine, short and slow, sparse, with a thin haze;
//   shower: streaks;
//   downpour: long, dense, slanted, in gusting sheets, with a heavier haze.
// Each falls in three layers at fixed depths (RAIN_DEPTH shares of the world's depth),
// nearer layers a little faster and longer (the orthographic camera's only parallax). A
// layer shows at a pixel only where the walk ended behind it and its point on the ray is
// open to the sky (no solid over it in its column): rain stops at a roof and behind nearer
// terrain. Streaks are one-pixel lines (antialiased, so they move smoothly), in lanes of
// the raster sheared by the slant; each lane has its own speed and offset, and each streak
// cell along a lane its own draw, so the pattern never repeats in view.

const RAIN_DEPTH = vec3(0.1, 0.32, 0.66);
const RAIN_LAYER_SPEED = vec3(1.0, 0.82, 0.66);
const RAIN_LAYER_ALPHA = vec3(1.0, 0.72, 0.48);

// Per mode (drizzle, shower, downpour): fall speed (voxels a second), streak length
// (voxels), lane width (voxels), share of streak cells occupied, alpha, slant (x per y), and
// the haze veil it lays over the depth of the scene.
const RAIN_SPEED = vec3(5.0, 24.0, 34.0);
const RAIN_LEN = vec3(0.2, 1.1, 2.4);
const RAIN_LANE = vec3(0.55, 0.7, 0.5);
const RAIN_FILL = vec3(0.28, 0.3, 0.55);
const RAIN_ALPHA = vec3(0.4, 0.3, 0.32);
const RAIN_SLANT = vec3(0.04, 0.1, 0.24);
const RAIN_VEIL = vec3(0.1, 0.05, 0.16);
// Against a sky brighter than the rain, the rain is drawn this much of the sky behind it:
// darker than the sky, as rain curtains are (Wrysk, 2026-09-25: pale streaks vanished
// against the bright day sky and the rain seemed to appear from nowhere over the land).
const RAIN_CURTAIN: f32 = 0.45;
// The rain keeps its own pale colour only where it is clearly brighter than the sky behind
// it (in luma, by this much: a night sky); against a sky as bright as it or brighter (the
// day sky, the same luma as pale rain) it turns to the curtain.
const RAIN_CURTAIN_RAMP: f32 = 0.25;
// A curtain's streaks are this much more opaque than the rain's own: thin dark lines on a
// bright sky need more cover to read than pale ones on dark terrain.
const RAIN_CURTAIN_ALPHA: f32 = 1.8;

// The downpour's gusts: bands of heavier rain sweeping along the ring (whole periods round
// it and in the clock's wrap), between GUST_LOW and 1.
const GUST_LOW: f32 = 0.35;

fn gustAt(xv: f32) -> f32 {
    let t = wxTime();
    let n1 = max(1.0, floor(f32(W) / 38.0 + 0.5));
    let n2 = max(1.0, floor(f32(W) / 17.0 + 0.5));
    let a = sin(6.28318530718 * (xv * n1 / f32(W) - t * 0.05));
    let b = sin(6.28318530718 * (xv * n2 / f32(W) - t * 0.11) + 1.7);
    return mix(GUST_LOW, 1.0, clamp(0.5 + 0.35 * a + 0.25 * b, 0.0, 1.0));
}

// One layer of one mode at raster pixel (px, py): its coverage times its brightness along
// the streak (the head brighter than the tail), 0 where there is no streak.
fn rainStreak(px: f32, py: f32, mode: i32, layer: i32) -> f32 {
    let Sf = f32(S);
    let speed = RAIN_SPEED[mode] * RAIN_LAYER_SPEED[layer];
    let len = RAIN_LEN[mode] * mix(1.0, RAIN_LAYER_SPEED[layer], 0.6);
    let period = len / 0.4;
    // Lanes across the ring: a whole number of them round it, so the seam is seamless.
    let ringPx = f32(W) * Sf;
    let lanes = max(1.0, floor(ringPx / (RAIN_LANE[mode] * Sf) + 0.5));
    let laneW = ringPx / lanes;
    let xs = px + 0.5 - RAIN_SLANT[mode] * (py + 0.5);
    let lf = floor(xs / laneW);
    let lane = wrapI(i32(lf), i32(lanes));
    let h = cellHash(lane, layer, mode, 170);
    let off = (0.15 + 0.7 * f32(h & 0xFFu) / 255.0) * laneW;
    let width = max(1.0, Sf / 10.0);
    let cx = clamp(0.5 * width + 0.5 - abs(xs - lf * laneW - off), 0.0, 1.0);
    if (cx <= 0.0) { return 0.0; }
    // Down the lane, in voxels: y' = v - t·speed, so a streak moves down the screen.
    let s = wholeIn3600(speed * (0.85 + 0.3 * f32((h >> 8u) & 0xFFu) / 255.0), period);
    let yp = (py + 0.5) / Sf - wxTime() * s + period * f32((h >> 16u) & 0xFFu) / 255.0;
    let cell = floor(yp / period);
    let hc = cellHash(lane, i32(cell), layer * 3 + mode, 171);
    if (f32(hc & 0xFFFFu) / 65535.0 >= RAIN_FILL[mode]) { return 0.0; }
    let pos = yp - cell * period;
    let l = len * (0.75 + 0.5 * f32((hc >> 16u) & 0xFFu) / 255.0);
    let cy = clamp((l - pos) * Sf, 0.0, 1.0) * clamp(pos * Sf + 0.5, 0.0, 1.0);
    return cx * cy * (0.35 + 0.65 * clamp(pos / l, 0.0, 1.0));
}

// Whether rain falls at the world point on this pixel's ray `zl` slabs in: open to the sky
// straight up (above the world, or in air with no solid over it in its column).
fn rainOpen(x: i32, y0: f32, k: f32, zl: f32) -> bool {
    let y = y0 - k * zl;
    if (y >= f32(H)) { return true; }
    if (y < 0.0) { return false; }
    let yi = i32(y);
    let zi = i32(zl);
    return !solidAt(x, yi, zi) && roofGap(x, yi, zi) == 0;
}

// --- splashes on open tops ---
//
// On a solid top open to the sky, each voxel cell holds a drop now and then (SPLASH_SLOT
// seconds a slot, a chance from the modes' weights): a bright dot that opens into a small
// crown and fades within SPLASH_LIFE of its slot.
const SPLASH_SLOT: f32 = 0.5;
const SPLASH_LIFE: f32 = 0.36;
const SPLASH_CHANCE = vec3(0.08, 0.22, 0.4);

// The splash at world point (x, z) on a top: 0..1.
fn splashAt(w: vec2<f32>, copy: i32) -> f32 {
    let tt = wxTime() / SPLASH_SLOT + 0.5 * f32(copy);
    let slot = floor(tt);
    let phase = tt - slot;
    let c = vec2<i32>(floor(w + 0.5 * f32(copy)));
    let h = cellHash(wrapI(c.x, W), c.y, i32(slot), 180 + copy);
    let chance = dot(u.rainK.xyz, SPLASH_CHANCE);
    let hv = f32(h & 0xFFFFu) / 65535.0;
    if (hv >= chance) { return 0.0; }
    let age = (phase - f32((h >> 16u) & 0xFFu) / 255.0 * (1.0 - SPLASH_LIFE)) / SPLASH_LIFE;
    if (age <= 0.0 || age >= 1.0) { return 0.0; }
    let spot = vec2(0.2) + 0.6 * vec2(f32((h >> 24u) & 0xFu), f32((h >> 28u) & 0xFu)) / 15.0;
    let e = (w + 0.5 * f32(copy) - vec2<f32>(c) - spot) * vec2(f32(S), f32(RISE));
    let d = length(e);
    let r = (0.08 + 0.22 * sqrt(age)) * f32(S);
    let ring = max(1.0 - abs(d - r) / 0.9, 0.0) * (1.0 - age);
    let dot_ = max(1.0 - d / 1.2, 0.0) * (1.0 - smoothstep(0.0, 0.35, age));
    let fade = smoothstep(0.0, 0.08, age) * clamp((chance - hv) / 0.08, 0.0, 1.0);
    return max(ring, dot_) * fade;
}

// --- lightning ---
//
// The bolt: a jagged path in the vertical plane of the struck column, from above the frame
// to the middle of the column's top face, with two forks, drawn as a thin near-white core
// and a violet glow (its own halo: the bloom reads the walk's emission, not this pass).
// Seeded by the strike, so every strike differs and one strike is the same on every frame.
const BOLT_SEGS = 18;
const FORK_SEGS = 5;
// How far the path swings sideways a segment, as a share of the segment's height, and how
// far each segment's middle kinks.
const BOLT_SWING: f32 = 0.5;
const BOLT_KINK: f32 = 0.18;

fn segDist(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let t = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-4), 0.0, 1.0);
    return length(pa - ba * t);
}

fn boltHash(i: i32, salt: i32) -> f32 {
    let h = cellHash(i32(u.strikeL.x), i32(u.strikeL.y), i, salt);
    return f32(h & 0xFFFFu) / 65535.0 - 0.5;
}

// The distance from p to the kinked segment a..b (its middle pushed sideways by `kink`).
fn kinkDist(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, kink: f32) -> f32 {
    let m = 0.5 * (a + b) + vec2(kink, 0.0);
    return min(segDist(p, a, m), segDist(p, m, b));
}

// The bolt's light at raster pixel p (linear, additive), or zero.
fn boltAt(p: vec2<f32>) -> vec3<f32> {
    let Sf = f32(S);
    var end = vec2(u.strikeK.x * Sf,
                   f32(BASE) - u.strikeK.z * f32(RISE) - (u.strikeK.y + 1.0) * Sf
                       - 0.5 * f32(RISE));
    // The copy of the strike nearest the pixel round the ring.
    let ringPx = f32(W) * Sf;
    end.x += ringPx * floor((p.x - end.x) / ringPx + 0.5);
    if (abs(p.x - end.x) > 24.0 * Sf || p.y > end.y + 2.0 * Sf) { return vec3(0.0); }
    let top = vec2(end.x + boltHash(0, 192) * 8.0 * Sf, -0.03 * f32(u.extent.w));
    let segH = (end.y - top.y) / f32(BOLT_SEGS);
    let swing = BOLT_SWING * segH * 2.0;
    // A random walk sideways from the top, bent so that it ends on the strike.
    var drift = 0.0;
    for (var j = 1; j <= BOLT_SEGS; j++) { drift += boltHash(j, 190) * swing; }
    let forkAt0 = 3 + i32((boltHash(0, 193) + 0.5) * 5.0);
    let forkAt1 = forkAt0 + 3 + i32((boltHash(1, 193) + 0.5) * 5.0);
    var fork0 = top;
    var fork1 = top;
    var d = 1e9;
    var a = top;
    var walk = 0.0;
    for (var i = 1; i <= BOLT_SEGS; i++) {
        walk += boltHash(i, 190) * swing;
        let f = f32(i) / f32(BOLT_SEGS);
        var y = top.y + segH * f32(i);
        if (i < BOLT_SEGS) { y += boltHash(i, 191) * 0.5 * segH; }
        let b = vec2(top.x + walk + (end.x - top.x - drift) * f, y);
        // Skip segments far from the pixel: only the distance to near ones matters.
        if (abs(p.y - 0.5 * (a.y + b.y)) < segH + 2.0 * Sf) {
            d = min(d, kinkDist(p, a, b, boltHash(i, 197) * 2.0 * BOLT_KINK * segH));
        }
        if (i == forkAt0) { fork0 = b; }
        if (i == forkAt1) { fork1 = b; }
        a = b;
    }
    // Two forks, weaker, thinning toward their ends.
    var fd = 1e9;
    for (var f = 0; f < 2; f++) {
        let side = select(1.0, -1.0, boltHash(f, 194) < 0.0);
        var fa = select(fork1, fork0, f == 0);
        for (var j = 1; j <= FORK_SEGS; j++) {
            let fb = fa + vec2(side * (0.3 + 0.9 * (boltHash(10 * f + j, 195) + 0.5)) * 0.6 * segH,
                               segH * (0.45 + 0.4 * (boltHash(10 * f + j, 196) + 0.5)));
            fd = min(fd, kinkDist(p, fa, fb, boltHash(10 * f + j, 198) * 0.3 * segH)
                             + f32(j) * 0.4);
            fa = fb;
        }
    }
    let w = max(0.75, Sf / 9.0);
    let core = clamp(w + 0.5 - d, 0.0, 1.0) + 0.55 * clamp(0.5 * w + 0.5 - fd, 0.0, 1.0);
    let glow = exp(-d / (0.9 * Sf)) + 0.4 * exp(-fd / (0.6 * Sf));
    let coreC = vec3(0.92, 0.9, 1.0);
    let glowC = mix(u.lightC.rgb, vec3(0.62, 0.42, 1.0), 0.6);
    return u.strikeK.w * (2.2 * core * coreC + 0.6 * glow * glowC);
}

// --- the weather pass ---

// Premultiplied "over": a layer of colour `lc` and coverage `la` in front of (c, a).
fn over(c: ptr<function, vec3<f32>>, a: ptr<function, f32>, lc: vec3<f32>, la: f32) {
    *c = *c * (1.0 - la) + lc * la;
    *a = *a + la * (1.0 - *a);
}

// What the weather lays over the raster at pixel px down to slab `zEnd`, premultiplied
// (colour, coverage): fog (unless the volumetric march carries it), the rain's haze,
// splashes, the streaks far to near, then the bolt, added. `weatherOver` (the walk's in
// `voxel.frag.wgsl`, the weather pass's in `voxel_weather.frag.wgsl`) says how deep.
fn weatherAt(px: vec2<i32>, zEnd: f32) -> vec4<f32> {
    let k = f32(RISE) / f32(S);
    let xw = (f32(PX) + 0.5) / f32(S);
    let y0 = (f32(BASE - px.y - 1) + 0.5) / f32(S);
    let x = PX / S;
    var c = vec3(0.0);
    var a = 0.0;

    if (!VOLUMETRIC && u.fogK.x > 0.0) {
        var t: f32;
        if (i32(u.fogM.x + 0.5) == 3) {
            t = exp(-fogDepthDeep(xw, y0, k, zEnd));
        } else {
            t = exp(-fogDepth(y0, k, zEnd, fogNoiseFor(xw, y0, k, zEnd)));
        }
        if (t < 0.999) {
            // The light it sends back: the ambient and the sun, and near an emitter its glow.
            var fl = u.fogC.rgb + u.rainC.w * u.dayL.y * u.sunLean.rgb;
            if (GLOW && u.fogC.w > 0.0) {
                let zg = max(zEnd - 1.5, 0.5);
                fl += u.fogC.w * glowAt(vec3(xw, max(y0 - k * zg, 0.5), zg));
            }
            over(&c, &a, fl, 1.0 - t);
        }
    }

    let wsum = u.rainK.x + u.rainK.y + u.rainK.z;
    if (wsum > 0.0 || u.rainH.x + u.rainH.y + u.rainH.z > 0.0) {
        let depth = min(zEnd, f32(D)) / f32(max(D, 1));
        var gust = 1.0;
        if (u.rainK.z > 0.0 || u.rainH.z > 0.0) { gust = gustAt(xw - 0.24 * y0); }
        // Where the ray ends in a sky that is not clearly darker than the rain, the rain (its
        // haze and its streaks) darkens toward a curtain of the sky itself; over the terrain
        // and a night sky, both darker than the rain, it stays the rain's own pale colour.
        var curtain = vec3(0.0);
        var dark = 0.0;
        if (zEnd >= f32(D)) {
            let bg = textureLoad(backdropTex, px, 0).rgb;
            const Y = vec3(0.2126, 0.7152, 0.0722);
            dark = 1.0 - smoothstep(0.0, RAIN_CURTAIN_RAMP, dot(u.rainC.rgb, Y) - dot(bg, Y));
            curtain = bg * RAIN_CURTAIN;
        }
        // The rain's own haze over the depth of the scene: its own weights, which gather and
        // thin slowly, so it lingers a while after the streaks stop.
        let veil = dot(u.rainH.xyz, RAIN_VEIL * vec3(1.0, 1.0, gust)) * depth;
        if (veil > 0.0) { over(&c, &a, mix(hazed(u.rainC.rgb, 0.5), curtain, dark), veil); }
        // Splashes where the ray ended on an open solid top.
        let zi = i32(zEnd);
        if (zi < D) {
            let q = BASE - zi * RISE - px.y - 1;
            let level = select(-1, q / S, q >= 0);
            let r = q - level * S;
            if (level >= 1 && r < RISE && level - 1 < H && solidAt(x, level - 1, zi)
                && !(level < H && solidAt(x, level, zi)) && roofGap(x, level - 1, zi) == 0) {
                let w = vec2(xw, f32(zi) + 1.0 - (f32(RISE - 1 - r) + 0.5) / f32(RISE));
                let sp = max(splashAt(w, 0), splashAt(w, 1));
                if (sp > 0.0) {
                    let sc = hazed(mix(u.rainC.rgb, vec3(u.dayL.x), 0.15), hazeAt(zEnd));
                    over(&c, &a, sc, 0.5 * sp);
                }
            }
        }
        // The streaks, far layers first.
        for (var l = 2; l >= 0; l--) {
            let zl = RAIN_DEPTH[l] * f32(D);
            if (zEnd <= zl || !rainOpen(x, y0, k, zl)) { continue; }
            let rc = mix(hazed(u.rainC.rgb, hazeAt(zl)), curtain, dark);
            for (var m = 0; m < 3; m++) {
                let wm = u.rainK[m];
                if (wm <= 0.0) { continue; }
                let st = rainStreak(f32(px.x), f32(px.y), m, l);
                if (st <= 0.0) { continue; }
                let g = select(1.0, gust, m == 2);
                let cover = st * wm * g * RAIN_ALPHA[m] * RAIN_LAYER_ALPHA[l];
                over(&c, &a, rc, clamp(cover * mix(1.0, RAIN_CURTAIN_ALPHA, dark), 0.0, 1.0));
            }
        }
    }

    if (u.strikeK.w > 0.0 && zEnd >= u.strikeK.z) {
        c += boltAt(vec2<f32>(px) + 0.5);
    }
    return vec4(c, a);
}

// The flat tier's rain (the panel's): one layer of streaks, the heaviest mode's look, in the
// rain's colour over the whole picture, faded by the modes' weights; on the look's clock,
// which the flat tier moves once a tick, so it redraws no more often than it did.
fn flatRain(px: i32, py: i32, colour: vec3<f32>) -> vec3<f32> {
    let wsum = u.rainK.x + u.rainK.y + u.rainK.z;
    let m = select(select(0, 1, u.rainK.y >= u.rainK.x), 2, u.rainK.z >= max(u.rainK.x, u.rainK.y));
    let st = rainStreak(f32(px), f32(py), m, 0);
    if (st <= 0.0) { return colour; }
    return mix(colour, u.rainC.rgb, clamp(st * min(wsum, 1.0) * RAIN_ALPHA[m] * 0.8, 0.0, 1.0));
}

// In the walk: whether the ray goes through this plant cell for the plant pass to draw it.
// Only while nothing has been met in front of it: behind water (falling water, a pool's
// surface) the walk draws the cell itself, still, under the water's colour, as ever.
fn walkSkipsPlant(v: vec4<u32>, trans: vec3<f32>) -> bool {
    return plantHidden(v) && all(trans == vec3(1.0)) && (!LIT || wEntry < 0);
}

// The world column a raster column draws: turned round the ring by the view.
fn worldCol(sx: i32) -> i32 {
    let rw = W * S;
    let v = (sx + u.view.x) % rw;
    return select(v, v + rw, v < 0);
}

// Whether cell (x, y, z) is one of the highlighted animal's.
fn highlighted(x: i32, y: i32, z: i32) -> bool {
    if (u.view.y == 0) { return false; }
    let dxh = wrapMod(x - u.hiMin.x, W);
    if (dxh > u.hiMax.x || y < u.hiMin.y || y > u.hiMax.y || z < u.hiMin.z || z > u.hiMax.z) {
        return false;
    }
    if (u.view.y < 0) { return true; }
    let key = u32(wrapX(x)) | (u32(y) << 12u) | (u32(z) << 22u);
    for (var i = 0; i < u.view.y; i++) {
        if (u.hiCells[i >> 2u][i & 3] == key) { return true; }
    }
    return false;
}

fn hiTint(c: vec3<f32>) -> vec3<f32> { return mix(c, vec3(1.0), u.hiK.x); }
