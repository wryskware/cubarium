//! LIVE-TWINS: the `twins` comparison's knobs, layout and measurement, on hand-built
//! fixtures (a small authored strip, at most a few hundred ticks).

use cubarium_voxel_fauna::Founder;

use super::*;
use crate::es::optimizer::Adam;
use crate::es::rng::perturbation;
use crate::es::voxel::kin::{KinConfig, LineageKin, ascend_pairs};
use crate::es::voxel::live::variants::VariantTable;

const F: Founder = Founder::Blind;

fn twins_kin(f: Founder, knobs: TwinKnobs) -> KinConfig {
    let mut k = KinConfig::default();
    k.lineages[f.index()] = LineageKin { comparison: Comparison::Twins, twins: knobs, ..Default::default() };
    k
}

#[test]
fn the_kin_file_selects_twins_and_checks_its_layout() {
    let n = F.name();
    let k = KinConfig::parse(&format!("[{n}]\ncomparison = \"twins\"\n[{n}.twins]\nwindow_s = 90.0\npairs = 8\nstarts = 2\nnulls = 0\n")).unwrap();
    let l = k.of(F);
    assert_eq!(l.comparison, Comparison::Twins);
    assert_eq!(l.twins, TwinKnobs { window_s: 90.0, pairs: 8, starts: 2, nulls: 0 });
    assert!(!l.is_today(), "twins is not today's training");
    // Left out, the layout is the default; every other lineage stays today's.
    let d = KinConfig::parse(&format!("[{n}]\ncomparison = \"twins\"\n")).unwrap();
    assert!(d.of(F).twins.is_default());
    assert!(d.of(Founder::Browser).is_today());
    // Every knob is range-checked by its registry row.
    assert!(KinConfig::parse(&format!("[{n}.twins]\nstarts = 0\n")).is_err());
    assert!(KinConfig::parse(&format!("[{n}.twins]\nwindow_s = 0.0\n")).is_err());
    assert!(KinConfig::parse(&format!("[{n}.twins]\nwhatever = 1\n")).is_err());
    // A config that never names the layout writes what it wrote before (the checkpoint).
    let today = serde_json::to_string(&KinConfig::default()).unwrap();
    assert!(!today.contains("twins"), "{today}");
    let back: KinConfig = serde_json::from_str(&serde_json::to_string(&k).unwrap()).unwrap();
    assert_eq!(back, k);
}

#[test]
fn the_layout_spreads_each_pair_over_distinct_starts() {
    let knobs = TwinKnobs { window_s: 1.0, pairs: 5, starts: 3, nulls: 0 };
    let plan = layout(&knobs, 4).unwrap();
    assert_eq!(plan.len(), 15);
    for p in 0..5 {
        let mut s: Vec<usize> = plan.iter().filter(|m| m.pair == p).map(|m| m.start).collect();
        assert_eq!(s.len(), 3);
        s.sort_unstable();
        s.dedup();
        assert_eq!(s.len(), 3, "pair {p}'s starts are distinct worlds");
    }
    let mut per_start = [0usize; 4];
    for m in &plan {
        per_start[m.start] += 1;
    }
    assert!(per_start.iter().max().unwrap() - per_start.iter().min().unwrap() <= 1, "{per_start:?}");
    assert!(layout(&TwinKnobs { starts: 5, ..knobs }, 4).is_err(), "more starts than worlds");
    assert!(layout(&TwinKnobs { pairs: 0, ..knobs }, 4).is_err());
}

/// A plain brain that holds still (`rest`) or runs flat out without feeding (`run`):
/// every action logit through its output bias, the rest of the net zero.
fn brain(f: Founder, run: bool) -> Vec<f64> {
    let m = f.manifest();
    let o = crate::es::tensor::shape_offsets_hidden(m.inputs(), m.outputs(), m.hidden);
    let mut theta = vec![0.0; m.parameter_count()];
    for (k, a) in m.actions.iter().enumerate() {
        let go = run && matches!(a.name, "forward_effort" | "sprint_effort");
        theta[o.b_o + k] = if go { 6.0 } else { -6.0 };
    }
    theta
}

