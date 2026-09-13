---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Lanternjaw brief review: concrete geometry regressions

Astra review of the **in-progress, uncommitted**
[Fable brief](living-world-next-brief-2026-09-13.md),
[multipart API](../../crates/cubarium-render/src/multipart.rs),
[sprite additions](../../crates/cubarium-render/src/sprite.rs), and
[Lanternjaw API](../../crates/cubarium/src/lanternjaw.rs).
At initial inspection, `carry_offset`, `stamp_part`, `parts` and `draw` had `todo!()`
bodies. **During review Fable replaced the carried-part API with root-owned
`RigPart` / `stamp_rig`, same-material layers and composed state blends. Sections
1–5 therefore preserve counterexamples to the superseded draft, not outstanding
demands to repair obsolete functions. Section 6 reviews the replacement.** The
brief still described carried parts when last read; synchronize it before worker
test authoring. These findings concern specified behavior, not a claim that
finished runtime images have already failed tests. They do not promote Astra's
algorithm to canon. Preserve the authored silhouette and useful implementation.

## 1. Carried anchors do not establish shared hull ownership

This numerical case was checked against the repository's compiled surface
`travel` and `unfold` helpers, without changing repository source:

| Quantity | Value |
| --- | --- |
| Root | Front `(63, 1)`, heading `(1, 0)` |
| Second part offset | Body-local `(3, 0)` |
| Carried second anchor | Right `(2, 1)`, heading `(1, 0)` |
| Destination pixel | Top `(63, 63)`, center `(63.5, 63.5)` |
| Root-owned image | Front chart `(63.5, -0.5)`, via Front→Top |
| Second-part-owned image | Right chart `(0.5, -0.5)`, via Right→Top |
| Inferred body material coordinate from root | `(0.5, -1.5)` |
| Inferred body material coordinate from second part | `(1.5, -1.5)` |

The same destination reads two DIFFERENT body material positions. This is not
floating-point noise or a tie: the reported distances are approximately 1.58114
and 2.12132 from their respective anchors. Each helper is individually correct.

Concrete rendering fixture: two 4×4 sprites, both pivot `(2,2)`. The root part
paints only texel `(2,0)`; the part at offset `(3,0)` paints only texel `(0,0)`.
These are distinct neighboring body texels centered at `(0.5,-1.5)` and
`(1.5,-1.5)`, comfortably within nine-pixel part bounds. At the named Top pixel,
both stamps sample their painted texel centers with full weight. If each texel
has white premultiplied RGBA `(0.5,0.5,0.5,0.5)`, source-over gives 0.75 white
light on black, rather than a single 0.5 contribution. With different colors,
painting order conceals one distinct material texel beneath the other.

Therefore "no pixel brighter than 1" cannot prove the brief's "no duplicated
pixels / no doubled hull": normal source-over remains bounded even when material
has been painted twice. This fixture should be retained as a regression against
accidental overlap at a material partition, plus its symmetric gap cases and
approaches from all incident faces. A test that accepts 0.75 simply because it is
below 1 does not exercise ownership.

Required correction: define body-wide ownership for joined hull material and
test it with marked partitions. One shared root-owned chart is a straightforward
solution; another projection can be valid if it supplies an equally explicit
ownership/joint contract and passes the observable tests. Independently bent
appendages can intentionally overlap in depth, but that is different from a
duplicate hull texel. The existing surface contract already permits a localized
vertex cut; it does not make unrelated per-part cuts equivalent to one body cut.

## 2. The rim skip rule contradicts flat-crop registration

The brief requires both "a re-pivot past the budget skips that part" and exact
agreement with a translated flat body on every surviving row. They cannot both
hold for all admitted parts. A concrete fixture even satisfies the proposed
Lanternjaw `PART_EXTENT_MAX = 8`:

- Root Front `(32,60)`, heading `(0,1)`, part offset `(6,0)`, scale 1.
- Sprite dimensions 14×4, pivot `(7,2)`, two opaque texels at `(0,1)` and `(13,1)`.
  Their material centers are `(-6.5,-0.5)` and `(6.5,-0.5)`; original extent is
  approximately 7.726 pixels under `Sprite`'s current support calculation.
