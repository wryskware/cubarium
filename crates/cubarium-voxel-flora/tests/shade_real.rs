//! **SHADE-REAL** (`design/handoffs/vault-spacing-2026-10-10.md`): Beer–Lambert crown shade
//! from leaf area for every plant, crowns of one height shading each other by half, and
//! the vaulttree's winged seed on the mechanistic WALD from its crown.
//!
//! Every check is a mechanism on hand-built stands, against whatever the config says: no
//! test reads a shipped coefficient, leaf area, wind or light as a number to match.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command, FaceLayer, Flora, FloraConfig, Site, Species, shade_share};

const VOXEL_M: f64 = 0.125;
const SUPPORT: u32 = 2;

/// A one-row plain at 0.125 m: bedrock floor, soil in `1..=SUPPORT` at field capacity,
/// open sky over every column.
fn plain(width: u32) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height: 80,
        depth: 1,
        voxel_m: VOXEL_M,
        seed: 31,
        ..VoxelConfig::default()
    });
    let pore = cubarium_voxel::Materials::DEFAULT.field_capacity(Material::Soil);
    let cap = w.config().materials.pore_capacity(Material::Soil) * w.config().voxel_volume();
    for x in 0..i64::from(width) {
        for y in 1..=SUPPORT {
            w.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: pore * cap });
            w.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
        }
    }
    w
}

fn at(x: u32) -> Site {
    Site { x, y: SUPPORT, z: 0 }
}

/// A flora on `world` holding `stands` (`x`, species, wood), its light published without a
/// tick ([`Flora::refresh_founder_light`]).
fn lit(world: &World, stands: &[(u32, Species, f64)]) -> Flora {
    let mut flora = Flora::new(FloraConfig::for_voxel_size(VOXEL_M));
    for &(x, species, wood) in stands {
        assert!(flora.apply(world, Command::Seed { x: i64::from(x), z: 0, species, wood }));
    }
    flora.refresh_founder_light(world);
    flora
}

fn light(flora: &Flora, x: u32, species: Species) -> f64 {
    flora
        .view()
        .stands
        .iter()
        .find(|s| s.site == at(x) && s.species == species)
        .unwrap_or_else(|| panic!("a {} at {x}", species.name()))
        .light
}

fn wood_max(sp: Species) -> f64 {
    FloraConfig::for_voxel_size(VOXEL_M).species(sp).wood_max
}

/// The fixture's premise: `b` stands inside `a`'s crown.
fn covers(a: (u32, Species, f64), b: u32) -> bool {
    let sc = FloraConfig::for_voxel_size(VOXEL_M).species(a.1).clone();
    f64::from(a.0.abs_diff(b)) <= sc.crown_radius(a.2, VOXEL_M)
}

/// **More foliage over a column means less light under it.** The same vaulttree over the
/// same turf, once full and once browsed: the browsed crown's layers each hold no more
/// optical depth than the full one's, at least one less, and the turf under it reads more
/// light.
#[test]
fn more_foliage_over_a_column_means_less_light_under_it() {
    let world = plain(48);
    let tree = (10, Species::Vaulttree, wood_max(Species::Vaulttree));
    assert!(covers(tree, 14), "fixture: the turf stands under the crown");
    let turf = (14, Species::Springturf, wood_max(Species::Springturf));
    let full = lit(&world, &[tree, turf]);
    let mut browsed = lit(&world, &[tree, turf]);
    let taken = browsed.take_foliage(at(10), 1e9).expect("the crown gives foliage");
    assert!(taken.organic > 0.0, "fixture: something was browsed");
    browsed.refresh_founder_light(&world);

    let depths = |f: &Flora| -> Vec<f64> {
        let v = f.view();
        let s = v.stands.iter().find(|s| s.site == at(10)).unwrap();
        v.layers(s).map(|l| l.optical_depth).collect()
    };
    let (d_full, d_browsed) = (depths(&full), depths(&browsed));
    assert_eq!(d_full.len(), d_browsed.len());
    assert!(d_full.iter().zip(&d_browsed).all(|(a, b)| b <= a), "{d_full:?} against {d_browsed:?}");
    assert!(d_full.iter().zip(&d_browsed).any(|(a, b)| b < a), "{d_full:?} against {d_browsed:?}");

    let (under_full, under_browsed) = (light(&full, 14, Species::Springturf), light(&browsed, 14, Species::Springturf));
    assert!(under_full < under_browsed, "full crown {under_full}, browsed {under_browsed}");
    assert!(under_browsed <= 1.0);
}

/// **Equal-height overlapping crowns shade each other symmetrically.** Two full vaulttrees
/// inside each other's crowns on level ground read the same light, less than the open
/// sky a lone one of the same size reads.
#[test]
fn equal_height_overlapping_crowns_shade_each_other_symmetrically() {
    let world = plain(64);
    let w = wood_max(Species::Vaulttree);
    assert!(covers((10, Species::Vaulttree, w), 14) && covers((14, Species::Vaulttree, w), 10));
    let lone = (50, Species::Vaulttree, w);
    assert!(!covers((14, Species::Vaulttree, w), 50), "fixture: the lone tree stands clear");
    let flora = lit(&world, &[(10, Species::Vaulttree, w), (14, Species::Vaulttree, w), lone]);
    let (a, b, alone) = (
        light(&flora, 10, Species::Vaulttree),
        light(&flora, 14, Species::Vaulttree),
        light(&flora, 50, Species::Vaulttree),
    );
    assert_eq!(a, b, "a level pair shades symmetrically");
    assert!(a < alone, "a level pair shades: {a} against the lone tree's {alone}");
}

