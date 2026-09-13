---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Spiretree wind room: independent rollout review

Reviewed exact **1236205**, its parent diff, the frozen source at
`/tmp/cubarium-spire-rollout-1236205`, Fable's
[report](spiretree-wind-room-2026-09-13.md), `captures/spiretree-wind.png`, and
native 256×128 frames from `/tmp/cubarium-spire-6SzpR7`. No moving meal-prototype
source was used. **No concrete shipped-art/geometry blocker found.** Root's clean
workspace tests, release build and runtime cadence check remain the rollout gates;
this review started no build, simulation, bake or live operation.

## Visual and asset evidence

The centered two-pixel tip gives the spiretree a clean pointed dome without losing
its mint/cyan character or widening the silhouette. In the enlarged sheet and native
calm/8-second bare/vined frames, the bare crown changes visibly but remains restrained;
the stem reads as joined, with no obvious detached root or bright seam. Vined motion
is intentionally smaller. These are screen captures, not room-distance or physical
LED validation, and still images do not alone prove temporal continuity.

Independent RGBA comparison of the previous committed atlas with the frozen new
atlas found exactly **1128 changed pixels** in the 384×112 atlas: 192 in the spiretree
trunk row and 936 in its crown row. Spiretree base and all glasscane/vine rows are
pixel-identical. Only `tall.png` changed under `assets/atelier`; pack metadata, small
plants and their authored growth atlases are unchanged. All 24 frames of each tall
part share one alpha silhouette. I did not independently rerun the reported Godot
reproducibility bakes.

## Geometry, growth and wind scope

`trunk_strip` uses **all** trunk frames' top-row transparency to opt in; it does not
depend on a species-name exception or a changing sampled frame. Thus the shipped
spiretree selects rows 1–4, while the old pack, glasscane and vine retain rows 0–3.
No periodic sampling-boundary switch is introduced. The rule is recomputed during
column drawing, not actually cached once at pack load as some prose might imply.

With the existing global height coordinate, the shifted first strip owns heights
5..11, the next 11..15, then 15..19, etc. Adjacent intervals still meet exactly,
and each uses the same `grown` ceiling. Four-row periodicity makes their interior
pattern agree despite moving which tile owns a row. Base, first-strip floor,
height pacing, cap index `height+1`, stamp heading/anchor and the shared bend curve
are unchanged. The spiretree's newest-row start envelope changes slightly because
strip boundaries move by one row; this is **not** a bit-identical old spiretree
growth claim. Other families keep their previous boundaries and assets.

The base at/below the horizon remains on the zero-displacement part of the same
rooted curve. All pieces retain a common physical height-dependent displacement;
growing a segment does not change bend length or retarget the root. Existing
surface queries still carry stamps across the side/top seam; no independent crown
anchor transport or enlarged footprint was added. The native crown spans Front/Top
cleanly in the examined full-height capture.

The loader still derives the cap from same-index crown/trunk frames; independently
reading the changed pixels gives **tail row 8**, leaving the dome without a second
blurred copy of its trunk. Clearing crown row 15 matters: otherwise the now-clear
trunk would no longer subtract that crown-tail row.

The increased budget is earned by changed painted support, not shrinking the whole
plant, raising the 9-pixel footprint, or silently dropping an over-budget stamp.
Recorded pack budgets are spiretree 1.314 and vine 0.465; bare desired tip remains
0.9 and vined columns take the existing minimum. A conservative support calculation
over all 24 actual base/trunk/derived-cap frames, using the renderer's published
one-pixel bilinear support and maximum varied amplitude 0.9×1.1=0.99, gave worst
radii **8.8601 / 8.2620 / 8.8573**, respectively, below 9. This validates the
source-contract bound for these pixels; it is not a new independent topology oracle.
The global packet, quiet intervals, phase, variation and wind field are untouched.

The extra strip decision is bounded (at most 24×16 alpha reads per drawn spiretree
column in this pack) and adds no allocation or extra segment limit. I measured no
wall-clock render cost here; do not promote the README's identity-path statement
into proof that this complete presenter now costs exactly the same as before.

## Test/report qualifications and follow-ups

- The budget minimum examines every frame. The larger-radius drawing sweep named
  `every_shipped_plant_and_column_frame_draws_completely_at_its_own_budget` actually
  uses stride 6, i.e. four samples per 24-frame clip, at an interior, side seam and
  top vertex. Its source is stronger than only a brightness bound, but it is not
  literally every-frame rendering. Identical tall alpha silhouettes reduce the
  concern for this exact change; no new general all-pose proof is claimed.
- Shifted strip ownership/stripe checks are primarily hand-built geometry tests;
  the row-15 test also exercises the actual presenter through growth. Fable reports
  22 wind tests and the host suite passing. I read their scope but did not duplicate
  root's clean build/tests or count them as independently rerun here.
- **Row 0 is not universally never drawn:** the ninth segment still owns it, and
  the new loud-row test explicitly observes it there. The shipped cap joins that
  endpoint, but documentation saying both end rows are never drawn should retain
  this exception. `art/PLANTS.md` also needs its old all-16-row periodicity sentence
  qualified to painted rows 1–14 and end-row rules.
- Clearing vine ends is not an automatic next application of `trunk_strip`: vine
  overlays use their own odd-index `TALL_VINE_FLOOR/TOP` branch. Any future vine
  edit needs its own endpoint, coverage and host-sharing checks before rebaking.
- `author_spire_wind.py` records a one-time transform, not a portable bake command:
  it hard-codes this workspace's art directory and refuses already-applied input.
  Do not run it from an isolated worktree expecting that worktree to be edited.
  Its blanket unchanged-pixels/end-row prose has the same ninth-segment and growth
  qualifications above. Checked-in SVGs remain the source of truth.

These qualifications do not identify a current rollout defect. Preserve the chosen
visual direction, validate the exact clean candidate and its shipped pack together,
and judge the modest wind gain on the cube after the ordinary rollout checks.
