---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Terrain, hydrology and ecosystem design: research notes

Evidence gathered for the [landscape/ecosystem proposal](../terrain-and-ecosystem-proposal-2026-09-16.md).
Primary sources below were accessed on 2026-09-16. This is a targeted literature
pass, not a systematic review. No new simulation, training, capture or hardware
experiment was run. Equations and species in the proposal are design candidates,
not calibrated results from these sources.

## 1. Terrain structured by drainage

Génevaux, Galin, Guérin, Peytavie & Beneš (2013), *Terrain Generation Using
Procedural Models Based on Hydrology*, ACM TOG 32(4), article 143.
[Author-hosted paper](https://www.cs.purdue.edu/cgvlab/www/resources/papers/Genevaux-ACM_Trans_Graph-2013-Terrain_Generation_Using_Procedural_Models_Based_on_Hydrology.pdf),
[DOI](https://doi.org/10.1145/2461912.2461996).

Sections 3–4 describe building a drainage graph, increasing elevation upstream,
and deriving terrain features around the river network. This is a useful
precedent for producing connected geography from a few controllable structures.
It suggests a better starting vocabulary than independently noisy pixels.

**Cubarium inference:** use broad ridges, receiving basins and a drainage skeleton,
then add detail. Compare against noise-only terrain. The full paper's hierarchy,
river classification and scale are unnecessary for the initial desk landscape.
Its bounded-domain construction is not directly a torus generator; interior
terminal basins and seam-consistent geometry would be our adaptation. The paper
does not establish suitability at 64×64 pixels or ecological sustainability.

## 2. Depression analysis without deleting every pond

Barnes, Lehman & Mulla (2014), *Priority-Flood: An Optimal Depression-Filling and
Watershed-Labeling Algorithm for Digital Elevation Models*, Computers &
Geosciences 62, 117–127.
[Paper record/abstract](https://arxiv.org/abs/1511.04463),
[authors' reference implementation](https://github.com/r-barnes/Barnes2013-Depressions).
The journal year is 2014; the linked arXiv submission is 2015.

Priority-Flood supplies depression/watershed analysis across several grid and
mesh connectivities. Standard depression filling makes a surface drain; that is
not the same objective as preserving ponds with finite storage and spill levels.

**Cubarium inference:** use basin/spill analysis to diagnose and construct
catchments, preserving selected closed depressions. Do not “repair” away every
lake. A periodic domain has no ordinary outer boundary from which to seed the
standard algorithm: choose terminal sinks or a depression hierarchy explicitly.
It is a generation/analysis aid, not the runtime fluid solver.

## 3. Groundwater is storage connected to surface water

USGS, *Ground Water and Surface Water: A Single Resource*, Circular 1139:
[Box A, concepts and stores](https://pubs.usgs.gov/circ/circ1139/htdocs/boxa.htm),
[hydrologic interactions](https://pubs.usgs.gov/circ/circ1139/htdocs/natural_processes_of_ground.htm).
Also [Springs and the Water Cycle](https://www.usgs.gov/water-science-school/science/springs-and-water-cycle).

These sources distinguish unsaturated soil water from saturated groundwater and
describe exchanges with streams and springs. A spring can occur where groundwater
reaches the land surface. Depending on relative water levels, surface waters can
gain groundwater or recharge it. Water available to roots is not identical to
visible standing water.

**Cubarium inference:** represent surface, root-zone and aquifer stores, with
head-controlled discharge and explicit capacity/overflow. Start with a few
catchment reservoirs rather than a full groundwater PDE. That approximation
and its time constants require tests. An external deep-aquifer input is a valid
stylized boundary condition, but must be booked as input; a spring alone is an
internal transfer, not an unlimited source.

The proposal's exponential reservoir example and proposed volume-transfer
rules are our mathematical model sketches. They are not quoted USGS simulation methods.
No claim of reproducing MODFLOW, shallow-water momentum, capillary physics or
quantitative natural hydrology is made.

## 4. Local niches and dispersal must be designed together

Thompson et al. (2020), *A process-based metacommunity framework linking local
and regional scale community ecology*, Ecology Letters.
[Publisher full text](https://onlinelibrary.wiley.com/doi/10.1111/ele.13568).

The paper treats environmental responses, interactions, dispersal and stochastic
processes together. Its discussion and model show why increasing connectivity
can help colonisation while also homogenising communities. The model includes
abiotic niche breadth and distance-dependent dispersal; it uses a toroidal
spatial layout to avoid edge effects.

**Cubarium inference:** different soils and light fields will not preserve
patches if seeds, food or nutrients redistribute almost globally. Test niche
breadth, dispersal distance and patch spacing jointly, and measure composition
between patches as well as overall richness. This does not prescribe one
optimal dispersal rate or prove coexistence in a trophic network; the paper's
organisms, interaction model and scales differ from Cubarium's.

## 5. Preserve multiple ecosystem candidates

Mouret & Clune (2015), *Illuminating search spaces by mapping elites*.
[Authors' paper record/abstract](https://arxiv.org/abs/1504.04909).

MAP-Elites retains high-performing solutions across user-chosen descriptive
dimensions. This supports considering a set of distinct viable solutions rather
than selecting one scalar champion. It already appears in Cubarium's recurrent
organism research; the extension here is to make a complete ecosystem recipe
the candidate, with separate feasibility and presentation assessments.

**Cubarium inference:** begin with a small bounded set spanning canopy cover,
wet habitat and succession characteristics, then test whether those descriptors
actually correspond to interesting different worlds. Descriptor occupancy is
not ecological niche formation, coexistence or indefinite evolution. Optimizing
the descriptor alone can produce empty decorative water or inert forests;
reproduction, turnover and budgets need independent evidence.

## 6. Topology is separate from the display's shape

Allen Hatcher, *Algebraic Topology*, Chapter 0, printed page 5:
[Author-hosted chapter](https://pi.math.cornell.edu/~hatcher/AT/ATch0.pdf).
It describes a torus by identifying opposite sides of a square.

**Cubarium derivation:** the five connected physical faces have a disk-like
surface with a four-segment boundary; suitable opposite boundary identifications
can turn that abstract surface into a torus. Ordinary physical cube seams alone
do not. A panel periodic in one ground dimension is cylindrical; two periodic
ground dimensions give a torus. Extra seam orientation and vertex tests are
needed before implementing either representation.

The topology discussion above records the earlier surface-model exploration.
Wrysk subsequently selected voxel terrain with globally downward gravity and
prioritized a shallow panel ringworld. The current proposal uses a horizontally
periodic volume first; a toroidal cube skin is not the chosen implementation.
The subsequent platform clarification places this development on desktop with
a local renderer; ringworld is the layout, not an instruction to use the panel
hardware during this cycle.

The current local shim [geometry contract](/home/wrysk/vuzic/led-cube-shim/docs/GEOMETRY.md)
was read during this pass. It defines eight physical seams, four open bottom
edges, direction transport and cubemap projection. Its cubemap cameras are at
the origin looking outward. An interior landscape seen through exterior glass
faces requires a different presentation design. Reuse the shim's hardware
mapping; hypothetical extra world connections belong in Cubarium.

## 7. Voxel terrain and fluid-solver scope

Following Wrysk's voxel clarification, two primary implementation references
were inspected:

- Ryan Geiss, [Generating Complex Procedural Terrains Using the GPU](https://developer.nvidia.com/gpugems/gpugems3/part-i-geometry/chapter-1-generating-complex-procedural-terrains-using-gpu),
  GPU Gems 3, chapter 1. It describes volumetric terrain generation and surface
  extraction, supporting the distinction between a 3D solid representation and
  what is rendered. It does not require Cubarium to use its GPU pipeline.
- Keenan Crane, Ignacio Llamas & Sarah Tariq,
  [Real-Time Simulation and Rendering of 3D Fluids](https://developer.nvidia.com/gpugems/gpugems3/part-v-physics-simulation/chapter-30-real-time-simulation-and-rendering-3d-fluids),
  GPU Gems 3, chapter 30, especially §§30.2–30.3. It describes grid velocity,
  pressure projection, solid boundaries and liquid rendering. It provides a
  possible stronger solver reference if simple conservative volume rules fail.

These are historical algorithm references, not current-library recommendations
or performance predictions for Cubarium hardware. Voxel materials, a voxel
fluid solver and a visibly blocky renderer are separate choices. The owner has
selected voxel terrain; the numerical fluid method is still a design question.
The previous single-heightfield flux sketch is inadequate for arbitrary stacked
surfaces and has been removed from the working proposal. Required tests include
overhangs, covered passages, communicating vessels, wrap conservation and
long-run volume drift. No voxel solver has been implemented or benchmarked here.

## 8. What was deliberately not concluded

No source establishes the correct cube projection, target organism count,
terrain resolution, ecological timescale or training budget. This pass does not
establish buyer demand. It does not infer successful live evolution from an
offline optimizer's variety. Earlier food-web and recurrent-policy evidence is
linked from the proposal rather than repeated as newly discovered research.

A searched Bristol PDF named `gw4-bates.pdf` proved to be a research-project
description, not the Bates–Horritt–Fewtrell 2010 shallow-water paper. It is not
used as evidence for a solver. A full inertial water model would need a separate
numerical-method review if the simpler flow proposal fails the required behaviors.
