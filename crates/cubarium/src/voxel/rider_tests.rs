//! A rider's figure (`specs`' rider branch): with the baked library a lanternjaw rider
//! draws its lineage's rider model ([`super::super::model::rider_model`]) on its host's
//! back, facing the host's heading; without it, the glyph. Nothing here pins a cell count
//! or a colour: the cells are read from the library.

use std::path::Path;

use cubarium_voxel::{Command as WorldCommand, Config as WorldConfig, Material, World};
use cubarium_voxel_fauna::{Attachment, Command, Fauna, FaunaConfig, StartingStores};

use super::super::model;
use super::*;

fn library() -> ModelLibrary {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/voxel-models");
    ModelLibrary::load(&root, 0.125).expect("the baked library")
}

/// A frondgrazer host facing `heading` and a lanternjaw riding it, on a soil plain.
fn host_and_rider(heading: f64) -> (World, Fauna, [Animal; 2]) {
    let c = WorldConfig { width: 24, height: 8, depth: 12, voxel_m: 0.125, ..WorldConfig::default() };
    let mut w = World::empty(c.clone());
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            w.apply(WorldCommand::SetMaterial { x, y: 0, z, material: Material::Soil });
        }
    }
    let mut fauna = Fauna::new(FaunaConfig::placeholder());
    for (x, founder) in [(8, Founder::Browser), (16, Founder::Lanternjaw)] {
        assert!(fauna.apply(
            &w,
            Command::IntroduceFounder { x, z: 6, founder, stores: StartingStores::FULL, heading_rad: heading }
        ));
    }
    let fv = fauna.view();
    let mut host = *fv.animals.iter().find(|a| a.founder == Some(Founder::Browser)).unwrap();
    let mut rider = *fv.animals.iter().find(|a| a.founder == Some(Founder::Lanternjaw)).unwrap();
    host.body = fv.config.founder(Founder::Browser).core.body_max;
    host.state = State::Resting;
    rider.body = fv.config.founder(Founder::Lanternjaw).core.body_min;
    rider.attached = Some(Attachment { host: host.id });
    rider.site = host.site;
    rider.pose = host.pose;
    rider.pose.heading_rad = heading + 1.0; // its own heading is not the one drawn
    let mut pair = [host, rider];
    pair.sort_by_key(|a| a.id);
    (w, fauna, pair)
}

/// The rider's figure, its host, and the rows it is lifted by.
fn rider_spec(lib: Option<&ModelLibrary>, models: &mut FigureModels, heading: f64) -> (Spec, Animal, u32) {
    let (w, fauna, pair) = host_and_rider(heading);
    let rider = *pair.iter().find(|a| a.attached.is_some()).unwrap();
    let host = *pair.iter().find(|a| a.attached.is_none()).unwrap();
    let view = w.view();
    let fv = FaunaView { animals: &pair, ..fauna.view() };
    let c = view.config;
    let grid = Animals::empty(c.width, c.height, c.depth);
    let mut out = Vec::new();
    specs(&grid, &view, fv, lib, models, &mut out);
    let s = out.into_iter().find(|s| s.key == (rider.id, 0)).expect("the rider's figure");
    let lift = animal::rider_lift(&fv, host.id, c.voxel_m).expect("the host's lift");
    (s, host, lift)
}

#[test]
fn a_rider_with_the_library_draws_its_larva_on_the_hosts_back() {
    let lib = library();
    let larva = lib
        .specimen(model::rider_model(Founder::Lanternjaw).unwrap())
        .and_then(|m| m.select(f64::INFINITY))
        .expect("the baked larva");
    let mut models = FigureModels::default();
    let heading = 0.7;
    let (s, host, lift) = rider_spec(Some(&lib), &mut models, heading);
    let info = models.info(s.model).clone();
    assert_eq!(info.count as usize, larva.len(), "the larva's cells");
    let voxels = models.voxels();
    let drawn: Vec<[i8; 3]> = voxels[info.first as usize..][..info.count as usize].iter().map(|v| v.offset()).collect();
    for m in larva {
        assert!(drawn.contains(&m.offset.map(|o| o as i8)), "cell {:?} drawn", m.offset);
    }
    assert!(info.paints.iter().all(|p| matches!(p, Paint::Coat { .. })), "painted in a coat");
    assert_eq!(s.orient, Orient::Ground(heading), "facing the host's heading");
    let feet = feet(&host, 0.125);
    assert_eq!(s.pos, [feet[0], feet[1] + f64::from(lift), feet[2]], "on the host's back, its centre line");
}

#[test]
fn a_rider_without_the_library_draws_the_glyph() {
    let mut models = FigureModels::default();
    let (s, _, _) = rider_spec(None, &mut models, 0.7);
    assert!(models.info(s.model).paints.iter().all(|p| *p == Paint::Style), "the glyph's style");
}
