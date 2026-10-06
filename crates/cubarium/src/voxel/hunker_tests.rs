//! The hunker pose on the live grid ([`super::super::hunker`]): where the drawn frondgrazer
//! ends up, that standing is untouched, and that the move between is monotonic. Nothing
//! here pins a pixel or a tuned number: the targets are read from the baked models and the
//! config.

use std::path::Path;

use cubarium_gpu::voxel::{PoseTable, pose_at};
use cubarium_voxel::{Command as WorldCommand, Config as WorldConfig, Material, World};
use cubarium_voxel_fauna::{Command, Fauna, FaunaConfig, Founder, StartingStores};
use rustc_hash::{FxHashMap, FxHashSet};

use super::*;

const S: i32 = 8;

fn library() -> ModelLibrary {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/voxel-models");
    ModelLibrary::load(&root, 0.125).expect("the baked library")
}

/// A grown frondgrazer facing +x (model offsets are world offsets) on a soil plain.
fn grown_browser() -> (World, Fauna, Animal) {
    let c = WorldConfig {
        width: 24,
        height: 8,
        depth: 12,
        voxel_m: 0.125,
        ..WorldConfig::default()
    };
    let mut w = World::empty(c.clone());
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            w.apply(WorldCommand::SetMaterial {
                x,
                y: 0,
                z,
                material: Material::Soil,
            });
        }
    }
    let mut fauna = Fauna::new(FaunaConfig::placeholder());
    assert!(fauna.apply(
        &w,
        Command::IntroduceFounder {
            x: 10,
            z: 6,
            founder: Founder::Browser,
            stores: StartingStores::FULL,
            heading_rad: std::f64::consts::FRAC_PI_2,
        }
    ));
    let mut a = fauna.view().animals[0];
    a.body = fauna.view().config.founder(Founder::Browser).core.body_max;
    a.state = State::Resting;
    (w, fauna, a)
}

fn stamp(grid: &mut Animals, w: &World, fauna: &Fauna, a: &Animal, tick: u64, lib: &ModelLibrary) {
    let mut view = w.view();
    view.tick = tick;
    let fv = fauna.view();
    grid.rebuild_with(
        &view,
        Some(FaunaView {
            animals: std::slice::from_ref(a),
            ..fv
        }),
        lib,
    );
}

fn class_of(byte: u8) -> sleep::Class {
    match byte >> 6 {
        0 => sleep::Class::Torso,
        1 => sleep::Class::Breathing,
        2 => sleep::Class::Leg1,
        _ => sleep::Class::Leg2,
    }
}

/// Every model cell of the body as drawn at the grid's pose: its model offset and its
/// drawn `(bottom, top)` pixel rows (empty when folded away).
fn drawn(grid: &Animals, anchor: [i64; 3]) -> Vec<([i32; 3], (i32, i32))> {
    let table = PoseTable::EMPTY;
    grid.cells()
        .filter(|&(x, y, z)| grid.at(x.into(), y.into(), z).is_block())
        .map(|(x, y, z)| {
            let off = [
                (i64::from(x) - anchor[0]) as i32,
                (i64::from(y) - anchor[1]) as i32,
                (i64::from(z) - anchor[2]) as i32,
            ];
            let byte = grid.pose_at(x.into(), y.into(), z);
            let span = if byte == 0 {
                (off[1] * S, (off[1] + 1) * S)
            } else {
                let e = grid.pose_entries()[usize::from(byte & 63)];
                let settle = pose_at(&table, &e, 0.0).settle;
                sleep::span(class_of(byte), off[1], settle, 0.0, S)
            };
            (off, span)
        })
        .collect()
}

fn anchor_of(a: &Animal) -> [i64; 3] {
    [
        (a.pose.x / 0.125).floor() as i64,
        i64::from(a.sense_layer(0.125)) + 1,
        (a.pose.z / 0.125).floor() as i64,
    ]
}

