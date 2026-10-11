//! Springturf spread (`design/handoffs/springturf-spread-2026-10-04.md`): runners rooting
//! through a ground cover, burrs on animals, the WALD wind kernel, curing in drought, and
//! velvetpad's saturation ceiling. Every test builds its inputs by hand or reads its
//! expected values from the config: no tuned number is pinned.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};

use crate::seeds::{ln_gamma, package_of, retention_distance, wald_distance, wald_offset, weibull_scale};
use crate::step::{DOMAIN_WIND, Rng};
use crate::{Carrier, Command, DeathCause, Flora, FloraConfig, Site, Species, Via, establishment_gates_with_sky};

const TURF: Species = Species::Springturf;
const PAD: Species = Species::Velvetpad;
const STONE: Species = Species::Stonecushion;

/// `width` soil columns at field capacity, one deep: bedrock at `y = 0`, soil at `y = 1..=2`,
/// support faces at `y = 2` in open sky, 1 m voxels.
fn slab(width: u32) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 7,
        ..VoxelConfig::default()
    });
    let pore = cubarium_voxel::Materials::DEFAULT.field_capacity(Material::Soil);
    let cap = w.config().materials.pore_capacity(Material::Soil) * w.config().voxel_volume();
    for x in 0..i64::from(width) {
        for y in 1..=2u32 {
            w.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: pore * cap });
            w.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
        }
    }
    w
}

/// Fill every soil voxel's pore to `fraction` of its capacity more (free water added to
/// the voxel as air, then turned back to soil, which keeps it as pore water).
fn add_pore(world: &mut World, fraction: f64) {
    let cap = world.config().materials.pore_capacity(Material::Soil) * world.config().voxel_volume();
    for x in 0..i64::from(world.config().width) {
        for y in 1..=2u32 {
            world.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Air });
            world.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: fraction * cap });
            world.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
        }
    }
}

fn face(x: u32) -> Site {
    Site { x, y: 2, z: 0 }
}

fn plant(flora: &mut Flora, world: &World, x: u32, species: Species) {
    let wood = flora.config.species(species).wood_max;
    assert!(
        flora.apply(world, Command::SeedOnFace { site: face(x), species, wood }),
        "{species:?} planted at {x}"
    );
}

fn residuals(flora: &Flora) -> (f64, f64, f64) {
    let v = flora.view();
    (
        v.organic() - v.ledger.expected_organic(),
        v.mineral() - v.ledger.expected_mineral(),
        v.energy() - v.ledger.expected_energy(),
    )
}

fn assert_conserved(flora: &Flora, when: &str) {
    let v = flora.view();
    let (o, n, e) = residuals(flora);
    assert!(o.abs() <= 1e-9 * v.organic().max(1.0), "{when}: organic {o}");
    assert!(n.abs() <= 1e-9 * v.mineral().max(1.0), "{when}: mineral {n}");
    assert!(e.abs() <= 1e-9 * v.energy().max(1.0), "{when}: energy {e}");
}

/// A config where nothing but the turf reproduces, and nothing's seeds die.
fn quiet() -> FloraConfig {
    let mut c = FloraConfig::default();
    for sp in Species::ALL {
        let s = c.species_mut(sp);
        s.seed_attrition_per_s = 0.0;
        s.propagule_rate = 0.0;
        s.burr_pickup = 0.0;
        s.cure_after_s = 0.0;
    }
    c
}

// ------------------------------------------------------------------------ runners

/// A turf donor at x = 0 of a three-column ring with `left`/`right` covers on its two
/// neighbours; every package a runner, reach one column.
fn runner_world(config: FloraConfig, cover: Species) -> (Flora, World) {
    let world = slab(3);
    let mut flora = Flora::new(config);
    plant(&mut flora, &world, 0, TURF);
    plant(&mut flora, &world, 1, cover);
    plant(&mut flora, &world, 2, cover);
    (flora, world)
}

fn runner_config() -> FloraConfig {
    let mut c = quiet();
    let t = &mut c.springturf;
    t.propagule_rate = FloraConfig::default().springturf.propagule_rate.max(1e-3);
    t.clonal_share = 1.0;
    t.runner_reach = 1;
    t.runners_overtop = true;
    t.overtop_covers = vec![PAD];
    c
}

