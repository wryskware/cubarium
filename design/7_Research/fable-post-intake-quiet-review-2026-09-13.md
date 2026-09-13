---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Review of the earned post-intake quiet proposal (`post_intake_pause_v1`)

Reviewed Astra's [proposal](astra-post-intake-quiet-proposal-2026-09-13.md) with the
completed [two-hour biological review](astra-quiet-two-hour-biological-review-2026-09-13.md),
against the current core (`world.rs` step order, `controller.rs::decide_quiet`, `quiet.rs`
budget and state, the schema 12 → 13 quiet projection) and the isolated fauna ledger's
hook sites (`captures/diagnostic-source/fauna-development-flow-2026-09-13`, read only).
Nothing was run, edited or launched; the foreground ledger session was not touched and is
not assumed finished. Design evidence only, not canon.

**Recommendation: proceed to the read-only opportunity shadow as proposed, with four
minimal corrections below folded into its specification first.** The rule family is
sound in its ordering against the current core, and the mechanisms it reuses
(`quiet_hold` re-testing the strict budget before every held decision, release without
latch, `underlying` mode carried by the controller, full-ID entries) already exist and
behave as the proposal assumes. The corrections concern the quota normalisation, the
refractory representation, memory bounding, and what the shadow must record.

## Ordering against the current step (verified)

The step is: decide (with the quiet override read *before* the ordinary controller,
`world.rs` ~1063), move, settle feeding once per cell from pre-transfer fields (~1565,
`fed_this_tick` set at 1706), hunter digestion, physiology per organism in one loop
(oxidation → growth → gestation due / new escrow funding → death check, ~1783–1970),
commit deaths and births (~1972) where the existing `quiet_admit` is called at insertion.

- **Trigger vs settlement:** a positive settlement this tick is the only thing that can
  raise credit, and it happens after the tick's decision. So an attempt can be evaluated
  only in the physiology loop or at commit, and its first held decision is the next
  tick's: exactly the proposal's "never retroactively paused". Placing the attempt at the
  *end* of the organism's physiology iteration, after the escrow branch and the death
  check, satisfies "after all existing physiology/funding" and "never prioritise the
  pause before that tick's otherwise-paid child" with no reordering of existing code.
  A same-tick refusal for a just-funded escrow follows from the no-escrow rule.
- **Held ticks:** `decide_quiet` with `hold` zeroes fruit/graze/scavenge effort and
  budding, keeps residual locomotion at rest effort, updates hunger memory once, draws
  the same noise pair. No fresh credit can form while held (no request → no
  settlement), so "credit cannot fire early during a pause" is automatic.
- **Affordability:** `Budget::of` is the proposal's formula (`Sbound = max(S, Sa)`,
  rest-effort movement, sensing, growth and oxidation ceilings), strict, charges nothing;
  `quiet_hold` re-tests it over the remaining decisions plus one tick and aborts
  immediately to the ordinary controller for that same decision. Reusable unchanged.
- **Hunter worlds:** `QuietState::validate` refuses an enabled policy in a hunter world;
  the proposal's boundary is the existing one.

## Corrections (minimal)

