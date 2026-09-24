//! The study observation cache: one hemisphere ray per site, shared across the six species
//! and reused across observations while the terrain is unchanged.
//!
//! `design/handoffs/voxel-study-observer-cache-2026-09-18.md` asks for the cached gate
//! fields to be the uncached ones on open and roofed fixtures, across a terrain edit and
//! across worlds that share a version number, and for water and dead wood to be re-read on
//! every observation. These are those checks, on small hand-built worlds. `voxel_m` is 1 m
//! so one voxel of standing water is a metre deep.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Deposit, DepositKind, Flora, FloraConfig, Gates, Site, SkyCache, Species,
};

// ------------------------------------------------------------------- fixtures

/// One slab deep: a bedrock floor, soil at `y = 1` and `y = 2` at pore 0.2, air above.
/// Every column's support face is `y = 2`, in open sky.
fn plain(width: u32, height: u32) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height,
        depth: 1,
        voxel_m: 1.0,
        seed: 23,
        ..VoxelConfig::default()
    });
    for x in 0..width as i64 {
        for y in 1..=2 {
            let cap = Material::Soil.pore_capacity() * w.config().voxel_volume();
            w.apply(WorldCommand::AddWater {
                x,
                y,
                z: 0,
                // Drained soil (package F's available water 1): every species' water
                // gate but the wetland pair's is open on any soil numbers.
                volume_m3: Material::Soil.field_capacity() * cap,
            });
            w.apply(WorldCommand::SetMaterial {
                x,
                y,
                z: 0,
                material: Material::Soil,
            });
        }
    }
    w
}

fn roof(world: &mut World, x: i64, y: u32) {
    world.apply(WorldCommand::SetMaterial {
        x,
        y,
        z: 0,
        material: Material::Soil,
    });
}

fn sites_of(world: &World) -> Vec<Site> {
    (0..world.config().width)
        .map(|x| Site { x, y: 2, z: 0 })
        .collect()
}

/// The uncached reading at every site, for comparison with the cached one.
fn uncached(world: &World, flora: &Flora, sites: &[Site], species: Species) -> Vec<Gates> {
    let view = world.view();
    sites
        .iter()
        .map(|s| flora.view().establishment_gates(&view, *s, species))
        .collect()
}

fn cached(
    world: &World,
    flora: &Flora,
    sites: &[Site],
    species: Species,
    sky: &mut SkyCache,
) -> Vec<Gates> {
    flora
        .view()
        .establishment_gates_over(&world.view(), sites, species, sky)
}

/// Every cached gate field is the uncached one, for every species, on this fixture.
fn assert_cached_equals_uncached(world: &World, flora: &Flora, sites: &[Site], label: &str) {
    let mut sky = SkyCache::new();
    for species in Species::ALL {
        let got = cached(world, flora, sites, species, &mut sky);
        let want = uncached(world, flora, sites, species);
        assert_eq!(got, want, "{label}: {} gate fields", species.name());
    }
    // One reading per site, whatever the species: memory is bounded by the skyline.
    assert_eq!(sky.len(), sites.len(), "{label}: cache size");
}

// ================================================ cached readings are the model's
//
// Open sky and a roof: the cache must reproduce the uncached predicate exactly on both,
// and the roof must actually move the geometry it shares.

#[test]
fn cached_fields_are_the_uncached_fields_on_open_and_roofed_ground() {
    let flora = Flora::new(FloraConfig::default());
    let open = plain(6, 6);
    assert_cached_equals_uncached(&open, &flora, &sites_of(&open), "open");

    let mut roofed = plain(6, 6);
    roof(&mut roofed, 2, 4);
    assert_cached_equals_uncached(&roofed, &flora, &sites_of(&roofed), "roofed");

    // The roof is a real change to the shared geometry, not a no-op fixture.
    let sites = sites_of(&roofed);
    let open_sky = cached(
        &open,
        &flora,
        &sites,
        Species::Bloomcrown,
        &mut SkyCache::new(),
    );
    let roofed_sky = cached(
        &roofed,
        &flora,
        &sites,
        Species::Bloomcrown,
        &mut SkyCache::new(),
    );
    assert!(
        roofed_sky[2].sky_visibility < open_sky[2].sky_visibility,
        "a roof at (2, 4) must lower the sky over x = 2"
    );
}

// ==================================================== a terrain edit invalidates
//
// One cache, one world, a material change between two observations: the second reading must
// be the new terrain's and not the first's.

