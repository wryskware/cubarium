# Working policy — Wrysk's correction, 2026-09-13

This records the owner's explicit working preferences, not a new ecological design.
It overrides older agent-authored plans that assume indefinite retention, repeated
independent reviews, or a permanently pinned release workflow.

- Before delegation or extended autonomous work, read the personal
  `/home/wrysk/.codex/skills/bounded-agent-work/SKILL.md`. Fresh bounded contexts,
  narrow ownership, one targeted review, explicit spending checkpoints. Do not
  resume hundred-thousand-token histories for small assignments.
- The cube is a development display: run the latest development checkout with
  normal `assets/atelier`, not art or executables from `captures/` release copies.
  Rebuild/restart after relevant checks when shipping changes. Keep an accurate
  build ID, but do not use it to justify retaining an old program. Unfinished
  experimental switches can remain off without freezing the entire deployment.
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
