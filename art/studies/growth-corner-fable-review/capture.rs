//! Paired capture of the integrated corner-cap owner through the actual host presenter
//! (`ArtPresenter::observe` / `draw`, real wind packets and art phase), on a recorded world
//! stepped live or on a synthetic rich field. Two presenters draw every frame from the same
//! views: one with the shipped pack (capability on), one with the same pack cleared in
//! memory (the original cap path). Writes native 256×128 nets for both, an 8× nearest strip
//! of the Top corner / side crown / seam neighbour around the chosen column per frame set,
//! and `timeline.json` with the column's paced height, the wind amplitude and the paired
//! difference per frame.
//!
//! ```text
//! corner_cap_capture --scan --world OPENING.cubw --ticks N          # corner column heights over time
//! corner_cap_capture --world OPENING.cubw --ticks N --from A --to B --face F --cx CX --out DIR
//! corner_cap_capture --synthetic --grow-from T --from A --to B --face F --cx CX --out DIR
//! ```
//!
//! `--synthetic`: every cell rich from tick 0 except the chosen column's cells, which are
//! bare until `--grow-from` and rich after it, so the column climbs at the presenter's own
//! paced rate (`TALL_GROW_PX_PER_S`) through the corner while the real wind blows. That is
//! the presenter's actual growth pacing on a made-up field, not an ecological trajectory.
use std::fs;
use std::path::{Path, PathBuf};

use cube_proto::{Face, Frame};
use cubarium::art::ArtPack;
use cubarium::art_present::{ArtPresenter, SOIL_SCALE, tall_amplitude, tall_column_of};
use cubarium::net::net_rgb8;
use cubarium::present::PRODUCER_SATURATION;
use cubarium::sink::png::write_net_png;
use cubarium_core::view::RenderView;
use cubarium_core::{World, decode_snapshot};
use cubarium_render::Canvas;
use cubarium_surface::{CELL_COUNT, CellId};

const FPT: u64 = 3;
const PRODUCER_MAX: f64 = 10.0;

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}
fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

fn face_of(i: u8) -> Face {
    Face::ALL[usize::from(i)]
}

/// Net origin of a face in the 256×128 net.
fn origin(face: Face) -> (usize, usize) {
    match face {
        Face::Top => (64, 0),
        Face::Left => (0, 64),
        Face::Front => (64, 64),
        Face::Right => (128, 64),
        Face::Back => (192, 64),
    }
}

fn write_png(path: &Path, w: usize, h: usize, rgb: &[u8]) {
    let f = fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(f, w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgb).unwrap();
}

/// Three 12×16 crops around a corner column's crown: the Top face corner nearest it, the
/// column's own face top rows, and the neighbouring face's edge across the seam.
fn regions(face: Face, cx: u8) -> Vec<(&'static str, Face, usize, usize)> {
    // Side faces in ring order Front → Right → Back → Left → Front; the Top face's edge
    // adjoining each side face differs, so take the Top corner nearest the column by
    // brute force: the Top pixel at minimum embedded distance from the column's top anchor.
    let left_edge = cx < 2;
    let neighbour = match (face, left_edge) {
        (Face::Front, true) => Face::Left,
        (Face::Front, false) => Face::Right,
        (Face::Right, true) => Face::Front,
        (Face::Right, false) => Face::Back,
        (Face::Back, true) => Face::Right,
        (Face::Back, false) => Face::Left,
        (Face::Left, true) => Face::Back,
        (Face::Left, false) => Face::Front,
        _ => Face::Front,
    };
    let own_x = if left_edge { 0 } else { 52 };
    let nb_x = if left_edge { 52 } else { 0 };
    let anchor = cubarium::art_present::tall_anchor_at(face, cx, 10.0).embed();
    let mut best = (f64::INFINITY, 0usize, 0usize);
    for y in 0..64u8 {
        for x in 0..64u8 {
            let p = cubarium_surface::SurfacePoint::pixel_center(Face::Top, x, y).embed();
            let d = (0..3).map(|i| (p[i] - anchor[i]).powi(2)).sum::<f64>();
            if d < best.0 {
                best = (d, usize::from(x), usize::from(y));
            }
        }
    }
    let tx = best.1.saturating_sub(6).min(52);
    let ty = best.2.saturating_sub(8).min(48);
    vec![("top", Face::Top, tx, ty), ("own", face, own_x, 0), ("seam", neighbour, nb_x, 0)]
}

