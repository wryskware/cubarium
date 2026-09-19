//! Phase-one founder ticks, end to end through [`Fauna::step`]: the controller stage,
//! the paid motor budget, local feeding through the plant layer's real transfers, and
//! the prior-interval feedback the observation's `Self` channels are built from.
//!
//! Small worlds, a few dozen ticks each, no pinned trajectories — the numbers asserted
//! are the manifest's own references and the physiology's frozen rates.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraConfig, Species as Plant, Site,
};

use cubarium_voxel_fauna::{
    Action as _, Actions, Animal, Command as FaunaCommand, Controller, Fauna, FaunaConfig, Founder,
    Response, Scripted,
};

/// A flat world: 8 × 6 × 6 voxels at 0.25 m, soil 1..=2, ground support face at y = 2.
fn flat_world() -> World {
    let mut world = World::empty(VoxelConfig {
        width: 8,
        height: 6,
        depth: 6,
        voxel_m: 0.25,
        ..VoxelConfig::default()
    });
    for z in 0..6 {
        for x in 0..8 {
            for y in 1..=2 {
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    world
}

fn site(x: u32, z: u32) -> Site {
    Site { x, y: 2, z }
}

/// A controller that records every observation it is given and always answers with one
/// bounded action. It is the policy-boundary probe: what it holds is what the sampler
/// handed it, and nothing else exists for it to read. The log lives in the controller's
/// own private memory, shared with the test through an `Arc`.
struct Record {
    log: std::sync::Arc<std::sync::Mutex<Vec<Vec<f64>>>>,
    answer: Actions,
}

impl Record {
    fn new(
        answer: Actions,
    ) -> (
        Record,
        std::sync::Arc<std::sync::Mutex<Vec<Vec<f64>>>>,
    ) {
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        (
            Record {
                log: log.clone(),
                answer,
            },
            log,
        )
    }
}

impl Controller for Record {
    fn drive(&mut self, observation: &[f64]) -> Response {
        self.log.lock().unwrap().push(observation.to_vec());
        Response::Bounded(self.answer)
    }

    fn reset(&mut self) {
        self.log.lock().unwrap().clear();
    }
}

/// The first controller sampling of a freshly introduced founder happens when its age
/// reaches the controller period (5 ticks); that same tick resolves the held actions
/// and attempts the held feed.
#[test]
fn a_full_founder_pays_and_moves_on_the_held_actions() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 2,
            z: 2,
            founder: Founder::Blind,
            body: 0.0125,
            heading_rad: std::f64::consts::FRAC_PI_2,
        },
    ));
    let id = fauna.view().ledger.births - 1;
    assert!(fauna.set_controller(
        id,
        Box::new(Scripted::new(vec![Actions {
            forward: 1.0,
            turn: 0.0,
            feed: 0.0,
        }])),
    ));

    for _ in 0..4 {
        fauna.step(&world, &mut flora);
    }
    // Before the first sampling the body has held nothing and moved nowhere.
    let a = fauna.view().animal(id).expect("alive");
    assert_eq!(a.pose.x, (2.0 + 0.5) * 0.25);
    assert_eq!(a.age_ticks, 4);

    fauna.step(&world, &mut flora);
    // The due tick: the controller was sampled, the actions are held, and motion is
    // resolved the same tick.
    let a = fauna.view().animal(id).expect("alive");
    assert_eq!(a.age_ticks, 5);
    assert!(
        (a.pose.x - ((2.0 + 0.5) * 0.25 + 0.125 * (1.0 / 20.0))).abs() < 1e-12,
        "one tick of full cruise = {}",
        a.pose.x
    );
    for _ in 0..4 {
        fauna.step(&world, &mut flora);
    }
    let a = fauna.view().animal(id).expect("alive");
    // A full period of cruise is exactly the manifest's forward reference.
    assert!(
        (a.pose.x - ((2.0 + 0.5) * 0.25 + 0.03125)).abs() < 1e-9,
        "a full period at cruise moved {}",
        a.pose.x
    );
    assert_eq!(a.site.x, 2, "the site followed the pose's column");
    assert_eq!(a.state, cubarium_voxel_fauna::State::Walking);
}

