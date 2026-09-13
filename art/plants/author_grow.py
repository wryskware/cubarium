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
def splice(scene: str, clips: list[dict], nodes: str, ext: list[tuple[str, str]] = ()) -> str:
    text = (SCENES / f'{scene}.tscn').read_text()
    for c in clips:
        assert f'Animation_{c["name"]}' not in text, f'{scene}: {c["name"]} already present'

    # 0. new external textures (the canopy steps use sub-parts cut from the stage art),
    #    appended after the scene's last ext_resource with fresh ids.
    for tex_id, part in ext:
        assert f'id="{tex_id}"' not in text, f'{scene}: texture id {tex_id} already used'
        last = text.rindex('[ext_resource')
        end = text.index('\n', last) + 1
        line = f'[ext_resource type="Texture2D" path="res://parts/{part}.svg" id="{tex_id}"]\n\n'
        text = text[:end] + '\n' + line.rstrip('\n') + '\n' + text[end:]

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
                  lambda m: f'load_steps={int(m.group(1)) + len(clips) + len(ext)}', text, count=1)
    (SCENES / f'{scene}.tscn').write_text(text)
    print(f'{scene}: added {", ".join(c["name"] for c in clips)}')


def node(name, parent, kind='Sprite2D', pos=None, tex=None, flip=False,
         hidden=False, scale=None, rot=None, flip_v=False):
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
    if flip_v:
        out += 'flip_v = true\n'
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


# =============================================================== canopy sub-parts
# The two top-down species open from their centre, so their steps need the stage art in
# pieces that can extend and slide separately: ribs apart from blades, petals apart from the
# crown. Every piece below is cut *pixel for pixel* out of the existing part it names — the
# same colours at the same offsets, nothing repainted — and the pieces of one stage are a
# partition of it (checked here), so a step's last frame is the stage art exactly.
PARTS = SCENES.parent / 'parts'
MINT, CYAN = '#7BEBC9', '#42C5F8'


def read_part(name: str) -> tuple[int, int, dict]:
    text = (PARTS / f'{name}.svg').read_text()
    w = int(re.search(r'width="(\d+)"', text).group(1))
    h = int(re.search(r'height="(\d+)"', text).group(1))
    px: dict[tuple[int, int], str] = {}
    for fill, d in re.findall(r'<path fill="(#[0-9A-Fa-f]{6})" d="([^"]+)"/>', text):
        for x, y in re.findall(r'M(\d+) (\d+)h1v1h-1z', d):
            px[(int(x), int(y))] = fill
    return w, h, px


def write_part(name: str, w: int, h: int, px: dict) -> None:
    """The emitter of art/plants/author.py: one path per colour, pixels in scan order."""
    by: dict[str, list[str]] = {}
    for y in range(h):
        for x in range(w):
            if (x, y) in px:
                by.setdefault(px[(x, y)], []).append(f'M{x} {y}h1v1h-1z')
    paths = ''.join(f'\n  <path fill="{c}" d="{"".join(d)}"/>' for c, d in by.items())
    (PARTS / f'{name}.svg').write_text(
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" '
        f'shape-rendering="crispEdges">{paths}\n</svg>\n')


def cut(px: dict, x0: int, y0: int, x1: int, y1: int, keep=lambda p, c: True) -> dict:
    """The pixels of `px` inside the inclusive box, re-based to its corner."""
    return {(x - x0, y - y0): c for (x, y), c in px.items()
            if x0 <= x <= x1 and y0 <= y <= y1 and keep((x, y), c)}


def placed(piece: dict, x0: int, y0: int, w: int, h: int, flip_h=False, flip_v=False) -> dict:
    """Where a piece's pixels land in the parent's frame, drawn at box corner (x0, y0)."""
    out = {}
    for (x, y), c in piece.items():
        fx = w - 1 - x if flip_h else x
        fy = h - 1 - y if flip_v else y
        out[(x0 + fx, y0 + fy)] = c
    return out


