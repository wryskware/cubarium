//! The contract's required ray-caster test: the cube preview's face and `(u, v)`
//! inversion round-trips the `cubarium_surface::face_frame` embedding for the pixel
//! centers of every face — all 20,480 of them — and agrees with the frame bytes the
//! shim receives.

use std::collections::HashSet;

use cube_proto::{Face, Frame};
use cubarium::raycast::{Camera, cast, invert_face};
use cubarium_surface::{SurfacePoint, face_frame};

#[test]
fn the_inversion_round_trips_every_pixel_center_on_every_face() {
    let mut checked = 0u32;
    for face in Face::ALL {
        let n = face_frame(face).normal;
        for y in 0..64u8 {
            for x in 0..64u8 {
                let target = SurfacePoint::pixel_center(face, x, y).embed();

                // The embedding inverts exactly.
                let (u, v) = invert_face(face, target).expect("a pixel center is on its face");
                assert!((u - (f64::from(x) + 0.5)).abs() < 1e-9, "{face:?} ({x},{y}) u = {u}");
                assert!((v - (f64::from(y) + 0.5)).abs() < 1e-9, "{face:?} ({x},{y}) v = {v}");

                // And the whole cast — nearest of the five planes, inside the square —
                // picks this face and this pixel.
                let origin = [target[0] + n[0] * 4.0, target[1] + n[1] * 4.0, target[2] + n[2] * 4.0];
                let hit = cast(origin, [-n[0], -n[1], -n[2]]).expect("the ray hits the cube");
                assert_eq!(hit.face, face, "{face:?} ({x},{y})");
                assert_eq!(hit.pixel(), (x, y), "{face:?} ({x},{y})");
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 20_480, "every surface pixel center was checked");
}

#[test]
fn the_cube_samples_the_same_bytes_the_shim_receives() {
    // Tag every face with its own color; whatever the preview shows must be one of
    // those exact byte triples, read out of the encoded frame.
    let mut frame = Frame::black();
    let colors = [[10, 0, 0], [0, 20, 0], [0, 0, 30], [40, 40, 0], [0, 50, 50]];
    for (face, c) in Face::ALL.into_iter().zip(colors) {
        frame.fill_face(face, c);
    }

    let camera = Camera::default();
    let mut hits = Vec::new();
    camera.trace_viewport(96, &mut hits);

    let mut seen = HashSet::new();
    let mut background = 0;
    for h in &hits {
        match h {
            Some((face, x, y)) => {
                let got = frame.get(*face, usize::from(*x), usize::from(*y));
                assert_eq!(got, colors[face.index()], "{face:?}");
                seen.insert(*face);
            }
            None => background += 1,
        }
    }
    assert!(background > 0, "the viewport corners must be background");
    assert!(seen.len() >= 3, "the default corner view shows three faces: {seen:?}");
}

#[test]
fn no_ray_ever_reports_a_pixel_outside_a_chart() {
    // Sweep the camera all the way round, including from below the open bottom.
    for yaw_k in 0..12 {
        for pitch_deg in [-85.0, -40.0, 0.0, 30.0, 85.0] {
            let c = Camera {
                yaw: f64::from(yaw_k) * std::f64::consts::TAU / 12.0,
                pitch: pitch_deg * std::f64::consts::PI / 180.0,
                ..Camera::default()
            };
            let mut hits = Vec::new();
            c.trace_viewport(48, &mut hits);
            assert_eq!(hits.len(), 48 * 48);
            for h in hits.iter().flatten() {
                assert!(h.1 < 64 && h.2 < 64, "out-of-chart pixel {h:?}");
            }
        }
    }
}

#[test]
fn there_is_no_sixth_face_where_the_bottom_would_be() {
    // Only five planes exist, so a ray up the y axis passes through the open bottom and
    // lands on the inside of Top rather than on a bottom face.
    assert_eq!(cast([0.0, -6.0, 0.0], [0.0, 1.0, 0.0]).map(|h| h.face), Some(Face::Top));
    assert_eq!(cast([0.4, -6.0, -0.3], [0.0, 1.0, 0.0]).map(|h| h.face), Some(Face::Top));
    // A ray grazing just below the cube hits no face square at all: background.
    assert!(cast([0.0, -1.5, 5.0], [0.0, 0.0, -1.0]).is_none());
    // No cast ever names a face outside the five-face set.
    for h in [
        cast([5.0, 1.0, 5.0], [-1.0, -0.2, -1.0]),
        cast([-5.0, 3.0, 0.0], [1.0, -0.5, 0.0]),
    ]
    .into_iter()
    .flatten()
    {
        assert!(Face::ALL.contains(&h.face));
    }
}
