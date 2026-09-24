//! Hot-path geometry, item 2 (`design/handoffs/voxel-hot-path-geometry-2026-09-24.md`):
//! a stand's crown geometry, cached where the flora keeps its stands.
//!
//! Everything about a crown that follows from its species, its wood and the voxel size —
//! the profile stage, the crown's height and radius, `crown_voxels`, every layer's disc
//! cell and trunk run — is computed when the stand appears or its wood moves and read
//! afterwards. The one requirement is that the cache **is** a fresh computation: every
//! test here checks, for every stand, that the view holds a current entry and that it,
//! the view's layers and its `crown_voxels` equal [`Crown::of`], [`layers_of`] and
//! `SpeciesConfig::crown_voxels` computed from scratch — to the bit — after growth and
//! dieback, a bite, a death, a vaulttree's fall and a new seedling. Stocks are state and
//! never cached: a bite shows in the view's layers at once.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command, Crown, Flora, FloraConfig, FloraView, Site, Species, Stand, StandLayer, layers_of,
};

/// The plain's support layer.
const SUPPORT: u32 = 2;

/// Bit-level equality: `PartialEq` on floats would pass `-0.0 == 0.0`.
fn bits(l: &StandLayer) -> Vec<u64> {
    let mut v = vec![
        l.index as u64,
        l.foliage_index.map_or(u64::MAX, |i| i as u64),
        l.cell as u64,
        l.cells.0 as u64,
        l.cells.1 as u64,
    ];
    v.extend(
        [
            l.band_v[0],
            l.band_v[1],
            l.band_m[0],
            l.band_m[1],
            l.radius_v,
            l.radius_m,
            l.share,
            l.capacity,
            l.stock,
            l.floor,
            l.porosity,
            l.area_m2,
        ]
        .map(f64::to_bits),
    );
    v
}

fn all_bits<'a>(layers: impl IntoIterator<Item = &'a StandLayer>) -> Vec<Vec<u64>> {
    layers.into_iter().map(bits).collect()
}

