//! Chord-filtered all-pairs neighbor lists with exact local unfolding.

use cubarium_surface::{ChartImage, MAX_LOCAL_RADIUS, SurfacePoint, Vec2, unfold_with};

use crate::ids::OrganismId;

/// One neighbor as seen from an observer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Neighbor {
    pub id: OrganismId,
    /// Neighbor position in the observer's chart.
    pub local: Vec2,
    pub distance: f64,
    pub extent: f64,
}

/// Per-organism bounded neighbor lists, rebuilt each tick.
#[derive(Clone, Debug, Default)]
pub struct NeighborLists {
    /// Indexed by slot; each list sorted by `(distance, id)` and truncated to `max_neighbors`.
    pub lists: Vec<Vec<Neighbor>>,
    /// Statistics for telemetry.
    pub pairs_considered: u64,
    pub pairs_unfolded: u64,
    pub lists_truncated: u64,
}

/// Input row for the pair pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub id: OrganismId,
    pub pos: SurfacePoint,
    pub sense_radius: f64,
    pub extent: f64,
}

/// Normative: for every unordered pair `(i, j)` with `i < j` by slot, let
/// `reach = max(sense_i, sense_j) + extent_i + extent_j`; skip if `chord_sq > reach²`;
/// else `unfold(pos_i, pos_j, reach)` once and, when `Some`, add `j` to `i`'s list if
/// `distance ≤ sense_i + extent_i + extent_j` and `i` to `j`'s list if
/// `distance ≤ sense_j + extent_i + extent_j` (the reverse `local` is
/// `pos_j.chart() + map.inverse().apply(pos_i.chart() − local)`; unfolding is symmetric).
/// Lists are then sorted by `(distance, id)` and truncated to `max_neighbors`, counting
/// truncations. `images` caches `chart_images` for the five faces (index by face).
///
/// The three statistics accumulate across calls; the world resets them when it emits a
/// telemetry sample.
pub fn build(bodies: &[Body], images: &[Vec<ChartImage>; 5], max_neighbors: usize, out: &mut NeighborLists) {
    let NeighborLists { lists, pairs_considered, pairs_unfolded, lists_truncated } = out;

    let slots = bodies.iter().map(|b| b.id.slot as usize + 1).max().unwrap_or(0);
    if lists.len() < slots {
        lists.resize_with(slots, Vec::new);
    }
    for list in lists.iter_mut() {
        list.clear();
    }

    for (k, a) in bodies.iter().enumerate() {
        for b in &bodies[k + 1..] {
            *pairs_considered += 1;
            // `unfold` never looks past MAX_LOCAL_RADIUS; a larger reach would panic.
            let reach = (a.sense_radius.max(b.sense_radius) + a.extent + b.extent).min(MAX_LOCAL_RADIUS);
            if a.pos.chord_sq(&b.pos) > reach * reach {
                continue;
            }
            *pairs_unfolded += 1;
            let Some(u) = unfold_with(&images[a.pos.face.index()], a.pos, b.pos, reach) else {
                continue;
            };
            let pad = a.extent + b.extent;
            if u.distance <= a.sense_radius + pad {
                lists[a.id.slot as usize].push(Neighbor { id: b.id, local: u.local, distance: u.distance, extent: b.extent });
            }
            if u.distance <= b.sense_radius + pad {
                // Unfolding is symmetric: carry a's offset from b back into b's chart.
                let local = b.pos.chart() + u.map.inverse().apply(a.pos.chart() - u.local);
                lists[b.id.slot as usize].push(Neighbor { id: a.id, local, distance: u.distance, extent: a.extent });
            }
        }
    }

    for list in lists.iter_mut() {
        list.sort_by(|x, y| x.distance.total_cmp(&y.distance).then(x.id.cmp(&y.id)));
        if list.len() > max_neighbors {
            *lists_truncated += 1;
            list.truncate(max_neighbors);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use cubarium_surface::{FACE_EXTENT, Face, MAX_SEAMS, chart_images, unfold};

    fn images() -> [Vec<ChartImage>; 5] {
        std::array::from_fn(|i| {
            let mut v = Vec::new();
            chart_images(Face::from_index(i as u8).expect("five faces"), MAX_SEAMS, &mut v);
            v
        })
    }

    fn id(slot: u32) -> OrganismId {
        OrganismId { slot, generation: 1 }
    }

    fn body(slot: u32, face: Face, u: f64, v: f64, sense: f64) -> Body {
        Body { id: id(slot), pos: SurfacePoint::new(face, u, v), sense_radius: sense, extent: 1.4 }
    }

    /// A deterministic stand-in for `rng::draw`, which another module owns.
    fn next(state: &mut u64) -> f64 {
        *state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        ((*state >> 11) as f64) * (1.0 / (1u64 << 53) as f64)
    }

    fn random_bodies(n: u32, seed: u64) -> Vec<Body> {
        let mut s = seed;
        (0..n)
            .map(|k| {
                let face = Face::from_index((next(&mut s) * 5.0).floor() as u8).expect("five faces");
                let u = next(&mut s) * FACE_EXTENT;
                let v = next(&mut s) * FACE_EXTENT;
                let sense = 2.0 + next(&mut s) * 10.0;
                body(k, face, u, v, sense)
            })
            .collect()
    }

    /// Every list entry, as `(observer slot, neighbor id)`, ignoring order.
    fn edges(out: &NeighborLists) -> Vec<(u32, OrganismId)> {
        let mut v: Vec<(u32, OrganismId)> = out
            .lists
            .iter()
            .enumerate()
            .flat_map(|(slot, l)| l.iter().map(move |n| (slot as u32, n.id)))
            .collect();
        v.sort_unstable();
        v
    }

    #[test]
    fn equal_radii_give_symmetric_lists() {
        let bodies = random_bodies(200, 0xC0FFEE);
        let bodies: Vec<Body> = bodies.into_iter().map(|b| Body { sense_radius: 8.0, ..b }).collect();
        let mut out = NeighborLists::default();
        build(&bodies, &images(), 64, &mut out);

        for (slot, list) in out.lists.iter().enumerate() {
            for n in list {
                let back = &out.lists[n.id.slot as usize];
                assert!(
                    back.iter().any(|m| m.id.slot == slot as u32),
                    "slot {slot} sees {:?} but not the reverse",
                    n.id
                );
                let mirror = back.iter().find(|m| m.id.slot == slot as u32).expect("reverse entry");
                assert!((mirror.distance - n.distance).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn chord_rejection_matches_brute_force_unfolding() {
        let bodies = random_bodies(200, 0x5EED_1234);
        let mut out = NeighborLists::default();
        build(&bodies, &images(), bodies.len(), &mut out);

        let mut expected: Vec<(u32, OrganismId)> = Vec::new();
        for (k, a) in bodies.iter().enumerate() {
            for b in &bodies[k + 1..] {
                let pad = a.extent + b.extent;
                let reach = (a.sense_radius.max(b.sense_radius) + pad).min(cubarium_surface::MAX_LOCAL_RADIUS);
                let Some(u) = unfold(a.pos, b.pos, reach) else { continue };
                if u.distance <= a.sense_radius + pad {
                    expected.push((a.id.slot, b.id));
                }
                if u.distance <= b.sense_radius + pad {
                    expected.push((b.id.slot, a.id));
                }
            }
        }
        expected.sort_unstable();

        assert_eq!(edges(&out), expected);
        assert!(!expected.is_empty(), "the fixture should produce some neighbors");
        assert_eq!(out.pairs_considered, (200 * 199) / 2);
        assert!(out.pairs_unfolded < out.pairs_considered, "the chord filter must reject something");
    }

    #[test]
    fn truncation_keeps_the_nearest_with_id_ties() {
        // Four neighbors exactly 3 px away in the chart, so the tie-break is by ID.
        let bodies = vec![
            body(0, Face::Front, 32.0, 32.0, 8.0),
            body(1, Face::Front, 35.0, 32.0, 8.0),
            body(2, Face::Front, 29.0, 32.0, 8.0),
            body(3, Face::Front, 32.0, 35.0, 8.0),
            body(4, Face::Front, 32.0, 29.0, 8.0),
        ];
        let mut out = NeighborLists::default();
        build(&bodies, &images(), 4, &mut out);
        let full: Vec<OrganismId> = out.lists[0].iter().map(|n| n.id).collect();
        assert_eq!(full, vec![id(1), id(2), id(3), id(4)]);
        for n in &out.lists[0] {
            assert!((n.distance - 3.0).abs() < 1e-12, "{}", n.distance);
        }

        let mut out = NeighborLists::default();
        build(&bodies, &images(), 2, &mut out);
        let kept: Vec<OrganismId> = out.lists[0].iter().map(|n| n.id).collect();
        assert_eq!(kept, vec![id(1), id(2)]);
        assert!(out.lists_truncated >= 1);
    }

    #[test]
    fn neighbors_are_found_across_a_seam() {
        // Two bodies 2 px apart across the Front/Right seam at u = 64.
        let bodies = vec![body(0, Face::Front, 63.0, 20.0, 8.0), body(1, Face::Right, 1.0, 20.0, 8.0)];
        let mut out = NeighborLists::default();
        build(&bodies, &images(), 16, &mut out);
        assert_eq!(out.lists[0].len(), 1);
        assert_eq!(out.lists[1].len(), 1);
        assert!((out.lists[0][0].distance - 2.0).abs() < 1e-9, "{}", out.lists[0][0].distance);
        // The image of the Right-face body lies just past the Front chart's right edge.
        assert!((out.lists[0][0].local.x - 65.0).abs() < 1e-9, "{:?}", out.lists[0][0].local);
        // And the reverse image lies just past the Right chart's left edge.
        assert!((out.lists[1][0].local.x + 1.0).abs() < 1e-9, "{:?}", out.lists[1][0].local);
    }

    #[test]
    fn statistics_accumulate_across_calls() {
        let bodies = random_bodies(20, 7);
        let mut out = NeighborLists::default();
        build(&bodies, &images(), 16, &mut out);
        let first = out.pairs_considered;
        build(&bodies, &images(), 16, &mut out);
        assert_eq!(out.pairs_considered, 2 * first);
    }
}
