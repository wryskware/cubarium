//! GLOW-EASE round 2's mechanisms on hand-built switches, against whatever fade the test
//! gives (no default is asserted).

use super::*;

/// The packer's rate: a style fade moves once a pack.
const PACK_HZ: f64 = 20.0;
const FPS: f64 = 60.0;

const GREEN: Style = Style { wood: [0.1, 0.1, 0.3], crown: [0.1, 0.6, 0.8], heart: [0.7, 0.9, 1.0] };
const STRAW: Style = Style { wood: [0.1, 0.1, 0.3], crown: [0.7, 0.6, 0.2], heart: [0.9, 0.8, 0.5] };

fn lum(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// The crown's distance from `a` to `b`, largest channel.
fn apart(a: Style, b: Style) -> f32 {
    (0..3).map(|k| (a.crown[k] - b.crown[k]).abs()).fold(0.0, f32::max)
}

/// A cell packed every pack from `t0` for `seconds` with target `to`, `held` what it drew
/// the pack before: what it draws each pack.
fn packs(f: &mut StyleFades, cell: u32, mut held: Style, to: Style, t0: f64, seconds: f64) -> Vec<Style> {
    (0..=(seconds * PACK_HZ).round() as usize)
        .map(|k| {
            let now = t0 + k as f64 / PACK_HZ;
            f.begin(now);
            held = f.draw(cell, Some(held), to, now);
            held
        })
        .collect()
}

#[test]
fn a_style_swap_passes_through_blends_of_the_two_and_never_outruns_the_fade() {
    let fade = 3.0;
    let mut f = StyleFades::new(fade);
    let drawn = packs(&mut f, 5, GREEN, STRAW, 10.0, f64::from(fade) + 1.0);
    let whole = apart(GREEN, STRAW);
    let allowed = whole / (fade * PACK_HZ as f32) + 1e-5;
    let mut between = 0;
    for w in drawn.windows(2) {
        assert!(apart(w[0], w[1]) <= allowed, "one pack moved {} (the fade allows {allowed})", apart(w[0], w[1]));
    }
    for s in &drawn {
        let (a, b) = (apart(*s, GREEN), apart(*s, STRAW));
        if a > 1e-4 && b > 1e-4 {
            between += 1;
            // A blend lies on the line between the two: its distances add up to the whole.
            assert!((a + b - whole).abs() < 1e-4, "{s:?} is not a blend of the two");
        }
    }
    assert!(between as f64 >= 0.9 * f64::from(fade) * PACK_HZ, "only {between} packs in between");
    assert_eq!(drawn[0], GREEN, "the first pack still draws the old colours");
    assert_eq!(*drawn.last().unwrap(), STRAW, "it ends on the new ones");
    assert!(f.is_empty(), "a finished ramp is forgotten");
}

#[test]
fn a_style_reversal_mid_fade_runs_back_from_where_it_got_to() {
    let fade = 2.0;
    let mut f = StyleFades::new(fade);
    let there = packs(&mut f, 1, GREEN, STRAW, 0.0, 0.8);
    let mid = *there.last().unwrap();
    assert!(apart(mid, GREEN) > 0.0 && apart(mid, STRAW) > 0.0, "part way");
    // The stand wakes again: back toward green from the blend, no jump on the way.
    let back = packs(&mut f, 1, mid, GREEN, 0.8 + 1.0 / PACK_HZ, f64::from(fade) + 0.5);
    assert!(lum(GREEN.crown) < lum(STRAW.crown), "fixture");
    let allowed = apart(GREEN, STRAW) / (fade * PACK_HZ as f32) + 1e-5;
    assert!(apart(mid, back[0]) <= allowed, "the reversal starts where the swap had got to");
    for w in back.windows(2) {
        assert!(apart(w[0], w[1]) <= allowed);
        // Green's crown is darker than straw's: heading back only darkens.
        assert!(lum(w[1].crown) <= lum(w[0].crown) + 1e-6, "it heads back");
    }
    assert_eq!(*back.last().unwrap(), GREEN);
}

#[test]
fn a_continuously_moving_style_is_followed_without_a_step() {
    // A source that drifts a little every pack (the style path's wilt): each pack's drawn
    // move is at most the target's own move plus what the fade allows, and it ends there.
    let fade = 2.0;
    let mut f = StyleFades::new(fade);
    let mut held = GREEN;
    let n = (6.0 * PACK_HZ) as usize;
    for k in 0..=n {
        let now = k as f64 / PACK_HZ;
        let to = blend(GREEN, STRAW, k as f32 / n as f32);
        f.begin(now);
        let d = f.draw(3, Some(held), to, now);
        assert!(apart(d, held) <= apart(GREEN, STRAW) / n as f32 + apart(GREEN, STRAW) / (fade * PACK_HZ as f32) + 1e-5);
        held = d;
    }
    let end = 6.0 + f64::from(fade) + 0.1;
    f.begin(end);
    assert_eq!(f.draw(3, Some(held), STRAW, end), STRAW);
}

#[test]
fn a_clock_jump_or_no_fade_draws_the_target_at_once() {
    let mut f = StyleFades::new(0.0);
    assert_eq!(f.draw(9, Some(GREEN), STRAW, 1.0), STRAW, "no fade: at once");
    let mut f = StyleFades::new(2.0);
    f.begin(0.0);
    assert_eq!(f.draw(9, Some(GREEN), STRAW, 0.0), GREEN);
    f.begin(0.0 + JUMP_S + 1.0);
    assert_eq!(f.draw(9, Some(GREEN), STRAW, JUMP_S + 1.0), GREEN, "after a jump a new swap starts afresh");
    f.begin(-5.0);
    assert!(f.is_empty(), "a clock going back ends every ramp");
}

const FULL: Tile = (0x0f, 2);
const BARE: Tile = (0x0f, 0);
const FRUIT: Tile = (0x0f, 2 | 3 << 4);

/// The old tile's share of `cell` each frame from `t0` for `seconds`.
fn shares(v: &VineFades, cell: u32, t0: f64, seconds: f64) -> Vec<(Option<Tile>, f32)> {
    (0..=(seconds * FPS).round() as usize)
        .map(|k| match v.old(cell, t0 + k as f64 / FPS) {
            Some((t, s)) => (Some(t), s),
            None => (None, 0.0),
        })
        .collect()
}

#[test]
fn a_vine_tile_swap_cross_fades_the_old_tile_out_over_frames() {
    let fade = 2.5;
    let mut v = VineFades::new(fade);
    v.switch(40, FULL, BARE, 3.0);
    let s = shares(&v, 40, 3.0, f64::from(fade) + 0.5);
    let allowed = 1.0 / (fade * FPS as f32) + 1e-5;
    assert_eq!(s[0], (Some(FULL), 1.0), "at the switch the old tile shows whole");
    for w in s.windows(2) {
        assert!(w[0].1 - w[1].1 >= -1e-6 && w[0].1 - w[1].1 <= allowed, "the old tile's share falls smoothly");
    }
    let between = s.iter().filter(|(_, x)| *x > 0.01 && *x < 0.99).count();
    assert!(between as f64 >= 0.9 * f64::from(fade) * FPS);
    assert_eq!(s.last().unwrap().1, 0.0, "and is gone at the end");
    v.retire(3.0 + f64::from(fade));
    assert!(v.is_empty());
}

#[test]
fn a_vine_reversal_mid_fade_runs_back_and_a_third_tile_starts_from_the_one_showing_more() {
    let fade = 2.0;
    let mut v = VineFades::new(fade);
    v.switch(7, FULL, BARE, 0.0);
    let t = 0.5;
    let (_, s) = v.old(7, t).unwrap();
    // Back to full at t: the bare tile is now the old one, showing what full did not.
    v.switch(7, BARE, FULL, t);
    let (old, s2) = v.old(7, t).unwrap();
    assert_eq!(old, BARE);
    assert!((s2 - (1.0 - s)).abs() < 1e-5, "full still shows {s}, so bare shows {}", 1.0 - s);
    let back = shares(&v, 7, t, f64::from(fade));
    for w in back.windows(2) {
        assert!(w[0].1 >= w[1].1 - 1e-6 && w[0].1 - w[1].1 <= 1.0 / (fade * FPS as f32) + 1e-5);
    }
    // A third tile while two show: the one showing more is the new ramp's old tile.
    let mut v = VineFades::new(fade);
    v.switch(8, FULL, BARE, 0.0);
    v.switch(8, BARE, FRUIT, 0.2 * f64::from(fade));
    assert_eq!(v.old(8, 0.2 * f64::from(fade)).unwrap().0, FULL);
    // No fade: nothing is held.
    let mut v = VineFades::new(0.0);
    v.switch(9, FULL, BARE, 0.0);
    assert!(v.old(9, 0.0).is_none());
}
