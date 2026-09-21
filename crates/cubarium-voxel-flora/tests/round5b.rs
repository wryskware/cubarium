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
    Command, DT, Deposit, DepositKind, Flora, FloraConfig, Site, Species, SpeciesConfig, Trophic,
};

// ------------------------------------------------------------------- fixtures

/// `round5a.rs`'s `fill`: turn one air voxel into `material` holding exactly `pore` of that
/// material's own pore capacity, by adding the water first and converting after.
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
            w.apply(WorldCommand::SetMaterial {
                x,
                y: 0,
                z: 0,
                material: Material::Air,
            });
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
            w.apply(WorldCommand::SetMaterial {
                x,
                y: 4,
                z,
                material: Material::Rock,
            });
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
        flora.deposit(
            site,
            Deposit {
                kind: DepositKind::DeadWood,
                organic,
                mineral,
                energy
            }
        ),
        "the log was refused at {site:?}"
    );
    (organic, mineral, energy)
}

/// A glowcap founder on a kept column, at half its own `wood_max` — the harness's founder
/// size, and exactly `donor_min` for all six presets.
fn plant_glowcap(flora: &mut Flora, world: &World, x: i64) -> Site {
    let wood = 0.5 * flora.config().species(Species::Glowcap).wood_max;
    assert!(flora.apply(
        world,
        Command::Seed {
            x,
            z: 0,
            species: Species::Glowcap,
            wood
        }
    ));
    let site = cubarium_voxel_flora::highest_support(&world.view(), x, 0).expect("a support face");
    assert_eq!(
        flora.view().stand_at(site).expect("a founder").species,
        Species::Glowcap
    );
    site
}

fn dead_wood_at(flora: &Flora, site: Site) -> f64 {
    flora.view().ground_at(site).map_or(0.0, |g| g.dead_wood)
}

fn litter_at(flora: &Flora, site: Site) -> f64 {
    flora.view().ground_at(site).map_or(0.0, |g| g.litter)
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
    assert_eq!(
        sc.trophic,
        Trophic::Saprotroph,
        "the preset is the thing being tested"
    );
    assert_eq!(
        sc.assimilation, 0.0,
        "a saprotroph fixes nothing from light"
    );

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
    assert!(
        gone > uptake(&flora),
        "{gone} left the log against {} eaten",
        uptake(&flora)
    );
    assert!(
        f.wood > w0,
        "the fed fungus did not grow: {} against {w0}",
        f.wood
    );
    assert!(
        f.light > 0.0 && f.moisture > 0.0,
        "the fixture is lit and damp: {f:?}"
    );

    // The starved one earns nothing at all under the same sky, and pays maintenance out of
    // its reserve.
    assert_eq!(
        dead_wood_at(&flora, starved),
        0.0,
        "the bare face has no log"
    );
    assert!(
        s.wood <= w0,
        "the starved fungus grew on nothing: {} against {w0}",
        s.wood
    );
    assert!(
        s.reserve < q0,
        "the starved fungus paid nothing: {} against {q0}",
        s.reserve
    );
    assert!(
        s.light > 0.0,
        "the starved stand is in open sky and still earns nothing: {s:?}"
    );

    // And the light boundary never moved for either of them.
    assert_eq!(
        v.ledger.fixed_in, fixed0,
        "a saprotroph fixed organic matter from light"
    );
    assert_eq!(
        v.ledger.light_in, 0.0,
        "a saprotroph took energy off the light boundary"
    );
    assert_residuals(&flora, "after 1 s of one fed and one starved fungus");
}

/// **The uptake is bounded by the pool, and stops when it is empty.** The rate asks for
/// 2e-5 of organic matter on this fixture's first tick and the log holds half of that: the
/// fungus takes the log, exactly, and the pool's mineral and energy go with the last of it
/// rather than leaving float dust behind claiming to be a stock. Every tick after that
/// takes nothing **out of that pool** — the fungus goes on eating the litter it sheds
/// itself, which is the litter diet and has its own tests below.
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
    assert!(
        organic < want,
        "the log has to be smaller than one tick's demand"
    );

    flora.step(&mut world);
    assert!(
        (uptake(&flora) - organic).abs() <= 1e-18,
        "took {} of {organic}",
        uptake(&flora)
    );
    let g = flora.view().ground_at(site).expect("ground").clone();
    assert_eq!(
        g.dead_wood, 0.0,
        "the pool went negative or kept dust: {}",
        g.dead_wood
    );
    assert_eq!(
        g.dead_wood_mineral, 0.0,
        "mineral left behind on an empty pool"
    );
    assert_eq!(
        g.dead_wood_energy, 0.0,
        "energy left behind on an empty pool"
    );
    // The mineral that came with the wood is in the fungus and in the site's pool, and
    // nowhere else: `arrived` over what the tissue needed is released to the soil.
    let v = flora.view();
    let stand = *v.stand_at(site).expect("alive");
    let pool = v.ground_at(site).expect("ground").mineral;
    assert!(stand.mineral > 0.0 && pool > 0.0, "{stand:?} {pool}");

    // **The empty pool feeds nothing more.** Since the litter diet the fungus also eats
    // the foliage it sheds on its own site, so the *uptake* does not stop here — what
    // stops is this pool: the dead wood stays at exactly zero for every one of the next
    // twenty ticks, and everything the fungus takes after the log comes out of the litter
    // it shed itself, which is the only other stock on the face.
    let after_one = uptake(&flora);
    let mut last = after_one;
    for _ in 0..20 {
        flora.step(&mut world);
        assert_eq!(
            dead_wood_at(&flora, site),
            0.0,
            "an empty log went on feeding the fungus"
        );
        // The two pools are the only stocks `feed` reads, and this one is empty on every
        // one of these ticks, so whatever the fungus still takes came out of the litter
        // it sheds — and it is still bounded by the rate.
        let tick = uptake(&flora) - last;
        last = uptake(&flora);
        assert!(
            tick <= want + 1e-18,
            "a tick took {tick} against a demand of {want}"
        );
    }
    assert!(
        uptake(&flora) > after_one,
        "the litter the fungus shed fed it nothing"
    );
    assert!(
        litter_at(&flora, site) > 0.0,
        "the fixture has no litter in it at all"
    );
    assert_eq!(dead_wood_at(&flora, site), 0.0);
    assert_residuals(&flora, "after a log was eaten whole");
    assert!(
        mineral > 0.0 && energy > 0.0,
        "the fixture laid a log with substance in it"
    );
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
    assert!(
        g.dead_wood_mineral < log_mineral,
        "none left the log: {}",
        g.dead_wood_mineral
    );
    assert!(stand.mineral > 0.0, "the mycelium holds none");
    assert!(g.litter_mineral > 0.0, "the caps shed none to litter");
    assert!(g.mineral > 0.0, "nothing reached the soil");
    // And the total is exactly where it was: no boundary flow touched it.
    assert!(
        (v.mineral() - total0).abs() <= 1e-12,
        "total mineral moved from {total0} to {}",
        v.mineral()
    );
    assert_eq!(
        v.ledger.expected_mineral(),
        expected0,
        "a mineral boundary flow was booked"
    );
    assert_residuals(&flora, "after 4 s of digestion");
}

