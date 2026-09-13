//! Independent image-level regression fixtures from the in-progress Astra review.
//! These assert observable continuity and compositing properties, not mask formulas.

use cubarium_render::{Canvas, Mask, Pose, Sprite, stamp_pose};
use cubarium_surface::{SurfacePoint, Vec2};
use cube_proto::Face;

fn draw(pose: Pose<'_>, anchor: SurfacePoint, scale: f64, mask: Mask) -> Canvas {
    let mut image = Canvas::new();
    stamp_pose(
        &mut image,
        anchor,
        Vec2::new(1.0, 0.0),
        pose,
        scale,
        1.0,
        mask,
        &mut Vec::new(),
    );
    image
}

fn pixels(image: &Canvas) -> impl Iterator<Item = [f32; 3]> + '_ {
    Face::ALL.into_iter().flat_map(move |face| {
        (0..64u8).flat_map(move |y| (0..64u8).map(move |x| image.get(face, x, y)))
    })
}

fn peak(image: &Canvas) -> f32 {
    pixels(image).flatten().fold(0.0, f32::max)
}

fn total(image: &Canvas) -> [f64; 3] {
    pixels(image).fold([0.0; 3], |mut sum, pixel| {
        for c in 0..3 {
            sum[c] += f64::from(pixel[c]);
        }
        sum
    })
}

fn assert_same_image(actual: &Canvas, expected: &Canvas) {
    let max_difference = pixels(actual)
        .zip(pixels(expected))
        .flat_map(|(a, b)| (0..3).map(move |c| (a[c] - b[c]).abs()))
        .fold(0.0, f32::max);
    assert!(max_difference < 1e-6, "image difference {max_difference}");
}

fn white_pixel() -> Sprite {
    Sprite::from_rgba(1, 1, Vec2::new(0.5, 0.5), &[255, 255, 255, 255]).unwrap()
}

#[test]
fn opaque_pose_overlap_never_exposes_the_background() {
    let red = Sprite::from_rgba(1, 1, Vec2::new(0.5, 0.5), &[255, 0, 0, 255]).unwrap();
    let green = Sprite::from_rgba(1, 1, Vec2::new(0.5, 0.5), &[0, 255, 0, 255]).unwrap();
    for mix in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let mut image = Canvas::new();
        image.set(Face::Front, 32, 32, [0.0, 0.0, 1.0]);
        stamp_pose(
            &mut image,
            SurfacePoint::pixel_center(Face::Front, 32, 32),
            Vec2::new(1.0, 0.0),
            Pose {
                first: &red,
                second: &green,
                mix,
            },
            1.0,
            1.0,
            Mask::None,
            &mut Vec::new(),
        );
        let pixel = image.get(Face::Front, 32, 32);
        assert!(pixel[2] < 1e-6, "background leaked at mix {mix}: {pixel:?}");
        assert!((pixel[0] + pixel[1] - 1.0).abs() < 1e-6);
        if mix == 0.5 {
            assert!((pixel[0] - pixel[1]).abs() < 1e-6);
        }
    }
}

#[test]
fn radial_reveal_starts_continuously_at_centered_and_subpixel_pivots() {
    let sprite = white_pixel();
    for (x, y) in [(32.5, 32.5), (32.25, 32.25)] {
        let anchor = SurfacePoint::new(Face::Front, x, y);
        let zero = draw(
            Pose::still(&sprite),
            anchor,
            1.0,
            Mask::Radial { reveal: 0.0 },
        );
        let tiny = draw(
            Pose::still(&sprite),
            anchor,
            1.0,
            Mask::Radial { reveal: 1e-6 },
        );
        let full = draw(Pose::still(&sprite), anchor, 1.0, Mask::None);
        assert_eq!(peak(&zero), 0.0);
        assert!(peak(&full) > 0.1, "fixture must paint visible pixels");
        assert!(
            peak(&tiny) < 1e-4,
            "an infinitesimal radial reveal visibly popped at ({x}, {y}): {}",
            peak(&tiny)
        );
    }
}

#[test]
fn axial_reveal_starts_continuously_including_filtered_bottom_edge() {
    let sprite = white_pixel();
    // The offset makes a destination pixel sample below the source texel's center,
    // where the renderer's spatial filter still carries nonzero sprite coverage.
    for (x, y) in [(32.5, 32.5), (32.5, 32.25)] {
        let anchor = SurfacePoint::new(Face::Front, x, y);
        let zero = draw(
            Pose::still(&sprite),
            anchor,
            1.0,
            Mask::Axial { reveal: 0.0 },
        );
        let tiny = draw(
            Pose::still(&sprite),
            anchor,
            1.0,
            Mask::Axial { reveal: 1e-6 },
        );
        assert_eq!(peak(&zero), 0.0);
        assert!(
            peak(&tiny) < 1e-4,
            "an infinitesimal axial reveal visibly popped at ({x}, {y}): {}",
            peak(&tiny)
        );
    }
}

