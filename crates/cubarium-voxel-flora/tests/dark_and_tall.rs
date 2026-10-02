//! Glowcaps in the dark and latticevines on tall walls
//! (`design/handoffs/plants-glowcap-vine-2026-10-01.md`): darkness, the dark gates, the seep
//! income and its ledger line, tallest-wall vine founders and the per-m² founder count, and
//! registry rows for every new knob. Small hand-built worlds, a few hundred ticks at most.
//! The spore-bias test lives beside `spore_target` in `src/seeds.rs`.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command, Face, FaceDir, Flora, FloraConfig, Site, Species, SpeciesConfig, VineConfig,
    establishment_gates, registry,
};

// ------------------------------------------------------------------- fixtures

fn world(width: u32, height: u32, depth: u32, voxel_m: f64, seed: u64) -> World {
    World::empty(VoxelConfig {
        width,
        height,
        depth,
        voxel_m,
        seed,
        rain_m_per_s: 0.0,
        ..VoxelConfig::default()
    })
}

fn set(w: &mut World, x: i64, y: u32, z: u32, material: Material) {
    w.apply(WorldCommand::SetMaterial { x, y, z, material });
}

/// Rock at `y = 1..=2` over the whole footprint — no soil anywhere — so every column's
/// support face is `y = 2`, with a rock roof at `y = 4` when `roofed`. Five deep, so a
/// roofed floor can be dark at all (a one-deep strip leaks every leaning ray).
fn rock_cellar(roofed: bool) -> World {
    let (width, depth) = (6u32, 5u32);
    let mut w = world(width, 10, depth, 1.0, 5);
    for z in 0..depth {
        for x in 0..width as i64 {
            for y in 1..=2 {
                set(&mut w, x, y, z, Material::Rock);
            }
            if roofed {
                set(&mut w, x, 4, z, Material::Rock);
            }
        }
    }
    w
}

/// The middle of the cellar floor.
const CELLAR: Site = Site { x: 1, y: 2, z: 2 };

/// The glowcap with every dark knob zeroed: today's glowcap.
fn undarkened(mut sc: SpeciesConfig) -> SpeciesConfig {
    sc.dark_sky_max = 0.0;
    sc.dark_sky_full = 0.0;
    sc.dark_uptake_per_s = 0.0;
    sc.dark_damp = 0.0;
    sc.dark_establish_min = 0.0;
    sc.dark_spore_bias = 0.0;
    sc.dark_founder_share = 0.0;
    sc
}

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

// ------------------------------------------------------------------- 1. darkness

#[test]
fn darkness_is_one_at_and_below_full_half_at_the_midpoint_and_zero_from_max() {
    let sc = SpeciesConfig::glowcap();
    let (max, full) = (sc.dark_sky_max, sc.dark_sky_full);
    assert!(full < max, "the glowcap has a dark habitat: {full} < {max}");
    let mid = 0.5 * (max + full);
    for (sky, want) in [(0.0, 1.0), (full, 1.0), (mid, 0.5), (max, 0.0), (1.0, 0.0)] {
        let got = sc.darkness(sky);
        assert!((got - want).abs() < 1e-12, "darkness at sky {sky} is {got}, not {want}");
    }
    // Off for every other species, and for a glowcap with the knobs zeroed.
    assert_eq!(SpeciesConfig::springturf().darkness(0.0), 0.0);
    assert_eq!(undarkened(SpeciesConfig::glowcap()).darkness(0.0), 0.0);
}

// ------------------------------------------------------------------- 2. the gates

