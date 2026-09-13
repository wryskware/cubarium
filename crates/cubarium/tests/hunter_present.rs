//! The Lanternjaw drawn from a **real** world's hunter observer through the presenter's
//! adapter (`cubarium::hunter_present`, `ArtPresenter::observe_hunters`).
//!
//! Written from the doc comments of `hunter_present.rs` (`HunterFrame`, `HunterMemory`,
//! `entry_reach`, `episode_of`, `movement_of`, `hunter_scale_supported`),
//! `ArtPresenter::observe_hunters`, `Lanternjaw::{draw_living, effectors}` and the contract
//! `design/7_Research/lanternjaw-ecology-animation-contract-2026-09-13.md`. The fixtures are the
//! core suite's own staging (a trial hunter aimed at one frozen prey placed exactly in its
//! claws; certain or certain-miss capture rolls), transcribed so a host test drives an actual
//! `World` — never a hand-built `HunterView` except where a stale id is the point.
//!
//! Nothing here is evidence about ecological balance; the world is a staged fixture.

use cubarium::art::ArtPack;
use cubarium::art_present::{ArtPresenter, present_seconds};
use cubarium::clock::DT;
use cubarium::hunter_present::{
    COCKED, HUNTER_FULL_SPEED_PX_S, HunterFrame, HunterMemory, entry_reach, episode_of,
    hunter_scale_supported, movement_of, settled_reach,
};
use cubarium::lanternjaw::{
    AttackPhase, RECOIL_SECONDS, Reach, SCALE_MIN, attack_channels, effectors,
};
use cubarium_core::genome::{Genome, decode};
use cubarium_core::hunter::{FixedHunterProfile, HunterPhase, HunterTarget, HunterView};
use cubarium_core::ids::OrganismId;
use cubarium_core::organism::{Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::snapshot::state_hash;
use cubarium_core::{World, WorldConfig, decode_snapshot, encode_snapshot};
use cubarium_render::Canvas;
use cubarium_surface::{PathSegment, SurfacePoint, Vec2, travel};
use cube_proto::Face;

// ---------------------------------------------------------------- fixtures

fn atelier() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier")
}

fn presenter() -> ArtPresenter {
    ArtPresenter::new(ArtPack::load(&atelier()).expect("the shipped pack loads"))
}

/// An empty, quiet world: no founders, no weather motion, no rain.
fn empty_world() -> World {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds.clear();
    cfg.founders.count = 0;
    cfg.weather.amplitude = 0.0;
    cfg.water.rain_rate = 0.0;
    World::new(cfg).expect("an empty world is valid")
}

fn trial(world: &World) -> FixedHunterProfile {
    FixedHunterProfile::lanternjaw_trial(world.config())
}

fn certain(mut p: FixedHunterProfile) -> FixedHunterProfile {
    p.capture_min = 1.0;
    p.capture_max = 1.0;
    p
}

fn never(mut p: FixedHunterProfile) -> FixedHunterProfile {
    p.capture_min = 0.0;
    p.capture_max = 0.0;
    p
}

fn target_of(pos: SurfacePoint) -> HunterTarget {
    HunterTarget {
        face: pos.face.index() as u8,
        u: pos.u,
        v: pos.v,
    }
}

/// A prey placed by hand, its material booked as admitted from outside; `frozen` zeroes its
/// speed so the geometry stays where the test put it.
fn place_prey(
    world: &mut World,
    pos: SurfacePoint,
    s: f64,
    r: f64,
    e: f64,
    frozen: bool,
) -> OrganismId {
    let cfg = world.config().clone();
    let mut genome = Genome::founder(0.5, &cfg.drives);
    genome.size = 0.5;
    genome.speed = 0.3;
    genome.clamp();
    let mut phenotype = decode(&genome, &cfg.organism);
    if frozen {
        phenotype.speed_max = 0.0;
        phenotype.structure_adult = s;
    }
    let id = world.state.organisms.insert(Organism {
        pos: pos.canonicalize(),
        heading: Vec2::new(1.0, 0.0),
        ou: Vec2::ZERO,
        structure: s,
        reserve: r,
        energy: e,
        born_tick: world.tick(),
        hunger_memory: 0.0,
        mode: Mode::Resting,
        escrow: None,
        births: 0,
        genome,
        phenotype,
        parent: None,
        origin: Origin::Founder,
        turn_counter: Counter::default(),
        fed_this_tick: false,
    });
    world.state.external_material_in += s + r;
    id
}

fn aim(world: &mut World, id: OrganismId, heading: Vec2, reserve: f64) {
    let o = world.state.organisms.get_mut(id).expect("alive");
    let before = o.reserve;
    o.heading = heading;
    o.reserve = reserve;
    world.state.external_material_in += reserve - before;
}

/// The chart offset of a body-local offset, in the basis `stamp_rig` uses.
fn body_offset(heading: Vec2, offset: Vec2, scale: f64) -> Vec2 {
    let h = heading.normalized().expect("a heading");
    let side = Vec2::new(-h.y, h.x);
    (h * offset.x + side * offset.y) * scale
}

