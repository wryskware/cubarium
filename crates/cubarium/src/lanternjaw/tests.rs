use cubarium_surface::Topology;
use super::raster::HEAD;
use super::*;
use cubarium_render::{Canvas, RigPart, rig_radius};
use cubarium_surface::{SurfacePoint, Vec2};

fn rig_parts(parts: &[Part]) -> Vec<RigPart<'_>> {
    parts.iter().map(Part::rig_part).collect()
}

/// Twenty instants that straddle every keyframe of the study's strike as well as
/// ordinary fractions: the refactor's own fixture.
const INSTANTS: [f64; 20] = [
    0.0, 0.333, 1.25, 3.16, 3.22, 3.28, 3.34, 3.44, 3.54, 5.9, 7.7, 11.99, 0.0417, 0.7183, 2.0891,
    2.9737, 4.4429, 5.0613, 6.6271, 9.3847,
];

/// The refactor's identity: the gallery is the channel rasterizer driven by the study
/// producer, texel for texel, not merely "close".
#[test]
fn parts_is_rasterize_of_the_study_channels() {
    let rig = Lanternjaw::new();
    let (mut a, mut b) = (Vec::new(), Vec::new());
    let mut texels = 0usize;
    for mode in Mode::ALL {
        for t in INSTANTS
            .into_iter()
            .chain((0..120).map(|f| f64::from(f) / 10.0))
        {
            rig.parts(t, mode, &mut a);
            rig.rasterize(&Channels::study(t, mode), &mut b);
            same(&a, &b, "parts is rasterize of the study channels");
            texels += a
                .iter()
                .map(|p| p.sprite.width() * p.sprite.height())
                .sum::<usize>();
        }
    }
    assert!(texels > 500_000, "only {texels} texels compared");
}

/// `Channels::living` with no attack, no meal and no escrow *is* the gallery's `Rest` at
/// movement 0 and its `Move` at movement 1 — the same texels, so every silhouette and
/// palette property of the study still describes the living body.
#[test]
fn the_living_body_at_rest_and_in_motion_is_the_gallery() {
    let rig = Lanternjaw::new();
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for t in INSTANTS {
        for (movement, mode) in [(0.0, Mode::Rest), (1.0, Mode::Move)] {
            let pose = LivingPose {
                ambient: t,
                movement,
                attack: None,
                gut: 0.0,
                cocoon: None,
            };
            rig.parts_living(&pose, &mut a);
            rig.parts(t, mode, &mut b);
            same(&a, &b, "the living body is the gallery mode");
        }
    }
}

/// No escrow, no cocoon; no gut, no breath — whatever else the pose says. Both are
/// *exact*: the cocoon is skipped and the gut breath adds a literal zero.
#[test]
fn without_a_meal_or_an_escrow_nothing_is_added_to_the_body() {
    let rig = Lanternjaw::new();
    let (mut a, mut b) = (Vec::new(), Vec::new());
    let quiet = LivingPose {
        ambient: 2.5,
        movement: 0.3,
        attack: None,
        gut: 0.0,
        cocoon: None,
    };
    rig.parts_living(&quiet, &mut a);
    // A handling phase whose gut is empty, and satiety without an escrow: same picture
    // as the same body with no phase at all once the hush has released.
    rig.parts_living(
        &LivingPose {
            gut: 0.0,
            cocoon: None,
            ..quiet
        },
        &mut b,
    );
    same(&a, &b, "no gut and no escrow");
    // A real meal and a real escrow both change it, so the test is not vacuous.
    rig.parts_living(&LivingPose { gut: 1.0, ..quiet }, &mut b);
    assert!(differs(&a, &b), "a real gut should breathe");
    rig.parts_living(
        &LivingPose {
            cocoon: Some(1.0),
            ..quiet
        },
        &mut b,
    );
    assert!(differs(&a, &b), "a funded escrow should show a cocoon");
    // And `Some(0)` paints no cocoon at all: the reveal starts from nothing.
    rig.parts_living(
        &LivingPose {
            cocoon: Some(0.0),
            ..quiet
        },
        &mut b,
    );
    let unlifted = Channels::living(&LivingPose {
        cocoon: Some(0.0),
        ..quiet
    });
    assert_eq!(
        unlifted.tail_lift, 0.0,
        "an unrevealed cocoon does not lift the tail"
    );
    same(&a, &b, "an escrow at progress 0");
}

