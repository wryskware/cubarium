//! One fin-only coverage candidate; all other sail layers retain point sampling.
#[path = "aa_comparison.rs"]
#[allow(dead_code)]
mod base;
use base::Clip;
use cubarium_render::{Canvas, Mask, stamp_layers};
use cube_proto::{Face, Frame};
use serde_json::{Value, json};
use std::{fs, path::Path};

fn blend(from: &Clip, to: &Clip, scene: &str, scale: f64, t: f64) -> Canvas {
    let (anchor, heading, _) = base::placement(scene, 1.37 + t);
    let a = from.pose(if from.looping { 1.37 + t } else { from.seconds }, false);
    let b = to.pose(if to.looping { 0.31 + t } else { t.max(0.) }, false);
    let w = (t / 0.3).clamp(0., 1.) as f32;
    let mut canvas = Canvas::new();
    stamp_layers(
        &mut canvas,
        anchor,
        heading,
        &[(a, 1. - w), (b, w)],
        scale,
        1.,
        Mask::None,
        &mut Vec::new(),
    );
    canvas
}

fn max_channel_difference(a: &Canvas, b: &Canvas) -> f32 {
    let mut maximum = 0.0f32;
    for face in Face::ALL {
        for y in 0..64 {
            for x in 0..64 {
                for (u, v) in a.get(face, x, y).into_iter().zip(b.get(face, x, y)) {
                    maximum = maximum.max((u - v).abs());
                }
            }
        }
    }
    maximum
}

fn endpoints(clip: &Clip, scene: &str, scale: f64) -> Value {
    // Fixed placement isolates the clip boundary from motion of the anchor.
    let draw = |time| {
        let (anchor, heading, _) = base::placement(scene, 0.37);
        let mut canvas = Canvas::new();
        stamp_layers(
            &mut canvas,
            anchor,
            heading,
            &[(clip.pose(time, false), 1.)],
            scale,
            1.,
            Mask::None,
            &mut Vec::new(),
        );
        canvas
    };
    let before = draw(clip.seconds - 1e-7);
    let at = draw(clip.seconds);
    let after = draw(clip.seconds + 1e-7);
    let delta = max_channel_difference(&before, &after);
    assert!(delta < 1e-5, "loop/clamp endpoint delta {delta}");
    if clip.looping {
        assert_eq!(max_channel_difference(&at, &draw(0.)), 0.);
    } else {
        assert_eq!(max_channel_difference(&at, &draw(clip.seconds + 100.)), 0.);
    }
    json!({"max_channel_at_epsilon":delta,"exact_endpoint":true})
}