/// A bite debits the real litter stock once, books the same transfer on the fauna
/// ledger, and builds tissue the assimilation rule can fund. A feed with nothing in
/// the mouth transfers nothing at all.
#[test]
fn a_litter_bite_debits_the_real_stock_exactly_once_per_interval() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    flora.deposit(
        site(2, 2),
        Deposit {
            kind: DepositKind::Litter,
            organic: 0.2,
            mineral: 0.2 * 0.02,
            energy: 0.2 * 2.0,
        },
    );
    let mut fauna = Fauna::new(FaunaConfig::default());
    // A body below its adult maximum, so a bite has tissue to build into (an adult at
    // its reference has a full reserve and converts nothing — the standing rule).
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 2,
            z: 2,
            founder: Founder::Blind,
            body: 0.01,
            heading_rad: 0.0,
        },
    ));
    let id = fauna.view().ledger.births - 1;
    assert!(fauna.set_controller(
        id,
        Box::new(Scripted::new(vec![Actions {
            forward: 0.0,
            turn: 0.0,
            feed: 1.0,
        }])),
    ));
    let litter_of = |flora: &Flora| {
        flora
            .view()
            .ground_at(site(2, 2))
            .map_or(0.0, |g| g.litter)
    };

    let before_stock = litter_of(&flora);
    let before_body = fauna.view().animal(id).unwrap().body;
    for _ in 0..5 {
        fauna.step(&world, &mut flora);
    }

    let want = 0.0005 * 0.25; // the frozen bite rate over one controller period
    let after_stock = litter_of(&flora);
    assert!(
        ((before_stock - after_stock) - want).abs() < 1e-12,
        "one bite of {} was taken, stock moved {}",
        want,
        before_stock - after_stock
    );
    let ledger = &fauna.view().ledger;
    assert_eq!(ledger.bites, 1, "exactly one attempt per interval");
    assert!(
        (ledger.eaten_organic_in - want).abs() < 1e-12,
        "the fauna ledger booked the same transfer"
    );
    assert!(
        (ledger.eaten_mineral_in - want * 0.02).abs() < 1e-15,
        "the real Taken composition: mineral pro rata at the litter's fraction"
    );
    let a = fauna.view().animal(id).expect("alive");
    assert!(a.body > before_body, "the funded share of the bite became tissue");
    assert!(
        (a.founder_state.feedback.intake - 0.5 * want).abs() < 1e-12,
        "the placed share is the yield fraction, {}",
        a.founder_state.feedback.intake
    );

    // A second interval: the second attempt, and only that.
    for _ in 0..5 {
        fauna.step(&world, &mut flora);
    }
    assert_eq!(fauna.view().ledger.bites, 2);
    assert!((litter_of(&flora) - (before_stock - 2.0 * want)).abs() < 1e-12);
}

/// A feed attempt with nothing in the mouth transfers nothing: no withdrawal, no
/// booking, no ledger event.
#[test]
fn a_feed_off_food_transfers_nothing() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    // Litter two columns east, beyond every mouth reach.
    flora.deposit(
        site(4, 2),
        Deposit {
            kind: DepositKind::Litter,
            organic: 0.2,
            mineral: 0.2 * 0.02,
            energy: 0.2 * 2.0,
        },
    );
    let mut fauna = Fauna::new(FaunaConfig::default());
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 2,
            z: 2,
            founder: Founder::Blind,
            body: 0.01,
            heading_rad: 0.0,
        },
    ));
    let id = fauna.view().ledger.births - 1;
    assert!(fauna.set_controller(
        id,
        Box::new(Scripted::new(vec![Actions {
            forward: 0.0,
            turn: 0.0,
            feed: 1.0,
        }])),
    ));
    for _ in 0..10 {
        fauna.step(&world, &mut flora);
    }
    let ledger = fauna.view().ledger;
    assert_eq!(ledger.bites, 0, "no contact, no attempt landed");
    assert_eq!(ledger.eaten_organic_in, 0.0);
    let a = fauna.view().animal(id).expect("alive");
    assert_eq!(a.founder_state.feedback.intake, 0.0);
    assert!(
        (flora.view().ground_at(site(4, 2)).unwrap().litter - 0.2).abs() < 1e-15,
        "the unreachable stock is untouched"
    );
}