/// A wet soil plain in `1..=SUPPORT`, tall enough for a vaulttree and a pool deep
/// enough to drown one. The soil holds pore water (poured into the air cell before it
/// becomes soil), so the stands grow and their wood moves every tick.
fn plain(voxel_m: f64) -> World {
    let height = ((4.0 / voxel_m).ceil() as u32) + SUPPORT + 3;
    let (width, depth) = (28u32, 3u32);
    let mut w = World::empty(VoxelConfig {
        width,
        height,
        depth,
        voxel_m,
        seed: 31,
        ..VoxelConfig::default()
    });
    let pore = Material::Soil.pore_capacity() * w.config().voxel_volume() * 0.35;
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=SUPPORT {
                w.apply(WorldCommand::AddWater {
                    x,
                    y,
                    z,
                    volume_m3: pore,
                });
                w.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    w
}

/// Every stand of the view: a current cache entry, equal to a fresh computation, and the
/// view's readings through it equal to the uncached ones, to the bit.
fn check(view: &FloraView<'_>, what: &str) -> usize {
    let config = view.config;
    let voxel_m = config.voxel_m;
    for stand in view.stands {
        let sc = config.species(stand.species);
        let fresh = Crown::of(config, stand, voxel_m);
        let cached = view
            .cached_crown(stand)
            .unwrap_or_else(|| panic!("{what}: {:?} at {:?} has no current crown", stand.species, stand.site));
        assert_eq!(*cached, fresh, "{what}: {:?} at {:?}", stand.species, stand.site);
        assert!(cached.matches(stand), "{what}");
        assert_eq!(
            view.crown_voxels(stand),
            sc.crown_voxels(stand.wood, voxel_m),
            "{what}: crown_voxels of {:?}",
            stand.species
        );
        assert_eq!(cached.crown_voxels(), sc.crown_voxels(stand.wood, voxel_m), "{what}");
        let profile = layers_of(config, stand, voxel_m);
        let foliage: Vec<StandLayer> = profile
            .iter()
            .copied()
            .filter(|l| l.kind.bears_foliage())
            .collect();
        let got: Vec<StandLayer> = view.layers(stand).collect();
        assert_eq!(all_bits(&got), all_bits(&foliage), "{what}: layers of {:?}", stand.species);
        assert_eq!(
            all_bits(&view.profile_layers(stand)),
            all_bits(&profile),
            "{what}: profile of {:?}",
            stand.species
        );
        // A copy is not the view's stand: no cache entry is lent to it, and its readings
        // are its own even when its wood is not the stand's.
        let mut copy: Stand = *stand;
        copy.wood *= 0.61;
        let want: Vec<StandLayer> = layers_of(config, &copy, voxel_m)
            .into_iter()
            .filter(|l| l.kind.bears_foliage())
            .collect();
        let read: Vec<StandLayer> = view.layers(&copy).collect();
        assert_eq!(all_bits(&read), all_bits(&want), "{what}: a copy of {:?}", stand.species);
        assert_eq!(view.crown_voxels(&copy), sc.crown_voxels(copy.wood, voxel_m), "{what}");
    }
    view.stands.len()
}

/// The reach index against its definition: at every column of the world, the stands
/// listed are exactly those whose widest layer disc boxes it — within `max_disc_span`
/// in wrapped `x` and in `z` — ascending; off the strip, none.
fn check_index(view: &FloraView<'_>, world: &World, what: &str) {
    let c = world.config();
    let (w, d) = (i64::from(c.width), i64::from(c.depth));
    for z in 0..c.depth {
        for x in 0..w {
            let got = view
                .stands_near(x, z, c.width, c.depth)
                .unwrap_or_else(|| panic!("{what}: no current reach index"));
            let want: Vec<u32> = view
                .stands
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    let span = Crown::of(view.config, s, view.config.voxel_m).max_disc_span();
                    let dx = (x - i64::from(s.site.x)).rem_euclid(w);
                    let dx = dx.min(w - dx);
                    let dz = (i64::from(z) - i64::from(s.site.z)).abs();
                    dx <= span && dz <= span
                })
                .map(|(i, _)| i as u32)
                .collect();
            assert_eq!(got, &want[..], "{what}: column ({x}, {z})");
        }
    }
    assert_eq!(view.stands_near(-1, 0, c.width, c.depth), Some(&[][..]), "{what}");
    assert_eq!(view.stands_near(0, c.depth, c.width, c.depth), Some(&[][..]), "{what}");
    assert_eq!(view.stands_near(0, 0, c.width + 1, c.depth), None, "{what}: another world");
    let _ = d;
}

fn seed(flora: &mut Flora, world: &World, x: i64, species: Species, fraction: f64) -> bool {
    let wood = flora.config().species(species).wood_max * fraction;
    flora.apply(
        world,
        Command::Seed {
            x,
            z: 1,
            species,
            wood,
        },
    )
}

/// Pour water over `site` until it stands deeper than `depth_m`.
fn flood(w: &mut World, site: Site, depth_m: f64) {
    let cell = w.config().voxel_volume();
    let mut y = site.y + 1;
    while !(w.view().water_depth_m(i64::from(site.x), site.y, site.z) > depth_m) {
        assert!(y + 1 < w.config().height, "the fixture is too short to drown");
        w.apply(WorldCommand::AddWater {
            x: i64::from(site.x),
            y,
            z: site.z,
            volume_m3: cell,
        });
        y += 1;
    }
}

