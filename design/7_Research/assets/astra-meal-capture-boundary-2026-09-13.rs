//! Synthetic published views isolate the meal-to-retained-prey renderer handoff.
//! They do not assert that this remote synthetic capture is biologically admissible.
use cubarium::{art::ArtPack, art_present::ArtPresenter};
use cubarium_core::{World, WorldConfig, FixedHunterProfile, HunterTarget, HunterView};
use cubarium_core::{hunter::HunterPhase, organism::Mode, view::RenderView};
use cubarium_render::Canvas;
use cubarium_surface::{Face, SurfacePoint, Vec2};

fn presenter(enabled: bool) -> ArtPresenter {
    let p = ArtPresenter::new(ArtPack::load(std::path::Path::new(
        "/home/wrysk/wryskware/cubarium/assets/atelier")).unwrap());
    if enabled { p } else { p.without_meal_onset() }
}

fn observe(p: &mut ArtPresenter, view: &RenderView, hunter: HunterView) {
    p.observe(view);
    p.observe_hunters(view, &[hunter], &[]).unwrap();
}

fn boundary_delta(enabled: bool) -> (usize, f32) {
    let mut config = WorldConfig::default();
    config.founders.kinds.clear();
    config.founders.count = 1;
    let mut world = World::new(config).unwrap();
    let prey = world.render_view().organisms[0].id;
    world.start_hunter_trial(FixedHunterProfile::lanternjaw_trial(world.config()),
        HunterTarget { face: 0, u: 45., v: 45. }).unwrap();
    let mut h = world.hunter_view()[0];
    h.phase = HunterPhase::Strike;
    h.entered_from = HunterPhase::Windup;
    h.phase_started_tick = 100;
    h.phase_ends_tick = 131;
    h.episode = 1;
    h.target = Some(prey);
    let mut view = world.render_view();
    for field in [&mut view.producer, &mut view.detritus, &mut view.fruit,
        &mut view.water] { field.fill(0.); }
    view.rain.fill(0.);
    let mut p = presenter(enabled);
    for tick in 100..=130 {
        view.tick = tick;
        let o = view.organisms.iter_mut().find(|o| o.id == prey).unwrap();
        o.pos = SurfacePoint::new(Face::Front,10.,10.);
        o.heading = Vec2::new(1.,0.);
        o.moved.clear(); o.fed = tick > 100; o.mode = Mode::Feeding;
        o.gestation = None; o.juvenile = false; o.form = 1;
        observe(&mut p,&view,h);
    }
    assert_eq!(p.meal_of(prey).unwrap().weight,1.);
    let mut before = Canvas::new();
    p.draw(&view,1.,&mut before);
    view.tick = 131;
    view.organisms.retain(|o| o.id != prey);
    h.phase = HunterPhase::Handling;
    h.entered_from = HunterPhase::Strike;
    h.phase_started_tick = 131;
    h.phase_ends_tick = 140;
    h.target = None;
    observe(&mut p,&view,h);
    assert!(p.hunter_of(h.id).unwrap().prey.is_some());
    let mut after = Canvas::new();
    p.draw(&view,0.,&mut after);
    let mut count = 0; let mut max = 0.0f32;
    for y in 2..19 { for x in 2..19 {
        let a=before.get(Face::Front,x,y); let b=after.get(Face::Front,x,y);
        if a != b { count+=1; }
        for (u,v) in a.into_iter().zip(b) { max=max.max((u-v).abs()); }
    } }
    (count,max)
}

#[test]
fn existing_shared_phase_control_is_continuous_in_the_same_fixture() {
    assert_eq!(boundary_delta(false),(0,0.));
}

#[test]
fn retained_prey_must_keep_its_outgoing_meal_pose_at_capture_boundary() {
    let (pixels,delta)=boundary_delta(true);
    assert_eq!(pixels,0,"meal handoff changed {pixels} pixels at one instant, max linear-channel delta {delta}");
}
