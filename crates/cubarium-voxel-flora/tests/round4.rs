//! Round 4's three presets, each against the four checks Astra's R6.2 asked every new
//! preset to leave behind: a **paid birth** on its contract habitat, **survival and
//! income** for the stand that birth produced, a **failing neighbour** — the neighbouring
//! condition the role excludes, where the same package never germinates and reaches the
//! litter with its mineral — and **validity** of every preset's own numbers.
//!
//! Conventions, the same ones `round3.rs` uses so a fixture here reads the same way:
//! `voxel_m` is 1 m, so a soil voxel holds `Material::Soil.pore_capacity()` = 0.35 m³ of
//! pore water and a metre of free water over a face is a metre deep. Soil is wetted by
//! adding free water to an air cell and then turning the cell to soil. The world is never
//! stepped in these fixtures: the only thing that moves water is the plant layer's own
//! bounded withdrawal, so a fixture's pore fraction is the condition it says it is.
//!
//! Where a test needs a rate the placeholders do not give, it sets that rate in its
//! **own** config and says why. Three of them recur, and they are the same three
//! `round3.rs` uses:
//!
//! - `propagule_rate` 3.0 /s and `reserve_cap` 40 against the presets' own tiny rates, so
//!   that one donor funds one whole package in a single tick and a paid birth is a
//!   two-tick fixture rather than a 600-second one. What is being tested is the birth and
//!   the ledger, not how long a donor takes to save.
//! - `seed_max_age_s` 0.1 s — two ticks — against the placeholder 600, so that a bank
//!   which is refused reaches the litter inside a short run.
//! - `decomposition` 0, so that what reaches the litter stays there and can be read off.
//!
//! None of those is read back as a placeholder.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command, Flora, FloraConfig, FloraLedger, Site, Species, SpeciesConfig, establishment_gates,
    highest_support,
};

// ------------------------------------------------------------------- fixtures

/// Turn one air voxel into `material` holding exactly `pore` of that material's own pore
/// capacity, by adding the water first and converting after. `round3.rs`'s `fill`.
fn fill(w: &mut World, x: i64, y: u32, z: u32, material: Material, pore: f64) {
    let want = pore * material.pore_capacity() * w.config().voxel_volume();
    if want > 0.0 {
        let got = w.apply(WorldCommand::AddWater { x, y, z, volume_m3: want });
        assert!((got - want).abs() < 1e-12, "the void took {got} of {want}");
    }
    w.apply(WorldCommand::SetMaterial { x, y, z, material });
    assert!(
        (w.view().pore_at(x, y, z) - pore).abs() < 1e-12,
        "pore {} at ({x},{y},{z})",
        w.view().pore_at(x, y, z)
    );
    assert_eq!(w.view().free_at(x, y, z), 0.0, "nothing may be left standing");
}

fn empty_world(width: u32, depth: u32) -> World {
    World::empty(VoxelConfig {
        width,
        height: 10,
        depth,
        voxel_m: 1.0,
        seed: 5,
        ..VoxelConfig::default()
    })
}

/// Take a column's bedrock away, so it holds nothing solid at all and therefore **no
/// support face**: nothing can land on it and nothing can stand on it. `World::empty` lays
/// a bedrock foundation across the whole footprint, and a bare foundation cell is a
/// support face like any other (`round3.rs`'s `strip_gap` makes the same hole).
fn void_column(w: &mut World, x: i64, z: u32) {
    w.apply(WorldCommand::SetMaterial { x, y: 0, z, material: Material::Air });
    assert!(highest_support(&w.view(), x, z).is_none(), "({x},{z}) must be void");
}

/// A strip in which only the columns of `keep` are solid at all: bedrock at `y = 0`, soil
/// at `y = 1..=2` holding `pore` of soil's capacity, and every other column void. So every
/// kept column's support face is `y = 2` in open sky, and a donor's dispersal has exactly
/// the candidates the fixture chose to give it — which is what pins a landing site under a
/// one-package-at-a-time draw.
fn pillars(width: u32, keep: &[i64], pore: f64) -> World {
    let mut w = empty_world(width, 1);
    for x in 0..width as i64 {
        if keep.contains(&x) {
            for y in 1..=2 {
                fill(&mut w, x, y, 0, Material::Soil, pore);
            }
        } else {
            void_column(&mut w, x, 0);
        }
    }
    for &x in keep {
        assert_eq!(highest_support(&w.view(), x, 0), Some(at(x as u32)), "kept column {x}");
    }
    w
}

