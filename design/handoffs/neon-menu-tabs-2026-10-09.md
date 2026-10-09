---
status: open
date: 2026-10-09
---

# The tabbed page becomes the menu (2026-10-09)

> **Amended the same day:** Wrysk switched to the hybrid. The pause menu stays, with Resume, Save, Load, New world, Field guide, Settings, Dev metrics and Quit; Field guide and Settings open the tabbed page on that tab. The tabbed page has Field guide, Populations (disabled) and Settings, and no World tab. His words: "i think hybrid is probably better. the pause menu should be different. it has shortcuts and 'quit'. probably add other stuff there eventually too.." Where the text below says otherwise, this note wins.

**Rules.** `design/handoffs/worker-rules-2026-10-07.md` applies in full, with AGENTS.md and WORKING_POLICY.md. The hard ones:
- **No windows.** Every Godot run goes through `scripts/hidden.sh`.
- **The 5-minute rule.** Wrap everything in `timeout 270`. Never poll, sleep-loop or wait on a job; anything longer goes detached and you hand back.
- **Never write to the main checkout**, including `game/bin`. Wrysk's app is running from it.
- **Fonts are gitignored.** Copy `/home/wrysk/wryskware/cubarium/game/fonts/*.ttf` into your worktree, then import headless.
- **Commits.** Explicit paths only. Trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Don't merge.

**Main's `game/bin/libcubarium_godot.so` is stale.** It predates commit 80e8d4c2, which added `Leaning` to `config/`, so every new terrain fails with "unknown variant `Leaning`". Build your own extension in your worktree:
1. Seed your own `target/` by reflink (`cp -a --reflink=auto` of main's `target/release/{deps,build,.fingerprint}`).
2. Run `scripts/build-godot-ext.sh --release` in the worktree, under `timeout 270`. If it won't fit, run it detached and hand back.

Never point CARGO_TARGET_DIR at a shared directory. Found test saves with your worktree's `cubarium-world` (`found --config config/desktop/terrarium.toml --seed 7 --out DIR`).

## Objective

Wrysk chose "option 1": the field guide's tabbed page (Field guide / Populations / World / Settings, from UI round 1) becomes the app's one menu, and the pause menu goes away. The neon look is D-0006 (`design/ui/design-language.md`), and every screen is already a neon `.tscn`:
- `game/ui/field_guide/`;
- `game/ui/new_world.tscn`;
- `game/ui/settings.tscn`;
- `game/ui/save_slots.tscn`;
- `game/ui/pause_menu.tscn`.

Keep it **editor-first**: scenes laid out in the editor format, scripts only for behaviour, sample content when opened with no world.

## What it does

- **One shell scene**, e.g. `game/ui/menu/menu.tscn`, holds:
  - the top bar the field guide has now: the tab strip, plus the clock with day, time and Paused;
  - the main panel;
  - the key hints.

  Each tab's content is its own scene, instanced into the panel. Move the field guide's top bar out of `field_guide.tscn` into the shell, so the field guide becomes the first tab's content. Keep its node names and the guide test working.
- **Tabs:**
  - **Field guide:** as now.
  - **Populations:** shown, disabled. It isn't designed yet.
  - **World:**
    - A short summary of the running world on the left: the save name if it has one, day and time, size, terrain, climate, seed. Use the field guide's label/value fact rows and what the session already reports. If a fact isn't available, leave it out.
    - The actions: Save, Load, New world, Quit. Save and Load open the existing save-slots screen, New world opens the existing picker, and Quit does what the pause menu's Quit did. Each of those screens goes back to the World tab.
    - Make the smallest call on button styles, matching the pause menu's current choices, and list it.
  - **Settings:** the settings page's content (Movement, Subpixel) inside the tab, not as its own full screen. Put a secondary "Dev metrics" button at the bottom, so the pad can still reach the dev panel. The dev panel itself stays as it is, and F3 still opens it.
- **Opening and closing:**
  - Start or Esc opens the menu on the last tab used (Field guide the first time) and pauses the world, as the pause menu did.
  - B, Esc or Start closes it and resumes, unless the player had paused with Select or P.
  - G and the pad's Y still open it straight on the Field guide tab without changing the pause state, as now. Inside, Y stays "Show in world".
  - LB and RB switch tabs, and so do the keyboard's Q and E (or the nearest free pair; check `main.gd`'s actions). Show the LB/RB hint now that it does something. A click on a tab switches to it.
- **No running world** (first start, or after a failed load): the menu opens on World, with Save off and only Load, New world and Quit on. Field guide is disabled while there's no world, since it needs one.
- **The old pause menu** (`pause_menu.gd/.tscn`) goes. Remove its uses from `main.gd`. The busy panel, notices, dialog and inspect panel stay as they are.
- **Behaviour carries over:** every action the pause menu had still works, through the World tab and Settings.

## Verification

1. `timeout 270 scripts/hidden.sh godot --headless --editor --path game --quit-after 200` reports no errors.
2. The tests pass, each under `timeout 270`:
   - `tests/shell_test.gd`
   - `tests/guide_test.gd`
   - `tests/picker_test.gd`
   - `tests/smoke.gd`
   - `tests/quit_test.gd`
   - `tests/metrics_test.gd`

   Update the tests that drove the pause menu so they drive the tabs, keeping every check's meaning, and say which. Add checks for:
   - Start opens it on the last tab and pauses;
   - B resumes;
   - LB/RB and Q/E switch tabs;
   - with no world it opens on World with Save disabled;
   - World → New world → back returns to World;
   - Settings changes still apply.
3. Screenshots at 1920×1080 through `scripts/hidden.sh`, with a world running:
   - each tab;
   - World with the actions focused;
   - Settings with Dev metrics;
   - the no-world state.

   Also screenshot the shell scene opened in the editor.

Cosmetic UI needs no separate test pass. The screenshots are the verification.

## Authority and exclusions

- **Smallest calls.** Where this brief and the existing screens are silent, make the smallest call consistent with them, and list each one. Don't invent new looks.
- **Out of scope:**
  - the Populations screen;
  - hover;
  - the dev metrics panel's look;
  - Rust (beyond building the extension for your own runs);
  - merging.

## Return (≤40 lines)

- Commits and files.
- Screenshot paths.
- Test and editor-load output.
- The tests you changed, and why.
- Judgment calls and open questions.
