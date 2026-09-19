//! The parameter vector: the exact flatten/unflatten order and the seeded centre.
//!
//! A [`Gru32`] is six tensors. The trainer evolves one flat `Vec<f64>` of
//! [`PARAMS`] = 10,215 values, and this module is the **only** place that decides
//! which value is which. The order is fixed and documented here because a trainer
//! that flattened differently would silently learn a permuted network:
//!
//! | range | tensor | shape | layout |
//! | --- | --- | --- | --- |
//! | `0 .. 3H·I` | `w_i` | `[3H × I]` | row-major, gate rows in `(r, z, n)` order |
//! | `.. + 3H·H` | `w_h` | `[3H × H]` | row-major, same gate order |
//! | `.. + 3H` | `b_i` | `[3H]` | same gate order |
//! | `.. + 3H` | `b_h` | `[3H]` | same gate order |
//! | `.. + O·H` | `w_o` | `[O × H]` | row-major |
//! | `.. + O` | `b_o` | `[O]` | — |
//!
//! That is the declaration order of the struct's own fields, so the mapping is
//! readable next to `crate::neural::gru`. Nothing here reorders the GRU's loops:
//! the world still evaluates gates in `(r, z, n)` order for each animal in slot order.

use cubarium_core::neural::gru::{
    parameter_count, Gru, GATES, GRU_PARAMETERS, HIDDEN, INPUT, OUTPUT, Z,
};
use cubarium_core::neural::{ActionAdapter, Gru32, Policy, ShapePolicy};

use super::rng::{gaussian, stream};

/// Number of evolved scalars: `3·H·(I + H + 2) + O·(H + 1)` = 10,215.
pub const PARAMS: usize = GRU_PARAMETERS;

/// Gain of the fan-in-scaled Gaussian used for every **matrix** weight at initialisation.
///
/// A row of `w_i` has `I = 70` inputs, a row of `w_h` and of `w_o` has `H = 32`. Each element
/// is drawn `N(0, (GAIN / sqrt(fan_in))²)`, so a gate pre-activation built from inputs in
/// `[0, 1]` has a standard deviation of about `GAIN · rms(x)` regardless of width. At
/// `GAIN = 0.5` that is well inside the non-saturating part of `σ` and `tanh`, which is the
/// property the contract asks initialisation to have. It is a starting choice, not a tuned one.
pub const INIT_GAIN: f64 = 0.5;

/// Update-gate retention timescales, in **controller updates** (one update is 0.1 s).
///
/// `H = 32` units are split into four fixed groups of eight. Group `k` gets
/// `b_hz = logit(exp(-1 / tau_k))`, so a unit that receives no input drive retains a fraction
/// `z = exp(-1/tau)` of its state per update and forgets with time constant `tau`. Every other
/// bias is zero. The groups are fixed, not drawn: two runs with different training seeds start
/// from the same retention structure and differ only in their matrix weights.
pub const RETENTION_TAUS: [f64; 4] = [10.0, 30.0, 100.0, 300.0];

/// P1-A replaces these frozen layout strings with its manifest serialization; this is the one
/// integration hook, kept in search rather than core so the neural crate stays voxel-agnostic.
pub const BLIND_VOXEL_SCHEMA_TEXT: &str = "voxel-blind|inputs:23(Self[8],Contact[5],Wet[2],Taste[3],Chem[3],Light[2])|actions:forward[0,1],turn[1],feed[2]|gru32-reset-after|cadence:0.25s";
pub const BROWSER_VOXEL_SCHEMA_TEXT: &str = "voxel-browser|inputs:37(Self[8],Contact[5],Wet[2],Taste[3],Cone[19])|actions:forward[0,1],turn[1],feed[2]|gru32-reset-after|cadence:0.25s";

