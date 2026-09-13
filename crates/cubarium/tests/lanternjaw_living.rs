//! Independent tests for the **living** Lanternjaw — the semantic pose entry points of
//! `cubarium::lanternjaw` — written from the public doc comments of that module's "The living
//! body" section (`SCALE_MIN`, `SCALE_MAX`, `FAR_LAG_SECONDS`, `EXTEND_SECONDS`,
//! `RECOIL_SECONDS`, `BLINK_AFTER_RECOIL`, `HANDLING_RELEASE_SECONDS`, `CHARGE_DECAY_SECONDS`,
//! `COCOON_REVEAL`, `COCOON_BREATH_SECONDS`, `GUT_BREATH_PX`, `GUT_BREATH_SECONDS`, `WAVE_HUSH`,
//! `Effectors`/`effectors`, `Reach`, `AttackPhase`, `AttackEpisode`, `AttackChannels` and its
//! `reach()`, the normative schedule of `attack_channels`, `LivingPose`, `Channels` with
//! `Channels::{study, living}`, and `Lanternjaw::{rasterize, parts_living, draw_living}`), of the
//! gallery API they re-express (`parts`, `draw`, `hunt_state`, `envelope`, `Mode`, `PartName`,
//! `Part`, the `BOUND_*` constants), of `cubarium_render::{stamp_rig, stamp_rig_scaled,
//! rig_radius, RigPart}` and of `Sprite::{texel, pivot, extent}` — never from their bodies — and
//! from Astra's contract, `design/7_Research/lanternjaw-ecology-animation-contract-2026-09-13.md`
//! ("Real phases, not the six-second demonstration loop", "Juveniles and escrow", "Seam/rim
//! contact must agree with the root-owned artwork").
//!
//! Every expectation is either recomputed here from the normative formulas (the windup's cocked
//! end, the strike's hold and its cubic arrival, the recoil's smoothstep, the accent and blink
//! envelopes, the 45 ms far lag, the cocoon's `smoothstep(p / COCOON_REVEAL)` reveal, the scaled
//! footprint), built a second independent way through the public API (the gallery's own
//! `parts(seconds, mode)` as the reference image for a living pose; a hand-written `Channels`
//! literal as the reference for `Channels::study`; `stamp_rig` of `parts_living` as the reference
//! for `draw_living` at scale 1; the same scaled body thirty rows up the face as the reference
//! for the rim cut), or a property the contract says a wrong implementation would break:
//!
//! * a held `Windup` or `Strike` that replays the study's six-second gesture — "hold each real
//!   phase for longer than six seconds: no synthetic attacks", and the claws must not retract
//!   half a second before the core can capture anything;
//! * an interrupted phase that snaps to a study keyframe instead of continuing from its
//!   displayed reach;
//! * a cocoon that follows a phase or satiety rather than a funded escrow;
//! * a gut breath invented from the phase name ("gut-zero cannot pretend to chew a meal");
//! * an attack advanced by ambient presentation time (or ambient rhythms frozen by a synthetic
//!   hunt timestamp);
//! * a juvenile whose head and arms stay at adult distances, or whose scaled body reflects at the
//!   open rim instead of being cut.
//!
//! The renderer's own scaling rules are tested against synthetic fixtures in
//! `crates/cubarium-render/tests/multipart_scale.rs`; the gallery's palette, footprint and timing
//! live in `crates/cubarium/tests/lanternjaw.rs`. Fixtures are copied from those suites rather
//! than imported, since integration tests are separate crates.
//!
//! **Two places where the doc's own claims do not hold as written**, tested as far as they do and
//! reported rather than papered over:
//!
//! * `attack_channels`' closing sentence says the study's `hunt_state` "is this schedule" with the
//!   three named episodes, "so at the study's keyframes the two agree". That is true of
//!   `near`/`far`/`compress`/`lunge`/`charge`/`blink` at every keyframe, but **not of `accent`
//!   across the Strike → Recovering boundary**: the study's accent envelope is still running down
//!   at `T_OPEN` (it peaks there and ends at `T_SNAP − 0.02 + ACCENT_SECONDS` = 3.44) while
//!   `Recovering` is specified with `accent = 0`. The keyframe table below therefore compares the
//!   accent against the phase that owns it (see `ACCENT_OWNED`).
//! * the `Strike` accent is specified twice over, and the two clauses do not meet: `accent = 0`
//!   is listed among the channels held "for `t < hold`", while the `t ≥ hold` branch gives
//!   `envelope((t − (hold − 0.02)) / ACCENT_SECONDS)` — an envelope whose `− 0.02` pre-roll can
//!   only be drawn *during* the hold. Gated on `t ≥ hold`, the accent would **switch on** at
//!   `envelope(0.02 / 0.24) = 0.1033` in one frame, which is precisely what `envelope`'s zero
//!   slope at both ends exists to prevent ("nothing in this body is allowed to switch on for a
//!   single frame"), and the work order's own boundaries — accent "0 at `t ≤ 0.85`" and "> 0
//!   somewhere in `(0.86, 1.1)`", i.e. from `hold − 0.02` on — assume it is not gated. So the
//!   **continuous** reading is what is asserted here: the accent is exactly 0 for every `t ≤ hold
//!   − 0.02`, so a paid approach still never accents, and from there it rises through the
//!   envelope. `attack_channels`' own doc now records the same reading, awaiting Fable's ruling
//!   on the wording; the literal reading is testable by flipping one `if` in
//!   `a_real_strike_extends_only_in_its_final_hundred_and_twenty_milliseconds_and_then_holds`.
//! * "`movement: 0` … is texel-identical to `Mode::Rest`" requires the zero-weight second wave,
//!   pulse and blink of `Channels::living` to contribute *exactly* zero even though their periods
//!   differ from the study's dummy `(0, 1)` entries. `Channels` documents exactly that ("an
//!   amplitude of 0 contributes 0 exactly", "a weight of 0 contributes 0 exactly"), so the
//!   identity is asserted texel for texel.

use std::panic::AssertUnwindSafe;

use cubarium::lanternjaw::*;
use cubarium_render::{
    Canvas, RigPart, SUPERSAMPLE_REACH, Sprite, rig_radius, stamp_rig, stamp_rig_scaled,
};
use cubarium_surface::{SurfacePoint, Vec2};
use cube_proto::Face;

// ---------------------------------------------------------------------------
// sampling and canvas helpers (copied from tests/lanternjaw.rs)
// ---------------------------------------------------------------------------

const FPS: usize = 60;

/// Thirteen instants, most of them fractional, spanning more than two hunt cycles and covering
/// every one of the study's keyframes: the sweep every "identical to the gallery" claim uses.
const INSTANTS: [f64; 13] = [
    0.0,
    1.0 / 60.0,
    0.25,
    0.7333333333333333,
    1.5,
    2.4,
    3.1,
    3.22,
    3.28,
    3.34,
    3.54,
    5.9166666666666665,
    11.99,
];

fn every_pixel() -> impl Iterator<Item = (Face, u8, u8)> {
    Face::ALL
        .into_iter()
        .flat_map(|face| (0..64u8).flat_map(move |y| (0..64u8).map(move |x| (face, x, y))))
}

fn assert_identical(a: &Canvas, b: &Canvas, what: &str) {
    if let Some((f, x, y)) = every_pixel().find(|&(f, x, y)| a.get(f, x, y) != b.get(f, x, y)) {
        panic!(
            "{what}: ({f:?}, {x}, {y}) is {:?} vs {:?}",
            a.get(f, x, y),
            b.get(f, x, y)
        );
    }
}

fn max_diff(a: &Canvas, b: &Canvas) -> f32 {
    every_pixel()
        .flat_map(|(f, x, y)| {
            let (p, q) = (a.get(f, x, y), b.get(f, x, y));
            (0..3).map(move |c| (p[c] - q[c]).abs())
        })
        .fold(0.0, f32::max)
}

fn total_light(image: &Canvas) -> f64 {
    every_pixel()
        .flat_map(|(f, x, y)| image.get(f, x, y))
        .map(f64::from)
        .sum()
}

fn lit_faces(image: &Canvas) -> usize {
    Face::ALL
        .into_iter()
        .filter(|&f| (0..64u8).any(|y| (0..64u8).any(|x| image.get(f, x, y) != [0.0; 3])))
        .count()
}

fn filled(rgb: [f32; 3]) -> Canvas {
    let mut canvas = Canvas::new();
    for (f, x, y) in every_pixel() {
        canvas.set(f, x, y, rgb);
    }
    canvas
}

/// A mid-face anchor on a pixel centre, so the body lattice lands on the face lattice and every
/// whole-pixel comparison below is exact rather than approximate.
fn mid() -> SurfacePoint {
    SurfacePoint::pixel_center(Face::Front, 32, 32)
}

fn forward() -> Vec2 {
    Vec2::new(1.0, 0.0)
}

fn study(seconds: f64, mode: Mode) -> Vec<Part> {
    let mut out = Vec::new();
    Lanternjaw::new().parts(seconds, mode, &mut out);
    out
}

fn living(pose: &LivingPose) -> Vec<Part> {
    let mut out = Vec::new();
    Lanternjaw::new().parts_living(pose, &mut out);
    out
}

fn drawn_living(pose: &LivingPose, anchor: SurfacePoint, heading: Vec2, scale: f64) -> Canvas {
    let mut canvas = Canvas::new();
    Lanternjaw::new().draw_living(
        &mut canvas,
        anchor,
        heading,
        pose,
        scale,
        1.0,
        &mut Vec::new(),
        &mut Vec::new(),
    );
    canvas
}

/// The chosen parts of one living frame, assembled into one scaled `stamp_rig_scaled` call at
/// their own layers: `parts_living` and `Part::rig_part` are public, so a sub-rig of the real
/// frame is a legitimate second path.
fn stamp_subset(
    parts: &[Part],
    keep: &[PartName],
    anchor: SurfacePoint,
    heading: Vec2,
    scale: f64,
) -> Canvas {
    let rig: Vec<RigPart<'_>> = parts
        .iter()
        .filter(|p| keep.contains(&p.name))
        .map(Part::rig_part)
        .collect();
    let mut canvas = Canvas::new();
    stamp_rig_scaled(
        &mut canvas,
        anchor,
        heading,
        &[(&rig[..], 1.0)],
        scale,
        1.0,
        &mut Vec::new(),
    );
    canvas
}

fn part_of<'a>(parts: &'a [Part], name: PartName) -> &'a Part {
    parts
        .iter()
        .find(|p| p.name == name)
        .expect("every frame carries every part")
}

/// The body-local centre of texel `(tx, ty)` of `part`: `offset + (texel centre − pivot)`, the
/// footprint contract's own expression.
fn texel_body(part: &Part, tx: usize, ty: usize) -> Vec2 {
    part.offset
        + Vec2::new(
            tx as f64 + 0.5 - part.sprite.pivot().x,
            ty as f64 + 0.5 - part.sprite.pivot().y,
        )
}

fn painted(sprite: &Sprite) -> impl Iterator<Item = (usize, usize)> + '_ {
    (0..sprite.height())
        .flat_map(move |ty| (0..sprite.width()).map(move |tx| (tx, ty)))
        .filter(move |&(tx, ty)| sprite.texel(tx as i32, ty as i32)[3] > 0.0)
}

fn assert_parts_identical(a: &[Part], b: &[Part], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: part counts differ");
    for (p, q) in a.iter().zip(b) {
        assert_part_identical(p, q, what);
    }
}