fn differs(a: &[Part], b: &[Part]) -> bool {
    a.iter().zip(b).any(|(p, q)| {
        (0..p.sprite.height() as i32).any(|y| {
            (0..p.sprite.width() as i32).any(|x| p.sprite.texel(x, y) != q.sprite.texel(x, y))
        })
    })
}

/// The study's `hunt_state` is the real schedule with `Windup { D = T_SNAP − T_COIL }`,
/// `Strike { D = E = T_OPEN − T_SNAP }` and `Recovering` chained by their displayed
/// reaches — so the two agree over the whole gesture, not only at its ends.
///
/// One documented exception: `hunt_state` carries its accent envelope on past `T_OPEN`
/// (it is still at plateau there and only finishes at `T_SNAP − 0.02 + ACCENT_SECONDS =
/// 3.44`), whereas the real schedule finishes that envelope during the **Strike's
/// settlement** — which the study, recoiling the instant it arrives, has no room for —
/// and `Recovering` is documented to carry no accent at all. The accent is therefore
/// compared over the windup's and the strike's own spans.
#[test]
fn the_phase_schedule_is_the_studys_hunt_state() {
    let windup = |elapsed: f64| AttackEpisode {
        phase: AttackPhase::Windup,
        elapsed,
        duration: T_SNAP - T_COIL,
        from: Reach::FOLDED,
    };
    let strike_from = attack_channels(Some(&windup(T_SNAP - T_COIL))).reach();
    let strike = |elapsed: f64| AttackEpisode {
        phase: AttackPhase::Strike,
        elapsed,
        duration: T_OPEN - T_SNAP,
        from: strike_from,
    };
    let recover_from = attack_channels(Some(&strike(T_OPEN - T_SNAP))).reach();
    let recovering = |elapsed: f64| AttackEpisode {
        phase: AttackPhase::Recovering,
        elapsed,
        duration: T_END - T_OPEN,
        from: recover_from,
    };
    let close = |a: f64, b: f64, what: &str| {
        assert!((a - b).abs() <= 1e-9, "{what}: {a} vs {b}");
    };
    let mut checked = 0usize;
    // 1 ms steps through the whole articulated gesture, plus its four keyframes exactly.
    for step in 0..=440 {
        let t_h = T_COIL + f64::from(step) / 1000.0;
        let (episode, accent_too) = if t_h < T_SNAP {
            (windup(t_h - T_COIL), t_h <= T_SNAP - 0.02)
        } else if t_h <= T_OPEN {
            (strike(t_h - T_SNAP), true)
        } else {
            (recovering(t_h - T_OPEN), false)
        };
        let h = hunt_state(t_h);
        let c = attack_channels(Some(&episode));
        close(c.near_reach, h.reach, &format!("reach at {t_h}"));
        close(c.compress, h.compress, &format!("compress at {t_h}"));
        close(c.lunge, h.lunge, &format!("lunge at {t_h}"));
        close(c.charge, h.charge, &format!("charge at {t_h}"));
        if accent_too {
            close(c.accent, h.accent, &format!("accent at {t_h}"));
        }
        checked += 1;
    }
    assert_eq!(checked, 441);
    // The keyframes themselves, with the strike still owning its settlement boundary.
    for (t_h, episode) in [
        (T_COIL, windup(0.0)),
        (T_SNAP, strike(0.0)),
        (T_OPEN, strike(T_OPEN - T_SNAP)),
        (T_END, recovering(T_END - T_OPEN)),
    ] {
        let h = hunt_state(t_h);
        let c = attack_channels(Some(&episode));
        close(c.near_reach, h.reach, &format!("keyframe reach at {t_h}"));
        close(
            c.compress,
            h.compress,
            &format!("keyframe compress at {t_h}"),
        );
        close(c.lunge, h.lunge, &format!("keyframe lunge at {t_h}"));
        close(c.accent, h.accent, &format!("keyframe accent at {t_h}"));
    }
    // The one post-strike blink lands where the study's does.
    close(
        attack_channels(Some(&recovering(T_END + 0.3 + 0.14 - T_OPEN))).blink,
        hunt_state(T_END + 0.3 + 0.14).blink,
        "the post-recoil blink",
    );
    // And no phase is a phase at all.
    assert_eq!(attack_channels(None), AttackChannels::default());
    assert_eq!(attack_channels(None).reach(), Reach::FOLDED);
}

