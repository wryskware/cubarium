---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream U (Opus): the grasp-only apex radius, paired under the shipped motor

Fable orchestrates. Item 1 of the reconciled next steps in
[the round-4 result](../7_Research/ecology-v1-round4-results-2026-09-16.md),
Astra's "single most informative cheap experiment now"
([round-4 review](../7_Research/ecology-v1-round4-review-2026-09-16.md)). You
own this brief; Fable reviews once with at most two repair cycles. Model: Opus
5, medium reasoning effort (one variable on an existing paired harness). Time
target: a short session. **Worktree.** Run under `AGENTS.md` and
`WORKING_POLICY.md`. Shared `target/`: set
`CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target`; a build failing on a
symbol the tests just used means another tree was building — `touch
crates/cubarium-core/src/lib.rs` and rebuild; pin
`CUBARIUM_SEARCH_BUILD=<your commit>` and copy the release binary out of the
shared target before running arms. First `git log --oneline -1` and `git reset
--hard <the brief commit>` if HEAD differs. `runs/` is git-ignored: read
retained rows from the main checkout and copy your outputs back there at the
end. Commit by path only; do not push, tag, restart the cube, or touch
`state/`, port 7393, the shim or the running `cubarium`. Commit messages end
with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Tests first,
from the definitions. Never add a `WorldConfig` field.

**Files you own:** `crates/cubarium-core/src/motor.rs` (the `Sweep` branch of
`turn_radius_px_in` only), `crates/cubarium-core/tests/`,
`crates/cubarium-search/src/apex_audit.rs` (one flag beside `--motor` and
`--pursuit-stop`), its CLI lines, `crates/cubarium-search/tests/`,
`design/7_Research/`, `runs/`.

## Read first

- [T's note](../7_Research/ecology-v1-motor-inertial-2026-09-16.md) as
  corrected: `MotorModel::{Sweep, Inertial}`, `turn_radius_px_in(o, apex,
  model)`, the apex's lobes are 9 px and its grasp reach 14.8 px; under
  `Sweep` the apex is told 9 px by the observation and bounded by 14.8 in the
  envelope and bill; T's own recommendation of exactly this intermediate.
  [P's note](../7_Research/ecology-v1-apex-predicate-2026-09-16.md) as
  corrected: the eight-seed reach-envelope design and its rows
  (`runs/ecology-v1-apex-predicate/reach-envelope-8.json`), E's usable-energy
  ratio. Astra's review, P2 on T and next steps item 1: what to record and
  the rule.

## Deliverables

1. **The switch:** an opt-in `World` transient (beside the motor model and
   pursuit stop; no `WorldConfig` field) under which `Sweep`'s apex radius is
   the lobe extent rather than `max(lobes, grasp)`; ordinary bodies untouched
   by construction. Default off = byte-identical (state hash over 9,000 ticks
   of a two-apex world, pinned before the code, as T did). Tests first: an
   apex's envelope radius under the switch equals its lobes; an ordinary body's
   is unchanged; the observation, envelope and bill read the same number.
2. **The pair** (≤ 3 wall minutes, 8 workers): P's eight-seed two-apex design
   with `--pursuit-stop reach-envelope`, `--motor sweep`, switch off and on.
   The off arm must reproduce P's `reach-envelope-8` rows. Report, per arm:
   prey population at introduction (must be identical between arms), delivered
   translation and rotation, signed gap closure per burst, contacts and
   captures by initial-gap bin, captures per life, E's usable-energy ratio
   (gut battery credit + 0.8·2.0 × gut reserve credit, over upkeep + motor +
   strike and handling — state the formula and use it, not a material ratio),
   lifetime, death cause, readiness terms. Put T's `Inertial` arm-A numbers
   beside them for comparison (from its retained rows, not re-run).
3. **Verdict by Astra's rule:** confirmed that geometry is the principal apex
   motor defect if the switch recovers most of `Inertial`'s −0.57 px closure
   and 2.9 captures per life in the identical prey world; refuted if it stays
   near `Sweep`'s +0.21 px and 2.1. State plainly which of the two paths this
   supports: correct the apex geometry and keep `Sweep`, or the disc model is
   needed for the apex's gain.
4. **Result note** `design/7_Research/ecology-v1-apex-grasp-2026-09-16.md`;
   tests green (`cargo test -p cubarium-core`, `-p cubarium-search`); `graft
   build`; commit on the branch. Storage `runs/ecology-v1-apex-grasp/` ≤ 20
   MiB.

Return: branch and commits, test totals, the three-column table (sweep,
grasp-only, inertial), the verdict, wall time, usage, evidence and reasoning.