def canopy_parts() -> None:
    # umbrellafrond: frond1 = ribs1 (the mint ribs and their cyan tips, a 9×9 cross) +
    # blades1 (everything else, dark edge and teal); frond2 = cross2 (cardinal ribs, 13×13)
    # + diag2 (the four diagonal ribs, 9×9) + blades2 (the rest, 15×15).
    _, _, f1 = read_part('plant_umbrellafrond_frond1')
    rib = lambda p, c: c in (MINT, CYAN)
    ribs1 = cut(f1, 1, 1, 9, 9, rib)
    blades1 = cut(f1, 1, 1, 9, 9, lambda p, c: not rib(p, c))
    assert placed(ribs1, 1, 1, 9, 9) | placed(blades1, 1, 1, 9, 9) == f1, 'frond1 partition'
    assert not (set(ribs1) & set(blades1))
    write_part('plant_umbrellafrond_ribs1', 9, 9, ribs1)
    write_part('plant_umbrellafrond_blades1', 9, 9, blades1)

    _, _, f2 = read_part('plant_umbrellafrond_frond2')
    on_axis = lambda p, c: rib(p, c) and (p[0] == 7 or p[1] == 7)
    off_axis = lambda p, c: rib(p, c) and not (p[0] == 7 or p[1] == 7)
    cross2 = cut(f2, 1, 1, 13, 13, on_axis)
    diag2 = cut(f2, 3, 3, 11, 11, off_axis)
    blades2 = cut(f2, 0, 0, 14, 14, lambda p, c: not rib(p, c))
    assert placed(cross2, 1, 1, 13, 13) | placed(diag2, 3, 3, 9, 9) | blades2 == f2, 'frond2 partition'
    assert len(cross2) + len(diag2) + len(blades2) == len(f2)
    write_part('plant_umbrellafrond_cross2', 13, 13, cross2)
    write_part('plant_umbrellafrond_diag2', 9, 9, diag2)
    write_part('plant_umbrellafrond_blades2', 15, 15, blades2)

    # bloomcrown: petals1 = one 3×3 petal drawn four times (flipped into each quadrant) plus
    # the dark cross the stage's own centre covers; petals2 = one 4×3 side lobe drawn four
    # times, one 3×4 tip drawn twice (north, and flipped south), the one-row waist between
    # the side lobes, plus the interior the stage's centre2 covers.
    _, _, p1 = read_part('plant_bloomcrown_petals1')
    petal1 = cut(p1, 2, 2, 4, 4)
    _, _, c1 = read_part('plant_bloomcrown_center1')
    four = {}
    for x0, y0, fh, fv in [(2, 2, False, False), (6, 2, True, False), (2, 6, False, True), (6, 6, True, True)]:
        four |= placed(petal1, x0, y0, 3, 3, fh, fv)
    covered1 = {p: c for p, c in p1.items() if p not in four}
    assert set(covered1) <= {(x + 4, y + 4) for (x, y) in c1}, 'petals1: uncovered remainder'
    assert four | covered1 == p1 and not (set(four) & set(covered1)), 'petals1 partition'
    write_part('plant_bloomcrown_petal1', 3, 3, petal1)

    _, _, p2 = read_part('plant_bloomcrown_petals2')
    lobe2 = cut(p2, 2, 4, 5, 6)
    tip2 = cut(p2, 6, 1, 8, 4)
    waist2 = cut(p2, 4, 7, 5, 7)
    core2 = cut(p2, 6, 5, 8, 9)
    _, _, c2 = read_part('plant_bloomcrown_center2')
    pieces = {}
    for x0, y0, fh, fv in [(2, 4, False, False), (9, 4, True, False), (2, 8, False, True), (9, 8, True, True)]:
        pieces |= placed(lobe2, x0, y0, 4, 3, fh, fv)
    pieces |= placed(tip2, 6, 1, 3, 4) | placed(tip2, 6, 10, 3, 4, flip_v=True)
    pieces |= placed(waist2, 4, 7, 2, 1) | placed(waist2, 9, 7, 2, 1, flip_h=True)
    pieces |= placed(core2, 6, 5, 3, 5)
    assert len(pieces) == 4 * len(lobe2) + 2 * len(tip2) + 2 * len(waist2) + len(core2), 'petals2 pieces overlap'
    assert pieces == p2, 'petals2 partition'
    # The core's rows under the crown are what centre2 covers in the stage; it is painted
    # anyway so the body is whole while the crown is still the small one.
    assert set(cut(p2, 6, 6, 8, 8)) <= {(x - 1, y - 1) for (x, y) in c2}, 'centre2 covers the core'
    write_part('plant_bloomcrown_lobe2', 4, 3, lobe2)
    write_part('plant_bloomcrown_tip2', 3, 4, tip2)
    write_part('plant_bloomcrown_waist2', 2, 1, waist2)
    write_part('plant_bloomcrown_core2', 3, 5, core2)
    print('canopy sub-parts written: umbrellafrond ribs1/blades1/cross2/diag2/blades2, '
          'bloomcrown petal1/lobe2/tip2/waist2/core2')


