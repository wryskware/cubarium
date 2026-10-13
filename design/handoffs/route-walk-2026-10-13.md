---
status: open
date: 2026-10-13
owner: Opus (high), worktree `.claude/worktrees/route-walk`, branch `route-walk`
---

# ROUTE-WALK: the pack route graph must link the ground a hound walks

## Rules

Read `design/handoffs/behaviour-ai-worker-rules-2026-10-12.md` first (the 2026-10-12 rules hold).
- Never wrap `scripts/lab/lab` in `timeout`. Quick lane only (`quick = true`, `hosts = ["desktop"]`,
  `cwd` set, under 15 minutes). Anything longer: submit and hand back with the job id. Never wait
  on a job; no tool call over 5 minutes.
- Build in your own worktree's `target/` (reflink-seeded from the main checkout's) at -j 8 or lower.
- Never write "chorister" except where code needs the identifier. The animal is the Veilhound.

## The finding (DEN-REACH follow-up, diagnostics merged in 84f76949)

Seed 9 of the Veilhound clone world (`runs/bin/den_reach_trace-diag{1,2,3}`, jobs
`1013-102232-den-reach-diag1-4e4b`, `1013-102500-den-reach-diag2-9d76`,
`1013-102725-den-reach-diag3-47de`; trace example `crates/cubarium-search/examples/den_reach_trace.rs`
with `map_at_s=` and `moves=1`):
- The den (6,14,1) is in a 23-face component (a one-column strip x=6–7, z=1 along the world's
  depth edge). Between it and a basin component (x=11–18, y=11–12), columns x=8–10 at z=1 have
  **no graph faces**, yet hounds walk across that gap both ways (out at ticks 612–622 through
  (9,14,1); back at 5886/6000 from (10,13,1) to (9,14,1)).
- So three hounds 0.4–2 m from their den have "no way home" (`must_walk_home`, `pack.rs`) and rest
  in the basin from t≈60 s to t≈290 s.
- Likely mechanism (inferred, not verified on that cell): `RouteGraph::for_founder` drops a face
  unless a disc of footprint radius + `clearance_margin_m` (0.03, `knobs.rs:713`) clears at the
  face centre (`encounter.rs` ~:500, ~:507); the motor walks it with the bare radius from an
  off-centre position against the wall behind.

The graph is a planning model of what the body can walk. Where it disagrees with the motor it is
wrong (an absurd world for the animal); fix the model, not the den rule.

## Do

1. **Verify the mechanism** on that cell (file:line): which test drops the faces at x=8–10, z=1.
2. **Fix the graph** so faces/edges exist where the motor walks, without linking places the motor
   can't pass (a graph edge the motor refuses strands a hound mid-route, which is worse). Options:
   judge clearance at the best position within the face rather than its centre; judge with the
   bare radius and keep the margin for path smoothing; or whatever the evidence shows. Any new
   number is a serde knob with a registry row.
3. **Tests** (mechanism, small fixtures; no tuned numbers): a strip against a wall that the motor
   walks is linked on the graph; a gap narrower than the body stays unlinked; the motor walks
   every edge the fix adds in a fixture (step a body along it).
4. **Blast radius:** the graph is shared with hunting (routes, PIT, intercept). Run the fauna,
   voxel-sim and search suites. `a_plain_lineage_trains_exactly_as_today` must still pass (if it
   reads the graph and changes, say exactly why and stop for the parent). Measure graph build
   time before/after on a wide world (it runs in the sim).
5. **Re-check (quick lane):** the seed-9 trace (`den_reach_trace`, share=1) before/after:
   `stranded_member_samples`, `no_way_member_samples`; and one 10-minute wide-world census
   (`config/desktop/terrarium-wide.toml`) for pack kills and stuck hounds, before/after.

## Boundaries

- Graph construction only. No change to den rules, hunt tactics, kicks or knob defaults beyond the
  graph's own. A save change bumps the fauna schema to the next free number; say which.
- Don't merge (the parent merges). EC2 never. Commit explicit paths, never -a. Trailer:
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Return (at most 40 lines)

The mechanism (file:line), the fix and knobs, tests, suite results, graph build time before/after,
re-check job ids and numbers, commits, departures.
