//! KIN-TRAIN's mechanism checks, on hand-built records and small worlds. No tuned number
//! is asserted: shares and scores here are fixture values.

use super::roster::{KinRecord, Relation, Roster};

fn rec(id: u64, mother: Option<u64>, pack: Option<u64>) -> KinRecord {
    KinRecord { id, mother, pack }
}

fn rel(r: &Roster, a: u64, b: u64) -> Option<Relation> {
    r.relatives(a).iter().find(|(o, _)| *o == b).map(|(_, x)| *x)
}

#[test]
fn roster_reads_mother_calf_sibling_and_packmate() {
    // 1 is the mother of 2 and 3; 4 is a stranger; 5 and 6 share pack 9 with 1.
    let r = Roster::fixed(&[
        rec(1, None, Some(9)),
        rec(2, Some(1), None),
        rec(3, Some(1), None),
        rec(4, None, None),
        rec(5, None, Some(9)),
        rec(6, None, Some(9)),
    ]);
    assert_eq!(rel(&r, 1, 2), Some(Relation::Calf));
    assert_eq!(rel(&r, 2, 1), Some(Relation::Mother));
    assert_eq!(rel(&r, 2, 3), Some(Relation::Sibling));
    assert_eq!(rel(&r, 3, 2), Some(Relation::Sibling));
    assert_eq!(rel(&r, 5, 6), Some(Relation::Packmate));
    assert_eq!(rel(&r, 1, 5), Some(Relation::Packmate));
    assert!(r.relatives(4).is_empty(), "a stranger has no relatives");
    assert_eq!(rel(&r, 1, 1), None, "never its own relative");
}

#[test]
fn the_closest_relation_wins_one_per_pair() {
    // A pup in its mother's pack is her calf, not her packmate; two littermates in one
    // pack are siblings, not packmates.
    let r = Roster::fixed(&[
        rec(1, None, Some(7)),
        rec(2, Some(1), Some(7)),
        rec(3, Some(1), Some(7)),
        rec(4, None, Some(7)),
    ]);
    for (a, b) in [(1, 2), (2, 1), (2, 3), (1, 4)] {
        let n = r.relatives(a).iter().filter(|(o, _)| *o == b).count();
        assert_eq!(n, 1, "{a}→{b} holds one relation");
    }
    assert_eq!(rel(&r, 1, 2), Some(Relation::Calf));
    assert_eq!(rel(&r, 2, 1), Some(Relation::Mother));
    assert_eq!(rel(&r, 2, 3), Some(Relation::Sibling));
    assert_eq!(rel(&r, 2, 4), Some(Relation::Packmate));
}

#[test]
fn a_founder_with_a_phantom_mother_id_has_no_live_mother() {
    // Founders 2 and 3 carry founding mother id 100, which names no animal.
    let r = Roster::fixed(&[rec(2, Some(100), None), rec(3, Some(100), None)]);
    assert!(!r.contains(100));
    assert!(
        r.relatives(2).iter().all(|(_, x)| *x != Relation::Mother),
        "no mother relation to an id that names no body"
    );
    assert_eq!(rel(&r, 2, 3), Some(Relation::Sibling), "a shared recorded id is a sibling key");
}

#[test]
fn a_newborn_joins_its_mothers_roster_at_birth() {
    let mut r = Roster::fixed(&[rec(1, None, Some(5)), rec(2, Some(1), Some(5)), rec(3, None, Some(5))]);
    r.join(rec(10, Some(1), Some(5)));
    assert_eq!(rel(&r, 1, 10), Some(Relation::Calf));
    assert_eq!(rel(&r, 10, 1), Some(Relation::Mother));
    assert_eq!(rel(&r, 2, 10), Some(Relation::Sibling));
    assert_eq!(rel(&r, 3, 10), Some(Relation::Packmate));
    // Joining twice changes nothing: relations are fixed once made.
    let before = r.relatives(1).len();
    r.join(rec(10, None, None));
    assert_eq!(r.relatives(1).len(), before);
}