/// Hunkered: the shield's lowest drawn cells are on the ground line, every drawn cell is
/// inside the baked hunker's cells (one voxel lower than it stood, the shield's settle),
/// and the neck, scoop and stalks that stood out of the front are folded away or inside
/// the notch, the hunker's front.
#[test]
fn a_hunkered_frondgrazer_has_its_shield_on_the_ground_and_its_neck_in_the_notch() {
    let lib = library();
    let (w, fauna, mut a) = grown_browser();
    let cfg = fauna.view().config;
    let rest = model_cells(&lib, cfg, &a).expect("the standing model");
    let target = hunker_cells(&lib, cfg, &a).expect("the baked hunker");
    let rest_of: FxHashMap<[i32; 3], ModelCell> = rest.iter().map(|m| (m.offset, *m)).collect();
    let hunker: FxHashSet<[i32; 3]> = target.iter().map(|m| m.offset).collect();
    let neck_slots: Vec<u8> = ["a_limb", "a_head", "a_glow", "a_eye"]
        .iter()
        .filter_map(|n| model::palette_index(n))
        .collect();
    let front = target.iter().map(|m| m.offset[0]).max().unwrap();
    let top = target.iter().map(|m| m.offset[1]).max().unwrap();
    // Standing, the neck stands out of the hunker's front or over its top: else this test
    // would show nothing.
    assert!(rest.iter().any(
        |m| neck_slots.contains(&m.material) && (m.offset[0] > front || m.offset[1] > top + 1)
    ));

    a.hunkered = true;
    let mut grid = Animals::empty(24, 8, 12);
    stamp(&mut grid, &w, &fauna, &a, 100, &lib);
    let anchor = anchor_of(&a);
    let cells = drawn(&grid, anchor);
    assert_eq!(
        cells.len(),
        rest.len(),
        "the standing model is what is drawn"
    );
    let shown: Vec<_> = cells.iter().filter(|(_, (lo, hi))| hi > lo).collect();
    let ground = 0;
    let lowest = shown.iter().map(|(_, (lo, _))| *lo).min().unwrap();
    assert_eq!(lowest, ground, "the shield's rim is on the ground line");
    for (off, (lo, hi)) in &shown {
        // The cell it is drawn in: whole voxels down from where it stood.
        assert_eq!(
            (hi - lo) % S,
            0,
            "{off:?} drawn {lo}..{hi}: whole at full weight"
        );
        assert_eq!(lo % S, 0);
        let at = [off[0], lo / S, off[2]];
        assert!(
            hunker.contains(&at),
            "{off:?} drawn at {at:?}, outside the baked hunker"
        );
        let m = rest_of[off];
        if neck_slots.contains(&m.material) {
            assert!(
                at[0] <= front && at[1] <= top,
                "neck cell {off:?} at {at:?} outside the notch"
            );
        }
    }
    // And the body is still there: most of the hunker is covered.
    let covered: FxHashSet<[i32; 3]> = shown
        .iter()
        .map(|(o, (lo, _))| [o[0], lo / S, o[2]])
        .collect();
    assert!(
        covered.len() * 4 >= hunker.len() * 3,
        "{} of {} hunker cells drawn",
        covered.len(),
        hunker.len()
    );
}

