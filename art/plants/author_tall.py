#!/usr/bin/env python3
"""One-time authoring of the tall plants (spiretree, glasscane, vinecoil) and the three
8×8 ground-cover tiles.

The checked-in SVGs and .tscn files are the source of truth; this is how Fable first
painted them. It reuses the palette and pixel-grid conventions of author.py but keeps its
own part registry so the seven original plants are never rewritten.

Tall plants are columns of 16×16 tiles stacked 4 px apart, so a `trunk` tile must be
periodic in y with period 4 (any overlap between neighbouring segments then draws the same
pixels) and must not rotate in its sway: segments at different heights would tear. Trunk
sway is therefore a glow pulse: a sub-pixel drift never moves a nearest-sampled pixel and
a whole-pixel one breaks the 9-px budget at the tile ends, so every tall part pulses only.
"""
from pathlib import Path
import math

REPO = Path('/home/wrysk/wryskware/cubarium')
PARTS = REPO / 'art' / 'parts'
SCENES = REPO / 'art' / 'plants'
GROUND = REPO / 'art' / 'ground'
GROUND.mkdir(exist_ok=True)

PAL = {
    'O': '#18115D',  # outline indigo
    'o': '#2C3488',  # lighter outline for glass edges
    'I': '#1E2798',  # indigo blue
    'B': '#1E9BF2',  # blue
    'C': '#42C5F8',  # electric cyan
    'V': '#5A1D9D',  # violet
    'M': '#B037C4',  # magenta
    'D': '#510B6D',  # dark violet
    'W': '#FF9B50',  # warm accent
    'P': '#FF2AFC',  # hot magenta
    'T': '#1F8A96',  # turquoise
    'G': '#7BEBC9',  # mint
    'N': '#123B4C',  # deep teal edge
    'h': ('#42C5F8', 0.45),  # cyan halo
    'w': ('#FF9B50', 0.45),  # warm halo
    'g': ('#7BEBC9', 0.55),  # mint glass
    'b': ('#1E9BF2', 0.55),  # blue glass
    # ground-cover inks (alpha carries coverage)
    'q': ('#3A1650', 0.55),  # soil grain
    'r': ('#241033', 0.35),  # soil dust
    'Q': ('#B037C4', 0.60),  # soil glint
    'k': ('#1F8A96', 0.45),  # moss weave
    'K': ('#42C5F8', 0.70),  # moss node
    'j': ('#123B4C', 0.30),  # moss shadow
    'f': ('#7BEBC9', 0.50),  # frond mat
    'F': ('#42C5F8', 0.80),  # frond highlight
    'e': ('#1F8A96', 0.35),  # frond shade
    'u': ('#B037C4', 0.30),  # faint glint (dim frame)
    'U': ('#42C5F8', 0.35),  # faint node (dim frame)
    'v': ('#42C5F8', 0.40),  # faint highlight (dim frame)
}

parts: dict[str, list[str]] = {}

def part(name: str, grid: str) -> str:
    rows = grid.strip('\n').split('\n')
    w = len(rows[0])
    assert all(len(r) == w for r in rows), name
    parts[name] = rows
    return name

def periodic_trunk(name: str, period: list[str]) -> str:
    assert len(period) == 4, name
    return part(name, '\n'.join(period * 4))

# --- spiretree ----------------------------------------------------------------------
# Trunk: turquoise body, cyan vein, a mint ring every 4 px (flora leans teal; magenta is for fauna and accents). Period 4 so stacked segments match.
SPIRE = ['OGGO', 'OTCO', 'OTCO', 'OTCO']
periodic_trunk('plant_spiretree_trunk', SPIRE)

def column(name: str, rows: list[str], first_tile_row: int, period: list[str]):
    """A 15-wide part whose rows are either free art or `None`, meaning "the trunk
    pattern for this tile row" (positions 5..8, so it lines up with a trunk tile placed at
    x = 6..9). Every explicit row must be 15 characters."""
    out = []
    for k, row in enumerate(rows):
        tile_row = first_tile_row + k
        if row is None:
            row = '.....' + period[tile_row % 4] + '......'
        assert len(row) == 15, (name, k, row)
        out.append(row)
    return part(name, '\n'.join(out))

