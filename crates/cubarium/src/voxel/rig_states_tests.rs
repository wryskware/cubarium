//! RIG-STATES (`design/handoffs/rig-states-2026-10-10.md`): rigged fauna's sleep,
//! idle and hunker play as rig clips with their states' eased weights, the class path's
//! moves standing down, and the clips end where the class path's poses end. Nothing here
//! pins a cell count or a tuned number: the targets are the class path's own rules
//! (`sleep::classify`, `hunker::classify`) on the baked models.

use std::path::Path;

use cubarium_voxel::{Command as WorldCommand, Config as WorldConfig, Material, World};
use cubarium_voxel_fauna::{Command, Fauna, FaunaConfig, MobilityMode, StartingStores};
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::rig::ClipAt;
use super::*;

fn library() -> ModelLibrary {
    library_at(0.125)
}

fn library_at(voxel_m: f64) -> ModelLibrary {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/voxel-models");
    ModelLibrary::load(&root, voxel_m).expect("the baked library")
}

/// A grown frondgrazer standing on a soil plain.
fn grown() -> (World, Fauna, Animal) {
    let c = WorldConfig { width: 24, height: 8, depth: 12, voxel_m: 0.125, ..WorldConfig::default() };
    let mut w = World::empty(c.clone());
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            w.apply(WorldCommand::SetMaterial { x, y: 0, z, material: Material::Soil });
        }
    }
    let mut fauna = Fauna::new(FaunaConfig::placeholder());
    assert!(fauna.apply(
        &w,
        Command::IntroduceFounder { x: 10, z: 6, founder: Founder::Browser, stores: StartingStores::FULL, heading_rad: 1.0 }
    ));
    let mut a = fauna.view().animals[0];
    a.body = fauna.view().config.founder(Founder::Browser).core.body_max;
    a.state = State::Resting;
    (w, fauna, a)
}

/// The body's figure after the grid has followed it from `from` to `to` (ticks).
fn figure(lib: &ModelLibrary, models: &mut FigureModels, a: &Animal, from: u64, to: u64) -> Spec {
    let (w, fauna, _) = grown();
    let c = w.config().clone();
    let mut grid = Animals::empty(c.width, c.height, c.depth);
    let mut out = Vec::new();
    for tick in from..=to {
        let mut view = w.view();
        view.tick = tick;
        let fv = FaunaView { animals: std::slice::from_ref(a), ..fauna.view() };
        grid.rebuild_with(&view, Some(fv), lib);
        specs(&grid, &view, fv, Some(lib), &Default::default(), models, &mut out);
    }
    out.into_iter().find(|s| s.key == (a.id, 0)).expect("its figure")
}

/// Asleep, idling and hunkered, the figure carries its rig's clip for that state (eased
/// in from its weight), and its pose says the rig plays it, so the class path stands down.
#[test]
fn a_rigged_frondgrazer_plays_sleep_idle_and_hunker_as_clips() {
    let lib = library();
    let rig = lib.rig(Founder::Browser).expect("the frondgrazer's rig");
    let (_, _, standing) = grown();
    for (name, state, hunkered, ticks) in [
        ("asleep", State::Asleep, false, 1),
        ("hunker", State::Resting, true, 1),
        // idling starts once it has stood still for the hold
        ("idle", State::Resting, false, (2.0 * idle::HOLD_S / DT) as u64),
    ] {
        let a = Animal { state, hunkered, ..standing };
        let mut models = FigureModels::default();
        let s = figure(&lib, &mut models, &a, 100, 100 + ticks);
        let clip = rig.clip(name).unwrap_or_else(|| panic!("a {name} clip"));
        let on = s.clips.iter().find(|c| usize::from(c.clip) == clip).unwrap_or_else(|| panic!("{name}: no clip"));
        assert!(on.weight > 0.0 || on.rate > 0.0, "{name}: its weight eases in");
        let pose = s.pose.expect("a pose");
        assert_eq!(pose.rig, RigStates { asleep: true, idle: true, hunker: true }, "{name}");
    }
}

