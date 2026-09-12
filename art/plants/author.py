#!/usr/bin/env python3
"""One-time authoring of the Cubarium plant parts and scenes.

The checked-in SVGs and .tscn files are the source of truth for the asset pipeline; this
script is how Fable first painted them. Grids are pixel art: one letter per pixel.
"""
from pathlib import Path
import math

REPO = Path('/home/wrysk/wryskware/cubarium')
PARTS = REPO / 'art' / 'parts'
SCENES = REPO / 'art' / 'plants'
SCENES.mkdir(exist_ok=True)

PAL = {
    'O': '#18115D',  # outline indigo
    'I': '#1E2798',  # indigo blue
    'B': '#1E9BF2',  # blue
    'C': '#42C5F8',  # electric cyan
    'V': '#5A1D9D',  # violet
    'M': '#B037C4',  # magenta
    'D': '#510B6D',  # dark violet (detritus)
    'W': '#FF9B50',  # warm accent
    'P': '#FF2AFC',  # hot magenta tips
    'T': '#1F8A96',  # turquoise
    'G': '#7BEBC9',  # mint
    'N': '#123B4C',  # deep teal edge
    'h': ('#42C5F8', 0.45),  # cyan halo (bioluminescence)
    'w': ('#FF9B50', 0.45),  # warm halo
    'm': ('#FF2AFC', 0.28),  # magenta halo
}

parts: dict[str, list[str]] = {}

def part(name: str, grid: str) -> str:
    rows = [r for r in grid.strip('\n').split('\n')]
    w = len(rows[0])
    assert all(len(r) == w for r in rows), name
    parts[name] = rows
    return name

def procedural(name: str, size: int, fn) -> str:
    rows = []
    for y in range(size):
        row = ''
        for x in range(size):
            row += fn(x + 0.5 - size / 2, y + 0.5 - size / 2) or '.'
        rows.append(row)
    return part(name, '\n'.join(rows))

# --- lanternstalk -------------------------------------------------------------------
part('plant_lanternstalk_stalk2', """
.OVO.
.OVO.
OVVO.
.OVO.
.OVVO
.OVO.
.OVO.
.OVO.
OOVOO
""")
part('plant_lanternstalk_bulb2', """
...h...
..OOO..
.OBCBO.
hOCCCOh
.OBCBO.
..OOO..
...h...
""")
part('plant_lanternstalk_bulb2f', """
...w...
..OOO..
.OWCWO.
wOWWWOw
.OWWWO.
..OOO..
...w...
""")
part('plant_lanternstalk_berry', """
.O.
OWO
.O.
""")
part('plant_lanternstalk_stalk1', """
OVO
OVO
OVO
OVO
OVO
""")
part('plant_lanternstalk_bulb1', """
..h..
.OBO.
hBCBh
.OBO.
..h..
""")
part('plant_lanternstalk_sprout', """
.C.
OVO
OVO
""")

# --- tendrilfan ---------------------------------------------------------------------
# Recolored 2026-09-12: flora leans teal; magenta stays on tendril tips and berries only.
part('plant_tendrilfan_base2', """
.OTTTO.
OTGGGTO
OOTTTOO
""")
part('plant_tendrilfan_tendril2', """
.PT..
PTO..
TO...
OT...
.TO..
.TO..
..TO.
..TTO
""")
part('plant_tendrilfan_center2', """
.PP
PTO
TO.
OT.
.TO
.TO
.TO
.TO
OTO
""")
part('plant_tendrilfan_berry', """
.ww.
wCWw
wWWw
.ww.
""")
part('plant_tendrilfan_base1', """
.OTO.
OTTTO
""")
part('plant_tendrilfan_tendril1', """
.PT
PT.
TO.
.TO
.TO
""")
part('plant_tendrilfan_sprout', """
P.P
.T.
OTO
""")

