//! **The sun on a saprotroph** (GLOWCAP-2): exposure, the share of open sun reaching a
//! fungus's cap with its own face's plant over it, dries its caps and its surface mycelium
//! to its face's litter above `sun_shade_max`, in proportion to the ramp to full sun and to
//! the daylight. Shade and the dark cost nothing.
//!
//! Hand-built: a soil slab at field capacity, open to the sky or under a rock roof. Every
//! other glowcap rate that would move its stocks is zero unless a fixture turns it on, so
//! the sun is the only thing that changes a stand. Every expectation is computed from the
//! fixture's own config and the exposure the layer reports; no preset is pinned.

use super::*;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};

const FUNGUS: Species = Species::Glowcap;

fn site() -> Site {
    Site { x: 2, y: 2, z: 0 }
}

/// Five soil columns at field capacity, faces at `y = 2`; `roof` lays rock over every
/// column at `y = 4`, so the faces are in the dark.
fn world(roof: bool) -> World {
    let mut w = World::empty(VoxelConfig {
        width: 5,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 41,
        ..VoxelConfig::default()
    });
    let water = w.config().materials.field_capacity(Material::Soil)
        * w.config().materials.pore_capacity(Material::Soil)
        * w.config().voxel_volume();
    for x in 0..5 {
        for y in 1..=2 {
            w.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: water });
            w.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
        }
        if roof {
            w.apply(WorldCommand::SetMaterial { x, y: 4, z: 0, material: Material::Rock });
        }
    }
    w
}

/// The glowcap holds still but for the sun: no upkeep, no reserve room, no growth, no
/// shedding, no decomposition. `cap`, `myc` and `shade` are the three sun knobs.
fn config(cap: f64, myc: f64, shade: f64) -> FloraConfig {
    let mut c = FloraConfig::for_voxel_size(1.0);
    c.decomposition = 0.0;
    c.wood_decomposition = 0.0;
    c.carrion_decomposition = 0.0;
    c.upkeep_q10 = 1.0;
    c.initial_mineral = 1.0;
    c.step_every = 1;
    for sp in Species::ALL {
        let s = c.species_mut(sp);
        s.propagule_rate = 0.0;
        s.seed_attrition_per_s = 0.0;
        s.burr_pickup = 0.0;
        s.cure_after_s = 0.0;
    }
    let s = c.species_mut(FUNGUS);
    s.substrate_budgeted = true;
    s.substrate_required = true;
    s.rooting_radius = 0;
    s.rooting_depth = 1;
    s.wood_max = 1.0;
    s.alpha = 0.5;
    s.reserve_cap = 0.0;
    s.maintenance = 0.0;
    s.dieback = 1.0;
    s.alive_min = 0.3;
    s.foliage_rate = 0.0;
    s.wood_rate = 0.0;
    s.senescence = 0.0;
    s.dark_uptake_per_s = 0.0;
    s.substrate_uptake_per_s = 10.0;
    s.substrate_yield = 0.5;
    s.energy_density = 2.0;
    s.defence_per_bite = 0.0;
    s.establish_substrate_min = 0.1;
    s.establish_water_min = 0.0;
    s.establish_saturated_max = 1.0;
    s.wilt_water = 0.0;
    s.full_water = 0.5;
    s.sun_cap_loss_per_s = cap;
    s.sun_mycelium_loss_per_s = myc;
    s.sun_shade_max = shade;
    c.litter_energy_cap = c.glowcap.energy_density;
    c
}

fn seed(c: FloraConfig, w: &World) -> Flora {
    let wood = c.glowcap.wood_max;
    let mut f = Flora::new(c);
    assert!(f.apply(w, Command::SeedOnFace { site: site(), species: FUNGUS, wood }));
    let s = f.config.glowcap.clone();
    assert!(f.deposit(
        site(),
        Deposit {
            kind: DepositKind::Litter,
            organic: 1.0,
            mineral: s.n_tissue,
            energy: s.energy_density,
        }
    ));
    assert!(f.view().fungus_at(site()).unwrap().foliage > 0.0, "fixture: the founder has caps");
    f
}