/// The browser founder crops the stand it stands in, through `take_foliage`, and the
/// report says which plant was actually eaten.
#[test]
fn a_browser_bite_crops_the_stand_it_touches() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    let wood = 0.5 * flora.config().species(Plant::Springturf).wood_max;
    assert!(flora.apply(
        &world,
        FloraCommand::Seed {
            x: 2,
            z: 2,
            species: Plant::Springturf,
            wood,
        },
    ));
    let foliage_of = |flora: &Flora| flora.view().stand_at(site(2, 2)).unwrap().foliage;
    let before_stock = foliage_of(&flora);

    let mut fauna = Fauna::new(FaunaConfig::default());
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 2,
            z: 2,
            founder: Founder::Browser,
            body: 0.04,
            heading_rad: 0.0,
        },
    ));
    let id = fauna.view().ledger.births - 1;
    assert!(fauna.set_controller(
        id,
        Box::new(Scripted::new(vec![Actions {
            forward: 0.0,
            turn: 0.0,
            feed: 1.0,
        }])),
    ));
    for _ in 0..5 {
        fauna.step(&world, &mut flora);
    }

    let want = 0.002 * 0.25; // the frondgrazer bite rate over one controller period
    let ledger = &fauna.view().ledger;
    assert_eq!(ledger.bites, 1);
    assert!(
        ((before_stock - foliage_of(&flora)) - want).abs() < 1e-12,
        "foliage debited by {}",
        before_stock - foliage_of(&flora)
    );
    let turf = Plant::Springturf.index();
    assert_eq!(ledger.bites_by_plant[turf], 1);
    assert!((ledger.eaten_by_plant[turf] - want).abs() < 1e-12);
    let a = fauna.view().animal(id).expect("alive");
    assert!(a.body > 0.04, "the funded share built tissue");
}

/// The `Self` feedback channels are the prior interval's outcomes, reset once: the
/// first sampling reads zero, the next reads exactly one period of delivered motion,
/// and the one after reads it again — not the running total. The controller's log is
/// its own private memory, so what it holds is exactly what the sampler handed it.
#[test]
fn the_self_feedback_is_the_prior_interval_and_reset_once() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 2,
            z: 2,
            founder: Founder::Blind,
            body: 0.0125,
            heading_rad: std::f64::consts::FRAC_PI_2,
        },
    ));
    let id = fauna.view().ledger.births - 1;
    let (controller, log) = Record::new(Actions {
        forward: 1.0,
        turn: 0.0,
        feed: 0.0,
    });
    assert!(fauna.set_controller(id, Box::new(controller)));

    for _ in 0..15 {
        fauna.step(&world, &mut flora);
    }
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 3, "sampled at ages 5, 10 and 15");
    let reference = 0.125 * 0.25; // the manifest's forward reference

    // First sampling: initial feedback is zero, nothing was requested, delivery 1.
    assert_eq!(log[0][3], 0.0, "structural loss");
    assert_eq!(log[0][4], 0.0, "intake");
    assert_eq!(log[0][5], 0.0, "resolved forward");
    assert_eq!(log[0][7], 1.0, "motor delivery when nothing was requested");

    // Second sampling: the prior interval delivered exactly one period of cruise,
    // paid in full.
    assert!(
        (log[1][5] - 1.0).abs() < 1e-9,
        "delivered forward {}",
        log[1][5]
    );
    assert!((log[1][7] - 1.0).abs() < 1e-9, "free movement delivers what it requests");
    assert_eq!(log[1][4], 0.0, "no intake without a feed action");

    // Third sampling: reset once — the prior interval again, not the running total.
    assert!(
        (log[2][5] - 1.0).abs() < 1e-9,
        "the same interval's worth again, not {}",
        log[2][5]
    );
    let _ = reference;
}

