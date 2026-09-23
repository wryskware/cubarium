//! The two-voxel rule: nothing solid and nothing empty thinner than two voxels.
//!
//! A one-voxel fin, slot, sheet or slit is a resolution artifact, not a landform: it draws
//! as a four-pixel sliver, traps a column of water or lets one fall through a roof. The
//! landscape rethink (`design/handoffs/landscape-generation-rethink-2026-09-22.md`) found
//! all four on the panel's natural ring; the terrarium tidies them away before any water
//! datum is taken.
//!
//! The front and back walls count as solid, and `x` wraps. Bedrock is never touched.

use crate::{Config, Material};

/// One-voxel features, by kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Thin {
    /// Solid cells with void on both sides in `x`, or on both sides in `z`.
    pub fins: usize,
    /// Solid cells with void directly above and below.
    pub sheets: usize,
    /// Void cells with solid on both sides in `x`, or on both sides in `z`.
    pub slots: usize,
    /// Void cells with solid directly above and below.
    pub slits: usize,
}

impl Thin {
    pub fn total(&self) -> usize {
        self.fins + self.sheets + self.slots + self.slits
    }
}

/// Whether `(x, y, z)` is solid, with the front and back walls solid and above the world
/// void. Below the floor is solid.
fn solid(c: &Config, m: &[Material], x: i64, y: i64, z: i64) -> bool {
    if z < 0 || z >= c.depth as i64 || y < 0 {
        return true;
    }
    if y >= c.height as i64 {
        return false;
    }
    m[c.index(x, y as u32, z as u32)].is_solid()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Fin,
    Sheet,
    Slot,
    Slit,
}

fn classify(c: &Config, m: &[Material], x: i64, y: i64, z: i64) -> Option<Kind> {
    let s = |dx: i64, dy: i64, dz: i64| solid(c, m, x + dx, y + dy, z + dz);
    if s(0, 0, 0) {
        if y == 0 {
            return None;
        }
        if (!s(-1, 0, 0) && !s(1, 0, 0)) || (!s(0, 0, -1) && !s(0, 0, 1)) {
            return Some(Kind::Fin);
        }
        if !s(0, 1, 0) && !s(0, -1, 0) {
            return Some(Kind::Sheet);
        }
    } else {
        if (s(-1, 0, 0) && s(1, 0, 0)) || (s(0, 0, -1) && s(0, 0, 1)) {
            return Some(Kind::Slot);
        }
        if s(0, 1, 0) && s(0, -1, 0) {
            return Some(Kind::Slit);
        }
    }
    None
}

/// Count the one-voxel features in a material array.
pub fn count(c: &Config, m: &[Material]) -> Thin {
    let mut thin = Thin::default();
    for i in 0..m.len() {
        let (x, y, z) = c.coords(i);
        match classify(c, m, x as i64, y as i64, z as i64) {
            Some(Kind::Fin) => thin.fins += 1,
            Some(Kind::Sheet) => thin.sheets += 1,
            Some(Kind::Slot) => thin.slots += 1,
            Some(Kind::Slit) => thin.slits += 1,
            None => {}
        }
    }
    thin
}

/// Where the one-voxel features are, as `(x, y, z, kind)`. For diagnostics.
pub fn list(c: &Config, m: &[Material]) -> Vec<(u32, u32, u32, &'static str)> {
    let mut out = Vec::new();
    for i in 0..m.len() {
        let (x, y, z) = c.coords(i);
        let kind = match classify(c, m, x as i64, y as i64, z as i64) {
            Some(Kind::Fin) => "fin",
            Some(Kind::Sheet) => "sheet",
            Some(Kind::Slot) => "slot",
            Some(Kind::Slit) => "slit",
            None => continue,
        };
        out.push((x, y, z, kind));
    }
    out
}

