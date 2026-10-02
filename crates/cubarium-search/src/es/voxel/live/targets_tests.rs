//! Targets v2 (`design/handoffs/cotrain-stage0-2026-10-01.md` §3, Wrysk 2026-10-01:
//! persist, breed, bounded): the penalty's new parts, and unified mode with an untrained
//! lineage. Each at most 200 ticks; no pinned hashes.

use cubarium_voxel_fauna::Founder;

use super::ecology::EcologyParams;
use super::search::{Band, BandSpec, EvalSpec, Trace, breeding_shortfall, evaluate, penalty};
use super::variants::VariantTable;
use crate::es::tensor::initial_center_shape;
use crate::es::voxel::{EpisodeDriver, TRAINED_LINEAGES};

const FOUNDED: usize = 16;

fn band() -> Band {
    Band::for_founding(FOUNDED, true, &BandSpec::default())
}

/// A two-hour trace: `n` animals every minute, `lj` of them lanternjaws.
fn trace(n: usize, lj: usize) -> Trace {
    Trace {
        horizon_min: 120.0,
        minutes: (1..=120).map(f64::from).collect(),
        animals: vec![n; 120],
        lanternjaws: vec![lj; 120],
        browser_moving: 0.4,
        missing: 0,
        silent_lineages: 0.0,
    }
}

/// The animal band is 1x, 3x and 5x the founding animal count by default, and each is a
/// flag.
#[test]
fn the_band_scales_with_the_founding_count() {
    let b = band();
    assert_eq!((b.lo, b.hi, b.peak), (16.0, 48.0, 80.0));
    let spec = BandSpec {
        lo_x: 2.0,
        hi_x: 4.0,
        peak_x: 6.0,
        ..BandSpec::default()
    };
    let b = Band::for_founding(10, true, &spec);
    assert_eq!((b.lo, b.hi, b.peak), (20.0, 40.0, 60.0));
    assert!(
        Band::for_founding(10, false, &spec)
            .lanternjaw_share
            .is_none()
    );
}

/// Every new part is zero inside its band and grows monotonically outside it.
#[test]
fn each_part_is_zero_inside_its_band_and_monotonic_outside() {
    let b = band();
    // Inside everything: 20-48 animals, 5-15 % lanternjaws.
    for (n, lj) in [(20, 1), (40, 2), (40, 6), (48, 7)] {
        let p = penalty(&trace(n, lj), &b);
        assert_eq!(p.total, 0.0, "{n} animals, {lj} lanternjaws: {p:?}");
    }
    // Bounded: below the floor and above the ceiling.
    let mut last = 0.0;
    for n in (1..16).rev() {
        let v = penalty(&trace(n, 0), &b).bound;
        assert!(v > last, "{n} animals: {v} after {last}");
        last = v;
    }
    let mut last = 0.0;
    for n in 49..200 {
        let v = penalty(&trace(n, n / 10), &b).bound;
        assert!(v > last, "{n} animals: {v} after {last}");
        last = v;
    }
    // Never above the peak.
    let mut last = 0.0;
    for spike in 81..200 {
        let mut t = trace(40, 4);
        t.animals[10] = spike;
        let v = penalty(&t, &b).peak;
        assert!(v > last, "a spike of {spike}: {v} after {last}");
        last = v;
    }
    // The lanternjaw share: under 5 % and over 15 %.
    let mut last = 0.0;
    for lj in (0..2).rev() {
        let v = penalty(&trace(40, lj), &b).share;
        assert!(v > last, "{lj} of 40: {v} after {last}");
        last = v;
    }
    let mut last = 0.0;
    for lj in 7..=40 {
        let v = penalty(&trace(40, lj), &b).share;
        assert!(v > last, "{lj} of 40: {v} after {last}");
        last = v;
    }
    // Moving: a stiller browser costs more.
    let mut last = 0.0;
    for m in [0.29, 0.2, 0.1, 0.0] {
        let mut t = trace(40, 4);
        t.browser_moving = m;
        let v = penalty(&t, &b).lively;
        assert!(v > last, "moving {m}: {v} after {last}");
        last = v;
    }
    // Persistence and breeding: each lineage or species lost, and each silent lineage,
    // costs the same fixed amount.
    for k in 1..4 {
        let mut gone = trace(40, 4);
        gone.missing = k;
        let mut silent = trace(40, 4);
        silent.silent_lineages = k as f64;
        assert_eq!(penalty(&gone, &b).persist, k as f64 * b.missing_cost);
        assert_eq!(penalty(&silent, &b).persist, k as f64 * b.missing_cost);
    }
    // A world founded without the egg bank has no lanternjaw share to keep.
    let no_bank = Band::for_founding(FOUNDED, false, &BandSpec::default());
    assert_eq!(penalty(&trace(40, 0), &no_bank).share, 0.0);
}

