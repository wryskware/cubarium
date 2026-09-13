#!/usr/bin/env python3
"""Splice hand-authored grow<from><to> clips into the Cubarium plant scenes.

The .tscn text stays the source of truth; this script is how Worker B wrote it, the
way art/plants/author.py is how the scenes were first painted. Re-running it on an
already-spliced scene is refused (it checks the clip is absent).
"""
from __future__ import annotations
import re, sys
from pathlib import Path

SCENES = Path('/home/wrysk/wryskware/cubarium/art/plants')

W = 1.0  # opaque alpha shorthand


def fmt(v: float) -> str:
    s = f'{v:.6f}'.rstrip('0').rstrip('.')
    return s if s not in ('', '-0') else '0'


# ---------------------------------------------------------------- track builders
def vis(path, keys):
    return (path, 1, [(t, 'true' if v else 'false') for t, v in keys])


def alpha(path, keys, prop='self_modulate'):
    return (f'{path}:{prop}' if ':' not in path else path, 0,
            [(t, f'Color(1, 1, 1, {fmt(a)})') for t, a in keys])


def vec2(path, keys):
    return (path, 0, [(t, f'Vector2({fmt(x)}, {fmt(y)})') for t, (x, y) in keys])


def flt(path, keys):
    return (path, 0, [(t, fmt(v)) for t, v in keys])


def track_text(i, path, update, keys):
    times = ', '.join(fmt(t) for t, _ in keys)
    return (f'tracks/{i}/type = "value"\n'
            f'tracks/{i}/path = NodePath("{path}")\n'
            f'tracks/{i}/interp = 1\n'
            f'tracks/{i}/enabled = true\n'
            f'tracks/{i}/keys = {{\n'
            f'"times": PackedFloat32Array({times}),\n'
            f'"transitions": PackedFloat32Array({", ".join("1" for _ in keys)}),\n'
            f'"update": {update},\n'
            f'"values": [{", ".join(v for _, v in keys)}]\n'
            f'}}\n')


# ---------------------------------------------------------------- scene surgery
def splice(scene: str, clips: list[dict], nodes: str) -> str:
    text = (SCENES / f'{scene}.tscn').read_text()
    for c in clips:
        assert f'Animation_{c["name"]}' not in text, f'{scene}: {c["name"]} already present'

    # 1. RESET: append the neutral value of every newly animated property.
    reset_start = text.index('[sub_resource type="Animation" id="Animation_RESET"]')
    reset_end = text.index('[sub_resource', reset_start + 10)
    reset = text[reset_start:reset_end]
    next_i = max(int(m) for m in re.findall(r'tracks/(\d+)/type', reset)) + 1
    seen = set(re.findall(r'tracks/\d+/path = NodePath\("([^"]+)"\)', reset))
    added = ''
    for c in clips:
        for path, update, neutral in c['reset']:
            if path in seen:
                continue
            seen.add(path)
            added += '\n' + track_text(next_i, path, update, [(0, neutral)])
            next_i += 1
    text = text[:reset_end] + text[reset_end:]
    text = text[:reset_start] + reset.rstrip('\n') + '\n' + added + '\n\n' + text[reset_end:]

    # 2. the clips themselves, before the AnimationLibrary.
    lib_at = text.index('[sub_resource type="AnimationLibrary"')
    blocks = ''
    for c in clips:
        body = (f'[sub_resource type="Animation" id="Animation_{c["name"]}"]\n'
                f'resource_name = "{c["name"]}"\nlength = {fmt(c["length"])}\nloop_mode = 0\n\n')
        body += '\n'.join(track_text(i, p, u, k) for i, (p, u, k) in enumerate(c['tracks']))
        blocks += body + '\n'
    text = text[:lib_at] + blocks + text[lib_at:]

    # 3. register them in the library.
    def add_lib(m):
        inner = m.group(1).rstrip()
        extra = ''.join(f',\n&"{c["name"]}": SubResource("Animation_{c["name"]}")' for c in clips)
        return '_data = {\n' + inner + extra + '\n}'
    text, n = re.subn(r'_data = \{\n(.*?)\n\}', add_lib, text, count=1, flags=re.S)
    assert n == 1, scene

    # 4. the node subtree, before the AnimationPlayer.
    ap = text.index('[node name="AnimationPlayer"')
    text = text[:ap] + nodes.strip('\n') + '\n\n' + text[ap:]

    # 5. load_steps counts external resources + sub-resources + the root node + 1.
    text = re.sub(r'load_steps=(\d+)',
                  lambda m: f'load_steps={int(m.group(1)) + len(clips)}', text, count=1)
    (SCENES / f'{scene}.tscn').write_text(text)
    print(f'{scene}: added {", ".join(c["name"] for c in clips)}')


