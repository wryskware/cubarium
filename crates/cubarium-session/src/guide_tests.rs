//! The field guide's table ([`crate::guide`]): mechanisms only, against configs built here.
//! No number is pinned: every expectation is read back from the config under test.

use std::collections::BTreeSet;

use cubarium_voxel_fauna::{Diet, FaunaConfig, Food, Founder};
use cubarium_voxel_flora::{FloraConfig, Species};
use serde_json::Value;

use crate::guide::{LATTICEVINE, guide_table, sources};

fn names(list: &Value) -> Vec<String> {
    list.as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap().to_string()).collect()
}

fn entry<'a>(table: &'a Value, group: &str, name: &str) -> &'a Value {
    table[group].as_array().unwrap().iter().find(|e| e["name"] == name).unwrap_or_else(|| panic!("no {group} entry {name}"))
}

fn strings(v: &Value) -> BTreeSet<String> {
    v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect()
}

#[test]
fn every_lineage_and_plant_is_listed_once_by_the_species_query_names() {
    let table = guide_table(&FloraConfig::default(), &FaunaConfig::default());
    let species = crate::species_table();
    let want_animals = names(&species["animals"]);
    assert_eq!(want_animals.len(), Founder::ALL.len());
    assert_eq!(names(&table["animals"]), want_animals, "every lineage once, in Founder::ALL order");
    let mut want_plants = names(&species["plants"]);
    assert_eq!(want_plants.len(), Species::ALL.len());
    want_plants.push(LATTICEVINE.to_string());
    let got = names(&table["plants"]);
    assert_eq!(got, want_plants, "every flora species once, then the latticevine");
    assert_eq!(got.iter().collect::<BTreeSet<_>>().len(), got.len(), "no name twice");
}

#[test]
fn an_animals_food_classes_are_its_configured_diet() {
    let flora = FloraConfig::default();
    let base = FaunaConfig::default();
    for f in Founder::ALL {
        let table = guide_table(&flora, &base);
        let foods: Vec<String> = entry(&table, "animals", f.name())["diet"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["food"].as_str().unwrap().to_string())
            .collect();
        let want: Vec<String> = base.founder(f).eating_diet().foods().map(|x| x.name().to_string()).collect();
        assert_eq!(foods, want, "{}", f.name());
    }
    // Change one lineage's diet and the answer follows, here and in the plants it eats.
    let mut changed = base;
    let f = Founder::Browser;
    let new_diet = if base.founder(f).diet.eats(Food::Fruit) { Diet::of(&[Food::Film]) } else { Diet::of(&[Food::Fruit]) };
    changed.founders[f.index()].diet = new_diet;
    let table = guide_table(&flora, &changed);
    let foods: Vec<String> =
        entry(&table, "animals", f.name())["diet"].as_array().unwrap().iter().map(|d| d["food"].as_str().unwrap().to_string()).collect();
    let want: Vec<String> = changed.founder(f).eating_diet().foods().map(|x| x.name().to_string()).collect();
    assert_eq!(foods, want);
    for p in table["plants"].as_array().unwrap() {
        let name = p["name"].as_str().unwrap();
        let eats_it = changed.founder(f).eating_diet().foods().any(|food| sources(&flora, food).contains(&name));
        assert_eq!(strings(&p["eaten_by"]).contains(f.name()), eats_it, "{name}");
    }
}

#[test]
fn food_sources_follow_the_flora_config() {
    let base = FloraConfig::default();
    // A species made to fruit (or not) moves in and out of the fruit sources.
    let mut changed = base.clone();
    let s = Species::Bloomcrown;
    let fruits = base.species(s).fruit_per_s > 0.0 && base.species(s).fruit_cap > 0.0;
    let c = &mut changed.bloomcrown;
    if fruits {
        c.fruit_per_s = 0.0;
    } else {
        c.fruit_per_s = 1.0;
        c.fruit_cap = 1.0;
    }
    assert_eq!(sources(&base, Food::Fruit).contains(&s.name()), fruits);
    assert_eq!(sources(&changed, Food::Fruit).contains(&s.name()), !fruits);
    // The vine's nectar comes and goes with its rate.
    let mut dry = base.clone();
    dry.latticevine.nectar_per_face_s = 0.0;
    assert!(!sources(&dry, Food::Nectar).contains(&LATTICEVINE));
    let mut wet = base.clone();
    wet.latticevine.nectar_per_face_s = 1.0;
    assert!(sources(&wet, Food::Nectar).contains(&LATTICEVINE));
}

