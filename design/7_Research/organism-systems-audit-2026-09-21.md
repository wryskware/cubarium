---
design_status: exploration
last_reviewed: 2026-09-21
decision_refs: []
source_revision: 954ccb4bd8000249f875bcf5e7bcefab47a10821
---

# Organism systems audit — 21 September 2026

The browser problem is a broken chain from plant production through physically
accessible food, sensory evidence, movement and recruitment. More standing foliage,
a taller picture, or a successful two-patch policy does not establish that chain.
The anatomy proposal improves the vocabulary for expressing it, but its body sizes,
feeding bands and foliage bookkeeping are not yet a consistent specification.

This is the read-only audit requested in
[`voxel-organism-audit-2026-09-21.md`](../handoffs/voxel-organism-audit-2026-09-21.md).
Only this research document was authored. No model, policy, constant, design decision
or deployment was changed. Source claims refer to the revision above; historical
measurements retain their original revisions. Organic units are **not kilograms**;
the model supplies no conversion. Seconds below are simulated seconds.

Authority: the ledger contains D-0001–D-0003, covering the vault, product brief and
working rules, not these anatomical mechanisms
(`design/0_Canon/DECISIONS.md:16–42`). The scale document reports owner approvals in
§7 but remains proposal-labelled and internally inconsistent with the anatomy
document. Following this audit's explicit instruction, neither is promoted here.
The picture should agree with simulated interactions, as required by
[`Cubarium_Art_Direction_v0.1.md`](../art-direction/Cubarium_Art_Direction_v0.1.md),
pillar 1 and §04; this audit proposes no new look. Backlog §1's rates and thresholds
are retained, not retuned.