def node(name, parent, kind='Sprite2D', pos=None, tex=None, flip=False,
         hidden=False, scale=None, rot=None):
    out = f'[node name="{name}" type="{kind}" parent="{parent}"]\n'
    if hidden:
        out += 'visible = false\n'
    if pos is not None:
        out += f'position = Vector2({fmt(pos[0])}, {fmt(pos[1])})\n'
    if scale is not None:
        out += f'scale = Vector2({fmt(scale[0])}, {fmt(scale[1])})\n'
    if rot is not None:
        out += f'rotation = {fmt(rot)}\n'
    if tex is not None:
        out += f'texture = ExtResource("{tex}")\n'
    if flip:
        out += 'flip_h = true\n'
    return out + '\n'


L = 4.0


def ramp(t0, v0, t1, v1, hold=L):
    """A linear ramp with the endpoints held flat, so the sampled endpoints are exact."""
    keys = [(0, v0)] if t0 > 0 else []
    keys += [(t0, v0), (t1, v1)]
    if hold > t1:
        keys.append((hold, v1))
    return keys


# =============================================================== lanternstalk
def lanternstalk():
    G = 'Grow12'
    # Stem: stalk2 (5x9) bottom-anchored on rig row 15 -> position.y = -4.5 * scale.y.
    s = [(0, 0.625), (1.0, 0.625), (2.6, 1.0), (4.0, 1.0)]
    # Bulb: bulb2 (7x7) riding the stem top, `d` below it. It sits exactly over bulb1's
    # centre when it starts to fade in (1.6 s) and settles onto the mature offset by
    # 3.6 s; its overlap with the stem's top row is never less than 2.1 px.
    d = {0: 0.390625, 1.0: 0.390625, 1.6: 0.390625, 2.6: -0.0546875, 3.6: -0.5, 4.0: -0.5}
    sv = {0: 0.625, 1.0: 0.625, 1.6: 0.765625, 2.6: 1.0, 3.6: 1.0, 4.0: 1.0}
    bulb_pos = [(t, (0, -9.0 * sv[t] + d[t])) for t in sorted(d)]
    tracks = [
        vis(f'{G}:visible', [(0, True)]),
        vis(f'{G}/Old:visible', [(0, True), (1.35, False)]),
        alpha(f'{G}/Old', [(0.8, 1), (1.3, 0)]),
        vis(f'{G}/OldBulb:visible', [(0, True), (2.85, False)]),
        alpha(f'{G}/OldBulb', [(2.0, 1), (2.8, 0)]),
        vis(f'{G}/Stem:visible', [(0, False), (0.4, True)]),
        alpha(f'{G}/Stem', [(0.4, 0), (0.9, 1)]),
        vec2(f'{G}/Stem:scale', [(t, (1, v)) for t, v in s]),
        vec2(f'{G}/Stem:position', [(t, (0, -4.5 * v)) for t, v in s]),
        vis(f'{G}/Bulb:visible', [(0, False), (1.6, True)]),
        alpha(f'{G}/Bulb', [(1.6, 0), (2.4, 1)]),
        vec2(f'{G}/Bulb:scale', [(0, (0.5, 0.5)), (1.6, (0.5, 0.5)),
                                 (3.6, (1, 1)), (4.0, (1, 1))]),
        vec2(f'{G}/Bulb:position', bulb_pos),
    ]
    reset = [
        (f'{G}:visible', 1, 'false'),
        (f'{G}/Old:visible', 1, 'false'),
        (f'{G}/Old:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/OldBulb:visible', 1, 'false'),
        (f'{G}/OldBulb:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/Stem:visible', 1, 'false'),
        (f'{G}/Stem:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/Stem:scale', 0, 'Vector2(1, 1)'),
        (f'{G}/Stem:position', 0, 'Vector2(0, -4.5)'),
        (f'{G}/Bulb:visible', 1, 'false'),
        (f'{G}/Bulb:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/Bulb:scale', 0, 'Vector2(1, 1)'),
        (f'{G}/Bulb:position', 0, 'Vector2(0, -9.5)'),
    ]
    nodes = (node(G, '.', 'Node2D', pos=(0.5, 7), hidden=True)
             + node('Old', G, tex=2, pos=(0, -2.5), hidden=True)
             + node('Stem', G, tex=4, pos=(0, -4.5), hidden=True)
             + node('OldBulb', G, tex=3, pos=(0, -6.5), hidden=True)
             + node('Bulb', G, tex=5, pos=(0, -9.5), hidden=True))
    splice('lanternstalk', [dict(name='grow12', length=L, tracks=tracks, reset=reset)], nodes)