# Crown: a mint/cyan dome seen from the side with the trunk running up into it. Placed at
# tile row 0; rows 9–15 carry the trunk period so a trunk segment 4 px lower joins it.
column('plant_spiretree_crown', [
    '.....OOOO......',
    '....OGGGGGO....',
    '...OGGCGGCGO...',
    '..OGCGGGGGGCO..',
    '..OTGGGCGGGGTO.',
    '..OTTGGGGGGTTO.',
    '...OTTTGGGTTO..',
    '....OOTTTTOO...',
    None, None, None, None, None, None, None, None,
], 0, SPIRE)
# Base: the trunk meets a knotted bole with root nubs at the horizon. Placed at tile row 3.
column('plant_spiretree_base', [
    None, None, None, None, None, None,
    '....OTTCTO.....',
    '...OTTNTTTO....',
    '..OTNTTTTTNTO..',
    '..OO.OTTTTO.O..',
    '...O..OTTO.O...',
    '.....OTTTTO....',
    '.....OOOO......',
], 3, SPIRE)

# --- glasscane ----------------------------------------------------------------------
# Trunk: a translucent blue cane, magenta joint every 4 px.
GLASS = ['oMMo', 'oBCo', 'obCo', 'oBCo']
periodic_trunk('plant_glasscane_trunk', GLASS)
# Crown: a cluster of three lantern bulbs on the cane; the right one carries the warm accent.
column('plant_glasscane_crown', [
    '......h........',
    '.....OOO.......',
    '....OBCBO......',
    '....OCCCO......',
    '....OBCBO......',
    '.....OOO.......',
    '.OOO.obCo.OOO..',
    'OBCBOoBCoOBWBO.',
    'OCCCOoMMoOWWWO.',
    'OBCBOoBCoOBWBO.',
    '.OOO.obCo.OOO..',
    '..h..oBCo..w...',
    None, None, None, None,
], 0, GLASS)
# Base: a bulb of root-glass at the horizon, the cane rising from it. Placed at tile row 3.
column('plant_glasscane_base', [
    None, None, None, None, None, None,
    '....obBCbo.....',
    '...obbBCBbbo...',
    '..obbIBCBIbbbo.',
    '..oo.oIbbIo.o..',
    '...o..oIIo.o...',
    '.....obbbbo....',
    '.....oooo......',
], 3, GLASS)

# --- vinecoil -----------------------------------------------------------------------
# Trunk only: a teal tendril spiralling around a hollow centre with a magenta tip once per
# turn, period 4, so it reads over another tree's trunk.
periodic_trunk('plant_vinecoil_trunk', ['PT..', '.OT.', '..TT', '.TO.'])

# --- ground cover (8×8, tileable on a torus) ------------------------------------------
def grit_frames() -> list[str]:
    """Sparse soil grains on the 8-torus: grains where (3x + 5y) % 8 == 0 (eight per tile,
    evenly spread), dust where (3x + 5y) % 8 == 4, and one magenta glint that moves from
    grain to grain across the four frames. Stationary, so the wrap is invisible."""
    grains = [(x, y) for y in range(8) for x in range(8) if (3 * x + 5 * y) % 8 == 0]
    frames = []
    for k in range(4):
        rows = []
        for y in range(8):
            row = ''
            for x in range(8):
                if (x, y) == grains[(2 * k) % len(grains)]:
                    row += 'Q'
                elif (x, y) == grains[(2 * k + 5) % len(grains)]:
                    row += 'u'
                elif (3 * x + 5 * y) % 8 == 0:
                    row += 'q'
                elif (3 * x + 5 * y) % 8 == 4:
                    row += 'r'
                else:
                    row += '.'
            rows.append(row)
        frames.append('\n'.join(rows))
    return frames

def frondmat_frames() -> list[str]:
    """An interlocking frond lattice on the 8-torus: mint fronds along (x + y) % 4 == 0,
    teal shade along (x - y) % 4 == 2, and one cyan highlight per frame that walks the
    lattice so the mat breathes. Periodic in both axes by construction."""
    frames = []
    spots = [(0, 0), (4, 0), (2, 2), (6, 6)]
    for k in range(4):
        rows = []
        for y in range(8):
            row = ''
            for x in range(8):
                if (x, y) == spots[k]:
                    row += 'F'
                elif (x, y) == spots[(k + 2) % 4]:
                    row += 'v'
                elif (x + y) % 4 == 0:
                    row += 'f'
                elif (x - y) % 4 == 2:
                    row += 'e'
                else:
                    row += '.'
            rows.append(row)
        frames.append('\n'.join(rows))
    return frames

