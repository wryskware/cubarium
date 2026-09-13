//! Isolated authored-coverage study; uses production sprite/surface code, not the host.
use cubarium_render::{Canvas, Mask, Pose, Sprite, stamp_pose};
use cubarium_surface::{SurfacePoint, Vec2, travel};
use cube_proto::{Face, Frame};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::Path,
    time::Instant,
};
#[path = "../../../crates/cubarium/src/net.rs"]
#[allow(dead_code)]
mod net;

const FRAMES: usize = 360;
const COLS: usize = 15;

struct Clip {
    sprites: Vec<Sprite>,
    white: Vec<Sprite>,
    seconds: f64,
}
impl Clip {
    fn load(path: &Path, seconds: f64) -> Self {
        let mut reader = png::Decoder::new(BufReader::new(File::open(path).unwrap()))
            .read_info()
            .unwrap();
        let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut bytes).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(info.height, 16);
        assert_eq!(info.width % 16, 0);
        let mut sprites = Vec::new();
        let mut white = Vec::new();
        for frame in 0..info.width as usize / 16 {
            let mut tile = Vec::new();
            for y in 0..16 {
                let start = (y * info.width as usize + frame * 16) * 4;
                tile.extend_from_slice(&bytes[start..start + 64]);
            }
            sprites.push(Sprite::from_rgba(16, 16, Vec2::new(8., 8.), &tile).unwrap());
            for p in tile.chunks_exact_mut(4) {
                p[..3].fill(255);
            }
            white.push(Sprite::from_rgba(16, 16, Vec2::new(8., 8.), &tile).unwrap());
        }
        Self {
            sprites,
            white,
            seconds,
        }
    }
    fn pose(&self, time: f64, white: bool) -> Pose<'_> {
        // Same bracketing schedule for both bakes. This is not a separate oracle for Clip::sample.
        let frames = if white { &self.white } else { &self.sprites };
        let position = (time / self.seconds).rem_euclid(1.) * frames.len() as f64;
        let i = position.floor() as usize;
        Pose {
            first: &frames[i],
            second: &frames[(i + 1) % frames.len()],
            mix: position.fract() as f32,
        }
    }
}

fn png(path: &Path, width: usize, height: usize, bytes: &[u8]) {
    let mut enc = png::Encoder::new(
        BufWriter::new(File::create(path).unwrap()),
        width as u32,
        height as u32,
    );
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(bytes).unwrap();
}

fn placement(scene: &str, time: f64) -> (SurfacePoint, Vec2, f64) {
    let t = time * std::f64::consts::TAU / 6.;
    let (start, displacement, heading, scale) = match scene {
        "rooted" | "quiet" => (
            SurfacePoint::new(Face::Front, 32., 32.),
            Vec2::new(0., 0.),
            Vec2::new(1., 0.),
            1.,
        ),
        "translated" => (
            SurfacePoint::new(Face::Front, 32., 32.),
            Vec2::new(2.3 * t.sin(), 0.7 * t.cos()),
            Vec2::new((0.18 * t.sin()).cos(), (0.18 * t.sin()).sin()),
            1.,
        ),
        "juvenile" => (
            SurfacePoint::new(Face::Front, 32., 32.),
            Vec2::new(2.3 * t.sin(), 0.7 * t.cos()),
            Vec2::new(1., 0.),
            0.6,
        ),
        "seam" => (
            SurfacePoint::new(Face::Front, 63., 32.),
            Vec2::new(2.3 * t.sin(), 0.),
            Vec2::new(1., 0.),
            1.,
        ),
        "rim" => (
            SurfacePoint::new(Face::Front, 32., 63.),
            Vec2::new(0., 2.3 * t.sin()),
            Vec2::new(1., 0.),
            1.,
        ),
        "vertex" => (
            SurfacePoint::new(Face::Top, 0.3, 0.4),
            Vec2::new(0., 0.),
            Vec2::new(0.8, 0.6),
            1.,
        ),
        _ => panic!("unknown scene"),
    };
    let moved = travel(start, displacement);
    assert!(!moved.fallback);
    (moved.end, moved.map.apply(heading), scale)
}

fn render(clip: &Clip, scene: &str, time: f64, white: bool) -> Canvas {
    let (point, heading, scale) = placement(scene, time);
    let mut canvas = Canvas::new();
    stamp_pose(
        &mut canvas,
        point,
        heading,
        clip.pose(if scene == "quiet" { 0.37 } else { time }, white),
        scale,
        1.,
        Mask::None,
        &mut Vec::new(),
    );
    canvas
}

fn linear(canvas: &Canvas) -> Vec<f64> {
    let mut values = Vec::with_capacity(64 * 64 * 5);
    for face in Face::ALL {
        for y in 0..64 {
            for x in 0..64 {
                let p = canvas.get(face, x, y);
                assert!(p.iter().all(|v| v.is_finite() && *v >= 0. && *v <= 1.00001));
                values.push(0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64);
            }
        }
    }
    values
}

