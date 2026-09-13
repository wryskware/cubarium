---
design_status: exploration
last_reviewed: 2026-09-13
---

# Root disposition: one non-intervening post-intake opportunity check

Proceed with a bounded streaming shadow of the
[Astra proposal](astra-post-intake-quiet-proposal-2026-09-13.md), informed by
[Fable's review](fable-post-intake-quiet-review-2026-09-13.md). This authorizes
measurement implementation, not an active rest policy, ecological retuning,
schema change, canon promotion or deployment. The sparse birth policy stays Off.

## Fixed rule for this check

- Ordinary actual adults, no escrow, alive, active ordinary mode, no hunter
  world. A fresh positive field-assimilation transaction must trigger an attempt.
- Credit is actual material credited to reserve, not raw food, a fed flag or
  reserve difference. Accumulate within an episode; cap at Q. At a settlement
  boundary, expire old credit if at least20 ticks have elapsed since the last
  positive settlement, **before** adding that boundary's new settlement.
- Select Fable's diet-permitted channel sum, not its alternative maximum-rate
  formula: `Q = 2s * eta_m * (I_g*graze_rate + I_f*graze_rate + I_s*scavenge_rate)`.
  `I_g` requires grazing enabled and diet>=DIET_GATE; `I_f` requires grazing
  enabled and diet>=FRUIT_DIET; `I_s` requires scavenging enabled and
  diet<=1-DIET_GATE. Rates/inputs must be finite and nonnegative, Q positive.
  These constants come from the controller, not duplicated approximate values.
  This is a **diet-permitted unconstrained rate ceiling**, not an attainable
  meal rate: density, headroom, food energy and mode can reduce actual settlement.
- Test admission only after the completed tick's existing physiology, funding
  and death checks. Use the unchanged `Budget::of`/`affordable` for30 hypothetical
  decisions plus one safety tick. Never delay that tick's paid reproduction.
- A hypothetical window starts at completed boundaryB, covers decisionsB..B+29,
  and ordinarily releases atB+30. Refractory ends600 ordinary ticks after
  release/abort, or600 ticks after a refused attempt. Every attempt consumes
  credit, even refusal; none may fire from a timer without fresh assimilation.
- During refractory, a fresh bounded episode may accrue credit. During a
  reserved hypothetical window, do not accrue credit from the continuing
  baseline's intake as if the animal had really been held. Record that baseline
  intake separately; it is not measured counterfactual food loss.
- Retest hypothetical affordability at each baseline decision boundary. Death
  or an incompatible baseline escrow/qualification change ends the compatible
  shadow window with its own reason, not a claimed production-policy abort.
  The baseline is still free to feed and reproduce; its future differs from an
  intervention, which is why this is only an opportunity check.
- Full generation-bearing IDs; prune dead entries at removal. Expire credit
  separately from an outstanding refractory deadline. Remove an idle entry only
  when credit, active window and future deadline are all absent. Bound memory by
  the live capacity; never transfer credit/cooldown on same-slot reuse.

## Review clarifications

Fable's suggestions are not all changes to adopt verbatim:

1. The proposal already starts refractory on **refusal**. Gestating adults
   therefore cannot retry every few seconds. Report actual attempts and distinct
   affected IDs/gestations separately, without deleting repeat attempts from the
   event record or changing the rule to manufacture better acceptance.
2. Dropping an expired-credit map entry must not erase its still-active cooldown.
3. Credit can accumulate during cooldown, so the claimed mandatory extra2–4s
   earning period after cooldown is not established. Keep only the proposed
   long-run held-duty ceiling30/(600+30), not a smaller claimed feeding-loss cap.
4. The1.5s hold is unchanged. Existing outgoing meal hold/fade can consume up
   to about.55s; a guaranteed>=1s pure-rest screen would pre-reject this timer
   arithmetically. Report the duration conservatively and later inspect actual
   native64 transitions; do not lengthen the timer before evidence.
5. Rest effort retains real residual motion. A pause is not a position freeze;
   release's mode and clip must come from actual observation, not an assumption
   that every animal returns to feeding. Shadow output does not predict candidate
   trajectories or provide a proven numerical upper bound.

## Source, finite work and verification

Use an isolated branch from **9fc29eb**, the frozen quiet-comparison baseline,
with policy **Off** in every world. This keeps the existing tested pure budget
helper and matches retained no-care/fed reference artifacts. Do not port the full
schema9 fauna ledger or mix its different cohort age window into this check.
Reuse its minimal settlement-hook pattern to expose actual per-ID assimilation;
all observation state is transient and absent from snapshots/controller/RNG.
No biological extension is added. Live remains the presentation-only6d7a831.

First implement and test the shadow and exact boundary/refusal cases, then run
only **seeds1 and8, no-care and single Standard Feed**, for12000 elapsed ticks
from the retained tick144000 openings. Feed remains Front(32,48), elapsed600,
dose1000. Demand observer-on/off exact payload/events and exact retained Off-arm
continuations. Preserve raw opening provenance, binary/source hashes, failed
artifacts and fixed audit limits. Use exclusive outputs.

Report joint per-ID assimilation, Q, eligible exposure, post-physiology R/E and
budget, actual modes, all attempt/refusal/abort/release/censor reasons, episode
duration, memory bound, and per-form zeros. Separate admission opportunity,
baseline-compatible window length and eventual native readability. Historical
feeding flags and aggregate lifetime flows cannot reconstruct these quantities.

Root reviews this four-arm pilot before an unchanged all12×2 ten-minute shadow.
The proposed usefulness screen remains at least.2% eligible-adult exposure and
15% whole-world compatible-window time in9/12 no-care seeds, spanning multiple
exposed forms; thresholds are unvalidated, not ecological acceptance. No parameter
sweep, longer run or active-policy implementation is authorized by this note.
Quiet/hunter coexistence remains an unfinished overall-goal requirement, not
permanently excluded by this first hunter-free measurement.
