//! **Roster R2b, written before the rule** (`design/handoffs/voxel-roster-2026-10-01.md`,
//! Brief R2b "Tests first"): the ripple-snail, a slow water-bound grazer of glassfilm.
//! It takes film off the ground pool at its standing face and nothing else, never drowns,
//! keeps to the water's edge (`water_margin_m`), smells film on its own `Chem(film)` field,
//! and lays its eggs on film.
//!
//! In-crate because the `Chem` claim reads the observation vector (`body::observation`),
//! which is crate-private. Every fixture is a flat soil plain at 0.25 m — soil `y = 1..=2`,
//! the support face `y = 2` — with whole (or half) cells of water placed at `y = 3` over
//! chosen columns, on every `z`. **The world is never stepped**: `Fauna::step` reads the
//! water and does not move it, so the water holds still. Film is put on the wet faces by
//! the plant layer's own [`Flora::seed_film`] (R2a). No run is longer than 200 ticks.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraConfig, Site, Species as Plant,
};

use crate::{
    Actions, Command, Departure, Diet, EggSite, Fauna, FaunaConfig, Food, Founder,
    FounderPhysiology, Founding, FoundingSite, Hunter, Reproduction, Scripted, StartingStores,
};

const V: f64 = 0.25;
const SNAIL: Founder = Founder::RippleSnail;
/// Facing `+x`: `Pose::forward` is `(sin h, cos h)`.
const EAST: f64 = std::f64::consts::FRAC_PI_2;
/// Hungry enough to bite, with room in the reserve.
const HUNGRY_ADULT: StartingStores = StartingStores {
    body: 1.0,
    reserve: 0.5,
};

