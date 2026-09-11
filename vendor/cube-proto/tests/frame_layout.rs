//! `Frame` storage contract, checked against `docs/ARCHITECTURE.md` ("Face convention",
//! "cube-proto"). Expected byte offsets are computed from the document's formula,
//! never by asking the crate where it put something.

use cube_proto::{Face, Frame, FACE_BYTES, FACE_SIZE, FRAME_BYTES, NUM_FACES};

/// The doc: "faces stored contiguously in Face order, each face row-major RGB8", so a
/// pixel lives at `face_index * 12288 + (y * 64 + x) * 3`.
fn doc_offset(face_index: usize, x: usize, y: usize) -> usize {
    face_index * 12288 + (y * 64 + x) * 3
}

#[test]
fn constants_match_the_document() {
    assert_eq!(FACE_SIZE, 64);
    assert_eq!(NUM_FACES, 5);
    assert_eq!(FACE_BYTES, 12288);
    assert_eq!(FRAME_BYTES, 61440);
    assert_eq!(cube_proto::DEFAULT_PORT, 7392);
}

#[test]
fn face_order_is_front_right_back_left_top() {
    let want = [Face::Front, Face::Right, Face::Back, Face::Left, Face::Top];
    assert_eq!(Face::ALL, want);
    for (i, f) in want.into_iter().enumerate() {
        assert_eq!(f.index(), i, "{f:?} must have index {i}");
        assert_eq!(Face::from_index(i as u8), Some(f));
    }
    assert_eq!(Face::from_index(5), None);
    assert_eq!(Face::from_index(0xFF), None);
}

#[test]
fn black_is_all_zero() {
    assert!(Frame::black().as_bytes().iter().all(|&b| b == 0));
    assert_eq!(Frame::black().as_bytes().len(), FRAME_BYTES);
}

#[test]
fn set_get_round_trip_at_every_face_corner() {
    let corners = [(0, 0), (63, 0), (0, 63), (63, 63)];
    let mut frame = Frame::black();

    // Give every (face, corner) a distinct colour so an aliasing bug cannot hide.
    for (fi, face) in Face::ALL.into_iter().enumerate() {
        for (ci, (x, y)) in corners.into_iter().enumerate() {
            let rgb = [(fi * 20 + 1) as u8, (ci * 30 + 2) as u8, (fi * 4 + ci) as u8];
            frame.set(face, x, y, rgb);
        }
    }
    for (fi, face) in Face::ALL.into_iter().enumerate() {
        for (ci, (x, y)) in corners.into_iter().enumerate() {
            let rgb = [(fi * 20 + 1) as u8, (ci * 30 + 2) as u8, (fi * 4 + ci) as u8];
            assert_eq!(frame.get(face, x, y), rgb, "{face:?} corner ({x},{y})");
            let o = doc_offset(fi, x, y);
            assert_eq!(
                &frame.as_bytes()[o..o + 3],
                &rgb,
                "{face:?} corner ({x},{y}) must live at byte {o}"
            );
        }
    }
}

#[test]
fn pixel_offsets_follow_the_documented_formula_everywhere() {
    // Exhaustive over every pixel of every face: write a value derived from the pixel,
    // then confirm the byte at the documented offset carries it.
    let mut frame = Frame::black();
    for (fi, face) in Face::ALL.into_iter().enumerate() {
        for y in 0..FACE_SIZE {
            for x in 0..FACE_SIZE {
                frame.set(face, x, y, [x as u8, y as u8, fi as u8]);
            }
        }
    }
    let bytes = frame.as_bytes();
    for (fi, _face) in Face::ALL.into_iter().enumerate() {
        for y in 0..FACE_SIZE {
            for x in 0..FACE_SIZE {
                let o = doc_offset(fi, x, y);
                assert_eq!(
                    [bytes[o], bytes[o + 1], bytes[o + 2]],
                    [x as u8, y as u8, fi as u8],
                    "face {fi} pixel ({x},{y}) at byte {o}"
                );
            }
        }
    }
}

#[test]
fn face_slices_are_exact_disjoint_and_in_face_order() {
    let mut frame = Frame::black();
    // Stamp each face with a unique constant.
    for (fi, face) in Face::ALL.into_iter().enumerate() {
        for y in 0..FACE_SIZE {
            for x in 0..FACE_SIZE {
                frame.set(face, x, y, [(fi + 1) as u8; 3]);
            }
        }
    }
    for (fi, face) in Face::ALL.into_iter().enumerate() {
        let slice = frame.face(face);
        assert_eq!(slice.len(), FACE_BYTES, "{face:?} slice length");
        assert!(
            slice.iter().all(|&b| b == (fi + 1) as u8),
            "{face:?} slice is not disjoint from its neighbours"
        );
        // In Face order: the slice must be byte-identical to the frame window at
        // `fi * FACE_BYTES`.
        let o = fi * FACE_BYTES;
        assert_eq!(slice, &frame.as_bytes()[o..o + FACE_BYTES]);
    }
}

#[test]
fn face_mut_writes_land_in_the_right_window() {
    let mut frame = Frame::black();
    frame.face_mut(Face::Left)[0] = 0xAB;
    frame.face_mut(Face::Left)[FACE_BYTES - 1] = 0xCD;
    assert_eq!(frame.as_bytes()[3 * FACE_BYTES], 0xAB);
    assert_eq!(frame.as_bytes()[4 * FACE_BYTES - 1], 0xCD);
    assert_eq!(frame.get(Face::Left, 0, 0)[0], 0xAB);
    assert_eq!(frame.get(Face::Left, 63, 63)[2], 0xCD);
}

#[test]
fn as_bytes_mut_is_the_same_storage() {
    let mut frame = Frame::black();
    frame.as_bytes_mut()[doc_offset(4, 10, 11)] = 0x7F;
    assert_eq!(frame.get(Face::Top, 10, 11), [0x7F, 0, 0]);
}

#[test]
#[should_panic(expected = "out of range")]
fn get_rejects_x_out_of_range() {
    Frame::black().get(Face::Front, FACE_SIZE, 0);
}

#[test]
#[should_panic(expected = "out of range")]
fn set_rejects_y_out_of_range() {
    Frame::black().set(Face::Front, 0, FACE_SIZE, [0; 3]);
}
