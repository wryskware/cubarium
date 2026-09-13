//! Rain-response study capture (`art/studies/rain-response`): one recorded world, stepped
//! once, drawn twice — with the rain quiver and without — so a reviewer compares the
//! presentation alone on the same rain. Not a behaviour test; built and run inside the
//! isolated source copy the study's `run.sh` prepares.
//!
//! ```text
//! rain_response_capture --world OPENING.cubw --ticks 1200 --out DIR \
//!     [--rain-at 200 --target 0] [--from 180 --to 420]
//! ```
//!
//! With `--rain-at`, one Standard Rain (the care contract's own shower: 4 depth over 120
//! ticks, two graph hops) is applied at the prescribed target before stepping that tick;
//! without it the run carries only the world's natural weather. Writes `old/` and `new/`
//! native 256×128 nets, three per tick, for ticks `--from..=--to`; `timeline.json` with
//! every tick's rain (max rate, rained cells, applied receipt) and, for the frame window,
//! every rained or quivering cell's rate, level, species and stage; and `report.json`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cube_proto::Frame;
use cubarium::art::ArtPack;
use cubarium::art_present::{ArtPresenter, band_of, rain_response, species_of};
use cubarium::net::net_rgb8;
use cubarium::sink::png::write_net_png;
use cubarium_core::care::{CareCommand, CareKind, CareTarget};
use cubarium_core::{World, decode_snapshot};
use cubarium_render::Canvas;
use cubarium_surface::CellId;

const TARGETS: [CareTarget; 3] = [
    CareTarget { face: 0, u: 32.0, v: 48.0 },
    CareTarget { face: 0, u: 63.5, v: 48.0 },
    CareTarget { face: 1, u: 32.0, v: 63.5 },
];
const FRAMES_PER_TICK: u64 = 3;

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

/// Where the plants are at the end of a run: every slot with a grown stage, so a reviewer
/// can see which cells a shower could ever move (and aim `--at` one).
fn write_stages(out: &PathBuf, presenter: &ArtPresenter) {
    let mut planted: Vec<serde_json::Value> = Vec::new();
    let mut by_species: BTreeMap<String, usize> = BTreeMap::new();
    for cell in CellId::all() {
        if let Some(stage) = presenter.stage_of(cell) {
            let species = species_of(band_of(cell), cell);
            *by_species.entry(species.to_string()).or_default() += 1;
            planted.push(serde_json::json!({"face": cell.face().index(), "cx": cell.cx(), "cy": cell.cy(), "species": species, "stage": stage, "responds": rain_response(species) > 0.0}));
        }
    }
    std::fs::write(out.join("stages.json"), serde_json::to_string(&serde_json::json!({"by_species": by_species, "planted": planted})).unwrap()).expect("stages");
}

