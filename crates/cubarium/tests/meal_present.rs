//! The meal onset of ordinary fauna (`cubarium::meal_present`), written from its doc
//! comments and those of `ArtPresenter` (`observe`, `draw`, `without_meal_onset`,
//! `meal_of`), `state_of`, `clip_time`, `phase_of` and `present_seconds`: a bout of actual
//! intake reads the authored `feed` clip from its own onset, faded in and out, and nothing
//! else about a body changes. Every expected image is hand-built through the public stamp.
//!
//! Fixtures are synthetic published views (the world's own `OrganismView`), so a case that
//! never occurs naturally — a body reporting `Feeding` and no intake for seconds, an id
//! recycled with a new generation — is stated exactly. Nothing here is evidence about how
//! often meals happen; that is the paired capture's job.

use std::path::Path;

use cubarium::art::ArtPack;
use cubarium::art_present::{
    ArtPresenter, BODY_FADE_SECONDS, FEED_STATE, clip_time, phase_of, present_seconds, rig_of,
    state_of,
};
use cubarium::clock::DT;
use cubarium::meal_present::{MEAL_FADE_SECONDS, MEAL_SETTLE_SECONDS, MealMemory, Meals};
use cubarium_core::ids::OrganismId;
use cubarium_core::organism::Mode;
use cubarium_core::view::{OrganismView, RenderView};
use cubarium_render::{Canvas, Mask, Pose, stamp_layers};
use cubarium_surface::{CELL_COUNT, PathSegment, PixelImage, SurfacePoint, Vec2, travel};
use cube_proto::{FACE_SIZE, Face};

const PRODUCER_MAX: f64 = 10.0;

fn atelier() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier")
}

fn pack() -> ArtPack {
    ArtPack::load(&atelier()).expect("the baked pack at assets/atelier must load")
}

fn bare_view(tick: u64) -> RenderView {
    RenderView {
        tick,
        producer: vec![0.0; CELL_COUNT],
        detritus: vec![0.0; CELL_COUNT],
        fruit: vec![0.0; CELL_COUNT],
        water: vec![0.0; CELL_COUNT],
        rain: vec![0.0; CELL_COUNT],
        producer_max: PRODUCER_MAX,
        organisms: Vec::new(),
    }
}

fn organism(id: OrganismId, mode: Mode, fed: bool) -> OrganismView {
    OrganismView {
        id,
        pos: SurfacePoint::new(Face::Front, 30.0, 30.0),
        heading: Vec2::new(1.0, 0.0),
        lobes: vec![(0.0, 0.0, 2.0)],
        hue: 0.5,
        mode,
        fed,
        juvenile: false,
        gestation: None,
        form: u8::MAX,
        moved: Vec::new(),
    }
}

/// A view of one body at `tick`.
fn view_of(tick: u64, body: OrganismView) -> RenderView {
    let mut v = bare_view(tick);
    v.organisms = vec![body];
    v
}

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL.into_iter().flat_map(|f| {
        (0..FACE_SIZE as u8).flat_map(move |y| (0..FACE_SIZE as u8).map(move |x| (f, x, y)))
    })
}

fn differing(a: &Canvas, b: &Canvas) -> Vec<(Face, u8, u8)> {
    every_pixel()
        .filter(|&(f, x, y)| a.get(f, x, y) != b.get(f, x, y))
        .collect()
}

fn assert_same_canvas(a: &Canvas, b: &Canvas, what: &str) {
    let diff = differing(a, b);
    assert!(
        diff.is_empty(),
        "{what}: {} pixels differ, first at {:?}: {:?} vs {:?}",
        diff.len(),
        diff[0],
        a.get(diff[0].0, diff[0].1, diff[0].2),
        b.get(diff[0].0, diff[0].1, diff[0].2),
    );
}

fn draw(p: &mut ArtPresenter, v: &RenderView, f: f64) -> Canvas {
    let mut canvas = Canvas::new();
    p.draw(v, f, &mut canvas);
    canvas
}

/// The background at a tick: a presenter that has seen nothing but bare ground.
fn background(tick: u64) -> Canvas {
    let mut p = ArtPresenter::new(pack());
    let v = bare_view(tick);
    p.observe(&v);
    draw(&mut p, &v, 0.0)
}

