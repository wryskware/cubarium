//! Round 5b: the **glowcap**, a saprotroph stand — what it earns, where it may start, and
//! what it does to a log.
//!
//! Short function tests of the boundaries the round-5b brief states, and nothing else. No
//! population claim, no carrying capacity, no viability: the `community` run in the
//! experiment note is the only coupled observation, and it is an observation.
//!
//! Conventions are `round5a.rs`'s, so a fixture here reads the same way: `voxel_m` is 1 m,
//! so a soil voxel holds `Material::Soil.pore_capacity()` = 0.35 m³ of pore water; soil is
//! wetted by adding free water to an air cell and turning the cell to soil; and the world
//! is **never stepped**, so the only thing that moves water is the plant layer's own
//! bounded withdrawal and a fixture's pore fraction is the condition it says it is.
//!
//! A **log** is a `DepositKind::DeadWood` deposit carrying the mineral and the energy a
//! dead trunk holds — `n_tissue · organic` and `e_v · organic` — because a log laid without
//! its energy is a log with nothing in it to eat (`DepositKind::DeadWood`'s own doc), and a
//! fixture that wants to say something about income must not accidentally say that.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command, Deposit, DepositKind, Flora, FloraConfig, Site, Species, SpeciesConfig, Trophic, DT,
};

// ------------------------------------------------------------------- fixtures

