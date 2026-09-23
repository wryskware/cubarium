//! **Plants have layers** (`design/handoffs/voxel-plant-layers-2026-09-22.md`,
//! deliverable 1). Written before the rule, from the brief and from decisions §4/§5.
//!
//! A stand is no longer a lollipop: its species carries a `Profile` staged by
//! `wood / wood_max`, and the stand holds one stock per foliage-bearing layer of that
//! profile. The claims this file pins, each as a short function test:
//!
//! 1. The per-layer stocks sum to the stand's scalar `foliage` — after a bite, after a
//!    growth interval, across a stage transition and at death — and organic, mineral
//!    and energy are conserved across every one of them.
//! 2. **Persistent lower depletion** (the audit's §5): taking 0.10 from below a
//!    0.25 / 0.75 stand leaves 0.15 / 0.75, never 0.225 / 0.675.
//! 3. Regrowth fills bottom-up, senescence loses from the top.
//! 4. A stage transition re-bins the **existing** total into the new profile bottom-up
//!    and never creates tissue.
//! 5. A single-layer species shades **identically to before** on the 0.25 m reference
//!    grid: the geometry, the area, the occlusion order and the weighting are today's
//!    digit for digit, and the authored porosity is the only new factor.
//!
//! The reach and sight claims live with the consumer that asks them
//! (`cubarium-voxel-fauna`'s `tests/layers.rs`), because reach is a mouth's question.
//!
//! No world here is stepped more than a handful of ticks and nothing is a golden hash.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command, Flora, FloraConfig, LayerKind, Profile, Site, Species, SpeciesConfig,
};

// ------------------------------------------------------------------- fixtures

/// A one-row plain: bedrock floor, soil in `1..=support` at `pore` of capacity, open sky
/// over every column. `shade_area.rs`'s fixture.
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

fn at(x: u32, support: u32) -> Site {
    Site {
        x,
        y: support,
        z: 0,
    }
}

/// The three flora residuals, relative.
fn assert_residuals(flora: &Flora, when: &str) {
    let v = flora.view();
    for (name, got) in [
        ("organic", v.organic() - v.ledger.expected_organic()),
        ("mineral", v.mineral() - v.ledger.expected_mineral()),
        ("energy", v.energy() - v.ledger.expected_energy()),
    ] {
        let scale = match name {
            "organic" => v.organic().abs(),
            "mineral" => v.mineral().abs(),
            _ => v.energy().abs(),
        };
        assert!(
            got.abs() <= 1e-9 * scale.max(1.0),
            "{when}: {name} residual {got}"
        );
    }
}

/// Every stand's layer stocks sum to its `foliage`, and no stock is negative.
fn assert_layers_sum(flora: &Flora, when: &str) {
    let v = flora.view();
    for stand in v.stands {
        let layers: Vec<f64> = v.layers(stand).map(|l| l.stock).collect();
        let sum: f64 = layers.iter().sum();
        assert!(
            (sum - stand.foliage).abs() <= 1e-12 * stand.foliage.abs().max(1.0),
            "{when}: {:?} at {:?} holds {layers:?} = {sum} against foliage {}",
            stand.species,
            stand.site,
            stand.foliage
        );
        assert!(
            layers.iter().all(|s| *s >= 0.0),
            "{when}: a negative layer stock {layers:?}"
        );
    }
}

// -------------------------------------------------------------- 1. the shapes

