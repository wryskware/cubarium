---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream V (Opus): adopt the reach-envelope pursuit predicate as the shipped rule

Fable orchestrates. Item 2 of the reconciled next steps in
[the round-4 result](../7_Research/ecology-v1-round4-results-2026-09-16.md):
Astra's order is "adopt the reach-envelope predicate **separately**, with a
production default, a resume regression and `check_motor` in host seeding".
Wrysk's standing rules apply: always fresh, never migrate; old policies need
not reload, they are refused by name. You own this brief; Fable reviews once
with at most two repair cycles. Model: Opus 5, **high** reasoning effort (this
is a contract change across three crates, not an experiment). Time target: a
short session. **Worktree.** Run under `AGENTS.md` and `WORKING_POLICY.md`.
Shared `target/`: set `CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target`;
a build failing on a symbol the tests just used means another tree was
building — `touch crates/cubarium-core/src/lib.rs` and rebuild; pin
`CUBARIUM_SEARCH_BUILD=<your commit>` and copy the release binary out of the
shared target before running anything. First `git log --oneline -1` and `git
reset --hard <the brief commit>` if HEAD differs. `runs/` is git-ignored: read
retained rows from the main checkout and copy your outputs back there at the
end. Commit by path only; do not push, tag, restart the cube, or touch
`state/`, port 7393, the shim or the running `cubarium`. Commit messages end
with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Tests first,
from the definitions below. Never add a `WorldConfig` field.

**Files you own:** `crates/cubarium-core/src/hunter/geometry.rs`,
`crates/cubarium-core/src/hunter/strike.rs`,
`crates/cubarium-core/src/world/hunter.rs`, the comments in
`crates/cubarium-core/src/world/step.rs` that name the shipped rule,
`crates/cubarium-core/src/snapshot.rs`, `crates/cubarium-core/src/world/state.rs`
docs, new files under `crates/cubarium-core/tests/`,
`crates/cubarium-search/src/{calibrate.rs,evaluate.rs,main.rs}`,
`crates/cubarium-search/src/es/export.rs` docs only,
`crates/cubarium-search/tests/`, `crates/cubarium/src/runner/mod.rs`,
`crates/cubarium/tests/run_neural_seed.rs`,
`crates/cubarium/tests/run_persistence.rs`, `design/7_Research/`,
`design/backlog.md` §1 (one row), `runs/`. **Do not touch**
`crates/cubarium-core/src/motor.rs` or `crates/cubarium-search/src/apex_audit.rs`
(workstream U is in them concurrently); Fable reconciles their wording at
integration.

## Read first

