"""Latticevine (D15) look study: the O4 braided fan with rosette tips, modelled on a
rock face and voxelized by the bake's own rule.

    blender -b -P scripts/blender/latticevine.py -- [--out DIR] [--seed N]

Spec: design/art-direction/species-dossier-D15-latticevine-2026-09-23.md (O4, Revision 1).
Brief: design/handoffs/latticevine-model-2026-09-23.md. Nothing here is wired into the
simulation.

Axes: Blender x across the face, z up, y into the rock. The rock face is the plane
y = 0 (a voxel boundary); the vine lives at y < 0, pressed flat, except the bowed low
runners and root arches, which stand out about one voxel.

Growth habit (procedural, seeded):
  * 1-4 runners start from separate root arches along the foot (the dens).
  * Each climbs a fixed chain through the bower band (grown/full: bowed ~1 voxel off
    the rock between two holdfasts); two of them converge and braid (a two-strand
    twist, crossing in depth) and split again at the braid's top.
  * Above that, space colonization grows Y-forks (at most two children per node) into
    a lopsided envelope that climbs up-right along a diagonal lower edge; runner radii
    follow a pipe model, so every fork is thinner than its parent.
  * Rosette clumps (1-4 rosettes, 1-2 voxels across, shingle ramp outer -> centre) are
    added at tips (weighted) and along the upper runners until the leafed fraction of the
    envelope reaches the variant's target (50 % / 80 %). The dense variant is the half
    variant plus more rosettes (same stream).
  * Spurs at irregular sites on any runner, interior included, with scattered phases.

Voxelization: `voxelize()` from bake_voxel_models.py, loaded without running that
module's main() (its source is parsed and the bare `main()` call dropped; nothing is
forked or edited).
"""

import ast
import math
import os
import random
import sys

import bmesh
import bpy
import numpy as np
from mathutils import Matrix, Vector
from mathutils.kdtree import KDTree

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import organism_lineup as L  # noqa: E402  (import-safe: main() is guarded)

REPO = os.path.normpath(os.path.join(HERE, "..", ".."))
ARGS = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []


def arg(name, default=None):
    return ARGS[ARGS.index(name) + 1] if name in ARGS else default


OUT = arg("--out", os.path.join(REPO, "runs", "latticevine-model-2026-09-23"))
SEED = int(arg("--seed", "2309"))
THIN_ONLY = "--thin" in ARGS  # fix 1: only the dense thin-rule shots (*-thin.png)
VOX = 0.125


def load_voxelize():
    """bake_voxel_models.voxelize, without executing the module's trailing main()."""
    path = os.path.join(HERE, "bake_voxel_models.py")
    with open(path) as fh:
        tree = ast.parse(fh.read(), path)
    tree.body = [n for n in tree.body
                 if not (isinstance(n, ast.Expr) and isinstance(n.value, ast.Call)
                         and getattr(n.value.func, "id", None) == "main")]
    ns = {"__name__": "bake_voxel_models", "__file__": path}
    exec(compile(tree, path, "exec"), ns)
    return ns["voxelize"]


voxelize = load_voxelize()

# --- palette and roles (dossier hexes; one material per role) ------------------------

LV = {
    "lv_rock": "#12093A", "lv_joint": "#2A0E4A", "lv_hollow": "#07031A", "lv_soil": "#2A2148",
    "lv_runner_old": "#2A0E4A", "lv_runner_young": "#3A1A7A", "lv_root": "#510B6D",
    "lv_pad": "#510B6D", "lv_pad_rim": "#B99BE6",
    "lv_shingle0": "#1E2798", "lv_shingle1": "#2B6AD0", "lv_shingle2": "#42C5F8",
    "lv_spur": "#3A1A7A", "lv_bud_tip": "#B99BE6", "lv_spent": "#510B6D",
    "lv_bell": "#B99BE6", "lv_mouth": "#42C5F8", "lv_bead": "#FF2AFC",
}
L.PAL.update(LV)
ROLE = {
    "lv_runner_old": "trunk", "lv_runner_young": "trunk", "lv_root": "trunk", "lv_pad": "trunk",
    "lv_spur": "trunk", "lv_spent": "trunk",
    "lv_shingle0": "foliage", "lv_shingle1": "foliage", "lv_shingle2": "foliage",
    "lv_bell": "accent", "lv_mouth": "accent", "lv_bead": "accent",
    # scenery and sub-voxel trims: drawn, never voxelized
    "lv_rock": None, "lv_joint": None, "lv_hollow": None, "lv_soil": None,
    "lv_pad_rim": None, "lv_bud_tip": None,
}


def role_of(mat):
    return ROLE.get(mat)


# Thin rule (fix 1): the woody parts -- runners, braid, root arches, holdfast pads, and the
# spur stalks/buds/stubs/spent stalks -- claim only the cells whose centres lie inside them,
# or, when a part holds no centre, the cell its centre line passes through at each step
# along it (steps of vox/8, so a runner is a continuous 1-voxel line). Rosettes keep the
# half-voxel rule, bells and beads the accent rule.
THIN_MATS = {"lv_runner_old", "lv_runner_young", "lv_root", "lv_pad", "lv_spur", "lv_spent"}


def thin_cells(segs, vox):
    cells = {}
    for p0, p1, r0, r1, mat in segs:
        d = p1 - p0
        ln2 = d.length_squared
        rr = max(r0, r1)
        lo = [min(p0[a], p1[a]) - rr for a in range(3)]
        hi = [max(p0[a], p1[a]) + rr for a in range(3)]
        hit = []
        for i in range(math.floor(lo[0] / vox), math.floor(hi[0] / vox) + 1):
            for j in range(math.floor(lo[1] / vox), math.floor(hi[1] / vox) + 1):
                for k in range(math.floor(lo[2] / vox), math.floor(hi[2] / vox) + 1):
                    c = Vector(((i + 0.5) * vox, (j + 0.5) * vox, (k + 0.5) * vox))
                    t = 0.0 if ln2 < 1e-12 else min(1.0, max(0.0, (c - p0).dot(d) / ln2))
                    if (c - (p0 + d * t)).length <= r0 + (r1 - r0) * t:
                        hit.append((i, j, k))
        if not hit:
            n = max(1, math.ceil(math.sqrt(ln2) / (vox / 8)))
            hit = {tuple(math.floor(v / vox) for v in p0 + d * (m / n)) for m in range(n + 1)}
        for key in hit:
            if key[2] >= 0 and key[1] < 0:
                cells[key] = mat
    return cells


# --- sizes and variants -------------------------------------------------------------