# =============================================================== glowcap
def glowcap():
    clips, nodes, resets = [], '', []
    # ---- grow01: sprout -> stem1 + Cap1
    G = 'Grow01'
    s = [(0, 0.5), (0.8, 0.5), (2.4, 1.0), (4.0, 1.0)]          # stem1 (3x4)
    off = {0: 0.0, 0.8: 0.0, 2.4: 0.0, 2.8: 0.0, 3.6: -1.0, 4.0: -1.0}
    sv = {0: 0.5, 0.8: 0.5, 2.4: 1.0, 2.8: 1.0, 3.6: 1.0, 4.0: 1.0}
    cap_pos = [(t, (0, -4.0 * sv[t] + off[t])) for t in sorted(off)]
    t1 = [
        vis(f'{G}:visible', [(0, True)]),
        vis(f'{G}/Sprout:visible', [(0, True), (1.75, False)]),
        alpha(f'{G}/Sprout', [(1.1, 1), (1.7, 0)]),
        vis(f'{G}/Stem:visible', [(0, False), (0.2, True)]),
        alpha(f'{G}/Stem', [(0.2, 0), (0.7, 1)]),
        vec2(f'{G}/Stem:scale', [(t, (1, v)) for t, v in s]),
        vec2(f'{G}/Stem:position', [(t, (0, -2.0 * v)) for t, v in s]),
        vis(f'{G}/Cap:visible', [(0, False), (1.2, True)]),
        alpha(f'{G}/Cap', [(1.2, 0), (1.8, 1)], 'modulate'),
        vec2(f'{G}/Cap:scale', [(0, (0.43, 1)), (1.2, (0.43, 1)), (3.4, (1, 1)), (4.0, (1, 1))]),
        vec2(f'{G}/Cap:position', cap_pos),
        vis(f'{G}/Cap/Gills:visible', [(0, False), (2.8, True)]),
        alpha(f'{G}/Cap/Gills', [(2.8, 0), (3.9, 1)]),
    ]
    resets += [
        (f'{G}:visible', 1, 'false'),
        (f'{G}/Sprout:visible', 1, 'false'),
        (f'{G}/Sprout:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/Stem:visible', 1, 'false'),
        (f'{G}/Stem:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/Stem:scale', 0, 'Vector2(1, 1)'),
        (f'{G}/Stem:position', 0, 'Vector2(0, -2)'),
        (f'{G}/Cap:visible', 1, 'false'),
        (f'{G}/Cap:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/Cap:scale', 0, 'Vector2(1, 1)'),
        (f'{G}/Cap:position', 0, 'Vector2(0, -5)'),
        (f'{G}/Cap/Gills:visible', 1, 'false'),
        (f'{G}/Cap/Gills:self_modulate', 0, 'Color(1, 1, 1, 1)'),
    ]
    nodes += (node(G, '.', 'Node2D', pos=(0.5, 7), hidden=True)
              + node('Sprout', G, tex=1, pos=(0, -1.5), hidden=True)
              + node('Stem', G, tex=2, pos=(0, -2), hidden=True)
              + node('Cap', G, 'Node2D', pos=(0, -5), hidden=True)
              + node('Cap', f'{G}/Cap', tex=3, pos=(0, -1))
              + node('Gills', f'{G}/Cap', tex=4, pos=(0, 1), hidden=True))
    clips.append(dict(name='grow01', length=L, tracks=t1, reset=[]))

    # ---- grow12: stem1/Cap1 -> stem2 + Cap2 + Cap2b
    H = 'Grow12'
    s2 = [(0, 0.67), (0.8, 0.67), (2.6, 1.0), (4.0, 1.0)]        # stem2 (3x6)
    # Cap2 rides the stem top, one pixel below it during the cross-fade so its gills land
    # on the rows Cap1's gills held, then settling onto the top as the stem finishes.
    sv2 = {0: 0.67, 0.8: 0.67, 1.1: 0.725, 2.6: 1.0, 4.0: 1.0}
    off2 = {0: 1.0, 0.8: 1.0, 1.1: 1.0, 2.6: 0.0, 4.0: 0.0}
    cap2_pos = [(t, (0, -6.0 * sv2[t] + off2[t])) for t in sorted(sv2)]
    t2 = [
        vis(f'{H}:visible', [(0, True)]),
        vis(f'{H}/Old:visible', [(0, True), (1.25, False)]),
        alpha(f'{H}/Old', [(0.5, 1), (1.2, 0)]),
        vis(f'{H}/OldCap:visible', [(0, True), (1.45, False)]),
        alpha(f'{H}/OldCap', [(0.6, 1), (1.4, 0)], 'modulate'),
        vis(f'{H}/Stem:visible', [(0, False), (0.2, True)]),
        alpha(f'{H}/Stem', [(0.2, 0), (0.9, 1)]),
        vec2(f'{H}/Stem:scale', [(t, (1, v)) for t, v in s2]),
        vec2(f'{H}/Stem:position', [(t, (0, -3.0 * v)) for t, v in s2]),
        vis(f'{H}/Cap:visible', [(0, False), (0.3, True)]),
        alpha(f'{H}/Cap', [(0.3, 0), (1.1, 1)], 'modulate'),
        vec2(f'{H}/Cap:scale', [(0, (0.78, 1)), (1.0, (0.78, 1)), (3.4, (1, 1)), (4.0, (1, 1))]),
        vec2(f'{H}/Cap:position', cap2_pos),
        vis(f'{H}/Shelf:visible', [(0, False), (2.0, True)]),
        alpha(f'{H}/Shelf', [(2.0, 0), (2.8, 1)], 'modulate'),
        vec2(f'{H}/Shelf:scale', [(0, (0.5, 0.5)), (2.0, (0.5, 0.5)),
                                  (3.9, (1, 1)), (4.0, (1, 1))]),
    ]
    resets += [
        (f'{H}:visible', 1, 'false'),
        (f'{H}/Old:visible', 1, 'false'),
        (f'{H}/Old:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/OldCap:visible', 1, 'false'),
        (f'{H}/OldCap:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/Stem:visible', 1, 'false'),
        (f'{H}/Stem:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/Stem:scale', 0, 'Vector2(1, 1)'),
        (f'{H}/Stem:position', 0, 'Vector2(0, -3)'),
        (f'{H}/Cap:visible', 1, 'false'),
        (f'{H}/Cap:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/Cap:scale', 0, 'Vector2(1, 1)'),
        (f'{H}/Cap:position', 0, 'Vector2(0, -6)'),
        (f'{H}/Shelf:visible', 1, 'false'),
        (f'{H}/Shelf:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/Shelf:scale', 0, 'Vector2(1, 1)'),
    ]
    nodes += (node(H, '.', 'Node2D', pos=(0.5, 7), hidden=True)
              + node('Old', H, tex=2, pos=(0, -2), hidden=True)
              + node('OldCap', H, 'Node2D', pos=(0, -5), hidden=True)
              + node('Cap', f'{H}/OldCap', tex=3, pos=(0, -1))
              + node('Gills', f'{H}/OldCap', tex=4, pos=(0, 1))
              + node('Stem', H, tex=5, pos=(0, -3), hidden=True)
              + node('Cap', H, 'Node2D', pos=(0, -6), hidden=True)
              + node('Cap', f'{H}/Cap', tex=6, pos=(0, -3.5))
              + node('Gills', f'{H}/Cap', tex=7, pos=(0, -1))
              + node('Shelf', H, 'Node2D', pos=(2, -4), hidden=True)
              + node('Cap', f'{H}/Shelf', tex=8, pos=(1, -1))
              + node('Gills', f'{H}/Shelf', tex=4, pos=(1, 1)))
    clips.append(dict(name='grow12', length=L, tracks=t2, reset=resets))
    splice('glowcap', clips, nodes)