/// Every port's sleep uses the state weight and leaves the class breath in place.
#[test]
fn the_sleep_ports_play_their_track_with_the_states_eased_weight() {
    let lib = library();
    let (_, _, standing) = grown();
    for (founder, mode) in [
        (Founder::Chorister, MobilityMode::Ground),
        (Founder::Lanternjaw, MobilityMode::Ground),
        (Founder::Seedporter, MobilityMode::Ground),
        (Founder::Seedporter, MobilityMode::Perched),
        (Founder::Bellwing, MobilityMode::Ground),
    ] {
        let mut a = Animal {
            founder: Some(founder),
            state: State::Asleep,
            ..standing
        };
        a.mobility.mode = mode;
        let rig = lib
            .rig_in_mode(founder, mode)
            .expect("its rig in the mode")
            .0;
        let clip = rig.clip("asleep").expect("sleep track");
        let mut models = FigureModels::default();
        let s = figure(&lib, &mut models, &a, 100, 101);
        assert!(
            s.clips
                .iter()
                .any(|c| usize::from(c.clip) == clip && (c.weight > 0.0 || c.rate > 0.0))
        );
        let p = s.pose.expect("the sleep pose");
        assert!(p.rig.asleep);
        assert!(!p.rig.idle && !p.rig.hunker, "only sleep ported");
        assert_eq!(p.at(1.0, true)[0], 0.0, "the class settle stands down");
        assert_eq!(
            p.at(1.0, true)[1],
            p.at(1.0, false)[1],
            "the class breath stays"
        );
    }
}

#[test]
fn a_bellwing_settles_on_its_rig_only_on_the_ground() {
    let lib = library();
    let (_, _, standing) = grown();
    for mode in [MobilityMode::Ground, MobilityMode::Perched, MobilityMode::Fly] {
        let mut a = Animal { founder: Some(Founder::Bellwing), state: State::Asleep, ..standing };
        a.mobility.mode = mode;
        let rig = lib.rig_in_mode(Founder::Bellwing, mode).expect("the shared bellwing rig").0;
        let clip = rig.clip("asleep").expect("sleep track");
        let mut models = FigureModels::default();
        let s = figure(&lib, &mut models, &a, 100, 101);
        let ground = mode == MobilityMode::Ground;
        assert_eq!(s.pose.expect("class sleep pose").rig.asleep, ground);
        assert_eq!(s.clips.iter().any(|c| usize::from(c.clip) == clip), ground);
    }
}

/// With the rig drawn the hunkered body wears its standing model (no swap to the classes
/// toward the baked hunker); with the rig off the class path swaps, as before.
#[test]
fn the_hunker_swaps_its_model_only_when_the_rig_is_not_drawn() {
    let lib = library();
    let (_, _, standing) = grown();
    let hunkered = Animal { hunkered: true, ..standing };
    for drawn in [true, false] {
        let mut models = FigureModels::default();
        models.set_rig_drawn(drawn);
        let up = figure(&lib, &mut models, &standing, 100, 100).model;
        let down = figure(&lib, &mut models, &hunkered, 100, 100).model;
        assert_eq!(up == down, drawn, "rig drawn {drawn}");
    }
}

/// Each cell of bin `bin` posed by `clip` at full weight: the place its centre lands in
/// (model offsets, whole voxels), by cell.
fn posed(rig: &AnimalRig, bin: usize, cells: &[ModelCell], clip: usize) -> Vec<[i32; 3]> {
    let mut pose = Vec::new();
    rig.pose(bin, &[ClipAt { clip, time_s: 0.0, weight: 1.0 }], &mut pose);
    cells
        .iter()
        .map(|c| {
            let m = &pose[usize::from(rig.bone_of(bin, c.offset))];
            let p = [f64::from(c.offset[0]), f64::from(c.offset[1]) + 0.5, f64::from(c.offset[2])];
            let q: [f64; 3] = [0, 1, 2].map(|i| (0..3).map(|j| f64::from(m[i][j]) * p[j]).sum::<f64>() + f64::from(m[i][3]));
            [q[0].round() as i32, q[1].floor() as i32, q[2].round() as i32]
        })
        .collect()
}