# name, cover W x H voxels, runners, braid turns (0 none), bowers, arch (den w, h, others
# w range, h range) in voxels, root radius m, spur phases
SIZES = [
    ("newborn", 1, 2, 1, 0.0, False, (0.7, 0.4, (0.7, 0.7), (0.4, 0.4)), 0.012, {"bud": 1}),
    ("young", 3, 5, 2, 0.0, False, (1.0, 0.6, (0.8, 1.0), (0.5, 0.6)), 0.015,
     {"flower": 1, "fruit": 1, "bud": 1, "stub": 1}),
    ("grown", 6, 10, 3, 1.5, True, (2.5, 1.5, (1.2, 1.8), (0.8, 1.1)), 0.022,
     {"flower": 4, "fruit": 4, "bud": 3, "stub": 3, "spent": 2}),
    ("full", 10, 16, 4, 2.0, True, (3.0, 2.0, (1.2, 2.0), (0.8, 1.3)), 0.026,
     {"flower": 8, "fruit": 8, "bud": 5, "stub": 5, "spent": 3}),
]
SIZE = {s[0]: s for s in SIZES}
VARIANTS = [("half", 0.50), ("dense", 0.80)]

R_TIP, R_MAX, PIPE = 0.009, 0.032, 2.2   # runner radii (1 px ~ 0.021 m) and pipe exponent
OLD_R = 0.019                            # thicker than this: old runner colour
BACK = 0.004                             # every part stays this far off the rock (y < 0)
BOW = 0.10                               # bower stand-off, m (about one voxel)
BRAID_AMP_X, BRAID_AMP_Y = 0.028, 0.035
RASTER = 0.02                            # coverage raster, m


# --- geometry accumulator -----------------------------------------------------------

class Geo:
    """Parts go into one mesh per material; each accent is its own closed object (the
    accent rule tests insideness per accent)."""

    def __init__(self):
        self.parts = {}
        self.accents = []
        self.thin = []   # centre lines of the thin-rule parts: (p0, p1, r0, r1, mat)

    def _bm(self, mat, accent):
        if accent:
            bm = bmesh.new()
            self.accents.append((mat, bm))
            return bm
        if mat not in self.parts:
            self.parts[mat] = bmesh.new()
        return self.parts[mat]

    def cyl(self, p0, p1, r0, r1, mat, seg=8, accent=False):
        p0, p1 = Vector(p0), Vector(p1)
        d = p1 - p0
        if d.length < 1e-6:
            return
        if mat in THIN_MATS and not accent:
            self.thin.append((p0, p1, r0, r1, mat))
        q = Vector((0, 0, 1)).rotation_difference(d.normalized())
        m = Matrix.Translation((p0 + p1) / 2) @ q.to_matrix().to_4x4()
        bmesh.ops.create_cone(self._bm(mat, accent), cap_ends=True, cap_tris=False, segments=seg,
                              radius1=r0, radius2=r1, depth=d.length, matrix=m)

    def ell(self, c, r, mat, rot=None, accent=False, u=12, v=8):
        if mat in THIN_MATS and not accent:
            self.thin.append((Vector(c), Vector(c), max(r), max(r), mat))
        m = Matrix.Translation(Vector(c)) @ (rot or Matrix.Identity(4)) @ Matrix.Diagonal((*r, 1))
        bmesh.ops.create_uvsphere(self._bm(mat, accent), u_segments=u, v_segments=v, radius=1.0, matrix=m)

    def box(self, c, s, mat, rot=None):
        m = Matrix.Translation(Vector(c)) @ (rot or Matrix.Identity(4)) @ Matrix.Diagonal((*s, 1))
        bmesh.ops.create_cube(self._bm(mat, False), size=1.0, matrix=m)

    def objects(self, col, tag):
        out = []
        items = [(m, b) for m, b in self.parts.items()] + self.accents
        for n, (mat, bm) in enumerate(items):
            me = bpy.data.meshes.new(f"{tag}:{mat}:{n}")
            bm.to_mesh(me)
            bm.free()
            ob = bpy.data.objects.new(me.name, me)
            ob.data.materials.append(L.M(mat))
            col.objects.link(ob)
            out.append(ob)
        return out


def flat_rot(theta):
    """In the face plane: local x along (cos, 0, sin), local y into the rock, det +1."""
    c, s = math.cos(theta), math.sin(theta)
    return Matrix(((c, 0, -s), (0, 1, 0), (s, 0, c))).to_4x4()


YROT = Matrix.Rotation(math.radians(90), 4, "X")  # cylinder axis z -> along -y


# --- the tree -----------------------------------------------------------------------

class Node:
    __slots__ = ("x", "z", "parent", "children", "fixed", "grow", "yoff", "r", "leaves", "since_pad",
                 "bowed", "braid")

    def __init__(self, x, z, parent=None, fixed=False, grow=True, yoff=0.0, bowed=False, braid=False):
        self.x, self.z, self.parent = x, z, parent
        self.children = []
        self.fixed, self.grow, self.yoff, self.bowed, self.braid = fixed, grow, yoff, bowed, braid
        self.r, self.leaves, self.since_pad = R_TIP, 0, 0.0


