---
status: open
date: 2026-10-08
---

# Species dossier sync (2026-10-08)

Rules: `design/handoffs/worker-rules-2026-10-07.md`, AGENTS.md (the canon rules
especially), WORKING_POLICY.md. Worktree `.claude/worktrees/dossier-sync`, branch
`dossier-sync`. No sim runs or builds are needed.

## Objective

The species dossiers were written 2026-09-21 to 09-25 and have drifted from the sim.
Wrysk: they "should be updated and kept in sync with the current state of the game / sim
world. we may periodically have to do updates as we make more changes, so dont treat
them as frozen." They will also become the prose source for an in-game field guide,
whose numeric facts will come live from the species configs.

## Files

- `design/art-direction/species-dossiers-2026-09-21.md` (D1–D14),
  `design/art-direction/species-dossier-D15-latticevine-2026-09-23.md`
- `design/animal-body-reimagining-2026-09-24.md` (body dossiers: Veilhound, ripple-snail,
  capgnawer, seedporter)
- `design/art-direction/organism-scale-and-roster-2026-09-21.md` (the roster)

Sources of truth for the sim: the code (`crates/cubarium-voxel-fauna`, `-flora`,
`cubarium-observatory`, `cubarium-session`'s `species_table`), the configs under
`config/`, `design/parameters.md`, `design/0_Canon/DECISIONS.md`, and git history since
2026-09-21 (`git log --since=2026-09-21`; commit messages are the reports).

## What to change

- Per species, bring **ecology and behaviour** in line with the sim: diet and foods, life
  stages, breeding, senses, movement modes, habitat, defences, predators and prey, size,
  status (live founder, model-only, or not in the sim), names. Chorister is now
  **Veilhound** (code identifiers stay `chorister`).
- Add a `### Sim state` section to each species with the facts as consistent `key: value`
  lines, each citing its source (config key or `file:line`), and the commit you synced
  against. The next sync is then a diff of this section.
- Where the dossier describes something the sim doesn't do, keep the text and label it
  *not in the sim yet*. Don't delete design intent and don't turn a proposal into a fact.
- Frontmatter: update `last_reviewed`, add `synced_at: <commit>`. `design_status` stays as
  it is.

## What not to change

- **Look**: body plans, parts, shapes, colours, surfaces, the visual-review notes. These
  are Wrysk's (artbook picks, `species-colours-2026-09-24.md`). Where the sim's model
  differs from the dossier's look (for example vane or stem counts), list it in the drift
  list below; don't edit it.
- No accepted decisions appended, nothing marked decided, no canon superseded.

## Deliverable

The edited dossiers, plus `design/handoffs/dossier-sync-2026-10-08-drift.md`: a short
list of contradictions that need Wrysk (look mismatches, behaviours the sim dropped,
species whose role changed), one line each with sources.

## Return (≤40 lines)

Commit, per-species one-line summary of what changed, the drift list's top items, and
anything you couldn't resolve from the sources. Commits on your branch with explicit
paths, ending `Co-Authored-By: Codex (gpt-6.1-sol) <noreply@openai.com>` and no
`Claude-Session:` line. Never merge.
