//! **Rigs** (FEED-RIG, `design/handoffs/feeding-motion-rig-2026-10-09.md`; FEED-ROLLOUT,
//! `design/handoffs/feed-rollout-2026-10-10.md`): a founder's bones and its sampled clips,
//! one per animal state, baked beside its model, and the per-frame bone transforms the model
//! pass draws the animal with (`[figures] rig`). A founder's pose model (the seedporter's
//! `seedporter-stand`) carries its own rig, named under the manifest's `rigs` by the model's
//! name (`ModelLibrary::rig_in_mode`).
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
//! # Aiming (FEED-ROLLOUT round 2)
//!
//! A clip can be **aimed** at the food: the sim keeps the cell of each body's last bite
//! (`cubarium_voxel_fauna::Animal::bite`, presentation memory, never saved), the presenter
//! turns it into the bite's height above the feet and its bearing from the heading, eased
//! ([`AimTrack`], carried on the figure's `Spec`), and the rig reads the eased [`Aim`]:
//!
//! - **height**: a state may have several clips, each baked reaching for one height (its
//!   aim, per bin, in the bin's metres); the rig blends the two that bracket the aim's
//!   height, bone by bone (outside them, the nearest);
//! - **bearing**: a rig with a turn **symmetry** about up (the loftstrider's six-fold:
//!   [`Symmetry`]) bakes its aimed clips toward the heading only; the rig plays them turned
//!   by whole steps toward the bearing, each bone's keys handed to its image under the
//!   step and turned with it (so the frond pair nearest the bite does the work, and the
//!   lean and the neck turn toward it), blending two neighbouring steps only within
//!   [`SECTOR_BLEND`] of the boundary between them;
//! - **lowering**: an aimed clip may hold the body part way down its rig's `asleep` track
//!   (its **lower**, a share of the track, blended by the aim's height like its keys and
//!   eased by the clip's weight): the track's solved frames crouch the body with the feet
//!   planted, and the clip's own keys, baked relative to the track's frame at that depth,
//!   add its lean and reach;
//! - **no bite** ([`Aim::weight`] 0): the state's first clip at the heading, as the rig
//!   played before aiming existed. The weight eases the aim in from there and back.
//!
//! # The file format
//!
//! `assets/voxel-models/<voxel_mm>/<name>.rig`, named under the manifest's `rigs`; rebaked
//! with the model, never migrated (a `CRG2` file, the same without the symmetry and the
//! aims, still reads: nothing in it is aimed). Little-endian:
//!
//! ```text
//! b"CRG3"
//! n_bones u8, then per bone: len u8, name bytes, parent u8 (255: none; before the bone)
//! order u8 (the turn symmetry about up; 0 or 1: none), and when it is 2 or more, per
//!     bone its image u8 (the bone one step of 2π / order about +y carries it onto)
//! n_clips u8, then per clip: len u8, state name bytes, mode u8 (0 loop, 1 track),
//!     fps f32, n_frames u16
//! n_bins u8, then per bin: length_m f64, scale f32 (voxels per metre of its pivots),
//!     per clip its aim f32 (the bite height above the feet it reaches for, the bin's
//!     metres; NaN: not aimed) and its lower f32 (the share of the `asleep` track it holds
//!     the body at; 0: none),
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

/// Seconds an aim takes to move to a new bite (its height and bearing, eased), and to ease
/// in from the unaimed clip and back out ([`AimTrack`]).
pub const AIM_EASE_S: f64 = 0.6;

/// The share of a symmetry step, about the boundary between two steps, over which a
/// bearing blends the two (outside it the nearer step plays alone, so a bite off to one
/// side is one frond pair's, not a mix of two).
pub const SECTOR_BLEND: f64 = 1.0 / 3.0;

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

/// One size bin: its body length, its scale (voxels per metre of its pivots), each clip's
/// aim (the bite height it reaches for, metres above the feet; `None`: not aimed) and
/// lower, each bone's pivot, each clip's keys (frame-major), each cell's bone.
#[derive(Clone, Debug, PartialEq)]
pub struct RigBin {
    pub length_m: f64,
    pub scale: f64,
    pub aims: Vec<Option<f64>>,
    /// Per clip, the share of the rig's `asleep` track it holds the body at (the module
    /// header's lowering; 0: none).
    pub lowers: Vec<f64>,
    heads: Vec<[f64; 3]>,
    keys: Vec<Vec<Key>>,
    bones: FxHashMap<[i32; 3], u8>,
}