#[test]
fn a_terrain_edit_is_seen_by_the_next_cached_observation() {
    let mut world = plain(6, 6);
    let flora = Flora::new(FloraConfig::default());
    let sites = sites_of(&world);
    let mut sky = SkyCache::new();

    let before = cached(&world, &flora, &sites, Species::Bloomcrown, &mut sky);
    assert_eq!(sky.len(), sites.len());

    roof(&mut world, 2, 4);
    let after = cached(&world, &flora, &sites, Species::Bloomcrown, &mut sky);

    assert!(
        after[2].sky_visibility < before[2].sky_visibility,
        "the cached reading moved with the roof"
    );
    assert_eq!(after, uncached(&world, &flora, &sites, Species::Bloomcrown));
    // The cache did not grow a second copy of the site list.
    assert_eq!(sky.len(), sites.len());
}

// ============================================== another world, the same version
//
// Two worlds built from the same floor plan and given exactly one material command each hold
// the same `terrain_version` and different roofs. One cache used on both must not hand the
// first world's geometry to the second.

#[test]
fn a_cache_does_not_cross_a_world_that_shares_a_version_number() {
    let flora = Flora::new(FloraConfig::default());
    let mut a = plain(6, 6);
    let mut b = plain(6, 6);
    roof(&mut a, 1, 4);
    roof(&mut b, 2, 4);
    assert_eq!(
        a.terrain_version(),
        b.terrain_version(),
        "the fixture is the same version with a different roof"
    );

    let sites = sites_of(&a);
    let mut sky = SkyCache::new();
    let from_a = cached(&a, &flora, &sites, Species::Bloomcrown, &mut sky);
    let from_b = cached(&b, &flora, &sites, Species::Bloomcrown, &mut sky);

    assert_eq!(from_b, uncached(&b, &flora, &sites, Species::Bloomcrown));
    assert_ne!(
        from_a[1].sky_visibility, from_b[1].sky_visibility,
        "the two floor plans read differently at their own roofs"
    );

    // A clone shares the terrain, so its cached reading is right too.
    let clone = a.clone();
    let from_clone = cached(&clone, &flora, &sites, Species::Bloomcrown, &mut sky);
    assert_eq!(
        from_clone,
        uncached(&a, &flora, &sites, Species::Bloomcrown)
    );
}

// ============================================== another world, another shape
//
// Identical material bytes and an identical `terrain_version` under different grid
// dimensions are different geometry: the flat index layout and the ray march both read
// `Config`, so the cache key must name the shape. Here a 2 x 3 x 1 world and a 1 x 1 x 6
// world are driven to the same six material cells and the same version, and one cache is
// asked about the same site in both.

#[test]
fn a_cache_does_not_cross_a_differently_shaped_world() {
    let config = |width, height, depth| VoxelConfig {
        width,
        height,
        depth,
        voxel_m: 1.0,
        seed: 23,
        ..VoxelConfig::default()
    };
    // P: 2 x 3 x 1, index = 2y + x. Target [Bedrock, Bedrock, Air, Air, Soil, Air] with the
    // soil as a roof at (0, 2, 0) over the support at (0, 0, 0).
    let mut p = World::empty(config(2, 3, 1));
    p.apply(WorldCommand::SetMaterial {
        x: 0,
        y: 2,
        z: 0,
        material: Material::Soil,
    });
    assert_eq!(p.terrain_version(), 1);
    // Pad P to Q's version with a three-step change-and-revert of its own floor cell: the
    // bytes come back and the version advances, so the two worlds agree on both.
    for m in [Material::Rock, Material::Soil, Material::Bedrock] {
        p.apply(WorldCommand::SetMaterial {
            x: 0,
            y: 0,
            z: 0,
            material: m,
        });
    }

    // Q: 1 x 1 x 6, index = z. The same bytes land on a grid one cell tall.
    let mut q = World::empty(config(1, 1, 6));
    q.apply(WorldCommand::SetMaterial {
        x: 0,
        y: 0,
        z: 2,
        material: Material::Air,
    });
    q.apply(WorldCommand::SetMaterial {
        x: 0,
        y: 0,
        z: 3,
        material: Material::Air,
    });
    q.apply(WorldCommand::SetMaterial {
        x: 0,
        y: 0,
        z: 4,
        material: Material::Soil,
    });
    q.apply(WorldCommand::SetMaterial {
        x: 0,
        y: 0,
        z: 5,
        material: Material::Air,
    });

    assert_eq!(
        p.view().material,
        q.view().material,
        "the fixture is the same bytes under different dimensions"
    );
    assert_eq!(
        p.terrain_version(),
        q.terrain_version(),
        "the fixture shares a version number as well"
    );

    let sites = vec![Site { x: 0, y: 0, z: 0 }];
    let flora = Flora::new(FloraConfig::default());
    let mut sky = SkyCache::new();
    let from_p = cached(&p, &flora, &sites, Species::Bloomcrown, &mut sky);
    let from_q = cached(&q, &flora, &sites, Species::Bloomcrown, &mut sky);

    assert_eq!(from_p, uncached(&p, &flora, &sites, Species::Bloomcrown));
    assert_eq!(from_q, uncached(&q, &flora, &sites, Species::Bloomcrown));
    assert!(
        from_p[0].sky_visibility < 1.0,
        "the roof over P's site must shut part of its sky"
    );
    assert_eq!(
        from_q[0].sky_visibility, 1.0,
        "Q is one cell tall, so every site is top-of-world open sky"
    );
    assert_ne!(
        from_p[0].sky_visibility, from_q[0].sky_visibility,
        "the same bytes under another shape read differently"
    );
}

