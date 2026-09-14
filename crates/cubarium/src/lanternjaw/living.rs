use cubarium_render::{Canvas, RigPart};
use cubarium_surface::{PixelImage, SurfacePoint, Vec2};

use super::envelopes::{
    ACCENT_SECONDS, BLINK_PERIOD_BUD, BLINK_PERIOD_MOVE, BLINK_PERIOD_REST, BLINK_SECONDS,
    HUNT_PERIOD, QUERY_RADIUS_MAX, envelope, hunt_state, smoothstep, unit,
};
use super::model::{Mode, Part};
use super::raster::{CELL_CENTRE, LIMB_STRIKE, Lanternjaw};

// ---------------------------------------------------------------------------------------
// The living body: semantic pose inputs, real attack phases and juvenile scale.
//
// `Mode` and `parts(seconds, mode)` are the study's gallery: a six-second loop that strikes
// every cycle and a `Bud` that always wears a cocoon. Ecology has none of that. The living
// entry points below take independent channels — ambient time for the rhythms, real
// movement, a real attack episode with its own elapsed time, real gut and funded escrow —
// and the study modes are re-expressed as one producer of the same explicit `Channels`, so
// the gallery, its palette and its silhouette tests stay exactly what they were.
// (Contract: `design/7_Research/lanternjaw-ecology-animation-contract-2026-09-13.md`.)
// ---------------------------------------------------------------------------------------

/// Smallest whole-rig scale the body is drawn at. The adult is 1. The mapping from core
/// structure to this scale is the core profile's (`body_scale = max(body_scale_min, (S /
/// S_adult)^body_scale_exponent)`, trial minimum 0.2 and exponent 0.5, so a valid world with
/// `child_structure_fraction = 0.1` yields `sqrt(0.1) = 0.316…` and the default 0.4 yields
/// 0.632…); the renderer admits exactly the core's minimum and scales *everything* by the
/// value it is handed (see [`Lanternjaw::draw_living`]), so the drawn claws and the core's
/// contact geometry can never separate. At 0.2 the body is under four pixels long and reads
/// as a violet-and-cyan smudge with a brighter head — coherent with the adult's proportions,
/// not legible as the adult's structure; that is the admitted range, not a rendering choice.
pub const SCALE_MIN: f64 = 0.2;
/// Largest admitted whole-rig scale: the adult.
pub const SCALE_MAX: f64 = 1.0;
/// Seconds the far forelimb lags the near one, in **attack** time (the study's 45 ms), never a
/// frame count and never a modulo of presentation time.
pub const FAR_LAG_SECONDS: f64 = 0.045;
/// Seconds of a real `Strike` spent on the cubic extension, at its **end**: the study's
/// 120 ms snap, placed so full contact is reached exactly at the phase's settlement boundary.
pub const EXTEND_SECONDS: f64 = 0.12;
/// Seconds of recoil from the displayed reach at the start of `Recovering` / `Handling`.
pub const RECOIL_SECONDS: f64 = 0.2;
/// Seconds after the recoil ends before the one post-strike blink starts.
pub const BLINK_AFTER_RECOIL: f64 = 0.3;
/// Seconds over which the hush of an attack releases into the ambient body during
/// `Handling` (the meal is shown by the gut breath, not by the arms).
pub const HANDLING_RELEASE_SECONDS: f64 = 1.0;
/// Time constant, seconds, of the lantern chain's charge bleeding out after a release.
pub const CHARGE_DECAY_SECONDS: f64 = 0.9;
/// Fraction of a real gestation over which the cocoon is revealed (its alpha rises with
/// `smoothstep(progress / COCOON_REVEAL)`); after that it breathes at full alpha.
pub const COCOON_REVEAL: f64 = 0.2;
/// The cocoon's breath period, seconds of ambient time (the study's 2.6 s).
pub const COCOON_BREATH_SECONDS: f64 = 2.6;
/// The gut breath: a slow vertical swell of the abdomen columns while gut material remains,
/// `GUT_BREATH_PX · gut · sin(2π t / GUT_BREATH_SECONDS)` added to the wave of template
/// columns 4..=8, in body pixels at gut fraction 1.
pub const GUT_BREATH_PX: f64 = 0.3;
pub const GUT_BREATH_SECONDS: f64 = 2.2;
/// How far an attack hushes the ambient wave: amplitude × `(1 − WAVE_HUSH · hush)` (the study's
/// hunt wave is 0.22 of rest's 0.55).
pub const WAVE_HUSH: f64 = 0.6;