fn assert_part_identical(p: &Part, q: &Part, what: &str) {
    assert_eq!(p.name, q.name, "{what}");
    assert_eq!(p.offset, q.offset, "{what}: {:?}'s offset", p.name);
    assert_eq!(
        p.sprite.pivot(),
        q.sprite.pivot(),
        "{what}: {:?}'s pivot",
        p.name
    );
    assert_eq!(
        (p.sprite.width(), p.sprite.height()),
        (q.sprite.width(), q.sprite.height()),
        "{what}: {:?}'s image size",
        p.name
    );
    for ty in 0..p.sprite.height() {
        for tx in 0..p.sprite.width() {
            assert_eq!(
                p.sprite.texel(tx as i32, ty as i32),
                q.sprite.texel(tx as i32, ty as i32),
                "{what}: {:?} texel ({tx}, {ty})",
                p.name
            );
        }
    }
}

fn parts_differ(a: &[Part], b: &[Part], name: PartName) -> bool {
    let (p, q) = (part_of(a, name), part_of(b, name));
    if p.offset != q.offset || p.sprite.pivot() != q.sprite.pivot() {
        return true;
    }
    (0..p.sprite.height()).any(|ty| {
        (0..p.sprite.width())
            .any(|tx| p.sprite.texel(tx as i32, ty as i32) != q.sprite.texel(tx as i32, ty as i32))
    })
}

// ---------------------------------------------------------------------------
// the normative formulas, recomputed here from the doc comments
// ---------------------------------------------------------------------------

/// The study's `smoothstep`: the Hermite polynomial on a clamped argument.
fn smoothstep(u: f64) -> f64 {
    let c = if u.is_nan() { 0.0 } else { u.clamp(0.0, 1.0) };
    c * c * (3.0 - 2.0 * c)
}

/// The raised-cosine envelope, recomputed from `envelope`'s doc comment.
fn envelope_reference(u: f64) -> f64 {
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

/// `Channels::study`'s normative mapping, transcribed term by term from its doc comment.
fn study_channels_reference(seconds: f64, mode: Mode) -> Channels {
    let t = if seconds.is_finite() { seconds } else { 0.0 };
    let mut ch = Channels {
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
        Mode::Rest => {}
        Mode::Move => {
            ch.waves = [(1.15, 2.4), (0.0, 1.0)];
            ch.pulses = [(1.0, 2.3), (0.0, 1.0)];
            ch.blinks = [(1.0, BLINK_PERIOD_MOVE), (0.0, 1.0)];
            ch.gait_amount = 1.0;
        }
        Mode::Hunt => {
            let h = hunt_state(t.rem_euclid(HUNT_PERIOD));
            let far = hunt_state((t - FAR_LAG_SECONDS).rem_euclid(HUNT_PERIOD));
            ch.waves = [(0.22, 6.0), (0.0, 1.0)];
            ch.pulses = [(1.0, 3.0), (0.0, 1.0)];
            ch.pulse_gain = 0.38 + 0.92 * h.charge;
            ch.blinks = [(0.0, 1.0), (0.0, 1.0)];
            ch.blink_extra = h.blink;
            ch.reach = h.reach;
            ch.far_reach = far.reach;
            ch.compress = h.compress;
            ch.lunge = h.lunge;
            ch.accent = h.accent;
        }
        Mode::Bud => {
            ch.waves = [(0.4, 6.5), (0.0, 1.0)];
            ch.pulses = [(1.0, 3.6), (0.0, 1.0)];
            ch.pulse_gain = 0.8;
            ch.blinks = [(1.0, BLINK_PERIOD_BUD), (0.0, 1.0)];
            ch.tail_lift = 1.0;
            ch.cocoon = Some(1.0);
        }
    }
    ch
}

// ---------------------------------------------------------------------------
// living-pose fixtures
// ---------------------------------------------------------------------------

/// A phase-free body: no episode, no meal, no escrow.
fn quiet(ambient: f64, movement: f64) -> LivingPose {
    LivingPose {
        ambient,
        movement,
        attack: None,
        gut: 0.0,
        cocoon: None,
    }
}

fn episode(phase: AttackPhase, elapsed: f64, duration: f64, from: Reach) -> AttackEpisode {
    AttackEpisode {
        phase,
        elapsed,
        duration,
        from,
    }
}

/// The channels of `ep` re-timed to `elapsed`: every timing test below sweeps one episode this
/// way, which is also the contract's own model — an episode is a phase plus its elapsed time, not
/// a running animation.
fn at(ep: &AttackEpisode, elapsed: f64) -> AttackChannels {
    attack_channels(Some(&AttackEpisode { elapsed, ..*ep }))
}

/// The candidate core schedule of the contract's timing recommendation: a 0.6 s windup, a 1 s
/// strike, one contact check at the strike's end.
const WINDUP_SECONDS: f64 = 0.6;
const STRIKE_SECONDS: f64 = 1.0;

/// The cocked reach a real 0.6 s windup leaves behind — `Reach { near: −0.35, compress: 1.7,
/// lunge: 0, charge: 1 }` by the `Windup` rules at `u = 1`, read through the public API so the
/// `from` handed to the strike is the picture the windup actually drew.
fn cocked() -> Reach {
    let windup = episode(
        AttackPhase::Windup,
        WINDUP_SECONDS,
        WINDUP_SECONDS,
        Reach::FOLDED,
    );
    attack_channels(Some(&windup)).reach()
}

fn strike_episode(elapsed: f64) -> AttackEpisode {
    episode(AttackPhase::Strike, elapsed, STRIKE_SECONDS, cocked())
}

/// A body holding full extension past its settlement boundary: the pose the contract's contact
/// check is evaluated at.
fn settled(ambient: f64) -> LivingPose {
    LivingPose {
        ambient,
        movement: 0.0,
        attack: Some(strike_episode(STRIKE_SECONDS + 0.2)),
        gut: 0.0,
        cocoon: None,
    }
}

/// The frontmost painted body-`x` of the near forelimb, in body pixels: `max x` over the
/// `NearLimb` texels carrying real coverage (`α > 0.2`, so a filter tail is not mistaken for a
/// claw), each texel's body centre computed from `offset + texel centre − pivot`.
fn near_limb_front(parts: &[Part]) -> f64 {
    let part = part_of(parts, PartName::NearLimb);
    let mut front = f64::NEG_INFINITY;
    for (tx, ty) in painted(&part.sprite) {
        if part.sprite.texel(tx as i32, ty as i32)[3] > 0.2 {
            front = front.max(texel_body(part, tx, ty).x);
        }
    }
    assert!(front.is_finite(), "the near limb painted nothing at all");
    front
}

// ---------------------------------------------------------------------------
// 1. the gallery is a producer of channels, and nothing else changed
// ---------------------------------------------------------------------------

/// `Channels::study` is "**exactly** the quantities `Lanternjaw::parts` computed for `(seconds,
/// mode)`", and its doc gives every one of them per mode. The reference here is a hand-written
/// `Channels` literal (`study_channels_reference`), transcribed from that list, so the test is a
/// direct reading of the normative mapping rather than a re-derivation of the study.
///
/// The `Hunt` row is the interesting one: its `far_reach` must be `hunt_state` at `t −
/// FAR_LAG_SECONDS` *modulo the cycle* (the gallery's own 45 ms lag) and its `blink_extra`, not
/// its ambient blinks, must carry the post-recoil blink.
#[test]
fn study_channels_are_the_documented_mapping_of_every_mode() {
    for mode in Mode::ALL {
        for &t in &INSTANTS {
            assert_eq!(
                Channels::study(t, mode),
                study_channels_reference(t, mode),
                "{mode:?} at {t}"
            );
        }
        // "A non-finite `seconds` reads as 0."
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                Channels::study(bad, mode),
                Channels::study(0.0, mode),
                "{mode:?}: a seconds of {bad} must read as 0"
            );
        }
    }
}

/// "**`parts(seconds, mode)` is `rasterize(&Channels::study(seconds, mode), out)`, bit for
/// bit**." Thirteen instants — fractional ones, the four study keyframes and two cycles apart —
/// in all four modes, compared part by part: name, offset, pivot, image size and every texel.
///
/// This is what keeps the gallery's palette, silhouette and footprint suites valid after the
/// refactor: if the channel path lost the tail lift, the cocoon, the gait or the far limb's lag,
/// one of these 52 frames differs.
#[test]
fn parts_is_rasterize_of_the_study_channels() {
    let rig = Lanternjaw::new();
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for mode in Mode::ALL {
        for &t in &INSTANTS {
            rig.parts(t, mode, &mut a);
            rig.rasterize(&Channels::study(t, mode), &mut b);
            assert_parts_identical(
                &a,
                &b,
                &format!("{mode:?} at {t}: parts vs rasterize(study)"),
            );
            assert_eq!(
                a.len(),
                8,
                "{mode:?} at {t}: the eight parts of PartName::ALL"
            );
        }
    }
    // And `Bud`'s cocoon really is the channel path's cocoon: the hand-written Bud channels
    // (waves (0.4, 6.5), pulses (1, 3.6) at gain 0.8, blink 5.3, tail 1, cocoon Some(1)) must
    // rasterize to the gallery's `Bud` frame, which is the only test of `rasterize`'s cocoon and
    // tail-lift paths that does not go through `Channels::study` at all.
    for &t in &INSTANTS {
        rig.parts(t, Mode::Bud, &mut a);
        rig.rasterize(&study_channels_reference(t, Mode::Bud), &mut b);
        assert_parts_identical(&a, &b, &format!("Bud at {t}: hand-built channels"));
    }
}

/// "`movement = 0` with no attack, no gut and no cocoon is texel-identical to `Mode::Rest`;
/// `movement = 1` likewise to `Mode::Move`." Thirteen instants, most of them fractional.
///
/// The identity is not free: `Channels::living` blends *two* waves, two pulses and two blinks, so
/// at `movement = 0` the move wave (period 2.4), the move pulse (2.3) and the move blink (5.9)
/// must contribute **exactly** zero at weight zero, and at `movement = 1` the rest ones must. A
/// blend that multiplied a non-zero sine by a zero amplitude and then rounded, or that used
/// `min`/`max` instead of a weight, would show up here as a one-ulp texel difference.
#[test]
fn the_living_body_without_a_phase_is_the_study_gallery() {
    let rig = Lanternjaw::new();
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for (movement, mode) in [(0.0, Mode::Rest), (1.0, Mode::Move)] {
        for &t in &INSTANTS {
            let pose = quiet(t, movement);
            rig.parts_living(&pose, &mut a);
            rig.parts(t, mode, &mut b);
            assert_parts_identical(
                &a,
                &b,
                &format!("movement {movement} at ambient {t} vs {mode:?}"),
            );
        }
    }
    // A non-finite ambient instant reads as 0, as the study's does.
    for bad in [f64::NAN, f64::INFINITY] {
        rig.parts_living(&quiet(bad, 0.0), &mut a);
        rig.parts(0.0, Mode::Rest, &mut b);
        assert_parts_identical(&a, &b, &format!("an ambient of {bad} must read as 0"));
    }
}

// ---------------------------------------------------------------------------
// 2. the attack schedule
// ---------------------------------------------------------------------------

/// "`None` is a phase-free body (all zero, `Reach::FOLDED`)" — the state of a `Perched` or
/// `Stalking` hunter, and the `from` of an attack that starts from rest.
#[test]
fn no_episode_is_all_zero_and_folded() {
    let a = attack_channels(None);
    assert_eq!(a, AttackChannels::default());
    assert_eq!(a.near_reach, 0.0);
    assert_eq!(a.far_reach, 0.0);
    assert_eq!(a.compress, 0.0);
    assert_eq!(a.lunge, 0.0);
    assert_eq!(a.charge, 0.0);
    assert_eq!(a.accent, 0.0);
    assert_eq!(a.blink, 0.0);
    assert_eq!(a.hush, 0.0, "a phase-free body is not hushed");
    assert_eq!(a.reach(), Reach::FOLDED);
}

