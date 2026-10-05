//! **Roster R3, written before the rule** (`design/handoffs/voxel-roster-2026-10-01.md`
//! Brief R3 "Tests first"; the design is `design/predation-and-flight-2026-09-28.md`
//! §2.10, §2.14 and `design/organism-large-animals-2026-09-23.md` §5): the chorister — a
//! diurnal pack hunter on the lanternjaw's hunter table, living as packs with a den, one
//! breeder, provisioned pups and a territory.
//!
//! A child of `step` beside the predation files, sharing their fixture: a flat soil plain
//! at the 0.25 m reference voxel (soil at `y = 1..=2`, every support face `y = 2`), the
//! day clock off so every rhythm is awake, bodies driven by [`Toggle`] or by their own
//! heuristic. No run is longer than 200 ticks.
//!
//! Readings these tests fix where the brief leaves a choice (each named again where it is
//! used):
//! - **Packs are founded through [`Command::FoundPack`]**: `pack_size` adults on the den
//!   face, each a member of one new [`crate::PackRecord`], none a pup, the first the
//!   breeder. A test then places them.
//! - **A pup is a member with [`crate::PackMember::pup`] set**; a test makes one by
//!   introducing a small body and marking it. A founded adult is never a pup.
//! - **The pack seam is in the strike rule**: a carnivore's `feed` begins a strike on a
//!   body in its capsule whose structure is inside `prey_max_ratio` × the summed body of
//!   its own pack's members within `pack_radius_m` (itself included). Members of another
//!   pack do not count. A hit's wound still reads the striker's own body (§2.4).
//! - **A pup's strike is refused by the step**, whatever its controller asks.
//! - **"At the den"** is within `den_radius_m` of the den face's column centre.
//! - **Provisioning is booked** as `provision_given_organic` (out of the adults'
//!   reserves) and `provision_received_organic` (into the pups'), the difference respired.
//! - **The `calling` flag** is [`crate::PackMember::calling_ticks`], set to `call_s` when the
//!   member begins a strike.
//! - **Territory and pup distances** are a body centre's distance to a den's column centre
//!   on the ring, with a quarter-metre (one voxel) tolerance.
//! - **The lanternjaw leaves a carcass** through the prey flee rule: once the chorister
//!   hunts it, a lanternjaw is hunted, and its ambush heuristic flees a threat it sees.

use std::sync::atomic::Ordering;

use super::predation_fixture::*;
use crate::{
    Activity, BirthMode, BodyKind, Command, Departure, Diet, Fauna, FaunaConfig, Food, Founder,
    FounderPhysiology, Founding, Hunter, MobilityMode, Pack, PackMember, Reproduction,
    StartingStores, is_prey_of, kind_of,
};
use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig, Site};

const C: Founder = Founder::Chorister;

/// A chorister hungry enough to strike and not full enough to lie down.
const HUNTING_CHORISTER: StartingStores = HUNTING;

/// A small body, for a pup or a juvenile prey: three tenths of its lineage's `body_max`.
const SMALL: StartingStores = StartingStores {
    body: 0.3,
    reserve: 0.2,
};