#[test]
fn families_are_the_connected_groups() {
    let r = Roster::fixed(&[
        rec(1, None, None),
        rec(2, Some(1), None),
        rec(3, None, Some(4)),
        rec(5, None, Some(4)),
        rec(6, None, None),
    ]);
    assert_eq!(r.families(), vec![vec![1, 2], vec![3, 5], vec![6]]);
}

/// A small authored strip with `counts` founders, on their heuristics, one thread.
pub(super) fn small_sim(counts: &[(cubarium_voxel_fauna::Founder, usize)]) -> cubarium_voxel_sim::Sim {
    use cubarium_voxel_fauna::Founder;
    let cfg = cubarium_voxel::Config { width: 64, height: 24, depth: 6, ..Default::default() };
    let mut world = cubarium_voxel_sim::scene::authored(cfg);
    let mut flora = cubarium_voxel_flora::Flora::new(Default::default());
    let mut fauna =
        cubarium_voxel_fauna::Fauna::new(cubarium_voxel_fauna::FaunaConfig::placeholder());
    let mut n = [0; Founder::COUNT];
    for (f, k) in counts {
        n[f.index()] = *k;
    }
    cubarium_voxel_sim::habitat::seed_with_founder_counts(&mut world, &mut flora, &mut fauna, n);
    cubarium_voxel_sim::Sim::ambient(
        world,
        flora,
        fauna,
        cubarium_voxel_sim::SimConfig { threads: 1, overlap: false, water_every: 1 },
    )
}

#[test]
fn heuristic_bodies_are_metered_and_on_the_roster() {
    use crate::es::voxel::live::window::Window;
    use cubarium_voxel_fauna::Founder;
    let mut sim = small_sim(&[(Founder::Blind, 3)]);
    let ids: Vec<u64> = sim.fauna().view().animals.iter().map(|a| a.id).collect();
    assert!(!ids.is_empty(), "the fixture seeded bodies");
    // No lineage has a table: every body runs its heuristic.
    let tables = vec![None; Founder::COUNT];
    let mut w = Window::begin(&mut sim, &tables, &[0; Founder::COUNT], 40);
    while !w.done() {
        w.step(&mut sim);
    }
    let r = w.finish(&sim, 0);
    assert!(r.scores.is_empty(), "no variant, nothing ranked");
    for id in &ids {
        let m = r.metered.iter().find(|m| m.body_id == *id).expect("metered");
        assert_eq!(m.candidate, None);
        assert!(m.base().is_finite());
        assert!(r.roster.contains(*id), "on the roster");
    }
}

// ---- step 2 and 3: the kin-weighted score and the comparison switch ----

use super::compare::{self, PairScores};
use super::config::{Comparison, KinConfig, KinShares};
use super::score::kin_scores;
use crate::es::voxel::live::window::{BodyScore, MeteredBody, NetEnergy};
use cubarium_voxel_fauna::Founder;

const F: Founder = Founder::Chorister;

/// A body whose base score is `base` (alive: E_start 0, E_end `base`), or one that died
/// holding `base` (its base is `−base`); body_ref 1, nothing given.
fn metered(id: u64, candidate: Option<usize>, base: f64, died: bool) -> MeteredBody {
    let score = if died {
        NetEnergy::of(base, 0.0, 0.0, true, 0.0, 0.0, 1.0)
    } else {
        NetEnergy::of(0.0, base, 0.0, false, 0.0, 0.0, 1.0)
    };
    MeteredBody { body_id: id, founder: F, candidate, newborn: false, score }
}

fn kin_with(shares: KinShares, comparison: Comparison) -> KinConfig {
    let mut k = KinConfig::default();
    k.lineages[F.index()].shares = shares;
    k.lineages[F.index()].comparison = comparison;
    k
}

