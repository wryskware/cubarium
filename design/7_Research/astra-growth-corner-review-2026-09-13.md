---
design_status: exploration
last_reviewed: 2026-09-13
---

# Growth-corner pop: isolated diagnosis and retained-owner candidate

The reported pop is real, pre-existing **moving crown chart ownership**, not vine
retiling, discrete growth, a baked-frame change, or a wind-budget change. A bounded
study candidate removes it while preserving lower growth and the mature endpoint.
Recommend independent visual review before any production integration. No production
source, pack, schema, ecology or live process changed in this task.

## Exact cause, including a real selected column

[Fable's vine review](fable-vine-wind-review-2026-09-13.md) reports the forced
Front0/Right15 spire columns' frame1177→1178 pop. Those two columns are **not selected**
by the real hash. But selected **Left0 glasscane+vine** exhibits the same discontinuity.
The study compiles the actual private `draw_column`, extracted without source edits;
its cap path matches `a9eb064` exactly.

For Front0 the crown center is `(2,v)`, with `v=42−4(height+1)`. The affected Top
pixel `(0,60)` has center `(0.5,60.5)`. Both valid images lie inside the cap budget:

| Owning path | Target image in Front | Distance at frame1177 /1178 |
| --- | --- | --- |
| Front→Left→Top | `(-3.5,-0.5)` |6.35815 /6.34315 |
| Front→Top | `(0.5,-3.5)` |6.36915 /6.34000 |

Their squared-distance difference is `16−6v`; the selected image switches at
`v=8/3`, i.e. **height53/6**. Heights8.8275 and8.835 straddle this. A tiny step does
not make the two material coordinates approach one another.

At fixed clip time49.625, zero wind and height53/6±1e−8, maximum linear-channel
changes in the **whole column** are:

| Column | Original | Isolated final-owner candidate |
| --- | ---: | ---: |
| Forced Front0 spire |0.407226 |5.96e−8 |
| Forced Right15 spire |0.644894 |1.79e−7 |
| Selected Left0 glasscane+vine |0.190272 |5.96e−8 |

Left0's Top `(3,0)` goes from black to `[19,69,121]` RGB8. Removing only its cap
eliminates the discontinuity; retaining/removing the new vine opt-in does not change
either image. Decreasing epsilon from1e−4 to1e−8 does not remove the original jump.
These tests remain deliberately red-case assertions, not claims production is fixed.

`surface-topology.md` explicitly described localized shortest-owner discontinuity at
a curvature singularity. That is relevant design history/current implementation,
not a newly promoted canonical rule. We do not change generic shortest ownership,
blend competing charts, add a second contribution, or enlarge sprite support.

## One local mechanism, with a rejected owner placement preserved

The study applies a retained owning chart **only to near-corner host caps** (`cx`
0,1,14,15), not trunks/vines/bases. The actual cap center, heading, pose, opacity,
bend, growth and layer order remain unchanged.

- Through height7 the ordinary renderer is used.
- Above height7 the cap center travels from face-local `v<10` to `v=2`, while its
  owner is held at the same column's final cap position `(u,2)`.
- `stamp_pose_in_chart` provides the existing single-owner compositing path.
  The owner-center distance is at most8; query radius is at most17, below32.
  Sampling still has the hard9px footprint about the actual center.
- The handoff is deliberately made before visible cap support needs the corner.
  Owner coordinates do jump, but **sampled material does not** for the tested shipped
  caps. Tests cover all authored frames and midframes, all four side faces, all four
  near-corner slots, both host families, and both signs of their full family budgets.
- At mature height9, owner and center coincide, restoring the original endpoint
  exactly in the captured poses. There is no cache/history, so holds, rewind and
  growth reversal do not require an episode-dependent decision.

The first placement retained `(u,10)` instead. It passed continuity/support but
removed a visible mature Top lobe: selected Left0 cap RGB-sum light fell from22.49325
to18.27024 (18.77%). **Rejected as a visual tradeoff.** The final-position owner
restores22.49325 with zero measured endpoint difference. The first trial's numbers
are retained in the committed `evidence.json`; its enlarged image remains
`captures/growth-corner-2026-09-13/rejected-v10-owner-endpoint-8x.png`.

## Evidence and limits

[Study source/tests/viewer](../../art/studies/growth-corner/README.md), including
committed numeric evidence. Latest focused run: **6 passed,0 failed**,37.94s;
`captures/growth-corner-final-owner-tests.log`. No host suite or live test claimed.

Tests cover causal controls, fixed-time determinism, exact lower-growth equality,
handoff/identified-boundary continuity, endpoint preservation, and nonzero painted
destinations within physical9px at all corners/both families/representative growth
and phase samples/±full family wind budgets. Physical-distance checks use the
production surface helper: consistency verification, not a separate geometry proof.
The dense handoff checks use every authored frame **and** each adjacent midpoint;
they are finite fixtures, not a universal proof for arbitrary custom artwork.

Native pair sequence: `captures/growth-corner-2026-09-13/selected-left-60fps/`.
Its241 frames are a synthetic linear height7→9 sweep over4s at fixed art phase and
zero wind; **not an observed biological trajectory**. Whole-frame maximum change
falls from0.453371 to0.027000 linear. The old worst frame is180, height8.5, another
ownership switch (Top `(1,0)`, black→179 blue); the candidate's worst is ordinary
continuous growth at height8. This is not a claim every frame improves or every
animation artifact disappears.

Inspected native full nets and native/enlarged contact sheets. The final candidate
keeps the recognizable mature crown and improves the diagnostic intermediate Top
projection without softening/brightening source art. The viewer provides native and
3× playback, stops at maturity and holds exact frames when scrubbed. No physical
panel viewing or measured browser/live cadence is claimed. Per-frame CPU cost of
the larger query remains unmeasured; profile it during any integration review.

Production follow-up should retain the original failure fixture and add an explicit
shipped-cap capability/opt-in boundary, rather than silently applying this to custom
caps by family name. Validate custom geometry/clock/coverage or keep its legacy path.
Keep `draw_column` private, transplant only the local owner choice, and rerun full
column wind/growth/root/quiet tests plus actual-world visual/cadence review. In
particular, inspect the crown-to-vine projection during growth around the corner;
these isolated captures are not an ecological/world-density acceptance test.

Provenance: presenter SHA256
`1dd719f0ec8838c6891aa77aad6eebd55659d97241d3f76ba6b7c7ec08881f52`;
unchanged tall atlas
`b911d16a777e12713536ec1134268109b40400ed31b2f5f7dc00efc6ab16bfa7`.
`git diff a9eb064 -- art_present.rs art.rs pack.json` (full repository paths) was
empty at capture time. Main's unrelated concurrent changes were not staged.
