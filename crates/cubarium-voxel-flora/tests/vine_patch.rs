//! **VINE-C** (Wrysk 2026-10-09, option C): a bite or a sip at a vine draws on every face in
//! the mouth's reach, each in proportion to what it holds. The flora side:
//! [`Flora::take_fruit_patch`] and [`Flora::take_nectar_patch`].
//!
//! - The draw is proportional: each face gives the same share of what it held.
//! - A patch gives what one face holding their total gives (the bite's stock is the sum).
//! - Asking for more than the patch holds empties every face and no more.
//! - Faces in another phase, or listed twice, give nothing extra.
//! - The books close through every take.
//!
//! Amounts are the fixture's own (set through the `set_spur` fixture hook); no shipped
//! number is asserted.

use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Face, FaceDir, Flora, FloraConfig, Site, SpurPhase, VineSeed};

/// A soil plain with a rock wall column at `x = 6` across `z`, rows 3..=10.
fn wall() -> World {
    let mut world = World::empty(VoxelConfig {
        width: 16,
        height: 14,
        depth: 8,
        voxel_m: 0.25,
        seed: 3,
        day: DayClock { start_hour: 12.0, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    for z in 0..8 {
        for x in 0..16 {
            for y in 1..=2 {
                world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
        for y in 3..=10 {
            world.apply(WorldCommand::SetMaterial { x: 6, y, z, material: Material::Rock });
        }
    }
    world
}

/// The wall's west faces rows 3..=6, z 0..=3, covered by one vine.
fn vine(world: &World) -> (Flora, Vec<Face>) {
    let mut c = FloraConfig::default();
    c.latticevine.spread_cost = 1e12;
    c.latticevine.sister_cost = 1e12;
    let mut flora = Flora::new(c);
    let wf = |y: u32, z: u32| Face::new(6, y, z, FaceDir::NegX);
    let id = flora
        .seed_vine(world, VineSeed { root: Site { x: 5, y: 2, z: 0 }, first: wf(3, 0), reserve: 100.0, lineage: None })
        .expect("fixture: the vine roots");
    let mut faces = vec![wf(3, 0)];
    for z in 0..4 {
        for y in 3..=6 {
            if (y, z) != (3, 0) {
                assert!(flora.cover_face(world, id, wf(y, z), 1.0), "fixture: cover");
                faces.push(wf(y, z));
            }
        }
    }
    (flora, faces)
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

fn held(flora: &Flora, f: Face, phase: SpurPhase) -> f64 {
    flora.spur_stock(f, phase)
}

#[test]
fn a_patch_bite_draws_from_each_face_in_proportion_and_the_books_close() {
    let world = wall();
    let (mut flora, faces) = vine(&world);
    let patch = &faces[..3];
    let stocks = [1.0e-3, 2.0e-3, 4.0e-3];
    for (&f, &s) in patch.iter().zip(&stocks) {
        assert!(flora.set_spur(f, SpurPhase::Fruit, s));
    }
    closes(&flora, "set");
    let total: f64 = stocks.iter().sum();
    let want = 0.5 * total;
    let organic = flora.view().organic();
    let t = flora.take_fruit_patch(patch, want).expect("fruit in the patch");
    assert!((t.organic - want).abs() <= 1e-15, "the whole bite comes out of the patch: {} of {want}", t.organic);
    let shares: Vec<f64> = patch.iter().zip(&stocks).map(|(&f, &s)| (s - held(&flora, f, SpurPhase::Fruit)) / s).collect();
    for s in &shares {
        assert!((s - shares[0]).abs() <= 1e-12, "every face gives the same share of what it held: {shares:?}");
    }
    assert!((flora.view().organic() - (organic - t.organic)).abs() <= 1e-12 * organic.max(1.0), "the layer holds what was taken less");
    closes(&flora, "after the bite");
}

#[test]
fn a_patch_gives_what_one_face_holding_its_total_gives() {
    let world = wall();
    let total = 3.0e-3;
    for want in [0.25 * total, total, 4.0 * total] {
        // One face holding the total.
        let (mut one, faces) = vine(&world);
        assert!(one.set_spur(faces[0], SpurPhase::Fruit, total));
        let a = one.take_fruit_patch(&faces[..1], want).expect("fruit");
        // Four faces holding a quarter each.
        let (mut four, faces) = vine(&world);
        for &f in &faces[..4] {
            assert!(four.set_spur(f, SpurPhase::Fruit, 0.25 * total));
        }
        let b = four.take_fruit_patch(&faces[..4], want).expect("fruit");
        assert!((a.organic - b.organic).abs() <= 1e-15 * total.max(1.0), "want {want}: one face {} against four {}", a.organic, b.organic);
        assert!((a.energy - b.energy).abs() <= 1e-12, "the energy follows the organic");
        closes(&one, "one face");
        closes(&four, "four faces");
    }
}

#[test]
fn asking_past_the_patch_empties_it_and_no_more() {
    let world = wall();
    let (mut flora, faces) = vine(&world);
    let patch = &faces[..4];
    for (k, &f) in patch.iter().enumerate() {
        assert!(flora.set_spur(f, SpurPhase::Flower, 1.0e-3 * (k + 1) as f64));
    }
    let total: f64 = patch.iter().map(|&f| held(&flora, f, SpurPhase::Flower)).sum();
    let t = flora.take_nectar_patch(patch, 10.0 * total).expect("nectar");
    assert!((t.organic - total).abs() <= 1e-15, "all of it: {} of {total}", t.organic);
    for &f in patch {
        assert_eq!(held(&flora, f, SpurPhase::Flower), 0.0, "{f:?} emptied");
    }
    assert!(flora.take_nectar_patch(patch, 1.0).is_none(), "an empty patch gives nothing");
    closes(&flora, "emptied");
}

#[test]
fn faces_in_another_phase_or_listed_twice_give_nothing_extra() {
    let world = wall();
    let (mut flora, faces) = vine(&world);
    assert!(flora.set_spur(faces[0], SpurPhase::Fruit, 2.0e-3));
    assert!(flora.set_spur(faces[1], SpurPhase::Flower, 5.0e-3));
    // A fruit bite over a fruiting face listed twice and a flowering one: only the fruit.
    let t = flora.take_fruit_patch(&[faces[0], faces[0], faces[1]], 1.0).expect("fruit");
    assert!((t.organic - 2.0e-3).abs() <= 1e-15, "{}", t.organic);
    assert_eq!(held(&flora, faces[1], SpurPhase::Flower), 5.0e-3, "the flower keeps its nectar");
    closes(&flora, "mixed phases");
}
