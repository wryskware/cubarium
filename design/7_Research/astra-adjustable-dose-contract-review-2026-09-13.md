---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Adjustable dose: bounded contract review

Reviewed handoff `e55501d` against the pre-dose core care/world/water code,
snapshot mirrors 7–10 and current schema 11, host intake/journal and HTTP parser.
Lore retrieval and canon review preceded source inspection. This is a contract
review while Opus implements the package, not implementation sign-off or a claim
that its migration tests have passed. No production or live changes were made.

The handoff identifies the important existing guarantees correctly: held-boundary
durable admission, uncertain-write hold, bounded journal/queue/client state,
integer semantic identity, unchanged cooldowns and allowance, real rain and
proportional litter-energy export. Two details need explicit implementation gates:

1. **Separate legacy omission from a malformed new record.** `accepted` with no
   dose means 1000. Reject a dose field on that legacy discriminator rather than
   interpreting it differently from an old executable. `accepted_dose_v1` must
   require its integer dose; omission is corruption, not a request for the default.
   Test missing/null/invalid doses in the final newline-terminated record as well
   as interior lines, including records behind the snapshot cursor. The existing
   verifier parses all complete records before building the remaining schedule.
   Its unknown-discriminator refusal already gives the proposed new tag a useful
   fail-closed boundary for old readers. Old `accepted` readers ignore extra fields,
   which is precisely why adding a dose field alone is unsafe.
2. **Negotiate viewer capability.** The old `parse_care_request` also ignores extra
   fields. A new/cached viewer can therefore send 1500 to an old host and receive a
   standard action. Add a status capability with version/default/range; without
   that capability the viewer must remain standard-only and not send a nonstandard
   dose. Receipts must show the queued dose, not the current UI selection. Test the
   new viewer against an old-capability status response. Journal versioning alone
   does not protect this mixed-version HTTP case.

## Concrete implementation checks

- Freeze the old `ActiveShower` and `CareState` layout, not just `WorldState`.
  `v8.rs`, `v9.rs` and `v10.rs` currently import live `crate::care::CareState`;
  each must use the frozen shape, and a new v11 mirror must preserve hunters as
  well. Schema 7 has no care and needs no fabricated care bytes. Preserve the
  existing refusal of *any* non-empty schema-10 hunter history/control, not only
  currently living hunters.
- A migrated shower must retain sequence, application boundary, cell ordering,
  weights and delivered count, with standard dose added explicitly. Validate the
  new persisted dose along with existing `delivered == tick - apply_after_tick`,
  unique cells, normalized positive weights and one-active-shower constraints.
  Saving at delivered 0, 1 and 119 catches distinct boundary mistakes. A receipt's
  scheduled total is not proof of delivered water.
- Preserve standard arithmetic grouping: feed calculates `add = m * weight`, then
  `rho * add`; rain calculates `(4 * weight) * envelope[k]`; clean caps each cell
  with `min(0.5 * D, 2 * weight)` and exports `De * (take / D)`. Compute the scaled
  nominal once without rearranging these operations. Compare genuine old-command
  continuations with the new standard command path, including seam/rim targets;
  comparing only omitted and explicit standard in the new implementation would
  miss a common arithmetic regression. The no-shower path must still pass `None`
  to water, not a newly allocated zero array.
- Canonicalize HTTP omission before duplicate comparison. Same ID/kind/target
  with omitted versus explicit 1000 is a duplicate; another dose is a conflict
  while queued and after terminal outcome. After receipt eviction retain the
  existing stale refusal, and after restart retain epoch retirement: neither is
  permission to retry with a fresh identity automatically. Outcome JSON remains
  diagnostic; accepted commands, not outcome quantities, drive replay.
- Exercise the existing delayed-ack and uncertain-partial-write tests with a
  nonstandard command, including a mixed old/new batch. Reopen from the matching
  old snapshot with no outcome and from a post-application snapshot; each must
  apply the accepted dose exactly once at its recorded boundary. Recheck the
  512-byte accepted estimate/outcome reserve against maximal serialized rows.

Legacy projection helpers are comparison tools, not rollback images. Current
v8/v9 projections already drop later extensions; v7 ecology hashing drops care,
corrections and hunter-extension semantics. Equal projected hashes cannot prove
equal pending rain or equal full continuation. Prefer refusing projection of a
nonstandard active shower; if a lossy comparison is retained, name and document
that loss explicitly. Even after the shower finishes and old state fields can
represent its numerical effects, a dose-bearing journal remains incompatible with
an old executable. Rollback still needs a matched pre-upgrade snapshot/journal.

Disposition: the bounded dose design is implementable without changing ambient
biology. Forward the two explicit gates above and verify the listed invariants in
the worker's actual code/tests before acceptance. No new tests or long runs were
performed for this source-grounded handoff review.