/// The body hand-built from explicit clip readings: `(clip index, clip seconds, weight)`.
fn expected(
    art: &ArtPack,
    background: &Canvas,
    body: &OrganismView,
    readings: &[(usize, f64, f32)],
    f: f64,
) -> Canvas {
    let rig = rig_of(body.form, body.hue, art.creature_count());
    let poses: Vec<(Pose<'_>, f32)> = readings
        .iter()
        .map(|&(state, at, weight)| (art.clips[rig * 4 + state].sample(at), weight))
        .collect();
    let (anchor, facing) = cubarium::present::interpolate(&body.moved, body.pos, body.heading, f);
    let mut canvas = background.clone();
    let mut scratch: Vec<PixelImage> = Vec::new();
    stamp_layers(
        &mut canvas,
        anchor,
        facing,
        &poses,
        1.0,
        1.0,
        Mask::None,
        &mut scratch,
    );
    canvas
}

/// The shared-phase reading of a clip at `(tick, f)`: exactly [`clip_time`].
fn shared(art: &ArtPack, body: &OrganismView, state: usize, tick: u64, f: f64) -> f64 {
    let rig = rig_of(body.form, body.hue, art.creature_count());
    let clip = &art.clips[rig * 4 + state];
    clip_time(
        clip,
        present_seconds(tick, f),
        phase_of(body.id, clip.seconds),
        body.gestation,
    )
}

/// Observe `ticks` in a row, each with the body in `mode` and `fed` as the closure says.
fn run(
    p: &mut ArtPresenter,
    id: OrganismId,
    mode: Mode,
    ticks: std::ops::RangeInclusive<u64>,
    fed: impl Fn(u64) -> bool,
) -> RenderView {
    let mut last = None;
    for tick in ticks {
        let v = view_of(tick, organism(id, mode, fed(tick)));
        p.observe(&v);
        last = Some(v);
    }
    last.expect("at least one tick")
}

/// Ticks the settle window is, and ticks a whole fade takes, rounded up.
fn settle_ticks() -> u64 {
    (MEAL_SETTLE_SECONDS / DT).round() as u64
}
fn fade_ticks() -> u64 {
    (MEAL_FADE_SECONDS / DT).ceil() as u64
}

// ---------------------------------------------------------------------------

#[test]
fn a_feeding_body_with_no_intake_is_drawn_exactly_as_before() {
    let id = OrganismId {
        slot: 3,
        generation: 1,
    };
    let mut p = ArtPresenter::new(pack());
    let mut old = ArtPresenter::new(pack()).without_meal_onset();
    // Seeking, then Feeding for two seconds without a single intake tick.
    let t = 100u64;
    for tick in t..=t + 40 {
        let v = view_of(
            tick,
            organism(
                id,
                if tick < t + 3 {
                    Mode::Seeking
                } else {
                    Mode::Feeding
                },
                false,
            ),
        );
        p.observe(&v);
        old.observe(&v);
        for f in [0.0, 1.0 / 3.0, 2.0 / 3.0] {
            assert_same_canvas(
                &draw(&mut p, &v, f),
                &draw(&mut old, &v, f),
                &format!("tick {tick} f {f}: no intake, yet the meal changed the body"),
            );
        }
    }
    let m = p.meal_of(id).expect("a tracked body");
    assert_eq!(m.onset, None, "no intake, no onset");
    assert_eq!(m.last_fed, None);
    assert_eq!((m.weight_prev, m.weight), (0.0, 0.0));
    assert_eq!(
        state_of(&organism(id, Mode::Feeding, false)),
        FEED_STATE,
        "the fixture is on the feed clip"
    );
}

#[test]
fn a_sustained_bout_has_one_onset_fades_in_and_is_read_at_bout_time() {
    let art = pack();
    let id = OrganismId {
        slot: 4,
        generation: 2,
    };
    let mut p = ArtPresenter::new(pack());
    let mut old = ArtPresenter::new(pack()).without_meal_onset();
    let t = 200u64;
    // Feeding for a while without intake, then intake from `t + 10` on for two seconds.
    let onset_tick = t + 10;
    let fed = |tick: u64| tick >= onset_tick;
    let last = run(&mut p, id, Mode::Feeding, t..=onset_tick + 40, fed);
    run(&mut old, id, Mode::Feeding, t..=onset_tick + 40, fed);
    let m = p.meal_of(id).expect("tracked");
    assert_eq!(
        m.onset,
        Some(present_seconds(onset_tick, 0.0)),
        "one onset at the first intake tick"
    );
    assert_eq!(m.last_fed, Some(onset_tick + 40));
    assert_eq!(m.weight, 1.0, "the bout-time reading is fully in");

    // On the frame the bout began (f = 0) the weight is still 0 — a fade, not a cut.
    let mut q = ArtPresenter::new(pack());
    run(&mut q, id, Mode::Feeding, t..=onset_tick, fed);
    let m0 = q.meal_of(id).unwrap();
    assert_eq!(m0.weight_prev, 0.0);
    assert!(
        m0.weight > 0.0 && m0.weight < 1.0,
        "one tick into the fade: {}",
        m0.weight
    );
    let v0 = view_of(onset_tick, organism(id, Mode::Feeding, true));
    let bg = background(onset_tick);
    let body = &v0.organisms[0];
    assert_same_canvas(
        &draw(&mut q, &v0, 0.0),
        &expected(
            &art,
            &bg,
            body,
            &[(
                FEED_STATE,
                shared(&art, body, FEED_STATE, onset_tick, 0.0),
                1.0,
            )],
            0.0,
        ),
        "the frame the bout begins on is still the shared phase",
    );
    // Half a tick in, the two readings are mixed by the interpolated weight.
    let f = 0.5;
    let w = m0.weight_at(f);
    let seconds = present_seconds(onset_tick, f);
    let bout = seconds - present_seconds(onset_tick, 0.0);
    assert_same_canvas(
        &draw(&mut q, &v0, f),
        &expected(
            &art,
            &bg,
            body,
            &[
                (
                    FEED_STATE,
                    shared(&art, body, FEED_STATE, onset_tick, f),
                    1.0 - w,
                ),
                (FEED_STATE, bout, w),
            ],
            f,
        ),
        "mid-tick the feed layer is the weighted mix of both readings",
    );

    // Two seconds in: the bout-time reading alone, at exactly `seconds − onset`.
    let bg = background(onset_tick + 40);
    let body = &last.organisms[0];
    for f in [0.0, 1.0 / 3.0, 0.5, 2.0 / 3.0] {
        let bout = present_seconds(onset_tick + 40, f) - present_seconds(onset_tick, 0.0);
        assert_same_canvas(
            &draw(&mut p, &last, f),
            &expected(&art, &bg, body, &[(FEED_STATE, bout, 1.0)], f),
            &format!("f {f}: a sustained bout is the feed clip read at bout time"),
        );
    }
    // And it is not the old image: the onset is visible, unless the shared phase happened
    // to coincide (it does not for this id).
    let rig = rig_of(body.form, body.hue, art.creature_count());
    let clip = &art.clips[rig * 4 + FEED_STATE];
    let shared_at = shared(&art, body, FEED_STATE, onset_tick + 40, 0.0).rem_euclid(clip.seconds);
    let bout_at = (present_seconds(onset_tick + 40, 0.0) - present_seconds(onset_tick, 0.0))
        .rem_euclid(clip.seconds);
    assert!(
        (shared_at - bout_at).abs() > 0.05,
        "the fixture's shared phase coincides with the bout"
    );
    assert!(
        !differing(&draw(&mut p, &last, 0.0), &draw(&mut old, &last, 0.0)).is_empty(),
        "the onset changed nothing"
    );
}

#[test]
fn continued_intake_continues_the_loop_and_a_short_gap_neither_ends_nor_restarts_it() {
    let id = OrganismId {
        slot: 5,
        generation: 1,
    };
    let mut p = ArtPresenter::new(pack());
    let t = 300u64;
    let onset_tick = t + 5;
    let gap = settle_ticks(); // the longest gap that is still one bout
    let gap_from = onset_tick + 20;
    let fed = |tick: u64| tick >= onset_tick && !(gap_from..gap_from + gap).contains(&tick);
    run(&mut p, id, Mode::Feeding, t..=gap_from + gap + 20, fed);
    let m = p.meal_of(id).unwrap();
    assert_eq!(
        m.onset,
        Some(present_seconds(onset_tick, 0.0)),
        "the gap must not restart the bout"
    );
    assert_eq!(m.weight, 1.0);
    // Through the gap the weight never left 1: the loop simply went on.
    let mut q = ArtPresenter::new(pack());
    for tick in t..=gap_from + gap + 1 {
        q.observe(&view_of(tick, organism(id, Mode::Feeding, fed(tick))));
        if tick >= onset_tick + fade_ticks() {
            assert_eq!(
                q.meal_of(id).unwrap().weight,
                1.0,
                "tick {tick}: the weight dipped inside the settle window"
            );
        }
    }
}

#[test]
fn a_gap_past_the_settle_window_ends_the_bout_and_the_next_intake_is_a_new_onset() {
    let id = OrganismId {
        slot: 6,
        generation: 1,
    };
    let mut p = ArtPresenter::new(pack());
    let t = 400u64;
    let first = t + 5;
    let stop = first + 20;
    let again = stop + settle_ticks() + fade_ticks() + 8;
    let fed = |tick: u64| (first..stop).contains(&tick) || tick >= again;
    // Up to just before the second bout: the weight has fallen all the way.
    run(&mut p, id, Mode::Feeding, t..=again - 1, fed);
    let m = p.meal_of(id).unwrap();
    assert_eq!(m.onset, Some(present_seconds(first, 0.0)));
    assert_eq!(
        m.weight, 0.0,
        "past the settle window the bout fades out completely"
    );
    assert!(!m.active_at(again - 1));
    // The next intake is a new bout with its own onset.
    run(&mut p, id, Mode::Feeding, again..=again + 10, fed);
    let m = p.meal_of(id).unwrap();
    assert_eq!(
        m.onset,
        Some(present_seconds(again, 0.0)),
        "a new meal, a new onset"
    );
    assert!(m.weight > 0.0);
}

#[test]
fn an_intake_returning_while_the_weight_still_falls_resumes_the_same_origin() {
    let id = OrganismId {
        slot: 7,
        generation: 1,
    };
    let mut p = ArtPresenter::new(pack());
    let t = 500u64;
    let first = t + 5;
    let stop = first + 20;
    // Just past the settle window: the weight starts falling; two ticks later intake returns.
    let back = stop + settle_ticks() + 3;
    let fed = |tick: u64| (first..stop).contains(&tick) || tick >= back;
    run(&mut p, id, Mode::Feeding, t..=back - 1, fed);
    let falling = p.meal_of(id).unwrap();
    assert!(
        falling.weight > 0.0 && falling.weight < 1.0,
        "the fixture must catch the weight falling: {}",
        falling.weight
    );
    run(&mut p, id, Mode::Feeding, back..=back + 20, fed);
    let m = p.meal_of(id).unwrap();
    assert_eq!(
        m.onset,
        Some(present_seconds(first, 0.0)),
        "the origin must not jump"
    );
    assert_eq!(m.weight, 1.0);
}

#[test]
fn gestation_beats_the_meal_and_the_hand_back_is_the_ordinary_cross_fade() {
    let id = OrganismId {
        slot: 8,
        generation: 1,
    };
    let mut p = ArtPresenter::new(pack());
    let mut old = ArtPresenter::new(pack()).without_meal_onset();
    let t = 600u64;
    // A fed body that holds an escrow is budding: the bud clip, untouched by the meal.
    for tick in t..=t + 30 {
        let mut o = organism(id, Mode::Feeding, tick >= t + 2);
        o.gestation = Some(((tick - t) as f32 / 60.0).min(1.0));
        let v = view_of(tick, o);
        p.observe(&v);
        old.observe(&v);
        for f in [0.0, 0.5] {
            assert_same_canvas(
                &draw(&mut p, &v, f),
                &draw(&mut old, &v, f),
                &format!("tick {tick}: the bud clip must not read the meal"),
            );
        }
    }
    // A bout that ends with the mode changing to Seeking: through the settle window the
    // body is still the bout-time feed loop alone (the mode's own cross-fade runs underneath
    // at weight 0), then the bout fades out onto the mode's clip — by then the move clip
    // alone — and after that the body is the old image.
    let art = pack();
    let id = OrganismId {
        slot: 9,
        generation: 1,
    };
    let mut p = ArtPresenter::new(pack());
    let mut old = ArtPresenter::new(pack()).without_meal_onset();
    let t = 700u64;
    let onset_tick = t + 3;
    let leave = onset_tick + 30;
    let done = leave + settle_ticks() + fade_ticks() + 2;
    for tick in t..=done + 10 {
        let (mode, fed) = if tick < leave {
            (Mode::Feeding, tick >= onset_tick)
        } else {
            (Mode::Seeking, false)
        };
        let v = view_of(tick, organism(id, mode, fed));
        p.observe(&v);
        old.observe(&v);
        let body = &v.organisms[0];
        let bg = background(tick);
        if tick == leave + 1 {
            // Inside the settle window: the bout is whole, whatever the mode cross-fade does.
            let f = 0.5;
            let bout = present_seconds(tick, f) - present_seconds(onset_tick, 0.0);
            assert_eq!(p.meal_of(id).unwrap().weight, 1.0);
            assert_same_canvas(
                &draw(&mut p, &v, f),
                &expected(&art, &bg, body, &[(FEED_STATE, bout, 1.0)], f),
                "inside the settle window the bout holds the feed loop",
            );
        }
        if tick == leave + settle_ticks() + 3 {
            // Mid hand-back: the mode's move clip (its own fade long over) under the bout.
            let f = 0.5;
            let m = p.meal_of(id).unwrap();
            let w = m.weight_at(f);
            assert!(
                w > 0.05 && w < 0.95,
                "the fixture must sit inside the hand-back: {w}"
            );
            assert_eq!(
                p.body_of(id).unwrap().layers_at(present_seconds(tick, f)),
                vec![(1usize, 1.0f32)]
            );
            let bout = present_seconds(tick, f) - present_seconds(onset_tick, 0.0);
            assert_same_canvas(
                &draw(&mut p, &v, f),
                &expected(
                    &art,
                    &bg,
                    body,
                    &[
                        (1, shared(&art, body, 1, tick, f), 1.0 - w),
                        (FEED_STATE, bout, w),
                    ],
                    f,
                ),
                "the hand-back mixes the mode's clip in as the bout fades",
            );
        }
        if tick >= done {
            assert_same_canvas(
                &draw(&mut p, &v, 0.5),
                &draw(&mut old, &v, 0.5),
                &format!("tick {tick}: after the hand-back the body is the old image"),
            );
        }
    }
}

/// The case the recorded worlds are made of: `Feeding` with intake for a tick, `Seeking`
/// and a one-pixel shuffle for a tick or two, `Feeding` again. Without the meal the body
/// cross-fades between the move and feed clips every tick or two; with it, once the bout is
/// in, every frame is the feed loop read at bout time alone — on the body's real path, at
/// its real heading — and the loop's origin never moves through the whole run.
#[test]
fn a_nibbling_bout_holds_one_feed_loop_through_the_one_tick_seeking_gaps_on_the_real_path() {
    let art = pack();
    let id = OrganismId {
        slot: 14,
        generation: 1,
    };
    let mut p = ArtPresenter::new(pack());
    let mut old = ArtPresenter::new(pack()).without_meal_onset();
    let t = 1200u64;
    let onset_tick = t + 4;
    let mut pos = SurfacePoint::new(Face::Front, 20.0, 30.0);
    let mut last = None;
    let mut blended_old = 0;
    for tick in t..=onset_tick + 60 {
        // Nibble, step, step, nibble, ...: one tick fed and still, two ticks moving a pixel.
        let phase = (tick.wrapping_sub(onset_tick)) % 3;
        let nibbling = tick >= onset_tick && phase == 0;
        let mut o = organism(
            id,
            if nibbling {
                Mode::Feeding
            } else {
                Mode::Seeking
            },
            nibbling,
        );
        if !nibbling && tick >= onset_tick {
            let travelled = travel(pos, Vec2::new(0.5, 0.0));
            o.moved = travelled.segments.clone();
            pos = travelled.end;
        }
        o.pos = pos;
        let v = view_of(tick, o);
        p.observe(&v);
        old.observe(&v);
        if tick >= onset_tick + fade_ticks() + 1 {
            let m = p.meal_of(id).unwrap();
            assert_eq!(
                m.onset,
                Some(present_seconds(onset_tick, 0.0)),
                "tick {tick}: the origin moved"
            );
            assert_eq!(
                m.weight, 1.0,
                "tick {tick}: the bout dipped inside a nibble gap"
            );
            let body = &v.organisms[0];
            let bg = background(tick);
            for f in [0.0, 0.5] {
                let bout = present_seconds(tick, f) - present_seconds(onset_tick, 0.0);
                assert_same_canvas(
                    &draw(&mut p, &v, f),
                    &expected(&art, &bg, body, &[(FEED_STATE, bout, 1.0)], f),
                    &format!("tick {tick} f {f}: one feed loop on the real path"),
                );
                // The old presentation is a move/feed blend on most of these frames.
                if old
                    .body_of(id)
                    .unwrap()
                    .layers_at(present_seconds(tick, f))
                    .len()
                    > 1
                {
                    blended_old += 1;
                }
            }
        }
        last = Some(v);
    }
    assert!(
        blended_old > 40,
        "the old presentation blended clips on only {blended_old} frames"
    );
    let v = last.unwrap();
    assert!(!v.organisms[0].moved.is_empty() || v.organisms[0].mode == Mode::Feeding);
    assert_ne!(
        v.organisms[0].pos,
        SurfacePoint::new(Face::Front, 20.0, 30.0),
        "the fixture must have walked"
    );
}

#[test]
fn a_body_first_seen_fed_has_no_known_onset_and_a_recycled_id_starts_fresh() {
    let art = pack();
    let id = OrganismId {
        slot: 10,
        generation: 2,
    };
    let mut p = ArtPresenter::new(pack());
    let mut old = ArtPresenter::new(pack()).without_meal_onset();
    let t = 800u64;
    // Already eating when first seen: no invented history, the old image, for the whole bout.
    let last = run(&mut p, id, Mode::Feeding, t..=t + 30, |_| true);
    run(&mut old, id, Mode::Feeding, t..=t + 30, |_| true);
    let m = p.meal_of(id).unwrap();
    assert_eq!(
        m.onset, None,
        "an onset cannot be known for a body first seen mid-meal"
    );
    assert_eq!(m.last_fed, Some(t + 30));
    assert_eq!(m.weight, 0.0);
    assert_same_canvas(
        &draw(&mut p, &last, 0.5),
        &draw(&mut old, &last, 0.5),
        "a bout of unknown onset is drawn as before",
    );
    // Its next real meal has an onset.
    let pause = t + 31 + settle_ticks() + 2;
    run(&mut p, id, Mode::Feeding, t + 31..=pause, |_| false);
    run(&mut p, id, Mode::Feeding, pause + 1..=pause + 10, |_| true);
    assert_eq!(
        p.meal_of(id).unwrap().onset,
        Some(present_seconds(pause + 1, 0.0))
    );
    // The view drops the id; the slot comes back with a new generation, fed at first sight:
    // fresh memory, no onset, the old generation forgotten.
    let reborn = OrganismId {
        slot: 10,
        generation: 3,
    };
    p.observe(&view_of(pause + 11, organism(reborn, Mode::Feeding, true)));
    assert_eq!(p.meal_of(id), None, "a dead id must be forgotten");
    let m = p.meal_of(reborn).expect("the new generation is tracked");
    assert_eq!(
        m,
        MealMemory {
            onset: None,
            last_fed: Some(pause + 11),
            weight_prev: 0.0,
            weight: 0.0
        }
    );
    assert_eq!(p.meals_tracked(), 1);
    let _ = art;
}

#[test]
fn a_repeated_observation_changes_nothing_and_a_rewind_forgets_every_meal() {
    let id = OrganismId {
        slot: 11,
        generation: 1,
    };
    let mut p = ArtPresenter::new(pack());
    let t = 900u64;
    let last = run(&mut p, id, Mode::Feeding, t..=t + 3, |tick| tick >= t + 2);
    let before = p.meal_of(id).unwrap();
    assert!(before.onset.is_some() && before.weight > 0.0 && before.weight_prev < before.weight);
    let image = draw(&mut p, &last, 0.5);
    p.observe(&last);
    p.observe(&last);
    assert_eq!(
        p.meal_of(id),
        Some(before),
        "the same tick observed again must not advance the fade"
    );
    assert_same_canvas(
        &draw(&mut p, &last, 0.5),
        &image,
        "a repeated draw is the same frame",
    );
    // A rewind: everything starts over, and the same ticks give the same memory as a fresh one.
    let earlier = view_of(t + 1, organism(id, Mode::Feeding, true));
    p.observe(&earlier);
    let mut fresh = ArtPresenter::new(pack());
    fresh.observe(&earlier);
    assert_eq!(p.meal_of(id), fresh.meal_of(id));
    assert_eq!(
        p.meal_of(id).unwrap().onset,
        None,
        "a rewind carries no meal history"
    );
    assert_same_canvas(
        &draw(&mut p, &earlier, 0.5),
        &draw(&mut fresh, &earlier, 0.5),
        "after a rewind the presenter is a fresh one",
    );
}

#[test]
fn thirty_sixty_and_a_hundred_and_twenty_fps_draw_the_same_instant_alike() {
    let id = OrganismId {
        slot: 12,
        generation: 1,
    };
    let t = 1000u64;
    let onset_tick = t + 4;
    let fed = |tick: u64| tick >= onset_tick;
    // Every presenter observes every tick (the runner does); only the frame fractions differ.
    let mut a = ArtPresenter::new(pack());
    let mut b = ArtPresenter::new(pack());
    let mut c = ArtPresenter::new(pack());
    for tick in t..=onset_tick + 3 {
        let v = view_of(tick, organism(id, Mode::Feeding, fed(tick)));
        for p in [&mut a, &mut b, &mut c] {
            p.observe(&v);
        }
        // Instants the three cadences share: the tick boundary and its midpoint.
        for f in [0.0, 0.5] {
            let x = draw(&mut a, &v, f);
            let y = draw(&mut b, &v, f);
            let z = draw(&mut c, &v, f);
            assert_same_canvas(&x, &y, &format!("tick {tick} f {f}: 30 vs 60 fps"));
            assert_same_canvas(&y, &z, &format!("tick {tick} f {f}: 60 vs 120 fps"));
        }
        // The frames between are the linear weight: the 120 fps frame at 1/6 lies between
        // the boundary and the 60 fps frame at 1/3 in weight, never outside.
        let m = a.meal_of(id).unwrap();
        let (w0, w1, w2) = (
            m.weight_at(0.0),
            m.weight_at(1.0 / 6.0),
            m.weight_at(1.0 / 3.0),
        );
        assert!(
            (w0 <= w1 && w1 <= w2) || (w0 >= w1 && w1 >= w2),
            "tick {tick}: weights {w0} {w1} {w2}"
        );
        // And drawing frames in another order or twice changes nothing.
        let again = draw(&mut a, &v, 0.5);
        assert_same_canvas(&again, &draw(&mut b, &v, 0.5), "a redraw");
    }
}

#[test]
fn a_bout_carried_across_a_seam_is_read_at_bout_time_on_both_faces() {
    let art = pack();
    let id = OrganismId {
        slot: 13,
        generation: 1,
    };
    let mut p = ArtPresenter::new(pack());
    let t = 1100u64;
    // Eating since `t + 2`, walking east across the Front/Right seam during tick `t + 12`.
    let start = SurfacePoint::new(Face::Front, 62.0, 30.0);
    let step = |tick: u64| -> OrganismView {
        let mut o = organism(id, Mode::Feeding, tick >= t + 2);
        if tick >= t + 12 {
            let travelled = travel(start, Vec2::new(3.0, 0.0));
            let segments: Vec<PathSegment> = travelled.segments.clone();
            o.pos = travelled.end;
            o.moved = if tick == t + 12 { segments } else { Vec::new() };
            o.heading = Vec2::new(1.0, 0.0);
        } else {
            o.pos = start;
        }
        o
    };
    let mut last = None;
    for tick in t..=t + 12 {
        let v = view_of(tick, step(tick));
        p.observe(&v);
        last = Some(v);
    }
    let v = last.unwrap();
    let body = &v.organisms[0];
    assert_eq!(
        body.pos.face,
        Face::Right,
        "the fixture must end on the next face"
    );
    assert!(!body.moved.is_empty(), "the fixture must move this tick");
    let bg = background(t + 12);
    let mut lit_faces = std::collections::BTreeSet::new();
    for f in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let bout = present_seconds(t + 12, f) - present_seconds(t + 2, 0.0);
        let image = draw(&mut p, &v, f);
        assert_same_canvas(
            &image,
            &expected(&art, &bg, body, &[(FEED_STATE, bout, 1.0)], f),
            &format!("f {f}: across the seam"),
        );
        for (face, _, _) in differing(&image, &bg) {
            lit_faces.insert(face);
        }
        for (face, x, y) in every_pixel() {
            assert!(
                image
                    .get(face, x, y)
                    .iter()
                    .all(|&c| (0.0..=1.0 + 1e-6).contains(&c)),
                "{face:?} ({x}, {y})"
            );
        }
    }
    assert!(
        lit_faces.contains(&Face::Front) && lit_faces.contains(&Face::Right),
        "the body was drawn on {lit_faces:?}, not on both faces"
    );
}