/// Ticks for the turf donor to fill `n` packages at its rate, with room to spare.
fn ticks_for_packages(c: &FloraConfig, n: f64) -> u64 {
    let sc = &c.springturf;
    let per_tick = sc.propagule_rate * crate::DT / (1.0 + sc.build);
    (n * package_of(sc) / per_tick).ceil() as u64 + 20
}

/// **Allowed**: a runner roots on a velvetpad face where the turf's gates pass; the pad
/// dies the ordinary death, booked as a cover death by a runner, and the turf daughter
/// is the parent's paid package. Every ledger closes.
#[test]
fn a_runner_roots_through_an_allowed_cover_and_the_displaced_pad_is_booked() {
    let c = runner_config();
    let (mut flora, mut world) = runner_world(c.clone(), PAD);
    for _ in 0..ticks_for_packages(&c, 1.0) {
        flora.step(&mut world);
        assert_conserved(&flora, "runner");
        if flora.view().ledger.runner_displaced[PAD.index()] > 0 {
            break;
        }
    }
    let l = flora.view().ledger;
    assert_eq!(l.runner_displaced[PAD.index()], 1, "one pad displaced");
    assert_eq!(l.overtopped[PAD.index()], 1);
    assert_eq!(l.overtop_pairs[PAD.index()][TURF.index()], 1);
    assert_eq!(l.deaths_by_cause[DeathCause::Displaced.index()][PAD.index()], 1);
    assert_eq!(l.clonal_births[TURF.index()], 1, "the daughter is a runner");
    let turf: Vec<u32> = flora.view().stands.iter().filter(|s| s.species == TURF).map(|s| s.site.x).collect();
    assert_eq!(turf.len(), 2, "{turf:?}");
    let d = flora.deliveries().iter().find(|d| d.via == Via::Runner).expect("a runner receipt");
    assert_eq!(d.origin, face(0));
    assert_eq!(d.organic, package_of(&c.springturf), "the parent's whole package");
}

/// **Forbidden**: a cover the turf's list leaves out (stonecushion), the switch off, or
/// the turf's own gates shut on the cover's face — the cover is never displaced, and the
/// package leaves as a seed instead.
#[test]
fn a_runner_never_displaces_a_cover_it_may_not_take() {
    let mut off = runner_config();
    off.springturf.runners_overtop = false;
    let mut shut = runner_config();
    shut.springturf.establish_light_min = 2.0; // no face ever passes the turf's gates
    for (name, c, cover) in [("stone", runner_config(), STONE), ("off", off, PAD), ("gates", shut, PAD)] {
        let (mut flora, mut world) = runner_world(c.clone(), cover);
        for _ in 0..ticks_for_packages(&c, 3.0) {
            flora.step(&mut world);
        }
        assert_conserved(&flora, name);
        let l = flora.view().ledger;
        assert_eq!(l.runner_displaced, [0; Species::COUNT], "{name}");
        assert_eq!(l.overtopped[cover.index()], 0, "{name}");
        assert_eq!(l.clonal_births[TURF.index()], 0, "{name}");
        assert!(l.propagule_landed[TURF.index()] > 0.0, "{name}: packages still left, as seed");
        let covers = flora.view().stands.iter().filter(|s| s.species == cover).count();
        assert_eq!(covers, 2, "{name}: both covers hold");
    }
}

/// Moss never takes grass: velvetpad runners do not root on a springturf face by default.
#[test]
fn a_velvetpad_runner_does_not_displace_springturf_unless_its_own_list_allows() {
    let c = FloraConfig::default();
    assert_eq!(
        crate::seeds::runner_may_take(&c, PAD, TURF),
        c.velvetpad.runners_overtop
            && (c.velvetpad.overtop_covers.is_empty() || c.velvetpad.overtop_covers.contains(&TURF)),
        "the rule reads velvetpad's own switch and list"
    );
    let mut c = FloraConfig::default();
    c.velvetpad.runners_overtop = false;
    assert!(!crate::seeds::runner_may_take(&c, PAD, TURF));
}

