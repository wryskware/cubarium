use cubarium_surface::{CELL_COUNT, CellId, Face, FieldGraph, SurfacePoint, Vec2, cell_of};

use crate::DT;
use crate::config::WorldConfig;
use crate::events::LifeEvent;
use crate::genome::{Genome, decode};
use crate::ids::OrganismId;
use crate::organism::{DeathCause, Escrow, Mode, Organism, Origin};

use super::*;

use super::invariants::{edible_detritus, stored_energy};
use super::lifecycle::{sense_depth, sense_rings, ticks_from_seconds, up_direction};

/// The M2 fixture: v1 founders (one omnivore genotype, `diet` 0.7, drawn hues), so the
/// intake, gestation and accounting tests below read as they were written. The fauna v2
/// kinds have their own tests, which set `founders.kinds` explicitly.
fn config() -> WorldConfig {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds.clear();
    cfg
}

/// The founder diet as the world widens it from the `f32` genome.
fn founder_diet() -> f64 {
    f64::from(Genome::founder(0.5, &config().drives).diet)
}

/// Move `count` founders onto one cell and empty them out, so they contest its producer.
fn crowd_onto_cell(world: &mut World, cell: CellId, producer: f64) -> Vec<OrganismId> {
    let ids: Vec<OrganismId> = world.state.organisms.iter().map(|(id, _)| id).collect();
    let base = cell.center();
    for (k, id) in ids.iter().enumerate() {
        let o = world.state.organisms.get_mut(*id).expect("live founder");
        o.pos = SurfacePoint::new(base.face, base.u + k as f64 * 0.5, base.v);
        o.reserve = 0.0;
        o.hunger_memory = 1.0;
        o.mode = Mode::Seeking;
        assert_eq!(cell_of(&o.pos), cell, "test bodies must share the cell");
    }
    world.state.fields.p[cell.index()] = producer;
    ids
}

#[test]
fn founders_are_created_with_recorded_external_material() {
    let world = World::new(config()).expect("default config is valid");
    assert_eq!(world.population(), config().founders.count as usize);
    let expected: f64 = world
        .state
        .organisms
        .iter()
        .map(|(_, o)| o.structure + o.reserve)
        .sum();
    assert!((world.state.external_material_in - expected).abs() < 1e-12);
    assert!(world.mass_residual().abs() < 1e-12);
    world
        .check_invariants()
        .expect("a fresh world is consistent");
    // Founders land on every face: 72 draws over five faces effectively never miss one.
    let mut faces = [0u32; 5];
    for (_, o) in world.state.organisms.iter() {
        faces[o.pos.face.index()] += 1;
    }
    assert!(faces.iter().all(|&n| n > 0), "{faces:?}");
}

#[test]
fn six_hundred_ticks_conserve_material_and_keep_the_population() {
    let mut world = World::new(config()).expect("default config is valid");
    for _ in 0..600 {
        world.step();
    }
    world
        .check_invariants()
        .expect("invariants hold after 600 ticks");
    assert!(world.population() > 0, "the world died out");
    assert!(
        world.mass_residual().abs() < 1e-9,
        "mass residual {}",
        world.mass_residual()
    );
    let sample = world.telemetry();
    assert_eq!(sample.tick, 600);
    assert!(sample.light_in > 0.0 && sample.heat_out > 0.0);
    assert!(sample.occupied_cells > 0);
}

#[test]
fn replay_is_deterministic_and_seed_sensitive() {
    let mut a = World::new(config()).expect("valid");
    let mut b = World::new(config()).expect("valid");
    let mut c = World::new(WorldConfig {
        seed: config().seed + 1,
        ..config()
    })
    .expect("valid");
    for _ in 0..300 {
        a.step();
        b.step();
        c.step();
    }
    assert_eq!(a.telemetry().state_hash, b.telemetry().state_hash);
    assert_ne!(a.telemetry().state_hash, c.telemetry().state_hash);
}

#[test]
fn contested_feeding_splits_the_cell_and_conserves_material() {
    let mut cfg = config();
    cfg.founders.count = 3;
    // Low enough that three full mouthfuls (3 x 0.0025 m) cannot all be served.
    cfg.drives.feed_min = 0.001;
    // Linear requests: the spec's proportional allocation is unchanged by the intake
    // saturation, and a saturating mouth on a cell this poor asks for far too little to
    // contest it (three organisms would need `K_P` near zero or a crowd of ~180).
    cfg.organism.intake_half_saturation = 0.0;
    let mut world = World::new(cfg).expect("valid");
    let cell = CellId::new(Face::Front, 0, 0);
    let ids = crowd_onto_cell(&mut world, cell, 0.004);

    let before = world.mass_residual();
    let available = world.state.fields.p[cell.index()];
    world.step();

    let gains: Vec<f64> = ids
        .iter()
        .map(|id| world.state.organisms.get(*id).expect("alive").reserve)
        .collect();
    assert!(gains.iter().all(|&g| g > 0.0), "{gains:?}");
    for pair in gains.windows(2) {
        assert!(
            (pair[0] - pair[1]).abs() < 1e-15,
            "unequal shares {gains:?}"
        );
    }
    // The cell is emptied: the requests exceeded what it held.
    assert!(world.state.fields.p[cell.index()] >= 0.0);
    assert!(
        world.state.fields.p[cell.index()] < 1e-12,
        "{}",
        world.state.fields.p[cell.index()]
    );
    // Each organism assimilated η_m of its share; the rest became detritus in the cell.
    let taken: f64 = gains.iter().sum();
    let eta = world.config().organism.assimilation_material;
    assert!(
        (taken - eta * available).abs() < 1e-6,
        "{taken} vs {}",
        eta * available
    );
    assert!(
        (world.mass_residual() - before).abs() < 1e-12,
        "{} -> {}",
        before,
        world.mass_residual()
    );
    for (_, o) in world.state.organisms.iter() {
        assert!(o.fed_this_tick);
    }
}

#[test]
fn the_render_view_reports_gestation_only_while_an_escrow_is_held() {
    let mut cfg = config();
    cfg.founders.count = 1;
    let gestation_ticks = ticks_from_seconds(cfg.organism.gestation_seconds, DT);
    assert!(
        gestation_ticks > 1,
        "this test needs a multi-tick gestation"
    );
    let mut world = World::new(cfg).expect("valid");
    let id = world
        .state
        .organisms
        .iter()
        .map(|(id, _)| id)
        .next()
        .expect("one founder");

    // No escrow: nothing to show.
    assert_eq!(world.render_view().organisms[0].gestation, None);

    // Started this tick: no progress yet.
    world.state.tick = 100;
    let genome = world.state.organisms.get(id).expect("alive").genome.clone();
    world.state.organisms.get_mut(id).expect("alive").escrow = Some(Escrow {
        structure: 0.4,
        reserve: 0.2,
        energy: 0.5,
        started_tick: 100,
        genome,
    });
    assert_eq!(world.render_view().organisms[0].gestation, Some(0.0));

    // Halfway through, to within a tick of rounding.
    world.state.tick = 100 + gestation_ticks / 2;
    let half = world.render_view().organisms[0]
        .gestation
        .expect("gestating");
    assert!((half - 0.5).abs() < 1.0 / gestation_ticks as f32, "{half}");

    // Exactly due, and then well past it: clamped at 1, never above.
    world.state.tick = 100 + gestation_ticks;
    assert_eq!(world.render_view().organisms[0].gestation, Some(1.0));
    world.state.tick = 100 + gestation_ticks * 9;
    assert_eq!(world.render_view().organisms[0].gestation, Some(1.0));

    // And it goes away with the escrow.
    world.state.organisms.get_mut(id).expect("alive").escrow = None;
    assert_eq!(world.render_view().organisms[0].gestation, None);
}

#[test]
fn a_birth_at_the_cap_refunds_the_escrow_to_the_parent() {
    let mut cfg = config();
    cfg.capacity.max_organisms = 1;
    cfg.founders.count = 1;
    // Keep the parent's stocks still so the refund is the only change to its reserve.
    cfg.mechanisms.grazing = false;
    cfg.mechanisms.scavenging = false;
    let mut world = World::new(cfg).expect("valid");
    let id = world
        .state
        .organisms
        .iter()
        .map(|(id, _)| id)
        .next()
        .expect("one founder");

    // A gestation that finished long ago, so this tick is the birth tick.
    world.state.tick = 700;
    let (structure, reserve, energy, before_reserve, before_energy) = {
        let o = world.state.organisms.get_mut(id).expect("alive");
        let escrow = Escrow {
            structure: 0.4,
            reserve: 0.2,
            energy: 0.5,
            started_tick: 0,
            genome: o.genome.clone(),
        };
        let snapshot = (
            escrow.structure,
            escrow.reserve,
            escrow.energy,
            o.reserve,
            o.energy,
        );
        o.escrow = Some(escrow);
        snapshot
    };

    world.step();

    let o = world.state.organisms.get(id).expect("alive");
    assert!(o.escrow.is_none(), "the escrow must be released");
    assert_eq!(o.reserve, before_reserve + (structure + reserve));
    assert!(
        o.energy > before_energy,
        "energy {} vs {before_energy}",
        o.energy
    );
    assert!(
        o.energy < before_energy + energy,
        "movement still costs energy"
    );
    assert_eq!(world.population(), 1);
    assert_eq!(world.state.births_total, 0);
    assert_eq!(world.state.cap_rejections_total, 1);
}

