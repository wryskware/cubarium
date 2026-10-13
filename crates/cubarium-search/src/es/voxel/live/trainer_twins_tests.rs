//! LIVE-TWINS: the live loop's generation with a lineage on the `twins` comparison, on a
//! small authored strip (windows and twin copies of at most a few hundred ticks).
//!
//! - a null table (σ 0) gives bit-identical twins and a zero update;
//! - a `twins` lineage leaves a `per_body` lineage's update and the live worlds untouched;
//! - without a `twins` lineage the generation is the loop's old steps exactly.

use super::*;
use crate::es::voxel::kin::{Comparison, KinConfig, LineageKin, TwinKnobs};

const A: Founder = Founder::Blind;
/// A second lineage whose variants change its window on the strip.
const B: Founder = Founder::Seedporter;

fn spec(name: &str, centres: Vec<(Founder, PathBuf)>, kin: KinConfig, sigma: f64, worlds: usize) -> LiveSpec {
    let out = std::env::temp_dir().join(format!("cub-live-twins-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    LiveSpec {
        centres,
        worlds,
        threads: 2,
        pairs: 2,
        window_ticks: 60,
        generations: 1,
        wall_seconds: 60.0,
        first_generation_seconds: 60.0,
        sigma,
        learning_rate: 0.01,
        train_seed: 3,
        world_seed: 7,
        water_every: 1,
        population_cap: 100,
        save_every: 1,
        out,
        resume: None,
        ecology: EcologyParams::defaults(),
        world: None,
        curiosity: Default::default(),
        stomach_aware_reward: false,
        kin,
        check: None,
        top_up: Vec::new(),
    }
}

fn twins(f: Founder, knobs: TwinKnobs) -> KinConfig {
    let mut k = KinConfig::default();
    k.lineages[f.index()] = LineageKin { comparison: Comparison::Twins, twins: knobs, ..Default::default() };
    k
}

/// A small authored strip with `counts` founders, one thread.
fn small_sim(counts: &[(Founder, usize)]) -> Sim {
    let cfg = cubarium_voxel::Config { width: 64, height: 24, depth: 6, ..Default::default() };
    let mut world = cubarium_voxel_sim::scene::authored(cfg);
    let mut flora = cubarium_voxel_flora::Flora::new(Default::default());
    let mut fauna = cubarium_voxel_fauna::Fauna::new(cubarium_voxel_fauna::FaunaConfig::placeholder());
    let mut n = [0; Founder::COUNT];
    for (f, k) in counts {
        n[f.index()] = *k;
    }
    cubarium_voxel_sim::habitat::seed_with_founder_counts(&mut world, &mut flora, &mut fauna, n);
    Sim::ambient(world, flora, fauna, cubarium_voxel_sim::SimConfig { threads: 1, overlap: false, water_every: 1 })
}

/// `worlds` small strips holding `counts`, prepared as the loop prepares a founded world.
fn worlds(spec: &LiveSpec, lineages: &[Lineage], counts: &[(Founder, usize)], n: usize) -> Vec<LiveWorld> {
    (0..n)
        .map(|id| {
            let mut sim = small_sim(counts);
            for _ in 0..3 * id {
                sim.step();
            }
            prepare(&mut sim, spec, lineages);
            LiveWorld {
                id,
                sim,
                ecology: spec.ecology.clone(),
                info: world::FoundInfo { seed: 7, accepted: true, lake_rejected: 0, habitat_rejected: 0, founders: [0; Founder::COUNT], stands: 0 },
                windows: 0,
                memory: Default::default(),
            }
        })
        .collect()
}

fn lineages(spec: &LiveSpec) -> Vec<Lineage> {
    let mut l: Vec<Lineage> = spec.centres.iter().map(|(f, p)| Lineage::load(spec, *f, p).unwrap()).collect();
    l.sort_by_key(|l| l.founder.index());
    l
}

/// The window the loop runs, without the staging run's checkpoints.
fn plain_window(spec: &LiveSpec) -> impl Fn(&mut LiveWorld, &[Option<VariantTable>], &[usize]) -> Result<WindowResult, String> + Sync + '_ {
    move |w, tables, first| {
        let memory = std::mem::take(&mut w.memory);
        let mut win = window::Window::begin_kin(&mut w.sim, tables, first, spec.window_ticks, &spec.curiosity, memory, &spec.kin);
        win.stomach_aware_reward = spec.stomach_aware_reward;
        while !win.done() {
            win.step(&mut w.sim);
        }
        let mut r = win.finish(&w.sim, w.id);
        w.memory = std::mem::take(&mut r.memory);
        w.windows += 1;
        Ok(r)
    }
}