/// A flat soil plain `width × 8 × depth` at 0.25 m, day clock off, with `cells(x)` of
/// water standing on the face of column `x` on every `z` (a whole cell is 0.25 m; a
/// fraction is that fraction of it).
fn pond(width: u32, depth: u32, cells: impl Fn(u32) -> f64) -> World {
    let mut world = World::empty(VoxelConfig {
        width,
        height: 8,
        depth,
        voxel_m: V,
        day: cubarium_voxel::weather::DayClock {
            mode: cubarium_voxel::weather::DayMode::Off,
            ..Default::default()
        },
        ..VoxelConfig::default()
    });
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=2 {
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    let cell = world.config().voxel_volume();
    for z in 0..depth {
        for x in 0..width {
            let mut left = cells(x);
            let mut y = 3;
            while left > 0.0 {
                let share = left.min(1.0);
                world.apply(WorldCommand::AddWater {
                    x: i64::from(x),
                    y,
                    z,
                    volume_m3: share * cell,
                });
                left -= share;
                y += 1;
            }
        }
    }
    for z in 0..depth {
        for x in 0..width {
            let d = world.view().standing_depth_m(i64::from(x), 2, z);
            assert!(
                (d - cells(x) * V).abs() < 1e-9,
                "fixture: column {x} stands {d} m of water, want {} m",
                cells(x) * V
            );
        }
    }
    world
}

/// No water at all.
fn dry(width: u32, depth: u32) -> World {
    pond(width, depth, |_| 0.0)
}

fn site(x: u32, z: u32) -> Site {
    Site { x, y: 2, z }
}

fn film_at(flora: &Flora, s: Site) -> f64 {
    flora.view().film_at(s)
}

fn litter_at(flora: &Flora, s: Site) -> f64 {
    flora.view().ground_at(s).map_or(0.0, |g| g.litter)
}

fn deposit(flora: &mut Flora, s: Site, kind: DepositKind, organic: f64) {
    assert!(flora.deposit(
        s,
        Deposit {
            kind,
            organic,
            mineral: organic * 0.02,
            energy: organic * 2.0,
        },
    ));
}

/// Introduce a founder at a column; returns its id.
fn introduce(
    fauna: &mut Fauna,
    world: &World,
    founder: Founder,
    x: i64,
    z: u32,
    heading_rad: f64,
    stores: StartingStores,
) -> u64 {
    assert!(
        fauna.apply(
            world,
            Command::IntroduceFounder {
                x,
                z,
                founder,
                stores,
                heading_rad,
            },
        ),
        "the fixture places a {founder:?} at ({x}, {z})"
    );
    fauna.view().ledger.births - 1
}

/// Hold one scripted action every tick.
fn hold(fauna: &mut Fauna, id: u64, forward: f64, feed: f64) {
    assert!(fauna.set_controller(
        id,
        Box::new(Scripted::new(vec![Actions {
            forward,
            turn: 0.0,
            feed,
        }])),
    ));
}

/// Introduce a founder holding a full feed action every tick.
fn feeding(fauna: &mut Fauna, world: &World, founder: Founder, x: i64, z: u32) -> u64 {
    let id = introduce(fauna, world, founder, x, z, 0.0, HUNGRY_ADULT);
    hold(fauna, id, 0.0, 1.0);
    id
}

fn steps(fauna: &mut Fauna, world: &World, flora: &mut Flora, n: usize) {
    for _ in 0..n {
        fauna.step(world, flora);
    }
}

/// Both layers' books close, and what the plant layer lost is what the animal booked
/// eating minus what it put back.
fn ledgers_close(fauna: &Fauna, flora: &Flora, flora_organic_before: f64) {
    let av = fauna.view();
    let fv = flora.view();
    for (name, got, want) in [
        ("fauna organic", av.organic(), av.ledger.expected_organic()),
        ("fauna mineral", av.mineral(), av.ledger.expected_mineral()),
        ("fauna energy", av.energy(), av.ledger.expected_energy()),
        ("flora organic", fv.organic(), fv.ledger.expected_organic()),
        ("flora mineral", fv.mineral(), fv.ledger.expected_mineral()),
        ("flora energy", fv.energy(), fv.ledger.expected_energy()),
    ] {
        assert!(
            (got - want).abs() <= 1e-9 * got.abs().max(1.0),
            "the {name} ledger must close: {got} against {want}"
        );
    }
    let lost = flora_organic_before - fv.organic();
    let net = av.ledger.eaten_organic_in - av.ledger.deposited_organic_out;
    assert!(
        (lost - net).abs() <= 1e-12 * lost.abs().max(1.0),
        "the plant layer lost {lost} and the animal booked {net}"
    );
}

fn index_of(fauna: &Fauna, id: u64) -> usize {
    fauna
        .animals
        .iter()
        .position(|a| a.id == id)
        .expect("the body is alive")
}

fn module(founder: Founder, prefix: &str) -> crate::Module {
    *founder
        .manifest()
        .modules
        .iter()
        .find(|m| m.name.starts_with(prefix))
        .unwrap_or_else(|| panic!("{founder:?} has a {prefix} module"))
}

/// The body's centre column.
fn column_of(fauna: &Fauna, id: u64) -> u32 {
    fauna.view().animal(id).expect("alive").site.x
}

// ------------------------------------------------------------------ the decisions

/// The R2b table (Fable's decisions, brief R2b): the lineage's name and role; the body,
/// pace and film diet; the yield the browser's foliage yield; the shredder's table scaled
/// by body volume exactly as the capgnawer's is, with the shredder's reproduction mode
/// and timings; `aquatic` and `water_margin_m` 0.25; eggs on film where the shredder and
/// the capgnawer keep litter; founding at food, count 0 by default; on no hunter's list;
/// its own manifest at the shredder's width with `Chem(film)` in the `Chem` slot.
#[test]
fn the_r2b_physiology_table_is_the_briefs() {
    let shredder = FounderPhysiology::frozen(Founder::Blind);
    let grazer = FounderPhysiology::frozen(Founder::Browser);
    let gnawer = FounderPhysiology::frozen(Founder::Capgnawer);
    let snail = FounderPhysiology::frozen(SNAIL);
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-9 * b.abs().max(1e-12);

    assert_eq!(SNAIL.name(), "ripple-snail", "the specimen key");
    assert_eq!(SNAIL.role(), "film_grazer");
    assert!(Founder::ALL.contains(&SNAIL));
    assert!(Food::ALL.contains(&Food::Film) && Food::Film.is_pool(), "a ground pool");
    assert_eq!(Food::Film.name(), "film");

    // Food.
    assert_eq!(snail.diet, Diet::only(Food::Film));
    assert!(
        close(snail.yield_for(Food::Film), grazer.yield_for(Food::Foliage)),
        "fresh tissue: the browser's foliage yield"
    );

    // Body and pace.
    assert_eq!(
        (snail.adult_length_m, snail.adult_width_m, snail.adult_height_m),
        (0.19, 0.19, 0.063)
    );
    assert!(close(snail.cruise_body_lengths_per_s, 0.3));

    // The shredder's table by body volume, as the capgnawer's is built.
    let r = (0.19 * 0.19 * 0.063) / (0.375 * 0.125 * 0.125);
    for (what, got, want) in [
        ("body_max", snail.core.body_max, shredder.core.body_max * r),
        ("body_min", snail.core.body_min, shredder.core.body_min * r),
        ("birth_body", snail.core.birth_body, shredder.core.birth_body * r),
        ("bite_per_s", snail.core.bite_per_s, shredder.core.bite_per_s * r),
        ("growth_max_per_s", snail.core.growth_max_per_s, shredder.core.growth_max_per_s * r),
        ("egg_organic", snail.core.reproduction.egg_organic, shredder.core.reproduction.egg_organic * r),
        // Per unit of body: the specific rates are the shredder's.
        ("maintenance_per_s", snail.core.maintenance_per_s, shredder.core.maintenance_per_s),
        ("motor_respiration_per_s", snail.motor_respiration_per_s, shredder.motor_respiration_per_s),
        ("reserve_cap", snail.core.reserve_cap, shredder.core.reserve_cap),
    ] {
        assert!(close(got, want), "{what}: {got} against {want}");
    }
    let (s, t) = (snail.core.reproduction, shredder.core.reproduction);
    assert_eq!(s.mode, t.mode, "the shredder's reproduction mode");
    assert_eq!(
        (s.surplus_hold_s, s.birth_interval_s, s.incubation_s, s.clutch_size, s.dormant),
        (t.surplus_hold_s, t.birth_interval_s, t.incubation_s, t.clutch_size, t.dormant),
        "and its timings"
    );
    assert_eq!(snail.core.lifespan_median_s, shredder.core.lifespan_median_s);

    // Aquatic.
    assert!(snail.aquatic, "water-bound");
    assert!(close(snail.water_margin_m, 0.25));
    for f in Founder::ALL.into_iter().filter(|&f| f != SNAIL) {
        assert!(!FounderPhysiology::frozen(f).aquatic, "{f:?} is not aquatic");
    }

    // Eggs.
    assert_eq!(snail.egg_site, EggSite::Film);
    assert_eq!(shredder.egg_site, EggSite::Litter);
    assert_eq!(gnawer.egg_site, EggSite::Litter);

    // Founding.
    assert_eq!((snail.founding, snail.founding_site), (Founding::Bodies, FoundingSite::AtFood));
    assert_eq!(snail.founding_count, 0, "a new lineage founds none by default");
    assert_eq!(FaunaConfig::default().founder_counts()[SNAIL.index()], 0);

    // Prey: on no hunter's list.
    assert!(snail.hunter.is_none());
    assert!(!Hunter::LANTERNJAW.hunts(SNAIL));
    for f in Founder::ALL {
        if let Some(h) = FounderPhysiology::frozen(f).hunter {
            assert!(!h.hunts(SNAIL), "{f:?} does not hunt the snail");
        }
    }

    // Its own manifest, the shredder's width, `Chem(film)` in the shredder's `Chem` slot.
    let blind = module(Founder::Blind, "Chem");
    let film = module(SNAIL, "Chem");
    assert_eq!(film.name, "Chem(film)");
    assert_eq!((film.offset, film.width), (blind.offset, blind.width), "the same slot");
    assert_eq!(SNAIL.manifest().inputs(), Founder::Blind.manifest().inputs());
    assert_ne!(SNAIL.manifest().digest(), Founder::Blind.manifest().digest());
    assert_ne!(SNAIL.manifest().digest(), Founder::Capgnawer.manifest().digest());
    assert!(module_opt_wet(SNAIL), "`Wet` is in the layout");
}

fn module_opt_wet(founder: Founder) -> bool {
    founder.manifest().modules.iter().any(|m| m.name == "Wet")
}

// ------------------------------------------------------------------ feeding

/// **A snail on a submerged film site takes film, the books close, and the film pool
/// falls.** A pond one cell deep over the whole plain, film seeded on every face; a
/// feeding snail standing still bites film off its own face, the plant layer's
/// `film_consumed` books what it gave, and both ledgers close.
#[test]
fn a_snail_on_submerged_film_takes_film_the_books_close_and_the_pool_falls() {
    let world = pond(16, 6, |_| 1.0);
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.seed_film(&world) > 0, "the pond's faces are wet-eligible");
    let face = site(4, 2);
    let film = film_at(&flora, face);
    assert!(film > 0.0, "film on the snail's face");
    let organic = flora.view().organic();
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = feeding(&mut fauna, &world, SNAIL, 4, 2);
    assert_eq!(fauna.view().animal(id).expect("placed").site, face);

    steps(&mut fauna, &world, &mut flora, 20);
    let l = &fauna.view().ledger;
    assert!(l.bites_by_food[Food::Film.index()] >= 1, "a film bite: {:?}", l.bites_by_food);
    assert!(l.bites_by_founder[SNAIL.index()] >= 1);
    assert!(l.eaten_by_food[Food::Film.index()] > 0.0);
    assert!(l.assimilated_by_founder[SNAIL.index()] > 0.0, "it built tissue out of it");
    assert!(film_at(&flora, face) < film, "the film pool fell");
    assert!(flora.view().ledger.film_consumed > 0.0, "the plant layer booked it consumed");
    assert!(
        (flora.view().ledger.film_consumed - l.eaten_by_food[Food::Film.index()]).abs()
            <= 1e-12 * film,
        "what the film gave is what the snail booked eating"
    );
    ledgers_close(&fauna, &flora, organic);
}

