//! Paced plant and tall-column growth state.

use super::*;

// --- Paced growth --------------------------------------------------------------------
//
// The fields say what a cell warrants; these say how the picture gets there. A cell's
// visual walks one stage at a time, at [`STAGE_GROW_SECONDS`] up and
// [`STAGE_WILT_SECONDS`] down, and may turn round mid-step. Nothing here touches the
// simulation: the target is still exactly what [`next_stage`] decides.

/// The visual growth of one cell's plant slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Growth {
    /// The stage the visual has completed (`None` = bare).
    pub from: Option<u8>,
    /// The stage it is moving to; `== from` when idle.
    pub to: Option<u8>,
    /// Progress of `from → to` in [0, 1]; 1 when idle.
    pub g: f64,
    /// The resource-driven target with hysteresis (what the presenter's `stages` used to
    /// hold): what [`next_stage`] says the field warrants.
    pub target: Option<u8>,
    /// Fruit-accent blend in [0, 1].
    pub fruit: f64,
}

impl Growth {
    /// The growth of a slot that is already where the field wants it: no transition, and
    /// the fruit accent fully on iff a full-grown plant is in fruit.
    pub fn snapped(target: Option<u8>, in_fruit: bool) -> Growth {
        let fruit = if in_fruit && target == Some(2) {
            1.0
        } else {
            0.0
        };
        Growth {
            from: target,
            to: target,
            g: 1.0,
            target,
            fruit,
        }
    }
}

/// A stage as a rank: `None` is −1, `Some(s)` is `s`.
fn rank(stage: Option<u8>) -> i32 {
    stage.map_or(-1, i32::from)
}

/// The stage one rank step from `stage` in direction `dir`, clamped to the three stages.
fn stage_at(stage: Option<u8>, dir: i32) -> Option<u8> {
    match (rank(stage) + dir).clamp(-1, 2) {
        -1 => None,
        r => Some(r as u8),
    }
}

/// One tick of a cell's paced growth.
///
/// **Normative**, pure, and called once per simulation tick with `dt` the simulated
/// seconds the tick took (`dt = 0` is legal: only the retarget happens). With `rank(None)
/// = −1` and `rank(Some(s)) = s`:
///
/// 1. `target` is recorded. A non-finite or negative `dt` is 0.
/// 2. Idle (`from == to`) with `target != to` **and `dt > 0`**: one step starts, `to = from
///    ± 1` toward `target` and `g = 0` — one stage at a time, so `None → 2` runs `None → 0
///    → 1 → 2`. With `dt = 0` (a repeated observe of the same tick) nothing starts, so a
///    step that has just completed is not immediately followed by the next one's
///    bookkeeping on a call that represents no time.
/// 3. In flight (`from != to`) with `target != to`: if `target` lies on the `from` side of
///    `to` (including `target == from`) the step **reverses** — `from` and `to` swap and
///    `g` becomes `1 − g`, so a plant that starts wilting and is fed again grows back out
///    of exactly the pose it had reached. A target beyond `to` in the same direction
///    finishes this step first.
/// 4. `g` advances by `dt / `[`STAGE_GROW_SECONDS`] rising or `dt /
///    `[`STAGE_WILT_SECONDS`] falling, and the step completes (`from = to`) at 1. At most
///    one step starts per call.
/// 5. The fruit accent moves toward 1 over [`FRUIT_FADE_SECONDS`] while the plant is
///    full-grown (`from == to == Some(2)`) and in fruit, never overshooting, and is 0 the
///    moment either stops being true: it is the food signal and does not linger.
pub fn advance_growth(g: Growth, target: Option<u8>, in_fruit: bool, dt: f64) -> Growth {
    let dt = if dt.is_finite() && dt > 0.0 { dt } else { 0.0 };
    let mut g = Growth { target, ..g };
    if g.from == g.to {
        if target != g.to && dt > 0.0 {
            g.to = stage_at(g.from, (rank(target) - rank(g.from)).signum());
            g.g = 0.0;
        }
    } else if target != g.to
        && (rank(target) - rank(g.to)).signum() != (rank(g.to) - rank(g.from)).signum()
    {
        std::mem::swap(&mut g.from, &mut g.to);
        g.g = 1.0 - g.g;
    }
    if g.from != g.to {
        let seconds = if rank(g.to) > rank(g.from) {
            STAGE_GROW_SECONDS
        } else {
            STAGE_WILT_SECONDS
        };
        let step = if seconds > 0.0 { dt / seconds } else { 1.0 };
        g.g = (g.g + step).min(1.0);
        if g.g >= 1.0 {
            g.from = g.to;
            g.g = 1.0;
        }
    }
    g.fruit = if in_fruit && g.from == Some(2) && g.to == Some(2) {
        let step = if FRUIT_FADE_SECONDS > 0.0 {
            dt / FRUIT_FADE_SECONDS
        } else {
            1.0
        };
        (g.fruit + step).min(1.0)
    } else {
        0.0
    };
    g
}

