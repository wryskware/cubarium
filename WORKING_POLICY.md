# Working policy — Wrysk's correction, 2026-09-13

This records the owner's explicit working preferences, not a new ecological design.
It overrides older agent-authored plans that assume indefinite retention, repeated
independent reviews, or a permanently pinned release workflow.

- The cube is a development display: run the latest development checkout with
  normal `assets/atelier`, not art or executables from `captures/` release copies.
  Rebuild/restart after relevant checks when shipping changes. Keep an accurate
  build ID, but do not use it to justify retaining an old program. Expose usable
  new behavior through ordinary development controls; ecological balance is not
  a gate for access to features on this development display.
- Source history belongs in Git commits and lightweight tags. Old captures,
  disposable experiment data, frozen binaries and redundant build caches are not
  archival deliverables. Use one normal build cache; clean task-owned temporary
  artifacts at the agreed end of the task. Ask before generating over1GiB more.
- Preserve uncommitted authored work before removing any worktree. Verify that a
  cleanup target is not needed by a live process. Existing-data cleanup still needs
  explicit scope; do not erase the repository, shim, or unrelated user files.
- World continuity is useful, not sacred. Wrysk reports that the current world has
  lost variety and vegetation and expects a fresh run. A requested reset must not
  be blocked by an agent-invented preservation requirement. Do not hide ecological
  decline behind a healthy process, passing numerical audits or smooth frame rate.

Wrysk subsequently authorized the cleanup and fresh-world deployment. Use
`./scripts/run-cube.sh` to rebuild the current checkout and launch normal assets;
stop the existing owner first. It does not silently erase state or auto-watch
source files. Old raw captures are deliberately discarded, not missing backups;
historical reports must not imply they can still be revalidated from local data.

## Development access and ecology — correction, 2026-09-14

Wrysk wants new features available to try as this young project evolves. Add
ordinary controls when an action should be explicit: apex spawning should offer
one or two predators at random locations and enable their complete lifecycle.
Do not require users to discover internal experimental policies before using
implemented features. Ecological failures are evidence for iteration, not a
reason to hide the feature. Preserve meaningful correctness checks and use Git
for recovery; do not invent production-release gates.

Tune the whole ecosystem together. The next research direction is fast headless
simulation with genetic search over ecological parameters, evaluated on
sustainability and variety across the world. Keep search work bounded and
measure throughput before selecting a compute budget; do not launch an
unbounded parameter search or tune predators against a frozen ecology alone.

## Fast iteration — correction, 2026-09-16

Wrysk: this is "a zero stakes art project ... little pixels wiggling around on a
screen for me to watch", closer to prototyping a game than to systems
engineering. Iteration speed is priority one. Clean code, fast and dirty process.

- **Tests check that a function works**: build the smallest thing, run a few
  frames at most. **Never bake tuned numbers into a test.** That means no assert on a
  default value, a current ratio, population or score, or a pass/fail verdict on
  today's defaults. Check the mechanism on hand-built inputs, against whatever the
  config says, so retuning never breaks a test. No pinned hashes of worlds or configs, no provenance /
  adoption / migration / regression suites, no test that steps a world past a
  few hundred ticks. A long run someone wants is an `#[ignore = "study: run by
  name"]`. `cargo nextest run --workspace --exclude cubarium-gpu` is the check
  and should stay under a few seconds; `.config/nextest.toml` flags anything over
  two seconds. (The 2026-09-16 cull: 2,019 tests / 154 s serial → 1,502 / 2.3 s.)
- **Run only the crate you touched.** Never the full suite for a doc or plan
  commit.
- **The commit message is the report.** No evidence tables, no report file per
  package unless Wrysk asks. Briefs stay short.
- **Astra review rounds are opt-in**: only when Wrysk says a thread should use
  that workflow. Never route work to Astra on your own; it is not a lane in the
  model routing. When asked, run it through codex-lane: `dispatch` with
  `model: "gpt-6-astra"`, effort `high` (`xhigh` for the hardest reviews), and a
  self-contained brief. Astra is for adversarial review of the most complex
  work, for research, and for reviewing research; not for implementation. A
  review brief says: read-only, don't edit or commit (this workspace runs
  codex-lane under `danger-full-access`), and return findings ranked by
  severity, each with its evidence. A research job may write its report to
  `design/7_Research/astra-<topic>-<date>.md`.
- **Worlds and snapshots are disposable.** Do not ask whether it is OK to reset
  a world; reset, redeploy, delete run output freely. Nothing on the cube or
  the panel is a production run.
- The ecology training tooling (`cubarium-search`) stays in the workspace; it
  may be reused or rewritten when the ecology changes. Live knobs and hot
  reload are undecided; restarting to reload is fine.

## Training resources — correction, 2026-09-18

### Simulation study CPU placement — Wrysk, 2026-10-06

Keep each multithreaded simulation study (censuses, probes, staged arms and soaks)
within one shared L3 cache group. Distribute independent runs between groups;
do not rely on the scheduler to keep a process's threads on one chiplet.

