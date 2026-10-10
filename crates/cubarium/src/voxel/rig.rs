//! **Rigs** (FEED-RIG, `design/handoffs/feeding-motion-rig-2026-10-09.md`; FEED-ROLLOUT,
//! `design/handoffs/feed-rollout-2026-10-10.md`): a founder's bones and its sampled clips,
//! one per animal state, baked beside its model, and the per-frame bone transforms the model
//! pass draws the animal with (`[figures] rig`).
//!
//! # What a rig is
//!
//! The Blender builder (`scripts/blender/animal_models.py`: `fg_feed_*`, `ch_feed_*`,
//! `ls_*`) draws the founder with an armature: every part on one bone, and one action per
//! clip. The bake (`scripts/blender/bake_voxel_models.py`) records each cell's bone the way
//! it records its material, and samples each action: per frame, per bone, its turn about its
//! own pivot relative to its parent, and the pivot's displacement. A founder whose size bins
//! are one build scaled shares one sampling; one whose bins are each their own build (the
//! loftstrider's calf blend) carries its bones' pivots and clips per bin.
//!
//! # Clips per state
//!
//! Each clip names the animal state it plays in (`cropping`, `asleep`) and how it plays:
//!
//! - a **loop** ([`ClipMode::Loop`]) runs on the frame clock at the animal's own phase
//!   ([`phase_offset_s`]); its weight eases each bone's turn from none by a slerp and its
//!   displacement by the weight, so the weight's ease is the reach and the return to rest;
//! - a **track** ([`ClipMode::Track`]) is walked by its weight, its first frame (the rest
//!   pose) at 0 and its last at 1: a transition whose every step was solved (feet planted)
//!   rather than blended.
//!
//! Each clip's weight eases 0 → 1 while its state holds and back after (the feeding clip's
//! over [`FEED_EASE_S`], `Animals::feed_weight`; the rest's with the sleep blend,
//! `Animals::sleep_weight`), drawn through a smoothstep ([`ease`]). Clips partly on at once
//! layer: each bone's turns compose in the rig's clip order and its displacements add
//! ([`AnimalRig::pose`]), so a clip easing out under another ends where the other alone is.
//!
//! # The file format
//!
//! `assets/voxel-models/<voxel_mm>/<name>.rig`, named under the manifest's `rigs`; rebaked
//! with the model, never migrated. Little-endian:
//!
//! ```text
//! b"CRG2"
//! n_bones u8, then per bone: len u8, name bytes, parent u8 (255: none; before the bone)
//! n_clips u8, then per clip: len u8, state name bytes, mode u8 (0 loop, 1 track),
//!     fps f32, n_frames u16
//! n_bins u8, then per bin: length_m f64, scale f32 (voxels per metre of its pivots),
//!     per bone its pivot f32 x 3 (the model's axes — x forward, y up, z side — from the
//!     anchor), per clip per frame per bone its turn (a unit quaternion w, x, y, z, f32 x 4,
//!     about its pivot relative to its parent) and its pivot's displacement f32 x 3,
//!     n_cells u32, then per cell: dx i8, dy i8, dz i8, bone u8 (the `.cvm` bin's cells)
//! ```

use anyhow::{Context, Result, bail};
use rustc_hash::FxHashMap;

/// Seconds the feeding clip's weight takes to ease in (the reach) and out (the return to
/// rest). The builder's `FG_FEED_EASE_S`, which the flipbook harness mirrors, is the same.
pub const FEED_EASE_S: f64 = 0.4;

/// One bone: its name and its parent.
#[derive(Clone, Debug, PartialEq)]
pub struct RigBone {
    pub name: String,
    pub parent: Option<usize>,
}

/// How a clip plays (the module header).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipMode {
    /// On the frame clock, blended from rest by its weight.
    Loop,
    /// Walked by its weight, first frame to last.
    Track,
}

/// One clip: the animal state it plays in, how, its frame rate and frames.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub state: String,
    pub mode: ClipMode,
    pub fps: f64,
    pub frames: usize,
}

/// One bone at one sampled frame: its turn (a unit quaternion `[w, x, y, z]`) and its
/// pivot's displacement (the bin's metres).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Key {
    q: [f64; 4],
    d: [f64; 3],
}

