#!/usr/bin/env python3
"""Voxel storyboard: a hand-authored strip rendered as pixel art at 1920x1080.

Disposable art study, Package C of
design/handoffs/voxel-first-wave-briefs-2026-09-16.md.  No simulation: a small
3D occupancy array is authored in code, then projected the way the presenter is
meant to project it, so the owner can pick a camera tilt, a pixel-per-voxel
scale and a habitat depth by looking.

The voxels are the model, not the look.  Nothing is drawn as a cube.  Per
column of each depth slab we draw only the top face of an exposed surface run
and the -z-facing cut wall beneath it; the crest is autotiled between integer
neighbour heights so a one-voxel stair step becomes an s-pixel diagonal while a
real cliff stays a cliff; strata undulate so the layering does not read as
ruled lines; there are no per-voxel outlines and no per-voxel noise.

Projection (from the brief):
    sx = x * s
    sy = base - y * s - z * s * tan(tilt)
drawn back to front (largest z first).

Palette: design/appearance.md and crates/cubarium/src/present.rs.

Usage:  python3 storyboard.py [outdir]
"""

from __future__ import annotations

import math
import os
import sys

import numpy as np
from PIL import Image

SCREEN_W = 1920
SCREEN_H = 1080
PAD = 64  # row padding on the strata tables, for the per-column undulation

ATLAS_DIR = os.path.normpath(
    os.path.join(
        os.path.dirname(os.path.abspath(__file__)),
        "..",
        "..",
        "..",
        "assets",
        "atelier",
    )
)


def rgb(v: int) -> np.ndarray:
    return np.array([(v >> 16) & 0xFF, (v >> 8) & 0xFF, v & 0xFF], dtype=np.float32)


# ---- present.rs constants -------------------------------------------------
FLOOR = rgb(0x12093A)
PRODUCER_LOW = rgb(0x1E2798)
PRODUCER_HIGH = rgb(0x42C5F8)
DETRITUS = rgb(0x510B6D)
WARM = rgb(0xFF9B50)

# ---- derived scene colors, same indigo/violet/cyan family -----------------
SKY_TOP = rgb(0x040310)
SKY_HORIZON = rgb(0x120A2C)
SKY_GLOW = rgb(0x2B1048)
HAZE = rgb(0x221856)

BEDROCK_DEEP = rgb(0x0B0722)
BEDROCK = rgb(0x110B30)
ROCK_A = rgb(0x18113C)
ROCK_B = rgb(0x1F1550)
ROCK_C = rgb(0x241A5C)
SEAM = rgb(0x271B5E)

SOIL_A = rgb(0x3A2356)
SOIL_B = rgb(0x2C1A48)
SOIL_C = rgb(0x21143A)

ROCK_LIT = rgb(0x312764)
GROUND_LIT = rgb(0x5C3E88)

WATER_SURF = rgb(0x2F6BD2)
WATER_CREST = rgb(0x6FB6EE)
WATER_DEEP = rgb(0x120A46)
FOAM = rgb(0xCBE9F7)
BLACK = np.zeros(3, dtype=np.float32)

SHADE_LEVELS = (1.0, 0.70, 0.42)

# ---- authored terrain, as fractions of the vertical span ------------------
PLAIN = 0.200
WATER_LEVEL = 0.138

TERRACE_U0, TERRACE_U1 = 0.362, 0.470
TERRACE_LO, TERRACE_SPAN = 0.300, 0.024

GORGE_U0, GORGE_U1 = 0.434, 0.514
FALL_Z = 5  # the slab that keeps its height: the cliff the stream falls over
FALL_U = 0.452

SHELF_U0, SHELF_U1 = 0.684, 0.760  # the overhang

SPIRETREE_U, SPIRETREE_Z = 0.380, 2
GLOWCAP_U0, GLOWCAP_U1 = 0.598, 0.658
CREATURE_NEAR_U, CREATURE_NEAR_Z = 0.792, 1
CREATURE_FAR_U = 0.792  # same column: only depth separates them


def wrapd(d: float) -> float:
    return (d + 0.5) % 1.0 - 0.5


def bump(u: float, c: float, w: float) -> float:
    d = wrapd(u - c) / w
    return math.exp(-d * d)


def smoothstep(t: float) -> float:
    t = max(0.0, min(1.0, t))
    return t * t * (3.0 - 2.0 * t)


