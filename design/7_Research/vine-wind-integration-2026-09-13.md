---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Vinecoil wind integration: intact atlas, explicit derived pieces

Implemented the bounded integration supported by the
[Astra study](astra-vine-wind-study-2026-09-13.md) and
[Fable review](fable-vine-wind-review-2026-09-13.md). This report is implementation
evidence, not a live rollout or a new biological decision. The candidate preserves
the existing vine weave and allows vined spiretrees their already-authored wind
response. Independent visual review found a modest native64 improvement; all
physical-panel and actual-world cadence claims remain outside this report.

## Delivered behavior

- The baker adds `"vine_strips": "period4_endpoint_v1"` only to the vinecoil trunk
  row in `pack.json`. **Every atlas/source pixel is unchanged.** Pack version remains5.
- The loader validates the ORIGINAL 16×16/pivot(8,8) clip: every frame's full16-row
  exact four-row period, valid sample count, finite positive duration and looping
  semantics. A flagged wrong family/part, base/crown-bearing vine, malformed or
  unknown selector, contradictory loop, bad clock or pixel drift is an error.
  Validation happens before rows are cleared. Unflagged custom art keeps its
  original, more permissive behavior.
- `TallPlant.trunk` stays intact. Separate cached `VineStrips { trunk, endpoint }`
  holds rows1..14 and rows4..7 respectively, with exact inherited clocks. The
  endpoint is never stored in or interpreted as `cap`.
- Only the explicit vine path uses those pieces. Its derived trunk keeps the old
  eight-row strip layout through tile9, ending at local height12; its endpoint at
  tile10 owns global material40..44 with the original Axial grown ceiling.
  Tile9 remains the chart owner; tile10 is the bounded support center. The root's
  hardened `stamp_pose_in_chart` provides this facility; this package does not edit
  the renderer helper. It requires helper commit `33d1119` (independent renderer
  review/tests `1d648bf`). `draw_column` remains private.
- The derived trunk and endpoint **replace**, rather than augment, the raw-trunk
  wind-budget measurement. Vine room becomes3.54495px instead of0.46510px; a vined
  spire's minimum becomes its own1.31417px, admitting its existing desired0.9px.
  Glasscane remains restricted by its own unchanged budget. No wind field, gust,
  phase, palette, ecology, state schema or user-input behavior is changed.

The endpoint query is skipped only while its entire masked bilinear support is
empty (`reveal <= 7`); this does not introduce a new growth fade. Quiet rendering,
root registration and equal-amplitude output are compared directly against the
legacy path, including the vertex that defeated independent endpoint unfolding.

## Validation

Seven new production-path tests pass. Four loader tests cover current metadata,
raw/derived texels and inherited clocks; null/boolean/number/object/unknown flags;
wrong family/part and base/crown-bearing vines; first/last-frame drift at original
rows0/5/15; explicit looping and invalid duration/sample/layout cases. Direct
derivation tests also exercise nonfinite clocks, non-looping clips, empty/one-frame
clips, wrong tile dimensions and wrong pivot. Unflagged drifted custom clips
remain accepted rather than acquiring the opt-in's restrictions.

Three renderer/presenter tests port the reviewed study to the real cached pair:
1,460 exact quiet images; 2,920 equal-amplitude comparisons; 180 exact colored-
background comparisons; 21,120 quiet/held cases across all64 side slots and both
hosts; 1,024 rooted-row and1,024 authored-loop checks; 4,320 correctly parameterized
larger-query clipping comparisons; and48,480 nonzero endpoint destinations checked
against the physical9px neighborhood. The expected new budget is asserted
separately from equal-amplitude parity. These bounded sweeps are not a general
proof for arbitrary custom geometry.

Two existing `art_wind` tests initially failed because they still measured or
stamped the **unrendered raw vine trunk at the new derived-family budget**. They
now independently sweep the actual derived pieces and additionally retain the
legacy raw-trunk budget/footprint checks at its own admission. No tolerance or
coverage guard was relaxed. A synthetic `TallPlant` constructor gained only
`vine_strips: None`. The focused existing suite passes21 tests.

The first full-host run encountered25 sandbox web-listener bind failures; those
are environment restrictions, not ecology or renderer failures. It was rerun
with test-only local socket access. Final command and log:

```sh
CARGO_TARGET_DIR=captures/build-cache/vine-production cargo test -p cubarium --lib --tests --offline
# captures/vine-production-host-tests-final.log
```

Final full-host result: **543 passed, 0 failed, 16 ignored**, across35 test-target
summaries, exit0. This tests current main's host; the planned schema12 release
must still cherry-pick only the renderer
helper and this art package onto9cf0e1d and run its clean release checks. No core
or schema13 experiment is part of this package.

## Reproducible bake and exact older-reader check

`godot --headless --path art --script bake.gd -- --out=.../captures/vine-metadata-rebake-2026-09-13`
exited0. `diff -rq assets/atelier captures/vine-metadata-rebake-2026-09-13` reports
no difference, metadata included. The known sandboxed Godot user-log warning did
not prevent the bake. No generated image was installed because no image changed.

The exact installed old9cf0e1d executable loaded both the copied flagged pack and
the original unflagged pack in separate fresh PNG-only runs (seed9193,0.2s,20fps).
Both exited0 at tick4 with three submitted frames and the same population/residual.
These were new isolated compatibility state directories, not the live world.
The old loader reads individual fields from `serde_json::Value` and ignores this
additive selector. All five atlas files are byte-identical to the old pack.

**The two old-runtime PNG streams are not paired image-identity evidence:** their
fractional wall-clock sampling differs. The same-clock current-loader regression
provides image parity; successful old-executable loading and unchanged atlas bytes
establish backward compatibility without claiming the separately timed PNGs match.

```text
old binary SHA256  543ec4a39640865daf16f5ec28d77c07bdc915d8acf9cebd01f692374f2c761b
unchanged tall.png b911d16a777e12713536ec1134268109b40400ed31b2f5f7dc00efc6ab16bfa7
flagged pack.json  836888ff876369623b19f4dbeee6016a43045e9980f251d2e389cc7bf1f13ae2
```

Logs: `captures/vine-old-reader-{flagged,legacy}.log`,
`captures/vine-metadata-rebake.log`,
`captures/vine-production-focused-tests-fixed.log`,
`captures/vine-production-art-wind-tests.log`. The frozen study and its dependencies
were not changed during integration. No live process, shim, production state,
backup bundle or release cache was touched.