fn strip(frames: &[&Frame], regions: &[(&str, Face, usize, usize)], scale: usize) -> (usize, usize, Vec<u8>) {
    let cw = regions.len() * (12 + 1) - 1;
    let w = cw * scale;
    let h = frames.len() * (16 + 1) * scale;
    let mut rgb = vec![0u8; w * h * 3];
    for (r, frame) in frames.iter().enumerate() {
        for (ri, (_, face, x0, y0)) in regions.iter().enumerate() {
            for y in 0..16 {
                for x in 0..12 {
                    let px = frame.get(*face, x0 + x, y0 + y);
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let dx = (ri * 13 + x) * scale + sx;
                            let dy = (r * 17 + y) * scale + sy;
                            let i = (dy * w + dx) * 3;
                            rgb[i..i + 3].copy_from_slice(&px);
                        }
                    }
                }
            }
        }
    }
    (w, h, rgb)
}

fn diff(a: &Frame, b: &Frame) -> (u8, usize, Vec<(Face, usize, usize, u8)>) {
    let mut worst = 0u8;
    let mut n = 0;
    let mut list = Vec::new();
    for face in Face::ALL {
        for y in 0..64 {
            for x in 0..64 {
                let p = a.get(face, x, y);
                let q = b.get(face, x, y);
                let d = (0..3).map(|c| p[c].abs_diff(q[c])).max().unwrap();
                if d > 0 {
                    n += 1;
                    if list.len() < 16 {
                        list.push((face, x, y, d));
                    }
                }
                worst = worst.max(d);
            }
        }
    }
    (worst, n, list)
}

fn cleared(atelier: &Path) -> ArtPack {
    let mut art = ArtPack::load(atelier).unwrap();
    for p in art.tall.iter_mut() {
        p.corner_cap_owner = false;
    }
    art
}

