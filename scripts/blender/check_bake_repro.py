"""Headless mechanism checks: blender -b -P scripts/blender/check_bake_repro.py."""
import os
import sys
from types import SimpleNamespace

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bake_voxel_models as B
from mathutils import Vector

# A cube gives exactly tied nearest faces at its centre. Material IDs make
# index remapping observable, independently of the input face enumeration.
vertices = [Vector(v) for v in ((-1, -1, -1), (1, -1, -1), (1, 1, -1), (-1, 1, -1),
                               (-1, -1, 1), (1, -1, 1), (1, 1, 1), (-1, 1, 1))]
loops = [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4),
         (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)]
points = [Vector(v) for v in ((0, 0, 0), (0, 0, -1), (0, 0, -1.5),
                             (1, 1, 1), (0.2, 0.3, 0.4))]
expected = None
for order in (list(range(6)), list(reversed(range(6))), [3, 1, 5, 0, 4, 2]):
    for rotation in range(4):
        mesh = SimpleNamespace(polygons=[
            SimpleNamespace(vertices=loops[n][rotation:] + loops[n][:rotation],
                            material_index=n, index=i)
            for i, n in enumerate(order)])
        bvh = B.BakeBVH(vertices, mesh)
        hits = []
        for point in points:
            loc, normal, index, distance = bvh.find_nearest(point)
            hits.append((tuple(loc), tuple(normal), mesh.polygons[index].material_index, distance))
        if expected is None:
            expected = hits
        assert hits == expected, (order, rotation, hits, expected)
# Equal distances choose the canonical loop (0, 1, 5, 4), material 2.
assert expected[0][2:] == (2, 1.0), expected[0]
# Existing occupancy rules include exactly half a voxel and keep dot==0 outside.
assert expected[2][3] == 0.5
assert (points[1] - Vector(expected[1][0])).dot(Vector(expected[1][1])) == 0
assert B.BakeBVH(vertices, SimpleNamespace(polygons=[])).find_nearest(points[0]) == (None,) * 4
print('PASS: 12 face-order/loop permutations, tied normals/materials, boundary distances, empty mesh')