class Stand:
    """One latticevine stand, skeleton seeded by size, leaves/spurs by variant."""

    def __init__(self, size_name, variant, seed=SEED):
        (self.name, wv, hv, self.nrun, self.turns, self.bowers, arch, self.root_r,
         self.spur_plan) = SIZE[size_name]
        self.variant, self.target = variant
        self.W, self.H = wv * VOX, hv * VOX
        self.rng = random.Random(f"latticevine:{seed}:{size_name}")
        self.leaf_rng = random.Random(f"latticevine:{seed}:{size_name}:leaves")
        self.geo = Geo()
        self.nodes = []
        self.arches = []
        self.spurs = []      # (phase, {centre points by accent})
        self.rosettes = []
        self._feet(arch)
        self._layout_bands()
        self._chains()
        self._colonize()
        self._smooth()
        self._radii()
        self._raster_init()
        self._draw_arches()
        self._draw_runners()
        self._rosettes()
        self._spurs()

    # -- layout
    def _feet(self, arch):
        rng, W = self.rng, self.W
        if self.nrun == 1:
            xs = [0.5 * W]
        else:
            lo = 0.1 * W
            hi = lo + max(0.55 * W, (self.nrun - 1) * VOX * 1.15)
            for _ in range(500):
                xs = sorted(rng.uniform(lo, hi) for _ in range(self.nrun))
                if all(b - a >= VOX for a, b in zip(xs, xs[1:])):
                    break
        den_w, den_h, (w0, w1), (h0, h1) = arch
        den = min(1, self.nrun - 1)
        self.feet = []
        for k, x in enumerate(xs):
            if k == den:
                w, h = den_w * VOX, den_h * VOX
            else:
                w, h = rng.uniform(w0, w1) * VOX, rng.uniform(h0, h1) * VOX
            self.feet.append((x, w, h))
        self.feet_hi = xs[-1]

    def _layout_bands(self):
        H = self.H
        self.arch_top = max(h for _, _, h in self.feet)
        self.zbow = (0.25 * H + 0.1) if self.bowers else self.arch_top
        self.zlo = self.zbow if self.bowers else self.arch_top + 0.02
        self.braid_z = (0.36 * H + 0.03, 0.62 * H) if self.turns else None
        if self.braid_z:
            self.braid_z = (max(self.braid_z[0], self.zbow + 0.1), self.braid_z[1])
        self.top_phase = self.rng.uniform(0, 6.28)
        self.step = max(0.02, min(0.04, H / 14))

    def top(self, x):
        u = min(max(x / self.W, 0.0), 1.0)
        t = self.H * (0.58 + 0.42 * u ** 0.9) * (1 + 0.05 * math.sin(9 * u + self.top_phase))
        return min(t, self.H)

    def bottom(self, x):
        return self.zlo + max(0.0, x - self.feet_hi) * 0.7

    def inside(self, x, z):
        return 0 <= x <= self.W and self.bottom(x) <= z <= self.top(x)

    # -- fixed chains: foot -> bower band -> (braid) -> growth seeds
    def _add(self, *a, **k):
        n = Node(*a, **k)
        self.nodes.append(n)
        if n.parent is not None:
            self.nodes[n.parent].children.append(len(self.nodes) - 1)
        return len(self.nodes) - 1

    def _bowed(self, z, h):
        if not self.bowers or z <= h or z >= self.zbow:
            return 0.0
        return -BOW * math.sin(math.pi * (z - h) / (self.zbow - h))

    def _path(self, parent, x0, z0, x1, z1, h, grow_end=True):
        rng = self.rng
        n = max(2, math.ceil(math.hypot(x1 - x0, z1 - z0) / self.step))
        wob, ph = rng.uniform(0.008, 0.018), rng.uniform(0, 6.28)
        idx = parent
        for k in range(1, n + 1):
            t = k / n
            x = x0 + (x1 - x0) * t + wob * math.sin(ph + 7 * t) * math.sin(math.pi * t)
            z = z0 + (z1 - z0) * t
            yb = self._bowed(z, h)
            idx = self._add(x, z, idx, fixed=True, grow=(k == n and grow_end), yoff=yb, bowed=yb < -0.01)
        return idx

    def _chains(self):
        rng, H = self.rng, self.H
        braided = set()
        if self.turns and self.nrun >= 2:
            mids = [abs((self.feet[k][0] + self.feet[k + 1][0]) / 2 - 0.4 * self.W) for k in range(self.nrun - 1)]
            b = mids.index(min(mids))
            braided = {b, b + 1}
        self.roots = []
        for k, (x0, w, h) in enumerate(self.feet):
            root = self._add(x0, h, None, fixed=True, grow=False)
            self.roots.append(root)
            self.arches.append((x0, w, h))
            if k in braided:
                continue
            zt = (self.zbow + rng.uniform(0.0, 0.08) * H) if self.bowers else h + rng.uniform(0.2, 0.3) * H
            xt = x0 + rng.uniform(0.15, 0.4) * (zt - h)
            self._path(root, x0, h, xt, zt, h)
        if braided:
            ka, kb = sorted(braided)
            zs, ze = self.braid_z
            xa, xb = self.feet[ka][0], self.feet[kb][0]
            sx = (xa + xb) / 2 + 0.2 * (zs - self.arch_top)
            ex = sx + 0.3 * (ze - zs)
            nb = max(8, int(12 * self.turns))
            for s_i, k in enumerate((ka, kb)):
                x0, _, h = self.feet[k]
                sign = 1 if s_i == 0 else -1
                idx = self._path(self.roots[k], x0, h, sx - sign * BRAID_AMP_X, zs, h, grow_end=False)
                for m in range(1, nb + 1):
                    s = m / nb
                    ph = 2 * math.pi * self.turns * s + (0 if s_i == 0 else math.pi)
                    ax, az = sx + (ex - sx) * s, zs + (ze - zs) * s
                    # s_i 0 starts at -amp so the pair begins apart; y crosses in depth
                    x = ax - BRAID_AMP_X * math.cos(ph)
                    yo = -BRAID_AMP_Y * (0.5 + 0.5 * math.sin(ph))
                    idx = self._add(x, az, idx, fixed=True, grow=(m == nb), yoff=yo, braid=True)
            self.braid_span = (zs, ze)

    # -- space colonization
    def _colonize(self):
        rng = self.leaf_rng.__class__(f"{SEED}:{self.name}:sc")
        area = self.W * self.H
        sp = min(0.09, max(0.035, math.sqrt(area) / 5))
        pts = []
        nx, nz = int(self.W / sp) + 1, int(self.H / sp) + 1
        for i in range(nx):
            for j in range(nz):
                x, z = (i + rng.random()) * sp, (j + rng.random()) * sp
                if self.inside(x, z):
                    pts.append(Vector((x, 0, z)))
        infl, kill = 3.2 * sp, max(1.6 * self.step, 0.7 * sp)
        bias = Vector((0.18, 0, 0.45))
        for _ in range(260):
            if not pts:
                break
            grow = [i for i, n in enumerate(self.nodes)
                    if n.grow and len(n.children) < (2 if n.z >= self.zlo else 1)]
            if not grow:
                break
            kd = KDTree(len(grow))
            for gi, i in enumerate(grow):
                kd.insert((self.nodes[i].x, 0, self.nodes[i].z), gi)
            kd.balance()
            pull = {}
            for p in pts:
                co, gi, d = kd.find(p)
                if gi is not None and d < infl:
                    pull.setdefault(grow[gi], []).append(p)
            if not pull:
                break
            added = 0
            for i, lst in pull.items():
                n = self.nodes[i]
                here = Vector((n.x, 0, n.z))
                d = Vector((0, 0, 0))
                for p in lst:
                    d += (p - here).normalized()
                d = d.normalized() + bias
                if n.parent is not None:
                    par = self.nodes[n.parent]
                    d += 0.5 * (here - Vector((par.x, 0, par.z))).normalized()
                if d.length < 1e-6:
                    n.grow = False
                    continue
                q = here + d.normalized() * self.step
                if any((Vector((self.nodes[c].x, 0, self.nodes[c].z)) - q).length < 0.35 * self.step
                       for c in n.children):
                    n.grow = False
                    continue
                self._add(q.x, q.z, i)
                added += 1
            if not added:
                break
            kd2 = KDTree(len(self.nodes))
            for i, n in enumerate(self.nodes):
                kd2.insert((n.x, 0, n.z), i)
            kd2.balance()
            pts = [p for p in pts if kd2.find(p)[2] > kill]

    def _smooth(self):
        for _ in range(3):
            for n in self.nodes:
                if n.fixed or n.parent is None or len(n.children) != 1:
                    continue
                p, c = self.nodes[n.parent], self.nodes[n.children[0]]
                n.x = 0.5 * n.x + 0.25 * (p.x + c.x)
                n.z = 0.5 * n.z + 0.25 * (p.z + c.z)

    def _radii(self):
        for n in reversed(self.nodes):
            n.leaves = 1 if not n.children else sum(self.nodes[c].leaves for c in n.children)
            n.r = min(R_MAX, R_TIP * n.leaves ** (1 / PIPE))

    def p3(self, n):
        return Vector((n.x, -(n.r + BACK) + n.yoff, n.z))

    # -- coverage raster (the face, envelope above the bower band)
    def _raster_init(self):
        self.gx = int(math.ceil((self.W + 0.4) / RASTER))
        self.gz = int(math.ceil((self.H + 0.2) / RASTER))
        self.ox = -0.2
        xs = self.ox + (np.arange(self.gx) + 0.5) * RASTER
        zs = (np.arange(self.gz) + 0.5) * RASTER
        self.region = np.zeros((self.gx, self.gz), bool)
        for i, x in enumerate(xs):
            for j, z in enumerate(zs):
                self.region[i, j] = self.inside(x, z)
        self.cover = np.zeros_like(self.region)
        self.gxs, self.gzs = xs, zs

    def _stamp(self, x, z, r, apply=True):
        i0 = max(0, int((x - r - self.ox) / RASTER))
        i1 = min(self.gx, int((x + r - self.ox) / RASTER) + 1)
        j0, j1 = max(0, int((z - r) / RASTER)), min(self.gz, int((z + r) / RASTER) + 1)
        if i0 >= i1 or j0 >= j1:
            return 1.0
        X, Z = np.meshgrid(self.gxs[i0:i1], self.gzs[j0:j1], indexing="ij")
        m = (X - x) ** 2 + (Z - z) ** 2 <= r * r
        if not m.any():
            return 1.0
        frac = self.cover[i0:i1, j0:j1][m].mean()
        if apply:
            self.cover[i0:i1, j0:j1][m] = True
        return frac

    def coverage(self):
        return float(self.cover[self.region].mean())

    # -- drawing
    def _draw_arches(self):
        g, rng = self.geo, self.rng
        for x0, w, h in self.arches:
            ra = self.root_r
            depth = BOW * 0.8 if self.bowers else 0.25 * h
            prev = None
            for k in range(13):
                t = k / 12
                p = Vector((x0 + (t - 0.5) * w, -(ra + BACK) - depth * math.sin(math.pi * t),
                            h * math.sin(math.pi * t)))
                if prev is not None:
                    g.cyl(prev, p, ra, ra, "lv_root", seg=8)
                    g.ell(p, (ra, ra, ra), "lv_root", u=8, v=6)
                prev = p
            for side in (-1, 1):
                lx = x0 + side * w / 2
                for sp in (-1, 1):
                    g.cyl((lx, -(ra + BACK), 0.02), (lx + sp * 0.035 + side * 0.01, -0.05, -0.03),
                          ra * 0.5, ra * 0.3, "lv_root", seg=6)
            if h >= 0.09:  # the den: a dark hollow under the arch
                g.ell((x0, 0.0, 0.0), (0.42 * w, 0.003, 0.8 * h), "lv_hollow", u=16, v=8)
            # the runner leaves the arch apex: a pad pins it there
            self._pad(x0, h + 0.01, ra)

    def _pad(self, x, z, r_run):
        r = max(0.031, r_run + 0.01)
        self.geo.cyl((x, -0.004, z), (x, -0.006, z), r + 0.006, r + 0.006, "lv_pad_rim", seg=12)
        self.geo.cyl((x, -0.0045, z), (x, -0.009, z), r, r, "lv_pad", seg=12)

    def _draw_runners(self):
        g = self.geo
        bow_mid = {}
        for idx, n in enumerate(self.nodes):
            if n.parent is None:
                continue
            p = self.nodes[n.parent]
            a, b = self.p3(p), self.p3(n)
            mat = "lv_runner_old" if n.r >= OLD_R or n.bowed else "lv_runner_young"
            g.cyl(a, b, p.r, n.r, mat, seg=8)
            if n.r >= 0.012:
                g.ell(b, (n.r, n.r, n.r), mat, u=8, v=6)
            self._stamp_segment(p, n)
            # holdfasts: at forks and every 9-15 cm of pressed runner, never mid-bower
            n.since_pad = p.since_pad + (b - a).length
            if not n.bowed and not n.braid and (len(n.children) >= 2 or n.since_pad > self.rng.uniform(0.09, 0.15)):
                self._pad(n.x, n.z, n.r)
                n.since_pad = 0.0
            if n.bowed:
                bow_mid.setdefault(self._root_of(idx), []).append(n)
        for nodes in bow_mid.values():  # the bower hollow behind each bowed runner
            zs = [n.z for n in nodes]
            xs = [n.x for n in nodes]
            g.ell((sum(xs) / len(xs), 0.0, (min(zs) + max(zs)) / 2),
                  (0.055, 0.003, 0.42 * (max(zs) - min(zs))), "lv_hollow", u=12, v=8)

    def _root_of(self, idx):
        while self.nodes[idx].parent is not None:
            idx = self.nodes[idx].parent
        return idx

    def _stamp_segment(self, p, n):
        steps = max(1, int(math.hypot(n.x - p.x, n.z - p.z) / (RASTER / 2)))
        for k in range(steps + 1):
            t = k / steps
            self._stamp(p.x + (n.x - p.x) * t, p.z + (n.z - p.z) * t, max(n.r, RASTER * 0.6))

    def _rosette(self, x, z, R, yb):
        g, rng = self.geo, self.leaf_rng
        th = 0.006
        rings = [(7 if R >= 0.09 else 6, 0.55, 0.45, 0.30, "lv_shingle0", 0.0),
                 (5, 0.32, 0.30, 0.22, "lv_shingle1", -0.007),
                 (rng.choice((2, 3)), 0.10, 0.17, 0.13, "lv_shingle2", -0.013)]
        for n, rc, hl, hw, mat, dy in rings:
            ph = rng.uniform(0, 6.28)
            for k in range(n):
                a = ph + 2 * math.pi * k / n + rng.uniform(-0.25, 0.25)
                c = (x + math.cos(a) * rc * R, yb + dy - rng.uniform(0, 0.002), z + math.sin(a) * rc * R)
                g.ell(c, (hl * R, th, hw * R), mat, rot=flat_rot(a), u=10, v=6)
        self.rosettes.append((x, z, R))

    def _rosettes(self):
        rng = self.leaf_rng
        cands, weights = [], []
        for n in self.nodes:
            if n.fixed or n.z < self.zlo:
                continue
            cands.append(n)
            weights.append(6.0 if not n.children else (2.0 if n.z > 0.55 * self.H else 1.0))
        if not cands:
            cands = [self.nodes[-1]]
            weights = [1.0]
        rmin, rmax = (0.045, 0.07) if self.H < 0.3 else (0.5 * VOX, 1.0 * VOX)
        limit = 0.55
        tries = 0
        while self.coverage() < self.target and tries < 6000:
            tries += 1
            if tries in (1500, 3000, 4500):
                limit += 0.12
            n = rng.choices(cands, weights)[0]
            size = rng.choices((1, 2, 3, 4), (35, 30, 20, 15))[0]
            x, z = n.x + rng.uniform(-0.01, 0.01), n.z + rng.uniform(-0.01, 0.01)
            yb = -(2 * n.r + BACK + 0.008)
            placed = []
            for k in range(size):
                R = rng.uniform(rmin, rmax)
                if k:
                    a = rng.uniform(-0.3, math.pi + 0.3)
                    px, pz = placed[-1]
                    x, z = px + math.cos(a) * R * rng.uniform(0.7, 1.1), pz + math.sin(a) * R * rng.uniform(0.7, 1.1)
                if self._stamp(x, z, 0.9 * R, apply=False) > limit:
                    break
                self._stamp(x, z, 0.9 * R)
                self._rosette(x, z, R, yb - rng.uniform(0, 0.008))
                placed.append((x, z))
                if self.coverage() >= self.target:
                    break

    def _covered(self, x, z):
        i, j = int((x - self.ox) / RASTER), int(z / RASTER)
        return 0 <= i < self.gx and 0 <= j < self.gz and self.cover[i, j] and any(
            (x - rx) ** 2 + (z - rz) ** 2 < (0.9 * R) ** 2 for rx, rz, R in self.rosettes)

    def _spurs(self):
        rng = random.Random(f"latticevine:{SEED}:{self.name}:spurs")
        g = self.geo
        phases = [p for p, k in self.spur_plan.items() for _ in range(k)]
        rng.shuffle(phases)
        cands = [n for n in self.nodes if n.parent is not None and not n.bowed and not n.braid
                 and n.z > self.arch_top + 0.04]
        gap = 0.09 if self.H > 0.3 else 0.04
        sites = []
        for ph in phases:
            for _ in range(400):
                n = rng.choice(cands)
                if all((n.x - s.x) ** 2 + (n.z - s.z) ** 2 >= gap * gap for s in sites):
                    break
            sites.append(n)
            front = -(2 * n.r + BACK + (0.03 if self._covered(n.x, n.z) else 0.006))
            base = Vector((n.x, front + 0.01, n.z))
            end = base + Vector((rng.uniform(-0.012, 0.012), -0.028, -0.03))
            sr = 0.008
            info = {"x": n.x, "z": n.z, "interior": 0.15 * self.W < n.x < 0.85 * self.W
                    and self.bottom(n.x) + 0.1 * self.H < n.z < self.top(n.x) - 0.1 * self.H}
            if ph == "stub":
                g.cyl(base, base + Vector((0, -0.03, -0.035)), sr, sr * 0.8, "lv_spur", seg=6)
            elif ph == "bud":
                g.cyl(base, end, sr, sr, "lv_spur", seg=6)
                g.ell(end + Vector((0, 0, -0.028)), (0.021, 0.021, 0.031), "lv_spur", u=10, v=8)
                g.ell(end + Vector((0, 0, -0.056)), (0.009, 0.009, 0.009), "lv_bud_tip", u=8, v=6)
            elif ph == "spent":
                mid = base + Vector((0, -0.03, -0.018))
                g.cyl(base, mid, sr, sr, "lv_spent", seg=6)
                g.cyl(mid, mid + Vector((rng.uniform(-0.01, 0.01), -0.006, -0.045)), sr, sr * 0.6, "lv_spent", seg=6)
            elif ph == "flower":
                g.cyl(base, end, sr, sr, "lv_spur", seg=6)
                # the bell hangs tilted ~35 deg toward the viewer so its mouth shows
                axis = Vector((0, -0.57, -0.82))
                top = end + Vector((0, 0, -0.004))
                bot = top + axis * 0.07
                mouth = bot + axis * 0.004
                tilt = Vector((0, 0, 1)).rotation_difference(axis).to_matrix().to_4x4()
                # mouth first: where bell and mouth share a cell, the later accent (bell) wins
                g.ell(mouth, (0.034, 0.034, 0.009), "lv_mouth", rot=tilt, accent=True, u=14, v=6)
                g.cyl(top, bot, 0.012, 0.03, "lv_bell", seg=14, accent=True)
                info["bell"], info["mouth"], info["top"] = (top + bot) / 2, mouth, top
            elif ph == "fruit":
                g.cyl(base, end, sr, sr, "lv_spur", seg=6)
                c = end + Vector((0, -0.004, -0.05))
                k = rng.randint(4, 6)
                beads = []
                for _ in range(300):
                    if len(beads) == k:
                        break
                    v = Vector((rng.uniform(-1, 1), rng.uniform(-1, 1), rng.uniform(-1, 1)))
                    if v.length > 1:
                        continue
                    p = c + v * 0.03
                    if all((p - q).length >= 0.034 for q in beads):
                        beads.append(p)
                for p in beads:
                    g.ell(p, (0.021, 0.021, 0.021), "lv_bead", accent=True, u=12, v=8)
                info["beads"] = beads
            self.spurs.append((ph, info))

    def build(self, col, tag):
        return self.geo.objects(col, tag)


