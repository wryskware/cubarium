//! The float functions a rule may call, one name on both targets.
//!
//! `core` has no `floor`, `sqrt`, `exp`, `tanh`, `sin` or `cos` for `f64` — they are
//! `std`'s, because on the host they are libm's. So a rule calls these instead: on the
//! host they **are** the `std` methods (the CPU simulation's numbers do not move).
//!
//! The nvptx64 half (CUDA's libdevice `__nv_*`, called through `extern "C"`) was removed
//! on 2026-10-08: calling it is `unsafe`, and the crate forbids `unsafe` without Wrysk's
//! approval. Its kernel crate lives on the `rules-trial` branch (faabb5b), which still
//! has it; bringing a device driver back means asking for that FFI first.

mod imp {
    #[inline(always)]
    pub fn floor(x: f64) -> f64 {
        x.floor()
    }
    #[inline(always)]
    pub fn ceil(x: f64) -> f64 {
        x.ceil()
    }
    #[inline(always)]
    pub fn sqrt(x: f64) -> f64 {
        x.sqrt()
    }
    #[inline(always)]
    pub fn exp(x: f64) -> f64 {
        x.exp()
    }
    #[inline(always)]
    pub fn tanh(x: f64) -> f64 {
        x.tanh()
    }
    #[inline(always)]
    pub fn sin(x: f64) -> f64 {
        x.sin()
    }
    #[inline(always)]
    pub fn cos(x: f64) -> f64 {
        x.cos()
    }
    /// `f64::rem_euclid`.
    #[inline(always)]
    pub fn rem_euclid(x: f64, m: f64) -> f64 {
        x.rem_euclid(m)
    }
}

pub use imp::*;