/// "The study's `hunt_state` is this schedule with `Windup { D = T_SNAP − T_COIL, from: FOLDED }`,
/// `Strike { D = E = T_OPEN − T_SNAP, from: the windup's end }` and `Recovering { from: the
/// strike's end }`, so at the study's keyframes the two agree."
///
/// The three episodes are built exactly that way — each one's `from` is the *previous* episode's
/// `AttackChannels::reach()` at its final elapsed time, which is also the interruption-continuity
/// rule — and every channel is compared with `hunt_state` at seven instants: mid-coil, the
/// release boundary from both sides, mid-snap, the settlement boundary from both sides, mid-recoil,
/// the recoil's end, and the post-recoil blink's peak.
///
/// `accent` is compared against the phase that **owns** it (`ACCENT_OWNED` below): at `T_OPEN` the
/// study's accent envelope is at its plateau and still has 100 ms to run, while `Recovering` is
/// specified with `accent = 0`, so the study and the real schedule genuinely disagree there. That
/// is a deliberate difference — "accent signals an attempt, not success", and a real recovery is
/// not allowed to keep flashing — recorded here rather than asserted away.
#[test]
fn the_attack_schedule_agrees_with_the_studys_keyframes() {
    let d_coil = T_SNAP - T_COIL;
    let d_snap = T_OPEN - T_SNAP;
    let windup = episode(AttackPhase::Windup, 0.0, d_coil, Reach::FOLDED);
    let strike = episode(
        AttackPhase::Strike,
        0.0,
        d_snap,
        at(&windup, d_coil).reach(),
    );
    let recovering = episode(
        AttackPhase::Recovering,
        0.0,
        1.0,
        at(&strike, d_snap).reach(),
    );

    // Whether the episode row owns the study's accent at that instant (see the note above).
    const ACCENT_OWNED: bool = true;
    let rows: [(&str, f64, &AttackEpisode, f64, bool); 8] = [
        ("mid-coil", T_COIL + 0.06, &windup, 0.06, ACCENT_OWNED),
        (
            "the release, from the windup",
            T_SNAP,
            &windup,
            d_coil,
            !ACCENT_OWNED,
        ),
        (
            "the release, from the strike",
            T_SNAP,
            &strike,
            0.0,
            ACCENT_OWNED,
        ),
        ("mid-snap", T_SNAP + 0.06, &strike, 0.06, ACCENT_OWNED),
        (
            "settlement, from the strike",
            T_OPEN,
            &strike,
            d_snap,
            ACCENT_OWNED,
        ),
        (
            "settlement, from the recovery",
            T_OPEN,
            &recovering,
            0.0,
            !ACCENT_OWNED,
        ),
        ("mid-recoil", T_OPEN + 0.1, &recovering, 0.1, ACCENT_OWNED),
        (
            "the recoil's end",
            T_END,
            &recovering,
            RECOIL_SECONDS,
            ACCENT_OWNED,
        ),
    ];

    for (what, t_h, ep, elapsed, accent_owned) in rows {
        let h = hunt_state(t_h);
        let far = hunt_state(t_h - FAR_LAG_SECONDS);
        let c = at(ep, elapsed);
        let close = |name: &str, got: f64, want: f64| {
            assert!(
                (got - want).abs() <= 1e-9,
                "{what} ({:?} at {elapsed}) vs hunt_state({t_h}): {name} is {got}, not {want}",
                ep.phase
            );
        };
        close("near", c.near_reach, h.reach);
        close("far", c.far_reach, far.reach);
        close("compress", c.compress, h.compress);
        close("lunge", c.lunge, h.lunge);
        close("charge", c.charge, h.charge);
        close("blink", c.blink, h.blink);
        if accent_owned {
            close("accent", c.accent, h.accent);
        }
        assert_eq!(
            c.hush, 1.0,
            "{what}: an attack hushes the ambient body throughout"
        );
    }

    // The one post-recoil blink is the study's, to the instant: `envelope((t − (RECOIL_SECONDS +
    // BLINK_AFTER_RECOIL)) / BLINK_SECONDS)` against `envelope((t_h − (T_END + 0.3)) /
    // BLINK_SECONDS)`, which is the same blink because `T_END = T_OPEN + RECOIL_SECONDS`.
    for step in 0..40 {
        let elapsed = f64::from(step) * 0.03;
        let t_h = T_OPEN + elapsed;
        let (got, want) = (at(&recovering, elapsed).blink, hunt_state(t_h).blink);
        assert!(
            (got - want).abs() <= 1e-9,
            "the recovery blink at {elapsed} is {got}, where the study has {want} at {t_h}"
        );
    }
    // Non-vacuity: that sweep really did see the blink.
    assert!(at(&recovering, RECOIL_SECONDS + BLINK_AFTER_RECOIL + 0.14).blink == 1.0);
}

/// The real strike of the contract's candidate schedule: 1 s long, entered from a 0.6 s windup's
/// cocked pose. "Hold the cocked pose during the early paid approach, and spend the FINAL
/// approximately 120 ms of Strike on its cubic extension. Keep full extension through the actual
/// settlement boundary."
///
/// So with `E = EXTEND_SECONDS = 0.12` and `hold = D − E = 0.88`:
///
/// * for `t < 0.88` the pose is the entry pose **exactly** — no extension, no contact, and no
///   accent (its envelope opens at `hold − 0.02 = 0.86`). The far limb is the one channel that
///   moves during the hold, and only because it is catching up: it holds `from.far` for the first
///   45 ms and then reads the held `from.near`;
/// * at `t = 1.0`, the settlement boundary, the pose is full contact: `near = 1`, `compress =
///   −0.3`, `lunge = 1.1`;
/// * and at 1.5 s, 3 s and 8 s it is *still* full contact. This is the assertion that fails if
///   the study's complete 440 ms gesture is replayed at the strike's start: the claws would have
///   retracted roughly half a second before the core's single contact check.
///
/// The lantern charge is the one thing that legitimately keeps changing after settlement — the doc
/// gives it `f.charge · exp(−(t − hold) / CHARGE_DECAY_SECONDS)`, so it is checked as a decay
/// rather than as a hold.
#[test]
fn a_real_strike_extends_only_in_its_final_hundred_and_twenty_milliseconds_and_then_holds() {
    let from = cocked();
    assert!(
        (from.near + 0.35).abs() < 1e-12,
        "the windup's end is cocked, got {}",
        from.near
    );
    assert!(
        (from.compress - 1.7).abs() < 1e-12,
        "the windup's end is coiled"
    );
    assert_eq!(from.lunge, 0.0, "a windup never lunges");
    assert!(
        (from.charge - 1.0).abs() < 1e-12,
        "the windup charges the chain to 1"
    );

    let strike = strike_episode(0.0);
    let hold = STRIKE_SECONDS - EXTEND_SECONDS;
    assert!(
        (hold - 0.88).abs() < 1e-12,
        "the candidate schedule holds for 0.88 s"
    );

    for &t in &[
        0.0, 0.005, 0.02, 0.0449, 0.045, 0.1, 0.3, 0.5, 0.8, 0.85, 0.86, 0.87, 0.8799,
    ] {
        let c = at(&strike, t);
        assert_eq!(
            c.near_reach, from.near,
            "the strike must hold its entry reach at {t}"
        );
        assert_eq!(
            c.compress, from.compress,
            "the strike must hold its entry compression at {t}"
        );
        assert_eq!(
            c.lunge, from.lunge,
            "the strike must hold its entry lunge at {t}"
        );
        assert_eq!(
            c.charge, from.charge,
            "the strike must hold its entry charge at {t}"
        );
        // The accent's own envelope opens 20 ms before the extension — the study's pre-roll,
        // whose place in a real strike is the last 20 ms of the hold. Before that the paid
        // approach carries no accent at all.
        let want_accent = envelope_reference((t - (hold - 0.02)) / ACCENT_SECONDS);
        assert!(
            (c.accent - want_accent).abs() <= 1e-9,
            "during the hold the accent at {t} is {}, not the envelope's {want_accent}",
            c.accent
        );
        if t <= hold - 0.02 {
            assert_eq!(c.accent, 0.0, "no accent during the paid approach, at {t}");
        }
        assert_eq!(c.blink, 0.0, "a strike never blinks, at {t}");
        assert_eq!(c.hush, 1.0, "a strike hushes the ambient body, at {t}");
        let want_far = if t < FAR_LAG_SECONDS {
            from.far
        } else {
            from.near
        };
        assert_eq!(
            c.far_reach, want_far,
            "the far limb during the hold, at {t}"
        );
    }

    // Full contact at the settlement boundary, and still full contact seven seconds later.
    for &t in &[STRIKE_SECONDS, 1.5, 3.0, 8.0] {
        let c = at(&strike, t);
        for (name, got, want) in [
            ("near", c.near_reach, 1.0),
            ("compress", c.compress, -0.3),
            ("lunge", c.lunge, 1.1),
        ] {
            assert!(
                (got - want).abs() <= 1e-9,
                "at {t} s of a 1 s strike, {name} is {got}, not the settled {want} — a held \
                 strike must not retract"
            );
        }
        // The far limb arrives 45 ms after the near one, so at the settlement boundary itself it
        // is still on the last sliver of its cubic (`near_reach(1 − 0.045)`), and only from
        // `D + FAR_LAG_SECONDS` on is it at full extension. That is the doc's rule and the
        // contract's: the far claw "trails by 45 ms", and `Effectors::far_claw` is named "at
        // settlement (its 45 ms lag over)".
        if t >= STRIKE_SECONDS + FAR_LAG_SECONDS {
            assert!(
                (c.far_reach - 1.0).abs() <= 1e-9,
                "at {t} s the far limb has had its 45 ms, but its reach is {}",
                c.far_reach
            );
        } else {
            let want = at(&strike, t - FAR_LAG_SECONDS).near_reach;
            assert_eq!(
                c.far_reach, want,
                "at {t} s the far limb is 45 ms behind the near one"
            );
            assert!(
                c.far_reach > 0.9 && c.far_reach < 1.0,
                "at the settlement boundary the far limb is nearly there, not {}",
                c.far_reach
            );
        }
        assert_eq!(c.blink, 0.0, "a strike never blinks, at {t}");
    }
    // The charge bleeds out instead of holding.
    let charges: Vec<f64> = [1.0, 1.5, 3.0, 8.0]
        .iter()
        .map(|&t| at(&strike, t).charge)
        .collect();
    for w in charges.windows(2) {
        assert!(
            w[1] < w[0],
            "the chain's charge must bleed out after the release: {charges:?}"
        );
    }
    assert!(
        charges[3] < 0.001,
        "eight seconds on, the chain is dark: {}",
        charges[3]
    );

    // The accent: an envelope that opens 20 ms before the extension, peaks inside it and is over
    // before the settlement boundary plus its own 240 ms.
    assert_eq!(at(&strike, 0.85).accent, 0.0);
    assert_eq!(
        at(&strike, hold - 0.02).accent,
        0.0,
        "the envelope is 0 at its own start"
    );
    let peak = at(&strike, hold - 0.02 + 0.5 * ACCENT_SECONDS).accent;
    assert_eq!(peak, 1.0, "the accent's plateau is 1");
    for &t in &[1.2, 1.5, 4.0] {
        assert_eq!(at(&strike, t).accent, 0.0, "the accent is over by {t}");
    }
    // The whole envelope, term by term: `envelope((t − (hold − 0.02)) / ACCENT_SECONDS)`, one
    // raised cosine with its 20 ms pre-roll, over four seconds of attack time — and **continuous**
    // there, which is the reason the pre-roll is not gated on the extension (see the header note).
    // The continuity bound is the envelope's own: its steepest slope is `0.5 · (π / 0.4) = 3.927`
    // per unit of `u`, and one millisecond advances `u` by `0.001 / ACCENT_SECONDS = 0.004167`, so
    // no millisecond may move the accent by more than `0.0164`. A gated pre-roll would step by
    // `envelope(0.02 / 0.24) = 0.1033` in one millisecond and fail here.
    let mut largest = 0.0f64;
    let mut previous = at(&strike, 0.0).accent;
    for step in 0..=4000 {
        let t = f64::from(step) * 0.001;
        let a = at(&strike, t).accent;
        assert!(
            (0.0..=1.0).contains(&a),
            "the accent is bounded by the envelope, got {a} at {t}"
        );
        let want = envelope_reference((t - (hold - 0.02)) / ACCENT_SECONDS);
        assert!(
            (a - want).abs() <= 1e-9,
            "the accent at {t} is {a}, not the envelope's {want}"
        );
        largest = largest.max((a - previous).abs());
        previous = a;
    }
    assert!(
        largest > 0.0 && largest <= 0.5 * std::f64::consts::PI / 0.4 * 0.001 / ACCENT_SECONDS,
        "the accent's largest one-millisecond step is {largest}: it switches on rather than \
         rising through the envelope"
    );
    eprintln!("the accent's largest step per millisecond is {largest:.5}");
}