# =============================================================== rootveil
def rootveil():
    clips, nodes, resets = [], '', []
    G = 'Grow01'
    sx = [(0, 0.43), (0.6, 0.43), (3.3, 1.0), (4.0, 1.0)]
    sy = [(0, 1.0), (0.6, 1.0), (3.3, 1.0), (4.0, 1.0)]
    t1 = [
        vis(f'{G}:visible', [(0, True)]),
        vis(f'{G}/Sprout:visible', [(0, True), (1.25, False)]),
        alpha(f'{G}/Sprout', [(0.6, 1), (1.2, 0)]),
        vis(f'{G}/Crust:visible', [(0, False), (0.2, True)]),
        alpha(f'{G}/Crust', [(0.2, 0), (0.9, 1)], 'modulate'),
        vec2(f'{G}/Crust:scale', [(t, (x, y)) for (t, x), (_, y) in zip(sx, sy)]),
        vis(f'{G}/Crust/Glints:visible', [(0, False), (2.6, True)]),
        alpha(f'{G}/Crust/Glints', [(2.6, 0), (3.9, 1)]),
    ]
    resets += [
        (f'{G}:visible', 1, 'false'),
        (f'{G}/Sprout:visible', 1, 'false'),
        (f'{G}/Sprout:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/Crust:visible', 1, 'false'),
        (f'{G}/Crust:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/Crust:scale', 0, 'Vector2(1, 1)'),
        (f'{G}/Crust/Glints:visible', 1, 'false'),
        (f'{G}/Crust/Glints:self_modulate', 0, 'Color(1, 1, 1, 1)'),
    ]
    nodes += (node(G, '.', 'Node2D', pos=(0.5, 7), hidden=True)
              + node('Sprout', G, tex=1, pos=(0, -1), hidden=True)
              + node('Crust', G, 'Node2D', pos=(0, 0), hidden=True)
              + node('Veil', f'{G}/Crust', tex=2, pos=(0, -1.5))
              + node('Glints', f'{G}/Crust', tex=3, pos=(0, -1.5), hidden=True))
    clips.append(dict(name='grow01', length=L, tracks=t1, reset=[]))

    H = 'Grow12'
    sc = [(0, (0.64, 0.6)), (1.0, (0.64, 0.6)), (3.4, (1, 1)), (4.0, (1, 1))]
    t2 = [
        vis(f'{H}:visible', [(0, True)]),
        vis(f'{H}/Old:visible', [(0, True), (1.55, False)]),
        alpha(f'{H}/Old', [(0.7, 1), (1.5, 0)], 'modulate'),
        vis(f'{H}/Crust:visible', [(0, False), (0.4, True)]),
        alpha(f'{H}/Crust', [(0.4, 0), (1.2, 1)], 'modulate'),
        vec2(f'{H}/Crust:scale', sc),
        vis(f'{H}/Crust/Glints:visible', [(0, False), (2.8, True)]),
        alpha(f'{H}/Crust/Glints', [(2.8, 0), (3.9, 1)]),
    ]
    resets += [
        (f'{H}:visible', 1, 'false'),
        (f'{H}/Old:visible', 1, 'false'),
        (f'{H}/Old:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/Crust:visible', 1, 'false'),
        (f'{H}/Crust:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/Crust:scale', 0, 'Vector2(1, 1)'),
        (f'{H}/Crust/Glints:visible', 1, 'false'),
        (f'{H}/Crust/Glints:self_modulate', 0, 'Color(1, 1, 1, 1)'),
    ]
    nodes += (node(H, '.', 'Node2D', pos=(0.5, 7), hidden=True)
              + node('Old', H, 'Node2D', pos=(0, 0), hidden=True)
              + node('Veil', f'{H}/Old', tex=2, pos=(0, -1.5))
              + node('Glints', f'{H}/Old', tex=3, pos=(0, -1.5))
              + node('Crust', H, 'Node2D', pos=(0, 0), hidden=True)
              + node('Veil', f'{H}/Crust', tex=4, pos=(0, -2.5))
              + node('Glints', f'{H}/Crust', tex=5, pos=(0, -2.5), hidden=True))
    clips.append(dict(name='grow12', length=L, tracks=t2, reset=resets))
    splice('rootveil', clips, nodes)


