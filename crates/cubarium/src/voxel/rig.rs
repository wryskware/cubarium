//! **Rigs** (FEED-RIG, `design/handoffs/feeding-motion-rig-2026-10-09.md`): a founder's bones
//! and its sampled feeding clip, baked beside its model, and the per-frame bone transforms
//! the model pass draws a feeding animal with (`[figures] rig`, a trial setting).
//!
//! # What a rig is
//!
//! The Blender builder (`scripts/blender/animal_models.py`, `fg_feed_*`) draws the founder
//! with an armature: every part on one bone, and one action (the feeding loop in the reached
//! posture). The bake (`scripts/blender/bake_voxel_models.py`) records each cell's bone the
//! way it records its material, and samples the action at its frame rate: per frame, per
//! bone, its turn about its own pivot relative to its parent, and the pivot's displacement.
//!
//! # Drawing it
//!
//! A cropping animal's **clip weight** eases 0 → 1 over [`FEED_EASE_S`] and back after
//! (`Animals::feed_weight`, as the meal flash's blend), eased by a smoothstep ([`ease`]).
//! Each bone's turn is eased from none by a slerp and its displacement by the weight, about
//! its own pivot, and composed down the chain ([`AnimalRig::pose`]): the weight's ease is the
//! reach to the ground and the return to rest. The clip runs on the frame clock, each
//! animal at its own phase ([`phase_offset_s`]), so two grazers are out of step.
//!
//! # The file format
//!
//! `assets/voxel-models/<voxel_mm>/<name>.rig`, named under the manifest's `rigs`; rebaked
//! with the model, never migrated. Little-endian:
//!
//! ```text
//! b"CRG1"
//! n_bones u8, then per bone: len u8, name bytes, parent u8 (255: none), head f32 x 3
//!     (the bone's pivot, the model's axes — x forward, y up, z side — in metres at the
//!     builder's reference size, from the anchor)
//! fps f32, n_frames u16, then per frame, per bone: rotation f32 x 9 (row-major, the model's
//!     axes; its turn about its pivot, relative to its parent), displacement f32 x 3 (metres)
//! n_bins u8, then per bin: length_m f64, scale f32 (voxels per reference metre),
//!     n_cells u32, then per cell: dx i8, dy i8, dz i8, bone u8 (the `.cvm` bin's cells)
//! ```

use anyhow::{Context, Result, bail};
use rustc_hash::FxHashMap;

/// Seconds the clip weight takes to ease in (the reach) and out (the return to rest). The
/// builder's `FG_FEED_EASE_S`, which the flipbook harness mirrors, is the same.
pub const FEED_EASE_S: f64 = 0.4;

/// One bone: its name, its parent, its pivot (the model's axes, reference metres).
#[derive(Clone, Debug, PartialEq)]
pub struct RigBone {
    pub name: String,
    pub parent: Option<usize>,
    pub head_m: [f64; 3],
}

/// One bone at one sampled frame: its turn (a unit quaternion `[w, x, y, z]`) and its
/// pivot's displacement (reference metres).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Key {
    q: [f64; 4],
    d: [f64; 3],
}

/// One size bin: its body length, its scale (voxels per reference metre), each cell's bone.
#[derive(Clone, Debug, PartialEq)]
pub struct RigBin {
    pub length_m: f64,
    pub scale: f64,
    bones: FxHashMap<[i32; 3], u8>,
}

/// A founder's rig: its bones, its clip, its bins.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimalRig {
    pub bones: Vec<RigBone>,
    pub fps: f64,
    frames: usize,
    keys: Vec<Key>,
    pub bins: Vec<RigBin>,
}

/// One bone's transform in a bin's voxels: rows `[m0, m1, m2, t]`, a rest point `p` of the
/// model's frame going to `M p + t` (`cubarium_gpu::figures::FigureFrame::bones`).
pub type BoneRows = [[f32; 4]; 3];

impl AnimalRig {
    /// Seconds the sampled loop lasts.
    pub fn loop_s(&self) -> f64 {
        self.frames as f64 / self.fps
    }

