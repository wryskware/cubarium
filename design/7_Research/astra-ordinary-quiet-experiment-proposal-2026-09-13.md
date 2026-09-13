---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# First ordinary quiet experiment: affordable post-birth recovery

**Proposal only: one opt-in, resource-screened two-second pause after a real paid
birth.** It is genuine interrupted activity, not biological satiation, healing,
free energy, or a cosmetic rest pose. No implementation, new run, live change or
canonical decision is made here. Unattended viability is a condition to test,
not a guarantee this untested candidate can make.

## Evidence and alternatives

The frozen ten-minute diagnosis (`astra-quiet-results-2026-09-13.md`, tool
`a89179a`, all twelve mature seeds with untreated/Feed arms) found no active→Resting
transitions. All 417/413 rests were newborn one-tick initialization. Maximum
completed reserve fraction was .70071; updated hunger memory never fell below
.306, while rest needs memory below .1. Observed new escrows repeatedly began
with reserve just above .7 and left roughly .01–.17. With hunger `1 − R/Rmax`,
the approximately .9 stock needed for satiation competes with reproduction at .7.
Most organisms also genuinely carried little reserve; the funding interaction is
not proof that removing reproduction would make every animal well fed.

| Mechanism | What it would mean | Main tradeoff | First experiment? |
| --- | --- | --- | --- |
| Delay funding until satiation/rest | Let R pass .7 and approach .9 before paying the unchanged child | Changes allocation and offspring timing broadly; an unattainable .9 gate can suppress reproduction. Quiet still depends on sustained rich intake. | No: larger viability risk and could merely prevent births. |
| Pause immediately after funding | Recover after committing material/energy to gestation | Acts at the measured stock minimum; missed intake can starve a depleted parent. Gestation art also currently takes visual priority. | No: wrong initial stock moment for the smallest slice. |
| Interrupt foraging after a meal bout | Brief non-satiated handling/recovery despite remaining hungry | Applies broadly, but “fed” is any intake, not meal amount. Frequent pauses can create a duty-cycle food deficit; needs a separate bout/rearming design. | Later family, not bundled here. |
| Short affordable pause after successful birth | A parent briefly reduces activity after completing an already paid offspring | Sparse and reproduction-associated; misses some intake and may delay the next funding, but screens resources and never changes the completed birth. | **Proposed first candidate.** |

Do not simply raise seek-off above .3: that would relabel relatively hungry stock
as satiated, change autonomous hysteresis broadly, and still need its own study.
Do not add a free population, energy or appetite adjustment to rescue a candidate.

## Candidate `post_birth_pause_v1` — one fixed rule

Two seconds = **40 ticks** at the current 20 Hz, an unvalidated candidate duration,
not an accepted default. Reference policy is Off. Candidate/reference share all
existing drives, growth, upkeep, oxidation, feeding, reproduction costs, terrain,
weather, care amounts and RNG streams. The first experiment excludes hunter
worlds; threat/escape interactions need a later explicit contract.

After an **actual child insertion** completes boundary B, the surviving ordinary
parent may start one pause. Admission is based on that parent's current stocks
and the conservative budget below. No start on an escrow funding, cap refusal,
refund, miscarriage, death, replay of a log record or a newborn's own appearance.
The trigger belongs to the successful core birth commit, not the observer's
incomplete post-step escrow inference. If admission fails, record the refusal and
forget this opportunity; do not make a hungry parent wait for permission to act.

If admitted, decisions at B through B+39 produce completed Resting intervals
B+1 through B+40. Expiry at decision boundary B+40 returns to the ordinary
controller. The just-completed birth tick itself is not retroactively repainted
as a recovery step.

During the pause:

- Public mode is genuinely Resting. Use the individual's **existing** rest effort
  and configured rest turn fraction before calculating movement/OU updates. At
  current settings these are .05 and zero. No new translation multiplier or
  “energetic” animation is introduced; tiny residual drift is still real movement.
- Fruit, grazing and scavenging efforts are zero; `fed_this_tick` must therefore
  be false. New budding requests are suppressed until release. The completed
  child's stores and birth event are unchanged; there is no new offspring payment.