def band_weight(uw: float, u0: float, u1: float, soft: float) -> float:
    """1 inside [u0,u1], tapering to 0 over `soft` at each edge."""
    if uw < u0 - soft or uw > u1 + soft:
        return 0.0
    a = smoothstep((uw - (u0 - soft)) / soft) if uw < u0 else 1.0
    b = smoothstep(((u1 + soft) - uw) / soft) if uw > u1 else 1.0
    return min(a, b)


def profile(u: float, t: float) -> float:
    """Surface height as a fraction of the vertical span.  Periodic in u.

    `t` is z/(depth-1): back slabs sit a little higher and a little shifted, so
    receding ridges peek over the nearer ones instead of hiding behind them.
    """
    us = u + 0.020 * t
    h = PLAIN
    h += 0.400 * bump(us, 0.270, 0.100)  # the broad ridge
    h += 0.095 * bump(us, 0.145, 0.085)  # its shoulder
    h += 0.150 * bump(us, 0.880, 0.072)  # a second ridge, wraps through x = 0
    h += 0.105 * bump(us, 0.722, 0.058)  # the hill that carries the overhang
    h -= 0.118 * bump(us, 0.575, 0.086) * (1.0 - 0.58 * t)  # the hollow
    h -= 0.030 * bump(us, 0.500, 0.034)  # its lip toward the gorge
    # weak correlated noise, last and small
    h += 0.0100 * math.sin(us * 2 * math.pi * 3.0 + 1.3 + 0.8 * t)
    h += 0.0065 * math.sin(us * 2 * math.pi * 7.0 + 2.1 - 1.1 * t)
    h += 0.0040 * math.sin(us * 2 * math.pi * 13.0 + 0.4 + 2.0 * t)
    h *= 1.0 + 0.070 * t
    # the terrace: a flat step cut into the ridge's right flank
    w = band_weight(u % 1.0, TERRACE_U0, TERRACE_U1, 0.022)
    if w > 0.0:
        lvl = TERRACE_LO + 0.012 * t
        flat = min(max(h, lvl), lvl + TERRACE_SPAN)
        h = h * (1.0 - w) + flat * w
    return h


