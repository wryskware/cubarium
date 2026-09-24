//! Cache study C (`design/handoffs/voxel-cache-and-pinning-2026-09-24.md`): a stand's
//! layers without the vector. `layers_of` built a `Vec<StandLayer>` on every call, and
//! a mouth reads every stand in reach, every body, every tick — the tick's one
//! allocation per stand read. [`layer_iter`] yields the same layers one at a time.
//!
//! The reference below is `layers_of` **as it was** (copied before the change), so the
//! iterator is checked against the vector it replaced, to the bit, for every species
//! across its whole wood range and its stage boundaries, and with uneven layer stocks.

use cubarium_voxel::{Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command, Flora, FloraConfig, MAX_FOLIAGE_LAYERS, MIN_LAYER_AREA_M2, Species, Stand, StandLayer,
    disc_offset, layer_iter, layers_of, trunk_offsets,
};

/// `layers_of` before cache study C, verbatim but for the names.
fn reference(config: &FloraConfig, stand: &Stand, voxel_m: f64) -> Vec<StandLayer> {
    let sc = config.species(stand.species);
    let stage = sc.profile_at(stand.wood);
    let height_v = sc.crown_height(stand.wood, voxel_m);
    let radius_v = sc.crown_radius(stand.wood, voxel_m).max(0.0);
    let cap = sc.alpha * stand.wood.max(0.0);
    let base = f64::from(stand.site.y);
    let mut out = Vec::with_capacity(stage.layers.len());
    let mut foliage_index = 0usize;
    for (index, layer) in stage.layers.iter().enumerate() {
        let is_foliage = layer.kind.bears_foliage();
        let fi = if is_foliage {
            let i = foliage_index;
            foliage_index += 1;
            (i < MAX_FOLIAGE_LAYERS).then_some(i)
        } else {
            None
        };
        if is_foliage && fi.is_none() {
            continue;
        }
        let lo_v = base + layer.band[0] * height_v;
        let hi_v = base + layer.band[1] * height_v;
        let r_v = (layer.radius * radius_v).max(0.0);
        let r_m = r_v * voxel_m;
        let cell = i64::from(stand.site.y) + i64::from(disc_offset(layer.band[1], height_v));
        let cells = if is_foliage {
            (cell, cell)
        } else {
            let (lo, hi) = trunk_offsets(layer.band, height_v);
            (
                i64::from(stand.site.y) + i64::from(lo),
                i64::from(stand.site.y) + i64::from(hi),
            )
        };
        out.push(StandLayer {
            index,
            foliage_index: fi,
            kind: layer.kind,
            band_v: [lo_v, hi_v],
            band_m: [lo_v * voxel_m, hi_v * voxel_m],
            radius_v: r_v,
            radius_m: r_m,
            cell,
            cells,
            share: if is_foliage { layer.share } else { 0.0 },
            capacity: if is_foliage { layer.share * cap } else { 0.0 },
            stock: fi.map_or(0.0, |i| stand.layer_stock[i]),
            floor: if is_foliage {
                sc.graze_refuge * layer.share * cap
            } else {
                0.0
            },
            porosity: layer.porosity,
            area_m2: (std::f64::consts::PI * r_m * r_m).max(MIN_LAYER_AREA_M2),
        });
    }
    out
}

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

/// One stand of each species seeded on a small plain, as the model makes them.
fn seeded(config: FloraConfig) -> Flora {
    let mut world = World::empty(VoxelConfig {
        width: 32,
        height: 12,
        depth: 1,
        voxel_m: config.voxel_m,
        seed: 5,
        ..VoxelConfig::default()
    });
    for x in 0..32 {
        for y in 1..=3 {
            world.apply(cubarium_voxel::Command::SetMaterial {
                x,
                y,
                z: 0,
                material: Material::Soil,
            });
        }
    }
    let mut flora = Flora::new(config);
    for (k, species) in Species::ALL.iter().enumerate() {
        let wood = flora.config().species(*species).wood_max * 0.5;
        flora.apply(
            &world,
            Command::Seed {
                x: 2 + 3 * k as i64,
                z: 0,
                species: *species,
                wood,
            },
        );
    }
    flora
}

#[test]
fn the_iterator_yields_the_vectors_layers_to_the_bit() {
    for voxel_m in [0.25, 0.125] {
        let config = FloraConfig::for_voxel_size(voxel_m);
        let stands = seeded(config.clone()).view().stands.to_vec();
        assert!(!stands.is_empty(), "the plain took some seeds");
        let mut compared = 0;
        for base in &stands {
            let wood_max = config.species(base.species).wood_max;
            // Across the whole range, through every stage boundary, and past both ends.
            for step in 0..=40 {
                let mut stand = *base;
                stand.wood = wood_max * (f64::from(step) / 32.0 - 0.1);
                for (i, s) in stand.layer_stock.iter_mut().enumerate() {
                    *s = 0.1 * (i as f64 + 1.0) + f64::from(step) * 1e-3;
                }
                let want = reference(&config, &stand, voxel_m);
                let lazy: Vec<StandLayer> = layer_iter(&config, &stand, voxel_m).collect();
                let eager = layers_of(&config, &stand, voxel_m);
                assert_eq!(
                    lazy.iter().map(bits).collect::<Vec<_>>(),
                    want.iter().map(bits).collect::<Vec<_>>(),
                    "{:?} at wood {}",
                    stand.species,
                    stand.wood
                );
                assert_eq!(eager, want);
                compared += want.len();
            }
        }
        assert!(compared > 0);
    }
}

#[test]
fn the_views_layers_are_the_foliage_bearing_ones_in_profile_order() {
    let config = FloraConfig::for_voxel_size(0.25);
    let flora = seeded(config.clone());
    let view = flora.view();
    let mut read = 0;
    for stand in view.stands {
        let want: Vec<StandLayer> = reference(&config, stand, config.voxel_m)
            .into_iter()
            .filter(|l| l.kind.bears_foliage())
            .collect();
        let got: Vec<StandLayer> = view.layers(stand).collect();
        assert_eq!(got, want, "{:?}", stand.species);
        let sum: f64 = got.iter().map(|l| l.stock).sum();
        assert!(
            (sum - stand.foliage).abs() <= 1e-12 * stand.foliage.abs().max(1.0),
            "{:?}: the layers still hold the stand's foliage",
            stand.species
        );
        read += got.len();
    }
    assert!(read > 0);
}