/// A strip with four of `F` on `centre`, stepped a little, captured.
fn start(centre: &[f64]) -> TwinStart {
    let mut sim = super::super::tests::small_sim(&[(F, 4)]);
    let driver = crate::es::voxel::EpisodeDriver::gru(centre, F).unwrap();
    crate::es::voxel::live::install_centres(sim.fauna_mut(), &[(F, driver.clone())]);
    for _ in 0..5 {
        sim.step();
    }
    TwinStart::capture(0, &sim, &[(F, driver)], 1, &Default::default())
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// **The update follows a variant that helps.** By construction `+` helps here: in one
/// twin every body holds still, in the other every body runs and pays for it, and neither
/// feeds. With the resting brain on `+` the measurement is positive and the step moves the
/// centre along `+ε_0`; with the brains swapped, against it.
#[test]
fn the_update_follows_a_variant_that_helps() {
    let knobs = TwinKnobs { window_s: 10.0, pairs: 1, starts: 1, nulls: 1 };
    let metering = Metering { kin: twins_kin(F, knobs), ..Default::default() };
    let centre = brain(F, false);
    let s = [start(&centre)];
    assert!(!s[0].bodies[F.index()].is_empty());
    let (seed, generation, sigma) = (3u64, 0u32, 0.05);
    let mut eps = vec![0.0; centre.len()];
    perturbation(seed, u64::from(generation), 0, &mut eps);
    let mut moves = Vec::new();
    for rest_first in [true, false] {
        let pair = if rest_first { [brain(F, false), brain(F, true)] } else { [brain(F, true), brain(F, false)] };
        let table = VariantTable::build(F, &pair).unwrap();
        let r = measure(&[TwinJob { lineage: F, table: &table, knobs }], &s, &metering, 2).unwrap().remove(0);
        assert_eq!(r.scores.pairs.len(), 1);
        let (p, plus, minus) = r.scores.pairs[0];
        assert_eq!(p, 0);
        assert_eq!(plus, -minus, "a pair enters the update as (d/2, -d/2)");
        assert_eq!(r.readout.nulls_identical, 1, "the null twin is bit-identical");
        assert_eq!(r.readout.share_diverged, 1.0, "the twins' worlds differ");
        let d = plus - minus;
        assert_eq!(d > 0.0, rest_first, "resting beats running: d = {d}");
        let mut theta = centre.clone();
        let mut adam = Adam::new(theta.len());
        let (g, _) = ascend_pairs(&mut theta, &mut adam, &r.scores, sigma, |p, out| perturbation(seed, u64::from(generation), p as u64, out));
        assert!(g > 0.0);
        let step: Vec<f64> = theta.iter().zip(&centre).map(|(a, b)| a - b).collect();
        moves.push(dot(&step, &eps));
    }
    assert!(moves[0] > 0.0, "with + helping the centre moves along +eps: {moves:?}");
    assert!(moves[1] < 0.0, "with - helping it moves against +eps: {moves:?}");
}

/// A start without the lineage measures nothing: its measurements are missing, never zero.
#[test]
fn a_start_without_the_lineage_is_missing_not_zero() {
    let knobs = TwinKnobs { window_s: 2.0, pairs: 2, starts: 1, nulls: 0 };
    let metering = Metering { kin: twins_kin(Founder::Browser, knobs), ..Default::default() };
    let centre = brain(F, false);
    let s = [start(&centre)];
    let m = Founder::Browser.manifest();
    let other = crate::es::tensor::initial_center_runtime(m.inputs(), m.outputs(), m.hidden, 5);
    let table = VariantTable::antithetic(Founder::Browser, &other, 3, 0.05, 0, 2).unwrap();
    let r = measure(&[TwinJob { lineage: Founder::Browser, table: &table, knobs }], &s, &metering, 2).unwrap().remove(0);
    assert_eq!(r.readout.measurements_missing, 2);
    assert!(r.scores.pairs.is_empty());
    assert_eq!(r.scores.missing, 2);
}

#[test]
fn the_shipped_twins_kin_file_parses() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/training/kin-twins-2026-10-13.toml");
    let k = KinConfig::load(&path).unwrap();
    for f in [Founder::Lanternjaw, Founder::Seedporter] {
        assert_eq!(k.of(f).comparison, Comparison::Twins);
        assert!(layout(&k.of(f).twins, 16).is_ok(), "{f:?}'s layout fits the 16-world specs");
    }
    assert!(k.of(F).is_today(), "a lineage left out trains as before");
}
