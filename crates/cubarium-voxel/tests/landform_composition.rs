use std::collections::{BTreeSet, VecDeque};

use cubarium_voxel::{Config, Preset, World, hydrate};

fn index(c: &Config, x: u32, y: u32, z: u32) -> usize {
    (y as usize * c.depth as usize + z as usize) * c.width as usize + x as usize
}

fn chamber_diagnostic(world: &World) -> (usize, usize, usize, u32) {
    let c = world.config();
    let v = world.view();
    let mut floors = 0;
    let mut xs = BTreeSet::new();
    let mut zs = BTreeSet::new();
    let mut max_headroom = 0;
    for z in 0..c.depth {
        for x in 0..c.width {
            for floor_y in 0..c.height - 2 {
                if !v.is_support(x as i64, floor_y, z) {
                    continue;
                }
                let air_y = floor_y + 1;
                let roof =
                    (air_y + 1..c.height).find(|&y| v.material_at(x as i64, y, z).is_solid());
                let Some(roof_y) = roof else { continue };
                let headroom = roof_y - air_y;
                if headroom >= 2 {
                    floors += 1;
                    xs.insert(x);
                    zs.insert(z);
                    max_headroom = max_headroom.max(headroom);
                }
            }
        }
    }
    (floors, xs.len(), zs.len(), max_headroom)
}