/// A rig's turn symmetry about up: `order` steps of `2π / order` about +y map the body
/// onto itself, one step carrying bone `k` onto bone `image[k]` (the loftstrider's legs
/// and fronds one round, its body, neck and head onto themselves).
#[derive(Clone, Debug, PartialEq)]
pub struct Symmetry {
    pub order: usize,
    pub image: Vec<usize>,
    /// `image` undone: the bone one step carries onto `k`.
    preimage: Vec<usize>,
}

impl Symmetry {
    /// The symmetry of `order` with one step's `image` of each bone, if `image` is a
    /// permutation whose `order`-th power is the identity.
    pub fn new(order: usize, image: Vec<usize>) -> Option<Symmetry> {
        let n = image.len();
        let mut preimage = vec![usize::MAX; n];
        for (k, &j) in image.iter().enumerate() {
            if j >= n || preimage[j] != usize::MAX {
                return None;
            }
            preimage[j] = k;
        }
        let round = (0..n).all(|k| (0..order).fold(k, |j, _| image[j]) == k);
        (order >= 2 && round).then_some(Symmetry { order, image, preimage })
    }

    /// The angle of one step, radians.
    pub fn step(&self) -> f64 {
        std::f64::consts::TAU / self.order as f64
    }

    /// The bone `m` steps carry onto `k` (`m` any integer).
    fn source(&self, k: usize, m: i64) -> usize {
        let m = m.rem_euclid(self.order as i64);
        (0..m).fold(k, |j, _| self.preimage[j])
    }
}

/// A founder's rig: its bones, its clips, its bins, its turn symmetry (if any).
#[derive(Clone, Debug, PartialEq)]
pub struct AnimalRig {
    pub bones: Vec<RigBone>,
    pub clips: Vec<Clip>,
    pub bins: Vec<RigBin>,
    pub symmetry: Option<Symmetry>,
}

/// Where an animal's aimed clips reach (the module header): the bite's height above the
/// feet (metres) and its bearing from the heading (radians, the turn about up that
/// carries the heading onto it), and how far the aim is on (0: the unaimed clip, 1: the
/// bite's own), after its ease.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aim {
    pub weight: f64,
    pub height_m: f64,
    pub bearing_rad: f64,
}

/// One animal's aim at a tick, as the presenter keeps it (`Animals::aim`): the aim's
/// weight (before its ease) and change per tick, and the height and bearing it moves from
/// (`from`, at tick `start`) to (`to`) over [`AIM_EASE_S`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AimTrack {
    pub tick: u64,
    pub weight: f32,
    pub rate: f32,
    pub from: [f32; 2],
    pub to: [f32; 2],
    pub start: u64,
}

impl AimTrack {
    /// The aim at drawn time `td` (ticks): the weight run on from the staged tick, the
    /// height and bearing eased from `from` to `to` since `start`.
    pub fn at(&self, td: f64, tick_hz: f64) -> Aim {
        let ease_ticks = AIM_EASE_S * tick_hz;
        let u = if ease_ticks > 0.0 { ease((td - self.start as f64) / ease_ticks) } else { 1.0 };
        let lerp = |i: usize| f64::from(self.from[i]) + (f64::from(self.to[i]) - f64::from(self.from[i])) * u;
        Aim {
            weight: ease(f64::from(self.weight) + f64::from(self.rate) * (td - self.tick as f64)),
            height_m: lerp(0),
            bearing_rad: lerp(1),
        }
    }
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

    /// The aim height of the state's first aimed clip in bin `bin` (the unaimed reach), if
    /// clip `clip`'s state has aimed clips there.
    pub fn default_aim(&self, bin: usize, clip: usize) -> Option<f64> {
        self.default_aim_in(self.bins.get(bin)?, clip)
    }