// -------------------------------------------------------------------------- burrs

fn carrier(id: u64, x: u32, grounded: bool, coat: f64) -> Carrier {
    Carrier {
        id,
        site: face(x),
        x_m: f64::from(x) + 0.5,
        z_m: 0.5,
        grounded,
        coat,
    }
}

fn burr_config() -> FloraConfig {
    let mut c = quiet();
    c.springturf.burr_pickup = 1.0;
    c.springturf.burr_retention_mean_m = 6.0;
    c.springturf.burr_retention_shape = 1.0;
    c.burr_cap = 3;
    c
}

/// A ring of `width` faces with a fat turf donor on every even face.
fn burr_world(config: FloraConfig, width: u32) -> (Flora, World) {
    let world = slab(width);
    let mut flora = Flora::new(config);
    for x in (0..width).step_by(2) {
        plant(&mut flora, &world, x, TURF);
    }
    (flora, world)
}

fn burr_totals(flora: &Flora) -> (u64, u64, u64, f64, f64, f64) {
    let l = flora.view().ledger;
    let k = TURF.index();
    (
        l.burrs_picked[k],
        l.burrs_dropped[k],
        l.burrs_dropped_at_carcass[k],
        l.burr_picked_organic[k],
        l.burr_dropped_organic[k],
        l.burr_carcass_organic[k],
    )
}

/// **Burrs conserve**: two carriers walk the ring picking up seed; one dies (leaves the
/// list) carrying some. Picked = carried + dropped + dropped at the carcass, in counts and
/// in organic matter, and every ledger closes at every pass; the dead carrier's seeds land
/// in the bank of its last face.
#[test]
fn burrs_conserve_picked_carried_dropped_and_dropped_at_the_carcass() {
    let c = burr_config();
    let width = 16;
    let (mut flora, world) = burr_world(c.clone(), width);
    let mut last_a = 0;
    for step in 0..40u32 {
        let a = step % width;
        let b = (step + 5) % width;
        let mut carriers = vec![carrier(1, a, true, 1.0)];
        if step < 30 {
            carriers.push(carrier(2, b, true, 1.0));
        }
        flora.carry_burrs(&world, &carriers);
        assert_conserved(&flora, "burrs");
        let (picked, dropped, carcass, po, dop, co) = burr_totals(&flora);
        let carried = flora.burrs().len() as u64;
        let carried_o: f64 = flora.burrs().iter().map(|b| b.organic).sum();
        assert_eq!(picked, carried + dropped + carcass, "step {step}");
        assert!((po - carried_o - dop - co).abs() <= 1e-12 * po.max(1.0), "step {step}");
        for b in flora.burrs() {
            let mine = flora.burrs().iter().filter(|x| x.carrier == b.carrier).count() as u32;
            assert!(mine <= c.burr_cap, "the cap holds");
        }
        if step == 29 {
            last_a = b;
        }
    }
    let (picked, dropped, carcass, ..) = burr_totals(&flora);
    assert!(picked > 0 && dropped > 0, "{picked} picked, {dropped} dropped");
    assert!(carcass > 0, "the second carrier died carrying seed");
    let at_carcass: Vec<_> = flora.deliveries().iter().filter(|d| d.via == Via::Burr).collect();
    assert!(!at_carcass.is_empty());
    assert!(flora.burrs().iter().all(|b| b.carrier == 1), "nothing rides a dead carrier");
    let bank = flora.view().ground_at(face(last_a)).map_or(0.0, |g| g.seed_organic(TURF));
    assert!(bank > 0.0, "the carcass's face holds turf seed");
}