- Launch through `scripts/run-study.py --slot N -- <headless command...>` on the
  execution host. Use the zero-based arm/run index as `N`; it wraps over the
  available groups. `--list` shows the topology. The wrapper detects sysfs L3
  sharing, respects inherited CPU restrictions, and applies affinity before
  execution so new threads and children inherit it. It refuses to launch if it
  cannot determine the topology. Programs must not widen this inherited mask.
- For SSH runs, invoke the wrapper **on the remote host**, with that host's copy
  of the script; desktop CPU numbers must not be sent to eidolon. With standalone
  uploaded study binaries, upload this wrapper alongside them.
- Set the study's worker/pool options to fit its assigned group; wrapping does
  not shrink an explicitly sized pool. Budget concurrency across simultaneous
  runs, counting shared groups rather than granting each process the full host.
  The existing CPU reserve, memory limit and staged-run rules still apply.
- Slots distribute a batch; they do not reserve CPUs or coordinate separate
  batches. Check existing jobs and the live world's affinity before choosing
  slots or concurrency. On the desktop, prefer the other group when the live
  world or a game occupies the V-cache group.
- Verify every thread's `Cpus_allowed_list` after launch. Existing running jobs
  are not changed automatically. A deliberately cross-chiplet benchmark may
  bypass the wrapper, but state that choice in its command or brief.
- Independent episode training is exempt: workers simulate separate worlds and
  may use both chiplets under the existing resource budget and `--pin` policy.
  The live world's existing host-profile placement remains in use.

Wrysk authorizes explicitly launched offline training and parameter-search runs
to use up to 90% of the machine's logical CPUs and 64 GiB of memory. Determine
the CPU worker ceiling from the machine at launch, leaving at least 10% and one
logical CPU free, and apply a 64 GiB hard address-space limit to the training
process. Check current system load before a long run and reduce concurrency if
the machine is no longer otherwise idle. Keep the experiment's wall-time and
episode limits; this resource allowance does not authorize unbounded searches
or make ordinary tests larger. The voxel episode trainer is capped at 16 workers:
measurements on this machine showed diminishing returns above that point.

## Lean orchestration — correction, 2026-09-17

Wrysk: tokens are getting tighter; cut the parallel overhead of agents getting
up to context. "If things are slow, I'll ask for more agents explicitly."

- **Persistent workers, resumed by message.** One worker per area (the flora
  crate and its harness; the fauna crate and the host; the renderer) keeps its
  context and takes the next package by message. A fresh spawn is the
  exception and needs a reason (a genuinely independent area, or an
  independent test pass on a model-rule change). Parallel workers on the same
  crate only when Wrysk asks.
- **Briefs by pointer.** A brief names the review item or design section and
  states the decisions made; it does not restate them.
- **Returns capped at about 40 lines**: commits, the numbers that changed a
  decision, defects with evidence. No API dumps, no tables that were not asked
  for. The commit message is still the report.
- **Effort by kind.** High only for model-rule changes and anything
  concurrency-shaped; medium for repairs, reporting and harness work already
  specified line by line.
- **One experiment section per Astra round**, not per package: a short table,
  appended once. Smoke runs still happen; long write-ups do not.
- **Verification once.** A worker runs the crate it touched; the integrator
  runs the full suite once at merge.
- **The integrator reads skeletons and diff stats, not source.** Reading a
  whole file into the top-level context is the most expensive single act.
- The independent test-authoring pass on a model-rule change stays.

## Art direction — Wrysk, 2026-09-17

`design/art-direction/Cubarium_Art_Direction_v0.1.md` is the art direction.
Every brief that touches the presenter, the GPU renderer, palettes, glyphs,
organism bodies or motion cites it and does not invent look; agents implement
it and make no taste calls. The current voxel autotile presenter and the
interim glyphs are **dev mode** (stage 1) and stay available as a diagnostic;
production art is stage 2 → 3 of that document. A package needing a look the
document does not cover stops and asks, shipping an interim glyph named as
interim. Assets are AI-generated and selected by Wrysk; workers do not generate
production art on their own.

## Lean delegation and Tachyon builds — correction, 2026-09-21

Wrysk requests that the main thread stay lean: delegate bounded implementation
and verification to appropriately tiered persistent workers, then integrate
concise results. Tachyon deployment must stop `cubarium.service` before native
compilation to free board resources.

## Verification runs — correction, 2026-09-29

Wrysk: 4 h and 6 h censuses are "a long time to wait to verify every ecological
edit", so future workers must not start with them. The audit
(`design/7_Research/census-length-audit-2026-09-29.md`) found:
- most verdicts were clear within 60–120 min;
- no 6 h run reached a steady state;
- single-seed end numbers were noise.

- **Check t = 0 first.** If the founding state already decides the question, don't
  run: for example, a species with no root water or zero growth rate, or stay-or-go
  numbers out of band.
- **The default check for an ecological edit is 8 seeds × 2 h, in parallel.**
  - Judge the spread across seeds.
  - Stop a seed's run early once its verdict is settled.
  - Until the census package lands, `voxel_census` still defaults to 6 h, so pass
    the hours explicitly.