const STEPS: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

/// What a body occupying `places` (model offsets, `[a, row, c]`) shows: each face of a
/// place over the ground line whose neighbour is open air reached from outside the body
/// (a pocket shut inside it shows nothing).
fn outer_faces(places: &FxHashSet<[i32; 3]>) -> FxHashSet<([i32; 3], usize)> {
    let over: FxHashSet<[i32; 3]> = places.iter().copied().filter(|p| p[1] >= 0).collect();
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for p in &over {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i] - 1);
            hi[i] = hi[i].max(p[i] + 1);
        }
    }
    lo[1] = 0;
    let start = [lo[0], hi[1], lo[2]];
    let (mut air, mut todo) = (FxHashSet::from_iter([start]), vec![start]);
    while let Some(q) = todo.pop() {
        for d in STEPS {
            let r = [q[0] + d[0], q[1] + d[1], q[2] + d[2]];
            if (0..3).all(|i| (lo[i]..=hi[i]).contains(&r[i])) && !over.contains(&r) && air.insert(r) {
                todo.push(r);
            }
        }
    }
    over.iter()
        .flat_map(|&p| (0..6).map(move |k| (p, k)))
        .filter(|&(p, k)| air.contains(&[p[0] + STEPS[k][0], p[1] + STEPS[k][1], p[2] + STEPS[k][2]]))
        .collect()
}

/// The places a posed body's cells take, checking that two cells share a place only
/// inside the body (all six neighbours held, or under the ground line), so no face of one
/// is drawn over the other's.
fn places(posed: &[[i32; 3]], what: &str) -> FxHashSet<[i32; 3]> {
    let mut count: FxHashMap<[i32; 3], usize> = FxHashMap::default();
    for p in posed {
        *count.entry(*p).or_default() += 1;
    }
    for (p, &n) in &count {
        if n > 1 && p[1] >= 0 {
            let inside = STEPS.iter().all(|d| {
                let q = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
                q[1] < 0 || count.contains_key(&q)
            });
            assert!(inside, "{what}: {n} cells share {p:?} on the body's surface");
        }
    }
    count.into_keys().collect()
}

