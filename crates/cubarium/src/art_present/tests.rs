use super::*;
use crate::present::Presenter;
use cube_proto::Face;
use std::path::Path;

fn pack() -> ArtPack {
    ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"))
        .expect("the baked art pack")
}

fn organism(slot: u32, hue: f32, mode: Mode) -> OrganismView {
    OrganismView {
        id: OrganismId {
            slot,
            generation: 1,
        },
        pos: SurfacePoint::new(Face::Front, 32.0, 32.0),
        heading: Vec2::new(1.0, 0.0),
        lobes: vec![(0.0, 0.0, 1.4), (2.0, 0.0, 0.9)],
        hue,
        mode,
        fed: false,
        juvenile: false,
        gestation: None,
        // No `form`: the hue tercile decides, as these fixtures were written to.
        form: u8::MAX,
        moved: Vec::new(),
    }
}

fn empty_view() -> RenderView {
    RenderView {
        tick: 0,
        producer: vec![0.0; CELL_COUNT],
        detritus: vec![0.0; CELL_COUNT],
        fruit: vec![0.0; CELL_COUNT],
        water: vec![0.0; CELL_COUNT],
        rain: vec![0.0; CELL_COUNT],
        producer_max: 2.0,
        organisms: Vec::new(),
    }
}

/// Every pixel with no soil in it, compared exactly. The M2 image says nothing about
/// the soil band, so comparisons against it are only meaningful above the horizon.
fn identical_above_horizon(a: &Canvas, b: &Canvas) -> bool {
    Face::ALL.into_iter().all(|face| {
        (0..64u8).all(|y| {
            (0..64u8)
                .all(|x| soil_weight(face, x, y) > 0.0 || a.get(face, x, y) == b.get(face, x, y))
        })
    })
}

/// The art image of a view with no organisms: floor, both grounds, and whatever
/// plants its fields grow.
fn art_ground(view: &RenderView) -> Canvas {
    let mut canvas = Canvas::new();
    ArtPresenter::new(pack()).draw(view, 0.0, &mut canvas);
    canvas
}

/// Pixels of `after` that differ from `before`, as (face, x, y).
fn added(before: &Canvas, after: &Canvas) -> Vec<(Face, u8, u8)> {
    let mut out = Vec::new();
    for face in Face::ALL {
        for y in 0..64u8 {
            for x in 0..64u8 {
                if before.get(face, x, y) != after.get(face, x, y) {
                    out.push((face, x, y));
                }
            }
        }
    }
    out
}

#[test]
fn the_form_is_the_hue_tercile_and_never_leaves_the_three_rigs() {
    assert_eq!(form_of(0.0), 0);
    // The 0/1 boundary is exactly 1/3: just below is rig 0, at and above is rig 1.
    assert_eq!(form_of(0.333), 0);
    assert_eq!(form_of(0.34), 1);
    assert_eq!(form_of(0.99), 2);
    assert_eq!(
        form_of(1.0),
        2,
        "min(2, floor(3)) is 2, not an out-of-range rig"
    );
    assert_eq!(form_of(f32::NAN), 0);
    // Values the genome does not produce still land on a real rig.
    assert_eq!(form_of(-0.5), 0);
    assert_eq!(form_of(5.0), 2);
    assert_eq!(form_of(f32::INFINITY), 2);
    assert_eq!(form_of(f32::NEG_INFINITY), 0);
}

#[test]
fn the_form_gene_names_the_rig_and_the_hue_is_only_the_fallback() {
    // In range: the gene wins whatever the hue says.
    assert_eq!(rig_of(3, 0.0, 4), 3);
    assert_eq!(rig_of(0, 0.99, 4), 0);
    // Out of range, including the v1 marker: the hue tercile, clamped into the pack.
    assert_eq!(rig_of(u8::MAX, 0.5, 4), 1);
    assert_eq!(rig_of(7, 0.99, 4), 2);
    assert_eq!(
        rig_of(4, 0.99, 2),
        1,
        "a two-rig pack clamps tercile 2 to rig 1"
    );
    assert_eq!(
        rig_of(0, 0.0, 0),
        0,
        "an empty count never divides or indexes by zero"
    );
}

#[test]
fn a_body_is_drawn_with_the_rig_its_form_names() {
    let art = pack();
    let count = art.creature_count();
    assert!(count >= 4, "the pack has four rigs");
    let mut view = empty_view();
    let mut o = organism(0, 0.0, Mode::Resting);
    o.form = 3;
    view.organisms = vec![o.clone()];
    let mut presenter = ArtPresenter::new(pack());
    let mut drawn = Canvas::new();
    presenter.observe(&view);
    presenter.draw(&view, 0.0, &mut drawn);

    // The same frame stamped by hand with rig 3's rest clip on the same ground.
    let mut expected = Canvas::new();
    let mut bare = view.clone();
    bare.organisms.clear();
    presenter.observe(&bare);
    presenter.draw(&bare, 0.0, &mut expected);
    let clip = &art.clips[3 * 4];
    let seconds = present_seconds(view.tick, 0.0);
    let pose = clip.sample(clip_time(clip, seconds, phase_of(o.id, clip.seconds), None));
    stamp_pose(
        &mut expected,
        o.pos,
        o.heading,
        pose,
        1.0,
        1.0,
        Mask::None,
        &mut vec![],
    );
    let same = |a: &Canvas, b: &Canvas| {
        Face::ALL.into_iter().all(|face| {
            (0..64u8).all(|y| (0..64u8).all(|x| a.get(face, x, y) == b.get(face, x, y)))
        })
    };
    assert!(
        same(&drawn, &expected),
        "form 3 must draw rig 3's rest frame"
    );

    // With the hue-tercile fallback the body is rig 0 instead, so the images differ.
    let mut fallback = Canvas::new();
    view.organisms[0].form = u8::MAX;
    presenter.observe(&view);
    presenter.draw(&view, 0.0, &mut fallback);
    assert!(
        !same(&drawn, &fallback),
        "the fallback rig differs from rig 3"
    );
}

#[test]
fn gestation_beats_mode_and_every_mode_maps_to_its_clip() {
    for (mode, want) in [(Mode::Resting, 0), (Mode::Seeking, 1), (Mode::Feeding, 2)] {
        let mut o = organism(0, 0.0, mode);
        assert_eq!(state_of(&o), want, "{mode:?}");
        o.gestation = Some(0.0);
        assert_eq!(
            state_of(&o),
            3,
            "a budding {mode:?} organism draws the bud clip"
        );
    }
}

#[test]
fn the_clip_phase_is_stable_per_id_bounded_and_not_shared() {
    let a = OrganismId {
        slot: 3,
        generation: 1,
    };
    let b = OrganismId {
        slot: 4,
        generation: 1,
    };
    let recycled = OrganismId {
        slot: 3,
        generation: 2,
    };
    assert_eq!(
        phase_of(a, 2.0),
        phase_of(a, 2.0),
        "the phase is a pure hash"
    );
    assert_ne!(
        phase_of(a, 2.0),
        phase_of(b, 2.0),
        "two slots must not animate together"
    );
    assert_ne!(
        phase_of(a, 2.0),
        phase_of(recycled, 2.0),
        "a reused slot is a new body"
    );
    for slot in 0..256u32 {
        let p = phase_of(
            OrganismId {
                slot,
                generation: 7,
            },
            2.5,
        );
        assert!((0.0..2.5).contains(&p), "slot {slot}: {p}");
    }
    assert_eq!(phase_of(a, 0.0), 0.0);
    assert_eq!(phase_of(a, f64::NAN), 0.0);
}