/// A three-slab strip whose only two support faces are `(1, 1)` and `(2, 1)`, both soil at
/// `pore` with their faces at `y = 2` in open sky. Three slabs deep because **shutting a
/// site's sky needs walls on every side of it**: the sky fan is a zenith ray, eight
/// azimuths at 60° of elevation and the same eight at 30°, and on a one-slab strip every
/// ray with a `z` component leaves through the world's own `z` face whatever the terrain
/// does. See [`shut_the_sky`].
fn well(pore: f64) -> World {
    let mut w = empty_world(5, 3);
    for z in 0..3 {
        for x in 0..5i64 {
            if z == 1 && (x == 1 || x == 2) {
                for y in 1..=2 {
                    fill(&mut w, x, y, z, Material::Soil, pore);
                }
            } else {
                void_column(&mut w, x, z);
            }
        }
    }
    w
}

/// Raise every one of a site's eight neighbouring columns into a wall from just above its
/// own face to the ceiling. The site keeps its support — nothing is put in its own column
/// — and its sky visibility falls to the **zenith alone**, 1 of the fan's 11.93 of weight.
///
/// This is the model's own version of "under a canopy" at germination, and it is terrain
/// and not a plant: `Gates::sky_visibility` is geometric sky with **no canopy in it**, so a
/// living crown cannot shut the germination light gate at all. Astra's R6.2 named
/// canopy-sensitive germination as an explicit rule addition, and round 4 does not make
/// it, so the shaded-ground case is the one the model can be asked about. The adult side
/// of the same claim — a springturf under a bloomcrown crown earns less light than one in
/// the open — is `springturf_gives_up_first_on_dry_ground_and_loses_light_under_a_crown`.
fn shut_the_sky(w: &mut World, site: Site) {
    let before = w.view().sky_visibility(site.x as i64, site.y, site.z);
    let height = w.config().height;
    for dz in -1i64..=1 {
        let z = site.z as i64 + dz;
        if z < 0 || z >= w.config().depth as i64 {
            continue;
        }
        for dx in -1i64..=1 {
            if dx == 0 && dz == 0 {
                continue;
            }
            for y in site.y + 1..height {
                w.apply(WorldCommand::SetMaterial {
                    x: site.x as i64 + dx,
                    y,
                    z: z as u32,
                    material: Material::Rock,
                });
            }
        }
    }
    let after = w.view().sky_visibility(site.x as i64, site.y, site.z);
    assert!(before > 0.99, "the fixture's premise: open sky before the wall, not {before}");
    assert!(after < 0.1, "the wall left {after} of the sky open, not the zenith alone");
}

fn at(x: u32) -> Site {
    Site { x, y: 2, z: 0 }
}

fn run(flora: &mut Flora, world: &mut World, ticks: u32) {
    for _ in 0..ticks {
        flora.step(world);
    }
}

/// The three residuals, as `round3.rs` computes them.
fn assert_residuals(flora: &Flora, when: &str) {
    let v = flora.view();
    let (o, n, e) = (
        v.organic() - v.ledger.expected_organic(),
        v.mineral() - v.ledger.expected_mineral(),
        v.energy() - v.ledger.expected_energy(),
    );
    assert!(o.abs() <= 1e-9 * v.organic().abs().max(1.0), "{when}: organic residual {o}");
    assert!(n.abs() <= 1e-9 * v.mineral().abs().max(1.0), "{when}: mineral residual {n}");
    assert!(e.abs() <= 1e-9 * v.energy().abs().max(1.0), "{when}: energy residual {e}");
}

/// A config in which one species funds a whole package in a single tick: see the module
/// doc for why, and `round3.rs` for the same two rates.
fn fast_donor(species: Species) -> FloraConfig {
    let mut config = FloraConfig::default();
    {
        let sc = species_mut(&mut config, species);
        sc.propagule_rate = 3.0;
        sc.reserve_cap = 40.0;
    }
    config
}

fn species_mut(config: &mut FloraConfig, species: Species) -> &mut SpeciesConfig {
    match species {
        Species::Bloomcrown => &mut config.bloomcrown,
        Species::Umbrellafrond => &mut config.umbrellafrond,
        Species::Springturf => &mut config.springturf,
        Species::Stonecushion => &mut config.stonecushion,
        Species::Velvetpad => &mut config.velvetpad,
    }
}