/// In every size bin, the rig's asleep and hunker tracks at full weight show what the
/// class path's poses show (for the rigs whose sleep is still today's settle; the sleep
/// poses of RIG-STATES round 3 end on their own pose, `the_sleep_poses_end_gap_free_and_on_the_ground`): asleep, the torso row and all over it lowered by its settle,
/// its cut leg rows gone and unclassified rows kept; hunkered, the same where the baked hunker holds a cell (the
/// neck, scoop and stalks inside the shield or under the ground line).
#[test]
fn the_rigs_asleep_and_hunker_end_on_the_class_paths_poses() {
    for voxel_m in [0.125, 0.25] {
        let lib = library_at(voxel_m);
        for founder in [Founder::Browser, Founder::Seedporter, Founder::Bellwing] {
            let (rig, model) = lib
                .rig_in_mode(founder, MobilityMode::Ground)
                .expect("ground rig and model");
            let asleep = rig.clip("asleep").expect("asleep");
            for (bin, b) in model.bins.iter().enumerate() {
                let local: Vec<sleep::Local> = b
                    .cells
                    .iter()
                    .map(|m| sleep::Local {
                        a: m.offset[0],
                        c: m.offset[2],
                        row: m.offset[1],
                        bump: true,
                    })
                    .collect();
                let k = sleep::classify(&local);
                // The class path keeps unclassified rows (e.g. paws further under
                // the torso than its maximum drop), cuts its leg classes, lowers torso.
                let sleeping: FxHashSet<[i32; 3]> = b
                    .cells
                    .iter()
                    .zip(&k.class)
                    .filter_map(|(m, c)| match c {
                        None => Some(m.offset),
                        Some(sleep::Class::Torso | sleep::Class::Breathing) => {
                            Some([m.offset[0], m.offset[1] - k.drop, m.offset[2]])
                        }
                        _ => None,
                    })
                    .collect();
                let shows = |clip: usize, what: &str| {
                    outer_faces(&places(&posed(&rig, bin, &b.cells, clip), what))
                };
                assert_eq!(
                    shows(asleep, "asleep"),
                    outer_faces(&sleeping),
                    "{voxel_m} {} bin {bin} asleep",
                    founder.name()
                );
                println!(
                    "{voxel_m} {} bin {bin}: settle {}, asleep end faces equal: {} faces",
                    founder.name(),
                    k.drop,
                    outer_faces(&sleeping).len()
                );
                if let Some(down) = rig.clip("hunker") {
                    let hunker = lib
                        .specimen(super::super::model::hunker_model(founder).unwrap())
                        .expect("the baked hunker");
                    let target: FxHashSet<[i32; 3]> =
                        hunker.bins[bin].cells.iter().map(|m| m.offset).collect();
                    let hunkered: FxHashSet<[i32; 3]> = sleeping
                        .iter()
                        .copied()
                        .filter(|p| target.contains(p))
                        .collect();
                    assert_eq!(
                        shows(down, "hunker"),
                        outer_faces(&hunkered),
                        "{voxel_m} bin {bin} hunkered"
                    );
                }
            }
        }
    }
}

