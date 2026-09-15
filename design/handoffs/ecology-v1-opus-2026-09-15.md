---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Ecology v1 (Opus): structured plants, distinct remains, distinct digestion

**Ready for dispatch.** The repaired [ecology v1 contract](../ecology-v1-contract.md)
cleared targeted verification after repair 2; see the
[review](../7_Research/ecology-v1-contract-review-2026-09-15.md).
Wrysk's standing rules apply: worlds are always restarted fresh, never migrated;
old trained policies need not reload. Run it in `/home/wrysk/wryskware/cubarium`
under `AGENTS.md` and `WORKING_POLICY.md`. Fable orchestrates and reviews once;
you own all code in this assignment.

Scope discipline, stated here rather than by reference: fresh worker context,
this brief only, one milestone, one targeted review, at most two repair cycles,
link artifacts instead of pasting them, report real usage if the harness shows
it, and finish without finishing the wider goal if the budget runs out, saying
which.

Model and effort: Opus 5 at high reasoning effort. The work crosses the field
reactions, settlement, commit, snapshot schema and the recurrent runtime's
capability masks, and its correctness is an accounting identity, not a test
that a worker can write to confirm its own reading.

Time target: one working session.

## Objective

Implement the contract's §3–§10 exactly as written, with the §11 provisional
values as config defaults, so that the §13.1 accounting tests pass and the
§13.2 scenarios run headless and print their measurements. Stop there.

## Read first

- `design/ecology-v1-contract.md`, whole; §4.0 (subphase table), §4.8
  (propagules), §5 (joint withdrawal rule), §6.4 (bite accounting), §9 (per-stock
  budgets) and §13 (tests) are the parts most likely to be misread. Where the contract is ambiguous or contradicts the
  code, stop and name the clause; do not resolve it silently.
- `design/m2-world-spec.md` "Conversion table" and "Tick order": the accounting
  conventions every new transfer must follow.
- `crates/cubarium-core/src/fields.rs:108-202` (`react`), `world/step.rs:1441-1500`
  (settlement), `:1688-2000` (physiology), `:2001-2060` (commit),
  `world/invariants.rs`, `world/state.rs:23-91`, `snapshot.rs:141-202` for the
  extension pattern (append a trailing field; do not add a mirror module).
- `crates/cubarium-core/examples/food_stock_flow.rs`: the fixture pattern the
  scenario binary reuses (strip to patch, pinned bodies, no weather, no mutation).

## Deliverables

1. **Core model**: config (`PlantConfig`, detritus and organism additions,
   `CONFIG_VERSION` 8, validation as §14), the trailing `EcologyV1State`
   extension with `SCHEMA_VERSION` 16 and refusal of every older schema by name,
   the eight field subphases (§4.0–4.8), the three decompositions and remains
   routing (§5, §8), capability decode and masks (§6.1–6.2), shared mouth rate
   with effort normalisation and the §6.4 bite accounting, observation reading
   `D_eff + C_eff`, invariants and stored energy (§10), view, field dump,
   telemetry and the split intake diagnostics (§14).
2. **Runtime edits, exactly these** (§15.2): `PROFILE_TEXT` gains `|eco:v1`;
   `Capability` masks come from `cap_* > 0`; the `ate` feedback normalises by
   `mouth_rate`; observation sampling reads `D_eff + C_eff`. Update the digest
   and parameter-count tests. Nothing else in `neural/` changes.
3. **Search fixture**: `Layout::build` paints wood and reserve with its foliage
   (§14 "search"); `painted_material` includes them. The optimizer, trainer
   loop, export format and protocol are otherwise untouched; the layout and
   protocol hashes change by construction and that is expected.
4. **No migration** (§15.1): old-schema `.cubw` fixtures become refusal tests
   that assert the named `UnsupportedSchema`; the `*-plus600-r0b` / `-r0d`
   continuation comparisons are retired with a note in their provenance files.
   Do not delete fixture files or generate replacements under this assignment.
5. **Accounting tests A1–A9** in `crates/cubarium-core/tests/ecology_v1.rs`,
   plus the updates the change forces in the existing suites.