/// The six live species' profiles are well formed: bands inside `[0, 1]` and rising,
/// radii positive, foliage shares summing to one, stages in increasing threshold order
/// with a last stage that catches every wood fraction.
#[test]
fn the_six_authored_profiles_are_well_formed() {
    let config = FloraConfig::default();
    for species in Species::ALL {
        let sc = config.species(species);
        assert!(
            !sc.profile.is_empty(),
            "{}: no profile at all",
            species.name()
        );
        let mut previous = 0.0f64;
        for (i, stage) in sc.profile.iter().enumerate() {
            assert!(
                stage.wood_fraction_max >= previous,
                "{} stage {i}: threshold {} below the one before",
                species.name(),
                stage.wood_fraction_max
            );
            previous = stage.wood_fraction_max;
            let mut share = 0.0;
            for layer in &stage.layers {
                assert!(
                    layer.band[0] >= 0.0 && layer.band[1] <= 1.0 && layer.band[1] > layer.band[0],
                    "{} stage {i}: band {:?}",
                    species.name(),
                    layer.band
                );
                assert!(layer.radius > 0.0 && layer.radius <= 1.0);
                assert!((0.0..=1.0).contains(&layer.porosity));
                if layer.kind.bears_foliage() {
                    share += layer.share;
                }
            }
            assert!(
                (share - 1.0).abs() < 1e-12,
                "{} stage {i}: foliage shares sum to {share}",
                species.name()
            );
        }
        assert!(
            sc.profile.last().expect("a stage").wood_fraction_max >= 1.0,
            "{}: the last stage must catch a full-grown stand",
            species.name()
        );
    }
}

/// Decisions §5: an adult bloomcrown keeps a basal rosette holding a quarter of its
/// foliage for its whole life, and a woody seedling is a ground rosette no taller than
/// 0.125 m whatever the interpolated crown height says.
#[test]
fn the_decided_corrections_are_in_the_profiles() {
    let config = FloraConfig::default();
    let bc = config.species(Species::Bloomcrown);
    let adult = bc.profile_at(bc.wood_max);
    let basal = adult
        .layers
        .iter()
        .filter(|l| l.kind.bears_foliage())
        .min_by(|a, b| a.band[0].total_cmp(&b.band[0]))
        .expect("a lowest foliage layer");
    assert!(
        (basal.share - 0.25).abs() < 1e-12,
        "the adult rosette holds {} of the foliage",
        basal.share
    );
    assert!(basal.band[0] == 0.0, "and it sits on the ground");

    for species in [Species::Bloomcrown, Species::Umbrellafrond] {
        let sc = config.species(species);
        let seedling = sc.profile_at(0.0);
        assert_eq!(
            seedling.height_m_max,
            Some(0.125),
            "{}: a woody seedling is capped at 0.125 m",
            species.name()
        );
        // And the cap actually binds: the interpolated height is taller than the cap.
        // Since package L the range's own minimum is what the cap trims.
        let free = sc.crown_height_m[0];
        assert!(
            free > 0.125,
            "{}: the cap would be inert ({free} m free)",
            species.name()
        );
    }
}

/// Decisions §3: velvetpad and stonecushion stay browser food, so neither carries a
/// diet flag and both are ordinary foliage layers.
#[test]
fn the_pad_and_the_cushion_are_still_foliage() {
    let config = FloraConfig::default();
    for species in [Species::Velvetpad, Species::Stonecushion] {
        let sc = config.species(species);
        let adult = sc.profile_at(sc.wood_max);
        assert!(
            adult
                .layers
                .iter()
                .any(|l| l.kind.bears_foliage() && l.share > 0.0),
            "{} has no foliage-bearing layer",
            species.name()
        );
    }
}

// ------------------------------------------------ 2. persistent lower depletion

