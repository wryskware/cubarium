# Cubarium

A persistent artificial ecosystem living on the five visible surfaces of a
64×64 LED cube. Small organisms should acquire recognizable habits and shapes,
alter their surroundings, leave descendants, and sometimes disappear. The world
continues between glances.

**Current state (2026-09-13):** The world is stratified. The four side faces
are soil below a horizon and foliage above it; the top face is canopy. Light
falls with depth, litter falls to the soil floor, rain sweeps the cube and
runs down the walls into puddles, and producers ripen fruit where they are
rich. Four founder kinds (burrower, grazer, glider, skimmer) carry heritable
diet, preferred height, swimming and a rig, sense food two cells out, hold
still while resting, and mutate sparsely at birth. All of it keeps the closed
material accounting and the exact energy and water budgets, on the same
continuous five-face surface and shim transport as before. The design is
recorded in [stratified-world](design/stratified-world.md),
[water](design/water.md) and [fauna-v2](design/fauna-v2.md); the normative
rules live in the [M2 world spec](design/m2-world-spec.md).

`cubarium run --art assets/atelier --sink web` shows it: seven alien plants
with growth stages and fruit, spire trees and glass canes whose crowns cross
onto the canopy, ground cover, pools with shimmer and rain, and the four rigs.
The art is a [Godot 4.7 project](art/README.md) baked headlessly into
`assets/atelier`; see [art/PLANTS.md](art/PLANTS.md) for the plant contract.
Without `--art` the older disc image is drawn unchanged. Known open items:
the skimmer kind does not yet hold its numbers and lineage diversity is thin.
Matched twelve-hour runs now exist; corrected accounting passes the original
limits, but surviving total population does not establish ecological balance.
See the [twelve-hour evidence](design/7_Research/corrected-care-twelve-hour-results-2026-09-13.md).
The [developmental follow-up](design/7_Research/fauna-development-followup-2026-09-13.md)
now locates those eight skimmer losses in the original twelve-seed histories:
initial populations grow, then disappear at about71–120 simulated minutes.
Individual growth/intake/allocation still needs causal investigation from tick0.

The first animation pass adds interpolated poses and turns, softer rain, and
paced, resource-driven plant stages and tree height at 60 fps. Existing vegetation
initializes from the loaded world; ongoing growth is presentation history, not
individual plant age. See the [animation review](design/7_Research/animation-slice1-2026-09-13.md)
and the [ideas and next steps](design/animation-roadmap.md).

The second animation pass added a shared, intermittent breeze with rooted plant
bends and joined tree/vine motion. The original lanternstalk growth pilot has since
expanded to authored transitions for all seven plant species. These remain
presentation changes: ecology advances at20Hz, with60fps output.
See the [second-pass review](design/7_Research/animation-slice2-2026-09-12.md)
for measurements, limits, and remaining work, including higher-resolution ideas.

The web viewer can now mirror the **same owning runner** as the physical cube,
using `--sink shim --mirror-web`. It fans out one encoded frame, not a second
simulation; `/status` identifies the process, state directory and world tick.
See the [verified shared-viewer handover](design/7_Research/shared-viewer-rollout-2026-09-12.md).

Optional feed/rain/litter-cleanup is implemented and enabled on the shared live
viewer at `http://127.0.0.1:7393/`. Open **Care (optional)**, select a location on
the net, and use Feed, Rain or Clean. Inputs have explicit source/export
accounting, schema-7 migration and a bounded durable journal. The final build
passed 774 tests and real-browser checks on a copied world before rollout. See the
[copied-world browser checks](design/7_Research/care-browser-rollout-review-2026-09-12.md)
and [short matched numerical audit](design/7_Research/care-matched-audit-2026-09-12.md).
Zero input preserves autonomous ecology; cleanup removes litter, not a new toxin
pool, and no hands-on autonomy preset has been tuned.