/// **It never takes litter or foliage.** On a dry plain, litter at its face and a
/// velvetpad (ground cover, inside its band) at its column: a feeding snail bites nothing
/// and both are untouched. A shredder in its place eats the litter, so the pile is in
/// reach.
#[test]
fn a_snail_never_takes_litter_or_foliage() {
    let world = dry(16, 6);
    let mut flora = Flora::new(FloraConfig::default());
    let face = site(4, 2);
    let wood = flora.config().species(Plant::Velvetpad).wood_max;
    assert!(flora.apply(
        &world,
        FloraCommand::Seed {
            x: 4,
            z: 2,
            species: Plant::Velvetpad,
            wood,
        },
    ));
    let pad = flora.view().stand_at(face).expect("the pad").foliage;
    assert!(pad > 0.0);
    deposit(&mut flora, face, DepositKind::Litter, 1e-3);
    let litter = litter_at(&flora, face);

    let mut fauna = Fauna::new(FaunaConfig::default());
    let _ = feeding(&mut fauna, &world, SNAIL, 4, 2);
    steps(&mut fauna, &world, &mut flora, 20);
    let l = &fauna.view().ledger;
    assert_eq!(l.bites, 0, "neither is its food: {:?}", l.bites_by_food);
    assert_eq!(litter_at(&flora, face), litter, "the litter is untouched");
    assert_eq!(flora.view().stand_at(face).expect("the pad").foliage, pad, "the pad too");

    let mut shredders = Fauna::new(FaunaConfig::default());
    let _ = feeding(&mut shredders, &world, Founder::Blind, 4, 2);
    steps(&mut shredders, &world, &mut flora, 20);
    assert!(
        shredders.view().ledger.bites_by_food[Food::Litter.index()] >= 1,
        "the fixture's litter is in a mouth's reach"
    );
    // The pad's foliage stands in the snail's own band: the first cell over its face.
    let band = fauna.config().founder(SNAIL).adult_body().mouth_layers(2, V);
    assert!(
        flora.view().layers_at(face).any(|l| l.stock > 0.0 && band.contains(&l.cell)),
        "the pad is in the snail's mouth band {band:?}"
    );
}