#[test]
fn a_roofed_rock_face_with_nothing_on_it_passes_the_glowcap_gates_only_with_the_dark_knobs() {
    let cellar = rock_cellar(true);
    let view = cellar.view();
    let sc = SpeciesConfig::glowcap();
    let g = establishment_gates(&view, CELLAR, &sc);
    assert_eq!(g.soil_voxels, 0, "fixture: a rock face, no soil in the box");
    assert!(g.sky_visibility <= sc.dark_sky_full, "fixture: the roof leaks: {g:?}");
    assert!(g.passes(), "a fully dark rock face refused the glowcap: {g:?}");

    let off = undarkened(SpeciesConfig::glowcap());
    let g = establishment_gates(&view, CELLAR, &off);
    assert!(!g.passes(), "today's glowcap took a bare dark rock face: {g:?}");

    // An open face with no wood and no litter is still refused, rock or soil.
    let open = rock_cellar(false);
    let g = establishment_gates(&open.view(), CELLAR, &sc);
    assert!(g.sky_visibility >= sc.dark_sky_max, "fixture: the open floor is lit: {g:?}");
    assert!(!g.passes() && !g.substrate_ok, "an open bare face admitted a spore: {g:?}");
}

// ------------------------------------------------------------------- 3. the seep

fn dark_glowcap(sc: SpeciesConfig) -> (Flora, World, f64) {
    let world = rock_cellar(true);
    let mut config = FloraConfig::default();
    config.glowcap = sc;
    let mut flora = Flora::new(config);
    let wood = 0.5 * flora.config().glowcap.wood_max;
    assert!(flora.apply(
        &world,
        Command::SeedOnFace {
            site: CELLAR,
            species: Species::Glowcap,
            wood,
        }
    ));
    (flora, world, wood)
}

#[test]
fn a_dark_glowcap_with_no_substrate_grows_on_the_seep_and_the_ledgers_close() {
    let (mut flora, mut world, wood) = dark_glowcap(SpeciesConfig::glowcap());
    for t in 0..300 {
        flora.step(&mut world);
        if t % 50 == 0 {
            assert_residuals(&flora, &format!("tick {t}"));
        }
    }
    assert_residuals(&flora, "end");
    let stand = *flora.view().stand_at(CELLAR).expect("the glowcap lives");
    assert!(stand.wood > wood + 1e-6, "no growth on the seep: {} from {wood}", stand.wood);
    let l = &flora.view().ledger;
    let sc = &flora.config().glowcap;
    assert!(l.seep_organic_in > 0.0, "no seep booked");
    assert!((l.seep_mineral_in - sc.n_tissue * l.seep_organic_in).abs() < 1e-15);
    assert!((l.seep_energy_in - sc.energy_density * l.seep_organic_in).abs() < 1e-15);
    assert_eq!(l.fixed_in, 0.0, "a saprotroph fixes nothing from light");

    // The same stand with the knobs zeroed earns nothing and does not grow.
    let (mut flora, mut world, wood) = dark_glowcap(undarkened(SpeciesConfig::glowcap()));
    for _ in 0..300 {
        flora.step(&mut world);
    }
    assert_residuals(&flora, "undarkened");
    assert_eq!(flora.view().ledger.seep_organic_in, 0.0);
    let after = flora.view().stand_at(CELLAR).map_or(0.0, |s| s.wood);
    assert!(after <= wood, "today's glowcap grew on bare rock: {after} from {wood}");
}

// ------------------------------------------------------------------- 5. vine founders

/// A soil floor (`y = 1..=2`) one deep, on the 0.25 m voxel, with rock walls `(x, top)`
/// standing on it: a wall at `x` up to `top` has `top − 2` eligible faces on each side.
fn walls(width: u32, list: &[(i64, u32)], seed: u64) -> World {
    let mut w = world(width, 14, 1, 0.25, seed);
    let (wp, fc) = (
        cubarium_voxel::Materials::DEFAULT.wilting_point(Material::Soil),
        cubarium_voxel::Materials::DEFAULT.field_capacity(Material::Soil),
    );
    let cap = cubarium_voxel::Materials::DEFAULT.pore_capacity(Material::Soil)
        * w.config().voxel_volume();
    for x in 0..width as i64 {
        for y in 1..=2 {
            w.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: (wp + 0.8 * (fc - wp)) * cap });
            set(&mut w, x, y, 0, Material::Soil);
        }
    }
    for &(x, top) in list {
        for y in 1..=top {
            set(&mut w, x, y, 0, Material::Rock);
        }
    }
    w
}

