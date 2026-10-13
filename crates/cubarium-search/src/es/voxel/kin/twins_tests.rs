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
    assert_eq!(l.twins, TwinKnobs { window_s: 90.0, pairs: 8, starts: 2, nulls: 0, ..Default::default() });
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
    let knobs = TwinKnobs { window_s: 1.0, pairs: 5, starts: 3, nulls: 0, ..Default::default() };
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
    let knobs = TwinKnobs { window_s: 10.0, pairs: 1, starts: 1, nulls: 1, carrier_share: 1.0, stranger_weight: 1.0 };
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
        let r = measure(&[TwinJob { lineage: F, table: &table, knobs, seed: 1 }], &s, &metering, 2).unwrap().remove(0);
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
    let knobs = TwinKnobs { window_s: 2.0, pairs: 2, starts: 1, nulls: 0, ..Default::default() };
    let metering = Metering { kin: twins_kin(Founder::Browser, knobs), ..Default::default() };
    let centre = brain(F, false);
    let s = [start(&centre)];
    let m = Founder::Browser.manifest();
    let other = crate::es::tensor::initial_center_runtime(m.inputs(), m.outputs(), m.hidden, 5);
    let table = VariantTable::antithetic(Founder::Browser, &other, 3, 0.05, 0, 2).unwrap();
    let r = measure(&[TwinJob { lineage: Founder::Browser, table: &table, knobs, seed: 1 }], &s, &metering, 2).unwrap().remove(0);
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

// ---- TWIN-MIX: half-and-half twins ----

#[test]
fn the_carrier_knobs_parse_and_refuse_what_they_cannot_mean() {
    let n = F.name();
    let k = KinConfig::parse(&format!("[{n}]\ncomparison = \"twins\"\n[{n}.twins]\ncarrier_share = 0.25\nstranger_weight = 0.5\n")).unwrap();
    assert_eq!(k.of(F).twins.carrier_share, 0.25);
    assert_eq!(k.of(F).twins.stranger_weight, 0.5);
    // All-sign is carrier_share 1 with the dial at 1; at 1 the dial cannot be anything else.
    assert!(KinConfig::parse(&format!("[{n}.twins]\ncarrier_share = 1.0\nstranger_weight = 1.0\n")).is_ok());
    assert!(KinConfig::parse(&format!("[{n}.twins]\ncarrier_share = 1.0\n")).is_err());
    assert!(KinConfig::parse(&format!("[{n}.twins]\ncarrier_share = 0.0\n")).is_err(), "someone must carry");
    assert!(KinConfig::parse(&format!("[{n}.twins]\nstranger_weight = 1.5\n")).is_err());
}

#[test]
fn the_carrier_draw_is_the_seeds_and_keeps_a_non_carrier() {
    let ids: Vec<u64> = (10..20).collect();
    let a = carriers(&ids, 0.5, 7);
    assert_eq!(a, carriers(&ids, 0.5, 7), "the same seed draws the same carriers");
    assert_eq!(a.len(), 5);
    assert!(a.windows(2).all(|w| w[0] < w[1]) && a.iter().all(|x| ids.contains(x)));
    assert!((0..20).any(|s| carriers(&ids, 0.5, s) != a), "the draw moves with the seed");
    assert_eq!(carrier_count(0, 0.5), 0);
    assert_eq!(carrier_count(1, 0.5), 1, "one body alone carries");
    assert_eq!(carrier_count(3, 0.9), 2, "below a share of 1 a non-carrier is kept");
    assert_eq!(carrier_count(3, 0.01), 1, "at least one carries");
    assert_eq!(carrier_count(4, 1.0), 4);
    assert_eq!(carriers(&ids, 1.0, 3), ids);
}

/// A hand-built twin from the estimator's model: `Δb_k = [k∈C]·δ + a_k·e_R + u_k·e_S`,
/// carriers' own = base (no curiosity), the `−` copy all zero.
fn modelled(carrier: &[bool], related: &[(usize, usize)], shares: f64, delta: f64, e_r: f64, e_s: f64) -> (Vec<MixBody>, MixCopy, MixCopy) {
    let m = carrier.iter().filter(|c| **c).count() as u32;
    let mut bodies: Vec<MixBody> = carrier.iter().map(|&c| MixBody { carrier: c, related_carriers: 0, kin_weight: 0.0 }).collect();
    for &(x, y) in related {
        for (k, other) in [(x, y), (y, x)] {
            if carrier[other] {
                bodies[k].related_carriers += 1;
                bodies[k].kin_weight += shares;
            }
        }
    }
    let plus: Vec<f64> = bodies
        .iter()
        .map(|b| {
            let u = f64::from(m - b.related_carriers - u32::from(b.carrier));
            (if b.carrier { delta } else { 0.0 }) + f64::from(b.related_carriers) * e_r + u * e_s
        })
        .collect();
    let n = carrier.len();
    (
        bodies,
        MixCopy { own: plus.clone(), base: plus, ..Default::default() },
        MixCopy { own: vec![0.0; n], base: vec![0.0; n], ..Default::default() },
    )
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-12
}