class Scene:
    """Integer voxel occupancy for one (depth, px_per_voxel) cell."""

    def __init__(self, s: int, depth: int):
        self.s = s
        self.depth = depth
        self.w = SCREEN_W // s
        self.span = SCREEN_H / s  # vertical extent of the world, in voxels
        self.wl = int(round(WATER_LEVEL * self.span))

        w, d = self.w, depth
        self.top = np.ones((d, w), dtype=np.int32)
        self.shelf0 = np.zeros((d, w), dtype=np.int32)
        self.shelf1 = np.zeros((d, w), dtype=np.int32)
        self.shade = np.zeros((d, w), dtype=np.int32)
        self.rich = np.zeros((d, w), dtype=np.float32)
        self.stream = np.zeros((d, w), dtype=bool)
        self.bare = np.zeros((d, w), dtype=bool)

        gorge_floor = self.wl - max(2, int(0.013 * self.span))
        th = max(3, int(round(0.024 * self.span)))  # overhang thickness
        gap = max(6, int(round(0.062 * self.span)))  # the recess under it
        shelf_mid = 0.5 * (SHELF_U0 + SHELF_U1)
        zmax = max(2, int(round(min(d - 2, 0.45 * d))))

        for z in range(d):
            t = z / max(1, d - 1)
            for x in range(w):
                uw = (x + 0.5) / w
                hf = profile(uw, t) * self.span
                # the gorge: the nearer slabs are cut to a wet channel, so the
                # cliff at z = FALL_Z shows its face and the stream is visible
                if z < FALL_Z:
                    g = band_weight(uw, GORGE_U0, GORGE_U1, 0.045)
                    floor = gorge_floor + 0.9 * z
                    if g > 0.0 and hf > floor:
                        hf = hf * (1.0 - g) + floor * g
                h = int(round(hf))
                # the overhang: the top `th` voxels of the hill stay, a recess
                # of `gap` is carved beneath them over the nearer slabs
                sw = band_weight(uw, SHELF_U0, SHELF_U1, 0.020)
                if sw > 0.0:
                    zs = int(round(2 + (zmax - 2) * sw * bump(uw, shelf_mid, 0.046)))
                    dry = self.wl + th + gap + 4
                    if z <= zs and h > dry:
                        self.shelf0[z, x] = h - th
                        self.shelf1[z, x] = h
                        h = h - th - gap
                        self.shade[z, x] = 2
                    elif z <= zs + 4 and h > dry:
                        self.shade[z, x] = 2  # the back wall of the recess
                self.top[z, x] = max(1, h)

        # a soft cast shadow on the ground just downhill of the overhang lip
        for z in range(d):
            for x in range(w):
                uw = (x + 0.5) / w
                if self.shade[z, x] == 0 and SHELF_U1 < uw < SHELF_U1 + 0.020:
                    self.shade[z, x] = 1

        # the stream: a one-slab thread meandering along the terrace top and
        # straightening as it reaches the lip the fall goes over
        for x in range(w):
            uw = (x + 0.5) / w
            if not (0.366 <= uw <= FALL_U + 0.006):
                continue
            lip = smoothstep((uw - 0.424) / 0.030)
            meander = 2.1 * math.sin(uw * 260.0) * (1.0 - lip)
            zc = int(round(FALL_Z + 1.4 * (1.0 - lip) + meander))
            if 0 <= zc < d:
                self.stream[zc, x] = True

        # standing crop: large low-frequency patches, wettest near the pool,
        # nothing on steep faces.  Patch size, not instance count, per the
        # landscape review's visual finding.
        for z in range(d):
            t = z / max(1, d - 1)
            for x in range(w):
                uw = (x + 0.5) / w
                h = int(self.top[z, x])
                above = (h - self.wl) / max(6.0, 0.055 * self.span)
                wet = max(0.0, 1.0 - max(0.0, above))
                slope = abs(
                    int(self.top[z, (x + 1) % w]) - int(self.top[z, (x - 1) % w])
                )
                steep = min(1.0, slope / 5.0)
                p = (
                    0.55
                    + 0.30 * math.sin(uw * 2 * math.pi * 2.0 + 0.7 + 1.3 * t)
                    + 0.22 * math.sin(uw * 2 * math.pi * 5.0 - 1.1 + 0.6 * t)
                )
                p = max(0.0, min(1.0, (p - 0.34) / 0.52))
                r = (0.12 + 0.88 * wet) * p * (1.0 - 0.9 * steep)
                if h <= self.wl:
                    r *= 0.2
                self.rich[z, x] = max(0.0, min(1.0, r))
                self.bare[z, x] = steep > 0.55


# ---- sprites --------------------------------------------------------------


def load_atlases():
    return (
        Image.open(os.path.join(ATLAS_DIR, "tall.png")).convert("RGBA"),
        Image.open(os.path.join(ATLAS_DIR, "plants.png")).convert("RGBA"),
        Image.open(os.path.join(ATLAS_DIR, "creatures.png")).convert("RGBA"),
    )


def tile(atlas: Image.Image, row: int, frame: int, size: int) -> np.ndarray:
    """One 16 px atlas cell, nearest-scaled to `size` px, as float RGBA."""
    cell = atlas.crop((frame * 16, row * 16, frame * 16 + 16, row * 16 + 16))
    if size != 16:
        cell = cell.resize((size, size), Image.NEAREST)
    return np.asarray(cell, dtype=np.float32)


# ---- renderer -------------------------------------------------------------


