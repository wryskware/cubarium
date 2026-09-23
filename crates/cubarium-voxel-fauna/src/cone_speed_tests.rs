//! Cone speed (`design/handoffs/voxel-cone-speed-2026-09-22.md`), written before the
//! implementation:
//!
//! - item 2: the dense occupancy answers every cell query and every ray exactly as the
//!   hash-map occupancy it replaces, on a seeded landscape with bodies, stands, stripped
//!   crowns, ground pools (films and heaps), water and ledges — whole-world and windowed;
//! - item 3: the cell-exact traversal clips a corner the fixed sub-step skipped, walks
//!   exactly the cells under an axis-aligned ray, and reports the analytic entry into a
//!   box, and into water or a pool at the plane of its surface;
//! - item 4: a held (static-episode) occupancy patched after a bite, a partial meal and a
//!   death equals a fresh build, and stepping with it held changes nothing.
//!
//! The reference map below is the pre-package `build_occupancy`, kept verbatim here as the
//! oracle; nothing outside this file uses it.

use std::collections::HashMap;

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, VoxelView, World};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraConfig, FloraView, Site,
    Species as Plant,
};

use crate::senses::{
    self, CellOccupancy, Dda, Fine, HeldCone, POOL_BULK_DENSITY, build_occupancy,
    ray_first_hit_cell_in,
};
use crate::{Command, Fauna, FaunaConfig, Founder, StartingStores};

const V: f64 = 0.25;

// ------------------------------------------------------------------ the reference map

/// The hash-map occupancy as it stood at `retrain` 534b510.
struct MapOccupancy {
    environment: HashMap<usize, Fine>,
    bodies: HashMap<usize, Vec<u64>>,
    pools: HashMap<usize, f64>,
}

impl MapOccupancy {
    fn build(
        view: &VoxelView<'_>,
        fv: &FloraView<'_>,
        fauna: &crate::FaunaView<'_>,
        pools_occlude: bool,
        window: Option<&[bool]>,
    ) -> MapOccupancy {
        let c = view.config;
        let v = c.voxel_m;
        let seen = |cx: i64| window.is_none_or(|w| w[cx.rem_euclid(i64::from(c.width)) as usize]);
        let mut environment: HashMap<usize, Fine> = HashMap::new();
        for foliage_pass in [true, false] {
            for stand in fv.stands.iter() {
                for layer in fv.profile_layers(stand) {
                    if layer.kind.bears_foliage() != foliage_pass {
                        continue;
                    }
                    let class = if !layer.kind.bears_foliage() {
                        Fine::Trunk
                    } else if layer.stock > 0.0 {
                        Fine::FoliageCrown
                    } else {
                        Fine::StrippedCrown
                    };
                    let span = layer.radius_v.ceil() as i64;
                    let r2 = layer.radius_v * layer.radius_v;
                    let root = i64::from(stand.site.x);
                    if window.is_some() && !(root - span..=root + span).any(&seen) {
                        continue;
                    }
                    for cell_y in layer.cells.0..=layer.cells.1 {
                        if cell_y <= 0 || cell_y as u32 >= c.height {
                            continue;
                        }
                        for dz in -span..=span {
                            for dx in -span..=span {
                                if (dx * dx + dz * dz) as f64 > r2 {
                                    continue;
                                }
                                let z = i64::from(stand.site.z) + dz;
                                if z < 0 || z >= i64::from(c.depth) {
                                    continue;
                                }
                                let cx =
                                    (i64::from(stand.site.x) + dx).rem_euclid(i64::from(c.width));
                                if !seen(cx) {
                                    continue;
                                }
                                let cell = c.index(cx, cell_y as u32, z as u32);
                                environment.entry(cell).or_insert(class);
                            }
                        }
                    }
                }
            }
        }
        let mut pools: HashMap<usize, f64> = HashMap::new();
        if pools_occlude {
            for g in fv.ground.iter() {
                let organic = g.litter + g.carrion + g.dead_wood;
                if !(organic > 0.0) || !seen(i64::from(g.site.x)) {
                    continue;
                }
                let floor = crate::surface_m(g.site.y, v);
                let top = floor + organic / (v * v * POOL_BULK_DENSITY);
                let mut y = g.site.y + 1;
                while y < c.height && f64::from(y) * v < top {
                    let cell = c.index(i64::from(g.site.x), y, g.site.z);
                    if !environment.contains_key(&cell) {
                        pools.entry(cell).or_insert(top);
                    }
                    y += 1;
                }
            }
        }
        let mut bodies: HashMap<usize, Vec<u64>> = HashMap::new();
        for a in fauna.animals {
            if let Some((ax, az)) = a.pose.column(c.voxel_m, c.depth) {
                if !seen(ax) {
                    continue;
                }
                let layer = i64::from(a.site.y) + 1;
                if layer > 0 && layer < i64::from(c.height) {
                    let cell = c.index(ax, layer as u32, az);
                    bodies.entry(cell).or_default().push(a.id);
                }
            }
        }
        MapOccupancy {
            environment,
            bodies,
            pools,
        }
    }
}

