//! Round 3's three corrections, tested by someone who did not write them.
//!
//! This file is deliberately separate from `flora.rs` and `model.rs`: those tests were
//! written by the author of the code and confirm the author's own reading of it. What is
//! here is what that reading leaves unstated — the arithmetic of the two currencies
//! across a whole life cycle, the *shape* of the aeration response rather than its two
//! endpoints, the rules that decide between two species, and the places where a design
//! call has a consequence nobody wrote down.
//!
//! Conventions, the same ones `flora.rs` uses so a fixture here reads the same way:
//! `voxel_m` is 1 m, so a soil voxel holds `Material::Soil.pore_capacity()` = 0.35 m³ of
//! pore water and a metre of free water over a face is a metre deep. Soil is wetted by
//! adding free water to an air cell and then turning the cell to soil, which converts the
//! water to pore water and displaces nothing while it fits.
//!
//! Where a test needs a rate the placeholders do not give, it sets that rate in its
//! **own** config and says why in a comment. No test here changes a default and then
//! reads the default back.
//!
//! A test whose failure is a fact about the model rather than about the test is kept,
//! `#[ignore]`d, with the measured numbers in the reason string. Round 2's package G set
//! that convention.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Ground, Site, Species, SpeciesConfig};

// ------------------------------------------------------------------- fixtures

/// A strip of `depth` slabs: bedrock at `y = 0`, soil at `y = 1..=2` at a chosen pore
/// fraction, air above. Every column's support face is `y = 2`, in open sky.
/// Wet enough for every species of these fixtures on any soil: available water 1.5 on
/// package F's scale (field capacity is 1), so the wetland umbrellafrond is at full
/// moisture and can germinate, the upland bloomcrown is too, and no voxel reaches
/// `saturated_pore` on any soil in the soil-retention window.
fn wet() -> f64 {
    let wp = Material::Soil.wilting_point();
    wp + 1.5 * (Material::Soil.field_capacity() - wp)
}

fn strip(width: u32, depth: u32, pore: f64) -> World {
    let config = VoxelConfig {
        width,
        height: 8,
        depth,
        voxel_m: 1.0,
        seed: 5,
        ..VoxelConfig::default()
    };
    let mut w = World::empty(config);
    for z in 0..depth {
        for x in 0..width as i64 {
            for y in 1..=2 {
                fill(&mut w, x, y, z, Material::Soil, pore);
            }
        }
    }
    w
}

/// A four-column strip whose last column is **void**: bedrock-free, soilless air, so it
/// has no support face at all. A `hop`-1 donor at `x0` therefore has exactly **one**
/// recipient, `x1` — `x3` offers nothing to land on and `x2` is out of reach — which pins
/// the landing site under any dispersal rule, including round 3b's one-package-at-a-time
/// draw. Every other column's support face is `y = 2`, in open sky.
fn strip_gap(pore: f64) -> World {
    let config = VoxelConfig {
        width: 4,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 5,
        ..VoxelConfig::default()
    };
    let mut w = World::empty(config);
    for x in 0..3i64 {
        for y in 1..=2 {
            fill(&mut w, x, y, 0, Material::Soil, pore);
        }
    }
    // `World::empty` lays a bedrock foundation across the whole footprint, and a bare
    // foundation cell is a support face like any other. Take x3's away, and the column has
    // nothing to land on at all. The world is never stepped in these fixtures, so the hole
    // in the foundation drains nothing.
    w.apply(cubarium_voxel::Command::SetMaterial {
        x: 3,
        y: 0,
        z: 0,
        material: Material::Air,
    });
    assert!(
        cubarium_voxel_flora::highest_support(&w.view(), 3, 0).is_none(),
        "x3 must be void"
    );
    w
}

/// Turn one air voxel into `material` holding exactly `pore` of that material's own pore
/// capacity, by adding the water first and converting after.
fn fill(w: &mut World, x: i64, y: u32, z: u32, material: Material, pore: f64) {
    let want = pore * material.pore_capacity() * w.config().voxel_volume();
    if want > 0.0 {
        let got = w.apply(WorldCommand::AddWater {
            x,
            y,
            z,
            volume_m3: want,
        });
        assert!((got - want).abs() < 1e-12, "the void took {got} of {want}");
    }
    w.apply(WorldCommand::SetMaterial { x, y, z, material });
    assert!(
        (w.view().pore_at(x, y, z) - pore).abs() < 1e-12,
        "pore {} at ({x},{y},{z})",
        w.view().pore_at(x, y, z)
    );
    assert_eq!(
        w.view().free_at(x, y, z),
        0.0,
        "nothing may be left standing"
    );
}

/// Take pore water out of one voxel down to `target`, through the core's own bounded
/// withdrawal — the only way to *dry* a voxel without rebuilding it.
fn drain_to(w: &mut World, x: i64, y: u32, z: u32, target: f64) {
    let cap = Material::Soil.pore_capacity() * w.config().voxel_volume();
    let want = (w.view().pore_at(x, y, z) - target) * cap;
    assert!(want > 0.0, "already at or below {target}");
    let took = -w.apply(WorldCommand::WithdrawPore {
        x,
        y,
        z,
        volume_m3: want,
    });
    assert!(
        (took - want).abs() < 1e-12,
        "the core gave {took} of {want}"
    );
    assert!((w.view().pore_at(x, y, z) - target).abs() < 1e-12);
}

fn at(x: u32, z: u32) -> Site {
    Site { x, y: 2, z }
}

fn site(x: u32) -> Site {
    at(x, 0)
}

fn run(flora: &mut Flora, world: &mut World, ticks: u32) {
    for _ in 0..ticks {
        flora.step(world);
    }
}

/// The three residuals. Organic matter against its two named boundary flows, mineral
/// against nothing but seeding and removal, energy as in v1.
fn residuals(flora: &Flora) -> (f64, f64, f64) {
    let v = flora.view();
    (
        v.organic() - v.ledger.expected_organic(),
        v.mineral() - v.ledger.expected_mineral(),
        v.energy() - v.ledger.expected_energy(),
    )
}

fn assert_residuals(flora: &Flora, when: &str) {
    let v = flora.view();
    let (o, n, e) = residuals(flora);
    assert!(
        o.abs() <= 1e-9 * v.organic().abs().max(1.0),
        "{when}: organic residual {o}"
    );
    assert!(
        n.abs() <= 1e-9 * v.mineral().abs().max(1.0),
        "{when}: mineral residual {n}"
    );
    assert!(
        e.abs() <= 1e-9 * v.energy().abs().max(1.0),
        "{when}: energy residual {e}"
    );
}

fn bank_organic(g: &Ground, species: Species) -> f64 {
    g.seed_organic(species)
}

// ============================================================ the two currencies

/// Mineral is a **closed** stock: the only way in is a `Seed` or a new site's
/// `initial_mineral`, and the only way out is a removal. So over a run with one seeding
/// phase and no terrain edit, every unit of mineral in the world must still be there,
/// whatever it has been through — and this run puts it through the whole cycle in one
/// go: a founder that grows and draws mineral out of its site's pool, pays a propagule
/// package out of its reserve, two banks that germinate into stands, a second founder
/// that starves to death, and the litter and dead wood it leaves decomposing back into
/// the pool.
///
/// Two rates are the test's own. `umbrellafrond.propagule_rate` is 2.0 /s against the
/// placeholder 2e-4, so one donor's whole spendable reserve crosses the germination
/// threshold in a single tick and the test is 400 ticks rather than 40,000;
/// `bloomcrown.maintenance` is 0.4 /s against the placeholder 0.0002, so the second
/// founder starves inside thirty ticks. Neither rate is read back as a placeholder.
#[test]
fn mineral_is_conserved_across_a_whole_life_cycle() {
    let mut config = FloraConfig::default();
    config.umbrellafrond.propagule_rate = 2.0;
    config.bloomcrown.maintenance = 0.4;
    let mut world = strip(8, 1, wet());
    let mut flora = Flora::new(config);

    // The donor: umbrellafrond, below `wood_max` so it really grows, with `hop` 1 so it
    // reaches exactly x1 and x3.
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Umbrellafrond,
            wood: 0.5
        }
    ));
    // The victim: a bloomcrown at `alive_min`, far outside the donor's reach.
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 6,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.02
        }
    ));

    let seeded_mineral = flora.view().ledger.seeded_mineral_in;
    let mineral0 = flora.view().mineral();
    assert!(
        (mineral0 - seeded_mineral).abs() < 1e-15,
        "before anything moves, every unit of mineral is a seeded one: {mineral0} against {seeded_mineral}"
    );
    let pool0 = flora
        .view()
        .ground_at(site(2))
        .expect("the donor's ground")
        .mineral;

    run(&mut flora, &mut world, 400);

    let v = flora.view();
    // The cycle really happened, all of it. Round 3b: the donor saves for one recipient at
    // a time and draws which, so in 400 ticks it recruited one of its two neighbours and
    // not both — one germination is what the cycle needs, and the test no longer names the
    // column.
    assert!(v.ledger.establishments >= 1, "no bank germinated at all");
    assert_eq!(
        v.ledger.deaths, 1,
        "the victim did not die, or something else did"
    );
    let born: Vec<&cubarium_voxel_flora::Stand> = v
        .stands
        .iter()
        .filter(|s| s.site != site(2) && s.site != site(6))
        .collect();
    assert!(!born.is_empty(), "nothing germinated anywhere");
    for s in &born {
        assert!(
            [site(1), site(3)].contains(&s.site),
            "born outside the donor's hop: {s:?}"
        );
        assert_eq!(s.species, Species::Umbrellafrond);
        assert!(
            s.mineral > 0.0,
            "a germinated stand holds no mineral: {s:?}"
        );
    }
    let donor = v.stand_at(site(2)).expect("the donor");
    assert!(donor.wood > 0.5, "the donor never grew: {}", donor.wood);
    let pool = v.ground_at(site(2)).expect("ground").mineral;
    assert!(
        pool < pool0,
        "the donor's growth drew no mineral: {pool0} -> {pool}"
    );
    let grave = v.ground_at(site(6)).expect("the victim's ground");
    assert!(
        grave.litter > 0.0 && grave.dead_wood > 0.0,
        "the victim left no remains: {grave:?}"
    );
    assert!(
        grave.mineral > flora.config().initial_mineral,
        "decomposition returned no mineral to the grave's pool: {} against {}",
        grave.mineral,
        flora.config().initial_mineral
    );
    assert!(grave.litter_mineral > 0.0, "the litter carries no mineral");

    // And the total is untouched. `removed_*` is zero here, so `expected_mineral` is the
    // seeding alone: the number the world started with.
    assert_eq!(v.ledger.removed_mineral_out, 0.0, "nothing was removed");
    // Conserved to f64 noise, which is not the same as "to the bit": mineral moves by a
    // paired subtract-and-add between stocks of very different magnitudes — a 1e-9
    // propagule package out of a 1.0 pool — and `(a − x) + (b + x)` is not `a + b` in f64.
    // Measured here: a residual of 3.6e-15 on a stock of 4.04, about four ulps of the
    // total, after 400 ticks of the whole cycle. On the pure-respiration fixture
    // (`respiration_is_the_only_organic_leak…`) it is exactly zero. The commit message's
    // "conserved to the bit" is the right *intent* and the wrong tolerance to test at.
    let total = v.mineral();
    let (_, n, _) = residuals(&flora);
    assert!(
        n.abs() <= 1e-14 * total,
        "mineral is not conserved: residual {n} on a stock of {total} (seeded {seeded_mineral})"
    );
    // Nothing anywhere holds negative mineral.
    for s in v.stands {
        assert!(s.mineral >= 0.0, "negative stand mineral: {s:?}");
    }
    for g in v.ground {
        assert!(
            g.mineral >= 0.0 && g.litter_mineral >= 0.0 && g.dead_wood_mineral >= 0.0,
            "negative ground mineral: {g:?}"
        );
        for c in &g.seeds {
            assert!(c.mineral >= 0.0, "negative cohort mineral: {c:?}");
        }
    }
    assert_residuals(&flora, "after a whole life cycle");
}