New [care flourishes](design/7_Research/care-flourish-integration-2026-09-13.md)
add smooth local feed crumbs and proportional cleanup flecks to the same shared
frame. Their initial rollout included authored side-plant growth, rooted top-reed wind,
and improved viewer delivery in build `0.1.0+a44dc98`. That rollout preserved
the exact running world and all five existing care inputs; 854 tests passed,
and the live mirror measured about 60 distinct frames/s in isolated Chromium.
See the [schema 9 rollout and rollback record](design/7_Research/presentation-v9-rollout-2026-09-13.md).
Current live build **`0.1.0+d55d8af`** includes adjustable care amounts, authored
canopy growth, readable spiretree wind and continuous actual-intake meal gestures.
It resumed the same world at tick560883;1098 workspace tests passed and the live
shared viewer measured59.9 distinct frames/s. See the
[current update record](design/7_Research/meal-live-update-2026-09-13.md).
An isolated [local care screen](design/7_Research/local-care-observation-2026-09-13.md)
now measures existing biological responses across all twelve mature worlds and
three fixed targets. Feed increased local feeding in 27 of 36 patches; cleanup
usually reduced it and rain was mixed. Actual Resting was nearly absent in this
short screen. A [meal-continuity candidate](design/7_Research/meal-onset-continuity-2026-09-13.md)
is now deployed after matched visual and boundary review. The
[quiet diagnosis](design/7_Research/astra-quiet-results-2026-09-13.md) found no
active-to-rest transitions; an affordable post-birth pause remains a separate
[experimental proposal](design/7_Research/astra-ordinary-quiet-experiment-proposal-2026-09-13.md).
The quiet policy is now being implemented as an isolated opt-in experiment; it is
not yet reviewed, validated, or deployed. No hunter is live.

The first complete [hunter screen](design/7_Research/hunter-profile3-two-hour-results-2026-09-13.md)
ran all twelve seeds in six matched arms for two hours. All 72 numerical audits
passed and the hunting variants made 479 captures, but every introduced hunter
starved and none reproduced. Stock diagnostics and exact reproduction auditing
are now implemented and independently hardened. A matched
[reserve-target experiment](design/7_Research/hunter-reserve-targets-recipe-2026-09-13.md)
has now completed on all frozen copies: captures and average survival improved,
but mature energy readiness and funded gestations remained zero. The
[complete paired result](design/7_Research/astra-hunter-reserve-complete-results-2026-09-13.md)
retains every seed and prey-form outcome. Profile viability, diversity and longer
runs remain open; neither screen is a passed live-introduction gate.

The next [paid-charging experiment](design/7_Research/astra-hunter-paid-charging-proposal-2026-09-13.md)
is implemented as an explicit opt-in hunter policy, leaving the existing policy
and ordinary fauna unchanged. It raises only the battery threshold for spending
reserve on charge, not available energy or reproduction subsidies. Independent
review is complete. Both full matched jobs have now terminated: the background
completed, but the candidate retained a seed6 state-invariant failure. That
failure prevents a full-cohort pass. The
[retained results and exact replay](design/7_Research/astra-hunter-charging-results-2026-09-13.md)
show real paid births but zero adult descendants. The
[reviewed cleanup fix](design/7_Research/astra-hunter-target-cleanup-review-2026-09-13.md)
is frozen at512ee52; both unchanged full twelve-seed reruns have now completed
and pass the strict comparison. They produced18 paid offspring but no adult
descendants. Full biological interpretation is underway; no live introduction
follows from these results, and the original failure remains retained.