fn stats(values: &[f64]) -> Value {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    json!({"mean":mean, "min":values.iter().copied().fold(f64::INFINITY, f64::min),
        "max":values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        "stddev":(values.iter().map(|v| (v-mean).powi(2)).sum::<f64>() / values.len() as f64).sqrt()})
}

fn measure(clip: &Clip, scene: &str) -> Value {
    let mut previous: Option<Vec<f64>> = None;
    let mut previous_delta: Option<Vec<f64>> = None;
    let mut deltas = Vec::new();
    let mut acceleration = Vec::new();
    let mut energy = Vec::new();
    let mut areas = Vec::new();
    let mut opaque_areas = Vec::new();
    let mut root_coverage = Vec::new();
    let mut peaks = Vec::new();
    for frame in 0..FRAMES {
        let time = frame as f64 / 60.;
        let canvas = render(clip, scene, time, false);
        let current = linear(&canvas);
        energy.push(current.iter().sum());
        peaks.push(current.iter().copied().fold(0., f64::max));
        let alpha = render(clip, scene, time, true);
        let a = linear(&alpha);
        areas.push(a.iter().sum());
        opaque_areas.push(a.iter().filter(|v| **v >= 0.5).count() as f64);
        // Bottom two source rows plus bilinear support at the fixed native root.
        root_coverage.push(
            (38..41)
                .flat_map(|y| (24..41).map(move |x| (x, y)))
                .map(|(x, y)| alpha.get(Face::Front, x, y)[0] as f64)
                .sum(),
        );
        if let Some(prev) = previous {
            let delta: Vec<_> = current.iter().zip(prev).map(|(a, b)| a - b).collect();
            deltas.push(delta.iter().map(|x| x.abs()).sum());
            if let Some(old) = previous_delta {
                acceleration.push(delta.iter().zip(old).map(|(a, b)| (a - b).abs()).sum());
            }
            previous_delta = Some(delta);
        }
        previous = Some(current);
    }
    if scene == "quiet" {
        assert!(deltas.iter().all(|v| *v == 0.));
        let mut a = Frame::default();
        let mut b = Frame::default();
        render(clip, scene, 0., false).encode(&mut a);
        render(clip, scene, 5., false).encode(&mut b);
        assert_eq!(a.as_bytes(), b.as_bytes());
    }
    assert!(areas.iter().all(|a| *a > 0.));
    json!({"linear_luma_sum":stats(&energy), "alpha_area":stats(&areas), "alpha_ge_half_pixels":stats(&opaque_areas),
        "peak_luma":stats(&peaks), "frame_l1":stats(&deltas), "second_difference_l1":stats(&acceleration),
        "fixed_front_root_band_alpha":stats(&root_coverage)})
}

fn benchmark(clips: &[Clip; 2], scene: &str) -> Value {
    let mut timings: [Vec<f64>; 2] = [vec![], vec![]];
    let mut canvas = Canvas::new();
    let mut scratch = Vec::new();
    // Paired alternating batches; timing excludes clearing, allocation of the canvas, encoding and file IO.
    for round in 0..7 {
        for j in 0..2 {
            let mode = (round + j) % 2;
            let mut elapsed = 0.;
            for frame in 0..FRAMES {
                canvas.clear();
                let time = frame as f64 / 60.;
                let (point, heading, scale) = placement(scene, time);
                let pose = clips[mode].pose(time, false);
                let begin = Instant::now();
                stamp_pose(
                    &mut canvas,
                    point,
                    heading,
                    pose,
                    scale,
                    1.,
                    Mask::None,
                    &mut scratch,
                );
                elapsed += begin.elapsed().as_secs_f64();
                std::hint::black_box(&canvas);
            }
            if round > 0 {
                timings[mode].push(elapsed * 1e6 / FRAMES as f64);
            }
        }
    }
    json!({"nearest_us_per_stamp":stats(&timings[0]), "coverage4_us_per_stamp":stats(&timings[1])})
}

