//! The opt-in art image for the live world (`cubarium run --art <dir>`).
//!
//! Same contract as [`crate::present::Presenter`] — [`ArtPresenter::observe`] once per
//! completed tick, [`ArtPresenter::draw`] once per rendered frame — but the bodies are
//! the authored sprite clips of an [`ArtPack`] instead of procedural discs, and the
//! producer substrate grows habitat motifs where it is rich.
//!
//! What this presenter keeps from the decided M2 image: the night floor, the producer
//! ramp with its saturation point and squared brightness, and the detritus flecks,
//! drawn with exactly the arguments [`crate::present::Presenter::draw`] uses. What it
//! drops: trails, disc lobes, and the warm feeding flash. The clips carry that
//! expression, and stacking both reads as two creatures on top of each other.
//!
//! Nothing here is wall-clock driven. Looping clips advance on *simulated* time
//! (`view.tick × DT`), so `--speed 8` animates eight times faster and a paused world
//! holds its pose, and the bud clip advances on the organism's own gestation progress.

use cubarium_core::OrganismId;
use cubarium_core::organism::Mode;
use cubarium_core::view::{OrganismView, RenderView};
use cubarium_render::{Canvas, draw_field, stamp_sprite};
use cubarium_surface::{CELL_COUNT, CellId, PixelImage, ScalarField, SurfacePoint, Vec2};

use crate::art::{ArtPack, Clip};
use crate::clock::DT;
use crate::present::{
    self, DETRITUS_SCALE, DETRITUS_THRESHOLD, JUVENILE_SCALE, PALETTE, PRODUCER_SATURATION,
};
use crate::rng::SplitMix64;

// --- Motif constants ---------------------------------------------------------------
//
// All three are review-tunable: they set how much producer biomass it takes before the
// substrate grows visible scenery and how loud that scenery is. Change them from what a
// viewing session says, not from what a test prefers.

/// Producer density (as a fraction of the ramp's saturation point) below which a cell
/// grows no motif at all: motifs mark rich ground, they are not a background texture.
/// Review-tunable.
pub const MOTIF_THRESHOLD: f64 = 0.35;
/// The density at which a motif reaches its full opacity. Review-tunable.
pub const MOTIF_FULL: f64 = 0.8;
/// The opacity a motif reaches at [`MOTIF_FULL`] — under 1 so authored dark outlines
/// never read as a hard cutout over the substrate. Review-tunable.
pub const MOTIF_OPACITY: f32 = 0.85;

/// Seeds the per-cell motif hash. Any fixed value works; this one keeps the placement
/// stream separate from every other `SplitMix64` stream in the host.
const MOTIF_SEED: u64 = 0x6D6F_7469_6600_0001;
/// Seeds the per-organism clip-phase hash, for the same reason.
const PHASE_SEED: u64 = 0x7068_6173_6500_0001;

/// A cell's motif: which habitat sprite, where in the cell, and facing where. Fixed for
/// the life of the presenter; only its opacity moves with the producer field.
#[derive(Clone, Copy, Debug)]
struct Motif {
    kind: usize,
    at: SurfacePoint,
    heading: Vec2,
}

/// Which rig draws an organism, from its inherited cosmetic hue.
///
/// **Normative**: `form = min(2, floor(hue × 3))`, and a `NaN` hue is form 0.
///
/// `hue` is a cosmetic gene copied exactly at birth, so a lineage keeps its rig for as
/// long as it survives and a bud looks like its parent. This is a *rig per hue tercile*
/// and nothing more: it is not a species, not a diet, not a capability, and the world
/// does not know it exists. Two organisms with the same rig differ in every way the
/// ecology actually models.
pub fn form_of(hue: f32) -> usize {
    if hue.is_nan() {
        return 0;
    }
    // A hue outside `[0, 1]` is not something the genome produces; clamping rather than
    // wrapping keeps an out-of-range value on an end rig instead of panicking on a cast.
    ((hue * 3.0).floor() as i64).clamp(0, 2) as usize
}

/// Which clip an organism's state selects.
///
/// **Normative**, in this order: an organism holding an escrow is budding
/// (`bud`, state 3) whatever else it is doing; otherwise [`Mode::Feeding`] is `feed`
/// (2), [`Mode::Seeking`] is `move` (1), and [`Mode::Resting`] is `rest` (0). Gestation
/// beats mode because a birth is the rarer and more legible event.
pub fn state_of(o: &OrganismView) -> usize {
    if o.gestation.is_some() {
        return 3;
    }
    match o.mode {
        Mode::Resting => 0,
        Mode::Seeking => 1,
        Mode::Feeding => 2,
    }
}