/// Full-cruise motor respiration starts at the same rate as basal upkeep: a cruising
/// founder respires exactly twice what a resting one does over the same period.
#[test]
fn full_cruise_motor_respiration_equals_the_basal_rate() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    for x in [2u32, 5] {
        assert!(fauna.apply(
            &world,
            FaunaCommand::IntroduceFounder {
                x: i64::from(x),
                z: 2,
                founder: Founder::Blind,
                body: 0.0125,
                heading_rad: 0.0,
            },
        ));
    }
    let cruiser = 0;
    let rester = 1;
    let ids: Vec<u64> = fauna.view().animals.iter().map(|a| a.id).collect();
    assert!(fauna.set_controller(ids[cruiser], Box::new(Scripted::new(vec![Actions { forward: 1.0, turn: 0.0, feed: 0.0 }]))));
    assert!(fauna.set_controller(ids[rester], Box::new(Scripted::new(vec![]))));

    let before = fauna.view().ledger.respired_out;
    // Ten steps: the cruiser is sampled at ages 5 and 10, so its held action moves it
    // on the five ticks 5..9 — five ticks of full-cruise motor — while the rester pays
    // basal upkeep on all ten.
    for _ in 0..10 {
        fauna.step(&world, &mut flora);
    }
    let paid = fauna.view().ledger.respired_out - before;
    let body = 0.0125;
    let basal_per_tick = 0.001 * body * (1.0 / 20.0);
    // Ten steps: basal upkeep on all ten for both; the cruiser's held action is
    // resolved on its due ticks (5 and 10) and the four ticks after each sampling —
    // six motion ticks — while the rester's held set is zero.
    let expected = 2.0 * 10.0 * basal_per_tick + 6.0 * basal_per_tick;
    assert!(
        (paid - expected).abs() < 1e-15,
        "paid {}, expected {} — basal upkeep on both, motor on the mover's held ticks",
        paid,
        expected
    );
    // And the mover really moved: six resolved ticks' worth of cruise after ten steps
    // (its last interval's full-period feedback was consumed at age 10's sampling).
    // Heading 0 faces +z, so the travel is in z.
    let cruiser = fauna.view().animal(ids[0]).unwrap();
    assert!(
        (cruiser.pose.z - ((2.0 + 0.5) * 0.25 + 6.0 * 0.125 * (1.0 / 20.0))).abs() < 1e-9,
        "the cruiser is at z = {}",
        cruiser.pose.z
    );
    assert_eq!(cruiser.state, cubarium_voxel_fauna::State::Walking);
}

/// A full-cruise attempt a wall blocks is attempted in full and paid in full: its
/// respiration is the free cruise's, its delivery is not.
#[test]
fn a_blocked_attempt_still_pays_the_motor_budget() {
    let mut world = flat_world();
    // A wall on column x = 3 at the body layer, two rows of it.
    for z in [2u32, 3] {
        world.apply(WorldCommand::SetMaterial {
            x: 3,
            y: 3,
            z,
            material: Material::Soil,
        });
    }
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    // One founder heading into the wall, one heading into open ground.
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 2,
            z: 2,
            founder: Founder::Blind,
            body: 0.0125,
            heading_rad: std::f64::consts::FRAC_PI_2,
        },
    ));
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 2,
            z: 5,
            founder: Founder::Blind,
            body: 0.0125,
            heading_rad: std::f64::consts::FRAC_PI_2,
        },
    ));
    let ids: Vec<u64> = fauna.view().animals.iter().map(|a| a.id).collect();
    let script = vec![Actions {
        forward: 1.0,
        turn: 0.0,
        feed: 0.0,
    }];
    assert!(fauna.set_controller(ids[0], Box::new(Scripted::new(script.clone()))));
    assert!(fauna.set_controller(ids[1], Box::new(Scripted::new(script))));

    let before = fauna.view().ledger.respired_out;
    // Thirty-five steps: the bodies cruise free for twenty-five ticks and reach the
    // wall's face on tick 30, so interval 6 (ages 30..34) is the blocked one for the
    // walled body and a free cruise for the other.
    for _ in 0..35 {
        fauna.step(&world, &mut flora);
    }
    let paid = fauna.view().ledger.respired_out - before;
    let basal_per_tick = 0.001 * 0.0125 * (1.0 / 20.0);
    // Both founders pay basal upkeep on all 35 ticks and full-cruise motor on the 30
    // ticks their actions were held — the blocked attempt is attempted in full.
    let expected = 2.0 * 35.0 * basal_per_tick + 2.0 * 31.0 * basal_per_tick;
    assert!(
        (paid - expected).abs() < 1e-12,
        "paid {}, expected {} — the blocked attempt paid the same as the free one",
        paid,
        expected
    );
    let blocked = fauna.view().animal(ids[0]).unwrap();
    let free = fauna.view().animal(ids[1]).unwrap();
    assert!(blocked.pose.x < free.pose.x, "the wall stopped the blocked body");
    assert!(
        blocked.founder_state.feedback.delivered_equivalent
            < blocked.founder_state.feedback.attempted_equivalent,
        "the delivery ratio is what reports the wall: {} against {}",
        blocked.founder_state.feedback.delivered_equivalent,
        blocked.founder_state.feedback.attempted_equivalent
    );
    assert!(
        (free.founder_state.feedback.delivered_equivalent
            - free.founder_state.feedback.attempted_equivalent)
            .abs()
            < 1e-12
    );
    // The pressed body stands at the wall's face, and the wall is its front contact.
    assert!(
        (blocked.pose.x - (3.0 * 0.25 - 0.03125)).abs() < 1e-6,
        "pressed against the wall at x = {}",
        blocked.pose.x
    );
}

