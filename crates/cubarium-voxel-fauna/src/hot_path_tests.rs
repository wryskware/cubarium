//! Hot-path geometry (`design/handoffs/voxel-hot-path-geometry-2026-09-24.md`), written
//! before the implementation:
//!
//! - item 4: the mouth's reach read through the flora's cached crowns equals the scan it
//!   replaces — `reachable_layers` as it stood at `main` a31ecdc, kept verbatim below as
//!   the oracle — for every stand of a seeded strip, every mouth column set around it and
//!   every band, stripped and nibbled crowns included;
//! - item 5: the dense detritus field reads exactly what the hash-map field it replaces
//!   reads — the map field is kept verbatim below — at every support face and between
//!   them, update after update, through deposits, meals and a terrain edit;
//! - item 6: the light memo, warmed whole for a frozen episode or filled lazily, answers
//!   every support face's sky visibility exactly as the direct call does, a ledge and the
//!   floor under it in one column included, and follows a terrain edit.

use rustc_hash::{FxHashMap, FxHashSet};

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, VoxelView, World};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraConfig, FloraView, Site,
    Species as Plant, layers_of,
};

use crate::Pose;
use crate::body::M_EMIT;
use crate::senses::{DIFFUSE_FRACTION, DetritusField, HALF_LIFE_S, Senses, THRESHOLD, UPDATE_S};

const V: f64 = 0.25;
const W: u32 = 32;
const H: u32 = 16;
const D: u32 = 8;

/// A small deterministic generator for the fixtures.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn unit(&mut self) -> f64 {
        self.next() as f64 / f64::from(1u32 << 31)
    }
}

fn soil(world: &mut World, x: i64, y: u32, z: u32) {
    world.apply(WorldCommand::SetMaterial {
        x,
        y,
        z,
        material: Material::Soil,
    });
}

/// Rolling ground 1..=5 high, a wall splitting the floor, a few two-column ledges over
/// it (a ledge and the floor under it: two support faces in one column), stands of every
/// species at several sizes — some stripped to their grazing floor, some nibbled — and
/// ground pools of litter and carrion.
fn strip(seed: u64) -> (World, Flora, Vec<u32>) {
    let mut rng = Rng(seed);
    let mut world = World::empty(VoxelConfig {
        width: W,
        height: H,
        depth: D,
        voxel_m: V,
        seed,
        ..VoxelConfig::default()
    });
    let mut top = vec![0u32; (W * D) as usize];
    for z in 0..D {
        for x in 0..W {
            let h = 1 + rng.below(5) as u32;
            top[(z * W + x) as usize] = h;
            for y in 1..=h {
                soil(&mut world, i64::from(x), y, z);
            }
        }
    }
    // A wall across the strip at x = 20.
    for z in 0..D {
        for y in 1..=9 {
            soil(&mut world, 20, y, z);
        }
        top[(z * W + 20) as usize] = 9;
    }
    for _ in 0..5 {
        let (x, z) = (rng.below(u64::from(W - 1)) as i64, rng.below(u64::from(D)) as u32);
        if x == 19 || x == 20 {
            continue;
        }
        let y = 10 + rng.below(3) as u32;
        soil(&mut world, x, y, z);
        soil(&mut world, x + 1, y, z);
    }
    let mut flora = Flora::new(FloraConfig::for_voxel_size(V));
    for _ in 0..60 {
        let (x, z) = (rng.below(u64::from(W)) as u32, rng.below(u64::from(D)) as u32);
        let sp = Plant::ALL[rng.below(Plant::ALL.len() as u64) as usize];
        let wood = (0.05 + 0.95 * rng.unit()) * flora.config().species(sp).wood_max;
        let _ = flora.apply(
            &world,
            FloraCommand::Seed {
                x: i64::from(x),
                z,
                species: sp,
                wood,
            },
        );
    }
    let sites: Vec<Site> = flora.view().stands.iter().map(|s| s.site).collect();
    assert!(sites.len() >= 25, "the fixture seeded too few stands");
    for (i, &site) in sites.iter().enumerate() {
        match i % 4 {
            0 => {
                let _ = flora.take_foliage(site, 1e9); // stripped to the grazing floor
            }
            1 => {
                let _ = flora.take_foliage(site, 0.01); // the lowest layer nibbled
            }
            _ => {}
        }
    }
    for i in 0..18 {
        let (x, z) = (rng.below(u64::from(W)) as u32, rng.below(u64::from(D)) as u32);
        let site = Site {
            x,
            y: top[(z * W + x) as usize],
            z,
        };
        let organic = [0.002, 0.02, 0.05, 0.3][i % 4];
        let kind = if i % 3 == 0 {
            DepositKind::Carrion
        } else {
            DepositKind::Litter
        };
        flora.deposit(
            site,
            Deposit {
                kind,
                organic,
                mineral: 0.02 * organic,
                energy: 2.0 * organic,
            },
        );
    }
    (world, flora, top)
}