#[test]
fn a_completed_gestation_places_a_child() {
    let mut cfg = config();
    cfg.founders.count = 1;
    cfg.mechanisms.grazing = false;
    cfg.mechanisms.scavenging = false;
    let mut world = World::new(cfg).expect("valid");
    let parent = world
        .state
        .organisms
        .iter()
        .map(|(id, _)| id)
        .next()
        .expect("one founder");
    world.state.tick = 700;
    {
        let o = world.state.organisms.get_mut(parent).expect("alive");
        o.escrow = Some(Escrow {
            structure: 0.4,
            reserve: 0.2,
            energy: 0.5,
            started_tick: 0,
            genome: o.genome.clone(),
        });
    }
    let residual = world.mass_residual();
    world.step();

    assert_eq!(world.population(), 2);
    assert_eq!(world.state.births_total, 1);
    let child = world
        .state
        .organisms
        .iter()
        .find(|(_, o)| o.origin == Origin::Descendant)
        .map(|(id, o)| (id, o.clone()))
        .expect("a child");
    assert_eq!(child.1.parent, Some(parent));
    assert_eq!(child.1.born_tick, 701);
    assert_eq!(child.1.structure, 0.4);
    assert_eq!(child.1.reserve, 0.2);
    assert!((child.1.heading.length() - 1.0).abs() < 1e-12);
    let parent_pos = world.state.organisms.get(parent).expect("alive").pos;
    let offset = cubarium_surface::surface_distance(parent_pos, child.1.pos, 16.0).expect("nearby");
    assert!((offset - 2.5).abs() < 0.1, "child placed {offset} px away");
    assert!((world.mass_residual() - residual).abs() < 1e-12);
}

#[test]
fn render_view_and_telemetry_describe_the_world() {
    let mut world = World::new(config()).expect("valid");
    world.step();
    let view = world.render_view();
    assert_eq!(view.tick, 1);
    assert_eq!(view.producer.len(), CELL_COUNT);
    assert_eq!(view.detritus.len(), CELL_COUNT);
    assert_eq!(view.organisms.len(), world.population());
    assert!(view.organisms.iter().all(|o| !o.lobes.is_empty()));
    assert!(
        view.organisms.iter().any(|o| !o.moved.is_empty()),
        "resting still drifts a little"
    );

    let sample = world.telemetry();
    assert_eq!(sample.population, world.population() as u32);
    assert_eq!(
        sample.population_by_face.iter().sum::<u32>(),
        sample.population
    );
    assert!((sample.producer_by_face.iter().sum::<f64>() - sample.producer).abs() < 1e-9);
    assert!((sample.detritus_by_face.iter().sum::<f64>() - sample.detritus).abs() < 1e-9);
    assert!(sample.producer_by_face.iter().all(|&p| p > 0.0));
    assert_eq!(
        sample.mode_resting + sample.mode_seeking + sample.mode_feeding,
        sample.population
    );
    assert!(sample.pairs_considered > 0);
    // The sample resets the per-sample counters.
    let empty = world.telemetry();
    assert_eq!(empty.pairs_considered, 0);
    assert_eq!(empty.light_in, 0.0);
}

/// `moved_segments` is the same data the render view publishes, borrowed instead of cloned.
/// An observer that wants one number per tick must not have to build a whole view, and must
/// not get a different answer for taking the cheaper door.
#[test]
fn moved_segments_is_exactly_what_the_render_view_publishes() {
    let mut world = World::new(config()).expect("valid");
    for _ in 0..40 {
        world.step();
        let view = world.render_view();
        assert_eq!(view.organisms.len(), world.population());
        for o in &view.organisms {
            assert_eq!(
                world.moved_segments(o.id),
                o.moved.as_slice(),
                "the borrowed segments differ from the published ones for {:?}",
                o.id
            );
        }
    }
    // Some organism really did move, so the equality above is not two empty slices.
    let view = world.render_view();
    assert!(view.organisms.iter().any(|o| !o.moved.is_empty()));
    // A handle the arena never issued has no segments rather than panicking.
    assert!(
        world
            .moved_segments(OrganismId {
                slot: u32::MAX,
                generation: 1
            })
            .is_empty()
    );
    // And reading them changes nothing at all.
    let before = crate::snapshot::state_hash(&world.state);
    for (id, _) in world.state.organisms.iter() {
        let _ = world.moved_segments(id);
    }
    assert_eq!(crate::snapshot::state_hash(&world.state), before);
}

#[test]
fn the_energy_audit_closes_every_tick_and_cumulatively() {
    let mut world = World::new(config()).expect("valid");
    let opening = stored_energy(&world.state);
    let mut worst: f64 = 0.0;
    for _ in 0..6000 {
        let before = stored_energy(&world.state);
        let ledgers = world.state.energy_ledgers();
        world.step();
        let booked = world.state.energy_ledgers().net_since(ledgers);
        let drift = (stored_energy(&world.state) - before) - booked;
        worst = worst.max(drift.abs());
    }
    assert!(worst < 1e-9, "worst per-tick energy drift {worst:e}");
    // The per-tick identity is exact to rounding; the cumulative sum of 6000 ticks of
    // rounding is bounded relative to the energy being differenced, not absolutely. The
    // cumulative side reads the compensated ledgers (`crate::accounting`), which for a
    // world created here (corrections open at zero) is the whole history.
    let booked = world.state.net_energy_in_corrected();
    let total = stored_energy(&world.state);
    let overall = (total - opening) - booked;
    assert!(
        overall.abs() < 1e-9 * total.max(1.0),
        "cumulative energy drift {overall:e} over 6000 ticks"
    );
    // A real leak would be many orders larger than accumulated rounding.
    assert!(
        overall.abs() / 6000.0 < 1e-10,
        "systematic energy drift {:e} per tick",
        overall.abs() / 6000.0
    );
    assert!(world.state.light_in_total > 0.0 && world.state.heat_out_total > 0.0);
}

#[test]
fn the_intake_request_saturates_at_half_at_k_p() {
    // One organism alone on a frozen cell: its reserve gain is exactly `η_m · q`.
    fn gain(half_saturation: f64, producer: f64) -> f64 {
        let mut cfg = config();
        cfg.founders.count = 1;
        cfg.mechanisms.scavenging = false;
        cfg.producer.growth = 0.0;
        cfg.producer.mortality = 0.0;
        cfg.detritus.decomposition = 0.0;
        cfg.nutrient.diffusion = 0.0;
        // A rich cell would ripen a trace of fruit before the bite and shift `P`.
        cfg.fruit.ripen = 0.0;
        cfg.organism.intake_half_saturation = half_saturation;
        let mut world = World::new(cfg).expect("valid");
        let id = world
            .state
            .organisms
            .iter()
            .map(|(id, _)| id)
            .next()
            .expect("one founder");
        let cell = CellId::new(Face::Front, 0, 0);
        {
            let o = world.state.organisms.get_mut(id).expect("alive");
            o.pos = cell.center();
            o.reserve = 0.0;
            o.hunger_memory = 1.0;
            o.mode = Mode::Seeking;
        }
        world.state.fields.p[cell.index()] = producer;
        world.step();
        world.state.organisms.get(id).expect("alive").reserve
    }

    let org = config().organism;
    let k_p = org.intake_half_saturation;
    assert!(k_p > 0.0, "the default is a saturating mouth");

    // `K_P = 0` is the linear law: a full mouthful of `graze_rate · dt = k_mouth · diet ·
    // dt` (`design/fauna-v2.md`), assimilated at η_m (to rounding: the world multiplies
    // the same factors in its own order).
    let linear = gain(0.0, k_p);
    let mouthful = org.assimilation_material * (org.mouth_rate * founder_diet()) * DT;
    assert!(
        (linear - mouthful).abs() < 1e-15 * mouthful,
        "{linear} vs {mouthful}"
    );

    // At `P = K_P` the type-II term is exactly one half.
    let saturating = gain(k_p, k_p);
    assert_eq!(saturating, 0.5 * linear);

    // And it is monotone in the cell's stock: more food, bigger bite, never more than one.
    let richer = gain(k_p, 3.0 * k_p);
    assert!(saturating < richer && richer < linear);
    assert!(
        (richer - 0.75 * linear).abs() < 1e-15 * linear,
        "{richer} vs {}",
        0.75 * linear
    );
}

