//! Independent public-contract checks for the ambient wind sampler and asset budgets.

use std::path::Path;

use cubarium::art::{ArtPack, Band, Clip, Plant, Transition};
use cubarium::art_present::{
    PLANT_BEND_LENGTH, PLANT_BEND_ROOT, WIND_FALL, WIND_HOLD, WIND_PERIOD, WIND_RISE,
    WIND_SLOT_VARIATION, effective_tip, plant_bend, plant_bend_budget, wind_at, wind_chart,
    wind_strength,
};
use cubarium_render::{Bend, Canvas, Mask, Pose, Sprite, stamp_layers_bent};
use cubarium_surface::{SurfacePoint, Vec2, travel};
use cube_proto::{Edge, Face};

#[test]
fn gusts_remain_bounded_and_have_an_exact_shared_quiet_interval() {
    let mut peak = 0.0f64;
    for i in 0..100_000 {
        let strength = wind_strength(f64::from(i) * 0.01);
        assert!(
            (0.0..=1.0).contains(&strength),
            "peak modulation exceeded the admitted wind range"
        );
        peak = peak.max(strength);
    }
    assert!(
        peak > 0.5,
        "a permanently still sampler cannot satisfy this fixture"
    );
    let quiet_middle = (WIND_RISE + WIND_HOLD + WIND_FALL + WIND_PERIOD) * 0.5;
    for packet in 0..10 {
        let time = f64::from(packet) * WIND_PERIOD + quiet_middle;
        assert_eq!(wind_strength(time), 0.0);
        for face in Face::ALL {
            for u in [0.5, 16.0, 32.0, 48.0, 63.5] {
                for lag in [0.0, 0.1, 0.2] {
                    assert_eq!(
                        wind_at(SurfacePoint::new(face, u, 32.0), time, lag),
                        Vec2::ZERO
                    );
                }
            }
        }
    }
}

#[test]
fn all_packet_boundaries_are_continuous_including_the_flutter_hold() {
    let epsilon = 1e-6;
    for packet in 1..6 {
        for boundary in [
            0.0,
            WIND_RISE,
            WIND_RISE + WIND_HOLD,
            WIND_RISE + WIND_HOLD + WIND_FALL,
        ] {
            let time = f64::from(packet) * WIND_PERIOD + boundary;
            let left = wind_strength(time - epsilon);
            let at = wind_strength(time);
            let right = wind_strength(time + epsilon);
            assert!(
                (at - left).abs().max((right - at).abs()) < 1e-5,
                "wind jumped at packet boundary {time}: {left}, {at}, {right}"
            );
        }
    }
}