/// One donor's one paid package, and the stand it bought.
struct PaidBirth {
    flora: Flora,
    /// The site the package landed on and the newborn stands on.
    site: Site,
    /// `alive_min / w_frac`: the material a germination needs to build a stand at exactly
    /// `alive_min` of wood.
    package: f64,
    /// The whole ledger at the end of the tick the package landed, before the donor was
    /// cleared and before the birth.
    landed: FloraLedger,
}

/// Seed a full-grown donor of `species` on column `donor_x`, run **one** tick so that it
/// funds and sends exactly one package, take the ledger, remove the donor so nothing else
/// can land, and run one more tick so the bank germinates.
///
/// Two ticks and not more, because the phases say so: a package lands in step 9 of the
/// tick it is funded in, and step 8 of the **next** tick is the lottery that spends it.
/// Clearing the donor in between is what makes the birth one package's worth and not a
/// stream of them, so the ledger line and the newborn's stocks are the same arithmetic.
fn paid_birth(world: &mut World, config: FloraConfig, species: Species, donor_x: i64, site: Site) -> PaidBirth {
    let sc = config.species(species).clone();
    let package = sc.alive_min / sc.propagule_split[0];
    let mut flora = Flora::new(config);
    assert!(
        flora.apply(world, Command::Seed { x: donor_x, z: 0, species, wood: sc.wood_max }),
        "the donor could not be planted on column {donor_x}"
    );
    assert!(
        cubarium_voxel_flora::can_establish(&world.view(), site, &sc),
        "the fixture's premise: {} must pass its own gates at {site:?} — {:?}",
        species.name(),
        establishment_gates(&world.view(), site, &sc)
    );

    flora.step(world);
    let landed = flora.view().ledger.clone();
    assert_eq!(flora.view().ledger.establishments, 0, "nothing can be born on the landing tick");
    assert!(
        flora.view().ground_at(site).is_some_and(|g| g.seed_organic(species) > 0.0),
        "no package reached {site:?}"
    );
    assert!(flora.apply(world, Command::Clear { x: donor_x, z: 0 }), "the donor would not clear");

    flora.step(world);
    PaidBirth { flora, site, package, landed }
}

/// The four assertions every paid birth has to satisfy, whatever the species: the ledger's
/// three reproductive figures against each other, exactly one establishment, a newborn
/// holding exactly one package with its wood at exactly `alive_min`, and the three
/// residuals.
fn assert_paid_birth(birth: &PaidBirth, species: Species) {
    let sc = birth.flora.config().species(species).clone();
    let i = species.index();
    let l = &birth.landed;
    let (req, fund, land) =
        (l.propagule_requested[i], l.propagule_funded[i], l.propagule_landed[i]);

    // One tick of a donor over `donor_min` with a reserve well above its own floor: it
    // asked for `propagule_rate · dt / (1 + c_g)` net and was funded all of it.
    let ask_net = sc.propagule_rate * cubarium_voxel::DT / (1.0 + sc.build);
    assert!((req - ask_net).abs() <= 1e-15, "{}: requested {req}, not {ask_net}", species.name());
    assert!((fund - req).abs() <= 1e-15, "{}: funded {fund} of a {req} ask", species.name());
    assert!(
        (land - birth.package).abs() <= 1e-15,
        "{}: landed {land}, not the one {} package",
        species.name(),
        birth.package
    );
    // What was funded and has not left is standing in the donor's parcel — and the donor
    // was cleared, so by now it has been booked out rather than lost.
    assert!(fund > land, "{}: a whole package left with nothing saved", species.name());
    // Nobody else reproduced.
    for other in Species::ALL {
        if other != species {
            assert_eq!(l.propagule_landed[other.index()], 0.0, "{} landed something", other.name());
        }
    }

    let v = birth.flora.view();
    assert_eq!(v.ledger.establishments, 1, "{}: the bank did not germinate", species.name());
    let born = *v.stand_at(birth.site).expect("nothing stands on the landing site");
    assert_eq!(born.species, species);
    assert_eq!(born.wood, sc.alive_min, "{}: born with {} of wood", species.name(), born.wood);
    assert!(
        (born.organic() - birth.package).abs() <= 1e-16,
        "{}: born holding {} of a {} package",
        species.name(),
        born.organic(),
        birth.package
    );
    // Germination reads the bank before its own tick's attrition (K7), so the newborn is a
    // whole package and not a package minus a tick of decay.
    assert!(born.mineral > 0.0, "{}: born with no mineral", species.name());
    assert!(born.reserve > 0.0 && born.foliage > 0.0, "{}: {born:?}", species.name());
    assert_residuals(&birth.flora, "after a paid birth");
}