fn transition_sheet(from: &[Clip; 2], to: &[Clip; 2], scale: f64, path: &Path) {
    let mut sheet = vec![0; 1920 * 1536 * 3];
    for frame in 0..360 {
        for mode in 0..2 {
            let mut bytes = Frame::default();
            blend(
                &from[mode],
                &to[mode],
                "rooted",
                scale,
                frame as f64 / 60. - 1.,
            )
            .encode(&mut bytes);
            for y in 0..64 {
                for x in 0..64 {
                    let dest =
                        (((frame / 15) * 64 + y) * 1920 + (frame % 15) * 128 + mode * 64 + x) * 3;
                    sheet[dest..dest + 3].copy_from_slice(&bytes.get(Face::Front, x, y));
                }
            }
        }
    }
    base::png(path, 1920, 1536, &sheet);
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3, "sail_comparison BAKEDIR NEW_OUTPUTDIR");
    let input = Path::new(&args[1]);
    let output = Path::new(&args[2]);
    fs::create_dir(output).unwrap();
    let metadata: Value =
        serde_json::from_slice(&fs::read(input.join("cases.json")).unwrap()).unwrap();
    assert_eq!(metadata["passed"], true, "bake validation failed");
    let mut clips = Vec::new();
    let mut result = Vec::new();
    for c in metadata["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let seconds = c["seconds"].as_f64().unwrap();
        let mut pair = [
            Clip::load(&input.join(format!("{name}-nearest.png")), seconds),
            Clip::load(&input.join(format!("{name}-fin4.png")), seconds),
        ];
        for clip in &mut pair {
            clip.looping = c["loop"].as_bool().unwrap();
        }
        let mut scenes = Vec::new();
        for (scale, label) in [(1., "adult"), (0.6, "juvenile")] {
            for scene in ["rooted", "translated", "seam", "rim", "vertex", "quiet"] {
                scenes.push(json!({"scene":scene,"scale":scale,"nearest":base::measure_scaled(&pair[0],scene,Some(scale)),
                    "fin4":base::measure_scaled(&pair[1],scene,Some(scale)),
                    "endpoints_nearest_then_fin4":[endpoints(&pair[0],scene,scale),endpoints(&pair[1],scene,scale)]}));
            }
            base::pictures_scaled(&pair, &format!("{name}-{label}"), output, Some(scale));
        }
        result.push(json!({"case":c,"scenes":scenes,"timing_rooted":base::benchmark(&pair,"rooted"),
            "timing_juvenile":base::benchmark(&pair,"juvenile"),"timing_seam":base::benchmark(&pair,"seam"),
            "max_extent_nearest_then_fin4":pair.iter().map(|p|p.sprites.iter().map(|s|s.extent()).fold(0.,f64::max)).collect::<Vec<_>>()}));
        clips.push(pair);
        eprintln!("measured {name}");
    }
    let names = ["rest", "move", "feed", "bud"];
    let mut transitions = Vec::new();
    for from in 0..4 {
        for to in 0..4 {
            if from == to {
                continue;
            }
            for (scale, label) in [(1., "adult"), (0.6, "juvenile")] {
                for scene in ["rooted", "seam", "rim"] {
                    for mode in 0..2 {
                        for edge in [0., 0.3] {
                            let a = blend(
                                &clips[from][mode],
                                &clips[to][mode],
                                scene,
                                scale,
                                edge - 1e-7,
                            );
                            let b = blend(
                                &clips[from][mode],
                                &clips[to][mode],
                                scene,
                                scale,
                                edge + 1e-7,
                            );
                            let delta = max_channel_difference(&a, &b);
                            assert!(
                                delta < 1e-5,
                                "{}→{} {scene} scale{scale} mode{mode} edge{edge}: {delta}",
                                names[from],
                                names[to]
                            );
                            transitions.push(json!({"from":names[from],"to":names[to],"scale":scale,"scene":scene,"mode":mode,"edge":edge,"max_channel_at_epsilon":delta}));
                        }
                    }
                }
                transition_sheet(
                    &clips[from],
                    &clips[to],
                    scale,
                    &output.join(format!(
                        "sail-{}-to-{}-{label}-rooted-motion.png",
                        names[from], names[to]
                    )),
                );
            }
        }
    }
    let candidate = metadata["candidate"].as_str().unwrap_or("fin4, point body and bud");
    fs::write(output.join("measurements.json"),serde_json::to_vec_pretty(&json!({"fps":60,"frames_per_sequence":360,"candidate":candidate, "cases":result,"transition_endpoints":transitions})).unwrap()).unwrap();
    let mut viewer = include_str!("sail_viewer.html").to_owned();
    if candidate == "stable-body-plus-fin4" {
        viewer = viewer.replace("unchanged point-sampled body and bud", "point-sampled body and bud; move body held at scale 1 instead of its tiny squash")
            .replace("Sail: coverage on fins, crisp body", "Sail: stable moving body + fin coverage")
            .replace("Current / fin coverage", "Original / stable body + fin coverage");
    }
    fs::write(output.join("viewer.html"), viewer).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_render::Sprite;
    use cubarium_surface::Vec2;
    #[test]
    fn nonlooping_clip_holds_its_last_pose_instead_of_wrapping() {
        let a = Sprite::from_rgba(1, 1, Vec2::new(0.5, 0.5), &[255, 0, 0, 255]).unwrap();
        let b = Sprite::from_rgba(1, 1, Vec2::new(0.5, 0.5), &[0, 255, 0, 255]).unwrap();
        let clip = Clip {
            sprites: vec![a.clone(), b.clone()],
            white: vec![a, b],
            seconds: 5.,
            looping: false,
        };
        let pose = clip.pose(100., false);
        assert!(std::ptr::eq(pose.first, &clip.sprites[1]));
        assert_eq!(pose.mix, 0.);
    }
}