/// The audit's §5 example, exactly: a stand holding 0.25 below and 0.75 above loses
/// 0.10 from below and is left at 0.15 / 0.75. A pro-rata reconstruction of the
/// remaining 0.90 would give 0.225 / 0.675 and move 0.075 of crown into reachable
/// tissue with no growth; that is the state no scalar total can tell apart.
#[test]
fn a_bite_from_below_leaves_the_upper_stock_untouched() {
    let support = 2;
    let mut world = plain(0.25, 8, 10, support, 0.6);
    let mut config = FloraConfig::default();
    // A two-layer species whose shares are exactly the audit's, so the numbers in the
    // assertion are the numbers in the document.
    config.species_mut(Species::Bloomcrown).profile = vec![Profile {
        wood_fraction_max: f64::INFINITY,
        height_m_max: None,
        layers: vec![
            SpeciesConfig::foliage_layer([0.0, 0.5], 1.0, 0.25, 0.0),
            SpeciesConfig::foliage_layer([0.5, 1.0], 1.0, 0.75, 0.0),
        ],
    }];
    let alpha = config.species(Species::Bloomcrown).alpha;
    let mut flora = Flora::new(config);
    // `Seed` fills a stand to `alpha · wood` of foliage, so a wood of `1 / alpha` is a
    // stand holding exactly one unit and the split is 0.25 / 0.75.
    let wood = 1.0 / alpha;
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 4,
            z: 0,
            species: Species::Bloomcrown,
            wood,
        }
    ));
    let site = at(4, support);
    let before: Vec<f64> = flora.view().layers_at(site).map(|l| l.stock).collect();
    assert_eq!(before.len(), 2);
    assert!(
        (before[0] - 0.25).abs() < 1e-12 && (before[1] - 0.75).abs() < 1e-12,
        "{before:?}"
    );

    // The lowest layer only: the cell range of the mouth that can reach it.
    let lowest = flora.view().layers_at(site).next().expect("a layer").cell;
    let taken = flora
        .take_foliage_in_layers(site, 0.10, &(lowest..=lowest))
        .expect("a bite from below");
    assert!((taken.taken.organic - 0.10).abs() < 1e-12, "{taken:?}");
    assert!((taken.per_layer[0] - 0.10).abs() < 1e-12, "{taken:?}");
    assert_eq!(taken.per_layer[1], 0.0, "the crown gave nothing");

    let after: Vec<f64> = flora.view().layers_at(site).map(|l| l.stock).collect();
    assert!(
        (after[0] - 0.15).abs() < 1e-12 && (after[1] - 0.75).abs() < 1e-12,
        "0.25/0.75 minus 0.10 from below is 0.15/0.75, not {after:?}"
    );
    assert_layers_sum(&flora, "after a bite from below");
    assert_residuals(&flora, "after a bite from below");
    let _ = &mut world;
}

/// A bite the lowest layer cannot pay for spills **upwards only as far as the mouth
/// reaches**: a band that only touches the rosette stops at the rosette, and the
/// withdrawal reports the shortfall by returning less than was wanted.
#[test]
fn a_bite_is_bounded_by_the_layers_the_mouth_reaches() {
    let support = 2;
    let world = plain(0.25, 8, 10, support, 0.6);
    let mut config = FloraConfig::default();
    config.species_mut(Species::Bloomcrown).profile = vec![Profile {
        wood_fraction_max: f64::INFINITY,
        height_m_max: None,
        layers: vec![
            SpeciesConfig::foliage_layer([0.0, 0.34], 1.0, 0.25, 0.0),
            SpeciesConfig::foliage_layer([0.34, 1.0], 1.0, 0.75, 0.0),
        ],
    }];
    // The claim is the layer bound, not the grazing refuge: no floor here.
    config.species_mut(Species::Bloomcrown).graze_refuge = 0.0;
    let alpha = config.species(Species::Bloomcrown).alpha;
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 4,
            z: 0,
            species: Species::Bloomcrown,
            wood: 1.0 / alpha,
        }
    ));
    let site = at(4, support);
    let cells: Vec<i64> = flora.view().layers_at(site).map(|l| l.cell).collect();
    assert!(
        cells[1] > cells[0],
        "the fixture needs two distinct discs: {cells:?}"
    );

    let taken = flora
        .take_foliage_in_layers(site, 1.0, &(cells[0]..=cells[0]))
        .expect("the rosette");
    assert!(
        (taken.taken.organic - 0.25).abs() < 1e-12,
        "a mouth at the rosette got {taken:?}"
    );
    let after: Vec<f64> = flora.view().layers_at(site).map(|l| l.stock).collect();
    assert!(
        after[0] == 0.0 && (after[1] - 0.75).abs() < 1e-12,
        "{after:?}"
    );
    assert_layers_sum(&flora, "after a stripped rosette");
    assert_residuals(&flora, "after a stripped rosette");
}

