//! Isolated public-API diagnostics. Three adapter tests fail against f4420fc;
//! the renderer's full admitted grid-transition sweep passes against 5b24ac4.
#![cfg(test)]
use cubarium::{art::ArtPack, art_present::ArtPresenter, hunter_present::HunterMemory, lanternjaw};
use cubarium_core::{FixedHunterProfile, HunterTarget, HunterView, World, WorldConfig};
use cubarium_core::{hunter::HunterPhase, view::RenderView};
use cubarium_surface::Vec2;

fn presenter() -> ArtPresenter {
    ArtPresenter::new(ArtPack::load(std::path::Path::new(
        "/home/wrysk/wryskware/cubarium/assets/atelier"
    )).unwrap())
}

fn world(custom_geometry: bool) -> World {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds.clear();
    cfg.founders.count = 0;
    let mut world = World::new(cfg).unwrap();
    let mut profile = FixedHunterProfile::lanternjaw_trial(world.config());
    if custom_geometry { profile.capture_offset_body = Vec2::ZERO; }
    world.start_hunter_trial(profile, HunterTarget { face: 0, u: 32.0, v: 32.0 }).unwrap();
    world
}

// Synthetic observer frames isolate presentation bookkeeping, not actual ecological phases.
fn frame(world: &World, tick: u64, phase: HunterPhase, started: u64, ends: u64, origin: HunterPhase)
    -> (RenderView, HunterView)
{
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
fn observe(p: &mut ArtPresenter, pair: &(RenderView, HunterView)) {
    p.observe(&pair.0);
    p.observe_hunters(&pair.0, &[pair.1]);
}

#[test]
fn repeated_same_tick_observation_preserves_fractional_pose() {
    let w = world(false);
    let strike = frame(&w, 19, HunterPhase::Strike, 1, 20, HunterPhase::Windup);
    let handled = frame(&w, 20, HunterPhase::Handling, 20, 20, HunterPhase::Strike);
    let mut p = presenter();
    observe(&mut p, &strike);
    observe(&mut p, &handled);
    let before = p.hunter_of(handled.1.id).unwrap().living_pose(20, 0.5, &[]);
    observe(&mut p, &handled);
    let after = p.hunter_of(handled.1.id).unwrap().living_pose(20, 0.5, &[]);
    assert_eq!(before, after, "a repeated observation must not discard the prior tick's phase");
}

#[test]
fn rewound_presenter_reconstructs_hunter_memory_like_fresh() {
    let w = world(false);
    let strike = frame(&w, 19, HunterPhase::Strike, 1, 20, HunterPhase::Windup);
    let handled = frame(&w, 20, HunterPhase::Handling, 20, 20, HunterPhase::Strike);
    let old = frame(&w, 5, HunterPhase::Windup, 2, 10, HunterPhase::Stalking);
    let mut reused = presenter();
    observe(&mut reused, &strike);
    observe(&mut reused, &handled);
    observe(&mut reused, &old);
    let mut fresh = presenter();
    observe(&mut fresh, &old);
    let a: &HunterMemory = reused.hunter_of(old.1.id).unwrap();
    let b: &HunterMemory = fresh.hunter_of(old.1.id).unwrap();
    assert_eq!(a.from, b.from, "future attack reach must not leak into a rewound world");
}

#[test]
fn a_supported_named_rig_agrees_with_admitted_core_contact_geometry() {
    let w = world(true);
    let view = w.render_view();
    let h = w.hunter_view()[0];
    let mut p = presenter();
    p.observe(&view);
    p.observe_hunters(&view, &[h]);
    if p.hunter_ids().contains(&h.id) {
        assert_eq!(h.geometry.capture_offset_body, lanternjaw::effectors(h.body_scale).near_claw,
            "supported scale alone cannot authorize a different capture effector");
    }
}

#[test]
fn all_admitted_grid_boundaries_are_continuous_and_work_is_bounded() {
    use cubarium_render::{Canvas, RigPart, Sprite, MIN_RIG_SCALE, grid_schedule, stamp_rig_scaled};
    use cubarium_surface::{Face, SurfacePoint};
    let sprite = Sprite::from_premultiplied(1,1,Vec2::new(0.5,0.5),vec![[1.0;4]]).unwrap();
    let parts = [RigPart { sprite: &sprite, offset: Vec2::ZERO, layer: 0 }];
    let draw = |scale| {
        let mut canvas = Canvas::new();
        stamp_rig_scaled(&mut canvas, SurfacePoint::new(Face::Front,32.31,32.77),
            Vec2::new(0.6,-0.8), &[(&parts,1.0)], scale, 1.0, &mut Vec::new());
        canvas
    };
    for k in 1..=8 {
        let center = 1.0 / k as f64;
        let a = draw((center - 1e-9).max(MIN_RIG_SCALE));
        let b = draw(center + 1e-9);
        for y in 29..36 { for x in 29..36 {
            for (u,v) in a.get(Face::Front,x,y).into_iter().zip(b.get(Face::Front,x,y)) {
                assert!((u-v).abs() < 1e-5, "grid{k} discontinuity");
            }
        }}
    }
    for i in 0..=1000 {
        let scale = MIN_RIG_SCALE + (1.0-MIN_RIG_SCALE)*i as f64/1000.0;
        let (lo,hi,w) = grid_schedule(scale);
        assert!(lo>=1 && hi<=8 && w>=0.0 && w<1.0);
        let samples = if lo==hi || w==0.0 { hi*hi } else { lo*lo+hi*hi };
        assert!(samples<=113);
    }
}