# --- glowcap ------------------------------------------------------------------------
part('plant_glowcap_cap2', """
..OMMMO..
.OVVVVVO.
OVVVVVVVO
""")
part('plant_glowcap_gills2', """
C.C.C.C
.hhhhh.
""")
part('plant_glowcap_cap2b', """
.OMMMO.
OVVVVVO
""")
part('plant_glowcap_gills2b', """
C.C.C
.hhh.
""")
part('plant_glowcap_stem2', """
OVO
OVO
OVO
OVO
OVO
OVO
""")
part('plant_glowcap_cap1', """
.OMMMO.
OVVVVVO
""")
part('plant_glowcap_stem1', """
OVO
OVO
OVO
OVO
""")
part('plant_glowcap_sprout', """
.O.
OMO
OVO
""")

# --- rootveil -----------------------------------------------------------------------
part('plant_rootveil_veil2', """
V...V.V...V
.V.V...V.V.
..VDV.VDV..
.VDDDVDDDV.
..DDDDDDD..
""")
part('plant_rootveil_glints2', """
...........
...........
..B.....B..
.....B.....
...B...B...
""")
part('plant_rootveil_veil1', """
V..V..V
.V.V.V.
..DDD..
""")
part('plant_rootveil_glints1', """
.......
.......
..B.B..
""")
part('plant_rootveil_sprout', """
.V.
VDV
""")

# --- umbrellafrond (procedural, top-down) ----------------------------------------------
def frond(radius: float, ribs: int, edge: bool):
    def fn(x, y):
        d = math.hypot(x, y)
        if d < 1.0:
            return None  # the glowing center is its own sprite
        on_rib = False
        ang = math.atan2(y, x)
        for k in range(ribs):
            a = k * 2 * math.pi / ribs
            # distance from the rib line, only in the rib's half-plane
            along = x * math.cos(a) + y * math.sin(a)
            across = -x * math.sin(a) + y * math.cos(a)
            if along > 0.5 and abs(across) < 0.55:
                on_rib = True
        if d <= radius:
            if on_rib and d > radius - 1.2:
                return 'C'
            return 'G' if on_rib else 'T'
        if edge and d <= radius + 0.8:
            return 'N'
        return None
    return fn

procedural('plant_umbrellafrond_frond2', 15, frond(6.2, 8, True))
procedural('plant_umbrellafrond_frond1', 11, frond(4.0, 4, True))
part('plant_umbrellafrond_center', """
.C.
CCC
.C.
""")
part('plant_umbrellafrond_sprout', """
.G.
GCG
.G.
""")

# --- bloomcrown (procedural, top-down) --------------------------------------------------
def petals(count: int, dist: float, ra: float, rt: float, tip: float):
    def fn(x, y):
        best = None
        for k in range(count):
            a = k * 2 * math.pi / count + math.pi / count
            cx, cy = dist * math.cos(a), dist * math.sin(a)
            # petal frame: radial and tangential coordinates about the petal center
            along = (x - cx) * math.cos(a) + (y - cy) * math.sin(a)
            across = -(x - cx) * math.sin(a) + (y - cy) * math.cos(a)
            e = (along / ra) ** 2 + (across / rt) ** 2
            if e <= 1.0:
                r = math.hypot(x, y)
                return 'M' if r > tip else 'V'
            if e <= 1.9:
                best = 'O'
        return best
    return fn

procedural('plant_bloomcrown_petals2', 15, petals(6, 3.3, 2.4, 1.35, 4.9))
procedural('plant_bloomcrown_petals1', 11, petals(4, 2.2, 1.7, 1.0, 3.2))
part('plant_bloomcrown_center2', """
.MMM.
MWWWM
MWWWM
MWWWM
.MMM.
""")
part('plant_bloomcrown_center1', """
.M.
MWM
.M.
""")
part('plant_bloomcrown_fruit2', """
.OOO.
OCWWO
OWWWO
OWWWO
.OOO.
""")
part('plant_bloomcrown_sprout', """
.M.
OVO
.V.
""")

# --- reedspire ------------------------------------------------------------------------
def reed(name: str, h: int) -> str:
    rows = ['.C', 'OC'] + ['OB'] * (h - 2)
    return part(name, '\n'.join(rows))