/// Where the body touches the world, in **adult** body pixels at the named pose, for the core
/// adapter's contact geometry (`lanternjaw-ecology-animation-contract-2026-09-13.md`). A
/// study coordinate names a pixel, so these are painted texel *centres* (the `+0.5` of the
/// body lattice included), without the decorative wave. Multiply by the rig scale for a
/// juvenile ([`effectors`]). Not a claim about ecology: the core owns any contact tolerance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effectors {
    /// The ingestion mouth: the outer jaw column's centre. `(8.5, 0)` folded, `(9.6, 0)` at
    /// full lunge.
    pub mouth: Vec2,
    /// The near claw's centre at full extension: `(12.3 + head_dx + 0.5, 0.6 + 0.5)` with
    /// `head_dx = −0.3 · (4/17) + 1.1 · 0.5`, i.e. `(13.2794, 1.1)`.
    pub near_claw: Vec2,
    /// The far claw's centre once its 45 ms lag is over — **after** the settlement boundary,
    /// not at it (at the boundary the far claw is still 62 % through its own extension): the
    /// same as the near claw, one pixel higher. A contact check at settlement should use
    /// `near_claw`.
    pub far_claw: Vec2,
}

/// The adult effectors at full extension (`lunge = 1.1`, `compress = −0.3`, `reach = 1`),
/// scaled by `scale`. **Normative**: `mouth = scale · (8 + 1.1 + 0.5, 0)`, `near_claw = scale ·
/// (12.3 + head_dx + 0.5, 1.1)`, `far_claw = near_claw − scale · (0, 1)`, with
/// `head_dx = −0.3 · (1 − 13/17) + 1.1 · clamp((13 − 9) / 8, 0, 1)`.
pub fn effectors(scale: f64) -> Effectors {
    let head_dx = -0.3 * (1.0 - 13.0 / 17.0) + 1.1 * ((13.0 - 9.0) / 8.0f64).clamp(0.0, 1.0);
    let near = Vec2::new(
        (LIMB_STRIKE.1[0] + head_dx + CELL_CENTRE) * scale,
        (LIMB_STRIKE.1[1] + CELL_CENTRE) * scale,
    );
    Effectors {
        mouth: Vec2::new((8.0 + 1.1 + CELL_CENTRE) * scale, 0.0),
        near_claw: near,
        far_claw: Vec2::new(near.x, near.y - scale),
    }
}

/// The displayed extension state of the forelimbs and hull at one instant, carried from one
/// attack phase into the next so an interrupted motion continues from where it was drawn
/// rather than snapping to a study keyframe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reach {
    /// Near forelimb: `< 0` cocked, 0 folded, 1 fully extended.
    pub near: f64,
    /// Far forelimb, the same scale (it lags the near one by [`FAR_LAG_SECONDS`]).
    pub far: f64,
    /// Hull compression (the study's `compress`, 1.7 fully coiled, −0.3 at full lunge).
    pub compress: f64,
    /// Head lunge (the study's `lunge`, 1.1 at full extension).
    pub lunge: f64,
    /// Lantern chain charge, 0..1.
    pub charge: f64,
}

impl Reach {
    /// Arms folded, hull at rest, chain uncharged: the state of every phase-free body.
    pub const FOLDED: Reach = Reach {
        near: 0.0,
        far: 0.0,
        compress: 0.0,
        lunge: 0.0,
        charge: 0.0,
    };
    /// Full extension at settlement, the study's snap arrival.
    pub const EXTENDED: Reach = Reach {
        near: 1.0,
        far: 1.0,
        compress: -0.3,
        lunge: 1.1,
        charge: 1.0,
    };
}