/// A flat soil plain `width` cells (0.25 m) around the ring, 3 m deep and 2 m tall, the
/// day clock off.
fn lane(width: u32, seed: u64) -> World {
    let depth = 12u32;
    let mut w = World::empty(VoxelConfig {
        width,
        height: 8,
        depth,
        voxel_m: VOXEL_M,
        seed,
        day: DayClock {
            mode: DayMode::Off,
            ..DayClock::default()
        },
        ..VoxelConfig::default()
    });
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=SURFACE_Y {
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

/// The support face under `(x_m, z_m)`.
fn face_at(x_m: f64, z_m: f64) -> Site {
    Site {
        x: (x_m / VOXEL_M).floor() as u32,
        y: SURFACE_Y,
        z: (z_m / VOXEL_M).floor() as u32,
    }
}

/// The default table with the given pack table on the chorister.
fn config_with(pack: impl FnOnce(&mut Pack)) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    let mut p = c.founders[C.index()]
        .pack
        .expect("the chorister is a pack lineage");
    pack(&mut p);
    c.founders[C.index()].pack = Some(p);
    c.founders[C.index()].active = Activity::Always;
    c.founders[Founder::Lanternjaw.index()].active = Activity::Always;
    c
}

fn pack_of(fauna: &Fauna) -> Pack {
    fauna
        .config
        .founder(C)
        .pack
        .expect("the chorister is a pack lineage")
}

/// Found one pack on `den` (refused on the R3 stub) and return its id.
fn found_pack(fauna: &mut Fauna, world: &World, den: Site, stores: StartingStores) -> u64 {
    let before = fauna.packs().len();
    assert!(
        fauna.apply(
            world,
            Command::FoundPack {
                den,
                founder: C,
                stores,
                heading_rad: EAST,
            }
        ),
        "founding a chorister pack on {den:?}"
    );
    assert_eq!(fauna.packs().len(), before + 1, "one new pack");
    fauna.packs().last().expect("just founded").id
}

/// A pack's members, by id.
fn members(fauna: &Fauna, pack: u64) -> Vec<u64> {
    fauna
        .animals
        .iter()
        .filter(|a| a.pack.is_some_and(|m| m.pack == pack))
        .map(|a| a.id)
        .collect()
}

/// Introduce a small chorister and make it a pup of `pack`.
fn pup(fauna: &mut Fauna, world: &World, pack: u64, stores: StartingStores) -> u64 {
    let id = introduce(fauna, world, C, stores);
    animal_mut(fauna, id).pack = Some(PackMember {
        pack,
        pup: true,
        calling_ticks: 0,
    });
    id
}

/// Centre-to-centre distance on the ring from a body to a den face's column centre.
fn to_den(fauna: &Fauna, world: &World, id: u64, den: Site) -> f64 {
    let p = animal(fauna, id).pose;
    let ring = f64::from(world.config().width) * VOXEL_M;
    let (cx, cz) = (
        (f64::from(den.x) + 0.5) * VOXEL_M,
        (f64::from(den.z) + 0.5) * VOXEL_M,
    );
    let dx = (p.x - cx).rem_euclid(ring);
    let dx = dx.min(ring - dx);
    (dx * dx + (p.z - cz).powi(2)).sqrt()
}

fn chorister_strikes(fauna: &Fauna) -> u64 {
    fauna.ledger.strikes_by_founder[C.index()]
}

fn chorister_hits(fauna: &Fauna) -> u64 {
    fauna.ledger.hits_by_founder[C.index()]
}

/// The choristers alive, by id.
fn choristers(fauna: &Fauna) -> Vec<u64> {
    fauna
        .animals
        .iter()
        .filter(|a| a.founder == Some(C))
        .map(|a| a.id)
        .collect()
}

// ------------------------------------------------------------------ the table

/// Brief R3's structural choices: the lineage, the lanternjaw's table scaled by body
/// volume, live birth, diurnal, carrion, its prey list
/// (Fable: a **grounded** seedporter too, as on the lanternjaw's), its founding mode, its
/// pack, and a manifest with no lure. No tuned number is pinned.
#[test]
fn the_chorister_table_scales_the_lanternjaws_and_hunts_its_list() {
    assert_eq!(C.name(), "chorister");
    assert_eq!(C.role(), "pack_hunter");

    let ch = FounderPhysiology::placeholder(C);
    let lj = FounderPhysiology::placeholder(Founder::Lanternjaw);
    // The stocks scale from the lanternjaw's by the two tables' own body volumes.
    let r = (ch.adult_length_m * ch.adult_width_m * ch.adult_height_m)
        / (lj.adult_length_m * lj.adult_width_m * lj.adult_height_m);
    let near = |a: f64, b: f64| (a - b).abs() <= 1e-9 * b.abs().max(1e-12);
    for (what, got, base) in [
        ("body_max", ch.core.body_max, lj.core.body_max),
        ("body_min", ch.core.body_min, lj.core.body_min),
        ("birth_body", ch.core.birth_body, lj.core.birth_body),
        (
            "growth_max_per_s",
            ch.core.growth_max_per_s,
            lj.core.growth_max_per_s,
        ),
    ] {
        assert!(
            near(got, base * r),
            "{what}: {got}, not the lanternjaw's {base} x {r}"
        );
    }
    // A fraction of body, not scaled by the volume ratio (the detritus web D2 set the
    // lanternjaw's own apart from it).
    assert!(ch.core.reserve_cap > 0.0 && ch.core.reserve_cap <= 1.0, "a fraction of body, not scaled");
    assert_eq!(
        ch.core.reproduction.mode,
        BirthMode::Gestation,
        "live birth"
    );
    assert!(!ch.core.reproduction.dormant, "no egg bank");
    assert_eq!(
        ch.active,
        Activity::Diurnal,
        "the lanternjaw keeps the night"
    );
    assert_eq!(ch.diet, Diet::only(Food::Carrion));

    // The lanternjaw's hunter table and strike machinery; its own prey list.
    let h = ch.hunter.expect("the chorister hunts");
    assert_eq!(
        Hunter {
            prey: h.prey,
            lure_power: h.lure_power,
            ..Hunter::LANTERNJAW
        },
        h,
        "the lanternjaw's strike table"
    );
    for f in Founder::ALL {
        let want = matches!(
            f,
            Founder::Browser
                | Founder::Loftstrider
                | Founder::Lanternjaw
                | Founder::Seedporter
                | Founder::Bellwing
        );
        assert_eq!(
            h.hunts(f),
            want,
            "the chorister's list and the {}",
            f.name()
        );
    }
    let ljh = lj.hunter.expect("the lanternjaw hunts");
    assert!(!ljh.hunts(C), "the chorister is on no lanternjaw's list");
    assert!(!ljh.hunts(Founder::Loftstrider), "nor is the loftstrider");
    assert!(
        Founder::manifest(C)
            .actions
            .iter()
            .all(|a| a.name != "lure"),
        "no lure: {:?}",
        Founder::manifest(C)
            .actions
            .iter()
            .map(|a| a.name)
            .collect::<Vec<_>>()
    );

    // Founded as packs; the shipped count is the table's own knob.
    assert_eq!(ch.founding, Founding::Pack);
    assert_eq!(
        FaunaConfig::default().founder_counts()[C.index()],
        FounderPhysiology::frozen(C).founding_count
    );
    let p = ch.pack.expect("a pack lineage");
    assert!(p.pack_size > 0 && p.litter_size_max > 0);
    assert!(p.pack_radius_m > 0.0 && p.den_radius_m > 0.0 && p.territory_radius_m > 0.0);
    assert!(p.provision_rate_per_s > 0.0 && p.flank_offset_m > 0.0);
    assert!((0.0..1.0).contains(&p.chase_reserve_floor) && p.chase_reserve_floor > 0.0);
    for f in Founder::ALL.into_iter().filter(|&f| f != C) {
        assert!(
            FounderPhysiology::placeholder(f).pack.is_none(),
            "{}: no pack",
            f.name()
        );
    }
    FaunaConfig::default()
        .validate()
        .expect("the shipped table validates");

    // A grounded seedporter is its prey; a perched or gliding one is not.
    let world = lane(48, 1);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let hunter = introduce(&mut fauna, &world, C, StartingStores::FULL);
    let porter = introduce(
        &mut fauna,
        &world,
        Founder::Seedporter,
        StartingStores::FULL,
    );
    let (hunter, porter) = (animal(&fauna, hunter), animal(&fauna, porter));
    for (mode, want) in [
        (MobilityMode::Ground, true),
        (MobilityMode::Perched, false),
        (MobilityMode::Climb, false),
        (MobilityMode::Glide, false),
    ] {
        let mut p = porter;
        p.mobility.mode = mode;
        assert_eq!(
            is_prey_of(&fauna.config, &hunter, &p),
            want,
            "a {mode:?} seedporter"
        );
    }
}

// ------------------------------------------------------------------ the size window

/// The size-window fixture: the test's own 6 m `pack_radius_m`, and a window ratio set
/// from the two tables' own bodies so that an adult loftstrider is twice what one
/// chorister may take and two-thirds of what three may.
fn size_window_config(pack_size: usize) -> FaunaConfig {
    let mut c = config_with(|p| {
        p.pack_size = pack_size;
        p.pack_radius_m = 6.0;
    });
    let ratio = 0.5 * c.founder(Founder::Loftstrider).core.body_max / c.founder(C).core.body_max;
    c.founders[C.index()]
        .hunter
        .as_mut()
        .expect("a hunter")
        .prey_max_ratio = ratio;
    c
}

/// One case of the pack seam: `pack_size` choristers per pack, `packs` packs, the striker
/// (the first pack's first member) facing an adult loftstrider in its capsule with `feed`
/// held, every other chorister `others_behind_m` behind it. The strikes begun in 60 ticks.
fn loftstrider_strikes(pack_size: usize, packs: usize, others_behind_m: f64) -> u64 {
    let world = lane(192, 3);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(size_window_config(pack_size));
    let prey = introduce(
        &mut fauna,
        &world,
        Founder::Loftstrider,
        StartingStores::FULL,
    );
    place(&mut fauna, prey, STRIKER_X + 1.6, LANE_Z, WEST);
    let mut all = Vec::new();
    for _ in 0..packs {
        let id = found_pack(
            &mut fauna,
            &world,
            face_at(STRIKER_X, LANE_Z),
            HUNTING_CHORISTER,
        );
        all.extend(members(&fauna, id));
    }
    let striker = all[0];
    place(&mut fauna, striker, STRIKER_X, LANE_Z, EAST);
    let _feed = drive(&mut fauna, striker, 0.0, true);
    for (k, &id) in all.iter().enumerate().skip(1) {
        let z = 0.5 + 0.25 * (k as f64);
        place(&mut fauna, id, STRIKER_X - others_behind_m, z, EAST);
        drive(&mut fauna, id, 0.0, false);
    }
    step_until(&mut fauna, &world, &mut flora, 60, |f| {
        chorister_strikes(f) > 0
    });
    chorister_strikes(&fauna)
}

/// The pack seam (Brief R3): the size window measures prey against the summed body of
/// packmates within `pack_radius_m` (the fixture's 6 m), so a lone chorister does not take
/// an adult loftstrider and a pack does. Packmates further than the radius, or choristers
/// of another pack beside it, do not count.
#[test]
fn a_lone_chorister_does_not_take_a_loftstrider_and_a_pack_does() {
    let c = size_window_config(1);
    let (ch, ls) = (c.founder(C), c.founder(Founder::Loftstrider));
    let h = ch.hunter.expect("a hunter");
    assert!(
        ls.core.body_max > h.prey_max_ratio * ch.core.body_max,
        "an adult loftstrider is outside one chorister's own window"
    );
    assert!(
        ls.core.body_max <= h.prey_max_ratio * 3.0 * ch.core.body_max,
        "and inside three's"
    );
    assert_eq!(
        loftstrider_strikes(1, 1, 2.0),
        0,
        "a lone chorister lets it be"
    );
    assert!(
        loftstrider_strikes(3, 1, 2.0) > 0,
        "a pack of three, 2 m apart, strikes"
    );
    assert_eq!(
        loftstrider_strikes(3, 1, 8.0),
        0,
        "packmates 8 m off, past pack_radius_m, do not count"
    );
    assert_eq!(
        loftstrider_strikes(1, 3, 2.0),
        0,
        "three lone packs beside each other are not a pack"
    );
}

/// Two choristers' wounds on one loftstrider add (§2.10): each hit wounds it by the
/// lanternjaw's formula on the striker's own body, `strike_power × (S_pred / S_prey) ×
/// (1 − armour)`, and the second hit's wound is on top of the first.
#[test]
fn two_choristers_wounds_on_a_loftstrider_add() {
    let world = lane(192, 4);
    let mut flora = Flora::new(FloraConfig::default());
    let mut config = config_with(|p| p.pack_size = 2);
    config.founders[Founder::Loftstrider.index()].agility = 0.0;
    let mut fauna = Fauna::new(config);
    let prey = introduce(
        &mut fauna,
        &world,
        Founder::Loftstrider,
        StartingStores::FULL,
    );
    place(&mut fauna, prey, STRIKER_X + 1.6, LANE_Z, WEST);
    let pack = found_pack(
        &mut fauna,
        &world,
        face_at(STRIKER_X, LANE_Z),
        HUNTING_CHORISTER,
    );
    let pair = members(&fauna, pack);
    assert_eq!(pair.len(), 2);
    let mut feeds = Vec::new();
    for (k, &id) in pair.iter().enumerate() {
        place(
            &mut fauna,
            id,
            STRIKER_X,
            LANE_Z + if k == 0 { -0.5 } else { 0.5 },
            EAST,
        );
        feeds.push(drive(&mut fauna, id, 0.0, false));
    }
    let h = fauna.config.founder(C).hunter.expect("a hunter");
    let armour = fauna.config.founder(Founder::Loftstrider).armour;
    let per_hit = |f: &Fauna, striker: u64| {
        h.strike_power * (animal(f, striker).body / animal(f, prey).body) * (1.0 - armour)
    };
    let want = per_hit(&fauna, pair[0]) + per_hit(&fauna, pair[1]);
    assert!(
        want < 1.0,
        "two hits do not kill an adult loftstrider: {want}"
    );
    for (k, feed) in feeds.iter().enumerate() {
        feed.store(true, Ordering::Relaxed);
        let before = chorister_hits(&fauna);
        step_until(&mut fauna, &world, &mut flora, 60, |f| {
            chorister_hits(f) > before
        })
        .unwrap_or_else(|| panic!("chorister {k} never hit the loftstrider"));
        feed.store(false, Ordering::Relaxed);
    }
    let wound = animal(&fauna, prey).wound;
    // 1e-3: the seconds between the two hits heal a little (§2.4).
    assert!(
        (wound - want).abs() < 1e-3,
        "two hits wound {wound}, not {want}"
    );
    assert_closes(&fauna, "after two hits");
}

// ------------------------------------------------------------------ threat

/// The chorister hunts the lanternjaw (§2.14), so a moving chorister is a threat to a
/// lanternjaw and a lanternjaw is prey to a chorister.
#[test]
fn a_lanternjaw_reads_a_chorister_as_a_threat_and_is_its_prey() {
    let world = lane(48, 5);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let ch = introduce(&mut fauna, &world, C, StartingStores::FULL);
    let lj = introduce(
        &mut fauna,
        &world,
        Founder::Lanternjaw,
        StartingStores::FULL,
    );
    animal_mut(&mut fauna, ch).still_ticks = 0;
    let (a, b) = (animal(&fauna, lj), animal(&fauna, ch));
    assert_eq!(
        kind_of(&fauna.config, &a, &b, 2.0),
        BodyKind::Threat,
        "the lanternjaw's reading"
    );
    assert_eq!(
        kind_of(&fauna.config, &b, &a, 2.0),
        BodyKind::Prey,
        "the chorister's reading"
    );
}

/// A lanternjaw eating a carcass under its own heuristic, and a chorister walking at it from
/// 2.5 m ahead (Toggle forward, no feed). Without the chorister the lanternjaw stays at the
/// carcass for 200 ticks; with it, it leaves (its centre gets at least 1 m from where it
/// was eating).
#[test]
fn a_lanternjaw_leaves_a_carcass_a_chorister_approaches() {
    let run = |with_chorister: bool| -> (f64, f64) {
        let world = lane(48, 6);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(config_with(|_| {}));
        let lj = introduce(
            &mut fauna,
            &world,
            Founder::Lanternjaw,
            StartingStores {
                body: 1.0,
                reserve: 0.3,
            },
        );
        place(&mut fauna, lj, STRIKER_X, LANE_Z, EAST);
        let site = animal(&fauna, lj).site;
        assert!(flora.deposit(
            site,
            cubarium_voxel_flora::Deposit {
                kind: cubarium_voxel_flora::DepositKind::Carrion,
                organic: crate::SHIPPED_MASS_SCALE * 0.075,
                mineral: crate::SHIPPED_MASS_SCALE * 0.05 * 0.075,
                energy: crate::SHIPPED_MASS_SCALE * 2.0 * 0.075,
            }
        ));
        fauna
            .controllers
            .set(lj, Box::new(crate::LanternjawAmbush::new()));
        if with_chorister {
            let ch = introduce(&mut fauna, &world, C, HUNTING_CHORISTER);
            place(&mut fauna, ch, STRIKER_X + 2.5, LANE_Z, WEST);
            drive(&mut fauna, ch, 1.0, false);
        }
        let (x0, z0) = (animal(&fauna, lj).pose.x, animal(&fauna, lj).pose.z);
        let mut furthest: f64 = 0.0;
        for _ in 0..200 {
            fauna.step_with(&world, &mut flora, 1);
            let p = animal(&fauna, lj).pose;
            furthest = furthest.max(((p.x - x0).powi(2) + (p.z - z0).powi(2)).sqrt());
        }
        (
            furthest,
            fauna.ledger.eaten_carrion_by_founder[Founder::Lanternjaw.index()],
        )
    };
    let (stayed, ate) = run(false);
    assert!(
        stayed < 0.5,
        "alone it stays at the carcass: moved {stayed} m"
    );
    assert!(ate > 0.0, "and eats it");
    let (left, _) = run(true);
    assert!(
        left >= 1.0,
        "a chorister coming at it, it leaves: moved only {left} m"
    );
}

/// A lone chorister (no pack) can kill a lanternjaw: one in its capsule with no dodge dies
/// to its strikes, `Killed { by: chorister }`, and the books close.
#[test]
fn a_chorister_can_kill_a_lanternjaw() {
    let world = lane(48, 7);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config_with(|_| {}));
    let lj = introduce(
        &mut fauna,
        &world,
        Founder::Lanternjaw,
        StartingStores::FULL,
    );
    place(&mut fauna, lj, STRIKER_X + 1.2, LANE_Z, WEST);
    let ch = introduce(&mut fauna, &world, C, HUNTING_CHORISTER);
    place(&mut fauna, ch, STRIKER_X, LANE_Z, EAST);
    drive(&mut fauna, ch, 0.0, true);
    let killed = step_until(&mut fauna, &world, &mut flora, 200, |f| {
        f.view().animal(lj).is_none()
    });
    assert!(
        killed.is_some(),
        "the lanternjaw is still alive after 200 ticks"
    );
    assert_eq!(
        fauna
            .ledger
            .departed_founder(Founder::Lanternjaw, Departure::Killed { by: C }),
        1
    );
    assert_closes(&fauna, "after the kill");
}

