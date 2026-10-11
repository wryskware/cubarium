#version 450
// The voxel strip, drawn per pixel by a slab walk.
//
// `crates/cubarium/src/voxel/present.rs` is the definition of this picture and draws it
// on the CPU far to near, rectangle by rectangle. This walks the depth slabs **near to
// far** for one raster pixel and asks which voxel and which face owns it in each slab.
// The first opaque face ends the walk; water blends front to back on the way.
//
// The projection inversion is `crate::voxel::slab_hit`, which has the derivation. For
// slab z, writing q = base - z*rise - sy - 1, the pixel is in the band of voxel
// level = q/s at r = q - level*s rows up from that band's bottom, and two faces can own
// it: the front face of `level` (at face row s-1-r), and — only when r < rise — the top
// face of `level-1` (at cap row rise-1-r), because a cap occupies the bottom `rise` rows
// of the band above it. Within a slab the CPU paints y ascending, so front-to-back means
// the `level` faces first and the `level-1` cap second.
//
// Everything below that is the CPU presenter's colour rules, ported one for one. Two of
// its ownership tests are genuinely local functions of a cell and its nearer neighbour
// and are ported as well: the water body's `stop` row and `nearerOwns` for a water top.
// The rest of the CPU's bookkeeping — `front_hidden`, `top_hidden`, the y-descending
// clipping — is what walking front to back replaces.


#include "world.glsl"

#if defined(PLANT_VIS) || defined(PLANT_SHADE)
// This file again as the plant pass's two fragment stages (`crate::plants`, compiled with
// `-DPLANT_VIS` and `-DPLANT_SHADE`): the walk's own face functions, so a plant drawn as a
// model shows the stamped cell's texels, lit as the walk lights them.
#include "plant.glsl"
#endif

#ifdef PLANT_VIS
layout(location = 0) out uint outId;
#else
layout(location = 0) out vec4 outColour;
// Lit tier only (the flat tier's pass has no attachment here, so the writes are dropped):
// the light this pixel emits, for the bloom (`cubarium_gpu::bloom`). An emitting texel's
// emissive colour, less the haze in front of it and times the water's transmission in
// front of it; zero for every pixel that is not an emitter.
layout(location = 1) out vec4 emitOut;
#endif
#if !defined(PLANT_VIS) && !defined(PLANT_SHADE)
// The walk's end depth for the model pass (H2): the z, in voxels, where the ray met its
// first opaque face (a front face at its slab, a cap at the row's own depth across it), or
// FAR where it met none. Animals are drawn after the walk, each texel tested against this.
layout(location = 2) out float depthOut;
#endif
const float FAR = 65504.0;

// --- the faces ------------------------------------------------------------------------

// Micro-dithering grain across solid voxel faces to break up flat surfaces
float faceGrain(int x, int y, int z, int dx, int dy) {
    uint h = uint(x * 73856093 ^ y * 19349663 ^ z * 83492791 ^ dx * 2654435761u ^ dy * 38291);
    h = (h ^ (h >> 13)) * 1274126177u;
    return float(int(h & 15u) - 7) * 0.005;
}

// The lit tier's front rectangle: `blockFront`'s rules with its two tones lit. The face's
// own tone is the base under this face's light; where the flat tier leans toward the top
// face's light (the rim row, the chamfer, the riser's head) this leans toward the base
// under the top face's light at its front edge.
vec3 blockFrontLit(int x, int y, int z, uvec4 v, int r, int dx) {
    int dy = S - 1 - r;
    bool openUp = !solidAt(x, y + 1, z);
    bool openLeft = !solidAt(x - 1, y, z);
    bool openRight = !solidAt(x + 1, y, z);
    bool riser = openUp && z > 0 && !solidAt(x, y, z - 1) && solidAt(x, y - 1, z - 1);
    bool onSide = (dx == 0 && openLeft) || (dx + 1 == S && openRight);

    int m = matOf(v);
    int slot = (m == 3 && openUp && texOn(TEX_TURF_SIDE)) ? TEX_TURF_SIDE : 2 * (m - 1);
    vec3 body = blockBody(v, faceBase(m, slot, x, y, z, 0, dx, dy));
    if (m != 0 && u.roofK.z > 0.0) {
        body = body * (1.0 + faceGrain(x, y, z, dx, dy) * (u.roofK.z / 0.04));
    }
    vec3 own = shadeFront(body, x, y, z, dx, dy, canopyAt(x, z - 1, y));
    if (riser) {
        vec3 lit = shadeTop(body, x, y, z, dx, RISE - 1, canopyAt(x, z, y));
        float t = S > 1 ? float(dy) / float(S - 1) : 0.0;
        vec3 slope = mix(lit, own, u.shadeB.z * t);
        float hz = hazeAt(float(z) - t);
        return hazed(onSide ? slope * u.shadeB.x : slope, hz);
    }
    float haze = hazeAt(float(z));
    vec3 c;
    if (dy == 0 && openUp) {
        vec3 lit = shadeTop(body, x, y, z, dx, RISE - 1, canopyAt(x, z, y));
        c = onSide ? lit : mix(own, lit, u.shadeA.w);
    } else {
        c = onSide ? own * u.shadeB.x : own;
    }
    return hazed(c, haze);
}

// `VoxelPresenter::block`'s front rectangle: rim row, side bevel, chamfered corner, or
// — where the ground steps one voxel into depth — the riser's lean with no rim at all.
vec3 blockFront(int x, int y, int z, uvec4 v, int r, int dx) {
    if (LIT) { return blockFrontLit(x, y, z, v, r, dx); }
    int dy = S - 1 - r;
    bool openUp = !solidAt(x, y + 1, z);
    bool openLeft = !solidAt(x - 1, y, z);
    bool openRight = !solidAt(x + 1, y, z);
    bool riser = openUp && z > 0 && !solidAt(x, y, z - 1) && solidAt(x, y - 1, z - 1);
    bool onSide = (dx == 0 && openLeft) || (dx + 1 == S && openRight);

    int m = matOf(v);
    // Soil under open sky wears the turf: the same soil, with a fringe over its top rows.
    int slot = (m == 3 && openUp && texOn(TEX_TURF_SIDE)) ? TEX_TURF_SIDE : 2 * (m - 1);
    vec3 body = blockBody(v, faceBase(m, slot, x, y, z, 0, dx, dy));
    if (m != 0 && u.roofK.z > 0.0) {
        body = body * (1.0 + faceGrain(x, y, z, dx, dy) * (u.roofK.z / 0.04));
    }
    vec3 lit = blockLit(body, roofShade(roofGap(x, y, z)));
    if (riser) {
        float t = S > 1 ? float(dy) / float(S - 1) : 0.0;
        vec3 slope = mix(lit, body, u.shadeB.z * t);
        float hz = hazeAt(float(z) - t);
        return hazed(onSide ? slope * u.shadeB.x : slope, hz);
    }
    float haze = hazeAt(float(z));
    bool onCap = dy == 0 && openUp;
    vec3 c;
    if (onCap) {
        c = onSide ? lit : mix(body, lit, u.shadeA.w);   // the chamfer, then the rim
    } else {
        c = onSide ? body * u.shadeB.x : body;
    }
    return hazed(c, haze);
}

// The lit tier's top rectangle: `blockTop`'s rules with its two tones lit. The face's own
// tone is the base under this face's light; where the flat tier leans back toward the
// unlit body (the contour row, the bevel at a drop) this leans toward the own tone over
// TOP_GAIN, the flat tier's own ratio of a top to the front it caps, so both keep their
// flat-tier contrast at every rung of the ladder.
vec3 blockTopLit(int x, int y, int z, uvec4 v, int r, int dx) {
    int dy = RISE - 1 - r;
    bool openLeft = !solidAt(x - 1, y, z);
    bool openRight = !solidAt(x + 1, y, z);
    bool dropLeft = openLeft && !solidAt(x - 1, y - 1, z);
    bool dropRight = openRight && !solidAt(x + 1, y - 1, z);
    bool backContinues = z + 1 < D && solidAt(x, y, z + 1) && !solidAt(x, y + 1, z + 1);

    int m = matOf(v);
    vec3 body = blockBody(v, faceBase(m, 2 * (m - 1) + 1, x, y, z, 1, dx, dy));
    if (m != 0 && u.roofK.z > 0.0) {
        body = body * (1.0 + faceGrain(x, y, z, dx, dy + 100) * (u.roofK.z / 0.04));
    }
    vec3 own = shadeTop(body, x, y, z, dx, dy, canopyAt(x, z, y));
    vec3 dark = own / u.shadeA.x;
    float hz = hazeAt(float(z) + float(r) / float(RISE));
    vec3 plane = (dy == 0 && RISE > 2 && !backContinues) ? mix(own, dark, u.shadeA.z) : own;
    bool onDrop = (dx == 0 && dropLeft) || (dx + 1 == S && dropRight);
    return hazed(onDrop ? mix(plane, dark, u.shadeB.y) : plane, hz);
}

// `VoxelPresenter::block`'s top rectangle: the contour row only where the ground really
// ends going back, and a bevel only at a real drop.
vec3 blockTop(int x, int y, int z, uvec4 v, int r, int dx) {
    if (LIT) { return blockTopLit(x, y, z, v, r, dx); }
    int dy = RISE - 1 - r;
    bool openLeft = !solidAt(x - 1, y, z);
    bool openRight = !solidAt(x + 1, y, z);
    bool dropLeft = openLeft && !solidAt(x - 1, y - 1, z);
    bool dropRight = openRight && !solidAt(x + 1, y - 1, z);
    bool backContinues = z + 1 < D && solidAt(x, y, z + 1) && !solidAt(x, y + 1, z + 1);

    int m = matOf(v);
    vec3 body = blockBody(v, faceBase(m, 2 * (m - 1) + 1, x, y, z, 1, dx, dy));
    if (m != 0 && u.roofK.z > 0.0) {
        body = body * (1.0 + faceGrain(x, y, z, dx, dy + 100) * (u.roofK.z / 0.04));
    }
    vec3 lit = blockLit(body, roofShade(roofGap(x, y, z)));
    float hz = hazeAt(float(z) + float(r) / float(RISE));
    vec3 plane = (dy == 0 && RISE > 2 && !backContinues) ? mix(lit, body, u.shadeA.z) : lit;
    bool onDrop = (dx == 0 && dropLeft) || (dx + 1 == S && dropRight);
    return hazed(onDrop ? mix(plane, body, u.shadeB.y) : plane, hz);
}

vec3 glyphPigment(uvec4 v, uint q) {
    return styleAt(int(v.a), int(q & 3u));
}

// The texture slot a style's cells draw with on this face, or -1: only a baked model's
// trunk, foliage and drape have one.
int roleSlot(uvec4 v, bool top) {
    int role = int(texelFetch(styleTex, ivec2(0, int(v.a)), 0).a + 0.5);
    if (role < ROLE_BARK || role > ROLE_DRAPE) { return -1; }
    return TEX_BARK_SIDE + 2 * (role - ROLE_BARK) + (top ? 1 : 0);
}

// A style's species face slot on this face (`VoxelStyle::with_faces`: the alpha of its
// crown column for the side, of its heart column for the top, slot + 1), or -1.
int speciesSlot(uvec4 v, bool top) {
    return int(texelFetch(styleTex, ivec2(top ? 2 : 1, int(v.a)), 0).a + 0.5) - 1;
}

// A drape cord's texel (`drape_cord.glsl`): never its cap; on its front face, the cord's
// pixels in the walk, and every pixel in the plant pass, which tested the cord itself where
// the swaying cell put it (`plantCordShows`). The cord is the cell's whole cutout: no drape
// texture over it.
bool cordShows(int x, int y, int z, bool top, int dx, int dy) {
#if defined(PLANT_VIS) || defined(PLANT_SHADE)
    return !top;
#else
    return !top && cordTexel(x, y, z, dx, dy);
#endif
}

// A model cell's texture over its pigment. A species face (`species/<species>/`) is
// direct colour and replaces the pigment; a generic one's `r / 128` multiplies the style
// colour, so the colour pass still decides the hue. False where a leaf or drape cutout
// has a hole: that texel is not this cell's, and the walk goes on to whatever is behind
// it.
bool plantTexel(int x, int y, int z, uvec4 v, bool top, int dx, int dy, inout vec3 base) {
    if (cordCell(v)) { return cordShows(x, y, z, top, dx, dy); }
    int own = speciesSlot(v, top);
    if (own >= 0) {
        vec4 t = faceTexel(own, x, y, z, top ? 1 : 0, dx, dy);
        int role = int(texelFetch(styleTex, ivec2(0, int(v.a)), 0).a + 0.5);
        if (role >= ROLE_BARK + 1 && role <= ROLE_DRAPE && t.a < 0.5) { return false; }
        base = srgbToLinear(t.rgb);
        return true;
    }
    int slot = roleSlot(v, top);
    if (!texOn(slot)) { return true; }
    vec4 t = faceTexel(slot, x, y, z, top ? 1 : 0, dx, dy);
    if (slot > TEX_BARK_SIDE + 1 && t.a < 0.5) { return false; }
    base *= t.r * (255.0 / 128.0);
    return true;
}