/// A real one-second strike: the cocked entry pose is held through the paid approach, the
/// extension is the final [`EXTEND_SECONDS`], and full contact is reached **at** the
/// settlement boundary and held past it. No recoil ever comes from the strike itself.
#[test]
fn a_real_strike_holds_its_entry_pose_then_settles_at_full_extension() {
    let from = Reach {
        near: -0.35,
        far: -0.35,
        compress: 1.7,
        lunge: 0.0,
        charge: 1.0,
    };
    let strike = |elapsed: f64| {
        attack_channels(Some(&AttackEpisode {
            phase: AttackPhase::Strike,
            elapsed,
            duration: 1.0,
            from,
        }))
    };
    for t in [0.0, 0.4, 0.8, 0.87, 0.88] {
        let c = strike(t);
        assert_eq!(
            c.near_reach, from.near,
            "the entry pose should be held at {t}"
        );
        assert_eq!(c.compress, from.compress, "compress at {t}");
        assert_eq!(c.lunge, from.lunge, "lunge at {t}");
    }
    // The accent is one continuous envelope that opens its 20 ms *before* the extension —
    // inside the hold, exactly where the study opens it before `T_SNAP` — so it rises
    // from 0 rather than entering at `envelope(0.02 / ACCENT_SECONDS) ≈ 0.103`, and it is
    // still exactly 0 through the approach proper.
    let hold = 1.0 - EXTEND_SECONDS;
    for t in [0.0, 0.4, 0.8, hold - 0.02] {
        assert_eq!(
            strike(t).accent,
            0.0,
            "no accent during the paid approach, at {t}"
        );
    }
    for t in [0.87, 0.88, 0.94, 1.0, 1.1] {
        let want = envelope((t - (hold - 0.02)) / ACCENT_SECONDS);
        assert!(
            (strike(t).accent - want).abs() < 1e-12,
            "the accent at {t} is {}, not the envelope's {want}",
            strike(t).accent
        );
    }
    assert!(strike(0.87).accent > 0.0, "the envelope has opened by 0.87");
    assert_eq!(
        strike(1.5).accent,
        0.0,
        "the accent finishes during the settlement"
    );
    // Mid-extension it is genuinely on the way, not a jump.
    let mid = strike(0.94);
    assert!(
        mid.near_reach > from.near && mid.near_reach < 1.0,
        "mid {mid:?}"
    );
    for t in [1.0, 1.5, 8.0] {
        let c = strike(t);
        let e = Reach::EXTENDED;
        assert!(
            (c.near_reach - e.near).abs() < 1e-12,
            "near at {t}: {}",
            c.near_reach
        );
        assert!(
            (c.compress - e.compress).abs() < 1e-12,
            "compress at {t}: {}",
            c.compress
        );
        assert!(
            (c.lunge - e.lunge).abs() < 1e-12,
            "lunge at {t}: {}",
            c.lunge
        );
        assert_eq!(c.hush, 1.0, "a strike hushes the ambient body at {t}");
    }
    // Eight seconds in — longer than the study's whole cycle — it has still not struck
    // twice and has still not recoiled.
    assert!(strike(8.0).near_reach >= 1.0 - 1e-12);
}

/// A windup held far past its own duration cocks and stays cocked: no extension, no
/// contact, no synthetic strike.
#[test]
fn a_held_windup_never_extends() {
    for t in [0.6, 1.0, 6.5, 8.0, 60.0] {
        let c = attack_channels(Some(&AttackEpisode {
            phase: AttackPhase::Windup,
            elapsed: t,
            duration: 0.6,
            from: Reach::FOLDED,
        }));
        assert!(
            (c.near_reach - -0.35).abs() < 1e-12,
            "at {t}: near {}",
            c.near_reach
        );
        assert!(c.far_reach <= 0.0, "at {t}: far {}", c.far_reach);
        assert_eq!(c.lunge, 0.0, "at {t}");
        assert_eq!(c.accent, 0.0, "at {t}");
        assert!(
            (c.compress - 1.7).abs() < 1e-12,
            "at {t}: compress {}",
            c.compress
        );
    }
}