/// **A small finite log runs out, and the fungus starts losing.** Direction only, and no
/// death claim: over 200 ticks the dead-wood pool never rises, the uptake per window
/// falls, and once the log is gone the reserve falls every window.
///
/// The uptake does not fall to *nothing*, as it did before the litter diet: a glowcap eats
/// the foliage it sheds, so an emptied log leaves a trickle of self-shed litter behind it.
/// The trickle is a fraction of what the log paid — the claim is that it does not keep the
/// fungus solvent, which is what the falling reserve says.
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
    assert_eq!(
        windows[3].0, 0.0,
        "the log is not empty at 4 s: {windows:?}"
    );
    assert!(
        windows[3].1 < 0.2 * windows[0].1,
        "the emptied log left the fungus as well fed as the log did: {windows:?}"
    );
    assert!(
        windows[3].2 < windows[2].2,
        "the reserve did not fall: {windows:?}"
    );
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
    let with_log = flora
        .view()
        .establishment_gates(&view, at(1), Species::Glowcap);
    let without = flora
        .view()
        .establishment_gates(&view, at(4), Species::Glowcap);
    assert!(with_log.passes() && flora.view().can_establish(&view, at(1), Species::Glowcap));
    assert!(
        !without.passes(),
        "a face with no log admitted a spore: {without:?}"
    );
    assert_eq!(
        (
            with_log.pore_ok,
            with_log.aeration_ok,
            with_log.depth_ok,
            with_log.light_ok
        ),
        (
            without.pore_ok,
            without.aeration_ok,
            without.depth_ok,
            without.light_ok
        ),
        "the two faces differ in something other than the substrate"
    );
    assert!(
        with_log.substrate_ok && !without.substrate_ok,
        "{with_log:?} {without:?}"
    );
    assert!((with_log.dead_wood - 1.0).abs() < 1e-15 && without.dead_wood == 0.0);
    // The light gate is open for a saprotroph whatever the sky says, and the free function
    // that cannot see a log refuses both faces — which is what it is documented to do.
    assert!(with_log.light_ok && without.light_ok);
    let sc = flora.config().species(Species::Glowcap);
    assert!(
        !cubarium_voxel_flora::can_establish(&view, at(1), sc),
        "no substrate, no pass"
    );

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
    let glow = under
        .view()
        .establishment_gates(&view, cellar, Species::Glowcap);
    assert_eq!(
        glow.sky_visibility, turf.sky_visibility,
        "one site, one sky"
    );
    assert_eq!(turf.sky_visibility, 0.0, "the roof leaks: {turf:?}");
    assert!(
        !turf.light_ok && !turf.passes(),
        "springturf started in the dark: {turf:?}"
    );
    assert!(
        !frond.light_ok,
        "the shade-tolerant plant still wants some sky: {frond:?}"
    );
    assert!(
        glow.light_ok && glow.passes(),
        "the dark shut the fungus out: {glow:?}"
    );
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
        assert!(flora.apply(
            &world,
            Command::Seed {
                x: 1,
                z: 0,
                species: Species::Glowcap,
                wood
            }
        ));
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
    assert_eq!(
        v.ledger.establishments, 1,
        "no birth in 1 s: {:?}",
        v.ledger
    );
    let born = *v
        .stand_at(at(2))
        .expect("nothing stands on the only landing site");
    assert_eq!(born.species, Species::Glowcap);
    assert_eq!(
        born.wood,
        config().glowcap.alive_min,
        "born with {} of wood",
        born.wood
    );
    assert!(
        (born.organic() - package).abs() <= 1e-16,
        "born holding {} of a {package} package",
        born.organic()
    );
    assert!(
        born.foliage > 0.0 && born.reserve > 0.0 && born.mineral > 0.0,
        "{born:?}"
    );
    assert!(v.ledger.propagule_landed[Species::Glowcap.index()] > 0.0);
    assert_residuals(&flora, "after a paid glowcap birth");
    // The only face inside `hop` 1 is the one it landed on, so the landing is geometry and
    // not a lucky draw.

    // The same run with a threshold the log cannot meet: the bank lands, waits, and never
    // germinates. Nothing else differs.
    let (refused, _world, _) = birth(10.0);
    let v = refused.view();
    assert_eq!(
        v.ledger.establishments, 0,
        "a spore germinated over too little wood"
    );
    assert!(
        v.stand_at(at(2)).is_none(),
        "something stands on the refused face"
    );
    assert!(
        v.ground_at(at(2))
            .is_some_and(|g| g.seed_organic(Species::Glowcap) > 0.0),
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
    let g = v
        .ground_at(site)
        .expect("the deposit provisioned no ground")
        .clone();
    assert_eq!(
        (g.dead_wood, g.dead_wood_mineral, g.dead_wood_energy),
        (organic, mineral, energy)
    );
    assert_eq!(
        (g.litter, g.carrion),
        (0.0, 0.0),
        "a log is neither litter nor a corpse"
    );
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
    assert!(
        (gone - 1.0 * DT * organic).abs() < 1e-15,
        "{gone} of {organic} in one tick"
    );
    assert!(
        (v.ledger.respired_out - gone).abs() < 1e-15,
        "the wood was not respired"
    );
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
    config
        .glowcap
        .validate("glowcap")
        .expect("the shipped glowcap");
    assert_eq!(config.glowcap.trophic, Trophic::Saprotroph);
    for species in Species::ALL {
        let sc = config.species(species);
        let is_fungus = sc.trophic == Trophic::Saprotroph;
        assert_eq!(
            is_fungus,
            species == Species::Glowcap,
            "{} is the wrong mode",
            species.name()
        );
        // The five plants carry the three numbers inert and at zero, so switching one to a
        // saprotroph cannot silently inherit a rate.
        if !is_fungus {
            assert_eq!(sc.substrate_uptake_per_s, 0.0, "{}", species.name());
            assert_eq!(sc.substrate_yield, 0.0, "{}", species.name());
            assert_eq!(sc.establish_substrate_min, 0.0, "{}", species.name());
        }
    }

    for (what, mutate) in [
        (
            "substrate_yield",
            (|sc: &mut SpeciesConfig| sc.substrate_yield = 1.5) as fn(&mut _),
        ),
        ("substrate_yield", |sc: &mut SpeciesConfig| {
            sc.substrate_yield = -0.1
        }),
        ("substrate_yield", |sc: &mut SpeciesConfig| {
            sc.substrate_yield = f64::NAN
        }),
        ("substrate_uptake_per_s", |sc: &mut SpeciesConfig| {
            sc.substrate_uptake_per_s = f64::NAN
        }),
        ("substrate_uptake_per_s", |sc: &mut SpeciesConfig| {
            sc.substrate_uptake_per_s = -1.0
        }),
        ("establish_substrate_min", |sc: &mut SpeciesConfig| {
            sc.establish_substrate_min = -1.0
        }),
        ("establish_substrate_min", |sc: &mut SpeciesConfig| {
            sc.establish_substrate_min = f64::INFINITY
        }),
    ] {
        let mut broken = config.clone();
        mutate(broken.species_mut(Species::Glowcap));
        let e = broken.validate().expect_err("a broken {what} was accepted");
        assert!(e.contains("glowcap") && e.contains(what), "{e}");
        assert!(
            Flora::try_new(broken).is_err(),
            "a broken config reached a tick"
        );
    }

    // A zero-income saprotroph is a legal frozen fixture, not a refusal.
    let mut frozen = config.clone();
    frozen.glowcap.substrate_uptake_per_s = 0.0;
    frozen.glowcap.substrate_yield = 0.0;
    frozen
        .validate()
        .expect("a frozen fungus is a valid preset");
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

// -------------------------------------------- Astra's round 8: the deposit boundary

/// **R8.1: pruning a site books out its carrion too.** A carrion deposit of
/// `(0.4, 0.012, 0.9)` on a support face, the support taken away, one step: the three
/// stocks leave through `removed_*_out` and the residuals stay at noise. Before the repair
/// they were −0.4, −0.012 and −0.9 — an actual residual, because `FloraView`'s totals count
/// the carrion pool.
#[test]
fn pruning_an_unsupported_site_books_out_its_carrion() {
    let mut world = pillars(4, &[1], 0.5);
    let mut flora = Flora::new(config());
    let site = at(1);
    let (organic, mineral, energy) = (0.4, 0.012, 0.9);
    assert!(flora.deposit(
        site,
        Deposit {
            kind: DepositKind::Carrion,
            organic,
            mineral,
            energy
        }
    ));
    let pool = flora.view().ground_at(site).expect("provisioned").mineral;
    assert_residuals(&flora, "after a carrion deposit");

    // Take the support away: the face at `y = 2` is no longer a support, so the site is
    // gone with it. (The column's bedrock is still there, so the *column* has a support
    // face lower down; what the site lost is its own.)
    for y in 1..=2 {
        world.apply(WorldCommand::SetMaterial {
            x: 1,
            y,
            z: 0,
            material: Material::Air,
        });
    }
    assert!(
        !world.view().is_support(1, 2, 0),
        "the face is still a support"
    );
    flora.step(&mut world);

    let v = flora.view();
    assert!(
        v.ground_at(site).is_none(),
        "the site survived losing its support"
    );
    assert_eq!(
        v.ledger.removed_organic_out, organic,
        "the carrion was not booked out"
    );
    assert_eq!(
        v.ledger.removed_mineral_out,
        mineral + pool,
        "with the soluble pool"
    );
    assert_eq!(v.ledger.removed_energy_out, energy);
    assert_eq!(
        v.organic(),
        0.0,
        "something is still standing: {:?}",
        v.stands
    );
    assert_residuals(&flora, "after the support was taken away");
}

/// **R8.1, the other half**: a deposit made **directly** on a site that is not a support
/// face is accepted — `deposit` reads no world — and booked out by the next tick's step 1,
/// which is what its own doc promises. All three kinds, and all three residuals.
#[test]
fn a_deposit_on_a_site_with_no_support_is_booked_out_by_the_next_tick() {
    for kind in [
        DepositKind::Carrion,
        DepositKind::Litter,
        DepositKind::DeadWood,
    ] {
        let mut world = pillars(4, &[1], 0.5);
        let mut flora = Flora::new(config());
        // A site in the void: `pillars` left column 2 with nothing solid at all.
        let nowhere = Site { x: 2, y: 2, z: 0 };
        assert!(cubarium_voxel_flora::highest_support(&world.view(), 2, 0).is_none());
        let (organic, mineral, energy) = (0.4, 0.012, 0.9);
        assert!(
            flora.deposit(
                nowhere,
                Deposit {
                    kind,
                    organic,
                    mineral,
                    energy
                }
            ),
            "{kind:?}"
        );
        assert_residuals(&flora, "after a deposit into the void");

        flora.step(&mut world);
        let v = flora.view();
        assert!(
            v.ground_at(nowhere).is_none(),
            "{kind:?}: the void site survived"
        );
        assert_eq!(v.ledger.removed_organic_out, organic, "{kind:?}");
        assert_eq!(
            v.ledger.removed_mineral_out,
            mineral + flora.config().initial_mineral,
            "{kind:?}"
        );
        // Litter is the one kind with an energy cap: `e_d_max · D` is 0.8 of the 0.9
        // offered, and the 0.1 it cannot hold left as heat when the deposit landed, which
        // is the rule round 5a already had.
        let held = if kind == DepositKind::Litter {
            flora.config().litter_energy_cap * organic
        } else {
            energy
        };
        assert_eq!(v.ledger.removed_energy_out, held, "{kind:?}");
        assert_eq!(v.ledger.heat_out, energy - held, "{kind:?}");
        assert_residuals(&flora, "after an unsupported deposit was pruned");
    }
}

/// **R8.2: a deposit with no organic matter in it settles at once.** A dead pool
/// decomposes `rate · dt · organic`, so `(0, 0.02, 0.4)` in a pool would have held its
/// mineral for ever and never released its energy. It is still accepted and still booked
/// `deposited_*_in` in full — an exhausted consumer has to have somewhere to put the
/// mineral its respiration left behind — but the mineral goes straight to the site's
/// soluble pool and the energy leaves as heat, which is where decomposition would have sent
/// them. Both for all three kinds, with the pools left empty.
#[test]
fn a_zero_organic_deposit_credits_the_pool_and_the_heat_at_once() {
    for kind in [
        DepositKind::Carrion,
        DepositKind::Litter,
        DepositKind::DeadWood,
    ] {
        let mut world = pillars(4, &[1], 0.5);
        let mut flora = Flora::new(config());
        let site = at(1);
        let (mineral, energy) = (0.02, 0.4);
        assert!(
            flora.deposit(
                site,
                Deposit {
                    kind,
                    organic: 0.0,
                    mineral,
                    energy
                }
            ),
            "{kind:?}: a consumer with only mineral left was refused"
        );
        let v = flora.view();
        let g = v.ground_at(site).expect("provisioned").clone();
        assert_eq!(
            (g.litter, g.dead_wood, g.carrion),
            (0.0, 0.0, 0.0),
            "{kind:?}: a pool"
        );
        assert_eq!(
            (g.litter_mineral, g.dead_wood_mineral, g.carrion_mineral),
            (0.0, 0.0, 0.0),
            "{kind:?}: mineral stranded in a pool that can never release it"
        );
        assert_eq!(
            g.mineral,
            flora.config().initial_mineral + mineral,
            "{kind:?}"
        );
        assert_eq!(v.ledger.heat_out, energy, "{kind:?}");
        assert_eq!(v.ledger.deposited_mineral_in, mineral, "{kind:?}");
        assert_eq!(v.ledger.deposited_energy_in, energy, "{kind:?}");
        assert_eq!(v.ledger.deposited_organic_in, 0.0, "{kind:?}");
        assert_residuals(&flora, "after a zero-organic deposit");

        // And it stays settled: nothing about it moves again.
        run(&mut flora, &mut world, 5);
        let v = flora.view();
        assert_eq!(
            v.ground_at(site).expect("ground").mineral,
            flora.config().initial_mineral + mineral
        );
        assert_eq!(v.ledger.heat_out, energy, "{kind:?}");
        assert_residuals(&flora, "five ticks after a zero-organic deposit");
    }
}

/// **R8.5: what waits for the next tick is the organic throughput, and not every
/// currency.** `decompose` sizes its draw on the **tick-start** organic stock but takes the
/// mineral and the energy at the pool's **current** density, so material parcels are not
/// age-isolated: mineral that arrived inside this tick can leave inside this tick.
///
/// Astra's own case, built out of the model: one old unit of litter holding **no** mineral
/// in the tick-start snapshot, one fresh unit shed inside the tick carrying **one** mineral,
/// and a decomposition step of half the old stock — `dec = 0.5`, pool now 2.0, so
/// `f = 0.25` and **0.25 of mineral reaches the soluble pool immediately**. That is the
/// inherited well-mixed-pool rule and it conserves every currency; it is documented rather
/// than changed, and this pins it before a consumer relies on a stronger claim.
#[test]
fn decomposition_delays_organic_matter_and_not_the_mineral_of_a_mixed_pool() {
    let mut config = config();
    // A stand that sheds its whole canopy in one tick and does nothing else: no income, no
    // maintenance, so no growth, no dieback and no mineral draw.
    let sc = &mut config.bloomcrown;
    sc.assimilation = 0.0;
    sc.maintenance = 0.0;
    sc.senescence = 1e9;
    sc.alpha = 2.0;
    sc.n_tissue = 1.0;
    sc.wood_max = 1.0;
    // Half of the tick-start stock per tick: `rate · DT` = 0.5.
    config.decomposition = 0.5 / DT;
    config.wood_decomposition = 0.0;
    let initial_mineral = config.initial_mineral;
    let mut world = pillars(4, &[1], 0.5);
    let mut flora = Flora::new(config);
    let site = at(1);

    // The old unit: one of organic matter with no mineral and no energy in it, laid in the
    // inter-tick so that it is in the next tick's snapshot.
    assert!(flora.deposit(
        site,
        Deposit {
            kind: DepositKind::Litter,
            organic: 1.0,
            mineral: 0.0,
            energy: 0.0
        }
    ));
    // The fresh unit: a founder at wood 0.5 sheds `alpha · W` = 1.0 of foliage this tick,
    // and the fraction rule sends `n_tissue · 1.0` = 1.0 of mineral with it.
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 1,
            z: 0,
            species: Species::Bloomcrown,
            wood: 0.5
        }
    ));
    let pool0 = flora.view().ground_at(site).expect("ground").mineral;
    assert_eq!(pool0, initial_mineral, "the fixture's premise");
    assert_residuals(&flora, "before the mixed tick");

    flora.step(&mut world);

    let v = flora.view();
    let g = v.ground_at(site).expect("ground").clone();
    // 1.0 old + 1.0 shed = 2.0, less `dec` = 0.5 of it.
    assert!((g.litter - 1.5).abs() < 1e-12, "litter {}", g.litter);
    // And the mineral: 1.0 arrived this tick, 0.25 of it is already in the soluble pool.
    assert!(
        (g.mineral - (pool0 + 0.25)).abs() < 1e-12,
        "the soluble pool is {} and not {}: the mixed-density release",
        g.mineral,
        pool0 + 0.25
    );
    assert!(
        (g.litter_mineral - 0.75).abs() < 1e-12,
        "litter mineral {}",
        g.litter_mineral
    );
    assert_residuals(&flora, "after one mixed-density decomposition step");
}

