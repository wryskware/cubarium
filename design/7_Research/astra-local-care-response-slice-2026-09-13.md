---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# First readable care response: a local meal, not a global wake-up

Source-backed recommendation, not a biological tuning decision. Read against HEAD
`0ae1022` plus the visible in-progress hunter-charging and tallwind edits; those
workers' files were not modified. Canon D-0002 supports an ambient world that does
not depend on attention, not any particular controller change below. Lore retrieval
was followed by direct inspection of the implementation.

## What already responds

| Input | Actual local change | Expected biological response, not guaranteed benefit |
| --- | --- | --- |
| Feed | `World::feed` adds charged detritus D and De, Standard 3 material at the configured detritus energy cap, over one graph hop. | Hungry compatible scavenging diets can sense the edible gradient, arrive, take a meal and eventually rest. Pure herbivores need not respond. |
| Rain | `World::shower` schedules Standard 4 total depth over two hops and 120 ticks / 6 seconds; water is delivered and accounted for by `World::step`. | Immediate rainfall/pools, then potentially changed producer growth and foraging. Non-swimmers slow in water; flooded growth is inhibited. |
| Clean | `World::clean` exports up to Standard 2 material, at most half of each cell's D, with its proportional De. | Less litter and potentially less scavenger food. Feeding may cease and searching resume; there is no toxin-relief, comfort or health state. |

These are totals over graph footprints, not per-pixel amounts. Cleanup does not
remove nutrient N, water, living producers or organisms. It also removes future
recycling substrate; do not describe it as universal fertilization or recovery.
Sources: `crates/cubarium-core/src/care.rs` constants/`footprint`;
`world.rs` `feed`, `shower`, `clean` and tick stages 3–7;
`fields.rs` `react`, `water_factors`.

The ordinary controller receives own-cell P/F/edible-D and local field gradients,
not care identity, rainfall, water depth or a distant target. Sensing follows
`ceil(sense_radius / 4)` graph hops and unfolded cell centers, then normalizes each
food gradient independently. Edible D discounts low chemical-energy density.
Resting wakes only when hunger memory exceeds the individual's `seek_on`; food
presence alone does not wake a satisfied animal. Default memory time is 10 s,
seek-on/off are .3/.1. Actual organisms have decoded, potentially mutated drives.
Sources: `controller.rs::Observation/decide`, `world.rs` observe/decide stage,
`config.rs::DriveConfig::default`, `genome.rs::decode`.

Successful feeding deliberately quiets locomotion: default effort is 1 Seeking,
.2 Feeding and .05 Resting; turn fractions are 1/.1/0. Water further divides speed
by `1 + depth * (1 - swim)`. Food requests settle at the **post-movement** cell,
after decisions observed the pre-movement cell, and are limited by reserve
headroom, Type-II intake and shared available stock. More crumbs cannot imply
proportionally faster mouths or stronger steering after gradient normalization.
Sources: `controller.rs::decide`, `world.rs` movement/feeding settlement stages.

## Why a response could look weak

These are plausible explanations from source, not measured causes of the current
live user's experience:

- The selected patch may lack hungry compatible animals within their actual local
  sensing range, or already offer plentiful food. Keep these zero-opportunity
  cases; do not move the target after seeing the result.
- A small dose may not clear individual feeding thresholds in every footprint
  cell. In an ordinary four-neighbor interior, a 250-permille feed adds .25 D at
  the center and .125 at each neighbor before reactions; the default feed minimum
  is .2. Existing stock/energy density, reactions and actual drives decide the
  gate, so this is not a universal dose cutoff.
- A genuine response may be **more eating and less travel**, followed by a quiet
  rest. Rain's plant payoff is slower than its six-second shower and can be
  negative in an already wet patch.
- `ArtPresenter::state_of` maps Feeding to a looping feed clip, with gestation
  taking priority. It ignores `OrganismView.fed` and deliberately omits the older
  procedural feeding flash. Clip time is world time plus a stable individual
  phase, not a newly admitted meal's onset. This can make a new meal hard to
  distinguish from what was already playing. Conversely, Feeding mode can request
  **zero** food at full reserve. The existing `a_full_organism_requests_nothing`
  test proves that mode is not an ingestion counter.

