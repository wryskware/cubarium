---
status: open, not dispatched
date: 2026-09-20
owner: Wrysk decides when and whether; Fable briefs
---

# A soil organic-matter pool: how it would integrate with the whole ecology

Wrysk, 2026-09-20: explore how adding a soil pool would integrate and affect
the overall ecology and simulation; it would not exist solely as a resource
for glowcaps. This is the brief for that exploration. It is **not
dispatched**; it waits for Wrysk's go.

## Why the question arose

The generated world with the closed water cycle keeps five of six plant
species alive for six hours and grows 376 stands from 62 seeded
(`design/7_Research/voxel-census-2026-09-20.md`), but glowcap, the only
decomposer, dies out at five hours. Its income rule is saprotroph uptake
from the **dead-wood** pools in its mycelium box, and its establishment gate
is dead wood in that box (`crates/cubarium-voxel-flora/src/lib.rs`, `Trophic::
Saprotroph`, `substrate_uptake_per_s`, `establish_substrate_min`,
`dead_wood_in_box`). Dead wood comes only from the eight seeded logs and from
stands that die back or die (`step.rs`, `ground[gi].dead_wood += die_back`
and `g.dead_wood += stand.wood`). When the water cycle keeps plants alive,
nothing dies, the logs are eaten out, and the decomposer starves. Package S2
(`voxel-decomposers-and-defaults-2026-09-20.md`) is adding **litter** to its
diet; take that as the baseline. Litter is otherwise the littershredder
founder's pool (`take_litter`); carrion has its own pool and a
`carrion_decomposition` rule; each pool carries organic, mineral and energy
companions. There is no soil organic-matter pool.

Real soils hold most of an ecosystem's organic matter, and most decomposition
happens there, by fungi and microbes, releasing minerals that plants take up
again. In the model the plants' mineral pool may have no source at all
beyond the seeded stock; that is one of the questions below.

## What the exploration must answer

Read-only against every crate. Deliverable: one design note,
`design/soil-organic-matter-exploration-2026-09-20.md`, front matter
`design_status: exploration`, at most 250 lines, every claim about the code
with a `file:line` pointer. Read first: `design/ecology-v1-contract.md`,
`design/terrain-and-ecosystem-proposal-2026-09-16.md`,
`design/voxel-ecology-sketch-2026-09-16.md`, the flora crate's ground pools
and their accounting, `design/0_Canon/DECISIONS.md` for anything binding on
soil or nutrients, and `design/7_Research/voxel-tick-profile-2026-09-18.md`
for the measured tick.

1. **The flows today.** A text diagram of organic and mineral matter: what
   creates it (light fixation), the pools (living tissue, foliage, litter,
   dead wood, carrion, animal bodies, seed banks), who draws on which, what
   leaves the world (respiration, transpiration, the outlet in the open
   budget), and where the ledgers close. Name the mineral pool's sources
   today; if the seeded stock is the only one, say so, because then every
   world is mining a finite mineral reserve.
2. **Where a soil pool would sit.** Per soil voxel, per column, or per
   support face beside the existing ground pools. What feeds it: litter
   decay, carrion decomposition, dead-wood decay, root turnover on stand
   death. What draws on it: saprotrophs, mineralisation into the mineral
   pool that plants take up, and nothing else. What the contract already
   implies for rates, and what would be a new placeholder
   (`design/backlog.md` §1 lists the placeholder rule).
3. **Effect on the whole ecology.** Plant mineral nutrition and whether the
   pool closes a loop the world currently leaks; the shredder–glowcap
   competition once both eat litter and only one can eat soil; the
   decomposer's floor when nothing dies; whether a slow soil pool damps the
   boom-and-crash the founders show; what happens to litter tiles that today
   are the shredder's only food. Say which existing flows must change so
   that matter is not counted twice.
4. **Simulation cost.** State per voxel or per column; which tick phase it
   runs in (the flora step already walks the ground sites); whether it needs
   diffusion between columns or only local decay and vertical exchange with
   the water; an estimate against the measured tick, and whether it must be
   sparse like the water's active set.
5. **Presentation and game.** One paragraph each: soil colour or texture
   following organic content (the art direction is Wrysk's; describe what
   the data could drive, not the look); mushrooms appearing where the soil
   is rich; and the game lever of a player enriching or depleting soil,
   alongside the water lever already decided.
6. **Recommendation.** Two or three bounded packages with one-sentence
   tests, and a plain list of what Wrysk would be deciding: the pool's
   granularity, whether mineralisation is the plants' mineral source, and
   whether the shredder also eats from it.

## Rules

No crate edits. Preserve modality: nothing in the note is a decision; a
proposal is not accepted canon. No knob tuning proposals dressed as model
changes. The note is committed by explicit path with the
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>` line. Return
≤ 40 lines to Fable: the section list, the findings that most change the
picture with their evidence, the mineral-source answer, the tick estimate,
and the packages in one line each.