/// The window's scored bodies from metered ones under `kin` (as `Window::finish` does).
fn scored(world: usize, m: &[MeteredBody], roster: &Roster, kin: &KinConfig) -> Vec<BodyScore> {
    let k = kin_scores(m, roster, kin);
    m.iter()
        .filter_map(|b| {
            Some(BodyScore {
                world,
                body_id: b.body_id,
                delta_body_normalized: b.score.delta_normalized,
                delta_stomach_aware_normalized: b.score.delta_normalized,
                founder: b.founder,
                candidate: b.candidate?,
                score: b.score,
                newborn: false,
                kin: k[&b.body_id],
            })
        })
        .collect()
}

#[test]
fn a_dead_calfs_loss_counts_against_its_mother() {
    let roster = Roster::fixed(&[rec(1, None, None), rec(2, Some(1), None)]);
    let m = [metered(1, Some(0), 0.3, false), metered(2, Some(1), 0.8, true)];
    let calf_base = m[1].base();
    assert!(calf_base < 0.0, "a death loses the body");
    let kin = kin_with(KinShares { calf: 0.5, ..KinShares::default() }, Comparison::PerBody);
    let k = kin_scores(&m, &roster, &kin);
    assert_eq!(k[&1].relatives, 0.5 * calf_base);
    assert_eq!(k[&1].combined, k[&1].own + 0.5 * calf_base);
    // The calf's own share of its mother is 0 here: it gets nothing back.
    assert_eq!(k[&2].relatives, 0.0);
}

#[test]
fn a_relative_contributes_its_own_base_not_its_kin_score() {
    // 1 → 2 → 3 (grandmother, mother, calf). With r_calf, the grandmother counts only her
    // calf's own base, never her calf's calf through it.
    let roster = Roster::fixed(&[rec(1, None, None), rec(2, Some(1), None), rec(3, Some(2), None)]);
    let m = [metered(1, Some(0), 0.1, false), metered(2, Some(1), 0.2, false), metered(3, Some(0), 0.4, false)];
    let kin = kin_with(KinShares { calf: 0.5, ..KinShares::default() }, Comparison::PerBody);
    let k = kin_scores(&m, &roster, &kin);
    assert_eq!(k[&1].relatives, 0.5 * m[1].base());
}

#[test]
fn shares_at_zero_score_exactly_as_today_on_a_window() {
    use crate::es::voxel::live::trainer::pooled_scores;
    use crate::es::voxel::live::variants::VariantTable;
    use crate::es::voxel::live::window::Window;
    let founder = Founder::Blind;
    let mut sim = small_sim(&[(founder, 4)]);
    let m = founder.manifest();
    let centre = crate::es::tensor::initial_center_runtime(m.inputs(), m.outputs(), m.hidden, 5);
    let mut tables = vec![None; Founder::COUNT];
    tables[founder.index()] = Some(VariantTable::antithetic(founder, &centre, 3, 0.05, 0, 2).unwrap());
    let mut w = Window::begin(&mut sim, &tables, &[0; Founder::COUNT], 60);
    while !w.done() {
        w.step(&mut sim);
    }
    let r = w.finish(&sim, 0);
    assert!(!r.scores.is_empty());
    for b in &r.scores {
        assert_eq!(b.kin.combined.to_bits(), b.score.score.to_bits(), "combined is today's score");
    }
    let today: Vec<Vec<(usize, f64)>> = vec![r.scores.iter().map(|b| (b.candidate, b.score.score)).collect()];
    let (a, na) = pooled_scores(&today, 4);
    let (b, nb) = compare::per_body(&[r.scores.as_slice()], 4);
    assert_eq!(na, nb);
    assert_eq!(a.iter().map(|x| x.to_bits()).collect::<Vec<_>>(), b.iter().map(|x| x.to_bits()).collect::<Vec<_>>());
    // And through the kin scorer directly at shares 0 with relatives present.
    let roster = Roster::fixed(&[rec(1, None, Some(3)), rec(2, Some(1), Some(3))]);
    let ms = [metered(1, Some(0), 0.25, false), metered(2, Some(1), -0.5, true)];
    let k = kin_scores(&ms, &roster, &KinConfig::default());
    for x in &ms {
        assert_eq!(k[&x.body_id].combined.to_bits(), x.score.score.to_bits());
    }
}

