//! **The scent field** (SENSES-CORE, S1 of the senses pass;
//! `design/7_Research/senses-review-2026-10-06.md` §3 item 2): smell as a field animals lay,
//! not a test between two bodies. It gives a patch and a gradient, never an individual, and
//! its level grows with the density of what lays it, so a hunter that follows it is drawn to
//! a boom with no switching rule.
//!
//! - **Channels** ([`ScentChannel`]): small prey, grazers, hunters (each lineage lays the one
//!   its [`crate::SenseTable::scent`] names) and carrion (laid by the ground's carrion stock).
//! - **Nodes**: the surface, in cells of [`crate::ScentConfig::cell_m`]. Within one cell's
//!   columns, the support faces that connect are one node; two faces in neighbouring columns
//!   connect when their heights differ by at most [`crate::ScentConfig::step_m`] and the air
//!   over the lower one is open up to the higher (a ledge connects, a wall does not). So
//!   scent never crosses solid rock, and the floor under a roof is not the roof.
//! - **The update**, every [`crate::senses::UPDATE_TICKS`] ticks (0.5 s): each standing body
//!   lays [`crate::ScentConfig::deposit_per_s`] × its organic matter × 0.5 s on its node;
//!   each face's carrion [`crate::ScentConfig::carrion_per_s`] × its stock; every node decays
//!   with [`crate::ScentConfig::half_life_s`]; then a conservative mix with its neighbours,
//!   [`crate::ScentConfig::diffuse_fraction`] of the difference shared over
//!   `max(degree, 4)`; levels under [`crate::ScentConfig::threshold`] are dropped.
//! - **The receptor** reads its node's level and the gradient to its neighbours, per metre
//!   in the ring's `x` and `z` ([`ScentReading`]). A body off the ground reads nothing.
//! - **Nectar is a plume in the air** (NECTAR-SCENT, [`nectar_plume`]), not a deposit: a
//!   flower gives off its scent at its bloom, so a flyer smells it by 3D distance from the
//!   flower head — strongest at the flower, at the flower's height — not from the soil
//!   under the plant. Its kernel is this field's own steady state around one source on
//!   open ground ([`plume_kernel`]), so it reaches as far as a ground scent of the same
//!   strength; a source counts only on a clear line through the air. No wind (the world
//!   has none). Until 2026-10-08 nectar was laid on the face graph at the plant's rooting
//!   face, and bellwings led by it circled the roots a metre off the flower.
//!
//! **Cost**: the nodes are few (a 0.5 m cell holds 16 columns at 0.125 m), every node is
//! worked each update, four channels: about `4 × nodes × degree` adds per half second. The
//! graph is rebuilt only when the terrain or the cell and step knobs change.
//!
//! **Calibration of the smell floors** ([`SMELL_AT_2_M`] …): the steady level of the grazer
//! channel at a distance from one still adult frondgrazer (organic 2.25) on open flat ground
//! 6 m deep at the shipped knobs, read off a settled field (the `smell_floors_follow_the_field`
//! study in the tests prints them). A herd lays more, so it is smelled farther: by design. A floor is a lineage's knob; these say what the review's
//! metres mean in the field's units, not a law.

use std::sync::Arc;

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::FloraView;
use serde::{Deserialize, Serialize};

use crate::{Animal, FaunaConfig, MobilityMode};

/// A scent channel: what lays it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScentChannel {
    /// The small ground prey: littershredder, capgnawer, ripple-snail, a grounded bellwing.
    SmallPrey,
    /// The grazers and browsers: frondgrazer, loftstrider, seedporter.
    Grazer,
    /// The hunters: lanternjaw, Veilhound.
    Hunter,
    /// Carrion on the ground.
    Carrion,
    /// **Nectar** (S3): the bellwing's floral smell. Never laid on the field: it is a plume
    /// in the air around each flower ([`nectar_plume`], NECTAR-SCENT); the slot stays empty.
    Nectar,
}

impl ScentChannel {
    pub const COUNT: usize = 5;
    pub const ALL: [ScentChannel; 5] =
        [ScentChannel::SmallPrey, ScentChannel::Grazer, ScentChannel::Hunter, ScentChannel::Carrion, ScentChannel::Nectar];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            ScentChannel::SmallPrey => "small-prey",
            ScentChannel::Grazer => "grazer",
            ScentChannel::Hunter => "hunter",
            ScentChannel::Carrion => "carrion",
            ScentChannel::Nectar => "nectar",
        }
    }
}