/// **A taller crown takes no less light than before**: a shorter neighbour inside its
/// crown takes none of its light (it reads exactly what it reads with the neighbour
/// cleared), while the shorter one is shaded.
#[test]
fn a_taller_crown_takes_no_less_light_than_before() {
    let world = plain(48);
    let tall = (10, Species::Vaulttree, wood_max(Species::Vaulttree));
    let short = (14, Species::Vaulttree, 0.5 * wood_max(Species::Vaulttree));
    let sc = FloraConfig::for_voxel_size(VOXEL_M).species(Species::Vaulttree).clone();
    assert!(sc.crown_height(short.2, VOXEL_M) < sc.crown_height(tall.2, VOXEL_M));
    assert!(covers(tall, 14) && covers(short, 10), "fixture: the crowns overlap");
    let pair = lit(&world, &[tall, short]);
    let alone = lit(&world, &[tall]);
    assert_eq!(
        light(&pair, 10, Species::Vaulttree),
        light(&alone, 10, Species::Vaulttree),
        "the shorter neighbour takes nothing from the taller crown"
    );
    assert!(light(&pair, 14, Species::Vaulttree) < light(&alone, 10, Species::Vaulttree));
}

/// **The share rule itself**: none from a band whose top is below the receiver's, half
/// between equal bands, all of a band wholly above the receiver's middle, and never less
/// as the occluder rises.
#[test]
fn the_share_is_none_below_half_level_and_all_above() {
    for &(lo, hi) in &[(0.0, 1.0), (2.0, 5.5), (3.0, 3.0)] {
        let r = [lo, hi];
        assert_eq!(shade_share([lo - 1.0, hi - 0.01], r), 0.0, "a lower top shades nothing");
        assert_eq!(shade_share(r, r), 0.5, "equal bands share by half: {r:?}");
        let mid = 0.5 * (lo + hi);
        assert_eq!(shade_share([mid + 0.01, hi + 1.0], r), 1.0, "wholly above the middle");
        let mut last = 0.0;
        for k in 0..40 {
            let rise = f64::from(k) * 0.1;
            let s = shade_share([lo + rise, hi + rise], r);
            assert!(s >= last && (0.0..=1.0).contains(&s), "rise {rise}: {s} after {last}");
            last = s;
        }
    }
}

/// **The fungus exception and `same_face_shades` are unchanged**: a face's fungus never
/// shades the plant it shares the face with (the plant reads what it reads alone), while
/// the cap's exposure still has its own face's plant over it.
#[test]
fn a_faces_fungus_never_shades_its_plant_and_its_plant_still_shades_the_cap() {
    let world = plain(24);
    let config = FloraConfig::for_voxel_size(VOXEL_M);
    let plant = Species::Bloomcrown;
    let cap = Species::Glowcap;
    assert!(
        config.species(plant).crown_height(wood_max(plant), VOXEL_M)
            > config.species(cap).crown_height(wood_max(cap), VOXEL_M),
        "fixture: the plant stands over the cap"
    );
    let make = |with_plant: bool, with_cap: bool| {
        let mut flora = Flora::new(config.clone());
        if with_plant {
            assert!(flora.apply(&world, Command::SeedOnFace { site: at(8), species: plant, wood: wood_max(plant) }));
        }
        if with_cap {
            assert!(flora.apply(&world, Command::SeedOnFace { site: at(8), species: cap, wood: wood_max(cap) }));
        }
        flora.refresh_founder_light(&world);
        flora
    };
    let both = make(true, true);
    let read = |f: &Flora, place: FaceLayer| f.view().stands.iter().find(|s| s.site == at(8) && s.place == place).unwrap().light;
    assert_eq!(read(&both, FaceLayer::Plant), read(&make(true, false), FaceLayer::Plant), "the cap shades no plant");
    assert!(
        read(&both, FaceLayer::Fungus) < read(&make(false, true), FaceLayer::Fungus),
        "the cap's exposure has its own face's plant over it"
    );
}

/// **The vaulttree's kernel follows its crown and the wind**: from the crown, the mean
/// grows with the donor's height and with the wind and shrinks with the fall speed, and
/// the wind fattens the tail (`CV² = mean / shape`); with no release height the fixed
/// kernel is untouched.
#[test]
fn the_vaulttree_kernel_mean_grows_with_release_height_and_wind() {
    let config = FloraConfig::for_voxel_size(VOXEL_M);
    let mut sc = config.vaulttree.clone();
    if !(sc.seed_release_share > 0.0) {
        sc.seed_release_share = 0.8;
    }
    let (u, a) = (config.seed_wind_m_per_s.max(0.5), config.seed_eddy_ratio);
    let young = 0.3 * sc.wood_max;
    assert!(sc.crown_height_m_at(young) < sc.crown_height_m_at(sc.wood_max), "fixture: a taller donor");
    let (m_young, _) = sc.wald_kernel(young, u, a);
    let (m_old, s_old) = sc.wald_kernel(sc.wood_max, u, a);
    assert!(m_young > 0.0 && m_old > m_young, "release height: {m_young} then {m_old}");
    let (m_windy, s_windy) = sc.wald_kernel(sc.wood_max, 2.0 * u, a);
    assert!(m_windy > m_old, "wind: {m_old} then {m_windy}");
    assert!(m_windy / s_windy > m_old / s_old, "the wind fattens the tail");
    let mut heavy = sc.clone();
    heavy.seed_fall_m_per_s = 2.0 * sc.seed_fall_m_per_s;
    assert!(heavy.wald_kernel(sc.wood_max, u, a).0 < m_old, "a faster fall lands nearer");
    let mut fixed = sc.clone();
    fixed.seed_release_share = 0.0;
    fixed.wald_mean_m = 1.5;
    fixed.wald_shape_m = 0.7;
    assert_eq!(fixed.wald_kernel(sc.wood_max, u, a), (1.5, 0.7), "no release height: the fixed kernel");
}
