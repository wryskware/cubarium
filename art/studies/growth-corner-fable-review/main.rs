//! Cost probe for the growth-corner study candidate (art/studies/growth-corner-fable-review):
//! how much longer does a near-corner column take to draw with the retained-owner cap
//! than with the ordinary cap path? Times whole-column draws (base, trunk, vine, cap) of
//! the actual selected Left0 glasscane+vine column and, for scale, an interior column the
//! candidate leaves on the ordinary path, at several growth heights, fixed clip time and
//! no wind. Prints JSON. Wall-clock on this machine, single thread, release profile; not
//! a host frame budget measurement.
use cubarium::art::{ArtPack, Clip, TallPlant};
use cubarium::art_present::*;
use cubarium_render::{Bend, Canvas, Mask, Pose, stamp_layers_bent, stamp_pose_in_chart};
use cubarium_surface::{PixelImage, SurfacePoint};
use cube_proto::Face;
use serde_json::json;
use std::path::Path;
use std::time::Instant;
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
        stamp_layers_bent(canvas, center, heading, layers, scale, opacity, mask, bend, scratch);
    } else {
        assert_eq!(scale, 1.);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].1, 1.);
        stamp_pose_in_chart(canvas, owner, center, heading, layers[0].0, opacity, mask, bend, scratch);
    }
}

fn time_draws(art: &ArtPack, col: TallColumn, height: f64, candidate: bool, n: usize) -> f64 {
    let plant = art.tall_plant(TALL_PLANTS[col.pick]).unwrap();
    let vine = art.tall_plant(VINE_PLANT);
    let mut canvas = Canvas::new();
    let mut scratch = Vec::new();
    // Warm up once, then time.
    for _ in 0..3 {
        canvas.clear();
        if candidate {
            draw_column_candidate(&mut canvas, &col, height, plant, vine, 49.625, 0., &mut scratch);
        } else {
            draw_column(&mut canvas, &col, height, plant, vine, 49.625, 0., &mut scratch);
        }
    }
    let start = Instant::now();
    for _ in 0..n {
        canvas.clear();
        if candidate {
            draw_column_candidate(&mut canvas, &col, height, plant, vine, 49.625, 0., &mut scratch);
        } else {
            draw_column(&mut canvas, &col, height, plant, vine, 49.625, 0., &mut scratch);
        }
    }
    start.elapsed().as_secs_f64() * 1e6 / n as f64
}

fn main() {
    let art = ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../assets/atelier")).unwrap();
    let n = 400;
    let corner = tall_column_of(Face::Left, 0).unwrap();
    let interior = tall_column_of(Face::Front, 7).unwrap();
    let corners: Vec<TallColumn> = tall_columns().into_iter().filter(|c| c.cx < 2 || c.cx > 13).collect();
    let mut rows = Vec::new();
    for (name, col) in [("selected Left0 (corner)", corner), ("selected Front7 (interior)", interior)] {
        for height in [6.0, 7.5, 8.0, 8.5, 53.0 / 6.0, 9.0] {
            let o = time_draws(&art, col, height, false, n);
            let c = time_draws(&art, col, height, true, n);
            rows.push(json!({"column": name, "height": height, "original_us": (o * 10.0).round() / 10.0, "candidate_us": (c * 10.0).round() / 10.0, "ratio": (c / o * 100.0).round() / 100.0}));
        }
    }
    // All selected near-corner columns at once, at height 8 (the candidate's costliest region).
    let all_o: f64 = corners.iter().map(|c| time_draws(&art, *c, 8.0, false, n)).sum();
    let all_c: f64 = corners.iter().map(|c| time_draws(&art, *c, 8.0, true, n)).sum();
    println!("{}", serde_json::to_string_pretty(&json!({
        "draws_per_sample": n, "clip_time": 49.625, "wind": 0.0,
        "rows": rows,
        "all_selected_corner_columns_at_height_8": {"count": corners.len(), "original_us": all_o.round(), "candidate_us": all_c.round(), "columns": corners.iter().map(|c| format!("{c:?}")).collect::<Vec<_>>()},
        "note": "Single-thread wall clock per whole-column draw in this probe's release build; the host draws all columns once per frame."
    })).unwrap());
}
