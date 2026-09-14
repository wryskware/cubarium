/// The hull template, one character per cube pixel, exactly as `fable.js` has it: row 0 is
/// `dy = −4`, column 0 is `dx = −9`; `o` rim, `c` plate, `d` seam, `v` head plate, `f` fan,
/// `e` eye, `j` jaw, `L` lantern (the brow lamp is the `L` at column 13).
pub const ROWS: [&str; 8] = [
    "                  ",
    "             oooo ",
    "f   oooooooooLvvvo",
    "ff oLdLdLdLdLccecj",
    "fffocdcdcdcdccccjj",
    "ff  ooooooooooddj ",
    "f                 ",
    "                  ",
];
/// `ROWS[0]` sits at this body-local `y`.
pub const ROW_TOP: f64 = -4.0;
/// Column 0 sits at this body-local `x`.
pub const COL_LEFT: f64 = -9.0;
/// Template columns.
pub const COL_COUNT: usize = 18;

/// The hunt cycle, seconds; every sub-rhythm divides it, so `Hunt` at `t` and `t + 6` are
/// the same picture.
pub const HUNT_PERIOD: f64 = 6.0;
/// Coil begins.
pub const T_COIL: f64 = 3.1;
/// Forelimbs release.
pub const T_SNAP: f64 = 3.22;
/// Full extension.
pub const T_OPEN: f64 = 3.34;
/// Folded again: the articulated strike lasts `T_END − T_COIL` = 0.44 s.
pub const T_END: f64 = 3.54;
/// A blink, closed and open again, eased at both ends.
pub const BLINK_SECONDS: f64 = 0.28;
/// The strike accent on jaw and claw.
pub const ACCENT_SECONDS: f64 = 0.24;
/// Blink periods per mode (rest, move, bud); the hunt blinks once after its recoil.
pub const BLINK_PERIOD_REST: f64 = 4.7;
pub const BLINK_PERIOD_MOVE: f64 = 5.9;
pub const BLINK_PERIOD_BUD: f64 = 5.3;

/// Body-local footprint bound behind the anchor (the fan at `dx = −9` plus half a pixel).
pub const BOUND_BACK: f64 = 10.0;
/// Ahead of the anchor: the claw at 12.3 plus the head's lunge.
pub const BOUND_FRONT: f64 = 13.5;
/// Above the mid-line: the lantern glow two rows over a plate lifted by the wave.
pub const BOUND_ABOVE: f64 = 5.5;
/// Below: a leg's lower pixel on a plate pushed down by the wave.
pub const BOUND_BELOW: f64 = 5.5;
/// The largest extent any part may have from its own pivot, in sprite pixels.
pub const PART_EXTENT_MAX: f64 = 8.0;
/// The largest query radius the rig may need ([`cubarium_render::rig_radius`]): the claw at
/// `BOUND_FRONT` plus its part's support and the rig margin, rounded up.
pub const QUERY_RADIUS_MAX: f64 = 16.0;

/// The raised-cosine accent envelope of the study: a cosine ramp up over the first 40 % of
/// `u ∈ (0, 1)`, a plateau to 60 %, a cosine ramp down; 0 at and outside both ends, with
/// zero slope there. **Normative** — the blink, the strike accent and nothing else ride it.
pub fn envelope(u: f64) -> f64 {
    if !(u > 0.0 && u < 1.0) {
        return 0.0;
    }
    if u < 0.4 {
        0.5 - 0.5 * (std::f64::consts::PI * u / 0.4).cos()
    } else if u < 0.6 {
        1.0
    } else {
        0.5 - 0.5 * (std::f64::consts::PI * (1.0 - u) / 0.4).cos()
    }
}

/// Everything the hunt cycle drives at cycle phase `t_h ∈ [0, HUNT_PERIOD)`, exactly
/// `huntState` of the study: `reach` (< 0 cocked, 0 folded, 1 extended), `compress`,
/// `lunge`, `charge`, `accent` and `blink`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HuntState {
    pub reach: f64,
    pub compress: f64,
    pub lunge: f64,
    pub charge: f64,
    pub accent: f64,
    pub blink: f64,
}