fn pictures(clips: &[Clip; 2], name: &str, out: &Path) {
    for scene in [
        "rooted",
        "translated",
        "juvenile",
        "seam",
        "rim",
        "vertex",
        "quiet",
    ] {
        let net_view = matches!(scene, "seam" | "rim" | "vertex");
        let (cell_width, cell_height) = if net_view { (512, 128) } else { (128, 64) };
        let width = COLS * cell_width;
        let mut sheet = vec![0; width * (FRAMES / COLS) * cell_height * 3];
        for frame in 0..FRAMES {
            for (mode, clip) in clips.iter().enumerate() {
                let mut bytes = Frame::default();
                render(clip, scene, frame as f64 / 60., false).encode(&mut bytes);
                let mut net_bytes = Vec::new();
                if net_view {
                    net::net_rgb8(&bytes, &mut net_bytes);
                }
                for y in 0..cell_height {
                    for x in 0..cell_width / 2 {
                        let i = (((frame / COLS) * cell_height + y) * width
                            + (frame % COLS) * cell_width
                            + mode * cell_width / 2
                            + x)
                            * 3;
                        if net_view {
                            let src = (y * 256 + x) * 3;
                            sheet[i..i + 3].copy_from_slice(&net_bytes[src..src + 3]);
                        } else {
                            sheet[i..i + 3].copy_from_slice(&bytes.get(Face::Front, x, y));
                        }
                    }
                }
            }
        }
        png(
            &out.join(format!("{name}-{scene}-motion.png")),
            width,
            FRAMES / COLS * cell_height,
            &sheet,
        );
    }
    let mut boundary = vec![0; 512 * 128 * 3 * 3];
    for (row, scene) in ["seam", "rim", "vertex"].iter().enumerate() {
        for (mode, clip) in clips.iter().enumerate() {
            let mut frame = Frame::default();
            render(clip, scene, 1.1, false).encode(&mut frame);
            let mut net = Vec::new();
            net::net_rgb8(&frame, &mut net);
            for y in 0..128 {
                let dst = ((row * 128 + y) * 512 + mode * 256) * 3;
                boundary[dst..dst + 256 * 3].copy_from_slice(&net[y * 256 * 3..(y + 1) * 256 * 3]);
            }
        }
    }
    png(
        &out.join(format!("{name}-boundaries.png")),
        512,
        384,
        &boundary,
    );
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3, "aa_comparison BAKEDIR NEW_OUTPUTDIR");
    let input = Path::new(&args[1]);
    let output = Path::new(&args[2]);
    fs::create_dir(output).expect("requires a new output directory");
    let cases: Vec<Value> =
        serde_json::from_slice(&fs::read(input.join("cases.json")).unwrap()).unwrap();
    let mut result = Vec::new();
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let seconds = case["seconds"].as_f64().unwrap();
        let clips = [
            Clip::load(&input.join(format!("{name}-nearest.png")), seconds),
            Clip::load(&input.join(format!("{name}-coverage4.png")), seconds),
        ];
        let mut scenes = Vec::new();
        for scene in [
            "rooted",
            "translated",
            "juvenile",
            "seam",
            "rim",
            "vertex",
            "quiet",
        ] {
            scenes.push(json!({"scene":scene,"nearest":measure(&clips[0],scene),"coverage4":measure(&clips[1],scene),"timing":benchmark(&clips,scene)}));
        }
        pictures(&clips, name, output);
        let footprint: Vec<_> = clips.iter().map(|c| json!({"max_extent":c.sprites.iter().map(Sprite::extent).fold(0.,f64::max),
            "min_bend_headroom_root1_5_length13":c.sprites.iter().map(|s| s.bend_headroom(1.5,13.,0.)).fold(f64::INFINITY,f64::min)})).collect();
        result.push(
            json!({"case":case,"footprint_nearest_then_coverage4":footprint,"scenes":scenes}),
        );
        eprintln!("measured {name}");
    }
    fs::write(
        output.join("measurements.json"),
        serde_json::to_vec_pretty(&json!({"fps":60,"frames":FRAMES,"seconds":6,"cases":result}))
            .unwrap(),
    )
    .unwrap();
    fs::write(output.join("viewer.html"), include_str!("viewer.html")).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Clip {
        let mut bytes = vec![0; 16 * 16 * 4];
        for y in 3..15 {
            bytes[(y * 16 + 8) * 4..(y * 16 + 8) * 4 + 4].copy_from_slice(&[80, 180, 220, 255]);
        }
        let sprite = Sprite::from_rgba(16, 16, Vec2::new(8., 8.), &bytes).unwrap();
        Clip {
            sprites: vec![sprite.clone(), sprite.clone()],
            white: vec![sprite.clone(), sprite],
            seconds: 3.,
        }
    }
    #[test]
    fn held_pose_remains_byte_identical() {
        let clip = fixture();
        let mut first = Frame::default();
        let mut last = Frame::default();
        render(&clip, "quiet", 0., false).encode(&mut first);
        render(&clip, "quiet", 100., false).encode(&mut last);
        assert_eq!(first.as_bytes(), last.as_bytes());
    }
    #[test]
    fn seam_path_really_crosses_and_rim_reflects() {
        assert_eq!(placement("seam", 0.).0.face, Face::Front);
        assert_eq!(placement("seam", 1.5).0.face, Face::Right);
        assert!(placement("rim", 1.5).0.v < 63.);
    }
    #[test]
    fn boundary_studies_paint_finite_nonzero_pixels() {
        let clip = fixture();
        for scene in ["seam", "rim", "vertex"] {
            for t in [0., 1.5, 3., 4.5] {
                assert!(linear(&render(&clip, scene, t, false)).iter().sum::<f64>() > 0.);
            }
        }
    }
}