/// A carrier picks up only when it steps onto a new face, on the ground, with a coat: a
/// flyer in the air and a coatless body pick up nothing; the same flyer grounded does.
#[test]
fn a_grounded_flyer_picks_up_seed_and_one_in_flight_does_not() {
    let c = burr_config();
    for (grounded, coat, expect) in [(false, 1.0, false), (true, 0.0, false), (true, 1.0, true)] {
        let (mut flora, world) = burr_world(c.clone(), 8);
        flora.carry_burrs(&world, &[carrier(5, 1, grounded, coat)]);
        flora.carry_burrs(&world, &[carrier(5, 2, grounded, coat)]);
        let picked = flora.view().ledger.burrs_picked[TURF.index()];
        assert_eq!(picked > 0, expect, "grounded {grounded}, coat {coat}");
    }
    // Standing still on a donor's face is not a contact.
    let (mut flora, world) = burr_world(c, 8);
    for _ in 0..5 {
        flora.carry_burrs(&world, &[carrier(5, 2, true, 1.0)]);
    }
    assert_eq!(flora.view().ledger.burrs_picked[TURF.index()], 0);
}

/// A dropped burr is an ordinary seed in the bank: it joins the bin, the wheel and the
/// receipts, and the donor paid for it out of its propagule budget.
#[test]
fn a_burr_is_paid_by_its_donor_and_lands_in_the_bank_where_it_falls() {
    let mut c = burr_config();
    c.springturf.burr_retention_mean_m = 1e-9; // falls at the first step
    let (mut flora, world) = burr_world(c.clone(), 8);
    let funded0 = flora.view().ledger.propagule_funded[TURF.index()];
    flora.carry_burrs(&world, &[carrier(9, 1, true, 1.0)]);
    flora.carry_burrs(&world, &[carrier(9, 2, true, 1.0)]);
    assert_eq!(flora.burrs().len(), 1);
    assert!(flora.view().ledger.propagule_funded[TURF.index()] > funded0, "paid from the donor's budget");
    flora.carry_burrs(&world, &[carrier(9, 3, true, 1.0)]);
    let l = flora.view().ledger;
    assert_eq!(l.burrs_dropped[TURF.index()], 1);
    let bank = flora.view().ground_at(face(3)).map_or(0.0, |g| g.seed_organic(TURF));
    assert_eq!(bank, package_of(&c.springturf));
    let d = flora.deliveries().iter().find(|d| d.via == Via::Burr).expect("receipt");
    assert_eq!((d.origin, d.recipient), (face(2), face(3)));
    assert_conserved(&flora, "dropped");
}

// ----------------------------------------------------------------- the distributions

fn moments(draws: &[f64]) -> (f64, f64) {
    let n = draws.len() as f64;
    let mean = draws.iter().sum::<f64>() / n;
    let var = draws.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / (n - 1.0);
    (mean, var)
}

#[test]
fn ln_gamma_is_the_gamma_function() {
    for (x, g) in [(1.0, 1.0), (2.0, 1.0), (5.0, 24.0), (0.5, std::f64::consts::PI.sqrt()), (3.5, 3.323_350_970_447_843)] {
        assert!((ln_gamma(x).exp() - g).abs() <= 1e-10 * g, "Γ({x})");
    }
}

/// **Retention**: many draws of the configured Weibull match its mean
/// `scale · Γ(1 + 1/k)` and variance `scale² · (Γ(1 + 2/k) − Γ(1 + 1/k)²)`, at the
/// shipped springturf's mean and shape and at two others.
#[test]
fn sampled_retention_matches_its_weibull_mean_and_variance() {
    let sc = FloraConfig::default().springturf;
    for (mean, k) in [(sc.burr_retention_mean_m, sc.burr_retention_shape), (2.0, 1.0), (5.0, 1.8)] {
        let draws: Vec<f64> = (0..200_000u64)
            .map(|i| retention_distance(&mut Rng::keyed(99, 3, i, 0), mean, k))
            .collect();
        let (m, v) = moments(&draws);
        let scale = weibull_scale(mean, k);
        let g1 = ln_gamma(1.0 + 1.0 / k).exp();
        let g2 = ln_gamma(1.0 + 2.0 / k).exp();
        let var = scale * scale * (g2 - g1 * g1);
        assert!((m - mean).abs() < 0.02 * mean, "k {k}: mean {m} against {mean}");
        assert!((v - var).abs() < 0.06 * var, "k {k}: variance {v} against {var}");
    }
}

