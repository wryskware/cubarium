---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Independent reproduction-event API review

Reviewed implementation `5ee8fee` and its contract/report `022367f` against the
actual `World::step` mutation sites and the hunter experiment contract, sections
4 and 6. Canon was read first and Lore retrieved the transaction contract before
source verification. This is evidence, not canon or an ecological balance claim.

**Disposition: no blocking defect found in the reviewed event additions.** They
provide the exact mutation-site quantities the paired harness lacked, without
changing the biological assignments, draw calls, persisted layout, or existing
offspring/birth identities. Root still owns host reconciliation. No production
source, test, live process, or live state was changed by this review.

## Mutation-site verification

| Record | Actual mutation and accounting | Linkage and ordering |
| --- | --- | --- |
| Funded | Captures parent R/E immediately before and after the existing debits; escrow S/R/E are exactly those assigned; build heat is the same `build_cost * S` passed to `heat`. | Full parent ID plus persisted `Escrow.started_tick`; emitted before the subsequent death check. |
| Born | Records the same escrow S/R/E assigned to the inserted child; birth heat is the same `e_r * S` released because completed structure carries no chemical energy. | Same escrow key; child is the actual inserted ID; one Offspring and one ordinary Birth carry the same parent/child and completed tick. |
| Refunded | Captures parent stocks around the actual additions of escrow S+R and E; nothing is discarded or additionally heated at this boundary. | Same escrow key, no child/Offspring/Born; existing retry delay remains unchanged. |
| Miscarried | Reports escrow S+R and `e_r*(S+R)+E`, split by the actual detritus energy cap into stored energy and heat. | Emitted before removing hunter membership; key/cause identify the gestation lost with the parent's death. Body and carried gut are separate terms, not silently included in escrow. |
| NotFunded | Emitted only in the existing insufficient-stock or capacity branches, where no escrow was assigned and no resource debit occurred. | No escrow key and no future closure. A repeated refusal is not another funded gestation. |

Natural-death commitment occurs before placement of due births. The physiology
pass removes a dying parent from the due-birth list, so it produces a miscarriage,
not both a birth and a miscarriage. A parent that funds and reaches its age limit
in the same tick first emits Funded, then Miscarried with the same key and tick.
Both remain observable even though its closing world state contains neither
parent nor escrow. Hunters are excluded as prey, so the earlier predation removal
path does not bypass a hunter escrow's natural-death hook.

The refund's “no heat” means **no new heat at refund**, not reversal of the build
heat already spent when funding. At birth only the escrowed structure's reserve
energy is released; it is not charged a second build cost. A miscarriage exports
the escrow's still-stored energy, not the earlier build heat. These distinctions
are necessary to avoid double-counting in the observer's common energy audit.

## Timing, identity, and observer handoff

- All new records use the actual completed step tick `now + 1`. The existing
  escrow uses `started_tick = now`, deliberately retained without a biology
  change: **Funded.key.started_tick = Funded.tick - 1**. Later closures use that
  same persisted key, not a new timestamp or counter. Full generation-bearing
  parent IDs prevent slot reuse from aliasing a gestation.
- A parent's single escrow cannot be funded twice in the same tick, and the
  due-birth branch cannot refund and reopen it in that pass. Each observed funded
  key therefore has one of the three terminal outcomes, unless still open at
  observation end. Preserve the emitted same-tick ordering when handling loss.
- The hunter stream emits **Offspring before Reproduction::Born** for a birth;
  ordinary LifeEvent::Birth is in the separate life stream. Reconcile the complete
  tick batch one-to-one, rather than requiring Born to precede Offspring or using
  current membership to reject a same-tick-deceased parent. The new `tick()` and
  `hunter()` accessors cover every variant by inspection.
- “Exact” here means authentic quantities read at the mutation, not bitwise
  algebraic equality after arbitrary floating-point subtraction. The regression
  tests themselves allow numerical residuals. Use explicit finite/range checks
  and the experiment's justified residual limits, not universal `==` on derived
  funding/refund identities.
- The queue is still transient `World` state, drained by `mem::take`, not part of
  `WorldState` or snapshots. Drain/stream it each tick. A mid-gestation restart
  preserves the escrow key and reproduces **future** events; it does not recreate
  an already-drained or crash-lost Funded record. If an observer starts from such
  a snapshot, mark opening escrows as pre-existing/left-truncated or restore its
  own prior evidence. Never fabricate the missing funding debit from closing
  stocks. This API is not a durable event journal.

## Independent tests and neutrality evidence

Ran foreground, to terminal success, against clean current core sources whose
latest implementation commit was `5ee8fee`:

```text
cargo test -p cubarium-core --offline --test hunter_reproduction --test hunter
7 reproduction + 27 hunter = 34 passed, 0 failed
```

Read all seven reproduction tests: funding/birth identities and actual child
inventory, due-birth cap refund, pre-funding cap refusal, escrow-only miscarriage
with capped energy, same-tick funding/loss, saved-gestation event/hash replay, and
accessors/order. The existing hunter suite also checks full-world funding/birth
conservation and death/refund behavior. There is no dedicated new fixture for
`NotFunded::Stocks`; that simple observation-only branch was verified in source,
so the seven-test result should not be described as exhaustively exercising every
refusal reason. This is not a blocker identified by the review.

Independently verified the provenance of the two no-biology-change hash fixtures:
extracted `git archive 9eacb7e` into
`/tmp/cubarium-reproduction-review-SPe1mG`, added only a temporary test reusing
that archived suite's unchanged helpers and the current fixture's exact scenarios,
and built/run the **pre-event core** offline. The production sources in the archive
were untouched. Its results matched the current committed regression:

| Scenario | Steps | Pre-event and current full-state hash |
| --- | ---: | ---: |
| Certain capture followed by digestion in the quiet fixture | 400 | 15771965630209811797 |
| Funded breeder in the quiet fixture, with one actual birth | 200 | 3410443867288973882 |

The archive probe passed **1/1**, checking both hashes and the actual birth count.
This goes beyond trusting constants described as recorded from an earlier build.
Source diff inspection additionally confirms that the new paths only capture
locals and append transient events: the same debit/refund/heat statements and
RNG draw calls remain in the same order. `WorldState`, profile values and snapshot
encoding were not changed by `5ee8fee`. The intervening schema-10 change relative
to `9eacb7e` was refusal wording/documentation, not a successful-load or tick change.

Two deterministic scenarios are bounded regression evidence, not proof of every
configuration or long-run ecological outcome. Together with the unchanged
mutation/draw sites and saved-gestation replay test, they support the claimed
observation-only nature of this API slice.
