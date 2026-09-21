---
status: open
date: 2026-09-21
owner: Fable (orchestration); decision recorded from Wrysk
---

# The browser lifts its head: a vertical mouth reach, then retrain

Wrysk, 2026-09-21, on the D3 autopsy (`design/7_Research/voxel-census-
2026-09-20.md` §D3): the browser starves with foliage in its cone because
its mouth only reaches a crown at exactly its own head layer, and grown
crowns leave that layer. Decision: **give the browser a vertical mouth reach
of one voxel above its head (0.25 m, one body length), as a body trait; then
retrain the browser on an arena whose crowns stand at several heights; then
ship the new centre as the live default.** Foliage along the crown's height
(the plant-side alternative) is deferred to the art round and is out of
scope here.

Facts the package rests on (`crates/cubarium-voxel-fauna/src/{manifest,body}.rs`,
`crates/cubarium-voxel-flora/src/lib.rs`): browser body 0.25 × 0.125 m,
`mouth_reach_body_lengths` 0.25, bite point 0.125 m ahead of centre, probe
five columns about one voxel wide; `mouth_foliage_stand` accepts a stand only
when `site.y + crown_voxels(wood) == standing_y + 1`; crown heights grow with
wood: bloomcrown and umbrellafrond 1 → 3 voxels, springturf and velvetpad
0.5 → 1, stonecushion and glowcap 0.5, one preset 2 → 5. D3: 2 of 33 deaths
had a crown at the mouth; intake stopped at minute 45 with 4.5 organic of
foliage inside 2 m; cone sectors read zero at the stop.

Owner: one Opus 5 worker, high effort, steps in order. Crates:
`cubarium-voxel-fauna` (manifest, body), `cubarium-voxel-sim` (arena),
`cubarium-search` (es/voxel), `cubarium` (assets/policies, README there).

## Step 1 — the body trait

Add `mouth_reach_up_voxels: u32` to the manifest: browser 1, blind 0. The
mouth accepts a stand whose crown layer is in
`standing_y + 1 ..= standing_y + 1 + mouth_reach_up_voxels`; the contact,
taste and cone rules are unchanged. The blind lineage's manifest digest must
not change (its behaviour is identical); if the digest hashes the new field,
say so and keep the blind digest stable by construction. The browser digest
changes, so every existing browser centre, including the shipped default, is
refused by this build until step 4 replaces it: that is the always-fresh
rule, not a regression.

Tests, specified here, written before the rule: (a) a browser at standing
height y bites a stand whose crown is at y + 2 and could not before; (b) it
still cannot bite a crown at y + 3; (c) the blind founder's mouth on litter
is unchanged and its digest equals the value recorded before this change;
(d) `browser_mouth_foliage` (the D3 diagnostic) agrees with the stepping
rule on a fixture with crowns at y + 1, y + 2 and y + 3.

## Step 2 — an arena whose crowns stand at several heights

The frozen arena does not grow plants, so seed them grown. In the browser's
Stage A and Stage B layouts, draw each springturf-or-other stand's wood per
layout so that crown layers cover **head, head + 1 and head + 2** across the
training set (the last one is unreachable and must be learned as such), with
at least one reachable crown per layout and the start off food as today.
Version the arena protocol strings (`task.rs`) so no earlier centre is
evaluated on this geometry unqualified. A short test proves the height mix
over the 16 training layouts and the held-out eight, and that every layout
keeps a reachable crown.

## Step 3 — retrain

The P3-C pipeline (`design/handoffs/voxel-senses-phase2-briefs-2026-09-19.md`,
package P3-C and integration note 6): record the browser heuristic's
teacher streams on the new arena, fit the clone, evaluate it, then ES from
the clone on Stage A and on Stage B landed (32 pairs, 512 updates, 16
layouts). Report the landed held-out table with the P3-B accounting (score,
acquired, depleted, bitten, reacquired, closest approach, sense ticks) and a
new column: bites on crowns at head + 1. The Stage B result to beat is
P3-C's 8/8 reacquired at .808.

## Step 4 — ship and measure on the living world

Replace `crates/cubarium/assets/policies/frondgrazer-*.json` with the new
centre (replace, never edit; update the README's provenance row). Then the
D3 arm exactly: `voxel_founder_autopsy 60 generated closed` with the built-in
defaults. Evidence of improvement, from the autopsy brief: browser bites
continue past minute 45; the mouth-contact fraction at death rises well above
2/33; browser starvation deaths fall; plant regrowth and the closed water
ledger unchanged. Report the browser death count and minute distribution
beside D3's 33.

## Rules

Explicit-path commits ending with `Co-Authored-By: Claude Fable 5.1
<noreply@anthropic.com>`; never `git add -A`; do not edit
`design/handoffs/README.md`, `config/tachyon/*` or `scripts/tachyon-*` (the
owner has uncommitted edits there). Always fresh. Tests ≤ 200 ticks; no
bit-identical pins (a digest equality on a manifest is fine). No other
constant changes: not the horizontal reach, not the cone, not births. Runs
may use all cores; `runs/` is disposable. Windows only through the png sink.
Return ≤ 40 lines: commits per step, the digest note, the arena height mix,
the retrain table against P3-C, and the 60-minute autopsy against D3.
