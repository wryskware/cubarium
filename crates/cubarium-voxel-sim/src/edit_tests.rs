//! WORLD-EDIT: the edit API on a small hand-built world. Mechanisms only: no tuned number
//! is asserted, and every body and plant size comes from the shipped config.

use cubarium_voxel::{Command as WorldCommand, Config, Material, World};
use cubarium_voxel_fauna::{BodySpec, BodyStage, Departure, Fauna, FaunaConfig, Founder, Group, Placed};
use cubarium_voxel_flora::{Flora, FloraConfig, PlantPlaced, PlantStage, Site, Species};

use crate::edit::{At, Edit, HeldTable, Scenario, command};

const W: u32 = 64;
const H: u32 = 24;
const D: u32 = 32;
const V: f64 = 0.25;

/// A soil plain (support at y = 2) with a one-voxel pond at `x 40..=47, z 10..=17` (support at
/// y = 1, water over it), a rock pillar the full height at `x 20..=21, z 4..=5`, and a low rock
/// roof at `y 6..=7` over `x 8..=15, z 8..=15`.
fn world() -> World {
    let mut w = World::empty(Config { width: W, height: H, depth: D, voxel_m: V, ..Config::default() });
    for z in 0..D {
        for x in 0..i64::from(W) {
            let pond = (40..=47).contains(&x) && (10..=17).contains(&z);
            let pillar = (20..=21).contains(&x) && (4..=5).contains(&z);
            let top = if pond { 1 } else if pillar { H - 1 } else { 2 };
            for y in 1..=top {
                let material = if pillar { Material::Rock } else { Material::Soil };
                w.apply(WorldCommand::SetMaterial { x, y, z, material });
            }
            if (8..=15).contains(&x) && (8..=15).contains(&z) {
                for y in 6..=7 {
                    w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Rock });
                }
            }
            if pond {
                w.apply(WorldCommand::AddWater { x, y: 2, z, volume_m3: V * V * V });
            }
        }
    }
    w
}

fn layers() -> (World, Flora, Fauna) {
    (world(), Flora::new(FloraConfig::for_voxel_size(V)), Fauna::new(FaunaConfig::default()))
}

/// Both layers' books close on all three currencies.
fn closes(flora: &Flora, fauna: &Fauna) {
    let a = fauna.view();
    let f = flora.view();
    for (have, want, what) in [
        (a.organic(), a.ledger.expected_organic(), "fauna organic"),
        (a.mineral(), a.ledger.expected_mineral(), "fauna mineral"),
        (a.energy(), a.ledger.expected_energy(), "fauna energy"),
        (f.organic(), f.ledger.expected_organic(), "flora organic"),
        (f.mineral(), f.ledger.expected_mineral(), "flora mineral"),
        (f.energy(), f.ledger.expected_energy(), "flora energy"),
    ] {
        assert!((have - want).abs() <= 1e-9 * want.abs().max(1.0), "{what}: held {have}, books say {want}");
    }
}

fn state(flora: &Flora, fauna: &Fauna) -> (Vec<u8>, Vec<u8>) {
    (fauna.save(), cubarium_voxel_flora::snapshot::encode(flora))
}

fn body_id(p: Placed) -> u64 {
    match p {
        Placed::Body { id, .. } => id,
        other => panic!("not a body: {other:?}"),
    }
}

fn adult() -> BodySpec {
    BodySpec::default()
}

#[test]
fn a_placed_animal_appears_and_a_removed_one_leaves_both_booked() {
    let (world, mut flora, mut fauna) = layers();
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    let id = body_id(e.place_animal(Founder::Browser, At::Column { x: 30, z: 20 }, adult()).expect("placed"));
    assert!(fauna.view().animal(id).is_some());
    assert_eq!(fauna.view().ledger.introduced, 1);
    assert!(fauna.view().ledger.introduced_organic_in > 0.0);
    closes(&flora, &fauna);
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    e.remove_animal(id).expect("removed");
    assert!(fauna.view().animal(id).is_none());
    assert_eq!(fauna.view().ledger.departed(Departure::Removed), 1);
    assert!(fauna.view().ledger.removed_organic_out > 0.0);
    closes(&flora, &fauna);
}

