---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Independent explicit ambient-horizon review

Reviewed `c569f41643d228325b8cc8b4d8e94b4e49596472`, its reducer diff and
[horizon report](ambient-explicit-horizon-reducer-2026-09-13.md), using Lore/Graft
context and the existing noncanonical ambient experiment contract. The tested
reducer and its original test file match that commit. **No defect found in the
bounded horizon-propagation question.** This is tooling readiness, not a 24/72-hour
result or ecological/default-change approval.

The caller explicitly selects the horizon; the manifest cannot choose a shorter
one. CLI spellings are `two-hour`, `twenty-four-hour`, and `seventy-two-hour`, not
the shorthand `24h`/`72h`. Two-hour remains the default.

| Selection | Elapsed ticks | Closing tick | Samples per arm | Attempts per cared arm |
| --- | ---: | ---: | ---: | ---: |
| two-hour | 144000 | 288000 | 720 | 60 |
| twenty-four-hour | 1728000 | 1872000 | 8640 | 720 |
| seventy-two-hour | 5184000 | 5328000 | 25920 | 2160 |

The selected duration reaches manifest/aggregate/arm endpoint checks, extinction
bounds, exact sample count, exact receipt count, sample means and tick-integrated
water means. Receipt and sample constructors reject mixed horizons. The common
opening remains 144000, cadence 200, first shower 60, period 2400, fixed targets
and doses unchanged. All last showers finish before their horizon. Strict
`1e-8 * max(PRE inventory, 1)` material/energy/water limits are unchanged: duration
does not inflate them, and the existing independent/receipt energy gates remain.
Build/SHA, source snapshots, all twelve seeds/six arms, config-factor isolation,
full-width hashes and final snapshot-envelope checks remain in the call path.

## Independent verification

- `node scripts/reduce-ambient-support.test.mjs`: **15 passed**, including strict
  endpoints, unsupported/mixed horizons, missing/extra samples and receipts.
- New `scripts/astra-ambient-horizon-review.test.mjs`: **4 passed**. For each
  longer horizon, a complete synthetic stream changes population, form count,
  producer and fruit halfway through. Their expected global/local means are
  checked independently, along with tick-water means and exact two-birth window
  reconciliation. A two-hour prefix cannot finish either longer stream, and
  energy drift exactly at the original limit is refused. Separate Standard-dose
  streams retain periodic refusals through the exact last scheduled shower and
  reconcile only actually delivered water. These are pure reducer fixtures, not
  simulated ecosystems or evidence of a biological outcome.
- Full completed real two-hour reduction reproduces SHA-256
  `ff9dd52fde1778e316aedac9fbb7e11a9485a46f43aa0ae91b37902dbca14435`, identical to
  the prior committed artifact. No two-hour output field changed.
- Read-only reduction of the actual incomplete 24-hour collection returns
  `artifact_checks_passed:false`, reports the missing aggregate `summary.json`,
  and retains all twelve seed slots. No process handle was polled or restarted,
  and no experiment was launched.

Only this review and the separate independent tests were added. Existing reducer
limits still apply: recorded audit gates and snapshot envelopes are checked, not
independent semantic decoding or material/energy transaction reconstruction.
The ongoing 24-hour collection needs its complete retained artifacts and its own
all-seed biological interpretation when it terminates.
