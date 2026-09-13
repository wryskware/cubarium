---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# 05 — Readable care response, especially rain

Feed, Rain and Clean already act on the same cube/web world, with target selection,
independent doses, accounting and durable replay. Visible acknowledgements and
actual-intake meal continuity are shipped. Clean removes litter, which can remove
food too; reduced feeding afterwards is not automatically a bug or failed flourish.

The historical care screen found heterogeneous biological responses: Feed usually
increased local feeding, rain was mixed and Clean usually reduced it. Fable preferred
a slower v3 rain-response candidate without glowcap, but the root native held-frame
review found subtle readability. That candidate is experimental, not deployed.

## First bounded milestone

Choose one specific missing response to rain that can be perceived at 64 px.
Distinguish a brief acknowledgement of real water input from a biological response
requiring actual water/feeding conditions. Evaluate the existing v3 idea first;
do not commission a new series of variants or repeat the entire care screen.

Use one small reproducible scene or matched short run, including dry/no-care
baseline and repeat input. Retain restrained intensity, local scope, interpolation
and restart continuity. No fake satiation, unconditional swarm, global wake-up,
or attention punishment. Do not add a toxin pool just to rename existing cleanup.

## Starting sources

- [Care response results](../7_Research/astra-care-response-results-2026-09-13.md).
- [Rain playback review](../7_Research/fable-rain-response-playback-review-2026-09-13.md).
- [Root disposition](../7_Research/root-visual-and-ecology-disposition-2026-09-13.md).
- `art/studies/rain-response/`, `art/studies/rain-response-review/`,
  `crates/cubarium/src/care/mod.rs` and `art_present.rs`.

Done for the slice: one visibly justified accept/reject result, with actual care
semantics and relevant replay/accounting checks preserved. If a biological change
is necessary, formulate its bounded test before expanding scope. Deploy accepted
code/assets through the normal launcher; a docs-only or rejected study needs no reset.
