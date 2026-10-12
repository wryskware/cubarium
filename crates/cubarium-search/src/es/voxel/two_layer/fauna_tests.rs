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
