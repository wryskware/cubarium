//! **The adapter** (TWO-LAYER half 2, step 2): a [`TwoLayerBrain`] behind the fauna's
//! menu seam, [`Controller::decide`] over [`DecisionInput`] (CONTRACTS, fauna schema 81),
//! and its memory in the fauna's save as [`BrainMemory`].
//!
//! The mapping, one sample at a time:
//!
//! - **Intents.** The fauna's menu is the named intents then the spares
//!   ([`cubarium_voxel_fauna::Menu`]); the core's shape is `menu` named + `spare`. The
//!   core's intent mask is the named part of the fauna's (its spares are always masked).
//! - **Rows.** All `K` rows go to the core in the fauna's order, so a core row index *is*
//!   the fauna's row: a body row as a body, a place row as a place, an empty row as a row
//!   valid for no intent (the pointer never scores it and no key matches it). Each row's
//!   validity per named intent is the fauna's `candidate_mask` row; its key is the fauna's
//!   `keys` entry ([`cubarium_voxel_fauna::subjects::subject_key`]), so the core's
//!   key-held subject finds its row however the rows reorder (§11.1).
//! - **`held_row`.** The fauna re-marks the held subject to its current row each sample.
//!   Where that row's key is not the one the brain holds (a place re-keyed, a remembered
//!   row), the brain's held subject is re-marked to it before the sample; where the fauna
//!   has no row for it, the core's own lookup finds none and ends the hold.
//! - **`early_end`.** Subject dead or lost → the core's `SubjectLost`; masked →
//!   `IntentMasked`; an emergency → `Emergency`.
//! - **The answer.** Today's logits as [`Response::Logits`]; the [`Decision`] carries the
//!   discrete heads (kick, call kind with 0 silence, rest), the held intent and subject row
//!   after the brain's own hold, and the proposal's probabilities.
//! - **Memory.** [`BrainState`] as JSON bytes under [`BRAIN_MEMORY_TAG`]; the fauna saves
//!   it with the world and hands it back before the body's next sample.

use cubarium_voxel_fauna::subjects::RowKind;
use cubarium_voxel_fauna::{BrainMemory, Controller, Decision, DecisionInput, Manifest, Response};

use super::arbitration::EarlyEnd;
use super::brain::{BrainState, CandidateRow, DecisionSample, SubjectRef, TwoLayerBrain};
use super::net::SubjectKind;
use super::shape::TwoLayerShape;

/// The [`BrainMemory::tag`] a two-layer brain writes: "TL" and the policy layout version.
pub const BRAIN_MEMORY_TAG: u32 = 0x544C_0002;

/// The discrete heads a lineage's bottom carries, and its top's widths: what a menu shape
/// adds to the lineage's manifest. Architecture, carried in the policy file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuHeads {
    pub kick: bool,
    /// Call kinds, not counting silence.
    pub call_kinds: usize,
    pub rest: bool,
    pub top_hidden: usize,
    pub pointer_width: usize,
}

impl TwoLayerShape {
    /// A menu lineage's shape: the manifest's bottom (observation, hidden, actions, legacy
    /// options), its menu (named + spare), its `K` candidate rows of width `F`, and `heads`.
    /// A plain lineage's manifest gives its plain shape whatever `heads` says.
    pub fn menu_from_manifest(manifest: &Manifest, heads: MenuHeads) -> TwoLayerShape {
        let plain = TwoLayerShape::plain_from_manifest(manifest);
        if manifest.menu.plain() {
            return plain;
        }
        TwoLayerShape {
            menu: manifest.menu.named.len(),
            spare: manifest.menu.spare,
            candidates: manifest.candidates(),
            row_width: manifest.row_width(),
            kick: heads.kick,
            call_kinds: heads.call_kinds,
            rest: heads.rest,
            top_hidden: heads.top_hidden,
            pointer_width: if manifest.candidates() > 0 { heads.pointer_width } else { 0 },
            ..plain
        }
    }
}

fn early_end(e: cubarium_voxel_fauna::EarlyEnd) -> EarlyEnd {
    match e {
        cubarium_voxel_fauna::EarlyEnd::SubjectDead | cubarium_voxel_fauna::EarlyEnd::SubjectLost => {
            EarlyEnd::SubjectLost
        }
        cubarium_voxel_fauna::EarlyEnd::Masked => EarlyEnd::IntentMasked,
        cubarium_voxel_fauna::EarlyEnd::Emergency => EarlyEnd::Emergency,
    }
}