impl CellOccupancy for MapOccupancy {
    fn other_body(&self, cell: usize, observer_id: u64) -> bool {
        self.bodies
            .get(&cell)
            .is_some_and(|ids| ids.iter().any(|&id| id != observer_id))
    }

    fn environment(&self, cell: usize) -> Option<Fine> {
        self.environment.get(&cell).copied()
    }

    fn pool_top(&self, cell: usize) -> Option<f64> {
        self.pools.get(&cell).copied()
    }
}

// ------------------------------------------------------------------ the seeded landscape

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

const W: u32 = 40;
const H: u32 = 16;
const D: u32 = 10;

fn solid(world: &mut World, x: i64, y: u32, z: u32) {
    world.apply(WorldCommand::SetMaterial {
        x,
        y,
        z,
        material: Material::Soil,
    });
}

/// A seeded strip: rolling ground 1..=5 high, a few two-column ledges over it, water in
/// some hollows (fractions and more than a cell), stands of five species at several sizes
/// with some crowns stripped or half eaten, ground pools from a film to a three-cell heap,
/// and founders of both lineages — two of them sharing a column.
fn landscape(seed: u64) -> (World, Flora, Fauna) {
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
                solid(&mut world, i64::from(x), y, z);
            }
        }
    }
    for _ in 0..4 {
        let (x, z) = (
            rng.below(u64::from(W)) as i64,
            rng.below(u64::from(D)) as u32,
        );
        let y = 9 + rng.below(3) as u32;
        solid(&mut world, x, y, z);
        solid(&mut world, x + 1, y, z);
    }
    for _ in 0..10 {
        let (x, z) = (
            rng.below(u64::from(W)) as u32,
            rng.below(u64::from(D)) as u32,
        );
        let h = top[(z * W + x) as usize];
        let cells = [0.3, 0.5, 0.9, 1.6][rng.below(4) as usize];
        world.apply(WorldCommand::AddWater {
            x: i64::from(x),
            y: h + 1,
            z,
            volume_m3: cells * V * V * V,
        });
    }
    let mut flora = Flora::new(FloraConfig::default());
    let species = [
        Plant::Bloomcrown,
        Plant::Umbrellafrond,
        Plant::Springturf,
        Plant::Velvetpad,
        Plant::Stonecushion,
    ];
    let mut seeded = Vec::new();
    for _ in 0..40 {
        let (x, z) = (
            rng.below(u64::from(W)) as u32,
            rng.below(u64::from(D)) as u32,
        );
        let sp = species[rng.below(species.len() as u64) as usize];
        let wood = (0.2 + 0.8 * rng.unit()) * flora.config().species(sp).wood_max;
        if flora.apply(
            &world,
            FloraCommand::Seed {
                x: i64::from(x),
                z,
                species: sp,
                wood,
            },
        ) {
            seeded.push(x);
        }
    }
    let sites: Vec<Site> = flora.view().stands.iter().map(|s| s.site).collect();
    assert!(sites.len() >= 15, "the fixture seeded too few stands");
    for (i, &site) in sites.iter().enumerate() {
        match i % 4 {
            0 => {
                let _ = flora.take_foliage(site, 1e9); // stripped bare
            }
            1 => {
                let _ = flora.take_foliage(site, 0.02); // the lowest layer nibbled
            }
            _ => {}
        }
    }
    for i in 0..14 {
        let (x, z) = (
            rng.below(u64::from(W)) as u32,
            rng.below(u64::from(D)) as u32,
        );
        let site = Site {
            x,
            y: top[(z * W + x) as usize],
            z,
        };
        let cell_organic = V * V * V * POOL_BULK_DENSITY;
        let organic = [0.1, 0.5, 1.0, 2.7][i % 4] * cell_organic;
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
    let mut fauna = Fauna::new(FaunaConfig::default());
    fauna.set_births_enabled(false);
    let mut columns: Vec<(u32, u32)> = (0..10)
        .map(|_| {
            (
                rng.below(u64::from(W)) as u32,
                rng.below(u64::from(D)) as u32,
            )
        })
        .collect();
    columns.push(columns[0]); // two bodies in one cell
    for (k, (x, z)) in columns.into_iter().enumerate() {
        let founder = if k % 3 == 2 {
            Founder::Blind
        } else {
            Founder::Browser
        };
        fauna.apply(
            &world,
            Command::IntroduceFounder {
                x: i64::from(x),
                z,
                founder,
                stores: StartingStores::FULL,
                heading_rad: rng.unit() * std::f64::consts::TAU,
            },
        );
    }
    assert!(
        fauna.view().animals.len() >= 8,
        "the fixture placed too few bodies"
    );
    (world, flora, fauna)
}