/// "`Windup`: fold → cock over the whole real windup; **no extension, no contact**." A 0.6 s
/// windup held for twelve seconds — the contract's "hold each real phase for longer than six
/// seconds: no synthetic attacks" — never extends: its reach is cocked (`−0.35` exactly once
/// `u` saturates) and never positive, its lunge is exactly zero throughout, and it neither
/// accents nor blinks.
#[test]
fn a_real_windup_cocks_and_holds_without_ever_extending() {
    let windup = episode(AttackPhase::Windup, 0.0, WINDUP_SECONDS, Reach::FOLDED);
    let mut previous = 1.0;
    for step in 0..=1200 {
        let t = f64::from(step) * 0.01;
        let c = at(&windup, t);
        assert!(
            c.near_reach <= 0.0,
            "a windup extended to {} at {t}",
            c.near_reach
        );
        assert!(
            c.far_reach <= 0.0,
            "a windup's far limb extended to {} at {t}",
            c.far_reach
        );
        assert_eq!(c.lunge, 0.0, "a windup lunged at {t}");
        assert_eq!(c.accent, 0.0, "a windup accented at {t}");
        assert_eq!(c.blink, 0.0, "a windup blinked at {t}");
        assert_eq!(c.hush, 1.0, "a windup must hush the ambient body, at {t}");
        assert!(
            c.near_reach <= previous + 1e-12,
            "the cock must not bounce back at {t}"
        );
        previous = c.near_reach;
        let u = smoothstep(t / WINDUP_SECONDS);
        assert!(
            (c.near_reach + 0.35 * u).abs() <= 1e-12,
            "the cock is −0.35 · smoothstep at {t}"
        );
        assert!(
            (c.compress - 1.7 * u).abs() <= 1e-12,
            "the coil is 1.7 · smoothstep at {t}"
        );
        assert!(
            (c.charge - u).abs() <= 1e-12,
            "the charge is smoothstep at {t}"
        );
    }
    let held = at(&windup, 8.0);
    assert_eq!(
        held.near_reach, -0.35,
        "a held windup sits at exactly −0.35"
    );
    assert_eq!(held.lunge, 0.0);
    assert_eq!(held.compress, 1.7);
    // A negative elapsed reads as 0, and a degenerate duration is an instantaneous phase
    // (`u = 1`), not a division by zero.
    assert_eq!(
        at(&windup, -3.0),
        at(&windup, 0.0),
        "a negative elapsed reads as 0"
    );
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let instant = episode(AttackPhase::Windup, 0.0, bad, Reach::FOLDED);
        let c = at(&instant, 0.0);
        assert_eq!(
            c.near_reach, -0.35,
            "a duration of {bad} is an instantaneous phase"
        );
        assert_eq!(
            c.compress, 1.7,
            "a duration of {bad} is an instantaneous phase"
        );
    }
}

/// "`Recovering`: recoil from the entry reach over `RECOIL_SECONDS`, then folded stillness for the
/// rest of the recovery. **No repeat strike.**" And the contract's interruption rule: "If a phase
/// is interrupted, recoil from its displayed reach rather than snapping to an unrelated study
/// keyframe."
///
/// Two entries are tested. From `Reach::EXTENDED` the recoil falls monotonically to exactly 0 by
/// `RECOIL_SECONDS` and stays there for four seconds — a body that re-struck while waiting, or
/// that rebounded, fails. From a **partial** reach (a strike interrupted a third of the way into
/// its extension) the channels at `t = 0` equal that reach *exactly*, which is the continuity the
/// contract asks for, and reach zero on the same 200 ms schedule.
#[test]
fn a_recoil_falls_from_the_reach_it_was_handed_and_stays_folded() {
    let partial = Reach {
        near: 0.4,
        far: 0.3,
        compress: 0.9,
        lunge: 0.5,
        charge: 0.6,
    };
    for (what, from) in [
        ("full extension", Reach::EXTENDED),
        ("an interrupted strike", partial),
    ] {
        let recovering = episode(AttackPhase::Recovering, 0.0, 4.0, from);

        // Continuity: the first frame of the recovery is the last frame of what it interrupted.
        let start = at(&recovering, 0.0);
        assert_eq!(
            start.near_reach, from.near,
            "{what}: the recoil must start where it was"
        );
        assert_eq!(
            start.far_reach, from.far,
            "{what}: the far limb keeps its displayed reach"
        );
        assert_eq!(
            start.compress, from.compress,
            "{what}: the hull keeps its compression"
        );
        assert_eq!(start.lunge, from.lunge, "{what}: the head keeps its lunge");
        assert_eq!(
            start.charge, from.charge,
            "{what}: the chain keeps its charge"
        );

        let mut previous = f64::INFINITY;
        for step in 0..=400 {
            let t = f64::from(step) * 0.01;
            let c = at(&recovering, t);
            assert!(
                c.near_reach <= previous + 1e-12,
                "{what}: the recoil bounced back to {} at {t}",
                c.near_reach
            );
            previous = c.near_reach;
            assert_eq!(c.accent, 0.0, "{what}: a recovery never accents, at {t}");
            assert_eq!(c.hush, 1.0, "{what}: a recovery stays hushed, at {t}");
            let u = smoothstep(t / RECOIL_SECONDS);
            assert!(
                (c.near_reach - from.near * (1.0 - u)).abs() <= 1e-12,
                "{what}: the recoil is `f.near · (1 − smoothstep(t / RECOIL_SECONDS))`, at {t}"
            );
            assert!(
                (c.compress - from.compress * (1.0 - u)).abs() <= 1e-12,
                "{what} at {t}"
            );
            assert!(
                (c.lunge - from.lunge * (1.0 - u)).abs() <= 1e-12,
                "{what} at {t}"
            );
        }
        for &t in &[RECOIL_SECONDS, 0.25, 1.0, 4.0] {
            let c = at(&recovering, t);
            assert_eq!(c.near_reach, 0.0, "{what}: folded by {t}");
            assert_eq!(c.compress, 0.0, "{what}: uncoiled by {t}");
            assert_eq!(c.lunge, 0.0, "{what}: no lunge by {t}");
            // The far limb finishes its own recoil 45 ms later, by the lag rule.
            if t >= RECOIL_SECONDS + FAR_LAG_SECONDS {
                assert_eq!(c.far_reach, 0.0, "{what}: the far limb is folded by {t}");
            }
        }
        assert_eq!(
            at(&recovering, RECOIL_SECONDS + FAR_LAG_SECONDS).far_reach,
            0.0,
            "{what}: the far limb folds exactly 45 ms after the near one"
        );
    }
}

/// "`Handling`: as `Recovering`, except `hush = 1 − smoothstep((t − RECOIL_SECONDS) /
/// HANDLING_RELEASE_SECONDS)`" — "the successful-capture recoil, after which the hush releases and
/// the ambient body resumes; the meal itself is the gut breath, never a flourish implied by the
/// phase name."
///
/// So handling's *recoil* is bit-identical to a recovery's from the same reach (every channel but
/// the hush), its hush is still 1 at the end of the recoil, and it is exactly 0 one
/// `HANDLING_RELEASE_SECONDS` later — while a recovery stays hushed at four seconds. A renderer
/// that released the hush at phase entry would let the ambient wave run through the recoil; one
/// that never released it would leave a handling hunter unnaturally still forever.
#[test]
fn handling_releases_its_hush_where_a_recovery_keeps_it() {
    let from = Reach::EXTENDED;
    let handling = episode(AttackPhase::Handling, 0.0, 4.0, from);
    let recovering = episode(AttackPhase::Recovering, 0.0, 4.0, from);

    for step in 0..=400 {
        let t = f64::from(step) * 0.01;
        let (h, r) = (at(&handling, t), at(&recovering, t));
        assert_eq!(
            h.near_reach, r.near_reach,
            "handling's recoil is a recovery's, at {t}"
        );
        assert_eq!(h.far_reach, r.far_reach, "at {t}");
        assert_eq!(h.compress, r.compress, "at {t}");
        assert_eq!(h.lunge, r.lunge, "at {t}");
        assert_eq!(h.charge, r.charge, "at {t}");
        assert_eq!(h.accent, r.accent, "at {t}");
        assert_eq!(
            h.blink, r.blink,
            "handling blinks once, exactly as a recovery does, at {t}"
        );
        assert!(
            (0.0..=1.0).contains(&h.hush),
            "the hush is a 0..1 fraction, got {} at {t}",
            h.hush
        );
        let want = 1.0 - smoothstep((t - RECOIL_SECONDS) / HANDLING_RELEASE_SECONDS);
        assert!(
            (h.hush - want).abs() <= 1e-12,
            "handling's hush at {t} is {}, not {want}",
            h.hush
        );
        assert_eq!(r.hush, 1.0, "a recovery stays hushed, at {t}");
    }
    assert_eq!(at(&handling, 0.0).hush, 1.0);
    assert_eq!(
        at(&handling, RECOIL_SECONDS).hush,
        1.0,
        "the hush holds through the recoil"
    );
    assert_eq!(
        at(&handling, RECOIL_SECONDS + HANDLING_RELEASE_SECONDS).hush,
        0.0,
        "the hush is fully released one HANDLING_RELEASE_SECONDS after the recoil"
    );
    assert_eq!(
        at(&recovering, 4.0).hush,
        1.0,
        "a recovery is hushed at four seconds"
    );
}