// ------------------------------------------------------------------ water

/// **It does not drown after `drown_after_s` under water, where a shredder in the same
/// place does.** Half a metre of water over the whole plain, deeper than either body;
/// both lineages' `drown_after_s` cut to 1 s so the window is short. After 3 s the
/// shredder has drowned and the snail is alive.
#[test]
fn a_snail_does_not_drown_where_a_shredder_does() {
    let world = pond(16, 6, |_| 2.0);
    let mut flora = Flora::new(FloraConfig::default());
    let mut config = FaunaConfig::default();
    for f in [Founder::Blind, SNAIL] {
        config.founders[f.index()].drown_after_s = 1.0;
    }
    let mut fauna = Fauna::new(config);
    let shredder = introduce(&mut fauna, &world, Founder::Blind, 3, 2, 0.0, StartingStores::FULL);
    let snail = introduce(&mut fauna, &world, SNAIL, 10, 2, 0.0, StartingStores::FULL);
    hold(&mut fauna, shredder, 0.0, 0.0);
    hold(&mut fauna, snail, 0.0, 0.0);
    for (id, f) in [(shredder, Founder::Blind), (snail, SNAIL)] {
        let a = fauna.view().animal(id).expect("placed");
        let phys = fauna.config().founder(f);
        assert!(
            world.view().water_depth_m(i64::from(a.site.x), a.site.y, a.site.z)
                > phys.drown_depth_m(&phys.body_at(a.body)),
            "{f:?} is under water"
        );
    }

    let ticks = (3.0 / DT).round() as usize;
    steps(&mut fauna, &world, &mut flora, ticks);
    let v = fauna.view();
    assert!(v.animal(shredder).is_none(), "the shredder drowned");
    assert_eq!(v.ledger.deaths_by_cause[Departure::Drowned.index()], 1, "one drowning");
    assert!(v.animal(snail).is_some(), "the snail lives under water");
}