// -------------------------------------------- R9.1: the mineral budget before the tissue

/// A log with a **declared** mineral content: `log_on`'s deposit with the density chosen
/// by the caller rather than a dead trunk's own `n_tissue`. A log carrying less mineral
/// than the tissue it would become is exactly what R9.1 is about, and there is no way to
/// say it with `log_on`.
fn log_with(flora: &mut Flora, site: Site, organic: f64, mineral: f64) -> (f64, f64, f64) {
    let energy = flora.config().species(Species::Glowcap).energy_density * organic;
    assert!(
        flora.deposit(
            site,
            Deposit {
                kind: DepositKind::DeadWood,
                organic,
                mineral,
                energy
            }
        ),
        "the log was refused at {site:?}"
    );
    (organic, mineral, energy)
}

/// A config for the R9.1 fixtures: a **bare** mineral pool, so the only mineral in the
/// world is the log's own; **no decomposition at all**, so the only thing that moves the
/// log is the fungus; and **no spores**, so the reserve funds nothing and every unit the
/// ledger respires is this tick's metabolism. All three are fixture conditions and none of
/// them is a preset change.
fn budget_config() -> FloraConfig {
    let mut config = config();
    config.initial_mineral = 0.0;
    config.decomposition = 0.0;
    config.wood_decomposition = 0.0;
    config.glowcap.propagule_rate = 0.0;
    config
}