fn close(f: &Flora) {
    let v = f.view();
    for (held, expected, what) in [
        (v.organic(), v.ledger.expected_organic(), "organic"),
        (v.mineral(), v.ledger.expected_mineral(), "mineral"),
        (v.energy(), v.ledger.expected_energy(), "energy"),
    ] {
        assert!(
            (held - expected).abs() < 1e-10 * held.abs().max(1.0),
            "{what}: held {held}, expected {expected}"
        );
    }
}

/// How far `exposure` is from shade to full sun: 0 at and below `shade`, 1 at full sun.
fn sun(shade: f64, exposure: f64) -> f64 {
    if exposure <= shade { 0.0 } else { ((exposure - shade) / (1.0 - shade)).min(1.0) }
}

/// The share of a stock one tick of sun takes at `exposure`, from the config's own knobs.
fn share(rate: f64, shade: f64, exposure: f64, daylight: f64) -> f64 {
    (rate * sun(shade, exposure) * daylight * DT).min(1.0)
}

#[test]
fn a_sunlit_face_with_litter_is_a_poorer_glowcap_site_than_a_dark_one() {
    let (cap, myc, shade) = (0.02, 0.01, 0.5);
    let mut ends = Vec::new();
    for roof in [false, true] {
        let mut w = world(roof);
        let mut f = seed(config(cap, myc, shade), &w);
        let before = *f.view().fungus_at(site()).unwrap();
        let litter0 = f.view().ground_at(site()).unwrap().litter;
        f.step(&mut w);
        close(&f);
        let after = *f.view().fungus_at(site()).unwrap();
        let e = after.light;
        if roof {
            assert!(e <= shade, "fixture: the roofed face is shade ({e})");
        } else {
            assert!(e > shade, "fixture: the open face is in the sun ({e})");
        }
        let d = w.life_daylight();
        assert!(d > 0.0, "fixture: daylight");
        let want_p = before.foliage * (1.0 - share(cap, shade, e, d));
        let want_w = before.wood * (1.0 - share(myc, shade, e, d));
        assert!((after.foliage - want_p).abs() < 1e-12, "roof={roof}: caps {} want {want_p}", after.foliage);
        assert!((after.wood - want_w).abs() < 1e-12, "roof={roof}: mycelium {} want {want_w}", after.wood);
        let dried = (before.foliage - after.foliage) + (before.wood - after.wood);
        assert!((f.view().ledger.sun_dried[FUNGUS.index()] - dried).abs() < 1e-12);
        let litter1 = f.view().ground_at(site()).unwrap().litter;
        assert!((litter1 - litter0 - dried).abs() < 1e-12, "the dried tissue is the face's litter");
        // Run on until the sunlit one is gone.
        let mut steps = 0;
        while f.view().fungus_at(site()).is_some() && steps < 100_000 {
            f.step(&mut w);
            steps += 1;
        }
        close(&f);
        ends.push((f.view().fungus_at(site()).copied(), f.view().ledger.deaths_by_cause));
    }
    let (open, open_deaths) = &ends[0];
    let (dark, dark_deaths) = &ends[1];
    assert!(open.is_none(), "the sun dried the open stand away");
    assert_eq!(open_deaths[DeathCause::Desiccated.index()][FUNGUS.index()], 1);
    assert_eq!(open_deaths[DeathCause::Starved.index()][FUNGUS.index()], 0);
    assert!(dark.is_some(), "the shaded stand stands");
    assert_eq!(dark_deaths[DeathCause::Desiccated.index()][FUNGUS.index()], 0);
}