/// Organic matter has exactly two boundary flows, and on a world where nothing can fix
/// light there is only one: every unit of organic matter that leaves is respired, and
/// `respired_out` accounts for the whole of the stock's fall. Nothing decays into a
/// residual, and nothing leaks through the litter energy cap either — with the
/// placeholders, `energy_density` and `litter_energy_cap` are both 2.0, so heat leaves in
/// lockstep with organic matter at `e_v` per unit and the cap never bites.
///
/// Dry soil (pore 0) is what makes `fixed_in` exactly zero: `μ = 0` is a hard zero in the
/// income, not a small number. `bloomcrown.maintenance` is 0.4 /s (placeholder 0.0002) so
/// the stand spends its reserve, diebacks and dies inside the run.
#[test]
fn respiration_is_the_only_organic_leak_and_it_takes_its_energy_with_it() {
    let mut config = FloraConfig::default();
    config.bloomcrown.maintenance = 0.4;
    let e_v = config.bloomcrown.energy_density;
    assert_eq!(
        e_v, config.litter_energy_cap,
        "the fixture's premise: e_v == e_d_max"
    );
    let mut world = strip(4, 1, 0.0);
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 1,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.6
        }
    ));
    let seeded_organic = flora.view().ledger.seeded_organic_in;
    let seeded_energy = flora.view().ledger.seeded_energy_in;

    run(&mut flora, &mut world, 400);

    let v = flora.view();
    assert_eq!(
        v.ledger.fixed_in, 0.0,
        "a wilting stand fixed organic matter"
    );
    assert_eq!(v.ledger.light_in, 0.0, "a wilting stand fixed light");
    assert_eq!(v.ledger.removed_organic_out, 0.0, "nothing was removed");
    assert_eq!(v.ledger.deaths, 1, "the stand did not die");
    assert!(v.ledger.respired_out > 0.0, "nothing respired");

    // The one flow accounts for the whole fall of the stock.
    let fell = seeded_organic - v.organic();
    assert!(
        (fell - v.ledger.respired_out).abs() <= 1e-14 * seeded_organic,
        "organic matter fell by {fell} and respired_out is {}",
        v.ledger.respired_out
    );
    // And the energy left with it, at `e_v` per unit, with nothing lost to the cap.
    let heat = seeded_energy - v.energy();
    assert!(
        (heat - e_v * v.ledger.respired_out).abs() <= 1e-12 * seeded_energy,
        "heat {heat} against e_v * respired_out {}",
        e_v * v.ledger.respired_out
    );
    assert!(
        (v.ledger.heat_out - e_v * v.ledger.respired_out).abs() <= 1e-12 * seeded_energy,
        "heat_out {} against e_v * respired_out {}: the litter energy cap is leaking",
        v.ledger.heat_out,
        e_v * v.ledger.respired_out
    );
    // Mineral is closed even here, where organic matter is not.
    assert_eq!(v.ledger.removed_mineral_out, 0.0);
    let (_, n, _) = residuals(&flora);
    // Round 3b: not exactly zero any more, and 1.1e-15 on a stock of 2.04 is about five
    // ulps of it. The stand is over `donor_min`, so it saves a parcel out of its reserve
    // before it starves, and every transfer out of it — litterfall, dieback, the parcel's
    // own share at delivery — is a paired subtract and add against its **material**
    // including that parcel. Nothing crosses the boundary: `expected_mineral` is still the
    // seeding alone.
    assert!(
        n.abs() <= 1e-14 * flora.view().mineral(),
        "mineral residual {n} in a world with no income at all"
    );
    assert_residuals(&flora, "after 400 ticks of pure respiration");
}

/// A propagule package's construction respiration destroys organic matter and releases no
/// mineral — and round 3b moved **where** that leaves the mineral. The donor is debited
/// for the package when the package leaves, by the fraction rule over its whole material,
/// so what travels is the package's own share at the donor's own density. The mineral of
/// the `c_g` that was respired stays in the donor, exactly as the mineral of burned
/// maintenance reserve does.
///
/// Round 3 sent the mineral of the **gross** `1 + c_g` along with the net package, which
/// made every cohort — and every stand born of one — `1 + c_g` times as mineral-rich per
/// unit as its parent, at a density that had nothing to do with `n_tissue`. That factor is
/// gone. What is left is a slow enrichment of a **saving** donor: between deliveries its
/// parcel holds organic matter whose mineral is still in the stand, so its density creeps
/// above `n_tissue` and each package leaves at the density of the moment it leaves.
///
/// The donor is **frozen** — `assimilation`, `maintenance` and `senescence` all zero — so
/// its stocks move only where the propagule debits them and the arithmetic can be checked
/// exactly. `propagule_rate` is 0.18 /s (placeholder 2e-4), which is 0.0075 of parcel a
/// tick, so the first package is full on the seventh tick; `hop` is 1 (placeholder 2) so
/// there are two candidate recipients on a four-column ring and one of them receives.
#[test]
fn a_package_keeps_all_its_mineral_through_construction_respiration() {
    let mut config = FloraConfig::default();
    let sc = &mut config.bloomcrown;
    sc.assimilation = 0.0;
    sc.maintenance = 0.0;
    sc.senescence = 0.0;
    sc.hop = 1;
    sc.propagule_rate = 0.18;
    let build = config.bloomcrown.build;
    let n_tissue = config.bloomcrown.n_tissue;
    let rate = config.bloomcrown.propagule_rate;
    let package = config.bloomcrown.alive_min / config.bloomcrown.propagule_split[0];
    let mut world = strip(4, 1, 0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 1,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.5
        }
    ));

    let before = *flora.view().stand_at(site(1)).expect("the donor");
    let organic0 = before.organic();
    let mineral0 = before.mineral;
    assert!(
        (mineral0 - n_tissue * organic0).abs() < 1e-18,
        "a founder arrives at exactly n_tissue: {mineral0} against {}",
        n_tissue * organic0
    );

    // Seven ticks: six of saving, and the seventh sends one package.
    run(&mut flora, &mut world, 7);

    let after = *flora.view().stand_at(site(1)).expect("still there");
    assert_eq!(
        flora.view().ledger.fixed_in,
        0.0,
        "the frozen donor earned something"
    );
    assert_eq!(after.wood, before.wood, "the frozen donor's wood moved");
    assert_eq!(
        after.foliage, before.foliage,
        "the frozen donor's foliage moved"
    );

    let gross = rate * DT;
    let net = gross / (1.0 + build);
    let banked: Vec<Site> = flora
        .view()
        .ground
        .iter()
        .filter(|g| !g.seeds.is_empty())
        .map(|g| g.site)
        .collect();
    assert_eq!(banked.len(), 1, "one package, one recipient: {banked:?}");
    assert!(
        [site(0), site(2)].contains(&banked[0]),
        "{banked:?} is not a hop-1 neighbour"
    );
    let g = flora.view().ground_at(banked[0]).expect("the recipient");
    assert_eq!(g.seeds.len(), 1, "one cohort: {:?}", g.seeds);
    let c = g.seeds[0];
    assert_eq!(c.species, Species::Bloomcrown);
    assert_eq!(c.bin_start_tick, 0, "the bin tick 0 opened");
    assert!(
        (c.organic - package).abs() <= 1e-15 * package,
        "{c:?} for a {package} package"
    );

    // The density that replaced the `1 + c_g` factor: the cohort is at the donor's own
    // density at the moment it left, which is `n_tissue` plus the enrichment of six ticks
    // of saving — well under `(1 + c_g) · n_tissue` and nowhere near it.
    let density = c.mineral / c.organic;
    assert!(
        density > n_tissue && density < (1.0 + build) * n_tissue,
        "cohort density {density} is outside (n_tissue {n_tissue}, (1 + c_g) n_tissue {})",
        (1.0 + build) * n_tissue
    );
    assert!(
        (density - n_tissue).abs() < 0.01 * n_tissue,
        "cohort density {density} against the donor's founding {n_tissue}: the old rule's \
         factor of {} is back",
        1.0 + build
    );

    // The donor was debited exactly what it saved, gross, in organic matter; and exactly
    // the package's fraction of its mineral, which is what arrived.
    let saved = 7.0 * net;
    let sent = organic0 - after.organic();
    assert!(
        (sent - 7.0 * gross).abs() <= 1e-15,
        "the donor's reserve fell by {sent} for seven ticks of {gross}"
    );
    assert!(
        (after.parcel - (saved - package)).abs() <= 1e-15,
        "the parcel holds {} of {saved} saved less one {package} package",
        after.parcel
    );
    let debited = mineral0 - after.mineral;
    assert!(
        (debited - c.mineral).abs() <= 1e-12 * debited,
        "the donor was debited {debited} of mineral and {} arrived",
        c.mineral
    );
    // Its material is conserved: what left the reserve is the parcel, the package and the
    // construction that was respired.
    let respired = flora.view().ledger.respired_out;
    assert!(
        (respired - build * saved).abs() <= 1e-15 * respired,
        "respired_out {respired} against c_g times {saved} saved"
    );
    let (_, n, _) = residuals(&flora);
    assert!(
        n.abs() <= 1e-14 * flora.view().mineral(),
        "mineral crossed the respiration boundary: residual {n} on a stock of {}",
        flora.view().mineral()
    );
}

/// The consequence of respiration leaving mineral behind, stated on the stand itself:
/// a stand that cannot pay its maintenance burns reserve, loses organic matter and keeps
/// every unit of its mineral, so its mineral density climbs without bound above
/// `n_tissue`; a stand that can pay grows new tissue at `n_tissue` and its density falls
/// back towards it.
///
/// `maintenance` is 0.4 /s for bloomcrown (the starving arm, placeholder 0.0002) and left
/// alone for umbrellafrond (the paying arm). Both stand in the same wet, open fixture, so
/// maintenance is the whole difference.
#[test]
fn a_starving_stand_gets_mineral_rich_and_a_paying_one_tends_to_n_tissue() {
    let mut config = FloraConfig::default();
    config.bloomcrown.maintenance = 0.4;
    let n_tissue = config.bloomcrown.n_tissue;
    assert_eq!(
        config.umbrellafrond.n_tissue, n_tissue,
        "the two species share it this round"
    );
    let mut world = strip(8, 1, wet());
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 1,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 5,
            z: 0,
            species: Species::Umbrellafrond,
            wood: 0.1
        }
    ));

    let starving0 = *flora.view().stand_at(site(1)).unwrap();
    run(&mut flora, &mut world, 20);

    let starving = *flora.view().stand_at(site(1)).expect("not dead yet");
    let paying = *flora.view().stand_at(site(5)).expect("alive");
    assert!(
        starving.organic() < starving0.organic(),
        "the starving arm did not lose organic matter"
    );
    // Its mineral fell too — senescence sheds foliage and litterfall takes the same
    // *fraction* of the mineral as of the organic matter — but strictly less of it, in
    // proportion, than its organic matter fell, because respiration took organic matter
    // and left the mineral where it was.
    let organic_kept = starving.organic() / starving0.organic();
    let mineral_kept = starving.mineral / starving0.mineral;
    assert!(
        mineral_kept > organic_kept,
        "kept {mineral_kept} of the mineral and {organic_kept} of the organic matter: \
         respiration moved mineral"
    );
    let starving_density = starving.mineral / starving.organic();
    assert!(
        starving_density > n_tissue,
        "the starving stand is not mineral-rich: {starving_density} against n_tissue {n_tissue}"
    );
    // The paying arm grew, and every unit it built cost `n_tissue`, so its density is
    // still exactly that.
    assert!(paying.mineral > 0.0);
    let paying_density = paying.mineral / paying.organic();
    assert!(
        (paying_density - n_tissue).abs() <= 1e-12 * n_tissue,
        "the paying stand's density is {paying_density}, not n_tissue {n_tissue}"
    );
    assert_residuals(&flora, "after twenty ticks of one starving and one paying");
}

/// An exhausted mineral pool does not merely stop growth: `A_pot`'s `mineral / n_tissue`
/// cap is on the income itself, and income is where maintenance is paid from first, so a
/// stand on a bare pool fixes **nothing** at full light and full moisture and has to burn
/// reserve to stand still. Worth pinning because the correction is written as a cap on
/// *growth* and it is not one.
#[test]
fn a_bare_mineral_pool_stops_the_income_and_not_just_the_growth() {
    let mut bare_config = FloraConfig::default();
    bare_config.initial_mineral = 0.0;
    let mut world = strip(4, 1, 0.6);
    let mut bare = Flora::new(bare_config);
    let mut ample = Flora::new(FloraConfig::default());
    for f in [&mut bare, &mut ample] {
        assert!(f.apply(
            &world,
            Command::Seed {
                x: 1,
                z: 0,
                species: Species::Bloomcrown,
                wood: 0.1
            }
        ));
    }
    let reserve0 = flora_reserve(&bare);

    let mut bare_world = world.clone();
    // The first tick: nothing has fallen yet, so the pool is still bare and the income is
    // a hard zero — not a small number, a zero.
    bare.step(&mut bare_world);
    assert_eq!(
        bare.view().ledger.fixed_in,
        0.0,
        "a bare pool fixed organic matter on the very first tick"
    );
    run(&mut bare, &mut bare_world, 49);
    run(&mut ample, &mut world, 50);

    let s = *bare.view().stand_at(site(1)).expect("alive");
    assert_eq!(s.light, 1.0, "open sky");
    assert_eq!(s.moisture, 1.0, "past full_water");
    assert_eq!(
        s.aeration_stress, 0.0,
        "pore 0.6 is nowhere near saturated_pore"
    );
    assert!(
        flora_reserve(&bare) < reserve0,
        "it paid nothing out of reserve"
    );
    assert!(
        ample.view().ledger.fixed_in > 0.0,
        "the ample control fixed nothing either"
    );
    // After fifty ticks it is *not* zero any more, and that is the second unstated thing
    // here: the stand's own senescence puts mineral into the site's litter, decomposition
    // hands a fraction of it back to the pool every tick, and the stand starts earning
    // again off what it shed. An exhausted pool is not a permanent floor — it recovers on
    // the litter's decomposition timescale — so "growth stops when the pool is spent" is
    // asymptotic and not absolute.
    let bare_fixed = bare.view().ledger.fixed_in;
    assert!(
        bare_fixed > 0.0,
        "the litter loop did not restart the income at all"
    );
    assert!(
        bare_fixed < 1e-6 * ample.view().ledger.fixed_in,
        "a bare pool earned {bare_fixed} against the ample control's {}",
        ample.view().ledger.fixed_in
    );
    let pool = bare.view().ground_at(site(1)).expect("ground").mineral;
    assert!(
        pool > 0.0,
        "the litter released no mineral to the bare pool"
    );
    assert_residuals(&bare, "after 50 ticks on a bare pool");
}

