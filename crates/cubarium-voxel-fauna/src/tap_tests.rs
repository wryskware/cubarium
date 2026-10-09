//! The executed-action tap (contract v3, P0): what it records, per body, and that recording
//! changes nothing. Mechanism only; the reflex cases live beside their fixtures
//! (`roster_mechanics_tests`' shallows, `flight_tests`' vine nest) and use [`Spy`].

#[path = "predation_fixture.rs"]
#[allow(dead_code)]
mod fixture;

use std::sync::{Arc, Mutex};

use crate::{Actions, Controller, Fauna, Founder, Manifest, Response, Scripted, Seek, StartingStores, Steer};
use cubarium_voxel_flora::{Flora, FloraConfig};
use fixture::*;

/// What a [`Spy`] saw its controller answer at each sample: the adapted actions and the
/// lure and lift its seams held.
pub(crate) type Answers = Arc<Mutex<Vec<(Actions, f64, f64)>>>;

/// A transparent wrapper that keeps every answer of the controller it wraps — through the
/// shared adapter, before any reflex — and passes every opt-in through, so the reflexes
/// apply to the wrapped body exactly as they would without it.
pub(crate) struct Spy {
    inner: Box<dyn Controller>,
    manifest: Manifest,
    answers: Answers,
}

impl Spy {
    pub(crate) fn new(inner: Box<dyn Controller>, founder: Founder) -> (Spy, Answers) {
        let answers = Answers::default();
        (Spy { inner, manifest: founder.manifest(), answers: Arc::clone(&answers) }, answers)
    }
}

impl Controller for Spy {
    fn drive(&mut self, observation: &[f64]) -> Response {
        let r = self.inner.drive(observation);
        let a = crate::resolve_actions(r, &self.manifest);
        self.answers.lock().expect("answers").push((a, self.inner.lure(), self.inner.lift()));
        r
    }
    fn reset(&mut self) {
        self.inner.reset();
    }
    fn taste_only(&self) -> bool {
        self.inner.taste_only()
    }
    fn lure(&self) -> f64 {
        self.inner.lure()
    }
    fn lift(&self) -> f64 {
        self.inner.lift()
    }
    fn flees(&self) -> bool {
        self.inner.flees()
    }
    fn pack_tactic(&self) -> bool {
        self.inner.pack_tactic()
    }
    fn at_carcass(&self) -> bool {
        self.inner.at_carcass()
    }
    fn hunt_patrol(&self) -> bool {
        self.inner.hunt_patrol()
    }
    fn rests_when_fed(&self) -> bool {
        self.inner.rests_when_fed()
    }
    fn seek(&self) -> Seek {
        self.inner.seek()
    }
    fn reads_tail(&self) -> bool {
        self.inner.reads_tail()
    }
}

/// A scripted walker on the plain, at `(x, z)` facing `heading`, holding `actions`.
fn scripted(fauna: &mut Fauna, world: &cubarium_voxel::World, x: f64, z: f64, heading: f64, actions: Actions) -> u64 {
    let id = introduce(fauna, world, Founder::Blind, StartingStores { body: 1.0, reserve: 0.5 });
    place(fauna, id, x, z, heading);
    assert!(fauna.set_controller(id, Box::new(Scripted::new(vec![actions]))));
    id
}

/// **A body with no override records its own response for both labels**: a scripted body
/// opts into no reflex, so every sample's layer is its own and what it held is what it
/// answered; the observation is its manifest's width.
#[test]
fn a_body_with_no_override_records_its_own_answer_as_both_labels() {
    let world = clock_off(3);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(hunting_config());
    let ask = Actions { forward: 0.6, turn: -0.3, feed: 1.0 };
    let id = scripted(&mut fauna, &world, 3.0, 1.5, EAST, ask);
    fauna.start_tap();
    for _ in 0..30 {
        fauna.step(&world, &mut flora);
    }
    let tap = fauna.take_tap().expect("the tap");
    let s = tap.stream(id).expect("its stream");
    assert!(s.len() >= 5, "one sample per controller period: {}", s.len());
    for x in s {
        assert_eq!(x.steer, Steer::Heuristic, "its own layer");
        assert_eq!(x.own, x.executed, "both labels are its own answer");
        assert_eq!(x.executed.actions, ask);
        assert_eq!(x.observation.len(), Founder::Blind.manifest().inputs());
    }
    assert!(fauna.tap().is_none(), "taking the tap stops it");
}

/// **Streams stay per body**: two bodies on different scripts each get their own stream,
/// in tick order, holding only their own answers; a body sampled at its own cadence.
#[test]
fn every_body_keeps_its_own_stream() {
    let world = clock_off(4);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(hunting_config());
    let a = Actions { forward: 0.5, turn: 0.4, feed: 0.0 };
    let b = Actions { forward: 0.0, turn: 0.0, feed: 1.0 };
    let ia = scripted(&mut fauna, &world, 2.0, 1.0, EAST, a);
    let ib = scripted(&mut fauna, &world, 8.0, 2.0, WEST, b);
    fauna.start_tap();
    for _ in 0..40 {
        fauna.step(&world, &mut flora);
    }
    let tap = fauna.take_tap().expect("the tap");
    let period = Founder::Blind.manifest().cadence_ticks();
    for (id, want) in [(ia, a), (ib, b)] {
        let s = tap.stream(id).expect("a stream");
        assert!(s.iter().all(|x| x.own.actions == want), "only its own answers");
        assert!(s.windows(2).all(|w| w[1].tick - w[0].tick == period), "one sample per period, in order");
    }
    assert_eq!(tap.streams().count(), 2);
    assert!(tap.streams().all(|(id, f, _)| (id == ia || id == ib) && f == Founder::Blind));
}

/// **Recording changes nothing**: a tapped world and an untapped twin step to the same
/// bodies, poses and stores, heuristics and reflexes included (every lineage on its own
/// heuristic, a hunter among them).
#[test]
fn a_tapped_world_steps_exactly_as_an_untapped_one() {
    let world = clock_off(5);
    let build = || {
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(hunting_config());
        for (k, f) in Founder::ALL.into_iter().enumerate() {
            let id = introduce(&mut fauna, &world, f, StartingStores { body: 1.0, reserve: 0.3 });
            place(&mut fauna, id, 0.5 + 1.25 * k as f64, 1.0 + 0.2 * k as f64, 0.7 * k as f64);
        }
        for _ in 0..5 {
            fauna.step(&world, &mut flora);
        }
        (fauna, flora)
    };
    let (mut plain, mut pf) = build();
    let (mut tapped, mut tf) = build();
    tapped.start_tap();
    for t in 0..200 {
        plain.step(&world, &mut pf);
        tapped.step(&world, &mut tf);
        let (p, q) = (plain.view().animals, tapped.view().animals);
        assert_eq!(p.len(), q.len(), "tick {t}");
        for (x, y) in p.iter().zip(q) {
            assert_eq!(x, y, "tick {t}: the same body");
            assert_eq!(plain.steer_of(x.id), tapped.steer_of(y.id), "tick {t}");
        }
    }
    let samples: usize = tapped.tap().expect("running").streams().map(|(_, _, s)| s.len()).sum();
    assert!(samples > 0, "the tap recorded");
}
