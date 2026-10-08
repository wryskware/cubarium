//! The mesher on hand-built grids: mechanisms only (faces, culling, edges, water level,
//! change tracking), no pinned counts of a real world.

use super::*;
use crate::palette::Look;

/// A mesher over `dims` with every face and no merging, unless a test turns them on.
fn mesher(dims: [usize; 3], wrap_x: bool) -> Mesher {
    let mut m = Mesher::new(dims, Look::plain(), wrap_x);
    m.greedy = false;
    m
}

/// A chunk's feed bytes with the cells `solid` set to rock and `wet` holding free water.
fn chunk_bytes(solid: &[[usize; 3]], wet: &[([usize; 3], u8)]) -> Vec<u8> {
    let mut b = vec![0u8; 3 * CELLS];
    let l = |c: [usize; 3]| (c[2] * CHUNK + c[1]) * CHUNK + c[0];
    for &c in solid {
        b[l(c)] = 2;
    }
    for &(c, f) in wet {
        b[CELLS + l(c)] = f;
    }
    b
}

/// The quads of a mesh, as their four corners.
fn quads(m: &MeshData) -> Vec<[[f32; 3]; 4]> {
    m.index
        .chunks_exact(6)
        .map(|t| {
            // Two triangles over four distinct vertices, which are consecutive.
            let base = *t.iter().min().unwrap() as usize;
            [
                m.pos[base],
                m.pos[base + 1],
                m.pos[base + 2],
                m.pos[base + 3],
            ]
        })
        .collect()
}

fn normals(m: &MeshData) -> Vec<[f32; 3]> {
    m.index
        .chunks_exact(6)
        .map(|t| m.normal[t[0] as usize])
        .collect()
}

/// The area of an axis-aligned quad.
fn area(q: &[[f32; 3]; 4]) -> f32 {
    let e1 = [q[1][0] - q[0][0], q[1][1] - q[0][1], q[1][2] - q[0][2]];
    let e2 = [q[3][0] - q[0][0], q[3][1] - q[0][1], q[3][2] - q[0][2]];
    let c = [
        e1[1] * e2[2] - e1[2] * e2[1],
        e1[2] * e2[0] - e1[0] * e2[2],
        e1[0] * e2[1] - e1[1] * e2[0],
    ];
    (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt()
}

/// Each triangle's winding faces its normal: Godot's front faces are clockwise, so the
/// cross product of the edges points against the normal.
fn wound_to_face_their_normals(m: &MeshData) -> bool {
    m.index.chunks_exact(3).all(|t| {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| m.pos[i as usize]);
        let n = m.normal[t[0] as usize];
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let x = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        x[0] * n[0] + x[1] * n[1] + x[2] * n[2] < 0.0
    })
}

#[test]
fn a_single_voxel_gives_six_faces() {
    let mut m = mesher([16, 16, 16], false);
    m.grid.set(5, 6, 7, 2, 0, 0);
    let c = m.mesh_chunk([0, 0, 0]);
    assert_eq!(c.terrain.quads(), 6);
    assert!(c.water.is_empty());
    let mut ns = normals(&c.terrain);
    ns.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ns.dedup();
    assert_eq!(ns.len(), 6, "one face toward each direction");
    for q in quads(&c.terrain) {
        assert!((area(&q) - 1.0).abs() < 1e-6, "each a whole voxel face");
        for p in q {
            // The cell spans x 5..6, y 6..7, and z 7..8 (Godot's −8..−7).
            assert!(
                (5.0..=6.0).contains(&p[0])
                    && (6.0..=7.0).contains(&p[1])
                    && (-8.0..=-7.0).contains(&p[2])
            );
        }
    }
    assert!(wound_to_face_their_normals(&c.terrain));
}

#[test]
fn neighbours_share_no_hidden_faces() {
    for greedy in [false, true] {
        let mut m = mesher([16, 16, 16], false);
        m.greedy = greedy;
        m.grid.set(3, 3, 3, 2, 0, 0);
        m.grid.set(4, 3, 3, 2, 0, 0);
        let c = m.mesh_chunk([0, 0, 0]);
        let qs = quads(&c.terrain);
        // No face lies in the shared plane x = 4.
        assert!(
            !qs.iter().any(|q| q.iter().all(|p| p[0] == 4.0)),
            "greedy {greedy}"
        );
        let total: f32 = qs.iter().map(area).sum();
        assert!(
            (total - 10.0).abs() < 1e-5,
            "ten voxel faces of area (greedy {greedy}): {total}"
        );
        if greedy {
            assert_eq!(qs.len(), 6, "one colour: each side merges into one quad");
        } else {
            assert_eq!(qs.len(), 10);
        }
    }
}

