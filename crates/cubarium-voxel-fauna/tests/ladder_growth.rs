//! **Package L: animal bodies at the ladder, and what the browser's mouth reaches on the
//! new plants** (`design/handoffs/voxel-ladder-growth-2026-09-23.md` §3 and §5 items
//! 4–5). Written before the rule, by a separate pass, from the brief.
//!
//! ## The interface these tests read
//!
//! All of it exists today; nothing new is assumed.
//!
//! - `FaunaConfig::default().founder(f).adult_body()` → [`Body`] with `length_m`,
//!   `width_m`, `height_m`, `mouth_ceiling_m`, and `Body::mouth_layers(standing_y,
//!   voxel_m)`, the crown layers the mouth band selects (the live mouth's own rule).
//! - `FloraConfig::for_voxel_size(v)`, `Command::Seed`, and `FloraView::layers_at(site)`,
//!   whose [`StandLayer`]s carry `band_m` (metres above the world floor, `site.y · v` at
//!   a `0.0` band) and `cell` (the one cell layer the layer's disc occupies).
//!
//! "In band" is asked two ways. In **metres**, relative to the plant's own base, as the
//! brief states it (rosette 0–0.15 m under a 0.5 m ceiling). And by **cell**, which is
//! what the mouth actually does: a layer is reachable when its `cell` is in
//! `mouth_layers(site.y, v)`. The umbrellafrond claims are cell claims only: its adult
//! lowest tier `[0.4, 0.55]` of a 1.2 m frond starts at 0.48 m, under the 0.49875 m
//! ceiling in metres, but its disc sits at `round(0.55 · h)` — above the band on both
//! grids — and the disc is what a mouth can bite.
//!
//! ## Status before the implementation
//!
//! Compiles today. All four tests **fail at run time** (only the 1.2 m umbrellafrond
//! half would pass on its own): the bodies are still 0.375 × 0.1875 × 0.1875 m and 0.19 × 0.0625 × 0.0625 m, the
//! browser's ceiling is 0.249 m, and a bloomcrown adult is 0.75 m tall.
//!
//! No world is stepped. Nothing pins a hash.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command as FloraCommand, Flora, FloraConfig, Site, Species as Plant, StandLayer,
};

use cubarium_voxel_fauna::{Body, FaunaConfig, Founder};

/// The two shipped voxel sizes.
const GRIDS: [f64; 2] = [0.125, 0.25];

const SUPPORT: u32 = 2;

/// The adult browser's mouth ceiling: `1.33 × 0.375 m = 0.49875 m`, "0.5 m" in the
/// brief.
const BROWSER_CEILING_M: f64 = 0.5;
const CEILING_TOL_M: f64 = 0.005;

fn adult(founder: Founder) -> Body {
    FaunaConfig::default().founder(founder).adult_body()
}