    /// Clip `clip`'s sampled turn and displacement of bone `k` in bin `b`, unweighted: a
    /// loop's at `at`'s time, a track's at the frame `at`'s weight reaches.
    fn frame_key(&self, b: &RigBin, clip: usize, at: &ClipAt, k: usize) -> Option<Key> {
        let c = self.clips.get(clip)?;
        let keys = b.keys.get(clip)?;
        let n = self.bones.len();
        if c.frames == 0 {
            return None;
        }
        let (i0, i1, u) = match c.mode {
            ClipMode::Loop => {
                let t = (at.time_s * c.fps).rem_euclid(c.frames as f64);
                let i0 = (t.floor() as usize).min(c.frames - 1);
                (i0, (i0 + 1) % c.frames, t - i0 as f64)
            }
            ClipMode::Track => {
                let t = at.weight.clamp(0.0, 1.0) * (c.frames - 1) as f64;
                let i0 = (t.floor() as usize).min(c.frames - 1);
                (i0, (i0 + 1).min(c.frames - 1), t - i0 as f64)
            }
        };
        Some(mix(*keys.get(i0 * n + k)?, *keys.get(i1 * n + k)?, u))
    }

    /// The aimed clips of clip `clip`'s state in bin `b` that bracket `height_m` (the
    /// nearest outside them), and how far between them it is; `None` for an unaimed state.
    fn bracket(&self, b: &RigBin, clip: usize, height_m: f64) -> Option<(usize, usize, f64)> {
        let state = &self.clips.get(clip)?.state;
        let (mut lo, mut hi): (Option<(f64, usize)>, Option<(f64, usize)>) = (None, None);
        for (c, k) in self.clips.iter().enumerate() {
            let Some(h) = b.aims.get(c).copied().flatten() else { continue };
            if &k.state != state {
                continue;
            }
            if h <= height_m && lo.is_none_or(|(l, _)| h > l) {
                lo = Some((h, c));
            }
            if h >= height_m && hi.is_none_or(|(u, _)| h < u) {
                hi = Some((h, c));
            }
        }
        let ((h0, c0), (h1, c1)) = match (lo, hi) {
            (Some(l), Some(u)) => (l, u),
            (Some(l), None) => (l, l),
            (None, Some(u)) => (u, u),
            (None, None) => return None,
        };
        Some((c0, c1, if h1 > h0 { (height_m - h0) / (h1 - h0) } else { 0.0 }))
    }

    /// The height and bearing an aimed clip reaches for: the aim eased in from the unaimed
    /// reach (the state's first clip, at the heading) by its weight.
    fn resolve(&self, b: &RigBin, clip: usize, aim: Option<Aim>) -> (f64, f64) {
        let h0 = self.default_aim_in(b, clip).unwrap_or(0.0);
        match aim {
            Some(a) => {
                let aw = a.weight.clamp(0.0, 1.0);
                (h0 + (a.height_m - h0) * aw, a.bearing_rad * aw)
            }
            None => (h0, 0.0),
        }
    }

    /// The aimed key of bone `k` (the module header): the state's clips that bracket
    /// `height_m` blended, from the bone the symmetry's steps toward `bearing_rad` carry
    /// onto `k`, turned with them; two neighbouring steps blended near their boundary.
    fn aimed_key(&self, b: &RigBin, at: &ClipAt, k: usize, height_m: f64, bearing_rad: f64) -> Option<Key> {
        let Some((c0, c1, f)) = self.bracket(b, at.clip, height_m) else {
            return self.frame_key(b, at.clip, at, k);
        };
        let at_step = |m: i64| -> Option<Key> {
            let (src, angle) = match &self.symmetry {
                Some(s) => (s.source(k, m), m as f64 * s.step()),
                None => (k, 0.0),
            };
            let a = self.frame_key(b, c0, at, src)?;
            let key = if c1 == c0 { a } else { mix(a, self.frame_key(b, c1, at, src)?, f) };
            Some(turned(key, angle))
        };
        match &self.symmetry {
            None => at_step(0),
            Some(s) => {
                let p = bearing_rad / s.step();
                let m0 = p.floor();
                let g = ease((p - m0 - 0.5) / SECTOR_BLEND + 0.5);
                let m0 = m0 as i64;
                if g <= 0.0 {
                    at_step(m0)
                } else if g >= 1.0 {
                    at_step(m0 + 1)
                } else {
                    Some(mix(at_step(m0)?, at_step(m0 + 1)?, g))
                }
            }
        }
    }