// ------------------------------------------------------------------ breeding

/// Fast live birth for the breeding cases: every adult in surplus at once (a reserve cap
/// ten times its body, full), a 1.5 s hold, a 1 s gestation and no ordinary refractory to
/// speak of, so only the pack rules hold a birth back.
fn breeding_config(litter_size_max: u32, litter_interval_s: f64) -> FaunaConfig {
    let mut c = config_with(|p| {
        p.pack_size = 4;
        p.litter_size_max = litter_size_max;
        p.litter_interval_s = litter_interval_s;
    });
    let core = &mut c.founders[C.index()].core;
    core.reserve_cap = 10.0;
    core.reproduction = Reproduction {
        surplus_floor: 0.0,
        surplus_hold_s: 1.5,
        gestation_s: 1.0,
        birth_interval_s: 0.05,
        ..Reproduction::LIVE_BIRTH_PLACEHOLDER
    };
    c
}

/// The litters a run of `ticks` saw: `(tick, newborn ids)`, a litter being the choristers
/// that appeared on one tick. Also checks, every tick, that no member but `breeder` is
/// gestating or has given anything.
fn litters(
    fauna: &mut Fauna,
    world: &World,
    flora: &mut Flora,
    ticks: u32,
    breeder: impl Fn(&Fauna) -> Option<u64>,
) -> Vec<(u32, Vec<u64>)> {
    let mut seen = choristers(fauna);
    let mut out = Vec::new();
    for t in 1..=ticks {
        fauna.step_with(world, flora, 1);
        let now = choristers(fauna);
        let born: Vec<u64> = now
            .iter()
            .copied()
            .filter(|id| !seen.contains(id))
            .collect();
        if !born.is_empty() {
            out.push((t, born));
        }
        seen = now;
        let b = breeder(fauna);
        for a in fauna
            .animals
            .iter()
            .filter(|a| a.founder == Some(C) && Some(a.id) != b)
        {
            assert!(
                a.reproduction.escrow.is_none() && a.reproduction.given == 0.0,
                "tick {t}: #{} is not the breeder and conceived",
                a.id
            );
        }
    }
    out
}

