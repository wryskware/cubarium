//! **GLOW-EASE** (Wrysk, 2026-10-10, `design/handoffs/glow-ease-2026-10-10.md`): "glowy
//! things should never change brightness discontinuously, and we should add some flicker
//! to them."
//!
//! Every emitting cell the lit tier draws (a ripe lantern, a latticevine flower's mouth, a
//! glowcap's lip, a bloomcrown's core and bloom) is an **instance** here, keyed by its cell
//! (the voxels' texture index). Once a tick the packer hands over what each cell should
//! glow now (a [`Target`]: its dossier colour and a level, 1 lit, 0 dark, a flower's
//! nectar fill between); the instance's drawn colour then moves there in a straight line
//! over [`Ease::fade_s`] of the presenter's clock, from whatever it showed when the target
//! moved. A switch on or off is a ramp of brightness, a colour swap passes through the
//! blends of the two colours, and a target that moves again mid-ramp starts its new ramp
//! from where the old one had got to, so nothing jumps. A cell no longer targeted (eaten,
//! unripe, closed) ramps to nothing and is forgotten once there; until then the packer
//! keeps drawing what it drew ([`GlowEase::released`], [`GlowEase::holds`]).
//!
//! On top of the ramp, per frame: a lantern's candle flicker ([`flicker`]), a glowcap lip's
//! candle (`super::candle`, unchanged), and the flower cycle candidate ([`cycle`]).
//!
//! What a frame draws for an instance is a colour (linear light) and an **on** share: the
//! shader draws the cell's emitting texels as `colour + (1 − on) × the texel's lit
//! pigment`, so a texel fading out ends on its pigment, not on black. Flicker and candle
//! dim the colour only; the ramp, the nectar and the cycle dim both.

use std::f64::consts::TAU;

use rustc_hash::FxHashMap;

use super::LightConfig;
use super::candle::{Candle, PACES, mix};

/// A clock further from the last update than this, or behind it, is a jump (a load, a
/// seek): every emitter is drawn as it stands, with no ramp.
pub const JUMP_S: f64 = 5.0;

/// A target closer than this to the one an instance is ramping to is the same target.
const SAME: f32 = 1e-4;

/// The GLOW-EASE knobs (`[light] emit_fade_s`, `lantern_flicker`, `lantern_flicker_s`,
/// `flower_nectar_glow`, `flower_cycle`, `flower_cycle_s`), and the glowcap candle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ease {
    pub fade_s: f32,
    pub flicker: f32,
    pub flicker_s: f32,
    pub nectar: bool,
    pub cycle: bool,
    pub cycle_s: f32,
    /// The glowcap candle, when it runs.
    pub candle: Option<Candle>,
}

impl Ease {
    pub fn of(light: &LightConfig) -> Ease {
        Ease {
            fade_s: light.emit_fade_s.max(0.0),
            flicker: light.lantern_flicker.clamp(0.0, 1.0),
            flicker_s: light.lantern_flicker_s,
            nectar: light.flower_nectar_glow,
            cycle: light.flower_cycle,
            cycle_s: light.flower_cycle_s,
            candle: Some(Candle::of(light)).filter(Candle::on),
        }
    }

    /// Every switch at once, no flicker, flowers full while open, no cycle, no candle: the
    /// lit tier's emitters as they were before GLOW-EASE (and before the candle).
    pub const STEPS: Ease = Ease {
        fade_s: 0.0,
        flicker: 0.0,
        flicker_s: 1.0,
        nectar: false,
        cycle: false,
        cycle_s: 1.0,
        candle: None,
    };

    /// A latticevine flower's level at nectar fill `fill` (its nectar over what a flower
    /// opens with): the fill itself, so 0 only when empty; 1 whatever the
    /// fill with `flower_nectar_glow` off.
    pub fn flower_level(&self, fill: f64) -> f32 {
        if !self.nectar {
            return 1.0;
        }
        if fill.is_nan() { 0.0 } else { fill.clamp(0.0, 1.0) as f32 }
    }

