---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Independent review of the sail candidate (stable body + fin-only coverage)

Reviewed commit `a88b15b` ("Stabilize sail move silhouette with explicit fin-only coverage
bake"): `art/creatures/sail.tscn`, `art/bake.gd`, `art/sail_fin_coverage.gd`,
`assets/atelier/creatures.png`, Astra's report `astra-sail-stable-body-2026-09-13.md` and
study `astra-sail-aa-study-2026-09-13.md`, and the evidence under
`captures/sail-stable-2026-09-13-{render-final,meal-seed1,meal-seed8}`. At review time the
candidate was still dirty in the working tree; it was committed as `a88b15b` while this
review ran, and the review targets that commit. Nothing was edited; no source, atlas, host
or live process was touched. Root owns deployment.

**Verdict: targeted concern, not a hold.** Ship the moving-body hold on its own merits; the
fin coverage is a real trade whose direction static frames cannot settle, so root's
copied-world preview should judge it against a body-hold-only fallback before it goes live.

## What the candidate actually is (verified against the diff)

- Scene: the move clip's five `Body:scale` keys become `(1, 1)` instead of `(1, 0.95)` ↔
  `(1, 1.05)`; one new metadata flag `sail_fin_coverage4`. Nothing else in the rig moves:
  fin keys, timing and amplitude are the originals; rest/feed body tracks and the bud clip
  are untouched.
- Bake: an explicit opt-in compositor for this rig only. The two fin layers are resolved
  with a 4×4 coverage kernel in linear premultiplied colour (uniform blocks keep their
  exact codes), then the point-baked body and bud are composited over them with opaque
  body texels copied exactly. The layer count, order, paths and textures are validated, so
  the policy cannot silently spread to another rig.
- Atlas (my own comparison of `HEAD~`'s `creatures.png` against the candidate): only rows
  4–7 (sail rest/move/feed/bud) differ; the other twelve rows are byte-identical;
  `pack.json` is byte-identical. The bud row differs only in RGB stored under alpha 0
  (144 texels, none visible) — rendered bud is exact.

## What I measured independently

Fin texels are every texel that is not part of the byte-identical opaque body. Per frame,
mean over the 16 samples:

| state | fin texels at alpha ≥ 0.9, old → new | at alpha ≥ 0.5 | summed fin alpha ("light") |
| --- | --- | --- | --- |
| rest | 18.4 → 8.0 | 18.4 → 14.1 | 18.4 → 18.2 |
| move | 21.6 → 13.4 | 21.6 → 20.4 | 21.6 → 20.8 |
| feed | 21.5 → 11.4 | 21.5 → 18.0 | 21.5 → 22.0 |

So the fins keep their light but lose about half of their solid texels: the gold tips and
purple edges become two-texel gradients. In the move loop, four of the sixteen samples
(the axis-aligned fin pose) are byte-identical to the old bake, the other twelve are
soft; the fin edges therefore go crisp → soft → crisp with every flap. Astra's own
measurements (`measurements.json`) agree with what I see: the body alone is 28 texels in
all 16 move poses (was 19 ↔ 37), fixed-anchor alpha-area SD falls 4.38 → 1.07 in move,
1.20 → 0.19 in rest, 1.54 → 0.35 in feed, and the ≥ 0.5-alpha silhouette shrinks in rest
and feed (51.4 → 47.2, 50.2 → 47.2 texels).

## What I looked at, and what it shows

Inspection was of static frames: my own 8× strips of all sixteen samples of each changed
row, old over new (`/tmp/sail-review-DK0EnM/strip-*.png`), 8× crops from the recorded
meal captures' native old/new frame streams on the sail's own face (an adult with a real
Feed-pulse onset, seed 1 slot 8:4; the retained weak juvenile case, seed 8 slot 5:3; two
adults mid-bout), and Astra's browser screenshots of the study viewer. The 64-px motion
sheets are too small for the image tool at their native size, and the browser screenshots
render the native panels too small to judge; the enlarged panels were readable. No
continuous playback and no physical cube was observed; nothing below is a claim about
either.

- **Move.** The old strip shows the body popping a whole texel row taller every fourth
  sample — the "whole fish inflating" — which is not an authored gesture but the 5 %
  squash crossing a sampling boundary. The candidate's body is rigid and crisp in all
  sixteen samples and the cyan face is exact. This half of the change is an unambiguous
  fix with no visible cost: a 0.3-px squash on a six-texel body cannot be expressed at
  this resolution, only aliased.
- **Rest and feed.** The old fins rotate as crisp texel steps (the twinkle the coverage is
  meant to remove); the candidate's fins are soft halos with translucent tips beside a
  fully crisp body. In a still frame this reads as mixed sampling — a sharp fish with
  blurred fins — and the fins are less *defined* even though they are not dimmer in
  total. On LEDs a 15–30 % alpha texel is a dim LED, so at room distance the solid part
  of a resting sail's fins is about half its former extent. Whether smoother motion
  outweighs the loss of definition in the calm states is exactly the question a still
  frame cannot answer.
- **Juvenile (0.6 scale).** In the recorded seed 8 juvenile crops the fins are smudges in
  both old and new; the candidate is not visibly worse there but the body-hold benefit is
  also hard to see at that size. Astra's measured −8.2 % mean juvenile move luma is a
  real cost of softer fins under bilinear minification; I could not see it in the crops.
- **Seam, rim, vertex.** Only Astra's screenshots and metrics were available at a
  judgeable size; the candidate uses the unchanged production owner/transport helpers,
  so I have no reason to expect a seam-specific difference and saw none in the enlarged
  seam and rim panels.
- **Handoffs.** Feed → bud and bud → move enlarged panels show no jump; the bud row is
  rendered-identical, and its static fins are crisp because they do not rotate, so the
  feed → bud fade also fades the fins from soft to crisp over 0.3 s. That is consistent
  with the rule, not an artifact, but it is one more place the fins change character.
- **Actual meals (seed 1 and seed 8 recorded worlds, same frames both packs).** On the
  crowded top face the two packs are nearly indistinguishable at 8×: the world's plants,
  ground and neighbouring bodies dominate, and the fins' softness is one texel-gradient
  among many. The candidate is restrained in the real world; it is also true that its
  benefit is mostly visible in the isolated move loop.

## Concrete changes identified (not made)

1. **Bake a body-hold-only fallback** — the scene change without the metadata opt-in — so
   root's copied-world preview can compare three packs: shipped, body-hold-only,
   candidate. If the softened fins read as blur on the cube, the body-hold-only pack keeps
   the whole defect fix at zero fin cost; if they read as smoother flapping, the candidate
   stands. This is a one-flag rebake, not a new study.
2. If the fins are judged too soft but the smoothing is wanted: a 2×2 kernel on the same
   two layers (alpha in quarters, one-texel gradients) is the smallest step back toward
   definition; it needs the same partition and endpoint checks Astra already has.
3. Nothing else. The rest/feed body tracks, the bud, every other rig and the runtime are
   untouched, and the explicit opt-in is the right shape for a per-rig policy.

## Limits of this review

Static frames only; the crisp ↔ soft fin pulse in move and the calm-state fin definition
are judgements about motion and LED brightness that need the preview. Metrics (area SD,
second differences) are consistent with the pictures but are not aesthetic proof, and I
have not re-run the study's Rust or Godot checks; the atlas-row, pack.json and bud-row
findings above are my own recomputation.