# =============================================================== tendrilfan
def tendrilfan():
    clips, nodes, resets = [], '', []
    G = 'Grow01'
    kl = [(0, 0.3), (1.0, 0.3), (3.5, 1.0), (4.0, 1.0)]
    rl = [(0, 0.6), (1.0, 0.6), (3.5, 0.0), (4.0, 0.0)]
    kr = [(0, 0.3), (1.3, 0.3), (3.9, 1.0), (4.0, 1.0)]
    rr = [(0, -0.6), (1.3, -0.6), (3.9, 0.0), (4.0, 0.0)]
    t1 = [
        vis(f'{G}:visible', [(0, True)]),
        vis(f'{G}/Sprout:visible', [(0, True), (1.25, False)]),
        alpha(f'{G}/Sprout', [(0.6, 1), (1.2, 0)]),
        vis(f'{G}/Base:visible', [(0, False), (0.3, True)]),
        alpha(f'{G}/Base', [(0.3, 0), (0.9, 1)]),
        vis(f'{G}/TendrilL:visible', [(0, False), (0.6, True)]),
        alpha(f'{G}/TendrilL', [(0.6, 0), (1.2, 1)], 'modulate'),
        vec2(f'{G}/TendrilL:scale', [(t, (k, k)) for t, k in kl]),
        flt(f'{G}/TendrilL:rotation', rl),
        vis(f'{G}/TendrilR:visible', [(0, False), (0.9, True)]),
        alpha(f'{G}/TendrilR', [(0.9, 0), (1.5, 1)], 'modulate'),
        vec2(f'{G}/TendrilR:scale', [(t, (k, k)) for t, k in kr]),
        flt(f'{G}/TendrilR:rotation', rr),
    ]
    resets += [
        (f'{G}:visible', 1, 'false'),
        (f'{G}/Sprout:visible', 1, 'false'),
        (f'{G}/Sprout:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/Base:visible', 1, 'false'),
        (f'{G}/Base:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/TendrilL:visible', 1, 'false'),
        (f'{G}/TendrilL:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/TendrilL:scale', 0, 'Vector2(1, 1)'),
        (f'{G}/TendrilL:rotation', 0, '0.0'),
        (f'{G}/TendrilR:visible', 1, 'false'),
        (f'{G}/TendrilR:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/TendrilR:scale', 0, 'Vector2(1, 1)'),
        (f'{G}/TendrilR:rotation', 0, '0.0'),
    ]
    nodes += (node(G, '.', 'Node2D', pos=(0, 0), hidden=True)
              + node('Sprout', G, tex=1, pos=(0.5, 5.5), hidden=True)
              + node('Base', G, tex=2, pos=(0.5, 6), hidden=True)
              + node('TendrilL', G, 'Node2D', pos=(0, 6), hidden=True)
              + node('Sprite', f'{G}/TendrilL', tex=3, pos=(-1.5, -3.5))
              + node('TendrilR', G, 'Node2D', pos=(1, 6), hidden=True)
              + node('Sprite', f'{G}/TendrilR', tex=3, pos=(1.5, -3.5), flip=True))
    clips.append(dict(name='grow01', length=L, tracks=t1, reset=[]))

    H = 'Grow12'
    k2 = [(0, 0.6), (0.8, 0.6), (3.4, 1.0), (4.0, 1.0)]
    t2 = [
        vis(f'{H}:visible', [(0, True)]),
        vis(f'{H}/OldBase:visible', [(0, True), (1.45, False)]),
        alpha(f'{H}/OldBase', [(0.6, 1), (1.4, 0)]),
        vis(f'{H}/OldL:visible', [(0, True), (1.55, False)]),
        alpha(f'{H}/OldL', [(0.7, 1), (1.5, 0)], 'modulate'),
        vis(f'{H}/OldR:visible', [(0, True), (1.55, False)]),
        alpha(f'{H}/OldR', [(0.7, 1), (1.5, 0)], 'modulate'),
        vis(f'{H}/Base:visible', [(0, False), (0.3, True)]),
        alpha(f'{H}/Base', [(0.3, 0), (1.1, 1)], 'modulate'),
        vec2(f'{H}/Base:scale', [(0, (0.7, 1)), (0.6, (0.7, 1)), (2.6, (1, 1)), (4.0, (1, 1))]),
        vis(f'{H}/TendrilL:visible', [(0, False), (0.4, True)]),
        alpha(f'{H}/TendrilL', [(0.4, 0), (1.2, 1)], 'modulate'),
        vec2(f'{H}/TendrilL:scale', [(t, (k, k)) for t, k in k2]),
        flt(f'{H}/TendrilL:rotation', [(0, 0.4), (0.8, 0.4), (3.4, 0.0), (4.0, 0.0)]),
        vis(f'{H}/TendrilR:visible', [(0, False), (0.4, True)]),
        alpha(f'{H}/TendrilR', [(0.4, 0), (1.2, 1)], 'modulate'),
        vec2(f'{H}/TendrilR:scale', [(t, (k, k)) for t, k in k2]),
        flt(f'{H}/TendrilR:rotation', [(0, -0.4), (0.8, -0.4), (3.4, 0.0), (4.0, 0.0)]),
        vis(f'{H}/Center:visible', [(0, False), (1.4, True)]),
        alpha(f'{H}/Center', [(1.4, 0), (2.0, 1)], 'modulate'),
        vec2(f'{H}/Center:scale', [(0, (1, 0.3)), (1.4, (1, 0.3)), (3.9, (1, 1)), (4.0, (1, 1))]),
    ]
    resets += [
        (f'{H}:visible', 1, 'false'),
        (f'{H}/OldBase:visible', 1, 'false'),
        (f'{H}/OldBase:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/OldL:visible', 1, 'false'),
        (f'{H}/OldL:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/OldR:visible', 1, 'false'),
        (f'{H}/OldR:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/Base:visible', 1, 'false'),
        (f'{H}/Base:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/Base:scale', 0, 'Vector2(1, 1)'),
        (f'{H}/TendrilL:visible', 1, 'false'),
        (f'{H}/TendrilL:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/TendrilL:scale', 0, 'Vector2(1, 1)'),
        (f'{H}/TendrilL:rotation', 0, '0.0'),
        (f'{H}/TendrilR:visible', 1, 'false'),
        (f'{H}/TendrilR:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/TendrilR:scale', 0, 'Vector2(1, 1)'),
        (f'{H}/TendrilR:rotation', 0, '0.0'),
        (f'{H}/Center:visible', 1, 'false'),
        (f'{H}/Center:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/Center:scale', 0, 'Vector2(1, 1)'),
    ]
    nodes += (node(H, '.', 'Node2D', pos=(0, 0), hidden=True)
              + node('OldBase', H, tex=2, pos=(0.5, 6), hidden=True)
              + node('OldL', H, 'Node2D', pos=(0, 6), hidden=True)
              + node('Sprite', f'{H}/OldL', tex=3, pos=(-1.5, -3.5))
              + node('OldR', H, 'Node2D', pos=(1, 6), hidden=True)
              + node('Sprite', f'{H}/OldR', tex=3, pos=(1.5, -3.5), flip=True)
              + node('Base', H, 'Node2D', pos=(0.5, 7), hidden=True)
              + node('Sprite', f'{H}/Base', tex=4, pos=(0, -1.5))
              + node('Center', H, 'Node2D', pos=(0.5, 5), hidden=True)
              + node('Sprite', f'{H}/Center', tex=5, pos=(0, -5.5))
              + node('TendrilL', H, 'Node2D', pos=(0, 5), hidden=True)
              + node('Sprite', f'{H}/TendrilL', tex=7, pos=(-2.5, -4))
              + node('TendrilR', H, 'Node2D', pos=(1, 5), hidden=True)
              + node('Sprite', f'{H}/TendrilR', tex=7, pos=(2.5, -4), flip=True))
    clips.append(dict(name='grow12', length=L, tracks=t2, reset=resets))
    splice('tendrilfan', clips, nodes)