#[test]
fn fully_revealed_poses_equal_unmasked_poses() {
    let first = Sprite::from_rgba(
        2,
        2,
        Vec2::new(1.0, 1.0),
        &[
            255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 255, 255, 255, 255, 0,
        ],
    )
    .unwrap();
    let second = Sprite::from_rgba(
        2,
        2,
        Vec2::new(1.0, 1.0),
        &[
            0, 255, 0, 128, 255, 0, 0, 255, 255, 255, 255, 0, 0, 0, 255, 255,
        ],
    )
    .unwrap();
    for mix in [0.0, 0.37, 1.0] {
        let pose = Pose {
            first: &first,
            second: &second,
            mix,
        };
        for anchor in [
            SurfacePoint::new(Face::Front, 32.25, 32.25),
            SurfacePoint::new(Face::Front, 63.25, 32.25),
        ] {
            let expected = draw(pose, anchor, 1.0, Mask::None);
            assert!(peak(&expected) > 0.0);
            for mask in [Mask::Axial { reveal: 32.0 }, Mask::Radial { reveal: 32.0 }] {
                assert_same_image(&draw(pose, anchor, 1.0, mask), &expected);
            }
        }
    }
}

#[test]
fn fractional_pose_blending_preserves_coverage_across_side_and_top_seams() {
    let first = Sprite::from_rgba(
        4,
        1,
        Vec2::new(2.0, 0.5),
        &[
            255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 255, 255, 255, 255, 255,
        ],
    )
    .unwrap();
    let second = Sprite::from_rgba(
        4,
        1,
        Vec2::new(2.0, 0.5),
        &[
            0, 0, 255, 128, 255, 0, 0, 255, 255, 255, 255, 255, 0, 255, 0, 255,
        ],
    )
    .unwrap();
    for mix in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let pose = Pose {
            first: &first,
            second: &second,
            mix,
        };
        let center = draw(
            pose,
            SurfacePoint::new(Face::Front, 32.25, 32.25),
            1.0,
            Mask::None,
        );
        for (anchor, adjacent) in [
            (SurfacePoint::new(Face::Front, 63.25, 32.25), Face::Right),
            (SurfacePoint::new(Face::Front, 32.25, 0.25), Face::Top),
        ] {
            let seam = draw(pose, anchor, 1.0, Mask::None);
            for (a, b) in total(&seam).into_iter().zip(total(&center)) {
                assert!(
                    (a - b).abs() < 1e-5,
                    "coverage changed at {adjacent:?}, mix {mix}: {a} vs {b}"
                );
            }
            for face in [Face::Front, adjacent] {
                let visible = (0..64u8)
                    .any(|y| (0..64u8).any(|x| seam.get(face, x, y).into_iter().any(|c| c > 0.0)));
                assert!(visible, "seam fixture must actually paint {face:?}");
            }
        }
    }
}

fn unequal_footprints() -> (Sprite, Sprite) {
    let first = Sprite::from_rgba(5, 1, Vec2::new(2.5, 0.5), &[255; 20]).unwrap();
    let mut bytes = [0; 20];
    bytes[8..12].copy_from_slice(&[255, 255, 255, 255]);
    let second = Sprite::from_rgba(5, 1, Vec2::new(2.5, 0.5), &bytes).unwrap();
    (first, second)
}

#[test]
fn exact_second_endpoint_ignores_unused_first_sprite_footprint() {
    let (first, second) = unequal_footprints();
    let scale = 3.0;
    assert!(first.extent() * scale > 9.0);
    assert!(second.extent() * scale < 9.0);
    let anchor = SurfacePoint::new(Face::Front, 32.25, 32.25);
    let expected = draw(Pose::still(&second), anchor, scale, Mask::None);
    let endpoint = draw(
        Pose {
            first: &first,
            second: &second,
            mix: 1.0,
        },
        anchor,
        scale,
        Mask::None,
    );
    assert!(peak(&expected) > 0.0);
    assert_same_image(&endpoint, &expected);
}

#[test]
fn exact_second_endpoint_reports_the_footprint_it_actually_draws() {
    let (first, second) = unequal_footprints();
    let endpoint = Pose {
        first: &first,
        second: &second,
        mix: 1.0,
    };
    assert_eq!(
        endpoint.extent(),
        second.extent(),
        "unused first pose must not inflate endpoint bounds"
    );
}
