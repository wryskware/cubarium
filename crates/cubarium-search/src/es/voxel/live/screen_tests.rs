//! The R/S/F pre-screen (`design/ecology-parameter-registry-2026-09-28.md` §5; audit §1.4,
//! `scripts/invariants.py`): stay-or-go arithmetic over the resolved parameters and the
//! founding plan. No ticks.

use cubarium_voxel_fauna::{FaunaConfig, Founder};
use cubarium_voxel_flora::{FloraConfig, Species};

use super::screen::{ScreenSpec, rsf};

const VOXEL_M: f64 = 0.125;

/// The bands, margin and weights are set here by hand, so the checks below read these
/// numbers and never the shipped ones.
fn spec() -> ScreenSpec {
    ScreenSpec {
        r: [0.02, 0.3],
        s: [0.02, 0.2],
        f_min: 10.0,
        base: 200.0,
        k: 10.0,
        run_margin: 2.0,
        k_soft: 5.0,
        r_readout: false,
        ..ScreenSpec::default()
    }
}

/// A hand-built world where parking wins by arithmetic — one founded turf whose regrowth
/// alone keeps a parked browser fed many times over — is rejected, on R.
#[test]
fn the_screen_rejects_a_world_where_parking_wins_by_arithmetic() {
    let spec = spec();
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
    let spec = spec();
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
    let spec = spec();
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

/// The refused candidate's penalty is graded: zero distance inside the bands, growing with
/// the log-distance outside each, and always above the worst candidate that ran.
#[test]
fn the_screen_penalty_grows_with_the_distance_outside_the_bands() {
    let spec = spec();
    let mut s = rsf(
        &FloraConfig::for_voxel_size(VOXEL_M),
        &FaunaConfig::default(),
        VOXEL_M,
        &spec,
    );
    (s.r, s.s, s.f) = (0.1, 0.1, 20.0);
    assert_eq!(s.distance(&spec), 0.0);
    let graded = |s: &super::screen::Rsf| spec.graded(s, f64::NEG_INFINITY);
    type Set = fn(&mut super::screen::Rsf, f64);
    let sides: [(&str, Set, [f64; 4]); 5] = [
        ("R high", |s, v| s.r = v, [0.31, 0.5, 1.0, 5.0]),
        ("R low", |s, v| s.r = v, [0.019, 0.01, 0.001, 0.0]),
        ("S high", |s, v| s.s = v, [0.21, 0.4, 1.0, 9.0]),
        ("S low", |s, v| s.s = v, [0.019, 0.005, 1e-4, 0.0]),
        ("F low", |s, v| s.f = v, [9.9, 5.0, 1.0, 0.0]),
    ];
    for (name, set, steps) in sides {
        let mut x = s.clone();
        let mut last = graded(&x);
        for v in steps {
            set(&mut x, v);
            let g = graded(&x);
            assert!(g > last, "{name} at {v}: {g} after {last}");
            last = g;
        }
    }
    // Above every candidate that ran, however bad.
    (s.r, s.s, s.f) = (0.31, 0.1, 20.0);
    assert!(spec.graded(&s, 1e4) > 1e4);
    assert!(spec.graded(&s, 0.0) >= spec.base);
}

/// The shipped start (`cotrain-start.toml`) sets cotrain's genes to the file's own values,
/// and a start naming a gene cotrain lacks is refused.
#[test]
fn the_shipped_start_sets_its_genes_and_refuses_unknown_ones() {
    use super::ecology::EcologyParams;
    let search = EcologyParams::cotrain().genome().cloned().expect("a genome");
    let start = cubarium_voxel_sim::params::load_genome(include_str!(
        "../../../../../../config/training/search/cotrain-start.toml"
    ))
    .expect("a valid start");
    let g = super::search::start_from(&search, &start).expect("its genes are cotrain's");
    for gene in &start.genes {
        assert_eq!(g.get(&gene.name), Some(gene.value), "{}", gene.name);
    }
    let mut bad = start.clone();
    bad.genes[0].name = "no_such_gene".into();
    assert!(super::search::start_from(&search, &bad).is_err());
}

/// The soft margin: inside the bands a candidate runs and pays no soft term; within
/// `run_margin` of them it runs and pays `k_soft × (log-distance)`, growing with the
/// distance; beyond the margin it is refused, as before.
#[test]
fn the_soft_margin_runs_near_misses_and_refuses_the_rest() {
    use super::screen::Verdict;
    let spec = spec();
    let mut s = rsf(
        &FloraConfig::for_voxel_size(VOXEL_M),
        &FaunaConfig::default(),
        VOXEL_M,
        &spec,
    );
    (s.r, s.s, s.f) = (0.1, 0.1, 20.0);
    assert_eq!(spec.verdict(&s), Verdict::Inside);
    assert_eq!(spec.soft(&s), 0.0, "inside pays no soft term");
    // Just outside on each side of each band, within the margin: runs.
    for (r, sv, f) in [
        (0.31, 0.1, 20.0),
        (0.011, 0.1, 20.0),
        (0.1, 0.21, 20.0),
        (0.1, 0.011, 20.0),
        (0.1, 0.1, 5.5),
        (0.59, 0.39, 5.1),
    ] {
        (s.r, s.s, s.f) = (r, sv, f);
        assert_eq!(spec.verdict(&s), Verdict::Margin, "({r}, {sv}, {f})");
        assert!(spec.soft(&s) > 0.0);
    }
    // Beyond the margin on any one of them: refused.
    for (r, sv, f) in [
        (0.61, 0.1, 20.0),
        (0.009, 0.1, 20.0),
        (0.1, 0.41, 20.0),
        (0.1, 0.009, 20.0),
        (0.1, 0.1, 4.9),
    ] {
        (s.r, s.s, s.f) = (r, sv, f);
        assert_eq!(spec.verdict(&s), Verdict::Refused, "({r}, {sv}, {f})");
    }
    // Monotonic in the distance outside, on each side.
    (s.r, s.s, s.f) = (0.1, 0.1, 20.0);
    let mut last = 0.0;
    for r in [0.31, 0.35, 0.45, 0.59] {
        s.r = r;
        let v = spec.soft(&s);
        assert!(v > last, "R {r}: {v} after {last}");
        last = v;
    }
    s.r = 0.1;
    let mut last = 0.0;
    for f in [9.0, 8.0, 6.0, 5.0] {
        s.f = f;
        let v = spec.soft(&s);
        assert!(v > last, "F {f}: {v} after {last}");
        last = v;
    }
}