fn flora_reserve(flora: &Flora) -> f64 {
    flora.view().stands.iter().map(|s| s.reserve).sum()
}

// =========================================================== funded reproduction

/// A **stressed** donor asks for its rate every tick and is funded nothing, so its parcel
/// never grows and nothing ever lands. This is the distinction Astra's R4.4 asked the
/// diagnosis to make: `propagule_rate` is not the binding constraint on a stand that
/// cannot pay, and raising it would create no income at all.
///
/// The instrument is `donor_reserve_floor` 1.0 (placeholder 0.5): the donor keeps its
/// whole reserve for itself, so its surplus is exactly zero however much reserve it has.
/// `propagule_rate` is 3.0 /s (placeholder 2e-4) so the *ask* is large and visible — it is
/// the funding that is absent, not the request. `assimilation` and `maintenance` are 0
/// (placeholders 0.004 and 0.0002) so nothing else moves the reserve in either direction
/// and it can be read back to the bit.
#[test]
fn a_stressed_donor_asks_every_tick_and_its_parcel_never_grows() {
    let mut config = FloraConfig::default();
    config.bloomcrown.donor_reserve_floor = 1.0;
    config.bloomcrown.propagule_rate = 3.0;
    config.bloomcrown.assimilation = 0.0;
    config.bloomcrown.maintenance = 0.0;
    let mut world = strip(5, 1, 0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.6
        }
    ));
    let reserve0 = flora.view().stand_at(site(2)).expect("seeded").reserve;
    let sc = flora.config().species(Species::Bloomcrown).clone();
    let net = sc.propagule_rate * DT / (1.0 + sc.build);

    run(&mut flora, &mut world, 100);

    let donor = *flora.view().stand_at(site(2)).expect("alive, just broke");
    assert_eq!(
        donor.parcel, 0.0,
        "an unfunded parcel grew: {}",
        donor.parcel
    );
    assert_eq!(
        donor.reserve, reserve0,
        "the reserve moved: {} -> {}",
        reserve0, donor.reserve
    );
    assert!(
        flora.view().ground.iter().all(|g| g.seeds.is_empty()),
        "an unfunded donor landed something: {:?}",
        flora.view().ground
    );
    let l = flora.view().ledger;
    let i = Species::Bloomcrown.index();
    assert!(
        (l.propagule_requested[i] - 100.0 * net).abs() <= 1e-12 * 100.0 * net,
        "it asked for {} over a hundred ticks of {net}",
        l.propagule_requested[i]
    );
    assert_eq!(
        l.propagule_funded[i], 0.0,
        "a donor with no surplus was funded"
    );
    assert_eq!(
        l.propagule_landed[i], 0.0,
        "a donor with no surplus landed something"
    );
    assert_eq!(l.establishments, 0);
    assert_residuals(&flora, "after a hundred ticks of an unfunded donor");
}

/// A parcel **dies with its donor**, booked into the site's litter with its share of the
/// donor's mineral. Nothing paid for is deleted and nothing is stranded.
///
/// The donor saves for a while, then starves: `maintenance` is 0.4 /s (placeholder 0.0002)
/// and `assimilation` 0, so it spends its reserve, diebacks and drops under `alive_min`
/// inside eighty ticks; `propagule_rate` is 0.06 /s (placeholder 2e-4), which is 0.0025 of
/// parcel a tick, so what it manages to save before the reserve reaches the floor stays
/// **under** one 0.05 package and is still in the parcel when it dies. `senescence` is 0
/// so the site's litter is the death alone and the arithmetic can be read exactly.
#[test]
fn a_parcel_dies_with_its_donor_and_reaches_the_litter() {
    let mut config = FloraConfig::default();
    config.bloomcrown.maintenance = 0.4;
    config.bloomcrown.assimilation = 0.0;
    config.bloomcrown.senescence = 0.0;
    config.bloomcrown.propagule_rate = 0.06;
    config.decomposition = 0.0;
    config.wood_decomposition = 0.0;
    let e_v = config.bloomcrown.energy_density;
    let mut world = strip(5, 1, 0.6);
    let mut flora = Flora::new(config);
    let home = site(2);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.6
        }
    ));

    // Step until it dies, keeping the last tick it was alive and its site's litter.
    let mut last = *flora.view().stand_at(home).expect("seeded");
    let mut litter_before = 0.0;
    let mut died_at = None;
    for tick in 1..=200u32 {
        let g = flora
            .view()
            .ground_at(home)
            .map(|g| g.litter)
            .unwrap_or(0.0);
        flora.step(&mut world);
        match flora.view().stand_at(home) {
            Some(s) => last = *s,
            None => {
                litter_before = g;
                died_at = Some(tick);
                break;
            }
        }
    }
    let died_at = died_at.expect("the donor outlived two hundred ticks");
    assert_eq!(flora.view().ledger.deaths, 1, "it did not die, or twice");
    assert!(
        last.parcel > 0.0,
        "it never saved anything to lose: {last:?}"
    );
    assert!(
        last.parcel < 0.05,
        "the fixture's premise: the parcel is under one package, not a sent one: {}",
        last.parcel
    );

    // The tick it died on: `senescence` and income are off, so the litter it left is
    // exactly its foliage, its reserve and its parcel, and the dead wood is its wood.
    let g = flora.view().ground_at(home).expect("the grave").clone();
    let want = last.foliage + last.reserve + last.parcel;
    assert!(
        (g.litter - litter_before - want).abs() <= 1e-12 * want,
        "tick {died_at}: the litter rose by {} for foliage {} + reserve {} + parcel {}",
        g.litter - litter_before,
        last.foliage,
        last.reserve,
        last.parcel
    );
    assert!(
        g.litter > litter_before + last.parcel,
        "the parcel did not reach the litter at all"
    );
    assert!(g.litter_mineral > 0.0, "the litter carries no mineral");
    assert!(
        (g.litter_energy - e_v * g.litter).abs() <= 1e-12 * g.litter_energy,
        "litter energy {} against e_v times litter {}",
        g.litter_energy,
        e_v * g.litter
    );
    // Nothing was stranded: the three residuals are the check that the parcel is neither
    // lost nor conjured, and a parcel dropped on death would show up in the organic one.
    assert_residuals(&flora, "after a donor died holding a parcel");
}

// ================================================================== the seed bank

/// A site losing its support takes the whole site out in three currencies: the stand on
/// it, the bank waiting under that stand, the litter and dead wood, **and the mineral
/// pool itself**, which is the one stock with no organic matter attached to it and the
/// easiest to forget. A pruned stand is not a death, either.
///
/// Three rates are the test's own, all to build the state in a hundred ticks:
/// `bloomcrown.propagule_rate` 2.0 /s and `hop` 1 so two donors bank on the middle site;
/// `umbrellafrond.reserve_cap` 0.0 and `maintenance` 0.01 /s so the stand on that site
/// has no reserve to pay from, diebacks every tick and piles up dead wood while staying
/// alive — a stand in continuous dieback is the only way to have live wood and dead wood
/// on one site at once.
#[test]
fn a_pruned_site_books_its_stand_its_bank_and_its_pool_in_three_currencies() {
    let mut config = FloraConfig::default();
    config.bloomcrown.propagule_rate = 2.0;
    config.bloomcrown.hop = 1;
    config.umbrellafrond.reserve_cap = 0.0;
    config.umbrellafrond.maintenance = 0.01;
    let e_bloom = config.bloomcrown.energy_density;
    let e_frond = config.umbrellafrond.energy_density;
    let mut world = strip(5, 1, 0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Umbrellafrond,
            wood: 0.5
        }
    ));
    for x in [1i64, 3] {
        assert!(flora.apply(
            &world,
            Command::Seed {
                x,
                z: 0,
                species: Species::Bloomcrown,
                wood: 0.6
            }
        ));
    }

    run(&mut flora, &mut world, 100);

    let doomed = site(2);
    let stand = *flora
        .view()
        .stand_at(doomed)
        .expect("the victim is still standing");
    let g = flora.view().ground_at(doomed).expect("ground").clone();
    assert!(
        !g.seeds.is_empty(),
        "no bank waiting under the stand: {g:?}"
    );
    assert!(g.litter > 0.0, "no litter");
    assert!(
        g.dead_wood > 0.0,
        "no dead wood: a stand in dieback left none"
    );
    assert!(g.mineral > 0.0, "no pool");
    assert!(
        g.litter_mineral > 0.0 && g.dead_wood_mineral > 0.0,
        "detritus carries no mineral"
    );

    let want_organic =
        stand.organic() + g.litter + g.dead_wood + g.seeds.iter().map(|c| c.organic).sum::<f64>();
    let want_mineral = stand.mineral
        + g.mineral
        + g.litter_mineral
        + g.dead_wood_mineral
        + g.seeds.iter().map(|c| c.mineral).sum::<f64>();
    let want_energy = e_frond * stand.organic()
        + g.litter_energy
        + g.dead_wood_energy
        + g.seeds.iter().map(|c| e_bloom * c.organic).sum::<f64>();
    let l0 = flora.view().ledger.clone();

    // Bury it: a solid over the support face is no longer a support face.
    world.apply(WorldCommand::SetMaterial {
        x: 2,
        y: 3,
        z: 0,
        material: Material::Soil,
    });
    assert!(
        !world.view().is_support(2, 2, 0),
        "burying did not take the support away"
    );
    flora.step(&mut world);

    let l = flora.view().ledger;
    let (d_o, d_n, d_e) = (
        l.removed_organic_out - l0.removed_organic_out,
        l.removed_mineral_out - l0.removed_mineral_out,
        l.removed_energy_out - l0.removed_energy_out,
    );
    assert!(
        (d_o - want_organic).abs() <= 1e-14 * want_organic,
        "organic out {d_o} against the site's {want_organic}"
    );
    assert!(
        (d_n - want_mineral).abs() <= 1e-14 * want_mineral,
        "mineral out {d_n} against the site's {want_mineral} — the pool is the easy one to miss"
    );
    assert!(
        (d_e - want_energy).abs() <= 1e-14 * want_energy,
        "energy out {d_e} against the site's {want_energy}"
    );
    assert_eq!(l.deaths, l0.deaths, "a pruned stand is not a death");
    assert!(
        flora.view().stand_at(doomed).is_none(),
        "the stand outlived its support"
    );
    assert!(
        flora.view().ground_at(doomed).is_none(),
        "the ground outlived its support"
    );
    assert_residuals(&flora, "after a support was buried under a whole site");
}

/// The gap a *drowning* opens is filled in the same tick it opens. Drowning is step 3 of
/// the tick and germination is step 8, so a bank that was waiting under a stand becomes a
/// stand in the single step that killed its predecessor — and since K7 it does that
/// **before** the tick's attrition is charged, so the newborn is a whole package exactly
/// and not a package minus a tick of decay.
///
/// `umbrellafrond.propagule_rate` is 3.0 /s (placeholder 2e-4) so the bank is over the
/// germination threshold in one tick. The water is the fixture's: 0.1 m over the support
/// face is past bloomcrown's `drown_depth_m` of 0.05 and well under umbrellafrond's 0.5,
/// so one species drowns where the other can stand.
#[test]
fn a_drowned_stand_s_gap_is_filled_by_its_bank_in_the_same_tick() {
    let mut config = FloraConfig::default().drop_seeds_checked_each_tick();
    config.umbrellafrond.propagule_rate = 3.0;
    let mut world = strip(5, 1, wet());
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 0,
            z: 0,
            species: Species::Umbrellafrond,
            wood: 0.6
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 1,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.6
        }
    ));

    run(&mut flora, &mut world, 20);
    let victim = site(1);
    assert_eq!(
        flora.view().stand_at(victim).map(|s| s.species),
        Some(Species::Bloomcrown),
        "the bloomcrown should still be standing on the bank"
    );
    let g = flora.view().ground_at(victim).expect("ground");
    let bank = bank_organic(g, Species::Umbrellafrond);
    let bank_mineral = g.seed_mineral(Species::Umbrellafrond);
    let sc = flora.config().species(Species::Umbrellafrond).clone();
    assert!(
        sc.propagule_split[0] * bank >= sc.alive_min,
        "the bank is not over the threshold: {bank}"
    );
    let (deaths0, est0) = (
        flora.view().ledger.deaths,
        flora.view().ledger.establishments,
    );

    // 0.1 m³ in a one-metre voxel over the face: a tenth of a metre of standing water.
    let got = world.apply(WorldCommand::AddWater {
        x: 1,
        y: 3,
        z: 0,
        volume_m3: 0.1,
    });
    assert!((got - 0.1).abs() < 1e-12, "the void took {got}");
    assert!((world.view().water_depth_m(1, 2, 0) - 0.1).abs() < 1e-12);

    // One single step.
    flora.step(&mut world);

    assert_eq!(
        flora.view().ledger.deaths,
        deaths0 + 1,
        "the bloomcrown did not drown"
    );
    assert_eq!(
        flora.view().ledger.establishments,
        est0 + 1,
        "the gap was not filled in the tick that opened it"
    );
    let born = *flora.view().stand_at(victim).expect("the gap stayed empty");
    assert_eq!(born.species, Species::Umbrellafrond);
    // Its stocks are **one package**, not the whole bank: round 3b's germination spends
    // one `alive_min / w_frac` and leaves the rest ageing (Astra R4.5). The bank here held
    // two packages, so the newborn is half of it.
    let package = sc.alive_min / sc.propagule_split[0];
    // A whole package, to one ulp — the three split products re-sum to 0.05 where the
    // package itself is 0.049999999999999996 — and not the package minus a tick of decay,
    // because since K7 germination reads the bank before the attrition of its own tick.
    assert!(
        (born.organic() - package).abs() <= 1e-16,
        "born with {} out of a {bank} bank, for a {package} package",
        born.organic()
    );
    // And the wood is `alive_min` exactly: a newborn is a minimum viable stand, on the
    // nose, at any bank size.
    assert_eq!(
        born.wood, sc.alive_min,
        "born with {} of wood, not alive_min",
        born.wood
    );
    // Its mineral is the consumed bins' own, at the bank's density.
    let density = bank_mineral / bank;
    assert!(
        (born.mineral - density * package).abs() <= 1e-9 * born.mineral,
        "born with {} of mineral for {package} at the bank's density {density}",
        born.mineral
    );
    // The predecessor's remains are under it, and the bank of its own species is spent.
    let g = flora.view().ground_at(victim).expect("ground");
    assert!(
        g.dead_wood > 0.0 && g.litter > 0.0,
        "the drowned stand left no remains"
    );
    // The bank that germinated was spent, and round 3b leaves it spent: a donor saves for
    // one recipient at a time now, so unless this was the tick its parcel filled there is
    // no fresh package behind the one that germinated. Round 3's rule landed something on
    // every recipient every tick, so this site could never be seen with an empty bank
    // while a donor was in reach of it — a property of paying every neighbour at once,
    // not of the seed bank.
    let left = g.seed_organic(Species::Umbrellafrond);
    assert!(
        (left - (bank - package)).abs() <= 1e-3 * package,
        "the bank should be one package lighter: {left} of {bank} less {package}"
    );
    // One bin whatever is there: the placeholders' bin is 150 s wide, so every tick of this
    // fixture is inside the bin that opened at tick 0.
    for c in &g.seeds {
        assert_eq!(c.bin_start_tick, 0, "not this run's own bin: {:?}", g.seeds);
    }
    assert_residuals(&flora, "after a drowning was replaced in one tick");
}