- **No single-seed 4–8 h census per edit.** A run longer than 2 h needs a stated
  reason: a clock slower than 2 h that the edit actually touches (lifespan, seed
  banks, dead wood).
- **Edits to slow clocks branch from a saved world.** Use
  `voxel_census save=`/`load=`, then run 1–2 h beside the baseline continuing from
  the same save. Don't found fresh and wait out the founders' lives.
- **Long runs are soaks, off the critical path.** 8 seeds × 8 h on eidolon, overnight
  or per merge batch. A soak never blocks an edit.

## Staged runs and scenario maps — Wrysk, 2026-10-04

A 5 h study batch ran for 2 h with every animal frozen: the study's load path dropped
their controllers. Nobody looked until the end. Wrysk: "we absolutely cannot be doing
multi-hour runs without doing pause or checking in or reevaluating."
- **Every run is staged.** It saves a checkpoint each segment and can be paused, resumed
  and branched from it.
- **A scripted liveness check** runs after every load and after the first short segment
  (a few sim-minutes). It checks that:
  - animals move and eat;
  - each mechanism under test fires (its counters are nonzero);
  - the ledgers conserve.

  If it fails, the run stops and reports. Nothing runs on unattended.
- **Re-evaluate before continuing.** Read the first segments' readouts, then continue,
  stop arms that are clearly out, or fix the setup.
- **Pick the map for the question.** Small (Tachyon-size) maps suit plant and grazing
  questions. Predators need larger ones. Branch from saved starts rather than founding
  each time.
- **Target:** one ecological change answered in under 30 minutes.

## Every number is a knob — Wrysk, 2026-10-01

A large tuning pass over all parameters comes later, so don't tune new mechanisms to
today's failing ecology. Give them plausible defaults instead. Write every ecological
number as a tunable knob:
- **Where it lives:** a named, serde-visible config field with a doc comment that gives
  its unit and meaning.
- **What's not allowed:** inline literals, and new `const`s for ecological numbers. Only
  pure numerics stay `const`: tolerances, epsilons, cache sizes.
- **Registry:** each new field gets a row in its crate's registry table
  (`cubarium_voxel::registry`, the tables built by R1). The coverage test fails on a field
  with no row. Regenerate `design/parameters.md` with
  `cargo run -p cubarium-voxel-sim --example params -- --write design/parameters.md`.

## Balance through roles — Wrysk, 2026-10-05

"the whole purpose is to have other organisms that fill a niche which balances the ecology. like too
many nutrients locked in a mushroom infestation? something should be thiving that eats them because
theyre so abundant. this idea should go for everything. ... numbers arent always enough. we need to
think about behaviours that fix the problems as well, or whether the ecosystem is missing roles."

- **An imbalance is first a question about roles.** When a stock grows or shrinks without check
  (litter, a fungus, a booming prey, a retreating plant), ask three things:
  - what in a real ecosystem would thrive on that abundance and pull it back;
  - whether that role exists here;
  - if it does, why it isn't answering: it breeds too slowly, can't find the food, has the wrong
    diet, or lacks a behaviour such as switching to abundant prey or breeding faster when fed.
- **Rate tuning comes last.** Fix the behaviour, or add the role.
- **Reports and briefs:** every ecological report names the stocks that are growing or shrinking
  without check, and the role that should answer each. Every ecology brief restates this rule.

## No unsafe code — Wrysk, 2026-10-08 (hard rule)

"NO unsafe code allowed in the repo without my approval. external library code excluded."

- No `unsafe` block, `unsafe fn`, `unsafe impl` or raw-pointer trick in this repository's code
  unless Wrysk approved that specific use. Code inside external crates doesn't count: prefer a
  safe wrapper crate (rayon chunks, native atomics, `nix`/`core_affinity`-style syscall
  wrappers) over writing `unsafe` here.
- Crates with no approved `unsafe` carry `#![forbid(unsafe_code)]` at their root.
- Existing `unsafe` (2026-10-08): `cubarium-gpu` (raw Vulkan through `ash`), `cubarium-rules`
  (water host, `math.rs`), `cubarium` (`save.rs`, `placement.rs`, `chiplet.rs`, `state.rs`,
  the web sink, `render_bench`), `cubarium-search` (CPU pinning, perf counters) and
  `cubarium-godot` (gdext's required `unsafe impl ExtensionLibrary`). It is either removed or
  listed here as approved; until then, no new `unsafe` lands anywhere.
- Every brief that touches these crates restates this rule.
- **Approved (Wrysk, 2026-10-08, "option 1 ... 2-4 above all sounds fine"):**
  - Vulkan calls through `ash` inside `cubarium-gpu`, including new ones such as H2's model
    pass, until the renderer moves to wgpu (backlog item 11). Only the FFI call itself sits in
    the `unsafe` block; nothing else in that crate may use `unsafe`.
  - `cubarium-godot`'s one `unsafe impl ExtensionLibrary`, which gdext requires.
  - Everything else goes: CPU pinning, perf counters and topology move to safe wrapper crates,
    and `save.rs`, `placement.rs`, `state.rs`, the web sink and `render_bench` become safe code.