#[test]
fn poor_detritus_assimilates_less_and_still_closes() {
    let mut cfg = config();
    cfg.founders.count = 2;
    cfg.mechanisms.grazing = false;
    // Freeze the fields so the detritus the organisms bite into is exactly what is set
    // here: the type-II request reads the cell's pre-settlement stock, and both test
    // cells are on a side face, so the fall step would slide a slice of it away first.
    cfg.producer.growth = 0.0;
    cfg.producer.mortality = 0.0;
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.fall = 0.0;
    cfg.nutrient.diffusion = 0.0;
    let e_r = cfg.organism.reserve_energy_density;
    let mut world = World::new(cfg).expect("valid");
    let ids: Vec<OrganismId> = world.state.organisms.iter().map(|(id, _)| id).collect();
    // Two cells with the same detritus but energy densities of e_r/8 and e_r/2.
    let cells = [
        CellId::new(Face::Front, 0, 0),
        CellId::new(Face::Front, 4, 4),
    ];
    let densities = [e_r / 8.0, e_r / 2.0];
    // Enough detritus that even the poor cell's edible share clears `feed_min`.
    let detritus = 2.0;
    for (k, id) in ids.iter().enumerate() {
        let o = world.state.organisms.get_mut(*id).expect("alive");
        o.pos = cells[k].center();
        o.reserve = 0.0;
        o.hunger_memory = 1.0;
        o.mode = Mode::Seeking;
        let c = cells[k].index();
        world.state.fields.p[c] = 0.0;
        world.state.fields.d[c] = detritus;
        world.state.fields.de[c] = densities[k] * detritus;
    }
    let before = stored_energy(&world.state);
    let (light, heat) = (world.state.light_in_total, world.state.heat_out_total);
    world.step();

    let gains: Vec<f64> = ids
        .iter()
        .map(|id| world.state.organisms.get(*id).expect("alive").reserve)
        .collect();
    assert!(gains[0] > 0.0 && gains[1] > 0.0, "{gains:?}");
    // Poor detritus loses twice: `η` scales with `ρ/e_r` (a factor of four here) and the
    // type-II request scales with `D_eff/(D_eff + K_P)` on top of it.
    let k_p = world.config().organism.intake_half_saturation;
    let edible: Vec<f64> = densities
        .iter()
        .map(|rho| detritus * (rho / e_r).min(1.0))
        .collect();
    let bite = |food: f64| food / (food + k_p);
    let expected = 4.0 * bite(edible[1]) / bite(edible[0]);
    assert!(
        expected > 4.0,
        "the saturating request must widen the gap, not close it"
    );
    assert!(
        (gains[1] / gains[0] - expected).abs() < 1e-6,
        "ratio {} vs {expected}",
        gains[1] / gains[0]
    );
    // The poor detritus never credits more reserve energy than the food carried.
    for (k, id) in ids.iter().enumerate() {
        let o = world.state.organisms.get(*id).expect("alive");
        let c = cells[k].index();
        assert!(world.state.fields.de[c] >= 0.0);
        assert!(o.reserve * e_r <= densities[k] * detritus + 1e-12);
    }
    let booked = (world.state.light_in_total - light) - (world.state.heat_out_total - heat);
    assert!(((stored_energy(&world.state) - before) - booked).abs() < 1e-9);
}

#[test]
fn energy_free_detritus_is_not_food() {
    let mut cfg = config();
    cfg.founders.count = 2;
    cfg.mechanisms.grazing = false;
    // `De = 2.0` on `D = 1.0` needs a cap that admits it; the point is `ρ ≥ e_r`.
    cfg.detritus.energy_cap = 2.0;
    let mut world = World::new(cfg).expect("valid");
    let ids: Vec<OrganismId> = world.state.organisms.iter().map(|(id, _)| id).collect();
    let cells = [
        CellId::new(Face::Front, 0, 0),
        CellId::new(Face::Front, 8, 8),
    ];
    // Same detritus, no energy versus fully charged.
    let energies = [0.0, 2.0];
    for (k, id) in ids.iter().enumerate() {
        let o = world.state.organisms.get_mut(*id).expect("alive");
        o.pos = cells[k].center();
        o.reserve = 0.0;
        o.hunger_memory = 1.0;
        o.mode = Mode::Seeking;
        let c = cells[k].index();
        world.state.fields.p[c] = 0.0;
        world.state.fields.d[c] = 1.0;
        world.state.fields.de[c] = energies[k];
    }
    world.step();

    let spent = world.state.organisms.get(ids[0]).expect("alive");
    assert_eq!(
        spent.mode,
        Mode::Seeking,
        "energy-free detritus must not read as food"
    );
    assert_eq!(spent.reserve, 0.0);
    assert!(!spent.fed_this_tick);

    let rich = world.state.organisms.get(ids[1]).expect("alive");
    assert_eq!(rich.mode, Mode::Feeding, "charged detritus is food");
    assert!(rich.reserve > 0.0);
    assert!(rich.fed_this_tick);
}

#[test]
fn both_intake_channels_respect_the_reserve_ceiling() {
    let mut cfg = config();
    cfg.founders.count = 1;
    let mut world = World::new(cfg).expect("valid");
    let id = world
        .state
        .organisms
        .iter()
        .map(|(id, _)| id)
        .next()
        .expect("one founder");
    let cell = CellId::new(Face::Front, 0, 0);
    // Headroom of 0.0014 m: more than grazing's saturated bite alone (about 0.0012 m at
    // `diet` 0.7), less than the two channels' bites together
    // (`graze_rate + scavenge_rate = mouth_rate`, a full `k_mouth · dt = 0.0025 m`).
    let (reserve_max, headroom) = {
        let o = world.state.organisms.get_mut(id).expect("alive");
        o.pos = cell.center();
        o.hunger_memory = 1.0;
        o.mode = Mode::Seeking;
        o.reserve = o.phenotype.reserve_max - 0.0014;
        (o.phenotype.reserve_max, 0.0014)
    };
    let c = cell.index();
    world.state.fields.p[c] = 1.0;
    world.state.fields.d[c] = 1.0;
    world.state.fields.de[c] = 1.0;
    let before = world.state.organisms.get(id).expect("alive").reserve;
    world.step();

    let o = world.state.organisms.get(id).expect("alive");
    assert!(
        o.reserve <= reserve_max,
        "reserve {} exceeds {reserve_max}",
        o.reserve
    );
    assert!(o.reserve - before <= headroom + 1e-12);
    // Grazing alone could add at most η_m times its saturated request; more than that
    // proves the scavenging channel ran too.
    let org = &world.config().organism;
    let k_p = org.intake_half_saturation;
    let grazing_only = org.assimilation_material * (0.0025 * founder_diet()) * (1.0 / (1.0 + k_p));
    assert!(
        o.reserve - before > grazing_only,
        "{} vs {grazing_only}",
        o.reserve - before
    );
    assert!(o.fed_this_tick);
}

#[test]
fn energy_never_goes_negative_while_starving() {
    let mut cfg = config();
    cfg.producer.growth = 0.0;
    cfg.producer.initial_fraction = 0.0;
    // No initial litter either (`detritus.initial_dark`): a foodless world has nothing
    // to scavenge.
    cfg.detritus.initial_dark = 0.0;
    // Dry: standing in a pool slows a body (`design/water.md` "Wading") and so cuts its
    // movement cost, which stretches starvation past this test's five minutes. The test
    // is about the audit while starving, not about pools.
    cfg.water.rain_rate = 0.0;
    let mut world = World::new(cfg).expect("valid");
    let opening = stored_energy(&world.state);
    // **R0b.** 6,000 ticks (300 s) used to be enough. Under the corrected shared budget a body
    // that turns spends most of its capability on the turn and travels very little, so the
    // motor half of the bill is much smaller and a starving body lives longer on the same
    // stores — R0a measured ~400 s of pure upkeep from a full one. 20,000 ticks (1,000 s) is
    // the same test with a horizon that outlasts the new upkeep-only survival; the loop still
    // stops the moment the world empties.
    for _ in 0..20_000 {
        world.step();
        for (_, o) in world.state.organisms.iter() {
            assert!(o.energy >= 0.0, "energy {} went negative", o.energy);
        }
        if world.population() == 0 {
            break;
        }
    }
    assert_eq!(world.population(), 0, "a foodless world must empty out");
    assert!(
        world.state.deaths_total[0] > 0,
        "starvation is the cause: {:?}",
        world.state.deaths_total
    );
    assert!(world.mass_residual().abs() < 1e-9);
    let closing = stored_energy(&world.state);
    let booked = world.state.net_energy_in_corrected();
    let drift = (closing - opening) - booked;
    println!(
        "starvation audit: opening {opening:e} closing {closing:e} booked {booked:e} drift {drift:e}"
    );
    assert!(drift.abs() < 1e-9 * opening.max(1.0), "drift {drift:e}");
}

#[test]
fn life_events_account_for_every_birth_and_death() {
    let cfg = config();
    // Budding needs the age gate, then a full gestation, before a child appears.
    let earliest_parent_age =
        ((cfg.drives.bud_min_age_seconds + cfg.organism.gestation_seconds) / DT).floor() as u64;
    let mut world = World::new(cfg).expect("valid");

    let mut births = 0u64;
    let mut deaths = 0u64;
    let mut seen: Vec<OrganismId> = Vec::new();
    for _ in 0..12_000 {
        world.step();
        for event in world.drain_events() {
            assert_eq!(
                event.tick(),
                world.tick(),
                "an event is dated off its commit tick"
            );
            match event {
                LifeEvent::Birth {
                    id,
                    parent,
                    parent_age_ticks,
                    parent_births,
                    origin,
                    genome,
                    ..
                } => {
                    assert_ne!(id, parent);
                    assert_eq!(origin, Origin::Descendant, "founders emit no birth event");
                    assert!(parent_births >= 1, "the parent's own birth is counted");
                    assert!(
                        parent_age_ticks >= earliest_parent_age,
                        "parent aged {parent_age_ticks} ticks cannot have gestated yet"
                    );
                    assert_ne!(genome, 0);
                    assert_eq!(
                        world.state.organisms.get(id).map(|o| o.parent),
                        Some(Some(parent))
                    );
                    assert!(!seen.contains(&id), "organism id {id:?} was born twice");
                    seen.push(id);
                    births += 1;
                }
                LifeEvent::Death {
                    id,
                    age_ticks,
                    births: had,
                    ..
                } => {
                    assert!(age_ticks > 0);
                    assert!(
                        world.state.organisms.get(id).is_none(),
                        "a dead organism is gone"
                    );
                    let _ = had;
                    deaths += 1;
                }
            }
        }
        // A second drain in the same tick yields nothing.
        assert!(world.drain_events().is_empty());
    }

    assert!(
        births > 0 && deaths > 0,
        "the run produced {births} births and {deaths} deaths"
    );
    assert_eq!(
        births, world.state.births_total,
        "birth events do not match births_total"
    );
    assert_eq!(
        deaths,
        world.state.deaths_total.iter().sum::<u64>(),
        "death events do not match deaths_total"
    );
    assert!(world.drain_events().is_empty());
}