    /// What multiplies an instance's ramped colour at `t`, and its on share: a lantern's
    /// flicker and a lip's candle dim the colour; the flower cycle dims both.
    pub fn modulation(&self, kind: Kind, seed: u64, t: f64) -> (f32, f32) {
        match kind {
            Kind::Steady => (1.0, 1.0),
            Kind::Lantern => (1.0 + self.flicker * flicker(seed, t, f64::from(self.flicker_s)), 1.0),
            Kind::Lip => (self.candle.map_or(1.0, |c| c.brightness(seed, t)), 1.0),
            Kind::Flower => {
                let c = if self.cycle { cycle(seed, t, f64::from(self.cycle_s)) } else { 1.0 };
                (c, c)
            }
        }
    }
}

/// How an instance moves between targets, per frame: see [`Ease::modulation`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A bloomcrown's core and bloom.
    Steady,
    /// A lanternberry's lantern: the candle flicker, on the cell's own phases.
    Lantern,
    /// A latticevine flower: its nectar level, and the flower cycle when it runs.
    Flower,
    /// A glowcap's lip: the glowcap candle of its stand (the seed is the stand's id).
    Lip,
}

/// What one emitting cell should glow this tick: its dossier colour at full (linear light)
/// and its level (`0..=1`). The instance ramps toward `rgb × level`, on share `level`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub cell: u32,
    pub rgb: [f32; 3],
    pub level: f32,
    pub kind: Kind,
    /// The phases' seed: the stand's id for a lip (its candle), the cell's for the rest.
    pub seed: u64,
}

/// A straight line from `from` to `to` (colour and on share) over the fade, from `start`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Ramp {
    from: [f32; 4],
    to: [f32; 4],
    start: f64,
}

impl Ramp {
    fn at(&self, t: f64, fade: f64) -> [f32; 4] {
        let p = if fade > 0.0 { ((t - self.start) / fade).clamp(0.0, 1.0) as f32 } else { 1.0 };
        std::array::from_fn(|k| self.from[k] + (self.to[k] - self.from[k]) * p)
    }

    fn done(&self, t: f64, fade: f64) -> bool {
        fade <= 0.0 || t - self.start >= fade
    }
}

#[derive(Clone, Copy, Debug)]
struct Instance {
    kind: Kind,
    seed: u64,
    ramp: Ramp,
    /// Targeted by the last update.
    live: bool,
}

/// The emitting cells and their ramps: see the module.
#[derive(Debug)]
pub struct GlowEase {
    pub ease: Ease,
    instances: FxHashMap<u32, Instance>,
    last: Option<f64>,
    released: Vec<u32>,
    gone: Vec<u32>,
}

impl GlowEase {
    pub fn new(ease: Ease) -> GlowEase {
        GlowEase {
            ease,
            instances: FxHashMap::default(),
            last: None,
            released: Vec::new(),
            gone: Vec::new(),
        }
    }

    /// This tick's targets at presenter time `now` (seconds). A cell targeted for the first
    /// time ramps up from nothing, one whose target moved ramps to it from what it shows,
    /// one no longer targeted ramps to nothing; the first update and a jump of the clock
    /// ([`JUMP_S`]) draw every target as it stands.
    pub fn update(&mut self, now: f64, targets: impl IntoIterator<Item = Target>) {
        let snap = self.ease.fade_s <= 0.0
            || match self.last {
                None => true,
                Some(last) => now < last || now - last > JUMP_S,
            };
        let fade = f64::from(self.ease.fade_s);
        self.released.clear();
        self.gone.clear();
        let was: Vec<u32> = self.instances.iter().filter(|(_, i)| i.live).map(|(&c, _)| c).collect();
        for i in self.instances.values_mut() {
            i.live = false;
        }
        for t in targets {
            let to = [t.rgb[0] * t.level, t.rgb[1] * t.level, t.rgb[2] * t.level, t.level];
            let i = self.instances.entry(t.cell).or_insert(Instance {
                kind: t.kind,
                seed: t.seed,
                ramp: Ramp { from: [0.0; 4], to, start: now },
                live: true,
            });
            i.kind = t.kind;
            i.seed = t.seed;
            i.live = true;
            retarget(&mut i.ramp, to, now, fade, snap);
        }
        for c in was {
            if let Some(i) = self.instances.get_mut(&c)
                && !i.live
            {
                self.released.push(c);
                retarget(&mut i.ramp, [0.0; 4], now, fade, snap);
            }
        }
        let gone = &mut self.gone;
        self.instances.retain(|&c, i| {
            let keep = i.live || (!snap && (i.ramp.to != [0.0; 4] || !i.ramp.done(now, fade)));
            if !keep {
                gone.push(c);
            }
            keep
        });
        self.released.sort_unstable();
        self.gone.sort_unstable();
        self.last = Some(now);
    }