GROUND_TILES = {
    'grit': ('soil', grit_frames()),
    'mossweave': ('foliage', [
        """
k.j.k.j.
.k.k.k.k
j.K.j.k.
.k.k.k.k
k.j.k.j.
.k.k.k.k
j.k.j.K.
.k.k.k.k
""",
        """
k.j.k.j.
.k.k.k.k
j.U.j.k.
.k.k.k.k
k.j.k.j.
.k.k.k.k
j.k.j.U.
.k.k.k.k
""",
        """
k.j.k.j.
.k.k.k.k
j.k.j.K.
.k.k.k.k
k.j.k.j.
.k.k.k.k
j.K.j.k.
.k.k.k.k
""",
        """
k.j.k.j.
.k.k.k.k
j.k.j.U.
.k.k.k.k
k.j.k.j.
.k.k.k.k
j.U.j.k.
.k.k.k.k
""",
    ]),
    'frondmat': ('canopy', frondmat_frames()),
}

# --- scenes -------------------------------------------------------------------------
# Clip contract for tall plants: looping `base`, `trunk`, `crown` (whichever parts exist).
# All parts of one species share one pulse sequence, so the rows where a cap overlaps a
# trunk segment render identically in every frame and the join stays invisible.
# Frames are sampled at 0, L/4, L/2, 3L/4. Trunks never rotate (see module doc).
L = 3.0

def S(node, name, topleft, flip_h=False):
    return dict(node=node, part=name, topleft=topleft, flip_h=flip_h)

TALL = [
    dict(name='spiretree', pivots=[
        dict(name='Base', at=(8, 8), sprites=[S('Sprite', 'plant_spiretree_base', (1, 3))]),
        dict(name='Trunk', at=(8, 8), sprites=[S('Sprite', 'plant_spiretree_trunk', (6, 0))]),
        dict(name='Crown', at=(8, 8), sprites=[S('Sprite', 'plant_spiretree_crown', (1, 0))]),
    ], clips=dict(
        base=dict(show=['Base'], rot={}, pos={}, mod={'Base/Sprite': [0.75, 0.9, 1.0, 0.85]}),
        trunk=dict(show=['Trunk'], rot={}, pos={}, mod={'Trunk/Sprite': [0.75, 0.9, 1.0, 0.85]}),
        crown=dict(show=['Crown'], rot={}, pos={}, mod={'Crown/Sprite': [0.75, 0.9, 1.0, 0.85]}),
    )),
    dict(name='glasscane', pivots=[
        dict(name='Base', at=(8, 8), sprites=[S('Sprite', 'plant_glasscane_base', (1, 3))]),
        dict(name='Trunk', at=(8, 8), sprites=[S('Sprite', 'plant_glasscane_trunk', (6, 0))]),
        dict(name='Crown', at=(8, 8), sprites=[S('Sprite', 'plant_glasscane_crown', (1, 0))]),
    ], clips=dict(
        base=dict(show=['Base'], rot={}, pos={}, mod={'Base/Sprite': [1.0, 0.85, 0.7, 0.9]}),
        trunk=dict(show=['Trunk'], rot={}, pos={}, mod={'Trunk/Sprite': [1.0, 0.85, 0.7, 0.9]}),
        crown=dict(show=['Crown'], rot={}, pos={}, mod={'Crown/Sprite': [1.0, 0.85, 0.7, 0.9]}),
    )),
    dict(name='vinecoil', pivots=[
        dict(name='Trunk', at=(8, 8), sprites=[S('Sprite', 'plant_vinecoil_trunk', (6, 0))]),
    ], clips=dict(
        trunk=dict(show=['Trunk'], rot={}, pos={}, mod={'Trunk/Sprite': [0.7, 0.85, 1.0, 0.9]}),
    )),
]

def svg_of(rows: list[str]) -> str:
    h, w = len(rows), len(rows[0])
    by_color: dict = {}
    for y, row in enumerate(rows):
        for x, ch in enumerate(row):
            if ch == '.':
                continue
            by_color.setdefault(PAL[ch], []).append(f'M{x} {y}h1v1h-1z')
    def attrs(c):
        return f'fill="{c[0]}" fill-opacity="{c[1]}"' if isinstance(c, tuple) else f'fill="{c}"'
    paths = ''.join(f'\n  <path {attrs(c)} d="{"".join(d)}"/>' for c, d in by_color.items())
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" '
            f'shape-rendering="crispEdges">{paths}\n</svg>\n')