pub fn voxel_schema_digest(manifest_text: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in manifest_text.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Where each tensor starts inside the flat vector.
pub const OFF_W_I: usize = 0;
pub const OFF_W_H: usize = OFF_W_I + GATES * INPUT;
pub const OFF_B_I: usize = OFF_W_H + GATES * HIDDEN;
pub const OFF_B_H: usize = OFF_B_I + GATES;
pub const OFF_W_O: usize = OFF_B_H + GATES;
pub const OFF_B_O: usize = OFF_W_O + OUTPUT * HIDDEN;

/// Shape-parameterized offsets, with the legacy constants above remaining byte-for-byte flat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapeOffsets {
    pub w_i: usize,
    pub w_h: usize,
    pub b_i: usize,
    pub b_h: usize,
    pub w_o: usize,
    pub b_o: usize,
    pub params: usize,
}
pub const fn shape_offsets(input: usize, output: usize) -> ShapeOffsets {
    let w_h = GATES * input;
    let b_i = w_h + GATES * HIDDEN;
    let b_h = b_i + GATES;
    let w_o = b_h + GATES;
    let b_o = w_o + output * HIDDEN;
    ShapeOffsets {
        w_i: 0,
        w_h,
        b_i,
        b_h,
        w_o,
        b_o,
        params: b_o + output,
    }
}

/// `theta` → weights. Exact: no scaling, no clipping, no reordering.
pub fn unflatten(theta: &[f64]) -> Result<Gru32, String> {
    if theta.len() != PARAMS {
        return Err(format!(
            "parameter vector has {} values, expected {PARAMS}",
            theta.len()
        ));
    }
    Ok(Gru32 {
        w_i: theta[OFF_W_I..OFF_W_H].to_vec(),
        w_h: theta[OFF_W_H..OFF_B_I].to_vec(),
        b_i: theta[OFF_B_I..OFF_B_H].to_vec(),
        b_h: theta[OFF_B_H..OFF_W_O].to_vec(),
        w_o: theta[OFF_W_O..OFF_B_O].to_vec(),
        b_o: theta[OFF_B_O..].to_vec(),
    })
}

/// Weights → `theta`. The exact inverse of [`unflatten`].
pub fn flatten(w: &Gru32) -> Vec<f64> {
    let mut v = Vec::with_capacity(PARAMS);
    v.extend_from_slice(&w.w_i);
    v.extend_from_slice(&w.w_h);
    v.extend_from_slice(&w.b_i);
    v.extend_from_slice(&w.b_h);
    v.extend_from_slice(&w.w_o);
    v.extend_from_slice(&w.b_o);
    v
}

pub fn unflatten_shape<const I: usize, const O: usize>(theta: &[f64]) -> Result<Gru<I, O>, String> {
    let o = shape_offsets(I, O);
    if theta.len() != o.params {
        return Err(format!(
            "parameter vector has {} values, expected {}",
            theta.len(),
            o.params
        ));
    }
    Ok(Gru {
        w_i: theta[o.w_i..o.w_h].to_vec(),
        w_h: theta[o.w_h..o.b_i].to_vec(),
        b_i: theta[o.b_i..o.b_h].to_vec(),
        b_h: theta[o.b_h..o.w_o].to_vec(),
        w_o: theta[o.w_o..o.b_o].to_vec(),
        b_o: theta[o.b_o..].to_vec(),
    })
}

pub fn flatten_shape<const I: usize, const O: usize>(w: &Gru<I, O>) -> Vec<f64> {
    let mut v = Vec::with_capacity(parameter_count(I, O));
    v.extend_from_slice(&w.w_i);
    v.extend_from_slice(&w.w_h);
    v.extend_from_slice(&w.b_i);
    v.extend_from_slice(&w.b_h);
    v.extend_from_slice(&w.w_o);
    v.extend_from_slice(&w.b_o);
    v
}

pub fn initial_center_shape<const I: usize, const O: usize>(seed: u64) -> Vec<f64> {
    let o = shape_offsets(I, O);
    let mut theta = vec![0.; o.params];
    let si = INIT_GAIN / (I as f64).sqrt();
    let sh = INIT_GAIN / (HIDDEN as f64).sqrt();
    for (i, x) in theta[o.w_i..o.w_h].iter_mut().enumerate() {
        *x = si * gaussian(seed, stream::ES_INIT, 0, i as u64);
    }
    for (i, x) in theta[o.w_h..o.b_i].iter_mut().enumerate() {
        *x = sh * gaussian(seed, stream::ES_INIT, 1, i as u64);
    }
    for (i, x) in theta[o.w_o..o.b_o].iter_mut().enumerate() {
        *x = sh * gaussian(seed, stream::ES_INIT, 2, i as u64);
    }
    let per = HIDDEN / RETENTION_TAUS.len();
    for unit in 0..HIDDEN {
        theta[o.b_h + Z + unit] =
            retention_bias(RETENTION_TAUS[(unit / per).min(RETENTION_TAUS.len() - 1)]);
    }
    theta
}

