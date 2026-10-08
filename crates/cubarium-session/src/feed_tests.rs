//! Small hand-built inputs; view reads never mutate their simulation layers.
use crate::{
    feed::{FeedConfig, Frame, Tracker},
    view_state,
};
use cubarium_voxel::{Command as WorldCommand, Config, Material, World};
use cubarium_voxel_fauna::{Command as AnimalCommand, Fauna, FaunaConfig, Founder, StartingStores, WallClimb};
use cubarium_voxel_flora::{Carcass, Deposit, DepositKind, Face, FaceDir, Flora, FloraConfig, Site, Species, VineSeed};
use serde_json::json;

pub(crate) fn world() -> World {
    let mut w =
        World::empty(Config { width: 32, height: 12, depth: 4, voxel_m: 0.25, rain_m_per_s: 0.0, ..Config::default() });
    for x in 0..32 {
        for z in 0..4 {
            w.apply(WorldCommand::SetMaterial { x, y: 0, z, material: Material::Bedrock });
            w.apply(WorldCommand::SetMaterial { x, y: 1, z, material: Material::Soil });
        }
    }
    w
}

pub(crate) fn flora() -> Flora {
    Flora::new(FloraConfig::for_voxel_size(0.25))
}
fn fauna() -> Fauna {
    Fauna::new(FaunaConfig::default())
}
fn site(x: u32) -> Site {
    Site { x, y: 1, z: 1 }
}

pub(crate) fn deposit(f: &mut Flora, x: u32, kind: DepositKind, organic: f64) {
    assert!(f.deposit(site(x), Deposit { kind, organic, mineral: 0.0, energy: 0.0 }));
}

#[test]
fn starving_clears_when_fed_and_a_rider_names_its_host() {
    let w = world();
    let f = flora();
    let mut animals = fauna();
    for x in [3, 5] {
        assert!(animals.apply(
            &w,
            AnimalCommand::IntroduceFounder {
                x,
                z: 1,
                founder: Founder::Browser,
                stores: StartingStores::FULL,
                heading_rad: 0.0
            }
        ));
    }
    let host = animals.view().animals[0].id;
    let mut state = serde_json::to_value(&animals).unwrap();
    state["animals"][1]["body"] = json!(0.0);
    state["animals"][1]["reserve"] = json!(0.0);
    state["animals"][1]["attached"] = json!({"host": host});
    animals = serde_json::from_value(state.clone()).unwrap();
    let a = Frame::of(&w, &f, &animals).animals;
    assert_ne!(a.flags[1] & crate::feed::FLAG_STARVING, 0);
    assert_eq!(a.flags[0] & crate::feed::FLAG_STARVING, 0);
    assert_eq!(a.riding_host, [-1, host as i64]);
    state["animals"][1]["reserve"] = json!(1.0);
    animals = serde_json::from_value(state).unwrap();
    assert_eq!(Frame::of(&w, &f, &animals).animals.flags[1] & crate::feed::FLAG_STARVING, 0);
}

#[test]
fn a_descending_climber_faces_behind_its_travel_heading() {
    let w = world();
    let mut animals = fauna();
    assert!(animals.apply(
        &w,
        AnimalCommand::IntroduceFounder {
            x: 4,
            z: 1,
            founder: Founder::Seedporter,
            stores: StartingStores::FULL,
            heading_rad: std::f64::consts::FRAC_PI_2
        }
    ));
    let climb = WallClimb { ascending: false, from: site(4), to: site(5), dir: (1, 0), progress_m: 0.0, rise_m: 1.0 };
    let mut state = serde_json::to_value(&animals).unwrap();
    state["animals"][0]["mobility"]["climb"] = serde_json::to_value(climb).unwrap();
    animals = serde_json::from_value(state).unwrap();
    let a = Frame::of(&w, &flora(), &animals).animals;
    assert_eq!(a.wall_toward, [-1, 0]);
    assert!(a.heading[0].sin() > 0.0);
}

#[test]
fn plant_wilt_layers_and_parcel_are_the_legacy_model_inputs() {
    let w = world();
    let mut f = flora();
    for (x, species) in [(3, Species::Springturf), (6, Species::Lanternberry), (9, Species::Bloomcrown)] {
        let wood = f.config().species(species).wood_max;
        assert!(f.apply(&w, cubarium_voxel_flora::Command::Seed { x, z: 1, species, wood }));
    }
    let mut data = toml::Value::try_from(&f).unwrap();
    for s in data["stands"].as_array_mut().unwrap() {
        s["moisture"] = toml::Value::Float(0.0);
        s["parcel"] = toml::Value::Float(0.25);
        s["nectar"] = toml::Value::Float(0.125);
        s["fruit"] = toml::Value::Float(0.0625);
        for (i, stock) in s["layer_stock"].as_array_mut().unwrap().iter_mut().enumerate() {
            *stock = toml::Value::Float((i + 1) as f64 / 10.0);
        }
    }
    f = data.try_into().unwrap();
    let before = toml::Value::try_from(&f).unwrap();
    let p = Frame::of(&w, &f, &fauna()).plants;
    for (i, s) in f.view().stands.iter().enumerate() {
        let sc = f.config().species(s.species);
        assert_eq!(p.wilt[i], view_state::wilt_level(s.moisture));
        assert!(p.wilt[i] > view_state::wilt_level(1.0));
        assert_eq!(p.moisture[i], s.moisture as f32);
        let package = sc.propagule_package();
        assert_eq!(p.parcel_fullness[i], if package > 0.0 { s.parcel / package } else { 0.0 });
        let ripe = if sc.nectar_cap > 0.0 && sc.nectar_per_s > 0.0 {
            1.0
        } else if sc.fruit_cap > 0.0 && sc.fruit_per_s > 0.0 {
            (s.fruit / sc.fruit_cap).clamp(0.0, 1.0)
        } else {
            p.parcel_fullness[i]
        };
        assert_eq!(p.ripeness[i], ripe);
        let begin = p.foliage_offsets[i] as usize;
        let end = p.foliage_offsets[i + 1] as usize;
        let capacities = sc.layer_capacities(s.wood);
        let fractions = view_state::foliage_fractions(s, sc);
        for (j, at) in (begin..end).enumerate() {
            let cap = capacities.get(j).copied().unwrap_or(sc.alpha * s.wood.max(0.0));
            let stock = if j < capacities.len() { s.layer_stock[j] } else { s.foliage };
            assert_eq!(p.foliage_stock[at], stock);
            assert_eq!(p.foliage_capacity[at], cap);
            assert_eq!(if cap > 0.0 { p.foliage_stock[at] / cap } else { 0.0 }, fractions[j], "what keeps reads");
        }
    }
    assert_eq!(toml::Value::try_from(&f).unwrap(), before, "feed read wrote state");
}

