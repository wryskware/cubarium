//! Can a body get all the way around the ring on foot?
//!
//! A hard constraint on generation, not a simulation: incision that cuts a slot across
//! the strip, or a wall it raises, would leave a habitat with two halves and no route
//! between them. The ecology never says so out loud — a browser simply never arrives —
//! so the generator answers it here, once, in the open.
//!
//! The ground is the world's own **support faces** ([`crate::world::View::is_support`]):
//! a solid voxel with void over it, at any depth. Two faces are a step apart when their
//! columns are face-neighbours in `(x, z)` and the rise between them is within
//! `step_m`. `x` wraps; front and back are walls.
//!
//! # One rule, three consumers
//!
//! That adjacency is the **founder step rule**
//! (`design/handoffs/voxel-founder-step-2026-09-22.md`): a body may move between
//! face-neighbouring support faces whose standing layers differ by at most its climb.
//! It is written here, once, in [`climb_voxels`] and [`components`], because this crate
//! is below both the fauna crate and the host: the generator's ring gate, the fauna
//! crate's `walkable_components` (and through it the edible-stock observer) and the
//! seeder's feeding-face adjacency all answer the question with these.

use crate::World;

/// A climb height in metres as whole voxels, **rounded to nearest**: the one conversion
/// of a physical climb to a layer count, applied at the consumer and never inside a
/// body's definition (`design/voxel-encounter-contract-2026-09-21.md` §4).
///
/// The founders' placeholders (`design/backlog.md` §1) are the browser's 0.25 m — two
/// voxels on the 0.125 m `small` preset, one on the 0.25 m `default` and `wide` — and
/// the shredder's 0.125 m, one voxel on both. A non-finite or negative height is no
/// climb at all.
pub fn climb_voxels(climb_m: f64, voxel_m: f64) -> u32 {
    if !climb_m.is_finite() || climb_m <= 0.0 || !voxel_m.is_finite() || voxel_m <= 0.0 {
        return 0;
    }
    (climb_m / voxel_m).round().clamp(0.0, f64::from(u32::MAX)) as u32
}

/// The connected components of a set of support faces under the step rule: a component
/// id per face, in the order of `faces`.
///
/// Two faces are joined when their columns are face-neighbours in `(x, z)` — `x` wraps
/// over `width`, the strip's `z` ends are walls — and their layers differ by at most
/// `climb`. `climb = 0` is the level rule this replaced. Nothing here filters the face
/// set: headroom, wade depth and whatever else makes a face standable are the caller's,
/// because they are the caller's body's.
pub fn components(faces: &[(u32, u32, u32)], width: u32, climb: u32) -> Vec<usize> {
    components_linked(faces, width, climb, &[])
}

/// [`components`], with `links` — pairs of indices into `faces` — joined as well: the
/// edges a caller's own rule adds to the step rule's, such as a wall a climber goes up and
/// down (`cubarium-voxel-fauna`'s route maps, package mobility).
pub fn components_linked(
    faces: &[(u32, u32, u32)],
    width: u32,
    climb: u32,
    links: &[(usize, usize)],
) -> Vec<usize> {
    let index: rustc_hash::FxHashMap<(u32, u32, u32), usize> =
        faces.iter().enumerate().map(|(i, &f)| (f, i)).collect();
    let mut union = Union::new(faces.len());
    for &(a, b) in links {
        if a < faces.len() && b < faces.len() {
            union.join(a, b);
        }
    }
    let climb = i64::from(climb);
    for (i, &(x, y, z)) in faces.iter().enumerate() {
        for (dx, dz) in [(1i64, 0i64), (0, 1)] {
            let nx = (i64::from(x) + dx).rem_euclid(i64::from(width.max(1))) as u32;
            let nz = i64::from(z) + dz;
            if nz < 0 {
                continue;
            }
            for dy in -climb..=climb {
                let ny = i64::from(y) + dy;
                if ny < 0 {
                    continue;
                }
                if let Some(&j) = index.get(&(nx, ny as u32, nz as u32)) {
                    union.join(i, j);
                }
            }
        }
    }
    (0..faces.len()).map(|i| union.find(i)).collect()
}