// ----------------------------------------------------- 3. regrowth and losses

/// Regrowth fills the layers bottom-up to each layer's capacity, so a browsed plant
/// refills its floor tissue before its crown; senescence takes from the top.
#[test]
fn regrowth_fills_from_the_bottom_and_senescence_loses_from_the_top() {
    let support = 2;
    let mut world = plain(0.25, 8, 10, support, 0.6);
    let mut config = FloraConfig::default();
    // The rosette is stripped to nothing here, which predates the grazing refuge.
    config.species_mut(Species::Bloomcrown).graze_refuge = 0.0;
    let alpha = config.species(Species::Bloomcrown).alpha;
    let wood_max = config.species(Species::Bloomcrown).wood_max;
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 4,
            z: 0,
            species: Species::Bloomcrown,
            wood: wood_max,
        }
    ));
    let site = at(4, support);
    let cells: Vec<i64> = flora.view().layers_at(site).map(|l| l.cell).collect();
    let lowest = cells[0];

    // Strip the rosette and step: the income refills the bottom layer first.
    flora
        .take_foliage_in_layers(site, 1.0, &(lowest..=lowest))
        .expect("the rosette");
    let stripped: Vec<f64> = flora.view().layers_at(site).map(|l| l.stock).collect();
    assert_eq!(stripped[0], 0.0);
    let crown_before = *stripped.last().expect("a crown");

    for _ in 0..20 {
        flora.step(&mut world);
    }
    let after: Vec<f64> = flora.view().layers_at(site).map(|l| l.stock).collect();
    assert!(
        after[0] > 0.0,
        "twenty ticks put nothing back in the rosette: {after:?}"
    );
    // The crown only ever lost tissue over those ticks (senescence from the top); it
    // never gained while the rosette was below its capacity.
    assert!(
        *after.last().expect("a crown") <= crown_before + 1e-15,
        "the crown grew while the floor was empty: {after:?} against {stripped:?}"
    );
    assert_layers_sum(&flora, "after twenty ticks of regrowth");
    assert_residuals(&flora, "after twenty ticks of regrowth");
    let _ = alpha;
}

/// Capacity per layer is the layer's `share` of the growth model's own foliage cap,
/// `alpha · W`, and a full stand's layers are each at their own capacity.
#[test]
fn a_full_stand_holds_each_layers_share_of_alpha_w() {
    let support = 2;
    let world = plain(0.25, 8, 10, support, 0.6);
    let config = FloraConfig::default();
    let sc = config.species(Species::Umbrellafrond).clone();
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 4,
            z: 0,
            species: Species::Umbrellafrond,
            wood: sc.wood_max,
        }
    ));
    let site = at(4, support);
    let cap = sc.alpha * sc.wood_max;
    for layer in flora.view().layers_at(site) {
        assert!(
            (layer.stock - layer.share * cap).abs() < 1e-12,
            "layer {} holds {} against its capacity {}",
            layer.index,
            layer.stock,
            layer.share * cap
        );
        assert!((layer.capacity - layer.share * cap).abs() < 1e-12);
    }
}

// ---------------------------------------------------- 4. stage transitions

