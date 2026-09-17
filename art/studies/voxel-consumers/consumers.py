#!/usr/bin/env python3
"""Voxel consumers: how a glowcap cluster and a frondgrazer read at 4 and 8 px per voxel.

Disposable art study for the Fable art thread of
design/handoffs/voxel-round5bc-consumers-briefs-2026-09-17.md. It hand-authors a
tiny strip and projects it the way `crates/cubarium/src/voxel/project.rs` does:

    col = x * s                    (wraps)
    front_row(y, z) = base - (y + 1) * s - z * rise,   rise = round(s * tan(tilt))

slabs far to near, bottom to top, a front face `s` rows tall under a top face `rise`
rows tall. The block and plant shading follow `present.rs` (`TOP_GAIN`, `RIM`,
`EDGE_DARK`, `PLANT_TOP_GAIN`, `PLANT_RIM`, `CROWN_UNDER`, haze by slab) and the
plant palettes are `stand.rs`'s five, so the two new organisms are judged against
what is actually on screen. Everything is sRGB mixed in linear light, as the
presenter does.

Usage:  python3 consumers.py [outdir]     (~2 s)

Outputs (recommended first):
  recommended_px4.png       the scene at 4 px per voxel, x6 nearest for viewing
  recommended_px8.png       the same scene at 8 px per voxel, x3
  variants_grazer.png       three body treatments side by side, both scales
  variants_glowcap.png      three cap treatments side by side, both scales
  states.png                grazer resting / cropping / starving; glowcap full / half / dry
"""

from __future__ import annotations

import math
import os
import sys

import numpy as np
from PIL import Image

# ---- present.rs / stand.rs constants (sRGB hex) ---------------------------------
SKY, SKY_BRIGHT = 0x12093A, 0.12
BEDROCK, ROCK, SOIL = 0x1A1038, 0x2C2070, 0x59206B
WATER_DEEP, WATER_SURFACE, LIGHT, HAZE = 0x1E2798, 0x42C5F8, 0x42C6FF, 0x241657
TOP_GAIN, TOP_TINT, RIM, EDGE_DARK, TOP_EDGE = 2.4, 0.20, 0.38, 0.72, 0.18
PLANT_TOP_GAIN, PLANT_TOP_TINT, PLANT_RIM = 1.5, 0.14, 0.42
TRUNK_SHADE, TRUNK_LIGHT_AT = (0.62, 1.22), 0.35
CROWN_EDGE, CROWN_UNDER, HEART_TINT = 0.70, 0.22, 0.75
HAZE_AMOUNT = 0.55  # VoxelConfig::haze is a config; this is the look at the defaults

PLANTS = {
    "bloomcrown": (0x752A58, 0xE04A96, 0xFFA85C),
    "umbrellafrond": (0x215A70, 0x33D2AE, 0xA6F5DC),
    "springturf": (0x2E4FB5, 0x42C5F8, 0xBFEBFF),
    "stonecushion": (0x57506E, 0xB9A8D6, 0xEFE6FF),
    "velvetpad": (0x2B1B6B, 0x7B5CF0, 0xC3B4FF),
}

# ---- the two consumers: candidate treatments -------------------------------------
# Each grazer treatment: (body front, body top, core, name). The core is the head's
# directional accent; cropping swaps it for the appearance doc's one warm accent.
WARM = 0xFF9B50
GRAZERS = {
    "A_dark_body_ice_core": (0x22285C, 0x4A5AA8, 0xE8FAFF),
    "B_slate_body_pink_core": (0x3A4A7E, 0x6E82C4, 0xFF2AFC),
    "C_pale_body_dark_core": (0x9AA6D8, 0xD6DEFF, 0x140F3A),
}
# Each glowcap treatment: (gills/front, cap glow, halo strength, name).
GLOWCAPS = {
    "A_foxfire_lime": (0x2C2160, 0xA8FF5C, 0.35),
    "B_family_pink": (0x2C2160, 0xFF8DF0, 0.35),
    "C_lime_no_halo": (0x2C2160, 0xA8FF5C, 0.0),
}
LOG_FRONT, LOG_TOP, LOG_DARK, LOG_RIM = 0x4E2846, 0x6E3E62, 0x3A1C36, 0x7A4A6E


