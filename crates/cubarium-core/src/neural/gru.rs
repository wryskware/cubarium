//! The shape-generic GRU forward pass and the policy tensor set (contract §5).
//!
//! One GRU layer with caller-selected I, O and H, `f64` throughout. Legacy aliases use H = 32.
//! **Reset-after** convention with PyTorch gate order `(r, z, n)` and two bias vectors per
//! gate:
//!
//! ```text
//! r = σ(W_ir x + b_ir + W_hr h + b_hr)
//! z = σ(W_iz x + b_iz + W_hz h + b_hz)
//! n = tanh(W_in x + b_in + r ⊙ (W_hn h + b_hn))
//! h' = z ⊙ h + (1 − z) ⊙ n
//! y  = W_o h' + b_o
//! ```
//!
//! Tensor layout: `W_i: [3H × I]`, `W_h: [3H × H]`, `b_i, b_h: [3H]`, rows in gate order,
//! row-major. `W_o: [O × H]`, `b_o: [O]`. The loop order is fixed — for each animal in slot
//! order, gates in the order above — so a trainer may not reorder it and get the same world.

use serde::{Deserialize, Serialize};

use super::obs::OBS_LEN;

/// Hidden width shared by every phase-one policy.
pub const HIDDEN: usize = 32;
pub const INPUT: usize = OBS_LEN;
pub const OUTPUT: usize = super::action::ACT_LEN;
pub const GATES: usize = 3 * HIDDEN;
pub const GRU_PARAMETERS: usize = parameter_count(INPUT, OUTPUT);
pub const R: usize = 0;
pub const Z: usize = HIDDEN;
pub const N: usize = 2 * HIDDEN;

pub const fn parameter_count(input: usize, output: usize) -> usize {
    parameter_count_hidden(input, output, HIDDEN)
}

pub const fn parameter_count_hidden(input: usize, output: usize, hidden: usize) -> usize {
    3 * hidden * (input + hidden + 2) + output * (hidden + 1)
}

/// Shape-parameterized GRU arithmetic. Gate order is `(r,z,n)` and recurrence is reset-after.
///
/// The legacy type was a concrete `struct Gru32`; today it is this generic type aliased. Its
/// Serde **type name is deliberately not pinned to `"Gru32"`**: nothing persisted depends on
/// it — snapshot payloads are `postcard`, which is not self-describing and never encodes a
/// type name; the JSON policy artifacts (`es::export::PolicyFile`, `es::trainer::Checkpoint`)
/// store the flat `theta` vector, never a `Gru32`; and no externally-tagged enum in the
/// workspace embeds one. A blanket `#[serde(rename = "Gru32")]` would also mislabel every
/// other instantiation (`Gru23`, `Gru37`), so the generic name stands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gru<const I: usize, const O: usize, const H: usize = HIDDEN> {
    pub w_i: Vec<f64>,
    pub w_h: Vec<f64>,
    pub b_i: Vec<f64>,
    pub b_h: Vec<f64>,
    pub w_o: Vec<f64>,
    pub b_o: Vec<f64>,
}

pub type Gru32 = Gru<INPUT, OUTPUT>;
pub type Gru23 = Gru<23, 3>;
pub type Gru37 = Gru<37, 3>;

impl<const I: usize, const O: usize, const H: usize> Default for Gru<I, O, H> {
    fn default() -> Self {
        Self::zeros()
    }
}
impl<const I: usize, const O: usize, const H: usize> Gru<I, O, H> {
    pub fn zeros() -> Self {
        Self {
            w_i: vec![0.; (3 * H) * I],
            w_h: vec![0.; (3 * H) * H],
            b_i: vec![0.; 3 * H],
            b_h: vec![0.; 3 * H],
            w_o: vec![0.; O * H],
            b_o: vec![0.; O],
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        self.view().validate(I, O, H)
    }
    pub fn view(&self) -> GruView<'_> {
        GruView {
            w_i: &self.w_i,
            w_h: &self.w_h,
            b_i: &self.b_i,
            b_h: &self.b_h,
            w_o: &self.w_o,
            b_o: &self.b_o,
        }
    }