/// Remove one-voxel features until none are left or `passes` run out, and say what was
/// changed. Cells `fixed` marks — water that must stay open, rims that must stay closed —
/// are never changed, so a feature leaning on one may survive.
///
/// A gap is filled. A loose fin or a stray sheet is cleared, and a cell cleared is never
/// filled again, so the passes cannot go round in circles; one braced between solids
/// (clearing it would leave a gap) or a sheet in a plate is thickened to two instead.
pub fn tidy(c: &Config, m: &mut [Material], fixed: &[bool], passes: usize) -> Thin {
    let mut changed = Thin::default();
    let mut cleared = vec![false; m.len()];
    for _ in 0..passes {
        let mut this = Thin::default();
        for i in 0..m.len() {
            if fixed[i] || m[i] == Material::Bedrock {
                continue;
            }
            let (x, y, z) = c.coords(i);
            let (xi, yi, zi) = (x as i64, y as i64, z as i64);
            let grow = |m: &mut [Material], x: i64, y: i64, z: i64| -> bool {
                if y <= 0 || y >= c.height as i64 || z < 0 || z >= c.depth as i64 {
                    return false;
                }
                let j = c.index(x, y as u32, z as u32);
                if fixed[j] || cleared[j] || m[j].is_solid() {
                    return false;
                }
                m[j] = Material::Rock;
                true
            };
            match classify(c, m, xi, yi, zi) {
                Some(Kind::Slot) | Some(Kind::Slit) if !cleared[i] => {
                    let slot = classify(c, m, xi, yi, zi) == Some(Kind::Slot);
                    m[i] = Material::Rock;
                    if slot {
                        this.slots += 1;
                    } else {
                        this.slits += 1;
                    }
                }
                Some(Kind::Fin) => {
                    let thin_x = !solid(c, m, xi - 1, yi, zi) && !solid(c, m, xi + 1, yi, zi);
                    let (dx, dz) = if thin_x { (1, 0) } else { (0, 1) };
                    if !gap_if_cleared(c, m, xi, yi, zi) {
                        m[i] = Material::Air;
                        cleared[i] = true;
                        this.fins += 1;
                    } else if grow(m, xi + dx, yi, zi + dz) || grow(m, xi - dx, yi, zi - dz) {
                        this.fins += 1;
                    }
                }
                Some(Kind::Sheet) => {
                    if !gap_if_cleared(c, m, xi, yi, zi) && !plate(c, m, xi, yi, zi) {
                        m[i] = Material::Air;
                        cleared[i] = true;
                        this.sheets += 1;
                    } else if grow(m, xi, yi - 1, zi) || grow(m, xi, yi + 1, zi) {
                        this.sheets += 1;
                    }
                }
                _ => {}
            }
        }
        changed.fins += this.fins;
        changed.sheets += this.sheets;
        changed.slots += this.slots;
        changed.slits += this.slits;
        if this.total() == 0 {
            break;
        }
    }
    changed
}

/// Whether the solid at `(x, y, z)` is part of a plate: solid on at least two sides.
fn plate(c: &Config, m: &[Material], x: i64, y: i64, z: i64) -> bool {
    [(-1, 0), (1, 0), (0, -1), (0, 1)]
        .iter()
        .filter(|&&(dx, dz)| solid(c, m, x + dx, y, z + dz))
        .count()
        >= 2
}

/// Whether clearing the solid at `(x, y, z)` would leave a one-voxel gap there.
fn gap_if_cleared(c: &Config, m: &[Material], x: i64, y: i64, z: i64) -> bool {
    let s = |dx: i64, dy: i64, dz: i64| solid(c, m, x + dx, y + dy, z + dz);
    (s(-1, 0, 0) && s(1, 0, 0)) || (s(0, 0, -1) && s(0, 0, 1)) || (s(0, 1, 0) && s(0, -1, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_config() -> Config {
        Config {
            width: 12,
            height: 10,
            depth: 8,
            ..Config::default()
        }
    }

    #[test]
    fn a_one_voxel_wall_is_a_fin_and_tidy_clears_it() {
        let c = box_config();
        let mut m = vec![Material::Air; c.cells()];
        for x in 0..12 {
            for z in 0..8 {
                m[c.index(x, 0, z)] = Material::Bedrock;
            }
        }
        for z in 0..8 {
            for y in 1..5 {
                m[c.index(5, y, z)] = Material::Rock;
            }
        }
        assert!(count(&c, &m).fins > 0);
        tidy(&c, &mut m, &vec![false; c.cells()], 4);
        assert_eq!(count(&c, &m).total(), 0);
    }

    #[test]
    fn a_one_voxel_gap_is_a_slot_and_tidy_fills_it_unless_fixed() {
        let c = box_config();
        let mut m = vec![Material::Air; c.cells()];
        for x in 0..12 {
            for z in 0..8 {
                for y in 0..4 {
                    m[c.index(x, y, z)] = if y == 0 {
                        Material::Bedrock
                    } else {
                        Material::Rock
                    };
                }
            }
        }
        for z in 0..8 {
            for y in 1..4 {
                m[c.index(6, y, z)] = Material::Air;
            }
        }
        assert!(count(&c, &m).slots > 0);
        let mut fixed = vec![false; c.cells()];
        for z in 0..8 {
            for y in 1..4 {
                fixed[c.index(6, y, z)] = true;
            }
        }
        let mut kept = m.clone();
        tidy(&c, &mut kept, &fixed, 4);
        assert_eq!(kept, m, "a fixed channel stays open");
        tidy(&c, &mut m, &vec![false; c.cells()], 4);
        assert_eq!(count(&c, &m).total(), 0);
    }
}