# =============================================================== umbrellafrond
def umbrellafrond():
    """Top-down, centred: the ribs extend out of the sprout first, the blades unfurl out
    from the ribs (two copies squeezed along one rib axis each, spreading sideways), and the
    cyan centre lights last. No part is ever scaled as a whole."""
    clips, nodes, resets = [], '', []
    ext = [('5', 'plant_umbrellafrond_ribs1'), ('6', 'plant_umbrellafrond_blades1'),
           ('7', 'plant_umbrellafrond_cross2'), ('8', 'plant_umbrellafrond_diag2'),
           ('9', 'plant_umbrellafrond_blades2')]
    # ---- grow01: sprout -> ribs1 + blades1 + center
    G = 'Grow01'
    t1 = [
        vis(f'{G}:visible', [(0, True)]),
        vis(f'{G}/Sprout:visible', [(0, True), (3.85, False)]),
        # At scale 0.5 the 9×9 cross samples exactly the sprout's mint diamond, so it can
        # come in under it invisibly and then extend to its full 9 px, tips last.
        vis(f'{G}/Ribs:visible', [(0, False), (0.5, True)]),
        alpha(f'{G}/Ribs', [(0.5, 0), (0.9, 1)]),
        vec2(f'{G}/Ribs:scale', [(0, (0.5, 0.5)), (0.6, (0.5, 0.5)), (2.2, (1, 1)), (4.0, (1, 1))]),
        vis(f'{G}/BladesA:visible', [(0, False), (1.2, True)]),
        alpha(f'{G}/BladesA', [(1.2, 0), (1.8, 1)]),
        vec2(f'{G}/BladesA:scale', [(0, (0.35, 1)), (1.4, (0.35, 1)), (3.4, (1, 1)), (4.0, (1, 1))]),
        vis(f'{G}/BladesB:visible', [(0, False), (1.4, True)]),
        alpha(f'{G}/BladesB', [(1.4, 0), (2.0, 1)]),
        vec2(f'{G}/BladesB:scale', [(0, (1, 0.35)), (1.6, (1, 0.35)), (3.6, (1, 1)), (4.0, (1, 1))]),
        vis(f'{G}/Center:visible', [(0, False), (3.0, True)]),
        alpha(f'{G}/Center', [(3.0, 0), (3.8, 1)]),
    ]
    resets += [
        (f'{G}:visible', 1, 'false'),
        (f'{G}/Sprout:visible', 1, 'false'),
        (f'{G}/Ribs:visible', 1, 'false'),
        (f'{G}/Ribs:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/Ribs:scale', 0, 'Vector2(1, 1)'),
        (f'{G}/BladesA:visible', 1, 'false'),
        (f'{G}/BladesA:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/BladesA:scale', 0, 'Vector2(1, 1)'),
        (f'{G}/BladesB:visible', 1, 'false'),
        (f'{G}/BladesB:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{G}/BladesB:scale', 0, 'Vector2(1, 1)'),
        (f'{G}/Center:visible', 1, 'false'),
        (f'{G}/Center:self_modulate', 0, 'Color(1, 1, 1, 1)'),
    ]
    nodes += (node(G, '.', 'Node2D', pos=(0.5, 0.5), hidden=True)
              + node('Sprout', G, tex=1, pos=(0, 0), hidden=True)
              + node('BladesA', G, tex=6, pos=(0, 0), hidden=True)
              + node('BladesB', G, tex=6, pos=(0, 0), hidden=True)
              + node('Ribs', G, tex=5, pos=(0, 0), hidden=True)
              + node('Center', G, tex=3, pos=(0, 0), hidden=True))
    clips.append(dict(name='grow01', length=L, tracks=t1, reset=[]))

    # ---- grow12: frond1 + center -> cross2 + diag2 + blades2 + center
    H = 'Grow12'
    # blades2 copies: the along-rib extent follows the cross ribs (11/15 -> 1 over
    # 0.8-2.4 s), then each copy spreads sideways out from its rib (0.45 -> 1).
    a = [(0, (0.45, 11 / 15)), (0.8, (0.45, 11 / 15)), (1.8, (0.45, 0.9)),
         (2.4, (0.45 + 0.55 * 0.6 / 1.8, 1)), (3.6, (1, 1)), (4.0, (1, 1))]
    b = [(0, (11 / 15, 0.45)), (0.8, (11 / 15, 0.45)), (1.8, (0.9, 0.45)), (2.0, (0.925, 0.45)),
         (2.4, (1, 0.45 + 0.55 * 0.4 / 1.8)), (3.8, (1, 1)), (4.0, (1, 1))]
    t2 = [
        vis(f'{H}:visible', [(0, True)]),
        vis(f'{H}/Old:visible', [(0, True), (3.95, False)]),
        vis(f'{H}/Center:visible', [(0, True)]),
        # The cardinal ribs come in over frond1's at the length they already have (2/3 of
        # 13 px is 9) and push out to 13.
        vis(f'{H}/Cross:visible', [(0, False), (0.4, True)]),
        alpha(f'{H}/Cross', [(0.4, 0), (1.0, 1)]),
        vec2(f'{H}/Cross:scale', [(0, (2 / 3, 2 / 3)), (0.8, (2 / 3, 2 / 3)), (2.4, (1, 1)), (4.0, (1, 1))]),
        # Four new diagonal ribs sprout from the centre.
        vis(f'{H}/Diag:visible', [(0, False), (1.4, True)]),
        alpha(f'{H}/Diag', [(1.4, 0), (1.9, 1)]),
        vec2(f'{H}/Diag:scale', [(0, (0.36, 0.36)), (1.4, (0.36, 0.36)), (3.0, (1, 1)), (4.0, (1, 1))]),
        vis(f'{H}/BladesA:visible', [(0, False), (1.5, True)]),
        alpha(f'{H}/BladesA', [(1.5, 0), (2.1, 1)]),
        vec2(f'{H}/BladesA:scale', a),
        vis(f'{H}/BladesB:visible', [(0, False), (1.7, True)]),
        alpha(f'{H}/BladesB', [(1.7, 0), (2.3, 1)]),
        vec2(f'{H}/BladesB:scale', b),
    ]
    resets += [
        (f'{H}:visible', 1, 'false'),
        (f'{H}/Old:visible', 1, 'false'),
        (f'{H}/Center:visible', 1, 'false'),
        (f'{H}/Cross:visible', 1, 'false'),
        (f'{H}/Cross:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/Cross:scale', 0, 'Vector2(1, 1)'),
        (f'{H}/Diag:visible', 1, 'false'),
        (f'{H}/Diag:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/Diag:scale', 0, 'Vector2(1, 1)'),
        (f'{H}/BladesA:visible', 1, 'false'),
        (f'{H}/BladesA:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/BladesA:scale', 0, 'Vector2(1, 1)'),
        (f'{H}/BladesB:visible', 1, 'false'),
        (f'{H}/BladesB:self_modulate', 0, 'Color(1, 1, 1, 1)'),
        (f'{H}/BladesB:scale', 0, 'Vector2(1, 1)'),
    ]
    nodes += (node(H, '.', 'Node2D', pos=(0.5, 0.5), hidden=True)
              + node('Old', H, tex=2, pos=(0, 0), hidden=True)
              + node('BladesA', H, tex=9, pos=(0, 0), hidden=True)
              + node('BladesB', H, tex=9, pos=(0, 0), hidden=True)
              + node('Cross', H, tex=7, pos=(0, 0), hidden=True)
              + node('Diag', H, tex=8, pos=(0, 0), hidden=True)
              + node('Center', H, tex=3, pos=(0, 0), hidden=True))
    clips.append(dict(name='grow12', length=L, tracks=t2, reset=resets))
    splice('umbrellafrond', clips, nodes, ext)


