//! Explicit timing study for the Lanternjaw rig. Run in release, alone:
//! `cargo test --release -p cubarium --test lanternjaw_cost -- --ignored --nocapture`
//!
//! These are host-dependent measurements, not correctness tests, so they are `#[ignore]`d.
//! They measure what one and two bodies cost per drawn frame — `Lanternjaw::draw`, which is
//! the per-frame rasterization of eight parts plus one root-owned `unfold_pixels` query and
//! its compositing — mid-face and with the anchor straddling the Front/Right seam (where the
//! query takes the general multi-chart path), in `move` and in `hunt` (whose strike is the
//! widest pose and its largest query). A desktop capture is not a hardware observation and
//! none of this measures the browser or shim transport.

use std::{hint::black_box, path::Path, time::Instant};

use cubarium::{
    art::ArtPack,
    art_present::ArtPresenter,
    lanternjaw::{Lanternjaw, Mode},
};
use cubarium_core::{
    OrganismId,
    organism::Mode as OrganismMode,
    view::{OrganismView, RenderView},
};
use cubarium_render::Canvas;
use cubarium_surface::{CELL_COUNT, Face, SurfacePoint, Vec2};

/// Ten seconds of presentation at 60 fps: a whole `move` gait and blink cycle and more than
/// one 6 s hunt cycle, so the strike and its widest query are inside every measurement.
const FRAMES: usize = 600;

/// Mid-face and seam-straddling anchors. The second body of a pair is a different sub-pixel
/// phase and a different row, so nothing is shared or cached between them.
fn anchors(seam: bool) -> [SurfacePoint; 2] {
    if seam {
        [
            SurfacePoint::new(Face::Front, 63.0, 32.0),
            SurfacePoint::new(Face::Front, 63.4, 48.7),
        ]
    } else {
        [
            SurfacePoint::new(Face::Front, 32.0, 32.0),
            SurfacePoint::new(Face::Front, 32.4, 48.7),
        ]
    }
}

/// Mean and worst microseconds per frame for `bodies` Lanternjaws drawn onto a cleared
/// canvas, over [`FRAMES`] frames of 60 fps presentation time.
fn draw_cost(rig: &Lanternjaw, bodies: usize, mode: Mode, seam: bool) -> (f64, f64) {
    let anchors = anchors(seam);
    let heading = Vec2::new(1.0, 0.0);
    let mut canvas = Canvas::new();
    let mut parts = Vec::new();
    let mut scratch = Vec::new();
    // Warm the allocations the same way a running route does before it is measured.
    for body in 0..bodies {
        rig.draw(
            &mut canvas, anchors[body], heading, 0.0, mode, 1.0, &mut parts, &mut scratch,
        );
    }
    let mut total = 0.0f64;
    let mut worst = 0.0f64;
    for frame in 0..FRAMES {
        let seconds = frame as f64 / 60.0;
        canvas.clear();
        let began = Instant::now();
        for body in 0..bodies {
            rig.draw(
                &mut canvas,
                anchors[body],
                heading,
                seconds,
                mode,
                1.0,
                &mut parts,
                &mut scratch,
            );
        }
        let elapsed = began.elapsed().as_secs_f64() * 1e6;
        black_box(canvas.get(Face::Front, 32, 32));
        total += elapsed;
        worst = worst.max(elapsed);
    }
    (total / FRAMES as f64, worst)
}

#[test]
#[ignore = "timing study: run explicitly in release"]
fn one_and_two_bodies_cost_per_frame() {
    let rig = Lanternjaw::new();
    println!("Lanternjaw::draw, {FRAMES} frames at 60 fps presentation time, release");
    println!("  place      mode  bodies    mean µs   worst µs");
    for seam in [false, true] {
        for mode in [Mode::Move, Mode::Hunt] {
            for bodies in [1usize, 2] {
                let (mean, worst) = draw_cost(&rig, bodies, mode, seam);
                println!(
                    "  {:9}  {:4}  {bodies:6}   {mean:8.1}   {worst:8.1}",
                    if seam { "seam" } else { "mid-face" },
                    if mode == Mode::Move { "move" } else { "hunt" },
                );
            }
        }
    }
}