- Maintenance, sensing, actual movement bills, oxidation, growth and death checks
  still run in their existing order, with their existing transfers. Hunger and
  hunger memory remain truthful and continue updating. Age is not suspended.
- A depleted parent may abort early before that tick's pause decision if it cannot
  cover the remaining conservative budget. It then uses the ordinary controller
  on that same tick; no free meal, stock floor, deferred bill or compensation.

**Avoid a hidden hysteresis latch.** Merely writing `Mode::Resting` and clearing
a timer can keep an animal resting in the .1–.3 hysteresis band after the promised
pause. Retain the underlying ordinary mode in the pause entry. On each pause tick,
evaluate its normal hunger/food mode transition from the actual current world and
that underlying mode, then apply the recovery override **before** turn/effort and
intake/bud selection. Update the underlying mode and the real hunger memory once.
On exit use the underlying mode for ordinary hysteresis, not the imposed Resting
value. Keep exactly the original turn RNG draws; do not run two independent
controllers or reset noise/memory. If natural satiated rest is then warranted it
can occur, but is recorded separately from the bounded recovery override.

This is a pause in work and foraging. “Recovery” does not assert that energy must
increase during it; upkeep can still consume stores.

## Resource safety and opportunity cost

At admission, and before every paused decision, let `h` be remaining ticks plus
one ordinary-tick safety margin, expressed in seconds. With this individual's
structure S, adult structure Sa, reserve R, energy E, speed vmax, rest effort f,
sense radius r and maintenance coefficient m, use:

```text
Sbound = max(S, Sa)
B = m*Sbound + move_cost*Sbound*(f*vmax) + sense_cost*r   [energy/second]
G = min(growth_rate*h, max(0, Sa-S))                     [material]
Q = oxidation_rate*h                                   [material]
admit/continue only if E > B*h + build_cost*G and R > Q + G
```

Use validated finite nonnegative inputs. This bounds the no-food costs under the
present ordinary core: wading cannot increase speed, growth cannot exceed Sa,
oxidation can burn at most Q and adds rather than removes usable E. The bound
deliberately assumes maximum oxidation/growth and **does not count their possible
energy benefit**. The extra ordinary tick leaves a small positive margin. Age
death remains possible; the check is not immortality or a long-run survival proof.

This is an affordability test, **not a debit or escrow**. Actual existing bills
are paid once, in their usual phase and at actual amounts. Do not charge B upfront
and then charge ordinary upkeep again. At defaults an adult two-second interval
needs a conservative reserve margin just over .02 plus any possible growth;
actual E budget depends on the parent's size, movement and sensing, not a common
free-energy allowance. Newly funded .01-reserve parents would often fail—which
is why the proposal does not use funding itself as the trigger.

Skipping food has a real cost: at most two seconds of would-be intake, and at
most two seconds of immediate next-funding opportunity per admitted pause.
Against Seeking, rest saves the movement component proportional to `(1-f)*vmax`;
against Feeding it saves only `(feed_effort-f)*vmax`. Maintenance and sensing
remain paid. These savings need not outweigh lost food. No intake amount can be
computed from the current `fed` flag; compare measured stocks, births and survival.

Reference opportunity scale is modest: 417 births across twelve ten-minute
worlds is about one birth per **17.3 seconds per world**. If every reference birth
admitted a full two-second pause, it would supply only about **0.12%** of observed
organism-time as recovery. Actual admission may be lower, and candidate births
can differ. This is a scale estimate, not a prediction or a claim that it supplies
all desired ambient stillness. If it is too sparse, report that and evaluate a
separate mechanism; do not secretly lengthen it or force depleted parents to rest.

## Persistence and truthful presentation

Add an opt-in versioned ordinary-quiet extension outside the legacy nested
organism/config payloads. Policy Off plus an empty map is inert. Active entries
need full parent ID, originating successful-birth identity (e.g. child ID plus
boundary), start/end ticks and underlying ordinary mode. Bound entries by live
organism capacity; remove them on expiry, abort or death, and reject stale IDs,
duplicates, invalid policy/version/durations and impossible ranges at load.

