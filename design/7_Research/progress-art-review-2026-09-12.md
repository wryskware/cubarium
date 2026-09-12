---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Progress and art-direction review, 2026-09-12

Review of working tree `96737d2`, Wrysk's screenshot and feedback, the running
web viewer, and a separate continuation of its checkpoint at normal speed.
This records observations and recommendations. It does not accept a body
grammar, change milestone requirements, or authorize any new canonical choice.

Wrysk's feedback: the current presentation suggests an infestation; the
creatures resemble small colored maggots, spin in place, and wander for food.
They want more diversity and creative expression, clarity about what remains
in the build, and a useful role for their own art or art direction.

The engineering foundation supports a persistent feeding world. Much of the
intended visible personality remains unimplemented. The existing plans do
address inheritance and readable behavior, but their proposed body vocabulary
is narrow enough that implementing it literally could preserve the current
impression. Treat this feedback as evidence for revisiting that proposal.

**Verified build position**

| Area | Current evidence | Remaining work |
| --- | --- | --- |
| M1 surface and output | Shared five-face surface, body stamps across seams, independent geometry oracle/tests, preview, shim and PNG sinks | Physical seam/vertex/rim observation is still recorded as pending |
| M2 living world | Resource production, grazing/scavenging, recycling, paid movement and budding, death, weather, snapshots/resume, telemetry and life-event logs | The documented 30-minute ambient watch and physical-cube review remain incomplete; this short review does not close them |
| Recent presentation | Web cube/net viewer, interpolated positions, 60 fps output, Outrun palette and revised producer brightness ramp | The viewer needs explicit playback-rate context for reliable visual review |
| M3a/b inheritance and appearance | Specification exists; runtime still uses genome v1, one functional founder genotype with cosmetic hue differences, and exact genome copies at birth | Sparse mutation, variable body grammar, gait, visual gallery, mutant probes, and evidence of visibly inherited habits |
| M3c onward | Predation, dormancy, optional niche mechanisms, generalized environmental inputs and installation soak are proposals | Implementation and validation; these should not be represented as existing features |

Sources: [implementation plan](../implementation-plan.md),
[M3a specification](../m3a-spec.md),
[genome and phenotype](../../crates/cubarium-core/src/genome.rs),
[world creation and birth](../../crates/cubarium-core/src/world.rs), and
[host contract](../../crates/cubarium/README.md). The older introductory sentence
in the implementation plan saying no experiments have run is stale relative
to its own current-status paragraph and the E2/E3 reports.

**Presentation findings**

1. **The live session is accelerated.** The process serving port 7393 was
   launched with `--sink web --fps 60 --speed 8 --fresh`. Thirty wall seconds
   cover approximately four simulated minutes. Its activity cannot establish
   real-time ambient pacing. The viewer's displayed `tick` is a submitted-frame
   counter, and `fps` measures browser animation frames; neither reports the
   simulation multiplier. Recommend exposing simulation speed in the explicit
   development viewer, outside the cube image. See
   [web sink](../../crates/cubarium/src/sink/web.rs) and
   [viewer loop](../../crates/cubarium/src/sink/web/index.html).

2. **Spinning has an identifiable controller cause.** Under the current
   defaults, maximum translation is 0.3 px/s. Feeding requests 20% of that
   (0.06 px/s); resting requests 5% (0.015 px/s). The controller still updates
   wandering noise and permits turns up to 90 degrees/s in every mode. Over
   four simulated seconds a resting creature can travel at most 0.06 pixels,
   while its angular budget permits a complete revolution. This is a bound,
   not a measurement that every individual actually turns that far. At 8x,
   those four seconds pass in half a wall second. The normal-speed sequence
   still shows changing orientations in a small footprint. Proposed response:
   evaluate mode-dependent turning/noise, sustained headings during travel,
   real stillness while resting, and a local feeding gesture. Validate the
   ecological consequences because the slow translation was introduced to
   support local depletion and recovery. Sources:
   [controller](../../crates/cubarium-core/src/controller.rs),
   [defaults](../../crates/cubarium-core/src/config.rs), and
   [movement settlement](../../crates/cubarium-core/src/world.rs).

3. **The body vocabulary is almost singular.** All current founders have the
   same size and speed genes. Their phenotype has the same three overlapping
   discs: core, head and tail. Birth copies that genome; the presenter scales
   juveniles but has no gait animation. Soft coverage makes the connected discs
   read as a small rounded capsule. Hue and current feeding brightness supply
   much of the apparent variation. M3a proposes one to four axial lobes,
   aspect ratio, a head/tail appendage and oscillation. Those are useful
   controls, but recognizable diversity remains a visual hypothesis. Broaden
   the candidate silhouettes before treating that grammar as sufficient.

4. **The habitat reads as a field visualization.** Food and detritus are
   presented from 16×16 field cells per face, each covering 4×4 pixels.
   Producer filtering softens the boundaries only slightly; detritus uses
   unfiltered cells. The captures consequently show colored blocks beneath
   moving creatures, with little recognizable stationary growth. A promising
   art experiment is to express the existing producer biomass through stable
   local growth motifs—mats, rosettes or branching patches—whose coverage
   changes with that biomass. This can begin as presentation of the existing
   producer pool. Independent plant-like organisms, new diets, and additional
   ecological roles would require separate simulation work. Sources:
   [field grid](../../crates/cubarium-surface/src/field.rs),
   [presenter](../../crates/cubarium/src/present.rs).

