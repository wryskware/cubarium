//! The two-layer brain's **shape** (its runtime dimensions, from the lineage) and its
//! **flat parameter layout**: which values of the ES vector are the bottom layer and which
//! the top, so a stage-1 search can perturb the top alone (§10.4).
//!
//! # The flat vector
//!
//! `[ bottom GRU | top head ]`, nothing else.
//!
//! - **Bottom:** today's GRU tensors in [`crate::es::tensor`]'s order
//!   (`w_i, w_h, b_i, b_h, w_o, b_o`) over the widened input and output described on
//!   [`TwoLayerShape::bottom_inputs`] and [`TwoLayerShape::bottom_outputs`], then the
//!   **same-sample readout** `w_c` (`[O × C]`, `C` = [`TwoLayerShape::conditioning`]): the
//!   action readout's weights over the intent and subject **settled at this sample**
//!   (§11.3, amended 2026-10-12), `y = w_o h + b_o + w_c c`. For a plain shape (a one-item
//!   menu, no subjects, no discrete heads) `C = 0` and this is **exactly** today's vector,
//!   value for value.
//! - **Top** (absent on a plain shape), in this order:
//!
//! | name | shape | what |
//! | --- | --- | --- |
//! | `w1` | `[T × U]` | top hidden layer over `u = [h, held one-hot, time]`, `U = H + M' + 1` |
//! | `b1` | `[T]` | |
//! | `w2` | `[M' × T]` | intent logits, `M' = menu + spare` |
//! | `b2` | `[M']` | |
//! | `wr` | `[P × (F + 2)]` | pointer: one candidate row (features + body/place tag) |
//! | `wq` | `[P × T]` | pointer: the top's hidden layer |
//! | `e` | `[P × M']` | pointer: the proposed intent's embedding (column per intent) |
//! | `bg` | `[P]` | |
//! | `v` | `[P]` | pointer score from its hidden layer |
//! | `none` | `[M']` | the score of "no subject", per proposed intent |
//!
//! The pointer tensors (`wr` … `none`) exist only when the lineage has subject
//! candidates (`K > 0`).

use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::es::rng::{gaussian, stream};
use crate::es::tensor::{self, INIT_GAIN, ShapeOffsets, shape_offsets_hidden};

/// A continuous action's transfer, as the fauna's manifest names it (`σ` for `[0, 1]`,
/// `tanh` for `[-1, 1]`). Used only by cloning's loss; the runtime answers raw logits and
/// the fauna's own adapter transfers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionTransfer {
    Sigmoid,
    Tanh,
}

impl ActionTransfer {
    pub fn apply(self, logit: f64) -> f64 {
        if !logit.is_finite() {
            return 0.0;
        }
        match self {
            ActionTransfer::Sigmoid => 1.0 / (1.0 + (-logit).exp()),
            ActionTransfer::Tanh => logit.tanh(),
        }
    }

    /// `d transfer(y) / dy`, given the already-computed value.
    pub fn derivative(self, logit: f64, value: f64) -> f64 {
        if !logit.is_finite() {
            return 0.0;
        }
        match self {
            ActionTransfer::Sigmoid => value * (1.0 - value),
            ActionTransfer::Tanh => 1.0 - value * value,
        }
    }
}

/// A lineage's two-layer brain dimensions. Runtime configuration, not constants: the
/// adapter builds it from the lineage's manifest and its intent menu.
///
/// Architecture widths (`hidden`, `top_hidden`, `pointer_width`) are part of the policy
/// file, like today's manifest `hidden`: a file is refused under another shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwoLayerShape {
    /// Observation width, in manifest order (the manifest's `inputs()`).
    pub observation: usize,
    /// The bottom GRU's hidden width `H` (the manifest's `hidden`).
    pub hidden: usize,
    /// The continuous actions, in manifest order, with their transfers.
    pub actions: Vec<ActionTransfer>,
    /// Today's option-head rows (the manifest's `option_head`, plan item 23), carried after
    /// the actions and passed through untouched so a plain file keeps its exact outputs.
    /// The two-layer menu replaces them; no loss here trains them.
    pub legacy_options: usize,
    /// Named intents on the lineage's menu `M`. `1` is a plain brain.
    pub menu: usize,
    /// Unnamed spare intents after the named ones, always masked until the
    /// self-invented-intent study (§10.1: two).
    pub spare: usize,
    /// Subject candidate rows `K` per decision sample (`0`: no subjects).
    pub candidates: usize,
    /// Each candidate row's feature width `F`, without its type tag.
    pub row_width: usize,
    /// Whether the bottom has a kick trigger head (one logit, kick when positive).
    pub kick: bool,
    /// Call kinds, not counting "none". `0`: no call head; otherwise the head has
    /// `call_kinds + 1` logits, index 0 being silence.
    pub call_kinds: usize,
    /// Whether the bottom has a rest-request head (one logit, ask when positive).
    pub rest: bool,
    /// The top head's hidden width `T`.
    pub top_hidden: usize,
    /// The subject pointer's hidden width `P`.
    pub pointer_width: usize,
}

