//! Paired native capture of the meal onset (`cubarium::meal_present`) on a **recorded**
//! world: the same opening, the same single Feed pulse (or none), the same ticks, drawn
//! twice — with the meal onset and without it — so a reviewer compares the presentation
//! alone on one naturally occurring trajectory. Not a behaviour test.
//!
//! ```text
//! meal_capture --world captures/hunter-openings-2026-09-13/seed-1/world-144000.cubw \
//!     --feed-at 600 --target 0 --ticks 1200 --from 560 --to 900 --out DIR
//! ```
//!
//! Writes, under `DIR`: `old/frame_NNNNN.png` and `new/frame_NNNNN.png` (native 256×128
//! nets, three per tick from `--from` to `--to`), `organisms.jsonl` (every organism every
//! tick: id, face, u, v, mode, fed, and the presenter's meal memory), `bouts.json` (per id:
//! the onsets seen, intake gap lengths inside and between bouts) and `report.json`.
//! The Feed pulse is the screen's own: one standard dose at the prescribed target
//! (`0` Front interior (32, 48), `1` Front seam (63.5, 48), `2` Right rim (32, 63.5)),
//! applied before stepping tick `--feed-at`; `--feed-at` absent means no input at all.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

use cubarium::art::ArtPack;
use cubarium::art_present::ArtPresenter;
use cubarium::net::net_rgb8;
use cubarium::sink::png::write_net_png;
use cubarium_core::care::{CareCommand, CareKind, CareTarget};
use cubarium_core::{World, decode_snapshot, encode_snapshot};
use cubarium_render::Canvas;
use cube_proto::Frame;