/// Only the breeder conceives (all four adults are in surplus); a litter is 2 to
/// `litter_size_max` (3 here) pups of the pack; and with the interval far off there is one
/// litter in 200 ticks, though the ordinary rule would allow several.
#[test]
fn only_the_breeder_conceives_and_a_litter_is_two_to_litter_size_max() {
    let world = lane(48, 8);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(breeding_config(3, 1000.0));
    let den = face_at(STRIKER_X, LANE_Z);
    let pack = found_pack(&mut fauna, &world, den, StartingStores::FULL);
    let adults = members(&fauna, pack);
    assert_eq!(adults.len(), 4, "pack_size adults");
    let breeder = fauna.pack(pack).and_then(|p| p.breeder).expect("a breeder");
    assert!(adults.contains(&breeder));
    let got = litters(&mut fauna, &world, &mut flora, 200, |f| {
        f.pack(pack).and_then(|p| p.breeder)
    });
    assert_eq!(
        got.len(),
        1,
        "one litter, none before the interval: {got:?}"
    );
    let (_, born) = &got[0];
    assert!((2..=3).contains(&born.len()), "a litter of {}", born.len());
    for &id in born {
        let m = animal(&fauna, id).pack.expect("born into the pack");
        assert_eq!((m.pack, m.pup), (pack, true), "#{id} is a pup of its pack");
    }
    assert!(
        animal(&fauna, breeder).reproduction.given > 0.0,
        "the breeder gave birth"
    );
}