`World::render_view` already publishes `fed_this_tick` and transported movement
segments. It publishes raw D but not De, so its soil/litter appearance alone is
not an edible-food-quality display. Art plants are field-driven presentations,
not individual biological plants with a new hydration/comfort controller.

## Small next implementation: observe isolated pulses, then show meal bouts

Implement the observer in the copied-world example first, with no controller,
resource, RNG, schema or live changes. The current mixed recipe (Feed at 0, Rain
at 60, Clean at 300 ticks) confounds a meal response after just three seconds.
Retain it as combined-care stress evidence, not causal evidence for one action.

Clone each fixed existing mature opening into untreated / Feed-only / Rain-only /
Clean-only, one Standard pulse at the same boundary/target, unchanged ambient
settings and no hunters. Record seed/opening hash, target/footprint, receipt,
actual dose and mechanism settings. Use the same prescribed ordinary/seam/rim
targets in all arms, not a post-hoc responsive patch. A short two-minute screen
is practical; rain/producer follow-up may extend separately to ten minutes.
Report paired per-seed differences and zeros, not only a pooled activity score.

Minimal tickwise accumulators and fixed snapshots at 1/3/6/15/30/120 seconds:

- Local actual-fed organism-ticks, mode organism-ticks and Feeding→Resting
  transitions, with continuous Resting duration/max bout. These separate meal
  activity from travel and later quiet. Label `fed` as **any field intake**, not
  specifically scavenging, an amount, or proof it was a manually deposited crumb.
- Sum transported path-segment lengths, not raw face coordinates/end-point
  subtraction across seams. Report local occupancy and arrivals/exits separately.
- Publish both fixed pre-pulse local-ID outcomes (full slot+generation; dead or
  absent IDs retained as such) and dynamic local organism-time denominators.
  Stratify initial diet, hunger memory/mode and reserve headroom. A fixed spatial
  footprint plus two-hop observation halo is a report region, **not** a claim
  that every resident senses the food; record individual sensing reach if making
  an opportunity claim. No denominator for zero exposure means null, not zero %.
- Local D, De and computed edible D; P/F; water peak and depth-above-flood
  cell-time. Keep world totals too: water can flow beyond the selected patch.
  Rain acknowledgement/delivery is distinct from growth benefit, and a cleanup
  export is distinct from an increase in animal motion.

Use existing strict inventory/receipt/independent-energy audits. Reserve deltas
are **not** measured intake: oxidation, maintenance, growth and reproduction also
change stores. Existing telemetry lacks per-diet intake amounts. Do not add a
large transaction API just to obtain the first yes/no meal comparison; add narrow
transient settlement totals later only if that ambiguity blocks a real decision.

If meals increase but remain visually ambiguous, the first production candidate
is a small **actual-intake-driven meal onset/settling presentation**, using the
existing per-organism `fed` signal and authored feeding pose. Observe every tick,
key bounded presenter memory by full ID, and trigger on a bout onset rather than
restarting every fed tick. Respect gestation and existing hunter presentation;
continuous eating should remain an individual loop, then hand back to the actual
mode/quiet pose. Choose the short visual duration by a paired capture, not as a
new biological timer. No new flash, mandatory crowd arrival or global speedup is
needed. This represents naturally occurring meals too: unchanged **ecology** is
guaranteed by a presentation-only change, not byte-identical no-input imagery.

Do not lower hunger thresholds or increase movement merely to force a flourish.
Such generic biological changes also alter autonomous behavior with no care.
A truly care-only biological alert would require an explicit local persisted
stimulus and replay semantics, not secretly changing hunger or revealing distant
input; that is unnecessary scope for this first slice. Quiet habits already have
real hunger hysteresis, reduced effort and still headings—make their contrasts
readable before adding another state machine.

Ambient support remains independent. The earlier
`astra-ambient-support-experiment-proposal-2026-09-13.md` isolates natural rainfall;
do not change it during this response screen or call less rain “less autonomous”
before measuring flooding as well as growth. Input dose is not animation gain.

## Bounded verification

Foreground current-source tests: controller rules 6, settlement 2, controller
unit tests 6, energy-free-detritus rejection 1, wetting/flooded-growth 1: **16
passed, 0 failed**. They verify the cited mechanisms, not a live response rate or
the proposed presentation. No experiment was run and no production file edited.