/// **R9.1, the zero-mineral case.** Astra's own: `initial_mineral` 0, a moist half-grown
/// glowcap (`W` 0.05) and an energy-bearing, **mineral-free** log. One default tick takes
/// `5e-5` of organic matter, earns `2e-5` of it and pays `5e-7` of maintenance out of that
/// — and builds **nothing**, because the `1e-7` of mineral that `5e-6` of new wood needs
/// does not exist anywhere it can draw on. Before the repair it grew the wood anyway and
/// the tissue held less than `n_tissue`.
///
/// The stand's own `mineral` inventory is 0.001 and is **not** a reserve it can build out
/// of: that is the standing R4.3 limitation, and this fixture is where it bites.
#[test]
fn a_mineral_free_log_pays_the_upkeep_and_builds_nothing() {
    let mut world = pillars(4, &[1], 0.5);
    let mut flora = Flora::new(budget_config());
    let site = plant_glowcap(&mut flora, &world, 1);
    let (organic, _, energy) = log_with(&mut flora, site, 1.0, 0.0);
    let sc = flora.config().species(Species::Glowcap).clone();
    let before = flora.view().stand_at(site).expect("the founder").clone();
    assert_eq!(
        (before.wood, before.foliage, before.reserve),
        (0.05, 0.1, 0.025)
    );
    assert!(
        before.mineral > 0.0,
        "a founder arrives with its own tissue mineral"
    );
    assert_eq!(
        flora.view().ground_at(site).expect("ground").mineral,
        0.0,
        "a bare pool"
    );

    flora.step(&mut world);

    let v = flora.view();
    let after = v.stand_at(site).expect("the founder").clone();
    let g = v.ground_at(site).expect("ground").clone();
    // What it ate: the rate on a full-moisture box, and the log still holds the rest.
    let want = sc.substrate_uptake_per_s * before.wood * 1.0 * DT;
    assert!(
        (uptake(&flora) - want).abs() <= 1e-15 * want,
        "it took {} of {want}",
        uptake(&flora)
    );
    assert!((g.dead_wood - (organic - want)).abs() < 1e-15);
    // And what it built: nothing at all, in any of the three tissues.
    assert_eq!(after.wood, before.wood, "no wood was built");
    assert_eq!(
        after.reserve, before.reserve,
        "the income paid the upkeep, the reserve did not"
    );
    assert!(after.foliage < before.foliage, "the caps only senesced");
    let shed = sc.senescence * before.foliage * DT;
    assert!((after.foliage - (before.foliage - shed)).abs() <= 1e-15 * before.foliage);
    // Nothing was drawn from the pool and nothing was released to it: there was no
    // mineral on either side of the settlement.
    assert_eq!(g.mineral, 0.0, "the pool is still bare");
    // Every unit it ate was respired, and the log's energy left as heat with it: with no
    // decomposition in this fixture the fungus is the only thing respiring.
    assert!(
        (v.ledger.respired_out - want).abs() <= 1e-12 * want,
        "respired {} of {want}",
        v.ledger.respired_out
    );
    let e_v = energy / organic;
    assert!(
        (v.ledger.heat_out - e_v * want).abs() <= 1e-12 * e_v * want,
        "heat {} of {}",
        v.ledger.heat_out,
        e_v * want
    );
    assert_residuals(&flora, "one tick on a mineral-free log");
}

