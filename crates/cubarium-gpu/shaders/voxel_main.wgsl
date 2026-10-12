// The voxel strip, drawn per pixel by a slab walk: the walk's fragment stage, shared by
// `voxel.frag.wgsl` and `voxel_weather.frag.wgsl`, which differ only in `weatherOver`.
//
// The projection inversion is `crate::voxel::slab_hit`, which has the derivation. For
// slab z, writing q = base - z*rise - sy - 1, the pixel is in the band of voxel
// level = q/s at r = q - level*s rows up from that band's bottom, and two faces can own
// it: the front face of `level` (at face row s-1-r), and — only when r < rise — the top
// face of `level-1` (at cap row rise-1-r), because a cap occupies the bottom `rise` rows
// of the band above it. Within a slab the CPU paints y ascending, so front-to-back means
// the `level` faces first and the `level-1` cap second.

// #import voxel.wgsl
// #import volumetric.wgsl

// Where the walk met nothing.
const FAR: f32 = 65504.0;

struct WalkOut {
    @location(0) colour: vec4<f32>,
    // Lit tier only (the flat tier's pass has no attachment here, so the writes are dropped):
    // the light this pixel emits, for the bloom (`cubarium_gpu::bloom`). An emitting texel's
    // emissive colour, less the haze in front of it and times the water's transmission in
    // front of it; zero for every pixel that is not an emitter.
    @location(1) emit: vec4<f32>,
    // The walk's end depth for the model pass (H2): the z, in voxels, where the ray met its
    // first opaque face (a front face at its slab, a cap at the row's own depth across it), or
    // FAR where it met none. Animals are drawn after the walk, each texel tested against this.
    @location(2) depth: f32,
}

// A latticevine texel at face pixel (dx, row S - 1 - r) of the vine `vv` carried by voxel
// (x, level, zc), drawn at depth z (z - 1 is the voxel in front, whose canopy shades it):
// its colour into `acc` through `trans`. Returns its coverage: 1 ends the walk here (and
// `trans` is set as the walk's end leaves it), 0 is a hole, between is a vine cell part way
// through a change of tile (GLOW-EASE round 2: the old tile's share of coverage and colour
// over the new one's), and `trans` is cut by it.
fn vineDraw(vv: vec4<u32>, x: i32, level: i32, zc: i32, z: i32, dx: i32, r: i32,
            acc: ptr<function, vec3<f32>>, trans: ptr<function, vec3<f32>>) -> f32 {
    let dy = S - 1 - r;
    let t = vineTexel(vv, x, level, zc, dx, dy);
    // The cell's emitter table entry, looked up once for its old tile and its emission.
    let en = LIT && vineEmits(t);
    var ent = -1;
    if (en || (u32(u.view.w) & EMIT_VINES) != 0u) { ent = cellEntry(x, level, zc); }
    var o = vv;
    let s = entryVineFrom(ent, &o);
    let cn = select(0.0, 1.0 - s, t.a >= 0.5);
    var p = vec4(0.0);
    var co = 0.0;
    if (s > 0.0) {
        p = vineTexel(o, x, level, zc, dx, dy);
        co = select(0.0, s, p.a >= 0.5);
    }
    let cover = cn + co;
    if (cover <= 0.0) { return 0.0; }
    let h = hazeAt(f32(z));
    // GLOW-EASE: a flagged mouth texel draws the host's colour for the cell this frame
    // over its pigment's share. The old tile fades out as its pigment (an emitter's glow
    // went before its tile). Round 3: the two pigments are lit together, once: the face's
    // light is affine in the pigment (`litBy`), so their weighted mean lit equals the
    // weighted mean of each lit, at half the walk's lighting code.
    var e = vec3(0.0);
    var em = false;
    var wn = cn;
    if (cn > 0.0 && en) {
        e = t.rgb;
        var on = 1.0;
        entryEmit(ent, &e, &on);
        wn = cn * (1.0 - on);
        em = any(e > vec3(0.0));
    }
    var col = cn * e;
    let wp = wn + co;
    if (wp > 0.0) {
        var pig = (wn * t.rgb + co * p.rgb) / wp;
        if (LIT) { pig = shadeFront(pig, x, level, z, dx, dy, canopyAt(x, z - 1, level)); }
        col += wp * pig;
    }
    if (cover >= 0.999) {
        *acc += *trans * hazed(col, h);
        if (em) {
            *trans = emitterTrans(*trans, hazed(e * cn, h), h);
        } else {
            *trans = vec3(0.0);
        }
        return 1.0;
    }
    *acc += *trans * cover * hazed(col / cover, h);
    *trans *= 1.0 - cover;
    return cover;
}