/// **Normative**: the piecewise schedule of `fable.js` `huntState`, unrounded. Coil
/// (`T_COIL..T_SNAP`): `u = smoothstep`, `reach = −0.35u`, `compress = 1.7u`, `charge = u`.
/// Snap (`T_SNAP..T_OPEN`): `e = 1 − (1 − u)³`, `reach = −0.35 + 1.35e`, `compress = 1.7 −
/// 2.0e`, `lunge = 1.1e`. Recoil (`T_OPEN..T_END`): `u = smoothstep`, `reach = 1 − u`,
/// `compress = −0.3 + 0.3u`, `lunge = 1.1(1 − u)`. From `T_SNAP` on, `charge = exp(−(t_h −
/// T_SNAP) / 0.9) · smoothstep((HUNT_PERIOD − t_h) / 0.6)`. `accent = envelope((t_h −
/// (T_SNAP − 0.02)) / ACCENT_SECONDS)`, `blink = envelope((t_h − (T_END + 0.3)) /
/// BLINK_SECONDS)`. A non-finite `t_h` is the default (all zero).
pub fn hunt_state(t_h: f64) -> HuntState {
    if !t_h.is_finite() {
        return HuntState::default();
    }
    let mut s = HuntState::default();
    if t_h >= T_COIL && t_h < T_SNAP {
        let u = smoothstep((t_h - T_COIL) / (T_SNAP - T_COIL));
        s.reach = -0.35 * u;
        s.compress = 1.7 * u;
        s.charge = u;
    } else if t_h >= T_SNAP && t_h < T_OPEN {
        let u = (t_h - T_SNAP) / (T_OPEN - T_SNAP);
        // Fast release, soft arrival.
        let e = 1.0 - (1.0 - u) * (1.0 - u) * (1.0 - u);
        s.reach = -0.35 + 1.35 * e;
        s.compress = 1.7 - 2.0 * e;
        s.lunge = 1.1 * e;
    } else if t_h >= T_OPEN && t_h < T_END {
        let u = smoothstep((t_h - T_OPEN) / (T_END - T_OPEN));
        s.reach = 1.0 - u;
        s.compress = -0.3 + 0.3 * u;
        s.lunge = 1.1 * (1.0 - u);
    }
    if t_h >= T_SNAP {
        // The charge bleeds out of the chain and is forced to zero before the cycle wraps,
        // so the loop has no step.
        s.charge = (-(t_h - T_SNAP) / 0.9).exp() * smoothstep((HUNT_PERIOD - t_h) / 0.6);
    }
    s.accent = envelope((t_h - (T_SNAP - 0.02)) / ACCENT_SECONDS);
    s.blink = envelope((t_h - (T_END + 0.3)) / BLINK_SECONDS);
    s
}

/// The study's `smoothstep`: the Hermite polynomial on a clamped argument.
pub(super) fn smoothstep(u: f64) -> f64 {
    let c = if u.is_nan() { 0.0 } else { u.clamp(0.0, 1.0) };
    c * c * (3.0 - 2.0 * c)
}

/// A semantic channel read as a fraction of one: NaN is 0 and the range is clamped, so no
/// adapter mistake can make the body vanish or paint outside its footprint.
pub(super) fn unit(v: f64) -> f64 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }
}

/// The study's `frac`.
pub(super) fn frac(v: f64) -> f64 {
    v - v.floor()
}

/// The study's `mix` on sRGB triples: a clamped linear interpolation.
pub(super) fn mix3(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    let u = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    [
        a[0] + (b[0] - a[0]) * u,
        a[1] + (b[1] - a[1]) * u,
        a[2] + (b[2] - a[2]) * u,
    ]
}

/// Seconds since the most recent blink start, folded into [`envelope`].
pub(super) fn blink_closure(t: f64, period: f64) -> f64 {
    envelope((frac(t / period) * period) / BLINK_SECONDS)
}