#[test]
fn air_lies_past_the_worlds_edge() {
    // A world that is not a multiple of the chunk size: its last cells border the padding.
    let mut m = mesher([20, 20, 20], false);
    m.grid.set(19, 19, 19, 2, 0, 0);
    let c = m.mesh_chunk([1, 1, 1]);
    assert_eq!(
        c.terrain.quads(),
        6,
        "the corner cell shows every face, the far ones too"
    );
    // Past x = 0 too, when the ring does not wrap.
    m.grid.set(0, 5, 5, 2, 0, 0);
    let c = m.mesh_chunk([0, 0, 0]);
    assert!(normals(&c.terrain).contains(&[-1.0, 0.0, 0.0]));
}

#[test]
fn the_ring_wraps_at_the_seam_when_asked() {
    let mut m = mesher([32, 16, 16], true);
    m.grid.set(0, 5, 5, 2, 0, 0);
    m.grid.set(31, 5, 5, 2, 0, 0);
    let first = m.mesh_chunk([0, 0, 0]);
    assert!(
        !normals(&first.terrain).contains(&[-1.0, 0.0, 0.0]),
        "x = 0 sees x = 31 across the seam"
    );
    let last = m.mesh_chunk([1, 0, 0]);
    assert!(!normals(&last.terrain).contains(&[1.0, 0.0, 0.0]));
    // A lone cell at the seam still shows its face there.
    let mut m = mesher([32, 16, 16], true);
    m.grid.set(0, 5, 5, 2, 0, 0);
    assert!(normals(&m.mesh_chunk([0, 0, 0]).terrain).contains(&[-1.0, 0.0, 0.0]));
}

#[test]
fn a_water_cell_has_its_surface_at_the_fill_level() {
    let mut m = mesher([16, 16, 16], false);
    m.grid.set(4, 0, 4, 2, 0, 0);
    let fill = 128u8;
    m.grid.set(4, 1, 4, 0, fill, 0);
    let c = m.mesh_chunk([0, 0, 0]);
    let level = 1.0 + f32::from(fill) / 255.0;
    let qs = quads(&c.water);
    let ns = normals(&c.water);
    let tops: Vec<_> = qs
        .iter()
        .zip(&ns)
        .filter(|(_, n)| **n == [0.0, 1.0, 0.0])
        .collect();
    assert_eq!(tops.len(), 1, "one surface");
    assert!(
        tops[0].0.iter().all(|p| (p[1] - level).abs() < 1e-6),
        "at the fill level"
    );
    // Its sides rise from the floor to the same level, and nothing sits under it.
    for (q, n) in qs.iter().zip(&ns) {
        if n[1] == 0.0 {
            let (lo, hi) = q
                .iter()
                .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[1]), b.max(p[1])));
            assert!((lo - 1.0).abs() < 1e-6 && (hi - level).abs() < 1e-6);
        }
        assert_ne!(*n, [0.0, -1.0, 0.0], "the rock below hides the bottom");
    }
    assert!(wound_to_face_their_normals(&c.water));
    // Water standing above fills the cell below whole: no surface between them.
    m.grid.set(4, 1, 4, 0, 255, 0);
    m.grid.set(4, 2, 4, 0, 64, 0);
    let c = m.mesh_chunk([0, 0, 0]);
    let tops: Vec<f32> = quads(&c.water)
        .iter()
        .zip(normals(&c.water))
        .filter(|(_, n)| *n == [0.0, 1.0, 0.0])
        .map(|(q, _)| q[0][1])
        .collect();
    assert_eq!(tops.len(), 1);
    assert!((tops[0] - (2.0 + 64.0 / 255.0)).abs() < 1e-6);
}

