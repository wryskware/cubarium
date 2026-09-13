---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Hunter reproductive opportunity: first actual-trial replay

The frozen profile-3 two-hour comparison is still running across all twelve seeds.
This is a diagnosis of **seed 6, facultative-on**, not a cohort conclusion or a
reason to deploy hunters. No profile, live runner, saved input, or care setting was
changed. The chosen body remains Fable's Lanternjaw.

## Implementation and scope

`0ff2f42` adds `hunter_compare/eligibility.rs` and the `hunter_probe` example.
The common harness now records hunter stocks in its sparse census and counts
end-of-step reproductive obstacles over **member-ticks**. Obstacles overlap;
their counts must not be summed into a time denominator. The nine local predicates
are distinct from the two material/energy affordability checks. Population-cap
availability and queued births are intentionally not inferred here.

These are boundary-state diagnostics, **not actual funding attempts**: physiology
can change stocks within a tick, and its age/cooldown check uses the preceding
tick boundary. The newly committed core reproduction records supply the real
transaction evidence. Empty lineages report zero member-ticks and null maxima,
not a misleading eligibility percentage. Storage is constant-size; the per-tick
path allocates no JSON and retains no lifetime ID history.

The probe loads a separate `World` from a copied trial snapshot, advances it without
interventions, drains both transient event queues, checks invariants, and prints
diagnostics. It never writes a snapshot or contacts a running world. An optional
expected closing full-state hash produces a nonzero exit on mismatch. This is an
endpoint fingerprint comparison, not a record of every intermediate state and not
a replacement for the paired conservation audit.

## Replay evidence

Input:
`captures/hunter-profile3-two-hour-2026-09-13/seed-6/facultative_on/post-initialization.cubw`

- Snapshot SHA256: `f1c604f28f6aeb093438913d31fdb759144e676cbe2eaf5b450c94b6f8b3dd41`.
- Opening tick 144000; full-state hash `9332234388831387713`; schema 11.
- Frozen comparison build `0.1.0+9eacb7e`; its executable SHA256 is
  `4906811c15fbf680fe61840d109eae0a7e040ddaaa1121f11661c79432506aa3`.
- Probe binary was compiled with build label `0.1.0+1cbca05`, while the new
  diagnostic source was still uncommitted; that exact source was then checked in
  as `0ff2f42`. Concurrent host art edits were present, but the probe does not draw.
- 144000 elapsed ticks, closing at 288000; terminal exit **0**.
- Closing full-state hash **`8365334319777227972`**, exactly matching the earlier
  frozen comparison's `summary.json` for this arm.
- Output retained at `/tmp/cubarium-hunter-seed6-opportunity.json`. Its initial
  field name `trajectory_matches_frozen_trial` is being narrowed to
  `closing_state_matches_frozen_trial`; only the endpoint was compared.

The hunter made **333 settled attempts**: 96 captures, 117 misses, and 120
out-of-reach settlements. It eventually died of starvation. There were **zero
offspring and zero reproduction transaction/refusal records**, not a funded
cocoon that later disappeared.

| Boundary-state quantity | Observed | Profile requirement |
| --- | ---: | ---: |
| Maximum reserve fraction | 0.5 | 0.8 |
| Maximum energy fraction | 0.7499056079830375 | 0.75 |
| Reserve-gate blocked member-ticks | 138121 / 138121 | — |
| Energy-gate blocked member-ticks | 138121 / 138121 | — |
| Age-gate blocked member-ticks | 23999 / 138121 | 1200 seconds minimum age |
| Local gate open member-ticks | 0 | — |

Material affordability was separately blocked for 116652 member-ticks, usable
energy affordability for 2663, gut occupancy for 18304, and target/hunting for
16797 each. Cooldown, escrow and juvenile-size obstacles each contributed zero.
The opening stocks were reserve 2/4 and energy 3/4; required parental funding was
material 1.6 and usable energy 1.0. Thus this replay points toward sustained stock
replenishment and the reproduction thresholds, not a cap-refunded gestation.
It does **not** yet identify which metabolic, feeding, or behavioral parameter
family should change. Finish the fixed cohort before tuning; do not rescue,
reseed, or filter the failed lineages.

## Checks

- `cargo test -p cubarium --example hunter_compare`: **39 distinct tests passed**
  after the eligibility slice (36 existing plus three new diagnostics tests).
- The probe target also ran those same three tests successfully; these are not
  three additional distinct tests. They check gate versus affordability,
  independent overlapping obstacles, no-member output, non-mutating reads and
  equal continuation hashes over 400 steps with extra observations.
- A real CLI invocation on the same input with `--ticks 1 --expect-state-hash 0`
  printed mismatch false and exited **1**, as intended.
- After both real CLI runs, the input SHA256 still matched the archived opening
  checksum above.

The reproduction audit integration and Fable's presenter/minification corrections
are separate work in progress. The live cube remains on the frozen schema-9
presentation build; none of this introduces a hunter into it.
