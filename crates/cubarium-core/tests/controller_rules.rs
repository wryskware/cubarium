//! The controller's named rules, from `design/m2-world-spec.md` "Controller (named drives,
//! two memories)" and the normative doc comment on `controller::decide`.
//!
//! Every organism here is hand-built from `Genome::founder` + `genome::decode`, so the tests
//! read only the public construction path.

use cubarium_core::config::{DriveConfig, OrganismConfig};
use cubarium_core::controller::{Observation, decide};
use cubarium_core::genome::{Genome, decode};
use cubarium_core::organism::{Escrow, Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::DT;
use cubarium_surface::{Face, SurfacePoint, Vec2};

fn drives() -> DriveConfig {
    DriveConfig::default()
}

fn organism(mode: Mode, reserve: f64, energy: f64, hunger_memory: f64) -> Organism {
    let genome = Genome::founder(0.5, &drives());
    let phenotype = decode(&genome, &OrganismConfig::default());
    Organism {
        pos: SurfacePoint::new(Face::Front, 32.0, 32.0),
        heading: Vec2::new(1.0, 0.0),
        ou: Vec2::ZERO,
        structure: phenotype.structure_adult,
        reserve,
        energy,
        born_tick: 0,
        hunger_memory,
        mode,
        escrow: None,
        births: 0,
        genome,
        phenotype,
        parent: None,
        origin: Origin::Founder,
        turn_counter: Counter::default(),
        fed_this_tick: false,
    }
}

/// An organism whose hunger memory is pinned to its instantaneous hunger, so the documented
/// lag `m_h += (1 − exp(−dt/τ))(h − m_h)` is a no-op and `decide` sees exactly that value.
/// The memory under test is therefore `1 − reserve` (with `R_max = 1`), chosen exactly.
fn organism_with_pinned_hunger(mode: Mode, reserve: f64) -> Organism {
    let mut org = organism(mode, reserve, 2.0, 0.0);
    assert_eq!(org.phenotype.reserve_max, 1.0, "the fixture assumes R_max = 1");
    org.hunger_memory = org.hunger();
    org
}

/// The thresholds as the controller sees them: the genome stores them as `f32`, and `decide`
/// widens them with `f64::from`, so an `f64` config literal is not the comparison point.
fn seek_on() -> f64 {
    f64::from(Genome::founder(0.5, &drives()).drives.seek_on)
}

fn seek_off() -> f64 {
    f64::from(Genome::founder(0.5, &drives()).drives.seek_off)
}

/// Spec: "Mode with hysteresis: `Seeking` when `m_h > seek_on`, back to `Resting` when
/// `m_h < seek_off`." Both comparisons are strict, so the thresholds themselves hold.
#[test]
fn hysteresis_uses_strict_comparisons_at_both_thresholds() {
    let on = seek_on();
    let off = seek_off();
    assert!(off < on, "the fixture needs a real hysteresis band");

    // Exactly at `seek_on`: not greater than, so Resting stands.
    let org = organism_with_pinned_hunger(Mode::Resting, 1.0 - on);
    assert_eq!(org.hunger_memory, on, "the fixture did not land on seek_on");
    let d = decide(&org, &Observation::default(), 0, DT, true, true);
    assert_eq!(d.hunger_memory, on, "the pinned memory drifted");
    assert_eq!(d.mode, Mode::Resting, "m_h == seek_on must not start Seeking");

    // One representable step above: Seeking.
    let org = organism_with_pinned_hunger(Mode::Resting, (1.0 - on).next_down());
    assert!(org.hunger_memory > on, "the fixture did not clear seek_on");
    let d = decide(&org, &Observation::default(), 0, DT, true, true);
    assert!(d.hunger_memory > on, "the pinned memory drifted below the threshold");
    assert_eq!(d.mode, Mode::Seeking, "m_h just above seek_on must start Seeking");

    // Exactly at `seek_off`: not less than, so Seeking stands.
    let org = organism_with_pinned_hunger(Mode::Seeking, 1.0 - off);
    assert_eq!(org.hunger_memory, off, "the fixture did not land on seek_off");
    let d = decide(&org, &Observation::default(), 0, DT, true, true);
    assert_eq!(d.hunger_memory, off, "the pinned memory drifted");
    assert_eq!(d.mode, Mode::Seeking, "m_h == seek_off must not stop Seeking");

    // One representable step below: Resting.
    let org = organism_with_pinned_hunger(Mode::Seeking, (1.0 - off).next_up());
    assert!(org.hunger_memory < off, "the fixture did not drop below seek_off");
    let d = decide(&org, &Observation::default(), 0, DT, true, true);
    assert!(d.hunger_memory < off, "the pinned memory drifted above the threshold");
    assert_eq!(d.mode, Mode::Resting, "m_h just below seek_off must stop Seeking");

    // Inside the band nothing changes, in either direction.
    for mode in [Mode::Resting, Mode::Seeking] {
        let org = organism_with_pinned_hunger(mode, 1.0 - 0.5 * (on + off));
        assert!(
            org.hunger_memory > off && org.hunger_memory < on,
            "the fixture left the hysteresis band: {}",
            org.hunger_memory
        );
        let d = decide(&org, &Observation::default(), 0, DT, true, true);
        assert_eq!(d.mode, mode, "the hysteresis band must hold {mode:?}");
    }
}

/// Spec: "The heading turns toward `s` at most `turn_rate_max · dt`."
#[test]
fn the_turn_is_capped_at_the_maximum_turn_rate() {
    let limit = drives().turn_rate_max_deg.to_radians() * DT;

    // Steering exactly 90° away from the heading, in both senses, and 180° away.
    for steer in [Vec2::new(0.0, -1.0), Vec2::new(0.0, 1.0), Vec2::new(-1.0, 0.0)] {
        // Empty reserve gives `h = 1`, so the food gradient reaches the steering vector at
        // full weight; the other three terms are zero.
        let org = organism(Mode::Seeking, 0.0, 2.0, 1.0);
        let obs = Observation { grad_p: steer, ..Observation::default() };
        let d = decide(&org, &obs, 0, DT, true, true);

        let turned = (d.heading.length() - 1.0).abs();
        assert!(turned <= 1e-12, "the new heading is not a unit vector: {:?}", d.heading);

        let cos = org.heading.dot(d.heading).clamp(-1.0, 1.0);
        let angle = cos.acos();
        assert!(
            angle <= limit + 1e-9,
            "steering toward {steer:?} turned the heading by {angle} rad, above the \
             per-tick limit {limit} rad"
        );
        assert!(
            angle > 0.9 * limit,
            "a 90°-or-more steering demand only turned the heading by {angle} rad; the cap \
             {limit} rad should have been saturated"
        );
    }

    // Steering the heading already points at leaves it alone (no wobble at the cap).
    let org = organism(Mode::Seeking, 0.0, 2.0, 1.0);
    let obs = Observation { grad_p: org.heading, ..Observation::default() };
    let d = decide(&org, &obs, 0, DT, true, true);
    let angle = org.heading.dot(d.heading).clamp(-1.0, 1.0).acos();
    assert!(angle <= 1e-9, "aligned steering turned the heading by {angle} rad");
}

/// Spec: "`Feeding` when Seeking and the own cell holds `P ≥ feed_min` (or `D ≥ feed_min`
/// with the scavenging channel)."
#[test]
fn feeding_needs_the_mechanism_and_the_threshold() {
    let feed_min = f64::from(Genome::founder(0.5, &drives()).drives.feed_min);
    let hungry = || organism(Mode::Seeking, 0.0, 2.0, 1.0);

    // Grazing: exactly at the threshold feeds, one ulp below does not.
    let at = Observation { p_here: feed_min, ..Observation::default() };
    let d = decide(&hungry(), &at, 0, DT, true, true);
    assert_eq!(d.mode, Mode::Feeding, "p_here == feed_min must feed");
    assert_eq!((d.graze_effort, d.scavenge_effort), (1.0, 0.0));

    let below = Observation { p_here: feed_min.next_down(), ..Observation::default() };
    let d = decide(&hungry(), &below, 0, DT, true, true);
    assert_eq!(d.mode, Mode::Seeking, "p_here just below feed_min must not feed");
    assert_eq!((d.graze_effort, d.scavenge_effort), (0.0, 0.0));

    // With grazing off, only detritus decides.
    let plenty_of_producer = Observation { p_here: 10.0, ..Observation::default() };
    let d = decide(&hungry(), &plenty_of_producer, 0, DT, false, true);
    assert_eq!(d.mode, Mode::Seeking, "grazing is off; producer must not feed anyone");
    assert_eq!((d.graze_effort, d.scavenge_effort), (0.0, 0.0));

    let detritus = Observation { p_here: 10.0, d_here: feed_min, ..Observation::default() };
    let d = decide(&hungry(), &detritus, 0, DT, false, true);
    assert_eq!(d.mode, Mode::Feeding, "d_here == feed_min must feed when scavenging is on");
    assert_eq!((d.graze_effort, d.scavenge_effort), (0.0, 1.0));

    let thin = Observation { d_here: feed_min.next_down(), ..Observation::default() };
    let d = decide(&hungry(), &thin, 0, DT, false, true);
    assert_eq!(d.mode, Mode::Seeking, "d_here just below feed_min must not feed");

    // With both channels on and both cells stocked, both requests go out.
    let both = Observation { p_here: 1.0, d_here: 1.0, ..Observation::default() };
    let d = decide(&hungry(), &both, 0, DT, true, true);
    assert_eq!((d.graze_effort, d.scavenge_effort), (1.0, 1.0));

    // With both mechanisms off nothing feeds, whatever the cell holds.
    let d = decide(&hungry(), &both, 0, DT, false, false);
    assert_eq!(d.mode, Mode::Seeking);
    assert_eq!((d.graze_effort, d.scavenge_effort), (0.0, 0.0));

    // Feeding falls back to Seeking when the cell drops below the threshold.
    let feeding = organism(Mode::Feeding, 0.0, 2.0, 1.0);
    let d = decide(&feeding, &Observation::default(), 0, DT, true, true);
    assert_eq!(d.mode, Mode::Seeking);
}

/// Spec: "Effort: Seeking 1.0, Feeding 0.2, Resting `rest_effort`", with the two named
/// values coming from the phenotype's drives.
#[test]
fn effort_is_the_phenotypes_drive_for_the_mode() {
    let cfg = drives();
    let genome_drives = Genome::founder(0.5, &cfg).drives;

    // Seeking: hungry, nothing underfoot.
    let seeking = organism(Mode::Seeking, 0.0, 2.0, 1.0);
    let d = decide(&seeking, &Observation::default(), 0, DT, true, true);
    assert_eq!(d.mode, Mode::Seeking);
    assert_eq!(d.effort, 1.0, "Seeking effort is 1.0 by the spec");

    // Feeding: hungry, standing on producer.
    let obs = Observation { p_here: 1.0, ..Observation::default() };
    let d = decide(&seeking, &obs, 0, DT, true, true);
    assert_eq!(d.mode, Mode::Feeding);
    assert_eq!(d.effort, f64::from(genome_drives.feed_effort));
    // The genome stores drives as `f32`, so the config value is only recovered to `f32`
    // precision; the founder genome must still be carrying the configured drive.
    assert!(
        (d.effort - cfg.feed_effort).abs() < 1e-7,
        "feeding effort {} is not the configured {}",
        d.effort,
        cfg.feed_effort
    );

    // Resting: sated.
    let resting = organism(Mode::Resting, 1.0, 2.0, 0.0);
    let d = decide(&resting, &obs, 0, DT, true, true);
    assert_eq!(d.mode, Mode::Resting);
    assert_eq!(d.effort, f64::from(genome_drives.rest_effort));
    assert!(
        (d.effort - cfg.rest_effort).abs() < 1e-7,
        "resting effort {} is not the configured {}",
        d.effort,
        cfg.rest_effort
    );

    // A Resting organism never requests food, however stocked the cell is.
    assert_eq!((d.graze_effort, d.scavenge_effort), (0.0, 0.0));
}

/// Spec: budding requires `R ≥ bud_reserve · R_max`, `E ≥ bud_energy · E_max`,
/// `age ≥ bud_min_age`, and "not gestating".
#[test]
fn budding_requires_every_condition_including_an_empty_escrow() {
    let cfg = drives();
    let organism_cfg = OrganismConfig::default();
    let genome = Genome::founder(0.5, &cfg);
    let phenotype = decode(&genome, &organism_cfg);

    // The thresholds as the controller reads them, out of the `f32` genome.
    let reserve = f64::from(phenotype.drives.bud_reserve) * phenotype.reserve_max;
    let energy = f64::from(phenotype.drives.bud_energy) * phenotype.energy_max;
    let min_age = f64::from(phenotype.drives.bud_min_age_seconds);
    let old_enough = (min_age / DT).ceil() as u64;
    assert!(
        (min_age - cfg.bud_min_age_seconds).abs() < 1e-4,
        "the founder genome lost the configured bud_min_age_seconds"
    );

    let ready = |now: u64, reserve: f64, energy: f64| {
        let mut org = organism(Mode::Resting, reserve, energy, 0.0);
        org.born_tick = 0;
        decide(&org, &Observation::default(), now, DT, true, true).bud
    };

    assert!(ready(old_enough, reserve, energy), "every condition met but bud was refused");

    // Each condition alone blocks conception.
    assert!(!ready(old_enough - 1, reserve, energy), "a too-young organism budded");
    assert!(!ready(old_enough, reserve.next_down(), energy), "a thin reserve budded");
    assert!(!ready(old_enough, reserve, energy.next_down()), "a low energy budded");

    // An existing escrow blocks it even when everything else is satisfied.
    let mut gestating = organism(Mode::Resting, reserve, energy, 0.0);
    gestating.escrow = Some(Escrow {
        structure: organism_cfg.child_structure_fraction * phenotype.structure_adult,
        reserve: organism_cfg.child_reserve_fraction * phenotype.reserve_max,
        energy: organism_cfg.child_energy_fraction * phenotype.energy_max,
        started_tick: 0,
        genome: genome.clone(),
    });
    let d = decide(&gestating, &Observation::default(), old_enough, DT, true, true);
    assert!(!d.bud, "an organism already gestating requested a second bud");
}

/// Spec: "`ou` is the organism's Ornstein–Uhlenbeck turn noise"; the doc gives
/// `ou' = ou · (1 − dt/τ_ou) + noise · turn_noise · sqrt(dt)` with `τ_ou = 2 s`.
#[test]
fn the_turn_noise_relaxes_and_is_driven_by_the_supplied_normal() {
    const TAU_OU: f64 = 2.0;
    let turn_noise = f64::from(Genome::founder(0.5, &drives()).drives.turn_noise);

    // With no noise the vector decays geometrically toward zero.
    let mut org = organism(Mode::Resting, 1.0, 2.0, 0.0);
    org.ou = Vec2::new(1.0, -0.5);
    let d = decide(&org, &Observation::default(), 0, DT, true, true);
    let expected = org.ou * (1.0 - DT / TAU_OU);
    assert!(
        (d.ou.x - expected.x).abs() < 1e-15 && (d.ou.y - expected.y).abs() < 1e-15,
        "OU decay was {:?}, expected {expected:?}",
        d.ou
    );

    // The supplied standard normal enters scaled by `turn_noise · sqrt(dt)`.
    let mut org = organism(Mode::Resting, 1.0, 2.0, 0.0);
    org.ou = Vec2::ZERO;
    let obs = Observation { noise: Vec2::new(1.0, 2.0), ..Observation::default() };
    let d = decide(&org, &obs, 0, DT, true, true);
    let scale = turn_noise * DT.sqrt();
    assert!(
        (d.ou.x - scale).abs() < 1e-15 && (d.ou.y - 2.0 * scale).abs() < 1e-15,
        "OU drive was {:?}, expected {:?}",
        d.ou,
        Vec2::new(scale, 2.0 * scale)
    );

    // `decide` is pure: it reports the new vector rather than mutating the organism.
    assert_eq!(org.ou, Vec2::ZERO, "decide mutated the organism's OU vector");
}
