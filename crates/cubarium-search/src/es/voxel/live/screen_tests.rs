//! The R/S/F pre-screen (`design/ecology-parameter-registry-2026-09-28.md` §5; audit §1.4,
//! `scripts/invariants.py`): stay-or-go arithmetic over the resolved parameters and the
//! founding plan. No ticks.

use cubarium_voxel_fauna::{FaunaConfig, Founder};
use cubarium_voxel_flora::{FloraConfig, Species};

use super::screen::{ScreenSpec, rsf};

const VOXEL_M: f64 = 0.125;

/// A hand-built world where parking wins by arithmetic — one founded turf whose regrowth
/// alone keeps a parked browser fed many times over — is rejected, on R.
#[test]
fn the_screen_rejects_a_world_where_parking_wins_by_arithmetic() {
    let spec = ScreenSpec::default();
    let mut flora = FloraConfig::for_voxel_size(VOXEL_M);
    let turf = flora.species_mut(Species::Springturf);
    turf.foliage_rate *= 1000.0;
    turf.assimilation *= 1000.0;
    turf.founder_per_m2 = turf.founder_per_m2.max(1.0);
    turf.founder_cap = turf.founder_cap.max(10);
    let fauna = FaunaConfig::default();
    let s = rsf(&flora, &fauna, VOXEL_M, &spec);
    assert!(
        s.r > 1.0,
        "a parked browser's regrowth in reach covers its need: R = {}",
        s.r
    );
    assert!(!s.passes(&spec), "{s:?}");
    assert!(
        s.failures(&spec).iter().any(|f| f.starts_with('R')),
        "{:?}",
        s.failures(&spec)
    );
}

/// All three are per unit of the browser's need: a browser twice as heavy halves R and S
/// and F alike (audit §1.4's one lever).
#[test]
fn the_three_numbers_fall_with_the_browsers_mass() {
    let spec = ScreenSpec::default();
    let flora = FloraConfig::for_voxel_size(VOXEL_M);
    let light = FaunaConfig::default();
    let mut heavy = light;
    heavy.founders[Founder::Browser.index()].scale_mass(2.0);
    let (a, b) = (
        rsf(&flora, &light, VOXEL_M, &spec),
        rsf(&flora, &heavy, VOXEL_M, &spec),
    );
    for (name, x, y) in [("R", a.r, b.r), ("S", a.s, b.s), ("F", a.f, b.f)] {
        assert!(x > 0.0 && x.is_finite(), "{name} = {x}");
        assert!((y / x - 0.5).abs() < 1e-9, "{name}: {x} -> {y}");
    }
}

/// The bands: inside all three passes; each side of each band fails and names it.
#[test]
fn the_bands_pass_inside_and_fail_outside() {
    let spec = ScreenSpec::default();
    let mut s = rsf(
        &FloraConfig::for_voxel_size(VOXEL_M),
        &FaunaConfig::default(),
        VOXEL_M,
        &spec,
    );
    (s.r, s.s, s.f) = (0.1, 0.1, 20.0);
    assert!(s.passes(&spec), "{:?}", s.failures(&spec));
    for (r, sv, f, which) in [
        (0.01, 0.1, 20.0, 'R'),
        (0.5, 0.1, 20.0, 'R'),
        (0.1, 0.01, 20.0, 'S'),
        (0.1, 0.5, 20.0, 'S'),
        (0.1, 0.1, 5.0, 'F'),
    ] {
        (s.r, s.s, s.f) = (r, sv, f);
        assert!(!s.passes(&spec));
        assert!(
            s.failures(&spec).iter().any(|x| x.starts_with(which)),
            "{which}: {:?}",
            s.failures(&spec)
        );
    }
}