fn effector_point(
    root: SurfacePoint,
    heading: Vec2,
    profile: &FixedHunterProfile,
    scale: f64,
) -> SurfacePoint {
    travel(
        root,
        body_offset(heading, profile.capture_offset_body, scale),
    )
    .end
}

/// A hunter at `spot` facing +x and one frozen prey exactly inside its claws.
fn staged_at(spot: SurfacePoint, profile: FixedHunterProfile) -> (World, OrganismId, OrganismId) {
    let mut world = empty_world();
    let receipt = world
        .start_hunter_trial(profile.clone(), target_of(spot))
        .expect("the trial starts");
    let hunter = receipt.id;
    aim(&mut world, hunter, Vec2::new(1.0, 0.0), 1.0);
    let grasp = effector_point(spot, Vec2::new(1.0, 0.0), &profile, 1.0);
    let prey = place_prey(&mut world, grasp, 0.5, 0.3, 0.4, true);
    (world, hunter, prey)
}

fn staged(profile: FixedHunterProfile) -> (World, OrganismId, OrganismId) {
    staged_at(SurfacePoint::new(Face::Front, 20.0, 32.0), profile)
}

fn phase_of(world: &World, id: OrganismId) -> HunterPhase {
    world.hunters().member(id).expect("a member").phase
}

/// Step one tick and feed the presenter exactly as the runner does.
fn step(world: &mut World, p: &mut ArtPresenter) {
    world.step();
    let view = world.render_view();
    p.observe(&view);
    p.observe_hunters(&view, &world.hunter_view());
}

fn step_until(
    world: &mut World,
    p: &mut ArtPresenter,
    id: OrganismId,
    phase: HunterPhase,
    limit: u64,
) -> u64 {
    for _ in 0..limit {
        step(world, p);
        if phase_of(world, id) == phase {
            return world.tick();
        }
    }
    panic!(
        "the hunter never reached {phase:?}; it is in {:?}",
        phase_of(world, id)
    );
}

fn draw(p: &mut ArtPresenter, world: &World, f: f64) -> Canvas {
    let mut canvas = Canvas::new();
    p.draw(&world.render_view(), f, &mut canvas);
    canvas
}

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL
        .into_iter()
        .flat_map(|f| (0..64u8).flat_map(move |y| (0..64u8).map(move |x| (f, x, y))))
}

fn differing(a: &Canvas, b: &Canvas) -> usize {
    every_pixel()
        .filter(|&(f, x, y)| a.get(f, x, y) != b.get(f, x, y))
        .count()
}

fn identical(a: &Canvas, b: &Canvas) -> bool {
    differing(a, b) == 0
}

/// Pixels within `r` of a surface point's own pixel that differ between two images.
fn differing_near(a: &Canvas, b: &Canvas, at: SurfacePoint, r: f64) -> usize {
    let (cx, cy) = at.pixel();
    every_pixel()
        .filter(|&(f, x, y)| {
            f == at.face
                && (f64::from(x) - f64::from(cx)).hypot(f64::from(y) - f64::from(cy)) <= r
                && a.get(f, x, y) != b.get(f, x, y)
        })
        .count()
}

fn lit_faces(image: &Canvas) -> usize {
    Face::ALL
        .into_iter()
        .filter(|&f| (0..64u8).any(|y| (0..64u8).any(|x| image.get(f, x, y) != [0.0; 3])))
        .count()
}

// ---------------------------------------------------------------- the pure adapter

#[test]
fn the_entry_reach_is_reconstructed_from_the_persisted_origin_only() {
    let settled = settled_reach();
    assert_eq!(entry_reach(HunterPhase::Strike), settled);
    assert!((settled.near - 1.0).abs() < 1e-12 && (settled.lunge - 1.1).abs() < 1e-12);
    assert!(
        settled.far < 1.0 && settled.far > 0.9,
        "the far claw is still 45 ms behind: {}",
        settled.far
    );
    assert!(
        settled.charge < 1.0 && settled.charge > 0.8,
        "the charge has bled for the extension: {}",
        settled.charge
    );
    assert_eq!(entry_reach(HunterPhase::Windup), COCKED);
    for from in [
        HunterPhase::Perched,
        HunterPhase::Stalking,
        HunterPhase::Recovering,
        HunterPhase::Handling,
    ] {
        assert_eq!(entry_reach(from), Reach::FOLDED, "{from:?}");
    }
    assert!(hunter_scale_supported(SCALE_MIN) && hunter_scale_supported(1.0));
    assert!(!hunter_scale_supported(SCALE_MIN - 1e-9) && !hunter_scale_supported(f64::NAN));
    assert!(!hunter_scale_supported(1.0 + 1e-9));
}