/// **It refuses a step onto dry ground beyond `water_margin_m`, and takes one within the
/// margin.** Water one cell deep over columns 0..=5, dry ground from column 6 on; the
/// margin is 0.25 m, one column at 0.25 m. A snail starting at column 2 under water,
/// holding full forward east for 200 ticks, walks through the water (an aquatic body
/// pays no wade refusal), onto column 6 (dry, its face 0.25 m from the water), and never
/// onto column 7 (0.5 m from it). Its cruise is raised for the test so the walk fits in
/// the window; the rule is about where, not how fast.
///
/// "Within the margin" is read here as the distance between face centres: column 6's
/// face is one column, 0.25 m, from column 5's water.
#[test]
fn a_snail_refuses_dry_ground_beyond_its_margin_and_takes_it_within() {
    let wet = |x: u32| if x <= 5 { 1.0 } else { 0.0 };
    let world = pond(16, 6, wet);
    let mut flora = Flora::new(FloraConfig::default());
    let mut config = FaunaConfig::default();
    config.founders[SNAIL.index()].cruise_body_lengths_per_s = 2.0;
    let margin = config.founder(SNAIL).water_margin_m;
    assert!((margin - 0.25).abs() < 1e-12, "the knob's default");
    let mut fauna = Fauna::new(config);
    let id = introduce(&mut fauna, &world, SNAIL, 2, 2, EAST, HUNGRY_ADULT);
    hold(&mut fauna, id, 1.0, 0.0);

    let mut visited = vec![column_of(&fauna, id)];
    for _ in 0..200 {
        fauna.step(&world, &mut flora);
        let c = column_of(&fauna, id);
        if visited.last() != Some(&c) {
            visited.push(c);
        }
    }
    assert!(
        visited.contains(&5),
        "it walks through the water to the edge: {visited:?}"
    );
    assert!(visited.contains(&6), "and onto dry ground within the margin: {visited:?}");
    assert!(
        visited.iter().all(|&c| c <= 6),
        "but never onto dry ground beyond it: {visited:?}"
    );
    assert_eq!(column_of(&fauna, id), 6, "it ends held at the margin");
}

// ------------------------------------------------------------------ the film field

/// The `Chem` response (validity 1) of body `i`, after the field settled.
fn chem(fauna: &Fauna, i: usize, world: &World, flora: &Flora, senses: &mut crate::Senses) -> f64 {
    let founder = fauna.animals[i].founder.expect("a founder");
    let m = module(founder, "Chem");
    let manifest = fauna.config().manifest(founder);
    let (view, fv) = (world.view(), flora.view());
    let obs = crate::body::observation(fauna, i, &view, &fv, &manifest, Some(senses), None);
    assert_eq!(obs[m.offset + 2], 1.0, "{founder:?}: the chem receptor is valid");
    obs[m.offset]
}