/// The real hunting phases that move the body (the core's `Perched` and `Stalking` are
/// phase-free: no episode, arms folded, the ambient body driven by real movement).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttackPhase {
    /// Fold → cock over the whole real windup; no extension, no contact.
    Windup,
    /// Hold the cocked entry pose, then one cubic extension in the final
    /// [`EXTEND_SECONDS`], reaching full contact **at** the phase's settlement boundary and
    /// holding it past it.
    Strike,
    /// Recoil from the entry reach over [`RECOIL_SECONDS`], then folded stillness for the
    /// rest of the recovery. No repeat strike.
    Recovering,
    /// The successful-capture recoil, after which the hush releases and the ambient body
    /// resumes; the meal itself is the gut breath of [`LivingPose::gut`], never a flourish
    /// implied by the phase name.
    Handling,
}

/// One real attack episode as the adapter sees it at a presentation instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackEpisode {
    pub phase: AttackPhase,
    /// Seconds of **attack time** since this phase was entered (from the phase-entry tick and
    /// the presenter's fractional clock), never a modulo of anything. Negative reads as 0.
    pub elapsed: f64,
    /// The phase's real duration in seconds (the core profile's windup / strike length;
    /// for `Recovering` and `Handling` any positive value — only their recoil and release
    /// constants matter). Non-positive or non-finite reads as an instantaneous phase.
    pub duration: f64,
    /// The reach displayed at the instant this phase was entered — the previous episode's
    /// [`AttackChannels::reach`] at its final elapsed time, or [`Reach::FOLDED`] for an attack
    /// that starts from rest — so every phase continues from the picture the last one left.
    pub from: Reach,
}

/// What an attack episode contributes to the body at one instant.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AttackChannels {
    pub near_reach: f64,
    pub far_reach: f64,
    pub compress: f64,
    pub lunge: f64,
    pub charge: f64,
    /// The strike accent on jaw and claw, 0..1, bounded by [`envelope`].
    pub accent: f64,
    /// The one post-recoil blink, 0..1, added to the ambient closure.
    pub blink: f64,
    /// How far the ambient rhythms are hushed, 0..1: the wave amplitude scales by
    /// `1 − WAVE_HUSH · hush` and the lantern gain by `1 − hush · (0.62 − 0.92 · charge)`.
    pub hush: f64,
}

impl AttackChannels {
    /// The displayed state, to hand to the next episode as its `from`.
    pub fn reach(&self) -> Reach {
        Reach {
            near: self.near_reach,
            far: self.far_reach,
            compress: self.compress,
            lunge: self.lunge,
            charge: self.charge,
        }
    }
}