1. **Quota normalisation is form-biased as written.** `Q = 2 s · η_m · (2·graze_rate +
   scavenge_rate)` doubles the graze term for fruit, but fruit, producer and detritus
   bites share one reserve headroom and fruit is absent for most forms most of the time
   (the ledger pilots record zero frugivory for the skimmer). A pure grazer must therefore
   eat for at least 4 s at full Type-II saturation to earn what the rule calls "two
   seconds", a scavenger-leaning form even longer, while a form standing in fruit earns
   it in ~2 s. Use the diet's *attainable* channels: `Q = 2 s · η_m · Σ over channels the
   diet can open (graze on producer, graze on fruit only if `diet ≥ FRUIT_DIET`,
   scavenge if `diet ≤ 1 − DIET_GATE`)`, or, simpler and still one constant, `2 s · η_m ·
   max(graze_rate, scavenge_rate)`. Either way the shadow must report episode duration
   to Q **per form**, or per-form zeros will be misread as biology.
2. **Represent the refractory as a tick, not a decision counter.** With no stacking on
   the post-birth family, every decision after release is ordinary and unheld, so "600
   non-held decisions" equals `release_tick + 600` in every reachable state. Persist one
   `refractory_until: u64` per full ID. A per-ID counter that must be incremented every
   ordinary tick is a mutation on every organism every tick and a reconciliation burden
   for nothing.
3. **Bound the per-ID memory explicitly.** Credit, last-positive tick and refractory live
   per live full ID (`OrganismId {slot, generation}`; the existing stale-generation test
   shows the pattern). The proposal must say the entry is removed at the organism's
   removal (the commit step, ~1972) and that the map never exceeds `max_organisms`
   entries; otherwise a long world accumulates dead IDs. Expired credit (1 s without a
   positive settlement) should be dropped from the map, not kept at zero.
4. **The consume-on-refusal rule interacts with gestation.** A gestating adult keeps
   eating, re-earns Q every few seconds and is refused (no-escrow) each time, so the
   event stream carries one refusal per Q-accumulation per gestating adult for the whole
   gestation. That is correct behaviour but the shadow's refusal counts must be reported
   per reason with **per-ID de-duplication** (attempts per gestation), or "refusals"
   will dwarf admissions and read as a budget problem.

## Correctness points that hold as written

- **Full-ID expiry:** generation-bearing IDs make slot reuse at the same boundary
  distinct (the ledger pilots record one such reuse). A stale entry cannot resolve to a
  new occupant.
- **Starvation / missed-intake duty:** death needs `E ≤ 0 ∧ R ≤ 0`; an admitted animal
  has just credited reserve and passed the strict bound, and the hold aborts on the first
  failed re-test, so a death *inside* a held window is not reachable under the bound
  (age death excepted, as in the current family). The duty cap 1.5/(30 + 1.5) ≈ 4.8 % is
  a ceiling; a continuous feeder's real cycle is ≥ 30 s + the time to re-earn Q (≥ 2–4 s),
  so ≤ ~4.5 % of its *feeding* time, i.e. a real if small intake loss for a population
  whose only death cause is starvation. That cost belongs to the later intervention
  comparison and cannot come out of the shadow; the proposal says so.
- **Persisted state / Off:** the quiet extension is appended in schema 13; schema 12
  projection refuses any enabled policy or held pause, migration opens Off and empty,
  and an Off world reads one boolean and never enters quiet code. A new policy variant
  with its own per-ID vector is a change to the quiet extension's own version, refused
  by older readers by construction (postcard rejects the unknown discriminant). The
  `QuietPause.child` field has no meaning for an intake pause; the new family needs its
  own record shape and event variants (`Begin`/`Refuse`/`End`/`Abort` keyed by the
  organism and the credited amount, not by a child), and the quiet observer's
  Begin↔Birth matching (`quiet_compare/bouts.rs`) must not be reused for it.
- **Restart mid-episode / mid-pause:** with credit, last-positive tick and refractory
  persisted, a reload continues the same episode; the presenter's meal memory is lossy
  and is correctly not consulted.

## Meal presentation and native-64 visibility (a confound to measure, not a defect)

The presenter bridges intake gaps for 0.25 s and cross-fades the body over 0.3 s
(`meal_present.rs`: `MEAL_SETTLE_SECONDS`, `MEAL_FADE_SECONDS`). The first held decision
comes one tick after the trigger settlement, so the feed gesture runs ≈ 0.55 s into the
1.5 s hold and the rest pose shows for ≈ 0.95 s; on release the underlying mode is almost
always still Feeding (food present, hunger memory above `seek_off`), so the animal
returns to the feed clip with another 0.3 s fade. At native 64 px that is feed → 0.3 s
fade → ≤ 0.95 s rest → 0.3 s fade → feed. The sail's newly adopted rest gesture peaks once
per 4 s at a hashed phase, so it appears in roughly a quarter of such windows. Whether
this reads as a *pause* or as a state flicker is the visual gate the proposal already
names; the shadow should therefore report the distribution of **predicted visible rest
duration** (held ticks minus the presenter's 0.55 s tail), and the acceptance screen
should ask for ≥ 1 s of visible rest, not 1.5 s of held decisions. If it does not read, a
bounded revision is a longer hold with the same budget, not a shorter presenter tail.

## Reusing the fauna ledger hooks for the shadow

The isolated ledger already records, per member per tick, exactly the quantities the
rule consumes at the sites where they happen: `open_tick` (stocks before anything moves),
`record_intake_settlement` (per channel: requested, actual, `to_reserve`, energy, share),
`record_intake_close` (any actual intake this tick), oxidation, growth gate and growth,
`record_gestation_observation` (bud request, escrow held), funding, death, and
`close_tick` (stocks at the end of the member's tick). Its two-hour pilots proved observer
neutrality and stock reconciliation at 1e−12. A streaming shadow accumulator beside those
hooks is the right shape and needs no aggregate inference. The committed pilot JSONs
cannot supply it: `MemberRecord.intake` holds life sums (`actual_amount`, `to_reserve`,
ticks), not a per-tick series, so episodes are unrecoverable from them.

Exact missing joint data the current hooks do not carry, and must be added for the shadow:

| needed by the rule | present at the hooks? | what to add |
| --- | --- | --- |
| per-tick assimilated material per ID | yes (`record_intake_settlement.to_reserve`, summed at `record_intake_close`) | nothing |
| stocks at the admission point (after funding and death check) | yes: `close_tick` is called after the commit step (worktree `world.rs` ~1318, after deaths and births), so its stocks are post-physiology and post-funding | nothing |
| the tick's ordinary mode (Seeking/Feeding/Resting) | **no** | one argument at `open_tick` or a hook after decisions |
| adult qualification, escrow held | structure at `open_tick`; `record_gestation_observation.escrow_held` | nothing |
| phenotype rates for Q (`graze_rate`, `scavenge_rate`, `diet`) | **no** (only `reserve_max`, `energy_max`) | record once at member creation |
| budget inputs (`maintenance`, `speed_max`, `sense_radius`, `rest_effort`, `structure_adult`) | partially (`adult_target`) | record once at member creation, or evaluate `Budget::of` in-run at the hook |
| refractory clock, episode last-positive tick | derived in the shadow | shadow state |

Two provenance cautions. The ledger worktree is base `1d7b386`, schema 9, pre-hunter: a
shadow there measures the retained cohort's opportunity under that core, which matches
the proposal's hunter-free scope but not the current release's code. The proposal's
screen (9/12 no-care seeds, fed histories) refers to the twelve-seed quiet cohort at
`9fc29eb`, schema 13, on which no ledger hooks exist. So the honest options are: (a) run
the shadow on the two-seed ledger base now for a first opportunity look, clearly labelled
as two seeds of the pre-hunter core, or (b) port only the minimal hook set above into a
frozen copy of the current release and replay the twelve no-care/fed histories read-only.
(b) is what the screen actually requires. Neither is authorised by this review, and
neither may start while the foreground recovery session owns that worktree.

## Ecological confounds to keep visible

- Gestating adults are structurally excluded (no-escrow), so visible quiet concentrates
  on non-reproducing adults; the shadow must report eligible-adult exposure with and
  without escrow so this is not misread as a fecundity effect later.
- The pause freezes an animal on the patch it was eating; it resumes the same patch on
  release. Local depletion and contention are unchanged by design, but neighbours'
  shares during the held ticks rise; the later comparison must attribute that, not the
  shadow.
- Everything above is a **non-intervening opportunity estimate** (root's correction
  stands): held ticks change later credit, stocks and positions in either direction, so
  the shadow bounds nothing about the intervention.

## Limits of this review

Source reading and design reasoning only; no run, no numbers of my own. The ledger
worktree was read, not built. The presenter timing is taken from the current constants;
the actual visible rest length still needs the native capture the proposal already lists.