#[test]
fn only_a_changed_chunk_is_remeshed_and_reported() {
    let mut m = Mesher::new([32, 16, 16], Look::plain(), false);
    m.faces = VIEW_FACES;
    let floor: Vec<[usize; 3]> = (0..16)
        .flat_map(|x| (0..16).map(move |z| [x, 0, z]))
        .collect();
    let a = chunk_bytes(&floor, &[]);
    let b = chunk_bytes(&floor, &[]);
    m.write_chunk([0, 0, 0], &a);
    m.write_chunk([1, 0, 0], &b);
    let first = m.flush(2);
    assert_eq!(first.len(), 2, "both reported the first time");
    // The same bytes again: nothing to report.
    m.write_chunk([0, 0, 0], &a);
    m.write_chunk([1, 0, 0], &b);
    assert!(m.flush(2).is_empty());
    // A change in chunk 0's interior: only chunk 0, and its mesh is different.
    let mut solid = floor.clone();
    solid.push([7, 1, 7]);
    let a2 = chunk_bytes(&solid, &[([3, 1, 3], 200)]);
    m.write_chunk([0, 0, 0], &a2);
    m.write_chunk([1, 0, 0], &b);
    let changed = m.flush(2);
    assert_eq!(
        changed.iter().map(|c| c.at).collect::<Vec<_>>(),
        vec![[0, 0, 0]]
    );
    let before = first.iter().find(|c| c.at == [0, 0, 0]).unwrap();
    assert_ne!(changed[0].terrain, before.terrain);
    assert!(!changed[0].water.is_empty());
}

#[test]
fn a_change_on_a_shared_face_remeshes_the_neighbour_only_if_its_mesh_moves() {
    let mut m = Mesher::new([32, 16, 16], Look::plain(), false);
    m.greedy = false;
    m.write_chunk([0, 0, 0], &chunk_bytes(&[], &[]));
    m.write_chunk([1, 0, 0], &chunk_bytes(&[[0, 4, 4]], &[]));
    m.flush(1);
    // A cell at chunk 0's x = 15 hides chunk 1's face at x = 16: both are reported.
    m.write_chunk([0, 0, 0], &chunk_bytes(&[[15, 4, 4]], &[]));
    let changed = m.flush(1);
    let mut at: Vec<_> = changed.iter().map(|c| c.at).collect();
    at.sort();
    assert_eq!(at, vec![[0, 0, 0], [1, 0, 0]]);
    let next = changed.iter().find(|c| c.at == [1, 0, 0]).unwrap();
    assert!(
        !normals(&next.terrain).contains(&[-1.0, 0.0, 0.0]),
        "the shared face is gone"
    );
    // A change on the shared face that does not touch the neighbour's mesh: chunk 0 only.
    m.write_chunk([0, 0, 0], &chunk_bytes(&[[15, 4, 4], [15, 9, 9]], &[]));
    let changed = m.flush(1);
    assert_eq!(
        changed.iter().map(|c| c.at).collect::<Vec<_>>(),
        vec![[0, 0, 0]]
    );
}

#[test]
fn pore_water_darkens_soil_in_levels() {
    let mut m = mesher([16, 16, 16], false);
    m.grid.set(2, 2, 2, 3, 0, 0);
    let dry = m.mesh_chunk([0, 0, 0]).terrain.color[0];
    m.grid.set(2, 2, 2, 3, 0, 255);
    let wet = m.mesh_chunk([0, 0, 0]).terrain.color[0];
    assert_ne!(dry, wet);
    // A pore change within one level does not change the picture.
    let mut m = Mesher::new([16, 16, 16], Look::plain(), false);
    let mut bytes = chunk_bytes(&[], &[]);
    bytes[(2 * CHUNK + 2) * CHUNK + 2] = 3;
    m.write_chunk([0, 0, 0], &bytes);
    m.flush(1);
    bytes[2 * CELLS + (2 * CHUNK + 2) * CHUNK + 2] = 1;
    assert!(!m.write_chunk([0, 0, 0], &bytes).any);
    assert!(m.flush(1).is_empty());
}

#[test]
fn the_view_meshes_only_tops_and_fronts() {
    let mut m = mesher([16, 16, 16], false);
    m.faces = VIEW_FACES;
    m.grid.set(5, 5, 5, 2, 0, 0);
    let c = m.mesh_chunk([0, 0, 0]);
    let mut ns = normals(&c.terrain);
    ns.sort_by(|a, b| a.partial_cmp(b).unwrap());
    // Up, and toward the viewer (Godot's +z).
    assert_eq!(ns, vec![[0.0, 0.0, 1.0], [0.0, 1.0, 0.0]]);
}