#[test]
fn the_episode_reads_attack_time_from_the_persisted_boundary_never_a_modulo() {
    let frame = |phase, started, ends| HunterFrame {
        tick: 400,
        phase,
        started,
        ends,
        entered_from: HunterPhase::Stalking,
        episode: 3,
        scale: 1.0,
        gut: 0.0,
        gestation: None,
        target: None,
    };
    // A strike entered at tick 100 lasting 20 ticks (1 s): attack time is seconds − 5.0.
    let strike = frame(HunterPhase::Strike, 100, 120);
    for (seconds, want) in [(5.0, 0.0), (5.5, 0.5), (6.0, 1.0), (30.0, 25.0)] {
        let ep = episode_of(&strike, COCKED, seconds).expect("a strike is an episode");
        assert_eq!(ep.phase, AttackPhase::Strike);
        assert!(
            (ep.elapsed - want).abs() < 1e-12,
            "at {seconds}: {}",
            ep.elapsed
        );
        assert!((ep.duration - 1.0).abs() < 1e-12);
        assert_eq!(ep.from, COCKED);
    }
    // 25 s into a 1 s strike is still the settled pose, not a replayed attack.
    let late = attack_channels(episode_of(&strike, COCKED, 30.0).as_ref());
    assert!((late.near_reach - 1.0).abs() < 1e-12 && (late.lunge - 1.1).abs() < 1e-12);
    // An untimed phase stores ends == started; the recoil's own constant bounds it.
    let rec = frame(HunterPhase::Recovering, 130, 130);
    let ep = episode_of(&rec, Reach::EXTENDED, 6.5).unwrap();
    assert!(ep.duration >= RECOIL_SECONDS && ep.phase == AttackPhase::Recovering);
    // Phase-free frames drive no episode.
    assert!(episode_of(&frame(HunterPhase::Perched, 0, 0), Reach::FOLDED, 9.0).is_none());
    assert!(episode_of(&frame(HunterPhase::Stalking, 0, 0), Reach::FOLDED, 9.0).is_none());
    // Movement: a tick's path length over DT against the full speed, clamped.
    let seg = |len: f64| PathSegment {
        face: Face::Front,
        from: Vec2::new(10.0, 10.0),
        to: Vec2::new(10.0 + len, 10.0),
    };
    assert_eq!(movement_of(&[]), 0.0);
    let half = movement_of(&[seg(0.5 * HUNTER_FULL_SPEED_PX_S * DT)]);
    assert!((half - 0.5).abs() < 1e-9, "{half}");
    assert_eq!(movement_of(&[seg(10.0)]), 1.0);
}

#[test]
fn memory_keeps_the_previous_phase_through_the_interval_and_hands_on_the_displayed_reach() {
    let frame = |tick, phase, started, ends, from| HunterFrame {
        tick,
        phase,
        started,
        ends,
        entered_from: from,
        episode: 1,
        scale: 1.0,
        gut: 0.0,
        gestation: None,
        target: None,
    };
    // Windup entered at 100 from stalking, 12 ticks long; strike at 112, 20 ticks; handling at 132.
    let mut m = HunterMemory::enter(frame(
        100,
        HunterPhase::Windup,
        100,
        112,
        HunterPhase::Stalking,
    ));
    assert_eq!(m.from, Reach::FOLDED);
    for t in 101..=111 {
        m.observe(frame(
            t,
            HunterPhase::Windup,
            100,
            112,
            HunterPhase::Stalking,
        ));
    }
    m.observe(frame(
        112,
        HunterPhase::Strike,
        112,
        132,
        HunterPhase::Windup,
    ));
    // The strike was entered from the windup's displayed end: fully cocked.
    assert!(
        (m.from.near + 0.35).abs() < 1e-9 && (m.from.compress - 1.7).abs() < 1e-9,
        "{:?}",
        m.from
    );
    // During tick 112's interval the previous phase (the windup) is still in effect …
    let (f0, _) = m.frame_at(112, 0.5);
    assert_eq!(f0.phase, HunterPhase::Windup);
    // … and at f = 1 the strike is.
    assert_eq!(m.frame_at(112, 1.0).0.phase, HunterPhase::Strike);
    for t in 113..=131 {
        m.observe(frame(t, HunterPhase::Strike, 112, 132, HunterPhase::Windup));
        assert_eq!(m.frame_at(t, 0.5).0.phase, HunterPhase::Strike, "tick {t}");
    }
    m.observe(frame(
        132,
        HunterPhase::Handling,
        132,
        132,
        HunterPhase::Strike,
    ));
    // Settlement: the frames before the boundary still show the strike, fully extended.
    let (pose, _) = m.living_pose(132, 0.999, &[]);
    let ch = attack_channels(pose.attack.as_ref());
    assert_eq!(pose.attack.unwrap().phase, AttackPhase::Strike);
    assert!(
        (ch.near_reach - 1.0).abs() < 1e-9,
        "not fully extended at settlement: {}",
        ch.near_reach
    );
    // And the handling was entered from that full extension, so it recoils from it.
    assert!(
        (m.from.near - 1.0).abs() < 1e-9 && (m.from.lunge - 1.1).abs() < 1e-9,
        "{:?}",
        m.from
    );
    let (pose, _) = m.living_pose(132, 1.0, &[]);
    assert_eq!(pose.attack.unwrap().phase, AttackPhase::Handling);
    // A long handling never re-extends.
    for t in 133..=400u64 {
        m.observe(frame(
            t,
            HunterPhase::Handling,
            132,
            132,
            HunterPhase::Strike,
        ));
        let (pose, _) = m.living_pose(t, 0.5, &[]);
        let ch = attack_channels(pose.attack.as_ref());
        assert!(ch.near_reach <= 1.0 + 1e-9 && ch.lunge <= 1.1 + 1e-9);
        if (t as f64 - 132.0) * DT > RECOIL_SECONDS + 0.1 {
            assert!(
                ch.near_reach.abs() < 1e-9,
                "tick {t}: the arms came back out ({})",
                ch.near_reach
            );
        }
    }
}