/// Every browser fan direction at eight headings.
fn fan() -> Vec<(f64, f64, f64)> {
    let m = Founder::Browser.manifest();
    let mut out = Vec::new();
    for h in 0..8 {
        let heading = f64::from(h) * std::f64::consts::FRAC_PI_4 + 0.1;
        for &c in m.sector_centres_deg {
            for &y in m.ray_yaw_offsets_deg {
                for &p in m.ray_pitch_offsets_deg {
                    out.push(senses::ray_direction(heading, c, y, p));
                }
            }
        }
    }
    out
}

/// Eyes over every column's skyline face at three heights, off the cell centre.
fn origins(view: &VoxelView<'_>) -> Vec<(f64, f64, f64)> {
    let c = view.config;
    let mut out = Vec::new();
    for z in 0..c.depth {
        for x in 0..c.width {
            let Some(y) = (0..c.height)
                .rev()
                .find(|&y| view.is_support(i64::from(x), y, z))
            else {
                continue;
            };
            for eye in [0.07, 0.19, 0.41] {
                out.push((
                    (f64::from(x) + 0.37) * V,
                    crate::surface_m(y, V) + eye,
                    (f64::from(z) + 0.61) * V,
                ));
            }
        }
    }
    out
}

// ------------------------------------------------------------------ item 2: dense == map

fn assert_same_cells<A: CellOccupancy, B: CellOccupancy>(
    view: &VoxelView<'_>,
    a: &A,
    b: &B,
    observers: &[u64],
) {
    for cell in 0..view.config.cells() {
        assert_eq!(
            a.environment(cell),
            b.environment(cell),
            "environment, cell {cell}"
        );
        assert_eq!(a.pool_top(cell), b.pool_top(cell), "pool top, cell {cell}");
        for &o in observers {
            assert_eq!(
                a.other_body(cell, o),
                b.other_body(cell, o),
                "body, cell {cell}, observer {o}"
            );
        }
    }
}