    /// Frames sampled.
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// The bin of the model cells `length_m` selects (`AnimalModel::select`'s rule).
    pub fn select(&self, length_m: f64) -> Option<usize> {
        super::model::nearest(self.bins.iter().map(|b| b.length_m), length_m)
    }

    /// The bone of bin `bin`'s cell at `offset`: 0 (the root) for a cell the rig does not
    /// name.
    pub fn bone_of(&self, bin: usize, offset: [i32; 3]) -> u8 {
        self.bins
            .get(bin)
            .and_then(|b| b.bones.get(&offset))
            .copied()
            .unwrap_or(0)
    }

    /// Every bone's transform in bin `bin`'s voxels at `phase_s` seconds into the loop
    /// (between its sampled frames: turns slerped, displacements mixed) with clip weight
    /// `w` (0 the rest pose, 1 the clip), into `out` (cleared): each bone's turn eased from
    /// none by a slerp and its displacement by `w`, about its own pivot, composed down the
    /// chain (a parent before its children, as the bake writes them). The builder's
    /// `fg_feed_pose` is the same sum.
    pub fn pose(&self, bin: usize, phase_s: f64, w: f64, out: &mut Vec<BoneRows>) {
        out.clear();
        let n = self.bones.len();
        let Some(b) = self.bins.get(bin) else { return };
        if n == 0 || self.frames == 0 {
            return;
        }
        let at = (phase_s * self.fps).rem_euclid(self.frames as f64);
        let i0 = (at.floor() as usize).min(self.frames - 1);
        let i1 = (i0 + 1) % self.frames;
        let u = at - i0 as f64;
        let w = w.clamp(0.0, 1.0);
        let mut chain: Vec<([[f64; 3]; 3], [f64; 3])> = Vec::with_capacity(n);
        for (k, bone) in self.bones.iter().enumerate() {
            let (a, c) = (self.keys[i0 * n + k], self.keys[i1 * n + k]);
            let q = slerp([1.0, 0.0, 0.0, 0.0], slerp(a.q, c.q, u), w);
            let r = mat_of(q);
            let h = bone.head_m.map(|v| v * b.scale);
            let d = [0, 1, 2].map(|i| (a.d[i] + (c.d[i] - a.d[i]) * u) * w * b.scale);
            // L(x) = R (x − h) + h + d
            let rh = mul(r, h);
            let t = [0, 1, 2].map(|i| h[i] + d[i] - rh[i]);
            let (m, t) = match bone.parent.filter(|&p| p < k) {
                Some(p) => {
                    let (pm, pt) = chain[p];
                    let pt2 = mul(pm, t);
                    (matmul(pm, r), [0, 1, 2].map(|i| pt2[i] + pt[i]))
                }
                None => (r, t),
            };
            chain.push((m, t));
            out.push(
                [0, 1, 2].map(|i| [m[i][0] as f32, m[i][1] as f32, m[i][2] as f32, t[i] as f32]),
            );
        }
    }

    /// Read one `.rig` file (the module header has the layout).
    pub fn parse(bytes: &[u8]) -> Result<AnimalRig> {
        let mut r = Reader { bytes, at: 0 };
        if r.take(4)? != b"CRG1" {
            bail!("not a CRG1 rig file");
        }
        let n = usize::from(r.u8()?);
        if n == 0 || n > usize::from(cubarium_gpu::figures::BONE_LIMIT) {
            bail!("{n} bones");
        }
        let mut bones = Vec::with_capacity(n);
        for k in 0..n {
            let len = usize::from(r.u8()?);
            let name = std::str::from_utf8(r.take(len)?)
                .context("a bone name")?
                .to_string();
            let parent = match r.u8()? {
                255 => None,
                p if usize::from(p) < k => Some(usize::from(p)),
                p => bail!("bone {name}'s parent {p} is not before it"),
            };
            let head_m = [r.f32()?, r.f32()?, r.f32()?];
            bones.push(RigBone {
                name,
                parent,
                head_m,
            });
        }
        let fps = r.f32()?;
        if !(fps > 0.0) {
            bail!("frame rate {fps}");
        }
        let frames = usize::from(r.u16()?);
        let mut keys = Vec::with_capacity(frames * n);
        for _ in 0..frames * n {
            let mut m = [[0.0; 3]; 3];
            for row in &mut m {
                for v in row.iter_mut() {
                    *v = r.f32()?;
                }
            }
            let d = [r.f32()?, r.f32()?, r.f32()?];
            keys.push(Key { q: quat_of(m), d });
        }
        let n_bins = usize::from(r.u8()?);
        let mut bins = Vec::with_capacity(n_bins);
        for _ in 0..n_bins {
            let length_m = r.f64()?;
            let scale = r.f32()?;
            let cells = r.u32()? as usize;
            let raw = r.take(cells.checked_mul(4).context("cell count")?)?;
            let mut map = FxHashMap::default();
            for c in raw.chunks_exact(4) {
                if usize::from(c[3]) >= n {
                    bail!("cell bone {} of {n}", c[3]);
                }
                map.insert([c[0] as i8, c[1] as i8, c[2] as i8].map(i32::from), c[3]);
            }
            bins.push(RigBin {
                length_m,
                scale,
                bones: map,
            });
        }
        if r.at != bytes.len() {
            bail!("{} trailing bytes", bytes.len() - r.at);
        }
        Ok(AnimalRig {
            bones,
            fps,
            frames,
            keys,
            bins,
        })
    }
}