const TARGETS: [CareTarget; 3] = [
    CareTarget {
        face: 0,
        u: 32.0,
        v: 48.0,
    },
    CareTarget {
        face: 0,
        u: 63.5,
        v: 48.0,
    },
    CareTarget {
        face: 1,
        u: 32.0,
        v: 63.5,
    },
];
const FRAMES_PER_TICK: u64 = 3;
const BUILD: &str = concat!(env!("CARGO_PKG_VERSION"), "+", env!("CUBARIUM_GIT_HASH"));

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let world_path = arg("--world").expect("--world PATH.cubw");
    let out = PathBuf::from(arg("--out").expect("--out DIR"));
    let ticks: u64 = arg("--ticks").map_or(900, |s| s.parse().expect("--ticks"));
    let feed_at: Option<u64> = arg("--feed-at").map(|s| s.parse().expect("--feed-at"));
    let target: usize = arg("--target").map_or(0, |s| s.parse().expect("--target"));
    let from: u64 = arg("--from").map_or(0, |s| s.parse().expect("--from"));
    let to: u64 = arg("--to").map_or(ticks.saturating_sub(1), |s| s.parse().expect("--to"));
    assert!(
        ticks > 0 && from <= to && to < ticks,
        "capture range must lie inside the run"
    );
    assert!(target < TARGETS.len(), "unknown target");
    assert!(
        feed_at.is_none_or(|t| t < ticks),
        "Feed must lie inside the run"
    );
    let bytes = std::fs::read(&world_path).expect("reading the opening");
    let (meta, state) = decode_snapshot(&bytes).expect("decoding the opening");
    let mut world = World::from_state(state).expect("a valid world");
    let opening_hash = cubarium_core::snapshot::state_hash(&world.state);
    let start_tick = world.tick();
    let atelier = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier");
    let pack = ArtPack::load(&atelier).expect("pack");
    let pack_creatures = pack.creature_count();
    // `(looping, seconds)` of each rig's feed clip, enough for `clip_time`.
    let feed_clips: Vec<cubarium::art::Clip> = (0..pack_creatures)
        .map(|rig| {
            let c = &pack.clips[rig * 4 + cubarium::art_present::FEED_STATE];
            cubarium::art::Clip {
                frames: Vec::new(),
                seconds: c.seconds,
                looping: c.looping,
            }
        })
        .collect();
    let mut phases: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut new = ArtPresenter::new(pack);
    let mut old = ArtPresenter::new(ArtPack::load(&atelier).expect("pack")).without_meal_onset();
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).expect("output parent");
    }
    std::fs::create_dir(&out)
        .expect("new output directory (existing evidence is never overwritten)");
    for label in ["old", "new"] {
        std::fs::create_dir(out.join(label)).expect("out dir");
    }
    let mut organisms = std::io::BufWriter::new(
        std::fs::File::create(out.join("organisms.jsonl")).expect("organisms.jsonl"),
    );
    // Per id: (onsets in presentation seconds, gaps between fed ticks in ticks, first tick fed seen).
    let mut bouts: BTreeMap<String, (Vec<f64>, Vec<u64>, Option<u64>)> = BTreeMap::new();
    let mut receipt = serde_json::Value::Null;
    let mut canvas = Canvas::new();
    let mut frame = Frame::black();
    let mut rgb = Vec::new();
    let mut written = 0u64;
    for elapsed in 0..ticks {
        if feed_at == Some(elapsed) {
            let cmd = CareCommand::standard(
                world.care().admitted_seq + 1,
                world.tick(),
                CareKind::Feed,
                TARGETS[target],
            );
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
            p.observe_hunters(&view, &hunters, &hunted)
                .expect("drawable");
        }
        for o in &view.organisms {
            let key = format!("{}:{}", o.id.slot, o.id.generation);
            let entry = bouts
                .entry(key.clone())
                .or_insert((Vec::new(), Vec::new(), None));
            if o.fed {
                if let Some(last) = entry.2 {
                    if view.tick > last + 1 {
                        entry.1.push(view.tick - last - 1);
                    }
                }
                entry.2 = Some(view.tick);
            }
            let m = new.meal_of(o.id);
            if let Some(m) = m {
                if let Some(onset) = m.onset {
                    if entry.0.last() != Some(&onset) {
                        entry.0.push(onset);
                        // Where the shared-phase loop stood at the onset: the seconds into the
                        // 2 s feed cycle the old presentation was showing when the meal began
                        // (its bite peaks at 0.5 s and 1.5 s of the cycle).
                        let rig = cubarium::art_present::rig_of(o.form, o.hue, pack_creatures);
                        let clip = &feed_clips[rig];
                        let shared = cubarium::art_present::clip_time(
                            clip,
                            onset,
                            cubarium::art_present::phase_of(o.id, clip.seconds),
                            None,
                        )
                        .rem_euclid(clip.seconds);
                        phases
                            .entry(key.clone())
                            .or_insert_with(Vec::new)
                            .push(shared);
                    }
                }
            }
            if (from..=to).contains(&elapsed) {
                writeln!(
                    organisms,
                    "{}",
                    serde_json::json!({
                        "elapsed": elapsed, "tick": view.tick, "id": key, "face": o.pos.face.index(), "u": o.pos.u, "v": o.pos.v,
                        "mode": format!("{:?}", o.mode), "fed": o.fed, "gestation": o.gestation, "juvenile": o.juvenile, "form": o.form,
                        "onset": m.and_then(|m| m.onset), "weight": m.map(|m| m.weight), "weight_prev": m.map(|m| m.weight_prev),
                        "moved_px": o.moved.iter().map(|s| s.length()).sum::<f64>(),
                    })
                )
                .expect("write");
            }
        }
        if (from..=to).contains(&elapsed) {
            for k in 0..FRAMES_PER_TICK {
                let f = k as f64 / FRAMES_PER_TICK as f64;
                for (label, p) in [("old", &mut old), ("new", &mut new)] {
                    canvas.clear();
                    p.draw(&view, f, &mut canvas);
                    canvas.encode(&mut frame);
                    rgb.clear();
                    net_rgb8(&frame, &mut rgb);
                    write_net_png(
                        &out.join(label)
                            .join(format!("frame_{:05}.png", elapsed * FRAMES_PER_TICK + k)),
                        &rgb,
                    )
                    .expect("png");
                }
                written += 1;
            }
        }
    }
    organisms.flush().expect("flush");
    std::fs::write(
        out.join("closing.cubw"),
        encode_snapshot(&world.state, BUILD),
    )
    .expect("closing snapshot");
    let bouts_json: BTreeMap<&String, serde_json::Value> = bouts
        .iter()
        .map(|(k, (onsets, gaps, last))| (k, serde_json::json!({"onsets": onsets, "shared_phase_at_onset": phases.get(k), "gaps_ticks": gaps, "last_fed_tick": last})))
        .collect();
    std::fs::write(
        out.join("bouts.json"),
        serde_json::to_string_pretty(&bouts_json).unwrap(),
    )
    .expect("bouts");
    let report = serde_json::json!({
        "kind": "meal-onset-paired-capture-v1",
        "build": BUILD, "opening_hash": format!("{opening_hash:016x}"),
        "closing_hash": format!("{:016x}", cubarium_core::snapshot::state_hash(&world.state)),
        "world": world_path, "input_schema": meta.schema, "start_tick": start_tick, "ticks": ticks,
        "feed_at": feed_at, "target": feed_at.map(|_| target), "target_chart": feed_at.map(|_| serde_json::json!({"face": TARGETS[target].face, "u": TARGETS[target].u, "v": TARGETS[target].v})),
        "receipt": receipt, "frames_from": from, "frames_to": to, "frames_per_tick": FRAMES_PER_TICK, "frame_pairs_written": written,
        "population_end": world.population(), "settle_seconds": cubarium::meal_present::MEAL_SETTLE_SECONDS,
        "note": "Presentation-only paired capture on one recorded trajectory; the world is stepped once and drawn twice. fed is any field intake, not manual-crumb ingestion.",
    });
    std::fs::write(
        out.join("report.json"),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .expect("report");
    println!(
        "{} frame pairs to {}; {} ids seen; population {}",
        written,
        out.display(),
        bouts.len(),
        world.population()
    );
}