/// The smell floors' calibration (module note): the grazer channel's steady level this many
/// metres from a still adult frondgrazer (organic 2.25) on open flat ground 6 m deep at the
/// shipped scent knobs (the study read 0.90, 0.69, 0.42 and 0.13 at 2, 2.5, 3.5 and 6 m;
/// 3.7 on its own node).
pub const SMELL_AT_2_M: f64 = 0.9;
pub const SMELL_AT_2_5_M: f64 = 0.69;
pub const SMELL_AT_3_5_M: f64 = 0.42;
pub const SMELL_AT_6_M: f64 = 0.13;

/// One channel's reading where a body stands: the level, and its gradient per metre along
/// the ring's `x` and `z`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScentReading {
    pub level: f64,
    pub gx: f64,
    pub gz: f64,
}

/// The surface graph the field lives on (module note). Pure geometry.
#[derive(Clone, Debug, Default)]
struct ScentGraph {
    version: u64,
    width: u32,
    height: u32,
    depth: u32,
    /// Columns per cell side, and the step in voxels: the knobs the graph was built with.
    block: u32,
    step: u32,
    /// Per column `z * width + x`: its faces `col_start[k]..col_start[k + 1]`, ascending in
    /// `y`, each with its node.
    col_start: Vec<u32>,
    face_y: Vec<u32>,
    face_node: Vec<u32>,
    /// Per node: a face on it `(x, y, z)` (to carry values across a rebuild), and its
    /// neighbours `nb_start[n]..nb_start[n + 1]` with their weight and the ring delta to
    /// their centre, metres.
    rep: Vec<(u32, u32, u32)>,
    nb_start: Vec<u32>,
    nb: Vec<u32>,
    nb_w: Vec<f64>,
    nb_d: Vec<(f64, f64)>,
}

impl ScentGraph {
    fn current(&self, view: &VoxelView<'_>, block: u32, step: u32) -> bool {
        let c = view.config;
        self.version == view.terrain_version
            && (self.width, self.height, self.depth) == (c.width, c.height, c.depth)
            && (self.block, self.step) == (block, step)
            && !self.col_start.is_empty()
    }

    fn column(&self, x: i64, z: u32) -> usize {
        z as usize * self.width as usize + x.rem_euclid(i64::from(self.width.max(1))) as usize
    }

    /// The node of the face `(x, y, z)`, if that is a face.
    fn node(&self, x: i64, y: u32, z: u32) -> Option<u32> {
        if z >= self.depth || self.width == 0 {
            return None;
        }
        let k = self.column(x, z);
        let (a, b) = (self.col_start[k] as usize, self.col_start[k + 1] as usize);
        (a..b).find(|&f| self.face_y[f] == y).map(|f| self.face_node[f])
    }

    fn nodes(&self) -> usize {
        self.rep.len()
    }

