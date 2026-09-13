---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Next quiet family: earned post-intake disengagement

**Design proposal only; do not implement or launch yet.** Investigate one bounded,
amount-earned pause in ordinary adult foraging, with unchanged real costs. The next
gate is a read-only **opportunity reconstruction** using actual intake transfers and
coincident stocks. If those records cannot support the rule, report the missing
measurement; do not substitute `fed` counts or retune several thresholds.

This is a new family, not a longer/cheaper version of the birth timer. The completed
[two-hour diagnostic](astra-quiet-two-hour-biological-review-2026-09-13.md) establishes
that post-birth recovery remains~0.03% of living time and~2.8% of world time. The
family is functionally sound but too sparse to establish everyday quiet. Off remains
default; no schema, core, art, care recipe, live state or new simulation changes here.

## Why intake is the next grounded opportunity

The [actual care observations](astra-care-response-results-2026-09-13.md) show real
local eating: Feed raises pooled local actual-fed exposure64.17→77.67%, while fixed
cohorts never genuinely rest in that short screen. The separate
[ordinary diagnosis](astra-quiet-results-2026-09-13.md) finds whole-world actual-fed
time~47%, with form-specific34%,34%,85%,36%, yet zero active→satiated-rest entries.
Mean reserve fraction~.159 and maximum~.701 do not approach the~.9 reserve needed
for hunger memory to enter existing satiated rest; reproduction competes near.7.
Do not relabel that hungry state satiated or delay/discount child funding to force it.

Actual meals contain many one-to-three-tick intake gaps; the
[meal presentation](meal-onset-continuity-2026-09-13.md) bridges them visually. A
single `fed=false` tick is therefore a poor biological “finished meal” trigger.
Likewise one positive flag can represent a very small/low-quality bite. Neither a
mode change nor net reserve growth measures ingested food.

Verified current source: `world.rs` feeding settlement at1569–1714 applies actual
post-sharing fruit/graze/scavenge transfers and their assimilated reserve/usable
energy before later physiology. Scavenged material assimilation depends on actual
detritus energy density. `controller.rs:177–236` computes ordinary hunger/food mode
before the quiet override; resting uses existing effort/turn and suppresses food/new
bud requests. `meal_present.rs:35–45,130–183` bridges intake for.25 s and fades.3 s.
These were read through Lore/Graft and source, not inferred from display poses.

## One candidate to assess, not a sweep

Working name **`post_intake_pause_v1`**. The following are explicit unvalidated
design constants for an opportunity check, not accepted values or a rollout recipe:

| Component | Single proposed rule | Reason / limit |
|---|---|---|
| Population | Ordinary adults at/above actual adult structure, no escrow; no hunter worlds | Protect the already-fragile juvenile growth path and keep threat interactions out of this first isolated slice. This is not all-fauna coverage. |
| Productive episode | Accumulate actual material credited to reserve by field intake; reset after1 s with no positive assimilation | Brief nibble gaps remain one episode; old meals cannot bank a rest indefinitely. One second is an explicit candidate segmentation rule, not copied presentation state. |
| Earned amount | `Q = 2 s × η_m × (2·graze_rate + scavenge_rate)` | Two seconds of a conservative maximum assimilation rate. Fruit and producer each have a graze request; using just `mouth_rate` would omit that second channel. It is a normalization, not a claim every diet can attain this bound. |
| Fresh trigger | A new positive settlement reaches/has reached Q, current actual ordinary mode is Seeking/Feeding, refractory has ended, and completed physiology still qualifies | No timer-only firing, newborn initialization, care receipt, stale history, full-reserve zero request or already-natural-rest admission. |
| Pause |30 decisions =1.5 s, followed by ordinary controller | Leaves roughly.95 s after the existing≤.55 s outgoing meal hold/fade; continuous body transition still needs an actual visual check. |
| Refractory | At least600 ordinary non-held decisions after release/abort/refusal before the next attempt | At most~4.76% long-run added rest duty for repeatedly successful individuals:1.5/(30+1.5). This is a cap, not a target or predicted frequency. |
| Consume opportunity | Clear earned amount on every attempt, successful or refused; no queued retry | A hungry animal is immediately free to forage, not waiting for a hidden affordability latch. Future attempt needs fresh intake again. |

During refractory, current intake can form a fresh bounded episode, but it cannot
fire early; the eventual attempt still requires new positive settlement that tick.
Credit is capped at Q, not an unbounded food-history sum; its expiry is independent
of the refractory. Set finite/positive-rate preconditions; zero maximum rate means
no opportunity, not division by zero. At first enable/load without recorded history,
credit starts empty; no retrospective meal or automatic immediate rest.

The animal deliberately disengages after enough productive intake even if its
patch remains edible. That is the **behavior being proposed**, not an assertion
that existing biology already ends the meal there. Continuous feeders must not be
excluded merely because their food never disappears; exhausted-patch departure is
not the only possible trigger. Do not add grooming, healing, satiety or food bonuses.

## Payment, needs and interruption

