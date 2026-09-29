//! **The sleep pose** (WX4): what `State::Asleep` looks like on the voxel grid.
//!
//! An asleep animal is **lowered** onto the ground by up to [`MAX_DROP_ROWS`] rows, its legs
//! tucked (the rows the torso came down through are simply gone), and its back **breathes**:
//! a slow swell, one cell up, along the middle of the body, on a [`BREATH_PERIOD_S`] cycle.
//! Falling asleep and waking blend over [`BLEND_S`] seconds.
//!
//! # Why it is dithered
//!
//! The animal is voxels, and the presenter has no sub-voxel motion, so a row is the smallest
//! step there is. Rather than move a whole body a whole row at one instant (a pop), each
//! **column** of the body takes its own moment inside the blend, drawn from a hash of the
//! column: at blend `b` a column has lowered iff its hash is under `smooth(b)`. The body
//! settles column by column over the second, and the breath fills the same way, one cell at a
//! time along the back, and empties. The same trick the models' foliage thinning uses
//! ([`super::model::keeps`]). Everything here is a pure function of the pose and a seed, so a
//! frame is a function of its inputs and two runs draw the same picture.
//!
//! The pose works in the body's **own frame** (`a` along the body, `c` across it, `row` up
//! from the lowest cell), so one function serves the glyph path and the baked models, and
//! rotation to the heading stays where it was.

use std::collections::BTreeMap;

/// The most rows a body comes down. As far as its own legs allow: a body whose torso is
/// already on the ground (a crawler) does not move.
pub const MAX_DROP_ROWS: i32 = 2;
/// Seconds to fall asleep or wake.
pub const BLEND_S: f64 = 1.0;
/// Seconds per breath.
pub const BREATH_PERIOD_S: f64 = 4.0;
/// The part of the body's length, front to back, that breathes: the middle of the back.
const BREATH_BAND: (f32, f32) = (0.2, 0.8);

/// How asleep an animal is drawn this frame: both `0..=1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SleepPose {
    /// 0 awake, 1 fully lowered; moves at `1 / BLEND_S` per second toward its target.
    pub blend: f32,
    /// The breath's swell, 0 (out) to 1 (in), already scaled by nothing else.
    pub breath: f32,
}

impl SleepPose {
    pub const AWAKE: SleepPose = SleepPose {
        blend: 0.0,
        breath: 0.0,
    };

    pub fn is_awake(self) -> bool {
        !(self.blend > 0.0)
    }
}

/// One cell of a body in its own frame. `bump` says a breath may raise a cell above it (the
/// back, not a head or a sense patch).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Local {
    pub a: i32,
    pub c: i32,
    pub row: i32,
    pub bump: bool,
}

/// One cell of the posed body: `src` indexes the cell it came from (a breath cell is a copy
/// of the cell below it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Posed {
    pub src: usize,
    pub a: i32,
    pub c: i32,
    pub row: i32,
}

/// The breath at `time_s`, `0..=1`, out of step between animals by their id.
pub fn breath(time_s: f64, id: u64) -> f32 {
    let phase = hash01(id, 0, 0, 7) as f64;
    (0.5 - 0.5 * (std::f64::consts::TAU * (time_s / BREATH_PERIOD_S + phase)).cos()) as f32
}

/// The blend one step on: toward asleep or awake at `1 / BLEND_S` per second. Never more
/// than `dt_s / BLEND_S` from where it was, so it cannot jump.
pub fn advance_blend(blend: f32, asleep: bool, dt_s: f64) -> f32 {
    let step = (dt_s / BLEND_S) as f32;
    let target = if asleep { 1.0 } else { 0.0 };
    if blend < target {
        (blend + step).min(target)
    } else {
        (blend - step).max(target)
    }
}

fn smooth(b: f32) -> f32 {
    let b = b.clamp(0.0, 1.0);
    b * b * (3.0 - 2.0 * b)
}

