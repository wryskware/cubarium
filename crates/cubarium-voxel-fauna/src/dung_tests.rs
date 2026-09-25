//! Dung: the gut, its void and where it goes. A child of `step`, so a case can set an
//! animal's age and gut directly (every direct edit is booked as introduced matter, so
//! the layer's residuals stay closed) and call `respire` itself.

use super::{GUT_VOID_S, Respiration, respire};
use crate::{Command, DT, Fauna, FaunaConfig, Species, SpeciesConfig};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command as FloraCommand, Flora, FloraConfig, Site, Species as Plant, Taken,
};

/// A flat soil plain, one voxel deep, top face at y = 2, soil at a third of its pore
/// capacity (round5c's fixture).
fn plain(width: u32) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height: 10,
        depth: 1,
        voxel_m: 1.0,
        seed: 5,
        ..VoxelConfig::default()
    });
    for x in 0..width as i64 {
        for y in 1..=2 {
            let want = 0.3 * Material::Soil.pore_capacity() * w.config().voxel_volume();
            w.apply(WorldCommand::AddWater {
                x,
                y,
                z: 0,
                volume_m3: want,
            });
            w.apply(WorldCommand::SetMaterial {
                x,
                y,
                z: 0,
                material: Material::Soil,
            });
        }
    }
    w
}

fn config_with(edit: impl FnOnce(&mut SpeciesConfig)) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    edit(c.species_mut(Species::Frondgrazer));
    c
}

fn grazer(fauna: &mut Fauna, world: &World, x: i64, body: f64) -> usize {
    assert!(fauna.apply(
        world,
        Command::Introduce {
            x,
            z: 0,
            species: Species::Frondgrazer,
            body
        }
    ));
    fauna.animals.len() - 1
}

/// Put `t` into animal `i`'s gut, booked as introduced so the ledger still closes.
fn give_gut(fauna: &mut Fauna, i: usize, t: Taken) {
    fauna.animals[i].gut.organic += t.organic;
    fauna.animals[i].gut.mineral += t.mineral;
    fauna.animals[i].gut.energy += t.energy;
    fauna.ledger.introduced_organic_in += t.organic;
    fauna.ledger.introduced_mineral_in += t.mineral;
    fauna.ledger.introduced_energy_in += t.energy;
}

fn period() -> u64 {
    (GUT_VOID_S / DT).round() as u64
}

fn assert_fauna_closes(fauna: &Fauna, when: &str) {
    let v = fauna.view();
    for (held, expected, what) in [
        (v.organic(), v.ledger.expected_organic(), "organic"),
        (v.mineral(), v.ledger.expected_mineral(), "mineral"),
        (v.energy(), v.ledger.expected_energy(), "energy"),
    ] {
        assert!(
            (held - expected).abs() <= 1e-12 * held.abs().max(1e-6),
            "{when}: fauna {what} holds {held}, the ledger expects {expected}"
        );
    }
}

fn assert_flora_closes(flora: &Flora, when: &str) {
    let v = flora.view();
    for (held, expected, what) in [
        (v.organic(), v.ledger.expected_organic(), "organic"),
        (v.mineral(), v.ledger.expected_mineral(), "mineral"),
        (v.energy(), v.ledger.expected_energy(), "energy"),
    ] {
        assert!(
            (held - expected).abs() <= 1e-9 * held.abs().max(1.0),
            "{when}: flora {what} holds {held}, the ledger expects {expected}"
        );
    }
}

/// The gut is kept until the tick an animal's age reaches a multiple of `GUT_VOID_S`
/// (600 ticks at 20 Hz), not a tick earlier, and then lands whole as litter on the face
/// the animal stands on.
#[test]
fn the_gut_voids_on_its_period_tick_and_not_before_as_litter_on_its_site() {
    assert_eq!(period(), 600, "30 s at 20 ticks a second");
    let world = plain(6);
    let mut flora = Flora::new(FloraConfig::default());
    // No upkeep, so respiration sheds nothing and the gut is only what the case put there.
    let mut fauna = Fauna::new(config_with(|s| s.maintenance_per_s = 0.0));
    let i = grazer(&mut fauna, &world, 2, 0.02);
    // Energy at one unit per unit organic, under the litter's energy cap.
    let dung = Taken {
        organic: 1e-3,
        mineral: 4e-5,
        energy: 1e-3,
    };
    give_gut(&mut fauna, i, dung);
    fauna.animals[i].age_ticks = period() - 2;

    fauna.step(&world, &mut flora);
    let a = fauna.animals[i];
    assert_eq!(a.age_ticks, period() - 1);
    assert_eq!(a.gut, dung, "one tick short of the period, the gut is kept");
    assert_eq!(fauna.ledger.deposited_organic_out, 0.0);
    assert_eq!(fauna.ledger.deposited_mineral_out, 0.0);
    assert!(
        flora.view().ground_at(a.site).is_none(),
        "nothing has reached its face"
    );

    fauna.step(&world, &mut flora);
    let a = fauna.animals[i];
    assert_eq!(a.age_ticks, period());
    assert_eq!(a.gut, Taken::default(), "the void empties the gut");
    assert_eq!(fauna.ledger.deposited_organic_out, dung.organic);
    assert_eq!(fauna.ledger.deposited_mineral_out, dung.mineral);
    assert_eq!(fauna.ledger.deposited_energy_out, dung.energy);
    let g = flora.view().ground_at(a.site).expect("the dung provisioned its face").clone();
    assert_eq!(g.litter, dung.organic, "the dung is litter");
    assert_eq!(g.litter_mineral, dung.mineral);
    assert_eq!(g.litter_energy, dung.energy);
    assert_eq!(g.carrion, 0.0, "and not carrion");
    assert_eq!(
        g.mineral,
        flora.config().initial_mineral,
        "an organic void does not touch the soluble pool"
    );
    assert_fauna_closes(&fauna, "after a void");
    assert_flora_closes(&flora, "after a void");
}