5. **Existing life events have limited visible expression.** Rest is principally
   a brightness change. Feeding adds warm light over the core; overlapping
   light often makes the core look pale pink/white in these captures. The
   immutable render view carries no gestation progress, so the proposed
   attached growing bud is absent. Expressing feeding, rest, development and
   budding would make the existing simulation easier to read. Any new visual
   cue should follow the actual state it claims to depict. Sources:
   [render view](../../crates/cubarium-core/src/view.rs),
   [body presentation](../../crates/cubarium/src/present.rs).

**Ecological diversity needs its own work**

The [E2 confirmation](e2-confirm-2026-09-11.md) reports survival in eleven of
twelve 24-hour runs across speeds/weather settings, repeated local depletion
and recovery, and small material residuals. The one extinction was at 0.4 px/s;
the current 0.3 px/s configuration survived all six runs in that batch.
That establishes useful resource dynamics, with limits on unattended survival.

The [E3 report](e3-default-2026-09-11.md) found only one founder lineage alive
at 24 hours in each of three default runs. This is evidence of severe ancestry
bottlenecks in the fixed functional genotype population, not proof of selection
for one superior strategy or a demonstration that future mutation must fail.
Nevertheless, simply introducing more attractive founders will not establish
sustained ecological diversity. Future comparisons need lineage survival,
trait distributions and local occupation as well as population totals.

**Proposed next slice and Wrysk's contribution**

Bring the visual gallery and real-time behavior review to the front of the
next slice. Pair it with work on movement and visible life events. Keep the
current accounting, surface geometry and persistence foundation; use the
gallery to test the proposed visual vocabulary before a longer evolutionary
run depends on it. The following is a recommendation, not a newly accepted
milestone or a fixed species catalog.

- Start with a small silhouette sheet, roughly six strongly contrasting
  candidates. Consider broad pods, angular skimmers and asymmetric walkers;
  retain a chain form as one possibility. These are drawing prompts, not
  ecological classes. Compare them in one color at intended pixel size.
- Give the strongest candidates recognizable movement: settle, probe, glide,
  step, recoil, unfold. Wrysk can direct these with short written descriptions,
  reference clips or a few key poses. Actual simulation mechanisms determine
  which actions the world can support.
- Develop a 64×64 habitat mockup with recognizable producer growth, occupied
  patches and breathing room. Judge the whole composition alongside individual
  creatures. Retain the current indigo/cyan/magenta family as a starting point
  while testing shape, value and warm accents.
- Implement a small selection in a development gallery, including seam
  crossings and actual rest/feed/bud states. Compare related variants and
  unrelated shapes without relying on color to tell them apart. Then compare
  real-time ecosystem clips and the physical cube.
- Feed those observations back into the proposed grammar and controller,
  then run the inheritance/viability experiments with lineage tracking.

Wrysk can author the visual language that evolution explores: silhouette
families, allowed variations, markings, material character, movement cadence,
growth motifs and palette relationships. A sketch with a clear personality is
useful even before it fits the final pixel budget. A particularly useful first
art contribution would be **one desired 64×64 scene plus three inhabitants
shown resting, moving and feeding**. Mood references and annotations are also
valid input; finished pixel art is not a prerequisite.

The current renderer consumes procedural discs, so there is no existing
drop-in sprite or artist authoring pipeline. Integrating authored masks,
components or sprite fragments would be deliberate implementation work.
Authoring reusable visual components is compatible with inheritance and
variation. Decorative choices can be presented as decorative; only claimed
biological functions need corresponding mechanisms and tradeoffs.

**Review evidence and limits**

- Inspected the supplied screenshot and 61 live `/frame` samples over 30.002
  wall seconds. Frame counters advanced from 1,229,609 to 1,231,409, consistent
  with 60 submitted frames/s. The process command independently established 8x.
- Copied checkpoint `world-3296400.cubw` into an isolated temporary state
  directory and continued it for 30 simulated seconds at `--speed 1`, using
  the existing release executable and PNG sink. Original checkpoint SHA-256:
  `ca26ba02e11f711f9b5381174cc7c7301d9f5527160958e89b8a318727bf8995`.
- The separate continuation completed 600 ticks and 1,797 rendered frames in
  30.00 wall seconds, ending at tick 3,297,000 with population 91 and material
  residual −2.274e−13. Sixty PNG samples were captured at approximately 0.5 s
  intervals. Its source snapshot was about 45.8 simulated hours old.
- Reviewed [normal-speed frame sequence](assets/progress-2026-09-12/front-normal-speed.png)
  and the live net. A [normal-speed clip](assets/progress-2026-09-12/normal-speed.gif)
  is included for playback; it samples at 2 fps, so it does not establish
  full-frame-rate smoothness. Pixel sampling is enlarged with nearest-neighbor
  scaling, and GIF playback uses palette quantization.
- The 8x live recording and 1x continuation start at different times and are
  not a matched comparison of identical starting states. Neither replaces the
  planned longer watch or checks actual LED brightness and room conditions.
- This review changed documentation and saved review captures. It did not
  modify the runtime, the live viewer process, its original state, or canon.
  Existing automated and long-run reports were read; the full test suite and
  E2/E3 experiments were not rerun for this review.
