// The model pass's tile raster, shading (H2): one figure's visible faces, lit by the walk's
// own light at the texel's world point and with its real normal.
//
// Each baked model cell samples the stamped cell's glyph (animal part 5, glyph 1):
// an open top rim and plain other rows. Glyph bodies also keep their outline, lit and
// gain tones from the same atlas. Face coordinates and open-plane crease masks are
// local to the model, so they follow its turns. AO uses the walk's crease width and
// strength, before the light ladder. Terrain/plant neighbours are not in these masks.
// The figures' sun map shadows terrain, plants, other figures and their own faces;
// its resolution and low-sun cutoff differ from the stamped volume's sun march.

// #import world.wgsl
// #import figure.wgsl

struct Pal {
    base: vec4<f32>,  // linear colour; w > 0.5: the cell emits
    emit: vec4<f32>,  // its emissive colour, linear
}
@group(1) @binding(3) var<storage, read> pal: array<Pal>;

struct Varyings {
    @location(0) vWorld: vec3<f32>,
    @location(1) vNormal: vec3<f32>,
    @location(2) vFace: vec2<f32>,
    @location(3) @interpolate(flat) vInfo: vec4<i32>,
    @location(4) @interpolate(flat) vFaceSize: vec2<i32>,
    @location(5) vRimWorld: vec3<f32>,
    @location(6) vRimFace: vec2<f32>,
    @location(7) @interpolate(flat) vCapSize: vec2<i32>,
}

struct TileOut {
    @location(0) colour: vec4<f32>,
    @location(1) emit: vec4<f32>,
    @location(2) z: f32,
}

// The light a face with normal n gets at world point p (lit tier).
fn litFace(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, ao: f32) -> vec3<f32> {
    let c = vec3<i32>(floor(p + n * 0.5));
    let ndl = max(dot(n, u.sunK.xyz), 0.0);
    var sun = 0.0;
    if (ndl > 0.0 && u.sunK.y > 0.0) {
        sun = sunReaches(p + n * 0.01, c);
    }
    let a = skyOpen(c.x, c.y, c.z) * ao * canopyAt(c.x, c.z, i32(floor(p.y - n.y * 0.01)));
    return litBy(base, a, sun, ndl, glowAt(p + n * 0.5));
}

// The flat tier's top light over the cell at p (`plantLit` under its column's roof).
fn flatTop(base: vec3<f32>, p: vec3<f32>) -> vec3<f32> {
    let c = vec3<i32>(floor(p - vec3(0.0, 0.01, 0.0)));
    var gap = 0;
    if (inY(c.y)) { gap = roofGap(c.x, c.y, c.z); }
    return plantLit(base, roofShade(gap));
}

@fragment
fn main(f: Varyings) -> TileOut {
    setWorld();

    let n = normalize(f.vNormal);
    let p = f.vWorld;
    let top = (f.vInfo.w & 7) == 3 || (f.vInfo.w & 7) == 2;
    let openUp = f.vInfo.z != 0;
    let entry = f.vInfo.x;
    let glyph = f.vInfo.y;
    let hz = hazeAt(p.z);
    var out: TileOut;
    out.z = p.z;

    // Atlas rows are in face space, with the cap's RISE rows after the S side rows.
    let face = f.vInfo.w & 7;
    let cap = face == 2 || face == 3;
    let rows = select(S, RISE, cap);
    let dx = clamp(i32(f.vFace.x * f32(S)), 0, S - 1);
    let dy = clamp(i32(f.vFace.y * f32(rows)), 0, rows - 1);
    let stampGlyph = select(glyph, 1, glyph == 0);
    let q = glyphTexel(ANIMAL_INTERIM, stampGlyph, dx, select(0, S, cap) + dy);
    let pigment = select(i32(q & 3u), 0, glyph == 0);
    let tone = i32(q >> 2u);
    if (tone == 63) { discard; }
    let aoTexel = clamp(vec2<i32>(f.vFace * vec2<f32>(f.vFaceSize)), vec2(0), f.vFaceSize - 1);
    let ao = modelCrease((u32(f.vInfo.w) >> 3u) & 255u, aoTexel.x, aoTexel.y, f.vFaceSize);
    let capTexel = clamp(vec2<i32>(f.vRimFace * vec2<f32>(f.vCapSize)), vec2(0), f.vCapSize - 1);
    let capAo = modelCrease((u32(f.vInfo.w) >> 11u) & 255u, capTexel.x, capTexel.y, f.vCapSize);
    let pe = pal[entry + pigment];
    var base = pe.base.rgb;

    if (LIT && (pe.base.w > 0.5 || tone == TONE_EMIT)) {
        if (any(pe.emit.rgb > vec3(0.0))) {
            // An emitter: its emissive colour, unshadowed and at full value.
            let drawn = hazed(pe.emit.rgb, hz);
            out.colour = vec4(drawn, 1.0);
            out.emit = vec4(max(drawn - clamp(hz, 0.0, 1.0) * u.hazeC.rgb, vec3(0.0)), 1.0);
            return out;
        }
    }
    out.emit = vec4(0.0, 0.0, 0.0, 1.0);

    var c: vec3<f32>;
    if (LIT) {
        let up = tiles[pc.k.x].ay.xyz;
        if (top) {
            c = litFace(base, p, n, ao);
            if (tone == 1) {
                c *= u.plantA.w;
            } else if (tone >= 32 && tone <= 47) {
                let gain = 0.5 + f32(tone & 15) / 16.0;
                c = mix(c, c / u.plantA.x * gain, u.shadeB.y);
            }
        } else if (tone == 3) {
            c = litFace(base, f.vRimWorld, up, capAo);
        } else {
            if (tone == 1) {
                base *= u.shadeB.x;
            } else if (tone == 4 || tone == 5) {
                base = mix(base, pal[entry].base.rgb, u.plantB.x);
                if (tone == 5) { base *= u.plantA.w; }
            } else if (tone >= 16 && tone <= 31) {
                base *= 0.5 + f32(tone & 15) / 16.0;
            }
            c = litFace(base, p, n, ao);
            if (tone == 2 && openUp) {
                c = mix(c, litFace(base, f.vRimWorld, up, capAo), u.plantA.z);
            }
        }
    } else {
        if (top) {
            c = flatTop(base, p);
            if (tone == 1) {
                c *= u.plantA.w;
            } else if (tone >= 32 && tone <= 47) {
                let gain = 0.5 + f32(tone & 15) / 16.0;
                c = mix(c, base * gain, u.shadeB.y);
            }
        } else if (tone == 3) {
            c = flatTop(base, p);
        } else {
            if (tone == 1) {
                base *= u.shadeB.x;
            } else if (tone == 2 && openUp) {
                base = mix(base, flatTop(base, p), u.plantA.z);
            } else if (tone == 4 || tone == 5) {
                base = mix(base, pal[entry].base.rgb, u.plantB.x);
                if (tone == 5) { base *= u.plantA.w; }
            } else if (tone >= 16 && tone <= 31) {
                base *= 0.5 + f32(tone & 15) / 16.0;
            }
            c = base;
        }
    }
    var o = hazed(c, hz);
    // The flat tier's day and night: a tint over the hazed world (1 by day), as the walk's.
    if (!LIT) { o *= u.tint.rgb; }
    out.colour = vec4(o, 1.0);
    return out;
}
