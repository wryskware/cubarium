---
status: open
date: 2026-10-03
owner: opus-medium worker, worktree `.claude/worktrees/savenow`, branch `savenow`
---

# Save the whole world on demand

Wrysk, 2026-10-03: "we should probably add the ability to export/save state even if its not
started with state persistence. like a console or api command.. or later a menu with 'save
game'".

**Today:**
- `cubarium voxel` saves the layered snapshot (`world-N.voxel`, `flora-N.bin`, `fauna-N.bin`)
  only when it was started with a state directory.
- The stdin console's `w PATH` (`crates/cubarium/src/voxel/mod.rs`, around line 2861) saves
  terrain and water only.

## Build

1. **Console:** `save [DIR]` writes the full layered snapshot (world, plants, animals) into
   `DIR`.
   - The default is `saves/<YYYYMMDD-HHMMSS>-t<tick>/` under the working directory.
   - It uses the existing `save_voxel_snapshot` layout, so `cubarium voxel --load DIR` resumes
     it directly.
   - Rename the old terrain-only save to `save-terrain PATH`. `w` becomes `save`.
   - Print the path on stderr.
2. **Signal:** `SIGUSR1` triggers the same save into the default directory. That works for
   services (voidrunner and Tachyon run as user services, with no stdin):
   `systemctl --user kill -s USR1 cubarium` or `kill -USR1 <pid>`. Handle it with a flag the
   loop polls; do nothing in the handler itself.
3. **Window key:** if the desktop window sink already receives key events, `Ctrl+S` saves. If
   it doesn't, skip this and say so. A menu is Wrysk's later UI work.
4. **The save runs off the frame,** as `SnapshotWriter::save_in_background` does. A request
   while a write is in flight is reported, not queued silently.
5. **Docs:** a short section in the README or the voxel run docs.

## Tests

- The console `save DIR` on a small founded world writes a directory that loads back with the
  same tick and the same plant and animal counts.
- The default directory name has the tick.
- The signal flag triggers exactly one save.
- `save-terrain` still writes terrain only.

## Constraints

- **Worktree:** `git worktree add .claude/worktrees/savenow -b savenow main`. Commit with
  explicit paths only. Never merge, deploy, or restart a host.
- **Never put a window on screen.** Tests are headless.
- **Stay in `crates/cubarium`.** Other workers are on flora, fauna and `cubarium-search`.
- Use `nice -n 10` and `-j 8`.
- Never `pkill -f` a pattern in your own command line. Leave no background shells.
- End each commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01BGC2SEjYCJTk6fgfNZcrru
  ```

## Return (≤ 25 lines)

Branch and head; the commands, the signal and the key (if any) as built, with `file:line`;
the tests; one example save path.