/// The attack channels of an episode at its `elapsed` time; `None` is a phase-free body
/// (all zero, [`Reach::FOLDED`]).
///
/// **Normative**, with `t = max(elapsed, 0)`, `D = duration` (non-positive or non-finite ⇒ 0),
/// `f = from`, `smoothstep` the Hermite polynomial on a clamped argument and `e(u) = 1 −
/// (1 − u)³`:
///
/// * `Windup`: `u = smoothstep(t / D)` (1 when `D = 0`); `near = f.near + (−0.35 − f.near)·u`,
///   `compress = f.compress + (1.7 − f.compress)·u`, `lunge = f.lunge·(1 − u)`, `charge =
///   f.charge + (1 − f.charge)·u`, `accent = blink = 0`, `hush = 1`.
/// * `Strike`: `E = min(EXTEND_SECONDS, D)`, `hold = D − E`. For `t < hold` the entry pose is
///   held: `near = f.near`, `compress = f.compress`, `lunge = f.lunge`, `charge = f.charge`.
///   For `t ≥ hold`: `u = clamp((t − hold) / E, 0, 1)` (1 when `E = 0`), `near = f.near + (1 −
///   f.near)·e(u)`, `compress = f.compress + (−0.3 − f.compress)·e(u)`, `lunge = f.lunge + (1.1
///   − f.lunge)·e(u)`, `charge = f.charge · exp(−(t − hold) / CHARGE_DECAY_SECONDS)`. In
///   **both** branches `accent = envelope((t − (hold − 0.02)) / ACCENT_SECONDS)`: the study's
///   20 ms pre-roll opens inside the hold, rising from exactly 0 (it is 0 for every `t ≤ hold −
///   0.02`, so a paid approach never accents) rather than entering with a step. Past `D` the
///   full pose is **held** (settlement), the accent finishing on its own. `blink = 0`, `hush =
///   1`.
/// * `Recovering`: `u = smoothstep(t / RECOIL_SECONDS)`; `near = f.near·(1 − u)`, `compress =
///   f.compress·(1 − u)`, `lunge = f.lunge·(1 − u)`, `charge = f.charge · exp(−t /
///   CHARGE_DECAY_SECONDS)`, `accent = 0`, `blink = envelope((t − (RECOIL_SECONDS +
///   BLINK_AFTER_RECOIL)) / BLINK_SECONDS)`, `hush = 1` throughout.
/// * `Handling`: as `Recovering`, except `hush = 1 − smoothstep((t − RECOIL_SECONDS) /
///   HANDLING_RELEASE_SECONDS)`.
/// * The far forelimb lags: `far_reach` is the `near` of the same rules evaluated at `t −
///   FAR_LAG_SECONDS` when that is ≥ 0, and `f.far` before that (the previous episode's
///   displayed far reach holds through the lag, so the far claw never jumps at a phase
///   boundary).
///
/// The study's `hunt_state` is this schedule with `Windup { D = T_SNAP − T_COIL, from:
/// FOLDED }`, `Strike { D = E = T_OPEN − T_SNAP, from: the windup's end }` and `Recovering
/// { from: the strike's end }`, so at the study's keyframes reach, compress, lunge, charge
/// and blink agree to 1e-9. The accent agrees only within the phase that owns it: cutting the
/// study's gesture into phases makes its 20 ms pre-roll unreachable where it falls before a
/// phase boundary (the study's own 120 ms windup ends at `T_SNAP`, so over `(T_SNAP − 0.02,
/// T_SNAP)` this schedule has no accent where `hunt_state` has up to 0.103), and `Recovering`
/// carries none where the study's plateau still shows at `T_OPEN`. A real windup's pre-roll
/// lives inside its own strike, so a real attack never loses it.
pub fn attack_channels(episode: Option<&AttackEpisode>) -> AttackChannels {
    let Some(episode) = episode else {
        return AttackChannels::default();
    };
    let t = attack_time(episode.elapsed);
    let mut channels = phase_channels(episode, t);
    // The far forelimb reads the same schedule 45 ms of attack time earlier; before the
    // episode began there is nothing to read, so the previous episode's displayed far reach
    // holds through the lag and the far claw never jumps at a phase boundary.
    let lagged = t - FAR_LAG_SECONDS;
    channels.far_reach = if lagged >= 0.0 {
        phase_channels(episode, lagged).near_reach
    } else {
        episode.from.far
    };
    channels
}

/// `max(elapsed, 0)` with a NaN read as 0 (an infinite elapsed is a held phase, which every
/// rule below saturates correctly).
fn attack_time(elapsed: f64) -> f64 {
    if elapsed.is_nan() {
        0.0
    } else {
        elapsed.max(0.0)
    }
}