fn bits(x: &[f64]) -> Vec<u64> {
    x.iter().map(|v| v.to_bits()).collect()
}

/// Every animal's id and pose, bit for bit.
fn poses(w: &LiveWorld) -> Vec<(u64, [u64; 4])> {
    w.sim.fauna().view().animals.iter().map(|a| (a.id, [a.pose.x, a.pose.z, a.pose.heading_rad, a.pose.y_m].map(f64::to_bits))).collect()
}

#[test]
fn a_null_table_gives_bit_identical_twins_and_a_zero_update() {
    // Half-and-half (the default; TWIN-MIX) and all-sign alike.
    let base = TwinKnobs { window_s: 4.0, pairs: 2, starts: 2, nulls: 1, ..Default::default() };
    for knobs in [base, TwinKnobs { carrier_share: 1.0, stranger_weight: 1.0, ..base }] {
        null_generation(knobs);
    }
}

fn null_generation(knobs: TwinKnobs) {
    let path = crate::es::voxel::store::test_centre_path(A, "twins-null");
    let spec = spec("null", vec![(A, path)], twins(A, knobs), 0.0, 2);
    let mut ls = lineages(&spec);
    let mut ws = worlds(&spec, &ls, &[(A, 4)], 2);
    let theta = ls[0].theta.clone();
    let adam = serde_json::to_string(&ls[0].adam).unwrap();
    let (results, updates, wall) = run_generation(&spec, &mut ls, &mut ws, 0, &plain_window(&spec), &|| Ok(())).unwrap();
    assert!(wall.is_some());
    let u = &updates[0];
    assert_eq!(u.comparison, Comparison::Twins);
    let t = u.twins.as_ref().expect("the twins readout");
    assert_eq!((t.measurements, t.measurements_missing), (4, 0));
    assert_eq!(t.pairs_scored, 2);
    assert_eq!(t.share_diverged, 0.0, "a null table's twins never differ: {t:?}");
    assert_eq!(t.share_d_zero, 1.0);
    assert_eq!((t.nulls, t.nulls_identical), (1, 1));
    assert_eq!(t.null_max_d, 0.0);
    if knobs.all_sign() {
        assert_eq!(t.mean_carriers, 0.0, "all-sign reads no estimate");
    } else {
        assert_eq!(t.mean_carriers, 2.0, "half of the four carry: {t:?}");
        assert_eq!((t.mean_direct, t.mean_social, t.mean_kin), (0.0, 0.0, 0.0));
    }
    assert_eq!((u.gradient_norm, u.update_rms), (0.0, 0.0));
    assert_eq!(bits(&ls[0].theta), bits(&theta), "the centre does not move");
    assert_eq!(serde_json::to_string(&ls[0].adam).unwrap(), adam, "nor does the optimizer");
    // The live windows ran with the lineage on its centre: metered, never on a variant.
    assert!(results.iter().all(|r| r.scores.iter().all(|b| b.founder != A)));
    assert!(results.iter().any(|r| r.metered.iter().any(|m| m.founder == A && m.candidate.is_none())));
    let _ = std::fs::remove_dir_all(&spec.out);
}

