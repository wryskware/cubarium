---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Vinecoil wind: preserve the weave, retile the endpoint

**Promising bounded implementation candidate, not a production switch.** Clearing
the vine tile's ends alone is rejected. A four-row endpoint retile with retained
chart ownership preserves the existing quiet pixels and allows a vined spire's
already-authored0.9px wind response. Recommend Fable's independent temporal review,
then hardened integration and actual-world cadence checks before rollout.

Source is frozen **9cf0e1d** in `captures/build-cache/vine-wind-frozen`, with only
the [saved study patch](../../art/studies/vine-wind/renderer.patch) applied there.
Main production art, renderer, core, schema13 experiments, live cube and release
worktrees are untouched. The [helper/reproduction instructions](../../art/studies/vine-wind/README.md)
and [measured evidence](../../art/studies/vine-wind/evidence.json) are committed;
large outputs stay under `captures/`. Lore/Graft retrieval, canon, `art/README.md`
Wind, the spire-wind study and its independent review informed this work; no new
canonical decision or biological behavior is proposed.

## Why direct strip opt-in fails

`trunk_strip` controls **host** four-row strips. The vine branch does not use it:
odd tiles1,3,5,7 own local heights4..12; tile9 owns4..16. A periodic four-pixel
weave makes rows15 and the repeated lower material redundant, but **row0 of tile9
is visible above the host** near full growth. Clearing0/15 loses this endpoint.
The direct clear-only fixture changes40 quiet images in the1,460-case sweep.

Godot removes exactly96 atlas pixels, only tall-atlas rows96 and111 (vine source
rows0/15 across24frames). All other tall pixels and every other pack file remain
byte-exact. Thus this failure is not unrelated bake drift. The copied source
image is changed only in the study; no SVG or production atlas was edited.

## One alternative, with its necessary ownership correction

The candidate retains eight-row strips through tile9, now always ending at local
height12: together they own global material through height40. A derived endpoint
uses the identical vine clip's rows4..7 at tile10, owning global heights40..44.
The weave's four-row period and clip clock are unchanged. `Mask::Axial` retains
the original grown ceiling without introducing a new strip-start fade at40.
Every piece uses the same amplitude and global `D(H)` as its host; there is no
vine-specific phase, sliding, requested wind increase or extra height.

**Independently unfolding the new endpoint anchor is wrong near a vertex.**
Front column cx0 at height8.875 changes Top(0,62): the full-column quiet mismatch
is0.374 linear in one channel. The saved isolated endpoint regression shows
tile9-owned RGB `[0,0,0]` versus independently tile10-owned
`[0.2226006,0.006143244,0.21707682]`. This is an ownership disagreement, not a
rounding tolerance to relax.

The refined retile therefore retains tile9's original owning chart while moving
the **support center** to the valid tile10 anchor. The surface helper queries a
superset about the owner and the sampler rejects every point outside its normal
radius, capped at9, about the support center in that chart. The query radius may
exceed9; the stamp's footprint does not. Surface queries already support32.
Independent physical-distance checks also confirm every painted endpoint pixel
lies within9 of the new center, rather than relying only on chart coordinates.
No tile11/off-face anchor is used.

This costs a small explicit renderer facility; it is **not** free reuse of the
existing strip flag. The study borrows `vine.cap` to carry its derived endpoint.
A real integration should name/cache the endpoint separately, validate the
required four-row periodicity and shared clock, retain old-pack behavior, and
harden the specialized helper's inputs. Do not install the study atlas by itself
or infer that arbitrary top-transparent custom vines satisfy this contract.

## Native evidence and measured tradeoffs

`captures/vine-wind-retile-2026-09-13/viewer.html` shows original/new at1× and3×,
with20s/1,200-frame full/growth tracks for interior Front cx10, vertex Front cx0,
and Right cx15. The interior is an actual spire+vine slot; edge spires are explicit
diagnostic fixtures, not claims about the current world's vegetation. Full height
crosses onto Top. Growth is a controlled linear-height sweep, not simulated
biology. Optional `*-pair-native60.mp4` files place the two native nets side by
side; the PNG frames are the authoritative evidence.

The characteristic magenta/teal helix, mint crown and anchored root remain
recognizable. The interior's additional lean is modest at64px but visible in the
native peak pair; it is clearer enlarged. Vertex/right motion remains faint,
as the unchanged wind field intends. Inspected stills show no detached root,
lost endpoint or new halo. This is not a claim that still screenshots alone
establish smooth playback or that the change was viewed on physical panels.

| Full-height20s metric | Original | Retiled |
|---|---:|---:|
| Vine support budget |0.46510|3.54495|
| Vined spire effective desired tip |0.42282|0.90000|
| Actual sampled interior maximum amplitude |0.37691|0.80229|
| Interior mean linear-light sum |40.14830|40.59688|
| Interior maximum pixel luma |0.65291|0.65291|
| Interior mean pixels with luma>0.01 |247.214|246.434|
| Interior mean adjacent-frame RGB L1 |0.89269|0.91323|
| Interior worst adjacent-frame RGB L1 |1.25405|1.29868|