/// "The far forelimb lags: `far_reach` is the `near` of the same rules evaluated at `t −
/// FAR_LAG_SECONDS` when that is ≥ 0, and `f.far` before that." And the contract: "the far claw's
/// lag is 45 ms of simulated **attack** time, not a frame count and not a modulo loop that can
/// replay a previous attack. Clamp before the current episode begins."
///
/// Checked against the *same function* re-timed 45 ms earlier, for all four phases, over a sweep
/// that starts before the lag and runs past every phase's end — so an implementation that read the
/// study's `hunt_state((t − 0.045) mod 6)` (which would replay the previous attack) or that lagged
/// by a frame count fails at the first instant.
#[test]
fn the_far_forelimb_lags_the_near_one_by_forty_five_milliseconds_of_attack_time() {
    let from = Reach {
        near: 0.4,
        far: -0.2,
        compress: 0.9,
        lunge: 0.5,
        charge: 0.6,
    };
    for ep in [
        episode(AttackPhase::Windup, 0.0, WINDUP_SECONDS, Reach::FOLDED),
        strike_episode(0.0),
        episode(AttackPhase::Recovering, 0.0, 2.0, Reach::EXTENDED),
        episode(AttackPhase::Handling, 0.0, 2.0, from),
        episode(AttackPhase::Strike, 0.0, STRIKE_SECONDS, from),
    ] {
        for step in 0..=300 {
            let t = f64::from(step) * 0.005;
            let c = at(&ep, t);
            if t < FAR_LAG_SECONDS {
                assert_eq!(
                    c.far_reach, ep.from.far,
                    "{:?}: before the lag is over the far limb holds the reach it was handed, at {t}",
                    ep.phase
                );
            } else {
                let earlier = at(&ep, t - FAR_LAG_SECONDS).near_reach;
                assert!(
                    (c.far_reach - earlier).abs() <= 1e-12,
                    "{:?} at {t}: the far limb reads {}, but the near limb read {earlier} \
                     45 ms earlier",
                    ep.phase,
                    c.far_reach
                );
            }
        }
    }
}

/// "`blink = envelope((t − (RECOIL_SECONDS + BLINK_AFTER_RECOIL)) / BLINK_SECONDS)`" — one blink
/// after the recoil, in `Recovering` and `Handling` only, bounded by the envelope and with the
/// envelope's zero slope at both ends (so nothing switches on for a single frame).
#[test]
fn the_one_post_recoil_blink_starts_after_the_recoil_and_is_bounded() {
    let start = RECOIL_SECONDS + BLINK_AFTER_RECOIL;
    for phase in [AttackPhase::Recovering, AttackPhase::Handling] {
        let ep = episode(phase, 0.0, 4.0, Reach::EXTENDED);
        for &t in &[0.0, 0.05, 0.1, 0.2, 0.25, 0.3, 0.4, 0.45, 0.49] {
            assert_eq!(
                at(&ep, t).blink,
                0.0,
                "{phase:?}: no blink before the recoil ends, at {t}"
            );
        }
        // At the envelope's own ends the value is 0 with zero slope, so a last-bit difference in
        // the argument shows up as a vanishing value rather than as a jump: the boundary rows are
        // asserted as "not open yet / over" rather than bit-exactly zero.
        assert!(
            at(&ep, start).blink <= 1e-12,
            "{phase:?}: the blink is not open at its own start, got {}",
            at(&ep, start).blink
        );
        for &u in &[0.05, 0.2, 0.4, 0.5, 0.6, 0.8, 0.95] {
            let b = at(&ep, start + u * BLINK_SECONDS).blink;
            assert!(
                b > 0.0,
                "{phase:?}: the blink must be open at {u} of its window, got {b}"
            );
            assert!(
                b <= 1.0,
                "{phase:?}: the blink is bounded by the envelope, got {b}"
            );
        }
        assert_eq!(
            at(&ep, start + 0.5 * BLINK_SECONDS).blink,
            1.0,
            "{phase:?}: the plateau is 1"
        );
        assert!(
            at(&ep, start + BLINK_SECONDS).blink <= 1e-12,
            "{phase:?}: the blink is not closed at the end of its window, got {}",
            at(&ep, start + BLINK_SECONDS).blink
        );
        for &t in &[
            start + BLINK_SECONDS + 0.001,
            start + BLINK_SECONDS + 0.5,
            3.9,
        ] {
            assert_eq!(
                at(&ep, t).blink,
                0.0,
                "{phase:?}: exactly one blink, none again at {t}"
            );
        }
    }
    for phase in [AttackPhase::Windup, AttackPhase::Strike] {
        let ep = episode(phase, 0.0, 1.0, Reach::FOLDED);
        for step in 0..=200 {
            let t = f64::from(step) * 0.02;
            assert_eq!(at(&ep, t).blink, 0.0, "{phase:?} must not blink, at {t}");
        }
    }
}

// ---------------------------------------------------------------------------
// 3. held phases: no synthetic hunts
// ---------------------------------------------------------------------------

/// The contract's acceptance item, at the level of drawn pixels rather than channels: "Hold each
/// real phase for longer than six seconds: **no synthetic attacks**", and, from the timing
/// recommendation, the claws must still be out at the settlement boundary rather than having
/// retracted half a second earlier.
///
/// Measured on the near forelimb's frontmost painted texel in body coordinates (`α > 0.2`, so a
/// filter tail cannot pass for a claw):
///
/// * a 0.6 s `Windup` held for twelve seconds of both attack and ambient time never reaches
///   farther forward than the folded pose does (half a pixel of slack for the coil pulling the
///   whole head-attached limb forward by `compress · (1 − 13/17) = 0.4` px). A body replaying the
///   study's six-second loop would put the claw past 12 px twice in that sweep;
/// * a 1 s `Strike` held from its settlement boundary to nine seconds keeps the claw past 12.5 px
///   — the folded pose's own frontmost is around 6 px, so the two measurements cannot be confused;
/// * and at a *fixed* ambient instant the held strike's near limb is texel-identical across
///   settlement instants, because nothing in the pose changes any more. The one exception is the
///   accent: at `t = 1.0` exactly the strike's accent envelope is still on its plateau, so that
///   frame has the same coverage and a warmer claw, which is asserted separately.
#[test]
fn a_held_windup_never_extends_and_a_held_strike_never_retracts() {
    let rig = Lanternjaw::new();
    let mut parts = Vec::new();

    // The folded reference, over the same ambient sweep.
    let mut folded_front = f64::NEG_INFINITY;
    for frame in 0..(12 * FPS) {
        let t = frame as f64 / FPS as f64;
        rig.parts_living(&quiet(t, 0.0), &mut parts);
        folded_front = folded_front.max(near_limb_front(&parts));
    }
    assert!(
        folded_front < 8.0,
        "the folded pose's own front is {folded_front}, not folded at all"
    );

    let windup = episode(AttackPhase::Windup, 0.0, WINDUP_SECONDS, Reach::FOLDED);
    let mut windup_front = f64::NEG_INFINITY;
    for frame in 0..(12 * FPS) {
        let t = frame as f64 / FPS as f64;
        let pose = LivingPose {
            ambient: t,
            movement: 0.0,
            attack: Some(AttackEpisode {
                elapsed: t,
                ..windup
            }),
            gut: 0.0,
            cocoon: None,
        };
        rig.parts_living(&pose, &mut parts);
        let front = near_limb_front(&parts);
        assert!(
            front <= folded_front + 0.5,
            "a windup held for {t} s reached body x {front}, past the folded {folded_front} — a \
             synthetic strike"
        );
        windup_front = windup_front.max(front);
    }
    eprintln!("held windup: frontmost near-limb x {windup_front:.3} (folded {folded_front:.3})");

    // A strike held at settlement keeps its claws out.
    let mut step = 0;
    while step <= 160 {
        let elapsed = STRIKE_SECONDS + f64::from(step) * 0.05;
        let pose = LivingPose {
            ambient: elapsed,
            movement: 0.0,
            attack: Some(strike_episode(elapsed)),
            gut: 0.0,
            cocoon: None,
        };
        rig.parts_living(&pose, &mut parts);
        let front = near_limb_front(&parts);
        assert!(
            front >= 12.5 - 1e-9,
            "a strike held to {elapsed} s has its claw at body x {front}, not out past 12.5 — the \
             gesture retracted before the contact check"
        );
        step += 1;
    }

    // At one ambient instant the settled strike is a still picture.
    let mut reference = Vec::new();
    rig.parts_living(
        &LivingPose {
            attack: Some(strike_episode(1.2)),
            ..quiet(2.0, 0.0)
        },
        &mut reference,
    );
    for &elapsed in &[1.2, 2.0, 5.0, 9.0] {
        let pose = LivingPose {
            attack: Some(strike_episode(elapsed)),
            ..quiet(2.0, 0.0)
        };
        rig.parts_living(&pose, &mut parts);
        assert_part_identical(
            part_of(&parts, PartName::NearLimb),
            part_of(&reference, PartName::NearLimb),
            &format!("the settled strike's near limb at {elapsed} s of attack time"),
        );
    }
    // The accent frame: the same coverage, a warmer claw.
    rig.parts_living(
        &LivingPose {
            attack: Some(strike_episode(1.0)),
            ..quiet(2.0, 0.0)
        },
        &mut parts,
    );
    let (hot, cool) = (
        part_of(&parts, PartName::NearLimb),
        part_of(&reference, PartName::NearLimb),
    );
    let mut warmer = 0usize;
    for ty in 0..hot.sprite.height() {
        for tx in 0..hot.sprite.width() {
            let (h, c) = (
                hot.sprite.texel(tx as i32, ty as i32),
                cool.sprite.texel(tx as i32, ty as i32),
            );
            assert_eq!(
                h[3], c[3],
                "the accent must not move the limb: texel ({tx}, {ty})"
            );
            if h != c {
                assert!(
                    h[0] - h[2] > c[0] - c[2],
                    "texel ({tx}, {ty}) changed without getting warmer: {h:?} vs {c:?}"
                );
                warmer += 1;
            }
        }
    }
    assert!(warmer > 0, "the accent's peak did not warm the claw at all");
}

// ---------------------------------------------------------------------------
// 4. the gut breath
// ---------------------------------------------------------------------------

/// "The gut breath adds `GUT_BREATH_PX · gut · sin(2π t / GUT_BREATH_SECONDS)` to `dy` of template
/// columns 4..=8 (and to the legs and cocoon on those columns, which ride the same columns); a gut
/// of 0 adds **exactly** 0" — and the contract's "gut-zero cannot pretend to chew a meal", with
/// `LivingPose::gut` documented as "0 is no meal, whatever the phase says".
///
/// Three things follow, each asserted: with a full gut the `Abdomen` (columns 4..=8) and the
/// `Underside` (the leg pair on column 8) move, while `Tail` (0..=3), `Thorax` (9..=12), `Head`
/// (13..=17) and both limbs are texel-identical; at an ambient instant where the breath's sine is
/// exactly zero a full gut changes nothing at all; and a gut of 0 — or of a nonsense value, which
/// the channel producer clamps — is the body with no meal. (`Glow` is deliberately not asserted
/// either way: the lanterns of columns 4, 6 and 8 ride the same breath, so their halos may move
/// with it.)
#[test]
fn the_gut_breath_moves_the_abdomen_and_the_legs_that_ride_it_and_nothing_else() {
    // A quarter into the breath period: `sin(2π · 0.55 / 2.2) = 1`, the largest swell there is.
    let ambient = 0.25 * GUT_BREATH_SECONDS;
    let full = LivingPose {
        gut: 1.0,
        ..quiet(ambient, 0.0)
    };
    let empty = LivingPose {
        gut: 0.0,
        ..quiet(ambient, 0.0)
    };
    let (a, b) = (living(&full), living(&empty));
    for name in [PartName::Abdomen, PartName::Underside] {
        assert!(
            parts_differ(&a, &b, name),
            "a full gut must breathe: {name:?} is identical to the empty body"
        );
    }
    for name in [
        PartName::Tail,
        PartName::Thorax,
        PartName::Head,
        PartName::NearLimb,
        PartName::FarLimb,
    ] {
        assert!(
            !parts_differ(&a, &b, name),
            "the gut breath is columns 4..=8 only, but {name:?} moved with it"
        );
    }

    // At the breath's zero crossing a full gut is the empty body, exactly.
    for &t in &[0.0, GUT_BREATH_SECONDS, 2.0 * GUT_BREATH_SECONDS] {
        assert_parts_identical(
            &living(&LivingPose {
                gut: 1.0,
                ..quiet(t, 0.0)
            }),
            &living(&quiet(t, 0.0)),
            &format!("at ambient {t} the breath's sine is 0, so a full gut adds exactly 0"),
        );
    }

    // No meal, no breath — at every instant, and whatever nonsense the adapter hands over.
    for &t in &INSTANTS {
        assert_parts_identical(
            &living(&LivingPose {
                gut: 0.0,
                ..quiet(t, 0.0)
            }),
            &study(t, Mode::Rest),
            &format!("gut 0 at {t} is the resting gallery body"),
        );
    }
    assert_eq!(
        Channels::living(&LivingPose {
            gut: 0.0,
            ..quiet(1.0, 0.0)
        })
        .gut,
        0.0
    );
    assert_eq!(
        Channels::living(&LivingPose {
            gut: 1.0,
            ..quiet(1.0, 0.0)
        })
        .gut,
        1.0
    );
    assert_eq!(
        Channels::living(&LivingPose {
            gut: 4.0,
            ..quiet(1.0, 0.0)
        })
        .gut,
        1.0,
        "`gut = clamp(gut, 0, 1)`"
    );
    assert_eq!(
        Channels::living(&LivingPose {
            gut: -2.0,
            ..quiet(1.0, 0.0)
        })
        .gut,
        0.0
    );
    assert_eq!(
        Channels::living(&LivingPose {
            gut: f64::NAN,
            ..quiet(1.0, 0.0)
        })
        .gut,
        0.0,
        "a NaN gut reads as 0"
    );
}

