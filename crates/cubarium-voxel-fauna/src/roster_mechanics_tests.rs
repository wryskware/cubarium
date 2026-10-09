//! **Roster mechanics, written before the fixes**
//! (`design/handoffs/roster-mechanics-2026-10-02.md`, "Tests"): the owed items of the
//! roster brief that are wrong at any parameter value.
//!
//! 1–2. A glide never ends in water deeper than the glider wades: the end cell decides,
//!      not the take-off or the water overflown.
//! 3–4. A walker stops at the edge of water deeper than it wades, and a body already in
//!      such water heads for the shallows.
//! 5.   A latticevine seed voided from a gut enters a seed bank and can found a vine.
//! 6.   A hungry seedporter finds a fruiting stand out of its cone's 2 m.
//! 7–8. The film cue and the film taste read against the film's capacity on the world's
//!      own cell size (the coordinator's items 5 and 6).
//!
//! Hand-built worlds at 0.25 m unless named: soil `y = 1..=2`, the support face `y = 2`,
//! its top (the **surface**) 0.75 m. Water is laid by hand and the world is never stepped,
//! so a pond keeps its shape.

use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_flora::{
    Command as FloraCommand, Face, FaceDir, Flora, FloraConfig, Site, Species as Plant,
};

use crate::{
    Actions, Animal, Command, Fauna, FaunaConfig, Food, Founder, GutSeedLot, MobilityMode,
    Pose, Scripted, SeedPlant, StartingStores,
};

const V: f64 = 0.25;
const SP: Founder = Founder::Seedporter;
const LS: Founder = Founder::Loftstrider;
const SNAIL: Founder = Founder::RippleSnail;
/// Facing `+x`: `Pose::forward` is `(sin h, cos h)`.
const EAST: f64 = std::f64::consts::FRAC_PI_2;
const HUNGRY_ADULT: StartingStores = StartingStores {
    body: 1.0,
    reserve: 0.3,
};

// ------------------------------------------------------------------ fixtures

fn config(width: u32, height: u32, depth: u32, voxel_m: f64, hour: Option<f64>) -> VoxelConfig {
    VoxelConfig {
        width,
        height,
        depth,
        voxel_m,
        seed: 7,
        day: match hour {
            Some(h) => DayClock {
                start_hour: h,
                ..DayClock::default()
            },
            None => DayClock {
                mode: DayMode::Off,
                ..DayClock::default()
            },
        },
        ..VoxelConfig::default()
    }
}

/// Soil at `y = 1..=2` everywhere, at field capacity.
fn plain_with(c: VoxelConfig) -> World {
    let mut world = World::empty(c);
    let c = world.config().clone();
    let m = cubarium_voxel::Materials::DEFAULT;
    let fc = m.field_capacity(Material::Soil);
    let cap = m.pore_capacity(Material::Soil) * c.voxel_volume();
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            for y in 1..=2 {
                world.apply(WorldCommand::AddWater {
                    x,
                    y,
                    z,
                    volume_m3: fc * cap,
                });
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    world
}

fn plain(width: u32, height: u32, depth: u32) -> World {
    plain_with(config(width, height, depth, V, None))
}

/// Rock over columns `xs` (every `z`) at `ys`.
fn rock(world: &mut World, xs: std::ops::RangeInclusive<i64>, ys: std::ops::RangeInclusive<u32>) {
    let depth = world.config().depth;
    for z in 0..depth {
        for x in xs.clone() {
            for y in ys.clone() {
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Rock,
                });
            }
        }
    }
}

/// `cells` voxels of standing water on the plain's support face over columns `xs`, every
/// `z` (1.5 m is six cells).
fn water(world: &mut World, xs: std::ops::RangeInclusive<i64>, cells: u32) {
    let c = world.config().clone();
    let cell = c.voxel_volume();
    for z in 0..c.depth {
        for x in xs.clone() {
            for y in 3..3 + cells {
                world.apply(WorldCommand::AddWater { x, y, z, volume_m3: cell });
            }
        }
    }
    let d = world.view().water_depth_m(*xs.start(), 2, 0);
    assert!((d - f64::from(cells) * c.voxel_m).abs() < 1e-9, "fixture: {d} m of water");
}