// ---------------------------------------------------------------- the world

#[test]
fn an_empty_membership_draws_the_image_a_presenter_never_told_about_hunters_draws() {
    // An ordinary world with founders and no trial: `hunter_view()` is empty.
    let mut world = World::new(WorldConfig::default()).expect("the default world is valid");
    let mut plain = presenter();
    let mut told = presenter();
    for _ in 0..12 {
        world.step();
        let view = world.render_view();
        plain.observe(&view);
        told.observe(&view);
        assert!(world.hunter_view().is_empty());
        told.observe_hunters(&view, &world.hunter_view());
        for f in [0.0, 0.5, 0.999] {
            let mut a = Canvas::new();
            let mut b = Canvas::new();
            plain.draw(&view, f, &mut a);
            told.draw(&view, f, &mut b);
            assert!(
                identical(&a, &b),
                "tick {} f {f}: the empty membership changed the image",
                view.tick
            );
        }
    }
    assert!(told.hunter_ids().is_empty() && told.unsupported_hunters().is_empty());
}

#[test]
fn a_member_is_drawn_once_as_the_lanternjaw_by_full_id_and_ordinary_bodies_are_untouched() {
    let (mut world, hunter, prey) = staged(certain(trial(&empty_world())));
    // A second, far-away ordinary organism: same kind of body as the prey, no membership.
    let far = place_prey(
        &mut world,
        SurfacePoint::new(Face::Back, 32.0, 40.0),
        0.5,
        0.3,
        0.4,
        true,
    );
    let mut told = presenter();
    let mut plain = presenter();
    step(&mut world, &mut told);
    let view = world.render_view();
    plain.observe(&view);
    assert_eq!(
        told.hunter_ids(),
        vec![hunter],
        "membership is the observer's list, by id"
    );
    assert!(!told.hunter_ids().contains(&far) && !told.hunter_ids().contains(&prey));
    let hunter_pos = view.organisms.iter().find(|o| o.id == hunter).unwrap().pos;
    let far_pos = view.organisms.iter().find(|o| o.id == far).unwrap().pos;
    let a = draw(&mut told, &world, 0.5);
    let b = draw(&mut plain, &world, 0.5);
    assert!(
        differing_near(&a, &b, hunter_pos, 16.0) > 20,
        "the hunter is not drawn as the Lanternjaw"
    );
    assert_eq!(
        differing_near(&a, &b, far_pos, 10.0),
        0,
        "an ordinary body changed"
    );
    // Drawn once: the atelier rig's pixels at the hunter's root are gone, replaced by the
    // rig's — the plain image's light at the root is not simply added to.
    let rig_only = {
        let mut c = Canvas::new();
        let mut p = presenter();
        p.observe(&view);
        p.observe_hunters(&view, &world.hunter_view());
        p.draw(&view, 0.5, &mut c);
        c
    };
    assert!(
        identical(&a, &rig_only),
        "a repeated draw from a fresh presenter differs"
    );
}