/// **R9.1, the partially funded case.** The same tick on three logs that differ in one
/// number: a log at a dead trunk's own density over-funds the tick and the excess mineral
/// is released to the site's pool, a log at a **twentieth** of that density funds exactly
/// half of the wood the income could otherwise build, and the growth is exactly what the
/// mineral pays for — `Δw = arrived / n_tissue`, to the bit.
///
/// A fresh founder is the clean case on purpose: `Command::Seed` gives it `α · W` of
/// foliage and `reserve_cap · W` of reserve, so `d_p` and `d_q` are both zero on its first
/// tick and the only new tissue in it is wood.
#[test]
fn a_partly_mineralised_log_builds_exactly_what_its_mineral_funds() {
    /// One tick on a log of `organic` holding `mineral`: the wood built, and the site's
    /// mineral pool and the log's own mineral afterwards.
    fn one_tick(mineral: f64) -> (f64, f64, f64) {
        let mut world = pillars(4, &[1], 0.5);
        let mut flora = Flora::new(budget_config());
        let site = plant_glowcap(&mut flora, &world, 1);
        log_with(&mut flora, site, 1.0, mineral);
        let w0 = flora.view().stand_at(site).expect("the founder").wood;
        flora.step(&mut world);
        assert_residuals(&flora, "one tick on a partly mineralised log");
        let v = flora.view();
        let g = v.ground_at(site).expect("ground").clone();
        (
            v.stand_at(site).expect("the founder").wood - w0,
            g.mineral,
            mineral - g.dead_wood_mineral,
        )
    }

    let sc = FloraConfig::default().species(Species::Glowcap).clone();
    let d_w = (sc.wood_rate * 0.05 * DT).min(sc.wood_max - 0.05);
    assert!(
        (d_w - 5e-6).abs() <= 1e-15 * 5e-6,
        "the tick's wood demand is {d_w}"
    );

    // Over-funded: a dead trunk's own density. The wood is the rate's own demand, and the
    // mineral the tissue did not need went to the site's pool.
    let (dw_full, pool_full, arrived_full) = one_tick(sc.n_tissue * 1.0);
    // The tolerances are relative because both numbers are differences of stocks a
    // thousand times their own size: `0.050005 - 0.05` carries an ulp of 0.05 with it.
    assert!(
        (dw_full - d_w).abs() <= 1e-9 * d_w,
        "the full log built {dw_full} of {d_w}"
    );
    assert!(
        (arrived_full - 1e-6).abs() <= 1e-9 * 1e-6,
        "arrived {arrived_full}"
    );
    let need_full = sc.n_tissue * dw_full;
    assert!(
        (pool_full - (arrived_full - need_full)).abs() <= 1e-9 * (arrived_full - need_full),
        "the excess {} went to the pool, not {}",
        arrived_full - need_full,
        pool_full
    );

    // Half-funded: a log at 0.001 of mineral per unit, a twentieth of a trunk's density,
    // so the tick's arrival is 5e-8 and pays for 2.5e-6 of wood exactly.
    let (dw_half, pool_half, arrived_half) = one_tick(0.001);
    assert!(
        (arrived_half - 5e-8).abs() <= 1e-9 * 5e-8,
        "arrived {arrived_half}"
    );
    assert!(
        (dw_half - arrived_half / sc.n_tissue).abs() <= 1e-9 * dw_half,
        "it built {dw_half}, and its mineral funds {}",
        arrived_half / sc.n_tissue
    );
    assert!(
        (dw_half - 0.5 * dw_full).abs() <= 1e-9 * dw_full,
        "half the mineral, half the wood"
    );
    // Nothing is left over and nothing is drawn: the arrival was spent exactly.
    assert_eq!(pool_half, 0.0, "the pool neither gained nor could give");
}