/// Every support face of the world, column-major.
fn faces(view: &VoxelView<'_>) -> Vec<Site> {
    let c = view.config;
    let mut out = Vec::new();
    for z in 0..c.depth {
        for x in 0..c.width {
            for y in view.supports_in_column(i64::from(x), z) {
                out.push(Site { x, y, z });
            }
        }
    }
    out
}

// ------------------------------------------------------------- item 4: the mouth's reach

/// `reachable_layers` at `main` a31ecdc, verbatim but for the name and its reads going
/// straight to the species config (`crown_voxels`) and [`layers_of`]: no cache anywhere.
fn reach_reference(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    stand: &cubarium_voxel_flora::Stand,
    cols: &[(i64, u32)],
    layers: &std::ops::RangeInclusive<i64>,
) -> Vec<(usize, f64)> {
    let mut out = Vec::new();
    let sc = fv.config.species(stand.species);
    let lowest = i64::from(stand.site.y) + 1;
    let highest =
        i64::from(stand.site.y) + i64::from(sc.crown_voxels(stand.wood, fv.config.voxel_m));
    if *layers.end() < lowest || *layers.start() > highest {
        return out;
    }
    let width = i64::from(view.config.width);
    let depth = i64::from(view.config.depth);
    for layer in layers_of(fv.config, stand, fv.config.voxel_m)
        .into_iter()
        .filter(|l| l.kind.bears_foliage())
    {
        let edible = layer.edible();
        if !(edible > 0.0) || !layers.contains(&layer.cell) {
            continue;
        }
        let span = layer.radius_v.floor() as i64;
        let r2 = layer.radius_v * layer.radius_v;
        let mut touching = false;
        'cells: for dz in -span..=span {
            for dx in -span..=span {
                if (dx * dx + dz * dz) as f64 > r2 {
                    continue;
                }
                let cx = (i64::from(stand.site.x) + dx).rem_euclid(width);
                let cz = i64::from(stand.site.z) + dz;
                if cz < 0 || cz >= depth {
                    continue;
                }
                if cols.contains(&(cx, cz as u32)) {
                    touching = true;
                    break 'cells;
                }
            }
        }
        if touching {
            out.push((layer.foliage_index.unwrap_or(0), edible));
        }
    }
    out
}

fn reach_bits(v: &[(usize, f64)]) -> Vec<(usize, u64)> {
    v.iter().map(|&(i, s)| (i, s.to_bits())).collect()
}

#[test]
fn the_mouths_reach_through_the_cache_is_the_uncached_scan() {
    for seed in [3, 17] {
        let (world, flora, _) = strip(seed);
        let view = world.view();
        let fv = flora.view();
        let (w, d) = (i64::from(W), D);
        let mut compared = 0usize;
        let mut touched = 0usize;
        for stand in fv.stands {
            assert!(
                fv.cached_crown(stand).is_some(),
                "the reach is read through a current cache entry"
            );
            let (sx, sz) = (i64::from(stand.site.x), i64::from(stand.site.z));
            let y = i64::from(stand.site.y);
            // Mouth column sets: every single column in a box around the stand (across
            // the seam, off the strip's ends, and unwrapped `x` a mouth never passes),
            // pairs, and a mouth's worth.
            let mut sets: Vec<Vec<(i64, u32)>> = Vec::new();
            for dz in -6i64..=6 {
                for dx in -6i64..=6 {
                    let z = sz + dz;
                    if z < 0 || z >= i64::from(d) {
                        continue;
                    }
                    let x = (sx + dx).rem_euclid(w);
                    sets.push(vec![(x, z as u32)]);
                    sets.push(vec![(x, z as u32), ((x + 1).rem_euclid(w), z as u32)]);
                }
            }
            sets.push(vec![(sx + w, sz as u32)]); // not wrapped: matches nothing
            sets.push(vec![(sx, d + 1)]); // off the strip
            let last = i64::from(d) - 1;
            let mouth: Vec<(i64, u32)> = (-1..=1)
                .flat_map(|dx| (0..3).map(move |dz| ((sx + dx).rem_euclid(w), (sz + dz).min(last) as u32)))
                .collect();
            sets.push(mouth);
            for set in &sets {
                for band in [
                    y + 1..=y + 1,
                    y + 1..=y + 2,
                    y..=y + 4,
                    y + 2..=y + 6,
                    y + 3..=y + 20,
                    0..=i64::from(H),
                    y + 30..=y + 31,
                ] {
                    let want = reach_reference(&fv, &view, stand, set, &band);
                    let got = crate::body::reachable_layers(&fv, &view, stand, set, &band);
                    assert_eq!(
                        reach_bits(&got),
                        reach_bits(&want),
                        "{:?} wood {} at {:?}, cols {set:?}, band {band:?}",
                        stand.species,
                        stand.wood,
                        stand.site
                    );
                    let sum: f64 = want.iter().map(|&(_, s)| s).sum();
                    let stock = crate::body::reachable_layer_stock(&fv, &view, stand, set, &band);
                    assert!(
                        stock == sum || (!(stock > 0.0) && !(sum > 0.0)),
                        "stock {stock} against {sum}"
                    );
                    compared += 1;
                    touched += usize::from(!want.is_empty());
                }
            }
        }
        assert!(compared > 10_000 && touched > 50, "{compared} compared, {touched} touching");
    }
}