#[test]
fn a_placed_plant_appears_and_a_removed_one_leaves_both_booked() {
    let (world, mut flora, mut fauna) = layers();
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    let PlantPlaced::Stand { id } = e.place_plant(Species::Bloomcrown, At::Column { x: 30, z: 20 }, PlantStage::Mature).expect("placed")
    else {
        panic!("a stand")
    };
    let site = Site { x: 30, y: 2, z: 20 };
    assert_eq!(flora.view().stand_at(site).map(|s| s.id), Some(id));
    closes(&flora, &fauna);
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    let seeds = e.place_plant(Species::Bloomcrown, At::Column { x: 34, z: 20 }, PlantStage::Seed).expect("seeded");
    assert!(matches!(seeds, PlantPlaced::Seeds { .. }));
    e.remove_plant(id).expect("removed");
    assert!(flora.view().stand_at(site).is_none());
    assert!(flora.view().ledger.removed_organic_out > 0.0);
    closes(&flora, &fauna);
}

#[test]
fn a_moved_or_carried_animal_keeps_everything_but_its_place() {
    let (world, mut flora, mut fauna) = layers();
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    let id = body_id(e.place_animal(Founder::Browser, At::Column { x: 30, z: 20 }, adult()).unwrap());
    let before = *fauna.view().animal(id).unwrap();
    let same = |a: &cubarium_voxel_fauna::Animal| {
        let mut b = *a;
        b.site = before.site;
        b.pose = before.pose;
        b.mobility = before.mobility;
        b
    };
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    e.move_animal(id, At::Column { x: 36, z: 22 }).expect("moved");
    let moved = *fauna.view().animal(id).unwrap();
    assert_ne!(moved.site, before.site);
    assert_eq!(same(&moved), before);
    // A lift and a set-down: the same body, booked out and back in.
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    let held = e.lift_animal(id).expect("lifted");
    assert!(fauna.view().animal(id).is_none());
    closes(&flora, &fauna);
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    assert_eq!(e.set_down_animal(held, At::Column { x: 50, z: 26 }).expect("set down"), id);
    let back = *fauna.view().animal(id).unwrap();
    assert_eq!(back.site, Site { x: 50, y: 2, z: 26 });
    assert_eq!(same(&back), before);
    closes(&flora, &fauna);
}

#[test]
fn a_transplanted_plant_keeps_its_state() {
    let (world, mut flora, mut fauna) = layers();
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    let PlantPlaced::Stand { id } = e.place_plant(Species::Bloomcrown, At::Column { x: 30, z: 20 }, PlantStage::Mature).unwrap() else {
        panic!()
    };
    let before = *flora.view().stands.iter().find(|s| s.id == id).unwrap();
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    e.move_plant(id, At::Column { x: 36, z: 22 }).expect("moved");
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    let held = e.lift_plant(id).expect("lifted");
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    assert_eq!(e.set_down_plant(held, At::Column { x: 50, z: 26 }).expect("set down"), id);
    let after = *flora.view().stands.iter().find(|s| s.id == id).unwrap();
    assert_eq!(after.site, Site { x: 50, y: 2, z: 26 });
    let mut same = after;
    same.site = before.site;
    assert_eq!(same, before);
    closes(&flora, &fauna);
}

