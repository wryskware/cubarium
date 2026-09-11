//! The desktop preview: a `minifb` window on the main thread showing the unfolded net
//! and a software-ray-cast cube, both sampled from the same encoded frame bytes the shim
//! receives.
//!
//! The window's `update_with_buffer` *is* the sink's submit: the simulation and render
//! loop runs on the main thread and calls this sink once per rendered frame.

use std::path::PathBuf;

use anyhow::{Context, Result};
use cube_proto::{FACE_SIZE, Face, Frame};
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};

use crate::net::{NET_COLS, NET_ROWS, face_at, net_origin, net_size};
use crate::raycast::Camera;

use super::FrameSink;

/// Window background and the net's separator color.
const BACKGROUND: u32 = 0x0010_1012;
/// Separator width between face cells, in window pixels.
const SEPARATOR: usize = 1;
/// The cube viewport is 256 cube-pixels square.
const CUBE_PIXELS: usize = 256;
/// Radians of yaw per window pixel of drag.
const DRAG_YAW: f64 = 0.008;
/// Radians of pitch per window pixel of drag.
const DRAG_PITCH: f64 = 0.006;

/// A `minifb` preview window.
pub struct PreviewSink {
    window: Window,
    buffer: Vec<u32>,
    width: usize,
    height: usize,
    scale: usize,
    net_w: usize,
    net_h: usize,
    cube_size: usize,
    camera: Camera,
    hits: Vec<Option<(Face, u8, u8)>>,
    hits_stale: bool,
    show_net: bool,
    show_cube: bool,
    quit: bool,
    drag: Option<(f32, f32)>,
    capture_dir: PathBuf,
    captures: u64,
    net_buf: Vec<u8>,
}

impl PreviewSink {
    pub fn new(scale: usize, capture_dir: impl Into<PathBuf>) -> Result<PreviewSink> {
        let scale = scale.max(1);
        let (net_w, net_h) = net_size(scale, SEPARATOR);
        let cube_size = CUBE_PIXELS * scale;
        let width = net_w + SEPARATOR + cube_size;
        let height = net_h.max(cube_size);
        let window = Window::new(
            "cubarium",
            width,
            height,
            WindowOptions { resize: false, ..WindowOptions::default() },
        )
        .context("creating the preview window")?;
        Ok(PreviewSink {
            window,
            buffer: vec![BACKGROUND; width * height],
            width,
            height,
            scale,
            net_w,
            net_h,
            cube_size,
            camera: Camera::default(),
            hits: Vec::new(),
            hits_stale: true,
            show_net: true,
            show_cube: true,
            quit: false,
            drag: None,
            capture_dir: capture_dir.into(),
            captures: 0,
            net_buf: Vec::new(),
        })
    }

    pub fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    pub fn camera(&self) -> Camera {
        self.camera
    }

    fn handle_input(&mut self, frame: &Frame) {
        for key in self.window.get_keys_pressed(KeyRepeat::No) {
            match key {
                Key::N => self.show_net = !self.show_net,
                Key::C => self.show_cube = !self.show_cube,
                Key::S => {
                    if let Err(e) = self.capture(frame) {
                        eprintln!("cubarium: capture failed: {e:#}");
                    }
                }
                Key::Q | Key::Escape => self.quit = true,
                _ => {}
            }
        }

        // Mouse drag orbits the cube.
        let pos = self.window.get_mouse_pos(MouseMode::Pass);
        if self.window.get_mouse_down(MouseButton::Left) {
            if let Some((x, y)) = pos {
                if let Some((px, py)) = self.drag {
                    let (dx, dy) = (f64::from(x - px), f64::from(y - py));
                    if dx != 0.0 || dy != 0.0 {
                        self.camera.orbit(dx * DRAG_YAW, dy * DRAG_PITCH);
                        self.hits_stale = true;
                    }
                }
                self.drag = Some((x, y));
            }
        } else {
            self.drag = None;
        }
    }

    fn capture(&mut self, frame: &Frame) -> Result<()> {
        std::fs::create_dir_all(&self.capture_dir).with_context(|| {
            format!("creating capture directory {}", self.capture_dir.display())
        })?;
        crate::net::net_rgb8(frame, &mut self.net_buf);
        let path = self.capture_dir.join(format!("preview_{:06}.png", self.captures));
        super::png::write_net_png(&path, &self.net_buf)?;
        self.captures += 1;
        eprintln!("cubarium: wrote {}", path.display());
        Ok(())
    }

    fn draw(&mut self, frame: &Frame) {
        self.buffer.fill(BACKGROUND);
        if self.show_net {
            self.draw_net(frame);
            if self.window.is_key_down(Key::L) {
                self.draw_face_letters();
            }
        }
        if self.show_cube {
            self.draw_cube(frame);
        }
    }

    /// The unfolded net: each face a `64 · scale` square, separators left as background.
    fn draw_net(&mut self, frame: &Frame) {
        let s = self.scale;
        for row in 0..NET_ROWS {
            for col in 0..NET_COLS {
                let Some(face) = face_at(col, row) else { continue };
                let (ox, oy) = net_origin(face, s, SEPARATOR);
                for y in 0..FACE_SIZE {
                    for x in 0..FACE_SIZE {
                        let rgb = frame.get(face, x, y);
                        let c = pack(rgb);
                        for dy in 0..s {
                            let wy = oy + y * s + dy;
                            let base = wy * self.width + ox + x * s;
                            self.buffer[base..base + s].fill(c);
                        }
                    }
                }
            }
        }
    }

