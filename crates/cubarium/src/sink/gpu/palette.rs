//! The GPU style table as a persistent palette: a style keeps its slot from tick to tick
//! while any texel draws with it, so a texel whose style did not change does not change
//! either, and a change-only pack has nothing to write for it.
//!
//! Each slot counts the texels drawing with it. A slot whose count falls to zero is
//! free at once, and the next new style takes the lowest free slot first, so a palette
//! started empty numbers its styles in first-need order, as the per-tick table did. A
//! frame that needs more than [`MAX_STYLES`] styles at once overflows exactly as before
//! (the caller falls back to a slot it already holds); styles that come and go over time
//! never run it out.

use cubarium_gpu::voxel::{MAX_STYLES, VoxelStyle};

use crate::voxel::stand::Style;

/// One GPU style slot's contents: the colours, the texture role (`ROLE_*`), the species
/// face slots (`VoxelStyle::with_faces`) and the emission (`emit_key`: the colour's bits,
/// then the whole-cell bit and the candle curve above it).
pub(crate) type StyleKey = (Style, u8, [Option<u16>; 2], Option<[u32; 4]>);

/// [`StyleKey`] as plain bits, for hashing and exact comparison.
pub(crate) type KeyBits = [u32; 18];

/// A key's bits: equal bits, equal slot contents.
pub(crate) fn key_bits(key: &StyleKey) -> KeyBits {
    let (style, role, faces, emit) = key;
    let mut k = [0u32; 18];
    for (i, v) in style.wood.iter().chain(&style.crown).chain(&style.heart).enumerate() {
        k[i] = v.to_bits();
    }
    k[9] = u32::from(*role);
    k[10] = faces[0].map_or(u32::MAX, u32::from);
    k[11] = faces[1].map_or(u32::MAX, u32::from);
    if let Some(e) = emit {
        k[12] = 1;
        k[13..17].copy_from_slice(e);
    }
    k
}

/// One style slot's contents as the GPU reads them.
pub(crate) fn voxel_style(key: &StyleKey) -> VoxelStyle {
    let (style, role, faces, emit) = key;
    VoxelStyle::new(style.wood, style.crown, style.heart)
        .with_role(*role)
        .with_faces(*faces)
        .with_emit(emit.map(|e| [e[0], e[1], e[2]].map(f32::from_bits)))
        .emit_whole(emit.is_some_and(|e| e[3] & 1 != 0))
}

/// A slot's candle curve: `0` steady, else the curve its emission follows.
pub(crate) fn curve_of(key: &StyleKey) -> u8 {
    key.3.map_or(0, |e| (e[3] >> 1) as u8)
}

/// The style table: slot contents, live counts and the free slots.
pub(crate) struct Palette {
    keys: Vec<Option<StyleKey>>,
    count: Vec<u32>,
    map: rustc_hash::FxHashMap<KeyBits, u8>,
    /// Free slots, highest first, so the lowest is taken next.
    free: Vec<u8>,
    version: u64,
    made: u64,
}

impl Palette {
    pub(crate) fn new() -> Palette {
        let mut p = Palette {
            keys: vec![None; MAX_STYLES],
            count: vec![0; MAX_STYLES],
            map: Default::default(),
            free: Vec::with_capacity(MAX_STYLES),
            version: 0,
            made: 0,
        };
        p.reset();
        p
    }

    /// Every slot free again, as a fresh palette.
    pub(crate) fn reset(&mut self) {
        self.keys.fill(None);
        self.count.fill(0);
        self.map.clear();
        self.free.clear();
        self.free.extend((0..MAX_STYLES as u16).rev().map(|s| s as u8));
        self.version += 1;
    }

    /// The slot holding `key`, if one does.
    pub(crate) fn find(&self, key: &StyleKey) -> Option<u8> {
        self.map.get(&key_bits(key)).copied()
    }

    /// One more texel draws with `key`: its slot, given one if it has none, or `None` when
    /// every slot holds a live style.
    pub(crate) fn acquire(&mut self, key: &StyleKey) -> Option<u8> {
        let bits = key_bits(key);
        let slot = match self.map.get(&bits) {
            Some(&s) => s,
            None => {
                let s = self.free.pop()?;
                self.keys[usize::from(s)] = Some(*key);
                self.map.insert(bits, s);
                self.version += 1;
                self.made += 1;
                s
            }
        };
        self.count[usize::from(slot)] += 1;
        Some(slot)
    }

