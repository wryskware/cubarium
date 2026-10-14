//! PG-PILOT: the live loop's generation with a lineage on the `policy_gradient`
//! comparison, on the staged feed scene's world (windows of a few hundred ticks).
//!
//! - the window runs the lineage on its centre, exploring: no variants, every body logged
//!   (decisions and an energy reading every tick), metered like a centre's body;
//! - each body's brain carries its memory into the exploring window;
//! - the update moves the top's intent head and nothing else, and the record carries the
//!   `pg` readout;
//! - a plain centre, a stage-2 table or the stomach-aware reward are refused by name.

use super::*;
use crate::es::voxel::kin::{BrainStage, Comparison, KinConfig, KinShares, LineageKin};
use crate::es::voxel::two_layer::{Layer, MenuHeads, TwoLayerNet, TwoLayerShape, initial_center};
use cubarium_voxel_sim::mode_scenes::{self, ModeScene};

const F: Founder = Founder::Lanternjaw;

fn centre(name: &str) -> PathBuf {
    let heads = MenuHeads { kick: false, call_kinds: 0, rest: false, top_hidden: 8, pointer_width: 4 };
    let shape = TwoLayerShape::menu_from_manifest(&F.manifest(), heads);
    let net = TwoLayerNet::new(shape.clone(), initial_center(&shape, 17)).unwrap();
    let path = std::env::temp_dir().join(format!("cub-pg-centre-{}-{name}.json", std::process::id()));
    TwoLayerPolicyFile::new(F, &net, 3, "test").write(&path).unwrap();
    path
}

fn pg_kin() -> KinConfig {
    let mut k = KinConfig::default();
    k.lineages[F.index()] = LineageKin { comparison: Comparison::PolicyGradient, shares: KinShares::uniform(0.5), ..Default::default() };
    k
}