pub fn shape_policy<const I: usize, const O: usize>(
    theta: &[f64],
    digest: u64,
) -> Result<ShapePolicy<I, O>, String> {
    let policy = ShapePolicy::new(unflatten_shape(theta)?, digest);
    policy.validate(digest)?;
    Ok(policy)
}

/// A policy from a parameter vector, stamped with this build's schema digest.
pub fn policy(theta: &[f64]) -> Result<Policy, String> {
    policy_in(theta, ActionAdapter::CubAct1)
}

/// [`policy`], stamped for a named [`ActionAdapter`]. `CubAct1` is [`policy`] itself.
///
/// The **weights are untouched**: the adapter changes only which schema digest the policy
/// carries, and so which world will accept it. That is exactly what a paired replay of one
/// weight set under two adapters needs.
pub fn policy_in(theta: &[f64], adapter: ActionAdapter) -> Result<Policy, String> {
    let policy = Policy::new_in(unflatten(theta)?, adapter);
    policy.validate_in(adapter)?;
    Ok(policy)
}

/// `logit(exp(-1 / tau))`: the update-gate bias whose retention is `exp(-1/tau)` per update.
pub fn retention_bias(tau: f64) -> f64 {
    let z = (-1.0 / tau).exp();
    (z / (1.0 - z)).ln()
}

/// The seeded centre: fan-in-scaled Gaussian matrices, zero biases except the four fixed
/// update-gate retention groups.
///
/// Every draw is a pure function of `(seed, element index)`, so the centre does not depend on
/// the order anything was generated in.
pub fn initial_center(seed: u64) -> Vec<f64> {
    let mut theta = vec![0.0; PARAMS];
    let scale_i = INIT_GAIN / (INPUT as f64).sqrt();
    let scale_h = INIT_GAIN / (HIDDEN as f64).sqrt();
    for (i, slot) in theta[OFF_W_I..OFF_W_H].iter_mut().enumerate() {
        *slot = scale_i * gaussian(seed, stream::ES_INIT, 0, i as u64);
    }
    for (i, slot) in theta[OFF_W_H..OFF_B_I].iter_mut().enumerate() {
        *slot = scale_h * gaussian(seed, stream::ES_INIT, 1, i as u64);
    }
    for (i, slot) in theta[OFF_W_O..OFF_B_O].iter_mut().enumerate() {
        *slot = scale_h * gaussian(seed, stream::ES_INIT, 2, i as u64);
    }
    // `b_i`, `b_o` and the `r`/`n` halves of `b_h` stay at zero. The `z` rows carry the
    // retention groups: eight units per timescale, in unit order.
    let per_group = HIDDEN / RETENTION_TAUS.len();
    for unit in 0..HIDDEN {
        let tau = RETENTION_TAUS[(unit / per_group).min(RETENTION_TAUS.len() - 1)];
        theta[OFF_B_H + Z + unit] = retention_bias(tau);
    }
    theta
}