Use the next available snapshot schema when implemented; freeze the preceding
schema's complete nested layout before adding fields. Old snapshots migrate to
Off with no retroactive pauses. A candidate snapshot resumes its remaining ticks
and underlying mode exactly, including an immediate restart after the birth.
Do not infer timers from old birth logs or serialize them in a browser preference.
An old-format projection of an enabled policy/active pause is not behaviorally
equivalent and must refuse; Off continuation can still be compared with the
genuine prior schema. Default no-care **and default care** trajectories must remain
byte-identical in that compatible projection, with no additional RNG draws.

No live control or care-journal change is needed for this first copied-world
experiment: choose policy explicitly in the experiment initializer, snapshot it,
and never hot-toggle it. Standard Feed stays the existing durable care command.
Publish transient begin/refuse/end/abort records with reason and identity for
measurement. Normal art may use the actual Resting mode; development output must
distinguish `satiated`, `post_birth_recovery` and `newborn_initial`, with no analytic
labels added to the ambient display. Do not route this through the meal gesture.

## Fixed paired experiment and gates

Use all twelve existing mature openings, no re-aging or winner selection. Four
arms per seed: Off/no-care, candidate/no-care, Off/fixed-Feed, candidate/fixed-Feed.
The care recipe is exactly one Standard Feed at elapsed 600, Front (32,48), as in
the quiet diagnosis; no later rescue, cleanup or rain. This is a controlled input,
not a care obligation. Ambient support is identical throughout. The candidate is
one behavior policy, not a support/hunger/growth sweep.

Run ten minutes first, then the unchanged candidate/reference recipe for two
hours, 24 hours and 72 hours if earlier gates pass. Keep every seed, empty form,
rejected pause, death and failed technical arm. No hunter introduction in this
slice. Numerical success and biological acceptance remain separate.

Measure exact recovery admissions/refusals/abort reasons, actual completed
duration, active→recovery versus natural active→Resting versus newborn rest,
censoring and cooldown identity. Accumulate tickwise intake/mode/real transported
path lengths and store/energy/heat changes. Retain full IDs, parents, actual paid
births, refunded/miscarried escrows, offspring outcomes, population organism-time,
starvation/age deaths, form richness and surviving opening ancestry. Forms are
not lineages. Do not claim exact funding/oxidation from post-step deltas where the
ordinary API still lacks mutation-site evidence.

Technical gates: unchanged strict inventories and independent flow/receipt
audits; exact Off continuation; unchanged paid-child accounting; no food or bud
request during a held interval; full actual upkeep; no extra RNG draws; seam/rim
motion; budget-boundary/early-abort tests; no timer extension from duplicate
observation; uninterrupted versus mid-pause restart equality; no newborn/refund
or dead-parent false admission; release from the hysteresis band at the exact end.

Behavioral screen (proposed, not validated): real non-newborn recovery bouts of
at least one second in at least nine of twelve **no-care** seeds, without forcing
the rest. Report opportunity-normalized rates and visual sparsity even if this
screen passes. A candidate with only one-tick entries has not met the objective.

Viability is decided from no-care arms first. Do not promote a candidate with new
extinctions, losses of forms/ancestry that persist in their matched references,
or a consistent decline in living-population time and paid reproduction across
seeds/horizons. Inspect every paired loss, not just a favorable pooled mean;
longer-term child recruitment matters more than raw birth count. If shorter
pauses save energy but suppress recruitment, that is a tradeoff to resolve, not
proof of success. Feeding must not be necessary to clear these gates. Twelve
seeds cannot prove universal noninferiority; retain uncertainty and leave the
autonomous default Off until the evidence supports a change.

Sources inspected: current `controller.rs::decide`; `world.rs` movement bills,
ordinary physiology and successful birth commit; `organism.rs` hunger/escrow;
the frozen quiet diagnostic/report. Lore and Graft were used before source
inspection; research remains noncanonical under the unchanged canon rules.