- Intended part pivot is chart `(32,66)`. The rear texel lands at physical center
  `(32.5,59.5)`: Front pixel `(32,59)` is visibly on the surface. The front texel
  is past the open rim and should disappear by clipping.
- `carry_offset`'s documented inset produces anchor `(32,63.999)` and
  `short=2.001`. Re-pivoting to `(4.999,2)` puts the invisible front texel at local
  `(8.501,-0.5)`, giving an extent about 9.723. `with_pivot` rejects the whole
  sprite, so the perfectly visible rear pixel vanishes too.

Test against the same sprite/root at Front `(32,30)` shifted down 30 rows, as the
brief already requests. The near-rim image must retain pixel `(32,59)`; do not
replace this assertion with an expectation that the sprite vanishes. Sweep the
root across the rejection boundary: the current contract would pop the whole
part off while some of it remains visible.

Required correction: clip visible material without rejecting it because of
off-surface support. A body-chart query naturally does this. A revised carried
anchor implementation may instead partition/restrict support correctly; it must
not shrink, reflect or silently omit surviving art. Keep the existing nine-pixel
local part limit; the actual surface query limit is 32, despite the multipart
module comment incorrectly claiming its proof stops at nine.

## 3. Re-pivot units and inactive pose endpoints

Two further API-level regressions follow directly from `stamp_part`'s doc:

- **Scale:** `short` is chart distance, while `Sprite::pivot` is unscaled sprite
  distance. The documented pivot subtraction is missing `/ scale`. At scale 0.5
  the intended shift is twice the stated one; in the fixture above the art lands
  1.0005 chart pixels too far inward. Test rim registration at scales 0.5 and 1,
  using a smaller painted fixture whose re-pivot is otherwise accepted. The
  correction must also validate physical scaled support consistently; `with_pivot`
  rejecting unscaled extent can be unnecessarily strict for a scaled-down part.
- **Inactive endpoint:** the contract re-pivots BOTH sprites and skips if either
  fails. But `Pose` at mix 0 samples only `first`; at mix 1 only `second` matters.
  Use the rim fixture with `first` painting only the rear texel and `second`
  painting both. At mix 0 the visible first sprite fits the re-pivot budget; the
  unused second should not suppress it. Test exact endpoint identity against
  `Pose::still(first)` and the symmetric mix-1 case. Preserve this property even
  if the rim implementation is replaced wholesale.

## 4. Painting order is necessary, not sufficient

Far arm behind hull / near arm in front is useful and should remain. It does not
solve coverage of a single material cut across two sprite buffers, even on a
flat face. At a half-pixel translation across a partition, two neighboring
opaque texels can each contribute 0.5 filtered coverage to the same destination.
They should reconstruct coverage 1 for the continuous material. Compositing the
two part samples source-over yields only 0.75. Source-local bilinear splatting
followed by destination bilinear sampling also needs visual review for excessive
softening; "fractional" alone does not establish a crisp silhouette.

Required tests: compare split versus unsplit opaque and translucent flat patches
at fractional anchors and diagonal headings. Separately check true depth
overlaps. For `Lanternjaw::draw(opacity)`, clarify whether opacity belongs to the
whole animal or independently to each part: two overlapping opaque parts drawn
at opacity 0.5 yield 0.75 coverage, whereas fading their assembled opaque body
yields 0.5. Do not accidentally build fade-dependent ridges into the hull.

## 5. Repair the orientation test, not correct transport

The brief asks heading `(-1,0)` to mirror `(1,0)` about the anchor column. Existing
stamp coordinates use `side=(-h.y,h.x)`: reversing heading negates BOTH axes, a
180-degree rotation, not a horizontal reflection. Lanternjaw's dorsal lanterns
and ventral limbs are asymmetric, so the proposed mirror-only test would reject
correct geometry. Test a 180-degree rotation around the anchor, and test seam
transport using the shim-derived tangent maps. A distinct visual flip convention
would be an explicit new choice, not a bug fix to `stamp_part`.