    /// How far down its `asleep` track an aimed clip at `at` holds the body (the module
    /// header's lowering): its state's lowers bracketing the aim's height, blended, times
    /// the clip's weight.
    fn lowering(&self, b: &RigBin, at: &ClipAt, aim: Option<Aim>) -> f64 {
        if b.aims.get(at.clip).copied().flatten().is_none() {
            return 0.0;
        }
        let (h, _) = self.resolve(b, at.clip, aim);
        let Some((c0, c1, f)) = self.bracket(b, at.clip, h) else { return 0.0 };
        let l = |c: usize| b.lowers.get(c).copied().unwrap_or(0.0);
        (l(c0) + (l(c1) - l(c0)) * f) * at.weight.clamp(0.0, 1.0)
    }

    /// Clip `at.clip`'s turn and displacement of bone `k` in bin `b`, weighted, aimed by
    /// `aim` when the clip is aimed (none: the unaimed reach).
    fn clip_key(&self, b: &RigBin, at: &ClipAt, k: usize, aim: Option<Aim>) -> Option<Key> {
        let clip = self.clips.get(at.clip)?;
        let w = at.weight.clamp(0.0, 1.0);
        let raw = match b.aims.get(at.clip).copied().flatten() {
            Some(_) => {
                let (h, bearing) = self.resolve(b, at.clip, aim);
                self.aimed_key(b, at, k, h, bearing)?
            }
            None => self.frame_key(b, at.clip, at, k)?,
        };
        let blend = match clip.mode {
            ClipMode::Loop => w,
            ClipMode::Track => 1.0,
        };
        Some(Key {
            q: slerp(IDENTITY, raw.q, blend),
            d: raw.d.map(|v| v * blend),
        })
    }

    /// [`Self::default_aim`] in bin `b`.
    fn default_aim_in(&self, b: &RigBin, clip: usize) -> Option<f64> {
        let state = &self.clips.get(clip)?.state;
        self.clips
            .iter()
            .enumerate()
            .find(|(c, k)| &k.state == state && b.aims.get(*c).copied().flatten().is_some())
            .and_then(|(c, _)| b.aims[c])
    }

    /// [`Self::pose_aimed`] with no bite: every aimed clip at its unaimed reach.
    pub fn pose(&self, bin: usize, clips: &[ClipAt], out: &mut Vec<BoneRows>) {
        self.pose_aimed(bin, clips, None, out);
    }