/// A stable per-organism offset into a looping clip, in `[0, seconds)`.
///
/// **Normative**: a hash of the organism's `OrganismId` (slot *and* generation, so a
/// recycled slot is a new body with a new phase). Without it every organism on the cube
/// would breathe in lockstep, which reads as a machine rather than as a population.
/// A non-finite or non-positive `seconds` yields 0.
pub fn phase_of(id: OrganismId, seconds: f64) -> f64 {
    if !(seconds.is_finite() && seconds > 0.0) {
        return 0.0;
    }
    let mut hash = SplitMix64::new(
        PHASE_SEED ^ (u64::from(id.slot) << 32) ^ u64::from(id.generation).wrapping_mul(GENERATION_ODD),
    );
    // `next_f64` is in `[0, 1)`, so the phase never lands exactly on `seconds`.
    hash.next_f64() * seconds
}

/// An odd multiplier so `slot` and `generation` cannot cancel each other out in the
/// phase hash.
const GENERATION_ODD: u64 = 0x9E37_79B9_7F4A_7C15;

/// Where in a clip to sample this frame.
///
/// **Normative**: the non-looping `bud` clip is driven by the organism's own gestation
/// progress — `progress × clip.seconds`, so the clip's last frame lands exactly when the
/// world commits the birth. Every looping clip is driven by *simulated* time,
/// `tick × DT + phase`, never by wall time: `--speed` and pauses then stay honest, and
/// two frames of the same tick show the same pose.
///
/// A non-looping clip with no gestation cannot arise from [`state_of`]; it yields 0.
pub fn clip_time(clip: &Clip, tick: u64, phase: f64, gestation: Option<f32>) -> f64 {
    if clip.looping {
        return tick as f64 * DT + phase;
    }
    match gestation {
        Some(progress) => f64::from(progress) * clip.seconds,
        None => 0.0,
    }
}

/// The live world drawn with the baked art. Holds the pack, the scratch buffers the
/// field and sprite paths need, and the fixed per-cell motif placement.
pub struct ArtPresenter {
    pack: ArtPack,
    producer: ScalarField,
    detritus: ScalarField,
    scratch: Vec<PixelImage>,
    /// One entry per field cell, in `CellId` index order. Placement never changes; only
    /// whether and how brightly it is drawn does.
    motifs: Vec<Motif>,
}

impl ArtPresenter {
    /// Build the presenter and lay out every cell's motif once.
    pub fn new(pack: ArtPack) -> ArtPresenter {
        let motifs = CellId::all().map(motif_of).collect();
        ArtPresenter {
            pack,
            producer: ScalarField::zeros(),
            detritus: ScalarField::zeros(),
            scratch: Vec::new(),
            motifs,
        }
    }

    /// The loaded pack, for tests that want to compare a drawn body against its sprite.
    pub fn pack(&self) -> &ArtPack {
        &self.pack
    }

    /// Record one completed tick. The art image keeps no renderer-side history — there
    /// are no trails to feed — so this does nothing; it exists so the runner can call
    /// the same two methods on either presenter at the same two sites.
    pub fn observe(&mut self, _view: &RenderView) {}