@fragment
fn main(@builtin(position) fragCoord: vec4<f32>) -> WalkOut {
    setWorld();
    var out: WalkOut;

    let px = vec2<i32>(fragCoord.xy);
    if (WEATHER) {
        PX = worldCol(px.x);
        PY = px.y;
        out.colour = weatherOver(px);
        return out;
    }
    if (SKY) {
        out.colour = litSky(px.x, px.y);
        return out;
    }
    let wcol = worldCol(px.x);
    let x = wcol / S;
    let dx = wcol - x * S;

    var acc = vec3(0.0);
    var trans = vec3(1.0);
    if (LIT) {
        PX = wcol;
        PY = px.y;
        waterLitReset();
    }

    // The slab the walk ends in outlives the walk: the light shafts stop there.
    var z = 0;
    var zHit = FAR;
    // The walk ended on a plant cell the plant pass would draw, because water lay in front
    // of it (`walkSkipsPlant`).
    var plantBehind = false;
    for (; z < D; z++) {
        let q = BASE - z * RISE - px.y - 1;
        // q falls by `rise` a slab: once the pixel is below this slab's y = 0 front face
        // it is below every deeper slab's too.
        if (q < 0) { break; }
        let level = q / S;
        let r = q - level * S;

        // The band's own voxel, which the CPU paints last of the two and which therefore
        // comes first walking front to back.
        if (level < H) {
            var v = at(x, level, z);
            let solid = solidV(v);
            // The vine drawn here, if any (one call site: GLOW-EASE round 3 keeps the walk's
            // code small). A covered rock face looking at the camera: its vine, carried by
            // the air cell in front, is drawn flush on the rock, which shows through its
            // holes. An open cell: a plant stands in the void and the water of its own cell
            // blends over it (a trunk in a pool is submerged, so the water is nearer than
            // the plant), then a curtain or a sliver of vine.
            var vc = v;
            var vz = z;
            var vineHere = false;
            if (solid) {
                if (z > 0 && u.tex.y != 0) {
                    vc = at(x, level, z - 1);
                    vz = z - 1;
                    vineHere = isVine(vc) && vineKind(vc) == VINE_FLUSH;
                }
            } else {
                if (v.g != 0u) { waterAt(x, level, z, v, px.y, &acc, &trans); }
                if (isVine(v)) {
                    let k = vineKind(v);
                    let sliver = max(1, S / 4);
                    vineHere = k == VINE_CURTAIN || (k == VINE_SLIVER_L && dx < sliver)
                        || (k == VINE_SLIVER_R && dx >= S - sliver);
                }
            }
            if (vineHere) {
                if (vineDraw(vc, x, level, vz, z, dx, r, &acc, &trans) >= 1.0) {
                    zHit = f32(z); break;
                }
            }
            if (solid) {
                acc += trans * blockFront(x, level, z, v, r, dx);
                trans = vec3(0.0);
                zHit = f32(z); break;
            }
            let p = partOf(v);
            if (p != 0 && !walkSkipsPlant(v, trans)) {
                // A posing body (WX4): whichever of its cells (or their caps) the settle
                // has moved over this pixel, if any, instead of the band's own voxel.
                var fy = level;
                var fl = S - 1 - r;
                var found = true;
                if (isPosed(v)) {
                    found = posedFind(x, level, z, r, &fy, &fl);
                    v = at(x, fy, z);
                }
                var art = vec3(0.0);
                var glow = vec3(0.0);
                var frontShown = false;
                if (found) { frontShown = glyphFrontAt(x, fy, z, v, fl, dx, &art, &glow); }
                if (frontShown) {
                    plantBehind = plantHidden(v);
                    if (partOf(v) == ANIMAL_INTERIM && highlighted(x, fy, z)) { art = hiTint(art); }
                    acc += trans * art;
                    if (LIT && any(glow > vec3(0.0))) {
                        trans = emitterTrans(trans, glow, hazeAt(f32(z)));
                    } else {
                        trans = vec3(0.0);
                    }
                    zHit = f32(z); break;
                }
            }
        }

        // The cap of the voxel below the band, which reaches its bottom `rise` rows.
        // Getting here means the band's own voxel is air, holds no block plant, or holds
        // a model's leaf or drape with a hole at this pixel - so the cap is what shows
        // through, which for the terrain is the CPU's `open_up` and `!covered_up`.
        let below = level - 1;
        if (r < RISE && below >= 0 && below < H) {
            let v = at(x, below, z);
            if (solidV(v)) {
                acc += trans * blockTop(x, below, z, v, r, dx);
                trans = vec3(0.0);
                zHit = f32(z) + (f32(r) + 0.5) / f32(RISE); break;
            }
            // The cell's own water before its own plant, here as at the level above: the
            // CPU stamps a plant and then blends the water of its cell over it, and a
            // full-to-the-brim cell with air over it has its water *top* in the band
            // above — which is this band, exactly where the plant's cap is. A submerged
            // crown is seen through that surface, not instead of it.
            if (v.g != 0u) { waterAt(x, below, z, v, px.y, &acc, &trans); }
            let p = partOf(v);
            var cap = vec3(0.0);
            var glow = vec3(0.0);
            // A posing cell's cap is where its settle and breath put it (WX4).
            var cl = RISE - 1 - r;
            if (isPosed(v)) { cl = posedCapRow(v, level, r); }
            var capShown = false;
            if (isBlockPart(p) && !walkSkipsPlant(v, trans) && cl >= 0) {
                capShown = glyphCapAt(x, below, z, v, cl, dx, &cap, &glow);
            }
            if (capShown) {
                plantBehind = plantHidden(v);
                if (p == ANIMAL_INTERIM && highlighted(x, below, z)) { cap = hiTint(cap); }
                acc += trans * cap;
                if (LIT && any(glow > vec3(0.0))) {
                    trans = emitterTrans(trans, glow, hazeAt(f32(z) + f32(r) / f32(RISE)));
                } else {
                    trans = vec3(0.0);
                }
                zHit = f32(z) + (f32(r) + 0.5) / f32(RISE); break;
            }
        }

        if (all(trans <= vec3(0.0))) { zHit = f32(z); break; }
    }

    // An emitter ended the walk: what it sends the camera is the bloom's.
    var emitted = vec3(0.0);
    if (LIT && trans.x < -0.5) {
        emitted = -trans - 1.0;
        trans = vec3(0.0);
    }

    // The presenter clears to the sky and paints over it; front to back, the sky is
    // whatever light is left.
    let sky = skyColor(px.x, px.y);
    // The brightest stars' share of the bloom (the sky pass's alpha, `[light] star_bloom`),
    // through whatever is in front of them.
    if (LIT && u.dayB.w > 0.0) { emitted += trans * sky * textureLoad(backdropTex, px, 0).a; }
    // The weather pass's depth, in the emission's alpha (`WEATHER_DEPTH` only): the slab the
    // ray ended in, or the water surface in front of it (the fog and the rain stop there).
    var zEnd = z;
    if (LIT && wEntry >= 0) { zEnd = min(z, (wEntry >> 12u) & 0xFFF); }
    out.emit = vec4(emitted, select(1.0, f32(min(zEnd, 255)) / 255.0, WEATHER_DEPTH));
    // A plant cell drawn here under the water in front of it: a slab nearer, so the plant
    // pass's own samples of it count as behind and are not drawn over the water.
    out.depth = select(zHit, zHit - 1.0, plantBehind);
    // The flat tier has no light to scale: its day and night are a tint over the world (1
    // by day), under the sky's own gradient.
    var colour = select(acc * u.tint.rgb, acc, LIT) + trans * sky;
    if (LIT && wEntry >= 0) { colour = litWaterFinish(colour); }
    if (LIT && VOLUMETRIC) {
        // Through the open air in front of the walk's end, and never past a water surface;
        // fog in that air also hides what is behind it.
        let sc = inScatter(zEnd);
        colour = colour * sc.a + sc.rgb;
    }
    out.colour = vec4(colour, 1.0);

    // The flat tier's rain: streaks over the whole picture (the lit tier's are the weather
    // pass's, depth-aware).
    if (!LIT && u.rainK.x + u.rainK.y + u.rainK.z > 0.0) {
        out.colour = vec4(flatRain(px.x, px.y, out.colour.rgb), out.colour.a);
    }
    return out;
}
