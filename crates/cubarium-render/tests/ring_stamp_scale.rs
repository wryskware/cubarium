//! FW-6: the stamp budget at world scale, written from
//! `design/flat-world-plan-2026-09-16.md` §2 (`footprint_radius() = 9·S`, FW-1 owns the
//! value), §9's FW-3 row ("**adopt** `Scale::footprint_radius()` at both check sites …
//! a `scale = 2` stamp draws instead of vanishing") and the FW-1 freeze.
//!
//! **Status at the time of writing.** FW-3 has shaped the canvas by topology, so stamps
//! now run on a ring, but it has *not* adopted the scale at the two budget check sites:
//! `sprite.rs` still compares `extent · scale` against a private `FOOTPRINT_RADIUS = 9.0`.
//! The budget value and the `S = 1` behaviour are pinned here in full; the `S = 2` stamp is
//! written and `#[ignore]`d, and is the test for the remaining adoption.

use cubarium_render::{Canvas, Sprite, stamp_sprite};
use cubarium_surface::{FOOTPRINT_PIXELS, Face, Scale, SurfacePoint, Topology, Vec2};

/// A square RGBA sprite, fully opaque white, pivoted at its centre.
fn solid(side: usize) -> Result<Sprite, String> {
    let bytes = vec![255u8; side * side * 4];
    let half = side as f64 / 2.0;
    Sprite::from_rgba(side, side, Vec2::new(half, half), &bytes)
}

/// Half a pixel diagonal plus half a pixel of bilinear support — the distance beyond a
/// painted texel's centre that stamping it can still touch. Written out independently.
const TEXEL_SUPPORT: f64 = std::f64::consts::FRAC_1_SQRT_2 + 0.5;

/// The extent the plan's rule gives a solid square of `side` texels pivoted at its centre:
/// the far corner texel's centre offset, plus the support.
fn expected_extent(side: usize) -> f64 {
    let d = side as f64 / 2.0 - 0.5;
    d.hypot(d) + TEXEL_SUPPORT
}

#[test]
fn the_stamp_budget_is_nine_pixels_times_the_world_scale() {
    assert_eq!(FOOTPRINT_PIXELS, 9.0, "the budget at S = 1");
    assert_eq!(Scale::ONE.footprint_radius(), 9.0);
    assert_eq!(Scale::new(2.0).footprint_radius(), 18.0, "9·S");
    assert_eq!(Scale::new(0.5).footprint_radius(), 4.5);
    assert_eq!(Scale::default(), Scale::ONE, "the default world scale is 1");
}

/// The budget is not a tunable: it must fit inside the radius the shared unfolding is
/// proven correct for, on every rung of §6's ladder and on the cube.
#[test]
fn the_budget_fits_inside_the_local_radius_on_every_rung() {
    let rungs: [(Topology, Scale, f64, f64); 3] = [
        (Topology::Cube, Scale::ONE, 9.0, 32.0),
        (Topology::Ring { w: 320, h: 180 }, Scale::ONE, 9.0, 90.0),
        (Topology::Ring { w: 640, h: 360 }, Scale::new(2.0), 18.0, 180.0),
    ];
    for (topo, scale, budget, radius) in rungs {
        assert_eq!(scale.footprint_radius(), budget, "{topo:?}: budget");
        assert_eq!(topo.max_local_radius(), radius, "{topo:?}: local radius");
        assert!(budget <= radius, "{topo:?}: the stamp budget must fit inside the unfolding");
    }
}

/// The first check site (`Sprite::from_rgba`): a sprite whose painted extent exceeds the
/// budget is refused outright, and the refusal names the budget.
#[test]
fn a_sprite_wider_than_the_budget_is_refused() {
    let too_big = solid(13).expect_err("a 13×13 solid square reaches 9.69 px from its pivot");
    assert!(too_big.contains("budget"), "the refusal should name the budget, got {too_big:?}");
    assert!(expected_extent(13) > 9.0, "sanity: {} > 9", expected_extent(13));

    let ok = solid(11).expect("an 11×11 solid square reaches 8.28 px and fits");
    assert!((ok.extent() - expected_extent(11)).abs() <= 1e-9, "extent {}", ok.extent());
    assert!(ok.extent() <= Scale::ONE.footprint_radius());
}

/// Transparent padding never enlarges the footprint, so the budget is about painted light,
/// not about the image rectangle.
#[test]
fn transparent_padding_does_not_spend_the_budget() {
    // A 64×64 image with a single opaque texel at its centre: far inside the budget.
    let side = 64usize;
    let mut bytes = vec![0u8; side * side * 4];
    let centre = (side / 2) * side + side / 2;
    bytes[centre * 4..centre * 4 + 4].copy_from_slice(&[255, 255, 255, 255]);
    let sprite = Sprite::from_rgba(side, side, Vec2::new(32.5, 32.5), &bytes)
        .expect("one painted texel at the pivot is within any budget");
    assert!(
        (sprite.extent() - TEXEL_SUPPORT).abs() <= 1e-9,
        "a single texel on the pivot costs only the support: {}",
        sprite.extent()
    );
    assert_eq!(sprite.width(), 64);
    assert_eq!(sprite.height(), 64);
}