/// A gut holding only mineral — shed by respiration with nothing egested — lands in the
/// face's soluble mineral pool, the flora layer's rule for a zero-organic deposit, and
/// strands nothing in litter.
#[test]
fn a_mineral_only_gut_voids_into_the_soluble_pool() {
    let world = plain(6);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config_with(|s| s.maintenance_per_s = 0.0));
    let i = grazer(&mut fauna, &world, 2, 0.02);
    let dung = Taken {
        organic: 0.0,
        mineral: 4e-5,
        energy: 0.0,
    };
    give_gut(&mut fauna, i, dung);
    fauna.animals[i].age_ticks = period() - 1;

    fauna.step(&world, &mut flora);
    let a = fauna.animals[i];
    assert_eq!(a.gut, Taken::default());
    assert_eq!(fauna.ledger.deposited_mineral_out, dung.mineral);
    let g = flora.view().ground_at(a.site).expect("provisioned").clone();
    assert_eq!(g.mineral, flora.config().initial_mineral + dung.mineral);
    assert_eq!(g.litter, 0.0);
    assert_eq!(g.litter_mineral, 0.0, "nothing is stranded in litter");
    assert_fauna_closes(&fauna, "after a mineral-only void");
    assert_flora_closes(&flora, "after a mineral-only void");
}

/// Respiration — upkeep or motion, out of the reserve or out of the body — sheds the
/// mineral its burned organic matter held to the gut, so the body's mineral:organic ratio
/// is what it was, the body's mineral plus the gut's is constant, and a body respired to
/// nothing hands all of its mineral to the gut.
#[test]
fn respiration_keeps_the_body_s_mineral_to_organic_ratio() {
    let world = plain(6);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let i = grazer(&mut fauna, &world, 2, 0.02);
    let start = fauna.animals[i];
    assert!(start.reserve > 0.0 && start.mineral > 0.0);
    let ratio = start.mineral / start.organic();

    // Motion out of the reserve alone.
    let want = 0.5 * start.reserve;
    let (paid, from_body) = respire(&mut fauna, i, want, Respiration::Motor);
    assert_eq!((paid, from_body), (want, 0.0));
    let a = fauna.animals[i];
    assert!((a.mineral / a.organic() - ratio).abs() <= 1e-12 * ratio);
    assert!((a.gut.mineral - start.mineral * want / start.organic()).abs() <= 1e-15);
    assert_eq!(fauna.ledger.respired_motor_out, want);

    // Upkeep that empties the reserve and eats into the body.
    let want = a.reserve + 0.25 * a.body;
    let (_, from_body) = respire(&mut fauna, i, want, Respiration::Maintenance);
    assert!(from_body > 0.0, "the body paid part of it");
    let b = fauna.animals[i];
    assert!((b.mineral / b.organic() - ratio).abs() <= 1e-12 * ratio);
    assert!((b.mineral + b.gut.mineral - start.mineral).abs() <= 1e-15 * start.mineral);

    // Respired to nothing: every unit of its mineral is in the gut.
    respire(&mut fauna, i, 1.0, Respiration::Maintenance);
    let c = fauna.animals[i];
    assert_eq!(c.organic(), 0.0);
    assert_eq!(c.mineral, 0.0);
    assert!((c.gut.mineral - start.mineral).abs() <= 1e-15 * start.mineral);
    assert_eq!(c.gut.organic, 0.0, "respiration puts no organic matter in the gut");
    assert_fauna_closes(&fauna, "after respiration");
}

