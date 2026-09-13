---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Reserve-targets-v1: independent experiment-readiness review

Disposition: no concrete blocker found to freezing and executing the full paired
experiment. This approves collecting evidence, not selecting the candidate,
claiming biological viability, or deploying hunters to the live cube.

Reviewed the root-owned diff in `examples/hunter_compare.rs` and
`examples/hunter_compare/eligibility.rs`, subsequently committed as `b547ad0`.
Used Lore first and checked the actual source against the
[single-family proposal](astra-hunter-stock-bottleneck-proposal-2026-09-13.md) and
[closed reproduction review](astra-reproduction-hardening-closure-2026-09-13.md).
The reproduction module SHA256 remains
`a0f8d7ce233d8205dbb6db4e65b3388a21116f1a5bb056d67d3a0559a8899a66`, exactly the
observer verified in the latter review (`573d5a5`, source `4fe612b`).

## Findings

- Recipe isolation is explicit. CLI default is `baseline`; only
  `--profile reserve-targets-v1` sets seek/perch to 0.80/0.90. The two profile
  modifiers preserving facultative and attack-disabled arms are unchanged.
  Whole-profile equality tests compare each candidate against its corresponding
  baseline with exactly those two assignments. Both profiles validate.
- Founder bodies, fields, configuration and actual material/energy/heat receipts
  match between initializers in all six arms. Costs, reproductive gates, geometry,
  imports and placement have no alternate candidate path in this diff. The
  manifest and openings identify the recipe; actual living-arm profiles and their
  hashes remain recorded.
- The new conditional denominator is **age-and-adult-size-ready member-ticks**,
  not world ticks or all living hunter ticks. Its three stock numerators test
  reserve, energy and both gates within exactly that denominator. Carrying,
  hunting, cooldown or existing escrow can still block an actual funding; these
  counters deliberately do not claim otherwise. Zero denominator means no such
  opportunity observed, not a zero-percent stock success rate. Existing maxima
  remain all-member maxima rather than mature-only maxima.
- Completion requires the entire requested horizon (at least 144000 elapsed
  ticks), an unfailed audit and current arm/reproduction observers. The loader
  requires all twelve distinct prescribed seeds; the global gate requires six
  complete arms for every seed and no failure. Paired-local failures propagate
  into the shared reason before arm summaries; finalization failures prevent the
  global flag. Local and whole-population right-censoring remain reported rather
  than treated as missing observations or successful recovery.

## Verification and limits

`cargo test --offline -p cubarium --example hunter_compare` completed in the
foreground: **74 passed, 0 failed, 0 ignored, exit 0**. This includes candidate
isolation/default/initializer tests, mature-stock classification, complete-horizon
gate cases, hardened reproduction branches, partial-observer failure evidence and
paired recovery tests. The positive two-hour completion test is a predicate test,
not a substitute for a completed two-hour run.

This review ran no new ecological comparison and made no production or live
changes. Both full baseline and candidate cohorts still need frozen-run evidence.
The candidate can increase capture work and prey pressure, and the independent
energy barrier or near-side strike inefficiency can still prevent replacement.
The completion flag certifies observation coverage, not a successful population.
