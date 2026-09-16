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