/// Not hunkered, the grid (cells, pose bytes, entries) is the same with or without the
/// baked hunker in the library, standing, idling (head moves and all) and asleep: the
/// resting pose is the one main draws.
#[test]
fn not_hunkered_the_resting_pose_is_unchanged() {
    let lib = library();
    let mut without = ModelLibrary::default();
    without.insert_animal(
        Founder::Browser,
        lib.animal(Founder::Browser).unwrap().clone(),
    );
    let (w, fauna, mut a) = grown_browser();
    let mut with_grid = Animals::empty(24, 8, 12);
    let mut without_grid = Animals::empty(24, 8, 12);
    let mut posed = 0;
    for tick in 100..(100 + (2.5 * idle::MOVE_EVERY_S / DT) as u64) {
        a.state = if (400..440).contains(&tick) {
            State::Asleep
        } else {
            State::Resting
        };
        stamp(&mut with_grid, &w, &fauna, &a, tick, &lib);
        stamp(&mut without_grid, &w, &fauna, &a, tick, &without);
        assert_eq!(
            with_grid.pose_entries(),
            without_grid.pose_entries(),
            "tick {tick}"
        );
        let bytes = |g: &Animals| {
            let mut v: Vec<_> = g
                .cells()
                .map(|(x, y, z)| {
                    (
                        (x, y, z),
                        g.at(x.into(), y.into(), z).is_block(),
                        g.pose_at(x.into(), y.into(), z),
                    )
                })
                .collect();
            v.sort();
            v
        };
        assert_eq!(bytes(&with_grid), bytes(&without_grid), "tick {tick}");
        assert!(
            with_grid
                .pose_entries()
                .iter()
                .all(|e| e.hunker == 0.0 && e.hunker_rate == 0.0)
        );
        posed += usize::from(!with_grid.pose_entries().is_empty());
    }
    assert!(posed > 0, "it idled or slept, so the poses were compared");
}

/// The weight the GPU draws every frame, and the settle it gives, only rise while hunkered
/// and only fall after, by at most a frame's share of the configured transition time, and
/// get all the way down and back up. A shorter time (the knob) is quicker.
#[test]
fn the_hunker_transition_is_monotonic_over_its_time() {
    let lib = library();
    let (w, fauna, mut a) = grown_browser();
    for hunker_s in [hunker::DEFAULT_S, 0.4] {
        let mut grid = Animals::empty(24, 8, 12);
        grid.set_hunker_s(hunker_s);
        let table = PoseTable::EMPTY;
        let (mut weights, mut settles) = (Vec::new(), Vec::new());
        let (on, off) = (110u64, 160u64);
        for tick in 100..220u64 {
            a.hunkered = (on..off).contains(&tick);
            stamp(&mut grid, &w, &fauna, &a, tick, &lib);
            let (wt, rate) = grid.hunker_weight(&a).unwrap_or((0.0, 0.0));
            let body = grid
                .pose_entries()
                .iter()
                .find(|e| e.hunker > 0.0 || e.hunker_rate != 0.0)
                .copied();
            for k in 0..3 {
                let f = k as f32 / 3.0;
                weights.push((tick, (wt + rate * f).clamp(0.0, 1.0)));
                settles.push((tick, body.map_or(0.0, |e| pose_at(&table, &e, f).settle)));
            }
        }
        let frame = (1.0 / 60.0 / hunker_s) as f32;
        for p in weights.windows(2) {
            let d = p[1].1 - p[0].1;
            assert!(
                d.abs() <= frame + 1e-5,
                "{hunker_s} s: a frame moved the weight {d}"
            );
            if p[1].0 < off {
                assert!(
                    d >= 0.0,
                    "{hunker_s} s: going down, the weight fell at tick {}",
                    p[1].0
                );
            } else {
                assert!(
                    d <= 0.0,
                    "{hunker_s} s: coming up, the weight rose at tick {}",
                    p[1].0
                );
            }
        }
        for p in settles.windows(2) {
            let d = p[1].1 - p[0].1;
            if p[1].0 < off {
                assert!(d >= -1e-6, "{hunker_s} s: the body rose while going down");
            } else {
                assert!(d <= 1e-6, "{hunker_s} s: the body sank while coming up");
            }
        }
        let down_at = weights
            .iter()
            .find(|(_, w)| *w >= 1.0)
            .expect("it got all the way down")
            .0;
        let took = (down_at - on) as f64 * DT;
        assert!(
            (took - hunker_s).abs() <= DT + 1e-9,
            "{hunker_s} s: down in {took} s"
        );
        assert_eq!(weights.last().unwrap().1, 0.0, "and back up");
        assert!(settles.iter().any(|(_, s)| *s > 0.0));
    }
}
