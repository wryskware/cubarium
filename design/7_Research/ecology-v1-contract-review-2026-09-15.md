---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Ecology v1 contract review

**Latest status — cleared after repair 2:** the remaining shared-withdrawal,
growth-cap arithmetic and inert-fixture corrections are verified. The Opus handoff
is ready for dispatch. This clears the implementation contract, not the ecological
outcomes or a live deployment. Earlier findings below are retained as history.

**Original disposition: repair the contract before dispatching Opus.** The ecological
direction is suitable: foliage, protected structure and finite recovery reserves;
distinct remains; inherited dietary exclusions; paid transfers; and incremental
validation. The findings below concern executable semantics and verification, not
a request to reopen the food-web design.

Reviewed [the contract](../ecology-v1-contract.md) and
[Opus handoff](../handoffs/ecology-v1-opus-2026-09-15.md), with targeted source checks
of mouth-rate decoding and fruit energy accounting. Evaluated small examples of the
written equations in Python; these are arithmetic counterexamples, not simulator
results. No simulator runs, source edits, agents or live changes were performed.
The stated owner choices of fresh worlds and incompatible old policies are retained.

## 1. P1 — Recolonisation cannot establish a growing plant

Contract §4.8 (`:172–180`) transfers all non-respired propagule material to `W`.
A previously dead recipient therefore has `P = Q = 0`. Income is proportional to
`P`, and foliage growth requires income or `Q`. Thus `P = Q = 0` remains invariant
regardless of light or mineral nutrients. Becoming “alive” by crossing `W_min`
does not make the recipient capable of growth.

At the threshold, with the provisional constants:

```
W = 0.02, P = 0, Q = 0, dt = 0.05
A = 0
unpaid maintenance = 0.0005 * 0.02 * 0.05 = 0.0000005
W after dieback = 0.0199995 < W_min
```

It dies on the next tick. An overshoot can delay death but cannot create leaves.
B4b's establishment/rebuilding expectation is impossible under these equations.

**Repair:** specify a paid viable propagule, for example an explicitly budgeted
allocation to structure and starter foliage/reserve. Define accumulation below
`W_min`, whether that material pays maintenance or decays, and the one-time
establishment event/counter. Clarify initialization too: §11 can seed positive
`P,Q` with `W < W_min`, even though that is classified as bare. Add a deterministic
establishment scenario that demonstrates positive foliage and subsequent income,
not only crossing the wood threshold. No free starter material or energy.

## 2. P1 — The update order specifies three different donor balances

§4.8 caps donor spending from **post-3a–3d** reserves; §9 step 3h says
**post-3a** reserves; §9's final paragraph says **pre-tick** reserves. Earlier
clauses also describe all field reactions as reading pre-tick stocks while their
equations mutate those stocks sequentially. These are different algorithms.

A pre-tick donor balance can already have been spent on maintenance or reflush;
spending it again on propagules can overdraw reserve. Senescence, ripening, death,
decomposition and transport likewise need explicit rules for competing withdrawals
and newly added material. Deterministic iteration alone does not resolve this.

**Repair:** give every subphase a named input state, capped transfers and an output
state. Recommended: compute local plant results first; take one immutable snapshot
of the surviving donors after those results; proportionally allocate each donor's
remaining budget across recipients; commit all transfers together. Specify where
construction nutrients are deposited. State exactly when new litter/carrion/wood
becomes eligible for decomposition and transport. Remove contradictory clauses.

## 3. P2 — Several acceptance scenarios cannot test their stated claims

- **A2:** `g = 0` does not disable every energy source. Unchanged fruit ripening
  imports `(e_f - e_v) * delta_F` from light; current `fields.rs:146–148` confirms
  that separate path. An illuminated mature stand with growth disabled can still
  gain stored energy. For a no-input test, explicitly disable every light-energy
  conversion and external input. Separately test the full ledger with ripening on.
- **B6:** a spatially closed planted patch still receives light and creates food.
  It is not a finite-energy-surplus fixture as written. Separate a finite-input
  reproduction test from a coupled, renewing patch where births and deaths are
  measured without assuming starvation must occur.