/// One pack of four per world, every member a packmate of every other. The `+` variant
/// on body 1 makes body 2 (its twin neighbour, on `−`) gain `delta`; nothing else changes.
fn pack_world(delta: f64) -> (Roster, Vec<MeteredBody>) {
    let roster = Roster::fixed(&[rec(1, None, Some(9)), rec(2, None, Some(9)), rec(3, None, Some(9)), rec(4, None, Some(9))]);
    let m = vec![
        metered(1, Some(0), 0.10, false),
        metered(2, Some(1), 0.20 + delta, false),
        metered(3, Some(2), 0.05, false),
        metered(4, Some(3), 0.30, false),
    ];
    (roster, m)
}

#[test]
fn per_body_cancels_the_shared_kin_term_with_one_pack_per_world() {
    // Documentation of why the switch exists (§11.7 "The cancellation"): with equal
    // shares and one pack per world, a member's score is (1 − r)·own + r·total, and
    // z-scoring within the world removes the shared total, so the scores are own's.
    let (roster, m) = pack_world(0.4);
    let own = scored(0, &m, &roster, &KinConfig::default());
    let kin = kin_with(KinShares::uniform(0.5), Comparison::PerBody);
    let with = scored(0, &m, &roster, &kin);
    assert!(with.iter().any(|b| b.kin.relatives != 0.0), "the kin term is there before scoring");
    let (a, _) = compare::per_body(&[own.as_slice()], 4);
    let (b, _) = compare::per_body(&[with.as_slice()], 4);
    for (x, y) in a.iter().zip(&b) {
        assert!((x - y).abs() < 1e-9, "z-scores equal with and without kin: {a:?} {b:?}");
    }
}

#[test]
fn a_relatives_gain_survives_the_family_scorer() {
    // A family of mother 1 and calf 2. In world 0 the family carries pair0+, in world 1
    // pair0−; the + variant gains only the calf energy (`delta`), the mother is unchanged.
    let roster = Roster::fixed(&[rec(1, None, None), rec(2, Some(1), None)]);
    let kin = kin_with(KinShares { calf: 0.5, mother: 0.5, ..KinShares::default() }, Comparison::Family);
    let world = |c: usize, delta: f64| {
        let m = [metered(1, Some(c), 0.1, false), metered(2, Some(c), 0.2 + delta, false)];
        scored(0, &m, &roster, &kin)
    };
    let (plus, minus) = (world(0, 0.3), world(1, 0.0));
    let by = compare::family(&[(plus.as_slice(), &roster), (minus.as_slice(), &roster)], 2);
    let pairs = PairScores::from_candidates(&by);
    assert_eq!(pairs.missing, 0);
    let (_, p, n) = pairs.pairs[0];
    assert!(p > n, "the relative's gain reaches the + candidate: {p} vs {n}");
}

#[test]
fn a_relatives_gain_survives_the_matched_scorer() {
    // Matched copies: focal mother 1, identical in both copies except her sign; the +
    // copy's calf gains `delta`, the mother's own energy is the same.
    let roster = Roster::fixed(&[rec(1, None, None), rec(2, Some(1), None)]);
    let kin = kin_with(KinShares { calf: 0.5, ..KinShares::default() }, Comparison::Matched);
    let copy = |delta: f64| kin_scores(&[metered(1, None, 0.1, false), metered(2, None, 0.2 + delta, false)], &roster, &kin)[&1];
    let (p, n) = (copy(0.3), copy(0.0));
    assert_eq!(p.own, n.own, "the focal's own energy did not change");
    assert!(p.combined - n.combined > 0.0, "the difference is the relative's gain: {}", p.combined - n.combined);
    let pairs = PairScores::from_candidates(&[Some(p.combined), Some(n.combined)]);
    assert_eq!(pairs.pairs.len(), 1);
}