/// An interrupted motion recoils from the reach that was actually on screen.
#[test]
fn recovering_starts_from_the_reach_it_was_handed() {
    let from = Reach {
        near: 0.4,
        far: 0.4,
        compress: -0.1,
        lunge: 0.44,
        charge: 0.5,
    };
    let episode = |elapsed: f64| AttackEpisode {
        phase: AttackPhase::Recovering,
        elapsed,
        duration: 5.0,
        from,
    };
    let start = attack_channels(Some(&episode(0.0)));
    assert_eq!(
        start.near_reach, 0.4,
        "the recoil starts from the displayed reach"
    );
    assert_eq!(start.far_reach, 0.4, "and so does the far limb");
    assert_eq!(start.compress, -0.1);
    assert_eq!(start.lunge, 0.44);
    // Negative elapsed is the same instant, never a rewind into the previous phase.
    assert_eq!(attack_channels(Some(&episode(-1.0))), start);
    let done = attack_channels(Some(&episode(RECOIL_SECONDS)));
    assert!(
        done.near_reach.abs() < 1e-12 && done.lunge.abs() < 1e-12,
        "{done:?}"
    );
    // Folded stillness for the rest of the recovery, and the hush stays on.
    for t in [0.5, 2.0, 4.9, 9.0] {
        let c = attack_channels(Some(&episode(t)));
        assert!(
            c.near_reach.abs() < 1e-12 && c.lunge.abs() < 1e-12,
            "at {t}: {c:?}"
        );
        assert_eq!(c.hush, 1.0, "at {t}");
    }
    // `Handling` recoils identically but releases the hush over the documented second.
    let handling = |elapsed: f64| AttackEpisode {
        phase: AttackPhase::Handling,
        elapsed,
        ..episode(elapsed)
    };
    assert_eq!(attack_channels(Some(&handling(0.0))).near_reach, 0.4);
    assert_eq!(attack_channels(Some(&handling(RECOIL_SECONDS))).hush, 1.0);
    assert_eq!(
        attack_channels(Some(&handling(RECOIL_SECONDS + HANDLING_RELEASE_SECONDS))).hush,
        0.0
    );
    let half = attack_channels(Some(&handling(RECOIL_SECONDS + 0.5))).hush;
    assert!(
        (half - 0.5).abs() < 1e-12,
        "the release is a smoothstep: {half}"
    );
}

/// The far forelimb is the near one 45 ms of **attack** time ago, and before the episode
/// began it is whatever the previous episode last displayed — never a modulo of the
/// presentation clock and never a replayed earlier attack.
#[test]
fn the_far_forelimb_lags_the_near_one_by_far_lag_seconds() {
    let from = Reach {
        near: -0.2,
        far: 0.31,
        compress: 0.5,
        lunge: 0.2,
        charge: 0.4,
    };
    for phase in [
        AttackPhase::Windup,
        AttackPhase::Strike,
        AttackPhase::Recovering,
        AttackPhase::Handling,
    ] {
        let at = |elapsed: f64| {
            attack_channels(Some(&AttackEpisode {
                phase,
                elapsed,
                duration: 1.0,
                from,
            }))
        };
        // Before the lag has elapsed, the far limb holds the reach it was handed.
        for t in [0.0, 0.02, 0.0449] {
            assert_eq!(
                at(t).far_reach,
                from.far,
                "{phase:?} at {t} should hold from.far"
            );
        }
        for t in [FAR_LAG_SECONDS, 0.3, 0.9, 1.0, 2.0] {
            let far = at(t).far_reach;
            let near_before = at(t - FAR_LAG_SECONDS).near_reach;
            assert!(
                (far - near_before).abs() < 1e-15,
                "{phase:?} at {t}: far {far} should be the near reach at {}: {near_before}",
                t - FAR_LAG_SECONDS
            );
        }
    }
}