    /// Every bone's transform in bin `bin`'s voxels with the clips `clips` on, into `out`
    /// (cleared): each clip's turn of a bone (a loop's eased from none by its weight, a
    /// track's at the frame its weight reaches; an aimed clip's toward `aim`) composed in
    /// the given order and their displacements added, about the bone's own pivot, then
    /// composed down the chain (a parent before its children, as the bake writes them).
    /// With one loop clip it is the builder's `fg_feed_pose`.
    pub fn pose_aimed(&self, bin: usize, clips: &[ClipAt], aim: Option<Aim>, out: &mut Vec<BoneRows>) {
        out.clear();
        let n = self.bones.len();
        let Some(b) = self.bins.get(bin) else { return };
        if n == 0 {
            return;
        }
        // An aimed clip's lowering walks the `asleep` track at least that far.
        let track = self.clip("asleep").filter(|&c| self.clips[c].mode == ClipMode::Track);
        let lower = clips.iter().map(|at| self.lowering(b, at, aim)).fold(0.0, f64::max);
        let mut walked: Vec<ClipAt>;
        let clips = match track {
            Some(t) if lower > 0.0 => {
                walked = clips.iter().copied().filter(|at| at.clip != t).collect();
                let asleep = clips.iter().filter(|at| at.clip == t).map(|at| at.weight).fold(0.0, f64::max);
                walked.push(ClipAt { clip: t, time_s: 0.0, weight: asleep.max(lower) });
                walked.sort_by_key(|at| at.clip);
                &walked[..]
            }
            _ => clips,
        };
        let mut chain: Vec<([[f64; 3]; 3], [f64; 3])> = Vec::with_capacity(n);
        for (k, bone) in self.bones.iter().enumerate() {
            let mut q = IDENTITY;
            let mut d = [0.0; 3];
            for at in clips {
                if let Some(key) = self.clip_key(b, at, k, aim) {
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
        let version = match r.take(4)? {
            b"CRG3" => 3,
            b"CRG2" => 2,
            _ => bail!("not a CRG3 rig file (rebake it)"),
        };
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
        let symmetry = match if version >= 3 { usize::from(r.u8()?) } else { 0 } {
            0 | 1 => None,
            order => {
                let image = (0..n).map(|_| Ok(usize::from(r.u8()?))).collect::<Result<Vec<_>>>()?;
                Some(Symmetry::new(order, image).with_context(|| format!("a symmetry of order {order}"))?)
            }
        };
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
            let (mut aims, mut lowers) = (vec![None; clips.len()], vec![0.0; clips.len()]);
            if version >= 3 {
                for c in 0..clips.len() {
                    aims[c] = Some(r.f32()?).filter(|h| h.is_finite());
                    lowers[c] = Some(r.f32()?).filter(|l| l.is_finite()).unwrap_or(0.0).clamp(0.0, 1.0);
                }
            }
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
                aims,
                lowers,
                heads,
                keys,
                bones: map,
            });
        }
        if r.at != bytes.len() {
            bail!("{} trailing bytes", bytes.len() - r.at);
        }
        Ok(AnimalRig { bones, clips, bins, symmetry })
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

/// Key `a` `u` of the way to `c`: its turn by a slerp, its displacement linearly.
fn mix(a: Key, c: Key, u: f64) -> Key {
    Key {
        q: slerp(a.q, c.q, u),
        d: [0, 1, 2].map(|i| a.d[i] + (c.d[i] - a.d[i]) * u),
    }
}

/// Key `k` turned by `angle` about +y: its turn conjugated, its displacement turned. A
/// bone at a pivot the turn carries onto another's, posed by it, is that bone posed by `k`
/// and turned whole.
fn turned(k: Key, angle: f64) -> Key {
    if angle == 0.0 {
        return k;
    }
    let (s, c) = (0.5 * angle).sin_cos();
    let r = [c, 0.0, s, 0.0];
    let ri = [c, 0.0, -s, 0.0];
    let (sa, ca) = angle.sin_cos();
    let [x, y, z] = k.d;
    Key {
        q: qmul(qmul(r, k.q), ri),
        d: [ca * x + sa * z, y, -sa * x + ca * z],
    }
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
                aims: vec![None; clips.len()],
                lowers: vec![0.0; clips.len()],
                heads: vec![[0.0; 3], [1.0, 0.0, 0.0]],
                keys: clips.into_iter().map(|(_, _, f)| f.into_iter().flatten().collect()).collect(),
                bones: FxHashMap::default(),
            }],
            symmetry: None,
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

    // --- aiming (FEED-ROLLOUT round 2) ---------------------------------------------------

    const QUARTER_Z: [[f64; 3]; 3] = [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];

    /// A root with two arms either side of it (pivots ±1 m on x), two-fold about up, and
    /// two aimed one-frame loops while cropping: at height 1 m arm 0 is lifted a quarter
    /// about +z, at 3 m it is not.
    fn aimed_rig() -> AnimalRig {
        let lift = key(quat_of(QUARTER_Z), [0.0; 3]);
        let low = vec![rest(), lift, rest()];
        let high = vec![rest(), rest(), rest()];
        AnimalRig {
            bones: vec![
                RigBone { name: "root".into(), parent: None },
                RigBone { name: "arm0".into(), parent: Some(0) },
                RigBone { name: "arm1".into(), parent: Some(0) },
            ],
            clips: ["cropping", "cropping"]
                .iter()
                .map(|s| Clip { state: s.to_string(), mode: ClipMode::Loop, fps: 1.0, frames: 1 })
                .collect(),
            bins: vec![RigBin {
                length_m: 1.0,
                scale: 1.0,
                aims: vec![Some(1.0), Some(3.0)],
                lowers: vec![0.0, 0.0],
                heads: vec![[0.0; 3], [1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]],
                keys: vec![low, high],
                bones: FxHashMap::default(),
            }],
            symmetry: Symmetry::new(2, vec![0, 2, 1]),
        }
    }