/// The growth drawn a fraction `f` of a tick after `prev` (the state the previous
/// `observe` left) on the way to `cur` (the state this tick's `observe` left).
///
/// **Normative**: `f` is clamped to `[0, 1]` (non-finite → 0), `target` is `cur`'s and
/// `fruit` is the linear mix. The pair and progress follow the one thing that can have
/// happened in a tick: the same step in flight (`g` mixed linearly); a step that started
/// from idle (`g` from 0); a step that completed into idle (`g` to 1); a reversal (drawn in
/// `prev`'s orientation, `g` mixed toward `1 − cur.g`); a reversal that completed back
/// where it started (`g` toward 0). Anything else — a snap, a band change — is `cur`.
/// Every case is continuous at `f = 0` with `prev` and at `f = 1` with `cur`, so growth
/// drawn at 60 fps is as continuous as the sway.
pub fn growth_between(prev: Growth, cur: Growth, f: f64) -> Growth {
    let f = if f.is_finite() {
        f.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mix = |a: f64, b: f64| a + (b - a) * f;
    let fruit = mix(prev.fruit, cur.fruit);
    let prev_idle = prev.from == prev.to;
    let cur_idle = cur.from == cur.to;
    let (from, to, g) = if prev.from == cur.from && prev.to == cur.to {
        (cur.from, cur.to, mix(prev.g, cur.g))
    } else if prev_idle && !cur_idle && prev.to == cur.from {
        (cur.from, cur.to, mix(0.0, cur.g))
    } else if !prev_idle && cur_idle && cur.to == prev.to {
        (prev.from, prev.to, mix(prev.g, 1.0))
    } else if !prev_idle && !cur_idle && prev.from == cur.to && prev.to == cur.from {
        (prev.from, prev.to, mix(prev.g, 1.0 - cur.g))
    } else if !prev_idle && cur_idle && cur.to == prev.from {
        (prev.from, prev.to, mix(prev.g, 0.0))
    } else {
        (cur.from, cur.to, cur.g)
    };
    // A pair drawn at progress 1 is that pair's `to`, idle; keep the invariant so callers
    // can test `from == to` for idleness.
    if from != to && g >= 1.0 {
        return Growth {
            from: to,
            to,
            g: 1.0,
            target: cur.target,
            fruit,
        };
    }
    if from != to && g <= 0.0 {
        return Growth {
            from,
            to: from,
            g: 1.0,
            target: cur.target,
            fruit,
        };
    }
    Growth {
        from,
        to,
        g,
        target: cur.target,
        fruit,
    }
}

/// One stage step as the *drawing* rules see it: the two stages it runs between, ordered by
/// rank, and how far along the upper one it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrowthStep {
    /// The lower-ranked of the two stages; `None` is bare ground (the `None ↔ 0` step).
    pub lower: Option<u8>,
    /// The higher-ranked stage, which is always a real stage.
    pub upper: u8,
    /// The **upper stage's progress** in `[0, 1]`: 0 is the lower stage alone, 1 the upper
    /// stage alone, whichever way the step is travelling.
    pub t: f64,
}

/// The step a frame's [`Growth`] draws, or `None` for an idle growth (which draws one stage
/// whole).
///
/// **Normative**: with `rank(None) = −1`, the pair is ordered by rank — `(lower, upper) =
/// (from, to)` while rising and `(to, from)` while falling — and `t = g` rising, `1 − g`
/// falling. So `t` always runs from the lower stage to the upper one **whichever direction
/// the step is going**, and a reversal is drawn by exactly the same rule at exactly the same
/// `t`: a plant that starts to wilt and is fed again retraces the very pictures it came
/// through, in reverse, rather than restarting anything.
pub fn growth_step(growth: Growth) -> Option<GrowthStep> {
    if growth.from == growth.to {
        return None;
    }
    let rising = rank(growth.to) > rank(growth.from);
    let (lower, upper) = if rising {
        (growth.from, growth.to)
    } else {
        (growth.to, growth.from)
    };
    // The higher-ranked of two *different* stages is never bare ground; the `?` is a
    // safeguard, not a case.
    let upper = upper?;
    Some(GrowthStep {
        lower,
        upper,
        t: if rising { growth.g } else { 1.0 - growth.g },
    })
}

