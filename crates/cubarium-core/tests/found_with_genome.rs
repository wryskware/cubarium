//! The third founding door: [`World::found_animal_with_genome`]
//! (`design/handoffs/ecology-v1-diet-factorial-opus-2026-09-16.md`, deliverable 1).
//!
//! The factorial that decides whether the skimmer's loss is its diet or its body needs to
//! put a *named* genome into a live world at a chosen place and tick. The two existing doors
//! cannot: `found_training_animal` fixes the genome to the unit adult, and the roster path
//! inside `World::new` only runs at creation. This file tests the new door from its
//! definition rather than from what it happens to produce:
//!
//! 1. **It is the roster path.** Founding with the roster's skimmer genome yields the same
//!    phenotype, field for field, as the skimmer `World::new` places. If that is not true,
//!    every "clone" in the factorial is a different animal and the control is worthless.
//! 2. **The material box closes.** The body arrives from outside, so `structure + reserve`
//!    is booked into `external_material_in` exactly, and the world's own audits still pass.
//! 3. **It refuses by name** — capacity, a zero heading, and a genome outside the genome's
//!    own bounds — rather than silently founding something else or nothing at all.

use cubarium_core::genome::{Genome, Phenotype, decode};
use cubarium_core::organism::Origin;
use cubarium_core::{World, WorldConfig};
use cubarium_surface::{Face, SurfacePoint, Vec2};

/// The pack's creature order (`crates/cubarium-core/src/config.rs:490-519`): lantern 0 =
/// grazer, sail 1 = glider, mossback 2 = burrower, skimmer 3.
const SKIMMER_FORM: u8 = 3;

fn world(seed: u64) -> World {
    World::new(WorldConfig {
        seed,
        ..WorldConfig::default()
    })
    .expect("the shipped defaults build a world")
}

fn here() -> (SurfacePoint, Vec2) {
    (
        SurfacePoint::new(Face::Front, 20.0, 20.0),
        Vec2::new(1.0, 0.0),
    )
}

/// The genome and phenotype of the first founder of `form`, as `World::new` built it.
fn roster_body(world: &World, form: u8) -> (Genome, Phenotype) {
    world
        .state
        .organisms
        .iter()
        .find(|(_, o)| o.phenotype.form == form)
        .map(|(_, o)| (o.genome.clone(), o.phenotype.clone()))
        .unwrap_or_else(|| panic!("the default roster places a founder of form {form}"))
}

// --- 1. the door is the roster path -----------------------------------------------------

/// Every founder of a kind carries the *same* genome — the kind fixes every locus it names
/// and the hue too — so "the roster's skimmer genome" is a well-defined object and reading
/// it off any one skimmer is reading it off all five.
#[test]
fn the_roster_gives_every_founder_of_a_kind_one_genome() {
    let w = world(1);
    for form in [0u8, 1, 2, 3] {
        let genomes: Vec<Genome> = w
            .state
            .organisms
            .iter()
            .filter(|(_, o)| o.phenotype.form == form)
            .map(|(_, o)| o.genome.clone())
            .collect();
        assert!(!genomes.is_empty(), "form {form} is on the roster");
        for g in &genomes {
            assert_eq!(
                g, &genomes[0],
                "form {form}: the kind fixes every locus, hue included"
            );
        }
    }
}

/// The controlling test for the whole factorial: the door founds *the roster's skimmer*,
/// not something that resembles it. Compared field by field so a failure names the field.
#[test]
fn the_door_matches_the_roster_path_for_every_phenotype_field() {
    let mut w = world(7);
    let (genome, roster) = roster_body(&w, SKIMMER_FORM);
    let (pos, heading) = here();
    let id = w
        .found_animal_with_genome(pos, heading, genome.clone())
        .expect("a roster genome founds");
    let o = w.state.organisms.get(id).expect("the founded body is live");

    assert_eq!(
        o.genome, genome,
        "the genome is stored exactly as handed in"
    );
    let p = &o.phenotype;
    assert_eq!(p.structure_adult, roster.structure_adult, "structure_adult");
    assert_eq!(p.reserve_max, roster.reserve_max, "reserve_max");
    assert_eq!(p.energy_max, roster.energy_max, "energy_max");
    assert_eq!(p.speed_max, roster.speed_max, "speed_max");
    assert_eq!(p.mouth_rate, roster.mouth_rate, "mouth_rate");
    assert_eq!(p.sense_radius, roster.sense_radius, "sense_radius");
    assert_eq!(p.maintenance, roster.maintenance, "maintenance");
    assert_eq!(p.lobes, roster.lobes, "lobes");
    assert_eq!(p.extent, roster.extent, "extent");
    assert_eq!(p.hue, roster.hue, "hue");
    assert_eq!(p.cap_foliage, roster.cap_foliage, "cap_foliage");
    assert_eq!(p.cap_detrital, roster.cap_detrital, "cap_detrital");
    assert_eq!(p.diet, roster.diet, "diet");
    assert_eq!(p.h_pref, roster.h_pref, "h_pref");
    assert_eq!(p.swim, roster.swim, "swim");
    assert_eq!(p.form, roster.form, "form");
    assert_eq!(p.drives, roster.drives, "drives");
    // And the whole struct, so a field added later is covered without editing this test.
    assert_eq!(p, &roster, "every phenotype field");
}