- [P's note](../7_Research/ecology-v1-apex-predicate-2026-09-16.md) as
  corrected: what the two rules are, where the one predicate is written
  (`ContactMeasure::pursuit_holds`), the paired result (held at burst start
  89.4 % → 5.4 %, contacts 88 → 140, captures 38 → 67, ledger 11.1 % → 18.5 %,
  apex still starves 32/32). [T's note](../7_Research/ecology-v1-motor-inertial-2026-09-16.md)
  for the pattern a transient contract follows (`MotorModel`, `check_motor`,
  `Protocol.motor`, `StagePlan.motor`, `RunOptions.motor`, missing = shipped).
- `crates/cubarium-core/src/snapshot.rs`: `SCHEMA_VERSION = 16`, the header
  format, `decode_exact`, and how schema 16 refuses 7–15 by name. The
  [always-fresh rule](../7_Research/ecology-v1-next-results-2026-09-15.md)
  section on schema 16.
- `crates/cubarium/src/runner/mod.rs::seed_neural_animals` and
  `crates/cubarium/tests/run_neural_seed.rs` (seven tests; the ecology check
  is the model for the motor check).

## Deliverables

1. **The shipped rule becomes the reach envelope.** `PursuitStop::default()`
   is `ReachEnvelope`; `StrikeRecorder` follows by construction; an ordinary
   `World` runs it without being told. `StrikeRecord.stop`'s serde default
   stays `ForwardHalfSpace` by an explicit named function, because every
   record written before the field existed ran under that rule; say so in its
   doc. `ForwardHalfSpace` stays available as the opt-in variant
   (`World::set_pursuit_stop`), so the retained rows remain reproducible.
   Rewrite the doc comments on `PursuitStop`, `world/hunter.rs`, the step.rs
   hunt-intent comments and `parse_pursuit_stop`'s CLI help so that "shipped"
   names the envelope and "the rule before 2026-09-16" names the half-space.
2. **Schema 17.** A world's saved bytes do not carry the rule, so a schema-16
   snapshot resumed under this build would silently change behaviour: that is
   a migration by another name, and the standing rule forbids it.
   `SCHEMA_VERSION` becomes 17 with **no payload change**; schema 16 is
   refused by number and by name like 7–15 (the shape check cannot tell 16
   from 17, so the header must). Extend the schema doc block: 17 is the first
   bump for a semantics-only change and says which. Tests: a schema-16 header
   over a valid payload is refused with the schema named; a fresh schema-17
   world reports `pursuit_stop() == ReachEnvelope`; a world saved and resumed
   under 17 runs the envelope on both sides and the resumed hash equals the
   uninterrupted hash over ≥ 2,000 ticks of a two-apex world (that is the
   resume regression); a world told `ForwardHalfSpace` before its first tick
   reproduces the pre-brief default's hash over 9,000 ticks of the same
   two-apex world (pin the hash from the brief commit **before** flipping the
   default, as T did). `run_persistence.rs` in the host: the cube's
   `--resume` path refuses a schema-16 file by name and `--fresh` starts 17.
3. **Provenance.** `evaluate::RunOptions` gains `pursuit_stop:
   cubarium_core::hunter::PursuitStop` (default = the shipped rule), set on the
   world before the first tick beside `motor`; `calibrate::StagePlan` gains
   `pursuit_stop: String` beside `motor` (missing on a retained row =
   `forward_half_space`, and the reader says so); `calibrate`, `precondition`,
   `factorial` and `es-evaluate`/`es-population` get `--pursuit-stop
   {half-space,reach-envelope}` with the same help text pattern as `--motor`.
   The ES `Protocol`/`PolicyFile` do **not** gain a field: check
   `es/fixture.rs` and `es/episode.rs`, confirm no training layout founds a
   hunter, and write that finding and its consequence (policies are
   predicate-independent) in the note; if you find a layout that does, stop
   and say so rather than adding the field.
4. **Host contract for the motor.** `seed_neural_animals` calls
   `file.check_motor(world.motor_model())` after `check_ecology`, refused by
   name. `run_neural_seed.rs` gains: a policy file naming `inertial` is
   refused with the message naming both contracts; a file with no `motor`
   field seeds (missing = sweep). The host never sets a motor, so this is the
   contract existing, not a behaviour change; say so in the note.
5. **What the cube will show.** Re-run the selected `fast-leaf` and
   `baseline` calibration rows (`runs/ecology-v1-calibration/selected/*.json`
   name their stage, seed and arm; the [calibration note](../7_Research/ecology-v1-calibration-2026-09-15.md)
   and its errata say how a row is re-run) under the new default at the
   retained length, and once more with `--pursuit-stop half-space`. The
   half-space re-run must match the retained row's `final_state_hash`; that is
   the proof that the flip is the only change. Report the reach-envelope rows
   beside the retained ones: population trajectory, plant totals, apex
   lifetimes, captures and death causes. This is a **description**, not a
   recalibration: do not select, do not tune. ≤ 10 wall minutes, 8 workers.
6. **Result note** `design/7_Research/ecology-v1-predicate-adoption-2026-09-16.md`
   with a "what changed for an operator" paragraph in Wrysk's terms (the apex
   now bursts when it pays for a burst; nothing else moved; the cube must be
   started fresh); one row in `design/backlog.md` §1 for `--pursuit-stop`;
   tests green (`cargo test -p cubarium-core`, `-p cubarium-search`, `-p
   cubarium`); `graft build`; commit on the branch.

Return: branch and commits, test totals per crate, the pinned hashes (before
and after, half-space and envelope), the fast-leaf/baseline comparison table,
the hunter-in-layouts finding, wall time, usage, evidence and reasoning.