- **B3/B4b:** an exact end-of-tick `P_cap` target is not generally attainable with
  senescence/ripening after growth; even `P = 1.2` falls to `1.19994` after the
  stated senescence alone. Use a defined recovery fraction and report censored
  times. B4b cannot establish an approximately one-hour rebuilding claim inside
  the blanket 30-minute horizon. Keep the budget and report partial recovery,
  or explicitly specify a justified longer scenario rather than silently extending it.
- **B5:** a “foliage-only” cell can generate edible litter through senescence.
  Freeze those conversions for direct dietary-exclusion checks; in the coupled
  scenario record which food was actually consumed. Initial food identity alone
  is insufficient to conclude that a detritivore consumed foliage.
- **A3:** zeroing the listed rates does not freeze every new pool: `Q` can still
  fill from income, and death/routing can still alter others. Define a genuinely
  inert fixture. Also reconcile the test for `rate*dt > 1` with §14's instruction
  to reject those configurations; a named validation refusal is a valid outcome.

These fixes do not require training or a broad parameter experiment.

## 4. P2 — The quoted plant/grazer balance has an arithmetic error

§11 compares available plant production of `0.0018 m/s` to a bite written as
`0.05 * 0.73`. That expression is `0.0365 m/s`, about **20 times larger**,
not approximately equal. Mouth-rate decoding indeed uses per-second rates
(`genome.rs:406`). Intake must additionally distinguish genome mouth size, effort,
saturation, swallowed material and assimilated yield.

**Repair:** recalculate the example with one fully specified animal and stand,
including senescence/ripening and construction costs where relevant. Reconcile
B1's expected coexistence with that calculation, or mark its outcome unresolved.
Do not tune provisional defaults merely to preserve the original expectation.

## 5. P2 — Handoff constraints contradict required implementation changes

- Deliverable 1 requests a new frozen `snapshot/v15.rs`; contract §14 explicitly
  says no v15 mirror and §15.1 specifies refusal rather than migration.
- The handoff says the only runtime edit is the profile digest, while it also
  requires capability decoding/masks, intake feedback normalization and changed
  observation sampling. “Same dimensions and GRU” does not mean “digest edit only.”
- It freezes the trainer while requiring ES fixture painting changes. Name the
  permitted fixture/protocol compatibility work and preserve the optimizer separately.
- The final return format asks for re-anchored fixtures while deliverables retire
  those comparisons and forbid replacements. Ask for the actual retired/refusal
  coverage instead.
- The handoff references a `bounded-agent-work` skill which is no longer present
  in the advertised local skill directory; current WORKING_POLICY.md also removed
  that reference. Follow current repository instructions and make the assignment's
  own scope explicit rather than depending on a missing file.

**Repair:** make the contract and handoff agree on these items so Opus can proceed
without choosing which instruction to violate. Preserve the stated fresh-world and
policy incompatibility choices; no migration design is requested.

## Presentation and milestone boundary

Deferring finished artwork is reasonable for this headless core milestone. However,
the contract explicitly says dead wood will still look like bare ground. This
milestone therefore cannot yet claim to resolve Wrysk's visual observation. Keep a
small subsequent presentation task explicit, showing persistent living structure,
foliage loss/recovery and dead wood before judging those effects on the cube.

## Fable repair handoff

Repair the contract and its Opus handoff for findings 1–5. Preserve the ecological
scope and owner's compatibility choices. Provide exact revised establishment and
subphase semantics, coherent scenario conditions, corrected arithmetic and matching
implementation constraints. Add a short response mapping each finding to its fix.

Use equation-level examples where sufficient. Do not implement the simulator, run
training, launch another research campaign, or change the live display. Return the
two repaired documents for targeted verification; do not dispatch Opus yet.

Billed usage is unavailable. Graft reported approximately 15,092 tokens saved.

## First repair verification

The revised contract supplies paid propagules containing structure, starter foliage
and reserve, defines the establishing class, avoids inconsistent initial bare plants,
and adds an establishment counter and scenarios. At the proposed crossing package
`W=0.02, P=0.02, Q=0.01`, with `L_eff*mu=0.6, N=0.4`, potential income is
`2.953846e-6 m/tick` against maintenance `2e-7 m/tick`; the original zero-income
dead end is removed. This is an equation check, not a simulation outcome.