/// The same for the other three kinds: the door is not skimmer-specific.
#[test]
fn the_door_matches_the_roster_path_for_every_kind() {
    let mut w = world(11);
    for form in [0u8, 1, 2, 3] {
        let (genome, roster) = roster_body(&w, form);
        let pos = SurfacePoint::new(Face::Front, 8.0 + 8.0 * f64::from(form), 8.0);
        let id = w
            .found_animal_with_genome(pos, Vec2::new(0.0, 1.0), genome)
            .expect("a roster genome founds");
        let o = w.state.organisms.get(id).expect("live");
        assert_eq!(o.phenotype, roster, "form {form}: every phenotype field");
    }
}

/// A body founded through the door is an adult of the current tick, from outside, seeking.
#[test]
fn the_door_founds_an_adult_of_the_current_tick_from_outside() {
    let mut w = world(3);
    for _ in 0..50 {
        w.step();
    }
    let tick = w.tick();
    let (genome, _) = roster_body(&w, SKIMMER_FORM);
    let (pos, heading) = here();
    let id = w
        .found_animal_with_genome(pos, heading, genome)
        .expect("founds");
    let o = w.state.organisms.get(id).expect("live");
    assert_eq!(o.born_tick, tick, "born at the tick it was founded on");
    assert_eq!(
        o.origin,
        Origin::Founder,
        "it arrived from outside, like every founder"
    );
    assert_eq!(
        o.structure, o.phenotype.structure_adult,
        "adult at founding"
    );
    assert_eq!(o.births, 0);
    assert!(o.escrow.is_none());
}

// --- 2. the material box closes ---------------------------------------------------------

/// `structure + reserve` is booked into `external_material_in`, exactly, and the world's own
/// invariants still hold afterwards and on through a hundred ticks.
#[test]
fn the_material_box_closes_when_the_door_founds() {
    let mut w = world(5);
    let (genome, _) = roster_body(&w, SKIMMER_FORM);
    let phenotype = decode(&genome, &w.state.config.organism);
    let before = w.state.external_material_in;
    let (pos, heading) = here();
    let id = w
        .found_animal_with_genome(pos, heading, genome)
        .expect("founds");
    let o = w.state.organisms.get(id).expect("live");
    let expected = o.structure + o.reserve;

    assert_eq!(
        w.state.external_material_in - before,
        expected,
        "exactly the body's own structure and reserve is imported"
    );
    assert_eq!(o.structure, phenotype.structure_adult);
    assert!(
        o.reserve > 0.0 && o.reserve <= phenotype.reserve_max,
        "reserve inside its cap"
    );
    assert!(
        o.energy > 0.0 && o.energy <= phenotype.energy_max,
        "battery inside its cap"
    );
    w.check_invariants()
        .expect("the audits pass immediately after founding");
    for _ in 0..100 {
        w.step();
    }
    w.check_invariants().expect("and a hundred ticks later");
}

/// Nothing is imported when the door refuses: a refused founding leaves the world exactly as
/// it found it.
#[test]
fn a_refused_founding_imports_nothing() {
    let mut w = world(5);
    let (genome, _) = roster_body(&w, SKIMMER_FORM);
    let before = (w.state.external_material_in, w.population());
    let err = w
        .found_animal_with_genome(here().0, Vec2::ZERO, genome)
        .expect_err("a zero heading is refused");
    assert!(
        err.contains("heading"),
        "the refusal names the heading: {err}"
    );
    assert_eq!((w.state.external_material_in, w.population()), before);
}

// --- 3. the refusals, by name -----------------------------------------------------------