/// A crowded art-mode world frame, built the way `animation_load.rs` builds one: 200
/// organisms, a mature producer/detritus/fruit field, water and rain, so the plants, the
/// ground and the water are all drawn.
fn dense_view() -> RenderView {
    RenderView {
        // Six simulated seconds: inside a gust, not its quiet interval.
        tick: 121,
        producer: vec![10.0; CELL_COUNT],
        detritus: vec![1.5; CELL_COUNT],
        fruit: vec![1.0; CELL_COUNT],
        water: vec![1.0; CELL_COUNT],
        rain: vec![1.0; CELL_COUNT],
        producer_max: 10.0,
        organisms: (0..200u32)
            .map(|slot| OrganismView {
                id: OrganismId { slot, generation: 1 },
                pos: SurfacePoint::new(
                    Face::ALL[(slot % 5) as usize],
                    (slot % 13) as f64 * 4.5 + 3.0,
                    (slot % 11) as f64 * 5.5 + 3.0,
                ),
                heading: Vec2::new(1.0, 0.0),
                lobes: vec![],
                hue: (slot % 7) as f32 / 7.0,
                mode: OrganismMode::Seeking,
                fed: false,
                juvenile: slot % 3 == 0,
                gestation: (slot % 5 == 0).then_some(0.5),
                form: (slot % 4) as u8,
                moved: Vec::new(),
            })
            .collect(),
    }
}

#[test]
#[ignore = "timing study: run explicitly in release"]
fn incremental_cost_on_a_dense_world_frame() {
    let pack = ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"))
        .expect("shipped art pack");
    let mut presenter = ArtPresenter::new(pack);
    let view = dense_view();
    let rig = Lanternjaw::new();
    let anchors = anchors(false);
    let heading = Vec2::new(1.0, 0.0);
    let mut canvas = Canvas::new();
    let mut parts = Vec::new();
    let mut scratch = Vec::new();
    presenter.observe(&view);
    for _ in 0..5 {
        presenter.draw(&view, 0.0, &mut canvas);
    }
    // Three measured passes: the world frame alone, and the same frame with one and with two
    // Lanternjaws drawn over it. `observe` is outside every measured span, and the frame
    // fraction walks so the presenter interpolates as it does live.
    let mut mean = [0.0f64; 3];
    let mut worst = [0.0f64; 3];
    for bodies in 0..3usize {
        let mut total = 0.0;
        for frame in 0..FRAMES {
            let f = (frame % 3) as f64 / 3.0;
            let seconds = frame as f64 / 60.0;
            let began = Instant::now();
            presenter.draw(&view, f, &mut canvas);
            for body in 0..bodies {
                rig.draw(
                    &mut canvas,
                    anchors[body],
                    heading,
                    seconds,
                    Mode::Hunt,
                    1.0,
                    &mut parts,
                    &mut scratch,
                );
            }
            let elapsed = began.elapsed().as_secs_f64() * 1e6;
            black_box(canvas.get(Face::Front, 32, 32));
            total += elapsed;
            worst[bodies] = worst[bodies].max(elapsed);
        }
        mean[bodies] = total / FRAMES as f64;
    }
    println!("Dense art-mode frame (200 organisms, mature fields, water, rain), release");
    println!("  bodies    mean µs   worst µs   mean increment µs");
    for bodies in 0..3usize {
        println!(
            "  {bodies:6}   {:8.1}   {:8.1}   {:+8.1}",
            mean[bodies],
            worst[bodies],
            mean[bodies] - mean[0]
        );
    }
    println!(
        "  16.7 ms frame budget: {:.1} % of it at two bodies",
        mean[2] / 16_700.0 * 100.0
    );
}