Donor reserves now consistently come from post-plant results. The no-input energy
fixture disables ripening, dietary-exclusion scenarios freeze conversion pathways,
finite-input reproduction is separated from renewing-patch reproduction, and
recovery measurements are fractional/censored. The handoff now agrees about
snapshot refusal, runtime changes and fixture updates. These repairs are accepted
within the original review scope.

### Remaining blocker: shared withdrawal budget (original finding 2)

Revised §4.0 and §5 take both decomposition and fall from the **pre-tick** stock.
Each withdrawal is individually bounded, but their sum is not. A3b explicitly
permits each fractional rate to reach one per tick. For a cell with a downhill
neighbour, no new deposits, `D_minus=1`, `De_minus=2`, and
`k_d*dt=fall*dt=1`:

```
D_after  = 1 - 1 - 1 = -1
De_after = 2 - 2 - 2 = -2
```

The same issue applies to carrion. The provisional defaults do not trigger this,
but the contract explicitly requires the allowed endpoint configurations to remain
nonnegative. An implementation cannot satisfy both instructions as written.

**Recommended repair:** let fall act on the surviving portion of the old stock:
`eligible_D=(1-k_d*dt)*D_minus`, with energy treated identically, then transfer
`fall*dt` of that eligible amount. Apply the corresponding `k_c` rule to carrion.
Keep new deposits and incoming transport ineligible until the next tick as intended.
Alternatively, define and validate a joint withdrawal cap explicitly. Add the
combined-rate endpoint to A3b/A1; separate endpoint tests miss the defect.

### Remaining numerical correction (original finding 4)

The revised §11 computes leaf growth as `(A-M)/(1+c_g)=0.00163 m/s`.
However, §4.2 caps growth at `r_p*W=0.002*0.6=0.0012 m/s`, and also at the
remaining foliage capacity. At `P=P_cap` there is no growth headroom at all.
The stated `0.0004 m/s` net yield and the resulting “roughly a dozen stands”
estimate therefore are not the output of the written model at that state.

**Repair:** evaluate a specified below-cap stand with both caps, then senescence
and fruit ripening. Or label an uncapped income calculation as an upper bound,
without using it as a predicted sustained yield. Update B1b's area estimate
accordingly while retaining its unresolved outcome; no parameter changes needed.

Minor fixture clarification while editing: A3a's claim that every field is inert
also needs `drop=0` or explicitly zero initial fruit. Fruit drop was not included
in its disabled-rate list. Use a dry, nonreacting water fixture as intended.

### Second repair handoff

Fable: address only the remaining shared-withdrawal rule, the capped plant-yield
calculation and the inert-fixture clarification above. Keep the accepted repairs,
ecological scope, defaults and implementation boundary. Update the contract's
response table. No simulator implementation, training or live changes. Return for
verification before Opus dispatch.

No simulation or source changes were performed during this verification. Graft
reported approximately 10,462 tokens saved in this turn; billed usage unavailable.

## Second repair verification — cleared

The joint decomposition/fall rule now limits total removal to the original stock:
`k*dt + fall*dt*(1-k*dt) <= 1`. Energy withdrawals use the source's current density.
Verified 486 small equation cases spanning empty/stocked cells, current-tick deposits,
mixed energy densities and both rate endpoints. Material and energy balances closed,
stocks remained nonnegative, and density caps held. These are arithmetic checks,
not simulation tests or evidence of ecosystem sustainability.

The revised yield calculation applies the foliage growth cap and accounts for
senescence and ripening. Its light-class estimates are explicitly compared with
the new B0 measurements. A3a now disables fruit drop and specifies an inert dry
fixture. A3b includes the previously missing joint withdrawal endpoints. The
handoff references these rules and includes their targeted verification.

Two routine clarifications were made directly during sign-off: all zero-denominator
withdrawal ratios are zero (avoiding `0/0` after complete decomposition), and B0's
terminal states are the baseline for later fixtures without claiming equilibrium
from a 30-minute horizon. No equations, defaults or ecological scope changed.

The contract is ready for the bounded Opus implementation. Fable orchestrates the
implementation review as assigned. Training, presentation and live deployment
remain subsequent tasks. No source implementation, simulator runs, agent launches
or live changes were performed here. Billed usage unavailable.