    fn build(view: &VoxelView<'_>, block: u32, step: u32) -> ScentGraph {
        let c = view.config;
        let (w, d) = (c.width, c.depth);
        let block = block.max(1);
        // 1. The faces, column by column.
        let mut col_start: Vec<u32> = Vec::with_capacity(w as usize * d as usize + 1);
        let mut face_y: Vec<u32> = Vec::new();
        let mut face_col: Vec<(u32, u32)> = Vec::new();
        for z in 0..d {
            for x in 0..i64::from(w) {
                col_start.push(face_y.len() as u32);
                let mut from = 0;
                while let Some(y) = view.lowest_support_in(x, z, from, c.height) {
                    face_y.push(y);
                    face_col.push((x as u32, z));
                    from = y + 1;
                }
            }
        }
        col_start.push(face_y.len() as u32);
        let n_faces = face_y.len();
        let col = |x: i64, z: u32| z as usize * w as usize + x.rem_euclid(i64::from(w)) as usize;
        // 2. The fine links: to the `+x` and `+z` neighbour columns, within the step, with
        //    open air over the lower face up to the higher one's top.
        let mut links: Vec<(u32, u32)> = Vec::new();
        for f in 0..n_faces {
            let (x, z) = face_col[f];
            let y = face_y[f];
            for (dx, dz) in [(1i64, 0u32), (0, 1)] {
                let nz = z + dz;
                if nz >= d || (dx == 1 && w < 2) {
                    continue;
                }
                let nx = i64::from(x) + dx;
                let k = col(nx, nz);
                for g in col_start[k] as usize..col_start[k + 1] as usize {
                    let gy = face_y[g];
                    if gy.abs_diff(y) > step {
                        continue;
                    }
                    let (lo, hi) = (y.min(gy), y.max(gy));
                    let (lx, lz) = if y <= gy { (i64::from(x), z) } else { (nx, nz) };
                    if lo < hi && view.solid_in(lx, lz, lo + 1, hi + 1) {
                        continue;
                    }
                    links.push((f as u32, g as u32));
                }
            }
        }
        // 3. One node per connected set of faces within a cell.
        let mut parent: Vec<u32> = (0..n_faces as u32).collect();
        fn find(p: &mut [u32], mut a: u32) -> u32 {
            while p[a as usize] != a {
                p[a as usize] = p[p[a as usize] as usize];
                a = p[a as usize];
            }
            a
        }
        let cell = |f: usize| (face_col[f].0 / block, face_col[f].1 / block);
        for &(f, g) in &links {
            if cell(f as usize) == cell(g as usize) {
                let (a, b) = (find(&mut parent, f), find(&mut parent, g));
                if a != b {
                    parent[a.max(b) as usize] = a.min(b);
                }
            }
        }
        let mut face_node = vec![u32::MAX; n_faces];
        let mut rep: Vec<(u32, u32, u32)> = Vec::new();
        let mut sum: Vec<(f64, f64, f64)> = Vec::new();
        let v = c.voxel_m;
        for f in 0..n_faces {
            let r = find(&mut parent, f as u32) as usize;
            if face_node[r] == u32::MAX {
                face_node[r] = rep.len() as u32;
                rep.push((face_col[r].0, face_y[r], face_col[r].1));
                sum.push((0.0, 0.0, 0.0));
            }
            let n = face_node[r];
            face_node[f] = n;
            let s = &mut sum[n as usize];
            s.0 += (f64::from(face_col[f].0) + 0.5) * v;
            s.1 += (f64::from(face_col[f].1) + 0.5) * v;
            s.2 += 1.0;
        }
        let centre: Vec<(f64, f64)> = sum.iter().map(|s| (s.0 / s.2, s.1 / s.2)).collect();
        // 4. Node links: every fine link across cells, once.
        let mut pairs: Vec<(u32, u32)> = links
            .iter()
            .map(|&(f, g)| (face_node[f as usize], face_node[g as usize]))
            .filter(|(a, b)| a != b)
            .flat_map(|(a, b)| [(a, b), (b, a)])
            .collect();
        pairs.sort_unstable();
        pairs.dedup();
        let nodes = rep.len();
        let mut nb_start = vec![0u32; nodes + 1];
        for &(a, _) in &pairs {
            nb_start[a as usize + 1] += 1;
        }
        for k in 0..nodes {
            nb_start[k + 1] += nb_start[k];
        }
        let deg = |n: u32| (nb_start[n as usize + 1] - nb_start[n as usize]) as usize;
        let ring = f64::from(w) * v;
        let nb: Vec<u32> = pairs.iter().map(|&(_, b)| b).collect();
        let nb_w: Vec<f64> = pairs.iter().map(|&(a, b)| 1.0 / deg(a).max(deg(b)).max(4) as f64).collect();
        let nb_d: Vec<(f64, f64)> = pairs
            .iter()
            .map(|&(a, b)| {
                let (pa, pb) = (centre[a as usize], centre[b as usize]);
                crate::pose::wrapped_delta(pa.0, pa.1, pb.0, pb.1, ring)
            })
            .collect();
        ScentGraph {
            version: view.terrain_version,
            width: w,
            height: c.height,
            depth: d,
            block,
            step,
            col_start,
            face_y,
            face_node,
            rep,
            nb_start,
            nb,
            nb_w,
            nb_d,
        }
    }
}

/// **The scent field** (module note): one level per node and channel.
#[derive(Clone, Debug, Default)]
pub struct ScentField {
    graph: Arc<ScentGraph>,
    value: [Vec<f64>; ScentChannel::COUNT],
    emit: Vec<f64>,
    next: Vec<f64>,
}