// ---------------------------------------------------------------------------
// 5. the cocoon follows funded escrow, not a phase
// ---------------------------------------------------------------------------

/// "`None` means no cocoon at all, whatever the phase, satiety or reserve" (the contract: "A
/// cocoon appears only for actual funded escrow … No escrow means no cocoon, even during satiety,
/// Handling or recovery"), and "`cocoon Some(smoothstep(p / COCOON_REVEAL))` … `tail_lift` = that
/// reveal", with `rasterize` painting "the study's six cells with every alpha multiplied by
/// `reveal`; `None` paints none".
///
/// So: no escrow paints no cocoon texel at all — including in `Handling`, which is exactly the
/// phase a cocoon must not be inferred from; a gestation of 0 is also invisible (its reveal is
/// `smoothstep(0) = 0`) and is bit-identical to no escrow; an early gestation paints the same
/// cells at a *lower* alpha, in proportion to its reveal (the excess alpha over the cocoon-free
/// underside is linear in the reveal, since a source-over of a scaled front alpha over the same
/// background is `reveal · α_f · (1 − α_b)`); and the reveal itself is
/// `smoothstep(p / COCOON_REVEAL)`, saturating at `COCOON_REVEAL` rather than at birth.
#[test]
fn a_cocoon_appears_only_for_a_funded_escrow_and_rises_with_its_real_gestation() {
    // The reveal mapping, and the tail lift that follows it.
    for &p in &[
        0.0, 0.02, 0.05, 0.1, 0.15, 0.19, 0.2, 0.3, 0.7, 1.0, 2.0, -1.0,
    ] {
        let ch = Channels::living(&LivingPose {
            cocoon: Some(p),
            ..quiet(1.0, 0.0)
        });
        let want = smoothstep(p / COCOON_REVEAL);
        assert_eq!(
            ch.cocoon,
            Some(want),
            "a gestation of {p} reveals smoothstep({p} / 0.2)"
        );
        assert_eq!(
            ch.tail_lift, want,
            "the tail lift follows the reveal, at {p}"
        );
    }
    let nan = Channels::living(&LivingPose {
        cocoon: Some(f64::NAN),
        ..quiet(1.0, 0.0)
    });
    assert_eq!(nan.cocoon, Some(0.0), "a NaN gestation reveals nothing");
    assert_eq!(nan.tail_lift, 0.0);
    let none = Channels::living(&quiet(1.0, 0.0));
    assert_eq!(none.cocoon, None, "no escrow, no cocoon");
    assert_eq!(none.tail_lift, 0.0, "and no tail lift either");
    assert!(
        Channels::living(&LivingPose {
            cocoon: Some(0.5),
            ..quiet(1.0, 0.0)
        })
        .cocoon
            == Some(1.0),
        "past COCOON_REVEAL the cocoon is fully revealed and breathes at full alpha"
    );

    // No escrow paints no cocoon, in every phase and at any satiety. The cocoon's six cells are
    // the study's `(−5 … −3, 2 … 3)`, whose painted centres are the body columns `−4.5`, `−3.5`
    // and `−2.5`. The window used below is the first two of those: the *gait* swings the rear leg
    // (base `−1`) by `1.3 · sin(2π u)`, so a walking body's own leg splat reaches the `−2.5`
    // column, and only `x ≤ −3` is cocoon-only ground. Two of the six cells are enough for the
    // claim, and keeping the window honest is what makes "no escrow, no cocoon" mean something.
    let handling = Some(episode(AttackPhase::Handling, 0.4, 2.0, Reach::EXTENDED));
    let cocoon_coverage = |pose: &LivingPose| {
        let parts = living(pose);
        let part = part_of(&parts, PartName::Underside);
        let mut painted_cells = 0usize;
        for (tx, ty) in painted(&part.sprite) {
            let b = texel_body(part, tx, ty);
            if (-5.5..=-3.0).contains(&b.x) && (1.0..=5.0).contains(&b.y) {
                painted_cells += 1;
            }
        }
        painted_cells
    };
    for &t in &INSTANTS {
        assert_parts_identical(
            &living(&LivingPose {
                cocoon: None,
                ..quiet(t, 0.0)
            }),
            &living(&LivingPose {
                cocoon: Some(0.0),
                ..quiet(t, 0.0)
            }),
            &format!("at {t}: a gestation of 0 reveals nothing, exactly as no escrow does"),
        );
        for (what, movement, gut, attack) in [
            ("perched and empty", 0.0, 0.0, None),
            ("walking", 1.0, 0.0, None),
            ("handling a real meal", 0.0, 1.0, handling),
        ] {
            let pose = LivingPose {
                movement,
                attack,
                gut,
                cocoon: None,
                ..quiet(t, 0.0)
            };
            assert_eq!(
                cocoon_coverage(&pose),
                0,
                "at {t}, {what} with no escrow painted a cocoon texel"
            );
        }
        assert!(
            cocoon_coverage(&LivingPose {
                cocoon: Some(1.0),
                ..quiet(t, 0.0)
            }) >= 4,
            "at {t} a funded escrow must actually paint its cocoon"
        );
    }

    // The reveal scales the cocoon's coverage: measured as the alpha the cocoon adds to the
    // underside over the same body with no escrow.
    let ambient = 0.9;
    let excess = |gestation: Option<f64>| {
        let with = living(&LivingPose {
            cocoon: gestation,
            ..quiet(ambient, 0.0)
        });
        let without = living(&quiet(ambient, 0.0));
        let (p, q) = (
            part_of(&with, PartName::Underside),
            part_of(&without, PartName::Underside),
        );
        let mut sum = 0.0f64;
        for ty in 0..p.sprite.height() {
            for tx in 0..p.sprite.width() {
                sum += f64::from(p.sprite.texel(tx as i32, ty as i32)[3])
                    - f64::from(q.sprite.texel(tx as i32, ty as i32)[3]);
            }
        }
        sum
    };
    let (half, full) = (excess(Some(0.1)), excess(Some(1.0)));
    assert_eq!(
        excess(None),
        0.0,
        "no escrow adds no coverage to the underside"
    );
    assert_eq!(
        excess(Some(0.0)),
        0.0,
        "a gestation of 0 adds no coverage either"
    );
    assert!(
        half > 0.05,
        "an early gestation must paint something, got {half}"
    );
    assert!(
        full > half,
        "a full reveal must be more opaque than an early one ({full} vs {half})"
    );
    // `smoothstep(0.1 / 0.2) = 0.5` exactly, and the excess is linear in the reveal.
    assert!(
        (half / full - 0.5).abs() < 0.02,
        "the cocoon's coverage should scale with its reveal: {half} / {full} = {}",
        half / full
    );
}

// ---------------------------------------------------------------------------
// 6. juvenile scale
// ---------------------------------------------------------------------------

/// "`draw_living` … then **one** `stamp_rig_scaled(canvas, anchor, heading, &[(rig_parts, 1.0)],
/// scale, opacity, scratch)`. Nothing else." At scale 1 that is the unscaled rig, so the drawn
/// image must be bit-identical to a `stamp_rig` of `parts_living`'s own parts — assembled here by
/// the test, which is the second, independent path.
#[test]
fn draw_living_is_one_scaled_stamp_of_its_own_parts() {
    let rig = Lanternjaw::new();
    let poses = [quiet(1.3, 0.0), quiet(0.4, 1.0), settled(2.0)];
    for (i, pose) in poses.iter().enumerate() {
        let mut parts = Vec::new();
        rig.parts_living(pose, &mut parts);
        let assembled: Vec<RigPart<'_>> = parts.iter().map(Part::rig_part).collect();
        assert!(
            rig_radius(&[(&assembled[..], 1.0)]) <= QUERY_RADIUS_MAX,
            "pose {i} needs a query radius past QUERY_RADIUS_MAX"
        );
        for (what, anchor, heading) in [
            ("mid-face", mid(), forward()),
            ("a diagonal", mid(), Vec2::new(0.6, -0.8)),
            (
                "a seam",
                SurfacePoint::new(Face::Front, 63.5, 32.5),
                forward(),
            ),
            (
                "a top vertex",
                SurfacePoint::new(Face::Top, 0.5, 0.5),
                Vec2::new(1.0, 1.0),
            ),
        ] {
            let mut reference = filled([0.03, 0.02, 0.07]);
            stamp_rig(
                &mut reference,
                anchor,
                heading,
                &[(&assembled[..], 1.0)],
                1.0,
                &mut Vec::new(),
            );
            let mut drawn = filled([0.03, 0.02, 0.07]);
            rig.draw_living(
                &mut drawn,
                anchor,
                heading,
                pose,
                1.0,
                1.0,
                &mut Vec::new(),
                &mut Vec::new(),
            );
            assert_identical(&reference, &drawn, &format!("pose {i} at {what}, scale 1"));
            assert!(
                max_diff(&drawn, &filled([0.03, 0.02, 0.07])) > 0.01,
                "pose {i} at {what}: nothing was drawn"
            );
        }
    }
}