/// The two type tags a candidate row carries into the networks: body, place.
pub const TYPE_TAGS: usize = 2;

impl TwoLayerShape {
    /// The plain brain of a lineage: a one-item menu, no subjects, no discrete heads, no
    /// top. Its bottom is today's GRU exactly.
    pub fn plain(
        observation: usize,
        hidden: usize,
        actions: Vec<ActionTransfer>,
        legacy_options: usize,
    ) -> TwoLayerShape {
        TwoLayerShape {
            observation,
            hidden,
            actions,
            legacy_options,
            menu: 1,
            spare: 0,
            candidates: 0,
            row_width: 0,
            kick: false,
            call_kinds: 0,
            rest: false,
            top_hidden: 0,
            pointer_width: 0,
        }
    }

    /// A founder's plain shape, from its manifest.
    pub fn plain_from_manifest(manifest: &cubarium_voxel_fauna::Manifest) -> TwoLayerShape {
        let actions = manifest
            .actions
            .iter()
            .map(|a| match a.transfer {
                cubarium_voxel_fauna::Transfer::Sigmoid => ActionTransfer::Sigmoid,
                cubarium_voxel_fauna::Transfer::Tanh => ActionTransfer::Tanh,
            })
            .collect();
        TwoLayerShape::plain(
            manifest.inputs(),
            manifest.hidden,
            actions,
            manifest.option_head,
        )
    }

    /// Whether the brain has a top layer: a menu of more than one intent, or subjects.
    pub fn has_top(&self) -> bool {
        self.menu > 1 || self.candidates > 0
    }

    /// Whether this is a plain brain: no top and no discrete heads. Its flat vector and
    /// its outputs are today's GRU's.
    pub fn is_plain(&self) -> bool {
        !self.has_top() && !self.kick && self.call_kinds == 0 && !self.rest
    }

    /// `M' = menu + spare`: the width of the held-intent one-hot and of the intent logits.
    pub fn intents(&self) -> usize {
        self.menu + self.spare
    }

    /// Width of the held-intent block of the bottom's input (zero without a top: a
    /// constant intent input is a bias, and a plain file has none).
    pub fn intent_block(&self) -> usize {
        if self.has_top() { self.intents() } else { 0 }
    }

    /// Width of the subject block of the bottom's input: the held subject's row and its
    /// type tag, zeros with no subject.
    pub fn subject_block(&self) -> usize {
        if self.candidates > 0 {
            self.row_width + TYPE_TAGS
        } else {
            0
        }
    }

    /// Width of the time-in-intent block (one squashed value, with a top).
    pub fn time_block(&self) -> usize {
        usize::from(self.has_top())
    }

    /// Width `C` of the conditioning block: `[intent one-hot | subject row + tag | time in
    /// intent]`. The recurrent update reads it for the hold **before** a sample's decision;
    /// the same-sample readout reads it for the hold **after** it. Zero without a top.
    pub fn conditioning(&self) -> usize {
        self.intent_block() + self.subject_block() + self.time_block()
    }

    /// The bottom GRU's input: `[observation | held intent one-hot | subject row + tag |
    /// time in intent]`.
    pub fn bottom_inputs(&self) -> usize {
        self.observation + self.conditioning()
    }

    /// Logits of the call head (silence first), zero without one.
    pub fn call_logits(&self) -> usize {
        if self.call_kinds > 0 {
            self.call_kinds + 1
        } else {
            0
        }
    }

    /// The bottom GRU's output rows: `[actions | legacy options | kick | call | rest]`.
    pub fn bottom_outputs(&self) -> usize {
        self.actions.len()
            + self.legacy_options
            + usize::from(self.kick)
            + self.call_logits()
            + usize::from(self.rest)
    }

    /// Where each head sits in the bottom's output rows.
    pub fn heads(&self) -> BottomHeads {
        let actions = self.actions.len() + self.legacy_options;
        let kick = self.kick.then_some(actions);
        let call_start = actions + usize::from(self.kick);
        let call = (self.call_kinds > 0).then(|| call_start..call_start + self.call_logits());
        let rest = self.rest.then_some(call_start + self.call_logits());
        BottomHeads {
            continuous: 0..self.actions.len(),
            passthrough: 0..actions,
            kick,
            call,
            rest,
        }
    }

