---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream XY2 (Opus): the two cheap follow-ups X and Y named

Fable orchestrates. Items 1 and 2 of the next recommendation in
[the round-5 result](../7_Research/ecology-v1-round5-results-2026-09-16.md):
each is the one measurement its parent workstream said would decide its
open question, each is minutes of simulation, and neither changes a default.
You own this brief; Fable reviews once with at most two repair cycles.
Model: Opus 5, medium reasoning effort (two existing harnesses, re-run with
one argument moved each). Time target: a short session. **Worktree.** Run
under `AGENTS.md` and `WORKING_POLICY.md`. Shared `target/`: set
`CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target` and run `touch
crates/cubarium-core/src/lib.rs crates/cubarium-search/src/lib.rs
crates/cubarium-search/src/main.rs` immediately before **every** cargo
invocation (another tree builds concurrently); pin
`CUBARIUM_SEARCH_BUILD=<your commit>` and copy the release binary out of the
shared target before running rows. **Never run `cargo fmt`.** First `git log
--oneline -1`; if HEAD's subject does not begin "Ecology v1 round 5: brief
XY2", run `git reset --hard main`. `runs/` is git-ignored: read retained rows
from the main checkout and copy your outputs back there at the end. Commit
by path only; do not push, tag, restart the cube, or touch `state/`, port
7393, the shim or the running `cubarium`. Commit messages end with
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Pre-register
each reading rule before its rows exist. Never add a `WorldConfig` field.

**Files you own:** `crates/cubarium-search/src/census.rs` (only if the
arm-2 ladder needs an argument it lacks), `crates/cubarium-search/src/es/turnband.rs`
(only if the comparison needs a column it lacks), the `Census` and
`EsTurnBand` command lines in `main.rs`, new files under
`crates/cubarium-search/tests/`, `design/7_Research/`, `runs/`. **Do not
touch** `cubarium-core`, `precondition.rs`, `lifecycle.rs`, `evaluate.rs`,
`calibrate.rs`, `factorial.rs`, `apex_audit.rs`, `population.rs`, the ES
trainer, protocol, export or fixture modules, or other commands in `main.rs`
(workstream Z is in `precondition.rs`/`lifecycle.rs` concurrently).

## Part 1 — X's second training seed, with its own control

Read [X's note](ecology-v1-turn-deadband-2026-09-16.md) §"Deliverable 3"
and §"What this does not establish", and its exact commands. X trained one
`cub-act-2` arm at `--train-seed 20260915` against the retained `cub-act-1`
run at the same seed and found the aggregate columns up (15/16 generations)
with held-out residence flat; X itself says a second seed is the cheapest
thing that separates "this adapter is better" from "this seed was luckier".

1. **Pre-register** the seed (`--train-seed 20260916`) and the rule: the
   adapter effect is *replicated* if the paired-by-generation sign tests on
   mean population score and mean producer intake per lived tick again
   favour `cub-act-2` at p < 0.05 **and** the held-out minimum is higher;
   *not replicated* if either aggregate column fails to separate; *mixed*
   otherwise. State that held-out opening residence is reported, not part
   of the rule, because X found it flat under the first seed.
2. **Run both arms at the new seed**: the retained command verbatim with
   `--train-seed 20260916`, once with `--adapter cub-act-1` (`--out
   runs/es-eco-v1-fastleaf-s2`) and once with `--adapter cub-act-2`
   (`--out runs/es-eco-v1-fastleaf-act2-s2`). 16 pairs, 16 generations,
   horizon 36,000, 8 workers, 20-minute cap each, no retry or tuning. Export
   and evaluate the selected centre of each on the held-out layouts as X
   did.
3. **Report** X's training table and held-out table for the second seed
   beside the first, the rule's verdict, and the seed-by-seed picture (two
   seeds × two adapters). Do not propose the training default; say what
   the two seeds together support.

## Part 2 — Y's ladder at arm 2

Read [Y's note](ecology-v1-depth-ladder-2026-09-16.md) §"What this campaign
found that it was not asked to" and §"The next task this implies". Y ran
the six-rung ladder at arm 0 and found no rung acceptable; re-reading R's
retained rows, R's `fast-leaf` result (grazer 0.47× at 0.55, lineage 5/6)
lives in arms 1 and 2, not arm 0 (0.90×, 3/6).

1. **Pre-register**: the same design (six rungs × two configurations × six
   training seeds, R's horizon, sampling, budgets, census and Y's two added
   measures), **arm 2 only** (two apex adults introduced at R's tick), 72
   runs; R's arm-2 rows at 0.10 and 0.55 are the reproduction targets
   (state hash included). **Note that arm 2 has predators, so it runs under
   the shipped reach-envelope predicate (schema 17) while R's rows ran the
   half-space: reproduce R's arm-2 rows with `--pursuit-stop half-space`
   (add the flag to `census` if it lacks one, defaulting to the shipped
   rule as every other command does), and run the ladder itself under the
   shipped rule.** State both in the pre-registration. Y's acceptance rule
   unchanged (L ∧ ¬M ∧ ¬V, 5 of 6 seeds binds).
2. **Run** (≤ 6 wall minutes, 8 workers) and **report** Y's per-rung table
   for both configurations at arm 2, with Y's arm-0 table beside it, the
   per-seed picture, the served-by-channel and generation-split measures,
   and the verdict per rung. Do not propose a roster change.

## Result note

`design/7_Research/ecology-v1-round5-followups-2026-09-16.md` with both
parts, their pre-registrations, reproductions, tables, verdicts, and the
routine decisions you made; tests green (`cargo test -p cubarium-search`);
`graft build`; commit on the branch. Storage: Part 1 under
`runs/es-eco-v1-fastleaf-s2/` and `runs/es-eco-v1-fastleaf-act2-s2/`
(≤ 12 MiB together); Part 2 under `runs/ecology-v1-depth-ladder-arm2/`
(≤ 30 MiB).

Return: branch and commits (pre-registrations first), test totals, the
reproductions, both parts' tables and verdicts, wall time, usage, evidence
and reasoning.