#[test]
fn the_field_dump_and_cell_graph_describe_every_cell() {
    let mut world = World::new(config()).expect("valid");
    world.step();
    let dump = world.field_dump();
    assert_eq!(dump.tick, 1);
    assert_eq!(dump.n, world.state.fields.n);
    assert_eq!(dump.p, world.state.fields.p);
    assert_eq!(dump.d, world.state.fields.d);
    assert_eq!(dump.de, world.state.fields.de);
    assert_eq!(dump.organisms.len(), CELL_COUNT);
    let counted: u32 = dump.organisms.iter().map(|&c| u32::from(c)).sum();
    assert_eq!(
        counted,
        world.population() as u32,
        "every organism is counted once"
    );
    for (_, o) in world.state.organisms.iter() {
        assert!(dump.organisms[cell_of(&o.pos).index()] > 0);
    }

    let neighbors = world.cell_neighbors();
    assert_eq!(neighbors.len(), CELL_COUNT);
    // The open rim leaves 64 cells with three neighbors; everyone else has four.
    let rim = neighbors
        .iter()
        .filter(|n| n.iter().any(Option::is_none))
        .count();
    assert_eq!(rim, 64);
    for (i, n) in neighbors.iter().enumerate() {
        for &there in n.iter().flatten() {
            assert!(
                neighbors[usize::from(there)]
                    .iter()
                    .flatten()
                    .any(|&back| usize::from(back) == i),
                "cell {i} -> {there} is not reciprocal"
            );
        }
    }
}

#[test]
fn from_state_rebuilds_and_zeroes_the_residual() {
    let mut world = World::new(config()).expect("valid");
    for _ in 0..20 {
        world.step();
    }
    let state = world.state.clone();
    let reloaded = World::from_state(state).expect("a stepped state is valid");
    assert!(reloaded.mass_residual().abs() < 1e-12);
    assert_eq!(reloaded.tick(), world.tick());
    assert_eq!(reloaded.population(), world.population());
}

/// `design/water.md`: the water budget is an exact identity for a world created dry,
/// `Σw == rain_in_total − evap_out_total`, checked every tick and cumulatively.
#[test]
fn the_water_budget_closes_every_tick_and_cumulatively() {
    let mut world = World::new(config()).expect("valid");
    assert_eq!(
        world.state.fields.w.iter().sum::<f64>(),
        0.0,
        "a new world is dry"
    );
    let mut worst: f64 = 0.0;
    for _ in 0..2000 {
        let before: f64 = world.state.fields.w.iter().sum();
        let (rain, evap) = (world.state.rain_in_total, world.state.evap_out_total);
        world.step();
        let booked = (world.state.rain_in_total - rain) - (world.state.evap_out_total - evap);
        let drift = (world.state.fields.w.iter().sum::<f64>() - before) - booked;
        worst = worst.max(drift.abs());
    }
    assert!(worst < 1e-9, "worst per-tick water drift {worst:e}");
    let total: f64 = world.state.fields.w.iter().sum();
    assert!(
        world.water_residual().abs() < 1e-9 * total.max(1.0),
        "residual {:e}",
        world.water_residual()
    );
    assert!(
        world.state.rain_in_total > 0.0,
        "it must have rained somewhere in 100 s"
    );
    assert!(world.state.evap_out_total > 0.0);
    assert!(total > 0.0);
    // The view and telemetry carry the same water.
    let view = world.render_view();
    assert_eq!(view.water, world.state.fields.w);
    assert_eq!(view.rain.len(), CELL_COUNT);
    let sample = world.telemetry();
    assert!((sample.water - total).abs() < 1e-12);
    assert!((sample.water_by_face.iter().sum::<f64>() - total).abs() < 1e-9);
}

/// `design/water.md` "Wading": an organism's speed is divided by `1 + w` of its cell.
///
/// **R0b.** Wading divides `speed_cap`, and since R0b `speed_cap` is the whole shared motor
/// budget rather than the translation half of a union of two ceilings. So what wading divides
/// exactly is `|v| + r·|ω|` — that is the assertion below. *Travel alone* no longer halves
/// when the body is also turning: a seeking body asks for its full cap and for whatever turn
/// its steering wants, the resolver scales both by one common factor `u / demand`, and the
/// granted speed is therefore `u²/demand`, which falls faster than linearly in `u` while the
/// turn request stays the same. Measured here: travel falls 3.86x at unit depth, and the
/// budget falls exactly 2x. Wading also now slows *turning*, which it never did before.
#[test]
fn wading_halves_the_motor_budget_at_unit_depth() {
    /// `(travel px, sweep px)` over one tick — the two halves of `|v| + r·|ω| · dt`.
    fn motion(depth: f64) -> (f64, f64) {
        let mut cfg = config();
        cfg.founders.count = 1;
        // Freeze the water so the depth the organism wades through is exactly `depth`.
        cfg.water.rain_rate = 0.0;
        cfg.water.flow = 0.0;
        cfg.water.evap = 0.0;
        let mut world = World::new(cfg).expect("valid");
        let id = world
            .state
            .organisms
            .iter()
            .map(|(id, _)| id)
            .next()
            .expect("founder");
        let cell = {
            let o = world.state.organisms.get_mut(id).expect("alive");
            o.mode = Mode::Seeking;
            o.hunger_memory = 1.0;
            o.reserve = 0.0;
            cell_of(&o.pos)
        };
        world.state.fields.w[cell.index()] = depth;
        // Keep the cell's food below the feeding threshold so the mode stays Seeking.
        world.state.fields.p[cell.index()] = 0.0;
        world.state.fields.d[cell.index()] = 0.0;
        let (before, extent) = {
            let o = world.state.organisms.get(id).expect("alive");
            (o.heading, o.phenotype.extent)
        };
        world.step();
        let after = world.state.organisms.get(id).expect("alive").heading;
        let turn = {
            use std::f64::consts::{PI, TAU};
            let d = after.screen_angle() - before.screen_angle();
            (d + PI).rem_euclid(TAU) - PI
        };
        let view = world.render_view();
        let travel: f64 = view.organisms[0].moved.iter().map(|s| s.length()).sum();
        (travel, extent * turn.abs())
    }
    let budget = |(travel, sweep): (f64, f64)| travel + sweep;
    let dry = motion(0.0);
    let wading = motion(1.0);
    let deep = motion(3.0);
    assert!(dry.0 > 0.0, "a seeking organism moves");
    assert!(dry.1 > 0.0, "and this fixture's organism also turns, so the budget is shared");
    assert!(
        (budget(dry) / budget(wading) - 2.0).abs() < 1e-9,
        "dry {dry:?} wading {wading:?}"
    );
    assert!(
        (budget(dry) / budget(deep) - 4.0).abs() < 1e-9,
        "dry {dry:?} deep {deep:?}"
    );
    // Turning is throttled by the water too, which is the behaviour the old envelope's
    // independent rotation allowance hid.
    assert!(wading.1 < dry.1, "wading did not slow the turn: {wading:?} vs {dry:?}");
    assert!(deep.1 < wading.1, "deeper water did not slow the turn further");
}