    /// The top's input width `U = H + M' + 1`.
    pub fn top_inputs(&self) -> usize {
        self.hidden + self.intents() + 1
    }

    /// Refuse an inconsistent shape by name.
    pub fn validate(&self) -> Result<(), String> {
        if self.observation == 0 || self.hidden == 0 {
            return Err("two-layer shape: observation and hidden widths must be nonzero".into());
        }
        if self.actions.is_empty() {
            return Err("two-layer shape: at least one continuous action".into());
        }
        if self.menu == 0 {
            return Err("two-layer shape: the menu has at least one intent".into());
        }
        if self.has_top() && self.top_hidden == 0 {
            return Err("two-layer shape: a brain with a top needs top_hidden > 0".into());
        }
        if self.candidates > 0 && (self.row_width == 0 || self.pointer_width == 0) {
            return Err(
                "two-layer shape: subject candidates need row_width > 0 and pointer_width > 0"
                    .into(),
            );
        }
        if !self.has_top() && self.spare > 0 {
            return Err("two-layer shape: spare intents need a top".into());
        }
        Ok(())
    }
}

/// Where each head sits in the bottom's output rows ([`TwoLayerShape::heads`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BottomHeads {
    /// The continuous actions (cloning's MSE).
    pub continuous: Range<usize>,
    /// Everything handed to the fauna's adapter as today's logits: the actions and the
    /// legacy option rows.
    pub passthrough: Range<usize>,
    pub kick: Option<usize>,
    pub call: Option<Range<usize>>,
    pub rest: Option<usize>,
}

/// Absolute offsets of the top's tensors in the flat vector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TopOffsets {
    pub w1: usize,
    pub b1: usize,
    pub w2: usize,
    pub b2: usize,
    /// The pointer tensors; all equal to `end` when the shape has no candidates.
    pub wr: usize,
    pub wq: usize,
    pub e: usize,
    pub bg: usize,
    pub v: usize,
    pub none: usize,
    pub end: usize,
}

/// Which layer of the brain a parameter belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layer {
    /// The fast bottom layer: the GRU and every head on its hidden state.
    Bottom,
    /// The slow top layer: the intent head and the subject pointer.
    Top,
}

/// The flat vector's layout for one [`TwoLayerShape`]: the ES top/bottom API (§10.4).
///
/// A stage-1 search builds its perturbation as usual and calls
/// [`TwoLayerLayout::restrict`] with [`Layer::Top`], so the bottom stays frozen at its
/// copy; stage 2 perturbs everything. The trainers are not wired here (KIN-TRAIN).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TwoLayerLayout {
    /// The bottom GRU's tensor offsets, starting at 0.
    pub bottom: ShapeOffsets,
    /// The same-sample readout `w_c` (`[O × C]`, row-major by output), right after the GRU
    /// (empty without a top). Part of the bottom layer.
    pub readout: Range<usize>,
    /// The top's tensors, when the shape has a top.
    pub top: Option<TopOffsets>,
    /// Total parameters.
    pub params: usize,
}

impl TwoLayerLayout {
    pub fn new(shape: &TwoLayerShape) -> TwoLayerLayout {
        let bottom =
            shape_offsets_hidden(shape.bottom_inputs(), shape.bottom_outputs(), shape.hidden);
        let readout = bottom.params..bottom.params + shape.bottom_outputs() * shape.conditioning();
        if !shape.has_top() {
            return TwoLayerLayout {
                bottom,
                readout: readout.clone(),
                top: None,
                params: readout.end,
            };
        }
        let (t, u, m) = (shape.top_hidden, shape.top_inputs(), shape.intents());
        let w1 = readout.end;
        let b1 = w1 + t * u;
        let w2 = b1 + t;
        let b2 = w2 + m * t;
        let wr = b2 + m;
        let (p, f) = (shape.pointer_width, shape.row_width + TYPE_TAGS);
        let (wq, e, bg, v, none, end) = if shape.candidates > 0 {
            let wq = wr + p * f;
            let e = wq + p * t;
            let bg = e + p * m;
            let v = bg + p;
            let none = v + p;
            (wq, e, bg, v, none, none + m)
        } else {
            (wr, wr, wr, wr, wr, wr)
        };
        let top = TopOffsets {
            w1,
            b1,
            w2,
            b2,
            wr,
            wq,
            e,
            bg,
            v,
            none,
            end,
        };
        TwoLayerLayout {
            bottom,
            readout,
            top: Some(top),
            params: end,
        }
    }