/// The litter interval holds the next litter back: at a 3 s interval the breeder has a
/// second litter inside 200 ticks, at least 60 ticks after the first (the ordinary rule
/// alone would allow one about 30 ticks after).
#[test]
fn a_second_litter_waits_for_the_litter_interval() {
    let world = lane(48, 9);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(breeding_config(4, 3.0));
    let pack = found_pack(
        &mut fauna,
        &world,
        face_at(STRIKER_X, LANE_Z),
        StartingStores::FULL,
    );
    let got = litters(&mut fauna, &world, &mut flora, 200, |f| {
        f.pack(pack).and_then(|p| p.breeder)
    });
    assert!(
        got.len() >= 2,
        "a second litter after the interval: {got:?}"
    );
    let gap = got[1].0 - got[0].0;
    assert!(
        gap >= 60,
        "litters {gap} ticks apart, under the 3 s interval"
    );
    for (_, born) in &got {
        assert!((2..=4).contains(&born.len()), "a litter of {}", born.len());
    }
}

/// Succession: when the breeder dies, the oldest adult left in the pack becomes the
/// breeder, and it then conceives while the others do not.
#[test]
fn when_the_breeder_dies_the_oldest_adult_breeds() {
    let world = lane(48, 10);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(breeding_config(4, 1000.0));
    fauna.set_births_enabled(false);
    let pack = found_pack(
        &mut fauna,
        &world,
        face_at(STRIKER_X, LANE_Z),
        StartingStores::FULL,
    );
    let adults = members(&fauna, pack);
    assert_eq!(adults.len(), 4);
    let breeder = fauna.pack(pack).and_then(|p| p.breeder).expect("a breeder");
    let rest: Vec<u64> = adults.iter().copied().filter(|&id| id != breeder).collect();
    for (&id, age) in rest.iter().zip([300u64, 900, 600]) {
        animal_mut(&mut fauna, id).age_ticks = age;
    }
    animal_mut(&mut fauna, breeder).age_ticks = 2000;
    let oldest = rest[1];
    crate::predation::apply_hit(&mut fauna, &mut flora, breeder, 1.0, Founder::Lanternjaw);
    assert!(
        fauna.view().animal(breeder).is_none(),
        "the breeder is dead"
    );
    fauna.step_with(&world, &mut flora, 1);
    assert_eq!(
        fauna.pack(pack).and_then(|p| p.breeder),
        Some(oldest),
        "the oldest adult left breeds"
    );
    fauna.set_births_enabled(true);
    litters(&mut fauna, &world, &mut flora, 150, |f| {
        f.pack(pack).and_then(|p| p.breeder)
    });
    let a = animal(&fauna, oldest);
    assert!(
        a.reproduction.escrow.is_some() || a.reproduction.given > 0.0,
        "the new breeder conceives"
    );
}