/// Not a test of anything: prints where the water stands after two simulated hours of a
/// default world (soil / foliage / canopy by cell height, the soil floor row, the top
/// face's wettest cells), for the short-run report in `design/water.md`'s slice.
/// `cargo test -p cubarium-core --release --lib water_by_band -- --ignored --nocapture`.
#[test]
#[ignore]
fn report_water_by_band_after_two_hours() {
    let seed: u64 = std::env::var("CUBARIUM_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let mut cfg = config();
    cfg.seed = seed;
    let mut world = World::new(cfg).expect("valid");
    let mut pop_min = world.population();
    // Ponds on the level top face come and go with the showers, so sample them over
    // time as well as at the end: how often at least one interior top cell holds more
    // than 0.5, and the most such cells seen at once.
    let (mut top_pond_ticks, mut top_pond_peak, mut samples) = (0u64, 0usize, 0u64);
    for tick in 0..(7200.0 / DT) as u64 {
        world.step();
        pop_min = pop_min.min(world.population());
        if tick % 20 == 0 {
            samples += 1;
            let ponds = CellId::all()
                .filter(|c| {
                    c.face() == Face::Top && (1..15).contains(&c.cx()) && (1..15).contains(&c.cy())
                })
                .filter(|c| world.state.fields.w[c.index()] > 0.5)
                .count();
            if ponds > 0 {
                top_pond_ticks += 1;
            }
            top_pond_peak = top_pond_peak.max(ponds);
        }
    }
    println!(
        "top-face interior ponds (>0.5) over the run: present in {:.0}% of once-a-second samples, peak {top_pond_peak} cells at once",
        100.0 * top_pond_ticks as f64 / samples as f64
    );
    let w = &world.state.fields.w;
    let (mut soil, mut foliage, mut canopy, mut floor) = ((0.0, 0), (0.0, 0), (0.0, 0), (0.0, 0));
    let mut top_cells: Vec<(f64, CellId)> = Vec::new();
    for cell in CellId::all() {
        let h = cell.center().embed()[1];
        let i = cell.index();
        if cell.face() == Face::Top {
            canopy.0 += w[i];
            canopy.1 += 1;
            top_cells.push((w[i], cell));
        } else if h < -0.33 {
            soil.0 += w[i];
            soil.1 += 1;
            if cell.cy() == 15 {
                floor.0 += w[i];
                floor.1 += 1;
            }
        } else {
            foliage.0 += w[i];
            foliage.1 += 1;
        }
    }
    let total: f64 = w.iter().sum();
    top_cells.sort_by(|a, b| b.0.total_cmp(&a.0));
    let pools_floor = (0..64).filter(|_| true).count();
    let _ = pools_floor;
    let floor_cells: Vec<f64> = CellId::all()
        .filter(|c| c.face() != Face::Top && c.cy() == 15)
        .map(|c| w[c.index()])
        .collect();
    let floor_pools = floor_cells.iter().filter(|&&x| x > 0.5).count();
    let floor_dry = floor_cells.iter().filter(|&&x| x < 0.3).count();
    let floor_max = floor_cells.iter().cloned().fold(0.0, f64::max);
    let floor_min = floor_cells.iter().cloned().fold(f64::MAX, f64::min);
    let top_pools = top_cells.iter().filter(|(x, _)| *x > 0.5).count();
    println!(
        "water after 2 h (seed {seed}): total {total:.2}; soil {:.2} ({:.0}%), foliage {:.2} ({:.0}%), canopy {:.2} ({:.0}%)",
        soil.0,
        100.0 * soil.0 / total,
        foliage.0,
        100.0 * foliage.0 / total,
        canopy.0,
        100.0 * canopy.0 / total
    );
    println!(
        "soil floor row (64 cells): {:.2} total, mean {:.3}, min {floor_min:.3}, max {floor_max:.3}, {floor_pools} cells deeper than 0.5, {floor_dry} cells under 0.3 (dry gaps)",
        floor.0,
        floor.0 / floor.1 as f64
    );
    println!("population: min {pop_min}, end {}", world.population());
    println!(
        "top face: mean {:.3}, {top_pools} cells deeper than 0.5, wettest {:?}",
        canopy.0 / canopy.1 as f64,
        top_cells
            .iter()
            .take(3)
            .map(|(x, c)| (format!("{x:.3}"), c.cx(), c.cy()))
            .collect::<Vec<_>>()
    );
    println!(
        "budget: rain_in {:.2} evap_out {:.2} residual {:e}; population {}",
        world.state.rain_in_total,
        world.state.evap_out_total,
        world.water_residual(),
        world.population()
    );
}

// --- Fauna v2 (`design/fauna-v2.md`) -------------------------------------------------

/// One default-kind founder of each kind, by `form`.
fn one_of_each_kind(world: &World) -> Vec<Organism> {
    let mut out = Vec::new();
    for form in [2u8, 0, 1, 3] {
        out.push(
            world
                .state
                .organisms
                .iter()
                .find(|(_, o)| o.phenotype.form == form)
                .map(|(_, o)| o.clone())
                .expect("a founder of each kind"),
        );
    }
    out
}

#[test]
fn the_default_kinds_place_twenty_four_founders_with_the_tables_genomes() {
    let world = World::new(WorldConfig::default()).expect("valid");
    assert_eq!(world.population(), 24, "4 + 10 + 5 + 5 founders");
    let mut by_form = [0usize; 8];
    for (_, o) in world.state.organisms.iter() {
        by_form[o.phenotype.form as usize] += 1;
        assert_eq!(o.genome.version, Genome::VERSION);
    }
    assert_eq!(
        by_form[..4],
        [10, 5, 4, 5],
        "lantern 0 = grazer, sail 1 = glider, mossback 2 = burrower, skimmer 3"
    );
    let Ok([burrower, grazer, glider, skimmer]) =
        <[Organism; 4]>::try_from(one_of_each_kind(&world))
    else {
        panic!("four kinds");
    };
    let g = |o: &Organism| {
        (
            o.genome.diet,
            o.genome.depth,
            o.genome.speed,
            o.genome.size,
            o.genome.swim,
            o.genome.hue,
        )
    };
    assert_eq!(g(&burrower), (0.10, 0.10, 0.6, 1.0, 0.0, 0.15));
    assert_eq!(g(&grazer), (0.85, 0.55, 1.0, 1.0, 0.0, 0.50));
    assert_eq!(g(&glider), (0.90, 1.00, 1.0, 1.0, 0.0, 0.85));
    assert_eq!(g(&skimmer), (0.60, 0.10, 0.9, 0.9, 1.0, 0.65));
    // Unnamed loci keep the v1 founder values, and the phenotype carries the kind.
    assert_eq!(
        (
            burrower.genome.metabolism,
            burrower.genome.mouth,
            burrower.genome.reserve
        ),
        (0.7, 1.0, 1.0),
        "the burrower kind fixes metabolism; the rest stay v1"
    );
    assert_eq!(grazer.genome.metabolism, 1.0);
    assert_eq!(
        burrower.genome.sense,
        WorldConfig::default().organism.sense_radius as f32
    );
    assert!((glider.phenotype.h_pref - 1.0).abs() < 1e-12);
    assert!((burrower.phenotype.h_pref + 0.8).abs() < 1e-6);
    assert_eq!(skimmer.phenotype.swim, 1.0);
    assert!(
        (grazer.phenotype.graze_rate - 0.85f32 as f64 * grazer.phenotype.mouth_rate).abs() < 1e-12
    );
    // The founders' material is booked and the world is consistent.
    world
        .check_invariants()
        .expect("a fresh kinds world is consistent");
    assert!(world.mass_residual().abs() < 1e-12);
}

#[test]
fn an_empty_kind_list_falls_back_to_v1_founders_and_kinds_are_deterministic() {
    let v1 = World::new(config()).expect("valid");
    assert_eq!(v1.population(), config().founders.count as usize);
    for (_, o) in v1.state.organisms.iter() {
        assert_eq!(
            (o.genome.diet, o.genome.depth, o.genome.swim),
            (0.7, 0.5, 0.0)
        );
        assert_eq!(
            o.genome.form,
            crate::genome::form_of_hue(o.genome.hue),
            "v1 founders take the hue tercile"
        );
    }
    let a = World::new(WorldConfig::default()).expect("valid");
    let b = World::new(WorldConfig::default()).expect("valid");
    assert_eq!(
        a.state, b.state,
        "two kinds worlds from one seed are identical"
    );
    // A kind's own draws do not move when another kind's count changes.
    let mut fewer = WorldConfig::default();
    fewer.founders.kinds[0].count = 2;
    let c = World::new(fewer).expect("valid");
    let gliders = |w: &World| {
        let mut v: Vec<(u32, SurfacePoint)> = w
            .state
            .organisms
            .iter()
            .filter(|(_, o)| o.phenotype.form == 1)
            .map(|(id, o)| (id.slot, o.pos))
            .collect();
        v.sort_by_key(|x| x.0);
        v.into_iter().map(|x| x.1).collect::<Vec<_>>()
    };
    assert_eq!(
        gliders(&a),
        gliders(&c),
        "glider placements are their own stream"
    );
}

#[test]
fn a_v1_genome_is_upgraded_in_place_on_load() {
    let mut world = World::new(config()).expect("valid");
    for _ in 0..5 {
        world.step();
    }
    let mut state = world.state.clone();
    for (_, o) in state.organisms.iter_mut() {
        o.genome.version = 1;
        o.genome.form = crate::genome::FORM_UNSET;
    }
    let reloaded = World::from_state(state).expect("a v1-genome state is upgraded, not refused");
    for (_, o) in reloaded.state.organisms.iter() {
        assert_eq!(o.genome.version, Genome::VERSION);
        assert_eq!(o.genome.form, crate::genome::form_of_hue(o.genome.hue));
        assert_eq!(
            o.phenotype.form, o.genome.form,
            "the phenotype is re-decoded"
        );
    }
}

/// A single organism on a frozen cell of a kinds-free world, with the given genome
/// loci, its cell stocked as asked. Returns the world, the id and the cell index.
fn frozen_feeder(diet: f32, p: f64, f: f64, d: f64, de: f64) -> (World, OrganismId, usize) {
    let mut cfg = config();
    cfg.founders.count = 1;
    cfg.producer.growth = 0.0;
    cfg.producer.mortality = 0.0;
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.fall = 0.0;
    cfg.nutrient.diffusion = 0.0;
    cfg.fruit.ripen = 0.0;
    cfg.fruit.drop = 0.0;
    cfg.water.rain_rate = 0.0;
    cfg.mechanisms.mutation = false;
    let mut world = World::new(cfg).expect("valid");
    let id = world
        .state
        .organisms
        .iter()
        .map(|(id, _)| id)
        .next()
        .expect("one founder");
    let cell = CellId::new(Face::Front, 4, 4);
    {
        let o = world.state.organisms.get_mut(id).expect("alive");
        o.genome.diet = diet;
        o.phenotype = decode(&o.genome, &world.state.config.organism);
        o.pos = cell.center();
        o.reserve = 0.0;
        o.hunger_memory = 1.0;
        o.mode = Mode::Seeking;
    }
    let c = cell.index();
    world.state.fields.p[c] = p;
    world.state.fields.f[c] = f;
    world.state.fields.d[c] = d;
    world.state.fields.de[c] = de;
    (world, id, c)
}

#[test]
fn frugivory_comes_first_and_the_diet_gates_hold() {
    // A pure grazer on a cell with fruit and producer eats the fruit first: with
    // linear intake and headroom for exactly one mouthful, the fruit bite fills it and
    // the producer, whose request comes second, gets nothing.
    let (mut world, id, c) = frozen_feeder(1.0, 1.0, 1.0, 1.0, 1.0);
    world.state.config.organism.intake_half_saturation = 0.0;
    {
        let o = world.state.organisms.get_mut(id).expect("alive");
        o.reserve = o.phenotype.reserve_max - o.phenotype.graze_rate * DT;
    }
    let (p0, f0, d0) = (
        world.state.fields.p[c],
        world.state.fields.f[c],
        world.state.fields.d[c],
    );
    // The hand-stocked cell moved the residual once; the step must not move it again.
    let residual = world.mass_residual();
    let before = stored_energy(&world.state);
    let (light, heat) = (world.state.light_in_total, world.state.heat_out_total);
    world.step();
    let o = world.state.organisms.get(id).expect("alive");
    assert_eq!(o.mode, Mode::Feeding);
    assert!(o.fed_this_tick);
    let q = f0 - world.state.fields.f[c];
    assert!(q > 0.0, "fruit was eaten");
    assert!(
        (q - o.phenotype.graze_rate * DT).abs() < 1e-12,
        "a full fruit mouthful {q}"
    );
    assert_eq!(
        world.state.fields.p[c], p0,
        "the producer waited its turn and got nothing"
    );
    // Frugivory: η_m of the bite to reserve, the rest to detritus; scavenging is gated off
    // for a pure grazer, so detritus only grew.
    let eta_m = world.config().organism.assimilation_material;
    let r0 = o.phenotype.reserve_max - o.phenotype.graze_rate * DT;
    assert!((o.reserve - (r0 + eta_m * q)).abs() < 1e-12);
    assert!((world.state.fields.d[c] - (d0 + q - eta_m * q)).abs() < 1e-12);
    let booked = (world.state.light_in_total - light) - (world.state.heat_out_total - heat);
    assert!(
        ((stored_energy(&world.state) - before) - booked).abs() < 1e-9,
        "fruit energy is audited"
    );
    assert!(
        (world.mass_residual() - residual).abs() < 1e-12,
        "frugivory conserves material"
    );

    // A pure scavenger never grazes or eats fruit, however rich the cell.
    let (mut world, id, c) = frozen_feeder(0.0, 1.0, 1.0, 0.0, 0.0);
    world.step();
    let o = world.state.organisms.get(id).expect("alive");
    assert_eq!(
        o.mode,
        Mode::Seeking,
        "no detritus, and leaf is not its food"
    );
    assert_eq!(
        (world.state.fields.p[c], world.state.fields.f[c]),
        (1.0, 1.0)
    );
    assert_eq!(o.reserve, 0.0);

    // A pure grazer never scavenges.
    let (mut world, id, c) = frozen_feeder(1.0, 0.0, 0.0, 1.0, 1.0);
    world.step();
    let o = world.state.organisms.get(id).expect("alive");
    assert_eq!(o.mode, Mode::Seeking);
    assert_eq!(world.state.fields.d[c], 1.0);
    assert_eq!(o.reserve, 0.0);

    // An omnivore below the fruit diet leaves fruit alone but grazes.
    let (mut world, id, c) = frozen_feeder(0.4, 1.0, 1.0, 0.0, 0.0);
    world.step();
    let o = world.state.organisms.get(id).expect("alive");
    assert_eq!(o.mode, Mode::Feeding);
    assert_eq!(world.state.fields.f[c], 1.0, "fruit needs diet ≥ 0.5");
    assert!(world.state.fields.p[c] < 1.0);
}

#[test]
fn fruit_is_conserved_material_over_a_default_run() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    let opening = stored_energy(&world.state);
    let mut worst: f64 = 0.0;
    for _ in 0..2000 {
        let before = stored_energy(&world.state);
        let ledgers = world.state.energy_ledgers();
        world.step();
        let booked = world.state.energy_ledgers().net_since(ledgers);
        worst = worst.max(((stored_energy(&world.state) - before) - booked).abs());
        assert!(
            world.mass_residual().abs() < 1e-9,
            "mass residual {}",
            world.mass_residual()
        );
    }
    assert!(
        worst < 1e-9,
        "worst per-tick energy drift {worst:e} with fruit in the sum"
    );
    let total_fruit: f64 = world.state.fields.f.iter().sum();
    assert!(
        total_fruit > 0.0,
        "a lit default world ripens some fruit in 100 s"
    );
    let booked = world.state.net_energy_in_corrected();
    let overall = (stored_energy(&world.state) - opening) - booked;
    assert!(
        overall.abs() < 1e-9 * stored_energy(&world.state).max(1.0),
        "cumulative {overall:e}"
    );
    // The view, the dump and telemetry all carry the same fruit.
    let view = world.render_view();
    assert_eq!(view.fruit, world.state.fields.f);
    assert_eq!(world.field_dump().f, world.state.fields.f);
    let sample = world.telemetry();
    assert!((sample.fruit - total_fruit).abs() < 1e-12);
}

