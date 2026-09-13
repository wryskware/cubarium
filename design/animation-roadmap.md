---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Animation and living-world ideas

Working backlog from Wrysk's review and authorization to begin the animation work.
This is implementation direction and a record of possibilities, not new canon.

## First: motion that reads on five 64×64 faces

Keep the current 20 Hz ecological simulation and 60 fps presentation initially.
The live web feeds measured approximately 60 submitted frames per second at 1×
speed during the review; that does not establish browser or physical frame pacing.
At the review baseline, the presenter sampled many effects at whole ticks,
four-frame plant clips held each pose for about 750 ms, tall plants mostly pulsed,
and rain included binary sparkles and integer-position jumps. The first pass below
addresses those temporal steps; spatial sprite filtering already existed.

Priorities, in order:

1. Smooth existing motion: fractional simulated presentation time, continuous
   pose/glow transitions, calm light envelopes, and less stepped rain. Preserve
   sharp readable silhouettes and the existing palette; avoid blur as a substitute
   for articulation. Validate at native resolution and normal playback speed.
2. Readable growth: sprout → extending stalk → greater height/unfolding leaves →
   buds/flowering/fruit. Current stage and tree-height thresholds can jump multiple
   stages in one tick. Add bounded visual transitions tied to real resource state;
   distinguish presentation history from future individual-plant biology. Established
   vegetation should not replay its entire birth merely because a viewer restarts.
3. Shared gentle wind: a gust passes across connected faces; fine reeds respond
   quickly, crowns lag, and tendrils settle. Keep roots anchored, tiled trunks joined,
   and the shim's geometry/transport contract intact. Wind begins as presentation;
   ecological transport or energetic effects need a separately described change.
4. Visible interactions: plants react to nearby fauna, and resting/feeding poses
   communicate distinct creature habits. Leave still areas and quiet intervals.

## Creative backlog

- Touch-responsive vegetation: nudged fronds, bowed landing stalks, sensitive
  flowers curling and reopening. Favor a clear one-pixel silhouette response.
- Rain aftermath: pooled ripples and slowly settling leaves first; droplets
  accumulating on leaves, hanging, falling, and rebounding later.
- Local bioluminescence: disturbance produces a soft brightening/fade, with
  occasional neighboring responses. Do not imply a simulated communication
  mechanism unless one exists.
- Ecological traces: fallen fruit, litter, recovering clearings, and new shoots;
  nibbled leaf detail later. Keep representations grounded in actual world state.
- Creature rituals: antennae testing the air, skimmers pausing at pool edges,
  gliders folding fins to rest, subtle feeding gestures and different settling times.
- Longer plant life histories: senescence, wilting, collapse, regrowth, and persistent
  individual age/flowering history. These exceed a visual transition alone.

## Resolution and presentation

Wrysk explicitly favors the ideas but deprioritizes droplets on leaves and nibbled
foliage: they may be hard or impossible to distinguish at 64 pixels. Keep them as
later experiments for the web viewer and a possible future higher-resolution LCD
cube. An LCD cube is a possibility, not an accepted hardware or runtime migration.
Enlarging today's 64×64 output does not create additional scene detail. Higher native
resolution requires revisiting logical dimensions, art scale, surface budgets, and
the display contract; do not expand that scope in the initial animation work.

## Collaboration

Wrysk requested Astra and Fable 5.1 for planning and sprite/animation work, with
Claude favored for implementation throughput. Keep concrete briefs with the work;
Fable may delegate scoped implementation to Opus 5 high/medium. Record validation
and remaining limitations here or in linked research notes, without promoting canon.

The first implementation pass is tracked in
[Fable's implementation record](7_Research/animation-slice1-2026-09-13.md), with
[Astra's proposal](7_Research/astra-animation-plan-2026-09-13.md) and
[the independent review](7_Research/astra-animation-review-2026-09-13.md).

The second pass (2026-09-12: shared wind, rooted bend, lanternstalk growth pilot) is in
[Fable's slice 2 record](7_Research/animation-slice2-2026-09-12.md), directed by
[Astra's slice 2 plan and review](7_Research/astra-animation-slice2-2026-09-12.md), with
the work order in [the slice 2 brief](7_Research/animation-slice2-brief-2026-09-12.md).

## Status after slice 2 (2026-09-12)

Landed (presentation only; no simulation, snapshot or transport change):

- Priority 1 (smooth motion) and 2 (readable growth) from slice 1 are unchanged.
- Priority 3, shared gentle wind: one deterministic breeze packet every 30 s with an
  exactly quiet 12 s interval, Astra's seam-joining chart circulation, a rooted horizontal
  bend in the renderer with a measured per-family amplitude budget under the nine-pixel
  footprint, species stiffness and lag, canopy plants rotating in place, tall columns
  bending as one curve with their vines. Roots are fixed; ground, water, rain and bodies
  do not move.
- A first authored growth clip: lanternstalk sprout → stalk (pack v5 `grow<from><to>`
  rows, non-looping), played by the presenter with short endpoint blends into the idle
  sway, reversible, the reveal masks kept as the fallback for every other step.

Remaining, in the order Fable would take them:

1. **Wind readability at 1×.** The shipped art leaves only 0.30 px (spiretree cap), 0.47
   px (glasscane/vine trunk) and 0.31 px (tendrilfan in fruit) of sideways room; those
   families barely move. Explore narrower art or recentered per-part bend bounds and
   trunk-strip anchors; these are candidates, not a decided redesign. Each needs its
   own seam and registration proof. Judge on the cube first: the lanternstalk (0.45 px) and reed
   (0.70 px) do read.
2. Authored growth for lanternstalk 1 → 2 (its own stalk flare and 7×7 bulb), then the
   other side species; the top-face radial species keep the masks until a top-down
   "opening" convention is authored.
3. A spin response for a reed standing on a flooded top-face cell (currently still).
4. Bake-side coverage sampling (Astra's 4×4 subpixel proposal), only if a compared capture
   shows a thin stalk still jumping between nearest-sampled frames.
5. Priority 4 (visible interactions): readable plant–creature reactions, rain aftermath,
   bioluminescent responses, and creature rituals remain candidates after the animation
   foundation. Favor silhouette-scale changes at 64 px. Droplets on leaves and nibble
   marks remain specifically lower-priority, higher-resolution experiments.

## New owner direction: optional care and megafauna

Wrysk approved the next growth/articulation direction and requested feeding/watering,
a balanced flourish of activity, possible autonomy tuning, and rare larger apex
predators. Preserve the current unattended experience. The
[care and megafauna proposal](7_Research/care-and-megafauna-proposal-2026-09-12.md)
records candidate mechanics, single-offspring reproduction, resource-accounting
risks, and isolated validation steps. These features are not implemented. Add the
narrow optional care pass alongside the animation work; keep predation a separate
experiment before changing the approved world.
