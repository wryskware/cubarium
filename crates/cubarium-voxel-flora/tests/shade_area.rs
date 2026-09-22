//! **Canopy shade in square metres** (`design/handoffs/voxel-body-anchors-2026-09-22.md`,
//! deliverable 1; the audit's §1 "shade-area units bug").
//!
//! A crown attenuates the light under it by `exp(-k · P / A)`. `A` used to be the
//! crown's area in **cells²**, so the same physical plant with the same foliage shaded
//! four times less when the cell halved — an optical depth that depends on the grid is
//! not a plant. `A` is now the crown's area in **m²**, floored at one reference cell,
//! `(0.25 m)²`.
//!
//! Two claims, written before the rule:
//!
//! 1. On the 0.25 m reference grid nothing moves. `shade_k_per_m2 = shade_k · (0.25)²`
//!    and `A_m² = A_cells · (0.25)²` — including the floor, which is one reference cell
//!    either way — so the exponent is the old one, digit for digit.
//! 2. Off it, the same physical crown gives the same physical shade: a 0.125 m world and
//!    a 0.25 m world holding the same plant attenuate identically.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Site, Species};

/// The historical canopy coefficient, per **cell²**, as it stood before this package
/// (`FloraConfig::shade_k` = 1.5). Claim 1 is measured against it.
const HISTORICAL_SHADE_K_PER_CELL2: f64 = 1.5;
/// The reference cell the physical floor is one of.
const REFERENCE_VOXEL_M: f64 = 0.25;

/// A one-row plain: bedrock floor, soil in `1..=support` at `pore` of capacity, open sky
/// over every column, so every support face reads full sky.
fn plain(voxel_m: f64, width: u32, height: u32, support: u32, pore: f64) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height,
        depth: 1,
        voxel_m,
        seed: 23,
        ..VoxelConfig::default()
    });
    let cap = Material::Soil.pore_capacity() * w.config().voxel_volume();
    for x in 0..width as i64 {
        for y in 1..=support {
            if pore > 0.0 {
                w.apply(WorldCommand::AddWater {
                    x,
                    y,
                    z: 0,
                    volume_m3: pore * cap,
                });
            }
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

/// `L_eff = L (1 + half) / (L + half)`, the species' own light response.
fn light_response(l: f64, half: f64) -> f64 {
    l * (1.0 + half) / (l + half)
}

/// One tall umbrellafrond at `x = 8` and one short bloomcrown at `x = 9`, inside its
/// crown. Returns `(the shaded stand's light, the shading crown's foliage, its radius in
/// cells, the shaded species' light_half)`.
fn shaded_pair(voxel_m: f64, support: u32) -> (f64, f64, f64, f64) {
    let mut world = plain(voxel_m, 16, 16, support, 0.6);
    let config = FloraConfig::for_voxel_size(voxel_m);
    let tall_wood = 0.6;
    let short_wood = 0.1;
    let uc = config.species(Species::Umbrellafrond).clone();
    let bc = config.species(Species::Bloomcrown).clone();
    assert!(
        uc.crown_height(tall_wood) > bc.crown_height(short_wood),
        "the fixture's premise: the shading crown is the taller one at {voxel_m} m"
    );
    assert!(
        uc.crown_radius(tall_wood) >= 1.0,
        "and it covers its neighbour"
    );

    let mut flora = Flora::new(config);
    for (x, species, wood) in [
        (8i64, Species::Umbrellafrond, tall_wood),
        (9, Species::Bloomcrown, short_wood),
    ] {
        assert!(flora.apply(&world, Command::Seed { x, z: 0, species, wood }));
    }
    flora.step(&mut world);

    let shaded = flora
        .view()
        .stand_at(Site {
            x: 9,
            y: support,
            z: 0,
        })
        .expect("the shaded bloomcrown")
        .light;
    assert_eq!(
        flora
            .view()
            .stand_at(Site {
                x: 8,
                y: support,
                z: 0
            })
            .expect("the shading stand")
            .light,
        1.0,
        "nothing shades the tall stand itself, so the fixture's sky is open"
    );
    (
        shaded,
        uc.alpha * tall_wood,
        uc.crown_radius(tall_wood),
        bc.light_half,
    )
}

/// Claim 1: on the 0.25 m reference grid the attenuation is exactly what the cell²
/// model gave, floor included. The conversion is a unit change and not a tuning.
#[test]
fn the_reference_grid_shade_is_numerically_what_it_always_was() {
    let (light, foliage, radius_cells, half) = shaded_pair(REFERENCE_VOXEL_M, 2);

    // The historical expression, in cells.
    let area_cells = (std::f64::consts::PI * radius_cells * radius_cells).max(1.0);
    let historical = light_response(
        (-HISTORICAL_SHADE_K_PER_CELL2 * foliage / area_cells).exp(),
        half,
    );
    assert!(
        (light - historical).abs() < 1e-12,
        "the reference grid moved: {light} against the historical {historical}"
    );

    // And the shipped coefficient is the historical one converted, not a new number.
    assert!(
        (FloraConfig::default().shade_k_per_m2
            - HISTORICAL_SHADE_K_PER_CELL2 * REFERENCE_VOXEL_M * REFERENCE_VOXEL_M)
            .abs()
            < 1e-15,
        "shade_k_per_m2 is shade_k × (0.25 m)²"
    );
}

/// Claim 2: the same physical crown shades the same on a 0.125 m world and a 0.25 m
/// world. `FloraConfig::for_voxel_size` doubles the crown's radius in cells when the
/// cell halves, so its area in m² — and the optical depth — are unchanged.
#[test]
fn the_same_physical_crown_shades_the_same_on_both_grids() {
    // Support faces chosen so both standing surfaces are at 0.75 m.
    let (fine, foliage_f, r_f, half_f) = shaded_pair(0.125, 5);
    let (coarse, foliage_c, r_c, half_c) = shaded_pair(0.25, 2);

    assert!((foliage_f - foliage_c).abs() < 1e-12, "the same plant");
    assert!(
        (r_f * 0.125 - r_c * 0.25).abs() < 1e-12,
        "the same crown radius in metres: {} against {}",
        r_f * 0.125,
        r_c * 0.25
    );
    assert!((half_f - half_c).abs() < 1e-15);
    assert!(
        (fine - coarse).abs() < 1e-12,
        "a physical crown shaded {fine} on the fine grid and {coarse} on the coarse one"
    );
    assert!(fine < 1.0, "and it did shade something: {fine}");
}
