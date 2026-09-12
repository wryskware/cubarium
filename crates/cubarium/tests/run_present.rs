//! The contract's presentation verification: the PNG sink shows substrate and bodies on
//! more than one face, and the ambient image contains nothing but what the spec lists.

mod support;

use cube_proto::{FACE_SIZE, Face};
use cubarium::net::{PNG_HEIGHT, PNG_WIDTH, net_origin};
use cubarium_render::srgb_encode;
use support::{Scratch, run};

/// Decode one of the sink's captures into a tightly packed RGB8 net image.
fn read_net(path: &std::path::Path) -> Vec<u8> {
    let file = std::io::BufReader::new(std::fs::File::open(path).unwrap());
    let decoder = png::Decoder::new(file);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!((info.width as usize, info.height as usize), (PNG_WIDTH, PNG_HEIGHT));
    assert_eq!(info.color_type, png::ColorType::Rgb);
    buf.truncate(info.buffer_size());
    buf
}

/// The encoded night floor: `#12093A` at 0.12 brightness, which every pixel carries.
fn floor_bytes() -> [u8; 3] {
    let f = cubarium::present::PALETTE.floor;
    [srgb_encode(f[0]), srgb_encode(f[1]), srgb_encode(f[2])]
}

/// Per-face counts of pixels above the bare floor and of pixels brighter than the
/// substrate can reach.
///
/// The producer substrate ramps `#1E2798` → `#42C5F8` scaled by `min(P/P_max, 1)`, and a
/// settled world keeps `P` under about half of `P_max`, so the substrate's brightest
/// channel encodes to roughly 150. Bodies are drawn at 0.55–0.8 of a full-scale hue and
/// clear 200 in their dominant channel at either end of the magenta-to-cyan ramp, so
/// 170 separates them without assuming a hue.
fn face_stats(net: &[u8]) -> Vec<(Face, usize, usize)> {
    let floor = floor_bytes();
    Face::ALL
        .into_iter()
        .map(|face| {
            let (ox, oy) = net_origin(face, 1, 0);
            let (mut lit, mut bright) = (0, 0);
            for y in 0..FACE_SIZE {
                for x in 0..FACE_SIZE {
                    let o = ((oy + y) * PNG_WIDTH + ox + x) * 3;
                    let px = [net[o], net[o + 1], net[o + 2]];
                    if px != floor {
                        lit += 1;
                    }
                    if px.iter().any(|&c| c > 170) {
                        bright += 1;
                    }
                }
            }
            (face, lit, bright)
        })
        .collect()
}

#[test]
fn the_png_sink_captures_substrate_and_bodies_on_several_faces() {
    let scratch = Scratch::new("png");
    let state = scratch.join("state");
    let out = scratch.join("captures");

    // Ten simulated seconds at twenty times real time: half a second of wall clock,
    // enough for several rendered frames.
    let outcome = run(&[
        "--sink", "png",
        "--speed", "20",
        "--seconds", "10",
        "--fresh",
        "--state", state.to_str().unwrap(),
        "--out", out.to_str().unwrap(),
        "--every", "3",
    ]);
    assert_eq!(outcome.final_tick, 200);
    assert!(outcome.frames > 0, "the png sink must have been given frames");

    let mut captures: Vec<std::path::PathBuf> = std::fs::read_dir(&out)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    captures.sort();
    assert!(captures.len() >= 2, "expected several captures, got {captures:?}");
    assert!(
        captures.iter().any(|p| p.file_name().unwrap() == "final.png"),
        "the sink must write final.png on a clean stop"
    );

    let net = read_net(&out.join("final.png"));
    let stats = face_stats(&net);
    let substrate_faces = stats.iter().filter(|(_, lit, _)| *lit > 0).count();
    assert_eq!(substrate_faces, 5, "the producer substrate covers every face: {stats:?}");
    let body_faces = stats.iter().filter(|(_, _, bright)| *bright > 0).count();
    assert!(body_faces > 1, "bodies must appear on more than one face: {stats:?}");

    // A body is a compact stamp, not a wash: far fewer bright pixels than lit ones.
    let total_lit: usize = stats.iter().map(|(_, l, _)| l).sum();
    let total_bright: usize = stats.iter().map(|(_, _, b)| b).sum();
    assert!(total_bright > 0 && total_bright * 4 < total_lit, "{total_bright} of {total_lit}");

    // The floor is under every pixel of every face, including the faces the world has
    // not reached: nothing inside a face image is black.
    for face in Face::ALL {
        let (ox, oy) = net_origin(face, 1, 0);
        for y in 0..FACE_SIZE {
            for x in 0..FACE_SIZE {
                let o = ((oy + y) * PNG_WIDTH + ox + x) * 3;
                assert!(
                    net[o + 2] >= floor_bytes()[2],
                    "{face:?} {x},{y} is below the night floor"
                );
            }
        }
    }
}

#[test]
fn the_presenter_paints_only_what_the_spec_lists() {
    use cubarium::present::{PALETTE, Presenter};
    use cubarium_core::{World, WorldConfig};
    use cubarium_render::Canvas;

    let mut world = World::new(WorldConfig::default()).unwrap();
    for _ in 0..200 {
        world.step();
    }
    let view = world.render_view();
    let mut presenter = Presenter::new();
    presenter.observe(&view);
    let mut canvas = Canvas::new();
    presenter.draw(&view, 0.0, &mut canvas);

    // The same fields with the organisms taken out: whatever the two images differ by is
    // exactly the bodies and their trails, whatever hue those bodies happen to carry.
    let mut fields_only = view.clone();
    fields_only.organisms.clear();
    let mut substrate = Canvas::new();
    let mut blank = Presenter::new();
    blank.draw(&fields_only, 0.0, &mut substrate);

    let mut body_faces = std::collections::HashSet::new();
    for face in Face::ALL {
        for y in 0..64u8 {
            for x in 0..64u8 {
                let px = canvas.get(face, x, y);
                for c in px {
                    assert!(c.is_finite() && c >= 0.0, "{face:?} {x},{y}: {px:?}");
                }
                let under = substrate.get(face, x, y);
                // Bodies and trails only ever add light.
                for i in 0..3 {
                    assert!(px[i] >= under[i] - 1e-6, "{face:?} {x},{y}: {px:?} vs {under:?}");
                }
                if (0..3).any(|i| px[i] - under[i] > 1e-4) {
                    body_faces.insert(face);
                }
            }
        }
    }
    assert!(body_faces.len() > 1, "bodies must be visible on several faces: {body_faces:?}");

    // With no organisms and no fields there is the night floor and nothing else.
    let mut empty = fields_only;
    empty.producer.iter_mut().for_each(|v| *v = 0.0);
    empty.detritus.iter_mut().for_each(|v| *v = 0.0);
    let mut blank = Presenter::new();
    blank.draw(&empty, 0.0, &mut canvas);
    for face in Face::ALL {
        for y in 0..64u8 {
            for x in 0..64u8 {
                assert_eq!(canvas.get(face, x, y), PALETTE.floor, "{face:?} {x},{y}");
            }
        }
    }
}