/// **Wind**: many WALD draws match the configured mean `μ` and variance `μ³/λ`, and the
/// whole-column offsets the landing uses keep the mean in columns.
#[test]
fn sampled_wind_distances_match_the_wald_kernel() {
    let sc = FloraConfig::default().springturf;
    for (mu, lambda) in [(sc.wald_mean_m, sc.wald_shape_m), (0.75, 0.5), (3.0, 2.0)] {
        let draws: Vec<f64> = (0..200_000u64)
            .map(|i| wald_distance(&mut Rng::keyed(DOMAIN_WIND, 5, i, 1), mu, lambda))
            .collect();
        let (m, v) = moments(&draws);
        assert!(draws.iter().all(|d| *d >= 0.0));
        assert!((m - mu).abs() < 0.02 * mu, "μ {mu}: mean {m}");
        let var = mu.powi(3) / lambda;
        assert!((v - var).abs() < 0.1 * var, "μ {mu}: variance {v} against {var}");
        // The offsets, in columns of 0.125 m, far from the cap.
        let mut s = sc.clone();
        s.wald_mean_m = mu;
        s.wald_shape_m = lambda;
        let voxel = 0.125;
        let cols: Vec<f64> = (0..100_000u64)
            .map(|i| {
                let (dx, dz) = wald_offset(&mut Rng::keyed(DOMAIN_WIND, 6, i, 1), s.wald_mean_m, s.wald_shape_m, voxel, 1e6);
                ((dx * dx + dz * dz) as f64).sqrt()
            })
            .collect();
        let (mc, _) = moments(&cols);
        assert!((mc * voxel - mu).abs() < 0.05 * mu + voxel, "μ {mu}: offset mean {} m", mc * voxel);
    }
}

// ------------------------------------------------------------------------- curing

/// A cure-ready turf on one face of a three-column slab.
fn cure_config() -> FloraConfig {
    let mut c = quiet();
    let t = &mut c.springturf;
    t.cure_water = 0.5;
    t.cure_resume_water = 0.6;
    t.cure_after_s = 1.0;
    t.cure_shed = 0.8;
    t.cure_upkeep = 0.1;
    t.cure_max_s = 0.0;
    c
}

fn dry(world: &mut World) {
    let cap = world.config().materials.pore_capacity(Material::Soil) * world.config().voxel_volume();
    for x in 0..i64::from(world.config().width) {
        for y in 1..=2u32 {
            world.apply(WorldCommand::WithdrawPore { x, y, z: 0, volume_m3: cap });
        }
    }
}

fn wet(world: &mut World) {
    add_pore(world, cubarium_voxel::Materials::DEFAULT.field_capacity(Material::Soil));
}

fn turf(flora: &Flora) -> Option<crate::Stand> {
    flora.view().stand_at(face(1)).copied()
}

fn cure_world(c: FloraConfig) -> (Flora, World) {
    let mut world = slab(3);
    let mut flora = Flora::new(c);
    plant(&mut flora, &world, 1, TURF);
    flora.step(&mut world);
    (flora, world)
}

/// **A dry stand cures**: after `cure_after_s` below `cure_water` it drops `cure_shed` of
/// its foliage to the litter — booked as shed foliage and as the cure's own line — and
/// while dormant it fixes nothing at all.
#[test]
fn a_dry_stand_drops_its_foliage_to_litter_and_never_photosynthesises_while_dormant() {
    let c = cure_config();
    let (mut flora, mut world) = cure_world(c.clone());
    dry(&mut world);
    let mut before = None;
    for _ in 0..200 {
        let s = turf(&flora).expect("alive");
        let litter = flora.view().ground_at(face(1)).map_or(0.0, |g| g.litter);
        flora.step(&mut world);
        assert_conserved(&flora, "curing");
        if turf(&flora).is_some_and(|t| t.cured) {
            before = Some((s, litter));
            break;
        }
    }
    let (s, litter0) = before.expect("cured");
    let l = flora.view().ledger;
    assert_eq!(l.cures[TURF.index()], 1);
    let shed = l.cure_shed_organic[TURF.index()];
    assert!((shed - c.springturf.cure_shed * s.foliage).abs() <= 1e-9 * s.foliage, "{shed} of {}", s.foliage);
    let litter = flora.view().ground_at(face(1)).map_or(0.0, |g| g.litter);
    assert!(litter >= litter0 + 0.99 * shed, "the shed foliage is in the litter");
    // Dormant: nothing fixed, the foliage stands.
    let fixed = flora.view().ledger.fixed_in;
    let foliage = turf(&flora).unwrap().foliage;
    for _ in 0..100 {
        flora.step(&mut world);
        assert_conserved(&flora, "dormant");
    }
    assert_eq!(flora.view().ledger.fixed_in, fixed, "no photosynthesis while cured");
    let t = turf(&flora).expect("still alive");
    assert!(t.cured);
    assert_eq!(t.foliage, foliage, "no growth, no senescence");
}