#[test]
fn missing_signs_are_never_scored_as_zero() {
    let pairs = PairScores::from_candidates(&[Some(1.0), None, Some(0.5), Some(-0.5), None, None]);
    assert_eq!(pairs.pairs, vec![(1, 0.5, -0.5)]);
    assert_eq!(pairs.missing, 2);
    // The update reads only the kept pair: the perturbation of pair 1 alone.
    let mut theta = vec![0.0; 3];
    let mut adam = crate::es::optimizer::Adam::new(3);
    let mut asked = Vec::new();
    let cell = std::cell::RefCell::new(&mut asked);
    compare::ascend_pairs(&mut theta, &mut adam, &pairs, 0.1, |p, out| {
        cell.borrow_mut().push(p);
        out.copy_from_slice(&[1.0, 0.0, 0.0]);
    });
    assert_eq!(asked, vec![1], "only the complete pair's perturbation is drawn");
    assert!(theta[0] > 0.0 && theta[1] == 0.0);
    // With no complete pair nothing moves.
    let none = PairScores::from_candidates(&[Some(1.0), None]);
    let mut t2 = vec![0.0; 3];
    assert_eq!(compare::ascend_pairs(&mut t2, &mut adam, &none, 0.1, |_, _| unreachable!()), (0.0, 0.0));
}

#[test]
fn family_placement_gives_a_family_one_variant() {
    use crate::es::voxel::live::window::Placed;
    let roster = Roster::fixed(&[rec(1, None, None), rec(2, Some(1), None), rec(3, None, None), rec(4, Some(3), None)]);
    let bodies = [
        Placed { id: 1, x_m: 0.0 },
        Placed { id: 2, x_m: 5.0 },
        Placed { id: 3, x_m: 1.0 },
        Placed { id: 4, x_m: 9.0 },
    ];
    let placed: rustc_hash::FxHashMap<u64, usize> = compare::place_families(&bodies, &roster, 2, 0).into_iter().collect();
    assert_eq!(placed[&1], placed[&2]);
    assert_eq!(placed[&3], placed[&4]);
    assert_eq!(placed[&1] / 2, placed[&3] / 2, "two families are one pair's twins");
    assert_ne!(placed[&1], placed[&3]);
}

#[test]
fn the_kin_file_parses_and_checks_its_shares() {
    let k = KinConfig::parse("[chorister]\ncomparison = \"matched\"\nstage = \"both\"\n[chorister.shares]\ncalf = 0.5\npackmate = 0.5\n").unwrap();
    assert_eq!(k.of(F).comparison, Comparison::Matched);
    assert_eq!(k.of(F).shares.calf, 0.5);
    assert!(k.of(Founder::Blind).is_today());
    assert!(KinConfig::parse("[chorister.shares]\ncalf = 1.5\n").is_err(), "a share is a fraction");
    assert!(KinConfig::parse("[nobody]\n").is_err());
    super::config::registry().check(&KinConfig::default()).expect("defaults have rows and are in range");
}

#[test]
fn the_shipped_kin_file_parses() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/training/kin-2026-10-12.toml");
    let k = KinConfig::load(&path).unwrap();
    assert!(k.of(Founder::Browser).is_today(), "a lineage left out trains as before");
}

// ---- step 4: matched copies ----

use super::matched::{self, EngageScene, MatchedPlan, Scene, SceneCopy};

/// A short staged encounter: ENGAGE's own test fixture (2 s, small strip), arm 0.
fn engage(seconds: f64) -> EngageScene {
    let config = cubarium_voxel_sim::engagement::Config {
        duration_s: seconds,
        episodes: 1,
        width: 64,
        depth: 40,
        height: 24,
        distance_m: [1.0, 1.5],
        spacing_m: 0.5,
        depth_jitter_m: 0.0,
        ..Default::default()
    };
    let arm = config.arms[0].clone();
    EngageScene { config, arm, seed: 73, centres: Vec::new() }
}

fn table(f: Founder, pairs: usize) -> crate::es::voxel::live::variants::VariantTable {
    let m = f.manifest();
    let centre = crate::es::tensor::initial_center_runtime(m.inputs(), m.outputs(), m.hidden, 5);
    crate::es::voxel::live::variants::VariantTable::antithetic(f, &centre, 3, 0.05, 0, pairs).unwrap()
}

