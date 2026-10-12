//! **NUTS, the crop's fall** (Wrysk 2026-10-12: "vaulttree should probably have nuts";
//! `design/larva-nuts-plan-2026-10-12.md` §2): the vaulttree's nuts are its fruit crop, so an
//! uneaten crop falls to the site's litter with its mineral at `fruit_drop_s`, booked as
//! `fruit_fell`; nothing is lost. The fixture's rate and drop time are set here; the nut's
//! mineral share is read from the config.

use cubarium_voxel::weather::DayClock;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Site, Species};

/// A wet soil plain 16 × 32 × 8 at 0.25 m held at noon.
fn noon() -> World {
    let (width, depth) = (16u32, 8u32);
    let mut w = World::empty(VoxelConfig {
        width,
        height: 32,
        depth,
        voxel_m: 0.25,
        seed: 5,
        day: DayClock { start_hour: 12.0, ..DayClock::default() },
        ..VoxelConfig::default()
    });
    let m = cubarium_voxel::Materials::DEFAULT;
    let cap = m.pore_capacity(Material::Soil) * w.config().voxel_volume();
    let fc = m.field_capacity(Material::Soil);
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=2 {
                w.apply(WorldCommand::AddWater { x, y, z, volume_m3: fc * cap });
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    assert!(w.life_daylight() > 0.9, "fixture: noon");
    w
}

/// The vaulttree's crop at fixture speed, falling after `drop_s`, never coming back; no seed
/// parcel.
fn config(drop_s: f64) -> FloraConfig {
    let mut c = FloraConfig::default();
    c.vaulttree.fruit_per_s = 1e-3;
    c.vaulttree.fruit_cap = 2e-3;
    c.vaulttree.fruit_drop_s = drop_s;
    c.vaulttree.fruit_rest_s = 1e9;
    c.vaulttree.flower_s = 0.0;
    c.vaulttree.propagule_rate = 0.0;
    c
}

fn vaulttree(flora: &mut Flora, w: &World) -> Site {
    let wood = flora.config().species(Species::Vaulttree).wood_max;
    assert!(flora.apply(w, Command::Seed { x: 8, z: 4, species: Species::Vaulttree, wood }));
    Site { x: 8, y: 2, z: 4 }
}

fn closes(flora: &Flora, when: &str) {
    let v = flora.view();
    for (name, got, want) in [
        ("organic", v.organic(), v.ledger.expected_organic()),
        ("mineral", v.mineral(), v.ledger.expected_mineral()),
        ("energy", v.energy(), v.ledger.expected_energy()),
    ] {
        assert!((got - want).abs() <= 1e-9 * got.abs().max(1.0), "{when}: the {name} ledger must close: {got} against {want}");
    }
}

/// **An uneaten crop falls to the litter with its mineral at `fruit_drop_s`.** Twins of one
/// vaulttree, one dropping after 4 s and one never: nothing falls before 4 s; at the tick the
/// crop falls, the dropping twin's site gains the crop's organic matter and
/// `fruit_mineral_fraction` of it in mineral over its twin's (within that tick's litter
/// turnover), its stand gives up that mineral (its own settling with the site aside),
/// and the ledger books the crop fallen. The books close every tick.
#[test]
fn an_uneaten_nut_crop_falls_to_the_litter_with_its_mineral_at_fruit_drop_s() {
    // After the crop is made (2 s at the fixture rate), so the fall takes a finished crop.
    const DROP_S: f64 = 4.0;
    let fraction = FloraConfig::default().vaulttree.fruit_mineral_fraction;
    assert!(fraction > 0.0, "fixture: nuts carry mineral");
    let mut day = noon();
    let mut drops = Flora::new(config(DROP_S));
    let mut hangs = Flora::new(config(1e9));
    let site = vaulttree(&mut drops, &day);
    let _ = vaulttree(&mut hangs, &day);
    assert!(drops.view().stand_at(site).expect("tree").profile_stage >= 1, "fixture: a crowned tree");
    let drop_ticks = (DROP_S / DT).round() as usize;
    let litter = |f: &Flora| f.view().ground_at(site).map_or((0.0, 0.0), |g| (g.litter, g.litter_mineral));
    let mut fell_at = None;
    for t in 1..=drop_ticks + 40 {
        let before = drops.view().stand_at(site).expect("tree").fruit;
        let mineral_before = drops.view().stand_at(site).expect("tree").mineral;
        let (h_mineral_before, h_fruit_before) = {
            let s = hangs.view().stand_at(site).expect("twin");
            (s.mineral, s.fruit)
        };
        drops.step(&mut day);
        hangs.step(&mut day);
        closes(&drops, &format!("tick {t}"));
        let fell = drops.view().ledger.fruit_fell[Species::Vaulttree.index()];
        if fell_at.is_none() && fell > 0.0 {
            fell_at = Some(t);
            assert!(t + 1 >= drop_ticks, "tick {t}: nothing falls before {DROP_S} s");
            assert!(before > 0.0, "fixture: a crop hung");
            assert!((fell - before).abs() <= 1e-15, "the whole crop fell: {fell} of {before}");
            assert_eq!(drops.view().stand_at(site).expect("tree").fruit, 0.0, "none hangs after the fall");
            let (o, m) = litter(&drops);
            let (ho, hm) = litter(&hangs);
            // The twins differ by the crop: in the litter, and in what the hanging twin made
            // this tick (its fruit was already full: it made none).
            let h_made = hangs.view().stand_at(site).expect("twin").fruit - h_fruit_before;
            assert!(h_made.abs() <= 1e-15, "fixture: the twin's crop is full");
            // Within this tick's turnover of the litter (it decays and releases at once).
            assert!(((o - ho) - before).abs() <= 1e-4 * before, "the litter gains the crop: {} against {before}", o - ho);
            assert!(((m - hm) - fraction * before).abs() <= 1e-4 * fraction * before, "with its mineral: {} against {}", m - hm, fraction * before);
            let stand_gave = (mineral_before - drops.view().stand_at(site).expect("tree").mineral)
                - (h_mineral_before - hangs.view().stand_at(site).expect("twin").mineral);
            assert!((stand_gave - fraction * before).abs() <= 1e-4 * fraction * before, "out of the stand's mineral: {stand_gave}");
        }
    }
    assert!(fell_at.is_some(), "the crop fell");
    assert_eq!(hangs.view().ledger.fruit_fell[Species::Vaulttree.index()], 0.0, "the twin's crop still hangs");
}