/// Two species' banks on one site, both holding whole packages, both passing their own
/// predicate: the gap goes to a **local lottery** weighted by the whole packages each bank
/// holds, the winner spends exactly one of them, and every other bank stays where it is.
///
/// Astra's R4.5: the old rule gave the gap to the first qualifying species in
/// `Species::ALL`, so bloomcrown pre-empted umbrellafrond in every contested gap in the
/// world whatever the two banks held, and three more species after it would have inherited
/// that precedence. This fixture is the same contest, and the winner here is
/// **umbrellafrond** on a draw where the two banks are within 2e-5 of each other — which
/// under the old rule was impossible by construction.
///
/// The contest is reachable now that a donor saves for one recipient at a time: a stand
/// **holds** the contested site while both banks build under it (a bank waits for a gap),
/// and then the site is cleared and the next tick decides. Its occupant is a bloomcrown at
/// wood 0.25, under `donor_min` 0.3, so it holds the site without being a third donor.
///
/// Four rates are the test's own: both species' `propagule_rate` 3.0 /s (placeholder 2e-4)
/// and `reserve_cap` 40 (placeholder 0.5), so each donor funds a package every tick for
/// the whole run and both banks hold seventeen of them; and `bloomcrown.hop` 1
/// (placeholder 2) so both donors reach the contested column and no other.
#[test]
fn a_contested_gap_is_drawn_by_weight_and_the_losing_bank_stays() {
    let mut config = FloraConfig::default().drop_seeds_checked_each_tick();
    config.bloomcrown.propagule_rate = 3.0;
    config.bloomcrown.hop = 1;
    config.bloomcrown.reserve_cap = 40.0;
    config.umbrellafrond.propagule_rate = 3.0;
    config.umbrellafrond.reserve_cap = 40.0;
    let mut world = strip(5, 1, wet());
    let mut flora = Flora::new(config);
    let contested = site(2);
    // The two donors, either side of the contested column, and the placeholder occupant
    // that holds it while their banks build.
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 1,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.6
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 3,
            z: 0,
            species: Species::Umbrellafrond,
            wood: 0.6
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.25
        }
    ));

    run(&mut flora, &mut world, 40);

    let g = flora.view().ground_at(contested).expect("ground").clone();
    let (b, u) = (
        g.seed_organic(Species::Bloomcrown),
        g.seed_organic(Species::Umbrellafrond),
    );
    let package = flora.config().species(Species::Bloomcrown).alive_min
        / flora.config().species(Species::Bloomcrown).propagule_split[0];
    for (species, bank) in [(Species::Bloomcrown, b), (Species::Umbrellafrond, u)] {
        assert!(
            bank >= 2.0 * package,
            "{}'s bank holds {bank}, under the two packages this test wants",
            species.name()
        );
    }
    assert_eq!(
        flora.view().stand_at(contested).map(|s| s.species),
        Some(Species::Bloomcrown),
        "the occupant should still be holding the site"
    );
    // The two donors' *other* neighbours, x0 and x4, are bare and have germinated already
    // — that is the rule working — so the count to watch is the change over the one tick
    // that decides the contested site.
    let est0 = flora.view().ledger.establishments;

    // Open the gap and let the next tick decide it. Both donors are cleared with the
    // occupant, so no package lands in the deciding tick and the banks can be read against
    // what germination spent rather than against what arrived behind it.
    for x in [1i64, 2, 3] {
        assert!(
            flora.apply(&world, Command::Clear { x, z: 0 }),
            "clearing x{x}"
        );
    }
    flora.step(&mut world);

    let born = *flora
        .view()
        .stand_at(contested)
        .expect("nothing germinated");
    assert_eq!(
        born.species,
        Species::Umbrellafrond,
        "this draw's winner, on banks of {u} against {b}: the enum order is not the tiebreak"
    );
    assert_eq!(
        flora.view().ledger.establishments,
        est0 + 1,
        "one gap, one germination: the contested site"
    );
    // Exactly one package became the stand, at `alive_min` of wood on the nose.
    let sc = flora.config().species(born.species).clone();
    assert!(
        (born.organic() - package).abs() <= 1e-12 * package,
        "born with {} for a {package} package",
        born.organic()
    );
    assert!(
        (born.wood - sc.alive_min).abs() <= 1e-15,
        "born with {} of wood, not alive_min {}",
        born.wood,
        sc.alive_min
    );
    assert!(
        (born.foliage - sc.propagule_split[1] * package).abs() <= 1e-12 * package,
        "{born:?}"
    );
    assert!(
        (born.reserve - sc.propagule_split[2] * package).abs() <= 1e-12 * package,
        "{born:?}"
    );

    // The winner's own remainder is still banked and still ageing, and the loser's bank is
    // untouched to the attrition of the one tick.
    let g = flora.view().ground_at(contested).expect("ground").clone();
    let left_u = g.seed_organic(Species::Umbrellafrond);
    let left_b = g.seed_organic(Species::Bloomcrown);
    assert!(
        (left_u - (u - package)).abs() <= 1e-3 * package,
        "the winner's remainder is {left_u} of {u} less one {package} package"
    );
    assert!(
        (left_b - b).abs() <= 1e-3 * package,
        "the loser's bank moved: {left_b} against {b}"
    );
    assert!(
        left_b > 0.0 && left_u > 0.0,
        "a bank was emptied: {:?}",
        g.seeds
    );

    // And the loser's bank stays while the winner stands, however long — until its own
    // bins age out, which at the placeholders is 600 s away.
    run(&mut flora, &mut world, 40);
    assert_eq!(
        flora.view().stand_at(contested).map(|s| s.species),
        Some(Species::Umbrellafrond),
        "the winner did not hold the site"
    );
    assert!(
        flora
            .view()
            .ground_at(contested)
            .unwrap()
            .seed_organic(Species::Bloomcrown)
            > 0.0,
        "the loser's bank vanished instead of waiting"
    );
    assert_residuals(&flora, "after a contested gap was drawn by weight");
}

/// An **oversized** bank builds one stand of exactly one package and keeps the rest, and
/// the packages it spends come out of its **oldest** bins first.
///
/// Astra's R4.5, second half: germination used to spend the whole bank, so
/// `wood = w_frac · pooled` had no `wood_max` bound at all and a bank waiting under a
/// living stand could produce an oversized "small" preset that growth's later demand cap
/// does not shrink. Here the bank holds twenty packages and the stand that comes out of it
/// is the same size as one that comes out of two.
///
/// The fixture is the contested one with a single species: a donor funding a package a
/// tick (`propagule_rate` 3.0 /s against 2e-4, `reserve_cap` 40 against 0.5, `hop` 1
/// against 2) and a sub-`donor_min` occupant holding the site while the bank grows.
/// `seed_max_age_s` is 2 s (placeholder 600) so the bin width is ten ticks and the run
/// spans four bins, which is what makes "oldest first" observable.
#[test]
fn an_oversized_bank_spends_one_package_out_of_its_oldest_bins() {
    let mut config = FloraConfig::default().drop_seeds_checked_each_tick();
    config.bloomcrown.propagule_rate = 3.0;
    config.bloomcrown.hop = 1;
    config.bloomcrown.reserve_cap = 40.0;
    config.bloomcrown.seed_max_age_s = 2.0;
    let mut world = strip(3, 1, 0.6);
    let mut flora = Flora::new(config);
    let held = site(1);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 0,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.6
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 1,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.25
        }
    ));
    let sc = flora.config().species(Species::Bloomcrown).clone();
    let package = sc.alive_min / sc.propagule_split[0];

    run(&mut flora, &mut world, 35);

    let g = flora.view().ground_at(held).expect("ground").clone();
    let bank = g.seed_organic(Species::Bloomcrown);
    assert!(bank > 5.0 * package, "the bank is not oversized: {bank}");
    assert!(
        g.seeds.len() >= 3,
        "fewer bins than the run spans: {:?}",
        g.seeds
    );
    let oldest = g.seeds[0];
    let bins_before = g.seeds.len();

    // Open the gap: one package, out of the oldest bin. The donor goes with the occupant
    // so that nothing lands behind the germination and the bank can be read exactly.
    for x in [0i64, 1] {
        assert!(
            flora.apply(&world, Command::Clear { x, z: 0 }),
            "clearing x{x}"
        );
    }
    flora.step(&mut world);

    let born = *flora.view().stand_at(held).expect("nothing germinated");
    assert!(
        (born.organic() - package).abs() <= 1e-12 * package,
        "an oversized bank built a {} stand out of a {bank} bank",
        born.organic()
    );
    assert!(
        (born.wood - sc.alive_min).abs() <= 1e-15,
        "born with {} of wood, not alive_min {}",
        born.wood,
        sc.alive_min
    );
    assert!(
        born.wood <= sc.wood_max,
        "a newborn over wood_max: {}",
        born.wood
    );
    let g = flora.view().ground_at(held).expect("ground").clone();
    assert!(
        (g.seed_organic(Species::Bloomcrown) - (bank - package)).abs() <= 1e-3 * package,
        "the surplus was spent or lost: {} of {bank} less {package}",
        g.seed_organic(Species::Bloomcrown)
    );
    // Oldest first, in whole seeds (package S): the seed came out of the bin that held
    // the run's first packages — one seed fewer there, or the bin gone whole if it held
    // just one — and every bin left holds whole seeds, so no float dust is left behind.
    let n_oldest = (oldest.organic / package).round();
    assert!(n_oldest >= 1.0, "the oldest bin holds no whole seed: {oldest:?}");
    if n_oldest == 1.0 {
        assert_eq!(
            g.seeds.len(),
            bins_before - 1,
            "a one-seed bin should have gone whole: {:?}",
            g.seeds
        );
        assert!(
            g.seeds[0].bin_start_tick > oldest.bin_start_tick,
            "the oldest bin is still the front of the bank: {:?}",
            g.seeds
        );
    } else {
        assert_eq!(g.seeds[0].bin_start_tick, oldest.bin_start_tick);
        assert!(
            (g.seeds[0].organic / package - (n_oldest - 1.0)).abs() < 1e-9,
            "the oldest bin did not give up exactly one seed: {:?}",
            g.seeds[0]
        );
    }
    assert!(
        g.seeds.iter().all(|c| {
            let n = c.organic / package;
            (n - n.round()).abs() < 1e-9 && n.round() >= 1.0
        }),
        "a bin that is not whole seeds: {:?}",
        g.seeds
    );
    assert_residuals(&flora, "after an oversized bank spent one package");
}

