# Cubarium working rules

Read `WORKING_POLICY.md` before autonomous work, delegation, deployment or capture
generation. It records Wrysk's cost, latest-build and no-archive corrections.

Cubarium is a persistent ambient ecosystem for five connected 64×64 cube faces.
Current repository status and next work are in README.md and
design/implementation-plan.md. Plans do not imply an implementation exists.

Before using or editing design material, read design/0_Canon/README.md and the
relevant entries in design/0_Canon/DECISIONS.md.

- Written, polished, detailed, or implemented does not mean canonical. Only
  active accepted ledger entries and their cited sources bind within their scope.
- Treat documents without design_status as unclassified exploration.
- Preserve modality: do not turn a leaning, proposal, example, or current
  implementation into a requirement.
- Do not append accepted decisions, mark documents decided, or supersede canon
  without Wrysk's explicit authorization for that specific choice. Continue
  routine authorized design and implementation work without approval rituals.
- Material under design/7_Research/ is evidence, not decisions.
- Use Lore retrieval first for codebase and design-history questions when
  available; verify the referenced source and fall back to file tools when needed.
- The display shim at ~/vuzic/led-cube-shim owns output transport and hardware
  mapping. Consult its current contract and reuse its geometry helpers.
- Keep the normal display free of analytical UI. Development diagnostics belong
  in explicit tools, captures, or logs.

<!-- graft:start -->
## Graft — repo context graph

This repo is indexed in `graft/`: small linked markdown nodes that explain each
system and carry exact file:line spans, kept in sync with the code through git.

For ANY task here — understanding how something works, finding where code lives,
or scoping a change — get context from the graph before grepping or opening
source files. Re-ask freely (it's cheap) and reuse literal identifiers you
already have (symbol, error string, file name) as the query. New to this repo?
Run `graft map` first — a token-budgeted orientation (dir clusters, hubs,
hotspots), no LLM, no key.

- Run `graft ask "<your question>" --source` → ranked nodes with the relevant
  code spans inlined (each hit's ≤8-line crux by default; `--full` for whole
  definitions when the crux isn't enough). Match the tool to the task shape:
  for understanding or editing, the top node IS the answer — cite its
  `covers:` file:line spans and edit straight from `--source`. For
  exhaustive tasks ("every occurrence / every caller of this pattern"), ranked
  results are top-N, not complete — run `graft grep "<literal>"` instead
  (exhaustive over indexed files, grouped by enclosing symbol), falling back
  to raw `grep -rn` only for unindexed files.
- `graft skeleton <file>` → every definition's signature + span, ~10× cheaper
  than reading the file; use it to skim an API surface.
- `graft callers <symbol>` gives precomputed, exact edges — who calls this.
  Add `--direction out` for what it calls, or `--depth N` to walk
  transitively for the full blast radius. For structural questions, skip
  ranking and use this directly.
- Or browse: `graft/INDEX.md` lists every node; follow the links.
- Monorepos and folders of multiple repos rank fairly across sub-projects —
  hits carry `[scope/]` labels naming which one they're from. Narrow with
  `graft ask "<task>" --in <scope>/` once you know where you're working.

If a returned span is truncated ("+N more lines"), open the file at that exact
range before finalizing. Only open source files when a node genuinely lacks a
needed detail, and then at the exact file:line the node points to — never
re-read whole files.

After big code changes, refresh the graph with `graft build` (deterministic,
no API key, $0).
<!-- graft:end -->