    pub fn parameters(&self) -> usize {
        self.w_i.len()
            + self.w_h.len()
            + self.b_i.len()
            + self.b_h.len()
            + self.w_o.len()
            + self.b_o.len()
    }
    pub fn forward(&self, x: &[f64; I], hidden: &mut [f64; H]) -> [f64; O] {
        let mut next = [0.; H];
        let mut y = [0.; O];
        self.view().forward(x, hidden, &mut next, &mut y);
        y
    }
}

/// Borrowed GRU tensors for manifests whose dimensions are known at runtime.
/// Action and option heads occupy separate row ranges of w_o and b_o, sharing h'.
#[derive(Clone, Copy, Debug)]
pub struct GruView<'a> {
    pub w_i: &'a [f64],
    pub w_h: &'a [f64],
    pub b_i: &'a [f64],
    pub b_h: &'a [f64],
    pub w_o: &'a [f64],
    pub b_o: &'a [f64],
}
impl GruView<'_> {
    pub fn validate(&self, input: usize, output: usize, hidden: usize) -> Result<(), String> {
        if input == 0 || hidden == 0 || output == 0 {
            return Err("GRU dimensions must be nonzero".into());
        }
        for (name, v, want) in [
            ("w_i", self.w_i, 3 * hidden * input),
            ("w_h", self.w_h, 3 * hidden * hidden),
            ("b_i", self.b_i, 3 * hidden),
            ("b_h", self.b_h, 3 * hidden),
            ("w_o", self.w_o, output * hidden),
            ("b_o", self.b_o, output),
        ] {
            if v.len() != want {
                return Err(format!(
                    "policy tensor {name} has {} values, expected {want}",
                    v.len()
                ));
            }
            if let Some(i) = v.iter().position(|x| !x.is_finite()) {
                return Err(format!("policy tensor {name}[{i}] is not finite"));
            }
        }
        Ok(())
    }

    /// Reset-after arithmetic shared by the const-generic and manifest-driven runtimes.
    /// `next` is scratch; neither this function nor the runtime controller allocates.
    pub fn forward(&self, x: &[f64], hidden: &mut [f64], next: &mut [f64], y: &mut [f64]) {
        let h = hidden.len();
        assert_eq!(next.len(), h);
        let gate = |row: usize| {
            let mut gi = self.b_i[row];
            for c in 0..x.len() {
                gi += self.w_i[row * x.len() + c] * x[c];
            }
            let mut gh = self.b_h[row];
            for c in 0..h {
                gh += self.w_h[row * h + c] * hidden[c];
            }
            (gi, gh)
        };
        for j in 0..h {
            let (ir, hr) = gate(j);
            let (iz, hz) = gate(h + j);
            let (inn, hn) = gate(2 * h + j);
            let r = sigmoid(ir + hr);
            let z = sigmoid(iz + hz);
            let n = (inn + r * hn).tanh();
            next[j] = z * hidden[j] + (1. - z) * n;
        }
        hidden.copy_from_slice(next);
        for (o, slot) in y.iter_mut().enumerate() {
            let mut a = self.b_o[o];
            for j in 0..h {
                a += self.w_o[o * h + j] * hidden[j];
            }
            *slot = a;
        }
    }
}

/// A shape-aware policy whose schema meaning is supplied by its caller.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShapePolicy<const I: usize, const O: usize, const H: usize = HIDDEN> {
    pub weights: Gru<I, O, H>,
    pub schema_digest: u64,
}
impl<const I: usize, const O: usize, const H: usize> ShapePolicy<I, O, H> {
    pub fn new(weights: Gru<I, O, H>, schema_digest: u64) -> Self {
        Self {
            weights,
            schema_digest,
        }
    }
    pub fn validate(&self, expected_digest: u64) -> Result<(), String> {
        self.weights.validate()?;
        if self.schema_digest != expected_digest {
            return Err(format!(
                "policy schema_digest {:#018x} does not match expected {:#018x}",
                self.schema_digest, expected_digest
            ));
        }
        Ok(())
    }
}