/// One phase's channels at attack time `t`, everything but the far forelimb's lag.
fn phase_channels(episode: &AttackEpisode, t: f64) -> AttackChannels {
    let f = episode.from;
    let d = if episode.duration.is_finite() && episode.duration > 0.0 {
        episode.duration
    } else {
        0.0
    };
    let mut c = AttackChannels {
        far_reach: f.far,
        ..AttackChannels::default()
    };
    match episode.phase {
        AttackPhase::Windup => {
            // The study's 120 ms coil stretched over the whole real windup: no extension and
            // no contact, whatever the windup's length.
            let u = if d > 0.0 { smoothstep(t / d) } else { 1.0 };
            c.near_reach = f.near + (-0.35 - f.near) * u;
            c.compress = f.compress + (1.7 - f.compress) * u;
            c.lunge = f.lunge * (1.0 - u);
            c.charge = f.charge + (1.0 - f.charge) * u;
            c.hush = 1.0;
        }
        AttackPhase::Strike => {
            // Hold the entry pose through the paid approach, then one cubic extension in the
            // final `EXTEND_SECONDS`, arriving at full contact exactly at the settlement
            // boundary and holding it past that.
            let e_span = EXTEND_SECONDS.min(d);
            let hold = d - e_span;
            // The accent envelope opens its documented 20 ms *before* the extension, which
            // for a strike with a paid approach falls inside the hold — the study's own
            // pre-roll before `T_SNAP`. It is evaluated in both branches so it rises from 0
            // instead of entering at `envelope(0.02 / ACCENT_SECONDS) ≈ 0.103`; it is 0 for
            // every `t ≤ hold − 0.02`, so nothing accents during the approach proper.
            c.accent = envelope((t - (hold - 0.02)) / ACCENT_SECONDS);
            if t < hold {
                c.near_reach = f.near;
                c.compress = f.compress;
                c.lunge = f.lunge;
                c.charge = f.charge;
            } else {
                let u = if e_span > 0.0 {
                    ((t - hold) / e_span).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let e = 1.0 - (1.0 - u) * (1.0 - u) * (1.0 - u);
                c.near_reach = f.near + (1.0 - f.near) * e;
                c.compress = f.compress + (-0.3 - f.compress) * e;
                c.lunge = f.lunge + (1.1 - f.lunge) * e;
                c.charge = f.charge * (-(t - hold) / CHARGE_DECAY_SECONDS).exp();
            }
            c.hush = 1.0;
        }
        AttackPhase::Recovering | AttackPhase::Handling => {
            // Recoil from the reach actually displayed when the phase was entered, then
            // folded stillness: no repeat strike while the recovery runs out.
            let u = smoothstep(t / RECOIL_SECONDS);
            c.near_reach = f.near * (1.0 - u);
            c.compress = f.compress * (1.0 - u);
            c.lunge = f.lunge * (1.0 - u);
            c.charge = f.charge * (-t / CHARGE_DECAY_SECONDS).exp();
            c.blink = envelope((t - (RECOIL_SECONDS + BLINK_AFTER_RECOIL)) / BLINK_SECONDS);
            c.hush = if episode.phase == AttackPhase::Handling {
                // The meal is the gut breath, not the arms: the hush releases into the
                // ambient body once the capture recoil is over.
                1.0 - smoothstep((t - RECOIL_SECONDS) / HANDLING_RELEASE_SECONDS)
            } else {
                1.0
            };
        }
    }
    c
}

/// The semantic inputs of one frame of the living body. Every field is independent; nothing
/// here is a mode and nothing loops on its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LivingPose {
    /// Presentation seconds ([`crate::art_present::present_seconds`]) for the **ambient**
    /// rhythms only: the body wave, the lantern crest, the ambient blink, the gait phase, the
    /// cocoon and gut breaths. An attack never reads it.
    pub ambient: f64,
    /// 0 = perched (the rest wave, planted legs, the rest blink), 1 = full locomotion (the
    /// move wave, the gait, the move blink); in between a blend. The adapter derives it from
    /// real root movement (for example speed over the profile's maximum), so a valid target
    /// does not by itself move the legs.
    pub movement: f64,
    /// The real attack episode, if any.
    pub attack: Option<AttackEpisode>,
    /// Actual gut fill, 0..1 (`gut_material / gut_capacity`): the abdomen breathes by
    /// [`GUT_BREATH_PX`]` · gut`. 0 is no meal, whatever the phase says.
    pub gut: f64,
    /// Normalized gestation of a **funded** escrow, `Some(0..1)`; `None` means no cocoon at
    /// all, whatever the phase, satiety or reserve.
    pub cocoon: Option<f64>,
}