# --- voxel stats ---------------------------------------------------------------------

def vox_stats(st, cells, vox):
    inside_rock = [k for k in cells if k[1] >= 0]
    for k in inside_rock:
        del cells[k]
    by = {}
    for m in cells.values():
        by[m] = by.get(m, 0) + 1
    cols = {(i, k) for i, _, k in cells}
    reg = 0
    hit = 0
    for i in range(-2, int(st.W / vox) + 3):
        for k in range(0, int(st.H / vox) + 2):
            if st.inside((i + 0.5) * vox, (k + 0.5) * vox):
                reg += 1
                hit += (i, k) in cols
    def key(p):
        return (math.floor(p.x / vox), math.floor(p.y / vox), math.floor(p.z / vox))

    # the lower band: columns across the feet span, below the bower band's top
    x0 = min(x - w / 2 for x, w, _ in st.feet)
    x1 = max(x + w / 2 for x, w, _ in st.feet)
    bi = range(math.floor(x0 / vox), math.floor(x1 / vox) + 1)
    bk = range(0, max(1, math.floor(st.zlo / vox)))
    band = (sum((i, k) in cols for i in bi for k in bk), len(bi) * len(bk))
    fl = [s for p, s in st.spurs if p == "flower"]
    fr = [s for p, s in st.spurs if p == "fruit"]
    return {
        "cells": len(cells), "by": by, "dropped_in_rock": len(inside_rock),
        "cover": hit / reg if reg else 0.0,
        "bells": sum(cells.get(key(s["bell"])) == "lv_bell" or cells.get(key(s["top"])) == "lv_bell" for s in fl),
        "band": band,
        "mouths": sum(cells.get(key(s["mouth"])) == "lv_mouth" for s in fl),
        "flowers": len(fl),
        "bunches": sum(any(cells.get(key(p)) == "lv_bead" for p in s["beads"]) for s in fr),
        "fruit": len(fr),
    }