## 6. Replacement root-owned API: readiness checks

The new `RigPart` / `stamp_rig` contract uses one root query, sums same-material
parts, depth-composites each state, blends completed states and applies opacity
once. This structurally addresses the prior draft's chart mismatch, rim
re-pivot/scale failure and per-part fade ridges. Its bodies were still `todo!()`
when read; do not call those properties delivered yet. Keep the fixtures above
as observable regression tests under the new public contract.

Three adjustments are still needed in the replacement documentation/tests:

1. **Complete support contradicts unconditional legacy identity.** The new
   `RIG_MARGIN=0.5` is conservative for rigid parts: the missing radial support
   from the legacy extent is at most
   `sqrt(2) - (1/sqrt(2) + 0.5) ≈ 0.207107`. But this means a one-part rig can
   correctly paint a filter tail that `stamp_sprite` previously clipped.
   Exact fixture: 16×16 sprite, pivot `(8,8)`, only texel `(12,12)` opaque white;
   root Front `(32.05,32.05)`, heading `(1,0)`. Front destination `(37,37)` has
   body coordinate `(5.45,5.45)` and distance about 7.70746. Legacy extent is
   about 7.57107, so the ordinary stamp excludes it; bilinear sampling there is
   white 0.0025, which the rig's larger valid query includes. All of this material's
   true support still fits nine pixels. Do not demand bit identity for this case
   and then "fix" the rig by reintroducing clipping. Compare to a generous legal
   query/sample oracle; require legacy identity only where its radius covers
   the actual nonzero support, or explicitly document corrected tail coverage.
2. **Do not release-clamp an invalid group radius.** The new comment specifies
   `debug_assert!` above 32 but `min(radius,32)` in release. That hides a
   configuration error by silently dropping potentially visible material, contrary
   to the surface API's explicit bound rejection. Define a consistent error/panic
   contract or validate a reusable rig configuration before drawing; do not rely
   only on test coverage of current art. Check finite offsets and radii for active
   parts/states. Zero-weight states must neither fail validation nor enlarge the
   active query. The generous-reference test helper must also reject >32, not
   silently clamp its supposed oracle.
3. **Query margin is not local part validation.** Expanding the group query fixes
   enumeration, but `Sprite::extent() <= 9` alone does not prove the true bilinear
   support of EVERY generic sprite stays within the nine-pixel material bound.
   Lanternjaw's proposed `PART_EXTENT_MAX=8` leaves ample slack (8+0.207107 <9)
   for the current rigid-part design. Retain that verified guard over every mode
   and interpolated extremum, and either validate full support at generic rig
   admission or precisely scope the stronger material claim to validated assets.
   Do not mistake a bigger root query for a proof of smaller per-material support.

The layer sum reconstructs split material only if the asset authoring actually
partitions one shared sampling lattice without duplicating source coverage. A
clamp of summed alpha/color keeps values bounded but does not prove correct
partitioning; the split-versus-unsplit and marked-vertex tests remain necessary.

## Retain and proceed

Keep the six semantic parts, code-native template, fractional motion, eased feet,
premultiplied constructors, explicit true-depth ordering, isolated study route,
compatibility guard against adding a default form, native captures and cost
measurements. None of the findings requires scrapping Fable's silhouette or the
authored plant-growth work. Finish geometry contracts and discriminating tests
before treating green tests generated from the initial docs as acceptance.

Validation for this review: read both source/API drafts and the brief; ran a small Rust diagnostic
against the existing compiled `cubarium_surface` library to obtain the table in
section 1. No production files edited and no large suites run during worker
ownership. The diagnostic binary is `/tmp/astra-lanternjaw-chart-probe`; its exact
inputs and observed coordinates are recorded above. Rim failures are deductions
from the documented algorithm and actual `with_pivot`, not fabricated test-run
results. Both API drafts were still incomplete when reviewed.