/// A stage transition re-bins the **existing** total bottom-up into the new profile:
/// the sum is unchanged, no tissue is created, and the floor fills before the crown.
///
/// The fixture grows a seedling bloomcrown over its `wood / wood_max = 0.2` threshold
/// inside 200 ticks, which the placeholder rates cannot do, so it sets its **own**
/// income and wood rates and says so: what is being tested is the re-binning, not how
/// long a seedling takes to make a stem.
#[test]
fn a_stage_transition_rebins_the_total_and_creates_nothing() {
    let support = 2;
    let mut world = plain(0.25, 8, 10, support, 0.6);
    let mut config = FloraConfig::default();
    {
        let sc = config.species_mut(Species::Bloomcrown);
        sc.assimilation *= 400.0;
        sc.wood_rate *= 400.0;
        sc.nutrient_draw_max *= 400.0;
        sc.foliage_rate *= 400.0;
    }
    let sc = config.species(Species::Bloomcrown).clone();
    let seedling_wood = 0.19 * sc.wood_max;
    assert_eq!(
        sc.profile_index(seedling_wood),
        0,
        "the fixture starts a seedling"
    );
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 4,
            z: 0,
            species: Species::Bloomcrown,
            wood: seedling_wood,
        }
    ));
    let site = at(4, support);
    assert_eq!(
        flora.view().layers_at(site).count(),
        1,
        "a seedling bloomcrown is one rosette"
    );
    let before_total: f64 = flora.view().layers_at(site).map(|l| l.stock).sum();

    let mut crossed = None;
    for tick in 1..=200 {
        flora.step(&mut world);
        let stand = *flora.view().stand_at(site).expect("alive");
        if sc.profile_index(stand.wood) != 0 {
            crossed = Some(tick);
            break;
        }
        assert_layers_sum(&flora, &format!("tick {tick}"));
    }
    let tick = crossed.expect("the fixture must cross the seedling threshold in 200 ticks");
    let stand = *flora.view().stand_at(site).expect("alive");
    let after: Vec<f64> = flora.view().layers_at(site).map(|l| l.stock).collect();
    let caps: Vec<f64> = flora.view().layers_at(site).map(|l| l.capacity).collect();
    assert_eq!(
        after.len(),
        2,
        "the juvenile profile has a rosette and a crown"
    );

    let sum: f64 = after.iter().sum();
    assert!(
        (sum - stand.foliage).abs() <= 1e-12 * stand.foliage.max(1.0),
        "tick {tick}: the re-binned layers {after:?} sum to {sum}, not {}",
        stand.foliage
    );
    // Nothing was created: the total is what the ticks' own income and losses made of
    // it, and the re-bin itself moved no mass — the ledger says so.
    assert!(
        stand.foliage > 0.0 && stand.foliage.is_finite(),
        "{before_total} -> {}",
        stand.foliage
    );
    // Bottom-up: the rosette is at its capacity before the crown holds anything.
    if after[1] > 0.0 {
        assert!(
            after[0] >= caps[0] - 1e-12,
            "the crown holds {} while the rosette is {} of a capacity {}",
            after[1],
            after[0],
            caps[0]
        );
    }
    assert_layers_sum(&flora, "after a stage transition");
    assert_residuals(&flora, "after a stage transition");
}

/// Death deposits the sum of the layers, as it always deposited the scalar: the litter
/// a dead stand leaves is its whole foliage and the ledger closes.
#[test]
fn death_deposits_the_sum_of_the_layers() {
    let support = 2;
    let mut world = plain(0.25, 8, 10, support, 0.6);
    let mut config = FloraConfig::default();
    // Kill it inside one tick: `round5a.rs`'s maintenance trick.
    config.species_mut(Species::Bloomcrown).maintenance = 1000.0;
    let sc = config.species(Species::Bloomcrown).clone();
    let mut flora = Flora::new(config);
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 4,
            z: 0,
            species: Species::Bloomcrown,
            wood: sc.wood_max,
        }
    ));
    let site = at(4, support);
    let foliage: f64 = flora.view().layers_at(site).map(|l| l.stock).sum();
    assert!(foliage > 0.0);

    flora.step(&mut world);
    assert!(
        flora.view().stand_at(site).is_none(),
        "the fixture must kill the stand in one tick"
    );
    let litter = flora.view().ground_at(site).expect("a ground slot").litter;
    assert!(
        litter >= foliage - 1e-9,
        "the stand held {foliage} of foliage and left {litter} of litter"
    );
    assert_residuals(&flora, "after a death");
}

// ------------------------------------------------------------ 5. light identity

/// `L_eff = L (1 + half) / (L + half)`, the species' own light response.
fn light_response(l: f64, half: f64) -> f64 {
    l * (1.0 + half) / (l + half)
}