#[test]
fn prey_are_lineages_and_eaten_by_is_the_inverse_of_eating() {
    let flora = FloraConfig::default();
    let fauna = FaunaConfig::default();
    let table = guide_table(&flora, &fauna);
    let lineages: BTreeSet<&str> = Founder::ALL.iter().map(|f| f.name()).collect();
    let mut eats: BTreeSet<(String, String)> = BTreeSet::new();
    for a in table["animals"].as_array().unwrap() {
        let who = a["name"].as_str().unwrap().to_string();
        for p in a["prey"].as_array().unwrap() {
            assert!(lineages.contains(p["name"].as_str().unwrap()), "{who} preys on a non-lineage {p}");
        }
        for d in a["diet"].as_array().unwrap() {
            for s in strings(&d["sources"]) {
                eats.insert((who.clone(), s));
            }
        }
    }
    let mut eaten: BTreeSet<(String, String)> = BTreeSet::new();
    for p in table["plants"].as_array().unwrap() {
        let plant = p["name"].as_str().unwrap().to_string();
        for by in strings(&p["eaten_by"]) {
            eaten.insert((by, plant.clone()));
        }
    }
    assert_eq!(eaten, eats, "eaten_by is exactly the inverse of the animals' sources");
    // The prey list follows the hunter table: strike one lineage off and it leaves.
    let hunter = Founder::ALL.into_iter().find(|&f| fauna.founder(f).active_hunter().is_some()).expect("a hunter");
    let first = entry(&table, "animals", hunter.name())["prey"][0]["name"].as_str().map(str::to_string).expect("some prey");
    let gone = Founder::ALL.into_iter().find(|f| f.name() == first).unwrap();
    let mut changed = fauna;
    let mut h = changed.founders[hunter.index()].hunter.unwrap();
    h.prey[gone.index()] = false;
    changed.founders[hunter.index()].hunter = Some(h);
    let after = guide_table(&flora, &changed);
    assert!(!names(&entry(&after, "animals", hunter.name())["prey"]).contains(&first));
}

#[test]
fn the_session_answers_from_the_running_worlds_config() {
    let scratch = std::env::temp_dir().join(format!("cubarium-session-guide-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    let path = scratch.join("tiny.toml");
    std::fs::write(
        &path,
        "threads = 2\n[world]\nwidth = 32\nheight = 24\ndepth = 4\nvoxel_m = 0.25\n\
         [world.landform]\npreset = \"default\"\n\
         [world.landform.water]\nmin_lake_m2 = 0.0\nmin_tier_pools = 0\n\
         [world.climate]\nopening_min_s = 1.0\n",
    )
    .unwrap();
    let spec = crate::WorldSpec { config: Some(path), seed: Some(5), ..crate::WorldSpec::default() };
    let prepared = crate::prepare_new(&spec.sim_params().unwrap(), &spec).expect("a tiny world founds");
    let want = guide_table(prepared.flora.config(), prepared.fauna.config());
    let session = crate::Session::new(crate::SessionOptions {
        exec: crate::SimParams { threads: 2, ..crate::SimParams::default() },
        ..crate::SessionOptions::default()
    });
    assert!(session.query("guide", &Value::Null).is_err(), "no world, no guide");
    session.install(prepared);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let got = loop {
        if let Ok(v) = session.query("guide", &Value::Null) {
            break v;
        }
        assert!(std::time::Instant::now() < deadline, "the guide never arrived");
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert_eq!(got, want);
    session.close();
    let _ = std::fs::remove_dir_all(&scratch);
}