# --- scene layout ----------------------------------------------------------------------

SRC = None


def new_col(name, link=True):
    col = bpy.data.collections.new(name)
    if link:
        bpy.context.scene.collection.children.link(col)
    return col


_cache = {}


def stand(size, variant):
    key = (size, variant[0])
    if key not in _cache:
        st = Stand(size, variant)
        objs = st.build(SRC, f"{size}-{variant[0]}")
        _cache[key] = (st, objs, {})
    return _cache[key]


def voxels(size, variant, vox):
    st, objs, vc = stand(size, variant)
    if vox not in vc:
        cells = voxelize(objs, vox, role_of)
        vc[vox] = (cells, vox_stats(st, cells, vox))
    return vc[vox]


def voxels_thin(size, variant, vox):
    """Thin rule for the woody parts, the bake's voxelize() for rosettes, bells and beads,
    and the mouth split: when a bell's top and its mouth fall in different cells, the
    bell takes the upper cell and the mouth the lower."""
    st, objs, vc = stand(size, variant)
    if ("thin", vox) not in vc:
        def fol_role(m):
            return None if m in THIN_MATS or m == "lv_mouth" else role_of(m)
        cells = thin_cells(st.geo.thin, vox)
        cells.update(voxelize(objs, vox, fol_role))

        def key(p):
            return (math.floor(p.x / vox), math.floor(p.y / vox), math.floor(p.z / vox))
        split = 0
        for ph, info in st.spurs:
            if ph != "flower":
                continue
            kt, km = key(info["top"]), key(info["mouth"])
            if kt != km and km[2] >= 0:
                cells[kt], cells[km] = "lv_bell", "lv_mouth"
                split += 1
        s = vox_stats(st, cells, vox)
        s["split"] = split
        vc[("thin", vox)] = (cells, s)
    return vc[("thin", vox)]