// ------------------------------------------------------------------ pups

/// Provisioning: an adult at the den whose reserve is above `provision_reserve_fraction`
/// passes reserve to the hungriest pup there at `provision_efficiency` (the test's 0.7,
/// above the test's 0.6 fraction), the loss
/// respired, and the books close. An adult under the fraction, or away from the den, gives
/// nothing.
#[test]
fn an_adult_at_the_den_feeds_the_hungriest_pup_at_seventy_percent() {
    let run = |adult_reserve: f64, adult_off_m: f64| -> Fauna {
        let world = lane(48, 11);
        let mut flora = Flora::new(FloraConfig::default());
        // The minimal gut: empty founding stomachs, so the adult has nothing to
        // regurgitate and the reserve route (the fallback) is what this case weighs.
        let mut config = config_with(|p| {
            p.pack_size = 1;
            p.provision_reserve_fraction = 0.6;
            p.provision_efficiency = 0.7;
        });
        config.founders[C.index()].gut.s0 = 0.0;
        let mut fauna = Fauna::new(config);
        fauna.set_births_enabled(false);
        let den = face_at(STRIKER_X, LANE_Z);
        let pack = found_pack(
            &mut fauna,
            &world,
            den,
            StartingStores {
                body: 1.0,
                reserve: adult_reserve,
            },
        );
        let adult = members(&fauna, pack)[0];
        place(&mut fauna, adult, STRIKER_X + adult_off_m, LANE_Z, EAST);
        let hungry = pup(
            &mut fauna,
            &world,
            pack,
            StartingStores {
                body: 0.3,
                reserve: 0.0,
            },
        );
        let fed = pup(
            &mut fauna,
            &world,
            pack,
            StartingStores {
                body: 0.3,
                reserve: 0.5,
            },
        );
        place(&mut fauna, hungry, STRIKER_X, LANE_Z + 0.5, EAST);
        place(&mut fauna, fed, STRIKER_X, LANE_Z - 0.5, EAST);
        let (h0, f0) = (animal(&fauna, hungry).reserve, animal(&fauna, fed).reserve);
        fauna.step_with(&world, &mut flora, 1);
        if adult_reserve > 0.6 && adult_off_m == 0.0 {
            assert!(
                animal(&fauna, hungry).reserve > h0,
                "the hungriest pup is fed first"
            );
            assert!(animal(&fauna, fed).reserve <= f0, "the fuller pup waits");
        }
        for _ in 0..19 {
            fauna.step_with(&world, &mut flora, 1);
        }
        assert_closes(&fauna, "after provisioning");
        fauna
    };
    let fauna = run(0.9, 0.0);
    let (given, received) = (
        fauna.ledger.provision_given_organic,
        fauna.ledger.provision_received_organic,
    );
    assert!(
        given > 0.0,
        "an adult at the den above the fraction provisions"
    );
    assert!(
        (received - 0.7 * given).abs() <= 1e-9 * given,
        "received {received} of {given}, not 70 %"
    );
    assert_eq!(
        run(0.5, 0.0).ledger.provision_given_organic,
        0.0,
        "under the fraction: nothing"
    );
    assert_eq!(
        run(0.9, 6.0).ledger.provision_given_organic,
        0.0,
        "away from the den: nothing"
    );
}