/// A one-line description of the initialisation, for the frozen protocol record.
pub fn init_description() -> String {
    format!(
        "matrix weights N(0, (gain/sqrt(fan_in))^2) with gain {INIT_GAIN} \
         (w_i fan_in {INPUT} -> sd {:.6}, w_h/w_o fan_in {HIDDEN} -> sd {:.6}); \
         all biases zero except b_hz in four fixed groups of {} units at tau = {:?} updates \
         (b_hz = {:?})",
        INIT_GAIN / (INPUT as f64).sqrt(),
        INIT_GAIN / (HIDDEN as f64).sqrt(),
        HIDDEN / RETENTION_TAUS.len(),
        RETENTION_TAUS,
        RETENTION_TAUS.map(retention_bias),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parameter_count_is_the_contracts() {
        assert_eq!(PARAMS, 10_215);
        assert_eq!(OFF_B_O + OUTPUT, PARAMS);
    }

    #[test]
    fn flatten_and_unflatten_round_trip_exactly() {
        let theta = initial_center(20_260_915);
        let weights = unflatten(&theta).expect("shape");
        let back = flatten(&weights);
        assert_eq!(
            theta, back,
            "flatten(unflatten(theta)) must be theta, value for value"
        );

        // And the other direction, from a weight set whose every tensor is distinguishable.
        let mut w = Gru32::zeros();
        for (i, x) in w.w_i.iter_mut().enumerate() {
            *x = 1.0 + i as f64;
        }
        for (i, x) in w.w_h.iter_mut().enumerate() {
            *x = -1.0 - i as f64;
        }
        for (i, x) in w.b_i.iter_mut().enumerate() {
            *x = 100.0 + i as f64;
        }
        for (i, x) in w.b_h.iter_mut().enumerate() {
            *x = -100.0 - i as f64;
        }
        for (i, x) in w.w_o.iter_mut().enumerate() {
            *x = 1000.0 + i as f64;
        }
        for (i, x) in w.b_o.iter_mut().enumerate() {
            *x = -1000.0 - i as f64;
        }
        assert_eq!(unflatten(&flatten(&w)).expect("shape"), w);
    }

    #[test]
    fn unflatten_refuses_a_wrong_length() {
        assert!(unflatten(&[0.0; 3]).is_err());
    }

    #[test]
    fn the_centre_is_finite_seeded_and_non_saturating() {
        let a = initial_center(7);
        let b = initial_center(7);
        let c = initial_center(8);
        assert_eq!(a, b, "the centre is a pure function of the seed");
        assert_ne!(a, c);
        assert!(a.iter().all(|x| x.is_finite()));
        let max_matrix = a[OFF_W_I..OFF_B_I]
            .iter()
            .fold(0.0f64, |m, x| m.max(x.abs()));
        assert!(
            max_matrix < 1.0,
            "matrix weights should be small, saw {max_matrix}"
        );
        // Biases: zero everywhere except the update-gate rows.
        assert!(a[OFF_B_I..OFF_B_H].iter().all(|x| *x == 0.0));
        assert!(a[OFF_B_O..].iter().all(|x| *x == 0.0));
        for unit in 0..HIDDEN {
            let b = a[OFF_B_H + Z + unit];
            let tau = RETENTION_TAUS[unit / (HIDDEN / RETENTION_TAUS.len())];
            assert!((b - retention_bias(tau)).abs() < 1e-12);
        }
    }

    #[test]
    fn retention_biases_hit_their_timescales() {
        for tau in RETENTION_TAUS {
            let b = retention_bias(tau);
            let z = 1.0 / (1.0 + (-b).exp());
            assert!(
                (z - (-1.0 / tau).exp()).abs() < 1e-12,
                "tau {tau} gives z {z}"
            );
        }
    }

    #[test]
    fn the_centre_is_a_policy_this_build_accepts() {
        let p = policy(&initial_center(3)).expect("a valid policy");
        assert_eq!(p.schema_digest, cubarium_core::neural::schema_digest());
    }

    #[test]
    fn voxel_shapes_have_contract_counts_and_exact_round_trip() {
        assert_eq!(shape_offsets(23, 3).params, 5_571);
        assert_eq!(shape_offsets(37, 3).params, 6_915);
        for theta in [
            initial_center_shape::<23, 3>(9),
            initial_center_shape::<37, 3>(9),
        ] {
            if theta.len() == 5_571 {
                assert_eq!(
                    theta,
                    flatten_shape(&unflatten_shape::<23, 3>(&theta).unwrap())
                );
            } else {
                assert_eq!(
                    theta,
                    flatten_shape(&unflatten_shape::<37, 3>(&theta).unwrap())
                );
            }
        }
        assert!(unflatten_shape::<23, 3>(&[0.; 5_570]).is_err());
    }
}
