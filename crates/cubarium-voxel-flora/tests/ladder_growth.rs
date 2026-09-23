//! **Package L: plant crowns in metres, at the ladder, and the growth fix**
//! (`design/handoffs/voxel-ladder-growth-2026-09-23.md` §1, §2 and §5 items 1–3).
//! Written before the rule, by a separate pass, from the brief.
//!
//! ## The interface these tests read
//!
//! Nothing here names the crown fields. Every claim is read through the **resolved
//! geometry** — [`cubarium_voxel_flora::FloraView::profile_layers`] (that is,
//! `layers_of`), the one place a stand's layers get their bands and radii in metres, and
//! what the shade model, the mouth, the cone and the presenter all read. So the
//! implementer may name the metre-valued fields as they like (for instance
//! `crown_height_m: [f64; 2]` / `crown_radius_m: [f64; 2]` on `SpeciesConfig`), provided
//! `layers_of` is driven by them. What the tests assume:
//!
//! - `FloraConfig::for_voxel_size(v)` records `voxel_m = v` and, after package L, no
//!   longer rescales crown geometry (rooting, hop and substrate reach may still scale).
//! - `Command::Seed { x, z, species, wood }` plants a stand of exactly `wood`, refused
//!   below `alive_min` (today's behaviour).
//! - `StandLayer::band_m` is metres above the world floor, `site.y · voxel_m` at the
//!   bottom of a `0.0` band; `StandLayer::radius_m` is `layer.radius ×` the crown radius
//!   in metres (today's behaviour).
//! - `SpeciesConfig::profile_at(wood)` / `profile_index(wood)` / `profile[0]` with
//!   `wood_fraction_max` and `height_m_max` (today's).
//!
//! The crown **height** of a stand is recovered as `(top of the layer whose band reaches
//! highest − site.y · voxel_m) / that band's top fraction`, and the crown **radius** as
//! `radius_m / radius fraction` of the widest layer — both exact inverses of `layers_of`.
//!
//! ## Status before the implementation
//!
//! This file **compiles today** against the current API. The grid-invariance test
//! passes today (the per-grid rescale); the other three **fail at run time**: crowns
//! are still in 0.25 m voxels, rescaled per voxel size, at the old sizes; the seedling
//! caps height only; and growth after the seedling runs from the range's minimum.
//!
//! No world here is stepped; each test seeds a handful of stands and reads them.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Site, Species};

/// The two shipped voxel sizes: `small` / `desktop` (0.125 m) and `default` (0.25 m).
const GRIDS: [f64; 2] = [0.125, 0.25];

/// The seedling ceiling on both height and radius, metres (brief §2).
const SEEDLING_M: f64 = 0.125;

/// Brief §1's ladder: `(species, height m [min, max], radius m [min, max])`.
const LADDER: [(Species, [f64; 2], [f64; 2]); 6] = [
    (Species::Springturf, [0.125, 0.1875], [0.125, 0.25]),
    (Species::Velvetpad, [0.125, 0.125], [0.1875, 0.375]),
    (Species::Stonecushion, [0.125, 0.1875], [0.125, 0.15625]),
    (Species::Glowcap, [0.125, 0.25], [0.0625, 0.0625]),
    (Species::Bloomcrown, [0.375, 1.0], [0.125, 0.3125]),
    (Species::Umbrellafrond, [1.0, 2.0], [0.375, 0.75]),
];

const SUPPORT: u32 = 2;