/// The second check site (`stamp_layers*`): `extent · scale` against the budget. At
/// `S = 1` a `scale = 2` stamp of a 6 px sprite is over budget and must not draw — that is
/// the behaviour §9 calls "vanishing", and it is correct while the world scale is 1.
#[test]
fn at_scale_one_an_oversized_stamp_is_over_budget() {
    let sprite = solid(8).expect("an 8×8 solid square fits at scale 1");
    let extent = sprite.extent();
    assert!(extent <= 9.0, "extent {extent}");
    assert!(extent * 2.0 > Scale::ONE.footprint_radius(), "doubled it is over budget");
    assert!(
        extent * 2.0 <= Scale::new(2.0).footprint_radius(),
        "but inside the budget of a world at S = 2, which is what FW-3 must adopt"
    );
}

// ---------------------------------------------------------------------------
// Stamping on a ring canvas
// ---------------------------------------------------------------------------

/// Every pixel a stamp paints, and how far it is from the anchor on the surface.
fn painted(canvas: &Canvas, anchor: SurfacePoint) -> (Vec<(u16, u16)>, f64) {
    let topo = canvas.topology();
    let mut pixels = Vec::new();
    let mut worst: f64 = 0.0;
    for (face, x, y) in canvas.coords() {
        if canvas.get(face, x, y) != [0.0; 3] {
            pixels.push((x, y));
            let p = SurfacePoint::pixel_center(topo, face, x, y);
            worst = worst.max(topo.chord_sq(&anchor, &p).sqrt());
        }
    }
    (pixels, worst)
}

/// A stamp on a ring canvas paints on both sides of the wrap and stays inside `9·S`.
#[test]
fn a_stamp_straddling_the_wrap_paints_both_sides_inside_the_budget() {
    let topo = Topology::Ring { w: 320, h: 180 };
    let mut canvas = Canvas::new(topo, Scale::ONE);
    let sprite = solid(8).expect("an 8×8 solid square fits the budget");
    let anchor = SurfacePoint::new(Face::Front, 0.5, 90.5);
    let mut scratch = Vec::new();
    stamp_sprite(&mut canvas, anchor, Vec2::new(1.0, 0.0), &sprite, 1.0, 1.0, &mut scratch);

    let (pixels, worst) = painted(&canvas, anchor);
    assert!(!pixels.is_empty(), "the stamp drew nothing");
    assert!(pixels.iter().any(|(x, _)| *x < 8), "nothing painted east of the wrap");
    assert!(pixels.iter().any(|(x, _)| *x > 311), "nothing painted west of the wrap");
    assert!(
        worst <= Scale::ONE.footprint_radius() + 1e-9,
        "a painted pixel is {worst} from the anchor, past the {} budget",
        Scale::ONE.footprint_radius()
    );
    // The same stamp in the middle of the chart paints the same number of pixels.
    let mut mid_canvas = Canvas::new(topo, Scale::ONE);
    let mid = SurfacePoint::new(Face::Front, 160.5, 90.5);
    stamp_sprite(&mut mid_canvas, mid, Vec2::new(1.0, 0.0), &sprite, 1.0, 1.0, &mut scratch);
    assert_eq!(painted(&mid_canvas, mid).0.len(), pixels.len(), "the wrap costs no pixels");
}

/// A sprite that is over budget draws nothing at all — the refusal is silent but total.
#[test]
fn an_over_budget_stamp_draws_nothing() {
    let topo = Topology::Ring { w: 320, h: 180 };
    let mut canvas = Canvas::new(topo, Scale::ONE);
    let sprite = solid(8).expect("fits at scale 1");
    let anchor = SurfacePoint::new(Face::Front, 160.5, 90.5);
    let mut scratch = Vec::new();
    assert!(sprite.extent() * 2.0 > Scale::ONE.footprint_radius(), "over budget at scale 2");
    stamp_sprite(&mut canvas, anchor, Vec2::new(1.0, 0.0), &sprite, 2.0, 1.0, &mut scratch);
    assert!(canvas.pixels().iter().all(|p| *p == [0.0; 3]), "an over-budget stamp must not draw");
}

/// **pending FW-3.** §9's FW-3 row: "adopt `Scale::footprint_radius()` at both check sites
/// … a `scale = 2` stamp draws instead of vanishing". The canvas now carries the scale
/// (`Canvas::scale()`), but `sprite.rs` still compares `extent · scale` against a private
/// `FOOTPRINT_RADIUS = 9.0`, so on a world at `S = 2` — where the budget is 18 — the stamp
/// still vanishes. Ignored until that adoption lands; it is the test for it.
#[test]
#[ignore = "pending FW-3: the sprite check sites still compare against the constant 9.0"]
fn a_scale_two_stamp_draws_on_a_world_at_s2() {
    let topo = Topology::Ring { w: 640, h: 360 };
    let scale = Scale::new(2.0);
    let mut canvas = Canvas::new(topo, scale);
    let sprite = solid(8).expect("fits at scale 1");
    let anchor = SurfacePoint::new(Face::Front, 320.5, 180.5);
    let mut scratch = Vec::new();
    assert!(sprite.extent() * 2.0 <= scale.footprint_radius(), "inside the S = 2 budget");
    stamp_sprite(&mut canvas, anchor, Vec2::new(1.0, 0.0), &sprite, 2.0, 1.0, &mut scratch);

    let (pixels, worst) = painted(&canvas, anchor);
    assert!(!pixels.is_empty(), "a scale = 2 stamp must draw on a world at S = 2");
    assert!(worst <= scale.footprint_radius() + 1e-9, "painted {worst} px from the anchor");
}