    /// One more texel draws with the style already in `slot`.
    #[cfg(test)]
    pub(crate) fn retain(&mut self, slot: u8) {
        debug_assert!(self.keys[usize::from(slot)].is_some());
        self.count[usize::from(slot)] += 1;
    }

    /// One texel fewer draws with `slot`; at zero the slot is free.
    pub(crate) fn release(&mut self, slot: u8) {
        let s = usize::from(slot);
        debug_assert!(self.count[s] > 0, "releasing a free slot");
        self.count[s] = self.count[s].saturating_sub(1);
        if self.count[s] == 0
            && let Some(key) = self.keys[s].take()
        {
            self.map.remove(&key_bits(&key));
            self.free.push(slot);
            self.version += 1;
        }
    }

    /// What `slot` holds, `None` when it is free.
    pub(crate) fn key(&self, slot: u8) -> Option<&StyleKey> {
        self.keys[usize::from(slot)].as_ref()
    }

    /// Slots holding a live style.
    pub(crate) fn live(&self) -> usize {
        MAX_STYLES - self.free.len()
    }

    /// Styles given a slot since the palette was made.
    pub(crate) fn made(&self) -> u64 {
        self.made
    }

    /// Bumped whenever a slot's contents change: the key the style plane is uploaded under.
    pub(crate) fn version(&self) -> u64 {
        self.version
    }

    /// The style table and its emission curves as the GPU reads them; a free slot is zero.
    pub(crate) fn write(&self, styles: &mut [VoxelStyle], curves: &mut [u8; MAX_STYLES]) {
        for (slot, key) in self.keys.iter().enumerate() {
            match key {
                Some(k) => {
                    styles[slot] = voxel_style(k);
                    curves[slot] = curve_of(k);
                }
                None => {
                    styles[slot] = VoxelStyle::default();
                    curves[slot] = 0;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u32) -> StyleKey {
        let c = [n as f32, 0.5, 0.25];
        (
            Style {
                wood: c,
                crown: c,
                heart: c,
            },
            0,
            [None; 2],
            None,
        )
    }

    /// Styles that come and go: thousands over time, at most eight at once, never run
    /// the table out, and a freed slot is taken by the next new style.
    #[test]
    fn styles_that_come_and_go_never_exhaust_the_table() {
        let mut p = Palette::new();
        let mut live: std::collections::VecDeque<(u32, u8)> = Default::default();
        for n in 0..4 * MAX_STYLES as u32 {
            // Three texels each.
            let slot = p.acquire(&key(n)).expect("a slot for a new style");
            assert_eq!(p.acquire(&key(n)), Some(slot), "the same style, the same slot");
            p.retain(slot);
            assert_eq!(p.find(&key(n)), Some(slot));
            live.push_back((n, slot));
            if live.len() > 8 {
                let (old, s) = live.pop_front().unwrap();
                for _ in 0..3 {
                    p.release(s);
                }
                assert_eq!(p.find(&key(old)), None, "a style nothing draws with is gone");
                assert!(p.key(s).is_none(), "and its slot is free");
            }
            assert!(p.live() <= 9);
        }
        // A fresh palette numbers in first-need order.
        p.reset();
        assert_eq!(p.acquire(&key(1)), Some(0));
        assert_eq!(p.acquire(&key(2)), Some(1));
        let v = p.version();
        p.release(0);
        assert_ne!(p.version(), v, "freeing a slot changes the table");
        let mut styles = vec![VoxelStyle::new([0.5; 3], [0.5; 3], [0.5; 3]); MAX_STYLES];
        let mut curves = [7u8; MAX_STYLES];
        p.write(&mut styles, &mut curves);
        assert_eq!(styles[0], VoxelStyle::default(), "a free slot is written zero");
        assert_ne!(styles[1], VoxelStyle::default());
        assert!(curves.iter().all(|&c| c == 0));
    }

    /// More styles at once than the table holds: the overflow is refused, not aliased.
    #[test]
    fn a_full_table_refuses_a_new_style() {
        let mut p = Palette::new();
        for n in 0..MAX_STYLES as u32 {
            assert!(p.acquire(&key(n)).is_some());
        }
        assert_eq!(p.acquire(&key(9999)), None);
        assert_eq!(p.live(), MAX_STYLES);
    }
}