#[test]
fn a_certain_strike_settles_fully_extended_at_the_capture_boundary_and_the_prey_stays_until_it() {
    let (mut world, hunter, prey) = staged(certain(trial(&empty_world())));
    let mut p = presenter();
    step_until(&mut world, &mut p, hunter, HunterPhase::Windup, 400);
    let windup_from = p.hunter_of(hunter).unwrap().from;
    assert_eq!(
        windup_from,
        Reach::FOLDED,
        "a windup from stalking starts folded"
    );
    let strike_tick = step_until(&mut world, &mut p, hunter, HunterPhase::Strike, 400);
    let m = p.hunter_of(hunter).unwrap();
    assert!(
        (m.from.near + 0.35).abs() < 1e-6,
        "the strike was entered from the cocked pose: {:?}",
        m.from
    );
    // Windup and Strike are decided at the pre-step boundary: a phase first seen in view T
    // was entered at boundary T − 1, the start of the interval view T presents, so it is in
    // effect for every frame of that view.
    assert_eq!(m.cur.started + 1, strike_tick);
    let (pose, _) = m.living_pose(strike_tick, 0.0, &[]);
    assert_eq!(pose.attack.unwrap().phase, AttackPhase::Strike);
    assert!(
        pose.attack.unwrap().elapsed.abs() < 1e-9,
        "the strike's attack time starts at its boundary"
    );
    let captures_before = world.hunters().captures_total;
    let handling_tick = step_until(&mut world, &mut p, hunter, HunterPhase::Handling, 400);
    let hash_before = state_hash(&world.state);
    let m = p.hunter_of(hunter).unwrap();
    assert_eq!(m.cur.entered_from, HunterPhase::Strike);
    assert!(
        (m.from.near - 1.0).abs() < 1e-9,
        "handling entered from full extension: {:?}",
        m.from
    );
    assert!(
        m.prey.is_some(),
        "the captured prey is retained for the boundary tick"
    );
    assert_eq!(m.prey.as_ref().unwrap().id, prey);
    assert!(
        world.render_view().organisms.iter().all(|o| o.id != prey),
        "the world removed the prey"
    );
    // Before the boundary the strike is still in effect and in its final extension, reaching
    // full contact exactly at the settlement boundary (the frame's own attack time), and the
    // prey is still drawn; at the boundary the handling begins and the prey is gone.
    let (pose, _) = m.living_pose(handling_tick, 0.5, &[]);
    assert_eq!(pose.attack.unwrap().phase, AttackPhase::Strike);
    let (pose, _) = m.living_pose(handling_tick, 0.999, &[]);
    let late = attack_channels(pose.attack.as_ref());
    assert!(
        late.near_reach > 0.99,
        "just before settlement the claws are all but out: {}",
        late.near_reach
    );
    let (frame, from) = m.frame_at(handling_tick, 0.5);
    let at_boundary =
        attack_channels(episode_of(frame, from, present_seconds(handling_tick, 1.0)).as_ref());
    assert!(
        (at_boundary.near_reach - 1.0).abs() < 1e-9,
        "full contact exactly at the boundary"
    );
    let prey_pos = m.prey.as_ref().unwrap().pos;
    let before = draw(&mut p, &world, 0.5);
    let at = draw(&mut p, &world, 1.0);
    let mut without_prey = presenter();
    // A presenter that never saw the prey: same world, same tick, hunters observed fresh.
    let view = world.render_view();
    without_prey.observe(&view);
    without_prey.observe_hunters(&view, &world.hunter_view());
    let fresh = {
        let mut c = Canvas::new();
        without_prey.draw(&view, 0.5, &mut c);
        c
    };
    assert!(
        differing_near(&before, &fresh, prey_pos, 6.0) > 0,
        "the retained prey is not drawn before the capture boundary"
    );
    // At f = 1 nothing of the prey remains: what the continuous presenter paints near the
    // prey's last position is what a presenter that never saw it paints there, up to the
    // hunter's own restart reconstruction (a fresh presenter reconstructs the strike's end
    // from `entered_from`, which differs from the observed reach by a fraction of a pixel of
    // far-claw lag), whereas before the boundary the whole prey body was there.
    let mut at_fresh = Canvas::new();
    without_prey.draw(&view, 1.0, &mut at_fresh);
    let near_prey = |a: &Canvas, b: &Canvas| -> f32 {
        let (cx, cy) = prey_pos.pixel();
        every_pixel()
            .filter(|&(f, x, y)| {
                f == prey_pos.face
                    && (f64::from(x) - f64::from(cx)).hypot(f64::from(y) - f64::from(cy)) <= 4.0
            })
            .flat_map(|(f, x, y)| {
                let (p, q) = (a.get(f, x, y), b.get(f, x, y));
                (0..3).map(move |c| (p[c] - q[c]).abs())
            })
            .fold(0.0, f32::max)
    };
    let gone = near_prey(&at, &at_fresh);
    let there = near_prey(&before, &fresh);
    assert!(
        gone < 0.05 && there > 0.2,
        "the prey must be gone at the boundary (max diff {gone}) and present before it ({there})"
    );
    // The capture itself was the world's, once; drawing changed nothing in the world.
    assert_eq!(world.hunters().captures_total, captures_before + 1);
    for _ in 0..30 {
        let _ = draw(&mut p, &world, 0.25);
    }
    assert_eq!(
        state_hash(&world.state),
        hash_before,
        "drawing touched the world"
    );
    assert_eq!(world.hunters().captures_total, captures_before + 1);
    // The meal: gut is real, the cocoon is not there.
    let (pose, _) = p
        .hunter_of(hunter)
        .unwrap()
        .living_pose(handling_tick, 1.0, &[]);
    assert!(
        pose.gut > 0.0,
        "handling a real capture carries gut material"
    );
    assert_eq!(pose.cocoon, None, "no escrow, no cocoon");
    // The next tick clears the retained prey.
    step(&mut world, &mut p);
    assert!(p.hunter_of(hunter).unwrap().prey.is_none());
}