/// A stand's income over a window, read off the ledger and off the stand: what the system
/// fixed against what it respired, and whether the stand itself is bigger than it was.
struct Income {
    fixed: f64,
    respired: f64,
    grew: f64,
    deaths: u64,
}

fn income_over(flora: &mut Flora, world: &mut World, site: Site, ticks: u32) -> Income {
    let before = flora.view();
    let (fixed0, respired0, deaths0) =
        (before.ledger.fixed_in, before.ledger.respired_out, before.ledger.deaths);
    let material0 = before.stand_at(site).expect("nothing stands here to start with").material();
    run(flora, world, ticks);
    let after = flora.view();
    Income {
        fixed: after.ledger.fixed_in - fixed0,
        respired: after.ledger.respired_out - respired0,
        grew: after.stand_at(site).map_or(f64::NAN, |s| s.material()) - material0,
        deaths: after.ledger.deaths - deaths0,
    }
}

fn assert_survives_on_income(flora: &Flora, world: &World, site: Site, species: Species, i: &Income) {
    let sc = flora.config().species(species);
    let stand = flora.view().stand_at(site).unwrap_or_else(|| {
        panic!("{} died on its contract habitat: {:?}", species.name(), establishment_gates(&world.view(), site, sc))
    });
    assert_eq!(i.deaths, 0, "{}: {} deaths over the window", species.name(), i.deaths);
    assert!(i.fixed > 0.0, "{}: fixed nothing at all", species.name());
    assert!(
        i.fixed > i.respired,
        "{}: fixed {} against {} respired — the income does not cover the upkeep",
        species.name(),
        i.fixed,
        i.respired
    );
    assert!(i.grew > 0.0, "{}: the stand shrank by {}", species.name(), -i.grew);
    assert!(stand.wood >= sc.alive_min, "{}: wood {} under alive_min", species.name(), stand.wood);
    assert!(stand.moisture > 0.0, "{}: wilting on its own habitat", species.name());
    assert_residuals(flora, "after a newborn's income window");
}

// ========================================================================= springturf
//
// The pioneer turf of open, moist soil: shallow, sun-demanding, fast, short-lived, a wide
// hop and a crown one cell tall. It wins the first years on bare moist ground and loses
// under a canopy and on dry ground.

/// **Paid birth.** A springturf donor on open soil at 0.6 of pore capacity funds one whole
/// 0.015 package in a tick, it lands on the fixture's single candidate column, and the next
/// tick builds a stand there at exactly `alive_min` 0.006 of wood.
///
/// The fixture is `pillars(8, [0, 1])`: springturf's `hop` is 3, the widest of the five, so
/// six of the eight columns are void to leave the donor exactly one recipient. That is the
/// hop being *used* rather than replaced by a test's own value.
#[test]
fn a_paid_springturf_birth_on_open_moist_soil_is_one_package_at_alive_min() {
    let mut world = pillars(8, &[0, 1], 0.6);
    let birth = paid_birth(&mut world, fast_donor(Species::Springturf), Species::Springturf, 0, at(1));
    assert!((birth.package - 0.015).abs() < 1e-15, "a {} package", birth.package);
    assert_paid_birth(&birth, Species::Springturf);
}

/// **Survival and income.** The stand that birth bought, alone on its habitat with the
/// donor gone, fixes more than the whole system respires over 200 ticks and is bigger at
/// the end than it was at the start — at ten times the base maintenance.
#[test]
fn a_newborn_springturf_earns_its_upkeep_on_open_moist_soil() {
    let mut world = pillars(8, &[0, 1], 0.6);
    let mut birth =
        paid_birth(&mut world, fast_donor(Species::Springturf), Species::Springturf, 0, at(1));
    let income = income_over(&mut birth.flora, &mut world, birth.site, 200);
    assert_survives_on_income(&birth.flora, &world, birth.site, Species::Springturf, &income);
    // The species' own light response at full sky is exactly 1: `light_half` only bites in
    // shade, and the next test is where it does.
    let stand = birth.flora.view().stand_at(birth.site).expect("alive");
    assert!((stand.light - 1.0).abs() < 1e-12, "open sky read as {}", stand.light);
}