The mean-light increase is1.12%, with identical peak light and slightly smaller
thresholded area: a resampling/compositing consequence of greater displacement,
not brightness compensation. More motion also increases temporal RGB changes;
the maximum rises3.56%, not an antialiasing improvement claim. Glasscane remains
limited by its own unchanged0.465px budget; this study does not make it sway more.

For exact temporal inspection, use both variants' frames **338–342** around the
interior displacement peak340 (35.667s), and **796–800** around their shared
worst RGB-change boundary798 (43.300s). Vertex peak317/worst898; Right
peak317/worst629. Those are finite-frame metric maxima, not human quality scores.
Each measured full-height variant includes120 truly zero-wind frames with exact
original quiet pixels.

Five1,000-draw rounds measured isolated full-column rendering including scratch/
canvas allocation but excluding ecology, encoding and browser. Final median
interior cost0.13810→0.14283ms; vertex0.17789→0.17433ms;
Right0.17595→0.17238ms. An earlier noisier run measured0.14417→0.16365ms
interior. Do not extrapolate the small negative differences into a speedup or
claim the whole16.7ms frame budget passed. Local preloaded-PNG Chromium playback
measured60.003fps over6.016s with362 unique frames; this is a viewer check only.

## Acceptance evidence and remaining integration gates

Four release-mode study tests pass, no ignored tests. They retain both rejection
proofs and exercise:

- 1,460 exact quiet images plus2,920 equal-amplitude old/new comparisons,
  and180 exact colored-background comparisons.
- 21,120 quiet/held comparisons across all64 side slots and both host species,
  including near-zero and integer/endpoint growth epsilon cases and clip boundaries.
- 1,024 rooted-row identities and1,024 authored three-second loop comparisons.
- 4,320 actual-frame larger-query clipping comparisons across faces, corner/interior
  positions, subpixel coordinates, both bend signs and low/high placements.
- 48,480 nonzero endpoint destinations checked against a separately requested
  physical9px surface neighborhood. Finite RGB is asserted along that sweep.

These are bounded sweeps plus the row-ownership argument, not an exhaustive proof
for every real coordinate or arbitrary custom rig. All actual candidate poses are
included in the support checks; derived endpoint support participates in the
same column budget as the host and vine trunk. The smaller query is compared to
a valid12px oracle, not a silently rejected over-limit stamp.

Next: independently review the native temporal neighborhoods and chart-preserving
patch; if approved, make one explicit pack/renderer integration with old/custom
pack validation, then render a copied actual
world with typical/high column populations and check cadence. No production
switch is authorized by this study's test count alone.

## Assertion correction and preferred integration representation

Follow-up source review found that the study's supplemental headroom assertion
passed `(base, root, length)` to an API taking `(root, length, base)`. That made
this one assertion vacuous (zero length returns infinite headroom). The actual
4,320 larger-query comparisons constructed `Bend` with named fields correctly;
their clipping evidence was not affected. The assertion now uses the correct
argument order. All four tests pass again, unchanged in scope and tolerances,
in6.88s; log `captures/vine-wind-headroom-correction-tests.log`. Original evidence
is retained rather than retroactively claiming that assertion was useful.

Root proposed a better **production representation**, which I recommend over
shipping the study's clear-ended atlas: retain the original fully periodic
`TallPlant.trunk` and atlas bytes; put explicit additive metadata on the vine
trunk row; derive/cache `VineStrips { trunk, endpoint }` in a separate optional
field at load. This derives exactly the same pixels as the study while old
loaders ignore the additive field and still see the complete original endpoint.
The existing loader reads individual fields from `serde_json::Value`, so this
compatibility is supported by source; an exact old-binary loading test should
still accompany integration. Keep pack version5 for an optional enhancement.

Suggested selector: `"vine_strips": "period4_endpoint_v1"`, absent by default.
Reject malformed/unknown selectors or a flagged base/crown-bearing plant, rather
than silently falling back from an invalid opt-in. Validate all16×16/pivot(8,8)
frames, finite positive loop duration, actual looping, and exact four-row
periodicity over **all16 rows**, not the existing permissive endpoint test.
Compare premultiplied texels so invisible RGB is irrelevant. Copy the original
sample count, duration and loop flag into both derived clips; clear only0/15 in
the derived trunk and retain only4..7 in the endpoint. Unflagged old/custom packs
remain on the exact legacy path. Limit activation to the supported vine role;
do not imply that arbitrary other tall entries are routed as climbers.

For an opt-in vine, its budget must be the minimum of the **derived** trunk at
tile9 and endpoint at tile10, replacing—not additionally including—the raw
trunk budget. Otherwise the retained raw0.465px limit defeats the enhancement.
Do not change host strip heuristics to recognize the now-intact raw vine trunk:
the vine renderer must explicitly select the cached derived pair. The tested
Axial growth ceiling, tile9 chart owner/tile10 support center, compositing order
and one shared bend remain necessary. Atlas/representation changes alone do not
remove the proven vertex ownership failure.