/// The share of an authored growth clip's progress ([`GrowthStep::t`]) spent blending into
/// the idle stage clip at each end of the step. Review-tunable.
///
/// A growth clip is baked from its plant's *neutral* pose, but the stage clips it grows out
/// of and into are sway loops running at the slot's own phase and, for a sprout, pulsing
/// their whole sprite. Cutting straight to and from the clip would therefore step the
/// brightness and the lean at both ends of every step. Blending over the first and last
/// `GROW_BLEND` of the progress — 0.48 s of the pilot's 4 s clip — hides both without
/// stretching the authored motion. 0 disables the blends (the clip alone, endpoint cuts
/// included); it must stay below 0.5 or the two blends would overlap.
pub const GROW_BLEND: f64 = 0.12;

/// The three layer weights an authored growth stamp carries at a step's progress `t`:
/// `[w_from, w_grow, w_to]` — the lower stage's idle sway clip, the growth clip, and the
/// upper stage's idle sway clip.
///
/// **Normative**, with `smoothstep` the clamped Hermite `t²(3 − 2t)`:
///
/// ```text
/// w_from = 1 − smoothstep(t / GROW_BLEND)                  (exactly 0 once t ≥ GROW_BLEND)
/// w_to   = smoothstep((t − (1 − GROW_BLEND)) / GROW_BLEND)  (exactly 0 until t > 1 − GROW_BLEND)
/// w_grow = 1 − w_from − w_to
/// ```
///
/// The three weights sum to 1, so the stamp is an exact lerp and a pixel opaque in every
/// layer stays opaque. Because [`GROW_BLEND`] is below ½ the two edge blends never overlap:
/// `w_from` and `w_to` are never both positive, `w_grow` is 1 through the middle of the
/// step, and each end of the step is the neighbouring idle clip *alone* — `t = 0` is the
/// lower stage's own idle image and `t = 1` the upper stage's, so a step neither enters nor
/// leaves with a cut. Both edges have zero slope in `t`. A `NaN` `t` reads as the lower
/// stage held still.
pub fn growth_weights(t: f64) -> [f32; 3] {
    let w_from = 1.0 - hermite(t / GROW_BLEND);
    let w_to = hermite((t - (1.0 - GROW_BLEND)) / GROW_BLEND);
    [w_from as f32, (1.0 - w_from - w_to) as f32, w_to as f32]
}

/// The visual height of one tall column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TallGrowth {
    /// Trunk segments shown, continuous, in `0..=`[`TALL_MAX_SEGMENTS`].
    pub height: f64,
    /// The segments the column's density warrants ([`next_tall`], with hysteresis).
    pub target: u8,
}

/// One tick of a column's paced height.
///
/// **Normative**: `height` moves toward `target` by `dt · `[`TALL_GROW_PX_PER_S`]` / 4`
/// rising and `dt · `[`TALL_WILT_PX_PER_S`]` / 4` falling (a segment is 4 px), never
/// overshooting. A non-finite or negative `dt` is 0; a non-finite height snaps.
pub fn advance_tall(g: TallGrowth, target: u8, dt: f64) -> TallGrowth {
    let dt = if dt.is_finite() && dt > 0.0 { dt } else { 0.0 };
    let want = f64::from(target);
    if !g.height.is_finite() {
        return TallGrowth {
            height: want,
            target,
        };
    }
    let rate = if want > g.height {
        TALL_GROW_PX_PER_S
    } else {
        TALL_WILT_PX_PER_S
    } / 4.0;
    let height = if want > g.height {
        (g.height + dt * rate).min(want)
    } else {
        (g.height - dt * rate).max(want)
    };
    TallGrowth { height, target }
}

/// The column height drawn a fraction `f` of a tick after `prev` on the way to `cur`: the
/// linear mix of the two heights (`f` clamped, non-finite → 0), with `cur`'s target.
pub fn tall_between(prev: TallGrowth, cur: TallGrowth, f: f64) -> TallGrowth {
    let f = if f.is_finite() {
        f.clamp(0.0, 1.0)
    } else {
        0.0
    };
    TallGrowth {
        height: prev.height + (cur.height - prev.height) * f,
        target: cur.target,
    }
}