def place_smooth(col, size, variant, x, z=0.0):
    _, objs, _ = stand(size, variant)
    for ob in objs:
        o = ob.copy()
        o.location = (x, 0, z)
        col.objects.link(o)


def place_vox(col, size, variant, vox, x, z=0.0, thin=False):
    cells, _ = (voxels_thin if thin else voxels)(size, variant, vox)
    if thin:  # the drawn den/bower hollows on the rock (scenery, not voxels)
        for ob in stand(size, variant)[1]:
            if ob.active_material.name == "lv_hollow":
                o = ob.copy()
                o.location = (x, 0, z)
                col.objects.link(o)
    by = {}
    for key, mat in cells.items():
        by.setdefault(mat, []).append(key)
    for mat, keys in by.items():
        bm = bmesh.new()
        for i, j, k in keys:
            m = Matrix.Translation(((i + 0.5) * vox + x, (j + 0.5) * vox, (k + 0.5) * vox + z)) \
                @ Matrix.Diagonal((vox, vox, vox, 1))
            bmesh.ops.create_cube(bm, size=1.0, matrix=m)
        me = bpy.data.meshes.new(f"vox{int(vox * 1000)}:{size}:{variant[0]}:{mat}")
        bm.to_mesh(me)
        bm.free()
        ob = bpy.data.objects.new(me.name, me)
        ob.data.materials.append(L.M(mat))
        col.objects.link(ob)


def wall(col, x0, x1, z0, ztop, depth=0.55, seed=0):
    """Rock face (y >= 0) from x0 to x1, a soil ledge in front at the foot, diagonal joints."""
    g = Geo()
    g.box(((x0 + x1) / 2, 0.15, (z0 - 0.3 + ztop) / 2), (x1 - x0, 0.3, ztop - z0 + 0.3), "lv_rock")
    g.box(((x0 + x1) / 2, -depth / 2, z0 - 0.15), (x1 - x0, depth, 0.3), "lv_soil")
    rng = random.Random(seed)
    x = x0 + rng.uniform(0.1, 0.6)
    while x < x1:
        ang = math.radians(rng.uniform(38, 58))
        ln = rng.uniform(0.5, 1.4)
        c = (x, -0.0005, z0 + rng.uniform(0.3, ztop - z0 - 0.3))
        dx, dz = math.cos(ang) * ln / 2, math.sin(ang) * ln / 2
        if x0 + 0.05 < c[0] - dx and c[0] + dx < x1 - 0.05 and z0 + 0.02 < c[2] - dz and c[2] + dz < ztop:
            g.box(c, (ln, 0.001, 0.012), "lv_joint", rot=flat_rot(ang))
        if rng.random() < 0.5:
            hl = rng.uniform(0.3, 0.8)
            c2 = (x + rng.uniform(-0.2, 0.2), -0.0005, z0 + rng.uniform(0.3, ztop - z0 - 0.2))
            if x0 + 0.05 < c2[0] - hl / 2 and c2[0] + hl / 2 < x1 - 0.05:
                g.box(c2, (hl, 0.001, 0.01), "lv_joint", rot=flat_rot(math.radians(rng.uniform(-8, 8))))
        x += rng.uniform(0.5, 1.0)
    g.objects(col, "wall")


