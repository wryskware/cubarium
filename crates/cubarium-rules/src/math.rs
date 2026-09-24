//! The float functions a rule may call, one name on both targets.
//!
//! `core` has no `floor`, `sqrt`, `exp`, `tanh`, `sin` or `cos` for `f64` — they are
//! `std`'s, because on the host they are libm's. So a rule calls these instead: on the
//! host they **are** the `std` methods (the CPU simulation's numbers do not move), and on
//! nvptx64 they are CUDA's libdevice (`__nv_*`), linked into the PTX by the kernel crate's
//! build (`-C link-arg=.../libdevice.10.bc`), which is what CUDA C++ calls too. (That
//! kernel crate is on the `rules-trial` branch; the device half is kept so it can return.)

#[cfg(not(target_arch = "nvptx64"))]
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

#[cfg(target_arch = "nvptx64")]
mod imp {
    unsafe extern "C" {
        fn __nv_floor(x: f64) -> f64;
        fn __nv_ceil(x: f64) -> f64;
        fn __nv_sqrt(x: f64) -> f64;
        fn __nv_exp(x: f64) -> f64;
        fn __nv_tanh(x: f64) -> f64;
        fn __nv_sin(x: f64) -> f64;
        fn __nv_cos(x: f64) -> f64;
        fn __nv_fmod(x: f64, y: f64) -> f64;
    }
    #[inline(always)]
    pub fn floor(x: f64) -> f64 {
        unsafe { __nv_floor(x) }
    }
    #[inline(always)]
    pub fn ceil(x: f64) -> f64 {
        unsafe { __nv_ceil(x) }
    }
    #[inline(always)]
    pub fn sqrt(x: f64) -> f64 {
        unsafe { __nv_sqrt(x) }
    }
    #[inline(always)]
    pub fn exp(x: f64) -> f64 {
        unsafe { __nv_exp(x) }
    }
    #[inline(always)]
    pub fn tanh(x: f64) -> f64 {
        unsafe { __nv_tanh(x) }
    }
    #[inline(always)]
    pub fn sin(x: f64) -> f64 {
        unsafe { __nv_sin(x) }
    }
    #[inline(always)]
    pub fn cos(x: f64) -> f64 {
        unsafe { __nv_cos(x) }
    }
    /// `f64::rem_euclid` for `m > 0`. The two ranges a pose or a heading is ever in are
    /// answered without `fmod` (`x - m` is exact there); otherwise `fmod`, which is exact.
    /// On the device, plain `%` would lower to a divide-truncate-multiply, which is not.
    #[inline(always)]
    pub fn rem_euclid(x: f64, m: f64) -> f64 {
        let r = if x >= 0.0 && x < m {
            x
        } else if x >= m && x < 2.0 * m {
            x - m
        } else {
            unsafe { __nv_fmod(x, m) }
        };
        if r < 0.0 { r + m.abs() } else { r }
    }
}

pub use imp::*;