fn centre(x: u32, z: u32) -> (f64, f64) {
    ((f64::from(x) + 0.5) * V, (f64::from(z) + 0.5) * V)
}

fn on_ground(fauna: &mut Fauna, world: &World, founder: Founder, x: i64, z: u32, heading_rad: f64, stores: StartingStores) -> u64 {
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

fn aloft(fauna: &mut Fauna, world: &World, x_m: f64, z_m: f64, y_m: f64) -> u64 {
    assert!(
        fauna.apply(
            world,
            Command::IntroduceAloft {
                pose: Pose {
                    x: x_m,
                    z: z_m,
                    heading_rad: EAST,
                    y_m,
                },
                founder: SP,
                stores: StartingStores::FULL,
            },
        ),
        "the fixture places a seedporter at ({x_m}, {y_m}, {z_m})"
    );
    fauna.view().ledger.births - 1
}

fn hold(fauna: &mut Fauna, id: u64, forward: f64, turn: f64, feed: f64) {
    assert!(fauna.set_controller(id, Box::new(Scripted::new(vec![Actions { forward, turn, feed }]))));
}

/// Drive a body by its lineage's own heuristic (an introduced body has no controller).
fn heuristic(fauna: &mut Fauna, id: u64, founder: Founder) {
    let teachers = fauna.config().teachers;
    assert!(fauna.set_controller(id, crate::controller::heuristic(founder, teachers)));
}

fn animal(fauna: &Fauna, id: u64) -> Animal {
    *fauna.view().animal(id).expect("the body is alive")
}

fn alive(fauna: &Fauna, id: u64) -> bool {
    fauna.view().animal(id).is_some()
}

fn depth_at(world: &World, s: Site) -> f64 {
    world.view().water_depth_m(i64::from(s.x), s.y, s.z)
}

fn wade(fauna: &Fauna, id: u64) -> f64 {
    let a = animal(fauna, id);
    let p = fauna.config().founder(a.founder.expect("a founder"));
    p.wade_depth_m(&p.body_at(a.body))
}

/// A full-grown stand of `species` on column `(x, z)`.
fn full_stand(flora: &mut Flora, world: &World, species: Plant, x: i64, z: u32) -> Site {
    let wood = flora.config().species(species).wood_max;
    assert!(flora.apply(world, FloraCommand::Seed { x, z, species, wood }));
    flora
        .view()
        .stands
        .iter()
        .find(|s| i64::from(s.site.x) == x && s.site.z == z)
        .expect("the stand")
        .site
}

fn crown_top(flora: &Flora, site: Site) -> f64 {
    let top = flora
        .view()
        .layers_at(site)
        .filter(|l| l.foliage_index.is_some())
        .map(|l| l.cell)
        .max()
        .expect("a crown");
    (top + 1) as f64 * V
}

// ------------------------------------------------------------------ 1–2. glides

/// The glide fixture: a rock plateau over `x = 0..=9` topped at `y = 12` (3.25 m), a full
/// lanternberry on it at `x = 7` (its crown top 4.5 m), optionally a rock island over
/// `island` topped at `y = 9` (2.5 m), and 1.5 m of water on the plain east of the
/// plateau over `pond` wherever the island is not. Returns the world, the plant layer and
/// the stand.
fn plateau_over_pond(
    pond: std::ops::RangeInclusive<i64>,
    island: Option<std::ops::RangeInclusive<i64>>,
) -> (World, Flora, Site) {
    let mut world = plain(96, 32, 8);
    rock(&mut world, 0..=9, 3..=12);
    if let Some(island) = island.clone() {
        rock(&mut world, island, 3..=9);
    }
    for x in pond {
        if island.as_ref().is_none_or(|i| !i.contains(&x)) {
            water(&mut world, x..=x, 6);
        }
    }
    let mut flora = Flora::new(FloraConfig::default());
    let site = full_stand(&mut flora, &world, Plant::Lanternberry, 7, 4);
    assert_eq!(site.y, 12, "fixture: the stand is on the plateau");
    (world, flora, site)
}

/// A seedporter perched on the plateau's crown, facing east, holding full forward.
fn perched_facing_east(world: &World, flora: &Flora, site: Site) -> (Fauna, u64) {
    let mut fauna = Fauna::new(FaunaConfig::default());
    let (x, z) = centre(site.x, site.z);
    let id = aloft(&mut fauna, world, x, z, crown_top(flora, site));
    hold(&mut fauna, id, 0.0, 0.0, 0.0);
    // An introduced glider's perch resolves on its first tick.
    let mut flora = flora.clone();
    fauna.step(world, &mut flora);
    hold(&mut fauna, id, 1.0, 0.0, 0.0);
    (fauna, id)
}

/// **Test 1. A glide whose only landing within range is 1.5 m deep water is refused; with
/// a dry cell in range it lands there.** Perched on a crown 4.5 m up at the plateau's east
/// edge, facing a pond that covers everything the glide could reach, it holds full forward
/// for 200 ticks and never glides: it stays perched, alive. The same perch with a rock
/// island rising out of the pond across the glide's path (`x = 24..=40`, topped 2.5 m,
/// above the water) glides and lands on the island, on a face no deeper than it wades.
#[test]
fn a_glide_with_only_deep_water_in_range_is_refused_and_a_dry_cell_in_range_is_landed_on() {
    // Only water in range.
    let (world, mut flora, site) = plateau_over_pond(10..=80, None);
    let (mut fauna, id) = perched_facing_east(&world, &flora, site);
    assert_eq!(animal(&fauna, id).mobility.mode, MobilityMode::Perched, "fixture: perched");
    for t in 0..200 {
        fauna.step(&world, &mut flora);
        assert!(alive(&fauna, id), "tick {t}: alive");
        assert_ne!(animal(&fauna, id).mobility.mode, MobilityMode::Glide, "tick {t}: the glide is refused");
    }
    assert_eq!(animal(&fauna, id).mobility.mode, MobilityMode::Perched, "it stays on its perch");

    // A dry island in range.
    let (world, mut flora, site) = plateau_over_pond(10..=80, Some(24..=40));
    let (mut fauna, id) = perched_facing_east(&world, &flora, site);
    let mut glided = false;
    for _ in 0..400 {
        fauna.step(&world, &mut flora);
        let m = animal(&fauna, id).mobility.mode;
        glided |= m == MobilityMode::Glide;
        if glided && m != MobilityMode::Glide {
            break;
        }
    }
    assert!(glided, "with a dry cell in range it glides");
    let a = animal(&fauna, id);
    assert_eq!(a.mobility.mode, MobilityMode::Ground, "it landed on the ground");
    assert!((24..=40).contains(&a.site.x) && a.site.y == 9, "on the island: {:?}", a.site);
    assert!(depth_at(&world, a.site) <= wade(&fauna, id), "its feet are dry enough");
}

/// **Test 2. Only the end cell decides.** From the plateau's ground edge (`x = 9`, facing
/// east), a glide starts out over 1.5 m of water (`x = 10..=15`; the glide is still above
/// the water's surface when it clears the far shore) toward dry plain beyond it: it glides (the water below the take-off does not refuse it) and lands on the dry
/// plain past the pond, where the water is no deeper than it wades.
#[test]
fn a_glide_across_a_pond_toward_dry_ground_lands_on_the_dry_ground() {
    let mut world = plain(96, 32, 8);
    rock(&mut world, 0..=9, 3..=12);
    water(&mut world, 10..=15, 6);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = on_ground(&mut fauna, &world, SP, 9, 4, EAST, StartingStores::FULL);
    assert_eq!(animal(&fauna, id).site.y, 12, "fixture: on the plateau");
    hold(&mut fauna, id, 1.0, 0.0, 0.0);
    let mut glided = false;
    for _ in 0..600 {
        fauna.step(&world, &mut flora);
        assert!(alive(&fauna, id), "alive");
        let m = animal(&fauna, id).mobility.mode;
        glided |= m == MobilityMode::Glide;
        if glided && m != MobilityMode::Glide {
            break;
        }
    }
    assert!(glided, "it glides out over the pond");
    let a = animal(&fauna, id);
    assert_eq!(a.mobility.mode, MobilityMode::Ground, "it landed");
    assert!(a.site.x > 15 && a.site.y == 2, "on the dry plain past the pond: {:?}", a.site);
    assert!(depth_at(&world, a.site) <= wade(&fauna, id), "dry enough");
}

// ------------------------------------------------------------------ 3–4. walkers

/// The loftstrider fixture: a plain 64 × 40 × 24 with 1.5 m of water over `x = 40..=63`
/// (the pond's west shore at 10 m).
fn shore() -> World {
    let mut world = plain(64, 40, 24);
    water(&mut world, 40..=63, 6);
    world
}

/// **Test 3. A loftstrider walking at a 1.5 m pond stops at the edge within one body
/// length, its feet never deeper than it wades** — head-on, and 30° off the shore normal.
#[test]
fn a_loftstrider_walking_at_a_deep_pond_stops_at_the_edge() {
    let shore_m = 40.0 * V;
    for heading in [EAST, EAST - std::f64::consts::FRAC_PI_6] {
        let world = shore();
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::default());
        let id = on_ground(&mut fauna, &world, LS, 20, 8, heading, HUNGRY_ADULT);
        hold(&mut fauna, id, 1.0, 0.0, 0.0);
        let wade_m = wade(&fauna, id);
        let length = fauna.config().founder(LS).body_at(animal(&fauna, id).body).length_m;
        for t in 0..200 {
            fauna.step(&world, &mut flora);
            assert!(alive(&fauna, id), "heading {heading}: tick {t}: alive");
            let a = animal(&fauna, id);
            assert!(
                depth_at(&world, a.site) <= wade_m + 1e-9,
                "heading {heading}: tick {t}: its feet are in {} m of water, it wades {wade_m}",
                depth_at(&world, a.site)
            );
        }
        let x = animal(&fauna, id).pose.x;
        assert!(x <= shore_m && shore_m - x <= length, "heading {heading}: stopped at {x}, shore {shore_m}, body {length}");
    }
}

