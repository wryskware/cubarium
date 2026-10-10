//! GLOW-EASE's mechanisms on hand-built emitters, against whatever fade and flicker the
//! test's own [`Ease`] says (no default is asserted).

use super::*;

const FPS: f64 = 60.0;

fn ease(fade_s: f32) -> Ease {
    Ease { fade_s, ..Ease::STEPS }
}

fn lantern(cell: u32, rgb: [f32; 3], level: f32) -> Target {
    Target { cell, rgb, level, kind: Kind::Steady, seed: 1 }
}

fn lum(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

/// Frames from `t0` for `seconds`, the value of `cell` at each (none: 0).
fn film(g: &GlowEase, cell: u32, t0: f64, seconds: f64) -> Vec<([f32; 3], f32)> {
    (0..=(seconds * FPS).round() as usize)
        .map(|k| g.value(cell, t0 + k as f64 / FPS).unwrap_or(([0.0; 3], 0.0)))
        .collect()
}

#[test]
fn a_switch_on_and_off_passes_through_intermediate_values_and_never_outruns_the_fade() {
    let e = ease(1.5);
    let full = [0.9, 0.5, 0.2];
    let mut g = GlowEase::new(e);
    g.update(0.0, []);
    // On at t = 1: from nothing, through values in between, to full by the fade's end.
    g.update(1.0, [lantern(7, full, 1.0)]);
    let frames = film(&g, 7, 1.0, f64::from(e.fade_s) + 0.5);
    let allowed = lum(full) / (e.fade_s * FPS as f32) + 1e-5;
    let mut between = 0;
    for w in frames.windows(2) {
        let (a, b) = (lum(w[0].0), lum(w[1].0));
        assert!((b - a).abs() <= allowed, "one frame moved {} (the fade allows {allowed})", (b - a).abs());
        assert!(b >= a, "switching on never dims");
    }
    for (rgb, on) in &frames {
        if lum(*rgb) > 0.01 * lum(full) && lum(*rgb) < 0.99 * lum(full) {
            between += 1;
            assert!(*on > 0.0 && *on < 1.0, "the on share ramps with it");
        }
    }
    assert!(between as f64 >= 0.9 * f64::from(e.fade_s) * FPS, "only {between} frames in between");
    assert_eq!(frames.first().unwrap().0, [0.0; 3], "it starts dark");
    assert_eq!(frames.last().unwrap().0, full, "it ends full");
    // Off at t = 4: no longer targeted, it ramps down the same way and is forgotten at 0.
    g.update(4.0, []);
    assert_eq!(g.released(), &[7], "let go this tick");
    assert!(g.holds(7), "held while it fades");
    let frames = film(&g, 7, 4.0, f64::from(e.fade_s));
    for w in frames.windows(2) {
        let d = lum(w[0].0) - lum(w[1].0);
        assert!((0.0..=allowed).contains(&d), "switching off moves down by at most the fade's step, {d}");
    }
    assert!(lum(frames[frames.len() / 2].0) > 0.0, "still glowing half way");
    g.update(4.0 + f64::from(e.fade_s) + 0.01, []);
    assert_eq!(g.gone(), &[7]);
    assert!(g.value(7, 10.0).is_none() && !g.holds(7));
}

#[test]
fn a_target_moving_mid_ramp_starts_from_where_it_had_got_to() {
    let e = ease(2.0);
    let mut g = GlowEase::new(e);
    g.update(0.0, []);
    g.update(1.0, [lantern(3, [1.0; 3], 1.0)]);
    let mid = g.value(3, 2.0).unwrap();
    // Turned off half way up: no jump at the turn, then down.
    g.update(2.0, []);
    let after = g.value(3, 2.0).unwrap();
    assert_eq!(mid, after, "no jump when the target moves");
    assert!(lum(g.value(3, 2.5).unwrap().0) < lum(after.0));
}

#[test]
fn a_colour_swap_passes_through_blends_of_the_two_colours() {
    let (a, b) = ([0.9, 0.3, 0.6], [1.0, 0.45, 0.05]);
    let e = ease(1.0);
    let mut g = GlowEase::new(e);
    g.update(0.0, [lantern(5, a, 1.0)]);
    g.update(0.05, [lantern(5, b, 1.0)]);
    let mut seen_mid = false;
    let mut last_p = 0.0;
    for (rgb, _) in film(&g, 5, 0.05, 1.2) {
        // rgb = a + p (b − a) for one p in [0, 1], the same p on every channel.
        let p: Vec<f32> = (0..3).map(|k| (rgb[k] - a[k]) / (b[k] - a[k])).collect();
        assert!(p.iter().all(|q| (q - p[0]).abs() < 1e-4), "{rgb:?} is not a blend of the two");
        assert!((-1e-6..=1.0 + 1e-6).contains(&p[0]), "{} outside the two", p[0]);
        assert!(p[0] + 1e-6 >= last_p, "the blend runs one way");
        last_p = p[0];
        seen_mid |= p[0] > 0.2 && p[0] < 0.8;
    }
    assert!(seen_mid, "it passed through the blends");
    assert!((last_p - 1.0).abs() < 1e-6, "it ends on the new colour");
}

#[test]
fn a_flower_glows_more_with_more_nectar_and_is_dark_only_when_empty() {
    let mut e = ease(0.0);
    e.nectar = true;
    let fills = [0.0, 0.01, 0.2, 0.5, 0.9, 1.0];
    let levels: Vec<f32> = fills.iter().map(|&f| e.flower_level(f)).collect();
    assert_eq!(levels[0], 0.0, "empty: dark");
    for w in levels.windows(2) {
        assert!(w[1] > w[0], "more nectar, brighter: {levels:?}");
    }
    // Through the instance: the drawn colour and on share follow the level.
    let mut g = GlowEase::new(e);
    let mouth = [0.05, 0.56, 0.94];
    let mut was = -1.0;
    for &f in &fills {
        let t = Target { cell: 1, rgb: mouth, level: e.flower_level(f), kind: Kind::Flower, seed: 9 };
        g.update(0.0, [t]);
        let (rgb, on) = g.value(1, 0.0).unwrap();
        assert!(lum(rgb) > was, "fill {f}: brighter than the fill before");
        assert_eq!(on > 0.0, f > 0.0, "on only with nectar");
        was = lum(rgb);
    }
    // With the nectar glow off an open flower is full whatever its fill (as before).
    e.nectar = false;
    assert!(fills.iter().all(|&f| e.flower_level(f) == 1.0));
}

#[test]
fn the_flicker_is_smooth_and_its_long_mean_is_the_unflickered_glow() {
    let e = Ease { flicker: 0.3, flicker_s: 1.0, ..Ease::STEPS };
    for seed in [1u64, 2, 77, 1 << 40] {
        let mut sum = 0.0f64;
        let mut max_step = 0.0f32;
        let mut prev = None;
        let n = (600.0 * FPS) as usize;
        for k in 0..n {
            let (light, on) = e.modulation(Kind::Lantern, seed, k as f64 / FPS);
            assert_eq!(on, 1.0, "the flicker dims the glow, not the on share");
            assert!((1.0 - e.flicker..=1.0 + e.flicker).contains(&light));
            sum += f64::from(light);
            if let Some(p) = prev {
                max_step = max_step.max((light - p as f32).abs());
            }
            prev = Some(light);
        }
        let mean = sum / n as f64;
        assert!((mean - 1.0).abs() < 0.05 * f64::from(e.flicker), "seed {seed}: mean {mean}");
        assert!(max_step < 0.25 * e.flicker, "seed {seed}: a frame moved {max_step} of a {} flicker", e.flicker);
    }
    // Each lantern on its own phase.
    let a: Vec<f32> = (0..60).map(|k| flicker(1, f64::from(k) / 10.0, 1.0)).collect();
    let b: Vec<f32> = (0..60).map(|k| flicker(2, f64::from(k) / 10.0, 1.0)).collect();
    assert_ne!(a, b);
}

#[test]
fn the_flower_cycle_swells_and_fades_smoothly_on_its_own_phase() {
    let e = Ease { cycle: true, cycle_s: 20.0, ..Ease::STEPS };
    let film: Vec<(f32, f32)> = (0..(40.0 * FPS) as usize).map(|k| e.modulation(Kind::Flower, 3, k as f64 / FPS)).collect();
    let lo = film.iter().map(|m| m.0).fold(f32::MAX, f32::min);
    let hi = film.iter().map(|m| m.0).fold(f32::MIN, f32::max);
    assert!(lo < 0.01 && hi > 0.99, "it goes off and on: {lo}..{hi}");
    assert!(film.iter().all(|m| m.0 == m.1), "it dims the on share with the glow");
    let step = film.windows(2).map(|w| (w[1].0 - w[0].0).abs()).fold(0.0, f32::max);
    assert!(step < 0.01, "a frame moved {step}");
    assert_ne!(cycle(3, 5.0, 20.0), cycle(4, 5.0, 20.0), "its own phase");
    // Off: the flower's glow is its nectar's alone.
    assert_eq!(Ease { cycle: false, ..e }.modulation(Kind::Flower, 3, 7.0), (1.0, 1.0));
}

#[test]
fn the_first_update_and_a_jump_draw_targets_as_they_stand() {
    let mut g = GlowEase::new(ease(3.0));
    g.update(100.0, [lantern(2, [1.0; 3], 1.0)]);
    assert_eq!(g.value(2, 100.0).unwrap().0, [1.0; 3], "the first update: no ramp");
    g.update(100.0 + JUMP_S + 1.0, [lantern(4, [1.0; 3], 1.0)]);
    assert!(g.value(2, 200.0).is_none(), "the jump forgot the cell let go");
    assert_eq!(g.value(4, 200.0).unwrap().0, [1.0; 3], "and drew the new one as it stands");
}