/// Undo only the appended sleep skeleton, then compare what the frame actually draws.
/// No fixture hashes or tuned counts: the old feed keys, cells and class rules supply
/// the reference. Include bounds, lip/crease bits, and each cell's transform so identity
/// child keys cannot hide extra internal faces or a changed atlas sampling phase.
#[test]
fn zero_sleep_weight_draws_the_original_feed_rig_cell_for_cell() {
    for voxel_m in [0.125, 0.25] {
        let lib = library_at(voxel_m);
        for (founder, mode) in [
            (Founder::Chorister, MobilityMode::Ground),
            (Founder::Lanternjaw, MobilityMode::Ground),
            (Founder::Seedporter, MobilityMode::Ground),
            (Founder::Seedporter, MobilityMode::Perched),
            (Founder::Bellwing, MobilityMode::Ground),
        ] {
            let (rig, model) = lib
                .rig_in_mode(founder, mode)
                .expect("its rig in the mode");
            let original = Arc::new(rig.before_sleep_port());
            for (bin, b) in model.bins.iter().enumerate() {
                let mut models = FigureModels::default();
                let port = models
                    .intern_rigged(1, Some((rig.clone(), bin)), || {
                        baked(&b.cells, None, Some((&rig, bin)))
                    })
                    .unwrap();
                let reference = models
                    .intern_rigged(2, Some((original.clone(), bin)), || {
                        baked(&b.cells, None, Some((&original, bin)))
                    })
                    .unwrap();
                for feed_weight in [0.0, 0.37, 1.0] {
                    let draw = |model, r: &AnimalRig, sleep_weight| {
                        let mut clips = vec![ClipSpec {
                            clip: r.clip("cropping").unwrap() as u8,
                            tick: 1,
                            weight: feed_weight,
                            rate: 0.0,
                            phase_s: 0.37 * r.loop_s(r.clip("cropping").unwrap()),
                        }];
                        if let Some(clip) = r.clip("asleep") {
                            clips.push(ClipSpec {
                                clip: clip as u8,
                                tick: 1,
                                weight: sleep_weight,
                                rate: 0.0,
                                phase_s: 0.0,
                            });
                        }
                        let s = Spec {
                            key: (1, 0),
                            model,
                            pos: [10.0, 2.0, 6.0],
                            orient: Orient::Ground(1.0),
                            palette: vec![],
                            pose: Some(PoseSpec {
                                tick: 1,
                                body: PoseEntry {
                                    drop: models.info(model).drop as f32,
                                    idle: 0.6,
                                    idle_phase: 0.3,
                                    ..PoseEntry::ZERO
                                },
                                head: Some(PoseEntry {
                                    idle: 0.6,
                                    idle_phase: 0.3,
                                    head: HeadMove {
                                        start: 0.0,
                                        ticks: 3.0,
                                        dip: 0.4,
                                    },
                                    ..PoseEntry::ZERO
                                }),
                                sleep: sleep::breath(1, DT),
                                idle: idle::breath(1, DT),
                                rig: RigStates::of(r),
                            }),
                            clips,
                            aim: None,
                        };
                        let mut track = FigureTrack::new(FigureConfig::default(), 32);
                        track.push(1, &[s], 0.0);
                        track.frame(1.0, None, &models).0
                    };
                    let (a, z) = (draw(port, &rig, 0.0), draw(reference, &original, 0.0));
                    let (a_tile, z_tile) = (a.figures[0].a, z.figures[0].a);
                    assert_eq!(
                        a_tile.radius, z_tile.radius,
                        "{founder:?} bin {bin}: atlas bounds"
                    );
                    assert_eq!(a_tile.centre, z_tile.centre);
                    assert_eq!(a_tile.pose, z_tile.pose);
                    assert_eq!(a_tile.count, z_tile.count);
                    let cells = |t: cubarium_gpu::figures::FigureTile| {
                        &models.voxels[t.first as usize..(t.first + t.count) as usize]
                    };
                    for (v, old) in cells(a_tile).iter().zip(cells(z_tile)) {
                        assert_eq!(
                            v,
                            old,
                            "{founder:?} bin {bin} {:?}: cell, class, lip, faces and shade",
                            v.offset()
                        );
                        if let (Some((af, _)), Some((zf, _))) = (a_tile.bones, z_tile.bones) {
                            assert_eq!(
                                a.bones[af as usize + v.bone() as usize],
                                z.bones[zf as usize + old.bone() as usize]
                            );
                        } else {
                            assert_eq!(a_tile.bones, z_tile.bones);
                        }
                    }
                    // The alternate cells are selected at the frame's evaluated weight,
                    // including the first positive weight and the return to exact zero.
                    if models.info(port).without_tracks.is_some() {
                        assert_eq!(
                            a_tile.first,
                            models
                                .info(port)
                                .without_tracks
                                .map(|m| models.info(m).first)
                                .unwrap()
                        );
                        assert_eq!(
                            draw(port, &rig, 0.01).figures[0].a.first,
                            models.info(port).first
                        );
                    }
                }
            }
        }
    }
}

/// Each cell of bin `bin` posed by `clip` at full weight: its 8 corners and its centre
/// (model offsets, voxels; y up) and its turn (the bone's 3 x 3 rows).
fn posed_cells(rig: &AnimalRig, bin: usize, cells: &[ModelCell], clip: usize) -> Vec<([[f64; 3]; 8], [f64; 3], [[f64; 3]; 3])> {
    let mut pose = Vec::new();
    rig.pose(bin, &[ClipAt { clip, time_s: 0.0, weight: 1.0 }], &mut pose);
    cells
        .iter()
        .map(|c| {
            let m = &pose[usize::from(rig.bone_of(bin, c.offset))];
            let at = |p: [f64; 3]| -> [f64; 3] {
                [0, 1, 2].map(|i| (0..3).map(|j| f64::from(m[i][j]) * p[j]).sum::<f64>() + f64::from(m[i][3]))
            };
            let o = c.offset.map(f64::from);
            let mut corners = [[0.0; 3]; 8];
            for (k, corner) in corners.iter_mut().enumerate() {
                let d = [if k & 1 == 0 { -0.5 } else { 0.5 }, if k & 2 == 0 { 0.0 } else { 1.0 }, if k & 4 == 0 { -0.5 } else { 0.5 }];
                *corner = at([o[0] + d[0], o[1] + d[1], o[2] + d[2]]);
            }
            let turn = [0, 1, 2].map(|i| [0, 1, 2].map(|j| f64::from(m[i][j])));
            (corners, at([o[0], o[1] + 0.5, o[2]]), turn)
        })
        .collect()
}