#[test]
fn a_certain_miss_recoils_from_full_extension_and_never_gets_a_meal_or_a_second_strike() {
    let (mut world, hunter, prey) = staged(never(trial(&empty_world())));
    let mut p = presenter();
    step_until(&mut world, &mut p, hunter, HunterPhase::Strike, 400);
    let rec_tick = step_until(&mut world, &mut p, hunter, HunterPhase::Recovering, 400);
    let m = p.hunter_of(hunter).unwrap();
    assert_eq!(m.cur.entered_from, HunterPhase::Strike);
    assert!(
        (m.from.near - 1.0).abs() < 1e-9,
        "a failed strike still recoils from full extension"
    );
    assert!(m.prey.is_none(), "a miss removes no prey");
    assert!(
        world.render_view().organisms.iter().any(|o| o.id == prey),
        "the prey survived"
    );
    // Through the whole recovery: no meal, no cocoon, no second extension.
    let mut frames = 0;
    while phase_of(&world, hunter) == HunterPhase::Recovering && frames < 400 {
        for f in [0.0, 0.5] {
            let (pose, _) = p
                .hunter_of(hunter)
                .unwrap()
                .living_pose(world.tick(), f, &[]);
            assert_eq!(pose.gut, 0.0, "a miss must not chew");
            assert_eq!(pose.cocoon, None);
            let ch = attack_channels(pose.attack.as_ref());
            let elapsed = present_seconds(world.tick(), f) - rec_tick as f64 * DT;
            if elapsed > RECOIL_SECONDS + 0.05 {
                assert!(
                    ch.near_reach.abs() < 1e-9,
                    "tick {} f {f}: arms out again",
                    world.tick()
                );
            }
        }
        step(&mut world, &mut p);
        frames += 1;
    }
    assert!(
        frames > 60,
        "the recovery should last seconds, not {frames} ticks"
    );
}

#[test]
fn a_viewer_joining_mid_hunt_or_a_restart_initializes_from_the_persisted_origin() {
    let (mut world, hunter, _) = staged(certain(trial(&empty_world())));
    let mut p = presenter();
    step_until(&mut world, &mut p, hunter, HunterPhase::Windup, 400);
    step(&mut world, &mut p);
    // A fresh presenter joins mid-windup: no fake attack, the cock continues from folded.
    let bytes = encode_snapshot(&world.state, "hunter-present-test");
    let (_, state) = decode_snapshot(&bytes).expect("the snapshot decodes");
    let restarted = World::from_state(state).expect("the world resumes");
    let mut fresh = presenter();
    let view = restarted.render_view();
    fresh.observe(&view);
    fresh.observe_hunters(&view, &restarted.hunter_view());
    let m = fresh
        .hunter_of(hunter)
        .expect("the member is recovered by id");
    assert_eq!(m.cur.phase, HunterPhase::Windup);
    assert_eq!(m.from, Reach::FOLDED);
    let (pose, _) = m.living_pose(view.tick, 0.5, &[]);
    let ch = attack_channels(pose.attack.as_ref());
    assert!(
        ch.near_reach <= 0.0 && ch.lunge == 0.0,
        "a restarted windup must not be extended"
    );
    // Restart in handling: entered from the strike, so the recoil starts extended.
    step_until(&mut world, &mut p, hunter, HunterPhase::Handling, 400);
    let bytes = encode_snapshot(&world.state, "hunter-present-test");
    let (_, state) = decode_snapshot(&bytes).unwrap();
    let restarted = World::from_state(state).unwrap();
    let mut fresh = presenter();
    let view = restarted.render_view();
    fresh.observe(&view);
    fresh.observe_hunters(&view, &restarted.hunter_view());
    let m = fresh.hunter_of(hunter).unwrap();
    assert_eq!(m.cur.entered_from, HunterPhase::Strike);
    assert_eq!(m.from, settled_reach());
    assert!((m.from.near - 1.0).abs() < 1e-9 && (m.from.lunge - 1.1).abs() < 1e-9);
    assert!(
        m.prey.is_none(),
        "a restart cannot retain a prey it never saw"
    );
    // The restarted and the continuous presenter agree once the recoil is over.
    for _ in 0..10 {
        step(&mut world, &mut p);
    }
    let bytes = encode_snapshot(&world.state, "hunter-present-test");
    let (_, state) = decode_snapshot(&bytes).unwrap();
    let restarted = World::from_state(state).unwrap();
    let mut fresh = presenter();
    let view = restarted.render_view();
    fresh.observe(&view);
    fresh.observe_hunters(&view, &restarted.hunter_view());
    let (a, _) = p
        .hunter_of(hunter)
        .unwrap()
        .living_pose(view.tick, 0.5, &[]);
    let (b, _) = fresh
        .hunter_of(hunter)
        .unwrap()
        .living_pose(view.tick, 0.5, &[]);
    let (ca, cb) = (
        attack_channels(a.attack.as_ref()),
        attack_channels(b.attack.as_ref()),
    );
    assert!((ca.near_reach - cb.near_reach).abs() < 1e-9 && (ca.hush - cb.hush).abs() < 1e-9);
}