def write_svgs():
    for name, rows in parts.items():
        (PARTS / f'{name}.svg').write_text(svg_of(rows))
    for name, (_band, frames) in GROUND_TILES.items():
        for k, grid in enumerate(frames):
            rows = grid.strip('\n').split('\n')
            assert len(rows) == 8 and all(len(r) == 8 for r in rows), (name, k)
            (GROUND / f'{name}_{k}.svg').write_text(svg_of(rows))

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
    rot_nodes = sorted({n for c in clips.values() for n in c['rot']})
    pos_nodes = sorted({n for c in clips.values() for n in c['pos']})
    mod_sprites = sorted({n for c in clips.values() for n in c['mod']})
    pivot_names = [piv['name'] for piv in p['pivots']]
    pivot_at = {piv['name']: piv['at'] for piv in p['pivots']}

    def vec(n: str, dx: float, dy: float) -> str:
        px, py = pivot_at[n][0] - 8 + dx, pivot_at[n][1] - 8 + dy
        return f'Vector2({fmt(px)}, {fmt(py)})'

    def anim(name: str, length: float, loop: int, tracks: list[str]) -> str:
        body = (f'[sub_resource type="Animation" id="Animation_{name}"]\n'
                f'resource_name = "{name}"\nlength = {fmt(length)}\nloop_mode = {loop}\n\n')
        return body + '\n'.join(tracks) + '\n'

    subs = []
    t = []
    i = 0
    for n in pivot_names:
        t.append(track(i, f'{n}:visible', 1, [0], ['false'])); i += 1
    for n in rot_nodes:
        t.append(track(i, f'{n}:rotation', 0, [0], ['0.0'])); i += 1
    for n in pos_nodes:
        t.append(track(i, f'{n}:position', 0, [0], [vec(n, 0, 0)])); i += 1
    for n in mod_sprites:
        t.append(track(i, f'{n}:self_modulate', 0, [0], [color(1.0)])); i += 1
    subs.append(anim('RESET', 0.001, 0, t))

    times = [0, L / 4, L / 2, 3 * L / 4, L]
    for cname, c in clips.items():
        t = []
        i = 0
        for n in c['show']:
            t.append(track(i, f'{n}:visible', 1, [0], ['true'])); i += 1
        for n, vals in c['rot'].items():
            t.append(track(i, f'{n}:rotation', 0, times, [fmt(v) for v in vals + [vals[0]]])); i += 1
        for n, vals in c['pos'].items():
            t.append(track(i, f'{n}:position', 0, times, [vec(n, *v) for v in vals + [vals[0]]])); i += 1
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
    autoplay = 'trunk'
    nodes.append('[node name="AnimationPlayer" type="AnimationPlayer" parent="."]\nlibraries = {\n&"": SubResource("Library")\n}\n'
                 f'autoplay = "{autoplay}"\n')
    steps = len(textures) + len(subs) + 2
    text = f'[gd_scene load_steps={steps} format=3]\n\n' + ext + '\n'.join(subs) + '\n' + lib + '\n' + '\n'.join(nodes)
    (SCENES / f'{p["name"]}.tscn').write_text(text)

def check_extents():
    """Every painted pixel of every part must stay within the 9-px surface budget from
    (8,8)."""
    worst = 0.0
    for p in TALL:
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
                        assert e <= 9.0, (p['name'], s['part'], x, y, round(e, 2))
    print(f'worst tall-plant extent {worst:.2f} px (budget 9.0)')

def check_trunk_periods():
    for name in ('plant_spiretree_trunk', 'plant_glasscane_trunk', 'plant_vinecoil_trunk'):
        rows = parts[name]
        assert len(rows) == 16 and all(rows[y] == rows[y % 4] for y in range(16)), name

if __name__ == '__main__':
    check_trunk_periods()
    write_svgs()
    for p in TALL:
        write_scene(p)
    check_extents()
    print(f'{len(parts)} tall parts, {len(TALL)} scenes, {sum(len(f) for _, f in GROUND_TILES.values())} ground frames')