#[test]
fn the_focal_and_the_sign_rotate() {
    let plans = matched::rotate(&[10, 20, 30], 2, 6);
    assert_eq!(plans.iter().map(|p| p.focal).collect::<Vec<_>>(), vec![10, 20, 30, 10, 20, 30]);
    assert_eq!(plans.iter().map(|p| p.pair).collect::<Vec<_>>(), vec![0, 1, 0, 1, 0, 1]);
    // Each focal's second turn swaps which copy carries +.
    for k in 0..3 {
        assert_ne!(plans[k].plus_first, plans[k + 3].plus_first, "focal {} sign slot rotates", plans[k].focal);
    }
    // The two copies differ only in the focal's sign.
    let [a, b] = MatchedPlan { focal: 7, pair: 3, plus_first: false }.copies();
    assert_eq!((a.focal, b.focal), (7, 7));
    assert_eq!((a.candidate, b.candidate), (Some(7), Some(6)));
    assert!(matched::rotate(&[], 2, 4).is_empty());
}

#[test]
fn matched_copies_change_only_the_focal() {
    let scene = engage(1.0);
    let copy = scene.open().unwrap();
    let focals = scene.focals(&copy, Founder::Loftstrider);
    assert!(!focals.is_empty(), "the family is focal-eligible");
    let before: Vec<(u64, bool)> = copy.sim().fauna().view().animals.iter().map(|a| (a.id, copy.sim().fauna().has_controller(a.id))).collect();
    drop(copy);
    let t = table(Founder::Loftstrider, 2);
    let kin = KinConfig::default();
    let plan = MatchedPlan { focal: focals[0], pair: 1, plus_first: true };
    let [p0, p1] = plan.copies();
    let a = matched::run_copy(&scene, p0, Some(&t), &kin).unwrap();
    let b = matched::run_copy(&scene, p1, Some(&t), &kin).unwrap();
    // The runner replaced one controller per copy, the focal's, with its sign.
    assert_eq!(a.installed, vec![(focals[0], Some(2))]);
    assert_eq!(b.installed, vec![(focals[0], Some(3))]);
    // Every body the scene founded is metered in both copies.
    for (id, driven) in before {
        assert!(driven, "the scene drives body {id}");
        assert!(a.scores.contains_key(&id) && b.scores.contains_key(&id), "body {id} metered in both");
    }
    assert!(!a.decisions.is_empty(), "the focal's decisions are recorded");
    let r = matched::run_pair(&scene, plan, &t, &kin, 1).unwrap();
    assert!(r.difference().is_some_and(f64::is_finite));
    let s = matched::pair_scores(&[r], 2);
    assert_eq!(s.pairs.len(), 1);
    assert_eq!(s.missing, 1, "pair 0 never ran: missing, not zero");
}

#[test]
fn a_null_pair_reports_its_gap() {
    let scene = engage(1.0);
    let copy = scene.open().unwrap();
    let focal = scene.focals(&copy, Founder::Chorister)[0];
    drop(copy);
    let t = table(Founder::Chorister, 1);
    let np = matched::null_pair(&scene, matched::CopyPlan { focal, candidate: Some(0) }, Some(&t), &KinConfig::default(), 2).unwrap();
    assert!(np.bodies > 0);
    assert!(np.focal_gap.is_finite() && np.max_combined_gap.is_finite());
    assert!(np.ticks[0] > 0 && np.ticks[1] > 0);
}

#[test]
fn a_saved_start_opens_into_copies() {
    let mut sim = small_sim(&[(Founder::Blind, 2)]);
    for _ in 0..5 {
        sim.step();
    }
    let start = matched::SimStart::capture(&sim, &[]);
    let scene = matched::SavedScene { start, ticks: 20, centres: Vec::new(), threads: 1, water_every: 1 };
    let mut c = scene.open().unwrap();
    assert_eq!(c.sim().world().tick(), sim.world().tick());
    let mut n = 0;
    while c.step() {
        n += 1;
    }
    assert_eq!(n, 20, "a saved scene runs its window");
}