    /// The index range a layer occupies (empty for the top of a plain brain).
    pub fn range(&self, layer: Layer) -> Range<usize> {
        match layer {
            Layer::Bottom => 0..self.readout.end,
            Layer::Top => self.readout.end..self.params,
        }
    }

    /// `true` at every index of `layer`.
    pub fn mask(&self, layer: Layer) -> Vec<bool> {
        let r = self.range(layer);
        (0..self.params).map(|i| r.contains(&i)).collect()
    }

    /// Zero every value of `v` (a perturbation, a gradient) outside `layer`.
    pub fn restrict(&self, layer: Layer, v: &mut [f64]) {
        assert_eq!(v.len(), self.params, "vector does not match the layout");
        let r = self.range(layer);
        for (i, x) in v.iter_mut().enumerate() {
            if !r.contains(&i) {
                *x = 0.0;
            }
        }
    }

    /// Every named tensor with its layer and range, in vector order (for reports and
    /// per-tensor step sizes).
    pub fn named_ranges(&self) -> Vec<(&'static str, Layer, Range<usize>)> {
        let b = &self.bottom;
        let mut out = vec![
            ("bottom.w_i", Layer::Bottom, b.w_i..b.w_h),
            ("bottom.w_h", Layer::Bottom, b.w_h..b.b_i),
            ("bottom.b_i", Layer::Bottom, b.b_i..b.b_h),
            ("bottom.b_h", Layer::Bottom, b.b_h..b.w_o),
            ("bottom.w_o", Layer::Bottom, b.w_o..b.b_o),
            ("bottom.b_o", Layer::Bottom, b.b_o..b.params),
        ];
        if !self.readout.is_empty() {
            out.push(("bottom.readout", Layer::Bottom, self.readout.clone()));
        }
        if let Some(t) = &self.top {
            for (name, r) in [
                ("top.w1", t.w1..t.b1),
                ("top.b1", t.b1..t.w2),
                ("top.w2", t.w2..t.b2),
                ("top.b2", t.b2..t.wr),
                ("top.pointer.wr", t.wr..t.wq),
                ("top.pointer.wq", t.wq..t.e),
                ("top.pointer.e", t.e..t.bg),
                ("top.pointer.bg", t.bg..t.v),
                ("top.pointer.v", t.v..t.none),
                ("top.pointer.none", t.none..t.end),
            ] {
                if !r.is_empty() {
                    out.push((name, Layer::Top, r));
                }
            }
        }
        out
    }
}

/// Init sub-stream keys for the top's matrices, past the GRU's own (0, 1, 2).
const TOP_INIT_KEYS: [u64; 5] = [16, 17, 18, 19, 20];

/// A seeded starting vector for `shape`. The bottom is
/// [`tensor::initial_center_runtime`] over the widened input and output, so a plain
/// shape starts exactly where today's GRU does; the same-sample readout starts at zero (the
/// settled hold adds nothing until a fit or a search gives it weight); the top's matrices
/// are fan-in-scaled Gaussians on their own sub-streams, its biases zero.
pub fn initial_center(shape: &TwoLayerShape, seed: u64) -> Vec<f64> {
    let layout = TwoLayerLayout::new(shape);
    let mut theta = tensor::initial_center_runtime(
        shape.bottom_inputs(),
        shape.bottom_outputs(),
        shape.hidden,
        seed,
    );
    theta.resize(layout.params, 0.0);
    if let Some(t) = layout.top {
        let fill = |theta: &mut [f64], r: Range<usize>, fan_in: usize, key: u64| {
            let s = INIT_GAIN / (fan_in.max(1) as f64).sqrt();
            for (i, x) in theta[r].iter_mut().enumerate() {
                *x = s * gaussian(seed, stream::ES_INIT, key, i as u64);
            }
        };
        fill(&mut theta, t.w1..t.b1, shape.top_inputs(), TOP_INIT_KEYS[0]);
        fill(&mut theta, t.w2..t.b2, shape.top_hidden, TOP_INIT_KEYS[1]);
        if shape.candidates > 0 {
            fill(
                &mut theta,
                t.wr..t.wq,
                shape.row_width + TYPE_TAGS,
                TOP_INIT_KEYS[2],
            );
            fill(&mut theta, t.wq..t.e, shape.top_hidden, TOP_INIT_KEYS[3]);
            fill(&mut theta, t.v..t.none, shape.pointer_width, TOP_INIT_KEYS[4]);
        }
    }
    theta
}
