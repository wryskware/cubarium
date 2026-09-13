//! Independent observable regressions for the public wind deformation contract.

use cubarium_render::{Bend, Canvas, Mask, Pose, Sprite, stamp_layers, stamp_layers_bent};
use cubarium_surface::{SurfacePoint, Vec2};
use cube_proto::Face;

fn draw(sprite: &Sprite, anchor: SurfacePoint, mask: Mask, bend: Bend) -> Canvas {
    let mut image = Canvas::new();
    stamp_layers_bent(
        &mut image,
        anchor,
        Vec2::new(1.0, 0.0),
        &[(Pose::still(sprite), 1.0)],
        1.0,
        1.0,
        mask,
        bend,
        &mut Vec::new(),
    );
    image
}

fn max_difference(a: &Canvas, b: &Canvas) -> f32 {
    let mut difference = 0.0f32;
    for face in Face::ALL {
        for y in 0..64u8 {
            for x in 0..64u8 {
                for (a, b) in a.get(face, x, y).into_iter().zip(b.get(face, x, y)) {
                    difference = difference.max((a - b).abs());
                }
            }
        }
    }
    difference
}

#[test]
fn zero_wind_is_exactly_the_existing_layer_compositor() {
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
    let layers = [(
        Pose {
            first: &first,
            second: &second,
            mix: 0.37,
        },
        1.0,
    )];
    for anchor in [
        SurfacePoint::new(Face::Front, 32.25, 32.75),
        SurfacePoint::new(Face::Front, 63.25, 0.25),
    ] {
        for mask in [
            Mask::None,
            Mask::Axial { reveal: 1.4 },
            Mask::Radial { reveal: 1.2 },
        ] {
            let mut old = Canvas::new();
            let mut bent = Canvas::new();
            stamp_layers(
                &mut old,
                anchor,
                Vec2::new(0.8, 0.6),
                &layers,
                1.0,
                0.85,
                mask,
                &mut Vec::new(),
            );
            stamp_layers_bent(
                &mut bent,
                anchor,
                Vec2::new(0.8, 0.6),
                &layers,
                1.0,
                0.85,
                mask,
                Bend::NONE,
                &mut Vec::new(),
            );
            assert_eq!(max_difference(&old, &bent), 0.0);
        }
    }
}

#[test]
fn radial_reveal_moves_with_the_material_under_a_constant_bend() {
    let sprite = Sprite::from_rgba(5, 5, Vec2::new(2.5, 2.5), &[255; 100]).unwrap();
    // The whole sprite is above the bend's mature length, so the public deformation
    // is an exact one-pixel translation. A material mask must translate with it.
    let bend = Bend {
        amplitude: 1.0,
        base: 100.0,
        root: 0.0,
        length: 1.0,
    };
    assert!(sprite.bend_headroom(bend.root, bend.length, bend.base) >= bend.amplitude);
    let mask = Mask::Radial { reveal: 1.25 };
    let actual = draw(
        &sprite,
        SurfacePoint::new(Face::Front, 32.5, 32.5),
        mask,
        bend,
    );
    let expected = draw(
        &sprite,
        SurfacePoint::new(Face::Front, 33.5, 32.5),
        mask,
        Bend::NONE,
    );
    assert!(
        max_difference(&actual, &expected) < 1e-6,
        "the radial mask stayed in destination coordinates instead of following translated material"
    );
}

#[test]
fn admitted_headroom_keeps_a_nonzero_bilinear_tail_inside_the_unfold() {
    // Its only texel center is 7.7px above the pivot: a valid source sprite near
    // the extent limit. The bend changes substantially across that texel's support.
    let sprite = Sprite::from_rgba(1, 1, Vec2::new(0.5, 8.2), &[255; 4]).unwrap();
    let amplitude = sprite.bend_headroom(0.0, 1.0, 0.0);
    assert!(amplitude.is_finite() && amplitude >= 0.0);
    let bend = Bend {
        amplitude,
        base: 0.0,
        root: 0.0,
        length: 1.0,
    };
    // This sample is 0.8px right/up from the white texel center, so its bilinear
    // coverage is 4%. It is beyond the mature bend length and receives full amplitude.
    // Place a real output pixel at that forward-mapped position, without reproducing
    // the implementation's headroom calculation or its footprint estimate.
    let anchor = SurfacePoint::new(Face::Front, 32.5 - (0.8 + amplitude), 32.5 + 8.5);
    let image = draw(&sprite, anchor, Mask::None, bend);
    let pixel = image.get(Face::Front, 32, 32);
    assert!(
        pixel[0] > 0.039 && pixel[0] < 0.041,
        "admitted amplitude {amplitude} clipped a known 4%-coverage filter tail: {pixel:?}"
    );
}

#[test]
fn admitted_headroom_preserves_filter_corners_under_a_constant_bend() {
    // This texel's center is at (0,5) relative to its pivot. The sampled corner
    // below is inside the original sprite extent, so this isolates loss caused by
    // wind rather than requiring a change to the existing unbent renderer.
    let sprite = Sprite::from_rgba(1, 1, Vec2::new(0.5, -4.5), &[255; 4]).unwrap();
    let amplitude = sprite.bend_headroom(0.0, 1.0, 100.0);
    assert!(amplitude.is_finite() && amplitude >= 0.0);
    let sample_x: f64 = 0.9;
    let sample_y: f64 = 5.9;
    assert!(sample_x.hypot(sample_y) < sprite.extent());
    let anchor = SurfacePoint::new(Face::Front, 32.5 - sample_x - amplitude, 32.5 - sample_y);
    let image = draw(
        &sprite,
        anchor,
        Mask::None,
        Bend {
            amplitude,
            base: 100.0,
            root: 0.0,
            length: 1.0,
        },
    );
    let pixel = image.get(Face::Front, 32, 32);
    assert!(
        pixel[0] > 0.0099 && pixel[0] < 0.0101,
        "admitted constant bend {amplitude} clipped a known 1%-coverage filter corner: {pixel:?}"
    );
}

#[test]
fn rooted_rows_stay_fixed_while_an_upper_row_moves() {
    let sprite = Sprite::from_rgba(1, 5, Vec2::new(0.5, 2.5), &[255; 20]).unwrap();
    let anchor = SurfacePoint::new(Face::Front, 32.5, 32.5);
    let still = draw(&sprite, anchor, Mask::None, Bend::NONE);
    for amplitude in [-0.5, 0.5] {
        let bent = draw(
            &sprite,
            anchor,
            Mask::None,
            Bend {
                amplitude,
                base: 0.0,
                root: 1.5,
                length: 3.0,
            },
        );
        for y in 33..=34 {
            for x in 29..=35 {
                assert_eq!(
                    bent.get(Face::Front, x, y),
                    still.get(Face::Front, x, y),
                    "root row moved under wind {amplitude}"
                );
            }
        }
        assert!(
            max_difference(&still, &bent) > 0.1,
            "fixture must actually move its tip"
        );
    }
}