// ------------------------------------------ R9.3: what a mycelium neighbourhood reaches

/// A strip with **steps** in it: `(x, top)` makes column `x` bedrock at `y = 0` and soil
/// from 1 to `top`, so its support face is `top`; every other column is void. What
/// [`pillars`] is for one flat row, for a row a grove would have to climb.
fn terraces(width: u32, tops: &[(i64, u32)], pore: f64) -> World {
    let mut w = empty_world(width, 1);
    for x in 0..width as i64 {
        match tops.iter().find(|&&(cx, _)| cx == x) {
            Some(&(_, top)) => {
                for y in 1..=top {
                    fill(&mut w, x, y, 0, Material::Soil, pore);
                }
                assert_eq!(
                    cubarium_voxel_flora::highest_support(&w.view(), x, 0).map(|s| s.y),
                    Some(top),
                    "column {x} must stand at {top}"
                );
            }
            None => {
                w.apply(WorldCommand::SetMaterial {
                    x,
                    y: 0,
                    z: 0,
                    material: Material::Air,
                });
                assert!(
                    cubarium_voxel_flora::highest_support(&w.view(), x, 0).is_none(),
                    "({x},0) must be void"
                );
            }
        }
    }
    w
}

fn face(x: u32, y: u32) -> Site {
    Site { x, y, z: 0 }
}

/// **R9.3: substrate access reaches one row up and one row down, and no further.** The
/// rule decision R9.3 asked for, pinned as geometry: one log on `(3,2)`, and the
/// germination predicate the model itself runs — read off the flora's own ground — on four
/// faces that differ from it only in height.
///
/// Same level `(3,2)` and `(2,3)` one **up** and `(4,1)` one **down** all have the log in
/// their mycelium box and pass; `(8,4)` is two rows above a log one column away and
/// `(5,2)` is level with one two columns away, and both are refused on substrate alone
/// with every other gate open. Before the repair the two vertical cases failed too, which
/// is what refused all three of the round-5b `community` run's landings.
#[test]
fn substrate_access_reaches_one_row_up_and_one_row_down() {
    let world = terraces(10, &[(2, 3), (3, 2), (4, 1), (5, 2), (7, 2), (8, 4)], 0.5);
    let mut flora = Flora::new(config());
    let (organic, _, _) = log_on(&mut flora, face(3, 2), 1.0);
    log_on(&mut flora, face(7, 2), 1.0);
    let sc = flora.config().species(Species::Glowcap).clone();
    assert_eq!(
        sc.substrate_reach_up_down, 1,
        "the placeholder this case is about"
    );
    assert!(
        organic > sc.establish_substrate_min,
        "a log is more than the gate asks for"
    );

    let v = flora.view();
    let view = world.view();
    for (site, what) in [
        (face(3, 2), "the log's own face"),
        (face(2, 3), "one row up"),
        (face(4, 1), "one row down"),
    ] {
        let g = v.establishment_gates(&view, site, Species::Glowcap);
        assert!(
            (g.dead_wood - organic).abs() < 1e-15,
            "{what} {site:?}: its box holds {} of dead wood",
            g.dead_wood
        );
        assert!(g.passes(), "{what} {site:?} must pass: {g:?}");
    }
    for (site, what) in [
        (face(8, 4), "two rows above a log one column away"),
        (face(5, 2), "level with a log two columns away"),
    ] {
        let g = v.establishment_gates(&view, site, Species::Glowcap);
        assert_eq!(
            g.dead_wood, 0.0,
            "{what} {site:?} is genuinely substrate-free"
        );
        assert!(!g.substrate_ok, "{what}: the substrate gate is what shuts");
        assert!(
            g.pore_ok && g.aeration_ok && g.depth_ok && g.light_ok,
            "{what}: and nothing else is: {g:?}"
        );
        assert!(!g.passes());
    }
    // And the box is the substrate's own geometry, not the water's: `rooting_depth` is
    // untouched, so the soil-water reading of the one-row-up face is still its own row.
    assert_eq!(
        sc.rooting_depth, 1,
        "the soil-water box was not widened to do this"
    );
}

// ------------------------------------------------------- the litter diet (S2)
//
// A saprotroph's substrate is the **sum of dead wood and litter** in its mycelium box,
// for its income and for its establishment gate alike. The five tests below are the ones
// `design/handoffs/voxel-decomposers-and-defaults-2026-09-20.md` §S2 specifies, in its
// order.

/// A config that leaves the model alone and turns off the two *other* things that move a
/// litter pool, so that a test can say what the fungus took and not merely what changed:
/// the pools' own decomposition (`decomposition`, `wood_decomposition`) and the glowcap's
/// litterfall (`senescence`). Both rules have their own tests; neither is under test here,
/// and both would otherwise be added to, or subtracted from, the withdrawal being
/// measured. The five plants are untouched, and so is every number the income rule reads.
fn isolating() -> FloraConfig {
    let mut c = FloraConfig::default();
    c.decomposition = 0.0;
    c.wood_decomposition = 0.0;
    c.glowcap.senescence = 0.0;
    c
}

/// Lay litter on `site` — `organic` units with the mineral and the energy the tissue it
/// fell from held, as a consumer's droppings or a shed canopy would arrive. `e_v` is
/// exactly `litter_energy_cap`, so nothing is capped away and the fixture's energy is the
/// energy it says. Returns the three numbers deposited.
fn litter_on(flora: &mut Flora, site: Site, organic: f64) -> (f64, f64, f64) {
    let sc = flora.config().species(Species::Glowcap).clone();
    let (mineral, energy) = (sc.n_tissue * organic, sc.energy_density * organic);
    assert!(
        energy <= flora.config().litter_energy_cap * organic,
        "the fixture's litter would lose energy to the e_d_max cap"
    );
    assert!(
        flora.deposit(
            site,
            Deposit {
                kind: DepositKind::Litter,
                organic,
                mineral,
                energy
            }
        ),
        "the litter was refused at {site:?}"
    );
    (organic, mineral, energy)
}

