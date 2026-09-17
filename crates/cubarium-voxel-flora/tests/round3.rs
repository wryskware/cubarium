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

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World, DT};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Ground, Site, Species, SpeciesConfig};

// ------------------------------------------------------------------- fixtures

/// A strip of `depth` slabs: bedrock at `y = 0`, soil at `y = 1..=2` at a chosen pore
/// fraction, air above. Every column's support face is `y = 2`, in open sky.
fn strip(width: u32, depth: u32, pore: f64) -> World {
    let config =
        VoxelConfig { width, height: 8, depth, voxel_m: 1.0, seed: 5, ..VoxelConfig::default() };
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

/// Turn one air voxel into `material` holding exactly `pore` of that material's own pore
/// capacity, by adding the water first and converting after.
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

/// Take pore water out of one voxel down to `target`, through the core's own bounded
/// withdrawal — the only way to *dry* a voxel without rebuilding it.
fn drain_to(w: &mut World, x: i64, y: u32, z: u32, target: f64) {
    let cap = Material::Soil.pore_capacity() * w.config().voxel_volume();
    let want = (w.view().pore_at(x, y, z) - target) * cap;
    assert!(want > 0.0, "already at or below {target}");
    let took = -w.apply(WorldCommand::WithdrawPore { x, y, z, volume_m3: want });
    assert!((took - want).abs() < 1e-12, "the core gave {took} of {want}");
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
    assert!(o.abs() <= 1e-9 * v.organic().abs().max(1.0), "{when}: organic residual {o}");
    assert!(n.abs() <= 1e-9 * v.mineral().abs().max(1.0), "{when}: mineral residual {n}");
    assert!(e.abs() <= 1e-9 * v.energy().abs().max(1.0), "{when}: energy residual {e}");
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
    let mut world = strip(8, 1, 0.6);
    let mut flora = Flora::new(config);

    // The donor: umbrellafrond, below `wood_max` so it really grows, with `hop` 1 so it
    // reaches exactly x1 and x3.
    assert!(flora.apply(&world, Command::Seed { x: 2, z: 0, species: Species::Umbrellafrond, wood: 0.5 }));
    // The victim: a bloomcrown at `alive_min`, far outside the donor's reach.
    assert!(flora.apply(&world, Command::Seed { x: 6, z: 0, species: Species::Bloomcrown, wood: 0.02 }));

    let seeded_mineral = flora.view().ledger.seeded_mineral_in;
    let mineral0 = flora.view().mineral();
    assert!(
        (mineral0 - seeded_mineral).abs() < 1e-15,
        "before anything moves, every unit of mineral is a seeded one: {mineral0} against {seeded_mineral}"
    );
    let pool0 = flora.view().ground_at(site(2)).expect("the donor's ground").mineral;

    run(&mut flora, &mut world, 400);

    let v = flora.view();
    // The cycle really happened, all of it.
    assert_eq!(v.ledger.establishments, 2, "the two banks did not both germinate");
    assert_eq!(v.ledger.deaths, 1, "the victim did not die, or something else did");
    for x in [1u32, 3] {
        let s = v.stand_at(site(x)).unwrap_or_else(|| panic!("nothing germinated at {x}"));
        assert_eq!(s.species, Species::Umbrellafrond);
        assert!(s.mineral > 0.0, "a germinated stand holds no mineral: {s:?}");
    }
    let donor = v.stand_at(site(2)).expect("the donor");
    assert!(donor.wood > 0.5, "the donor never grew: {}", donor.wood);
    let pool = v.ground_at(site(2)).expect("ground").mineral;
    assert!(pool < pool0, "the donor's growth drew no mineral: {pool0} -> {pool}");
    let grave = v.ground_at(site(6)).expect("the victim's ground");
    assert!(grave.litter > 0.0 && grave.dead_wood > 0.0, "the victim left no remains: {grave:?}");
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
    assert_eq!(e_v, config.litter_energy_cap, "the fixture's premise: e_v == e_d_max");
    let mut world = strip(4, 1, 0.0);
    let mut flora = Flora::new(config);
    assert!(flora.apply(&world, Command::Seed { x: 1, z: 0, species: Species::Bloomcrown, wood: 0.6 }));
    let seeded_organic = flora.view().ledger.seeded_organic_in;
    let seeded_energy = flora.view().ledger.seeded_energy_in;

    run(&mut flora, &mut world, 400);

    let v = flora.view();
    assert_eq!(v.ledger.fixed_in, 0.0, "a wilting stand fixed organic matter");
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
    // Exactly zero here: nothing on this fixture moves mineral except litterfall and
    // decomposition, which take and give the same value.
    assert_eq!(n, 0.0, "mineral residual {n} in a world with no income at all");
    assert_residuals(&flora, "after 400 ticks of pure respiration");
}

/// A propagule package's construction respiration destroys organic matter and releases no
/// mineral, so the whole of what the donor debited itself in mineral travels on in the
/// cohort — which is exactly `1 + c_g` times as mineral-rich per unit of organic matter as
/// the donor it came from.
///
/// That factor is the unstated half of the design call. It is not a leak — the mineral is
/// all still there — but it means a seed bank, and therefore every stand born out of one,
/// carries mineral at a density that has nothing to do with `n_tissue`.
///
/// The donor is **frozen** — `assimilation`, `maintenance` and `senescence` all zero — so
/// its stocks move only where the propagule debits them and the arithmetic can be checked
/// exactly. `hop` is 1 (placeholder 2) so there are exactly two recipients on a
/// four-column ring.
#[test]
fn a_package_keeps_all_its_mineral_through_construction_respiration() {
    let mut config = FloraConfig::default();
    let sc = &mut config.bloomcrown;
    sc.assimilation = 0.0;
    sc.maintenance = 0.0;
    sc.senescence = 0.0;
    sc.hop = 1;
    let build = config.bloomcrown.build;
    let n_tissue = config.bloomcrown.n_tissue;
    let rate = config.bloomcrown.propagule_rate;
    let mut world = strip(4, 1, 0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(&world, Command::Seed { x: 1, z: 0, species: Species::Bloomcrown, wood: 0.5 }));

    let before = *flora.view().stand_at(site(1)).expect("the donor");
    let organic0 = before.organic();
    let mineral0 = before.mineral;
    assert!(
        (mineral0 - n_tissue * organic0).abs() < 1e-18,
        "a founder arrives at exactly n_tissue: {mineral0} against {}",
        n_tissue * organic0
    );

    flora.step(&mut world);

    let after = *flora.view().stand_at(site(1)).expect("still there");
    assert_eq!(flora.view().ledger.fixed_in, 0.0, "the frozen donor earned something");
    assert_eq!(after.wood, before.wood, "the frozen donor's wood moved");
    assert_eq!(after.foliage, before.foliage, "the frozen donor's foliage moved");

    // Two recipients, each the same package: `propagule_rate · DT` sent, `/(1 + c_g)`
    // banked.
    let each_sent = rate * DT;
    let each_net = each_sent / (1.0 + build);
    let mut banked_organic = 0.0;
    let mut banked_mineral = 0.0;
    for x in [0u32, 2] {
        let g = flora.view().ground_at(site(x)).unwrap_or_else(|| panic!("nothing at {x}"));
        assert_eq!(g.seeds.len(), 1, "one cohort: {:?}", g.seeds);
        let c = g.seeds[0];
        assert_eq!(c.species, Species::Bloomcrown);
        assert_eq!(c.age_ticks, 0, "a cohort that just landed is age 0");
        assert!(
            (c.organic - each_net).abs() <= 1e-15 * each_net,
            "{c:?} for a {each_net} package"
        );
        // The unstated factor: the cohort is `1 + c_g` times as mineral-rich as its donor.
        let density = c.mineral / c.organic;
        assert!(
            (density - (1.0 + build) * n_tissue).abs() <= 1e-12 * density,
            "cohort density {density}, donor density {}, ratio {}",
            mineral0 / organic0,
            density / (mineral0 / organic0)
        );
        banked_organic += c.organic;
        banked_mineral += c.mineral;
    }

    // The donor was debited exactly what arrived plus what the build respired, in both
    // currencies — and the mineral side of that is the whole debit, not `1 / (1 + c_g)` of
    // it.
    // Both differences are read off stocks three orders of magnitude larger than they
    // are, so the tolerance is absolute at the stock's own ulp and not relative to the
    // difference: `1.75 - 1.74998` cannot be exact in f64 whatever the model does.
    let sent = organic0 - after.organic();
    assert!(
        (sent - 2.0 * each_sent).abs() <= 1e-15,
        "the donor sent {sent} for two {each_sent} packages"
    );
    let debited = mineral0 - after.mineral;
    assert!(
        (debited - banked_mineral).abs() <= 1e-16,
        "the donor was debited {debited} of mineral and {banked_mineral} arrived"
    );
    // Against the two packages themselves, not against the cancelled `sent`.
    let respired = flora.view().ledger.respired_out;
    assert!(
        (respired - (2.0 * each_sent - banked_organic)).abs() <= 1e-15 * respired,
        "respired_out {respired} against sent - banked {}",
        2.0 * each_sent - banked_organic
    );
    let (_, n, _) = residuals(&flora);
    // 8.9e-16 on a stock of 3.03, one ulp of the total: no mineral crossed the boundary,
    // and the pairwise-transfer rounding is all that is left.
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
    assert_eq!(config.umbrellafrond.n_tissue, n_tissue, "the two species share it this round");
    let mut world = strip(8, 1, 0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(&world, Command::Seed { x: 1, z: 0, species: Species::Bloomcrown, wood: 0.1 }));
    assert!(flora.apply(&world, Command::Seed { x: 5, z: 0, species: Species::Umbrellafrond, wood: 0.1 }));

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
        assert!(f.apply(&world, Command::Seed { x: 1, z: 0, species: Species::Bloomcrown, wood: 0.1 }));
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
    assert_eq!(s.moisture, 1.0, "past sat_pore");
    assert_eq!(s.aeration_stress, 0.0, "pore 0.6 is nowhere near saturated_pore");
    assert!(flora_reserve(&bare) < reserve0, "it paid nothing out of reserve");
    assert!(ample.view().ledger.fixed_in > 0.0, "the ample control fixed nothing either");
    // After fifty ticks it is *not* zero any more, and that is the second unstated thing
    // here: the stand's own senescence puts mineral into the site's litter, decomposition
    // hands a fraction of it back to the pool every tick, and the stand starts earning
    // again off what it shed. An exhausted pool is not a permanent floor — it recovers on
    // the litter's decomposition timescale — so "growth stops when the pool is spent" is
    // asymptotic and not absolute.
    let bare_fixed = bare.view().ledger.fixed_in;
    assert!(bare_fixed > 0.0, "the litter loop did not restart the income at all");
    assert!(
        bare_fixed < 1e-6 * ample.view().ledger.fixed_in,
        "a bare pool earned {bare_fixed} against the ample control's {}",
        ample.view().ledger.fixed_in
    );
    let pool = bare.view().ground_at(site(1)).expect("ground").mineral;
    assert!(pool > 0.0, "the litter released no mineral to the bare pool");
    assert_residuals(&bare, "after 50 ticks on a bare pool");
}

fn flora_reserve(flora: &Flora) -> f64 {
    flora.view().stands.iter().map(|s| s.reserve).sum()
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
    assert!(flora.apply(&world, Command::Seed { x: 2, z: 0, species: Species::Umbrellafrond, wood: 0.5 }));
    for x in [1i64, 3] {
        assert!(flora.apply(&world, Command::Seed { x, z: 0, species: Species::Bloomcrown, wood: 0.6 }));
    }

    run(&mut flora, &mut world, 100);

    let doomed = site(2);
    let stand = *flora.view().stand_at(doomed).expect("the victim is still standing");
    let g = flora.view().ground_at(doomed).expect("ground").clone();
    assert!(!g.seeds.is_empty(), "no bank waiting under the stand: {g:?}");
    assert!(g.litter > 0.0, "no litter");
    assert!(g.dead_wood > 0.0, "no dead wood: a stand in dieback left none");
    assert!(g.mineral > 0.0, "no pool");
    assert!(g.litter_mineral > 0.0 && g.dead_wood_mineral > 0.0, "detritus carries no mineral");

    let want_organic = stand.organic() + g.litter + g.dead_wood + g.seeds.iter().map(|c| c.organic).sum::<f64>();
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
    world.apply(WorldCommand::SetMaterial { x: 2, y: 3, z: 0, material: Material::Soil });
    assert!(!world.view().is_support(2, 2, 0), "burying did not take the support away");
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
    assert!(flora.view().stand_at(doomed).is_none(), "the stand outlived its support");
    assert!(flora.view().ground_at(doomed).is_none(), "the ground outlived its support");
    assert_residuals(&flora, "after a support was buried under a whole site");
}

/// The gap a *drowning* opens is filled in the same tick it opens. Drowning is step 3 of
/// the tick and germination is step 8, so a bank that was waiting under a stand becomes a
/// stand in the single step that killed its predecessor — and it pays that tick's
/// attrition on the way through, which is why its stocks are the bank's minus one tick of
/// decay rather than the bank's exactly.
///
/// `umbrellafrond.propagule_rate` is 3.0 /s (placeholder 2e-4) so the bank is over the
/// germination threshold in one tick. The water is the fixture's: 0.1 m over the support
/// face is past bloomcrown's `drown_depth_m` of 0.05 and well under umbrellafrond's 0.5,
/// so one species drowns where the other can stand.
#[test]
fn a_drowned_stand_s_gap_is_filled_by_its_bank_in_the_same_tick() {
    let mut config = FloraConfig::default();
    config.umbrellafrond.propagule_rate = 3.0;
    let mut world = strip(5, 1, 0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(&world, Command::Seed { x: 0, z: 0, species: Species::Umbrellafrond, wood: 0.6 }));
    assert!(flora.apply(&world, Command::Seed { x: 1, z: 0, species: Species::Bloomcrown, wood: 0.6 }));

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
    let (deaths0, est0) = (flora.view().ledger.deaths, flora.view().ledger.establishments);

    // 0.1 m³ in a one-metre voxel over the face: a tenth of a metre of standing water.
    let got = world.apply(WorldCommand::AddWater { x: 1, y: 3, z: 0, volume_m3: 0.1 });
    assert!((got - 0.1).abs() < 1e-12, "the void took {got}");
    assert!((world.view().water_depth_m(1, 2, 0) - 0.1).abs() < 1e-12);

    // One single step.
    flora.step(&mut world);

    assert_eq!(flora.view().ledger.deaths, deaths0 + 1, "the bloomcrown did not drown");
    assert_eq!(
        flora.view().ledger.establishments,
        est0 + 1,
        "the gap was not filled in the tick that opened it"
    );
    let born = *flora.view().stand_at(victim).expect("the gap stayed empty");
    assert_eq!(born.species, Species::Umbrellafrond);
    // Its stocks are the bank's, less the one tick of attrition the bank paid first.
    let kept = 1.0 - sc.seed_attrition_per_s * DT;
    assert!(
        (born.organic() - bank * kept).abs() <= 1e-12 * bank,
        "born with {} out of a {bank} bank ({} expected after one tick of attrition)",
        born.organic(),
        bank * kept
    );
    assert!(
        (born.mineral - bank_mineral * kept).abs() <= 1e-12 * bank_mineral,
        "born with {} of mineral out of {bank_mineral}",
        born.mineral
    );
    // The predecessor's remains are under it, and the bank of its own species is spent.
    let g = flora.view().ground_at(victim).expect("ground");
    assert!(g.dead_wood > 0.0 && g.litter > 0.0, "the drowned stand left no remains");
    // The bank that germinated was spent — what is on the site now is the single fresh
    // package the donor landed later in the very same tick, because `propagate` is step 9
    // and germination is step 8. So a site never has an empty bank for even one tick while
    // a donor is in reach of it.
    assert_eq!(g.seeds.len(), 1, "{:?}", g.seeds);
    assert_eq!(g.seeds[0].age_ticks, 0, "not a fresh package: {:?}", g.seeds);
    assert!(
        g.seeds[0].organic < 0.01 * bank,
        "the bank that germinated was not consumed: {} of a {bank} bank is still there",
        g.seeds[0].organic
    );
    assert_residuals(&flora, "after a drowning was replaced in one tick");
}

/// Two species' banks on one site, both over the threshold, both passing their own
/// predicate: the winner is whichever comes **earlier in `Species::ALL`**, and not the
/// larger bank. Here umbrellafrond's bank is four times bloomcrown's and still loses, and
/// the losing cohorts stay banked rather than being spent.
///
/// A fixed order is the right kind of tiebreak — it is not a `HashMap` iteration — but it
/// is worth writing down that it is *arbitrary*: bloomcrown pre-empts umbrellafrond in
/// every contested gap in the world, whatever the two banks hold.
///
/// Three rates are the test's own: both species' `propagule_rate` 3.0 /s (placeholder
/// 2e-4, umbrellafrond's 30.0 so that its donor's reserve and not the rate is what bounds
/// its package) so both banks clear the threshold in one tick, `bloomcrown.hop` 1
/// (placeholder 2) so a three-column ring is enough, and `umbrellafrond.reserve_cap` 2.0
/// (placeholder 0.5) so its donor has four times as much spendable reserve and its bank is
/// visibly the bigger one.
#[test]
fn the_earlier_species_in_the_fixed_order_wins_a_contested_gap_whatever_the_banks_hold() {
    assert_eq!(Species::ALL[0], Species::Bloomcrown, "the order this test reads");
    let mut config = FloraConfig::default();
    config.bloomcrown.propagule_rate = 3.0;
    config.bloomcrown.hop = 1;
    config.umbrellafrond.propagule_rate = 30.0;
    config.umbrellafrond.reserve_cap = 2.0;
    let mut world = strip(3, 1, 0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(&world, Command::Seed { x: 0, z: 0, species: Species::Umbrellafrond, wood: 0.6 }));
    assert!(flora.apply(&world, Command::Seed { x: 2, z: 0, species: Species::Bloomcrown, wood: 0.6 }));

    // One tick to bank on the one bare site.
    flora.step(&mut world);
    let contested = site(1);
    let g = flora.view().ground_at(contested).expect("ground");
    let (b, u) = (g.seed_organic(Species::Bloomcrown), g.seed_organic(Species::Umbrellafrond));
    for (species, bank) in [(Species::Bloomcrown, b), (Species::Umbrellafrond, u)] {
        let sc = flora.config().species(species);
        assert!(
            sc.propagule_split[0] * bank >= sc.alive_min,
            "{}'s bank is not over the threshold: {bank}",
            species.name()
        );
    }
    assert!(u > 3.0 * b, "the fixture's premise: umbrellafrond banked {u} against {b}");

    // The next tick germinates it.
    flora.step(&mut world);
    let born = *flora.view().stand_at(contested).expect("nothing germinated");
    assert_eq!(
        born.species,
        Species::Bloomcrown,
        "the larger bank won; the order is not the tiebreak"
    );
    assert_eq!(flora.view().ledger.establishments, 1, "only the one bare site can germinate");
    let g = flora.view().ground_at(contested).expect("ground");
    // Spent: what is there is the single fresh package the donor landed later in the same
    // tick, because `propagate` is step 9 and germination is step 8.
    assert!(
        g.seed_organic(Species::Bloomcrown) < 0.01 * b,
        "the winner's bank was not spent: {} of {b}",
        g.seed_organic(Species::Bloomcrown)
    );
    assert!(
        g.seed_organic(Species::Umbrellafrond) > 3.0 * born.organic(),
        "the loser's bank was spent or lost: {:?}",
        g.seeds
    );

    // And it stays banked while the winner stands, however long.
    run(&mut flora, &mut world, 40);
    assert_eq!(
        flora.view().stand_at(contested).map(|s| s.species),
        Some(Species::Bloomcrown),
        "the winner did not hold the site"
    );
    assert!(
        flora.view().ground_at(contested).unwrap().seed_organic(Species::Umbrellafrond) > 0.0,
        "the loser's bank vanished instead of waiting"
    );
    assert_residuals(&flora, "after a contested gap was decided by the fixed order");
}

/// The merge rule, both halves of it: a site a donor feeds every tick holds exactly one
/// cohort per species and that cohort's age stays 0, because the merge keeps the younger
/// age; a site that stops being fed keeps its single cohort and ages it one tick per tick
/// from the moment of its last arrival.
#[test]
fn a_fed_bank_stays_one_cohort_at_age_zero_and_ages_from_its_last_arrival() {
    let mut config = FloraConfig::default();
    config.bloomcrown.hop = 1;
    let mut world = strip(3, 1, 0.6);
    let mut flora = Flora::new(config);
    assert!(flora.apply(&world, Command::Seed { x: 0, z: 0, species: Species::Bloomcrown, wood: 0.6 }));

    let fed = site(1);
    for tick in 1..=30 {
        flora.step(&mut world);
        let g = flora.view().ground_at(fed).expect("ground");
        assert_eq!(g.seeds.len(), 1, "tick {tick}: {} cohorts, not one: {:?}", g.seeds.len(), g.seeds);
        assert_eq!(g.seeds[0].age_ticks, 0, "tick {tick}: a fed bank aged: {:?}", g.seeds);
    }
    let organic_fed = flora.view().ground_at(fed).unwrap().seeds[0].organic;

    // Take the donor away. The bank is one cohort and it now ages.
    assert!(flora.apply(&world, Command::Clear { x: 0, z: 0 }));
    for tick in 1..=25u64 {
        flora.step(&mut world);
        let g = flora.view().ground_at(fed).expect("ground");
        assert_eq!(g.seeds.len(), 1, "an unfed bank split: {:?}", g.seeds);
        assert_eq!(g.seeds[0].age_ticks, tick, "an unfed bank did not age: {:?}", g.seeds);
    }
    let c = flora.view().ground_at(fed).unwrap().seeds[0];
    assert!(c.organic < organic_fed, "attrition took nothing: {} -> {}", organic_fed, c.organic);
    assert!(flora.view().ground_at(fed).unwrap().litter > 0.0, "attrition is deletion, not decay");
    assert_residuals(&flora, "after a bank was fed and then abandoned");
}

/// The adversarial fixture the merge rule's own comment invites: a donor that comes and
/// goes on a period of more than two ticks. Ages two apart never merge, and once two
/// cohorts are two ticks apart they stay two ticks apart forever, so the bank grows by
/// one cohort per pulse and the merge bounds nothing.
///
/// Fifty pulses two ticks apart give fifty cohorts on one site. The only ceiling is
/// `seed_max_age_s`: at 600 s and `DT` = 0.05 a cohort lives 12,000 ticks, so one site can
/// hold about **6,000** cohorts of one species — a `Vec` the germination check sums over
/// every tick. `Ground::seeds`' own doc says "at most a few cohorts per site".
///
/// `bloomcrown.establish_light_min` is 2.0 here (placeholder 0.6), a predicate that can
/// never pass, so the bank can only accumulate and germination cannot end the experiment
/// early. The pulsing itself is `Seed` and `Clear`, which is what "a donor that flickers
/// across its reserve floor" looks like from the recipient site.
#[test]
fn a_pulsing_donor_stacks_one_cohort_per_pulse_and_the_merge_bounds_nothing() {
    let mut config = FloraConfig::default();
    config.bloomcrown.hop = 1;
    config.bloomcrown.establish_light_min = 2.0;
    let mut world = strip(3, 1, 0.6);
    let mut flora = Flora::new(config);

    let target = site(1);
    let pulses = 50;
    for _ in 0..pulses {
        assert!(flora.apply(&world, Command::Seed { x: 0, z: 0, species: Species::Bloomcrown, wood: 0.6 }));
        flora.step(&mut world);
        assert!(flora.apply(&world, Command::Clear { x: 0, z: 0 }));
        flora.step(&mut world);
    }

    let g = flora.view().ground_at(target).expect("ground");
    assert_eq!(
        g.seeds.len(),
        pulses,
        "{} cohorts after {pulses} pulses two ticks apart",
        g.seeds.len()
    );
    // Sorted by age, youngest first, and every gap is exactly the pulse period.
    for (i, c) in g.seeds.iter().enumerate() {
        assert_eq!(c.species, Species::Bloomcrown);
        assert_eq!(c.age_ticks, 1 + 2 * i as u64, "cohort {i}: {c:?}");
    }
    assert!(flora.view().stand_at(target).is_none(), "the predicate cannot pass here");
    assert_eq!(flora.view().ledger.establishments, 0);
    assert_residuals(&flora, "after fifty pulses");
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
    let config =
        VoxelConfig { width: 5, height: 8, depth: 3, voxel_m: 1.0, seed: 5, ..VoxelConfig::default() };
    let mut w = World::empty(config);
    for z in 0..3u32 {
        for x in 0..5i64 {
            for y in 1..=2u32 {
                let wet =
                    order.iter().position(|&t| t == (x, y, z)).is_some_and(|i| i < saturated);
                fill(&mut w, x, y, z, Material::Soil, if wet { 0.98 } else { 0.5 });
            }
        }
    }
    w
}

/// The shape of the aeration response, which the brief and the commit message both state
/// only at its two endpoints. The update is `stress += rate·dt·f − relax·dt·(1−f)`: an
/// increment that does not depend on `stress` at all. So it is a **ramp to a boundary**,
/// with no interior fixed point except at the single saturated fraction
/// `f* = relax / (rate + relax)`, and the species' response to its root zone is a step
/// function of `f` and not a gradient.
///
/// For bloomcrown `f* = 0.02 / 0.22 = 0.0909`. On an 18-voxel root box that sits between
/// one saturated voxel (1/18 = 0.0556) and two (2/18 = 0.1111), so **two wet voxels out of
/// eighteen** are the difference between a stand that relaxes to exactly 0 and a stand
/// that is pinned at exactly 1. No rate here is the test's own.
#[test]
fn aeration_stress_is_a_step_in_the_saturated_fraction_not_a_gradient() {
    let sc = SpeciesConfig::bloomcrown();
    let f_star = sc.relax_rate_per_s / (sc.stress_rate_per_s + sc.relax_rate_per_s);
    assert!(
        (f_star - 0.02 / 0.22).abs() < 1e-12,
        "the placeholders this test reads: f* = {f_star}"
    );
    assert!(1.0 / 18.0 < f_star && f_star < 2.0 / 18.0, "f* {f_star} is not between 1/18 and 2/18");

    // Start wholly saturated: the stress ramps to the ceiling.
    let mut world = box_world(18);
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.apply(&world, Command::Seed { x: 2, z: 1, species: Species::Bloomcrown, wood: 0.1 }));
    let here = at(2, 1);
    run(&mut flora, &mut world, 400);
    assert_eq!(
        flora.view().stand_at(here).unwrap().aeration_stress,
        1.0,
        "a wholly saturated box did not pin the stress at the ceiling"
    );
    assert_eq!(flora.view().stand_at(here).unwrap().moisture, 1.0, "μ is not what stopped it");

    // Dry sixteen of the eighteen: 2/18 = 0.1111 is still over f*, so the ceiling holds.
    let order = root_box_order();
    for &(x, y, z) in order.iter().take(16) {
        drain_to(&mut world, x, y, z, 0.5);
    }
    run(&mut flora, &mut world, 400);
    let s = flora.view().stand_at(here).unwrap();
    assert_eq!(
        s.aeration_stress, 1.0,
        "2 of 18 saturated voxels (f = {}) did not hold the ceiling",
        2.0 / 18.0
    );
    assert_eq!(s.moisture, 1.0, "the box is still wet enough for μ = 1");

    // Dry one more: 1/18 = 0.0556 is under f*, and the stress falls all the way to the
    // floor. `(f* − 1/18) · (rate + relax) · dt` per tick is 3.9e-4, so 2,572 ticks.
    let (x, y, z) = order[16];
    drain_to(&mut world, x, y, z, 0.5);
    run(&mut flora, &mut world, 2600);
    let s = flora.view().stand_at(here).unwrap();
    assert_eq!(
        s.aeration_stress, 0.0,
        "1 of 18 saturated voxels (f = {}) did not relax to the floor",
        1.0 / 18.0
    );
    assert_residuals(&flora, "after a root box crossed f* twice");
}