#[test]
fn skipped_ids_are_never_tracked_and_the_memory_is_bounded_by_the_living() {
    let mut meals = Meals::new();
    let hunter = OrganismId {
        slot: 1,
        generation: 1,
    };
    let a = OrganismId {
        slot: 2,
        generation: 1,
    };
    let b = OrganismId {
        slot: 3,
        generation: 1,
    };
    let mut v = bare_view(10);
    v.organisms = vec![
        organism(hunter, Mode::Feeding, true),
        organism(a, Mode::Feeding, true),
        organism(b, Mode::Seeking, false),
    ];
    meals.observe(&v, true, &|id| id == hunter);
    assert_eq!(
        meals.memory_of(hunter),
        None,
        "an id another presentation owns is never tracked"
    );
    assert_eq!(meals.len(), 2);
    // `b` leaves; only `a` remains.
    let mut v = bare_view(11);
    v.organisms = vec![
        organism(hunter, Mode::Feeding, true),
        organism(a, Mode::Feeding, true),
    ];
    meals.observe(&v, false, &|id| id == hunter);
    assert_eq!(meals.len(), 1);
    assert!(meals.memory_of(a).is_some() && meals.memory_of(b).is_none());
    // A bout the world does not report intake for is drawn as before: `bout` is None.
    assert_eq!(
        meals.bout(a, present_seconds(11, 0.5), 0.5),
        None,
        "unknown onset, no bout reading"
    );
    assert_eq!(meals.bout(hunter, present_seconds(11, 0.5), 0.5), None);
    // Ownership can change without the organism dying or changing its ID.
    v.tick += 1;
    meals.observe(&v, false, &|id| id == hunter || id == a);
    assert_eq!(meals.memory_of(a), None);
    assert!(meals.is_empty());
    let _ = BODY_FADE_SECONDS;
}