At admission after all existing physiology/funding, and before each held decision,
reuse the current conservative budget over remaining pause plus one ordinary tick:
`E > B·h + build_cost·G`, `R > oxidation_rate·h + G`, with actual phenotype,
`Sbound=max(S,Sa)`, and the existing maintenance/movement/sensing/growth bounds.
The admission test is not an escrow or extra debit. Actual maintenance, sensing,
movement, oxidation, ageing and any previously existing paid transfers run once in
their normal order. Current stocks, not historical intake credit, must fund the pause.

The completed trigger tick is never retroactively paused. If it just funded an
escrow or lost adult/energy/reserve eligibility, refuse. Never prioritize the new
pause before that tick's otherwise-paid child. While held, real mode is Resting,
real feed/new-bud requests are zero, residual locomotion stays genuine, hunger
memory/underlying hysteresis update once and RNG draws are unchanged. Recheck
affordability each decision; failure releases immediately to the ordinary controller
for that same decision, with no compensation or deferred cost. This can delay later
intake and new budding; those costs must be measured, not assumed offset by movement.

At default adult oxidation cap.01 material/s,1.5 s plus.05 s safety requires
R>.0155 before any possible growth term. Required E depends on the actual phenotype.
For illustrative rates `graze=.035`, `scavenge=.015`, `η_m=.6`, Q=.102 material;
that does **not** imply.102 remains in reserve after growth/funding/oxidation.
At poor food or contested settlement reaching Q may take far longer than2 s or
never happen. Resource-affordability bounds only immediate exposure, not long-run
viability or the opportunity cost of missed meals.

Do not weaken existing escape: the current quiet family explicitly rejects hunter
worlds, so retain that capability boundary. A future combined world needs a separate
contract where actual sensed pursuit preempts pause before effort/turn/feeding, paid
escape movement stays unchanged, and capture/death clear all history. Do not invent
an omniscient threat radius or claim that this first hunter-free proposal proves it.

Persist bounded per-live-full-ID episode credit/last-positive tick, refractory state,
and an active pause's boundary/end/underlying mode under its own explicit policy
version. Do not stack with post-birth pauses or borrow presenter's lossy meal memory.
Old readers must refuse enabled semantics, old worlds migrate Off, and default Off
must preserve exact no-care/care continuations. Exact representation/schema is an
implementation-review question, not permission to modify schema now.

## Missing evidence and the next finite gate

Existing flag-time, rendered meal captures and10 s stock samples do **not** supply
joint per-ID episode assimilation, current affordability and rearming frequency.
The isolated ordinary-fauna mutation ledger can supply the right transfers; the
[interrupted handoff review](astra-fauna-flow-recovery-review-2026-09-13.md) is not
proof that its later full pilot has finished. Reuse that already-scoped work once
its actual completion/neutrality/stock gates are verified; do not duplicate it or
infer amounts from its prefix/aggregate totals. If only aggregate transfers were
retained, the missing item is a narrowly scoped streaming shadow accumulator beside
those existing mutation hooks, **not** another full transaction architecture.

Before any intervention, report this one rule's hypothetical opportunities on fixed
no-care and fixed-Feed histories: eligible adult exposure, normalized dose/episode
durations, actual underlying modes, co-timed reserves/energy, all refusal reasons,
predicted held duration, cooldown blocks, burst/overlap, per-form and per-seed zeros.
Shadow output is a non-intervening opportunity estimate, not a proven numerical
upper bound: policy-induced missed food/turns and changed expenditure can alter
later credit and stocks in either direction. It is not a predicted candidate
trajectory or proof of successful repeated pauses. Keep full IDs, terminal-death
timing and censoring.

Proposed screen for whether implementation is worth discussing: materially exceed
the birth mechanism's~.03% living/2.8% world-time coverage—e.g. ≥.2% eligible-adult
living time and≥15% whole-world time with any eligible pause in at least9/12 no-care
seeds, with contributions from more than one exposed form. These are explicit
unvalidated usefulness thresholds, not the biological acceptance test; absent forms
and juveniles stay visible as excluded denominators. If the single quota fails,
report **sparse/unsafe opportunity**, do not immediately lower Q or weaken budgets.

Only after an accepted opportunity review: one fixed all12×4 ten-minute copied-world
comparison, actual native64 meal→rest→move captures including juvenile controls,
seam/rim/held/restart tests, strict unchanged inventories and mutation-site costs,
and full identity/recruitment reduction. Required mechanisms: no food/new funding
while actually held; no extra RNG; release without latch; expired/dead/reused IDs;
same-boundary funding/refusal; duplicate records; mid-episode/mid-pause restart;
bounded memory and exact Off identity. No artificial rest pose over ongoing work.

Stop on conservation/state/capability failure, new matched form extinction, repeated
adverse no-care survival/recruitment, dependence on care, unacceptable lost-intake
duty, or failure to show a readable quiet interval at native scale. Mixed outcomes
require review of every seed and both ancestry directions, not automatic rejection
for changed IDs. Passing numeric or visual gates is not permission for24/72 h or
deployment. No such run is authorized or started by this proposal.