def srgb_to_lin(v: int) -> np.ndarray:
    c = np.array([(v >> 16) & 255, (v >> 8) & 255, v & 255], dtype=np.float64) / 255.0
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def lin_to_srgb8(c: np.ndarray) -> np.ndarray:
    c = np.clip(c, 0.0, 1.0)
    s = np.where(c <= 0.0031308, c * 12.92, 1.055 * np.power(c, 1 / 2.4) - 0.055)
    return np.round(s * 255.0).astype(np.uint8)


def mix(a, b, t):
    return a * (1.0 - t) + b * t


def lum(c):
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]


C = {k: srgb_to_lin(v) for k, v in dict(
    sky=SKY, bedrock=BEDROCK, rock=ROCK, soil=SOIL, water_deep=WATER_DEEP,
    water_surface=WATER_SURFACE, light=LIGHT, haze=HAZE, warm=WARM,
    log_front=LOG_FRONT, log_top=LOG_TOP, log_dark=LOG_DARK, log_rim=LOG_RIM,
).items()}
C["sky"] = C["sky"] * SKY_BRIGHT


# ---- the strip --------------------------------------------------------------------
W, H, D = 44, 10, 8  # voxels
AIR, BED, ROCKM, SOILM = 0, 1, 2, 3


def build_terrain():
    m = np.zeros((D, H, W), dtype=np.int8)  # [z, y, x]
    for z in range(D):
        for x in range(W):
            # a gentle rise toward the back, a soil cap, a rock step at the right
            top = 3 + (z // 3) + (1 if 30 <= x < 40 else 0)
            for y in range(H):
                if y < 2:
                    m[z, y, x] = BED
                elif y < top - 1 or (30 <= x < 40 and y < top):
                    m[z, y, x] = ROCKM
                elif y < top:
                    m[z, y, x] = SOILM
    return m


def surface(m, x, z):
    for y in range(H - 1, -1, -1):
        if m[z, y, x] != AIR:
            return y
    return -1


# ---- projection and painter --------------------------------------------------------
class Painter:
    def __init__(self, s: int, tilt_deg: float = 30.0):
        self.s = s
        self.rise = max(1, int(round(s * math.tan(math.radians(tilt_deg)))))
        self.width = W * s
        self.height = H * s + (D - 1) * self.rise + 2 * s
        self.base = self.height
        self.fb = np.zeros((self.height, self.width, 3), dtype=np.float64)
        self.fb[:] = C["sky"]

    def front_row(self, y, z):
        return self.base - (y + 1) * self.s - z * self.rise

    def haze_at(self, z):
        return HAZE_AMOUNT * z / max(1, D - 1)

    def hazed(self, c, z):
        return mix(c, C["haze"], self.haze_at(z))

    def put(self, col, row, c):
        if 0 <= row < self.height:
            self.fb[row, col % self.width] = c

    def get(self, col, row):
        return self.fb[row, col % self.width]

    # a terrain block, per present.rs::block (front, rim, edge, chamfer; top, bevel)
    def block(self, m, x, y, z, body):
        s, rise = self.s, self.rise
        lit = mix(body * TOP_GAIN, C["light"], TOP_TINT)
        r0 = self.front_row(y, z)
        c0 = x * s
        nearer_solid = z > 0 and m[z - 1, y, x] != AIR
        open_up = m[z, y + 1, x] == AIR if y + 1 < H else True
        open_l = m[z, y, (x - 1) % W] == AIR
        open_r = m[z, y, (x + 1) % W] == AIR
        if not nearer_solid:
            for dy in range(s):
                for dx in range(s):
                    on_cap = dy == 0 and open_up
                    on_side = (dx == 0 and open_l) or (dx == s - 1 and open_r)
                    if on_cap and on_side:
                        c = lit
                    elif on_cap:
                        c = mix(body, lit, RIM)
                    elif on_side:
                        c = body * EDGE_DARK
                    else:
                        c = body
                    self.put(c0 + dx, r0 + dy, self.hazed(c, z))
        top_hidden = z > 0 and y + 1 < H and m[z - 1, y + 1, x] != AIR
        if open_up and not top_hidden:
            for dy in range(rise):
                zf = z + (rise - 1 - dy) / rise
                drop_l = open_l and (y == 0 or m[z, y - 1, (x - 1) % W] == AIR)
                drop_r = open_r and (y == 0 or m[z, y - 1, (x + 1) % W] == AIR)
                for dx in range(s):
                    on_drop = (dx == 0 and drop_l) or (dx == s - 1 and drop_r)
                    c = mix(lit, body, TOP_EDGE) if on_drop else lit
                    self.put(c0 + dx, r0 - rise + dy, mix(c, C["haze"], HAZE_AMOUNT * zf / (D - 1)))

    # a generic part: `front(dx)` and `cap(dx)` colours per pixel column, with the
    # plant rim rule; `mask(dx, dy)` may leave front pixels unpainted (a silhouette)
    def part(self, x, y, z, front, cap, covered_up=False, mask=None, cap_mask=None):
        s, rise = self.s, self.rise
        r0 = self.front_row(y, z)
        c0 = x * s
        for dy in range(s):
            for dx in range(s):
                if mask and not mask(dx, dy):
                    continue
                f = front(dx, dy)
                c = mix(f, cap(dx, 0), PLANT_RIM) if (dy == 0 and not covered_up) else f
                self.put(c0 + dx, r0 + dy, self.hazed(c, z))
        if not covered_up:
            for dy in range(rise):
                zf = z + (rise - 1 - dy) / rise
                for dx in range(s):
                    if cap_mask and not cap_mask(dx, dy):
                        continue
                    self.put(c0 + dx, r0 - rise + dy, mix(cap(dx, dy), C["haze"], HAZE_AMOUNT * zf / (D - 1)))

    def image(self, scale: int) -> Image.Image:
        im = Image.fromarray(lin_to_srgb8(self.fb), "RGB")
        return im.resize((im.width * scale, im.height * scale), Image.NEAREST)


def plant_lit(c):
    return mix(c * PLANT_TOP_GAIN, C["light"], PLANT_TOP_TINT)


def trunk_shade(dx, s):
    u = (dx + 0.5) / s
    t = max(0.0, min(1.0, 1.0 - abs(u - TRUNK_LIGHT_AT) / 0.65))
    return TRUNK_SHADE[0] + (TRUNK_SHADE[1] - TRUNK_SHADE[0]) * t


# ---- organisms ------------------------------------------------------------------------
def draw_plant(p: Painter, m, name, x, z, height, radius, fill=1.0):
    wood, crown, heart = (srgb_to_lin(v) for v in PLANTS[name])
    crown = mix(wood, crown, 0.15 + 0.85 * fill)
    y0 = surface(m, x, z)
    s = p.s
    for k in range(1, height):
        y = y0 + k
        p.part(x, y, z,
               lambda dx, dy: wood * trunk_shade(dx, s),
               lambda dx, dy: mix(plant_lit(wood), wood * trunk_shade(dx, s), TOP_EDGE),
               covered_up=True)
    top = y0 + height
    span = int(math.floor(radius))
    cells = [(dx, dz) for dz in range(-span, span + 1) for dx in range(-span, span + 1)
             if dx * dx + dz * dz <= radius * radius]
    for dx_, dz_ in sorted(cells, key=lambda c: -c[1]):
        cx, cz = x + dx_, z + dz_
        if not (0 <= cz < D):
            continue
        is_heart = dx_ == 0 and dz_ == 0
        open_l = (dx_ - 1, dz_) not in cells
        open_r = (dx_ + 1, dz_) not in cells

        def front(dx, dy, il=is_heart, ol=open_l, orr=open_r):
            base = mix(crown, heart, HEART_TINT * fill) if (il and s // 2 - 1 <= dx <= s // 2) else crown
            k = CROWN_EDGE if ((dx == 0 and ol) or (dx == s - 1 and orr)) else 1.0
            return mix(base, wood, CROWN_UNDER) * k

        def cap(dx, dy, il=is_heart, ol=open_l, orr=open_r):
            base = mix(crown, heart, HEART_TINT * fill) if (il and s // 2 - 1 <= dx <= s // 2) else crown
            k = CROWN_EDGE if ((dx == 0 and ol) or (dx == s - 1 and orr)) else 1.0
            return plant_lit(base) * k

        p.part(cx, top, cz, front, cap)


def draw_log(p: Painter, m, x, z, length=3):
    """Dead wood as a lying log: one voxel high, `length` voxels along x, a dull plum
    cylinder with a bark rim and a dark underside; its top is not ground-lit."""
    s = p.s
    for i in range(length):
        cx = x + i
        y = surface(m, cx, z) + 1
        end_l, end_r = i == 0, i == length - 1

        def front(dx, dy, el=end_l, er=end_r):
            t = dy / max(1, s - 1)
            c = mix(C["log_rim"], C["log_dark"], t)  # the cylinder: bright top, dark belly
            if (dx == 0 and el) or (dx == s - 1 and er):
                c = C["log_dark"]  # the cut end
            return c

        def cap(dx, dy):
            return C["log_top"]

        p.part(cx, y, z, front, cap)


def draw_glowcap(p: Painter, m, x, z, treatment, fill=1.0, wilt=0.0, on_log=True):
    gills, glow, halo = GLOWCAPS[treatment]
    gills, glow = srgb_to_lin(gills), srgb_to_lin(glow)
    if wilt > 0:
        g = lum(glow)
        glow = mix(glow, np.array([g, g, g]), 0.5 * wilt) * (1.0 - 0.25 * wilt)
    s, rise = p.s, p.rise
    y = surface(m, x, z) + (2 if on_log else 1)
    # caps by fill: at 4 px one or two caps 2 px wide; at 8 px one to three, 2 px wide
    if s <= 4:
        caps = [(1, 2)] if fill < 0.5 else [(0, 1), (2, 3)]
    else:
        caps = [(3, 4)] if fill < 0.34 else ([(1, 2), (5, 6)] if fill < 0.67 else [(0, 1), (3, 4), (6, 7)])

    def in_cap(dx):
        return any(a <= dx <= b for a, b in caps)

    def front(dx, dy):
        # the gill face: dark, a lit rim only under a cap
        return gills * (1.15 if in_cap(dx) else 0.85)

    def cap(dx, dy):
        return glow if in_cap(dx) else gills

    p.part(x, y, z, front, cap)
    if halo > 0:
        # the paid glow: the one-pixel ring around the cap's top face, lifted toward the
        # cap colour, painted over whatever ground or log is already there
        r0 = p.front_row(y, z) - rise
        c0 = x * s
        for col in range(c0 - 1, c0 + s + 1):
            for row in range(r0 - 1, r0 + rise + 1):
                inside = c0 <= col < c0 + s and r0 <= row < r0 + rise
                if inside:
                    continue
                p.put(col, row, mix(p.get(col, row), glow, halo * fill))


def draw_grazer(p: Painter, m, x, z, treatment, facing=1, cropping=False, starve=0.0):
    """A low broad body: 2 voxels along x, 1 high, 2 deep (z, z+1); the head is the
    leading x cell; its core sits on the cap's leading half."""
    body, top, core = (srgb_to_lin(v) for v in GRAZERS[treatment])
    if starve > 0:
        for arr in (body, top):
            g = lum(arr)
            arr[:] = mix(arr, np.array([g, g, g]), 0.5 * starve) * (1.0 - 0.3 * starve)
    core_c = C["warm"] if cropping else core
    s = p.s
    y = surface(m, x, z) + 1
    cells = [(x + i, z + j) for j in (1, 0) for i in (0, 1)]  # far row first
    for cx, cz in cells:
        lead = (cx == x + 1) if facing > 0 else (cx == x)
        left_end = cx == x
        right_end = cx == x + 1

        def mask(dx, dy, le=left_end, re=right_end):
            # a pill: the outer corner pixels of the end cells are not painted
            corner = (dy == 0 or dy == s - 1) and ((dx == 0 and le) or (dx == s - 1 and re))
            return not corner

        def front(dx, dy, le=left_end, re=right_end):
            t = dy / max(1, s - 1)
            c = mix(top, body, 0.35 + 0.65 * t)  # belly darker than back
            if (dx == 0 and le) or (dx == s - 1 and re):
                c = c * 0.8
            return c

        def cap(dx, dy, ld=lead, cz_=cz):
            # the head core: the leading half of the leading cell's cap, front row (nearer z)
            if ld and cz_ == z and dy >= p.rise - 2:
                if (facing > 0 and dx >= s - max(2, s // 3)) or (facing < 0 and dx < max(2, s // 3)):
                    return core_c
            # the spine: the back row of every cap is lifted, so the body reads as a
            # low back seen from above rather than a block seen from the front
            if dy == 0 and cz_ == z + 1:
                return mix(top, C["light"], 0.18)
            return top

        def front_head(dx, dy, ld=lead, cz_=cz, le=left_end, re=right_end):
            c = front(dx, dy, le, re)
            # cropping: the muzzle — the head cell's front rim row — goes warm with the core
            if cropping and ld and cz_ == z and dy == 0:
                return C["warm"]
            return c

        p.part(cx, y, cz, front_head, cap, mask=mask)


# ---- scenes -----------------------------------------------------------------------------
def scene(s, grazer="A_dark_body_ice_core", glowcap="A_foxfire_lime", cropping=False,
          starve=0.0, fill=1.0, wilt=0.0):
    m = build_terrain()
    p = Painter(s)
    for z in range(D - 1, -1, -1):
        for y in range(H):
            for x in range(W):
                mat = m[z, y, x]
                if mat == AIR:
                    continue
                body = C["bedrock"] if mat == BED else C["rock"] if mat == ROCKM else C["soil"]
                p.block(m, x, y, z, body)
        # organisms in this slab, drawn after the slab's terrain (painter's order)
        for name, (x, zz, h, r) in PLANT_SITES.items():
            if zz == z:
                draw_plant(p, m, name, x, zz, h, r)
        if z == LOG_Z:
            draw_log(p, m, LOG_X, LOG_Z)
            draw_glowcap(p, m, LOG_X + 1, LOG_Z, glowcap, fill=fill, wilt=wilt)
            draw_glowcap(p, m, LOG_X + 2, LOG_Z, glowcap, fill=max(0.0, fill - 0.4), wilt=wilt)
        if z == GRAZER_Z:
            draw_grazer(p, m, GRAZER_X, GRAZER_Z, grazer, facing=1, cropping=cropping, starve=starve)
    return p


PLANT_SITES = {
    "bloomcrown": (4, 4, 3, 1.5),
    "umbrellafrond": (12, 5, 4, 2.0),
    "springturf": (19, 2, 1, 0.5),
    "stonecushion": (34, 3, 1, 0.5),
    "velvetpad": (25, 3, 1, 1.5),
}
LOG_X, LOG_Z = 27, 1
GRAZER_X, GRAZER_Z = 16, 1  # beside the springturf it crops


def sheet(images, pad=8, bg=(10, 6, 30)):
    w = sum(im.width for im in images) + pad * (len(images) + 1)
    h = max(im.height for im in images) + 2 * pad
    out = Image.new("RGB", (w, h), bg)
    x = pad
    for im in images:
        out.paste(im, (x, pad))
        x += im.width + pad
    return out


def main(outdir: str):
    os.makedirs(outdir, exist_ok=True)
    scene(4).image(6).save(os.path.join(outdir, "recommended_px4.png"))
    scene(8).image(3).save(os.path.join(outdir, "recommended_px8.png"))
    rows = []
    for s, k in ((4, 6), (8, 3)):
        rows.append(sheet([scene(s, grazer=g).image(k) for g in GRAZERS]))
    stack(rows).save(os.path.join(outdir, "variants_grazer.png"))
    rows = []
    for s, k in ((4, 6), (8, 3)):
        rows.append(sheet([scene(s, glowcap=g).image(k) for g in GLOWCAPS]))
    stack(rows).save(os.path.join(outdir, "variants_glowcap.png"))
    rows = []
    for s, k in ((4, 6), (8, 3)):
        rows.append(sheet([
            scene(s).image(k),
            scene(s, cropping=True).image(k),
            scene(s, starve=1.0, fill=0.4, wilt=1.0).image(k),
        ]))
    stack(rows).save(os.path.join(outdir, "states.png"))


def stack(images, pad=8, bg=(10, 6, 30)):
    w = max(im.width for im in images) + 2 * pad
    h = sum(im.height for im in images) + pad * (len(images) + 1)
    out = Image.new("RGB", (w, h), bg)
    y = pad
    for im in images:
        out.paste(im, (pad, y))
        y += im.height + pad
    return out


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "out"))
