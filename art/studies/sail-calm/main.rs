//! Two point-baked authored-motion variants; all sampling/geometry reused unchanged.
#[path = "../aa/aa_comparison.rs"]
#[allow(dead_code)]
mod base;
#[path = "../../../crates/cubarium/src/net.rs"]
#[allow(dead_code)]
mod net;
use base::Clip;
use cubarium_render::{Canvas, Mask, stamp_layers};
use cube_proto::{Face, Frame};
use serde_json::json;
use std::{fs, path::Path};
const VARIANTS: [&str; 3] = ["original", "brace", "settle"];
const STATES: [&str; 4] = ["rest", "move", "feed", "bud"];
fn load(input: &Path, variant: &str, state: usize) -> Clip {
    let mut clip = Clip::load(
        &input
            .join(variant)
            .join(format!("sail-{}.png", STATES[state])),
        [4., 2.4, 2., 5.][state],
    );
    clip.looping = state != 3;
    clip
}
fn delta(a: &Canvas, b: &Canvas) -> f32 {
    Face::ALL
        .into_iter()
        .flat_map(|f| (0..64).flat_map(move |y| (0..64).map(move |x| (f, x, y))))
        .flat_map(|(f, x, y)| {
            a.get(f, x, y)
                .into_iter()
                .zip(b.get(f, x, y))
                .map(|(a, b)| (a - b).abs())
        })
        .fold(0., f32::max)
}
fn pose(clip: &Clip, scene: &str, scale: f64, time: f64) -> Canvas {
    let (at, h, _) = base::placement(scene, 0.37);
    let mut c = Canvas::new();
    stamp_layers(
        &mut c,
        at,
        h,
        &[(clip.pose(time, false), 1.)],
        scale,
        1.,
        Mask::None,
        &mut Vec::new(),
    );
    c
}
fn blend(a: &Clip, b: &Clip, scene: &str, scale: f64, time: f64) -> Canvas {
    let (at, h, _) = base::placement(scene, 1.37 + time);
    let w = (time / 0.3).clamp(0., 1.) as f32;
    let mut c = Canvas::new();
    stamp_layers(
        &mut c,
        at,
        h,
        &[
            (
                a.pose(if a.looping { 1.37 + time } else { a.seconds }, false),
                1. - w,
            ),
            (
                b.pose(if b.looping { 0.31 + time } else { time.max(0.) }, false),
                w,
            ),
        ],
        scale,
        1.,
        Mask::None,
        &mut Vec::new(),
    );
    c
}
fn picture(clips: &[Clip], state: &str, scene: &str, scale: f64, out: &Path) {
    let net = scene != "rooted";
    let (w, h) = if net { (768, 128) } else { (192, 64) };
    let cols = 12;
    let mut sheet = vec![0; w * cols * h * 30 * 3];
    for frame in 0..360 {
        for (variant, clip) in clips.iter().enumerate() {
            let mut image = Frame::black();
            base::render_scaled(clip, scene, frame as f64 / 60., false, Some(scale))
                .encode(&mut image);
            let mut rgb = Vec::new();
            if net {
                crate::net::net_rgb8(&image, &mut rgb);
            }
            for y in 0..h {
                for x in 0..w / 3 {
                    let dest = (((frame / cols) * h + y) * w * cols
                        + (frame % cols) * w
                        + variant * w / 3
                        + x)
                        * 3;
                    if net {
                        sheet[dest..dest + 3]
                            .copy_from_slice(&rgb[(y * 256 + x) * 3..(y * 256 + x) * 3 + 3]);
                    } else {
                        sheet[dest..dest + 3].copy_from_slice(&image.get(Face::Front, x, y));
                    }
                }
            }
        }
    }
    base::png(
        &out.join(format!("{state}-{scale}-{scene}.png")),
        w * cols,
        h * 30,
        &sheet,
    );
}
fn source_sheet(clips: &[Clip], state: &str, out: &Path) {
    let mut rgb = vec![0; 384 * 72 * 3];
    for (row, clip) in clips.iter().enumerate() {
        for k in 0..16 {
            let time = k as f64 * clip.seconds / if clip.looping { 16. } else { 15. };
            let mut frame = Frame::black();
            pose(clip, "rooted", 1., time).encode(&mut frame);
            for y in 0..24 {
                for x in 0..24 {
                    let dst = ((row * 24 + y) * 384 + k * 24 + x) * 3;
                    rgb[dst..dst + 3].copy_from_slice(&frame.get(Face::Front, 20 + x, 20 + y));
                }
            }
        }
    }
    base::png(
        &out.join(format!("source-{state}-native.png")),
        384,
        72,
        &rgb,
    );
    let mut large = vec![0; 1536 * 288 * 3];
    for y in 0..288 {
        for x in 0..1536 {
            let d = (y * 1536 + x) * 3;
            let s = ((y / 4) * 384 + x / 4) * 3;
            large[d..d + 3].copy_from_slice(&rgb[s..s + 3]);
        }
    }
    base::png(
        &out.join(format!("source-{state}-4x.png")),
        1536,
        288,
        &large,
    );
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    assert_eq!(args.len(), 3, "BAKE NEW_OUTPUT");
    let input = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir(out).unwrap();
    let meta: serde_json::Value =
        serde_json::from_slice(&fs::read(input.join("cases.json")).unwrap()).unwrap();
    assert_eq!(meta["passed"], true);
    let clips = STATES
        .iter()
        .enumerate()
        .map(|(i, _)| {
            VARIANTS
                .iter()
                .map(|v| load(input, v, i))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut measures = Vec::new();
    let mut checks = 0;
    for (state, set) in clips.iter().enumerate() {
        source_sheet(set, STATES[state], out);
        for scale in [1., 0.6] {
            for scene in ["rooted", "translated", "seam", "rim", "vertex", "quiet"] {
                let results = set
                    .iter()
                    .map(|clip| base::measure_scaled(clip, scene, Some(scale)))
                    .collect::<Vec<_>>();
                if state == 1 || state == 3 {
                    assert_eq!(results[0], results[1]);
                    assert_eq!(results[0], results[2]);
                }
                for clip in set {
                    assert!(
                        delta(
                            &pose(clip, scene, scale, clip.seconds - 1e-7),
                            &pose(clip, scene, scale, clip.seconds + 1e-7)
                        ) < 1e-5
                    );
                    checks += 1;
                }
                measures.push(
                    json!({"state":STATES[state],"scale":scale,"scene":scene,"variants":results}),
                );
            }
            for scene in ["rooted", "seam", "rim"] {
                picture(set, STATES[state], scene, scale, out);
            }
        }
    }
    for from in 0..4 {
        for to in 0..4 {
            if from == to {
                continue;
            }
            for variant in 0..3 {
                for scale in [1., 0.6] {
                    for scene in ["rooted", "seam", "rim"] {
                        for edge in [0., 0.3] {
                            assert!(
                                delta(
                                    &blend(
                                        &clips[from][variant],
                                        &clips[to][variant],
                                        scene,
                                        scale,
                                        edge - 1e-7
                                    ),
                                    &blend(
                                        &clips[from][variant],
                                        &clips[to][variant],
                                        scene,
                                        scale,
                                        edge + 1e-7
                                    )
                                ) < 1e-5
                            );
                            checks += 1;
                        }
                    }
                }
            }
        }
    }
    // Source fin definition is measured outside the unchanged actual body/bud partition.
    let mut source = Vec::new();
    for (state, set) in clips.iter().enumerate() {
        for (variant, clip) in set.iter().enumerate() {
            let body = load_body(input, VARIANTS[variant], state);
            let mut rows = Vec::new();
            for (i, sprite) in clip.sprites.iter().enumerate() {
                let mut solid = 0;
                let mut alpha = 0.;
                for y in 0..16 {
                    for x in 0..16 {
                        if body.sprites[i].texel(x, y)[3] == 0. {
                            let a = sprite.texel(x, y)[3];
                            alpha += a;
                            if a >= 0.9 {
                                solid += 1
                            }
                        }
                    }
                }
                rows.push(json!({"solid_fin_pixels":solid,"fin_alpha":alpha}));
            }
            source.push(json!({"state":STATES[state],"variant":VARIANTS[variant],"frames":rows}));
        }
    }
    fs::write(out.join("measurements.json"),serde_json::to_vec_pretty(&json!({"source":"renderer frozen3147775; current sail body-hold source; no production edits","variants":VARIANTS,"seconds":6,"fps":60,"endpoint_checks":checks,"measurements":measures,"source_fin_definition":source})).unwrap()).unwrap();
    fs::write(out.join("viewer.html"), include_str!("viewer.html")).unwrap();
}
fn load_body(input: &Path, variant: &str, state: usize) -> Clip {
    Clip::load(
        &input
            .join(variant)
            .join(format!("sail-{}-body.png", STATES[state])),
        [4., 2.4, 2., 5.][state],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../captures/sail-calm-2026-09-13-bake")
    }
    fn same(a: &cubarium_render::Sprite, b: &cubarium_render::Sprite) -> bool {
        (0..16).all(|y| (0..16).all(|x| a.texel(x, y) == b.texel(x, y)))
    }
    #[test]
    fn only_rest_feed_fins_change_and_all_stamps_stay_inside_existing_radius() {
        let p = input();
        for state in 0..4 {
            let old = load(&p, "original", state);
            for v in ["brace", "settle"] {
                let c = load(&p, v, state);
                for (i, s) in c.sprites.iter().enumerate() {
                    assert!(s.extent() <= 9.);
                    if state == 1 || state == 3 {
                        assert!(same(s, &old.sprites[i]));
                    }
                }
            }
        }
    }
    #[test]
    fn brace_has_native_quiet_interval_then_visible_adjustment_and_feed_body_still_moves() {
        let p = input();
        let rest = load(&p, "brace", 0);
        for s in &rest.sprites[..=10] {
            assert!(same(s, &rest.sprites[0]));
        }
        assert!(!same(&rest.sprites[12], &rest.sprites[0]));
        let feed = load(&p, "brace", 2);
        assert!(feed.sprites.iter().any(|s| !same(s, &feed.sprites[0])));
        // Explicit rejected alternative evidence: its authored rest hinge is subpixel-invisible.
        let settle = load(&p, "settle", 0);
        assert!(settle.sprites.iter().all(|s| same(s, &settle.sprites[0])));
    }
}