reed('plant_reedspire_reed11', 11)
reed('plant_reedspire_reed8', 8)
reed('plant_reedspire_reed6', 6)
reed('plant_reedspire_reed7', 7)
reed('plant_reedspire_reed5', 5)
reed('plant_reedspire_reed3', 3)
part('plant_reedspire_ripple2', """
BCBCBCBC
""")
part('plant_reedspire_ripple1', """
CBCBCB
""")

# --- scenes -----------------------------------------------------------------------------
# A plant scene: pivots (Node2D) each holding Sprite2D parts placed by tile top-left.
# Tile coordinates run 0..16 with the pivot of the whole tile at (8, 8).

L = 3.0  # clip length in seconds; frames are sampled at 0, L/4, L/2, 3L/4

def S(node, name, topleft, flip_h=False):
    return dict(node=node, part=name, topleft=topleft, flip_h=flip_h)

PLANTS = [
    dict(name='lanternstalk', band='foliage', pivots=[
        dict(name='Sprout', at=(8.5, 15), sprites=[S('Sprite', 'plant_lanternstalk_sprout', (7, 12))]),
        dict(name='Stalk1', at=(8.5, 15), sprites=[S('Stalk', 'plant_lanternstalk_stalk1', (7, 10)), S('Bulb', 'plant_lanternstalk_bulb1', (6, 6))]),
        dict(name='Stalk2', at=(8.5, 15), sprites=[S('Stalk', 'plant_lanternstalk_stalk2', (6, 6)), S('Bulb', 'plant_lanternstalk_bulb2', (5, 2))]),
        dict(name='Fruit', at=(8.5, 15), sprites=[S('Stalk', 'plant_lanternstalk_stalk2', (6, 6)), S('Bulb', 'plant_lanternstalk_bulb2f', (5, 2)), S('BerryL', 'plant_lanternstalk_berry', (5, 7)), S('BerryR', 'plant_lanternstalk_berry', (9, 9))]),
    ], clips=dict(
        stage0=dict(show=['Sprout'], rot={}, mod={'Sprout/Sprite': [0.7, 1.0, 0.7, 1.0]}),
        stage1=dict(show=['Stalk1'], rot={'Stalk1': [0, 0.10, 0, -0.10]}, mod={'Stalk1/Bulb': [0.7, 0.85, 1.0, 0.85]}),
        stage2=dict(show=['Stalk2'], rot={'Stalk2': [0, 0.09, 0, -0.09]}, mod={'Stalk2/Bulb': [0.7, 0.85, 1.0, 0.85]}),
        fruit=dict(show=['Fruit'], rot={'Fruit': [0, 0.09, 0, -0.09]}, mod={'Fruit/Bulb': [1.0, 0.85, 0.7, 0.85], 'Fruit/BerryL': [0.7, 1.0, 0.85, 0.6], 'Fruit/BerryR': [1.0, 0.7, 0.6, 0.85]}),
    )),
    dict(name='tendrilfan', band='foliage', pivots=[
        dict(name='Sprout', at=(8.5, 15), sprites=[S('Sprite', 'plant_tendrilfan_sprout', (7, 12))]),
        dict(name='Base1', at=(8.5, 15), sprites=[S('Sprite', 'plant_tendrilfan_base1', (6, 13))]),
        dict(name='TendrilL1', at=(8, 14), sprites=[S('Sprite', 'plant_tendrilfan_tendril1', (5, 8))]),
        dict(name='TendrilR1', at=(9, 14), sprites=[S('Sprite', 'plant_tendrilfan_tendril1', (9, 8), flip_h=True)]),
        dict(name='Base2', at=(8.5, 15), sprites=[S('Sprite', 'plant_tendrilfan_base2', (5, 12))]),
        dict(name='Center2', at=(8.5, 13), sprites=[S('Sprite', 'plant_tendrilfan_center2', (7, 3)), S('Berry', 'plant_tendrilfan_berry', (6, 1))]),
        dict(name='TendrilL2', at=(8, 13), sprites=[S('Sprite', 'plant_tendrilfan_tendril2', (3, 5)), S('Berry', 'plant_tendrilfan_berry', (2, 3))]),
        dict(name='TendrilR2', at=(9, 13), sprites=[S('Sprite', 'plant_tendrilfan_tendril2', (9, 5), flip_h=True), S('Berry', 'plant_tendrilfan_berry', (10, 3))]),
    ], clips=dict(
        stage0=dict(show=['Sprout'], rot={}, mod={'Sprout/Sprite': [0.7, 1.0, 0.7, 1.0]}),
        stage1=dict(show=['Base1', 'TendrilL1', 'TendrilR1'], rot={'TendrilL1': [0, 0.14, 0, -0.14], 'TendrilR1': [0.14, 0, -0.14, 0]}, mod={}),
        stage2=dict(show=['Base2', 'Center2', 'TendrilL2', 'TendrilR2'], rot={'TendrilL2': [0, 0.12, 0, -0.12], 'TendrilR2': [0.12, 0, -0.12, 0], 'Center2': [0, 0.05, 0, -0.05]}, mod={}, hide=['Center2/Berry', 'TendrilL2/Berry', 'TendrilR2/Berry']),
        fruit=dict(show=['Base2', 'Center2', 'TendrilL2', 'TendrilR2'], rot={'TendrilL2': [0, 0.12, 0, -0.12], 'TendrilR2': [0.12, 0, -0.12, 0], 'Center2': [0, 0.05, 0, -0.05]}, mod={'Center2/Berry': [1.0, 0.75, 0.6, 0.8], 'TendrilL2/Berry': [0.7, 1.0, 0.8, 0.6], 'TendrilR2/Berry': [0.6, 0.8, 1.0, 0.75]}),
    )),
    dict(name='glowcap', band='soil', pivots=[
        dict(name='Sprout', at=(8.5, 15), sprites=[S('Sprite', 'plant_glowcap_sprout', (7, 12))]),
        dict(name='Stem1', at=(8.5, 15), sprites=[S('Sprite', 'plant_glowcap_stem1', (7, 11))]),
        dict(name='Cap1', at=(8.5, 10), sprites=[S('Cap', 'plant_glowcap_cap1', (5, 8)), S('Gills', 'plant_glowcap_gills2b', (6, 10))]),
        dict(name='Stem2', at=(8.5, 15), sprites=[S('Sprite', 'plant_glowcap_stem2', (7, 9))]),
        dict(name='Cap2', at=(8.5, 9), sprites=[S('Cap', 'plant_glowcap_cap2', (4, 4)), S('Gills', 'plant_glowcap_gills2', (5, 7))]),
        dict(name='Cap2b', at=(10.5, 11), sprites=[S('Cap', 'plant_glowcap_cap2b', (8, 9)), S('Gills', 'plant_glowcap_gills2b', (9, 11))]),
    ], clips=dict(
        stage0=dict(show=['Sprout'], rot={}, mod={'Sprout/Sprite': [0.7, 1.0, 0.7, 1.0]}),
        stage1=dict(show=['Stem1', 'Cap1'], rot={'Cap1': [0, 0.05, 0, -0.05]}, mod={'Cap1/Gills': [0.5, 0.75, 1.0, 0.85]}),
        stage2=dict(show=['Stem2', 'Cap2', 'Cap2b'], rot={'Cap2': [0, 0.05, 0, -0.05], 'Cap2b': [0.05, 0, -0.05, 0]}, mod={'Cap2/Gills': [0.5, 0.75, 1.0, 0.85], 'Cap2b/Gills': [1.0, 0.8, 0.5, 0.7]}),
    )),
    dict(name='rootveil', band='soil', pivots=[
        dict(name='Sprout', at=(8.5, 15), sprites=[S('Sprite', 'plant_rootveil_sprout', (7, 13))]),
        dict(name='Veil1', at=(8.5, 15), sprites=[S('Veil', 'plant_rootveil_veil1', (5, 12)), S('Glints', 'plant_rootveil_glints1', (5, 12))]),
        dict(name='Veil2', at=(8.5, 15), sprites=[S('Veil', 'plant_rootveil_veil2', (3, 10)), S('Glints', 'plant_rootveil_glints2', (3, 10))]),
    ], clips=dict(
        stage0=dict(show=['Sprout'], rot={}, mod={'Sprout/Sprite': [0.7, 1.0, 0.7, 1.0]}),
        stage1=dict(show=['Veil1'], rot={}, mod={'Veil1/Glints': [0.4, 0.7, 1.0, 0.85]}),
        stage2=dict(show=['Veil2'], rot={}, mod={'Veil2/Glints': [0.4, 0.7, 1.0, 0.85]}),
    )),
    dict(name='umbrellafrond', band='canopy', pivots=[
        dict(name='Sprout', at=(8.5, 8.5), sprites=[S('Sprite', 'plant_umbrellafrond_sprout', (7, 7))]),
        dict(name='Frond1', at=(8.5, 8.5), sprites=[S('Frond', 'plant_umbrellafrond_frond1', (3, 3)), S('Center', 'plant_umbrellafrond_center', (7, 7))]),
        dict(name='Frond2', at=(8.5, 8.5), sprites=[S('Frond', 'plant_umbrellafrond_frond2', (1, 1)), S('Center', 'plant_umbrellafrond_center', (7, 7))]),
    ], clips=dict(
        stage0=dict(show=['Sprout'], rot={}, mod={'Sprout/Sprite': [0.7, 1.0, 0.7, 1.0]}),
        stage1=dict(show=['Frond1'], rot={'Frond1': [0, 0.10, 0, -0.10]}, mod={'Frond1/Center': [0.6, 0.8, 1.0, 0.9]}),
        stage2=dict(show=['Frond2'], rot={'Frond2': [0, 0.08, 0, -0.08]}, mod={'Frond2/Center': [0.6, 0.8, 1.0, 0.9]}),
    )),
    dict(name='bloomcrown', band='canopy', pivots=[
        dict(name='Sprout', at=(8.5, 8.5), sprites=[S('Sprite', 'plant_bloomcrown_sprout', (7, 7))]),
        dict(name='Bloom1', at=(8.5, 8.5), sprites=[S('Petals', 'plant_bloomcrown_petals1', (3, 3)), S('Center', 'plant_bloomcrown_center1', (7, 7))]),
        dict(name='Bloom2', at=(8.5, 8.5), sprites=[S('Petals', 'plant_bloomcrown_petals2', (1, 1)), S('Center', 'plant_bloomcrown_center2', (6, 6))]),
        dict(name='Fruit', at=(8.5, 8.5), sprites=[S('Petals', 'plant_bloomcrown_petals2', (1, 1)), S('Fruit', 'plant_bloomcrown_fruit2', (6, 6))]),
    ], clips=dict(
        stage0=dict(show=['Sprout'], rot={}, mod={'Sprout/Sprite': [0.7, 1.0, 0.7, 1.0]}),
        stage1=dict(show=['Bloom1'], rot={'Bloom1': [0, 0.10, 0, -0.10]}, mod={'Bloom1/Center': [0.7, 0.85, 1.0, 0.9]}),
        stage2=dict(show=['Bloom2'], rot={'Bloom2': [0, 0.08, 0, -0.08]}, mod={'Bloom2/Center': [0.7, 0.85, 1.0, 0.9]}),
        fruit=dict(show=['Fruit'], rot={'Fruit': [0, 0.08, 0, -0.08]}, mod={'Fruit/Petals': [0.7, 0.7, 0.7, 0.7], 'Fruit/Fruit': [1.0, 0.85, 0.65, 0.75]}),
    )),
    dict(name='reedspire', band='water', pivots=[
        dict(name='Sprout', at=(8, 15), sprites=[S('Sprite', 'plant_reedspire_reed3', (7, 12))]),
        dict(name='Ripple1', at=(8, 15), sprites=[S('Sprite', 'plant_reedspire_ripple1', (5, 14))]),
        dict(name='ReedA1', at=(7, 15), sprites=[S('Sprite', 'plant_reedspire_reed7', (6, 8))]),
        dict(name='ReedB1', at=(10, 15), sprites=[S('Sprite', 'plant_reedspire_reed5', (9, 10))]),
        dict(name='Ripple2', at=(8, 15), sprites=[S('Sprite', 'plant_reedspire_ripple2', (4, 14))]),
        dict(name='ReedA2', at=(6, 15), sprites=[S('Sprite', 'plant_reedspire_reed11', (5, 4))]),
        dict(name='ReedB2', at=(9, 15), sprites=[S('Sprite', 'plant_reedspire_reed8', (8, 7))]),
        dict(name='ReedC2', at=(11, 15), sprites=[S('Sprite', 'plant_reedspire_reed6', (10, 9))]),
    ], clips=dict(
        stage0=dict(show=['Sprout'], rot={}, mod={'Sprout/Sprite': [0.7, 1.0, 0.7, 1.0]}),
        stage1=dict(show=['Ripple1', 'ReedA1', 'ReedB1'], rot={'ReedA1': [0, 0.12, 0, -0.12], 'ReedB1': [0.12, 0, -0.12, 0]}, mod={'Ripple1/Sprite': [0.6, 0.8, 1.0, 0.8]}),
        stage2=dict(show=['Ripple2', 'ReedA2', 'ReedB2', 'ReedC2'], rot={'ReedA2': [0, 0.10, 0, -0.10], 'ReedB2': [0.10, 0, -0.10, 0], 'ReedC2': [0, -0.10, 0, 0.10]}, mod={'Ripple2/Sprite': [0.6, 0.8, 1.0, 0.8]}),
    )),
]