/// A policy: exact weights plus the digest of the interface they were authored against.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    pub weights: Gru32,
    /// [`super::schema_digest`] at the time the policy was written. A policy whose digest
    /// differs from this build's is refused by name on load, never reinterpreted.
    pub schema_digest: u64,
}

impl Policy {
    pub fn new(weights: Gru32) -> Policy {
        Policy::new_in(weights, super::ActionAdapter::CubAct1)
    }

    /// [`Policy::new`], stamped for a named [`super::ActionAdapter`]. `CubAct1` is
    /// [`Policy::new`] itself.
    pub fn new_in(weights: Gru32, adapter: super::ActionAdapter) -> Policy {
        Policy {
            weights,
            schema_digest: super::schema_digest_in(adapter),
        }
    }

    /// The adapter this policy's digest belongs to, if this build knows one.
    ///
    /// `None` means the digest is foreign to this build altogether — another observation
    /// layout, recurrence convention, motor contract or controller rate — which is the case
    /// [`Policy::validate`] refuses.
    pub fn adapter(&self) -> Option<super::ActionAdapter> {
        super::ActionAdapter::ALL
            .into_iter()
            .find(|a| super::schema_digest_in(*a) == self.schema_digest)
    }

    /// Shape, finiteness, and a digest this **build** can interpret under *some* adapter.
    ///
    /// This is the decode-time range check: it runs on every `WorldState::validate`, including
    /// the one inside a world that is deliberately running `cub-act-2`, so it cannot be the
    /// place that pins one adapter. Which adapter a policy may actually be *run* under is a
    /// property of the world, and [`Policy::validate_in`] is the check the one explicit door
    /// into the extension makes (`World::attach_neural_policy`).
    pub fn validate(&self) -> Result<(), String> {
        self.weights.validate()?;
        if self.adapter().is_none() {
            return Err(format!(
                "policy schema_digest {:#018x} is not this build's {:#018x}: the observation \
                 layout, action set, recurrence convention, motor contract or controller rate \
                 differs, and the weights cannot be reinterpreted",
                self.schema_digest,
                super::schema_digest()
            ));
        }
        Ok(())
    }

    /// [`Policy::validate`], against the adapter actually in force. A policy authored for the
    /// other adapter is refused **by name**: the two decode the same seven numbers differently,
    /// so running one under the other is running a different animal.
    pub fn validate_in(&self, adapter: super::ActionAdapter) -> Result<(), String> {
        self.weights.validate()?;
        let want = super::schema_digest_in(adapter);
        if self.schema_digest != want {
            let named = self
                .adapter()
                .map_or_else(String::new, |a| format!(" (the {} adapter)", a.name()));
            return Err(format!(
                "policy schema_digest {:#018x}{named} is not this world's {:#018x} (the {} \
                 adapter): the observation layout, action set, recurrence convention, motor \
                 contract or controller rate differs, and the weights cannot be reinterpreted",
                self.schema_digest,
                want,
                adapter.name()
            ));
        }
        Ok(())
    }
}