#[test]
fn the_loss_follows_the_ramp_between_shade_and_full_sun() {
    // A cap in part shade — under its face's plant — with the shade bound at half its
    // exposure: a fraction of the full rate, by the ramp.
    let shaded = |c: FloraConfig| {
        let w = world(false);
        let mut f = seed(c, &w);
        let wood = f.config.bloomcrown.wood_max;
        assert!(f.apply(&w, Command::SeedOnFace { site: site(), species: Species::Bloomcrown, wood }));
        (w, f)
    };
    let probe = {
        let (mut w, mut f) = shaded(config(0.0, 0.0, 0.0));
        f.step(&mut w);
        f.view().fungus_at(site()).unwrap().light
    };
    assert!(probe > 0.0 && probe < 1.0, "fixture: part shade ({probe})");
    let shade = 0.5 * probe;
    let (cap, myc) = (0.02, 0.01);
    let (mut w, mut f) = shaded(config(cap, myc, shade));
    let before = *f.view().fungus_at(site()).unwrap();
    f.step(&mut w);
    let after = *f.view().fungus_at(site()).unwrap();
    assert_eq!(after.light, probe, "the knobs do not move the exposure");
    let ramp = sun(shade, probe);
    assert!(ramp > 0.0 && ramp < 1.0, "fixture: partial sun ({ramp})");
    let d = w.life_daylight();
    assert!((after.foliage - before.foliage * (1.0 - cap * ramp * d * DT)).abs() < 1e-12);
    assert!((after.wood - before.wood * (1.0 - myc * ramp * d * DT)).abs() < 1e-12);
    close(&f);
}

#[test]
fn the_switch_off_restores_the_old_behaviour() {
    // Rates zero: the sun moves nothing, however exposed; the stand runs the old rules.
    // With growth on, the knobs off and the knobs on behind full shade agree to the bit.
    let growing = |cap: f64, myc: f64, shade: f64| {
        let mut c = config(cap, myc, shade);
        let s = &mut c.glowcap;
        s.wood_max = 2.0;
        s.wood_rate = 0.01;
        s.foliage_rate = 0.01;
        s.senescence = 0.005;
        s.maintenance = 0.01;
        s.reserve_cap = 0.2;
        c
    };
    let run = |c: FloraConfig| {
        let mut w = world(false);
        let mut f = seed(c, &w);
        for _ in 0..200 {
            f.step(&mut w);
            close(&f);
        }
        let v = f.view();
        (
            *v.fungus_at(site()).expect("alive"),
            v.ground_at(site()).unwrap().clone(),
            v.ledger.sun_dried[FUNGUS.index()],
            v.ledger.deaths_by_cause,
        )
    };
    let off = run(growing(0.0, 0.0, 0.0));
    let shaded = run(growing(0.05, 0.05, 1.0));
    assert!(off.0.light > 0.0, "fixture: in the sun");
    assert_eq!(off.2, 0.0, "nothing dried with the knobs off");
    assert_eq!(off.0, shaded.0, "the stand ran the same rules");
    assert_eq!(off.1, shaded.1, "the ground ran the same rules");
    assert_eq!(off.3, shaded.3);
    // And still in this fixture: nothing but the sun moves the stand, so off is still.
    let mut w = world(false);
    let mut f = seed(config(0.0, 0.0, 0.0), &w);
    let before = *f.view().fungus_at(site()).unwrap();
    for _ in 0..50 {
        f.step(&mut w);
    }
    let after = *f.view().fungus_at(site()).unwrap();
    assert_eq!((after.wood, after.foliage), (before.wood, before.foliage));
}

#[test]
fn a_cap_under_its_faces_plant_is_in_that_plants_shade() {
    let exposure = |with_plant: bool| {
        let mut w = world(false);
        let mut f = seed(config(0.0, 0.0, 0.0), &w);
        if with_plant {
            let wood = f.config.bloomcrown.wood_max;
            assert!(f.apply(&w, Command::SeedOnFace { site: site(), species: Species::Bloomcrown, wood }));
        }
        f.step(&mut w);
        f.view().fungus_at(site()).unwrap().light
    };
    let (bare, under) = (exposure(false), exposure(true));
    assert!(under < bare, "the face's plant shades its cap: {under} vs {bare}");
}