    /// Draw the art image: floor, producer ramp, detritus flecks, habitat motifs,
    /// bodies, in that order. `f` is the clock's interpolation fraction, used exactly as
    /// [`crate::present::Presenter::draw`] uses it.
    pub fn draw(&mut self, view: &RenderView, f: f64, canvas: &mut Canvas) {
        canvas.clear();
        present::draw_floor(canvas);

        // The decided producer ramp, with the same arguments the M2 presenter passes.
        present::copy_field(&mut self.producer, &view.producer);
        let saturation = view.producer_max * PRODUCER_SATURATION;
        present::draw_ramp_field(
            canvas,
            &self.producer,
            saturation,
            PALETTE.producer_low,
            PALETTE.producer_high,
            true,
        );

        // Detritus flecks, exactly as the M2 presenter draws them.
        present::threshold_field(&mut self.detritus, &view.detritus, DETRITUS_THRESHOLD);
        draw_field(canvas, &self.detritus, DETRITUS_SCALE, PALETTE.detritus, false);

        // Habitat motifs: scenery that follows the producer field. These are not
        // organisms — nothing in the world knows about them, they never move, and they
        // are not eaten. They are how a rich cell reads as overgrown rather than as
        // merely brighter.
        if saturation.is_finite() && saturation > 0.0 {
            for (index, motif) in self.motifs.iter().enumerate() {
                let t = (view.producer.get(index).copied().unwrap_or(0.0) / saturation).min(1.0);
                let opacity = motif_opacity(t);
                if opacity <= 0.0 {
                    continue;
                }
                stamp_sprite(
                    canvas,
                    motif.at,
                    motif.heading,
                    &self.pack.habitat[motif.kind],
                    1.0,
                    opacity,
                    &mut self.scratch,
                );
            }
        }

        // Bodies: one clip frame each, driven by the organism's real state.
        for o in &view.organisms {
            let form = form_of(o.hue);
            let state = state_of(o);
            let clip = &self.pack.clips[form * 4 + state];
            let phase = phase_of(o.id, clip.seconds);
            let sprite = clip.at(clip_time(clip, view.tick, phase, o.gestation));
            let (anchor, heading) = present::interpolate(&o.moved, o.pos, o.heading, f);
            let scale = if o.juvenile { JUVENILE_SCALE } else { 1.0 };
            stamp_sprite(canvas, anchor, heading, sprite, scale, 1.0, &mut self.scratch);
        }
    }
}

/// A motif's opacity at producer density `t` (already a fraction of the ramp's
/// saturation point): nothing at or below [`MOTIF_THRESHOLD`], ramping to
/// [`MOTIF_OPACITY`] at [`MOTIF_FULL`] and holding there.
pub fn motif_opacity(t: f64) -> f32 {
    // `NaN` and everything at or below the threshold grow nothing; the comparison is
    // strict, so a cell sitting exactly on the threshold is still bare ground.
    if t.is_nan() || t <= MOTIF_THRESHOLD {
        return 0.0;
    }
    let ramp = ((t - MOTIF_THRESHOLD) / (MOTIF_FULL - MOTIF_THRESHOLD)).clamp(0.0, 1.0);
    ramp as f32 * MOTIF_OPACITY
}

/// One cell's fixed motif: kind, jittered position and heading, all from a hash of the
/// cell index so the scenery is identical run to run and never crawls between frames.
fn motif_of(cell: CellId) -> Motif {
    let mut hash = SplitMix64::new(MOTIF_SEED ^ cell.index() as u64);
    // The kind is the cell's hash mod 3 — rosette, fern, lichen.
    let kind = (hash.next_u64() % 3) as usize;
    // Jitter stays within ±1 px of the cell center, which is 2 px from every cell edge,
    // so a motif never anchors in a neighbouring cell.
    let center = cell.center();
    let at = SurfacePoint::new(
        center.face,
        center.u + hash.range(-1.0, 1.0),
        center.v + hash.range(-1.0, 1.0),
    );
    let heading = Vec2::from_screen_angle(hash.range(0.0, std::f64::consts::TAU));
    Motif { kind, at, heading }
}