    /// The tips of the two arms (a metre beyond each pivot) at `aim`.
    fn tips(r: &AnimalRig, aim: Option<Aim>) -> [[f64; 3]; 2] {
        let mut out = Vec::new();
        r.pose_aimed(0, &[on(0, 0.0, 1.0)], aim, &mut out);
        [apply(&out[1], [2.0, 0.0, 0.0]), apply(&out[2], [-2.0, 0.0, 0.0])]
    }

    fn aim(height_m: f64, bearing_rad: f64) -> Option<Aim> {
        Some(Aim { weight: 1.0, height_m, bearing_rad })
    }

    #[test]
    fn an_aimed_state_blends_the_clips_bracketing_the_height_and_holds_the_nearest_outside() {
        let r = aimed_rig();
        let lifted = [1.0, 1.0, 0.0];
        assert!(near(tips(&r, aim(1.0, 0.0))[0], lifted), "at the low clip's height: its lift");
        assert!(near(tips(&r, aim(3.0, 0.0))[0], [2.0, 0.0, 0.0]), "at the high clip's: none");
        let h = std::f64::consts::FRAC_1_SQRT_2;
        assert!(near(tips(&r, aim(2.0, 0.0))[0], [1.0 + h, h, 0.0]), "half way: half the turn");
        assert!(near(tips(&r, aim(-4.0, 0.0))[0], lifted), "below every clip: the lowest");
        assert!(near(tips(&r, aim(9.0, 0.0))[0], [2.0, 0.0, 0.0]), "above every clip: the highest");
    }

    #[test]
    fn a_missing_bite_plays_the_states_first_clip_at_the_heading() {
        let r = aimed_rig();
        let first = tips(&r, aim(1.0, 0.0));
        assert_eq!(tips(&r, None), first);
        assert_eq!(tips(&r, Some(Aim { weight: 0.0, height_m: 3.0, bearing_rad: 2.0 })), first);
        assert_eq!(r.default_aim(0, 0), Some(1.0));
        // the aim's weight eases between the unaimed reach and the bite's
        let half = tips(&r, Some(Aim { weight: 0.5, height_m: 3.0, bearing_rad: 0.0 }));
        assert_eq!(half, tips(&r, aim(2.0, 0.0)));
    }

    #[test]
    fn the_bearing_hands_the_reach_to_the_bone_the_symmetry_turns_toward_it() {
        let r = aimed_rig();
        let pi = std::f64::consts::PI;
        // toward the heading arm 0 reaches; turned half round, arm 1 does, turned with it
        let [a, b] = tips(&r, aim(1.0, 0.0));
        assert!(near(a, [1.0, 1.0, 0.0]) && near(b, [-2.0, 0.0, 0.0]));
        let [a, b] = tips(&r, aim(1.0, pi));
        assert!(near(a, [2.0, 0.0, 0.0]) && near(b, [-1.0, 1.0, 0.0]), "{a:?} {b:?}");
        // a whole round is the heading again, either way
        let (a, b) = (tips(&r, aim(1.0, 2.0 * pi)), tips(&r, aim(1.0, 0.0)));
        assert!(near(a[0], b[0]) && near(a[1], b[1]));
        assert!(near(tips(&r, aim(1.0, -pi))[1], [-1.0, 1.0, 0.0]));
        // off the boundary between two steps one plays alone; on it, both half
        let edge = 0.5 * (1.0 - SECTOR_BLEND) * pi;
        assert_eq!(tips(&r, aim(1.0, edge - 1e-6)), tips(&r, aim(1.0, 0.0)));
        let [a, b] = tips(&r, aim(1.0, 0.5 * pi));
        assert!(a[1] > 0.1 && a[1] < 0.9 && (a[1] - b[1]).abs() < 1e-6, "{a:?} {b:?}");
    }