#[test]
fn a_twins_lineage_leaves_a_per_body_lineage_untouched() {
    let pa = crate::es::voxel::store::test_centre_path(A, "twins-iso-a");
    let pb = crate::es::voxel::store::test_centre_path(B, "twins-iso-b");
    let counts = [(A, 4), (B, 3)];
    let run = |name: &str, knobs: TwinKnobs| {
        let spec = spec(name, vec![(A, pa.clone()), (B, pb.clone())], twins(B, knobs), 0.05, 2);
        let mut ls = lineages(&spec);
        let mut ws = worlds(&spec, &ls, &counts, 2);
        let (results, updates, _) = run_generation(&spec, &mut ls, &mut ws, 0, &plain_window(&spec), &|| Ok(())).unwrap();
        let _ = std::fs::remove_dir_all(&spec.out);
        (results, updates, ls, ws)
    };
    let (r1, u1, l1, w1) = run("iso-1", TwinKnobs { window_s: 3.0, pairs: 1, starts: 1, nulls: 0, ..Default::default() });
    let (r2, u2, l2, w2) = run("iso-2", TwinKnobs { window_s: 5.0, pairs: 3, starts: 2, nulls: 1, ..Default::default() });
    // A's update is byte-identical whatever B's twins did.
    assert_eq!(u1[0].lineage, A.name());
    assert!(u1[0].bodies > 0, "A trained on its variants");
    assert_eq!(serde_json::to_string(&u1[0]).unwrap(), serde_json::to_string(&u2[0]).unwrap());
    assert_eq!(bits(&l1[0].theta), bits(&l2[0].theta));
    let scores = |r: &[WindowResult]| -> Vec<(u64, usize, u64)> {
        r.iter().flat_map(|w| w.scores.iter().map(|b| (b.body_id, b.candidate, b.score.score.to_bits()))).collect()
    };
    assert_eq!(scores(&r1), scores(&r2));
    // The live worlds lived on identically: the twin copies never touch them.
    for (a, b) in w1.iter().zip(&w2) {
        assert_eq!(poses(a), poses(b));
    }
    // B was trained from its copies, differently under the two layouts.
    assert_eq!(u1[1].comparison, Comparison::Twins);
    assert_eq!(u1[1].twins.as_ref().unwrap().measurements, 1);
    assert_eq!(u2[1].twins.as_ref().unwrap().measurements, 6);
    assert!(u2[1].twins.as_ref().unwrap().share_diverged > 0.0 && u2[1].gradient_norm > 0.0, "B's twins differ and move it");
    assert_ne!(bits(&l1[1].theta), bits(&l2[1].theta), "{}\n{}", serde_json::to_string(&u1[1]).unwrap(), serde_json::to_string(&u2[1]).unwrap());
}

#[test]
fn without_twins_the_generation_is_the_old_loop_step() {
    let path = crate::es::voxel::store::test_centre_path(A, "twins-plain");
    let spec = spec("plain", vec![(A, path)], KinConfig::default(), 0.05, 1);
    let (mut l1, mut l2) = (lineages(&spec), lineages(&spec));
    let (mut w1, mut w2) = (worlds(&spec, &l1, &[(A, 4)], 1), worlds(&spec, &l2, &[(A, 4)], 1));
    let (r1, u1, wall) = run_generation(&spec, &mut l1, &mut w1, 0, &plain_window(&spec), &|| Ok(())).unwrap();
    assert!(wall.is_none());
    // The old steps 1-3 by hand: the tables, one world's window from pair 0, the update.
    let tables = tables_for(&spec, &l2, 0).unwrap();
    let r2 = plain_window(&spec)(&mut w2[0], &tables, &[0; Founder::COUNT]).unwrap();
    let u2 = update(&spec, &mut l2[0], std::slice::from_ref(&r2), 0);
    assert_eq!(serde_json::to_string(&u1[0]).unwrap(), serde_json::to_string(&u2).unwrap());
    assert!(!serde_json::to_string(&u1[0]).unwrap().contains("twins"), "no twins field on a plain record");
    assert_eq!(bits(&l1[0].theta), bits(&l2[0].theta));
    assert_eq!(r1[0].scores.len(), r2.scores.len());
    let _ = std::fs::remove_dir_all(&spec.out);
}

