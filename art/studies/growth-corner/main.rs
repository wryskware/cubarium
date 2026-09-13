//! Read-only study of gliding crown chart ownership; does not change production geometry.
use cubarium::{
    art::{ArtPack, Clip, TallPlant},
    art_present::*,
    net::net_rgb8,
    sink::png::write_net_png,
};
use cubarium_render::{Bend, Canvas, Mask, Pose, stamp_layers_bent, stamp_pose_in_chart};
use cubarium_surface::{PixelImage, SurfacePoint, chart_images, segment_is_valid, unfold};
use cube_proto::{Face, Frame};
use serde_json::json;
use std::{fs, path::Path};
include!(concat!(env!("OUT_DIR"), "/column.rs"));
#[allow(clippy::too_many_arguments)]
fn candidate_stamp(
    canvas: &mut Canvas,
    owner: SurfacePoint,
    center: SurfacePoint,
    heading: cubarium_surface::Vec2,
    layers: &[(Pose, f32)],
    scale: f64,
    opacity: f32,
    mask: Mask,
    bend: Bend,
    scratch: &mut Vec<PixelImage>,
) {
    if owner == center {
        stamp_layers_bent(
            canvas, center, heading, layers, scale, opacity, mask, bend, scratch,
        );
    } else {
        assert_eq!(scale, 1.);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].1, 1.);
        stamp_pose_in_chart(
            canvas,
            owner,
            center,
            heading,
            layers[0].0,
            opacity,
            mask,
            bend,
            scratch,
        );
    }
}
fn candidate(art: &ArtPack, col: TallColumn, height: f64, seconds: f64) -> Canvas {
    let mut c = Canvas::new();
    draw_column_candidate(
        &mut c,
        &col,
        height,
        art.tall_plant(TALL_PLANTS[col.pick]).unwrap(),
        art.tall_plant(VINE_PLANT),
        seconds,
        0.,
        &mut Vec::new(),
    );
    c
}
fn candidate_cap(
    art: &ArtPack,
    col: TallColumn,
    height: f64,
    seconds: f64,
    amplitude: f64,
) -> Canvas {
    let clip = art
        .tall_plant(TALL_PLANTS[col.pick])
        .unwrap()
        .cap
        .as_ref()
        .unwrap();
    let pose = clip.sample(seconds + tall_phase_of(col.face, col.cx, clip.seconds));
    let mut c = Canvas::new();
    let center = tall_anchor_at(col.face, col.cx, height + 1.);
    let owner = if center.v < 10. {
        SurfacePoint::new(center.face, center.u, 2.)
    } else {
        center
    };
    stamp_pose_in_chart(
        &mut c,
        owner,
        center,
        tall_heading(col.face, col.cx),
        pose,
        TALL_OPACITY,
        Mask::None,
        Bend {
            amplitude,
            base: tall_bend_base(height + 1.),
            root: TALL_BEND_ROOT,
            length: TALL_BEND_LENGTH,
        },
        &mut Vec::new(),
    );
    c
}

fn column(art: &ArtPack, col: TallColumn, height: f64, seconds: f64) -> Canvas {
    let mut c = Canvas::new();
    draw_column(
        &mut c,
        &col,
        height,
        art.tall_plant(TALL_PLANTS[col.pick]).unwrap(),
        art.tall_plant(VINE_PLANT),
        seconds,
        0.,
        &mut Vec::new(),
    );
    c
}

