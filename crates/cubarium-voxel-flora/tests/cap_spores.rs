//! **Grazing caps reaches the fungus** (the capgnawer package, 2026-10-05; Astra's caution on
//! proposal §3.1E): a glowcap's caps are its fruiting bodies, so what a grazer eats off them
//! has to show in the fungus's recruitment or growth. Short function tests on hand-built
//! stands, checked against the config each sets; no tuned number is asserted.
//!
//! - **Spores come from the caps**: a donor's spore saving follows its cap tissue
//!   (`SpeciesConfig::propagule_cap_share`); a grazed glowcap saves fewer spores than the
//!   same stand with its caps, in proportion to its fullness, and with the share at 0 the two
//!   save alike (fails on main, where spore saving ignores the caps).
//! - **Regrowing caps costs the mycelium** (already so on main, shown here): new cap tissue
//!   is built out of the income before wood, so on a log that limits the income a grazed
//!   glowcap lays on less mycelium than an ungrazed one.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command, Deposit, DepositKind, FaceLayer, Flora, FloraConfig, Site, Species};

/// A strip of soil columns `0..width` at `y = 1..=2` holding half their pore capacity,
/// 1 m voxels, open sky: every column's support face is `y = 2`.
fn strip(width: u32) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height: 10,
        depth: 1,
        voxel_m: 1.0,
        seed: 5,
        ..VoxelConfig::default()
    });
    for x in 0..i64::from(width) {
        for y in 1..=2 {
            let want = 0.5 * cubarium_voxel::Materials::DEFAULT.pore_capacity(Material::Soil) * w.config().voxel_volume();
            w.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: want });
            w.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
        }
    }
    w
}

/// A glowcap at `wood` on column `x` over a log of `log` dead wood.
fn glowcap_on_log(flora: &mut Flora, world: &World, x: i64, wood: f64, log: f64) -> Site {
    assert!(flora.apply(world, Command::Seed { x, z: 0, species: Species::Glowcap, wood }));
    let site = cubarium_voxel_flora::highest_support(&world.view(), x, 0).expect("a support face");
    let sc = flora.config().species(Species::Glowcap).clone();
    if log > 0.0 {
        assert!(flora.deposit(
            site,
            Deposit { kind: DepositKind::DeadWood, organic: log, mineral: sc.n_tissue * log, energy: sc.energy_density * log }
        ));
    }
    site
}

fn fungus(flora: &Flora, site: Site) -> cubarium_voxel_flora::Stand {
    *flora.view().fungus_at(site).expect("the glowcap lives")
}

/// Graze `site`'s caps down as far as a mouth can take them (every layer to its floor).
fn graze(flora: &mut Flora, site: Site) {
    let all = i64::MIN..=i64::MAX;
    let _ = flora.take_foliage_of(site, FaceLayer::Fungus, 1.0e9, &all);
}

/// Spore saving a lone glowcap asks for over `ticks`, grazed every tick or not, under
/// `share`; and its mean cap fullness `P / (alpha · W)` over the same ticks.
fn spores(share: f64, grazed: bool, ticks: u32) -> (f64, f64) {
    let mut world = strip(4);
    let mut config = FloraConfig::default();
    config.glowcap.propagule_cap_share = share;
    let mut flora = Flora::new(config);
    let sc = flora.config().species(Species::Glowcap).clone();
    let site = glowcap_on_log(&mut flora, &world, 1, sc.wood_max, 5.0);
    let slot = Species::Glowcap.index();
    let before = flora.view().ledger.propagule_requested[slot];
    let mut fullness = 0.0;
    for _ in 0..ticks {
        if grazed {
            graze(&mut flora, site);
        }
        let s = fungus(&flora, site);
        fullness += (s.foliage / (sc.alpha * s.wood)).clamp(0.0, 1.0);
        flora.step(&mut world);
    }
    (flora.view().ledger.propagule_requested[slot] - before, fullness / f64::from(ticks))
}

/// **A grazed glowcap saves fewer spores, in proportion to its caps.** A donor at its full
/// size on a large log, grazed to its floor every tick, against the same stand left alone:
/// at `propagule_cap_share` 1 the grazed one asks for spores at its fullness's share of the
/// ungrazed one's; at a share of a half, halfway; at 0, the two ask alike.
#[test]
fn a_grazed_glowcap_saves_spores_in_proportion_to_its_caps() {
    let ticks = 40;
    for share in [1.0, 0.5] {
        let (full, f_full) = spores(share, false, ticks);
        let (grazed, f_grazed) = spores(share, true, ticks);
        assert!(full > 0.0, "fixture: the ungrazed donor saves spores");
        assert!(f_grazed < f_full, "fixture: grazing took cap tissue ({f_grazed} against {f_full})");
        let want = (1.0 - share + share * f_grazed) / (1.0 - share + share * f_full);
        let got = grazed / full;
        assert!(got < 1.0, "share {share}: grazing did not cut spore saving ({grazed} against {full})");
        assert!((got - want).abs() < 0.02, "share {share}: spores at {got} of the ungrazed, the caps say {want}");
    }
    // Off: the caps do not matter.
    let (full, _) = spores(0.0, false, ticks);
    let (grazed, _) = spores(0.0, true, ticks);
    assert!((grazed - full).abs() <= 1e-12 * full.max(1.0), "share 0: {grazed} against {full}");
}

/// **Regrowing caps costs the mycelium.** A young glowcap on a small log, whose income
/// cannot pay for caps and wood at once: grazed every tick, it builds cap tissue out of the
/// income first and lays on less wood than the same stand left alone. The fixture sets the
/// cap regrowth rate to the wood rate's ten times, so that a few seconds of cap building
/// competes with the wood for the income whatever the shipped regrowth rate is.
#[test]
fn regrowing_grazed_caps_slows_the_mycelium() {
    let grow = |grazed: bool| -> f64 {
        let mut world = strip(4);
        let mut config = FloraConfig::default();
        config.glowcap.foliage_rate = 10.0 * config.glowcap.wood_rate;
        let mut flora = Flora::new(config);
        let sc = flora.config().species(Species::Glowcap).clone();
        let site = glowcap_on_log(&mut flora, &world, 1, 0.3 * sc.wood_max, 0.05);
        let w0 = fungus(&flora, site).wood;
        for _ in 0..60 {
            if grazed {
                graze(&mut flora, site);
            }
            flora.step(&mut world);
        }
        fungus(&flora, site).wood - w0
    };
    let (alone, grazed) = (grow(false), grow(true));
    assert!(alone > 0.0, "fixture: the ungrazed glowcap grows its mycelium ({alone})");
    assert!(grazed < alone, "grazed caps cost the mycelium nothing: {grazed} against {alone}");
}