    /// The cells the last update let go of (targeted the update before, not this one),
    /// ascending: each still draws what it drew while it ramps out.
    pub fn released(&self) -> &[u32] {
        &self.released
    }

    /// The cells the last update forgot (ramped out), ascending.
    pub fn gone(&self) -> &[u32] {
        &self.gone
    }

    /// Whether `cell` is ramping out: no longer targeted, not yet at nothing.
    pub fn holds(&self, cell: u32) -> bool {
        self.instances.get(&cell).is_some_and(|i| !i.live)
    }

    /// Whether any cell is ramping out.
    pub fn holding(&self) -> bool {
        self.instances.values().any(|i| !i.live)
    }

    pub fn len(&self) -> usize {
        self.instances.len()
    }

    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// What `cell` draws at `t`: its colour (linear light) and on share.
    pub fn value(&self, cell: u32, t: f64) -> Option<([f32; 3], f32)> {
        self.instances.get(&cell).map(|i| self.of(i, t))
    }

    /// Every instance's cell, colour and on share at `t`, in no order.
    pub fn each(&self, t: f64, mut f: impl FnMut(u32, [f32; 3], f32)) {
        for (&c, i) in &self.instances {
            let (rgb, on) = self.of(i, t);
            f(c, rgb, on);
        }
    }

    fn of(&self, i: &Instance, t: f64) -> ([f32; 3], f32) {
        let v = i.ramp.at(t, f64::from(self.ease.fade_s));
        let (light, on) = self.ease.modulation(i.kind, i.seed, t);
        ([v[0] * light, v[1] * light, v[2] * light], (v[3] * on).clamp(0.0, 1.0))
    }
}

/// Point `ramp` at `to` from what it shows at `now`, unless it already heads there.
fn retarget(ramp: &mut Ramp, to: [f32; 4], now: f64, fade: f64, snap: bool) {
    if snap {
        *ramp = Ramp { from: to, to, start: now };
        return;
    }
    if ramp.to.iter().zip(&to).all(|(a, b)| (a - b).abs() <= SAME) {
        return;
    }
    *ramp = Ramp { from: ramp.at(now, fade), to, start: now };
}

/// **The lantern flicker**: smooth band-limited noise in `[−1, 1]`, mean 0 over time.
/// Three sines of about `period` (times 1, 0.71 and 1.43, each ±15 % by `seed`), at
/// `seed`'s phases, averaged: no frame-to-frame noise, so it can't read as stepping.
pub fn flicker(seed: u64, t: f64, period: f64) -> f32 {
    if !(period > 0.0) {
        return 0.0;
    }
    let mut h = mix(seed ^ 0x6c61_6e74_6572_6e21);
    let mut x = 0.0;
    for ratio in PACES {
        h = mix(h);
        let phase = (h >> 11) as f64 / (1u64 << 53) as f64 * TAU;
        let pace = 0.85 + 0.3 * ((h & 0xffff) as f64 / 65535.0);
        x += (TAU * t / (period * ratio * pace) + phase).sin();
    }
    (x / PACES.len() as f64) as f32
}

/// **The flower cycle** (the candidate): a flower's glow swelling and fading through
/// `period`, `seed`'s own phase, in `[0, 1]`: a sine through a smoothstep, so it lingers
/// near on and near off.
pub fn cycle(seed: u64, t: f64, period: f64) -> f32 {
    if !(period > 0.0) {
        return 1.0;
    }
    let phase = (mix(seed ^ 0x666c_6f77_6572_7321) >> 11) as f64 / (1u64 << 53) as f64 * TAU;
    let s = 0.5 + 0.5 * (TAU * t / period + phase).sin();
    (s * s * (3.0 - 2.0 * s)) as f32
}

/// A cell's phase seed: its texture index, mixed.
pub fn cell_seed(cell: u32) -> u64 {
    mix(u64::from(cell) ^ 0x6365_6c6c_7365_6564)
}

#[cfg(test)]
#[path = "glow_ease_tests.rs"]
mod tests;
