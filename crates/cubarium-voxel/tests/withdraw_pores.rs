//! `World::withdraw_pores`, the batched root-water withdrawal, against one
//! `Command::WithdrawPore` per cell on hand-built worlds: the same receipts, the same
//! water ledger, the same world (pore water, damp and drainable marks, atmosphere).

use cubarium_voxel::{Command, Config, Material, World};

/// A soil slab with every cell at its own wetness, a rock cell and an air cell in it, under
/// the open or the closed budget.
fn slab(closed: bool) -> World {
    let config = Config {
        width: 8,
        height: 4,
        depth: 2,
        voxel_m: 0.5,
        closed_water_budget: closed,
        rain_m_per_s: 0.0,
        ..Config::default()
    };
    let mut w = World::empty(config.clone());
    let cap = config.materials.pore_capacity(Material::Soil) * config.voxel_volume();
    let mut k = 0u32;
    for z in 0..config.depth {
        for y in 0..3 {
            for x in 0..config.width as i64 {
                k += 1;
                let wet = (k * 37 % 101) as f64 / 100.0;
                if wet > 0.05 {
                    w.apply(Command::AddWater { x, y, z, volume_m3: wet * cap });
                }
                w.apply(Command::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    w.apply(Command::SetMaterial { x: 3, y: 1, z: 0, material: Material::Rock });
    w
}

/// Ascending cells with asks below, at and above what each holds, a zero, a malformed
/// one, a rock cell and an air cell.
fn asks(w: &World) -> Vec<(usize, f64)> {
    let c = w.config();
    let cap = c.materials.pore_capacity(Material::Soil) * c.voxel_volume();
    let mut v: Vec<(usize, f64)> = Vec::new();
    let mut k = 0u32;
    for z in 0..c.depth {
        for y in 0..4 {
            for x in 0..c.width as i64 {
                k += 1;
                let ask = match k % 6 {
                    0 => 0.0,
                    1 => 2.0 * cap,
                    2 => f64::NAN,
                    3 => -0.1,
                    _ => (k % 7) as f64 / 13.0 * cap,
                };
                v.push((c.index(x, y, z), ask));
            }
        }
    }
    v
}

fn per_cell(w: &mut World, cells: &[(usize, f64)]) -> Vec<f64> {
    cells
        .iter()
        .map(|&(i, volume_m3)| {
            let (x, y, z) = w.config().coords(i);
            -w.apply(Command::WithdrawPore { x: x as i64, y, z, volume_m3 })
        })
        .collect()
}

#[test]
fn the_batch_gives_what_the_per_cell_commands_give_and_leaves_the_same_world() {
    for closed in [false, true] {
        let mut a = slab(closed);
        let mut b = a.clone();
        let cells = asks(&a);
        let one_by_one = per_cell(&mut a, &cells);
        let mut batched = Vec::new();
        b.withdraw_pores(&cells, &mut batched);

        assert_eq!(batched.len(), cells.len());
        for (k, (&p, &q)) in one_by_one.iter().zip(&batched).enumerate() {
            assert!(p == q, "cell {:?}: per cell {p}, batched {q}", cells[k]);
        }
        assert!(batched.iter().any(|&g| g > 0.0), "nothing was withdrawn");
        assert!(batched.iter().any(|&g| g == 0.0), "no ask was refused or empty");
        assert_eq!(a.view().ledger, b.view().ledger, "the water ledger");
        assert_eq!(a.view().atmosphere_m3.to_bits(), b.view().atmosphere_m3.to_bits());
        assert_eq!(a.view().pore, b.view().pore, "the pore water the pack diffs");
        // Every field, the damp and drainable sets included.
        assert!(a == b, "the worlds differ after the same withdrawals (closed {closed})");
        // And the world steps on identically from there.
        a.step();
        b.step();
        assert!(a == b, "the worlds differ one tick later (closed {closed})");
        let r = b.view().water_residual();
        assert!(r.abs() < 1e-12, "water residual {r}");
    }
}