#[test]
fn the_bud_clip_follows_gestation_and_looping_clips_follow_simulated_time() {
    let art = pack();
    let bud = &art.clips[3];
    assert!(!bud.looping);
    assert_eq!(clip_time(bud, 0.0, 0.0, Some(0.0)), 0.0);
    assert_eq!(clip_time(bud, 617.25, 1.7, Some(0.5)), 0.5 * bud.seconds);
    assert_eq!(clip_time(bud, 617.25, 1.7, Some(1.0)), bud.seconds);
    // Wall time is nowhere in it: the presentation seconds and the phase are the only
    // inputs, and the presentation seconds are the tick's own.
    let walk = &art.clips[1];
    assert!(walk.looping);
    assert_eq!(clip_time(walk, 0.0, 0.25, None), 0.25);
    assert_eq!(present_seconds(21, 0.0), 20.0 * DT);
    assert_eq!(
        clip_time(walk, present_seconds(21, 0.0), 0.25, None),
        20.0 * DT + 0.25
    );
    // Gestation is ignored by a looping clip, and a doubled tick rate doubles the
    // clip's progress, which is what makes `--speed` honest.
    assert_eq!(clip_time(walk, 40.0 * DT, 0.0, Some(0.5)), 40.0 * DT);
    // A frame between ticks lands between the two tick instants, continuously.
    assert_eq!(present_seconds(21, 0.5), 20.5 * DT);
    assert_eq!(present_seconds(21, 1.0), present_seconds(22, 0.0));
    assert_eq!(present_seconds(0, 0.0), 0.0, "tick 0 cannot go below zero");
    assert_eq!(
        present_seconds(9, f64::NAN),
        8.0 * DT,
        "a nonsense fraction reads as 0"
    );
}

#[test]
fn a_quiet_world_draws_the_m2_image_above_the_horizon() {
    // No organisms, and every cell below its band's first stage threshold: above the
    // horizon the art image is then the decided M2 image with nothing added. Below
    // it the soil ground replaces the producer ramp and the flecks, which is the
    // whole point of the band and is checked in `tests/art_bands.rs`.
    let mut view = empty_view();
    let saturation = view.producer_max * PRODUCER_SATURATION;
    // Under the lowest threshold anywhere on the cube, which is the canopy's, and
    // with every soil cell under the soil's first threshold too.
    for (i, v) in view.producer.iter_mut().enumerate() {
        *v = saturation * CANOPY_STAGES[0] * (i % 7) as f64 / 7.0;
    }
    for (i, v) in view.detritus.iter_mut().enumerate() {
        *v = SOIL_SCALE * SOIL_STAGES[0] * (i % 5) as f64 / 5.0;
    }
    let mut plain = Canvas::new();
    let mut art = Canvas::new();
    Presenter::new().draw(&view, 0.0, &mut plain);
    let mut presenter = ArtPresenter::new(pack());
    presenter.observe(&view);
    presenter.draw(&view, 0.0, &mut art);
    assert!(
        identical_above_horizon(&plain, &art),
        "the art image added something below every threshold"
    );
    // Non-vacuity: the soil band really is a different image.
    assert!(
        !added(&plain, &art).is_empty(),
        "the soil band drew nothing at all, so the comparison above proves nothing"
    );

    // And a cell exactly at its band's first threshold is still bare: the rule is
    // strict in every band.
    for (cell, v) in CellId::all().zip(view.producer.iter_mut()) {
        *v = saturation * stage_thresholds(band_of(cell))[0];
    }
    Presenter::new().draw(&view, 0.0, &mut plain);
    let mut presenter = ArtPresenter::new(pack());
    presenter.observe(&view);
    presenter.draw(&view, 0.0, &mut art);
    assert!(
        identical_above_horizon(&plain, &art),
        "a cell exactly at its first threshold grew a plant"
    );
    for cell in CellId::all() {
        assert_eq!(presenter.stage_of(cell), None, "{cell:?} is not bare");
    }
}

#[test]
fn each_state_draws_a_body_near_the_organism_and_bud_ends_on_the_last_frame() {
    let art = pack();
    for (mode, gestation) in [
        (Mode::Resting, None),
        (Mode::Seeking, None),
        (Mode::Feeding, None),
        (Mode::Resting, Some(1.0f32)),
    ] {
        let mut view = empty_view();
        let mut o = organism(0, 0.5, mode);
        o.gestation = gestation;
        view.organisms = vec![o];
        let mut canvas = Canvas::new();
        // The floor, the (empty) fields and the bare soil band alone, to subtract.
        let plain = art_ground(&empty_view());
        ArtPresenter::new(pack()).draw(&view, 0.0, &mut canvas);
        let lit = added(&plain, &canvas);
        assert!(!lit.is_empty(), "{mode:?}/{gestation:?} drew no body");
        for &(f, x, y) in &lit {
            assert_eq!(f, Face::Front, "{mode:?}: a body leaked onto {f:?}");
            let d = (f64::from(x) - 32.0).hypot(f64::from(y) - 32.0);
            assert!(d < 10.0, "{mode:?}: lit {x},{y}, {d:.1} px from the body");
        }
    }

    // A gestation of 1.0 is the bud clip's last frame, exactly.
    let bud = &art.clips[form_of(0.5) * 4 + 3];
    let at_birth = clip_time(bud, 0.0, 0.0, Some(1.0));
    assert!(std::ptr::eq(bud.at(at_birth), bud.frames.last().unwrap()));
    assert!(std::ptr::eq(
        bud.at(at_birth),
        art.creature(form_of(0.5), 3, bud.seconds),
    ));
    // And a gestation of 0 is its first frame, so the clip really does run.
    assert!(std::ptr::eq(
        bud.at(clip_time(bud, 0.0, 0.0, Some(0.0))),
        &bud.frames[0]
    ));
}

#[test]
fn a_juvenile_is_the_same_clip_drawn_smaller() {
    let mut view = empty_view();
    let mut o = organism(0, 0.9, Mode::Seeking);
    o.juvenile = true;
    view.organisms = vec![o.clone()];
    let mut small = Canvas::new();
    ArtPresenter::new(pack()).draw(&view, 0.0, &mut small);

    o.juvenile = false;
    view.organisms = vec![o];
    let mut full = Canvas::new();
    ArtPresenter::new(pack()).draw(&view, 0.0, &mut full);

    let floor = art_ground(&empty_view());
    let (a, b) = (added(&floor, &small).len(), added(&floor, &full).len());
    assert!(a > 0 && a < b, "a juvenile lit {a} pixels, an adult {b}");
    assert_eq!(JUVENILE_SCALE, 0.7);
}

#[test]
fn the_stage_rule_rises_strictly_falls_with_hysteresis_and_respects_the_cap() {
    let th = FOLIAGE_STAGES;
    assert_eq!(next_stage(None, 0.0, &th, 2), None);
    assert_eq!(next_stage(None, th[0], &th, 2), None, "rising is strict");
    assert_eq!(next_stage(None, th[0] + 1e-9, &th, 2), Some(0));
    assert_eq!(
        next_stage(None, th[2] + 0.01, &th, 2),
        Some(2),
        "rises all the way at once"
    );
    assert_eq!(
        next_stage(Some(2), th[2] - STAGE_HYST / 2.0, &th, 2),
        Some(2),
        "held"
    );
    assert_eq!(
        next_stage(Some(2), th[2] - STAGE_HYST - 1e-9, &th, 2),
        Some(1),
        "falls"
    );
    assert_eq!(
        next_stage(Some(2), 0.0, &th, 2),
        None,
        "falls all the way at once"
    );
    assert_eq!(
        next_stage(None, 1.0, &th, 0),
        Some(0),
        "a cap of 0 is a sprout at most"
    );
    assert_eq!(
        next_stage(Some(2), 1.0, &th, 1),
        Some(1),
        "the cap clips a held stage"
    );
    assert_eq!(next_stage(Some(1), f64::NAN, &th, 2), None);
    // Idempotent: re-applying to the same density changes nothing.
    for t in [0.0, 0.2, 0.26, 0.44, 0.5, 0.69, 0.71, 1.0] {
        for cur in [None, Some(0), Some(1), Some(2)] {
            let once = next_stage(cur, t, &th, 2);
            assert_eq!(next_stage(once, t, &th, 2), once, "t {t} from {cur:?}");
        }
    }
}

