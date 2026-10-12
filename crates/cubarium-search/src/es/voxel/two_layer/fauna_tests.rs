//! TWO-LAYER half 2's checks against the fauna: the adapter round trip and a save mid-hold
//! in a real fauna world. Mechanisms on hand-built fixtures, never a tuned number.

use std::sync::{Arc, Mutex};

use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::subjects::RowKind;
use cubarium_voxel_fauna::{
    Activity, BrainMemory, Command, Controller, Decision, DecisionInput, Fauna, FaunaConfig, Founder,
    Response, StartingStores,
};
use cubarium_voxel_flora::{Flora, FloraConfig};

use super::adapter::MenuHeads;
use super::brain::{TwoLayerBrain, TwoLayerNet};
use super::shape::{TwoLayerShape, initial_center};

fn heads() -> MenuHeads {
    MenuHeads {
        kick: true,
        call_kinds: 2,
        rest: true,
        top_hidden: 8,
        pointer_width: 6,
    }
}

fn loftstrider_net(seed: u64) -> Arc<TwoLayerNet> {
    let shape = TwoLayerShape::menu_from_manifest(&Founder::Loftstrider.manifest(), heads());
    Arc::new(TwoLayerNet::new(shape.clone(), initial_center(&shape, seed)).unwrap())
}

/// A hand-built decision input: `K` rows, rows `bodies` hold bodies keyed `100 + r`, the
/// rest empty; every body row valid for every named intent.
struct Input {
    menu: usize,
    mask: Vec<bool>,
    flat: Vec<f64>,
    kinds: Vec<RowKind>,
    keys: Vec<u64>,
    cmask: Vec<bool>,
    width: usize,
}

impl Input {
    fn new(shape: &TwoLayerShape, order: &[u64], seed: u64) -> Input {
        let (menu, k, w) = (shape.intents(), shape.candidates, shape.row_width);
        let mut flat = vec![0.0; k * w];
        let mut kinds = vec![RowKind::Empty; k];
        let mut keys = vec![cubarium_voxel_fauna::subjects::EMPTY_KEY; k];
        let mut cmask = vec![false; k * menu];
        for (r, &key) in order.iter().enumerate() {
            kinds[r] = RowKind::Body;
            keys[r] = key;
            // A row's features are its body's own, wherever it sits.
            for c in 0..w {
                flat[r * w + c] = ((key * 31 + c as u64 * 7 + seed) % 97) as f64 / 97.0;
            }
            for i in 0..shape.menu {
                cmask[r * menu + i] = true;
            }
        }
        let mut mask = vec![true; menu];
        for m in mask.iter_mut().skip(shape.menu) {
            *m = false;
        }
        Input { menu, mask, flat, kinds, keys, cmask, width: w }
    }

    fn view(&self, held_row: Option<usize>) -> DecisionInput<'_> {
        DecisionInput {
            menu: self.menu,
            intent_mask: &self.mask,
            candidates: &self.flat,
            row_width: self.width,
            kinds: &self.kinds,
            keys: &self.keys,
            candidate_mask: &self.cmask,
            held_row,
            early_end: None,
        }
    }
}

/// The adapter maps the fauna's rows by key: the held subject's row follows its body when
/// the rows reorder, the answer's row is the fauna's row, the proposal is the menu's, and a
/// memory round trip continues identically.
#[test]
fn the_adapter_maps_rows_by_key_and_its_memory_round_trips() {
    let net = loftstrider_net(5);
    let shape = net.shape().clone();
    let arb = cubarium_voxel_fauna::BrainArbitration {
        hold_s: 0.5,
        ..Default::default()
    };
    let period = Founder::Loftstrider.manifest().controller_period_s;
    let mut brain = net.brain(arb, period);
    let obs: Vec<f64> = (0..shape.observation).map(|k| (k % 5) as f64 * 0.2).collect();
    let forward = [101u64, 102, 103];
    let backward = [103u64, 102, 101];
    let mut held_row = None;
    let mut seen_subject = false;
    let mut decisions: Vec<Decision> = Vec::new();
    for t in 0..24 {
        let order: &[u64] = if t % 2 == 0 { &forward } else { &backward };
        let input = Input::new(&shape, order, 0);
        let (r, d) = brain.decide(&obs, &input.view(held_row));
        let d = d.expect("a menu brain decides");
        assert!(matches!(r, Response::Logits(_)));
        assert!(d.intent < shape.menu, "a named intent is held");
        let p: f64 = d.proposal[..shape.intents()].iter().sum();
        assert!((p - 1.0).abs() < 1e-9, "the proposal is over the menu");
        assert!(d.proposal[shape.menu..].iter().all(|v| *v == 0.0), "spares never proposed");
        if let Some(row) = d.subject {
            seen_subject = true;
            let key = input.keys[row];
            assert_eq!(Some(key), brain.held_subject().map(|s| s.key), "the row is the held body's");
        }
        held_row = d.subject;
        decisions.push(d);
    }
    assert!(seen_subject, "the fixture exercises a subject");

    // Memory round trip mid-run: a fresh brain restored from it continues identically.
    let memory: BrainMemory = brain.memory().expect("a two-layer brain keeps memory");
    let mut restored = net.brain(arb, period);
    assert!(restored.restore(&memory));
    for t in 24..40 {
        let order: &[u64] = if t % 3 == 0 { &forward } else { &backward };
        let input = Input::new(&shape, order, 0);
        let a = brain.decide(&obs, &input.view(held_row));
        let b = restored.decide(&obs, &input.view(held_row));
        assert_eq!(a, b, "sample {t}");
        held_row = a.1.and_then(|d| d.subject);
    }
    // Another brain's memory is refused, and the brain starts fresh.
    let mut other = net.brain(arb, period);
    assert!(!other.restore(&BrainMemory {
        tag: 7,
        bytes: vec![1, 2, 3]
    }));
}