/// The cell and step knobs in voxels.
fn grid_knobs(config: &FaunaConfig, voxel_m: f64) -> (u32, u32) {
    let s = config.scent;
    let cols = |m: f64, min: f64| if m.is_finite() && voxel_m > 0.0 { (m / voxel_m).round().max(min) as u32 } else { min as u32 };
    (cols(s.cell_m, 1.0), cols(s.step_m, 0.0))
}

/// The face a body lays scent on and reads it from: its standing face on the ground; none
/// off it (flying, perched, climbing, gliding, riding).
fn standing_face(a: &Animal) -> Option<(i64, u32, u32)> {
    (a.attached.is_none() && a.mobility.mode == MobilityMode::Ground && a.mobility.climb.is_none())
        .then_some((i64::from(a.site.x), a.site.y, a.site.z))
}

impl ScentField {
    fn ensure_graph(&mut self, view: &VoxelView<'_>, config: &FaunaConfig) {
        let (block, step) = grid_knobs(config, view.config.voxel_m);
        if self.graph.current(view, block, step) {
            return;
        }
        let graph = ScentGraph::build(view, block, step);
        // Carry each node's level from the node its representative face was on.
        let mut value: [Vec<f64>; ScentChannel::COUNT] = Default::default();
        for (ch, vals) in value.iter_mut().enumerate() {
            *vals = vec![0.0; graph.nodes()];
            if self.value[ch].is_empty() {
                continue;
            }
            for (n, &(x, y, z)) in graph.rep.iter().enumerate() {
                if let Some(m) = self.graph.node(i64::from(x), y, z) {
                    vals[n] = self.value[ch].get(m as usize).copied().unwrap_or(0.0);
                }
            }
        }
        self.value = value;
        self.emit = vec![0.0; graph.nodes()];
        self.next = vec![0.0; graph.nodes()];
        self.graph = Arc::new(graph);
    }

    /// One update (0.5 s of laying, decay and spread): the module note's order.
    pub(crate) fn update(&mut self, view: &VoxelView<'_>, fv: &FloraView<'_>, animals: &[Animal], config: &FaunaConfig) {
        self.ensure_graph(view, config);
        let graph = Arc::clone(&self.graph);
        let s = config.scent;
        let dt = crate::senses::UPDATE_S;
        let decay = if s.half_life_s > 0.0 { (-(std::f64::consts::LN_2 * dt / s.half_life_s)).exp() } else { 0.0 };
        let f = if s.diffuse_fraction.is_finite() { s.diffuse_fraction.clamp(0.0, 1.0) } else { 0.0 };
        for ch in ScentChannel::ALL {
            self.emit.iter_mut().for_each(|e| *e = 0.0);
            let mut any = false;
            if ch == ScentChannel::Carrion {
                if s.carrion_per_s > 0.0 {
                    for g in fv.ground.iter().filter(|g| g.carrion > 0.0) {
                        if let Some(n) = graph.node(i64::from(g.site.x), g.site.y, g.site.z) {
                            self.emit[n as usize] += s.carrion_per_s * g.carrion * dt;
                            any = true;
                        }
                    }
                }
            } else if ch == ScentChannel::Nectar {
                // NECTAR-SCENT: nectar is not laid on the ground; a flower's scent is a plume
                // in the air around its bloom ([`nectar_plume`]).
                continue;
            } else if s.deposit_per_s > 0.0 {
                for a in animals {
                    let laid = a.founder.and_then(|fd| config.founder(fd).senses.scent);
                    if laid != Some(ch) {
                        continue;
                    }
                    let Some((x, y, z)) = standing_face(a) else { continue };
                    if let Some(n) = graph.node(x, y, z) {
                        let m = a.organic();
                        if m.is_finite() && m > 0.0 {
                            self.emit[n as usize] += s.deposit_per_s * m * dt;
                            any = true;
                        }
                    }
                }
            }
            let vals = &mut self.value[ch.index()];
            if !any && vals.iter().all(|&u| u == 0.0) {
                continue;
            }
            for (u, e) in vals.iter_mut().zip(&self.emit) {
                *u = (*u + e) * decay;
            }
            for n in 0..graph.nodes() {
                let u = vals[n];
                let mut flux = 0.0;
                for k in graph.nb_start[n] as usize..graph.nb_start[n + 1] as usize {
                    flux += graph.nb_w[k] * (vals[graph.nb[k] as usize] - u);
                }
                let out = u + f * flux;
                self.next[n] = if out >= s.threshold { out } else { 0.0 };
            }
            std::mem::swap(vals, &mut self.next);
        }
    }

