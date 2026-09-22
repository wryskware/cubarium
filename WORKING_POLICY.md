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
  frames at most. No pinned hashes of worlds or configs, no provenance /
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
  that workflow.
- **Worlds and snapshots are disposable.** Do not ask whether it is OK to reset
  a world; reset, redeploy, delete run output freely. Nothing on the cube or
  the panel is a production run.
- The ecology training tooling (`cubarium-search`) stays in the workspace; it
  may be reused or rewritten when the ecology changes. Live knobs and hot
  reload are undecided; restarting to reload is fine.

## Training resources — correction, 2026-09-18

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