// --------------------------------------------------------- item 5: the detritus field

/// The support-layer graph as it stood at `main` a31ecdc.
#[derive(Clone, Debug, Default)]
struct MapConnectivity {
    version: u64,
    width: u32,
    height: u32,
    depth: u32,
    nodes: Vec<usize>,
    neighbors: FxHashMap<usize, Vec<usize>>,
}

impl MapConnectivity {
    fn current(&self, view: &VoxelView<'_>) -> bool {
        let c = view.config;
        self.version == view.terrain_version
            && self.width == c.width
            && self.height == c.height
            && self.depth == c.depth
    }

    fn build(view: &VoxelView<'_>) -> MapConnectivity {
        let c = view.config;
        let mut is_node = vec![false; c.cells()];
        let mut nodes: Vec<usize> = Vec::new();
        for z in 0..c.depth {
            for x in 0..c.width as i64 {
                for y in view.supports_in_column(x, z) {
                    let cell = c.index(x, y, z);
                    nodes.push(cell);
                    is_node[cell] = true;
                }
            }
        }
        nodes.sort_unstable();
        let mut neighbors: FxHashMap<usize, Vec<usize>> =
            FxHashMap::with_capacity_and_hasher(nodes.len(), Default::default());
        for &cell in &nodes {
            let (x, y, z) = c.coords(cell);
            let mut nb: Vec<usize> = Vec::with_capacity(4);
            for (dx, dz) in [(-1i64, 0i64), (1i64, 0i64), (0, -1), (0, 1)] {
                let nz = i64::from(z) + dz;
                if nz < 0 || nz >= i64::from(c.depth) {
                    continue;
                }
                let ncell = c.index(i64::from(x) + dx, y, nz as u32);
                if is_node[ncell] {
                    nb.push(ncell);
                }
            }
            nb.sort_unstable();
            neighbors.insert(cell, nb);
        }
        MapConnectivity {
            version: view.terrain_version,
            width: c.width,
            height: c.height,
            depth: c.depth,
            nodes,
            neighbors,
        }
    }

    fn has(&self, cell: usize) -> bool {
        self.neighbors.contains_key(&cell)
    }
}

/// The hash-map detritus field as it stood at `main` a31ecdc.
#[derive(Clone, Debug, Default)]
struct MapDetritus {
    value: FxHashMap<usize, f64>,
    graph: MapConnectivity,
}

fn detritus_at(fv: &FloraView<'_>, site: Site) -> f64 {
    fv.ground_at(site).map_or(0.0, |g| g.litter + g.carrion)
}

fn decay_factor() -> f64 {
    (-(std::f64::consts::LN_2 * UPDATE_S / HALF_LIFE_S)).exp()
}

impl MapDetritus {
    fn ensure_graph(&mut self, view: &VoxelView<'_>) {
        if !self.graph.current(view) {
            let old_nodes: FxHashSet<usize> = self.graph.nodes.iter().copied().collect();
            self.graph = MapConnectivity::build(view);
            self.value.retain(|cell, _| self.graph.has(*cell));
            let _ = old_nodes;
        }
    }

