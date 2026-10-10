---
status: open
date: 2026-10-10
owner: opus-high, read-only diagnosis (worktree only if a build is needed)
---

# VAULT-COUNT: 440 vaulttrees counted, far fewer seen

Rules: `design/handoffs/worker-rules-2026-10-07.md` (no windows, 5-minute calls, detached
long jobs with a DONE marker, own `target/` seeded by reflink, explicit-path commits, ≤ 40-line
return). Worktrees, targets and copied saves never go under `/tmp` (a RAM disk); put copies
under the main checkout's `runs/vault-count-2026-10-10/` (gitignored).

## Why

Wrysk (2026-10-10): "the world i have running on eidolon says there are 440+ vault trees, but
i dont think i see nearly that many displayed. definitely not unless they are all
practically on top of each other. might create a bit of an ecology issue with stands being
too close so idk what the right move would be to adjust if thats the case."

The world runs in the Godot app on eidolon (`ssh eidolon.local`, fish shell: send scripts
as `ssh eidolon.local bash -s <<'EOF'`), built from **1270d05c** (`~/cubarium-godot/
.deployed-rev`): before the vaulttree's crown radius went from 1.0 to 1.85 m (16684755) and
before plant quarter turns. Its saves are in `~/.local/share/godot/app_userdata/Cubarium/
saves/` (`World (autosave)` is the newest). **Read only: copy the save with scp; never
touch the running app, its files or its saves.**

## Find out (evidence, not guesses)

1. **What the 440 counts**: which number the app shows (find the counter in the Godot
   shell / session code) and what it includes (seedlings, juveniles, adults, dormant or
   dying stands).
2. **The stands in the save**: load the copied save with a build that reads its schema
   (1270d05c in a worktree if main refuses it; build detached if it takes long). Count
   vaulttree stands by growth stage and foliage state; per stage, the nearest-neighbour
   spacing (`crates/cubarium/examples/vault_spacing.rs` measured World 2 this way); where
   they are (faces, and whether the app's view shows those faces).
3. **What is drawn**: how many cells each stand stamps, and how many stands draw nothing
   or almost nothing (a seedling's one shelf, a stand whose cells another stand's crown
   already holds, thinning). A headless picture of the densest grove at the desktop scale
   with the stands marked, if cheap.
4. **The ecology**: what sets how close vaulttree stands establish and survive (the
   seedling gate under a crown, `crown_radius_m`'s shade and competition, dispersal), and
   whether 440 at this spacing is a problem (crowding that starves stands, browse
   pressure, the loftstrider's food). Say what main's 1.85 m crown changes about it.

## Return

The answers to 1–4 with their evidence (numbers, file:line); then the options for "the
right move" if crowding is real, each with its expected visible and ecological effect
(per `budget-before-tuning`: state the predicted number before any change). No config
change, no sim runs beyond loading the save and short measurements, no merge.