/// **FINDING.** The brief and package H's report both describe aeration stress as a level
/// a root box settles at — "it relaxes after the water table drops", "stays under its
/// stress ceiling". A box held at a fixed intermediate saturation has no such level: the
/// per-tick increment `rate·dt·f − relax·dt·(1−f)` is independent of `stress`, so every
/// fraction other than the single knife-edge `f* = relax / (rate + relax)` ramps to 0 or
/// to 1 and stays there. Half a saturated root box is not half a stress.
///
/// Measured on a nine-of-eighteen box (f = 0.5) held there for 20,000 ticks
/// (1,000 s), with `μ = 1` and light 1 throughout:
///   bloomcrown (rate 0.2 /s, relax 0.02 /s, f* = 0.0909): stress 1.0, income 0.
///   umbrellafrond (rate 0.01 /s, relax 0.05 /s, f* = 0.8333): stress 0.0, income full.
/// Neither is strictly inside `0..1` at any fraction but its own `f*`, where the
/// increment is zero and the stress simply never moves from wherever it started.
///
/// This is the specified update rule, not a mistake in implementing it, so nothing here is
/// changed to make the test pass. What it costs the model: the two species do not grade
/// into one another across the moisture gradient the terrain provides — each one either
/// earns everything or nothing, and the crossover sits at a fraction fixed by a rate ratio
/// nobody has measured. The backlog row calls the 20x ratio "the number most likely to be
/// wrong"; this is why it matters more than a factor of twenty usually would.
#[test]
#[ignore = "FINDING: aeration stress has no interior equilibrium. On a 9/18 saturated box \
            held for 20,000 ticks, bloomcrown reaches exactly 1.0 and umbrellafrond \
            exactly 0.0; the increment rate*dt*f - relax*dt*(1-f) does not depend on \
            stress, so every f except f* = relax/(rate+relax) (0.0909 bloomcrown, 0.8333 \
            umbrellafrond) ramps to a boundary. Specified rule, not an implementation \
            slip; left failing on purpose."]