/// `+` helps only its carrier: positive at every dial.
#[test]
fn a_variant_that_helps_only_its_carrier_scores_positive_at_any_dial() {
    let (b, p, m) = modelled(&[true, false, true, false, false, true], &[], 0.0, 0.3, 0.0, 0.0);
    for w in [0.0, 0.5, 1.0] {
        let e = estimate(&b, &p, &m, w).unwrap();
        assert!(close(e.direct, 0.3) && close(e.social, 0.0), "{e:?}");
        assert!(e.score > 0.0, "dial {w}: {e:?}");
    }
}

/// `+` costs its carrier and helps everyone else: negative at dial 0 (topic 2, no
/// kindness to strangers), positive at dial 1 once the benefit outweighs the cost.
#[test]
fn a_variant_that_helps_others_at_its_carriers_cost_turns_with_the_dial() {
    let (b, p, m) = modelled(&[true, false, true, false, true, false], &[], 0.0, -0.2, 0.0, 0.1);
    let e0 = estimate(&b, &p, &m, 0.0).unwrap();
    assert!(close(e0.direct, -0.2) && close(e0.stranger_pair, 0.1), "the model's effects come back: {e0:?}");
    assert!(close(e0.social, 0.5), "each carrier helped 5 others by 0.1: {e0:?}");
    assert!(e0.score < 0.0, "{e0:?}");
    let e1 = estimate(&b, &p, &m, 1.0).unwrap();
    assert!(e1.score > 0.0, "{e1:?}");
    // At dial 1 the score is the lineage's total change over the carriers.
    let total: f64 = p.base.iter().sum();
    assert!(close(e1.score, total / 3.0), "{e1:?}");
    // Too little benefit and dial 1 still says no.
    let (b, p, m) = modelled(&[true, false, true, false, true, false], &[], 0.0, -0.2, 0.0, 0.01);
    assert!(estimate(&b, &p, &m, 1.0).unwrap().score < 0.0);
}

/// A relative's gain counts by its share at dial 0; a stranger's does not.
#[test]
fn relatives_count_by_their_share_and_strangers_by_the_dial() {
    // Bodies 0 and 2 carry; 1 is 0's calf, 3 and 4 are strangers to both.
    let carrier = [true, false, true, false, false];
    let (b, p, m) = modelled(&carrier, &[(0, 1)], 0.5, 0.0, 0.4, 0.1);
    let e = estimate(&b, &p, &m, 0.0).unwrap();
    assert!(close(e.kin_pair, 0.4) && close(e.stranger_pair, 0.1) && close(e.direct, 0.0), "{e:?}");
    assert!(close(e.kin, 0.5 * 0.4 / 2.0), "the calf's gain from its mother at share 0.5, per carrier: {e:?}");
    assert!(close(e.score, e.kin));
    // Every share 0: nothing for the calf at dial 0.
    let (b, p, m) = modelled(&carrier, &[(0, 1)], 0.0, 0.0, 0.4, 0.1);
    assert_eq!(estimate(&b, &p, &m, 0.0).unwrap().kin, 0.0);
    assert!(close(estimate(&b, &p, &m, 0.0).unwrap().score, 0.0));
    // A carrier's kin to a relative that also carries is read through `ê_R`.
    let (b, p, m) = modelled(&[true, true, false, false], &[(0, 1)], 0.5, 0.0, 0.2, 0.0);
    let e = estimate(&b, &p, &m, 0.0).unwrap();
    assert_eq!(e.relatives, 0, "no non-carrier relative to read e_R from: {e:?}");
    assert_eq!(e.kin_pair, 0.0);
}