#[test]
fn an_established_meal_fades_into_gestation_without_a_phase_cut() {
    let id = OrganismId {
        slot: 8,
        generation: 3,
    };
    let mut p = ArtPresenter::new(pack());
    run(&mut p, id, Mode::Feeding, 600..=630, |tick| tick > 600);
    let before = view_of(630, organism(id, Mode::Feeding, true));
    let end = draw(&mut p, &before, 1.0);
    let mut v = view_of(631, organism(id, Mode::Feeding, true));
    v.organisms[0].gestation = Some(0.0);
    p.observe(&v);
    let m = p.meal_of(id).unwrap();
    assert_eq!(m.last_fed, None);
    assert_eq!(m.weight_prev, 1.0);
    assert!(m.weight < 1.0, "gestation ends the settle hold immediately");
    assert_same_canvas(
        &end,
        &draw(&mut p, &v, 0.0),
        "birth funding cannot cut the outgoing meal phase",
    );
    let mut old = ArtPresenter::new(pack()).without_meal_onset();
    for tick in 632..=645 {
        v.tick = tick;
        v.organisms[0].gestation = Some(0.1);
        p.observe(&v);
        old.observe(&v);
    }
    assert_eq!(p.meal_of(id).unwrap().weight, 0.0);
    assert_same_canvas(
        &draw(&mut p, &v, 0.5),
        &draw(&mut old, &v, 0.5),
        "the settled bud retains its exact authored priority",
    );
}
