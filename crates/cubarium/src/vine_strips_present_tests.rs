//! Production-path regression sweeps ported from the independently reviewed frozen study.
//! Exact same-amplitude/quiet comparisons deliberately exclude the intended budget gain.
use super::*;
use crate::art::ArtPack;
use cubarium_render::stamp_layers_bent_with_radius;
use cube_proto::Frame;
use serde_json::json;
use std::path::Path;

fn pack() -> ArtPack {
    ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier")).unwrap()
}
fn legacy_pack() -> ArtPack {
    let mut art = pack();
    art.tall_plant(VINE_PLANT)
        .unwrap()
        .vine_strips
        .as_ref()
        .unwrap();
    art.tall
        .iter_mut()
        .find(|p| p.name == VINE_PLANT)
        .unwrap()
        .vine_strips = None;
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
    for clip in [
        &vine.vine_strips.as_ref().unwrap().trunk,
        &vine.vine_strips.as_ref().unwrap().endpoint,
    ] {
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

#[test]
fn all_slots_growth_roots_loops_quiet_and_support_match_legacy() {
    println!("{}", strict(&legacy_pack(), &pack()));
}
#[test]
fn exact_material_registration_and_expected_budget_gain() {
    let old = legacy_pack();
    let new = pack();
    let mut naive = pack();
    let vine = naive
        .tall
        .iter_mut()
        .find(|p| p.name == VINE_PLANT)
        .unwrap();
    vine.trunk = vine.vine_strips.take().unwrap().trunk;
    println!("{}", verify(&old, &naive, &new));
    let old_p = ArtPresenter::new(legacy_pack());
    let new_p = ArtPresenter::new(pack());
    let spire = TallColumn {
        face: Face::Front,
        cx: 10,
        pick: 0,
        vine: true,
    };
    assert!(new_p.bend_budget(VINE_PLANT) > 3.5);
    assert!(old_p.bend_budget(VINE_PLANT) < 0.47);
    assert_eq!(
        new_p.column_budget(&spire),
        new_p.bend_budget(TALL_PLANTS[0])
    );
    assert_eq!(
        effective_tip(
            wind_response(TALL_PLANTS[0]).tip_px,
            new_p.column_budget(&spire)
        ),
        0.9
    );
    let glass = TallColumn { pick: 1, ..spire };
    assert_eq!(new_p.column_budget(&glass), old_p.column_budget(&glass));
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
                                    [0.03 + f32::from(x) / 256., 0.2, 0.04 + f32::from(y) / 512.],
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
fn retained_endpoint_chart_preserves_vertex_and_physical_footprint() {
    let art = pack();
    let clip = &art
        .tall_plant(VINE_PLANT)
        .unwrap()
        .vine_strips
        .as_ref()
        .unwrap()
        .endpoint;
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
    assert!(
        difference(&owned, &independent).1 > 0.1,
        "independent anchor must not replace retained owner"
    );
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
                        Pose::still(frame),
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
                                            SurfacePoint::new(
                                                f,
                                                f64::from(x) + 0.5,
                                                f64::from(y) + 0.5
                                            ),
                                            9.
                                        )
                                        .is_some(),
                                        "endpoint out of physical budget"
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
    println!("{checks} physically in-budget endpoint destinations");
}