fn centre(founder: Founder) -> Vec<f64> {
    match founder {
        Founder::Blind => initial_center_shape::<23, 3>(5),
        Founder::Browser => initial_center_shape::<37, 3>(5),
        _ => unreachable!("no centre exists for this lineage"),
    }
}

/// Unified mode with no lanternjaw centre runs a window: the trained lineages carry their
/// variants and are scored, the lanternjaw is founded (as its egg bank) and runs its
/// heuristic, and takes no score.
#[test]
fn unified_mode_with_no_lanternjaw_centre_runs_one_window() {
    let drivers: Vec<(Founder, EpisodeDriver)> = TRAINED_LINEAGES
        .iter()
        .map(|&f| {
            (
                f,
                EpisodeDriver::gru(&centre(f), f).expect("a valid centre"),
            )
        })
        .collect();
    let tables: Vec<Option<VariantTable>> = Founder::ALL
        .iter()
        .map(|&f| {
            TRAINED_LINEAGES
                .contains(&f)
                .then(|| VariantTable::antithetic(f, &centre(f), 3, 0.05, 0, 2).expect("valid"))
        })
        .collect();
    // A small still world of the training recipe, so the founding is quick.
    let world = super::world::still_water(cubarium_voxel::Config {
        width: 64,
        height: 24,
        depth: 8,
        voxel_m: 0.25,
        landform: cubarium_voxel::Landform::Staged(cubarium_voxel::Recipe::DEFAULT),
        ..cubarium_voxel::Config::default()
    });
    let spec = EvalSpec::new(world, 100, 100);
    let e = evaluate(&EcologyParams::defaults(), 7, &drivers, &tables, &spec);
    assert_eq!(e.ticks, 100, "one whole window: {:?}", e.stopped);
    assert!(!e.body_scores.is_empty(), "the trained bodies were scored");
    assert!(
        e.body_scores
            .iter()
            .all(|b| TRAINED_LINEAGES.contains(&b.founder)),
        "only lineages with a centre are scored"
    );
    let lj = Founder::Lanternjaw.index();
    assert!(
        e.lineages_end[lj] + e.eggs_end[lj] > 0,
        "the lanternjaw was founded: {:?} bodies, {:?} eggs",
        e.lineages_end,
        e.eggs_end
    );
}

/// Stage 1's switch: with the lanternjaw held to persistence, a world whose lanternjaws are
/// only eggs takes no share penalty and no breeding penalty, and still counts as holding
/// the lineage; without it, both apply.
#[test]
fn a_persist_only_lineage_of_eggs_takes_no_share_or_breeding_penalty() {
    let rest: toml::Table = "[targets]\npersist_only = [\"lanternjaw\"]\n"
        .parse()
        .unwrap();
    let held = BandSpec::default().with_targets(&rest).expect("valid");
    let lj = Founder::Lanternjaw.index();
    assert!(held.persist_only[lj]);
    let founded = [8, 8, 9, 0, 0, 0, 0];
    // Shredders and browsers bred late; the lanternjaw, all eggs, did not.
    let late = [2, 3, 0];
    let b = Band::for_founding(16, true, &held);
    let eggs_only = trace(30, 0);
    assert_eq!(penalty(&eggs_only, &b).share, 0.0);
    let lost = [false; Founder::COUNT];
    let ready = [0.0; 3];
    assert_eq!(
        breeding_shortfall(&founded, &late, &lost, &ready, &held, true),
        0.0
    );
    let mut t = eggs_only.clone();
    t.silent_lineages = breeding_shortfall(&founded, &late, &lost, &ready, &held, true);
    assert_eq!(penalty(&t, &b).total, 0.0, "{:?}", penalty(&t, &b));
    // Switched off: the same world pays for both.
    let open = BandSpec::default();
    let b = Band::for_founding(16, true, &open);
    assert!(penalty(&eggs_only, &b).share > 0.0);
    assert_eq!(
        breeding_shortfall(&founded, &late, &lost, &ready, &open, true),
        1.0
    );
    assert!(
        BandSpec::default()
            .with_targets(&"[targets]\npersist_only = [\"nobody\"]\n".parse().unwrap())
            .is_err()
    );
}