/// "Everything scales together: offsets, pivots, the lattice, the lunge, the limbs and the query
/// radius" — the contract's "Scaling only sprite images leaves a juvenile's head/arms detached at
/// adult distances."
///
/// At `SCALE_MIN` the whole body must fit inside the scaled footprint. The bound is recomputed
/// from the module's own footprint contract: a destination pixel is painted only if the art
/// coordinate it reads, `b / scale`, is within one pixel of a painted texel centre, and every
/// painted texel centre lies inside `x ∈ [−BOUND_BACK, BOUND_FRONT]`, `y ∈ [−BOUND_ABOVE,
/// BOUND_BELOW]`; so `b.x ≤ scale · (BOUND_FRONT + 1)` and `|b.y| ≤ scale · (BOUND_ABOVE + 1)`.
/// The adult is drawn too, and asserted to *break* the juvenile's bound — otherwise the whole
/// measurement would be vacuous.
///
/// The light a juvenile carries is between 0.15 and 0.4 of the adult's: the area argument gives
/// `scale² = 0.25`, and the doc warns that "one-pixel details average with their neighbours" under
/// minification, which moves the total either way by the sampling phase rather than conserving it
/// exactly.
#[test]
fn a_juvenile_stays_inside_its_scaled_footprint_and_carries_about_a_quarter_of_the_light() {
    let pose = settled(2.0);
    let origin = mid().chart();
    let bounds = |scale: f64| {
        let image = drawn_living(&pose, mid(), forward(), scale);
        let mut front = f64::NEG_INFINITY;
        let mut back = f64::INFINITY;
        let mut across = 0.0f64;
        for (f, x, y) in every_pixel() {
            if image.get(f, x, y) == [0.0; 3] {
                continue;
            }
            assert_eq!(
                f,
                Face::Front,
                "a mid-face body at scale {scale} left Front"
            );
            let b = Vec2::new(f64::from(x) + 0.5 - origin.x, f64::from(y) + 0.5 - origin.y);
            front = front.max(b.x);
            back = back.min(b.x);
            across = across.max(b.y.abs());
        }
        (front, back, across, total_light(&image))
    };

    // A half-size juvenile (the default child fraction 0.4 maps to 0.632; 0.5 is a dyadic
    // scale inside the admitted range). The smallest admitted scale, `SCALE_MIN`, is swept
    // with the other core-admitted scales in `lanternjaw_scale.rs`.
    const HALF: f64 = 0.5;
    let (a_front, a_back, a_across, adult) = bounds(SCALE_MAX);
    let (j_front, j_back, j_across, juvenile) = bounds(HALF);
    // Below scale 1 the stamp is box-filtered: a sample sits up to half a pixel from its
    // destination centre, so the scaled bound gains `SUPERSAMPLE_REACH`.
    let front_bound = HALF * (BOUND_FRONT + 1.0) + SUPERSAMPLE_REACH;
    let back_bound = -HALF * (BOUND_BACK + 1.0) - SUPERSAMPLE_REACH;
    let across_bound = HALF * (BOUND_ABOVE + 1.0) + SUPERSAMPLE_REACH;
    assert!(
        j_front <= front_bound + 1e-9,
        "the juvenile reaches body x {j_front}, past its scaled front bound {front_bound}"
    );
    assert!(
        j_back >= back_bound - 1e-9,
        "the juvenile reaches body x {j_back}, past its scaled back bound {back_bound}"
    );
    assert!(
        j_across <= across_bound + 1e-9,
        "the juvenile reaches body |y| {j_across}, past its scaled lateral bound {across_bound}"
    );
    assert!(
        a_front > front_bound && a_back < back_bound && a_across > across_bound,
        "the adult ({a_front}, {a_back}, {a_across}) fits inside the juvenile's bound, so the \
         bound proves nothing"
    );
    let ratio = juvenile / adult;
    assert!(
        (0.15..0.4).contains(&ratio),
        "the juvenile carries {juvenile} light where the adult carries {adult} — a ratio of \
         {ratio}, not the area's 0.25"
    );
    eprintln!("scale {HALF}: front {j_front:.3} (bound {front_bound}), light ratio {ratio:.4}");
}

/// "`effectors(scale)` names where the scaled claws and jaw are drawn." Two halves, and both
/// matter for the core adapter's contact geometry:
///
/// * the numbers themselves are the doc's (`mouth = scale · (8 + 1.1 + 0.5, 0)`, `near_claw =
///   scale · (12.3 + head_dx + 0.5, 1.1)` with `head_dx = −0.3 · (1 − 13/17) + 1.1 · 0.5 =
///   0.4794…`, `far_claw = near_claw − scale · (0, 1)`), recomputed here, and they scale exactly
///   linearly;
/// * the named near claw is really *where the claw is drawn*: at `SCALE_MIN`, with the body held
///   at settlement, some painted pixel of the `NearLimb` part alone lies within one pixel of
///   `effectors(SCALE_MIN).near_claw`. A rig that scaled only the sprite images would leave the
///   claw at its adult 13 px while the named effector moved to 6.6 px, which is exactly the
///   "capture region overlaps the visible claw, not the thorax" evidence the contract asks for.
#[test]
fn the_named_effectors_scale_with_the_rig_and_land_on_the_drawn_claw() {
    let head_dx = -0.3 * (1.0 - 13.0 / 17.0) + 1.1 * 0.5;
    let adult = effectors(SCALE_MAX);
    assert!(
        (adult.mouth.x - 9.6).abs() < 1e-9,
        "the ingestion mouth is at 9.6, got {}",
        adult.mouth.x
    );
    assert_eq!(adult.mouth.y, 0.0);
    assert!(
        (adult.near_claw.x - (12.3 + head_dx + 0.5)).abs() < 1e-9,
        "the near claw is at 13.2794…, got {}",
        adult.near_claw.x
    );
    assert!(
        (adult.near_claw.y - 1.1).abs() < 1e-9,
        "got {}",
        adult.near_claw.y
    );
    assert_eq!(
        adult.far_claw.x, adult.near_claw.x,
        "the far claw trails on the same column"
    );
    assert!(
        (adult.far_claw.y - (adult.near_claw.y - 1.0)).abs() < 1e-9,
        "one pixel higher"
    );

    // `effectors(0.5) == 0.5 · effectors(1.0)` per component, **exactly**: halving is exact in
    // binary floating point, and so is the `near.y − scale` the far claw is built with (`a − 1` is
    // exact for `a ∈ [1, 2]` by Sterbenz, and halving it again is exact). For a scale that is not
    // a power of two the product rounds, so those are compared to a last-bit tolerance.
    for &scale in &[0.5, SCALE_MAX] {
        let e = effectors(scale);
        assert_eq!(
            e.mouth.x,
            scale * adult.mouth.x,
            "the mouth scales linearly, at {scale}"
        );
        assert_eq!(e.mouth.y, scale * adult.mouth.y);
        assert_eq!(e.near_claw.x, scale * adult.near_claw.x, "at {scale}");
        assert_eq!(e.near_claw.y, scale * adult.near_claw.y, "at {scale}");
        assert_eq!(e.far_claw.x, scale * adult.far_claw.x, "at {scale}");
        assert_eq!(e.far_claw.y, scale * adult.far_claw.y, "at {scale}");
    }
    for &scale in &[SCALE_MIN, 0.51, 0.6, 0.75, 0.93] {
        let e = effectors(scale);
        for (name, got, want) in [
            ("mouth.x", e.mouth.x, scale * adult.mouth.x),
            ("mouth.y", e.mouth.y, scale * adult.mouth.y),
            ("near_claw.x", e.near_claw.x, scale * adult.near_claw.x),
            ("near_claw.y", e.near_claw.y, scale * adult.near_claw.y),
            ("far_claw.x", e.far_claw.x, scale * adult.far_claw.x),
            ("far_claw.y", e.far_claw.y, scale * adult.far_claw.y),
        ] {
            assert!(
                (got - want).abs() <= 1e-15 * (1.0 + want.abs()),
                "at scale {scale}, {name} is {got} where linear scaling gives {want}"
            );
        }
    }

    // And the drawn claw is there (the four core-admitted scales, `SCALE_MIN` included, are
    // swept in `lanternjaw_scale.rs`).
    let parts = living(&settled(0.0));
    let origin = mid().chart();
    for &scale in &[0.5, SCALE_MAX] {
        let image = stamp_subset(&parts, &[PartName::NearLimb], mid(), forward(), scale);
        let claw = effectors(scale).near_claw;
        let mut nearest = f64::INFINITY;
        for (f, x, y) in every_pixel() {
            if image.get(f, x, y) == [0.0; 3] {
                continue;
            }
            assert_eq!(f, Face::Front, "the near limb left Front at scale {scale}");
            let b = Vec2::new(f64::from(x) + 0.5 - origin.x, f64::from(y) + 0.5 - origin.y);
            nearest = nearest.min((b - claw).length());
        }
        assert!(
            nearest <= 1.0,
            "at scale {scale} the nearest painted near-limb pixel is {nearest} px from the named \
             claw at ({}, {})",
            claw.x,
            claw.y
        );
    }
}

/// "`SCALE_MIN..=SCALE_MAX`; anything else, or a non-finite scale, is a configuration error that
/// panics in every build — the adapter clamps its mapping to the admitted range." Six rejected
/// scales, each caught on its own so that one swallowed case cannot hide behind another's panic.
/// A renderer that clamped silently would draw an adult where a broken size mapping asked for a
/// tenth-scale juvenile, and the contract's "validate juvenile contact … at the smallest admitted
/// size" would be testing the wrong body.
#[test]
fn draw_living_refuses_a_scale_outside_the_admitted_range() {
    let rig = Lanternjaw::new();
    let pose = quiet(0.4, 0.0);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let outcome: Vec<(f64, bool)> = [0.19, 1.01, 0.0, -0.5, f64::NAN, f64::INFINITY]
        .into_iter()
        .map(|scale| {
            let refused = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let mut canvas = Canvas::new();
                rig.draw_living(
                    &mut canvas,
                    mid(),
                    forward(),
                    &pose,
                    scale,
                    1.0,
                    &mut Vec::new(),
                    &mut Vec::new(),
                );
            }))
            .is_err();
            (scale, refused)
        })
        .collect();
    // And the admitted ends are *not* refused.
    let admitted: Vec<(f64, bool)> = [SCALE_MIN, 0.7, SCALE_MAX]
        .into_iter()
        .map(|scale| {
            let refused = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let mut canvas = Canvas::new();
                rig.draw_living(
                    &mut canvas,
                    mid(),
                    forward(),
                    &pose,
                    scale,
                    1.0,
                    &mut Vec::new(),
                    &mut Vec::new(),
                );
            }))
            .is_err();
            (scale, refused)
        })
        .collect();
    std::panic::set_hook(previous);

    for (scale, refused) in outcome {
        assert!(refused, "a scale of {scale} was drawn instead of refused");
    }
    for (scale, refused) in admitted {
        assert!(!refused, "the admitted scale {scale} was refused");
    }
}

/// The contract's seam requirement for a scaled body: "At vertices retain the renderer's localized
/// cut", and contact must agree with "the root-owned artwork" through "ordinary side/top seams
/// without inventing new face mappings".
///
/// The observable form is the unscaled suite's: at a **matched lattice phase** (fractional chart
/// coordinates `(0.5, 0.5)`, which a side/top seam's quarter turn leaves invariant) every
/// destination pixel has the same body displacement mid-face as it does at the seam, so the scaled
/// body samples the same art coordinates and carries the same total light. A scaled rig that
/// re-derived per-part chart images would lose or double material exactly here.
#[test]
fn a_scaled_body_straddling_a_seam_carries_the_same_light_as_mid_face() {
    let pose = settled(1.3);
    let mid_face = SurfacePoint::new(Face::Front, 32.5, 32.5);
    // 0.5 rather than `SCALE_MIN`: at 0.2 a body lying along a seam half a pixel away is
    // too short to reach the next face at all; `lanternjaw_scale.rs` crosses the seams heading
    // across them at every admitted scale.
    for &scale in &[0.6, 0.5, SCALE_MAX] {
        for (which, heading) in [
            ("+x", forward()),
            ("a diagonal", Vec2::new(0.6, -0.8)),
            ("45 degrees", Vec2::new(1.0, 1.0)),
        ] {
            let middle = total_light(&drawn_living(&pose, mid_face, heading, scale));
            assert!(
                middle > 1.0,
                "at scale {scale} the body must carry real light, got {middle}"
            );
            for (where_, root) in [
                (
                    "a Front/Right seam",
                    SurfacePoint::new(Face::Front, 63.5, 32.5),
                ),
                (
                    "a Front/Right seam, from Right",
                    SurfacePoint::new(Face::Right, 0.5, 32.5),
                ),
                (
                    "a Right/Top seam, a quarter turn",
                    SurfacePoint::new(Face::Right, 32.5, 0.5),
                ),
            ] {
                let image = drawn_living(&pose, root, heading, scale);
                let light = total_light(&image);
                assert!(
                    (light - middle).abs() / middle < 1e-6,
                    "{where_} heading {which} at scale {scale}: the body carried {light} where \
                     mid-face at the same lattice phase it carries {middle}"
                );
                assert!(
                    lit_faces(&image) >= 2,
                    "{where_} heading {which} at scale {scale}: the body did not reach the \
                     neighbouring face, so no seam was crossed"
                );
            }
        }
    }
}

