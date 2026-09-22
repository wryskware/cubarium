---
status: open
date: 2026-09-22
owner: Fable (orchestration); one Opus worker, high effort
---

# Package 1a: founders step ledges, in metres

## Why

The edible-stock observer (`voxel-edible-stock-2026-09-21.md`, integration
note) found the broken link: a founder can never change its standing height.
`advance_candidate` in `crates/cubarium-voxel-fauna/src/body.rs` refuses any
centre column that is not a support face at the body's own standing layer,
and `founder_act` never writes `site.y`. On the shipped terraced landscapes
only 25–40 % of the foliage shares a level component with any browser, the
bloomcrowns share none on two presets, and the eight shredders are seeded on
seven or eight distinct heights, each alone. The legacy species rule has
`climb: u32`; the founders never got one.

Decisions in force: `voxel-organism-decisions-2026-09-21.md` §1 (bodies in
metres), §6 (shared encounter description), §9 (order). This is the route
half of the audit's package 1.

## The rule

- A lineage has a **climb height in metres**, `climb_m`, authored on
  `FounderPhysiology` (or the nearest founder-owned physiology type; not on
  the `Manifest`, whose geometry is in the trained-policy digest and must not
  refuse the shipped centres). Placeholders, listed in `design/backlog.md` §1:
  browser 0.25 m, shredder 0.125 m.
- Converted to whole voxels at the consumer once: `climb_voxels =
  round(climb_m / voxel_m)` (browser 2 on small, 1 on default/wide; shredder
  1 on both).
- A sub-step may move the centre column to a support face whose standing
  layer differs from the body's by at most `climb_voxels`, up or down, with
  the headroom, disc clearance and wade checks applied at the destination
  layer. Larger rises refuse as today; larger drops refuse (a cliff edge), no
  falling. The body's `site.y` and pose height follow the destination.
- **Contact** (`contact_readings`, `senses.rs`): a solid the body could step
  onto is not a wall. Contact reports solids above the climb height only, so
  a steppable ledge reads as open ground and a cliff face or a wall reads as
  before. Document the semantic change on the receptor; the observation
  vector's shape is unchanged.
- **Route connectivity** everywhere it is computed follows the same rule:
  the observer's component builder (`voxel_edible_stock.rs`), the seeder's
  `browser_faces` / feeding-face adjacency (`habitat.rs`), and the terrain
  crate's `walk.rs` if it is what those reuse. One function, one rule.
- The ES arena is flat; nothing changes there. The mouth and eye anchors are
  untouched (package 1b).

## Deliverables

1. The rule as above, in `body.rs` motion and `senses.rs` contact, with
   `climb_m` on the physiology and the voxel conversion beside
   `mouth_reach_up_voxels`.
2. Route connectivity unified on it (observer, seeder, walk).
3. Tests, ≤200 ticks each, written from this brief before the implementation:
   a body steps up one voxel and down one voxel on a 0.125 m fixture and on a
   0.25 m fixture per its `climb_m`; a two-voxel rise refuses on default and
   passes on small for the browser; a drop beyond climb refuses; contact
   reads a steppable ledge as clear and a taller one as solid; the observer's
   component count on a two-terrace fixture is 1 with the rule and 2 without;
   the fauna ledger conserves across a step (no organic/energy created).
4. Fauna snapshot schema bump if `site.y` semantics or any serialised state
   changed (fresh worlds only; refuse old ones).
5. Measurement, after: `voxel_edible_stock 0 preset=small|default|wide`
   route-connected fractions before and after (t = 0 table), and
   `voxel_founder_autopsy 60 preset=small` and `preset=default`: browser and
   shredder deaths, extinction minute, bites. Append both to the census note
   as "Step rule, 2026-09-22".

## Constraints

- No knob tuning beyond the two placeholders; no golden hashes; fresh worlds.
- Worktree `.claude/worktrees/founder-step`, branch `founder-step` from main
  (the commit after package 0's note); `CARGO_TARGET_DIR` inside it.
  Explicit-path commits only; every commit ends with
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Never touch `design/handoffs/README.md`, `design/README.md`,
  `config/tachyon/*`, `scripts/tachyon-*`, `docs/tachyon.md`, `art/gen/*`.
- Do not edit the shipped policy assets or the manifest digest.
- `graft ask "<question>" --source` before opening files; `graft callers
  advance_candidate --depth all` before changing it. No windows.

## Verification

`cargo nextest run --workspace --exclude cubarium-gpu` green; the tests in
deliverable 3; the t = 0 route fractions rise on every preset; Fable re-runs
one arm.

## Return (≤40 lines)

The before/after route table, the two 60-min autopsy arms, what the contact
change did to the trained centres' behaviour if anything visible, the commit
list, and anything the rule could not express.