#[test]
fn the_depth_term_points_up_the_side_faces_and_vanishes_on_top() {
    assert_eq!(up_direction(Face::Top), Vec2::ZERO);
    for face in [Face::Front, Face::Right, Face::Back, Face::Left] {
        let up = up_direction(face);
        assert!(
            (up - Vec2::new(0.0, -1.0)).length() < 1e-12,
            "{face:?}: {up:?}"
        );
        // Moving along `up` really raises the embedded height.
        let low = SurfacePoint::new(face, 32.0, 40.0);
        let higher = SurfacePoint::new(face, 32.0 + up.x * 4.0, 40.0 + up.y * 4.0);
        assert!(higher.embed()[1] > low.embed()[1]);
    }
    // A canopy-bound organism low on a wall heads up; a soil-bound one high up heads down.
    // No food anywhere (no producers, no litter), so nothing stops it to feed on the way.
    let mut cfg = config();
    cfg.founders.count = 1;
    cfg.producer.initial_fraction = 0.0;
    cfg.producer.growth = 0.0;
    cfg.detritus.initial_dark = 0.0;
    cfg.water.rain_rate = 0.0;
    cfg.drives.w_persist = 0.0;
    cfg.drives.turn_noise = 0.0;
    for (depth, expect_dy_sign) in [(1.0f32, -1.0f64), (0.0, 1.0)] {
        let mut world = World::new(cfg.clone()).expect("valid");
        let id = world
            .state
            .organisms
            .iter()
            .map(|(id, _)| id)
            .next()
            .expect("founder");
        let start = SurfacePoint::new(Face::Front, 32.0, 32.0);
        {
            let o = world.state.organisms.get_mut(id).expect("alive");
            o.genome.depth = depth;
            o.genome.drives.w_persist = 0.0;
            o.genome.drives.turn_noise = 0.0;
            o.phenotype = decode(&o.genome, &world.state.config.organism);
            o.pos = start;
            o.heading = Vec2::new(1.0, 0.0);
            o.reserve = 0.0;
            o.hunger_memory = 1.0;
            o.mode = Mode::Seeking;
        }
        // **R0b.** The shared budget caps a unit adult's turn at `u · dt / r` = 0.006 rad a
        // tick, so a body that starts facing along the chart needs ~260 ticks just to swing
        // the 90° onto the depth gradient, and travels almost nothing while it does. 200 ticks
        // no longer contains the manoeuvre; 1,600 (80 s) contains it with room to travel, and
        // is still well inside upkeep-only survival. Nothing about the assertion moved.
        for _ in 0..1_600 {
            world.step();
        }
        let o = world.state.organisms.get(id).expect("alive");
        let dv = o.pos.v - start.v;
        assert!(
            dv * expect_dy_sign > 0.5,
            "depth {depth}: moved {dv} in v on Front (expected sign {expect_dy_sign})"
        );
        assert!(
            (o.heading.y * expect_dy_sign) > 0.9,
            "heading {:?} settled toward the band",
            o.heading
        );
    }
}