def write_svgs():
    for name, rows in parts.items():
        h, w = len(rows), len(rows[0])
        by_color: dict[str, list[str]] = {}
        for y, row in enumerate(rows):
            for x, ch in enumerate(row):
                if ch == '.':
                    continue
                by_color.setdefault(PAL[ch], []).append(f'M{x} {y}h1v1h-1z')
        def attrs(c):
            return f'fill="{c[0]}" fill-opacity="{c[1]}"' if isinstance(c, tuple) else f'fill="{c}"'
        paths = ''.join(f'\n  <path {attrs(c)} d="{"".join(d)}"/>' for c, d in by_color.items())
        svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" '
               f'shape-rendering="crispEdges">{paths}\n</svg>\n')
        (PARTS / f'{name}.svg').write_text(svg)

def fmt(v: float) -> str:
    s = f'{v:.4f}'.rstrip('0').rstrip('.')
    return s if s not in ('', '-0') else '0'

def color(v: float) -> str:
    return f'Color({fmt(v)}, {fmt(v)}, {fmt(v)}, 1)'

def track(i: int, path: str, update: int, times: list[float], values: list[str]) -> str:
    return (f'tracks/{i}/type = "value"\n'
            f'tracks/{i}/path = NodePath("{path}")\n'
            f'tracks/{i}/interp = 1\n'
            f'tracks/{i}/enabled = true\n'
            f'tracks/{i}/keys = {{\n'
            f'"times": PackedFloat32Array({", ".join(fmt(t) for t in times)}),\n'
            f'"transitions": PackedFloat32Array({", ".join("1" for _ in times)}),\n'
            f'"update": {update},\n'
            f'"values": [{", ".join(values)}]\n'
            f'}}\n')