/// The gut breath is a *breath*: a sub-pixel swell of template columns 4..=8 and of the
/// parts that ride them (the abdomen hull piece, its lanterns' bloom in `Glow`, and the
/// one walking leg based on column 8), and of nothing else. The tail, the thorax, the
/// head and both forelimbs — which hang off the head's column — never move with a meal.
#[test]
fn the_gut_breath_swells_the_abdomen_columns_and_nothing_else() {
    let rig = Lanternjaw::new();
    let (mut empty, mut full) = (Vec::new(), Vec::new());
    // A quarter of the breath period past zero is the swell's peak.
    let pose = LivingPose {
        ambient: GUT_BREATH_SECONDS / 4.0,
        movement: 0.0,
        attack: None,
        gut: 0.0,
        cocoon: None,
    };
    rig.parts_living(&pose, &mut empty);
    rig.parts_living(&LivingPose { gut: 0.8, ..pose }, &mut full);
    let mut worst = [0.0f32; 8];
    for (i, (a, b)) in empty.iter().zip(&full).enumerate() {
        for y in 0..a.sprite.height() as i32 {
            for x in 0..a.sprite.width() as i32 {
                let (p, q) = (a.sprite.texel(x, y), b.sprite.texel(x, y));
                for c in 0..4 {
                    worst[i] = worst[i].max((p[c] - q[c]).abs());
                }
            }
        }
    }
    eprintln!(
        "gut breath 0.8 at its peak ({} px): worst texel change per part {:?}",
        GUT_BREATH_PX * 0.8,
        PartName::ALL.iter().zip(worst).collect::<Vec<_>>()
    );
    for (name, w) in PartName::ALL.into_iter().zip(worst) {
        let rides = matches!(
            name,
            PartName::Abdomen | PartName::Glow | PartName::Underside
        );
        if rides {
            assert!(
                w > 0.01,
                "{name:?} rides the abdomen columns but moved by {w}"
            );
        } else {
            assert_eq!(w, 0.0, "{name:?} must not move with a meal");
        }
    }
}

/// `Channels::living` reproduces the study's hunt gains at full hush, which is what makes
/// one hushed body and the gallery's hunt the same kind of picture.
#[test]
fn a_full_hush_reproduces_the_studys_hunt_wave_and_chain_gain() {
    let pose = LivingPose {
        ambient: 1.0,
        movement: 0.0,
        attack: Some(AttackEpisode {
            phase: AttackPhase::Windup,
            elapsed: 0.3,
            duration: 0.6,
            from: Reach::FOLDED,
        }),
        gut: 0.0,
        cocoon: None,
    };
    let ch = Channels::living(&pose);
    let a = attack_channels(pose.attack.as_ref());
    assert_eq!(a.hush, 1.0);
    assert!(
        (ch.waves[0].0 - 0.22).abs() < 1e-15,
        "hushed wave {:?}",
        ch.waves
    );
    assert!(
        (ch.pulse_gain - (0.38 + 0.92 * a.charge)).abs() < 1e-15,
        "chain gain {}",
        ch.pulse_gain
    );
    // Released, the ambient body is back: no attack at all is the rest wave and gain 1.
    let quiet = Channels::living(&LivingPose {
        attack: None,
        ..pose
    });
    assert_eq!(quiet.waves[0].0, 0.55);
    assert_eq!(quiet.pulse_gain, 1.0);
}

/// Every mode, every 60 fps instant of twelve seconds: eight parts in painting order,
/// every part inside its own budget and the whole rig inside its query bound. The
/// footprint itself is asserted inside `parts` (debug builds), which this exercises.
#[test]
fn every_frame_of_every_mode_builds_eight_bounded_parts() {
    let rig = Lanternjaw::new();
    let mut parts = Vec::new();
    let mut worst_extent = 0.0f64;
    let mut worst_radius = 0.0f64;
    for mode in Mode::ALL {
        for frame in 0..(12 * 60) {
            rig.parts(f64::from(frame) / 60.0, mode, &mut parts);
            assert_eq!(parts.len(), 8, "{mode:?} frame {frame}");
            for (part, name) in parts.iter().zip(PartName::ALL) {
                assert_eq!(part.name, name, "{mode:?}: parts are in painting order");
                worst_extent = worst_extent.max(part.sprite.extent());
                assert!(
                    part.sprite.extent() <= PART_EXTENT_MAX,
                    "{mode:?} {name:?}: extent {}",
                    part.sprite.extent()
                );
            }
            let radius = rig_radius(&[(&rig_parts(&parts), 1.0)]);
            worst_radius = worst_radius.max(radius);
            assert!(
                radius <= QUERY_RADIUS_MAX,
                "{mode:?} frame {frame}: radius {radius}"
            );
        }
    }
    // Non-vacuity: the budgets are actually approached, not trivially satisfied.
    assert!(
        worst_extent > 5.0 && worst_radius > 12.0,
        "{worst_extent} / {worst_radius}"
    );
}

