//! The 70-scalar observation vector and its spatial sampler (contract §1–2).
//!
//! The sampler is **pure**: the world hands it the cells it already unfolded, the bounded
//! neighbour list it already built and the organism's own scalars, and it returns the vector.
//! Nothing here reads the world, draws from the RNG or allocates per animal. That is what
//! makes the geometry testable without a world, and what keeps a seam invisible: every offset
//! arrives already expressed in the observer's own chart, and the only thing this module does
//! with a chart is rotate out of it into the body frame.

use cubarium_surface::Vec2;

/// Length of the observation vector.
pub const OBS_LEN: usize = 70;
/// Sectors of the body frame, 60° each, sector 0 centred on the heading, numbered clockwise.
pub const SECTORS: usize = 6;
/// Half-width of a sector window, radians. Adjacent windows overlap by half.
const SECTOR_HALF_WIDTH: f64 = std::f64::consts::FRAC_PI_3;

/// Index of the first `food_near` value.
pub const FOOD_NEAR: usize = 3;
/// Index of the first `food_far` value.
pub const FOOD_FAR: usize = 21;
/// Index of the first `body` pair.
pub const BODY: usize = 39;
/// Index of `crowd.fwd`.
pub const CROWD: usize = 51;
/// Index of `motor_avail`.
pub const MOTOR_AVAIL: usize = 63;
/// Index of the first feedback value (`ate.graze`).
pub const FEEDBACK: usize = 64;

/// One sensed cell, already unfolded into the observer's chart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SensedCell {
    /// Cell centre minus observer position, in the observer's chart.
    pub offset: Vec2,
    /// `true` for the near ring (graph hop 1), `false` for the far ring (hops 2–3 merged).
    pub near: bool,
    /// Producer stock, material.
    pub p: f64,
    /// Fruit stock, material.
    pub f: f64,
    /// Edible detritus `D_eff = D · min(1, ρ/e_r)`, material.
    pub d_eff: f64,
}

/// One sensed neighbouring body, already unfolded into the observer's chart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SensedBody {
    /// Neighbour position minus observer position, in the observer's chart.
    pub offset: Vec2,
    /// Distance in the observer's chart, px.
    pub distance: f64,
    /// The neighbour's physical crowding extent, px.
    pub extent: f64,
}

/// Everything in the vector that is not spatial, in the world's own units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelfState {
    /// Own-cell stocks, material.
    pub p_here: f64,
    pub f_here: f64,
    pub d_here: f64,
    /// `cfg.producer.max`, the one fixed scale every food channel is divided by.
    pub p_max: f64,
    /// The repulsion sum the world already computes, in the observer's chart.
    pub crowd: Vec2,
    /// Own-cell water, material, and `cfg.water.flood`.
    pub water: f64,
    pub w_flood: f64,
    /// Own-cell sampled light this tick.
    pub light: f64,
    /// Embedded height, Top = 1, rim = −1.
    pub height: f64,
    /// Unit up-slope direction in the observer's chart; zero on the level top face.
    pub up: Vec2,
    /// The observer's own extent, px, and sensing radius, px.
    pub extent: f64,
    pub sense_radius: f64,
    /// Internal stores and development.
    pub reserve: f64,
    pub reserve_max: f64,
    pub energy: f64,
    pub energy_max: f64,
    pub structure: f64,
    pub structure_adult: f64,
    pub gestating: bool,
    pub age_seconds: f64,
    pub max_age_seconds: f64,
    /// `min(1, u_full / v_max)`: what this tick's motion budget actually allows, sharing the
    /// world's own `min(v_max / wading, affordable_motor)`. Already a ratio.
    pub motor_avail: f64,
    /// The six feedback channels, already reduced to their contract ranges.
    pub feedback: [f64; 6],
}

/// The finished vector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Observation70(pub [f64; OBS_LEN]);

impl Default for Observation70 {
    fn default() -> Self {
        Observation70([0.0; OBS_LEN])
    }
}

impl Observation70 {
    pub fn as_slice(&self) -> &[f64; OBS_LEN] {
        &self.0
    }
}

/// The raised-cosine sector weight of a body-frame angle `theta` for sector `k`
/// (contract §2): `½(1 + cos(π·Δ/60°))` inside the window, 0 outside. The six windows
/// partition unity, so a source is never lost or double counted and a small turn moves
/// weight smoothly between two sectors.
pub fn sector_weight(k: usize, theta: f64) -> f64 {
    let centre = (k as f64) * SECTOR_HALF_WIDTH;
    let delta = wrap_pi(theta - centre);
    if delta.abs() >= SECTOR_HALF_WIDTH {
        0.0
    } else {
        0.5 * (1.0 + (std::f64::consts::PI * delta / SECTOR_HALF_WIDTH).cos())
    }
}