/// The bin rule, both halves of it, and the **rejuvenation** the old merge allowed is the
/// thing it now refuses (Astra R4.1). A site a donor feeds every tick holds one cohort per
/// bin, not one per tick — so the bank is still bounded — and that bin's age rises by one
/// every tick from the tick its window opened, whether or not anything else lands in it.
/// The old rule merged age 1 with age 0 and kept 0, so this same fixture read age 0 for
/// ever.
///
/// The bin width is `seed_max_age_s / seed_cohorts_max`; this fixture sets
/// `seed_max_age_s` to 2 s over the placeholder cap of 4, which is a 0.5 s bin, ten ticks,
/// so a 30-tick run crosses three bin boundaries and the test can name each one. Two more
/// rates are the test's own so that a package lands on **every** tick whatever the
/// dispersal rule is: `propagule_rate` 2.0 /s (placeholder 2e-4) is over one whole
/// package per tick, and `reserve_cap` 40 (placeholder 0.5) gives the donor the reserve to
/// pay for a hundred of them. The fixture's void column leaves exactly one recipient.
#[test]
fn a_fed_bank_holds_one_cohort_per_arrival_bin_and_its_age_never_stops_rising() {
    let mut config = FloraConfig::default().drop_seeds_checked_each_tick();
    config.bloomcrown.hop = 1;
    config.bloomcrown.seed_max_age_s = 2.0;
    config.bloomcrown.propagule_rate = 2.0;
    config.bloomcrown.reserve_cap = 40.0;
    assert_eq!(
        config.bloomcrown.seed_cohorts_max, 4,
        "the placeholder this test divides by"
    );
    let bin = 10u64; // 2.0 s / 4 bins = 0.5 s = ten ticks at DT = 1/20 s.
    let mut world = strip_gap(0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 0,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.6
        }
    ));

    let fed = site(1);
    for tick in 1..=30u64 {
        flora.step(&mut world);
        let g = flora.view().ground_at(fed).expect("ground");
        // One bin per window crossed: ticks 0..9 are the first, 10..19 the second, and so
        // on, and the newest bin is the last of the run because they are sorted oldest
        // first.
        let want_bins = (tick / bin + 1) as usize;
        assert_eq!(
            g.seeds.len(),
            want_bins,
            "tick {tick}: {} cohorts, not {want_bins}: {:?}",
            g.seeds.len(),
            g.seeds
        );
        for (i, c) in g.seeds.iter().enumerate() {
            assert_eq!(
                c.bin_start_tick,
                i as u64 * bin,
                "bin {i} at tick {tick}: {c:?}"
            );
        }
        // The oldest bin is as old as the run, never rejuvenated by the fresh landings
        // that keep arriving on top of it.
        assert_eq!(
            g.seeds[0].age_ticks(flora.tick()),
            tick,
            "the oldest bin was rejuvenated"
        );
    }
    let organic_fed: f64 = flora
        .view()
        .ground_at(fed)
        .unwrap()
        .seed_organic(Species::Bloomcrown);
    let oldest = flora.view().ground_at(fed).unwrap().seeds[0];

    // Take the donor away. Nothing new lands, every bin keeps ageing, and the oldest one
    // reaches `seed_max_age_s` = 40 ticks first — on its own schedule, which is what the
    // old rule could postpone for ever.
    assert!(flora.apply(&world, Command::Clear { x: 0, z: 0 }));
    run(&mut flora, &mut world, 10);
    let g = flora.view().ground_at(fed).unwrap().clone();
    assert_eq!(
        g.seeds.len(),
        4,
        "a bin arrived or left early: {:?}",
        g.seeds
    );
    assert_eq!(
        g.seeds[0].bin_start_tick, 0,
        "the oldest bin is still the first one"
    );
    assert_eq!(
        g.seeds[0].age_ticks(flora.tick()),
        40,
        "it did not age: {:?}",
        g.seeds
    );

    // One more tick and the first bin is 41 ticks old, past 2 s, and it goes to litter
    // whole with its mineral.
    let litter0 = g.litter;
    flora.step(&mut world);
    let g = flora.view().ground_at(fed).unwrap().clone();
    assert_eq!(
        g.seeds.len(),
        3,
        "the over-age bin is still banked: {:?}",
        g.seeds
    );
    assert_eq!(
        g.seeds[0].bin_start_tick, bin,
        "the wrong bin left: {:?}",
        g.seeds
    );
    assert!(
        g.litter > litter0 + 0.9 * oldest.organic,
        "the bin that left did not reach the litter: {} against {}",
        g.litter - litter0,
        oldest.organic
    );
    assert!(
        g.seed_organic(Species::Bloomcrown) < organic_fed,
        "attrition and expiry took nothing: {} -> {}",
        organic_fed,
        g.seed_organic(Species::Bloomcrown)
    );
    assert_residuals(&flora, "after a bank was fed, abandoned and aged out");
}

/// **Astra's R4.1 fixture.** A two-tick lifetime, germination that cannot happen, and tiny
/// arrivals that never stop: the old material has to reach the litter on schedule anyway,
/// with its organic matter, its mineral and its energy all booked.
///
/// `seed_max_age_s` is 0.1 s — two ticks — against the placeholder 600, so the bin width
/// is 0.025 s, under one tick, and every tick is its own bin: the sharpest possible test of
/// whether a fresh landing can hold old material back. `establish_light_min` is 2.0
/// (placeholder 0.6), a predicate nothing can pass, so nothing germinates and the bank can
/// only accumulate or leave. Both decomposition rates are 0 (placeholders 0.001 and
/// 0.0001) so what reaches the litter stays there and can be read off. `propagule_rate`
/// 2.0 /s and `reserve_cap` 40 (placeholders 2e-4 and 0.5) make the arrivals continuous —
/// one package a tick, for far longer than the run — and the fixture's void column leaves
/// exactly one recipient.
#[test]
fn tiny_continuing_arrivals_cannot_keep_old_seed_material_alive() {
    let mut config = FloraConfig::default().drop_seeds_checked_each_tick();
    config.bloomcrown.hop = 1;
    config.bloomcrown.seed_max_age_s = 0.1;
    config.bloomcrown.establish_light_min = 2.0;
    config.bloomcrown.propagule_rate = 2.0;
    config.bloomcrown.reserve_cap = 40.0;
    config.decomposition = 0.0;
    config.wood_decomposition = 0.0;
    let e_v = config.bloomcrown.energy_density;
    let attrition = config.bloomcrown.seed_attrition_per_s;
    let mut world = strip_gap(0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 0,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.6
        }
    ));

    let fed = site(1);
    // Long enough that the first arrivals are many lifetimes old.
    run(&mut flora, &mut world, 60);

    let g = flora.view().ground_at(fed).expect("ground").clone();
    let now = flora.tick();
    assert!(
        !g.seeds.is_empty(),
        "nothing is landing at all: the fixture is broken"
    );
    // Nothing older than the lifetime is still banked, however much keeps arriving.
    for c in &g.seeds {
        assert!(
            c.age_s(now) <= 0.1,
            "a {}-tick-old bin survived a two-tick lifetime under continuing arrivals: {c:?}",
            c.age_ticks(now)
        );
    }
    assert!(
        g.seeds.len() <= 4,
        "more bins than the lifetime allows: {:?}",
        g.seeds
    );
    // And the old material is in the litter, in all three currencies: organic matter, the
    // mineral that came with it, and its energy at the species' own density. With both
    // decomposition rates at zero this is everything that has ever aged out plus the
    // attrition on the way.
    assert!(
        g.litter > 0.0,
        "the expired bins were deleted rather than booked"
    );
    assert!(g.litter_mineral > 0.0, "the expired bins' mineral vanished");
    assert!(
        (g.litter_energy - e_v * g.litter).abs() <= 1e-12 * g.litter_energy,
        "litter energy {} against e_v * litter {}",
        g.litter_energy,
        e_v * g.litter
    );
    // What left the bank is mostly whole bins and not attrition: two ticks of attrition
    // is 1e-4 of a bin, so the litter is ~50 bins' worth and not 1 %.
    let banked = g.seed_organic(Species::Bloomcrown);
    assert!(
        g.litter > 4.0 * banked,
        "the litter {} is not the aged-out bins: {banked} is still banked, attrition is \
         {attrition}/s",
        g.litter
    );
    assert_eq!(
        flora.view().ledger.establishments,
        0,
        "the predicate cannot pass here"
    );
    assert_residuals(
        &flora,
        "after sixty ticks of tiny arrivals onto a two-tick bank",
    );
}

/// The adversarial fixture the old merge rule's own comment invited, under round 3b's
/// rule: a donor that comes and goes on a period of more than two ticks. Package I
/// measured what the age merge bounded — nothing, one cohort per pulse, about 6,000 per
/// site — and package J bounded the *count* by merging the two oldest, which Astra's R4.1
/// then showed bounds the `Vec` while sweeping almost every old deposit into one bucket
/// that can kill much younger material at the next expiry. Arrival bins bound the count
/// **and** the age: a landing joins the bin covering its own tick and moves no other bin's
/// age, so a species' live bins are at most `seed_cohorts_max + 1` and the first pulse's
/// material leaves on the first pulse's schedule.
///
/// Three arms of the same fifty pulses, at `seed_cohorts_max` 1, 4 (the placeholder) and 8
/// — which is a bin width of 40, 10 and 5 ticks against the same 2 s lifetime — so the
/// claim is read at three age resolutions:
///   - the bank holds at most `cap + 1` bins, sorted oldest first;
///   - no bin older than the lifetime is in it;
///   - **the first pulse's bin is gone in every arm**, which is the whole of R4.1: under
///     the old rule fifty pulses of fresh material kept it at age 0 for ever;
///   - nothing was deleted on the way (the three residuals, and the litter holds what
///     left).
///
/// `bloomcrown.establish_light_min` is 2.0 (placeholder 0.6), a predicate that can never
/// pass, so the bank can only accumulate or age out and germination cannot end the
/// experiment early. `seed_max_age_s` is 2 s (placeholder 600) so the lifetime fits in a
/// hundred-tick test. `propagule_rate` 2.0 /s and `reserve_cap` 40 (placeholders 2e-4 and
/// 0.5) make one pulse fund a whole package under any dispersal rule. The pulsing itself
/// is `Seed` and `Clear`, which is what "a donor that flickers across its reserve floor"
/// looks like from the recipient site.
#[test]
fn a_pulsing_donor_cannot_rejuvenate_a_bank_and_the_bins_bound_it() {
    let pulses = 50u64;
    for cap in [1usize, 4, 8] {
        let mut config = FloraConfig::default().drop_seeds_checked_each_tick();
        config.bloomcrown.hop = 1;
        config.bloomcrown.establish_light_min = 2.0;
        config.bloomcrown.seed_max_age_s = 2.0;
        config.bloomcrown.propagule_rate = 2.0;
        config.bloomcrown.reserve_cap = 40.0;
        assert_eq!(
            config.bloomcrown.seed_cohorts_max, 4,
            "the placeholder one arm reads"
        );
        config.bloomcrown.seed_cohorts_max = cap;
        let bin = (2.0 / cap as f64 / DT) as u64;
        let mut world = strip_gap(0.6);
        let mut flora = Flora::new(config);

        let target = site(1);
        for _ in 0..pulses {
            assert!(flora.apply(
                &world,
                Command::Seed {
                    x: 0,
                    z: 0,
                    species: Species::Bloomcrown,
                    wood: 0.6
                }
            ));
            flora.step(&mut world);
            assert!(flora.apply(&world, Command::Clear { x: 0, z: 0 }));
            flora.step(&mut world);
        }
        let now = flora.tick();
        assert_eq!(now, 2 * pulses, "the pulse loop is two ticks a pulse");

        let g = flora.view().ground_at(target).expect("ground").clone();
        assert!(!g.seeds.is_empty(), "cap {cap}: nothing landed at all");
        assert!(
            g.seeds.len() <= cap + 1,
            "cap {cap}: {} bins, more than cap + 1: {:?}",
            g.seeds.len(),
            g.seeds
        );
        assert!(g.seeds.iter().all(|c| c.species == Species::Bloomcrown));
        for w in g.seeds.windows(2) {
            assert!(
                w[0].bin_start_tick < w[1].bin_start_tick,
                "cap {cap}: not sorted oldest first: {:?}",
                g.seeds
            );
        }
        for c in &g.seeds {
            assert!(
                c.age_s(now) <= 2.0,
                "cap {cap}: a {}-tick-old bin outlived a 2 s lifetime: {c:?}",
                c.age_ticks(now)
            );
            assert_eq!(
                c.bin_start_tick % bin,
                0,
                "cap {cap}: {c:?} is not on a bin boundary"
            );
        }
        // R4.1 itself: the first pulse's material is not in the bank at any resolution.
        assert!(
            g.seeds[0].bin_start_tick > 0,
            "cap {cap}: the first pulse's bin is still banked after {pulses} pulses of fresh \
             arrivals: {:?}",
            g.seeds
        );
        // It is in the litter instead, with its mineral, and nothing was deleted.
        assert!(g.litter > 0.0 && g.litter_mineral > 0.0, "cap {cap}: {g:?}");
        assert!(
            flora.view().stand_at(target).is_none(),
            "the predicate cannot pass here"
        );
        assert_eq!(flora.view().ledger.establishments, 0);
        assert_residuals(&flora, "after fifty pulses onto a binned bank");
    }
}