/// **It regrows from its reserve** once the water is back above `cure_resume_water`.
#[test]
fn a_cured_stand_regrows_from_its_reserve_after_rewetting() {
    let c = cure_config();
    let (mut flora, mut world) = cure_world(c);
    dry(&mut world);
    while !turf(&flora).is_some_and(|t| t.cured) {
        flora.step(&mut world);
    }
    let cured = turf(&flora).unwrap();
    wet(&mut world);
    flora.step(&mut world);
    let woke = turf(&flora).expect("alive");
    assert!(!woke.cured, "woke");
    assert_eq!(flora.view().ledger.cure_wakes[TURF.index()], 1);
    for _ in 0..200 {
        flora.step(&mut world);
    }
    let t = turf(&flora).expect("alive");
    assert!(t.foliage > cured.foliage, "regrew: {} > {}", t.foliage, cured.foliage);
    assert_conserved(&flora, "regrown");
}

/// **A long enough drought kills it**: dormant past `cure_max_s`, it dies of drought.
#[test]
fn a_cured_stand_dies_past_the_long_drought_limit() {
    let mut c = cure_config();
    c.springturf.cure_max_s = 2.0;
    let (mut flora, mut world) = cure_world(c.clone());
    dry(&mut world);
    let limit = ((c.springturf.cure_after_s + c.springturf.cure_max_s) / crate::DT).ceil() as u64 + 5;
    for _ in 0..limit {
        flora.step(&mut world);
        assert_conserved(&flora, "drought");
    }
    assert!(turf(&flora).is_none(), "dead");
    let l = flora.view().ledger;
    assert_eq!(l.deaths_by_cause[DeathCause::Drought.index()][TURF.index()], 1);
}

// ---------------------------------------------------------- velvetpad on wet ground

/// **Saturated but unflooded** ground passes a species whose saturation ceiling is 1;
/// the same face under more standing water than it drowns in does not.
#[test]
fn velvetpad_with_no_saturation_ceiling_establishes_on_a_saturated_face_and_not_a_flooded_one() {
    let mut c = FloraConfig::default();
    c.velvetpad.establish_saturated_max = 1.0;
    let sc = c.velvetpad.clone();
    let mut world = slab(3);
    add_pore(&mut world, 1.0 - cubarium_voxel::Materials::DEFAULT.field_capacity(Material::Soil));
    let view = world.view();
    let g = establishment_gates_with_sky(&view, face(1), &sc, 1.0, 0.0, 0.0);
    assert!(g.saturated_fraction > 0.99, "saturated: {}", g.saturated_fraction);
    assert!(g.water_depth_m <= sc.drown_depth_m, "not flooded");
    assert!(g.passes(), "{g:?}");
    // A lower ceiling refuses the same face: the gate reads the config.
    let mut low = sc.clone();
    low.establish_saturated_max = 0.5;
    assert!(!establishment_gates_with_sky(&view, face(1), &low, 1.0, 0.0, 0.0).passes());
    // Flooded: standing water deeper than it drowns in.
    let depth = sc.drown_depth_m + 0.2;
    let mut world = world;
    let area = world.config().voxel_m * world.config().voxel_m;
    world.apply(WorldCommand::AddWater { x: 1, y: 3, z: 0, volume_m3: depth * area });
    let view = world.view();
    let g = establishment_gates_with_sky(&view, face(1), &sc, 1.0, 0.0, 0.0);
    assert!(g.water_depth_m > sc.drown_depth_m, "flooded: {}", g.water_depth_m);
    assert!(!g.passes());
}