#[test]
fn one_founder_roots_at_the_foot_of_the_six_face_wall_not_the_three_face_ones() {
    let mut config = FloraConfig::default();
    config.latticevine.founders = 1;
    config.latticevine.founders_per_m2 = 0.0;
    // The tall wall at x = 20 (faces y = 3..=8), three short ones (y = 3..=5).
    let tall = 20i64;
    for seed in 1..=12u64 {
        let world = walls(40, &[(6, 5), (tall, 8), (30, 5), (36, 5)], seed);
        let mut flora = Flora::new(config.clone());
        assert_eq!(flora.seed_vine_founders(&world), 1, "seed {seed}");
        let v = flora.view().cover.vines()[0].clone();
        assert!(
            v.root_face.x as i64 == tall,
            "seed {seed}: the founder took a short wall: {v:?}"
        );
        assert!(matches!(v.root_face.dir, FaceDir::NegX | FaceDir::PosX));
        assert_eq!(v.root_face.y, 3, "rooted at the foot, on the bottom face: {v:?}");
    }
}

#[test]
fn founders_spread_across_the_floor_before_they_share_a_tall_structure() {
    // Two tall walls a metre apart (x = 10 and 14, faces y = 3..=12) and one short wall
    // far off (x = 50, faces y = 3..=6), on the 0.25 m voxel: 80 voxels is 20 m.
    let world = |seed| walls(80, &[(10, 12), (14, 12), (50, 6)], seed);
    let near_tall = |x: u32| (8..=16).contains(&x);
    let mut config = FloraConfig::default();
    config.latticevine.founders = 2;
    config.latticevine.founders_per_m2 = 0.0;
    config.latticevine.founder_spacing = 2;
    for seed in 1..=8u64 {
        // Off: both founders take the tall pair, as before.
        config.latticevine.founder_spread_m = 0.0;
        let mut flora = Flora::new(config.clone());
        assert_eq!(flora.seed_vine_founders(&world(seed)), 2, "seed {seed}");
        let xs: Vec<u32> = flora.view().cover.vines().iter().map(|v| v.root.x).collect();
        assert!(xs.iter().all(|&x| near_tall(x)), "seed {seed}, spread off: {xs:?}");

        // 3 m apart: the tallest wall first, then the short wall 9 m away, not its
        // neighbour a metre off.
        config.latticevine.founder_spread_m = 3.0;
        let mut flora = Flora::new(config.clone());
        assert_eq!(flora.seed_vine_founders(&world(seed)), 2, "seed {seed}");
        let xs: Vec<u32> = flora.view().cover.vines().iter().map(|v| v.root.x).collect();
        assert_eq!(xs.iter().filter(|&&x| near_tall(x)).count(), 1, "seed {seed}: {xs:?}");
        assert!(xs.iter().any(|&x| (48..=52).contains(&x)), "seed {seed}: {xs:?}");
    }
    // When the floor runs out of spread-apart walls the count still holds: the rest
    // fill at founder_spacing.
    config.latticevine.founders = 3;
    config.latticevine.founder_spread_m = 3.0;
    let mut flora = Flora::new(config);
    assert_eq!(flora.seed_vine_founders(&world(1)), 3);
}