fn main() {
    let atelier = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/atelier");
    let ticks: u64 = arg("--ticks").map_or(0, |s| s.parse().unwrap());
    let from: u64 = arg("--from").map_or(u64::MAX, |s| s.parse().unwrap());
    let to: u64 = arg("--to").map_or(0, |s| s.parse().unwrap());
    let ticks = ticks.max(if to > 0 { to + 1 } else { 0 });
    let synthetic = flag("--synthetic");
    let grow_from: u64 = arg("--grow-from").map_or(200, |s| s.parse().unwrap());
    let face = arg("--face").map(|s| face_of(s.parse().unwrap()));
    let cx: Option<u8> = arg("--cx").map(|s| s.parse().unwrap());
    let flagged_pack = ArtPack::load(&atelier).unwrap();
    assert!(flagged_pack.tall_plant("glasscane").unwrap().corner_cap_owner, "the shipped pack must carry the capability");
    let mut flagged = ArtPresenter::new(flagged_pack);
    let mut plain = ArtPresenter::new(cleared(&atelier));
    let corner: Vec<(usize, cubarium::art_present::TallColumn)> =
        flagged.columns().iter().copied().enumerate().filter(|(_, c)| c.cx < 2 || c.cx > 13).collect();
    let chosen = match (face, cx) {
        (Some(f), Some(x)) => Some(tall_column_of(f, x).expect("the hash must select that column")),
        _ => None,
    };
    let chosen_index = chosen.map(|c| flagged.columns().iter().position(|k| *k == c).unwrap());
    let column_cells: Vec<usize> = chosen.map(|c| CellId::all().filter(|k| k.face() == c.face && k.cx() == c.cx).map(|k| k.index()).collect()).unwrap_or_default();

    let mut world: Option<World> = None;
    let mut world_path = None;
    if !synthetic {
        let p = arg("--world").expect("--world or --synthetic");
        let bytes = fs::read(&p).unwrap();
        let (_, state) = decode_snapshot(&bytes).unwrap();
        world = Some(World::from_state(state).unwrap());
        world_path = Some(p);
    }
    let out = arg("--out").map(PathBuf::from);
    if let Some(o) = &out {
        fs::create_dir(o).expect("--out must be a fresh directory with an existing parent");
        fs::create_dir_all(o.join("old")).unwrap();
        fs::create_dir_all(o.join("new")).unwrap();
        fs::create_dir_all(o.join("strips")).unwrap();
    }
    let regions = chosen.map(|c| regions(c.face, c.cx));
    let mut timeline = Vec::new();
    let mut canvas = Canvas::new();
    let mut fa = Frame::black();
    let mut fb = Frame::black();
    let mut rgb = Vec::new();
    let mut peak_old = (0u8, 0u64);
    let mut peak_new = (0u8, 0u64);
    let mut prev: Option<(Frame, Frame)> = None;
    for elapsed in 0..ticks {
        let view = match world.as_mut() {
            Some(w) => {
                w.step();
                w.drain_events();
                w.drain_hunter_events();
                w.render_view()
            }
            None => {
                let mut v = RenderView {
                    tick: elapsed,
                    producer: vec![PRODUCER_MAX * PRODUCER_SATURATION; CELL_COUNT],
                    detritus: vec![SOIL_SCALE; CELL_COUNT],
                    fruit: vec![0.0; CELL_COUNT],
                    water: vec![0.0; CELL_COUNT],
                    rain: vec![0.0; CELL_COUNT],
                    producer_max: PRODUCER_MAX,
                    organisms: Vec::new(),
                };
                if elapsed < grow_from {
                    for &i in &column_cells {
                        v.producer[i] = 0.0;
                    }
                }
                v
            }
        };
        flagged.observe(&view);
        plain.observe(&view);
        if flag("--scan") {
            if elapsed % 300 == 0 {
                let hs: Vec<String> = corner.iter().map(|(i, c)| format!("{:?}{}={:.2}/{}", c.face, c.cx, flagged.tall_growth_of(*i).height, flagged.tall_growth_of(*i).target)).collect();
                // The chosen column's wind amplitude (tile px) over the next 300 ticks:
                // the largest magnitude, so a windy window can be chosen for a capture.
                let wind = chosen.map(|c| {
                    let budget = flagged.column_budget(&c);
                    (0..300u64).map(|d| tall_amplitude(&c, budget, cubarium::art_present::present_seconds(view.tick + d, 0.0)).abs()).fold(0.0f64, f64::max)
                });
                println!("elapsed {elapsed}: {}  wind_max_next300={:?}", hs.join("  "), wind.map(|w| (w * 1000.0).round() / 1000.0));
            }
            continue;
        }
        if !(from..=to).contains(&elapsed) {
            continue;
        }
        let height = chosen_index.map(|i| flagged.tall_growth_of(i).height);
        let budget = chosen.map(|c| flagged.column_budget(&c)).unwrap_or(0.0);
        for k in 0..FPT {
            let f = k as f64 / FPT as f64;
            canvas.clear();
            flagged.draw(&view, f, &mut canvas);
            canvas.encode(&mut fa);
            canvas.clear();
            plain.draw(&view, f, &mut canvas);
            canvas.encode(&mut fb);
            let (worst, n, list) = diff(&fb, &fa);
            let seconds = cubarium::art_present::present_seconds(view.tick, f);
            let amplitude = chosen.map(|c| tall_amplitude(&c, budget, seconds)).unwrap_or(0.0);
            let mut step = serde_json::Value::Null;
            if let Some((po, pn)) = &prev {
                let (so, _, _) = diff(po, &fb);
                let (sn, _, _) = diff(pn, &fa);
                let n_frame = elapsed * FPT + k;
                if so > peak_old.0 {
                    peak_old = (so, n_frame);
                }
                if sn > peak_new.0 {
                    peak_new = (sn, n_frame);
                }
                step = serde_json::json!({"old": so, "new": sn});
            }
            timeline.push(serde_json::json!({"frame": elapsed * FPT + k, "tick": view.tick, "f": f, "height": height, "amplitude": amplitude, "old_vs_new": {"worst": worst, "pixels": n, "first": list.iter().map(|(fc, x, y, d)| format!("{fc:?}({x},{y}):{d}")).collect::<Vec<_>>()}, "step": step}));
            if let Some(o) = &out {
                let name = format!("frame_{:05}.png", elapsed * FPT + k);
                rgb.clear();
                net_rgb8(&fb, &mut rgb);
                write_net_png(&o.join("old").join(&name), &rgb).unwrap();
                rgb.clear();
                net_rgb8(&fa, &mut rgb);
                write_net_png(&o.join("new").join(&name), &rgb).unwrap();
                if let Some(r) = &regions {
                    let (w, h, px) = strip(&[&fb, &fa], r, 8);
                    write_png(&o.join("strips").join(&name), w, h, &px);
                }
            }
            prev = Some((fb.clone(), fa.clone()));
        }
    }
    if let Some(o) = &out {
        fs::write(o.join("timeline.json"), serde_json::to_string(&timeline).unwrap()).unwrap();
        let report = serde_json::json!({
            "kind": "corner-cap-integrated-paired-capture-v1",
            "world": world_path, "synthetic": synthetic, "grow_from": synthetic.then_some(grow_from),
            "column": chosen.map(|c| format!("{c:?}")), "frames_from": from, "frames_to": to, "frames_per_tick": FPT,
            "peak_step_old": {"max": peak_old.0, "frame": peak_old.1}, "peak_step_new": {"max": peak_new.0, "frame": peak_new.1},
            "strip_regions": regions.as_ref().map(|r| r.iter().map(|(n, f, x, y)| format!("{n}: {f:?} x{x}..{} y{y}..{}", x + 12, y + 16)).collect::<Vec<_>>()),
            "note": "Actual host presenter (observe/draw) with its real wind packets and art phase; 'old' is the same pack with corner_cap_owner cleared in memory. Not an ecological recording when synthetic.",
        });
        fs::write(o.join("report.json"), serde_json::to_string_pretty(&report).unwrap()).unwrap();
        println!("wrote {} frame sets to {}; peak step old {:?} new {:?}", timeline.len(), o.display(), peak_old, peak_new);
    }
}
