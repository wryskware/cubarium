---
status: open
date: 2026-10-09
---

# Neon settings controls, New world and Settings (2026-10-09)

**Rules.** `design/handoffs/worker-rules-2026-10-07.md` applies in full, with AGENTS.md and WORKING_POLICY.md. The hard ones:
- **No windows.** Every Godot run goes through `scripts/hidden.sh`.
- **The 5-minute rule.** Wrap everything in `timeout 270`. Never poll, sleep-loop or wait on a job; anything longer goes detached and you hand back.
- **Never touch main's `game/bin`** or anything else in the main checkout. Copy main's `game/bin/libcubarium_godot.so` into your worktree for runs. No Rust changes.
- **Fonts are gitignored.** Copy `/home/wrysk/wryskware/cubarium/game/fonts/*.ttf` into your worktree, then import headless.
- **Commits.** Explicit paths only. Trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Don't merge.

## Objective

Wrysk picked the settings controls in UI round 4 (`design/ui/round4/index.html`; its defaults are his picks): "Arrows, Dot Slider, Checkbox", and the dot row for progress. The spec is `design/ui/design-language.md`, section "Settings controls (round 4)". Its values bind.

Build the four controls as editor-first neon components. Then convert the New world picker and the Settings page to the neon style with them, the same way the field guide was built (`game/ui/field_guide/`, `game/ui/neon/README.md`).

A parallel package (`design/handoffs/neon-menus-2026-10-09.md`) is converting the pause menu, dialog, notices, save slots and inspect panel. Don't touch those files. In `game/main.gd`, change only the lines that create the picker and the settings page, so the two branches merge cleanly.

## Deliverables

1. **Components in `game/ui/neon/`.** Each is a `@tool` control with a `class_name`, with every value an exported Inspector field, drawing in the editor:
   - **Arrows choice.** Options, index, `changed` signal. Left and right (keys and pad) step through the options; a click on either arrow steps too. Whether it wraps at the ends: make the smallest call and list it. Pips show the position. Focus is the glow around the box.
   - **Dot slider.** Stop names, index, `changed` signal. Left and right move between stops; a click or drag on the track snaps to the nearest stop. It also works with two stops.
   - **Checkbox.** Text, a pressed state, `toggled` signal. A or Enter, or a click, toggles it.
   - **Dot-row progress.** A value from 0 to 1, a percentage label that can be switched off, and a small variant.

   Add all four, in every state, to `gallery.tscn`. Add a short section to the neon README.
2. **New world picker** (`game/ui/new_world.gd`, 769 lines). A `.tscn` laid out in the editor format with the neon theme and scrim, matching round 4's "New world" and "Founding" views:
   - size, terrain and climate as arrows;
   - water as the dot slider (Low / Medium / High);
   - the seed field styled like the field guide's search box;
   - terrain cards in notched inner boxes with the small dot-row progress while drawing;
   - founding with the large dot row, its phase line and detail.

   Every behaviour stays as it is: drawing, reroll, open large, cancel, the water slider re-wetting the same terrains, the founding progress, and the failure message. Opened in the editor with no world, it shows sample content.
3. **Settings page** (`game/ui/settings.gd`). A `.tscn` matching round 4's "Settings" view:
   - Movement as a two-stop dot slider (Smooth / Stepped);
   - Subpixel as a checkbox, disabled when Stepped, as now;
   - the notes in small labels-coloured text.

   Behaviour and the `changed` signal stay as they are.

## Verification

1. `timeout 270 scripts/hidden.sh godot --headless --editor --path game --quit-after 200` reports no errors. Paste any warnings.
2. The existing tests still pass, each under `timeout 270` (see each file's header):
   - `tests/picker_test.gd`
   - `tests/shell_test.gd`
   - `tests/smoke.gd`
   - `tests/quit_test.gd`
   - `tests/found_timing.gd`, if it fits.

   If a test reached into the old code-built nodes, update it to the new nodes without weakening what it checks. Say which, and why.
3. Screenshots at 1920×1080 through `scripts/hidden.sh`:
   - the picker with cards drawing and one focused;
   - the picker founding;
   - Settings with Subpixel focused;
   - Settings with Stepped (Subpixel disabled);
   - the gallery's new section;
   - each new scene in the editor with no world.

   `game/tests/picker_shots.gd` may already do part of this. Compare them side by side with round 4 in headless Chrome (`#shot&view=newworld`, `founding`, `settings`) and list the differences.

Cosmetic UI needs no separate test pass. The screenshots are the verification.

## Authority and exclusions

- **Smallest calls.** Make the smallest call consistent with round 4 and the field guide, and list each one.
- **Out of scope:**
  - the files of the parallel menus package;
  - the dev metrics panel;
  - the tab shell;
  - hover design;
  - Rust;
  - new looks;
  - merging.

## Return (≤40 lines)

- Commits and files.
- Where each control's values are edited in the Inspector.
- Screenshot paths and the differences from round 4.
- Test and editor-load output.
- Judgment calls and open questions.