    /// The ray-cast cube, sampled nearest-pixel from the same encoded frame.
    fn draw_cube(&mut self, frame: &Frame) {
        if self.hits_stale || self.hits.len() != self.cube_size * self.cube_size {
            self.camera.trace_viewport(self.cube_size, &mut self.hits);
            self.hits_stale = false;
        }
        let ox = self.net_w + SEPARATOR;
        let oy = self.height.saturating_sub(self.cube_size) / 2;
        for py in 0..self.cube_size {
            let wy = oy + py;
            if wy >= self.height {
                break;
            }
            let row = wy * self.width + ox;
            for px in 0..self.cube_size {
                if ox + px >= self.width {
                    break;
                }
                let c = match self.hits[py * self.cube_size + px] {
                    Some((face, x, y)) => pack(frame.get(face, usize::from(x), usize::from(y))),
                    None => BACKGROUND,
                };
                self.buffer[row + px] = c;
            }
        }
    }

    /// Face letters, drawn only outside the face images and only while `l` is held.
    /// The specified separators are one pixel wide, which no glyph fits in, so the
    /// letters go in the net panel's background: the empty top-row cells for the faces
    /// that have one above them, and the strip below the net for the rest.
    fn draw_face_letters(&mut self) {
        let s = self.scale.max(1);
        let (gw, gh) = (3 * s, 5 * s);
        for face in Face::ALL {
            let (col, row) = crate::net::net_cell(face);
            let pitch = FACE_SIZE * s + SEPARATOR;
            let (x, y) = if row > 0 && face_at(col, row - 1).is_none() {
                // The empty cell directly above this face.
                (col * pitch + s, (row - 1) * pitch + pitch - gh - s)
            } else {
                // No empty cell above: use the background strip under the net.
                (col * pitch + s, self.net_h + SEPARATOR + s)
            };
            self.draw_glyph(face.letter(), x, y, gw, gh, 0x0060_6068);
        }
    }

    fn draw_glyph(&mut self, ch: char, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let rows = glyph(ch);
        let (cw, chh) = (w / 3, h / 5);
        if cw == 0 || chh == 0 {
            return;
        }
        for (gy, bits) in rows.iter().enumerate() {
            for gx in 0..3 {
                if bits & (1 << (2 - gx)) == 0 {
                    continue;
                }
                for dy in 0..chh {
                    let wy = y + gy * chh + dy;
                    if wy >= self.height {
                        continue;
                    }
                    for dx in 0..cw {
                        let wx = x + gx * cw + dx;
                        if wx < self.width {
                            self.buffer[wy * self.width + wx] = color;
                        }
                    }
                }
            }
        }
    }
}

impl FrameSink for PreviewSink {
    fn submit(&mut self, frame: &Frame) -> Result<()> {
        self.handle_input(frame);
        self.draw(frame);
        self.window
            .update_with_buffer(&self.buffer, self.width, self.height)
            .context("updating the preview window")?;
        Ok(())
    }

    fn should_quit(&mut self) -> bool {
        self.quit || !self.window.is_open()
    }
}

#[inline]
fn pack(rgb: [u8; 3]) -> u32 {
    (u32::from(rgb[0]) << 16) | (u32::from(rgb[1]) << 8) | u32::from(rgb[2])
}

/// A 3×5 bitmap for the five face letters; each row holds three bits, high bit leftmost.
fn glyph(ch: char) -> [u8; 5] {
    match ch {
        'F' => [0b111, 0b100, 0b111, 0b100, 0b100],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        _ => [0; 5],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_face_letter_has_a_glyph() {
        for face in Face::ALL {
            assert_ne!(glyph(face.letter()), [0; 5], "{face:?}");
        }
        assert_eq!(glyph('?'), [0; 5]);
    }

    #[test]
    fn packing_matches_the_argb_layout_minifb_expects() {
        assert_eq!(pack([0, 0, 0]), 0x0000_0000);
        assert_eq!(pack([255, 255, 255]), 0x00FF_FFFF);
        assert_eq!(pack([18, 52, 86]), 0x0012_3456);
    }

    /// The window layout is computed without opening a window, so it can be checked in
    /// a headless test.
    #[test]
    fn the_layout_puts_the_net_left_of_a_square_cube_viewport() {
        for scale in [1usize, 2, 4, 8] {
            let (net_w, net_h) = net_size(scale, SEPARATOR);
            let cube = CUBE_PIXELS * scale;
            assert_eq!(net_w, 4 * (64 * scale + 1) - 1);
            assert_eq!(net_h, 2 * (64 * scale + 1) - 1);
            let width = net_w + SEPARATOR + cube;
            let height = net_h.max(cube);
            assert!(width > net_w && height >= cube);
            // The cube viewport starts after the net and fits.
            assert_eq!(width - (net_w + SEPARATOR), cube);
        }
    }
}