# =============================================================== reedspire
def reedspire():
    clips, nodes, resets = [], '', []
    G = 'Grow01'
    t1 = [
        vis(f'{G}:visible', [(0, True)]),
        vis(f'{G}/Sprout:visible', [(0, True), (1.25, False)]),
        alpha(f'{G}/Sprout', [(0.6, 1), (1.2, 0)]),
        vis(f'{G}/Ripple:visible', [(0, False), (0.4, True)]),
        alpha(f'{G}/Ripple', [(0.4, 0), (1.2, 1)]),
        vis(f'{G}/ReedA:visible', [(0, False), (0.2, True)]),
        alpha(f'{G}/ReedA', [(0.2, 0), (0.9, 1)], 'modulate'),
        vec2(f'{G}/ReedA:scale', [(0, (1, 3 / 7)), (0.6, (1, 3 / 7)), (2.8, (1, 1)), (4.0, (1, 1))]),
        vis(f'{G}/ReedB:visible', [(0, False), (1.0, True)]),
        alpha(f'{G}/ReedB', [(1.0, 0), (1.6, 1)], 'modulate'),
        vec2(f'{G}/ReedB:scale', [(0, (1, 0.6)), (1.2, (1, 0.6)), (3.9, (1, 1)), (4.0, (1, 1))]),
    ]
    resets += [
        (f'{G}:visible', 1, 'false'),
        (f'{G}/Sprout:visible', 1, 'false'),
        (f'{G}/Sprout:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/Ripple:visible', 1, 'false'),
        (f'{G}/Ripple:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/ReedA:visible', 1, 'false'),
        (f'{G}/ReedA:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/ReedA:scale', 0, 'Vector2(1, 1)'),
        (f'{G}/ReedB:visible', 1, 'false'),
        (f'{G}/ReedB:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/ReedB:scale', 0, 'Vector2(1, 1)'),
    ]
    nodes += (node(G, '.', 'Node2D', pos=(0, 0), hidden=True)
              + node('Sprout', G, tex=1, pos=(0, 5.5), hidden=True)
              + node('Ripple', G, tex=2, pos=(0, 6.5), hidden=True)
              + node('ReedA', G, 'Node2D', pos=(-1, 7), hidden=True)
              + node('Sprite', f'{G}/ReedA', tex=3, pos=(0, -3.5))
              + node('ReedB', G, 'Node2D', pos=(2, 7), hidden=True)
              + node('Sprite', f'{G}/ReedB', tex=4, pos=(0, -2.5)))
    clips.append(dict(name='grow01', length=L, tracks=t1, reset=[]))

    H = 'Grow12'
    t2 = [
        vis(f'{H}:visible', [(0, True)]),
        vis(f'{H}/OldRipple:visible', [(0, True), (1.45, False)]),
        alpha(f'{H}/OldRipple', [(0.6, 1), (1.4, 0)]),
        vis(f'{H}/Ripple:visible', [(0, False), (0.3, True)]),
        alpha(f'{H}/Ripple', [(0.3, 0), (1.1, 1)]),
        vis(f'{H}/OldA:visible', [(0, True), (1.55, False)]),
        alpha(f'{H}/OldA', [(0.7, 1), (1.5, 0)], 'modulate'),
        vis(f'{H}/OldB:visible', [(0, True), (1.55, False)]),
        alpha(f'{H}/OldB', [(0.7, 1), (1.5, 0)], 'modulate'),
        vis(f'{H}/ReedA:visible', [(0, False), (0.3, True)]),
        alpha(f'{H}/ReedA', [(0.3, 0), (1.1, 1)], 'modulate'),
        vec2(f'{H}/ReedA:scale', [(0, (1, 7 / 11)), (1.0, (1, 7 / 11)),
                                  (3.2, (1, 1)), (4.0, (1, 1))]),
        vis(f'{H}/ReedB:visible', [(0, False), (0.3, True)]),
        alpha(f'{H}/ReedB', [(0.3, 0), (1.1, 1)], 'modulate'),
        vec2(f'{H}/ReedB:scale', [(0, (1, 0.625)), (1.0, (1, 0.625)),
                                  (3.2, (1, 1)), (4.0, (1, 1))]),
        vis(f'{H}/ReedC:visible', [(0, False), (1.8, True)]),
        alpha(f'{H}/ReedC', [(1.8, 0), (2.4, 1)], 'modulate'),
        vec2(f'{H}/ReedC:scale', [(0, (1, 0.2)), (1.8, (1, 0.2)), (3.9, (1, 1)), (4.0, (1, 1))]),
    ]
    resets += [
        (f'{H}:visible', 1, 'false'),
        (f'{H}/OldRipple:visible', 1, 'false'),
        (f'{H}/OldRipple:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/Ripple:visible', 1, 'false'),
        (f'{H}/Ripple:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/OldA:visible', 1, 'false'),
        (f'{H}/OldA:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/OldB:visible', 1, 'false'),
        (f'{H}/OldB:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/ReedA:visible', 1, 'false'),
        (f'{H}/ReedA:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/ReedA:scale', 0, 'Vector2(1, 1)'),
        (f'{H}/ReedB:visible', 1, 'false'),
        (f'{H}/ReedB:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/ReedB:scale', 0, 'Vector2(1, 1)'),
        (f'{H}/ReedC:visible', 1, 'false'),
        (f'{H}/ReedC:modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/ReedC:scale', 0, 'Vector2(1, 1)'),
    ]
    nodes += (node(H, '.', 'Node2D', pos=(0, 0), hidden=True)
              + node('OldRipple', H, tex=2, pos=(0, 6.5), hidden=True)
              + node('Ripple', H, tex=5, pos=(0, 6.5), hidden=True)
              + node('OldA', H, 'Node2D', pos=(-1, 7), hidden=True)
              + node('Sprite', f'{H}/OldA', tex=3, pos=(0, -3.5))
              + node('OldB', H, 'Node2D', pos=(2, 7), hidden=True)
              + node('Sprite', f'{H}/OldB', tex=4, pos=(0, -2.5))
              + node('ReedA', H, 'Node2D', pos=(-2, 7), hidden=True)
              + node('Sprite', f'{H}/ReedA', tex=6, pos=(0, -5.5))
              + node('ReedB', H, 'Node2D', pos=(1, 7), hidden=True)
              + node('Sprite', f'{H}/ReedB', tex=7, pos=(0, -4))
              + node('ReedC', H, 'Node2D', pos=(3, 7), hidden=True)
              + node('Sprite', f'{H}/ReedC', tex=8, pos=(0, -3)))
    clips.append(dict(name='grow12', length=L, tracks=t2, reset=resets))
    splice('reedspire', clips, nodes)


if __name__ == '__main__':
    which = sys.argv[1:] or ['lanternstalk', 'glowcap', 'rootveil', 'tendrilfan', 'reedspire']
    for w in which:
        globals()[w]()
