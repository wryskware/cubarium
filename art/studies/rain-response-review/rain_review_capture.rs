//! Rain-response playback review capture (`art/studies/rain-response-review`): one fixture
//! stepped once and drawn by up to three presenters on identical frames — the original
//! (response off), the candidate built into this source copy (on), and optionally a
//! presenter *constructed* part-way through the run (`--restart-at`), to show what a host
//! restart mid-rain or just after rain looks like, since presentation history is not
//! persisted. Built and run inside a frozen source copy the review's `run.sh` prepares;
//! it never touches production sources.
//!
//! ```text
//! rain_review_capture --out DIR --from A --to B
//!     ( --world OPENING.cubw --ticks N [--rain-at T (--target K | --at FACE,U,V)]
//!     | --synthetic FACE,CX,CY [--rain-at T] [--rain-ticks N] )
//!     [--restart-at T]
//! ```
//!
//! Writes `old/`, `new/` (and `restart/` from T on) native 256×128 nets, three per tick
//! for ticks `A..=B`, `timeline.json` (per tick: rain, and for the frame window every
//! rained or quivering cell's rate, level, species and stage for `new` and `restart`),
//! `stages.json` and `report.json`. Refuses an existing output directory.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cube_proto::Frame;
use cubarium::art::ArtPack;
use cubarium::art_present::{ArtPresenter, SOIL_SCALE, band_of, rain_response, species_of};
use cubarium::net::net_rgb8;
use cubarium::present::PRODUCER_SATURATION;
use cubarium::sink::png::write_net_png;
use cubarium_core::care::{CareCommand, CareKind, CareTarget};
use cubarium_core::view::RenderView;
use cubarium_core::{World, decode_snapshot};
use cubarium_render::Canvas;
use cubarium_surface::{CELL_COUNT, CellId};

const TARGETS: [CareTarget; 3] = [
    CareTarget { face: 0, u: 32.0, v: 48.0 },
    CareTarget { face: 0, u: 63.5, v: 48.0 },
    CareTarget { face: 1, u: 32.0, v: 63.5 },
];
const FRAMES_PER_TICK: u64 = 3;
const PRODUCER_MAX: f64 = 10.0;

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn parse_list<T: std::str::FromStr>(s: &str, what: &str) -> Vec<T> {
    s.split(',').map(|x| x.trim().parse().unwrap_or_else(|_| panic!("{what}"))).collect()
}

fn presenter(atelier: &Path) -> ArtPresenter {
    ArtPresenter::new(ArtPack::load(atelier).expect("pack"))
}

/// The synthetic rich field: every slot at its cap, no water, and a Standard-shaped
/// shower (0.235 / 0.118 / 0.078 d/s by Chebyshev hop, sin² envelope over `rain_ticks`)
/// on the 3×3 patch around `centre` from `rain_at`.
fn synthetic_view(tick: u64, centre: CellId, rain_at: u64, rain_ticks: u64) -> RenderView {
    let mut v = RenderView {
        tick,
        producer: vec![PRODUCER_MAX * PRODUCER_SATURATION; CELL_COUNT],
        detritus: vec![SOIL_SCALE; CELL_COUNT],
        fruit: vec![0.0; CELL_COUNT],
        water: vec![0.0; CELL_COUNT],
        rain: vec![0.0; CELL_COUNT],
        producer_max: PRODUCER_MAX,
        organisms: Vec::new(),
    };
    if (rain_at..rain_at + rain_ticks).contains(&tick) {
        let phase = (tick - rain_at) as f64 / rain_ticks as f64;
        let env = (std::f64::consts::PI * phase).sin().powi(2);
        for (index, cell) in CellId::all().enumerate() {
            if cell.face() != centre.face() {
                continue;
            }
            let d = (i32::from(cell.cx()) - i32::from(centre.cx())).abs().max((i32::from(cell.cy()) - i32::from(centre.cy())).abs());
            let peak = match d {
                0 => 0.235,
                1 => 0.118,
                2 => 0.078,
                _ => 0.0,
            };
            v.rain[index] = (peak * env) as f32;
        }
    }
    v
}