    #[test]
    fn an_aim_track_eases_to_a_new_bite_without_a_jump() {
        let hz = 20.0;
        let t = AimTrack { tick: 100, weight: 1.0, rate: 0.0, from: [1.0, 0.0], to: [3.0, 1.0], start: 100 };
        let at = |td: f64| t.at(td, hz);
        assert_eq!((at(90.0).height_m, at(100.0).height_m), (1.0, 1.0), "from where it was");
        let end = 100.0 + AIM_EASE_S * hz;
        assert!((at(end).height_m - 3.0).abs() < 1e-6 && (at(end + 50.0).bearing_rad - 1.0).abs() < 1e-6);
        let mut last = at(100.0);
        for i in 1..=100 {
            let a = at(100.0 + (end - 100.0) * f64::from(i) / 100.0);
            assert!((a.height_m - last.height_m).abs() < 0.05 && a.height_m >= last.height_m, "step {i}");
            last = a;
        }
        // the weight runs on from the staged tick at its rate
        let w = AimTrack { weight: 0.5, rate: 0.1, ..t };
        assert!((w.at(100.0, hz).weight - ease(0.5)).abs() < 1e-6);
        assert!((w.at(102.0, hz).weight - ease(0.7)).abs() < 1e-6);
    }

    #[test]
    fn an_aimed_clip_lowers_the_body_along_the_asleep_track_by_its_height_and_weight() {
        // the aimed rig's two clips, the low one holding the body half down a track that
        // lowers the root a metre
        let mut r = aimed_rig();
        r.clips.push(Clip { state: "asleep".into(), mode: ClipMode::Track, fps: 1.0, frames: 3 });
        let b = &mut r.bins[0];
        b.aims.push(None);
        b.lowers = vec![0.5, 0.0, 0.0];
        b.keys.push(
            (0..3)
                .flat_map(|f| [key(IDENTITY, [0.0, -0.5 * f as f64, 0.0]), rest(), rest()])
                .collect(),
        );
        let root_y = |clips: &[ClipAt], aim: Option<Aim>| {
            let mut out = Vec::new();
            r.pose_aimed(0, clips, aim, &mut out);
            apply(&out[0], [0.0, 2.0, 0.0])[1] - 2.0
        };
        let feed = |w: f64| on(0, 0.0, w);
        assert!((root_y(&[feed(1.0)], aim(1.0, 0.0)) + 0.5).abs() < 1e-6, "the low clip: half down");
        assert!((root_y(&[feed(1.0)], aim(2.0, 0.0)) + 0.25).abs() < 1e-6, "half way up: a quarter");
        assert!(root_y(&[feed(1.0)], aim(3.0, 0.0)).abs() < 1e-6, "the high clip: standing");
        assert!((root_y(&[feed(0.5)], aim(1.0, 0.0)) + 0.25).abs() < 1e-6, "eased in by the weight");
        // asleep further down than the crouch: the sleep's depth; less: the crouch's
        let y = root_y(&[feed(1.0), on(2, 0.0, 0.8)], aim(1.0, 0.0));
        assert!((y + 0.8).abs() < 1e-6, "{y}");
        assert!((root_y(&[feed(1.0), on(2, 0.0, 0.2)], aim(1.0, 0.0)) + 0.5).abs() < 1e-6);
    }