/// **Its `Chem` rises toward film and reads 0 where there is none.** Film on a pond of
/// columns 10..=12: a snail one column from it reads more than one five columns off, and
/// more than zero. On a dry plain with only a litter pile, a snail next to it reads zero
/// (validly) where a shredder in the same place reads the pile.
#[test]
fn a_snail_chem_rises_toward_film_and_reads_zero_where_there_is_none() {
    let world = pond(20, 6, |x| if (10..=12).contains(&x) { 1.0 } else { 0.0 });
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.seed_film(&world) > 0, "film on the pond");
    let mut fauna = Fauna::new(FaunaConfig::default());
    let near = introduce(&mut fauna, &world, SNAIL, 9, 2, EAST, HUNGRY_ADULT);
    let far = introduce(&mut fauna, &world, SNAIL, 4, 2, EAST, HUNGRY_ADULT);
    let mut senses = crate::Senses::new();
    let (updates, converged) = senses.settle(&world.view(), &flora.view());
    assert!(converged, "the fields settled in {updates} updates");
    let (i_near, i_far) = (index_of(&fauna, near), index_of(&fauna, far));
    let q_near = chem(&fauna, i_near, &world, &flora, &mut senses);
    let q_far = chem(&fauna, i_far, &world, &flora, &mut senses);
    assert!(q_near > 0.0, "film one column away is smelled: {q_near}");
    assert!(q_near > q_far, "it rises toward the film: near {q_near}, far {q_far}");

    // No film anywhere: a litter pile only.
    let world = dry(20, 6);
    let mut flora = Flora::new(FloraConfig::default());
    deposit(&mut flora, site(10, 2), DepositKind::Litter, 0.2);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let snail = introduce(&mut fauna, &world, SNAIL, 9, 2, EAST, HUNGRY_ADULT);
    let mut senses = crate::Senses::new();
    let (updates, converged) = senses.settle(&world.view(), &flora.view());
    assert!(converged, "the fields settled in {updates} updates");
    let i = index_of(&fauna, snail);
    assert_eq!(chem(&fauna, i, &world, &flora, &mut senses), 0.0, "no film, no film cue");
    let mut shredders = Fauna::new(FaunaConfig::default());
    let shredder = introduce(&mut shredders, &world, Founder::Blind, 9, 2, EAST, HUNGRY_ADULT);
    let i = index_of(&shredders, shredder);
    assert!(
        chem(&shredders, i, &world, &flora, &mut senses) > 0.0,
        "the fixture's pile is on the detritus field"
    );
}

// ------------------------------------------------------------------ eggs

/// **Its eggs go only on a film face.** Half a cell of water (0.125 m) over columns
/// 0..=3, film seeded there; litter on dry column 8. Two snails ready to lay on the first
/// tick (a one-tick hold, a one-egg clutch of `body_min`, a reserve that holds it): the
/// one standing on film lays there, and the one standing on litter lays nothing and pays
/// nothing.
#[test]
fn a_snail_lays_only_on_a_film_face() {
    let world = pond(16, 6, |x| if x <= 3 { 0.5 } else { 0.0 });
    let mut flora = Flora::new(FloraConfig::default());
    assert!(flora.seed_film(&world) > 0, "film on the shallows");
    let (wet, bed) = (site(2, 2), site(8, 2));
    assert!(film_at(&flora, wet) > 0.0 && litter_at(&flora, wet) == 0.0, "film, no litter");
    deposit(&mut flora, bed, DepositKind::Litter, 0.2);
    assert_eq!(film_at(&flora, bed), 0.0, "litter, no film");

    let mut config = FaunaConfig::default();
    {
        let core = &mut config.founders[SNAIL.index()].core;
        core.reserve_cap = 1.0;
        core.reproduction = Reproduction {
            surplus_floor: 0.0,
            surplus_hold_s: DT,
            birth_interval_s: 20.0 * DT,
            clutch_size: 1,
            egg_organic: core.body_min,
            incubation_s: 100.0 * DT,
            ..core.reproduction
        };
        assert_eq!(core.reproduction.mode, crate::BirthMode::Eggs, "an egg-layer");
    }
    let mut fauna = Fauna::new(config);
    let on_film = introduce(&mut fauna, &world, SNAIL, 2, 2, 0.0, StartingStores::FULL);
    let on_litter = introduce(&mut fauna, &world, SNAIL, 8, 2, 0.0, StartingStores::FULL);
    hold(&mut fauna, on_film, 0.0, 0.0);
    hold(&mut fauna, on_litter, 0.0, 0.0);

    steps(&mut fauna, &world, &mut flora, 1);
    let v = fauna.view();
    let snails: Vec<_> = v.clutches.iter().filter(|c| c.lineage == SNAIL).collect();
    assert_eq!(snails.len(), 1, "one clutch: {:?}", v.clutches);
    assert_eq!(snails[0].site, wet, "on the film");
    assert!(film_at(&flora, snails[0].site) > 0.0);
    assert_eq!(v.ledger.eggs_laid_by_founder[SNAIL.index()], 1);
    let a = v.animal(on_litter).expect("alive");
    assert_eq!(a.reproduction.refractory_ticks, 0, "the litter is no place to lay");
}