/// **Failing neighbour.** The same package, on the same column, with the sky shut: it never
/// germinates, and when its bin's two seconds are up it falls to the litter whole, with its
/// mineral and its energy.
///
/// `establish_light_min` 0.75 is the highest of the five and the wall leaves the zenith
/// alone — 0.084 of the fan — so the light gate is the one that shuts and the other three
/// stay open, which the assertions read off `Gates` one by one.
#[test]
fn a_springturf_package_under_a_shut_sky_never_germinates_and_goes_to_litter() {
    let mut config = fast_donor(Species::Springturf);
    // Two ticks of seed life (placeholder 600 s) so the refusal reaches the litter inside
    // the run, and no decomposition (placeholder 0.001 /s) so what reaches it stays.
    config.springturf.seed_max_age_s = 0.1;
    config.decomposition = 0.0;
    let package = config.springturf.alive_min / config.springturf.propagule_split[0];

    let mut world = well(0.6);
    let site = Site { x: 2, y: 2, z: 1 };
    let donor = Site { x: 1, y: 2, z: 1 };
    let mut flora = Flora::new(config.clone());
    assert!(flora.apply(
        &world,
        Command::Seed { x: donor.x as i64, z: donor.z, species: Species::Springturf, wood: 0.06 }
    ));
    flora.step(&mut world);
    let banked = flora.view().ground_at(site).expect("ground").seed_organic(Species::Springturf);
    assert!((banked - package).abs() <= 1e-15, "{banked} landed, not one {package} package");

    // Now the wall. The donor's own column is one of the eight, so it loses its support and
    // is booked out as a pruned site — which is why the residuals are still checked.
    shut_the_sky(&mut world, site);
    let g = establishment_gates(&world.view(), site, &config.springturf);
    assert!(!g.light_ok, "the light gate did not shut: {g:?}");
    assert!(g.pore_ok && g.aeration_ok && g.depth_ok, "another gate shut too: {g:?}");
    assert_eq!(g.soil_voxels, 2, "the root box still reaches both columns' faces: {g:?}");

    // Well past the bin's own expiry: it opened on tick 1, its lifetime is two ticks, and
    // `age_cohorts` takes a bin on the first tick whose age is greater than that.
    run(&mut flora, &mut world, 8);
    let v = flora.view();
    assert_eq!(v.ledger.establishments, 0, "a refused package germinated anyway");
    let g = v.ground_at(site).expect("ground");
    assert_eq!(g.seed_organic(Species::Springturf), 0.0, "the bank is still there: {:?}", g.seeds);
    assert!(g.seeds.is_empty(), "an empty cohort was left behind: {:?}", g.seeds);
    assert!(
        (g.litter - package).abs() <= 1e-12,
        "the litter holds {} of a {package} package",
        g.litter
    );
    assert!(g.litter_mineral > 0.0, "the package's mineral did not reach the litter");
    assert!(g.litter_energy > 0.0, "the package's energy did not reach the litter");
    assert_residuals(&flora, "after a refused package expired");
}