/// Compile-time reminder that the motif table is one entry per field cell.
const _: () = assert!(CELL_COUNT == 1280);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::present::Presenter;
    use cube_proto::Face;
    use cubarium_surface::cell_of;
    use std::path::Path;

    fn pack() -> ArtPack {
        ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"))
            .expect("the baked art pack")
    }

    fn organism(slot: u32, hue: f32, mode: Mode) -> OrganismView {
        OrganismView {
            id: OrganismId { slot, generation: 1 },
            pos: SurfacePoint::new(Face::Front, 32.0, 32.0),
            heading: Vec2::new(1.0, 0.0),
            lobes: vec![(0.0, 0.0, 1.4), (2.0, 0.0, 0.9)],
            hue,
            mode,
            fed: false,
            juvenile: false,
            gestation: None,
            moved: Vec::new(),
        }
    }

    fn empty_view() -> RenderView {
        RenderView {
            tick: 0,
            producer: vec![0.0; CELL_COUNT],
            detritus: vec![0.0; CELL_COUNT],
            producer_max: 2.0,
            organisms: Vec::new(),
        }
    }

    /// Every pixel of both canvases, compared exactly.
    fn identical(a: &Canvas, b: &Canvas) -> bool {
        Face::ALL.into_iter().all(|face| {
            (0..64u8).all(|y| (0..64u8).all(|x| a.get(face, x, y) == b.get(face, x, y)))
        })
    }

    /// Pixels of `after` that differ from `before`, as (face, x, y).
    fn added(before: &Canvas, after: &Canvas) -> Vec<(Face, u8, u8)> {
        let mut out = Vec::new();
        for face in Face::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    if before.get(face, x, y) != after.get(face, x, y) {
                        out.push((face, x, y));
                    }
                }
            }
        }
        out
    }

    #[test]
    fn the_form_is_the_hue_tercile_and_never_leaves_the_three_rigs() {
        assert_eq!(form_of(0.0), 0);
        // The 0/1 boundary is exactly 1/3: just below is rig 0, at and above is rig 1.
        assert_eq!(form_of(0.333), 0);
        assert_eq!(form_of(0.34), 1);
        assert_eq!(form_of(0.99), 2);
        assert_eq!(form_of(1.0), 2, "min(2, floor(3)) is 2, not an out-of-range rig");
        assert_eq!(form_of(f32::NAN), 0);
        // Values the genome does not produce still land on a real rig.
        assert_eq!(form_of(-0.5), 0);
        assert_eq!(form_of(5.0), 2);
        assert_eq!(form_of(f32::INFINITY), 2);
        assert_eq!(form_of(f32::NEG_INFINITY), 0);
    }

    #[test]
    fn gestation_beats_mode_and_every_mode_maps_to_its_clip() {
        for (mode, want) in
            [(Mode::Resting, 0), (Mode::Seeking, 1), (Mode::Feeding, 2)]
        {
            let mut o = organism(0, 0.0, mode);
            assert_eq!(state_of(&o), want, "{mode:?}");
            o.gestation = Some(0.0);
            assert_eq!(state_of(&o), 3, "a budding {mode:?} organism draws the bud clip");
        }
    }

    #[test]
    fn the_clip_phase_is_stable_per_id_bounded_and_not_shared() {
        let a = OrganismId { slot: 3, generation: 1 };
        let b = OrganismId { slot: 4, generation: 1 };
        let recycled = OrganismId { slot: 3, generation: 2 };
        assert_eq!(phase_of(a, 2.0), phase_of(a, 2.0), "the phase is a pure hash");
        assert_ne!(phase_of(a, 2.0), phase_of(b, 2.0), "two slots must not animate together");
        assert_ne!(phase_of(a, 2.0), phase_of(recycled, 2.0), "a reused slot is a new body");
        for slot in 0..256u32 {
            let p = phase_of(OrganismId { slot, generation: 7 }, 2.5);
            assert!((0.0..2.5).contains(&p), "slot {slot}: {p}");
        }
        assert_eq!(phase_of(a, 0.0), 0.0);
        assert_eq!(phase_of(a, f64::NAN), 0.0);
    }

    #[test]
    fn the_bud_clip_follows_gestation_and_looping_clips_follow_simulated_time() {
        let art = pack();
        let bud = &art.clips[3];
        assert!(!bud.looping);
        assert_eq!(clip_time(bud, 0, 0.0, Some(0.0)), 0.0);
        assert_eq!(clip_time(bud, 12_345, 1.7, Some(0.5)), 0.5 * bud.seconds);
        assert_eq!(clip_time(bud, 12_345, 1.7, Some(1.0)), bud.seconds);
        // Wall time is nowhere in it: the tick and the phase are the only inputs.
        let walk = &art.clips[1];
        assert!(walk.looping);
        assert_eq!(clip_time(walk, 0, 0.25, None), 0.25);
        assert_eq!(clip_time(walk, 20, 0.25, None), 20.0 * DT + 0.25);
        // Gestation is ignored by a looping clip, and a doubled tick rate doubles the
        // clip's progress, which is what makes `--speed` honest.
        assert_eq!(clip_time(walk, 40, 0.0, Some(0.5)), 40.0 * DT);
    }

    #[test]
    fn a_quiet_world_draws_exactly_what_the_m2_presenter_draws() {
        // No organisms, and every cell below the motif threshold: the art image is then
        // the decided M2 image with nothing added.
        let mut view = empty_view();
        let below = view.producer_max * PRODUCER_SATURATION * MOTIF_THRESHOLD;
        for (i, v) in view.producer.iter_mut().enumerate() {
            *v = below * (i % 7) as f64 / 7.0;
        }
        for (i, v) in view.detritus.iter_mut().enumerate() {
            *v = 0.3 * (i % 5) as f64;
        }
        let mut plain = Canvas::new();
        let mut art = Canvas::new();
        Presenter::new().draw(&view, 0.0, &mut plain);
        ArtPresenter::new(pack()).draw(&view, 0.0, &mut art);
        assert!(identical(&plain, &art), "the art image added something below the threshold");

        // And a cell exactly at the threshold is still nothing: the rule is strict.
        for v in view.producer.iter_mut() {
            *v = below;
        }
        Presenter::new().draw(&view, 0.0, &mut plain);
        ArtPresenter::new(pack()).draw(&view, 0.0, &mut art);
        assert!(identical(&plain, &art), "a cell at MOTIF_THRESHOLD grew a motif");
    }

    #[test]
    fn a_rich_cell_grows_a_motif_and_a_poor_one_does_not() {
        let mut view = empty_view();
        let saturation = view.producer_max * PRODUCER_SATURATION;
        let cell = cubarium_surface::CellId::new(Face::Front, 8, 8);

        // Just below the threshold: the art image is the ramp and nothing else.
        view.producer[cell.index()] = saturation * (MOTIF_THRESHOLD - 1e-6);
        let mut plain = Canvas::new();
        let mut art = Canvas::new();
        Presenter::new().draw(&view, 0.0, &mut plain);
        ArtPresenter::new(pack()).draw(&view, 0.0, &mut art);
        assert!(identical(&plain, &art), "a cell below MOTIF_THRESHOLD added scenery");

        // Above MOTIF_FULL: the motif lights pixels the ramp alone does not, and they
        // are in or next to that cell rather than scattered over the cube.
        view.producer[cell.index()] = saturation * (MOTIF_FULL + 0.1);
        Presenter::new().draw(&view, 0.0, &mut plain);
        ArtPresenter::new(pack()).draw(&view, 0.0, &mut art);
        let lit = added(&plain, &art);
        assert!(!lit.is_empty(), "a saturated cell drew no motif");
        assert!(
            lit.iter().any(|&(f, x, y)| cell_of(&SurfacePoint::pixel_center(f, x, y)) == cell),
            "the motif must be inside its own cell: {lit:?}"
        );
        for &(f, x, y) in &lit {
            let at = cell_of(&SurfacePoint::pixel_center(f, x, y));
            let near = at.face() == cell.face()
                && at.cx().abs_diff(cell.cx()) <= 3
                && at.cy().abs_diff(cell.cy()) <= 3;
            assert!(near, "a motif for {cell:?} lit {f:?} {x},{y} in {at:?}");
        }
    }

    #[test]
    fn the_motif_opacity_ramp_starts_at_the_threshold_and_saturates() {
        assert_eq!(motif_opacity(0.0), 0.0);
        assert_eq!(motif_opacity(MOTIF_THRESHOLD), 0.0);
        assert_eq!(motif_opacity(MOTIF_FULL), MOTIF_OPACITY);
        assert_eq!(motif_opacity(1.0), MOTIF_OPACITY);
        let mid = motif_opacity((MOTIF_THRESHOLD + MOTIF_FULL) / 2.0);
        assert!((mid - MOTIF_OPACITY / 2.0).abs() < 1e-6, "{mid}");
        assert_eq!(motif_opacity(f64::NAN), 0.0);
    }

    #[test]
    fn motifs_stay_inside_their_own_cell_and_use_all_three_kinds() {
        let mut kinds = [0usize; 3];
        for cell in CellId::all() {
            let m = motif_of(cell);
            kinds[m.kind] += 1;
            assert_eq!(m.at.face, cell.face());
            assert_eq!(cell_of(&m.at), cell, "{cell:?} anchored its motif in another cell");
            assert!((m.heading.length() - 1.0).abs() < 1e-9, "{cell:?}: {:?}", m.heading);
            // Determinism: the same cell always lays out the same motif.
            let again = motif_of(cell);
            assert_eq!((again.kind, again.at, again.heading.x), (m.kind, m.at, m.heading.x));
        }
        assert!(kinds.iter().all(|&n| n > 300), "all three motifs must appear: {kinds:?}");
    }

    #[test]
    fn each_state_draws_a_body_near_the_organism_and_bud_ends_on_the_last_frame() {
        let art = pack();
        for (mode, gestation) in [
            (Mode::Resting, None),
            (Mode::Seeking, None),
            (Mode::Feeding, None),
            (Mode::Resting, Some(1.0f32)),
        ] {
            let mut view = empty_view();
            let mut o = organism(0, 0.5, mode);
            o.gestation = gestation;
            view.organisms = vec![o];
            let mut plain = Canvas::new();
            let mut canvas = Canvas::new();
            // The floor and (empty) fields alone, to subtract.
            Presenter::new().draw(&empty_view(), 0.0, &mut plain);
            ArtPresenter::new(pack()).draw(&view, 0.0, &mut canvas);
            let lit = added(&plain, &canvas);
            assert!(!lit.is_empty(), "{mode:?}/{gestation:?} drew no body");
            for &(f, x, y) in &lit {
                assert_eq!(f, Face::Front, "{mode:?}: a body leaked onto {f:?}");
                let d = (f64::from(x) - 32.0).hypot(f64::from(y) - 32.0);
                assert!(d < 10.0, "{mode:?}: lit {x},{y}, {d:.1} px from the body");
            }
        }

        // A gestation of 1.0 is the bud clip's last frame, exactly.
        let bud = &art.clips[form_of(0.5) * 4 + 3];
        let at_birth = clip_time(bud, 0, 0.0, Some(1.0));
        assert!(std::ptr::eq(bud.at(at_birth), bud.frames.last().unwrap()));
        assert!(std::ptr::eq(
            bud.at(at_birth),
            art.creature(form_of(0.5), 3, bud.seconds),
        ));
        // And a gestation of 0 is its first frame, so the clip really does run.
        assert!(std::ptr::eq(bud.at(clip_time(bud, 0, 0.0, Some(0.0))), &bud.frames[0]));
    }

    #[test]
    fn a_juvenile_is_the_same_clip_drawn_smaller() {
        let mut view = empty_view();
        let mut o = organism(0, 0.9, Mode::Seeking);
        o.juvenile = true;
        view.organisms = vec![o.clone()];
        let mut small = Canvas::new();
        ArtPresenter::new(pack()).draw(&view, 0.0, &mut small);

        o.juvenile = false;
        view.organisms = vec![o];
        let mut full = Canvas::new();
        ArtPresenter::new(pack()).draw(&view, 0.0, &mut full);

        let floor = {
            let mut c = Canvas::new();
            Presenter::new().draw(&empty_view(), 0.0, &mut c);
            c
        };
        let (a, b) = (added(&floor, &small).len(), added(&floor, &full).len());
        assert!(a > 0 && a < b, "a juvenile lit {a} pixels, an adult {b}");
        assert_eq!(JUVENILE_SCALE, 0.7);
    }

    /// Not a correctness test: the number the brief asks for. Run with
    /// `cargo test --release -p cubarium -- --ignored motif_and_body_draw_cost`.
    #[test]
    #[ignore = "timing, not behaviour"]
    fn motif_and_body_draw_cost() {
        let mut view = empty_view();
        // Every cell over MOTIF_FULL, so all 1,280 motifs stamp.
        for v in view.producer.iter_mut() {
            *v = view.producer_max;
        }
        for (i, v) in view.detritus.iter_mut().enumerate() {
            *v = 0.2 + (i % 11) as f64 * 0.1;
        }
        let mut rng = SplitMix64::new(11);
        view.organisms = (0..200u32)
            .map(|slot| {
                let mut o = organism(slot, rng.next_f64() as f32, Mode::Seeking);
                let face = cube_proto::Face::ALL[(slot as usize) % 5];
                o.pos = SurfacePoint::new(face, rng.range(1.0, 63.0), rng.range(1.0, 63.0));
                o.juvenile = slot % 3 == 0;
                o.gestation = (slot % 5 == 0).then_some(0.5);
                o
            })
            .collect();

        let mut presenter = ArtPresenter::new(pack());
        let mut canvas = Canvas::new();
        // Warm the caches, then time a run of frames.
        for _ in 0..5 {
            presenter.draw(&view, 0.0, &mut canvas);
        }
        let frames = 60;
        let t0 = std::time::Instant::now();
        for i in 0..frames {
            view.tick = i;
            presenter.draw(&view, 0.5, &mut canvas);
        }
        let per = t0.elapsed().as_secs_f64() / frames as f64;
        println!(
            "ArtPresenter::draw: {:.3} ms/frame (1280 motifs, 200 organisms) = {:.0}% of a 60 fps budget",
            per * 1e3,
            per / (1.0 / 60.0) * 100.0
        );
        assert!(per < 1.0 / 60.0, "draw took {:.3} ms, past the whole 60 fps budget", per * 1e3);
    }
}