    /// A node's reading: its level and its gradient over its neighbours.
    fn reading(&self, ch: ScentChannel, n: usize) -> ScentReading {
        let g = &self.graph;
        let vals = &self.value[ch.index()];
        let Some(&u) = vals.get(n) else {
            return ScentReading::default();
        };
        let (a, b) = (g.nb_start[n] as usize, g.nb_start[n + 1] as usize);
        let deg = b - a;
        let (mut gx, mut gz) = (0.0, 0.0);
        for k in a..b {
            let (dx, dz) = g.nb_d[k];
            let d2 = dx * dx + dz * dz;
            if d2 > 0.0 {
                let du = vals[g.nb[k] as usize] - u;
                gx += du * dx / d2;
                gz += du * dz / d2;
            }
        }
        if deg > 0 {
            gx *= 2.0 / deg as f64;
            gz *= 2.0 / deg as f64;
        }
        ScentReading { level: u, gx, gz }
    }

    /// What a body reads where it stands, every channel; nothing off the ground or before
    /// the field has a graph for this terrain.
    pub(crate) fn read(&self, view: &VoxelView<'_>, a: &Animal) -> [ScentReading; ScentChannel::COUNT] {
        let mut out = [ScentReading::default(); ScentChannel::COUNT];
        let Some((x, y, z)) = standing_face(a) else {
            return out;
        };
        self.read_face(view, x, y, z, &mut out);
        out
    }

    /// The reading on the face `(x, y, z)`.
    pub(crate) fn read_face(&self, view: &VoxelView<'_>, x: i64, y: u32, z: u32, out: &mut [ScentReading; ScentChannel::COUNT]) {
        if self.graph.version != view.terrain_version || self.graph.col_start.is_empty() {
            return;
        }
        let Some(n) = self.graph.node(x, y, z) else { return };
        for ch in ScentChannel::ALL {
            out[ch.index()] = self.reading(ch, n as usize);
        }
    }

    /// The level of `ch` on the face `(x, y, z)` (0 off the field).
    pub(crate) fn level(&self, view: &VoxelView<'_>, ch: ScentChannel, x: i64, y: u32, z: u32) -> f64 {
        let mut out = [ScentReading::default(); ScentChannel::COUNT];
        self.read_face(view, x, y, z, &mut out);
        out[ch.index()].level
    }
}

/// **The nectar plume** (NECTAR-SCENT) where a flyer smells it: the level, its gradient per
/// metre in the ring's `x`, the height `y` and `z` (it rises toward the bloom), and the
/// source that gives the most of it, `(x, y, z)` metres.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Plume {
    pub level: f64,
    pub gx: f64,
    pub gy: f64,
    pub gz: f64,
    pub source: Option<(f64, f64, f64)>,
}

/// The modified Bessel function `K₀(x)`, `x > 0` (Abramowitz and Stegun 9.8.1, 9.8.5,
/// 9.8.6; error under 1e-7): the field's steady state around one source in the plane.
pub(crate) fn bessel_k0(x: f64) -> f64 {
    if x <= 2.0 {
        let t = (x / 3.75) * (x / 3.75);
        let i0 = 1.0 + t * (3.5156229 + t * (3.0899424 + t * (1.2067492 + t * (0.2659732 + t * (0.0360768 + t * 0.0045813)))));
        let u = 0.25 * x * x;
        -(0.5 * x).ln() * i0
            + (-0.57721566 + u * (0.42278420 + u * (0.23069756 + u * (0.03488590 + u * (0.00262698 + u * (0.00010750 + u * 0.0000074))))))
    } else {
        let u = 2.0 / x;
        (-x).exp() / x.sqrt()
            * (1.25331414 + u * (-0.07832358 + u * (0.02189568 + u * (-0.01062446 + u * (0.00587872 + u * (-0.00251540 + u * 0.00053208))))))
    }
}