def text(col, body, x, z, size=0.08, mat="label"):
    L.COL = col
    return L.label(body, x, -0.02, z, size, mat)


GROUPS = []  # (name, collection, px per metre)


def group(name, ppm):
    col = new_col(name)
    GROUPS.append((name, col, ppm))
    return col


def build_all():
    full, grown = SIZE["full"], SIZE["grown"]
    Wf, Hf = full[1] * VOX, full[2] * VOX
    gap = 0.9

    # 1. full size, both variants, smooth
    X = 0.0
    col = group("full-smooth", 360)
    for n, var in enumerate(VARIANTS):
        x = X + n * (Wf + gap)
        place_smooth(col, "full", var, x)
        st = stand("full", var)[0]
        text(col, f"full 10x16 v, {var[0]} ({st.coverage() * 100:.0f} % leafed)", x + Wf / 2, Hf + 0.14)
    wall(col, X - 0.4, X + 2 * Wf + gap + 0.4, 0.0, Hf + 0.3, seed=1)

    # 2. full size, both variants, voxelized 0.125
    X = 20.0
    col = group("full-vox125", 360)
    for n, var in enumerate(VARIANTS):
        x = X + n * (Wf + gap)
        place_vox(col, "full", var, VOX, x)
        s = voxels("full", var, VOX)[1]
        text(col, f"full {var[0]}: {s['cells']} voxels @ 0.125 m", x + Wf / 2, Hf + 0.14)
    wall(col, X - 0.4, X + 2 * Wf + gap + 0.4, 0.0, Hf + 0.3, seed=1)

    # 3. compare: smooth | voxel for each variant
    X = 40.0
    col = group("compare-full-smooth-vox", 300)
    x = X
    for var in reversed(VARIANTS):
        place_smooth(col, "full", var, x)
        text(col, f"{var[0]} smooth", x + Wf / 2, Hf + 0.14)
        x += Wf + 0.5
        place_vox(col, "full", var, VOX, x)
        text(col, f"{var[0]} voxels 0.125", x + Wf / 2, Hf + 0.14)
        x += Wf + gap
    wall(col, X - 0.4, x - gap + 0.4, 0.0, Hf + 0.3, seed=2)

    # 4/5. growth rows (both variants stacked), smooth and voxel
    for X, name, vox in ((60.0, "growth-smooth", None), (80.0, "growth-vox125", VOX)):
        col = group(name, 300)
        for r, var in enumerate(reversed(VARIANTS)):
            z = r * (Hf + 0.9)
            x = X
            for sz in SIZES:
                W = sz[1] * VOX
                if vox:
                    place_vox(col, sz[0], var, vox, x, z)
                    lab = f"{sz[0]} {sz[1]}x{sz[2]}: {voxels(sz[0], var, vox)[1]['cells']} vox"
                else:
                    place_smooth(col, sz[0], var, x, z)
                    lab = f"{sz[0]} {sz[1]}x{sz[2]} v"
                text(col, lab, x + W / 2, z + sz[2] * VOX + 0.12, 0.07)
                x += W + 0.55
            text(col, var[0], X - 0.45, z + 0.3, 0.1)
            wall(col, X - 0.7, x - 0.15, z, z + Hf + 0.3, seed=3 + r)

    # 6. full at 0.25 m
    X = 100.0
    col = group("full-vox250", 360)
    for n, var in enumerate(VARIANTS):
        x = X + n * (Wf + gap)
        place_vox(col, "full", var, 0.25, x)
        s = voxels("full", var, 0.25)[1]
        text(col, f"full {var[0]}: {s['cells']} voxels @ 0.25 m", x + Wf / 2, Hf + 0.14)
    wall(col, X - 0.4, X + 2 * Wf + gap + 0.4, 0.0, Hf + 0.3, seed=1)

    # 7. true scale beside the frondgrazer and a bloomcrown (lineup builders, ladder sizes)
    X = 120.0
    col = group("true-scale", 240)
    L.COL = col
    L.ORG.clear()
    place_smooth(col, "full", VARIANTS[1], X)
    Lg, Wg, Hg = L.LADDER_ANIMALS["frondgrazer"]
    L.frondgrazer(X - 0.45, -0.3, Lg, Wg, Hg, overlays=False)
    i, h, r, layers = L.plant_size("ladder", "bloomcrown", 1.0)
    L.bloomcrown(X + Wf + 0.5, -0.4, h, r, layers, i, random.Random("bloomcrown:1.0:ladder"))
    L.post(X + Wf + 1.15, -0.3)
    text(col, "frondgrazer adult 0.75 m", X - 0.45, Hg + 0.12, 0.06, "sublabel")
    text(col, f"bloomcrown adult {h:.2f} m", X + Wf + 0.5, h + 0.12, 0.06, "sublabel")
    text(col, "latticevine full, dense, 1.25 x 2 m", X + Wf / 2, Hf + 0.14, 0.07)
    wall(col, X - 1.1, X + Wf + 1.5, 0.0, Hf + 0.35, depth=0.85, seed=7)


def build_thin():
    """Fix 1 shots: the dense variant only, thin rule, written as *-thin.png."""
    full = SIZE["full"]
    Wf, Hf = full[1] * VOX, full[2] * VOX
    dense = VARIANTS[1]

    col = group("full-vox125-thin", 360)
    place_vox(col, "full", dense, VOX, 0.0, thin=True)
    text(col, f"full dense, thin rule: {voxels_thin('full', dense, VOX)[1]['cells']} voxels @ 0.125 m", Wf / 2, Hf + 0.14)
    wall(col, -0.4, Wf + 0.4, 0.0, Hf + 0.3, seed=1)

    X = 20.0
    col = group("compare-full-smooth-vox-thin", 300)
    for n, (lab, fn) in enumerate((("dense smooth", None), ("old rule (half-voxel)", "old"), ("thin rule", "thin"))):
        x = X + n * (Wf + 0.5)
        if fn is None:
            place_smooth(col, "full", dense, x)
        else:
            place_vox(col, "full", dense, VOX, x, thin=(fn == "thin"))
        text(col, lab, x + Wf / 2, Hf + 0.14)
    wall(col, X - 0.4, X + 3 * Wf + 1.0 + 0.4, 0.0, Hf + 0.3, seed=2)

    X = 40.0
    col = group("growth-vox125-thin", 300)
    x = X
    for sz in SIZES:
        W = sz[1] * VOX
        place_vox(col, sz[0], dense, VOX, x, thin=True)
        text(col, f"{sz[0]} {sz[1]}x{sz[2]}: {voxels_thin(sz[0], dense, VOX)[1]['cells']} vox", x + W / 2,
             sz[2] * VOX + 0.12, 0.07)
        x += W + 0.55
    wall(col, X - 0.4, x - 0.15, 0.0, Hf + 0.3, seed=3)

    X = 60.0
    col = group("full-vox250-thin", 360)
    place_vox(col, "full", dense, 0.25, X, thin=True)
    text(col, f"full dense, thin rule: {voxels_thin('full', dense, 0.25)[1]['cells']} voxels @ 0.25 m", X + Wf / 2, Hf + 0.14)
    wall(col, X - 0.4, X + Wf + 0.4, 0.0, Hf + 0.3, seed=1)