// --- Wind ------------------------------------------------------------------------
//
// These check the *global* sampler [`wind_strength`] (a pure function of presentation
// seconds, the same everywhere on the cube), the *chart field* [`wind_chart`] (a pure
// function of position), and the *delayed per-root sampler* [`wind_at`] (the product of
// the two, at a time shifted by the part's lag and the point's own spatial phase, so its
// quiet interval sits a fraction of a second away from the global one). Each test says
// which.

#[test]
fn the_global_gust_is_bounded_rests_exactly_and_never_jerks() {
    // wind_strength, the global sampler.
    let mut peak = 0.0f64;
    let mut quiet = 0;
    let mut samples = 0;
    for i in 0..600_000 {
        let s = f64::from(i) * 0.001;
        let w = wind_strength(s);
        assert!((0.0..=1.0).contains(&w), "strength {w} at {s}");
        peak = peak.max(w);
        if w == 0.0 {
            quiet += 1;
        }
        samples += 1;
    }
    assert!(peak > 0.8, "the packet never reaches its peak: {peak}");
    // The quiet interval is exactly the period less the packet, to the sampling.
    let share = f64::from(quiet) / f64::from(samples);
    let want = WIND_QUIET_SECONDS / WIND_PERIOD;
    assert!(
        (share - want).abs() < 0.01,
        "calm for {share:.3} of the time, not {want:.3}"
    );
    // Exactly zero, not nearly: every instant of the quiet interval of ten packets.
    for packet in 0..10 {
        let start = f64::from(packet) * WIND_PERIOD + WIND_RISE + WIND_HOLD + WIND_FALL;
        for k in 0..=100 {
            let s = start + WIND_QUIET_SECONDS * f64::from(k) / 100.0;
            // The end of the quiet interval is the next packet's start, also zero.
            assert_eq!(wind_strength(s), 0.0, "not resting at {s}");
        }
    }
    assert_eq!(wind_strength(f64::NAN), 0.0);
    assert_eq!(wind_strength(f64::INFINITY), 0.0);
    // Continuous with a continuous slope at all four boundaries of the packet: the
    // one-sided differences agree, and the edges have (almost) no slope at all.
    let h = 1e-4;
    for packet in 0..3 {
        let base = f64::from(packet) * WIND_PERIOD;
        for edge in [
            0.0,
            WIND_RISE,
            WIND_RISE + WIND_HOLD,
            WIND_RISE + WIND_HOLD + WIND_FALL,
        ] {
            let t = base + edge;
            let (l, m, r) = (wind_strength(t - h), wind_strength(t), wind_strength(t + h));
            assert!(
                (m - l).abs() < 1e-3 && (r - m).abs() < 1e-3,
                "a jump at {t}"
            );
            let slope = |a: f64, b: f64| (b - a) / h;
            assert!(
                (slope(l, m) - slope(m, r)).abs() < 0.1,
                "a kink at {t}: {} vs {}",
                slope(l, m),
                slope(m, r)
            );
        }
        // The packet's own edges have zero value *and* zero slope.
        for edge in [0.0, WIND_RISE + WIND_HOLD + WIND_FALL] {
            let t = base + edge;
            assert_eq!(wind_strength(t), 0.0);
            assert!(wind_strength(t + h).abs() < 1e-6 && wind_strength(t - h).abs() < 1e-6);
        }
    }
    // A 60 fps step through a whole packet never moves the strength by much.
    let mut last = wind_strength(0.0);
    for frame in 1..=(WIND_PERIOD * 60.0) as i32 {
        let w = wind_strength(f64::from(frame) / 60.0);
        assert!((w - last).abs() < 0.02, "frame {frame}: {last} → {w}");
        last = w;
    }
}

#[test]
fn the_chart_field_joins_across_every_seam_under_the_real_tangent_transport() {
    // wind_chart, verified against `travel`'s own tangent map rather than a second seam
    // table: step across each connected seam and compare the transported vector with the
    // field on the far side.
    let eps = 1e-7;
    let mut crossings = 0;
    for face in Face::ALL {
        for edge in cubarium_surface::Edge::ALL {
            if face != Face::Top && edge == cubarium_surface::Edge::Bottom {
                continue; // the open rim reflects; it is not a seam
            }
            for k in 0..64 {
                let along = f64::from(k) + 0.5;
                let (u, v, step) = match edge {
                    cubarium_surface::Edge::Top => (along, eps, Vec2::new(0.0, -2.0 * eps)),
                    cubarium_surface::Edge::Right => (64.0 - eps, along, Vec2::new(2.0 * eps, 0.0)),
                    cubarium_surface::Edge::Bottom => {
                        (along, 64.0 - eps, Vec2::new(0.0, 2.0 * eps))
                    }
                    cubarium_surface::Edge::Left => (eps, along, Vec2::new(-2.0 * eps, 0.0)),
                };
                let here = SurfacePoint::new(face, u, v);
                let crossed = cubarium_surface::travel(here, step);
                assert_eq!(crossed.crossings, 1, "{face:?} {edge:?} is not one seam");
                let mine = wind_chart(here.face, here.u, here.v);
                let theirs = wind_chart(crossed.end.face, crossed.end.u, crossed.end.v);
                let transported = crossed.map.apply(mine);
                assert!(
                    (transported - theirs).length() < 1e-6,
                    "{face:?} {edge:?} at {along}: {transported:?} vs {theirs:?}"
                );
                crossings += 1;
            }
        }
    }
    assert_eq!(
        crossings,
        16 * 64,
        "sixteen connected half-edges, 64 positions each"
    );

    // Exactly zero where the design says so: every side/side seam (a side face's whole
    // left and right edge) and each of Top's four vertices.
    for face in [Face::Front, Face::Right, Face::Back, Face::Left] {
        for k in 0..64 {
            let v = f64::from(k) + 0.5;
            assert_eq!(wind_chart(face, 0.0, v), Vec2::ZERO, "{face:?} left seam");
            assert_eq!(wind_chart(face, 64.0, v), Vec2::ZERO, "{face:?} right seam");
        }
    }
    for u in [0.0, 64.0] {
        for v in [0.0, 64.0] {
            assert_eq!(
                wind_chart(Face::Top, u, v),
                Vec2::ZERO,
                "Top vertex ({u},{v})"
            );
        }
    }
    assert_eq!(
        wind_chart(Face::Top, 32.0, 32.0),
        Vec2::ZERO,
        "Top's centre is calm"
    );
    // And nowhere does it exceed the documented maximum, which it attains.
    let mut worst = 0.0f64;
    for face in Face::ALL {
        for i in 0..=128 {
            for j in 0..=128 {
                let (u, v) = (f64::from(i) * 0.5, f64::from(j) * 0.5);
                worst = worst.max(wind_chart(face, u, v).length());
            }
        }
    }
    assert!(worst <= WIND_CHART_MAX + 1e-12, "|W| reached {worst}");
    assert!(
        worst > WIND_CHART_MAX - 1e-9,
        "|W| never reaches its maximum: {worst}"
    );
    assert_eq!(wind_chart(Face::Front, f64::NAN, 3.0), Vec2::ZERO);
}

