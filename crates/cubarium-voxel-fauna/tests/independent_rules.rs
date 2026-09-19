//! **Independent** checks of the phase-one model rules (package P2-T).
//!
//! Written from `design/voxel-senses-phase1-plan.md` ("Frozen arena contract",
//! "Phase-one body and action contract", "Exact starting manifests", "Initial cue field
//! settings") and `design/voxel-senses-phase1-tests.md` §1, by an author who did not write
//! the code under test and did not read the implementing worker's own tests. Each test
//! names the rule it asserts, so a failure says which sentence of the plan the source
//! stopped honouring.
//!
//! Everything here drives the **public** surface only — `Fauna::apply`,
//! `Fauna::set_controller`, `Fauna::step_with_senses`, `FaunaView` and the manifests — so
//! nothing depends on a private helper's shape. Every test stays inside the working
//! policy's budget: the longest run is 120 ticks, there are no golden hashes, and every
//! numeric comparison carries a tolerance.

use std::sync::{Arc, Mutex};

use cubarium_voxel::{Command as WorldCommand, Config, Material, World};
use cubarium_voxel_fauna::{
    Actions, Command, Controller, Fauna, FaunaConfig, Founder, Manifest, Response, Scripted,
    Senses, StartingStores,
};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraConfig, Site, Species as Plant,
};

// ------------------------------------------------------------------ fixtures

/// The support layer of every fixture world: soil `1..=GROUND_Y`, air above.
const GROUND_Y: u32 = 2;
/// The voxel edge the arena contract fixes.
const VOXEL_M: f64 = 0.25;
/// Heading east, `+x`: the pose's forward vector is `(sin h, cos h)`.
const EAST: f64 = std::f64::consts::FRAC_PI_2;

