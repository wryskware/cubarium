// Bounded independent probes against the committed boundary adapter (12c41aa).
use cubarium::hunter_present::{HunterFrame, HunterMemory};
use cubarium_core::{World, WorldConfig, hunter::HunterPhase};
use cubarium_surface::{Face, SurfacePoint, Vec2, unfold, MAX_LOCAL_RADIUS};

fn memory() -> HunterMemory {
    let world = World::new(WorldConfig::default()).unwrap();
    let prey = world.render_view().organisms[0].clone();
    let mut m = HunterMemory::enter(HunterFrame {
        tick: 20, phase: HunterPhase::Handling, started: 20, ends: 20,
        entered_from: HunterPhase::Strike, episode: 1, scale: 1.0,
        gut: 0.5, gestation: None, target: None,
    });
    m.prey = Some(prey);
    m
}

#[test]
fn chord_endpoint_heading_is_expressed_in_the_returned_endpoint_chart() {
    let mut m = memory();
    let heading = Vec2::new(0.6, -0.8);
    let coordinates = [0.0, 0.125, 1.0, 63.0, 63.875];
    for from in Face::ALL {
        for to in Face::ALL {
            for x in coordinates { for y in coordinates {
                let p = SurfacePoint::new(from, x, y);
                for u in coordinates { for v in coordinates {
                    let q = SurfacePoint::new(to, u, v);
                    let Some(chord) = unfold(p, q, MAX_LOCAL_RADIUS) else { continue };
                    let prey = m.prey.as_mut().unwrap();
                    prey.pos = p;
                    prey.heading = heading;
                    m.prey_at = Some(q);
                    let (end, h) = m.retained_prey_pose(1.0).unwrap();
                    let expected = chord.map.inverse().apply(heading);
                    assert_eq!(end, q);
                    assert!((h - expected).length() < 1e-8,
                        "endpoint heading is in the wrong chart: {p:?} -> {q:?}, chord {:?}, got {h:?}, expected {expected:?}", chord.path);
                }}
            }}
        }
    }
}

#[test]
fn sampled_chords_approach_their_recorded_endpoint_without_a_last_frame_jump() {
    let mut m = memory();
    let coordinates = [0.0, 0.125, 1.0, 63.0, 63.875];
    for from in Face::ALL { for to in Face::ALL {
        for x in coordinates { for y in coordinates {
            let p = SurfacePoint::new(from, x, y);
            for u in coordinates { for v in coordinates {
                let q = SurfacePoint::new(to, u, v);
                let Some(chord) = unfold(p, q, MAX_LOCAL_RADIUS) else { continue };
                m.prey.as_mut().unwrap().pos = p;
                m.prey_at = Some(q);
                let (near, _) = m.retained_prey_pose(1.0 - 1e-6).unwrap();
                let remaining = unfold(near, q, MAX_LOCAL_RADIUS).map(|u| u.distance);
                assert!(remaining.is_some_and(|d| d <= chord.distance * 1e-6 + 1e-7),
                    "endpoint jump: {p:?} -> {q:?}, near {near:?}, distance {remaining:?}, chord {:?}", chord.path);
            }}
        }}
    }}
}
