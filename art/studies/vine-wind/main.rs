use cubarium::{
    art::{ArtPack, Clip},
    art_present::*,
    sink::png::write_net_png,
};
use cubarium_render::{
    Bend, Canvas, Mask, Sprite, stamp_layers_bent, stamp_layers_bent_with_radius,
};
use cube_proto::{Face, Frame};
use serde_json::json;
use std::{fs, path::Path};

fn pack(path: &str, retile: bool) -> ArtPack {
    let mut art = ArtPack::load(Path::new(path)).unwrap();
    if retile {
        let vine = art.tall.iter_mut().find(|p| p.name == VINE_PLANT).unwrap();
        assert_eq!(trunk_strip(vine), (TALL_STRIP_FLOOR, TALL_STRIP_TOP));
        vine.cap = Some(Clip {
            seconds: vine.trunk.seconds,
            looping: vine.trunk.looping,
            frames: vine
                .trunk
                .frames
                .iter()
                .map(|frame| {
                    let pixels = (0..16)
                        .flat_map(|y| {
                            (0..16).map(move |x| {
                                if (4..8).contains(&y) {
                                    frame.texel(x, y)
                                } else {
                                    [0.; 4]
                                }
                            })
                        })
                        .collect();
                    Sprite::from_premultiplied(16, 16, frame.pivot(), pixels).unwrap()
                })
                .collect(),
        });
    }
    art
}
fn draw(art: &ArtPack, column: &TallColumn, height: f64, seconds: f64, amplitude: f64) -> Canvas {
    let mut c = Canvas::new();
    draw_column(
        &mut c,
        column,
        height,
        art.tall_plant(TALL_PLANTS[column.pick]).unwrap(),
        art.tall_plant(VINE_PLANT),
        seconds,
        amplitude,
        &mut Vec::new(),
    );
    c
}
fn bytes(c: &Canvas) -> Frame {
    let mut f = Frame::black();
    c.encode(&mut f);
    f
}
fn difference(a: &Canvas, b: &Canvas) -> (usize, f32) {
    let mut count = 0;
    let mut max = 0.0f32;
    for face in Face::ALL {
        for y in 0..64 {
            for x in 0..64 {
                let d = a
                    .get(face, x, y)
                    .into_iter()
                    .zip(b.get(face, x, y))
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0f32, f32::max);
                if d > 0. {
                    count += 1;
                }
                max = max.max(d);
            }
        }
    }
    (count, max)
}
fn verify(old: &ArtPack, clear: &ArtPack, new: &ArtPack) -> serde_json::Value {
    let columns = Face::ALL
        .into_iter()
        .filter(|f| *f != Face::Top)
        .flat_map(|face| {
            [0, 1, 7, 14, 15].into_iter().map(move |cx| TallColumn {
                face,
                cx,
                pick: 0,
                vine: true,
            })
        })
        .collect::<Vec<_>>();
    let mut quiet = 0;
    let mut same_bend = 0;
    let mut rejected = 0;
    let mut worst = 0.0f32;
    for col in &columns {
        for k in 0..=72 {
            let h = k as f64 / 8.;
            let seconds = (k % 24) as f64 * 0.125;
            let a = draw(old, col, h, seconds, 0.);
            let b = draw(new, col, h, seconds, 0.);
            if bytes(&a).as_bytes() != bytes(&b).as_bytes() {
                for f in Face::ALL {
                    for y in 0..64 {
                        for x in 0..64 {
                            if a.get(f, x, y) != b.get(f, x, y) {
                                eprintln!(
                                    "{f:?} {x},{y}: {:?} {:?}",
                                    a.get(f, x, y),
                                    b.get(f, x, y)
                                );
                            }
                        }
                    }
                }
                panic!("quiet mismatch {col:?} height{h}: {:?}", difference(&a, &b));
            }
            quiet += 1;
            worst = worst.max(difference(&a, &b).1);
            if bytes(&a).as_bytes() != bytes(&draw(clear, col, h, seconds, 0.)).as_bytes() {
                rejected += 1;
            }
            for amp in [-0.4, 0.4] {
                let a = draw(old, col, h, seconds, amp);
                let b = draw(new, col, h, seconds, amp);
                let delta = difference(&a, &b).1;
                assert!(
                    delta < 1e-6,
                    "same-bend mismatch {col:?} height{h} amplitude{amp}: {delta}"
                );
                same_bend += 1;
            }
        }
    }
    assert!(
        rejected > 0,
        "naive clear-end candidate must expose its endpoint loss"
    );
    json!({"quiet_exact_frames":quiet,"same_amplitude_comparisons":same_bend,"quiet_max_linear_delta":worst,"naive_endpoint_failures":rejected})
}
fn strict(old: &ArtPack, new: &ArtPack) -> serde_json::Value {
    let mut heights = vec![0., 1e-9, 0.01, 0.5, 8.875, 9.];
    for k in 1..=9 {
        for d in [-1e-7, 0., 1e-7] {
            heights.push(k as f64 + d);
        }
    }
    let mut count = 0;
    let mut roots = 0;
    let mut loops = 0;
    for face in Face::ALL.into_iter().filter(|f| *f != Face::Top) {
        for cx in 0..16 {
            for pick in 0..2 {
                let col = TallColumn {
                    face,
                    cx,
                    pick,
                    vine: true,
                };
                for &h in &heights {
                    for time in [0., 0.0625, 2.9999999, 3., 3.0000001] {
                        let a = draw(old, &col, h, time, 0.);
                        let b = draw(new, &col, h, time, 0.);
                        assert!(
                            bytes(&a).as_bytes() == bytes(&b).as_bytes(),
                            "exact quiet {col:?} {h} {time}"
                        );
                        assert_eq!(
                            difference(&b, &draw(new, &col, h, time, 0.)),
                            (0, 0.),
                            "held draw changed"
                        );
                        count += 1;
                    }
                }
                let budget = tall_bend_budget(new.tall_plant(TALL_PLANTS[pick]).unwrap())
                    .min(tall_bend_budget(new.tall_plant(VINE_PLANT).unwrap()));
                for h in [0.01, 0.5, 1., 7.9999999, 8., 8.0000001, 8.875, 9.] {
                    let calm = draw(new, &col, h, 0., 0.);
                    let windy = draw(new, &col, h, 0., budget.min(0.99));
                    for y in tall_anchor(face, cx, 0).v.ceil() as u8..64 {
                        for x in 0..64 {
                            assert_eq!(
                                calm.get(face, x, y),
                                windy.get(face, x, y),
                                "root moved {col:?} {h} {x},{y}"
                            );
                        }
                    }
                    roots += 1;
                    // The host and vine both have3s authored clips. Wind itself is not3s periodic.
                    let a = draw(new, &col, h, 0., 0.3);
                    let b = draw(new, &col, h, 3., 0.3);
                    assert!(difference(&a, &b).1 < 1e-6, "clip loop changed {col:?} {h}");
                    loops += 1;
                }
            }
        }
    }
    // Every actual candidate trunk frame at low/high placements, subpixel pivots,
    // both wind signs: enlarged query is an observable clipping oracle.
    let vine = new.tall_plant(VINE_PLANT).unwrap();
    let mut support = 0;
    for clip in [&vine.trunk, vine.cap.as_ref().unwrap()] {
        for frame in &clip.frames {
            for base in [-4., 28., 32.] {
                for amplitude in [-1.314171379627988, 1.314171379627988] {
                    let bend = Bend {
                        amplitude,
                        base,
                        root: TALL_BEND_ROOT,
                        length: TALL_BEND_LENGTH,
                    };
                    assert!(
                        amplitude.abs()
                            <= frame.bend_headroom(TALL_BEND_ROOT, TALL_BEND_LENGTH, base)
                    );
                    for face in Face::ALL {
                        for (u, v) in [(0.1, 0.1), (63.9, 0.1), (31.3, 31.7)] {
                            let at = cubarium_surface::SurfacePoint::new(face, u, v);
                            let heading = cubarium_surface::Vec2::new(1., 0.);
                            let pose = cubarium_render::Pose::still(frame);
                            let mut a = Canvas::new();
                            let mut b = Canvas::new();
                            stamp_layers_bent(
                                &mut a,
                                at,
                                heading,
                                &[(pose, 1.)],
                                1.,
                                1.,
                                Mask::None,
                                bend,
                                &mut Vec::new(),
                            );
                            stamp_layers_bent_with_radius(
                                &mut b,
                                at,
                                heading,
                                &[(pose, 1.)],
                                1.,
                                1.,
                                Mask::None,
                                bend,
                                12.,
                                &mut Vec::new(),
                            );
                            assert_eq!(
                                difference(&a, &b),
                                (0, 0.),
                                "clipped support {face:?} {u},{v} base{base}"
                            );
                            support += 1;
                        }
                    }
                }
            }
        }
    }
    json!({"all_slots_both_hosts_quiet_held":count,"larger_query_support_comparisons":support,"root_rows_exact":roots,"authored_three_second_loops":loops})
}
fn measure(old: &ArtPack, new: &ArtPack) -> serde_json::Value {
    let mut result = Vec::new();
    for (name, col) in [
        (
            "interior",
            TallColumn {
                face: Face::Front,
                cx: 10,
                pick: 0,
                vine: true,
            },
        ),
        (
            "vertex",
            TallColumn {
                face: Face::Front,
                cx: 0,
                pick: 0,
                vine: true,
            },
        ),
        (
            "right",
            TallColumn {
                face: Face::Right,
                cx: 15,
                pick: 0,
                vine: true,
            },
        ),
    ] {
        for (label, art) in [("old", old), ("new", new)] {
            let budget = tall_bend_budget(art.tall_plant(TALL_PLANTS[col.pick]).unwrap())
                .min(tall_bend_budget(art.tall_plant(VINE_PLANT).unwrap()));
            let mut luma_sum = 0.;
            let mut area_sum = 0.;
            let mut peak = 0.0f64;
            let mut delta_sum = 0.;
            let mut delta_max = 0.0f64;
            let mut delta_max_frame = 0;
            let mut peak_amp = 0.0f64;
            let mut peak_frame = 0;
            let mut quiet_frames = 0;
            let mut prev: Option<Canvas> = None;
            for frame in 0..1200 {
                let time = 30. + frame as f64 / 60.;
                let amp = tall_amplitude(&col, budget, time);
                if amp.abs() > peak_amp {
                    peak_amp = amp.abs();
                    peak_frame = frame;
                }
                let c = draw(art, &col, 9., time, amp);
                let mut delta = 0.;
                if amp == 0. {
                    assert_eq!(difference(&c, &draw(old, &col, 9., time, 0.)), (0, 0.));
                    quiet_frames += 1;
                }
                for face in Face::ALL {
                    for y in 0..64 {
                        for x in 0..64 {
                            let p = c.get(face, x, y);
                            let l = f64::from(p[0]) * 0.2126
                                + f64::from(p[1]) * 0.7152
                                + f64::from(p[2]) * 0.0722;
                            luma_sum += l;
                            peak = peak.max(l);
                            if l > 0.01 {
                                area_sum += 1.;
                            }
                            if let Some(prev) = &prev {
                                let q = prev.get(face, x, y);
                                delta += p
                                    .into_iter()
                                    .zip(q)
                                    .map(|(p, q)| f64::from((p - q).abs()))
                                    .sum::<f64>();
                            }
                        }
                    }
                }
                delta_sum += delta;
                if delta > delta_max {
                    delta_max = delta;
                    delta_max_frame = frame;
                }
                prev = Some(c);
            }
            let mut timings = Vec::new();
            for _ in 0..5 {
                let start = std::time::Instant::now();
                for frame in 0..1000 {
                    let t = 30. + frame as f64 / 60.;
                    std::hint::black_box(draw(art, &col, 9., t, tall_amplitude(&col, budget, t)));
                }
                timings.push(start.elapsed().as_secs_f64() * 1000. / 1000.);
            }
            timings.sort_by(f64::total_cmp);
            result.push(json!({"case":name,"variant":label,"mean_luma_sum":luma_sum/1200.,"mean_pixels_luma_gt_01":area_sum/1200.,"peak_pixel_luma":peak,"mean_frame_rgb_l1":delta_sum/1199.,"max_frame_rgb_l1":delta_max,"max_frame_rgb_l1_frame":delta_max_frame,"max_amplitude":peak_amp,"peak_frame":peak_frame,"quiet_frames_exact":quiet_frames,"median_ms_per_column":timings[2],"timing_rounds_ms":timings}));
        }
    }
    json!({"full_height_cases":result,"cost_scope":"Five rounds of1000 isolated full column draws each; includes Canvas/scratch allocation, excludes ecology/encode/PNG/browser. Not whole-world frame cost."})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidate_growth_quiet_and_support() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../captures");
        let old = pack(
            root.join("vine-wind-original-2026-09-13").to_str().unwrap(),
            false,
        );
        let new = pack(
            root.join("vine-wind-clear-ends-2026-09-13")
                .to_str()
                .unwrap(),
            true,
        );
        println!("{}", strict(&old, &new));
    }
    #[test]
    fn direct_opt_in_rejects_and_same_bend_keeps_material() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../captures");
        let old = pack(
            root.join("vine-wind-original-2026-09-13").to_str().unwrap(),
            false,
        );
        let clear = pack(
            root.join("vine-wind-clear-ends-2026-09-13")
                .to_str()
                .unwrap(),
            false,
        );
        let new = pack(
            root.join("vine-wind-clear-ends-2026-09-13")
                .to_str()
                .unwrap(),
            true,
        );
        println!("{}", verify(&old, &clear, &new));
        let mut colored = 0;
        for face in Face::ALL.into_iter().filter(|f| *f != Face::Top) {
            for cx in [0, 7, 15] {
                for h in [0.01, 0.9999999, 8., 8.875, 9.] {
                    for amp in [-0.4, 0., 0.4] {
                        let col = TallColumn {
                            face,
                            cx,
                            pick: 0,
                            vine: true,
                        };
                        let mut a = Canvas::new();
                        for f in Face::ALL {
                            for y in 0..64 {
                                for x in 0..64 {
                                    a.set(
                                        f,
                                        x,
                                        y,
                                        [
                                            0.03 + f32::from(x) / 256.,
                                            0.2,
                                            0.04 + f32::from(y) / 512.,
                                        ],
                                    );
                                }
                            }
                        }
                        let mut b = a.clone();
                        for (canvas, art) in [(&mut a, &old), (&mut b, &new)] {
                            draw_column(
                                canvas,
                                &col,
                                h,
                                art.tall_plant(TALL_PLANTS[0]).unwrap(),
                                art.tall_plant(VINE_PLANT),
                                0.0625,
                                amp,
                                &mut Vec::new(),
                            );
                        }
                        assert_eq!(
                            difference(&a, &b),
                            (0, 0.),
                            "colored backdrop mismatch {col:?} {h} {amp}"
                        );
                        colored += 1;
                    }
                }
            }
        }
        println!("{colored} exact colored-background comparisons");
    }
    #[test]
    fn independent_endpoint_anchor_is_not_the_old_owning_chart() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../captures/vine-wind-clear-ends-2026-09-13");
        let art = pack(root.to_str().unwrap(), true);
        let clip = art.tall_plant(VINE_PLANT).unwrap().cap.as_ref().unwrap();
        let pose = clip.sample(0.);
        let mut owned = Canvas::new();
        let mut independent = Canvas::new();
        let owner = tall_anchor(Face::Front, 0, 9);
        let center = tall_anchor(Face::Front, 0, 10);
        let heading = tall_heading(Face::Front, 0);
        let mask = Mask::Axial {
            reveal: tall_grown_px(8.875) - tall_bend_base(10.),
        };
        cubarium_render::stamp_pose_in_chart(
            &mut owned,
            owner,
            center,
            heading,
            pose,
            1.,
            mask,
            Bend::NONE,
            &mut Vec::new(),
        );
        stamp_layers_bent(
            &mut independent,
            center,
            heading,
            &[(pose, 1.)],
            1.,
            1.,
            mask,
            Bend::NONE,
            &mut Vec::new(),
        );
        let a = owned.get(Face::Top, 0, 62);
        let b = independent.get(Face::Top, 0, 62);
        assert!(
            a.into_iter().zip(b).any(|(a, b)| (a - b).abs() > 0.1),
            "fixture no longer exposes vertex ownership difference"
        );
        println!("Top(0,62) tile9 owner={a:?}, independent tile10={b:?}");
    }
    #[test]
    fn retained_chart_endpoint_stays_within_nine_physical_pixels() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../captures/vine-wind-clear-ends-2026-09-13");
        let art = pack(root.to_str().unwrap(), true);
        let clip = art.tall_plant(VINE_PLANT).unwrap().cap.as_ref().unwrap();
        let mut checks = 0;
        for face in Face::ALL.into_iter().filter(|f| *f != Face::Top) {
            for cx in 0..16 {
                for frame in &clip.frames {
                    for amplitude in [-1.314171379627988, 0., 1.314171379627988] {
                        let center = tall_anchor(face, cx, 10);
                        let mut c = Canvas::new();
                        cubarium_render::stamp_pose_in_chart(
                            &mut c,
                            tall_anchor(face, cx, 9),
                            center,
                            tall_heading(face, cx),
                            cubarium_render::Pose::still(frame),
                            1.,
                            Mask::None,
                            Bend {
                                amplitude,
                                base: 32.,
                                root: 0.,
                                length: 48.,
                            },
                            &mut Vec::new(),
                        );
                        for f in Face::ALL {
                            for y in 0..64 {
                                for x in 0..64 {
                                    let p = c.get(f, x, y);
                                    assert!(p.iter().all(|v| v.is_finite()));
                                    if p.iter().any(|v| *v > 0.) {
                                        assert!(
                                            cubarium_surface::unfold(
                                                center,
                                                cubarium_surface::SurfacePoint::new(
                                                    f,
                                                    f64::from(x) + 0.5,
                                                    f64::from(y) + 0.5
                                                ),
                                                9.
                                            )
                                            .is_some(),
                                            "endpoint out of physical budget {face:?} {cx} -> {f:?} {x},{y}"
                                        );
                                        checks += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        println!(
            "{checks} nonzero endpoint destinations independently inside physical9px neighborhood"
        );
    }
}
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    assert_eq!(args.len(), 3, "OLD_PACK CLEAR_PACK NEW_OUTPUT");
    let old = pack(&args[0], false);
    let clear = pack(&args[1], false);
    let new = pack(&args[1], true);
    let result = verify(&old, &clear, &new);
    println!("{result}");
    if args[2] == "--verify" {
        println!("{}", strict(&old, &new));
        return;
    }
    if args[2] == "--measure" {
        println!("{}", measure(&old, &new));
        return;
    }
    fs::create_dir(&args[2]).unwrap();
    let oldp = ArtPresenter::new(pack(&args[0], false));
    let newp = ArtPresenter::new(pack(&args[1], true));
    let mut cases = Vec::new();
    for (name, col) in [
        (
            "interior",
            TallColumn {
                face: Face::Front,
                cx: 10,
                pick: 0,
                vine: true,
            },
        ),
        (
            "vertex",
            TallColumn {
                face: Face::Front,
                cx: 0,
                pick: 0,
                vine: true,
            },
        ),
        (
            "right",
            TallColumn {
                face: Face::Right,
                cx: 15,
                pick: 0,
                vine: true,
            },
        ),
    ] {
        for (mode, height) in [("full", 9.), ("growth", 0.)] {
            for label in ["old", "new"] {
                fs::create_dir(Path::new(&args[2]).join(format!("{name}-{mode}-{label}"))).unwrap();
            }
            for frame in 0..1200 {
                let seconds = 30. + frame as f64 / 60.;
                let h = if mode == "growth" {
                    (frame as f64 / 1200. * 9.).min(9.)
                } else {
                    height
                };
                for (label, art, p) in [("old", &old, &oldp), ("new", &new, &newp)] {
                    let amp = tall_amplitude(&col, p.column_budget(&col), seconds);
                    let c = draw(art, &col, h, seconds, amp);
                    let mut rgb = Vec::new();
                    cubarium::net::net_rgb8(&bytes(&c), &mut rgb);
                    write_net_png(
                        &Path::new(&args[2])
                            .join(format!("{name}-{mode}-{label}/frame_{frame:04}.png")),
                        &rgb,
                    )
                    .unwrap();
                }
            }
            cases.push(json!({"name":name,"mode":mode,"frames":1200,"seconds":20,"old_budget":oldp.column_budget(&col),"new_budget":newp.column_budget(&col)}));
        }
    }
    fs::write(Path::new(&args[2]).join("report.json"),serde_json::to_vec_pretty(&json!({"validation":result,"cases":cases,"frozen_source":"9cf0e1d plus saved study-only draw_column patch","vine_old_budget":oldp.bend_budget(VINE_PLANT),"vine_new_budget":newp.bend_budget(VINE_PLANT)})).unwrap()).unwrap();
}