/// A flat strip: bedrock at `y = 0`, soil to [`GROUND_Y`], air above. Support faces are
/// `(x, GROUND_Y, z)` for every column.
fn flat(width: u32, depth: u32) -> World {
    let mut world = World::empty(Config {
        width,
        height: 10,
        depth,
        voxel_m: VOXEL_M,
        seed: 1,
        ..Config::default()
    });
    for z in 0..depth {
        for x in 0..width as i64 {
            for y in 1..=GROUND_Y {
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

/// A solid column of rock from `y0` to `y1` inclusive across the whole `z` extent of
/// column `x`: a one-voxel-thick wall on the strip.
fn wall(world: &mut World, x: i64, y0: u32, y1: u32) {
    let depth = world.config().depth;
    for z in 0..depth {
        for y in y0..=y1 {
            world.apply(WorldCommand::SetMaterial {
                x,
                y,
                z,
                material: Material::Rock,
            });
        }
    }
}

fn site(x: u32, z: u32) -> Site {
    Site { x, y: GROUND_Y, z }
}

/// Litter on one ground site at the arena's own composition: a 0.02 mineral fraction and
/// a retained energy density of 2.0 (`cubarium-voxel-sim`'s `LITTER_*` constants, which is
/// the composition the blind founder's digestion was set from).
fn litter(flora: &mut Flora, at: Site, organic: f64) {
    assert!(
        flora.deposit(
            at,
            Deposit {
                kind: DepositKind::Litter,
                organic,
                mineral: 0.02 * organic,
                energy: 2.0 * organic,
            },
        ),
        "the plant layer refused the fixture's litter at {at:?}"
    );
}

/// A founder body the plan's own way: half-way between `body_min` and `body_max`, so both
/// the body and the reserve have room for a bite and nothing is clipped by a full store.
fn founder_body(founder: Founder) -> f64 {
    let core = cubarium_voxel_fauna::FounderPhysiology::frozen(founder).core;
    0.5 * (core.body_min + core.body_max)
}

/// The same body, as the starting-stores fractions `IntroduceFounder` now takes: that
/// body's share of `body_max`, with the full reserve these fixtures always had.
fn founder_stores(founder: Founder) -> StartingStores {
    let core = cubarium_voxel_fauna::FounderPhysiology::frozen(founder).core;
    StartingStores {
        body: founder_body(founder) / core.body_max,
        reserve: 1.0,
    }
}

/// A fauna layer with births disabled (the frozen arena contract) holding one founder at
/// column `(x, z)` with heading `heading_rad`. Returns the layer and the body's id.
fn one_founder(world: &World, founder: Founder, x: i64, z: u32, heading_rad: f64) -> (Fauna, u64) {
    let mut fauna = Fauna::new(FaunaConfig::default());
    fauna.set_births_enabled(false);
    let id = fauna.view().ledger.births;
    assert!(
        fauna.apply(
            world,
            Command::IntroduceFounder {
                x,
                z,
                founder,
                stores: founder_stores(founder),
                heading_rad,
            },
        ),
        "the fixture's founder was refused at column ({x}, {z})"
    );
    (fauna, id)
}

/// The input slot of one named channel of one named module, looked up from the manifest
/// rather than hard-coded, so a manifest reorder moves these tests with it.
fn slot(manifest: &Manifest, module: &str, channel: &str) -> usize {
    let m = manifest
        .modules
        .iter()
        .find(|m| m.name == module)
        .unwrap_or_else(|| panic!("this manifest has no {module} module"));
    let k = m
        .channels
        .iter()
        .position(|c| *c == channel)
        .unwrap_or_else(|| panic!("{module} has no {channel} channel"));
    m.offset + k
}

/// One sample and the support column the body stood in when it was taken.
type SampleLog = Arc<Mutex<Vec<(Vec<f64>, i64)>>>;

/// A controller that records every observation it is handed and answers with one fixed
/// bounded action. The recording is the only way an integration test can see the
/// observation, and it is also the policy boundary under test: this type receives a
/// `&[f64]` and nothing else.
#[derive(Clone)]
struct Recorder {
    log: Arc<Mutex<Vec<Vec<f64>>>>,
    action: Actions,
}

impl Recorder {
    fn new(action: Actions) -> (Recorder, Arc<Mutex<Vec<Vec<f64>>>>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        (
            Recorder {
                log: Arc::clone(&log),
                action,
            },
            log,
        )
    }
}

impl Controller for Recorder {
    fn drive(&mut self, observation: &[f64]) -> Response {
        self.log
            .lock()
            .expect("the recorder's log")
            .push(observation.to_vec());
        Response::Bounded(self.action)
    }

    fn reset(&mut self) {}
}

/// A recorder that also plays a script, one entry per controller sampling, holding the
/// last entry once the script runs out.
struct ScriptedRecorder {
    script: Vec<Actions>,
    next: usize,
    log: Arc<Mutex<Vec<Vec<f64>>>>,
}

impl ScriptedRecorder {
    fn new(script: Vec<Actions>) -> (ScriptedRecorder, Arc<Mutex<Vec<Vec<f64>>>>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        (
            ScriptedRecorder {
                script,
                next: 0,
                log: Arc::clone(&log),
            },
            log,
        )
    }
}

impl Controller for ScriptedRecorder {
    fn drive(&mut self, observation: &[f64]) -> Response {
        self.log
            .lock()
            .expect("the recorder's log")
            .push(observation.to_vec());
        let action = self.script[self.next.min(self.script.len() - 1)];
        self.next += 1;
        Response::Bounded(action)
    }

    fn reset(&mut self) {
        self.next = 0;
    }
}

/// Step the fauna layer `ticks` times against a settled field.
fn run(fauna: &mut Fauna, world: &World, flora: &mut Flora, senses: &mut Senses, ticks: u64) {
    for _ in 0..ticks {
        fauna.step_with_senses(world, flora, 1, senses);
    }
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * (1.0f64).max(a.abs().max(b.abs()))
}

// --------------------------------------------------------------- local motion

/// Plan, "Phase-one body and action contract": *"Attempted movement against a wall still
/// costs effort; collision cannot grant free searching."* The motor charge is scaled by
/// the **requested** equivalent displacement, so a body pressed against a wall must pay
/// exactly what the same request pays down an open lane.
///
/// A failure means a policy can buy free search by driving into terrain — the cheapest
/// possible degenerate strategy, and one the ES score would find immediately.
#[test]
fn a_refused_wall_step_pays_the_requested_equivalent_displacement() {
    // 41 ticks: the first eight controller intervals, of which the last several are spent
    // fully pressed against the wall in the blocked arm.
    let ticks = 41;
    let drive = Actions {
        forward: 1.0,
        turn: 0.0,
        feed: 0.0,
    };

    let outcome = |blocked: bool| {
        let mut world = flat(16, 3);
        if blocked {
            // One voxel of rock at the body layer two columns ahead: both a disc
            // obstruction and a column with no support face.
            wall(&mut world, 5, GROUND_Y + 1, GROUND_Y + 1);
        }
        let mut flora = Flora::new(FloraConfig::default());
        let (mut fauna, id) = one_founder(&world, Founder::Blind, 4, 1, EAST);
        let (recorder, log) = Recorder::new(drive);
        assert!(fauna.set_controller(id, Box::new(recorder)));
        let mut senses = Senses::new();
        run(&mut fauna, &world, &mut flora, &mut senses, ticks);
        let view = fauna.view();
        let a = view.animal(id).expect("the founder survived 41 ticks");
        let last = log
            .lock()
            .expect("the log")
            .last()
            .cloned()
            .expect("samples");
        (
            view.organic(),
            view.ledger.respired_motor_out,
            a.pose.x,
            last,
        )
    };

    let (open_organic, open_motor, open_x, open_last) = outcome(false);
    let (walled_organic, walled_motor, walled_x, walled_last) = outcome(true);

    // The wall really refused the sweep.
    assert!(
        walled_x < open_x - 0.05,
        "the wall did not constrain the body: open x {open_x}, walled x {walled_x}"
    );
    // The face of the wall is at x = 5 * 0.25; the disc's radius is body_width / 2.
    let face = 5.0 * VOXEL_M - Manifest::blind().body_width_m / 2.0;
    assert!(
        walled_x <= face + 1e-9,
        "the body ended {walled_x} m, past the wall face at {face} m"
    );

    // ... and it was paid for in full all the same.
    assert!(
        close(walled_motor, open_motor, 1e-12),
        "a blocked attempt paid {walled_motor} of motor respiration where the same request \
         paid {open_motor} down an open lane"
    );
    assert!(
        close(walled_organic, open_organic, 1e-12),
        "the blocked body kept {walled_organic} of organic matter against the open body's \
         {open_organic}: collision granted free searching"
    );
    assert!(open_motor > 0.0, "the open lane paid nothing at all");

    // `Self.motor_delivery` is `delivered / requested`, so the two arms are separable by
    // the observation as well as by the ledger.
    let delivery = slot(&Manifest::blind(), "Self", "motor_delivery");
    assert!(
        open_last[delivery] > 0.999,
        "an unobstructed cruise reported delivery {}",
        open_last[delivery]
    );
    assert!(
        walled_last[delivery] < 0.05,
        "a body already against the wall reported delivery {}",
        walled_last[delivery]
    );
}

/// Plan, "Phase-one body and action contract": *"Zero movement is rest; turning while
/// stopped is permitted and paid"*, against the shared budget
/// `abs(v) + body_radius * abs(yaw_rate)`.
///
/// A failure means either that a stopped body cannot turn (no orientation control at all)
/// or that turning is free (unlimited free scanning, and the motor term of the ES score
/// stops meaning anything).
#[test]
fn turning_while_stopped_moves_only_the_heading_and_is_paid() {
    let ticks = 41;
    let outcome = |turn: f64| {
        let world = flat(16, 3);
        let mut flora = Flora::new(FloraConfig::default());
        let (mut fauna, id) = one_founder(&world, Founder::Blind, 4, 1, EAST);
        assert!(fauna.set_controller(
            id,
            Box::new(Scripted::new(vec![Actions {
                forward: 0.0,
                turn,
                feed: 0.0,
            }])),
        ));
        let mut senses = Senses::new();
        run(&mut fauna, &world, &mut flora, &mut senses, ticks);
        let view = fauna.view();
        let a = *view.animal(id).expect("the founder survived");
        (
            view.organic(),
            view.ledger.respired_motor_out,
            view.ledger.respired_maintenance_out,
            a.pose,
        )
    };

    let (rest_organic, rest_motor, _, rest_pose) = outcome(0.0);
    let (turn_organic, turn_motor, turn_upkeep, turn_pose) = outcome(1.0);

    assert_eq!(rest_motor, 0.0, "resting paid a motor charge");
    assert_eq!(
        rest_pose.heading_rad, EAST,
        "a resting body's heading drifted"
    );

    // Only the heading moved.
    assert!(
        (turn_pose.x - rest_pose.x).abs() < 1e-12 && (turn_pose.z - rest_pose.z).abs() < 1e-12,
        "turning translated the body to ({}, {})",
        turn_pose.x,
        turn_pose.z
    );
    assert!(
        (turn_pose.heading_rad - EAST).abs() > 0.5,
        "a full turn effort left the heading at {}",
        turn_pose.heading_rad
    );

    // And it was paid at the budget the plan writes down. The body is held for the 36
    // ticks from the first sampling (tick 5) to the last stepped tick (tick 41).
    assert!(turn_motor > 0.0, "turning while stopped paid nothing");
    assert!(
        turn_organic < rest_organic,
        "the turning body kept as much organic matter as the resting one"
    );
    let manifest = Manifest::blind();
    let phys = cubarium_voxel_fauna::FounderPhysiology::frozen(Founder::Blind);
    let radius = manifest.body_width_m / 2.0;
    let ratio = radius * manifest.yaw_cap_rad_per_s / manifest.cruise_m_per_s;
    let held_ticks = (ticks - manifest.cadence_ticks() + 1) as f64;
    let expected = phys.motor_respiration_per_s
        * founder_body(Founder::Blind)
        * ratio
        * cubarium_voxel_fauna::DT
        * held_ticks;
    assert!(
        close(turn_motor, expected, 0.01),
        "a stopped full turn respired {turn_motor}, not the budget's {expected}"
    );
    // The frozen table sets full cruise at the basal rate, so a stopped full turn — half
    // the cruise equivalent for this founder — costs about half of upkeep.
    assert!(
        turn_motor < turn_upkeep,
        "the stopped turn ({turn_motor}) outcost basal upkeep ({turn_upkeep})"
    );
}

/// Plan, "Phase-one body and action contract": *"Use local swept collision/support checks,
/// with bounded steps to avoid tunneling."* A body driven at full cruise straight into a
/// one-voxel wall must never appear on its far side, on any tick.
///
/// A failure means the arena's geometry is not a constraint at all: a policy could walk
/// through terrain, and every claim about "reaching" food would be vacuous.
#[test]
fn full_cruise_cannot_tunnel_through_a_one_voxel_wall() {
    for founder in Founder::ALL {
        let manifest = founder.manifest();
        let mut world = flat(24, 3);
        // A one-voxel-thick wall three voxels tall, so nothing can climb it either.
        wall(&mut world, 6, GROUND_Y + 1, GROUND_Y + 3);
        let mut flora = Flora::new(FloraConfig::default());
        let (mut fauna, id) = one_founder(&world, founder, 3, 1, EAST);
        assert!(fauna.set_controller(
            id,
            Box::new(Scripted::new(vec![Actions {
                forward: 1.0,
                turn: 0.0,
                feed: 0.0,
            }])),
        ));
        let mut senses = Senses::new();

        let face = 6.0 * VOXEL_M - manifest.body_width_m / 2.0;
        for tick in 1..=120u64 {
            fauna.step_with_senses(&world, &mut flora, 1, &mut senses);
            let view = fauna.view();
            let a = view.animal(id).expect("the founder survived the sweep");
            assert!(
                a.pose.x <= face + 1e-9,
                "{}: tick {tick} put the body at x {} — past the wall face at {face}",
                founder.name(),
                a.pose.x
            );
            assert!(
                a.site.x < 6,
                "{}: tick {tick} put the body's support column at x {} — on or past the wall",
                founder.name(),
                a.site.x
            );
        }
        // It really did drive up to the wall rather than stall somewhere harmless.
        let reached = fauna.view().animal(id).expect("alive").pose.x;
        assert!(
            reached > face - manifest.body_width_m,
            "{}: the body only reached {reached} m, so the wall was never tested",
            founder.name()
        );
    }
}

// ------------------------------------------------------------------ paid food

/// Tests plan §1, "Paid food": *"Litter and foliage bites debit actual stocks and transfer
/// organic material, mineral and energy once"*, and the plan's *"one attempt per controller
/// interval"*. The three currencies are checked as residuals against the layer's own
/// boundary ledger and against the plant layer's stock, so nothing is taken on trust.
///
/// A failure means either food is free (a stock that does not fall, or matter that appears
/// in the animal without leaving the ground) or a bite is charged per tick rather than per
/// interval, which would silently multiply the arena's intake by the controller cadence.
#[test]
fn a_bite_debits_the_stock_once_per_interval_and_conserves_the_three_currencies() {
    let world = flat(8, 3);
    let mut flora = Flora::new(FloraConfig::default());
    let food = site(4, 1);
    litter(&mut flora, food, 0.2);

    let (mut fauna, id) = one_founder(&world, Founder::Blind, 4, 1, EAST);
    assert!(fauna.set_controller(
        id,
        Box::new(Scripted::new(vec![Actions {
            forward: 0.0,
            turn: 0.0,
            feed: 1.0,
        }])),
    ));

    let pools = |flora: &Flora| {
        let g = flora
            .view()
            .ground_at(food)
            .expect("the fixture's litter site")
            .clone();
        (g.litter, g.litter_mineral, g.litter_energy, g.mineral)
    };
    let before_ground = pools(&flora);
    let before_animal = *fauna.view().animal(id).expect("the founder");
    let before_consumed = flora.view().ledger.consumed_organic_out;

    // Ten ticks at the 0.25 s cadence: exactly two due controller intervals.
    let mut senses = Senses::new();
    run(&mut fauna, &world, &mut flora, &mut senses, 10);

    let view = fauna.view();
    let ledger = view.ledger;
    let after_ground = pools(&flora);
    let after_animal = *view.animal(id).expect("the founder survived ten ticks");

    // One withdrawal per interval, not per tick.
    assert_eq!(
        ledger.bites, 2,
        "two intervals produced {} bites",
        ledger.bites
    );
    let manifest = Manifest::blind();
    let want = cubarium_voxel_fauna::FounderPhysiology::frozen(Founder::Blind)
        .core
        .bite_per_s
        * manifest.controller_period_s;
    let debited = before_ground.0 - after_ground.0;
    assert!(
        close(debited, 2.0 * want, 1e-9),
        "two bites debited {debited} of litter, not 2 x {want}"
    );

    // The debit is the same number on both sides of the boundary.
    assert!(
        close(ledger.eaten_organic_in, debited, 1e-12),
        "the animal layer booked {} eaten against a stock that fell by {debited}",
        ledger.eaten_organic_in
    );
    assert!(
        close(
            flora.view().ledger.consumed_organic_out - before_consumed,
            ledger.eaten_organic_in,
            1e-12
        ),
        "the plant layer's consumed_organic_out and the animal layer's eaten_organic_in \
         disagree"
    );
    let mineral_debited = before_ground.1 - after_ground.1;
    assert!(
        close(ledger.eaten_mineral_in, mineral_debited, 1e-12),
        "mineral booked {} against a litter mineral fall of {mineral_debited}",
        ledger.eaten_mineral_in
    );
    let energy_debited = before_ground.2 - after_ground.2;
    assert!(
        close(ledger.eaten_energy_in, energy_debited, 1e-12),
        "energy booked {} against a litter energy fall of {energy_debited}",
        ledger.eaten_energy_in
    );
    assert!(
        ledger.eaten_organic_in > 0.0 && ledger.eaten_energy_in > 0.0,
        "the bite transferred nothing to check"
    );

    // What the animal holds is what it was given, minus what it respired and what it put
    // back. This is the "taken == gained + respired" residual, one currency at a time.
    let gained_organic = after_animal.organic() - before_animal.organic();
    assert!(
        close(
            gained_organic,
            ledger.eaten_organic_in - ledger.respired_out - ledger.deposited_organic_out,
            1e-12
        ),
        "organic residual: gained {gained_organic}, eaten {}, respired {}, deposited {}",
        ledger.eaten_organic_in,
        ledger.respired_out,
        ledger.deposited_organic_out
    );
    let gained_mineral = after_animal.mineral - before_animal.mineral;
    assert!(
        close(
            gained_mineral,
            ledger.eaten_mineral_in - ledger.deposited_mineral_out,
            1e-12
        ),
        "mineral residual: gained {gained_mineral}, eaten {}, deposited {}",
        ledger.eaten_mineral_in,
        ledger.deposited_mineral_out
    );
    let gained_energy = after_animal.energy - before_animal.energy;
    assert!(
        close(
            gained_energy,
            ledger.eaten_energy_in - ledger.heat_out - ledger.deposited_energy_out,
            1e-12
        ),
        "energy residual: gained {gained_energy}, eaten {}, heat {}, deposited {}",
        ledger.eaten_energy_in,
        ledger.heat_out,
        ledger.deposited_energy_out
    );
    // Mineral is never respired and never created: the excess came back as dung on the
    // site the animal stood on.
    assert!(
        close(
            after_ground.3 - before_ground.3,
            ledger.deposited_mineral_out,
            1e-12
        ),
        "the excreted mineral did not land on the ground"
    );
    // And the three splits of respiration add up to the total.
    assert!(
        close(
            ledger.respired_maintenance_out
                + ledger.respired_motor_out
                + ledger.respired_digestion_out,
            ledger.respired_out,
            1e-12
        ),
        "the respiration splits do not sum to the total"
    );
}

/// Tests plan §1, "Paid food": *"failed/no-contact feeding transfers nothing"*, and the
/// plan's *"The mouth can contact only its actual local footprint and short forward
/// reach"*. A full feed effort held for eight intervals over bare ground, with a stock
/// three columns away, must move nothing.
///
/// A failure means remote feeding: an arena's "acquisition" result would then say nothing
/// about whether the body ever reached the food.
#[test]
fn a_feed_effort_with_no_mouth_contact_transfers_nothing() {
    // Blind: litter three columns off, mouth reach is 0.25 body lengths.
    let world = flat(16, 3);
    let mut flora = Flora::new(FloraConfig::default());
    let far = site(7, 1);
    litter(&mut flora, far, 0.2);
    let pools = |flora: &Flora| {
        let g = flora
            .view()
            .ground_at(far)
            .expect("the fixture's litter site")
            .clone();
        (g.litter, g.litter_mineral, g.litter_energy)
    };
    let before = pools(&flora);

    let (mut fauna, id) = one_founder(&world, Founder::Blind, 4, 1, EAST);
    let (recorder, log) = Recorder::new(Actions {
        forward: 0.0,
        turn: 0.0,
        feed: 1.0,
    });
    assert!(fauna.set_controller(id, Box::new(recorder)));
    let mut senses = Senses::new();
    run(&mut fauna, &world, &mut flora, &mut senses, 40);

    let ledger = fauna.view().ledger;
    assert_eq!(
        ledger.bites, 0,
        "a mouth over bare ground took {} bites",
        ledger.bites
    );
    assert_eq!(ledger.eaten_organic_in, 0.0);
    assert_eq!(ledger.eaten_mineral_in, 0.0);
    assert_eq!(ledger.eaten_energy_in, 0.0);
    assert_eq!(pools(&flora), before, "an unreachable stock changed");

    // The observation says the same thing: no intake was ever reported back.
    let manifest = Manifest::blind();
    let intake = slot(&manifest, "Self", "assimilated_intake");
    let samples = log.lock().expect("the log").clone();
    assert!(
        samples.len() >= 8,
        "only {} samples in 40 ticks",
        samples.len()
    );
    for (k, o) in samples.iter().enumerate() {
        assert_eq!(o[intake], 0.0, "sample {k} reported intake {}", o[intake]);
    }

    // Browser: a stand three columns off, the mouth in air. Taste is invalid without
    // contact, which is the direct evidence that there was no mouth contact to feed from.
    let world = flat(16, 3);
    let mut flora = Flora::new(FloraConfig::default());
    let wood = 0.5 * flora.config().species(Plant::Springturf).wood_max;
    assert!(flora.apply(
        &world,
        FloraCommand::Seed {
            x: 7,
            z: 1,
            species: Plant::Springturf,
            wood,
        },
    ));
    let stand_before = flora
        .view()
        .stand_at(site(7, 1))
        .expect("the fixture's stand")
        .foliage;

    let (mut fauna, id) = one_founder(&world, Founder::Browser, 4, 1, EAST);
    let (recorder, log) = Recorder::new(Actions {
        forward: 0.0,
        turn: 0.0,
        feed: 1.0,
    });
    assert!(fauna.set_controller(id, Box::new(recorder)));
    let mut senses = Senses::new();
    run(&mut fauna, &world, &mut flora, &mut senses, 40);

    assert_eq!(
        fauna.view().ledger.bites,
        0,
        "the browser cropped out of reach"
    );
    assert_eq!(
        flora
            .view()
            .stand_at(site(7, 1))
            .expect("the stand is still there")
            .foliage,
        stand_before,
        "an out-of-reach stand lost foliage"
    );
    let taste_valid = slot(&Manifest::browser(), "Taste(1)", "valid");
    for (k, o) in log.lock().expect("the log").iter().enumerate() {
        assert_eq!(
            o[taste_valid], 0.0,
            "browser sample {k} claimed a valid mouth taste with the mouth in air"
        );
    }
}

// ------------------------------------------------------------------- feedback

/// Plan, "Exact starting manifests": `Self` carries *"actual structural loss over interval,
/// assimilated intake, resolved forward movement, resolved turn, motor delivery"*, and
/// *"Initial intake/loss/motion feedback is zero."* The feedback is the **previous**
/// interval only — it does not accumulate across intervals and it does not leak forward.
///
/// A failure means the recurrent policy is being fed a running total (so the `Self`
/// channels drift with episode length) or a stale one (so proprioception lags the body),
/// either of which quietly changes what a trained policy can learn.
#[test]
fn self_feedback_reports_the_previous_interval_only_and_starts_at_zero() {
    let manifest = Manifest::blind();
    let loss = slot(&manifest, "Self", "structural_loss");
    let intake = slot(&manifest, "Self", "assimilated_intake");
    let forward = slot(&manifest, "Self", "resolved_forward");
    let turn = slot(&manifest, "Self", "resolved_turn");
    let delivery = slot(&manifest, "Self", "motor_delivery");
    let trend = slot(&manifest, "Chem(litter)", "trend");

    let world = flat(16, 3);
    let mut flora = Flora::new(FloraConfig::default());
    let (mut fauna, id) = one_founder(&world, Founder::Blind, 4, 1, EAST);
    // One interval of motion, then rest: sample 1 sees nothing, sample 2 sees that one
    // interval, sample 3 sees the rest that followed it.
    let (recorder, log) = ScriptedRecorder::new(vec![
        Actions {
            forward: 0.5,
            turn: 0.4,
            feed: 0.0,
        },
        Actions::REST,
    ]);
    assert!(fauna.set_controller(id, Box::new(recorder)));

    let mut senses = Senses::new();
    run(&mut fauna, &world, &mut flora, &mut senses, 16);
    let samples = log.lock().expect("log").clone();
    assert_eq!(samples.len(), 3, "expected three samples in 16 ticks");

    // First sample after the body entered the world: every feedback channel is zero, and
    // delivery reads the contract's 1 because nothing was requested.
    for (name, k) in [
        ("structural_loss", loss),
        ("assimilated_intake", intake),
        ("resolved_forward", forward),
        ("resolved_turn", turn),
    ] {
        assert_eq!(
            samples[0][k], 0.0,
            "the first sample reported {name} = {}",
            samples[0][k]
        );
    }
    assert_eq!(
        samples[0][delivery], 1.0,
        "the first sample's motor delivery was {} and not the contract's 1",
        samples[0][delivery]
    );
    assert_eq!(
        samples[0][trend], 0.0,
        "the first sample invented a chemical trend"
    );

    // Second sample: exactly the interval that was held, normalized against one controller
    // period of the manifest's cruise and yaw references.
    assert!(
        close(samples[1][forward], 0.5, 1e-9),
        "resolved_forward was {} for a 0.5 cruise interval",
        samples[1][forward]
    );
    assert!(
        close(samples[1][turn], 0.4, 1e-9),
        "resolved_turn was {} for a 0.4 yaw interval",
        samples[1][turn]
    );
    assert!(
        close(samples[1][delivery], 1.0, 1e-9),
        "an unobstructed interval delivered {}",
        samples[1][delivery]
    );
    assert_eq!(samples[1][intake], 0.0, "intake appeared without food");

    // Third sample: the rest interval, not the motion before it.
    assert_eq!(
        samples[2][forward], 0.0,
        "the motion of the previous-but-one interval leaked forward as {}",
        samples[2][forward]
    );
    assert_eq!(samples[2][turn], 0.0, "a stale turn leaked forward");
    assert_eq!(
        samples[2][delivery], 1.0,
        "a rest interval reported delivery {}",
        samples[2][delivery]
    );
}

/// The same rule for the one channel that needs food: `Self.assimilated_intake` is the
/// previous interval's intake and nothing else. One feeding interval, then rest.
///
/// A failure means a policy sees its meal for longer than it lasted — a reward signal
/// leaking into the observation.
#[test]
fn intake_feedback_is_the_previous_interval_only() {
    let world = flat(8, 3);
    let mut flora = Flora::new(FloraConfig::default());
    litter(&mut flora, site(4, 1), 0.2);
    let (mut fauna, id) = one_founder(&world, Founder::Blind, 4, 1, EAST);
    let (recorder, log) = ScriptedRecorder::new(vec![
        Actions {
            forward: 0.0,
            turn: 0.0,
            feed: 1.0,
        },
        Actions::REST,
    ]);
    assert!(fauna.set_controller(id, Box::new(recorder)));
    let mut senses = Senses::new();
    run(&mut fauna, &world, &mut flora, &mut senses, 16);

    let intake = slot(&Manifest::blind(), "Self", "assimilated_intake");
    let samples = log.lock().expect("log").clone();
    assert_eq!(samples.len(), 3);
    assert_eq!(samples[0][intake], 0.0, "intake before the first bite");
    assert!(
        samples[1][intake] > 0.0,
        "the interval that bit reported intake {}",
        samples[1][intake]
    );
    assert_eq!(
        samples[2][intake], 0.0,
        "the bite's intake was still on the wire an interval later: {}",
        samples[2][intake]
    );
    assert_eq!(
        fauna.view().ledger.bites,
        1,
        "more than the one scripted bite"
    );
}

// ------------------------------------------------- chemical field and sampler

/// Plan, "Initial cue field settings": *"Use bilinear sampling among connected same-layer
/// nodes"*. Two receptor positions **inside one gradient cell** must read differently.
///
/// The two arms are the same world, the same frozen litter and the same tick, so the field
/// itself is identical; only the sub-voxel position differs, and both bodies are verified
/// to be in the same support column at the moment of the sample. A failure means the
/// sampler is piecewise-constant per voxel, which is a 0.25 m dead-band on the only remote
/// sense the blind founder has.
#[test]
fn two_receptor_positions_inside_one_gradient_cell_read_differently() {
    struct Recorded {
        action: Actions,
        log: SampleLog,
        column: Arc<Mutex<i64>>,
    }
    impl Controller for Recorded {
        fn drive(&mut self, o: &[f64]) -> Response {
            let col = *self.column.lock().expect("column");
            self.log.lock().expect("log").push((o.to_vec(), col));
            Response::Bounded(self.action)
        }
        fn reset(&mut self) {}
    }

    let outcome = |forward: f64| {
        let world = flat(16, 3);
        let mut flora = Flora::new(FloraConfig::default());
        litter(&mut flora, site(6, 1), 0.2);
        let mut senses = Senses::new();
        let (updates, converged) = senses.settle(&world.view(), &flora.view());
        assert!(
            converged,
            "the fixture's field did not settle in {updates} updates"
        );

        let (mut fauna, id) = one_founder(&world, Founder::Blind, 4, 1, EAST);
        let log = Arc::new(Mutex::new(Vec::new()));
        let column = Arc::new(Mutex::new(-1i64));
        assert!(fauna.set_controller(
            id,
            Box::new(Recorded {
                action: Actions {
                    forward,
                    turn: 0.0,
                    feed: 0.0,
                },
                log: Arc::clone(&log),
                column: Arc::clone(&column),
            }),
        ));
        for _ in 0..16 {
            *column.lock().expect("column") =
                i64::from(fauna.view().animal(id).expect("alive").site.x);
            fauna.step_with_senses(&world, &mut flora, 1, &mut senses);
        }
        let pose = fauna.view().animal(id).expect("alive").pose;
        (log.lock().expect("log").clone(), pose)
    };

    let (still, still_pose) = outcome(0.0);
    let (moved, moved_pose) = outcome(1.0);
    assert_eq!(still.len(), 3);
    assert_eq!(moved.len(), 3);

    let response = slot(&Manifest::blind(), "Chem(litter)", "response");
    let valid = slot(&Manifest::blind(), "Chem(litter)", "valid");
    assert_eq!(still[0].0[valid], 1.0, "the settled field read as invalid");
    assert!(
        still[0].0[response] > 1e-3,
        "the fixture has no cue to grade: response {}",
        still[0].0[response]
    );

    // Same tick, same field, same support column for both bodies.
    for k in 0..3 {
        assert_eq!(
            still[k].1, moved[k].1,
            "sample {k} was taken from different columns ({} vs {})",
            still[k].1, moved[k].1
        );
    }
    assert!(
        (still_pose.x - 4.5 * VOXEL_M).abs() < 1e-12,
        "the still body drifted to {}",
        still_pose.x
    );
    assert!(
        moved_pose.x > still_pose.x + 1e-3 && moved_pose.x < still_pose.x + VOXEL_M / 2.0,
        "the moving body left the cell: {} vs {}",
        moved_pose.x,
        still_pose.x
    );

    // The identical first sample is the control: both bodies are at the face centre.
    assert!(
        close(still[0].0[response], moved[0].0[response], 1e-15),
        "the two arms disagreed before either moved"
    );
    // The graded readings are the claim.
    for k in 1..3 {
        assert!(
            moved[k].0[response] > still[k].0[response] + 1e-9,
            "sample {k}: a body {} m further up the gradient inside the same voxel read \
             {} against the stationary body's {}",
            moved_pose.x - still_pose.x,
            moved[k].0[response],
            still[k].0[response]
        );
    }
}

/// Plan, "Initial cue field settings": *"preserving barriers"* — a support-layer node is an
/// exposed support face, so a one-voxel wall between two same-height nodes is not an edge,
/// and the far side's value is not interpolated into the near side's reading.
///
/// Two checks in one fixture, both against a control that differs only by that one wall:
/// the wall's own cell drops out of the receptor's bilinear stencil, and the cue cannot
/// spread across it. A failure means smell passes through terrain, so "reach the cue" stops
/// being a navigation problem.
#[test]
fn a_one_voxel_wall_between_two_nodes_blocks_the_cue_and_its_interpolation() {
    // The strip wraps, so a far wall closes the long way round in both arms; the near wall
    // at x = 5 is the only difference between them.
    let outcome = |near_wall: bool| {
        let mut world = flat(16, 3);
        wall(&mut world, 12, GROUND_Y + 1, GROUND_Y + 2);
        if near_wall {
            wall(&mut world, 5, GROUND_Y + 1, GROUND_Y + 2);
        }
        let mut flora = Flora::new(FloraConfig::default());
        litter(&mut flora, site(6, 1), 0.2);
        let mut senses = Senses::new();
        senses.settle(&world.view(), &flora.view());

        // The receptor stands at x = 4, immediately behind the wall's column: its bilinear
        // stencil covers columns 4 and 5, so the wall's cell is the far-side corner.
        let (mut fauna, id) = one_founder(&world, Founder::Blind, 4, 1, EAST);
        let (recorder, log) = Recorder::new(Actions::REST);
        assert!(fauna.set_controller(id, Box::new(recorder)));
        run(&mut fauna, &world, &mut flora, &mut senses, 6);
        log.lock()
            .expect("log")
            .first()
            .cloned()
            .expect("one sample")
    };

    let manifest = Manifest::blind();
    let response = slot(&manifest, "Chem(litter)", "response");
    let valid = slot(&manifest, "Chem(litter)", "valid");

    let open = outcome(false);
    let walled = outcome(true);

    assert_eq!(open[valid], 1.0);
    assert_eq!(
        walled[valid], 1.0,
        "a blocked receptor reported invalid rather than a valid zero"
    );
    assert!(
        open[response] > 1e-3,
        "the control never smelled the source: {}",
        open[response]
    );
    assert!(
        walled[response] < 1e-9,
        "the cue crossed a one-voxel wall: {} against the open arm's {}",
        walled[response],
        open[response]
    );
}

/// Plan, "Frozen arena contract": *"Resource depletion stops emission ... Remaining odor
/// decays normally"*, and "Initial cue field settings": *"decay with half-life 2 s"*.
///
/// The fixture makes the field spatially uniform (litter on every ground site of a small
/// strip), so diffusion is a no-op and the node value is pure decay once the source is
/// gone. Two samples four field updates — exactly 2 s — apart must be a factor of two
/// apart. A failure means either that a depleted patch keeps emitting (the arena's
/// depletion signal is a lie) or that the residue decays at the wrong rate, which changes
/// how long a Stage-C "decaying residue at a depleted patch" fixture is distinguishable.
#[test]
fn removing_the_source_stops_emission_and_the_residue_decays_at_the_stated_half_life() {
    let world = flat(8, 2);
    let mut flora = Flora::new(FloraConfig::default());
    let sites: Vec<Site> = (0..8u32)
        .flat_map(|x| (0..2u32).map(move |z| site(x, z)))
        .collect();
    for &s in &sites {
        litter(&mut flora, s, 0.1);
    }
    let mut senses = Senses::new();
    let (_, converged) = senses.settle(&world.view(), &flora.view());
    assert!(converged, "the uniform fixture's field did not settle");

    let (mut fauna, id) = one_founder(&world, Founder::Blind, 4, 0, EAST);
    let (recorder, log) = Recorder::new(Actions::REST);
    assert!(fauna.set_controller(id, Box::new(recorder)));

    // Ticks 1..=10 with the litter present; the field update at tick 10 is the last one
    // that emits.
    run(&mut fauna, &world, &mut flora, &mut senses, 10);
    for &s in &sites {
        flora.take_litter(s, 1.0);
    }
    for &s in &sites {
        assert_eq!(
            flora.view().ground_at(s).map_or(0.0, |g| g.litter),
            0.0,
            "the fixture failed to empty {s:?}"
        );
    }
    // Ticks 11..=60: field updates at 20, 30, 40, 50 and 60, all with zero emission.
    run(&mut fauna, &world, &mut flora, &mut senses, 50);

    let manifest = Manifest::blind();
    let response = slot(&manifest, "Chem(litter)", "response");
    let saturation = manifest.tunings.chem_saturation;
    let samples = log.lock().expect("log").clone();
    assert_eq!(samples.len(), 12, "expected a sample every five ticks");

    // `q = C / (C + saturation)`, so `C = saturation * q / (1 - q)`.
    let concentration = |q: f64| {
        assert!(
            q > 0.0 && q < 1.0,
            "a response of {q} carries no concentration"
        );
        saturation * q / (1.0 - q)
    };
    // Samples are at ticks 5, 10, ..., 60: index 3 is tick 20 and index 11 is tick 60.
    let c20 = concentration(samples[3][response]);
    let c60 = concentration(samples[11][response]);
    let ratio = c60 / c20;
    assert!(
        (ratio - 0.5).abs() < 0.01,
        "four 0.5 s updates after the source went (2 s, one stated half-life) left \
         {ratio:.4} of the residue, not 0.5 (C {c20:.5} -> {c60:.5})"
    );

    // Emission really stopped: every post-removal sample is strictly below the one before.
    for k in 4..samples.len() {
        assert!(
            samples[k][response] <= samples[k - 1][response] + 1e-15,
            "sample {k} rose to {} from {} after the source was gone",
            samples[k][response],
            samples[k - 1][response]
        );
    }
    assert!(
        samples[11][response] < samples[3][response] - 1e-6,
        "the residue did not decay at all"
    );
}

// ------------------------------------------------------------- material cone

/// Plan, "Exact starting manifests": *"Terrain, wood, litter and water still occlude even
/// though their identities are not exposed."* A rock standing in front of foliage must
/// register as an all-hit (the sector is not clear, and the proximity of what was hit is
/// real) while the foliage fraction of that sector reads zero.
///
/// A failure means the eye sees through terrain, so an arena with an occluder — Stage C's
/// whole point — measures nothing.
#[test]
fn a_rock_in_front_of_foliage_is_an_all_hit_with_a_zero_foliage_fraction() {
    let manifest = Manifest::browser();
    let clear = slot(&manifest, "Cone(3, foliage/body)", "s1.clear");
    let all_prox = slot(&manifest, "Cone(3, foliage/body)", "s1.all_proximity");
    let foliage = slot(&manifest, "Cone(3, foliage/body)", "s1.foliage");
    let foliage_prox = slot(&manifest, "Cone(3, foliage/body)", "s1.foliage_proximity");
    let valid = slot(&manifest, "Cone(3, foliage/body)", "valid");

    let outcome = |rock: bool| {
        let mut world = flat(24, 5);
        if rock {
            // Tall enough that the +/-20 degree pitch rays cannot pass over it.
            wall(&mut world, 6, GROUND_Y + 1, GROUND_Y + 3);
        }
        let mut flora = Flora::new(FloraConfig::default());
        let wood = 0.5 * flora.config().species(Plant::Springturf).wood_max;
        assert_eq!(
            flora.config().species(Plant::Springturf).crown_voxels(wood),
            1,
            "this fixture needs a crown sitting one voxel above its face"
        );
        assert!(flora.apply(
            &world,
            FloraCommand::Seed {
                x: 8,
                z: 2,
                species: Plant::Springturf,
                wood,
            },
        ));
        let (mut fauna, id) = one_founder(&world, Founder::Browser, 4, 2, EAST);
        let (recorder, log) = Recorder::new(Actions::REST);
        assert!(fauna.set_controller(id, Box::new(recorder)));
        let mut senses = Senses::new();
        run(&mut fauna, &world, &mut flora, &mut senses, 6);
        log.lock()
            .expect("log")
            .first()
            .cloned()
            .expect("one sample")
    };

    let open = outcome(false);
    let blocked = outcome(true);

    assert_eq!(open[valid], 1.0);
    assert_eq!(blocked[valid], 1.0);
    assert!(
        open[foliage] > 0.0 && open[foliage_prox] > 0.0,
        "the control never saw the foliage at all (fraction {}, proximity {})",
        open[foliage],
        open[foliage_prox]
    );
    assert_eq!(
        blocked[foliage], 0.0,
        "foliage behind a rock was reported at fraction {}",
        blocked[foliage]
    );
    assert_eq!(
        blocked[foliage_prox], 0.0,
        "a zero-hit foliage slot carried a proximity of {}",
        blocked[foliage_prox]
    );
    assert_eq!(
        blocked[clear], 0.0,
        "the forward sector still read {} clear with a wall two voxels ahead",
        blocked[clear]
    );
    assert!(
        blocked[all_prox] > 0.0,
        "the rock produced no all-hit proximity: {}",
        blocked[all_prox]
    );
    // The rock is nearer than the foliage was, so it must read as nearer.
    assert!(
        blocked[all_prox] > open[all_prox],
        "the nearer occluder ({}) read no closer than the far foliage ({})",
        blocked[all_prox],
        open[all_prox]
    );
}

/// The same occlusion rule for the other exposed class: a body behind the rock is not seen.
///
/// A failure means one founder can see another through terrain — an eye that reports
/// things no ray reached.
#[test]
fn a_body_behind_the_rock_is_not_seen() {
    let manifest = Manifest::browser();
    let body = slot(&manifest, "Cone(3, foliage/body)", "s1.body");
    let body_prox = slot(&manifest, "Cone(3, foliage/body)", "s1.body_proximity");

    let outcome = |rock: bool| {
        let mut world = flat(24, 5);
        if rock {
            wall(&mut world, 6, GROUND_Y + 1, GROUND_Y + 3);
        }
        let mut flora = Flora::new(FloraConfig::default());
        let (mut fauna, id) = one_founder(&world, Founder::Browser, 4, 2, EAST);
        // A second browser body four voxels down the lane, behind the rock.
        assert!(fauna.apply(
            &world,
            Command::IntroduceFounder {
                x: 8,
                z: 2,
                founder: Founder::Browser,
                stores: founder_stores(Founder::Browser),
                heading_rad: 0.0,
            },
        ));
        let (recorder, log) = Recorder::new(Actions::REST);
        assert!(fauna.set_controller(id, Box::new(recorder)));
        let mut senses = Senses::new();
        run(&mut fauna, &world, &mut flora, &mut senses, 6);
        log.lock()
            .expect("log")
            .first()
            .cloned()
            .expect("one sample")
    };

    let open = outcome(false);
    let blocked = outcome(true);

    assert!(
        open[body] > 0.0 && open[body_prox] > 0.0,
        "the control never saw the other body (fraction {}, proximity {})",
        open[body],
        open[body_prox]
    );
    assert_eq!(
        blocked[body], 0.0,
        "a body behind a rock was reported at fraction {}",
        blocked[body]
    );
    assert_eq!(
        blocked[body_prox], 0.0,
        "a zero-hit body slot carried a proximity"
    );
    // The observer does not occlude itself either: if it did, the control would be zero.
}

// ------------------------------------------------------------- policy boundary

/// Plan, "Exact starting manifests" and the tests plan's *"Controller receives only the
/// vector and its own memory"*: the slice handed to `Controller::drive` is exactly the
/// manifest's input width, every value finite and inside the channel's declared range.
///
/// A failure means the policy boundary leaks — either a shape the GRU loader would reject,
/// or a value outside the range the manifest's normalization promises.
#[test]
fn a_controller_receives_exactly_the_manifest_length_and_nothing_else() {
    for founder in Founder::ALL {
        let manifest = founder.manifest();
        let world = flat(16, 3);
        let mut flora = Flora::new(FloraConfig::default());
        litter(&mut flora, site(6, 1), 0.2);
        let mut senses = Senses::new();
        senses.settle(&world.view(), &flora.view());

        let (mut fauna, id) = one_founder(&world, founder, 4, 1, EAST);
        let (recorder, log) = Recorder::new(Actions {
            forward: 0.6,
            turn: -0.3,
            feed: 1.0,
        });
        assert!(fauna.set_controller(id, Box::new(recorder)));
        run(&mut fauna, &world, &mut flora, &mut senses, 21);

        let samples = log.lock().expect("log").clone();
        assert_eq!(samples.len(), 4, "{}: sample count", founder.name());
        // The two channels the schema encodes as signed: a resolved turn and a smoothed
        // time derivative. Everything else is a normalized magnitude in [0, 1].
        let signed: Vec<usize> = manifest
            .modules
            .iter()
            .flat_map(|m| {
                m.channels
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| **c == "resolved_turn" || **c == "trend")
                    .map(move |(k, _)| m.offset + k)
            })
            .collect();
        for (k, o) in samples.iter().enumerate() {
            assert_eq!(
                o.len(),
                manifest.inputs(),
                "{}: sample {k} was {} long, not the manifest's {}",
                founder.name(),
                o.len(),
                manifest.inputs()
            );
            for (i, v) in o.iter().enumerate() {
                assert!(
                    v.is_finite(),
                    "{}: sample {k} channel {i} is {v}",
                    founder.name()
                );
                let low = if signed.contains(&i) { -1.0 } else { 0.0 };
                assert!(
                    *v >= low && *v <= 1.0,
                    "{}: sample {k} channel {i} is {v}, outside [{low}, 1]",
                    founder.name()
                );
            }
        }
    }
    // The two widths the plan writes down, so a manifest edit cannot quietly move them.
    assert_eq!(Manifest::blind().inputs(), 23);
    assert_eq!(Manifest::browser().inputs(), 37);
}

/// Tests plan §1, "Policy boundary": *"the observation contains no value that changes when
/// a resource is moved outside sensing range"*. Two worlds identical but for where the
/// stock sits — inside the browser's 2 m cone in one, well outside it in the other — must
/// produce the same observation vector, element for element.
///
/// A failure means the observation carries a hidden channel of world knowledge, and any
/// "the policy used its senses" claim would be untestable.
#[test]
fn moving_a_resource_outside_sensing_range_changes_no_observation_value() {
    // Browser: the cone has a hard 2 m range, so 12 columns (3 m either way round a
    // 24-column strip) is outside it.
    let browser_sample = |stand_x: i64| {
        let world = flat(24, 5);
        let mut flora = Flora::new(FloraConfig::default());
        let wood = 0.5 * flora.config().species(Plant::Springturf).wood_max;
        assert!(flora.apply(
            &world,
            FloraCommand::Seed {
                x: stand_x,
                z: 2,
                species: Plant::Springturf,
                wood,
            },
        ));
        let (mut fauna, id) = one_founder(&world, Founder::Browser, 4, 2, EAST);
        let (recorder, log) = Recorder::new(Actions::REST);
        assert!(fauna.set_controller(id, Box::new(recorder)));
        let mut senses = Senses::new();
        run(&mut fauna, &world, &mut flora, &mut senses, 6);
        log.lock()
            .expect("log")
            .first()
            .cloned()
            .expect("one sample")
    };

    // A positive control first: inside the 2 m cone the stand does change the vector, so
    // the comparison that follows is not vacuous.
    let near = browser_sample(8);
    let far_a = browser_sample(14);
    let far_b = browser_sample(16);
    let manifest = Manifest::browser();
    let foliage = slot(&manifest, "Cone(3, foliage/body)", "s1.foliage");
    assert!(
        near[foliage] > 0.0,
        "the near arm did not see the stand, so the sensing range was never exercised"
    );
    assert!(
        near != far_a,
        "a stand inside the cone read the same as one outside it"
    );
    // Both of these are 2.5 m and 3.0 m away, out of range in either direction round the
    // 6 m strip. Moving the stock between them must move nothing at all.
    assert_eq!(far_a[foliage], 0.0, "an out-of-range stand was still seen");
    for (i, (a, b)) in far_a.iter().zip(far_b.iter()).enumerate() {
        assert_eq!(
            a, b,
            "browser channel {i} moved from {a} to {b} when an already out-of-range stand \
             was moved further away"
        );
    }

    // Blind: the litter cue field has no hard range, but it is discarded below 1e-5, so a
    // source 20 and 30 columns away on a 48-column strip is outside the field's support
    // either way. Both readings must be the same valid zero.
    let blind_sample = |litter_x: u32| {
        let world = flat(48, 3);
        let mut flora = Flora::new(FloraConfig::default());
        litter(&mut flora, site(litter_x, 1), 0.2);
        let mut senses = Senses::new();
        senses.settle(&world.view(), &flora.view());
        let (mut fauna, id) = one_founder(&world, Founder::Blind, 4, 1, EAST);
        let (recorder, log) = Recorder::new(Actions::REST);
        assert!(fauna.set_controller(id, Box::new(recorder)));
        run(&mut fauna, &world, &mut flora, &mut senses, 6);
        log.lock()
            .expect("log")
            .first()
            .cloned()
            .expect("one sample")
    };

    let manifest = Manifest::blind();
    let response = slot(&manifest, "Chem(litter)", "response");
    let valid = slot(&manifest, "Chem(litter)", "valid");
    let a = blind_sample(24);
    let b = blind_sample(34);
    assert_eq!(a[valid], 1.0, "an out-of-range cue read as invalid");
    assert_eq!(
        a[response], 0.0,
        "a source 20 columns off left a residue of {} at the receptor",
        a[response]
    );
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        assert_eq!(
            x, y,
            "blind channel {i} moved from {x} to {y} when the litter went further away"
        );
    }
}

/// **This test fails against the source at `8071efb`, and the failure is the report.**
///
/// Plan, "Initial cue field settings": *"Use bilinear sampling among connected same-layer
/// nodes"*, with the field carrying *"one value per support-layer node"*. A node's value
/// belongs at that support face — the face's centre — so a receptor standing at a face
/// centre should read that face's own value, and two receptors placed symmetrically either
/// side of a single source should read the same thing.
///
/// `LitterField::sample` (crates/cubarium-voxel-fauna/src/senses.rs:179-217) instead takes
/// `x0 = floor(pose.x / v)` and `z0 = floor(pose.z / v)` as the **corner** of the stencil
/// and weights cells `(x0, z0)`, `(x0+1, z0)`, `(x0, z0+1)`, `(x0+1, z0+1)` by the
/// fractional position inside the cell. A pose at a face centre has `dx = dz = 0.5`, so it
/// reads a flat quarter of its own node and of its `+x`, `+z` and `+x+z` neighbours, and
/// nothing of its `-x` or `-z` neighbours: the sampled field is displaced half a voxel
/// toward `-x, -z`. The observable consequence is that the cue reads **stronger when the
/// source is behind you in `-x`/`-z`** than when it is the same distance ahead.
///
/// It is a half-voxel bias, not a lost signal, and the blind founder can still climb the
/// gradient; but it is a direction-dependent distortion of the only remote sense the blind
/// founder has. Fixed on main by centring the stencil on `pose/v - 0.5`; this test is the
/// witness that it stays fixed.
#[test]
fn a_receptor_at_a_face_centre_reads_that_face_symmetrically() {
    let sample_at = |x: i64| {
        let world = flat(16, 3);
        let mut flora = Flora::new(FloraConfig::default());
        litter(&mut flora, site(8, 1), 0.2);
        let mut senses = Senses::new();
        senses.settle(&world.view(), &flora.view());
        let (mut fauna, id) = one_founder(&world, Founder::Blind, x, 1, EAST);
        let (recorder, log) = Recorder::new(Actions::REST);
        assert!(fauna.set_controller(id, Box::new(recorder)));
        run(&mut fauna, &world, &mut flora, &mut senses, 6);
        let o = log.lock().expect("log").first().cloned().expect("a sample");
        o[slot(&Manifest::blind(), "Chem(litter)", "response")]
    };

    // One column either side of the single source, both at their face centres.
    let behind = sample_at(7);
    let ahead = sample_at(9);
    assert!(
        close(behind, ahead, 1e-6),
        "a single source at x = 8 read {behind} from x = 7 and {ahead} from x = 9: the \
         bilinear stencil is cornered on floor(pose/voxel) rather than centred on the node, \
         so the sampled field is displaced half a voxel toward -x/-z"
    );

    // The same asymmetry across z, with the source one row back.
    let sample_z = |z: u32| {
        let world = flat(16, 5);
        let mut flora = Flora::new(FloraConfig::default());
        litter(&mut flora, site(8, 2), 0.2);
        let mut senses = Senses::new();
        senses.settle(&world.view(), &flora.view());
        let (mut fauna, id) = one_founder(&world, Founder::Blind, 8, z, EAST);
        let (recorder, log) = Recorder::new(Actions::REST);
        assert!(fauna.set_controller(id, Box::new(recorder)));
        run(&mut fauna, &world, &mut flora, &mut senses, 6);
        let o = log.lock().expect("log").first().cloned().expect("a sample");
        o[slot(&Manifest::blind(), "Chem(litter)", "response")]
    };
    let front = sample_z(1);
    let back = sample_z(3);
    assert!(
        close(front, back, 1e-6),
        "a single source at z = 2 read {front} from z = 1 and {back} from z = 3"
    );
}
