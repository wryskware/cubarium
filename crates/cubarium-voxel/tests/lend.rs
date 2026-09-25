//! The overlapped tick's water lend: after `take_readable_from` and `restore_water_from`
//! the live world holds exactly the water the read copy does — the copy-back skips only
//! cells that cannot differ — through rain, a pulse, a withdrawal, an addition and a
//! skipped lend.

use cubarium_voxel::{Command, Config, WaterLend, World};

fn same_water(live: &World, copy: &World, what: &str) {
    let (a, b) = (live.view(), copy.view());
    let free = a.free.iter().zip(b.free).filter(|(x, y)| x != y).count();
    let pore = a.pore.iter().zip(b.pore).filter(|(x, y)| x != y).count();
    assert_eq!((free, pore), (0, 0), "{what}: cells that differ (free, pore)");
}

#[test]
fn the_restored_water_is_the_lent_water() {
    for threads in [1usize, 4] {
        let mut live = World::new(Config {
            seed: 3,
            rain_m_per_s: 0.0005,
            ..Config::default()
        });
        live.apply(Command::SetOutlet { open: true });
        let mut copy = live.clone();
        let mut lend = WaterLend::default();
        for t in 0..60u64 {
            if t == 30 {
                // A lend skipped: the chained tick in between.
                live.step_with(threads);
            }
            copy.take_readable_from(&mut live, &mut lend);
            live.restore_water_from(&copy, &mut lend, threads);
            same_water(&live, &copy, &format!("{threads} threads, tick {t}"));
            // What a tick does to the live world between lends: water, a drink, and
            // between ticks a host's commands. The lend is partial from the second tick.
            live.step_with(threads);
            if t % 7 == 0 {
                live.apply(Command::RainPulse { volume_m3: 1.0 });
            }
            let (x, y, z) = (40, 10, 12);
            live.apply(Command::WithdrawPore {
                x,
                y,
                z,
                volume_m3: 1e-4,
            });
            live.apply(Command::AddWater {
                x: 70,
                y: 30,
                z: 5,
                volume_m3: 1e-3,
            });
            // (`step_with` moves the clock, as the overlapped tick's `Advance` does.)
        }
    }
}
