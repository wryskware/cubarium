//! RIG-STATES (`design/handoffs/rig-states-2026-10-10.md`): a rigged frondgrazer's sleep,
//! idle and hunker play as rig clips with their states' eased weights, the class path's
//! moves standing down, and the clips end where the class path's poses end. Nothing here
//! pins a cell count or a tuned number: the targets are the class path's own rules
//! (`sleep::classify`, `hunker::classify`) on the baked models.

use std::path::Path;

use cubarium_voxel::{Command as WorldCommand, Config as WorldConfig, Material, World};
use cubarium_voxel_fauna::{Command, Fauna, FaunaConfig, StartingStores};
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::rig::ClipAt;
use super::*;

fn library() -> ModelLibrary {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/voxel-models");
    ModelLibrary::load(&root, 0.125).expect("the baked library")
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
/// class path's poses show: asleep, the torso row and all over it lowered by its settle,
/// the legs under it gone; hunkered, the same where the baked hunker holds a cell (the
/// neck, scoop and stalks inside the shield or under the ground line).
#[test]
fn the_rigs_asleep_and_hunker_end_on_the_class_paths_poses() {
    let lib = library();
    let model = lib.animal(Founder::Browser).expect("the standing model");
    let hunker = lib.specimen(super::super::model::hunker_model(Founder::Browser).unwrap()).expect("the baked hunker");
    let rig = lib.rig(Founder::Browser).expect("the rig");
    let (asleep, down) = (rig.clip("asleep").expect("asleep"), rig.clip("hunker").expect("hunker"));
    for (bin, b) in model.bins.iter().enumerate() {
        let local: Vec<sleep::Local> =
            b.cells.iter().map(|m| sleep::Local { a: m.offset[0], c: m.offset[2], row: m.offset[1], bump: true }).collect();
        let k = sleep::classify(&local);
        let lowered = |m: &ModelCell| [m.offset[0], m.offset[1] - k.drop, m.offset[2]];
        let target: FxHashSet<[i32; 3]> = hunker.bins[bin].cells.iter().map(|m| m.offset).collect();
        let sleeping: FxHashSet<[i32; 3]> = b.cells.iter().filter(|m| m.offset[1] >= k.torso).map(lowered).collect();
        let hunkered: FxHashSet<[i32; 3]> = sleeping.iter().copied().filter(|p| target.contains(p)).collect();
        let shows = |clip: usize, what: &str| outer_faces(&places(&posed(&rig, bin, &b.cells, clip), what));
        assert_eq!(shows(asleep, "asleep"), outer_faces(&sleeping), "bin {bin} asleep");
        assert_eq!(shows(down, "hunker"), outer_faces(&hunkered), "bin {bin} hunkered");
    }
}