/// A body that dies carries its gut into its corpse: the carrion is the tissue plus the
/// gut, in all three currencies.
#[test]
fn death_puts_the_gut_into_the_carrion() {
    let world = plain(6);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config_with(|s| s.maintenance_per_s = 0.0));
    let i = grazer(&mut fauna, &world, 2, 0.02);
    let dung = Taken {
        organic: 2e-3,
        mineral: 1e-4,
        energy: 2e-3,
    };
    give_gut(&mut fauna, i, dung);
    // Starve it outright: a body under `body_min` dies this tick. The organic matter taken
    // off it is booked as removed so the ledger still closes.
    let body_min = fauna.config.species(Species::Frondgrazer).body_min;
    let a = fauna.animals[i];
    let cut = a.body - 0.5 * body_min;
    let f = cut / a.organic();
    let (m, e) = (a.mineral * f, a.energy * f);
    {
        let a = &mut fauna.animals[i];
        a.body -= cut;
        a.mineral -= m;
        a.energy -= e;
    }
    fauna.ledger.removed_organic_out += cut;
    fauna.ledger.removed_mineral_out += m;
    fauna.ledger.removed_energy_out += e;
    let before = fauna.animals[i];
    assert_ne!(before.age_ticks % period(), period() - 1, "no void this tick");

    fauna.step(&world, &mut flora);

    assert_eq!(fauna.ledger.deaths, 1, "it died");
    assert!(fauna.animals.is_empty());
    let g = flora.view().ground_at(before.site).expect("the corpse").clone();
    assert!((g.carrion - (before.organic() + dung.organic)).abs() <= 1e-18);
    assert!((g.carrion_mineral - (before.mineral + dung.mineral)).abs() <= 1e-18);
    assert!((g.carrion_energy - (before.energy + dung.energy)).abs() <= 1e-18);
    assert_eq!(g.litter, 0.0, "the gut went with the corpse, not as dung");
    assert_fauna_closes(&fauna, "after the death");
    assert_flora_closes(&flora, "after the death");
}

/// Three currencies across a feed-and-void cycle: a grazer bites, its gut fills, the gut
/// is voided onto its face, and it bites again. What the animal holds (gut included)
/// changes by exactly what it ate, less what it respired (heat) and what it voided; what
/// the flora layer booked in is what the fauna layer booked out; and the void is what the
/// gut held.
#[test]
fn a_feed_and_void_cycle_conserves_the_three_currencies() {
    let world = plain(8);
    let mut flora = Flora::new(FloraConfig::default());
    let wood = 0.5 * flora.config().springturf.wood_max;
    assert!(flora.apply(
        &world,
        FloraCommand::Seed {
            x: 3,
            z: 0,
            species: Plant::Springturf,
            wood
        }
    ));
    let mut fauna = Fauna::new(FaunaConfig::default());
    let i = grazer(&mut fauna, &world, 2, 0.02);
    // Void on the third tick: two ticks of bites before it, two after.
    fauna.animals[i].age_ticks = period() - 3;
    let before = fauna.animals[i];
    let site: Site = before.site;

    fauna.step(&world, &mut flora);
    fauna.step(&world, &mut flora);
    let fed = fauna.animals[i];
    assert!(fauna.ledger.bites > 0, "it ate");
    assert!(fed.gut.organic > 0.0, "and its gut holds the undigested share");
    assert_eq!(fauna.ledger.deposited_organic_out, 0.0, "not voided yet");

    fauna.step(&world, &mut flora);
    assert_eq!(fauna.animals[i].age_ticks % period(), 0, "the void tick");
    // The void is taken after the tick's upkeep and before its bite: the organic matter is
    // exactly what the gut held, the mineral that plus the upkeep's shed share.
    assert_eq!(fauna.ledger.deposited_organic_out, fed.gut.organic);
    assert!(fauna.ledger.deposited_mineral_out >= fed.gut.mineral);
    fauna.step(&world, &mut flora);
    fauna.step(&world, &mut flora);

    let a = fauna.animals[i];
    assert_eq!(a.site, site, "it grazed where it stood");
    let l = fauna.ledger;
    let checks = [
        (
            a.stored_organic() - before.stored_organic(),
            l.eaten_organic_in - l.respired_out - l.deposited_organic_out,
            "organic",
        ),
        (
            a.stored_mineral() - before.stored_mineral(),
            l.eaten_mineral_in - l.deposited_mineral_out,
            "mineral",
        ),
        (
            a.stored_energy() - before.stored_energy(),
            l.eaten_energy_in - l.heat_out - l.deposited_energy_out,
            "energy",
        ),
    ];
    for (gained, expected, what) in checks {
        assert!(
            (gained - expected).abs() <= 1e-15,
            "{what}: the animal gained {gained}, the ledger says {expected}"
        );
    }
    let fl = flora.view().ledger.clone();
    assert_eq!(fl.deposited_organic_in, l.deposited_organic_out);
    assert_eq!(fl.deposited_mineral_in, l.deposited_mineral_out);
    assert_eq!(fl.deposited_energy_in, l.deposited_energy_out);
    assert_eq!(fl.consumed_organic_out, l.eaten_organic_in);
    let g = flora.view().ground_at(site).expect("the dung's face").clone();
    assert_eq!(g.litter, l.deposited_organic_out, "the dung is litter on its face");
    assert_eq!(g.litter_mineral, l.deposited_mineral_out);
    assert!(a.gut.organic > 0.0, "and the gut is filling again");
    assert_fauna_closes(&fauna, "after a feed-and-void cycle");
    assert_flora_closes(&flora, "after a feed-and-void cycle");
}