// Organism anatomy and markings are already resolved in glyphTex by the shared
// appearance layer. This is deliberately generic: the shader knows only pigment slots
// and treatments, never bodies, heads, eyes or facing.
// `row` is the face's row from its top (S - 1 - r for a cell where it stands); a row from
// S on is row `row - S` of its cap, from the back edge, as the atlas lays them out. A
// posing body's cap (WX4) is drawn through here from the band's own voxel, so the walk
// still calls each glyph function from one place.
bool glyphFrontAt(int x, int y, int z, uvec4 v, int row, int dx, out vec3 rgb, out vec3 glow) {
    glow = vec3(0.0);
    uint q = cordGlyphTexel(v, dx, row);
    vec4 pigment = texelFetch(styleTex, ivec2(int(q & 3u), int(v.a)), 0);
    vec3 base = pigment.rgb;
    int tone = int(q >> 2);
    bool top = POSE && row >= S;
    int localY = top ? row - S : row;
    float hz = hazeAt(float(z) + (top ? float(RISE - 1 - localY) / float(RISE) : 0.0));
    if (tone == 63 && !top) { return false; }
    // GLOW-EASE: an emitter part way on adds its colour over its pigment's share.
    vec3 eAdd = vec3(0.0);
    float keep = 1.0;
    if (LIT && (tone == TONE_EMIT || emitsWhole(pigment.a))) {
        // An emitter: its style's emissive colour (or the host's for this cell this
        // frame), unshadowed and at full value.
        vec3 e = styleEmit(int(v.a));
        float on = 1.0;
        cellEmit(x, y, z, e, on);
        if (any(greaterThan(e, vec3(0.0)))) {
            // `glow`, the emission alone, is what the bloom and the walk's emitter test read.
            glow = hazed(e, hz);
            if (on >= 1.0) {
                rgb = glow;
                return true;
            }
            eAdd = e;
            keep = 1.0 - on;
        }
    }
    if (!plantTexel(x, y, z, v, top, dx, localY, base)) { return false; }
    if (LIT) {
        bool coveredUp = !top && (solidAt(x, y + 1, z)
            || (inY(y + 1) && isBlockPart(partOf(at(x, y + 1, z))) && !softMark(at(x, y + 1, z))));
        // The atlas's tones on the lit base: the rim and the lit column lean toward the
        // base under the cell's top light, every other tone keeps its pigment rule under
        // this face's light. A cap is the base under its top light, its edge tones leaning
        // toward it over PLANT_TOP_GAIN. One top light and one front light at most, so
        // each shading function is inlined once.
        // A plant cell's own stand does not shade it; an animal reads the column.
        bool animal = partOf(v) == ANIMAL_INTERIM;
        float cf = animal ? canopyAt(x, z - 1, y) : plantCanopy(v);
        float ct = animal ? canopyAt(x, z, y) : plantCanopy(v);
        bool rim = !top && tone == 2 && !coveredUp;
        vec3 tl = vec3(0.0);
        if (top || rim || tone == 3) {
            tl = shadeTop(base, x, y, z, dx, top ? localY : RISE - 1, ct);
        }
        vec3 c = tl;
        if (top) {
            if (tone == 1) {
                c *= u.plantA.w;
            } else if (tone >= 32 && tone <= 47) {
                float gain = 0.5 + float(tone & 15) / 16.0;
                c = mix(c, tl / u.plantA.x * gain, u.shadeB.y);
            }
        } else if (tone != 3) {
            if (tone == 1) {
                base *= u.shadeB.x;
            } else if (tone == 4 || tone == 5) {
                base = mix(base, styleAt(int(v.a), 0), u.plantB.x);
                if (tone == 5) { base *= mix(1.0, u.plantA.w, crownEdge(x, y, z, dx)); }
            } else if (tone >= 16 && tone <= 31) {
                base *= 0.5 + float(tone & 15) / 16.0;
            }
            vec3 f = shadeFront(base, x, y, z, dx, localY, cf);
            c = rim ? mix(f, tl, u.plantA.z) : f;
        }
        rgb = hazed(c * keep + eAdd, hz);
        return true;
    }
    if (top) {
        // A cap, as `glyphCapAt` draws it.
        vec3 c = plantLit(base, roofShade(roofGap(x, y, z)));
        if (tone == 1) {
            c *= u.plantA.w;
        } else if (tone >= 32 && tone <= 47) {
            float gain = 0.5 + float(tone & 15) / 16.0;
            c = mix(c, base * gain, u.shadeB.y);
        }
        rgb = hazed(c, hz);
        return true;
    }
    bool coveredUp = solidAt(x, y + 1, z)
        || (inY(y + 1) && isBlockPart(partOf(at(x, y + 1, z))) && !softMark(at(x, y + 1, z)));
    if (tone == 1) {
        base *= u.shadeB.x;
    } else if (tone == 2 && !coveredUp) {
        base = mix(base, plantLit(base, roofShade(roofGap(x, y, z))), u.plantA.z);
    } else if (tone == 3) {
        base = plantLit(base, roofShade(roofGap(x, y, z)));
    } else if (tone == 4 || tone == 5) {
        base = mix(base, styleAt(int(v.a), 0), u.plantB.x);
        if (tone == 5) { base *= mix(1.0, u.plantA.w, crownEdge(x, y, z, dx)); }
    } else if (tone >= 16 && tone <= 31) {
        base *= 0.5 + float(tone & 15) / 16.0;
    }
    rgb = hazed(base, hz);
    return true;
}

// `localY` is the cap's row from its back edge (RISE - 1 - r for a cell where it stands).
bool glyphCapAt(int x, int y, int z, uvec4 v, int localY, int dx, out vec3 rgb, out vec3 glow) {
    glow = vec3(0.0);
    int r = RISE - 1 - localY;
    int atlasY = (partOf(v) * 8 + glyphOf(v)) * (S + RISE) + S + localY;
    uint q = texelFetch(glyphTex, ivec2(dx, atlasY), 0).r;
    vec4 pigment = texelFetch(styleTex, ivec2(int(q & 3u), int(v.a)), 0);
    vec3 base = pigment.rgb;
    int tone = int(q >> 2);
    vec3 eAdd = vec3(0.0);
    float keep = 1.0;
    if (LIT && (tone == TONE_EMIT || emitsWhole(pigment.a))) {
        vec3 e = styleEmit(int(v.a));
        float on = 1.0;
        cellEmit(x, y, z, e, on);
        if (any(greaterThan(e, vec3(0.0)))) {
            glow = hazed(e, hazeAt(float(z) + float(r) / float(RISE)));
            if (on >= 1.0) {
                rgb = glow;
                return true;
            }
            eAdd = e;
            keep = 1.0 - on;
        }
    }
    if (!plantTexel(x, y, z, v, true, dx, localY, base)) { return false; }
    if (LIT) {
        // The cap's own tone is the base under its light; the edge tones lean toward it
        // over PLANT_TOP_GAIN, as the flat tier leans toward the unlit base.
        float ct = partOf(v) == ANIMAL_INTERIM ? canopyAt(x, z, y) : plantCanopy(v);
        vec3 own = shadeTop(base, x, y, z, dx, localY, ct);
        vec3 lc = own;
        if (tone == 1) {
            lc *= mix(1.0, u.plantA.w, crownEdge(x, y, z, dx));
        } else if (tone >= 32 && tone <= 47) {
            float gain = 0.5 + float(tone & 15) / 16.0;
            lc = mix(lc, own / u.plantA.x * gain, u.shadeB.y);
        }
        rgb = hazed(lc * keep + eAdd, hazeAt(float(z) + float(r) / float(RISE)));
        return true;
    }
    float shade = roofShade(roofGap(x, y, z));
    vec3 cap = plantLit(base, shade);
    if (tone == 1) {
        cap *= mix(1.0, u.plantA.w, crownEdge(x, y, z, dx));
    } else if (tone >= 32 && tone <= 47) {
        float gain = 0.5 + float(tone & 15) / 16.0;
        cap = mix(cap, base * gain, u.shadeB.y);
    }
    rgb = hazed(cap, hazeAt(float(z) + float(r) / float(RISE)));
    return true;
}

// A dry plant cell the plant pass draws instead of the walk (`PLANT_MODELS`). A cell with
// water in it stays the walk's: the water of its own cell blends over it.
bool plantHidden(uvec4 v) { return PLANT_MODELS && v.g == 0u && plantBlockPart(partOf(v)); }

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

bool isPosed(uvec4 v) { return POSE && partOf(v) == ANIMAL_INTERIM && v.b != 0u; }

// Where posing cell c of row sy is drawn: its front face's bottom and top, in the walk's
// q (y * S plus rows up). `sleep::span`.
ivec2 posedSpan(uvec4 c, int sy) {
    int cls = int(c.b >> 6);
    ivec2 db = ivec2(u.poseK[int(c.b & 63u)].xy);
    if (cls >= 2) {
        return ivec2(sy * S, max(sy * S, min((sy + 1) * S, (sy + cls - 1) * S - db.x)));
    }
    return ivec2(sy * S - db.x, (sy + 1) * S - db.x - (cls == 1 ? db.y : 0));
}

// The face row of posing cell c at q, given its span: a leg is cut at its top, its face
// where it was; a torso cell's face moves with it, and a breathing cell brought down by a
// dip keeps its rim row on top and loses the rows under it.
int posedRow(uvec4 c, int sy, ivec2 sp, int q) {
    if (int(c.b >> 6) >= 2) { return (sy + 1) * S - 1 - q; }
    int t = sp.y - 1 - q;
    return t > 0 ? t + S - (sp.y - sp.x) : 0;
}

// Which posing cell owns row r of band `level`, and which of its rows (a cap's from S on,
// for `glyphFrontAt`): from the rows a settling cell can reach the band from, two over it
// (a torso settles up to two rows) and the band's own. A front face wins, since they all
// lie in the slab's front plane with every cap behind; else the highest cap. The cell
// under the band shows only its cap, which the walk's cap site draws.
bool posedFind(int x, int level, int z, int r, out int gy, out int row) {
    int q = level * S + r;
    gy = level;
    row = -1;
    for (int i = 0; i < 3; ++i) {
        int sy = level + 2 - i;
        if (!inY(sy)) { continue; }
        uvec4 c = at(x, sy, z);
        if (!isPosed(c) || glyphOf(c) == 0) { continue; }
        ivec2 sp = posedSpan(c, sy);
        // A cell cut to nothing is folded away, cap and all (the hunker's neck; a sleeper's
        // leg is under its torso anyway).
        if (sp.y <= sp.x) { continue; }
        if (q >= sp.x && q < sp.y) {
            gy = sy;
            row = posedRow(c, sy, sp, q);
            return true;
        }
        if (row < 0 && q >= sp.y && q < sp.y + RISE) {
            gy = sy;
            row = S + RISE - 1 - (q - sp.y);
        }
    }
    return row >= 0;
}

// The cap row of posing cell v under band `level` at row r, or -1: where its settle and
// breath put its cap.
int posedCapRow(uvec4 v, int level, int r) {
    if (glyphOf(v) == 0) { return -1; }
    ivec2 sp = posedSpan(v, level - 1);
    if (sp.y <= sp.x) { return -1; }
    int q = level * S + r;
    return q >= sp.y && q < sp.y + RISE ? RISE - 1 - (q - sp.y) : -1;
}

// --- water ----------------------------------------------------------------------------

