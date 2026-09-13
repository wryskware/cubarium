//! Published-view fixtures isolate the outgoing prey-pose handoff, not capture eligibility.
//! The hunter is intentionally remote so its changing rig cannot hide prey pixel changes.
use cubarium::{art::ArtPack, art_present::ArtPresenter};
use cubarium_core::{FixedHunterProfile, HunterTarget, HunterView, World, WorldConfig};
use cubarium_core::{hunter::HunterPhase, ids::OrganismId, organism::Mode, view::RenderView};
use cubarium_render::Canvas;
use cubarium_surface::{Face, SurfacePoint, Vec2};

fn presenter(enabled: bool) -> ArtPresenter {
    let p = ArtPresenter::new(
        ArtPack::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"),
        )
        .unwrap(),
    );
    if enabled { p } else { p.without_meal_onset() }
}

fn observe(p: &mut ArtPresenter, view: &RenderView, hunter: HunterView) {
    p.observe(view);
    p.observe_hunters(view, &[hunter], &[]).unwrap();
}

fn draw(p: &mut ArtPresenter, v: &RenderView, f: f64) -> Canvas {
    let mut c = Canvas::new();
    p.draw(v, f, &mut c);
    c
}

fn same_prey(a: &Canvas, b: &Canvas) {
    let mut changed = 0;
    let mut maximum = 0.0f32;
    for y in 2..19 {
        for x in 2..19 {
            let u = a.get(Face::Front, x, y);
            let v = b.get(Face::Front, x, y);
            if u != v {
                changed += 1;
            }
            for (u, v) in u.into_iter().zip(v) {
                maximum = maximum.max((u - v).abs());
            }
        }
    }
    assert_eq!(
        changed, 0,
        "{changed} prey pixels changed; maximum linear channel difference {maximum}"
    );
}

struct Boundary {
    presenter: ArtPresenter,
    prey: OrganismId,
    before: RenderView,
    hunter_before: HunterView,
    captured: RenderView,
    hunter_after: HunterView,
    outgoing: Canvas,
}

fn boundary(enabled: bool, partial: bool) -> Boundary {
    let mut config = WorldConfig::default();
    config.founders.kinds.clear();
    config.founders.count = 1;
    let mut world = World::new(config).unwrap();
    let prey = world.render_view().organisms[0].id;
    world
        .start_hunter_trial(
            FixedHunterProfile::lanternjaw_trial(world.config()),
            HunterTarget {
                face: 0,
                u: 45.,
                v: 45.,
            },
        )
        .unwrap();
    let mut h = world.hunter_view()[0];
    h.phase = HunterPhase::Strike;
    h.entered_from = HunterPhase::Windup;
    h.phase_started_tick = 100;
    h.phase_ends_tick = 131;
    h.episode = 1;
    h.target = Some(prey);
    let mut view = world.render_view();
    for field in [
        &mut view.producer,
        &mut view.detritus,
        &mut view.fruit,
        &mut view.water,
    ] {
        field.fill(0.);
    }
    view.rain.fill(0.);
    let mut p = presenter(enabled);
    for tick in 100..=130 {
        view.tick = tick;
        let o = view.organisms.iter_mut().find(|o| o.id == prey).unwrap();
        o.pos = SurfacePoint::new(Face::Front, 10., 10.);
        o.heading = Vec2::new(1., 0.);
        o.moved.clear();
        o.gestation = None;
        o.juvenile = false;
        o.form = 1;
        o.fed = if partial { tick == 129 } else { tick > 100 };
        o.mode = if partial && tick != 129 {
            Mode::Seeking
        } else {
            Mode::Feeding
        };
        observe(&mut p, &view, h);
    }
    let weight = p.meal_of(prey).unwrap().weight;
    assert!(weight > 0.);
    if partial {
        assert!(weight < 1.);
    } else {
        assert_eq!(weight, 1.);
    }
    let outgoing = draw(&mut p, &view, 1.);
    let before = view.clone();
    let hunter_before = h;
    view.tick = 131;
    view.organisms.retain(|o| o.id != prey);
    h.phase = HunterPhase::Handling;
    h.entered_from = HunterPhase::Strike;
    h.phase_started_tick = 131;
    h.phase_ends_tick = 140;
    h.target = None;
    observe(&mut p, &view, h);
    assert!(p.hunter_of(h.id).unwrap().prey.is_some());
    assert!(
        p.meal_of(prey).is_none(),
        "a removed prey is not a living meal"
    );
    Boundary {
        presenter: p,
        prey,
        before,
        hunter_before,
        captured: view,
        hunter_after: h,
        outgoing,
    }
}

#[test]
fn shared_clock_control_and_established_meal_keep_capture_boundary_pixels() {
    for enabled in [false, true] {
        let mut s = boundary(enabled, false);
        let incoming = draw(&mut s.presenter, &s.captured, 0.);
        same_prey(&s.outgoing, &incoming);
    }
}

#[test]
fn a_partial_meal_keeps_its_outgoing_body_cross_fade_too() {
    let mut s = boundary(true, true);
    let incoming = draw(&mut s.presenter, &s.captured, 0.);
    same_prey(&s.outgoing, &incoming);
}

#[test]
fn repeated_capture_observation_holds_and_next_tick_discards_the_prey() {
    let mut s = boundary(true, true);
    let middle = draw(&mut s.presenter, &s.captured, 0.5);
    observe(&mut s.presenter, &s.captured, s.hunter_after);
    observe(&mut s.presenter, &s.captured, s.hunter_after);
    same_prey(&middle, &draw(&mut s.presenter, &s.captured, 0.5));
    s.captured.tick += 1;
    observe(&mut s.presenter, &s.captured, s.hunter_after);
    assert!(
        s.presenter
            .hunter_of(s.hunter_after.id)
            .unwrap()
            .prey
            .is_none()
    );
    assert!(s.presenter.meal_of(s.prey).is_none());
    let mut fresh = presenter(true);
    observe(&mut fresh, &s.captured, s.hunter_after);
    same_prey(
        &draw(&mut fresh, &s.captured, 0.),
        &draw(&mut s.presenter, &s.captured, 0.),
    );
}

#[test]
fn rewind_drops_the_outgoing_meal_instead_of_replaying_it() {
    let mut s = boundary(true, false);
    observe(&mut s.presenter, &s.before, s.hunter_before);
    let mut fresh = presenter(true);
    observe(&mut fresh, &s.before, s.hunter_before);
    assert_eq!(s.presenter.meal_of(s.prey), fresh.meal_of(s.prey));
    assert_eq!(s.presenter.meal_of(s.prey).unwrap().onset, None);
    same_prey(
        &draw(&mut fresh, &s.before, 0.5),
        &draw(&mut s.presenter, &s.before, 0.5),
    );
}