#[test]
fn the_dense_occupancy_reads_every_cell_and_ray_as_the_map_did() {
    for seed in [3u64, 17, 91] {
        let (world, flora, fauna) = landscape(seed);
        let (v, fv, av) = (world.view(), flora.view(), fauna.view());
        let mut observers: Vec<u64> = av.animals.iter().map(|a| a.id).collect();
        observers.push(u64::MAX);

        let map = MapOccupancy::build(&v, &fv, &av, true, None);
        let dense = build_occupancy(&v, &fv, &av, true, None);
        assert!(!map.environment.is_empty() && !map.pools.is_empty() && !map.bodies.is_empty());
        assert!(
            map.environment.values().any(|&f| f == Fine::StrippedCrown)
                && map.environment.values().any(|&f| f == Fine::FoliageCrown)
                && map.environment.values().any(|&f| f == Fine::Trunk),
            "seed {seed}: the fixture lacks a class"
        );
        assert_same_cells(&v, &map, &dense, &observers);

        let dirs = fan();
        let mut hits = 0usize;
        for &o in &origins(&v) {
            for (k, &d) in dirs.iter().enumerate() {
                let observer = observers[k % observers.len()];
                let want = ray_first_hit_cell_in(&v, &map, observer, o, d, 2.0);
                let got = ray_first_hit_cell_in(&v, &dense, observer, o, d, 2.0);
                assert_eq!(got, want, "seed {seed}: ray from {o:?} along {d:?}");
                hits += usize::from(want.is_some());
            }
        }
        assert!(
            hits > 1000,
            "seed {seed}: too few rays hit anything ({hits})"
        );

        // A window: every third column and a run, the way a stage's observers mark it.
        let window: Vec<bool> = (0..W)
            .map(|x| x % 3 == 0 || (10..20).contains(&x))
            .collect();
        let map_w = MapOccupancy::build(&v, &fv, &av, true, Some(&window));
        let dense_w = build_occupancy(&v, &fv, &av, true, Some(&window));
        assert_same_cells(&v, &map_w, &dense_w, &observers);
        // And with the pools left out (the diagnostic arm).
        let map_np = MapOccupancy::build(&v, &fv, &av, false, None);
        let dense_np = build_occupancy(&v, &fv, &av, false, None);
        assert_same_cells(&v, &map_np, &dense_np, &observers);
    }
}

// ------------------------------------------------------------------ item 3: the traversal

/// Open air, `n` cells on a side, no ground: nothing but what a test puts there.
fn air(n: u32) -> World {
    World::empty(VoxelConfig {
        width: n,
        height: n,
        depth: n,
        voxel_m: V,
        seed: 1,
        ..VoxelConfig::default()
    })
}

fn empty_layers() -> (Flora, Fauna) {
    (
        Flora::new(FloraConfig::default()),
        Fauna::new(FaunaConfig::default()),
    )
}

/// Slab-method `(entry, exit)` of a ray through the box `[lo, hi]`, if it enters within
/// `range`.
fn slab(
    o: (f64, f64, f64),
    d: (f64, f64, f64),
    lo: (f64, f64, f64),
    hi: (f64, f64, f64),
    range: f64,
) -> Option<(f64, f64)> {
    let (mut t0, mut t1) = (0.0f64, f64::INFINITY);
    for (o, d, lo, hi) in [
        (o.0, d.0, lo.0, hi.0),
        (o.1, d.1, lo.1, hi.1),
        (o.2, d.2, lo.2, hi.2),
    ] {
        if d == 0.0 {
            if o < lo || o >= hi {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo - o) / d, (hi - o) / d);
        let (a, b) = if a < b { (a, b) } else { (b, a) };
        t0 = t0.max(a);
        t1 = t1.min(b);
    }
    (t0 < t1 && t0 <= range).then_some((t0, t1))
}

/// A ray that clips a solid cell's corner by a fiftieth of a cell — inside it for well
/// under the old quarter-voxel sub-step — hits it, at its entry.
#[test]
fn a_ray_grazing_a_cell_corner_hits_it() {
    let mut world = air(24);
    solid(&mut world, 12, 12, 12);
    let (flora, fauna) = empty_layers();
    let (v, fv, av) = (world.view(), flora.view(), fauna.view());
    let occ = build_occupancy(&v, &fv, &av, true, None);
    // Horizontal, north-east, on the line x - z = c just inside the cell's (x = 12,
    // z = 13) corner: it is inside the cell for x in [12 V, 12 V + delta].
    let delta = 0.02 * V;
    let c = 12.0 * V - 13.0 * V + delta;
    let s = std::f64::consts::FRAC_1_SQRT_2;
    let d = (s, 0.0, s);
    let o = (8.0 * V, 12.5 * V, 8.0 * V - c);
    let lo = (12.0 * V, 12.0 * V, 12.0 * V);
    let hi = (13.0 * V, 13.0 * V, 13.0 * V);
    let (entry, exit) = slab(o, d, lo, hi, 2.0).expect("the fixture's ray enters the cell");
    assert!(
        exit - entry < 0.1 * V,
        "the fixture clips, it does not cross: {}",
        exit - entry
    );
    let (dist, fine, cell) =
        ray_first_hit_cell_in(&v, &occ, u64::MAX, o, d, 2.0).expect("the clipped corner is a hit");
    assert_eq!(fine, Fine::Terrain);
    assert_eq!(cell, v.config.index(12, 12, 12));
    assert!(
        (dist - entry).abs() < 1e-9,
        "entry {entry}, reported {dist}"
    );
}