/// `--synthetic FACE,CX,CY`: no recorded world. Every slot is grown to its cap from a rich,
/// rainless field (500 ticks), then a Standard-shaped shower (0.235 d/s at the centre, 0.118
/// one hop out, 0.078 two hops out, a smooth 120-tick envelope) falls on the 3×3 patch
/// around the named cell. This is the *ceiling* of the response — mature plants under the
/// contract's own peak rates — which the sparse recorded openings never show.
fn synthetic(out: &PathBuf, at: (u8, u8, u8), from: u64, to: u64) {
    use cubarium::art_present::SOIL_SCALE;
    use cubarium::present::PRODUCER_SATURATION;
    use cubarium_core::view::RenderView;
    use cubarium_surface::CELL_COUNT;
    const PRODUCER_MAX: f64 = 10.0;
    const RAIN_AT: u64 = 600;
    let centre = CellId::all().find(|c| c.face().index() == usize::from(at.0) && c.cx() == at.1 && c.cy() == at.2).expect("a cell");
    let view_at = |tick: u64| {
        let mut v = RenderView { tick, producer: vec![PRODUCER_MAX * PRODUCER_SATURATION; CELL_COUNT], detritus: vec![SOIL_SCALE; CELL_COUNT], fruit: vec![0.0; CELL_COUNT], water: vec![0.0; CELL_COUNT], rain: vec![0.0; CELL_COUNT], producer_max: PRODUCER_MAX, organisms: Vec::new() };
        if (RAIN_AT..RAIN_AT + 120).contains(&tick) {
            let phase = (tick - RAIN_AT) as f64 / 120.0;
            let env = (std::f64::consts::PI * phase).sin().powi(2);
            for (index, cell) in CellId::all().enumerate() {
                if cell.face() != centre.face() { continue; }
                let d = (i32::from(cell.cx()) - i32::from(centre.cx())).abs().max((i32::from(cell.cy()) - i32::from(centre.cy())).abs());
                let peak = match d { 0 => 0.235, 1 => 0.118, 2 => 0.078, _ => 0.0 };
                v.rain[index] = (peak * env) as f32;
            }
        }
        v
    };
    let atelier = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier");
    let mut new = ArtPresenter::new(ArtPack::load(&atelier).expect("pack"));
    let mut old = ArtPresenter::new(ArtPack::load(&atelier).expect("pack")).without_rain_response();
    for label in ["old", "new"] {
        std::fs::create_dir_all(out.join(label)).expect("out dir");
    }
    let mut timeline: Vec<serde_json::Value> = Vec::new();
    let mut canvas = Canvas::new();
    let mut frame = Frame::black();
    let mut rgb = Vec::new();
    let mut written = 0u64;
    for elapsed in 0..to + 1 {
        let view = view_at(elapsed);
        old.observe(&view);
        new.observe(&view);
        let in_window = (from..=to).contains(&elapsed);
        let mut cells: Vec<serde_json::Value> = Vec::new();
        if in_window {
            for (index, cell) in CellId::all().enumerate() {
                let (prev, level) = new.rain_level_of(cell);
                let rate = view.rain[index];
                if rate <= 0.0 && level <= 0.0 && prev <= 0.0 { continue; }
                let species = species_of(band_of(cell), cell);
                cells.push(serde_json::json!({"face": cell.face().index(), "cx": cell.cx(), "cy": cell.cy(), "index": index, "rate": rate, "level_prev": prev, "level": level, "species": species, "responds": rain_response(species) > 0.0, "stage": new.stage_of(cell)}));
            }
            for k in 0..FRAMES_PER_TICK {
                let f = k as f64 / FRAMES_PER_TICK as f64;
                for (label, p) in [("old", &mut old), ("new", &mut new)] {
                    canvas.clear();
                    p.draw(&view, f, &mut canvas);
                    canvas.encode(&mut frame);
                    rgb.clear();
                    net_rgb8(&frame, &mut rgb);
                    write_net_png(&out.join(label).join(format!("frame_{:05}.png", elapsed * FRAMES_PER_TICK + k)), &rgb).expect("png");
                }
                written += 1;
            }
        }
        let max_rate = view.rain.iter().copied().fold(0.0f32, f32::max);
        timeline.push(serde_json::json!({"elapsed": elapsed, "tick": view.tick, "max_rate": max_rate, "rained_cells": view.rain.iter().filter(|&&r| r > 0.0).count(), "cells": cells}));
    }
    std::fs::write(out.join("timeline.json"), serde_json::to_string(&timeline).unwrap()).expect("timeline");
    write_stages(out, &new);
    let report = serde_json::json!({"kind": "rain-response-synthetic-capture-v1", "centre": {"face": at.0, "cx": at.1, "cy": at.2}, "rain_at": RAIN_AT, "frames_from": from, "frames_to": to, "frames_per_tick": FRAMES_PER_TICK, "frame_pairs_written": written, "note": "Synthetic rich field, every slot at its cap, no world stepped: the response ceiling on mature plants under Standard-shaped peak rates."});
    std::fs::write(out.join("report.json"), serde_json::to_string_pretty(&report).unwrap()).expect("report");
    println!("{written} synthetic frame pairs to {}", out.display());
}