#[test]
fn sensing_reaches_the_configured_depth_and_finds_food_two_cells_out() {
    assert_eq!(sense_depth(6.0), 2);
    assert_eq!(sense_depth(4.0), 1);
    assert_eq!(sense_depth(12.0), 3);
    assert_eq!(sense_depth(0.0), 1);
    let rings = sense_rings(&FieldGraph::new());
    let origin = CellId::new(Face::Front, 8, 8);
    assert_eq!(rings[origin.index()][0].len(), 4);
    assert_eq!(rings[origin.index()][1].len(), 8);
    assert_eq!(rings[origin.index()][2].len(), 12);
    for (d, ring) in rings[origin.index()].iter().enumerate() {
        for c in ring {
            let dist = (i32::from(c.cx()) - 8).abs() + (i32::from(c.cy()) - 8).abs();
            assert_eq!(
                dist as usize,
                d + 1,
                "{c:?} is not at graph distance {}",
                d + 1
            );
        }
    }
    // A seam-adjacent cell's rings cross onto the neighbouring face and never the rim.
    let corner = CellId::new(Face::Front, 15, 15);
    assert!(
        rings[corner.index()][0]
            .iter()
            .any(|c| c.face() == Face::Right)
    );
    assert!(
        rings[corner.index()]
            .iter()
            .flatten()
            .all(|c| c.face() != Face::Top)
    );

    // Food two cells away, none adjacent: a 6 px sensor turns toward it; a 4 px one
    // sees a flat neighbourhood and holds its heading.
    let mut cfg = config();
    cfg.founders.count = 1;
    cfg.producer.initial_fraction = 0.0;
    cfg.producer.growth = 0.0;
    // Freeze everything that could leak a trace of the rich cell into the one-hop
    // neighbourhood before the observation (mortality → detritus → fall; ripening):
    // gradients are normalized, so any nonzero difference steers at full strength.
    cfg.producer.mortality = 0.0;
    cfg.detritus.fall = 0.0;
    cfg.detritus.initial_dark = 0.0;
    cfg.fruit.ripen = 0.0;
    cfg.water.rain_rate = 0.0;
    for (sense, turns) in [(6.0f32, true), (4.0, false)] {
        let mut world = World::new(cfg.clone()).expect("valid");
        let id = world
            .state
            .organisms
            .iter()
            .map(|(id, _)| id)
            .next()
            .expect("founder");
        let here = CellId::new(Face::Front, 8, 8);
        {
            let o = world.state.organisms.get_mut(id).expect("alive");
            o.genome.sense = sense;
            o.genome.depth = 0.5;
            o.genome.drives.w_persist = 0.0;
            o.genome.drives.turn_noise = 0.0;
            o.genome.drives.w_depth = 0.0;
            o.phenotype = decode(&o.genome, &world.state.config.organism);
            o.pos = here.center();
            o.heading = Vec2::new(1.0, 0.0);
            o.reserve = 0.0;
            o.hunger_memory = 1.0;
            o.mode = Mode::Seeking;
        }
        // Rich cells straight "up" the chart, two hops away, nothing at one hop.
        world.state.fields.p[CellId::new(Face::Front, 8, 6).index()] = 1.5;
        let (extent, speed_max) = {
            let o = world.state.organisms.get(id).expect("alive");
            (o.phenotype.extent, o.phenotype.speed_max)
        };
        world.step();
        let o = world.state.organisms.get(id).expect("alive");
        if turns {
            // **R0b.** One tick of turning is now at most `u · dt / r` — 0.006 rad for this
            // body, against the 0.0785 rad its genome's angular ceiling alone allowed — so the
            // old `y < −0.05` described the pre-R0b envelope, not the sensor. What the sensor
            // decides is the *direction*, and that is what is asserted: it turned the right
            // way, and it turned as hard as the shared budget permits.
            let ceiling = speed_max * crate::DT / extent;
            assert!(
                o.heading.y < 0.0 && o.heading.y.abs() > 0.8 * ceiling,
                "a 6 px sensor turned toward food two cells up: {:?} (tick ceiling {ceiling})",
                o.heading
            );
            assert!(
                o.heading.y.abs() <= ceiling * (1.0 + 1e-9),
                "it turned past the envelope: {:?}",
                o.heading
            );
        } else {
            assert!(
                (o.heading - Vec2::new(1.0, 0.0)).length() < 1e-9,
                "a 4 px sensor saw nothing: {:?}",
                o.heading
            );
        }
    }
}

#[test]
fn a_swimmer_ignores_pools_while_a_wader_is_slowed() {
    /// The motor budget `|v| + r·|ω|` this tick, in px. **R0b:** wading divides that whole
    /// budget, not travel alone — see `wading_halves_the_motor_budget_at_unit_depth`.
    fn traveled(swim: f32, depth: f64) -> f64 {
        let mut cfg = config();
        cfg.founders.count = 1;
        cfg.water.rain_rate = 0.0;
        cfg.water.flow = 0.0;
        cfg.water.evap = 0.0;
        let mut world = World::new(cfg).expect("valid");
        let id = world
            .state
            .organisms
            .iter()
            .map(|(id, _)| id)
            .next()
            .expect("founder");
        let cell = {
            let o = world.state.organisms.get_mut(id).expect("alive");
            o.genome.swim = swim;
            o.phenotype = decode(&o.genome, &world.state.config.organism);
            o.mode = Mode::Seeking;
            o.hunger_memory = 1.0;
            o.reserve = 0.0;
            cell_of(&o.pos)
        };
        world.state.fields.w[cell.index()] = depth;
        world.state.fields.p[cell.index()] = 0.0;
        world.state.fields.f[cell.index()] = 0.0;
        world.state.fields.d[cell.index()] = 0.0;
        let (before, extent) = {
            let o = world.state.organisms.get(id).expect("alive");
            (o.heading, o.phenotype.extent)
        };
        world.step();
        let after = world.state.organisms.get(id).expect("alive").heading;
        let turn = {
            use std::f64::consts::{PI, TAU};
            let d = after.screen_angle() - before.screen_angle();
            (d + PI).rem_euclid(TAU) - PI
        };
        let travel: f64 = world.render_view().organisms[0]
            .moved
            .iter()
            .map(|s| s.length())
            .sum();
        travel + extent * turn.abs()
    }
    let dry = traveled(0.0, 0.0);
    assert!(
        (dry / traveled(0.0, 1.0) - 2.0).abs() < 1e-9,
        "a wader halves at unit depth"
    );
    assert!(
        (traveled(1.0, 1.0) - dry).abs() < 1e-12,
        "a swimmer moves as if dry"
    );
    assert!((traveled(1.0, 3.0) - dry).abs() < 1e-12);
    assert!(
        (dry / traveled(0.5, 1.0) - 1.5).abs() < 1e-9,
        "half a swimmer wades at 1 + w/2"
    );
}

#[test]
fn mutation_records_its_loci_and_never_touches_form() {
    let mut cfg = WorldConfig::default();
    cfg.mutation.probability = 1.0;
    let mut world = World::new(cfg).expect("valid");
    let (mut births, mut mutated, mut loci) = (0u64, 0u64, std::collections::HashSet::new());
    for _ in 0..24_000 {
        world.step();
        for event in world.drain_events() {
            if let LifeEvent::Birth {
                id,
                parent,
                mutations,
                ..
            } = event
            {
                births += 1;
                let child = world.state.organisms.get(id).expect("newborn").clone();
                // Copies are exact where no mutation is recorded, and the parent (if it
                // still lives) shares the child's rig whatever else changed.
                if let Some(p) = world.state.organisms.get(parent) {
                    assert_eq!(child.genome.form, p.genome.form, "form must never mutate");
                    assert_eq!(child.phenotype.form, p.phenotype.form);
                }
                if !mutations.is_empty() {
                    mutated += 1;
                }
                for m in &mutations {
                    assert!(
                        crate::genome::MUTABLE_LOCI.contains(&m.locus),
                        "{} is not mutable",
                        m.locus
                    );
                    assert_ne!(m.from, m.to);
                    loci.insert(m.locus);
                }
                assert!(
                    mutations.len() <= 2,
                    "at most two loci per birth: {mutations:?}"
                );
                let mut g = child.genome.clone();
                assert!(!g.clamp(), "a mutated child is always in range");
            }
        }
    }
    assert!(births >= 20, "the run produced {births} births");
    assert!(
        mutated as f64 >= 0.8 * births as f64,
        "with p_mut = 1 nearly every child differs ({mutated}/{births})"
    );
    assert!(
        loci.len() >= 5,
        "many different loci were touched: {loci:?}"
    );
    world
        .check_invariants()
        .expect("mutated worlds stay consistent");

    // With mutation off every child is an exact copy of its parent's escrowed genome.
    let mut cfg = WorldConfig::default();
    cfg.mechanisms.mutation = false;
    let mut world = World::new(cfg).expect("valid");
    let mut seen = 0;
    for _ in 0..24_000 {
        world.step();
        for event in world.drain_events() {
            if let LifeEvent::Birth {
                id,
                parent,
                mutations,
                ..
            } = event
            {
                assert!(mutations.is_empty());
                if let Some(p) = world.state.organisms.get(parent) {
                    assert_eq!(
                        world.state.organisms.get(id).expect("newborn").genome,
                        p.genome
                    );
                    seen += 1;
                }
            }
        }
    }
    assert!(seen > 0, "some exact copies were compared");
}