/// One size bin: its body length, its scale (voxels per metre of its pivots), each bone's
/// pivot, each clip's keys (frame-major), each cell's bone.
#[derive(Clone, Debug, PartialEq)]
pub struct RigBin {
    pub length_m: f64,
    pub scale: f64,
    heads: Vec<[f64; 3]>,
    keys: Vec<Vec<Key>>,
    bones: FxHashMap<[i32; 3], u8>,
}

/// A founder's rig: its bones, its clips, its bins.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimalRig {
    pub bones: Vec<RigBone>,
    pub clips: Vec<Clip>,
    pub bins: Vec<RigBin>,
}

/// One clip's part in a pose: which clip, how far into it (a loop's seconds; a track
/// ignores it), and its eased weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipAt {
    pub clip: usize,
    pub time_s: f64,
    pub weight: f64,
}

/// One bone's transform in a bin's voxels: rows `[m0, m1, m2, t]`, a rest point `p` of the
/// model's frame going to `M p + t` (`cubarium_gpu::figures::FigureFrame::bones`).
pub type BoneRows = [[f32; 4]; 3];

const IDENTITY: [f64; 4] = [1.0, 0.0, 0.0, 0.0];

impl AnimalRig {
    /// The clip that plays in state `state` (`cropping`, `asleep`), if the rig has one.
    pub fn clip(&self, state: &str) -> Option<usize> {
        self.clips.iter().position(|c| c.state == state)
    }