fn main() {
    let out = PathBuf::from(arg("--out").expect("--out DIR"));
    if let Some(s) = arg("--synthetic") {
        let p: Vec<u8> = s.split(',').map(|x| x.trim().parse().expect("--synthetic FACE,CX,CY")).collect();
        let from: u64 = arg("--from").map_or(580, |s| s.parse().expect("--from"));
        let to: u64 = arg("--to").map_or(800, |s| s.parse().expect("--to"));
        std::fs::create_dir_all(&out).expect("out dir");
        synthetic(&out, (p[0], p[1], p[2]), from, to);
        return;
    }
    let world_path = arg("--world").expect("--world PATH.cubw");
    let ticks: u64 = arg("--ticks").map_or(1200, |s| s.parse().expect("--ticks"));
    let rain_at: Option<u64> = arg("--rain-at").map(|s| s.parse().expect("--rain-at"));
    let target: usize = arg("--target").map_or(0, |s| s.parse().expect("--target"));
    let from: u64 = arg("--from").map_or(u64::MAX, |s| s.parse().expect("--from"));
    let to: u64 = arg("--to").map_or(0, |s| s.parse().expect("--to"));
    assert!(target < TARGETS.len(), "unknown target");
    // `--at FACE,U,V` aims the shower anywhere on the surface instead of at one of the
    // care harness's three fixed targets: what a viewer's tap does.
    let chart = match arg("--at") {
        Some(s) => {
            let p: Vec<f64> = s.split(',').map(|x| x.trim().parse().expect("--at FACE,U,V")).collect();
            assert_eq!(p.len(), 3, "--at FACE,U,V");
            CareTarget { face: p[0] as u8, u: p[1], v: p[2] }
        }
        None => TARGETS[target],
    };
    let bytes = std::fs::read(&world_path).expect("reading the opening");
    let (meta, state) = decode_snapshot(&bytes).expect("decoding the opening");
    let mut world = World::from_state(state).expect("a valid world");
    let opening_hash = cubarium_core::snapshot::state_hash(&world.state);
    let start_tick = world.tick();
    let atelier = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier");
    let mut new = ArtPresenter::new(ArtPack::load(&atelier).expect("pack"));
    let mut old = ArtPresenter::new(ArtPack::load(&atelier).expect("pack")).without_rain_response();
    std::fs::create_dir_all(&out).expect("out dir");
    for label in ["old", "new"] {
        std::fs::create_dir_all(out.join(label)).expect("out dir");
    }
    let mut timeline: Vec<serde_json::Value> = Vec::new();
    let mut receipt = serde_json::Value::Null;
    let mut canvas = Canvas::new();
    let mut frame = Frame::black();
    let mut rgb = Vec::new();
    let mut written = 0u64;
    let mut first_natural: Option<u64> = None;
    for elapsed in 0..ticks {
        if rain_at == Some(elapsed) {
            let cmd = CareCommand::standard(world.care().admitted_seq + 1, world.tick(), CareKind::Rain, chart);
            let r = world.apply_care(&cmd);
            receipt = serde_json::to_value(&r).unwrap_or(serde_json::Value::Null);
        }
        world.step();
        world.drain_events();
        let hunted = world.drain_hunter_events();
        let view = world.render_view();
        let hunters = world.hunter_view();
        for p in [&mut old, &mut new] {
            p.observe(&view);
            p.observe_hunters(&view, &hunters, &hunted).expect("drawable");
        }
        let max_rate = view.rain.iter().copied().fold(0.0f32, f32::max);
        let rained = view.rain.iter().filter(|&&r| r > 0.0).count();
        if max_rate > 0.0 && rain_at.is_none_or(|t| elapsed < t) && first_natural.is_none() {
            first_natural = Some(elapsed);
        }
        let in_window = (from..=to).contains(&elapsed);
        let mut cells: Vec<serde_json::Value> = Vec::new();
        if in_window {
            for (index, cell) in CellId::all().enumerate() {
                let (prev, level) = new.rain_level_of(cell);
                let rate = view.rain[index];
                if rate <= 0.0 && level <= 0.0 && prev <= 0.0 {
                    continue;
                }
                let band = band_of(cell);
                let species = species_of(band, cell);
                cells.push(serde_json::json!({
                    "face": cell.face().index(), "cx": cell.cx(), "cy": cell.cy(), "index": index,
                    "rate": rate, "level_prev": prev, "level": level,
                    "species": species, "responds": rain_response(species) > 0.0,
                    "stage": new.stage_of(cell), "water": view.water[index],
                }));
            }
        }
        timeline.push(serde_json::json!({"elapsed": elapsed, "tick": view.tick, "max_rate": max_rate, "rained_cells": rained, "cells": cells}));
        if in_window {
            for k in 0..FRAMES_PER_TICK {
                let f = k as f64 / FRAMES_PER_TICK as f64;
                for (label, p) in [("old", &mut old), ("new", &mut new)] {
                    canvas.clear();
                    p.draw(&view, f, &mut canvas);
                    canvas.encode(&mut frame);
                    rgb.clear();
                    net_rgb8(&frame, &mut rgb);
                    write_net_png(&out.join(label).join(format!("frame_{:05}.png", elapsed * FRAMES_PER_TICK + k)), &rgb).expect("png");
                }
                written += 1;
            }
        }
    }
    let ecology_hash = cubarium_core::snapshot::state_hash(&world.state);
    std::fs::write(out.join("timeline.json"), serde_json::to_string(&timeline).unwrap()).expect("timeline");
    write_stages(&out, &new);
    let report = serde_json::json!({
        "kind": "rain-response-paired-capture-v1",
        "world": world_path, "input_schema": meta.schema, "start_tick": start_tick, "opening_hash": format!("{opening_hash:016x}"),
        "ticks": ticks, "rain_at": rain_at, "target": rain_at.map(|_| target),
        "target_chart": rain_at.map(|_| serde_json::json!({"face": chart.face, "u": chart.u, "v": chart.v})),
        "receipt": receipt, "first_natural_rain_elapsed": first_natural,
        "frames_from": from, "frames_to": to, "frames_per_tick": FRAMES_PER_TICK, "frame_pairs_written": written,
        "closing_hash": format!("{ecology_hash:016x}"),
        "note": "Presentation-only paired capture; the world is stepped once and drawn twice. The rain field is what actually fell this tick (natural weather and any applied shower); the quiver reads that field, never a requested dose.",
    });
    std::fs::write(out.join("report.json"), serde_json::to_string_pretty(&report).unwrap()).expect("report");
    println!("{written} frame pairs to {}; first natural rain at {:?}; closing hash {ecology_hash:016x}", out.display(), first_natural);
}