fn subject_kind(kind: RowKind) -> SubjectKind {
    match kind {
        RowKind::Place => SubjectKind::Place,
        RowKind::Body | RowKind::Empty => SubjectKind::Body,
    }
}

impl TwoLayerBrain {
    /// One menu decision sample, as the fauna gives it ([`Controller::decide`]'s body).
    /// Panics on a sample whose dimensions are not this brain's shape: a file is checked
    /// against its lineage's manifest when it loads, so a mismatch is a wiring error.
    pub fn decide_fauna(&mut self, observation: &[f64], input: &DecisionInput<'_>) -> (Response, Decision) {
        let s = self.net().shape().clone();
        assert_eq!(input.menu, s.intents(), "the fauna's menu is this brain's named + spare");
        assert_eq!(input.rows(), s.candidates, "the fauna's rows are this brain's K");
        assert!(s.candidates == 0 || input.row_width == s.row_width, "row width");
        let menu = input.menu;
        let none = vec![false; s.menu];
        let rows: Vec<CandidateRow<'_>> = (0..input.rows())
            .map(|r| CandidateRow {
                key: input.keys[r],
                kind: subject_kind(input.kinds[r]),
                features: &input.candidates[r * input.row_width..(r + 1) * input.row_width],
                valid_for: if input.kinds[r] == RowKind::Empty {
                    &none
                } else {
                    &input.candidate_mask[r * menu..r * menu + s.menu]
                },
            })
            .collect();
        // The fauna's re-marked row for the held subject.
        if let (Some(r), Some(held)) = (input.held_row, self.held_subject())
            && let Some(row) = rows.get(r)
            && input.kinds[r] != RowKind::Empty
            && (row.key != held.key || row.kind != held.kind)
        {
            self.remark_subject(SubjectRef {
                key: row.key,
                kind: row.kind,
            });
        }
        let answer = self.step(&DecisionSample {
            observation,
            intent_valid: &input.intent_mask[..s.menu],
            candidates: &rows,
            early_end: input.early_end.map(early_end),
        });
        let mut proposal = [0.0; cubarium_voxel_fauna::intent::MAX_MENU];
        let n = answer.intent_probs.len().min(proposal.len());
        proposal[..n].copy_from_slice(&answer.intent_probs[..n]);
        let decision = Decision {
            kick: answer.kick.unwrap_or(false),
            call: answer.call.flatten().map_or(0, |k| u8::try_from(k + 1).unwrap_or(u8::MAX)),
            rest: answer.rest.unwrap_or(false),
            intent: answer.intent,
            subject: answer.subject,
            proposal,
        };
        (Response::logits(&answer.action_logits), decision)
    }

    /// The brain's memory as the fauna saves it.
    pub fn memory_record(&self) -> BrainMemory {
        BrainMemory {
            tag: BRAIN_MEMORY_TAG,
            bytes: serde_json::to_vec(&self.state()).expect("a brain state serialises"),
        }
    }

    /// Restore from a saved [`BrainMemory`]: `Err` by name for another kind of brain or a
    /// state of another shape (the brain then keeps a fresh state).
    pub fn restore_record(&mut self, memory: &BrainMemory) -> Result<(), String> {
        if memory.tag != BRAIN_MEMORY_TAG {
            return Err(format!(
                "brain memory tag {:#010x} is not a two-layer brain's {BRAIN_MEMORY_TAG:#010x}",
                memory.tag
            ));
        }
        let state: BrainState = serde_json::from_slice(&memory.bytes)
            .map_err(|e| format!("two-layer brain memory: {e}"))?;
        self.restore_state(state)
    }
}

impl Controller for TwoLayerBrain {
    /// The observation-only seam: every named intent valid, no candidates, no early end.
    /// For a plain brain this is today's GRU step exactly; a menu lineage's body is driven
    /// through [`Controller::decide`] instead.
    fn drive(&mut self, observation: &[f64]) -> Response {
        self.drive_plain(observation)
    }

    fn reset(&mut self) {
        self.reset_state();
    }

    fn decide(&mut self, observation: &[f64], input: &DecisionInput<'_>) -> (Response, Option<Decision>) {
        if !self.net().shape().has_top() {
            return (self.drive_plain(observation), None);
        }
        let (r, d) = self.decide_fauna(observation, input);
        (r, Some(d))
    }

    fn memory(&self) -> Option<BrainMemory> {
        Some(self.memory_record())
    }

    fn restore(&mut self, memory: &BrainMemory) -> bool {
        match self.restore_record(memory) {
            Ok(()) => true,
            Err(_) => {
                self.reset_state();
                false
            }
        }
    }
}