/// A pup inside its independence age makes no strike, whatever its controller asks: with
/// `feed` held and a juvenile grazer in its capsule it begins none in 60 ticks, where the
/// same body marked as no pup does.
#[test]
fn a_pup_makes_no_strike() {
    let run = |is_pup: bool| -> u64 {
        let world = lane(48, 12);
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(config_with(|p| p.pack_size = 1));
        let den = face_at(STRIKER_X, LANE_Z);
        let pack = found_pack(&mut fauna, &world, den, StartingStores::FULL);
        let prey = introduce(&mut fauna, &world, Founder::Browser, SMALL);
        place(&mut fauna, prey, STRIKER_X + 0.9, LANE_Z, WEST);
        let young = pup(
            &mut fauna,
            &world,
            pack,
            StartingStores {
                body: 0.5,
                reserve: 0.5,
            },
        );
        animal_mut(&mut fauna, young)
            .pack
            .as_mut()
            .expect("a member")
            .pup = is_pup;
        place(&mut fauna, young, STRIKER_X, LANE_Z, EAST);
        drive(&mut fauna, young, 0.0, true);
        step_until(&mut fauna, &world, &mut flora, 60, |f| {
            chorister_strikes(f) > 0
        });
        chorister_strikes(&fauna)
    };
    assert!(run(false) > 0, "the control: an independent body strikes");
    assert_eq!(run(true), 0, "a pup does not");
}

/// A pup under its own heuristic stays within `den_radius_m` of the den, though a juvenile
/// grazer stands 1.8 m ahead of it just outside that radius.
#[test]
fn a_pup_stays_within_den_radius_under_its_heuristic() {
    let world = lane(48, 13);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config_with(|p| p.pack_size = 1));
    let den = face_at(STRIKER_X, LANE_Z);
    let pack = found_pack(&mut fauna, &world, den, StartingStores::FULL);
    let radius = pack_of(&fauna).den_radius_m;
    let prey = introduce(&mut fauna, &world, Founder::Browser, SMALL);
    place(&mut fauna, prey, STRIKER_X + radius + 0.8, LANE_Z, WEST);
    let young = pup(
        &mut fauna,
        &world,
        pack,
        StartingStores {
            body: 0.5,
            reserve: 0.2,
        },
    );
    place(&mut fauna, young, STRIKER_X + radius - 1.0, LANE_Z, EAST);
    let teachers = fauna.config.teachers;
    fauna.controllers.set(young, crate::heuristic(C, teachers));
    for t in 1..=200 {
        fauna.step_with(&world, &mut flora, 1);
        let d = to_den(&fauna, &world, young, den);
        assert!(
            d <= radius + 0.25,
            "tick {t}: the pup is {d} m from its den"
        );
    }
    assert_eq!(chorister_strikes(&fauna), 0, "and it strikes at nothing");
}

// ------------------------------------------------------------------ territory

/// One territory case on a 64 m ring: pack A's den at 4 m, its two members at 19 and
/// 19.5 m (outside both territories), hungry, under their heuristic, a juvenile grazer at
/// 21 m — inside pack B's territory when B's den is at 30 m. With `b_pack`, B's two members
/// stand at their den under their heuristic. Returns the nearest any A member came to B's
/// den and the nearest any B member came to A's den over 200 ticks.
/// The territory fixture's own radius, which the den and grazer placements are laid out
/// around.
const TERRITORY_M: f64 = 10.0;