/// The rest of the role, where the model can carry it: springturf is the species that gives
/// up first on dry ground, and an adult springturf under a crown reads less light than one
/// in the open.
///
/// The dry half is a gate reading — at 0.2 of pore capacity springturf's 0.25 floor shuts
/// and bloomcrown's 0.1 does not — and the canopy half is the shade model, which *does* see
/// crowns. Only germination light is canopy-blind.
#[test]
fn springturf_gives_up_first_on_dry_ground_and_loses_light_under_a_crown() {
    let config = FloraConfig::default();

    // Dry ground: 0.2 of capacity, below soil's own retained 0.25.
    let dry = pillars(4, &[0, 1, 2, 3], 0.2);
    let turf = establishment_gates(&dry.view(), at(1), &config.springturf);
    let bloom = establishment_gates(&dry.view(), at(1), &config.bloomcrown);
    assert!(!turf.pore_ok, "springturf accepted dry ground: {turf:?}");
    assert!(bloom.pore_ok && bloom.passes(), "bloomcrown refused the same ground: {bloom:?}");
    assert!(
        (turf.mean_pore.expect("soil") - 0.2).abs() < 1e-12,
        "the fixture is not at 0.2: {turf:?}"
    );

    // Under a crown: a full-grown bloomcrown at x2 covers x1 and x3 and stands three voxels
    // over their one, so the two springturfs beside it are shaded and the one at x5 is not.
    let mut world = pillars(8, &[1, 2, 3, 5], 0.6);
    let mut flora = Flora::new(config);
    for (x, species, wood) in [
        (2i64, Species::Bloomcrown, 0.6),
        (1, Species::Springturf, 0.06),
        (5, Species::Springturf, 0.06),
    ] {
        assert!(flora.apply(&world, Command::Seed { x, z: 0, species, wood }));
    }
    flora.step(&mut world);
    let v = flora.view();
    let shaded = v.stand_at(at(1)).expect("shaded springturf").light;
    let open = v.stand_at(at(5)).expect("open springturf").light;
    assert!((open - 1.0).abs() < 1e-12, "the open springturf read {open}");
    assert!(shaded < open, "the shaded springturf read {shaded}, the open one {open}");
    // `shade_k` 1.5 is a known-wrong placeholder — one full crown attenuates by about 9 %
    // (`design/backlog.md` §1) — so this is a small difference by construction. What is
    // being pinned is its sign and that it is the crown doing it, not its size.
    assert!(shaded > 0.8 * open, "an unexpectedly deep shade: {shaded} against {open}");
}
// =========================================================================== validity

/// **Validity.** Every one of the five presets passes [`SpeciesConfig::validate`], and the
/// four ways a preset can be silently unable to produce a living stand are each refused with
/// the species and the field named: a split that does not sum to one, `alive_min` over
/// `wood_max`, a zero wood fraction, and a non-finite rate.
///
/// `Flora::new` calls the same check, so a broken config cannot reach a tick at all; the
/// `try_new` half of this test is what a future `[flora]` TOML table would print.
#[test]
fn every_preset_validates_and_a_broken_clone_does_not() {
    let config = FloraConfig::default();
    assert!(config.validate().is_ok(), "the shipped config: {:?}", config.validate());
    for species in Species::ALL {
        let sc = config.species(species);
        sc.validate(species.name())
            .unwrap_or_else(|e| panic!("a shipped preset is invalid: {e}"));
        // The two properties the validator exists for, stated on every preset: a split that
        // conserves organic matter, and a package that builds a stand which is alive.
        let [w, p, q] = sc.propagule_split;
        assert!((w + p + q - 1.0).abs() < 1e-15, "{}: {:?}", species.name(), sc.propagule_split);
        assert!(w > 0.0 && sc.alive_min <= sc.wood_max, "{}: {sc:?}", species.name());
    }

    // A split summing to 1.1: every germination would create organic matter out of nothing.
    let mut broken = FloraConfig::default();
    broken.springturf.propagule_split = [0.5, 0.4, 0.2];
    let e = broken.validate().expect_err("a 1.1 split was accepted");
    assert!(e.contains("springturf") && e.contains("sums to"), "{e}");
    assert!(Flora::try_new(broken).is_err());

    // `alive_min` over `wood_max`: every newborn dies on its first growth tick.
    let mut broken = FloraConfig::default();
    broken.stonecushion.alive_min = broken.stonecushion.wood_max * 2.0;
    let e = broken.validate().expect_err("alive_min over wood_max was accepted");
    assert!(e.contains("stonecushion") && e.contains("alive_min"), "{e}");

    // A zero wood fraction: no package size, so the species can never germinate and never
    // says why. `package_of` returns 0.0 and germination skips it in silence.
    let mut broken = FloraConfig::default();
    broken.velvetpad.propagule_split = [0.0, 0.6, 0.4];
    let e = broken.validate().expect_err("a zero w_frac was accepted");
    assert!(e.contains("velvetpad") && e.contains("w_frac"), "{e}");

    // A non-finite rate: one stand's stocks become NaN and then the whole ledger does.
    let mut broken = FloraConfig::default();
    broken.bloomcrown.maintenance = f64::NAN;
    let e = broken.validate().expect_err("a NaN rate was accepted");
    assert!(e.contains("bloomcrown") && e.contains("maintenance"), "{e}");

    // And the shared rates, which are not a species' own.
    let mut broken = FloraConfig::default();
    broken.shade_k = f64::NEG_INFINITY;
    let e = broken.validate().expect_err("an infinite shade_k was accepted");
    assert!(e.contains("shade_k"), "{e}");
}