    fn sample(&self, view: &VoxelView<'_>, pose: &Pose, standing_y: u32) -> Option<f64> {
        let c = view.config;
        let (cx, cz) = pose.column(c.voxel_m, c.depth)?;
        let wx = cx.rem_euclid(i64::from(c.width));
        if !view.is_support(wx, standing_y, cz) {
            return None;
        }
        if !self.graph.current(view) || !self.graph.has(c.index(wx, standing_y, cz)) {
            return None;
        }
        let v = c.voxel_m;
        let xf = pose.x / v - 0.5;
        let zf = pose.z / v - 0.5;
        let x0 = xf.floor() as i64;
        let z0 = zf.floor() as i64;
        let (dx, dz) = (xf - x0 as f64, zf - z0 as f64);
        let mut sum = 0.0;
        let mut wsum = 0.0;
        for (odx, odz, w) in [
            (0i64, 0i64, (1.0 - dx) * (1.0 - dz)),
            (1, 0, dx * (1.0 - dz)),
            (0, 1, (1.0 - dx) * dz),
            (1, 1, dx * dz),
        ] {
            let z = z0 + odz;
            if z < 0 || z >= i64::from(c.depth) {
                continue;
            }
            let cell = c.index(x0 + odx, standing_y, z as u32);
            if !self.graph.has(cell) {
                continue;
            }
            if let Some(&value) = self.value.get(&cell) {
                sum += w * value;
            }
            wsum += w;
        }
        if wsum <= 0.0 { None } else { Some(sum / wsum) }
    }

    fn update(&mut self, view: &VoxelView<'_>, fv: &FloraView<'_>) -> f64 {
        self.ensure_graph(view);
        let c = view.config;
        let graph = &self.graph;
        let mut sources: Vec<usize> = Vec::new();
        for g in fv
            .ground
            .iter()
            .filter(|g| g.litter > 0.0 || g.carrion > 0.0)
        {
            let cell = c.index(g.site.x as i64, g.site.y, g.site.z);
            if graph.has(cell) {
                sources.push(cell);
            }
        }
        sources.sort_unstable();
        sources.dedup();
        let mut work: Vec<usize> = self.value.keys().copied().collect();
        work.extend(sources.iter().copied());
        work.sort_unstable();
        work.dedup();
        let roots = work.clone();
        for cell in roots {
            if let Some(nb) = graph.neighbors.get(&cell) {
                work.extend_from_slice(nb);
            }
        }
        work.sort_unstable();
        work.dedup();
        let decay = decay_factor();
        let mut prime: Vec<(usize, f64)> = Vec::with_capacity(work.len());
        for &cell in &work {
            let old = self.value.get(&cell).copied().unwrap_or(0.0);
            let emit = if sources.binary_search(&cell).is_ok() {
                let (x, y, z) = c.coords(cell);
                let stock = detritus_at(fv, Site { x: x as u32, y, z });
                (stock / M_EMIT).min(1.0) * UPDATE_S
            } else {
                0.0
            };
            prime.push((cell, (old + emit) * decay));
        }
        let by_cell: FxHashMap<usize, f64> = prime.iter().copied().collect();
        let mut max_change = 0.0f64;
        let mut next: FxHashMap<usize, f64> =
            FxHashMap::with_capacity_and_hasher(work.len(), Default::default());
        for &(cell, p) in &prime {
            let (x, y, z) = c.coords(cell);
            let mut total = 0.0;
            for (dx, dz) in [(-1i64, 0i64), (1i64, 0i64), (0, -1), (0, 1)] {
                let nz = i64::from(z) + dz;
                let nb_value = if nz < 0 || nz >= i64::from(c.depth) {
                    p
                } else {
                    let ncell = c.index(i64::from(x) + dx, y, nz as u32);
                    if graph
                        .neighbors
                        .get(&cell)
                        .is_some_and(|nb| nb.contains(&ncell))
                    {
                        by_cell.get(&ncell).copied().unwrap_or(0.0)
                    } else {
                        p
                    }
                };
                total += nb_value;
            }
            let mean = total / 4.0;
            let updated = (1.0 - DIFFUSE_FRACTION) * p + DIFFUSE_FRACTION * mean;
            let old = self.value.get(&cell).copied().unwrap_or(0.0);
            max_change = max_change.max((updated - old).abs());
            if updated >= THRESHOLD {
                next.insert(cell, updated);
            }
        }
        self.value = next;
        max_change
    }
}

