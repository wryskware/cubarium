---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Independent review of the completed ordinary-fauna pilots

**The two complete pilots support proceeding with the unchanged remaining ten
seeds once the output guard is fixed and the executable re-pinned.** They do not
establish an ecological cause or rule out resource-limited development. The
reducer needs the narrow self-consistency corrections below before its output
is described as validating a complete cohort. No new instrumentation architecture
or tuning is recommended.

## Review boundary and verification

Reviewed main package **9259c2b**, isolated source **4f00121**, base `1d7b386`
(schema 9), and the complete
[diagnostic report](fauna-development-flow-diagnostic-2026-09-13.md).
Canon, Lore and Graft were consulted. Root subsequently narrowed some claims in
`c343d62` and is editing the output guard; those later changes are not silently
included in this source sign-off. The
[interrupted-run review](astra-fauna-flow-recovery-review-2026-09-13.md) remains
historical evidence; the successful pilots are in a separate directory.

Re-ran existing tests using
`CARGO_TARGET_DIR=captures/build-cache/astra-fauna-flow-review` (absolute path
when invoking Cargo): **13 ledger tests, 5 example tests and 19 reducer tests
passed**. Commands are the two Cargo commands in the earlier review plus
`node scripts/fauna-development-flow.test.mjs`. The published patch reverse-applies
cleanly to isolated `4f00121`.

The original two fixture failures are resolved correctly: closing E is changed
from 1.5 to the arithmetically correct 1.25, and worst residual uses the documented
positive magnitude .25. Tolerance is unchanged at absolute **1e-12**. Added
tests exercise terminal dispersal, paid birth and distinct intake sources.

Actual artifacts under `captures/fauna-development-flow-pilots-2026-09-13`:

| Seed | Artifact SHA256 | Members | Deaths / censored | Reconciliation checks |
| --- | --- | ---: | ---: | ---: |
| 1 | `0755038cb537e1ae5d7909cdcc17861ad8c2ac5f609a61e7a0b648bd92047e21` | 455 | 354 / 101 | 13,513,740 |
| 2 | `4de83d121c1bfe852b283ccd401ad4881ca40f80e49fc057d52985bcc29c0451` | 428 | 340 / 88 | 12,326,932 |

Both have all five reported gates passing at tick **144000**; unlike the old
prefix, closing identity is asserted. Independently checked their referenced
input and closing snapshot envelopes, schema, SHA and state hash against actual
files. Reconstructed every retained census from member birth/death boundaries:
**1440 exact hundred-tick samples per seed**, matching all eight form slots and
total population. Across all **883 members**, also checked lifetime tick counts
against reconciliation/growth-observation/upkeep counts; finite nonnegative
opening/closing/residual stocks; paid-child reserve and energy debit identities;
escrow versus opening stocks; and per-form membership/adult/alive tallies. No
disagreement was found. This independently checks available artifact identities;
the observer-on/off state, events and per-tick counter comparisons remain runtime
claims implemented in the harness, not a new replay performed by this review.

Both original pilots name binary SHA
`3e5891a77869acd3174b2b158bdd6b6d0344bcf4aeccbc46fb34e7df21664512`.
The current rebuilt executable instead hashes to
`dba771ee0c85bfe561dc9f778450922e201112601ed36db843a84fc6bde27596`.
Its completed `rebuild-verification/seed-2.json` is **exactly equal to the original
seed-2 artifact after deleting only `diagnostic_binary_sha256`**. Do not relabel
the old pilots as having run the new executable. Pin the next collection's actual
binary and preserve both provenance records.

## Remaining operational and reducer corrections

At isolated `4f00121`, `fauna_development_flow.rs:1327` still uses
`std::fs::write`, with no occupied-output refusal. Thus the earlier no-overwrite
finding is **open at this reviewed commit**. Root owns its separate atomic
`create_new` correction. Use new paths and test that refusal happens before any
replay; do not overwrite the old failure or either pilot.

Four in-memory modifications of the real seed-1 artifact were accepted by
`reduceArtifact` despite the advertised internal-consistency coverage:

| Mutation | Accepted result / missing check |
| --- | --- |
| `forms[3].reached_adult_target = 38` | Prints 38 although member records still contain 27; tally per-form adults/alive/deaths from members, not just total membership |
| Census `samples_compared = 1`, `reconstructed_series = []` | Reports five gates passed with one sample; require exactly 1440 retained-cadence samples and matching series coverage |
| First descendant's `birth_payment.parent_energy_debit = -1` | Accepted; require finite nonnegative amounts and `parent_energy_debit == build_heat + escrow.energy`, alongside existing reserve identity |
| Delete `members[0].reconciliation.worst_residual.energy` | Accepted because iteration only checks present entries; require all three finite nonnegative residual components |

These probes only clone the JSON in memory; no original artifact was modified.
To reproduce, load `seed-1.json`, apply one mutation above to a fresh clone and
call the exported `reduceArtifact` from `scripts/fauna-development-flow.mjs`.
The same checks were independently performed on the **unmodified** pilot data
and passed. These are future false-certification paths, not evidence that the
measured trajectories are wrong. Keep the reducer's explicit distinction from
runtime replay; correcting these checks does not require re-running either pilot
or building another general validator.

## Biological interpretation that the data actually support

1. Only **one of the eight known extinction seeds** is represented by these two
   pilots; the other pilot survives. Seven known extinction histories remain
   unmeasured. Census reconstruction accounts for when members were lost, not
   why another ecological policy would preserve them.
2. Total adult counts include five already-adult skimmer founders per seed.
   Descendant maturation is **22/33** in seed 1 and **6/12** in seed 2. “Most
   skimmers were adult at some point” is not the same as most children completing
   development; in seed 2 exactly half did.
3. Growth steps were rate/headroom-limited **conditional on permission**. While
   still juvenile, the reserve gate was closed on **571704/600586 ticks
   (95.191%)** in seed 1 and **213785/223348 (95.718%)** in seed 2. Consequently
   “not resource-limited” is unsupported. This is a realized prerequisite
   count, not a causal result from lowering that prerequisite.
4. All offered budding decisions were affordable at the funding site. But
   `controller.rs:183–187` already requires reserve, energy, age and no escrow
   before offering `bud`. Zero downstream funding refusals cannot rule out
   stock-dependent reproductive eligibility upstream.
5. `world.rs:819–828` computes requests with **food/(food+K)**, as well as mouth
   rate, effort and headroom. Share=1 and actual=request exclude a loss from
   downstream sharing on these requests; they **do not exclude supply's prior
   influence on request size**. Positive mean own-cell producer does not resolve
   that distinction.
6. The report's skimmer “actual intake” .118/.110 per 1000 member-ticks is
   **assimilated reserve inflow** (.117543/.109982). The corresponding recorded
   raw quantity is .199769/.186361. For seed-1 grazers these measures are
   .168775 and .281832. Name the currency before comparing it. Sensing shares
   describe demand, not an attributed fraction of clamped payment.

The two pilots establish genuine paid growth, a continuing/replacing population
in one seed and cessation of births followed by extinction in the other. They
leave multiple stock, intake, controller and allocation explanations open.
Preserve that result and complete the unchanged all-twelve histories rather than
selecting a mechanism from the two examples. No biological default, core policy,
live world or diagnostic implementation was changed by this review; no long run
was started.

## Follow-up: output guard closed; reducer regressions preserved

Independently inspected isolated **cdeeb55** after the review above. It atomically
reserves the final output with `create_new` immediately after argument parsing,
before cohort reads or replay, and writes/syncs through the held handle. Existing
files, directories, symlinks and an interrupted empty reservation are refused.
Re-ran the example suite against this correction: **7 passed, exit 0**. This
closes the no-overwrite finding; it does not change the original pilots' binary
provenance or certify a later release executable.

The four reducer probes are now runnable, independently owned regressions in
[astra-fauna-flow-pilot-review.test.mjs](../../scripts/astra-fauna-flow-pilot-review.test.mjs).
`node scripts/astra-fauna-flow-pilot-review.test.mjs` against the unchanged
reducer reports **0 passed, 4 failed, exit 1**, each a missing expected refusal.
They read the original seed-1 JSON and mutate clones only. Preserve these red
results until the narrow reducer corrections land. Root's `62f0eb8` narrows the
report's inference claims; no further seed launch was performed by this review.
