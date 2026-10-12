//! BRAIN-NEXT step 1: the live trainer with two-layer centres beside plain ones. A plain
//! lineage's variants, window scores, update and written centre are today's, bit for bit;
//! a two-layer lineage at stage 1 moves only its top, through the variants, the update and
//! the write-back.

use super::*;
use crate::es::voxel::two_layer::{Layer, MenuHeads, TwoLayerNet, TwoLayerShape, initial_center};

fn small_sim(f: Founder, n: usize) -> Sim {
    let cfg = cubarium_voxel::Config { width: 64, height: 24, depth: 6, ..Default::default() };
    let mut world = cubarium_voxel_sim::scene::authored(cfg);
    let mut flora = cubarium_voxel_flora::Flora::new(Default::default());
    let mut fauna = cubarium_voxel_fauna::Fauna::new(cubarium_voxel_fauna::FaunaConfig::placeholder());
    let mut counts = [0; Founder::COUNT];
    counts[f.index()] = n;
    cubarium_voxel_sim::habitat::seed_with_founder_counts(&mut world, &mut flora, &mut fauna, counts);
    Sim::ambient(world, flora, fauna, cubarium_voxel_sim::SimConfig { threads: 1, overlap: false, water_every: 1 })
}

fn spec(name: &str, centres: Vec<(Founder, PathBuf)>) -> LiveSpec {
    let out = std::env::temp_dir().join(format!("cub-live-brain-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    LiveSpec {
        centres,
        worlds: 1,
        threads: 1,
        pairs: 2,
        window_ticks: 80,
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
        kin: Default::default(),
        check: None,
        top_up: Vec::new(),
    }
}

fn window(sim: &mut Sim, tables: &[Option<VariantTable>], spec: &LiveSpec) -> WindowResult {
    let mut w = window::Window::begin_kin(sim, tables, &[0; Founder::COUNT], spec.window_ticks, &spec.curiosity, Default::default(), &spec.kin);
    while !w.done() {
        w.step(sim);
    }
    w.finish(sim, 0)
}

fn bits(x: &[f64]) -> Vec<u64> {
    x.iter().map(|v| v.to_bits()).collect()
}

#[test]
fn a_plain_lineage_trains_exactly_as_today() {
    let f = Founder::Blind;
    let path = crate::es::voxel::store::test_centre_path(f, "brain-next-plain");
    let spec = spec("plain", vec![(f, path.clone())]);
    let file = VoxelPolicyFile::load_or_transfer(&path).unwrap();
    let mut l = Lineage::load(&spec, f, &path).unwrap();
    assert!(matches!(l.template, BrainFile::Plain(_)));

    // The variants: today's table, bit for bit.
    let tables = tables_for(&spec, std::slice::from_ref(&l), 0).unwrap();
    let today = VariantTable::antithetic(f, &file.theta, spec.train_seed, spec.sigma, 0, spec.pairs).unwrap();
    let mine = tables[f.index()].as_ref().unwrap();
    assert_eq!(mine.len(), today.len());
    for c in 0..today.len() {
        assert_eq!(bits(mine.theta(c)), bits(today.theta(c)), "candidate {c}");
    }

    // The window scores on twin worlds: identical.
    let mut today_tables = vec![None; Founder::COUNT];
    today_tables[f.index()] = Some(today);
    let (mut a, mut b) = (small_sim(f, 4), small_sim(f, 4));
    let ra = window(&mut a, &tables, &spec);
    let rb = window(&mut b, &today_tables, &spec);
    assert!(!ra.scores.is_empty());
    assert_eq!(ra.scores.len(), rb.scores.len());
    for (x, y) in ra.scores.iter().zip(&rb.scores) {
        assert_eq!((x.body_id, x.candidate, x.score.score.to_bits()), (y.body_id, y.candidate, y.score.score.to_bits()));
    }

    // The update: today's per-body scores and ascent.
    let mut theta = file.theta.clone();
    let mut adam = Adam::new(theta.len());
    adam.learning_rate = spec.learning_rate;
    let refs = [rb.scores.as_slice()];
    let (scores, _) = crate::es::voxel::kin::compare::per_body(&refs, 2 * spec.pairs);
    ascend(&mut theta, &mut adam, &scores, spec.sigma, spec.train_seed, 0);
    let u = update(&spec, &mut l, std::slice::from_ref(&ra), 0);
    assert_eq!(bits(&u.candidate_scores), bits(&scores));
    assert_eq!(bits(&l.theta), bits(&theta));

    // The written centre: today's file, byte for byte.
    save_centres(&spec, std::slice::from_ref(&l), 1, "final").unwrap();
    let written = std::fs::read(final_centre(&spec.out, f)).unwrap();
    let expected = VoxelPolicyFile {
        build: crate::evaluate::BUILD_ID.into(),
        train_seed: spec.train_seed,
        generation: Some(1),
        score: None,
        arena_protocol: LIVE_PROTOCOL.into(),
        protocol_hash: None,
        imitation: None,
        theta,
        ..file.clone()
    };
    let check = spec.out.join("expected.json");
    expected.write(&check).unwrap();
    assert_eq!(written, std::fs::read(&check).unwrap());
    // And its checkpoint entry is today's JSON.
    assert_eq!(
        serde_json::to_string(&SavedTemplate::from(&l.template)).unwrap(),
        serde_json::to_string(&file).unwrap()
    );
    let back: SavedTemplate = serde_json::from_str(&serde_json::to_string(&file).unwrap()).unwrap();
    assert!(matches!(BrainFile::from(back), BrainFile::Plain(_)));
    std::fs::remove_dir_all(&spec.out).unwrap();
}

fn two_layer_centre(f: Founder, name: &str) -> PathBuf {
    let heads = MenuHeads { kick: false, call_kinds: 0, rest: false, top_hidden: 8, pointer_width: 4 };
    let shape = TwoLayerShape::menu_from_manifest(&f.manifest(), heads);
    let net = TwoLayerNet::new(shape.clone(), initial_center(&shape, 17)).unwrap();
    let path = std::env::temp_dir().join(format!("cub-two-layer-centre-{}-{name}.json", std::process::id()));
    TwoLayerPolicyFile::new(f, &net, 3, "test").write(&path).unwrap();
    path
}

#[test]
fn a_two_layer_stage_one_run_moves_only_the_top() {
    let f = Founder::Seedporter;
    let path = two_layer_centre(f, "stage1");
    let spec = spec("two-layer", vec![(f, path.clone())]);
    assert_eq!(spec.kin.of(f).stage, crate::es::voxel::kin::BrainStage::Top, "stage 1 is the default");
    let file = TwoLayerPolicyFile::load(&path).unwrap();
    let layout = file.layout();
    let (bottom, top) = (layout.range(Layer::Bottom), layout.range(Layer::Top));
    assert!(!top.is_empty());
    let mut l = Lineage::load(&spec, f, &path).unwrap();
    assert!(matches!(l.template, BrainFile::TwoLayer(_)));
    assert!(matches!(l.driver(&spec).unwrap().kind(), crate::es::voxel::controller::EpisodeKind::TwoLayer(..)));

    // Every variant keeps the bottom and moves the top.
    let tables = tables_for(&spec, std::slice::from_ref(&l), 0).unwrap();
    let t = tables[f.index()].as_ref().unwrap();
    assert_eq!(t.len(), 2 * spec.pairs);
    for c in 0..t.len() {
        assert_eq!(bits(&t.theta(c)[bottom.clone()]), bits(&file.theta[bottom.clone()]), "candidate {c}'s bottom");
        assert_ne!(&t.theta(c)[top.clone()], &file.theta[top.clone()], "candidate {c}'s top");
    }

    // A window on two-layer brains, then the update: the bottom stays, the top moves.
    let mut sim = small_sim(f, 4);
    let r = window(&mut sim, &tables, &spec);
    assert!(r.scores.len() >= 2, "the lineage's bodies ran variants");
    let u = update(&spec, &mut l, std::slice::from_ref(&r), 0);
    assert!(u.gradient_norm > 0.0, "the scores gave a direction");
    assert_eq!(bits(&l.theta[bottom.clone()]), bits(&file.theta[bottom.clone()]), "the update leaves the bottom");
    assert_ne!(&l.theta[top.clone()], &file.theta[top.clone()], "the update moves the top");

    // Written back as a two-layer file: the bottom is the copy's, the top the update's.
    save_centres(&spec, std::slice::from_ref(&l), 1, "final").unwrap();
    let back = TwoLayerPolicyFile::load(&final_centre(&spec.out, f)).unwrap();
    assert_eq!(back.shape, file.shape);
    assert_eq!(back.generation, Some(1));
    assert_eq!(bits(&back.theta[bottom.clone()]), bits(&file.theta[bottom]));
    assert_eq!(bits(&back.theta[top.clone()]), bits(&l.theta[top]));
    // And the next generation's table is built from the written centre's weights.
    let again = Lineage::load(&spec, f, &final_centre(&spec.out, f)).unwrap();
    assert_eq!(bits(&again.theta), bits(&l.theta));

    // The checkpoint's template round-trips as a two-layer file.
    let text = serde_json::to_string(&SavedTemplate::from(&l.template)).unwrap();
    let saved: SavedTemplate = serde_json::from_str(&text).unwrap();
    assert!(matches!(BrainFile::from(saved), BrainFile::TwoLayer(_)));
    std::fs::remove_dir_all(&spec.out).unwrap();
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_centre_for_another_lineage_is_refused() {
    let path = two_layer_centre(Founder::Seedporter, "refuse");
    let spec = spec("refuse", Vec::new());
    let err = Lineage::load(&spec, Founder::Lanternjaw, &path).err().expect("refused");
    assert!(err.contains("seedporter"), "{err}");
    std::fs::remove_dir_all(&spec.out).unwrap();
    let _ = std::fs::remove_file(&path);
}


/// BRAIN-FIX step 4: a lanternjaw stage-1 generation's write-back loads and drives. One
/// window on the staged feed scene's world with the lineage on its variants, the update,
/// the write-back; the written centre then loads as a brain driver and decides for a
/// lanternjaw in a fresh scene (its records are the brain's, not the rules').
#[test]
fn a_lanternjaw_stage_one_write_back_loads_and_drives() {
    use cubarium_voxel_sim::mode_scenes::{self, ModeScene};
    let f = Founder::Lanternjaw;
    let path = two_layer_centre(f, "lj-stage1");
    let spec = spec("lj-stage1", vec![(f, path.clone())]);
    let mut l = Lineage::load(&spec, f, &path).unwrap();
    let tables = tables_for(&spec, std::slice::from_ref(&l), 0).unwrap();
    let cfg = mode_scenes::Config { duration_s: 10.0, ..mode_scenes::Config::default() };
    let mut sim = mode_scenes::found(&cfg, ModeScene::Feed, 1).unwrap().sim;
    let r = window(&mut sim, &tables, &spec);
    assert!(!r.scores.is_empty(), "the lanternjaw ran a variant");
    update(&spec, &mut l, std::slice::from_ref(&r), 0);
    save_centres(&spec, std::slice::from_ref(&l), 1, "final").unwrap();
    let back = final_centre(&spec.out, f);
    let driver = crate::es::voxel::store::load_brain_driver(&back, f).expect("the write-back loads");
    let mut e = mode_scenes::found(&cfg, ModeScene::Feed, 2).unwrap();
    crate::es::voxel::live::install_centres(e.sim.fauna_mut(), &[(f, driver)]);
    // The dry teacher labels brain-driven bodies only: its labels mark a brain at the wheel.
    e.sim.fauna_mut().dry_teach(f, Some(cubarium_voxel_fauna::DryScope::Brains));
    e.sim.fauna_mut().start_tap();
    let id = e.focal[0];
    let mut sampled = 0;
    for _ in 0..100 {
        e.step();
        let a = *e.sim.fauna().view().animal(id).expect("alive");
        sampled += usize::from(a.intent.sample_tick == e.sim.fauna().tick());
    }
    assert!(sampled > 0, "the brain decided");
    let tap = e.sim.fauna_mut().take_tap().unwrap();
    assert!(tap.teacher_labels(id).is_some_and(|l| !l.is_empty()), "a brain drives it");
    std::fs::remove_dir_all(&spec.out).unwrap();
    let _ = std::fs::remove_file(&path);
}

/// BRAIN-FIX step 4: the check runs a brain beside its heuristic on matched starts and
/// reads energy, holds and per-mode competence; on one start both arms see the same world.
#[test]
fn the_brain_check_reads_both_arms_on_matched_starts() {
    use crate::es::voxel::two_layer::check::{CheckSpec, check};
    use cubarium_voxel_sim::mode_scenes::{self, ModeScene};
    let f = Founder::Lanternjaw;
    let path = two_layer_centre(f, "lj-check");
    let s = spec("lj-check", vec![(f, path.clone())]);
    let driver = Lineage::load(&s, f, &path).unwrap().driver(&s).unwrap();
    let c = CheckSpec { every: 1, seeds: vec![1, 2], seconds: 5, scene_seeds: vec![1], threads: 2 };
    let found = |seed: u64| -> Result<Sim, String> { Ok(mode_scenes::found(&mode_scenes::Config::default(), ModeScene::Feed, seed)?.sim) };
    let r = check(f, &driver, 3, &c, &found);
    assert_eq!((r.heuristic.len(), r.brain.len()), (2, 2), "{:?}", r.skipped);
    for (h, b) in r.heuristic.iter().zip(&r.brain) {
        assert_eq!(h.seed, b.seed);
        assert_eq!((h.energy_start, h.bodies_start), (b.energy_start, b.bodies_start), "one start per seed");
    }
    assert!(r.holds_brain.decision_samples > 0 && r.holds_heuristic.decision_samples > 0);
    let names: Vec<_> = r.scenes.iter().map(|x| x.intent).collect();
    assert_eq!(names, ["feed", "give_way"]);
    assert!(r.gain_per_body_brain_minus_heuristic.is_finite());
    let _ = std::fs::remove_dir_all(&s.out);
    let _ = std::fs::remove_file(&path);
}