/// **The plume's kernel** (NECTAR-SCENT): the scent field's own steady state around one
/// source of unit rate on open flat ground, `amp · K₀(r / L)`, taken by 3D distance `r` from
/// the source. From the field's knobs and nothing else: the spread is
/// `D = diffuse_fraction · cell² / (4 · UPDATE_S)` m²/s (the update's mix over four
/// neighbours), the decay `ln 2 / half_life_s`, so `L = √(D · half_life_s / ln 2)` and
/// `amp = cell² / (2π D)`. A flower's smell therefore reaches as far through the air as a
/// body's of the same strength does over open ground. `None` when nothing spreads or lasts.
pub(crate) fn plume_kernel(config: &FaunaConfig, voxel_m: f64) -> Option<(f64, f64)> {
    let s = config.scent;
    let (block, _) = grid_knobs(config, voxel_m);
    let cell = f64::from(block) * voxel_m;
    let f = if s.diffuse_fraction.is_finite() { s.diffuse_fraction.clamp(0.0, 1.0) } else { 0.0 };
    let d = f * cell * cell / (4.0 * crate::senses::UPDATE_S);
    if !(d > 0.0) || !(s.half_life_s > 0.0) || !s.half_life_s.is_finite() {
        return None;
    }
    let l = (d * s.half_life_s / std::f64::consts::LN_2).sqrt();
    Some((cell * cell / (std::f64::consts::TAU * d), l))
}

/// The point sources of nectar scent and their rates, scent units a second: every flower
/// holding nectar at its bloom — a bloomcrown's flower head (the top of its top foliage
/// layer, over its site), a latticevine face in flower (the voxel in front of it) — by the
/// nectar it holds (`nectar_per_s` × its nectar over `nectar_full`), and every
/// lit lure but `except`'s (a lanternjaw does not smell its own) at its lure point, by
/// `lure_scent` × its lit fraction × a full flower's (the mimic holds none). The light
/// layer's beacons and lures: the same flowers the eye sees.
#[cfg(test)]
pub(crate) fn nectar_sources(fauna: &crate::Fauna, view: &VoxelView<'_>, except: Option<u64>) -> Vec<((f64, f64, f64), f64)> {
    let s = fauna.config.scent;
    if !(s.nectar_per_s > 0.0) {
        return Vec::new();
    }
    let ls = fauna.config.light.lure_scent;
    let flowers = fauna.beacons.flowers_by_scent().map(|(b, share)| ((b.x, b.y_m, b.z), s.nectar_per_s * share));
    let lures = crate::light::lures(fauna, view)
        .into_iter()
        .filter(|b| Some(b.body) != except && ls > 0.0 && b.power > 0.0)
        .map(|b| ((b.x, b.y_m, b.z), s.nectar_per_s * ls * b.power.clamp(0.0, 1.0)));
    flowers.chain(lures).collect()
}

/// Read the shared static flower tree, while lures retain their current poses.
/// Keeping the strongest flowers is exact: at most MAX_PLUME_RAYS sources can
/// affect the reading. If their sum misses the floor, refine the tree's aggregate
/// emission bounds (many individually weak flowers can collectively clear it).
pub(crate) fn nectar_plume_indexed(
    fauna: &crate::Fauna,
    view: &VoxelView<'_>,
    p: (f64, f64, f64),
    observer: u64,
    floor: f64,
) -> Plume {
    let c = &fauna.config;
    let v = view.config.voxel_m;
    let Some((amp, spread)) = plume_kernel(c, v) else {
        return Plume::default();
    };
    if !(c.scent.nectar_per_s > 0.0) {
        return Plume::default();
    }
    let ring = f64::from(view.config.width) * v;
    let indices = fauna.beacons.strongest_flowers(
        p,
        ring,
        v,
        amp,
        spread,
        c.scent.nectar_per_s,
        c.scent.threshold.max(0.0),
        MAX_PLUME_RAYS,
    );
    let lures: Vec<_> = crate::light::lures(fauna, view)
        .into_iter()
        .filter(|b| b.body != observer && c.light.lure_scent > 0.0 && b.power > 0.0)
        .map(|b| {
            (
                (b.x, b.y_m, b.z),
                c.scent.nectar_per_s * c.light.lure_scent * b.power.clamp(0.0, 1.0),
            )
        })
        .collect();
    let selected: Vec<_> = indices
        .into_iter()
        .map(|i| fauna.beacons.flower_source(i, c.scent.nectar_per_s))
        .chain(lures.iter().copied())
        .collect();
    let level = |(src, q): ((f64, f64, f64), f64)| {
        let (dx, dz) = crate::pose::wrapped_delta(p.0, p.2, src.0, src.2, ring);
        let dy = src.1 - p.1;
        q * amp * bessel_k0((dx * dx + dy * dy + dz * dz).sqrt().max(v) / spread)
    };
    let selected_total: f64 = selected
        .iter()
        .copied()
        .map(level)
        .filter(|&s| s > c.scent.threshold.max(0.0))
        .sum();
    if selected_total < floor {
        let lure_total: f64 = lures.iter().copied().map(level)
            .filter(|&s| s > c.scent.threshold.max(0.0)).sum();
        if !fauna.beacons.flowers_reach_floor(p, ring, v, amp, spread,
            c.scent.nectar_per_s, c.scent.threshold.max(0.0), floor - lure_total) {
            return Plume::default();
        }
    }
    // The complete pre-ray sum meets the floor; the retained strongest sources
    // now undergo exactly the same ranking, ray tests and gradient calculation.
    nectar_plume_from(view, c, selected, p, observer, 0.0)
}

