---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Lanternjaw boundary continuity: independent review

Reviewed `12c41aa` using Lore, actual source/tests, the continuity report and current
canon. Tested a fresh exact `git archive 12c41aa` at
`/tmp/cubarium-astra-boundary-fFKZYY`; concurrent core-dose edits were not used or
modified. The selected Fable body and live cube remain untouched.

## Concrete finding: endpoint heading in the wrong chart

The new chord interpolation closes the former whole-interval cross-face snap for
ordinary short crossings. One public-API endpoint defect remains in this commit:

```text
last position: Front (63, 1), heading (0.6, -0.8)
settlement:    Right (0.125, 0)
unfold path:   Front → Right, identity tangent transport
f = 1 output:  Right (0.125, 0), heading (-0.8, -0.6)
expected:     Right (0.125, 0), heading (0.6, -0.8)
```

`travel` reaches the target's top edge and crosses onward into Top at the exact
endpoint. The adapter then forces the returned position to the recorded Right
point but keeps the travel result's Top-chart heading. This mixes position and
tangent charts. `Unfolded.map` maps target-chart tangents back to the starting
chart, so its inverse supplies the exact target-chart heading when returning `q`.
Return `(q, u.map.inverse().apply(p.heading))` at `f >= 1` after finding a valid
unfolding and before calling `travel`.

This is an endpoint contract defect, **not an observed visible final-frame jump**:
the current presenter stops drawing retained prey at `f = 1`. The independent
position sweep approaching the endpoint passes. Root authorized the narrow source
correction and a production regression after this evidence was preserved; a later
addendum records that validation rather than changing this historical result.

Saved independent fixture:
[`astra-lanternjaw-boundary-probes-2026-09-13.rs`](assets/astra-lanternjaw-boundary-probes-2026-09-13.rs).
It enumerates bounded points on all five charts, including exact zero edges and
near-corner points, checking endpoint-heading agreement with the selected unfolding
and near-endpoint distance. This is shared-helper consistency testing, not an
independent proof of the surface library. Against untouched `12c41aa`: **1 passed,
1 failed**, exit 101. The heading test stops at the first counterexample above.

## What is verified and what remains approximate

- Owned adapter suite on the exact archive: **28 passed, 0 failed, 3 ignored**,
  exit 0. The newly forced Front→Right, Right→Top and Back→Top fixtures do check
  actual cross-face endpoints, quarter/half-turn heading transport, chord sampling
  and rim-near monotonicity. They are stronger than the previous conditional seam
  fixture, without asserting unique image ownership from bounded brightness.
- The image fixture uses real Capture settlement evidence but deliberately moves
  the last published prey view to Front. It isolates retained-prey light with a
  twin and tests locality, face-majority progression and disappearance at `f = 1`.
  It does not claim that the forced final movement came from ecology.
- `state_at` convexly interpolates previous/current scale, staying in the admitted
  range when both endpoints are admitted. Gut steps on a new phase boundary and
  interpolates inside a phase. Gestation presence changes step; continued progress
  interpolates. All use the same previous/current frame pair without mutation.
  Synthetic tests check these values and public-presenter scale images; one real
  capture checks gut values before/at settlement. Its trailing comment mentions an
  image comparison, but the actual real-meal test performs no such image assertion.
- These are presentation histories, not exact within-tick biological trajectories.
  A real capture whose physics and subsequent growth change scale is still not
  staged. Restart still lacks retained prey and precise interrupted entry reach.
- The chord approximates the missing prey path; tangent transport cannot recover
  a final biological turn. The report incorrectly says animation time is the last
  published one: `ArtPresenter` still passes current presentation seconds when
  stamping retained prey. Mode/heading originate from the last view; animation
  continues at the current time, with no recovered fade history.

The out-of-range fallback stays finite for valid core positions and explicitly
uses the settlement point and old heading throughout. The claim that a real capture
can *never* exceed the 32-px chord limit is not established universally: claw-root
distance does not bound last-prey-to-settlement distance, and configured movement
rates are not generally bounded by the default trial's displacement. State this as
an expected/default-configuration case or provide an actual validated movement
bound; keep the fallback documented. No >32-px genuine capture was constructed in
this review, and no such ecological claim follows from its synthetic fallback test.

Viewed `captures/lanternjaw/seam-crossing.png`: the isolated prey visibly moves
across the marked Front/Right seam and disappears on settlement, preserving the
selected Lanternjaw body. The staged/forced-view label is accurate. Native room-
distance LED legibility and natural hunting success are not established.

## Reproduction and evidence scope

Archive production target:

```text
cargo test -p cubarium --test hunter_present --target-dir /tmp/cubarium-fable-cont-0l8noL/target
```

Run from `/tmp/cubarium-astra-boundary-fFKZYY`. The independent target is a new
`tests/astra_boundary_probe.rs` in that archive containing only an include of the
saved research fixture; run the same command with `--test astra_boundary_probe`.
The reused target directory is build cache only; sources come from the exact archive.
The author's **497 passed / 14 ignored** full host result was not independently
rerun here; the independently established count is the bounded adapter suite above.
