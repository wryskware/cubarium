---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Fresh-thread handoff and progress

This is the current task index, not a new design decision. Priority is suggested;
Wrysk can choose any slice. Read the repository instructions, [working policy](../../WORKING_POLICY.md),
[canon rules](../0_Canon/README.md), and relevant [ledger](../0_Canon/DECISIONS.md)
entries, then **one** task brief. Do not ingest the whole research archive.

## Where we landed

**2026-09-14 planning update:** Wrysk has requested recurrent organism control
directly, without an MLP comparison. Review the research-backed
[recurrent-organism plan](../recurrent-organism-plan.md) and its
[movement/foraging overview](../movement-and-foraging-plan.md). The proposed next
handoff is R0: physical movement, local depletion/recovery and the senses/actions
contract. Implementation/delegation follows review; M1 sustainability-search
changes follow the behavioral redesign. The status below is historical.

For an early Opus thread while Fable's review is pending, use the narrower
[R0a movement-foundation handoff](r0a-movement-foundation-2026-09-14.md).
It separates physical movement fixes and food measurements from the remaining
design choices.

Source baseline: `0825d8b`, tagged `checkpoint/current-checkout-fresh-world-2026-09-13`.
The cube was restarted from tick zero with that build, normal `assets/atelier`,
speed 1, care enabled, and the same-instance viewer at `http://127.0.0.1:7393/`.
Ecology advances at 20 Hz; interpolated presentation targets 60 fps. A five-second
browser check after reset measured 59.8 unique frames/s; that is not physical-panel
scanout evidence. The docs-only handoff commit does not require restarting this world.
Runtime status is a dated observation: use `/status` to verify the current owner.

Delivered in main:

- One owning runner feeds the physical cube and web viewer. Feed, Rain and Clean
  have independently selectable doses, real resource accounting and durable input
  replay. Clean exports litter; there is no separate toxin pool.
- Pose/turn interpolation, paced plant transitions, intermittent rooted breeze,
  flooded-top reed motion, spire/vine sway and corner crown-growth continuity.
- Both authored growth transitions for all seven plant species; resource-driven
  height/crowns. This is not persistent individual plant age or senescence.
- Stable sail swimming bodies, calm rest/feed fins and actual-intake meal continuity.
  Blanket coverage-AA was not selected; selective AA remains an experiment.
- Wrysk's selected Fable Lanternjaw multipart body and world adapter exist.
  Hunters are **not enabled** in the live world: viable recruitment is unresolved.
  Veilwarden remains an alternate design.

The old world lost diversity and looked plant-poor. Its reset is operational
cleanup, **not an ecology fix**. Total survivors, accounting tests, and aggregate
producer biomass do not establish a healthy or visibly planted ecosystem.

## Remaining tasks

| ID | Suggested order | Bounded direction | State |
| --- | --- | --- | --- |
| [01](01-ecosystem-health.md) | First | Explain ordinary fauna loss and sparse visible vegetation | Diagnosis incomplete; no corrective tuning accepted |
| [02](02-quiet-habits.md) | After / alongside 01 if scoped separately | Find affordable, genuinely observable quiet behavior | Birth-only candidate too sparse; intake-shadow study unmerged |
| [03](03-lanternjaw-lineage.md) | After basic prey health | Make the selected apex lineage recruit without subsidies | Size-gate candidate does not establish viability |
| [04](04-glasscane-motion.md) | Independent visual slice | One readable glasscane wind/flicker improvement | Interrupted study source, no accepted production candidate |
| [05](05-care-and-rain.md) | Independent care slice | One restrained, readable response to actual care | Core interactions shipped; rain flourish still experimental |
| [06](06-autonomy.md) | After unattended health | Evaluate ambient support independently of manual dose | Default unchanged; lower-support evidence has adverse outcomes |

Choose one brief, not all six. Example fresh-thread prompt:

> Read AGENTS.md, WORKING_POLICY.md, design/handoffs/README.md and
> design/handoffs/01-ecosystem-health.md. Work only on its first bounded milestone.
> Preserve unrelated edits, report the evidence and remaining uncertainty, and
> commit/tag and push the resulting scoped work. Follow the deployment policy
> if runtime code or assets change; do not recreate the old capture archive.

## Evidence and source retention

On September 13 Wrysk authorized deleting the old world and generated evidence:
`captures/` went from about 88 GB to zero, `target/` from about 32 GB to 2.2 GB,
and 26 linked worktrees were removed. Git commits, checkpoint tags, study code,
and already committed compact reports remain. There are no backup copies of the
discarded untracked data. Old tracked artifacts may still exist in Git history.

Reports under `7_Research/` describe their own historical checkpoints. Claims that
jobs are “running,” raw outputs are “preserved,” or a frozen release is “live” are
not current instructions. A process exiting successfully is not biological review.
The remaining fauna cohort and 24-hour ambient outputs were discarded without a
completed interpretation; do not invent conclusions or assume their old paths work.
Input-dependent study checks cannot be rerun without new, explicitly bounded data.

The [interrupted-source checkpoint](../7_Research/interrupted-review-checkpoint-2026-09-13.md)
records source saved in `6000750`. Isolated source is recoverable from these tags:

| Study | Git ref (not a directory or deployed feature) |
| --- | --- |
| Post-intake shadow | `checkpoint/source-post-intake-shadow-2026-09-13` |
| Fauna development flow | `checkpoint/source-fauna-development-flow-2026-09-13` |
| Hunter size gate | `checkpoint/source-hunter-size-gate-2026-09-13` |
| Birth-only quiet | `checkpoint/source-quiet-screen-2026-09-13` |
| Glasscane study | `checkpoint/source-glasscane-wind-frozen` |

Inspect the selected source before deciding to port it. Do not merge a study
branch wholesale or recreate every old worktree/cohort. Use one normal build
cache, compact result summaries and disposable outputs. Ask before generating
more than 1 GiB. Commits and tags are the requested history, not copied releases.

## Later creative directions

- Persistent plant life history: sprout, height, budding/flowering, wilt and
  resource-paid regrowth. Distinguish biological age from the growth clips already shipped.
- Silhouette-scale fruiting/seed-release events and inherited visual variation,
  if tied to real resources and readable at 64 px.
- More varied quiet postures and restrained local care rituals, after their
  biological triggers exist; no global wake-up effect or mandatory maintenance.
- Selective thin-stalk AA only if a native-size temporal comparison shows a gain.
- Leaf droplets, nibbled foliage and richer high-resolution/LCD presentation are
  deliberately lower priority. An LCD cube remains exploration, not a hardware migration.

The [animation roadmap](../animation-roadmap.md) retains earlier ideas and dated
slice notes. These possibilities are not obligations to implement every detail.