#[test]
fn telemetry_counts_each_form_and_its_mean_height() {
    let mut world = World::new(WorldConfig::default()).expect("valid");
    world.step();
    let sample = world.telemetry();
    assert_eq!(
        sample.population_by_form.iter().sum::<u32>(),
        sample.population
    );
    assert_eq!(sample.population_by_form[..4], [10, 5, 4, 5]);
    for form in 0..4 {
        let expected: f64 = world
            .state
            .organisms
            .iter()
            .filter(|(_, o)| o.phenotype.form == form as u8)
            .map(|(_, o)| o.pos.embed()[1])
            .sum::<f64>()
            / f64::from(sample.population_by_form[form]);
        assert!((sample.mean_height_by_form[form] - expected).abs() < 1e-12);
        assert!(sample.mean_height_by_form[form].abs() <= 1.0);
    }
    assert_eq!(
        sample.mean_height_by_form[7], 0.0,
        "an empty form reports zero"
    );
    let view = world.render_view();
    assert!(view.organisms.iter().all(|o| o.form < 4));
}

/// Not a test of anything: the fauna v2 short-run reporter for the slice report.
/// `CUBARIUM_SEED=1 CUBARIUM_HOURS=2 cargo test -p cubarium-core --release --lib
/// report_fauna_by_form -- --ignored --nocapture`.
#[test]
#[ignore]
fn report_fauna_by_form_after_a_short_run() {
    let seed: u64 = std::env::var("CUBARIUM_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let hours: f64 = std::env::var("CUBARIUM_HOURS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2.0);
    let mut cfg = WorldConfig {
        seed,
        ..WorldConfig::default()
    };
    // Diagnostic counterfactual only (never a default): `CUBARIUM_ENERGY_CAP` overrides
    // `detritus.energy_cap`, which sets how edible detritus can be.
    if let Some(cap) = std::env::var("CUBARIUM_ENERGY_CAP")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
    {
        cfg.detritus.energy_cap = cap;
        println!("counterfactual: detritus.energy_cap = {cap}");
    }
    let e_r = cfg.organism.reserve_energy_density;
    let mut world = World::new(cfg).expect("valid");
    let names = [
        "lantern/grazer",
        "sail/glider",
        "mossback/burrower",
        "skimmer",
    ];
    // Edible litter on the soil floor at tick 0, against the feeding threshold.
    {
        let f = &world.state.fields;
        let floor: Vec<f64> = CellId::all()
            .filter(|c| c.face() != Face::Top && c.cy() == 15)
            .map(|c| edible_detritus(f.d[c.index()], f.de[c.index()], e_r))
            .collect();
        let mean = floor.iter().sum::<f64>() / floor.len() as f64;
        let max = floor.iter().cloned().fold(0.0, f64::max);
        let feed_min = world.config().drives.feed_min;
        println!(
            "tick 0 soil floor row: mean D_eff {mean:.3}, max {max:.3}, {} of 64 cells at or above feed_min {feed_min}",
            floor.iter().filter(|&&x| x >= feed_min).count()
        );
    }
    let (mut pop_min, mut pop_max) = (world.population(), world.population());
    let mut min_by_form = [u32::MAX; 4];
    let mut form_of: std::collections::HashMap<OrganismId, u8> = world
        .state
        .organisms
        .iter()
        .map(|(id, o)| (id, o.phenotype.form))
        .collect();
    let mut deaths_by_form = [[0u32; 3]; 4];
    let mut death_time_by_form = [0.0f64; 4];
    let mut death_age_by_form = [0.0f64; 4];
    // `CUBARIUM_TRACE_FORM=<form>` prints that kind's state once a simulated minute for
    // the first half hour: where it is, what it holds, what it stands on.
    let trace: Option<u8> = std::env::var("CUBARIUM_TRACE_FORM")
        .ok()
        .and_then(|s| s.parse().ok());
    let ticks = (hours * 3600.0 / DT) as u64;
    for tick in 0..ticks {
        if let Some(form) = trace
            && tick % 1200 == 0
            && tick <= 36_000
        {
            let f = &world.state.fields;
            let members: Vec<&Organism> = world
                .state
                .organisms
                .iter()
                .filter(|(_, o)| o.phenotype.form == form)
                .map(|(_, o)| o)
                .collect();
            if !members.is_empty() {
                let n = members.len() as f64;
                let mean =
                    |g: &dyn Fn(&Organism) -> f64| members.iter().map(|o| g(o)).sum::<f64>() / n;
                let modes = members.iter().fold([0; 3], |mut m, o| {
                    m[match o.mode {
                        Mode::Resting => 0,
                        Mode::Seeking => 1,
                        Mode::Feeding => 2,
                    }] += 1;
                    m
                });
                println!(
                    "    t {:>4.0}s form {form}: n {} h {:+.2} R/Rmax {:.2} E/Emax {:.2} m_h {:.2} modes rest/seek/feed {:?} D_eff here {:.3} P here {:.3} fed {}",
                    tick as f64 * DT,
                    members.len(),
                    mean(&|o| o.pos.embed()[1]),
                    mean(&|o| o.reserve / o.phenotype.reserve_max),
                    mean(&|o| o.energy / o.phenotype.energy_max),
                    mean(&|o| o.hunger_memory),
                    modes,
                    mean(&|o| {
                        let c = cell_of(&o.pos).index();
                        edible_detritus(f.d[c], f.de[c], e_r)
                    }),
                    mean(&|o| f.p[cell_of(&o.pos).index()]),
                    members.iter().filter(|o| o.fed_this_tick).count(),
                );
            }
        }
        world.step();
        for event in world.drain_events() {
            match event {
                LifeEvent::Birth { id, .. } => {
                    if let Some(o) = world.state.organisms.get(id) {
                        form_of.insert(id, o.phenotype.form);
                    }
                }
                LifeEvent::Death {
                    id,
                    cause,
                    age_ticks,
                    ..
                } => {
                    if let Some(&form) = form_of.get(&id)
                        && (form as usize) < 4
                    {
                        let slot = match cause {
                            DeathCause::Starvation => 0,
                            DeathCause::Age => 1,
                            DeathCause::Collapse => 2,
                            // This fixture runs no hunters.
                            DeathCause::Predation => continue,
                        };
                        deaths_by_form[form as usize][slot] += 1;
                        death_time_by_form[form as usize] += (tick + 1) as f64 * DT;
                        death_age_by_form[form as usize] += age_ticks as f64 * DT;
                    }
                }
            }
        }
        pop_min = pop_min.min(world.population());
        pop_max = pop_max.max(world.population());
        if tick % 100 == 0 {
            let mut by_form = [0u32; 4];
            for (_, o) in world.state.organisms.iter() {
                if (o.phenotype.form as usize) < 4 {
                    by_form[o.phenotype.form as usize] += 1;
                }
            }
            for f in 0..4 {
                min_by_form[f] = min_by_form[f].min(by_form[f]);
            }
        }
    }
    let sample = world.telemetry();
    println!(
        "seed {seed}, {hours} h: population end {} min {pop_min} max {pop_max}; residual {:e}",
        sample.population, sample.mass_residual
    );
    for f in 0..4 {
        let deaths: u32 = deaths_by_form[f].iter().sum();
        let mean_death = if deaths > 0 {
            death_time_by_form[f] / f64::from(deaths) / 60.0
        } else {
            0.0
        };
        let mean_age = if deaths > 0 {
            death_age_by_form[f] / f64::from(deaths) / 60.0
        } else {
            0.0
        };
        println!(
            "  {:<18} end {:>3}  min {:>3}  mean height {:+.3}  starved {:>3} (age/collapse {} / {}), mean age at death {mean_age:.1} min, mean death time {mean_death:.1} min",
            names[f],
            sample.population_by_form[f],
            min_by_form[f],
            sample.mean_height_by_form[f],
            deaths_by_form[f][0],
            deaths_by_form[f][1],
            deaths_by_form[f][2]
        );
    }
    let fields = &world.state.fields;
    let (mut soil, mut foliage, mut canopy) = (0.0, 0.0, 0.0);
    for cell in CellId::all() {
        let f = fields.f[cell.index()];
        if cell.face() == Face::Top {
            canopy += f;
        } else if cell.center().embed()[1] < -0.33 {
            soil += f;
        } else {
            foliage += f;
        }
    }
    let fruiting = fields.f.iter().filter(|&&x| x > 0.15).count();
    let ripe_cells = fields
        .p
        .iter()
        .filter(|&&p| p > world.config().fruit.fruit_min * world.config().producer.max)
        .count();
    let max_f = fields.f.iter().cloned().fold(0.0, f64::max);
    let max_p = fields.p.iter().cloned().fold(0.0, f64::max);
    println!(
        "  fruit total {:.3} ({fruiting} cells above 0.15, max F {max_f:.3}; {ripe_cells} cells with P above fruit_min·P_max, max P {max_p:.3}): soil {:.3} foliage {:.3} canopy {:.3}; P {:.1} D {:.1} N {:.1} water {:.1}",
        sample.fruit,
        soil,
        foliage,
        canopy,
        sample.producer,
        sample.detritus,
        sample.nutrient,
        sample.water
    );
}
