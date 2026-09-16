---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream T (Opus): an inertial motor model, paired against the shipped sweep model

Fable orchestrates, on Wrysk's direction (2026-09-16): "make movement expense
cost like actual physics would require, energy based on mass and momentum …
if we're just doing an approximation, use a mean radius / sublinear rotation
… we don't need to model 'are claws outstretched' when turning; add their
mass in, model everything as balls or cylinders and use rough heuristics."
Context: [P's result](../7_Research/ecology-v1-apex-predicate-2026-09-16.md)
(64 % of the apex's boosted budget is turn sweep priced at a 14.8 px radius
that is its grasp reach, not its body) and the motor contract in
`crates/cubarium-core/src/motor.rs` (`turn_radius_px`, `ROTATION_COST_SCALE`,
`MotorLimits`, `billed_motion`, `total_cost`). You own this brief; Fable
reviews once with at most two repair cycles. Model: Opus 5, high reasoning
effort (the motor contract is load-bearing for every body and for the neural
action adapter). Time target: one session. **Worktree.** Run under
`AGENTS.md` and `WORKING_POLICY.md`. Shared `target/`: set
`CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target`; a build failing on a
symbol the tests just used means another tree was building — `touch
crates/cubarium-core/src/lib.rs` and rebuild; pin
`CUBARIUM_SEARCH_BUILD=<your commit>` in recorded stamps. First `git log
--oneline -1` and `git reset --hard <the brief commit>` if HEAD differs.
`runs/` is git-ignored: read retained rows from the main checkout and copy
your outputs back there at the end. Commit by path only; leave `.claude/*`,
`WORKING_POLICY.md`, `.agents/` and other uncommitted design documents alone.
Do not push, tag, restart the cube, or touch `state/`, port 7393, the shim or
the running `cubarium`. Commit messages end with
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Tests are their
own pass, written from the definitions before the implementation. Ask no
questions; record routine choices in the note. **Never add a `WorldConfig`
field** (it changes `calibrate::config_hash` for every retained row).

**Files you own:** `crates/cubarium-core/src/motor.rs`, the motor-resolution
region of `crates/cubarium-core/src/world/step.rs` (where `MotorLimits`,
`turn_radius_px` and the bill are applied; another worker owns `lifecycle.rs`
and the plant region; the hunt-intent predicate region is settled by P — read
it, do not edit it), `crates/cubarium-core/src/neural/action.rs` **read-only**
(the adapter's envelope must keep working under both models; if it cannot,
say so and stop), `crates/cubarium-core/tests/`,
`crates/cubarium-search/src/apex_audit.rs` (a `--motor` flag beside
`--pursuit-stop`), `crates/cubarium-search/src/calibrate.rs` (a `--motor`
axis, add only), their CLI lines, `crates/cubarium-search/tests/`,
`design/7_Research/`, `runs/`.

## The model (Fable's design call; state its visible effect in the note)

- **Every organism is a uniform disc** of mass ∝ `structure` and radius
  `r = phenotype.extent` (its own lobes). **The apex uses the same rule**: its
  grasp (`capture_offset + capture_reach`) is contact geometry only and no
  longer enters `turn_radius_px` for the envelope or the bill.
- **Rotation is an energy-equivalent speed**: a disc at angular speed ω has
  the kinetic energy of translation at `v_rot = r·ω/√2` (radius of gyration
  of a disc). This replaces the outer-point sweep `r·ω` and the separate
  `ROTATION_COST_SCALE` in both the bill and the envelope.
- **The envelope combines as energies**: `sqrt(v² + v_rot²) ≤ speed_cap`
  (the shipped model is `|v| + r·|ω| ≤ cap`). Angular-rate ceilings and the
  capability that scales the cap (wading, burst) are unchanged.
- **The bill** is `move_cost · S · (|v| + v_rot) · dt` per tick — same units
  and same `move_cost` as today, so a body that only translates pays exactly
  the shipped bill (keep that bit-for-bit, and test it). Mass enters through
  `S` as it does now.
- **Not in this pass**: a cost of acceleration (momentum). Name it as the next
  step; do not implement it.

## Deliverables

1. **The switch**, `MotorModel::{Sweep, Inertial}` at `World` level (a
   transient, not persisted, not in `WorldConfig`; default `Sweep` =
   byte-identical to today by state hash over 9,000 ticks with animals and an
   apex). `Inertial` selectable from `apex-audit --motor inertial` and
   `calibrate --motor inertial`. Tests first: pure translation bills
   identically under both; a pure rotation at the same ω bills `r/√2` of the
   shipped sweep for an ordinary body and far less for an apex (its grasp no
   longer counts); the envelope admits `v = v_rot = cap/√2` under `Inertial`
   and refuses it under `Sweep`; the neural adapter's `Envelope` and
   `requested_speed` / `omega_attain` produce finite, in-envelope motion under
   both. **Provenance:** the ES `Protocol` and `PolicyFile` must record the
   motor model (a policy trained under one is refused by name under the
   other), and `es-train`/`es-evaluate` gain `--motor` — read
   `crates/cubarium-search/src/es/{fixture,export,commands}.rs` and add the
   field the way the ecology hash was added; do not train.
2. **Paired arm A, the apex** (≤ 3 wall minutes): P's eight-seed two-apex
   design with `--pursuit-stop reach-envelope` under `Sweep` and `Inertial`;
   report P's whole table (held fraction, delivered burst translation / rotation
   / total, gap closed, contacts, captures per life, earned fraction of bill,
   lifetime, death cause, prey population) and the motor share of the apex's
   ledger.
3. **Paired arm B, the whole world** (≤ 8 wall minutes): A's screen control
   rows — `baseline` and `fast-leaf`, the 6 training seeds, arms 0/1/2, the
   shipped price — under both models, ledger on. The `Sweep` rows must
   reproduce the retained hashes (`runs/ecology-v1-calibration/screen/evals.jsonl`,
   `runs/ecology-v1-ladder/ladder/evals.jsonl` arm 0). Report per config and
   arm: population and kinds alive, births and deaths by cause, foliage
   retention, litter, motor share of the bill by form, range (cells per body
   per window), residence and revisit (F's measures), the depletion counter's
   two branches, and the six gates. Compare with
   `design/7_Research/r0a-motor-cost-ecology-2026-09-14.md` (rotation price
   moved the legacy population 93 → 39 under the old envelope): say whether
   the legacy controller's turning habit changes the world materially under
   `Inertial`.
4. **Verdict:** does `Inertial` (a) let the corrected apex close and capture
   more, and at what earned fraction of its bill; (b) change the whole world's
   populations, variety, foliage or range beyond seed noise (state the seed
   spread); (c) keep every gate. State plainly what Wrysk would be adopting if
   `Inertial` became the contract: the motor contract's rotation term and
   envelope for every body, visible on the cube as bodies that turn more
   freely and an apex that can charge; a new protocol hash for training; and
   which calibration rows would need re-running.
5. **Result note** `design/7_Research/ecology-v1-motor-inertial-2026-09-16.md`
   with the model stated as above, the tests, both arms' tables, the verdict,
   the acceleration-cost next step named, what this does not establish; tests
   green (`cargo test -p cubarium-core`, `-p cubarium-search`, and the host's
   `cargo test -p cubarium --release --test run_neural_seed`); `graft build`;
   commit on the branch. Storage `runs/ecology-v1-motor-inertial/` ≤ 40 MiB.

Return: branch and commits, test totals, both tables in compact form, the
verdict with what Wrysk would be adopting, wall time, usage, evidence and
reasoning behind each decision.