/// A flat plain tall enough for a 2 m crown on either grid: soil in `1..=SUPPORT`.
fn plain(voxel_m: f64) -> World {
    let height = (3.0 / voxel_m).ceil() as u32 + SUPPORT;
    let mut w = World::empty(VoxelConfig {
        width: 4,
        height,
        depth: 1,
        voxel_m,
        seed: 29,
        ..VoxelConfig::default()
    });
    for x in 0..4 {
        for y in 1..=SUPPORT {
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

const SITE: Site = Site {
    x: 1,
    y: SUPPORT,
    z: 0,
};

/// One stand's foliage layers, bottom-up, on the shipped config for `voxel_m`.
fn foliage_layers(voxel_m: f64, species: Plant, wood: f64) -> Vec<StandLayer> {
    let world = plain(voxel_m);
    let mut flora = Flora::new(FloraConfig::for_voxel_size(voxel_m));
    assert!(
        flora.apply(
            &world,
            FloraCommand::Seed {
                x: i64::from(SITE.x),
                z: SITE.z,
                species,
                wood,
            },
        ),
        "{species:?} at wood {wood} refused"
    );
    let layers: Vec<StandLayer> = flora.view().layers_at(SITE).collect();
    assert!(!layers.is_empty(), "{species:?}: a stand has foliage");
    layers
}

/// A layer's band in metres above the plant's own base.
fn rel_band_m(layer: &StandLayer, voxel_m: f64) -> [f64; 2] {
    let base = f64::from(SITE.y) * voxel_m;
    [layer.band_m[0] - base, layer.band_m[1] - base]
}

/// The stand's crown height in metres: the top of its highest layer over its base. Every
/// authored stage has a foliage layer whose band ends at 1.0.
fn crown_height_m(voxel_m: f64, species: Plant, wood: f64) -> f64 {
    foliage_layers(voxel_m, species, wood)
        .iter()
        .map(|l| rel_band_m(l, voxel_m)[1])
        .fold(0.0, f64::max)
}

/// The wood at which `species` stands `target_m` tall on `voxel_m`, by bisection over
/// `[alive_min, wood_max]` (crown height rises with wood past the seedling).
fn wood_for_height(voxel_m: f64, species: Plant, target_m: f64) -> f64 {
    let sc = FloraConfig::default().species(species).clone();
    let (mut lo, mut hi) = (sc.alive_min, sc.wood_max);
    assert!(
        crown_height_m(voxel_m, species, hi) >= target_m,
        "{species:?} never reaches {target_m} m"
    );
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if crown_height_m(voxel_m, species, mid) < target_m {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let got = crown_height_m(voxel_m, species, hi);
    assert!(
        (got - target_m).abs() < 1e-3,
        "{species:?}: no wood gives {target_m} m (nearest {got} m)"
    );
    hi
}

// ------------------------------------------------------------------------ §3

/// Brief §3: the founders' adult bodies are at the ladder.
#[test]
fn adult_bodies_are_at_the_ladder() {
    for (founder, [l, w, h]) in [
        (Founder::Browser, [0.75, 0.375, 0.375]),
        (Founder::Blind, [0.375, 0.125, 0.125]),
    ] {
        let b = adult(founder);
        assert!(
            (b.length_m - l).abs() < 1e-12
                && (b.width_m - w).abs() < 1e-12
                && (b.height_m - h).abs() < 1e-12,
            "{founder:?}: {} × {} × {} m, want {l} × {w} × {h}",
            b.length_m,
            b.width_m,
            b.height_m
        );
    }
}

// ------------------------------------------------------------------------ §5 item 4

/// Brief §5.4, first half: the browser's adult mouth ceiling is 0.5 m
/// (`1.33 × 0.375 = 0.49875`), and on both grids its band selects the cells of the first
/// 0.5 m of air over the standing face: two 0.25 m cells, four 0.125 m cells.
#[test]
fn the_browser_adult_mouth_ceiling_is_half_a_metre() {
    let b = adult(Founder::Browser);
    assert!(
        (b.mouth_ceiling_m - BROWSER_CEILING_M).abs() < CEILING_TOL_M,
        "adult browser mouth ceiling {} m, want ≈ 0.5 m",
        b.mouth_ceiling_m
    );
    let lo = i64::from(SUPPORT) + 1;
    assert_eq!(b.mouth_layers(SUPPORT, 0.25), lo..=lo + 1);
    assert_eq!(b.mouth_layers(SUPPORT, 0.125), lo..=lo + 3);
}

/// Brief §5.4, second half: a full-grown bloomcrown is 1 m tall; its crown layer
/// (0.55–1.0 m) is out of the browser's band and its rosette (0–0.15 m) is in it — in
/// metres and by the cell the mouth asks about, on both grids.
#[test]
fn a_bloomcrown_adult_crown_is_out_of_band_and_its_rosette_in() {
    let b = adult(Founder::Browser);
    let wood = FloraConfig::default().species(Plant::Bloomcrown).wood_max;
    for v in GRIDS {
        let layers = foliage_layers(v, Plant::Bloomcrown, wood);
        let rosette = layers.first().unwrap();
        let crown = layers.last().unwrap();
        let (r, c) = (rel_band_m(rosette, v), rel_band_m(crown, v));
        assert!(
            (r[0] - 0.0).abs() < 1e-9 && (r[1] - 0.15).abs() < 1e-9,
            "{v} m: rosette band {r:?} m, want [0, 0.15]"
        );
        assert!(
            (c[0] - 0.55).abs() < 1e-9 && (c[1] - 1.0).abs() < 1e-9,
            "{v} m: crown band {c:?} m, want [0.55, 1.0]"
        );
        // Metres.
        assert!(
            r[1] <= b.mouth_ceiling_m,
            "{v} m: rosette top over the ceiling"
        );
        assert!(
            c[0] >= b.mouth_ceiling_m,
            "{v} m: crown bottom under the ceiling"
        );
        // Cells.
        let band = b.mouth_layers(SITE.y, v);
        assert!(
            band.contains(&rosette.cell),
            "{v} m: rosette cell {} not in mouth band {band:?}",
            rosette.cell
        );
        assert!(
            !band.contains(&crown.cell),
            "{v} m: crown cell {} in mouth band {band:?}",
            crown.cell
        );
    }
}

// ------------------------------------------------------------------------ §5 item 5

/// Brief §5.5 and §2's accepted consequence: an umbrellafrond 0.8 m tall still has its
/// lowest foliage tier in the browser's band; at 1.2 m it does not. On both grids, by
/// the cell the mouth asks about.
#[test]
fn an_umbrellafrond_lowest_tier_is_in_band_at_0_8_m_and_out_at_1_2_m() {
    let b = adult(Founder::Browser);
    for v in GRIDS {
        let band = b.mouth_layers(SITE.y, v);
        for (height_m, in_band) in [(0.8, true), (1.2, false)] {
            let wood = wood_for_height(v, Plant::Umbrellafrond, height_m);
            let layers = foliage_layers(v, Plant::Umbrellafrond, wood);
            let lowest = layers.first().unwrap();
            assert_eq!(
                band.contains(&lowest.cell),
                in_band,
                "{v} m grid, {height_m} m frond: lowest tier {:?} m (cell {}), mouth band {band:?}",
                rel_band_m(lowest, v),
                lowest.cell
            );
            if in_band {
                assert!(
                    rel_band_m(lowest, v)[0] < b.mouth_ceiling_m,
                    "{v} m grid, {height_m} m frond: lowest tier starts over the ceiling"
                );
            }
        }
    }
}
