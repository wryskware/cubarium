//! The opt-in paid hunter: founding, local hunting, capture settlement, digestion, death and
//! one funded offspring (`design/7_Research/astra-fixed-hunter-implementation-plan-2026-09-13.md`).
//!
//! Every quantity is recomputed here from public state, so a test never takes the world's own
//! bookkeeping as evidence for itself. Two of the trial parameters are replaced in these
//! fixtures to make the outcome deterministic: `capture_min = capture_max = 1` for a certain
//! capture and `= 0` for a certain miss. Nothing else about the profile is changed, and no
//! test here is evidence about ecological balance.

use cubarium_core::genome::{Genome, decode};
use cubarium_core::hunter::{
    AttemptOutcome, FixedHunterProfile, HunterEvent, HunterMember, HunterPhase, HunterTarget,
};
use cubarium_core::ids::OrganismId;
use cubarium_core::organism::{DeathCause, Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::snapshot::{SnapshotError, state_hash};
use cubarium_core::world::WorldState;
use cubarium_core::{LifeEvent, World, WorldConfig, decode_snapshot, encode_snapshot};
use cubarium_surface::{Face, SurfacePoint, Vec2, travel};

// ---------------------------------------------------------------- fixtures

/// An empty, quiet world: no founders, no weather motion, no rain. Only what a test places.
fn empty_world() -> World {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds.clear();
    cfg.founders.count = 0;
    cfg.weather.amplitude = 0.0;
    cfg.water.rain_rate = 0.0;
    World::new(cfg).expect("an empty world is valid")
}

/// A world with nothing growing, nothing decomposing and no litter of its own: whatever moves
/// in the fields here was moved by what the test placed there.
fn quiet_world() -> World {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds.clear();
    cfg.founders.count = 0;
    cfg.weather.amplitude = 0.0;
    cfg.water.rain_rate = 0.0;
    cfg.habitat.light_base = 0.0;
    cfg.habitat.light_height_gain = 0.0;
    cfg.habitat.light_noise_gain = 0.0;
    cfg.detritus.initial_dark = 0.0;
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.fall = 0.0;
    World::new(cfg).expect("a quiet world is valid")
}

fn trial(world: &World) -> FixedHunterProfile {
    FixedHunterProfile::lanternjaw_trial(world.config())
}

/// A profile whose capture roll always succeeds, so the settlement path is deterministic.
fn certain(mut p: FixedHunterProfile) -> FixedHunterProfile {
    p.capture_min = 1.0;
    p.capture_max = 1.0;
    p
}

/// A profile whose capture roll never succeeds: a paid, failed attempt.
fn never(mut p: FixedHunterProfile) -> FixedHunterProfile {
    p.capture_min = 0.0;
    p.capture_max = 0.0;
    p
}

fn target_of(pos: SurfacePoint) -> HunterTarget {
    HunterTarget { face: pos.face.index() as u8, u: pos.u, v: pos.v }
}

/// Place a prey by hand and book its material as admitted from outside, the way the redesign
/// fixtures do, so the closed box still reads zero.
///
/// `frozen` zeroes its top speed so the geometry of a test stays where the test put it; its
/// escape response is then visible as a heading change rather than as motion.
fn place_prey(world: &mut World, pos: SurfacePoint, s: f64, r: f64, e: f64, frozen: bool) -> OrganismId {
    let cfg = world.config().clone();
    let mut genome = Genome::founder(0.5, &cfg.drives);
    genome.size = 0.5;
    genome.speed = 0.3;
    genome.clamp();
    let mut phenotype = decode(&genome, &cfg.organism);
    if frozen {
        phenotype.speed_max = 0.0;
        // Also adult on arrival, so it neither grows into nor out of the eligibility window
        // while the test is watching.
        phenotype.structure_adult = s;
    }
    let id = world.state.organisms.insert(Organism {
        pos: pos.canonicalize(),
        heading: Vec2::new(1.0, 0.0),
        ou: Vec2::ZERO,
        structure: s,
        reserve: r,
        energy: e,
        born_tick: world.tick(),
        hunger_memory: 0.0,
        mode: Mode::Resting,
        escrow: None,
        births: 0,
        genome,
        phenotype,
        parent: None,
        origin: Origin::Founder,
        turn_counter: Counter::default(),
        fed_this_tick: false,
    });
    world.state.external_material_in += s + r;
    id
}

/// Point a hunter at a target and make it hungry enough to hunt, booking the reserve it gave
/// up so the material box stays closed.
fn aim(world: &mut World, id: OrganismId, heading: Vec2, reserve: f64) {
    let o = world.state.organisms.get_mut(id).expect("alive");
    let before = o.reserve;
    o.heading = heading;
    o.reserve = reserve;
    world.state.external_material_in += reserve - before;
}

/// The spec's stored energy, recomputed here, **including** what the hunters are carrying.
fn stored_energy(state: &WorldState) -> f64 {
    let e_p = state.config.producer.energy_density;
    let e_f = state.config.fruit.energy_density;
    let e_r = state.config.organism.reserve_energy_density;
    let cells: f64 = state.fields.p.iter().map(|p| e_p * p).sum::<f64>()
        + state.fields.f.iter().map(|f| e_f * f).sum::<f64>()
        + state.fields.de.iter().sum::<f64>();
    let organisms: f64 = state
        .organisms
        .iter()
        .map(|(_, o)| {
            o.energy
                + e_r * o.reserve
                + o.escrow.as_ref().map_or(0.0, |e| e_r * (e.structure + e.reserve) + e.energy)
        })
        .sum();
    cells + organisms + state.hunters.gut_energy_total()
}

/// The closed box, **including** carried carcasses.
fn total_material(state: &WorldState) -> f64 {
    let cells: f64 = state.fields.n.iter().sum::<f64>()
        + state.fields.p.iter().sum::<f64>()
        + state.fields.d.iter().sum::<f64>()
        + state.fields.f.iter().sum::<f64>();
    let organisms: f64 = state.organisms.iter().map(|(_, o)| o.material()).sum();
    cells + organisms + state.hunters.gut_material_total()
}

/// The chart offset of a body-local offset for a hunter with this heading, in the same basis
/// `stamp_rig` uses: `+x` along the heading, `+y` its clockwise side. Transcribed here rather
/// than imported, so a test places prey where the *renderer* would draw the claws.
fn body_offset(heading: Vec2, offset: Vec2, scale: f64) -> Vec2 {
    let h = heading.normalized().expect("a heading");
    let side = Vec2::new(-h.y, h.x);
    (h * offset.x + side * offset.y) * scale
}

/// The surface point of a hunter's capture effector, swept the way any offset is swept.
fn effector_point(root: SurfacePoint, heading: Vec2, profile: &FixedHunterProfile, scale: f64) -> SurfacePoint {
    travel(root, body_offset(heading, profile.capture_offset_body, scale)).end
}

/// A hunter and one frozen prey exactly inside its claws, both hungry enough to act.
fn staged(profile: FixedHunterProfile) -> (World, OrganismId, OrganismId) {
    staged_in(empty_world(), profile)
}

/// The same staging in a world the caller chose.
fn staged_in(mut world: World, profile: FixedHunterProfile) -> (World, OrganismId, OrganismId) {
    let spot = SurfacePoint::new(Face::Front, 20.0, 32.0);
    let receipt = world.start_hunter_trial(profile.clone(), target_of(spot)).expect("the trial starts");
    let hunter = receipt.id;
    aim(&mut world, hunter, Vec2::new(1.0, 0.0), 1.0);
    // In the grasp, not in front of the thorax: an adult's claws close 13.28 px ahead and
    // 1.16 px to its clockwise side.
    let grasp = effector_point(spot, Vec2::new(1.0, 0.0), &profile, 1.0);
    let prey = place_prey(&mut world, grasp, 0.5, 0.3, 0.4, true);
    (world, hunter, prey)
}

/// Step until a predicate holds, or panic with what the hunter was doing.
fn run_until(world: &mut World, ticks: u64, mut done: impl FnMut(&World) -> bool) -> u64 {
    for t in 1..=ticks {
        world.step();
        if done(world) {
            return t;
        }
    }
    panic!(
        "nothing happened in {ticks} ticks: phases {:?}",
        world.hunters().members.iter().map(|m| (m.phase, m.target, m.gut_material)).collect::<Vec<_>>()
    );
}

/// Step until a hunter record the caller is looking for appears, draining as it goes.
fn run_for_events(world: &mut World, ticks: u64, mut done: impl FnMut(&[HunterEvent]) -> bool) -> Vec<HunterEvent> {
    let mut seen = Vec::new();
    for _ in 1..=ticks {
        world.step();
        seen.extend(world.drain_hunter_events());
        if done(&seen) {
            return seen;
        }
    }
    panic!("no such hunter record in {ticks} ticks: {seen:?}")
}

/// A second member of the same lineage, placed by hand the way the initializer places the
/// first one — same derived inventory, booked in the same ledger, member list kept sorted.
fn add_hunter(world: &mut World, profile: &FixedHunterProfile, pos: SurfacePoint, heading: Vec2) -> OrganismId {
    let cfg = world.config().clone();
    let mut phenotype = decode(&profile.genome, &cfg.organism);
    phenotype.extent = profile.body_extent_px;
    let structure = phenotype.structure_adult;
    let reserve = profile.founder_reserve_fraction * phenotype.reserve_max;
    let energy = profile.founder_energy_fraction * phenotype.energy_max;
    let reserve_max = phenotype.reserve_max;
    let tick = world.tick();
    let id = world.state.organisms.insert(Organism {
        pos: pos.canonicalize(),
        heading,
        ou: Vec2::ZERO,
        structure,
        reserve,
        energy,
        born_tick: tick,
        hunger_memory: (1.0 - reserve / reserve_max).clamp(0.0, 1.0),
        mode: Mode::Resting,
        escrow: None,
        births: 0,
        genome: profile.genome.clone(),
        phenotype,
        parent: None,
        origin: Origin::Founder,
        turn_counter: Counter::default(),
        fed_this_tick: false,
    });
    world.state.hunters.members.push(HunterMember::new(id, tick));
    world.state.hunters.members.sort_by_key(|m| m.id);
    world.state.hunters.founder_material_in += structure + reserve;
    world.state.hunters.founder_energy_in += energy + cfg.organism.reserve_energy_density * reserve;
    world.state.hunters.founders_placed += 1;
    id
}

/// A world, a hunter and a prey whose relationship the test decides.
fn phase_of(world: &World, id: OrganismId) -> HunterPhase {
    world.hunters().member(id).expect("a member").phase
}

/// Checkpoint here, reload, and require the resumed world to stay bit-identical for `ticks`.
fn restart_matches(world: &mut World, ticks: u64, what: &str) {
    let bytes = encode_snapshot(&world.state, "hunter-restart");
    let (meta, state) = decode_snapshot(&bytes).unwrap_or_else(|e| panic!("{what}: {e:?}"));
    assert_eq!(meta.schema, cubarium_core::SCHEMA_VERSION);
    assert_eq!(state.hunters, world.state.hunters, "{what}: the extension did not round-trip");
    let mut reloaded = World::from_state(state).unwrap_or_else(|e| panic!("{what}: {e}"));
    assert_eq!(state_hash(&reloaded.state), state_hash(&world.state), "{what}: the load differs");
    for _ in 0..ticks {
        world.step();
        reloaded.step();
        assert_eq!(
            state_hash(&reloaded.state),
            state_hash(&world.state),
            "{what}: diverged at tick {}",
            world.tick()
        );
    }
    assert_eq!(reloaded.hunters(), world.hunters(), "{what}: the extensions diverged");
}

// ---------------------------------------------------------------- founding

#[test]
fn the_founder_inventory_is_derived_from_the_config_and_booked_once() {
    let mut world = empty_world();
    let profile = trial(&world);
    let before_material = total_material(&world.state);
    let before_energy = stored_energy(&world.state);
    let before_external = world.state.external_material_in;

    let spot = SurfacePoint::new(Face::Top, 32.0, 32.0);
    let r = world.start_hunter_trial(profile.clone(), target_of(spot)).expect("the trial starts");

    // The plan's derived founder at the default config: S = 2, R = 2, E = 3, so 4 material
    // and 7 energy — read off the world's own decode, not hardcoded.
    let phenotype = decode(&profile.genome, &world.config().organism);
    assert_eq!(r.structure, phenotype.structure_adult);
    assert_eq!(r.reserve, profile.founder_reserve_fraction * phenotype.reserve_max);
    assert_eq!(r.energy, profile.founder_energy_fraction * phenotype.energy_max);
    assert_eq!((r.structure, r.reserve, r.energy), (2.0, 2.0, 3.0));
    assert_eq!(r.material_in, 4.0);
    assert_eq!(r.energy_in, 7.0);
    assert_eq!(r.extent, profile.body_extent_px, "the assembled body's tested support");
    // Sensing is reconciled with the capture effector: the claws close about 14.8 px from the
    // root, so the profile senses at the genome's maximum and pays for it.
    assert_eq!(r.sense_radius, 12.0);
    assert!(
        r.sense_radius + r.extent > profile.capture_offset_body.length() + profile.capture_reach_px,
        "a hunter must be able to sense what it could grasp"
    );

    // Booked once, in the extension, and never in `external_material_in`.
    assert_eq!(world.hunters().founder_material_in, 4.0);
    assert_eq!(world.hunters().founder_energy_in, 7.0);
    assert_eq!(world.state.external_material_in, before_external, "the old ledger is untouched");
    assert_eq!(world.hunters().founders_placed, 1);
    assert_eq!(world.hunters().members.len(), 1);
    assert_eq!(world.hunters().members[0].id, r.id);
    assert_eq!(world.hunters().members[0].phase, HunterPhase::Perched);
    assert!(!world.hunters().members[0].carrying());

    // The closed box moved by exactly the import, and so did the stored energy.
    assert!((total_material(&world.state) - before_material - 4.0).abs() < 1e-12);
    assert!((stored_energy(&world.state) - before_energy - 7.0).abs() < 1e-12);
    assert!(world.mass_residual().abs() < 1e-12, "residual {}", world.mass_residual());
    world.check_invariants().expect("a founded world is consistent");

    // One hunter in the view, with its scaled contact geometry and semantic role.
    let view = world.hunter_view();
    assert_eq!(view.len(), 1);
    assert_eq!(view[0].id, r.id);
    assert_eq!(view[0].role, cubarium_core::HunterRole::Lanternjaw);
    assert_eq!(view[0].body_scale, 1.0, "a founder is an adult at scale 1");
    assert_eq!(view[0].geometry.capture_offset_body, profile.capture_offset_body);
    assert_eq!(view[0].geometry.capture_reach_px, profile.capture_reach_px);
    assert_eq!(view[0].geometry.ingestion_offset_body, profile.ingestion_offset_body);
    assert_eq!(view[0].geometry, r.geometry, "the receipt published the same geometry");
    assert_eq!(view[0].phase_started_tick, world.tick());
    assert_eq!(view[0].phase_ends_tick, world.tick());
    assert_eq!(view[0].entered_from, HunterPhase::Perched);
    assert_eq!(view[0].episode, 0);
    assert_eq!(view[0].attack_counter, 0);
    assert_eq!(view[0].structure_adult, phenotype.structure_adult);
    assert_eq!(view[0].gut_capacity, profile.gut_capacity_material);
    // On the open top face both anchors map honestly.
    assert!(view[0].capture_center.is_some() && view[0].ingestion_center.is_some());
    assert!(!view[0].juvenile);
}

#[test]
fn the_trial_refuses_a_repeat_an_invalid_profile_and_an_impossible_target() {
    let mut world = empty_world();
    let profile = trial(&world);
    let spot = SurfacePoint::new(Face::Top, 32.0, 32.0);

    let mut bad = profile.clone();
    bad.version = 99;
    assert!(world.start_hunter_trial(bad, target_of(spot)).is_err());
    assert_eq!(world.hunters().members.len(), 0, "a refusal changes nothing");
    assert!(world.hunters().profile.is_none());

    let mut fat = profile.clone();
    fat.body_extent_px = world.config().organism.body_extent_max + 1.0;
    let err = world.start_hunter_trial(fat, target_of(spot)).expect_err("too big to place");
    assert!(err.contains("body extent"), "{err}");

    assert!(
        world
            .start_hunter_trial(profile.clone(), HunterTarget { face: 9, u: 0.0, v: 0.0 })
            .is_err(),
        "an unresolvable target is refused"
    );
    assert!(world.hunters().profile.is_none(), "still nothing happened");

    world.start_hunter_trial(profile.clone(), target_of(spot)).expect("the first one works");
    let err = world.start_hunter_trial(profile, target_of(spot)).expect_err("the second must not");
    assert!(err.contains("already initialized"), "{err}");
    assert_eq!(world.hunters().members.len(), 1);
    assert_eq!(world.hunters().founder_material_in, 4.0, "and nothing was booked twice");
}

#[test]
fn the_budget_matched_control_deposits_the_same_inventory_and_no_hunter() {
    let mut world = empty_world();
    let profile = trial(&world);
    let spot = SurfacePoint::new(Face::Top, 32.0, 32.0);
    let before_material = total_material(&world.state);
    let before_energy = stored_energy(&world.state);
    let before_heat = world.state.heat_out_corrected();

    let r = world.deposit_hunter_budget_control(profile.clone(), target_of(spot)).expect("deposited");
    assert_eq!(r.material_in, 4.0, "the same derived founder material");
    assert_eq!(r.energy_in, 7.0, "the same derived founder energy");
    // The default detritus cap (2) holds all seven units of it here.
    assert_eq!(r.energy_stored, 7.0);
    assert_eq!(r.energy_heat, 0.0);
    assert!(world.hunters().members.is_empty(), "a control arm has no predator");
    assert!(world.hunters().control_deposited);
    assert_eq!(world.hunters().imported_material(), 4.0);
    assert_eq!(world.hunters().imported_energy(), 7.0);

    assert!((total_material(&world.state) - before_material - 4.0).abs() < 1e-12);
    assert!((stored_energy(&world.state) - before_energy - 7.0).abs() < 1e-12);
    assert_eq!(world.state.heat_out_corrected(), before_heat, "nothing spilled");
    assert!(world.mass_residual().abs() < 1e-12);

    // The arms are mutually exclusive, and neither repeats.
    assert!(world.deposit_hunter_budget_control(profile.clone(), target_of(spot)).is_err());
    let err = world.start_hunter_trial(profile, target_of(spot)).expect_err("no hunter here");
    assert!(err.contains("control"), "{err}");
}

#[test]
fn a_control_deposit_over_a_full_cell_turns_the_excess_into_real_heat() {
    let mut world = empty_world();
    let profile = trial(&world);
    let spot = SurfacePoint::new(Face::Top, 32.0, 32.0);
    // A cell whose detritus energy is already at the cap has no room for seven more units.
    let cell = cubarium_surface::cell_of(&spot).index();
    world.state.fields.d[cell] = 1.0;
    world.state.fields.de[cell] = 2.0;
    world.state.external_material_in += 1.0;

    let before_energy = stored_energy(&world.state);
    let before_heat = world.state.heat_out_corrected();
    let r = world.deposit_hunter_budget_control(profile, target_of(spot)).expect("deposited");
    assert_eq!(r.energy_in, 7.0);
    // Room is `cap · (D + 4) − De = 2 · 5 − 2 = 8`, so all seven still fit here.
    assert_eq!(r.energy_stored, 7.0);
    assert_eq!(r.energy_heat, 0.0);

    // Now a cap that cannot hold it: the excess is heat, and the identity still closes.
    let mut world = empty_world();
    world.state.config.detritus.energy_cap = 0.5;
    let profile = trial(&world);
    let before_energy2 = stored_energy(&world.state);
    let before_heat2 = world.state.heat_out_corrected();
    let r = world.deposit_hunter_budget_control(profile, target_of(spot)).expect("deposited");
    assert_eq!(r.energy_stored, 0.5 * 4.0, "the cap admits `energy_cap · material`");
    assert_eq!(r.energy_heat, 7.0 - 2.0);
    let stored_delta = stored_energy(&world.state) - before_energy2;
    let heat_delta = world.state.heat_out_corrected() - before_heat2;
    assert!((stored_delta + heat_delta - 7.0).abs() < 1e-12, "stored {stored_delta} heat {heat_delta}");
    let _ = (before_energy, before_heat);
}

// ---------------------------------------------------------------- the hunt

#[test]
fn a_hunt_runs_through_its_phases_and_pays_for_the_strike() {
    let profile = certain(trial(&empty_world()));
    let (mut world, hunter, prey) = staged(profile.clone());

    // It starts perched, then stalks the only eligible prey it senses.
    run_until(&mut world, 40, |w| w.hunters().members[0].phase == HunterPhase::Stalking);
    assert_eq!(world.hunters().members[0].target, Some(prey));

    // The jaw is already in reach, so the windup follows immediately and grants no capture.
    run_until(&mut world, 20, |w| w.hunters().members[0].phase == HunterPhase::Windup);
    assert!(world.state.organisms.get(prey).is_some(), "a windup never captures");
    assert_eq!(world.hunters().attacks_total, 0, "and it is not an attempt yet");

    // The strike is charged in full at entry, before any outcome is known.
    let energy_before = world.state.organisms.get(hunter).expect("alive").energy;
    run_until(&mut world, 40, |w| w.hunters().members[0].phase == HunterPhase::Strike);
    let spent = energy_before - world.state.organisms.get(hunter).expect("alive").energy;
    assert!(spent >= profile.strike_energy_cost, "the strike cost was not charged: {spent}");
    assert_eq!(world.hunters().attacks_total, 1);
    assert_eq!(world.hunters().members[0].attack_counter, 1);

    // And the attempt resolves after both creatures have moved.
    run_until(&mut world, 60, |w| w.hunters().captures_total == 1);
    assert!(world.state.organisms.get(prey).is_none(), "the prey was consumed");
    assert_eq!(world.hunters().members[0].phase, HunterPhase::Handling);
    assert!(world.hunters().members[0].carrying());
    world.check_invariants().expect("a world that just ate is consistent");
}

#[test]
fn a_capture_moves_the_whole_prey_into_the_gut_and_conserves_everything() {
    let profile = certain(trial(&empty_world()));
    let (mut world, hunter, prey) = staged(profile.clone());
    // Give the prey an escrow: a captured parent's gestation goes with it, once.
    {
        let cfg = world.config().clone();
        let o = world.state.organisms.get_mut(prey).expect("alive");
        o.escrow = Some(cubarium_core::organism::Escrow {
            structure: 0.05,
            reserve: 0.05,
            energy: 0.02,
            started_tick: 0,
            genome: Genome::founder(0.5, &cfg.drives),
        });
        world.state.external_material_in += 0.1;
    }
    let e_r = world.config().organism.reserve_energy_density;
    let inventory = |w: &World| -> (f64, f64) {
        let o = w.state.organisms.get(prey).expect("alive");
        let escrow = o.escrow.as_ref().expect("the escrow is held");
        (
            o.structure + o.reserve + escrow.structure + escrow.reserve,
            o.energy + e_r * o.reserve + escrow.energy + e_r * (escrow.structure + escrow.reserve),
        )
    };
    let material_before = total_material(&world.state);
    let energy_before = stored_energy(&world.state);
    let ledgers_before = world.energy_ledgers();
    let detritus_before: f64 = world.state.fields.d.iter().sum();

    // The prey is alive and metabolizing until the moment it is taken, so the inventory the
    // capture must transfer is the one it had entering the tick it died on.
    let mut expected = inventory(&world);
    for _ in 0..200 {
        world.step();
        if world.hunters().captures_total == 1 {
            break;
        }
        expected = inventory(&world);
    }
    assert_eq!(world.hunters().captures_total, 1, "no capture in 200 ticks");
    let (expected_material, expected_energy) = expected;

    let member = world.hunters().members[0];
    // Handling begins on the capture tick, so by the end of it one digestion portion has
    // already left the gut: what the transfer was is the capture record, checked below, and
    // the gut is that inventory less at most `digest_rate · dt`.
    let portion = profile.digest_rate * cubarium_core::DT;
    assert!(
        member.gut_material <= expected_material + 1e-12
            && member.gut_material >= expected_material - portion - 1e-12,
        "gut {} vs the prey's last inventory {expected_material}",
        member.gut_material
    );
    assert!(member.gut_energy > 0.0 && member.gut_energy < expected_energy + 1e-12);
    // The body did not also become detritus: it is carried, not dropped.
    let detritus_after: f64 = world.state.fields.d.iter().sum();
    assert!(detritus_after - detritus_before < expected_material * 0.5, "the corpse was double-counted");

    // Material is closed across the capture, and the energy identity still holds: everything
    // stored moved by light in minus heat out, with the capture itself an internal transfer.
    let material_after = total_material(&world.state);
    assert!((material_after - material_before).abs() < 1e-9, "material moved by {}", material_after - material_before);
    let booked = world.energy_ledgers().net_since(ledgers_before);
    let moved = stored_energy(&world.state) - energy_before;
    assert!((moved - booked).abs() < 1e-9, "energy identity broke by {}", moved - booked);

    // Exactly one death event, with the appended cause, and one capture record.
    let deaths: Vec<_> = world
        .drain_events()
        .into_iter()
        .filter(|e| matches!(e, LifeEvent::Death { id, .. } if *id == prey))
        .collect();
    assert_eq!(deaths.len(), 1, "{deaths:?}");
    assert!(matches!(deaths[0], LifeEvent::Death { cause: DeathCause::Predation, .. }));
    assert_eq!(world.state.deaths_total, [0, 0, 0], "the three natural counters are untouched");
    assert_eq!(world.hunters().predation_deaths_total, 1);
    let captures: Vec<_> = world
        .drain_hunter_events()
        .into_iter()
        .filter(|e| matches!(e, HunterEvent::Capture { .. }))
        .collect();
    assert_eq!(captures.len(), 1);
    match captures[0] {
        HunterEvent::Capture { hunter: h, prey: p, material, energy, .. } => {
            assert_eq!((h, p), (hunter, prey));
            // The record is the transfer itself: material cannot change between the start of
            // a tick and the settlement (only energy is spent moving), so this is exact.
            assert!((material - expected_material).abs() < 1e-12, "{material} vs {expected_material}");
            assert!(
                energy <= expected_energy && energy > expected_energy - 0.01,
                "{energy} vs {expected_energy}"
            );
        }
        ref other => panic!("{other:?}"),
    }
}

#[test]
fn a_failed_attempt_still_pays_and_the_prey_survives() {
    let profile = never(trial(&empty_world()));
    let (mut world, hunter, prey) = staged(profile.clone());
    let energy_before = world.state.organisms.get(hunter).expect("alive").energy;

    let seen = run_for_events(&mut world, 200, |seen| {
        seen.iter().any(|e| matches!(e, HunterEvent::Attempt { outcome: AttemptOutcome::Missed, .. }))
    });
    let missed = seen
        .iter()
        .find(|e| matches!(e, HunterEvent::Attempt { outcome: AttemptOutcome::Missed, .. }))
        .expect("the miss is in the log");
    match missed {
        HunterEvent::Attempt { hunter: h, target, energy_paid, .. } => {
            assert_eq!(*h, hunter);
            assert_eq!(*target, Some(prey));
            assert_eq!(*energy_paid, profile.strike_energy_cost, "the record names what it cost");
        }
        other => panic!("{other:?}"),
    }

    assert!(world.state.organisms.get(prey).is_some(), "a missed strike does not kill");
    assert_eq!(world.hunters().captures_total, 0);
    assert_eq!(world.hunters().attacks_total, 1, "it was still a paid attempt");
    let spent = energy_before - world.state.organisms.get(hunter).expect("alive").energy;
    assert!(spent >= profile.strike_energy_cost, "the miss was free: {spent}");
    assert_eq!(world.hunters().members[0].phase, HunterPhase::Recovering);
    assert_eq!(world.hunters().members[0].target, None);
}

#[test]
fn prey_outside_the_window_or_too_big_for_the_gut_is_never_attempted() {
    // Too large for the eligibility window (`0.75 · S_hunter`), too small to be worth a
    // strike, and a body that does not fit the remaining gut: none of them is stalked.
    // `0.12` is above the world's own collapse threshold and below the profile's
    // `prey_structure_min`, so "too small" means ineligible, not dying of its own accord.
    for (structure, capacity, what) in
        [(1.8, 4.0, "too big"), (0.12, 4.0, "too small"), (0.5, 0.5, "does not fit the gut")]
    {
        let mut profile = certain(trial(&empty_world()));
        profile.gut_capacity_material = capacity;
        let mut world = empty_world();
        let spot = SurfacePoint::new(Face::Front, 20.0, 32.0);
        let hunter =
            world.start_hunter_trial(profile.clone(), target_of(spot)).expect("the trial starts").id;
        aim(&mut world, hunter, Vec2::new(1.0, 0.0), 1.0);
        let grasp = effector_point(spot, Vec2::new(1.0, 0.0), &profile, 1.0);
        let prey = place_prey(&mut world, grasp, structure, 0.3, 0.4, true);

        for _ in 0..200 {
            world.step();
        }
        assert_eq!(phase_of(&world, hunter), HunterPhase::Perched, "{what}: it hunted anyway");
        assert_eq!(world.hunters().attacks_total, 0, "{what}: a paid attempt was made");
        assert!(world.state.organisms.get(prey).is_some(), "{what}: the prey is gone");
        assert!(!world.hunters().members[0].carrying());
    }
}

#[test]
fn attacks_disabled_keeps_the_same_living_hunter_and_never_attempts_a_capture() {
    let profile = certain(trial(&empty_world())).without_attacks();
    let (mut world, hunter, prey) = staged(profile);
    let energy_before = world.state.organisms.get(hunter).expect("alive").energy;

    for _ in 0..300 {
        world.step();
    }
    assert_eq!(world.hunters().attacks_total, 0);
    assert_eq!(world.hunters().captures_total, 0);
    assert_eq!(phase_of(&world, hunter), HunterPhase::Perched);
    assert!(world.state.organisms.get(prey).is_some(), "the control arm eats nobody");
    // It is still a living animal: maintenance and sensing were paid the whole time.
    let now = world.state.organisms.get(hunter).expect("alive").energy;
    assert!(now < energy_before, "a maintained hunter must still spend: {now} vs {energy_before}");
    assert!(world.drain_hunter_events().is_empty(), "and it logs no attempts");
}

#[test]
fn two_hunters_contesting_one_prey_produce_exactly_one_capture_and_two_paid_attempts() {
    let profile = certain(trial(&empty_world()));
    let mut world = empty_world();
    // Two hunters facing each other with the prey between them, in both grasps at once, so
    // both reach the same phase on the same tick and contest the same body.
    let left = SurfacePoint::new(Face::Front, 20.0, 32.0);
    let reach = profile.capture_offset_body;
    // Mirrored about the prey: the second hunter faces −x, so its clockwise side is −y, and
    // the same body offset lands on the same point.
    let right = SurfacePoint::new(Face::Front, 20.0 + 2.0 * reach.x, 32.0);
    let a = world.start_hunter_trial(profile.clone(), target_of(left)).expect("started").id;
    let b = add_hunter(&mut world, &profile, right, Vec2::new(-1.0, 0.0));
    aim(&mut world, a, Vec2::new(1.0, 0.0), 1.0);
    aim(&mut world, b, Vec2::new(-1.0, 0.0), 1.0);
    let middle = effector_point(left, Vec2::new(1.0, 0.0), &profile, 1.0);
    let prey = place_prey(&mut world, middle, 0.5, 0.3, 0.4, true);

    let material_before = total_material(&world.state);
    let seen = run_for_events(&mut world, 400, |seen| {
        seen.iter().filter(|e| matches!(e, HunterEvent::Attempt { .. })).count() >= 2
    });

    let attempts: Vec<_> = seen.iter().filter(|e| matches!(e, HunterEvent::Attempt { .. })).collect();
    assert_eq!(attempts.len(), 2, "{attempts:?}");
    let outcomes: Vec<AttemptOutcome> = attempts
        .iter()
        .map(|e| match e {
            HunterEvent::Attempt { outcome, .. } => *outcome,
            other => panic!("{other:?}"),
        })
        .collect();
    assert!(outcomes.contains(&AttemptOutcome::Captured), "nobody caught it: {outcomes:?}");
    assert!(
        outcomes.contains(&AttemptOutcome::TargetClaimed),
        "the loser must be told its prey was claimed: {outcomes:?}"
    );
    // One capture, one death, one carried body: the prey is never harvested twice.
    assert_eq!(world.hunters().captures_total, 1);
    assert_eq!(world.hunters().predation_deaths_total, 1);
    assert!(world.state.organisms.get(prey).is_none());
    let carrying: Vec<_> = world.hunters().members.iter().filter(|m| m.carrying()).collect();
    assert_eq!(carrying.len(), 1, "two hunters carrying one body: {carrying:?}");
    // Both paid: two attempts were counted and both hunters spent their strike cost.
    assert_eq!(world.hunters().attacks_total, 2);
    assert!((total_material(&world.state) - material_before).abs() < 1e-9);
    let _ = (a, b);
    world.check_invariants().expect("consistent after a contested capture");
}

#[test]
fn a_hunter_that_dies_hands_its_gut_to_the_cell_and_leaves_the_member_list() {
    let profile = certain(trial(&empty_world()));
    let (mut world, hunter, _prey) = staged(profile);
    run_until(&mut world, 200, |w| w.hunters().captures_total == 1);
    let member = world.hunters().members[0];
    assert!(member.carrying());

    // Starve it: no energy and no reserve is the world's own starvation rule.
    {
        let o = world.state.organisms.get_mut(hunter).expect("alive");
        let material = o.reserve;
        o.energy = 0.0;
        o.reserve = 0.0;
        world.state.external_material_in -= material;
    }
    // Measured over the whole surface: a resting hunter may still cross a cell boundary
    // before it dies, and the litter it leaves decomposes from the next tick on.
    let d_before: f64 = world.state.fields.d.iter().sum();
    let de_before: f64 = world.state.fields.de.iter().sum();
    let material_before = total_material(&world.state);
    let ledgers_before = world.energy_ledgers();
    let energy_before = stored_energy(&world.state);
    let gut = (member.gut_material, member.gut_energy);

    let seen = run_for_events(&mut world, 40, |seen| {
        seen.iter().any(|e| matches!(e, HunterEvent::Death { .. }))
    });

    assert!(world.state.organisms.get(hunter).is_none(), "it died");
    assert!(world.hunters().members.is_empty(), "and left the member list");
    assert_eq!(world.hunters().hunter_deaths_total, 1);
    assert!(world.hunter_view().is_empty());
    match seen.iter().find(|e| matches!(e, HunterEvent::Death { .. })).expect("the record") {
        HunterEvent::Death { id, cause, gut_material, gut_energy, gut_energy_stored, .. } => {
            assert_eq!(*id, hunter);
            assert_eq!(*cause, DeathCause::Starvation);
            assert!((gut_material - gut.0).abs() < 1e-12);
            assert!((gut_energy - gut.1).abs() < 1e-12);
            // The detritus cap applies to what it was carrying, exactly as it does to a body.
            let cap = world.config().detritus.energy_cap;
            assert!(*gut_energy_stored <= cap * gut_material + 1e-12);
        }
        other => panic!("{other:?}"),
    }
    // The carcass it was carrying landed with its body, and nothing vanished: the structure it
    // stood on plus the meal it held are both in the litter now.
    let d_after: f64 = world.state.fields.d.iter().sum();
    let de_after: f64 = world.state.fields.de.iter().sum();
    assert!(d_after > d_before + gut.0, "the gut did not land: {d_after} vs {d_before} + {}", gut.0);
    assert!(d_after > d_before + 2.0, "the body did not land either");
    assert!(de_after > de_before, "the meal carried energy into the litter");
    assert!((total_material(&world.state) - material_before).abs() < 1e-9);
    let booked = world.energy_ledgers().net_since(ledgers_before);
    let moved = stored_energy(&world.state) - energy_before;
    assert!((moved - booked).abs() < 1e-9, "energy identity broke by {}", moved - booked);
    world.check_invariants().expect("consistent after a hunter's death");
}

#[test]
fn the_grasp_reaches_across_a_seam_where_the_body_is_not() {
    let profile = certain(trial(&empty_world()));
    let mut world = empty_world();
    // Close enough to the edge that the claws land on the next face.
    let spot = SurfacePoint::new(Face::Front, 55.0, 32.0);
    let hunter = world.start_hunter_trial(profile.clone(), target_of(spot)).expect("started").id;
    aim(&mut world, hunter, Vec2::new(1.0, 0.0), 1.0);
    let grasp = effector_point(spot, Vec2::new(1.0, 0.0), &profile, 1.0);
    assert_ne!(grasp.face, spot.face, "this fixture needs the grasp to cross a seam");
    let prey = place_prey(&mut world, grasp, 0.5, 0.3, 0.4, true);
    assert_ne!(
        world.state.organisms.get(prey).expect("alive").pos.face,
        world.state.organisms.get(hunter).expect("alive").pos.face,
        "the prey must be on the other face"
    );

    run_until(&mut world, 300, |w| w.hunters().captures_total == 1);
    assert!(world.state.organisms.get(prey).is_none(), "contact across the seam failed");
    // And the published anchor is the transported, round-tripped point on the other face.
    let view = world.hunter_view();
    assert_eq!(view.len(), 1);
    let centre = view[0].capture_center.expect("an ordinary seam maps honestly");
    assert_ne!(centre.face, view[0].pos.face);
}

/// **Regression.** An earlier build swept the contact anchor with `travel` and used wherever it
/// landed, so a reach aimed past the open rim folded back onto the world and killed a prey
/// behind the hunter. Static artwork clips at the rim; only actual root travel reflects. The
/// grasp centre must now refuse to map, no capture may happen there, and the view must publish
/// `None` rather than a fabricated point.
#[test]
fn a_grasp_past_the_open_rim_never_captures_and_is_never_published() {
    let profile = certain(trial(&empty_world()));
    let mut world = empty_world();
    // The integration report's own fixture, scaled to the real effector: a hunter near the
    // open rim of a side face, aimed at it.
    let spot = SurfacePoint::new(Face::Front, 32.0, 63.0);
    let heading = Vec2::new(0.0, 1.0);
    let sweep = travel(spot, body_offset(heading, profile.capture_offset_body, 1.0));
    assert!(sweep.reflections > 0, "this fixture needs the reach to meet the rim");
    assert_eq!(sweep.end.face, spot.face, "and to fold back onto the same face");

    let hunter = world.start_hunter_trial(profile.clone(), target_of(spot)).expect("started").id;
    aim(&mut world, hunter, heading, 1.0);
    // Exactly where the old build would have grasped: the reflected point.
    let prey = place_prey(&mut world, sweep.end, 0.5, 0.3, 0.4, true);

    // Nothing is published there, because nothing is drawn there.
    let view = world.hunter_view();
    assert_eq!(view.len(), 1);
    assert_eq!(view[0].capture_center, None, "a reflected grasp centre must not be published");

    for _ in 0..400 {
        world.step();
    }
    assert_eq!(world.hunters().captures_total, 0, "a reflected reach captured something");
    assert!(world.state.organisms.get(prey).is_some(), "the prey behind the rim was eaten");
    world.check_invariants().expect("consistent after a refused off-rim reach");
}

// ---------------------------------------------------------------- escape

#[test]
fn threatened_prey_turns_away_and_may_briefly_outrun_its_own_maximum() {
    let profile = certain(trial(&empty_world()));
    let mut world = empty_world();
    let spot = SurfacePoint::new(Face::Front, 20.0, 32.0);
    let hunter = world.start_hunter_trial(profile.clone(), target_of(spot)).expect("started").id;
    aim(&mut world, hunter, Vec2::new(1.0, 0.0), 1.0);
    // Far enough to be sensed and stalked, close enough to stay sensed: no contact yet.
    let prey_spot = travel(spot, Vec2::new(1.0, 0.0) * 10.0).end;
    let prey = place_prey(&mut world, prey_spot, 0.5, 0.3, 0.6, false);
    let speed_max = world.state.organisms.get(prey).expect("alive").phenotype.speed_max;

    let mut outran = false;
    let mut turned = false;
    let mut previous = world.state.organisms.get(prey).expect("alive").pos;
    for _ in 0..200 {
        world.step();
        let Some(o) = world.state.organisms.get(prey) else { break };
        if phase_of(&world, hunter) == HunterPhase::Stalking && o.pos.face == previous.face {
            let step = (o.pos.chart() - previous.chart()).length();
            if step > speed_max * cubarium_core::DT + 1e-12 {
                outran = true;
            }
            // Away from the hunter is +u here: the hunter is behind it.
            if o.heading.x > 0.5 {
                turned = true;
            }
        }
        previous = o.pos;
    }
    assert!(turned, "a stalked prey never turned away from its pursuer");
    assert!(outran, "a stalked prey never exceeded its own maximum speed");
}

/// The dash and the burst are both bounded by the energy the creature has before it moves
/// (`MoveBill::affordable_speed`, unit-tested in `hunter.rs`). What this checks is the other
/// half of the rule: a hunter that cannot afford the **whole** strike does not attempt one,
/// and that refusal costs it nothing, consumes no draw and is not counted as an attack.
#[test]
fn an_unaffordable_strike_is_refused_before_payment() {
    let profile = certain(trial(&empty_world()));
    let (mut world, hunter, prey) = staged(profile.clone());
    {
        // Enough to stalk and wind up, never enough for the strike itself.
        let o = world.state.organisms.get_mut(hunter).expect("alive");
        o.energy = profile.strike_energy_cost * 0.5;
    }
    let energy_before = world.state.organisms.get(hunter).expect("alive").energy;

    let seen = run_for_events(&mut world, 200, |seen| {
        seen.iter().any(|e| matches!(e, HunterEvent::Attempt { outcome: AttemptOutcome::Unaffordable, .. }))
    });
    match seen
        .iter()
        .find(|e| matches!(e, HunterEvent::Attempt { outcome: AttemptOutcome::Unaffordable, .. }))
        .expect("the refusal is logged")
    {
        HunterEvent::Attempt { hunter: h, energy_paid, .. } => {
            assert_eq!(*h, hunter);
            assert_eq!(*energy_paid, 0.0, "a refusal must not charge anything");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(world.hunters().attacks_total, 0, "a refusal is not a paid attempt");
    assert_eq!(world.hunters().members[0].attack_counter, 0, "and consumes no draw");
    assert!(world.state.organisms.get(prey).is_some());
    // It spent only what living costs; the strike cost was never taken.
    let now = world.state.organisms.get(hunter).expect("alive").energy;
    assert!(now < energy_before + profile.strike_energy_cost, "{now}");
}

// ---------------------------------------------------------------- digestion

#[test]
fn a_meal_is_digested_a_portion_at_a_time_and_the_rest_is_rejected_as_litter() {
    let profile = certain(trial(&empty_world()));
    // A quiet world, so the only thing that can move the litter is this meal.
    let (mut world, hunter, _prey) = staged_in(quiet_world(), profile.clone());
    run_until(&mut world, 200, |w| w.hunters().captures_total == 1);

    let portion = profile.digest_rate * cubarium_core::DT;
    let cfg = world.config().clone();
    let (e_r, eta_m) = (cfg.organism.reserve_energy_density, cfg.organism.assimilation_material);
    let mut previous = world.hunters().members[0];
    let mut detritus = world.state.fields.d.iter().sum::<f64>();
    let mut reserve = world.state.organisms.get(hunter).expect("alive").reserve;
    let mut ticks_digesting = 0;
    for _ in 0..40 {
        let material_before = total_material(&world.state);
        let ledgers_before = world.energy_ledgers();
        let energy_before = stored_energy(&world.state);
        // The plan's own split, transcribed: `a = eta_m · min(1, rho/e_r) · q`, the rest is
        // rejected as energy-free litter.
        let density = previous.gut_energy / previous.gut_material;
        let eta = eta_m * (density / e_r).min(1.0);
        world.step();
        let member = world.hunters().members[0];
        // Each tick takes at most one portion, and the material and energy leave together.
        let taken = previous.gut_material - member.gut_material;
        assert!(taken <= portion + 1e-12, "digested {taken} in one tick");
        if taken > 0.0 {
            ticks_digesting += 1;
            let now_reserve = world.state.organisms.get(hunter).expect("alive").reserve;
            let now_detritus: f64 = world.state.fields.d.iter().sum();
            assert!(now_reserve > reserve, "nothing was stored from {taken}");
            let rejected = now_detritus - detritus;
            assert!(
                (rejected - taken * (1.0 - eta)).abs() < 1e-12,
                "rejected {rejected}, expected {} of {taken}",
                taken * (1.0 - eta)
            );
            assert!(
                (now_reserve - reserve - taken * eta).abs() < 1e-12,
                "stored {}, expected {}",
                now_reserve - reserve,
                taken * eta
            );
            reserve = now_reserve;
            detritus = now_detritus;
        }
        // The closed box and the energy identity hold on every tick of the meal.
        assert!((total_material(&world.state) - material_before).abs() < 1e-9);
        let booked = world.energy_ledgers().net_since(ledgers_before);
        let moved = stored_energy(&world.state) - energy_before;
        assert!((moved - booked).abs() < 1e-9, "energy identity broke by {}", moved - booked);
        previous = member;
        if !member.carrying() {
            break;
        }
    }
    assert!(ticks_digesting > 3, "the meal was not digested over several ticks");
}

#[test]
fn a_full_reserve_stops_digestion_and_the_gut_keeps_the_meal() {
    let profile = certain(trial(&empty_world()));
    let (mut world, hunter, _prey) = staged(profile);
    run_until(&mut world, 200, |w| w.hunters().captures_total == 1);

    // Fill the reserve: there is no headroom to store anything into.
    {
        let o = world.state.organisms.get_mut(hunter).expect("alive");
        let before = o.reserve;
        o.reserve = o.phenotype.reserve_max;
        let admitted = o.reserve - before;
        world.state.external_material_in += admitted;
    }
    let held = world.hunters().members[0];
    for _ in 0..40 {
        world.step();
    }
    let member = world.hunters().members[0];
    assert_eq!(member.gut_material, held.gut_material, "a full hunter digested anyway");
    assert_eq!(member.gut_energy, held.gut_energy);
    assert_eq!(member.phase, HunterPhase::Handling, "and it is still carrying its meal");
    // No hidden discard timer: the meal is still in the world's stored totals.
    assert!(world.hunters().gut_material_total() > 0.0);
    world.check_invariants().expect("consistent while satiated and carrying");
}

#[test]
fn a_hunter_that_cannot_pay_the_handling_cost_digests_nothing_that_tick() {
    let profile = certain(trial(&empty_world()));
    let (mut world, hunter, _prey) = staged(profile);
    run_until(&mut world, 200, |w| w.hunters().captures_total == 1);
    let held = world.hunters().members[0];
    {
        // An empty battery cannot pay for handling; the meal waits.
        let o = world.state.organisms.get_mut(hunter).expect("alive");
        o.energy = 0.0;
        let material = o.reserve;
        o.reserve = 0.0;
        world.state.external_material_in -= material;
    }
    world.step();
    let member = world.hunters().members.first().copied();
    if let Some(member) = member {
        assert_eq!(member.gut_material, held.gut_material, "it digested without paying");
    }
}

#[test]
fn a_facultative_hunter_scavenges_at_its_allocated_fraction_and_a_specialist_does_not() {
    let litter = |world: &mut World, id: OrganismId| {
        let cell = cubarium_surface::cell_of(&world.state.organisms.get(id).expect("alive").pos).index();
        world.state.fields.d[cell] = 2.0;
        world.state.fields.de[cell] = 4.0;
        world.state.external_material_in += 2.0;
    };
    let mut gains = Vec::new();
    for facultative in [false, true] {
        let base = certain(trial(&empty_world()));
        let profile = if facultative { base.facultative() } else { base };
        let mut world = quiet_world();
        // Inside a cell rather than on its edge, so the litter stays under its feet.
        let spot = SurfacePoint::new(Face::Top, 22.0, 34.0);
        let hunter =
            world.start_hunter_trial(profile, target_of(spot)).expect("the trial starts").id;
        // Hungry, with litter under it and no prey anywhere.
        aim(&mut world, hunter, Vec2::new(1.0, 0.0), 1.0);
        litter(&mut world, hunter);
        let before = world.state.organisms.get(hunter).expect("alive").reserve;
        for _ in 0..200 {
            world.step();
        }
        let after = world.state.organisms.get(hunter).expect("alive").reserve;
        gains.push(after - before);

        // Neither variant ever grazes a producer or eats fruit.
        assert_eq!(world.hunters().captures_total, 0);
    }
    assert!(gains[0].abs() < 1e-12, "a specialist scavenged {} anyway", gains[0]);
    assert!(gains[1] > 1e-6, "a facultative hunter scavenged nothing: {}", gains[1]);
}

// ---------------------------------------------------------------- one funded offspring

/// A breeding profile: the trial gates with the clocks wound down so a test can watch a whole
/// gestation. Nothing else about the lineage changes.
fn breeder(world: &World) -> FixedHunterProfile {
    let mut p = trial(world);
    p.reproduce_min_age_seconds = 0.0;
    p.gestation_seconds = 1.0;
    p.reproduce_interval_seconds = 2.0;
    p
}

/// Fill a hunter up so the profile's reproduction gate opens, booking what it gained.
fn feed_to_full(world: &mut World, id: OrganismId) {
    let o = world.state.organisms.get_mut(id).expect("alive");
    let before = o.reserve;
    o.reserve = o.phenotype.reserve_max;
    o.energy = o.phenotype.energy_max;
    world.state.external_material_in += o.reserve - before;
}

#[test]
fn one_funded_offspring_joins_the_lineage_with_the_fixed_genome() {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds.clear();
    cfg.founders.count = 0;
    cfg.weather.amplitude = 0.0;
    cfg.water.rain_rate = 0.0;
    // Mutation on, to prove a member's child is still an exact copy.
    cfg.mechanisms.mutation = true;
    cfg.mutation.probability = 1.0;
    let mut world = World::new(cfg).expect("valid");
    let profile = breeder(&world);
    let spot = SurfacePoint::new(Face::Top, 32.0, 32.0);
    let parent = world.start_hunter_trial(profile.clone(), target_of(spot)).expect("started").id;
    feed_to_full(&mut world, parent);

    // The escrow is funded out of the parent, at the world's own child fractions.
    let (reserve_before, energy_before) = {
        let o = world.state.organisms.get(parent).expect("alive");
        (o.reserve, o.energy)
    };
    run_until(&mut world, 40, |w| w.state.organisms.get(parent).is_some_and(|o| o.escrow.is_some()));
    {
        let o = world.state.organisms.get(parent).expect("alive");
        let e = o.escrow.as_ref().expect("held");
        let org = world.config().organism.clone();
        assert!((e.structure - org.child_structure_fraction * o.phenotype.structure_adult).abs() < 1e-12);
        assert!((e.reserve - org.child_reserve_fraction * o.phenotype.reserve_max).abs() < 1e-12);
        assert!((e.energy - org.child_energy_fraction * o.phenotype.energy_max).abs() < 1e-12);
        assert!(reserve_before - o.reserve >= e.structure + e.reserve - 1e-9, "the escrow was not paid for");
        assert!(energy_before - o.energy >= org.build_cost * e.structure + e.energy - 1e-9);
    }

    let material_before = total_material(&world.state);
    let ledgers_before = world.energy_ledgers();
    let energy_total_before = stored_energy(&world.state);
    let seen = run_for_events(&mut world, 200, |seen| {
        seen.iter().any(|e| matches!(e, HunterEvent::Offspring { .. }))
    });

    // Two members now, and the child is one of them by explicit membership.
    assert_eq!(world.hunters().members.len(), 2);
    assert_eq!(world.hunters().hunter_births_total, 1);
    let child = match seen.iter().find(|e| matches!(e, HunterEvent::Offspring { .. })).expect("record") {
        HunterEvent::Offspring { parent: p, child, .. } => {
            assert_eq!(*p, parent);
            *child
        }
        other => panic!("{other:?}"),
    };
    let member = world.hunters().member(child).expect("the child is a member");
    assert_eq!(member.phase, HunterPhase::Perched);
    assert!(!member.carrying());
    assert_eq!(member.target, None);
    assert_eq!(member.attack_counter, 0, "a fresh attack counter");

    // The fixed genome was copied exactly, mutation or not, and the body keeps the profile's
    // tested support.
    let child_o = world.state.organisms.get(child).expect("alive");
    assert_eq!(child_o.genome, profile.genome, "a hunter child must not mutate");
    assert_eq!(child_o.phenotype.extent, profile.body_extent_px);
    assert!(child_o.structure < child_o.phenotype.structure_adult, "it is born a juvenile");
    assert_eq!(child_o.parent, Some(parent));

    // The parent starts its recovery interval, measured from the birth.
    let interval = (profile.reproduce_interval_seconds / cubarium_core::DT).round() as u64;
    let parent_member = world.hunters().member(parent).expect("still a member");
    assert_eq!(parent_member.next_reproduction_tick, world.tick() + interval - 1 + 1);

    // Nothing was created out of nothing by the birth.
    assert!((total_material(&world.state) - material_before).abs() < 1e-9);
    let booked = world.energy_ledgers().net_since(ledgers_before);
    let moved = stored_energy(&world.state) - energy_total_before;
    assert!((moved - booked).abs() < 1e-9, "energy identity broke by {}", moved - booked);
    world.check_invariants().expect("consistent after a hunter birth");

    // Both hunters are in the view, keyed by full ID.
    let view = world.hunter_view();
    assert_eq!(view.len(), 2);
    assert!(view.iter().any(|v| v.id == child && v.juvenile));
}

#[test]
fn a_parent_that_dies_miscarries_once_and_leaves_the_lineage() {
    let mut world = quiet_world();
    let profile = breeder(&world);
    let spot = SurfacePoint::new(Face::Top, 32.0, 32.0);
    let parent = world.start_hunter_trial(profile, target_of(spot)).expect("started").id;
    feed_to_full(&mut world, parent);
    run_until(&mut world, 40, |w| w.state.organisms.get(parent).is_some_and(|o| o.escrow.is_some()));

    {
        // Starve it while it is gestating: no energy and no reserve.
        let o = world.state.organisms.get_mut(parent).expect("alive");
        let material = o.reserve;
        o.energy = 0.0;
        o.reserve = 0.0;
        world.state.external_material_in -= material;
    }
    // Baselines after the test's own edit, so what follows measures only the world's work.
    let material_before = total_material(&world.state);
    let ledgers_before = world.energy_ledgers();
    let energy_before = stored_energy(&world.state);
    let escrow_material = world
        .state
        .organisms
        .get(parent)
        .and_then(|o| o.escrow.as_ref().map(|e| e.structure + e.reserve))
        .expect("it is still gestating");
    run_until(&mut world, 40, |w| w.state.organisms.get(parent).is_none());

    assert_eq!(world.state.organisms.len(), 0, "no child was born from a dead parent");
    assert_eq!(world.hunters().members.len(), 0);
    assert_eq!(world.hunters().hunter_births_total, 0);
    assert_eq!(world.hunters().hunter_deaths_total, 1);
    let moved_material = total_material(&world.state) - material_before;
    assert!(moved_material.abs() < 1e-9, "the escrow vanished: material moved by {moved_material}");
    assert!(escrow_material > 0.0, "this fixture needs a funded escrow to miscarry");
    let booked = world.energy_ledgers().net_since(ledgers_before);
    let moved = stored_energy(&world.state) - energy_before;
    assert!((moved - booked).abs() < 1e-9, "energy identity broke by {}", moved - booked);
}

#[test]
fn a_birth_refused_by_the_cap_returns_the_escrow_and_waits() {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds.clear();
    cfg.founders.count = 0;
    cfg.weather.amplitude = 0.0;
    cfg.water.rain_rate = 0.0;
    // Room for the hunter, one bystander and one more arrival: the escrow starts with room,
    // and the world fills up before the birth is due.
    cfg.capacity.max_organisms = 3;
    let mut world = World::new(cfg).expect("valid");
    let profile = breeder(&world);
    let spot = SurfacePoint::new(Face::Top, 32.0, 32.0);
    let parent = world.start_hunter_trial(profile.clone(), target_of(spot)).expect("started").id;
    feed_to_full(&mut world, parent);
    // A bystander too big to be prey, so the cap is full but nothing gets hunted.
    let bystander = place_prey(&mut world, SurfacePoint::new(Face::Back, 8.0, 8.0), 1.8, 0.4, 0.5, true);

    run_until(&mut world, 60, |w| w.state.organisms.get(parent).is_some_and(|o| o.escrow.is_some()));
    // Now fill the last slot, so the birth this escrow is funding has nowhere to go.
    let crowd = place_prey(&mut world, SurfacePoint::new(Face::Left, 8.0, 8.0), 1.8, 0.4, 0.5, true);
    assert_eq!(world.state.organisms.len(), 3, "the world is at its cap");
    let rejections = world.state.cap_rejections_total;
    let (reserve, energy) = {
        let o = world.state.organisms.get(parent).expect("alive");
        (o.reserve, o.energy)
    };
    run_until(&mut world, 200, |w| w.state.cap_rejections_total > rejections);

    assert_eq!(world.state.organisms.len(), 3, "the cap held");
    let _ = crowd;
    assert_eq!(world.hunters().hunter_births_total, 0);
    let o = world.state.organisms.get(parent).expect("alive");
    assert!(o.escrow.is_none(), "the escrow was returned");
    assert!(o.reserve > reserve, "the material came back");
    assert!(o.energy > energy, "and so did the energy");
    // And it waits a gestation before trying again, so a full world cannot loop on it.
    let gestation = (profile.gestation_seconds / cubarium_core::DT).round() as u64;
    let member = world.hunters().member(parent).expect("a member");
    assert!(member.next_reproduction_tick >= world.tick() + gestation - 1, "{member:?}");
    let _ = bystander;
    world.check_invariants().expect("consistent after a refused hunter birth");
}

// ---------------------------------------------------------------- restart

#[test]
fn a_hunt_resumes_bit_for_bit_from_every_phase() {
    let profile = certain(trial(&empty_world()));

    // Mid-windup: a gesture in progress, with a live target and no payment yet.
    let (mut world, hunter, _) = staged(profile.clone());
    run_until(&mut world, 60, |w| phase_of(w, hunter) == HunterPhase::Windup);
    assert!(world.hunters().members[0].progress(world.tick()).is_some(), "a timed phase");
    restart_matches(&mut world, 120, "windup");

    // Mid-strike: the cost is already paid and the attempt is pending.
    let (mut world, hunter, _) = staged(profile.clone());
    run_until(&mut world, 80, |w| phase_of(w, hunter) == HunterPhase::Strike);
    assert_eq!(world.hunters().attacks_total, 1);
    restart_matches(&mut world, 120, "paid strike");

    // Part-filled gut: a meal in progress, which is stored energy and material.
    let (mut world, _hunter, _) = staged(profile.clone());
    run_until(&mut world, 200, |w| w.hunters().captures_total == 1);
    for _ in 0..10 {
        world.step();
    }
    let member = world.hunters().members[0];
    assert!(member.carrying() && member.phase == HunterPhase::Handling);
    assert!(member.gut_material > 0.0 && member.gut_energy > 0.0);
    restart_matches(&mut world, 120, "part-filled gut");

    // Mid-gestation: a funded escrow on its own, longer hunter clock.
    let mut world = quiet_world();
    let breeding = breeder(&world);
    let parent = world
        .start_hunter_trial(breeding, target_of(SurfacePoint::new(Face::Top, 22.0, 34.0)))
        .expect("started")
        .id;
    feed_to_full(&mut world, parent);
    run_until(&mut world, 60, |w| w.state.organisms.get(parent).is_some_and(|o| o.escrow.is_some()));
    restart_matches(&mut world, 60, "gestation");
    assert_eq!(world.hunters().hunter_births_total, 1, "the resumed gestation still ended in a birth");
}

// ---------------------------------------------------------------- stale handles

#[test]
fn a_stale_target_never_resolves_to_the_slot_it_used_to_name() {
    let profile = certain(trial(&empty_world()));
    let (mut world, hunter, prey) = staged(profile.clone());
    run_until(&mut world, 200, |w| w.hunters().captures_total == 1);
    assert!(world.state.organisms.get(prey).is_none());
    assert_eq!(world.hunters().members[0].target, None, "the eaten handle was dropped");

    // The freed slot is reused by a new animal with a new generation.
    let grasp = world.hunter_view()[0].capture_center.expect("an open-face grasp maps");
    let fresh = place_prey(&mut world, grasp, 0.5, 0.3, 0.4, true);
    assert_eq!(fresh.slot, prey.slot, "this fixture needs the slot to be reused");
    assert_ne!(fresh.generation, prey.generation);
    assert!(world.state.organisms.get(prey).is_none(), "the old ID still resolves to nobody");

    // Plant the stale handle back on the hunter: it must be cleared, not resolved.
    {
        let member = world.state.hunters.member_mut(hunter).expect("a member");
        member.gut_material = 0.0;
        member.gut_energy = 0.0;
        member.phase = HunterPhase::Stalking;
        member.phase_started_tick = world.state.tick;
        member.phase_ends_tick = world.state.tick;
        member.episode = 0;
        member.target = Some(prey);
    }
    world.state.validate().expect("a stale handle is a valid state, not a corrupt one");
    world.step();
    assert_ne!(world.hunters().members[0].target, Some(prey), "a stale handle was kept");
    assert!(world.state.organisms.get(fresh).is_some(), "the new occupant was not harvested for it");
    // No second capture came out of the same body.
    assert_eq!(world.hunters().captures_total, 1);
    assert_eq!(world.hunters().predation_deaths_total, 1);
}

// ---------------------------------------------------------------- decode hardening

#[test]
fn a_crafted_hunter_extension_is_refused_one_property_at_a_time() {
    let profile = certain(trial(&empty_world()));
    let base = || {
        let (mut world, _hunter, _prey) = staged(profile.clone());
        for _ in 0..5 {
            world.step();
        }
        world.state
    };

    // The control: the state these are built from really does load.
    let ok = base();
    ok.validate().expect("the control state is valid");
    decode_snapshot(&encode_snapshot(&ok, "hardening")).expect("the control snapshot loads");

    let refused = |state: WorldState, wanted: &str| {
        let err = state.validate().expect_err("this state must not validate");
        assert!(err.contains(wanted), "{err:?} does not name {wanted}");
        match decode_snapshot(&encode_snapshot(&state, "hardening")) {
            Err(SnapshotError::Invalid(e)) => assert!(e.contains(wanted), "{e:?}"),
            other => panic!("a crafted extension decoded as {other:?}"),
        }
    };

    // A member that is not a living organism.
    let mut ghost = base();
    ghost.hunters.members[0].id.generation += 1;
    refused(ghost, "live organism");

    // Two records for the same hunter.
    let mut twice = base();
    let first = twice.hunters.members[0];
    twice.hunters.members.push(first);
    refused(twice, "sorted and unique");

    // A gut past the profile's own capacity.
    let mut stuffed = base();
    stuffed.hunters.members[0].gut_material = profile.gut_capacity_material * 2.0;
    stuffed.hunters.members[0].gut_energy = 1.0;
    refused(stuffed, "capacity");

    // Energy in a gut that holds nothing.
    let mut ghost_meal = base();
    ghost_meal.hunters.members[0].gut_material = 0.0;
    ghost_meal.hunters.members[0].gut_energy = 5.0;
    refused(ghost_meal, "empty gut");

    // Handling nothing, and stalking nobody.
    let mut idle = base();
    idle.hunters.members[0].phase = HunterPhase::Handling;
    idle.hunters.members[0].target = None;
    idle.hunters.members[0].phase_ends_tick = idle.hunters.members[0].phase_started_tick;
    idle.hunters.members[0].gut_material = 0.0;
    idle.hunters.members[0].gut_energy = 0.0;
    refused(idle, "handling nothing");

    let mut aimless = base();
    aimless.hunters.members[0].phase = HunterPhase::Stalking;
    aimless.hunters.members[0].phase_ends_tick = aimless.hunters.members[0].phase_started_tick;
    aimless.hunters.members[0].target = None;
    refused(aimless, "stalking no one");

    // A timed phase with no duration, and an untimed phase that stores one.
    let mut instant = base();
    instant.hunters.members[0].phase = HunterPhase::Recovering;
    instant.hunters.members[0].target = None;
    instant.hunters.members[0].phase_ends_tick = instant.hunters.members[0].phase_started_tick;
    refused(instant, "no duration");

    let mut timed = base();
    timed.hunters.members[0].phase = HunterPhase::Perched;
    timed.hunters.members[0].target = None;
    timed.hunters.members[0].phase_ends_tick = timed.hunters.members[0].phase_started_tick + 10;
    refused(timed, "stores an end tick");

    // A hunter aiming at itself.
    let mut narcissist = base();
    let me = narcissist.hunters.members[0].id;
    narcissist.hunters.members[0].phase = HunterPhase::Stalking;
    narcissist.hunters.members[0].phase_ends_tick = narcissist.hunters.members[0].phase_started_tick;
    narcissist.hunters.members[0].target = Some(me);
    refused(narcissist, "hunting itself");

    // Members without a profile, and a control world that also holds a hunter.
    let mut orphan = base();
    orphan.hunters.profile = None;
    refused(orphan, "without a profile");

    let mut both = base();
    both.hunters.control_deposited = true;
    refused(both, "control");

    // A negative import is not an import.
    let mut negative = base();
    negative.hunters.founder_material_in = -1.0;
    refused(negative, "founder_material_in");

    // And a profile the build cannot speak.
    let mut future = base();
    future.hunters.profile.as_mut().expect("a profile").version = 99;
    refused(future, "version");
}