    /// A CRG3 file of two bones (root, arm), the symmetry `sym` (its order byte and the
    /// images), two one-frame loops while cropping aimed at 2.5 m and not at all.
    fn crg3(sym: &[u8]) -> Vec<u8> {
        let mut b = b"CRG3".to_vec();
        b.push(2);
        for (name, parent) in [("root", 255u8), ("arm", 0)] {
            b.push(name.len() as u8);
            b.extend_from_slice(name.as_bytes());
            b.push(parent);
        }
        b.extend_from_slice(sym);
        b.push(2);
        for _ in 0..2 {
            b.push(8);
            b.extend_from_slice(b"cropping");
            b.push(0);
            b.extend_from_slice(&15.0f32.to_le_bytes());
            b.extend_from_slice(&1u16.to_le_bytes());
        }
        b.push(1);
        b.extend_from_slice(&1.0f64.to_le_bytes());
        b.extend_from_slice(&8.0f32.to_le_bytes());
        for (aim, lower) in [(2.5f32, 0.5f32), (f32::NAN, 0.0)] {
            b.extend_from_slice(&aim.to_le_bytes());
            b.extend_from_slice(&lower.to_le_bytes());
        }
        for v in [0.0f32; 6] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for _ in 0..2 * 2 {
            for v in [1.0f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0] {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        b.extend_from_slice(&0u32.to_le_bytes());
        b
    }

    #[test]
    fn a_crg3_file_carries_the_symmetry_and_each_clips_aim() {
        let r = AnimalRig::parse(&crg3(&[0])).unwrap();
        assert_eq!(r.symmetry, None);
        assert_eq!(r.bins[0].aims, vec![Some(2.5), None]);
        assert_eq!(r.bins[0].lowers, vec![0.5, 0.0]);
        let r = AnimalRig::parse(&crg3(&[2, 1, 0])).unwrap();
        assert_eq!(r.symmetry.map(|s| s.image), Some(vec![1, 0]));
        assert!(AnimalRig::parse(&crg3(&[2, 0, 0])).is_err(), "an image that is no permutation");
        assert!(AnimalRig::parse(&crg3(&[3, 1, 0])).is_err(), "three steps of a swap are not round");
    }

    /// The baked loftstrider (the asset), if this checkout has it.
    fn loftstrider() -> Option<AnimalRig> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/voxel-models/125/loftstrider.rig");
        AnimalRig::parse(&std::fs::read(path).ok()?).ok()
    }

    #[test]
    fn the_loftstriders_feet_stay_planted_and_flat_at_every_aim() {
        let Some(r) = loftstrider() else { return };
        let Some(sym) = r.symmetry.clone() else { panic!("the loftstrider is six-fold") };
        let feed = r.clip("cropping").unwrap();
        let rest = r.clip("asleep").unwrap();
        let feet: Vec<usize> = (0..r.bones.len()).filter(|&k| r.bones[k].name.starts_with("foot")).collect();
        assert_eq!(feet.len(), 6);
        let mut out = Vec::new();
        let mut worst = [0.0f64; 2];
        for bin in 0..r.bins.len() {
            let b = &r.bins[bin];
            let heights: Vec<f64> = b.aims.iter().flatten().copied().collect();
            let (lo, hi) = heights.iter().fold((f64::MAX, f64::MIN), |(l, h), &x| (l.min(x), h.max(x)));
            for i in 0..=8 {
                let height_m = lo - 0.3 + (hi - lo + 0.6) * f64::from(i) / 8.0;
                for j in 0..12 {
                    let bearing_rad = sym.step() * f64::from(j) * 0.5;
                    for t in [0.0, 0.4, 0.9, 1.5, 2.2] {
                        // fully on and easing in (asleep and cropping at once is a
                        // passing blend, not a stance)
                        for (w, sleep) in [(1.0, 0.0), (0.5, 0.0), (0.2, 0.0)] {
                            let clips = [on(feed, t, w), on(rest, 0.0, sleep)];
                            r.pose_aimed(bin, &clips, aim(height_m, bearing_rad), &mut out);
                            for &k in &feet {
                                let h = b.heads[k].map(|v| v * b.scale);
                                let moved = apply(&out[k], h);
                                let drift = (0..3).map(|a| (moved[a] - h[a]).powi(2)).sum::<f64>().sqrt();
                                // the ankle where it stood, within half a voxel (the
                                // aims between two clips and the ease blend solved poses)
                                worst[usize::from(w < 1.0)] = worst[usize::from(w < 1.0)].max(drift);
                                assert!(drift < 0.5, "bin {bin} h {height_m:.2} b {bearing_rad:.2} t {t} w {w} sleep {sleep}: foot {k} moved {drift:.3} voxels");
                                // and the foot level: its turn leaves up as up
                                let up = [out[k][0][1], out[k][1][1], out[k][2][1]];
                                assert!(f64::from(up[1]) > 0.995, "foot {k} tilted: {up:?}");
                            }
                        }
                    }
                }
            }
        }
        println!("the feet's worst drift, voxels: fully on {:.3}, easing {:.3}", worst[0], worst[1]);
    }
}
