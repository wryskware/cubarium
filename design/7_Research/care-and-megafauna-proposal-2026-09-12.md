---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Optional care and rare megafauna

Wrysk approved the next animation direction, requested a first feeding/watering
interaction with a visible but balanced flourish, suggested tunable autonomy, and
asked how one or two larger apex predators could work. Wrysk also explicitly wants
to preserve the current good experience. This records a proposal, not accepted
mechanics or an implementation claim. No live state or defaults changed here.

## Existing footing

- [Environmental inputs](../environmental-inputs.md) proposes bounded, journaled
  stimuli and exact autonomous behavior with zero input. Its envelope is not an
  implemented API; `World::step` still has a placeholder admission phase.
- [Water](../water.md) supplies actual rain, flow, wet growth, flooding, and an open
  water budget. Manual rain should use those processes, not only draw streaks.
- [Fauna v2](../fauna-v2.md) supplies food gradients, feeding/resting, fruit,
  scavenging, inherited forms, and resource-funded budding. Predation is deferred.
- [The implementation plan](../implementation-plan.md), M3c, proposes a fixed-hunter
  experiment before evolved predation, not a guaranteed permanent predator guild.
  These are preferred designs, not canon preventing a different owner choice.
- README reports thin population margins and no validation beyond six simulated
  hours for the stratified world. Earlier-world longevity is not evidence for this one.

## First interaction: small, local, optional

Use an explicit web care panel outside the normal ambient view: Feed and Rain,
with an optional face-local target. Hardware/sensor adapters can follow later.
Keep the current world profile unchanged when no input is given.

Feed should deposit a finite, visibly identifiable edible resource; mineral nutrient
alone is fertilizer and will not give an immediate feeding response. Start with one
food category supported by existing diets, rather than universally attractive magic
food. If reusing fruit or detritus pools, supply a deposit visual that does not imply
an unsupported plant grew fruit. Hungry compatible creatures discover it through
local sensing, approach, feed, then rest; distant or satisfied creatures need not
react. No global wake-up, attraction teleport, free birth, or permanent speed boost.

Rain should ease in/out, add real water, run downhill, and affect pools and producer
growth. Immediate visible acknowledgement comes from rainfall; ecological payoff
can take longer and depends on existing wetness and nutrient/light availability.
Leaf deflection and pool ripples are possible later presentation responses. More
rain is not always better: flooding already inhibits growth and slows non-swimmers.

Keep two separate controls: ambient support (default exactly today's autonomous
conditions) and input dose. Test a more hands-on preset separately; do not equate
autonomy with a global animation-speed or activity multiplier. Less support can
make neglect consequential, so it must not silently become the default.

Engineering gate: bounded doses, pending queue, duplicate rejection, scheduling at
simulation ticks, journal/replay, seam-conserving distribution, and explicit source
accounting for food material, chemical energy, and water. The proposed old Moisture
effect is suitability, not actual rainfall; Food and Rain need precise units and
capabilities. Record rejected/capped requests and show acceptance in the care panel.
Persist/replay in-flight events correctly without duplicating a deposit after restart.

Important balance trap: food decomposing to nutrient does not remove its material
from this world. Cooldowns alone do not prevent accumulation over repeated feeding.
Measure cumulative input and long-run carrying capacity; choose a bounded cumulative
allowance or separately designed, explicitly audited export process before promising
unlimited daily care. Never silently discard excess material to make tests pass.

## Megafauna: one lineage before a roster

Prototype a recognizable canopy stalker: roughly 1.5–2 times a common creature's
apparent length as an art study, subject to actual footprint/seam/performance checks.
Use a distinctive slow gait, folded appendages at rest, a brief committed hunting
burst, and a long feeding/digestion pause. Size should carry real structural and
energetic consequences when integrated, not merely scale the existing sprite.

Start with one fixed founder in isolated copied worlds. Test paid pursuit/attack,
escape opportunities, failed hunts, handling time, satiety, and local prey depletion.
Candidate carrion feeding could bridge lean intervals but must carry explicit diet
tradeoffs, not give the hunter every food source for free. No automatic replenishment
of prey or predator, global prey-count breeding switch, or invisible invulnerability.

Recommend rare, resource-funded single-offspring budding/cocoon reproduction: one
adult can continue its lineage without requiring a mate when only one exists. Long
maturation, substantial parental reserve transfer, costly gestation, and a recovery
interval should make successful replacement notable. Juveniles begin smaller and
inherit the lineage's form, with limited heritable variation. These costs and timing
are candidate parameters to measure, not numbers already validated.

One or two adults is a desired typical outcome, not a promise of exactly two. Local
competition and prey productivity may produce zero adults, temporary juvenile/adult
overlap, or occasional excess. A guaranteed resident pair needs an explicit managed
population mode; do not hide it behind claims of emergence. Dormant eggs are a later
possible continuity mechanism, but require paid stored material, mortality, and
recruitment rules; they are not free respawns.

## Suggested sequence and acceptance

1. Continue authored growth/plant articulation; implement the narrow optional care
   path independently, beginning with rain and then an edible food deposit.
2. Compare unattended, occasional care, and repeated-input runs across matched seeds;
   retain real-time captures, resource audits, lineage/population results, and an
   unchanged zero-input baseline. Test disconnects, pause, replay, and excessive input.
3. Run a megafauna art study and fixed-hunter ecology experiment separately. Evaluate
   predation on/off against the same seeds and starting resources, accounting for the
   founder's mass/energy; do not add a free predator to only one conservation control.
4. Integrate an opt-in lineage only if survival, hunting, offspring costs, and prey
   recovery are readable and acceptable over longer runs. Expand lineage variety later.

Keep the approved preview/checkpoint and original world untouched during experiments.
This interaction work changes ecology and input/persistence contracts, unlike the
presentation-only animation passes; it needs its own implementation brief and tests.
