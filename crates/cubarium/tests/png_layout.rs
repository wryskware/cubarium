//! The contract's required PNG-layout test: face `(x, y)` pixels land at the documented
//! net offsets — Top above Front, and Left, Front, Right, Back in a row — with the
//! unused cells black.

use cube_proto::{FACE_SIZE, Face, Frame};
use cubarium::net::{PNG_HEIGHT, PNG_WIDTH, net_origin, net_rgb8};

/// The offsets the README documents, written out independently of `net_origin`.
const DOCUMENTED: [(Face, usize, usize); 5] = [
    (Face::Top, 64, 0),
    (Face::Left, 0, 64),
    (Face::Front, 64, 64),
    (Face::Right, 128, 64),
    (Face::Back, 192, 64),
];

fn px(buf: &[u8], x: usize, y: usize) -> [u8; 3] {
    let o = (y * PNG_WIDTH + x) * 3;
    [buf[o], buf[o + 1], buf[o + 2]]
}

#[test]
fn the_net_image_is_four_faces_wide_and_two_high() {
    assert_eq!((PNG_WIDTH, PNG_HEIGHT), (256, 128));
    for (face, ox, oy) in DOCUMENTED {
        assert_eq!(net_origin(face, 1, 0), (ox, oy), "{face:?}");
    }
}

#[test]
fn every_face_pixel_lands_at_its_documented_offset() {
    // A frame where every pixel encodes its own face and coordinates.
    let mut frame = Frame::black();
    for face in Face::ALL {
        for y in 0..FACE_SIZE {
            for x in 0..FACE_SIZE {
                frame.set(face, x, y, [face.index() as u8 + 1, x as u8, y as u8]);
            }
        }
    }
    let mut buf = Vec::new();
    net_rgb8(&frame, &mut buf);
    assert_eq!(buf.len(), PNG_WIDTH * PNG_HEIGHT * 3);

    for (face, ox, oy) in DOCUMENTED {
        for y in 0..FACE_SIZE {
            for x in 0..FACE_SIZE {
                assert_eq!(
                    px(&buf, ox + x, oy + y),
                    [face.index() as u8 + 1, x as u8, y as u8],
                    "{face:?} pixel ({x}, {y}) at net ({}, {})",
                    ox + x,
                    oy + y
                );
            }
        }
    }
}

#[test]
fn the_cells_without_a_face_stay_black() {
    let mut frame = Frame::black();
    frame.fill([200, 210, 220]);
    let mut buf = Vec::new();
    net_rgb8(&frame, &mut buf);

    // The top row holds only Top, in columns 64..128.
    let mut black = 0;
    for y in 0..64 {
        for x in 0..PNG_WIDTH {
            if (64..128).contains(&x) {
                assert_eq!(px(&buf, x, y), [200, 210, 220], "Top at ({x}, {y})");
            } else {
                assert_eq!(px(&buf, x, y), [0, 0, 0], "background at ({x}, {y})");
                black += 1;
            }
        }
    }
    assert_eq!(black, 3 * 64 * 64, "three empty cells");
    // The bottom row is entirely faces.
    for y in 64..128 {
        for x in 0..PNG_WIDTH {
            assert_eq!(px(&buf, x, y), [200, 210, 220], "({x}, {y})");
        }
    }
}