    /// Seconds clip `clip` lasts.
    pub fn loop_s(&self, clip: usize) -> f64 {
        self.clips.get(clip).map_or(0.0, |c| c.frames as f64 / c.fps)
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

    /// Clip `at.clip`'s turn and displacement of bone `k` in bin `b`, weighted.
    fn clip_key(&self, b: &RigBin, at: &ClipAt, k: usize) -> Option<Key> {
        let clip = self.clips.get(at.clip)?;
        let keys = b.keys.get(at.clip)?;
        let n = self.bones.len();
        if clip.frames == 0 {
            return None;
        }
        let w = at.weight.clamp(0.0, 1.0);
        let (i0, i1, u, blend) = match clip.mode {
            ClipMode::Loop => {
                let t = (at.time_s * clip.fps).rem_euclid(clip.frames as f64);
                let i0 = (t.floor() as usize).min(clip.frames - 1);
                (i0, (i0 + 1) % clip.frames, t - i0 as f64, w)
            }
            ClipMode::Track => {
                let t = w * (clip.frames - 1) as f64;
                let i0 = (t.floor() as usize).min(clip.frames - 1);
                (i0, (i0 + 1).min(clip.frames - 1), t - i0 as f64, 1.0)
            }
        };
        let (a, c) = (keys[i0 * n + k], keys[i1 * n + k]);
        Some(Key {
            q: slerp(IDENTITY, slerp(a.q, c.q, u), blend),
            d: [0, 1, 2].map(|i| (a.d[i] + (c.d[i] - a.d[i]) * u) * blend),
        })
    }

    /// Every bone's transform in bin `bin`'s voxels with the clips `clips` on, into `out`
    /// (cleared): each clip's turn of a bone (a loop's eased from none by its weight, a
    /// track's at the frame its weight reaches) composed in the given order and their
    /// displacements added, about the bone's own pivot, then composed down the chain (a
    /// parent before its children, as the bake writes them). With one loop clip it is the
    /// builder's `fg_feed_pose`.
    pub fn pose(&self, bin: usize, clips: &[ClipAt], out: &mut Vec<BoneRows>) {
        out.clear();
        let n = self.bones.len();
        let Some(b) = self.bins.get(bin) else { return };
        if n == 0 {
            return;
        }
        let mut chain: Vec<([[f64; 3]; 3], [f64; 3])> = Vec::with_capacity(n);
        for (k, bone) in self.bones.iter().enumerate() {
            let mut q = IDENTITY;
            let mut d = [0.0; 3];
            for at in clips {
                if let Some(key) = self.clip_key(b, at, k) {
                    q = qmul(q, key.q);
                    d = [0, 1, 2].map(|i| d[i] + key.d[i]);
                }
            }
            let r = mat_of(q);
            let h = b.heads[k].map(|v| v * b.scale);
            let d = d.map(|v| v * b.scale);
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
        if r.take(4)? != b"CRG2" {
            bail!("not a CRG2 rig file (rebake it)");
        }
        let n = usize::from(r.u8()?);
        if n == 0 || n > usize::from(cubarium_gpu::figures::BONE_LIMIT) {
            bail!("{n} bones");
        }
        let mut bones = Vec::with_capacity(n);
        for k in 0..n {
            let name = r.name().context("a bone name")?;
            let parent = match r.u8()? {
                255 => None,
                p if usize::from(p) < k => Some(usize::from(p)),
                p => bail!("bone {name}'s parent {p} is not before it"),
            };
            bones.push(RigBone { name, parent });
        }
        let n_clips = usize::from(r.u8()?);
        let mut clips = Vec::with_capacity(n_clips);
        for _ in 0..n_clips {
            let state = r.name().context("a clip's state")?;
            let mode = match r.u8()? {
                0 => ClipMode::Loop,
                1 => ClipMode::Track,
                m => bail!("clip {state}'s mode {m}"),
            };
            let fps = r.f32()?;
            if !(fps > 0.0) {
                bail!("clip {state}'s frame rate {fps}");
            }
            let frames = usize::from(r.u16()?);
            clips.push(Clip { state, mode, fps, frames });
        }
        let n_bins = usize::from(r.u8()?);
        let mut bins = Vec::with_capacity(n_bins);
        for _ in 0..n_bins {
            let length_m = r.f64()?;
            let scale = r.f32()?;
            let heads = (0..n)
                .map(|_| Ok([r.f32()?, r.f32()?, r.f32()?]))
                .collect::<Result<Vec<_>>>()?;
            let mut keys = Vec::with_capacity(clips.len());
            for clip in &clips {
                let mut k = Vec::with_capacity(clip.frames * n);
                for _ in 0..clip.frames * n {
                    let q = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
                    let l = q.iter().map(|v| v * v).sum::<f64>().sqrt();
                    if !(l > 0.5) {
                        bail!("a turn of length {l}");
                    }
                    k.push(Key {
                        q: q.map(|v| v / l),
                        d: [r.f32()?, r.f32()?, r.f32()?],
                    });
                }
                keys.push(k);
            }
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
                heads,
                keys,
                bones: map,
            });
        }
        if r.at != bytes.len() {
            bail!("{} trailing bytes", bytes.len() - r.at);
        }
        Ok(AnimalRig { bones, clips, bins })
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

/// Where the feeding clip with weight `w` (linear, before [`ease`]) is after `dt` seconds of
/// the body cropping (`true`, toward 1) or not (toward 0).
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

/// The turn `b`, then `a`: `a ⊗ b`.
fn qmul(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    let [aw, ax, ay, az] = a;
    let [bw, bx, by, bz] = b;
    [
        aw * bw - ax * bx - ay * by - az * bz,
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
    ]
}

/// A rotation matrix's unit quaternion `[w, x, y, z]`.
#[cfg(test)]
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
    fn name(&mut self) -> Result<String> {
        let len = usize::from(self.u8()?);
        Ok(std::str::from_utf8(self.take(len)?)?.to_string())
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

    const QUARTER_Y: [[f64; 3]; 3] = [[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]];

    fn key(q: [f64; 4], d: [f64; 3]) -> Key {
        Key { q, d }
    }

    fn rest() -> Key {
        key(IDENTITY, [0.0; 3])
    }

    /// A two-bone rig — a root, and an arm pivoting at (1, 0, 0) — in one bin of `scale`
    /// voxels a metre, with one clip per `(state, mode, frames as [root, arm] keys)`.
    fn rig(scale: f64, clips: Vec<(&str, ClipMode, Vec<[Key; 2]>)>) -> AnimalRig {
        AnimalRig {
            bones: vec![
                RigBone { name: "root".into(), parent: None },
                RigBone { name: "arm".into(), parent: Some(0) },
            ],
            clips: clips
                .iter()
                .map(|(state, mode, frames)| Clip {
                    state: state.to_string(),
                    mode: *mode,
                    fps: 1.0,
                    frames: frames.len(),
                })
                .collect(),
            bins: vec![RigBin {
                length_m: 1.0,
                scale,
                heads: vec![[0.0; 3], [1.0, 0.0, 0.0]],
                keys: clips.into_iter().map(|(_, _, f)| f.into_iter().flatten().collect()).collect(),
                bones: FxHashMap::default(),
            }],
        }
    }

    /// The arm's one-frame quarter turn about +y, as a loop that plays while cropping.
    fn quarter_loop() -> (&'static str, ClipMode, Vec<[Key; 2]>) {
        ("cropping", ClipMode::Loop, vec![[rest(), key(quat_of(QUARTER_Y), [0.0; 3])]])
    }

    /// A track that lowers the root by `drop` metres over `n` frames, evenly.
    fn lowering(drop: f64, n: usize) -> (&'static str, ClipMode, Vec<[Key; 2]>) {
        let frames = (0..n)
            .map(|f| [key(IDENTITY, [0.0, -drop * f as f64 / (n - 1) as f64, 0.0]), rest()])
            .collect();
        ("asleep", ClipMode::Track, frames)
    }

    fn on(clip: usize, time_s: f64, weight: f64) -> ClipAt {
        ClipAt { clip, time_s, weight }
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
        let r = rig(2.0, vec![quarter_loop()]);
        let mut out = Vec::new();
        r.pose(0, &[on(0, 0.0, 1.0)], &mut out);
        // the pivot, at 1 m = 2 voxels, does not move; a point a voxel beyond it turns a
        // quarter about +y: +x goes to −z
        assert!(near(apply(&out[1], [2.0, 0.0, 0.0]), [2.0, 0.0, 0.0]));
        assert!(near(apply(&out[1], [3.0, 0.0, 0.0]), [2.0, 0.0, -1.0]));
        assert!(near(apply(&out[0], [3.0, 1.0, 0.0]), [3.0, 1.0, 0.0]), "the root stays");
    }

    #[test]
    fn a_loops_weight_zero_is_the_rest_pose_and_a_half_weight_turns_half_way() {
        let r = rig(1.0, vec![quarter_loop()]);
        let mut out = Vec::new();
        r.pose(0, &[on(0, 0.0, 0.0)], &mut out);
        assert!(near(apply(&out[1], [2.0, 0.0, 0.0]), [2.0, 0.0, 0.0]));
        r.pose(0, &[on(0, 0.0, 0.5)], &mut out);
        let h = std::f64::consts::FRAC_1_SQRT_2;
        assert!(near(apply(&out[1], [2.0, 0.0, 0.0]), [1.0 + h, 0.0, -h]));
    }

    #[test]
    fn a_child_rides_on_its_parents_turn() {
        let mut r = rig(1.0, vec![quarter_loop()]);
        r.bones.push(RigBone { name: "hand".into(), parent: Some(1) });
        r.bins[0].heads.push([2.0, 0.0, 0.0]);
        let k = r.bins[0].keys[0].clone();
        r.bins[0].keys[0] = vec![k[0], k[1], rest()];
        let mut out = Vec::new();
        r.pose(0, &[on(0, 0.0, 1.0)], &mut out);
        // the hand's pivot (2, 0, 0) is carried by the arm's quarter turn to (1, 0, −1)
        assert!(near(apply(&out[2], [2.0, 0.0, 0.0]), [1.0, 0.0, -1.0]));
    }

    #[test]
    fn a_loop_moves_between_its_frames_and_comes_round() {
        let quarter = key(quat_of(QUARTER_Y), [0.0; 3]);
        let r = rig(1.0, vec![("cropping", ClipMode::Loop, vec![[rest(), rest()], [rest(), quarter]])]);
        let mut out = Vec::new();
        let tip = |r: &AnimalRig, t: f64, out: &mut Vec<BoneRows>| {
            r.pose(0, &[on(0, t, 1.0)], out);
            apply(&out[1], [2.0, 0.0, 0.0])
        };
        assert!(near(tip(&r, 1.0, &mut out), [1.0, 0.0, -1.0]), "frame 1 is the quarter turn");
        assert!(near(tip(&r, 2.0, &mut out), [2.0, 0.0, 0.0]), "the loop comes back to frame 0");
        let half = tip(&r, 0.5, &mut out);
        assert!(half[2] < 0.0 && half[2] > -1.0, "half way between frames: {half:?}");
    }

    #[test]
    fn a_track_is_walked_by_its_weight_from_rest_to_its_last_frame_without_a_jump() {
        let r = rig(1.0, vec![lowering(1.0, 5)]);
        let mut out = Vec::new();
        let at = |w: f64, out: &mut Vec<BoneRows>| {
            r.pose(0, &[on(0, 123.0, w)], out);
            apply(&out[0], [0.0, 2.0, 0.0])
        };
        assert!(near(at(0.0, &mut out), [0.0, 2.0, 0.0]), "weight 0: its first frame, the rest pose");
        assert!(near(at(1.0, &mut out), [0.0, 1.0, 0.0]), "weight 1: its last frame");
        assert!(near(at(0.5, &mut out), [0.0, 1.5, 0.0]), "between: the frames' mix");
        // eased in and out by the weight's smoothstep, step by small step: no jump, and
        // it never leaves the span from rest to the last frame
        let mut last = at(0.0, &mut out);
        for i in 1..=200 {
            let p = at(ease(f64::from(i) / 200.0), &mut out);
            assert!((p[1] - last[1]).abs() < 0.02 && (1.0..=2.0).contains(&p[1]), "step {i}: {p:?}");
            last = p;
        }
    }

    #[test]
    fn two_clips_layer_and_one_easing_out_ends_at_the_other_alone() {
        let r = rig(1.0, vec![quarter_loop(), lowering(1.0, 3)]);
        let mut out = Vec::new();
        let tip = |clips: &[ClipAt], out: &mut Vec<BoneRows>| {
            r.pose(0, clips, out);
            apply(&out[1], [2.0, 0.0, 0.0])
        };
        // both full: the arm's quarter turn, the whole lowered a metre
        assert!(near(tip(&[on(0, 0.0, 1.0), on(1, 0.0, 1.0)], &mut out), [1.0, -1.0, -1.0]));
        // the loop eased out: the track alone; the track at its start: the loop alone
        assert_eq!(
            tip(&[on(0, 0.0, 0.0), on(1, 0.0, 0.7)], &mut out),
            tip(&[on(1, 0.0, 0.7)], &mut out)
        );
        assert!(near(tip(&[on(0, 0.0, 1.0), on(1, 0.0, 0.0)], &mut out), [1.0, 0.0, -1.0]));
        // part way on each: between the two, continuous in each weight
        let a = tip(&[on(0, 0.0, 0.5), on(1, 0.0, 0.5)], &mut out);
        let b = tip(&[on(0, 0.0, 0.51), on(1, 0.0, 0.5)], &mut out);
        assert!((0..3).all(|i| (a[i] - b[i]).abs() < 0.05), "{a:?} {b:?}");
        assert!(a[1] < 0.0 && a[2] < 0.0 && a[2] > -1.0, "{a:?}");
    }

    #[test]
    fn a_rig_names_its_clip_per_state() {
        let r = rig(1.0, vec![quarter_loop(), lowering(1.0, 3)]);
        assert_eq!(r.clip("cropping"), Some(0));
        assert_eq!(r.clip("asleep"), Some(1));
        assert_eq!(r.clip("hunkered"), None);
        assert_eq!(r.loop_s(0), 1.0);
    }

    #[test]
    fn a_rig_file_round_trips_its_bones_clips_and_cells() {
        let mut b = b"CRG2".to_vec();
        b.push(2);
        for (name, parent) in [("root", 255u8), ("arm", 0)] {
            b.push(name.len() as u8);
            b.extend_from_slice(name.as_bytes());
            b.push(parent);
        }
        b.push(2);
        for (state, mode, frames) in [("cropping", 0u8, 1u16), ("asleep", 1, 2)] {
            b.push(state.len() as u8);
            b.extend_from_slice(state.as_bytes());
            b.push(mode);
            b.extend_from_slice(&30.0f32.to_le_bytes());
            b.extend_from_slice(&frames.to_le_bytes());
        }
        b.push(1);
        b.extend_from_slice(&0.75f64.to_le_bytes());
        b.extend_from_slice(&8.0f32.to_le_bytes());
        for v in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        // three frames (one loop, two track) of two bones: no turn, no displacement
        for _ in 0..3 * 2 {
            for v in [1.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0] {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        b.extend_from_slice(&2u32.to_le_bytes());
        b.extend_from_slice(&[0, 0, 0, 0, 3, 2, 0xff, 1]);
        let r = AnimalRig::parse(&b).unwrap();
        assert_eq!(r.bones[1].parent, Some(0));
        assert_eq!((r.clips[1].mode, r.clips[1].frames), (ClipMode::Track, 2));
        assert_eq!(r.bins[0].heads[1], [1.0, 0.0, 0.0]);
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