#[test]
fn a_plant_on_a_shared_sunlit_face_is_unharmed() {
    let run = |cap: f64, myc: f64| {
        let mut c = config(cap, myc, 0.0);
        // The plant grows by the defaults; only the fungus is held still.
        c.springturf.cure_after_s = 0.0;
        let mut w = world(false);
        let mut f = seed(c, &w);
        let wood = f.config.springturf.wood_max;
        assert!(f.apply(&w, Command::SeedOnFace { site: site(), species: Species::Springturf, wood }));
        for _ in 0..400 {
            f.step(&mut w);
            close(&f);
        }
        let v = f.view();
        let plant = *v.stands_on(site()).iter().find(|s| s.species == Species::Springturf).expect("the plant");
        (plant, v.fungus_at(site()).copied(), v.ledger.sun_dried[FUNGUS.index()])
    };
    let (plant_off, fungus_off, _) = run(0.0, 0.0);
    let (plant_on, fungus_on, dried) = run(0.05, 0.05);
    assert!(dried > 0.0, "fixture: the sun dried the fungus");
    assert!(
        fungus_on.is_none_or(|on| on.wood < fungus_off.unwrap().wood),
        "fixture: the sun cost the fungus"
    );
    assert_eq!(plant_on, plant_off, "the plant on the face is untouched by the fungus's sun");
}

#[test]
fn the_sun_loss_conserves_every_currency_into_the_litter() {
    let mut w = world(false);
    let mut f = seed(config(0.05, 0.03, 0.0), &w);
    let s0 = *f.view().fungus_at(site()).unwrap();
    let g0 = f.view().ground_at(site()).unwrap().clone();
    let mut steps = 0;
    while f.view().fungus_at(site()).is_some() && steps < 100_000 {
        f.step(&mut w);
        close(&f);
        steps += 1;
    }
    assert!(f.view().fungus_at(site()).is_none(), "fixture: the sun finished it");
    let g = f.view().ground_at(site()).unwrap().clone();
    // No upkeep, no uptake, no decomposition: the stand's whole matter is now litter.
    assert!((g.litter - (g0.litter + s0.material())).abs() < 1e-10, "{} vs {}", g.litter, g0.litter + s0.material());
    assert!((g.litter_mineral + g.mineral - (g0.litter_mineral + g0.mineral + s0.mineral)).abs() < 1e-10);
    assert_eq!(g.dead_wood, 0.0, "mycelium does not become a log");
    close(&f);
}

/// One tick of sun for `c` on a face whose neighbours on one side are roofed, so its sky is
/// part open; returns the mycelium lost.
fn part_roofed_loss(c: FloraConfig) -> f64 {
    let mut w = world(false);
    for x in 0..2 {
        w.apply(WorldCommand::SetMaterial { x, y: 4, z: 0, material: Material::Rock });
    }
    let mut f = seed(c, &w);
    let before = f.view().fungus_at(site()).unwrap().wood;
    f.step(&mut w);
    close(&f);
    before - f.view().fungus_at(site()).unwrap().wood
}

/// **SUN-SHELTER**: its own dark habitat shelters a glowcap from the drying in proportion to
/// its darkness, `drying × (1 − sun_dark_shelter · darkness)`. With no dark habitat
/// (`dark_sky_max` 0, darkness 0) the shelter changes nothing. With the face inside it
/// (`dark_sky_max` 1, `dark_sky_full` 0: darkness = 1 − sky), the loss falls linearly in the
/// knob, and 0 is the exposure-only rule.
#[test]
fn its_dark_habitat_shelters_a_glowcap_from_the_sun_in_proportion() {
    let with = |shelter: f64, dark_sky_max: f64| {
        let mut c = config(0.0, 0.01, 0.0);
        c.glowcap.dark_sky_full = 0.0;
        c.glowcap.dark_sky_max = dark_sky_max;
        c.glowcap.sun_dark_shelter = shelter;
        part_roofed_loss(c)
    };
    let open = with(0.0, 0.0);
    assert!(open > 0.0, "fixture: the part-open face dries");
    assert_eq!(with(1.0, 0.0), open, "no darkness, no shelter");
    let (none, half, full) = (with(0.0, 1.0), with(0.5, 1.0), with(1.0, 1.0));
    assert_eq!(none, open, "shelter 0 is the exposure-only rule");
    assert!(full > 0.0 && full < none, "partly dark: sheltered, not spared ({full} vs {none})");
    assert!((half - 0.5 * (none + full)).abs() < 1e-12 * none, "linear in the knob");
}