fn a_half_saturated_root_box_settles_at_an_interior_stress() {
    for (species, x) in [(Species::Bloomcrown, 2i64), (Species::Umbrellafrond, 2)] {
        let mut world = box_world(9);
        let mut flora = Flora::new(FloraConfig::default());
        assert!(flora.apply(&world, Command::Seed { x, z: 1, species, wood: 0.1 }));
        run(&mut flora, &mut world, 20_000);
        let s = *flora.view().stand_at(at(2, 1)).expect("alive");
        assert!(
            s.aeration_stress > 0.0 && s.aeration_stress < 1.0,
            "{} on a half-saturated box sits at stress {}, not strictly inside 0..1",
            species.name(),
            s.aeration_stress
        );
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
        assert!(f.apply(w, Command::Seed { x: 2, z: 1, species: Species::Bloomcrown, wood: 0.1 }));
    }

    a.step(&mut a_world);
    b.step(&mut b_world);

    let sa = *a.view().stand_at(at(2, 1)).unwrap();
    let sb = *b.view().stand_at(at(2, 1)).unwrap();
    assert_eq!(sb.aeration_stress, 0.0, "the control arm stressed");
    assert!(sa.aeration_stress > 0.0, "the stressed arm did not stress");
    assert_eq!(sa.light, sb.light, "the two arms saw different light");
    assert_eq!(sa.moisture, sb.moisture, "the two arms saw different moisture");

    let (fa, fb) = (a.view().ledger.fixed_in, b.view().ledger.fixed_in);
    assert!(fb > 0.0, "the control fixed nothing: the demand cap is still binding");
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
/// (placeholder 2e-4) and `reserve_cap` 4.0 (placeholder 0.5) so one donor's spendable
/// reserve clears the germination threshold on all nine of its `hop`-1 recipients in one
/// tick, and `hop` is 1 for bloomcrown (placeholder 2) so it reaches only its immediate
/// neighbours. The saturation is the fixture's.
#[test]
fn the_saturation_ceiling_is_non_strict_and_umbrellafrond_s_is_inert() {
    let mut config = FloraConfig::default();
    config.bloomcrown.propagule_rate = 3.0;
    config.bloomcrown.hop = 1;
    config.bloomcrown.reserve_cap = 4.0;
    config.umbrellafrond.propagule_rate = 3.0;
    config.umbrellafrond.reserve_cap = 4.0;
    assert_eq!(config.bloomcrown.establish_saturated_max, 0.25);
    assert_eq!(config.umbrellafrond.establish_saturated_max, 1.0);

    for (saturated, bloom_should) in [(4usize, true), (5, false), (18, false)] {
        let mut world = box_world(saturated);
        let mut flora = Flora::new(config.clone());
        // Two donors either side of the box's middle column, one of each species, so both
        // banks land on the same site and only the ceiling can separate them.
        assert!(flora.apply(&world, Command::Seed { x: 1, z: 1, species: Species::Bloomcrown, wood: 0.6 }));
        // One tick to bank, and the threshold is read *before* germination could spend it.
        flora.step(&mut world);
        let target = at(2, 1);
        let g = flora.view().ground_at(target).expect("ground");
        let bank = g.seed_organic(Species::Bloomcrown);
        let sc = flora.config().species(Species::Bloomcrown);
        assert!(
            sc.propagule_split[0] * bank >= sc.alive_min,
            "{saturated}/18: the bank is not over the threshold: {bank}"
        );
        // Ten more: germination happens on the next one if it is going to happen at all.
        run(&mut flora, &mut world, 10);
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
    assert!(flora.apply(&world, Command::Seed { x: 1, z: 1, species: Species::Umbrellafrond, wood: 0.6 }));
    flora.step(&mut world);
    let sc = SpeciesConfig::umbrellafrond();
    let bank = flora.view().ground_at(at(2, 1)).expect("ground").seed_organic(Species::Umbrellafrond);
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
/// `mean_pore`'s capacity weighting and `saturated_fraction`'s deliberate lack of it can
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
    assert!(Material::Rock.pore_capacity() > 0.0, "the premise: rock does hold pore water");
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
    let mut flora = Flora::new(cfg);
    assert!(flora.apply(&world, Command::Seed { x: 0, z: 0, species: Species::Bloomcrown, wood: 0.6 }));
    // A second stand rooted wholly in the saturated rock.
    assert!(flora.apply(&world, Command::Seed { x: 3, z: 0, species: Species::Bloomcrown, wood: 0.6 }));

    run(&mut flora, &mut world, 200);

    let on_rock = *flora.view().stand_at(site(3)).expect("alive, just thirsty");
    assert_eq!(on_rock.moisture, 0.0, "saturated rock watered a plant");
    assert_eq!(
        on_rock.aeration_stress, 0.0,
        "saturated rock drowned a root zone: an empty box is not waterlogged"
    );
    assert_eq!(flora.view().ledger.deaths, 0, "nothing was supposed to die in 200 ticks");

    // x2 and x4 root wholly in rock. Their banks are over the threshold and they never
    // germinate; x1, whose box reaches the one soil column, is the control that does.
    let sc = flora.config().species(Species::Bloomcrown);
    for x in [2u32, 4] {
        let g = flora.view().ground_at(site(x)).unwrap_or_else(|| panic!("nothing banked at {x}"));
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
    assert_residuals(&flora, "after 200 ticks over saturated rock");
}

/// **FINDING.** Umbrellafrond's tolerance of a saturated root zone is a *delay*, not a
/// tolerance. Its `f*` is 0.8333, so a root box saturated to the last voxel — which is
/// exactly what the basin the species is for looks like — is over its threshold too, and
/// it ramps to stress 1 and earns nothing, 20 times more slowly than bloomcrown but just
/// as completely. `establish_saturated_max` of 1.0 lets it germinate there first.
///
/// Measured on a wholly saturated 18-voxel box with `μ = 1` and light 1 throughout:
/// stress 0.5 at tick 1,000 (50 s), exactly 1.0 by tick 2,000 (100 s), and from there not
/// one further unit of organic matter fixed. Bloomcrown reaches the same place at tick
/// 100 (5 s). Correction 2 was meant to make the basin cost bloomcrown something and cost
/// umbrellafrond nothing; at full saturation it costs them both everything, on two
/// different clocks.
///
/// H's own aeration test reads umbrellafrond's stress after 150 ticks and finds it under
/// 0.1, which is true — and is 1/13th of the way along this ramp.
#[test]
fn umbrellafrond_also_drowns_in_a_wholly_saturated_root_box_only_twenty_times_slower() {
    let sc = SpeciesConfig::umbrellafrond();
    let f_star = sc.relax_rate_per_s / (sc.stress_rate_per_s + sc.relax_rate_per_s);
    assert!((f_star - 0.05 / 0.06).abs() < 1e-12, "the placeholders this test reads: f* = {f_star}");
    assert!(f_star < 1.0, "a wholly saturated box is over even umbrellafrond's threshold");

    let mut world = box_world(18);
    let mut flora = Flora::new(FloraConfig::default());
    let here = at(2, 1);
    assert!(flora.apply(&world, Command::Seed { x: 2, z: 1, species: Species::Umbrellafrond, wood: 0.1 }));

    // H's own reading point: after 150 ticks it looks tolerant.
    run(&mut flora, &mut world, 150);
    let early = *flora.view().stand_at(here).expect("alive");
    assert!(early.aeration_stress < 0.1, "at 150 ticks it is only {}", early.aeration_stress);
    assert_eq!(early.moisture, 1.0, "μ is not what is happening here");

    // Half way, at 1,000 ticks.
    run(&mut flora, &mut world, 850);
    let half = flora.view().stand_at(here).expect("alive").aeration_stress;
    assert!((half - 0.5).abs() < 0.01, "at 1,000 ticks the stress is {half}, not about 0.5");

    // And pinned at the ceiling by 2,000.
    run(&mut flora, &mut world, 1050);
    let s = *flora.view().stand_at(here).expect("still alive, just earning nothing");
    assert_eq!(s.aeration_stress, 1.0, "at 2,000 ticks the stress is {}", s.aeration_stress);
    assert_eq!(s.light, 1.0, "open sky");
    assert_eq!(s.moisture, 1.0, "wetter than sat_pore: water is not the limit");

    let fixed = flora.view().ledger.fixed_in;
    run(&mut flora, &mut world, 200);
    assert_eq!(
        flora.view().ledger.fixed_in, fixed,
        "a fully stressed umbrellafrond went on fixing light"
    );
    assert_eq!(
        flora.view().stand_at(here).unwrap().wood,
        s.wood,
        "and it went on growing"
    );
    assert_residuals(&flora, "after an umbrellafrond drowned in its own habitat");
}

/// The same finding on the fixture H's own aeration test uses, so it cannot be an artifact
/// of hand-set pore fractions: a water table charged above the support face, the world
/// stepped every tick, and no rain. At 150 coupled ticks umbrellafrond's stress is under
/// 0.1; at 2,100 it is exactly 1.0 and it has stopped earning.
#[test]
fn the_same_holds_on_a_water_table_basin_with_the_world_stepping() {
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
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.apply(&world, Command::Seed { x: 2, z: 0, species: Species::Umbrellafrond, wood: 0.1 }));

    let coupled = |flora: &mut Flora, world: &mut World, ticks: u32| {
        for _ in 0..ticks {
            world.step();
            flora.step(world);
        }
    };
    coupled(&mut flora, &mut world, 150);
    let early = flora.view().stand_at(site(2)).expect("alive").aeration_stress;
    assert!(early < 0.1, "at 150 coupled ticks it is {early}, which is where H reads it");

    coupled(&mut flora, &mut world, 1950);
    let s = *flora.view().stand_at(site(2)).expect("alive, earning nothing");
    assert_eq!(world.view().water_depth_m(2, 2, 0), 0.0, "this is wet soil, not a pool");
    assert_eq!(flora.view().ledger.deaths, 0, "drowning by depth is not what this is");
    assert_eq!(s.moisture, 1.0, "water is not the limit");
    assert_eq!(s.aeration_stress, 1.0, "at 2,100 coupled ticks the stress is {}", s.aeration_stress);
    let fixed = flora.view().ledger.fixed_in;
    coupled(&mut flora, &mut world, 100);
    assert_eq!(flora.view().ledger.fixed_in, fixed, "it went on fixing light");
    assert_residuals(&flora, "after an umbrellafrond drowned on a water-table basin");
}