/// The explicit inputs of the rasterizer — everything [`Lanternjaw::parts`] used to compute
/// from `(seconds, mode)` before splatting, made data. Two producers build it:
/// [`Channels::study`] (the gallery modes, bit for bit what they always drew) and
/// [`Channels::living`] (the semantic pose). Public so tests can probe either producer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Channels {
    /// Ambient seconds for every rhythm below.
    pub t: f64,
    /// Two body waves `(amplitude px, period s)`, summed: template column `i` is lifted by
    /// `Σ amp · taper_i · sin(2π t / period + 0.4 i)`; an amplitude of 0 contributes 0 exactly.
    pub waves: [(f64, f64); 2],
    /// Two lantern crests `(weight, period s)`, summed: column `i`'s brightness is
    /// `clamp(pulse_gain · Σ weight · smoothstep(1 − d / 0.26), 0, 1.25)` with `u = fract(t /
    /// period + (17 − i)·0.06)`, `d = min(u, 1 − u)`; a weight of 0 contributes 0 exactly.
    pub pulses: [(f64, f64); 2],
    pub pulse_gain: f64,
    /// Two ambient blinks `(weight, period s)`, summed into the eye closure, plus
    /// `blink_extra`; the sum is clamped to `[0, 1]`.
    pub blinks: [(f64, f64); 2],
    pub blink_extra: f64,
    /// The leg cycle period in seconds (the study's 1.2); `gait_amount` scales the swing
    /// (`1.3 · sin(2π u)`) and the lift bump (`sin(π u / 0.38)` for `u < 0.38`). A `gait_amount`
    /// of 0 plants the legs whatever the period.
    pub gait: f64,
    pub gait_amount: f64,
    /// The forelimbs and hull, as [`AttackChannels`] carries them.
    pub reach: f64,
    pub far_reach: f64,
    pub compress: f64,
    pub lunge: f64,
    pub accent: f64,
    /// Tail columns (`i < 3`) lifted by this many pixels.
    pub tail_lift: f64,
    /// `Some(reveal)`: the cocoon's six cells at their study colours times `reveal`, breathing
    /// on `t` at [`COCOON_BREATH_SECONDS`]; `None` draws no cocoon.
    pub cocoon: Option<f64>,
    /// The gut breath amplitude factor, 0..1 (see [`GUT_BREATH_PX`]).
    pub gut: f64,
}