Evidence boundary: D1 diagnosed dry soil and zero plant income; route B restored
five of six plant species at six hours, then glowcap's added litter diet restored
the sixth. D2 exposed stalled heuristic search; D3 found foliage near dying
browsers without mouth contact. Reach improved arena feeding but the historical
live arm overshot; gestation reduced that boom without ensuring persistence.
D4 then found differing visibility failures on the shipped landscapes. These are
successive, differently configured worlds, not a controlled one-variable series
(`design/7_Research/voxel-census-2026-09-20.md`, D1–D4 and route B;
decomposers, reach and reproduction handoffs' integration notes). In particular,
D4 retained **one browser at 3,600 s on small**, while default and its baseline
went extinct; “extinct within the hour on every landscape” overstates its table.
That table also predates the source revision and seeding measurements in this audit.

## 1. Units

**Proposed rule:** organism geometry and transport are authored in metres, areas in
m², time in seconds, speeds in m/s, angular geometry in radians or degrees, and
material in the model's declared organic/mineral/energy currencies. Body-relative
ratios and probabilities are dimensionless. Voxel indices, neighbour stencils and
raster pixels belong to the discretization and presentation interfaces. Converting
a physical shape into occupied cells must not redefine the animal.

Several handoff premises have already changed. Current mouth lift is converted from
the 0.25 m reference cell to the world's cell size; the arena uses 0.25 m cells and
several crown heights; seeding uses physical density and establishment checks.
These are existing implementation choices, not changes made by this audit
(`crates/cubarium-voxel-fauna/src/body.rs:576–586`,
`crates/cubarium-voxel-sim/src/arena.rs:45–96`,
`crates/cubarium/src/voxel/habitat.rs:188–263`).

| Quantity | Current representation | Remaining physical contract |
| --- | --- | --- |
| Animal length, width, cruise speed, cone range | Metres and m/s in the founder manifest | Specify adult dimensions and how juvenile mass changes geometry; distinguish structural length from the full silhouette. |
| Horizontal mouth reach | Dimensionless body-length multiplier; current browser reach 0.0625 m | Actual sampled columns remain a discrete approximation, not a continuous reach envelope. |
| Vertical mouth lift | Reference-cell count converted to a 0.25 m browser lift | Head origin and accepted crown layers still depend on cell size; converting the lift alone does not fix the whole mouth. |
| Eye and contact origins, support and body occupancy | Grid-relative offsets and cell tests | Define physical anchors, body clearance and support/drop sensing before rasterizing. |
| Plant crown and root geometry | Reference-cell dimensions rescaled by `for_voxel_size` for finer worlds | Use physical profiles as the source of truth; rounding still changes encounters. |
| Canopy attenuation | Foliage divided by crown area measured in **cell²** | Define extinction per physical foliage density; identical crowns currently shade differently at different resolutions. |
| Litter cue | Diffusion on a same-height neighbour graph | Specify physical transport, decay and source normalization, not just a fixed per-cell exchange. |
| Policy inputs/actions | Mostly normalized values and manifest-defined cadence | Keep the meaning, physical support, time interval and normalization of every channel invariant. |

Sources: `crates/cubarium-voxel-fauna/src/body.rs:576–626`,
`crates/cubarium-voxel-fauna/src/senses.rs:644–697`,
`crates/cubarium-voxel-flora/src/step.rs:340–379`; further receptor and transport
details are in §4. The light problem follows directly from
`area = max(π radius_cells², 1)`: halving the cell width doubles the radius in cells,
so it quarters optical depth at unchanged foliage and `shade_k`, wherever the area
floor is inactive. Physical plant height invariance is not ecological invariance.

There is also a coordinate error in the historical account. `Site.y` indexes the
solid support cell; its exposed upper surface is `(y+1)v`. The eye at `(y+1.5)v`
is therefore **0.0625 m above the surface on small and 0.125 m on default/wide**.
The quoted 0.1875/0.375 m elevations are offsets from the support cell's bottom.
This correction does not change D4's measured eye-to-crown angles, but it changes
what anatomy those numbers describe (`crates/cubarium-voxel/src/world.rs:186–195`,
`crates/cubarium-voxel-fauna/src/senses.rs:589,644–650`).

The proposed ladder does **not** resolve this by declaring one cell to be 0.125 m.
Its browser manifest length is 0.375 m, while anatomy §4 calls
0.75 × 0.375 × 0.375 m the model body. The corresponding shredder lengths are
0.19 m and 0.375 m. These are different animals, not rounding differences.
Anatomy also retains a one-layer occupancy and a mouth described as “up 1”.
Removing the presentation's doubling requires choosing which physical dimensions
were intended, including eye and mouth anchors; it cannot simultaneously preserve
both lengths (`design/art-direction/organism-scale-and-roster-2026-09-21.md:98–99`,
`design/organism-anatomy-2026-09-21.md:206–224`).

“The same animal” should mean the same physical encounter and expenditure within
declared discretization error, not identical world trajectories. A 0.125 m versus
0.25 m grid may produce different terrain and quantization; it should not silently
halve an eye's elevation or quarter canopy extinction. This is an interface choice,
not a proposal to tune a placeholder.

## 2. Trophic compatibility

Reach, diet permission, sensory visibility and travel access are separate tests.
A crown may pass one and fail the other three. “Edible share” below means a share of
existing foliage organic stock, not a share of stand count or projected canopy area.

The live browser already accepts foliage across species: the historical D3 arm
ate bloomcrown, umbrellafrond, springturf, velvetpad and glowcap; stonecushion was
not bitten in that arm. Thus it has one **resource class**, not one plant species.
Springturf is already a food, and glowcap tissue is currently treated as foliage
despite being fungal tissue. The shredder has one class, litter; glowcap consumes
both litter and dead wood. Carrion has decomposition but no animal feeding route.
Fallen foliage already enters litter; a separate fresh fallen-leaf food and fruit
pool are absent from this proposal's current model account
(`design/7_Research/voxel-census-2026-09-20.md:184–202`,
`design/handoffs/voxel-decomposers-and-defaults-2026-09-20.md`, S2 and integration
note; `design/art-direction/species-dossiers-2026-09-21.md`, D2 and D12).

The proposed exclusions of velvetpad and stonecushion are **diet changes**, not
consequences of drawing their layers. Removing velvetpad matters: it supplied
1.9708931 organic of D3's recorded 3.4055133 organic foliage removal, about 58%.
That is a historical arm's contribution, not a fixed nutritional requirement
(`design/organism-anatomy-2026-09-21.md:110–122`;
`design/7_Research/voxel-census-2026-09-20.md:184–188`).

### Current lollipop matrix

Let `f = wood/wood_max`. On level ground, conditional on a valid mouth probe
touching the crown, the browser can withdraw from the whole stand stock or none:
there is no spatial foliage share today. Stage names are not current model state;
the formula applies at every growth fraction. Terrain-relative height can reverse
any level-ground exclusion, so these are not global species prohibitions.

| Living stand | Crown-height parameter in metres, before rounding, relative to support-cell base | Browser on `small` (0.125 m cells) | Browser on `default` and `wide` (0.25 m cells) | Shredder, all stages/presets |
| --- | --- | --- | --- | --- |
| Bloomcrown | 0.25 + 0.50 f m | 100% for f < 0.375; 0% thereafter | 100% for f < 0.75; 0% thereafter | 0% foliage |
| Umbrellafrond | 0.50 + 0.75 f m | 0% | 100% for f < 1/6; 0% thereafter | 0% foliage |
| Springturf | 0.125 + 0.125 f m | 100% | 100% | 0% foliage |
| Stonecushion | 0.125 m | 100% | 100% | 0% foliage |
| Velvetpad | 0.125 + 0.125 f m | 100% | 100% | 0% foliage |
| Glowcap | 0.125 m | 100% | 100% | 0% cap tissue |
| Vaulttree, lanternberry, siphonreed | Not implemented | Not applicable | Not applicable | Not implemented |

Sources: `crates/cubarium-voxel-flora/src/lib.rs:658–687,767–832,893–919,961–987,1030–1055,1130–1161,1429–1450`;
`crates/cubarium-voxel-fauna/src/body.rs:509–541,576–585,615–679`.
The calculation is `h=max(1,round(s(a+bf)))`, `s=max(1,0.25/v)`, with current
acceptance `plant.y+h−animal.y` between 1 and `1+round(0.25/v)` inclusive.
Bloomcrown's escape wood is 0.225 organic on small and 0.45 on coarse grids;
umbrellafrond's coarse-grid escape is 0.10 organic.

These exact round-boundary thresholds use current source, including the already
scaled 0.25 m lift; they must not be substituted for D3's older mouth rule.
Horizontal overlap, water, other bodies and a route to a feeding pose are additional
conditions. A body on a higher terrace can reach a crown excluded by this table;
a nearby crown over an unwalkable pool need not be reachable at all.

### Proposed profiles: what can actually be calculated

Anatomy §2 says that intersecting a layer makes its foliage reachable, but gives no
within-layer density or partial-bite rule. Consequently an exact **physical** share
is not specified. Its “one cell up from the face” could mean a 0.125 m absolute
ceiling, or retention of today's extra 0.25 m lift above the head. It also changes
the API to a continuous face-relative band; today's head-layer test is not that rule.
The proposed animal's larger shell does not itself authorize a higher mouth.

For a transparent comparison, use two conditional ground-to-mouth bands,
`[0, C]` with C = 0.125 m and 0.25 m, on the same support plane. These are
interpretations, **not selected anatomy**. For a full layer spanning `[aH,bH]`
with foliage share s, a uniform vertical-density interpretation gives
`s × clamp((C − aH)/((b−a)H), 0, 1)`; sum over layers. The literal
“any intersection accesses the whole layer” interpretation instead gives s as soon
as there is positive overlap. A boundary touch alone contributes no stock in the
continuous interpretation. H is the proposal's linearly interpolated physical
height. These formulae cover each preset identically **only after** a metric mouth
and layer contract is chosen; current per-cell origins do not provide that property.

| Proposed species / growth stage | H in metres | Uniform-density reachable share, C = 0.125 m | Uniform-density reachable share, C = 0.25 m | Consequence / escape |
| --- | --- | --- | --- | --- |
| Springturf, all | 0.125–0.1875 | 100–66.7% | 100% | Never fully escapes; “entirely reachable” depends on the mouth interpretation. |
| Velvetpad, all | 0.125 | 100% | 100% | Geometry allows it; proposed diet says no. |
| Stonecushion, all | 0.125–0.1875 | 100–75% | 100% | Geometry allows it; proposed diet says no. |
| Glowcap, all | 0.125–0.25 | 100–50% | 100% | Its cap has changed geometry despite the claim of no model change. |
| Bloomcrown, seedling f ≤ 0.2 | 0.375–0.50 | 33.3–25% | 66.7–50% | Conflicts with the prose's 0.125 m seedling rosette; that alternative would be wholly reachable. |
| Bloomcrown, juvenile 0.2 < f ≤ 0.5 | >0.50–0.6875 | <40–29.1% | 40% | Basal stock accessible, upper crown out. |
| Bloomcrown, adult f > 0.5 | >0.6875–1.0 | 25%, then falling to 20.8% | 25% | Basal foliage persists: the whole adult does **not** escape browsing. |
| Umbrellafrond, seedling f ≤ 0.15 | 1.0–1.15 | 12.5–10.9% | 25–21.7% | The profile says a tall continuous band; “one tier at ground” needs separate geometry. |
| Umbrellafrond, juvenile 0.15 < f ≤ 0.5 | >1.15–1.5 | 0% | 0% | Lowest foliage starts above 0.575 m; claimed juvenile accessibility is false under either band. |
| Umbrellafrond, adult f > 0.5 | >1.5–2.0 | 0% | 0% | Lowest tier above 0.60 m; escape occurs at the seedling/juvenile transition. |
| Vaulttree, seedling f ≤ 0.1 | 2.25–2.375 | 5.56–5.26% | 11.1–10.5% | Again needs a distinct low seedling height to be a ground rosette. |
| Vaulttree, juvenile 0.1 < f ≤ 0.4 | >2.375–2.75 | 0% | 0% | Lowest foliage above 1.425 m. |
| Vaulttree, adult f > 0.4 | >2.75–3.5 | 0% | 0% | Even drapes begin above 0.9625 m; not fallback ground food. |
| Lanternberry, seedling f ≤ 0.2 | 0.625–0.725 | 20–17.2% | 40–34.5% | A low seedling override would change these results. |
| Lanternberry, adult f > 0.2 | >0.725–1.125 | 0% | <6.4%, falling to 0 at f = 5/12 | Adult lower edge is 0.3H; at C = 0.25 m it escapes when H ≥ 0.8333 m. |
| Siphonreed, all | 0.75–1.5 | 16.7–8.3% | 33.3–16.7% | Always has low tissue; access still fails where water or bank geometry excludes the browser. |

Derivations from `design/organism-anatomy-2026-09-21.md:88–204`, not measurements
of implemented profiles. Percentages assume full authored layer shares, no prior
cropping, sufficient horizontal reach, permission to eat, and uniform vertical
density. Under the literal whole-layer interpretation, every ground-starting
seedling, turf, pad, reed and glowcap layer is 100% accessible even if it extends
metres above the mouth; bloomcrown's basal juvenile/adult layers give 40%/25%.
That discrepancy is why a layer-intersection API is not yet a feeding model.
All these stands remain 0% foliage food to the current or proposed litter-only
shredder. Glowcap's own diet is a substrate-box overlap problem, not a mouth-height
problem. No browsing mouth is specified for the proposed bellwing; its nectar and
pollen diet needs separate paid products (dossiers D14).

### Today's mouth against the proposed profiles

For completeness, the cross-comparison uses the same uniform-density assumption
above but today's **accepted cell slabs**, face-relative `[0,0.375)` m on small
and `[0,0.5)` m on default/wide. The cell-centre heights are 0.0625–0.3125 m and
0.125/0.375 m respectively. Treating those slabs as continuous reach is a diagnostic
interpretation, not an implemented neck or a proposed choice. It exposes how retaining
today's raster rules would change the proposed animal across presets.

| Proposed profile/stage | Current small mouth slab | Current default/wide mouth slab |
| --- | --- | --- |
| Turf, pad, cushion, glowcap: all stages | 100% | 100% |
| Bloomcrown: seedling / juvenile / adult | 100–75% / <70–45.5% / 25% | 100% / <100–67.3% / <54.5–25% |
| Umbrellafrond: seedling / juvenile / adult | 37.5–32.6% / 0% / 0% | 50–43.5% / 0% / 0% |
| Vaulttree: seedling / juvenile / adult | 16.7–15.8% / 0% / 0% | 22.2–21.1% / 0% / 0% |
| Lanternberry: seedling / adult | 60–51.7% / <31.0–4.76% | 80–69.0% / <55.7–20.6% |
| Siphonreed: all stages | 50–25% | 66.7–33.3% |

Sources and exclusions are the preceding matrix's; all shredder foliage entries
remain zero. Conversely, a proposed continuous mouth applied to today's lollipop
would need a specified disc location/thickness convention: use the current matrix's
quantized layer and intersect its physical slab with the chosen mouth band. No unique
percentage follows from “up 1” alone. In particular, copying the 0.25 m continuous
ceiling is not equivalent to today's 0.5 m coarse-grid acceptance slab.

### Seeded and six-hour edible fraction

The required statistic is `sum(accessible foliage organic)/sum(all foliage organic)`.
Compute it separately for (a) geometric access from any legal pose and (b) access
within each animal's connected, affordable route; report pool foods separately.
For current stands the numerator uses the binary eligibility above and actual
support heights; for layers it sums accessible remaining layer stocks. Count-weighted
species percentages cannot substitute for this statistic.

It is **not numerically identifiable from the existing diagnostic output** at
either seeding or six hours. `voxel_census` prints counts and pooled totals, not
each stand's wood, foliage and feeding poses. `voxel_plant_autopsy` hardcodes the
authored world and steps plants without fauna; it does not accept the shipped
preset arms. Running it for six hours would answer a different question
(`crates/cubarium/examples/voxel_census.rs:208–287`,
`crates/cubarium/examples/voxel_plant_autopsy.rs:56–82,149–151`).
No new observer code was written under the handoff's no-code restriction.
Therefore, for **small/default/wide at t = 0 and t = 21,600 s**, the actual
stock-weighted fraction remains **unmeasured**, not zero. Counts measured in §7
and the conditional matrix are the supported results. Proposed six-hour fractions
are additionally undefined until anatomy, diet and layer stock rules are selected.

Neither consumer has demonstrated a succession-proof diet. D4 measured surviving
foliage without reliable access; the earlier reproduction census lost the shredder
at 7,320 s while litter subsequently accumulated. Candidate complements are
low basal browse alongside turf for the browser, and fungal tissue or carrion for
the shredder, subject to explicit diet, digestion and cue choices. These are
alternatives, not implemented foods or evidence that a second food alone ensures
replacement (`design/handoffs/voxel-reproduction-2026-09-21.md`, integration note).

## 3. Energetics in real units

For structure B, reserve R, requested straight speed v, requested turning rate ω, effective radius r
and cruise reference u, current organic consumption is
`C = B × [0.001 + 0.001 × (|v| + r|ω|)/u]` organic/s.
Reserve pays first, then structure; death occurs below minimum structure. This is
a speed-and-turning cost heuristic, not a calibrated mechanical work model
(`crates/cubarium-voxel-fauna/src/body.rs:103–143`,
`crates/cubarium-voxel-fauna/src/step.rs:226–275,652–665,1366–1370`).

| Animal / condition | Structure and initial reserve, organic | Rest, no food | Straight cruise, no food | Forward path budget |
| --- | --- | --- | --- | --- |
| Current 0.25 m browser newborn | 0.005 + 0.005 | ≈1,000 s | ≈500 s at 0.25 m/s | ≈125 m |
| Current fully stocked adult browser | 0.05 + 0.025 | ≈2,803 s | ≈1,401 s at 0.25 m/s | ≈350 m |
| Hypothetical 0.375 m browser, **length alone changed** | Same stocks as corresponding row above | Same | Same speed and lifetime | Same |
| Current shredder hatchling | 0.003125 + 0.000875 | ≈280 s | ≈140 s at 0.125 m/s | ≈17.5 m |
| Current fully stocked adult shredder | 0.0125 + 0.00625 | ≈1,886 s | ≈943 s at 0.125 m/s | ≈118 m |

These are continuous approximations, excluding food, reproduction and drowning.
After reserve exhaustion, shrinking B reduces expenditure: adult lifetime is
`[R₀/B₀ + ln(B₀/Bmin)]/k`, with k = 0.001/s at rest or 0.002/s at straight
cruise. The simulation's discrete step slightly changes the endpoints. Sources:
`crates/cubarium-voxel-fauna/src/body.rs:103–143`,
`crates/cubarium-voxel-fauna/src/manifest.rs:409–417,442–450`,
`crates/cubarium-voxel-fauna/src/step.rs:1160–1173,1346–1349`.

The hatchling row is a substantive correction: the handoff reused the former
0.00625 organic birth package, whereas eggs now contain 0.004. Calling both
lineages' newborn budgets 1,000/500 s conceals the shredder's shorter recruitment
window. Likewise, 0.03 browser `birth_body` is the parent's eligibility threshold,
not the newborn's structure (`crates/cubarium-voxel-fauna/src/step.rs:1069–1071`).

At adult browser structure 0.05, basal expenditure is 0.00005 organic/s and
straight-cruise expenditure 0.0001 organic/s. Maximum bite rate is **0.002 organic/s
per animal**, not per unit body. Assimilation is limited by both yield and mineral:
`min(0.5 × organic_taken, mineral_taken/0.05)`. For fresh foliage at mineral fraction
0.02, this permits 0.0008 organic/s, eight times straight-cruise expenditure.
Maintenance therefore requires approximately 6.25% of maximum-rate feeding time at
rest, or 12.5% at cruise, before growth and reproduction. Actual mineral density,
handling behaviour and replenishment can reduce this margin
(`crates/cubarium-voxel-fauna/src/step.rs:736,850–881`,
`crates/cubarium-voxel-fauna/src/lib.rs:805–806,879–900`).

Thus starvation within 3,600 s is an arithmetic certainty **without intake**, but
not a certainty for a fed animal. D4's 0.796 m and 1.668 m distances are from dying
browsers to the nearest **living crown**, not measurements of inter-crown spacing,
edible-patch spacing or legal route length. They are short compared with the ideal
125 m newborn path budget; that supports investigating encounter and navigation,
but cannot establish sufficient reachable production. With full forward effort and
maximum 2 rad/s turning, r = 0.0625 m gives k = 0.0025/s: the newborn budget falls
to about 400 s/100 m. Blocked effort can still cost energy. D4's large walked
distance and small count of visited columns are consistent with that waste
(`design/7_Research/voxel-census-2026-09-20.md:270–283,323–333`;
`crates/cubarium-voxel-fauna/src/step.rs:652–665`).

There is no implemented length→mass relation. Changing 0.25 m to 0.375 m alone
does not change the stocks, bite rate or 0.25 m/s cruise reference, so it does not
buy a 187.5 m newborn search budget. Under a separate assumption of geometric
similarity and constant density, that length increase implies 3.375 times mass;
retaining 0.05 organic adult structure contradicts that interpretation. A length
alone also does not determine mass without width, height and tissue density.

Biological allometry does not supply a unique replacement constant: White and
Seymour found approximately mass^(2/3) basal scaling in mammals, while Glazier's
analysis found dependence on metabolic state, with exponents approaching one at
extremes. These are warnings against calling independent geometry, linear upkeep
and fixed bite throughput one biological scaling model. They do not prescribe an
exponent, temperature, kg conversion or timescale for these fictional animals.
([White & Seymour, 2003](https://pubmed.ncbi.nlm.nih.gov/12637681/);
[Glazier, 2008](https://pubmed.ncbi.nlm.nih.gov/18348961/).)

## 4. Sensing as anatomy

The current browser evaluates **27 rays: nine per sector across three sectors**,
not nine total. Its pitches are −20°, 0°, +20° and its range is 2 m. A crown
inside the continuous pitch band need not intersect a sampled ray. Physical
first-hit classes distinguish foliage, stripped crowns, pools, bodies, terrain and
water diagnostically, but the policy gets coarser classes. Trunks currently are not
part of the cone's occupancy. The browser has no `Chem(litter)` or `Light` module,
despite anatomy §4 listing both (`crates/cubarium-voxel-fauna/src/manifest.rs:329–335,365–369,432–461`,
`crates/cubarium-voxel-fauna/src/senses.rs:501–560,669–739`).

At horizontal distance d, the pitch envelope allows roughly ±d tan(20°) of rise
relative to the eye: ±0.109 m at 0.30 m, ±0.291 m at 0.80 m and ±0.728 m at
2 m horizontal distance, with the separate 2 m slant-range limit truncating the
last case. Near food can lie above or below all sampled directions. Raising the
eye alone can improve high-crown detection while making low basal food harder to
see. A drawn raised neck changes neither this fan nor feeding geometry.

D4's nearest crown was simultaneously in range, in the pitch band and reachable
by an aimed ray at only 3/12 small, 1/24 default and 0/15 baseline deaths.
These are **potential visibility** tests, not mouth edibility or proof the policy's
actual fan hit the target. Also, “mostly clear” overstates the data: clear was the
largest single class, only 24.7–42.2%, not a majority. Different obstructions
collectively occupied most rays. Small had high crowns; default had distance and
ground-pool obstruction; the baseline had stripped foliage and bodies
(`design/7_Research/voxel-census-2026-09-20.md:257–321`).

A plausible ground-feeding analogue can combine distant orientation cues with
nearby chemical/tactile confirmation and head movements. In controlled Colorado
potato beetle experiments, visual and chemical cues interacted and vision could
dominate, so adding odour does not automatically provide a reliable second compass.
This supports testing cue combinations, not copying an insect's exact organs or
giving a controller privileged nearest-food coordinates
([Otalora-Luna & Dickens, 2011](https://www.ars.usda.gov/research/publications/publication/?seqNo115=263134)).
Foliage odour, contact with stems, a downward/upward feeding inspection, or a movable
head are design alternatives whose physical source and action costs must be explicit.

For the shredder, `Chem(litter)` is already a useful second *modality*, but not a
second *food*. Current transport mixes a fraction 0.4 across four same-height
neighbours every 0.5 s. On a regular unobstructed sheet this corresponds to the
diffusive approximation D = 0.2v² m²/s: 0.003125 m²/s on small, 0.0125 m²/s on
default/wide. Terrain steps interrupt the graph. Therefore the same numeric sensor
may describe different physical footprints and disconnected food on different grids
(`crates/cubarium-voxel-fauna/src/senses.rs:57–65,225–306`).
The approximation is not a measurement of actual cue spread on a landscape.

Contact uses a physical width-radius disc but tests solids at a body-relative cell
height; it is not a stem/food palp or complete support/drop receptor
(`crates/cubarium-voxel-fauna/src/body.rs:165–167,440–474`). Physical contact with a
stem, safe ground ahead and an edible leaf are different observations.

Policy transfer needs a declared schema for receptor anchors, observed quantities,
reference scales, cadence, and action interpretation. Keeping 23 or 37 numbers is
insufficient if their physical meaning changes. Even `birth_ready` currently checks
only structural and old reserve thresholds, omitting the new surplus hold,
refractory and egg rules (`crates/cubarium-voxel-fauna/src/body.rs:829–845` versus
`crates/cubarium-voxel-fauna/src/step.rs:1054–1075,1209–1288`). A changed sensory
meaning warrants a new policy contract and training on fresh worlds.

## 5. Plant anatomy versus animal anatomy

Layered foliage can express a cropped lawn, basal browse and canopy escape. It does
not yet establish those dynamics. Five interactions need explicit rules before
anatomy package 1 can claim its described behaviour.

**Persistent lower depletion.** Suppose a full adult bloomcrown has 1 organic of
foliage: 0.25 basal and 0.75 upper. Removing 0.10 from below should leave 0.15/0.75.
Recomputing fixed shares of the remaining 0.90 gives 0.225/0.675, moving 0.075 from
the crown into reachable tissue without growth. No scalar total distinguishes those
states. Per-layer amounts, damage state, or a declared alternative reconstruction
are needed; bottom-up filling alone is not a record of bottom-up bites
(`design/organism-anatomy-2026-09-21.md:52–84,128–139`).

**Conservation includes transitions.** Layer amounts must sum to total foliage;
withdrawing tissue must transfer the same organic, mineral and energy to the fauna
ledger. Stage changes, senescence, crown loss and dieback must redistribute or
deposit existing tissue explicitly, never create a new layer's full capacity.
Current withdrawal removes mineral pro rata from total stand material, and current
regrowth pays construction and mineral costs
(`crates/cubarium-voxel-flora/src/lib.rs:2429–2449`,
`crates/cubarium-voxel-flora/src/step.rs:1083–1145`). “Mass balance is untouched”
is a desired outcome, not proof that the new bookkeeping conserves it.

**Light needs receivers as well as occluders.** Today's crown shading already uses
continuous exponential attenuation; the binary part is membership in a footprint.
The proposal still tests shading layers against the recipient's **top**. That
misses shade on a basal rosette whose upper crown remains illuminated, and it does
not specify within-plant self-shading. A layered income calculation must say how
each surviving leaf layer contributes to assimilation, using physical area. It must
also avoid counting the same foliage independently in several light layers.
Porosity p = 0.45 does not imply more than half of light passes: the stated
transmission is `exp[−k(1−p)P/A]`, which also depends on P/A and k
(`crates/cubarium-voxel-flora/src/step.rs:340–394`;
`design/organism-anatomy-2026-09-21.md:70,165–167`).

**Porosity must have a sight meaning.** Reducing optical density while retaining
solid first-hit foliage occupancy produces a canopy transparent to plant light but
opaque to animal sensing. Axisymmetric bands also do not locate the pictured sky
gaps between lobes. A shared geometric or attenuation interpretation is needed for
light, cone rays and the picture. Drape containing 15% of vaulttree foliage but
classified only as a thin occluder is another unresolved food-versus-appearance
mapping (`design/organism-anatomy-2026-09-21.md:61,70–73,163`).

**Grazing does not automatically make a meadow.** Adults retaining 25% basal
bloomcrown foliage contradict the claim that they escape completely. Repeated
browsing may reduce income, but that alone does not guarantee shorter structure:
remaining upper leaves can fund growth, while current reserve-funded reflush only
activates below its existing whole-plant threshold. Nor does turf automatically
persist under a canopy. Canopy-sensitive germination and finite intercepted-light
budgets are already open model questions, explicitly recorded in the backlog;
they must not be smuggled in as parameter tuning
(`design/organism-anatomy-2026-09-21.md:132–139,228–235`,
`crates/cubarium-voxel-flora/src/step.rs:1097–1114`, `design/backlog.md:42–52`).

The proposal gives the browser several possible low tissues, not just one rosette:
turf, seedlings, basal bloomcrown, and reed edges if permitted and accessible.
It also threatens to remove existing pad/cushion foods, and juvenile fronds are
out of reach under the stated profiles. A persistent basal food plus an escape
canopy is a coherent possible ecology; universal seedling browsing can instead
prevent canopy recruitment. The trophic matrix must be evaluated through growth
and replenishment, not only at full foliage.

## 6. Life history and size

The current browser needs B ≥ 0.03 organic and reserve ≥ 0.015 for 120 s, then
transfers 0.01 organic into escrow over 180 s. Its 0.00005556 organic/s gestation
transfer approximately equals an adult's basal upkeep. A 300 s refractory period
followed by another hold and gestation makes the minimum birth-to-birth interval
about 600 s. Failure respires 25% of funded organic/energy and returns the rest;
mineral is retained/returned, not respired. These are the present placeholders,
not values endorsed by this audit
(`crates/cubarium-voxel-fauna/src/lib.rs:628–638`,
`crates/cubarium-voxel-fauna/src/step.rs:1054–1075,1102–1135,1153–1201`).

The shredder requires 0.004625 organic reserve for eligibility, holds for 120 s,
pays a 0.004 organic egg on litter at its current site, and has a 300 s refractory
period. Eggs incubate for 300 s without ongoing upkeep; removal or deep water
transfers them to carrion. Two hatchlings' minimum structure alone equals the
0.00625 adult reserve ceiling, before any reserve endowment or surplus floor, so
the present one-egg clutch follows from the frozen budget
(`crates/cubarium-voxel-fauna/src/lib.rs:651–658`,
`crates/cubarium-voxel-fauna/src/step.rs:1209–1288,1346–1349`).

These rules constrain population growth, but do not guarantee recruitment. The
shorter hatchling reserve window, a stationary incubation site, local fungal
competition for litter and changing water make **food available at hatch time**
more relevant than litter present when an egg was laid. Likewise, a gestating
browser needs accessible income for sustained recruitment, not just a large global
canopy. Concurrent feeding is not compulsory for every gestation: a full adult's
0.025 organic reserve can fund 0.01 escrow plus 0.009 basal upkeep over 180 s.
The reproduction integration note observed reduced overshoot but browser
extinction at 2,100 s and shredder extinction at 7,320 s in its historical world;
it did not establish carrying capacity on today's presets.

The anatomy proposal's newborn length ratio `(0.005/0.05)^(1/3) ≈ 0.464` is a
plausible geometric convention, not current founder mechanics: motor and sensing
use the fixed lineage manifest irrespective of structure
(`design/organism-anatomy-2026-09-21.md:216–219`,
`crates/cubarium-voxel-fauna/src/step.rs:648–675`,
`crates/cubarium-voxel-fauna/src/body.rs:829–864`). Under that proposed convention,
a 0.375 m adult has a 0.174 m newborn; a 0.75 m adult has a 0.348 m newborn.
Their eye heights, footing and reachable diets need not match. No evidence here
licenses declaring the existing reproductive timings biologically coherent for
either body without choosing mass/geometry and the intended simulation timescale.

## 7. Density and seeding

Current seeding already considers establishment and adult upkeep, future stream
depth, physical habitat density and several founder wood fractions
(0.30, 0.42, 0.55, 0.70, 0.85 of species cap). Its species targets per usable m²
are bloomcrown 0.25, umbrellafrond 0.15, springturf 0.60, velvetpad 0.40,
stonecushion 0.20 and glowcap 0.15. These are different denominators from whole-world
area (`crates/cubarium/src/voxel/habitat.rs:49–74,188–263,605–608`).

Fresh measurements in this audit used the existing `voxel_census` executable,
built in the normal release cache, equivalent to
`cargo run --release -p cubarium --example voxel_census -- 0 preset=NAME`,
for small/default/wide with deterministic preset seed 1. Small was repeated to
recover its output; default/wide ran once. They measured startup only, no ecology
horizon, and introduced no code changes.

| Preset | Horizontal area | Stands: bloom / frond / turf / cushion / pad / cap | Total stands per world m² | Each animal lineage per world m² | Nominal area per founder |
| --- | --- | --- | --- | --- | --- |
| small | 60 m² | 4 / 0 / 7 / 5 / 0 / 5 | 21/60 = 0.350 | 8/60 = 0.1333 | 7.5 m² |
| default | 192 m² | 19 / 0 / 43 / 21 / 2 / 13 | 98/192 = 0.5104 | 8/192 = 0.04167 | 24 m² |
| wide | 384 m² | 37 / 4 / 82 / 41 / 7 / 26 | 197/384 = 0.5130 | 8/384 = 0.02083 | 48 m² |

Every arm seeded eight browsers, eight shredders and 1.6 organic litter. Areas are
width × depth × cell-area from `crates/cubarium-voxel/src/recipe.rs:1331–1355`.
They are projected footprint areas, not total terraced surface, traversable habitat
or measured home ranges. A 2 m sensory radius is also not a home range. Fixed
animal counts across these areas imply markedly different initial densities.
The zero frond counts on small/default are actual startup observations, not an
inference of later extinction or proof of no possible wet niche.

Current browser placement seeks feeding-face components containing at least two
stands, but adjacency is built between **feeding faces**. Safe bare ground between
food patches is excluded from that connectivity calculation. This can reject a
walkable route without measuring the animal's true habitat. Separately, candidate
checks can inspect all support faces, whereas `Command::Seed{x,z}` resolves the
highest support in a column: a cave-face eligibility check may not describe the
eventual planting location. These are code-path risks, not measured frequency of
bad placements (`crates/cubarium/src/voxel/habitat.rs:163–167,238–243,448–558`,
`crates/cubarium-voxel-flora/src/lib.rs:2325–2341`).

The remaining choice is the startup contract. Establishment-only placement produces
an honest plant community but does not ensure consumer feeding routes. A habitat
acceptance check can require initial accessible foods and habitat for their renewal;
a seeder can instead deliberately provision a starting community, booking its inputs.
Neither is a guarantee of perpetual food, and automatic restocking would be a
different ecology. The current code already attempts a startup food guarantee;
its report should distinguish actual edible stock, safe connecting ground and
renewal potential. No density target or founder-size placeholder is retuned here.

## 8. The training world

The current arena is 8 × 4 × 3 m on 0.25 m cells, with flat support and a prepared
0.2 m-deep pond. Stage A has six browser stands covering three occupied crown
layers twice; Stage B has two stands at different reachable layers. Stage A foliage
is 0.06 organic per stand; Stage B's first patch holds 0.03. Starts are 0.5–1 m
from food, with browser target bearings within ±90°; Stage B's successor is
1–<1.5 m away in the near arm or at least 2 m in the landed arm. Births are disabled,
one animal acts, flora/hydrology are static, and horizons are 60/120 s
(`crates/cubarium-voxel-sim/src/arena.rs:45–56,88–138,230–246,355–391,423–471,508–639,1028–1085`,
`crates/cubarium-search/src/es/voxel/task.rs:23–41`).

Therefore the handoff's “0.125 m flat sheet, one head-height crown” description is
stale. Depletion can also leave stripped crowns; those are not categorically absent.
The shipped browser centre explicitly records the crown-height protocol and
generation 390 (`crates/cubarium/assets/policies/frondgrazer-p3d-reach-gen390.json:7–13`).
The defensible expectation was transfer of local acquisition/reacquisition skills
through shared body and sensing code. It was never evidence for transfer to relief,
growing food, showers, ground remains, many bodies or a full reproductive cycle.
The reach package's 8/8 arena reacquisition alongside live-world extinction is the
empirical counterexample (`design/handoffs/voxel-browser-reach-2026-09-21.md`,
integration note).

A representative curriculum should retain cheap controlled acquisition tasks and
then use fresh slices generated by the **same preset builder and ecology schedule**
as the host. Slices need relief and safe/unsafe edges; pools and shower transients;
food at every relevant developmental height; living, stripped and inedible tissue;
ground remains; other bodies; and juvenile as well as adult anatomy. A crop of a
world must specify its water, cue and movement boundaries so that cutting the slice
does not invent an easier habitat. Hold out landscape seeds and compare physical
resolution variants; keep training episodes bounded under the working resource policy.

The imitation teacher currently follows foliage sectors, alternates its blank-scene
turn bias every three controller samples, and generally applies full forward effort
unless contact intervenes. Recorded streams copy its adapted actions on the same
arena layouts (`crates/cubarium-voxel-fauna/src/controller.rs:452–493`,
`crates/cubarium-search/src/es/voxel/imitate.rs:250–319`). A new seed should copy
demonstrated observation-only acquisition, patch departure, recovery from obstruction
and successful travel on representative terrain. It should not copy a privileged
food locator or preserve a teacher's stationary turning simply because the imitation
loss is small.

Retrain after body, reach, layer and sensor semantics settle, then assess feeding
and recruitment in the coupled landscapes. Local policy skill can still be measured
separately from whole-world survival; neither score replaces the other.

## 9. Additional solution space

The following angles are not developed in the supplied anatomy/scale/dossier and
D1–D4/reach/reproduction documents. Novelty is scoped to that reading set, not a
claim that nobody has considered them elsewhere. Each is a hypothesis to compare,
not a requested feature or a tuning recommendation.

| Angle | Why the evidence makes it plausible | Distinguishing observation / consequence |
| --- | --- | --- |
| **Food quality and complementary nutrients**, rather than a second interchangeable calorie stock | Current assimilation is the minimum of organic yield and mineral supply; foliage mineral density changes independently of the visible foliage class (`crates/cubarium-voxel-fauna/src/step.rs:850–881`; `crates/cubarium-voxel-flora/src/lib.rs:2429–2449`). | Record assimilated currencies per food and limiting currency. A second diet might relieve mineral limitation, or merely add the same shortage; it needs taste/learning and accounting. Diet mixing improved performance in experimental generalist herbivores, without uniquely distinguishing nutrient complementation from toxin dilution ([Hägele & Rowell-Rahier, 1999](https://pubmed.ncbi.nlm.nih.gov/28307710/)). |
| **Leave a patch before complete depletion** | Training explicitly rewards depletion/reacquisition while D4 reports much walking through few columns; patch residence is not the same as successful landscape search (census D4 table; arena `508–555`). | Compare return per second, residence time and revisits with travel cost. A memory of recent intake could drive departure before the patch becomes blank. This is an inference from [Charnov's marginal-value model](https://quantitative.uw.edu/wp-content/uploads/sites/25/2020/03/CharnovMVT1976.pdf), not proof its optimality assumptions hold here. |
| **A shared encounter description for food, barriers and traversable gaps** | Ground pools become first-hit occluders, contact does not read stems, and feeding-face connectivity excludes bare paths (census D4; fauna `body.rs:440–474`; habitat `448–558`). | Compare what the animal can see, touch, stand on and eat at the same physical location. A pool amount need not imply a solid wall; define its actual volume/opacity and avoid sensor–motion contradictions. This changes representation, not cone tuning. |
| **Recruitment-site quality across incubation**, including within-patch dispersal | Eggs remain at the laying site for 300 s, then hatch with only about 140 s of cruising reserve, while glowcap shares litter (`step.rs:1209–1288,1346–1349`; decomposers handoff S2). | Measure litter and legal escape routes at hatch, local competitor load, and parent versus offspring intake. Site choice, dispersal or parental provisioning would be distinct paid mechanisms; a global litter total cannot test them. |
| **Spatial transport of detritus and its cues** | Litter is per-site and the cue graph is restricted to same-height neighbours; a rich pool can be effectively disconnected from a blind animal (`senses.rs:225–306`; dossiers D2). | Compare food connectivity and cue connectivity across slopes. Rain/runoff redistribution or an explicit physical cue medium could connect resources, but must transport the existing currencies rather than manufacture a uniform floor. |
| **Ontogenetic bottlenecks in the plants themselves** | Proposed frond/vault profiles lose low foliage abruptly at their stage transition; edible seedlings and inaccessible adults create strongly size-selective mortality (anatomy `145–163`). | Track cohort passage across stages, not just adult stand count. A landscape can keep old canopy while all replacement seedlings are eaten. Spatial nursery refuges and discontinuous profile changes have different consequences from simply speeding plant growth. |

The handoff's examples remain relevant alternatives—browsing-induced growth,
size-dependent animal diets, climbing/rearing, fresh leaf or fruit products,
predation and trails—but they are not counted as new discoveries here. In
particular, fallen leaves already have a litter destination; making them browser
food means choosing freshness, chemistry or digestibility semantics, not inventing
a second copy of the same organic material.

## 10. Dependency order and package disposition

This is a proposed sequence, not a dispatch. Fresh worlds only. Checks remain short
function/geometry/conservation checks; ecological horizons are explicit bounded
studies, not ordinary tests. After rule changes, use the existing 3,600 s autopsy
arms and, when the observables exist, a 21,600 s census. The handoff's approximate
150 s wall time per 3,600 simulated seconds is historical throughput, not a promise
for taller/layered worlds. Measure current throughput before allocating studies.

| Order | Package and explicit interface | What to measure before proceeding |
| --- | --- | --- |
| 0 | **Reconcile physical identity and observations.** Specify adult dimensions, mass-growth geometry, support datum, mouth interval, eye/contact anchors, and food permissions; name whether profiles use local amounts or a reconstruction rule. Add an observational edible-stock report in a separately authorized implementation package. | Same physical fixtures at 0.125/0.25 m resolution; exact mouths versus plant stages; t0/21,600 s stock-weighted edible fractions and route-connected food. Resolve missing measurements without tuning. |
| 1 | **Physical geometry and material interfaces.** A shared physical body/profile view, converted to cells at consumers; consistent area units for light and physical litter-cue transport. Presenter uses the same anchors. | Quantization error in reach/contact/light/cue extent; metre-scale route connectivity; correspondence of visible mouth and actual bite. No assumption that unchanged policy dimensions preserve semantics. |
| 2 | **Layer stocks and optical/feeding rules.** `layers(stand)` exposes bounds, remaining organic and optical behaviour; withdrawal returns booked organic/mineral/energy; growth/death/stage transitions preserve inventories. | Lower bite with upper stock retained; paid bottom-up regrowth; stage-transition balances; visible gaps versus ray/light results; layered production and escape thresholds. Then one bounded autopsy per preset. |
| 3 | **Complete the chosen diets and life histories.** Explicit resource identity, acceptance and digestion; consistent juvenile body/feeding geometry; leave backlog values fixed unless Wrysk separately changes the scope. | Currency-limited assimilation, accessible-food income versus upkeep/escrow, hatchling first successful feed, consumer and producer recruitment. A second food must improve access or quality, not merely inflate the same count. |
| 4 | **Startup habitat and anatomy presets.** Reuse establishment/upkeep checks, ensure checked face equals seeded face, connect food through walkable terrain, author physical layer dimensions. Add the three new stands only against those interfaces. | Per-species sites and starts per m², unmet niches, food-path lengths, water compatibility, basal production through succession, total imported material. Current seeding already implements much of the starting point. |
| 5 | **Representative training and one policy refresh.** Same body/food/sensor contract in cheap tasks and living landscape slices; regenerate observation-only imitation streams, then train under the bounded resource policy. | Acquisition, patch departure, reacquisition, metres walked versus unique physical area, blocked-cost fraction, supported diets, and held-out landscape/size performance. |
| 6 | **Coupled assessment and presentation.** Fresh small/default/wide worlds with chosen profiles and new policies; render actual states under the art direction. | At 3,600 s: death causes, visibility *and* mouth access, intake and regrowth. At 21,600 s: edible stock, edible production, births reaching maturity, plant cohort replacement, both animal lineages, and closed ledgers. A numerical ledger pass does not substitute for visible ecological variety. |
| 7 | **Bellwing, later.** Flight/support transitions plus paid nectar/pollen/product interfaces and associated sensing. | Flight expense, resting and feeding access, product withdrawal versus seed allocation, and life-cycle replacement; not merely animation or altitude occupancy. |

Against anatomy §6's five packages (`design/organism-anatomy-2026-09-21.md:238–254`):
**1, Layers changes** to include persistent local stocks, stage transitions, physical
light and cone semantics. **2, Ladder geometry changes** to metre-authored dimensions
and a separate presentation/world-height change; do not reship already-landed world
geometry as future organism work. **3, Animals 1:1 keeps its objective but changes**
to resolve 0.375/0.75 m length and juvenile anatomy before removing the drawn scale
factor. **4, New stands keeps the roster but changes**: reed needs explicit water
access, lanternberry's parcel must not be spent twice as fruit and seed, and the
vaulttree's death distribution needs a physical length, valid destination faces and
conserved shares (the anatomy says crown radius, dossier D11 says hop). **5 keeps
its deferral unchanged**, but flight alone is not the complete bellwing package;
its food/product interfaces precede ecological claims. No implementation package
survives wholly unchanged; the deferral and the 1:1 objective do.

The narrowest useful next implementation is therefore a shared physical encounter
contract plus the missing edible-stock observer. It can reveal whether low food is
absent, unreachable, unseen or simply not acquired before choosing among the larger
anatomical alternatives. This is the audit's recommendation, not an accepted decision.

**Decisions for Wrysk**

- **Adult size and growth:** retain the ladder's 0.375 m browser/0.19 m shredder
  manifest lengths, or the anatomy's 0.75/0.375 m full lengths; separately decide
  whether physical body dimensions grow with structure. These choices change
  clearance, anchors and juvenile feeding, not just how big the sprite appears.
- **Feeding anatomy:** choose a physical resting/extended mouth band and whether
  posture changes it. A low browser selects low tissues; a higher-reaching animal
  changes the escape strategy of the plants and needs corresponding sensing.
- **Diet breadth:** preserve today's broad foliage acceptance, adopt the proposed
  pad/cushion exclusions, or specify a broader resource diet; choose whether the
  shredder also consumes caps or carrion. Each changes a real food-web connection
  and needs paid digestion and a usable cue; turf is already browser food.
- **Layer state:** keep explicit remaining tissue per layer, or choose a simpler
  reconstruction model and accept what it cannot depict. Only a specified state
  rule can honestly retain lower cropping independently of an intact upper crown.
- **Meadow and succession:** choose whether adult bloomcrown keeps renewable basal
  browse, which woody stages escape, and whether seedlings have their own physical
  heights. These choices determine ongoing floor food and canopy recruitment;
  the current proposal contains contradictory answers.
- **Sensory anatomy:** keep a fixed fan with physically consistent anchors, add
  head inspection, or add specified chemical/tactile food cues. These alternatives
  alter what information is available and require an updated training contract.
- **Physical scaling ambition:** retain an explicitly abstract organic-unit model
  with independently authored geometry, or add a declared mass/geometry and
  life-history relationship. The latter requires a biological/time-scale choice;
  allometry does not supply a unique numerical answer.
- **Starting-world guarantee:** require only establishment, or also an initially
  connected and renewable consumer diet. The latter can be a habitat-selection
  rule or declared provisioning; neither implies ongoing restocking.
- **Scope of the next package:** authorize the physical encounter/edible-stock
  observer first, or an anatomy implementation that includes those interfaces.
  The observer closes the missing t0/six-hour measurements; the larger package
  commits to layer and body choices before those measurements are available.
