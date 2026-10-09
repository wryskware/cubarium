//! **FLOWER-FRUIT**, written from the brief before the rule (Wrysk's decisions of
//! 2026-10-09, `design/training-plan-2026-10-08.md` §6 items 19–20):
//!
//! - A crop eaten out does not refill before its rest; a dropped crop rests as before.
//! - A unit of edible fruit costs the stand more than a unit of reserve, the overhead is
//!   respired, and the books close.
//! - A flowering stand no pollinator visits sets fruit only at the floor; one visited sets
//!   more, and the visit is booked to that stand. The same for a latticevine flower.
//! - A latticevine bunch eaten out does not refill before its face's next flowering.
//!
//! Every fixture sets the phase lengths and rates it needs; the shipped numbers are read
//! from the config, never asserted (beyond the spec's own inequalities: a floor below a full
//! crop, a cost above par).

use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_flora::{
    Command, CropPhase, Face, FaceDir, Flora, FloraConfig, Site, Species, SpurPhase, VineSeed, pollinated_share,
};

fn ticks(seconds: f64) -> usize {
    (seconds / DT).round() as usize
}

/// A wet soil plain `w × 12 × d` at 0.25 m, the day clock held at noon (never stepped), the
/// solids in `cells` over it.
fn plain(w: u32, d: u32, cells: &[(i64, u32, u32)]) -> World {
    let mut world = World::empty(VoxelConfig {
        width: w,
        height: 18,
        depth: d,
        voxel_m: 0.25,
        seed: 5,
        day: DayClock { start_hour: 12.0, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    let m = cubarium_voxel::Materials::DEFAULT;
    let cap = m.pore_capacity(Material::Soil) * world.config().voxel_volume();
    let fc = m.field_capacity(Material::Soil);
    for z in 0..d {
        for x in 0..i64::from(w) {
            for y in 1..=2 {
                world.apply(WorldCommand::AddWater { x, y, z, volume_m3: fc * cap });
                world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    for &(x, y, z) in cells {
        world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Rock });
    }
    assert!(world.life_daylight() > 0.9, "fixture: noon");
    world
}

/// The shipped flora with the plant clock every tick and lanternberries on a short cycle:
/// flowers `flower_s`, fruit fast (full in 2 s), falling after `drop_s`, resting `rest_s` on
/// average, no seed parcel and no drinking (the plain never dries).
fn config(flower_s: f64, drop_s: f64, rest_s: f64) -> FloraConfig {
    let mut c = FloraConfig::default();
    c.step_every = 1;
    let l = &mut c.lanternberry;
    l.flower_s = flower_s;
    l.fruit_per_s = 1e-3;
    l.fruit_cap = 2e-3;
    l.fruit_drop_s = drop_s;
    l.fruit_rest_s = rest_s;
    l.propagule_rate = 0.0;
    l.transpiration_m3_per_s = 0.0;
    c
}

fn bush(flora: &mut Flora, world: &World, x: i64, z: u32) -> Site {
    let wood = flora.config().lanternberry.wood_max;
    assert!(flora.apply(world, Command::Seed { x, z, species: Species::Lanternberry, wood }));
    Site { x: x as u32, y: 2, z }
}

fn stand(flora: &Flora, site: Site) -> cubarium_voxel_flora::Stand {
    *flora.view().stand_at(site).expect("the bush lives")
}

/// Step until `done` or `cap_s` seconds; the seconds taken.
fn until(flora: &mut Flora, world: &mut World, cap_s: f64, done: impl Fn(&Flora) -> bool) -> f64 {
    let mut n = 0;
    while !done(flora) {
        assert!(n < ticks(cap_s), "not within {cap_s} s");
        flora.step(world);
        n += 1;
    }
    n as f64 * DT
}

fn closes(flora: &Flora, when: &str) {
    let v = flora.view();
    for (name, got, want) in [
        ("organic", v.organic(), v.ledger.expected_organic()),
        ("mineral", v.mineral(), v.ledger.expected_mineral()),
        ("energy", v.energy(), v.ledger.expected_energy()),
    ] {
        assert!((got - want).abs() <= 1e-9 * got.abs().max(1.0), "{when}: the {name} ledger must close: {got} against {want}");
    }
}

/// A crop fully made and hanging.
fn made(f: &Flora, s: Site) -> bool {
    let st = stand(f, s);
    st.crop.phase == CropPhase::Fruit && st.crop.to_make == 0.0 && st.fruit > 0.0
}

// ------------------------------------------------------------------ 1. a picked-clean crop rests

/// **Eaten out, a crop rests like a dropped one.** A bush whose crop is made and hanging is
/// eaten out: at the next plant tick the crop is over, a rest is drawn, and no flower and no
/// fruit come until it has run. A bite that leaves fruit ends nothing. Left alone, the next
/// crop falls at `fruit_drop_s` and rests the same way.
#[test]
fn a_crop_eaten_out_does_not_refill_before_its_rest_and_a_dropped_crop_rests_as_before() {
    let mut w = plain(12, 8, &[]);
    let mut c = config(5.0, 60.0, 100.0);
    c.lanternberry.pollination_floor = 1.0;
    let mut flora = Flora::new(c);
    let s = bush(&mut flora, &w, 5, 4);
    until(&mut flora, &mut w, 60.0, |f| made(f, s));

    // A bite that leaves fruit: the crop goes on.
    let half = stand(&flora, s).fruit / 2.0;
    assert!(flora.take_stand_fruit(s, half).is_some());
    flora.step(&mut w);
    let st = stand(&flora, s);
    assert_eq!(st.crop.phase, CropPhase::Fruit, "a crop with fruit left goes on");
    assert_eq!(st.fruit_rest_s, 0.0);

    // Eaten out.
    assert!(flora.take_stand_fruit(s, 1e9).is_some());
    flora.step(&mut w);
    let st = stand(&flora, s);
    assert_eq!(st.crop.phase, CropPhase::Idle, "eaten out, the crop is over");
    let rest = st.fruit_rest_s;
    assert!(rest > 0.0, "and the bush rests: {rest}");
    let mut t = 0.0;
    while stand(&flora, s).fruit_rest_s > 0.0 {
        let st = stand(&flora, s);
        assert!(st.fruit == 0.0 && st.nectar == 0.0 && st.crop.phase == CropPhase::Idle, "{t} s into a {rest} s rest: {st:?}");
        flora.step(&mut w);
        t += DT;
    }
    assert!(t >= rest - 2.0 * DT, "the rest ran its {rest} s ({t} s)");
    until(&mut flora, &mut w, 60.0, |f| made(f, s));
    closes(&flora, "after the eaten crop");

    // Left alone, the crop falls and the bush rests.
    let litter0 = flora.view().ground_at(s).map_or(0.0, |g| g.litter);
    until(&mut flora, &mut w, 120.0, |f| stand(f, s).crop.phase == CropPhase::Idle);
    let st = stand(&flora, s);
    assert_eq!(st.fruit, 0.0);
    assert!(st.fruit_rest_s > 0.0, "a fallen crop rests: {st:?}");
    assert!(flora.view().ground_at(s).map_or(0.0, |g| g.litter) > litter0, "it fell to the litter");
    closes(&flora, "after the fallen crop");
}

/// **A latticevine bunch eaten out does not refill.** A face in fruit eaten out holds no
/// beads again until it has flowered again.
#[test]
fn a_vine_bunch_eaten_out_does_not_refill_before_the_face_flowers_again() {
    let (mut w, mut flora, faces) = vine_wall(|_| {});
    let fruiting = |f: &Flora| faces.iter().copied().find(|&x| f.view().cover.face(x).is_some_and(|c| c.spur == SpurPhase::Fruit && c.fruit > 0.0));
    until(&mut flora, &mut w, 7200.0, |f| fruiting(f).is_some());
    let face = fruiting(&flora).expect("a bunch");
    assert!(flora.take_fruit(face, 1e9).is_some());
    let mut flowered = false;
    for _ in 0..ticks(3600.0) {
        flora.step(&mut w);
        let c = *flora.view().cover.face(face).expect("covered");
        flowered |= c.spur == SpurPhase::Flower;
        if !flowered {
            assert_eq!(c.fruit, 0.0, "no beads before the face flowers again: {c:?}");
        }
    }
    closes(&flora, "the vine");
}

// ------------------------------------------------------------------ 2. fruit costs more

/// **A unit of fruit costs the bush more than a unit of reserve.** A bush that earns nothing,
/// pays no upkeep, sheds and reflushes nothing and makes no nectar: its reserve moves only
/// for its fruit. Over a crop it falls by `fruit_cost` × the fruit made, the overhead is
/// booked as respired (and `fruit_overhead`), and the books close every second.
#[test]
fn a_unit_of_fruit_costs_the_bush_more_than_a_unit_of_reserve_and_the_books_close() {
    let shipped = FloraConfig::default();
    assert!(shipped.lanternberry.fruit_cost > 1.0, "the spec: fruit costs more than par");
    assert!(shipped.latticevine.fruit_cost > 1.0, "the spec: vine fruit costs more than par");
    let mut w = plain(12, 8, &[]);
    let mut c = config(5.0, 1e9, 0.0);
    c.lanternberry.pollination_floor = 1.0;
    let l = &mut c.lanternberry;
    l.assimilation = 0.0;
    l.maintenance = 0.0;
    l.senescence = 0.0;
    l.reflush_below = 0.0;
    l.nectar_per_s = 0.0;
    let cost = l.fruit_cost;
    let mut flora = Flora::new(c);
    let s = bush(&mut flora, &w, 5, 4);
    until(&mut flora, &mut w, 60.0, |f| stand(f, s).crop.phase == CropPhase::Fruit);
    let (q0, fr0) = (stand(&flora, s).reserve, stand(&flora, s).fruit);
    let (resp0, over0) = (flora.view().ledger.respired_out, flora.view().ledger.fruit_overhead);
    let mut n = 0;
    while !made(&flora, s) {
        flora.step(&mut w);
        n += 1;
        assert!(n < ticks(60.0), "the crop is made");
        if n % ticks(1.0) == 0 {
            closes(&flora, &format!("{n} ticks into the crop"));
        }
    }
    let st = stand(&flora, s);
    let fruit = st.fruit - fr0;
    assert!(fruit > 0.0);
    let paid = q0 - st.reserve;
    assert!(paid > fruit, "the bush paid {paid} for {fruit} of fruit");
    assert!((paid - cost * fruit).abs() <= 1e-12, "it paid fruit_cost {cost} a unit: {paid} for {fruit}");
    let l = &flora.view().ledger;
    assert!((l.fruit_overhead - over0 - (cost - 1.0) * fruit).abs() <= 1e-12, "the overhead booked");
    assert!((l.respired_out - resp0 - (cost - 1.0) * fruit).abs() <= 1e-12, "and respired");
    closes(&flora, "the crop made");
}

// ------------------------------------------------------------------ 3. pollination

/// **Unvisited, the floor; visited, more.** Two bushes flower together. One is never visited:
/// it sets `pollination_floor` of a full crop. The other is visited `pollination_visits` times
/// while in flower — each visit booked to it, counted on its crop and in the ledger — and
/// sets a full crop. A visit to a bush not in flower books nothing.
#[test]
fn an_unvisited_flowering_sets_only_the_floor_and_a_visited_one_sets_its_crop() {
    let shipped = FloraConfig::default().lanternberry;
    assert!(shipped.flower_s > 0.0, "the spec: lanternberries flower before they fruit");
    assert!(shipped.pollination_floor > 0.0 && shipped.pollination_floor < 1.0, "the spec: a small floor, not none and not all");
    assert!(shipped.nectar_per_s > 0.0 && shipped.nectar_cap > 0.0, "the spec: the flowers hold nectar");
    let mut w = plain(16, 8, &[]);
    let mut flora = Flora::new(config(30.0, 1e9, 0.0));
    let sc = flora.config().lanternberry.clone();
    let (a, b) = (bush(&mut flora, &w, 3, 4), bush(&mut flora, &w, 11, 4));
    until(&mut flora, &mut w, 10.0, |f| [a, b].iter().all(|&s| stand(f, s).crop.phase == CropPhase::Flower));
    until(&mut flora, &mut w, 10.0, |f| stand(f, b).nectar > 0.0);
    let k = sc.pollination_visits.ceil().max(1.0) as u32;
    for i in 0..k {
        assert!(flora.visit_stand(b), "visit {i} booked");
    }
    assert_eq!(stand(&flora, b).crop.visits, k, "booked to that bush");
    assert_eq!(stand(&flora, a).crop.visits, 0, "and not to the other");
    assert_eq!(flora.view().ledger.flower_visits[0], u64::from(k));
    let no_stand = Site { x: 7, y: 2, z: 4 };
    assert!(!flora.visit_stand(no_stand), "no stand, no visit");

    until(&mut flora, &mut w, 60.0, |f| [a, b].iter().all(|&s| made(f, s)));
    let (fa, fb) = (stand(&flora, a).fruit, stand(&flora, b).fruit);
    let floor = sc.pollination_floor * sc.fruit_cap;
    assert!((fa - floor).abs() <= 1e-12, "unvisited, the floor: {fa} against {floor}");
    let full = pollinated_share(sc.pollination_floor, sc.pollination_visits, f64::from(k)) * sc.fruit_cap;
    assert!((fb - full).abs() <= 1e-12 && fb > fa, "visited, its crop: {fb} against {full}");
    let l = &flora.view().ledger;
    assert_eq!(l.crops_set, [1, 1], "one crop set unvisited, one visited");
    assert!((l.fruit_set[0] - fa).abs() <= 1e-12 && (l.fruit_set[1] - fb).abs() <= 1e-12);
    assert!(!flora.visit_stand(b), "a bush in fruit is not in flower: no visit");
    assert_eq!(flora.view().ledger.flower_visits[0], u64::from(k));
    closes(&flora, "the two crops");
}

/// **The visit-to-fruit rule**: the floor with no visit, rising with the visits to a full
/// crop at `pollination_visits`, never past it.
#[test]
fn the_share_set_rises_from_the_floor_to_a_full_crop_with_the_visits() {
    for (floor, full) in [(0.1, 3.0), (0.0, 1.0), (0.4, 5.0)] {
        assert_eq!(pollinated_share(floor, full, 0.0), floor);
        let mut was = floor;
        for v in 1..=10 {
            let s = pollinated_share(floor, full, f64::from(v));
            assert!(s >= was && s <= 1.0, "{floor} {full} {v}: {s}");
            was = s;
        }
        assert_eq!(pollinated_share(floor, full, full.ceil()), 1.0);
    }
}

/// A rock wall at `x = 6`, `y = 1..=10`, across `z = 0..12`, its west faces `y = 3..=10`
/// covered by one vine with a purse that pays every spur; spreading and sisters unaffordable,
/// no drinking, and the spur clock short (a face cycles every few minutes).
fn vine_wall(tune: impl Fn(&mut FloraConfig)) -> (World, Flora, Vec<Face>) {
    let mut cells = Vec::new();
    for z in 0..12 {
        for y in 1..=10 {
            cells.push((6, y, z));
        }
    }
    let w = plain(12, 12, &cells);
    let mut c = FloraConfig::default();
    c.step_every = 1;
    let v = &mut c.latticevine;
    v.spread_cost = 1e12;
    v.sister_cost = 1e12;
    v.transpiration_m3_per_s = 0.0;
    v.spur_share = 0.5;
    tune(&mut c);
    let mut flora = Flora::new(c);
    let wf = |y: u32, z: u32| Face::new(6, y, z, FaceDir::NegX);
    let id = flora
        .seed_vine(&w, VineSeed { root: Site { x: 5, y: 2, z: 0 }, first: wf(3, 0), reserve: 1e4, lineage: None })
        .expect("the vine roots");
    let mut faces = vec![wf(3, 0)];
    for z in 0..12 {
        for y in 3..=10 {
            if (y, z) != (3, 0) {
                assert!(flora.cover_face(&w, id, wf(y, z), 1.0), "fixture: cover {:?}", wf(y, z));
                faces.push(wf(y, z));
            }
        }
    }
    (w, flora, faces)
}

/// **A vine flower unvisited sets the floor; visited, a full bunch.** Of two faces in flower,
/// one is visited `pollination_visits` times: at the set it holds a full bunch, the other
/// the floor's share of one; each set and the visits are booked.
#[test]
fn an_unvisited_vine_flower_sets_the_floor_and_a_visited_one_a_full_bunch() {
    let (mut w, mut flora, faces) = vine_wall(|_| {});
    let vc = flora.config().latticevine.clone();
    assert!(vc.pollination_floor > 0.0 && vc.pollination_floor < 1.0, "the spec: a small floor");
    let in_flower = |f: &Flora| -> Vec<Face> {
        faces.iter().copied().filter(|&x| f.view().cover.face(x).is_some_and(|c| c.spur == SpurPhase::Flower)).collect()
    };
    until(&mut flora, &mut w, 7200.0, |f| in_flower(f).len() >= 2);
    let open = in_flower(&flora);
    let (a, b) = (open[0], open[1]);
    let k = vc.pollination_visits.ceil().max(1.0) as u32;
    for _ in 0..k {
        assert!(flora.visit_face(b), "a visit to a flower is booked");
    }
    assert_eq!(u32::from(flora.view().cover.face(b).expect("b").visits), k);
    assert_eq!(flora.view().ledger.flower_visits[1], u64::from(k));
    let set = |f: &Flora, x: Face| f.view().cover.face(x).is_some_and(|c| c.spur != SpurPhase::Flower);
    until(&mut flora, &mut w, 600.0, |f| set(f, a) && set(f, b));
    let (ca, cb) = (*flora.view().cover.face(a).expect("a"), *flora.view().cover.face(b).expect("b"));
    assert_eq!((ca.spur, cb.spur), (SpurPhase::Fruit, SpurPhase::Fruit));
    let bunch = vc.fruit_per_bunch();
    assert!((ca.fruit - vc.pollination_floor * bunch).abs() <= 1e-12, "unvisited, the floor: {} of {bunch}", ca.fruit);
    assert!((cb.fruit - bunch).abs() <= 1e-12, "visited, a full bunch: {} of {bunch}", cb.fruit);
    assert!(!flora.visit_face(b), "a face in fruit is not in flower");
    let l = &flora.view().ledger;
    assert!(l.crops_set[0] >= 1 && l.crops_set[1] >= 1);
    assert!(l.fruit_overhead > 0.0, "the bunches cost their overhead");
    closes(&flora, "the vine's bunches");
}