/// A flat plain tall enough for a 2 m crown on either grid: soil in `1..=SUPPORT`.
fn plain(voxel_m: f64) -> World {
    let height = ((3.0 / voxel_m).ceil() as u32).max(8) + SUPPORT;
    let mut w = World::empty(VoxelConfig {
        width: 4,
        height,
        depth: 1,
        voxel_m,
        seed: 23,
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

/// One stand's physical crown, read back out of its resolved layers.
#[derive(Debug, Clone, Copy)]
struct Crown {
    height_m: f64,
    radius_m: f64,
    stage: usize,
}

/// Seed one stand of `species` at `wood` on the `voxel_m` plain, with the shipped config
/// for that grid, and read its crown in metres.
fn crown(voxel_m: f64, species: Species, wood: f64) -> Crown {
    let world = plain(voxel_m);
    let config = FloraConfig::for_voxel_size(voxel_m);
    assert_eq!(config.voxel_m, voxel_m);
    let sc = config.species(species).clone();
    let mut flora = Flora::new(config);
    assert!(
        flora.apply(
            &world,
            Command::Seed {
                x: 1,
                z: 0,
                species,
                wood,
            },
        ),
        "{species:?} at wood {wood} refused"
    );
    let site = Site {
        x: 1,
        y: SUPPORT,
        z: 0,
    };
    let view = flora.view();
    let stand = view.stand_at(site).expect("the seeded stand");
    let resolved = view.profile_layers(stand);
    let stage = sc.profile_at(wood);
    assert_eq!(
        resolved.len(),
        stage.layers.len(),
        "{species:?}: every layer resolved"
    );

    let base_m = f64::from(site.y) * voxel_m;
    let (top_i, top) = stage
        .layers
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.band[1].total_cmp(&b.1.band[1]))
        .expect("a stage has layers");
    assert!(top.band[1] > 0.0);
    let height_m = (resolved[top_i].band_m[1] - base_m) / top.band[1];

    let (wide_i, wide) = stage
        .layers
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.radius.total_cmp(&b.1.radius))
        .expect("a stage has layers");
    assert!(wide.radius > 0.0);
    let radius_m = resolved[wide_i].radius_m / wide.radius;

    Crown {
        height_m,
        radius_m,
        stage: sc.profile_index(wood),
    }
}

fn wood_max(species: Species) -> f64 {
    FloraConfig::default().species(species).wood_max
}

/// The first stage's wood-fraction threshold, if the species opens with a capped
/// seedling (`height_m_max` set).
fn capped_seedling(species: Species) -> Option<f64> {
    let config = FloraConfig::default();
    let sc = config.species(species);
    let first = sc.profile.first()?;
    first.height_m_max.map(|_| first.wood_fraction_max)
}

fn close(got: f64, want: f64, tol: f64) -> bool {
    (got - want).abs() <= tol
}

// ------------------------------------------------------------------------ §5 item 1

/// Brief §5.1: the same stand is the **same physical plant** on the 0.125 m and the
/// 0.25 m grid — crown height and radius in metres agree at every wood fraction, for
/// every species. Today's per-grid rescale already makes this hold; the test is the
/// guard that stating crowns in metres (and dropping the rescale) keeps it. The ladder
/// values themselves are the next test's.
#[test]
fn crowns_are_the_same_physical_size_on_both_grids() {
    for species in Species::ALL {
        let sc = FloraConfig::default().species(species).clone();
        for wf in [0.0, 0.05, 0.1, 0.15, 0.2, 0.3, 0.5, 0.75, 1.0] {
            let wood = (wf * sc.wood_max).max(sc.alive_min);
            let fine = crown(0.125, species, wood);
            let coarse = crown(0.25, species, wood);
            assert!(
                close(fine.height_m, coarse.height_m, 1e-9)
                    && close(fine.radius_m, coarse.radius_m, 1e-9),
                "{species:?} wf {wf}: 0.125 m grid {fine:?} vs 0.25 m grid {coarse:?}"
            );
        }
    }
}

/// Brief §1: the crowns are stated in metres **at the ladder**, on both grids. Every
/// species reaches its `[h_max, r_max]` at `wf = 1`; a species without a capped
/// seedling starts at `[h_min, r_min]` and grows linearly between them (§2, last bullet).
#[test]
fn crowns_are_at_the_ladder_in_metres_on_both_grids() {
    for (species, h, r) in LADDER {
        let wmax = wood_max(species);
        for voxel_m in GRIDS {
            let full = crown(voxel_m, species, wmax);
            assert!(
                close(full.height_m, h[1], 1e-9) && close(full.radius_m, r[1], 1e-9),
                "{species:?} at wf 1 on {voxel_m} m: {full:?}, want h {} r {}",
                h[1],
                r[1]
            );
            if capped_seedling(species).is_some() {
                continue;
            }
            let alive = FloraConfig::default().species(species).alive_min;
            for wf in [alive / wmax, 0.5] {
                let c = crown(voxel_m, species, wf * wmax);
                let want_h = h[0] + wf * (h[1] - h[0]);
                let want_r = r[0] + wf * (r[1] - r[0]);
                assert!(
                    close(c.height_m, want_h, 1e-9) && close(c.radius_m, want_r, 1e-9),
                    "{species:?} at wf {wf} on {voxel_m} m: {c:?}, want h {want_h} r {want_r}"
                );
            }
        }
    }
}

// ------------------------------------------------------------------------ §5 item 2

/// Brief §5.2: a capped-seedling species at `wf ≤ w0` is at most 0.125 m tall **and**
/// at most 0.125 m in radius, on both grids. Today only the height is capped: an
/// umbrellafrond seedling is a flat star 0.8 m across.
#[test]
fn a_capped_seedling_is_capped_in_height_and_radius() {
    let capped: Vec<Species> = Species::ALL
        .into_iter()
        .filter(|s| capped_seedling(*s).is_some())
        .collect();
    assert!(capped.contains(&Species::Bloomcrown) && capped.contains(&Species::Umbrellafrond));
    for species in capped {
        let w0 = capped_seedling(species).unwrap();
        let sc = FloraConfig::default().species(species).clone();
        let wmax = sc.wood_max;
        let alive_wf = sc.alive_min / wmax;
        // Just inside the threshold, so the float division cannot tip into stage 1.
        let at_w0 = w0 * (1.0 - 1e-9);
        for wf in [alive_wf, 0.5 * (alive_wf + at_w0), at_w0] {
            for voxel_m in GRIDS {
                let c = crown(voxel_m, species, wf * wmax);
                assert_eq!(c.stage, 0, "{species:?} wf {wf}: still a seedling");
                assert!(
                    c.height_m <= SEEDLING_M + 1e-9 && c.radius_m <= SEEDLING_M + 1e-9,
                    "{species:?} seedling at wf {wf} on {voxel_m} m: {c:?}"
                );
            }
        }
    }
}

// ------------------------------------------------------------------------ §5 item 3

/// Brief §5.3 and §2's formula: just past `w0` a stand is ≈ 0.125 m in height and
/// radius — growth starts from the seedling, not from the range's minimum — halfway
/// from `w0` to 1 it is halfway from 0.125 m to the maximum, and at `wf = 1` it is at
/// `h_max`, `r_max`. On both grids.
#[test]
fn growth_after_the_seedling_starts_from_the_seedling() {
    for (species, h, r) in LADDER {
        let Some(w0) = capped_seedling(species) else {
            continue;
        };
        let wmax = wood_max(species);
        for voxel_m in GRIDS {
            let past = crown(voxel_m, species, w0 * (1.0 + 1e-6) * wmax);
            assert!(
                past.stage >= 1,
                "{species:?}: past w0 is no longer a seedling"
            );
            assert!(
                close(past.height_m, SEEDLING_M, 1e-3) && close(past.radius_m, SEEDLING_M, 1e-3),
                "{species:?} just past w0 = {w0} on {voxel_m} m: {past:?}, want ≈ 0.125 m in both"
            );

            let mid_wf = 0.5 * (w0 + 1.0);
            let mid = crown(voxel_m, species, mid_wf * wmax);
            let want_h = SEEDLING_M + 0.5 * (h[1] - SEEDLING_M);
            let want_r = SEEDLING_M + 0.5 * (r[1] - SEEDLING_M);
            assert!(
                close(mid.height_m, want_h, 1e-9) && close(mid.radius_m, want_r, 1e-9),
                "{species:?} at wf {mid_wf} on {voxel_m} m: {mid:?}, want h {want_h} r {want_r}"
            );

            let full = crown(voxel_m, species, wmax);
            assert!(
                close(full.height_m, h[1], 1e-9) && close(full.radius_m, r[1], 1e-9),
                "{species:?} at wf 1 on {voxel_m} m: {full:?}, want h {} r {}",
                h[1],
                r[1]
            );
        }
    }
}
