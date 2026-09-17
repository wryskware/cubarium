//! Feature-guided landform generation: broad ridge and basin first, soil pockets and
//! rock layers next, one overhang and one covered passage, then weak noise. Periodic in
//! `x`. The core worker replaces this stub.

use crate::{Material, World};

/// Fill `world.material` from `world.config`. The stub lays a flat soil-over-bedrock
/// floor so a frontend has something to draw before the generator exists.
pub fn landform(world: &mut World) {
    let c = world.config.clone();
    for x in 0..c.width as i64 {
        for z in 0..c.depth {
            for y in 0..c.height.min(12) {
                let m = if y < 4 { Material::Bedrock } else if y < 8 { Material::Rock } else { Material::Soil };
                world.material[c.index(x, y, z)] = m;
            }
        }
    }
}