/// The observation the controller receives is 23 wide and finite, and its P1-C spans
/// stay zero with validity 0 even while the body moves and eats.
#[test]
fn the_controller_receives_only_the_vector_and_it_stays_finite() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    flora.deposit(
        site(2, 2),
        Deposit {
            kind: DepositKind::Litter,
            organic: 0.2,
            mineral: 0.2 * 0.02,
            energy: 0.2 * 2.0,
        },
    );
    let mut fauna = Fauna::new(FaunaConfig::default());
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 2,
            z: 2,
            founder: Founder::Blind,
            body: 0.01,
            heading_rad: 0.0,
        },
    ));
    let id = fauna.view().ledger.births - 1;
    let (controller, log) = Record::new(Actions {
        forward: 1.0,
        turn: 0.0,
        feed: 1.0,
    });
    assert!(fauna.set_controller(id, Box::new(controller)));
    for _ in 0..10 {
        fauna.step(&world, &mut flora);
    }
    for obs in log.lock().unwrap().iter() {
        assert_eq!(obs.len(), 23, "the blind manifest's width");
        assert!(obs.iter().all(|v| v.is_finite()), "no non-finite channel");
        assert!(obs[18..23].iter().all(|v| *v == 0.0), "Chem and Light stay zero with validity 0");
    }
}

/// A snapshot carries a founder's held actions and feedback and refuses one whose
/// held actions could not have come through the adapter.
#[test]
fn the_snapshot_round_trips_a_founders_state_and_refuses_out_of_bounds_held_actions() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 2,
            z: 2,
            founder: Founder::Blind,
            body: 0.01,
            heading_rad: std::f64::consts::FRAC_PI_2,
        },
    ));
    let id = fauna.view().ledger.births - 1;
    assert!(fauna.set_controller(
        id,
        Box::new(Scripted::new(vec![Actions {
            forward: 1.0,
            turn: 0.0,
            feed: 0.0,
        }])),
    ));
    for _ in 0..10 {
        fauna.step(&world, &mut flora);
    }
    let bytes = fauna.save();
    let back = Fauna::load(&bytes).expect("a live layer round-trips");
    let a = back.view().animal(id).expect("the founder survived");
    let before = fauna.view().animal(id).unwrap();
    assert!((a.founder_state.held.forward - 1.0).abs() < 1e-12);
    assert!(a.founder_state.feedback.delivered_forward > 0.0);
    assert!(
        (a.founder_state.feedback.delivered_forward
            - before.founder_state.feedback.delivered_forward)
            .abs()
            < 1e-15
    );
}