/// The hull pieces, the underside and the glow share the body lattice: integer offsets
/// and integer sprite pivots, so their texel centres coincide in body coordinates.
#[test]
fn the_summed_material_parts_share_the_body_lattice() {
    let rig = Lanternjaw::new();
    let mut parts = Vec::new();
    rig.parts(1.25, Mode::Hunt, &mut parts);
    for part in &parts {
        let lattice = matches!(
            part.name,
            PartName::Tail
                | PartName::Abdomen
                | PartName::Thorax
                | PartName::Head
                | PartName::Underside
                | PartName::Glow
        );
        if !lattice {
            continue;
        }
        let (offset, pivot) = (part.offset, part.sprite.pivot());
        assert_eq!(offset.x.fract(), 0.0, "{:?} offset {offset:?}", part.name);
        assert_eq!(offset.y.fract(), 0.0, "{:?} offset {offset:?}", part.name);
        assert_eq!(pivot.x.fract(), 0.0, "{:?} pivot {pivot:?}", part.name);
        assert_eq!(pivot.y.fract(), 0.0, "{:?} pivot {pivot:?}", part.name);
    }
    // And the four hull pieces are one material in `PartName` column order.
    let offsets: Vec<f64> = parts
        .iter()
        .filter(|p| p.name.layer() == 2)
        .map(|p| p.offset.x)
        .collect();
    assert_eq!(
        offsets,
        vec![-7.0, -3.0, 2.0, 6.0],
        "left column dx + 2 per piece"
    );
}

/// A pure function of `(seconds, mode)`: a repeated call is the same sprite texel for
/// texel, and a non-finite instant reads as 0.
#[test]
fn parts_are_a_pure_function_of_the_instant_and_the_mode() {
    let rig = Lanternjaw::new();
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for (seconds, mode) in [(0.0, Mode::Rest), (3.3, Mode::Hunt), (7.77, Mode::Move)] {
        rig.parts(seconds, mode, &mut a);
        rig.parts(seconds, mode, &mut b);
        same(&a, &b, "a repeated call");
    }
    rig.parts(f64::NAN, Mode::Bud, &mut a);
    rig.parts(0.0, Mode::Bud, &mut b);
    same(&a, &b, "a non-finite instant reads as 0");
    // The hunt cycle is exactly periodic, so a loop has no step.
    rig.parts(1.75, Mode::Hunt, &mut a);
    rig.parts(1.75 + HUNT_PERIOD, Mode::Hunt, &mut b);
    same(&a, &b, "the hunt cycle at t and t + HUNT_PERIOD");
}

fn same(a: &[Part], b: &[Part], what: &str) {
    assert_eq!(a.len(), b.len());
    for (p, q) in a.iter().zip(b) {
        assert_eq!(p.name, q.name);
        assert_eq!(p.offset, q.offset, "{what}: {:?}", p.name);
        for y in 0..p.sprite.height() as i32 {
            for x in 0..p.sprite.width() as i32 {
                assert_eq!(
                    p.sprite.texel(x, y),
                    q.sprite.texel(x, y),
                    "{what}: {:?} texel ({x}, {y})",
                    p.name
                );
            }
        }
    }
}