/// `lively_weight` multiplies the moving-share term and nothing else.
#[test]
fn the_lively_weight_scales_only_the_lively_term() {
    let mut t = trace(30, 3);
    t.browser_moving = 0.15;
    t.silent_lineages = 0.5;
    let one = Band::for_founding(
        16,
        true,
        &BandSpec {
            lively_weight: 1.0,
            ..BandSpec::default()
        },
    );
    let ten = Band::for_founding(16, true, &BandSpec::default());
    assert_eq!(BandSpec::default().lively_weight, 10.0, "the default");
    let (p1, p10) = (penalty(&t, &one), penalty(&t, &ten));
    assert!((p1.lively - 0.5).abs() < 1e-12, "{p1:?}");
    assert!((p10.lively - 5.0).abs() < 1e-12, "{p10:?}");
    assert_eq!(
        (p1.bound, p1.peak, p1.share, p1.persist),
        (p10.bound, p10.peak, p10.share, p10.persist)
    );
    assert!((p10.total - p1.total - 4.5).abs() < 1e-12);
}

/// A lineage founded only as eggs (the lanternjaw's bank) counts as founded: it is held to
/// persistence, its share target applies, and it is lost only with no bodies and no eggs.
/// `breed_exempt` drops its breeding term alone.
#[test]
fn an_egg_founded_lineage_is_scored() {
    use super::search::{founded_lineages, lost_lineages};
    let founded = founded_lineages(&[8, 8, 0, 0, 0, 0, 0], &[0, 0, 9, 0, 0, 0, 0]);
    assert_eq!(founded, vec![8, 8, 9, 0, 0, 0, 0]);
    let lj = Founder::Lanternjaw.index();
    let spec = BandSpec {
        persist_only: [false; Founder::COUNT],
        ..BandSpec::default()
    };
    let b = Band::for_founding(16, founded[lj] > 0, &spec);
    assert!(b.lanternjaw_share.is_some());
    assert!(penalty(&trace(30, 0), &b).share > 0.0, "the share target applies");
    assert_eq!(lost_lineages(&founded, &[3, 3, 0, 0, 0, 0, 0], &[0, 0, 4, 0, 0, 0, 0]), 0, "eggs keep it");
    assert_eq!(lost_lineages(&founded, &[3, 3, 0, 0, 0, 0, 0], &[0, 0, 0, 0, 0, 0, 0]), 1, "no bodies, no eggs");
    // Breeding: held to it by default, exempt with breed_exempt; persistence either way.
    let lost = [false; Founder::COUNT];
    let ready = [0.0; 3];
    let late = [1, 1, 0];
    assert_eq!(breeding_shortfall(&founded, &late, &lost, &ready, &spec, true), 1.0);
    let rest: toml::Table = "[targets]\nbreed_exempt = [\"lanternjaw\"]\n".parse().unwrap();
    let exempt = spec.with_targets(&rest).expect("valid");
    assert!(exempt.breed_exempt[lj] && !exempt.persist_only[lj]);
    assert_eq!(breeding_shortfall(&founded, &late, &lost, &ready, &exempt, true), 0.0);
    let b = Band::for_founding(16, true, &exempt);
    assert!(b.lanternjaw_share.is_some(), "the share target stays");
}