#[test]
fn the_cache_is_a_fresh_computation_through_growth_bites_deaths_a_fall_and_a_seedling() {
    for voxel_m in [0.25, 0.125] {
        let mut world = plain(voxel_m);
        let mut flora = Flora::new(FloraConfig::for_voxel_size(voxel_m));
        // One stand of every species, the vaulttree in the middle with room to fall.
        let mut x = 3i64;
        for species in Species::ALL {
            let at = if species == Species::Vaulttree { 14 } else { x };
            assert!(seed(&mut flora, &world, at, species, 0.55), "{species:?} took");
            x += 2;
            if x == 14 {
                x += 2;
            }
        }
        assert_eq!(check(&flora.view(), "seeded"), Species::ALL.len());
        check_index(&flora.view(), &world, "seeded");

        // Growth and dieback: the wood moves every tick, and the cache with it.
        let before: Vec<f64> = flora.view().stands.iter().map(|s| s.wood).collect();
        for t in 0..40 {
            world.step_with(1);
            flora.step(&mut world);
            check(&flora.view(), &format!("tick {t}"));
            check_index(&flora.view(), &world, &format!("tick {t}"));
        }
        let moved = flora
            .view()
            .stands
            .iter()
            .zip(&before)
            .filter(|(s, w)| s.wood.to_bits() != w.to_bits())
            .count();
        assert!(moved > 0, "the run moved some wood, so the cache was exercised");

        // A bite: stocks move, geometry does not, and the view reads the new stocks.
        let bitten = flora
            .view()
            .stands
            .iter()
            .find(|s| s.foliage > 0.0 && s.species != Species::Glowcap)
            .map(|s| s.site)
            .expect("a leafy stand");
        let stock_before = flora.view().stand_at(bitten).map(|s| s.foliage).unwrap();
        let taken = flora.take_foliage_in_layers(bitten, stock_before * 0.3, &(0..=i64::MAX));
        assert!(taken.is_some(), "the bite took something");
        check(&flora.view(), "after a bite");

        // A death in the middle of the stand list: every stand after it moves down one.
        let middle = flora.view().stands[flora.view().stands.len() / 2].site;
        assert!(flora.apply(
            &world,
            Command::Clear {
                x: i64::from(middle.x),
                z: middle.z,
            }
        ));
        assert!(flora.view().stand_at(middle).is_none());
        check(&flora.view(), "after a clear");
        check_index(&flora.view(), &world, "after a clear");

        // The vaulttree's fall: drowned, its wood laid along the fall line.
        let tree = flora
            .view()
            .stands
            .iter()
            .find(|s| s.species == Species::Vaulttree)
            .map(|s| s.site)
            .expect("the vaulttree stands");
        let drown = flora.config().species(Species::Vaulttree).drown_depth_m;
        flood(&mut world, tree, drown);
        flora.step(&mut world);
        assert!(flora.view().stand_at(tree).is_none(), "the vaulttree fell");
        assert!(
            flora.view().ground.iter().any(|g| g.dead_wood > 0.0),
            "it left logs"
        );
        check(&flora.view(), "after the fall");
        check_index(&flora.view(), &world, "after the fall");

        // A new seedling at the front of the list: every stand moves up one.
        assert!(seed(&mut flora, &world, 0, Species::Bloomcrown, 0.05));
        assert_eq!(flora.view().stands[0].site.x, 0);
        check(&flora.view(), "after a seedling");
        check_index(&flora.view(), &world, "after a seedling");
        for t in 0..10 {
            world.step_with(1);
            flora.step(&mut world);
            check(&flora.view(), &format!("seedling tick {t}"));
        }
    }
}

/// A flora read back from its snapshot, or cloned, reads the same geometry: the cache is
/// derived state and never saved.
#[test]
fn a_loaded_or_cloned_flora_reads_the_same_crowns() {
    let voxel_m = 0.25;
    let mut world = plain(voxel_m);
    let mut flora = Flora::new(FloraConfig::for_voxel_size(voxel_m));
    for (k, species) in Species::ALL.iter().enumerate() {
        seed(&mut flora, &world, 1 + 3 * k as i64, *species, 0.4);
    }
    for _ in 0..5 {
        world.step_with(1);
        flora.step(&mut world);
    }
    let clone = flora.clone();
    check(&clone.view(), "clone");
    let loaded = cubarium_voxel_flora::snapshot::decode(&cubarium_voxel_flora::snapshot::encode(&flora))
        .expect("round trip");
    check(&loaded.view(), "loaded");
    // A decode has no world in hand: no index until the next tick or command, and a
    // reader is told so rather than handed a stale one.
    let c = world.config();
    assert_eq!(loaded.view().stands_near(0, 0, c.width, c.depth), None);
    let mut loaded = loaded;
    loaded.step(&mut world);
    check_index(&loaded.view(), &world, "loaded, stepped");
}