/// The smoothstep a clip weight is drawn at: the reach and return start and stop gently.
pub fn ease(w: f64) -> f64 {
    let w = w.clamp(0.0, 1.0);
    w * w * (3.0 - 2.0 * w)
}

/// Seconds animal `id`'s clip runs ahead of the frame clock, `0..loop_s`: its own phase,
/// so animals that start cropping together are out of step.
pub fn phase_offset_s(id: u64, loop_s: f64) -> f64 {
    let mut z = id.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64 * loop_s
}

/// Where a clip with weight `w` (linear, before [`ease`]) is after `dt` seconds of the body
/// cropping (`true`, toward 1) or not (toward 0).
pub fn advance_weight(w: f32, cropping: bool, dt: f64) -> f32 {
    let step = if FEED_EASE_S > 0.0 {
        (dt / FEED_EASE_S) as f32
    } else {
        1.0
    };
    (if cropping { w + step } else { w - step }).clamp(0.0, 1.0)
}

// --- rotations ----------------------------------------------------------------------------

fn mul(m: [[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

fn matmul(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    [0, 1, 2].map(|i| [0, 1, 2].map(|j| a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j]))
}

/// A rotation matrix's unit quaternion `[w, x, y, z]`.
fn quat_of(m: [[f64; 3]; 3]) -> [f64; 4] {
    let tr = m[0][0] + m[1][1] + m[2][2];
    let q = if tr > 0.0 {
        let s = (tr + 1.0).sqrt() * 2.0;
        [
            0.25 * s,
            (m[2][1] - m[1][2]) / s,
            (m[0][2] - m[2][0]) / s,
            (m[1][0] - m[0][1]) / s,
        ]
    } else if m[0][0] > m[1][1] && m[0][0] > m[2][2] {
        let s = (1.0 + m[0][0] - m[1][1] - m[2][2]).sqrt() * 2.0;
        [
            (m[2][1] - m[1][2]) / s,
            0.25 * s,
            (m[0][1] + m[1][0]) / s,
            (m[0][2] + m[2][0]) / s,
        ]
    } else if m[1][1] > m[2][2] {
        let s = (1.0 + m[1][1] - m[0][0] - m[2][2]).sqrt() * 2.0;
        [
            (m[0][2] - m[2][0]) / s,
            (m[0][1] + m[1][0]) / s,
            0.25 * s,
            (m[1][2] + m[2][1]) / s,
        ]
    } else {
        let s = (1.0 + m[2][2] - m[0][0] - m[1][1]).sqrt() * 2.0;
        [
            (m[1][0] - m[0][1]) / s,
            (m[0][2] + m[2][0]) / s,
            (m[1][2] + m[2][1]) / s,
            0.25 * s,
        ]
    };
    let l = q.iter().map(|v| v * v).sum::<f64>().sqrt();
    q.map(|v| v / l)
}

fn mat_of(q: [f64; 4]) -> [[f64; 3]; 3] {
    let [w, x, y, z] = q;
    [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - w * z),
            2.0 * (x * z + w * y),
        ],
        [
            2.0 * (x * y + w * z),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - w * x),
        ],
        [
            2.0 * (x * z - w * y),
            2.0 * (y * z + w * x),
            1.0 - 2.0 * (x * x + y * y),
        ],
    ]
}