/// A single-layer species shades **exactly** as it did before layers existed on the
/// 0.25 m reference grid.
///
/// The claim is about everything the layer model could have moved: the receiver's
/// reference height (the band's top, which for a `0..1.0` layer is the crown top the
/// old model compared), the occluder test (strictly higher, footprint by the same disc
/// rule), the area (`π (r · v)²` floored at one reference cell), and the weighting
/// (one layer holding the whole of `P`). Only the authored **porosity** is new, and it
/// enters as one factor `(1 - p)` in the exponent, so this test asserts both forms:
/// with `p = 0` the number is today's digit for digit, and with the authored `p` it is
/// today's raised to `1 - p` and nothing else.
#[test]
fn a_single_layer_species_shades_exactly_as_it_did_before() {
    let support = 2;
    // Package L's ladder: velvetpad (0.125 m) is now the lowest full-grown crown and
    // glowcap (0.25 m) one of the taller small ones, too narrow (0.0625 m) to cover a
    // neighbour, so the one single-layer pair left is springturf (0.1875 m, one cell
    // wide) over a velvetpad.
    for (shader, shaded) in [(Species::Springturf, Species::Velvetpad)] {
        for porosity in [0.0, -1.0] {
            let mut config = FloraConfig::default();
            let shader_sc = config.species(shader).clone();
            assert_eq!(
                shader_sc
                    .profile_at(shader_sc.wood_max)
                    .layers
                    .iter()
                    .filter(|l| l.kind.bears_foliage())
                    .count(),
                1,
                "{} must be a single-layer species for this test",
                shader.name()
            );
            // `porosity < 0` means "leave the authored value alone".
            let p = if porosity < 0.0 {
                shader_sc.profile_at(shader_sc.wood_max).layers[0].porosity
            } else {
                for stage in &mut config.species_mut(shader).profile {
                    for layer in &mut stage.layers {
                        layer.porosity = 0.0;
                    }
                }
                0.0
            };

            let mut world = plain(0.25, 16, 12, support, 0.6);
            let shaded_sc = config.species(shaded).clone();
            let shader_wood = shader_sc.wood_max;
            let shaded_wood = shaded_sc.wood_max;
            assert!(
                shader_sc.crown_height(shader_wood, 0.25) > shaded_sc.crown_height(shaded_wood, 0.25),
                "{} must stand over {}",
                shader.name(),
                shaded.name()
            );
            assert!(shader_sc.crown_radius(shader_wood, 0.25) >= 1.0, "and cover it");

            let mut flora = Flora::new(config);
            for (x, species, wood) in [(8i64, shader, shader_wood), (9, shaded, shaded_wood)] {
                assert!(flora.apply(
                    &world,
                    Command::Seed {
                        x,
                        z: 0,
                        species,
                        wood
                    }
                ));
            }
            flora.step(&mut world);

            let light = flora
                .view()
                .stand_at(at(9, support))
                .expect("the shaded stand")
                .light;
            // The pre-layer expression, verbatim: one disc holding the whole of `P`,
            // over `π (r · v)²` floored at one reference cell.
            let r_m = shader_sc.crown_radius(shader_wood, 0.25) * 0.25;
            let area = (std::f64::consts::PI * r_m * r_m).max(0.25 * 0.25);
            let foliage = shader_sc.alpha * shader_wood;
            let historical = (-FloraConfig::default().shade_k_per_m2 * foliage / area).exp();
            let expected = light_response(historical.powf(1.0 - p), shaded_sc.light_half);
            assert!(
                (light - expected).abs() < 1e-12,
                "{} under {} (porosity {p}): {light} against {expected}",
                shaded.name(),
                shader.name()
            );
            if p == 0.0 {
                assert!(light < 1.0, "the fixture must actually shade something");
            }
        }
    }
}

