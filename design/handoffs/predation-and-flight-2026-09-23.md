# Handoff: predation and flight substrates

For an ecology-adjacent thread, from Wrysk's organism design session (2026-09-23). Two
mechanisms the roster now needs, and neither exists in the voxel line:

- **Predation**, for the lanternjaw (mesopredator) and later the chorister (pack hunter).
- **Flight**, for the bellwing (and possibly a flying capgnawer or a gliding seedporter,
  depending on which body plans Wrysk picks).

The job: design both as substrates (model mechanics, not species tuning), write the
package briefs, and bring the decisions that are Wrysk's to him. Body plans are **not**
settled for the new animals and are not needed here. Design for the niche and size, not
the look.

## Read first

- Roster and sizes: `design/art-direction/organism-scale-and-roster-2026-09-21.md` §4–5;
  `design/organism-large-animals-2026-09-23.md` (lanternjaw at coyote scale, chorister,
  loftstrider, §6 hard questions); `design/animal-body-candidates-2026-09-23.md` (the
  candidate bodies, so you know the range of shapes).
- Roles and rules: `design/theoretical-biosphere-2026-09-16.md` §4 (seven animal roles:
  lanternjaw, bellwing, …) and §5–6. Paid reproduction, no spontaneous appearance, all
  deaths to the right pool, glow costs reserve.
- What the fauna model is today: `design/handoffs/voxel-organism-decisions-2026-09-21.md`
  and its package notes (bodies and anchors in metres, mouth band, encounter contract,
  diets: shredder eats litter + caps + carrion), `design/handoffs/voxel-retrain-2026-09-22.md`
  (sensor manifest v2, 45 rays, trained founders, the retrain branch). Code:
  `crates/cubarium-voxel-fauna/` (`body.rs`, `encounter.rs`, `senses.rs`, `lib.rs`),
  `crates/cubarium-voxel-flora` ground pools (carrion). The old lanternjaw lineage is in
  the legacy `cubarium-core` only.
- Canon: `design/0_Canon/README.md` and `DECISIONS.md` before relying on any document.

## Predation: what has to be designed

- **Encounter and strike:** detection (the cone already has a Body class), approach,
  strike reach and timing, success chance (from the prey's state and orientation?),
  handling time. Ambush (the lanternjaw's long stillness, a paid lure) versus pursuit
  (the chorister).
- **Damage and death:** today bodies have structure and reserve, but no health. Decide
  whether a strike kills outright, wounds (a damage stock that costs upkeep), or drains.
  The carcass goes to carrion at the kill site with its full mass (conservation).
- **Feeding from a carcass:** predators and scavengers draw from the carrion pool, which
  exists and which the shredder already eats. One large kill feeding several animals over
  time is the point.
- **Defence:** the prey's side. The loftstrider kicks, the grazer flees; decide
  what a defence costs and does. Vulnerable states (feeding, braced, starving) should
  matter.
- **Sensing and training:** predators need prey in their manifest; prey need a predator
  cue (flight distance). Episodes with both sides, and how that interacts with the
  retrain. Pack coordination (a call channel, roles) is the chorister's later package;
  leave a seam for it, don't build it.
- **Population viability** (large-animals §6.1), **deferred by Wrysk.** His ideas: the tiny
  world may host a different community; or a predator needs only one big meal and then
  rests or hibernates for a long time. Make sure the substrate can express a long,
  cheap rest (low upkeep while dormant) so that option stays open.

## Flight: what has to be designed

- **Movement:** today founders stand on support faces and step (climb rule). Flight is
  movement free in y within a band (the bellwing's 4–20 voxels above the ground), with a
  cost per second airborne above walking. Landing and perching on surfaces (bloomcrown
  cores, stems, frond undersides) and resting there.
- **Occupancy and collision** in air cells: foliage layers, drapes, terrain overhangs.
  The layered plant model already gives foliage bands.
- **Senses in 3D:** the cone fan pitches (−40..+40) were designed for walkers; a flyer
  may need a different fan or a downward cone.
- **Food:** the bellwing's is nectar and pollen from bloomcrown (the biosphere's
  reproductive products; today bloomcrown's parcel is the only product). Decide whether
  nectar is a new flora product or reads the parcel. Pollen carriage (outcrossing) is
  later.
- **Weather:** showers exist (5–15 min). Does rain ground flyers?

## Deliverable

1. A design note (`design/predation-and-flight-<date>.md`, `design_status: proposal`):
   the mechanics above, options where there is a real choice, and a recommendation.
2. Package briefs in the house style (`design/handoffs/voxel-*.md`): objective, files,
   rules that must hold (conservation, paid reproduction, no spontaneous appearance),
   tests to author first, measurements over 8 seeds × presets, return format.
3. A short list of **Wrysk's decisions**, each with a recommendation.

Order suggestion: predation substrate → lanternjaw founder → flight → bellwing. The
loftstrider is its own later package (large-animals §8.3), and the chorister comes after
predation works for one animal.