impl Channels {
    /// The study's four modes as channels: **exactly** the quantities [`Lanternjaw::parts`]
    /// computed for `(seconds, mode)` (a non-finite `seconds` reads as 0), so that
    /// [`Lanternjaw::rasterize`] of these is texel-identical to the gallery.
    ///
    /// **Normative.** `Rest`: waves `[(0.55, 5.5), (0, 1)]`, pulses `[(1, 3.0), (0, 1)]`, gain
    /// 1, blinks `[(1, BLINK_PERIOD_REST), (0, 1)]`, gait `(1.2, 0)`, reach/far/compress/lunge/
    /// accent 0, tail 0, cocoon `None`, gut 0. `Move`: waves `[(1.15, 2.4), (0, 1)]`, pulses
    /// `[(1, 2.3), (0, 1)]`, blinks `[(1, BLINK_PERIOD_MOVE), (0, 1)]`, gait `(1.2, 1)`. `Hunt`,
    /// with `h = hunt_state(t mod 6)` and `h′ = hunt_state((t − 0.045) mod 6)`: waves `[(0.22,
    /// 6.0), (0, 1)]`, pulses `[(1, 3.0), (0, 1)]`, gain `0.38 + 0.92 h.charge`, blinks `[(0, 1),
    /// (0, 1)]`, `blink_extra = h.blink`, reach `h.reach`, far `h′.reach`, compress `h.compress`,
    /// lunge `h.lunge`, accent `h.accent`. `Bud`: waves `[(0.4, 6.5), (0, 1)]`, pulses `[(1, 3.6),
    /// (0, 1)]`, gain 0.8, blinks `[(1, BLINK_PERIOD_BUD), (0, 1)]`, tail 1, cocoon `Some(1)`.
    pub fn study(seconds: f64, mode: Mode) -> Channels {
        let t = if seconds.is_finite() { seconds } else { 0.0 };
        // `Rest`, and the shared skeleton of the other three. The second wave, crest and
        // blink are at weight 0 in every mode, and a zero weight contributes exactly 0, so
        // the gallery is the single-wave, single-crest, single-blink body it always was.
        let rest = Channels {
            t,
            waves: [(0.55, 5.5), (0.0, 1.0)],
            pulses: [(1.0, 3.0), (0.0, 1.0)],
            pulse_gain: 1.0,
            blinks: [(1.0, BLINK_PERIOD_REST), (0.0, 1.0)],
            blink_extra: 0.0,
            gait: 1.2,
            gait_amount: 0.0,
            reach: 0.0,
            far_reach: 0.0,
            compress: 0.0,
            lunge: 0.0,
            accent: 0.0,
            tail_lift: 0.0,
            cocoon: None,
            gut: 0.0,
        };
        match mode {
            Mode::Rest => rest,
            Mode::Move => Channels {
                waves: [(1.15, 2.4), (0.0, 1.0)],
                pulses: [(1.0, 2.3), (0.0, 1.0)],
                blinks: [(1.0, BLINK_PERIOD_MOVE), (0.0, 1.0)],
                gait_amount: 1.0,
                ..rest
            },
            Mode::Hunt => {
                let h = hunt_state(t.rem_euclid(HUNT_PERIOD));
                // The study's far limb reads the cycle 45 ms earlier, modulo and all: that
                // synthetic loop is exactly what the gallery mode is, and why the living
                // body reads a real episode instead.
                let far = hunt_state((t - FAR_LAG_SECONDS).rem_euclid(HUNT_PERIOD));
                Channels {
                    waves: [(0.22, HUNT_PERIOD), (0.0, 1.0)],
                    pulses: [(1.0, 3.0), (0.0, 1.0)],
                    pulse_gain: 0.38 + 0.92 * h.charge,
                    blinks: [(0.0, 1.0), (0.0, 1.0)],
                    blink_extra: h.blink,
                    reach: h.reach,
                    far_reach: far.reach,
                    compress: h.compress,
                    lunge: h.lunge,
                    accent: h.accent,
                    ..rest
                }
            }
            Mode::Bud => Channels {
                waves: [(0.4, 6.5), (0.0, 1.0)],
                pulses: [(1.0, 3.6), (0.0, 1.0)],
                pulse_gain: 0.8,
                blinks: [(1.0, BLINK_PERIOD_BUD), (0.0, 1.0)],
                tail_lift: 1.0,
                cocoon: Some(1.0),
                ..rest
            },
        }
    }