/// RIG-STATES round 3's sleep poses (the Veilhound lying down, the lanternjaw's folded arms,
/// the seedporter hunched on its perch) no longer end on the class path's settle; in every
/// size bin at both voxel sizes they end gap-free and resting where they sleep. On the
/// ground each cell ends wholly under the ground line (folded away) or wholly over it, the
/// lowest shown resting on it; on the perch nothing hides and the feet and the hanging tail
/// (the cells at and under the perch's row) keep their places. Either way the shown cells
/// are one piece and no two take one place facing the same way (their faces would fight).
#[test]
fn the_sleep_poses_end_gap_free_and_on_the_ground() {
    const EPS: f64 = 0.15;
    let dist = |a: [f64; 3], b: [f64; 3]| (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt();
    for voxel_m in [0.125, 0.25] {
        let lib = library_at(voxel_m);
        for (founder, mode) in [
            (Founder::Chorister, MobilityMode::Ground),
            (Founder::Lanternjaw, MobilityMode::Ground),
            (Founder::Seedporter, MobilityMode::Perched),
        ] {
            let ground = mode == MobilityMode::Ground;
            let (rig, model) = lib.rig_in_mode(founder, mode).expect("its rig in the mode");
            let asleep = rig.clip("asleep").expect("a sleep track");
            for (bin, b) in model.bins.iter().enumerate() {
                let what = format!("{voxel_m} {} {mode:?} bin {bin}", founder.name());
                let posed = posed_cells(rig, bin, &b.cells, asleep);
                let mut shown = Vec::new();
                for (c, (corners, centre, turn)) in b.cells.iter().zip(&posed) {
                    let lo = corners.iter().map(|p| p[1]).fold(f64::MAX, f64::min);
                    let hi = corners.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
                    if ground {
                        if hi <= 0.02 {
                            continue;
                        }
                        assert!(lo >= -EPS, "{what}: cell {:?} ends across the ground line ({lo:.2} .. {hi:.2})", c.offset);
                    } else if c.offset[1] <= 0 {
                        let rest = [f64::from(c.offset[0]), f64::from(c.offset[1]) + 0.5, f64::from(c.offset[2])];
                        assert!(dist(*centre, rest) < 1e-4, "{what}: perch cell {:?} moved", c.offset);
                    }
                    shown.push((lo, *centre, *turn));
                }
                if ground {
                    let lowest = shown.iter().map(|s| s.0).fold(f64::MAX, f64::min);
                    assert!(lowest.abs() <= EPS, "{what}: the sleeping body floats ({lowest:.2})");
                }
                for (i, a) in shown.iter().enumerate() {
                    for c in &shown[i + 1..] {
                        let same_turn = (0..3).all(|r| (0..3).all(|k| (a.2[r][k] - c.2[r][k]).abs() < 0.17));
                        assert!(!(dist(a.1, c.1) <= 0.5 && same_turn), "{what}: two cells share {:?}", a.1);
                    }
                }
                let mut joined = vec![false; shown.len()];
                let mut todo = vec![0];
                joined[0] = true;
                while let Some(i) = todo.pop() {
                    for (k, s) in shown.iter().enumerate() {
                        if !joined[k] && dist(shown[i].1, s.1) <= 1.8 {
                            joined[k] = true;
                            todo.push(k);
                        }
                    }
                }
                assert!(joined.iter().all(|&j| j), "{what}: the sleeping body is in pieces");
                println!("{what}: {} cells show, {} folded away", shown.len(), b.cells.len() - shown.len());
            }
        }
    }
}