/// Build the whole observation vector. `heading` is the observer's unit heading in its own
/// chart; every offset in `cells` and `bodies` is in that same chart.
pub fn observe(
    heading: Vec2,
    cells: &[SensedCell],
    bodies: &[SensedBody],
    s: &SelfState,
) -> Observation70 {
    let mut v = [0.0f64; OBS_LEN];
    let basis = Basis::of(heading);
    let p_max = if s.p_max.is_finite() && s.p_max > 0.0 {
        s.p_max
    } else {
        1.0
    };

    v[0] = ratio(s.p_here, p_max);
    v[1] = ratio(s.f_here, p_max);
    v[2] = ratio(s.d_here, p_max);

    // Food rings: a weighted *mean* over the cells present, so a sector's value does not grow
    // with how many cells happen to fall in it. Absent cells (past the rim, or beyond the
    // body's hop count) contribute nothing and read as 0, exactly as an empty cell does.
    let mut sums = [[0.0f64; 3]; 2 * SECTORS];
    let mut weights = [0.0f64; 2 * SECTORS];
    for cell in cells {
        let Some(theta) = basis.angle(cell.offset) else {
            continue;
        };
        let ring = usize::from(!cell.near);
        for k in 0..SECTORS {
            let w = sector_weight(k, theta);
            if w <= 0.0 {
                continue;
            }
            let slot = ring * SECTORS + k;
            weights[slot] += w;
            sums[slot][0] += w * cell.p;
            sums[slot][1] += w * cell.f;
            sums[slot][2] += w * cell.d_eff;
        }
    }
    for ring in 0..2 {
        let base = if ring == 0 { FOOD_NEAR } else { FOOD_FAR };
        for k in 0..SECTORS {
            let slot = ring * SECTORS + k;
            if weights[slot] <= 0.0 {
                continue;
            }
            for c in 0..3 {
                v[base + 3 * k + c] = ratio(sums[slot][c] / weights[slot], p_max);
            }
        }
    }

    // Bodies: presence and observable relative size only. No id, genome, energy or role.
    //
    // **Ties are resolved by list order.** The neighbour list arrives sorted by
    // `(distance, id)` and the strict `>` below keeps the *first* contributor that attains the
    // maximum, so two neighbours with an identical weighted presence are separated by
    // distance and then by their full `OrganismId` — deterministic, and independent of slot
    // reuse order.
    let r_sense = if s.sense_radius.is_finite() && s.sense_radius > 0.0 {
        s.sense_radius
    } else {
        0.0
    };
    for body in bodies {
        let Some(theta) = basis.angle(body.offset) else {
            continue;
        };
        let near = if r_sense > 0.0 {
            (1.0 - body.distance / r_sense).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if near <= 0.0 {
            continue;
        }
        for k in 0..SECTORS {
            let presence = sector_weight(k, theta) * near;
            if presence > v[BODY + 2 * k] {
                v[BODY + 2 * k] = presence;
                let mine = if s.extent.is_finite() && s.extent > 0.0 {
                    s.extent
                } else {
                    0.0
                };
                let theirs = if body.extent.is_finite() && body.extent > 0.0 {
                    body.extent
                } else {
                    0.0
                };
                let sum = mine + theirs;
                v[BODY + 2 * k + 1] = if sum > 0.0 {
                    (theirs / sum).clamp(0.0, 1.0)
                } else {
                    0.0
                };
            }
        }
    }

    // Crowd: the world's own repulsion sum, rotated into the body frame and squashed by
    // magnitude so the direction survives and the magnitude saturates.
    let crowd = basis.rotate(s.crowd);
    let len = crowd.length();
    if len.is_finite() && len > 0.0 {
        let scale = 1.0 / (1.0 + len);
        v[CROWD] = (crowd.x * scale).clamp(-1.0, 1.0);
        v[CROWD + 1] = (crowd.y * scale).clamp(-1.0, 1.0);
    }

    v[53] = ratio(s.water, if s.w_flood > 0.0 { s.w_flood } else { 1.0 });
    v[54] = clamp01(s.light);
    v[55] = clamp_unit(s.height);
    let up = basis.rotate(s.up);
    v[56] = clamp_unit(up.x);
    v[57] = clamp_unit(up.y);

    v[58] = ratio(s.reserve, s.reserve_max);
    v[59] = ratio(s.energy, s.energy_max);
    v[60] = ratio(s.structure, s.structure_adult);
    v[61] = f64::from(u8::from(s.gestating));
    v[62] = ratio(s.age_seconds, s.max_age_seconds);
    v[MOTOR_AVAIL] = clamp01(s.motor_avail);

    v[FEEDBACK] = clamp01(s.feedback[0]);
    v[FEEDBACK + 1] = clamp01(s.feedback[1]);
    v[FEEDBACK + 2] = clamp01(s.feedback[2]);
    v[FEEDBACK + 3] = clamp01(s.feedback[3]);
    v[FEEDBACK + 4] = clamp_unit(s.feedback[4]);
    v[FEEDBACK + 5] = clamp01(s.feedback[5]);

    Observation70(v)
}

/// The observer's body frame: `+x` along the heading, `+y` its clockwise side on screen.
#[derive(Clone, Copy, Debug)]
struct Basis {
    fwd: Vec2,
    side: Vec2,
}

impl Basis {
    fn of(heading: Vec2) -> Basis {
        let fwd = heading.normalized().unwrap_or(Vec2::new(1.0, 0.0));
        // Screen coordinates run `y` downward, so the clockwise perpendicular of `(x, y)` is
        // `(−y, x)`: heading right gives side down, which is the quarter turn clockwise.
        Basis {
            fwd,
            side: Vec2::new(-fwd.y, fwd.x),
        }
    }

    fn rotate(&self, v: Vec2) -> Vec2 {
        Vec2::new(v.dot(self.fwd), v.dot(self.side))
    }

    /// Body-frame bearing of an offset, clockwise-positive, or `None` for a degenerate one.
    fn angle(&self, offset: Vec2) -> Option<f64> {
        let r = self.rotate(offset);
        if !r.x.is_finite() || !r.y.is_finite() || r.length_sq() <= f64::EPSILON {
            return None;
        }
        Some(r.y.atan2(r.x))
    }
}

fn ratio(x: f64, scale: f64) -> f64 {
    if !x.is_finite() || !scale.is_finite() || scale <= 0.0 {
        return 0.0;
    }
    (x / scale).clamp(0.0, 1.0)
}

fn clamp01(x: f64) -> f64 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn clamp_unit(x: f64) -> f64 {
    if x.is_finite() {
        x.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

fn wrap_pi(a: f64) -> f64 {
    use std::f64::consts::{PI, TAU};
    if !a.is_finite() {
        return 0.0;
    }
    let mut a = (a + PI).rem_euclid(TAU) - PI;
    if a <= -PI {
        a += TAU;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bare() -> SelfState {
        SelfState {
            p_here: 0.0,
            f_here: 0.0,
            d_here: 0.0,
            p_max: 1.5,
            crowd: Vec2::ZERO,
            water: 0.0,
            w_flood: 1.5,
            light: 0.0,
            height: 0.0,
            up: Vec2::ZERO,
            extent: 2.5,
            sense_radius: 8.0,
            reserve: 0.0,
            reserve_max: 1.0,
            energy: 0.0,
            energy_max: 1.0,
            structure: 1.0,
            structure_adult: 1.0,
            gestating: false,
            age_seconds: 0.0,
            max_age_seconds: 1.0,
            motor_avail: 0.0,
            feedback: [0.0, 0.0, 0.0, 0.0, 0.0, 1.0],
        }
    }

    #[test]
    fn the_sector_windows_partition_unity() {
        for step in 0..720 {
            let theta = -std::f64::consts::PI + std::f64::consts::TAU * f64::from(step) / 720.0;
            let sum: f64 = (0..SECTORS).map(|k| sector_weight(k, theta)).sum();
            assert!(
                (sum - 1.0).abs() < 1e-12,
                "Σ w_k({theta}) = {sum}, not 1: a source would be lost or double counted"
            );
        }
    }

    #[test]
    fn a_lone_cell_dead_ahead_lands_in_sector_zero_only() {
        let cells = [SensedCell {
            offset: Vec2::new(4.0, 0.0),
            near: true,
            p: 1.5,
            f: 0.0,
            d_eff: 0.0,
        }];
        let o = observe(Vec2::new(1.0, 0.0), &cells, &[], &bare());
        assert!((o.0[FOOD_NEAR] - 1.0).abs() < 1e-12, "sector 0 carries it");
        for k in 1..SECTORS {
            assert_eq!(o.0[FOOD_NEAR + 3 * k], 0.0, "sector {k} is empty");
        }
        assert!(o.0[FOOD_FAR..FOOD_FAR + 18].iter().all(|x| *x == 0.0));
    }

    #[test]
    fn rotating_the_observer_by_sixty_degrees_shifts_every_sector_by_one_index() {
        // One cell per 60° bearing, so every sector carries a distinct value.
        let cells: Vec<SensedCell> = (0..SECTORS)
            .map(|k| {
                let a = SECTOR_HALF_WIDTH * k as f64;
                SensedCell {
                    offset: Vec2::new(4.0 * a.cos(), 4.0 * a.sin()),
                    near: true,
                    p: 0.1 + 0.2 * k as f64,
                    f: 0.0,
                    d_eff: 0.0,
                }
            })
            .collect();
        let straight = observe(Vec2::new(1.0, 0.0), &cells, &[], &bare());
        // Turning the body 60° clockwise moves each source one sector *anticlockwise*.
        let turned = observe(
            Vec2::new(SECTOR_HALF_WIDTH.cos(), SECTOR_HALF_WIDTH.sin()),
            &cells,
            &[],
            &bare(),
        );
        for k in 0..SECTORS {
            let before = straight.0[FOOD_NEAR + 3 * ((k + 1) % SECTORS)];
            let after = turned.0[FOOD_NEAR + 3 * k];
            assert!(
                (before - after).abs() < 1e-9,
                "sector {k} after the turn is {after}, sector {} before was {before}",
                (k + 1) % SECTORS
            );
        }
    }

    #[test]
    fn a_sector_value_is_a_mean_not_a_sum() {
        let one = [SensedCell {
            offset: Vec2::new(4.0, 0.0),
            near: true,
            p: 0.75,
            f: 0.0,
            d_eff: 0.0,
        }];
        let three: Vec<SensedCell> = (0..3).map(|_| one[0]).collect();
        let a = observe(Vec2::new(1.0, 0.0), &one, &[], &bare());
        let b = observe(Vec2::new(1.0, 0.0), &three, &[], &bare());
        assert!((a.0[FOOD_NEAR] - b.0[FOOD_NEAR]).abs() < 1e-12);
        assert!(
            (a.0[FOOD_NEAR] - 0.5).abs() < 1e-12,
            "0.75 m of a 1.5 m ceiling"
        );
    }

    #[test]
    fn presence_decays_to_zero_at_the_sensing_radius() {
        let mut s = bare();
        s.sense_radius = 8.0;
        for (distance, want) in [(0.0f64, 1.0f64), (4.0, 0.5), (8.0, 0.0), (9.0, 0.0)] {
            let bodies = [SensedBody {
                offset: Vec2::new(distance.max(1e-6), 0.0),
                distance,
                extent: 2.5,
            }];
            let o = observe(Vec2::new(1.0, 0.0), &[], &bodies, &s);
            assert!(
                (o.0[BODY] - want).abs() < 1e-6,
                "presence at {distance} px is {} not {want}",
                o.0[BODY]
            );
        }
    }

    #[test]
    fn relative_size_is_gated_on_presence_and_is_the_share_of_the_extent_sum() {
        let s = bare();
        let bodies = [SensedBody {
            offset: Vec2::new(2.0, 0.0),
            distance: 2.0,
            extent: 7.5,
        }];
        let o = observe(Vec2::new(1.0, 0.0), &[], &bodies, &s);
        assert!((o.0[BODY + 1] - 0.75).abs() < 1e-12, "7.5 / (2.5 + 7.5)");
        for k in 1..SECTORS {
            // No presence in the other sectors, so no size either.
            if o.0[BODY + 2 * k] == 0.0 {
                assert_eq!(o.0[BODY + 2 * k + 1], 0.0);
            }
        }
    }

    #[test]
    fn a_one_hop_body_has_an_all_zero_far_ring() {
        let cells = [SensedCell {
            offset: Vec2::new(4.0, 0.0),
            near: true,
            p: 1.5,
            f: 1.5,
            d_eff: 1.5,
        }];
        let o = observe(Vec2::new(1.0, 0.0), &cells, &[], &bare());
        assert!(o.0[FOOD_FAR..FOOD_FAR + 18].iter().all(|x| *x == 0.0));
    }

    #[test]
    fn every_value_is_finite_and_inside_its_stated_range() {
        let mut s = bare();
        s.crowd = Vec2::new(1e9, -1e9);
        s.up = Vec2::new(0.6, -0.8);
        s.light = 12.0;
        s.height = -4.0;
        s.water = 99.0;
        s.feedback = [9.0, -9.0, f64::NAN, 4.0, -4.0, 4.0];
        let cells = [SensedCell {
            offset: Vec2::new(0.0, 0.0),
            near: true,
            p: f64::NAN,
            f: 1e9,
            d_eff: -3.0,
        }];
        let bodies = [SensedBody {
            offset: Vec2::new(1.0, 1.0),
            distance: 1.414,
            extent: f64::INFINITY,
        }];
        let o = observe(Vec2::new(0.0, 0.0), &cells, &bodies, &s);
        for (i, x) in o.0.iter().enumerate() {
            assert!(x.is_finite(), "index {i} is {x}");
            assert!((-1.0..=1.0).contains(x), "index {i} is {x}");
        }
    }
}