/// Both fields' readings at every support face — its value, and samples at its centre,
/// near its corners and between it and its neighbours — agree exactly.
fn same_readings(dense: &DetritusField, map: &MapDetritus, view: &VoxelView<'_>, what: &str) -> usize {
    let c = view.config;
    let mut nonzero = 0;
    for face in faces(view) {
        let cell = c.index(i64::from(face.x), face.y, face.z);
        let want = map.value.get(&cell).copied().unwrap_or(0.0);
        assert_eq!(dense.value_at(cell), want, "{what}: value at {face:?}");
        nonzero += usize::from(want > 0.0);
        for (fx, fz) in [(0.5, 0.5), (0.05, 0.05), (0.95, 0.9), (0.2, 0.7), (0.99, 0.01)] {
            let pose = Pose {
                x: (f64::from(face.x) + fx) * V,
                z: (f64::from(face.z) + fz) * V,
                heading_rad: 0.0,
            };
            assert_eq!(
                dense.sample(view, &pose, face.y),
                map.sample(view, &pose, face.y),
                "{what}: sample at {pose:?} on layer {}",
                face.y
            );
        }
    }
    nonzero
}

#[test]
fn the_dense_detritus_field_reads_what_the_map_field_reads() {
    for seed in [5, 23] {
        let (mut world, mut flora, top) = strip(seed);
        let mut dense = DetritusField::default();
        let mut map = MapDetritus::default();
        let mut spread = 0;
        for k in 0..40 {
            let (view, fv) = (world.view(), flora.view());
            let a = dense.update(&view, &fv);
            let b = map.update(&view, &fv);
            assert_eq!(a, b, "seed {seed}, update {k}: max change");
            spread = spread.max(same_readings(&dense, &map, &view, &format!("seed {seed}, update {k}")));
            // Between updates the sources move: a meal, a new pool, and once the terrain.
            match k {
                8 => {
                    let site = flora
                        .view()
                        .ground
                        .iter()
                        .find(|g| g.litter > 0.0)
                        .map(|g| g.site)
                        .expect("a litter pool");
                    let _ = flora.take_litter(site, 1e9);
                }
                14 => {
                    let (x, z) = (3u32, 2u32);
                    flora.deposit(
                        Site {
                            x,
                            y: top[(z * W + x) as usize],
                            z,
                        },
                        Deposit {
                            kind: DepositKind::Litter,
                            organic: 0.4,
                            mineral: 0.008,
                            energy: 0.8,
                        },
                    );
                }
                22 => {
                    // A block raised on the floor: faces vanish, a new one appears, the
                    // graph is rebuilt and the values on surviving faces are kept.
                    for x in 5..=8 {
                        let z = 3u32;
                        let y = top[(z * W + x) as usize] + 1;
                        soil(&mut world, i64::from(x), y, z);
                    }
                }
                _ => {}
            }
        }
        assert!(spread > 20, "the cue spread over the floor ({spread} faces)");
    }
}

// ------------------------------------------------------------- item 6: the light memo

fn check_light(senses: &mut Senses, view: &VoxelView<'_>, what: &str) {
    let w = i64::from(view.config.width);
    let all = faces(view);
    assert!(!all.is_empty());
    // Twice over, in two orders, so a column's ledge and floor are read alternately.
    for pass in 0..2 {
        let order: Vec<&Site> = if pass == 0 {
            all.iter().collect()
        } else {
            all.iter().rev().collect()
        };
        for face in order {
            let (x, y, z) = (i64::from(face.x), face.y, face.z);
            let want = view.sky_visibility(x, y, z);
            for xx in [x, x + w, x - w] {
                assert_eq!(
                    senses.held_sky(view, xx, y, z),
                    Some(want),
                    "{what}: sky at ({xx}, {y}, {z})"
                );
            }
        }
    }
}

#[test]
fn the_light_memo_is_the_direct_sky_warm_or_lazy() {
    let (mut world, _, _) = strip(9);
    let view = world.view();
    let ledges = faces(&view)
        .windows(2)
        .filter(|p| (p[0].x, p[0].z) == (p[1].x, p[1].z))
        .count();
    assert!(ledges > 0, "some column holds a ledge over a floor");

    let mut warm = Senses::new();
    warm.hold_light();
    warm.warm_light(&view);
    check_light(&mut warm, &view, "warm");
    // A clone carries the warm memo: every episode of a frozen fixture starts warm.
    let mut episode = warm.clone();
    check_light(&mut episode, &view, "cloned");

    let mut lazy = Senses::new();
    lazy.hold_light();
    check_light(&mut lazy, &view, "lazy");

    // Not held: nothing is memoed and the caller computes the sky itself.
    let mut free = Senses::new();
    assert_eq!(free.held_sky(&view, 1, 2, 1), None);

    // A terrain edit moves the terrain version, and the memo answers for the new terrain.
    for x in 10..=14 {
        world.apply(WorldCommand::SetMaterial {
            x,
            y: 14,
            z: 4,
            material: Material::Rock,
        });
    }
    let view = world.view();
    check_light(&mut warm, &view, "warm, after an edit");
    check_light(&mut lazy, &view, "lazy, after an edit");
}