/// Along each axis, the traversal visits exactly the cells under the ray, in order, each
/// once, entering the first at 0 and each next one at its face.
#[test]
fn a_ray_along_an_axis_visits_exactly_the_cells_under_it() {
    let o = (5.5 * V, 6.3 * V, 7.8 * V);
    let range = 2.0;
    for (axis, sign) in [
        (0, 1.0),
        (0, -1.0),
        (1, 1.0),
        (1, -1.0),
        (2, 1.0),
        (2, -1.0),
    ] {
        let mut d = [0.0; 3];
        d[axis] = sign;
        let cells: Vec<_> = Dda::new(V, o, (d[0], d[1], d[2]), range).collect();
        let start = [5i64, 6, 7];
        let frac = [0.5, 0.3, 0.8][axis];
        let first_face = if sign > 0.0 { 1.0 - frac } else { frac } * V;
        // Cells entered at 0, first_face, first_face + V, ... while the entry is in range.
        let expected = 1 + ((range - first_face) / V).floor() as usize + 1;
        assert_eq!(cells.len(), expected, "axis {axis} sign {sign}: {cells:?}");
        for (k, c) in cells.iter().enumerate() {
            let mut want = start;
            want[axis] += if sign > 0.0 { k as i64 } else { -(k as i64) };
            assert_eq!([c.x, c.y, c.z], want, "axis {axis} sign {sign}, cell {k}");
            let t_in = if k == 0 {
                0.0
            } else {
                first_face + (k - 1) as f64 * V
            };
            assert!(
                (c.t_in - t_in).abs() < 1e-12,
                "entry of cell {k}: {}",
                c.t_in
            );
            assert!((c.t_out - (first_face + k as f64 * V)).abs() < 1e-12);
        }
    }
}

/// Seeded rays against one solid cell in open air: the reported distance is the slab
/// method's entry, and a ray that misses the box reports nothing.
#[test]
fn distances_agree_with_an_analytic_box_intersection() {
    let mut world = air(40);
    solid(&mut world, 20, 20, 20);
    let (flora, fauna) = empty_layers();
    let (v, fv, av) = (world.view(), flora.view(), fauna.view());
    let occ = build_occupancy(&v, &fv, &av, true, None);
    let lo = (20.0 * V, 20.0 * V, 20.0 * V);
    let hi = (21.0 * V, 21.0 * V, 21.0 * V);
    let mut rng = Rng(7);
    let (mut hits, mut misses) = (0, 0);
    for _ in 0..20_000 {
        let o = (
            (16.0 + 9.0 * rng.unit()) * V,
            (16.0 + 9.0 * rng.unit()) * V,
            (16.0 + 9.0 * rng.unit()) * V,
        );
        // Aim near the box so a good share hit.
        let aim = (
            (19.5 + 2.0 * rng.unit()) * V - o.0,
            (19.5 + 2.0 * rng.unit()) * V - o.1,
            (19.5 + 2.0 * rng.unit()) * V - o.2,
        );
        let n = (aim.0 * aim.0 + aim.1 * aim.1 + aim.2 * aim.2).sqrt();
        if n < 1e-9 {
            continue;
        }
        let d = (aim.0 / n, aim.1 / n, aim.2 / n);
        let want = slab(o, d, lo, hi, 2.0).map(|(t, _)| t);
        let got = ray_first_hit_cell_in(&v, &occ, u64::MAX, o, d, 2.0);
        match (want, got) {
            (Some(t), Some((dist, fine, cell))) => {
                assert_eq!(fine, Fine::Terrain);
                assert_eq!(cell, v.config.index(20, 20, 20));
                assert!(
                    (dist - t).abs() < 1e-9,
                    "{o:?} {d:?}: entry {t}, got {dist}"
                );
                hits += 1;
            }
            (None, None) => misses += 1,
            (w, g) => panic!("{o:?} along {d:?}: analytic {w:?}, traversal {g:?}"),
        }
    }
    assert!(hits > 2000 && misses > 2000, "hits {hits}, misses {misses}");
}

