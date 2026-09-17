//! Water: prescribed rain and evaporation, a conservative free-water solver over the
//! void voxels (fall, then equalize every connected water region to one surface level),
//! infiltration into soil pores, drainage to the aquifer, spring discharge where head
//! exceeds an outlet, and one named outlet that exports. The core worker implements this;
//! the stub only applies commands so a frontend can run.

use crate::{Command, Material, World};

pub fn step(_world: &mut World) {}

pub fn apply(world: &mut World, command: Command) -> f64 {
    match command {
        Command::AddWater { x, y, z, volume_m3 } => {
            let i = world.config.index(x, y, z);
            if world.material[i].is_solid() {
                return 0.0;
            }
            let v = world.config.voxel_volume();
            let room = (1.0 - world.free[i] as f64) * v;
            let take = volume_m3.min(room).max(0.0);
            world.free[i] += (take / v) as f32;
            world.ledger.user_in += take;
            take
        }
        Command::SetMaterial { x, y, z, material } => {
            let i = world.config.index(x, y, z);
            let v = world.config.voxel_volume();
            let lost = world.free[i] as f64 * v
                + world.pore[i] as f64 * v * world.material[i].pore_capacity();
            world.material[i] = material;
            world.free[i] = 0.0;
            world.pore[i] = 0.0;
            world.ledger.displaced_out += lost;
            0.0
        }
        Command::ChargeAquifer { volume_m3 } => {
            let take = volume_m3.max(-world.aquifer_m3);
            world.aquifer_m3 += take;
            world.ledger.user_in += take;
            take
        }
        Command::SetOutlet { open } => {
            world.outlet_open = open;
            0.0
        }
        Command::RainPulse { .. } => 0.0,
    }
}

#[allow(dead_code)]
fn _material_used(_: Material) {}