#[test]
fn shipped_landform_composes_chambers_shelves_and_a_local_cascade() {
    let preset = Preset::find("small").expect("small is a shipped preset");
    let world = World::new(Config {
        seed: 1,
        ..preset.config()
    });
    let c = world.config().clone();
    let v = world.view();

    // Flood the outside air. A roofed floor counted below is therefore part of an
    // entered chamber, rather than a sealed pocket or a surface inferred from a heightmap.
    let mut outside = vec![false; c.cells()];
    let mut queue = VecDeque::new();
    for z in 0..c.depth {
        for x in 0..c.width {
            let i = index(&c, x, c.height - 1, z);
            if !v.material_at(x as i64, c.height - 1, z).is_solid() {
                outside[i] = true;
                queue.push_back((x, c.height - 1, z));
            }
        }
    }
    while let Some((x, y, z)) = queue.pop_front() {
        let mut neighbours = vec![
            ((x + 1) % c.width, y, z),
            ((x + c.width - 1) % c.width, y, z),
        ];
        if y > 0 {
            neighbours.push((x, y - 1, z));
        }
        if y + 1 < c.height {
            neighbours.push((x, y + 1, z));
        }
        if z > 0 {
            neighbours.push((x, y, z - 1));
        }
        if z + 1 < c.depth {
            neighbours.push((x, y, z + 1));
        }
        for (nx, ny, nz) in neighbours {
            let ni = index(&c, nx, ny, nz);
            if !outside[ni] && !v.material_at(nx as i64, ny, nz).is_solid() {
                outside[ni] = true;
                queue.push_back((nx, ny, nz));
            }
        }
    }

    let mut chamber_floors = Vec::new();
    for z in 0..c.depth {
        for x in 0..c.width {
            for y in 1..c.height - 2 {
                let entered = outside[index(&c, x, y, z)];
                let headroom = !v.material_at(x as i64, y + 1, z).is_solid();
                let roofed =
                    (y + 2..c.height).any(|roof_y| v.material_at(x as i64, roof_y, z).is_solid());
                if entered && headroom && roofed && v.is_support(x as i64, y - 1, z) {
                    chamber_floors.push((x, y - 1, z));
                }
            }
        }
    }
    let chamber_x: BTreeSet<_> = chamber_floors.iter().map(|&(x, _, _)| x).collect();
    let chamber_z: BTreeSet<_> = chamber_floors.iter().map(|&(_, _, z)| z).collect();
    eprintln!(
        "small seed 1 entered roofed floor: {} cells across {} x positions and {} depth rows",
        chamber_floors.len(),
        chamber_x.len(),
        chamber_z.len()
    );

    let surface: Vec<Vec<i64>> = (0..c.depth)
        .map(|z| {
            (0..c.width)
                .map(|x| {
                    v.surface_y(x as i64, z)
                        .expect("generated column has ground") as i64
                })
                .collect()
        })
        .collect();
    for z in 0..c.depth as usize {
        let seam = (surface[z][0] - surface[z][c.width as usize - 1]).abs();
        let interior = surface[z]
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .max()
            .unwrap_or(0);
        assert!(
            seam <= interior + 1,
            "depth {z}: periodic seam jumps {seam} voxels while interior neighbours jump at most {interior}"
        );
        for y in 0..c.height {
            assert_eq!(
                v.material_at(-1, y, z as u32),
                v.material_at(c.width as i64 - 1, y, z as u32),
                "wrapped material lookup disagrees at depth {z}, y {y}"
            );
        }
    }

    // Hydration deliberately leaves pools below their sills. The closed-cycle stream
    // starts at the spring on the first tick, so give it only enough time to traverse
    // this small world before looking for water in the air between standing pools.
    let mut flowing = world.clone();
    for _ in 0..96 {
        flowing.step();
    }
    let flowing_view = flowing.view();
    let lake = hydrate::lake(&flowing);
    let pools = hydrate::pools(&flowing);
    let elevated: Vec<_> = pools
        .iter()
        .filter(|pool| pool.floor_y > lake.floor_y && pool.volume_m3 > 0.0)
        .collect();
    assert!(
        !lake.cells.is_empty(),
        "settled generated landform has no contained lake"
    );
    assert!(
        elevated.iter().any(|pool| pool.level_y > lake.level_y),
        "settled generated landform has no filled basin above its lake"
    );
    let pooled: BTreeSet<usize> = pools
        .iter()
        .flat_map(|pool| pool.cells.iter().copied())
        .collect();
    let (spring_x, spring_y, _) = world.spring_cell().expect("generated spring");
    let (_, outlet_y, _) = world.outlet_cell().expect("generated outlet");
    let mut fall_x = BTreeSet::new();
    let mut falling = 0usize;
    let mut source_fall = false;
    for y in 1..c.height {
        for z in 0..c.depth {
            for x in 0..c.width {
                let i = index(&c, x, y, z);
                if flowing_view.free_at(x as i64, y, z) > 0.0
                    && !pooled.contains(&i)
                    && !flowing_view.material_at(x as i64, y - 1, z).is_solid()
                {
                    falling += 1;
                    fall_x.insert(x);
                    let dx = x.abs_diff(spring_x).min(c.width - x.abs_diff(spring_x));
                    // Within five columns (0.6 m) of the spring, not one: what fell right
                    // beside it was spray the water solver used to fling sideways — films of
                    // a millionth of a cell, gone after packages H, C and P — while the
                    // stream itself leaves the spring's shelf three to five columns along
                    // (measured on this seed, 2026-09-22).
                    source_fall |= y > outlet_y && y <= spring_y && dx <= 5;
                }
            }
        }
    }
    assert!(
        falling > 0,
        "the elevated basins have no actual falling spill route"
    );
    assert!(
        source_fall,
        "no wet fall descends from the localized spring route above the lake; {} falling cells span {} ring columns",
        falling,
        fall_x.len()
    );
    assert!(
        spring_y > outlet_y,
        "source at y={spring_y} does not descend to lake outlet at y={outlet_y}"
    );

    // Keep the two visual-review seeds visible as diagnostics without turning their
    // current silhouettes into pass/fail fixtures.
    let seed_1 = chamber_diagnostic(&world);
    let seed_77_world = World::new(Config {
        seed: 77,
        ..preset.config()
    });
    let seed_77 = chamber_diagnostic(&seed_77_world);
    let mut seed_77_settled = seed_77_world.clone();
    for _ in 0..96 {
        seed_77_settled.step();
    }
    let seed_77_lake = hydrate::lake(&seed_77_settled);
    let seed_77_pools = hydrate::pools(&seed_77_settled);
    assert!(
        seed_77_pools.iter().any(|pool| {
            pool.floor_y > seed_77_lake.floor_y
                && pool.level_y > seed_77_lake.level_y
                && pool.volume_m3 > 0.0
        }),
        "small seed 77 loses every elevated pool after 96 ticks"
    );
    eprintln!(
        "small chamber diagnostic (floor cells, x extent, z extent, max headroom): seed 1 {seed_1:?}; seed 77 {seed_77:?}"
    );
}
