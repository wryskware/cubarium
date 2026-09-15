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

/// Living wood at `W_max`, which is what the structural read calls a full-grown stand since
/// ecology v1.
const WOOD_MAX: f64 = 0.6;
/// The foliage a full stand carries in these fixtures: `P / W` above the fullness shoulder,
/// so `fullness` below decides whether the silhouette layer is stamped at all.
const FULL_FOLIAGE: f64 = WOOD_MAX;

/// `fullness` is the cell's `P / W`: 1 or more is an ungrazed stand, which stamps exactly
/// what this presenter stamped before ecology v1; anything between 0 and the shoulder stamps
/// the living-wood silhouette *as well*, which is this feature's worst case per cell.
/// `dead` adds standing dead wood in every cell on top, a third stamp.
fn transition_draw_cost_at(growing: bool, wet: bool, fullness: f64, dead: f64, what: &str) {
    let mut view = RenderView {
        tick: 121, // Six simulated seconds: inside a gust, not its quiet interval.
        producer: vec![
            if growing { 0.0 } else { fullness * WOOD_MAX };
            CELL_COUNT
        ],
        detritus: vec![if growing { 0.0 } else { 1.5 }; CELL_COUNT],
        fruit: vec![if growing { 0.0 } else { 1.0 }; CELL_COUNT],
        wood: vec![if growing { 0.0 } else { WOOD_MAX }; CELL_COUNT],
        plant_reserve: vec![0.0; CELL_COUNT],
        dead_wood: vec![if growing { 0.0 } else { dead }; CELL_COUNT],
        carrion: vec![0.0; CELL_COUNT],
        water: vec![if wet { 1.0 } else { 0.0 }; CELL_COUNT],
        rain: vec![1.0; CELL_COUNT],
        producer_max: 10.0,
        wood_max: 0.6,
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
    view
        .producer
        .fill(if growing { fullness * WOOD_MAX } else { 0.0 });
    view.wood.fill(if growing { WOOD_MAX } else { 0.0 });
    view.dead_wood.fill(if growing { dead } else { 0.0 });
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
        "{} / {} / {what} / rain / 200 bodies: mean {mean:.3} ms/frame; worst one-second mean {worst:.3}; buckets {buckets:.3?}",
        if growing { "growing" } else { "wilting" },
        if wet { "wet" } else { "dry" },
    );
    assert!(
        worst < 1_000.0 / 60.0,
        "crowded transition exceeds the draw budget: {worst:.3} ms"
    );
}

/// The pre-ecology comparison point: every stand ungrazed, so no silhouette is stamped and
/// the frame is the one this presenter drew before the feature existed.
fn transition_draw_cost(growing: bool, wet: bool) {
    transition_draw_cost_at(growing, wet, FULL_FOLIAGE / WOOD_MAX, 0.0, "full canopy");
}

/// Ecology v1's worst case: every cell half-grazed, so every cell stamps its foliage *and*
/// the living-wood silhouette under it.
#[test]
#[ignore = "release-only timing study; run alone with --test-threads=1"]
fn crowded_half_grazed_draw_cost() {
    transition_draw_cost_at(true, false, 0.5, 0.0, "half-grazed");
}

/// Worse still, and not a state the ecology sustains: every cell half-grazed *and* carrying
/// standing dead wood, so every cell stamps three times.
#[test]
#[ignore = "release-only timing study; run alone with --test-threads=1"]
fn crowded_half_grazed_over_dead_wood_draw_cost() {
    transition_draw_cost_at(true, false, 0.5, WOOD_MAX, "half-grazed over dead wood");
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