/// Water and a pool are hit where the ray crosses their surface, solved against the
/// plane, not at a sample below it.
#[test]
fn water_and_a_pool_are_entered_at_the_plane_of_their_surface() {
    // Water: half a cell over the face at (10, 4, 10) of a floor at y = 3.
    let mut world = air(20);
    for z in 0..20 {
        for x in 0..20 {
            solid(&mut world, x, 3, z);
        }
    }
    world.apply(WorldCommand::AddWater {
        x: 10,
        y: 4,
        z: 10,
        volume_m3: 0.5 * V * V * V,
    });
    let (flora, fauna) = empty_layers();
    let (v, fv, av) = (world.view(), flora.view(), fauna.view());
    let fill = v.free[v.config.index(10, 4, 10)];
    assert!(fill > 0.4 && fill < 0.6, "the fixture's fill: {fill}");
    let surface = (4.0 + fill) * V;
    let occ = build_occupancy(&v, &fv, &av, true, None);
    // Down at 30° along +x, from over column 7, crossing the cell's top face and then
    // the surface inside column 10.
    let s = std::f64::consts::FRAC_PI_6;
    let d = (s.cos(), -s.sin(), 0.0);
    let x_at_surface = 10.6 * V;
    let o_y = 5.9 * V;
    let t_s = (o_y - surface) / s.sin();
    let o = (x_at_surface - t_s * d.0, o_y, 10.5 * V);
    let (dist, fine, cell) = ray_first_hit_cell_in(&v, &occ, u64::MAX, o, d, 2.0).expect("water");
    assert_eq!(fine, Fine::Water);
    assert_eq!(cell, v.config.index(10, 4, 10));
    assert!((dist - t_s).abs() < 1e-9, "plane {t_s}, got {dist}");

    // A litter pool a third of a cell tall on the same kind of face, dry.
    let mut dry = air(20);
    for z in 0..20 {
        for x in 0..20 {
            solid(&mut dry, x, 3, z);
        }
    }
    let mut flora = Flora::new(FloraConfig::default());
    let organic = V * V * V * POOL_BULK_DENSITY / 3.0;
    assert!(flora.deposit(
        Site { x: 10, y: 3, z: 10 },
        Deposit {
            kind: DepositKind::Litter,
            organic,
            mineral: 0.02 * organic,
            energy: 2.0 * organic,
        },
    ));
    let (v, fv, av) = (dry.view(), flora.view(), fauna.view());
    let occ = build_occupancy(&v, &fv, &av, true, None);
    let top = 4.0 * V + V / 3.0;
    let t_p = (o_y - top) / s.sin();
    let o = (10.5 * V - t_p * d.0, o_y, 10.5 * V);
    let (dist, fine, cell) = ray_first_hit_cell_in(&v, &occ, u64::MAX, o, d, 2.0).expect("pool");
    assert_eq!(fine, Fine::GroundPool);
    assert_eq!(cell, v.config.index(10, 4, 10));
    assert!((dist - t_p).abs() < 1e-9, "plane {t_p}, got {dist}");
    // Level over the top of the pool: nothing.
    let over = ray_first_hit_cell_in(
        &v,
        &occ,
        u64::MAX,
        (8.5 * V, top + 0.01, 10.5 * V),
        (1.0, 0.0, 0.0),
        2.0,
    );
    assert!(over.is_none(), "a ray over the pool passes: {over:?}");
}

// ------------------------------------------------------------------ item 4: held and patched