#[test]
fn the_delayed_sampler_rests_with_the_packet_and_stays_inside_the_chart_maximum() {
    // wind_at: the chart field times the global strength at a *shifted* time, so its own
    // quiet interval is the global one moved by the lag and the point's spatial phase —
    // at most `WIND_TRAVEL_SECONDS` of the 12-second rest, which is why the middle of a
    // quiet interval is calm everywhere at once.
    let middle = WIND_RISE + WIND_HOLD + WIND_FALL + WIND_QUIET_SECONDS / 2.0;
    for face in Face::ALL {
        for i in 0..8 {
            for j in 0..8 {
                let p = SurfacePoint::new(face, f64::from(i) * 8.0 + 0.5, f64::from(j) * 8.0 + 0.5);
                assert!(wind_phase(p).abs() <= 1.0, "phase out of range at {p:?}");
                for lag in [0.0, 0.05, 0.1, 0.15, 0.2] {
                    assert_eq!(wind_at(p, middle, lag), Vec2::ZERO, "{p:?} lag {lag}");
                    for k in 0..60 {
                        let s = f64::from(k) * 0.5;
                        let w = wind_at(p, s, lag);
                        assert!(w.length() <= WIND_CHART_MAX + 1e-12, "{p:?} at {s}");
                    }
                }
            }
        }
    }
    // The lag really delays: a point in the middle of a face, during the rise, answers
    // later with a larger lag.
    let p = SurfacePoint::new(Face::Front, 32.0, 32.0);
    let during = WIND_RISE * 0.5;
    assert!(wind_at(p, during, 0.0).length() > wind_at(p, during, 0.2).length());
    // And the spatial phase really varies: two points on opposite sides of the cube
    // answer at different times.
    let far = SurfacePoint::new(Face::Back, 32.0, 32.0);
    assert!((wind_phase(p) - wind_phase(far)).abs() > 0.5);
    assert_eq!(wind_at(p, f64::NAN, 0.0), Vec2::ZERO);
    assert_eq!(
        wind_at(p, 6.0, f64::NAN).length(),
        wind_at(p, 6.0, 0.0).length()
    );
}

/// Which clip and frame of a plant is the one holding its budget down — the art a
/// review session would have to narrow to buy more movement.
fn limiting_clip(art: &ArtPack, name: &str) -> String {
    let mut worst = (f64::INFINITY, String::from("nothing"));
    let mut consider = |label: String, clip: &Clip, base: f64, root: f64, length: f64| {
        for (i, frame) in clip.frames.iter().enumerate() {
            let room = frame.bend_headroom(root, length, base);
            if room < worst.0 {
                worst = (room, format!("{label} frame {i}"));
            }
        }
    };
    if let Some(plant) = art.plant(name) {
        for (i, clip) in plant.stages.iter().enumerate() {
            consider(
                format!("stage {i}"),
                clip,
                0.0,
                PLANT_BEND_ROOT,
                PLANT_BEND_LENGTH,
            );
        }
        if let Some(clip) = &plant.fruit {
            consider(
                "fruit".into(),
                clip,
                0.0,
                PLANT_BEND_ROOT,
                PLANT_BEND_LENGTH,
            );
        }
        for t in &plant.transitions {
            consider(
                format!("grow {}→{}", t.from, t.to),
                &t.clip,
                0.0,
                PLANT_BEND_ROOT,
                PLANT_BEND_LENGTH,
            );
        }
    }
    if let Some(plant) = art.tall_plant(name) {
        let top = f64::from(TALL_MAX_SEGMENTS);
        consider(
            "trunk".into(),
            &plant.trunk,
            tall_bend_base(top),
            TALL_BEND_ROOT,
            TALL_BEND_LENGTH,
        );
        if let Some(clip) = &plant.base {
            consider(
                "base".into(),
                clip,
                tall_bend_base(0.0),
                TALL_BEND_ROOT,
                TALL_BEND_LENGTH,
            );
        }
        if let Some(clip) = &plant.cap {
            consider(
                "cap".into(),
                clip,
                tall_bend_base(top + 1.0),
                TALL_BEND_ROOT,
                TALL_BEND_LENGTH,
            );
        }
    }
    worst.1
}

#[test]
fn the_shipped_pack_admits_a_bend_for_every_species_that_wants_one() {
    let presenter = ArtPresenter::new(pack());
    let art = pack();
    for (name, budget) in presenter.bend_budgets() {
        println!(
            "  {name:<14} budget {budget:>6.2}  held by its {}",
            limiting_clip(&art, name)
        );
    }
    println!("\n  asset          budget   desired   effective (±10 %)");
    for (name, budget) in presenter.bend_budgets() {
        let r = wind_response(name);
        let eff = effective_tip(r.tip_px, *budget);
        println!(
            "  {name:<14} {budget:>6.2}   {:>7.2}   {eff:>6.3} .. {:>6.3}{}",
            r.tip_px,
            eff * (1.0 + WIND_SLOT_VARIATION),
            if r.spin_deg > 0.0 {
                format!("   (rotates {}°)", r.spin_deg)
            } else {
                String::new()
            },
        );
        assert!(*budget >= 0.0, "{name}: a negative budget");
        // The hard bound: no slot of this family, at full wind, can exceed the budget.
        assert!(
            eff * (1.0 + WIND_SLOT_VARIATION) <= *budget + 1e-12,
            "{name}: {eff} × 1.1 escapes the budget {budget}"
        );
        assert!(
            eff <= r.tip_px + 1e-12,
            "{name}: more than the species asked for"
        );
    }
    // The clamp is real on this pack, not a theoretical guard: at least one species asks
    // for more tip travel than its own widest frame can afford, and gets less.
    let clamped: Vec<&str> = presenter
        .bend_budgets()
        .iter()
        .filter(|(name, budget)| {
            let want = wind_response(name).tip_px;
            want > 0.0 && effective_tip(want, *budget) < want - 1e-12
        })
        .map(|(name, _)| name.as_str())
        .collect();
    println!("  clamped by the pack's own art: {clamped:?}");
    assert!(
        !clamped.is_empty(),
        "no species is bounded by the pack, so the rule is untested"
    );
    // Every side-face species that wants to move is admitted *some* movement, or the
    // slice has no visible effect and the art has to change instead.
    for name in [
        "lanternstalk",
        "tendrilfan",
        "reedspire",
        "glowcap",
        "spiretree",
        "glasscane",
    ] {
        let eff = effective_tip(wind_response(name).tip_px, presenter.bend_budget(name));
        assert!(eff > 0.0, "{name} cannot move at all on this pack");
    }
    // An unmeasured name never moves.
    assert_eq!(presenter.bend_budget("nosuchplant"), 0.0);
    assert_eq!(effective_tip(0.9, 0.0), 0.0);
    // A column's budget is its own family's, tightened by the vine where it carries one.
    for column in presenter.columns() {
        let own = presenter.bend_budget(TALL_PLANTS[column.pick]);
        let want = if column.vine {
            own.min(presenter.bend_budget(VINE_PLANT))
        } else {
            own
        };
        assert_eq!(presenter.column_budget(column), want);
    }
}

