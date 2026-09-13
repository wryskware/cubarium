//! Explicit timing study for crowded *transitions*, not just fully grown snapshots.
//! Run in release, alone: cargo test --release -p cubarium --test animation_load
//! -- --ignored --nocapture --test-threads=1
//! These timings depend on the host and are not ordinary correctness tests.

use std::{hint::black_box, path::Path, time::Instant};

use cubarium::{art::ArtPack, art_present::ArtPresenter};
use cubarium_core::{
    OrganismId,
    organism::Mode,
    view::{OrganismView, RenderView},
};
use cubarium_render::Canvas;
use cubarium_surface::{CELL_COUNT, Face, SurfacePoint, Vec2};

fn transition_draw_cost(growing: bool, wet: bool) {
    let mut view = RenderView {
        tick: 121, // Six simulated seconds: inside a gust, not its quiet interval.
        producer: vec![if growing { 0.0 } else { 10.0 }; CELL_COUNT],
        detritus: vec![if growing { 0.0 } else { 1.5 }; CELL_COUNT],
        fruit: vec![if growing { 0.0 } else { 1.0 }; CELL_COUNT],
        water: vec![if wet { 1.0 } else { 0.0 }; CELL_COUNT],
        rain: vec![1.0; CELL_COUNT],
        producer_max: 10.0,
        organisms: (0..200u32)
            .map(|slot| OrganismView {
                id: OrganismId {
                    slot,
                    generation: 1,
                },
                pos: SurfacePoint::new(
                    Face::ALL[(slot % 5) as usize],
                    (slot % 13) as f64 * 4.5 + 3.0,
                    (slot % 11) as f64 * 5.5 + 3.0,
                ),
                heading: Vec2::new(1.0, 0.0),
                lobes: vec![],
                hue: (slot % 7) as f32 / 7.0,
                mode: Mode::Seeking,
                fed: false,
                juvenile: slot % 3 == 0,
                gestation: (slot % 5 == 0).then_some(0.5),
                form: (slot % 4) as u8,
                moved: Vec::new(),
            })
            .collect(),
    };
    let pack = ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"))
        .expect("shipped art pack");
    let mut presenter = ArtPresenter::new(pack);
    let mut canvas = Canvas::new();
    presenter.observe(&view);
    for _ in 0..5 {
        presenter.draw(&view, 0.0, &mut canvas);
    }
    view.producer.fill(if growing { 10.0 } else { 0.0 });
    view.detritus.fill(if growing { 1.5 } else { 0.0 });
    view.fruit.fill(if growing { 1.0 } else { 0.0 });

    // Three interpolated renders per ecology tick, twelve seconds of growth/wilting.
    // Observe is outside the measured span: compare these draw timings with the existing
    // mature-scene fixtures; neither measures the browser/transport end-to-end budget.
    let mut buckets = [0.0; 12];
    for frame in 0..720usize {
        view.tick = 122 + (frame / 3) as u64;
        if frame % 3 == 0 {
            presenter.observe(&view);
        }
        let began = Instant::now();
        presenter.draw(&view, (frame % 3) as f64 / 3.0, &mut canvas);
        black_box(&canvas);
        buckets[frame / 60] += began.elapsed().as_secs_f64() * 1_000.0 / 60.0;
    }
    let mean = buckets.iter().sum::<f64>() / buckets.len() as f64;
    let worst = buckets.iter().copied().fold(0.0, f64::max);
    println!(
        "{} / {} / rain / 200 bodies: mean {mean:.3} ms/frame; worst one-second mean {worst:.3}; buckets {buckets:.3?}",
        if growing { "growing" } else { "wilting" },
        if wet { "wet" } else { "dry" },
    );
    assert!(
        worst < 1_000.0 / 60.0,
        "crowded transition exceeds the draw budget: {worst:.3} ms"
    );
}

#[test]
#[ignore = "release-only timing study; run alone with --test-threads=1"]
fn crowded_growth_including_authored_clip_draw_cost() {
    transition_draw_cost(true, false);
}

#[test]
#[ignore = "release-only timing study; run alone with --test-threads=1"]
fn crowded_wet_decline_draw_cost() {
    transition_draw_cost(false, true);
}