fn cells_json(view: &RenderView, p: &ArtPresenter) -> Vec<serde_json::Value> {
    let mut cells = Vec::new();
    for (index, cell) in CellId::all().enumerate() {
        let (prev, level) = p.rain_level_of(cell);
        let rate = view.rain[index];
        if rate <= 0.0 && level <= 0.0 && prev <= 0.0 {
            continue;
        }
        let species = species_of(band_of(cell), cell);
        cells.push(serde_json::json!({
            "face": cell.face().index(), "cx": cell.cx(), "cy": cell.cy(), "index": index,
            "rate": rate, "level_prev": prev, "level": level,
            "species": species, "responds": rain_response(species) > 0.0, "stage": p.stage_of(cell),
        }));
    }
    cells
}

fn write_stages(out: &Path, presenter: &ArtPresenter) {
    let mut planted = Vec::new();
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

fn main() {
    let out = PathBuf::from(arg("--out").expect("--out DIR"));
    let from: u64 = arg("--from").map_or(u64::MAX, |s| s.parse().expect("--from"));
    let to: u64 = arg("--to").map_or(0, |s| s.parse().expect("--to"));
    let rain_at: Option<u64> = arg("--rain-at").map(|s| s.parse().expect("--rain-at"));
    let restart_at: Option<u64> = arg("--restart-at").map(|s| s.parse().expect("--restart-at"));
    std::fs::create_dir(&out).expect("--out must be a fresh directory with an existing parent");
    let atelier = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier");
    let mut old = presenter(&atelier).without_rain_response();
    let mut new = presenter(&atelier);
    let mut restart: Option<ArtPresenter> = None;
    for label in ["old", "new"] {
        std::fs::create_dir_all(out.join(label)).expect("out dir");
    }
    if restart_at.is_some() {
        std::fs::create_dir_all(out.join("restart")).expect("out dir");
    }

    // The fixture: a recorded world stepped live, or the synthetic field.
    let synthetic: Option<(CellId, u64, u64)> = arg("--synthetic").map(|s| {
        let p: Vec<u8> = parse_list(&s, "--synthetic FACE,CX,CY");
        let centre = CellId::all().find(|c| c.face().index() == usize::from(p[0]) && c.cx() == p[1] && c.cy() == p[2]).expect("a cell");
        (centre, rain_at.unwrap_or(600), arg("--rain-ticks").map_or(120, |s| s.parse().expect("--rain-ticks")))
    });
    let mut world: Option<World> = None;
    let mut meta_schema = None;
    let mut opening_hash = None;
    let world_path = arg("--world");
    let ticks: u64 = match &synthetic {
        Some(_) => to + 1,
        None => arg("--ticks").map_or(to + 1, |s| s.parse().expect("--ticks")),
    };
    if synthetic.is_none() {
        let bytes = std::fs::read(world_path.as_ref().expect("--world or --synthetic")).expect("reading the opening");
        let (meta, state) = decode_snapshot(&bytes).expect("decoding the opening");
        let w = World::from_state(state).expect("a valid world");
        opening_hash = Some(format!("{:016x}", cubarium_core::snapshot::state_hash(&w.state)));
        meta_schema = Some(meta.schema);
        world = Some(w);
    }
    let chart = match arg("--at") {
        Some(s) => {
            let p: Vec<f64> = parse_list(&s, "--at FACE,U,V");
            CareTarget { face: p[0] as u8, u: p[1], v: p[2] }
        }
        None => TARGETS[arg("--target").map_or(0, |s| s.parse().expect("--target"))],
    };

    let mut timeline: Vec<serde_json::Value> = Vec::new();
    let mut receipt = serde_json::Value::Null;
    let mut canvas = Canvas::new();
    let mut frame = Frame::black();
    let mut rgb = Vec::new();
    let mut written = 0u64;
    let mut first_natural: Option<u64> = None;
    for elapsed in 0..ticks {
        let view = match (&synthetic, world.as_mut()) {
            (Some((centre, at, len)), _) => synthetic_view(elapsed, *centre, *at, *len),
            (None, Some(w)) => {
                if rain_at == Some(elapsed) {
                    let cmd = CareCommand::standard(w.care().admitted_seq + 1, w.tick(), CareKind::Rain, chart);
                    receipt = serde_json::to_value(w.apply_care(&cmd)).unwrap_or(serde_json::Value::Null);
                }
                w.step();
                w.drain_events();
                w.drain_hunter_events();
                w.render_view()
            }
            _ => unreachable!(),
        };
        if restart_at == Some(elapsed) {
            restart = Some(presenter(&atelier));
        }
        old.observe(&view);
        new.observe(&view);
        if let Some(r) = restart.as_mut() {
            r.observe(&view);
        }
        let max_rate = view.rain.iter().copied().fold(0.0f32, f32::max);
        if max_rate > 0.0 && rain_at.is_none_or(|t| elapsed < t) && first_natural.is_none() {
            first_natural = Some(elapsed);
        }
        let in_window = (from..=to).contains(&elapsed);
        let mut entry = serde_json::json!({
            "elapsed": elapsed, "tick": view.tick, "max_rate": max_rate,
            "rained_cells": view.rain.iter().filter(|&&r| r > 0.0).count(),
        });
        if in_window {
            entry["cells"] = serde_json::Value::Array(cells_json(&view, &new));
            if let Some(r) = restart.as_ref() {
                entry["restart_cells"] = serde_json::Value::Array(cells_json(&view, r));
            }
            for k in 0..FRAMES_PER_TICK {
                let f = k as f64 / FRAMES_PER_TICK as f64;
                let name = format!("frame_{:05}.png", elapsed * FRAMES_PER_TICK + k);
                let mut draw = |label: &str, p: &mut ArtPresenter| {
                    canvas.clear();
                    p.draw(&view, f, &mut canvas);
                    canvas.encode(&mut frame);
                    rgb.clear();
                    net_rgb8(&frame, &mut rgb);
                    write_net_png(&out.join(label).join(&name), &rgb).expect("png");
                };
                draw("old", &mut old);
                draw("new", &mut new);
                if let Some(r) = restart.as_mut() {
                    draw("restart", r);
                }
                written += 1;
            }
        }
        timeline.push(entry);
    }
    std::fs::write(out.join("timeline.json"), serde_json::to_string(&timeline).unwrap()).expect("timeline");
    write_stages(&out, &new);
    let report = serde_json::json!({
        "kind": "rain-response-review-capture-v1",
        "fixture": match &synthetic {
            Some((c, at, len)) => serde_json::json!({"synthetic": {"face": c.face().index(), "cx": c.cx(), "cy": c.cy(), "rain_at": at, "rain_ticks": len}}),
            None => serde_json::json!({"world": world_path, "input_schema": meta_schema, "opening_hash": opening_hash, "ticks": ticks, "rain_at": rain_at,
                "target_chart": rain_at.map(|_| serde_json::json!({"face": chart.face, "u": chart.u, "v": chart.v})), "receipt": receipt, "first_natural_rain_elapsed": first_natural}),
        },
        "restart_at": restart_at,
        "frames_from": from, "frames_to": to, "frames_per_tick": FRAMES_PER_TICK, "frame_sets_written": written,
        "closing_hash": world.as_ref().map(|w| format!("{:016x}", cubarium_core::snapshot::state_hash(&w.state))),
        "note": "Presentation-only: one fixture stepped once, drawn by the original presenter (response off), the candidate in this source copy (on), and optionally a presenter constructed at restart_at. The rain field is what actually fell each tick.",
    });
    std::fs::write(out.join("report.json"), serde_json::to_string_pretty(&report).unwrap()).expect("report");
    println!("{written} frame sets to {}", out.display());
}