/// Identical copies estimate exactly 0; no carrier is missing.
#[test]
fn identical_copies_estimate_zero_and_no_carrier_is_missing() {
    let (b, p, _) = modelled(&[true, false, true, false], &[(0, 1)], 0.5, 0.3, 0.2, 0.1);
    let e = estimate(&b, &p, &p, 0.0).unwrap();
    assert_eq!((e.score, e.direct, e.social, e.kin), (0.0, 0.0, 0.0, 0.0));
    let none = [MixBody { carrier: false, related_carriers: 0, kin_weight: 0.0 }];
    let c = MixCopy { own: vec![1.0], base: vec![1.0], ..Default::default() };
    assert!(estimate(&none, &c, &c, 0.0).is_none());
}

fn half(w: f64) -> TwinKnobs {
    TwinKnobs { window_s: 10.0, pairs: 1, starts: 1, nulls: 1, carrier_share: 0.5, stranger_weight: w }
}

/// On the strip: resting beats running for the body that does it and costs nobody else,
/// so a half-and-half twin with `+` resting is positive at both ends of the dial; the
/// null twin is bit-identical and estimates 0.
#[test]
fn on_the_strip_a_self_only_benefit_is_positive_at_both_dials() {
    let centre = brain(F, false);
    let s = [start(&centre)];
    let ids = &s[0].bodies[F.index()];
    assert!(ids.len() >= 2);
    let table = VariantTable::build(F, &[brain(F, false), brain(F, true)]).unwrap();
    for w in [0.0, 1.0] {
        let knobs = half(w);
        let metering = Metering { kin: twins_kin(F, knobs), ..Default::default() };
        let r = measure(&[TwinJob { lineage: F, table: &table, knobs, seed: 5 }], &s, &metering, 2).unwrap().remove(0);
        let t = &r.readout;
        assert_eq!(t.mean_carriers, carrier_count(ids.len(), 0.5) as f64);
        assert_eq!(t.nulls_identical, 1, "{t:?}");
        assert_eq!(t.null_max_d, 0.0);
        assert!(t.mean_d > 0.0, "dial {w}: resting carriers beat running ones: {t:?}");
        assert!(t.mean_direct > 0.0, "{t:?}");
    }
}

/// `carrier_share = 1, stranger_weight = 1` is the all-sign twin, bit for bit: the scores
/// equal a hand-run of every body (and newborn) on the candidate, summed over the lineage
/// and divided by the start's head count.
#[test]
fn all_carriers_at_dial_one_is_the_all_sign_twin_bit_for_bit() {
    let centre = brain(F, false);
    let s = [start(&centre)];
    let ids = s[0].bodies[F.index()].clone();
    let table = VariantTable::build(F, &[brain(F, false), brain(F, true)]).unwrap();
    let knobs = TwinKnobs { carrier_share: 1.0, stranger_weight: 1.0, ..half(1.0) };
    let metering = Metering { kin: twins_kin(F, knobs), ..Default::default() };
    let r = measure(&[TwinJob { lineage: F, table: &table, knobs, seed: 5 }], &s, &metering, 2).unwrap().remove(0);
    let hand = |c: usize| {
        let assign: Vec<(u64, usize)> = ids.iter().map(|&id| (id, c)).collect();
        let options = crate::es::voxel::kin::matched::AssignOptions { newborns: Some((F, c)), ..Default::default() };
        let o = crate::es::voxel::kin::matched::run_assigned_with(&s[0].scene(window_ticks(&knobs)), &assign, &table, &metering.kin, options, &mut |_| {}).unwrap();
        let sum: f64 = o.metered.iter().filter(|m| m.founder == F).filter_map(|m| o.scores.get(&m.body_id)).map(|k| k.combined).sum();
        sum / ids.len() as f64
    };
    let d = hand(0) - hand(1);
    assert_eq!(r.scores.pairs, vec![(0, d / 2.0, -d / 2.0)]);
    assert_eq!(r.readout.mean_d.to_bits(), d.to_bits());
    assert_eq!(r.readout.mean_carriers, 0.0, "the all-sign path reads no estimate");
}

#[test]
fn the_shipped_twin_mix_kin_file_parses() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/training/kin-twin-mix-2026-10-13.toml");
    let k = KinConfig::load(&path).unwrap();
    for f in [Founder::Lanternjaw, Founder::Seedporter] {
        assert_eq!(k.of(f).comparison, Comparison::Twins);
        assert!(!k.of(f).twins.all_sign(), "{f:?} runs half-and-half twins");
        assert!(layout(&k.of(f).twins, 16).is_ok(), "{f:?}'s layout fits the 16-world specs");
    }
    assert!(k.of(F).is_today(), "a lineage left out trains as before");
}