/// (a) **Litter alone feeds a fungus.** A glowcap on a face holding litter and no dead
/// wood at all gains tissue over 100 ticks, and the litter pool falls by exactly what the
/// withdrawal took, in each of the three currencies: the organic matter against the
/// ledger's own `substrate_uptake`, and the mineral and the energy against the pool's
/// density rule, which is `take_pool`'s pro rata and the same one `take_litter` gives a
/// consumer.
///
/// The booking is `substrate_uptake` and **not** `consumed_organic_out`, which stays at
/// zero: a glowcap is a stand inside this layer, so its meal crosses no boundary, and the
/// test pins that distinction because a shredder eating the same pool *is* a boundary
/// flow (test (c)).
#[test]
fn a_glowcap_on_litter_alone_gains_tissue_and_the_pool_falls_by_what_it_took() {
    let mut world = pillars(4, &[1], 0.5);
    let mut flora = Flora::new(isolating());
    let site = plant_glowcap(&mut flora, &world, 1);
    let (organic, mineral, energy) = litter_on(&mut flora, site, 0.02);
    let before = *flora.view().stand_at(site).expect("planted");
    assert_eq!(dead_wood_at(&flora, site), 0.0, "the fixture has no wood");

    run(&mut flora, &mut world, 100);

    let v = flora.view();
    let g = v.ground_at(site).expect("ground").clone();
    let stand = *v.stand_at(site).expect("the fungus died on litter");
    let took = organic - g.litter;
    assert!(took > 0.0, "the fungus took nothing off a litter face");
    assert_eq!(
        dead_wood_at(&flora, site),
        0.0,
        "wood appeared from nowhere"
    );
    // Tissue: the whole point of the meal.
    assert!(
        stand.material() > before.material(),
        "no tissue gained: {} against {}",
        stand.material(),
        before.material()
    );
    // Organic: the pool's fall **is** the ledger's uptake, nothing else moved it.
    assert!(
        (took - uptake(&flora)).abs() <= 1e-12 * took,
        "the pool fell by {took} against an uptake of {}",
        uptake(&flora)
    );
    // Mineral and energy: the pool's own density, so the same fraction of each.
    let f = took / organic;
    let (took_mineral, took_energy) = (mineral - g.litter_mineral, energy - g.litter_energy);
    assert!(
        (took_mineral - f * mineral).abs() <= 1e-12 * f * mineral,
        "mineral fell by {took_mineral} against {} at the pool's density",
        f * mineral
    );
    assert!(
        (took_energy - f * energy).abs() <= 1e-12 * f * energy,
        "energy fell by {took_energy} against {} at the pool's density",
        f * energy
    );
    // And none of it was a boundary flow.
    assert_eq!(
        (
            v.ledger.consumed_organic_out,
            v.ledger.consumed_mineral_out,
            v.ledger.consumed_energy_out
        ),
        (0.0, 0.0, 0.0),
        "a stand inside the layer was booked as a consumer outside it"
    );
    assert_residuals(&flora, "after 100 ticks of a fungus on litter");
}

/// (b) **Two pools, one rate, split pro rata by stock.** The declared rule: dead wood and
/// litter are two pools in the same pro-rata draw, with **no preference** between them, so
/// a box holding three times as much wood as litter gives up three times as much wood, and
/// the two shares sum to the rate bound `substrate_uptake_per_s · W · μ · dt`.
///
/// One tick, because the rule is a per-tick one, and on a fixture where the demand is far
/// below both pools, so nothing but the split is being measured.
#[test]
fn the_draw_splits_pro_rata_between_the_two_pools_and_sums_to_the_rate_bound() {
    let mut world = pillars(4, &[1], 0.5);
    let mut flora = Flora::new(isolating());
    let site = plant_glowcap(&mut flora, &world, 1);
    let sc = flora.config().species(Species::Glowcap).clone();
    let (wood, litter) = (0.03, 0.01);
    log_on(&mut flora, site, wood);
    litter_on(&mut flora, site, litter);

    // Full moisture on this fixture, so the bound is the rate's own.
    let want = sc.substrate_uptake_per_s * 0.5 * sc.wood_max * DT;
    assert!((want - 5e-5).abs() < 1e-18, "the fixture's premise: {want}");
    assert!(
        want < 0.1 * litter.min(wood),
        "the demand has to be far below both pools"
    );

    flora.step(&mut world);

    let g = flora.view().ground_at(site).expect("ground").clone();
    let (took_wood, took_litter) = (wood - g.dead_wood, litter - g.litter);
    assert!(took_wood > 0.0 && took_litter > 0.0, "one pool was skipped");
    assert!(
        (took_wood + took_litter - want).abs() <= 1e-18,
        "the two shares are {took_wood} + {took_litter}, not {want}"
    );
    assert!(
        (took_wood / took_litter - wood / litter).abs() <= 1e-12,
        "the split is {took_wood}:{took_litter} against stocks {wood}:{litter}"
    );
    assert!(
        (uptake(&flora) - want).abs() <= 1e-18,
        "the diagnostic flux is {} against {want}",
        uptake(&flora)
    );
    assert_residuals(&flora, "after one tick on two pools");
}

/// (c) **A shredder and a fungus on one litter pool.** An animal biting the same litter
/// through `Flora::take_litter` and a glowcap drawing on it through step 5b together take
/// **exactly what the pool held and not a unit more** over 100 ticks, and the two ledgers
/// still close: this layer's three residuals hold, and the `consumed_*_out` this layer
/// books is to the bit the three numbers the shredder was handed and owes on its own
/// books. That identity is the whole of "the fauna ledger closes" as this crate can state
/// it — the fauna crate books the `Taken` it received and nothing else.
///
/// The demand is deliberately more than three times the stock, so the pool's bound is what
/// is being tested and not the rate's.
#[test]
fn a_shredder_and_a_glowcap_on_one_pool_take_no_more_than_it_held() {
    let mut world = pillars(4, &[1], 0.5);
    let mut flora = Flora::new(isolating());
    let site = plant_glowcap(&mut flora, &world, 1);
    let (organic, mineral, energy) = litter_on(&mut flora, site, 0.004);
    let bite = 1e-4;

    let (mut ate_o, mut ate_m, mut ate_e) = (0.0, 0.0, 0.0);
    for _ in 0..100 {
        flora.step(&mut world);
        let held = flora.view().ground_at(site).map_or(0.0, |g| g.litter);
        if let Some(t) = flora.take_litter(site, bite) {
            assert!(
                t.organic <= held + 1e-18,
                "a bite of {} off a pool holding {held}",
                t.organic
            );
            ate_o += t.organic;
            ate_m += t.mineral;
            ate_e += t.energy;
        }
    }

    let v = flora.view();
    let g = v.ground_at(site).expect("ground").clone();
    assert!(
        v.stand_at(site).is_some(),
        "the fungus died and shed a pool this test is counting"
    );
    assert!(ate_o > 0.0 && uptake(&flora) > 0.0, "one eater got nothing");
    // The two of them ate the pool, and the pool is what they ate. The hard bound is the
    // pool's own — `take_pool` hands over `min(want, stock)` and the stock below is
    // exactly zero, never negative — so the sum is checked to float dust and not to the
    // bit: two receipts summed in a different order than the withdrawals is a few ulps.
    assert!(
        ate_o + uptake(&flora) <= organic * (1.0 + 1e-12),
        "{ate_o} + {} came out of a pool of {organic}",
        uptake(&flora)
    );
    assert!(
        (ate_o + uptake(&flora) - organic).abs() <= 1e-12 * organic,
        "the pool was not emptied: {ate_o} + {} of {organic}",
        uptake(&flora)
    );
    assert_eq!(
        (g.litter, g.litter_mineral, g.litter_energy),
        (0.0, 0.0, 0.0),
        "an emptied pool kept dust"
    );
    // The boundary the two ledgers share: what the animal was handed is what this layer
    // booked out, to the bit, and the fungus's own meal is in neither number.
    assert_eq!(
        (
            v.ledger.consumed_organic_out,
            v.ledger.consumed_mineral_out,
            v.ledger.consumed_energy_out
        ),
        (ate_o, ate_m, ate_e),
        "the two ledgers disagree about what the shredder took"
    );
    assert!(
        ate_m < mineral && ate_e < energy,
        "the shredder took the fungus's share too"
    );
    assert_residuals(&flora, "after a shredder and a fungus shared a pool");
}