class Frame:
    def __init__(self, scene: Scene, tilt_deg: float):
        self.sc = scene
        self.s = scene.s
        self.tilt = tilt_deg
        self.dz = scene.s * math.tan(math.radians(tilt_deg))
        self.base = float(SCREEN_H)
        self.fb = np.zeros((SCREEN_H, SCREEN_W, 3), dtype=np.float32)
        self._strata()
        self._wobble()

    # -- precomputation --

    def _wobble(self):
        """Per-pixel-column row offset for the strata, so the layering reads as
        rock beds rather than as ruled lines across the frame."""
        x = np.arange(SCREEN_W, dtype=np.float32)
        # whole numbers of cycles across the strip, so the beds meet at x = 0
        k = 2.0 * math.pi / SCREEN_W
        w = (
            2.2 * np.sin(x * k * 6)
            + 1.3 * np.sin(x * k * 15 + 2.0)
            + 0.7 * np.sin(x * k * 34 - 1.1)
        )
        self.wob = np.round(w * self.s * 0.9).astype(np.int32)

    def _strata(self):
        span = self.sc.span
        n = int(span) + 96
        raw = np.zeros((n, 3), dtype=np.float32)
        beds = (
            (0.055, BEDROCK_DEEP),
            (0.118, BEDROCK),
            (0.228, ROCK_A),
            (0.306, ROCK_B),
            (0.432, ROCK_A * 0.92),
            (0.524, ROCK_C),
            (9.999, ROCK_A),
        )
        for y in range(n):
            f = y / span
            for lim, c in beds:
                if f < lim:
                    raw[y] = c
                    break
        for lim, _ in beds[:-1]:
            y = int(round(lim * span))
            if 0 <= y < n:
                raw[y] = SEAM * 0.45 + raw[y] * 0.55
        self.strata_raw = raw

        rows = np.arange(-PAD, SCREEN_H + PAD, dtype=np.float32) + 0.5
        self.front = []
        for z in range(self.sc.depth):
            a = self.haze(z)
            yv = np.clip(
                ((self.base - z * self.dz - rows) / self.s).astype(np.int32), 0, n - 1
            )
            self.front.append(raw[yv] * (1.0 - a) + HAZE * a)

    def haze(self, z: int) -> float:
        return min(0.74, 1.0 - math.exp(-z / 34.0))

    def fade(self, c: np.ndarray, z: int) -> np.ndarray:
        a = self.haze(z)
        return c * (1.0 - a) + HAZE * a

    # -- primitives --

    def sky(self):
        horizon = 0.44 * SCREEN_H
        for r in range(SCREEN_H):
            t = min(1.0, r / horizon)
            t = math.floor(t * 13.0) / 13.0
            c = SKY_TOP * (1.0 - t) + SKY_HORIZON * t
            if t > 0.75:
                g = (t - 0.75) / 0.25
                c = c * (1.0 - 0.20 * g) + SKY_GLOW * (0.20 * g)
            self.fb[r, :] = c

    def fill(self, x: int, r0: float, r1: float, color):
        a = max(0, int(round(r0)))
        b = min(SCREEN_H, int(round(r1)))
        if b <= a or not (0 <= x < SCREEN_W):
            return
        self.fb[a:b, x] = color

    def fill_wall(self, x: int, r0: float, r1: float, table: np.ndarray):
        """The -z-facing cut wall, with the strata undulating per column."""
        a = max(0, int(round(r0)))
        b = min(SCREEN_H, int(round(r1)))
        if b <= a or not (0 <= x < SCREEN_W):
            return
        off = int(self.wob[x])
        self.fb[a:b, x] = table[a + PAD + off : b + PAD + off]

    def blend(self, x: int, r0: float, r1: float, color, alpha: float):
        a = max(0, int(round(r0)))
        b = min(SCREEN_H, int(round(r1)))
        if b <= a or not (0 <= x < SCREEN_W):
            return
        seg = self.fb[a:b, x]
        seg *= 1.0 - alpha
        seg += color * alpha

    # -- one surface run --

    def crest(self, z: int, x: int, h: int, hl: int, hr: int):
        """Autotiled crest: fractional surface height per pixel column.

        Neighbour differences are clamped to one voxel, so a single stair step
        becomes an s-pixel diagonal and a multi-voxel cliff stays sharp.
        """
        s = self.s
        dl = hl - h
        dr = hr - h
        if abs(dl) > 1:
            dl = 0
        if abs(dr) > 1:
            dr = 0
        out = []
        for j in range(s):
            u = (j + 0.5) / s - 0.5
            hi = h + (-u * dl if u < 0 else u * dr)
            out.append(self.base - hi * s - z * self.dz)
        return out

    def top_color(self, z: int, x: int) -> np.ndarray:
        sc = self.sc
        r = float(sc.rich[z, x])
        if sc.bare[z, x]:
            return ROCK_LIT * (1.05 + 0.25 * r)
        veg = PRODUCER_LOW * (1.0 - r) + PRODUCER_HIGH * r
        lift = 0.34 + 0.66 * r * r
        return GROUND_LIT * (1.0 - r) * 0.70 + veg * lift * 1.35

    def shadow(self, px: int, yc: float, amt: float):
        """Occlusion under an overhang: strongest at the surface, dying out a
        few voxels down, so a shaded column never becomes a black rectangle."""
        s = self.s
        self.blend(px, yc - self.dz - 1, yc + 2 * s, BLACK, amt)
        self.blend(px, yc + 2 * s, yc + 5 * s, BLACK, amt * 0.52)
        self.blend(px, yc + 5 * s, yc + 9 * s, BLACK, amt * 0.20)

    def run(
        self,
        z: int,
        x: int,
        h: int,
        hl: int,
        hr: int,
        top_c,
        cap: int,
        bottom: float | None = None,
        shade: int = 0,
        grad: float = 0.0,
    ):
        """Draw one solid run's top face, soil cap and -z cut wall."""
        s = self.s
        table = self.front[z]
        soil = [self.fade(c, z) for c in (SOIL_A, SOIL_B, SOIL_C)]
        soil[0] = soil[0] * 0.28 + np.asarray(top_c, dtype=np.float32) * 0.72
        lip = table[0] if cap == 0 else self.fade(SOIL_C, z)
        ys = self.crest(z, x, h, hl, hr)
        bot = SCREEN_H + 1.0 if bottom is None else bottom
        amt = 1.0 - SHADE_LEVELS[shade]
        for j in range(s):
            px = x * s + j
            yc = ys[j]
            self.fill_wall(px, yc + grad + cap * s, bot, table)
            for k in range(cap - 1, -1, -1):
                self.fill(
                    px,
                    min(yc + grad + k * s, bot),
                    min(yc + grad + (k + 1) * s, bot),
                    soil[min(k, 2)],
                )
            b = int(round(yc))
            a = min(b - 1, int(round(yc - self.dz)))
            self.fill(px, a, b, top_c)
            if grad > 0.0:
                self.fill(px, b, b + grad, np.asarray(top_c, np.float32) * 0.88)
            # the contour line belongs to a real face, not to a one-voxel
            # z-step that the gradient already smoothed away
            if cap > 0 and grad <= 0.0:
                self.fill(px, b, b + 1, lip)
            if amt > 0.0:
                self.shadow(px, yc, amt)
        return ys

    # -- one depth slab --

    def draw_slab(self, z: int):
        sc = self.sc
        s, w = self.s, sc.w
        wl = sc.wl
        wsurf = self.fade(WATER_SURF, z)
        wcrest = self.fade(WATER_CREST, z)
        wdeep = self.fade(WATER_DEEP, z)

        for x in range(w):
            sh = int(sc.shade[z, x])
            h = int(sc.top[z, x])
            hl = int(sc.top[z, (x - 1) % w])
            hr = int(sc.top[z, (x + 1) % w])

            tc = self.top_color(z, x)
            if sc.stream[z, x]:
                tc = tc * 0.34 + (WATER_SURF * 0.72 + WATER_CREST * 0.28) * 0.66
            tc = self.fade(tc, z)
            # how much of this slab's surface the nearer slab leaves visible
            if z == 0:
                grad = 0.0
            else:
                expose = s * (h - int(sc.top[z - 1, x])) + self.dz - self.dz
                t = smoothstep(expose / (3.0 * s))
                grad = max(0.0, min(expose, 1.3 * s)) * (1.0 - t)
            ys = self.run(
                z, x, h, hl, hr, tc, 0 if sc.bare[z, x] else 3, shade=sh, grad=grad
            )

            # the overhang shelf: a second solid run above the recess
            if sc.shelf1[z, x] > 0:
                sh1 = int(sc.shelf1[z, x])
                sl = int(sc.shelf1[z, (x - 1) % w]) or sh1
                sr = int(sc.shelf1[z, (x + 1) % w]) or sh1
                s0 = int(sc.shelf0[z, x])
                ybot = self.base - s0 * s - z * self.dz
                stc = self.fade(self.top_color(z, x), z)
                self.run(z, x, sh1, sl, sr, stc, 2, bottom=ybot)
                for j in range(s):
                    px = x * s + j
                    self.blend(px, ybot - 1, ybot + 1, BLACK, 0.5)

            # ---- free water ----
            if h < wl:
                yw = self.base - wl * s - z * self.dz
                d = (wl - h) / max(6.0, 0.055 * sc.span)
                for j in range(s):
                    px = x * s + j
                    self.blend(px, yw, ys[j], wdeep, min(0.84, 0.32 + 0.55 * d))
                    b = int(round(yw))
                    a = min(b - 1, int(round(yw - self.dz)))
                    self.blend(px, a, b, wsurf, 0.78)
                # ripple dashes, clustered by a smooth function so they read as
                # a surface and not as scattered noise
                g = math.sin(x * 0.41 + z * 2.3) * math.sin(x * 0.093 - z * 1.7)
                if g > 0.80:
                    b = int(round(yw))
                    n = max(2, int(2 + 3 * (g - 0.80) / 0.20) * s // 2)
                    for px in range(x * s, x * s + n):
                        self.blend(px, b - 1, b, wcrest, 0.38)

        # ---- the stream falling over the cliff at z = FALL_Z ----
        if FALL_Z <= z <= FALL_Z + 1:
            xc = int(FALL_U * w)
            ytop = self.base - int(sc.top[z, xc]) * s - z * self.dz
            ybot = self.base - wl * s - z * self.dz
            half = max(3, int(round(2.3 * s)))
            core = 0.84 if z == FALL_Z else 0.40
            span = max(1.0, ybot - ytop)
            for px in range(xc * s - half, xc * s + half + 1):
                e = abs(px - xc * s) / float(half + 0.5)
                # a slight flare, so the sheet is neither a bar nor a spike
                top = ytop
                self.blend(px, top, ybot, wsurf, core * (1.0 - 0.28 * e))
                # two low-contrast streaks in the sheet
                if abs(e - 0.22) < 0.09 or abs(e - 0.62) < 0.07:
                    self.blend(px, top + 0.08 * span, ybot, wcrest, 0.22 * core)
                # a short brighter run where it leaves the lip
                self.blend(px, top, top + 0.05 * span, wcrest, 0.26 * core)

        # ---- splash and ripple rings, at the front so nothing hides them ----
        if z <= 1:
            xc = int(FALL_U * w)
            yw = self.base - wl * s - z * self.dz
            half = max(2, int(round(1.6 * s)))
            for px in range(xc * s - half, xc * s + half + 1):
                e = abs(px - xc * s) / float(half)
                h = (1.0 - e) ** 3
                self.blend(px, yw - h * 1.8 * s, yw, FOAM, 0.26 * h)
            for rad in (3, 5):
                for k in (-rad, rad):
                    x0 = xc * s + k * s
                    for px in range(x0, x0 + max(2, s - 1)):
                        self.blend(px, yw - 1, yw, wcrest, 0.36 - 0.035 * rad)

    # -- sprites --

    def paste(self, spr, x: float, y: float, z: int, warm=False, lift=1.0):
        h, w = spr.shape[0], spr.shape[1]
        x0, y0 = int(round(x)), int(round(y))
        sx0, sy0 = max(0, x0), max(0, y0)
        sx1, sy1 = min(SCREEN_W, x0 + w), min(SCREEN_H, y0 + h)
        if sx1 <= sx0 or sy1 <= sy0:
            return
        sub = spr[sy0 - y0 : sy1 - y0, sx0 - x0 : sx1 - x0]
        col = sub[:, :, :3].copy()
        alpha = (sub[:, :, 3:4] / 255.0).astype(np.float32)
        col *= lift
        if warm and alpha.any():
            lum = col.sum(axis=2)
            vis = alpha[:, :, 0] > 0
            cut = np.percentile(lum[vis], 84)
            m = vis & (lum >= cut)
            col[m] = WARM
        a = self.haze(z)
        col = col * (1.0 - a) + HAZE * a
        dst = self.fb[sy0:sy1, sx0:sx1]
        dst *= 1.0 - alpha
        dst += col * alpha

    def glow(self, spr, x: float, y: float, z: int):
        """A one-pixel additive halo, for the glowcaps only.  Not a general
        bright outline: appearance.md rules those out."""
        h, w = spr.shape[0], spr.shape[1]
        x0, y0 = int(round(x)) - 1, int(round(y)) - 1
        a = (spr[:, :, 3] > 0).astype(np.float32)
        pad = np.zeros((h + 2, w + 2), dtype=np.float32)
        for dy in (0, 1, 2):
            for dx in (0, 1, 2):
                pad[dy : dy + h, dx : dx + w] += a
        pad = np.clip(pad, 0, 1) * 0.16
        sx0, sy0 = max(0, x0), max(0, y0)
        sx1, sy1 = min(SCREEN_W, x0 + w + 2), min(SCREEN_H, y0 + h + 2)
        if sx1 <= sx0 or sy1 <= sy0:
            return
        m = pad[sy0 - y0 : sy1 - y0, sx0 - x0 : sx1 - x0][:, :, None]
        self.fb[sy0:sy1, sx0:sx1] += self.fade(PRODUCER_HIGH, z) * m

    def surface_y(self, x: int, z: int) -> float:
        return self.base - int(self.sc.top[z, x]) * self.s - z * self.dz

    def draw_sprites(self, z: int, atlases):
        tall, plants, creatures = atlases
        sc = self.sc
        s, w = self.s, sc.w
        size = max(4, int(round(16 * s / 4.0)))  # px_per_voxel/4 of a 16 px tile
        k = size / 16.0

        if z == SPIRETREE_Z:
            x = int(SPIRETREE_U * w)
            yb = self.surface_y(x, z) + self.dz * 0.5
            left = x * s - size // 2
            self.paste(tile(tall, 0, 0, size), left, yb - size, z, lift=1.25)
            ty = yb - size - 12 * k
            for i in range(2):
                self.paste(tile(tall, 1, 0, size), left, ty, z, lift=1.25)
                ty -= 14 * k
            self.paste(tile(tall, 2, 0, size), left, ty, z, lift=1.25)

        # the glowcap patch on the pool shore, over several depths
        if z in (1, 2, 4, 6, min(9, sc.depth - 1)):
            x0, x1 = int(GLOWCAP_U0 * w), int(GLOWCAP_U1 * w)
            step = max(2, int(round(2.6 * 4 / s)))
            for i, x in enumerate(range(x0, x1, step)):
                if int(sc.top[z, x]) <= sc.wl:
                    continue
                if ((x * 1103515245 + z * 12345) >> 7) % 5 == 0:
                    continue
                row = (2, 1, 2, 0, 2)[(i + z) % 5]
                frame = (i * 5 + z * 3) % 24
                yb = self.surface_y(x, z) + self.dz * 0.5
                spr = tile(plants, row, frame, size)
                self.glow(spr, x * s - size // 2, yb - size, z)
                self.paste(spr, x * s - size // 2, yb - size, z, lift=1.45)

        if z == CREATURE_NEAR_Z:
            x = int(CREATURE_NEAR_U * w)
            yb = self.surface_y(x, z) + self.dz * 0.5
            self.paste(
                tile(creatures, 10, 4, size),
                x * s - size // 2,
                yb - size,
                z,
                warm=True,
                lift=1.3,
            )

        if z == int(sc.depth * 0.72):
            x = int(CREATURE_FAR_U * w)
            yb = self.surface_y(x, z) + self.dz * 0.5
            self.paste(
                tile(creatures, 0, 0, size), x * s - size // 2, yb - size, z, lift=1.3
            )

    # -- whole frame --

    def render(self, atlases) -> Image.Image:
        self.sky()
        for z in range(self.sc.depth - 1, -1, -1):
            self.draw_slab(z)
            self.draw_sprites(z, atlases)
        return Image.fromarray(np.clip(self.fb, 0, 255).astype(np.uint8), "RGB")


# ---- driver ---------------------------------------------------------------


def save_small(img: Image.Image, path: str, colors: int = 96) -> int:
    q = img.convert("P", palette=Image.ADAPTIVE, colors=colors, dither=Image.NONE)
    q.save(path, optimize=True)
    return os.path.getsize(path)


def render_cell(tilt: float, depth: int, px: int, atlases) -> Image.Image:
    return Frame(Scene(px, depth), tilt).render(atlases)


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    outdir = sys.argv[1] if len(sys.argv) > 1 else here
    os.makedirs(outdir, exist_ok=True)
    atlases = load_atlases()

    for tilt in (25, 35):
        for depth in (16, 32):
            for px in (3, 4):
                name = f"tilt{tilt}_d{depth}_px{px}.png"
                n = save_small(render_cell(tilt, depth, px, atlases), os.path.join(outdir, name))
                print(f"{name}  {n // 1024} KiB")

    # the middle of the matrix, 2x on the pool, to judge the water surface, the
    # falling stream and the near creature at pixel level
    img = render_cell(30, 24, 4, atlases)
    crop = img.crop((800, 690, 1280, 960)).resize((960, 540), Image.NEAREST)
    n = save_small(crop, os.path.join(outdir, "zoom_pool_tilt30_d24_px4.png"))
    print(f"zoom_pool_tilt30_d24_px4.png  {n // 1024} KiB")


if __name__ == "__main__":
    main()
