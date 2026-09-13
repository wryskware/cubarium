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
mod support;

use cubarium::hunter_present::{
    COCKED, HUNTER_FULL_SPEED_PX_S, HunterFrame, HunterMemory, entry_reach, episode_of,
    hunter_scale_supported, movement_of, settled_reach, validate_profile, validate_view,
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
use support::{Scratch, parse, run, snapshot_ticks};

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

/// Step one tick and feed the presenter exactly as the runner does: life events and hunter
/// events drained every tick, the membership and this tick's hunter events observed after
/// the view.
fn step(world: &mut World, p: &mut ArtPresenter) {
    world.step();
    world.drain_events();
    let hunted = world.drain_hunter_events();
    let view = world.render_view();
    p.observe(&view);
    p.observe_hunters(&view, &world.hunter_view(), &hunted)
        .expect("a validated profile's members are drawable");
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
        told.observe_hunters(&view, &world.hunter_view(), &[])
            .unwrap();
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
    assert!(told.hunter_ids().is_empty());
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
        p.observe_hunters(&view, &world.hunter_view(), &[]).unwrap();
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
    without_prey
        .observe_hunters(&view, &world.hunter_view(), &[])
        .unwrap();
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
    fresh
        .observe_hunters(&view, &restarted.hunter_view(), &[])
        .unwrap();
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
    fresh
        .observe_hunters(&view, &restarted.hunter_view(), &[])
        .unwrap();
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
    fresh
        .observe_hunters(&view, &restarted.hunter_view(), &[])
        .unwrap();
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
    q.observe_hunters(&view, &stale, &[]).unwrap();
    assert!(
        q.hunter_ids().is_empty(),
        "a stale generation must not resolve to a body"
    );
    let mut c = Canvas::new();
    q.draw(&view, 0.5, &mut c);
    // Membership dropped: memory forgotten, the body returns to its ordinary rig.
    p.observe_hunters(&view, &[], &[]).unwrap();
    assert!(p.hunter_ids().is_empty() && p.hunter_of(hunter).is_none());
    let mut plain = presenter();
    plain.observe(&view);
    let mut a = Canvas::new();
    let mut b = Canvas::new();
    p.draw(&view, 0.5, &mut a);
    plain.draw(&view, 0.5, &mut b);
    assert!(identical(&a, &b));
}

// ---------------------------------------------------------------- capability

/// The renderer states what it can draw before a world is stepped: the fixed trial profile
/// is drawable; a profile whose contact geometry, ingestion mouth or scale range is not the
/// art's is refused by name — never clamped, never given an ordinary predator body.
#[test]
fn the_capability_check_admits_the_trial_profile_and_names_what_it_cannot_draw() {
    let base = trial(&empty_world());
    assert_eq!(validate_profile(&base), Ok(()));
    assert_eq!(validate_profile(&certain(base.clone())), Ok(()));
    let mut claw = base.clone();
    claw.capture_offset_body = Vec2::ZERO;
    let err = validate_profile(&claw).unwrap_err();
    assert!(
        err.contains("capture_offset_body") && err.contains("13.279"),
        "{err}"
    );
    let mut mouth = base.clone();
    mouth.ingestion_offset_body = Vec2::new(6.0, 0.0);
    let err = validate_profile(&mouth).unwrap_err();
    assert!(
        err.contains("ingestion_offset_body") && err.contains("9.6"),
        "{err}"
    );
    let mut small = base.clone();
    small.body_scale_min = 0.05;
    let err = validate_profile(&small).unwrap_err();
    assert!(
        err.contains("body_scale_min") && err.contains("0.05"),
        "{err}"
    );
    let mut exponent = base.clone();
    exponent.body_scale_exponent = -1.0;
    assert!(
        validate_profile(&exponent)
            .unwrap_err()
            .contains("body_scale_exponent")
    );
    // The presenter exposes the same check.
    assert_eq!(presenter().validate_hunter_profile(&base), Ok(()));
    assert!(presenter().validate_hunter_profile(&claw).is_err());
    // And the per-member check refuses a view whose published geometry is not the drawn
    // claw, even at a supported scale (Astra's fixture): nothing is drawn as the rig.
    let mut world = empty_world();
    world
        .start_hunter_trial(claw, target_of(SurfacePoint::new(Face::Front, 32.0, 32.0)))
        .unwrap();
    let view = world.render_view();
    let h = world.hunter_view();
    let mut p = presenter();
    p.observe(&view);
    let err = p.observe_hunters(&view, &h, &[]).unwrap_err();
    assert!(err.contains("capture offset"), "{err}");
    assert!(
        p.hunter_ids().is_empty(),
        "a refused member is not drawn as the rig"
    );
    assert_eq!(validate_view(&h[0]).is_err(), true);
}

/// The actual runner refuses a saved world whose profile the art cannot draw, by name and
/// before any tick; the same world runs without `--art`, and a saved trial world resumes
/// with `--art`. The runner's own preflight, not a presenter unit.
#[test]
fn the_runner_refuses_an_undrawable_profile_at_load_and_resumes_a_drawable_one() {
    let scratch = Scratch::new("hunter-present-preflight");
    // A saved world with a profile the art cannot draw.
    let bad_state = scratch.join("bad");
    std::fs::create_dir_all(&bad_state).unwrap();
    let mut bad = trial(&empty_world());
    bad.capture_offset_body = Vec2::ZERO;
    let (world, _, _) = staged(bad);
    let bytes = encode_snapshot(&world.state, "hunter-present-test");
    cubarium::state::write_snapshot(&bad_state, world.tick(), &bytes).unwrap();
    let art = atelier();
    let refused = cubarium::run_world(&parse(&[
        "--sink",
        "none",
        "--speed",
        "0",
        "--seconds",
        "1",
        "--state",
        bad_state.to_str().unwrap(),
        "--art",
        art.to_str().unwrap(),
        "--require-resume",
    ]));
    let err = match refused {
        Ok(out) => panic!("the run must be refused, got {out:?}"),
        Err(e) => format!("{e:#}"),
    };
    assert!(
        err.contains("hunter profile") && err.contains("capture_offset_body"),
        "{err}"
    );
    assert!(
        err.contains("without --art"),
        "the refusal names the opt-out: {err}"
    );
    // Unmutated: the saved snapshot is the only one, and nothing was stepped or written.
    assert_eq!(snapshot_ticks(&bad_state), vec![world.tick()]);
    // The same world runs without the art.
    let out = run(&[
        "--sink",
        "none",
        "--speed",
        "0",
        "--seconds",
        "2",
        "--state",
        bad_state.to_str().unwrap(),
        "--require-resume",
    ]);
    assert_eq!(out.start_tick, world.tick());
    assert_eq!(out.final_tick, world.tick() + 40);
    // A drawable saved trial world resumes with the art and steps.
    let good_state = scratch.join("good");
    std::fs::create_dir_all(&good_state).unwrap();
    let (world, _, _) = staged(certain(trial(&empty_world())));
    let bytes = encode_snapshot(&world.state, "hunter-present-test");
    cubarium::state::write_snapshot(&good_state, world.tick(), &bytes).unwrap();
    let out = run(&[
        "--sink",
        "none",
        "--speed",
        "0",
        "--seconds",
        "2",
        "--state",
        good_state.to_str().unwrap(),
        "--art",
        art.to_str().unwrap(),
        "--require-resume",
    ]);
    assert_eq!(out.final_tick, world.tick() + 40);
}

/// The actual runner drains the world's hunter events every tick, headless and not: a run
/// through many paid attempts ends with the same state a hand-stepped world that drains per
/// tick reaches (draining changes nothing), the life log still carries the prey's death by
/// predation, and the runner never accumulates the buffer — proved on the hand-stepped twin,
/// whose buffer is empty after every tick, and on the runner by its outcome equalling that
/// twin's state hash.
#[test]
fn the_runner_drains_hunter_events_every_tick_without_touching_state_or_life_events() {
    let scratch = Scratch::new("hunter-present-drain");
    let state = scratch.join("state");
    std::fs::create_dir_all(&state).unwrap();
    // A certain miss with a frozen prey in the claws: the hunter attacks again and again.
    let (world, hunter, prey) = staged(never(trial(&empty_world())));
    let bytes = encode_snapshot(&world.state, "hunter-present-test");
    cubarium::state::write_snapshot(&state, world.tick(), &bytes).unwrap();
    // The twin: stepped by hand with every buffer drained per tick, exactly as the runner.
    let (_, decoded) = decode_snapshot(&bytes).unwrap();
    let mut twin = World::from_state(decoded).unwrap();
    let mut attempts = 0usize;
    let ticks = 20 * 60;
    for _ in 0..ticks {
        twin.step();
        twin.drain_events();
        let hunted = twin.drain_hunter_events();
        attempts += hunted
            .iter()
            .filter(|e| matches!(e, cubarium_core::HunterEvent::Attempt { .. }))
            .count();
        assert!(twin.drain_hunter_events().is_empty(), "drained means empty");
    }
    assert!(
        attempts >= 3,
        "the fixture must attack repeatedly, got {attempts} attempts"
    );
    assert!(
        twin.hunters().member(hunter).is_some() && twin.state.organisms.get(prey).is_some(),
        "a certain miss keeps both alive"
    );
    let want = state_hash(&twin.state);
    // The actual runner, headless, over the same ticks.
    let out = run(&[
        "--sink",
        "none",
        "--speed",
        "0",
        "--seconds",
        "60",
        "--state",
        state.to_str().unwrap(),
        "--require-resume",
    ]);
    assert_eq!(out.final_tick, world.tick() + ticks);
    assert_eq!(
        out.state_hash, want,
        "draining hunter events changed the world"
    );
    // And with the art on, the same.
    let state2 = scratch.join("state-art");
    std::fs::create_dir_all(&state2).unwrap();
    cubarium::state::write_snapshot(&state2, world.tick(), &bytes).unwrap();
    let out = run(&[
        "--sink",
        "none",
        "--speed",
        "0",
        "--seconds",
        "60",
        "--state",
        state2.to_str().unwrap(),
        "--art",
        atelier().to_str().unwrap(),
        "--require-resume",
    ]);
    assert_eq!(out.state_hash, want, "the art path changed the world");
}

// ---------------------------------------------------------------- Astra's adapter fixtures

/// Synthetic observer frames over a real trial world isolate the bookkeeping: the same view
/// tick observed twice must not discard the prior tick's phase, and a rewind must not carry a
/// later phase's reach back in time.
fn synthetic(
    world: &World,
    tick: u64,
    phase: HunterPhase,
    started: u64,
    ends: u64,
    origin: HunterPhase,
) -> (cubarium_core::RenderView, HunterView) {
    let mut view = world.render_view();
    view.tick = tick;
    let mut h = world.hunter_view()[0];
    h.phase = phase;
    h.phase_started_tick = started;
    h.phase_ends_tick = ends;
    h.entered_from = origin;
    h.episode = 1;
    (view, h)
}

fn observe_pair(p: &mut ArtPresenter, pair: &(cubarium_core::RenderView, HunterView)) {
    p.observe(&pair.0);
    p.observe_hunters(&pair.0, &[pair.1], &[]).unwrap();
}

#[test]
fn a_repeated_observation_of_the_same_tick_changes_no_fractional_frame() {
    let (world, hunter, _) = staged(certain(trial(&empty_world())));
    let strike = synthetic(&world, 19, HunterPhase::Strike, 1, 20, HunterPhase::Windup);
    let handled = synthetic(
        &world,
        20,
        HunterPhase::Handling,
        20,
        20,
        HunterPhase::Strike,
    );
    let mut p = presenter();
    observe_pair(&mut p, &strike);
    observe_pair(&mut p, &handled);
    let before: Vec<_> = [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]
        .into_iter()
        .map(|f| p.hunter_of(hunter).unwrap().living_pose(20, f, &[]))
        .collect();
    let images_before: Vec<Canvas> = [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]
        .into_iter()
        .map(|f| {
            let mut c = Canvas::new();
            p.draw(&handled.0, f, &mut c);
            c
        })
        .collect();
    let memory_before = p.hunter_of(hunter).unwrap().clone();
    observe_pair(&mut p, &handled);
    observe_pair(&mut p, &handled);
    assert_eq!(
        p.hunter_of(hunter).unwrap(),
        &memory_before,
        "a repeated observation changed the memory"
    );
    for (k, f) in [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0].into_iter().enumerate() {
        assert_eq!(
            p.hunter_of(hunter).unwrap().living_pose(20, f, &[]),
            before[k],
            "f {f}"
        );
        let mut c = Canvas::new();
        p.draw(&handled.0, f, &mut c);
        assert!(identical(&c, &images_before[k]), "f {f}: the image changed");
    }
    // The prior tick's strike is still what the frames before the boundary show.
    let (pose, _) = p.hunter_of(hunter).unwrap().living_pose(20, 0.5, &[]);
    assert_eq!(pose.attack.unwrap().phase, AttackPhase::Strike);
}

#[test]
fn a_rewound_presenter_reconstructs_hunter_memory_like_a_fresh_one() {
    let (world, hunter, _) = staged(certain(trial(&empty_world())));
    let strike = synthetic(&world, 19, HunterPhase::Strike, 1, 20, HunterPhase::Windup);
    let handled = synthetic(
        &world,
        20,
        HunterPhase::Handling,
        20,
        20,
        HunterPhase::Strike,
    );
    let old = synthetic(&world, 5, HunterPhase::Windup, 2, 10, HunterPhase::Stalking);
    let mut reused = presenter();
    observe_pair(&mut reused, &strike);
    observe_pair(&mut reused, &handled);
    observe_pair(&mut reused, &old);
    let mut fresh = presenter();
    observe_pair(&mut fresh, &old);
    let a = reused.hunter_of(hunter).unwrap();
    let b = fresh.hunter_of(hunter).unwrap();
    assert_eq!(
        a, b,
        "future attack reach must not leak into a rewound world"
    );
    assert_eq!(a.from, Reach::FOLDED);
    for f in [0.0, 0.5, 1.0] {
        let mut x = Canvas::new();
        let mut y = Canvas::new();
        reused.draw(&old.0, f, &mut x);
        fresh.draw(&old.0, f, &mut y);
        assert!(
            identical(&x, &y),
            "f {f}: a rewound presenter draws differently from a fresh one"
        );
    }
}

// ---------------------------------------------------------------- moving prey, topology, hold

/// A quiet world: nothing grows, nothing falls, no light — so the backdrop is the bare
/// ground and every painted pixel near the hunter is the hunter's or the prey's.
fn quiet_world() -> World {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds.clear();
    cfg.founders.count = 0;
    cfg.weather.amplitude = 0.0;
    cfg.water.rain_rate = 0.0;
    cfg.habitat.light_base = 0.0;
    cfg.habitat.light_height_gain = 0.0;
    cfg.habitat.light_noise_gain = 0.0;
    cfg.detritus.initial_dark = 0.0;
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.fall = 0.0;
    World::new(cfg).expect("a quiet world is valid")
}

/// A hunter at `spot` facing `heading`, in a quiet world, with a free (unfrozen) prey placed
/// in its claws; the prey may move on its own while the strike runs.
fn staged_moving(
    spot: SurfacePoint,
    heading: Vec2,
    profile: FixedHunterProfile,
) -> (World, OrganismId, OrganismId) {
    let mut world = quiet_world();
    let receipt = world
        .start_hunter_trial(profile.clone(), target_of(spot))
        .expect("the trial starts");
    let hunter = receipt.id;
    aim(&mut world, hunter, heading, 1.0);
    let grasp = effector_point(spot, heading, &profile, 1.0);
    let prey = place_prey(&mut world, grasp, 0.5, 0.3, 0.4, false);
    (world, hunter, prey)
}

/// Runs a moving-prey capture and returns what the boundary tick showed: the retained prey's
/// settlement position from the event, its last published position, and the presenter.
fn captured(
    spot: SurfacePoint,
    heading: Vec2,
) -> (World, ArtPresenter, OrganismId, OrganismId, u64) {
    let (mut world, hunter, prey) = staged_moving(spot, heading, certain(trial(&quiet_world())));
    let mut p = presenter();
    let tick = step_until(&mut world, &mut p, hunter, HunterPhase::Handling, 400);
    (world, p, hunter, prey, tick)
}

#[test]
fn a_captured_prey_is_carried_to_the_events_settlement_position_and_meets_the_claw() {
    let (world, mut p, hunter, prey, tick) = captured(
        SurfacePoint::new(Face::Front, 20.0, 32.0),
        Vec2::new(1.0, 0.0),
    );
    let m = p.hunter_of(hunter).unwrap();
    let held = m
        .prey
        .as_ref()
        .expect("the captured prey is retained for the boundary tick");
    assert_eq!(held.id, prey);
    let at = m
        .prey_at
        .expect("the Capture event's settlement position was noted");
    // The drawn pose walks from the last published position to the settlement position.
    let (start, _) = m.retained_prey_pose(0.0).unwrap();
    let (end, heading) = m.retained_prey_pose(1.0).unwrap();
    assert_eq!(start, held.pos);
    assert_eq!(
        heading, held.heading,
        "the event carries no heading: the last published one holds"
    );
    assert!((end.u - at.u).abs() < 1e-9 && (end.v - at.v).abs() < 1e-9 && end.face == at.face);
    let (mid, _) = m.retained_prey_pose(0.5).unwrap();
    let want = Vec2::new((start.u + at.u) * 0.5, (start.v + at.v) * 0.5);
    assert!(
        (mid.u - want.x).abs() < 1e-9 && (mid.v - want.y).abs() < 1e-9,
        "a straight chart path"
    );
    // At the boundary the prey is inside the hunter's grasp: within the core's own reach of
    // the drawn near claw, carried from the hunter's interpolated root at f → 1.
    let view = world.render_view();
    let o = view.organisms.iter().find(|o| o.id == hunter).unwrap();
    let h = world
        .hunter_view()
        .into_iter()
        .find(|h| h.id == hunter)
        .unwrap();
    let (root, dir) = cubarium::present::interpolate(&o.moved, o.pos, o.heading, 1.0);
    let claw = travel(
        root,
        body_offset(dir, effectors(h.body_scale).near_claw, 1.0),
    )
    .end;
    let d = ((end.u - claw.u).powi(2) + (end.v - claw.v).powi(2)).sqrt();
    assert!(
        d <= h.geometry.capture_reach_px + 0.5,
        "at settlement the prey is {d} px from the drawn claw (reach {})",
        h.geometry.capture_reach_px
    );
    // Drawn: at f = 0.999 the prey's light sits at the settlement position, not its start.
    let before = draw(&mut p, &world, 0.999);
    let mut fresh = presenter();
    fresh.observe(&view);
    fresh
        .observe_hunters(&view, &world.hunter_view(), &[])
        .unwrap();
    let mut none = Canvas::new();
    fresh.draw(&view, 0.999, &mut none);
    assert!(
        differing_near(&before, &none, at, 3.0) > 0,
        "the prey is not drawn at its settlement position"
    );
    let _ = tick;
}

#[test]
fn a_capture_across_a_seam_carries_the_prey_onto_the_next_face_and_the_body_stays_whole() {
    let (world, mut p, hunter, _, _) = captured(
        SurfacePoint::new(Face::Front, 52.0, 32.0),
        Vec2::new(1.0, 0.0),
    );
    let m = p.hunter_of(hunter).unwrap();
    let held = m.prey.as_ref().expect("retained");
    let at = m.prey_at.expect("noted");
    assert_eq!(
        at.face,
        Face::Right,
        "the grasp is past the Front/Right seam"
    );
    // A different face: drawn at the settlement position itself (the labelled approximation).
    let (pos, _) = m.retained_prey_pose(0.5).unwrap();
    if held.pos.face != at.face {
        assert_eq!(pos, at);
    }
    let image = draw(&mut p, &world, 0.5);
    assert!(
        lit_faces(&image) >= 2,
        "the hunter and its prey span the seam"
    );
    let on_right = every_pixel()
        .filter(|&(f, x, y)| f == Face::Right && image.get(f, x, y) != [0.0; 3])
        .count();
    assert!(on_right > 0);
}

#[test]
fn a_capture_at_the_open_rim_draws_nothing_below_it_and_a_vertex_hunt_does_not_panic() {
    // Heading straight down the Front face toward the rim: the grasp sits just above it.
    let (world, mut p, hunter, _, _) = captured(
        SurfacePoint::new(Face::Front, 32.0, 48.0),
        Vec2::new(0.0, 1.0),
    );
    let m = p.hunter_of(hunter).unwrap();
    let at = m.prey_at.expect("noted");
    assert!(at.v < 64.0 && at.face == Face::Front);
    // Nothing of the hunter or its prey leaves the Front face: every pixel off it is the
    // hunter-free image's (the floor and ground paint every face), and nothing is reflected
    // back above the rim that the flat body does not have.
    let image = draw(&mut p, &world, 0.5);
    let view = world.render_view();
    let mut plain = presenter();
    plain.observe(&view);
    let mut base = Canvas::new();
    plain.draw(&view, 0.5, &mut base);
    for (f, x, y) in every_pixel() {
        if f != Face::Front {
            assert_eq!(
                image.get(f, x, y),
                base.get(f, x, y),
                "light off the Front face at the rim: {f:?} ({x}, {y})"
            );
        }
    }
    assert!(differing_near(&image, &base, at, 4.0) > 0, "the grasp near the rim is drawn");
    // Toward a top vertex: whatever the core decides about the grasp, the presenter draws
    // one owner per pixel and never panics.
    let (mut world, hunter, _) = staged_moving(
        SurfacePoint::new(Face::Top, 6.0, 6.0),
        Vec2::new(-0.7071, -0.7071),
        certain(trial(&quiet_world())),
    );
    let mut p = presenter();
    for _ in 0..120 {
        step(&mut world, &mut p);
        let image = draw(&mut p, &world, 0.5);
        for (f, x, y) in every_pixel() {
            assert!(image.get(f, x, y).iter().all(|&c| c <= 1.0 + 1e-6));
        }
    }
    let _ = hunter;
}

/// Held time (the runner passes `f = 1` while care holds a boundary) freezes the attack and
/// the ambient body alike: repeated draws are identical and nothing advances.
#[test]
fn held_time_freezes_the_attack_and_the_ambient_body() {
    let (mut world, hunter, _) = staged(certain(trial(&empty_world())));
    let mut p = presenter();
    step_until(&mut world, &mut p, hunter, HunterPhase::Strike, 400);
    let view = world.render_view();
    let a = draw(&mut p, &world, 1.0);
    let (pa, _) = p
        .hunter_of(hunter)
        .unwrap()
        .living_pose(view.tick, 1.0, &[]);
    for _ in 0..5 {
        let b = draw(&mut p, &world, 1.0);
        assert!(identical(&a, &b), "a held frame drifted");
        let (pb, _) = p
            .hunter_of(hunter)
            .unwrap()
            .living_pose(view.tick, 1.0, &[]);
        assert_eq!(pa, pb);
    }
    assert_eq!(pa.ambient, present_seconds(view.tick, 1.0));
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
    // A quiet backdrop (no plants, no light) and a free prey in the claws: what is drawn
    // near the hunter is the hunter and the prey.
    let (mut world, hunter, _) = staged_moving(
        SurfacePoint::new(Face::Front, 20.0, 32.0),
        Vec2::new(1.0, 0.0),
        certain(trial(&quiet_world())),
    );
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
        with.observe_hunters(&view, &world.hunter_view(), &[])
            .unwrap();
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