def write_scene(p: dict):
    textures: list[str] = []
    for piv in p['pivots']:
        for s in piv['sprites']:
            if s['part'] not in textures:
                textures.append(s['part'])
    clips = p['clips']
    # Which properties any clip animates or which RESET must pin.
    rot_nodes = sorted({n for c in clips.values() for n in c['rot']})
    mod_sprites = sorted({n for c in clips.values() for n in c['mod']})
    vis_sprites = sorted({n for c in clips.values() for n in c.get('hide', [])})
    pivot_names = [piv['name'] for piv in p['pivots']]

    def anim(name: str, length: float, loop: int, tracks: list[str]) -> str:
        body = (f'[sub_resource type="Animation" id="Animation_{name}"]\n'
                f'resource_name = "{name}"\nlength = {fmt(length)}\nloop_mode = {loop}\n\n')
        return body + '\n'.join(tracks) + '\n'

    subs = []
    # RESET: every pivot hidden and unrotated, every animated sprite white and visible.
    t = []
    i = 0
    for n in pivot_names:
        t.append(track(i, f'{n}:visible', 1, [0], ['false'])); i += 1
    for n in rot_nodes:
        t.append(track(i, f'{n}:rotation', 0, [0], ['0.0'])); i += 1
    for n in mod_sprites:
        t.append(track(i, f'{n}:self_modulate', 0, [0], [color(1.0)])); i += 1
    for n in vis_sprites:
        t.append(track(i, f'{n}:visible', 1, [0], ['true'])); i += 1
    subs.append(anim('RESET', 0.001, 0, t))

    times = [0, L / 4, L / 2, 3 * L / 4, L]
    for cname, c in clips.items():
        t = []
        i = 0
        for n in c['show']:
            t.append(track(i, f'{n}:visible', 1, [0], ['true'])); i += 1
        for n in c.get('hide', []):
            t.append(track(i, f'{n}:visible', 1, [0], ['false'])); i += 1
        for n, vals in c['rot'].items():
            t.append(track(i, f'{n}:rotation', 0, times, [fmt(v) for v in vals + [vals[0]]])); i += 1
        for n, vals in c['mod'].items():
            t.append(track(i, f'{n}:self_modulate', 0, times, [color(v) for v in vals + [vals[0]]])); i += 1
        subs.append(anim(cname, L, 1, t))

    lib = '[sub_resource type="AnimationLibrary" id="Library"]\n_data = {\n' + ',\n'.join(
        f'&"{n}": SubResource("Animation_{n}")' for n in ['RESET'] + list(clips)) + '\n}\n'

    ext = ''.join(f'[ext_resource type="Texture2D" path="res://parts/{tex}.svg" id="{k + 1}"]\n\n'
                  for k, tex in enumerate(textures))
    nodes = [f'[node name="{p["name"].capitalize()}" type="Node2D"]\ntexture_filter = 1\n']
    for piv in p['pivots']:
        px, py = piv['at'][0] - 8, piv['at'][1] - 8
        nodes.append(f'[node name="{piv["name"]}" type="Node2D" parent="."]\nposition = Vector2({fmt(px)}, {fmt(py)})\nvisible = false\n')
        for s in piv['sprites']:
            rows = parts[s['part']]
            h, w = len(rows), len(rows[0])
            cx = s['topleft'][0] + w / 2 - 8 - px
            cy = s['topleft'][1] + h / 2 - 8 - py
            flip = '\nflip_h = true' if s['flip_h'] else ''
            nodes.append(f'[node name="{s["node"]}" type="Sprite2D" parent="{piv["name"]}"]\n'
                         f'position = Vector2({fmt(cx)}, {fmt(cy)})\n'
                         f'texture = ExtResource("{textures.index(s["part"]) + 1}"){flip}\n')
    nodes.append('[node name="AnimationPlayer" type="AnimationPlayer" parent="."]\nlibraries = {\n&"": SubResource("Library")\n}\nautoplay = "stage2"\n')
    steps = len(textures) + len(subs) + 2
    text = f'[gd_scene load_steps={steps} format=3]\n\n' + ext + '\n'.join(subs) + '\n' + lib + '\n' + '\n'.join(nodes)
    (SCENES / f'{p["name"]}.tscn').write_text(text)

def check_extents():
    """Every stage's painted pixels must stay within the 9-px surface budget from (8,8)."""
    worst = 0.0
    for p in PLANTS:
        for piv in p['pivots']:
            for s in piv['sprites']:
                rows = parts[s['part']]
                for y, row in enumerate(rows):
                    for x, ch in enumerate(row):
                        if ch == '.':
                            continue
                        tx = s['topleft'][0] + x + 0.5 - 8
                        ty = s['topleft'][1] + y + 0.5 - 8
                        e = math.hypot(tx, ty) + math.sqrt(0.5) + 0.5
                        worst = max(worst, e)
                        assert e <= 9.0, (p['name'], s['part'], x, y, e)
    print(f'worst rest-pose extent {worst:.2f} px (budget 9.0; sway adds a little)')

if __name__ == '__main__':
    write_svgs()
    for p in PLANTS:
        write_scene(p)
    check_extents()
    print(f'{len(parts)} parts, {len(PLANTS)} scenes')