/// Whether a closed route around the ring exists, stepping at most `step_m` in height
/// between neighbouring support faces — the rise converted by [`climb_voxels`], the same
/// conversion every other consumer of the step rule uses.
///
/// Answered on the **double cover**: the ring is unrolled twice, side by side, with no
/// wrap, and the question becomes whether some face in the first copy is connected to
/// the same face in the second. A path between the two is a walk that gained a whole
/// circumference of `x`, and since the ring's own graph is that walk glued end to end, it
/// is a closed route. Asking it this way costs two copies of the face list instead of a
/// copy per column of displacement.
pub fn around_the_ring(world: &World, step_m: f64) -> bool {
    let c = world.config().clone();
    let v = world.view();
    let (w, d) = (c.width as usize, c.depth as usize);
    let rise = i64::from(climb_voxels(step_m, c.voxel_m));

    // Support faces per column of the unrolled strip, ascending in y.
    let faces: Vec<Vec<u32>> = (0..w)
        .flat_map(|x| (0..d).map(move |z| (x, z)))
        .map(|(x, z)| v.supports_in_column(x as i64, z as u32))
        .collect();
    let column = |x: usize, z: usize| &faces[(x % w) * d + z];
    let total: usize = faces.iter().map(|f| f.len()).sum();
    if total == 0 {
        return false;
    }
    // Node ids over two copies of the ring: the first face of column `x, z` sits at
    // `offset[x * d + z]`.
    let mut offset = vec![0usize; 2 * w * d + 1];
    for x in 0..2 * w {
        for z in 0..d {
            offset[x * d + z + 1] = offset[x * d + z] + column(x, z).len();
        }
    }
    let nodes = offset[2 * w * d];
    let mut union = Union::new(nodes);
    let link = |union: &mut Union, ax: usize, az: usize, bx: usize, bz: usize| {
        let (a, b) = (column(ax, az), column(bx, bz));
        for (i, &ay) in a.iter().enumerate() {
            for (j, &by) in b.iter().enumerate() {
                if (ay as i64 - by as i64).abs() <= rise {
                    union.join(offset[ax * d + az] + i, offset[bx * d + bz] + j);
                }
            }
        }
    };
    for x in 0..2 * w {
        for z in 0..d {
            if x + 1 < 2 * w {
                link(&mut union, x, z, x + 1, z);
            }
            if z + 1 < d {
                link(&mut union, x, z, x, z + 1);
            }
        }
    }
    (0..w).any(|x| {
        (0..d).any(|z| {
            (0..column(x, z).len()).any(|i| {
                union.find(offset[x * d + z] + i) == union.find(offset[(x + w) * d + z] + i)
            })
        })
    })
}

/// Union-find with path halving. Small enough to keep here.
struct Union(Vec<usize>);

impl Union {
    fn new(n: usize) -> Union {
        Union((0..n).collect())
    }
    fn find(&mut self, mut i: usize) -> usize {
        while self.0[i] != i {
            self.0[i] = self.0[self.0[i]];
            i = self.0[i];
        }
        i
    }
    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.0[a] = b;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, Material, World};

    /// A flat ring of rock with a lid of open sky over it.
    fn flat(width: u32, depth: u32) -> World {
        let config = Config {
            width,
            height: 16,
            depth,
            voxel_m: 0.25,
            ..Config::default()
        };
        let mut world = World::empty(config.clone());
        for z in 0..depth {
            for x in 0..width as i64 {
                for y in 1..=4 {
                    world.material[config.index(x, y, z)] = Material::Rock;
                }
            }
        }
        world
    }

    fn raise(world: &mut World, x: i64, z: u32, to: u32) {
        let c = world.config().clone();
        for y in 1..=to {
            world.material[c.index(x, y, z)] = Material::Rock;
        }
    }

    /// Flat ground is walkable; a wall three voxels high across the whole depth is not,
    /// until a ramp column either side of it at the front turns the climb into steps.
    #[test]
    fn a_wall_across_the_strip_closes_the_ring_and_a_ramp_opens_it() {
        let step_m = 0.5;
        let mut world = flat(16, 4);
        assert!(
            around_the_ring(&world, step_m),
            "flat ground is walkable all the way round"
        );

        // A wall at x = 8: 4 -> 7 is three voxels, 0.75 m, over the 0.5 m bound.
        for z in 0..4 {
            raise(&mut world, 8, z, 7);
        }
        world.rebuild_active_sets();
        assert!(
            !around_the_ring(&world, step_m),
            "a 0.75 m wall is a wall, not a step"
        );

        // One column of ramp either side of it, at the front: 4 -> 6 -> 7 -> 6 -> 4,
        // every step within the bound.
        raise(&mut world, 7, 0, 6);
        raise(&mut world, 9, 0, 6);
        world.rebuild_active_sets();
        assert!(
            around_the_ring(&world, step_m),
            "the ramp at the front opens the route again"
        );
    }
}