// `VoxelPresenter::nearer_owns`: does the slab one step nearer already own this screen
// row, in the pixel columns of voxel (x, y, z)? The bands are the unclipped ones on
// purpose — where the nearer slab's own face is itself culled, whatever culls it is
// nearer still.
bool nearerOwns(int x, int y, int z, int row) {
    if (z == 0) { return false; }
    int R = frontRow(y, z);
    for (int dy = 0; dy < 3; ++dy) {
        int yn = y + dy;
        int f = R + RISE - dy * S;
        int top;
        if (solidAt(x, yn, z - 1)) {
            top = f - RISE;
        } else {
            int fill = fillPxAt(x, yn, z - 1);
            if (fill == 0) { continue; }
            int skin = f + S - fill;
            top = waterOpenUp(x, yn, z - 1) ? skin - RISE : skin;
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

int wEntry;         // the first water: y | z << 12 | top-row << 24 | flat << 25 | falling << 26,
                    // or -1 for none
vec3 wScatter;      // in-scatter so far, unlit and unhazed, weighted by transmission
float wHaze;        // the haze share of that in-scatter (its weight, averaged over the channels)
float wSurf;        // the height of the entered water's surface
int PX;             // this pixel's raster column
int PY;             // and row

void waterLitReset() {
    wEntry = -1;
    wScatter = vec3(0.0);
    wHaze = 0.0;
    wSurf = 0.0;
}

// Per-channel absorption per voxel of water path.
vec3 waterSigma() { return -log(max(u.waterDeepC.rgb, vec3(1.0 / 4096.0))) * u.waterL.x; }

// Entry flags: the pixel's water is drawn by the flat tier's rule (`waterAt`), and its
// first water is falling.
const int ENTRY_FLAT = 1 << 25;
const int ENTRY_FALLING = 1 << 26;

// Whether the water in cell (x, y, z) is falling: the cell under it is neither solid nor
// full (in drawn pixels). `flowAt` reads the same.
bool fallingAt(int x, int y, int z) {
    return y > 0 && !solidAt(x, y - 1, z) && fillPxAt(x, y - 1, z) < S;
}

// The lit water for one screen row. A pixel whose first water is the lake's cut face at
// the world's front edge, or falling water, takes the flat tier's rule for all of its
// water (`waterAt`'s colours and opacities, off the ladder), carried in the lit tier's
// own accumulators: running the flat code itself in the walk costs +0.8 ms at 13 px in
// occupancy.
void waterAtLit(int x, int y, int z, uvec4 v, int row, inout vec3 acc, inout vec3 trans) {
    int fill = fillPxQ(int(v.g));
    int R = frontRow(y, z);
    int bottom = R + S;
    int skinRow = bottom - fill;
    bool openUp = waterOpenUp(x, y, z);
    bool topRow = openUp && row >= skinRow - RISE && row < skinRow && !nearerOwns(x, y, z, row);
    bool nearSolid = z > 0 && solidAt(x, y, z - 1);
    int nearFill = z > 0 ? fillPxAt(x, y, z - 1) : 0;
    int nearSkin = R + RISE + S - nearFill;
    int stop = bottom;
    if (nearSolid) {
        stop = skinRow;
    } else if (nearFill > 0) {
        bool nearOpenUp = waterOpenUp(x, y, z - 1);
        stop = min(bottom, nearOpenUp ? nearSkin - RISE : nearSkin);
    }
    bool bodyRow = row >= skinRow && row < stop;

    float k = float(RISE) / float(S);
    // The pixel's height on this slab's front plane; the ray falls k voxels a slab.
    float yf = (float(BASE - z * RISE - row - 1) + 0.5) / float(S);
    float h = hazeAt(float(z) + 0.5);
    if (wEntry < 0) {
        if (!topRow && !bodyRow) { return; }
        wEntry = y | (z << 12) | (topRow ? 1 << 24 : 0);
        // The lake's cut face at the world's front edge keeps the flat tier's deep indigo,
        // and falling water its bright surface colour: both by the flat rule, off the
        // ladder (the finish adds the fall's streaks).
        bool falling = fallingAt(x, y, z);
        if ((z == 0 && !topRow) || falling) {
            wEntry |= ENTRY_FLAT | (falling ? ENTRY_FALLING : 0);
        } else {
            // The surface over the entry: its own top face, or up the column a front face
            // belongs to.
            int ys = y;
            for (int i = 0; i < 8 && !topRow; ++i) {
                if (solidAt(x, ys + 1, z) || freeQ(x, ys + 1, z) == 0) { break; }
                ++ys;
            }
            wSurf = float(ys) + float(topRow ? fill : fillPxAt(x, ys, z)) / float(S);
            if (bodyRow && row == skinRow && openUp) {
                // The front's top row: the flat tier's skin line, in the surface colour.
                float a = min(clamp(u.knobs.y, 0.0, 1.0) * u.waterK.x, 0.95);
                wScatter += trans * a * (1.0 - h) * u.waterSurfaceC.rgb;
                wHaze += dot(trans, vec3(a * h / 3.0));
                trans *= 1.0 - a;
            }
        }
    }
    if ((wEntry & ENTRY_FLAT) != 0) {
        float alpha = clamp(u.knobs.y, 0.0, 1.0);
        float skinAlpha = min(alpha * u.waterK.x, 0.95);
        float a;
        vec3 c;
        float hf;
        if (topRow) {
            a = skinAlpha * u.waterK.y;
            c = u.waterSurfaceC.rgb;
            hf = hazeAt(float(z) + float(RISE - 1 - (row - (skinRow - RISE))) / float(RISE));
        } else if (bodyRow) {
            bool isSkin = row == skinRow && openUp;
            a = isSkin ? skinAlpha : alpha;
            c = isSkin ? u.waterSurfaceC.rgb : u.waterDeepC.rgb;
            hf = hazeAt(float(z));
        } else {
            return;
        }
        wScatter += trans * a * (1.0 - hf) * c;
        wHaze += dot(trans, vec3(a * hf / 3.0));
        trans *= 1.0 - a;
        return;
    }
    // The ray's piece in this cell's water, in this slab.
    float top = float(y) + float(fill) / float(S);
    float hi = min(yf, top);
    float lo = max(yf - k, float(y));
    float l = max(hi - lo, 0.0) / k * sqrt(1.0 + k * k);
    if (l <= 0.0) { return; }
    vec3 T = exp(-waterSigma() * l);
    // The in-scatter ramp runs on depth under the surface, as the view ray's path to
    // that depth (l / (hi - lo) voxels of path a voxel of depth).
    float depth = max(wSurf - 0.5 * (hi + lo), 0.0) * sqrt(1.0 + k * k) / k;
    vec3 cin = mix(u.waterSurfaceC.rgb, u.waterDeepC.rgb, 1.0 - exp(-u.waterL.x * depth));
    wScatter += trans * (1.0 - T) * (1.0 - h) * cin;
    wHaze += dot(trans * (1.0 - T), vec3(h / 3.0));
    trans *= T;
}

// `VoxelPresenter::water`, for one screen row: the surface's own receding top face and
// the body below it, each blended at most once.
void waterAt(int x, int y, int z, uvec4 v, int row, inout vec3 acc, inout vec3 trans) {
    if (LIT) { waterAtLit(x, y, z, v, row, acc, trans); return; }
    int fill = fillPxQ(int(v.g));
    int R = frontRow(y, z);
    int bottom = R + S;
    int skinRow = bottom - fill;
    float alpha = clamp(u.knobs.y, 0.0, 1.0);
    float skinAlpha = min(alpha * u.waterK.x, 0.95);
    bool openUp = waterOpenUp(x, y, z);

    if (openUp && row >= skinRow - RISE && row < skinRow && !nearerOwns(x, y, z, row)) {
        int dy = row - (skinRow - RISE);
        vec3 c = hazed(u.waterSurfaceC.rgb, hazeAt(float(z) + float(RISE - 1 - dy) / float(RISE)));
        float a = skinAlpha * u.waterK.y;
        acc += trans * a * c;
        trans *= 1.0 - a;
    }

    bool nearSolid = z > 0 && solidAt(x, y, z - 1);
    int nearFill = z > 0 ? fillPxAt(x, y, z - 1) : 0;
    int nearSkin = R + RISE + S - nearFill;
    int stop = bottom;
    if (nearSolid) {
        stop = skinRow;
    } else if (nearFill > 0) {
        bool nearOpenUp = waterOpenUp(x, y, z - 1);
        stop = min(bottom, nearOpenUp ? nearSkin - RISE : nearSkin);
    }
    if (row >= skinRow && row < stop) {
        bool isSkin = row == skinRow && openUp;
        float haze = hazeAt(float(z));
        vec3 c = hazed(isSkin ? u.waterSurfaceC.rgb : u.waterDeepC.rgb, haze);
        float a = isSkin ? skinAlpha : alpha;
        acc += trans * a * c;
        trans *= 1.0 - a;
    }
}

// --- the walk -------------------------------------------------------------------------

// The twilight band (`dayK.w`, zero away from dawn and dusk), `t` of the way down the
// raster: purple fading in from above, magenta, then the palette's warm orange low down,
// where the sky meets the far ridge.
vec3 bandAt(float t) {
    float b = smoothstep(0.04, 0.56, t);
    vec3 c = mix(u.bandA.rgb, u.bandB.rgb, smoothstep(0.0, 0.5, b));
    c = mix(c, u.bandC.rgb, smoothstep(0.5, 1.0, b));
    return c * (b * b * u.dayK.w);
}

// Vertical sky gradient from zenith down to horizon (the time of day's: the palette's by
// day), with the twilight band over it at dawn and dusk.
vec3 skyAt(int py) {
    if (u.roofK.w > 0.5) {
        float t = clamp(float(py + u.camera.x) / float(max(u.camera.y, 1)), 0.0, 1.0);
        vec3 c = mix(u.skyC.rgb, u.skyHorizon.rgb, t);
        if (u.dayK.w > 0.0) { c += bandAt(t); }
        return c;
    }
    return u.skyC.rgb;
}

// Atmospheric sky with drifting moisture cloud wisps. The lit tier's is the sky pass's
// (stars and clouds instead of wisps).
vec3 skyColor(int px_x, int px_y) {
    if (LIT) { return texelFetch(backdropTex, ivec2(px_x, px_y), 0).rgb; }
    vec3 sky = skyAt(px_y);
    float moisture = clamp(u.knobs.z, 0.0, 1.0);
    int mistRows = min(u.extent.w / 3, 56);
    if (u.roofK.w > 0.5 && moisture > 0.0 && px_y < mistRows) {
        float verticalT = 1.0 - (float(px_y) / float(mistRows));
        float rowFactor = verticalT * verticalT * moisture;
        int driftX = int(u.knobs.w) / 2;
        float wave1 = sin(float(px_x + driftX + px_y * 4) * 0.045) * 0.5 + 0.5;
        float wave2 = cos(float(px_x * 2 - driftX + 37) * 0.025) * 0.5 + 0.5;
        float density = wave1 * wave2 * rowFactor;
        if (density > 0.10) {
            float a = min((density - 0.10) * 1.8, 0.70);
            vec3 cloudC = vec3(0.038, 0.020, 0.102);
            vec3 cloudEdge = vec3(0.113, 0.171, 0.354);
            vec3 col = mix(cloudC, cloudEdge, wave1 * 0.5);
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
const float WALL = -1.0e6;
// The surface drop per voxel that is full speed, and the steps the speed is quantised to
// (0, 1/3, 2/3, 1: a drop under a sixth of FLOW_FULL per voxel is still water).
const float FLOW_FULL = 0.5;
const float FLOW_STEPS = 3.0;

// The free surface near level y in column (x, z): the water over it, in it, or the water
// or ground at most two cells under it.
float surfaceNear(int x, int y, int z) {
    if (z < 0 || z >= D || solidAt(x, y, z)) { return WALL; }
    int ga = solidAt(x, y + 1, z) ? 0 : freeQ(x, y + 1, z);
    if (ga > 0) { return float(y + 1) + float(ga) / 255.0; }
    int g = freeQ(x, y, z);
    if (g > 0) { return float(y) + float(g) / 255.0; }
    for (int yy = y - 1; yy >= max(y - 2, 0); --yy) {
        if (solidAt(x, yy, z)) { return float(yy + 1); }
        int gg = freeQ(x, yy, z);
        if (gg > 0) { return float(yy) + float(gg) / 255.0; }
    }
    return float(max(y - 2, 0));
}

// The surface's slope along one axis from its two neighbours: central where both are
// water or open, one-sided against a wall, flat between two walls.
float slope1(float a, float h0, float b) {
    bool wa = a < WALL * 0.5;
    bool wb = b < WALL * 0.5;
    if (wa && wb) { return 0.0; }
    if (wa) { return b - h0; }
    if (wb) { return h0 - a; }
    return 0.5 * (b - a);
}

// xy: the flow's direction (world x, z) times its quantised speed; z: 1 where falling.
vec3 flowAt(ivec3 c) {
    int x = c.x;
    int y = c.y;
    int z = c.z;
    if (y > 0 && !solidAt(x, y - 1, z) && fillPxAt(x, y - 1, z) < S) {
        return vec3(0.0, 0.0, 1.0);
    }
    // The cell's own column by the same rule as its neighbours': a submerged cell (a
    // front face's lower rows) reads the surface over it, as they do.
    float h0 = surfaceNear(x, y, z);
    float gx = slope1(surfaceNear(x - 1, y, z), h0, surfaceNear(x + 1, y, z));
    float gz = slope1(surfaceNear(x, y, z - 1), h0, surfaceNear(x, y, z + 1));
    vec2 f = -vec2(gx, gz);
    float m = length(f);
    float q = floor(clamp(m / FLOW_FULL, 0.0, 1.0) * FLOW_STEPS + 0.5) / FLOW_STEPS;
    return vec3(m > 0.0 ? f / m * q : vec2(0.0), 0.0);
}

int wrapI(int a, int m) { return wrapMod(a, m); }

// Steps in one flow-map cycle, and voxels the ripples move a step at full speed (one
// voxel a second at 12 Hz). Two copies half a cycle apart take turns: a ripple line
// appears, drifts along the flow and fades within its copy's cycle, so the advection never
// runs away and a restart is never seen.
const float FLOW_CYCLE = 64.0;
const float FLOW_ADVECT = 1.0 / 12.0;
// The ripple lines: segments this many voxels long along x (a line takes part of one),
// and the share of (row, segment) places that carry a line some time in its cycle.
const float LINE_SEG = 2.0;
const float LINE_DENSITY = 0.16;
// A line lives this share of its copy's cycle (between the two), starting at a random
// point in it; it fades in and out over its life (sin²) and grows from its middle.
const float LINE_LIFE_MIN = 0.3;
const float LINE_LIFE_MAX = 0.6;
// A breeze: each row of lines slides along +x at its own speed, between these voxels a
// second at full water speed, on still and moving water alike.
const float BREEZE_MIN = 0.1;
const float BREEZE_MAX = 0.3;

// One copy of the ripple lines at world (x, z), `phase` of the way through cycle `cycle`:
// x is 0 for none, else the tilt along z, signed, times the line's fade (0 to 1); y is the
// line's glint weight, its fade times a taper from the line's middle (1) to its ends (0).
// A line is one raster row of a top face (RISE rows a voxel of z) and part of a
// LINE_SEG-voxel segment.
vec2 rippleLine(vec2 w, vec2 flow, float phase, int cycle, int salt) {
    float t = phase * FLOW_CYCLE * FLOW_ADVECT;
    vec2 a = w - flow * t;
    int row = int(floor(a.y * float(RISE)));
    uint hr = cellHash(row, 0, cycle, salt);
    a.x -= mix(BREEZE_MIN, BREEZE_MAX, float((hr >> 8) & 0xFFu) / 255.0) * t;
    // Segments across the ring: a whole number, so the pattern meets itself at the seam.
    int segs = max(1, int(float(W) / LINE_SEG + 0.5));
    float sx = a.x * float(segs) / float(W) + float(hr & 0xFFu) / 256.0;
    int seg = int(floor(sx));
    uint h = cellHash(wrapI(seg, segs), row, cycle, salt + 1);
    if (float(h & 0xFFFFu) / 65535.0 >= LINE_DENSITY) { return vec2(0.0); }
    uint hl = cellHash(wrapI(seg, segs), row, cycle, salt + 2);
    float life = mix(LINE_LIFE_MIN, LINE_LIFE_MAX, float(hl & 0xFFu) / 255.0);
    float age = (phase - float((hl >> 8) & 0xFFu) / 255.0 * (1.0 - life)) / life;
    if (age <= 0.0 || age >= 1.0) { return vec2(0.0); }
    float fade = sin(3.14159265 * age);
    fade *= fade;
    float f = sx - float(seg);
    float mid = 0.25 + float((h >> 16) & 0xFFu) / 255.0 * 0.5;
    float halfLen = (0.1 + float((h >> 24) & 0x7Fu) / 127.0 * 0.15) * sqrt(fade);
    if (abs(f - mid) >= halfLen) { return vec2(0.0); }
    float along = (f - mid) / halfLen;
    float taper = 1.0 - along * along;
    return vec2(((h & 0x80000000u) != 0u ? 1.0 : -1.0) * fade, fade * taper * taper);
}

// The surface normal at world (x, z) under flow `flow`: straight up, or, on a ripple line,
// tilted along z (toward or away from the camera) by up to the ripple knob. Lines are
// sparse, thin and horizontal; they fade in, glide with the breeze (and the flow, on
// moving water) and fade out. `glint` is the line's glint weight where it tilts toward
// the sun (the normal leans the sun's way along z), else 0.
vec3 rippleNormal(vec2 w, vec2 flow, out float glint) {
    float st = u.clock.y;
    float c1 = st / FLOW_CYCLE;
    float c2 = c1 + 0.5;
    // The step wraps at 2048 (`VoxelRenderer::set_clock`): 32 cycles.
    vec2 s = rippleLine(w, flow, fract(c1), wrapI(int(floor(c1)), 32), 60);
    if (s.x == 0.0) {
        s = rippleLine(w + vec2(0.37, 0.0), flow, fract(c2), wrapI(int(floor(c2)), 32), 62);
    }
    glint = 0.0;
    if (s.x == 0.0) { return vec3(0.0, 1.0, 0.0); }
    vec3 n = normalize(vec3(0.0, 1.0, -s.x * u.waterL.z));
    if (n.z * u.sunK.z > 0.0) { glint = s.y; }
    return n;
}

// --- the surface's details (W checkpoint 2) ------------------------------------------------
//
// Everything here moves on the water's clock (`u.clock.y`, every frame by default) and
// fades or moves continuously: nothing appears or vanishes between two frames.

float hash01(int x, int y, int z, int salt) {
    return float(cellHash(x, y, z, salt) & 0xFFFFu) / 65535.0;
}

// Smooth value noise over the world's (x, z) plane at about `freq` lattice cells a voxel
// (a whole number of them round the ring, so it meets itself at the seam), on noise
// layer `layer`.
float vnoise(vec2 w, float freq, int layer, int salt) {
    int period = max(1, int(float(W) * freq + 0.5));
    vec2 q = vec2(w.x * float(period) / float(W), w.y * freq);
    vec2 i = floor(q);
    vec2 f = q - i;
    f = f * f * (3.0 - 2.0 * f);
    int x0 = wrapI(int(i.x), period);
    int x1 = wrapI(int(i.x) + 1, period);
    int y0 = int(i.y);
    float a = hash01(x0, y0, layer, salt);
    float b = hash01(x1, y0, layer, salt);
    float c = hash01(x0, y0 + 1, layer, salt);
    float d = hash01(x1, y0 + 1, layer, salt);
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

// The same noise evolving in time: `t` layers on from the first, cross-faded (smoothstep)
// from each layer to the next. Layers wrap at 256, so `t` must too.
float vnoiseT(vec2 w, float freq, float t, int salt) {
    float lt = floor(t);
    float ft = t - lt;
    ft = ft * ft * (3.0 - 2.0 * ft);
    int l0 = wrapI(int(lt), 256);
    int l1 = wrapI(int(lt) + 1, 256);
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
const float STAR_CELL = 1.0;
const float STAR_SHARE = 0.16;
const float STAR_GAIN = 0.9;
const float STAR_MIN = 0.09;
const float STAR_MAX = 3.0;
const float STAR_POWER = 1.5;
// Above this brightness a star has a cross and feeds the bloom.
const float STAR_BRIGHT = 0.9;

vec4 starAt(int px, int py, float t) {
    int C = max(4, int(float(S) * STAR_CELL + 0.5));
    ivec2 cell = ivec2(px / C, py / C);
    uint h = cellHash(cell.x, cell.y, 0, 71);
    if (float(h & 0xFFFFu) / 65536.0 >= STAR_SHARE) { return vec4(0.0); }
    uint g = cellHash(cell.x, cell.y, 1, 72);
    uint f = cellHash(cell.x, cell.y, 2, 73);
    // r in (0, 1]: the power law's draw.
    float r = (float(f & 0xFFFFu) + 1.0) / 65536.0;
    float b = min(STAR_MIN * pow(r, -1.0 / STAR_POWER), STAR_MAX);
    int sz = S >= 10 ? 2 : 1;
    // Kept a pixel in from the cell's edge, so a cross stays inside it.
    int span = max(C - sz - 1, 1);
    ivec2 at = ivec2(1 + int(g % uint(span)), 1 + int((g >> 8) % uint(span)));
    ivec2 d = ivec2(px, py) - cell * C - at;
    float k;
    if (d.x >= 0 && d.y >= 0 && d.x < sz && d.y < sz) {
        k = 1.0;
    } else if (b > STAR_BRIGHT && ((d.x == -1 || d.x == sz) && d.y >= 0 && d.y < sz
                                || (d.y == -1 || d.y == sz) && d.x >= 0 && d.x < sz)) {
        k = 0.22 * min((b - STAR_BRIGHT) / STAR_BRIGHT, 1.0);
    } else {
        return vec4(0.0);
    }
    // Twinkle: two sines, each a whole number of periods in the 3600 s wrap (0.2-0.9 Hz
    // and 0.07-0.3 Hz), deeper for a faint star and toward the horizon.
    float w1 = 6.28318530718 * float(720u + (g >> 16) % 2520u) / 3600.0;
    float w2 = 6.28318530718 * float(250u + (f >> 16) % 830u) / 3600.0;
    float ph1 = float(h >> 16) * 0.0000958738;
    float ph2 = float(g >> 24) * 0.0245436926;
    float wave = 0.5 + 0.5 * (0.65 * sin(u.cloudK.w * w1 + ph1) + 0.35 * sin(u.cloudK.w * w2 + ph2));
    float faint = 1.0 - smoothstep(STAR_MIN, STAR_BRIGHT, b);
    float depth = clamp(mix(0.18, 0.55, faint) * (1.0 + 1.2 * smoothstep(0.1, 0.5, t)), 0.0, 0.9);
    float tw = 1.0 - depth * wave;
    // Colour: a temperature, and a rare palette tint.
    float temp = float((f >> 16) & 0xFFu) / 255.0;
    vec3 c = temp < 0.55 ? mix(vec3(0.72, 0.84, 1.0), vec3(1.0), temp / 0.55)
                         : mix(vec3(1.0), vec3(1.0, 0.82, 0.62), (temp - 0.55) / 0.45);
    uint rare = (f >> 24) % 16u;
    if (rare == 0u) { c = mix(c, u.lightC.rgb / max(max(u.lightC.r, u.lightC.g), max(u.lightC.b, 1e-3)), 0.5); }
    if (rare == 1u) { c = mix(c, u.bandB.rgb / max(max(u.bandB.r, u.bandB.g), max(u.bandB.b, 1e-3)), 0.4); }
    float lum = b * tw * k;
    return vec4(c * lum, k * max(b - STAR_BRIGHT, 0.0) / (STAR_MAX - STAR_BRIGHT));
}

// The cloud layer's noise: value-noise fbm over the raster in voxel units, x round the ring
// (so it meets itself at the seam), y stretched CLOUD_FLAT times so clouds are wide and
// flat, domain-warped by a slower field that drifts at half the clouds' speed (so shapes
// change as they go).
const float CLOUD_FREQ = 1.0 / 36.0;
const float CLOUD_FLAT = 3.0;
const float CLOUD_WARP = 20.0;

float cloudFbm(vec2 q, int octaves, int salt) {
    float sum = 0.0;
    float amp = 0.5;
    float norm = 0.0;
    float freq = CLOUD_FREQ;
    for (int i = 0; i < octaves; ++i) {
        sum += amp * vnoise(q, freq, i, salt);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    return sum / norm;
}

float cloudNoise(vec2 q) {
    vec2 wq = q - vec2(0.5 * u.cloudK.y, 0.0);
    vec2 warp = vec2(cloudFbm(wq, 3, 120), cloudFbm(wq + vec2(64.0, 23.0), 3, 121)) - 0.5;
    return cloudFbm(q + warp * CLOUD_WARP, 5, 122);
}

// The day sky (WX2 checkpoint 1b), `t` of the way down the raster: zenith, middle, and the
// lightest at the horizon, which sits about where the far ridge does (the twilight band's
// own reach).
const float DAY_HORIZON_T = 0.55;

vec3 daySkyAt(float t) {
    float g = smoothstep(0.0, DAY_HORIZON_T, t);
    vec3 c = mix(u.dayA.rgb, u.dayB.rgb, smoothstep(0.0, 0.55, g));
    return mix(c, u.dayC.rgb, smoothstep(0.45, 1.0, g));
}

// The lit sky: the gradient (the day sky's over the palette's by `dayA.w`), the twilight
// band, the stars, the clouds. `.a` is the light the pixel sends the bloom (a bright
// star's, times `dayB.w`).
vec4 litSky(int px, int py) {
    py += u.camera.x;
    float t = clamp(float(py) / float(max(u.camera.y, 1)), 0.0, 1.0);
    bool gradient = u.roofK.w > 0.5;
    vec3 sky = gradient ? mix(u.skyC.rgb, u.skyHorizon.rgb, t) : u.skyC.rgb;
    float day = u.dayA.w;
    if (day > 0.0) {
        // Coming in (and going out) through the twilight band's own colours, so the sky
        // brightens rose and orange into the day's rather than through a grey: a linear
        // mix of near-black and a pale colour is a grey.
        float b = smoothstep(0.04, 0.56, t);
        vec3 warm = mix(mix(u.bandA.rgb, u.bandB.rgb, smoothstep(0.0, 0.5, b)), u.bandC.rgb,
                        smoothstep(0.5, 1.0, b));
        sky = mix(sky, mix(warm, daySkyAt(t), smoothstep(0.0, 1.0, day)), day);
    }
    if (gradient && u.dayK.w > 0.0) {
        // The band, a little brighter on the sun's side.
        float xs = float(px) / float(max(W * S, 1)) * 2.0 - 1.0;
        sky += bandAt(t) * (1.0 + 0.4 * u.sunLean.w * xs);
    }
    float glow = 0.0;
    if (u.cloudBody.w > 0.001) {
        vec4 st = starAt(px, py, t) * (u.cloudBody.w * (1.0 - smoothstep(0.25, 0.6, t)));
        sky += st.rgb * STAR_GAIN;
        glow = st.a * u.dayB.w;
    }
    // A lightning flash lifts the sky a little everywhere, and lights the clouds from
    // within (below), most over the strike.
    float flash = u.flashC.w;
    float nearStrike = 0.0;
    if (flash > 0.0) {
        float ringPx = float(W * S);
        float dx = abs(float(px) + 0.5 - u.strikeK.x * float(S));
        dx = min(dx, ringPx - dx) / (28.0 * float(S));
        nearStrike = exp(-dx * dx);
        sky += u.flashC.rgb * (0.04 + 0.06 * nearStrike);
    }
    float cover = u.cloudK.x;
    if (cover < 0.001) { return vec4(sky, glow); }
    float storm = u.cloudK.z;
    // The layer: down from the top of the sky, thinning toward the horizon; a storm brings
    // it lower.
    float env = 1.0 - smoothstep(mix(0.22, 0.5, storm), mix(0.52, 0.8, storm), t);
    if (env <= 0.0) { return vec4(sky, glow); }
    vec2 q = vec2((float(px) + 0.5) / float(S) + u.cloudK.y,
                  (float(py) + 0.5) / float(S) * CLOUD_FLAT);
    float n = cloudNoise(q);
    float thr = mix(0.74, 0.36, cover) - 0.16 * storm;
    float d = smoothstep(thr, thr + 0.08, n) * env * smoothstep(0.0, 0.2, cover);
    if (d <= 0.0) { return vec4(sky, glow); }
    // Lit where the cloud thins toward the sun: the sun is on the camera's side, so on the
    // screen its light comes from above and from its own side.
    vec2 toSun = normalize(vec2(u.sunK.x, -max(u.sunK.y, 0.25) * CLOUD_FLAT));
    float n2 = cloudNoise(q + toSun * 2.0);
    float edge = clamp((n - n2) * 10.0 + 0.2, 0.0, 1.0);
    float lit = edge * u.cloudLit.w;
    // Thick cores and the shaded side darker.
    vec3 body = u.cloudBody.rgb * (1.15 - 0.45 * d * (1.0 - lit)) * (1.0 - 0.5 * storm);
    vec3 col = body + u.cloudLit.rgb * lit;
    if (day > 0.0) {
        // By day: near white where the sun reaches and through the body, violet on the
        // shadowed underside (where the cloud thins away from the sun); heavy cover and a
        // storm push toward the shade.
        // A storm's clouds are mostly underside, and dark (checkpoint 2).
        float sunward = clamp(edge * 2.0 + 0.3 - 0.4 * cover * cover, 0.0, 1.0) * (1.0 - 0.7 * storm);
        vec3 dayCol = mix(u.cloudShade.rgb, u.cloudDay.rgb, sunward) * (1.0 - 0.72 * storm);
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
const float FOAM_WIDTH = 0.4;
const float FOAM_BREATHE = 0.45;
const float FOAM_STEPS = 8.0;
const float PLUNGE_RADIUS = 1.4;
const float PLUNGE_STEPS = 4.0;

// The distance (voxels, in x and z) from surface point `p` in water cell `c` to the
// nearest solid beside it at the surface's level (and the level over it for a brim-full
// cell), or a large number for none: the four sides by the face between, the four corners
// by the corner point.
float shoreDist(ivec3 c, vec3 p, bool brim) {
    float fx = p.x - float(c.x);
    float fz = p.z - float(c.z);
    float d = 9.0;
    for (int dz = -1; dz <= 1; ++dz) {
        for (int dx = -1; dx <= 1; ++dx) {
            if (dx == 0 && dz == 0) { continue; }
            int nx = c.x + dx;
            int nz = c.z + dz;
            if (nz < 0 || nz >= D) { continue; }
            if (!solidAt(nx, c.y, nz) && !(brim && solidAt(nx, c.y + 1, nz))) { continue; }
            float ex = dx < 0 ? fx : (dx > 0 ? 1.0 - fx : 0.0);
            float ez = dz < 0 ? fz : (dz > 0 ? 1.0 - fz : 0.0);
            d = min(d, length(vec2(ex, ez)));
        }
    }
    return d;
}

// The distance (voxels, in x and z) from surface point `p` to the nearest fall landing
// on the pool around column (x, z) of cell `c`: a water cell at level `yp` over a cell
// that is neither solid nor full (`fallingAt`), measured to its column's centre; a large
// number for none.
float plungeDist(ivec3 c, vec3 p, int yp) {
    float d = 9.0;
    for (int dz = -1; dz <= 1; ++dz) {
        for (int dx = -1; dx <= 1; ++dx) {
            int nx = c.x + dx;
            int nz = c.z + dz;
            if (nz < 0 || nz >= D || freeQ(nx, yp, nz) == 0 || !fallingAt(nx, yp, nz)) {
                continue;
            }
            d = min(d, length(p.xz - vec2(float(nx) + 0.5, float(nz) + 0.5)));
        }
    }
    return d;
}

// The foam's cover (0 to 1) at surface point `p` of water cell `c`, `fill` pixels full.
float foamAt(ivec3 c, vec3 p, int fill) {
    bool brim = fill >= S;
    float st = u.clock.y;
    float cover = 0.0;
    float ds = shoreDist(c, p, brim);
    if (ds < FOAM_WIDTH * (1.0 + FOAM_BREATHE)) {
        float t = st / FOAM_STEPS;
        float w = FOAM_WIDTH * (FOAM_BREATHE + vnoiseT(p.xz, 1.0, t, 80));
        float lace = smoothstep(0.3, 0.7, vnoiseT(p.xz, 3.0, t, 81));
        cover = (1.0 - smoothstep(0.35 * w, w, ds)) * mix(0.45, 1.0, lace);
    }
    float dp = plungeDist(c, p, (brim ? c.y + 1 : c.y) + 1);
    if (dp < PLUNGE_RADIUS) {
        float churn = vnoiseT(p.xz, 3.0, st / PLUNGE_STEPS, 82);
        cover = max(cover, (1.0 - smoothstep(0.25, PLUNGE_RADIUS, dp)) * mix(0.5, 1.0, churn));
    }
    return cover;
}

// Rain rings: the surface is cut into cells about RAIN_CELL voxels on a side, and in each
// time slot of RAIN_SLOT animation steps a cell holds a drop with the chance `rainK.w` (the rain rate's), at a
// random place and moment in the slot. Its ring grows to RING_RADIUS voxels over RING_LIFE
// of the slot, easing out, fading in fast and out slowly. Two copies, offset half a cell
// and half a slot, overlap. A ring stays inside its cell, so a pixel asks only its own.
const float RAIN_CELL = 2.0;
const float RAIN_SLOT = 8.0;
const float RING_RADIUS = 0.6;
const float RING_LIFE = 0.7;

float rainRing(vec2 w, int copy) {
    float tt = u.clock.y / RAIN_SLOT + 0.5 * float(copy);
    // The step wraps at 2048: 256 slots.
    int slot = wrapI(int(floor(tt)), 256);
    float phase = fract(tt);
    int cellsX = max(1, int(float(W) / RAIN_CELL + 0.5));
    float cellW = float(W) / float(cellsX);
    vec2 g = vec2(w.x / cellW, w.y / RAIN_CELL) + 0.5 * float(copy);
    ivec2 cell = ivec2(floor(g));
    uint h = cellHash(wrapI(cell.x, cellsX), cell.y, slot, 90 + copy);
    // The density follows the rain rate (`rainK.w`: W-2's 0.55 in a shower, more in a
    // downpour, few in a drizzle). A drop whose draw is near the density is faint, so as the
    // rate eases a ring fades rather than appearing or vanishing mid-life.
    float chance = min(u.rainK.w, 0.98);
    float hv = float(h & 0xFFFFu) / 65535.0;
    if (hv >= chance) { return 0.0; }
    float margin = clamp((chance - hv) / 0.06, 0.0, 1.0);
    float age = (phase - float((h >> 16) & 0xFFu) / 255.0 * (1.0 - RING_LIFE)) / RING_LIFE;
    if (age <= 0.0 || age >= 1.0) { return 0.0; }
    // The drop's place in its cell, far enough in that its ring stays there.
    vec2 m = vec2(RING_RADIUS / cellW, RING_RADIUS / RAIN_CELL);
    vec2 at = vec2(float((h >> 24) & 0xFu), float((h >> 28) & 0xFu)) / 15.0;
    vec2 centre = m + at * (1.0 - 2.0 * m);
    // Voxels from the drop, and the ring's distance in raster pixels (a voxel is S pixels
    // across and RISE rows deep), so it is as thin at its top as at its sides.
    vec2 e = (g - floor(g) - centre) * vec2(cellW, RAIN_CELL);
    float l = length(e);
    float r = RING_RADIUS * (1.0 - (1.0 - age) * (1.0 - age));
    vec2 grad = l > 1e-4 ? e / l / vec2(float(S), float(RISE)) : vec2(1.0 / float(S), 0.0);
    float px = abs(l - r) / max(length(grad), 1e-6);
    float edge = max(1.0 - px / 0.8, 0.0);
    float fade = smoothstep(0.0, 0.12, age) * (1.0 - age) * (1.0 - age);
    return edge * fade * margin;
}

// A ceiling seen in a reflection: the underside of a solid, which the picture never
// draws; the ambient ladder's floor rung and whatever local light reaches it.
vec3 undersideLit(ivec3 c, uvec4 v, vec3 hp) {
    vec3 body = blockBody(v, strataOf(matOf(v)));
    return hazed(body * ladderLight(0.0, 0.0, glowAt(hp - vec3(0.0, 0.5, 0.0))),
                 hazeAt(float(c.z)));
}

// The front-face texel (r rows up from its bottom, dx across) a reflected ray entering
// cell `c` through `axis` at `hp` lands on: a side face (axis 0) is drawn as the front's
// edge column on that side, an underside (axis 1) as its bottom row.
ivec2 reflectTexel(vec3 hp, ivec3 c, int axis, float dirX) {
    int dx = clamp(int(floor(fract(hp.x) * float(S))), 0, S - 1);
    int dy = clamp(int(floor((float(c.y + 1) - hp.y) * float(S))), 0, S - 1);
    if (axis == 0) { dx = dirX > 0.0 ? 0 : S - 1; }
    if (axis == 1) { dy = S - 1; }
    return ivec2(S - 1 - dy, dx);
}

bool glyphFilled(int base, int col, int row) {
    if (col < 0 || col >= S || row < 0 || row >= S) { return true; }
    return (texelFetch(glyphTex, ivec2(col, base + row), 0).r >> 2) != 63u;
}

// Whether a reflected ray stops at a block part's face texel: the texel and its four
// neighbours on the face all drawn (past the face's edge counts as drawn, so a body of
// several cells keeps its edges). Thin parts (stems, sprigs, a berry's pixel) are passed
// through: in a rippled mirror they are specks that blink.
bool glyphTexelAt(uvec4 v, vec3 hp, ivec3 c, int axis, float dirX) {
    ivec2 t = reflectTexel(hp, c, axis, dirX);
    int base = (partOf(v) * 8 + glyphOf(v)) * (S + RISE);
    int row = S - 1 - t.x;
    int col = t.y;
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
bool reflectMarch(vec3 p, vec3 r, out ivec3 hit, out vec3 hp, out int axis) {
    ivec3 c = ivec3(floor(p + r * 1e-3));
    ivec3 dir = ivec3(sign(r));
    vec3 inv = vec3(
        abs(r.x) > 1e-6 ? 1.0 / abs(r.x) : 1e30,
        abs(r.y) > 1e-6 ? 1.0 / abs(r.y) : 1e30,
        abs(r.z) > 1e-6 ? 1.0 / abs(r.z) : 1e30);
    vec3 next = vec3(
        (dir.x > 0 ? float(c.x + 1) - p.x : p.x - float(c.x)) * inv.x,
        (dir.y > 0 ? float(c.y + 1) - p.y : p.y - float(c.y)) * inv.y,
        (dir.z > 0 ? float(c.z + 1) - p.z : p.z - float(c.z)) * inv.z);
    int cap = int(u.waterL.w);
    float tIn = 0.0;
    axis = -1;
    hit = c;
    for (int i = 0; i < cap; ++i) {
        if (c.y >= H || c.y < 0 || c.z < 0 || c.z >= D) { break; }
        if (i > 0) {
            uvec4 v = at(c.x, c.y, c.z);
            // A block part counts where its glyph has a texel (the atlas's own holes let
            // the ray on); a texture's leaf cutout does not (`reflectionShade`).
            if (solidV(v) || (isBlockPart(partOf(v)) && glyphTexelAt(v, p + r * tIn, c, axis, r.x))) {
                hit = c;
                hp = p + r * tIn;
                return true;
            }
        }
        axis = (next.x < next.y && next.x < next.z) ? 0 : (next.y < next.z ? 1 : 2);
        if (axis == 0) { tIn = next.x; c.x += dir.x; next.x += inv.x; }
        else if (axis == 1) { tIn = next.y; c.y += dir.y; next.y += inv.y; }
        else { tIn = next.z; c.z += dir.z; next.z += inv.z; }
    }
    hp = p + r * tIn;
    return false;
}

// The reflection's colour: the hit's face under L's light (`shadeFront`: the ladder, AO,
// the sun and its shadows, the glow), from its base colour (terrain: the material or its
// texture, wet; a block part: its glyph texel's pigment or texture, and an emitter at
// full value), without the picture's edge treatments (rims, bevels, tones), which a
// ripple-broken mirror image at 40 % would not show and which cost the whole shader
// occupancy to inline a second time; a solid's underside at the ladder's floor; or the
// sky gradient at the raster row where the ray left.
vec3 reflectionShade(bool found, ivec3 c, vec3 hp, int axis, vec3 r) {
    if (!found) {
        int row = int(floor(float(BASE) - hp.y * float(S) - hp.z * float(RISE)));
        int col = wrapI(int(floor(hp.x * float(S))), max(W * S, 1));
        return texelFetch(backdropTex, ivec2(col, clamp(row, 0, u.extent.w - 1)), 0).rgb;
    }
    uvec4 v = at(c.x, c.y, c.z);
    ivec2 t = reflectTexel(hp, c, axis, r.x);
    int dy = S - 1 - t.x;
    bool solid = solidV(v);
    if (solid && axis == 1) { return undersideLit(c, v, hp); }
    vec3 base;
    float canopy;
    if (solid) {
        int m = matOf(v);
        base = blockBody(v, faceBase(m, 2 * (m - 1), c.x, c.y, c.z, 0, t.y, dy));
        canopy = canopyAt(c.x, c.z - 1, c.y);
    } else {
        int atlasY = (partOf(v) * 8 + glyphOf(v)) * (S + RISE) + dy;
        uint q = texelFetch(glyphTex, ivec2(t.y, atlasY), 0).r;
        vec4 pigment = texelFetch(styleTex, ivec2(int(q & 3u), int(v.a)), 0);
        vec3 e = styleEmit(int(v.a));
        bool emits = int(q >> 2) == TONE_EMIT || emitsWhole(pigment.a);
        float on = 1.0;
        if (emits) { cellEmit(c.x, c.y, c.z, e, on); }
        if (emits && any(greaterThan(e, vec3(0.0)))) {
            return hazed(e, hazeAt(float(c.z)));
        }
        base = pigment.rgb;
        // A texture's leaf hole: the cell's crown pigment.
        if (!plantTexel(c.x, c.y, c.z, v, false, t.y, dy, base)) { base = styleAt(int(v.a), 1); }
        canopy = partOf(v) == ANIMAL_INTERIM ? canopyAt(c.x, c.z - 1, c.y) : plantCanopy(v);
    }
    return hazed(shadeFront(base, c.x, c.y, c.z, t.y, dy, canopy), hazeAt(float(c.z)));
}

// A falling-water streak at raster pixel (px, py): a third of the pixel columns carry
// one, S/3 rows long (at least 2) once every 16 rows (32 from 8 px a voxel up), moving
// down S/4 rows (at least 1) an animation step. The periods divide the step's wrap.
bool fallStreak(int px, int py, float st) {
    uint h = cellHash(wrapI(px, W * S), 0, 0, 50);
    if (h % 3u != 0u) { return false; }
    int period = S >= 8 ? 32 : 16;
    int len = max(2, S / 3) + int((h >> 8) & 1u);
    int speed = max(1, S / 4);
    int ph = wrapI(py - int(floor(st * float(speed))) + int(h >> 16), period);
    return ph < len;
}

// The capture-only flow overlay (`VoxelParams::debug_flow`): still water dark blue,
// moving water hued by its direction (+x red, +z green, -x cyan, -z violet) and brighter
// with speed, falling water white-on-violet stripes; from 8 px a voxel, a white line (its
// head yellow) from each top face's centre along its flow.
vec3 flowDebug(vec3 fl, bool top, vec3 p) {
    if (fl.z > 0.5) {
        return fallStreak(PX, PY, u.clock.y) ? vec3(1.0) : vec3(0.35, 0.05, 0.5);
    }
    float q = length(fl.xy);
    if (q < 0.01) { return vec3(0.01, 0.02, 0.2); }
    float a = atan(fl.y, fl.x);
    vec3 hue = 0.5 + 0.5 * vec3(cos(a), cos(a - 2.0944), cos(a + 2.0944));
    vec3 c = hue * (0.25 + 0.75 * q);
    if (top && S >= 8) {
        vec2 lp = vec2(fract(p.x) * float(S), (1.0 - fract(p.z)) * float(RISE));
        vec2 o = vec2(0.5 * float(S), 0.5 * float(RISE));
        vec2 d = normalize(vec2(fl.x * float(S), -fl.y * float(RISE)));
        vec2 e = o + d * 0.45 * float(S) * vec2(1.0, float(RISE) / float(S));
        vec2 pa = lp - o;
        vec2 ba = e - o;
        float t = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-4), 0.0, 1.0);
        if (length(pa - ba * t) < 0.75) { c = t > 0.6 ? vec3(1.0, 1.0, 0.3) : vec3(1.0); }
    }
    return c;
}

// The lit water, put together once the walk is over: `colour` is what the walk made of
// the pixel with the water's absorption in it and its in-scatter still out. The entry is
// recomputed from `wEntry`: its cell, whether it is the surface, the point where the ray
// entered, and the open cell whose light it reads.
vec3 litWaterFinish(vec3 colour) {
    if ((wEntry & ENTRY_FLAT) != 0) {
        colour += wScatter * u.dayL.x + wHaze * u.hazeC.rgb;
        if ((wEntry & ENTRY_FALLING) == 0) { return colour; }
        if (u.clock.z > 0.5) { return fallStreak(PX, PY, u.clock.y) ? vec3(1.0) : vec3(0.35, 0.05, 0.5); }
        // A fall's streaks over the flat tier's sheet: the surface colour itself (opaque,
        // hazed), a third of the columns, moving down. The palette has nothing brighter,
        // and the sheet between them keeps the flat tier's brightness.
        int zf = (wEntry >> 12) & 0xFFF;
        return fallStreak(PX, PY, u.clock.y)
            ? hazed(u.waterSurfaceC.rgb * u.dayL.x, hazeAt(float(zf))) : colour;
    }
    int y = wEntry & 0xFFF;
    int z = (wEntry >> 12) & 0xFFF;
    bool top = (wEntry >> 24) != 0;
    int x = PX / S;
    int fill = fillPxAt(x, y, z);
    float k = float(RISE) / float(S);
    float yf = (float(BASE - z * RISE - PY - 1) + 0.5) / float(S);
    float xw = (float(PX) + 0.5) / float(S);
    float ys = float(y) + float(fill) / float(S);
    vec3 p = top ? vec3(xw, ys, float(z) + clamp((yf - ys) / k, 0.0, 1.0)) : vec3(xw, yf, float(z));
    ivec3 open = top ? ivec3(x, fill < S ? y : y + 1, z) : ivec3(x, y, z - 1);
    float canopy = top ? canopyAt(x, z, y) : canopyAt(x, z - 1, y);

    vec3 fl = flowAt(ivec3(x, y, z));
    if (u.clock.z > 0.5) { return flowDebug(fl, top, p); }
    // The ladder's light at the entry, as a ratio to open sunlit sky.
    float sunOn = u.sunK.y > 0.0 ? 1.0 : 0.0;
    float ndl = top ? u.sunK.y : -u.sunK.z;
    float sun = (sunOn > 0.0 && ndl > 0.0) ? sunReaches(p, open) : 0.0;
    vec3 glow = glowAt(top ? p + vec3(0.0, 0.5, 0.0) : p - vec3(0.0, 0.0, 0.5));
    vec3 here = ladderLight(skyOpen(open.x, open.y, open.z) * canopy, sun, glow);
    vec3 ratio = here / max(ladderRef(sunOn), vec3(1e-4));
    colour += wScatter * ratio + wHaze * u.hazeC.rgb;
    if (!top) { return colour; }
    float glint;
    vec3 n = rippleNormal(p.xz, fl.xy, glint);
    vec3 d = normalize(vec3(0.0, -k, 1.0));
    float cosi = clamp(-dot(d, n), 0.0, 1.0);
    float f = clamp(u.waterL.y * (0.02 + 0.98 * pow(1.0 - cosi, 5.0)), 0.0, 1.0);
    vec3 r = reflect(d, n);
    r.y = max(r.y, 0.05);
    r = normalize(r);
    // What is behind the surface first, so the march and the shading carry little state.
    if (REFLECTIONS) {
        vec3 under = (1.0 - f) * colour;
        ivec3 hc;
        vec3 hp;
        int axis;
        bool found = reflectMarch(p, r, hc, hp, axis);
        colour = under + f * reflectionShade(found, hc, hp, axis, r);
    }
    // The surface's details over the reflection: rain rings and foam in the palette's
    // surface colour, off the light as the falls' streaks are (lit, foam in a shaded
    // crevice went a muddy grey), and glints in the palette's light. Foam and rings only
    // lighten: where the water is already brighter they leave it be. Rings and glints lean
    // from their palette colour toward white by the highlight knob (waterM.w).
    float hz = hazeAt(float(z) + 0.5);
    vec3 surf = u.waterSurfaceC.rgb * u.dayL.x;
    vec3 froth = max(colour, hazed(surf, hz));
    if (u.waterM.z > 0.0 && u.rainK.w > 0.0 && skyOpen(open.x, open.y, open.z) > 0.0) {
        float ring = max(rainRing(p.xz, 0), rainRing(p.xz, 1));
        vec3 ringC = max(colour, hazed(mix(surf, vec3(u.dayL.x), u.waterM.w), hz));
        colour = mix(colour, ringC, clamp(u.waterM.z * ring, 0.0, 1.0));
    }
    if (u.waterM.x > 0.0) {
        float foam = foamAt(ivec3(x, y, z), p, fill);
        colour = mix(colour, froth, clamp(u.waterM.x * foam, 0.0, 1.0));
    }
    if (u.waterM.y > 0.0 && glint > 0.0 && sun > 0.0) {
        vec3 glintC = hazed(mix(u.sunLean.rgb, vec3(1.0), u.waterM.w), hz);
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
float wxTime() { return u.cloudK.w; }

// A speed (units a second) nudged so that it moves a whole number of `period`s in the
// clock's 3600 s wrap: the pattern is the same on both sides of the wrap.
float wholeIn3600(float speed, float period) {
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
float fogShape(float y) {
    float h = y - (u.fogK.y + u.fogK.z);
    return h <= 0.0 ? 1.0 : exp(-h / max(u.fogK.w, 1e-3));
}

// The noise's layers go by at this rate (a whole 256 of them in the 3600 s wrap), and it
// drifts with the clouds.
const float FOG_EVOLVE = 256.0 / 3600.0;
const float FOG_FREQ = 1.0 / 9.0;

// The banks' size for the breathing (FOG-MOTION): about 32 voxels across.
const float FOG_BANK_FREQ = 1.0 / 32.0;

// The drifting density at ground point (x, z): 0.35 to 1.65, about 1, times the banks'
// breath when the fog breathes (`fogM`, FOG-MOTION's trial):
//   0 fixed: the pattern alone;
//   1 breathing: two still noise fields over the ground, turned through each other on the
//     breath's period (a cos + b sin), so each bank swells and thins in place on its own
//     phase and the whole never pulses together;
//   2 flowing: that, the banks and the pattern in them carried along +x at the flow;
//   3 deep: flowing, drawn by `fogDepthDeep` (candidate d).
float fogNoise(float x, float z) {
    int mode = int(u.fogM.x + 0.5);
    float flow = mode >= 2 ? mod(wxTime() * wholeIn3600(u.fogM.w, float(W)), float(W)) : 0.0;
    vec2 w = vec2(x - u.cloudK.y - flow, z * 1.6);
    float n = 0.65 * vnoiseT(w, FOG_FREQ, wxTime() * FOG_EVOLVE, 140)
            + 0.35 * vnoise(w, FOG_FREQ * 2.3, 7, 141);
    float d = 0.35 + 1.3 * n;
    if (mode >= 1) {
        vec2 wb = vec2(x - flow, z * 1.6);
        float a = 2.0 * vnoise(wb, FOG_BANK_FREQ, 11, 142) - 1.0;
        float b = 2.0 * vnoise(wb, FOG_BANK_FREQ, 12, 143) - 1.0;
        // Whole periods in the clock's 3600 s wrap.
        float per = 3600.0 / max(1.0, floor(3600.0 / max(u.fogM.z, 1.0) + 0.5));
        float ph = 6.2831853 * wxTime() / per;
        d *= max(0.0, 1.0 + u.fogM.y * (a * cos(ph) + b * sin(ph)));
    }
    return d;
}

// The slab the pixel's ray reaches the fog's band in, and the slabs of band it crosses up
// to `zEnd`: the ray falls k voxels a slab from height y0 at z = 0.
vec2 fogSpan(float y0, float k, float zEnd) {
    float top = u.fogK.y + u.fogK.z;
    float zb = clamp((y0 - top) / k, 0.0, zEnd);
    return vec2(zb, zEnd - zb);
}

// The fog's optical depth along the ray from z = 0 to `zEnd` slabs, at the pixel's noise:
// the band part, plus the exponential tail above it, integrated in closed form.
float fogDepth(float y0, float k, float zEnd, float noise) {
    vec2 span = fogSpan(y0, k, zEnd);
    float top = u.fogK.y + u.fogK.z;
    float fw = max(u.fogK.w, 1e-3);
    float above = y0 > top
        ? (fw / k) * (exp(-(y0 - k * span.x - top) / fw) - exp(-(y0 - top) / fw))
        : 0.0;
    return u.fogK.x * noise * (above + span.y) * sqrt(1.0 + k * k);
}

// The pixel's fog noise, where its ray first reaches the band (the middle of what it
// crosses there).
float fogNoiseFor(float xw, float y0, float k, float zEnd) {
    vec2 span = fogSpan(y0, k, zEnd);
    return fogNoise(xw, span.x + 0.5 * span.y);
}

// FOG-MOTION candidate d (`fogM.x` 3, "deep": flowing, and less of a flat sheet): the
// band's top billows with the noise (FOG_BILLOW voxels for each unit of it off 1, found in
// one refinement: the flat top's crossing, then the billowed top there), and the density
// is taken at two depths along the ray, where it reaches the band and where it ends, each
// for half of the band it crosses. Its optical depth, as `fogDepth`'s.
const float FOG_BILLOW = 3.0;
float fogDepthDeep(float xw, float y0, float k, float zEnd) {
    float base = u.fogK.y + u.fogK.z;
    float nb = fogNoise(xw, clamp((y0 - base) / k, 0.0, zEnd));
    float top = base + FOG_BILLOW * (nb - 1.0);
    float zb = clamp((y0 - top) / k, 0.0, zEnd);
    float ne = fogNoise(xw, zEnd);
    float band = zEnd - zb;
    float fw = max(u.fogK.w, 1e-3);
    float above = y0 > top
        ? (fw / k) * (exp(-(y0 - k * zb - top) / fw) - exp(-(y0 - top) / fw))
        : 0.0;
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

const vec3 RAIN_DEPTH = vec3(0.1, 0.32, 0.66);
const vec3 RAIN_LAYER_SPEED = vec3(1.0, 0.82, 0.66);
const vec3 RAIN_LAYER_ALPHA = vec3(1.0, 0.72, 0.48);

// Per mode (drizzle, shower, downpour): fall speed (voxels a second), streak length
// (voxels), lane width (voxels), share of streak cells occupied, alpha, slant (x per y), and
// the haze veil it lays over the depth of the scene.
const vec3 RAIN_SPEED = vec3(5.0, 24.0, 34.0);
const vec3 RAIN_LEN = vec3(0.2, 1.1, 2.4);
const vec3 RAIN_LANE = vec3(0.55, 0.7, 0.5);
const vec3 RAIN_FILL = vec3(0.28, 0.3, 0.55);
const vec3 RAIN_ALPHA = vec3(0.4, 0.3, 0.32);
const vec3 RAIN_SLANT = vec3(0.04, 0.1, 0.24);
const vec3 RAIN_VEIL = vec3(0.1, 0.05, 0.16);
// Against a sky brighter than the rain, the rain is drawn this much of the sky behind it:
// darker than the sky, as rain curtains are (Wrysk, 2026-09-25: pale streaks vanished
// against the bright day sky and the rain seemed to appear from nowhere over the land).
const float RAIN_CURTAIN = 0.45;
// The rain keeps its own pale colour only where it is clearly brighter than the sky behind
// it (in luma, by this much: a night sky); against a sky as bright as it or brighter (the
// day sky, the same luma as pale rain) it turns to the curtain.
const float RAIN_CURTAIN_RAMP = 0.25;
// A curtain's streaks are this much more opaque than the rain's own: thin dark lines on a
// bright sky need more cover to read than pale ones on dark terrain.
const float RAIN_CURTAIN_ALPHA = 1.8;

// The downpour's gusts: bands of heavier rain sweeping along the ring (whole periods round
// it and in the clock's wrap), between GUST_LOW and 1.
const float GUST_LOW = 0.35;

float gustAt(float xv) {
    float t = wxTime();
    float n1 = max(1.0, floor(float(W) / 38.0 + 0.5));
    float n2 = max(1.0, floor(float(W) / 17.0 + 0.5));
    float a = sin(6.28318530718 * (xv * n1 / float(W) - t * 0.05));
    float b = sin(6.28318530718 * (xv * n2 / float(W) - t * 0.11) + 1.7);
    return mix(GUST_LOW, 1.0, clamp(0.5 + 0.35 * a + 0.25 * b, 0.0, 1.0));
}

// One layer of one mode at raster pixel (px, py): its coverage times its brightness along
// the streak (the head brighter than the tail), 0 where there is no streak.
float rainStreak(float px, float py, int mode, int layer) {
    float Sf = float(S);
    float speed = RAIN_SPEED[mode] * RAIN_LAYER_SPEED[layer];
    float len = RAIN_LEN[mode] * mix(1.0, RAIN_LAYER_SPEED[layer], 0.6);
    float period = len / 0.4;
    // Lanes across the ring: a whole number of them round it, so the seam is seamless.
    float ringPx = float(W) * Sf;
    float lanes = max(1.0, floor(ringPx / (RAIN_LANE[mode] * Sf) + 0.5));
    float laneW = ringPx / lanes;
    float xs = px + 0.5 - RAIN_SLANT[mode] * (py + 0.5);
    float lf = floor(xs / laneW);
    int lane = wrapI(int(lf), int(lanes));
    uint h = cellHash(lane, layer, mode, 170);
    float off = (0.15 + 0.7 * float(h & 0xFFu) / 255.0) * laneW;
    float width = max(1.0, Sf / 10.0);
    float cx = clamp(0.5 * width + 0.5 - abs(xs - lf * laneW - off), 0.0, 1.0);
    if (cx <= 0.0) { return 0.0; }
    // Down the lane, in voxels: y' = v - t·speed, so a streak moves down the screen.
    float s = wholeIn3600(speed * (0.85 + 0.3 * float((h >> 8) & 0xFFu) / 255.0), period);
    float yp = (py + 0.5) / Sf - wxTime() * s + period * float((h >> 16) & 0xFFu) / 255.0;
    float cell = floor(yp / period);
    uint hc = cellHash(lane, int(cell), layer * 3 + mode, 171);
    if (float(hc & 0xFFFFu) / 65535.0 >= RAIN_FILL[mode]) { return 0.0; }
    float pos = yp - cell * period;
    float l = len * (0.75 + 0.5 * float((hc >> 16) & 0xFFu) / 255.0);
    float cy = clamp((l - pos) * Sf, 0.0, 1.0) * clamp(pos * Sf + 0.5, 0.0, 1.0);
    return cx * cy * (0.35 + 0.65 * clamp(pos / l, 0.0, 1.0));
}

// Whether rain falls at the world point on this pixel's ray `zl` slabs in: open to the sky
// straight up (above the world, or in air with no solid over it in its column).
bool rainOpen(int x, float y0, float k, float zl) {
    float y = y0 - k * zl;
    if (y >= float(H)) { return true; }
    if (y < 0.0) { return false; }
    int yi = int(y);
    int zi = int(zl);
    return !solidAt(x, yi, zi) && roofGap(x, yi, zi) == 0;
}

// --- splashes on open tops ---
//
// On a solid top open to the sky, each voxel cell holds a drop now and then (SPLASH_SLOT
// seconds a slot, a chance from the modes' weights): a bright dot that opens into a small
// crown and fades within SPLASH_LIFE of its slot.
const float SPLASH_SLOT = 0.5;
const float SPLASH_LIFE = 0.36;
const vec3 SPLASH_CHANCE = vec3(0.08, 0.22, 0.4);

// The splash at world point (x, z) on a top: 0..1.
float splashAt(vec2 w, int copy) {
    float tt = wxTime() / SPLASH_SLOT + 0.5 * float(copy);
    float slot = floor(tt);
    float phase = tt - slot;
    ivec2 c = ivec2(floor(w + 0.5 * float(copy)));
    uint h = cellHash(wrapI(c.x, W), c.y, int(slot), 180 + copy);
    float chance = dot(u.rainK.xyz, SPLASH_CHANCE);
    float hv = float(h & 0xFFFFu) / 65535.0;
    if (hv >= chance) { return 0.0; }
    float age = (phase - float((h >> 16) & 0xFFu) / 255.0 * (1.0 - SPLASH_LIFE)) / SPLASH_LIFE;
    if (age <= 0.0 || age >= 1.0) { return 0.0; }
    vec2 at = vec2(0.2) + 0.6 * vec2(float((h >> 24) & 0xFu), float((h >> 28) & 0xFu)) / 15.0;
    vec2 e = (w + 0.5 * float(copy) - vec2(c) - at) * vec2(float(S), float(RISE));
    float d = length(e);
    float r = (0.08 + 0.22 * sqrt(age)) * float(S);
    float ring = max(1.0 - abs(d - r) / 0.9, 0.0) * (1.0 - age);
    float dot_ = max(1.0 - d / 1.2, 0.0) * (1.0 - smoothstep(0.0, 0.35, age));
    float fade = smoothstep(0.0, 0.08, age) * clamp((chance - hv) / 0.08, 0.0, 1.0);
    return max(ring, dot_) * fade;
}

// --- lightning ---
//
// The bolt: a jagged path in the vertical plane of the struck column, from above the frame
// to the middle of the column's top face, with two forks, drawn as a thin near-white core
// and a violet glow (its own halo: the bloom reads the walk's emission, not this pass).
// Seeded by the strike, so every strike differs and one strike is the same on every frame.
const int BOLT_SEGS = 18;
const int FORK_SEGS = 5;
// How far the path swings sideways a segment, as a share of the segment's height, and how
// far each segment's middle kinks.
const float BOLT_SWING = 0.5;
const float BOLT_KINK = 0.18;

float segDist(vec2 p, vec2 a, vec2 b) {
    vec2 pa = p - a;
    vec2 ba = b - a;
    float t = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-4), 0.0, 1.0);
    return length(pa - ba * t);
}

float boltHash(int i, int salt) {
    uint h = cellHash(int(u.strikeL.x), int(u.strikeL.y), i, salt);
    return float(h & 0xFFFFu) / 65535.0 - 0.5;
}

// The distance from p to the kinked segment a..b (its middle pushed sideways by `kink`).
float kinkDist(vec2 p, vec2 a, vec2 b, float kink) {
    vec2 m = 0.5 * (a + b) + vec2(kink, 0.0);
    return min(segDist(p, a, m), segDist(p, m, b));
}

// The bolt's light at raster pixel p (linear, additive), or zero.
vec3 boltAt(vec2 p) {
    float Sf = float(S);
    vec2 end = vec2(u.strikeK.x * Sf,
                    float(BASE) - u.strikeK.z * float(RISE) - (u.strikeK.y + 1.0) * Sf
                        - 0.5 * float(RISE));
    // The copy of the strike nearest the pixel round the ring.
    float ringPx = float(W) * Sf;
    end.x += ringPx * floor((p.x - end.x) / ringPx + 0.5);
    if (abs(p.x - end.x) > 24.0 * Sf || p.y > end.y + 2.0 * Sf) { return vec3(0.0); }
    vec2 top = vec2(end.x + boltHash(0, 192) * 8.0 * Sf, -0.03 * float(u.extent.w));
    float segH = (end.y - top.y) / float(BOLT_SEGS);
    float swing = BOLT_SWING * segH * 2.0;
    // A random walk sideways from the top, bent so that it ends on the strike.
    float drift = 0.0;
    for (int j = 1; j <= BOLT_SEGS; ++j) { drift += boltHash(j, 190) * swing; }
    int forkAt0 = 3 + int((boltHash(0, 193) + 0.5) * 5.0);
    int forkAt1 = forkAt0 + 3 + int((boltHash(1, 193) + 0.5) * 5.0);
    vec2 fork0 = top;
    vec2 fork1 = top;
    float d = 1e9;
    vec2 a = top;
    float walk = 0.0;
    for (int i = 1; i <= BOLT_SEGS; ++i) {
        walk += boltHash(i, 190) * swing;
        float f = float(i) / float(BOLT_SEGS);
        float y = top.y + segH * float(i);
        if (i < BOLT_SEGS) { y += boltHash(i, 191) * 0.5 * segH; }
        vec2 b = vec2(top.x + walk + (end.x - top.x - drift) * f, y);
        // Skip segments far from the pixel: only the distance to near ones matters.
        if (abs(p.y - 0.5 * (a.y + b.y)) < segH + 2.0 * Sf) {
            d = min(d, kinkDist(p, a, b, boltHash(i, 197) * 2.0 * BOLT_KINK * segH));
        }
        if (i == forkAt0) { fork0 = b; }
        if (i == forkAt1) { fork1 = b; }
        a = b;
    }
    // Two forks, weaker, thinning toward their ends.
    float fd = 1e9;
    for (int f = 0; f < 2; ++f) {
        float side = boltHash(f, 194) < 0.0 ? -1.0 : 1.0;
        vec2 fa = f == 0 ? fork0 : fork1;
        for (int j = 1; j <= FORK_SEGS; ++j) {
            vec2 fb = fa + vec2(side * (0.3 + 0.9 * (boltHash(10 * f + j, 195) + 0.5)) * 0.6 * segH,
                                segH * (0.45 + 0.4 * (boltHash(10 * f + j, 196) + 0.5)));
            fd = min(fd, kinkDist(p, fa, fb, boltHash(10 * f + j, 198) * 0.3 * segH)
                             + float(j) * 0.4);
            fa = fb;
        }
    }
    float w = max(0.75, Sf / 9.0);
    float core = clamp(w + 0.5 - d, 0.0, 1.0) + 0.55 * clamp(0.5 * w + 0.5 - fd, 0.0, 1.0);
    float glow = exp(-d / (0.9 * Sf)) + 0.4 * exp(-fd / (0.6 * Sf));
    vec3 coreC = vec3(0.92, 0.9, 1.0);
    vec3 glowC = mix(u.lightC.rgb, vec3(0.62, 0.42, 1.0), 0.6);
    return u.strikeK.w * (2.2 * core * coreC + 0.6 * glow * glowC);
}

// --- the weather pass ---

// Premultiplied "over": a layer of colour `lc` and coverage `la` in front of (c, a).
void over(inout vec3 c, inout float a, vec3 lc, float la) {
    c = c * (1.0 - la) + lc * la;
    a = a + la * (1.0 - a);
}

// What the weather lays over the raster at pixel px, premultiplied (colour, coverage): fog
// (unless the volumetric march carries it), the rain's haze, splashes, the streaks far to
// near, then the bolt, added.
vec4 weatherOver(ivec2 px) {
    float zEnd = floor(texelFetch(depthTex, px, 0).a * 255.0 + 0.5);
    float k = float(RISE) / float(S);
    float xw = (float(PX) + 0.5) / float(S);
    float y0 = (float(BASE - px.y - 1) + 0.5) / float(S);
    int x = PX / S;
    vec3 c = vec3(0.0);
    float a = 0.0;

    if (!VOLUMETRIC && u.fogK.x > 0.0) {
        float t = int(u.fogM.x + 0.5) == 3
            ? exp(-fogDepthDeep(xw, y0, k, zEnd))
            : exp(-fogDepth(y0, k, zEnd, fogNoiseFor(xw, y0, k, zEnd)));
        if (t < 0.999) {
            // The light it sends back: the ambient and the sun, and near an emitter its glow.
            vec3 fl = u.fogC.rgb + u.rainC.w * u.dayL.y * u.sunLean.rgb;
            if (GLOW && u.fogC.w > 0.0) {
                float zg = max(zEnd - 1.5, 0.5);
                fl += u.fogC.w * glowAt(vec3(xw, max(y0 - k * zg, 0.5), zg));
            }
            over(c, a, fl, 1.0 - t);
        }
    }

    float wsum = u.rainK.x + u.rainK.y + u.rainK.z;
    if (wsum > 0.0 || u.rainH.x + u.rainH.y + u.rainH.z > 0.0) {
        float depth = min(zEnd, float(D)) / float(max(D, 1));
        float gust = u.rainK.z > 0.0 || u.rainH.z > 0.0 ? gustAt(xw - 0.24 * y0) : 1.0;
        // Where the ray ends in a sky that is not clearly darker than the rain, the rain (its
        // haze and its streaks) darkens toward a curtain of the sky itself; over the terrain
        // and a night sky, both darker than the rain, it stays the rain's own pale colour.
        vec3 curtain = vec3(0.0);
        float dark = 0.0;
        if (zEnd >= float(D)) {
            vec3 bg = texelFetch(backdropTex, px, 0).rgb;
            const vec3 Y = vec3(0.2126, 0.7152, 0.0722);
            dark = 1.0 - smoothstep(0.0, RAIN_CURTAIN_RAMP, dot(u.rainC.rgb, Y) - dot(bg, Y));
            curtain = bg * RAIN_CURTAIN;
        }
        // The rain's own haze over the depth of the scene: its own weights, which gather and
        // thin slowly, so it lingers a while after the streaks stop.
        float veil = dot(u.rainH.xyz, RAIN_VEIL * vec3(1.0, 1.0, gust)) * depth;
        if (veil > 0.0) { over(c, a, mix(hazed(u.rainC.rgb, 0.5), curtain, dark), veil); }
        // Splashes where the ray ended on an open solid top.
        int zi = int(zEnd);
        if (zi < D) {
            int q = BASE - zi * RISE - px.y - 1;
            int level = q >= 0 ? q / S : -1;
            int r = q - level * S;
            if (level >= 1 && r < RISE && level - 1 < H && solidAt(x, level - 1, zi)
                && !(level < H && solidAt(x, level, zi)) && roofGap(x, level - 1, zi) == 0) {
                vec2 w = vec2(xw, float(zi) + 1.0 - (float(RISE - 1 - r) + 0.5) / float(RISE));
                float sp = max(splashAt(w, 0), splashAt(w, 1));
                if (sp > 0.0) {
                    vec3 sc = hazed(mix(u.rainC.rgb, vec3(u.dayL.x), 0.15), hazeAt(zEnd));
                    over(c, a, sc, 0.5 * sp);
                }
            }
        }
        // The streaks, far layers first.
        for (int l = 2; l >= 0; --l) {
            float zl = RAIN_DEPTH[l] * float(D);
            if (zEnd <= zl || !rainOpen(x, y0, k, zl)) { continue; }
            vec3 rc = mix(hazed(u.rainC.rgb, hazeAt(zl)), curtain, dark);
            for (int m = 0; m < 3; ++m) {
                float wm = u.rainK[m];
                if (wm <= 0.0) { continue; }
                float st = rainStreak(float(px.x), float(px.y), m, l);
                if (st <= 0.0) { continue; }
                float g = m == 2 ? gust : 1.0;
                float cover = st * wm * g * RAIN_ALPHA[m] * RAIN_LAYER_ALPHA[l];
                over(c, a, rc, clamp(cover * mix(1.0, RAIN_CURTAIN_ALPHA, dark), 0.0, 1.0));
            }
        }
    }

    if (u.strikeK.w > 0.0 && zEnd >= u.strikeK.z) {
        c += boltAt(vec2(px) + 0.5);
    }
    return vec4(c, a);
}

// The flat tier's rain (the panel's): one layer of streaks, the heaviest mode's look, in the
// rain's colour over the whole picture, faded by the modes' weights; on the look's clock,
// which the flat tier moves once a tick, so it redraws no more often than it did.
vec3 flatRain(int px, int py, vec3 colour) {
    float wsum = u.rainK.x + u.rainK.y + u.rainK.z;
    int m = u.rainK.z >= max(u.rainK.x, u.rainK.y) ? 2 : (u.rainK.y >= u.rainK.x ? 1 : 0);
    float st = rainStreak(float(px), float(py), m, 0);
    if (st <= 0.0) { return colour; }
    return mix(colour, u.rainC.rgb, clamp(st * min(wsum, 1.0) * RAIN_ALPHA[m] * 0.8, 0.0, 1.0));
}

#include "volumetric.glsl"

// In the walk: whether the ray goes through this plant cell for the plant pass to draw it.
// Only while nothing has been met in front of it: behind water (falling water, a pool's
// surface) the walk draws the cell itself, still, under the water's colour, as ever.
bool walkSkipsPlant(uvec4 v, vec3 trans) {
    return plantHidden(v) && all(equal(trans, vec3(1.0))) && (!LIT || wEntry < 0);
}

// The world column a raster column draws: turned round the ring by the view.
int worldCol(int sx) {
    int rw = W * S;
    int v = (sx + u.view.x) % rw;
    return v < 0 ? v + rw : v;
}

// Whether cell (x, y, z) is one of the highlighted animal's.
bool highlighted(int x, int y, int z) {
    if (u.view.y == 0) { return false; }
    int dxh = wrapMod(x - u.hiMin.x, W);
    if (dxh > u.hiMax.x || y < u.hiMin.y || y > u.hiMax.y || z < u.hiMin.z || z > u.hiMax.z) {
        return false;
    }
    if (u.view.y < 0) { return true; }
    uint key = uint(wrapX(x)) | (uint(y) << 12) | (uint(z) << 22);
    for (int i = 0; i < u.view.y; ++i) {
        if (u.hiCells[i >> 2][i & 3] == key) { return true; }
    }
    return false;
}

vec3 hiTint(vec3 c) { return mix(c, vec3(1.0), u.hiK.x); }

#if defined(PLANT_VIS) || defined(PLANT_SHADE)
#include "plant_main.glsl"
#else
// A latticevine texel at face pixel (dx, row S - 1 - r) of the vine `vv` carried by voxel
// (x, level, zc), drawn at depth z (z - 1 is the voxel in front, whose canopy shades it):
// its colour into `acc` through `trans`. Returns its coverage: 1 ends the walk here (and
// `trans` is set as the walk's end leaves it), 0 is a hole, between is a vine cell part way
// through a change of tile (GLOW-EASE round 2: the old tile's share of coverage and colour
// over the new one's), and `trans` is cut by it.
float vineDraw(uvec4 vv, int x, int level, int zc, int z, int dx, int r,
               inout vec3 acc, inout vec3 trans) {
    int dy = S - 1 - r;
    vec4 t = vineTexel(vv, x, level, zc, dx, dy);
    // The cell's emitter table entry, looked up once for its old tile and its emission.
    bool en = LIT && vineEmits(t);
    int ent = (en || (uint(u.view.w) & EMIT_VINES) != 0u) ? cellEntry(x, level, zc) : -1;
    uvec4 o = vv;
    float s = entryVineFrom(ent, o);
    float cn = t.a >= 0.5 ? 1.0 - s : 0.0;
    vec4 p = vec4(0.0);
    float co = 0.0;
    if (s > 0.0) {
        p = vineTexel(o, x, level, zc, dx, dy);
        co = p.a >= 0.5 ? s : 0.0;
    }
    float cover = cn + co;
    if (cover <= 0.0) { return 0.0; }
    float h = hazeAt(float(z));
    // GLOW-EASE: a flagged mouth texel draws the host's colour for the cell this frame
    // over its pigment's share. The old tile fades out as its pigment (an emitter's glow
    // went before its tile). Round 3: the two pigments are lit together, once: the face's
    // light is affine in the pigment (`litBy`), so their weighted mean lit equals the
    // weighted mean of each lit, at half the walk's lighting code.
    vec3 e = vec3(0.0);
    bool em = false;
    float wn = cn;
    if (cn > 0.0 && en) {
        e = t.rgb;
        float on = 1.0;
        entryEmit(ent, e, on);
        wn = cn * (1.0 - on);
        em = any(greaterThan(e, vec3(0.0)));
    }
    vec3 col = cn * e;
    float wp = wn + co;
    if (wp > 0.0) {
        vec3 pig = (wn * t.rgb + co * p.rgb) / wp;
        if (LIT) { pig = shadeFront(pig, x, level, z, dx, dy, canopyAt(x, z - 1, level)); }
        col += wp * pig;
    }
    if (cover >= 0.999) {
        acc += trans * hazed(col, h);
        trans = em ? emitterTrans(trans, hazed(e * cn, h), h) : vec3(0.0);
        return 1.0;
    }
    acc += trans * cover * hazed(col / cover, h);
    trans *= 1.0 - cover;
    return cover;
}

void main() {
    S = u.geom.x;
    RISE = u.geom.y;
    BASE = u.geom.z;
    W = u.extent.x;
    H = u.extent.y;
    D = u.extent.z;

    ivec2 px = ivec2(gl_FragCoord.xy);
    if (WEATHER) {
        PX = worldCol(px.x);
        PY = px.y;
        outColour = weatherOver(px);
        return;
    }
    if (SKY) {
        outColour = litSky(px.x, px.y);
        return;
    }
    int wcol = worldCol(px.x);
    int x = wcol / S;
    int dx = wcol - x * S;

    vec3 acc = vec3(0.0);
    vec3 trans = vec3(1.0);
    if (LIT) {
        PX = wcol;
        PY = px.y;
        waterLitReset();
    }

    // The slab the walk ends in outlives the walk: the light shafts stop there.
    int z = 0;
    float zHit = FAR;
    // The walk ended on a plant cell the plant pass would draw, because water lay in front
    // of it (`walkSkipsPlant`).
    bool plantBehind = false;
    for (; z < D; ++z) {
        int q = BASE - z * RISE - px.y - 1;
        // q falls by `rise` a slab: once the pixel is below this slab's y = 0 front face
        // it is below every deeper slab's too.
        if (q < 0) { break; }
        int level = q / S;
        int r = q - level * S;

        // The band's own voxel, which the CPU paints last of the two and which therefore
        // comes first walking front to back.
        if (level < H) {
            uvec4 v = at(x, level, z);
            bool solid = solidV(v);
            // The vine drawn here, if any (one call site: GLOW-EASE round 3 keeps the walk's
            // code small). A covered rock face looking at the camera: its vine, carried by
            // the air cell in front, is drawn flush on the rock, which shows through its
            // holes. An open cell: a plant stands in the void and the water of its own cell
            // blends over it (a trunk in a pool is submerged, so the water is nearer than
            // the plant), then a curtain or a sliver of vine.
            uvec4 vc = v;
            int vz = z;
            bool vineHere = false;
            if (solid) {
                if (z > 0 && u.tex.y != 0) {
                    vc = at(x, level, z - 1);
                    vz = z - 1;
                    vineHere = isVine(vc) && vineKind(vc) == VINE_FLUSH;
                }
            } else {
                if (v.g != 0u) { waterAt(x, level, z, v, px.y, acc, trans); }
                if (isVine(v)) {
                    int k = vineKind(v);
                    int q = max(1, S / 4);
                    vineHere = k == VINE_CURTAIN || (k == VINE_SLIVER_L && dx < q)
                        || (k == VINE_SLIVER_R && dx >= S - q);
                }
            }
            if (vineHere && vineDraw(vc, x, level, vz, z, dx, r, acc, trans) >= 1.0) {
                zHit = float(z); break;
            }
            if (solid) {
                acc += trans * blockFront(x, level, z, v, r, dx);
                trans = vec3(0.0);
                zHit = float(z); break;
            }
            int p = partOf(v);
            if (p != 0 && !walkSkipsPlant(v, trans)) {
                // A posing body (WX4): whichever of its cells (or their caps) the settle
                // has moved over this pixel, if any, instead of the band's own voxel.
                int fy = level;
                int fl = S - 1 - r;
                bool found = true;
                if (isPosed(v)) {
                    found = posedFind(x, level, z, r, fy, fl);
                    v = at(x, fy, z);
                }
                vec3 art;
                vec3 glow;
                if (found && glyphFrontAt(x, fy, z, v, fl, dx, art, glow)) {
                    plantBehind = plantHidden(v);
                    if (partOf(v) == ANIMAL_INTERIM && highlighted(x, fy, z)) { art = hiTint(art); }
                    acc += trans * art;
                    trans = LIT && any(greaterThan(glow, vec3(0.0)))
                        ? emitterTrans(trans, glow, hazeAt(float(z))) : vec3(0.0);
                    zHit = float(z); break;
                }
            }
        }

        // The cap of the voxel below the band, which reaches its bottom `rise` rows.
        // Getting here means the band's own voxel is air, holds no block plant, or holds
        // a model's leaf or drape with a hole at this pixel - so the cap is what shows
        // through, which for the terrain is the CPU's `open_up` and `!covered_up`.
        int below = level - 1;
        if (r < RISE && below >= 0 && below < H) {
            uvec4 v = at(x, below, z);
            if (solidV(v)) {
                acc += trans * blockTop(x, below, z, v, r, dx);
                trans = vec3(0.0);
                zHit = float(z) + (float(r) + 0.5) / float(RISE); break;
            }
            // The cell's own water before its own plant, here as at the level above: the
            // CPU stamps a plant and then blends the water of its cell over it, and a
            // full-to-the-brim cell with air over it has its water *top* in the band
            // above — which is this band, exactly where the plant's cap is. A submerged
            // crown is seen through that surface, not instead of it.
            if (v.g != 0u) { waterAt(x, below, z, v, px.y, acc, trans); }
            int p = partOf(v);
            vec3 cap;
            vec3 glow;
            // A posing cell's cap is where its settle and breath put it (WX4).
            int cl = isPosed(v) ? posedCapRow(v, level, r) : RISE - 1 - r;
            if (isBlockPart(p) && !walkSkipsPlant(v, trans) && cl >= 0
                && glyphCapAt(x, below, z, v, cl, dx, cap, glow)) {
                plantBehind = plantHidden(v);
                if (p == ANIMAL_INTERIM && highlighted(x, below, z)) { cap = hiTint(cap); }
                acc += trans * cap;
                trans = LIT && any(greaterThan(glow, vec3(0.0)))
                    ? emitterTrans(trans, glow, hazeAt(float(z) + float(r) / float(RISE)))
                    : vec3(0.0);
                zHit = float(z) + (float(r) + 0.5) / float(RISE); break;
            }
        }

        if (all(lessThanEqual(trans, vec3(0.0)))) { zHit = float(z); break; }
    }

    // An emitter ended the walk: what it sends the camera is the bloom's.
    vec3 emitted = vec3(0.0);
    if (LIT && trans.x < -0.5) {
        emitted = -trans - 1.0;
        trans = vec3(0.0);
    }

    // The presenter clears to the sky and paints over it; front to back, the sky is
    // whatever light is left.
    vec3 sky = skyColor(px.x, px.y);
    // The brightest stars' share of the bloom (the sky pass's alpha, `[light] star_bloom`),
    // through whatever is in front of them.
    if (LIT && u.dayB.w > 0.0) { emitted += trans * sky * texelFetch(backdropTex, px, 0).a; }
    // The weather pass's depth, in the emission's alpha (`WEATHER_DEPTH` only): the slab the
    // ray ended in, or the water surface in front of it (the fog and the rain stop there).
    int zEnd = LIT && wEntry >= 0 ? min(z, (wEntry >> 12) & 0xFFF) : z;
    emitOut = vec4(emitted, WEATHER_DEPTH ? float(min(zEnd, 255)) / 255.0 : 1.0);
    // A plant cell drawn here under the water in front of it: a slab nearer, so the plant
    // pass's own samples of it count as behind and are not drawn over the water.
    depthOut = plantBehind ? zHit - 1.0 : zHit;
    // The flat tier has no light to scale: its day and night are a tint over the world (1
    // by day), under the sky's own gradient.
    vec3 colour = (LIT ? acc : acc * u.tint.rgb) + trans * sky;
    if (LIT && wEntry >= 0) { colour = litWaterFinish(colour); }
    if (LIT && VOLUMETRIC) {
        // Through the open air in front of the walk's end, and never past a water surface;
        // fog in that air also hides what is behind it.
        vec4 sc = inScatter(zEnd);
        colour = colour * sc.a + sc.rgb;
    }
    outColour = vec4(colour, 1.0);

    // The flat tier's rain: streaks over the whole picture (the lit tier's are the weather
    // pass's, depth-aware).
    if (!LIT && u.rainK.x + u.rainK.y + u.rainK.z > 0.0) {
        outColour.rgb = flatRain(px.x, px.y, outColour.rgb);
    }
}
#endif