fn territory_case(b_pack: bool) -> (f64, f64) {
    let world = lane(256, 14);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config_with(|p| {
        p.pack_size = 2;
        p.territory_radius_m = TERRITORY_M;
    }));
    let (den_a, den_b) = (face_at(4.0, LANE_Z), face_at(30.0, LANE_Z));
    let a = found_pack(&mut fauna, &world, den_a, HUNTING_CHORISTER);
    let a_members = members(&fauna, a);
    place(&mut fauna, a_members[0], 19.0, LANE_Z - 0.25, EAST);
    place(&mut fauna, a_members[1], 19.5, LANE_Z + 0.25, EAST);
    let b_members = if b_pack {
        let b = found_pack(&mut fauna, &world, den_b, HUNTING_CHORISTER);
        let m = members(&fauna, b);
        place(&mut fauna, m[0], 30.0, LANE_Z - 0.25, WEST);
        place(&mut fauna, m[1], 30.0, LANE_Z + 0.25, EAST);
        m
    } else {
        Vec::new()
    };
    let bait = introduce(&mut fauna, &world, Founder::Browser, SMALL);
    place(&mut fauna, bait, 21.0, LANE_Z, EAST);
    let teachers = fauna.config.teachers;
    for &id in a_members.iter().chain(&b_members) {
        fauna.controllers.set(id, crate::heuristic(C, teachers));
    }
    let (mut a_near_b, mut b_near_a) = (f64::INFINITY, f64::INFINITY);
    for _ in 0..200 {
        fauna.step_with(&world, &mut flora, 1);
        for &id in &a_members {
            if fauna.view().animal(id).is_some() {
                a_near_b = a_near_b.min(to_den(&fauna, &world, id, den_b));
            }
        }
        for &id in &b_members {
            if fauna.view().animal(id).is_some() {
                b_near_a = b_near_a.min(to_den(&fauna, &world, id, den_a));
            }
        }
    }
    (a_near_b, b_near_a)
}

/// Territory: with two packs, neither pack's members enter the other's
/// `territory_radius_m` (the fixture's 10 m) around its den, though pack A's members stand 1–1.5 m from a
/// grazer just inside B's. The control: with no pack B, A's members do go after it, past
/// where B's territory would begin.
#[test]
fn with_two_packs_neither_enters_the_others_territory() {
    let radius = TERRITORY_M;
    let (a_near_b, _) = territory_case(false);
    assert!(
        a_near_b < radius - 0.25,
        "the control: with no pack B, A goes after the grazer (came to {a_near_b} m of x = 30)"
    );
    let (a_near_b, b_near_a) = territory_case(true);
    assert!(
        a_near_b >= radius - 0.25,
        "pack A came {a_near_b} m from B's den"
    );
    assert!(
        b_near_a >= radius - 0.25,
        "pack B came {b_near_a} m from A's den"
    );
}

// ------------------------------------------------------------------ call and snapshot

/// The call: a member that begins a strike calls for its `call_s` and then
/// stops.
#[test]
fn a_member_that_strikes_calls_for_call_s() {
    let world = lane(48, 15);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config_with(|p| p.pack_size = 1));
    let pack = found_pack(
        &mut fauna,
        &world,
        face_at(STRIKER_X, LANE_Z),
        HUNTING_CHORISTER,
    );
    let striker = members(&fauna, pack)[0];
    place(&mut fauna, striker, STRIKER_X, LANE_Z, EAST);
    let prey = introduce(&mut fauna, &world, Founder::Browser, SMALL);
    place(&mut fauna, prey, STRIKER_X + 1.2, LANE_Z, WEST);
    let calling = |f: &Fauna| animal(f, striker).pack.is_some_and(|m| m.calling());
    assert!(!calling(&fauna), "silent before");
    let feed = drive(&mut fauna, striker, 0.0, true);
    step_until(&mut fauna, &world, &mut flora, 60, |f| {
        chorister_strikes(f) > 0
    })
    .expect("it strikes");
    feed.store(false, Ordering::Relaxed);
    assert!(calling(&fauna), "it calls as it strikes");
    let call_ticks = (pack_of(&fauna).call_s * f64::from(crate::TICK_HZ)).round() as u32;
    for _ in 0..call_ticks + 2 {
        fauna.step_with(&world, &mut flora, 1);
    }
    assert!(!calling(&fauna), "and stops after call_s");
}

/// A snapshot round trip keeps the packs (den, breeder), every member's pack id, its pup
/// flag and its call.
#[test]
fn a_snapshot_round_trip_keeps_pack_den_breeder_pups_and_calls() {
    let world = lane(48, 16);
    let mut fauna = Fauna::new(config_with(|p| p.pack_size = 3));
    let pack = found_pack(
        &mut fauna,
        &world,
        face_at(STRIKER_X, LANE_Z),
        StartingStores::FULL,
    );
    let adults = members(&fauna, pack);
    animal_mut(&mut fauna, adults[1])
        .pack
        .as_mut()
        .expect("a member")
        .calling_ticks = 7;
    let young = pup(&mut fauna, &world, pack, SMALL);
    let loaded = Fauna::load(&fauna.save()).expect("the layer round-trips");
    assert_eq!(
        loaded.packs(),
        fauna.packs(),
        "the packs, their dens and breeders"
    );
    let record = loaded.pack(pack).expect("the pack");
    assert_eq!(record.den, face_at(STRIKER_X, LANE_Z));
    assert!(record.breeder.is_some_and(|b| adults.contains(&b)));
    for &id in adults.iter().chain([&young]) {
        assert_eq!(
            loaded.view().animal(id).expect("alive").pack,
            animal(&fauna, id).pack,
            "#{id}'s membership"
        );
    }
    assert!(
        loaded
            .view()
            .animal(adults[1])
            .expect("alive")
            .pack
            .expect("a member")
            .calling()
    );
    assert!(
        loaded
            .view()
            .animal(young)
            .expect("alive")
            .pack
            .expect("a member")
            .pup
    );
}