/// A stand's income is assessed **per foliage layer**, weighted by that layer's share
/// of the stand's stock: a two-layer receiver whose lower layer stands in shade and
/// whose upper layer stands in light earns between the two, and moving stock between
/// its layers moves its income even though its total foliage never changes.
#[test]
fn a_layered_receiver_is_weighted_by_where_its_tissue_is() {
    let support = 2;
    let mut config = FloraConfig::default();
    // A receiver with one layer at the floor and one at the top, no porosity, and a
    // shader that covers only the floor one.
    config.species_mut(Species::Bloomcrown).profile = vec![Profile {
        wood_fraction_max: f64::INFINITY,
        height_m_max: None,
        layers: vec![
            SpeciesConfig::foliage_layer([0.0, 0.2], 1.0, 0.5, 0.0),
            SpeciesConfig::foliage_layer([0.8, 1.0], 1.0, 0.5, 0.0),
        ],
    }];
    let bc = config.species(Species::Bloomcrown).clone();
    let uc = config.species(Species::Umbrellafrond).clone();
    let mut world = plain(0.25, 16, 16, support, 0.6);
    let mut flora = Flora::new(config);
    // The frond's lowest tier stands above the bloomcrown's floor layer and below its
    // crown, so exactly one of the receiver's two layers is shaded.
    for (x, species, wood) in [
        (8i64, Species::Umbrellafrond, uc.wood_max),
        (9, Species::Bloomcrown, bc.wood_max * 0.5),
    ] {
        assert!(flora.apply(
            &world,
            Command::Seed {
                x,
                z: 0,
                species,
                wood
            }
        ));
    }
    flora.step(&mut world);
    let both = flora
        .view()
        .stand_at(at(9, support))
        .expect("the receiver")
        .light;
    assert!(
        both > 0.0 && both < 1.0,
        "the fixture must shade partly: {both}"
    );

    // Strip the receiver's lower layer: all of its tissue is now in the lit layer, so
    // its income rises, with the same total foliage capacity and the same geometry.
    let lowest = flora
        .view()
        .layers_at(at(9, support))
        .next()
        .expect("a layer")
        .cell;
    flora
        .take_foliage_in_layers(at(9, support), 1e9, &(lowest..=lowest))
        .expect("the floor layer");
    flora.step(&mut world);
    let only_top = flora
        .view()
        .stand_at(at(9, support))
        .expect("the receiver")
        .light;
    assert!(
        only_top > both,
        "moving tissue out of the shade did not raise the income: {only_top} against {both}"
    );
}

// ----------------------------------------------------------- 6. the whole tick

/// Two hundred ticks of an ordinary mixed stand: the layers sum to the scalar on every
/// tick and the three ledgers close.
#[test]
fn two_hundred_ticks_keep_the_layers_summing_and_the_ledgers_closed() {
    let support = 2;
    // 32 columns wide since package N: nine species three columns apart need 28.
    let mut world = plain(0.25, 32, 16, support, 0.6);
    let config = FloraConfig::default();
    let woods: Vec<(i64, Species, f64)> = Species::ALL
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let sc = config.species(*s);
            (
                4 + 3 * i as i64,
                *s,
                sc.alive_min + 0.5 * (sc.wood_max - sc.alive_min),
            )
        })
        .collect();
    let mut flora = Flora::new(config);
    for (x, species, wood) in woods {
        assert!(flora.apply(
            &world,
            Command::Seed {
                x,
                z: 0,
                species,
                wood
            }
        ));
    }
    for tick in 0..200 {
        flora.step(&mut world);
        if tick % 25 == 0 {
            assert_layers_sum(&flora, &format!("tick {tick}"));
            assert_residuals(&flora, &format!("tick {tick}"));
        }
    }
    assert_layers_sum(&flora, "tick 200");
    assert_residuals(&flora, "tick 200");
}

/// The kinds are what the anatomy document names, and only `Trunk` bears no foliage.
#[test]
fn only_a_trunk_bears_no_foliage() {
    assert!(!LayerKind::Trunk.bears_foliage());
    for kind in [LayerKind::Foliage, LayerKind::Drape, LayerKind::Mat] {
        assert!(kind.bears_foliage(), "{kind:?}");
    }
}