fn spec(name: &str, path: PathBuf, kin: KinConfig) -> LiveSpec {
    let out = std::env::temp_dir().join(format!("cub-live-pg-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    LiveSpec {
        centres: vec![(F, path)],
        worlds: 2,
        threads: 2,
        pairs: 2,
        window_ticks: 160,
        generations: 1,
        wall_seconds: 60.0,
        first_generation_seconds: 60.0,
        sigma: 0.05,
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

/// Feed-scene worlds (lanternjaws and their prey), prepared as the loop prepares a world.
fn worlds(spec: &LiveSpec, lineages: &[Lineage], n: usize) -> Vec<LiveWorld> {
    let cfg = mode_scenes::Config { duration_s: 10.0, ..mode_scenes::Config::default() };
    (0..n)
        .map(|id| {
            let mut sim = mode_scenes::found(&cfg, ModeScene::Feed, 1 + id as u64).unwrap().sim;
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

fn plain_window(spec: &LiveSpec) -> impl Fn(&mut LiveWorld, &[Option<VariantTable>], &[usize]) -> Result<WindowResult, String> + Sync + '_ {
    move |w, tables, first| {
        let memory = std::mem::take(&mut w.memory);
        let mut win = window::Window::begin_kin(&mut w.sim, tables, first, spec.window_ticks, &spec.curiosity, memory, &spec.kin);
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

#[test]
fn a_policy_gradient_generation_logs_every_body_and_moves_only_the_intent_head() {
    let path = centre("gen");
    let spec = spec("gen", path.clone(), pg_kin());
    let mut ls = vec![Lineage::load(&spec, F, &path).unwrap()];
    let mut ws = worlds(&spec, &ls, 2);
    let before = ls[0].theta.clone();
    let layout = TwoLayerPolicyFile::load(&path).unwrap().layout();
    let top = layout.top.unwrap();
    let (results, updates, wall) = run_generation(&spec, &mut ls, &mut ws, 0, &plain_window(&spec), &|| Ok(())).unwrap();
    assert!(wall.is_none(), "no twin copies");
    for r in &results {
        // No variants: the lineage is metered as a centre's, never scored as a candidate.
        assert!(r.scores.iter().all(|b| b.founder != F));
        assert!(r.metered.iter().any(|m| m.founder == F && m.candidate.is_none()));
        let w = r.pg.iter().find(|w| w.founder == F).expect("the lineage's record");
        assert!(!w.bodies.is_empty());
        for b in &w.bodies {
            if b.start == 0 && !b.died {
                assert_eq!(b.series.len() as u64, spec.window_ticks + 1, "a reading at the start and after every tick");
            }
            assert!(b.decisions.iter().all(|d| d.tick < spec.window_ticks && d.valid[d.intent]));
        }
        assert!(w.bodies.iter().any(|b| !b.decisions.is_empty()), "the bodies decided");
    }
    let u = &updates[0];
    assert_eq!(u.comparison, Comparison::PolicyGradient);
    let p = u.pg.as_ref().expect("the pg readout");
    assert!(p.decisions > 0);
    assert!(serde_json::to_string(u).unwrap().contains("\"pg\""));
    // Only the intent head moved: the bottom and the pointer are the centre's bit for bit.
    let head = top.w1..top.wr;
    for (i, (a, b)) in ls[0].theta.iter().zip(&before).enumerate() {
        if !head.contains(&i) {
            assert_eq!(a.to_bits(), b.to_bits(), "parameter {i} is outside the intent head");
        }
    }
    assert_ne!(bits(&ls[0].theta[head.clone()]), bits(&before[head]), "the intent head moved");
    // The checkpoint keeps the learner, and a plain run's checkpoint writes no `pg`.
    save_checkpoint(&spec, &ls, &ws, 1, 99).unwrap();
    let mut back = vec![Lineage::load(&spec, F, &path).unwrap()];
    load_checkpoint(&spec, &spec.out, &mut back).unwrap();
    assert_eq!(back[0].pg, ls[0].pg);
    assert_eq!(bits(&back[0].theta), bits(&ls[0].theta));
    let _ = std::fs::remove_dir_all(&spec.out);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn an_exploring_window_carries_each_bodys_memory_on() {
    let path = centre("memory");
    let spec = spec("memory", path.clone(), pg_kin());
    let ls = vec![Lineage::load(&spec, F, &path).unwrap()];
    let mut w = worlds(&spec, &ls, 1).remove(0);
    for _ in 0..40 {
        w.sim.step();
    }
    let ids: Vec<u64> = w.sim.fauna().view().animals.iter().filter(|a| a.founder == Some(F)).map(|a| a.id).collect();
    assert!(!ids.is_empty());
    let memory = |sim: &mut cubarium_voxel_sim::Sim, id: u64| {
        let c = sim.fauna_mut().take_controller(id).expect("a controller");
        let m = c.memory().expect("a brain's memory");
        sim.fauna_mut().set_controller(id, c);
        m
    };
    let held: Vec<_> = ids.iter().map(|&id| memory(&mut w.sim, id)).collect();
    let tables = tables_for(&spec, &ls, 0).unwrap();
    let win = window::Window::begin_kin(&mut w.sim, &tables, &[0; Founder::COUNT], spec.window_ticks, &spec.curiosity, Default::default(), &spec.kin);
    for (id, m) in ids.iter().zip(&held) {
        assert_eq!(&memory(&mut w.sim, *id), m, "body {id} continues its hold and GRU state");
    }
    drop(win);
    let _ = std::fs::remove_dir_all(&spec.out);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn what_the_policy_gradient_cannot_train_is_refused_by_name() {
    // A plain centre.
    let plain = crate::es::voxel::store::test_centre_path(Founder::Blind, "pg-plain");
    let mut kin = KinConfig::default();
    kin.lineages[Founder::Blind.index()].comparison = Comparison::PolicyGradient;
    let s = LiveSpec { centres: vec![(Founder::Blind, plain.clone())], ..spec("refuse-plain", plain.clone(), kin) };
    let err = Lineage::load(&s, Founder::Blind, &plain).err().expect("refused");
    assert!(err.contains("plain centre"), "{err}");
    // The stomach-aware reward.
    let path = centre("refuse");
    let s = LiveSpec { stomach_aware_reward: true, ..spec("refuse-stomach", path.clone(), pg_kin()) };
    let err = Lineage::load(&s, F, &path).err().expect("refused");
    assert!(err.contains("stomach-aware"), "{err}");
    // Stage 2, at the kin file.
    let err = KinConfig::parse("[lanternjaw]\ncomparison = \"policy_gradient\"\nstage = \"both\"\n").err().expect("refused");
    assert!(err.contains("stage 1"), "{err}");
    let _ = BrainStage::Both;
    let _ = std::fs::remove_file(&path);
    let _ = Layer::Top;
}