/// **Astra's R5.4, at model level.** A descendant that is born and dies inside an
/// observation window leaves no trace in the final state: the site is empty, the species
/// has no living stand, and a harness that counts only survivors reports no recruitment
/// over a real birth. The counters see it — one establishment, one death, one identity that
/// was alive and is not — and that is what the probe now reads per tick.
///
/// The fixture is one donor and one recipient behind a void column, so the only landing,
/// the only birth and the only death in the run belong to this one lineage.
/// `propagule_rate` 3.0 /s (placeholder 2e-4) funds the package from the donor's own
/// starting reserve in one tick; `assimilation` 0 and `maintenance` 0.4 /s (placeholders
/// 0.004 and 0.0002) mean the newborn — born at exactly `alive_min` with `q_frac` of a
/// package in reserve — cannot pay its own bill and diebacks under the threshold within a
/// few ticks. The donor, twenty times its wood, outlives the window.
#[test]
fn a_descendant_born_and_dead_inside_the_window_is_still_a_birth() {
    let mut config = FloraConfig::default().drop_seeds_checked_each_tick();
    config.bloomcrown.propagule_rate = 3.0;
    config.bloomcrown.hop = 1;
    config.bloomcrown.assimilation = 0.0;
    config.bloomcrown.maintenance = 0.4;
    // 0.75 (placeholder 0.5): the founder's starting reserve funds exactly **one** package.
    // A second one would be a whole seed that waits in the bank (package S) and recruits
    // into the gap the first descendant leaves, which is a second lineage in the window.
    config.bloomcrown.donor_reserve_floor = 0.75;
    let sc = config.bloomcrown.clone();
    let mut world = strip_gap(0.6);
    let mut flora = Flora::new(config);
    let recipient = site(1);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 0,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.6
        }
    ));
    let founder_id = flora.view().stand_at(site(0)).expect("planted").id;

    // Watch the way the probe watches: every identity of this species seen alive at the end
    // of any tick, and the survivors at the end.
    let mut seen: Vec<u64> = Vec::new();
    for _ in 0..60 {
        flora.step(&mut world);
        for s in flora
            .view()
            .stands
            .iter()
            .filter(|s| s.species == Species::Bloomcrown)
        {
            if s.id != founder_id && !seen.contains(&s.id) {
                seen.push(s.id);
            }
        }
    }

    // One birth happened, and nothing of it is standing at the end.
    assert_eq!(
        seen.len(),
        1,
        "identities seen alive besides the founder: {seen:?}"
    );
    assert_eq!(flora.view().ledger.establishments, 1, "one germination");
    assert_eq!(flora.view().ledger.deaths, 1, "the newborn did not die");
    assert_eq!(
        flora.view().ledger.births,
        2,
        "the founder and the one descendant"
    );
    assert!(
        flora.view().stand_at(recipient).is_none(),
        "the descendant is still standing"
    );
    let survivors = flora
        .view()
        .stands
        .iter()
        .filter(|s| s.species == Species::Bloomcrown && s.id != founder_id)
        .count();
    assert_eq!(survivors, 0, "a survivor makes this the wrong fixture");
    // The founder is still there, so the death was the descendant's.
    assert!(
        flora.view().stands.iter().any(|s| s.id == founder_id),
        "the founder died: the death may not be the descendant's"
    );
    // So the final state alone says "no recruitment" and the per-tick identity watch says
    // one birth and no survivor. Both readings are printed by the probe.
    assert!(
        seen.len() > survivors,
        "the birth is invisible in the final state, and counted"
    );
    // Its remains are on the site it was born on, and nothing was lost on the way.
    let g = flora
        .view()
        .ground_at(recipient)
        .expect("the grave")
        .clone();
    assert!(
        g.dead_wood > 0.0 && g.litter > 0.0,
        "the descendant left no remains: {g:?}"
    );
    assert!(
        g.dead_wood >= sc.alive_min * 0.5,
        "it was born at alive_min: {g:?}"
    );
    assert_residuals(
        &flora,
        "after a descendant was born and died inside the window",
    );
}

/// **Astra's R5.5: the expiry boundary, both sides of it.** K7 germinates before charging
/// attrition and expiry, so a bin on the first tick past its `seed_max_age_s` gets one last
/// chance to recruit before it goes to litter. This pins that tick and the one after it:
/// a gate that opens exactly on the removal tick recruits out of the bin, and an identical
/// twin whose gate is still shut loses the whole bin to litter on that same tick and can
/// never recruit afterwards, however wide its gate opens.
///
/// The gate is the **saturation ceiling**, driven by the world and not by a config change
/// mid-run: both target sites start with wholly saturated root boxes, which is a saturated
/// fraction of 1 against bloomcrown's `establish_saturated_max` of 0.25, so the predicate
/// refuses them; draining one root box through the core's own bounded withdrawal opens its
/// gate on the tick of the operator's choosing.
///
/// Four values are the test's own. `seed_max_age_s` is 0.1 s — **two ticks** — against the
/// placeholder 600, and `seed_cohorts_max` is 1 against 4, which makes the bin two ticks
/// wide so that the package landing on tick 1 joins the bin that opened at tick **0** and
/// is therefore removed on tick 3 (age 3 ticks = 0.15 s > 0.1 s). `seed_attrition_per_s`
/// and `decomposition` are 0 (placeholders 0.001 and 0.001) so that what reaches the
/// litter is exactly the bin and stays readable. `propagule_rate` is 2.0 /s (placeholder
/// 2e-4) so one tick of each donor's own reserve funds one whole package, and the donors
/// are cleared straight afterwards so that each target holds exactly one package in
/// exactly one bin.
#[test]
fn an_expiring_bin_gets_one_last_germination_and_then_goes_to_litter() {
    let mut config = FloraConfig::default().drop_seeds_checked_each_tick();
    config.bloomcrown.seed_max_age_s = 0.1;
    config.bloomcrown.seed_cohorts_max = 1;
    config.bloomcrown.seed_attrition_per_s = 0.0;
    config.bloomcrown.propagule_rate = 2.0;
    config.bloomcrown.hop = 1;
    config.decomposition = 0.0;
    config.wood_decomposition = 0.0;
    let sc = config.bloomcrown.clone();
    let package = sc.alive_min / sc.propagule_split[0];
    let e_v = sc.energy_density;

    // Eight columns: two donors at x0 and x4, their only recipients at x1 and x5, and void
    // columns at x3 and x7 so that each donor has exactly one support face in reach. The
    // root boxes of x1 (x0..x2) and x5 (x4..x6) are three columns apart, so draining one
    // leaves the other saturated.
    let vconfig = VoxelConfig {
        width: 8,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 5,
        ..VoxelConfig::default()
    };
    let mut world = World::empty(vconfig);
    for x in [0i64, 1, 2, 4, 5, 6] {
        for y in 1..=2u32 {
            fill(&mut world, x, y, 0, Material::Soil, 0.98);
        }
    }
    for x in [3i64, 7] {
        world.apply(WorldCommand::SetMaterial {
            x,
            y: 0,
            z: 0,
            material: Material::Air,
        });
        assert!(
            cubarium_voxel_flora::highest_support(&world.view(), x, 0).is_none(),
            "x{x} void"
        );
    }
    let (opens, blocked) = (site(1), site(5));
    let mut flora = Flora::new(config);
    for x in [0i64, 4] {
        assert!(flora.apply(
            &world,
            Command::Seed {
                x,
                z: 0,
                species: Species::Bloomcrown,
                wood: 0.6
            }
        ));
    }

    // Tick 1: one package to each target, in the bin that opened at tick 0.
    flora.step(&mut world);
    for at in [opens, blocked] {
        let g = flora
            .view()
            .ground_at(at)
            .unwrap_or_else(|| panic!("nothing landed at {at:?}"));
        assert_eq!(g.seeds.len(), 1, "{at:?}: {:?}", g.seeds);
        assert_eq!(
            g.seeds[0].bin_start_tick, 0,
            "{at:?}: not the bin tick 0 opened"
        );
        assert_eq!(g.seeds[0].organic, package, "{at:?}: {:?}", g.seeds);
        assert!(g.seeds[0].mineral > 0.0, "{at:?} carries no mineral");
    }
    let banked_mineral = flora.view().ground_at(blocked).unwrap().seeds[0].mineral;
    for x in [0i64, 4] {
        assert!(
            flora.apply(&world, Command::Clear { x, z: 0 }),
            "clearing the donor at x{x}"
        );
    }

    // Tick 2: the saturation ceiling refuses both, and the bin is exactly at its lifetime
    // (age 2 ticks = 0.1 s, which is not *past* 0.1 s), so nothing is removed either.
    flora.step(&mut world);
    assert_eq!(
        flora.view().ledger.establishments,
        0,
        "something germinated on saturated soil"
    );
    for at in [opens, blocked] {
        assert_eq!(
            flora.view().ground_at(at).unwrap().seeds.len(),
            1,
            "{at:?}: the bin left at its lifetime rather than past it"
        );
        assert_eq!(
            flora.view().ground_at(at).unwrap().seeds[0].age_ticks(flora.tick()),
            2
        );
    }

    // Open one gate, between the ticks: drain x1's whole root box under the species'
    // `saturated_pore` of 0.95. The twin at x5 stays saturated.
    for x in 0..3i64 {
        for y in 1..=2u32 {
            drain_to(&mut world, x, y, 0, 0.5);
        }
    }

    // Tick 3: the bin is one tick past its lifetime, and the lottery runs first. The drained
    // site recruits out of it; the blocked twin loses the whole bin to litter.
    flora.step(&mut world);
    assert_eq!(
        flora.view().ledger.establishments,
        1,
        "the expiring bin's last chance was not taken"
    );
    let born = *flora
        .view()
        .stand_at(opens)
        .expect("the drained site did not recruit");
    assert_eq!(born.species, Species::Bloomcrown);
    assert_eq!(born.wood, sc.alive_min, "born with {} of wood", born.wood);
    assert!(
        (born.organic() - package).abs() <= 1e-16,
        "born with {} for a whole {package} package",
        born.organic()
    );
    assert!(
        flora.view().ground_at(opens).unwrap().seeds.is_empty(),
        "the spent bin is still there: {:?}",
        flora.view().ground_at(opens).unwrap().seeds
    );
    // The twin: nothing born, the bin gone, and every unit of it in the litter with its
    // mineral and its energy.
    assert!(
        flora.view().stand_at(blocked).is_none(),
        "the blocked twin recruited"
    );
    let g = flora.view().ground_at(blocked).expect("ground").clone();
    assert!(
        g.seeds.is_empty(),
        "the expired bin is still banked: {:?}",
        g.seeds
    );
    assert!(
        (g.litter - package).abs() <= 1e-16,
        "{} of a {package} bin reached the litter",
        g.litter
    );
    assert!(
        (g.litter_mineral - banked_mineral).abs() <= 1e-18,
        "{} of the bin's {banked_mineral} of mineral reached the litter",
        g.litter_mineral
    );
    assert!(
        (g.litter_energy - e_v * g.litter).abs() <= 1e-15,
        "litter energy {} against e_v times litter {}",
        g.litter_energy,
        e_v * g.litter
    );

    // Open the twin's gate too, one tick too late, and step again: there is nothing left to
    // recruit out of. The material is in the litter, not in a bank.
    for x in 4..7i64 {
        for y in 1..=2u32 {
            drain_to(&mut world, x, y, 0, 0.5);
        }
    }
    flora.step(&mut world);
    assert_eq!(
        flora.view().ledger.establishments,
        1,
        "the twin recruited a tick after its bin had gone to litter"
    );
    assert!(
        flora.view().stand_at(blocked).is_none(),
        "the twin recruited out of nothing"
    );
    assert!(
        (flora.view().ground_at(blocked).unwrap().litter - package).abs() <= 1e-16,
        "the litter moved after the expiry"
    );
    assert_residuals(&flora, "after an expiring bin's last chance");
}

// ================================================================ lineage by id