/// A flat soil plain, the clock off.
fn plain_world(seed: u64) -> World {
    let (width, depth, height) = (48u32, 12u32, 8u32);
    let mut w = World::empty(VoxelConfig {
        width,
        height,
        depth,
        voxel_m: 0.25,
        seed,
        day: DayClock {
            mode: DayMode::Off,
            ..DayClock::default()
        },
        ..VoxelConfig::default()
    });
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=2 {
                w.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    w
}

/// The latest copy of a brain after its last decision (a test's view into the fauna's
/// boxed controller).
type Snapshot = Arc<Mutex<Option<TwoLayerBrain>>>;

/// A two-layer brain that keeps a copy of itself after each decision, and — in a loaded
/// world — runs an uninterrupted `twin` and a cold-started `cold` copy on the very same
/// inputs, recording whether each answered as it did.
struct Spy {
    brain: TwoLayerBrain,
    snapshot: Snapshot,
    twin: Option<TwoLayerBrain>,
    cold: Option<TwoLayerBrain>,
    /// `(decisions, matched the twin, matched the cold copy)`.
    tally: Arc<Mutex<(usize, usize, usize)>>,
    restored: Arc<Mutex<bool>>,
}

impl Controller for Spy {
    fn drive(&mut self, o: &[f64]) -> Response {
        self.brain.drive(o)
    }
    fn reset(&mut self) {
        self.brain.reset();
    }
    fn decide(&mut self, o: &[f64], input: &DecisionInput<'_>) -> (Response, Option<Decision>) {
        let answer = self.brain.decide(o, input);
        *self.snapshot.lock().unwrap() = Some(self.brain.clone());
        if let (Some(twin), Some(cold)) = (self.twin.as_mut(), self.cold.as_mut()) {
            let (t, c) = (twin.decide(o, input), cold.decide(o, input));
            let mut tally = self.tally.lock().unwrap();
            tally.0 += 1;
            tally.1 += usize::from(t == answer);
            tally.2 += usize::from(c == answer);
        }
        answer
    }
    fn memory(&self) -> Option<BrainMemory> {
        self.brain.memory()
    }
    fn restore(&mut self, m: &BrainMemory) -> bool {
        let ok = self.brain.restore(m);
        *self.restored.lock().unwrap() = ok;
        ok
    }
}

fn spy(brain: TwoLayerBrain, twin: Option<TwoLayerBrain>, cold: Option<TwoLayerBrain>) -> (Spy, Snapshot) {
    let snapshot = Snapshot::default();
    (
        Spy {
            brain,
            snapshot: snapshot.clone(),
            twin,
            cold,
            tally: Arc::default(),
            restored: Arc::default(),
        },
        snapshot,
    )
}

/// **A save mid-hold continues identically** at the fauna level: two loftstriders on brains
/// and a Veilhound beside them (a threat row). Saved while the brains hold an intent, the
/// loaded world's fresh brains get their memory back from the step before their next
/// sample and then answer exactly as an uninterrupted copy of each brain does on the same
/// inputs; a cold-started copy does not. (The loaded world itself need not be bit-identical
/// to the original; the brain's continuation is what is checked.)
#[test]
fn a_fauna_save_mid_hold_continues_the_brains_identically() {
    let w = plain_world(3);
    let mut config = FaunaConfig::default();
    for f in Founder::ALL {
        config.founders[f.index()].active = Activity::Always;
    }
    let mut f = Fauna::new(config);
    let mut flora = Flora::new(FloraConfig::default());
    let mut brains = Vec::new();
    for (x, founder) in [(10, Founder::Loftstrider), (14, Founder::Loftstrider), (22, Founder::Chorister)] {
        assert!(f.apply(
            &w,
            Command::IntroduceFounder {
                x,
                z: 6,
                founder,
                stores: StartingStores::FULL,
                heading_rad: std::f64::consts::FRAC_PI_2,
            }
        ));
        if founder == Founder::Loftstrider {
            brains.push(f.view().animals.last().unwrap().id);
        }
    }
    let net = loftstrider_net(11);
    let arb = cubarium_voxel_fauna::BrainArbitration::default();
    let period = Founder::Loftstrider.manifest().controller_period_s;
    let mut snapshots = Vec::new();
    for &id in &brains {
        let (c, snap) = spy(net.brain(arb, period), None, None);
        assert!(f.set_controller(id, Box::new(c)));
        snapshots.push(snap);
    }
    for _ in 0..120 {
        f.step(&w, &mut flora);
    }
    let bytes = f.save();
    let mut g = Fauna::load(&bytes).expect("loads");
    let mut tallies = Vec::new();
    for (k, &id) in brains.iter().enumerate() {
        let twin = snapshots[k].lock().unwrap().clone().expect("the brain decided");
        let state = twin.state();
        assert!(state.held.is_some() && state.hold_age_s > 0.0, "saved mid-hold");
        assert_eq!(f.view().animal(id).unwrap().intent, g.view().animal(id).unwrap().intent);
        let (c, _) = spy(net.brain(arb, period), Some(twin), Some(net.brain(arb, period)));
        tallies.push((c.tally.clone(), c.restored.clone()));
        assert!(g.set_controller(id, Box::new(c)));
    }
    let mut flora_g = flora.clone();
    for _ in 0..120 {
        g.step(&w, &mut flora_g);
    }
    let mut cold_differs = false;
    for (k, (tally, restored)) in tallies.iter().enumerate() {
        let (n, twin, cold) = *tally.lock().unwrap();
        assert!(*restored.lock().unwrap(), "body {k}: the step handed its memory back");
        assert!(n > 4, "body {k} decided after the load ({n})");
        assert_eq!(twin, n, "body {k}: the restored brain answers as the uninterrupted one");
        cold_differs |= cold < n;
    }
    assert!(cold_differs, "a cold start answers otherwise: the memory is what continues");
}

/// **Loading** (step 3): a menu lineage's policy made before the intent contract is refused
/// by name through either format's loader, and a two-layer file loads into a two-layer
/// driver whose brains decide.
#[test]
fn a_pre_contract_menu_policy_is_refused_by_name_and_a_two_layer_file_loads() {
    use crate::es::voxel::store::{load_brain_driver, test_centre};
    let founder = Founder::Loftstrider;
    let mut before = founder.manifest();
    before.menu = &cubarium_voxel_fauna::intent::PLAIN;
    let stale = before.digest();
    let dir = std::env::temp_dir().join(format!("two-layer-load-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let mut plain = test_centre(founder, 2);
    plain.digest = stale;
    let p = dir.join("plain.json");
    plain.write(&p).unwrap();
    let err = load_brain_driver(&p, founder).expect_err("refused");
    assert!(err.contains("before the intent contract"), "{err}");

    let net = loftstrider_net(4);
    let mut file = super::file::TwoLayerPolicyFile::new(founder, &net, 1, "test");
    let q = dir.join("two.json");
    file.write(&q).unwrap();
    let d = load_brain_driver(&q, founder).expect("a two-layer file loads");
    assert_eq!(d.name(), "two-layer");
    assert!(load_brain_driver(&q, Founder::Seedporter).is_err(), "another lineage's brain");
    file.digest = stale;
    file.write(&q).unwrap();
    let err = load_brain_driver(&q, founder).expect_err("refused");
    assert!(err.contains("before the intent contract"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- Step 5: teacher streams, forced modes, the shadow teacher ----

use super::clone::{CloneSet, RecordSource};
use super::record::{ShadowTeacher, records_from_tap};

/// Seedporters and a lanternjaw under their heuristics on the plain, a Veilhound beside
/// them; returns the world, its plants, the layer and the menu bodies.
fn teacher_scene() -> (World, Flora, Fauna, Vec<(u64, Founder)>) {
    let w = plain_world(5);
    let mut config = FaunaConfig::default();
    for f in Founder::ALL {
        config.founders[f.index()].active = Activity::Always;
    }
    let mut f = Fauna::new(config);
    let teachers = f.config().teachers;
    let mut ids = Vec::new();
    for (x, founder) in [
        (6, Founder::Seedporter),
        (9, Founder::Seedporter),
        (12, Founder::Lanternjaw),
        (20, Founder::Chorister),
    ] {
        assert!(f.apply(
            &w,
            Command::IntroduceFounder {
                x,
                z: 6,
                founder,
                stores: StartingStores::FULL,
                heading_rad: std::f64::consts::FRAC_PI_2,
            }
        ));
        let id = f.view().animals.last().unwrap().id;
        assert!(f.set_controller(id, cubarium_voxel_fauna::heuristic(founder, teachers)));
        if founder != Founder::Chorister {
            ids.push((id, founder));
        }
    }
    (w, Flora::new(FloraConfig::default()), f, ids)
}

/// The tap becomes the core's records: teacher stretches with a label before the hold,
/// forced stretches as alternative-mode records with none, every held subject among its
/// sample's rows, and the records fit the lineage's shape.
#[test]
fn the_tap_becomes_teacher_and_alternative_mode_records() {
    let (w, mut flora, mut f, ids) = teacher_scene();
    f.start_tap();
    let forced_from = 200u64;
    for t in 0..400u64 {
        if t == forced_from {
            for &(id, _) in &ids {
                f.force_own(id, true);
            }
        }
        f.step(&w, &mut flora);
    }
    let tap = f.take_tap().expect("tap");
    for founder in [Founder::Seedporter, Founder::Lanternjaw] {
        let shape = TwoLayerShape::menu_from_manifest(
            &founder.manifest(),
            MenuHeads { kick: false, call_kinds: 0, rest: false, top_hidden: 8, pointer_width: 6 },
        );
        let records = records_from_tap(&tap, founder, &shape, &|_, tick| tick > forced_from, 1);
        assert!(records.iter().any(|(_, r)| r.source == RecordSource::Teacher), "{}", founder.name());
        assert!(records.iter().any(|(_, r)| r.source == RecordSource::AlternativeMode), "{}", founder.name());
        let menu = founder.manifest().menu;
        for (_, r) in &records {
            for s in &r.samples {
                assert_eq!(s.observation.len(), shape.observation);
                assert_eq!(s.candidates.len(), shape.candidates);
                if let Some(sub) = s.held.subject {
                    assert!(s.candidates.iter().any(|c| c.key == sub.key), "a held subject has its row");
                }
                match r.source {
                    RecordSource::Teacher => assert!(s.proposal.is_some()),
                    _ => {
                        assert!(s.proposal.is_none());
                        let name = menu.name(s.held.intent);
                        assert!(["forage", "ambush", "rest", "feed"].contains(&name), "forced {name}");
                    }
                }
            }
        }
        let only: Vec<_> = records.into_iter().map(|(_, r)| r).collect();
        let arb = cubarium_voxel_fauna::BrainArbitration::default();
        CloneSet::new(&shape, &arb, &only).expect("the records fit the shape");
    }
}

/// The shadow teacher labels the learner's states and never changes its body: a brain
/// with a shadow beside it moves exactly as the same brain alone.
#[test]
fn the_shadow_teacher_does_not_change_the_brain_driven_body() {
    let founder = Founder::Seedporter;
    let shape = TwoLayerShape::menu_from_manifest(
        &founder.manifest(),
        MenuHeads { kick: false, call_kinds: 0, rest: false, top_hidden: 8, pointer_width: 6 },
    );
    let net = Arc::new(TwoLayerNet::new(shape.clone(), initial_center(&shape, 8)).unwrap());
    let arb = cubarium_voxel_fauna::BrainArbitration::default();
    let period = founder.manifest().controller_period_s;
    let (w, mut flora_a, mut a, ids) = teacher_scene();
    let (_, mut flora_b, mut b, _) = teacher_scene();
    let teachers = a.config().teachers;
    let log = Arc::new(Mutex::new(Vec::new()));
    for &(id, f) in &ids {
        if f != founder {
            continue;
        }
        let shadow = ShadowTeacher::new(net.brain(arb, period), cubarium_voxel_fauna::heuristic(founder, teachers), founder, log.clone());
        assert!(a.set_controller(id, Box::new(shadow)));
        assert!(b.set_controller(id, Box::new(net.brain(arb, period))));
    }
    for t in 0..300 {
        a.step(&w, &mut flora_a);
        b.step(&w, &mut flora_b);
        for &(id, _) in &ids {
            assert_eq!(a.view().animal(id).map(|x| x.pose), b.view().animal(id).map(|x| x.pose), "tick {t}");
        }
    }
    let log = log.lock().unwrap();
    assert!(!log.is_empty(), "the shadow labelled the learner's samples");
    assert!(log.iter().all(|s| s.proposal.is_none() && s.actions.len() == shape.actions.len()));
}

/// BRAIN-NEXT step 2: the dry-run teacher beside two-layer seedporter brains labels the
/// states they reach — a top label (the rules' proposal before their hold) on every sample,
/// conditioning from the rules' own hold — and the brain-driven bodies move exactly as they
/// do with it off.
#[test]
fn the_dry_teacher_labels_the_learners_states_with_intents_and_changes_nothing() {
    let founder = Founder::Seedporter;
    let shape = TwoLayerShape::menu_from_manifest(
        &founder.manifest(),
        MenuHeads { kick: false, call_kinds: 0, rest: false, top_hidden: 8, pointer_width: 6 },
    );
    let net = Arc::new(TwoLayerNet::new(shape.clone(), initial_center(&shape, 8)).unwrap());
    let arb = cubarium_voxel_fauna::BrainArbitration::default();
    let period = founder.manifest().controller_period_s;
    let (w, mut flora_a, mut a, ids) = teacher_scene();
    let (_, mut flora_b, mut b, _) = teacher_scene();
    for &(id, f) in &ids {
        if f == founder {
            assert!(a.set_controller(id, Box::new(net.brain(arb, period))));
            assert!(b.set_controller(id, Box::new(net.brain(arb, period))));
        }
    }
    a.dry_teach(founder, Some(cubarium_voxel_fauna::DryScope::Brains));
    a.start_tap();
    b.start_tap();
    for t in 0..300 {
        a.step(&w, &mut flora_a);
        b.step(&w, &mut flora_b);
        for &(id, _) in &ids {
            assert_eq!(a.view().animal(id).map(|x| (x.pose, x.intent)), b.view().animal(id).map(|x| (x.pose, x.intent)), "tick {t}");
        }
    }
    assert!(a.save() == b.save(), "the saved layer is the same");
    let tap = a.take_tap().unwrap();
    let records = super::record::closed_loop_records(&tap, founder, &shape, 1);
    assert!(!records.is_empty(), "the learner's samples were labelled");
    let menu = founder.manifest().menu;
    for (_, r) in &records {
        assert_eq!(r.source, RecordSource::ClosedLoop);
        for s in &r.samples {
            let p = s.proposal.expect("every closed-loop sample carries the teacher's intent");
            assert!(p.intent < menu.named.len());
            if let Some(sub) = s.held.subject {
                assert!(s.candidates.iter().any(|c| c.key == sub.key));
            }
        }
    }
    let only: Vec<_> = records.into_iter().map(|(_, r)| r).collect();
    CloneSet::new(&shape, &arb, &only).expect("the records fit the shape");
}

/// BRAIN-NEXT step 3: a staged scene's teacher records carry its mode, and a two-layer brain
/// runs in it with the dry teacher labelling every sample it reaches.
#[test]
fn a_staged_scene_records_its_mode_and_runs_a_brain_closed_loop() {
    use cubarium_voxel_sim::mode_scenes::{Config, ModeScene};
    let cfg = Config { duration_s: 10.0, ..Config::default() };
    let scene = ModeScene::FleeToTrees;
    let founder = scene.founder();
    let shape = TwoLayerShape::menu_from_manifest(
        &founder.manifest(),
        MenuHeads { kick: false, call_kinds: 0, rest: false, top_hidden: 8, pointer_width: 6 },
    );
    let menu = founder.manifest().menu;
    let k = menu.index(scene.intent()).unwrap();
    let records = super::scenes::record(&cfg, scene, 1, &shape, 1).unwrap();
    assert!(records.iter().all(|r| r.source == RecordSource::Teacher));
    assert!(records.iter().flat_map(|r| &r.samples).any(|s| s.proposal.map(|p| p.intent) == Some(k)), "the scene's mode is labelled");
    let net = Arc::new(TwoLayerNet::new(shape.clone(), initial_center(&shape, 3)).unwrap());
    let driver = crate::es::voxel::EpisodeDriver::two_layer(net, cubarium_voxel_fauna::BrainArbitration::default(), founder);
    let closed = super::scenes::closed_loop(&cfg, scene, 1, &driver, &shape, 1).unwrap();
    assert!(!closed.is_empty());
    assert!(closed.iter().flat_map(|r| &r.samples).all(|s| s.proposal.is_some()));
}