# =============================================================== bloomcrown
def bloomcrown():
    """Top-down, centred: the warm centre swells out of the sprout as a bud whose four
    petals show only their dark corners round it, then the petals slide out diagonally in
    opposite pairs; a stage later the four petals become the
    wider side lobes and slide outward, two new petals push out north and south from under
    the crown, and the 5 px crown opens over the 3 px one last. No fruit part is ever used."""
    clips, nodes, resets = [], '', []
    ext = [('7', 'plant_bloomcrown_petal1'), ('8', 'plant_bloomcrown_lobe2'),
           ('9', 'plant_bloomcrown_tip2'), ('10', 'plant_bloomcrown_waist2'),
           ('11', 'plant_bloomcrown_core2')]
    # ---- grow01: sprout -> four petals + center1
    G = 'Grow01'
    def slide(name, t0, t1, start, end):
        return vec2(f'{G}/{name}:position', [(0, start), (t0, start), (t1, end), (4.0, end)])
    t1 = [
        vis(f'{G}:visible', [(0, True)]),
        vis(f'{G}/Sprout:visible', [(0, True), (1.05, False)]),
        # center1's alpha plane is the sprout's, so the bud simply warms over it.
        vis(f'{G}/Center:visible', [(0, False), (0.4, True)]),
        alpha(f'{G}/Center', [(0.4, 0), (1.0, 1)]),
        # Petals start tucked under the centre and slide out diagonally, NW/SE leading.
        vis(f'{G}/PetalNW:visible', [(0, False), (0.6, True)]),
        alpha(f'{G}/PetalNW', [(0.6, 0), (1.0, 1)]),
        slide('PetalNW', 1.0, 2.6, (0, 0), (-2, -2)),
        vis(f'{G}/PetalSE:visible', [(0, False), (0.6, True)]),
        alpha(f'{G}/PetalSE', [(0.6, 0), (1.0, 1)]),
        slide('PetalSE', 1.0, 2.6, (0, 0), (2, 2)),
        vis(f'{G}/PetalNE:visible', [(0, False), (0.9, True)]),
        alpha(f'{G}/PetalNE', [(0.9, 0), (1.3, 1)]),
        slide('PetalNE', 1.3, 2.9, (0, 0), (2, -2)),
        vis(f'{G}/PetalSW:visible', [(0, False), (0.9, True)]),
        alpha(f'{G}/PetalSW', [(0.9, 0), (1.3, 1)]),
        slide('PetalSW', 1.3, 2.9, (0, 0), (-2, 2)),
    ]
    resets += [(f'{G}:visible', 1, 'false'), (f'{G}/Sprout:visible', 1, 'false'),
               (f'{G}/Center:visible', 1, 'false'), (f'{G}/Center:self_modulate', 0, 'Color(1, 1, 1, 1)')]
    for name, (x, y) in [('PetalNW', (-2, -2)), ('PetalSE', (2, 2)), ('PetalNE', (2, -2)), ('PetalSW', (-2, 2))]:
        resets += [(f'{G}/{name}:visible', 1, 'false'),
                   (f'{G}/{name}:self_modulate', 0, 'Color(1, 1, 1, 1)'),
                   (f'{G}/{name}:position', 0, f'Vector2({fmt(x)}, {fmt(y)})')]
    nodes += (node(G, '.', 'Node2D', pos=(0.5, 0.5), hidden=True)
              + node('Sprout', G, tex=1, pos=(0, 0), hidden=True)
              + node('PetalNW', G, tex=7, pos=(-2, -2), hidden=True)
              + node('PetalNE', G, tex=7, pos=(2, -2), hidden=True, flip=True)
              + node('PetalSW', G, tex=7, pos=(-2, 2), hidden=True, flip_v=True)
              + node('PetalSE', G, tex=7, pos=(2, 2), hidden=True, flip=True, flip_v=True)
              + node('Center', G, tex=3, pos=(0, 0), hidden=True))
    clips.append(dict(name='grow01', length=L, tracks=t1, reset=[]))

    # ---- grow12: petals1 + center1 -> lobes + tips + waist + center2
    H = 'Grow12'
    def slide2(name, t0, t1, start, end):
        return vec2(f'{H}/{name}:position', [(0, start), (t0, start), (t1, end), (4.0, end)])
    t2 = [
        vis(f'{H}:visible', [(0, True)]),
        # The old bloom holds until every lobe is opaque over its petal (1.8 s).
        vis(f'{H}/Old:visible', [(0, True), (1.85, False)]),
        # The small crown stays on top throughout; the body's core forms under it first,
        # and the 5 px crown opens over the small one as the last thing (a cross-fade: a
        # nearest-sampled scale-up would lose the ring and read as a solid warm fruit).
        vis(f'{H}/Center1:visible', [(0, True), (3.45, False)]),
        vis(f'{H}/Core:visible', [(0, False), (0.9, True)]),
        alpha(f'{H}/Core', [(0.9, 0), (1.5, 1)]),
        vis(f'{H}/Center:visible', [(0, False), (2.6, True)]),
        alpha(f'{H}/Center', [(2.6, 0), (3.4, 1)]),
        # Side lobes form over the old petals, then slide one pixel outward, NW/SE leading.
        vis(f'{H}/LobeNW:visible', [(0, False), (0.9, True)]),
        alpha(f'{H}/LobeNW', [(0.9, 0), (1.5, 1)]),
        slide2('LobeNW', 2.0, 3.2, (-2.5, -2), (-3.5, -2)),
        vis(f'{H}/LobeSE:visible', [(0, False), (0.9, True)]),
        alpha(f'{H}/LobeSE', [(0.9, 0), (1.5, 1)]),
        slide2('LobeSE', 2.0, 3.2, (2.5, 2), (3.5, 2)),
        vis(f'{H}/LobeNE:visible', [(0, False), (1.2, True)]),
        alpha(f'{H}/LobeNE', [(1.2, 0), (1.8, 1)]),
        slide2('LobeNE', 2.3, 3.5, (2.5, -2), (3.5, -2)),
        vis(f'{H}/LobeSW:visible', [(0, False), (1.2, True)]),
        alpha(f'{H}/LobeSW', [(1.2, 0), (1.8, 1)]),
        slide2('LobeSW', 2.3, 3.5, (-2.5, 2), (-3.5, 2)),
        # Two new petals push out from under the crown, north first.
        vis(f'{H}/TipN:visible', [(0, False), (1.3, True)]),
        alpha(f'{H}/TipN', [(1.3, 0), (1.7, 1)]),
        slide2('TipN', 1.6, 3.2, (0, -1.5), (0, -4.5)),
        vis(f'{H}/TipS:visible', [(0, False), (1.6, True)]),
        alpha(f'{H}/TipS', [(1.6, 0), (2.0, 1)]),
        slide2('TipS', 1.9, 3.5, (0, 1.5), (0, 4.5)),
        vis(f'{H}/WaistW:visible', [(0, False), (2.0, True)]),
        alpha(f'{H}/WaistW', [(2.0, 0), (2.6, 1)]),
        vis(f'{H}/WaistE:visible', [(0, False), (2.0, True)]),
        alpha(f'{H}/WaistE', [(2.0, 0), (2.6, 1)]),
    ]
    resets += [(f'{H}:visible', 1, 'false'), (f'{H}/Old:visible', 1, 'false'),
               (f'{H}/Center1:visible', 1, 'false'),
               (f'{H}/Core:visible', 1, 'false'), (f'{H}/Core:self_modulate', 0, 'Color(1, 1, 1, 1)'),
               (f'{H}/Center:visible', 1, 'false'), (f'{H}/Center:self_modulate', 0, 'Color(1, 1, 1, 1)'),
               (f'{H}/WaistW:visible', 1, 'false'), (f'{H}/WaistW:self_modulate', 0, 'Color(1, 1, 1, 1)'),
               (f'{H}/WaistE:visible', 1, 'false'), (f'{H}/WaistE:self_modulate', 0, 'Color(1, 1, 1, 1)')]
    for name, (x, y) in [('LobeNW', (-3.5, -2)), ('LobeSE', (3.5, 2)), ('LobeNE', (3.5, -2)),
                         ('LobeSW', (-3.5, 2)), ('TipN', (0, -4.5)), ('TipS', (0, 4.5))]:
        resets += [(f'{H}/{name}:visible', 1, 'false'),
                   (f'{H}/{name}:self_modulate', 0, 'Color(1, 1, 1, 1)'),
                   (f'{H}/{name}:position', 0, f'Vector2({fmt(x)}, {fmt(y)})')]
    nodes += (node(H, '.', 'Node2D', pos=(0.5, 0.5), hidden=True)
              + node('Old', H, 'Node2D', pos=(0, 0), hidden=True)
              + node('Petals', f'{H}/Old', tex=2, pos=(0, 0))
              + node('Core', H, tex=11, pos=(0, 0), hidden=True)
              + node('LobeNW', H, tex=8, pos=(-3.5, -2), hidden=True)
              + node('LobeNE', H, tex=8, pos=(3.5, -2), hidden=True, flip=True)
              + node('LobeSW', H, tex=8, pos=(-3.5, 2), hidden=True, flip_v=True)
              + node('LobeSE', H, tex=8, pos=(3.5, 2), hidden=True, flip=True, flip_v=True)
              + node('TipN', H, tex=9, pos=(0, -4.5), hidden=True)
              + node('TipS', H, tex=9, pos=(0, 4.5), hidden=True, flip_v=True)
              + node('WaistW', H, tex=10, pos=(-2.5, 0), hidden=True)
              + node('WaistE', H, tex=10, pos=(2.5, 0), hidden=True, flip=True)
              + node('Center1', H, tex=3, pos=(0, 0), hidden=True)
              + node('Center', H, tex=5, pos=(0, 0), hidden=True))
    clips.append(dict(name='grow12', length=L, tracks=t2, reset=resets))
    splice('bloomcrown', clips, nodes, ext)


if __name__ == '__main__':
    which = sys.argv[1:] or ['lanternstalk', 'glowcap', 'rootveil', 'tendrilfan', 'reedspire']
    for w in which:
        globals()[w]()