/// "Never reflect an off-rim mouth/claw offset back onto the world. Static artwork clips at the
/// open bottom." A scaled body pointed at the open rim is **cut**, and the expectation is built the
/// second way: the same scaled body thirty rows up the same face. Both roots have integer chart
/// coordinates exactly thirty pixels apart, so a surviving pixel `(x, y)` and the flat body's
/// `(x, y − 30)` have the same body displacement bit for bit — the comparison is exact, not
/// approximate.
#[test]
fn a_scaled_body_over_the_open_rim_is_cut_and_never_reflected() {
    let pose = settled(1.3);
    // Heading `+y` is image-down, so the body's front (and its extended claw) points at the rim.
    let heading = Vec2::new(0.0, 1.0);
    for &scale in &[0.6, SCALE_MIN] {
        let rim = drawn_living(
            &pose,
            SurfacePoint::new(Face::Front, 32.0, 62.0),
            heading,
            scale,
        );
        let flat = drawn_living(
            &pose,
            SurfacePoint::new(Face::Front, 32.0, 32.0),
            heading,
            scale,
        );
        assert!(
            (34..64u8).any(|y| (0..64u8).any(|x| flat.get(Face::Front, x, y) != [0.0; 3])),
            "at scale {scale} the body does not reach past where the rim is, so nothing is cut"
        );
        assert!(
            (0..64u8).any(|x| rim.get(Face::Front, x, 63) != [0.0; 3]),
            "at scale {scale} the body at the rim painted nothing on the last row that exists"
        );
        for (f, x, y) in every_pixel() {
            let want = if f == Face::Front && y >= 30 {
                flat.get(Face::Front, x, y - 30)
            } else {
                [0.0; 3]
            };
            assert_eq!(
                rim.get(f, x, y),
                want,
                "at scale {scale}, over the rim, ({f:?}, {x}, {y}) is {:?} where the same body \
                 thirty rows up has {want:?} — a reflected or clamped body, not a cut one",
                rim.get(f, x, y)
            );
        }
    }
}

/// The contract asks for juvenile evidence at "fractional roots". A scaled body must move
/// fractionally too: a whole-pixel anchor shift is a whole-pixel translation of the image (bit for
/// bit, because every destination pixel's body displacement is the same double one column over),
/// a half-pixel shift is a *different* picture (so the anchor is not rounded), and it differs by
/// sub-pixel brightness only.
///
/// The bound is derived, as the gallery's own fractional-anchor test derives it: for one part on
/// one layer over black at opacity 1 the canvas value is the clamped bilinear sample, whose slope
/// along the chart is the art's slope divided by `scale`, so a half-pixel chart shift can move a
/// pixel by at most `0.5 · J / scale` with `J` the largest adjacent-texel step of that part's own
/// sprite.
#[test]
fn a_fractional_anchor_moves_a_scaled_body_by_sub_pixel_brightness_only() {
    let pose = quiet(0.4, 1.0);
    let scale = 0.7;
    let at_u = |u: f64| {
        drawn_living(
            &pose,
            SurfacePoint::new(Face::Front, u, 32.5),
            forward(),
            scale,
        )
    };
    let (base, shifted) = (at_u(32.5), at_u(33.5));
    for y in 0..64u8 {
        for x in 1..64u8 {
            assert_eq!(
                shifted.get(Face::Front, x, y),
                base.get(Face::Front, x - 1, y),
                "a one-pixel anchor shift must translate the scaled image: Front ({x}, {y})"
            );
        }
    }
    let half = at_u(33.0);
    assert!(
        max_diff(&half, &base) > 0.01 && max_diff(&half, &shifted) > 0.01,
        "a half-pixel anchor shift changed nothing at scale {scale} — the anchor is being rounded"
    );

    let parts = living(&pose);
    for name in [PartName::Head, PartName::Tail, PartName::NearLimb] {
        let part = part_of(&parts, name);
        let mut jump = 0.0f32;
        for ty in 0..part.sprite.height() as i32 {
            for tx in -1..part.sprite.width() as i32 {
                let (a, b) = (part.sprite.texel(tx, ty), part.sprite.texel(tx + 1, ty));
                for c in 0..3 {
                    jump = jump.max((a[c] - b[c]).abs());
                }
            }
        }
        let one = |u: f64| {
            stamp_subset(
                &parts,
                &[name],
                SurfacePoint::new(Face::Front, u, 32.5),
                forward(),
                scale,
            )
        };
        let moved = max_diff(&one(32.5), &one(33.0));
        let bound = 0.5 * f64::from(jump) / scale;
        assert!(
            f64::from(moved) <= bound + 1e-6,
            "{name:?}: a half-pixel anchor shift moved a scaled pixel by {moved}, more than the \
             bilinear slope allows ({bound})"
        );
        assert!(
            moved > 1e-4,
            "{name:?}: a half-pixel shift moved nothing ({moved})"
        );
    }
}

// ---------------------------------------------------------------------------
// 7. purity: one instant, one picture, whatever the frame rate
// ---------------------------------------------------------------------------

/// The contract's "30/60/120-Hz common-instant identity, pause and tick-zero hold" and the
/// gallery's own purity claim, restated for the living body: `parts_living` is a pure function of
/// its pose, so the same instant reached through a 30, 60 or 120 Hz schedule is the same picture,
/// a repeated draw is the same image, and pausing holds the pose.
///
/// The pose swept here carries an attack as well as ambient time — a host running at a different
/// rate feeds a different *sequence* of elapsed times, and if any of them advanced hidden state the
/// common instants would diverge.
#[test]
fn the_living_body_is_a_pure_function_of_its_pose_at_every_frame_rate() {
    let rig = Lanternjaw::new();
    let (mut a, mut b, mut c) = (Vec::new(), Vec::new(), Vec::new());
    for i in 1..=24 {
        let pose = |t: f64| LivingPose {
            ambient: t,
            movement: 0.35,
            attack: Some(strike_episode(t)),
            gut: 0.4,
            cocoon: Some(0.12),
        };
        let (t30, t60, t120) = (
            f64::from(i) / 30.0,
            f64::from(2 * i) / 60.0,
            f64::from(4 * i) / 120.0,
        );
        rig.parts_living(&pose(t30), &mut a);
        rig.parts_living(&pose(t60), &mut b);
        rig.parts_living(&pose(t120), &mut c);
        assert_parts_identical(&a, &b, &format!("the instant {t30} at 30 Hz vs 60 Hz"));
        assert_parts_identical(&a, &c, &format!("the instant {t30} at 30 Hz vs 120 Hz"));
        // Asking for another frame in between must not disturb the answer.
        rig.parts_living(&pose(t30 + 0.37), &mut b);
        rig.parts_living(&pose(t30), &mut b);
        assert_parts_identical(&a, &b, &format!("a repeated pose at {t30}"));
    }

    // And the drawn image repeats, with the scratch buffers reused for something else in between.
    for (what, anchor, scale) in [
        ("mid-face, adult", mid(), SCALE_MAX),
        (
            "a seam, juvenile",
            SurfacePoint::new(Face::Front, 63.5, 32.5),
            SCALE_MIN,
        ),
    ] {
        let pose = settled(2.5);
        let mut parts = Vec::new();
        let mut scratch = Vec::new();
        let mut first = filled([0.03, 0.02, 0.07]);
        let mut second = filled([0.03, 0.02, 0.07]);
        rig.draw_living(
            &mut first,
            anchor,
            Vec2::new(0.6, -0.8),
            &pose,
            scale,
            0.9,
            &mut parts,
            &mut scratch,
        );
        let mut junk = Canvas::new();
        rig.draw_living(
            &mut junk,
            mid(),
            forward(),
            &quiet(1.0, 1.0),
            0.8,
            1.0,
            &mut parts,
            &mut scratch,
        );
        rig.draw_living(
            &mut second,
            anchor,
            Vec2::new(0.6, -0.8),
            &pose,
            scale,
            0.9,
            &mut parts,
            &mut scratch,
        );
        assert_identical(
            &first,
            &second,
            &format!("a repeated living draw at {what}"),
        );
        assert!(
            max_diff(&first, &filled([0.03, 0.02, 0.07])) > 0.01,
            "{what}: nothing was drawn"
        );
    }
}

/// "Ambient time … for the **ambient** rhythms only: the body wave, the lantern crest, the ambient
/// blink, the gait phase, the cocoon and gut breaths. **An attack never reads it.**" The contract
/// says the same twice: "attack channels read the authoritative attack episode and progress, not
/// modulo time", and "Do not freeze every ambient rhythm by feeding a synthetic hunt timestamp to
/// the whole body."
///
/// The sharp form: two poses that differ **only** in their ambient seconds produce `Channels` that
/// differ only in `t`. Every attack-derived field — the near and far reach, the compression, the
/// lunge, the accent, the blink the attack contributes and the hush-derived pulse gain and wave
/// amplitudes — is therefore identical, while the rhythms those channels drive still run on the
/// ambient clock (asserted by the pictures differing).
#[test]
fn ambient_time_never_advances_the_attack() {
    let with_attack = |ambient: f64| LivingPose {
        ambient,
        movement: 0.25,
        attack: Some(strike_episode(0.93)),
        gut: 0.5,
        cocoon: Some(0.15),
    };
    let reference = Channels::living(&with_attack(0.0));
    for &ambient in &[0.5, 2.0, 7.31, 13.0, -4.5] {
        let mut other = Channels::living(&with_attack(ambient));
        assert_eq!(other.t, ambient, "the channels' `t` is the ambient clock");
        other.t = reference.t;
        assert_eq!(
            other, reference,
            "an ambient shift to {ambient} changed something other than the ambient clock"
        );
    }
    // The ambient rhythms do keep running: the same episode at two ambient instants is a
    // different picture (the wave, the crest and the breaths moved), so the identity above is not
    // the trivial one of a frozen body.
    let (a, b) = (living(&with_attack(0.0)), living(&with_attack(0.37)));
    assert!(
        parts_differ(&a, &b, PartName::Abdomen) || parts_differ(&a, &b, PartName::Tail),
        "the ambient rhythms were frozen by the attack: the body did not move at all"
    );
    // A non-finite ambient instant reads as 0, as the gallery's does.
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut ch = Channels::living(&with_attack(bad));
        assert_eq!(ch.t, 0.0, "a non-finite ambient instant reads as 0");
        ch.t = reference.t;
        assert_eq!(
            ch, reference,
            "a non-finite ambient instant changed more than the clock"
        );
    }
}