/// (d) **The substrate gate opens on litter.** A face whose mycelium box holds litter at
/// or above `establish_substrate_min` and **no dead wood at all** passes the establishment
/// predicate for a glowcap; a face holding less than the gate asks for does not, and says
/// so through `substrate_ok` alone. `Gates` reports the two stocks apart and gates on their
/// sum, which is what `Gates::substrate` is.
#[test]
fn the_substrate_gate_passes_on_litter_with_no_dead_wood() {
    let mut world = pillars(6, &[1, 4], 0.5);
    let mut flora = Flora::new(config());
    let sc = flora.config().species(Species::Glowcap).clone();
    let enough = 1.5 * sc.establish_substrate_min;
    let short = 0.5 * sc.establish_substrate_min;
    litter_on(&mut flora, at(1), enough);
    litter_on(&mut flora, at(4), short);

    let view = world.view();
    let v = flora.view();
    let rich = v.establishment_gates(&view, at(1), Species::Glowcap);
    let poor = v.establishment_gates(&view, at(4), Species::Glowcap);

    assert_eq!(rich.dead_wood, 0.0, "the fixture laid no wood");
    assert!((rich.litter - enough).abs() <= 1e-15, "{rich:?}");
    assert_eq!(rich.substrate(), rich.dead_wood + rich.litter);
    assert!(
        rich.substrate_ok && rich.passes(),
        "litter alone shut the gate: {rich:?}"
    );
    assert!(
        v.can_establish(&view, at(1), Species::Glowcap),
        "the predicate the tick runs disagrees with its own gates"
    );
    assert!(
        !poor.substrate_ok && !poor.passes(),
        "a face under the threshold admitted a spore: {poor:?}"
    );
    assert_eq!(
        (poor.pore_ok, poor.aeration_ok, poor.depth_ok, poor.light_ok),
        (rich.pore_ok, rich.aeration_ok, rich.depth_ok, rich.light_ok),
        "the two faces differ in something other than the substrate"
    );
    // And the three box readings agree with each other.
    assert_eq!(v.dead_wood_in_box(&view, at(1), &sc), 0.0);
    assert!((v.litter_in_box(&view, at(1), &sc) - enough).abs() <= 1e-15);
    assert!((v.substrate_in_box(&view, at(1), &sc) - enough).abs() <= 1e-15);
    // Dropping the same amount of *wood* on the poor face opens it, which is the sum rule
    // read from the other side.
    log_on(&mut flora, at(4), sc.establish_substrate_min);
    let mixed = flora
        .view()
        .establishment_gates(&view, at(4), Species::Glowcap);
    assert!(
        mixed.dead_wood > 0.0 && mixed.litter > 0.0 && mixed.substrate_ok,
        "wood plus litter did not reach the gate: {mixed:?}"
    );
    let _ = &mut world;
}

/// (e) **The five plants are unchanged.** None of them takes anything from litter: their
/// `substrate_uptake` is exactly zero after 100 ticks standing on a litter-bearing face,
/// and a litter pool on a **neighbouring** face — one inside a saprotroph's mycelium box,
/// and the one pool in the fixture nothing sheds onto — ends the run holding exactly what
/// the same pool holds in a world with no plant in it at all.
///
/// The plant's own face cannot be compared that way, because its litterfall lands there;
/// the neighbour can, and it is the face a fungus in its place would have eaten.
#[test]
fn the_five_photo_species_take_nothing_from_litter() {
    for species in Species::ALL {
        if species == Species::Glowcap {
            continue;
        }
        let (mut planted, mut bare) = (pillars(6, &[1, 2], 0.5), pillars(6, &[1, 2], 0.5));
        let (mut with_plant, mut alone) = (Flora::new(config()), Flora::new(config()));
        let sc = with_plant.config().species(species).clone();
        assert_eq!(sc.trophic, Trophic::Photo, "{species:?} is not a plant");
        assert!(with_plant.apply(
            &planted,
            Command::Seed {
                x: 1,
                z: 0,
                species,
                wood: 0.5 * sc.wood_max
            }
        ));
        litter_on(&mut with_plant, at(1), 0.02);
        litter_on(&mut with_plant, at(2), 0.02);
        litter_on(&mut alone, at(1), 0.02);
        litter_on(&mut alone, at(2), 0.02);

        run(&mut with_plant, &mut planted, 100);
        run(&mut alone, &mut bare, 100);

        let v = with_plant.view();
        assert_eq!(
            v.ledger.substrate_uptake[species.index()],
            0.0,
            "{species:?} ate litter"
        );
        assert!(
            v.stand_at(at(2)).is_none(),
            "{species:?} spread onto the neighbour face and the comparison is not clean"
        );
        let (neighbour, control) = (
            v.ground_at(at(2)).expect("ground").clone(),
            alone.view().ground_at(at(2)).expect("ground").clone(),
        );
        assert_eq!(
            (
                neighbour.litter,
                neighbour.litter_mineral,
                neighbour.litter_energy
            ),
            (
                control.litter,
                control.litter_mineral,
                control.litter_energy
            ),
            "{species:?} moved a litter pool next door"
        );
        assert_eq!(
            neighbour.dead_wood, 0.0,
            "{species:?} put wood on the neighbour face"
        );
        assert_residuals(&with_plant, "after 100 ticks of a plant on litter");
    }
}
