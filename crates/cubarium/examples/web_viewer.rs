//! Drive the web sink on its own, with no scene and no CLI wiring.
//!
//!     cargo run -p cubarium --example web_viewer -- 8787
//!
//! Serves an orientation test pattern at 60 fps so the page's face mapping can be
//! checked against `cubarium_surface::face_frame` by eye: each face gets a distinct
//! base colour, a ramp (red grows with chart +u, green with chart +v) and bright
//! corner markers — white at chart (0, 0), red at (63, 0), green at (0, 63) — plus a
//! scan line sweeping down +v so the tick is visibly advancing.
//!
//! The expected corners are the table in `sink/web/index.html`: Front's white marker
//! sits on the cube corner (-1, +1, +1), where Left's red marker and Top's green
//! marker meet it.

use std::time::{Duration, Instant};

use cube_proto::{FACE_SIZE, Face, Frame};
use cubarium::sink::{FrameSink, WebSink};

const BASE: [[u8; 3]; 5] = [
    [58, 10, 14],  // Front  red
    [10, 58, 18],  // Right  green
    [14, 18, 66],  // Back   blue
    [58, 48, 10],  // Left   amber
    [50, 12, 58],  // Top    violet
];

/// A 3x3 block anchored at the named chart corner, matching the page's own marker.
fn mark(frame: &mut Frame, face: Face, x0: usize, y0: usize, rgb: [u8; 3]) {
    for dy in 0..3 {
        for dx in 0..3 {
            let x = if x0 == 0 { dx } else { x0 - dx };
            let y = if y0 == 0 { dy } else { y0 - dy };
            frame.set(face, x, y, rgb);
        }
    }
}

fn pattern(tick: u64) -> Frame {
    let mut frame = Frame::black();
    let scan = (tick % FACE_SIZE as u64) as usize;
    for (i, face) in Face::ALL.into_iter().enumerate() {
        let base = BASE[i];
        for y in 0..FACE_SIZE {
            for x in 0..FACE_SIZE {
                let lit = y == scan;
                frame.set(
                    face,
                    x,
                    y,
                    [
                        base[0].saturating_add(x as u8).saturating_add(if lit { 90 } else { 0 }),
                        base[1].saturating_add(y as u8).saturating_add(if lit { 90 } else { 0 }),
                        base[2].saturating_add(if lit { 90 } else { 0 }),
                    ],
                );
            }
        }
        mark(&mut frame, face, 0, 0, [255, 255, 255]);
        mark(&mut frame, face, 63, 0, [255, 40, 40]);
        mark(&mut frame, face, 0, 63, [40, 255, 90]);
    }
    frame
}

fn main() -> anyhow::Result<()> {
    let port: u16 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(8787);
    let mut sink = WebSink::new(port)?;
    println!("cubarium web viewer: {}  (ctrl-c to stop)", sink.url());

    let frame_time = Duration::from_micros(16_667);
    let mut tick = 0u64;
    loop {
        let t0 = Instant::now();
        sink.submit(&pattern(tick))?;
        tick += 1;
        if let Some(rest) = frame_time.checked_sub(t0.elapsed()) {
            std::thread::sleep(rest);
        }
    }
}
