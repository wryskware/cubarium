//! Public-API regressions for presentation history and visible transition boundaries.

use std::path::Path;

use cubarium::art::{ArtPack, Band};
use cubarium::art_present::{
    ArtPresenter, STAGE_HYST, band_of, plant_cap, present_seconds, stage_thresholds, tall_rise,
};
use cubarium::present::PRODUCER_SATURATION;
use cubarium_core::view::RenderView;
use cubarium_render::Canvas;
use cubarium_surface::{CELL_COUNT, CellId};

const PRODUCER_MAX: f64 = 10.0;

fn pack() -> ArtPack {
    ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"))
        .expect("checked-in art pack must load")
}

fn view(tick: u64, density: f64) -> RenderView {
    RenderView {
        tick,
        producer: vec![density * PRODUCER_MAX * PRODUCER_SATURATION; CELL_COUNT],
        detritus: vec![0.0; CELL_COUNT],
        fruit: vec![0.0; CELL_COUNT],
        water: vec![0.0; CELL_COUNT],
        rain: vec![0.0; CELL_COUNT],
        producer_max: PRODUCER_MAX,
        organisms: Vec::new(),
    }
}

fn full_foliage_slot() -> CellId {
    CellId::all()
        .find(|&cell| band_of(cell) == Band::Foliage && plant_cap(Band::Foliage, cell) == Some(2))
        .expect("the fixture has a slot capable of all stages")
}

#[test]
fn tick_zero_holds_the_initial_presentation_time_at_every_fraction() {
    for fraction in [0.0, 0.25, 0.5, 0.99, 1.0] {
        assert_eq!(
            present_seconds(0, fraction),
            present_seconds(0, 0.0),
            "there is no completed tick to interpolate at startup"
        );
    }
}

#[test]
fn observing_a_completed_stage_twice_does_not_start_the_next_stage() {
    let mut presenter = ArtPresenter::new(pack());
    let cell = full_foliage_slot();
    presenter.observe(&view(0, 0.0));
    // Detect the actual completion through public state; do not duplicate its timing
    // arithmetic or assume a particular tuned duration.
    for tick in 1..=1000 {
        let rich = view(tick, 1.0);
        presenter.observe(&rich);
        let completed = presenter.growth_of(cell);
        if completed.from == Some(0) && completed.to == Some(0) {
            assert_eq!(
                completed.target,
                Some(2),
                "fixture must still have another stage to grow"
            );
            presenter.observe(&rich);
            assert_eq!(
                presenter.growth_of(cell),
                completed,
                "observing the same completed tick must be idempotent, including at a stage boundary"
            );
            return;
        }
    }
    panic!("fixture did not complete its sprout stage within 1000 ticks");
}

#[test]
fn rewind_initialization_matches_a_fresh_presenter_inside_hysteresis() {
    let mut reused = ArtPresenter::new(pack());
    reused.observe(&view(100, 1.0));
    let cell = full_foliage_slot();
    assert_eq!(reused.stage_of(cell), Some(2));
    let density = stage_thresholds(Band::Foliage)[2] - STAGE_HYST * 0.5;
    let earlier = view(10, density);
    let mut fresh = ArtPresenter::new(pack());
    fresh.observe(&earlier);
    reused.observe(&earlier);
    assert_eq!(
        reused.growth_of(cell),
        fresh.growth_of(cell),
        "a replaced world's plant must not inherit another world's hysteresis"
    );
    // Exercise a threshold interval for tall plants independently of the small-plant
    // thresholds; restoring a view must have the same meaning for either morphology.
    let mut reused = ArtPresenter::new(pack());
    reused.observe(&view(100, 1.0));
    let earlier = view(10, tall_rise(2) - 0.01);
    let mut fresh = ArtPresenter::new(pack());
    fresh.observe(&earlier);
    reused.observe(&earlier);
    for i in 0..fresh.columns().len() {
        assert_eq!(reused.tall_growth_of(i), fresh.tall_growth_of(i));
    }
}

#[test]
fn drawing_at_30_60_or_120_fps_cannot_advance_growth_history() {
    let mut art = pack();
    // Growth state is independent of whether the optional decorative layers exist.
    // Keep this state-only fixture cheap enough to draw many times.
    art.plants.clear();
    art.tall.clear();
    art.ground.clear();
    let mut presenter = ArtPresenter::new(art);
    presenter.observe(&view(0, 0.0));
    let rich = view(1, 1.0);
    presenter.observe(&rich);
    let before: Vec<_> = CellId::all()
        .map(|cell| presenter.growth_of(cell))
        .collect();
    let tall_before: Vec<_> = (0..presenter.columns().len())
        .map(|i| presenter.tall_growth_of(i))
        .collect();
    let mut image = Canvas::new();
    for fps in [30, 60, 120] {
        for frame in 0..fps {
            presenter.draw(&rich, f64::from(frame) / f64::from(fps), &mut image);
        }
        let after: Vec<_> = CellId::all()
            .map(|cell| presenter.growth_of(cell))
            .collect();
        let tall_after: Vec<_> = (0..presenter.columns().len())
            .map(|i| presenter.tall_growth_of(i))
            .collect();
        assert_eq!(after, before, "{fps} fps mutated plant history");
        assert_eq!(tall_after, tall_before, "{fps} fps mutated column history");
    }
}