#[test]
fn capacity_is_refused_by_name() {
    let mut config = WorldConfig::default();
    let founders: u32 = config.founders.kinds.iter().map(|k| k.count).sum();
    config.capacity.max_organisms = founders;
    // `count` is ignored when `kinds` is non-empty, but `validate` still reads it against
    // the cap, so it comes down with the cap.
    config.founders.count = founders;
    let mut w = World::new(config).expect("a world exactly at its cap");
    assert_eq!(w.population() as u32, founders);
    let (genome, _) = roster_body(&w, SKIMMER_FORM);
    let (pos, heading) = here();
    let err = w
        .found_animal_with_genome(pos, heading, genome)
        .expect_err("the cap refuses");
    assert!(
        err.contains("cannot found another animal") && err.contains(&founders.to_string()),
        "the refusal names the capacity: {err}"
    );
    assert_eq!(w.population() as u32, founders, "and nothing was founded");
}

/// A genome outside the genome's own bounds is refused rather than quietly clamped: a caller
/// that hands in `size = 9` gets an error, not a `size = 2` animal it did not ask for.
#[test]
fn a_genome_outside_its_own_bounds_is_refused_by_name() {
    let mut w = world(2);
    let (genome, _) = roster_body(&w, SKIMMER_FORM);
    for (what, mutate) in [
        ("size", (|g: &mut Genome| g.size = 9.0) as fn(&mut Genome)),
        ("diet", |g: &mut Genome| g.diet = 1.5),
        ("diet", |g: &mut Genome| g.diet = -0.1),
        ("speed", |g: &mut Genome| g.speed = 0.0),
        ("sense", |g: &mut Genome| g.sense = 99.0),
    ] {
        let mut bad = genome.clone();
        mutate(&mut bad);
        let before = w.population();
        match w.found_animal_with_genome(here().0, here().1, bad) {
            Ok(id) => panic!("{what} out of range must be refused, got {id:?}"),
            Err(e) => assert!(
                e.contains("bounds"),
                "the refusal says the genome is out of bounds ({what}): {e}"
            ),
        }
        assert_eq!(w.population(), before, "{what}: and nothing was founded");
    }
}

/// The door founds what it was given, unchanged: the same genome in, the same genome out,
/// for every legal value of `diet` the factorial uses.
#[test]
fn the_door_never_rewrites_the_genome_it_was_given() {
    let mut w = world(13);
    let (base, _) = roster_body(&w, SKIMMER_FORM);
    for diet in [0.10f32, 0.60, 0.85, 0.90] {
        let mut g = base.clone();
        g.diet = diet;
        let id = w
            .found_animal_with_genome(here().0, here().1, g.clone())
            .expect("founds");
        let o = w.state.organisms.get(id).expect("live");
        assert_eq!(o.genome, g, "diet {diet}: stored exactly");
        assert_eq!(
            o.phenotype,
            decode(&g, &w.state.config.organism),
            "diet {diet}: decoded once"
        );
    }
}

/// The contract's own arithmetic, read off bodies the door founded: `cap_foliage = φ(diet)`
/// and `cap_detrital = φ(1 − diet)` with the gate `θ`, so `diet = 0.85` is a *pure* foliage
/// feeder and `diet = 0.60` is worse than the specialist at both foods.
#[test]
fn the_caps_the_door_produces_are_the_contracts() {
    let mut w = world(17);
    let (base, _) = roster_body(&w, SKIMMER_FORM);
    let theta = w.state.config.organism.capability_gate;
    let gamma = w.state.config.organism.capability_exponent;
    assert_eq!((theta, gamma), (0.2, 1.0), "the shipped gate and exponent");
    let phi = |x: f64| {
        if x >= theta && x > 0.0 {
            x.powf(gamma)
        } else {
            0.0
        }
    };
    for diet in [0.10f32, 0.60, 0.85, 0.90] {
        let mut g = base.clone();
        g.diet = diet;
        let id = w
            .found_animal_with_genome(here().0, here().1, g)
            .expect("founds");
        let p = &w.state.organisms.get(id).expect("live").phenotype;
        let d = f64::from(diet);
        assert!(
            (p.cap_foliage - phi(d)).abs() < 1e-12,
            "diet {diet}: cap_foliage"
        );
        assert!(
            (p.cap_detrital - phi(1.0 - d)).abs() < 1e-12,
            "diet {diet}: cap_detrital"
        );
    }
}

/// The two existing doors are untouched: `found_training_animal` still founds the unit adult
/// at [`cubarium_core::world::TRAINING_FOUNDER_HUE`].
#[test]
fn the_training_door_still_founds_the_unit_adult() {
    let mut w = world(19);
    let id = w.found_training_animal(here().0, here().1).expect("founds");
    let o = w.state.organisms.get(id).expect("live");
    let expect = Genome::founder(
        cubarium_core::world::TRAINING_FOUNDER_HUE,
        &w.state.config.drives,
    );
    assert_eq!(
        o.genome, expect,
        "the training body is still the unit adult"
    );
    assert_eq!(o.origin, Origin::Founder);
}
