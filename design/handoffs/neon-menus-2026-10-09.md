---
status: open
date: 2026-10-09
---

# The app's menus in the neon style, first pass (2026-10-09)

**Rules.** `design/handoffs/worker-rules-2026-10-07.md` applies in full, with AGENTS.md and WORKING_POLICY.md. The hard ones:
- **No windows.** Every Godot run goes through `scripts/hidden.sh`.
- **The 5-minute rule.** Wrap everything in `timeout 270`. Never poll, sleep-loop or wait on a job; anything longer goes detached and you hand back.
- **Never touch main's `game/bin`.** Wrysk runs the app from it. No Rust changes are needed. Copy main's `game/bin/libcubarium_godot.so` into your worktree for runs.
- **Fonts are gitignored.** Copy `/home/wrysk/wryskware/cubarium/game/fonts/*.ttf` into your worktree, then import headless.
- **Commits.** Explicit paths only. Trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Don't merge.

## Objective

Wrysk: "go ahead and start converting more of the interface using the new style". The neon look (D-0006, `design/ui/design-language.md`) exists as a theme and resources in `game/ui/neon/` (see its README). The field guide (`game/ui/field_guide/`) is the first real screen built with it.

The app's other screens still use the old dev theme and are built in code. Convert them **editor-first**, as the field guide was: each screen is a `.tscn` laid out in the editor format, using `neon_theme.tres` and the neon resources, with its script keeping only behaviour. Opened in the editor with no world, each must show plausible sample content, so Wrysk can see and edit the look.

Behaviour, signals, the public API `main.gd` uses, and keys and pad buttons all stay as they are.

## In scope

1. **Pause menu** (`game/ui/pause_menu.gd`). The title, and its items in order, with the disabled states it has now.
2. **Dialog** (`game/ui/dialog.gd`). A confirmation, and the name prompt with its text field. The text field is styled like the field guide's search box.
3. **Notices** (`game/ui/notices.gd`). They keep their fade.
4. **Save slots** (`game/ui/save_slots.gd`). The list with thumbnails, day and species summary; save new, overwrite, rename, delete, load; and the "from another version" state.
5. **Inspect panel** (`game/ui/inspect_panel.gd`). One animal's stats and life history, with facts in the field guide's label/value style.
6. **Small shell pieces in `game/main.gd`.** The paused indicator and the busy panel.
7. **Field guide: both groups collapse.** Wrysk asked why Plants has a collapsible group and Animals doesn't. Give the Animals header the same toggle and chevron as Plants. Start states stay as now: Animals open, Plants closed. Search still opens both.
8. **Backdrop.** Every screen that covers the world uses the neon scrim behind its panel, as the field guide does.

Reuse what exists:
- **Buttons:** NeonButton with main and secondary styles.
- **Rows:** list rows, with selected and focus states.
- **Frames:** notched frames and dithered panels.
- **Text:** chrome labels, title and text sizes.
- **Focus:** glow focus.

Hover still equals focus. Gridlite sizes stay multiples of 13; text sizes are 23, 21 and 18.

## Not in scope: wait for Wrysk

- **New-world picker and Settings.** They use sliders, an option dropdown, a checkbox and progress bars, which aren't designed yet. Leave both screens and their controls exactly as they are.
- **Dev metrics panel.** It stays on the dev theme.
- **Tab shell.** Don't move these screens under the tab strip. Each stays its own overlay, as now.
- **Hover design.**
- **Rust.**
- **Merging.**

## Decision authority

Where the spec and the existing components don't settle something, make the smallest call consistent with the field guide and round 3, and list each one:
- which button style a menu item gets;
- panel widths;
- how a save row lays out its thumbnail.

Don't invent new component looks. If a screen truly needs something the language doesn't have, use the plainest existing piece and list it as an open question.

## Verification

1. `timeout 270 scripts/hidden.sh godot --headless --editor --path game --quit-after 200` reports no errors. Paste any warnings.
2. The existing tests still pass, each under `timeout 270`:
   - `tests/shell_test.gd`
   - `tests/quit_test.gd`
   - `tests/smoke.gd`
   - `tests/guide_test.gd`
   - `tests/picker_test.gd`
   - `tests/metrics_test.gd`

   See each file's header for its command. A test that needs a save needs one founded by main's build: `cargo build --release -p cubarium-session --bin cubarium-world` in your worktree, then `target/release/cubarium-world found --config config/desktop/terrarium.toml --seed 7 --out DIR`. Build into your own `target/`, seeded by reflink from main's `target/release` (only `deps`, `build` and `.fingerprint`), detached if it won't fit in 270 s.
3. Screenshots at 1920×1080 through `scripts/hidden.sh`, with a world running, of:
   - each converted screen;
   - the dialog in both modes;
   - the save slots with at least two slots, one focused;
   - the inspect panel on a live animal;
   - the field guide with Animals collapsed.

   Also screenshot each scene opened in the editor with no world.

Cosmetic UI needs no separate test pass. The screenshots are the verification.

## Return (≤40 lines)

- Commits and files.
- Screenshot paths.
- The test and editor-load output.
- Every judgment call, and any open question.
