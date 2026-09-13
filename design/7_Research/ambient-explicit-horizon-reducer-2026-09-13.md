---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Explicit longer-horizon rainfall reduction

The read-only ambient reducer now supports the frozen harness's exact two-hour,
24-hour and 72-hour contracts. Two-hour remains the default. Longer horizons must
be explicitly selected; neither the manifest nor a partial sample stream can
choose or shorten the required horizon. This is tooling readiness, not a longer
experiment result or permission to change the live rainfall default.

```sh
node scripts/reduce-ambient-support.mjs captures/ambient-rain-twenty-four-hour-3e9bc2f --horizon twenty-four-hour
```

The source build and executable SHA, twelve original openings, six arms, 200-tick
census cadence, receipt schedule and opening-scaled strict audit limits are
unchanged. The explicit durations require respectively 144,000 / 1,728,000 /
5,184,000 elapsed ticks, 720 / 8,640 / 25,920 samples per arm, and 60 / 720 /
2,160 rain attempts per cared arm. Endpoints, extinction bounds, means and
depth-time denominators use that selected duration. There is no arbitrary tick
override or automatic horizon detection.

## Verification

- `node scripts/reduce-ambient-support.test.mjs`: 15 tests pass. Added checks
  reject relabelled/shortened horizons, cadence and schedule changes, inflated
  limits, missing/excess receipts, mixed receipt/sample contracts, short census
  prefixes, extra samples and mismatched local endpoints. Synthetic complete
  24-hour and 72-hour streams check coverage and mean denominators.
- Root independently reran the complete real two-hour reduction. Exit 0 and
  `cmp` confirm byte-for-byte identity with the existing committed artifact:
  SHA-256 `ff9dd52fde1778e316aedac9fbb7e11a9485a46f43aa0ae91b37902dbca14435`.
  Output: `/tmp/cubarium-ambient-explicit-horizon-two-hour.json`.
- Against the actual ongoing 24-hour collection, explicit selection passed the
  manifest/build checks but correctly returned exit 1 for missing aggregate
  `summary.json`. All twelve seed slots remain represented: seed 1 has a
  technically complete recorded result; seeds 2–12 had no result yet. This is
  not a full review of seed 1 or acceptance of the partial experiment. Output:
  `/tmp/cubarium-ambient-explicit-horizon-incomplete-24h.json`.

The existing process handle 80539 was polled and confirmed live during this
turn. It was not restarted. No 72-hour simulation was launched. Limitations of
the original reducer still apply: recorded audits and snapshot envelopes are
checked, not independent semantic decoding or material/energy transaction
replay; biology is sampled and does not establish unattended viability.

## Operational check and next decision

The live cube/shared viewer still reports build `0.1.0+9cf0e1d`, PID 3771509,
same state directory and resumed opening tick 633208; observed current tick
660932. Port 7393 has that sole listener; 7395 and 7396 have none. This tooling
change needs no cube restart and deploys no experimental ecology.

Previous turn classification: the requested goal rewrite was delivered, but
made no implementation progress. This turn changes the reducer, completes its
tests and real two-hour identity check, and verifies the ongoing wait. Next:
reduce the complete unchanged 24-hour collection when it terminates, retain
failures and form losses, and decide whether longer evidence is warranted.
