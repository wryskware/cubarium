//! The art adapter's capability under the paid-charging semantic version.
//!
//! Version 4 is a **physiology** policy. The renderer's capability check is about role,
//! effectors and scale — a version shortcut has never been part of it, and must not become one.
//! So two things have to hold, and neither is obvious enough to assume:
//!
//! * a version 4 profile is accepted exactly where a version 3 one is, and **rejected** exactly
//!   where a version 3 one is, so raising the version cannot smuggle a geometry change past the
//!   renderer;
//! * given the *same supplied state and pose*, both versions draw the same silhouette, to the
//!   byte. What the two versions do differently is happen, not look different.
//!
//! This file adds no behaviour to the presentation crate and edits none of it: it only asks the
//! published capability questions. Nothing here is evidence about ecological balance.

use cubarium::art::ArtPack;
use cubarium::art_present::ArtPresenter;
use cubarium::hunter_present::validate_profile;
use cubarium_core::hunter::{
    FixedHunterProfile, HunterTarget, PROFILE_VERSION, PROFILE_VERSION_CHARGE80,
};
use cubarium_core::{World, WorldConfig};
use cube_proto::Frame;
use cubarium_render::Canvas;

fn atelier() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier")
}

fn presenter() -> ArtPresenter {
    ArtPresenter::new(ArtPack::load(&atelier()).expect("the shipped pack loads"))
}

/// A quiet world with one founded member at the given semantic profile version.
fn founded(version: u32) -> World {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds.clear();
    cfg.founders.count = 0;
    cfg.weather.amplitude = 0.0;
    cfg.water.rain_rate = 0.0;
    let mut world = World::new(cfg).expect("a quiet world is valid");
    let mut profile = FixedHunterProfile::lanternjaw_trial(world.config());
    if version == PROFILE_VERSION_CHARGE80 {
        profile = profile.charge80();
    }
    world
        .start_hunter_trial(profile, HunterTarget { face: 4, u: 32.0, v: 32.0 })
        .expect("the trial starts");
    world
}

/// One way to break a profile's geometry, named so a failure says which.
type Damage = fn(&mut FixedHunterProfile);

/// The renderer accepts both versions, and its refusals are unchanged by the version: the
/// capability is still about the body, exactly as it was.
#[test]
fn the_renderer_accepts_both_versions_and_refuses_the_same_geometry_either_way() {
    let cfg = WorldConfig::default();
    let three = FixedHunterProfile::lanternjaw_trial(&cfg);
    let four = three.clone().charge80();
    assert_eq!(three.version, PROFILE_VERSION);
    assert_eq!(four.version, PROFILE_VERSION_CHARGE80);

    validate_profile(&three).expect("version 3 is drawable");
    validate_profile(&four).expect("version 4 is drawable: it changed no geometry");
    let art = presenter();
    art.validate_hunter_profile(&three).expect("version 3");
    art.validate_hunter_profile(&four).expect("version 4");

    // Every refusal the renderer already had is still a refusal at version 4, with the same
    // reason — the version is not a bypass, and not a second gate either.
    let damage: [(&str, Damage); 4] = [
        ("claw", |p| p.capture_offset_body.x += 1.0),
        ("mouth", |p| p.ingestion_offset_body.y += 1.0),
        ("scale", |p| p.body_scale_min = 0.01),
        ("role", |p| p.role = cubarium_core::HunterRole::Lanternjaw),
    ];
    for (name, break_it) in damage {
        let (mut a, mut b) = (three.clone(), four.clone());
        break_it(&mut a);
        break_it(&mut b);
        assert_eq!(
            validate_profile(&a).err(),
            validate_profile(&b).err(),
            "{name}: the two versions must give the renderer the same answer"
        );
    }
}

/// Given the same supplied state and pose, the two versions draw the same frame, byte for byte.
///
/// The worlds are built identically and never stepped, so what differs between them is the
/// stored version alone — which is exactly the question: does raising it change the picture?
#[test]
fn both_versions_draw_the_same_supplied_state_to_the_byte() {
    let mut frames = Vec::new();
    for version in [PROFILE_VERSION, PROFILE_VERSION_CHARGE80] {
        let world = founded(version);
        assert_eq!(world.hunters().profile().expect("a profile").version, version);
        let mut art = presenter();
        art.validate_hunter_profile(world.hunters().profile().expect("a profile"))
            .expect("drawable");
        let view = world.render_view();
        art.observe(&view);
        art.observe_hunters(&view, &world.hunter_view(), &[]).expect("the adapter accepts it");
        let mut canvas = Canvas::new();
        art.draw(&view, 0.0, &mut canvas);
        let mut frame = Frame::black();
        canvas.encode(&mut frame);
        frames.push(frame);
    }
    assert_eq!(
        frames[0].as_bytes(),
        frames[1].as_bytes(),
        "raising the semantic version changed the silhouette"
    );
    assert!(
        frames[0].as_bytes().iter().any(|&b| b != 0),
        "the fixture must actually be drawing something"
    );
}