/// **Test 4. A loftstrider in 1.5 m water reaches water no deeper than it wades within
/// 60 s, alive**, under its own heuristic. A basin of 1.5 m water over `x = 20..=44` on the
/// shore fixture's plain (dry either side); it is put down dry in the middle and the water
/// is laid round it.
#[test]
fn a_loftstrider_in_deep_water_heads_for_the_shallows() {
    let mut world = plain(64, 40, 24);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = on_ground(&mut fauna, &world, LS, 32, 12, 0.0, HUNGRY_ADULT);
    heuristic(&mut fauna, id, LS);
    water(&mut world, 20..=44, 6);
    let wade_m = wade(&fauna, id);
    assert!(depth_at(&world, animal(&fauna, id).site) > wade_m, "fixture: in deep water");
    let mut out = None;
    for t in 0..(60.0 / DT) as usize {
        fauna.step(&world, &mut flora);
        assert!(alive(&fauna, id), "tick {t}: alive");
        if depth_at(&world, animal(&fauna, id).site) <= wade_m {
            out = Some(t);
            break;
        }
    }
    assert!(out.is_some(), "it reached the shallows within 60 s");
}

// ------------------------------------------------------------------ 5. vine seeds

/// **Test 5. A gut-carried latticevine seed voided on a valid vine site enters the seed
/// bank and can found a vine, by the same rule as a lanternberry seed.** A seedporter at
/// the foot of a bare rock wall (`x = 6`), holding one vine seed package in its gut, due
/// now: the void banks it on its face (`banked`), and at the site's next bank check a vine
/// roots there on the wall face beside it. The plant layer's books close throughout. The
/// same seed voided on an open plain, with no wall to climb, stays banked and founds
/// nothing.
#[test]
fn a_voided_latticevine_seed_enters_the_bank_and_founds_a_vine() {
    for wall in [true, false] {
        let mut world = plain(16, 20, 5);
        if wall {
            rock(&mut world, 6..=6, 1..=12);
        }
        let mut fc = FloraConfig::default();
        fc.seed_check_s = 1.0;
        let mut flora = Flora::new(fc);
        let package = cubarium_voxel_flora::vine_seed_package(&flora.config().latticevine);
        assert!(package > 0.0, "a vine seed has a size");
        let mut fauna = Fauna::new(FaunaConfig::default());
        let id = on_ground(&mut fauna, &world, SP, 5, 2, EAST, StartingStores::FULL);
        hold(&mut fauna, id, 0.0, 0.0, 0.0);
        let here = animal(&fauna, id).site;
        assert_eq!(here, Site { x: 5, y: 2, z: 2 }, "fixture: at the wall's foot");
        let i = fauna.animals.iter().position(|a| a.id == id).expect("placed");
        let organic = 1.5 * package;
        fauna.animals[i].gut_seeds.lots[0] = GutSeedLot {
            plant: Some(SeedPlant::Latticevine),
            organic,
            mineral: 0.0,
            energy: flora.config().latticevine.energy_density * organic,
            source: here,
            opened_tick: 0,
            due_tick: 1,
        };
        let before = flora.view().organic();
        for _ in 0..3 {
            fauna.step(&world, &mut flora);
        }
        let voids = fauna.take_seed_voids();
        assert_eq!(voids.len(), 1, "wall {wall}: one void");
        assert!(voids[0].banked, "wall {wall}: the vine seed is banked");
        let banked = flora.view().ground_at(here).map_or(0.0, |g| g.vine_seed_organic());
        assert!((banked - organic).abs() <= 1e-12, "wall {wall}: {banked} banked of {organic}");
        assert!(
            (flora.view().organic() - before - organic).abs() <= 1e-12,
            "wall {wall}: the plant layer holds what was voided"
        );
        for _ in 0..(3.0 / DT) as usize {
            flora.step(&mut world);
        }
        let rooted = flora.view().cover.vines().iter().any(|v| v.root == here);
        assert_eq!(rooted, wall, "wall {wall}: a vine roots from the seed only beside a wall");
        if wall {
            assert!(
                flora.view().cover.face(Face::new(6, 3, 2, FaceDir::NegX)).is_some()
                    || flora.view().cover.vines().iter().any(|v| v.root == here),
                "the new vine covers the wall beside it"
            );
            let left = flora.view().ground_at(here).map_or(0.0, |g| g.vine_seed_organic());
            assert_eq!(left, 0.0, "the bank spent the seed");
        } else {
            let left = flora.view().ground_at(here).map_or(0.0, |g| g.vine_seed_organic());
            assert!(left > 0.0, "no wall: it waits in the bank");
        }
        let fv = flora.view();
        for (name, got, want) in [
            ("organic", fv.organic(), fv.ledger.expected_organic()),
            ("mineral", fv.mineral(), fv.ledger.expected_mineral()),
            ("energy", fv.energy(), fv.ledger.expected_energy()),
        ] {
            assert!((got - want).abs() <= 1e-9 * got.abs().max(1.0), "wall {wall}: flora {name} {got} vs {want}");
        }
    }
}