#[test]
fn a_held_occupancy_patched_after_a_bite_and_a_death_equals_a_fresh_build() {
    let (world, mut flora, mut fauna) = landscape(29);
    let v = world.view();
    let mut held = HeldCone::default();
    let fresh = |flora: &Flora, fauna: &Fauna| {
        build_occupancy(&v, &flora.view(), &fauna.view(), true, None)
    };
    assert_eq!(
        *held.refresh(&v, &flora.view(), &fauna.view()),
        fresh(&flora, &fauna)
    );

    // A bite that strips a crown: a stand with foliage left loses all of it.
    let before = fresh(&flora, &fauna);
    let site = flora
        .view()
        .stands
        .iter()
        .find(|s| s.foliage > 0.0)
        .map(|s| s.site)
        .expect("a stand with foliage");
    assert!(flora.take_foliage(site, 1e9).is_some());
    let after = fresh(&flora, &fauna);
    assert_ne!(before, after, "stripping a crown changes the eye's classes");
    assert_eq!(
        *held.refresh(&v, &flora.view(), &fauna.view()),
        after,
        "after a bite"
    );

    // A partial meal off a pool: the heap shrinks.
    let pool = flora
        .view()
        .ground
        .iter()
        .find(|g| g.litter > 0.3)
        .map(|g| g.site)
        .expect("a litter heap");
    assert!(flora.take_litter(pool, 0.25).is_some());
    assert_eq!(
        *held.refresh(&v, &flora.view(), &fauna.view()),
        fresh(&flora, &fauna),
        "after a meal"
    );

    // A death: the body leaves and its carrion lands on a face that had no pool.
    let dead = fauna.animals[0];
    fauna.animals.remove(0);
    let face = Site {
        x: dead.site.x,
        y: dead.site.y,
        z: dead.site.z,
    };
    assert!(flora.deposit(
        face,
        Deposit {
            kind: DepositKind::Carrion,
            organic: 0.9,
            mineral: 0.02,
            energy: 1.8,
        },
    ));
    assert_eq!(
        *held.refresh(&v, &flora.view(), &fauna.view()),
        fresh(&flora, &fauna),
        "after a death"
    );

    // Bodies move between stages: re-indexed, nothing else rebuilt.
    fauna.animals[1].pose.x += 1.3 * V;
    assert_eq!(
        *held.refresh(&v, &flora.view(), &fauna.view()),
        fresh(&flora, &fauna),
        "after a move"
    );
}

/// A static episode stepped with the held occupancy observes exactly what one stepped
/// with a per-stage build observes: the same bodies, poses, stores and plant layer after
/// every tick, bites and all.
#[test]
fn stepping_with_a_held_occupancy_changes_nothing() {
    let (world, flora, fauna) = landscape(41);
    let (mut flora_a, mut fauna_a) = (flora.clone(), fauna.clone());
    let (mut flora_b, mut fauna_b) = (flora, fauna);
    let mut senses_a = senses::Senses::new();
    senses_a.settle(&world.view(), &flora_a.view());
    let mut senses_b = senses_a.clone();
    senses_a.hold_cone();
    for tick in 0..150 {
        fauna_a.step_with_senses(&world, &mut flora_a, 1, &mut senses_a);
        fauna_b.step_with_senses(&world, &mut flora_b, 1, &mut senses_b);
        assert_eq!(
            format!("{:?}", fauna_a.view().animals),
            format!("{:?}", fauna_b.view().animals),
            "tick {tick}: the bodies diverged"
        );
        assert_eq!(
            format!("{:?}", flora_a.view().stands),
            format!("{:?}", flora_b.view().stands),
            "tick {tick}: the stands diverged"
        );
    }
    assert!(
        flora_a.view().ledger.consumed_organic_out > 0.0,
        "the fixture's bodies never ate: nothing was patched"
    );
}

/// The traversal is part of what the browser's policy sees, so its digest records it; the
/// blind founder has no cone and its digest does not move.
#[test]
fn the_browser_digest_records_the_traversal() {
    let token = format!("|traversal:{}", crate::manifest::CONE_TRAVERSAL);
    assert!(
        Founder::Browser
            .manifest()
            .canonical_text()
            .contains(&token)
    );
    assert!(
        !Founder::Blind
            .manifest()
            .canonical_text()
            .contains("|traversal:")
    );
}
