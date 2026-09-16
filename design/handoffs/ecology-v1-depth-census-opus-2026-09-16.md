---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream R (Opus): the skimmer at depth 0.55 in a reproducing world

Fable orchestrates. Item 3 of the reconciled next steps in
[the round-3 result](../7_Research/ecology-v1-round3-results-2026-09-16.md),
from O's named next task and Astra's
[round-3 review](../7_Research/ecology-v1-round3-review-2026-09-16.md) (P2 on O,
next steps item 3). You own this brief; Fable reviews once with at most two
repair cycles. Model: Opus 5, high reasoning effort. Time target: half a
session. **Worktree.** Run under `AGENTS.md` and `WORKING_POLICY.md`. Shared `target/`: set `CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target`; a build failing on a symbol the tests just used means another tree was building — `touch crates/cubarium-core/src/lib.rs` and rebuild; pin `CUBARIUM_SEARCH_BUILD=<your commit>` in recorded stamps. If you are in a worktree, first `git log --oneline -1` and `git reset --hard <the brief commit>` if HEAD differs. `runs/` is git-ignored: read retained rows from the main checkout and copy your outputs back there at the end. Commit by path only (never `git add -A`); leave `.claude/*`, `WORKING_POLICY.md`, `.agents/` and other uncommitted design documents alone. Do not push, tag, restart the cube, or touch `state/`, port 7393, the shim or the running `cubarium` (the cube is live for the owner). Commit messages end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Tests are their own pass, written from the definitions before the implementation. Ask no questions; record routine choices in the note. Never change `WorldConfig`'s fields: adding one changes `calibrate::config_hash` for every existing TOML and breaks policy provenance and every retained row.

**Files you own:** a **new** `crates/cubarium-search/src/census.rs` (+ its
`lib.rs` module line and CLI dispatch line), `crates/cubarium-search/tests/`,
`design/7_Research/`, `runs/`. Read-only use of `calibrate`, `evaluate`,
`movement`, `depletion`, `factorial` (`Roster::of`); no core change. Another
worker owns `lifecycle.rs` and the calibrate/depletion files this round.

## Read first

- [O's note](../7_Research/ecology-v1-depth-factorial-2026-09-16.md) as
  corrected: `depth` decodes only to the preferred embedded height in the
  steering term; the roster skimmer's 0.10 parks it on the wet rim where the
  world grows litter and no foliage; at 0.55 (the grazer's value) cold-founded
  sterile clones reach the horizon 26 of 32 and the foliage diet wins; the
  reconciliation with F is plausible, not identified, and **F's census with
  only the roster depth changed is required**. [F's note](../7_Research/ecology-v1-movement-2026-09-16.md)
  for the census by form × diet bin × guild and its 150-minute, six-seed,
  three-arm design; I's and M's rows for the control hashes at the shipped
  price (`runs/ecology-v1-ladder/ladder/evals.jsonl`, `runs/ecology-v1-plant-budget/present-off/evals.jsonl`).
- Astra: confirm the roster correction if the skimmer establishes a lineage
  across seeds without replacing the world with a new monoculture; refute if
  the founding rescue disappears under reproduction or harms variety. Do not
  bundle with a wet-floor producer.

## Deliverables

1. **The depth override, search-side.** After the ordinary world is built
   (24 founders), set every roster skimmer's `genome.depth` to the treatment
   value and re-decode its phenotype at tick 0, changing nothing else (no
   `WorldConfig` field; verify with a test that a 0.10 override reproduces the
   untouched world's state hash over 2,000 ticks, and that a 0.55 override
   changes only the skimmers' genome and phenotype at tick 0). Reproduction
   and mutation stay on for everyone; offspring inherit as usual.
2. **The census** (≤ 6 wall minutes, 8 workers): `depth` ∈ {0.10 (control),
   0.55} × {baseline, `fast-leaf`} × the 6 training seeds × arms 0/1/2 (apex
   introduced at 6,000 as A's screen), horizon 180,000, ledger on. The 0.10
   rows at arm 0 must reproduce I's / M's shipped-price rows by
   `final_state_hash` (and A's screen rows at arms 1/2, in
   `runs/ecology-v1-calibration/screen/evals.jsonl`). Report per arm: founder
   skimmer survival and first brood, descendant census by form × diet bin ×
   guild over time and at the horizon, founder kinds alive at the end, net
   margin by form × diet bin (from the ledger), water-depth distribution under
   skimmers, and the effect on the other three kinds (their populations,
   births, deaths, margins) and on foliage and litter.
3. **Verdict by Astra's rule**, with the world as the replicate (per-seed
   agreement): does the skimmer lineage persist at 0.55 across seeds; does it
   displace or starve another kind; does the depth-0.55 skimmer's diet
   distribution drift toward foliage as O's interaction predicts? State what
   Wrysk would be approving if he accepted the roster change: one genome value
   in the founder roster, visible on the cube as skimmers that stop dying on
   the rim.
4. **Result note** `design/7_Research/ecology-v1-depth-census-2026-09-16.md`,
   tests green (`cargo test -p cubarium-search`), `graft build`, commit on the
   branch. Storage `runs/ecology-v1-depth-census/` ≤ 30 MiB.

Return: branch and commits, test totals, the census table in compact form with
per-seed agreement, the verdict, wall time, usage, evidence and reasoning
behind each decision.
