---
status: open
date: 2026-10-07
---

# Worker rules for the 2026-10-07 dispatch

Every brief dated 2026-10-07 points here. AGENTS.md and WORKING_POLICY.md (repo root) are
the full rules; this is what matters for these packages.

- **Say "Veilhound", never "chorister"**, in replies, docs, commits and the names of files you
  make. `chorister` is only the old code identifier; type it only where code needs it
  (AGENTS.md).
- **No window on Wrysk's screen, ever.** Pictures headless: `--gpu-target headless
  --gpu-capture DIR`. A window that is truly needed goes through `scripts/hidden.sh <cmd>`.
  Browsers run headless (`--headless --screenshot`).
- **No tool call of yours may take longer than 5 minutes.** Wrap builds, tests and runs in
  `timeout 270`.
  - Anything longer is a detached batch: one `setsid nohup` script that runs straight
    through under one long timeout, writes its final table and pictures itself, then a
    `DONE` marker. No chunking into 270 s pieces, no early aborts on zero counters (report
    zeros in the table).
  - Launch it and hand back at once: what runs where, the output and marker paths, what is
    left to do once it finishes, the decisions already made. The 5-minute limit covers
    your own commands, not the detached batch.
  - Never poll, sleep-loop, run Monitor or wait on anything.
- **Your worktree has its own `target/`**, seeded by reflink from main's. Don't point
  `CARGO_TARGET_DIR` at a shared directory.
- **Sim runs** go through `chrt -i 0 nice -n 19 scripts/run-study.py --slot N -- <cmd>` (copy
  `scripts/run-study.py` from the main checkout if your branch lacks it). Check `uptime` and
  other jobs first; other workers run in parallel today, so use one slot each unless the
  brief says otherwise.
- **Tests check mechanisms** on hand-built inputs against the config. Never assert a tuned
  default, ratio, population or verdict; no pinned hashes; nothing steps past a few hundred
  ticks. Run only the crate you touched: `timeout 270 cargo nextest run -p <crate>`.
- **Every ecological number is a knob**: a serde config field with a doc comment (unit and
  meaning) and a registry row. Regenerate `design/parameters.md` with
  `cargo run -p cubarium-voxel-sim --example params -- --write design/parameters.md`.
- **Look**: don't invent it beyond what the brief states (`design/art-direction/`). Rendering
  effects may be smooth; pixel art is the illustration only. Animated effects fade or move
  on the every-frame clock and never pop.
- **Commits**: on your branch, explicit paths only (never `commit -a`). The commit message is
  the report. Never merge, deploy, or touch the services on voidrunner, tachyon or eidolon.
  Codex commits end with `Co-Authored-By: Codex (gpt-6.1-sol) <noreply@openai.com>` and no
  `Claude-Session:` line.
- **Clean up** big temporary outputs you made under `/tmp` (binaries, checkpoints) when done;
  `/tmp` is a RAM disk.
- **Images and video for Wrysk**: absolute paths. Motion is MP4 (x264, yuv444p, crf 12, real
  time), before and after side by side in one file.
- **Return at most 40 lines**: commits, the numbers that matter, the decisions you made and
  why, what looks wrong, absolute paths.
