//! Adversarial public-contract checks for chart-owned patch stamping.

use cubarium_render::{
    Bend, Canvas, Mask, Pose, Sprite, stamp_layers_bent, stamp_pose_in_chart,
};
use cubarium_surface::{Face, SurfacePoint, Vec2, unfold, unfold_pixels};

fn sprite(rgb: [u8; 3]) -> Sprite {
    let mut rgba = vec![0; 9 * 9 * 4];
    for px in rgba.as_chunks_mut::<4>().0 {
        px.copy_from_slice(&[rgb[0], rgb[1], rgb[2], 224]);
    }
    Sprite::from_rgba(9, 9, Vec2::new(4.5, 4.5), &rgba).unwrap()
}

fn assert_same(actual: &Canvas, expected: &Canvas, label: &str) {
    for face in Face::ALL {
        for y in 0..64 {
            for x in 0..64 {
                assert_eq!(
                    actual.get(face, x, y),
                    expected.get(face, x, y),
                    "{label}: {face:?} {x},{y}",
                );
            }
        }
    }
}

#[test]
fn retained_chart_candidates_stay_inside_the_recentered_physical_disk() {
    let coords = [0.5, 2.5, 8.5, 16.5, 32.5, 47.5, 55.5, 63.5];
    for face in Face::ALL {
        for &ou in &coords {
            for &ov in &coords {
                let owner = SurfacePoint::new(face, ou, ov);
                for &cu in &coords {
                    for &cv in &coords {
                        let center = SurfacePoint::new(face, cu, cv);
                        let query = 9.0 + (owner.chart() - center.chart()).length();
                        if query > 32.0 {
                            continue;
                        }
                        let mut pixels = Vec::new();
                        unfold_pixels(owner, query, &mut pixels);
                        for p in pixels {
                            if (p.local - center.chart()).length() <= 9.0
                                && unfold(
                                    center,
                                    SurfacePoint::pixel_center(p.face, p.x, p.y),
                                    9.0,
                                )
                                .is_none()
                            {
                                panic!(
                                    "retained candidate outside physical radius: owner={owner:?} center={center:?} destination={:?} {},{} local={:?}",
                                    p.face, p.x, p.y, p.local
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // The public API admits continuous same-face offsets. Search irregular offsets
    // around every connected vertex as well as the lattice above.
    let mut state = 0x85f0_1da4_c321_9e67u64;
    for face in Face::ALL {
        for &(corner_u, corner_v) in &[(0.0, 0.0), (64.0, 0.0), (0.0, 64.0), (64.0, 64.0)] {
            for _ in 1..=1_000 {
                let next = |state: &mut u64| {
                    *state ^= *state << 13;
                    *state ^= *state >> 7;
                    *state ^= *state << 17;
                    (*state as f64) / (u64::MAX as f64)
                };
                let point = |state: &mut u64| {
                    let u = (corner_u + if corner_u == 0.0 { 1.0 } else { -1.0 } * next(state) * 31.0)
                        .clamp(0.0, 63.999_999);
                    let v = (corner_v + if corner_v == 0.0 { 1.0 } else { -1.0 } * next(state) * 31.0)
                        .clamp(0.0, 63.999_999);
                    SurfacePoint::new(face, u, v)
                };
                let owner = point(&mut state);
                let center = point(&mut state);
                let query = 9.0 + (owner.chart() - center.chart()).length();
                if query > 32.0 {
                    continue;
                }
                let mut pixels = Vec::new();
                unfold_pixels(owner, query, &mut pixels);
                for p in pixels {
                    if (p.local - center.chart()).length() <= 9.0
                        && unfold(
                            center,
                            SurfacePoint::pixel_center(p.face, p.x, p.y),
                            9.0,
                        )
                        .is_none()
                    {
                        panic!(
                            "retained candidate outside physical radius: owner={owner:?} center={center:?} destination={:?} {},{} local={:?}",
                            p.face, p.x, p.y, p.local
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn recentered_vertex_patches_paint_only_their_physical_nine_pixel_disk() {
    let first = sprite([255, 32, 0]);
    let second = sprite([0, 128, 255]);
    let corners = [
        ((0.5, 0.5), (8.5, 8.5)),
        ((63.5, 0.5), (55.5, 8.5)),
        ((0.5, 63.5), (8.5, 55.5)),
        ((63.5, 63.5), (55.5, 55.5)),
    ];
    let mut painted = 0;
    for face in Face::ALL {
        for &((ou, ov), (cu, cv)) in &corners {
            let owner = SurfacePoint::new(face, ou, ov);
            let center = SurfacePoint::new(face, cu, cv);
            let mut canvas = Canvas::new();
            stamp_pose_in_chart(
                &mut canvas,
                owner,
                center,
                Vec2::new(1.0, 0.0),
                Pose { first: &first, second: &second, mix: 0.37 },
                0.7,
                Mask::Axial { reveal: 8.5 },
                Bend { amplitude: 1.0, base: 32.0, root: 1.0, length: 40.0 },
                &mut Vec::new(),
            );
            for destination in Face::ALL {
                for y in 0..64u8 {
                    for x in 0..64u8 {
                        if canvas.get(destination, x, y).iter().any(|channel| *channel > 0.0) {
                            assert!(
                                unfold(center, SurfacePoint::pixel_center(destination, x, y), 9.0)
                                    .is_some(),
                                "painted outside physical nine-pixel disk: {owner:?} -> {center:?}; {destination:?} {x},{y}",
                            );
                            painted += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(painted > 100, "fixture must exercise the retained-chart path");
}

#[test]
fn coincident_chart_stamping_keeps_identity_blend_and_source_over_semantics() {
    let first = sprite([255, 32, 0]);
    let second = sprite([0, 128, 255]);
    for face in Face::ALL {
        for &(u, v) in &[(0.5, 0.5), (63.5, 0.5), (0.5, 63.5), (63.5, 63.5), (32.5, 32.5)] {
            let anchor = SurfacePoint::new(face, u, v);
            for mix in [0.0, 0.37, 1.0] {
                let pose = Pose { first: &first, second: &second, mix };
                let bend = Bend { amplitude: 0.8, base: 32.0, root: 1.0, length: 40.0 };
                let mut established = Canvas::new();
                let mut retained = Canvas::new();
                established.set(face, anchor.pixel().0, anchor.pixel().1, [0.1, 0.2, 0.3]);
                retained.set(face, anchor.pixel().0, anchor.pixel().1, [0.1, 0.2, 0.3]);
                stamp_layers_bent(
                    &mut established,
                    anchor,
                    Vec2::new(1.0, 0.0),
                    &[(pose, 1.0)],
                    1.0,
                    0.7,
                    Mask::None,
                    bend,
                    &mut Vec::new(),
                );
                stamp_pose_in_chart(
                    &mut retained,
                    anchor,
                    anchor,
                    Vec2::new(1.0, 0.0),
                    pose,
                    0.7,
                    Mask::None,
                    bend,
                    &mut Vec::new(),
                );
                assert_same(&retained, &established, &format!("{anchor:?}, mix={mix}"));
            }
        }
    }
}