// ---- step 5: stage 1 perturbs only the top ----

fn two_layer_file(f: Founder) -> crate::es::voxel::two_layer::TwoLayerPolicyFile {
    use crate::es::voxel::two_layer::{TwoLayerNet, TwoLayerPolicyFile, TwoLayerShape, initial_center};
    let mut shape = TwoLayerShape::plain_from_manifest(&f.manifest());
    shape.menu = 3;
    shape.spare = 2;
    shape.top_hidden = 4;
    let theta = initial_center(&shape, 11);
    let net = TwoLayerNet::new(shape, theta).unwrap();
    TwoLayerPolicyFile::new(f, &net, 3, "test")
}

#[test]
fn stage_one_leaves_every_bottom_parameter_untouched() {
    use super::config::BrainStage;
    use crate::es::voxel::two_layer::Layer;
    let file = two_layer_file(Founder::Loftstrider);
    let layout = file.layout();
    let (bottom, top) = (layout.range(Layer::Bottom), layout.range(Layer::Top));
    assert!(!top.is_empty());
    let one = super::stage::two_layer_variants(&file, BrainStage::Top, 3, 0.1, 0, 3).unwrap();
    assert_eq!(one.len(), 6);
    for v in &one {
        assert_eq!(&v.theta()[bottom.clone()], &file.theta[bottom.clone()], "stage 1 never moves the bottom");
        assert_ne!(&v.theta()[top.clone()], &file.theta[top.clone()], "stage 1 moves the top");
    }
    let two = super::stage::two_layer_variants(&file, BrainStage::Both, 3, 0.1, 0, 3).unwrap();
    assert!(two.iter().all(|v| v.theta()[bottom.clone()] != file.theta[bottom.clone()]), "stage 2 moves both");
    // The update draws the same restricted ε: a stage-1 step leaves the bottom where it is.
    let p = super::stage::Perturber::two_layer(&layout, BrainStage::Top);
    let mut theta = file.theta.clone();
    let mut adam = crate::es::optimizer::Adam::new(theta.len());
    let pairs = PairScores { pairs: vec![(0, 1.0, -1.0), (1, -0.5, 0.5), (2, 0.2, 0.1)], missing: 0 };
    compare::ascend_pairs(&mut theta, &mut adam, &pairs, 0.1, |k, out| p.eps(3, 0, k, out));
    assert_eq!(&theta[bottom.clone()], &file.theta[bottom], "the gradient is zero on the bottom");
    assert_ne!(&theta[top.clone()], &file.theta[top]);
    // A two-layer table drives bodies.
    let t = crate::es::voxel::live::variants::VariantTable::antithetic_two_layer(
        &file, BrainStage::Top, Default::default(), 3, 0.1, 0, 2).unwrap();
    assert_eq!(t.len(), 4);
    let obs = vec![0.1; Founder::Loftstrider.manifest().inputs()];
    let _ = t.controller(1).drive(&obs);
}

#[test]
fn plain_lineages_are_unchanged_by_the_stage() {
    use super::config::BrainStage;
    let f = Founder::Browser;
    let m = f.manifest();
    let centre = crate::es::tensor::initial_center_runtime(m.inputs(), m.outputs(), m.hidden, 5);
    let today = crate::es::voxel::live::variants::VariantTable::antithetic(f, &centre, 3, 0.05, 2, 3).unwrap();
    let plain = super::stage::Perturber::plain().antithetic(&centre, 3, 0.05, 2, 3);
    for (k, v) in plain.iter().enumerate() {
        assert_eq!(today.theta(k), v.as_slice(), "candidate {k}");
    }
    // A plain shape has no top: its two-layer perturber does not restrict.
    let shape = crate::es::voxel::two_layer::TwoLayerShape::plain_from_manifest(&m);
    let layout = crate::es::voxel::two_layer::TwoLayerLayout::new(&shape);
    assert_eq!(super::stage::Perturber::two_layer(&layout, BrainStage::Top), super::stage::Perturber::plain());
}