#[test]
fn a_carcass_appears_and_decomposition_advances_its_decay() {
    let mut w = world();
    let mut config = FloraConfig::for_voxel_size(0.25);
    config.carrion_decomposition = 1.0;
    config.step_every = 1;
    let mut f = Flora::new(config);
    let a = fauna();
    assert!(Frame::of(&w, &f, &a).carcasses.ids.is_empty());
    deposit(&mut f, 3, DepositKind::Carrion, 1.0);
    assert!(
        f.lay_carcass(site(3), Carcass { lineage: Founder::Browser.index() as u8, heading_rad: 0.75, organic: 1.0 })
    );
    let first = Frame::of(&w, &f, &a).carcasses;
    assert_eq!(first.ids.len(), 1);
    assert_eq!(first.species, [Founder::Browser.index() as i32]);
    assert_eq!(first.heading, [0.75]);
    for _ in 0..4 {
        w.step_with(1);
        f.step(&mut w);
    }
    let next = Frame::of(&w, &f, &a).carcasses;
    assert_eq!(next.ids, first.ids);
    assert!(next.remaining[0] < first.remaining[0]);
    let carrion = f.view().ground_at(site(3)).unwrap().carrion;
    let threshold = f.config().carcass_remains_fraction;
    f.take_carrion(site(3), carrion - threshold * 0.5).unwrap();
    assert_eq!(Frame::of(&w, &f, &a).carcasses.state, [1]);
}

#[test]
fn a_ground_change_only_resends_its_chunk_and_two_updates_union() {
    let w = world();
    let mut f = flora();
    let mut t = Tracker::new(&w, FeedConfig::default());
    t.update_surfaces(&w, &f);
    t.take_dirty();
    deposit(&mut f, 3, DepositKind::DeadWood, 1.0);
    t.update_surfaces(&w, &f);
    let one = t.take_dirty();
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].at, [0, 0, 0]);
    assert_eq!(one[0].marks[0].kind, 0);
    deposit(&mut f, 3, DepositKind::Litter, 1.0);
    t.update_surfaces(&w, &f);
    deposit(&mut f, 20, DepositKind::DeadWood, 1.0);
    t.update_surfaces(&w, &f);
    let both = t.take_dirty();
    assert_eq!(both.len(), 2);
    assert!(both.iter().any(|c| c.at == [0, 0, 0] && c.marks.len() == 2));
    assert!(both.iter().any(|c| c.at == [1, 0, 0] && c.marks.len() == 1));
    t.update_surfaces(&w, &f);
    assert!(t.take_dirty().is_empty());
}

pub(crate) fn vine(w: &mut World, f: &mut Flora) -> Face {
    for y in 2..9 { w.apply(WorldCommand::SetMaterial { x: 6, y, z: 2, material: Material::Rock }); }
    let first = Face::new(6, 2, 2, FaceDir::NegZ);
    let owner = f.seed_vine(w, VineSeed { root: site(6), first, reserve: 1.0, lineage: None }).unwrap();
    for y in 3..=5 { assert!(f.cover_face(w, owner, Face::new(6, y, 2, FaceDir::NegZ), 1.0)); }
    Face::new(6, 4, 2, FaceDir::NegZ)
}

#[test]
fn vine_faces_replace_the_chunk_and_removal_updates_neighbour_tiles() {
    let mut w = world();
    let mut f = flora();
    let face = vine(&mut w, &mut f);
    let mut t = Tracker::new(&w, FeedConfig::default());
    t.update_surfaces(&w, &f);
    let first = t.take_dirty();
    let covered = &first.iter().find(|c| c.at == [0, 0, 0]).unwrap().vines;
    assert_eq!(covered.len(), f.view().cover.draw().len());
    for record in covered {
        let d = f
            .view()
            .cover
            .draw()
            .into_iter()
            .find(|d| [d.face.x as i32, d.face.y as i32, d.face.z as i32] == record.site)
            .unwrap();
        let old_cells = view_state::vine::cells(&[d], w.config(), |id| f.view().cover.vine(id).map(|v| v.root));
        assert_eq!(record.density, old_cells[0].density as u8);
    }
    w.apply(WorldCommand::SetMaterial { x: face.x as i64, y: face.y, z: face.z, material: Material::Air });
    f.step(&mut w);
    t.update_surfaces(&w, &f);
    let next = t.take_dirty();
    assert_eq!(next.len(), 1);
    assert!(next[0].vines.iter().all(|v| v.site != [face.x as i32, face.y as i32, face.z as i32]));
    assert_ne!(next[0].vines, *covered);
}