/// At most this many sources, strongest first, are tested for a clear line and summed per
/// reading (a cost bound, not ecology, as the light layer's `MAX_LIGHT_RAYS`): the plume falls
/// off steeply, so the nearest flowers make the reading.
const MAX_PLUME_RAYS: usize = 6;

/// **A flyer's nectar smell** (NECTAR-SCENT): the plume at the point `p` (metres), summed
/// over `sources` ([`nectar_sources`]) by the [`plume_kernel`] of each one's 3D distance,
/// each only on a clear line through the air (no rock, no edge, no water between: scent
/// never crosses solid rock). Nearer than one voxel a source reads as at one voxel, with no
/// gradient (the smeller is at the bloom). Sources under the field's `threshold` are
/// dropped, and when the sum before the line tests is under `floor` nothing is smelled (a
/// cost bound: occlusion only lowers it); of the rest, the [`MAX_PLUME_RAYS`] strongest are
/// read.
#[cfg(test)]
pub(crate) fn nectar_plume(
    view: &VoxelView<'_>,
    config: &FaunaConfig,
    sources: &[((f64, f64, f64), f64)],
    p: (f64, f64, f64),
    observer: u64,
    floor: f64,
) -> Plume {
    nectar_plume_from(view, config, sources.iter().copied(), p, observer, floor)
}

fn nectar_plume_from(
    view: &VoxelView<'_>,
    config: &FaunaConfig,
    sources: impl IntoIterator<Item = ((f64, f64, f64), f64)>,
    p: (f64, f64, f64),
    observer: u64,
    floor: f64,
) -> Plume {
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let Some((amp, l)) = plume_kernel(config, v) else { return Plume::default() };
    let threshold = config.scent.threshold.max(0.0);
    let mut near: Vec<(f64, f64, (f64, f64, f64), (f64, f64, f64))> = Vec::new();
    let mut total = 0.0;
    for (src, q) in sources {
        let (dx, dz) = crate::pose::wrapped_delta(p.0, p.2, src.0, src.2, ring);
        let dy = src.1 - p.1;
        let r = (dx * dx + dy * dy + dz * dz).sqrt();
        let x = r.max(v) / l;
        let level = q * amp * bessel_k0(x);
        if !(level > threshold) {
            continue;
        }
        total += level;
        near.push((level, r, (dx, dy, dz), src));
    }
    if total < floor || near.is_empty() {
        return Plume::default();
    }
    near.sort_by(|a, b| b.0.total_cmp(&a.0));
    near.truncate(MAX_PLUME_RAYS);
    let mut out = Plume::default();
    let mut best = 0.0;
    for (level, r, (dx, dy, dz), src) in near {
        let clear = r <= v || {
            let dir = (dx / r, dy / r, dz / r);
            crate::senses::ray_first_hit(view, &crate::perception::TerrainOnly, observer, p, dir, r).is_none_or(|(h, _)| h >= r - v)
        };
        if !clear {
            continue;
        }
        out.level += level;
        if r > v {
            // d level / dr = q · amp · K₀'(r / L) / L < 0, so the gradient points from `p` to
            // the source: its size along the unit vector toward it.
            let x = r / l;
            let e = 1e-6 * x.max(1.0);
            let slope = level / bessel_k0(x) * (bessel_k0((x - e).max(1e-12)) - bessel_k0(x + e)) / (2.0 * e) / l;
            out.gx += slope * dx / r;
            out.gy += slope * dy / r;
            out.gz += slope * dz / r;
        }
        if level > best {
            best = level;
            out.source = Some(src);
        }
    }
    out
}