#[test]
fn a_quiet_interval_draws_the_windless_image_and_a_packet_does_not() {
    // Both samplers, through the presenter: at a quiet instant every slot's bend is the
    // identity and every column's amplitude is exactly zero, so the drawn image is the
    // windless one by construction; inside a packet neither is.
    let presenter = ArtPresenter::new(pack());
    let quiet = present_seconds(WIND_QUIET_TICK, 0.0);
    let windy = present_seconds(WIND_PEAK_TICK, 0.0);
    assert_eq!(wind_strength(quiet), 0.0);
    assert!(
        wind_strength(windy) > 0.5,
        "the peak fixture tick is not windy"
    );
    let mut moved = 0;
    let mut turned = 0;
    for cell in CellId::all() {
        let band = band_of(cell);
        let Some(plant) = presenter.plant_for(band, cell) else {
            continue;
        };
        let slot = slot_of(cell);
        let budget = presenter.bend_budget(&plant.name);
        let (bend, heading) = slot_wind(&slot, &plant.name, budget, quiet);
        assert!(
            bend.is_identity(),
            "{cell:?} bends in a quiet interval: {bend:?}"
        );
        assert_eq!(heading, slot.heading, "{cell:?} turns in a quiet interval");
        let (bend, heading) = slot_wind(&slot, &plant.name, budget, windy);
        if !bend.is_identity() {
            moved += 1;
            assert!(
                bend.amplitude.abs() <= budget + 1e-12,
                "{cell:?} bends {} past its budget {budget}",
                bend.amplitude
            );
            assert_eq!(bend.root, PLANT_BEND_ROOT);
            assert_eq!(bend.length, PLANT_BEND_LENGTH);
            assert_eq!(bend.base, 0.0);
        }
        if heading != slot.heading {
            turned += 1;
            let deg = signed_turn(slot.heading, heading).to_degrees().abs();
            assert!(
                deg <= wind_response(&plant.name).spin_deg * (1.0 + WIND_SLOT_VARIATION) + 1e-9,
                "{cell:?} turned {deg}°"
            );
            assert_eq!(cell.face(), Face::Top, "only a radial plant turns");
        }
    }
    assert!(moved > 100, "only {moved} slots bend at the packet's peak");
    assert!(
        turned > 100,
        "only {turned} canopy slots turn at the packet's peak"
    );
    for column in presenter.columns() {
        let budget = presenter.column_budget(column);
        assert_eq!(
            tall_amplitude(column, budget, quiet),
            0.0,
            "a column bends when calm"
        );
        assert!(tall_amplitude(column, budget, windy).abs() <= budget + 1e-12);
    }
    assert!(
        presenter
            .columns()
            .iter()
            .any(|c| tall_amplitude(c, presenter.column_budget(c), windy) != 0.0),
        "no column bends at the packet's peak"
    );
}

#[test]
fn a_columns_parts_share_one_amplitude_on_one_continuous_curve() {
    // The bend base of every tile of a column: 4i − 8, so the tile bottoms meet and one
    // `D(H)` runs from the base's root to the cap.
    assert_eq!(tall_bend_base(0.0), -8.0);
    assert_eq!(tall_bend_base(1.0), -4.0);
    assert_eq!(tall_bend_base(f64::from(TALL_MAX_SEGMENTS) + 1.0), 32.0);
    // Tile i's top edge is tile i+1's bottom edge plus the 12-row overlap: a sample at
    // the same *height* has the same displacement whichever tile paints it, which is
    // what keeps a join from opening.
    let amplitude = 0.5;
    let bend_at = |i: f64| Bend {
        amplitude,
        base: tall_bend_base(i),
        root: TALL_BEND_ROOT,
        length: TALL_BEND_LENGTH,
    };
    for i in 1..TALL_MAX_SEGMENTS {
        let lower = bend_at(f64::from(i));
        let upper = bend_at(f64::from(i + 1));
        // Height h above the horizon is tile row `16 − (h − base)` in each tile.
        for h in [12.0, 13.5, 15.0] {
            let a = lower.displacement(TILE_ROWS, TILE_ROWS - (h - lower.base));
            let b = upper.displacement(TILE_ROWS, TILE_ROWS - (h - upper.base));
            assert!(
                (a - b).abs() < 1e-12,
                "tile {i}/{} disagree at h={h}",
                i + 1
            );
        }
    }
    // The root of the column does not move, and the top of a full column moves fully.
    assert_eq!(
        bend_at(0.0).displacement(TILE_ROWS, 15.5),
        0.0,
        "the base's root row"
    );
    // The cap's top row of a full column sits at H = 47.5 of the 48-pixel bend length,
    // so it takes all but a thousandth of the amplitude.
    let top = bend_at(f64::from(TALL_MAX_SEGMENTS) + 1.0).displacement(TILE_ROWS, 0.5);
    assert!(
        (top - amplitude).abs() < 1e-3,
        "the cap's top row moved {top}"
    );
}

// --- Authored growth clips (pack v5) ----------------------------------------------
//
// The growth pilot: where the pack carries a `grow<from><to>` clip for the step a cell is
// in, `draw` plays that clip instead of revealing the upper stage from behind a mask.
// Every test here drives the presenter through `observe` only — paced growth, never
// injected state — and rebuilds the picture the documented rule asks for by hand.

/// Producer density, as a fraction of the ramp's saturation point, that warrants a sprout
/// in the foliage band and no more.
const AT_SPROUT: f64 = 0.30;
/// Producer density that warrants stage 1 in the foliage band and no more.
const AT_MID: f64 = 0.50;

/// The one slot these tests use: a rank-2 lanternstalk in the middle of Front. The
/// lanternstalk is the pilot species and the only plant of the shipped pack that carries
/// an authored growth clip.
fn growth_cell() -> CellId {
    CellId::all()
        .find(|&c| {
            c.face() == Face::Front
                && band_of(c) == Band::Foliage
                && plant_cap(Band::Foliage, c) == Some(2)
                && species_of(Band::Foliage, c) == "lanternstalk"
                && (4..=7).contains(&c.cy())
                && (4..=11).contains(&c.cx())
        })
        .expect("a rank-2 lanternstalk slot in the middle of Front")
}

/// A view whose only rich cell is `cell`, at `density` as a fraction of the producer
/// ramp's saturation point — which is exactly what [`plant_density`] measures. One cell
/// is far too little to grow a column ([`column_density`] averages a whole face column),
/// so the only thing on the canvas above the ground is that cell's plant.
fn one_cell_view(tick: u64, cell: CellId, density: f64) -> RenderView {
    let mut view = empty_view();
    view.tick = tick;
    view.producer[cell.index()] = density * view.producer_max * PRODUCER_SATURATION;
    view
}

fn drawn_with(p: &mut ArtPresenter, view: &RenderView, f: f64) -> Canvas {
    let mut canvas = Canvas::new();
    p.draw(view, f, &mut canvas);
    canvas
}

/// The largest absolute per-channel difference between two images.
fn worst_diff(a: &Canvas, b: &Canvas) -> f32 {
    let mut worst = 0.0f32;
    for face in Face::ALL {
        for y in 0..64u8 {
            for x in 0..64u8 {
                let (p, q) = (a.get(face, x, y), b.get(face, x, y));
                for c in 0..3 {
                    worst = worst.max((p[c] - q[c]).abs());
                }
            }
        }
    }
    worst
}

/// The same image with the plants taken out of the pack: floor, both grounds, the ground
/// cover and the water — the background a plant stamp lands on. The fixtures grow nothing
/// tall (asserted), so this needs no history of its own.
fn background(view: &RenderView, f: f64) -> Canvas {
    let mut art = pack();
    art.plants.clear();
    let mut p = ArtPresenter::new(art);
    p.observe(view);
    let canvas = drawn_with(&mut p, view, f);
    assert!(
        (0..p.columns().len()).all(|i| p.segments_of(i) == 0),
        "the one-cell fixture grew a tall column, so this is not just the ground"
    );
    canvas
}