/// `round5a.rs`'s `fill`: turn one air voxel into `material` holding exactly `pore` of that
/// material's own pore capacity, by adding the water first and converting after.
fn fill(w: &mut World, x: i64, y: u32, z: u32, material: Material, pore: f64) {
    let want = pore * material.pore_capacity() * w.config().voxel_volume();
    if want > 0.0 {
        let got = w.apply(WorldCommand::AddWater { x, y, z, volume_m3: want });
        assert!((got - want).abs() < 1e-12, "the void took {got} of {want}");
    }
    w.apply(WorldCommand::SetMaterial { x, y, z, material });
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

/// `round5a.rs`'s strip: only the columns of `keep` are solid at all — bedrock at `y = 0`,
/// soil at `y = 1..=2` holding `pore` of soil's capacity — so every kept column's support
/// face is `y = 2` in open sky and every other column has none.
fn pillars(width: u32, keep: &[i64], pore: f64) -> World {
    let mut w = empty_world(width, 1);
    for x in 0..width as i64 {
        if keep.contains(&x) {
            for y in 1..=2 {
                fill(&mut w, x, y, 0, Material::Soil, pore);
            }
        } else {
            w.apply(WorldCommand::SetMaterial { x, y: 0, z: 0, material: Material::Air });
            assert!(
                cubarium_voxel_flora::highest_support(&w.view(), x, 0).is_none(),
                "({x},0) must be void"
            );
        }
    }
    w
}

/// A **roofed** strip: soil at `y = 1..=2` over the whole footprint, so every column's
/// support face is `y = 2`, with a rock ceiling two cells above it. The world is five deep
/// on purpose — in a one-deep strip every ray that leans in `z` leaves the world at once
/// and counts as escaping, so a one-deep world cannot be made dark at all, which is worth
/// knowing before writing a shade fixture.
fn roofed(width: u32, depth: u32) -> World {
    let mut w = empty_world(width, depth);
    for z in 0..depth {
        for x in 0..width as i64 {
            for y in 1..=2 {
                fill(&mut w, x, y, z, Material::Soil, 0.5);
            }
            w.apply(WorldCommand::SetMaterial { x, y: 4, z, material: Material::Rock });
        }
    }
    w
}

/// The support face of a kept column of [`pillars`].
fn at(x: u32) -> Site {
    Site { x, y: 2, z: 0 }
}

fn run(flora: &mut Flora, world: &mut World, ticks: u32) {
    for _ in 0..ticks {
        flora.step(world);
    }
}

/// The three residuals, as `round3.rs`, `round4.rs` and `round5a.rs` compute them. A
/// saprotroph's uptake is an internal transfer and adds no term to any of the three, which
/// is the whole reason these still close with a fungus digesting.
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

/// A config whose glowcap preset is the shipped one unless a test says otherwise, with the
/// five plants left exactly as they ship.
fn config() -> FloraConfig {
    FloraConfig::default()
}

/// Lay a declared log on `site`: `organic` units of dead wood with the mineral and the
/// energy a dead trunk of that size holds, through 5b's new `DepositKind::DeadWood`.
/// Returns the three numbers it deposited.
fn log_on(flora: &mut Flora, site: Site, organic: f64) -> (f64, f64, f64) {
    let sc = flora.config().species(Species::Glowcap).clone();
    let (mineral, energy) = (sc.n_tissue * organic, sc.energy_density * organic);
    assert!(
        flora.deposit(site, Deposit { kind: DepositKind::DeadWood, organic, mineral, energy }),
        "the log was refused at {site:?}"
    );
    (organic, mineral, energy)
}

/// A glowcap founder on a kept column, at half its own `wood_max` — the harness's founder
/// size, and exactly `donor_min` for all six presets.
fn plant_glowcap(flora: &mut Flora, world: &World, x: i64) -> Site {
    let wood = 0.5 * flora.config().species(Species::Glowcap).wood_max;
    assert!(flora.apply(world, Command::Seed { x, z: 0, species: Species::Glowcap, wood }));
    let site = cubarium_voxel_flora::highest_support(&world.view(), x, 0).expect("a support face");
    assert_eq!(flora.view().stand_at(site).expect("a founder").species, Species::Glowcap);
    site
}

fn dead_wood_at(flora: &Flora, site: Site) -> f64 {
    flora.view().ground_at(site).map_or(0.0, |g| g.dead_wood)
}

fn uptake(flora: &Flora) -> f64 {
    flora.view().ledger.substrate_uptake[Species::Glowcap.index()]
}

// ------------------------------------------------------------------- the income

/// **A log is the income, and light is not.** Two glowcap founders in one world under one
/// open sky, identical but for the log under one of them: the one on the log takes wood,
/// keeps `substrate_yield` of it and grows, and the one on a bare face takes nothing, fixes
/// nothing — `fixed_in` stays at zero for the whole world, because a saprotroph is not on
/// the light boundary at all — and burns its reserve.
#[test]
fn a_glowcap_on_a_log_earns_and_one_on_a_bare_face_earns_nothing() {
    let mut world = pillars(6, &[1, 4], 0.5);
    let mut flora = Flora::new(config());
    let sc = flora.config().species(Species::Glowcap).clone();
    assert_eq!(sc.trophic, Trophic::Saprotroph, "the preset is the thing being tested");
    assert_eq!(sc.assimilation, 0.0, "a saprotroph fixes nothing from light");

    let fed = plant_glowcap(&mut flora, &world, 1);
    let starved = plant_glowcap(&mut flora, &world, 4);
    let (organic, _, _) = log_on(&mut flora, fed, 1.0);
    let fixed0 = flora.view().ledger.fixed_in;
    let (w0, q0) = {
        let s = flora.view().stand_at(starved).expect("planted");
        (s.wood, s.reserve)
    };

    run(&mut flora, &mut world, 50);

    let v = flora.view();
    let f = *v.stand_at(fed).expect("the fed stand died");
    let s = *v.stand_at(starved).expect("the starved stand died in 1 s");
    // The fed one took wood out of the log and grew on it.
    let gone = organic - dead_wood_at(&flora, fed);
    assert!(uptake(&flora) > 0.0, "nothing left the log for the fungus");
    // The log also decomposes on its own, so what left it is the uptake **and** the
    // `wood_decomposition` of the same 50 ticks: the fungus's share is the smaller.
    assert!(gone > uptake(&flora), "{gone} left the log against {} eaten", uptake(&flora));
    assert!(f.wood > w0, "the fed fungus did not grow: {} against {w0}", f.wood);
    assert!(f.light > 0.0 && f.moisture > 0.0, "the fixture is lit and damp: {f:?}");

    // The starved one earns nothing at all under the same sky, and pays maintenance out of
    // its reserve.
    assert_eq!(dead_wood_at(&flora, starved), 0.0, "the bare face has no log");
    assert!(s.wood <= w0, "the starved fungus grew on nothing: {} against {w0}", s.wood);
    assert!(s.reserve < q0, "the starved fungus paid nothing: {} against {q0}", s.reserve);
    assert!(s.light > 0.0, "the starved stand is in open sky and still earns nothing: {s:?}");

    // And the light boundary never moved for either of them.
    assert_eq!(v.ledger.fixed_in, fixed0, "a saprotroph fixed organic matter from light");
    assert_eq!(v.ledger.light_in, 0.0, "a saprotroph took energy off the light boundary");
    assert_residuals(&flora, "after 1 s of one fed and one starved fungus");
}

/// **The uptake is bounded by the pool, and stops when it is empty.** The rate asks for
/// 2e-5 of organic matter on this fixture's first tick and the log holds half of that: the
/// fungus takes the log, exactly, and the pool's mineral and energy go with the last of it
/// rather than leaving float dust behind claiming to be a stock. Every tick after that
/// takes nothing.
#[test]
fn the_uptake_never_exceeds_the_pool_and_stops_when_it_is_empty() {
    let mut world = pillars(4, &[1], 0.5);
    let mut flora = Flora::new(config());
    let site = plant_glowcap(&mut flora, &world, 1);
    let sc = flora.config().species(Species::Glowcap).clone();

    // What the rate alone would ask for on the first tick, at full moisture.
    let want = sc.substrate_uptake_per_s * 0.5 * sc.wood_max * DT;
    assert!((want - 5e-5).abs() < 1e-18, "the fixture's premise: {want}");
    let (organic, mineral, energy) = log_on(&mut flora, site, 0.5 * want);
    assert!(organic < want, "the log has to be smaller than one tick's demand");

    flora.step(&mut world);
    assert!((uptake(&flora) - organic).abs() <= 1e-18, "took {} of {organic}", uptake(&flora));
    let g = flora.view().ground_at(site).expect("ground").clone();
    assert_eq!(g.dead_wood, 0.0, "the pool went negative or kept dust: {}", g.dead_wood);
    assert_eq!(g.dead_wood_mineral, 0.0, "mineral left behind on an empty pool");
    assert_eq!(g.dead_wood_energy, 0.0, "energy left behind on an empty pool");
    // The mineral that came with the wood is in the fungus and in the site's pool, and
    // nowhere else: `arrived` over what the tissue needed is released to the soil.
    let v = flora.view();
    let stand = *v.stand_at(site).expect("alive");
    let pool = v.ground_at(site).expect("ground").mineral;
    assert!(stand.mineral > 0.0 && pool > 0.0, "{stand:?} {pool}");

    let after_one = uptake(&flora);
    run(&mut flora, &mut world, 20);
    assert_eq!(uptake(&flora), after_one, "an empty log went on feeding the fungus");
    assert_eq!(dead_wood_at(&flora, site), 0.0);
    assert_residuals(&flora, "after a log was eaten whole");
    assert!(mineral > 0.0 && energy > 0.0, "the fixture laid a log with substance in it");
}

/// **Mineral is closed while a fungus digests.** It only ever moves between stocks — out of
/// the log with the wood, into the mycelium, released to the soil where the tissue does not
/// need it, shed to litter with the caps, and handed back to the soil as the litter
/// decomposes — so the *total* cannot move at all once the setup is done, and the three
/// residuals hold.
#[test]
fn mineral_runs_from_wood_through_the_fungus_to_litter_and_the_site_pool_and_is_conserved() {
    let mut world = pillars(4, &[1], 0.5);
    let mut flora = Flora::new(config());
    let site = plant_glowcap(&mut flora, &world, 1);
    let (_, log_mineral, _) = log_on(&mut flora, site, 0.2);
    assert!(log_mineral > 0.0);

    let total0 = flora.view().mineral();
    let expected0 = flora.view().ledger.expected_mineral();
    assert_residuals(&flora, "before the fungus started");

    run(&mut flora, &mut world, 200);

    let v = flora.view();
    let g = v.ground_at(site).expect("ground").clone();
    let stand = *v.stand_at(site).expect("the fungus died");
    // Every link of the chain carries some of it.
    assert!(g.dead_wood_mineral < log_mineral, "none left the log: {}", g.dead_wood_mineral);
    assert!(stand.mineral > 0.0, "the mycelium holds none");
    assert!(g.litter_mineral > 0.0, "the caps shed none to litter");
    assert!(g.mineral > 0.0, "nothing reached the soil");
    // And the total is exactly where it was: no boundary flow touched it.
    assert!(
        (v.mineral() - total0).abs() <= 1e-12,
        "total mineral moved from {total0} to {}",
        v.mineral()
    );
    assert_eq!(v.ledger.expected_mineral(), expected0, "a mineral boundary flow was booked");
    assert_residuals(&flora, "after 4 s of digestion");
}

/// **A small finite log runs out, and the fungus starts losing.** Direction only, and no
/// death claim: over 200 ticks the pool never rises, the uptake per window falls to exactly
/// nothing, and once it does the reserve falls every window.
#[test]
fn a_small_finite_log_empties_and_then_the_reserve_falls() {
    let mut world = pillars(4, &[1], 0.5);
    let mut flora = Flora::new(config());
    let site = plant_glowcap(&mut flora, &world, 1);
    log_on(&mut flora, site, 0.002);

    let mut windows: Vec<(f64, f64, f64)> = Vec::new();
    let mut last_uptake = 0.0;
    for _ in 0..4 {
        run(&mut flora, &mut world, 50);
        let pool = dead_wood_at(&flora, site);
        let gained = uptake(&flora) - last_uptake;
        last_uptake = uptake(&flora);
        let reserve = flora.view().stand_at(site).map_or(f64::NAN, |s| s.reserve);
        windows.push((pool, gained, reserve));
    }
    for w in windows.windows(2) {
        assert!(w[1].0 <= w[0].0, "the pool rose: {:?}", windows);
        assert!(w[1].1 <= w[0].1, "the uptake rose: {:?}", windows);
    }
    assert_eq!(windows[3].0, 0.0, "the log is not empty at 4 s: {windows:?}");
    assert_eq!(windows[3].1, 0.0, "the empty log still fed it: {windows:?}");
    assert!(windows[3].2 < windows[2].2, "the reserve did not fall: {windows:?}");
    assert_residuals(&flora, "after a small log ran out");
}

// -------------------------------------------------------------------- the gates

/// **The substrate gate, and no light gate.** One world, two identical faces, a log under
/// one of them: the predicate the model itself runs — `FloraView::can_establish`, which
/// reads the dead wood off this layer's own ground — passes on the face with the log and
/// refuses the other, and says which gate shut. The light gate is **open** on both,
/// however dark, because a saprotroph does not eat light; the same faces refuse springturf
/// on light alone once they are shaded.
#[test]
fn germination_needs_a_log_and_never_needs_light() {
    let mut world = pillars(6, &[1, 4], 0.5);
    let mut flora = Flora::new(config());
    // A log on one face only. `deposit` provisions the `Ground` itself, so nothing else
    // about the two faces differs.
    log_on(&mut flora, at(1), 1.0);

    let view = world.view();
    let with_log = flora.view().establishment_gates(&view, at(1), Species::Glowcap);
    let without = flora.view().establishment_gates(&view, at(4), Species::Glowcap);
    assert!(with_log.passes() && flora.view().can_establish(&view, at(1), Species::Glowcap));
    assert!(!without.passes(), "a face with no log admitted a spore: {without:?}");
    assert_eq!(
        (with_log.pore_ok, with_log.aeration_ok, with_log.depth_ok, with_log.light_ok),
        (without.pore_ok, without.aeration_ok, without.depth_ok, without.light_ok),
        "the two faces differ in something other than the substrate"
    );
    assert!(with_log.substrate_ok && !without.substrate_ok, "{with_log:?} {without:?}");
    assert!((with_log.dead_wood - 1.0).abs() < 1e-15 && without.dead_wood == 0.0);
    // The light gate is open for a saprotroph whatever the sky says, and the free function
    // that cannot see a log refuses both faces — which is what it is documented to do.
    assert!(with_log.light_ok && without.light_ok);
    let sc = flora.config().species(Species::Glowcap);
    assert!(!cubarium_voxel_flora::can_establish(&view, at(1), sc), "no substrate, no pass");

    // And in the dark: a roofed site where every plant's light gate shuts — springturf's
    // and even umbrellafrond's, the shade-tolerant one — and the glowcap's does not
    // notice, because terrain geometry is the only thing germination light reads and a
    // saprotroph does not read it at all.
    let dark = roofed(6, 5);
    let mut under = Flora::new(config());
    let cellar = Site { x: 1, y: 2, z: 2 };
    log_on(&mut under, cellar, 1.0);
    let view = dark.view();
    let turf = cubarium_voxel_flora::establishment_gates(&view, cellar, &config().springturf);
    let frond = cubarium_voxel_flora::establishment_gates(&view, cellar, &config().umbrellafrond);
    let glow = under.view().establishment_gates(&view, cellar, Species::Glowcap);
    assert_eq!(glow.sky_visibility, turf.sky_visibility, "one site, one sky");
    assert_eq!(turf.sky_visibility, 0.0, "the roof leaks: {turf:?}");
    assert!(!turf.light_ok && !turf.passes(), "springturf started in the dark: {turf:?}");
    assert!(!frond.light_ok, "the shade-tolerant plant still wants some sky: {frond:?}");
    assert!(glow.light_ok && glow.passes(), "the dark shut the fungus out: {glow:?}");
}

/// **One spore package, one birth, at exactly `alive_min`.** A funded glowcap donor on a
/// log saves one whole package, sends it to the only support face inside its `hop`, and the
/// recipient — whose own mycelium box holds the donor's log, which is how a grove spreads
/// along a log — germinates a stand whose wood is `alive_min` on the nose and whose
/// material is the package.
///
/// The refusal arm is the **threshold** and not the geography, and that is worth saying:
/// with `hop` 1 and `rooting_radius` 1 a recipient's box always overlaps its donor's, so
/// "a package landing out of reach of any log" cannot be arranged on this fixture at the
/// placeholders. What bounds a grove is the log, not the hop.
#[test]
fn one_spore_package_births_a_glowcap_at_exactly_alive_min_and_only_over_the_substrate() {
    let birth = |substrate_min: f64| -> (Flora, World, f64) {
        let mut world = pillars(4, &[1, 2], 0.5);
        let mut config = config();
        // A fast donor, so the 0.025 package is saved in tens of ticks rather than 60 s.
        // It changes the saving rate only: the reserve, its floor and every other stock
        // are the preset's.
        config.glowcap.propagule_rate = 3.0;
        // And one that will spend its **whole** reserve on the parcel rather than keeping
        // half of it, so the package is away in a tick or two instead of in 60 s. Both
        // knobs are the saving rule and nothing else: `reserve_cap`, the income, the
        // stocks and every threshold are the preset's.
        config.glowcap.donor_reserve_floor = 0.0;
        config.glowcap.establish_substrate_min = substrate_min;
        let package = config.glowcap.alive_min / config.glowcap.propagule_split[0];
        let mut flora = Flora::new(config);
        // A **full-grown** donor, so that one tick's whole reserve `reserve_cap · wood_max`
        // is over one package and the parcel fills at once.
        let wood = flora.config().species(Species::Glowcap).wood_max;
        assert!(flora.apply(&world, Command::Seed { x: 1, z: 0, species: Species::Glowcap, wood }));
        log_on(&mut flora, at(1), 1.0);
        // Stop on the **birth tick**: the newborn's wood is `alive_min` exactly at the end
        // of the tick it is born in, because growth (phase 6) runs before the seed bank
        // (phase 8), and it grows from the next tick on. Ten ticks is a bound and not a
        // condition — the package lands on tick 1 and germinates on tick 2.
        for _ in 0..10 {
            flora.step(&mut world);
            if flora.view().ledger.establishments > 0 {
                break;
            }
        }
        (flora, world, package)
    };

    // The shipped threshold: the recipient's box holds the donor's whole log.
    let (flora, _world, package) = birth(config().glowcap.establish_substrate_min);
    let v = flora.view();
    assert_eq!(v.ledger.establishments, 1, "no birth in 1 s: {:?}", v.ledger);
    let born = *v.stand_at(at(2)).expect("nothing stands on the only landing site");
    assert_eq!(born.species, Species::Glowcap);
    assert_eq!(born.wood, config().glowcap.alive_min, "born with {} of wood", born.wood);
    assert!(
        (born.organic() - package).abs() <= 1e-16,
        "born holding {} of a {package} package",
        born.organic()
    );
    assert!(born.foliage > 0.0 && born.reserve > 0.0 && born.mineral > 0.0, "{born:?}");
    assert!(v.ledger.propagule_landed[Species::Glowcap.index()] > 0.0);
    assert_residuals(&flora, "after a paid glowcap birth");
    // The only face inside `hop` 1 is the one it landed on, so the landing is geometry and
    // not a lucky draw.

    // The same run with a threshold the log cannot meet: the bank lands, waits, and never
    // germinates. Nothing else differs.
    let (refused, _world, _) = birth(10.0);
    let v = refused.view();
    assert_eq!(v.ledger.establishments, 0, "a spore germinated over too little wood");
    assert!(v.stand_at(at(2)).is_none(), "something stands on the refused face");
    assert!(
        v.ground_at(at(2)).is_some_and(|g| g.seed_organic(Species::Glowcap) > 0.0),
        "the package never landed, so the refusal says nothing"
    );
    assert_residuals(&refused, "after a refused glowcap germination");
}

// ------------------------------------------------------------- the deposit kind

/// **`DepositKind::DeadWood`**: the log lands in the pool a dieback fills, with its mineral
/// and its energy, booked as `deposited_*_in`, and it decomposes at `wood_decomposition`
/// from the tick after the inter-tick it arrived in — the snapshot rule, unchanged.
#[test]
fn a_dead_wood_deposit_joins_the_pool_and_decomposes_at_the_wood_rate() {
    let mut world = pillars(4, &[1], 0.5);
    let mut config = config();
    // A readable wood rate: the placeholder 1e-4 /s would move 2e-6 of the pool per tick.
    config.wood_decomposition = 1.0;
    config.decomposition = 0.0;
    let mut flora = Flora::new(config);
    let site = at(1);
    let (organic, mineral, energy) = log_on(&mut flora, site, 0.4);

    let v = flora.view();
    let g = v.ground_at(site).expect("the deposit provisioned no ground").clone();
    assert_eq!((g.dead_wood, g.dead_wood_mineral, g.dead_wood_energy), (organic, mineral, energy));
    assert_eq!((g.litter, g.carrion), (0.0, 0.0), "a log is neither litter nor a corpse");
    assert_eq!(v.ledger.deposited_organic_in, organic);
    assert_eq!(v.ledger.deposited_mineral_in, mineral);
    assert_eq!(v.ledger.deposited_energy_in, energy);
    // The lazy provisioning is `seeded_mineral_in`, as it is for a founder.
    assert_eq!(v.ledger.seeded_mineral_in, flora.config().initial_mineral);
    assert_residuals(&flora, "after a log was laid");

    flora.step(&mut world);
    let v = flora.view();
    let g = v.ground_at(site).expect("ground").clone();
    let gone = organic - g.dead_wood;
    assert!((gone - 1.0 * DT * organic).abs() < 1e-15, "{gone} of {organic} in one tick");
    assert!((v.ledger.respired_out - gone).abs() < 1e-15, "the wood was not respired");
    // Mineral to the soil at the stock's own fraction, energy to heat at its own density.
    assert!((g.mineral - flora.config().initial_mineral - mineral * DT).abs() < 1e-15);
    assert!((v.ledger.heat_out - energy * DT).abs() < 1e-15);
    assert_residuals(&flora, "after one tick of log decomposition");
}

// ---------------------------------------------------------------------- validity

/// **Validity.** The shipped glowcap passes, and each of the three new fields is refused
/// with the species and the field named when it cannot mean anything: a yield over one
/// would build tissue out of nothing, and a non-finite or negative rate or threshold
/// becomes a `NaN` in the ledger within one step.
///
/// Deliberately **not** refused: a zero `substrate_uptake_per_s` or a zero
/// `substrate_yield` on a saprotroph. A species with no income is exactly what a frozen
/// fixture wants, and `assimilation` 0 has always been allowed on a plant for the same
/// reason; a validator that refused it would be a plausibility check
/// ([`SpeciesConfig::validate`]'s own doc says it is not one).
#[test]
fn validate_covers_the_three_new_fields() {
    let config = config();
    config.validate().expect("the shipped config");
    config.glowcap.validate("glowcap").expect("the shipped glowcap");
    assert_eq!(config.glowcap.trophic, Trophic::Saprotroph);
    for species in Species::ALL {
        let sc = config.species(species);
        let is_fungus = sc.trophic == Trophic::Saprotroph;
        assert_eq!(is_fungus, species == Species::Glowcap, "{} is the wrong mode", species.name());
        // The five plants carry the three numbers inert and at zero, so switching one to a
        // saprotroph cannot silently inherit a rate.
        if !is_fungus {
            assert_eq!(sc.substrate_uptake_per_s, 0.0, "{}", species.name());
            assert_eq!(sc.substrate_yield, 0.0, "{}", species.name());
            assert_eq!(sc.establish_substrate_min, 0.0, "{}", species.name());
        }
    }

    for (what, mutate) in [
        ("substrate_yield", (|sc: &mut SpeciesConfig| sc.substrate_yield = 1.5) as fn(&mut _)),
        ("substrate_yield", |sc: &mut SpeciesConfig| sc.substrate_yield = -0.1),
        ("substrate_yield", |sc: &mut SpeciesConfig| sc.substrate_yield = f64::NAN),
        ("substrate_uptake_per_s", |sc: &mut SpeciesConfig| sc.substrate_uptake_per_s = f64::NAN),
        ("substrate_uptake_per_s", |sc: &mut SpeciesConfig| sc.substrate_uptake_per_s = -1.0),
        ("establish_substrate_min", |sc: &mut SpeciesConfig| sc.establish_substrate_min = -1.0),
        (
            "establish_substrate_min",
            |sc: &mut SpeciesConfig| sc.establish_substrate_min = f64::INFINITY,
        ),
    ] {
        let mut broken = config.clone();
        mutate(broken.species_mut(Species::Glowcap));
        let e = broken.validate().expect_err("a broken {what} was accepted");
        assert!(e.contains("glowcap") && e.contains(what), "{e}");
        assert!(Flora::try_new(broken).is_err(), "a broken config reached a tick");
    }

    // A zero-income saprotroph is a legal frozen fixture, not a refusal.
    let mut frozen = config.clone();
    frozen.glowcap.substrate_uptake_per_s = 0.0;
    frozen.glowcap.substrate_yield = 0.0;
    frozen.validate().expect("a frozen fungus is a valid preset");
}

/// The glowcap's own shape, in the model rather than in the picture: **one crown cell at
/// every size**, and the lowest crown top of the six, so it shades nothing.
#[test]
fn a_glowcap_crown_is_one_cell_high_at_every_size() {
    let config = config();
    let sc = &config.glowcap;
    for wood in [sc.alive_min, 0.5 * sc.wood_max, sc.wood_max] {
        assert_eq!(sc.crown_voxels(wood), 1, "at wood {wood}");
        assert_eq!(sc.crown_height(wood), 0.5, "at wood {wood}");
        assert_eq!(sc.crown_radius(wood), 0.5, "at wood {wood}");
    }
    for species in Species::ALL {
        if species == Species::Glowcap {
            continue;
        }
        let other = config.species(species);
        assert!(
            other.crown_height(other.alive_min) >= sc.crown_height(sc.wood_max),
            "{} starts lower than a full-grown glowcap",
            species.name()
        );
    }
}