/// One founder on a tall wall with `reserve`, stepped `ticks` with `spread_hold`; leaf
/// regrowth off so spreading is the reserve's only big spender. The wall's frontier grows
/// a face per spread check, so one check (100 ticks) roots at most one face. Returns the
/// founder's faces and its reserve per face.
fn hold_run(spread_hold: f64, reserve: f64, ticks: u32) -> (usize, f64, Flora) {
    let mut world = walls(40, &[(20, 12)], 1);
    let mut config = FloraConfig::default();
    config.latticevine.founders = 1;
    config.latticevine.founders_per_m2 = 0.0;
    config.latticevine.founder_reserve = reserve;
    config.latticevine.regrow_rate = 0.0;
    config.latticevine.spread_hold = spread_hold;
    let mut flora = Flora::new(config);
    assert_eq!(flora.seed_vine_founders(&world), 1);
    for _ in 0..ticks {
        flora.step(&mut world);
    }
    assert_residuals(&flora, &format!("hold {spread_hold}"));
    let v = flora.view().cover.vines()[0];
    let faces = flora.view().cover.faces_of(v.id).len();
    (faces, v.reserve / faces as f64, flora)
}

#[test]
fn a_spreading_vine_keeps_back_its_flowering_reserve() {
    let vc = VineConfig::default();
    let new_cost = vc.spread_cost + vc.contest_threshold * vc.leaf_mass;
    // Just over one face's cost: with no hold the founder spends it on a new face and is
    // left under its spurs' threshold.
    let tight = new_cost + 0.005;
    let (faces, per_face, _) = hold_run(0.0, tight, 100);
    assert_eq!(faces, 2, "no hold: it should have spread");
    assert!(per_face < vc.spur_reserve_min, "no hold kept {per_face} per face");

    // The default hold keeps the flowering share: no new face, the threshold kept.
    let (faces, per_face, _) = hold_run(vc.spread_hold, tight, 100);
    assert_eq!(faces, 1, "the vine spread out of its flowering reserve");
    assert!(per_face >= vc.spur_reserve_min, "{per_face} per face under the spur threshold");

    // With reserve to spare the hold does not stop the spread, and it keeps the share.
    let (faces, per_face, _) = hold_run(vc.spread_hold, 0.5, 400);
    assert!(faces >= 4, "the hold stopped a vine with reserve to spare: {faces} faces");
    assert!(per_face >= vc.spread_hold * vc.spur_reserve_min);
}

#[test]
fn the_founder_count_scales_with_floor_area_over_the_floor() {
    let vc = VineConfig::default();
    assert_eq!(vc.founders, 6);
    assert_eq!(vc.founder_count(192.0), 12, "the desktop terrarium");
    assert_eq!(vc.founder_count(16.0), 6, "a small world keeps the floor");
    let none = VineConfig { founders_per_m2: 0.0, ..VineConfig::default() };
    assert_eq!(none.founder_count(1000.0), 6);

    // And the seeder uses it: 48 × 1 reference voxels is 3 m², at 1 per m² three
    // founders over a floor of 1, on four tall walls twelve voxels apart.
    let mut config = FloraConfig::default();
    config.latticevine.founders = 1;
    config.latticevine.founders_per_m2 = 1.0;
    let world = walls(48, &[(6, 8), (18, 8), (30, 8), (42, 8)], 3);
    let mut flora = Flora::new(config);
    assert_eq!(flora.seed_vine_founders(&world), 3);
}

// ------------------------------------------------------------------- 6. the registry

#[test]
fn every_new_knob_has_a_registry_row() {
    for path in [
        "flora.glowcap.dark_sky_max",
        "flora.glowcap.dark_sky_full",
        "flora.glowcap.dark_uptake_per_s",
        "flora.glowcap.dark_damp",
        "flora.glowcap.dark_establish_min",
        "flora.glowcap.dark_spore_bias",
        "flora.glowcap.dark_founder_share",
        "flora.springturf.dark_sky_max",
        "flora.latticevine.founders_per_m2",
        "flora.latticevine.founder_spread_m",
        "flora.latticevine.spread_hold",
    ] {
        assert!(registry().row_for(path).is_some(), "no registry row for {path}");
    }
    let _ = Face::new(0, 0, 0, FaceDir::PosX);
}