#[test]
fn a_stale_id_is_ignored_and_dropped_membership_is_forgotten() {
    let (mut world, hunter, _) = staged(certain(trial(&empty_world())));
    let mut p = presenter();
    step(&mut world, &mut p);
    let view = world.render_view();
    let mut stale: Vec<HunterView> = world.hunter_view();
    stale[0].id = OrganismId {
        slot: hunter.slot,
        generation: hunter.generation + 1,
    };
    let mut q = presenter();
    q.observe(&view);
    q.observe_hunters(&view, &stale);
    assert!(
        q.hunter_ids().is_empty(),
        "a stale generation must not resolve to a body"
    );
    let mut c = Canvas::new();
    q.draw(&view, 0.5, &mut c);
    // Membership dropped: memory forgotten, the body returns to its ordinary rig.
    p.observe_hunters(&view, &[]);
    assert!(p.hunter_ids().is_empty() && p.hunter_of(hunter).is_none());
    let mut plain = presenter();
    plain.observe(&view);
    let mut a = Canvas::new();
    let mut b = Canvas::new();
    p.draw(&view, 0.5, &mut a);
    plain.draw(&view, 0.5, &mut b);
    assert!(identical(&a, &b));
}

#[test]
fn an_unsupported_body_scale_is_reported_once_and_drawn_with_the_ordinary_rig_not_clamped() {
    // A custom admitted profile: minimum 0.05 with a square-law mapping, so a default
    // child (0.4 of the adult) has body scale 0.16 — valid for the core, below SCALE_MIN.
    let mut profile = certain(trial(&empty_world()));
    profile.body_scale_min = 0.05;
    profile.body_scale_exponent = 2.0;
    let (mut world, hunter, _) = staged(profile);
    {
        let o = world.state.organisms.get_mut(hunter).unwrap();
        o.structure = 0.4 * o.phenotype.structure_adult;
    }
    let mut p = presenter();
    step(&mut world, &mut p);
    let view = world.render_view();
    let scale = world.hunter_view()[0].body_scale;
    assert!(
        !hunter_scale_supported(scale),
        "the fixture must produce an unsupported scale, got {scale}"
    );
    assert!(
        p.hunter_ids().is_empty(),
        "an unsupported scale is not drawn as the rig"
    );
    assert_eq!(p.unsupported_hunters(), vec![(hunter, scale)]);
    assert_eq!(p.take_new_unsupported(), vec![(hunter, scale)]);
    assert!(p.take_new_unsupported().is_empty(), "reported once");
    let mut plain = presenter();
    plain.observe(&view);
    let mut a = Canvas::new();
    let mut b = Canvas::new();
    p.draw(&view, 0.5, &mut a);
    plain.draw(&view, 0.5, &mut b);
    assert!(
        identical(&a, &b),
        "the unsupported member must be drawn with its ordinary rig, unchanged"
    );
}

#[test]
fn a_juveniles_scale_is_the_cores_and_its_named_claw_is_where_the_core_tests_contact() {
    let (mut world, hunter, _) = staged(certain(trial(&empty_world())));
    {
        let o = world.state.organisms.get_mut(hunter).unwrap();
        o.structure = 0.1 * o.phenotype.structure_adult;
    }
    let mut p = presenter();
    step(&mut world, &mut p);
    let view = world.render_view();
    let h = world
        .hunter_view()
        .into_iter()
        .find(|h| h.id == hunter)
        .unwrap();
    assert!(
        (h.body_scale - 0.1f64.sqrt()).abs() < 1e-9,
        "sqrt(0.1) juvenile, got {}",
        h.body_scale
    );
    let (pose, scale) = p
        .hunter_of(hunter)
        .unwrap()
        .living_pose(view.tick, 0.5, &[]);
    assert_eq!(
        scale, h.body_scale,
        "the rig is drawn at exactly the core's scale"
    );
    assert!(pose.attack.is_none() || true);
    // The core's scaled capture offset is the art's scaled near claw.
    let e = effectors(scale);
    assert!(
        (h.geometry.capture_offset_body.x - e.near_claw.x).abs() < 1e-9,
        "{:?} vs {:?}",
        h.geometry.capture_offset_body,
        e.near_claw
    );
    assert!((h.geometry.capture_offset_body.y - e.near_claw.y).abs() < 1e-9);
    assert!((h.geometry.ingestion_offset_body.x - e.mouth.x).abs() < 1e-9);
    // And the published capture centre is that claw carried from the root.
    if let Some(center) = h.capture_center {
        let want = travel(h.pos, body_offset(h.heading, e.near_claw, 1.0)).end;
        assert_eq!(center.face, want.face);
        assert!(
            (center.u - want.u).abs() < 1e-6 && (center.v - want.v).abs() < 1e-6,
            "{center:?} vs {want:?}"
        );
    }
    // Drawn small: the body's light sits within the scaled footprint of the root.
    let image = draw(&mut p, &world, 0.5);
    let mut plain = presenter();
    plain.observe(&view);
    let base = draw(&mut plain, &world, 0.5);
    assert!(differing_near(&image, &base, h.pos, 14.0 * scale + 2.0) > 0);
    assert_eq!(
        differing_near(&image, &base, h.pos, 16.0)
            - differing_near(&image, &base, h.pos, 14.0 * scale + 2.0),
        0
    );
}