#[test]
fn the_delayed_wind_field_agrees_across_real_surface_transport() {
    let epsilon = 1e-6;
    for face in Face::ALL {
        for edge in Edge::ALL {
            // The open bottom is a reflection boundary rather than a connected seam.
            if face != Face::Top && edge == Edge::Bottom {
                continue;
            }
            for along in [8.0, 24.0, 40.0, 56.0] {
                let (u, v, delta) = match edge {
                    Edge::Top => (along, epsilon, Vec2::new(0.0, -2.0 * epsilon)),
                    Edge::Right => (64.0 - epsilon, along, Vec2::new(2.0 * epsilon, 0.0)),
                    Edge::Bottom => (along, 64.0 - epsilon, Vec2::new(0.0, 2.0 * epsilon)),
                    Edge::Left => (epsilon, along, Vec2::new(-2.0 * epsilon, 0.0)),
                };
                let start = SurfacePoint::new(face, u, v);
                let crossed = travel(start, delta);
                assert_eq!(crossed.crossings, 1, "fixture must cross a real seam");
                assert_eq!(crossed.reflections, 0);
                for seconds in [0.0, 5.0, 13.0, 21.0, 37.0] {
                    let transported = crossed.map.apply(wind_at(start, seconds, 0.1));
                    let actual = wind_at(crossed.end, seconds, 0.1);
                    assert!(
                        (transported - actual).length() < 1e-6,
                        "wind changes chart at {face:?}/{edge:?}, {along}, t={seconds}: {transported:?} vs {actual:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn per_slot_variation_cannot_exceed_the_final_admitted_tip_budget() {
    for budget in [0.0, 0.05, 0.2, 0.6] {
        for desired in [0.12, 0.45, 0.7, 0.9] {
            for variation in [1.0 - WIND_SLOT_VARIATION, 1.0, 1.0 + WIND_SLOT_VARIATION] {
                let tip = effective_tip(desired, budget) * variation;
                assert!(
                    tip >= 0.0 && tip <= budget + 1e-12,
                    "variation escaped budget: {tip} > {budget}"
                );
                for face in Face::ALL {
                    for u in [0.0, 16.0, 32.0, 48.0, 64.0] {
                        let wind = wind_chart(face, u, 32.0);
                        let bend = plant_bend(tip, wind, Vec2::new(0.8, 0.6));
                        assert!(bend.amplitude.abs() <= budget + 1e-12);
                    }
                }
            }
        }
    }
}

fn one_texel(x: usize, y: usize) -> Sprite {
    let mut bytes = vec![0; 16 * 16 * 4];
    bytes[(y * 16 + x) * 4..(y * 16 + x + 1) * 4].fill(255);
    Sprite::from_rgba(16, 16, Vec2::new(8.0, 8.0), &bytes).unwrap()
}

#[test]
fn authored_growth_frames_participate_in_the_family_wind_budget() {
    let ordinary = one_texel(8, 8);
    let wide_growth = one_texel(3, 2);
    let transition_limit = wide_growth.bend_headroom(PLANT_BEND_ROOT, PLANT_BEND_LENGTH, 0.0);
    let mut plant = Plant {
        name: "lanternstalk".into(),
        band: Band::Foliage,
        stages: std::array::from_fn(|_| Clip {
            frames: vec![ordinary.clone(), ordinary.clone()],
            seconds: 3.0,
            looping: true,
        }),
        fruit: None,
        transitions: Vec::new(),
    };
    assert!(
        plant_bend_budget(&plant) > transition_limit,
        "growth fixture must be the limiting pose"
    );
    plant.transitions.push(Transition {
        from: 0,
        to: 1,
        clip: Clip {
            frames: vec![ordinary, wide_growth],
            seconds: 4.0,
            looping: false,
        },
    });
    assert!(
        plant_bend_budget(&plant) <= transition_limit,
        "the family's admitted bend ignores a wider authored growth pose"
    );
}

#[test]
fn shipped_lanternstalk_root_contact_does_not_move_under_added_wind() {
    let art = ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"))
        .expect("the checked-in art pack must load");
    let plant = art.plant("lanternstalk").unwrap();
    let tip = plant_bend_budget(plant).min(0.45);
    assert!(tip > 0.0, "root fixture must apply real wind");
    for (stage, clip) in plant.stages.iter().enumerate() {
        let sprite = &clip.frames[0];
        let paint = |bend| {
            let mut canvas = Canvas::new();
            stamp_layers_bent(
                &mut canvas,
                SurfacePoint::new(Face::Front, 32.0, 32.0),
                Vec2::new(1.0, 0.0),
                &[(Pose::still(sprite), 1.0)],
                1.0,
                1.0,
                Mask::None,
                bend,
                &mut Vec::new(),
            );
            canvas
        };
        let still = paint(Bend::NONE);
        // The actual scene's shared root is tile(8.5,15); its lowest painted row
        // is row14, not row15. With this anchor that row occupies output y=38.
        assert!((24..40).any(|x| still.get(Face::Front, x, 38).into_iter().any(|c| c > 0.0)));
        for sign in [-1.0, 1.0] {
            let windy = paint(plant_bend(tip, Vec2::new(sign, 0.0), Vec2::new(1.0, 0.0)));
            for x in 23..41 {
                assert_eq!(
                    windy.get(Face::Front, x, 38),
                    still.get(Face::Front, x, 38),
                    "stage{stage} root-contact row skated at x={x}, wind sign {sign}"
                );
            }
        }
    }
}