    /// The semantic pose as channels.
    ///
    /// **Normative**, with `m = clamp(movement, 0, 1)` (NaN ⇒ 0), `a = attack_channels(pose.attack)`,
    /// `k = 1 − WAVE_HUSH · a.hush`: waves `[(0.55 (1 − m) k, 5.5), (1.15 m k, 2.4)]`, pulses
    /// `[(1 − m, 3.0), (m, 2.3)]`, gain `1 − a.hush · (0.62 − 0.92 a.charge)`, blinks `[(1 − m,
    /// BLINK_PERIOD_REST), (m, BLINK_PERIOD_MOVE)]`, `blink_extra = a.blink`, gait `(1.2, m)`,
    /// reach/far/compress/lunge/accent from `a`, cocoon `Some(smoothstep(p / COCOON_REVEAL))`
    /// for `Some(p)` (NaN ⇒ 0) and `None` otherwise, `tail_lift` = that reveal (0 without a
    /// cocoon), `gut = clamp(gut, 0, 1)` (NaN ⇒ 0), `t = ambient` (non-finite ⇒ 0).
    ///
    /// Consequences the tests hold it to: `movement = 0` with no attack, no gut and no cocoon
    /// is texel-identical to `Mode::Rest`; `movement = 1` likewise to `Mode::Move`; a held
    /// phase never strikes on its own; no escrow, no cocoon; no gut, no breath.
    pub fn living(pose: &LivingPose) -> Channels {
        let m = unit(pose.movement);
        let a = attack_channels(pose.attack.as_ref());
        // An attack hushes the ambient wave and re-gains the chain; at full hush the two
        // reproduce the study's hunt figures (0.55 · 0.4 = 0.22, and 1 − (0.62 − 0.92 c) =
        // 0.38 + 0.92 c).
        let k = 1.0 - WAVE_HUSH * a.hush;
        let reveal = pose.cocoon.map(|p| smoothstep(p / COCOON_REVEAL));
        Channels {
            t: if pose.ambient.is_finite() {
                pose.ambient
            } else {
                0.0
            },
            waves: [(0.55 * (1.0 - m) * k, 5.5), (1.15 * m * k, 2.4)],
            pulses: [(1.0 - m, 3.0), (m, 2.3)],
            pulse_gain: 1.0 - a.hush * (0.62 - 0.92 * a.charge),
            blinks: [(1.0 - m, BLINK_PERIOD_REST), (m, BLINK_PERIOD_MOVE)],
            blink_extra: a.blink,
            gait: 1.2,
            gait_amount: m,
            reach: a.near_reach,
            far_reach: a.far_reach,
            compress: a.compress,
            lunge: a.lunge,
            accent: a.accent,
            // The tail lifts to clear the cocoon exactly as far as the cocoon is revealed.
            tail_lift: reveal.unwrap_or(0.0),
            cocoon: reveal,
            gut: unit(pose.gut),
        }
    }
}

impl Lanternjaw {
    /// The living body's parts: `rasterize(&Channels::living(pose), out)`.
    pub fn parts_living(&self, pose: &LivingPose, out: &mut Vec<Part>) {
        self.rasterize(&Channels::living(pose), out);
    }

    /// Draw the living body at whole-rig `scale` ([`SCALE_MIN`]`..=`[`SCALE_MAX`]; anything else,
    /// or a non-finite scale, is a configuration error that panics in every build — the
    /// adapter clamps its mapping to the admitted range): [`Lanternjaw::parts_living`] then one
    /// [`cubarium_render::stamp_rig_scaled`]`(canvas, anchor, heading, &[(rig_parts, 1.0)],
    /// scale, opacity, scratch)`. Everything scales together: offsets, pivots, the lattice, the
    /// lunge, the limbs and the query radius, so `effectors(scale)` names where the scaled
    /// claws and jaw are drawn.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_living(
        &self,
        canvas: &mut Canvas,
        anchor: SurfacePoint,
        heading: Vec2,
        pose: &LivingPose,
        scale: f64,
        opacity: f32,
        parts: &mut Vec<Part>,
        scratch: &mut Vec<PixelImage>,
    ) {
        assert!(
            scale.is_finite() && (SCALE_MIN..=SCALE_MAX).contains(&scale),
            "Lanternjaw scale {scale} is outside the admitted {SCALE_MIN}..={SCALE_MAX}"
        );
        self.parts_living(pose, parts);
        let rig: Vec<RigPart<'_>> = parts.iter().map(Part::rig_part).collect();
        debug_assert!(
            cubarium_render::rig_radius(&[(&rig, 1.0)]) <= QUERY_RADIUS_MAX,
            "the rig needs a query radius past QUERY_RADIUS_MAX"
        );
        cubarium_render::stamp_rig_scaled(
            canvas,
            anchor,
            heading,
            &[(&rig, 1.0)],
            scale,
            opacity,
            scratch,
        );
    }
}
