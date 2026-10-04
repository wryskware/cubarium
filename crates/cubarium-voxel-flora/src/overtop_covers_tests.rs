//! `SpeciesConfig::overtop_covers` (plant succession, `design/handoffs/plant-succession-study-2026-10-03.md`,
//! Astra's finding 2): a seedling comes up through a ground cover only if its list allows
//! that cover — read when the site is armed, again when it sprouts — and an empty list is
//! the woody rule, any ground cover.
//!
//! One soil slab at field capacity in open sky, one site, a cover stand planted on it and
//! one seed landed under it. Every probability is 0 or 1, so each fixture is a fixed
//! sequence of checks: the first arms, the next sprouts.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};

use crate::seeds::{add_cohort, check_period, check_phase, package_of};
use crate::{Command, Flora, FloraConfig, Ground, Site, Species};

const SITE: Site = Site { x: 1, y: 2, z: 0 };
const TURF: Species = Species::Springturf;
const PAD: Species = Species::Velvetpad;
const STONE: Species = Species::Stonecushion;
const BLOOM: Species = Species::Bloomcrown;

/// Three soil columns at field capacity: bedrock at `y = 0`, soil at `y = 1..=2`, support
/// faces at `y = 2` in open sky.
fn slab() -> World {
    let mut w = World::empty(VoxelConfig {
        width: 3,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 11,
        ..VoxelConfig::default()
    });
    let pore = cubarium_voxel::Materials::DEFAULT.field_capacity(Material::Soil);
    let cap = w.config().materials.pore_capacity(Material::Soil) * w.config().voxel_volume();
    for x in 0..3i64 {
        for y in 1..=2u32 {
            w.apply(WorldCommand::AddWater {
                x,
                y,
                z: 0,
                volume_m3: pore * cap,
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

/// Springturf may come up through velvetpad only, at probability 1; nothing's seeds die.
fn config() -> FloraConfig {
    let mut c = FloraConfig::default();
    for sp in Species::ALL {
        c.species_mut(sp).seed_attrition_per_s = 0.0;
    }
    c.springturf.overtop_per_check = 1.0;
    c.springturf.overtop_covers = vec![PAD];
    c
}

/// Plant `cover` on [`SITE`] at its full wood.
fn plant(flora: &mut Flora, world: &World, cover: Species) -> u64 {
    let wood = flora.config.species(cover).wood_max;
    assert!(
        flora.apply(
            world,
            Command::SeedOnFace {
                site: SITE,
                species: cover,
                wood,
            }
        ),
        "{cover:?} planted"
    );
    flora.view().stand_at(SITE).expect("planted").id
}

/// Land one seed of `species` on [`SITE`] through the landing path (`add_cohort`), booked
/// as seeded material so the ledgers stay closed.
fn land(flora: &mut Flora, species: Species) {
    let sc = flora.config.species(species).clone();
    let organic = package_of(&sc);
    let mineral = sc.n_tissue * organic;
    let gi = match flora.ground.binary_search_by_key(&SITE, |g| g.site) {
        Ok(i) => i,
        Err(i) => {
            flora.ground.insert(i, Ground::new(SITE, 1.0));
            flora.ledger.seeded_mineral_in += 1.0;
            i
        }
    };
    let tick = flora.tick;
    add_cohort(&mut flora.ground[gi], species, organic, mineral, tick, &sc);
    flora.ledger.seeded_organic_in += organic;
    flora.ledger.seeded_mineral_in += mineral;
    flora.ledger.seeded_energy_in += sc.energy_density * organic;
    flora.bank_wheel.clear();
}

/// The first tick after `after` on which [`SITE`] is checked.
fn next_check(flora: &Flora, after: u64) -> u64 {
    let period = check_period(&flora.config);
    let phase = check_phase(SITE, period);
    let t = after - after % period + phase;
    if t > after { t } else { t + period }
}

/// Step to [`SITE`]'s next check, through it.
fn to_next_check(flora: &mut Flora, world: &mut World) {
    let t = next_check(flora, flora.tick);
    while flora.tick < t {
        flora.step(world);
    }
}

fn armed(flora: &Flora) -> Option<Species> {
    flora.view().ground_at(SITE).and_then(|g| g.sprouting)
}

fn banked(flora: &Flora, species: Species) -> bool {
    flora
        .view()
        .ground_at(SITE)
        .is_some_and(|g| g.seed_organic(species) > 0.0)
}

fn occupant(flora: &Flora) -> Option<(Species, u64)> {
    flora.view().stand_at(SITE).map(|s| (s.species, s.id))
}

/// A cover on the site, one seed of `seed` under it.
fn fixture(config: FloraConfig, cover: Species, seed: Species) -> (Flora, World, u64) {
    let mut world = slab();
    let mut flora = Flora::new(config);
    let id = plant(&mut flora, &world, cover);
    land(&mut flora, seed);
    flora.step(&mut world);
    (flora, world, id)
}

/// **Allowed**: at probability 1 a springturf seed under velvetpad arms at one check and
/// comes up at the next; the velvetpad dies an overtopped death and the event is counted
/// as velvetpad → springturf.
#[test]
fn springturf_at_one_comes_up_through_velvetpad_at_the_second_check() {
    let (mut flora, mut world, _) = fixture(config(), PAD, TURF);
    to_next_check(&mut flora, &mut world);
    assert_eq!(armed(&flora), Some(TURF), "armed at the first check");
    assert_eq!(occupant(&flora).map(|o| o.0), Some(PAD), "the pad still holds");
    to_next_check(&mut flora, &mut world);
    assert_eq!(occupant(&flora).map(|o| o.0), Some(TURF), "the turf came up");
    let l = flora.view().ledger;
    assert_eq!(l.overtopped[PAD.index()], 1);
    assert_eq!(l.overtop_pairs[PAD.index()][TURF.index()], 1, "counted as velvetpad -> springturf");
    assert_eq!(l.seeds_germinated[TURF.index()], 1);
    assert!(!banked(&flora, TURF), "the seed was spent");
    assert_eq!(armed(&flora), None);
}

/// **Forbidden covers**: a list of `["velvetpad"]` does not let springturf through
/// stonecushion or through springturf itself, and a stand that is not a ground cover
/// (bloomcrown) holds its site against every seed. The seed waits, unspent.
#[test]
fn springturf_does_not_come_up_through_a_cover_its_list_leaves_out() {
    for cover in [STONE, TURF, BLOOM] {
        let (mut flora, mut world, id) = fixture(config(), cover, TURF);
        for _ in 0..4 {
            to_next_check(&mut flora, &mut world);
            assert_eq!(armed(&flora), None, "{cover:?}: never armed");
        }
        assert_eq!(occupant(&flora), Some((cover, id)), "{cover:?} still holds its site");
        assert!(banked(&flora, TURF), "{cover:?}: the seed waits");
        let l = flora.view().ledger;
        assert_eq!(l.overtopped, [0; Species::COUNT], "{cover:?}");
        // (A springturf cover's own seed may germinate on the bare columns beside it.)
        assert_eq!(l.overtop_pairs, [[0; Species::COUNT]; Species::COUNT], "{cover:?}");
    }
}

/// **The occupant changes after arming**: a site armed under velvetpad whose velvetpad is
/// replaced by stonecushion before the sprouting check does not sprout, and the arm goes.
#[test]
fn an_arm_taken_under_velvetpad_does_not_sprout_through_a_stonecushion_that_replaced_it() {
    let (mut flora, mut world, _) = fixture(config(), PAD, TURF);
    to_next_check(&mut flora, &mut world);
    assert_eq!(armed(&flora), Some(TURF), "armed under the pad");
    assert!(flora.apply(&world, Command::Clear { x: 1, z: 0 }), "the pad goes");
    let id = plant(&mut flora, &world, STONE);
    to_next_check(&mut flora, &mut world);
    assert_eq!(occupant(&flora), Some((STONE, id)), "the stonecushion holds");
    assert_eq!(armed(&flora), None, "the arm is cleared");
    assert!(banked(&flora, TURF), "the seed waits");
    assert_eq!(flora.view().ledger.overtopped, [0; Species::COUNT]);
}

/// **The gates close between arming and sprouting**: the root box dried below the turf's
/// establishment water — no sprout, the arm goes, the pad holds.
#[test]
fn an_arm_under_velvetpad_whose_gates_shut_does_not_sprout() {
    let (mut flora, mut world, id) = fixture(config(), PAD, TURF);
    to_next_check(&mut flora, &mut world);
    assert_eq!(armed(&flora), Some(TURF));
    let cap = world.config().materials.pore_capacity(Material::Soil) * world.config().voxel_volume();
    for x in 0..3i64 {
        for y in 1..=2u32 {
            world.apply(WorldCommand::WithdrawPore {
                x,
                y,
                z: 0,
                volume_m3: cap,
            });
        }
    }
    to_next_check(&mut flora, &mut world);
    assert_eq!(occupant(&flora), Some((PAD, id)), "the pad holds");
    assert_eq!(armed(&flora), None);
    assert_eq!(flora.view().ledger.overtopped, [0; Species::COUNT]);
}

/// **The seed is lost between arming and sprouting**: the bank removed — no sprout, no
/// overtopped death, the pad holds.
#[test]
fn an_arm_under_velvetpad_whose_seed_is_gone_does_not_sprout() {
    let (mut flora, mut world, id) = fixture(config(), PAD, TURF);
    to_next_check(&mut flora, &mut world);
    assert_eq!(armed(&flora), Some(TURF));
    assert!(flora.apply(
        &world,
        Command::ClearBank {
            x: 1,
            z: 0,
            species: TURF
        }
    ));
    to_next_check(&mut flora, &mut world);
    assert_eq!(occupant(&flora), Some((PAD, id)), "the pad holds");
    assert_eq!(armed(&flora), None);
    let l = flora.view().ledger;
    assert_eq!(l.overtopped, [0; Species::COUNT]);
    assert_eq!(l.seeds_germinated[TURF.index()], 0);
}

/// **Probability 0 still means never**, whatever the list says.
#[test]
fn springturf_at_zero_never_comes_up_through_velvetpad() {
    let mut c = config();
    c.springturf.overtop_per_check = 0.0;
    let (mut flora, mut world, id) = fixture(c, PAD, TURF);
    for _ in 0..4 {
        to_next_check(&mut flora, &mut world);
    }
    assert_eq!(occupant(&flora), Some((PAD, id)));
    assert_eq!(armed(&flora), None);
}

/// **Woody replacement unchanged**: bloomcrown's empty list is any ground cover — it
/// comes up through velvetpad, stonecushion and springturf alike, each counted under its
/// own pair — and at 0 it never does.
#[test]
fn bloomcrown_with_an_empty_list_comes_up_through_every_ground_cover() {
    for cover in [PAD, STONE, TURF] {
        let mut c = config();
        c.bloomcrown.overtop_per_check = 1.0;
        assert!(c.bloomcrown.overtop_covers.is_empty(), "the woody default is the wildcard");
        let (mut flora, mut world, _) = fixture(c, cover, BLOOM);
        to_next_check(&mut flora, &mut world);
        assert_eq!(armed(&flora), Some(BLOOM), "{cover:?}: armed");
        to_next_check(&mut flora, &mut world);
        assert_eq!(occupant(&flora).map(|o| o.0), Some(BLOOM), "{cover:?}: came up");
        let l = flora.view().ledger;
        assert_eq!(l.overtopped[cover.index()], 1, "{cover:?}");
        assert_eq!(l.overtop_pairs[cover.index()][BLOOM.index()], 1, "{cover:?}");
    }
    let mut c = config();
    c.bloomcrown.overtop_per_check = 0.0;
    let (mut flora, mut world, id) = fixture(c, PAD, BLOOM);
    for _ in 0..4 {
        to_next_check(&mut flora, &mut world);
    }
    assert_eq!(occupant(&flora), Some((PAD, id)), "at 0 never");
}

/// The list reads and writes as species names, an unknown name is refused, and the
/// registry sets it by path.
#[test]
fn overtop_covers_is_a_list_of_species_names() {
    let c: crate::SpeciesConfig = toml::from_str("overtop_covers = [\"velvetpad\", \"stonecushion\"]").expect("parses");
    assert_eq!(c.overtop_covers, vec![PAD, STONE]);
    assert!(toml::from_str::<crate::SpeciesConfig>("overtop_covers = [\"moss\"]").is_err());
    let text = toml::to_string(&config().springturf).expect("writes");
    assert!(text.contains("overtop_covers = [\"velvetpad\"]"), "{text}");
    let mut f = FloraConfig::default();
    crate::registry()
        .set(
            &mut f,
            "flora.springturf.overtop_covers",
            toml::Value::Array(vec![
                toml::Value::String("velvetpad".into()),
                toml::Value::String("stonecushion".into()),
            ]),
        )
        .expect("set by path");
    assert_eq!(f.springturf.overtop_covers, vec![PAD, STONE]);
}

/// The study's attempt trace and the replacement's food readouts: a traced springturf
/// seed under velvetpad is banked at two checks, passes its gates at both, arms at the
/// first and comes up through the cover at the second; under stonecushion every check is
/// a forbidden cover. The overtopped pad's whole material is booked as the pulse.
#[test]
fn the_attempt_trace_follows_one_seed_through_arming_and_birth() {
    let mut world = slab();
    let mut flora = Flora::new(config());
    plant(&mut flora, &world, PAD);
    let pad = flora.view().stand_at(SITE).expect("pad").material();
    land(&mut flora, TURF);
    flora.trace_attempts(Some(TURF));
    to_next_check(&mut flora, &mut world);
    to_next_check(&mut flora, &mut world);
    let r = flora.attempts().expect("tracing").sites[&SITE];
    assert_eq!(
        (r.bank_checks, r.gate_pass, r.arms, r.births_through_cover, r.births_bare),
        (2, 2, 1, 1, 0),
        "{r:?}"
    );
    assert_eq!((r.lottery_lost, r.draw_failed, r.arm_lost, r.cover_forbidden), (0, 0, 0, 0));
    let booked = flora.view().ledger.overtopped_organic[PAD.index()];
    assert!(booked > 0.5 * pad, "{booked}: the pad held {pad} when planted");

    let (mut flora, mut world, _) = fixture(config(), STONE, TURF);
    flora.trace_attempts(Some(TURF));
    for _ in 0..3 {
        to_next_check(&mut flora, &mut world);
    }
    let r = flora.attempts().expect("tracing").sites[&SITE];
    assert_eq!((r.bank_checks, r.cover_forbidden, r.gate_pass, r.arms), (3, 3, 0, 0), "{r:?}");
}