# --- cameras ---------------------------------------------------------------------------

def frame(name, col, ppm, pad=0.12):
    xs, zs = [], []
    for ob in col.objects:
        loc = ob.location
        if ob.type == "MESH":
            for v in ob.data.vertices:
                p = loc + v.co
                xs.append(p.x)
                zs.append(p.z + 0.5 * p.y)
        elif ob.type == "FONT":
            xs.append(loc.x)
            zs.append(loc.z + 0.5 * loc.y)
    x0, x1, z0, z1 = min(xs) - pad, max(xs) + pad, min(zs) - pad, max(zs) + pad + 0.1
    w, h = int((x1 - x0) * ppm), int((z1 - z0) * ppm)
    cam = bpy.data.cameras.new(name)
    cam.type = "ORTHO"
    cam.ortho_scale = max(x1 - x0, z1 - z0)
    cam.clip_end = 200
    ob = bpy.data.objects.new(name, cam)
    bpy.context.scene.collection.objects.link(ob)
    ob.location = ((x0 + x1) / 2, -60, (z0 + z1) / 2)
    ob.rotation_euler = (math.radians(90), 0, 0)
    return ob, w, h


def main():
    global SRC
    os.makedirs(OUT, exist_ok=True)
    sc = L.setup_scene()
    # flat hexes: studio specular washes the dark dossier colours to grey on the face-on
    # rock, so it is off (everything else is the lineup's Workbench setup)
    sc.display.shading.show_specular_highlight = False
    SRC = new_col("sources", link=False)
    if THIN_ONLY:
        build_thin()
        cams = [(name, *frame(f"cam-{name}", col, ppm)) for name, col, ppm in GROUPS]
        bpy.ops.wm.save_as_mainfile(filepath=os.path.join(OUT, "latticevine-thin.blend"))
    else:
        build_all()
        cams = [(name, *frame(f"cam-{name}", col, ppm)) for name, col, ppm in GROUPS]
        # the panel-density copy of the true-scale shot: 12 px per voxel (96 px per metre)
        tcol = dict((n, c) for n, c, _ in GROUPS)["true-scale"]
        cams.append(("true-scale-panel-px", *frame("cam-true-scale-panel-px", tcol, 96)))
        bpy.ops.wm.save_as_mainfile(filepath=os.path.join(OUT, "latticevine.blend"))

    shear = Matrix.Identity(4)
    shear[2][1] = 0.5
    root = bpy.data.objects.new("oblique", None)
    sc.collection.objects.link(root)
    for ob in list(sc.objects):
        if ob.type in {"MESH", "FONT"}:
            ob.parent = root
            ob.matrix_parent_inverse = shear
    for name, cam, w, h in cams:
        L.render(cam, os.path.join(OUT, f"{name}.png"), w, h)

    if THIN_ONLY:
        lines = []
        dense = VARIANTS[1]
        for sz, vox in [(s, VOX) for s in SIZES] + [(SIZE["full"], 0.25)]:
            st = stand(sz[0], dense)[0]
            s = voxels_thin(sz[0], dense, vox)[1]
            lines.append(f"{sz[0]:8s} dense thin @{vox}: {s['cells']:4d} voxels (envelope cover {s['cover'] * 100:4.0f} %, "
                         f"lower band {s['band'][0]}/{s['band'][1]} columns filled, in-rock dropped "
                         f"{s['dropped_in_rock']})  bells {s['bells']}/{s['flowers']} mouths {s['mouths']}/{s['flowers']} "
                         f"(split {s['split']}) bunches {s['bunches']}/{s['fruit']}")
            lines.append("    by material: " + ", ".join(f"{k[3:]} {v}" for k, v in sorted(s["by"].items())))
            if vox == VOX:
                o = voxels(sz[0], dense, vox)[1]
                lines.append(f"    old rule: {o['cells']} voxels, cover {o['cover'] * 100:.0f} %, "
                             f"lower band {o['band'][0]}/{o['band'][1]}")
        with open(os.path.join(OUT, "stats-thin.txt"), "w") as fh:
            fh.write("\n".join(lines) + "\n")
        print("\n".join(lines), flush=True)
        return

    lines = []
    for sz in SIZES:
        for var in VARIANTS:
            st = stand(sz[0], var)[0]
            s = voxels(sz[0], var, VOX)[1]
            lines.append(f"{sz[0]:8s} {var[0]:5s} leafed {st.coverage() * 100:5.1f} %  nodes {len(st.nodes):4d}  "
                         f"rosettes {len(st.rosettes):3d}  voxels@0.125 {s['cells']:4d} (cover {s['cover'] * 100:4.0f} %, "
                         f"in-rock dropped {s['dropped_in_rock']})  bells {s['bells']}/{s['flowers']} "
                         f"mouths {s['mouths']}/{s['flowers']} bunches {s['bunches']}/{s['fruit']}  "
                         f"interior spurs {sum(i['interior'] for _, i in st.spurs)}/{len(st.spurs)}")
            lines.append("    by material: " + ", ".join(f"{k[3:]} {v}" for k, v in sorted(s["by"].items())))
    for var in VARIANTS:
        s = voxels("full", var, 0.25)[1]
        lines.append(f"full     {var[0]:5s} voxels@0.25 {s['cells']} bells {s['bells']}/{s['flowers']} "
                     f"mouths {s['mouths']}/{s['flowers']} bunches {s['bunches']}/{s['fruit']}")
        lines.append("    by material: " + ", ".join(f"{k[3:]} {v}" for k, v in sorted(s["by"].items())))
    with open(os.path.join(OUT, "stats.txt"), "w") as fh:
        fh.write("\n".join(lines) + "\n")
    print("\n".join(lines), flush=True)


main()