/// The turn `t` of the way from `a` to `b`, the short way round.
fn slerp(a: [f64; 4], b: [f64; 4], t: f64) -> [f64; 4] {
    let mut dot: f64 = (0..4).map(|i| a[i] * b[i]).sum();
    let b = if dot < 0.0 {
        dot = -dot;
        b.map(|v| -v)
    } else {
        b
    };
    let (ka, kb) = if dot > 0.9995 {
        (1.0 - t, t)
    } else {
        let th = dot.clamp(-1.0, 1.0).acos();
        let s = th.sin();
        (((1.0 - t) * th).sin() / s, (t * th).sin() / s)
    };
    let q = [0, 1, 2, 3].map(|i| ka * a[i] + kb * b[i]);
    let l = q.iter().map(|v| v * v).sum::<f64>().sqrt();
    q.map(|v| v / l)
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(n).context("length")?;
        let Some(s) = self.bytes.get(self.at..end) else {
            bail!("truncated at byte {}", self.at);
        };
        self.at = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn f32(&mut self) -> Result<f64> {
        Ok(f64::from(f32::from_le_bytes(self.take(4)?.try_into()?)))
    }
    fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.take(8)?.try_into()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-bone rig: a root, and a child pivoting at (1, 0, 0) that turns a quarter about
    /// +y over its one-frame clip, in a bin of `scale` voxels a metre.
    fn rig(scale: f64) -> AnimalRig {
        let quarter = quat_of([[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]]);
        AnimalRig {
            bones: vec![
                RigBone {
                    name: "root".into(),
                    parent: None,
                    head_m: [0.0; 3],
                },
                RigBone {
                    name: "arm".into(),
                    parent: Some(0),
                    head_m: [1.0, 0.0, 0.0],
                },
            ],
            fps: 1.0,
            frames: 1,
            keys: vec![
                Key {
                    q: [1.0, 0.0, 0.0, 0.0],
                    d: [0.0; 3],
                },
                Key {
                    q: quarter,
                    d: [0.0; 3],
                },
            ],
            bins: vec![RigBin {
                length_m: 1.0,
                scale,
                bones: FxHashMap::default(),
            }],
        }
    }

    fn apply(b: &BoneRows, p: [f64; 3]) -> [f64; 3] {
        [0, 1, 2].map(|i| {
            f64::from(b[i][0]) * p[0]
                + f64::from(b[i][1]) * p[1]
                + f64::from(b[i][2]) * p[2]
                + f64::from(b[i][3])
        })
    }

    fn near(a: [f64; 3], b: [f64; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-5)
    }

    #[test]
    fn a_bone_turns_about_its_own_pivot_and_its_pivot_stays() {
        let r = rig(2.0);
        let mut out = Vec::new();
        r.pose(0, 0.0, 1.0, &mut out);
        // the pivot, at 1 m = 2 voxels, does not move; a point a voxel beyond it turns
        // a quarter about +y: +x goes to −z
        assert!(near(apply(&out[1], [2.0, 0.0, 0.0]), [2.0, 0.0, 0.0]));
        assert!(near(apply(&out[1], [3.0, 0.0, 0.0]), [2.0, 0.0, -1.0]));
        assert!(
            near(apply(&out[0], [3.0, 1.0, 0.0]), [3.0, 1.0, 0.0]),
            "the root stays"
        );
    }

    #[test]
    fn weight_zero_is_the_rest_pose_and_a_half_weight_turns_half_way() {
        let r = rig(1.0);
        let mut out = Vec::new();
        r.pose(0, 0.0, 0.0, &mut out);
        assert!(near(apply(&out[1], [2.0, 0.0, 0.0]), [2.0, 0.0, 0.0]));
        r.pose(0, 0.0, 0.5, &mut out);
        let h = std::f64::consts::FRAC_1_SQRT_2;
        assert!(near(apply(&out[1], [2.0, 0.0, 0.0]), [1.0 + h, 0.0, -h]));
    }

    #[test]
    fn a_child_rides_on_its_parents_turn() {
        let mut r = rig(1.0);
        r.bones.push(RigBone {
            name: "hand".into(),
            parent: Some(1),
            head_m: [2.0, 0.0, 0.0],
        });
        r.keys.push(Key {
            q: [1.0, 0.0, 0.0, 0.0],
            d: [0.0; 3],
        });
        let mut out = Vec::new();
        r.pose(0, 0.0, 1.0, &mut out);
        // the hand's pivot (2, 0, 0) is carried by the arm's quarter turn to (1, 0, −1)
        assert!(near(apply(&out[2], [2.0, 0.0, 0.0]), [1.0, 0.0, -1.0]));
    }

    #[test]
    fn the_clip_moves_between_its_frames_and_loops() {
        let mut r = rig(1.0);
        let quarter = r.keys[1];
        r.frames = 2;
        r.keys = vec![
            Key {
                q: [1.0, 0.0, 0.0, 0.0],
                d: [0.0; 3],
            },
            Key {
                q: [1.0, 0.0, 0.0, 0.0],
                d: [0.0; 3],
            },
            r.keys[0],
            quarter,
        ];
        let mut out = Vec::new();
        let tip = |out: &Vec<BoneRows>| apply(&out[1], [2.0, 0.0, 0.0]);
        r.pose(0, 0.5, 1.0, &mut out);
        let half = tip(&out);
        r.pose(0, 1.0, 1.0, &mut out);
        assert!(
            near(tip(&out), [1.0, 0.0, -1.0]),
            "frame 1 is the quarter turn"
        );
        r.pose(0, 2.0, 1.0, &mut out);
        assert!(
            near(tip(&out), [2.0, 0.0, 0.0]),
            "the loop comes back to frame 0"
        );
        assert!(
            half[2] < 0.0 && half[2] > -1.0,
            "half way between frames: {half:?}"
        );
    }

    #[test]
    fn a_rig_file_round_trips_its_bones_and_cells() {
        let mut b = b"CRG1".to_vec();
        b.push(2);
        for (name, parent, head) in [("root", 255u8, [0.0f32; 3]), ("arm", 0, [1.0, 0.0, 0.0])] {
            b.push(name.len() as u8);
            b.extend_from_slice(name.as_bytes());
            b.push(parent);
            head.iter()
                .for_each(|v| b.extend_from_slice(&v.to_le_bytes()));
        }
        b.extend_from_slice(&30.0f32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        for _ in 0..2 {
            for v in [
                1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
            ] {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        b.push(1);
        b.extend_from_slice(&0.75f64.to_le_bytes());
        b.extend_from_slice(&6.0f32.to_le_bytes());
        b.extend_from_slice(&2u32.to_le_bytes());
        b.extend_from_slice(&[0, 0, 0, 0, 3, 2, 0xff, 1]);
        let r = AnimalRig::parse(&b).unwrap();
        assert_eq!(r.bones[1].parent, Some(0));
        assert_eq!(r.bone_of(0, [3, 2, -1]), 1);
        assert_eq!(r.bone_of(0, [9, 9, 9]), 0, "an unnamed cell is the root's");
        assert_eq!(r.select(0.7), Some(0));
        b.push(0);
        assert!(AnimalRig::parse(&b).is_err(), "trailing bytes are refused");
    }

    #[test]
    fn the_weight_eases_in_while_cropping_and_out_after() {
        let half = FEED_EASE_S / 2.0;
        let w = advance_weight(0.0, true, half);
        assert!((w - 0.5).abs() < 1e-6);
        assert_eq!(advance_weight(w, true, FEED_EASE_S), 1.0);
        assert_eq!(advance_weight(w, false, FEED_EASE_S), 0.0);
        assert!(ease(0.5) == 0.5 && ease(0.0) == 0.0 && ease(1.0) == 1.0);
        let loop_s = 1.2;
        let (a, b) = (phase_offset_s(1, loop_s), phase_offset_s(2, loop_s));
        assert!((0.0..loop_s).contains(&a) && a != b);
    }
}
