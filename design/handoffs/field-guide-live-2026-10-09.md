---
status: open
date: 2026-10-09
---

# The field guide, live (2026-10-09)

**Rules.** `design/handoffs/worker-rules-2026-10-07.md` applies in full, with AGENTS.md and WORKING_POLICY.md. The hard ones:
- **No windows.** Every Godot run goes through `scripts/hidden.sh`.
- **The 5-minute rule.** Wrap builds, tests and runs in `timeout 270`. Never poll, sleep-loop or wait on a job. If something needs longer, launch it detached (`setsid nohup … ; touch DONE`) and hand back with what runs where, the marker path, and what is left.
- **Your own `target/`.** Seed it by reflink from main's `target/` (only the profile you build), never a shared `CARGO_TARGET_DIR`. A cold build that won't fit in 270 s goes detached.
- **Never touch main's `game/bin`.** Wrysk runs the app from it. Build the extension into your worktree's `game/bin` (`scripts/build-godot-ext.sh` installs by rename there).
- **Commits.** Explicit paths only, never `commit -a`. Trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Don't merge; the parent merges.
- **No `unsafe`.** No sim behaviour or tuning changes.

## Objective

The neon theme and a static field-guide mock are on main (`game/ui/neon/`, see its README). Wrysk said "yeah get started" on making the field guide real: a screen in the Godot app showing every species with live data from the running world.

He builds the UI **editor-first**: he edits scenes and styles by hand in the Godot editor. So the real screen stays a `.tscn` laid out in the editor format, using the theme and resources in `game/ui/neon/`, with scripts only for behaviour and data. Opened in the editor with no world, it must still show a full sample page (the Lanternjaw), so he can see and edit the look.

Binding look: `design/ui/design-language.md` (D-0006) and `design/ui/round3/index.html` (its defaults are his picks). The mock `game/ui/neon/field_guide_mock.tscn` already matches round 3 in Godot; build from it, don't restyle it.

## Deliverables

### 1. One new sim read for the guide (Rust)

`CubariumSession.query(name, args)` (`crates/cubarium-session/src/session.rs:876`) already serves `populations`, `history` (6 h of 10 s samples: adults, juveniles, eggs per lineage, stands per plant species), `clock` and `species`. Add one query, e.g. `guide`, answered from the **running world's config** (not hardcoded tables), with per species:
- animals: adult body length and height in metres; diet as food classes (`FounderPhysiology::diet`, `Food` in `crates/cubarium-voxel-fauna/src/lib.rs:2547`) with each food's source species where the sim has them (nectar: bloomcrown, latticevine; fruit: lanternberry, latticevine; cap tissue: glowcap; foliage: photosynthetic stands); and prey lineages, from whatever rule the sim uses to decide what a predator may attack;
- plants (and latticevine, which lives in the vine cover layer, `crates/cubarium-voxel-flora/src/cover.rs`): mature height in metres, and `eaten_by`, the animals whose diet takes its tissue or its products.

Find where each fact really lives in the sim. If one isn't there, leave it out and say so; don't invent a number.

**Tests (specified here, before implementation).** Check the mechanism; pin no numbers:
- the answer lists every lineage in `Founder::ALL`, every flora `Species::ALL` and latticevine exactly once, by the names `query("species")` uses;
- each animal's food classes equal its configured diet: change one lineage's diet in a test config and the answer follows;
- every prey name is a lineage name, and `eaten_by` is exactly the inverse of what the animals eat.

### 2. The screen