Wrysk prefers Fable's **Lanternjaw** megafauna body. The
[comparison studio](art/studies/megafauna/README.md) retains Veilwarden as an
alternate. Lanternjaw now has a world-driven multipart adapter, including scaled
juveniles and real attack/gut/gestation state. Capability validation, runner-event
draining, repeated-observation/reset issues and the reviewed endpoint-heading
defect are corrected; [boundary continuity](design/7_Research/lanternjaw-boundary-continuity-2026-09-13.md)
now carries retained prey across seams and times meal/growth visuals to published
boundaries. Restart and unpublished-path reconstruction remain approximate, and
the hunter stays off the live cube pending viable ecology. Its renderer:
`cubarium::lanternjaw` draws it as a multipart rig through one root-owned surface query
(`cubarium_render::stamp_rig`), exercised by `cargo run --release -p cubarium --example
lanternjaw_study` (port 7399; `--sink png` for native captures, see
`captures/lanternjaw/`). It is not in the atelier pack; hunter membership is explicit,
not selected by form. The [progress record](design/7_Research/living-world-next-progress-2026-09-13.md)
has the geometry, the tests and the measured cost. All seven plant species now carry
both authored growth transitions, including the two top-down canopy plants
([art/PLANTS.md](art/PLANTS.md)).

The [adjustable care dose](design/7_Research/adjustable-care-dose-handoff-2026-09-13.md)
package is live: the capability-gated Gentle/Standard/Generous selector, exact
durable amounts and schema-12 migration passed source, browser and restart checks.
[Natural rainfall](design/7_Research/astra-ambient-support-experiment-proposal-2026-09-13.md)
is a separate ambient-support experiment, not a changed default or a generic
dependence-on-attention control. Its [reviewed harness](design/7_Research/astra-ambient-harness-review-2026-09-13.md)
passed25 focused tests; the frozen3e9bc2f twelve-seed/six-arm two-hour screen has
finished. [Full results](design/7_Research/astra-ambient-support-results-2026-09-13.md)
pass artifact/accounting checks but show additional skimmer losses at90% natural
rain without care. The unchanged24-hour collection is running; the live default
is not reduced. Both canopy plants' opening clips are
also live; missing clips in older/custom packs still use the reveal fallback.
Spiretree wind and actual-intake meal continuity are now live;
glasscane/vine headroom remains limited. The
[AA comparison](design/7_Research/astra-aa-comparison-2026-09-13.md) retains the
current renderer default: blanket coverage baking steadied wings but softened
small bodies too much. The [stable-sail candidate](design/7_Research/astra-sail-stable-body-2026-09-13.md)
combines fin-only coverage with removal of an over-quantized body squash. It is
committed at a88b15b and undergoing independent visual/release review, not live yet.

The [current release backlog](design/implementation-plan.md#current-release-status)
is the concise source for remaining work; dated rollout reports preserve history.

Start with the [design overview](design/README.md), then the
[implementation plan](design/implementation-plan.md). The
[owner's brief](design/brief.md) records the requirements; architecture and
ecological mechanisms are preferred proposals, explicitly marked `leaning`.

The existing display shim at `~/vuzic/led-cube-shim` owns hardware output and
already supplies surface geometry helpers. Cubarium will build on that contract.
See the [local integration findings](design/7_Research/local-contracts.md).

Design authority follows [Lore's vault rules](design/0_Canon/README.md) and the
[decision ledger](design/0_Canon/DECISIONS.md). `.lore.toml` enables `lore-v1` in
`annotate` mode. A detailed proposal is not an accepted decision.

## Development checkpoints and live updates

Wrysk's working preference (2026-09-13): commit scoped work and use lightweight
local `checkpoint/…` Git tags for milestones. Do not make a full executable/art/
state backup for every iteration. Git preserves code and authored assets, not
the running world's history; retain a matched snapshot and care journal only
when a storage migration or other concrete recovery risk requires it.

Periodically deploy verified development improvements to the owning cube/web
runner so progress is visible at a glance. Resume the existing world, confirm
the expected build and advancing tick through `/status`, and keep experimental
ecology opt-in until it meets its acceptance checks. Do not wait for unrelated
backlog items before shipping a ready presentation or interaction improvement.
The shared viewer remains on 7393; stale web-only previews on 7395–7396 may be
stopped after checking their process identity. The display shim is separate.
