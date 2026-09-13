//! Standalone diagnostic tests against the public renderer API; no production edits.
//! At 7b8ad4f three coverage/continuity tests FAIL, demonstrating the review findings.
#![cfg(test)]
use cubarium_render::{Canvas, RigPart, Sprite, rig_radius, stamp_rig_scaled};
use cubarium_surface::{Face, SurfacePoint, Vec2};

fn texel(alpha: f32) -> Sprite {
    Sprite::from_premultiplied(1, 1, Vec2::new(0.5, 0.5), vec![[alpha; 4]]).unwrap()
}

fn draw(root: SurfacePoint, heading: Vec2, scale: f64, pad: bool) -> Canvas {
    let sprite = texel(1.0);
    let transparent = texel(0.0);
    let mut parts = vec![RigPart { sprite: &sprite, offset: Vec2::ZERO, layer: 0 }];
    if pad {
        // A transparent part expands only the query, with no material contribution.
        parts.push(RigPart { sprite: &transparent, offset: Vec2::new(5.0, 0.0), layer: 0 });
    }
    let states = [(&parts[..], 1.0)];
    eprintln!("scale={scale} unscaled_query={} padded={pad}", rig_radius(&states));
    let mut canvas = Canvas::new();
    stamp_rig_scaled(&mut canvas, root, heading, &states, scale, 1.0, &mut Vec::new());
    canvas
}

#[test]
fn automatic_query_retains_the_minified_corner_tail() {
    let root = SurfacePoint::new(Face::Front, 31.902, 31.902);
    let auto = draw(root, Vec2::new(1.0, 0.0), 0.2, false);
    let wide = draw(root, Vec2::new(1.0, 0.0), 0.2, true);
    let expected = wide.get(Face::Front, 32, 32);
    eprintln!("auto={:?} wide={expected:?}", auto.get(Face::Front, 32, 32));
    assert!(expected[0] > 0.0, "the generous query must actually paint the fixture");
    assert_eq!(auto.get(Face::Front, 32, 32), expected);
}

#[test]
fn destination_box_does_not_sample_beyond_its_chart_square() {
    let root = SurfacePoint::new(Face::Front, 32.5, 31.68);
    let canvas = draw(root, Vec2::new(1.0, 1.0), 0.2, false);
    // Pixel32,32 has y extent[32,33]. The rotated sprite's ENTIRE bilinear
    // support reaches at most root.y + sqrt(2)*0.2 <32, hence no overlap.
    assert!(root.v + 2.0_f64.sqrt() * 0.2 < 32.0);
    let got = canvas.get(Face::Front, 32, 32);
    eprintln!("outside true destination box: {got:?}");
    assert_eq!(got, [0.0; 3]);
}

#[test]
fn representative_n4_scale_keeps_the_existing_margin() {
    for heading in [Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0)] {
        for phase in [0.0, 0.125, 0.49] {
            let root = SurfacePoint::new(Face::Front, 31.5 + phase, 31.625);
            let auto = draw(root, heading, 0.316, false);
            let wide = draw(root, heading, 0.316, true);
            for y in 28..36 {
                for x in 28..36 {
                    assert_eq!(auto.get(Face::Front, x, y), wide.get(Face::Front, x, y));
                }
            }
        }
    }
}

#[test]
fn approaching_adult_scale_does_not_pop_the_pixel_filter() {
    let root = SurfacePoint::new(Face::Front, 32.5, 32.5);
    let adult = draw(root, Vec2::new(1.0, 0.0), 1.0, false);
    let almost = draw(root, Vec2::new(1.0, 0.0), 1.0 - 1e-9, false);
    let a = adult.get(Face::Front, 32, 32)[0];
    let b = almost.get(Face::Front, 32, 32)[0];
    eprintln!("adult center={a} scale1-epsilon center={b}");
    assert!((a - b).abs() < 1e-5, "continuous geometry must not switch filters abruptly");
}