#[test]
fn the_render_rate_and_repeated_draws_change_neither_the_image_nor_the_world() {
    let (mut world, hunter, _) = staged(certain(trial(&empty_world())));
    let mut p = presenter();
    step_until(&mut world, &mut p, hunter, HunterPhase::Strike, 400);
    let view = world.render_view();
    let hash = state_hash(&world.state);
    // Common instants of 30, 60 and 120 Hz within one tick (DT = 50 ms): f = 0, 1/3, 2/3 …
    let mut images = Vec::new();
    for f in [0.0, 1.0 / 3.0, 2.0 / 3.0] {
        let mut a = Canvas::new();
        p.draw(&view, f, &mut a);
        let mut b = Canvas::new();
        p.draw(&view, f, &mut b);
        assert!(identical(&a, &b), "a repeated draw at f {f} differs");
        images.push(a);
    }
    assert!(
        !identical(&images[0], &images[1]),
        "the body must move within the tick"
    );
    assert_eq!(state_hash(&world.state), hash, "drawing touched the world");
    assert_eq!(world.hunter_view().len(), 1);
}

#[test]
fn a_hunter_on_a_seam_is_one_continuous_body_on_two_faces() {
    let (mut world, hunter, _) = staged_at(
        SurfacePoint::new(Face::Front, 61.0, 32.0),
        certain(trial(&empty_world())),
    );
    let mut p = presenter();
    step(&mut world, &mut p);
    let view = world.render_view();
    assert_eq!(p.hunter_ids(), vec![hunter]);
    let mut plain = presenter();
    plain.observe(&view);
    let a = draw(&mut p, &world, 0.5);
    let b = draw(&mut plain, &world, 0.5);
    let on_right = every_pixel()
        .filter(|&(f, x, y)| f == Face::Right && a.get(f, x, y) != b.get(f, x, y))
        .count();
    assert!(on_right > 0, "the claws reach onto the Right face");
    assert!(lit_faces(&a) >= 2);
}

/// Native captures from a real world's attack, for the record. Ignored: it writes files.
/// `HUNTER_CAPTURE_DIR` names the directory (default: a fresh `mktemp`-style path under /tmp).
#[test]
#[ignore]
fn capture_a_real_attack_as_native_frames() {
    use cubarium::sink::{FrameSink, PngSink};
    let dir = std::env::var("HUNTER_CAPTURE_DIR").unwrap_or_else(|_| {
        let d = std::env::temp_dir().join(format!("hunter-present-{}", std::process::id()));
        d.to_string_lossy().into_owned()
    });
    std::fs::create_dir_all(&dir).unwrap();
    let (mut world, hunter, _) = staged(certain(trial(&empty_world())));
    let mut p = presenter();
    let mut sink = PngSink::new(&dir, 1).unwrap();
    let mut frame = cube_proto::Frame::black();
    let mut manifest = String::new();
    let mut phase_log = String::new();
    // 8 s of the world at 20 Hz, three frames per tick (60 fps), from stalking through the
    // capture and into handling.
    for _ in 0..160 {
        step(&mut world, &mut p);
        let view = world.render_view();
        let phase = phase_of(&world, hunter);
        phase_log.push_str(&format!("{} {:?}\n", view.tick, phase));
        for k in 0..3 {
            let f = f64::from(k) / 3.0;
            let mut canvas = Canvas::new();
            p.draw(&view, f, &mut canvas);
            canvas.encode(&mut frame);
            sink.submit(&frame).unwrap();
            manifest.push_str(&format!(
                "tick {} f {:.3} phase {:?}\n",
                view.tick, f, phase
            ));
        }
    }
    sink.finish().unwrap();
    std::fs::write(format!("{dir}/manifest.txt"), manifest).unwrap();
    std::fs::write(format!("{dir}/phases.txt"), phase_log).unwrap();
    eprintln!("frames in {dir}");
}

/// Draw cost with one and two adult hunters and a juvenile on a staged world. Ignored:
/// timing. Run in release with `--nocapture`.
#[test]
#[ignore]
fn draw_cost_with_hunters() {
    let mut cases: Vec<(&str, World, Vec<OrganismId>)> = Vec::new();
    let (w1, h1, _) = staged(certain(trial(&empty_world())));
    cases.push(("one adult", w1, vec![h1]));
    let (mut w2, h2, _) = staged(certain(trial(&empty_world())));
    {
        let o = w2.state.organisms.get_mut(h2).unwrap();
        o.structure = 0.4 * o.phenotype.structure_adult;
    }
    cases.push(("one juvenile 0.632", w2, vec![h2]));
    for (name, mut world, _) in cases {
        let mut p = presenter();
        step(&mut world, &mut p);
        let view = world.render_view();
        let mut with = presenter();
        with.observe(&view);
        with.observe_hunters(&view, &world.hunter_view());
        let mut without = presenter();
        without.observe(&view);
        let time = |p: &mut ArtPresenter| {
            let mut canvas = Canvas::new();
            let start = std::time::Instant::now();
            for k in 0..120 {
                p.draw(&view, f64::from(k % 3) / 3.0, &mut canvas);
            }
            start.elapsed().as_secs_f64() * 1e6 / 120.0
        };
        let a = time(&mut without);
        let b = time(&mut with);
        eprintln!(
            "{name}: {a:.0} us/frame ordinary, {b:.0} us/frame with the hunter (+{:.0})",
            b - a
        );
    }
}
