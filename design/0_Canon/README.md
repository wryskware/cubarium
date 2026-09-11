---
design_status: decided
last_reviewed: 2026-09-11
decision_refs:
  - D-0001
---

# Cubarium — canon, authority, and certainty

Design is tentative by default. This vault follows the `lore-v1` conventions
requested by Wrysk. Its ledger is [DECISIONS.md](DECISIONS.md).

## Authority order

1. Wrysk's current instruction.
2. Active accepted entries in the decision ledger.
3. Documents those entries name as canonical sources, within the entry's scope.
4. `decided` implementation briefs, only within the accepted scope they cite.
5. `leaning`, `exploration`, and unclassified documents: nonbinding context.
6. `deprecated` and `99_Scratch` material: history or inspiration.

Authorship, polish, detail, and implementation do not create authority. Citing
D-0001 authorizes these vault rules; it cannot authorize an unrelated design.
When a source combines requirements and recommendations, preserve the distinction.

## Document states

| `design_status` | Meaning |
| --- | --- |
| `exploration` | Possibilities and questions; alternatives may contradict. |
| `leaning` | Preferred direction, usable for planning, still revisable. |
| `decided` | Account of accepted choices; cite active IDs in `decision_refs`. |
| `deprecated` | Retained historical or superseded material. |
| absent | Unclassified; treat as exploration. |

Use `last_reviewed: YYYY-MM-DD` to record an actual review, not to imply approval.
An optional local callout can distinguish an accepted requirement, working
assumption, candidate, or open question inside a document.

## Promotion and supersession

Only Wrysk, or an agent explicitly delegated a named decision, may append an
accepted entry, mark material `decided`, or supersede canon. The initial request
authorizes vault adoption and recording the supplied brief. It does not make
the proposed mechanisms or technology choices accepted.

Agents may develop and revise proposals without asking permission for each
routine edit. This is an authority rule, not a requirement to pause ordinary
authorized work. Request specific authorization only when promoting a proposal.

The ledger is append-only, oldest first. Accepted records are substantively
immutable. A new accepted decision supersedes an old one; do not rewrite history.
Keep just one `0_Canon` directory in this project. We start with one ledger;
Lore also supports per-file records at `0_Canon/decisions/D-NNNN-<slug>.md` if a
later authorized organizational change warrants them.

## Ledger grammar

Use `## D-NNNN — Title` with exactly four digits, followed by Date, Status, Scope,
Decided by, Decision, Rationale, Consequences, Supersedes, and Canonical sources.
Lore parses the ID, `**Status:**`, and `**Supersedes:**`. The other fields are
human provenance and remain necessary for an intelligible record.

Only `**Status:** Accepted` is active. Only an accepted entry can retire another.
For full supersession, the value must be a bare ID list such as `D-0003 and
D-0004`. `None` retires nothing; prose describing a partial supersession also
retires nothing in Lore. State partial scope carefully rather than assuming
the parser has understood it. In per-file records the filename ID is authoritative.

## What Lore interprets

The root `.lore.toml` selects `profile = "lore-v1"`, `behavior = "annotate"`.
This exposes authority metadata without changing retrieval order. `rank` can
also apply authority weights; `off` suspends interpretation. Annotation is the
initial setup choice, not an ecological or implementation decision.

On ordinary documents Lore reads `design_status` and `decision_refs`. An uncited
`decided` claim, or one citing no active decision, loses decided authority.
`7_Research/` is capped at evidence and `99_Scratch/` at scratch regardless of
frontmatter. Neither can establish a requirement. Those directory names and
`0_Canon/DECISIONS.md` carry semantics; other document and folder names do not.