// ============================================ water and dead wood are read afresh
//
// Fixed terrain, one reused cache, and the two inputs a gate can read that are not geometry:
// standing water and the saprotroph's dead wood. The next observation must respond to both.

#[test]
fn cached_observations_re_read_water_and_dead_wood() {
    let mut world = plain(6, 6);
    let mut flora = Flora::new(FloraConfig::default());
    let sites = sites_of(&world);
    let site = sites[1];
    let mut sky = SkyCache::new();

    let before = cached(&world, &flora, &sites, Species::Glowcap, &mut sky);
    assert!(!before[1].substrate_ok, "a fresh world holds no dead wood");

    // Fixed terrain (this only writes free water), a log and a puddle: the cache key is
    // unchanged, so both readings must come off the new world and the new ground.
    world.apply(WorldCommand::AddWater {
        x: site.x as i64,
        y: 3,
        z: site.z,
        volume_m3: 1.0,
    });
    assert!(flora.deposit(
        site,
        Deposit {
            kind: DepositKind::DeadWood,
            organic: 1.0,
            mineral: 0.0,
            energy: 0.0,
        }
    ));

    let after = cached(&world, &flora, &sites, Species::Glowcap, &mut sky);
    assert!(
        after[1].water_depth_m > before[1].water_depth_m,
        "the standing water is in the depth gate"
    );
    assert!(after[1].substrate_ok, "the log is in the substrate gate");
    assert_eq!(
        after,
        uncached(&world, &flora, &sites, Species::Glowcap),
        "the cached observation is still the predicate"
    );
    assert_eq!(
        after[1].sky_visibility, before[1].sky_visibility,
        "water and wood did not move the geometry"
    );
}

// ============================================================== eligibility sets
//
// The joined shape a study actually reads: the sorted set of sites a species passes, cached
// and uncached, on a fixture where one species is shut out by the roof and the saprotroph by
// its substrate.

#[test]
fn cached_and_uncached_eligible_sets_agree() {
    let mut world = plain(8, 6);
    roof(&mut world, 3, 4);
    let mut flora = Flora::new(FloraConfig::default());
    let sites = sites_of(&world);
    // A log on one site only, so the saprotroph's set is non-empty and smaller than the rest.
    assert!(flora.deposit(
        sites[5],
        Deposit {
            kind: DepositKind::DeadWood,
            organic: 1.0,
            mineral: 0.0,
            energy: 0.0,
        }
    ));

    let mut sky = SkyCache::new();
    for species in Species::ALL {
        let got: Vec<Site> = sites
            .iter()
            .copied()
            .zip(cached(&world, &flora, &sites, species, &mut sky))
            .filter(|(_, g)| g.passes())
            .map(|(s, _)| s)
            .collect();
        let want: Vec<Site> = sites
            .iter()
            .copied()
            .filter(|s| {
                flora
                    .view()
                    .establishment_gates(&world.view(), *s, species)
                    .passes()
            })
            .collect();
        assert_eq!(got, want, "{} eligible set", species.name());
    }

    // The substrate gate is doing work: glowcap passes at the log's mycelium box and not
    // away from it, so its set is non-empty and smaller than the whole skyline.
    let glowcap: Vec<Site> = sites
        .iter()
        .copied()
        .zip(cached(&world, &flora, &sites, Species::Glowcap, &mut sky))
        .filter(|(_, g)| g.passes())
        .map(|(s, _)| s)
        .collect();
    assert!(!glowcap.is_empty(), "the log must open the substrate gate");
    assert!(glowcap.contains(&sites[5]));
    assert!(glowcap.len() < sites.len(), "glowcap is shut out elsewhere");
}