- **Scene.** Turn the mock into the real screen, e.g. `game/ui/field_guide/field_guide.tscn` plus `field_guide.gd`. Keep the mock's nodes and names; the static Lanternjaw content stays in the `.tscn` as the editor preview. List rows are one small row scene instanced per species, so editing one row edits them all. The gallery keeps working.
- **Species list.** Two groups, Animals and Plants, with counts, in `game/data/field_guide.md`'s order. Plants starts collapsed, as in round 3; the group header opens it. Each row: the `swatch` colour from the notes file, the name, the live count. Typing in the search box filters the list by name.
- **Plate.** Pixelated artbook art, the same recipe as `design/ui/round1/assets/lanternjaw-px.png`:
  ```
  magick art/gen/artbook/X.png -trim +repage -filter box -resize 176x -colors 40 -dither None small.png
  magick small.png -filter point -resize 400% X-px.png
  ```
  Keep the pixel size the same on every plate (3 screen px per art pixel, as now). Tall or wide art is fitted inside the plate box by changing the small size, not by scaling at draw time. Commit the plates (art PNGs are fine in git; fonts are not) and a script that remakes them.

  Use these images:
  - lanternjaw.png, chorister.png, frondgrazer.png, littershredder.png, bellwing.png, loftstrider.png, capgnawer-rose-bark.png, ripple-snail-organic.png, seedporter.png;
  - bloomcrown-pink-preferred.png, umbrellafrond.png, springturf.png, stonecushion.png, velvetpad-provisional.png, glowcap-on-vaulttree.png, vaulttree.png, lanternberry-blue.png, siphonreed.png.

  Latticevine has no art: show the empty plate backdrop. Caption: "Plate NN · Adult" (animals) or "Plate NN · Mature" (plants), NN in list order. Some artbook images still have painted halos; don't fix art, just list any plate that looks wrong.
- **Live count.** A big number with a sub line. For animals: adults plus juveniles, with "N adults · N juveniles". For plants: stands (latticevine: whatever count the vine layer gives, labelled honestly). The bar chart is 36 bars over the last 6 h of world time from `history`. If the world is younger than 6 h, the missing bars stay empty.
- **Entry.** The kicker is `kind · kicker` from the notes and the name is in the title font. No Latin-name line (Latin names stay off the page). Then the rule and the field note. The entry scrolls above the fixed buttons, as in the mock.
- **Facts.**
  - Animals: Role (notes), Body ("1.0 m long · 0.45 m tall", from the sim), the habit row (notes; label from `habit_label`, default Habit), and Eats (chips: prey species with their swatch, then food classes with the labels colour).
  - Plants: Role, Height (sim), the habit row (default label Grows), and Eaten by (chips).

  Chips wrap.
- **Clock.** Day and time from `clock`. The Paused label shows when the world is paused. The guide doesn't change the pause state.
- **Buttons and keys.**
  - "Show in world" (Y): for an animal, close the guide, pan to one living individual of that species and select it the way a click does (highlight and inspect panel). For a plant, pan to a stand if the view feed has stand positions; otherwise disable it.
  - "Populations" (X): disabled. That screen isn't designed yet.
- **Tabs and hints.** The tab strip shows all four tabs. Field guide is selected; the other three are disabled until those screens are restyled. Hide the LB/RB hint while it does nothing.
- **Opening it.** Open from a "Field guide" item in the pause menu, a key (G) and a free gamepad button (check `game/main.gd:127` for what's taken). Close with B or Esc, back to wherever it was opened from, the way the dev metrics panel does (`game/main.gd`, `game/ui/dev/metrics_panel.gd`).
- **Refresh.** Ask the session about once a second, only while the guide is open.

## Verification

1. `timeout 270 scripts/hidden.sh godot --headless --editor --path game --quit-after 200`: the scenes load with no errors. Paste the warnings.
2. `cargo test` for the new query (its own tests plus the session crate's), under `timeout 270`.
3. Screenshots with a real world running, through `scripts/hidden.sh` at 1920×1080 (see `game/tests/neon_shots.gd` and `game/tests/shots.gd`):
   - an animal page (Lanternjaw);
   - a plant page (Vaulttree);
   - latticevine;
   - the list with Plants open;
   - the search filtering.

   Also take one of the editor preview with no world. Compare the Lanternjaw page side by side with round 3 (`#shot&view=screen`) and list the differences.
4. Open from the pause menu, the key and the pad button, and close with back. A scripted check is fine.

## Authority and exclusions

- **Smallest call.** Where this brief and round 3 are silent, make the smallest call that matches round 3, and list each one.
- **Out of scope:**
  - the populations screen;
  - restyling other menus;
  - hover design;
  - settings controls;
  - Latin names;
  - new art or art cleanup;
  - sim behaviour or tuning;
  - the dev metrics panel;
  - merging.

## Return (≤40 lines)

- Commits and files.
- What the new query returns, with one sample entry, and where each fact comes from in the sim (file:line).
- Screenshot paths: the round 3 side-by-side and the list of differences.
- The test and editor-load output.
- Every judgment call, and the open questions: plates that look wrong, facts the sim doesn't have, and the pad button chosen.
- Whether Wrysk must rebuild the extension to see it (he must, if the Rust changed): the exact command.