6. **Scenario binary** `crates/cubarium-core/examples/ecology_v1_scenarios.rs`
   with one subcommand per B0–B7, printing the §13.2 measurements as a compact
   table, censoring at the 36,000-tick horizon and never extending it. Run B0
   first: its measured terminal states are what the later scenarios paint and are
   judged against, and a large gap from the §11 hand table is a finding to
   report. Do not claim equilibrium from the horizon alone. Run each once and keep
   only the printed summaries.
7. **Result note** `design/7_Research/ecology-v1-implementation-2026-09-15.md`:
   build and commit, the test command and outcome, each scenario's measured
   numbers against its expected direction (B1b and B6b are unresolved by design
   and are reported either way), every place the contract had to be
   interpreted, and open findings. A mechanism that behaves contrary to §13.2
   is reported, not tuned away.
8. `graft build` after the code change.

## Constraints

- No change to the observation or action layout, the GRU, the optimizer, the
  trainer loop, or the motor contract beyond deliverable 2.
- No change to the §11 values except where `validate` would otherwise refuse
  them; say which and why.
- No tuning, no parameter search, no training, no live display change, no
  world reset, no captures. Do not attach policies or restart the cube.
- Keep the hunter extension behaviourally unchanged apart from the routing in
  §8; its existing tests must still pass.
- Presentation changes are out of scope; the presenter only needs to compile
  against the wider `RenderView`. The follow-up presentation task is named in
  contract §12 and is not yours.
- One normal build cache. Scenario outputs are printed summaries, not archives.

## Decision authority

Yours: module and type names, where the subphases live (inside `Fields::react`
or a sibling), test structure, the scenario binary's layout. Fable's: anything
that changes a §4–§10 equation, a §4.0 read/write, a §11 value, or a §15
compatibility rule. Wrysk's: the no-migration and no-reload rules and anything
that would preserve an old world or policy.

## Verification

```bash
cargo test -p cubarium-core
cargo test -p cubarium-search
cargo test -p cubarium
cargo run -p cubarium-core --release --example ecology_v1_scenarios -- b0   # … b7
```

Report failing tests with their output. A green suite is necessary, not
sufficient: the review re-derives A1, A2a, A3b, A6 and A9 independently and
reads the settlement, commit, 3e/3f and 3h diffs.

## Return format

The result note above, plus in the final message: commit hash, test totals,
the scenario tables, the list of contract interpretations, the list of
old-schema refusal tests and retired continuation comparisons, and measured
usage. Link files; paste no logs.

## Stop

Stop after the result note. Held-out checks, training in the revised ecology,
display deployment and the presentation task are separate assignments after
the review.

## Repair cycle 1 (2026-09-15, after the first implementation run)

The first run (`b1dd394`, `6d304f1`) is faithful to the contract and green; its
finding B0-1 exposed a contract defect, fixed in contract §4.4 (reserve share
first, reflush only below `p_reflush·P_cap`) with two new provisional values in
§11 (`q_share` 0.2, `p_reflush` 0.25) and two config fields in §14
(`reserve_share`, `reflush_below`). Contract §18 third round lists what was
accepted from your interpretations. Scope of this cycle, nothing more:

1. Implement the revised §4.4 exactly; add the two config fields with their
   validation; keep every other equation, read/write and value as it is.
2. Extend A1/A2b coverage to the new branches (reserve share taken, reflush
   gated below the threshold and capped at it, full reserve takes no share) and
   add one unit test that a stand at `P ≥ p_reflush·P_cap` never draws reserve
   for foliage.
3. Re-run the three suites and all of B0–B7 once; refresh the tables in the
   result note, keeping the first run's tables under a "run 1" heading so the
   change is visible. B4a's expectation is now "death censored, report the `W`
   decline rate" (§13.2). B3, B4b and B7 should now have donor/reserve arms
   that actually send; report whatever they measure.
4. `graft build`; commit as before; return in the same format, plus a short
   list of any finding from run 1 that the reserve fix did not resolve.