/// A founder dies and **its own species** germinates on its site in the same tick, and the
/// books see one death, one birth and a new identity. The site is never once observed
/// empty.
///
/// Astra's R4.7: the harness told founders from descendants by watching each founder's
/// site every tick and calling the founder gone once the site was seen without a stand of
/// its species on it. Steps 6 and 8 of the tick permit exactly this — death in the
/// per-stand pass, germination after it — so that watch could count a descendant as the
/// original founder for ever. `Stand::id`, from the ledger's birth counter, cannot miss
/// it.
///
/// The fixture: `assimilation` 0 (placeholder 0.004) so nothing earns and the victim
/// starves on a site that is still wet enough to germinate on; `maintenance` 0.4 /s
/// (placeholder 0.0002) so it starves inside thirty ticks; the victim is planted at
/// `alive_min` exactly, so the first unpaid tick's dieback kills it; and `propagule_rate`
/// 3.0 /s (placeholder 2e-4) lets the donor's whole spendable reserve — the placeholder
/// `reserve_cap`, untouched — buy the **two** packages the victim's site needs, in the
/// first two ticks. The fixture's void column leaves the donor exactly one recipient, so
/// the victim's site is the only one anything lands on and the only birth in the run is
/// its replacement. The donor starves too, but forty ticks after the tick this test
/// reads.
#[test]
fn a_founder_replaced_by_its_own_species_in_one_tick_is_still_a_death_and_a_birth() {
    let mut config = FloraConfig::default().drop_seeds_checked_each_tick();
    config.bloomcrown.assimilation = 0.0;
    config.bloomcrown.maintenance = 0.4;
    config.bloomcrown.propagule_rate = 3.0;
    config.bloomcrown.hop = 1;
    let mut world = strip_gap(0.6);
    let mut flora = Flora::new(config);
    let victim_site = site(1);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 0,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.6
        }
    ));
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 1,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.02
        }
    ));
    let victim_id = flora.view().stand_at(victim_site).expect("planted").id;
    let donor_id = flora.view().stand_at(site(0)).expect("planted").id;
    assert_ne!(victim_id, donor_id, "two founders, two identities");
    assert_eq!(
        flora.view().ledger.births,
        2,
        "two stands have been created"
    );

    // Step until the death, and check every tick that the site is never seen empty — the
    // observation the old site watch depended on, and which never happens here.
    let mut died_on = None;
    for tick in 1..=200u32 {
        let (deaths0, est0) = (
            flora.view().ledger.deaths,
            flora.view().ledger.establishments,
        );
        flora.step(&mut world);
        assert!(
            flora.view().stand_at(victim_site).is_some(),
            "tick {tick}: the site was seen empty, so this is not the same-tick case"
        );
        if flora.view().ledger.deaths > deaths0 {
            // The whole point, in one tick: one death and one birth, together.
            assert_eq!(
                flora.view().ledger.deaths,
                deaths0 + 1,
                "tick {tick}: one death"
            );
            assert_eq!(
                flora.view().ledger.establishments,
                est0 + 1,
                "tick {tick}: the death's own tick did not also germinate"
            );
            died_on = Some(tick);
            break;
        }
    }
    let died_on = died_on.expect("the victim outlived two hundred ticks");
    let v = flora.view();
    assert_eq!(
        v.ledger.deaths, 1,
        "tick {died_on}: one death in the whole run"
    );
    assert_eq!(v.ledger.establishments, 1, "one birth in the whole run");
    assert_eq!(v.ledger.births, 3, "two founders and one germination");

    // The stand on the site is a different stand of the same species, and the victim's
    // identity is gone from the world.
    let now = *v.stand_at(victim_site).expect("occupied throughout");
    assert_eq!(
        now.species,
        Species::Bloomcrown,
        "the replacement is its own species"
    );
    assert_ne!(
        now.id, victim_id,
        "the site watch's blind spot: same site, same species"
    );
    assert!(
        now.id >= 2,
        "a germinated stand's id comes after the founders': {}",
        now.id
    );
    assert!(
        !v.stands.iter().any(|s| s.id == victim_id),
        "the victim is still standing"
    );
    assert!(
        v.stands.iter().any(|s| s.id == donor_id),
        "the donor died too"
    );
    // It was born of the bank, so it is one package and not the victim's remains.
    let sc = flora.config().species(Species::Bloomcrown);
    let package = sc.alive_min / sc.propagule_split[0];
    assert!(
        (now.organic() - package).abs() <= 1e-12 * package,
        "born with {} for a {package} package",
        now.organic()
    );
    // And the victim's remains are under it.
    let g = v.ground_at(victim_site).expect("the grave");
    assert!(
        g.dead_wood > 0.0 && g.litter > 0.0,
        "the victim left no remains: {g:?}"
    );
    assert_residuals(&flora, "after a same-tick death and replacement");
}

// ============================================================ root-zone aeration

/// The eighteen voxels of `at(2, 1)`'s root box — `rooting_radius` 1 over three columns
/// and three slabs, two soil rows deep — in one fixed order, so a fixture and the code
/// that dries it out again agree on which voxel is which.
fn root_box_order() -> Vec<(i64, u32, u32)> {
    let mut v = Vec::new();
    for z in 0..3u32 {
        for y in 1..=2u32 {
            for x in 1..4i64 {
                v.push((x, y, z));
            }
        }
    }
    assert_eq!(v.len(), 18);
    v
}

/// A three-slab strip whose middle site has exactly `saturated` of its eighteen root
/// voxels at pore 0.98 — past both species' `saturated_pore` of 0.95 — and the rest at
/// 0.5. The pore fraction is built in when the soil is made: the core's `AddWater` puts
/// free water in a void and will not push pore water into a voxel that is already solid.
fn box_world(saturated: usize) -> World {
    assert!(saturated <= 18);
    let order = root_box_order();
    let config = VoxelConfig {
        width: 5,
        height: 8,
        depth: 3,
        voxel_m: 1.0,
        seed: 5,
        ..VoxelConfig::default()
    };
    let mut w = World::empty(config);
    for z in 0..3u32 {
        for x in 0..5i64 {
            for y in 1..=2u32 {
                let wet = order
                    .iter()
                    .position(|&t| t == (x, y, z))
                    .is_some_and(|i| i < saturated);
                fill(
                    &mut w,
                    x,
                    y,
                    z,
                    Material::Soil,
                    if wet { 0.98 } else { 0.5 },
                );
            }
        }
    }
    w
}

/// The shape of the aeration response. Package J made it first-order relaxation toward a
/// **target** the root box sets: `target = (f − tol) / (1 − tol)` clamped to `0..=1`, with
/// `f` the saturated fraction and `tol` the species' own `establish_saturated_max`, and
/// `stress` closes a fraction of the remaining gap every second — `stress_rate_per_s` of
/// it upward, `relax_rate_per_s` downward. So `f` picks the level and the rates pick only
/// how fast the stand gets there.
///
/// What it replaced, which package I measured: an increment `rate·dt·f − relax·dt·(1−f)`
/// that did not depend on `stress`, so the pair of rates was not a strength but a
/// threshold `f* = relax / (rate + relax)` = 0.0909 for bloomcrown — **two** saturated
/// voxels of an eighteen-voxel box pinned it at stress 1 forever. Those same two voxels
/// are `f` = 0.1111, under the tolerance 0.25, and now cost it exactly nothing.
///
/// Three readings of bloomcrown's own box, and both rates, with no rate the test's own:
///   - 2 of 18 (`f` = 0.1111 < tol): stress exactly 0, for 2,000 ticks.
///   - 18 of 18 (`f` = 1, target 1): `1 − (1 − rate·DT)^n`, which is 0.634 at 100 ticks —
///     one time constant of the 0.2 /s placeholder — and within 1e-6 of 1 by 2,000.
///   - back to 2 of 18: `(1 − relax·DT)^n` of where it was, ten times slower, which is
///     0.135 of it after 2,000 ticks.
#[test]
fn aeration_stress_relaxes_toward_the_level_its_saturated_fraction_asks_for() {
    let sc = SpeciesConfig::bloomcrown();
    assert_eq!(
        sc.establish_saturated_max, 0.25,
        "the tolerance the target measures from"
    );
    assert!(
        2.0 / 18.0 < sc.establish_saturated_max,
        "two of eighteen is under the tolerance"
    );
    let here = at(2, 1);

    // Two saturated voxels of eighteen: under the tolerance, so no stress ever, and the
    // stand earns the whole time.
    let mut world = box_world(2);
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 1,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    run(&mut flora, &mut world, 2000);
    assert_eq!(
        flora.view().stand_at(here).unwrap().aeration_stress,
        0.0,
        "2 of 18 saturated voxels (f = {:.4}) stressed a stand whose tolerance is 0.25",
        2.0 / 18.0
    );
    assert!(
        flora.view().ledger.fixed_in > 0.0,
        "it earned nothing under no stress at all"
    );

    // A wholly saturated box: the target is 1, approached at `stress_rate_per_s` per
    // second of the gap that is left.
    let mut world = box_world(18);
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 1,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    run(&mut flora, &mut world, 100);
    let rise = 1.0 - (1.0 - sc.stress_rate_per_s * DT).powi(100);
    let s = flora.view().stand_at(here).unwrap().aeration_stress;
    assert!(
        (s - rise).abs() < 1e-9,
        "after 100 ticks the stress is {s}, not {rise}"
    );
    assert!(
        (rise - 0.634).abs() < 1e-3,
        "one time constant of the placeholder rate is {rise}"
    );
    run(&mut flora, &mut world, 1900);
    let s = flora.view().stand_at(here).unwrap().aeration_stress;
    assert!(
        1.0 - s < 1e-6,
        "a wholly saturated box left the stress at {s}, not at its target 1"
    );
    assert_eq!(
        flora.view().stand_at(here).unwrap().moisture,
        1.0,
        "μ is not what is happening"
    );

    // Dry sixteen of the eighteen: `f` = 2/18 is under the tolerance again, the target is
    // 0, and the stress falls first-order at `relax_rate_per_s` — ten times slower than it
    // rose, which is the asymmetry the placeholders ask for.
    let order = root_box_order();
    for &(x, y, z) in order.iter().take(16) {
        drain_to(&mut world, x, y, z, 0.5);
    }
    let before = flora.view().stand_at(here).unwrap().aeration_stress;
    run(&mut flora, &mut world, 2000);
    let want = before * (1.0 - sc.relax_rate_per_s * DT).powi(2000);
    let s = flora.view().stand_at(here).unwrap().aeration_stress;
    assert!(
        (s - want).abs() < 1e-9,
        "after 2,000 ticks of relaxation the stress is {s}, not {want}"
    );
    assert!(
        (want - 0.135).abs() < 1e-3,
        "two time constants of the relax placeholder is {want}"
    );
    assert_residuals(&flora, "after a root box was wetted and dried");
}

/// **Package I's finding, fixed.** I measured that a box held at a fixed intermediate
/// saturation had no equilibrium stress at all: the old increment did not depend on
/// `stress`, so every fraction but the knife-edge `f*` ramped to 0 or to 1 and half a
/// saturated root box was never half a stress. The target rule gives it one, and this is
/// the same nine-of-eighteen fixture (`f` = 0.5) that measured the finding:
///
///   - bloomcrown, tol 0.25: target `(0.5 − 0.25) / 0.75` = **1/3**, and it settles there.
///     Its income is multiplied by 2/3 rather than by nothing, which is what "a cost" was
///     supposed to mean.
///   - umbrellafrond, tol 1.0: no saturation stress at any fraction whatever, so exactly
///     **0**. Under the old rule it read 0.0 here too — but by ramping *away* from the
///     boundary its own `f*` of 0.8333 put it above, not because saturation costs it
///     nothing.
///
/// 2,000 ticks is 20 time constants of bloomcrown's 0.2 /s rise, so 1e-6 is a slack
/// tolerance on a value converged to about 2e-9.
#[test]
fn a_half_saturated_root_box_settles_at_an_interior_stress() {
    assert_eq!(SpeciesConfig::bloomcrown().establish_saturated_max, 0.25);
    assert_eq!(SpeciesConfig::umbrellafrond().establish_saturated_max, 1.0);
    for (species, want) in [
        (Species::Bloomcrown, 1.0 / 3.0),
        (Species::Umbrellafrond, 0.0),
    ] {
        let mut world = box_world(9);
        let mut flora = Flora::new(FloraConfig::default());
        assert!(flora.apply(
            &world,
            Command::Seed {
                x: 2,
                z: 1,
                species,
                wood: 0.1
            }
        ));
        run(&mut flora, &mut world, 2000);
        let s = *flora.view().stand_at(at(2, 1)).expect("alive");
        assert!(
            (s.aeration_stress - want).abs() < 1e-6,
            "{} on a half-saturated box sits at stress {}, not {want}",
            species.name(),
            s.aeration_stress
        );
        assert!(
            s.moisture > 0.0,
            "μ is not what is being measured: {}",
            s.moisture
        );
        if want > 0.0 {
            assert!(
                s.aeration_stress > 0.0 && s.aeration_stress < 1.0,
                "{} is at a boundary, not strictly inside 0..1: {}",
                species.name(),
                s.aeration_stress
            );
        }
        assert_residuals(&flora, "after a half-saturated box settled");
    }
}

/// The income multiplier is exactly `1 − stress`, measured on the one tick where two arms
/// are otherwise identical: the stress is updated at the top of the per-stand pass and
/// applied to the same tick's income, so on the first tick one arm's stand has stress
/// `rate · dt` and the other's has 0 and everything else about them is equal to the bit.
///
/// `wood_rate` is 1.0 /s here (placeholder 0.001) in both arms: at the placeholder a
/// young stand's demand for new tissue is far below what its foliage could fix, so `A` is
/// demand-capped and the income factor cannot be read off it at all. `stress_rate_per_s`
/// is 0.0 in the control arm, which is the instrument itself.
#[test]
fn income_is_multiplied_by_one_minus_the_stress_this_tick() {
    let mut stressed_config = FloraConfig::default();
    stressed_config.bloomcrown.wood_rate = 1.0;
    let mut calm_config = stressed_config.clone();
    calm_config.bloomcrown.stress_rate_per_s = 0.0;

    let mut a_world = box_world(18);
    let mut b_world = box_world(18);
    let mut a = Flora::new(stressed_config);
    let mut b = Flora::new(calm_config);
    for (f, w) in [(&mut a, &mut a_world), (&mut b, &mut b_world)] {
        assert!(f.apply(
            w,
            Command::Seed {
                x: 2,
                z: 1,
                species: Species::Bloomcrown,
                wood: 0.1
            }
        ));
    }

    a.step(&mut a_world);
    b.step(&mut b_world);

    let sa = *a.view().stand_at(at(2, 1)).unwrap();
    let sb = *b.view().stand_at(at(2, 1)).unwrap();
    assert_eq!(sb.aeration_stress, 0.0, "the control arm stressed");
    assert!(sa.aeration_stress > 0.0, "the stressed arm did not stress");
    assert_eq!(sa.light, sb.light, "the two arms saw different light");
    assert_eq!(
        sa.moisture, sb.moisture,
        "the two arms saw different moisture"
    );

    let (fa, fb) = (a.view().ledger.fixed_in, b.view().ledger.fixed_in);
    assert!(
        fb > 0.0,
        "the control fixed nothing: the demand cap is still binding"
    );
    let ratio = fa / fb;
    assert!(
        (ratio - (1.0 - sa.aeration_stress)).abs() <= 1e-15,
        "income ratio {ratio} against 1 - stress {}",
        1.0 - sa.aeration_stress
    );
}

