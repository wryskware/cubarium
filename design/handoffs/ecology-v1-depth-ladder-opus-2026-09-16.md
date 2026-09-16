---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream Y (Opus): the skimmer depth ladder, on R's census, arm 0

Fable orchestrates. Item 5 of the reconciled next steps in
[the round-4 result](../7_Research/ecology-v1-round4-results-2026-09-16.md)
and the next task [R](../7_Research/ecology-v1-depth-census-2026-09-16.md)
named: *a depth ladder between the rim and the grazer, on this same census.*
Astra's rule, which you pre-register: *a depth is acceptable only if a lineage
persists across seeds without materially reducing the grazer; if none exists,
the choice becomes three viable heights and four kinds, or a wet-floor
producer, which is a new food web and not a repair.* **No roster change is
proposed by this workstream**; it measures. You own this brief; Fable reviews
once with at most two repair cycles. Model: Opus 5, medium reasoning effort
(one more level on a harness that already exists, with two small measures
added). Time target: a short session. **Worktree.** Run under `AGENTS.md` and
`WORKING_POLICY.md`. Shared `target/`: set
`CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target` and run `touch
crates/cubarium-core/src/lib.rs crates/cubarium-search/src/lib.rs
crates/cubarium-search/src/main.rs` immediately before **every** cargo
invocation (another tree builds concurrently); pin
`CUBARIUM_SEARCH_BUILD=<your commit>` and copy the release binary out of the
shared target before running rows. **Never run `cargo fmt`** in this
repository. First `git log --oneline -1`; if HEAD's subject does not begin
"Ecology v1 round 5: brief Y", run `git reset --hard main`. `runs/` is
git-ignored: read retained rows from the main checkout and copy your outputs
back there at the end. Commit by path only; do not push, tag, restart the
cube, or touch `state/`, port 7393, the shim or the running `cubarium`.
Commit messages end with `Co-Authored-By: Claude Fable 5.1
<noreply@anthropic.com>`. Tests first, from the definitions. Never add a
`WorldConfig` field; `cubarium-core` is not yours.

**Files you own:** `crates/cubarium-search/src/census.rs`, the `Census`
command's lines in `crates/cubarium-search/src/main.rs`,
`crates/cubarium-search/tests/depth_census.rs` (extend) and new files under
`crates/cubarium-search/tests/`, `design/7_Research/`, `runs/`. **Do not
touch** `crates/cubarium-core/`, `es/`, `evaluate.rs`, `calibrate.rs`,
`factorial.rs`, `apex_audit.rs`, `population.rs`, or the other commands in
`main.rs` (workstream X is in `es/` and the `Es*` commands concurrently).

## Read first

- [R's note](../7_Research/ecology-v1-depth-census-2026-09-16.md) in full:
  the pre-registration, the override (`census.rs`: the roster skimmer's
  `genome.depth` written at tick 0 and the phenotype re-decoded; the 0.10
  control is a no-op by construction), the design (two configurations, six
  training seeds, arms 0/1/2, 180,000 ticks, sample every 600), the measures
  (F's variety census columns, composition over time, where the bodies stood,
  E's net margin per body), the verdict clauses **L, M, V, F, D** and their
  thresholds (a clause needs 12 runs and 5 seeds; V is the grazer's horizon
  population at 0.60× the control), the disclosed pre-registration defect,
  and §"The next task this implies" — the ladder, and the two cheap measures
  it asks for. R's rows: `runs/ecology-v1-depth-census/{runs.jsonl,
  summary.json, report.txt, analyse.py}` (build `c38b5a6`, schema 16,
  half-space predicate).
- [O's note](../7_Research/ecology-v1-depth-factorial-2026-09-16.md) for what
  `depth` decodes to (`h_pref = −1 + 2·depth`) and why 0.55 is the grazer's
  own value; [F's note](../7_Research/ecology-v1-movement-2026-09-16.md) for
  the census columns.
- Since R ran, the shipped pursuit predicate changed (schema 17,
  [V](../7_Research/ecology-v1-predicate-adoption-2026-09-16.md)). Arm 0 has
  no predator, so its rows must be **unaffected**: R's arm-0 rows at 0.10 and
  0.55 are your reproduction targets, `final_state_hash` for
  `final_state_hash`, across that change. Say so in the note; it is a
  second check that the adoption reaches nothing without a predator.

## Deliverables

1. **The ladder, pre-registered before a row exists** (commit the
   pre-registration first, as R did): `skimmer.depth` ∈ {0.10, 0.20, 0.30,
   0.40, 0.55, 0.75} × {`baseline`, `fast-leaf`} × the six training seeds,
   **arm 0 only**, R's horizon, sampling, budgets and census unchanged: 72
   runs, ≤ 6 wall minutes, 8 workers. The 0.10 and 0.55 rows must reproduce
   R's arm-0 rows field for field (state hash included). The glider sits at
   1.00 and R found it unmoved; 0.75 is nobody's value and is the level that
   tests whether the rescue needs the grazer's niche or merely a height off
   the rim.
2. **R's two cheap measures, added to the census:** (a) E's net-margin bins
   split by **generation** (founder or descendant), so the founder-lifetime
   reversal R read between `baseline` and `fast-leaf` is measured; (b) each
   form-3 body's own **foliage and litter served, by channel**, from the
   per-body ledger it already holds, so "the skimmer now eats the grazer's
   leaf" is counted. Tests first for both: a synthetic ledger with one
   founder and one descendant bins correctly; a body with known served
   quantities is reported to the quantity.
3. **The verdict, per depth, by R's clauses and Astra's rule**, for each
   configuration: a depth is *acceptable* if **L** holds (lineage alive and
   breeding at the horizon in ≥ 12 of 18 runs over ≥ 5 of 6 seeds) **and V
   does not** (grazer horizon population ≥ 0.60× the 0.10 control), and M
   does not; report F and D beside them. The selected ecology is
   `fast-leaf`; `baseline` is reported, not weighed. If no depth is
   acceptable in `fast-leaf`, say so and state the alternative in Astra's
   words (three viable heights and four kinds, or a wet-floor producer: a
   world question, not a genome one). If exactly one or more are, say which
   and what the grazer paid at each. **Do not propose a roster change**; do
   not run arms 1/2; do not touch the diet.
4. **Result note** `design/7_Research/ecology-v1-depth-ladder-2026-09-16.md`
   with the per-depth table (L/M/V/F/D, skimmer lineage share, grazer horizon
   population and its ratio to control, kinds at the horizon, founder and
   descendant margins, form-3 foliage and litter served), a "what Wrysk would
   be approving" paragraph for each acceptable depth or the none case, and
   the routine decisions you made; tests green (`cargo test -p
   cubarium-search`); `graft build`; commit on the branch. Storage
   `runs/ecology-v1-depth-ladder/` ≤ 30 MiB.

Return: branch and commits (pre-registration first), test totals, the
reproduction of R's rows, the per-depth table for both configurations, the
verdict, wall time, usage, evidence and reasoning.
