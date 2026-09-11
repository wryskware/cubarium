//! The unfolded net layout shared by the PNG sink and the preview window.
//!
//! One grid, one table: Top above Front on the upper row, and Left, Front, Right, Back
//! in a row below it. Nothing here knows anything about seams — it is a picture layout,
//! not geometry.

use cube_proto::{FACE_SIZE, Face, Frame};

/// Columns in the net grid.
pub const NET_COLS: usize = 4;
/// Rows in the net grid.
pub const NET_ROWS: usize = 2;

/// The `(column, row)` cell each face occupies.
pub const fn net_cell(face: Face) -> (usize, usize) {
    match face {
        Face::Top => (1, 0),
        Face::Left => (0, 1),
        Face::Front => (1, 1),
        Face::Right => (2, 1),
        Face::Back => (3, 1),
    }
}

/// The face occupying a grid cell, if any. The three empty cells of the top row are
/// background.
pub const fn face_at(col: usize, row: usize) -> Option<Face> {
    match (col, row) {
        (1, 0) => Some(Face::Top),
        (0, 1) => Some(Face::Left),
        (1, 1) => Some(Face::Front),
        (2, 1) => Some(Face::Right),
        (3, 1) => Some(Face::Back),
        _ => None,
    }
}

/// Top-left pixel of a face's image, for a net drawn at `scale` with `sep`-pixel
/// separators between cells. The PNG sink uses `scale = 1, sep = 0`; the preview uses
/// the window scale with `sep = 1`.
pub const fn net_origin(face: Face, scale: usize, sep: usize) -> (usize, usize) {
    let (col, row) = net_cell(face);
    let pitch = FACE_SIZE * scale + sep;
    (col * pitch, row * pitch)
}

/// Size of the whole net image for a scale and separator width.
pub const fn net_size(scale: usize, sep: usize) -> (usize, usize) {
    let pitch = FACE_SIZE * scale + sep;
    (NET_COLS * pitch - sep, NET_ROWS * pitch - sep)
}

/// Width of the PNG capture: four 64-pixel faces side by side.
pub const PNG_WIDTH: usize = NET_COLS * FACE_SIZE;
/// Height of the PNG capture: two rows of 64-pixel faces.
pub const PNG_HEIGHT: usize = NET_ROWS * FACE_SIZE;

/// Render the net at scale 1 with no separators into a tightly packed RGB8 buffer of
/// `PNG_WIDTH * PNG_HEIGHT * 3` bytes. Cells without a face stay black.
pub fn net_rgb8(frame: &Frame, out: &mut Vec<u8>) {
    out.clear();
    out.resize(PNG_WIDTH * PNG_HEIGHT * 3, 0);
    for face in Face::ALL {
        let (ox, oy) = net_origin(face, 1, 0);
        for y in 0..FACE_SIZE {
            let row = (oy + y) * PNG_WIDTH + ox;
            for x in 0..FACE_SIZE {
                let rgb = frame.get(face, x, y);
                let o = (row + x) * 3;
                out[o..o + 3].copy_from_slice(&rgb);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_cells_are_top_above_front_with_the_side_faces_in_a_row() {
        assert_eq!(net_cell(Face::Top), (1, 0));
        assert_eq!(net_cell(Face::Front), (1, 1));
        assert_eq!(net_cell(Face::Left), (0, 1));
        assert_eq!(net_cell(Face::Right), (2, 1));
        assert_eq!(net_cell(Face::Back), (3, 1));
        for face in Face::ALL {
            let (c, r) = net_cell(face);
            assert_eq!(face_at(c, r), Some(face));
        }
        assert_eq!(face_at(0, 0), None);
        assert_eq!(face_at(2, 0), None);
        assert_eq!(face_at(3, 0), None);
    }

    #[test]
    fn scaled_origins_and_sizes_account_for_separators() {
        assert_eq!(net_size(1, 0), (256, 128));
        assert_eq!(net_size(4, 1), (4 * 256 + 3, 4 * 128 + 1));
        assert_eq!(net_origin(Face::Back, 4, 1), (3 * 257, 257));
        assert_eq!(net_origin(Face::Top, 4, 1), (257, 0));
    }
}
