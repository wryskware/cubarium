use super::*;

fn params() -> VoxelParams {
    let l = 6f32.sqrt();
    VoxelParams {
        lit: true,
        sun: [-1.0 / l, 2.0 / l, -1.0 / l],
        sun_tint: 0.18,
        sky: [0.001, 0.0005, 0.005],
        sky_horizon: [0.009, 0.004, 0.026],
        haze_colour: [0.018, 0.008, 0.095],
        ambient_colour: unit([0.9, 0.95, 1.2]),
        light: [0.054, 0.56, 1.0],
        ..crate::voxel::tests::params()
    }
}

/// The day clock's elevation and daylight at `phase` (daylight 0.67, 70° peak).
fn clock(phase: f32) -> (f32, f32) {
    let (rise, set) = (0.165, 0.835);
    let el = if (rise..set).contains(&phase) {
        (std::f32::consts::PI * (phase - rise) / 0.67).sin()
    } else {
        -(std::f32::consts::PI * (phase - set).rem_euclid(1.0) / 0.33).sin()
    } * 70f32.to_radians();
    (el, smoothstep(-6.0, 6.0, el.to_degrees()))
}

fn at(phase: f32, cloud: f32) -> Eased {
    let (el, dl) = clock(phase);
    Eased {
        day_phase: phase,
        daylight: dl,
        sun_elevation: el,
        cloud_cover: cloud,
        storm: 0.0,
        drift: 0.0,
        seconds: 0.0,
    }
}

#[test]
fn a_clear_noon_is_todays_picture() {
    let p = params();
    let l = noon(&p);
    assert_eq!((l.sky, l.sky_horizon, l.haze), (p.sky, p.sky_horizon, p.haze_colour));
    assert_eq!((l.ambient, l.sun, l.sun_lean), (p.ambient_colour, p.sun, p.light));
    assert_eq!((l.ambient_level, l.sun_strength, l.sun_tint), (1.0, 1.0, p.sun_tint));
    assert_eq!((l.tint, l.unlit), ([1.0; 3], 1.0));
    assert_eq!((l.stars, l.band, l.cloud_cover), (0.0, 0.0, 0.0));
    assert!(!l.animated());
}

#[test]
fn night_is_darker_starry_and_moonlit() {
    let p = params();
    let (day, night) = (noon(&p), look(&at(0.0, 0.0), &p));
    assert!(night.ambient_level < 0.5 * day.ambient_level);
    assert!(night.stars > 0.99 && night.animated());
    assert!(night.sun_strength > 0.0 && night.sun_strength <= MOON);
    assert!(luma(night.sky) < luma(day.sky) && luma(night.haze) < luma(day.haze));
    assert!(luma(night.tint) < 0.5);
    // The moonlight leans cool: bluer than red.
    assert!(night.sun_lean[2] > night.sun_lean[0]);
}

#[test]
fn the_sun_arcs_through_the_front_hemisphere() {
    let p = params();
    for k in 0..200 {
        let l = look(&at(k as f32 / 200.0, 0.0), &p);
        if l.sun_strength > 0.0 {
            assert!(l.sun[2] <= 0.0, "phase {}: behind the scene", k as f32 / 200.0);
            assert!(l.sun[1] >= MIN_SHADOW_DEG.to_radians().sin() - 1e-4);
        }
    }
    // Morning sun from the left, evening from the right.
    assert!(look(&at(0.2, 0.0), &p).sun[0] < -0.8);
    assert!(look(&at(0.8, 0.0), &p).sun[0] > 0.4);
}

#[test]
fn dawn_is_warm_low_and_banded() {
    let p = params();
    let l = look(&at(0.17, 0.0), &p);
    assert!(l.sun_lean[0] > l.sun_lean[2], "{:?}", l.sun_lean);
    assert!(l.sun_tint > p.sun_tint);
    assert!(l.band > 0.5 * BAND_GAIN);
    assert!(l.sun_strength < 1.0);
    // Overcast hides the band and the sun.
    let grey = look(&at(0.17, 1.0), &p);
    assert!(grey.band < 0.5 * l.band && grey.sun_strength < 0.5 * l.sun_strength);
}

#[test]
fn easing_moves_every_frame_and_never_jumps() {
    let mut e = WeatherEase::default();
    let first = e.advance(&Weather::CLEAR_NOON, 0.0, 256);
    assert_eq!(first.cloud_cover, 0.0);
    let overcast = Weather { cloud_cover: 1.0, ..Weather::CLEAR_NOON };
    let mut last = 0.0;
    for _ in 0..60 * 60 {
        let c = e.advance(&overcast, 1.0 / 60.0, 256).cloud_cover;
        assert!(c > last && c - last < 0.01);
        last = c;
    }
    assert!(last > 0.99);
}

#[test]
fn the_phase_eases_the_short_way_round() {
    let mut e = WeatherEase::default();
    e.advance(&Weather { day_phase: 0.99, ..Weather::CLEAR_NOON }, 0.0, 256);
    let p = e.advance(&Weather { day_phase: 0.01, ..Weather::CLEAR_NOON }, 0.1, 256).day_phase;
    assert!(!(0.02..0.99).contains(&p), "went the long way: {p}");
}

#[test]
fn clouds_drift_round_the_ring() {
    let mut e = WeatherEase::default();
    e.advance(&Weather::CLEAR_NOON, 0.0, 10);
    let d = (0..100).map(|_| e.advance(&Weather::CLEAR_NOON, 1.0, 10).drift).fold(0.0, f32::max);
    assert!(d < 10.0 && d > 0.0);
}

/// The day sky (checkpoint 1b) is whole at noon, absent at night and on the horizon (so
/// the dawn and dusk band are checkpoint 1's), and comes in without a jump.
#[test]
fn the_day_sky_leaves_twilight_and_night_alone() {
    let p = params();
    assert_eq!(noon(&p).day, 1.0);
    assert_eq!(look(&at(0.0, 0.0), &p).day, 0.0);
    assert_eq!(look(&at(0.165, 0.0), &p).day, 0.0);
    let mut last = 0.0f32;
    for i in 0..=1000 {
        let d = look(&at(0.1 + 0.4 * i as f32 / 1000.0, 0.0), &p).day;
        assert!((d - last).abs() < 0.02, "day sky jumps at step {i}: {last} -> {d}");
        last = d;
    }
}