/// `establish_saturated_max` is a **non-strict** ceiling read on the site now, and
/// umbrellafrond's placeholder of 1.0 makes it inert: a wholly saturated box has fraction
/// exactly 1.0 and `1.0 > 1.0` is false, so the bound never turns umbrellafrond away from
/// anywhere. Bloomcrown's 0.25 on an 18-voxel box is the discrete boundary between four
/// saturated voxels (4/18 = 0.2222, passes) and five (5/18 = 0.2778, refused).
///
/// The bank is paid for by a donor of its own species; `propagule_rate` is 3.0 /s
/// (placeholder 2e-4) and `reserve_cap` 40 (placeholder 0.5) so the donor funds a whole
/// package every tick for the whole run, and `hop` is 1 for bloomcrown (placeholder 2) so
/// it reaches only its immediate neighbours. Round 3b sends one package to one drawn
/// recipient per tick, so the middle column takes about an eighth of eighty of them
/// rather than a share of one tick's budget: this is 80 ticks where it was 1, and the
/// threshold is read off the bank whenever it has enough. The saturation is the fixture's.
#[test]
fn the_saturation_ceiling_is_non_strict_and_umbrellafrond_s_is_inert() {
    let mut config = FloraConfig::default().drop_seeds_checked_each_tick();
    config.bloomcrown.propagule_rate = 3.0;
    config.bloomcrown.hop = 1;
    config.bloomcrown.reserve_cap = 40.0;
    config.umbrellafrond.propagule_rate = 3.0;
    config.umbrellafrond.reserve_cap = 40.0;
    assert_eq!(config.bloomcrown.establish_saturated_max, 0.25);
    assert_eq!(config.umbrellafrond.establish_saturated_max, 1.0);

    for (saturated, bloom_should) in [(4usize, true), (5, false), (18, false)] {
        let mut world = box_world(saturated);
        let mut flora = Flora::new(config.clone());
        // Two donors either side of the box's middle column, one of each species, so both
        // banks land on the same site and only the ceiling can separate them.
        assert!(flora.apply(
            &world,
            Command::Seed {
                x: 1,
                z: 1,
                species: Species::Bloomcrown,
                wood: 0.6
            }
        ));
        // Eighty ticks of packages, then the threshold is read: on the arms where
        // germination is refused the bank is still there to read, and on the arm where it
        // is allowed the site holds the stand it built.
        run(&mut flora, &mut world, 80);
        let target = at(2, 1);
        let sc = flora.config().species(Species::Bloomcrown).clone();
        let g = flora.view().ground_at(target).expect("ground");
        let bank = g.seed_organic(Species::Bloomcrown);
        assert!(
            sc.propagule_split[0] * bank >= sc.alive_min || flora.view().stand_at(target).is_some(),
            "{saturated}/18: the bank is neither over the threshold nor spent: {bank}"
        );
        assert_eq!(
            flora.view().stand_at(target).is_some(),
            bloom_should,
            "{saturated} of 18 saturated (f = {:.4}) against bloomcrown's ceiling 0.25: \
             germinated = {}",
            saturated as f64 / 18.0,
            flora.view().stand_at(target).is_some()
        );
        assert_residuals(&flora, "after a saturation ceiling was tested");
    }

    // Umbrellafrond's 1.0 turns it away from nothing, including a box that is saturated to
    // the last voxel.
    let mut world = box_world(18);
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 1,
            z: 1,
            species: Species::Umbrellafrond,
            wood: 0.6
        }
    ));
    run(&mut flora, &mut world, 80);
    let sc = SpeciesConfig::umbrellafrond();
    let bank = flora
        .view()
        .ground_at(at(2, 1))
        .expect("ground")
        .seed_organic(Species::Umbrellafrond);
    assert!(
        sc.propagule_split[0] * bank >= sc.alive_min,
        "the umbrellafrond bank is not over the threshold: {bank}"
    );
    run(&mut flora, &mut world, 10);
    let born = flora.view().stand_at(at(2, 1));
    assert_eq!(
        born.map(|s| s.species),
        Some(Species::Umbrellafrond),
        "a ceiling of 1.0 refused a fraction of 1.0: the comparison is strict after all"
    );
}

/// **The root box admits only `Material::Soil`**, so every voxel in it has the same pore
/// capacity — which makes both of the weighting decisions in the water read unobservable:
/// `mean_water`'s band weighting and `saturated_fraction`'s deliberate lack of it can
/// never differ, today, on any fixture. And rock, which the core gives a real pore
/// capacity of 0.02, is invisible: a stand rooted wholly in saturated rock reads `μ = 0`
/// and a saturated fraction of 0, and a bank on such a site can never germinate however
/// wet the rock is.
///
/// Worth stating because correction 2's commit message defends the unweighted count as a
/// design call ("what a root needs is somewhere to breathe and not a volume of it") and
/// nothing in the model can currently tell the two apart.
#[test]
fn the_root_box_sees_soil_only_so_saturated_rock_is_neither_wet_nor_waterlogged() {
    assert!(
        Material::Rock.pore_capacity() > 0.0,
        "the premise: rock does hold pore water"
    );
    let config = VoxelConfig {
        width: 6,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 5,
        ..VoxelConfig::default()
    };
    let mut world = World::empty(config);
    // One soil column, five of saturated rock. Every support face is at y = 2.
    for y in 1..=2u32 {
        fill(&mut world, 0, y, 0, Material::Soil, 0.6);
        for x in 1..6i64 {
            fill(&mut world, x, y, 0, Material::Rock, 1.0);
        }
    }
    let mut cfg = FloraConfig::default();
    // `hop` 2 is the placeholder; it is what lets the one viable donor reach x2 and x4,
    // whose root boxes hold no soil at all.
    assert_eq!(cfg.bloomcrown.hop, 2);
    cfg.bloomcrown.propagule_rate = 3.0;
    // Round 3b: a donor saves for one drawn recipient at a time, so `reserve_cap` 40
    // against the placeholder 0.5 is what funds the dozens of packages four recipients
    // need before any of them holds two. At the placeholder the two donors could afford
    // two packages between them in the whole run.
    cfg.bloomcrown.reserve_cap = 40.0;
    let mut flora = Flora::new(cfg);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 0,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.6
        }
    ));
    // A second stand rooted wholly in the saturated rock.
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 3,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.6
        }
    ));

    run(&mut flora, &mut world, 300);

    let on_rock = *flora.view().stand_at(site(3)).expect("alive, just thirsty");
    assert_eq!(on_rock.moisture, 0.0, "saturated rock watered a plant");
    assert_eq!(
        on_rock.aeration_stress, 0.0,
        "saturated rock drowned a root zone: an empty box is not waterlogged"
    );
    assert_eq!(
        flora.view().ledger.deaths,
        0,
        "nothing was supposed to die in 300 ticks"
    );

    // x2 and x4 root wholly in rock. Their banks are over the threshold and they never
    // germinate; x1, whose box reaches the one soil column, is the control that does.
    let sc = flora.config().species(Species::Bloomcrown);
    for x in [2u32, 4] {
        let g = flora
            .view()
            .ground_at(site(x))
            .unwrap_or_else(|| panic!("nothing banked at {x}"));
        let bank = g.seed_organic(Species::Bloomcrown);
        assert!(
            sc.propagule_split[0] * bank >= sc.alive_min,
            "site {x} is not over the threshold: {bank}"
        );
        assert!(
            flora.view().stand_at(site(x)).is_none(),
            "a bank germinated on a site with no soil under it at all: {x}"
        );
    }
    assert!(
        flora.view().stand_at(site(1)).is_some(),
        "the control site, whose root box reaches the soil column, did not germinate"
    );
    assert_residuals(&flora, "after 300 ticks over saturated rock");
}

/// Umbrellafrond's `establish_saturated_max` is 1.0, and package J made that same number
/// the tolerance the stress target measures from, so the wet producer has **no**
/// saturation stress anywhere: a root box saturated to the last voxel is its habitat and
/// it earns in it. Package I measured the opposite under the increment rule — `f*` 0.8333,
/// stress 0.5 at tick 1,000 and exactly 1.0 by tick 2,000, with not one further unit of
/// organic matter fixed, "20 times more slowly than bloomcrown but just as completely".
///
/// Its drowning path is `drown_depth_m` alone, and that is the second half of this test:
/// 0.6 m of free water standing over the support face against its 0.5 m limit kills it in
/// one tick. Wet soil is not a pool, and only the pool is fatal.
#[test]
fn umbrellafrond_never_stresses_from_saturation_and_drowns_only_by_depth() {
    let sc = SpeciesConfig::umbrellafrond();
    assert_eq!(
        sc.establish_saturated_max, 1.0,
        "no aeration bound, so no aeration stress"
    );
    assert_eq!(sc.drown_depth_m, 0.5, "the one water that does kill it");

    let mut world = box_world(18);
    let mut flora = Flora::new(FloraConfig::default());
    let here = at(2, 1);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 1,
            species: Species::Umbrellafrond,
            wood: 0.1
        }
    ));

    // Twice as long as package I's ramp took to reach the ceiling.
    run(&mut flora, &mut world, 4000);
    let s = *flora.view().stand_at(here).expect("alive and earning");
    assert_eq!(
        s.aeration_stress, 0.0,
        "a wholly saturated box stressed it at all"
    );
    assert_eq!(s.light, 1.0, "open sky");
    assert_eq!(
        s.moisture, 1.0,
        "wetter than full_water: water is not a limit either"
    );
    assert!(s.wood > 0.1, "it did not grow: wood {}", s.wood);
    let fixed = flora.view().ledger.fixed_in;
    run(&mut flora, &mut world, 200);
    assert!(
        flora.view().ledger.fixed_in > fixed,
        "it stopped fixing light"
    );
    assert_eq!(
        flora.view().ledger.deaths,
        0,
        "waterlogged soil is not a pool"
    );

    // The one water that does kill it: 0.6 m standing on the face, over the 0.5 m limit.
    let took = world.apply(WorldCommand::AddWater {
        x: 2,
        y: 3,
        z: 1,
        volume_m3: 0.6,
    });
    assert!((took - 0.6).abs() < 1e-12, "the void took {took} of 0.6");
    let depth = world.view().water_depth_m(2, 2, 1);
    assert!(
        depth > sc.drown_depth_m,
        "{depth} m over the face is not over the limit"
    );
    run(&mut flora, &mut world, 1);
    assert_eq!(
        flora.view().ledger.deaths,
        1,
        "{depth} m of standing water did not drown it"
    );
    assert!(
        flora.view().stand_at(here).is_none(),
        "it drowned and is still standing"
    );
    assert_residuals(
        &flora,
        "after an umbrellafrond earned in a saturated box and drowned in a pool",
    );
}

/// The same two levels on a coupled fixture rather than on hand-set pore fractions, so
/// neither can be an artifact of the way the fixture was built: a water table charged
/// above the support face, the world stepped every tick, and no rain. The core tops both
/// soil rows to capacity, so the saturated fraction of either species' root box is 1 —
/// bloomcrown's target is 1 and umbrellafrond's is 0, and that is what they reach.
///
/// Nothing stands in free water here (`water_depth_m` at the face is 0 throughout), so
/// this is waterlogged *soil*, which is the thing the aeration rule is about.
#[test]
fn the_same_holds_on_a_water_table_basin_with_the_world_stepping() {
    let basin = || {
        let config = VoxelConfig {
            width: 6,
            height: 8,
            depth: 1,
            voxel_m: 1.0,
            seed: 11,
            initial_aquifer_head_m: 3.0,
            ..VoxelConfig::default()
        };
        let mut world = World::empty(config);
        for x in 0..6i64 {
            for y in 1..=2u32 {
                fill(&mut world, x, y, 0, Material::Soil, 0.98);
            }
        }
        world
    };
    let coupled = |flora: &mut Flora, world: &mut World, ticks: u32| {
        for _ in 0..ticks {
            world.step();
            flora.step(world);
        }
    };

    // Bloomcrown: the target is 1 and it converges on it, within 1e-6 by 2,000 ticks.
    let mut world = basin();
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.1
        }
    ));
    coupled(&mut flora, &mut world, 2000);
    let s = *flora
        .view()
        .stand_at(site(2))
        .expect("alive, earning almost nothing");
    assert_eq!(
        world.view().water_depth_m(2, 2, 0),
        0.0,
        "this is wet soil, not a pool"
    );
    assert_eq!(
        flora.view().ledger.deaths,
        0,
        "drowning by depth is not what this is"
    );
    assert_eq!(s.moisture, 1.0, "water is not the limit");
    assert!(
        1.0 - s.aeration_stress < 1e-6,
        "bloomcrown's stress is {}, not its target 1",
        s.aeration_stress
    );

    // Umbrellafrond on the same basin: no stress at all, and still earning.
    let mut world = basin();
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Umbrellafrond,
            wood: 0.1
        }
    ));
    coupled(&mut flora, &mut world, 2000);
    let s = *flora.view().stand_at(site(2)).expect("alive");
    assert_eq!(
        s.aeration_stress, 0.0,
        "umbrellafrond stressed to {}",
        s.aeration_stress
    );
    assert_eq!(s.moisture, 1.0, "water is not the limit");
    let fixed = flora.view().ledger.fixed_in;
    coupled(&mut flora, &mut world, 100);
    assert!(
        flora.view().ledger.fixed_in > fixed,
        "it stopped fixing light"
    );
    assert_residuals(&flora, "after two species sat on a water-table basin");
}