#[test]
fn an_impossible_placement_is_refused_with_a_reason_and_changes_nothing() {
    let (world, mut flora, mut fauna) = layers();
    {
        let mut e = Edit::new(&world, &mut flora, &mut fauna);
        e.place_plant(Species::Bloomcrown, At::Column { x: 30, z: 20 }, PlantStage::Mature).unwrap();
        e.place_animal(Founder::Chorister, At::Column { x: 50, z: 24 }, BodySpec { group: Group::New, ..adult() }).unwrap();
    }
    let s0 = state(&flora, &fauna);
    let cases: Vec<(&str, Box<dyn Fn(&mut Edit<'_>) -> Result<(), String>>)> = vec![
        ("walker against rock", Box::new(|e| e.place_animal(Founder::Browser, At::Face { x: 22, y: 2, z: 4 }, adult()).map(|_| ()))),
        ("snail on dry land", Box::new(|e| e.place_animal(Founder::RippleSnail, At::Column { x: 30, z: 26 }, adult()).map(|_| ()))),
        ("loftstrider under a roof", Box::new(|e| e.place_animal(Founder::Loftstrider, At::Face { x: 12, y: 2, z: 12 }, adult()).map(|_| ()))),
        ("bellwing inside rock", Box::new(|e| {
            e.place_animal(Founder::Bellwing, At::Point { x_m: 20.5 * V, y_m: 2.0, z_m: 4.5 * V, aloft: true }, adult()).map(|_| ())
        })),
        ("plant on an occupied face", Box::new(|e| e.place_plant(Species::Bloomcrown, At::Column { x: 30, z: 20 }, PlantStage::Mature).map(|_| ()))),
        ("plant on bare rock", Box::new(|e| e.place_plant(Species::Bloomcrown, At::Face { x: 12, y: 7, z: 12 }, PlantStage::Seedling).map(|_| ()))),
        ("join a pack that is not there", Box::new(|e| {
            e.place_animal(Founder::Chorister, At::Column { x: 52, z: 24 }, BodySpec { group: Group::Join(999), ..adult() }).map(|_| ())
        })),
        ("a pup founding a pack", Box::new(|e| {
            e.place_animal(Founder::Chorister, At::Column { x: 52, z: 24 }, BodySpec { stage: BodyStage::Juvenile, group: Group::New, ..adult() })
                .map(|_| ())
        })),
        ("a pack group on a lineage without one", Box::new(|e| {
            e.place_animal(Founder::Browser, At::Column { x: 30, z: 26 }, BodySpec { group: Group::New, ..adult() }).map(|_| ())
        })),
        ("a rider with no host", Box::new(|e| {
            e.place_animal(Founder::Lanternjaw, At::Column { x: 4, z: 28 }, BodySpec { stage: BodyStage::Rider, ..adult() }).map(|_| ())
        })),
        ("remove what is not there", Box::new(|e| e.remove_animal(12_345))),
    ];
    for (what, case) in cases {
        let mut e = Edit::new(&world, &mut flora, &mut fauna);
        let why = case(&mut e).expect_err(what);
        assert!(!why.trim().is_empty(), "{what}: a reason");
        assert!(state(&flora, &fauna) == s0, "{what}: the world changed");
    }
}

#[test]
fn auto_joins_the_nearest_pack_and_a_juvenile_joins_as_a_pup() {
    let (world, mut flora, mut fauna) = layers();
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    let Placed::Body { pack: Some(p), id: first } =
        e.place_animal(Founder::Chorister, At::Column { x: 30, z: 20 }, BodySpec { group: Group::New, ..adult() }).unwrap()
    else {
        panic!("a pack")
    };
    let Placed::Body { pack, .. } = e.place_animal(Founder::Chorister, At::Column { x: 32, z: 20 }, adult()).unwrap() else { panic!() };
    assert_eq!(pack, Some(p), "Auto joins the pack whose den is near");
    let pup = body_id(
        e.place_animal(Founder::Chorister, At::Column { x: 31, z: 20 }, BodySpec { stage: BodyStage::Juvenile, ..adult() }).unwrap(),
    );
    let m = fauna.view().animal(pup).unwrap().pack.unwrap();
    assert!(m.pup && m.pack == p);
    assert_eq!(fauna.pack(p).unwrap().breeder, Some(first));
    // Lifting the breeder hands the pack on; setting it down brings it back as a member.
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    let held = e.lift_animal(first).unwrap();
    assert_ne!(fauna.pack(p).unwrap().breeder, Some(first));
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    e.set_down_animal(held, At::Column { x: 34, z: 20 }).unwrap();
    assert_eq!(fauna.view().animal(first).unwrap().pack.map(|m| m.pack), Some(p));
}

#[test]
fn a_lift_that_empties_a_pack_keeps_its_record_with_the_body() {
    let (world, mut flora, mut fauna) = layers();
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    let Placed::Body { pack: Some(p), id } =
        e.place_animal(Founder::Chorister, At::Column { x: 30, z: 20 }, BodySpec { group: Group::New, ..adult() }).unwrap()
    else {
        panic!()
    };
    let held = e.lift_animal(id).unwrap();
    assert!(fauna.pack(p).is_none());
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    e.set_down_animal(held, At::Column { x: 40, z: 26 }).unwrap();
    assert_eq!(fauna.pack(p).unwrap().breeder, Some(id));
}

#[test]
fn lifting_a_host_carries_its_riders_and_lifting_a_rider_detaches_it() {
    let (world, mut flora, mut fauna) = layers();
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    let host = body_id(e.place_animal(Founder::Browser, At::Column { x: 30, z: 20 }, adult()).unwrap());
    let rider = body_id(
        e.place_animal(Founder::Lanternjaw, At::Column { x: 30, z: 20 }, BodySpec { stage: BodyStage::Rider, ..adult() }).unwrap(),
    );
    assert_eq!(fauna.view().animal(rider).unwrap().attached.map(|a| a.host), Some(host));
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    let held = e.lift_animal(host).unwrap();
    assert_eq!(held.riders().iter().map(|r| r.id).collect::<Vec<_>>(), vec![rider]);
    assert!(fauna.view().animal(rider).is_none(), "the rider went with its host");
    closes(&flora, &fauna);
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    e.set_down_animal(held, At::Column { x: 50, z: 26 }).unwrap();
    let (h, r) = (*fauna.view().animal(host).unwrap(), *fauna.view().animal(rider).unwrap());
    assert_eq!(r.attached.map(|a| a.host), Some(host));
    assert_eq!(r.site, h.site);
    let freed = fauna.view().ledger.riders_freed;
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    let held = e.lift_animal(rider).unwrap();
    assert!(held.animal().attached.is_none());
    assert_eq!(fauna.view().ledger.riders_freed, freed + 1);
    closes(&flora, &fauna);
}

#[test]
fn thin_leaves_exactly_the_count() {
    let (world, mut flora, mut fauna) = layers();
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    let p = e.populate(Founder::Browser, 12, 3, None, adult());
    assert_eq!(p.placed.len(), 12, "{:?}", p.shortfall);
    let t = e.thin(Founder::Browser, 5, 9);
    assert_eq!((t.before, t.after, t.removed.len()), (12, 5, 7));
    let n = fauna.view().animals.iter().filter(|a| a.founder == Some(Founder::Browser)).count();
    assert_eq!(n, 5);
    closes(&flora, &fauna);
}

#[test]
fn thinning_packs_never_splits_or_orphans_one() {
    let (world, mut flora, mut fauna) = layers();
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    let size = e.fauna.config().founder(Founder::Chorister).pack.unwrap().pack_size;
    let p = e.populate(Founder::Chorister, 3 * size, 1, None, adult());
    assert_eq!(p.placed.len(), 3 * size, "{:?}", p.shortfall);
    let packs: Vec<u64> = fauna.packs().iter().map(|p| p.id).collect();
    assert_eq!(packs.len(), 3);
    for &pk in &packs {
        let mut e = Edit::new(&world, &mut flora, &mut fauna);
        e.place_animal(Founder::Chorister, At::Column { x: 4, z: 28 }, BodySpec { stage: BodyStage::Juvenile, group: Group::Join(pk), ..adult() })
            .unwrap();
    }
    let members = |f: &Fauna, pk: u64| f.view().animals.iter().filter(|a| a.pack.is_some_and(|m| m.pack == pk)).count();
    let full: Vec<usize> = packs.iter().map(|&pk| members(&fauna, pk)).collect();
    let total: usize = full.iter().sum();
    for keep in 0..=total {
        let (mut fl, mut fa) = (flora.clone(), fauna.clone());
        let mut e = Edit::new(&world, &mut fl, &mut fa);
        let t = e.thin(Founder::Chorister, keep, keep as u64);
        assert_eq!(t.after, keep);
        let mut trimmed = 0;
        for (k, &pk) in packs.iter().enumerate() {
            let left: Vec<&cubarium_voxel_fauna::Animal> =
                fa.view().animals.iter().filter(|a| a.pack.is_some_and(|m| m.pack == pk)).collect();
            if left.is_empty() {
                assert!(fa.pack(pk).is_none(), "an empty pack is dropped");
                continue;
            }
            assert!(left.iter().any(|a| !a.pack.unwrap().pup), "keep {keep}: pack {pk} kept pups and no adult");
            assert!(fa.pack(pk).unwrap().breeder.is_some());
            if left.len() < full[k] {
                trimmed += 1;
            }
        }
        assert!(trimmed <= 1, "keep {keep}: more than one pack partly thinned");
        closes(&fl, &fa);
    }
}

#[test]
fn a_scenario_file_applies_and_says_what_it_refused_also_on_a_loaded_layer() {
    let (world, mut flora, mut fauna) = layers();
    let text = r#"
        [[populate]]
        lineage = "frondgrazer"
        count = 6
        seed = 2
        region = { x = [24, 60], z = [18, 30] }

        [[thin]]
        lineage = "frondgrazer"
        keep = 4

        [[place]]
        species = "bloomcrown"
        at = { x = 30, z = 4 }

        [[place]]
        lineage = "ripple-snail"
        at = { x = 30, z = 26 }

        [[paint]]
        species = "springturf"
        region = { x = [50, 53], z = [2, 4] }
        amount = 0.5

        [[clear]]
        faces = [[30, 2, 4]]
    "#;
    let sc = Scenario::from_toml(text).expect("parses");
    let r = sc.apply(&mut Edit::new(&world, &mut flora, &mut fauna));
    assert_eq!(r.refused.len(), 1, "the dry snail only: {r:?}");
    assert!(r.refused[0].contains("place[1]"));
    // `[[thin]]` runs before `[[populate]]`, whatever the file's order: nothing to thin yet.
    assert_eq!(fauna.view().animals.len(), 6);
    assert!(flora.view().stand_at(Site { x: 30, y: 2, z: 4 }).is_some(), "every removal runs before any addition");
    closes(&flora, &fauna);
    // The same on a layer read back from its bytes (a `load=`).
    let mut loaded = Fauna::load(&fauna.save()).expect("loads");
    loaded.replace_config(fauna.config().clone()).unwrap();
    let again = Scenario::from_toml("[[thin]]\nlineage = \"browser\"\nkeep = 1\n").unwrap();
    let r = again.apply(&mut Edit::new(&world, &mut flora, &mut loaded));
    assert!(r.refused.is_empty(), "{r:?}");
    assert_eq!(loaded.view().animals.len(), 1);
    closes(&flora, &loaded);
    assert!(Scenario::from_toml("[[place]]\nnope = 1\n").is_err(), "unknown fields are refused");
}

#[test]
fn json_edits_hold_and_set_down_by_token_and_a_save_keeps_the_held_at_its_lift_spot() {
    let (world, mut flora, mut fauna) = layers();
    let mut held = HeldTable::default();
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    let r = command(&mut e, &mut held, &serde_json::json!({"op": "place_animal", "lineage": "frondgrazer", "at": {"x": 30, "z": 20}}));
    assert_eq!(r["ok"], true, "{r}");
    let id = r["placed"]["body"]["id"].as_u64().unwrap();
    let r = command(&mut e, &mut held, &serde_json::json!({"op": "lift_animal", "id": id}));
    let token = r["held"].clone();
    assert_eq!(held.len(), 1);
    // A save now: a copy at the lift spot, the running world still without it.
    let (mut sf, mut sa) = (e.flora.clone(), e.fauna.clone());
    held.restore_copies(&mut sf, &mut sa);
    assert_eq!(sa.view().animal(id).map(|a| a.site), Some(Site { x: 30, y: 2, z: 20 }));
    closes(&sf, &sa);
    assert!(e.fauna.view().animal(id).is_none());
    let r = command(&mut e, &mut held, &serde_json::json!({"op": "set_down", "held": token, "at": {"x": 50, "z": 26}}));
    assert_eq!(r["ok"], true, "{r}");
    let r = command(&mut e, &mut held, &serde_json::json!({"op": "pick", "x_m": 50.5 * V, "y_m": 0.75, "z_m": 26.5 * V}));
    assert_eq!(r["picked"]["animals"][0]["id"].as_u64(), Some(id));
    let r = command(&mut e, &mut held, &serde_json::json!({"op": "remove_animal", "id": 999}));
    assert_eq!(r["ok"], false);
    assert!(r["refused"].as_str().is_some_and(|s| !s.is_empty()));
}

#[test]
fn every_lineage_places_where_it_can_live() {
    let (world, mut flora, mut fauna) = layers();
    let mut e = Edit::new(&world, &mut flora, &mut fauna);
    for (k, f) in Founder::ALL.into_iter().enumerate() {
        let at = if f == Founder::RippleSnail { At::Column { x: 43, z: 13 } } else { At::Column { x: 26 + 4 * k as i64, z: 24 } };
        let placed = e.place_animal(f, at, adult());
        assert!(placed.is_ok(), "{}: {placed:?}", f.name());
    }
    let eggs = e.place_animal(Founder::Lanternjaw, At::Column { x: 4, z: 4 }, BodySpec { stage: BodyStage::Egg, ..adult() });
    assert!(matches!(eggs, Ok(Placed::Eggs { eggs: 1, .. })), "{eggs:?}");
    assert_eq!(fauna.view().animals.len(), Founder::COUNT);
    closes(&flora, &fauna);
}
