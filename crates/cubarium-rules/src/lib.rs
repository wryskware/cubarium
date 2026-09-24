//! `cubarium-rules`: simulation rules written once, as plain allocation-free functions
//! over plain data, so one source can serve the CPU and, later, a GPU.
//!
//! Brought over from the single-source trial (`rules-trial`, faabb5b;
//! `design/handoffs/voxel-single-source-gpu-trial-2026-09-23.md`) for the water part only
//! (`design/handoffs/voxel-water-parallel-2026-09-24.md`). The trial also compiled this
//! crate to PTX for CUDA kernels; that driver, its kernel crate and the fauna rules stayed
//! on `rules-trial`. The crate is kept `no_std`-compatible (on `nvptx64` it is `no_std`)
//! so the GPU driver can come back.
//!
//! **The restricted style**, which is what a rule here has to be written in:
//! - no allocation and no `std` in a rule: slices, fixed arrays, `Option`, iterators,
//!   generics and closures are all fine;
//! - data is read through slices or small traits and written through closures or traits
//!   the caller supplies, so one rule serves a host's `Vec`s and a kernel's raw pointers;
//! - floats go through [`math`] (`std` on the host), not `f64`'s inherent `floor`, which
//!   `core` does not have;
//! - no hash maps, no dynamic dispatch on the hot path.
//!
//! Parity between drivers is **statistical**: they sum in different orders.
//!
//! - [`water`]: the store arithmetic, the local exchange, and the column phases (fall,
//!   infiltration, drainage, the water table), per column; host drivers in
//!   [`water::host`].

#![cfg_attr(target_arch = "nvptx64", no_std)]
// The rules mirror the code they were moved from, loops and comparisons included.
#![allow(
    clippy::needless_range_loop,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::collapsible_if,
    clippy::too_many_arguments
)]

pub mod math;
pub mod water;