// ------------------------------------------------------------------ 6. finding fruit

/// **Test 6. A hungry seedporter 6 m from a fruiting lanternberry, with no fruit in its
/// 2 m cone at the start, reaches the fruit and eats within 120 s, in 8 of 8 placements
/// around the stand**, under its own heuristic. The stand is full-grown at the centre of
/// a 64 × 20 × 64 plain held at noon, given 60 ticks of fast fruit; each placement faces
/// away from it.
#[test]
fn a_hungry_seedporter_finds_a_fruiting_stand_6_m_away() {
    let mut world = plain_with(config(64, 20, 64, V, Some(12.0)));
    assert!(world.life_daylight() > 0.5, "fixture: noon");
    let mut fc = FloraConfig::default();
    fc.lanternberry.fruit_per_s = 1e-3;
    fc.lanternberry.fruit_cap = 2e-3;
    fc.lanternberry.fruit_drop_s = 1e9;
    fc.lanternberry.propagule_rate = 0.0;
    // F3's fruit alone (FLOWER-FRUIT): no flowering stage, no rest.
    fc.lanternberry.flower_s = 0.0;
    fc.lanternberry.fruit_rest_s = 0.0;
    let mut flora = Flora::new(fc);
    let site = full_stand(&mut flora, &world, Plant::Lanternberry, 32, 32);
    for _ in 0..60 {
        flora.step(&mut world);
    }
    assert!(flora.view().stand_at(site).expect("stand").fruit > 0.0, "fixture: fruit hangs");
    let (sx, sz) = centre(site.x, site.z);
    let mut misses = Vec::new();
    for k in 0..8 {
        let a = f64::from(k) * std::f64::consts::FRAC_PI_4;
        let (x, z) = (sx + 6.0 * a.sin(), sz + 6.0 * a.cos());
        let (cx, cz) = ((x / V).floor() as i64, (z / V).floor() as u32);
        let mut flora = flora.clone();
        let mut fauna = Fauna::new(FaunaConfig::default());
        // Facing away from the stand.
        let id = on_ground(&mut fauna, &world, SP, cx, cz, a, HUNGRY_ADULT);
        heuristic(&mut fauna, id, SP);
        let mut ate = false;
        for _ in 0..(120.0 / DT) as usize {
            fauna.step(&world, &mut flora);
            if !alive(&fauna, id) {
                break;
            }
            if fauna.view().ledger.bites_by_food[Food::Fruit.index()] > 0 {
                ate = true;
                break;
            }
        }
        if !ate {
            misses.push(k);
        }
    }
    assert!(misses.is_empty(), "placements that never ate: {misses:?}");
}