fn sigmoid(x: f64) -> f64 {
    if !x.is_finite() {
        return if x > 0.0 { 1.0 } else { 0.0 };
    }
    1.0 / (1.0 + (-x).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An independent numerical reference for one step of a 2-wide reset-after GRU, written
    /// out by hand from the equations rather than from the implementation, then embedded in a
    /// 32-wide policy whose other 30 units are inert.
    ///
    /// The two units differ in every term that matters: unit 0 exercises the input path and a
    /// nonzero *previous hidden state* through both the update gate and the reset-gated
    /// recurrent term; unit 1 exercises the cross-unit recurrent weight, so a bug that read
    /// `W_h` down its columns instead of along its rows would change the answer.
    #[test]
    fn one_step_matches_a_hand_computed_reset_after_reference() {
        let mut w = Gru32::zeros();
        // Inputs: x[0] = 0.5, x[1] = −0.25, everything else zero.
        let x = {
            let mut x = [0.0f64; INPUT];
            x[0] = 0.5;
            x[1] = -0.25;
            x
        };
        // W_i rows (gate, unit) -> input column.
        let set_i = |w: &mut Gru32, gate: usize, unit: usize, col: usize, v: f64| {
            w.w_i[(gate + unit) * INPUT + col] = v;
        };
        let set_h = |w: &mut Gru32, gate: usize, unit: usize, col: usize, v: f64| {
            w.w_h[(gate + unit) * HIDDEN + col] = v;
        };

        set_i(&mut w, R, 0, 0, 1.0);
        set_i(&mut w, Z, 0, 0, -2.0);
        set_i(&mut w, N, 0, 1, 3.0);
        set_h(&mut w, R, 0, 0, 0.5);
        set_h(&mut w, Z, 0, 0, 0.25);
        set_h(&mut w, N, 0, 0, -1.5);
        w.b_i[R] = 0.1;
        w.b_h[Z] = -0.2;
        w.b_i[N] = 0.05;
        w.b_h[N] = 0.3;

        // Unit 1 reads unit 0's hidden state, not its own.
        set_i(&mut w, Z, 1, 0, 1.0);
        set_h(&mut w, N, 1, 0, 2.0);
        w.b_i[R + 1] = -0.4;
        w.b_i[N + 1] = 0.2;

        // Head: y0 = 2·h0 + 1, y1 = −1·h1.
        w.w_o[0 * HIDDEN] = 2.0;
        w.b_o[0] = 1.0;
        w.w_o[HIDDEN + 1] = -1.0;

        let mut h = [0.0f64; HIDDEN];
        h[0] = 0.4;
        h[1] = -0.6;

        // ---- reference, computed by hand from the equations in this module's header ----
        let sig = |v: f64| 1.0 / (1.0 + (-v as f64).exp());
        // unit 0
        let gi_r0 = 1.0 * 0.5 + 0.1; // 0.6
        let gh_r0 = 0.5 * 0.4 + 0.0; // 0.2
        let r0 = sig(gi_r0 + gh_r0); // σ(0.8)
        let gi_z0 = -2.0 * 0.5 + 0.0; // −1.0
        let gh_z0 = 0.25 * 0.4 - 0.2; // −0.1
        let z0 = sig(gi_z0 + gh_z0); // σ(−1.1)
        let gi_n0 = 3.0 * -0.25 + 0.05; // −0.7
        let gh_n0 = -1.5 * 0.4 + 0.3; // −0.3
        let n0 = (gi_n0 + r0 * gh_n0).tanh();
        let h0 = z0 * 0.4 + (1.0 - z0) * n0;
        // unit 1
        let r1 = sig(-0.4); // no recurrent term
        let z1 = sig(1.0 * 0.5); // σ(0.5)
        let gh_n1 = 2.0 * 0.4; // reads h[0] = 0.4
        let n1 = (0.2 + r1 * gh_n1).tanh();
        let h1 = z1 * -0.6 + (1.0 - z1) * n1;

        let y = w.forward(&x, &mut h);

        assert!((h[0] - h0).abs() < 1e-14, "h0 {} vs {h0}", h[0]);
        assert!((h[1] - h1).abs() < 1e-14, "h1 {} vs {h1}", h[1]);
        assert!((y[0] - (2.0 * h0 + 1.0)).abs() < 1e-14);
        assert!((y[1] - (-h1)).abs() < 1e-14);
        for j in 2..HIDDEN {
            assert_eq!(h[j], 0.0, "unit {j} was inert and stayed inert");
        }
    }

    /// Reset-after is not reset-before: with a nonzero `b_hn` the two conventions differ, and
    /// this pins which one is implemented.
    #[test]
    fn reset_after_differs_from_reset_before_and_this_is_reset_after() {
        let mut w = Gru32::zeros();
        w.w_h[N * HIDDEN] = 1.0; // W_hn[0,0]
        w.b_h[N] = 1.0; // b_hn[0], the term the two conventions treat differently
        w.b_i[R] = 0.0; // r = σ(0) = 0.5
        w.b_i[Z] = -40.0; // z ≈ 0, so h' ≈ n

        let x = [0.0f64; INPUT];
        let mut h = [0.0f64; HIDDEN];
        h[0] = 0.5;
        w.forward(&x, &mut h);

        let r = 0.5f64;
        let after = (r * (1.0 * 0.5 + 1.0)).tanh(); // reset multiplies (W_hn h + b_hn)
        let before = (1.0 * (r * 0.5) + 1.0).tanh(); // reset multiplies h only
        assert!(
            (h[0] - after).abs() < 1e-9,
            "{} is not reset-after {after}",
            h[0]
        );
        assert!(
            (after - before).abs() > 1e-3,
            "the fixture must separate the conventions"
        );
    }

    #[test]
    fn a_zero_policy_is_a_fixed_point_and_the_head_is_the_bias() {
        let w = Gru32::zeros();
        let mut h = [0.0f64; HIDDEN];
        let y = w.forward(&[0.0; INPUT], &mut h);
        // z = σ(0) = ½, n = tanh(0) = 0, so h' = ½·0 + ½·0 = 0.
        assert!(h.iter().all(|v| *v == 0.0));
        assert!(y.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn validate_rejects_a_wrong_shape_or_a_non_finite_weight() {
        let mut w = Gru32::zeros();
        w.b_o.push(0.0);
        assert!(w.validate().unwrap_err().contains("b_o"));
        let mut w = Gru32::zeros();
        w.w_i[17] = f64::NAN;
        assert!(w.validate().unwrap_err().contains("w_i[17]"));
    }

    #[test]
    fn the_shapes_add_up_to_the_contracts_parameter_count() {
        assert_eq!(Gru32::zeros().parameters(), GRU_PARAMETERS);
    }

    #[test]
    fn voxel_shapes_have_the_contract_parameter_counts() {
        assert_eq!(parameter_count(23, 3), 5_571);
        assert_eq!(parameter_count(37, 3), 6_915);
        assert_eq!(Gru23::zeros().parameters(), 5_571);
        assert_eq!(Gru37::zeros().parameters(), 6_915);
    }

    /// The same hand-computed reference logic as
    /// [`one_step_matches_a_hand_computed_reset_after_reference`], run on the **generic**
    /// `Gru` at a small shape (I = 2, O = 2) with nonzero input weights, nonzero recurrent
    /// weights and a cross-unit hidden read in every gate — so an indexing bug in the
    /// shape-parameterized row strides, gate offsets or reset-after term changes the answer
    /// instead of cancelling against a zero weight.
    #[test]
    fn generic_forward_matches_a_hand_computed_cross_unit_reference() {
        let mut w = Gru::<2, 2>::zeros();
        // W_i rows (gate, unit) -> input column; the row stride is the generic I = 2.
        let set_i = |w: &mut Gru<2, 2>, gate: usize, unit: usize, col: usize, v: f64| {
            w.w_i[(gate + unit) * 2 + col] = v;
        };
        let set_h = |w: &mut Gru<2, 2>, gate: usize, unit: usize, col: usize, v: f64| {
            w.w_h[(gate + unit) * HIDDEN + col] = v;
        };

        set_i(&mut w, R, 0, 0, 1.0);
        set_i(&mut w, Z, 0, 1, -2.0);
        set_i(&mut w, N, 0, 0, 3.0);
        set_i(&mut w, N, 0, 1, 0.5);
        set_h(&mut w, R, 0, 0, 0.5);
        set_h(&mut w, Z, 0, 1, 0.25); // unit 0's update gate reads unit 1's hidden state
        set_h(&mut w, N, 0, 0, -1.5);
        w.b_i[R] = 0.1;
        w.b_h[Z] = -0.2;
        w.b_i[N] = 0.05;
        w.b_h[N] = 0.3;

        // Unit 1 also reads unit 0's hidden state, not its own.
        set_i(&mut w, Z, 1, 0, 1.0);
        set_h(&mut w, N, 1, 0, 2.0);
        w.b_i[R + 1] = -0.4;
        w.b_i[N + 1] = 0.2;

        // Head: y0 = 2·h0 + 1, y1 = −1·h1.
        w.w_o[0 * HIDDEN] = 2.0;
        w.b_o[0] = 1.0;
        w.w_o[HIDDEN + 1] = -1.0;

        let x = [0.5, -0.25];
        let mut h = [0.0f64; HIDDEN];
        h[0] = 0.4;
        h[1] = -0.6;

        // ---- reference, computed by hand from the equations in this module's header ----
        let sig = |v: f64| 1.0 / (1.0 + (-v as f64).exp());
        // unit 0
        let gi_r0 = 1.0 * 0.5 + 0.1; // 0.6
        let gh_r0 = 0.5 * 0.4 + 0.0; // 0.2
        let r0 = sig(gi_r0 + gh_r0); // σ(0.8)
        let gi_z0 = -2.0 * -0.25 + 0.0; // 0.5
        let gh_z0 = 0.25 * -0.6 - 0.2; // −0.35, reads h[1]
        let z0 = sig(gi_z0 + gh_z0); // σ(0.15)
        let gi_n0 = 3.0 * 0.5 + 0.5 * -0.25 + 0.05; // 1.425
        let gh_n0 = -1.5 * 0.4 + 0.3; // −0.3
        let n0 = (gi_n0 + r0 * gh_n0).tanh();
        let h0 = z0 * 0.4 + (1.0 - z0) * n0;
        // unit 1
        let r1 = sig(-0.4); // no recurrent term
        let z1 = sig(1.0 * 0.5); // σ(0.5), no recurrent term
        let gh_n1 = 2.0 * 0.4; // reads h[0] = 0.4
        let n1 = (0.2 + r1 * gh_n1).tanh();
        let h1 = z1 * -0.6 + (1.0 - z1) * n1;

        let y = w.forward(&x, &mut h);

        assert!((h[0] - h0).abs() < 1e-14, "h0 {} vs {h0}", h[0]);
        assert!((h[1] - h1).abs() < 1e-14, "h1 {} vs {h1}", h[1]);
        assert!((y[0] - (2.0 * h0 + 1.0)).abs() < 1e-14);
        assert!((y[1] - (-h1)).abs() < 1e-14);
        for j in 2..HIDDEN {
            assert_eq!(h[j], 0.0, "unit {j} was inert and stayed inert");
        }
    }

    /// A shape policy carries its caller's digest and is refused, by both digests' names,
    /// when the expected one differs.
    #[test]
    fn a_shape_policy_refuses_a_wrong_digest() {
        let weights = Gru23::zeros();
        let p = ShapePolicy::new(weights, 0x0123_4567_89ab_cdef);
        p.validate(0x0123_4567_89ab_cdef).expect("matching digest");
        let err = p
            .validate(0xfedc_ba98_7654_3210)
            .expect_err("a foreign digest is refused");
        assert!(err.contains("0123456789abcdef"), "{err}");
        assert!(err.contains("fedcba9876543210"), "{err}");
    }
    #[test]
    fn hidden_width_changes_gate_offsets_and_all_tensor_shapes() {
        fn check<const H: usize>() {
            let mut w = Gru::<2, 2, H>::zeros();
            assert_eq!(w.parameters(), parameter_count_hidden(2, 2, H));
            w.validate().expect("generic dimensions");
            w.b_i[H + H - 1] = 0.7;
            w.b_h[2 * H + H - 1] = 0.3;
            w.w_o[H - 1] = 2.;
            let mut hidden = [0.; H];
            hidden[H - 1] = 0.4;
            let expected = sigmoid(0.7) * 0.4 + (1. - sigmoid(0.7)) * (0.5f64 * 0.3).tanh();
            let y = w.forward(&[0.; 2], &mut hidden);
            assert!((hidden[H - 1] - expected).abs() < 1e-14);
            assert!((y[0] - 2. * expected).abs() < 1e-14);
        }
        check::<3>();
        check::<7>();
        check::<64>();
    }
}
