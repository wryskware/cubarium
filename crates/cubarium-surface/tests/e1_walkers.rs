//! E1, continuously: "area-weighted long-run occupancy with isotropic, noninteracting
//! walkers and pure reflection, using sampling tolerances and correlated-sample estimates"
//! (design/surface-topology.md).
//!
//! Isotropic walkers with a fixed step and pure specular reflection at the open rim must
//! spend equal time per unit area everywhere. A seam that stretches or compresses the
//! surface, a reflection that loses or gains a component, or a rim that absorbs or
//! repels would all show up as a face imbalance or as rim cells being over- or
//! under-occupied relative to the interior.
//!
//! # Tolerances
//!
//! These are sampling-error estimates, not tuned numbers. The samples are heavily
//! correlated: a unit-step 2D walk has D = E[r^2]/4 = 1/4 px^2 per step, and the slowest
//! mode of this surface runs around the four side faces (wavelength 4 * 64 = 256 px), so
//! it decays at only D * (2*pi/L)^2 = 1.5e-4 per step -- an autocorrelation time of some
//! 6.6e3 steps. The effective sample count is therefore W*S/(2*tau), not W*S.
//!
//! At the 6.4e7 samples of the default run that model predicts a standard error of about
//! 0.003 for a face fraction, and three independent seeds gave worst-face deviations of
//! 0.0070, 0.0067 and 0.0055 and rim/interior ratios of 0.9960, 0.9550 and 1.0396,
//! i.e. sigma ~ 0.0045 for a face fraction and ~ 0.04 for the ratio. The default run
//! therefore asserts 0.02 and 0.15 (roughly four sigma each): tight enough that any real
//! bias -- a reflection that loses a component, a seam that stretches the surface -- shows
//! up as a many-sigma failure, and loose enough that a correct implementation does not
//! fail on noise.
//!
//! The ignored run takes ten times as many samples, which shrinks both errors by sqrt(10),
//! and asserts the tolerances the design note names: 0.01 on a face fraction (about seven
//! sigma there) and 0.05 on the rim/interior ratio (about four sigma).

use cubarium_surface::{
    CELL_COUNT, CELLS_PER_FACE_EDGE, CellId, Face, SurfacePoint, Travel, Vec2, cell_of,
    travel_into,
};

/// splitmix64: a tiny, fully specified PRNG so the run is identical on every machine.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

struct Occupancy {
    faces: [u64; 5],
    cells: Vec<u64>,
    samples: u64,
    fallbacks: u64,
}

fn walk(walkers: usize, steps: usize, burn_in: usize, seed: u64) -> Occupancy {
    let mut rng = Rng(seed);
    let mut occ = Occupancy {
        faces: [0; 5],
        cells: vec![0; CELL_COUNT],
        samples: 0,
        fallbacks: 0,
    };
    let mut buf = Travel::default();
    // Start uniformly over the surface: the walk is area-preserving, so an initially
    // uniform ensemble stays uniform and the burn-in only decorrelates the seed.
    let mut positions: Vec<SurfacePoint> = (0..walkers)
        .map(|_| {
            let face = Face::ALL[(rng.next_u64() % 5) as usize];
            SurfacePoint::new(face, rng.unit() * 64.0, rng.unit() * 64.0)
        })
        .collect();

    for step in 0..(burn_in + steps) {
        let recording = step >= burn_in;
        for p in positions.iter_mut() {
            // A fresh uniform direction every step, fixed unit step length.
            let angle = rng.unit() * std::f64::consts::TAU;
            travel_into(*p, Vec2::from_screen_angle(angle), &mut buf);
            if buf.fallback {
                occ.fallbacks += 1;
            }
            assert!(buf.end.is_canonical(), "walker left the surface: {:?}", buf.end);
            *p = buf.end;
            if recording {
                occ.faces[p.face.index()] += 1;
                occ.cells[cell_of(p).index()] += 1;
                occ.samples += 1;
            }
        }
    }
    occ
}

fn check(occ: &Occupancy, face_tol: f64, ratio_tol: f64) {
    let mut worst_face = 0.0f64;
    assert_eq!(occ.fallbacks, 0, "walkers hit the forward-progress fallback");
    let total = occ.samples as f64;
    assert_eq!(occ.faces.iter().sum::<u64>(), occ.samples);
    for face in Face::ALL {
        let fraction = occ.faces[face.index()] as f64 / total;
        worst_face = worst_face.max((fraction - 0.2).abs());
        assert!(
            (fraction - 0.2).abs() <= face_tol,
            "{face:?} holds {fraction} of the occupancy, expected 0.2 +- {face_tol}"
        );
    }

    // Every cell covers the same area, so every cell should hold the same occupancy; the
    // rim cells are where a mishandled reflection would pile walkers up or starve them.
    let last = (CELLS_PER_FACE_EDGE - 1) as u8;
    let mut rim = (0.0f64, 0usize);
    let mut interior = (0.0f64, 0usize);
    for c in CellId::all() {
        let n = occ.cells[c.index()] as f64;
        if c.face() != Face::Top && c.cy() == last {
            rim.0 += n;
            rim.1 += 1;
        } else {
            interior.0 += n;
            interior.1 += 1;
        }
        assert!(n > 0.0, "{c:?} was never visited");
    }
    assert_eq!(rim.1, 64);
    assert_eq!(interior.1, CELL_COUNT - 64);
    let ratio = (rim.0 / rim.1 as f64) / (interior.0 / interior.1 as f64);
    eprintln!(
        "E1: {} samples, worst face deviation {worst_face:.5} (tol {face_tol}), rim/interior {ratio:.5} (tol {ratio_tol})",
        occ.samples
    );
    assert!(
        (ratio - 1.0).abs() <= ratio_tol,
        "rim cells hold {ratio}x the mean interior occupancy, expected 1.0 +- {ratio_tol}"
    );
}

/// 64 walkers, 1,000,000 steps each after a 5,000-step burn-in: 6.4e7 samples in about
/// three seconds (a sweep costs some 40 ns). See the module tolerance note; the 1e5-step
/// run the design note sketches is ten times cheaper but its standard error on a face
/// fraction is 0.018, so nothing useful can be asserted at that length.
#[test]
fn isotropic_walkers_occupy_the_surface_uniformly() {
    let occ = walk(64, 1_000_000, 5_000, 0x0102_0304_0506_0708);
    check(&occ, 0.02, 0.15);
}

/// The same check with ten times the samples, at the tolerances `design/surface-topology.md`
/// names. Ignored by default: it is a half-minute run, not part of the normal suite.
#[test]
#[ignore = "long run; use --ignored to check E1 at the design note's tolerances"]
fn isotropic_walkers_occupy_the_surface_uniformly_long() {
    let occ = walk(64, 10_000_000, 5_000, 0x0102_0304_0506_0708);
    check(&occ, 0.01, 0.05);
}