/// A hash of a seed and a column, uniform in `[0, 1)`.
pub fn hash01(seed: u64, a: i32, c: i32, tag: u64) -> f32 {
    let mut z = seed
        ^ (a as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (c as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ tag.wrapping_mul(0x1656_67B1_9E37_79F9);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    ((z >> 40) as f32) / (1u32 << 24) as f32
}

/// The body under `pose`. An awake pose returns the cells as they were, in order.
pub fn pose(cells: &[Local], pose: SleepPose, seed: u64) -> Vec<Posed> {
    let identity = |c: &[Local]| {
        c.iter()
            .enumerate()
            .map(|(src, l)| Posed {
                src,
                a: l.a,
                c: l.c,
                row: l.row,
            })
            .collect::<Vec<_>>()
    };
    if pose.is_awake() || cells.is_empty() {
        return identity(cells);
    }
    // The torso is the fullest row, the lowest of a tie; the rows under it are legs.
    let mut per_row: BTreeMap<i32, usize> = BTreeMap::new();
    for l in cells {
        *per_row.entry(l.row).or_insert(0) += 1;
    }
    let most = per_row.values().copied().max().unwrap_or(0);
    let torso = per_row
        .iter()
        .find(|(_, n)| **n == most)
        .map(|(r, _)| *r)
        .unwrap_or(0);
    let lowest = *per_row.keys().next().expect("a cell");
    let drop = (torso - lowest).clamp(0, MAX_DROP_ROWS);
    let eased = smooth(pose.blend);
    let mut out = Vec::with_capacity(cells.len() + 8);
    for (src, l) in cells.iter().enumerate() {
        let down = hash01(seed, l.a, l.c, 0) < eased;
        let row = if !down {
            Some(l.row)
        } else if l.row < torso {
            // A leg: what is under the lowered torso stays, what the torso came down
            // through is tucked away.
            (l.row < torso - drop).then_some(l.row)
        } else {
            Some(l.row - drop)
        };
        if let Some(row) = row {
            out.push(Posed {
                src,
                a: l.a,
                c: l.c,
                row,
            });
        }
    }
    // The breath: along the middle of the back, one cell up, each column at its own point
    // of the swell.
    let amp = pose.breath.clamp(0.0, 1.0) * eased;
    if amp > 0.0 {
        let (lo, hi) = out
            .iter()
            .fold((i32::MAX, i32::MIN), |(lo, hi), p| (lo.min(p.a), hi.max(p.a)));
        let span = (hi - lo + 1) as f32;
        let mut top: BTreeMap<(i32, i32), i32> = BTreeMap::new();
        for p in &out {
            let t = top.entry((p.a, p.c)).or_insert(p.row);
            *t = (*t).max(p.row);
        }
        let mut swell = Vec::new();
        for p in &out {
            let f = (p.a - lo) as f32 + 0.5;
            let f = f / span;
            if !cells[p.src].bump
                || p.row != top[&(p.a, p.c)]
                || f < BREATH_BAND.0
                || f > BREATH_BAND.1
                // Only a column that has come down breathes: one still standing is not
                // yet asleep.
                || hash01(seed, p.a, p.c, 0) >= eased
                || hash01(seed, p.a, p.c, 1) >= amp
            {
                continue;
            }
            swell.push(Posed {
                row: p.row + 1,
                ..*p
            });
        }
        out.extend(swell);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A browser-like body: two legs a row under a 6 x 3 torso.
    fn body() -> Vec<Local> {
        let mut v = Vec::new();
        for a in 0..6 {
            for c in 0..3 {
                v.push(Local {
                    a,
                    c,
                    row: 2,
                    bump: a != 5,
                });
                if a == 0 || a == 5 {
                    for row in 0..2 {
                        v.push(Local {
                            a,
                            c,
                            row,
                            bump: false,
                        });
                    }
                }
            }
        }
        v
    }

    fn rows(cells: &[Local], posed: &[Posed]) -> Vec<(i32, i32, i32, usize)> {
        let mut v: Vec<_> = posed.iter().map(|p| (p.a, p.c, p.row, p.src)).collect();
        v.sort();
        let _ = cells;
        v
    }

    #[test]
    fn an_awake_pose_is_the_body_untouched() {
        let b = body();
        let p = pose(&b, SleepPose::AWAKE, 9);
        assert_eq!(p.len(), b.len());
        assert!(p.iter().all(|p| b[p.src].row == p.row && b[p.src].a == p.a));
        // A breath alone does nothing to an animal that is not asleep.
        let p = pose(
            &b,
            SleepPose {
                blend: 0.0,
                breath: 1.0,
            },
            9,
        );
        assert_eq!(p.len(), b.len());
    }

    #[test]
    fn an_asleep_body_is_lower_with_its_legs_tucked() {
        let b = body();
        let awake = pose(&b, SleepPose::AWAKE, 9);
        let asleep = pose(
            &b,
            SleepPose {
                blend: 1.0,
                breath: 0.0,
            },
            9,
        );
        assert_ne!(rows(&b, &awake), rows(&b, &asleep));
        let top = |p: &[Posed]| p.iter().map(|p| p.row).max().unwrap();
        assert_eq!(top(&awake), 2);
        assert_eq!(top(&asleep), 0, "the torso is on the ground");
        // The legs, the two rows the torso came down through, are gone.
        assert_eq!(asleep.len(), 18);
    }

    #[test]
    fn a_body_already_on_the_ground_does_not_move_but_still_breathes() {
        let crawler: Vec<Local> = (0..3)
            .map(|a| Local {
                a,
                c: 0,
                row: 0,
                bump: true,
            })
            .collect();
        let deep = SleepPose {
            blend: 1.0,
            breath: 1.0,
        };
        let p = pose(&crawler, deep, 3);
        assert!(p.iter().filter(|p| p.row == 0).count() == 3);
        assert!(p.iter().any(|p| p.row == 1), "the back swells");
    }

    /// The blend is continuous: the body's cells change by a few at a time as the blend
    /// crosses 0..1, the lowered columns only ever grow, and the ends are the two poses.
    #[test]
    fn the_blend_is_continuous_and_reaches_both_poses() {
        let b = body();
        let seed = 42;
        let at = |blend: f32| {
            pose(
                &b,
                SleepPose {
                    blend,
                    breath: 0.0,
                },
                seed,
            )
        };
        let down = |p: &[Posed]| p.iter().filter(|p| p.row < b[p.src].row).count();
        assert_eq!(rows(&b, &at(0.0)), rows(&b, &pose(&b, SleepPose::AWAKE, seed)));
        let (mut last, mut biggest) = (down(&at(0.0)), 0usize);
        let mut prev = at(0.0);
        for i in 1..=100 {
            let cur = at(i as f32 / 100.0);
            let n = down(&cur);
            assert!(n >= last, "a lowered cell came back up at {i}");
            let (x, y): (std::collections::BTreeSet<_>, std::collections::BTreeSet<_>) =
                (rows(&b, &cur).into_iter().collect(), rows(&b, &prev).into_iter().collect());
            let changed = x.symmetric_difference(&y).count();
            biggest = biggest.max(changed);
            last = n;
            prev = cur;
        }
        // A whole body is 54 cells; no one hundredth of the blend moves more than a
        // handful of them (a column of legs and torso is a few cells).
        assert!(biggest <= 12, "a step moved {biggest} cells at once");
        assert_eq!(
            rows(&b, &at(1.0)),
            rows(
                &b,
                &pose(
                    &b,
                    SleepPose {
                        blend: 1.0,
                        breath: 0.0
                    },
                    seed
                )
            )
        );
    }

    #[test]
    fn the_blend_moves_at_one_over_the_blend_time_and_never_jumps() {
        let mut b = 0.0;
        for _ in 0..10 {
            let next = advance_blend(b, true, 0.05);
            assert!(next - b <= (0.05 / BLEND_S) as f32 + 1e-6);
            b = next;
        }
        assert!((b - 0.5).abs() < 1e-5);
        for _ in 0..40 {
            b = advance_blend(b, true, 0.05);
        }
        assert_eq!(b, 1.0);
        for _ in 0..40 {
            b = advance_blend(b, false, 0.05);
        }
        assert_eq!(b, 0.0);
    }

    #[test]
    fn the_breath_has_a_four_second_period_and_swells_the_back_a_cell_at_a_time() {
        for id in [0u64, 5, 99] {
            let (a, b) = (breath(1.0, id), breath(1.0 + BREATH_PERIOD_S, id));
            assert!((a - b).abs() < 1e-4);
        }
        let b = body();
        let count = |breath: f32| {
            pose(
                &b,
                SleepPose {
                    blend: 1.0,
                    breath,
                },
                1,
            )
            .len()
        };
        assert_eq!(count(0.0), 18);
        let mut last = count(0.0);
        for i in 1..=50 {
            let n = count(i as f32 / 50.0);
            assert!(n >= last && n - last <= 4, "the swell grows a few cells at a time");
            last = n;
        }
        assert!(last > 18, "the back rose");
    }
}