// ------------------------------------------------------------------ 7–8. film units

/// A plain at `voxel_m` under one cell of water, film seeded at `fill` of its capacity.
fn filmed(voxel_m: f64, fill: f64) -> (World, Flora) {
    let mut world = World::empty(config(16, 8, 6, voxel_m, None));
    for z in 0..6 {
        for x in 0..16 {
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
    for z in 0..6 {
        for x in 0..16 {
            world.apply(WorldCommand::AddWater { x, y: 3, z, volume_m3: cell });
        }
    }
    // The world's own cell size, as a live world's plant layer records it: the film's
    // grazing refuge is a share of this cell's capacity.
    let mut fc = FloraConfig::for_voxel_size(voxel_m);
    fc.film_seed_per_m2 = fill * fc.film_capacity_per_m2;
    let mut flora = Flora::new(fc);
    assert!(flora.seed_film(&world) > 0, "fixture: the faces are wet-eligible");
    (world, flora)
}

/// **Test 7. A film site emits `Chem(film)` by its edible share of capacity**: at full
/// capacity `1 − film_graze_refuge`, at half `0.5 − film_graze_refuge` (TASTE: the film
/// above the grazing refuge, what a mouth can take), per second of emission, before the
/// field's transport, on the 0.125 m and the 0.25 m grid alike.
#[test]
fn a_film_site_emits_by_its_share_of_capacity() {
    let refuge = FloraConfig::default().film_graze_refuge;
    assert!(refuge < 0.5, "fixture: half a bed is above the refuge");
    for voxel_m in [0.125, 0.25] {
        for (fill, want) in [(1.0, 1.0 - refuge), (0.5, 0.5 - refuge)] {
            let (world, flora) = filmed(voxel_m, fill);
            let s = Site { x: 4, y: 2, z: 2 };
            let e = crate::senses::film_emission(&world.view(), &flora.view(), s).expect("a source");
            assert!((e - want).abs() <= 1e-6, "{voxel_m} m grid, film at {fill}: emits {e}, want {want}");
        }
    }
}

/// **Test 8. `taste_reference` is derived from the world's cell**: on a 0.25 m and a
/// 0.125 m world the snail's reference is `film_capacity_per_m2 × cell area`, and a snail
/// on a full site reads a taste of its edible share before saturation (`c / (c +
/// chem_saturation)` in the observation, `c = 1 − film_graze_refuge`, TASTE) on both.
#[test]
fn the_snails_taste_reference_is_the_film_capacity_of_the_worlds_cell() {
    for voxel_m in [0.25, 0.125] {
        let (world, flora) = filmed(voxel_m, 1.0);
        let fc = flora.config().clone();
        let mut fauna = Fauna::new(FaunaConfig::default());
        let p = *fauna.config().founder(SNAIL);
        let reference = p.taste_reference_on(fauna.config().cue.emit_reference, voxel_m, &fc);
        let want = fc.film_capacity_per_m2 * voxel_m * voxel_m;
        assert!(
            (reference - want).abs() <= 1e-12,
            "{voxel_m} m: the reference is {reference}, capacity x cell area is {want}"
        );
        let id = on_ground(&mut fauna, &world, SNAIL, 4, 2, 0.0, StartingStores::FULL);
        hold(&mut fauna, id, 0.0, 0.0, 0.0);
        let i = fauna.animals.iter().position(|a| a.id == id).expect("placed");
        let manifest = fauna.config().manifest(SNAIL);
        let m = *SNAIL
            .manifest()
            .modules
            .iter()
            .find(|m| m.name.starts_with("Taste"))
            .expect("a taste module");
        let obs = crate::body::observation(&fauna, i, &world.view(), &flora.view(), &manifest, None, None);
        let sat = manifest.tunings.chem_saturation;
        // TASTE: a full site's edible film is `1 − film_graze_refuge` of its capacity.
        let c = 1.0 - fc.film_graze_refuge;
        let full = c / (c + sat);
        assert!(
            (obs[m.offset] - full).abs() <= 1e-6,
            "{voxel_m} m: a full site tastes {} (want {full}, a cue of {c})",
            obs[m.offset]
        );
    }
}

/// **A founder is never founded where its disc hangs off the strip** (roster mechanics fix
/// 2's root cause: the drowned loftstriders were all founded on `z = 0`, their 1.1 m disc
/// over the strip's end, where the motor could not move them; they starved in place in
/// 0.75 m of water until the shrinking body was under it). On a plain, `founding_faces`
/// for the loftstrider offers no face whose centre is nearer either `z` end than its
/// radius, and still offers the middle.
#[test]
fn founding_faces_keep_the_disc_inside_the_strip() {
    let world = plain(32, 40, 24);
    let fauna = Fauna::new(FaunaConfig::default());
    let p = fauna.config().founder(LS);
    let r = p.adult_body().footprint_radius();
    let depth_m = 24.0 * V;
    let faces = crate::founding_faces(&world.view(), p, 0.0);
    assert!(faces.iter().any(|f| f.z == 12), "the middle is offered");
    for f in &faces {
        let z = (f64::from(f.z) + 0.5) * V;
        assert!(z >= r - 1e-9 && z <= depth_m - r + 1e-9, "face {f:?}: centre {z} m, radius {r} m");
    }
}