fn pack() -> ArtPack {
    ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../assets/atelier")).unwrap()
}
fn cap(art: &ArtPack, col: TallColumn, height: f64, seconds: f64) -> Canvas {
    cap_with_wind(art, col, height, seconds, 0.)
}
fn cap_with_wind(
    art: &ArtPack,
    col: TallColumn,
    height: f64,
    seconds: f64,
    amplitude: f64,
) -> Canvas {
    let plant = art.tall_plant(TALL_PLANTS[col.pick]).unwrap();
    let clip = plant.cap.as_ref().unwrap();
    let pose = clip.sample(seconds + tall_phase_of(col.face, col.cx, clip.seconds));
    let mut c = Canvas::new();
    stamp_layers_bent(
        &mut c,
        tall_anchor_at(col.face, col.cx, height + 1.),
        tall_heading(col.face, col.cx),
        &[(pose, 1.)],
        1.,
        TALL_OPACITY,
        Mask::None,
        Bend {
            amplitude,
            base: tall_bend_base(height + 1.),
            root: TALL_BEND_ROOT,
            length: TALL_BEND_LENGTH,
        },
        &mut Vec::new(),
    );
    c
}
fn worst(a: &Canvas, b: &Canvas) -> serde_json::Value {
    let mut best = 0.;
    let mut where_ = json!(null);
    let mut fa = Frame::black();
    let mut fb = Frame::black();
    a.encode(&mut fa);
    b.encode(&mut fb);
    for f in Face::ALL {
        for y in 0..64 {
            for x in 0..64 {
                let pa = a.get(f, x, y);
                let pb = b.get(f, x, y);
                let d = pa
                    .into_iter()
                    .zip(pb)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0f32, f32::max);
                if d > best {
                    best = d;
                    where_ = json!({"face":format!("{f:?}"),"x":x,"y":y,"before":pa,"after":pb,"before_rgb8":fa.get(f,x as usize,y as usize),"after_rgb8":fb.get(f,x as usize,y as usize)});
                }
            }
        }
    }
    json!({"max_linear_channel":best,"pixel":where_})
}
fn light(c: &Canvas) -> f64 {
    Face::ALL
        .into_iter()
        .flat_map(|f| (0..64).flat_map(move |y| (0..64).map(move |x| (f, x, y))))
        .map(|(f, x, y)| c.get(f, x, y).into_iter().map(f64::from).sum::<f64>())
        .sum()
}
fn paths(anchor: SurfacePoint, target: SurfacePoint) -> serde_json::Value {
    let mut charts = Vec::new();
    chart_images(anchor.face, 2, &mut charts);
    let mut result = Vec::new();
    for chart in charts {
        if chart.target_face == target.face {
            let local = chart.image_point(target.chart());
            let d = (local - anchor.chart()).length();
            if d <= 9. && segment_is_valid(anchor.face, anchor.chart(), local, &chart.path) {
                result.push(json!({"local":[local.x,local.y],"distance":d,"path":format!("{:?}",chart.path.steps())}));
            }
        }
    }
    json!({"anchor":[anchor.u,anchor.v],"candidates":result,"selected":unfold(anchor,target,9.).map(|u|format!("{:?}",u.path.steps()))})
}
fn report() -> serde_json::Value {
    let art = pack();
    let mut cases = Vec::new();
    let h = 53. / 6.;
    for col in [
        TallColumn {
            face: Face::Front,
            cx: 0,
            pick: 0,
            vine: true,
        },
        TallColumn {
            face: Face::Right,
            cx: 15,
            pick: 0,
            vine: true,
        },
    ]
    .into_iter()
    .chain(tall_columns())
    {
        let a = cap(&art, col, h - 1e-8, 49.625);
        let b = cap(&art, col, h + 1e-8, 49.625);
        cases.push(json!({"column":format!("{col:?}"),"selected":tall_column_of(col.face,col.cx)==Some(col),"threshold_height":h,"epsilon":1e-8,"cap_only":worst(&a,&b),"whole_column":worst(&column(&art,col,h-1e-8,49.625),&column(&art,col,h+1e-8,49.625)),"candidate_column":worst(&candidate(&art,col,h-1e-8,49.625),&candidate(&art,col,h+1e-8,49.625))}));
    }
    let mut evidence = Vec::new();
    for frame in [1177, 1178] {
        let height = frame as f64 / 1200. * 9.;
        let center = tall_anchor_at(Face::Front, 0, height + 1.);
        evidence.push(json!({"frame":frame,"height":height,"ownership":paths(center,SurfacePoint::new(Face::Top,0.5,60.5))}));
    }
    let mut endpoint = Vec::new();
    for col in tall_columns().into_iter().filter(|c| c.cx < 2 || c.cx > 13) {
        let a = cap(&art, col, 9., 49.625);
        let b = candidate_cap(&art, col, 9., 49.625, 0.);
        endpoint.push(json!({"column":format!("{col:?}"),"original_light":light(&a),"candidate_light":light(&b),"static_difference":worst(&a,&b)}));
    }
    json!({"source":"a9eb064 cap path; no production edits","actual_columns":tall_columns().into_iter().map(|c|format!("{c:?}")).collect::<Vec<_>>(),"threshold_cases":cases,"reported_boundary_paths":evidence,"endpoint_cap_light_rgb_sum":endpoint})
}
fn captures(path: &Path) {
    fs::create_dir_all(path).unwrap();
    let art = pack();
    let h = 53. / 6.;
    let cases = [
        (
            "forced-front",
            TallColumn {
                face: Face::Front,
                cx: 0,
                pick: 0,
                vine: true,
            },
            (0, 52),
        ),
        (
            "forced-right",
            TallColumn {
                face: Face::Right,
                cx: 15,
                pick: 0,
                vine: true,
            },
            (52, 0),
        ),
        (
            "selected-left",
            tall_column_of(Face::Left, 0).unwrap(),
            (0, 0),
        ),
    ];
    let mut contact = vec![0; 36 * 48 * 3];
    for (case, (name, col, (cx, cy))) in cases.into_iter().enumerate() {
        for (row, delta) in [-0.0075, -1e-8, 1e-8, 0.0075].into_iter().enumerate() {
            let c = column(&art, col, h + delta, 49.625);
            let mut frame = Frame::black();
            c.encode(&mut frame);
            let mut net = Vec::new();
            net_rgb8(&frame, &mut net);
            write_net_png(&path.join(format!("{name}-{row}.png")), &net).unwrap();
            let mut proposed = Frame::black();
            candidate(&art, col, h + delta, 49.625).encode(&mut proposed);
            net_rgb8(&proposed, &mut net);
            write_net_png(&path.join(format!("{name}-{row}-candidate.png")), &net).unwrap();
            for y in 0..12 {
                for x in 0..12 {
                    let rgb = frame.get(Face::Top, cx + x, cy + y);
                    let dst = ((row * 12 + y) * 36 + case * 12 + x) * 3;
                    contact[dst..dst + 3].copy_from_slice(&rgb);
                }
            }
        }
    }
    for scale in [1, 12] {
        let mut rgb = vec![0; 36 * 48 * 3 * scale * scale];
        for y in 0..48 * scale {
            for x in 0..36 * scale {
                let dst = (y * 36 * scale + x) * 3;
                let src = ((y / scale) * 36 + x / scale) * 3;
                rgb[dst..dst + 3].copy_from_slice(&contact[src..src + 3]);
            }
        }
        let f = fs::File::create(path.join(format!("top-corner-contact-{scale}x.png"))).unwrap();
        let mut enc = png::Encoder::new(f, (36 * scale) as u32, (48 * scale) as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&rgb).unwrap();
    }
    // Actual selected Left0: each row is Top corner | Left crown, original then candidate.
    let col = tall_column_of(Face::Left, 0).unwrap();
    let mut contact = vec![0; 48 * 64 * 3];
    for (row, height) in [7., 8., 53. / 6., 9.].into_iter().enumerate() {
        for (variant, c) in [
            column(&art, col, height, 49.625),
            candidate(&art, col, height, 49.625),
        ]
        .into_iter()
        .enumerate()
        {
            let mut frame = Frame::black();
            c.encode(&mut frame);
            for (part, face) in [Face::Top, Face::Left].into_iter().enumerate() {
                for y in 0..16 {
                    for x in 0..12 {
                        let dst = ((row * 16 + y) * 48 + variant * 24 + part * 12 + x) * 3;
                        contact[dst..dst + 3].copy_from_slice(&frame.get(face, x, y));
                    }
                }
            }
        }
    }
    for scale in [1, 8] {
        let mut rgb = vec![0; 48 * 64 * 3 * scale * scale];
        for y in 0..64 * scale {
            for x in 0..48 * scale {
                let dst = (y * 48 * scale + x) * 3;
                let src = ((y / scale) * 48 + x / scale) * 3;
                rgb[dst..dst + 3].copy_from_slice(&contact[src..src + 3]);
            }
        }
        let f = fs::File::create(path.join(format!("selected-left-handoff-endpoint-{scale}x.png")))
            .unwrap();
        let mut enc = png::Encoder::new(f, (48 * scale) as u32, (64 * scale) as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&rgb).unwrap();
    }
    let dir = path.join("selected-left-60fps");
    fs::create_dir_all(&dir).unwrap();
    let mut previous = None;
    let mut peaks = [
        json!({"max_linear_channel":0.}),
        json!({"max_linear_channel":0.}),
    ];
    for k in 0..=240 {
        let height = 7. + k as f64 / 120.;
        let views = [
            column(&art, col, height, 49.625),
            candidate(&art, col, height, 49.625),
        ];
        if let Some(ref prev) = previous {
            let prev: &[Canvas; 2] = prev;
            for j in 0..2 {
                let mut d = worst(&prev[j], &views[j]);
                if d["max_linear_channel"].as_f64() > peaks[j]["max_linear_channel"].as_f64() {
                    d["frame"] = json!(k);
                    d["height"] = json!(height);
                    peaks[j] = d;
                }
            }
        }
        let mut pair = vec![0; 512 * 128 * 3];
        for (j, c) in views.iter().enumerate() {
            let mut frame = Frame::black();
            c.encode(&mut frame);
            let mut net = Vec::new();
            net_rgb8(&frame, &mut net);
            for y in 0..128 {
                pair[(y * 512 + j * 256) * 3..(y * 512 + j * 256 + 256) * 3]
                    .copy_from_slice(&net[y * 256 * 3..(y + 1) * 256 * 3]);
            }
        }
        let file = fs::File::create(dir.join(format!("{k:04}.png"))).unwrap();
        let mut enc = png::Encoder::new(file, 512, 128);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&pair).unwrap();
        previous = Some(views);
    }
    fs::write(path.join("temporal.json"),serde_json::to_string_pretty(&json!({"scope":"Actual selected Left0; synthetic linear height7..9/4seconds; fixed clip time49.625; wind0. Not a recorded ecological trajectory.","frames":241,"fps":60,"old_peak":peaks[0],"candidate_peak":peaks[1]})).unwrap()).unwrap();
}
fn main() {
    if let Some(path) = std::env::args().nth(1) {
        captures(Path::new(&path));
    }
    println!("{}", serde_json::to_string_pretty(&report()).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_surface::Vec2;
    #[test]
    fn reported_boundary_is_an_ownership_switch_not_a_growth_step() {
        let h = 53. / 6.;
        let target = SurfacePoint::new(Face::Top, 0.5, 60.5);
        let a = unfold(tall_anchor_at(Face::Front, 0, h - 1e-8 + 1.), target, 9.).unwrap();
        let b = unfold(tall_anchor_at(Face::Front, 0, h + 1e-8 + 1.), target, 9.).unwrap();
        assert_ne!(a.path, b.path);
        assert!((a.distance - b.distance).abs() < 1e-6);
        assert_eq!(a.local, Vec2::new(-3.5, -0.5));
        assert_eq!(b.local, Vec2::new(0.5, -3.5));
        let art = pack();
        let col = TallColumn {
            face: Face::Front,
            cx: 0,
            pick: 0,
            vine: true,
        };
        assert!(
            worst(
                &cap(&art, col, h - 1e-8, 49.625),
                &cap(&art, col, h + 1e-8, 49.625)
            )["max_linear_channel"]
                .as_f64()
                .unwrap()
                > 0.2
        );
        assert!(tall_column_of(Face::Front, 0).is_none());
        assert!(tall_column_of(Face::Right, 15).is_none());
    }
    #[test]
    fn selected_column_retains_the_discontinuity_and_cap_removal_is_a_causal_control() {
        let mut art = pack();
        let col = tall_column_of(Face::Left, 0).unwrap();
        let h = 53. / 6.;
        for epsilon in [1e-4, 1e-6, 1e-8] {
            let a = column(&art, col, h - epsilon, 49.625);
            let b = column(&art, col, h + epsilon, 49.625);
            assert!(worst(&a, &b)["max_linear_channel"].as_f64().unwrap() > 0.15);
        }
        art.tall
            .iter_mut()
            .find(|p| p.name == TALL_PLANTS[col.pick])
            .unwrap()
            .cap = None;
        assert!(
            worst(
                &column(&art, col, h - 1e-8, 49.625),
                &column(&art, col, h + 1e-8, 49.625)
            )["max_linear_channel"]
                .as_f64()
                .unwrap()
                < 1e-6
        );
    }
    #[test]
    fn boundary_is_held_deterministic_and_is_not_the_vine_opt_in() {
        let mut art = pack();
        let col = tall_column_of(Face::Left, 0).unwrap();
        let h = 53. / 6.;
        let a = column(&art, col, h - 1e-8, 49.625);
        let b = column(&art, col, h + 1e-8, 49.625);
        assert_eq!(
            worst(&a, &column(&art, col, h - 1e-8, 49.625))["max_linear_channel"],
            0.
        );
        art.tall
            .iter_mut()
            .find(|p| p.name == VINE_PLANT)
            .unwrap()
            .vine_strips = None;
        for (height, expected) in [(h - 1e-8, a), (h + 1e-8, b)] {
            assert!(
                worst(&expected, &column(&art, col, height, 49.625))["max_linear_channel"]
                    .as_f64()
                    .unwrap()
                    < 1e-6
            );
        }
        let interior = tall_column_of(Face::Front, 7).unwrap();
        assert!(
            worst(
                &column(&art, interior, h - 1e-8, 49.625),
                &column(&art, interior, h + 1e-8, 49.625)
            )["max_linear_channel"]
                .as_f64()
                .unwrap()
                < 1e-6
        );
    }
    #[test]
    fn candidate_handoff_and_reported_boundary_are_continuous_with_exact_lower_growth() {
        let art = pack();
        for col in tall_columns() {
            for seconds in [0., 0.9375, 2.99999, 49.625] {
                for h in [0., 0.1, 1., 4.5, 6.99999, 7.] {
                    assert_eq!(
                        worst(
                            &column(&art, col, h, seconds),
                            &candidate(&art, col, h, seconds)
                        )["max_linear_channel"],
                        0.
                    );
                }
                for h in [7., 53. / 6.] {
                    assert!(
                        worst(
                            &candidate(&art, col, h - 1e-8, seconds),
                            &candidate(&art, col, h + 1e-8, seconds)
                        )["max_linear_channel"]
                            .as_f64()
                            .unwrap()
                            < 1e-6
                    );
                }
            }
        }
    }
    #[test]
    fn candidate_physical_nine_pixel_support_all_corners_species_and_wind_signs() {
        let art = pack();
        let mut checked = 0;
        for face in Face::ALL.into_iter().filter(|f| *f != Face::Top) {
            for cx in [0, 1, 14, 15] {
                for pick in 0..2 {
                    let col = TallColumn {
                        face,
                        cx,
                        pick,
                        vine: true,
                    };
                    let budget = tall_bend_budget(art.tall_plant(TALL_PLANTS[pick]).unwrap());
                    for h in [7., 7.5, 8., 53. / 6., 9.] {
                        for seconds in [0., 0.9375, 2.99999] {
                            for amplitude in [-budget, 0., budget] {
                                let center = tall_anchor_at(face, cx, h + 1.);
                                let c = candidate_cap(&art, col, h, seconds, amplitude);
                                for f in Face::ALL {
                                    for y in 0..64 {
                                        for x in 0..64 {
                                            if c.get(f, x, y) != [0.; 3] {
                                                assert!(
                                                    unfold(
                                                        center,
                                                        SurfacePoint::pixel_center(f, x, y),
                                                        9.
                                                    )
                                                    .is_some(),
                                                    "{col:?} h{h} t{seconds} amp{amplitude} target{f:?} {x},{y}"
                                                );
                                                checked += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(checked > 10_000);
    }
    #[test]
    fn final_owner_handoff_all_authored_frames_preserves_endpoint_and_held_identity() {
        let art = pack();
        for face in Face::ALL.into_iter().filter(|f| *f != Face::Top) {
            for cx in [0, 1, 14, 15] {
                for pick in 0..2 {
                    let col = TallColumn {
                        face,
                        cx,
                        pick,
                        vine: true,
                    };
                    let plant = art.tall_plant(TALL_PLANTS[pick]).unwrap();
                    let clip = plant.cap.as_ref().unwrap();
                    let budget = tall_bend_budget(plant);
                    for k in 0..clip.frames.len() * 2 {
                        let seconds = k as f64 * clip.seconds / (clip.frames.len() * 2) as f64
                            - tall_phase_of(face, cx, clip.seconds);
                        for amplitude in [-budget, 0., budget] {
                            let a = candidate_cap(&art, col, 7. - 1e-8, seconds, amplitude);
                            let b = candidate_cap(&art, col, 7. + 1e-8, seconds, amplitude);
                            assert!(
                                worst(&a, &b)["max_linear_channel"].as_f64().unwrap() < 1e-6,
                                "handoff {col:?} phase{k} amp{amplitude}"
                            );
                            let end = candidate_cap(&art, col, 9., seconds, amplitude);
                            assert!(worst(&end,&cap_with_wind(&art,col,9.,seconds,amplitude))["max_linear_channel"].as_f64().unwrap()<1e-6);
                            assert_eq!(
                                worst(&end, &candidate_cap(&art, col, 9., seconds, amplitude))["max_linear_channel"],
                                0.
                            );
                        }
                    }
                }
            }
        }
    }
}