/// The strike's warm accent belongs to the hunt alone, and it is an envelope rather than
/// a flash: the jaw walks to it and back through intermediate colours.
#[test]
fn the_strike_accent_is_a_hunt_only_envelope() {
    let rig = Lanternjaw::new();
    let mut parts = Vec::new();
    // The jaw's own colour is the accent's carrier; sample the Head piece's peak red.
    let mut hottest = |mode: Mode| {
        let mut peak = 0.0f32;
        for frame in 0..(12 * 60) {
            rig.parts(f64::from(frame) / 60.0, mode, &mut parts);
            let head = &parts[HEAD].sprite;
            for y in 0..head.height() as i32 {
                for x in 0..head.width() as i32 {
                    let t = head.texel(x, y);
                    // The accent is the only colour whose red far exceeds its blue.
                    if t[3] > 0.5 && t[0] > t[2] * 1.5 {
                        peak = peak.max(t[0]);
                    }
                }
            }
        }
        peak
    };
    let hunt = hottest(Mode::Hunt);
    assert!(
        hunt > 0.2,
        "the hunt's accent should reach the warm colour, got {hunt}"
    );
    for quiet in [Mode::Rest, Mode::Move, Mode::Bud] {
        assert_eq!(hottest(quiet), 0.0, "{quiet:?} showed the strike accent");
    }
    // Intermediate values, with no single-frame jump: the envelope's largest step at
    // 60 fps is far below its own swing.
    let mut previous = hunt_state(T_SNAP - 0.2).accent;
    let mut largest = 0.0f64;
    for frame in 0..60 {
        let accent = hunt_state(T_SNAP - 0.2 + f64::from(frame) / 60.0).accent;
        largest = largest.max((accent - previous).abs());
        previous = accent;
    }
    assert!(
        largest > 0.0 && largest <= 0.31,
        "largest accent step {largest}"
    );
}

/// At full extension the near limb is *in front of* the hull where the two overlap, and
/// its claw — its brightest texel, in the claw colour — is deliberately **past** the hull
/// altogether: `fable.md` has the tip four pixels beyond the jaw at `dx + 8`, which is
/// what [`BOUND_FRONT`] (12.3 plus the lunge) is sized for. Both halves matter: the depth
/// is observable on the shoulder, and the claw is not on the carapace at all.
#[test]
fn at_the_strike_the_near_limb_is_over_the_hull_and_its_claw_reaches_past_it() {
    use cubarium_render::stamp_rig;
    use cubarium_surface::Face;

    let rig = Lanternjaw::new();
    let mut parts = Vec::new();
    rig.parts(T_OPEN, Mode::Hunt, &mut parts);
    let anchor = SurfacePoint::pixel_center(Topology::Cube, Face::Front, 32, 32);
    let heading = Vec2::new(1.0, 0.0);
    let subset = |keep: &dyn Fn(PartName) -> bool| {
        let chosen: Vec<RigPart<'_>> = parts
            .iter()
            .filter(|p| keep(p.name))
            .map(Part::rig_part)
            .collect();
        let mut canvas = Canvas::new();
        stamp_rig(
            &mut canvas,
            anchor,
            heading,
            &[(&chosen, 1.0)],
            1.0,
            &mut Vec::new(),
        );
        canvas
    };
    let whole = subset(&|_| true);
    let without_near = subset(&|n| n != PartName::NearLimb);
    let hull = subset(&|n| n.layer() == 2);
    let near = subset(&|n| n == PartName::NearLimb);
    let light = |c: &Canvas, x: u16, y: u16| {
        c.get(Face::Front, x, y)
            .into_iter()
            .map(f64::from)
            .sum::<f64>()
    };

    let mut over_hull = 0usize;
    for x in 0..64u16 {
        for y in 0..64u16 {
            if light(&hull, x, y) <= 0.0 || light(&near, x, y) <= 0.0 {
                continue;
            }
            let moved = (0..3)
                .map(|c| {
                    (whole.get(Face::Front, x, y)[c] - without_near.get(Face::Front, x, y)[c]).abs()
                })
                .fold(0.0f32, f32::max);
            if moved > 0.005 {
                over_hull += 1;
            }
        }
    }
    assert!(
        over_hull >= 3,
        "the near limb changed only {over_hull} hull pixels"
    );

    // The brightest near-limb pixel is the claw, ahead of the hull's own front.
    let (bx, by) = (0..64u16)
        .flat_map(|x| (0..64u16).map(move |y| (x, y)))
        .max_by(|&a, &b| {
            light(&near, a.0, a.1)
                .partial_cmp(&light(&near, b.0, b.1))
                .unwrap()
        })
        .unwrap();
    assert!(
        bx >= 43,
        "the claw should reach past the jaw, but the brightest pixel is at {bx}"
    );
    assert_eq!(
        light(&hull, bx, by),
        0.0,
        "the claw at ({bx}, {by}) is past the hull, so no hull pixel is there"
    );
}