/// One plant stamp at a cell's slot with that slot's own wind at `seconds`: the single
/// [`stamp_layers_bent`] call the drawing rules make for a plant.
fn stamp_at(
    canvas: &mut Canvas,
    cell: CellId,
    plant: &Plant,
    budget: f64,
    layers: &[(Pose<'_>, f32)],
    opacity: f32,
    mask: Mask,
    seconds: f64,
) {
    let slot = slot_of(cell);
    let (bend, heading) = slot_wind(&slot, &plant.name, budget, seconds);
    stamp_layers_bent(
        canvas,
        slot.at,
        heading,
        layers,
        1.0,
        opacity,
        mask,
        bend,
        &mut Vec::new(),
    );
}

/// A presenter idle at stage 0 in `cell`, reached the way a viewer joining a living world
/// does: one snapping observe.
fn idle_sprout(art: ArtPack, cell: CellId) -> ArtPresenter {
    let th = stage_thresholds(band_of(cell));
    assert!(
        th[0] < AT_SPROUT && AT_SPROUT < th[1] && th[1] < AT_MID && AT_MID < th[2],
        "the fixture densities no longer bracket the band's thresholds"
    );
    let mut p = ArtPresenter::new(art);
    p.observe(&one_cell_view(1, cell, AT_SPROUT));
    assert_eq!(
        p.growth_of(cell),
        Growth::snapped(Some(0), false),
        "{cell:?}"
    );
    p
}

/// That presenter with the `0 → 1` step `quarters` quarters of the way along: each
/// observe is 20 ticks (one simulated second, a quarter of [`STAGE_GROW_SECONDS`]) after
/// the last, so `g = quarters / 4` exactly, and `quarters = 4` lands idle on stage 1.
fn growing_from(art: ArtPack, cell: CellId, quarters: u64) -> ArtPresenter {
    let mut p = idle_sprout(art, cell);
    for k in 1..=quarters {
        p.observe(&one_cell_view(1 + 20 * k, cell, AT_MID));
    }
    p
}

fn growing(cell: CellId, quarters: u64) -> ArtPresenter {
    growing_from(pack(), cell, quarters)
}

#[test]
fn the_growth_blend_weights_hold_the_two_idle_clips_at_the_ends() {
    assert!(
        GROW_BLEND > 0.0 && GROW_BLEND < 0.5,
        "the two edge blends would overlap"
    );
    for k in 0..=1000 {
        let t = f64::from(k) / 1000.0;
        let [from, grow, to] = growth_weights(t);
        assert!(
            from >= 0.0 && grow >= 0.0 && to >= 0.0,
            "t = {t}: {from} {grow} {to}"
        );
        assert!(
            ((from + grow + to) - 1.0).abs() < 1e-6,
            "t = {t}: not an exact lerp"
        );
        assert!(
            !(from > 0.0 && to > 0.0),
            "t = {t}: both edge blends are live at once"
        );
        assert_eq!(
            from > 0.0,
            t < GROW_BLEND,
            "t = {t}: the entry blend's reach"
        );
        assert_eq!(
            to > 0.0,
            t > 1.0 - GROW_BLEND,
            "t = {t}: the exit blend's reach"
        );
    }
    // The ends are the neighbouring idle clip alone; the middle is the growth clip alone.
    assert_eq!(growth_weights(0.0), [1.0, 0.0, 0.0]);
    assert_eq!(growth_weights(GROW_BLEND), [0.0, 1.0, 0.0]);
    assert_eq!(growth_weights(0.5), [0.0, 1.0, 0.0]);
    assert_eq!(growth_weights(1.0 - GROW_BLEND), [0.0, 1.0, 0.0]);
    let end = growth_weights(1.0);
    assert!(
        end[0] == 0.0 && end[1].abs() < 1e-6 && (end[2] - 1.0).abs() < 1e-6,
        "{end:?}"
    );
    assert_eq!(
        growth_weights(f64::NAN),
        [1.0, 0.0, 0.0],
        "nonsense holds the lower stage"
    );
    // Zero slope at both edges: the first thousandth moves far less than the middle.
    let slope = |t: f64| (growth_weights(t)[0] - growth_weights(t + 0.001)[0]).abs();
    assert!(
        slope(0.0) < slope(GROW_BLEND / 2.0) / 10.0,
        "a kink at the entry"
    );
    let rise = |t: f64| (growth_weights(t + 0.001)[2] - growth_weights(t)[2]).abs();
    assert!(
        rise(1.0 - 0.001) < rise(1.0 - GROW_BLEND / 2.0) / 10.0,
        "a kink at the exit"
    );
}

#[test]
fn an_authored_growth_clip_enters_from_the_lower_idle_stage_and_leaves_on_the_upper_one() {
    let cell = growth_cell();
    let art = pack();
    let plant = art.plant("lanternstalk").expect("the pilot species");
    assert!(
        plant.transition(0, 1).is_some(),
        "pack v5 must carry the 0 → 1 growth clip"
    );
    let budget = ArtPresenter::new(pack()).bend_budget(&plant.name);
    // The frames are *drawn* against a sparse density, where the two stages' opacities
    // differ — so the endpoint opacity is the endpoint stage's, not a blend of the two.
    let th = stage_thresholds(Band::Foliage);
    let low = stage_opacity(0, AT_SPROUT, &th, MOTIF_OPACITY);
    let high = stage_opacity(1, AT_SPROUT, &th, MOTIF_OPACITY);
    assert!(
        low > 0.0 && low < high,
        "{low} vs {high}: the endpoints must be tellable apart"
    );

    // t → 0: one tick into the step, a thousandth of the way through the frame. The
    // residual is the growth layer's own weight, `1 − smoothstep(t / GROW_BLEND) ≈ 3
    // (t / GROW_BLEND)²`, times at most one (premultiplied linear channels) times the
    // opacity — under 1e-7 here, so 1e-5 is a decade of slack over the arithmetic.
    let mut p = idle_sprout(pack(), cell);
    p.observe(&one_cell_view(2, cell, AT_MID));
    let f = 1e-3;
    let step = growth_step(growth_between(p.growth_prev_of(cell), p.growth_of(cell), f))
        .expect("the step is in flight");
    assert_eq!((step.lower, step.upper), (Some(0), 1));
    assert!(step.t > 0.0 && step.t < 1e-4, "t = {}", step.t);
    assert!(
        growth_weights(step.t)[1] < 1e-6,
        "the entry is not the idle sprout"
    );
    let v = one_cell_view(2, cell, AT_SPROUT);
    let seconds = present_seconds(v.tick, f);
    let mut want = background(&v, f);
    stamp_at(
        &mut want,
        cell,
        plant,
        budget,
        &[(stage_pose(plant, 0, cell, seconds), 1.0)],
        low,
        Mask::None,
        seconds,
    );
    let entry = worst_diff(&drawn_with(&mut p, &v, f), &want);
    assert!(entry < 1e-5, "the growth clip entered with a cut: {entry}");

    // t → 1: the step completed on the last observe, drawn a ten-thousandth before the
    // frame's end. Here the residual is the exit blend's, of the same size.
    let mut p = growing(cell, 4);
    assert_eq!(p.growth_of(cell), Growth::snapped(Some(1), false));
    assert_eq!(p.growth_prev_of(cell).g, 0.75, "the last in-flight tick");
    let (tick, f) = (81, 1.0 - 1e-4);
    let step = growth_step(growth_between(p.growth_prev_of(cell), p.growth_of(cell), f))
        .expect("the last frame of the step is still in flight");
    assert!(step.t > 1.0 - 1e-4 && step.t < 1.0, "t = {}", step.t);
    assert!(
        growth_weights(step.t)[1] < 1e-5,
        "the exit is not the idle stage 1"
    );
    let v = one_cell_view(tick, cell, AT_SPROUT);
    let seconds = present_seconds(tick, f);
    let mut want = background(&v, f);
    stamp_at(
        &mut want,
        cell,
        plant,
        budget,
        &[(stage_pose(plant, 1, cell, seconds), 1.0)],
        high,
        Mask::None,
        seconds,
    );
    let exit = worst_diff(&drawn_with(&mut p, &v, f), &want);
    assert!(exit < 1e-5, "the growth clip left with a cut: {exit}");
    // Non-vacuity: the plant is really on the canvas, and the two endpoints differ.
    assert!(
        worst_diff(&want, &background(&v, f)) > 0.01,
        "the fixture's plant is invisible"
    );
}

#[test]
fn a_reversed_step_draws_the_same_picture_at_the_same_progress() {
    // The rule first, as a pure function of the growth: a step and its reversal agree on
    // the pair *and* on the progress, so the clip simply runs backwards. (`1 − (1 − g)`
    // is `g` only up to one rounding of the complement, which is why the progress is
    // compared to the double's own precision and the pair exactly.)
    for k in 0..=20 {
        let g = f64::from(k) / 20.0;
        let up = growth_step(Growth {
            from: Some(0),
            to: Some(1),
            g,
            target: Some(1),
            fruit: 0.0,
        })
        .expect("in flight");
        let down = growth_step(Growth {
            from: Some(1),
            to: Some(0),
            g: 1.0 - g,
            target: Some(0),
            fruit: 0.0,
        })
        .expect("in flight");
        assert_eq!((up.lower, up.upper), (down.lower, down.upper), "g = {g}");
        assert_eq!(up.t, g);
        assert!(
            (up.t - down.t).abs() <= f64::EPSILON,
            "g = {g}: {} vs {}",
            up.t,
            down.t
        );
    }
    assert_eq!(
        growth_step(Growth::snapped(Some(1), false)),
        None,
        "an idle plant is no step"
    );
    assert_eq!(growth_step(Growth::snapped(None, false)), None);

    // And through the presenter: one plant growing 0 → 1 and one wilting 1 → 0, both
    // halfway through the step, drawn against the same view at the same instant.
    let cell = growth_cell();
    let mut up = growing(cell, 2);
    let mut down = ArtPresenter::new(pack());
    down.observe(&one_cell_view(1, cell, AT_MID));
    assert_eq!(
        down.growth_of(cell),
        Growth::snapped(Some(1), false),
        "snapped to stage 1"
    );
    // One second of a two-second wilt is half of it, as one second is a quarter of a grow.
    down.observe(&one_cell_view(21, cell, AT_SPROUT));
    assert_eq!(
        up.growth_of(cell),
        Growth {
            from: Some(0),
            to: Some(1),
            g: 0.5,
            target: Some(1),
            fruit: 0.0
        }
    );
    assert_eq!(
        down.growth_of(cell),
        Growth {
            from: Some(1),
            to: Some(0),
            g: 0.5,
            target: Some(0),
            fruit: 0.0
        }
    );
    let v = one_cell_view(41, cell, AT_SPROUT);
    let forward = drawn_with(&mut up, &v, 1.0);
    let backward = drawn_with(&mut down, &v, 1.0);
    assert_eq!(
        worst_diff(&forward, &backward),
        0.0,
        "a reversal drew a different picture"
    );
    assert!(
        worst_diff(&forward, &background(&v, 1.0)) > 0.01,
        "neither presenter drew a plant, so the comparison proves nothing"
    );
}

#[test]
fn a_growth_clip_is_never_advanced_or_restarted_by_a_draw() {
    let cell = growth_cell();
    let mut p = growing(cell, 2);
    let v = one_cell_view(41, cell, AT_MID);
    let before = (p.growth_of(cell), p.growth_prev_of(cell));
    let first = drawn_with(&mut p, &v, 0.37);
    for k in 0..3 {
        let again = drawn_with(&mut p, &v, 0.37);
        assert_eq!(
            worst_diff(&first, &again),
            0.0,
            "draw {k} moved the growth clip"
        );
    }
    assert_eq!(
        (p.growth_of(cell), p.growth_prev_of(cell)),
        before,
        "a draw advanced the paced growth"
    );
    // A different frame of the same tick *is* a different picture: the clip runs on the
    // frame's own instant rather than being held between ticks.
    assert!(
        worst_diff(&first, &drawn_with(&mut p, &v, 0.63)) > 0.0,
        "the clip is held"
    );
}

#[test]
fn a_pack_without_a_clip_for_the_step_keeps_the_reveal_masks() {
    let cell = growth_cell();
    // The shipped pack with every authored growth clip removed: a v4 pack, in effect.
    let mut v4 = pack();
    for plant in v4.plants.iter_mut() {
        plant.transitions.clear();
    }
    let mut p = growing_from(v4, cell, 2);
    assert_eq!(
        p.bend_budget("lanternstalk"),
        plant_bend_budget(pack().plant("lanternstalk").expect("the pilot species"),),
        "clearing the transitions must not change the measured budget, or the bends differ"
    );
    let art = pack();
    let plant = art.plant("lanternstalk").unwrap();
    let budget = p.bend_budget(&plant.name);
    let (v, f, t) = (one_cell_view(41, cell, AT_MID), 1.0, 0.5);
    let seconds = present_seconds(v.tick, f);
    let th = stage_thresholds(Band::Foliage);
    let mut want = background(&v, f);
    // The lower stage fades out whole...
    stamp_at(
        &mut want,
        cell,
        plant,
        budget,
        &[(stage_pose(plant, 0, cell, seconds), 1.0)],
        stage_opacity(0, AT_MID, &th, MOTIF_OPACITY) * (1.0 - t) as f32,
        Mask::None,
        seconds,
    );
    // ...under the upper stage, revealed up the stalk.
    stamp_at(
        &mut want,
        cell,
        plant,
        budget,
        &[(stage_pose(plant, 1, cell, seconds), 1.0)],
        stage_opacity(1, AT_MID, &th, MOTIF_OPACITY),
        Mask::Axial {
            reveal: t * PLANT_REVEAL_PX,
        },
        seconds,
    );
    let fallback = drawn_with(&mut p, &v, f);
    assert_eq!(
        worst_diff(&fallback, &want),
        0.0,
        "the fallback is not the reveal mask"
    );
    // And the v5 pack plays the clip instead, which is a different picture.
    let with_clip = drawn_with(&mut growing(cell, 2), &v, f);
    assert!(
        worst_diff(&fallback, &with_clip) > 0.01,
        "the authored clip changed nothing"
    );
}

#[test]
fn a_growth_stamp_takes_the_slots_wind_and_still_never_moves_its_root() {
    let cell = growth_cell();
    // Halfway through the step, at a tick whose instant sits inside a wind packet.
    let mut p = idle_sprout(pack(), cell);
    p.observe(&one_cell_view(WIND_PEAK_TICK - 20, cell, AT_MID));
    p.observe(&one_cell_view(WIND_PEAK_TICK, cell, AT_MID));
    let (v, f) = (one_cell_view(WIND_PEAK_TICK, cell, AT_MID), 1.0);
    let seconds = present_seconds(v.tick, f);
    assert!(
        wind_strength(seconds) > 0.5,
        "the fixture instant is not windy"
    );
    let art = pack();
    let plant = art.plant("lanternstalk").unwrap();
    let budget = p.bend_budget(&plant.name);
    let slot = slot_of(cell);
    let (bend, heading) = slot_wind(&slot, &plant.name, budget, seconds);
    assert!(
        !bend.is_identity(),
        "this slot does not answer the breeze: {bend:?}"
    );
    assert_eq!(
        heading, slot.heading,
        "a side-face plant bends, it does not turn"
    );

    let clip = plant.transition(0, 1).expect("the pilot's growth clip");
    let t = 0.5;
    let [w_from, w_grow, w_to] = growth_weights(t);
    let layers = [
        (stage_pose(plant, 0, cell, seconds), w_from),
        (clip.sample(t * clip.seconds), w_grow),
        (stage_pose(plant, 1, cell, seconds), w_to),
    ];
    let th = stage_thresholds(Band::Foliage);
    let under = stage_opacity(0, AT_MID, &th, MOTIF_OPACITY);
    let opacity = under + (stage_opacity(1, AT_MID, &th, MOTIF_OPACITY) - under) * t as f32;
    let mut want = background(&v, f);
    stamp_at(
        &mut want,
        cell,
        plant,
        budget,
        &layers,
        opacity,
        Mask::None,
        seconds,
    );
    assert_eq!(
        worst_diff(&drawn_with(&mut p, &v, f), &want),
        0.0,
        "the growth stamp is not the slot's own bent stamp"
    );
    // The bend is really doing something: the same stamp without it is a different image.
    let mut calm = background(&v, f);
    let (calm_bend, _) = (Bend::NONE, ());
    stamp_layers_bent(
        &mut calm,
        slot.at,
        heading,
        &layers,
        1.0,
        opacity,
        Mask::None,
        calm_bend,
        &mut Vec::new(),
    );
    assert!(
        worst_diff(&want, &calm) > 0.0,
        "the wind did not bend the growth stamp"
    );

    // And the root stays put. Stamped on the chart's own axes (heading (1, 0), so the
    // tile's rows are the face's rows), the shared root contact of `PLANTS.md` — tile row
    // 14, which this anchor puts on face row 38 — is bit-identical bent or calm, because
    // `PLANT_BEND_ROOT` holds the profile at exactly 0 there.
    let axis = SurfacePoint::new(Face::Front, 32.0, 32.0);
    let along = |bend: Bend| {
        let mut canvas = Canvas::new();
        stamp_layers_bent(
            &mut canvas,
            axis,
            Vec2::new(1.0, 0.0),
            &layers,
            1.0,
            1.0,
            Mask::None,
            bend,
            &mut Vec::new(),
        );
        canvas
    };
    let (still, windy) = (along(Bend::NONE), along(bend));
    assert!(
        (24..40).any(|x| still.get(Face::Front, x, 38).into_iter().any(|c| c > 0.0)),
        "the growth pose paints no root contact row, so the check is vacuous"
    );
    assert!(
        worst_diff(&still, &windy) > 0.0,
        "the bend moved nothing at all"
    );
    for x in 23..41u8 {
        assert_eq!(
            still.get(Face::Front, x, 38),
            windy.get(Face::Front, x, 38),
            "the growth stamp's root contact skated at x = {x}"
        );
    }
}

#[test]
fn a_radial_slot_with_a_clip_plays_it_unmasked_too() {
    // The canopy species carry their own clips (2026-09-13), so the top face's rule is
    // exercised on the shipped umbrellafrond's 0 → 1 and compared against the same pack
    // with that plant's clips stripped — the radial reveal it used before.
    let cell = CellId::all()
        .find(|&c| {
            c.face() == Face::Top
                && plant_cap(Band::Canopy, c) == Some(2)
                && species_of(Band::Canopy, c) == CANOPY_PLANTS[0]
                && (4..=11).contains(&c.cx())
                && (4..=11).contains(&c.cy())
        })
        .expect("a rank-2 umbrellafrond slot in the middle of Top");
    let stripped = || {
        let mut art = pack();
        let target = art
            .plants
            .iter_mut()
            .find(|p| p.name == CANOPY_PLANTS[0])
            .expect("the canopy plant");
        assert!(
            !target.transitions.is_empty(),
            "the shipped canopy plant carries clips"
        );
        target.transitions.clear();
        art
    };
    // The canopy's own thresholds bracket the same two fixture densities.
    let th = stage_thresholds(Band::Canopy);
    assert!(th[0] < AT_SPROUT && AT_SPROUT < th[1] && th[1] < AT_MID && AT_MID < th[2]);
    let mut p = growing(cell, 2);
    let (v, f, t) = (one_cell_view(41, cell, AT_MID), 1.0, 0.5);
    let seconds = present_seconds(v.tick, f);
    let art = pack();
    let plant = art.plant(CANOPY_PLANTS[0]).unwrap();
    let clip = plant
        .transition(0, 1)
        .expect("the shipped canopy 0 → 1 clip");
    let budget = p.bend_budget(&plant.name);
    // A radial plant is never bent: it turns in place, and the reveal it no longer uses
    // measured from that same stationary centre.
    let (bend, heading) = slot_wind(&slot_of(cell), &plant.name, budget, seconds);
    assert!(
        bend.is_identity(),
        "a radial plant must not be bent: {bend:?}"
    );
    let [w_from, w_grow, w_to] = growth_weights(t);
    let layers = [
        (stage_pose(plant, 0, cell, seconds), w_from),
        (clip.sample(t * clip.seconds), w_grow),
        (stage_pose(plant, 1, cell, seconds), w_to),
    ];
    let under = stage_opacity(0, AT_MID, &th, MOTIF_OPACITY);
    let opacity = under + (stage_opacity(1, AT_MID, &th, MOTIF_OPACITY) - under) * t as f32;
    let mut want = background(&v, f);
    stamp_layers_bent(
        &mut want,
        slot_of(cell).at,
        heading,
        &layers,
        1.0,
        opacity,
        Mask::None,
        bend,
        &mut Vec::new(),
    );
    let drawn = drawn_with(&mut p, &v, f);
    assert_eq!(
        worst_diff(&drawn, &want),
        0.0,
        "a top-face clip is not drawn unmasked"
    );
    // With the clips stripped the same slot keeps its radial reveal, which looks different.
    let masked = drawn_with(&mut growing_from(stripped(), cell, 2), &v, f);
    assert!(
        worst_diff(&drawn, &masked) > 0.01,
        "the radial reveal and the clip agree"
    );
}

/// Not a correctness test: the number the brief asks for. Run with
/// `cargo test --release -p cubarium -- --ignored plant_and_body_draw_cost`.
#[test]
#[ignore = "timing, not behaviour"]
fn plant_and_body_draw_cost() {
    let mut view = empty_view();
    // Every cell rich in every band, so every slot grows to its cap.
    for v in view.producer.iter_mut() {
        *v = view.producer_max;
    }
    for v in view.detritus.iter_mut() {
        *v = SOIL_SCALE;
    }
    let mut rng = SplitMix64::new(11);
    view.organisms = (0..200u32)
        .map(|slot| {
            let mut o = organism(slot, rng.next_f64() as f32, Mode::Seeking);
            let face = cube_proto::Face::ALL[(slot as usize) % 5];
            o.pos = SurfacePoint::new(face, rng.range(1.0, 63.0), rng.range(1.0, 63.0));
            o.juvenile = slot % 3 == 0;
            o.gestation = (slot % 5 == 0).then_some(0.5);
            o
        })
        .collect();

    let mut presenter = ArtPresenter::new(pack());
    let mut canvas = Canvas::new();
    presenter.observe(&view);
    // Warm the caches, then time a run of frames.
    for _ in 0..5 {
        presenter.draw(&view, 0.0, &mut canvas);
    }
    let grown = CellId::all()
        .filter(|&c| presenter.stage_of(c).is_some())
        .count();
    let frames = 60;
    // Inside a wind packet by default ([`wind_fixture_tick`]); `CUBARIUM_WIND_TICK` puts
    // the same fixture in a quiet interval to measure the identity path.
    let base = wind_fixture_tick();
    let t0 = std::time::Instant::now();
    for i in 0..frames {
        view.tick = base + i;
        presenter.draw(&view, 0.5, &mut canvas);
    }
    let per = t0.elapsed().as_secs_f64() / frames as f64;
    println!(
        "ArtPresenter::draw: {:.3} ms/frame ({grown} plants, 200 organisms, wind {:.3} at tick {base}) = {:.0}% of a 60 fps budget",
        per * 1e3,
        wind_strength(present_seconds(base, 0.5)),
        per / (1.0 / 60.0) * 100.0
    );
    assert!(
        per < 1.0 / 60.0,
        "draw took {:.3} ms, past the whole 60 fps budget",
        per * 1e3
    );
}
